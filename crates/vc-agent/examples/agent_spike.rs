use std::io::Write;
use tokio::sync::mpsc;
use vc_agent::Agent;
use vc_gateway::GatewayClient;
use vc_tools::{ReadFile, ToolRegistry};
use vc_types::AgentEvent;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let gateway = GatewayClient::from_env()?;

    let mut tools = ToolRegistry::new();
    tools.register(Box::new(ReadFile));

    let mut agent = Agent::new(gateway, tools);

    let (tx, mut rx) = mpsc::channel::<AgentEvent>(64);

    let printer = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                AgentEvent::Token(text) => {
                    print!("{text}");
                    std::io::stdout().flush().ok();
                }
                AgentEvent::ToolStarted { id, label } => {
                    println!("\n[tool →] {label} (id={id})");
                }
                AgentEvent::ToolFinished { id, ok } => {
                    let mark = if ok { "✓" } else { "✗" };
                    println!("[tool {mark}] id={id}");
                }
                AgentEvent::Done => println!("\n[agent done]"),
                other => println!("\n[event] {other:?}"),
            }
        }
    });

    agent
        .run(
            "Read the file Cargo.toml at the repository root and tell me which resolver version the workspace uses.",
            tx,
        )
        .await?;

    printer.await?;
    Ok(())
}
