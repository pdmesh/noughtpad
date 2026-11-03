//! The default local embedding model and how to fetch and load it.
//!
//! The default embedding model is downloaded once, on explicit opt-in, into
//! the app's data folder, never into the vault. Only model files travel over the network;
//! notes never leave the machine.

use super::embedder::ModelSpec;

/// BGE small English v1.5, statically quantised ONNX export (MIT licence).
/// 384 dimensions, ~65 MB, CLS pooling.
pub const BGE_SMALL_EN_V15_Q: ModelSpec = ModelSpec {
    id: "bge-small-en-v1.5-q",
    dims: 384,
    query_prefix: "",
    passage_prefix: "",
    // Measured (examples/semantic.rs): related note pairs 0.66–0.88, unrelated
    // up to ~0.57. On the 37-note sample notebook, short notes on different
    // topics scored 0.60–0.63 (a run log next to a bread note), and no
    // same-topic suggestion was below 0.63. Favours precision: a missed link is
    // better than a false one. Re-tune on real notebooks.
    min_related: 0.63,
    strong_related: 0.78,
    min_search: 0.55,
};

pub struct ModelSource {
    pub spec: ModelSpec,
    /// Hugging Face repository the files come from.
    pub repo: &'static str,
    pub onnx_file: &'static str,
    pub tokenizer_files: [&'static str; 4],
    /// Approximate download size, for progress before sizes are known.
    pub approx_bytes: u64,
}

pub const DEFAULT_MODEL: ModelSource = ModelSource {
    spec: BGE_SMALL_EN_V15_Q,
    repo: "Qdrant/bge-small-en-v1.5-onnx-Q",
    onnx_file: "model_optimized.onnx",
    tokenizer_files: TOKENIZER_FILES,
    approx_bytes: 67_000_000,
};

const TOKENIZER_FILES: [&str; 4] = [
    "tokenizer.json",
    "config.json",
    "special_tokens_map.json",
    "tokenizer_config.json",
];

/// Something whose files can be fetched from a Hugging Face repository.
pub trait ModelFiles {
    fn repo(&self) -> &'static str;
    fn remote_files(&self) -> Vec<&'static str>;
    fn approx_bytes(&self) -> u64;

    fn is_downloaded(&self, dir: &std::path::Path) -> bool {
        self.remote_files()
            .iter()
            .all(|f| dir.join(local_name(f)).metadata().is_ok_and(|m| m.len() > 0))
    }
}

/// Files are stored flat: `onnx/model_int8.onnx` becomes `model_int8.onnx`.
pub fn local_name(remote: &str) -> &str {
    remote.rsplit('/').next().unwrap_or(remote)
}

impl ModelFiles for ModelSource {
    fn repo(&self) -> &'static str {
        self.repo
    }
    fn remote_files(&self) -> Vec<&'static str> {
        std::iter::once(self.onnx_file).chain(self.tokenizer_files).collect()
    }
    fn approx_bytes(&self) -> u64 {
        self.approx_bytes
    }
}

/// Where the default embedding model lives under a models directory.
pub fn model_dir(models: &std::path::Path) -> std::path::PathBuf {
    models.join(DEFAULT_MODEL.spec.id)
}

pub fn bundle_downloaded(models: &std::path::Path) -> bool {
    DEFAULT_MODEL.is_downloaded(&model_dir(models))
}

#[cfg(feature = "download")]
pub use download::{download, download_bundle};

#[cfg(feature = "download")]
mod download {
    use std::io::{Read, Write};
    use std::path::Path;

    use super::{local_name, model_dir, ModelFiles, DEFAULT_MODEL};
    use crate::error::IoContext;
    use crate::{Error, Result};

    /// Download any missing files of one model into `dir`. `progress(received, total)`.
    pub fn download(src: &impl ModelFiles, dir: &Path, mut progress: impl FnMut(u64, u64)) -> Result<()> {
        let mut received = 0;
        fetch(src, dir, &mut received, src.approx_bytes(), &mut progress)
    }

    /// Download the default bundle (currently the embedding model)
    /// under `models`, reporting combined progress. Already-present files are skipped.
    pub fn download_bundle(models: &Path, mut progress: impl FnMut(u64, u64)) -> Result<()> {
        let e = model_dir(models);
        let parts: [(&dyn ModelFiles, &Path); 1] = [(&DEFAULT_MODEL, &e)];
        let total: u64 = parts
            .iter()
            .filter(|(m, d)| !m.is_downloaded(d))
            .map(|(m, _)| m.approx_bytes())
            .sum();
        let mut received = 0;
        for (m, d) in parts {
            fetch(m, d, &mut received, total, &mut progress)?;
        }
        Ok(())
    }

