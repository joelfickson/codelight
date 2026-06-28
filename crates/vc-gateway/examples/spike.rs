use vc_gateway::GatewayClient;
use vc_types::{Message, Role};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let client = GatewayClient::from_env()?;
    let messages = vec![Message {
        role: Role::User,
        content: "Say hello to a new Rust developer in exactly five words.".to_string(),
    }];

    let (text, usage) = client.chat(&messages).await?;
    println!("Reply: {text}");
    println!(
        "Tokens: {} in / {} out",
        usage.input_tokens, usage.output_tokens
    );
    Ok(())
}
