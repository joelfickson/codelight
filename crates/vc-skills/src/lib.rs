use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use vc_tools::Tool;

const BUILTIN: &[&str] = &[
    include_str!("skills/nextjs-app-router/SKILL.md"),
    include_str!("skills/vercel-preview-deploys/SKILL.md"),
    include_str!("skills/vercel-ai-sdk/SKILL.md"),
];

#[derive(Deserialize)]
struct Frontmatter {
    name: String,
    description: String,
}

pub struct Skill {
    pub name: String,
    pub description: String,
    body: String,
    dir: Option<PathBuf>,
}

fn parse_skill(content: &str) -> Option<(Frontmatter, String)> {
    let after_open = content.trim_start().strip_prefix("---")?;
    let end = after_open.find("\n---")?;
    let front_block = after_open[..end].trim();
    let body = after_open[end + 4..]
        .trim_start_matches(['\n', '\r'])
        .to_string();
    let front: Frontmatter = serde_yaml::from_str(front_block).ok()?;
    Some((front, body))
}

impl Skill {
    fn parse(content: &str, dir: Option<PathBuf>) -> Option<Self> {
        let (front, body) = parse_skill(content)?;
        Some(Skill {
            name: front.name,
            description: front.description,
            body,
            dir,
        })
    }
}

#[derive(Default)]
pub struct SkillRegistry {
    skills: Vec<Skill>,
}

impl SkillRegistry {
    pub fn load() -> Self {
        let mut registry = SkillRegistry::default();
        for content in BUILTIN {
            if let Some(skill) = Skill::parse(content, None) {
                registry.insert(skill);
            }
        }
        for dir in skill_dirs() {
            registry.load_dir(&dir);
        }
        registry
    }

    fn insert(&mut self, skill: Skill) {
        if let Some(existing) = self.skills.iter_mut().find(|s| s.name == skill.name) {
            *existing = skill;
        } else {
            self.skills.push(skill);
        }
    }

    fn load_dir(&mut self, dir: &Path) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let manifest = path.join("SKILL.md");
            if manifest.is_file()
                && let Ok(content) = std::fs::read_to_string(&manifest)
                && let Some(skill) = Skill::parse(&content, Some(path))
            {
                self.insert(skill);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.name == name)
    }

    pub fn advertise(&self) -> String {
        if self.skills.is_empty() {
            return String::new();
        }
        let mut out = String::from(
            "## Available skills\n\nWhen a task matches one of these, call load_skill(name) to load its full instructions before doing the work.\n\n",
        );
        for skill in &self.skills {
            out.push_str(&format!("- {}: {}\n", skill.name, skill.description));
        }
        out
    }
}

fn skill_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = directories::UserDirs::new().map(|u| u.home_dir().to_path_buf()) {
        dirs.push(home.join(".vercelcode/skills"));
        dirs.push(home.join(".claude/skills"));
    }
    dirs.push(PathBuf::from(".vercelcode/skills"));
    dirs.push(PathBuf::from(".claude/skills"));
    dirs
}

pub struct LoadSkill {
    registry: Arc<SkillRegistry>,
}

impl LoadSkill {
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for LoadSkill {
    fn name(&self) -> &str {
        "load_skill"
    }

    fn description(&self) -> &str {
        "Load the full instructions of an available skill by name (see the Available skills list). Call this before doing non-trivial work a skill covers, then follow its guidance."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": { "name": { "type": "string", "description": "The skill name to load" } },
            "required": ["name"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let name = args["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'name'"))?;
        match self.registry.get(name) {
            Some(skill) => Ok(serde_json::json!({
                "name": skill.name,
                "instructions": skill.body,
            })),
            None => Err(anyhow::anyhow!("unknown skill: {name}")),
        }
    }
}

pub struct ReadSkillResource {
    registry: Arc<SkillRegistry>,
}

impl ReadSkillResource {
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for ReadSkillResource {
    fn name(&self) -> &str {
        "read_skill_resource"
    }

    fn description(&self) -> &str {
        "Read a file bundled with a skill (from its scripts/, references/, or assets/ folder) by skill name and relative path. Use it when a loaded skill points you to one of its files."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "The skill name" },
                "path": { "type": "string", "description": "Relative path within the skill directory" }
            },
            "required": ["name", "path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let name = args["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'name'"))?;
        let rel = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'path'"))?;

        let skill = self
            .registry
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("unknown skill: {name}"))?;
        let dir = skill
            .dir
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("skill '{name}' has no resource files"))?;

        if rel.contains("..") || Path::new(rel).is_absolute() {
            return Err(anyhow::anyhow!("invalid resource path"));
        }

        let content = tokio::fs::read_to_string(dir.join(rel)).await?;
        Ok(serde_json::json!({ "path": rel, "content": content }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_body() {
        let (front, body) =
            parse_skill("---\nname: demo\ndescription: A demo skill.\n---\nThe body.\nMore body.")
                .unwrap();
        assert_eq!(front.name, "demo");
        assert_eq!(front.description, "A demo skill.");
        assert!(body.starts_with("The body."));
    }

    #[test]
    fn loads_builtin_skills_and_advertises() {
        let registry = SkillRegistry::load();
        assert!(registry.get("nextjs-app-router").is_some());
        let ad = registry.advertise();
        assert!(ad.contains("Available skills"));
        assert!(ad.contains("nextjs-app-router"));
    }

    #[tokio::test]
    async fn load_skill_returns_instructions() {
        let registry = Arc::new(SkillRegistry::load());
        let tool = LoadSkill::new(registry);
        let out = tool
            .execute(serde_json::json!({ "name": "nextjs-app-router" }))
            .await
            .unwrap();
        assert!(out["instructions"].as_str().unwrap().len() > 20);
        assert!(
            tool.execute(serde_json::json!({ "name": "nope" }))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn reads_disk_resource_and_blocks_traversal() {
        let base = std::env::temp_dir().join(format!("vcskills_{}", std::process::id()));
        let skill_dir = base.join("my-skill");
        std::fs::create_dir_all(skill_dir.join("references")).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: A test skill.\n---\nBody.",
        )
        .unwrap();
        std::fs::write(skill_dir.join("references/guide.md"), "REF CONTENT").unwrap();

        let mut registry = SkillRegistry::default();
        registry.load_dir(&base);
        assert!(registry.get("my-skill").is_some());

        let tool = ReadSkillResource::new(Arc::new(registry));
        let ok = tool
            .execute(serde_json::json!({ "name": "my-skill", "path": "references/guide.md" }))
            .await
            .unwrap();
        assert_eq!(ok["content"], "REF CONTENT");
        assert!(
            tool.execute(serde_json::json!({ "name": "my-skill", "path": "../secret" }))
                .await
                .is_err()
        );

        std::fs::remove_dir_all(&base).ok();
    }
}
