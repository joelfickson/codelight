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
