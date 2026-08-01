//! Neutral tool-result type shared by the MCP server and the in-app chat loop.
//! 1:1 port of upstream `ToolResult.swift` (`agent-SPEC.md` §4.4). The rmcp
//! conversion lives in `mcp::server`; this module stays transport-free so it is
//! unit-testable offline and reusable by the chat loop.

use serde::{Deserialize, Serialize};

use super::names::ToolName;

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
    /// Error details are private by default. Only dispatcher-owned, typed
    /// preflight failures opt in to LLM disclosure; serde deliberately drops
    /// this capability so crossing an unrecognised boundary fails closed.
    #[serde(skip)]
    pub(crate) llm_error: Option<PublicErrorKind>,
}

/// Fixed, reviewable classes of errors that may expose a compact detail to an
/// LLM after the final boundary rebuilds it from the matching tool schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PublicErrorKind {
    UnknownTool,
    InvalidArguments(ToolName),
    ResourceNotFound(ToolName),
    CapabilityUnavailable(ToolName),
    AnalysisLowConfidence(ToolName),
}

impl PublicErrorKind {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::UnknownTool => "MCP_UNKNOWN_TOOL",
            Self::InvalidArguments(_) => "MCP_INVALID_ARGUMENTS",
            Self::ResourceNotFound(_) => "MCP_RESOURCE_NOT_FOUND",
            Self::CapabilityUnavailable(_) => "MCP_CAPABILITY_UNAVAILABLE",
            Self::AnalysisLowConfidence(_) => "MCP_ANALYSIS_LOW_CONFIDENCE",
        }
    }

    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::UnknownTool => "The requested tool is not available.",
            Self::InvalidArguments(_) => "The tool request has invalid arguments.",
            Self::ResourceNotFound(_) => "The referenced project resource was not found.",
            Self::CapabilityUnavailable(_) => {
                "This capability is unavailable for the referenced media."
            }
            Self::AnalysisLowConfidence(_) => {
                "The analysis could not identify the requested subject reliably."
            }
        }
    }

    pub(crate) fn remediation(self) -> &'static str {
        match self {
            Self::UnknownTool => "Choose a tool returned by the current tool catalog, then retry.",
            Self::InvalidArguments(_) => "Correct the reported arguments, then retry.",
            Self::ResourceNotFound(_) => {
                "Refresh project state, choose an existing resource ID, then retry."
            }
            Self::CapabilityUnavailable(_) => {
                "Use a supported source type or restore the source media, then retry."
            }
            Self::AnalysisLowConfidence(_) => {
                "Choose a tighter, higher-contrast subject region and retry."
            }
        }
    }
}

impl ToolResult {
    /// A successful text result.
    pub fn ok(text: impl Into<String>) -> Self {
        ToolResult {
            content: vec![Block::text(text)],
            is_error: false,
            llm_error: None,
        }
    }

    /// A private error. The text remains available to trusted in-process
    /// diagnostics, but final MCP/chat boundaries must redact it.
    pub fn error(message: impl Into<String>) -> Self {
        ToolResult {
            content: vec![Block::text(message)],
            is_error: true,
            llm_error: None,
        }
    }

    /// A dispatcher-owned preflight error whose detail may be shown to an LLM
    /// only if the final boundary independently accepts the text as safe.
    pub(crate) fn public_error(kind: PublicErrorKind, message: impl Into<String>) -> Self {
        ToolResult {
            content: vec![Block::text(message)],
            is_error: true,
            llm_error: Some(kind),
        }
    }

    /// A successful result carrying explicit blocks.
    pub fn blocks(content: Vec<Block>) -> Self {
        ToolResult {
            content,
            is_error: false,
            llm_error: None,
        }
    }

    pub(crate) fn public_error_kind(&self) -> Option<PublicErrorKind> {
        self.llm_error
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
        assert_eq!(err.public_error_kind(), None);

        let public = ToolResult::public_error(
            PublicErrorKind::InvalidArguments(ToolName::AddClips),
            "entries[0] is invalid",
        );
        assert_eq!(
            public.public_error_kind(),
            Some(PublicErrorKind::InvalidArguments(ToolName::AddClips))
        );
    }

    #[test]
    fn serde_drops_public_error_capability() {
        let result = ToolResult::public_error(
            PublicErrorKind::InvalidArguments(ToolName::AddClips),
            "entries[0] is invalid",
        );
        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.contains("MCP_INVALID_ARGUMENTS"));
        let decoded: ToolResult = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.public_error_kind(), None);
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
