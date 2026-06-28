use serde::{Deserialize, Serialize};
use vc_types::{Message, Usage};

const GATEWAY_URL: &str = "https://ai-gateway.vercel.sh/v1/chat/completions";
pub const DEFAULT_MODEL: &str = "anthropic/claude-sonnet-4-6";

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
}
