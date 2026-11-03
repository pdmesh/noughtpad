//! The default local embedding model and how to fetch and load it.
//!
//! The default bundle (the embedding model) is downloaded once, on explicit
//! opt-in, into the app's data folder, never into the vault. Optional judge
//! models (reranker, NLI) are used only if installed. Only model files travel over the network;
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
    // up to ~0.57. 0.60 favours precision: a missed link is better than a
    // false one. Re-tune on real notebooks.
    min_related: 0.60,
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

/// A cross-encoder "judge" model (reads two texts together, outputs scores).
pub struct JudgeFiles {
    /// Folder name under the models directory.
    pub id: &'static str,
    pub repo: &'static str,
    /// Path inside the repository; stored locally under its file name.
    pub onnx_file: &'static str,
    pub tokenizer_files: [&'static str; 4],
    pub approx_bytes: u64,
}

/// Jina reranker v1 turbo (English), int8 ONNX, Apache-2.0. ~38 MB.
pub const RERANKER: JudgeFiles = JudgeFiles {
    id: "jina-reranker-v1-turbo-en-int8",
    repo: "jinaai/jina-reranker-v1-turbo-en",
    onnx_file: "onnx/model_int8.onnx",
    tokenizer_files: TOKENIZER_FILES,
    approx_bytes: 40_500_000,
};

/// DeBERTa-v3-xsmall trained for natural-language inference
/// (contradiction / entailment / neutral), Apache-2.0, int8 build for this
/// CPU family. ~87 MB.
pub const NLI: JudgeFiles = JudgeFiles {
    id: "nli-deberta-v3-xsmall-int8",
    repo: "cross-encoder/nli-deberta-v3-xsmall",
    onnx_file: if cfg!(target_arch = "aarch64") {
        "onnx/model_qint8_arm64.onnx"
    } else {
        "onnx/model_quint8_avx2.onnx"
    },
    tokenizer_files: TOKENIZER_FILES,
    approx_bytes: 96_500_000,
};

/// Thresholds for the default judges, measured with `examples/judges.rs` on the
/// sample notebook. Relevance is the reranker score averaged over both
/// directions: related pairs 0.22–0.63, unrelated ≤ 0.12 (a few look-alikes
/// up to ~0.20). The NLI thresholds are deliberately strict.
pub const DEFAULT_JUDGE_SPEC: super::judge::JudgeSpec = super::judge::JudgeSpec {
    min_relevance: 0.22,
    strong_relevance: 0.45,
    contradiction: 0.95,
    entailment: 0.90,
};

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

impl ModelFiles for JudgeFiles {
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

/// Where each model of the default bundle lives under a models directory.
pub fn bundle_dirs(models: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    (
        models.join(DEFAULT_MODEL.spec.id),
        models.join(RERANKER.id),
        models.join(NLI.id),
    )
}

/// The default bundle is the embedding model. Judge models (reranker, NLI)
/// are optional: measured on the sample notebook they did not improve on
/// embeddings + rules (see docs/DESIGN.md, M3.5), so they are only used when
/// present and never downloaded by default.
pub fn bundle_downloaded(models: &std::path::Path) -> bool {
    DEFAULT_MODEL.is_downloaded(&bundle_dirs(models).0)
}

/// Whether the optional reranker is installed (judges are loaded only then).
pub fn judge_installed(models: &std::path::Path) -> bool {
    RERANKER.is_downloaded(&bundle_dirs(models).1)
}

#[cfg(feature = "download")]
pub use download::{download, download_bundle, download_optional};

#[cfg(feature = "download")]
mod download {
    use std::io::{Read, Write};
    use std::path::Path;

    use super::{bundle_dirs, local_name, ModelFiles, DEFAULT_MODEL};
    use crate::error::IoContext;
    use crate::{Error, Result};

    /// Download any missing files of one model into `dir`. `progress(received, total)`.
    pub fn download(src: &impl ModelFiles, dir: &Path, mut progress: impl FnMut(u64, u64)) -> Result<()> {
        let mut received = 0;
        fetch(src, dir, &mut received, src.approx_bytes(), &mut progress)
    }

