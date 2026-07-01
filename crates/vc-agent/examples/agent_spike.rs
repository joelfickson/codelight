use std::io::Write;
use tokio::sync::mpsc;
use vc_agent::Agent;
use vc_gateway::GatewayClient;
use vc_tools::{ListDirectory, ReadFile, ToolRegistry, WriteFile};
use vc_types::AgentEvent;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let gateway = GatewayClient::from_env()?;

    let mut tools = ToolRegistry::new();
    tools.register(Box::new(ReadFile));
    tools.register(Box::new(WriteFile));
    tools.register(Box::new(ListDirectory));

    let mut agent = Agent::new(gateway, tools);

    let target = std::env::temp_dir().join("codelight_hello.txt");
    let target = target.to_string_lossy().into_owned();
    std::fs::remove_file(&target).ok();

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

    let prompt = format!(
        "Write a file at {target} containing exactly the line 'hello from codelight', then tell me you are done."
    );
    agent.run(&prompt, tx).await?;

    printer.await?;

    match std::fs::read_to_string(&target) {
        Ok(contents) => println!("\n[verify] file exists, contents: {contents:?}"),
        Err(err) => println!("\n[verify] file missing: {err}"),
    }
    Ok(())
}
