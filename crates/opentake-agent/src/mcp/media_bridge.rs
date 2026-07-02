//! `MediaBridge` — the injected side-door to the two capabilities that live
//! outside `opentake-core`: GPU compositing (in `opentake-render`, driven from
//! `src-tauri`) and the user-facing media-import machinery (in
//! `src-tauri/src/media.rs`, behind `MediaState`/`MediaEngine`).
//!
//! Why a *separate* trait instead of widening [`CoreHandle`](super::core_handle):
//! [`CoreHandle`] is deliberately the narrow *document* surface (read timeline,
//! read media, apply one command, project dir, decode analysis PCM). Its
//! production impl wraps only [`opentake_core::AppCore`]. The two capabilities the
//! media tools need reach into crates the agent layer does **not** — and by design
//! should not — link: `opentake-render` (wgpu) for compositing, and the
//! `src-tauri` import path (posters / manifest / events) for `import_media`.
//! Folding them into `CoreHandle` would either drag a GPU dependency into this
//! crate or hide logic behind default methods that can't reach the real paths.
//!
//! So the bridge is injected exactly like the plugin registry is — an optional
//! collaborator the [`Dispatcher`](super::dispatch::Dispatcher) holds. In a plain
//! `vite dev` / non-Tauri build (and in unit tests) there is no bridge and the two
//! tools report an honest "not available" instead of failing to compile. The real
//! implementation is constructed and injected in `src-tauri/src/mcp.rs`, where the
//! render + import code already lives.
//!
//! Both methods default to `Err("unsupported")` so a hand-rolled bridge (or the
//! absence of one) never breaks the build.

use crate::tools::result::Block;

/// One composited timeline frame produced by [`MediaBridge::inspect_timeline`],
/// ready to become MCP image content. `bytes` are already-encoded image data
/// (JPEG in the production path) — the agent crate never links an image encoder;
/// the bridge (which does) hands back finished bytes plus their media type.
#[derive(Debug, Clone)]
pub struct InspectedFrame {
    /// The project frame this image was rendered at.
    pub frame: i32,
    /// Encoded image bytes (e.g. JPEG).
    pub bytes: Vec<u8>,
    /// MIME type of `bytes` (e.g. `"image/jpeg"`).
    pub media_type: String,
}

/// The rendered result of an `inspect_timeline` call: the sampled frames plus the
/// downscaled render dimensions, mirroring upstream `inspectTimeline`'s
/// `imageBlocks + [metaJSON]` result. `total_frames` is echoed back in the meta.
#[derive(Debug, Clone)]
pub struct InspectResult {
    /// Composited frames, in sample order.
    pub frames: Vec<InspectedFrame>,
    /// Downscaled render width (px) — the `width` field of the meta block.
    pub width: u32,
    /// Downscaled render height (px).
    pub height: u32,
}

/// The outcome of an `import_media` call, mirroring upstream's `.ok("…")` string
/// results. The dispatcher wraps `message` in a [`crate::tools::result::ToolResult`].
#[derive(Debug, Clone)]
pub struct ImportOutcome {
    /// Human/LLM-facing confirmation line (same shape as upstream `.ok(...)`).
    pub message: String,
}

/// One decoded `source` object for [`MediaBridge::import_media`]. The dispatcher
/// has already enforced *exactly one* of `url` / `path` / `bytes` is set and that
/// `mime_type` is present when `bytes` is; the bridge does the IO.
#[derive(Debug, Clone)]
pub enum ImportSource {
    /// Absolute local file or directory path, imported in place (directories are
    /// mirrored recursively).
    Path(String),
    /// Base64-encoded inline bytes written into the project bundle's `media/`.
    Bytes {
        /// Raw base64 (already length-checked by the dispatcher).
        base64: String,
        /// Required MIME type (drives the written file's extension).
        mime_type: String,
    },
    /// HTTPS URL downloaded into the project bundle's `media/`.
    Url {
        /// The HTTPS URL (scheme already validated by the dispatcher).
        url: String,
        /// Optional MIME override for extension inference (signed URLs).
        mime_type: Option<String>,
    },
}

/// A user-visible import error the bridge raises. Carries an LLM-facing message
/// the dispatcher surfaces verbatim (upstream `ToolError` messages).
#[derive(Debug, Clone)]
pub struct BridgeError {
    /// The message shown to the model.
    pub message: String,
}

impl BridgeError {
    /// Wrap a message.
    pub fn new(message: impl Into<String>) -> Self {
        BridgeError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BridgeError {}

/// The injected capability boundary for the render + import tools. `Send + Sync`
/// so the [`Dispatcher`](super::dispatch::Dispatcher) can hold `Arc<dyn
/// MediaBridge>` across threads (matching [`CoreHandle`](super::core_handle)).
pub trait MediaBridge: Send + Sync {
    /// Composite the timeline at each `frames` value and return them as encoded
    /// image bytes, downscaled so the longest edge is at most `max_longest_edge`.
    /// Frame numbers are validated by the dispatcher; the bridge composites and
    /// encodes. Frames that fail to render are dropped (upstream `continue`s past a
    /// failed `generator.image(at:)`), so the returned `frames` may be shorter than
    /// the request; an all-empty render is an `Err`.
    fn inspect_timeline(
        &self,
        _frames: &[i32],
        _max_longest_edge: u32,
    ) -> Result<InspectResult, BridgeError> {
        Err(BridgeError::new(
            "inspect_timeline: rendering is not available in this build",
        ))
    }

    /// Import media through the SAME path as the user-facing import (posters,
    /// manifest entry, `MediaChanged` event). `folder_id`, when set, has already
    /// been checked to exist by the dispatcher. Returns the confirmation message.
    fn import_media(
        &self,
        _source: ImportSource,
        _name: Option<String>,
        _folder_id: Option<String>,
    ) -> Result<ImportOutcome, BridgeError> {
        Err(BridgeError::new(
            "import_media: importing is not available in this build",
        ))
    }
}

/// Turn one [`InspectedFrame`] into an MCP image [`Block`], base64-encoding the
/// bytes (rmcp image content is base64). Kept here so the dispatcher stays free of
/// encoding concerns.
pub fn frame_to_block(frame: &InspectedFrame) -> Block {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&frame.bytes);
    Block::image(b64, frame.media_type.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default trait methods report "unsupported" without a real bridge — the
    /// non-Tauri build path.
    struct NoopBridge;
    impl MediaBridge for NoopBridge {}

    #[test]
    fn default_inspect_timeline_is_unsupported() {
        let b = NoopBridge;
        let err = b.inspect_timeline(&[0], 512).unwrap_err();
        assert!(err.message.contains("not available"), "{}", err.message);
    }

    #[test]
    fn default_import_media_is_unsupported() {
        let b = NoopBridge;
        let err = b
            .import_media(ImportSource::Path("/x.mp4".into()), None, None)
            .unwrap_err();
        assert!(err.message.contains("not available"), "{}", err.message);
    }

    #[test]
    fn frame_to_block_base64_encodes_image_content() {
        let f = InspectedFrame {
            frame: 3,
            bytes: vec![0xff, 0xd8, 0xff, 0xe0],
            media_type: "image/jpeg".into(),
        };
        match frame_to_block(&f) {
            Block::Image { base64, media_type } => {
                use base64::Engine as _;
                assert_eq!(media_type, "image/jpeg");
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(base64)
                    .unwrap();
                assert_eq!(decoded, vec![0xff, 0xd8, 0xff, 0xe0]);
            }
            _ => panic!("expected image block"),
        }
    }
}
