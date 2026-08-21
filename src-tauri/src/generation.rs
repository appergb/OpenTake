//! Production asynchronous generation bridge shared by MCP and in-app Chat.
//!
//! Durable placeholder state is committed before provider submission. Provider
//! keys stay in the OS keychain; signed result URLs and provider diagnostics are
//! never persisted. Terminal downloads are probed, then streamed into the same
//! complete-bundle publication that makes the original placeholder ready.

use std::collections::{BTreeSet, HashMap};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use futures_util::StreamExt;
use opentake_agent::mcp::generation::{
    finalize_terminal_outputs, DownloadedGenerationArtifact, GenerationArtifactDownloader,
    GenerationBridge, GenerationFinalizationStore, GenerationRequest, GenerationSubmission,
};
use opentake_agent::tools::args::{
    GenerateAudioArgs, GenerateImageArgs, GenerateVideoArgs, UpscaleMediaArgs,
};
use opentake_core::{
    AppCore, GenerationStateUpdate, PreparedGenerationJob, PreparedGenerationOutput, ProbedMedia,
};
use opentake_domain::{ClipType, GenerationInput, GenerationJobStatus, MediaResolver, Timeline};
use opentake_gen::catalog::cost::cost_for_input;
use opentake_gen::{
    build_params, Catalog, CatalogEntry, ElevenLabsAdapter, FalAdapter, GenClient, GenError,
    JobStatus, KeyStore, KeyringStore, ModelKind, ModelRoute, OpenAiAdapter, ProviderKey,
    ProviderRegistry, ReplicateAdapter, ReqwestTransport, StaticToken, UiCapabilities,
};
use opentake_media::{MediaCancelToken, MediaEngine};

const RESULT_BYTES_MAX: u64 = 1024 * 1024 * 1024;
const DATA_URL_ENCODED_MAX: usize = 512 * 1024 * 1024;
const RESULT_REDIRECT_MAX: usize = 5;

#[derive(Default)]
struct GenerationRuntime {
    jobs: Mutex<HashMap<String, ActiveGenerationJob>>,
    terminal_leases: Mutex<BTreeSet<String>>,
    completed: Mutex<BTreeSet<String>>,
}

struct ActiveGenerationJob {
    cancel: MediaCancelToken,
    _admission: crate::updater::ActivityLease,
}

#[derive(Clone)]
pub(crate) struct TauriGenerationBridge {
    core: AppCore,
    engine: Arc<MediaEngine>,
    staging_root: PathBuf,
    runtime: Arc<GenerationRuntime>,
    clients: Arc<dyn GenerationClientFactory>,
    admission: crate::updater::InstallAdmissionGate,
}

trait GenerationClientFactory: Send + Sync {
    fn configured_byok_prefixes(&self) -> BTreeSet<String>;
    fn has_managed_credential(&self) -> bool;
    fn build(&self, provider: &str, managed: bool) -> Result<GenClient, String>;
}

struct ProductionGenerationClientFactory;

impl GenerationClientFactory for ProductionGenerationClientFactory {
    fn configured_byok_prefixes(&self) -> BTreeSet<String> {
        let store = KeyringStore::new();
        [
            ProviderKey::Fal,
            ProviderKey::Replicate,
            ProviderKey::OpenAI,
            ProviderKey::ElevenLabs,
        ]
        .into_iter()
        .filter_map(|key| {
            (&store as &dyn KeyStore)
                .load_key(key)
                .ok()
                .flatten()
                .map(|_| key.prefix().to_string())
        })
        .collect()
    }

    fn has_managed_credential(&self) -> bool {
        crate::account::generation_credential()
            .ok()
            .flatten()
            .is_some()
    }

    fn build(&self, provider: &str, managed: bool) -> Result<GenClient, String> {
        build_client(provider, managed)
    }
}

struct PreparedDispatch {
    plan: PreparedGenerationJob,
    references: Vec<PreparedReference>,
    timeline_span: Option<PreparedTimelineSpan>,
    requires_source_video: bool,
    model_kind: ModelKind,
    managed: bool,
}

struct PreparedTimelineSpan {
    timeline: Timeline,
    manifest: opentake_domain::MediaManifest,
    project_dir: Option<PathBuf>,
    start_frame: i32,
    end_frame: i32,
}

#[derive(Clone)]
struct PreparedReference {
    path: PathBuf,
    fallback: &'static str,
    trim_range: Option<(f64, f64)>,
}

struct StagedCleanup {
    path: PathBuf,
    armed: bool,
}

impl StagedCleanup {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn preserve(mut self) {
        self.armed = false;
    }
}

impl Drop for StagedCleanup {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl PreparedReference {
    fn whole(path: PathBuf, fallback: &'static str) -> Self {
        Self {
            path,
            fallback,
            trim_range: None,
        }
    }
}

pub(crate) fn build_bridge(
    core: AppCore,
    cache_root: PathBuf,
    models_dir: PathBuf,
    admission: crate::updater::InstallAdmissionGate,
) -> Arc<TauriGenerationBridge> {
    Arc::new(TauriGenerationBridge {
        core,
        engine: Arc::new(MediaEngine::new(cache_root.clone(), models_dir)),
        staging_root: cache_root.join("generation-staging"),
        runtime: Arc::new(GenerationRuntime::default()),
        clients: Arc::new(ProductionGenerationClientFactory),
        admission,
    })
}

#[cfg(test)]
fn build_bridge_with_clients(
    core: AppCore,
    cache_root: PathBuf,
    models_dir: PathBuf,
    clients: Arc<dyn GenerationClientFactory>,
) -> Arc<TauriGenerationBridge> {
    build_bridge_with_clients_and_admission(
        core,
        cache_root,
        models_dir,
        clients,
        crate::updater::InstallAdmissionGate::default(),
    )
}

#[cfg(test)]
fn build_bridge_with_clients_and_admission(
    core: AppCore,
    cache_root: PathBuf,
    models_dir: PathBuf,
    clients: Arc<dyn GenerationClientFactory>,
    admission: crate::updater::InstallAdmissionGate,
) -> Arc<TauriGenerationBridge> {
    Arc::new(TauriGenerationBridge {
        core,
        engine: Arc::new(MediaEngine::new(cache_root.clone(), models_dir)),
        staging_root: cache_root.join("generation-staging"),
        runtime: Arc::new(GenerationRuntime::default()),
        clients,
        admission,
    })
}

impl TauriGenerationBridge {
    pub(crate) fn has_active(&self) -> bool {
        self.runtime
            .jobs
            .lock()
            .map(|jobs| !jobs.is_empty())
            .unwrap_or(true)
    }

    pub(crate) fn cancel_all_active(&self) -> usize {
        let jobs = self
            .runtime
            .jobs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for job in jobs.values() {
            job.cancel.cancel();
        }
        jobs.len()
    }

    pub(crate) fn cancel(&self, job_id: &str) -> bool {
        self.runtime
            .jobs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(job_id)
            .is_some_and(|job| {
                job.cancel.cancel();
                true
            })
    }

    pub(crate) fn retry(
        &self,
        job_id: &str,
        cost_authorized: bool,
    ) -> Result<GenerationSubmission, String> {
        if !cost_authorized {
            return Err("cost authorization is required before retry".to_string());
        }
        let snapshot = self.core.runtime_snapshot();
        let outputs = snapshot
            .media
            .entries
            .iter()
            .filter(|entry| {
                entry
                    .generation_input
                    .as_ref()
                    .and_then(|input| input.job_id.as_deref())
                    == Some(job_id)
            })
            .collect::<Vec<_>>();
        let first = outputs
            .first()
            .ok_or_else(|| "generation job does not exist".to_string())?;
        if outputs.iter().any(|entry| {
            !matches!(
                entry
                    .generation_input
                    .as_ref()
                    .and_then(|input| input.status),
                Some(GenerationJobStatus::Failed | GenerationJobStatus::Cancelled)
            )
        }) {
            return Err("only a failed or cancelled generation can be retried".to_string());
        }
        let input = first
            .generation_input
            .as_ref()
            .ok_or_else(|| "generation provenance is missing".to_string())?;
        let catalog = Catalog::builtin();
        let model = catalog
            .entries()
            .iter()
            .find(|entry| entry.id == input.model)
            .ok_or_else(|| "generation model is no longer available".to_string())?;
        let request = match model.kind {
            ModelKind::Video => {
                let frames = input.image_url_asset_ids.clone().unwrap_or_default();
                GenerationRequest::Video(GenerateVideoArgs {
                    cost_authorized: Some(true),
                    prompt: input.prompt.clone(),
                    name: Some(first.name.clone()),
                    model: Some(input.model.clone()),
                    duration: Some(input.duration),
                    aspect_ratio: Some(input.aspect_ratio.clone()),
                    resolution: input.resolution.clone(),
                    start_frame_media_ref: frames.first().cloned(),
                    end_frame_media_ref: frames.get(1).cloned(),
                    source_video_media_ref: input.source_asset_id.clone(),
                    source_clip_id: input.source_clip_id.clone(),
                    reference_image_media_refs: input.reference_image_asset_ids.clone(),
                    reference_video_media_refs: input.reference_video_asset_ids.clone(),
                    reference_audio_media_refs: input.reference_audio_asset_ids.clone(),
                    folder_id: first.folder_id.clone(),
                })
            }
            ModelKind::Image => GenerationRequest::Image(GenerateImageArgs {
                cost_authorized: Some(true),
                prompt: input.prompt.clone(),
                name: Some(first.name.clone()),
                model: Some(input.model.clone()),
                aspect_ratio: Some(input.aspect_ratio.clone()),
                resolution: input.resolution.clone(),
                quality: input.quality.clone(),
                num_images: Some(outputs.len() as i32),
                reference_media_refs: input.reference_image_asset_ids.clone(),
                folder_id: first.folder_id.clone(),
            }),
            ModelKind::Audio => GenerationRequest::Audio(GenerateAudioArgs {
                cost_authorized: Some(true),
                prompt: Some(input.prompt.clone()),
                name: Some(first.name.clone()),
                model: Some(input.model.clone()),
                voice: input.voice.clone(),
                lyrics: input.lyrics.clone(),
                style_instructions: input.style_instructions.clone(),
                instrumental: input.instrumental,
                duration: Some(input.duration),
                video_source_start_frame: input.source_start_frame,
                video_source_end_frame: input.source_end_frame,
                video_source_media_ref: input.source_asset_id.clone(),
                folder_id: first.folder_id.clone(),
            }),
            ModelKind::Upscale => GenerationRequest::Upscale(UpscaleMediaArgs {
                cost_authorized: Some(true),
                media_ref: input
                    .source_asset_id
                    .clone()
                    .ok_or_else(|| "upscale source provenance is missing".to_string())?,
                model: Some(input.model.clone()),
                source_clip_id: input.source_clip_id.clone(),
            }),
        };
        self.submit(request, &MediaCancelToken::new())
    }

