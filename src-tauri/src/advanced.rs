//! Production desktop implementations for capability-gated advanced workflows.

use std::collections::VecDeque;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use opentake_agent::mcp::advanced::{
    AdvancedWorkflowBridge, AdvancedWorkflowCommit, AdvancedWorkflowError,
    AdvancedWorkflowErrorKind, AdvancedWorkflowRequest,
};
use opentake_agent::tools::args::{
    CloneVoiceArgs, GenerateAvatarArgs, GenerateMatteArgs, MatchColorArgs, RemoveObjectArgs,
    ScriptSegmentArg, ScriptToVideoArgs, SeparateStemsArgs, TrackMotionArgs, TranslateCaptionsArgs,
};
use opentake_agent::tools::names::ToolName;
use opentake_core::{
    AppCore, DerivedStemProvenance, MotionPlacement, PreparedMediaImportOp, ProbedMedia,
    ProjectRevision,
};
use opentake_domain::{
    luma709, AnimPair, CaptionTranslationInput, ColorGrade, ColorMatchInput, GenerationInput,
    GenerationJobStatus, Interpolation, Keyframe, KeyframeTrack, LiftGammaGain, Mask, Rgb,
    ScriptAssemblyPlan, ScriptAssemblySegment, TransitionKind, VoiceModelRecord,
};
use opentake_gen::{KeyStore, KeyringStore, ProviderKey};
use opentake_media::analysis::{
    track_region_motion, verify_rvm_model, NormalizedMotionRegion, RegionMotionTrack,
    RvmMattingSession, StemExecution, StemSeparationRequest,
};
use opentake_media::decode::spawn_video_stream;
use opentake_media::{
    decode_frames_at_cancellable, extract_pcm_cancellable, file_sha256, probe, ExportPreset,
    ExportResolution, FrameRequest, MediaCancelToken, MediaError, PcmFormat, PcmSpec, RgbaFrame,
    StreamVideoFrame, VideoCodec, VideoEncoder, VideoStream, VideoStreamRequest,
};
use opentake_ops::{
    CaptionTranslationChange, ClipEntry, EditCommand, KeyframePayload, KeyframeProperty,
};
use same_file::Handle;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, State};

use crate::media::MediaState;

pub struct TauriAdvancedWorkflowBridge {
    core: AppCore,
    cache_root: PathBuf,
    models_dir: PathBuf,
    caption_translator: Arc<dyn CaptionTranslationProvider>,
    avatar_provider: Arc<dyn AvatarProvider>,
    voice_provider: Arc<dyn VoiceCloneProvider>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CaptionTranslationDraft {
    id: String,
    text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CaptionTranslationFailure {
    id: String,
    message: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CaptionTranslationProviderResult {
    translations: Vec<CaptionTranslationDraft>,
    #[serde(default)]
    errors: Vec<CaptionTranslationFailure>,
}

trait CaptionTranslationProvider: Send + Sync {
    fn translate(
        &self,
        provider: &str,
        model: &str,
        source_locale: &str,
        target_locale: &str,
        captions: &[CaptionTranslationDraft],
        cancel: &MediaCancelToken,
    ) -> Result<CaptionTranslationProviderResult, AdvancedWorkflowError>;
}

struct NetworkCaptionTranslationProvider;

#[derive(Clone, Debug)]
struct AvatarProviderRequest {
    portrait_path: PathBuf,
    audio_path: PathBuf,
    model: String,
    destination: PathBuf,
}

#[derive(Clone, Debug)]
struct AvatarProviderOutput {
    request_id: String,
    media_type: String,
}

trait AvatarProvider: Send + Sync {
    fn generate(
        &self,
        request: &AvatarProviderRequest,
        cancel: &MediaCancelToken,
    ) -> Result<AvatarProviderOutput, AdvancedWorkflowError>;
}

#[derive(Clone, Debug)]
struct VoiceEnrollmentRequest {
    reference_path: PathBuf,
    voice_name: String,
}

#[derive(Clone, Debug)]
struct VoiceGenerationRequest {
    provider_voice_id: String,
    model: String,
    prompt: String,
    destination: PathBuf,
}

#[derive(Clone, Debug)]
struct VoiceProviderOutput {
    request_id: String,
    media_type: String,
}

trait VoiceCloneProvider: Send + Sync {
    fn enroll(
        &self,
        request: &VoiceEnrollmentRequest,
        cancel: &MediaCancelToken,
    ) -> Result<String, AdvancedWorkflowError>;

    fn generate(
        &self,
        request: &VoiceGenerationRequest,
        cancel: &MediaCancelToken,
    ) -> Result<VoiceProviderOutput, AdvancedWorkflowError>;

    fn revoke(
        &self,
        provider_voice_id: &str,
        cancel: &MediaCancelToken,
    ) -> Result<(), AdvancedWorkflowError>;
}

struct NetworkFalAvatarProvider {
    cache_root: PathBuf,
}

struct NetworkElevenLabsVoiceProvider;

fn valid_provider_resource_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn provider_key(key: ProviderKey, label: &str) -> Result<String, AdvancedWorkflowError> {
    KeyringStore::new()
        .load(key.account())
        .map_err(|error| advanced_execution(format!("could not read {label} key: {error}")))?
        .ok_or_else(|| {
            AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::ConsentRequired,
                format!("no {label} API key is configured; open Settings → AI"),
            )
        })
}

fn reference_data_url(
    path: &Path,
    fallback: &str,
    max_bytes: u64,
) -> Result<String, AdvancedWorkflowError> {
    use base64::Engine as _;
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| advanced_resource("reference media is unavailable"))?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > max_bytes
    {
        return Err(advanced_invalid(
            "reference media must be a bounded regular non-symlink file",
        ));
    }
    let bytes =
        std::fs::read(path).map_err(|_| advanced_resource("reference media could not be read"))?;
    let mime = opentake_gen::content_type_for(path, fallback);
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn bounded_json_response(
    response: reqwest::blocking::Response,
    provider: &str,
) -> Result<serde_json::Value, AdvancedWorkflowError> {
    const LIMIT: u64 = 4 * 1024 * 1024;
    if !response.status().is_success() {
        return Err(advanced_execution(format!(
            "{provider} provider returned HTTP {}",
            response.status()
        )));
    }
    if response
        .content_length()
        .is_some_and(|length| length > LIMIT)
    {
        return Err(advanced_execution(format!(
            "{provider} provider response is too large"
        )));
    }
    let mut body = Vec::new();
    response
        .take(LIMIT + 1)
        .read_to_end(&mut body)
        .map_err(|_| advanced_execution(format!("could not read {provider} response")))?;
    if body.len() as u64 > LIMIT {
        return Err(advanced_execution(format!(
            "{provider} provider response is too large"
        )));
    }
    serde_json::from_slice(&body)
        .map_err(|_| advanced_execution(format!("{provider} returned invalid JSON")))
}

impl AvatarProvider for NetworkFalAvatarProvider {
    fn generate(
        &self,
        request: &AvatarProviderRequest,
        cancel: &MediaCancelToken,
    ) -> Result<AvatarProviderOutput, AdvancedWorkflowError> {
        if cancel.checkpoint() {
            return Err(cancelled_workflow("avatar generation cancelled"));
        }
        let key = provider_key(ProviderKey::Fal, "fal")?;
        let image_url = reference_data_url(&request.portrait_path, "image", 20 * 1024 * 1024)?;
        let audio_url = reference_data_url(&request.audio_path, "audio", 50 * 1024 * 1024)?;
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|_| advanced_execution("avatar provider client initialization failed"))?;
        let queue_base = format!("https://queue.fal.run/{}", request.model);
        let submit = client
            .post(&queue_base)
            .header("Authorization", format!("Key {key}"))
            .json(&json!({"image_url": image_url, "audio_url": audio_url}))
            .send()
            .map_err(|_| advanced_execution("avatar provider submission failed"))?;
        let submit = bounded_json_response(submit, "avatar")?;
        let request_id = submit
            .get("request_id")
            .and_then(serde_json::Value::as_str)
            .filter(|value| valid_provider_resource_id(value))
            .ok_or_else(|| advanced_execution("avatar provider returned no request id"))?
            .to_string();
        let cancel_remote = || {
            let _ = client
                .put(format!("{queue_base}/requests/{request_id}/cancel"))
                .header("Authorization", format!("Key {key}"))
                .timeout(std::time::Duration::from_secs(10))
                .send();
        };
        let started = std::time::Instant::now();
        loop {
            if cancel.checkpoint() {
                cancel_remote();
                return Err(cancelled_workflow("avatar generation cancelled"));
            }
            if started.elapsed() > std::time::Duration::from_secs(30 * 60) {
                cancel_remote();
                return Err(advanced_execution("avatar provider timed out"));
            }
            let response = client
                .get(format!("{queue_base}/requests/{request_id}/status"))
                .header("Authorization", format!("Key {key}"))
                .send()
                .map_err(|_| advanced_execution("avatar provider status request failed"))?;
            let status = bounded_json_response(response, "avatar")?;
            match status.get("status").and_then(serde_json::Value::as_str) {
                Some("COMPLETED") => break,
                Some("FAILED") => {
                    return Err(advanced_execution("avatar provider generation failed"))
                }
                Some("IN_QUEUE" | "IN_PROGRESS") => {
                    std::thread::sleep(std::time::Duration::from_millis(350));
                }
                _ => {
                    return Err(advanced_execution(
                        "avatar provider returned unknown status",
                    ))
                }
            }
        }
        if cancel.checkpoint() {
            cancel_remote();
            return Err(cancelled_workflow("avatar generation cancelled"));
        }
        let result = client
            .get(format!("{queue_base}/requests/{request_id}"))
            .header("Authorization", format!("Key {key}"))
            .send()
            .map_err(|_| advanced_execution("avatar provider result request failed"))?;
        let result = bounded_json_response(result, "avatar")?;
        let result_url = result
            .pointer("/video/url")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| advanced_execution("avatar provider returned no video"))?;
        let (media_type, _) = crate::generation::secure_download_generation_result(
            self.cache_root.join("avatar-downloads"),
            cancel.clone(),
            result_url,
            &request.destination,
        )
        .map_err(advanced_execution)?;
        Ok(AvatarProviderOutput {
            request_id,
            media_type,
        })
    }
}

impl VoiceCloneProvider for NetworkElevenLabsVoiceProvider {
    fn enroll(
        &self,
        request: &VoiceEnrollmentRequest,
        cancel: &MediaCancelToken,
    ) -> Result<String, AdvancedWorkflowError> {
        if cancel.checkpoint() {
            return Err(cancelled_workflow("voice enrollment cancelled"));
        }
        let key = provider_key(ProviderKey::ElevenLabs, "ElevenLabs")?;
        let metadata = std::fs::symlink_metadata(&request.reference_path)
            .map_err(|_| advanced_resource("voice reference audio is unavailable"))?;
        if !metadata.file_type().is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() == 0
            || metadata.len() > 100 * 1024 * 1024
        {
            return Err(advanced_invalid(
                "voice reference audio is invalid or too large",
            ));
        }
        let bytes = std::fs::read(&request.reference_path)
            .map_err(|_| advanced_resource("voice reference audio could not be read"))?;
        let file_name = request
            .reference_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("voice-reference.wav")
            .to_string();
        let part = reqwest::blocking::multipart::Part::bytes(bytes).file_name(file_name);
        let form = reqwest::blocking::multipart::Form::new()
            .text("name", request.voice_name.clone())
            .part("files", part);
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .map_err(|_| advanced_execution("voice provider client initialization failed"))?;
        let response = client
            .post("https://api.elevenlabs.io/v1/voices/add")
            .header("xi-api-key", key.clone())
            .multipart(form)
            .send()
            .map_err(|_| advanced_execution("voice enrollment request failed"))?;
        let response = bounded_json_response(response, "voice")?;
        let provider_voice_id = response
            .get("voice_id")
            .and_then(serde_json::Value::as_str)
            .filter(|value| valid_provider_resource_id(value))
            .map(str::to_string)
            .ok_or_else(|| advanced_execution("voice provider returned no valid voice id"))?;
        if cancel.checkpoint() {
            let _ = client
                .delete(format!(
                    "https://api.elevenlabs.io/v1/voices/{provider_voice_id}"
                ))
                .header("xi-api-key", key)
                .timeout(std::time::Duration::from_secs(10))
                .send();
            return Err(cancelled_workflow("voice enrollment cancelled"));
        }
        Ok(provider_voice_id)
    }

    fn generate(
        &self,
        request: &VoiceGenerationRequest,
        cancel: &MediaCancelToken,
    ) -> Result<VoiceProviderOutput, AdvancedWorkflowError> {
        if cancel.checkpoint() {
            return Err(cancelled_workflow("voice generation cancelled"));
        }
        if !valid_provider_resource_id(&request.provider_voice_id) {
            return Err(advanced_invalid("voice provider id is invalid"));
        }
        let key = provider_key(ProviderKey::ElevenLabs, "ElevenLabs")?;
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .map_err(|_| advanced_execution("voice provider client initialization failed"))?;
        let mut response = client
            .post(format!(
                "https://api.elevenlabs.io/v1/text-to-speech/{}",
                request.provider_voice_id
            ))
            .header("xi-api-key", key)
            .json(&json!({"text": request.prompt, "model_id": request.model}))
            .send()
            .map_err(|_| advanced_execution("voice generation request failed"))?;
        if !response.status().is_success() {
            return Err(advanced_execution(format!(
                "voice provider returned HTTP {}",
                response.status()
            )));
        }
        if response
            .content_length()
            .is_some_and(|length| length > 100 * 1024 * 1024)
        {
            return Err(advanced_execution("voice output is too large"));
        }
        let media_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .unwrap_or("audio/mpeg")
            .to_string();
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&request.destination)
            .map_err(|_| advanced_execution("voice output staging is unavailable"))?;
        let mut total = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            if cancel.checkpoint() {
                let _ = std::fs::remove_file(&request.destination);
                return Err(cancelled_workflow("voice generation cancelled"));
            }
            let count = response
                .read(&mut buffer)
                .map_err(|_| advanced_execution("voice output stream failed"))?;
            if count == 0 {
                break;
            }
            total = total.saturating_add(count as u64);
            if total > 100 * 1024 * 1024 {
                let _ = std::fs::remove_file(&request.destination);
                return Err(advanced_execution("voice output is too large"));
            }
            output
                .write_all(&buffer[..count])
                .map_err(|_| advanced_execution("voice output staging failed"))?;
        }
        output
            .sync_all()
            .map_err(|_| advanced_execution("voice output staging failed"))?;
        if total == 0 {
            let _ = std::fs::remove_file(&request.destination);
            return Err(advanced_execution("voice provider returned empty audio"));
        }
        Ok(VoiceProviderOutput {
            request_id: format!("tts-{}", uuid::Uuid::new_v4()),
            media_type,
        })
    }

    fn revoke(
        &self,
        provider_voice_id: &str,
        cancel: &MediaCancelToken,
    ) -> Result<(), AdvancedWorkflowError> {
        if cancel.checkpoint() {
            return Err(cancelled_workflow("voice revocation cancelled"));
        }
        if !valid_provider_resource_id(provider_voice_id) {
            return Err(advanced_invalid("voice provider id is invalid"));
        }
        let key = provider_key(ProviderKey::ElevenLabs, "ElevenLabs")?;
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|_| advanced_execution("voice provider client initialization failed"))?;
        let response = client
            .delete(format!(
                "https://api.elevenlabs.io/v1/voices/{provider_voice_id}"
            ))
            .header("xi-api-key", key)
            .send()
            .map_err(|_| advanced_execution("voice revocation request failed"))?;
        if !response.status().is_success() && response.status() != reqwest::StatusCode::NOT_FOUND {
            return Err(advanced_execution(format!(
                "voice provider returned HTTP {}",
                response.status()
            )));
        }
        // Once the provider has accepted deletion, the local revocation record
        // must be committed even if cancellation arrives concurrently. Returning
        // Cancelled here would leave an apparently active local model whose
        // remote identity has already been irreversibly removed.
        Ok(())
    }
}

impl CaptionTranslationProvider for NetworkCaptionTranslationProvider {
    fn translate(
        &self,
        provider: &str,
        model: &str,
        source_locale: &str,
        target_locale: &str,
        captions: &[CaptionTranslationDraft],
        cancel: &MediaCancelToken,
    ) -> Result<CaptionTranslationProviderResult, AdvancedWorkflowError> {
        if cancel.checkpoint() {
            return Err(cancelled_workflow("caption translation cancelled"));
        }
        let key = match provider {
            "openai" => ProviderKey::OpenAI,
            "anthropic" => ProviderKey::Anthropic,
            _ => {
                return Err(AdvancedWorkflowError::new(
                    AdvancedWorkflowErrorKind::CapabilityUnavailable,
                    "caption translation supports OpenAI or Anthropic",
                ))
            }
        };
        let secret = KeyringStore::new()
            .load(key.account())
            .map_err(|error| advanced_execution(format!("could not read {provider} key: {error}")))?
            .ok_or_else(|| {
                AdvancedWorkflowError::new(
                    AdvancedWorkflowErrorKind::ConsentRequired,
                    format!("no {provider} API key is configured; open Settings → AI"),
                )
            })?;
        let payload = serde_json::to_string(captions)
            .map_err(|error| advanced_execution(error.to_string()))?;
        let instruction = format!(
            "Translate every caption from locale {source_locale} to {target_locale}. Preserve meaning and natural subtitle phrasing. Return only JSON with shape {{\"translations\":[{{\"id\":string,\"text\":string}}],\"errors\":[{{\"id\":string,\"message\":string}}]}}. Every input id must appear exactly once in translations or errors. Never change ids. Captions: {payload}"
        );
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|error| advanced_execution(format!("translation client: {error}")))?;
        let response = match provider {
            "openai" => client
                .post("https://api.openai.com/v1/chat/completions")
                .bearer_auth(secret)
                .json(&json!({
                    "model": model,
                    "temperature": 0,
                    "response_format": {"type": "json_object"},
                    "messages": [
                        {"role": "system", "content": "You are a precise audiovisual subtitle translator."},
                        {"role": "user", "content": instruction}
                    ]
                }))
                .send(),
            "anthropic" => client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", secret)
                .header("anthropic-version", "2023-06-01")
                .json(&json!({
                    "model": model,
                    "max_tokens": 8192,
                    "temperature": 0,
                    "system": "You are a precise audiovisual subtitle translator. Return only JSON.",
                    "messages": [{"role": "user", "content": instruction}]
                }))
                .send(),
            _ => unreachable!(),
        }
        .map_err(|error| advanced_execution(format!("caption translation network error: {error}")))?;
        if !response.status().is_success() {
            return Err(advanced_execution(format!(
                "caption translation provider returned HTTP {}",
                response.status()
            )));
        }
        const MAX_PROVIDER_RESPONSE_BYTES: u64 = 4 * 1024 * 1024;
        if response
            .content_length()
            .is_some_and(|bytes| bytes > MAX_PROVIDER_RESPONSE_BYTES)
        {
            return Err(advanced_execution(
                "translation provider response is too large",
            ));
        }
        let response = response;
        let mut body = Vec::new();
        response
            .take(MAX_PROVIDER_RESPONSE_BYTES + 1)
            .read_to_end(&mut body)
            .map_err(|error| {
                advanced_execution(format!("could not read provider response: {error}"))
            })?;
        if body.len() as u64 > MAX_PROVIDER_RESPONSE_BYTES {
            return Err(advanced_execution(
                "translation provider response is too large",
            ));
        }
        let wire: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|error| advanced_execution(format!("invalid provider response: {error}")))?;
        if cancel.checkpoint() {
            return Err(cancelled_workflow("caption translation cancelled"));
        }
        let raw = if provider == "openai" {
            wire.pointer("/choices/0/message/content")
        } else {
            wire.pointer("/content/0/text")
        }
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| advanced_execution("translation provider returned no JSON content"))?;
        let raw = raw
            .trim()
            .strip_prefix("```json")
            .or_else(|| raw.trim().strip_prefix("```"))
            .unwrap_or(raw.trim())
            .trim()
            .strip_suffix("```")
            .unwrap_or(raw.trim())
            .trim();
        serde_json::from_str(raw)
            .map_err(|error| advanced_execution(format!("invalid translation JSON: {error}")))
    }
}

pub struct AdvancedWorkflowCommandState {
    bridge: Arc<TauriAdvancedWorkflowBridge>,
    active: Mutex<Option<ActiveAdvancedWorkflow>>,
    admission: crate::updater::InstallAdmissionGate,
}

struct ActiveAdvancedWorkflow {
    token: MediaCancelToken,
    _admission: crate::updater::ActivityLease,
}

impl AdvancedWorkflowCommandState {
    pub fn new(
        bridge: Arc<TauriAdvancedWorkflowBridge>,
        admission: crate::updater::InstallAdmissionGate,
    ) -> Self {
        Self {
            bridge,
            active: Mutex::new(None),
            admission,
        }
    }

    fn begin(&self) -> Result<MediaCancelToken, String> {
        let admission = self.admission.begin_activity()?;
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.is_some() {
            return Err("advanced_workflow_busy".to_string());
        }
        let token = MediaCancelToken::new();
        *active = Some(ActiveAdvancedWorkflow {
            token: token.clone(),
            _admission: admission,
        });
        Ok(token)
    }

    fn finish(&self, token: &MediaCancelToken) {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active
            .as_ref()
            .is_some_and(|current| current.token.same_instance(token))
        {
            *active = None;
        }
    }

    pub fn cancel_active(&self) -> bool {
        let active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        active.as_ref().is_some_and(|current| {
            current.token.cancel();
            true
        })
    }
}

pub struct MattingModelInstallState {
    active: Mutex<Option<ActiveMattingModelInstall>>,
    admission: crate::updater::InstallAdmissionGate,
}

struct ActiveMattingModelInstall {
    token: MediaCancelToken,
    _admission: crate::updater::ActivityLease,
}

impl MattingModelInstallState {
    pub fn new(admission: crate::updater::InstallAdmissionGate) -> Self {
        Self {
            active: Mutex::new(None),
            admission,
        }
    }

    fn begin(&self) -> Result<MediaCancelToken, String> {
        let admission = self.admission.begin_activity()?;
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.is_some() {
            return Err("matting_model_download_busy".to_string());
        }
        let token = MediaCancelToken::new();
        *active = Some(ActiveMattingModelInstall {
            token: token.clone(),
            _admission: admission,
        });
        Ok(token)
    }

    fn finish(&self, token: &MediaCancelToken) {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active
            .as_ref()
            .is_some_and(|current| current.token.same_instance(token))
        {
            *active = None;
        }
    }

    fn cancel(&self) -> bool {
        let active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        active.as_ref().is_some_and(|current| {
            current.token.cancel();
            true
        })
    }

