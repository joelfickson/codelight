mod app;

use anyhow::Result;
use clap::Parser;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tui_input::InputRequest;
use vc_agent::{Agent, Approver, YesApprover};
use vc_gateway::{DEFAULT_MODEL, GatewayClient};
use vc_skills::{AddSkill, LoadSkill, ReadSkillResource, SearchSkills, SkillRegistry};
use vc_tools::{
    DeleteFile, EditFile, ListDirectory, MoveFile, PermissionPolicy, ReadFile, RunCommand,
    SearchDocs, SearchInFiles, ToolRegistry, WebFetch, WriteFile,
};
use vc_types::{AgentEvent, ApprovalRequest, Decision, Message};

use app::App;

pub struct PendingApproval {
    pub request: ApprovalRequest,
    pub respond: oneshot::Sender<Decision>,
}

struct TuiApprover {
    tx: mpsc::Sender<PendingApproval>,
}

#[async_trait::async_trait]
impl Approver for TuiApprover {
    async fn approve(&self, request: ApprovalRequest) -> Decision {
        let (respond, wait) = oneshot::channel();
        if self
            .tx
            .send(PendingApproval { request, respond })
            .await
            .is_err()
        {
            return Decision::Deny;
        }
        wait.await.unwrap_or(Decision::Deny)
    }
}

#[derive(Parser)]
#[command(name = "codelight", about = "A general-purpose AI coding agent")]
struct Cli {
    #[arg(long, help = "Validate the model connection and exit")]
    check: bool,
    #[arg(long, help = "Show a sample session without connecting to a model")]
    demo: bool,
    #[arg(long, help = "Model ID supported by the configured endpoint")]
    model: Option<String>,
    #[arg(long, help = "List models from the configured endpoint and exit")]
    list_models: bool,
    #[arg(long, help = "Skip all approval prompts and allow every tool call")]
    yolo: bool,
    #[arg(long, help = "Initialize the coding agent in a directory")]
    init: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    if cli.check {
        return check(cli.model).await;
    }

    if cli.list_models {
        return list_models_cli(cli.model).await;
    }

    if cli.init {
        return initialize_project();
    }

    run(cli.demo, cli.model, cli.yolo).await
}

fn initialize_project() -> Result<()> {
    let directory = std::env::current_dir()?;
    let mut found = false;

    for name in ["AGENTS.md", "CLAUDE.md"] {
        let path = directory.join(name);

        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => {
                println!("Found {}", path.display());
                found = true;
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    if !found {
        println!("No AGENTS.md or CLAUDE.md found in {}", directory.display());
    }

    Ok(())
}

async fn list_models_cli(model: Option<String>) -> Result<()> {
    let gateway = GatewayClient::from_env_with_model(model)?;
    for id in gateway.list_models().await? {
        println!("{id}");
    }
    Ok(())
}

fn project_name() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "project".to_string())
}

fn model_alias(model: &str) -> String {
    let short = model.split('/').next_back().unwrap_or(model);
    if let Some(rest) = short.strip_prefix("claude-") {
        if let Some((tier, version)) = rest.split_once('-') {
            return format!("{tier} {}", version.replace('-', "."));
        }
        return rest.to_string();
    }
    short.to_string()
}

async fn check(model: Option<String>) -> Result<()> {
    let gateway = GatewayClient::from_env_with_model(model)?;
    let (_reply, usage) = gateway
        .chat(&[Message::user("Reply with the single word: ok")])
        .await?;
    println!(
        "model connection: ok ({} in / {} out tokens)",
        usage.input_tokens, usage.output_tokens
    );
    Ok(())
}

