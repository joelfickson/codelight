use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use vc_tools::Tool;

const BUILTIN: &[&str] = &[
    include_str!("skills/explore-codebase/SKILL.md"),
    include_str!("skills/debug-and-test/SKILL.md"),
    include_str!("skills/verify-changes/SKILL.md"),
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
        dirs.push(home.join(".codelight/skills"));
        dirs.push(home.join(".claude/skills"));
    }
    dirs.push(PathBuf::from(".codelight/skills"));
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

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn is_skill_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

pub struct SearchSkills;

#[async_trait]
impl Tool for SearchSkills {
    fn name(&self) -> &str {
        "search_skills"
    }

    fn description(&self) -> &str {
        "Search the skills.sh registry for installable agent skills by keyword (runs `npx skills find`). Use it to discover skills for the project’s language and tools that you can then install with add_skill."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Keywords to search for" },
                "owner": { "type": "string", "description": "Optional GitHub owner to restrict to, e.g. owner" }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let query = args["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'query'"))?;

        let mut command = tokio::process::Command::new("npx");
        command
            .arg("--yes")
            .arg("skills@latest")
            .arg("find")
            .arg(query);
        if let Some(owner) = args["owner"].as_str() {
            command.arg("--owner").arg(owner);
        }

        let output = tokio::time::timeout(Duration::from_secs(60), command.output())
            .await
            .map_err(|_| anyhow::anyhow!("skills find timed out"))??;

        let stdout = strip_ansi(&String::from_utf8_lossy(&output.stdout));
        let results = if stdout.trim().is_empty() {
            strip_ansi(&String::from_utf8_lossy(&output.stderr))
        } else {
            stdout
        };
        Ok(serde_json::json!({ "results": results.trim() }))
    }
}

pub struct AddSkill;

#[async_trait]
impl Tool for AddSkill {
    fn name(&self) -> &str {
        "add_skill"
    }

    fn description(&self) -> &str {
        "Install a skill from the skills.sh registry into this project's .claude/skills directory (runs `npx skills add`), then return its instructions so you can use it immediately. Provide the source repo (e.g. owner/agent-skills) and the skill name. Installed skills persist and load automatically in future sessions."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": "The skill source repo, e.g. owner/agent-skills" },
                "skill": { "type": "string", "description": "The skill name to install, e.g. frontend-design" }
            },
            "required": ["source", "skill"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'source'"))?;
        let skill = args["skill"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'skill'"))?;
        if !is_skill_name(skill) {
            return Err(anyhow::anyhow!("invalid skill name"));
        }

        let output = tokio::time::timeout(
            Duration::from_secs(120),
            tokio::process::Command::new("npx")
                .arg("--yes")
                .arg("skills@latest")
                .arg("add")
                .arg(source)
                .arg("-a")
                .arg("claude-code")
                .arg("-s")
                .arg(skill)
                .arg("-y")
                .arg("--copy")
                .output(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("skills add timed out"))??;

        if !output.status.success() {
            return Ok(serde_json::json!({
                "ok": false,
                "error": strip_ansi(&String::from_utf8_lossy(&output.stderr)),
            }));
        }

        let manifest = Path::new(".claude/skills").join(skill).join("SKILL.md");
        match tokio::fs::read_to_string(&manifest).await {
            Ok(content) => Ok(serde_json::json!({
                "ok": true,
                "installed_at": manifest.to_string_lossy(),
                "instructions": content,
            })),
            Err(_) => Ok(serde_json::json!({
                "ok": true,
                "note": format!("installed '{skill}' from {source}; it will load next session"),
            })),
        }
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
    fn strips_ansi_escape_codes() {
        assert_eq!(strip_ansi("\u{1b}[36mhi\u{1b}[0m there"), "hi there");
        assert!(is_skill_name("frontend-design"));
        assert!(!is_skill_name("../evil"));
    }

    #[tokio::test]
    async fn add_skill_rejects_bad_skill_name() {
        let result = AddSkill
            .execute(serde_json::json!({ "source": "x/y", "skill": "../evil" }))
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn loads_builtin_skills_and_advertises() {
        let registry = SkillRegistry::load();
        assert!(registry.get("explore-codebase").is_some());
        let ad = registry.advertise();
        assert!(ad.contains("Available skills"));
        assert!(ad.contains("explore-codebase"));
    }

    #[tokio::test]
    async fn load_skill_returns_instructions() {
        let registry = Arc::new(SkillRegistry::load());
        let tool = LoadSkill::new(registry);
        let out = tool
            .execute(serde_json::json!({ "name": "explore-codebase" }))
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
