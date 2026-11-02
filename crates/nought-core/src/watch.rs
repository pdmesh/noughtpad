//! Filesystem watcher: keeps the index in step with edits made outside Nought
//! (another editor, git, a sync client).

use std::sync::Arc;
use std::time::Duration;

use notify_debouncer_full::notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};

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