async fn run(demo: bool, model_flag: Option<String>, yolo: bool) -> Result<()> {
    let gateway = if demo {
        None
    } else {
        Some(GatewayClient::from_env_with_model(model_flag.clone())?)
    };
    let model = gateway
        .as_ref()
        .map(|gateway| gateway.model().to_string())
        .or(model_flag)
        .or_else(|| std::env::var("CODELIGHT_MODEL").ok())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let mut app = App::new(project_name(), model_alias(&model));
    let (approvals_tx, mut approvals_rx) = mpsc::channel::<PendingApproval>(8);
    let mut agent: Option<Agent> = if let Some(gateway) = gateway {
        let skills = std::sync::Arc::new(SkillRegistry::load());
        let mut tools = ToolRegistry::new();
        tools.register(Box::new(ReadFile));
        tools.register(Box::new(WriteFile));
        tools.register(Box::new(EditFile));
        tools.register(Box::new(DeleteFile));
        tools.register(Box::new(MoveFile));
        tools.register(Box::new(ListDirectory));
        tools.register(Box::new(SearchInFiles));
        tools.register(Box::new(SearchDocs));
        tools.register(Box::new(RunCommand));
        tools.register(Box::new(WebFetch));
        tools.register(Box::new(LoadSkill::new(skills.clone())));
        tools.register(Box::new(ReadSkillResource::new(skills.clone())));
        tools.register(Box::new(SearchSkills));
        tools.register(Box::new(AddSkill));
        let approver: Arc<dyn Approver> = if yolo {
            Arc::new(YesApprover)
        } else {
            Arc::new(TuiApprover {
                tx: approvals_tx.clone(),
            })
        };
        let policy = PermissionPolicy::load(".codelight.toml");
        let mut agent = Agent::new(gateway, tools, approver, policy);
        agent.set_skills(&skills.advertise());
        Some(agent)
    } else {
        app.seed_demo();
        None
    };

    let (events_tx, mut events_rx) = mpsc::channel::<AgentEvent>(256);
    let (input_tx, mut input_rx) = mpsc::channel::<Event>(64);

    std::thread::spawn(move || {
        while let Ok(event) = event::read() {
            if input_tx.blocking_send(event).is_err() {
                break;
            }
        }
    });

    let mut terminal = ratatui::init();
    let mut running: Option<JoinHandle<Agent>> = None;
    let mut quit = false;
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(100));

    while !quit {
        terminal.draw(|frame| app.render(frame))?;

        tokio::select! {
            Some(event) = input_rx.recv() => {
                let Event::Key(key) = event else { continue };
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                if app.approval_is_open() {
                    match key.code {
                        KeyCode::Char('c') if ctrl => quit = true,
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            app.resolve_approval(Decision::AllowOnce)
                        }
                        KeyCode::Char('a') | KeyCode::Char('A')
                            if app.approval_offers_always() =>
                        {
                            app.resolve_approval(Decision::AllowAlways)
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            app.resolve_approval(Decision::Deny)
                        }
                        _ => {}
                    }
                    continue;
                }
                if app.picker_is_open() {
                    match key.code {
                        KeyCode::Char('c') if ctrl => quit = true,
                        KeyCode::Esc => app.close_picker(),
                        KeyCode::Up => app.picker_up(),
                        KeyCode::Down => app.picker_down(),
                        KeyCode::Backspace => app.picker_backspace(),
                        KeyCode::Enter => {
                            let chosen = app.picker_selected().map(str::to_string);
                            app.close_picker();
                            if let Some(model) = chosen
                                && let Some(active) = agent.as_mut()
                            {
                                active.set_model(&model);
                                app.set_model(model_alias(&model));
                                app.info(&format!("model set to {model}"));
                            }
                        }
                        KeyCode::Char(c) => app.picker_input(c),
                        _ => {}
                    }
                    continue;
                }
                match key.code {
                    KeyCode::Esc => quit = true,
                    KeyCode::Char('c') if ctrl => quit = true,
                    KeyCode::Enter => {
                        let text = app.input.value().trim().to_string();
                        if text == "/model" || text.starts_with("/model ") {
                            let arg = text.strip_prefix("/model").unwrap_or("").trim().to_string();
                            app.input.reset();
                            if !arg.is_empty() {
                                if let Some(active) = agent.as_mut() {
                                    active.set_model(&arg);
                                    app.set_model(model_alias(&arg));
                                    app.info(&format!("model set to {arg}"));
                                }
                            } else if let Some(active) = agent.as_ref() {
                                app.open_picker(&arg);
                                let gateway = active.gateway_client();
                                let tx = events_tx.clone();
                                tokio::spawn(async move {
                                    if let Ok(models) = gateway.list_models().await {
                                        tx.send(AgentEvent::ModelList(models)).await.ok();
                                    }
                                });
                            }
                        } else if !text.is_empty()
                            && let Some(mut ready) = agent.take()
                        {
                            app.input.reset();
                            app.push_user(&text);
                            let tx = events_tx.clone();
                            running = Some(tokio::spawn(async move {
                                ready.run(&text, tx).await.ok();
                                ready
                            }));
                        }
                    }
                    KeyCode::Up => app.scroll_up(),
                    KeyCode::Down => app.scroll_down(),
                    _ => {
                        if let Some(request) = to_request(key.code) {
                            app.input.handle(request);
                        }
                    }
                }
            }
            Some(event) = events_rx.recv() => {
                let done = matches!(event, AgentEvent::Done);
                app.apply(event);
                if done
                    && let Some(handle) = running.take()
                    && let Ok(reclaimed) = handle.await
                {
                    agent = Some(reclaimed);
                }
            }
            Some(pending) = approvals_rx.recv() => {
                app.open_approval(pending);
            }
            _ = ticker.tick() => {
                app.tick();
            }
        }
    }

    ratatui::restore();
    Ok(())
}

fn to_request(code: KeyCode) -> Option<InputRequest> {
    match code {
        KeyCode::Char(c) => Some(InputRequest::InsertChar(c)),
        KeyCode::Backspace => Some(InputRequest::DeletePrevChar),
        KeyCode::Delete => Some(InputRequest::DeleteNextChar),
        KeyCode::Left => Some(InputRequest::GoToPrevChar),
        KeyCode::Right => Some(InputRequest::GoToNextChar),
        KeyCode::Home => Some(InputRequest::GoToStart),
        KeyCode::End => Some(InputRequest::GoToEnd),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::model_alias;

    #[test]
    fn aliases_models() {
        assert_eq!(model_alias("anthropic/claude-sonnet-4-6"), "sonnet 4.6");
        assert_eq!(model_alias("anthropic/claude-haiku-4.5"), "haiku 4.5");
        assert_eq!(model_alias("anthropic/claude-opus-4-6"), "opus 4.6");
        assert_eq!(model_alias("openai/gpt-5"), "gpt-5");
    }
}
