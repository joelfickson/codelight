use futures::StreamExt;
use gateway::GatewayClient;
use types::Message;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let client = GatewayClient::from_env()?;
    let messages = vec![Message::user(
        "Say hello to a new Rust developer in exactly five words.",
    )];

    let mut stream = client.chat_stream(&messages, &[]).await?;

    while let Some(event) = stream.next().await {
        match event {
            types::StreamEvent::ResponseItems(_) => {}
            types::StreamEvent::Done { usage } => println!("Here is usage {:?} ", usage),
            types::StreamEvent::Error(error) => {
                if !error.is_empty() {
                    println!("Here is the error : {:?} ", error);
                }
            }
            types::StreamEvent::Token(tokens) => println!("Here is the error : {:?} ", tokens),
            types::StreamEvent::ToolCallStart { id, name } => {
                println!("ID of tool {:#?}", id);
                println!("name of tool {:#?}", name);
            }
            types::StreamEvent::ToolCallArgs { id, chunk } => {
                println!("args for {id}: {chunk}");
            }
            types::StreamEvent::ToolCallEnd { id } => {
                println!("tool {id} finished");
            }
        }
    }

    Ok(())
}
