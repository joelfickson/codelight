use crate::chatgpt_auth::ChatGptAuth;
use anyhow::{Context, Result, ensure};
use async_stream::stream;
use eventsource_stream::Eventsource;
use futures::{StreamExt, stream::BoxStream};
use serde_json::{Value, json};
use std::time::Duration;
use types::{Message, Role, StreamEvent, Usage};

#[derive(Clone)]
pub(crate) struct ChatGptClient {
    auth: ChatGptAuth,
    account: String,
    http: reqwest::Client,
}

impl ChatGptClient {
    pub async fn new() -> Result<Self> {
        let auth = ChatGptAuth::new()?;
        let account = auth.active_id().await?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self {
            auth,
            account,
            http,
        })
    }

    pub async fn list_models(&self) -> Result<Vec<String>> {
        let token = self.auth.access_token(&self.account).await?;
        let response = self
            .http
            .get("https://api.openai.com/v1/models")
            .bearer_auth(token)
            .send()
            .await?;
        ensure!(
            response.status().is_success(),
            "ChatGPT model discovery failed (HTTP {})",
            response.status().as_u16()
        );
        model_ids(&response.json::<Value>().await?)
    }

    pub async fn chat_stream(
        &self,
        model: &str,
        messages: &[Message],
        tools: &[Value],
    ) -> Result<BoxStream<'static, StreamEvent>> {
        let body = request_body(model, messages, tools)?;
        let token = self.auth.access_token(&self.account).await?;
        let response = self
            .http
            .post("https://api.openai.com/v1/responses")
            .bearer_auth(token)
            .json(&body)
            .send()
            .await?;
        ensure!(
            response.status().is_success(),
            "ChatGPT inference failed (HTTP {}). Check your plan limits or run codelight login to reconnect.",
            response.status().as_u16()
        );
        let events = response
            .bytes_stream()
            .eventsource()
            .map(|event| {
                event
                    .map(|event| event.data)
                    .map_err(|_| anyhow::anyhow!("ChatGPT stream transport failed"))
            })
            .boxed();
        Ok(decode_events(events))
    }
}

fn model_ids(value: &Value) -> Result<Vec<String>> {
    let models = value["models"]
        .as_array()
        .context("Invalid ChatGPT model catalog")?;
    Ok(models
        .iter()
        .filter(|model| model["visibility"] == "list")
        .filter_map(|model| {
            model["slug"]
                .as_str()
                .filter(|id| !id.is_empty())
                .map(str::to_string)
        })
        .collect())
}

