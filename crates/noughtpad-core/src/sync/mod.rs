//! The open half of Noughtpad Sync: what syncs, the [`SyncBackend`] interface
//! a remote store implements, a [`FolderBackend`] used by tests, and the
//! [`Journal`] of paths changed on this device. The engine that merges and
//! the Google Drive and iCloud backends live in the private `noughtpad-sync`
//! crate. Design: `docs/DESIGN.md` §12.
//!
//! A backend stores a notebook's files by notebook-relative path (`/`
//! separated, as the vault uses) and reports what changed. It knows nothing
//! about Markdown or merging. The one hard rule is in [`SyncBackend::put`]:
//! write only if the remote is still at the revision the engine last saw.

mod folder;
mod journal;

pub use folder::FolderBackend;
pub use journal::{Journal, Pending, JOURNAL_FILE};

use serde::{Deserialize, Serialize};

use crate::Result;

/// Folder inside `.noughtpad/` for this device's sync state; never synced.
pub const SYNC_DIR: &str = "sync";

/// Folders inside `.noughtpad/` that sync (DESIGN §12.2). Everything else
/// there (the index, sync state, the old user-state file) stays on the device.
pub const SYNCED_DATA_DIRS: [&str; 4] = ["chats", "trash", "user-state", "tombstones"];

/// Whether a notebook-relative path is synced: notes and any other file in
/// the notebook, plus chats, trash, user-state logs and tombstones. Hidden
/// folders (`.git`, `.obsidian`), temporary files and OS clutter are not.
pub fn is_synced(rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.iter().any(|p| p.is_empty() || *p == "." || *p == "..") {
        return false;
    }
    let name = parts[parts.len() - 1];
    if name == ".DS_Store"
        || name == "Thumbs.db"
        || name == "desktop.ini"
        || name.starts_with(".noughtpad-tmp-")
        || name.ends_with(".tmp")
    {
        return false;
    }
    let rest = if parts[0] == crate::vault::DATA_DIR {
        match parts.get(1) {
            Some(dir) if parts.len() > 2 && SYNCED_DATA_DIRS.contains(dir) => &parts[2..],
            _ => return false,
        }
    } else {
        &parts[..]
    };
    !rest.iter().any(|p| p.starts_with('.') || *p == "node_modules")
}

/// A backend's revision of one file. Opaque to the engine; compare for equality only.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rev(pub String);

/// Where a backend's change feed was read up to. Opaque to the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor(pub String);

/// A notebook on a backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteNotebook {
    /// Stable id, from the remote `notebook.json` (a ULID).
    pub id: String,
    /// The name the user sees, e.g. the folder name on Drive.
    pub name: String,
    /// Backend-specific handle (e.g. a Drive folder id).
    pub handle: String,
}

/// One file as the backend sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteEntry {
    pub path: String,
    pub rev: Rev,
    /// Hash of the content in the vault's format (`vault::hash_bytes`), when
    /// the backend knows it without downloading; lets the engine skip a get.
    pub hash: Option<String>,
    pub size: u64,
    /// Modification time, ms since the epoch.
    pub modified: i64,
    /// The file was removed (only in change pages).
    pub removed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangePage {
    pub entries: Vec<RemoteEntry>,
    pub cursor: Cursor,
    /// More pages follow; call again with `cursor`.
    pub more: bool,
}

/// The result of a conditional write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Put {
    /// Written; this is the new revision.
    Written(Rev),
    /// Not written: someone else changed the file first. This is their
    /// revision (`None`: it has been removed). The engine fetches it, merges
    /// and tries again.
    Moved(Option<Rev>),
}

/// A remote store for notebooks. Blocking calls, like the rest of the core;
/// the engine calls them from a background thread.
pub trait SyncBackend: Send + Sync {
    /// "gdrive", "icloud", "folder".
    fn kind(&self) -> &'static str;
    /// Find or create the notebook called `name`.
    fn open_notebook(&self, name: &str) -> Result<RemoteNotebook>;
    /// Every file, for joining and periodic full checks.
    fn list(&self, nb: &RemoteNotebook) -> Result<Vec<RemoteEntry>>;
    /// Changes since `cursor`. `None` starts from now: no entries, just a
    /// cursor (take it before a full `list` so nothing slips between them).
    /// Each path appears at most once per page, with its latest state.
    fn changes(&self, nb: &RemoteNotebook, cursor: Option<&Cursor>) -> Result<ChangePage>;
    /// The content and revision of a file; `Error::NotFound` if it doesn't exist.
    fn get(&self, nb: &RemoteNotebook, path: &str) -> Result<(Vec<u8>, Rev)>;
    /// Write `bytes` only if the file is still at `expected` (`None`: only if
    /// it doesn't exist yet).
    fn put(&self, nb: &RemoteNotebook, path: &str, bytes: &[u8], modified: i64, expected: Option<&Rev>) -> Result<Put>;
    /// Remove a file only if it is still at `expected` (into the backend's own
    /// trash where it has one). `Put::Written` carries `expected` back.
    fn remove(&self, nb: &RemoteNotebook, path: &str, expected: &Rev) -> Result<Put>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_syncs() {
        for yes in [
            "note.md",
            "2026/09/2026-09-12-2310-ownership.md",
            "images/photo.png",
            ".noughtpad/chats/01J8.json",
            ".noughtpad/trash/20261103-120000-2026__09__a.md",
            ".noughtpad/user-state/01J8.jsonl",
            ".noughtpad/tombstones/01J8.jsonl",
        ] {
            assert!(is_synced(yes), "{yes} should sync");
        }
        for no in [
            ".noughtpad/index.sqlite",
            ".noughtpad/index.sqlite-wal",
            ".noughtpad/user-state.sqlite",
            ".noughtpad/user-state.sqlite.bak",
            ".noughtpad/sync/state.sqlite",
            ".noughtpad/.gitignore",
            ".noughtpad/artifacts/a.md",
            ".noughtpad/chats",
            ".noughtpad/chats/01J8.json.tmp",
            "2026/09/.noughtpad-tmp-abc",
            ".DS_Store",
            "2026/.DS_Store",
            ".git/config",
            ".obsidian/app.json",
            "node_modules/x.md",
            "../outside.md",
            "a//b.md",
            "",
        ] {
            assert!(!is_synced(no), "{no} should not sync");
        }
    }
}
