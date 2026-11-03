//! What the user has opted into. Stored in the app's config file; API keys are
//! never stored here (see [`crate::keys`]).

use serde::{Deserialize, Serialize};

/// A bring-your-own service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteKind {
    Anthropic,
    OpenAi,
    Gemini,
    /// Ollama on this or another machine (no key).
    Ollama,
    /// Any OpenAI-compatible endpoint: LM Studio, llama.cpp server, vLLM, OpenRouter…
    OpenAiCompatible,
}

impl RemoteKind {
    pub fn label(self) -> &'static str {
        match self {
            RemoteKind::Anthropic => "Anthropic",
            RemoteKind::OpenAi => "OpenAI",
            RemoteKind::Gemini => "Google Gemini",
            RemoteKind::Ollama => "Ollama",
            RemoteKind::OpenAiCompatible => "OpenAI-compatible",
        }
    }

    pub fn needs_key(self) -> bool {
        matches!(self, RemoteKind::Anthropic | RemoteKind::OpenAi | RemoteKind::Gemini)
    }

    pub fn needs_url(self) -> bool {
        matches!(self, RemoteKind::OpenAiCompatible)
    }

    pub fn default_url(self) -> Option<&'static str> {
        match self {
            RemoteKind::Ollama => Some("http://localhost:11434"),
            RemoteKind::OpenAiCompatible => Some("http://localhost:1234/v1"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Remote {
    pub id: String,
    pub kind: RemoteKind,
    pub label: String,
    #[serde(default)]
    pub base_url: Option<String>,
    pub model: String,
}

impl Remote {
    pub fn endpoint(&self) -> Option<&str> {
        self.base_url
            .as_deref()
            .filter(|u| !u.trim().is_empty())
            .or(self.kind.default_url())
    }

    /// Where notes go when this provider answers, for the privacy prompt.
    /// Two providers with the same destination share one consent.
    pub fn destination(&self) -> String {
        match self.endpoint() {
            Some(url) => format!("{}:{}", kind_key(self.kind), url.trim_end_matches('/')),
            None => kind_key(self.kind).to_string(),
        }
    }
}

fn kind_key(k: RemoteKind) -> &'static str {
    match k {
        RemoteKind::Anthropic => "anthropic",
        RemoteKind::OpenAi => "openai",
        RemoteKind::Gemini => "gemini",
        RemoteKind::Ollama => "ollama",
        RemoteKind::OpenAiCompatible => "openai-compatible",
    }
}

/// The model that answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Active {
    Local { model: String },
    Remote { id: String },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LlmSettings {
    /// Nothing runs until a model is chosen; turning off clears this.
    pub active: Option<Active>,
    pub remotes: Vec<Remote>,
    /// Destinations the user agreed to send notes to (see [`Remote::destination`]).
    pub consented: Vec<String>,
    /// The destination of the last answer sent off this machine, to notice a change.
    pub last_destination: Option<String>,
}

impl LlmSettings {
    pub fn remote(&self, id: &str) -> Option<&Remote> {
        self.remotes.iter().find(|r| r.id == id)
    }

    pub fn active_remote(&self) -> Option<&Remote> {
        match &self.active {
            Some(Active::Remote { id }) => self.remote(id),
            _ => None,
        }
    }

    /// Whether sending to `remote` must first show the user which notes go where:
    /// the first time, and whenever the destination differs from the last one used.
    pub fn needs_consent(&self, remote: &Remote) -> bool {
        let dest = remote.destination();
        !self.consented.contains(&dest) || self.last_destination.as_ref().is_some_and(|d| *d != dest)
    }

    pub fn record_consent(&mut self, remote: &Remote) {
        let dest = remote.destination();
        if !self.consented.contains(&dest) {
            self.consented.push(dest.clone());
        }
        self.last_destination = Some(dest);
    }

    pub fn remove_remote(&mut self, id: &str) {
        self.remotes.retain(|r| r.id != id);
        if matches!(&self.active, Some(Active::Remote { id: a }) if a == id) {
            self.active = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(id: &str, kind: RemoteKind, url: Option<&str>) -> Remote {
        Remote {
            id: id.into(),
            kind,
            label: id.into(),
            base_url: url.map(Into::into),
            model: "m".into(),
        }
    }

    #[test]
    fn consent_first_time_and_on_change() {
        let mut s = LlmSettings::default();
        let a = remote("a", RemoteKind::Anthropic, None);
        let o = remote("o", RemoteKind::Ollama, None);
        assert!(s.needs_consent(&a));
        s.record_consent(&a);
        assert!(!s.needs_consent(&a));
        // Switching provider asks again…
        assert!(s.needs_consent(&o));
        s.record_consent(&o);
        // …and so does switching back.
        assert!(s.needs_consent(&a));
        s.record_consent(&a);
        assert!(!s.needs_consent(&a));
    }

    #[test]
    fn destination_uses_endpoint() {
        assert_eq!(
            remote("x", RemoteKind::Ollama, None).destination(),
            "ollama:http://localhost:11434"
        );
        assert_eq!(
            remote("x", RemoteKind::OpenAiCompatible, Some("https://openrouter.ai/api/v1/")).destination(),
            "openai-compatible:https://openrouter.ai/api/v1"
        );
        assert_eq!(remote("x", RemoteKind::OpenAi, None).destination(), "openai");
    }

    #[test]
    fn removing_the_active_remote_turns_off() {
        let mut s = LlmSettings {
            remotes: vec![remote("a", RemoteKind::Gemini, None)],
            ..Default::default()
        };
        s.active = Some(Active::Remote { id: "a".into() });
        s.remove_remote("a");
        assert_eq!(s.active, None);
    }

    #[test]
    fn settings_roundtrip_with_missing_fields() {
        let s: LlmSettings = serde_json::from_str(r#"{"active":{"kind":"local","model":"x"}}"#).unwrap();
        assert_eq!(s.active, Some(Active::Local { model: "x".into() }));
        assert!(s.remotes.is_empty());
    }
}
