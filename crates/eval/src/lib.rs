use agent::{Agent, ChatBackend, YesApprover};
use anyhow::{Result, ensure};
use async_trait::async_trait;
use context::SessionStore;
use futures::stream::BoxStream;
use gateway::GatewayClient;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tools::{EditFile, PermissionPolicy, ReadFile, RunCommand, Tool, ToolRegistry, WriteFile};
use types::{AgentEvent, Message, StreamEvent, Usage};

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum Case {
    Boundary,
    Refactor,
    TestRecovery,
}

impl Case {
    pub fn name(self) -> &'static str {
        match self {
            Self::Boundary => "boundary",
            Self::Refactor => "refactor",
            Self::TestRecovery => "test-recovery",
        }
    }
}

#[derive(Serialize)]
pub struct Report {
    pub case: String,
    pub mode: String,
    pub model: Option<String>,
    pub passed: bool,
    pub final_check_passed: bool,
    pub required_edit_present: bool,
    pub agent_completed: bool,
    pub checks_observed: Vec<i64>,
    pub tool_calls: usize,
    pub model_steps: usize,
    pub elapsed_ms: u128,
    pub error: Option<String>,
}

struct Fixture {
    root: PathBuf,
    case: Case,
    original: String,
    acceptance: String,
}

impl Fixture {
    fn new(case: Case) -> Result<Self> {
        let root = std::env::temp_dir().join(format!("codelight-eval-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src"))?;
        std::fs::create_dir(root.join("tests"))?;
        let (original, acceptance) = match case {
            Case::Boundary => (
                "pub fn inclusive_sum(end: u32) -> u32 { (0..end).sum() }\n",
                "use fixture::inclusive_sum;\n#[test]\nfn includes_end() { assert_eq!(inclusive_sum(0), 0); assert_eq!(inclusive_sum(1), 1); assert_eq!(inclusive_sum(5), 15); }\n",
            ),
            Case::Refactor => (
                "mod math;\npub fn total(a: i32, b: i32) -> i32 { math::sum(a, b) }\n",
                "use fixture::total;\n#[test]\nfn preserves_behavior() { assert_eq!(total(2, 3), 5); assert_eq!(total(-2, 2), 0); }\n",
            ),
            Case::TestRecovery => (
                "pub fn double_positive(value: i32) -> i32 { value.abs() * 2 }\n",
                "use fixture::double_positive;\n#[test]\nfn negative_is_zero() { assert_eq!(double_positive(-3), 0); assert_eq!(double_positive(0), 0); assert_eq!(double_positive(4), 8); }\n",
            ),
        };
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n",
        )?;
        std::fs::write(root.join("src/lib.rs"), original)?;
        std::fs::write(
            root.join("src/math.rs"),
            "pub fn sum(a: i32, b: i32) -> i32 { a + b }\n",
        )?;
        std::fs::write(root.join("tests/acceptance.rs"), acceptance)?;
        Ok(Self {
            root,
            case,
            original: original.into(),
            acceptance: acceptance.into(),
        })
    }

    fn command(&self) -> String {
        let path = self
            .root
            .join("Cargo.toml")
            .to_string_lossy()
            .replace('\'', "'\\''");
        format!("cargo test --offline --quiet --manifest-path '{path}'")
    }

    fn prompt(&self) -> String {
        let task = match self.case {
            Case::Boundary => {
                "Fix inclusive_sum so it includes the upper endpoint, including zero."
            }
            Case::Refactor => {
                "Rename math::sum to math::add and update its caller in src/lib.rs, preserving total's public API and behavior."
            }
            Case::TestRecovery => {
                "Run the tests, diagnose the failure, then fix double_positive so negative inputs return zero and nonnegative inputs are doubled. Run the tests again after editing."
            }
        };
        format!(
            "Work only in {}. {task} Read source files before editing. Only src/lib.rs and src/math.rs are writable. Do not modify acceptance tests or the manifest. Relative paths are resolved inside the fixture. The only permitted shell command is exactly: {}",
            self.root.display(),
            self.command()
        )
    }

    fn required_edit_present(&self) -> Result<bool> {
        let source = std::fs::read_to_string(self.root.join("src/lib.rs"))?;
        let tests = std::fs::read_to_string(self.root.join("tests/acceptance.rs"))?;
        if tests != self.acceptance || source == self.original {
            return Ok(false);
        }
        if matches!(self.case, Case::Refactor) {
            let math = std::fs::read_to_string(self.root.join("src/math.rs"))?;
            return Ok(math.contains("fn add(")
                && !math.contains("fn sum(")
                && source.contains("math::add("));
        }
        Ok(true)
    }

    async fn verify(&self) -> Result<bool> {
        let run = tokio::process::Command::new("cargo")
            .args(["test", "--offline", "--quiet", "--manifest-path"])
            .arg(self.root.join("Cargo.toml"))
            .kill_on_drop(true)
            .output();
        Ok(tokio::time::timeout(Duration::from_secs(60), run)
            .await??
            .status
            .success())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

struct FixtureTool {
    inner: Box<dyn Tool>,
    root: PathBuf,
    command: String,
    checks: Arc<Mutex<Vec<i64>>>,
}

#[async_trait]
impl Tool for FixtureTool {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn description(&self) -> &str {
        self.inner.description()
    }
    fn parameters_schema(&self) -> Value {
        self.inner.parameters_schema()
    }

    async fn execute(&self, mut args: Value) -> Result<Value> {
        if self.name() == "run_command" {
            ensure!(
                args["command"].as_str() == Some(self.command.as_str()),
                "only the fixture verification command is permitted"
            );
        } else {
            let path = Path::new(
                args["path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("missing path"))?,
            );
            let path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                self.root.join(path)
            }
            .canonicalize()?;
            let allowed = if self.name() == "read_file" {
                vec![
                    "src/lib.rs",
                    "src/math.rs",
                    "tests/acceptance.rs",
                    "Cargo.toml",
                ]
            } else {
                vec!["src/lib.rs", "src/math.rs"]
            };
            ensure!(
                allowed.iter().any(|name| self.root.join(name) == path),
                "path is outside the permitted fixture files"
            );
            args["path"] = json!(path);
        }
        let result = self.inner.execute(args).await?;
        if self.name() == "run_command" {
            self.checks
                .lock()
                .unwrap()
                .push(result["exit_code"].as_i64().unwrap_or(-1));
        }
        Ok(result)
    }
}

struct ScriptedBackend {
    turns: Mutex<VecDeque<Vec<StreamEvent>>>,
}

impl ScriptedBackend {
    fn new(fixture: &Fixture) -> Self {
        let mut turns = vec![tool_turn("read_file", json!({"path":"src/lib.rs"}))];
        if matches!(fixture.case, Case::Refactor) {
            turns.push(tool_turn("read_file", json!({"path":"src/math.rs"})));
        }
        turns.push(tool_turn(
            "run_command",
            json!({"command":fixture.command()}),
        ));
        let (old, new) = match fixture.case {
            Case::Boundary => ("0..end", "0..=end"),
            Case::Refactor => ("math::sum", "math::add"),
            Case::TestRecovery => ("value.abs()", "value.max(0)"),
        };
        turns.push(tool_turn(
            "edit_file",
            json!({"path":"src/lib.rs","old_string":old,"new_string":new}),
        ));
        if matches!(fixture.case, Case::Refactor) {
            turns.push(tool_turn(
                "edit_file",
                json!({"path":"src/math.rs","old_string":"fn sum(","new_string":"fn add("}),
            ));
        }
        turns.push(tool_turn(
            "run_command",
            json!({"command":fixture.command()}),
        ));
        turns.push(vec![
            StreamEvent::Token("Implemented and verified.".into()),
            StreamEvent::Done {
                usage: Usage::default(),
            },
        ]);
        Self {
            turns: Mutex::new(turns.into()),
        }
    }
}

fn tool_turn(name: &str, args: Value) -> Vec<StreamEvent> {
    let id = uuid::Uuid::new_v4().to_string();
    vec![
        StreamEvent::ToolCallStart {
            id: id.clone(),
            name: name.into(),
        },
        StreamEvent::ToolCallArgs {
            id: id.clone(),
            chunk: args.to_string(),
        },
        StreamEvent::ToolCallEnd { id },
        StreamEvent::Done {
            usage: Usage::default(),
        },
    ]
}

#[async_trait]
impl ChatBackend for ScriptedBackend {
    async fn chat_stream(
        &self,
        _messages: &[Message],
        _tools: &[Value],
    ) -> Result<BoxStream<'static, StreamEvent>> {
        let turn = self
            .turns
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("script exhausted"))?;
        Ok(Box::pin(futures::stream::iter(turn)))
    }
}

