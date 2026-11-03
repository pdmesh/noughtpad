//! noughtpad-core: the UI-agnostic engine behind Noughtpad.
//!
//! * [`note`]  — the Markdown note format (frontmatter + body), never rewritten by AI.
//! * [`index`] — the disposable SQLite index (metadata, labels, FTS5).
//! * [`vault`] — file operations on a vault folder, kept in sync with the index.
//! * [`semantic`] — local embeddings, related notes and hybrid search (M2).
//! * [`userstate`] — the user's judgements about derived data (not disposable).
//! * [`watch`] — filesystem watcher so edits made outside Noughtpad are picked up.

pub mod error;
pub mod index;
pub mod note;
pub mod semantic;
pub mod userstate;
pub mod vault;
pub mod watch;

pub use error::{Error, Result};
pub use index::{DaySummary, LabelCount, ListQuery, MonthLog, NoteSummary, SearchHit};
pub use note::{Note, NoteKind, NoteMeta, Source};
pub use vault::Vault;