    fn fetch(
        src: &(impl ModelFiles + ?Sized),
        dir: &Path,
        received: &mut u64,
        total: u64,
        progress: &mut impl FnMut(u64, u64),
    ) -> Result<()> {
        std::fs::create_dir_all(dir).at(dir)?;
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .proxy(ureq::Proxy::try_from_env())
            .build()
            .into();
        for file in src.remote_files() {
            let dest = dir.join(local_name(file));
            if dest.metadata().is_ok_and(|m| m.len() > 0) {
                continue;
            }
            let url = format!("https://huggingface.co/{}/resolve/main/{file}", src.repo());
            let resp = agent
                .get(&url)
                .call()
                .map_err(|e| Error::Embedding(format!("downloading {file}: {e}")))?;
            let part = dir.join(format!("{}.part", local_name(file)));
            let mut out = std::fs::File::create(&part).at(&part)?;
            let mut body = resp.into_body().into_reader();
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                let n = body
                    .read(&mut buf)
                    .map_err(|e| Error::Embedding(format!("downloading {file}: {e}")))?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n]).at(&part)?;
                *received += n as u64;
                progress(*received, total.max(*received));
            }
            out.sync_all().at(&part)?;
            std::fs::rename(&part, &dest).at(&dest)?;
        }
        Ok(())
    }
}

#[cfg(feature = "embedding")]
pub use fast::FastEmbedder;

#[cfg(feature = "embedding")]
mod fast {
    use std::path::Path;

    use fastembed::{InitOptionsUserDefined, Pooling, TextEmbedding, TokenizerFiles, UserDefinedEmbeddingModel};

    use super::super::embedder::{normalise, Embedder, ModelSpec, TextKind};
    use super::ModelSource;
    use crate::error::IoContext;
    use crate::{Error, Result};

    /// ONNX Runtime embedder via fastembed.
    pub struct FastEmbedder {
        spec: ModelSpec,
        model: TextEmbedding,
    }

    impl FastEmbedder {
        /// Load a downloaded model. `threads` caps CPU use for background work.
        pub fn load(src: &ModelSource, dir: &Path, threads: usize) -> Result<Self> {
            let read = |f: &str| std::fs::read(dir.join(f)).at(dir.join(f));
            let [tokenizer, config, special, tokenizer_config] = src.tokenizer_files;
            let files = TokenizerFiles {
                tokenizer_file: read(tokenizer)?,
                config_file: read(config)?,
                special_tokens_map_file: read(special)?,
                tokenizer_config_file: read(tokenizer_config)?,
            };
            let model = UserDefinedEmbeddingModel::new(read(src.onnx_file)?, files)
                .with_pooling(Pooling::Cls)
                .with_quantization(fastembed::QuantizationMode::Static);
            let options = InitOptionsUserDefined::new()
                .with_max_length(512)
                .with_intra_threads(threads.max(1));
            let model = TextEmbedding::try_new_from_user_defined(model, options)
                .map_err(|e| Error::Embedding(e.to_string()))?;
            Ok(FastEmbedder {
                spec: src.spec.clone(),
                model,
            })
        }
    }

    impl Embedder for FastEmbedder {
        fn spec(&self) -> &ModelSpec {
            &self.spec
        }

        fn embed(&mut self, texts: &[String], kind: TextKind) -> Result<Vec<Vec<f32>>> {
            let prefix = match kind {
                TextKind::Query => self.spec.query_prefix,
                TextKind::Passage => self.spec.passage_prefix,
            };
            let inputs: Vec<String> = texts.iter().map(|t| format!("{prefix}{t}")).collect();
            let mut out = self
                .model
                .embed(&inputs, Some(16))
                .map_err(|e| Error::Embedding(e.to_string()))?;
            out.iter_mut().for_each(|v| normalise(v));
            Ok(out)
        }
    }
}

/// End-to-end check against the real model (needs network for the first run).
/// `cargo test -p noughtpad-core --features embedding-static -- --ignored model_`
#[cfg(all(test, feature = "embedding"))]
mod model_tests {
    use super::super::embedder::{dot, Embedder, TextKind};
    use super::*;

    #[test]
    #[ignore = "downloads the model (~65 MB)"]
    fn model_thresholds_separate_related_from_unrelated() {
        let dir = std::env::var_os("NOUGHTPAD_MODEL_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("noughtpad-model-test"));
        download(&DEFAULT_MODEL, &dir, |_, _| {}).unwrap();
        let mut m = FastEmbedder::load(&DEFAULT_MODEL, &dir, 2).unwrap();
        let texts: Vec<String> = [
            "Venture returns follow a power law: a few outlier investments return the whole fund.",
            "A fund can have many mediocre investments and still do well if one company becomes enormous.",
            "Ownership determines how much of a company's value accrues to the fund.",
            "Book a ryokan in Kyoto near the temples for the spring trip.",
            "Customers describe what they want rather than what they actually do.",
        ]
        .map(String::from)
        .to_vec();
        let v = m.embed(&texts, TextKind::Passage).unwrap();
        let s = |a: usize, b: usize| dot(&v[a], &v[b]);
        let spec = m.spec().clone();
        eprintln!(
            "related: {:.3} {:.3} | unrelated: {:.3} {:.3} {:.3}",
            s(0, 1),
            s(0, 2),
            s(0, 3),
            s(1, 3),
            s(0, 4)
        );
        assert!(s(0, 1) >= spec.min_related, "related pair below threshold");
        for (a, b) in [(0, 3), (1, 3), (2, 3), (0, 4)] {
            assert!(
                s(a, b) < spec.min_related,
                "unrelated pair {a},{b} above threshold: {}",
                s(a, b)
            );
        }
        let q = m
            .embed(&["how much of the fund comes from big winners".into()], TextKind::Query)
            .unwrap();
        assert!(dot(&q[0], &v[0]) > dot(&q[0], &v[3]) + 0.1);
    }
}
