use std::collections::HashMap;

use async_stream::stream;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use types::{Message, StreamEvent, Usage};

const DEFAULT_BASE_URL: &str = "https://ai-gateway.vercel.sh/v1";
pub const DEFAULT_MODEL: &str = "anthropic/claude-sonnet-4-6";

#[derive(Serialize)]
struct ChatStreamRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    max_tokens: u32,
    stream: bool,
    stream_options: StreamOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [Value]>,
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Deserialize)]
struct ChatChunk {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
    usage: Option<ApiUsage>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Deserialize)]
struct ToolCallDelta {
    index: u32,
    id: Option<String>,
    function: Option<FunctionDelta>,
}

#[derive(Deserialize)]
struct FunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("Set CODELIGHT_API_KEY or AI_GATEWAY_API_KEY for the default Gateway")]
    MissingApiKey,
    #[error("Set CODELIGHT_MODEL when using a custom CODELIGHT_BASE_URL")]
    MissingModel,
    #[error(
        "CODELIGHT_BASE_URL must be an HTTP(S) API base URL without credentials, a query, or a fragment"
    )]
    InvalidBaseUrl,
    #[error("http transport error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("model endpoint returned {status}: {body}")]
    Status { status: u16, body: String },
}

#[derive(Clone)]
pub struct GatewayClient {
    http: reqwest::Client,
    api_key: Option<String>,
    model: String,
    base_url: String,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    max_tokens: u32,
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ResponseChoice>,
    #[serde(default)]
    usage: ApiUsage,
}

#[derive(Deserialize)]
struct ResponseChoice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

#[derive(Default, Deserialize)]
struct ApiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

impl From<ApiUsage> for Usage {
    fn from(u: ApiUsage) -> Self {
        Usage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
        }
    }
}

impl GatewayClient {
    pub fn from_env() -> Result<Self, GatewayError> {
        Self::from_env_with_model(None)
    }

    pub fn from_env_with_model(model: Option<String>) -> Result<Self, GatewayError> {
        Self::from_settings(
            std::env::var("CODELIGHT_BASE_URL").ok(),
            std::env::var("CODELIGHT_API_KEY").ok(),
            std::env::var("AI_GATEWAY_API_KEY").ok(),
            model.or_else(|| std::env::var("CODELIGHT_MODEL").ok()),
        )
    }