    pub(crate) fn cancel_active(&self) -> bool {
        self.cancel()
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MattingModelStatusDto {
    pub installed: bool,
    pub model: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct MattingProgressDto {
    fraction: f64,
    downloaded_bytes: u64,
    total_bytes: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GenerateMatteResultDto {
    pub result: serde_json::Value,
    pub action_name: Option<String>,
}

#[tauri::command]
pub fn matting_model_status(media: State<'_, MediaState>) -> MattingModelStatusDto {
    let installed = verify_rvm_model(media.engine().models_dir()).is_ok();
    MattingModelStatusDto {
        installed,
        model: opentake_media::analysis::RVM_MODEL_ID.to_string(),
        bytes: opentake_media::analysis::RVM_MODEL_BYTES,
        sha256: opentake_media::analysis::RVM_MODEL_SHA256.to_string(),
    }
}

#[tauri::command]
pub async fn download_matting_model(
    app: AppHandle,
    media: State<'_, MediaState>,
    state: State<'_, MattingModelInstallState>,
) -> Result<MattingModelStatusDto, String> {
    let token = state.begin()?;
    let models_dir = media.engine().models_dir().to_path_buf();
    let progress_app = app.clone();
    let progress = Arc::new(move |downloaded_bytes: u64, total_bytes: u64| {
        let fraction = if total_bytes == 0 {
            0.0
        } else {
            downloaded_bytes as f64 / total_bytes as f64
        };
        let _ = progress_app.emit(
            "matting://progress",
            MattingProgressDto {
                fraction: fraction.clamp(0.0, 1.0),
                downloaded_bytes,
                total_bytes,
            },
        );
    });
    let result = opentake_media::analysis::download_rvm_model(&models_dir, &token, Some(progress))
        .await
        .map_err(|error| error.to_string());
    state.finish(&token);
    result?;
    Ok(matting_model_status(media))
}

#[tauri::command]
pub fn cancel_matting_model_download(state: State<'_, MattingModelInstallState>) -> bool {
    state.cancel()
}

#[tauri::command]
pub async fn advanced_track_motion(
    state: State<'_, AdvancedWorkflowCommandState>,
    request: TrackMotionArgs,
) -> Result<GenerateMatteResultDto, String> {
    let token = state.begin()?;
    let bridge = Arc::clone(&state.bridge);
    let worker_token = token.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        bridge.execute(AdvancedWorkflowRequest::TrackMotion(request), &worker_token)
    })
    .await
    .map_err(|error| format!("advanced workflow worker failed: {error}"))
    .and_then(|result| result.map_err(|error| error.message));
    state.finish(&token);
    let commit = result?;
    Ok(GenerateMatteResultDto {
        result: commit.result,
        action_name: commit.action_name,
    })
}

#[tauri::command]
pub async fn advanced_generate_matte(
    state: State<'_, AdvancedWorkflowCommandState>,
    request: GenerateMatteArgs,
) -> Result<GenerateMatteResultDto, String> {
    let token = state.begin()?;
    let bridge = Arc::clone(&state.bridge);
    let worker_token = token.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        bridge.execute(
            AdvancedWorkflowRequest::GenerateMatte(request),
            &worker_token,
        )
    })
    .await
    .map_err(|error| format!("advanced workflow worker failed: {error}"))
    .and_then(|result| result.map_err(|error| error.message));
    state.finish(&token);
    let commit = result?;
    Ok(GenerateMatteResultDto {
        result: commit.result,
        action_name: commit.action_name,
    })
}

#[tauri::command]
pub async fn advanced_remove_object(
    state: State<'_, AdvancedWorkflowCommandState>,
    request: RemoveObjectArgs,
) -> Result<GenerateMatteResultDto, String> {
    let token = state.begin()?;
    let bridge = Arc::clone(&state.bridge);
    let worker_token = token.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        bridge.execute(
            AdvancedWorkflowRequest::RemoveObject(request),
            &worker_token,
        )
    })
    .await
    .map_err(|error| format!("advanced workflow worker failed: {error}"))
    .and_then(|result| result.map_err(|error| error.message));
    state.finish(&token);
    let commit = result?;
    Ok(GenerateMatteResultDto {
        result: commit.result,
        action_name: commit.action_name,
    })
}

#[tauri::command]
pub async fn advanced_match_color(
    state: State<'_, AdvancedWorkflowCommandState>,
    request: MatchColorArgs,
) -> Result<GenerateMatteResultDto, String> {
    let token = state.begin()?;
    let bridge = Arc::clone(&state.bridge);
    let worker_token = token.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        bridge.execute(AdvancedWorkflowRequest::MatchColor(request), &worker_token)
    })
    .await
    .map_err(|error| format!("advanced workflow worker failed: {error}"))
    .and_then(|result| result.map_err(|error| error.message));
    state.finish(&token);
    let commit = result?;
    Ok(GenerateMatteResultDto {
        result: commit.result,
        action_name: commit.action_name,
    })
}

#[tauri::command]
pub async fn advanced_translate_captions(
    state: State<'_, AdvancedWorkflowCommandState>,
    request: TranslateCaptionsArgs,
) -> Result<GenerateMatteResultDto, String> {
    let token = state.begin()?;
    let bridge = Arc::clone(&state.bridge);
    let worker_token = token.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        bridge.execute(
            AdvancedWorkflowRequest::TranslateCaptions(request),
            &worker_token,
        )
    })
    .await
    .map_err(|error| format!("advanced workflow worker failed: {error}"))
    .and_then(|result| result.map_err(|error| error.message));
    state.finish(&token);
    let commit = result?;
    Ok(GenerateMatteResultDto {
        result: commit.result,
        action_name: commit.action_name,
    })
}

#[tauri::command]
pub async fn advanced_script_to_video(
    state: State<'_, AdvancedWorkflowCommandState>,
    request: ScriptToVideoArgs,
) -> Result<GenerateMatteResultDto, String> {
    let token = state.begin()?;
    let bridge = Arc::clone(&state.bridge);
    let worker_token = token.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        bridge.execute(
            AdvancedWorkflowRequest::ScriptToVideo(request),
            &worker_token,
        )
    })
    .await
    .map_err(|error| format!("advanced workflow worker failed: {error}"))
    .and_then(|result| result.map_err(|error| error.message));
    state.finish(&token);
    let commit = result?;
    Ok(GenerateMatteResultDto {
        result: commit.result,
        action_name: commit.action_name,
    })
}

#[tauri::command]
pub async fn advanced_generate_avatar(
    state: State<'_, AdvancedWorkflowCommandState>,
    request: GenerateAvatarArgs,
) -> Result<GenerateMatteResultDto, String> {
    let token = state.begin()?;
    let bridge = Arc::clone(&state.bridge);
    let worker_token = token.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        bridge.execute(
            AdvancedWorkflowRequest::GenerateAvatar(request),
            &worker_token,
        )
    })
    .await
    .map_err(|error| format!("advanced workflow worker failed: {error}"))
    .and_then(|result| result.map_err(|error| error.message));
    state.finish(&token);
    let commit = result?;
    Ok(GenerateMatteResultDto {
        result: commit.result,
        action_name: commit.action_name,
    })
}