    /// Resume provider polling for durable non-terminal jobs after a project is
    /// opened. A queued record without a provider id is deliberately failed and
    /// exposed for explicit retry: resubmitting it automatically could create a
    /// second paid job if the process died between provider acceptance and the
    /// durable id write.
    pub(crate) fn recover_current_project(&self) -> usize {
        #[derive(Default)]
        struct RecoveryJob {
            provider: String,
            provider_job_id: Option<String>,
            placeholders: Vec<(usize, String)>,
            has_active_output: bool,
        }

        let snapshot = self.core.runtime_snapshot();
        let Some(project_dir) = snapshot.project_dir.clone() else {
            return 0;
        };
        let mut recoverable = HashMap::<String, RecoveryJob>::new();
        for entry in &snapshot.media.entries {
            let Some(input) = entry.generation_input.as_ref() else {
                continue;
            };
            let Some(job_id) = input.job_id.as_ref() else {
                continue;
            };
            let job = recoverable.entry(job_id.clone()).or_default();
            if job.provider.is_empty() {
                job.provider = input.provider.clone().unwrap_or_default();
            }
            if job.provider_job_id.is_none() {
                job.provider_job_id = input.provider_job_id.clone();
            }
            job.placeholders
                .push((input.output_index.unwrap_or(usize::MAX), entry.id.clone()));
            if matches!(
                input.status,
                Some(
                    GenerationJobStatus::Queued
                        | GenerationJobStatus::Generating
                        | GenerationJobStatus::Downloading
                        | GenerationJobStatus::Finalizing
                )
            ) {
                job.has_active_output = true;
            }
        }

        let mut resumed = 0;
        for (job_id, mut job) in recoverable {
            if !job.has_active_output {
                continue;
            }
            let Ok(admission) = self.admission.begin_activity() else {
                continue;
            };
            if self
                .runtime
                .jobs
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .contains_key(&job_id)
            {
                continue;
            }
            job.placeholders.sort_by_key(|(index, _)| *index);
            let placeholder_ids = job
                .placeholders
                .into_iter()
                .map(|(_, asset_id)| asset_id)
                .collect::<Vec<_>>();
            let Some(provider_job_id) = job.provider_job_id else {
                let _ = self.core.update_generation_job_for_project(
                    snapshot.project_epoch,
                    &project_dir,
                    &job_id,
                    GenerationStateUpdate {
                        status: GenerationJobStatus::Failed,
                        progress: None,
                        error_code: Some("GENERATION_RESTART_RETRY_REQUIRED".to_string()),
                        provider_job_id: None,
                        cost_credits: None,
                        created_at: Some(now_apple_reference_seconds()),
                    },
                );
                continue;
            };
            if job.provider.is_empty() {
                self.fail_nonterminal_outputs(
                    snapshot.project_epoch,
                    &project_dir,
                    &placeholder_ids,
                    "GENERATION_RECOVERY_STATE_INVALID",
                );
                continue;
            }
            let cancel = MediaCancelToken::new();
            self.runtime
                .jobs
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .insert(
                    job_id.clone(),
                    ActiveGenerationJob {
                        cancel: cancel.clone(),
                        _admission: admission,
                    },
                );
            let bridge = self.clone();
            let recovery_dir = project_dir.clone();
            let managed = !provider_job_id.starts_with(&format!("{}::", job.provider));
            tauri::async_runtime::spawn(async move {
                bridge
                    .run_recovered_job(
                        snapshot.project_epoch,
                        recovery_dir,
                        job_id,
                        placeholder_ids,
                        job.provider,
                        managed,
                        provider_job_id,
                        cancel,
                    )
                    .await;
            });
            resumed += 1;
        }
        resumed
    }

    fn fail_nonterminal_outputs(
        &self,
        project_epoch: u64,
        project_dir: &Path,
        placeholder_ids: &[String],
        code: &str,
    ) {
        let snapshot = self.core.runtime_snapshot();
        for asset_id in placeholder_ids {
            let terminal = snapshot
                .media
                .entries
                .iter()
                .find(|entry| entry.id == *asset_id)
                .and_then(|entry| entry.generation_input.as_ref())
                .and_then(|input| input.status)
                .is_some_and(|status| {
                    matches!(
                        status,
                        GenerationJobStatus::Ready
                            | GenerationJobStatus::Failed
                            | GenerationJobStatus::Cancelled
                    )
                });
            if !terminal {
                let _ = self.core.fail_generation_output_for_project(
                    project_epoch,
                    project_dir,
                    asset_id,
                    code,
                    Some(now_apple_reference_seconds()),
                );
            }
        }
    }

    fn cancel_nonterminal_outputs(
        &self,
        project_epoch: u64,
        project_dir: &Path,
        placeholder_ids: &[String],
    ) {
        for asset_id in placeholder_ids {
            let _ = self.core.cancel_generation_output_for_project(
                project_epoch,
                project_dir,
                asset_id,
                Some(now_apple_reference_seconds()),
            );
        }
    }

    fn configured_byok_prefixes(&self) -> BTreeSet<String> {
        self.clients.configured_byok_prefixes()
    }

    fn has_managed_credential(&self) -> bool {
        self.clients.has_managed_credential()
    }

