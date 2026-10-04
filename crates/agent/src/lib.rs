use anyhow::{Result, bail};
use context::{DEFAULT_CONTEXT_BYTES, SessionStore, bounded_context, repair_interrupted_calls};
use futures::StreamExt;
use futures::stream::BoxStream;
use gateway::GatewayClient;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tools::PermissionPolicy;
use tools::ToolRegistry;
use types::{AgentEvent, ApprovalRequest, Decision, Message, StreamEvent, ToolCall};

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
    max_steps: Option<usize>,
    context_bytes: usize,
    session: Option<SessionStore>,
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
            max_steps: None,
            context_bytes: DEFAULT_CONTEXT_BYTES,
            session: None,
            approver,
            policy: Mutex::new(policy),
        }
    }

    pub fn set_skills(&mut self, advertisement: &str) {
        if !advertisement.is_empty() {
            self.history[0] = Message::system(format!("{SYSTEM_PROMPT}\n\n{advertisement}"));
        }
    }

    pub fn configure_limits(
        &mut self,
        max_steps: Option<usize>,
        context_bytes: usize,
    ) -> Result<()> {
        anyhow::ensure!(
            max_steps.is_none_or(|limit| limit > 0) && context_bytes > 0,
            "step and context limits must be positive"
        );
        self.max_steps = max_steps;
        self.context_bytes = context_bytes;
        Ok(())
    }

    pub fn attach_session(
        &mut self,
        session: SessionStore,
        mut history: Vec<Message>,
    ) -> Result<()> {
        if !history.is_empty() {
            history[0] = self.history[0].clone();
            self.history = history;
        }
        self.session = Some(session);
        self.checkpoint()
    }

    fn checkpoint(&self) -> Result<()> {
        if let Some(session) = &self.session {
            session.save(&self.history)?;
        }
        Ok(())
    }

    pub async fn run(
        &mut self,
        user_message: &str,
        events: mpsc::Sender<AgentEvent>,
    ) -> Result<()> {
        let result = self.run_turn(user_message, &events).await;
        if result.is_err()
            && let Err(error) = repair_interrupted_calls(&mut self.history)
        {
            events.send(AgentEvent::Error(error.to_string())).await.ok();
        }
        let checkpoint = self.checkpoint();
        let result = result.and(checkpoint);
        if let Err(error) = &result {
            events.send(AgentEvent::Error(error.to_string())).await.ok();
        }
        events.send(AgentEvent::Done).await.ok();
        result
    }

    async fn run_turn(
        &mut self,
        user_message: &str,
        events: &mpsc::Sender<AgentEvent>,
    ) -> Result<()> {
        self.history.push(Message::user(user_message));
        self.checkpoint()?;

        let mut mutated_since_check = false;
        let mut nudged = false;

        let mut step = 0;
        loop {
            if self.max_steps.is_some_and(|limit| step >= limit) {
                bail!("reached max steps without a final answer");
            }
            step += 1;
            let definitions = self.tools.definitions();
            let (context, omitted) =
                bounded_context(&self.history, &definitions, self.context_bytes)?;
            if omitted > 0 && step == 1 {
                events.send(AgentEvent::Info(format!("Omitted {omitted} earlier messages from model context; full session history is retained."))).await.ok();
            }
            let mut stream = self.backend.chat_stream(&context, &definitions).await?;

            let mut answer = String::new();
            let mut calls: Vec<ToolCall> = Vec::new();
            let mut completed = false;
            let mut response_items = Vec::new();

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
                    StreamEvent::ResponseItems(items) => response_items = items,
                    StreamEvent::ToolCallEnd { .. } => {}
                    StreamEvent::Done { .. } => {
                        completed = true;
                        events.send(AgentEvent::StepComplete).await.ok();
                    }
                    StreamEvent::Error(message) => {
                        bail!("model stream failed: {message}");
                    }
                }
            }

            if !completed {
                bail!("model stream ended before completion; no pending tools were executed");
            }

            if calls.is_empty() {
                let mut message = Message::assistant(answer);
                message.response_items = response_items;
                self.history.push(message);
                if mutated_since_check && !nudged && self.max_steps.is_none_or(|limit| step < limit)
                {
                    nudged = true;
                    self.history.push(Message::system(
                        "You modified files this turn but ran no checks. Run the project's build, test, or lint command to verify your changes, or state explicitly why verification is not needed.",
                    ));
                    continue;
                }
                return Ok(());
            }

            let mut message = Message::assistant_tool_calls(calls.clone());
            message.content = answer;
            message.response_items = response_items;
            self.history.push(message);
            self.checkpoint()?;

            for call in calls {
                events
                    .send(AgentEvent::ToolStarted {
                        id: call.id.clone(),
                        label: summarize(&call),
                    })
                    .await
                    .ok();
                let (result, executed_ok) = self.execute(&call, events).await;
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
                self.checkpoint()?;
            }
        }
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
    use tools::{PermissionPolicy, RunCommand, ToolRegistry};

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
            usage: types::Usage::default(),
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
        PermissionPolicy::load(std::env::temp_dir().join(format!("agent_{name}.toml")))
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
    impl tools::Tool for NamedStubTool {
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
    async fn last_step_nudge_is_skipped_and_finishes_normally() {
        let backend = StubBackend::new(vec![
            tool_call_turn("edit_file", r#"{}"#),
            answer_turn("done"),
        ]);
        let mut tools = ToolRegistry::new();
        tools.register(Box::new(NamedStubTool {
            tool_name: "edit_file",
        }));
        let mut agent = Agent::with_backend(
            backend,
            tools,
            Arc::new(YesApprover),
            empty_policy("last_step_nudge"),
        );
        agent.max_steps = Some(2);
        let (tx, rx) = mpsc::channel(64);
        agent.run("edit something", tx).await.unwrap();
        let events = drain(rx).await;
        assert!(matches!(events.last(), Some(AgentEvent::Done)));
        assert!(!events.iter().any(|e| matches!(e, AgentEvent::Error(_))));
        let seen = agent.backend.seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
    }

    #[tokio::test]
    async fn allow_always_persists_pattern_and_executes() {
        let path = std::env::temp_dir().join("agent_allow_always.toml");
        std::fs::remove_file(&path).ok();
        let backend = StubBackend::new(vec![
            tool_call_turn("run_command", r#"{"command": "echo persisted"}"#),
            answer_turn("ok"),
        ]);
        let mut tools = ToolRegistry::new();
        tools.register(Box::new(RunCommand));
        let approver = Arc::new(ScriptedApprover::new(Decision::AllowAlways));
        let policy = PermissionPolicy::load(&path);
        let mut agent = Agent::with_backend(backend, tools, approver, policy);
        let (tx, rx) = mpsc::channel(64);
        agent.run("run a command", tx).await.unwrap();
        drain(rx).await;
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("echo *"));
        let seen = agent.backend.seen.lock().unwrap();
        let second_turn = &seen[1];
        let executed = second_turn.iter().any(|m| m.content.contains("persisted"));
        assert!(executed);
        std::fs::remove_file(&path).ok();
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
    struct FailOnceBackend {
        failed: std::sync::atomic::AtomicBool,
    }

    #[async_trait::async_trait]
    impl ChatBackend for FailOnceBackend {
        async fn chat_stream(
            &self,
            _messages: &[Message],
            _tools: &[Value],
        ) -> Result<BoxStream<'static, StreamEvent>> {
            if !self.failed.swap(true, std::sync::atomic::Ordering::SeqCst) {
                bail!("connection unavailable");
            }
            Ok(Box::pin(futures::stream::iter(answer_turn("recovered"))))
        }
    }

    #[tokio::test]
    async fn request_failure_completes_and_next_turn_recovers() {
        let mut agent = Agent::with_backend(
            FailOnceBackend {
                failed: std::sync::atomic::AtomicBool::new(false),
            },
            ToolRegistry::new(),
            Arc::new(YesApprover),
            empty_policy("recovery"),
        );
        let (tx, rx) = mpsc::channel(64);
        assert!(agent.run("first", tx).await.is_err());
        let events = drain(rx).await;
        assert!(matches!(events.first(), Some(AgentEvent::Error(_))));
        assert!(matches!(events.last(), Some(AgentEvent::Done)));
        let (tx, rx) = mpsc::channel(64);
        agent.run("try again", tx).await.unwrap();
        let events = drain(rx).await;
        assert!(
            events
                .iter()
                .any(|event| matches!(event, AgentEvent::Token(text) if text == "recovered"))
        );
    }

    #[tokio::test]
    async fn incomplete_stream_never_executes_pending_tools() {
        for error in [false, true] {
            let mut turn = tool_call_turn("run_command", r#"{"command":"pwd"}"#);
            turn.pop();
            if error {
                turn.push(StreamEvent::Error("connection lost".into()));
            }
            let backend = StubBackend::new(vec![turn]);
            let mut tools = ToolRegistry::new();
            tools.register(Box::new(RunCommand));
            let mut agent = Agent::with_backend(
                backend,
                tools,
                Arc::new(YesApprover),
                empty_policy("partial"),
            );
            let (tx, rx) = mpsc::channel(64);
            assert!(agent.run("run", tx).await.is_err());
            let events = drain(rx).await;
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, AgentEvent::ToolStarted { .. }))
            );
            assert!(matches!(events.last(), Some(AgentEvent::Done)));
            assert_eq!(agent.history.len(), 2);
        }
    }
    #[tokio::test]
    async fn checkpoint_survives_tool_execution_then_model_failure() {
        let directory = std::env::temp_dir().join(format!(
            "vc-agent-resume-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("session.json");
        let (store, history) = SessionStore::open(&path, &directory, false).unwrap();
        let backend = StubBackend::new(vec![
            tool_call_turn("run_command", r#"{"command":"pwd"}"#),
            vec![StreamEvent::Error("offline".into())],
        ]);
        let mut tools = ToolRegistry::new();
        tools.register(Box::new(RunCommand));
        let mut agent = Agent::with_backend(
            backend,
            tools,
            Arc::new(YesApprover),
            empty_policy("checkpoint"),
        );
        agent.attach_session(store, history).unwrap();
        let (tx, rx) = mpsc::channel(64);
        assert!(agent.run("inspect", tx).await.is_err());
        assert!(
            drain(rx)
                .await
                .iter()
                .any(|event| matches!(event, AgentEvent::ToolFinished { ok: true, .. }))
        );
        drop(agent);
        let (store, history) = SessionStore::open(&path, &directory, true).unwrap();
        assert_eq!(history.last().unwrap().role, types::Role::Tool);
        assert!(history.last().unwrap().content.contains("exit_code"));
        let mut resumed = Agent::with_backend(
            StubBackend::new(vec![answer_turn("continued")]),
            ToolRegistry::new(),
            Arc::new(YesApprover),
            empty_policy("resumed"),
        );
        resumed.set_skills("current skills");
        resumed.attach_session(store, history).unwrap();
        let (tx, rx) = mpsc::channel(64);
        resumed.run("continue", tx).await.unwrap();
        drain(rx).await;
        {
            let seen = resumed.backend.seen.lock().unwrap();
            assert!(seen[0][0].content.contains("current skills"));
            assert!(
                seen[0]
                    .iter()
                    .any(|message| message.role == types::Role::Tool)
            );
        }
        drop(resumed);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn context_overflow_completes_without_calling_backend() {
        let mut agent = Agent::with_backend(
            StubBackend::new(vec![]),
            ToolRegistry::new(),
            Arc::new(YesApprover),
            empty_policy("overflow"),
        );
        agent.configure_limits(None, 1).unwrap();
        let (tx, rx) = mpsc::channel(64);
        assert!(agent.run("hello", tx).await.is_err());
        assert!(agent.backend.seen.lock().unwrap().is_empty());
        assert!(matches!(drain(rx).await.last(), Some(AgentEvent::Done)));
    }
    #[tokio::test]
    async fn unlimited_turn_continues_past_twenty_steps_and_still_nudges() {
        let mut turns = vec![tool_call_turn("edit_file", "{}"); 25];
        turns.push(answer_turn("finished editing"));
        turns.push(answer_turn("verification is not needed for this fixture"));
        let mut tools = ToolRegistry::new();
        tools.register(Box::new(NamedStubTool {
            tool_name: "edit_file",
        }));
        let mut agent = Agent::with_backend(
            StubBackend::new(turns),
            tools,
            Arc::new(YesApprover),
            empty_policy("unlimited"),
        );
        let (tx, rx) = mpsc::channel(256);
        agent.run("keep working", tx).await.unwrap();
        let events = drain(rx).await;
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, AgentEvent::ToolFinished { ok: true, .. }))
                .count(),
            25
        );
        assert!(matches!(events.last(), Some(AgentEvent::Done)));
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, AgentEvent::Error(_)))
        );
        let seen = agent.backend.seen.lock().unwrap();
        assert_eq!(seen.len(), 27);
        assert!(
            seen.last()
                .unwrap()
                .iter()
                .any(|message| message.content.contains("ran no checks"))
        );
    }

    #[tokio::test]
    async fn explicit_step_limit_still_stops_and_completes_turn() {
        let mut tools = ToolRegistry::new();
        tools.register(Box::new(NamedStubTool {
            tool_name: "read_file",
        }));
        let mut agent = Agent::with_backend(
            StubBackend::new(vec![tool_call_turn("read_file", "{}"); 3]),
            tools,
            Arc::new(YesApprover),
            empty_policy("limited"),
        );
        assert!(
            agent
                .configure_limits(Some(0), DEFAULT_CONTEXT_BYTES)
                .is_err()
        );
        agent
            .configure_limits(Some(2), DEFAULT_CONTEXT_BYTES)
            .unwrap();
        let (tx, rx) = mpsc::channel(64);
        assert!(
            agent
                .run("inspect", tx)
                .await
                .unwrap_err()
                .to_string()
                .contains("max steps")
        );
        assert_eq!(agent.backend.seen.lock().unwrap().len(), 2);
        assert!(matches!(drain(rx).await.last(), Some(AgentEvent::Done)));
    }
}
