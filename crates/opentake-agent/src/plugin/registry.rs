//! Plugin registry: load from disk, validate, activate (`agent-SPEC.md` §7.2/
//! §7.3, `WORKFLOW-PLUGIN-SYSTEM.md`). Pure Agent-layer state — no core editing
//! is touched. Single-activation (extensible to multi).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use opentake_domain::{TrackRole, VideoType};

use crate::plugin::model::PluginManifest;
use crate::tools::names::ToolName;

/// Supported `schema_version` values.
const SUPPORTED_SCHEMA_VERSIONS: &[&str] = &["1.0"];

/// Errors loading/validating a plugin.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PluginError {
    #[error("plugin io error: {0}")]
    Io(String),
    #[error("plugin json error: {0}")]
    Json(String),
    #[error("plugin validation error: {0}")]
    Validation(String),
    #[error("workflow not found: {0}")]
    NotFound(String),
}

/// A loaded plugin: manifest + the read-in `instructions.md` + its directory.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub instructions_md: String,
    pub dir: PathBuf,
    /// Validation warnings (non-fatal): unknown tool names, unparseable roles.
    pub warnings: Vec<String>,
}

impl LoadedPlugin {
    pub fn id(&self) -> &str {
        &self.manifest.id
    }
    pub fn name(&self) -> &str {
        &self.manifest.name
    }
}

/// Map a plugin role string (doc uses "VoiceOver"/"BRollOverlay"/"TextOverlay"
/// etc.) to the real `opentake_domain::TrackRole`. Returns `None` for unknown.
pub fn parse_track_role(s: &str) -> Option<TrackRole> {
    match s {
        "MainCamera" => Some(TrackRole::MainCamera),
        "BRoll" | "B_Roll" | "BRollOverlay" | "B_RollOverlay" => Some(TrackRole::BRoll),
        "Voice" | "VoiceOver" => Some(TrackRole::Voice),
        "BGM" | "Bgm" => Some(TrackRole::Bgm),
        "SFX" | "Sfx" => Some(TrackRole::Sfx),
        "Text" | "TextOverlay" => Some(TrackRole::Text),
        "Caption" => Some(TrackRole::Caption),
        _ => None,
    }
}

/// Map a plugin video_type primary string to `VideoType`. Returns `None` for
/// unknown (plugin override then falls back to auto-detection).
pub fn parse_video_type(s: &str) -> Option<VideoType> {
    match s {
        "talking_head" => Some(VideoType::TalkingHead),
        "vlog" => Some(VideoType::Vlog),
        "montage" => Some(VideoType::Montage),
        "interview" => Some(VideoType::Interview),
        "short_form" => Some(VideoType::ShortForm),
        "long_form" => Some(VideoType::LongForm),
        _ => None,
    }
}

/// Validate a manifest, returning fatal errors as `Err` and non-fatal issues as
/// a warning list. Mirrors `opentake plugin validate`
/// (`WORKFLOW-PLUGIN-SYSTEM.md` §1.31 / `agent-SPEC.md` §7.2).
pub fn validate_manifest(m: &PluginManifest) -> Result<Vec<String>, PluginError> {
    if !SUPPORTED_SCHEMA_VERSIONS.contains(&m.schema_version.as_str()) {
        return Err(PluginError::Validation(format!(
            "unsupported schema_version '{}'; supported: {}",
            m.schema_version,
            SUPPORTED_SCHEMA_VERSIONS.join(", ")
        )));
    }
    if m.id.trim().is_empty() {
        return Err(PluginError::Validation("plugin id is required".into()));
    }
    if m.name.trim().is_empty() {
        return Err(PluginError::Validation("plugin name is required".into()));
    }
    // stage order uniqueness (fatal).
    let mut seen_orders = HashSet::new();
    for s in &m.workflow.stages {
        if !seen_orders.insert(s.order) {
            return Err(PluginError::Validation(format!(
                "duplicate stage order {} (orders must be unique)",
                s.order
            )));
        }
    }

    // Non-fatal warnings: unknown tool names, unparseable roles.
    let mut warnings = Vec::new();
    for stage in &m.workflow.stages {
        for action in &stage.actions {
            if ToolName::ALL.iter().all(|t| t.as_str() != action.tool) {
                warnings.push(format!(
                    "stage '{}' references unknown tool '{}'",
                    stage.id, action.tool
                ));
            }
        }
    }
    for (track, role) in &m.track_roles {
        if parse_track_role(&role.role).is_none() {
            warnings.push(format!(
                "track '{}' has unrecognized role '{}'",
                track, role.role
            ));
        }
    }
    if !m.video_type.primary.is_empty() && parse_video_type(&m.video_type.primary).is_none() {
        warnings.push(format!(
            "video_type.primary '{}' is not a recognized type",
            m.video_type.primary
        ));
    }
    Ok(warnings)
}