    fn prepare(&self, request: GenerationRequest) -> Result<PreparedDispatch, String> {
        let snapshot = self.core.runtime_snapshot();
        snapshot
            .project_dir
            .as_deref()
            .ok_or_else(|| "Save the project before starting generation".to_string())?;
        let configured = self.configured_byok_prefixes();
        let managed_available = self.has_managed_credential();
        let catalog = Catalog::builtin();

        match request {
            GenerationRequest::Video(args) => {
                let entry = select_model(
                    &catalog,
                    ModelKind::Video,
                    args.model.as_deref(),
                    &configured,
                    managed_available,
                )?;
                let UiCapabilities::Video(caps) = &entry.ui_capabilities else {
                    return Err("selected model has invalid video capabilities".to_string());
                };
                let duration = args
                    .duration
                    .map(|value| value.max(0) as u32)
                    .or_else(|| caps.durations.first().copied())
                    .unwrap_or(0);
                if !caps.durations.is_empty() && !caps.durations.contains(&duration) {
                    return Err("duration is not supported by the selected model".to_string());
                }
                let aspect_ratio = args
                    .aspect_ratio
                    .clone()
                    .or_else(|| caps.aspect_ratios.first().cloned())
                    .unwrap_or_default();
                if !caps.aspect_ratios.is_empty() && !caps.aspect_ratios.contains(&aspect_ratio) {
                    return Err("aspectRatio is not supported by the selected model".to_string());
                }
                validate_choice(
                    "resolution",
                    args.resolution.as_deref(),
                    caps.resolutions.as_deref(),
                )?;

                let frames = [
                    args.start_frame_media_ref.clone(),
                    args.end_frame_media_ref.clone(),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
                if args.start_frame_media_ref.is_some() && !caps.supports_first_frame {
                    return Err("selected model does not support a first frame".to_string());
                }
                if args.end_frame_media_ref.is_some() && !caps.supports_last_frame {
                    return Err("selected model does not support a last frame".to_string());
                }
                let image_refs = args.reference_image_media_refs.clone().unwrap_or_default();
                let video_refs = args.reference_video_media_refs.clone().unwrap_or_default();
                let audio_refs = args.reference_audio_media_refs.clone().unwrap_or_default();
                validate_reference_count("image", image_refs.len(), caps.max_reference_images)?;
                validate_reference_count("video", video_refs.len(), caps.max_reference_videos)?;
                validate_reference_count("audio", audio_refs.len(), caps.max_reference_audios)?;
                if let Some(max) = caps.max_total_references {
                    if image_refs.len() + video_refs.len() + audio_refs.len() > max as usize {
                        return Err(
                            "too many combined references for the selected model".to_string()
                        );
                    }
                }
                if caps.frames_and_references_exclusive
                    && !frames.is_empty()
                    && (!image_refs.is_empty() || !video_refs.is_empty() || !audio_refs.is_empty())
                {
                    return Err(
                        "frames and references are mutually exclusive for this model".to_string(),
                    );
                }
                if caps.requires_source_video && args.source_video_media_ref.is_none() {
                    return Err("selected model requires sourceVideoMediaRef".to_string());
                }
                if caps.requires_reference_image && image_refs.is_empty() {
                    return Err("selected model requires an image reference".to_string());
                }
                let source_trim = validate_source_clip(
                    &snapshot.timeline,
                    args.source_clip_id.as_deref(),
                    args.source_video_media_ref.as_deref(),
                )?;

                let mut references = Vec::new();
                if caps.requires_source_video {
                    if let Some(source) = args.source_video_media_ref.as_deref() {
                        references.push(PreparedReference {
                            path: resolve_media(&snapshot, source, ClipType::Video)?,
                            fallback: "video",
                            trim_range: source_trim,
                        });
                    }
                } else {
                    for media_ref in &frames {
                        references.push(PreparedReference::whole(
                            resolve_media(&snapshot, media_ref, ClipType::Image)?,
                            "image",
                        ));
                    }
                }
                for media_ref in &image_refs {
                    references.push(PreparedReference::whole(
                        resolve_media(&snapshot, media_ref, ClipType::Image)?,
                        "image",
                    ));
                }
                for media_ref in &video_refs {
                    references.push(PreparedReference::whole(
                        resolve_media(&snapshot, media_ref, ClipType::Video)?,
                        "video",
                    ));
                }
                for media_ref in &audio_refs {
                    references.push(PreparedReference::whole(
                        resolve_media(&snapshot, media_ref, ClipType::Audio)?,
                        "audio",
                    ));
                }

                let provider = provider_prefix(&entry.id)?;
                let input = GenerationInput {
                    prompt: args.prompt.clone(),
                    model: entry.id.clone(),
                    duration: duration as i32,
                    aspect_ratio,
                    resolution: args.resolution.clone(),
                    image_url_asset_ids: (!frames.is_empty()).then_some(frames),
                    reference_image_asset_ids: (!image_refs.is_empty()).then_some(image_refs),
                    reference_video_asset_ids: (!video_refs.is_empty()).then_some(video_refs),
                    reference_audio_asset_ids: (!audio_refs.is_empty()).then_some(audio_refs),
                    generate_audio: Some(true),
                    ..Default::default()
                };
                let estimated_cost_credits = cost_for_input(entry, &input);
                Ok(PreparedDispatch {
                    plan: PreparedGenerationJob {
                        name: display_name(args.name.as_deref(), &args.prompt, "Generated video"),
                        kind: ClipType::Video,
                        folder_id: args.folder_id,
                        provider: provider.clone(),
                        input,
                        output_count: 1,
                        source_asset_id: args.source_video_media_ref,
                        source_clip_id: args.source_clip_id,
                        estimated_cost_credits,
                        created_at: Some(now_apple_reference_seconds()),
                    },
                    references,
                    timeline_span: None,
                    requires_source_video: caps.requires_source_video,
                    model_kind: ModelKind::Video,
                    managed: !configured.contains(&provider) && managed_available,
                })
            }
            GenerationRequest::Image(args) => {
                let entry = select_model(
                    &catalog,
                    ModelKind::Image,
                    args.model.as_deref(),
                    &configured,
                    managed_available,
                )?;
                let UiCapabilities::Image(caps) = &entry.ui_capabilities else {
                    return Err("selected model has invalid image capabilities".to_string());
                };
                validate_choice(
                    "aspectRatio",
                    args.aspect_ratio.as_deref(),
                    Some(&caps.aspect_ratios),
                )?;
                validate_choice(
                    "resolution",
                    args.resolution.as_deref(),
                    caps.resolutions.as_deref(),
                )?;
                validate_choice(
                    "quality",
                    args.quality.as_deref(),
                    caps.qualities.as_deref(),
                )?;
                let refs = args.reference_media_refs.clone().unwrap_or_default();
                if !refs.is_empty() && !caps.supports_image_reference {
                    return Err("selected model does not support image references".to_string());
                }
                if refs.len() > caps.max_images as usize {
                    return Err("too many image references for the selected model".to_string());
                }
                let references = refs
                    .iter()
                    .map(|media_ref| {
                        resolve_media(&snapshot, media_ref, ClipType::Image)
                            .map(|path| PreparedReference::whole(path, "image"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let output_count = args.num_images.unwrap_or(1).clamp(1, 4) as usize;
                let provider = provider_prefix(&entry.id)?;
                let input = GenerationInput {
                    prompt: args.prompt.clone(),
                    model: entry.id.clone(),
                    duration: 0,
                    aspect_ratio: args
                        .aspect_ratio
                        .clone()
                        .or_else(|| caps.aspect_ratios.first().cloned())
                        .unwrap_or_default(),
                    resolution: args.resolution.clone(),
                    quality: args.quality.clone(),
                    num_images: Some(output_count as i32),
                    reference_image_asset_ids: (!refs.is_empty()).then_some(refs),
                    ..Default::default()
                };
                let estimated_cost_credits = cost_for_input(entry, &input);
                Ok(PreparedDispatch {
                    plan: PreparedGenerationJob {
                        name: display_name(args.name.as_deref(), &args.prompt, "Generated image"),
                        kind: ClipType::Image,
                        folder_id: args.folder_id,
                        provider: provider.clone(),
                        input,
                        output_count,
                        source_asset_id: None,
                        source_clip_id: None,
                        estimated_cost_credits,
                        created_at: Some(now_apple_reference_seconds()),
                    },
                    references,
                    timeline_span: None,
                    requires_source_video: false,
                    model_kind: ModelKind::Image,
                    managed: !configured.contains(&provider) && managed_available,
                })
            }
            GenerationRequest::Audio(args) => {
                let entry = select_model(
                    &catalog,
                    ModelKind::Audio,
                    args.model.as_deref(),
                    &configured,
                    managed_available,
                )?;
                let UiCapabilities::Audio(caps) = &entry.ui_capabilities else {
                    return Err("selected model has invalid audio capabilities".to_string());
                };
                let prompt = args.prompt.clone().unwrap_or_default();
                if prompt.chars().count() < caps.min_prompt_length as usize {
                    return Err("prompt is too short for the selected audio model".to_string());
                }
                if args.lyrics.is_some() && !caps.supports_lyrics {
                    return Err("selected model does not support lyrics".to_string());
                }
                if args.instrumental == Some(true) && !caps.supports_instrumental {
                    return Err("selected model does not support instrumental mode".to_string());
                }
                if args.style_instructions.is_some() && !caps.supports_style_instructions {
                    return Err("selected model does not support style instructions".to_string());
                }
                let timeline_span = match (
                    args.video_source_start_frame,
                    args.video_source_end_frame,
                ) {
                    (None, None) => None,
                    (Some(start), Some(end)) => {
                        if args.video_source_media_ref.is_some() {
                            return Err(
                                "timeline span and videoSourceMediaRef are mutually exclusive"
                                    .to_string(),
                            );
                        }
                        let total = snapshot.timeline.total_frames();
                        if start < 0 || end <= start || end > total {
                            return Err(
                                "video source frame range is outside the timeline".to_string()
                            );
                        }
                        Some(PreparedTimelineSpan {
                            timeline: snapshot.timeline.clone(),
                            manifest: snapshot.media.clone(),
                            project_dir: snapshot.project_dir.clone(),
                            start_frame: start,
                            end_frame: end,
                        })
                    }
                    _ => return Err(
                        "videoSourceStartFrame and videoSourceEndFrame must be provided together"
                            .to_string(),
                    ),
                };
                if (timeline_span.is_some() || args.video_source_media_ref.is_some())
                    && !caps
                        .inputs
                        .as_ref()
                        .is_some_and(|inputs| inputs.iter().any(|input| input == "video"))
                {
                    return Err("selected audio model does not support a video source".to_string());
                }
                let references = match args.video_source_media_ref.as_deref() {
                    Some(media_ref) => vec![PreparedReference::whole(
                        resolve_media(&snapshot, media_ref, ClipType::Video)?,
                        "video",
                    )],
                    None => Vec::new(),
                };
                let provider = provider_prefix(&entry.id)?;
                let input = GenerationInput {
                    prompt: prompt.clone(),
                    model: entry.id.clone(),
                    duration: args.duration.unwrap_or_else(|| {
                        timeline_span
                            .as_ref()
                            .map(|span| {
                                ((span.end_frame - span.start_frame) as f64
                                    / span.timeline.fps.max(1) as f64)
                                    .ceil() as i32
                            })
                            .unwrap_or(0)
                    }),
                    aspect_ratio: String::new(),
                    voice: args.voice,
                    lyrics: args.lyrics,
                    style_instructions: args.style_instructions,
                    instrumental: args.instrumental,
                    reference_video_asset_ids: args
                        .video_source_media_ref
                        .clone()
                        .map(|id| vec![id]),
                    source_start_frame: args.video_source_start_frame,
                    source_end_frame: args.video_source_end_frame,
                    ..Default::default()
                };
                let estimated_cost_credits = cost_for_input(entry, &input);
                Ok(PreparedDispatch {
                    plan: PreparedGenerationJob {
                        name: display_name(args.name.as_deref(), &prompt, "Generated audio"),
                        kind: ClipType::Audio,
                        folder_id: args.folder_id,
                        provider: provider.clone(),
                        input,
                        output_count: 1,
                        source_asset_id: args.video_source_media_ref,
                        source_clip_id: None,
                        estimated_cost_credits,
                        created_at: Some(now_apple_reference_seconds()),
                    },
                    references,
                    timeline_span,
                    requires_source_video: false,
                    model_kind: ModelKind::Audio,
                    managed: !configured.contains(&provider) && managed_available,
                })
            }
            GenerationRequest::Upscale(args) => {
                let source = snapshot
                    .media
                    .entries
                    .iter()
                    .find(|entry| entry.id == args.media_ref)
                    .ok_or_else(|| "upscale source asset does not exist".to_string())?;
                if !matches!(source.kind, ClipType::Image | ClipType::Video) {
                    return Err("upscale source must be image or video".to_string());
                }
                let source_trim = validate_source_clip(
                    &snapshot.timeline,
                    args.source_clip_id.as_deref(),
                    Some(&args.media_ref),
                )?;
                let entry = select_model(
                    &catalog,
                    ModelKind::Upscale,
                    args.model.as_deref(),
                    &configured,
                    managed_available,
                )?;
                let UiCapabilities::Upscale(caps) = &entry.ui_capabilities else {
                    return Err("selected model has invalid upscale capabilities".to_string());
                };
                let source_kind = if source.kind == ClipType::Image {
                    "image"
                } else {
                    "video"
                };
                if !caps.supported_types.iter().any(|kind| kind == source_kind) {
                    return Err("selected upscaler does not support the source type".to_string());
                }
                let source_path = resolve_media(&snapshot, &args.media_ref, source.kind)?;
                let provider = provider_prefix(&entry.id)?;
                let input = GenerationInput {
                    prompt: String::new(),
                    model: entry.id.clone(),
                    duration: source.duration.max(0.0).round() as i32,
                    aspect_ratio: String::new(),
                    source_asset_id: Some(args.media_ref.clone()),
                    source_clip_id: args.source_clip_id.clone(),
                    ..Default::default()
                };
                let estimated_cost_credits = cost_for_input(entry, &input);
                Ok(PreparedDispatch {
                    plan: PreparedGenerationJob {
                        name: format!("{} 2x", source.name),
                        kind: source.kind,
                        folder_id: source.folder_id.clone(),
                        provider: provider.clone(),
                        input,
                        output_count: 1,
                        source_asset_id: Some(args.media_ref),
                        source_clip_id: args.source_clip_id,
                        estimated_cost_credits,
                        created_at: Some(now_apple_reference_seconds()),
                    },
                    references: vec![PreparedReference {
                        path: source_path,
                        fallback: source_kind,
                        trim_range: source_trim,
                    }],
                    timeline_span: None,
                    requires_source_video: false,
                    model_kind: ModelKind::Upscale,
                    managed: !configured.contains(&provider) && managed_available,
                })
            }
        }
    }

    async fn run_job(
        self,
        project_epoch: u64,
        project_dir: PathBuf,
        local_job_id: String,
        placeholder_ids: Vec<String>,
        prepared: PreparedDispatch,
        cancel: MediaCancelToken,
    ) {
        let result = self
            .run_job_inner(
                project_epoch,
                &project_dir,
                &local_job_id,
                &placeholder_ids,
                &prepared,
                &cancel,
            )
            .await;
        if let Err(code) = result {
            if cancel.is_cancelled() {
                self.cancel_nonterminal_outputs(project_epoch, &project_dir, &placeholder_ids);
            } else {
                self.fail_nonterminal_outputs(project_epoch, &project_dir, &placeholder_ids, &code);
            }
        }
        self.runtime
            .jobs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&local_job_id);
    }

    async fn run_job_inner(
        &self,
        project_epoch: u64,
        project_dir: &Path,
        local_job_id: &str,
        placeholder_ids: &[String],
        prepared: &PreparedDispatch,
        cancel: &MediaCancelToken,
    ) -> Result<(), String> {
        cancelled(cancel)?;
        let client = self
            .clients
            .build(&prepared.plan.provider, prepared.managed)?;
        let mut references = prepared.references.clone();
        let timeline_cleanup = if let Some(span) = prepared.timeline_span.as_ref() {
            std::fs::create_dir_all(&self.staging_root)
                .map_err(|_| "GENERATION_SOURCE_PREPROCESS_FAILED".to_string())?;
            let destination = self.staging_root.join(format!(
                "{local_job_id}-{}.timeline.mp4",
                uuid::Uuid::new_v4()
            ));
            let timeline = span.timeline.clone();
            let manifest = span.manifest.clone();
            let project_dir = span.project_dir.clone();
            let start_frame = span.start_frame;
            let end_frame = span.end_frame;
            let output = destination.clone();
            // Export has its own final success boundary; committing its child
            // token must not make the later upload/download workflow immune
            // to cancellation on the parent generation token.
            let export_cancel = cancel.child();
            tokio::task::spawn_blocking(move || {
                crate::export::run_export_with_control(
                    &timeline,
                    &manifest,
                    &project_dir,
                    &crate::export::ExportRequest {
                        out_path: output.to_string_lossy().into_owned(),
                        codec: crate::export::ExportCodec::default(),
                        quality: crate::export::ExportQuality::default(),
                    },
                    crate::export::ExportRunOptions {
                        external_cancel: Some(export_cancel),
                        frame_range: Some((start_frame, end_frame)),
                        ..crate::export::ExportRunOptions::default()
                    },
                )
            })
            .await
            .map_err(|_| "GENERATION_SOURCE_PREPROCESS_FAILED".to_string())?
            .map_err(|error| {
                if error == crate::export::CANCELLED_SENTINEL {
                    "GENERATION_CANCELLED".to_string()
                } else {
                    "GENERATION_SOURCE_PREPROCESS_FAILED".to_string()
                }
            })?;
            references.push(PreparedReference::whole(destination.clone(), "video"));
            Some(StagedCleanup::new(destination))
        } else {
            None
        };
        let mut uploaded = Vec::with_capacity(references.len());
        for reference in &references {
            cancelled(cancel)?;
            let staged_trim = if let Some((start, end)) = reference.trim_range {
                let destination = self
                    .staging_root
                    .join(format!("{local_job_id}-{}.trim.mp4", uuid::Uuid::new_v4()));
                let source = reference.path.clone();
                let output = destination.clone();
                let trim_cancel = cancel.clone();
                tokio::task::spawn_blocking(move || {
                    opentake_media::trim_video_range(&source, &output, start, end, &trim_cancel)
                })
                .await
                .map_err(|_| "GENERATION_SOURCE_PREPROCESS_FAILED".to_string())?
                .map_err(|error| {
                    if matches!(error, opentake_media::MediaError::Cancelled) {
                        "GENERATION_CANCELLED".to_string()
                    } else {
                        "GENERATION_SOURCE_PREPROCESS_FAILED".to_string()
                    }
                })?;
                Some(destination)
            } else {
                None
            };
            let upload_path = staged_trim.as_deref().unwrap_or(&reference.path);
            let content_type = opentake_gen::content_type_for(upload_path, reference.fallback);
            let uploaded_url = if prepared.managed {
                client.upload_reference(upload_path, &content_type).await
            } else {
                client
                    .upload_reference_via(&prepared.plan.provider, upload_path, &content_type)
                    .await
            };
            if let Some(path) = staged_trim {
                let _ = std::fs::remove_file(path);
            }
            let uploaded_url = uploaded_url.map_err(|error| {
                generation_provider_error_code(&error, "GENERATION_REFERENCE_UPLOAD_FAILED")
            })?;
            uploaded.push(uploaded_url);
        }
        drop(timeline_cleanup);
        cancelled(cancel)?;
        let params = build_params(
            &prepared.plan.input,
            &uploaded,
            prepared.model_kind,
            prepared.requires_source_video,
        );
        let provider_job_id = if prepared.managed {
            client
                .submit(&prepared.plan.input.model, params, Some(local_job_id))
                .await
        } else {
            client.submit_byok(&prepared.plan.input.model, params).await
        }
        .map_err(|error| generation_provider_error_code(&error, "GENERATION_SUBMIT_FAILED"))?;
        self.core
            .update_generation_job_for_project(
                project_epoch,
                project_dir,
                local_job_id,
                GenerationStateUpdate {
                    status: GenerationJobStatus::Generating,
                    progress: Some(0.15),
                    error_code: None,
                    provider_job_id: Some(provider_job_id.clone()),
                    cost_credits: None,
                    created_at: Some(now_apple_reference_seconds()),
                },
            )
            .map_err(|_| "GENERATION_STATE_PERSIST_FAILED".to_string())?;

        self.watch_and_finalize(
            project_epoch,
            project_dir,
            local_job_id,
            placeholder_ids,
            client,
            &provider_job_id,
            cancel,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn watch_and_finalize(
        &self,
        project_epoch: u64,
        project_dir: &Path,
        local_job_id: &str,
        placeholder_ids: &[String],
        client: GenClient,
        provider_job_id: &str,
        cancel: &MediaCancelToken,
    ) -> Result<(), String> {
        let stream = client.watch(provider_job_id);
        futures_util::pin_mut!(stream);
        loop {
            cancelled(cancel)?;
            let next = tokio::select! {
                item = stream.next() => item,
                () = wait_for_cancel(cancel) => return Err("GENERATION_CANCELLED".to_string()),
            };
            let job = next
                .ok_or_else(|| "GENERATION_PROVIDER_STREAM_ENDED".to_string())?
                .map_err(|error| {
                    generation_provider_error_code(&error, "GENERATION_PROVIDER_POLL_FAILED")
                })?;
            match job.status {
                JobStatus::Queued => {}
                JobStatus::Running => {
                    let _ = self.core.update_generation_job_for_project(
                        project_epoch,
                        project_dir,
                        local_job_id,
                        GenerationStateUpdate {
                            status: GenerationJobStatus::Generating,
                            progress: Some(0.5),
                            error_code: None,
                            provider_job_id: Some(provider_job_id.to_string()),
                            cost_credits: None,
                            created_at: Some(now_apple_reference_seconds()),
                        },
                    );
                }
                JobStatus::Failed => return Err("GENERATION_PROVIDER_FAILED".to_string()),
                JobStatus::Succeeded => {
                    self.core
                        .update_generation_job_for_project(
                            project_epoch,
                            project_dir,
                            local_job_id,
                            GenerationStateUpdate {
                                status: GenerationJobStatus::Downloading,
                                progress: Some(0.8),
                                error_code: None,
                                provider_job_id: Some(provider_job_id.to_string()),
                                cost_credits: job.cost_credits,
                                created_at: Some(now_apple_reference_seconds()),
                            },
                        )
                        .map_err(|_| "GENERATION_STATE_PERSIST_FAILED".to_string())?;
                    let store = TauriFinalizationStore {
                        bridge: self.clone(),
                        project_epoch,
                        project_dir: project_dir.to_path_buf(),
                    };
                    let staging_root = self.staging_root.clone();
                    let urls = job.result_urls.unwrap_or_default();
                    let terminal_job_id = local_job_id.to_string();
                    let terminal_placeholder_ids = placeholder_ids.to_vec();
                    let download_cancel = cancel.clone();
                    tokio::task::spawn_blocking(move || {
                        let downloader =
                            SecureResultDownloader::new(staging_root, download_cancel)?;
                        finalize_terminal_outputs(
                            &store,
                            &downloader,
                            &terminal_job_id,
                            &terminal_placeholder_ids,
                            &urls,
                        )
                    })
                    .await
                    .map_err(|_| "GENERATION_FINALIZE_TASK_FAILED".to_string())?
                    .map_err(|error| {
                        if error == "GENERATION_CANCELLED" {
                            error
                        } else {
                            "GENERATION_FINALIZE_FAILED".to_string()
                        }
                    })?;
                    return Ok(());
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_recovered_job(
        self,
        project_epoch: u64,
        project_dir: PathBuf,
        local_job_id: String,
        placeholder_ids: Vec<String>,
        provider: String,
        managed: bool,
        provider_job_id: String,
        cancel: MediaCancelToken,
    ) {
        let result = match self.clients.build(&provider, managed) {
            Ok(client) => {
                self.watch_and_finalize(
                    project_epoch,
                    &project_dir,
                    &local_job_id,
                    &placeholder_ids,
                    client,
                    &provider_job_id,
                    &cancel,
                )
                .await
            }
            Err(_) => Err("GENERATION_RECOVERY_AUTH_UNAVAILABLE".to_string()),
        };
        if let Err(code) = result {
            if cancel.is_cancelled() {
                self.cancel_nonterminal_outputs(project_epoch, &project_dir, &placeholder_ids);
            } else {
                self.fail_nonterminal_outputs(project_epoch, &project_dir, &placeholder_ids, &code);
            }
        }
        self.runtime
            .jobs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&local_job_id);
    }
}

impl GenerationBridge for TauriGenerationBridge {
    fn can_generate(&self) -> bool {
        !self.configured_byok_prefixes().is_empty() || self.has_managed_credential()
    }

    fn submit(
        &self,
        request: GenerationRequest,
        cancel: &MediaCancelToken,
    ) -> Result<GenerationSubmission, String> {
        cancelled(cancel)?;
        let admission = self.admission.begin_activity()?;
        let prepared = self.prepare(request)?;
        let snapshot = self.core.runtime_snapshot();
        let project_dir = snapshot
            .project_dir
            .clone()
            .ok_or_else(|| "Save the project before starting generation".to_string())?;
        let committed = self
            .core
            .begin_generation_job_for_project(
                snapshot.project_epoch,
                &project_dir,
                prepared.plan.clone(),
            )
            .map_err(|error| error.to_string())?;
        let background_cancel = MediaCancelToken::new();
        self.runtime
            .jobs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                committed.job_id.clone(),
                ActiveGenerationJob {
                    cancel: background_cancel.clone(),
                    _admission: admission,
                },
            );
        let bridge = self.clone();
        let job_id = committed.job_id.clone();
        let placeholder_ids = committed.placeholder_asset_ids.clone();
        tauri::async_runtime::spawn(async move {
            bridge
                .run_job(
                    snapshot.project_epoch,
                    project_dir,
                    job_id,
                    placeholder_ids,
                    prepared,
                    background_cancel,
                )
                .await;
        });
        Ok(GenerationSubmission {
            job_id: committed.job_id,
            placeholder_asset_ids: committed.placeholder_asset_ids,
            status: "queued".to_string(),
        })
    }
}

#[derive(Clone)]
struct TauriFinalizationStore {
    bridge: TauriGenerationBridge,
    project_epoch: u64,
    project_dir: PathBuf,
}

impl GenerationFinalizationStore for TauriFinalizationStore {
    fn claim_terminal(&self, job_id: &str) -> Result<bool, String> {
        if self
            .bridge
            .runtime
            .completed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(job_id)
        {
            return Ok(false);
        }
        Ok(self
            .bridge
            .runtime
            .terminal_leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(job_id.to_string()))
    }

    fn release_terminal(&self, job_id: &str) -> Result<(), String> {
        self.bridge
            .runtime
            .terminal_leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(job_id);
        Ok(())
    }

    fn finalize_output(
        &self,
        asset_id: &str,
        artifact: DownloadedGenerationArtifact,
    ) -> Result<(), String> {
        let snapshot = self.bridge.core.runtime_snapshot();
        let entry = snapshot
            .media
            .entries
            .iter()
            .find(|entry| entry.id == asset_id)
            .ok_or_else(|| "generation placeholder disappeared".to_string())?;
        let probe = self
            .bridge
            .engine
            .probe(&artifact.path)
            .map_err(|_| "downloaded generation result could not be probed".to_string())?;
        let actual_kind = if probe.has_video {
            if probe.duration_secs > 0.0 {
                ClipType::Video
            } else {
                ClipType::Image
            }
        } else if probe.has_audio {
            ClipType::Audio
        } else {
            return Err("downloaded generation result has no supported stream".to_string());
        };
        if actual_kind != entry.kind {
            return Err("downloaded generation result has the wrong media type".to_string());
        }
        if let Some(source_id) = entry
            .generation_input
            .as_ref()
            .and_then(|input| input.source_asset_id.as_deref())
        {
            let source = snapshot
                .media
                .entries
                .iter()
                .find(|source| source.id == source_id)
                .ok_or_else(|| "upscale source disappeared".to_string())?;
            if let (Some(source_width), Some(source_height), Some(width), Some(height)) = (
                source.source_width,
                source.source_height,
                probe.width,
                probe.height,
            ) {
                if width as i32 != source_width.saturating_mul(2)
                    || height as i32 != source_height.saturating_mul(2)
                {
                    return Err(
                        "upscale result is not exactly 2x the source dimensions".to_string()
                    );
                }
            }
        }
        let extension = result_extension(
            &artifact.media_type,
            actual_kind,
            probe.format_name.as_deref(),
        )?;
        let leaf = format!("{asset_id}.{extension}");
        let mut source = std::fs::File::open(&artifact.path).map_err(|error| error.to_string())?;
        self.bridge
            .core
            .finalize_generation_output_with_media_for_project(
                self.project_epoch,
                &self.project_dir,
                PreparedGenerationOutput {
                    asset_id: asset_id.to_string(),
                    relative_path: format!("media/{leaf}"),
                    probe: ProbedMedia {
                        duration_secs: probe.duration_secs,
                        width: probe.width.map(|value| value as i32),
                        height: probe.height.map(|value| value as i32),
                        fps: probe.fps,
                        has_audio: probe.has_audio,
                        color: probe.color.clone(),
                    },
                    created_at: Some(now_apple_reference_seconds()),
                },
                &leaf,
                artifact.byte_size,
                &mut source,
            )
            .map_err(|error| error.to_string())?;
        let _ = std::fs::remove_file(&artifact.path);
        Ok(())
    }

    fn fail_output(&self, asset_id: &str, code: &str) -> Result<(), String> {
        self.bridge
            .core
            .fail_generation_output_for_project(
                self.project_epoch,
                &self.project_dir,
                asset_id,
                code,
                Some(now_apple_reference_seconds()),
            )
            .map_err(|error| error.to_string())
    }

    fn complete_job(&self, job_id: &str, _succeeded: usize, _failed: usize) -> Result<(), String> {
        self.bridge
            .runtime
            .terminal_leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(job_id);
        self.bridge
            .runtime
            .completed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(job_id.to_string());
        Ok(())
    }
}

struct SecureResultDownloader {
    client: reqwest::blocking::Client,
    staging_root: PathBuf,
    cancel: MediaCancelToken,
}

impl SecureResultDownloader {
    fn new(staging_root: PathBuf, cancel: MediaCancelToken) -> Result<Self, String> {
        std::fs::create_dir_all(&staging_root)
            .map_err(|_| "generation staging directory is unavailable".to_string())?;
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(10 * 60))
            .build()
            .map_err(|_| "generation result client initialization failed".to_string())?;
        Ok(Self {
            client,
            staging_root,
            cancel,
        })
    }

    fn write_staging(
        &self,
        asset_id: &str,
        media_type: String,
        bytes: &[u8],
    ) -> Result<DownloadedGenerationArtifact, String> {
        if bytes.len() as u64 > RESULT_BYTES_MAX {
            return Err("generation result exceeds the download limit".to_string());
        }
        let path = self
            .staging_root
            .join(format!("{asset_id}-{}.download", uuid::Uuid::new_v4()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = options
            .open(&path)
            .map_err(|_| "generation staging file creation failed".to_string())?;
        let cleanup = StagedCleanup::new(path.clone());
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| "generation staging write failed".to_string())?;
        cleanup.preserve();
        Ok(DownloadedGenerationArtifact {
            path,
            media_type,
            byte_size: bytes.len() as u64,
        })
    }
}

impl GenerationArtifactDownloader for SecureResultDownloader {
    fn download(
        &self,
        asset_id: &str,
        raw_url: &str,
    ) -> Result<DownloadedGenerationArtifact, String> {
        cancelled(&self.cancel)?;
        if raw_url.starts_with("data:") {
            if raw_url.len() > DATA_URL_ENCODED_MAX {
                return Err("generation data URL exceeds the download limit".to_string());
            }
            let (header, encoded) = raw_url
                .split_once(',')
                .ok_or_else(|| "generation data URL is malformed".to_string())?;
            let media_type = header
                .strip_prefix("data:")
                .and_then(|value| value.strip_suffix(";base64"))
                .filter(|value| {
                    value.starts_with("image/")
                        || value.starts_with("audio/")
                        || value.starts_with("video/")
                })
                .ok_or_else(|| "generation data URL media type is unsupported".to_string())?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|_| "generation data URL base64 is invalid".to_string())?;
            return self.write_staging(asset_id, media_type.to_string(), &bytes);
        }

        let mut current = validate_result_https_url(raw_url)?;
        for redirect_count in 0..=RESULT_REDIRECT_MAX {
            cancelled(&self.cancel)?;
            let mut response = self
                .client
                .get(current.clone())
                .send()
                .map_err(|_| "generation result download failed".to_string())?;
            if response
                .remote_addr()
                .is_some_and(|address| !is_public_result_ip(address.ip()))
            {
                return Err("generation result resolved to a private address".to_string());
            }
            if response.status().is_redirection() {
                if redirect_count == RESULT_REDIRECT_MAX {
                    return Err("generation result exceeded redirect limit".to_string());
                }
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| "generation result redirect is invalid".to_string())?;
                current = validate_result_https_url(
                    current
                        .join(location)
                        .map_err(|_| "generation result redirect is invalid".to_string())?
                        .as_str(),
                )?;
                continue;
            }
            if !response.status().is_success() {
                return Err("generation result download returned an error".to_string());
            }
            if response
                .content_length()
                .is_some_and(|length| length > RESULT_BYTES_MAX)
            {
                return Err("generation result exceeds the download limit".to_string());
            }
            let media_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(';').next())
                .unwrap_or("application/octet-stream")
                .trim()
                .to_ascii_lowercase();
            let path = self
                .staging_root
                .join(format!("{asset_id}-{}.download", uuid::Uuid::new_v4()));
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|_| "generation staging file creation failed".to_string())?;
            let cleanup = StagedCleanup::new(path.clone());
            let mut total = 0_u64;
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                cancelled(&self.cancel)?;
                let count = response
                    .read(&mut buffer)
                    .map_err(|_| "generation result stream failed".to_string())?;
                if count == 0 {
                    break;
                }
                total = total.saturating_add(count as u64);
                if total > RESULT_BYTES_MAX {
                    return Err("generation result exceeds the download limit".to_string());
                }
                file.write_all(&buffer[..count])
                    .map_err(|_| "generation staging write failed".to_string())?;
            }
            file.sync_all()
                .map_err(|_| "generation staging write failed".to_string())?;
            cleanup.preserve();
            return Ok(DownloadedGenerationArtifact {
                path,
                media_type,
                byte_size: total,
            });
        }
        Err("generation result download failed".to_string())
    }
}

/// Reuse the production generation downloader for advanced provider workflows.
/// The destination must be a fresh explicit file path chosen by the caller.
pub(crate) fn secure_download_generation_result(
    staging_root: PathBuf,
    cancel: MediaCancelToken,
    raw_url: &str,
    destination: &Path,
) -> Result<(String, u64), String> {
    let downloader = SecureResultDownloader::new(staging_root, cancel)?;
    let artifact = downloader.download("advanced", raw_url)?;
    let mut source = std::fs::File::open(&artifact.path)
        .map_err(|_| "generation staging result disappeared".to_string())?;
    let mut destination_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| "generated destination already exists or is unavailable".to_string())?;
    let copy_result =
        std::io::copy(&mut source, &mut destination_file).and_then(|_| destination_file.sync_all());
    let _ = std::fs::remove_file(&artifact.path);
    if copy_result.is_err() {
        let _ = std::fs::remove_file(destination);
        return Err("generated result could not be committed to staging".to_string());
    }
    Ok((artifact.media_type, artifact.byte_size))
}

fn build_client(provider: &str, managed: bool) -> Result<GenClient, String> {
    if managed {
        let (backend, token) = crate::account::generation_credential()?
            .ok_or_else(|| "managed generation credential is unavailable".to_string())?;
        let base = reqwest::Url::parse(&(backend + "/"))
            .map_err(|_| "managed generation backend is invalid".to_string())?;
        return Ok(GenClient::managed(base, Arc::new(StaticToken(token))));
    }
    let store = KeyringStore::new();
    let key = match provider {
        "fal" => ProviderKey::Fal,
        "replicate" => ProviderKey::Replicate,
        "openai" => ProviderKey::OpenAI,
        "elevenlabs" => ProviderKey::ElevenLabs,
        _ => return Err("generation provider is unsupported".to_string()),
    };
    let secret = (&store as &dyn KeyStore)
        .load_key(key)
        .map_err(|_| "generation provider key could not be loaded".to_string())?
        .ok_or_else(|| "generation provider key is not configured".to_string())?;
    let transport = Arc::new(ReqwestTransport::new());
    let registry = match provider {
        "fal" => ProviderRegistry::new().with(Arc::new(FalAdapter::new(transport, secret))),
        "replicate" => {
            ProviderRegistry::new().with(Arc::new(ReplicateAdapter::new(transport, secret)))
        }
        "openai" => ProviderRegistry::new().with(Arc::new(OpenAiAdapter::new(transport, secret))),
        "elevenlabs" => {
            ProviderRegistry::new().with(Arc::new(ElevenLabsAdapter::new(transport, secret)))
        }
        _ => unreachable!(),
    };
    Ok(GenClient::byok(registry, Catalog::builtin()))
}

fn generation_provider_error_code(error: &GenError, fallback: &str) -> String {
    match error {
        GenError::Unauthenticated | GenError::NotConfigured => "GENERATION_AUTH_FAILED",
        GenError::InsufficientCredits(_) => "GENERATION_INSUFFICIENT_CREDITS",
        GenError::Api { status: 429, .. } => "GENERATION_RATE_LIMITED",
        _ => fallback,
    }
    .to_string()
}

fn select_model<'a>(
    catalog: &'a Catalog,
    kind: ModelKind,
    requested: Option<&str>,
    configured: &BTreeSet<String>,
    managed: bool,
) -> Result<&'a CatalogEntry, String> {
    if let Some(requested) = requested {
        let entry = catalog
            .by_id(requested)
            .ok_or_else(|| "generation model does not exist".to_string())?;
        if entry.kind != kind {
            return Err("generation model has the wrong media kind".to_string());
        }
        let provider = provider_prefix(&entry.id)?;
        if !managed && !configured.contains(&provider) {
            return Err("selected model provider is not configured".to_string());
        }
        return Ok(entry);
    }
    catalog
        .entries()
        .iter()
        .find(|entry| {
            entry.kind == kind
                && (managed
                    || provider_prefix(&entry.id)
                        .ok()
                        .is_some_and(|provider| configured.contains(&provider)))
        })
        .ok_or_else(|| "no configured provider supports this generation type".to_string())
}

fn resolve_media(
    snapshot: &opentake_core::ProjectRuntimeSnapshot,
    media_ref: &str,
    expected_kind: ClipType,
) -> Result<PathBuf, String> {
    let entry = snapshot
        .media
        .entries
        .iter()
        .find(|entry| entry.id == media_ref)
        .ok_or_else(|| format!("referenced media does not exist: {media_ref}"))?;
    if entry.kind != expected_kind {
        return Err(format!("referenced media has the wrong type: {media_ref}"));
    }
    let path = MediaResolver::new(&snapshot.media, snapshot.project_dir.as_deref())
        .expected_path(media_ref)
        .ok_or_else(|| format!("referenced media cannot be resolved: {media_ref}"))?;
    if !path.is_file() {
        return Err(format!("referenced media is offline: {media_ref}"));
    }
    Ok(path)
}

fn validate_source_clip(
    timeline: &Timeline,
    clip_id: Option<&str>,
    media_ref: Option<&str>,
) -> Result<Option<(f64, f64)>, String> {
    let Some(clip_id) = clip_id else {
        return Ok(None);
    };
    let clip = timeline
        .tracks
        .iter()
        .flat_map(|track| &track.clips)
        .find(|clip| clip.id == clip_id)
        .ok_or_else(|| "sourceClipId does not exist".to_string())?;
    if media_ref.is_some_and(|media_ref| clip.media_ref != media_ref) {
        return Err("sourceClipId does not reference the requested media".to_string());
    }
    if timeline.fps <= 0 || clip.duration_frames <= 0 {
        return Err("sourceClipId has no valid visible source range".to_string());
    }
    let start = clip.trim_start_frame.max(0) as f64 / timeline.fps as f64;
    let consumed = clip.source_frames_consumed();
    if consumed <= 0 {
        return Err("sourceClipId has no valid visible source range".to_string());
    }
    Ok(Some((start, start + consumed as f64 / timeline.fps as f64)))
}

fn validate_choice(
    field: &str,
    value: Option<&str>,
    allowed: Option<&[String]>,
) -> Result<(), String> {
    if let (Some(value), Some(allowed)) = (value, allowed) {
        if !allowed.is_empty() && !allowed.iter().any(|candidate| candidate == value) {
            return Err(format!("{field} is not supported by the selected model"));
        }
    }
    Ok(())
}

fn validate_reference_count(label: &str, count: usize, max: u32) -> Result<(), String> {
    if count > max as usize {
        Err(format!(
            "too many {label} references for the selected model"
        ))
    } else {
        Ok(())
    }
}

fn provider_prefix(model: &str) -> Result<String, String> {
    ModelRoute::parse(model)
        .map(|route| route.prefix)
        .map_err(|_| "generation model id is invalid".to_string())
}

fn display_name(requested: Option<&str>, prompt: &str, fallback: &str) -> String {
    requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            let value = prompt.trim().chars().take(30).collect::<String>();
            (!value.is_empty()).then_some(value)
        })
        .unwrap_or_else(|| fallback.to_string())
}

