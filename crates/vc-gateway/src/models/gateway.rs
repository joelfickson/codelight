use std::collections::HashMap;

use async_stream::stream;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use vc_types::{Message, StreamEvent, Usage};

pub const GATEWAY_URL: &str = "https://ai-gateway.vercel.sh/v1/chat/completions";
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

pub struct GatewayClient {
    http: reqwest::Client,
    api_key: String,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    max_tokens: u32,
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
        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
        })
    }

    pub async fn chat(&self, messages: &[Message]) -> Result<(String, Usage), GatewayError> {
        let request = ChatRequest {
            model: DEFAULT_MODEL,
            messages,
            max_tokens: 1024,
        };

        let response = self
            .http
            .post(GATEWAY_URL)
            .bearer_auth(&self.api_key)
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
            model: DEFAULT_MODEL,
            messages,
            max_tokens: 1024,
            stream: true,
            stream_options: StreamOptions {
                include_usage: true,
            },
            tools: if tools.is_empty() { None } else { Some(tools) },
        };

        let response = self
            .http
            .post(GATEWAY_URL)
            .bearer_auth(&self.api_key)
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
