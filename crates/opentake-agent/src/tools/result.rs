//! Neutral tool-result type shared by the MCP server and the in-app chat loop.
//! 1:1 port of upstream `ToolResult.swift` (`agent-SPEC.md` §4.4). The rmcp
//! conversion lives in `mcp::server`; this module stays transport-free so it is
//! unit-testable offline and reusable by the chat loop.

use serde::{Deserialize, Serialize};

/// One content block in a tool result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Block {
    Text {
        text: String,
    },
    Image {
        base64: String,
        #[serde(rename = "mediaType")]
        media_type: String,
    },
}

impl Block {
    pub fn text(s: impl Into<String>) -> Self {
        Block::Text { text: s.into() }
    }
    pub fn image(base64: impl Into<String>, media_type: impl Into<String>) -> Self {
        Block::Image {
            base64: base64.into(),
            media_type: media_type.into(),
        }
    }
}

/// A tool invocation result: a list of content blocks plus an error flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<Block>,
    pub is_error: bool,
}

impl ToolResult {
    /// A successful text result.
    pub fn ok(text: impl Into<String>) -> Self {
        ToolResult {
            content: vec![Block::text(text)],
            is_error: false,
        }
    }

    /// An error text result (LLM-facing message).
    pub fn error(message: impl Into<String>) -> Self {
        ToolResult {
            content: vec![Block::text(message)],
            is_error: true,
        }
    }

    /// A successful result carrying explicit blocks.
    pub fn blocks(content: Vec<Block>) -> Self {
        ToolResult {
            content,
            is_error: false,
        }
    }

    /// Append a block (used by the context-signal engine to attach a signal
    /// block after the main result, `agent-SPEC.md` §6.1).
    pub fn push(&mut self, block: Block) {
        self.content.push(block);
    }

    /// Concatenated text of all text blocks (used by short-id shortening and by
    /// tests). Image blocks are skipped.
    pub fn text_joined(&self) -> String {
        self.content
            .iter()
            .filter_map(|b| match b {
                Block::Text { text } => Some(text.as_str()),
                Block::Image { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_and_error_shapes() {
        let ok = ToolResult::ok("done");
        assert!(!ok.is_error);
        assert_eq!(ok.content, vec![Block::text("done")]);

        let err = ToolResult::error("bad");
        assert!(err.is_error);
        assert_eq!(err.text_joined(), "bad");
    }

    #[test]
    fn push_appends_block() {
        let mut r = ToolResult::ok("a");
        r.push(Block::text("b"));
        assert_eq!(r.content.len(), 2);
        assert_eq!(r.text_joined(), "ab");
    }

    #[test]
    fn text_joined_skips_images() {
        let r = ToolResult::blocks(vec![
            Block::text("hello "),
            Block::image("AAAA", "image/png"),
            Block::text("world"),
        ]);
        assert_eq!(r.text_joined(), "hello world");
    }

    #[test]
    fn block_serde_roundtrip() {
        let b = Block::image("data", "image/jpeg");
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.contains("\"kind\":\"image\""));
        assert!(json.contains("\"mediaType\":\"image/jpeg\""));
        let back: Block = serde_json::from_str(&json).unwrap();
        assert_eq!(b, back);
    }
}