fn result_extension(
    media_type: &str,
    kind: ClipType,
    format_name: Option<&str>,
) -> Result<&'static str, String> {
    if let Some(format) = format_name {
        let formats = format.split(',').collect::<BTreeSet<_>>();
        let detected = match kind {
            ClipType::Image if formats.contains("png_pipe") => Some("png"),
            ClipType::Image if formats.contains("jpeg_pipe") || formats.contains("image2") => {
                Some("jpg")
            }
            ClipType::Image if formats.contains("webp_pipe") => Some("webp"),
            ClipType::Video if formats.contains("mov") || formats.contains("mp4") => Some("mp4"),
            ClipType::Audio if formats.contains("mp3") => Some("mp3"),
            ClipType::Audio if formats.contains("wav") => Some("wav"),
            ClipType::Audio if formats.contains("mov") || formats.contains("mp4") => Some("m4a"),
            _ => None,
        };
        if let Some(extension) = detected {
            return Ok(extension);
        }
    }
    match media_type {
        "image/png" => Ok("png"),
        "image/jpeg" | "image/jpg" => Ok("jpg"),
        "image/webp" => Ok("webp"),
        "video/mp4" => Ok("mp4"),
        "video/quicktime" => Ok("mov"),
        "audio/mpeg" => Ok("mp3"),
        "audio/wav" | "audio/x-wav" => Ok("wav"),
        "audio/mp4" => Ok("m4a"),
        "application/octet-stream" => match kind {
            ClipType::Image => Ok("png"),
            ClipType::Video => Ok("mp4"),
            ClipType::Audio => Ok("mp3"),
            ClipType::Text | ClipType::Lottie => {
                Err("generated media type is unsupported".to_string())
            }
        },
        _ => Err("generation result content type is unsupported".to_string()),
    }
}

