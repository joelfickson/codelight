mod app;

use anyhow::Result;
use clap::Parser;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tui_input::InputRequest;
use vc_agent::Agent;
use vc_gateway::GatewayClient;
use vc_tools::{ReadFile, ToolRegistry};
use vc_types::{AgentEvent, Message};

use app::App;

#[derive(Parser)]
#[command(name = "codelight", about = "A Vercel-specialized AI coding agent")]
struct Cli {
    #[arg(long, help = "Validate the Gateway connection and exit")]
    check: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    if cli.check {
        return check().await;
    }

    run().await
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

async fn run() -> Result<()> {
    let gateway = GatewayClient::from_env()?;
    let mut tools = ToolRegistry::new();
    tools.register(Box::new(ReadFile));
    let mut agent: Option<Agent> = Some(Agent::new(gateway, tools));

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
    let mut app = App::new();
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