    fn from_settings(
        base_url: Option<String>,
        api_key: Option<String>,
        legacy_key: Option<String>,
        model: Option<String>,
    ) -> Result<Self, GatewayError> {
        let base_url = base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        let parsed = reqwest::Url::parse(&base_url).map_err(|_| GatewayError::InvalidBaseUrl)?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(GatewayError::InvalidBaseUrl);
        }
        let base_url = parsed.as_str().trim_end_matches('/').to_string();
        let is_default = base_url == DEFAULT_BASE_URL;
        let api_key = api_key.filter(|key| !key.trim().is_empty());
        let api_key = if is_default {
            Some(
                api_key
                    .or_else(|| legacy_key.filter(|key| !key.trim().is_empty()))
                    .ok_or(GatewayError::MissingApiKey)?,
            )
        } else {
            api_key
        };
        let model = model.filter(|model| !model.trim().is_empty());
        let model = if is_default {
            model.unwrap_or_else(|| DEFAULT_MODEL.to_string())
        } else {
            model.ok_or(GatewayError::MissingModel)?
        };
        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
            model,
            base_url,
        })
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let request = self
            .http
            .request(method, format!("{}/{path}", self.base_url));
        match &self.api_key {
            Some(key) => request.bearer_auth(key),
            None => request,
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn set_model(&mut self, model: impl Into<String>) {
        self.model = model.into();
    }

    pub async fn list_models(&self) -> Result<Vec<String>, GatewayError> {
        let response = self.request(reqwest::Method::GET, "models").send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(GatewayError::Status {
                status: status.as_u16(),
                body,
            });
        }
        let parsed: ModelsResponse = response.json().await?;
        let mut ids: Vec<String> = parsed.data.into_iter().map(|entry| entry.id).collect();
        ids.sort();
        Ok(ids)
    }

    pub async fn chat(&self, messages: &[Message]) -> Result<(String, Usage), GatewayError> {
        let request = ChatRequest {
            model: self.model.as_str(),
            messages,
            max_tokens: 1024,
        };

        let response = self
            .request(reqwest::Method::POST, "chat/completions")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(GatewayError::Status {
                status: status.as_u16(),
                body,
            });
        }

        let parsed: ChatResponse = response.json().await?;
        let text = parsed
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .unwrap_or_default();

        Ok((text, parsed.usage.into()))
    }

    pub async fn chat_stream(
        &self,
        messages: &[Message],
        tools: &[Value],
    ) -> Result<BoxStream<'static, StreamEvent>, GatewayError> {
        let request = ChatStreamRequest {
            model: self.model.as_str(),
            messages,
            max_tokens: 1024,
            stream: true,
            stream_options: StreamOptions {
                include_usage: true,
            },
            tools: if tools.is_empty() { None } else { Some(tools) },
        };

        let response = self
            .request(reqwest::Method::POST, "chat/completions")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(GatewayError::Status {
                status: status.as_u16(),
                body,
            });
        }

        let mut events = response.bytes_stream().eventsource();

        let stream = stream! {
            let mut tool_ids: HashMap<u32, String> = HashMap::new();

            while let Some(event) = events.next().await {
                let event = match event {
                    Ok(event) => event,
                    Err(err) => {
                        yield StreamEvent::Error(err.to_string());
                        break;
                    }
                };

                if event.data == "[DONE]" {
                    break;
                }

                let chunk: ChatChunk = match serde_json::from_str(&event.data) {
                    Ok(chunk) => chunk,
                    Err(err) => {
                        yield StreamEvent::Error(format!("bad chunk: {err}"));
                        continue;
                    }
                };

                if let Some(usage) = chunk.usage {
                    yield StreamEvent::Done { usage: usage.into() };
                }

                let Some(choice) = chunk.choices.into_iter().next() else {
                    continue;
                };

                if let Some(content) = choice.delta.content
                    && !content.is_empty()
                {
                    yield StreamEvent::Token(content);
                }

                if let Some(tool_calls) = choice.delta.tool_calls {
                    for tc in tool_calls {
                        let ToolCallDelta { index, id, function } = tc;
                        if let Some(function) = function {
                            if let (Some(id), Some(name)) = (id, function.name) {
                                tool_ids.insert(index, id.clone());
                                yield StreamEvent::ToolCallStart { id, name };
                            }
                            if let Some(arguments) = function.arguments
                                && !arguments.is_empty()
                                && let Some(existing_id) = tool_ids.get(&index)
                            {
                                yield StreamEvent::ToolCallArgs {
                                    id: existing_id.clone(),
                                    chunk: arguments,
                                };
                            }
                        }
                    }
                }

                if let Some(reason) = choice.finish_reason
                    && reason == "tool_calls"
                {
                    for (_, id) in tool_ids.drain() {
                        yield StreamEvent::ToolCallEnd { id };
                    }
                }
            }
        };

        Ok(stream.boxed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn default_gateway_preserves_legacy_configuration() {
        let client =
            GatewayClient::from_settings(None, None, Some("legacy-test-key".into()), None).unwrap();
        assert_eq!(client.base_url, DEFAULT_BASE_URL);
        assert_eq!(client.model(), DEFAULT_MODEL);
        assert!(client.api_key.as_deref() == Some("legacy-test-key"));
        assert!(matches!(
            GatewayClient::from_settings(None, None, None, None),
            Err(GatewayError::MissingApiKey)
        ));
    }

    #[test]
    fn custom_endpoint_requires_model_and_never_inherits_gateway_key() {
        let base = Some("http://127.0.0.1:1234/v1/".into());
        assert!(matches!(
            GatewayClient::from_settings(base.clone(), None, Some("legacy-test-key".into()), None),
            Err(GatewayError::MissingModel)
        ));
        let client = GatewayClient::from_settings(
            base,
            None,
            Some("legacy-test-key".into()),
            Some("local-model".into()),
        )
        .unwrap();
        assert!(client.api_key.is_none());
        assert_eq!(client.base_url, "http://127.0.0.1:1234/v1");
        assert_eq!(client.model(), "local-model");
    }

    #[test]
    fn explicit_key_takes_precedence_and_invalid_urls_are_rejected() {
        let client = GatewayClient::from_settings(
            None,
            Some("custom-test-key".into()),
            Some("legacy-test-key".into()),
            None,
        )
        .unwrap();
        assert!(client.api_key.as_deref() == Some("custom-test-key"));
        for base in [
            "",
            "file:///tmp/models",
            "https://user:password@example.com/v1",
            "https://example.com/v1?key=value",
            "https://example.com/v1#fragment",
        ] {
            assert!(matches!(
                GatewayClient::from_settings(Some(base.into()), None, None, Some("model".into())),
                Err(GatewayError::InvalidBaseUrl)
            ));
        }
    }

    async fn mock_server(
        responses: Vec<(&'static str, &'static str)>,
    ) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/custom/v1/", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for (content_type, body) in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 4096];
                loop {
                    let count = socket.read(&mut buffer).await.unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&buffer[..count]);
                    if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if request.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8(request).unwrap());
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
            requests
        });
        (base, task)
    }

    #[tokio::test]
    async fn custom_endpoint_routes_chat_streaming_tools_and_models() {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let (base, server) = mock_server(vec![
                ("application/json", r#"{"data":[{"id":"z-model"},{"id":"a-model"}]}"#),
                ("application/json", r#"{"choices":[{"message":{"content":"ok"}}]}"#),
                ("text/event-stream", "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call-1\",\"function\":{\"name\":\"read_file\",\"arguments\":\"{}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n"),
            ]).await;
            let client = GatewayClient::from_settings(Some(base), Some("custom-test-key".into()), None, Some("a-model".into())).unwrap();
            assert_eq!(client.list_models().await.unwrap(), vec!["a-model", "z-model"]);
            let (reply, _) = client.chat(&[Message::user("hello")]).await.unwrap();
            assert_eq!(reply, "ok");
            let tools = [serde_json::json!({"type":"function","function":{"name":"read_file","parameters":{"type":"object"}}})];
            let events: Vec<_> = client.chat_stream(&[Message::user("read")], &tools).await.unwrap().collect().await;
            assert!(events.iter().any(|event| matches!(event, StreamEvent::Token(text) if text == "hello")));
            assert!(events.iter().any(|event| matches!(event, StreamEvent::ToolCallStart { name, .. } if name == "read_file")));
            assert!(events.iter().any(|event| matches!(event, StreamEvent::ToolCallEnd { id } if id == "call-1")));
            let requests = server.await.unwrap();
            assert!(requests[0].starts_with("GET /custom/v1/models "));
            for request in &requests {
                assert!(request.to_lowercase().contains("authorization: bearer custom-test-key"));
            }
            for request in &requests[1..] {
                assert!(request.starts_with("POST /custom/v1/chat/completions "));
                let body: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
                assert_eq!(body["model"], "a-model");
            }
            let body: Value = serde_json::from_str(requests[2].split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(body["stream"], true);
            assert_eq!(body["tools"], serde_json::json!(tools));
        }).await.unwrap();
    }

    #[tokio::test]
    async fn local_endpoint_sends_no_authorization_without_custom_key() {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let (base, server) = mock_server(vec![("application/json", r#"{"data":[]}"#)]).await;
            let client = GatewayClient::from_settings(
                Some(base),
                None,
                Some("legacy-test-key".into()),
                Some("local".into()),
            )
            .unwrap();
            assert!(client.list_models().await.unwrap().is_empty());
            let requests = server.await.unwrap();
            assert!(!requests[0].to_lowercase().contains("authorization:"));
        })
        .await
        .unwrap();
    }
}
