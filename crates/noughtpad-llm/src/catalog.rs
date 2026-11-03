//! Local models Noughtpad offers to download. Each is pinned to a revision and
//! checked against its SHA-256 after download.
//!
//! Only models that passed the grounding evaluation on the sample notebook
//! are listed (they cite the notes they use and stay within them); see
//! `docs/DESIGN.md` § M4.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalModel {
    /// Stable id used in settings and as the folder name.
    pub id: &'static str,
    pub name: &'static str,
    pub maker: &'static str,
    pub licence: &'static str,
    /// Hugging Face repository and pinned commit.
    pub repo: &'static str,
    pub revision: &'static str,
    pub file: &'static str,
    pub sha256: &'static str,
    pub bytes: u64,
    /// Memory needed while answering, roughly.
    pub ram_gb: f32,
    /// One line on what it is good for.
    pub blurb: &'static str,
    pub recommended: bool,
}

impl LocalModel {
    pub fn url(&self) -> String {
        format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            self.repo, self.revision, self.file
        )
    }
}

pub const CATALOG: &[LocalModel] = &[
    LocalModel {
        id: "qwen3-4b-instruct-2507-q4km",
        name: "Qwen3 4B Instruct",
        maker: "Alibaba Qwen",
        licence: "Apache-2.0",
        repo: "unsloth/Qwen3-4B-Instruct-2507-GGUF",
        revision: "a06e946bb6b655725eafa393f4a9745d460374c9",
        file: "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
        sha256: "3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597",
        bytes: 2_497_281_120,
        ram_gb: 4.0,
        blurb: "Best balance: cites your notes reliably and runs on most laptops.",
        recommended: true,
    },
    LocalModel {
        id: "qwen3-1.7b-q4km",
        name: "Qwen3 1.7B",
        maker: "Alibaba Qwen",
        licence: "Apache-2.0",
        repo: "unsloth/Qwen3-1.7B-GGUF",
        revision: "d7f544eead698dbd1f15126ef60b45a1e1933222",
        file: "Qwen3-1.7B-Q4_K_M.gguf",
        sha256: "b139949c5bd74937ad8ed8c8cf3d9ffb1e99c866c823204dc42c0d91fa181897",
        bytes: 1_107_409_472,
        ram_gb: 2.0,
        blurb: "Smaller and faster, for older machines. Answers are simpler.",
        recommended: false,
    },
    LocalModel {
        id: "qwen3.5-4b-q4km",
        name: "Qwen3.5 4B",
        maker: "Alibaba Qwen",
        licence: "Apache-2.0",
        repo: "unsloth/Qwen3.5-4B-GGUF",
        revision: "e87f176479d0855a907a41277aca2f8ee7a09523",
        file: "Qwen3.5-4B-Q4_K_M.gguf",
        sha256: "00fe7986ff5f6b463e62455821146049db6f9313603938a70800d1fb69ef11a4",
        bytes: 2_740_937_888,
        ram_gb: 4.5,
        blurb: "Newer model; noticeably slower without a GPU.",
        recommended: false,
    },
];

pub fn find(id: &str) -> Option<&'static LocalModel> {
    CATALOG.iter().find(|m| m.id == id)
}

pub fn recommended() -> &'static LocalModel {
    CATALOG.iter().find(|m| m.recommended).unwrap_or(&CATALOG[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_well_formed() {
        assert_eq!(CATALOG.iter().filter(|m| m.recommended).count(), 1);
        for m in CATALOG {
            assert_eq!(m.sha256.len(), 64, "{}", m.id);
            assert_eq!(m.revision.len(), 40, "{}", m.id);
            assert!(m.file.ends_with(".gguf"));
            assert_eq!(find(m.id), Some(m));
        }
        let mut ids: Vec<_> = CATALOG.iter().map(|m| m.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), CATALOG.len());
    }
}