#[tauri::command]
pub async fn advanced_clone_voice(
    state: State<'_, AdvancedWorkflowCommandState>,
    request: CloneVoiceArgs,
) -> Result<GenerateMatteResultDto, String> {
    let token = state.begin()?;
    let bridge = Arc::clone(&state.bridge);
    let worker_token = token.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        bridge.execute(AdvancedWorkflowRequest::CloneVoice(request), &worker_token)
    })
    .await
    .map_err(|error| format!("advanced workflow worker failed: {error}"))
    .and_then(|result| result.map_err(|error| error.message));
    state.finish(&token);
    let commit = result?;
    Ok(GenerateMatteResultDto {
        result: commit.result,
        action_name: commit.action_name,
    })
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyCaptionTranslationReviewRequest {
    project_epoch: u64,
    version: u64,
    source_locale: String,
    target_locale: String,
    provider: String,
    model: String,
    changes: Vec<CaptionTranslationReviewChange>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptionTranslationReviewChange {
    id: String,
    source_text: String,
    translated_text: String,
}

#[tauri::command]
pub fn advanced_apply_caption_translation_review(
    state: State<'_, AdvancedWorkflowCommandState>,
    request: ApplyCaptionTranslationReviewRequest,
) -> Result<GenerateMatteResultDto, String> {
    let _activity = crate::updater::begin_mutating_activity(&state.admission)?;
    if request.changes.is_empty() || request.changes.len() > 500 {
        return Err("select between 1 and 500 translated captions to apply".into());
    }
    if request.source_locale.is_empty()
        || request.source_locale.len() > 64
        || request.target_locale.is_empty()
        || request.target_locale.len() > 64
        || request.provider.is_empty()
        || request.provider.len() > 32
        || request.model.is_empty()
        || request.model.len() > 128
        || request.changes.iter().any(|change| {
            change.id.is_empty()
                || change.id.len() > 256
                || change.source_text.len() > 20_000
                || change.translated_text.len() > 20_000
        })
    {
        return Err("caption translation review contains invalid or oversized fields".into());
    }
    let changes = request
        .changes
        .into_iter()
        .map(|change| CaptionTranslationChange {
            clip_id: change.id,
            expected_source_text: change.source_text.clone(),
            translated_text: change.translated_text,
            input: CaptionTranslationInput {
                source_text: change.source_text,
                source_locale: request.source_locale.clone(),
                target_locale: request.target_locale.clone(),
                provider: request.provider.clone(),
                model: request.model.clone(),
            },
        })
        .collect();
    let edit = state
        .bridge
        .core
        .apply_at_revision(
            ProjectRevision {
                project_epoch: request.project_epoch,
                version: request.version,
            },
            EditCommand::ApplyCaptionTranslations { changes },
        )
        .map_err(|error| error.to_string())?;
    Ok(GenerateMatteResultDto {
        result: json!({"applied": edit.changed}),
        action_name: edit.changed.then_some(edit.action_name),
    })
}

#[tauri::command]
pub fn cancel_advanced_workflow(state: State<'_, AdvancedWorkflowCommandState>) -> bool {
    state.cancel_active()
}

impl TauriAdvancedWorkflowBridge {
    pub fn new(core: AppCore, cache_root: PathBuf, models_dir: PathBuf) -> Self {
        Self {
            core,
            avatar_provider: Arc::new(NetworkFalAvatarProvider {
                cache_root: cache_root.clone(),
            }),
            cache_root,
            models_dir,
            caption_translator: Arc::new(NetworkCaptionTranslationProvider),
            voice_provider: Arc::new(NetworkElevenLabsVoiceProvider),
        }
    }

    #[cfg(test)]
    fn with_caption_translator(
        core: AppCore,
        cache_root: PathBuf,
        models_dir: PathBuf,
        caption_translator: Arc<dyn CaptionTranslationProvider>,
    ) -> Self {
        Self {
            core,
            avatar_provider: Arc::new(NetworkFalAvatarProvider {
                cache_root: cache_root.clone(),
            }),
            cache_root,
            models_dir,
            caption_translator,
            voice_provider: Arc::new(NetworkElevenLabsVoiceProvider),
        }
    }

    #[cfg(test)]
    fn with_identity_providers(
        core: AppCore,
        cache_root: PathBuf,
        models_dir: PathBuf,
        avatar_provider: Arc<dyn AvatarProvider>,
        voice_provider: Arc<dyn VoiceCloneProvider>,
    ) -> Self {
        Self {
            core,
            cache_root,
            models_dir,
            caption_translator: Arc::new(NetworkCaptionTranslationProvider),
            avatar_provider,
            voice_provider,
        }
    }

    fn track_motion(
        &self,
        args: TrackMotionArgs,
        cancel: &MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        let snapshot = self.core.runtime_snapshot();
        let clip = snapshot
            .timeline
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .find(|clip| clip.id == args.clip_id)
            .cloned()
            .ok_or_else(|| advanced_resource(format!("clip not found: {}", args.clip_id)))?;
        if clip.media_type != opentake_domain::ClipType::Video || clip.nested_sequence_id.is_some()
        {
            return Err(advanced_invalid(
                "motion tracking requires an ordinary video clip",
            ));
        }
        let region: opentake_agent::tools::args::MotionRegionArg =
            serde_json::from_value(args.region)
                .map_err(|error| advanced_invalid(error.to_string()))?;
        let region = NormalizedMotionRegion {
            x: region.x,
            y: region.y,
            width: region.width,
            height: region.height,
        };
        let clip_end = clip.start_frame + clip.duration_frames;
        let start = args.start_frame.unwrap_or(clip.start_frame);
        let end = args.end_frame.unwrap_or(clip_end);
        if start < clip.start_frame || end > clip_end || end - start < 2 {
            return Err(advanced_invalid(
                "startFrame/endFrame must be a non-empty range inside the clip",
            ));
        }
        let first_relative = start - clip.start_frame;
        let last_relative = end - clip.start_frame - 1;
        let sample_count = (last_relative - first_relative + 1).clamp(2, 48) as usize;
        let relative_frames = (0..sample_count)
            .map(|index| {
                (first_relative as f64
                    + index as f64 * (last_relative - first_relative) as f64
                        / (sample_count - 1) as f64)
                    .round() as i32
            })
            .collect::<Vec<_>>();
        let fps = snapshot.timeline.fps.max(1) as f64;
        let source_start = clip.trim_start_frame as f64 / fps;
        let times = relative_frames
            .iter()
            .map(|frame| source_start + *frame as f64 * clip.speed.max(0.0001) / fps)
            .collect::<Vec<_>>();
        let (path, is_video) =
            crate::transcribe::resolve_asset_from_snapshot(&snapshot, &clip.media_ref)
                .map_err(advanced_resource)?;
        if !is_video {
            return Err(advanced_invalid("motion tracking source is not a video"));
        }
        let request = FrameRequest {
            max_size: (640, 360),
            tolerance_secs: 0.05,
            ..FrameRequest::default()
        };
        let decoded = decode_frames_at_cancellable(&path, &times, &request, cancel);
        let mut frames = decoded
            .into_iter()
            .filter_map(Result::ok)
            .map(|(actual, frame)| {
                let relative =
                    ((actual - source_start) * fps / clip.speed.max(0.0001)).round() as i32;
                (relative.clamp(first_relative, last_relative), frame)
            })
            .collect::<Vec<_>>();
        if cancel.is_cancelled() {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::Cancelled,
                "motion tracking cancelled",
            ));
        }
        frames.sort_by_key(|(frame, _)| *frame);
        frames.dedup_by_key(|(frame, _)| *frame);
        let tracked = track_region_motion(&frames, region, cancel)
            .map_err(|error| advanced_execution(error.to_string()))?;
        if tracked.minimum_confidence < 0.25 {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::AnalysisLowConfidence,
                format!(
                    "motion tracking confidence {:.3} is below the 0.25 threshold",
                    tracked.minimum_confidence
                ),
            ));
        }
        let keyframes = position_keyframes(&clip, &tracked);
        let apply_requested = args.apply.unwrap_or(false);
        let (applied, action_name) = if apply_requested {
            let result = self
                .core
                .apply_at_revision(
                    ProjectRevision {
                        project_epoch: snapshot.project_epoch,
                        version: snapshot.version,
                    },
                    EditCommand::SetKeyframes {
                        clip_id: clip.id.clone(),
                        property: KeyframeProperty::Position,
                        payload: KeyframePayload::Pair(KeyframeTrack::from_keyframes(
                            keyframes.clone(),
                        )),
                    },
                )
                .map_err(|error| advanced_execution(error.to_string()))?;
            (result.changed, result.changed.then_some(result.action_name))
        } else {
            (false, None)
        };
        let response_keyframes = keyframes
            .iter()
            .map(|keyframe| {
                json!({
                    "frame": keyframe.frame,
                    "position": {"x": keyframe.value.a, "y": keyframe.value.b},
                    "interpolation": "linear"
                })
            })
            .collect::<Vec<_>>();
        Ok(AdvancedWorkflowCommit {
            result: json!({
                "clipId": clip.id,
                "applied": applied,
                "algorithm": "opentake.region-block-match",
                "algorithmVersion": 1,
                "minimumConfidence": tracked.minimum_confidence,
                "region": {"x": region.x, "y": region.y, "width": region.width, "height": region.height},
                "keyframes": response_keyframes
            }),
            action_name,
        })
    }

    fn generate_matte(
        &self,
        args: GenerateMatteArgs,
        cancel: &MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        if cancel.checkpoint() {
            return Err(cancelled_workflow("advanced video workflow cancelled"));
        }
        let snapshot = self.core.runtime_snapshot();
        let clip = snapshot
            .timeline
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .find(|clip| clip.id == args.clip_id)
            .cloned()
            .ok_or_else(|| advanced_resource(format!("clip not found: {}", args.clip_id)))?;
        if clip.media_type != opentake_domain::ClipType::Video
            || clip.nested_sequence_id.is_some()
            || clip.reversed
            || (clip.speed - 1.0).abs() > f64::EPSILON
        {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CapabilityUnavailable,
                "matting currently requires an ordinary forward 1x video clip",
            ));
        }
        let installed = verify_rvm_model(&self.models_dir).map_err(|error| {
            AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CapabilityUnavailable,
                error.to_string(),
            )
        })?;
        if args
            .model
            .as_deref()
            .is_some_and(|model| model != installed.id)
        {
            return Err(advanced_invalid(format!(
                "unsupported matting model; installed model is {}",
                installed.id
            )));
        }
        let clip_end = clip.start_frame + clip.duration_frames;
        let start = args.start_frame.unwrap_or(clip.start_frame);
        let end = args.end_frame.unwrap_or(clip_end);
        if start < clip.start_frame || end > clip_end || start >= end {
            return Err(advanced_invalid(
                "startFrame/endFrame must be a non-empty range inside the clip",
            ));
        }
        let (source_path, is_video) =
            crate::transcribe::resolve_asset_from_snapshot(&snapshot, &clip.media_ref)
                .map_err(advanced_resource)?;
        if !is_video {
            return Err(advanced_invalid("matting source is not a video"));
        }
        let source_sha256 =
            file_sha256(&source_path).map_err(|error| advanced_execution(error.to_string()))?;
        let key_seed = format!(
            "{}|{}|{}|{}|{}|{}|{}",
            source_sha256,
            installed.sha256,
            clip.trim_start_frame,
            clip.duration_frames,
            snapshot.timeline.fps,
            start,
            end
        );
        let cache_key = format!("{:x}", Sha256::digest(key_seed.as_bytes()));
        let cache_dir = self.cache_root.join("matting");
        std::fs::create_dir_all(&cache_dir)
            .map_err(|error| advanced_execution(error.to_string()))?;
        let cache_path = cache_dir.join(format!("{cache_key}.mov"));
        if !cache_path.exists() {
            materialize_matte_video(
                &source_path,
                &cache_path,
                &clip,
                snapshot.timeline.fps,
                start,
                end,
                &self.models_dir,
                cancel,
            )?;
        }
        let mut cached_file = open_verified_regular_file(&cache_path)?;
        let output_probe = opentake_media::probe::probe_file(&cached_file)
            .map_err(|error| advanced_execution(error.to_string()))?;
        let expected_duration = clip.duration_frames as f64 / snapshot.timeline.fps.max(1) as f64;
        if !output_probe.has_video
            || output_probe.width.is_none()
            || output_probe.height.is_none()
            || output_probe
                .fps
                .is_none_or(|fps| (fps - snapshot.timeline.fps as f64).abs() > 0.01)
            || (output_probe.duration_secs - expected_duration).abs()
                > 1.5 / snapshot.timeline.fps.max(1) as f64
        {
            return Err(advanced_execution("matting output probe failed"));
        }
        let apply_requested = args.apply.unwrap_or(false);
        let mut asset_id = None;
        let mut action_name = None;
        let applied = if apply_requested {
            let project_dir = snapshot.project_dir.clone().ok_or_else(|| {
                advanced_invalid("save the project before applying a generated matte")
            })?;
            let project_media = crate::library::ProjectMediaCapability::open_verified(
                &self.core,
                snapshot.project_epoch,
                &project_dir,
                true,
            )
            .map_err(advanced_execution)?;
            let leaf_name = format!("matte-{}.mov", uuid::Uuid::new_v4());
            let mut published = project_media
                .create_import(Path::new(&leaf_name))
                .map_err(advanced_execution)?;
            cached_file
                .seek(SeekFrom::Start(0))
                .map_err(|error| advanced_execution(error.to_string()))?;
            std::io::copy(&mut cached_file, published.file_mut())
                .and_then(|_| published.file_mut().flush())
                .and_then(|_| published.file().sync_all())
                .map_err(|error| advanced_execution(error.to_string()))?;
            if !project_media
                .matches_leaf(&published)
                .map_err(advanced_execution)?
            {
                return Err(advanced_execution(
                    "matting output identity changed before project commit",
                ));
            }
            let provenance = GenerationInput {
                prompt: json!({"kind":"aiMatte","startFrame":start,"endFrame":end}).to_string(),
                model: installed.id.clone(),
                duration: clip.duration_frames,
                aspect_ratio: format!(
                    "{}:{}",
                    output_probe.width.unwrap_or(0),
                    output_probe.height.unwrap_or(0)
                ),
                provider: Some("opentake-matting".into()),
                status: Some(GenerationJobStatus::Ready),
                source_asset_id: Some(clip.media_ref.clone()),
                source_clip_id: Some(clip.id.clone()),
                source_start_frame: Some(start),
                source_end_frame: Some(end),
                ..GenerationInput::default()
            };
            let committed = self.core.commit_motion_media_for_project(
                snapshot.project_epoch,
                snapshot.version,
                &project_dir,
                published.path(),
                "AI Matte",
                &ProbedMedia {
                    duration_secs: output_probe.duration_secs,
                    width: output_probe
                        .width
                        .and_then(|value| i32::try_from(value).ok()),
                    height: output_probe
                        .height
                        .and_then(|value| i32::try_from(value).ok()),
                    fps: output_probe.fps,
                    has_audio: output_probe.has_audio,
                    color: output_probe.color,
                },
                provenance,
                MotionPlacement::Replace {
                    clip_id: clip.id.clone(),
                },
            );
            let committed = match committed {
                Ok(committed) => committed,
                Err(error) => return Err(advanced_execution(error.to_string())),
            };
            published.commit();
            asset_id = Some(committed.media.id);
            action_name = Some(committed.edit.action_name);
            true
        } else {
            false
        };
        Ok(AdvancedWorkflowCommit {
            result: json!({
                "clipId": clip.id,
                "sourceMediaRef": clip.media_ref,
                "assetId": asset_id,
                "applied": applied,
                "cacheKey": cache_key,
                "previewPath": cache_path,
                "frameCount": clip.duration_frames,
                "width": output_probe.width,
                "height": output_probe.height,
                "fps": output_probe.fps,
                "model": installed.id,
                "modelSha256": installed.sha256,
                "sourceSha256": source_sha256,
                "startFrame": start,
                "endFrame": end
            }),
            action_name,
        })
    }

    fn remove_object(
        &self,
        args: RemoveObjectArgs,
        cancel: &MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        if cancel.checkpoint() {
            return Err(cancelled_workflow("object removal cancelled"));
        }
        const PROVIDER: &str = "opentake-local";
        const MODEL: &str = "opentake-boundary-fill-v1";
        if args
            .provider
            .as_deref()
            .is_some_and(|provider| provider != PROVIDER && provider != "local")
        {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CapabilityUnavailable,
                "this build supports only the on-device opentake-local object-removal provider",
            ));
        }
        if args.model.as_deref().is_some_and(|model| model != MODEL) {
            return Err(advanced_invalid(format!(
                "unsupported object-removal model; available model is {MODEL}"
            )));
        }
        let snapshot = self.core.runtime_snapshot();
        let clip = snapshot
            .timeline
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .find(|clip| clip.id == args.clip_id)
            .cloned()
            .ok_or_else(|| advanced_resource(format!("clip not found: {}", args.clip_id)))?;
        if clip.media_type != opentake_domain::ClipType::Video
            || clip.nested_sequence_id.is_some()
            || clip.reversed
            || (clip.speed - 1.0).abs() > f64::EPSILON
        {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CapabilityUnavailable,
                "object removal currently requires an ordinary forward 1x video clip",
            ));
        }
        let mask_index = parse_mask_index(&args.mask_id)?;
        let mask = clip.masks.get(mask_index).cloned().ok_or_else(|| {
            advanced_resource(format!(
                "mask not found: {} (clip has {} masks)",
                args.mask_id,
                clip.masks.len()
            ))
        })?;
        let clip_end = clip.start_frame + clip.duration_frames;
        let start = args.start_frame.unwrap_or(clip.start_frame);
        let end = args.end_frame.unwrap_or(clip_end);
        if start < clip.start_frame || end > clip_end || start >= end {
            return Err(advanced_invalid(
                "startFrame/endFrame must be a non-empty range inside the clip",
            ));
        }
        let (source_path, is_video) =
            crate::transcribe::resolve_asset_from_snapshot(&snapshot, &clip.media_ref)
                .map_err(advanced_resource)?;
        if !is_video {
            return Err(advanced_invalid("object-removal source is not a video"));
        }
        let source_sha256 =
            file_sha256(&source_path).map_err(|error| advanced_execution(error.to_string()))?;
        let mask_json =
            serde_json::to_string(&mask).map_err(|error| advanced_execution(error.to_string()))?;
        let key_seed = format!(
            "{source_sha256}|{MODEL}|{}|{}|{}|{start}|{end}|{mask_json}",
            clip.trim_start_frame, clip.duration_frames, snapshot.timeline.fps
        );
        let cache_key = format!("{:x}", Sha256::digest(key_seed.as_bytes()));
        let cache_dir = self.cache_root.join("object-removal");
        std::fs::create_dir_all(&cache_dir)
            .map_err(|error| advanced_execution(error.to_string()))?;
        let cache_path = cache_dir.join(format!("{cache_key}.mov"));
        if !cache_path.exists() {
            materialize_object_removal_video(
                &source_path,
                &cache_path,
                &clip,
                snapshot.timeline.fps,
                start,
                end,
                &mask,
                cancel,
            )?;
        }
        if cancel.checkpoint() {
            return Err(cancelled_workflow("object removal cancelled"));
        }
        let mut cached_file = open_verified_regular_file(&cache_path)?;
        let output_probe = opentake_media::probe::probe_file(&cached_file)
            .map_err(|error| advanced_execution(error.to_string()))?;
        let expected_duration = clip.duration_frames as f64 / snapshot.timeline.fps.max(1) as f64;
        if !output_probe.has_video
            || output_probe.width.is_none()
            || output_probe.height.is_none()
            || output_probe
                .fps
                .is_none_or(|fps| (fps - snapshot.timeline.fps as f64).abs() > 0.01)
            || (output_probe.duration_secs - expected_duration).abs()
                > 1.5 / snapshot.timeline.fps.max(1) as f64
        {
            return Err(advanced_execution("object-removal output probe failed"));
        }

        let mut asset_id = None;
        let mut action_name = None;
        let applied = if args.apply.unwrap_or(false) {
            let project_dir = snapshot.project_dir.clone().ok_or_else(|| {
                advanced_invalid("save the project before applying object removal")
            })?;
            let project_media = crate::library::ProjectMediaCapability::open_verified(
                &self.core,
                snapshot.project_epoch,
                &project_dir,
                true,
            )
            .map_err(advanced_execution)?;
            let leaf_name = format!("object-removed-{}.mov", uuid::Uuid::new_v4());
            let mut published = project_media
                .create_import(Path::new(&leaf_name))
                .map_err(advanced_execution)?;
            cached_file
                .seek(SeekFrom::Start(0))
                .map_err(|error| advanced_execution(error.to_string()))?;
            std::io::copy(&mut cached_file, published.file_mut())
                .and_then(|_| published.file_mut().flush())
                .and_then(|_| published.file().sync_all())
                .map_err(|error| advanced_execution(error.to_string()))?;
            if !project_media
                .matches_leaf(&published)
                .map_err(advanced_execution)?
            {
                return Err(advanced_execution(
                    "object-removal output identity changed before project commit",
                ));
            }
            let provenance = GenerationInput {
                prompt: json!({
                    "kind":"objectRemoval",
                    "maskIndex":mask_index,
                    "mask":mask,
                    "startFrame":start,
                    "endFrame":end
                })
                .to_string(),
                model: MODEL.into(),
                duration: clip.duration_frames,
                aspect_ratio: format!(
                    "{}:{}",
                    output_probe.width.unwrap_or(0),
                    output_probe.height.unwrap_or(0)
                ),
                provider: Some(PROVIDER.into()),
                status: Some(GenerationJobStatus::Ready),
                source_asset_id: Some(clip.media_ref.clone()),
                source_clip_id: Some(clip.id.clone()),
                source_start_frame: Some(start),
                source_end_frame: Some(end),
                ..GenerationInput::default()
            };
            let committed = self
                .core
                .commit_generated_media_for_project(
                    snapshot.project_epoch,
                    snapshot.version,
                    &project_dir,
                    published.path(),
                    "Object Removed",
                    opentake_domain::ClipType::Video,
                    &ProbedMedia {
                        duration_secs: output_probe.duration_secs,
                        width: output_probe
                            .width
                            .and_then(|value| i32::try_from(value).ok()),
                        height: output_probe
                            .height
                            .and_then(|value| i32::try_from(value).ok()),
                        fps: output_probe.fps,
                        has_audio: output_probe.has_audio,
                        color: output_probe.color,
                    },
                    provenance,
                    MotionPlacement::ReplaceAndClearMasks {
                        clip_id: clip.id.clone(),
                    },
                    "Remove Masked Object",
                )
                .map_err(|error| advanced_execution(error.to_string()))?;
            published.commit();
            asset_id = Some(committed.media.id);
            action_name = Some(committed.edit.action_name);
            true
        } else {
            false
        };

        Ok(AdvancedWorkflowCommit {
            result: json!({
                "clipId": clip.id,
                "sourceMediaRef": clip.media_ref,
                "assetId": asset_id,
                "applied": applied,
                "cacheKey": cache_key,
                "previewPath": cache_path,
                "frameCount": clip.duration_frames,
                "width": output_probe.width,
                "height": output_probe.height,
                "fps": output_probe.fps,
                "provider": PROVIDER,
                "model": MODEL,
                "sourceSha256": source_sha256,
                "maskIndex": mask_index,
                "startFrame": start,
                "endFrame": end
            }),
            action_name,
        })
    }

    fn match_color(
        &self,
        args: MatchColorArgs,
        cancel: &MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        if cancel.checkpoint() {
            return Err(cancelled_workflow("color match cancelled"));
        }
        const ALGORITHM: &str = "opentake-luma-preserving-mean-match";
        const ALGORITHM_VERSION: u32 = 1;
        let snapshot = self.core.runtime_snapshot();
        let clip = snapshot
            .timeline
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .find(|clip| clip.id == args.clip_id)
            .cloned()
            .ok_or_else(|| advanced_resource(format!("clip not found: {}", args.clip_id)))?;
        if !matches!(
            clip.media_type,
            opentake_domain::ClipType::Image | opentake_domain::ClipType::Video
        ) || clip.nested_sequence_id.is_some()
            || clip.reversed
            || (clip.speed - 1.0).abs() > f64::EPSILON
        {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CapabilityUnavailable,
                "color match requires an ordinary forward 1x visual clip",
            ));
        }
        if clip.color_grade.is_some_and(|grade| !grade.is_identity())
            || clip.lut.is_some()
            || clip.chroma_key.is_some()
            || !clip.masks.is_empty()
            || !clip.effects.is_empty()
        {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CapabilityUnavailable,
                "color match currently requires a target clip without active pixel effects",
            ));
        }
        if args.reference_media_ref == clip.media_ref {
            return Err(advanced_invalid(
                "reference media must differ from the target source",
            ));
        }
        let reference = snapshot
            .media
            .entries
            .iter()
            .find(|entry| entry.id == args.reference_media_ref)
            .ok_or_else(|| {
                advanced_resource(format!(
                    "reference media not found: {}",
                    args.reference_media_ref
                ))
            })?;
        if !matches!(
            reference.kind,
            opentake_domain::ClipType::Image | opentake_domain::ClipType::Video
        ) {
            return Err(advanced_invalid(
                "reference media must be an image or video",
            ));
        }
        let target_frame = args.target_frame.unwrap_or(clip.start_frame);
        if !clip.contains(target_frame) {
            return Err(advanced_invalid(
                "targetFrame must be inside the target clip",
            ));
        }
        let reference_frame = args.reference_frame.unwrap_or(0);
        if reference_frame < 0 {
            return Err(advanced_invalid("referenceFrame must be non-negative"));
        }
        if reference.kind == opentake_domain::ClipType::Image && reference_frame != 0 {
            return Err(advanced_invalid(
                "image references support only referenceFrame=0",
            ));
        }

        let (target_path, _) =
            crate::transcribe::resolve_asset_from_snapshot(&snapshot, &clip.media_ref)
                .map_err(advanced_resource)?;
        let (reference_path, _) =
            crate::transcribe::resolve_asset_from_snapshot(&snapshot, &args.reference_media_ref)
                .map_err(advanced_resource)?;
        let timeline_fps = snapshot.timeline.fps.max(1) as f64;
        let target_time = (clip.trim_start_frame as f64
            + (target_frame - clip.start_frame) as f64 * clip.speed)
            / timeline_fps;
        let reference_time = if reference.kind == opentake_domain::ClipType::Image {
            0.0
        } else {
            reference_frame as f64 / reference.source_fps.unwrap_or(timeline_fps).max(0.001)
        };
        if reference.kind == opentake_domain::ClipType::Video
            && reference.duration > 0.0
            && reference_time >= reference.duration
        {
            return Err(advanced_invalid(
                "referenceFrame must be inside the reference video",
            ));
        }
        let request = FrameRequest {
            max_size: (640, 640),
            tolerance_secs: 0.1,
            ..FrameRequest::default()
        };
        let target = decode_color_sample(
            &target_path,
            target_time,
            clip.media_type == opentake_domain::ClipType::Image,
            &request,
            cancel,
        )?;
        let reference_frame_data = decode_color_sample(
            &reference_path,
            reference_time,
            reference.kind == opentake_domain::ClipType::Image,
            &request,
            cancel,
        )?;
        let target_mean = mean_linear_rgb(&target)?;
        let reference_mean = mean_linear_rgb(&reference_frame_data)?;
        let target_luma = luma709(target_mean.r, target_mean.g, target_mean.b);
        let reference_luma = luma709(reference_mean.r, reference_mean.g, reference_mean.b);
        if target_luma <= 0.005
            || reference_luma <= 0.005
            || [target_mean.r, target_mean.g, target_mean.b]
                .into_iter()
                .any(|channel| channel <= 0.001)
        {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::AnalysisLowConfidence,
                "color match needs non-black samples with usable values in every channel",
            ));
        }
        let luma_scale = target_luma / reference_luma;
        let desired = Rgb::new(
            reference_mean.r * luma_scale,
            reference_mean.g * luma_scale,
            reference_mean.b * luma_scale,
        );
        let grade = ColorGrade {
            lift_gamma_gain: LiftGammaGain {
                gain: Rgb::new(
                    (desired.r / target_mean.r).clamp(0.0, 4.0),
                    (desired.g / target_mean.g).clamp(0.0, 4.0),
                    (desired.b / target_mean.b).clamp(0.0, 4.0),
                ),
                ..LiftGammaGain::default()
            },
            ..ColorGrade::default()
        };
        grade
            .validate()
            .map_err(|error| advanced_execution(error.to_string()))?;
        let (matched_r, matched_g, matched_b) =
            grade.apply_linear(target_mean.r, target_mean.g, target_mean.b);
        let matched_mean = Rgb::new(matched_r, matched_g, matched_b);
        let delta_e_before = delta_e76(target_mean, desired);
        let delta_e_after = delta_e76(matched_mean, desired);
        let target_luma_after = luma709(matched_r, matched_g, matched_b);
        if delta_e_after >= delta_e_before || (target_luma_after - target_luma).abs() > 0.02 {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::AnalysisLowConfidence,
                "color match could not improve the sample within the luma-preservation limit",
            ));
        }
        let input = ColorMatchInput {
            reference_media_ref: args.reference_media_ref.clone(),
            reference_frame,
            target_frame,
            algorithm: ALGORITHM.into(),
            algorithm_version: ALGORITHM_VERSION,
            target_mean_linear: target_mean,
            reference_mean_linear: reference_mean,
            delta_e_before,
            delta_e_after,
            target_luma_before: target_luma,
            target_luma_after,
        };
        let apply_requested = args.apply.unwrap_or(false);
        let (applied, action_name) = if apply_requested {
            let result = self
                .core
                .apply_at_revision(
                    ProjectRevision {
                        project_epoch: snapshot.project_epoch,
                        version: snapshot.version,
                    },
                    EditCommand::ApplyColorMatch {
                        clip_id: clip.id.clone(),
                        grade,
                        input: input.clone(),
                    },
                )
                .map_err(|error| advanced_execution(error.to_string()))?;
            (result.changed, result.changed.then_some(result.action_name))
        } else {
            (false, None)
        };

        Ok(AdvancedWorkflowCommit {
            result: json!({
                "clipId": clip.id,
                "referenceMediaRef": args.reference_media_ref,
                "referenceFrame": reference_frame,
                "targetFrame": target_frame,
                "algorithm": ALGORITHM,
                "algorithmVersion": ALGORITHM_VERSION,
                "grade": grade,
                "targetMeanLinear": target_mean,
                "referenceMeanLinear": reference_mean,
                "matchedMeanLinear": matched_mean,
                "deltaEBefore": delta_e_before,
                "deltaEAfter": delta_e_after,
                "targetLumaBefore": target_luma,
                "targetLumaAfter": target_luma_after,
                "applied": applied
            }),
            action_name,
        })
    }

    fn separate_stems(
        &self,
        args: SeparateStemsArgs,
        cancel: &MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        const MODEL: &str = "opentake-center-v1";
        if cancel.checkpoint() {
            return Err(cancelled_workflow("stem separation cancelled"));
        }
        if args
            .provider
            .as_deref()
            .is_some_and(|provider| provider != "local" && provider != "opentake-local")
        {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CapabilityUnavailable,
                "this build supports only the on-device stem provider",
            ));
        }
        if args.model.as_deref().is_some_and(|model| model != MODEL) {
            return Err(advanced_invalid(format!(
                "unsupported local stem model; available model is {MODEL}"
            )));
        }
        let requested_start_frame = args.start_frame.unwrap_or(0);
        if args.import_to_tracks.unwrap_or(false) && requested_start_frame < 0 {
            return Err(advanced_invalid("startFrame must be non-negative"));
        }
        let snapshot = self.core.runtime_snapshot();
        let project_dir = snapshot
            .project_dir
            .clone()
            .ok_or_else(|| advanced_invalid("save the project before separating stems"))?;
        let source = snapshot
            .media
            .entries
            .iter()
            .find(|entry| entry.id == args.media_ref)
            .cloned()
            .ok_or_else(|| advanced_resource(format!("media not found: {}", args.media_ref)))?;
        if !matches!(
            source.kind,
            opentake_domain::ClipType::Audio | opentake_domain::ClipType::Video
        ) || !source
            .has_audio
            .unwrap_or(source.kind == opentake_domain::ClipType::Audio)
        {
            return Err(advanced_invalid("stem source must contain audio"));
        }
        let (source_path, _) =
            crate::transcribe::resolve_asset_from_snapshot(&snapshot, &args.media_ref)
                .map_err(advanced_resource)?;
        let output_dir = project_dir
            .join(opentake_project::layout::MEDIA_DIR)
            .join(format!("stems-{}", uuid::Uuid::new_v4()));
        let separated = opentake_media::analysis::separate_stems(
            StemSeparationRequest {
                source: &source_path,
                output_dir: &output_dir,
                execution: StemExecution::Local {
                    model_dir: &self.models_dir,
                },
            },
            cancel,
            None,
        )
        .map_err(|error| {
            let _ = std::fs::remove_dir_all(&output_dir);
            media_workflow_error(error)
        })?;
        if cancel.checkpoint() {
            let _ = std::fs::remove_dir_all(&output_dir);
            return Err(cancelled_workflow("stem separation cancelled"));
        }
        let vocals_probe = probe(&separated.vocals.path).map_err(media_workflow_error)?;
        let accompaniment_probe =
            probe(&separated.accompaniment.path).map_err(media_workflow_error)?;
        let probed = |value: &opentake_media::probe::MediaProbe| ProbedMedia {
            duration_secs: value.duration_secs,
            width: value.width.and_then(|width| i32::try_from(width).ok()),
            height: value.height.and_then(|height| i32::try_from(height).ok()),
            fps: value.fps,
            has_audio: value.has_audio,
            color: value.color.clone(),
        };
        let provenance = |stem: &str| DerivedStemProvenance {
            source_asset_id: args.media_ref.clone(),
            source_sha256: separated.provenance.source_sha256.clone(),
            execution: separated.provenance.execution.clone(),
            model_sha256: separated.provenance.model_sha256.clone(),
            stem: stem.into(),
        };
        let committed = self
            .core
            .import_media_batch_for_project_persisted(
                snapshot.project_epoch,
                &project_dir,
                vec![
                    PreparedMediaImportOp::ImportDerivedStem {
                        path: separated.vocals.path,
                        name: separated.vocals.name,
                        probe: probed(&vocals_probe),
                        provenance: provenance("vocals"),
                    },
                    PreparedMediaImportOp::ImportDerivedStem {
                        path: separated.accompaniment.path,
                        name: separated.accompaniment.name,
                        probe: probed(&accompaniment_probe),
                        provenance: provenance("accompaniment"),
                    },
                ],
            )
            .map_err(|error| {
                let _ = std::fs::remove_dir_all(&output_dir);
                advanced_execution(error.to_string())
            })?;
        if committed.len() != 2 {
            return Err(advanced_execution("stem import did not return two assets"));
        }
        let vocals_asset_id = committed[0].entry.id.clone();
        let accompaniment_asset_id = committed[1].entry.id.clone();
        let mut clip_ids = Vec::new();
        let mut action_name = None;
        if args.import_to_tracks.unwrap_or(false) {
            let start_frame = requested_start_frame;
            let current = self.core.runtime_snapshot();
            let duration_frames = (vocals_probe.duration_secs * current.timeline.fps.max(1) as f64)
                .round()
                .max(1.0) as i32;
            let entry = |media_ref: String| ClipEntry {
                media_ref,
                media_type: opentake_domain::ClipType::Audio,
                source_clip_type: opentake_domain::ClipType::Audio,
                track_index: 0,
                start_frame,
                duration_frames,
                trim_start_frame: None,
                trim_end_frame: None,
                has_audio: true,
                add_linked_audio: false,
                transform: None,
            };
            let placed = self
                .core
                .apply_at_revision(
                    ProjectRevision {
                        project_epoch: current.project_epoch,
                        version: current.version,
                    },
                    EditCommand::AddClipsToSeparateAutoTracks {
                        entries: vec![
                            entry(vocals_asset_id.clone()),
                            entry(accompaniment_asset_id.clone()),
                        ],
                    },
                )
                .map_err(|error| advanced_execution(error.to_string()))?;
            clip_ids = placed.affected_clip_ids;
            action_name = Some(placed.action_name);
        }
        Ok(AdvancedWorkflowCommit {
            result: json!({
                "sourceMediaRef": args.media_ref,
                "vocalsAssetId": vocals_asset_id,
                "accompanimentAssetId": accompaniment_asset_id,
                "sourceSha256": separated.provenance.source_sha256,
                "execution": separated.provenance.execution,
                "modelSha256": separated.provenance.model_sha256,
                "vocalSdrImprovementDb": separated.metrics.vocal_sdr_improvement_db,
                "importedToTracks": args.import_to_tracks.unwrap_or(false),
                "clipIds": clip_ids
            }),
            action_name,
        })
    }

    fn translate_captions(
        &self,
        args: TranslateCaptionsArgs,
        cancel: &MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        if cancel.checkpoint() {
            return Err(cancelled_workflow("caption translation cancelled"));
        }
        if args.caption_clip_ids.is_empty() || args.caption_clip_ids.len() > 500 {
            return Err(advanced_invalid(
                "captionClipIds must contain between 1 and 500 captions",
            ));
        }
        let source_locale = args.source_locale.as_deref().unwrap_or("auto").trim();
        let target_locale = args.target_locale.trim();
        if source_locale.is_empty()
            || source_locale.len() > 64
            || target_locale.is_empty()
            || target_locale.len() > 64
            || source_locale.eq_ignore_ascii_case(target_locale)
        {
            return Err(advanced_invalid(
                "sourceLocale and targetLocale must be distinct locale identifiers",
            ));
        }
        let provider = args.provider.as_deref().unwrap_or("openai");
        let model = args.model.as_deref().unwrap_or(match provider {
            "openai" => "gpt-4o-mini",
            "anthropic" => "claude-3-5-haiku-latest",
            _ => "",
        });
        if model.trim().is_empty() || model.len() > 128 {
            return Err(advanced_invalid("model must be a valid model identifier"));
        }
        if args.cost_authorized != Some(true) {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CostAuthorizationRequired,
                "caption translation sends subtitle text to the selected paid provider; set costAuthorized=true after review",
            ));
        }
        let snapshot = self.core.runtime_snapshot();
        let mut seen = std::collections::HashSet::new();
        let mut originals = Vec::with_capacity(args.caption_clip_ids.len());
        let mut source_bytes = 0_usize;
        for id in &args.caption_clip_ids {
            if !seen.insert(id.as_str()) {
                return Err(advanced_invalid(format!("duplicate caption clip: {id}")));
            }
            let clip = snapshot
                .timeline
                .tracks
                .iter()
                .flat_map(|track| &track.clips)
                .find(|clip| clip.id == *id)
                .ok_or_else(|| advanced_resource(format!("caption clip not found: {id}")))?;
            if clip.media_type != opentake_domain::ClipType::Text || clip.caption_group_id.is_none()
            {
                return Err(advanced_invalid(format!("clip is not a caption: {id}")));
            }
            let text = clip
                .text_content
                .as_deref()
                .filter(|text| !text.trim().is_empty())
                .ok_or_else(|| advanced_invalid(format!("caption text is empty: {id}")))?;
            if text.len() > 20_000 {
                return Err(advanced_invalid(format!("caption text is too large: {id}")));
            }
            source_bytes = source_bytes.saturating_add(text.len());
            if source_bytes > 1_000_000 {
                return Err(advanced_invalid(
                    "selected caption text exceeds the 1 MB translation limit",
                ));
            }
            originals.push(CaptionTranslationDraft {
                id: id.clone(),
                text: text.to_string(),
            });
        }
        let returned = self.caption_translator.translate(
            provider,
            model,
            source_locale,
            target_locale,
            &originals,
            cancel,
        )?;
        if returned
            .translations
            .len()
            .saturating_add(returned.errors.len())
            > originals.len().saturating_mul(2)
        {
            return Err(advanced_execution(
                "translation provider returned too many caption results",
            ));
        }
        let original_by_id: std::collections::HashMap<&str, &str> = originals
            .iter()
            .map(|caption| (caption.id.as_str(), caption.text.as_str()))
            .collect();
        let mut response_ids = std::collections::HashSet::new();
        let mut translations = Vec::new();
        let mut failures = Vec::new();
        for translation in returned.translations {
            if !original_by_id.contains_key(translation.id.as_str())
                || !response_ids.insert(translation.id.clone())
            {
                return Err(advanced_execution(
                    "translation provider returned an unknown or duplicate caption id",
                ));
            }
            if translation.text.trim().is_empty() || translation.text.len() > 20_000 {
                failures.push(CaptionTranslationFailure {
                    id: translation.id,
                    message: "provider returned empty or oversized translated text".into(),
                });
            } else {
                translations.push(translation);
            }
        }
        for failure in returned.errors {
            if !original_by_id.contains_key(failure.id.as_str())
                || !response_ids.insert(failure.id.clone())
            {
                return Err(advanced_execution(
                    "translation provider returned an unknown or duplicate caption id",
                ));
            }
            failures.push(CaptionTranslationFailure {
                id: failure.id,
                message: if failure.message.trim().is_empty() {
                    "provider could not translate this caption".into()
                } else {
                    failure.message.chars().take(512).collect()
                },
            });
        }
        for original in &originals {
            if !response_ids.contains(&original.id) {
                failures.push(CaptionTranslationFailure {
                    id: original.id.clone(),
                    message: "provider omitted this caption".into(),
                });
            }
        }
        if translations.is_empty() {
            return Err(advanced_execution(
                "caption translation produced no reviewable changes",
            ));
        }
        if cancel.checkpoint() {
            return Err(cancelled_workflow("caption translation cancelled"));
        }
        let changes: Vec<CaptionTranslationChange> = translations
            .iter()
            .map(|translation| {
                let source_text = original_by_id[translation.id.as_str()].to_string();
                CaptionTranslationChange {
                    clip_id: translation.id.clone(),
                    expected_source_text: source_text.clone(),
                    translated_text: translation.text.clone(),
                    input: CaptionTranslationInput {
                        source_text,
                        source_locale: source_locale.to_string(),
                        target_locale: target_locale.to_string(),
                        provider: provider.to_string(),
                        model: model.to_string(),
                    },
                }
            })
            .collect();
        let apply_requested = args.apply.unwrap_or(false);
        let (applied, action_name) = if apply_requested {
            let edit = self
                .core
                .apply_at_revision(
                    ProjectRevision {
                        project_epoch: snapshot.project_epoch,
                        version: snapshot.version,
                    },
                    EditCommand::ApplyCaptionTranslations { changes },
                )
                .map_err(|error| advanced_execution(error.to_string()))?;
            (edit.changed, edit.changed.then_some(edit.action_name))
        } else {
            (false, None)
        };
        let review: Vec<serde_json::Value> = translations
            .into_iter()
            .map(|translation| {
                let source_text = original_by_id[translation.id.as_str()];
                json!({
                    "id": translation.id,
                    "sourceText": source_text,
                    "translatedText": translation.text
                })
            })
            .collect();
        Ok(AdvancedWorkflowCommit {
            result: json!({
                "projectEpoch": snapshot.project_epoch,
                "version": snapshot.version,
                "sourceLocale": source_locale,
                "targetLocale": target_locale,
                "provider": provider,
                "model": model,
                "review": review,
                "errors": failures,
                "captionCount": originals.len(),
                "translatedCount": review.len(),
                "applied": applied
            }),
            action_name,
        })
    }

    fn script_to_video(
        &self,
        args: ScriptToVideoArgs,
        cancel: &MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        if cancel.checkpoint() {
            return Err(cancelled_workflow("script assembly cancelled"));
        }
        if args.segments.is_empty() || args.segments.len() > 100 {
            return Err(advanced_invalid(
                "segments must contain between 1 and 100 reviewed segments",
            ));
        }
        let segment_count = args.segments.len();
        let snapshot = self.core.runtime_snapshot();
        let start_frame = snapshot.timeline.total_frames();
        let fps = snapshot.timeline.fps.max(1) as f64;
        let mut segments = Vec::with_capacity(args.segments.len());
        for (index, value) in args.segments.into_iter().enumerate() {
            let segment: ScriptSegmentArg = serde_json::from_value(value)
                .map_err(|error| advanced_invalid(format!("segments[{index}]: {error}")))?;
            if segment.script.trim().is_empty()
                || segment.script.len() > 20_000
                || !(1..=36_000).contains(&segment.duration_frames)
            {
                return Err(advanced_invalid(format!(
                    "segments[{index}] requires non-empty script and durationFrames 1..36000"
                )));
            }
            let media = snapshot
                .media
                .entries
                .iter()
                .find(|entry| entry.id == segment.media_ref)
                .ok_or_else(|| {
                    advanced_resource(format!(
                        "segments[{index}] media not found: {}",
                        segment.media_ref
                    ))
                })?;
            if !matches!(
                media.kind,
                opentake_domain::ClipType::Image
                    | opentake_domain::ClipType::Video
                    | opentake_domain::ClipType::Lottie
            ) {
                return Err(advanced_invalid(format!(
                    "segments[{index}].mediaRef must be visual media"
                )));
            }
            if media.kind == opentake_domain::ClipType::Video
                && media.duration > 0.0
                && (media.duration * fps).round() as i32 + 1 < segment.duration_frames
            {
                return Err(advanced_invalid(format!(
                    "segments[{index}] is longer than its video source"
                )));
            }
            if let Some(narration_ref) = &segment.narration_media_ref {
                let narration = snapshot
                    .media
                    .entries
                    .iter()
                    .find(|entry| entry.id == *narration_ref)
                    .ok_or_else(|| {
                        advanced_resource(format!(
                            "segments[{index}] narration not found: {narration_ref}"
                        ))
                    })?;
                if !matches!(
                    narration.kind,
                    opentake_domain::ClipType::Audio | opentake_domain::ClipType::Video
                ) || !narration
                    .has_audio
                    .unwrap_or(narration.kind == opentake_domain::ClipType::Audio)
                {
                    return Err(advanced_invalid(format!(
                        "segments[{index}].narrationMediaRef must contain audio"
                    )));
                }
                let narration_frames = (narration.duration * fps).round() as i32;
                if narration.duration > 0.0
                    && (narration_frames - segment.duration_frames).abs() > 1
                {
                    return Err(advanced_invalid(format!(
                        "segments[{index}] narration duration must match within one frame"
                    )));
                }
            }
            let transition = match segment.transition.as_deref() {
                None => None,
                Some("crossDissolve") => Some(TransitionKind::CrossDissolve),
                Some(other) => {
                    return Err(advanced_invalid(format!(
                        "segments[{index}].transition is unsupported: {other}"
                    )))
                }
            };
            if transition.is_some() && index + 1 == segment_count {
                return Err(advanced_invalid(
                    "the final script segment cannot have an outgoing transition",
                ));
            }
            segments.push(ScriptAssemblySegment {
                script: segment.script,
                media_ref: segment.media_ref,
                narration_media_ref: segment.narration_media_ref,
                duration_frames: segment.duration_frames,
                transition,
            });
        }
        for index in 0..segments.len().saturating_sub(1) {
            if segments[index].transition.is_some()
                && (segments[index].duration_frames < 2 || segments[index + 1].duration_frames < 2)
            {
                return Err(advanced_invalid(format!(
                    "segments[{index}] transition needs at least two frames on both sides"
                )));
            }
        }
        let canonical = serde_json::to_vec(&json!({
            "planner": "opentake-script-assembly",
            "plannerVersion": 1,
            "startFrame": start_frame,
            "segments": segments
        }))
        .map_err(|error| advanced_execution(error.to_string()))?;
        let plan_hash = format!("{:x}", Sha256::digest(&canonical));
        let plan = ScriptAssemblyPlan {
            id: format!("script-plan-{}", &plan_hash[..16]),
            plan_hash: plan_hash.clone(),
            planner: "opentake-script-assembly".into(),
            planner_version: 1,
            start_frame,
            segments,
        };
        let apply_requested = args.apply.unwrap_or(false);
        let (applied, action_name, version) = if apply_requested {
            if !snapshot
                .timeline
                .script_assembly_plans
                .iter()
                .any(|existing| existing == &plan)
            {
                return Err(advanced_invalid(
                    "preview and persist this exact script plan before apply=true",
                ));
            }
            if cancel.checkpoint() {
                return Err(cancelled_workflow("script assembly cancelled"));
            }
            let edit = self
                .core
                .apply_at_revision(
                    ProjectRevision {
                        project_epoch: snapshot.project_epoch,
                        version: snapshot.version,
                    },
                    EditCommand::ApplyScriptAssemblyPlan {
                        plan_id: plan.id.clone(),
                    },
                )
                .map_err(|error| advanced_execution(error.to_string()))?;
            let version = self.core.runtime_snapshot().version;
            (
                edit.changed,
                edit.changed.then_some(edit.action_name),
                version,
            )
        } else {
            if cancel.checkpoint() {
                return Err(cancelled_workflow("script planning cancelled"));
            }
            let edit = self
                .core
                .apply_at_revision(
                    ProjectRevision {
                        project_epoch: snapshot.project_epoch,
                        version: snapshot.version,
                    },
                    EditCommand::SaveScriptAssemblyPlan { plan: plan.clone() },
                )
                .map_err(|error| advanced_execution(error.to_string()))?;
            let version = self.core.runtime_snapshot().version;
            (false, edit.changed.then_some(edit.action_name), version)
        };
        let mut cursor = start_frame;
        let review: Vec<serde_json::Value> = plan
            .segments
            .iter()
            .map(|segment| {
                let segment_start = cursor;
                cursor += segment.duration_frames;
                json!({
                    "script": segment.script,
                    "mediaRef": segment.media_ref,
                    "narrationMediaRef": segment.narration_media_ref,
                    "startFrame": segment_start,
                    "durationFrames": segment.duration_frames,
                    "transition": segment.transition
                })
            })
            .collect();
        Ok(AdvancedWorkflowCommit {
            result: json!({
                "projectEpoch": snapshot.project_epoch,
                "version": version,
                "planId": plan.id,
                "planHash": plan_hash,
                "planner": plan.planner,
                "plannerVersion": plan.planner_version,
                "startFrame": start_frame,
                "endFrame": cursor,
                "segments": review,
                "applied": applied
            }),
            action_name,
        })
    }

    fn generate_avatar(
        &self,
        args: GenerateAvatarArgs,
        cancel: &MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        const PROVIDER: &str = "fal";
        const MODEL: &str = "fal-ai/sync-lipsync/v3/image-to-video";
        validate_consent_id(&args.consent_id)?;
        if args.cost_authorized != Some(true) {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CostAuthorizationRequired,
                "avatar generation sends the consented portrait and narration to a paid provider; set costAuthorized=true after review",
            ));
        }
        if args
            .provider
            .as_deref()
            .is_some_and(|value| value != PROVIDER)
            || args.model.as_deref().is_some_and(|value| value != MODEL)
        {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CapabilityUnavailable,
                format!("this build supports {PROVIDER}:{MODEL}"),
            ));
        }
        if cancel.checkpoint() {
            return Err(cancelled_workflow("avatar generation cancelled"));
        }
        let snapshot = self.core.runtime_snapshot();
        let project_dir = snapshot
            .project_dir
            .clone()
            .ok_or_else(|| advanced_invalid("save the project before generating an avatar"))?;
        let portrait = snapshot
            .media
            .entries
            .iter()
            .find(|entry| entry.id == args.portrait_media_ref)
            .ok_or_else(|| advanced_resource("portrait media does not exist"))?;
        if portrait.kind != opentake_domain::ClipType::Image {
            return Err(advanced_invalid("avatar portrait must be an image"));
        }
        let audio = snapshot
            .media
            .entries
            .iter()
            .find(|entry| entry.id == args.audio_media_ref)
            .ok_or_else(|| advanced_resource("avatar narration media does not exist"))?;
        if audio.kind != opentake_domain::ClipType::Audio || !audio.has_audio.unwrap_or(true) {
            return Err(advanced_invalid("avatar narration must be an audio asset"));
        }
        if audio.duration <= 0.0 || audio.duration > 60.0 * 30.0 {
            return Err(advanced_invalid(
                "avatar narration duration must be between zero and 30 minutes",
            ));
        }
        let (portrait_path, _) =
            crate::transcribe::resolve_asset_from_snapshot(&snapshot, &args.portrait_media_ref)
                .map_err(advanced_resource)?;
        let (audio_path, _) =
            crate::transcribe::resolve_asset_from_snapshot(&snapshot, &args.audio_media_ref)
                .map_err(advanced_resource)?;
        let portrait_sha256 = file_sha256(&portrait_path).map_err(media_workflow_error)?;
        let audio_sha256 = file_sha256(&audio_path).map_err(media_workflow_error)?;
        let request_hash = canonical_request_hash(&json!({
            "provider": PROVIDER,
            "model": MODEL,
            "consentId": args.consent_id,
            "portraitMediaRef": args.portrait_media_ref,
            "portraitSha256": portrait_sha256,
            "audioMediaRef": args.audio_media_ref,
            "audioSha256": audio_sha256,
        }))?;
        let media_dir = project_dir.join(opentake_project::layout::MEDIA_DIR);
        std::fs::create_dir_all(&media_dir)
            .map_err(|_| advanced_execution("project media directory is unavailable"))?;
        let destination = media_dir.join(format!("avatar-{}.mp4", uuid::Uuid::new_v4()));
        let output = self.avatar_provider.generate(
            &AvatarProviderRequest {
                portrait_path,
                audio_path,
                model: MODEL.into(),
                destination: destination.clone(),
            },
            cancel,
        )?;
        if cancel.checkpoint() {
            let _ = std::fs::remove_file(&destination);
            return Err(cancelled_workflow("avatar generation cancelled"));
        }
        let output_probe = probe(&destination).map_err(|error| {
            let _ = std::fs::remove_file(&destination);
            media_workflow_error(error)
        })?;
        let fps = snapshot.timeline.fps.max(1) as f64;
        if !output_probe.has_video
            || !output_probe.has_audio
            || output_probe.duration_secs <= 0.0
            || ((output_probe.duration_secs - audio.duration) * fps).abs() > 1.0
        {
            let _ = std::fs::remove_file(&destination);
            return Err(advanced_execution(
                "avatar output must contain synchronized video/audio matching narration within one frame",
            ));
        }
        let start_frame = args.start_frame.unwrap_or(snapshot.timeline.total_frames());
        if start_frame < 0 {
            let _ = std::fs::remove_file(&destination);
            return Err(advanced_invalid("startFrame must be non-negative"));
        }
        let duration_frames = (output_probe.duration_secs * fps).round().max(1.0) as i32;
        let aspect_ratio = output_probe
            .width
            .zip(output_probe.height)
            .map(|(width, height)| format!("{width}:{height}"))
            .unwrap_or_else(|| "avatar".into());
        let provenance = GenerationInput {
            prompt: "lip-synchronized avatar".into(),
            model: MODEL.into(),
            duration: output_probe.duration_secs.round() as i32,
            aspect_ratio,
            image_urls: Some(vec![format!("sha256:{portrait_sha256}")]),
            reference_image_asset_ids: Some(vec![args.portrait_media_ref.clone()]),
            reference_audio_urls: Some(vec![format!("sha256:{audio_sha256}")]),
            reference_audio_asset_ids: Some(vec![args.audio_media_ref.clone()]),
            source_asset_id: Some(args.portrait_media_ref.clone()),
            provider: Some(PROVIDER.into()),
            provider_job_id: Some(output.request_id.clone()),
            status: Some(GenerationJobStatus::Ready),
            progress: Some(1.0),
            consent_id: Some(args.consent_id.clone()),
            request_hash: Some(request_hash.clone()),
            ..GenerationInput::default()
        };
        let probed = ProbedMedia {
            duration_secs: output_probe.duration_secs,
            width: output_probe
                .width
                .and_then(|value| i32::try_from(value).ok()),
            height: output_probe
                .height
                .and_then(|value| i32::try_from(value).ok()),
            fps: output_probe.fps,
            has_audio: output_probe.has_audio,
            color: output_probe.color.clone(),
        };
        let committed = self
            .core
            .commit_generated_media_for_project(
                snapshot.project_epoch,
                snapshot.version,
                &project_dir,
                &destination,
                "Generated avatar",
                opentake_domain::ClipType::Video,
                &probed,
                provenance,
                MotionPlacement::Add {
                    start_frame,
                    duration_frames,
                    track_index: None,
                },
                "Generate Avatar",
            )
            .map_err(|error| {
                let _ = std::fs::remove_file(&destination);
                advanced_execution(error.to_string())
            })?;
        Ok(AdvancedWorkflowCommit {
            result: json!({
                "assetId": committed.media.id,
                "clipIds": committed.edit.affected_clip_ids,
                "previewPath": destination,
                "provider": PROVIDER,
                "model": MODEL,
                "providerRequestId": output.request_id,
                "requestHash": request_hash,
                "consentId": args.consent_id,
                "portraitMediaRef": args.portrait_media_ref,
                "audioMediaRef": args.audio_media_ref,
                "durationFrames": duration_frames,
                "mediaType": output.media_type,
                "imported": true
            }),
            action_name: Some(committed.edit.action_name),
        })
    }

    fn clone_voice(
        &self,
        args: CloneVoiceArgs,
        cancel: &MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        const PROVIDER: &str = "elevenlabs";
        const MODEL: &str = "eleven_multilingual_v2";
        validate_consent_id(&args.consent_id)?;
        if args
            .provider
            .as_deref()
            .is_some_and(|value| value != PROVIDER)
            || args.model.as_deref().is_some_and(|value| value != MODEL)
        {
            return Err(AdvancedWorkflowError::new(
                AdvancedWorkflowErrorKind::CapabilityUnavailable,
                format!("this build supports {PROVIDER}:{MODEL}"),
            ));
        }
        if cancel.checkpoint() {
            return Err(cancelled_workflow("voice workflow cancelled"));
        }
        let snapshot = self.core.runtime_snapshot();
        let project_dir = snapshot
            .project_dir
            .clone()
            .ok_or_else(|| advanced_invalid("save the project before using voice cloning"))?;
        match args.action.as_str() {
            "enroll" => {
                if args.cost_authorized != Some(true) {
                    return Err(AdvancedWorkflowError::new(
                        AdvancedWorkflowErrorKind::CostAuthorizationRequired,
                        "voice enrollment sends consented reference audio to a paid provider; set costAuthorized=true after review",
                    ));
                }
                let source_id = args.reference_audio_media_ref.as_deref().ok_or_else(|| {
                    advanced_invalid("referenceAudioMediaRef is required for enrollment")
                })?;
                let voice_name = args
                    .voice_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty() && value.len() <= 128)
                    .ok_or_else(|| advanced_invalid("voiceName is required and too long"))?;
                let source = snapshot
                    .media
                    .entries
                    .iter()
                    .find(|entry| entry.id == source_id)
                    .ok_or_else(|| advanced_resource("voice reference media does not exist"))?;
                if source.kind != opentake_domain::ClipType::Audio
                    || !source.has_audio.unwrap_or(true)
                {
                    return Err(advanced_invalid("voice reference must be an audio asset"));
                }
                let (reference_path, _) =
                    crate::transcribe::resolve_asset_from_snapshot(&snapshot, source_id)
                        .map_err(advanced_resource)?;
                let source_sha256 = file_sha256(&reference_path).map_err(media_workflow_error)?;
                let request_hash = canonical_request_hash(&json!({
                    "provider": PROVIDER,
                    "model": MODEL,
                    "consentId": args.consent_id,
                    "sourceAudioAssetId": source_id,
                    "sourceAudioSha256": source_sha256,
                    "voiceName": voice_name,
                }))?;
                let voice_model_id = format!("voice-model-{}", &request_hash[..16]);
                if snapshot
                    .timeline
                    .voice_models
                    .iter()
                    .any(|record| record.id == voice_model_id)
                {
                    return Err(advanced_invalid(
                        "this consented voice enrollment is already registered",
                    ));
                }
                let provider_voice_id = self.voice_provider.enroll(
                    &VoiceEnrollmentRequest {
                        reference_path,
                        voice_name: voice_name.to_string(),
                    },
                    cancel,
                )?;
                if cancel.checkpoint() {
                    let _ = self
                        .voice_provider
                        .revoke(&provider_voice_id, &MediaCancelToken::new());
                    return Err(cancelled_workflow("voice enrollment cancelled"));
                }
                let record = VoiceModelRecord {
                    id: voice_model_id.clone(),
                    provider: PROVIDER.into(),
                    provider_voice_id: provider_voice_id.clone(),
                    model: MODEL.into(),
                    consent_id: args.consent_id.clone(),
                    source_audio_asset_id: source_id.to_string(),
                    source_audio_sha256: source_sha256,
                    request_hash: request_hash.clone(),
                    voice_name: voice_name.to_string(),
                    revoked: false,
                };
                let edit = self
                    .core
                    .apply_at_revision_persisted(
                        ProjectRevision {
                            project_epoch: snapshot.project_epoch,
                            version: snapshot.version,
                        },
                        EditCommand::SaveVoiceModel {
                            record: record.clone(),
                        },
                    )
                    .map_err(|error| {
                        let _ = self
                            .voice_provider
                            .revoke(&provider_voice_id, &MediaCancelToken::new());
                        advanced_execution(error.to_string())
                    })?;
                Ok(AdvancedWorkflowCommit {
                    result: json!({
                        "action": "enroll",
                        "voiceId": voice_model_id,
                        "voiceName": record.voice_name,
                        "provider": PROVIDER,
                        "model": MODEL,
                        "consentId": record.consent_id,
                        "sourceAudioMediaRef": record.source_audio_asset_id,
                        "sourceAudioSha256": record.source_audio_sha256,
                        "requestHash": request_hash,
                        "revoked": false
                    }),
                    action_name: edit.changed.then_some(edit.action_name),
                })
            }
            "generate" => {
                if args.cost_authorized != Some(true) {
                    return Err(AdvancedWorkflowError::new(
                        AdvancedWorkflowErrorKind::CostAuthorizationRequired,
                        "cloned-voice generation uses a paid provider; set costAuthorized=true after review",
                    ));
                }
                let voice_id = args
                    .voice_id
                    .as_deref()
                    .ok_or_else(|| advanced_invalid("voiceId is required for generation"))?;
                let record = snapshot
                    .timeline
                    .voice_models
                    .iter()
                    .find(|record| record.id == voice_id)
                    .cloned()
                    .ok_or_else(|| advanced_resource("voice model does not exist"))?;
                if record.provider != PROVIDER || record.model != MODEL {
                    return Err(AdvancedWorkflowError::new(
                        AdvancedWorkflowErrorKind::CapabilityUnavailable,
                        "voice model provider metadata is not supported by this build",
                    ));
                }
                if record.revoked {
                    return Err(AdvancedWorkflowError::new(
                        AdvancedWorkflowErrorKind::ConsentRequired,
                        "revoked voice models cannot generate audio",
                    ));
                }
                if record.consent_id != args.consent_id {
                    return Err(AdvancedWorkflowError::new(
                        AdvancedWorkflowErrorKind::ConsentRequired,
                        "consentId does not match the enrolled voice record",
                    ));
                }
                let prompt = args
                    .prompt
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty() && value.len() <= 20_000)
                    .ok_or_else(|| advanced_invalid("prompt is required and too long"))?;
                let request_hash = canonical_request_hash(&json!({
                    "provider": PROVIDER,
                    "model": MODEL,
                    "consentId": record.consent_id,
                    "voiceId": record.id,
                    "voiceRequestHash": record.request_hash,
                    "prompt": prompt,
                }))?;
                let media_dir = project_dir.join(opentake_project::layout::MEDIA_DIR);
                std::fs::create_dir_all(&media_dir)
                    .map_err(|_| advanced_execution("project media directory is unavailable"))?;
                let destination =
                    media_dir.join(format!("cloned-voice-{}.mp3", uuid::Uuid::new_v4()));
                let output = self.voice_provider.generate(
                    &VoiceGenerationRequest {
                        provider_voice_id: record.provider_voice_id.clone(),
                        model: MODEL.into(),
                        prompt: prompt.to_string(),
                        destination: destination.clone(),
                    },
                    cancel,
                )?;
                if cancel.checkpoint() {
                    let _ = std::fs::remove_file(&destination);
                    return Err(cancelled_workflow("voice generation cancelled"));
                }
                let output_probe = probe(&destination).map_err(|error| {
                    let _ = std::fs::remove_file(&destination);
                    media_workflow_error(error)
                })?;
                if output_probe.has_video
                    || !output_probe.has_audio
                    || output_probe.duration_secs <= 0.0
                {
                    let _ = std::fs::remove_file(&destination);
                    return Err(advanced_execution(
                        "voice provider output must be non-empty audio",
                    ));
                }
                let fps = snapshot.timeline.fps.max(1) as f64;
                let duration_frames = (output_probe.duration_secs * fps).round().max(1.0) as i32;
                let provenance = GenerationInput {
                    prompt: prompt.to_string(),
                    model: MODEL.into(),
                    duration: output_probe.duration_secs.round() as i32,
                    aspect_ratio: "audio".into(),
                    voice: Some(record.id.clone()),
                    reference_audio_urls: Some(vec![format!(
                        "sha256:{}",
                        record.source_audio_sha256
                    )]),
                    reference_audio_asset_ids: Some(vec![record.source_audio_asset_id.clone()]),
                    source_asset_id: Some(record.source_audio_asset_id.clone()),
                    provider: Some(PROVIDER.into()),
                    provider_job_id: Some(output.request_id.clone()),
                    status: Some(GenerationJobStatus::Ready),
                    progress: Some(1.0),
                    consent_id: Some(record.consent_id.clone()),
                    request_hash: Some(request_hash.clone()),
                    ..GenerationInput::default()
                };
                let probed = ProbedMedia {
                    duration_secs: output_probe.duration_secs,
                    width: None,
                    height: None,
                    fps: None,
                    has_audio: true,
                    color: None,
                };
                let committed = self
                    .core
                    .commit_generated_media_for_project(
                        snapshot.project_epoch,
                        snapshot.version,
                        &project_dir,
                        &destination,
                        format!("{} voice", record.voice_name),
                        opentake_domain::ClipType::Audio,
                        &probed,
                        provenance,
                        MotionPlacement::Add {
                            start_frame: snapshot.timeline.total_frames(),
                            duration_frames,
                            track_index: None,
                        },
                        "Generate Cloned Voice",
                    )
                    .map_err(|error| {
                        let _ = std::fs::remove_file(&destination);
                        advanced_execution(error.to_string())
                    })?;
                Ok(AdvancedWorkflowCommit {
                    result: json!({
                        "action": "generate",
                        "voiceId": record.id,
                        "assetId": committed.media.id,
                        "clipIds": committed.edit.affected_clip_ids,
                        "previewPath": destination,
                        "provider": PROVIDER,
                        "model": MODEL,
                        "providerRequestId": output.request_id,
                        "requestHash": request_hash,
                        "consentId": record.consent_id,
                        "durationFrames": duration_frames,
                        "mediaType": output.media_type,
                        "imported": true
                    }),
                    action_name: Some(committed.edit.action_name),
                })
            }
            "revoke" => {
                let voice_id = args
                    .voice_id
                    .as_deref()
                    .ok_or_else(|| advanced_invalid("voiceId is required for revocation"))?;
                let record = snapshot
                    .timeline
                    .voice_models
                    .iter()
                    .find(|record| record.id == voice_id)
                    .cloned()
                    .ok_or_else(|| advanced_resource("voice model does not exist"))?;
                if record.provider != PROVIDER || record.model != MODEL {
                    return Err(AdvancedWorkflowError::new(
                        AdvancedWorkflowErrorKind::CapabilityUnavailable,
                        "voice model provider metadata is not supported by this build",
                    ));
                }
                if record.consent_id != args.consent_id {
                    return Err(AdvancedWorkflowError::new(
                        AdvancedWorkflowErrorKind::ConsentRequired,
                        "consentId does not match the enrolled voice record",
                    ));
                }
                if !record.revoked {
                    self.voice_provider
                        .revoke(&record.provider_voice_id, cancel)?;
                    let current = self.core.runtime_snapshot();
                    if current.project_epoch != snapshot.project_epoch {
                        return Err(advanced_execution(
                            "project changed while revoking the provider voice",
                        ));
                    }
                    let edit = self
                        .core
                        .apply_at_revision_persisted(
                            ProjectRevision {
                                project_epoch: current.project_epoch,
                                version: current.version,
                            },
                            EditCommand::RevokeVoiceModel {
                                voice_model_id: record.id.clone(),
                            },
                        )
                        .map_err(|error| advanced_execution(error.to_string()))?;
                    return Ok(AdvancedWorkflowCommit {
                        result: json!({
                            "action": "revoke",
                            "voiceId": record.id,
                            "provider": PROVIDER,
                            "consentId": record.consent_id,
                            "revoked": true
                        }),
                        action_name: edit.changed.then_some(edit.action_name),
                    });
                }
                Ok(AdvancedWorkflowCommit {
                    result: json!({
                        "action": "revoke",
                        "voiceId": record.id,
                        "provider": PROVIDER,
                        "consentId": record.consent_id,
                        "revoked": true
                    }),
                    action_name: None,
                })
            }
            _ => Err(advanced_invalid(
                "voice action must be enroll, generate, or revoke",
            )),
        }
    }
}

