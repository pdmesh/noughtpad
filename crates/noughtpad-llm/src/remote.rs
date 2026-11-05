//! Bring-your-own providers through `genai`: Anthropic, OpenAI, Gemini,
//! Ollama and any OpenAI-compatible endpoint.

use std::sync::atomic::AtomicBool;

use crate::settings::{Remote, RemoteKind};
use crate::{ChatRequest, Error, NoteTools, Result, Usage};

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
        "this build of Noughtpad can't use online providers".into(),
    ))
}

#[cfg(not(feature = "remote"))]
pub async fn chat_with_tools(
    _remote: &Remote,
    _key: Option<String>,
    _req: &ChatRequest,
    _tools: &dyn NoteTools,
    _max_rounds: usize,
    _cancel: &AtomicBool,
    _on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<Usage> {
    Err(Error::Unavailable(
        "this build of Noughtpad can't use online providers".into(),
    ))
}

#[cfg(not(feature = "remote"))]
pub async fn list_models(_kind: RemoteKind, _base_url: Option<String>, _key: Option<String>) -> Result<Vec<String>> {
    Err(Error::Unavailable(
        "this build of Noughtpad can't use online providers".into(),
    ))
}

#[cfg(feature = "remote")]
mod imp {
    use super::*;
    use futures::StreamExt;
    use genai::adapter::AdapterKind;
    use genai::chat::{
        ChatMessage, ChatOptions, ChatRequest as GChatRequest, ChatStreamEvent, ContentPart, Tool, ToolCall,
        ToolResponse,
    };
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

    pub(super) fn client(kind: RemoteKind, base_url: Option<String>, key: Option<String>, model: String) -> Result<Client> {
        if kind.needs_key() && key.as_deref().is_none_or(|k| k.trim().is_empty()) {
            return Err(Error::Provider(format!("{} needs an API key", kind.label())));
        }
        let ak = adapter(kind);
        // Endpoints without a key still need a non-empty auth value for genai.
        let key = key.filter(|k| !k.trim().is_empty()).unwrap_or_else(|| "none".into());
        Client::builder()
            // Without this, genai infers the adapter from the model name
            // string on every call (its hardcoded prefix list doesn't know
            // every model name, e.g. "chat-latest"), and on a miss falls
            // back to Ollama's default endpoint for the *initial* lookup.
            // Our resolver below then only relabels the model's adapter kind
            // back to the right one, leaving that stale localhost:11434
            // endpoint in place for any remote whose kind doesn't set a
            // custom base_url (every hosted provider). Binding the kind here
            // skips the guesswork entirely.
            .with_adapter_kind(ak)
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

    fn endpoint(remote: &Remote) -> Option<String> {
        remote
            .base_url
            .clone()
            .filter(|u| !u.trim().is_empty())
            .or(remote.kind.default_url().map(Into::into))
    }

    fn base_request(req: &ChatRequest) -> GChatRequest {
        let mut chat = GChatRequest::default().with_system(&req.system);
        for t in &req.history {
            chat = chat
                .append_message(ChatMessage::user(&t.user))
                .append_message(ChatMessage::assistant(&t.assistant));
        }
        chat.append_message(ChatMessage::user(&req.user))
    }

    /// What one streamed round produced.
    struct Round {
        calls: Vec<ToolCall>,
        thoughts: Vec<String>,
    }

    async fn stream_round(
        client: &Client,
        model: &str,
        chat: GChatRequest,
        opts: &ChatOptions,
        cancel: &AtomicBool,
        on_text: &mut (dyn FnMut(&str) + Send),
        usage: &mut Usage,
    ) -> Result<Round> {
        let mut res = client
            .exec_chat_stream(model, chat, Some(opts))
            .await
            .map_err(friendly)?;
        let mut round = Round {
            calls: vec![],
            thoughts: vec![],
        };
        while let Some(ev) = res.stream.next().await {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            match ev.map_err(friendly)? {
                ChatStreamEvent::Chunk(c) => on_text(&c.content),
                ChatStreamEvent::End(end) => {
                    if let Some(u) = end.captured_usage {
                        *usage.prompt_tokens.get_or_insert(0) += u.prompt_tokens.unwrap_or(0).max(0) as u32;
                        *usage.completion_tokens.get_or_insert(0) += u.completion_tokens.unwrap_or(0).max(0) as u32;
                    }
                    if let Some(content) = end.captured_content {
                        for part in content.into_parts() {
                            match part {
                                ContentPart::ToolCall(tc) => round.calls.push(tc),
                                ContentPart::ThoughtSignature(t) => round.thoughts.push(t),
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(round)
    }

    pub async fn chat(
        remote: &Remote,
        key: Option<String>,
        req: &ChatRequest,
        cancel: &AtomicBool,
        on_text: &mut (dyn FnMut(&str) + Send),
    ) -> Result<Usage> {
        let client = client(remote.kind, endpoint(remote), key, remote.model.clone())?;
        let opts = ChatOptions::default()
            .with_max_tokens(req.max_tokens)
            .with_temperature(req.temperature as f64)
            .with_capture_usage(true);
        let start = std::time::Instant::now();
        let mut usage = Usage::default();
        stream_round(
            &client,
            &remote.model,
            base_request(req),
            &opts,
            cancel,
            on_text,
            &mut usage,
        )
        .await?;
        usage.secs = start.elapsed().as_secs_f32();
        Ok(usage)
    }

    fn mentions_tools(msg: &str) -> bool {
        let m = msg.to_ascii_lowercase();
        m.contains("tool") || m.contains("function")
    }

    /// Let the model search and read notes through `tools` for up to
    /// `max_rounds` rounds, streaming its text. Fails with
    /// [`Error::ToolsUnsupported`] when the first request is rejected for
    /// using tools, so the caller can fall back to choosing notes itself.
    pub async fn chat_with_tools(
        remote: &Remote,
        key: Option<String>,
        req: &ChatRequest,
        tools: &dyn NoteTools,
        max_rounds: usize,
        cancel: &AtomicBool,
        on_text: &mut (dyn FnMut(&str) + Send),
    ) -> Result<Usage> {
        let client = client(remote.kind, endpoint(remote), key, remote.model.clone())?;
        let gtools: Vec<Tool> = tools
            .specs()
            .into_iter()
            .map(|t| Tool::new(t.name).with_description(t.description).with_schema(t.schema))
            .collect();
        let opts = ChatOptions::default()
            .with_max_tokens(req.max_tokens)
            .with_temperature(req.temperature as f64)
            .with_capture_usage(true)
            .with_capture_tool_calls(true);
        let start = std::time::Instant::now();
        let mut usage = Usage::default();
        let mut chat = base_request(req).with_tools(gtools);
        for round in 0..=max_rounds {
            if round == max_rounds {
                // Out of rounds: ask for the answer with what has been read.
                chat = chat.append_message(ChatMessage::user(
                    "Answer now from the notes you have read. Don't call any more tools.",
                ));
            }
            let r = match stream_round(&client, &remote.model, chat.clone(), &opts, cancel, on_text, &mut usage).await {
                Err(Error::Provider(m)) if round == 0 && mentions_tools(&m) => return Err(Error::ToolsUnsupported(m)),
                other => other?,
            };
            if r.calls.is_empty() || round == max_rounds {
                break;
            }
            let calls = r.calls.clone();
            chat = chat.append_message(ChatMessage::assistant_tool_calls_with_thoughts(r.calls, r.thoughts));
            for call in calls {
                if cancel.load(Ordering::Relaxed) {
                    return Err(Error::Cancelled);
                }
                let out = tools.call(&call.fn_name, &call.fn_arguments);
                chat = chat.append_message(ToolResponse::new(call.call_id, out));
            }
            // Text before a tool call is thinking aloud; separate it from the answer.
            on_text("\n\n");
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
pub use imp::{chat, chat_with_tools, list_models};

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
            history: vec![],
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

    /// Serves a tool call on the first request and an answer on the next.
    fn fake_tool_server() -> (String, std::sync::mpsc::Receiver<String>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for stream in l.incoming() {
                let mut s = stream.unwrap();
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut len = 0usize;
                loop {
                    let mut line = String::new();
                    r.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; len];
                r.read_exact(&mut body).unwrap();
                let body = String::from_utf8_lossy(&body).to_string();
                let first = !body.contains("\"tool\"");
                tx.send(body).unwrap();
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n"
                );
                let chunk = |delta: &str, finish: &str| {
                    format!("data: {{\"id\":\"x\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"m\",\"choices\":[{{\"index\":0,\"delta\":{delta},\"finish_reason\":{finish}}}]}}\n\n")
                };
                if first {
                    let call = r#"{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"search_notes","arguments":"{\"query\":\"ownership\"}"}}]}"#;
                    let _ = write!(s, "{}{}", chunk(call, "null"), chunk("{}", "\"tool_calls\""));
                } else {
                    let _ = write!(
                        s,
                        "{}{}",
                        chunk(r#"{"content":"Ownership grew on you [1]."}"#, "null"),
                        chunk("{}", "\"stop\"")
                    );
                }
                let _ = write!(s, "data: [DONE]\n\n");
            }
        });
        (format!("http://{addr}/v1"), rx)
    }

    struct Host(std::sync::Mutex<Vec<String>>);
    impl NoteTools for Host {
        fn specs(&self) -> Vec<crate::ToolSpec> {
            vec![crate::ToolSpec {
                name: "search_notes",
                description: "Search notes",
                schema: serde_json::json!({"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}),
            }]
        }
        fn call(&self, name: &str, args: &serde_json::Value) -> String {
            self.0
                .lock()
                .unwrap()
                .push(format!("{name}:{}", args["query"].as_str().unwrap_or("")));
            "[1] 2026-03-03 · your own note\nOwnership may matter more than I expected.".into()
        }
    }

    #[tokio::test]
    async fn uses_tools_then_answers() {
        let (url, rx) = fake_tool_server();
        let remote = Remote {
            id: "r".into(),
            kind: RemoteKind::OpenAiCompatible,
            label: "x".into(),
            base_url: Some(url),
            model: "m".into(),
        };
        let req = ChatRequest {
            system: "sys".into(),
            history: vec![crate::Turn {
                user: "earlier q".into(),
                assistant: "earlier a".into(),
            }],
            user: "how did ownership change?".into(),
            max_tokens: 50,
            temperature: 0.2,
        };
        let host = Host(Default::default());
        let mut out = String::new();
        chat_with_tools(&remote, None, &req, &host, 4, &AtomicBool::new(false), &mut |t| {
            out.push_str(t)
        })
        .await
        .unwrap();
        assert_eq!(*host.0.lock().unwrap(), vec!["search_notes:ownership"]);
        assert!(out.trim().ends_with("Ownership grew on you [1]."), "{out:?}");
        let first = rx.recv().unwrap();
        assert!(first.contains("search_notes") && first.contains("earlier a"), "{first}");
        let second = rx.recv().unwrap();
        assert!(
            second.contains("Ownership may matter more"),
            "tool result sent back: {second}"
        );
    }

    #[tokio::test]
    async fn hosted_providers_need_a_key() {
        let err = list_models(RemoteKind::Anthropic, None, None).await.unwrap_err();
        assert!(err.to_string().contains("needs an API key"));
    }

    /// A hosted provider (no `base_url` override) must stay on its own
    /// endpoint even for a model name genai's own adapter-from-name guesser
    /// doesn't recognise (anything not starting with "gpt", "o1", "chatgpt"…
    /// falls back to Ollama's localhost:11434 there). Regression test for a
    /// bug where the chat call ended up hitting
    /// `http://localhost:11434/chat/completions` for an OpenAI remote whose
    /// model was named e.g. "chat-latest".
    #[test]
    fn hosted_remote_keeps_its_adapter_for_an_unrecognised_model_name() {
        use genai::adapter::AdapterKind;

        let c = imp::client(RemoteKind::OpenAi, None, Some("sk-test".into()), "chat-latest".into()).unwrap();
        assert_eq!(c.adapter_kind(), Some(AdapterKind::OpenAI));
    }
}
