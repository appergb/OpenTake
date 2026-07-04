//! Workflow-plugin `plugin.json` model (`agent-SPEC.md` §7.1,
//! `WORKFLOW-PLUGIN-SYSTEM.md` §1.2). Pure JSON — no Rust compile, no WASM. All
//! fields are `#[serde(default)]`-tolerant so a partial/legacy manifest never
//! fails to decode (validation is a separate, lenient pass).

use std::collections::BTreeMap;

use serde::Deserialize;

/// Plugin author block.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct PluginAuthor {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
}

/// Detection hints (free-form; used for auto-match recommendations).
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct DetectionHints {
    #[serde(default)]
    pub track_patterns: Vec<String>,
    #[serde(default)]
    pub broll_ratio: Option<f64>,
}

/// The plugin's declared video type (overrides auto-detection when active).
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PluginVideoType {
    #[serde(default)]
    pub primary: String,
    #[serde(default)]
    pub subtypes: Vec<String>,
    #[serde(default)]
    pub detection_hints: DetectionHints,
}

/// One suggested action inside a workflow stage.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct PluginAction {
    #[serde(default)]
    pub tool: String,
    #[serde(default)]
    pub tip: String,
}

/// One workflow stage.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct PluginStage {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub order: u32,
    #[serde(default)]
    pub actions: Vec<PluginAction>,
}

/// do/dont rule lists. `do` is a Rust keyword, so it's renamed.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct PluginRules {
    #[serde(default, rename = "do")]
    pub do_: Vec<String>,
    #[serde(default)]
    pub dont: Vec<String>,
}

/// The workflow definition.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct PluginWorkflow {
    #[serde(default)]
    pub approach: String,
    #[serde(default)]
    pub stages: Vec<PluginStage>,
    #[serde(default)]
    pub rules: PluginRules,
}

/// A declared track role override.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct PluginTrackRole {
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub locked: bool,
}

/// The whole `plugin.json`. All fields tolerant.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PluginManifest {
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: PluginAuthor,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub video_type: PluginVideoType,
    #[serde(default)]
    pub workflow: PluginWorkflow,
    /// Map like `{"V1": {role, label, locked?}, ...}`. BTreeMap for stable order.
    #[serde(default)]
    pub track_roles: BTreeMap<String, PluginTrackRole>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "schema_version": "1.0",
        "id": "opentake-workflow-popular-science",
        "name": "科普视频工作流",
        "description": "口播精剪 + 示意图 B-roll",
        "author": {"name": "作者名", "url": "https://x"},
        "license": "MIT",
        "tags": ["科普", "教育"],
        "video_type": {
            "primary": "talking_head",
            "subtypes": ["educational"],
            "detection_hints": {"track_patterns": ["1 video + 1 audio"], "broll_ratio": 0.3}
        },
        "workflow": {
            "approach": "audio_driven",
            "stages": [
                {"id": "import", "name": "导入素材", "order": 0,
                 "actions": [{"tool": "import_media", "tip": "分别导入"}]},
                {"id": "rough_cut", "name": "精剪口播", "order": 1,
                 "actions": [{"tool": "split_clip", "tip": "气口处分割"}]}
            ],
            "rules": {
                "do": ["每段配示意图", "关键术语叠文字"],
                "dont": ["不要连续 3 段以上无 B-roll 覆盖"]
            }
        },
        "track_roles": {
            "V1": {"role": "MainCamera", "label": "口播主画面"},
            "A1": {"role": "VoiceOver", "label": "口播音轨", "locked": true}
        }
    }"#;

    #[test]
    fn full_manifest_decodes() {
        let m: PluginManifest = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(m.id, "opentake-workflow-popular-science");
        assert_eq!(m.schema_version, "1.0");
        assert_eq!(m.video_type.primary, "talking_head");
        assert_eq!(m.workflow.approach, "audio_driven");
        assert_eq!(m.workflow.stages.len(), 2);
        assert_eq!(m.workflow.stages[0].actions[0].tool, "import_media");
        assert_eq!(m.workflow.rules.do_.len(), 2);
        assert_eq!(m.workflow.rules.dont.len(), 1);
        assert_eq!(m.track_roles["V1"].role, "MainCamera");
        assert!(m.track_roles["A1"].locked);
    }

    #[test]
    fn partial_manifest_tolerant() {
        let m: PluginManifest = serde_json::from_str(r#"{"id":"x","name":"X"}"#).unwrap();
        assert_eq!(m.id, "x");
        assert_eq!(m.name, "X");
        assert!(m.workflow.stages.is_empty());
        assert!(m.track_roles.is_empty());
        assert_eq!(m.schema_version, "");
    }

    #[test]
    fn rules_do_keyword_renamed() {
        let m: PluginManifest =
            serde_json::from_str(r#"{"workflow":{"rules":{"do":["a"],"dont":["b"]}}}"#).unwrap();
        assert_eq!(m.workflow.rules.do_, vec!["a"]);
        assert_eq!(m.workflow.rules.dont, vec!["b"]);
    }

    #[test]
    fn empty_object_decodes_to_default() {
        let m: PluginManifest = serde_json::from_str("{}").unwrap();
        assert_eq!(m, PluginManifest::default());
    }
}