fn request_body(model: &str, messages: &[Message], tools: &[Value]) -> Result<Value> {
    let mut input = Vec::new();
    for message in messages {
        if message.role == Role::Assistant && !message.response_items.is_empty() {
            input.extend(message.response_items.clone());
            continue;
        }
        match message.role {
            Role::Tool => input.push(json!({"type":"function_call_output", "call_id":message.tool_call_id.as_ref().context("Tool result has no call ID")?, "output":message.content})),
            Role::Assistant => {
                if !message.content.is_empty() {
                    input.push(json!({"role":"assistant", "content":message.content}));
                }
                for call in &message.tool_calls {
                    input.push(json!({"type":"function_call", "call_id":call.id,"name":call.name,"arguments":call.arguments}));
                }
            }
            Role::System => input.push(json!({"role":"developer", "content":message.content})),
            Role::User => input.push(json!({"role":"user", "content":message.content})),
        }
    }
    let tools = tools
        .iter()
        .map(|tool| {
            let function = tool["function"]
                .as_object()
                .context("Invalid function tool definition")?;
            let mut result = function.clone();
            result.insert("type".into(), json!("function"));
            result.insert("strict".into(), json!(false));
            Ok(Value::Object(result))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(
        json!({"model":model, "input":input, "tools":tools, "store":false, "stream":true, "include":["reasoning.encrypted_content"]}),
    )
}

fn completed_events(response: &Value) -> Result<Vec<StreamEvent>> {
    ensure!(
        response["status"] == "completed",
        "ChatGPT response did not complete"
    );
    let output = response["output"]
        .as_array()
        .context("Completed ChatGPT response has no output")?;
    let mut events = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for item in output {
        if item["type"] == "function_call" {
            let id = item["call_id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .context("Function call has no ID")?
                .to_string();
            ensure!(ids.insert(id.clone()), "Duplicate function call ID");
            let name = item["name"]
                .as_str()
                .filter(|name| !name.is_empty())
                .context("Function call has no name")?
                .to_string();
            let chunk = item["arguments"]
                .as_str()
                .context("Function call has no arguments")?
                .to_string();
            serde_json::from_str::<Value>(&chunk)
                .context("Function call arguments are not valid JSON")?;
            events.push(StreamEvent::ToolCallStart {
                id: id.clone(),
                name,
            });
            events.push(StreamEvent::ToolCallArgs {
                id: id.clone(),
                chunk,
            });
            events.push(StreamEvent::ToolCallEnd { id });
        }
    }
    events.push(StreamEvent::ResponseItems(output.clone()));
    let usage = response
        .get("usage")
        .filter(|value| !value.is_null())
        .cloned()
        .unwrap_or_else(|| json!({"input_tokens":0,"output_tokens":0}));
    events.push(StreamEvent::Done {
        usage: serde_json::from_value::<Usage>(usage).context("Invalid ChatGPT usage")?,
    });
    Ok(events)
}

fn decode_events(mut input: BoxStream<'static, Result<String>>) -> BoxStream<'static, StreamEvent> {
    Box::pin(stream! {
        while let Some(data) = input.next().await {
            let event = match data.and_then(|data| serde_json::from_str::<Value>(&data).map_err(|_| anyhow::anyhow!("Invalid ChatGPT stream event"))) {
                Ok(event) => event,
                Err(error) => { yield StreamEvent::Error(error.to_string()); return; }
            };
            match event["type"].as_str().unwrap_or_default() {
                "response.output_text.delta" | "response.refusal.delta" => {
                    if let Some(text) = event["delta"].as_str() { yield StreamEvent::Token(text.to_string()); }
                }
                "response.completed" => {
                    match completed_events(&event["response"]) {
                        Ok(events) => for event in events { yield event; },
                        Err(error) => yield StreamEvent::Error(error.to_string()),
                    }
                    return;
                }
                "response.failed" | "response.incomplete" | "error" => {
                    yield StreamEvent::Error("ChatGPT response failed or was incomplete. Check your plan limits and retry; no pending tools were executed.".into());
                    return;
                }
                _ => {}
            }
        }
        yield StreamEvent::Error("ChatGPT stream ended without response.completed".into());
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completed() -> Value {
        json!({"type":"response.completed", "response":{"status":"completed", "output":[
            {"type":"reasoning","id":"reasoning-test","summary":[],"encrypted_content":"opaque-test"},
            {"type":"function_call","id":"function-test","call_id":"call-test","name":"read_file","arguments":"{\"path\":\"README.md\"}"}
        ],"usage":{"input_tokens":10,"output_tokens":20}}})
    }

    async fn decode(values: Vec<Value>) -> Vec<StreamEvent> {
        decode_events(
            futures::stream::iter(values.into_iter().map(|value| Ok(value.to_string()))).boxed(),
        )
        .collect()
        .await
    }

    #[test]
    fn model_catalog_preserves_account_order_and_visibility() {
        assert_eq!(model_ids(&json!({"models":[{"slug":"second","visibility":"list"},{"slug":"hidden","visibility":"hide"},{"slug":"first","visibility":"list"}]})).unwrap(), vec!["second", "first"]);
        assert!(model_ids(&json!({"data":[]})).is_err());
    }

    #[tokio::test]
    async fn completed_tool_call_retains_reasoning_across_serialization() {
        let events = decode(vec![completed()]).await;
        assert!(
            matches!(events.last(), Some(StreamEvent::Done { usage }) if usage.output_tokens == 20)
        );
        let items = events
            .iter()
            .find_map(|event| match event {
                StreamEvent::ResponseItems(items) => Some(items.clone()),
                _ => None,
            })
            .unwrap();
        let mut assistant = Message::assistant_tool_calls(vec![types::ToolCall {
            id: "call-test".into(),
            name: "read_file".into(),
            arguments: "{}".into(),
        }]);
        assistant.response_items = items.clone();
        let assistant: Message =
            serde_json::from_str(&serde_json::to_string(&assistant).unwrap()).unwrap();
        let body = request_body("test-model", &[Message::user("read file"), assistant, Message::tool_result("call-test", "contents")], &[json!({"type":"function","function":{"name":"read_file","parameters":{"type":"object"}}})]).unwrap();
        assert_eq!(body["input"][1], items[0]);
        assert_eq!(body["input"][2], items[1]);
        assert_eq!(body["input"][3]["type"], "function_call_output");
        assert_eq!(body["tools"][0]["strict"], false);
        assert_eq!(body["store"], false);
        assert_eq!(body["stream"], true);
        assert_eq!(body["include"][0], "reasoning.encrypted_content");
    }

    #[tokio::test]
    async fn partial_failed_and_malformed_streams_never_release_tools() {
        for terminal in [
            None,
            Some(json!({"type":"response.failed"})),
            Some(json!({"type":"response.incomplete"})),
            Some(
                json!({"type":"response.completed","response":{"status":"incomplete","output":[]}}),
            ),
        ] {
            let mut input = vec![
                json!({"type":"response.output_item.done","item":{"type":"function_call","call_id":"test","name":"run_command","arguments":"{}"}}),
            ];
            input.extend(terminal);
            let events = decode(input).await;
            assert!(matches!(events.last(), Some(StreamEvent::Error(_))));
            assert!(!events.iter().any(|event| matches!(
                event,
                StreamEvent::Done { .. } | StreamEvent::ToolCallStart { .. }
            )));
        }
        let events: Vec<_> =
            decode_events(futures::stream::iter(vec![Ok("not-json".into())]).boxed())
                .collect()
                .await;
        assert!(matches!(events.last(), Some(StreamEvent::Error(_))));
        let mut invalid = completed();
        invalid["response"]["output"][1]["arguments"] = json!("bad");
        assert!(matches!(
            decode(vec![invalid]).await.last(),
            Some(StreamEvent::Error(_))
        ));
    }

    #[tokio::test]
    async fn text_stream_ends_once_at_completion() {
        let events = decode(vec![
            json!({"type":"response.output_text.delta","delta":"hello"}),
            json!({"type":"response.completed","response":{"status":"completed","output":[]}}),
            json!({"type":"error"}),
        ])
        .await;
        assert!(matches!(&events[0], StreamEvent::Token(text) if text == "hello"));
        assert!(matches!(events.last(), Some(StreamEvent::Done { .. })));
    }
}
