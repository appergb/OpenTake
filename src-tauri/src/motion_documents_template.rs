//! Built-in Motion Studio storage contract and starter sources.

pub(super) const MOTION_DOCUMENTS_DIR: &str = "motion-documents";
pub(super) const CATALOG_FILE: &str = "catalog.json";
pub(super) const DOCUMENT_MANIFEST_FILE: &str = "manifest.json";
pub(super) const HTML_FILE: &str = "index.html";
pub(super) const CSS_FILE: &str = "styles.css";
pub(super) const CATALOG_SCHEMA_VERSION: u32 = 1;
pub(super) const DOCUMENT_SCHEMA_VERSION: u32 = 1;
pub(super) const MAX_DOCUMENTS: usize = 256;
pub(super) const MAX_CATALOG_BYTES: usize = 1024 * 1024;
pub(super) const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub(super) const MAX_SOURCE_BYTES: usize = 512 * 1024;
pub(super) const MAX_PARAMETERS_BYTES: usize = 64 * 1024;
pub(super) const MAX_TITLE_CHARS: usize = 128;
pub(super) const MAX_PATCH_EDITS: usize = 2048;

pub(super) const STARTER_HTML: &str = r#"<main class="motion-stage">
  <p class="motion-kicker">Motion Studio</p>
  <h1>让创意动起来</h1>
  <p class="motion-subtitle">Real HTML · Real CSS · Real motion</p>
</main>
"#;

pub(super) const STARTER_CSS: &str = r#"html, body {
  width: 100%;
  height: 100%;
  margin: 0;
  overflow: hidden;
  background: #111214;
  color: #f7f7f5;
  font-family: Inter, "PingFang SC", sans-serif;
}

.motion-stage {
  box-sizing: border-box;
  display: grid;
  align-content: center;
  width: 100%;
  height: 100%;
  padding: 10%;
  background: radial-gradient(circle at 72% 24%, #5f5cff 0, transparent 34%);
}

.motion-kicker { color: #a9a7ff; letter-spacing: .18em; text-transform: uppercase; }
h1 { margin: .12em 0; font-size: clamp(48px, 8vw, 144px); animation: title-in 1.2s both; }
.motion-subtitle { font-size: clamp(18px, 2.2vw, 42px); opacity: .72; animation: subtitle-in 1.2s .18s both; }

@keyframes title-in {
  from { opacity: 0; transform: translateY(48px) scale(.96); filter: blur(12px); }
  to { opacity: 1; transform: translateY(0) scale(1); filter: blur(0); }
}

@keyframes subtitle-in {
  from { opacity: 0; transform: translateY(24px); }
  to { opacity: .72; transform: translateY(0); }
}
"#;