impl AdvancedWorkflowBridge for TauriAdvancedWorkflowBridge {
    fn supported_tools(&self) -> Vec<ToolName> {
        let mut tools = vec![
            ToolName::TrackMotion,
            ToolName::RemoveObject,
            ToolName::MatchColor,
            ToolName::SeparateStems,
            ToolName::TranslateCaptions,
            ToolName::ScriptToVideo,
            ToolName::GenerateAvatar,
            ToolName::CloneVoice,
        ];
        if verify_rvm_model(&self.models_dir).is_ok() {
            tools.push(ToolName::GenerateMatte);
        }
        tools
    }

    fn execute(
        &self,
        request: AdvancedWorkflowRequest,
        cancel: &MediaCancelToken,
    ) -> Result<AdvancedWorkflowCommit, AdvancedWorkflowError> {
        match request {
            AdvancedWorkflowRequest::TrackMotion(args) => self.track_motion(args, cancel),
            AdvancedWorkflowRequest::GenerateMatte(args) => self.generate_matte(args, cancel),
            AdvancedWorkflowRequest::RemoveObject(args) => self.remove_object(args, cancel),
            AdvancedWorkflowRequest::MatchColor(args) => self.match_color(args, cancel),
            AdvancedWorkflowRequest::SeparateStems(args) => self.separate_stems(args, cancel),
            AdvancedWorkflowRequest::TranslateCaptions(args) => {
                self.translate_captions(args, cancel)
            }
            AdvancedWorkflowRequest::ScriptToVideo(args) => self.script_to_video(args, cancel),
            AdvancedWorkflowRequest::GenerateAvatar(args) => self.generate_avatar(args, cancel),
            AdvancedWorkflowRequest::CloneVoice(args) => self.clone_voice(args, cancel),
        }
    }
}

