use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use types::{Message, Role};

pub const DEFAULT_CONTEXT_BYTES: usize = 256 * 1024;

#[derive(Serialize, Deserialize)]
struct Transcript {
    version: u32,
    workspace: PathBuf,
    messages: Vec<Message>,
}

pub struct SessionStore {
    path: PathBuf,
    workspace: PathBuf,
    _lock: File,
}

impl SessionStore {
    pub fn open(path: &Path, workspace: &Path, resume: bool) -> Result<(Self, Vec<Message>)> {
        let workspace = workspace.canonicalize()?;
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        let path = parent
            .canonicalize()?
            .join(path.file_name().context("session path needs a file name")?);
        let lock_path = path.with_file_name(format!(
            "{}.lock",
            path.file_name().unwrap().to_string_lossy()
        ));
        let lock = private_options()
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        lock.try_lock()
            .context("session is already open in another process")?;
        let store = Self {
            path,
            workspace,
            _lock: lock,
        };
        if resume {
            let transcript: Transcript = serde_json::from_slice(&std::fs::read(&store.path)?)
                .context("invalid session file")?;
            ensure!(transcript.version == 1, "unsupported session version");
            ensure!(
                transcript.workspace == store.workspace,
                "session belongs to a different workspace"
            );
            let mut messages = transcript.messages;
            repair_interrupted_calls(&mut messages)?;
            Ok((store, messages))
        } else {
            ensure!(!store.path.exists(), "session already exists; use --resume");
            Ok((store, Vec::new()))
        }
    }