    /// Download an optional model (e.g. the NLI model) into its folder under `models`.
    pub fn download_optional(src: &super::JudgeFiles, models: &Path, progress: impl FnMut(u64, u64)) -> Result<()> {
        download(src, &models.join(src.id), progress)
    }

    /// Download the default bundle (currently the embedding model)
    /// under `models`, reporting combined progress. Already-present files are skipped.
    pub fn download_bundle(models: &Path, mut progress: impl FnMut(u64, u64)) -> Result<()> {
        let (e, _, _) = bundle_dirs(models);
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
pub use fast::{FastEmbedder, FastJudge};

#[cfg(feature = "embedding")]
mod fast {
    use std::path::Path;

    use fastembed::{InitOptionsUserDefined, Pooling, TextEmbedding, TokenizerFiles, UserDefinedEmbeddingModel};

    use super::super::embedder::{normalise, Embedder, ModelSpec, TextKind};
    use super::ModelSource;
    use crate::error::IoContext;
    use crate::{Error, Result};

    /// ONNX Runtime allows one environment per process; all models share it.
    fn environment() -> Result<ort::environment::Environment> {
        static ENV: std::sync::OnceLock<ort::environment::Environment> = std::sync::OnceLock::new();
        if let Some(e) = ENV.get() {
            return Ok(e.clone());
        }
        let env = ort::init()
            .with_name("nought")
            .build()
            .map_err(|e| Error::Embedding(e.to_string()))?;
        Ok(ENV.get_or_init(|| env).clone())
    }

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
                .with_environment(environment()?)
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

    /// Reranker (fastembed) + natural-language inference model (ONNX Runtime).
    pub struct FastJudge {
        reranker: fastembed::TextRerank,
        nli: Option<NliModel>,
    }

    struct NliModel {
        session: ort::session::Session,
        tokenizer: tokenizers::Tokenizer,
        token_types: bool,
    }

    fn tokenizer_files(dir: &Path, files: [&str; 4]) -> Result<TokenizerFiles> {
        let read = |f: &str| std::fs::read(dir.join(f)).at(dir.join(f));
        let [tokenizer, config, special, tokenizer_config] = files;
        Ok(TokenizerFiles {
            tokenizer_file: read(tokenizer)?,
            config_file: read(config)?,
            special_tokens_map_file: read(special)?,
            tokenizer_config_file: read(tokenizer_config)?,
        })
    }

    impl FastJudge {
        pub fn load(models: &Path, threads: usize) -> Result<Self> {
            use super::{bundle_dirs, local_name, NLI, RERANKER};
            let err = |e: &dyn std::fmt::Display| Error::Embedding(e.to_string());
            let (_, rdir, ndir) = bundle_dirs(models);

            let onnx = std::fs::read(rdir.join(local_name(RERANKER.onnx_file))).at(&rdir)?;
            let model =
                fastembed::UserDefinedRerankingModel::new(onnx, tokenizer_files(&rdir, RERANKER.tokenizer_files)?);
            let reranker = fastembed::TextRerank::try_new_from_user_defined(
                model,
                fastembed::RerankInitOptionsUserDefined::new()
                    .with_environment(environment()?)
                    .with_max_length(512)
                    .with_intra_threads(threads.max(1)),
            )
            .map_err(|e| err(&e))?;

            // The NLI model is optional: loaded only if it was downloaded.
            let nli = if super::ModelFiles::is_downloaded(&NLI, &ndir) {
                let bytes = std::fs::read(ndir.join(local_name(NLI.onnx_file))).at(&ndir)?;
                let session = ort::session::Session::builder(&environment()?)
                    .map_err(|e| err(&e))?
                    .with_intra_threads(threads.max(1))
                    .map_err(|e| err(&e))?
                    .commit_from_memory(&bytes)
                    .map_err(|e| err(&e))?;
                let token_types = session.inputs().iter().any(|i| i.name() == "token_type_ids");
                let mut tokenizer =
                    tokenizers::Tokenizer::from_file(ndir.join("tokenizer.json")).map_err(|e| err(&e))?;
                tokenizer
                    .with_truncation(Some(tokenizers::TruncationParams {
                        max_length: 512,
                        ..Default::default()
                    }))
                    .map_err(|e| err(&e))?;
                tokenizer.with_padding(Some(tokenizers::PaddingParams::default()));
                Some(NliModel {
                    session,
                    tokenizer,
                    token_types,
                })
            } else {
                None
            };
            Ok(FastJudge { reranker, nli })
        }
    }

    impl super::super::judge::Judge for FastJudge {
        fn has_nli(&self) -> bool {
            self.nli.is_some()
        }

        fn rerank(&mut self, query: &str, docs: &[String]) -> Result<Vec<f32>> {
            if docs.is_empty() {
                return Ok(vec![]);
            }
            let refs: Vec<&str> = docs.iter().map(String::as_str).collect();
            let results = self
                .reranker
                .rerank(query, refs, false, Some(16))
                .map_err(|e| Error::Embedding(e.to_string()))?;
            let mut out = vec![0f32; docs.len()];
            for r in results {
                out[r.index] = super::super::judge::sigmoid(r.score);
            }
            Ok(out)
        }

        fn nli(&mut self, pairs: &[(String, String)]) -> Result<Vec<super::super::judge::Nli>> {
            use super::super::judge::{softmax3, Nli};
            let err = |e: &dyn std::fmt::Display| Error::Embedding(e.to_string());
            let Some(m) = self.nli.as_mut() else {
                return Err(Error::Embedding("no inference model installed".into()));
            };
            let mut out = Vec::with_capacity(pairs.len());
            for batch in pairs.chunks(8) {
                let inputs: Vec<tokenizers::EncodeInput> =
                    batch.iter().map(|(p, h)| (p.as_str(), h.as_str()).into()).collect();
                let enc = m.tokenizer.encode_batch(inputs, true).map_err(|e| err(&e))?;
                let (rows, cols) = (enc.len(), enc[0].len());
                let flat = |f: &dyn Fn(&tokenizers::Encoding) -> &[u32]| -> Vec<i64> {
                    enc.iter().flat_map(|e| f(e).iter().map(|&x| x as i64)).collect()
                };
                let ids = ndarray::Array2::from_shape_vec((rows, cols), flat(&|e| e.get_ids())).map_err(|e| err(&e))?;
                let mask = ndarray::Array2::from_shape_vec((rows, cols), flat(&|e| e.get_attention_mask()))
                    .map_err(|e| err(&e))?;
                let mut inputs = ort::inputs![
                    "input_ids" => ort::value::Tensor::from_array(ids).map_err(|e| err(&e))?,
                    "attention_mask" => ort::value::Tensor::from_array(mask).map_err(|e| err(&e))?,
                ];
                if m.token_types {
                    let types = ndarray::Array2::from_shape_vec((rows, cols), flat(&|e| e.get_type_ids()))
                        .map_err(|e| err(&e))?;
                    inputs.push((
                        "token_type_ids".into(),
                        ort::value::Tensor::from_array(types).map_err(|e| err(&e))?.into(),
                    ));
                }
                let outputs = m.session.run(inputs).map_err(|e| err(&e))?;
                let logits = outputs[0].try_extract_array::<f32>().map_err(|e| err(&e))?;
                for r in 0..rows {
                    // Label order from the model config: contradiction, entailment, neutral.
                    let p = softmax3([logits[[r, 0]], logits[[r, 1]], logits[[r, 2]]]);
                    out.push(Nli {
                        contradict: p[0],
                        entail: p[1],
                        neutral: p[2],
                    });
                }
            }
            Ok(out)
        }
    }
}

/// End-to-end check against the real model (needs network for the first run).
/// `cargo test -p nought-core --features embedding-static -- --ignored model_`
#[cfg(all(test, feature = "embedding"))]
mod model_tests {
    use super::super::embedder::{dot, Embedder, TextKind};
    use super::*;

    #[test]
    #[ignore = "downloads the model (~65 MB)"]
    fn model_thresholds_separate_related_from_unrelated() {
        let dir = std::env::var_os("NOUGHT_MODEL_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("nought-model-test"));
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
