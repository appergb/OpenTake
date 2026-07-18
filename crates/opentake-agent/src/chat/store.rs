//! Project-local chat-session persistence (`agent-SPEC.md` §5.8).

use std::path::Path;

use opentake_project::{ProjectError, ProjectRoot};

use crate::chat::ChatSession;

const MAX_SESSION_BYTES: usize = 8 * 1024 * 1024;
const MAX_SESSION_FILES: usize = 256;
const MAX_AGGREGATE_SESSION_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ChatSessionStoreError {
    #[error("invalid chat session id: {0}")]
    InvalidSessionId(String),
    #[error("chat session `{requested}` contains mismatched id `{stored}`")]
    MismatchedSessionId { requested: String, stored: String },
    #[error("chat session exceeds the {MAX_SESSION_BYTES}-byte limit")]
    TooLarge,
    #[error("project exceeds the {MAX_SESSION_FILES}-session limit")]
    TooManySessions,
    #[error("project chat history exceeds the {MAX_AGGREGATE_SESSION_BYTES}-byte limit")]
    AggregateTooLarge,
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Retained no-follow authority for one project's `chat-sessions/` directory.
/// Every save uses a sibling temp file + atomic rename inside that authority.
pub struct ChatSessionStore {
    root: ProjectRoot,
}

impl ChatSessionStore {
    pub fn open(project_dir: impl AsRef<Path>) -> Result<Self, ChatSessionStoreError> {
        Ok(Self {
            root: ProjectRoot::open(project_dir)?,
        })
    }

    pub fn root(&self) -> &ProjectRoot {
        &self.root
    }

    pub fn load(&self, session_id: &str) -> Result<Option<ChatSession>, ChatSessionStoreError> {
        Ok(self
            .load_with_size(session_id)?
            .map(|(session, _byte_len)| session))
    }

    fn load_with_size(
        &self,
        session_id: &str,
    ) -> Result<Option<(ChatSession, usize)>, ChatSessionStoreError> {
        let file_name = session_file_name(session_id)?;
        let Some(bytes) = self.root.read_chat_session(&file_name, MAX_SESSION_BYTES)? else {
            return Ok(None);
        };
        let session: ChatSession = serde_json::from_slice(&bytes)?;
        if session.id != session_id {
            return Err(ChatSessionStoreError::MismatchedSessionId {
                requested: session_id.to_string(),
                stored: session.id,
            });
        }
        let byte_len = bytes.len();
        Ok(Some((session, byte_len)))
    }

    pub fn save(&self, session: &ChatSession) -> Result<(), ChatSessionStoreError> {
        self.save_with_limits(session, MAX_SESSION_FILES, MAX_AGGREGATE_SESSION_BYTES)
    }

    fn save_with_limits(
        &self,
        session: &ChatSession,
        max_files: usize,
        max_aggregate_bytes: usize,
    ) -> Result<(), ChatSessionStoreError> {
        let file_name = session_file_name(&session.id)?;
        let bytes = serde_json::to_vec_pretty(session)?;
        if bytes.len() > MAX_SESSION_BYTES {
            return Err(ChatSessionStoreError::TooLarge);
        }
        if bytes.len() > max_aggregate_bytes {
            return Err(ChatSessionStoreError::AggregateTooLarge);
        }
        let files = self.session_files(max_files)?;
        let target_exists = files
            .iter()
            .any(|existing| existing.to_string_lossy() == file_name);
        if !target_exists && files.len() >= max_files {
            return Err(ChatSessionStoreError::TooManySessions);
        }
        let mut aggregate_bytes = bytes.len();
        for existing in files {
            let existing = existing
                .into_string()
                .map_err(|_| ChatSessionStoreError::InvalidSessionId("non-UTF-8 leaf".into()))?;
            if existing == file_name || !existing.ends_with(".json") {
                continue;
            }
            let Some(existing_bytes) = self.root.read_chat_session(&existing, MAX_SESSION_BYTES)?
            else {
                continue;
            };
            aggregate_bytes = aggregate_bytes
                .checked_add(existing_bytes.len())
                .ok_or(ChatSessionStoreError::AggregateTooLarge)?;
            if aggregate_bytes > max_aggregate_bytes {
                return Err(ChatSessionStoreError::AggregateTooLarge);
            }
        }
        self.root.write_chat_session_atomic(&file_name, &bytes)?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<ChatSession>, ChatSessionStoreError> {
        self.list_with_limits(MAX_SESSION_FILES, MAX_AGGREGATE_SESSION_BYTES)
    }

    fn list_with_limits(
        &self,
        max_files: usize,
        max_aggregate_bytes: usize,
    ) -> Result<Vec<ChatSession>, ChatSessionStoreError> {
        let mut sessions = Vec::new();
        let mut aggregate_bytes = 0usize;
        let files = self.session_files(max_files)?;
        for file_name in files {
            let file_name = file_name
                .into_string()
                .map_err(|_| ChatSessionStoreError::InvalidSessionId("non-UTF-8 leaf".into()))?;
            let Some(session_id) = file_name.strip_suffix(".json") else {
                continue;
            };
            validate_session_id(session_id)?;
            if let Some((session, byte_len)) = self.load_with_size(session_id)? {
                aggregate_bytes = aggregate_bytes
                    .checked_add(byte_len)
                    .ok_or(ChatSessionStoreError::AggregateTooLarge)?;
                if aggregate_bytes > max_aggregate_bytes {
                    return Err(ChatSessionStoreError::AggregateTooLarge);
                }
                sessions.push(session);
            }
        }
        sessions.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(sessions)
    }

    fn session_files(
        &self,
        max_files: usize,
    ) -> Result<Vec<std::ffi::OsString>, ChatSessionStoreError> {
        self.root
            .list_chat_session_files(max_files)
            .map_err(|error| {
                if error.to_string().contains("entry limit") {
                    ChatSessionStoreError::TooManySessions
                } else {
                    ChatSessionStoreError::Project(error)
                }
            })
    }
}

fn session_file_name(session_id: &str) -> Result<String, ChatSessionStoreError> {
    validate_session_id(session_id)?;
    Ok(format!("{session_id}.json"))
}

fn validate_session_id(session_id: &str) -> Result<(), ChatSessionStoreError> {
    if session_id.is_empty()
        || session_id.len() > 128
        || !session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ChatSessionStoreError::InvalidSessionId(
            session_id.to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::{ChatMessage, ChatSession};

    #[test]
    fn session_round_trips_atomically_and_lists_newest_first() {
        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("Chat.opentake");
        std::fs::create_dir(&bundle).unwrap();
        let store = ChatSessionStore::open(&bundle).unwrap();

        let mut older = ChatSession::new("chat-older");
        older.created_at = 10;
        older.messages.push(ChatMessage::user("first"));
        store.save(&older).unwrap();
        let mut newer = ChatSession::new("chat-newer");
        newer.created_at = 20;
        newer.messages.push(ChatMessage::user("second"));
        store.save(&newer).unwrap();

        assert_eq!(store.load("chat-older").unwrap().unwrap().messages.len(), 1);
        assert_eq!(
            store
                .list()
                .unwrap()
                .into_iter()
                .map(|session| session.id)
                .collect::<Vec<_>>(),
            ["chat-newer", "chat-older"]
        );
        let entries = std::fs::read_dir(bundle.join("chat-sessions"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .all(|name| !name.to_string_lossy().contains(".tmp")));
    }

    #[test]
    fn rejects_traversal_and_a_mismatched_persisted_id() {
        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("Chat.opentake");
        std::fs::create_dir(&bundle).unwrap();
        let store = ChatSessionStore::open(&bundle).unwrap();

        assert!(matches!(
            store.load("../escape"),
            Err(ChatSessionStoreError::InvalidSessionId(_))
        ));
        store.save(&ChatSession::new("safe")).unwrap();
        let path = bundle.join("chat-sessions/safe.json");
        let mut wrong = ChatSession::new("other");
        wrong.created_at = 1;
        std::fs::write(path, serde_json::to_vec(&wrong).unwrap()).unwrap();
        assert!(matches!(
            store.load("safe"),
            Err(ChatSessionStoreError::MismatchedSessionId { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlinked_chat_sessions_directory() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("Chat.opentake");
        let outside = temp.path().join("outside");
        std::fs::create_dir(&bundle).unwrap();
        std::fs::create_dir(&outside).unwrap();
        symlink(&outside, bundle.join("chat-sessions")).unwrap();
        let store = ChatSessionStore::open(&bundle).unwrap();

        assert!(store.save(&ChatSession::new("safe")).is_err());
        assert!(std::fs::read_dir(outside).unwrap().next().is_none());
    }

    #[test]
    fn listing_enforces_file_and_aggregate_limits() {
        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("Chat.opentake");
        std::fs::create_dir(&bundle).unwrap();
        let store = ChatSessionStore::open(&bundle).unwrap();
        store.save(&ChatSession::new("one")).unwrap();
        store.save(&ChatSession::new("two")).unwrap();

        assert!(matches!(
            store.list_with_limits(1, MAX_AGGREGATE_SESSION_BYTES),
            Err(ChatSessionStoreError::TooManySessions)
        ));
        assert!(matches!(
            store.list_with_limits(MAX_SESSION_FILES, 1),
            Err(ChatSessionStoreError::AggregateTooLarge)
        ));
    }

    #[test]
    fn successful_saves_always_remain_listable_with_the_same_limits() {
        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("Chat.opentake");
        std::fs::create_dir(&bundle).unwrap();
        let store = ChatSessionStore::open(&bundle).unwrap();
        let mut first = ChatSession::new("one");
        first.messages.push(ChatMessage::user("small"));
        store.save_with_limits(&first, 1, 1024).unwrap();
        assert_eq!(store.list_with_limits(1, 1024).unwrap().len(), 1);

        assert!(matches!(
            store.save_with_limits(&ChatSession::new("two"), 1, 1024),
            Err(ChatSessionStoreError::TooManySessions)
        ));
        first.messages.push(ChatMessage::user("replacement"));
        store.save_with_limits(&first, 1, 1024).unwrap();
        assert_eq!(store.list_with_limits(1, 1024).unwrap().len(), 1);

        assert!(matches!(
            store.save_with_limits(&first, 1, 1),
            Err(ChatSessionStoreError::AggregateTooLarge)
        ));
        assert_eq!(store.list_with_limits(1, 1024).unwrap().len(), 1);
    }
}
