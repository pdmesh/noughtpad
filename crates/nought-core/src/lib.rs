//! nought-core: the UI-agnostic engine behind Nought.
//!
//! * [`note`]  — the Markdown note format (frontmatter + body), never rewritten by AI.
//! * [`index`] — the disposable SQLite index (metadata, labels, FTS5).
//! * [`vault`] — file operations on a vault folder, kept in sync with the index.
//! * [`watch`] — filesystem watcher so edits made outside Nought are picked up.

pub mod error;
pub mod index;
pub mod note;
pub mod vault;
pub mod watch;

pub use error::{Error, Result};
pub use index::{DaySummary, LabelCount, ListQuery, MonthLog, NoteSummary, SearchHit};
pub use note::{Note, NoteKind, NoteMeta, Source};
pub use vault::Vault;
