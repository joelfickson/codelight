use std::collections::HashMap;
use std::time::Duration;

use async_stream::stream;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use vc_types::{Message, StreamEvent, Usage};

pub const GATEWAY_URL: &str = "https://ai-gateway.vercel.sh/v1/chat/completions";
pub const MODELS_URL: &str = "https://ai-gateway.vercel.sh/v1/models";
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
    #[error("AI_GATEWAY_API_KEY is not set")]
    MissingApiKey,
    #[error("http transport error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("gateway returned {status}: {body}")]
    Status { status: u16, body: String },
}

#[derive(Clone)]
pub struct GatewayClient {
    http: reqwest::Client,
    api_key: String,
    model: String,
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

#[derive(Deserialize)]
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
        let api_key =
            std::env::var("AI_GATEWAY_API_KEY").map_err(|_| GatewayError::MissingApiKey)?;
        let model = std::env::var("CODELIGHT_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        Ok(Self {
            http: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .read_timeout(Duration::from_secs(60))
                .timeout(Duration::from_secs(120))
                .build()?,
            api_key,
            model,
        })
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn set_model(&mut self, model: impl Into<String>) {
        self.model = model.into();
    }

    pub async fn list_models(&self) -> Result<Vec<String>, GatewayError> {
        let response =
            send_with_retry(self.http.get(MODELS_URL).bearer_auth(&self.api_key)).await?;
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

        let response = send_with_retry(
            self.http
                .post(GATEWAY_URL)
                .bearer_auth(&self.api_key)
                .json(&request),
        )
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

        let response = send_with_retry(
            self.http
                .post(GATEWAY_URL)
                .bearer_auth(&self.api_key)
                .json(&request),
        )
        .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(GatewayError::Status {
                status: status.as_u16(),
                body,
            });
        }

        Ok(decode_stream(response))
    }
}

fn decode_stream(response: reqwest::Response) -> BoxStream<'static, StreamEvent> {
    let mut events = response.bytes_stream().eventsource();

    let stream = stream! {
        let mut tool_ids: HashMap<u32, String> = HashMap::new();
        let mut finished = false;
        let mut usage = Usage::default();

        while let Some(event) = events.next().await {
            let event = match event {
                Ok(event) => event,
                Err(err) => {
                    yield StreamEvent::Error(err.to_string());
                    return;
                }
            };

            if event.data == "[DONE]" {
                break;
            }

            let chunk: ChatChunk = match serde_json::from_str(&event.data) {
                Ok(chunk) => chunk,
                Err(err) => {
                    yield StreamEvent::Error(format!("bad chunk: {err}"));
                    return;
                }
            };

            if let Some(reported) = chunk.usage {
                usage = reported.into();
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

            if let Some(reason) = choice.finish_reason {
                if !matches!(reason.as_str(), "stop" | "tool_calls") {
                    yield StreamEvent::Error(format!("model stopped with finish reason: {reason}"));
                    return;
                }
                finished = true;
                for (_, id) in tool_ids.drain() {
                    yield StreamEvent::ToolCallEnd { id };
                }
            }
        }
        if finished {
            yield StreamEvent::Done { usage };
        } else {
            yield StreamEvent::Error("model stream ended without a finish reason".into());
        }
    };

    stream.boxed()
}

async fn send_with_retry(
    request: reqwest::RequestBuilder,
) -> Result<reqwest::Response, GatewayError> {
    for attempt in 0..3 {
        let response = request
            .try_clone()
            .expect("JSON request is cloneable")
            .send()
            .await;
        let retry = match &response {
            Ok(response) => matches!(
                response.status().as_u16(),
                408 | 429 | 500 | 502 | 503 | 504
            ),
            Err(error) => error.is_connect() || error.is_timeout(),
        };
        if !retry || attempt == 2 {
            return Ok(response?);
        }
        drop(response);
        tokio::time::sleep(Duration::from_millis(250 * (1 << attempt))).await;
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn server(responses: Vec<(u16, String)>) -> (String, tokio::task::JoinHandle<usize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let mut count = 0;
            for (status, body) in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0; 4096];
                let mut received = Vec::new();
                while !received.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                    let count = socket.read(&mut request).await.unwrap();
                    assert!(count > 0);
                    received.extend_from_slice(&request[..count]);
                }
                let response = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
                count += 1;
            }
            count
        });
        (url, task)
    }

    async fn decode(body: &str) -> Vec<StreamEvent> {
        let (url, task) = server(vec![(200, body.into())]).await;
        let response = reqwest::get(url).await.unwrap();
        let events = decode_stream(response).collect().await;
        task.await.unwrap();
        events
    }

    #[tokio::test]
    async fn finish_without_usage_completes_once() {
        let events = decode("data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n").await;
        assert!(matches!(events.first(), Some(StreamEvent::Token(text)) if text == "ok"));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, StreamEvent::Done { .. }))
                .count(),
            1
        );
        assert!(matches!(events.last(), Some(StreamEvent::Done { .. })));
    }

    #[tokio::test]
    async fn truncated_malformed_and_length_limited_streams_fail() {
        for body in [
            "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"},\"finish_reason\":null}]}\n\n",
            "data: invalid\n\ndata: [DONE]\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n",
            "data: [DONE]\n\n",
        ] {
            let events = decode(body).await;
            assert!(matches!(events.last(), Some(StreamEvent::Error(_))));
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, StreamEvent::Done { .. }))
            );
        }
    }

    #[tokio::test]
    async fn retries_transient_responses_but_not_auth_errors() {
        let (url, task) = server(vec![
            (503, String::new()),
            (429, String::new()),
            (200, String::new()),
        ])
        .await;
        let response = send_with_retry(reqwest::Client::new().get(url))
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(task.await.unwrap(), 3);
        let (url, task) = server(vec![(401, String::new())]).await;
        let response = send_with_retry(reqwest::Client::new().get(url))
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
        assert_eq!(task.await.unwrap(), 1);
    }

    #[tokio::test]
    async fn stops_after_three_transient_responses() {
        let (url, task) = server(vec![(503, String::new()); 3]).await;
        let response = send_with_retry(reqwest::Client::new().get(url))
            .await
            .unwrap();
        assert_eq!(response.status(), 503);
        assert_eq!(task.await.unwrap(), 3);
    }
}