    pub fn save(&self, messages: &[Message]) -> Result<()> {
        let transcript = Transcript {
            version: 1,
            workspace: self.workspace.clone(),
            messages: messages.to_vec(),
        };
        let bytes = serde_json::to_vec(&transcript)?;
        let temporary = self
            .path
            .with_file_name(format!(".codelight-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> Result<()> {
            let mut file = private_options().create_new(true).open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            std::fs::rename(&temporary, &self.path)?;
            #[cfg(unix)]
            File::open(self.path.parent().unwrap())?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            std::fs::remove_file(&temporary).ok();
        }
        result.context("could not checkpoint session")
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn private_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

pub fn repair_interrupted_calls(messages: &mut Vec<Message>) -> Result<()> {
    ensure!(
        messages
            .first()
            .is_some_and(|message| message.role == Role::System),
        "session must begin with system instructions"
    );
    let mut pending = Vec::new();
    for message in messages.iter() {
        if message.role == Role::Tool {
            let id = message
                .tool_call_id
                .as_ref()
                .context("tool result has no call id")?;
            let index = pending
                .iter()
                .position(|pending_id| pending_id == id)
                .context("tool result has no matching call")?;
            pending.remove(index);
        } else {
            ensure!(
                pending.is_empty(),
                "session contains an unfinished tool batch before another message"
            );
            ensure!(
                message.tool_calls.is_empty() || message.role == Role::Assistant,
                "only assistant messages can call tools"
            );
            let mut unique = HashSet::new();
            for call in &message.tool_calls {
                ensure!(unique.insert(&call.id), "duplicate tool call id");
                pending.push(call.id.clone());
            }
        }
    }
    for id in pending {
        messages.push(Message::tool_result(id, serde_json::json!({
            "error": "Session interrupted before a result was checkpointed. This action may already have executed. Inspect current state before deciding what to do; do not blindly repeat it."
        }).to_string()));
    }
    Ok(())
}

pub fn bounded_context(
    history: &[Message],
    tools: &[serde_json::Value],
    max_bytes: usize,
) -> Result<(Vec<Message>, usize)> {
    ensure!(
        !history.is_empty(),
        "conversation has no system instructions"
    );
    let tools_bytes = serde_json::to_vec(tools)?.len();
    let mut start = 1;
    loop {
        let mut messages = vec![history[0].clone()];
        if start > 1 {
            messages.push(Message::system("Earlier completed turns were omitted to fit the context budget. Do not assume their contents; inspect project files when needed. The full transcript is retained in the session file when session saving is enabled."));
        }
        messages.extend_from_slice(&history[start..]);
        if serde_json::to_vec(&messages)?
            .len()
            .saturating_add(tools_bytes)
            <= max_bytes
        {
            return Ok((messages, start - 1));
        }
        let next = history
            .iter()
            .enumerate()
            .skip(start + 1)
            .find(|(_, message)| message.role == Role::User)
            .map(|(index, _)| index);
        match next {
            Some(next) => start = next,
            None => bail!(
                "current turn and tool definitions exceed the {max_bytes}-byte context budget; start a new session or increase --context-bytes"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::ToolCall;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("vc-session-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn session(&self) -> PathBuf {
            self.0.join("session.json")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn call(id: &str) -> ToolCall {
        ToolCall {
            id: id.into(),
            name: "write_file".into(),
            arguments: "{}".into(),
        }
    }

    #[test]
    fn checkpoint_round_trip_and_exclusive_lock() {
        let fixture = Fixture::new();
        let (store, _) = SessionStore::open(&fixture.session(), &fixture.0, false).unwrap();
        let messages = vec![
            Message::system("instructions"),
            Message::user("fix this"),
            Message::assistant("done"),
        ];
        store.save(&messages).unwrap();
        assert!(SessionStore::open(&fixture.session(), &fixture.0, true).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(fixture.session())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        drop(store);
        assert!(SessionStore::open(&fixture.session(), &fixture.0, false).is_err());
        let (_store, restored) = SessionStore::open(&fixture.session(), &fixture.0, true).unwrap();
        assert_eq!(
            serde_json::to_value(restored).unwrap(),
            serde_json::to_value(messages).unwrap()
        );
    }

    #[test]
    fn resume_rejects_wrong_workspace_corrupt_and_future_sessions() {
        let fixture = Fixture::new();
        let other = Fixture::new();
        let (store, _) = SessionStore::open(&fixture.session(), &fixture.0, false).unwrap();
        store.save(&[Message::system("instructions")]).unwrap();
        drop(store);
        assert!(SessionStore::open(&fixture.session(), &other.0, true).is_err());
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(fixture.session()).unwrap()).unwrap();
        value["version"] = 2.into();
        std::fs::write(fixture.session(), serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(SessionStore::open(&fixture.session(), &fixture.0, true).is_err());
        std::fs::write(fixture.session(), "broken").unwrap();
        assert!(SessionStore::open(&fixture.session(), &fixture.0, true).is_err());
    }

    #[test]
    fn unfinished_batch_gets_results_only_for_uncheckpointed_calls() {
        let fixture = Fixture::new();
        let (store, _) = SessionStore::open(&fixture.session(), &fixture.0, false).unwrap();
        store
            .save(&[
                Message::system("instructions"),
                Message::user("edit"),
                Message::assistant_tool_calls(vec![call("first"), call("second")]),
                Message::tool_result("first", "success"),
            ])
            .unwrap();
        drop(store);
        let (store, restored) = SessionStore::open(&fixture.session(), &fixture.0, true).unwrap();
        assert_eq!(restored.len(), 5);
        assert_eq!(restored[3].content, "success");
        assert_eq!(restored[4].tool_call_id.as_deref(), Some("second"));
        assert!(restored[4].content.contains("may already have executed"));
        store.save(&restored).unwrap();
        drop(store);
        let (_, again) = SessionStore::open(&fixture.session(), &fixture.0, true).unwrap();
        assert_eq!(again.len(), 5);
    }

    #[test]
    fn invalid_tool_pairing_is_rejected() {
        let mut messages = vec![
            Message::system("instructions"),
            Message::tool_result("missing", "ok"),
        ];
        assert!(repair_interrupted_calls(&mut messages).is_err());
        let mut messages = vec![
            Message::system("instructions"),
            Message::assistant_tool_calls(vec![call("a")]),
            Message::user("next"),
        ];
        assert!(repair_interrupted_calls(&mut messages).is_err());
    }

    #[test]
    fn pruning_preserves_current_tool_batch_and_full_history() {
        let history = vec![
            Message::system("instructions"),
            Message::user("old"),
            Message::assistant("x".repeat(4000)),
            Message::user("current"),
            Message::assistant_tool_calls(vec![call("a"), call("b")]),
            Message::tool_result("a", "one"),
            Message::tool_result("b", "two"),
        ];
        let (context, omitted) = bounded_context(&history, &[], 2000).unwrap();
        assert_eq!(omitted, 2);
        assert_eq!(context[0].content, "instructions");
        assert_eq!(context[2].content, "current");
        assert_eq!(context[3].tool_calls.len(), 2);
        assert_eq!(context[4].tool_call_id.as_deref(), Some("a"));
        assert_eq!(context[5].tool_call_id.as_deref(), Some("b"));
        assert_eq!(history[2].content.len(), 4000);
    }

    #[test]
    fn oversized_current_turn_and_tools_fail_explicitly() {
        let history = vec![
            Message::system("instructions"),
            Message::user("x".repeat(4000)),
        ];
        assert!(bounded_context(&history, &[], 2000).is_err());
        let history = vec![Message::system("instructions"), Message::user("short")];
        assert!(
            bounded_context(
                &history,
                &[serde_json::json!({"description": "x".repeat(4000)})],
                2000
            )
            .is_err()
        );
    }
}
