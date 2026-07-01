mod app;

use anyhow::Result;
use clap::Parser;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tui_input::InputRequest;
use vc_agent::Agent;
use vc_gateway::{DEFAULT_MODEL, GatewayClient};
use vc_tools::{ListDirectory, ReadFile, RunCommand, SearchInFiles, ToolRegistry, WriteFile};
use vc_types::{AgentEvent, Message};

use app::App;

#[derive(Parser)]
#[command(name = "codelight", about = "A Vercel-specialized AI coding agent")]
struct Cli {
    #[arg(long, help = "Validate the Gateway connection and exit")]
    check: bool,
    #[arg(
        long,
        help = "Seed a sample session to preview the UI without the Gateway"
    )]
    demo: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    if cli.check {
        return check().await;
    }

    run(cli.demo).await
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

fn model_label() -> String {
    DEFAULT_MODEL
        .split('/')
        .next_back()
        .unwrap_or(DEFAULT_MODEL)
        .to_string()
}

async fn check() -> Result<()> {
    let gateway = GatewayClient::from_env()?;
    let (_reply, usage) = gateway
        .chat(&[Message::user("Reply with the single word: ok")])
        .await?;
    println!(
        "gateway: ok ({} in / {} out tokens)",
        usage.input_tokens, usage.output_tokens
    );
    Ok(())
}

async fn run(demo: bool) -> Result<()> {
    let mut app = App::new(project_name(), model_label());
    let mut agent: Option<Agent> = if demo {
        app.seed_demo();
        None
    } else {
        let gateway = GatewayClient::from_env()?;
        let mut tools = ToolRegistry::new();
        tools.register(Box::new(ReadFile));
        tools.register(Box::new(WriteFile));
        tools.register(Box::new(ListDirectory));
        tools.register(Box::new(SearchInFiles));
        tools.register(Box::new(RunCommand));
        Some(Agent::new(gateway, tools))
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

    while !quit {
        terminal.draw(|frame| app.render(frame))?;

        tokio::select! {
            Some(event) = input_rx.recv() => {
                let Event::Key(key) = event else { continue };
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                match key.code {
                    KeyCode::Esc => quit = true,
                    KeyCode::Char('c') if ctrl => quit = true,
                    KeyCode::Enter => {
                        let text = app.input.value().trim().to_string();
                        if !text.is_empty()
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
