//! Bring-your-own providers through `genai`: Anthropic, OpenAI, Gemini,
//! Ollama and any OpenAI-compatible endpoint.

use std::sync::atomic::AtomicBool;

use crate::settings::{Remote, RemoteKind};
use crate::{ChatRequest, Error, Result, Usage};

/// Whether this build can talk to remote providers.
pub const AVAILABLE: bool = cfg!(feature = "remote");

#[cfg(not(feature = "remote"))]
pub async fn chat(
    _remote: &Remote,
    _key: Option<String>,
    _req: &ChatRequest,
    _cancel: &AtomicBool,
    _on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<Usage> {
    Err(Error::Unavailable(
        "this build of Nought can't use online providers".into(),
    ))
}

#[cfg(not(feature = "remote"))]
pub async fn list_models(_kind: RemoteKind, _base_url: Option<String>, _key: Option<String>) -> Result<Vec<String>> {
    Err(Error::Unavailable(
        "this build of Nought can't use online providers".into(),
    ))
}

#[cfg(feature = "remote")]
mod imp {
    use super::*;
    use futures::StreamExt;
    use genai::adapter::AdapterKind;
    use genai::chat::{ChatMessage, ChatOptions, ChatRequest as GChatRequest, ChatStreamEvent};
    use genai::resolver::{AuthData, Endpoint};
    use genai::{Client, ModelIden, ServiceTarget};
    use std::sync::atomic::Ordering;

    fn adapter(kind: RemoteKind) -> AdapterKind {
        match kind {
            RemoteKind::Anthropic => AdapterKind::Anthropic,
            RemoteKind::OpenAi | RemoteKind::OpenAiCompatible => AdapterKind::OpenAI,
            RemoteKind::Gemini => AdapterKind::Gemini,
            RemoteKind::Ollama => AdapterKind::Ollama,
        }
    }

    /// genai joins paths onto the base URL, so it must end with a slash.
    fn base(url: &str) -> String {
        let u = url.trim();
        if u.ends_with('/') {
            u.to_string()
        } else {
            format!("{u}/")
        }
    }

    fn client(kind: RemoteKind, base_url: Option<String>, key: Option<String>, model: String) -> Result<Client> {
        if kind.needs_key() && key.as_deref().is_none_or(|k| k.trim().is_empty()) {
            return Err(Error::Provider(format!("{} needs an API key", kind.label())));
        }
        let ak = adapter(kind);
        // Endpoints without a key still need a non-empty auth value for genai.
        let key = key.filter(|k| !k.trim().is_empty()).unwrap_or_else(|| "none".into());
        Client::builder()
            .with_service_target_resolver_fn(
                move |mut t: ServiceTarget| -> std::result::Result<ServiceTarget, genai::resolver::Error> {
                    if let Some(u) = &base_url {
                        t.endpoint = Endpoint::from_owned(base(u));
                    }
                    t.auth = AuthData::from_single(key.clone());
                    t.model = ModelIden::new(ak, model.clone());
                    Ok(t)
                },
            )
            .build()
            .map_err(friendly)
    }

    fn friendly(e: genai::Error) -> Error {
        let s = e.to_string();
        let lower = s.to_ascii_lowercase();
        if lower.contains("401")
            || lower.contains("unauthorized")
            || lower.contains("invalid x-api-key")
            || lower.contains("api key")
        {
            Error::Provider("the provider rejected the API key".into())
        } else if lower.contains("connect") || lower.contains("dns") || lower.contains("timed out") {
            Error::Provider(format!("couldn't reach the provider ({s})"))
        } else {
            Error::Provider(s)
        }
    }

    pub async fn chat(
        remote: &Remote,
        key: Option<String>,
        req: &ChatRequest,
        cancel: &AtomicBool,
        on_text: &mut (dyn FnMut(&str) + Send),
    ) -> Result<Usage> {
        let url = remote
            .base_url
            .clone()
            .filter(|u| !u.trim().is_empty())
            .or(remote.kind.default_url().map(Into::into));
        let client = client(remote.kind, url, key, remote.model.clone())?;
        let chat = GChatRequest::default()
            .with_system(&req.system)
            .append_message(ChatMessage::user(&req.user));
        let opts = ChatOptions::default()
            .with_max_tokens(req.max_tokens)
            .with_temperature(req.temperature as f64)
            .with_capture_usage(true);
        let start = std::time::Instant::now();
        let mut res = client
            .exec_chat_stream(remote.model.as_str(), chat, Some(&opts))
            .await
            .map_err(friendly)?;
        let mut usage = Usage::default();
        while let Some(ev) = res.stream.next().await {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            match ev.map_err(friendly)? {
                ChatStreamEvent::Chunk(c) => on_text(&c.content),
                ChatStreamEvent::End(end) => {
                    if let Some(u) = end.captured_usage {
                        usage.prompt_tokens = u.prompt_tokens.map(|n| n.max(0) as u32);
                        usage.completion_tokens = u.completion_tokens.map(|n| n.max(0) as u32);
                    }
                }
                _ => {}
            }
        }
        usage.secs = start.elapsed().as_secs_f32();
        Ok(usage)
    }

    pub async fn list_models(kind: RemoteKind, base_url: Option<String>, key: Option<String>) -> Result<Vec<String>> {
        let url = base_url
            .filter(|u| !u.trim().is_empty())
            .or(kind.default_url().map(Into::into));
        if kind.needs_key() && key.as_deref().is_none_or(|k| k.trim().is_empty()) {
            return Err(Error::Provider(format!("{} needs an API key", kind.label())));
        }
        let auth = AuthData::from_single(key.filter(|k| !k.trim().is_empty()).unwrap_or_else(|| "none".into()));
        let cfg: genai::resolver::ProviderConfig = match url {
            Some(u) => (Endpoint::from_owned(base(&u)), auth).into(),
            None => auth.into(),
        };
        let mut names = Client::new()
            .map_err(friendly)?
            .all_model_names(adapter(kind), cfg)
            .await
            .map_err(friendly)?;
        names.sort();
        names.dedup();
        Ok(names)
    }
}

#[cfg(feature = "remote")]
pub use imp::{chat, list_models};

#[cfg(all(test, feature = "remote"))]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    /// A fake OpenAI-compatible server: lists two models and streams "Hi [1]".
    /// Returns the base URL and a receiver of request bodies.
    fn fake_openai() -> (String, std::sync::mpsc::Receiver<(String, String)>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for stream in l.incoming() {
                let mut s = stream.unwrap();
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut first = String::new();
                r.read_line(&mut first).unwrap();
                let (mut len, mut auth) = (0usize, String::new());
                loop {
                    let mut line = String::new();
                    r.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    let lower = line.to_ascii_lowercase();
                    if let Some(v) = lower.strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                    if lower.starts_with("authorization:") {
                        auth = line.trim().to_string();
                    }
                }
                let mut body = vec![0; len];
                r.read_exact(&mut body).unwrap();
                tx.send((first.clone(), format!("{auth}\n{}", String::from_utf8_lossy(&body))))
                    .unwrap();
                if first.contains("/models") {
                    let b = r#"{"object":"list","data":[{"id":"b-model","object":"model"},{"id":"a-model","object":"model"}]}"#;
                    let _ = write!(
                        s,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{b}",
                        b.len()
                    );
                } else {
                    let _ = write!(
                        s,
                        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n"
                    );
                    for piece in ["Hi", " [1]"] {
                        let _ = write!(
                            s,
                            "data: {{\"id\":\"x\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"a-model\",\"choices\":[{{\"index\":0,\"delta\":{{\"content\":\"{piece}\"}},\"finish_reason\":null}}]}}\n\n"
                        );
                    }
                    let _ = write!(
                        s,
                        "data: {{\"id\":\"x\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"a-model\",\"choices\":[{{\"index\":0,\"delta\":{{}},\"finish_reason\":\"stop\"}}],\"usage\":{{\"prompt_tokens\":12,\"completion_tokens\":2,\"total_tokens\":14}}}}\n\ndata: [DONE]\n\n"
                    );
                }
            }
        });
        (format!("http://{addr}/v1"), rx)
    }

    #[tokio::test]
    async fn streams_from_an_openai_compatible_endpoint() {
        let (url, rx) = fake_openai();
        let models = list_models(RemoteKind::OpenAiCompatible, Some(url.clone()), Some("sk-test".into()))
            .await
            .unwrap();
        assert_eq!(models, vec!["a-model", "b-model"]);
        let (line, _) = rx.recv().unwrap();
        assert!(line.starts_with("GET /v1/models"), "{line}");

        let remote = Remote {
            id: "r".into(),
            kind: RemoteKind::OpenAiCompatible,
            label: "LM Studio".into(),
            base_url: Some(url),
            model: "a-model".into(),
        };
        let req = ChatRequest {
            system: "sys".into(),
            user: "question".into(),
            max_tokens: 50,
            temperature: 0.2,
        };
        let mut out = String::new();
        let usage = chat(
            &remote,
            Some("sk-test".into()),
            &req,
            &AtomicBool::new(false),
            &mut |t| out.push_str(t),
        )
        .await
        .unwrap();
        assert_eq!(out, "Hi [1]");
        assert_eq!(usage.completion_tokens, Some(2));
        let (line, body) = rx.recv().unwrap();
        assert!(line.starts_with("POST /v1/chat/completions"), "{line}");
        assert!(
            body.starts_with("authorization: Bearer sk-test") || body.starts_with("Authorization: Bearer sk-test"),
            "{body}"
        );
        assert!(body.contains("\"a-model\"") && body.contains("question") && body.contains("sys"));
    }

    #[tokio::test]
    async fn hosted_providers_need_a_key() {
        let err = list_models(RemoteKind::Anthropic, None, None).await.unwrap_err();
        assert!(err.to_string().contains("needs an API key"));
    }
}
