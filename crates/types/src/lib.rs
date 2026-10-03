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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub response_items: Vec<serde_json::Value>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            tool_calls: Vec::new(),
            response_items: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_calls: Vec::new(),
            response_items: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_calls: Vec::new(),
            response_items: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant_tool_calls(tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content: String::new(),
            tool_calls,
            response_items: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            response_items: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(into = "WireToolCall", from = "WireToolCall")]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Serialize, Deserialize)]
struct WireToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    function: WireFunction,
}

#[derive(Serialize, Deserialize)]
struct WireFunction {
    name: String,
    arguments: String,
}

impl From<ToolCall> for WireToolCall {
    fn from(call: ToolCall) -> Self {
        WireToolCall {
            id: call.id,
            kind: "function".to_string(),
            function: WireFunction {
                name: call.name,
                arguments: call.arguments,
            },
        }
    }
}

impl From<WireToolCall> for ToolCall {
    fn from(wire: WireToolCall) -> Self {
        ToolCall {
            id: wire.id,
            name: wire.function.name,
            arguments: wire.function.arguments,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Token(String),
    ResponseItems(Vec<serde_json::Value>),
    ToolCallStart { id: String, name: String },
    ToolCallArgs { id: String, chunk: String },
    ToolCallEnd { id: String },
    Done { usage: Usage },
    Error(String),
}

#[derive(Debug, Clone)]
pub enum AgentEvent {
    ModelSelected(String),
    ModelList(Vec<String>),
    Token(String),
    ToolStarted { id: String, label: String },
    ToolFinished { id: String, ok: bool },
    Info(String),
    Error(String),
    StepComplete,
    Done,
}

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub tool: String,
    pub action: String,
    pub suggested_pattern: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    AllowOnce,
    AllowAlways,
    Deny,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_round_trips_through_json() {
        let msg = Message::user("hi");
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"role":"user","content":"hi"}"#);

        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(back.role, Role::User);
    }

    #[test]
    fn tool_call_serializes_to_openai_wire_shape() {
        let call = ToolCall {
            id: "call_1".to_string(),
            name: "read_file".to_string(),
            arguments: r#"{"path":"a"}"#.to_string(),
        };

        let json = serde_json::to_value(&call).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "id": "call_1",
                "type": "function",
                "function": { "name": "read_file", "arguments": r#"{"path":"a"}"# }
            })
        );

        let back: ToolCall = serde_json::from_value(json).unwrap();
        assert_eq!(back.name, "read_file");
    }

    #[test]
    fn assistant_tool_call_message_omits_tool_call_id() {
        let msg = Message::assistant_tool_calls(vec![ToolCall {
            id: "call_1".to_string(),
            name: "read_file".to_string(),
            arguments: "{}".to_string(),
        }]);

        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["role"], "assistant");
        assert_eq!(json["tool_calls"][0]["type"], "function");
        assert_eq!(json["tool_calls"][0]["function"]["name"], "read_file");
        assert!(json.get("tool_call_id").is_none());
    }
}