fn position_keyframes(
    clip: &opentake_domain::Clip,
    tracked: &RegionMotionTrack,
) -> Vec<Keyframe<AnimPair>> {
    tracked
        .samples
        .iter()
        .map(|sample| {
            let absolute_frame = clip.start_frame + sample.frame;
            let base = clip.transform_at(absolute_frame);
            Keyframe::with_interpolation(
                sample.frame,
                AnimPair::new(
                    base.center_x + sample.translation_x,
                    base.center_y + sample.translation_y,
                ),
                Interpolation::Linear,
            )
        })
        .collect()
}

fn advanced_invalid(message: impl Into<String>) -> AdvancedWorkflowError {
    AdvancedWorkflowError::new(AdvancedWorkflowErrorKind::InvalidArguments, message)
}

fn validate_consent_id(value: &str) -> Result<(), AdvancedWorkflowError> {
    if value.len() < 8
        || value.len() > 256
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
    {
        return Err(AdvancedWorkflowError::new(
            AdvancedWorkflowErrorKind::ConsentRequired,
            "record explicit consent and provide its 8-256 character consentId",
        ));
    }
    Ok(())
}

fn canonical_request_hash(value: &serde_json::Value) -> Result<String, AdvancedWorkflowError> {
    let bytes = serde_json::to_vec(value).map_err(|error| advanced_execution(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn advanced_resource(message: impl Into<String>) -> AdvancedWorkflowError {
    AdvancedWorkflowError::new(AdvancedWorkflowErrorKind::ResourceNotFound, message)
}

fn advanced_execution(message: impl Into<String>) -> AdvancedWorkflowError {
    AdvancedWorkflowError::new(AdvancedWorkflowErrorKind::ExecutionFailed, message)
}

fn cancelled_workflow(message: impl Into<String>) -> AdvancedWorkflowError {
    AdvancedWorkflowError::new(AdvancedWorkflowErrorKind::Cancelled, message)
}

fn media_workflow_error(error: MediaError) -> AdvancedWorkflowError {
    let kind = if matches!(error, MediaError::Cancelled) {
        AdvancedWorkflowErrorKind::Cancelled
    } else {
        AdvancedWorkflowErrorKind::ExecutionFailed
    };
    AdvancedWorkflowError::new(kind, error.to_string())
}

fn parse_mask_index(mask_id: &str) -> Result<usize, AdvancedWorkflowError> {
    if mask_id == "primary" {
        return Ok(0);
    }
    let numeric = mask_id.strip_prefix("mask-").unwrap_or(mask_id);
    numeric
        .parse::<usize>()
        .map_err(|_| advanced_invalid("maskId must be primary, a zero-based index, or mask-N"))
}

fn decode_color_sample(
    path: &Path,
    time: f64,
    is_image: bool,
    request: &FrameRequest,
    cancel: &MediaCancelToken,
) -> Result<RgbaFrame, AdvancedWorkflowError> {
    if cancel.checkpoint() {
        return Err(cancelled_workflow("color match cancelled"));
    }
    if is_image {
        return opentake_media::thumbnail::image_thumbnail(path, 640).map_err(media_workflow_error);
    }
    let decoded = decode_frames_at_cancellable(path, &[time], request, cancel)
        .into_iter()
        .next()
        .ok_or_else(|| advanced_execution("color sample decoder returned no frame"))?
        .map_err(media_workflow_error)?;
    Ok(decoded.1)
}

fn mean_linear_rgb(frame: &RgbaFrame) -> Result<Rgb, AdvancedWorkflowError> {
    let mut sum = [0.0_f64; 3];
    let mut weight = 0.0_f64;
    for pixel in frame.rgba.as_chunks::<4>().0.iter() {
        let alpha = f64::from(pixel[3]) / 255.0;
        if alpha <= 0.01 {
            continue;
        }
        sum[0] += bt709_to_linear(f64::from(pixel[0]) / 255.0) * alpha;
        sum[1] += bt709_to_linear(f64::from(pixel[1]) / 255.0) * alpha;
        sum[2] += bt709_to_linear(f64::from(pixel[2]) / 255.0) * alpha;
        weight += alpha;
    }
    if weight <= f64::EPSILON {
        return Err(AdvancedWorkflowError::new(
            AdvancedWorkflowErrorKind::AnalysisLowConfidence,
            "color sample contains no visible pixels",
        ));
    }
    Ok(Rgb::new(sum[0] / weight, sum[1] / weight, sum[2] / weight))
}

fn bt709_to_linear(value: f64) -> f64 {
    if value < 0.081 {
        value / 4.5
    } else {
        ((value + 0.099) / 1.099).powf(1.0 / 0.45)
    }
}

fn delta_e76(left: Rgb, right: Rgb) -> f64 {
    let left = linear_rgb_to_lab(left);
    let right = linear_rgb_to_lab(right);
    ((left[0] - right[0]).powi(2) + (left[1] - right[1]).powi(2) + (left[2] - right[2]).powi(2))
        .sqrt()
}

fn linear_rgb_to_lab(rgb: Rgb) -> [f64; 3] {
    let x = (0.412_456_4 * rgb.r + 0.357_576_1 * rgb.g + 0.180_437_5 * rgb.b) / 0.950_47;
    let y = 0.212_672_9 * rgb.r + 0.715_152_2 * rgb.g + 0.072_175 * rgb.b;
    let z = (0.019_333_9 * rgb.r + 0.119_192 * rgb.g + 0.950_304_1 * rgb.b) / 1.088_83;
    fn pivot(value: f64) -> f64 {
        const EPSILON: f64 = 216.0 / 24_389.0;
        const KAPPA: f64 = 24_389.0 / 27.0;
        if value > EPSILON {
            value.cbrt()
        } else {
            (KAPPA * value + 16.0) / 116.0
        }
    }
    let fx = pivot(x);
    let fy = pivot(y);
    let fz = pivot(z);
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

#[allow(clippy::too_many_arguments)]
fn materialize_object_removal_video(
    source: &Path,
    cache_path: &Path,
    clip: &opentake_domain::Clip,
    timeline_fps: i32,
    removal_start: i32,
    removal_end: i32,
    mask: &Mask,
    cancel: &MediaCancelToken,
) -> Result<(), AdvancedWorkflowError> {
    if cancel.checkpoint() {
        return Err(cancelled_workflow("object removal cancelled"));
    }
    let cache_dir = cache_path
        .parent()
        .ok_or_else(|| advanced_execution("object-removal cache path has no parent"))?;
    let metadata = std::fs::symlink_metadata(cache_dir)
        .map_err(|error| advanced_execution(error.to_string()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(advanced_execution(
            "object-removal cache directory must be a regular directory",
        ));
    }
    let source_probe = probe(source).map_err(media_workflow_error)?;
    let source_start = i64::from(clip.trim_start_frame);
    let source_end = source_start + i64::from(clip.duration_frames);
    let stream = spawn_video_stream(VideoStreamRequest {
        path: source.to_path_buf(),
        start_frame: source_start,
        end_frame: Some(source_end),
        timeline_fps,
        max_size: (0, 0),
        queue_capacity: 8,
        apply_rotation: true,
    })
    .map_err(media_workflow_error)?;
    let first = match receive_video_frame(&stream, cancel) {
        Ok(frame) => frame,
        Err(error) => {
            let _ = stream.join();
            return Err(error);
        }
    };
    let width = first.frame.width;
    let height = first.frame.height;
    validate_removal_mask(mask, width, height)?;
    let partial_path = cache_dir.join(format!(
        ".object-removal-{}.partial.mov",
        uuid::Uuid::new_v4()
    ));
    let preset = ExportPreset::new(VideoCodec::ProRes422, ExportResolution::P1080);
    let mut encoder = match VideoEncoder::new(&partial_path, width, height, timeline_fps, &preset) {
        Ok(encoder) => encoder,
        Err(error) => {
            let _ = stream.join();
            let _ = std::fs::remove_file(&partial_path);
            return Err(media_workflow_error(error));
        }
    };
    let mut encoded_frames = 0_i32;
    let encode_result = (|| {
        encode_object_removal_frame(
            first.frame,
            encoded_frames,
            clip,
            removal_start,
            removal_end,
            mask,
            &mut encoder,
            cancel,
        )?;
        encoded_frames += 1;
        while encoded_frames < clip.duration_frames {
            if cancel.checkpoint() {
                return Err(cancelled_workflow("object removal cancelled"));
            }
            let next = receive_video_frame(&stream, cancel)?;
            if next.frame.width != width || next.frame.height != height {
                return Err(advanced_execution(
                    "object-removal source dimensions changed during decode",
                ));
            }
            encode_object_removal_frame(
                next.frame,
                encoded_frames,
                clip,
                removal_start,
                removal_end,
                mask,
                &mut encoder,
                cancel,
            )?;
            encoded_frames += 1;
        }
        if source_probe.has_audio {
            let fps = timeline_fps.max(1) as f64;
            let audio = extract_pcm_cancellable(
                source,
                &PcmSpec {
                    sample_rate: 48_000,
                    channels: 1,
                    format: PcmFormat::F32,
                },
                Some((source_start as f64 / fps, source_end as f64 / fps)),
                cancel,
            )
            .map_err(media_workflow_error)?;
            encoder.push_audio(audio).map_err(media_workflow_error)?;
        }
        Ok(())
    })();
    stream.request_stop();
    let _ = stream.join();
    if let Err(error) = encode_result {
        encoder.abort();
        let _ = std::fs::remove_file(&partial_path);
        return Err(error);
    }
    if let Err(error) = encoder.finish_cancellable(cancel, None) {
        let _ = std::fs::remove_file(&partial_path);
        return Err(media_workflow_error(error));
    }
    match std::fs::hard_link(&partial_path, cache_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            let _ = std::fs::remove_file(&partial_path);
            return Err(advanced_execution(error.to_string()));
        }
    }
    let _ = std::fs::remove_file(&partial_path);
    let published = std::fs::symlink_metadata(cache_path)
        .map_err(|error| advanced_execution(error.to_string()))?;
    if !published.is_file() || published.file_type().is_symlink() {
        return Err(advanced_execution(
            "object-removal cache output must be a regular file",
        ));
    }
    Ok(())
}

fn validate_removal_mask(
    mask: &Mask,
    width: u32,
    height: u32,
) -> Result<(), AdvancedWorkflowError> {
    let mut selected = 0_usize;
    for y in 0..height {
        for x in 0..width {
            let coverage = mask.coverage(
                (f64::from(x) + 0.5) / f64::from(width),
                (f64::from(y) + 0.5) / f64::from(height),
            );
            selected += usize::from(coverage > 0.001);
        }
    }
    let pixels = width as usize * height as usize;
    if selected == 0 {
        return Err(advanced_invalid(
            "the selected mask does not cover any source pixels",
        ));
    }
    if selected == pixels {
        return Err(advanced_invalid(
            "the selected mask covers the entire frame; object removal needs surrounding pixels",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn encode_object_removal_frame(
    mut frame: RgbaFrame,
    output_index: i32,
    clip: &opentake_domain::Clip,
    removal_start: i32,
    removal_end: i32,
    mask: &Mask,
    encoder: &mut VideoEncoder,
    cancel: &MediaCancelToken,
) -> Result<(), AdvancedWorkflowError> {
    let project_frame = clip.start_frame + output_index;
    if (removal_start..removal_end).contains(&project_frame) {
        inpaint_masked_region(&mut frame, mask, cancel)?;
    }
    encoder.push_frame(&frame).map_err(media_workflow_error)
}

fn inpaint_masked_region(
    frame: &mut RgbaFrame,
    mask: &Mask,
    cancel: &MediaCancelToken,
) -> Result<(), AdvancedWorkflowError> {
    validate_removal_mask(mask, frame.width, frame.height)?;
    let width = frame.width as usize;
    let height = frame.height as usize;
    let pixels = width * height;
    let mut coverage = Vec::with_capacity(pixels);
    let mut known = Vec::with_capacity(pixels);
    for y in 0..height {
        if y % 32 == 0 && cancel.checkpoint() {
            return Err(cancelled_workflow("object removal cancelled"));
        }
        for x in 0..width {
            let value = mask.coverage(
                (x as f64 + 0.5) / width as f64,
                (y as f64 + 0.5) / height as f64,
            );
            coverage.push(value);
            known.push(value <= 0.001);
        }
    }
    let original = frame.rgba.clone();
    let mut filled = frame.rgba.clone();
    let mut queued = vec![false; pixels];
    let mut queue = VecDeque::new();
    for index in 0..pixels {
        if !known[index] && has_known_neighbour(index, width, height, &known) {
            queue.push_back(index);
            queued[index] = true;
        }
    }
    let mut processed = 0_usize;
    while let Some(index) = queue.pop_front() {
        if processed.is_multiple_of(4096) && cancel.checkpoint() {
            return Err(cancelled_workflow("object removal cancelled"));
        }
        let mut sums = [0_u32; 3];
        let mut count = 0_u32;
        for neighbour in neighbours(index, width, height).into_iter().flatten() {
            if known[neighbour] {
                let offset = neighbour * 4;
                sums[0] += u32::from(filled[offset]);
                sums[1] += u32::from(filled[offset + 1]);
                sums[2] += u32::from(filled[offset + 2]);
                count += 1;
            }
        }
        if count == 0 {
            continue;
        }
        let offset = index * 4;
        for channel in 0..3 {
            filled[offset + channel] = (sums[channel] / count) as u8;
        }
        known[index] = true;
        processed += 1;
        for neighbour in neighbours(index, width, height).into_iter().flatten() {
            if !known[neighbour] && !queued[neighbour] {
                queue.push_back(neighbour);
                queued[neighbour] = true;
            }
        }
    }
    if known.iter().any(|value| !value) {
        return Err(advanced_execution(
            "object-removal mask could not be filled from its boundary",
        ));
    }
    for (index, alpha) in coverage.into_iter().enumerate() {
        if alpha <= 0.0 {
            continue;
        }
        let offset = index * 4;
        for channel in 0..3 {
            frame.rgba[offset + channel] = (f64::from(original[offset + channel]) * (1.0 - alpha)
                + f64::from(filled[offset + channel]) * alpha)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
    }
    Ok(())
}

fn has_known_neighbour(index: usize, width: usize, height: usize, known: &[bool]) -> bool {
    neighbours(index, width, height)
        .into_iter()
        .flatten()
        .any(|neighbour| known[neighbour])
}

fn neighbours(index: usize, width: usize, height: usize) -> [Option<usize>; 8] {
    let x = index % width;
    let y = index / width;
    let mut result = [None; 8];
    let mut cursor = 0;
    for dy in -1_i32..=1 {
        for dx in -1_i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx >= 0 && ny >= 0 && nx < width as i32 && ny < height as i32 {
                result[cursor] = Some(ny as usize * width + nx as usize);
                cursor += 1;
            }
        }
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn materialize_matte_video(
    source: &Path,
    cache_path: &Path,
    clip: &opentake_domain::Clip,
    timeline_fps: i32,
    matte_start: i32,
    matte_end: i32,
    models_dir: &Path,
    cancel: &MediaCancelToken,
) -> Result<(), AdvancedWorkflowError> {
    if cancel.checkpoint() {
        return Err(AdvancedWorkflowError::new(
            AdvancedWorkflowErrorKind::Cancelled,
            "matting cancelled",
        ));
    }
    let cache_dir = cache_path
        .parent()
        .ok_or_else(|| advanced_execution("matting cache path has no parent"))?;
    let metadata = std::fs::symlink_metadata(cache_dir)
        .map_err(|error| advanced_execution(error.to_string()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(advanced_execution(
            "matting cache directory must be a regular directory",
        ));
    }
    let source_probe = probe(source).map_err(media_workflow_error)?;
    let source_start = i64::from(clip.trim_start_frame);
    let source_end = source_start + i64::from(clip.duration_frames);
    let stream = spawn_video_stream(VideoStreamRequest {
        path: source.to_path_buf(),
        start_frame: source_start,
        end_frame: Some(source_end),
        timeline_fps,
        max_size: (0, 0),
        queue_capacity: 8,
        apply_rotation: true,
    })
    .map_err(media_workflow_error)?;
    let first = match receive_video_frame(&stream, cancel) {
        Ok(frame) => frame,
        Err(error) => {
            let _ = stream.join();
            return Err(error);
        }
    };
    let width = first.frame.width;
    let height = first.frame.height;
    let partial_path = cache_dir.join(format!(".matte-{}.partial.mov", uuid::Uuid::new_v4()));
    let preset = ExportPreset::new(VideoCodec::ProRes4444, ExportResolution::P1080);
    let mut encoder = match VideoEncoder::new(&partial_path, width, height, timeline_fps, &preset) {
        Ok(encoder) => encoder,
        Err(error) => {
            let _ = stream.join();
            let _ = std::fs::remove_file(&partial_path);
            return Err(media_workflow_error(error));
        }
    };
    let mut session = match RvmMattingSession::load(models_dir) {
        Ok(session) => session,
        Err(error) => {
            encoder.abort();
            let _ = stream.join();
            let _ = std::fs::remove_file(&partial_path);
            return Err(media_workflow_error(error));
        }
    };
    let mut encoded_frames = 0_i32;
    let encode_result = (|| {
        encode_matte_frame(
            first.frame,
            encoded_frames,
            clip,
            matte_start,
            matte_end,
            &mut session,
            &mut encoder,
            cancel,
        )?;
        encoded_frames += 1;
        while encoded_frames < clip.duration_frames {
            if cancel.checkpoint() {
                return Err(AdvancedWorkflowError::new(
                    AdvancedWorkflowErrorKind::Cancelled,
                    "matting cancelled",
                ));
            }
            let next = receive_video_frame(&stream, cancel)?;
            if next.frame.width != width || next.frame.height != height {
                return Err(advanced_execution(
                    "matting source dimensions changed during decode",
                ));
            }
            encode_matte_frame(
                next.frame,
                encoded_frames,
                clip,
                matte_start,
                matte_end,
                &mut session,
                &mut encoder,
                cancel,
            )?;
            encoded_frames += 1;
        }
        if source_probe.has_audio {
            let fps = timeline_fps.max(1) as f64;
            let audio = extract_pcm_cancellable(
                source,
                &PcmSpec {
                    sample_rate: 48_000,
                    channels: 1,
                    format: PcmFormat::F32,
                },
                Some((source_start as f64 / fps, source_end as f64 / fps)),
                cancel,
            )
            .map_err(media_workflow_error)?;
            encoder.push_audio(audio).map_err(media_workflow_error)?;
        }
        Ok(())
    })();
    stream.request_stop();
    let _ = stream.join();
    if let Err(error) = encode_result {
        encoder.abort();
        let _ = std::fs::remove_file(&partial_path);
        return Err(error);
    }
    if let Err(error) = encoder.finish_cancellable(cancel, None) {
        let _ = std::fs::remove_file(&partial_path);
        return Err(media_workflow_error(error));
    }
    match std::fs::hard_link(&partial_path, cache_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            let _ = std::fs::remove_file(&partial_path);
            return Err(advanced_execution(error.to_string()));
        }
    }
    let _ = std::fs::remove_file(&partial_path);
    let published = std::fs::symlink_metadata(cache_path)
        .map_err(|error| advanced_execution(error.to_string()))?;
    if !published.is_file() || published.file_type().is_symlink() {
        return Err(advanced_execution(
            "matting cache output must be a regular file",
        ));
    }
    Ok(())
}

fn receive_video_frame(
    stream: &VideoStream,
    cancel: &MediaCancelToken,
) -> Result<StreamVideoFrame, AdvancedWorkflowError> {
    loop {
        if cancel.checkpoint() {
            return Err(cancelled_workflow("advanced video workflow cancelled"));
        }
        match stream
            .receiver()
            .recv_timeout(std::time::Duration::from_millis(50))
        {
            Ok(frame) => return frame.map_err(media_workflow_error),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(advanced_execution(
                    "advanced video source ended before the clip",
                ));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_matte_frame(
    mut frame: RgbaFrame,
    output_index: i32,
    clip: &opentake_domain::Clip,
    matte_start: i32,
    matte_end: i32,
    session: &mut RvmMattingSession,
    encoder: &mut VideoEncoder,
    cancel: &MediaCancelToken,
) -> Result<(), AdvancedWorkflowError> {
    let project_frame = clip.start_frame + output_index;
    if (matte_start..matte_end).contains(&project_frame) {
        let matte = session
            .infer(&frame, cancel)
            .map_err(media_workflow_error)?;
        let mut rgba = Vec::with_capacity(matte.alpha.len() * 4);
        for (rgb, alpha) in matte
            .foreground_rgb
            .as_chunks::<3>()
            .0
            .iter()
            .zip(matte.alpha)
        {
            rgba.extend_from_slice(rgb);
            rgba.push(alpha);
        }
        frame = RgbaFrame::new(frame.width, frame.height, rgba);
    } else {
        for pixel in frame.rgba.as_chunks_mut::<4>().0.iter_mut() {
            pixel[3] = 255;
        }
    }
    encoder.push_frame(&frame).map_err(media_workflow_error)
}

fn open_verified_regular_file(path: &Path) -> Result<std::fs::File, AdvancedWorkflowError> {
    let before =
        std::fs::symlink_metadata(path).map_err(|error| advanced_execution(error.to_string()))?;
    if !before.is_file() || before.file_type().is_symlink() {
        return Err(advanced_execution(
            "advanced workflow cache must be a regular file",
        ));
    }
    let file = std::fs::File::open(path).map_err(|error| advanced_execution(error.to_string()))?;
    let after =
        std::fs::symlink_metadata(path).map_err(|error| advanced_execution(error.to_string()))?;
    if !after.is_file() || after.file_type().is_symlink() {
        return Err(advanced_execution(
            "advanced workflow cache identity changed",
        ));
    }
    let opened = Handle::from_file(
        file.try_clone()
            .map_err(|error| advanced_execution(error.to_string()))?,
    )
    .map_err(|error| advanced_execution(error.to_string()))?;
    let current = Handle::from_path(path).map_err(|error| advanced_execution(error.to_string()))?;
    if opened != current {
        return Err(advanced_execution(
            "advanced workflow cache identity changed",
        ));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_core::{PreparedMediaImportOp, ProbedMedia};
    use opentake_domain::{Clip, ClipType, MaskShape, Point2};
    use opentake_media::analysis::StabilizationMotionSample;
    use opentake_ops::ClipEntry;
    use std::collections::HashSet;
    use std::process::Command;

    #[test]
    fn advanced_work_cannot_begin_after_update_install_claims_admission() {
        let temp = tempfile::tempdir().unwrap();
        let admission = crate::updater::InstallAdmissionGate::default();
        let bridge = Arc::new(TauriAdvancedWorkflowBridge::new(
            AppCore::new(),
            temp.path().join("cache"),
            temp.path().join("models"),
        ));
        let state = AdvancedWorkflowCommandState::new(bridge, admission.clone());
        let matting = MattingModelInstallState::new(admission.clone());
        let _install = admission.begin_install().unwrap();

        assert_eq!(
            state.begin().err().unwrap(),
            "app update installation is in progress"
        );
        assert_eq!(
            matting.begin().err().unwrap(),
            "app update installation is in progress"
        );
    }

    #[test]
    fn provider_resource_ids_are_safe_path_segments() {
        assert!(valid_provider_resource_id("abc-123_DEF"));
        assert!(!valid_provider_resource_id(""));
        assert!(!valid_provider_resource_id("../voices"));
        assert!(!valid_provider_resource_id("voice/id"));
        assert!(!valid_provider_resource_id(&"a".repeat(257)));
    }

    struct FixtureAvatarProvider {
        fixture: PathBuf,
        fail: bool,
        calls: Arc<Mutex<usize>>,
    }

    impl AvatarProvider for FixtureAvatarProvider {
        fn generate(
            &self,
            request: &AvatarProviderRequest,
            cancel: &MediaCancelToken,
        ) -> Result<AvatarProviderOutput, AdvancedWorkflowError> {
            *self.calls.lock().unwrap() += 1;
            if cancel.checkpoint() {
                return Err(cancelled_workflow("fixture avatar cancelled"));
            }
            if self.fail {
                return Err(advanced_execution("fixture avatar provider failed"));
            }
            std::fs::copy(&self.fixture, &request.destination)
                .map_err(|error| advanced_execution(error.to_string()))?;
            Ok(AvatarProviderOutput {
                request_id: "avatar-request-fixture".into(),
                media_type: "video/mp4".into(),
            })
        }
    }

    struct FixtureVoiceProvider {
        fixture: PathBuf,
        fail_generation: bool,
        revoked: Arc<Mutex<HashSet<String>>>,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl VoiceCloneProvider for FixtureVoiceProvider {
        fn enroll(
            &self,
            _request: &VoiceEnrollmentRequest,
            cancel: &MediaCancelToken,
        ) -> Result<String, AdvancedWorkflowError> {
            self.calls.lock().unwrap().push("enroll".into());
            if cancel.checkpoint() {
                return Err(cancelled_workflow("fixture voice enrollment cancelled"));
            }
            Ok("provider-voice-fixture".into())
        }

        fn generate(
            &self,
            request: &VoiceGenerationRequest,
            cancel: &MediaCancelToken,
        ) -> Result<VoiceProviderOutput, AdvancedWorkflowError> {
            self.calls.lock().unwrap().push("generate".into());
            if cancel.checkpoint() {
                return Err(cancelled_workflow("fixture voice generation cancelled"));
            }
            if self.fail_generation {
                return Err(advanced_execution("fixture voice provider failed"));
            }
            if self
                .revoked
                .lock()
                .unwrap()
                .contains(&request.provider_voice_id)
            {
                return Err(advanced_execution("fixture voice was revoked"));
            }
            std::fs::copy(&self.fixture, &request.destination)
                .map_err(|error| advanced_execution(error.to_string()))?;
            Ok(VoiceProviderOutput {
                request_id: "voice-request-fixture".into(),
                media_type: "audio/mpeg".into(),
            })
        }

        fn revoke(
            &self,
            provider_voice_id: &str,
            cancel: &MediaCancelToken,
        ) -> Result<(), AdvancedWorkflowError> {
            self.calls.lock().unwrap().push("revoke".into());
            if cancel.checkpoint() {
                return Err(cancelled_workflow("fixture voice revocation cancelled"));
            }
            self.revoked
                .lock()
                .unwrap()
                .insert(provider_voice_id.to_string());
            Ok(())
        }
    }

    #[test]
    fn tracked_motion_becomes_editable_linear_position_keyframes() {
        let mut clip = Clip::new("clip", "asset", 100, 30);
        clip.transform.center_x = 0.4;
        clip.transform.center_y = 0.6;
        let keyframes = position_keyframes(
            &clip,
            &RegionMotionTrack {
                samples: vec![
                    StabilizationMotionSample {
                        frame: 0,
                        translation_x: 0.0,
                        translation_y: 0.0,
                        rotation_degrees: 0.0,
                    },
                    StabilizationMotionSample {
                        frame: 10,
                        translation_x: 0.1,
                        translation_y: -0.05,
                        rotation_degrees: 0.0,
                    },
                ],
                minimum_confidence: 0.9,
            },
        );
        assert_eq!(keyframes.len(), 2);
        assert_eq!(keyframes[1].frame, 10);
        assert!((keyframes[1].value.a - 0.5).abs() < 1e-9);
        assert!((keyframes[1].value.b - 0.55).abs() < 1e-9);
        assert_eq!(keyframes[1].interpolation_out, Interpolation::Linear);
    }

    #[test]
    fn boundary_fill_removes_masked_pixels_and_preserves_unmasked_pixels() {
        let mut rgba = Vec::new();
        for y in 0..12_u8 {
            for x in 0..16_u8 {
                if (5..11).contains(&x) && (4..8).contains(&y) {
                    rgba.extend_from_slice(&[240, 10, 10, 255]);
                } else {
                    rgba.extend_from_slice(&[30, 90, 140, 255]);
                }
            }
        }
        let mut frame = RgbaFrame::new(16, 12, rgba);
        let before = frame.rgba.clone();
        let mask = Mask {
            shape: MaskShape::Circle {
                center: Point2::new(0.5, 0.5),
                radius: Point2::new(0.22, 0.22),
            },
            ..Mask::default()
        };

        inpaint_masked_region(&mut frame, &mask, &MediaCancelToken::new()).unwrap();

        let center = (6 * 16 + 8) * 4;
        assert!(frame.rgba[center] < 80);
        assert!(frame.rgba[center + 1] > 70);
        assert!(frame.rgba[center + 2] > 110);
        assert_eq!(&frame.rgba[0..4], &before[0..4]);
        assert_eq!(frame.rgba[center + 3], 255);
    }

    #[test]
    fn cie_delta_e_is_zero_for_identical_linear_samples() {
        let sample = Rgb::new(0.2, 0.35, 0.5);
        assert!(delta_e76(sample, sample) < 1e-12);
    }

    #[test]
    fn real_video_tracking_preview_apply_and_undo() {
        if !opentake_media::ffmpeg_status::ffmpeg_available() {
            eprintln!("SKIP: ffmpeg unavailable");
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let frames_dir = root.path().join("frames");
        std::fs::create_dir(&frames_dir).unwrap();
        for frame_index in 0..12_u32 {
            let mut image = image::RgbaImage::from_pixel(96, 72, image::Rgba([8, 8, 8, 255]));
            let offset_x = 20 + frame_index;
            let offset_y = 24 + frame_index / 2;
            for y in offset_y..offset_y + 20 {
                for x in offset_x..offset_x + 24 {
                    let local_x = x - offset_x;
                    let local_y = y - offset_y;
                    image.put_pixel(
                        x,
                        y,
                        image::Rgba([
                            (local_x * 9 + local_y * 3) as u8,
                            (local_x * 2 + local_y * 11) as u8,
                            (local_x * 7 + local_y * 5) as u8,
                            255,
                        ]),
                    );
                }
            }
            image
                .save(frames_dir.join(format!("frame-{frame_index:03}.png")))
                .unwrap();
        }
        let source = root.path().join("moving-subject.mp4");
        let status = Command::new(opentake_media::ffmpeg_status::ffmpeg_path())
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-framerate",
                "10",
            ])
            .arg("-i")
            .arg(frames_dir.join("frame-%03d.png"))
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());

        let core = AppCore::new();
        core.apply(EditCommand::SetTimelineSettings {
            fps: 10,
            width: 96,
            height: 72,
        })
        .unwrap();
        let bundle = root.path().join("Tracking.opentake");
        core.save_project(Some(bundle.clone())).unwrap();
        let runtime = core.runtime_snapshot();
        let imported = core
            .import_media_batch_for_project_persisted(
                runtime.project_epoch,
                &bundle,
                vec![PreparedMediaImportOp::ImportFile {
                    path: source,
                    name: "moving-subject.mp4".into(),
                    probe: ProbedMedia {
                        duration_secs: 1.2,
                        width: Some(96),
                        height: Some(72),
                        fps: Some(10.0),
                        has_audio: false,
                        color: None,
                    },
                    folder: None,
                }],
            )
            .unwrap();
        let asset_id = imported[0].entry.id.clone();
        let placed = core
            .apply(EditCommand::AddClipsAutoTrack {
                entries: vec![ClipEntry {
                    media_ref: asset_id,
                    media_type: ClipType::Video,
                    source_clip_type: ClipType::Video,
                    track_index: 0,
                    start_frame: 0,
                    duration_frames: 12,
                    trim_start_frame: None,
                    trim_end_frame: None,
                    has_audio: false,
                    add_linked_audio: false,
                    transform: None,
                }],
            })
            .unwrap();
        let clip_id = placed.affected_clip_ids[0].clone();
        let bridge = TauriAdvancedWorkflowBridge::new(
            core.clone(),
            root.path().join("cache"),
            root.path().join("models"),
        );
        let request = TrackMotionArgs {
            clip_id: clip_id.clone(),
            region: json!({
                "x": 20.0 / 96.0,
                "y": 24.0 / 72.0,
                "width": 24.0 / 96.0,
                "height": 20.0 / 72.0
            }),
            start_frame: None,
            end_frame: None,
            apply: Some(false),
        };
        let preview = bridge
            .track_motion(request.clone(), &MediaCancelToken::new())
            .unwrap();
        assert!(preview.action_name.is_none());
        assert_eq!(preview.result["applied"], false);
        assert!(preview.result["keyframes"].as_array().unwrap().len() >= 2);
        assert!(core.runtime_snapshot().timeline.tracks[0].clips[0]
            .position_track
            .is_none());

        let applied = bridge
            .track_motion(
                TrackMotionArgs {
                    apply: Some(true),
                    ..request.clone()
                },
                &MediaCancelToken::new(),
            )
            .unwrap();
        assert_eq!(applied.result["applied"], true);
        assert_eq!(applied.action_name.as_deref(), Some("Set Keyframes"));
        let after = core.runtime_snapshot();
        let clip = after.timeline.tracks[0]
            .clips
            .iter()
            .find(|clip| clip.id == clip_id)
            .unwrap();
        assert!(clip.position_track.as_ref().unwrap().keyframes.len() >= 2);
        let tracked_position = clip.position_track.clone();
        core.save_project(None).unwrap();
        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();
        let reopened_snapshot = reopened.runtime_snapshot();
        assert_eq!(
            reopened_snapshot.timeline.tracks[0].clips[0].position_track,
            tracked_position
        );

        let out = root.path().join("tracked-export.mp4");
        let summary = crate::export::run_export(
            &after.timeline,
            &after.media,
            &after.project_dir,
            &crate::export::ExportRequest {
                out_path: out.to_string_lossy().into_owned(),
                codec: crate::export::ExportCodec::H264,
                quality: crate::export::ExportQuality::P720,
            },
        )
        .unwrap();
        assert_eq!(summary.frame_count, 12);
        assert!(!summary.has_audio);
        let exported = probe(&out).unwrap();
        assert!(exported.has_video);
        assert!(!exported.has_audio);
        core.undo().unwrap();
        let undone = core.runtime_snapshot();
        assert!(undone.timeline.tracks[0].clips[0].position_track.is_none());

        let cancelled = MediaCancelToken::new();
        cancelled.cancel();
        let error = bridge
            .track_motion(
                TrackMotionArgs {
                    apply: Some(true),
                    ..request
                },
                &cancelled,
            )
            .expect_err("pre-cancelled analysis must fail");
        assert_eq!(error.kind, AdvancedWorkflowErrorKind::Cancelled);
        assert!(core.runtime_snapshot().timeline.tracks[0].clips[0]
            .position_track
            .is_none());
    }

    #[test]
    fn official_matting_preview_apply_undo_and_reopen() {
        let Some(model_source) = std::env::var_os("OPENTAKE_TEST_RVM_MODEL") else {
            eprintln!("SKIP: OPENTAKE_TEST_RVM_MODEL is not set");
            return;
        };
        if !opentake_media::ffmpeg_status::ffmpeg_available() {
            eprintln!("SKIP: ffmpeg unavailable");
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let models_dir = root.path().join("models");
        let model_destination = opentake_media::analysis::matting_model_path(&models_dir);
        std::fs::create_dir_all(model_destination.parent().unwrap()).unwrap();
        std::fs::copy(model_source, model_destination).unwrap();

        let source = root.path().join("matting-source.mp4");
        let status = Command::new(opentake_media::ffmpeg_status::ffmpeg_path())
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=64x64:rate=5:duration=0.4",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=0.4",
                "-shortest",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
            ])
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());

        let core = AppCore::new();
        core.apply(EditCommand::SetTimelineSettings {
            fps: 5,
            width: 64,
            height: 64,
        })
        .unwrap();
        let bundle = root.path().join("Matting.opentake");
        core.save_project(Some(bundle.clone())).unwrap();
        let runtime = core.runtime_snapshot();
        let imported = core
            .import_media_batch_for_project_persisted(
                runtime.project_epoch,
                &bundle,
                vec![PreparedMediaImportOp::ImportFile {
                    path: source,
                    name: "matting-source.mp4".into(),
                    probe: ProbedMedia {
                        duration_secs: 0.4,
                        width: Some(64),
                        height: Some(64),
                        fps: Some(5.0),
                        has_audio: true,
                        color: None,
                    },
                    folder: None,
                }],
            )
            .unwrap();
        let source_asset_id = imported[0].entry.id.clone();
        let placed = core
            .apply(EditCommand::AddClipsAutoTrack {
                entries: vec![ClipEntry {
                    media_ref: source_asset_id.clone(),
                    media_type: ClipType::Video,
                    source_clip_type: ClipType::Video,
                    track_index: 0,
                    start_frame: 0,
                    duration_frames: 2,
                    trim_start_frame: None,
                    trim_end_frame: None,
                    has_audio: true,
                    add_linked_audio: false,
                    transform: None,
                }],
            })
            .unwrap();
        let clip_id = placed.affected_clip_ids[0].clone();
        let bridge =
            TauriAdvancedWorkflowBridge::new(core.clone(), root.path().join("cache"), models_dir);
        assert!(bridge.supported_tools().contains(&ToolName::GenerateMatte));
        let request = GenerateMatteArgs {
            clip_id: clip_id.clone(),
            model: None,
            start_frame: None,
            end_frame: None,
            apply: Some(false),
        };
        let preview = bridge
            .generate_matte(request.clone(), &MediaCancelToken::new())
            .unwrap();
        assert_eq!(preview.result["applied"], false);
        assert_eq!(core.media().entries.len(), 1);
        assert_eq!(
            core.runtime_snapshot().timeline.tracks[0].clips[0].media_ref,
            source_asset_id
        );

        let applied = bridge
            .generate_matte(
                GenerateMatteArgs {
                    apply: Some(true),
                    ..request.clone()
                },
                &MediaCancelToken::new(),
            )
            .unwrap();
        assert_eq!(applied.result["applied"], true);
        assert_eq!(applied.action_name.as_deref(), Some("Edit Motion Graphic"));
        let generated_asset_id = applied.result["assetId"].as_str().unwrap().to_string();
        let generated = core
            .media()
            .entries
            .into_iter()
            .find(|entry| entry.id == generated_asset_id)
            .unwrap();
        assert!(generated.carries_straight_alpha());
        assert_eq!(generated.has_audio, Some(true));
        assert_eq!(
            generated
                .generation_input
                .as_ref()
                .unwrap()
                .source_asset_id
                .as_deref(),
            Some(source_asset_id.as_str())
        );
        assert_eq!(
            core.runtime_snapshot().timeline.tracks[0].clips[0].media_ref,
            generated_asset_id
        );

        core.undo().unwrap();
        assert_eq!(
            core.runtime_snapshot().timeline.tracks[0].clips[0].media_ref,
            source_asset_id
        );
        core.redo().unwrap();
        core.save_project(None).unwrap();
        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();
        let reopened_clip = &reopened.runtime_snapshot().timeline.tracks[0].clips[0];
        assert_eq!(reopened_clip.id, clip_id);
        assert_eq!(reopened_clip.media_ref, generated_asset_id);
        assert!(reopened
            .media()
            .entries
            .iter()
            .any(|entry| entry.id == source_asset_id));

        let cancelled = MediaCancelToken::new();
        cancelled.cancel();
        let error = bridge
            .generate_matte(request, &cancelled)
            .expect_err("pre-cancelled matting must fail even when cached");
        assert_eq!(error.kind, AdvancedWorkflowErrorKind::Cancelled);
    }

    #[test]
    fn object_removal_preview_range_apply_undo_reopen_and_failure_atomicity() {
        if !opentake_media::ffmpeg_status::ffmpeg_available() {
            eprintln!("SKIP: ffmpeg unavailable");
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let frames_dir = root.path().join("frames");
        std::fs::create_dir(&frames_dir).unwrap();
        for frame_index in 0..8_u32 {
            let mut image = image::RgbaImage::from_pixel(
                64,
                48,
                image::Rgba([30 + frame_index as u8, 90, 140, 255]),
            );
            for y in 17..31 {
                for x in 24..40 {
                    image.put_pixel(x, y, image::Rgba([240, 12, 12, 255]));
                }
            }
            image
                .save(frames_dir.join(format!("frame-{frame_index:03}.png")))
                .unwrap();
        }
        let source = root.path().join("object-removal-source.mp4");
        let status = Command::new(opentake_media::ffmpeg_status::ffmpeg_path())
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-framerate",
                "4",
            ])
            .arg("-i")
            .arg(frames_dir.join("frame-%03d.png"))
            .args([
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=2",
                "-shortest",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
            ])
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());

        let core = AppCore::new();
        core.apply(EditCommand::SetTimelineSettings {
            fps: 4,
            width: 64,
            height: 48,
        })
        .unwrap();
        let bundle = root.path().join("ObjectRemoval.opentake");
        core.save_project(Some(bundle.clone())).unwrap();
        let runtime = core.runtime_snapshot();
        let imported = core
            .import_media_batch_for_project_persisted(
                runtime.project_epoch,
                &bundle,
                vec![PreparedMediaImportOp::ImportFile {
                    path: source,
                    name: "object-removal-source.mp4".into(),
                    probe: ProbedMedia {
                        duration_secs: 2.0,
                        width: Some(64),
                        height: Some(48),
                        fps: Some(4.0),
                        has_audio: true,
                        color: None,
                    },
                    folder: None,
                }],
            )
            .unwrap();
        let source_asset_id = imported[0].entry.id.clone();
        let placed = core
            .apply(EditCommand::AddClipsAutoTrack {
                entries: vec![ClipEntry {
                    media_ref: source_asset_id.clone(),
                    media_type: ClipType::Video,
                    source_clip_type: ClipType::Video,
                    track_index: 0,
                    start_frame: 0,
                    duration_frames: 8,
                    trim_start_frame: None,
                    trim_end_frame: None,
                    has_audio: true,
                    add_linked_audio: false,
                    transform: None,
                }],
            })
            .unwrap();
        let clip_id = placed.affected_clip_ids[0].clone();
        let mask = Mask {
            shape: MaskShape::Circle {
                center: Point2::new(0.5, 0.5),
                radius: Point2::new(0.16, 0.22),
            },
            feather: 0.01,
            ..Mask::default()
        };
        core.apply(EditCommand::SetMasks {
            clip_ids: vec![clip_id.clone()],
            masks: vec![mask.clone()],
        })
        .unwrap();
        core.save_project(None).unwrap();

        let bridge = TauriAdvancedWorkflowBridge::new(
            core.clone(),
            root.path().join("cache"),
            root.path().join("models"),
        );
        assert!(bridge.supported_tools().contains(&ToolName::RemoveObject));
        let request = RemoveObjectArgs {
            clip_id: clip_id.clone(),
            mask_id: "primary".into(),
            start_frame: Some(2),
            end_frame: Some(6),
            provider: Some("local".into()),
            model: Some("opentake-boundary-fill-v1".into()),
            cost_authorized: None,
            apply: Some(false),
        };
        let before_preview = core.runtime_snapshot();
        let preview = bridge
            .remove_object(request.clone(), &MediaCancelToken::new())
            .unwrap();
        assert_eq!(preview.result["applied"], false);
        assert_eq!(core.runtime_snapshot().timeline, before_preview.timeline);
        assert_eq!(core.runtime_snapshot().media, before_preview.media);
        let preview_path = PathBuf::from(preview.result["previewPath"].as_str().unwrap());
        let decoded = decode_frames_at_cancellable(
            &preview_path,
            &[0.25, 0.75],
            &FrameRequest {
                max_size: (64, 48),
                tolerance_secs: 0.1,
                ..FrameRequest::default()
            },
            &MediaCancelToken::new(),
        )
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
        let center = (24 * 64 + 32) * 4;
        assert!(
            decoded[0].1.rgba[center] > 180,
            "outside-range frame changed"
        );
        assert!(decoded[1].1.rgba[center] < 120, "masked object remained");

        let before_failure = core.runtime_snapshot();
        let error = bridge
            .remove_object(
                RemoveObjectArgs {
                    mask_id: "mask-99".into(),
                    ..request.clone()
                },
                &MediaCancelToken::new(),
            )
            .expect_err("missing mask must fail");
        assert_eq!(error.kind, AdvancedWorkflowErrorKind::ResourceNotFound);
        let after_failure = core.runtime_snapshot();
        assert_eq!(after_failure.timeline, before_failure.timeline);
        assert_eq!(after_failure.media, before_failure.media);
        assert_eq!(after_failure.version, before_failure.version);

        let applied = bridge
            .remove_object(
                RemoveObjectArgs {
                    apply: Some(true),
                    ..request.clone()
                },
                &MediaCancelToken::new(),
            )
            .unwrap();
        assert_eq!(applied.action_name.as_deref(), Some("Remove Masked Object"));
        let generated_asset_id = applied.result["assetId"].as_str().unwrap().to_string();
        let after_apply = core.runtime_snapshot();
        assert_eq!(after_apply.timeline.tracks[0].clips[0].id, clip_id);
        assert_eq!(
            after_apply.timeline.tracks[0].clips[0].media_ref,
            generated_asset_id
        );
        assert!(after_apply.timeline.tracks[0].clips[0].masks.is_empty());
        let generated = core
            .media()
            .entries
            .into_iter()
            .find(|entry| entry.id == generated_asset_id)
            .unwrap();
        let (applied_path, applied_is_video) =
            crate::transcribe::resolve_asset_from_snapshot(&after_apply, &generated_asset_id)
                .unwrap();
        assert!(applied_is_video);
        assert_eq!(
            file_sha256(&applied_path).unwrap(),
            file_sha256(&preview_path).unwrap(),
            "Apply must publish the exact reviewed derivative consumed by playback/export"
        );
        assert_eq!(generated.has_audio, Some(true));
        let provenance = generated.generation_input.unwrap();
        assert_eq!(provenance.provider.as_deref(), Some("opentake-local"));
        assert_eq!(provenance.model, "opentake-boundary-fill-v1");
        assert_eq!(
            provenance.source_asset_id.as_deref(),
            Some(source_asset_id.as_str())
        );

        core.undo().unwrap();
        let undone = core.runtime_snapshot();
        assert_eq!(
            undone.timeline.tracks[0].clips[0].media_ref,
            source_asset_id
        );
        assert_eq!(undone.timeline.tracks[0].clips[0].masks, vec![mask]);
        core.redo().unwrap();
        core.save_project(None).unwrap();
        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();
        let reopened_clip = &reopened.runtime_snapshot().timeline.tracks[0].clips[0];
        assert_eq!(reopened_clip.id, clip_id);
        assert_eq!(reopened_clip.media_ref, generated_asset_id);
        assert!(reopened_clip.masks.is_empty());
        assert!(reopened
            .media()
            .entries
            .iter()
            .any(|entry| entry.id == source_asset_id));

        let cancelled = MediaCancelToken::new();
        cancelled.cancel();
        let error = bridge
            .remove_object(request, &cancelled)
            .expect_err("pre-cancelled cached preview must fail");
        assert_eq!(error.kind, AdvancedWorkflowErrorKind::Cancelled);
    }

    #[test]
    fn color_match_improves_delta_e_preserves_luma_and_persists_editable_grade() {
        let root = tempfile::tempdir().unwrap();
        let target_path = root.path().join("target.png");
        let reference_path = root.path().join("reference.png");
        let black_path = root.path().join("black.png");
        image::RgbaImage::from_pixel(64, 48, image::Rgba([130, 105, 90, 255]))
            .save(&target_path)
            .unwrap();
        image::RgbaImage::from_pixel(64, 48, image::Rgba([95, 120, 145, 255]))
            .save(&reference_path)
            .unwrap();
        image::RgbaImage::from_pixel(64, 48, image::Rgba([0, 0, 0, 255]))
            .save(&black_path)
            .unwrap();

        let core = AppCore::new();
        core.apply(EditCommand::SetTimelineSettings {
            fps: 10,
            width: 64,
            height: 48,
        })
        .unwrap();
        let bundle = root.path().join("ColorMatch.opentake");
        core.save_project(Some(bundle.clone())).unwrap();
        let runtime = core.runtime_snapshot();
        let imported = core
            .import_media_batch_for_project_persisted(
                runtime.project_epoch,
                &bundle,
                vec![
                    PreparedMediaImportOp::ImportFile {
                        path: target_path,
                        name: "target.png".into(),
                        probe: ProbedMedia {
                            duration_secs: 0.0,
                            width: Some(64),
                            height: Some(48),
                            ..ProbedMedia::default()
                        },
                        folder: None,
                    },
                    PreparedMediaImportOp::ImportFile {
                        path: reference_path,
                        name: "reference.png".into(),
                        probe: ProbedMedia {
                            duration_secs: 0.0,
                            width: Some(64),
                            height: Some(48),
                            ..ProbedMedia::default()
                        },
                        folder: None,
                    },
                    PreparedMediaImportOp::ImportFile {
                        path: black_path,
                        name: "black.png".into(),
                        probe: ProbedMedia {
                            duration_secs: 0.0,
                            width: Some(64),
                            height: Some(48),
                            ..ProbedMedia::default()
                        },
                        folder: None,
                    },
                ],
            )
            .unwrap();
        let target_id = imported[0].entry.id.clone();
        let reference_id = imported[1].entry.id.clone();
        let black_id = imported[2].entry.id.clone();
        let placed = core
            .apply(EditCommand::AddClipsAutoTrack {
                entries: vec![ClipEntry {
                    media_ref: target_id,
                    media_type: ClipType::Image,
                    source_clip_type: ClipType::Image,
                    track_index: 0,
                    start_frame: 10,
                    duration_frames: 20,
                    trim_start_frame: None,
                    trim_end_frame: None,
                    has_audio: false,
                    add_linked_audio: false,
                    transform: None,
                }],
            })
            .unwrap();
        let clip_id = placed.affected_clip_ids[0].clone();
        core.save_project(None).unwrap();
        let bridge = TauriAdvancedWorkflowBridge::new(
            core.clone(),
            root.path().join("cache"),
            root.path().join("models"),
        );
        assert!(bridge.supported_tools().contains(&ToolName::MatchColor));
        let request = MatchColorArgs {
            clip_id: clip_id.clone(),
            reference_media_ref: reference_id.clone(),
            reference_frame: Some(0),
            target_frame: Some(12),
            apply: Some(false),
        };
        let before = core.runtime_snapshot();
        let preview = bridge
            .match_color(request.clone(), &MediaCancelToken::new())
            .unwrap();
        assert_eq!(preview.result["applied"], false);
        assert!(preview.result["deltaEAfter"].as_f64().unwrap() < 0.01);
        assert!(
            preview.result["deltaEAfter"].as_f64().unwrap()
                < preview.result["deltaEBefore"].as_f64().unwrap()
        );
        assert!(
            (preview.result["targetLumaAfter"].as_f64().unwrap()
                - preview.result["targetLumaBefore"].as_f64().unwrap())
            .abs()
                < 0.01
        );
        assert_eq!(core.runtime_snapshot().timeline, before.timeline);

        let failure_before = core.runtime_snapshot();
        let error = bridge
            .match_color(
                MatchColorArgs {
                    reference_media_ref: black_id,
                    ..request.clone()
                },
                &MediaCancelToken::new(),
            )
            .expect_err("black reference must be refused");
        assert_eq!(error.kind, AdvancedWorkflowErrorKind::AnalysisLowConfidence);
        let failure_after = core.runtime_snapshot();
        assert_eq!(failure_after.timeline, failure_before.timeline);
        assert_eq!(failure_after.version, failure_before.version);

        let applied = bridge
            .match_color(
                MatchColorArgs {
                    apply: Some(true),
                    ..request.clone()
                },
                &MediaCancelToken::new(),
            )
            .unwrap();
        assert_eq!(applied.action_name.as_deref(), Some("Match Color"));
        let after_apply = core.runtime_snapshot();
        let matched = &after_apply.timeline.tracks[0].clips[0];
        assert!(matched.color_grade.is_some());
        let provenance = matched.color_match_input.as_ref().unwrap();
        assert_eq!(provenance.reference_media_ref, reference_id);
        assert_eq!(provenance.reference_frame, 0);
        assert_eq!(provenance.target_frame, 12);
        assert!(provenance.delta_e_after < provenance.delta_e_before);

        core.undo().unwrap();
        let undone = core.runtime_snapshot();
        assert!(undone.timeline.tracks[0].clips[0].color_grade.is_none());
        assert!(undone.timeline.tracks[0].clips[0]
            .color_match_input
            .is_none());
        core.redo().unwrap();
        core.save_project(None).unwrap();
        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();
        let reopened_clip = &reopened.runtime_snapshot().timeline.tracks[0].clips[0];
        assert_eq!(reopened_clip.color_grade, matched.color_grade);
        assert_eq!(reopened_clip.color_match_input, matched.color_match_input);

        let cancelled = MediaCancelToken::new();
        cancelled.cancel();
        let error = bridge
            .match_color(request, &cancelled)
            .expect_err("pre-cancelled color match must fail");
        assert_eq!(error.kind, AdvancedWorkflowErrorKind::Cancelled);
    }

    #[test]
    fn stem_bridge_imports_provenance_aligned_tracks_undo_reopen_and_cancel() {
        if !opentake_media::ffmpeg_status::ffmpeg_available() {
            eprintln!("SKIP: ffmpeg unavailable");
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("mixture.wav");
        let sample_rate = 48_000_u32;
        let frames = 4_800_usize;
        let data_len = (frames * 2 * 2) as u32;
        let mut wav = Vec::with_capacity(44 + data_len as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&(sample_rate * 4).to_le_bytes());
        wav.extend_from_slice(&4_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for frame in 0..frames {
            let time = frame as f32 / sample_rate as f32;
            let vocal = 0.25 * (std::f32::consts::TAU * 440.0 * time).sin();
            let music = 0.18 * (std::f32::consts::TAU * 997.0 * time).sin();
            for sample in [vocal + music, vocal - music] {
                wav.extend_from_slice(&((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
            }
        }
        std::fs::write(&source, wav).unwrap();

        let core = AppCore::new();
        core.apply(EditCommand::SetTimelineSettings {
            fps: 30,
            width: 64,
            height: 64,
        })
        .unwrap();
        let bundle = root.path().join("Stems.opentake");
        core.save_project(Some(bundle.clone())).unwrap();
        let snapshot = core.runtime_snapshot();
        let imported = core
            .import_media_batch_for_project_persisted(
                snapshot.project_epoch,
                &bundle,
                vec![PreparedMediaImportOp::ImportFile {
                    path: source,
                    name: "mixture.wav".into(),
                    probe: ProbedMedia {
                        duration_secs: 0.1,
                        has_audio: true,
                        ..ProbedMedia::default()
                    },
                    folder: None,
                }],
            )
            .unwrap();
        let source_id = imported[0].entry.id.clone();
        let bridge = TauriAdvancedWorkflowBridge::new(
            core.clone(),
            root.path().join("cache"),
            root.path().join("models"),
        );
        assert!(bridge.supported_tools().contains(&ToolName::SeparateStems));
        let request = SeparateStemsArgs {
            media_ref: source_id.clone(),
            provider: Some("local".into()),
            model: Some("opentake-center-v1".into()),
            import_to_tracks: Some(true),
            start_frame: Some(45),
        };
        let separated = bridge
            .separate_stems(request.clone(), &MediaCancelToken::new())
            .unwrap();
        assert_eq!(
            separated.action_name.as_deref(),
            Some("Import Stems To Tracks")
        );
        assert_eq!(separated.result["importedToTracks"], true);
        assert_eq!(separated.result["clipIds"].as_array().unwrap().len(), 2);
        let after = core.runtime_snapshot();
        assert_eq!(after.media.entries.len(), 3);
        assert_eq!(after.timeline.tracks.len(), 2);
        assert!(after.timeline.tracks.iter().all(|track| {
            track.kind == ClipType::Audio
                && track.clips.len() == 1
                && track.clips[0].start_frame == 45
                && track.clips[0].duration_frames == 3
        }));
        for stem in &after.media.entries[1..] {
            let provenance = stem.generation_input.as_ref().unwrap();
            assert_eq!(
                provenance.source_asset_id.as_deref(),
                Some(source_id.as_str())
            );
            assert_eq!(provenance.provider.as_deref(), Some("local"));
            assert_eq!(provenance.model, "opentake-center-v1");
        }

        core.undo().unwrap();
        assert!(core.runtime_snapshot().timeline.tracks.is_empty());
        assert_eq!(core.media().entries.len(), 3);
        core.redo().unwrap();
        core.save_project(None).unwrap();
        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();
        assert_eq!(reopened.runtime_snapshot().timeline.tracks.len(), 2);
        assert_eq!(reopened.media().entries.len(), 3);

        let before_cancel = core.runtime_snapshot();
        let cancelled = MediaCancelToken::new();
        cancelled.cancel();
        let error = bridge
            .separate_stems(request, &cancelled)
            .expect_err("pre-cancelled separation must fail");
        assert_eq!(error.kind, AdvancedWorkflowErrorKind::Cancelled);
        let after_cancel = core.runtime_snapshot();
        assert_eq!(after_cancel.timeline, before_cancel.timeline);
        assert_eq!(after_cancel.media, before_cancel.media);
    }

    #[derive(Clone)]
    struct MockCaptionTranslator {
        result: Result<CaptionTranslationProviderResult, AdvancedWorkflowError>,
    }

    impl CaptionTranslationProvider for MockCaptionTranslator {
        fn translate(
            &self,
            _provider: &str,
            _model: &str,
            _source_locale: &str,
            _target_locale: &str,
            _captions: &[CaptionTranslationDraft],
            _cancel: &MediaCancelToken,
        ) -> Result<CaptionTranslationProviderResult, AdvancedWorkflowError> {
            self.result.clone()
        }
    }

    fn caption_translation_fixture() -> (AppCore, Vec<String>) {
        let core = AppCore::new();
        let placed = core
            .apply(EditCommand::AddCaptions {
                entries: [(4, 12, "Hello"), (21, 15, "World")]
                    .into_iter()
                    .map(
                        |(start_frame, duration_frames, content)| opentake_ops::CaptionEntry {
                            start_frame,
                            duration_frames,
                            content: content.into(),
                            text_style: opentake_domain::TextStyle::default(),
                            transform: opentake_domain::Transform::default(),
                            caption_group_id: "captions-1".into(),
                        },
                    )
                    .collect(),
            })
            .unwrap();
        (core, placed.affected_clip_ids)
    }

    fn translation_args(ids: Vec<String>, apply: bool) -> TranslateCaptionsArgs {
        TranslateCaptionsArgs {
            caption_clip_ids: ids,
            source_locale: Some("en-US".into()),
            target_locale: "zh-CN".into(),
            provider: Some("openai".into()),
            model: Some("mock-v1".into()),
            cost_authorized: Some(true),
            apply: Some(apply),
        }
    }

    #[test]
    fn caption_translation_mock_success_preserves_timing_undo_and_reopen() {
        let root = tempfile::tempdir().unwrap();
        let bundle = root.path().join("CaptionTranslation.opentake");
        let (core, ids) = caption_translation_fixture();
        core.save_project(Some(bundle.clone())).unwrap();
        let before = core.runtime_snapshot();
        let translator = MockCaptionTranslator {
            result: Ok(CaptionTranslationProviderResult {
                translations: vec![
                    CaptionTranslationDraft {
                        id: ids[0].clone(),
                        text: "你好".into(),
                    },
                    CaptionTranslationDraft {
                        id: ids[1].clone(),
                        text: "世界".into(),
                    },
                ],
                errors: vec![],
            }),
        };
        let bridge = TauriAdvancedWorkflowBridge::with_caption_translator(
            core.clone(),
            root.path().join("cache"),
            root.path().join("models"),
            Arc::new(translator),
        );
        assert!(bridge
            .supported_tools()
            .contains(&ToolName::TranslateCaptions));
        let preview = bridge
            .translate_captions(
                translation_args(ids.clone(), false),
                &MediaCancelToken::new(),
            )
            .unwrap();
        assert_eq!(preview.result["translatedCount"], 2);
        assert_eq!(core.runtime_snapshot().timeline, before.timeline);
        let applied = bridge
            .translate_captions(
                translation_args(ids.clone(), true),
                &MediaCancelToken::new(),
            )
            .unwrap();
        assert_eq!(applied.action_name.as_deref(), Some("Translate Captions"));
        let after = core.runtime_snapshot();
        for (index, clip) in after.timeline.tracks[0].clips.iter().enumerate() {
            let original = &before.timeline.tracks[0].clips[index];
            assert_eq!(clip.id, original.id);
            assert_eq!(clip.start_frame, original.start_frame);
            assert_eq!(clip.duration_frames, original.duration_frames);
            assert_eq!(clip.caption_group_id, original.caption_group_id);
            let provenance = clip.caption_translation_input.as_ref().unwrap();
            assert_eq!(provenance.source_locale, "en-US");
            assert_eq!(provenance.target_locale, "zh-CN");
        }
        core.undo().unwrap();
        assert_eq!(core.runtime_snapshot().timeline, before.timeline);
        core.redo().unwrap();
        core.save_project(None).unwrap();
        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();
        assert_eq!(
            reopened.runtime_snapshot().timeline,
            core.runtime_snapshot().timeline
        );
    }

    #[test]
    fn caption_translation_mock_partial_and_failure_keep_failed_text_untouched() {
        let root = tempfile::tempdir().unwrap();
        let (core, ids) = caption_translation_fixture();
        let before = core.runtime_snapshot();
        let partial = TauriAdvancedWorkflowBridge::with_caption_translator(
            core.clone(),
            root.path().join("cache"),
            root.path().join("models"),
            Arc::new(MockCaptionTranslator {
                result: Ok(CaptionTranslationProviderResult {
                    translations: vec![CaptionTranslationDraft {
                        id: ids[0].clone(),
                        text: "Bonjour".into(),
                    }],
                    errors: vec![CaptionTranslationFailure {
                        id: ids[1].clone(),
                        message: "temporary refusal".into(),
                    }],
                }),
            }),
        );
        let result = partial
            .translate_captions(
                translation_args(ids.clone(), true),
                &MediaCancelToken::new(),
            )
            .unwrap();
        assert_eq!(result.result["translatedCount"], 1);
        assert_eq!(result.result["errors"].as_array().unwrap().len(), 1);
        let after_partial = core.runtime_snapshot();
        assert_eq!(
            after_partial.timeline.tracks[0].clips[0]
                .text_content
                .as_deref(),
            Some("Bonjour")
        );
        assert_eq!(
            after_partial.timeline.tracks[0].clips[1]
                .text_content
                .as_deref(),
            Some("World")
        );
        core.undo().unwrap();
        assert_eq!(core.runtime_snapshot().timeline, before.timeline);

        let failed = TauriAdvancedWorkflowBridge::with_caption_translator(
            core.clone(),
            root.path().join("cache-2"),
            root.path().join("models"),
            Arc::new(MockCaptionTranslator {
                result: Err(advanced_execution("mock provider failed")),
            }),
        );
        assert!(failed
            .translate_captions(translation_args(ids, true), &MediaCancelToken::new())
            .is_err());
        assert_eq!(core.runtime_snapshot().timeline, before.timeline);
    }

    #[test]
    fn script_to_video_three_segment_plan_apply_cancel_reopen_and_export() {
        if !opentake_media::ffmpeg_status::ffmpeg_available()
            || !opentake_media::ffmpeg_status::ffprobe_available()
        {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let bundle = root.path().join("ScriptVideo.opentake");
        let core = AppCore::new();
        core.apply(EditCommand::SetTimelineSettings {
            fps: 6,
            width: 96,
            height: 54,
        })
        .unwrap();
        core.save_project(Some(bundle.clone())).unwrap();
        let colors = [[220, 40, 40, 255], [40, 220, 40, 255], [40, 40, 220, 255]];
        let mut imports = Vec::new();
        for (index, color) in colors.into_iter().enumerate() {
            let path = root.path().join(format!("scene-{index}.png"));
            image::RgbaImage::from_pixel(96, 54, image::Rgba(color))
                .save(&path)
                .unwrap();
            imports.push(PreparedMediaImportOp::ImportFile {
                path,
                name: format!("scene-{index}.png"),
                probe: ProbedMedia {
                    duration_secs: 0.0,
                    width: Some(96),
                    height: Some(54),
                    ..ProbedMedia::default()
                },
                folder: None,
            });
        }
        let narration_path = root.path().join("narration.wav");
        let status = Command::new(opentake_media::ffmpeg_status::ffmpeg_path())
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
            ])
            .arg("sine=frequency=440:sample_rate=48000:duration=1")
            .arg(&narration_path)
            .status()
            .unwrap();
        assert!(status.success());
        imports.push(PreparedMediaImportOp::ImportFile {
            path: narration_path,
            name: "narration.wav".into(),
            probe: ProbedMedia {
                duration_secs: 1.0,
                has_audio: true,
                ..ProbedMedia::default()
            },
            folder: None,
        });
        let snapshot = core.runtime_snapshot();
        let imported = core
            .import_media_batch_for_project_persisted(snapshot.project_epoch, &bundle, imports)
            .unwrap();
        let visual_ids: Vec<_> = imported[..3]
            .iter()
            .map(|item| item.entry.id.clone())
            .collect();
        let narration_id = imported[3].entry.id.clone();
        let args = ScriptToVideoArgs {
            segments: visual_ids
                .iter()
                .enumerate()
                .map(|(index, media_ref)| {
                    let mut value = json!({
                        "script": format!("Scene {}", index + 1),
                        "mediaRef": media_ref,
                        "narrationMediaRef": narration_id,
                        "durationFrames": 6
                    });
                    if index < 2 {
                        value["transition"] = json!("crossDissolve");
                    }
                    value
                })
                .collect(),
            apply: Some(false),
        };
        let bridge = TauriAdvancedWorkflowBridge::new(
            core.clone(),
            root.path().join("cache"),
            root.path().join("models"),
        );
        assert!(bridge.supported_tools().contains(&ToolName::ScriptToVideo));
        let planned = bridge
            .script_to_video(args.clone(), &MediaCancelToken::new())
            .unwrap();
        assert_eq!(planned.action_name.as_deref(), Some("Plan Script Video"));
        assert_eq!(planned.result["segments"].as_array().unwrap().len(), 3);
        let after_plan = core.runtime_snapshot();
        assert!(after_plan.timeline.tracks.is_empty());
        assert_eq!(after_plan.timeline.script_assembly_plans.len(), 1);

        let cancelled = MediaCancelToken::new();
        cancelled.cancel();
        let error = bridge
            .script_to_video(
                ScriptToVideoArgs {
                    apply: Some(true),
                    ..args.clone()
                },
                &cancelled,
            )
            .expect_err("pre-cancelled assembly must not mutate");
        assert_eq!(error.kind, AdvancedWorkflowErrorKind::Cancelled);
        assert_eq!(core.runtime_snapshot().timeline, after_plan.timeline);

        let applied = bridge
            .script_to_video(
                ScriptToVideoArgs {
                    apply: Some(true),
                    ..args
                },
                &MediaCancelToken::new(),
            )
            .unwrap();
        assert_eq!(applied.action_name.as_deref(), Some("Build Script Video"));
        let assembled = core.runtime_snapshot();
        assert_eq!(assembled.timeline.tracks.len(), 2);
        assert_eq!(assembled.timeline.tracks[0].clips.len(), 3);
        assert_eq!(assembled.timeline.tracks[1].clips.len(), 3);
        for index in 0..3 {
            assert_eq!(
                assembled.timeline.tracks[0].clips[index].start_frame,
                index as i32 * 6
            );
            assert_eq!(
                assembled.timeline.tracks[1].clips[index].start_frame,
                index as i32 * 6
            );
        }
        for index in 0..2 {
            let transition = assembled.timeline.tracks[0].clips[index]
                .transition_out
                .as_ref()
                .unwrap();
            assert_eq!(
                transition.to_clip_id,
                assembled.timeline.tracks[0].clips[index + 1].id
            );
        }
        core.save_project(None).unwrap();
        let reopened = AppCore::new();
        reopened.open_project(&bundle).unwrap();
        assert_eq!(reopened.runtime_snapshot().timeline, assembled.timeline);

        let out = root.path().join("script-video.mp4");
        let summary = crate::export::run_export(
            &assembled.timeline,
            &assembled.media,
            &assembled.project_dir,
            &crate::export::ExportRequest {
                out_path: out.to_string_lossy().into_owned(),
                codec: crate::export::ExportCodec::H264,
                quality: crate::export::ExportQuality::P720,
            },
        )
        .unwrap();
        assert_eq!(summary.frame_count, 18);
        assert!(summary.has_audio);
        let output_probe = probe(&out).unwrap();
        assert!(output_probe.has_audio);

        core.undo().unwrap();
        let undone = core.runtime_snapshot();
        assert!(undone.timeline.tracks.is_empty());
        assert_eq!(undone.timeline.script_assembly_plans.len(), 1);
    }

    struct IdentityFixture {
        root: tempfile::TempDir,
        bundle: PathBuf,
        core: AppCore,
        portrait_id: String,
        audio_id: String,
        avatar_video: PathBuf,
        generated_voice: PathBuf,
    }

    fn identity_fixture() -> Option<IdentityFixture> {
        if !opentake_media::ffmpeg_status::ffmpeg_available()
            || !opentake_media::ffmpeg_status::ffprobe_available()
        {
            return None;
        }
        let root = tempfile::tempdir().unwrap();
        let bundle = root.path().join("Identity.opentake");
        let portrait = root.path().join("portrait.png");
        image::RgbaImage::from_pixel(96, 54, image::Rgba([80, 140, 220, 255]))
            .save(&portrait)
            .unwrap();
        let narration = root.path().join("narration.wav");
        assert!(Command::new(opentake_media::ffmpeg_status::ffmpeg_path())
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=1",
            ])
            .arg(&narration)
            .status()
            .unwrap()
            .success());
        let generated_voice = root.path().join("generated-voice.mp3");
        assert!(Command::new(opentake_media::ffmpeg_status::ffmpeg_path())
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(&narration)
            .args(["-c:a", "libmp3lame"])
            .arg(&generated_voice)
            .status()
            .unwrap()
            .success());
        let avatar_video = root.path().join("avatar.mp4");
        assert!(Command::new(opentake_media::ffmpeg_status::ffmpeg_path())
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-loop",
                "1",
                "-i"
            ])
            .arg(&portrait)
            .arg("-i")
            .arg(&narration)
            .args([
                "-t",
                "1",
                "-r",
                "6",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-shortest",
            ])
            .arg(&avatar_video)
            .status()
            .unwrap()
            .success());
        let core = AppCore::new();
        core.apply(EditCommand::SetTimelineSettings {
            fps: 6,
            width: 96,
            height: 54,
        })
        .unwrap();
        core.save_project(Some(bundle.clone())).unwrap();
        let to_probe = |path: &Path| {
            let value = probe(path).unwrap();
            ProbedMedia {
                duration_secs: value.duration_secs,
                width: value.width.and_then(|width| i32::try_from(width).ok()),
                height: value.height.and_then(|height| i32::try_from(height).ok()),
                fps: value.fps,
                has_audio: value.has_audio,
                color: value.color,
            }
        };
        let imported = core
            .import_media_batch_for_project_persisted(
                core.runtime_snapshot().project_epoch,
                &bundle,
                vec![
                    PreparedMediaImportOp::ImportFile {
                        path: portrait.clone(),
                        name: "portrait.png".into(),
                        probe: to_probe(&portrait),
                        folder: None,
                    },
                    PreparedMediaImportOp::ImportFile {
                        path: narration.clone(),
                        name: "narration.wav".into(),
                        probe: to_probe(&narration),
                        folder: None,
                    },
                ],
            )
            .unwrap();
        Some(IdentityFixture {
            root,
            bundle,
            core,
            portrait_id: imported[0].entry.id.clone(),
            audio_id: imported[1].entry.id.clone(),
            avatar_video,
            generated_voice,
        })
    }

    #[test]
    fn avatar_consent_failure_cancel_import_undo_reopen_and_export() {
        let Some(fixture) = identity_fixture() else {
            return;
        };
        let calls = Arc::new(Mutex::new(0));
        let provider = Arc::new(FixtureAvatarProvider {
            fixture: fixture.avatar_video.clone(),
            fail: false,
            calls: calls.clone(),
        });
        let voice = Arc::new(FixtureVoiceProvider {
            fixture: fixture.generated_voice.clone(),
            fail_generation: false,
            revoked: Arc::new(Mutex::new(HashSet::new())),
            calls: Arc::new(Mutex::new(Vec::new())),
        });
        let bridge = TauriAdvancedWorkflowBridge::with_identity_providers(
            fixture.core.clone(),
            fixture.root.path().join("cache"),
            fixture.root.path().join("models"),
            provider,
            voice,
        );
        let args = GenerateAvatarArgs {
            portrait_media_ref: fixture.portrait_id.clone(),
            audio_media_ref: fixture.audio_id.clone(),
            consent_id: "consent-avatar-fixture".into(),
            provider: Some("fal".into()),
            model: Some("fal-ai/sync-lipsync/v3/image-to-video".into()),
            cost_authorized: Some(true),
            start_frame: Some(0),
        };
        let before = fixture.core.runtime_snapshot();
        let denied = bridge
            .generate_avatar(
                GenerateAvatarArgs {
                    cost_authorized: Some(false),
                    ..args.clone()
                },
                &MediaCancelToken::new(),
            )
            .unwrap_err();
        assert_eq!(
            denied.kind,
            AdvancedWorkflowErrorKind::CostAuthorizationRequired
        );
        assert_eq!(*calls.lock().unwrap(), 0);
        let cancelled = MediaCancelToken::new();
        cancelled.cancel();
        assert_eq!(
            bridge
                .generate_avatar(args.clone(), &cancelled)
                .unwrap_err()
                .kind,
            AdvancedWorkflowErrorKind::Cancelled
        );
        assert_eq!(fixture.core.runtime_snapshot().media, before.media);
        let result = bridge
            .generate_avatar(args, &MediaCancelToken::new())
            .unwrap();
        assert_eq!(result.action_name.as_deref(), Some("Generate Avatar"));
        assert_eq!(*calls.lock().unwrap(), 1);
        let generated = fixture.core.runtime_snapshot();
        assert_eq!(
            generated.media.entries.len(),
            before.media.entries.len() + 1
        );
        assert_eq!(generated.timeline.tracks.len(), 1);
        let entry = generated.media.entries.last().unwrap();
        let provenance = entry.generation_input.as_ref().unwrap();
        assert_eq!(
            provenance.consent_id.as_deref(),
            Some("consent-avatar-fixture")
        );
        assert_eq!(
            provenance.reference_audio_asset_ids.as_deref(),
            Some([fixture.audio_id.clone()].as_slice())
        );
        assert_eq!(provenance.request_hash.as_ref().unwrap().len(), 64);
        assert!(!serde_json::to_string(&generated.media)
            .unwrap()
            .contains("api-key"));
        let reopened = AppCore::new();
        reopened.open_project(&fixture.bundle).unwrap();
        assert_eq!(reopened.runtime_snapshot().timeline, generated.timeline);
        let out = fixture.root.path().join("avatar-export.mp4");
        let summary = crate::export::run_export(
            &generated.timeline,
            &generated.media,
            &generated.project_dir,
            &crate::export::ExportRequest {
                out_path: out.to_string_lossy().into_owned(),
                codec: crate::export::ExportCodec::H264,
                quality: crate::export::ExportQuality::P720,
            },
        )
        .unwrap();
        assert_eq!(summary.frame_count, 6);
        assert!(summary.has_audio);
        assert!(probe(&out).unwrap().has_audio);
        fixture.core.undo().unwrap();
        let undone = fixture.core.runtime_snapshot();
        assert_eq!(undone.media, before.media);
        assert!(undone.timeline.tracks.is_empty());
    }

    #[test]
    fn voice_clone_consent_provider_cancel_revoke_and_reopen_contract() {
        let Some(fixture) = identity_fixture() else {
            return;
        };
        let calls = Arc::new(Mutex::new(Vec::new()));
        let revoked = Arc::new(Mutex::new(HashSet::new()));
        let voice = Arc::new(FixtureVoiceProvider {
            fixture: fixture.generated_voice.clone(),
            fail_generation: false,
            revoked: revoked.clone(),
            calls: calls.clone(),
        });
        let avatar = Arc::new(FixtureAvatarProvider {
            fixture: fixture.avatar_video.clone(),
            fail: false,
            calls: Arc::new(Mutex::new(0)),
        });
        let bridge = TauriAdvancedWorkflowBridge::with_identity_providers(
            fixture.core.clone(),
            fixture.root.path().join("cache"),
            fixture.root.path().join("models"),
            avatar,
            voice,
        );
        let invalid = bridge
            .clone_voice(
                CloneVoiceArgs {
                    action: "enroll".into(),
                    reference_audio_media_ref: Some(fixture.audio_id.clone()),
                    consent_id: "no".into(),
                    voice_name: Some("Narrator".into()),
                    cost_authorized: Some(true),
                    ..CloneVoiceArgs::default()
                },
                &MediaCancelToken::new(),
            )
            .unwrap_err();
        assert_eq!(invalid.kind, AdvancedWorkflowErrorKind::ConsentRequired);
        let enrollment = bridge
            .clone_voice(
                CloneVoiceArgs {
                    action: "enroll".into(),
                    reference_audio_media_ref: Some(fixture.audio_id.clone()),
                    consent_id: "consent-voice-fixture".into(),
                    voice_name: Some("Narrator".into()),
                    provider: Some("elevenlabs".into()),
                    model: Some("eleven_multilingual_v2".into()),
                    cost_authorized: Some(true),
                    ..CloneVoiceArgs::default()
                },
                &MediaCancelToken::new(),
            )
            .unwrap();
        let voice_id = enrollment.result["voiceId"].as_str().unwrap().to_string();
        assert_eq!(calls.lock().unwrap().as_slice(), ["enroll"]);
        let enrolled = fixture.core.runtime_snapshot();
        assert_eq!(enrolled.timeline.voice_models.len(), 1);
        assert!(!enrolled.timeline.voice_models[0].revoked);
        let cancelled = MediaCancelToken::new();
        cancelled.cancel();
        assert_eq!(
            bridge
                .clone_voice(
                    CloneVoiceArgs {
                        action: "generate".into(),
                        consent_id: "consent-voice-fixture".into(),
                        voice_id: Some(voice_id.clone()),
                        prompt: Some("Hello from the cloned voice".into()),
                        cost_authorized: Some(true),
                        ..CloneVoiceArgs::default()
                    },
                    &cancelled,
                )
                .unwrap_err()
                .kind,
            AdvancedWorkflowErrorKind::Cancelled
        );
        assert_eq!(calls.lock().unwrap().as_slice(), ["enroll"]);
        let failed_calls = Arc::new(Mutex::new(Vec::new()));
        let failed_bridge = TauriAdvancedWorkflowBridge::with_identity_providers(
            fixture.core.clone(),
            fixture.root.path().join("failed-cache"),
            fixture.root.path().join("models"),
            Arc::new(FixtureAvatarProvider {
                fixture: fixture.avatar_video.clone(),
                fail: false,
                calls: Arc::new(Mutex::new(0)),
            }),
            Arc::new(FixtureVoiceProvider {
                fixture: fixture.generated_voice.clone(),
                fail_generation: true,
                revoked: revoked.clone(),
                calls: failed_calls,
            }),
        );
        let before_failure = fixture.core.runtime_snapshot();
        assert!(failed_bridge
            .clone_voice(
                CloneVoiceArgs {
                    action: "generate".into(),
                    consent_id: "consent-voice-fixture".into(),
                    voice_id: Some(voice_id.clone()),
                    prompt: Some("Provider failure".into()),
                    cost_authorized: Some(true),
                    ..CloneVoiceArgs::default()
                },
                &MediaCancelToken::new(),
            )
            .is_err());
        assert_eq!(fixture.core.runtime_snapshot().media, before_failure.media);
        let generated = bridge
            .clone_voice(
                CloneVoiceArgs {
                    action: "generate".into(),
                    consent_id: "consent-voice-fixture".into(),
                    voice_id: Some(voice_id.clone()),
                    prompt: Some("Hello from the cloned voice".into()),
                    cost_authorized: Some(true),
                    ..CloneVoiceArgs::default()
                },
                &MediaCancelToken::new(),
            )
            .unwrap();
        assert_eq!(generated.result["imported"], true);
        let after_generation = fixture.core.runtime_snapshot();
        assert_eq!(
            after_generation.media.entries.len(),
            enrolled.media.entries.len() + 1
        );
        assert_eq!(after_generation.timeline.tracks.len(), 1);
        let generated_entry = after_generation.media.entries.last().unwrap();
        assert_eq!(
            generated_entry
                .generation_input
                .as_ref()
                .unwrap()
                .voice
                .as_deref(),
            Some(voice_id.as_str())
        );
        fixture.core.undo().unwrap();
        assert_eq!(fixture.core.runtime_snapshot().media, enrolled.media);
        let revoked_result = bridge
            .clone_voice(
                CloneVoiceArgs {
                    action: "revoke".into(),
                    consent_id: "consent-voice-fixture".into(),
                    voice_id: Some(voice_id.clone()),
                    ..CloneVoiceArgs::default()
                },
                &MediaCancelToken::new(),
            )
            .unwrap();
        assert_eq!(revoked_result.result["revoked"], true);
        assert!(revoked.lock().unwrap().contains("provider-voice-fixture"));
        fixture.core.undo().unwrap();
        assert!(fixture.core.runtime_snapshot().timeline.voice_models[0].revoked);
        let call_count = calls.lock().unwrap().len();
        let rejected = bridge
            .clone_voice(
                CloneVoiceArgs {
                    action: "generate".into(),
                    consent_id: "consent-voice-fixture".into(),
                    voice_id: Some(voice_id),
                    prompt: Some("Must not run".into()),
                    cost_authorized: Some(true),
                    ..CloneVoiceArgs::default()
                },
                &MediaCancelToken::new(),
            )
            .unwrap_err();
        assert_eq!(rejected.kind, AdvancedWorkflowErrorKind::ConsentRequired);
        assert_eq!(calls.lock().unwrap().len(), call_count);
        let reopened = AppCore::new();
        reopened.open_project(&fixture.bundle).unwrap();
        assert!(reopened.runtime_snapshot().timeline.voice_models[0].revoked);
    }
}