fn validate_result_https_url(raw: &str) -> Result<reqwest::Url, String> {
    let url =
        reqwest::Url::parse(raw).map_err(|_| "generation result URL is invalid".to_string())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some_and(|port| port != 443)
    {
        return Err("generation result URL is not an accepted HTTPS URL".to_string());
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| !is_public_result_ip(address))
    {
        return Err("generation result URL host is not public".to_string());
    }
    Ok(url)
}

fn is_public_result_ip(address: std::net::IpAddr) -> bool {
    match address {
        std::net::IpAddr::V4(ip) => {
            let [a, b, _, _] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
                || ip.is_multicast()
                || a == 0
                || (a == 100 && (64..=127).contains(&b))
                || (a == 198 && (18..=19).contains(&b)))
        }
        std::net::IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return is_public_result_ip(std::net::IpAddr::V4(mapped));
            }
            let octets = ip.octets();
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_multicast()
                || octets[..4] == [0x20, 0x01, 0x0d, 0xb8]
                || (octets[0] == 0xfe && octets[1] & 0xc0 == 0xc0))
        }
    }
}

fn cancelled(cancel: &MediaCancelToken) -> Result<(), String> {
    if cancel.is_cancelled() {
        Err("GENERATION_CANCELLED".to_string())
    } else {
        Ok(())
    }
}

