use anyhow::Ok;
use futures::{StreamExt, stream};
use vc_gateway::models::gateway::GatewayClient;
use vc_types::{Message, Role};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let client = GatewayClient::from_env()?;
    let messages = vec![Message {
        role: Role::User,
        content: "Say hello to a new Rust developer in exactly five words.".to_string(),
    }];

    let mut stream = client.chat_stream(&messages).await?;

    while let Some(event) = stream.next().await {
        match event {
            vc_types::StreamEvent::Done { usage } => println!("Here is usage {:?} ", usage),
            vc_types::StreamEvent::Error(error) => {
                if !error.is_empty() {
                    println!("Here is the error : {:?} ", error);
                }
            }
            vc_types::StreamEvent::Token(tokens) => println!("Here is the error : {:?} ", tokens),
            vc_types::StreamEvent::ToolCallStart { id, name } => {
                println!("ID of tool {:#?}", id);
                println!("name of tool {:#?}", name);
            }
            vc_types::StreamEvent::ToolCallArgs { id, chunk } => {
                println!("args for {id}: {chunk}");
            }
            vc_types::StreamEvent::ToolCallEnd { id } => {
                println!("tool {id} finished");
            }
        }
    }

    Ok(())
}
