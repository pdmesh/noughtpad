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
    /// The provider or model rejected tool calls; the caller can retry without tools.
    #[error("this model can't use tools: {0}")]
    ToolsUnsupported(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// An earlier exchange in the same chat (text only).
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    pub user: String,
    pub assistant: String,
}

/// One request: earlier turns, then the new user message.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub system: String,
    pub history: Vec<Turn>,
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

/// A read-only tool the model may call.
#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    /// JSON Schema of the arguments.
    pub schema: serde_json::Value,
}

/// The app's side of tool use: runs a tool and returns text for the model.
/// Implementations enforce the chat's scope and report each step to the user.
pub trait NoteTools: Send + Sync {
    fn specs(&self) -> Vec<ToolSpec>;
    fn call(&self, name: &str, args: &serde_json::Value) -> String;
}