/// The plugin registry: all installed plugins + the active id.
#[derive(Debug, Default)]
pub struct PluginRegistry {
    installed: Vec<LoadedPlugin>,
    active: Option<String>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        PluginRegistry::default()
    }

    /// Load a single plugin directory: read `plugin.json` + `instructions.md`,
    /// validate. 1:1 with `PluginRegistry::load_dir` (`agent-SPEC.md` §7.2).
    pub fn load_dir(dir: &Path) -> Result<LoadedPlugin, PluginError> {
        let manifest_path = dir.join("plugin.json");
        let raw = fs::read_to_string(&manifest_path)
            .map_err(|e| PluginError::Io(format!("{}: {e}", manifest_path.display())))?;
        let manifest: PluginManifest =
            serde_json::from_str(&raw).map_err(|e| PluginError::Json(e.to_string()))?;
        let instructions_md = fs::read_to_string(dir.join("instructions.md")).unwrap_or_default();
        let warnings = validate_manifest(&manifest)?;
        Ok(LoadedPlugin {
            manifest,
            instructions_md,
            dir: dir.to_path_buf(),
            warnings,
        })
    }

    /// Parse a manifest + instructions from in-memory strings (for tests and
    /// for `activate` without disk). Validates.
    pub fn load_from_strings(
        manifest_json: &str,
        instructions_md: &str,
        dir: impl Into<PathBuf>,
    ) -> Result<LoadedPlugin, PluginError> {
        let manifest: PluginManifest =
            serde_json::from_str(manifest_json).map_err(|e| PluginError::Json(e.to_string()))?;
        let warnings = validate_manifest(&manifest)?;
        Ok(LoadedPlugin {
            manifest,
            instructions_md: instructions_md.to_string(),
            dir: dir.into(),
            warnings,
        })
    }

    /// Scan a directory of plugin subfolders, loading each. Folders that fail to
    /// load are skipped (their error is returned in the second vec).
    pub fn scan(root: &Path) -> (Self, Vec<PluginError>) {
        let mut registry = PluginRegistry::new();
        let mut errors = Vec::new();
        let Ok(entries) = fs::read_dir(root) else {
            return (registry, errors);
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                match Self::load_dir(&path) {
                    Ok(p) => registry.installed.push(p),
                    Err(e) => errors.push(e),
                }
            }
        }
        (registry, errors)
    }

    /// Register an already-loaded plugin.
    pub fn register(&mut self, plugin: LoadedPlugin) {
        // Replace an existing plugin with the same id.
        self.installed.retain(|p| p.id() != plugin.id());
        self.installed.push(plugin);
    }

    /// All installed plugins.
    pub fn installed(&self) -> &[LoadedPlugin] {
        &self.installed
    }

    /// The active plugin, if any.
    pub fn active(&self) -> Option<&LoadedPlugin> {
        let id = self.active.as_ref()?;
        self.installed.iter().find(|p| p.id() == id)
    }

    /// Activate a plugin by id (replaces the previous active one).
    pub fn activate(&mut self, id: &str) -> Result<&LoadedPlugin, PluginError> {
        if !self.installed.iter().any(|p| p.id() == id) {
            return Err(PluginError::NotFound(id.to_string()));
        }
        self.active = Some(id.to_string());
        Ok(self.active().expect("just set"))
    }

    /// Deactivate the current plugin (no-op if none active).
    pub fn deactivate(&mut self) {
        self.active = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest_json(id: &str) -> String {
        format!(
            r#"{{"schema_version":"1.0","id":"{id}","name":"N","workflow":{{"stages":[{{"id":"s0","name":"S0","order":0}}],"rules":{{"do":["x"],"dont":["y"]}}}},"track_roles":{{"V1":{{"role":"MainCamera"}}}}}}"#
        )
    }

    #[test]
    fn parse_roles_maps_doc_spellings() {
        assert_eq!(parse_track_role("VoiceOver"), Some(TrackRole::Voice));
        assert_eq!(parse_track_role("BRollOverlay"), Some(TrackRole::BRoll));
        assert_eq!(parse_track_role("MainCamera"), Some(TrackRole::MainCamera));
        assert_eq!(parse_track_role("BGM"), Some(TrackRole::Bgm));
        assert_eq!(parse_track_role("nope"), None);
    }

    #[test]
    fn parse_video_type_maps_snake_case() {
        assert_eq!(
            parse_video_type("talking_head"),
            Some(VideoType::TalkingHead)
        );
        assert_eq!(parse_video_type("short_form"), Some(VideoType::ShortForm));
        assert_eq!(parse_video_type("bogus"), None);
    }

    #[test]
    fn valid_manifest_has_no_warnings() {
        let p =
            PluginRegistry::load_from_strings(&valid_manifest_json("p1"), "# Guide", ".").unwrap();
        assert!(p.warnings.is_empty());
        assert_eq!(p.instructions_md, "# Guide");
    }

    #[test]
    fn unsupported_schema_version_is_fatal() {
        let json = r#"{"schema_version":"99","id":"p","name":"N"}"#;
        let err = PluginRegistry::load_from_strings(json, "", ".").unwrap_err();
        assert!(matches!(err, PluginError::Validation(_)));
    }

    #[test]
    fn empty_id_is_fatal() {
        let json = r#"{"schema_version":"1.0","id":"","name":"N"}"#;
        let err = PluginRegistry::load_from_strings(json, "", ".").unwrap_err();
        assert!(matches!(err, PluginError::Validation(m) if m.contains("id is required")));
    }

    #[test]
    fn duplicate_stage_order_is_fatal() {
        let json = r#"{"schema_version":"1.0","id":"p","name":"N","workflow":{"stages":[{"id":"a","order":0},{"id":"b","order":0}]}}"#;
        let err = PluginRegistry::load_from_strings(json, "", ".").unwrap_err();
        assert!(matches!(err, PluginError::Validation(m) if m.contains("duplicate stage order")));
    }

    #[test]
    fn unknown_tool_is_warning_not_fatal() {
        let json = r#"{"schema_version":"1.0","id":"p","name":"N","workflow":{"stages":[{"id":"a","order":0,"actions":[{"tool":"not_a_tool","tip":"x"}]}]}}"#;
        let p = PluginRegistry::load_from_strings(json, "", ".").unwrap();
        assert!(p
            .warnings
            .iter()
            .any(|w| w.contains("unknown tool 'not_a_tool'")));
    }

    #[test]
    fn unrecognized_role_is_warning() {
        let json =
            r#"{"schema_version":"1.0","id":"p","name":"N","track_roles":{"V1":{"role":"Wat"}}}"#;
        let p = PluginRegistry::load_from_strings(json, "", ".").unwrap();
        assert!(p
            .warnings
            .iter()
            .any(|w| w.contains("unrecognized role 'Wat'")));
    }

    #[test]
    fn activate_and_deactivate() {
        let mut reg = PluginRegistry::new();
        reg.register(
            PluginRegistry::load_from_strings(&valid_manifest_json("p1"), "I1", ".").unwrap(),
        );
        reg.register(
            PluginRegistry::load_from_strings(&valid_manifest_json("p2"), "I2", ".").unwrap(),
        );
        assert!(reg.active().is_none());

        let active = reg.activate("p2").unwrap();
        assert_eq!(active.id(), "p2");
        assert_eq!(reg.active().unwrap().id(), "p2");

        // Activating replaces.
        reg.activate("p1").unwrap();
        assert_eq!(reg.active().unwrap().id(), "p1");

        reg.deactivate();
        assert!(reg.active().is_none());
    }

    #[test]
    fn activate_unknown_errors() {
        let mut reg = PluginRegistry::new();
        let err = reg.activate("ghost").unwrap_err();
        assert_eq!(err, PluginError::NotFound("ghost".into()));
    }

    #[test]
    fn register_replaces_same_id() {
        let mut reg = PluginRegistry::new();
        reg.register(
            PluginRegistry::load_from_strings(&valid_manifest_json("p1"), "v1", ".").unwrap(),
        );
        reg.register(
            PluginRegistry::load_from_strings(&valid_manifest_json("p1"), "v2", ".").unwrap(),
        );
        assert_eq!(reg.installed().len(), 1);
        assert_eq!(reg.installed()[0].instructions_md, "v2");
    }

    #[test]
    fn load_dir_reads_files() {
        let dir = std::env::temp_dir().join(format!("ot-plugin-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("plugin.json"), valid_manifest_json("disk-p")).unwrap();
        fs::write(dir.join("instructions.md"), "# Disk guide").unwrap();
        let p = PluginRegistry::load_dir(&dir).unwrap();
        assert_eq!(p.id(), "disk-p");
        assert_eq!(p.instructions_md, "# Disk guide");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_dir_missing_instructions_defaults_empty() {
        let dir = std::env::temp_dir().join(format!("ot-plugin-noinstr-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("plugin.json"), valid_manifest_json("p")).unwrap();
        let p = PluginRegistry::load_dir(&dir).unwrap();
        assert_eq!(p.instructions_md, "");
        let _ = fs::remove_dir_all(&dir);
    }
}
