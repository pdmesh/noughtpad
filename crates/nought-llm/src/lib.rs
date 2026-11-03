//! nought-llm: the language models Nought can think with (M4).
//!
//! Nothing here runs until the user opts in on the Models page, either by
//! downloading a local model ([`catalog`], [`download`], [`local`]) or by adding
//! their own provider and key ([`remote`], [`keys`]). [`prompt`] builds the
//! grounded, cited prompt from the notes the app selected and showed the user.

pub mod catalog;
pub mod download;
pub mod keys;
pub mod local;
pub mod prompt;
pub mod remote;
pub mod settings;

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Unavailable(String),
    #[error("download failed: {0}")]
    Download(String),
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    Io(String),
    #[error("the model failed: {0}")]
    Model(String),
    #[error("{0}")]
    Provider(String),
    #[error("keychain: {0}")]
    Keychain(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// One grounded question.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub system: String,
    pub user: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub secs: f32,
}
