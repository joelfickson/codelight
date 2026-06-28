use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Token(String),
    ToolCallStart { id: String, name: String },
    ToolCallArgs { id: String, chunk: String },
    ToolCallEnd { id: String },
    Done { usage: Usage },
    Error(String),
}

#[derive(Debug, Clone)]
pub enum AgentEvent {
    ModelSelected(String),
    Token(String),
    ToolStarted { id: String, label: String },
    ToolFinished { id: String, ok: bool },
    Info(String),
    Error(String),
    StepComplete,
    Done,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_round_trips_through_json() {
        let msg = Message {
            role: Role::User,
            content: "hi".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"role":"user","content":"hi"}"#);

        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(back.role, Role::User);
    }
}
