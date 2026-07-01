use std::io::Write;
use tokio::sync::mpsc;
use vc_agent::Agent;
use vc_gateway::GatewayClient;
use vc_skills::{LoadSkill, ReadSkillResource, SkillRegistry};
use vc_tools::{
    DeleteFile, EditFile, ListDirectory, MoveFile, ReadFile, RunCommand, SearchDocs, SearchInFiles,
    ToolRegistry, WebFetch, WriteFile,
};
use vc_types::AgentEvent;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let gateway = GatewayClient::from_env()?;

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

    let skills = std::sync::Arc::new(SkillRegistry::load());
    tools.register(Box::new(LoadSkill::new(skills.clone())));
    tools.register(Box::new(ReadSkillResource::new(skills.clone())));

    let mut agent = Agent::new(gateway, tools);
    agent.set_skills(&skills.advertise());

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
            "Consult your nextjs-app-router skill, then tell me where a route-level loading state goes and how it streams.",
            tx,
        )
        .await?;

    printer.await?;
    Ok(())
}
