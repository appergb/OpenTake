use opentake_domain::{ClipType, GenerationJobStatus, MediaManifest};
use serde::Serialize;

/// Model-facing media catalog. This is deliberately separate from the durable
/// manifest so persistence-only fields cannot cross the LLM boundary by default.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ModelMediaCatalog<'a> {
    version: i64,
    entries: Vec<ModelMediaEntry<'a>>,
    folders: Vec<ModelMediaFolder<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelMediaEntry<'a> {
    id: &'a str,
    name: &'a str,
    #[serde(rename = "type")]
    kind: ClipType,
    duration: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_width: Option<i32>,
    #[serde(rename = "sourceFPS", skip_serializing_if = "Option::is_none")]
    source_fps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_height: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    has_audio: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    folder_id: Option<&'a str>,
    has_proxy: bool,
    is_hdr: bool,
    generation_status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_progress: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelMediaFolder<'a> {
    id: &'a str,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_folder_id: Option<&'a str>,
}

impl<'a> From<&'a MediaManifest> for ModelMediaCatalog<'a> {
    fn from(manifest: &'a MediaManifest) -> Self {
        Self {
            version: manifest.version,
            entries: manifest
                .entries
                .iter()
                .map(|entry| {
                    let generation = entry.generation_input.as_ref();
                    ModelMediaEntry {
                        id: &entry.id,
                        name: &entry.name,
                        kind: entry.kind,
                        duration: entry.duration,
                        source_width: entry.source_width,
                        source_height: entry.source_height,
                        source_fps: entry.source_fps,
                        has_audio: entry.has_audio,
                        folder_id: entry.folder_id.as_deref(),
                        has_proxy: entry.proxy.is_some(),
                        is_hdr: entry.color.as_ref().is_some_and(|color| color.is_hdr()),
                        generation_status: generation_status_label(
                            generation.and_then(|input| input.status),
                        ),
                        generation_progress: generation.and_then(|input| input.progress),
                    }
                })
                .collect(),
            folders: manifest
                .folders
                .iter()
                .map(|folder| ModelMediaFolder {
                    id: &folder.id,
                    name: &folder.name,
                    parent_folder_id: folder.parent_folder_id.as_deref(),
                })
                .collect(),
        }
    }
}

fn generation_status_label(status: Option<GenerationJobStatus>) -> &'static str {
    match status {
        Some(GenerationJobStatus::Queued | GenerationJobStatus::Generating) => "generating",
        Some(GenerationJobStatus::Downloading | GenerationJobStatus::Finalizing) => "downloading",
        Some(GenerationJobStatus::Failed) => "failed",
        Some(GenerationJobStatus::Cancelled) => "cancelled",
        Some(GenerationJobStatus::Ready) | None => "none",
    }
}
