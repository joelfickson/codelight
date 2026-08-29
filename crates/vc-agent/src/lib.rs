use anyhow::Result;
use futures::StreamExt;
use futures::stream::BoxStream;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use vc_gateway::GatewayClient;
use vc_tools::PermissionPolicy;
use vc_tools::ToolRegistry;
use vc_types::{AgentEvent, ApprovalRequest, Decision, Message, StreamEvent, ToolCall};

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
pub trait Approver: Send + Sync {
    async fn approve(&self, request: ApprovalRequest) -> Decision;
}

pub struct YesApprover;

#[async_trait::async_trait]
impl Approver for YesApprover {
    async fn approve(&self, _request: ApprovalRequest) -> Decision {
        Decision::AllowOnce
    }
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
    approver: Arc<dyn Approver>,
    policy: Mutex<PermissionPolicy>,
}

impl Agent<GatewayClient> {
    pub fn new(
        gateway: GatewayClient,
        tools: ToolRegistry,
        approver: Arc<dyn Approver>,
        policy: PermissionPolicy,
    ) -> Self {
        Self::with_backend(gateway, tools, approver, policy)
    }

    pub fn set_model(&mut self, model: &str) {
        self.backend.set_model(model);
    }

    pub fn gateway_client(&self) -> GatewayClient {
        self.backend.clone()
    }
}