pub async fn evaluate(case: Case, live: bool) -> Result<Report> {
    let fixture = Fixture::new(case)?;
    if live {
        let gateway = GatewayClient::from_env()?;
        let model = gateway.model().to_string();
        run(fixture, gateway, Some(model)).await
    } else {
        let backend = ScriptedBackend::new(&fixture);
        run(fixture, backend, None).await
    }
}

async fn run<B: ChatBackend>(
    fixture: Fixture,
    backend: B,
    model: Option<String>,
) -> Result<Report> {
    let started = Instant::now();
    let checks = Arc::new(Mutex::new(Vec::new()));
    let mut tools = ToolRegistry::new();
    for inner in [
        Box::new(ReadFile) as Box<dyn Tool>,
        Box::new(EditFile),
        Box::new(WriteFile),
        Box::new(RunCommand),
    ] {
        tools.register(Box::new(FixtureTool {
            inner,
            root: fixture.root.canonicalize()?,
            command: fixture.command(),
            checks: checks.clone(),
        }));
    }
    let mut agent = Agent::with_backend(
        backend,
        tools,
        Arc::new(YesApprover),
        PermissionPolicy::load(fixture.root.join(".codelight.toml")),
    );
    let (session, history) =
        SessionStore::open(&fixture.root.join("session.json"), &fixture.root, false)?;
    agent.attach_session(session, history)?;
    let (tx, mut rx) = mpsc::channel(256);
    let collect = tokio::spawn(async move {
        let mut calls = 0;
        let mut steps = 0;
        while let Some(event) = rx.recv().await {
            match event {
                AgentEvent::ToolStarted { .. } => calls += 1,
                AgentEvent::StepComplete => steps += 1,
                _ => {}
            }
        }
        (calls, steps)
    });
    let result =
        tokio::time::timeout(Duration::from_secs(120), agent.run(&fixture.prompt(), tx)).await;
    let (agent_completed, error) = match result {
        Ok(Ok(())) => (true, None),
        Ok(Err(error)) => (false, Some(error.to_string())),
        Err(_) => (false, Some("evaluation exceeded 120 seconds".into())),
    };
    let (tool_calls, model_steps) = collect.await?;
    let required_edit_present = fixture.required_edit_present()?;
    let final_check_passed = fixture.verify().await?;
    let checks_observed = checks.lock().unwrap().clone();
    let checked = checks_observed.last() == Some(&0);
    let recovered = !matches!(fixture.case, Case::TestRecovery)
        || checks_observed.iter().any(|code| *code != 0);
    Ok(Report {
        case: fixture.case.name().into(),
        mode: if model.is_some() { "live" } else { "scripted" }.into(),
        model,
        passed: agent_completed
            && final_check_passed
            && required_edit_present
            && checked
            && recovered,
        final_check_passed,
        required_edit_present,
        agent_completed,
        checks_observed,
        tool_calls,
        model_steps,
        elapsed_ms: started.elapsed().as_millis(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scripted_tasks_pass_real_acceptance_tests() {
        for case in [Case::Boundary, Case::Refactor, Case::TestRecovery] {
            let report = evaluate(case, false).await.unwrap();
            assert!(report.passed, "{}", serde_json::to_string(&report).unwrap());
            assert!(report.tool_calls >= 4);
            assert!(report.model_steps > report.tool_calls);
            if matches!(case, Case::TestRecovery) {
                assert_ne!(report.checks_observed[0], 0);
                assert_eq!(report.checks_observed.last(), Some(&0));
            }
        }
    }

    #[tokio::test]
    async fn claimed_success_without_a_fix_fails_evaluation() {
        let fixture = Fixture::new(Case::Boundary).unwrap();
        let backend = ScriptedBackend {
            turns: Mutex::new(
                vec![vec![
                    StreamEvent::Token("Done, all tests pass.".into()),
                    StreamEvent::Done {
                        usage: Usage::default(),
                    },
                ]]
                .into(),
            ),
        };
        let report = run(fixture, backend, None).await.unwrap();
        assert!(!report.passed);
        assert!(!report.final_check_passed);
        assert!(!report.required_edit_present);
    }

    #[tokio::test]
    async fn fixture_tools_refuse_test_edits_and_arbitrary_commands() {
        let fixture = Fixture::new(Case::Boundary).unwrap();
        let root = fixture.root.canonicalize().unwrap();
        let checks = Arc::new(Mutex::new(Vec::new()));
        let tool = FixtureTool {
            inner: Box::new(WriteFile),
            root: root.clone(),
            command: fixture.command(),
            checks: checks.clone(),
        };
        assert!(
            tool.execute(json!({"path":"tests/acceptance.rs","contents":""}))
                .await
                .is_err()
        );
        assert!(
            tool.execute(json!({"path":"../outside","contents":""}))
                .await
                .is_err()
        );
        let tool = FixtureTool {
            inner: Box::new(RunCommand),
            root,
            command: fixture.command(),
            checks,
        };
        assert!(
            tool.execute(json!({"command":"echo success"}))
                .await
                .is_err()
        );
    }
}