async fn wait_for_cancel(cancel: &MediaCancelToken) {
    while !cancel.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn now_apple_reference_seconds() -> f64 {
    const APPLE_REFERENCE_UNIX_OFFSET: f64 = 978_307_200.0;
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64() - APPLE_REFERENCE_UNIX_OFFSET)
        .unwrap_or(0.0)
}

#[tauri::command]
pub fn generation_cancel(
    bridge: tauri::State<'_, Arc<TauriGenerationBridge>>,
    job_id: String,
) -> Result<bool, String> {
    Ok(bridge.cancel(&job_id))
}

#[tauri::command]
pub fn generation_retry(
    bridge: tauri::State<'_, Arc<TauriGenerationBridge>>,
    job_id: String,
    cost_authorized: bool,
) -> Result<GenerationSubmission, String> {
    bridge.retry(&job_id, cost_authorized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    use image::{DynamicImage, ImageFormat};
    use opentake_agent::mcp::core_handle::AppCoreHandle;
    use opentake_agent::mcp::dispatch::Dispatcher;
    use opentake_agent::plugin::registry::PluginRegistry;
    use opentake_gen::{AuthMode, FalAdapter, HttpResponse, Method, MockTransport};
    use opentake_project::{GenerationLog, Project};
    use serde_json::json;
    use std::sync::RwLock;

    #[derive(Clone)]
    struct FixtureClients {
        client: GenClient,
    }

    impl GenerationClientFactory for FixtureClients {
        fn configured_byok_prefixes(&self) -> BTreeSet<String> {
            BTreeSet::from([
                "fal".to_string(),
                "openai".to_string(),
                "replicate".to_string(),
            ])
        }

        fn has_managed_credential(&self) -> bool {
            false
        }

        fn build(&self, _provider: &str, _managed: bool) -> Result<GenClient, String> {
            Ok(self.client.clone())
        }
    }

    fn fixture_client_with_interval(mock: &MockTransport, interval: Duration) -> GenClient {
        let transport = Arc::new(mock.clone());
        let fal = FalAdapter::new(transport.clone(), "fixture-secret").with_base("https://mockfal");
        let openai =
            OpenAiAdapter::new(transport.clone(), "fixture-secret").with_base("https://mockoai/v1");
        let replicate =
            ReplicateAdapter::new(transport, "fixture-secret").with_base("https://mockrep/v1");
        GenClient::with_transport(
            AuthMode::Byok {
                registry: ProviderRegistry::new()
                    .with(Arc::new(fal))
                    .with(Arc::new(openai))
                    .with(Arc::new(replicate)),
                catalog: Catalog::builtin(),
            },
            Arc::new(mock.clone()),
        )
        .with_poll_interval(interval)
    }

    fn fixture_client(mock: &MockTransport) -> GenClient {
        fixture_client_with_interval(mock, Duration::ZERO)
    }

    fn saved_core() -> (tempfile::TempDir, PathBuf, AppCore) {
        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("Generation.opentake");
        let mut project = Project::new(&bundle);
        project.generation_log = Some(GenerationLog::new());
        project.save().unwrap();
        let core = AppCore::new();
        core.open_project(&bundle).unwrap();
        (temp, bundle, core)
    }

    fn image_plan() -> PreparedGenerationJob {
        PreparedGenerationJob {
            name: "Recovered image".to_string(),
            kind: ClipType::Image,
            folder_id: None,
            provider: "fal".to_string(),
            input: GenerationInput {
                prompt: "fixture".to_string(),
                model: "fal:flux-pro".to_string(),
                duration: 0,
                aspect_ratio: "1:1".to_string(),
                num_images: Some(1),
                ..Default::default()
            },
            output_count: 1,
            source_asset_id: None,
            source_clip_id: None,
            estimated_cost_credits: None,
            created_at: Some(800_000_000.0),
        }
    }

    #[test]
    fn updater_gate_observes_and_cancels_every_active_generation() {
        let (_temp, bundle, core) = saved_core();
        let mock = MockTransport::new();
        let (cache, models) = runtime_dirs(&bundle);
        let bridge = build_bridge_with_clients(
            core,
            cache,
            models,
            Arc::new(FixtureClients {
                client: fixture_client(&mock),
            }),
        );
        let first = MediaCancelToken::new();
        let second = MediaCancelToken::new();
        {
            let mut jobs = bridge
                .runtime
                .jobs
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            jobs.insert(
                "job-1".to_string(),
                ActiveGenerationJob {
                    cancel: first.clone(),
                    _admission: bridge.admission.begin_activity().unwrap(),
                },
            );
            jobs.insert(
                "job-2".to_string(),
                ActiveGenerationJob {
                    cancel: second.clone(),
                    _admission: bridge.admission.begin_activity().unwrap(),
                },
            );
        }

        assert!(bridge.has_active());
        assert_eq!(bridge.cancel_all_active(), 2);
        assert!(first.is_cancelled());
        assert!(second.is_cancelled());

        bridge
            .runtime
            .jobs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
        assert!(!bridge.has_active());
        assert_eq!(bridge.cancel_all_active(), 0);
    }

    #[test]
    fn generation_cannot_submit_after_update_install_claims_admission() {
        let (_temp, bundle, core) = saved_core();
        let mock = MockTransport::new();
        let (cache, models) = runtime_dirs(&bundle);
        let admission = crate::updater::InstallAdmissionGate::default();
        let bridge = build_bridge_with_clients_and_admission(
            core,
            cache,
            models,
            Arc::new(FixtureClients {
                client: fixture_client(&mock),
            }),
            admission.clone(),
        );
        let _install = admission.begin_install().unwrap();

        assert_eq!(
            bridge
                .submit(
                    GenerationRequest::Image(GenerateImageArgs {
                        prompt: "must not submit".to_string(),
                        ..Default::default()
                    }),
                    &MediaCancelToken::new(),
                )
                .unwrap_err(),
            "app update installation is in progress"
        );
        assert!(bridge.runtime.jobs.lock().unwrap().is_empty());
    }

    fn runtime_dirs(bundle: &Path) -> (PathBuf, PathBuf) {
        let root = bundle.parent().unwrap();
        (root.join("cache"), root.join("models"))
    }

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::new_rgba8(width, height)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    fn png_data_url_for(width: u32, height: u32) -> String {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png_bytes(width, height))
        )
    }

    fn png_data_url() -> String {
        png_data_url_for(2, 2)
    }

    fn wav_bytes() -> Vec<u8> {
        let sample_rate = 8_000_u32;
        let sample_count = sample_rate / 10;
        let data_size = sample_count * 2;
        let mut bytes = Vec::with_capacity((44 + data_size) as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        bytes.resize((44 + data_size) as usize, 0);
        bytes
    }

    fn mp4_data_url(directory: &Path) -> String {
        let path = directory.join("generation-fixture.mp4");
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=16x16:d=0.1:r=10",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        let encoded =
            base64::engine::general_purpose::STANDARD.encode(std::fs::read(path).unwrap());
        format!("data:video/mp4;base64,{encoded}")
    }

    fn saved_core_with_source_image() -> (tempfile::TempDir, PathBuf, AppCore) {
        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("Upscale.opentake");
        let mut project = Project::new(&bundle);
        project.generation_log = Some(GenerationLog::new());
        project
            .manifest
            .entries
            .push(opentake_domain::MediaManifestEntry {
                id: "source-image".to_string(),
                name: "source.png".to_string(),
                kind: ClipType::Image,
                source: opentake_domain::MediaSource::Project {
                    relative_path: "media/source.png".to_string(),
                },
                duration: 0.0,
                generation_input: None,
                source_width: Some(2),
                source_height: Some(2),
                source_fps: None,
                has_audio: Some(false),
                color: None,
                proxy: None,
                folder_id: None,
                cached_remote_url: None,
                cached_remote_url_expires_at: None,
            });
        project.save().unwrap();
        std::fs::create_dir_all(bundle.join("media")).unwrap();
        std::fs::write(bundle.join("media/source.png"), png_bytes(2, 2)).unwrap();
        let core = AppCore::new();
        core.open_project(&bundle).unwrap();
        (temp, bundle, core)
    }

    async fn wait_for_ready_model(
        core: &AppCore,
        model: &str,
    ) -> opentake_domain::MediaManifestEntry {
        for _ in 0..150 {
            if let Some(entry) = core.media().entries.into_iter().find(|entry| {
                entry.generation_input.as_ref().is_some_and(|input| {
                    input.model == model && input.status == Some(GenerationJobStatus::Ready)
                })
            }) {
                return entry;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!(
            "generation did not become ready for {model}: {:?}",
            core.media().entries
        );
    }

    #[test]
    fn result_url_validation_rejects_local_and_private_network_targets() {
        assert!(validate_result_https_url("https://cdn.example.com/result.png").is_ok());
        for url in [
            "https://localhost/result.png",
            "https://service.local/result.png",
            "https://127.0.0.1/result.png",
            "https://10.0.0.1/result.png",
            "https://169.254.1.1/result.png",
            "https://[::1]/result.png",
            "https://[fc00::1]/result.png",
            "https://[::ffff:127.0.0.1]/result.png",
            "https://[2001:db8::1]/result.png",
        ] {
            assert!(
                validate_result_https_url(url).is_err(),
                "private result URL accepted: {url}"
            );
        }
    }

    #[test]
    fn restart_before_provider_id_requires_explicit_retry_without_resubmission() {
        let (_temp, bundle, core) = saved_core();
        let committed = core
            .begin_generation_job_for_project(1, &bundle, image_plan())
            .unwrap();
        // Model a real process restart. A second independent AppCore retaining
        // the old bundle at the same time is not a supported runtime state and
        // prevents same-target directory publication on Windows.
        drop(core);
        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();
        let mock = MockTransport::new();
        let (cache, models) = runtime_dirs(&bundle);
        let bridge = build_bridge_with_clients(
            reopened.clone(),
            cache,
            models,
            Arc::new(FixtureClients {
                client: fixture_client(&mock),
            }),
        );

        assert_eq!(bridge.recover_current_project(), 0);
        let persisted = reopened.media();
        let input = persisted
            .entries
            .iter()
            .find(|entry| entry.id == committed.placeholder_asset_ids[0])
            .unwrap()
            .generation_input
            .as_ref()
            .unwrap();
        assert_eq!(input.status, Some(GenerationJobStatus::Failed));
        assert_eq!(
            input.error_code.as_deref(),
            Some("GENERATION_RESTART_RETRY_REQUIRED")
        );
        assert!(mock.calls().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn restart_with_provider_id_resumes_and_finalizes_offline_fixture() {
        let (_temp, bundle, core) = saved_core();
        let runtime = core.runtime_snapshot();
        let committed = core
            .begin_generation_job_for_project(runtime.project_epoch, &bundle, image_plan())
            .unwrap();
        core.update_generation_job_for_project(
            runtime.project_epoch,
            &bundle,
            &committed.job_id,
            GenerationStateUpdate {
                status: GenerationJobStatus::Generating,
                progress: Some(0.25),
                error_code: None,
                provider_job_id: Some("fal::flux-pro|recover-1".to_string()),
                cost_credits: None,
                created_at: Some(800_000_001.0),
            },
        )
        .unwrap();

        // Model a real process restart before opening the same bundle again.
        drop(core);
        let reopened = AppCore::new();
        let reopened_snapshot = reopened.open_project(&bundle).unwrap();
        let mock = MockTransport::new();
        mock.on(
            Method::Get,
            "https://mockfal/flux-pro/requests/recover-1/status",
            200,
            json!({"status": "COMPLETED"}),
        );
        mock.on(
            Method::Get,
            "https://mockfal/flux-pro/requests/recover-1",
            200,
            json!({"images": [{"url": png_data_url()}]}),
        );
        let (cache, models) = runtime_dirs(&bundle);
        let bridge = build_bridge_with_clients(
            reopened.clone(),
            cache,
            models,
            Arc::new(FixtureClients {
                client: fixture_client(&mock),
            }),
        );

        assert_eq!(bridge.recover_current_project(), 1);
        let asset_id = committed.placeholder_asset_ids[0].clone();
        for _ in 0..100 {
            let status = reopened
                .media()
                .entries
                .iter()
                .find(|entry| entry.id == asset_id)
                .and_then(|entry| entry.generation_input.as_ref())
                .and_then(|input| input.status);
            if status == Some(GenerationJobStatus::Ready) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let entry = reopened
            .media()
            .entries
            .into_iter()
            .find(|entry| entry.id == asset_id)
            .unwrap();
        assert_eq!(
            entry
                .generation_input
                .as_ref()
                .and_then(|input| input.status),
            Some(GenerationJobStatus::Ready)
        );
        assert_eq!(entry.source_width, Some(2));
        assert_eq!(entry.source_height, Some(2));
        assert!(MediaResolver::new(&reopened.media(), Some(&bundle))
            .expected_path(&entry.id)
            .unwrap()
            .is_file());
        assert_eq!(
            reopened.project_revision().project_epoch,
            reopened_snapshot.project_epoch
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn production_dispatch_path_persists_and_finalizes_ordered_mock_results() {
        let (_temp, bundle, core) = saved_core();
        let mock = MockTransport::new();
        mock.on(
            Method::Post,
            "https://mockfal/flux-pro",
            200,
            json!({"request_id": "dispatch-1", "status": "IN_QUEUE"}),
        );
        mock.on(
            Method::Get,
            "https://mockfal/flux-pro/requests/dispatch-1/status",
            200,
            json!({"status": "COMPLETED"}),
        );
        mock.on(
            Method::Get,
            "https://mockfal/flux-pro/requests/dispatch-1",
            200,
            json!({"images": [{"url": png_data_url()}, {"url": png_data_url()}]}),
        );
        let (cache, models) = runtime_dirs(&bundle);
        let bridge = build_bridge_with_clients(
            core.clone(),
            cache,
            models,
            Arc::new(FixtureClients {
                client: fixture_client(&mock),
            }),
        );
        let dispatcher = Dispatcher::with_bridges(
            Arc::new(AppCoreHandle::new(core.clone())),
            Arc::new(RwLock::new(PluginRegistry::new())),
            None,
            Some(bridge),
        );

        let unauthorized = dispatcher.dispatch(
            "generate_image",
            json!({
                "costAuthorized": false,
                "prompt": "ordered fixture",
                "model": "fal:flux-pro",
                "numImages": 2
            }),
        );
        assert!(unauthorized.is_error);
        assert!(mock.calls().is_empty());

        let accepted = dispatcher.dispatch(
            "generate_image",
            json!({
                "costAuthorized": true,
                "prompt": "ordered fixture",
                "model": "fal:flux-pro",
                "aspectRatio": "1:1",
                "numImages": 2
            }),
        );
        assert!(!accepted.is_error, "{}", accepted.text_joined());
        for _ in 0..100 {
            let generated = core
                .media()
                .entries
                .into_iter()
                .filter(|entry| entry.generation_input.is_some())
                .collect::<Vec<_>>();
            if generated.len() == 2
                && generated.iter().all(|entry| {
                    entry
                        .generation_input
                        .as_ref()
                        .and_then(|input| input.status)
                        == Some(GenerationJobStatus::Ready)
                })
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let mut generated = core
            .media()
            .entries
            .into_iter()
            .filter(|entry| entry.generation_input.is_some())
            .collect::<Vec<_>>();
        generated.sort_by_key(|entry| {
            entry
                .generation_input
                .as_ref()
                .and_then(|input| input.output_index)
        });
        assert_eq!(generated.len(), 2);
        assert!(
            generated.iter().all(|entry| {
                entry
                    .generation_input
                    .as_ref()
                    .and_then(|input| input.status)
                    == Some(GenerationJobStatus::Ready)
                    && entry.source_width == Some(2)
                    && entry.source_height == Some(2)
            }),
            "generated outputs: {generated:?}"
        );
        assert_eq!(
            generated
                .iter()
                .filter_map(|entry| entry
                    .generation_input
                    .as_ref()
                    .and_then(|input| input.output_index))
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert!(generated.iter().all(|entry| {
            MediaResolver::new(&core.media(), Some(&bundle))
                .expected_path(&entry.id)
                .is_some_and(|path| path.is_file())
        }));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn configured_provider_smoke_covers_video_audio_and_upscale() {
        let (video_temp, video_bundle, video_core) = saved_core();
        let video_mock = MockTransport::new();
        video_mock.on(
            Method::Post,
            "https://mockfal/kling-video",
            200,
            json!({"request_id": "video-1", "status": "IN_QUEUE"}),
        );
        video_mock.on(
            Method::Get,
            "https://mockfal/kling-video/requests/video-1/status",
            200,
            json!({"status": "COMPLETED"}),
        );
        video_mock.on(
            Method::Get,
            "https://mockfal/kling-video/requests/video-1",
            200,
            json!({"video": {"url": mp4_data_url(video_temp.path())}}),
        );
        let (cache, models) = runtime_dirs(&video_bundle);
        let video_bridge = build_bridge_with_clients(
            video_core.clone(),
            cache,
            models,
            Arc::new(FixtureClients {
                client: fixture_client(&video_mock),
            }),
        );
        let video_dispatcher = Dispatcher::with_bridges(
            Arc::new(AppCoreHandle::new(video_core.clone())),
            Arc::new(RwLock::new(PluginRegistry::new())),
            None,
            Some(video_bridge),
        );
        let video_result = video_dispatcher.dispatch(
            "generate_video",
            json!({
                "costAuthorized": true,
                "prompt": "fixture video",
                "model": "fal:kling-video",
                "duration": 5,
                "aspectRatio": "16:9",
                "resolution": "720p"
            }),
        );
        assert!(!video_result.is_error, "{}", video_result.text_joined());
        let video = wait_for_ready_model(&video_core, "fal:kling-video").await;
        assert_eq!(video.kind, ClipType::Video);
        assert_eq!(video.source_width, Some(16));
        assert_eq!(video.source_height, Some(16));

        let (_audio_temp, audio_bundle, audio_core) = saved_core();
        let audio_mock = MockTransport::new();
        let mut audio_response = HttpResponse::new(200, wav_bytes());
        audio_response
            .headers
            .push(("Content-Type".to_string(), "audio/wav".to_string()));
        audio_mock.on_raw(
            Method::Post,
            "https://mockoai/v1/audio/speech",
            audio_response,
        );
        let (cache, models) = runtime_dirs(&audio_bundle);
        let audio_bridge = build_bridge_with_clients(
            audio_core.clone(),
            cache,
            models,
            Arc::new(FixtureClients {
                client: fixture_client(&audio_mock),
            }),
        );
        let audio_dispatcher = Dispatcher::with_bridges(
            Arc::new(AppCoreHandle::new(audio_core.clone())),
            Arc::new(RwLock::new(PluginRegistry::new())),
            None,
            Some(audio_bridge),
        );
        let audio_result = audio_dispatcher.dispatch(
            "generate_audio",
            json!({
                "costAuthorized": true,
                "prompt": "fixture speech",
                "model": "openai:tts-1",
                "voice": "alloy"
            }),
        );
        assert!(!audio_result.is_error, "{}", audio_result.text_joined());
        let audio = wait_for_ready_model(&audio_core, "openai:tts-1").await;
        assert_eq!(audio.kind, ClipType::Audio);
        assert_eq!(audio.has_audio, Some(true));
        assert!(audio.duration > 0.0);

        let (_upscale_temp, upscale_bundle, upscale_core) = saved_core_with_source_image();
        let source_before = std::fs::read(upscale_bundle.join("media/source.png")).unwrap();
        let upscale_mock = MockTransport::new();
        upscale_mock.on(
            Method::Post,
            "https://mockrep/v1/files",
            200,
            json!({"urls": {"get": "https://fixtures.invalid/source.png"}}),
        );
        upscale_mock.on(
            Method::Post,
            "https://mockrep/v1/predictions",
            200,
            json!({"id": "upscale-1", "status": "starting"}),
        );
        upscale_mock.on(
            Method::Get,
            "https://mockrep/v1/predictions/upscale-1",
            200,
            json!({"id": "upscale-1", "status": "succeeded", "output": png_data_url_for(4, 4)}),
        );
        let (cache, models) = runtime_dirs(&upscale_bundle);
        let upscale_bridge = build_bridge_with_clients(
            upscale_core.clone(),
            cache,
            models,
            Arc::new(FixtureClients {
                client: fixture_client(&upscale_mock),
            }),
        );
        let upscale_dispatcher = Dispatcher::with_bridges(
            Arc::new(AppCoreHandle::new(upscale_core.clone())),
            Arc::new(RwLock::new(PluginRegistry::new())),
            None,
            Some(upscale_bridge),
        );
        let upscale_result = upscale_dispatcher.dispatch(
            "upscale_media",
            json!({
                "costAuthorized": true,
                "mediaRef": "source-image",
                "model": "replicate:topaz-upscale"
            }),
        );
        assert!(!upscale_result.is_error, "{}", upscale_result.text_joined());
        let upscale = wait_for_ready_model(&upscale_core, "replicate:topaz-upscale").await;
        assert_eq!(upscale.kind, ClipType::Image);
        assert_eq!(upscale.source_width, Some(4));
        assert_eq!(upscale.source_height, Some(4));
        assert_eq!(
            std::fs::read(upscale_bundle.join("media/source.png")).unwrap(),
            source_before
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn production_dispatch_cancel_terminalizes_without_importing_media() {
        let (_temp, bundle, core) = saved_core();
        let mock = MockTransport::new();
        mock.on(
            Method::Post,
            "https://mockfal/flux-pro",
            200,
            json!({"request_id": "cancel-1", "status": "IN_QUEUE"}),
        );
        mock.on(
            Method::Get,
            "https://mockfal/flux-pro/requests/cancel-1/status",
            200,
            json!({"status": "IN_QUEUE"}),
        );
        let (cache, models) = runtime_dirs(&bundle);
        let bridge = build_bridge_with_clients(
            core.clone(),
            cache,
            models,
            Arc::new(FixtureClients {
                client: fixture_client_with_interval(&mock, Duration::from_secs(2)),
            }),
        );
        let dispatcher = Dispatcher::with_bridges(
            Arc::new(AppCoreHandle::new(core.clone())),
            Arc::new(RwLock::new(PluginRegistry::new())),
            None,
            Some(bridge.clone()),
        );
        let accepted = dispatcher.dispatch(
            "generate_image",
            json!({
                "costAuthorized": true,
                "prompt": "cancel fixture",
                "model": "fal:flux-pro",
                "aspectRatio": "1:1"
            }),
        );
        assert!(!accepted.is_error, "{}", accepted.text_joined());
        let (job_id, asset_id) = loop {
            if let Some(entry) = core
                .media()
                .entries
                .into_iter()
                .find(|entry| entry.generation_input.is_some())
            {
                let input = entry.generation_input.as_ref().unwrap();
                if input.provider_job_id.is_some() {
                    break (input.job_id.clone().unwrap(), entry.id);
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        assert!(bridge.cancel(&job_id));
        for _ in 0..100 {
            let status = core
                .media()
                .entries
                .iter()
                .find(|entry| entry.id == asset_id)
                .and_then(|entry| entry.generation_input.as_ref())
                .and_then(|input| input.status);
            if status == Some(GenerationJobStatus::Cancelled) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let entry = core
            .media()
            .entries
            .into_iter()
            .find(|entry| entry.id == asset_id)
            .unwrap();
        assert_eq!(
            entry
                .generation_input
                .as_ref()
                .and_then(|input| input.status),
            Some(GenerationJobStatus::Cancelled)
        );
        assert!(MediaResolver::new(&core.media(), Some(&bundle))
            .expected_path(&entry.id)
            .is_some_and(|path| !path.exists()));
    }

    #[test]
    fn upscale_finalization_is_exactly_two_x_and_preserves_source_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("Upscale.opentake");
        let source_bytes = png_bytes(3, 2);
        let mut project = Project::new(&bundle);
        project
            .manifest
            .entries
            .push(opentake_domain::MediaManifestEntry {
                id: "source-image".to_string(),
                name: "source.png".to_string(),
                kind: ClipType::Image,
                source: opentake_domain::MediaSource::Project {
                    relative_path: "media/source.png".to_string(),
                },
                duration: 0.0,
                generation_input: None,
                source_width: Some(3),
                source_height: Some(2),
                source_fps: None,
                has_audio: Some(false),
                color: None,
                proxy: None,
                folder_id: None,
                cached_remote_url: None,
                cached_remote_url_expires_at: None,
            });
        project.save().unwrap();
        std::fs::create_dir_all(bundle.join("media")).unwrap();
        std::fs::write(bundle.join("media/source.png"), &source_bytes).unwrap();
        let core = AppCore::new();
        let snapshot = core.open_project(&bundle).unwrap();
        let mock = MockTransport::new();
        let (cache, models) = runtime_dirs(&bundle);
        let bridge = build_bridge_with_clients(
            core.clone(),
            cache.clone(),
            models,
            Arc::new(FixtureClients {
                client: fixture_client(&mock),
            }),
        );
        let mut plan = image_plan();
        plan.provider = "replicate".to_string();
        plan.input.model = "replicate:topaz-upscale".to_string();
        plan.source_asset_id = Some("source-image".to_string());
        let committed = core
            .begin_generation_job_for_project(snapshot.project_epoch, &bundle, plan)
            .unwrap();
        core.update_generation_job_for_project(
            snapshot.project_epoch,
            &bundle,
            &committed.job_id,
            GenerationStateUpdate {
                status: GenerationJobStatus::Generating,
                progress: Some(0.5),
                error_code: None,
                provider_job_id: Some("replicate::fixture".to_string()),
                cost_credits: None,
                created_at: None,
            },
        )
        .unwrap();
        core.update_generation_job_for_project(
            snapshot.project_epoch,
            &bundle,
            &committed.job_id,
            GenerationStateUpdate {
                status: GenerationJobStatus::Downloading,
                progress: Some(0.8),
                error_code: None,
                provider_job_id: None,
                cost_credits: None,
                created_at: None,
            },
        )
        .unwrap();
        std::fs::create_dir_all(&cache).unwrap();
        let staged = cache.join("upscale.png");
        let staged_bytes = png_bytes(6, 4);
        std::fs::write(&staged, &staged_bytes).unwrap();
        let store = TauriFinalizationStore {
            bridge: bridge.as_ref().clone(),
            project_epoch: snapshot.project_epoch,
            project_dir: bundle.clone(),
        };
        store
            .finalize_output(
                &committed.placeholder_asset_ids[0],
                DownloadedGenerationArtifact {
                    path: staged,
                    media_type: "image/png".to_string(),
                    byte_size: staged_bytes.len() as u64,
                },
            )
            .unwrap();
        let media = core.media();
        let source = media
            .entries
            .iter()
            .find(|entry| entry.id == "source-image")
            .unwrap();
        let output = media
            .entries
            .iter()
            .find(|entry| entry.id == committed.placeholder_asset_ids[0])
            .unwrap();
        assert_eq!(
            (source.source_width, source.source_height),
            (Some(3), Some(2))
        );
        assert_eq!(
            (output.source_width, output.source_height),
            (Some(6), Some(4))
        );
        assert_eq!(
            std::fs::read(bundle.join("media/source.png")).unwrap(),
            source_bytes
        );
    }

    async fn assert_submit_failure(status: u16, body: serde_json::Value, expected_code: &str) {
        let (_temp, bundle, core) = saved_core();
        let mock = MockTransport::new();
        mock.on(Method::Post, "https://mockfal/flux-pro", status, body);
        let (cache, models) = runtime_dirs(&bundle);
        let bridge = build_bridge_with_clients(
            core.clone(),
            cache,
            models,
            Arc::new(FixtureClients {
                client: fixture_client(&mock),
            }),
        );
        let dispatcher = Dispatcher::with_bridges(
            Arc::new(AppCoreHandle::new(core.clone())),
            Arc::new(RwLock::new(PluginRegistry::new())),
            None,
            Some(bridge),
        );
        let accepted = dispatcher.dispatch(
            "generate_image",
            json!({
                "costAuthorized": true,
                "prompt": "failure fixture",
                "model": "fal:flux-pro",
                "aspectRatio": "1:1"
            }),
        );
        assert!(!accepted.is_error, "{}", accepted.text_joined());
        for _ in 0..100 {
            let terminal = core
                .media()
                .entries
                .iter()
                .find_map(|entry| entry.generation_input.as_ref())
                .is_some_and(|input| input.status == Some(GenerationJobStatus::Failed));
            if terminal {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let input = core
            .media()
            .entries
            .into_iter()
            .find_map(|entry| entry.generation_input)
            .unwrap();
        assert_eq!(input.status, Some(GenerationJobStatus::Failed));
        assert_eq!(input.error_code.as_deref(), Some(expected_code));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn production_dispatch_maps_auth_and_rate_limit_to_safe_fixed_codes() {
        assert_submit_failure(
            401,
            json!({"error": {"code": "unauthenticated", "message": "private"}}),
            "GENERATION_AUTH_FAILED",
        )
        .await;
        assert_submit_failure(
            429,
            json!({"error": {"code": "rate_limited", "message": "private"}}),
            "GENERATION_RATE_LIMITED",
        )
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn retry_requires_fresh_cost_authorization_and_creates_a_new_job() {
        let (_temp, bundle, core) = saved_core();
        let runtime = core.runtime_snapshot();
        let failed = core
            .begin_generation_job_for_project(runtime.project_epoch, &bundle, image_plan())
            .unwrap();
        core.update_generation_job_for_project(
            runtime.project_epoch,
            &bundle,
            &failed.job_id,
            GenerationStateUpdate {
                status: GenerationJobStatus::Failed,
                progress: None,
                error_code: Some("GENERATION_PROVIDER_FAILED".to_string()),
                provider_job_id: None,
                cost_credits: None,
                created_at: None,
            },
        )
        .unwrap();
        let mock = MockTransport::new();
        mock.on(
            Method::Post,
            "https://mockfal/flux-pro",
            200,
            json!({"request_id": "retry-1", "status": "IN_QUEUE"}),
        );
        mock.on(
            Method::Get,
            "https://mockfal/flux-pro/requests/retry-1/status",
            200,
            json!({"status": "COMPLETED"}),
        );
        mock.on(
            Method::Get,
            "https://mockfal/flux-pro/requests/retry-1",
            200,
            json!({"images": [{"url": png_data_url()}]}),
        );
        let (cache, models) = runtime_dirs(&bundle);
        let bridge = build_bridge_with_clients(
            core.clone(),
            cache,
            models,
            Arc::new(FixtureClients {
                client: fixture_client(&mock),
            }),
        );
        assert!(bridge.retry(&failed.job_id, false).is_err());
        assert!(mock.calls().is_empty());
        let retried = bridge.retry(&failed.job_id, true).unwrap();
        assert_ne!(retried.job_id, failed.job_id);
        for _ in 0..100 {
            let ready = core.media().entries.iter().any(|entry| {
                entry.generation_input.as_ref().is_some_and(|input| {
                    input.job_id.as_deref() == Some(retried.job_id.as_str())
                        && input.status == Some(GenerationJobStatus::Ready)
                })
            });
            if ready {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let statuses = core
            .media()
            .entries
            .iter()
            .filter_map(|entry| entry.generation_input.as_ref())
            .map(|input| (input.job_id.clone().unwrap(), input.status.unwrap()))
            .collect::<HashMap<_, _>>();
        assert_eq!(
            statuses.get(&failed.job_id),
            Some(&GenerationJobStatus::Failed)
        );
        assert_eq!(
            statuses.get(&retried.job_id),
            Some(&GenerationJobStatus::Ready)
        );
    }
}
