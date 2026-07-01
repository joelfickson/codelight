use futures::StreamExt;
use std::io::Write;
use vc_gateway::GatewayClient;
use vc_types::{Message, StreamEvent};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let client = GatewayClient::from_env()?;
    let messages = vec![Message::user("Count from 1 to 5, one number per line.")];

    let mut stream = client.chat_stream(&messages, &[]).await?;

    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::Token(text) => {
                print!("{text}");
                std::io::stdout().flush().ok();
            }
            StreamEvent::Done { usage } => {
                println!(
                    "\n[done] {} in / {} out",
                    usage.input_tokens, usage.output_tokens
                );
            }
            StreamEvent::Error(message) => eprintln!("\n[error] {message}"),
            other => println!("\n[event] {other:?}"),
        }
    }
    Ok(())
}
