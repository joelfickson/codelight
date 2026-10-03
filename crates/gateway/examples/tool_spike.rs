use futures::StreamExt;
use gateway::GatewayClient;
use types::{Message, StreamEvent};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let client = GatewayClient::from_env()?;

    let messages = vec![Message::user(
        "What is the weather in Tokyo? Use the get_weather tool.",
    )];

    let tools = vec![serde_json::json!({
        "type": "function",
        "function": {
            "name": "get_weather",
            "description": "Get the current weather for a city",
            "parameters": {
                "type": "object",
                "properties": {
                    "city": { "type": "string", "description": "City name" }
                },
                "required": ["city"]
            }
        }
    })];

    let mut stream = client.chat_stream(&messages, &tools).await?;

    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::ToolCallStart { id, name } => println!("[start] {name} (id={id})"),
            StreamEvent::ToolCallArgs { id, chunk } => println!("[args]  {id}: {chunk}"),
            StreamEvent::ToolCallEnd { id } => println!("[end]   {id}"),
            StreamEvent::Token(text) => print!("{text}"),
            StreamEvent::Done { usage } => {
                println!(
                    "\n[done] {} in / {} out",
                    usage.input_tokens, usage.output_tokens
                )
            }
            StreamEvent::Error(message) => eprintln!("\n[error] {message}"),
        }
    }
    Ok(())
}
