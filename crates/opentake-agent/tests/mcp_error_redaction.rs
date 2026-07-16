use opentake_agent::mcp::convert::to_call_tool_result;
use opentake_agent::tools::result::{Block, ToolResult};

fn error_wire(result: ToolResult) -> String {
    let converted = to_call_tool_result(result);
    assert_eq!(converted.is_error, Some(true));
    serde_json::to_string(&converted).expect("serialize MCP error")
}

#[test]
fn llm_errors_redact_paths_credentials_headers_provider_bodies() {
    // Each case isolates one adversarial category. The plain provider/nested
    // cases intentionally contain no keyword or punctuation that a deny-list
    // could rely on.
    for private in [
        "/Users/alice/private/project/media/secret.mp4",
        "sk-live-abcdefghijklmnopqrstuvwxyz",
        "Bearer oauth-super-secret-token",
        "Authorization: Basic cHJpdmF0ZQ==",
        "https://cdn.example/media.mp4?token=signed-secret&expires=999999",
        "quota exhausted for customer alice plan enterprise",
        "nested decoder detail for customer alice",
        "frame decoder stack detail line 77",
        "oauth-super-secret-token",
    ] {
        let wire = error_wire(ToolResult::error(private));
        assert!(
            !wire.contains(private),
            "private MCP content leaked {private:?}: {wire}"
        );
        assert!(
            wire.contains("MCP_TOOL_ERROR_REDACTED"),
            "private detail was not classified as redacted: {wire}"
        );
        assert!(wire.contains("code"), "typed error code missing: {wire}");
        assert!(
            wire.contains("remediation") && wire.contains("retry"),
            "redacted error lost actionable remediation: {wire}"
        );
    }
}

#[test]
fn multi_block_private_error_never_reassembles_on_the_wire() {
    let mut result = ToolResult::error("quota exhausted for customer alice");
    result.push(Block::text("nested decoder detail line 77"));
    result.push(Block::image("private-image-data", "image/png"));
    let wire = error_wire(result);
    for private in [
        "quota exhausted for customer alice",
        "nested decoder detail line 77",
        "private-image-data",
    ] {
        assert!(!wire.contains(private), "multi-block leak: {wire}");
    }
}