impl<B: ChatBackend> Agent<B> {
    pub fn with_backend(
        backend: B,
        tools: ToolRegistry,
        approver: Arc<dyn Approver>,
        policy: PermissionPolicy,
    ) -> Self {
        Self {
            backend,
            tools,
            history: vec![Message::system(SYSTEM_PROMPT)],
            max_steps: 20,
            approver,
            policy: Mutex::new(policy),
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

        let mut mutated_since_check = false;
        let mut nudged = false;

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
                if mutated_since_check && !nudged {
                    nudged = true;
                    self.history.push(Message::system(
                        "You modified files this turn but ran no checks. Run the project's build, test, or lint command to verify your changes, or state explicitly why verification is not needed.",
                    ));
                    continue;
                }
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
                let (result, executed_ok) = self.execute(&call, &events).await;
                if executed_ok {
                    match call.name.as_str() {
                        "edit_file" | "write_file" | "delete_file" | "move_file" => {
                            mutated_since_check = true;
                        }
                        "run_command" => {
                            mutated_since_check = false;
                        }
                        _ => {}
                    }
                }
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

    async fn execute(&self, call: &ToolCall, events: &mpsc::Sender<AgentEvent>) -> (String, bool) {
        let args: Value = serde_json::from_str(&call.arguments).unwrap_or(Value::Null);

        let Some(tool) = self.tools.get(&call.name) else {
            events
                .send(AgentEvent::ToolFinished {
                    id: call.id.clone(),
                    ok: false,
                })
                .await
                .ok();
            return (
                serde_json::json!({ "error": format!("unknown tool: {}", call.name) }).to_string(),
                false,
            );
        };

        let request = {
            let policy = self.policy.lock().unwrap();
            tool.approval_request(&args, &policy)
        };

        if let Some(request) = request {
            let pattern = request.suggested_pattern.clone();
            match self.approver.approve(request).await {
                Decision::Deny => {
                    events
                        .send(AgentEvent::ToolFinished {
                            id: call.id.clone(),
                            ok: false,
                        })
                        .await
                        .ok();
                    return (
                        serde_json::json!({ "error": "the user declined to allow this action" })
                            .to_string(),
                        false,
                    );
                }
                Decision::AllowAlways => {
                    if let Some(pattern) = pattern {
                        let persist_error = {
                            let mut policy = self.policy.lock().unwrap();
                            policy.persist_allow(&pattern).err()
                        };
                        if let Some(err) = persist_error {
                            events
                                .send(AgentEvent::Info(format!(
                                    "could not persist allow pattern: {err}"
                                )))
                                .await
                                .ok();
                        }
                    }
                }
                Decision::AllowOnce => {}
            }
        }

        match tool.execute(args).await {
            Ok(value) => {
                events
                    .send(AgentEvent::ToolFinished {
                        id: call.id.clone(),
                        ok: true,
                    })
                    .await
                    .ok();
                (value.to_string(), true)
            }
            Err(err) => {
                events
                    .send(AgentEvent::ToolFinished {
                        id: call.id.clone(),
                        ok: false,
                    })
                    .await
                    .ok();
                (
                    serde_json::json!({ "error": err.to_string() }).to_string(),
                    false,
                )
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
    use std::sync::Arc;
    use std::sync::Mutex;
    use vc_tools::{PermissionPolicy, RunCommand, ToolRegistry};

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
        let mut agent = Agent::with_backend(
            backend,
            ToolRegistry::new(),
            Arc::new(YesApprover),
            empty_policy("plain"),
        );
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

    struct ScriptedApprover {
        decision: Decision,
        asked: Mutex<Vec<ApprovalRequest>>,
    }

    impl ScriptedApprover {
        fn new(decision: Decision) -> Self {
            Self {
                decision,
                asked: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl Approver for ScriptedApprover {
        async fn approve(&self, request: ApprovalRequest) -> Decision {
            self.asked.lock().unwrap().push(request);
            self.decision
        }
    }

    fn empty_policy(name: &str) -> PermissionPolicy {
        PermissionPolicy::load(std::env::temp_dir().join(format!("vc_agent_{name}.toml")))
    }

    fn tool_call_turn(name: &str, arguments: &str) -> Vec<StreamEvent> {
        vec![
            StreamEvent::ToolCallStart {
                id: "call_1".into(),
                name: name.into(),
            },
            StreamEvent::ToolCallArgs {
                id: "call_1".into(),
                chunk: arguments.into(),
            },
            StreamEvent::ToolCallEnd {
                id: "call_1".into(),
            },
            done(),
        ]
    }

    fn answer_turn(text: &str) -> Vec<StreamEvent> {
        vec![StreamEvent::Token(text.into()), done()]
    }

    #[tokio::test]
    async fn denied_command_is_not_executed_and_model_sees_decline() {
        let backend = StubBackend::new(vec![
            tool_call_turn("run_command", r#"{"command": "rm -rf build"}"#),
            answer_turn("understood"),
        ]);
        let mut tools = ToolRegistry::new();
        tools.register(Box::new(RunCommand));
        let approver = Arc::new(ScriptedApprover::new(Decision::Deny));
        let mut agent = Agent::with_backend(backend, tools, approver.clone(), empty_policy("deny"));
        let (tx, rx) = mpsc::channel(64);
        agent.run("clean the build dir", tx).await.unwrap();
        drain(rx).await;
        assert_eq!(approver.asked.lock().unwrap().len(), 1);
        let seen = agent.backend.seen.lock().unwrap();
        let second_turn = &seen[1];
        let declined = second_turn.iter().any(|m| m.content.contains("declined"));
        assert!(declined);
    }

    #[tokio::test]
    async fn allowed_policy_command_skips_the_approver() {
        let backend = StubBackend::new(vec![
            tool_call_turn("run_command", r#"{"command": "git status"}"#),
            answer_turn("clean tree"),
        ]);
        let mut tools = ToolRegistry::new();
        tools.register(Box::new(RunCommand));
        let approver = Arc::new(ScriptedApprover::new(Decision::Deny));
        let mut agent = Agent::with_backend(backend, tools, approver.clone(), empty_policy("skip"));
        let (tx, rx) = mpsc::channel(64);
        agent.run("check git", tx).await.unwrap();
        drain(rx).await;
        assert!(approver.asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn allow_once_executes_the_command() {
        let backend = StubBackend::new(vec![
            tool_call_turn("run_command", r#"{"command": "echo approved-run"}"#),
            answer_turn("done"),
        ]);
        let mut tools = ToolRegistry::new();
        tools.register(Box::new(RunCommand));
        let approver = Arc::new(ScriptedApprover::new(Decision::AllowOnce));
        let mut agent = Agent::with_backend(backend, tools, approver, empty_policy("once"));
        let (tx, rx) = mpsc::channel(64);
        agent.run("say hi", tx).await.unwrap();
        drain(rx).await;
        let seen = agent.backend.seen.lock().unwrap();
        let second_turn = &seen[1];
        let executed = second_turn
            .iter()
            .any(|m| m.content.contains("approved-run"));
        assert!(executed);
    }

    struct NamedStubTool {
        tool_name: &'static str,
    }

    #[async_trait::async_trait]
    impl vc_tools::Tool for NamedStubTool {
        fn name(&self) -> &str {
            self.tool_name
        }
        fn description(&self) -> &str {
            "stub"
        }
        fn parameters_schema(&self) -> Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _args: Value) -> anyhow::Result<Value> {
            Ok(serde_json::json!({"ok": true}))
        }
    }

    #[tokio::test]
    async fn finishing_after_mutation_without_check_triggers_one_nudge() {
        let backend = StubBackend::new(vec![
            tool_call_turn("edit_file", r#"{}"#),
            answer_turn("all done"),
            answer_turn("really done"),
        ]);
        let mut tools = ToolRegistry::new();
        tools.register(Box::new(NamedStubTool {
            tool_name: "edit_file",
        }));
        let mut agent =
            Agent::with_backend(backend, tools, Arc::new(YesApprover), empty_policy("nudge"));
        let (tx, rx) = mpsc::channel(64);
        agent.run("edit something", tx).await.unwrap();
        drain(rx).await;
        let seen = agent.backend.seen.lock().unwrap();
        assert_eq!(seen.len(), 3);
        let nudge_turn = &seen[2];
        let nudged = nudge_turn
            .iter()
            .any(|m| m.content.contains("ran no checks"));
        assert!(nudged);
    }

    #[tokio::test]
    async fn run_command_clears_the_mutation_flag() {
        let backend = StubBackend::new(vec![
            tool_call_turn("edit_file", r#"{}"#),
            tool_call_turn("run_command", r#"{"command": "echo checked"}"#),
            answer_turn("verified and done"),
        ]);
        let mut tools = ToolRegistry::new();
        tools.register(Box::new(NamedStubTool {
            tool_name: "edit_file",
        }));
        tools.register(Box::new(RunCommand));
        let mut agent = Agent::with_backend(
            backend,
            tools,
            Arc::new(YesApprover),
            empty_policy("cleared"),
        );
        let (tx, rx) = mpsc::channel(64);
        agent.run("edit and verify", tx).await.unwrap();
        drain(rx).await;
        let seen = agent.backend.seen.lock().unwrap();
        assert_eq!(seen.len(), 3);
    }

    #[tokio::test]
    async fn no_mutation_means_no_nudge() {
        let mut agent = Agent::with_backend(
            StubBackend::new(vec![answer_turn("just an answer")]),
            ToolRegistry::new(),
            Arc::new(YesApprover),
            empty_policy("clean"),
        );
        let (tx, rx) = mpsc::channel(64);
        agent.run("hello", tx).await.unwrap();
        drain(rx).await;
        let seen = agent.backend.seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
    }
}
