//! Filesystem watcher: keeps the index in step with edits made outside Noughtpad
//! (another editor, git, a sync client).

use std::sync::Arc;
use std::time::Duration;

use notify_debouncer_full::notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};

use crate::sync::Journal;
use crate::vault::{Change, Vault};
use crate::Result;

pub struct VaultWatcher {
    _debouncer: Debouncer<notify_debouncer_full::notify::RecommendedWatcher, RecommendedCache>,
}

/// Watch the vault. `on_change` receives the effective changes of each
/// debounced batch (own writes are filtered out because their hash is already
/// indexed).
pub fn watch<F>(vault: Arc<Vault>, on_change: F) -> Result<VaultWatcher>
where
    F: Fn(Vec<Change>) + Send + 'static,
{
    watch_journaled(vault, None, on_change)
}

/// [`watch`], also recording every changed path that syncs in `journal`
/// (notes, other files, chats, user-state), whoever wrote it.
pub fn watch_journaled<F>(vault: Arc<Vault>, journal: Option<Arc<Journal>>, on_change: F) -> Result<VaultWatcher>
where
    F: Fn(Vec<Change>) + Send + 'static,
{
    let root = vault.root().to_path_buf();
    let v = vault.clone();
    let mut debouncer = new_debouncer(Duration::from_millis(400), None, move |res: DebounceEventResult| {
        let events = match res {
            Ok(events) => events,
            Err(errors) => {
                for e in errors {
                    tracing::warn!("watch error: {e}");
                }
                return;
            }
        };
        let mut paths: Vec<_> = events
            .iter()
            .filter(|e| !matches!(e.kind, EventKind::Access(_)))
            .flat_map(|e| e.paths.iter().cloned())
            .collect();
        paths.sort();
        paths.dedup();
        if let Some(j) = &journal {
            // A new or moved-in folder may arrive as one event for the folder,
            // so record the files inside it. A path that's gone may have been a
            // folder; the engine treats it as a prefix.
            let mut rels = Vec::new();
            for p in &paths {
                if p.is_dir() {
                    for e in walkdir::WalkDir::new(p).into_iter().filter_map(|e| e.ok()) {
                        if e.file_type().is_file() {
                            rels.extend(v.relative(e.path()));
                        }
                    }
                } else {
                    rels.extend(v.relative(p));
                }
            }
            if let Err(e) = j.record(rels.iter().map(String::as_str)) {
                tracing::warn!("could not record changes for sync: {e}");
            }
        }
        let changes: Vec<Change> = paths
            .iter()
            .filter_map(|p| match v.sync_path(p) {
                Ok(Change::Unchanged) => None,
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::warn!("failed to sync {}: {e}", p.display());
                    None
                }
            })
            .collect();
        if !changes.is_empty() {
            on_change(changes);
        }
    })?;
    debouncer.watch(&root, RecursiveMode::Recursive)?;
    Ok(VaultWatcher { _debouncer: debouncer })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn external_edits_reach_the_index() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Vault::open(dir.path()).unwrap());
        let (tx, rx) = mpsc::channel();
        let _w = watch(vault.clone(), move |c| {
            let _ = tx.send(c);
        })
        .unwrap();
        std::fs::write(vault.root().join("from-another-app.md"), "Written by another editor").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut seen = false;
        while std::time::Instant::now() < deadline && !seen {
            if let Ok(changes) = rx.recv_timeout(Duration::from_millis(500)) {
                seen = changes
                    .iter()
                    .any(|c| matches!(c, Change::Upserted { path, .. } if path == "from-another-app.md"));
            }
        }
        assert!(seen, "watcher did not report the external file");
        assert_eq!(vault.search("another editor", 5).unwrap().len(), 1);
    }

    #[test]
    fn changes_that_sync_reach_the_journal() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Vault::open(dir.path()).unwrap());
        let journal = Arc::new(Journal::open_in_memory().unwrap());
        let _w = watch_journaled(vault.clone(), Some(journal.clone()), |_| {}).unwrap();
        let root = vault.root().to_path_buf();
        std::fs::create_dir_all(root.join("img")).unwrap();
        std::fs::write(root.join("img/photo.png"), b"png").unwrap();
        std::fs::write(root.join("note.md"), "hello").unwrap();
        std::fs::write(root.join(".noughtpad/index-scratch.tmp"), "x").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut paths = Vec::new();
        while std::time::Instant::now() < deadline {
            paths = journal
                .pending()
                .unwrap()
                .into_iter()
                .map(|p| p.path)
                .collect::<Vec<_>>();
            if paths.len() >= 2 {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        paths.sort();
        assert_eq!(paths, vec!["img/photo.png", "note.md"]);
    }
}
