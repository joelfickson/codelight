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

pub struct WriteFile;

#[async_trait]
impl Tool for WriteFile {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write a UTF-8 text file at the given path, creating parent directories as needed. Overwrites the entire file if it already exists - read it first if you need to preserve any of its contents."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file to write" },
                "contents": { "type": "string", "description": "Full text to write to the file" }
            },
            "required": ["path", "contents"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'path'"))?;
        let contents = args["contents"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'contents'"))?;

        if let Some(parent) = std::path::Path::new(path).parent()
            && !parent.as_os_str().is_empty()
        {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, contents).await?;

        Ok(serde_json::json!({ "ok": true, "bytes_written": contents.len() }))
    }
}

pub struct ListDirectory;

#[async_trait]
impl Tool for ListDirectory {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List the entries of a directory, sorted, with a trailing slash on subdirectories. Use it to explore project structure before reading files."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory to list; defaults to the current directory" }
            },
            "required": []
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let path = args["path"].as_str().unwrap_or(".");

        let mut dir = tokio::fs::read_dir(path).await?;
        let mut entries = Vec::new();
        while let Some(entry) = dir.next_entry().await? {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = entry
                .file_type()
                .await
                .map(|kind| kind.is_dir())
                .unwrap_or(false);
            entries.push(if is_dir { format!("{name}/") } else { name });
        }
        entries.sort();

        Ok(serde_json::json!({ "entries": entries }))
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

    #[tokio::test]
    async fn writes_a_file_then_reads_it_back() {
        let path = std::env::temp_dir().join("vc_tools_write_test.txt");
        let path = path.to_string_lossy().into_owned();

        WriteFile
            .execute(serde_json::json!({ "path": path, "contents": "hello codelight" }))
            .await
            .unwrap();

        let read = ReadFile
            .execute(serde_json::json!({ "path": path }))
            .await
            .unwrap();
        assert_eq!(read["content"].as_str().unwrap(), "hello codelight");

        tokio::fs::remove_file(&path).await.ok();
    }

    #[tokio::test]
    async fn lists_the_crate_directory() {
        let result = ListDirectory
            .execute(serde_json::json!({ "path": "." }))
            .await
            .unwrap();
        let names: Vec<&str> = result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str())
            .collect();
        assert!(names.contains(&"Cargo.toml"));
        assert!(names.contains(&"src/"));
    }
}
