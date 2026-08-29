use anyhow::Result;
use futures::StreamExt;
use futures::stream::BoxStream;
use serde_json::Value;
use tokio::sync::mpsc;
use vc_gateway::GatewayClient;
use vc_tools::ToolRegistry;
use vc_types::{AgentEvent, Message, StreamEvent, ToolCall};

const SYSTEM_PROMPT: &str = include_str!("system_prompt.md");

#[async_trait::async_trait]
pub trait ChatBackend: Send + Sync {
    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: &[Value],
    ) -> Result<BoxStream<'static, StreamEvent>>;
}

#[async_trait::async_trait]
impl ChatBackend for GatewayClient {
    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: &[Value],
    ) -> Result<BoxStream<'static, StreamEvent>> {
        Ok(GatewayClient::chat_stream(self, messages, tools).await?)
    }
}

pub struct Agent<B: ChatBackend = GatewayClient> {
    backend: B,
    tools: ToolRegistry,
    history: Vec<Message>,
    max_steps: usize,
}

impl Agent<GatewayClient> {
    pub fn new(gateway: GatewayClient, tools: ToolRegistry) -> Self {
        Self::with_backend(gateway, tools)
    }

    pub fn set_model(&mut self, model: &str) {
        self.backend.set_model(model);
    }

    pub fn gateway_client(&self) -> GatewayClient {
        self.backend.clone()
    }
}

impl<B: ChatBackend> Agent<B> {
    pub fn with_backend(backend: B, tools: ToolRegistry) -> Self {
        Self {
            backend,
            tools,
            history: vec![Message::system(SYSTEM_PROMPT)],
            max_steps: 20,
        }
    }

    pub fn set_skills(&mut self, advertisement: &str) {
        if !advertisement.is_empty() {
            self.history[0] = Message::system(format!("{SYSTEM_PROMPT}\n\n{advertisement}"));
        }
    }

    pub async fn run(
        &mut self,
        user_message: &str,
        events: mpsc::Sender<AgentEvent>,
    ) -> Result<()> {
        self.history.push(Message::user(user_message));

        for _ in 0..self.max_steps {
            let definitions = self.tools.definitions();
            let mut stream = self
                .backend
                .chat_stream(&self.history, &definitions)
                .await?;

            let mut answer = String::new();
            let mut calls: Vec<ToolCall> = Vec::new();

            while let Some(event) = stream.next().await {
                match event {
                    StreamEvent::Token(text) => {
                        answer.push_str(&text);
                        events.send(AgentEvent::Token(text)).await.ok();
                    }
                    StreamEvent::ToolCallStart { id, name } => {
                        calls.push(ToolCall {
                            id,
                            name,
                            arguments: String::new(),
                        });
                    }
                    StreamEvent::ToolCallArgs { id, chunk } => {
                        if let Some(call) = calls.iter_mut().find(|call| call.id == id) {
                            call.arguments.push_str(&chunk);
                        }
                    }
                    StreamEvent::ToolCallEnd { .. } => {}
                    StreamEvent::Done { .. } => {
                        events.send(AgentEvent::StepComplete).await.ok();
                    }
                    StreamEvent::Error(message) => {
                        events.send(AgentEvent::Error(message)).await.ok();
                    }
                }
            }

            if calls.is_empty() {
                self.history.push(Message::assistant(answer));
                events.send(AgentEvent::Done).await.ok();
                return Ok(());
            }

            self.history
                .push(Message::assistant_tool_calls(calls.clone()));

            for call in calls {
                events
                    .send(AgentEvent::ToolStarted {
                        id: call.id.clone(),
                        label: summarize(&call),
                    })
                    .await
                    .ok();
                let result = self.execute(&call, &events).await;
                self.history.push(Message::tool_result(call.id, result));
            }
        }

        events
            .send(AgentEvent::Error(
                "reached max steps without a final answer".to_string(),
            ))
            .await
            .ok();
        events.send(AgentEvent::Done).await.ok();
        Ok(())
    }

    async fn execute(&self, call: &ToolCall, events: &mpsc::Sender<AgentEvent>) -> String {
        let args: Value = serde_json::from_str(&call.arguments).unwrap_or(Value::Null);

        let outcome = match self.tools.get(&call.name) {
            Some(tool) => tool.execute(args).await,
            None => Err(anyhow::anyhow!("unknown tool: {}", call.name)),
        };

        match outcome {
            Ok(value) => {
                events
                    .send(AgentEvent::ToolFinished {
                        id: call.id.clone(),
                        ok: true,
                    })
                    .await
                    .ok();
                value.to_string()
            }
            Err(err) => {
                events
                    .send(AgentEvent::ToolFinished {
                        id: call.id.clone(),
                        ok: false,
                    })
                    .await
                    .ok();
                serde_json::json!({ "error": err.to_string() }).to_string()
            }
        }
    }
}

fn summarize(call: &ToolCall) -> String {
    let value: Value = serde_json::from_str(&call.arguments).unwrap_or(Value::Null);
    let arg = ["query", "url", "path", "command", "from", "old_string"]
        .iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str));
    match arg {
        Some(text) => {
            let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
            let short = if flat.chars().count() > 56 {
                let head: String = flat.chars().take(56).collect();
                format!("{head}…")
            } else {
                flat
            };
            format!("{} {}", call.name, short)
        }
        None => call.name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use vc_tools::ToolRegistry;

    pub struct StubBackend {
        turns: Mutex<VecDeque<Vec<StreamEvent>>>,
        pub seen: Mutex<Vec<Vec<Message>>>,
    }

    impl StubBackend {
        pub fn new(turns: Vec<Vec<StreamEvent>>) -> Self {
            Self {
                turns: Mutex::new(turns.into()),
                seen: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl ChatBackend for StubBackend {
        async fn chat_stream(
            &self,
            messages: &[Message],
            _tools: &[Value],
        ) -> Result<futures::stream::BoxStream<'static, StreamEvent>> {
            self.seen.lock().unwrap().push(messages.to_vec());
            let events = self
                .turns
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| vec![StreamEvent::Error("stub exhausted".into())]);
            Ok(Box::pin(futures::stream::iter(events)))
        }
    }

    fn done() -> StreamEvent {
        StreamEvent::Done {
            usage: vc_types::Usage::default(),
        }
    }

    async fn drain(mut rx: mpsc::Receiver<AgentEvent>) -> Vec<AgentEvent> {
        let mut collected = Vec::new();
        while let Some(event) = rx.recv().await {
            collected.push(event);
        }
        collected
    }

    #[tokio::test]
    async fn plain_answer_streams_tokens_and_finishes() {
        let backend = StubBackend::new(vec![vec![
            StreamEvent::Token("hi".into()),
            StreamEvent::Token(" there".into()),
            done(),
        ]]);
        let mut agent = Agent::with_backend(backend, ToolRegistry::new());
        let (tx, rx) = mpsc::channel(64);
        agent.run("hello", tx).await.unwrap();
        let events = drain(rx).await;
        assert!(matches!(events.last(), Some(AgentEvent::Done)));
        let tokens: String = events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::Token(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(tokens, "hi there");
    }
}
