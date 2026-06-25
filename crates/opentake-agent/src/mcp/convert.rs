//! Convert the transport-neutral [`ToolResult`] into rmcp's [`CallToolResult`]
//! (`agent-SPEC.md` §8.3). Text blocks map to text content; image blocks (e.g. a
//! future `inspect_timeline` frame) map to base64 image content. The `is_error`
//! flag drives `CallToolResult::error` vs `success`.

use rmcp::model::{CallToolResult, Content};

use crate::tools::result::{Block, ToolResult};

/// Map one neutral [`Block`] to an rmcp [`Content`].
fn block_to_content(block: Block) -> Content {
    match block {
        Block::Text { text } => Content::text(text),
        Block::Image { base64, media_type } => Content::image(base64, media_type),
    }
}

/// Map a [`ToolResult`] to an rmcp [`CallToolResult`].
pub fn to_call_tool_result(result: ToolResult) -> CallToolResult {
    let content: Vec<Content> = result.content.into_iter().map(block_to_content).collect();
    if result.is_error {
        CallToolResult::error(content)
    } else {
        CallToolResult::success(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_result_maps_to_success_text() {
        let r = ToolResult::ok("hello");
        let c = to_call_tool_result(r);
        assert_ne!(c.is_error, Some(true));
        assert_eq!(c.content.len(), 1);
    }

    #[test]
    fn error_result_maps_to_error() {
        let r = ToolResult::error("nope");
        let c = to_call_tool_result(r);
        assert_eq!(c.is_error, Some(true));
    }

    #[test]
    fn image_block_maps_to_image_content() {
        let r = ToolResult::blocks(vec![Block::image("AAAA", "image/png")]);
        let c = to_call_tool_result(r);
        assert_eq!(c.content.len(), 1);
    }
}
