use anyhow::Result;
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

pub struct ReadFile;

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
                }

            },
            "required":["path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'path'"))?;

        let content = tokio::fs::read_to_string(path).await?;

        Ok(serde_json::json!({"content": content}))
    }
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .find(|tool| tool.name() == name)
            .map(|tool| tool.as_ref())
    }

    pub fn definitions(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|tool| tool.to_api_definitions())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reads_an_existing_file() {
        let result = ReadFile
            .execute(serde_json::json!({ "path": "Cargo.toml" }))
            .await
            .unwrap();
        let content = result["content"].as_str().unwrap();
        assert!(content.contains("vc-tools"));
    }

    #[tokio::test]
    async fn errors_when_path_is_missing() {
        let result = ReadFile.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }
}
