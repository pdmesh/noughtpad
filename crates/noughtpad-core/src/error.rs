use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("invalid frontmatter: {0}")]
    Frontmatter(String),
    #[error("note not found: {0}")]
    NotFound(String),
    #[error("path is outside the vault: {0}")]
    OutsideVault(PathBuf),
    #[error("note changed on disk since it was opened: {0}")]
    Conflict(String),
    #[error("embedding error: {0}")]
    Embedding(String),
    #[error("sync error: {0}")]
    Sync(String),
    #[error("watcher error: {0}")]
    Watch(#[from] notify_debouncer_full::notify::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) trait IoContext<T> {
    fn at(self, path: impl Into<PathBuf>) -> Result<T>;
}

impl<T> IoContext<T> for std::io::Result<T> {
    fn at(self, path: impl Into<PathBuf>) -> Result<T> {
        self.map_err(|source| Error::Io {
            path: path.into(),
            source,
        })
    }
}
