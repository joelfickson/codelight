use anyhow::{Ok, Result};
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;

    async fn execute(&self, args: Value) -> Result<Value>;

    fn to_api_definitions(&self) -> Value {
        serde_json::json!({
        "type":"function",
        "function":{
            "name":self.name(),
            "description": self.description(),
            "parameters": self.parameters_schema(),}
        })
    }
}

#[async_trait]
impl Tool for ReadFile {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the full contents of a file at the given path"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type":"object",
            "properties":{
                "path": {
                    "type":"string",
                    "description": "Path to the file to read"
                },
                "required":["path"]
            }
        })
    }

    async fn execute(&self, args: Value)-> Result<Value>{
        let path = args["path"].as_str().ok_or_else(|| anyhow::anyhow!("missing 'path'"))?;

        let content = tokio::fs::read_to_string(path).await?;

        Ok(serde_json::json!({"content": content}))

    }
}
