//! The Captions-tab command: `generate_captions`.
//!
//! The UI-facing sibling of the `add_captions` MCP tool. Both run the SAME pure
//! pipeline (`opentake_media::caption_specs` for packing/timing, then
//! `EditCommand::AddCaptions` to place atomically); this command is what the
//! React Captions tab calls, mirroring upstream `EditorViewModel.generateCaptions`
//! (`EditorViewModel+Captions.swift:97-117`) driving `CaptionTab`.
//!
//! Flow: resolve caption-eligible clips (all, a track, or a clip selection);
//! transcribe each unique source (cached, language hint bypasses the cache);
//! auto-pick the dominant spoken track when the source is "auto"; build caption
//! specs with the pure builder using this timeline's canvas for text-fit and the
//! per-line transform; place them as one undoable "Generate Captions" action.
//!
//! DTOs are camelCase (`web/src/lib/types.ts` contract; the repo's #1 bug class),
//! with a serde round-trip test.

use serde::{Deserialize, Serialize};

use opentake_core::dto::{handle_edit_apply, EditResultDto};
use opentake_core::AppCore;
use opentake_domain::{Clip, ClipType, MediaManifest, TextLayout, TextStyle, Transform};
use opentake_media::{
    caption_specs, dominant_speech_track, CaptionCase, CaptionTarget, TranscriptionResult,
};
use opentake_ops::{CaptionEntry, EditCommand};
use tauri::State;

use crate::media::MediaState;

/// Caption style/placement defaults, 1:1 with upstream `AppTheme.Caption`
/// (`UI/AppTheme.swift:239-249`).
const DEFAULT_FONT_SIZE: f64 = 48.0;
const DEFAULT_CENTER_X: f64 = 0.5;
const DEFAULT_CENTER_Y: f64 = 0.9;
const MAX_TEXT_WIDTH_RATIO: f64 = 0.9;

/// Which clips to caption (mirrors the Captions tab's source selector). `Auto`
/// captions every eligible clip and then keeps the dominant spoken track; `Track`
/// captions one track; `Clips` captions a specific selection.
#[derive(Clone, Debug, Deserialize, PartialEq, Default)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CaptionSource {
    /// All eligible audio, then narrowed to the dominant spoken track.
    #[default]
    Auto,
    /// Only clips on the track with this id.
    #[serde(rename_all = "camelCase")]
    Track { track_id: String },
    /// Only these clip ids.
    #[serde(rename_all = "camelCase")]
    Clips { clip_ids: Vec<String> },
}

/// Letter case on the wire (`auto`/`upper`/`lower`), mapped onto [`CaptionCase`].
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum CaptionCaseDto {
    #[default]
    Auto,
    Upper,
    Lower,
}

impl From<CaptionCaseDto> for CaptionCase {
    fn from(c: CaptionCaseDto) -> Self {
        match c {
            CaptionCaseDto::Auto => CaptionCase::Auto,
            CaptionCaseDto::Upper => CaptionCase::Upper,
            CaptionCaseDto::Lower => CaptionCase::Lower,
        }
    }
}

/// The Captions-tab request (mirror of upstream `CaptionRequest`). Style is the
/// full [`TextStyle`] (font/size/color/background/…); placement is a normalized
/// canvas center. `language` is an optional BCP-47/ISO-639 hint.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionRequestDto {
    #[serde(default)]
    pub source: CaptionSource,
    #[serde(default)]
    pub style: Option<TextStyle>,
    #[serde(default)]
    pub center_x: Option<f64>,
    #[serde(default)]
    pub center_y: Option<f64>,
    #[serde(default)]
    pub text_case: CaptionCaseDto,
    #[serde(default)]
    pub censor_profanity: bool,
    #[serde(default)]
    pub language: Option<String>,
}

/// Result of a caption Generate: the edit outcome plus a caption count for the UI.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GenerateCaptionsResult {
    /// The underlying edit result (version bump, affected clip ids, …).
    pub edit: EditResultDto,
    /// How many caption clips were placed (0 when no speech was detected).
    pub caption_count: usize,
}

/// `generate_captions`: transcribe the selected source and place styled caption
/// clips on a fresh top track, as one undoable action. Errors surface as a
/// `Result::Err(String)` for the UI to show (model-not-installed guides the user
/// to `download_transcribe_model`). Returns `caption_count == 0` (not an error)
/// when nothing was captionable / no speech was found, matching upstream's empty
/// return.
#[tauri::command]
pub fn generate_captions(
    core: State<'_, AppCore>,
    media: State<'_, MediaState>,
    request: CaptionRequestDto,
) -> Result<GenerateCaptionsResult, String> {
    let snapshot = core.get_timeline();
    let timeline = snapshot.timeline;
    let manifest = core.media();
    let fps = timeline.fps;

    // Style + placement (defaults: 48-pt caption near the bottom, white).
    let mut style = request.style.unwrap_or_else(|| TextStyle {
        font_size: DEFAULT_FONT_SIZE,
        ..TextStyle::default()
    });
    if style.font_size <= 0.0 {
        style.font_size = DEFAULT_FONT_SIZE;
    }
    let center_x = request.center_x.unwrap_or(DEFAULT_CENTER_X);
    let center_y = request.center_y.unwrap_or(DEFAULT_CENTER_Y);
    let case: CaptionCase = request.text_case.into();

    // Resolve the requested language against the backend's supported set.
    let language = match request.language.as_deref() {
        None => None,
        Some(lang) => Some(opentake_media::match_language(lang).ok_or_else(|| {
            format!("on-device transcription does not support language '{lang}'.")
        })?),
    };

    // Caption-eligible clips for the chosen source (each with its track id).
    let auto_detect = matches!(request.source, CaptionSource::Auto);
    let eligible = eligible_targets(&timeline, &manifest, &request.source);
    if eligible.is_empty() {
        return Ok(GenerateCaptionsResult {
            edit: unchanged_edit(&snapshot.version),
            caption_count: 0,
        });
    }

    // Transcribe each unique source once. Skip-don't-fail per source (a missing
    // file / decode error / model-not-installed skips just that clip); if EVERY
    // source failed with the same reason, surface it (so "model not installed"
    // reaches the UI instead of a silent empty result).
    //
    // A language hint OR profanity masking makes the transcript differ from the
    // shared auto-detect cache, so those variants transcribe directly with the
    // options threaded to the backend (upstream bypasses the cache for option
    // variants, `EditorViewModel+Captions.swift:127`). The plain case uses the
    // caching convenience so repeats are instant. `censor_profanity` is honored
    // here so it takes effect if/when the whisper backend gains masking (today it
    // is a no-op in the backend, matching upstream's transcription-level boundary).
    let uses_options = language.is_some() || request.censor_profanity;
    let mut transcripts: std::collections::HashMap<String, TranscriptionResult> =
        std::collections::HashMap::new();
    let mut first_error: Option<String> = None;
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for t in &eligible {
        if !seen.insert(t.media_ref.clone()) {
            continue;
        }
        let (path, is_video) = match crate::transcribe::resolve_asset(&core, &t.media_ref) {
            Ok(pair) => pair,
            Err(e) => {
                first_error = first_error.or(Some(e));
                continue;
            }
        };
        let result = if uses_options {
            crate::transcribe::load_backend(media.engine()).and_then(|backend| {
                let opts = opentake_media::TranscribeOptions {
                    preferred_language: language.clone(),
                    censor_profanity: request.censor_profanity,
                    ..Default::default()
                };
                opentake_media::transcribe::transcribe_file(&path, &backend, &opts)
                    .map_err(|e| e.to_string())
            })
        } else {
            crate::transcribe::transcribe_with_cache(media.engine(), &path, is_video, None)
        };
        match result {
            Ok(r) => {
                transcripts.insert(t.media_ref.clone(), r);
            }
            Err(e) => first_error = first_error.or(Some(e)),
        }
    }
    if transcripts.is_empty() {
        if let Some(e) = first_error {
            return Err(e);
        }
        return Ok(GenerateCaptionsResult {
            edit: unchanged_edit(&snapshot.version),
            caption_count: 0,
        });
    }

    // Build caption targets (clip + track id + resolved transcript).
    let targets: Vec<CaptionTarget<'_>> = eligible
        .iter()
        .map(|t| CaptionTarget {
            clip_id: t.clip.id.clone(),
            track_id: t.track_id.clone(),
            clip: t.clip,
            transcript: transcripts.get(&t.media_ref),
        })
        .collect();

    // Auto source: keep only the dominant spoken track.
    let targets: Vec<CaptionTarget<'_>> = if auto_detect {
        match dominant_speech_track(&targets, fps) {
            Some(winner) => targets
                .into_iter()
                .filter(|t| t.track_id == winner)
                .collect(),
            None => {
                return Ok(GenerateCaptionsResult {
                    edit: unchanged_edit(&snapshot.version),
                    caption_count: 0,
                })
            }
        }
    } else {
        targets
    };

    // Build specs via the pure builder. `fits` + the per-line transform use this
    // timeline's canvas (upstream `captionLineFits` / `captionTransform`).
    let group_id = new_caption_group_id();
    let canvas_w = timeline.width.max(1) as f64;
    let canvas_h = timeline.height.max(1) as f64;
    let max_text_w = canvas_w * MAX_TEXT_WIDTH_RATIO;
    let fits = |line: &str| {
        let (w, _) = TextLayout::natural_size(line, &style, f64::MAX, canvas_h);
        w <= max_text_w
    };
    let specs = caption_specs(&targets, fps, case, &group_id, &fits);
    if specs.is_empty() {
        return Ok(GenerateCaptionsResult {
            edit: unchanged_edit(&snapshot.version),
            caption_count: 0,
        });
    }

    let entries: Vec<CaptionEntry> = specs
        .into_iter()
        .map(|s| {
            let (w, h) = TextLayout::natural_size(&s.content, &style, max_text_w, canvas_h);
            let transform = Transform {
                center_x,
                center_y,
                width: w / canvas_w,
                height: h / canvas_h,
                ..Transform::default()
            };
            CaptionEntry {
                start_frame: s.start_frame,
                duration_frames: s.duration_frames,
                content: s.content,
                text_style: style.clone(),
                transform,
                caption_group_id: s.caption_group_id,
            }
        })
        .collect();

    let count = entries.len();
    // Place atomically through the core (snapshot/commit/version + TimelineChanged).
    let edit =
        handle_edit_apply(&core, EditCommand::AddCaptions { entries }).map_err(|e| e.message)?;
    Ok(GenerateCaptionsResult {
        edit,
        caption_count: count,
    })
}

/// One caption-eligible clip located on the timeline: the clip + its track id +
/// its source `media_ref`.
struct EligibleTarget<'a> {
    clip: &'a Clip,
    track_id: String,
    media_ref: String,
}

/// Caption-eligible clips for the chosen [`CaptionSource`], mirroring upstream
/// `captionTargets(in:)` (`EditorViewModel+Captions.swift:80-89`): keep
/// audio/video clips whose asset can be transcribed, but drop a **video** clip
/// whose link group also has a linked **audio** clip (that audio partner is
/// transcribed instead). `Track` restricts to one track; `Clips` to a selection.
fn eligible_targets<'a>(
    timeline: &'a opentake_domain::Timeline,
    manifest: &MediaManifest,
    source: &CaptionSource,
) -> Vec<EligibleTarget<'a>> {
    // Link groups that contain at least one audio clip anywhere.
    let audio_link_groups: std::collections::BTreeSet<&str> = timeline
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .filter(|c| c.media_type == ClipType::Audio)
        .filter_map(|c| c.link_group_id.as_deref())
        .collect();

    let want_track: Option<&str> = match source {
        CaptionSource::Track { track_id } => Some(track_id.as_str()),
        _ => None,
    };
    let want_clips: Option<std::collections::BTreeSet<&str>> = match source {
        CaptionSource::Clips { clip_ids } => Some(clip_ids.iter().map(String::as_str).collect()),
        _ => None,
    };

    let mut out = Vec::new();
    for track in &timeline.tracks {
        if let Some(tid) = want_track {
            if track.id != tid {
                continue;
            }
        }
        for clip in &track.clips {
            if let Some(clips) = &want_clips {
                if !clips.contains(clip.id.as_str()) {
                    continue;
                }
            }
            if !can_transcribe(clip, manifest) {
                continue;
            }
            if clip.media_type == ClipType::Video {
                if let Some(gid) = clip.link_group_id.as_deref() {
                    if audio_link_groups.contains(gid) {
                        continue;
                    }
                }
            }
            out.push(EligibleTarget {
                clip,
                track_id: track.id.clone(),
                media_ref: clip.media_ref.clone(),
            });
        }
    }
    out.sort_by_key(|t| t.clip.start_frame);
    out
}

/// Whether a clip can be transcribed, mirroring upstream `captionCanTranscribe`:
/// media type must be video/audio, and (when the asset is known) it must be audio
/// or a video WITH an audio track. Unknown assets are permissively eligible.
fn can_transcribe(clip: &Clip, manifest: &MediaManifest) -> bool {
    if !matches!(clip.media_type, ClipType::Video | ClipType::Audio) {
        return false;
    }
    match manifest.entries.iter().find(|e| e.id == clip.media_ref) {
        None => true,
        Some(entry) => {
            entry.kind == ClipType::Audio
                || (entry.kind == ClipType::Video && entry.has_audio.unwrap_or(false))
        }
    }
}

/// The "nothing changed" edit result (no caption track created). Mirrors the
/// shape of an `EditResult` for a no-op so the UI's version stays put.
fn unchanged_edit(version: &u64) -> EditResultDto {
    EditResultDto {
        changed: false,
        action_name: "Generate Captions".into(),
        affected_clip_ids: Vec::new(),
        timeline_version: *version,
        summary: String::new(),
    }
}

/// Mint a fresh caption-group id (upstream `UUID().uuidString`) without a uuid
/// dependency: a process-wide counter plus a nanosecond timestamp. Opaque; only
/// used for group membership (subtitle export + caption-group style sync).
fn new_caption_group_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("cap-{nanos:x}-{n:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentake_domain::{MediaManifestEntry, MediaSource, Timeline, Track};

    fn entry(id: &str, kind: ClipType, has_audio: bool) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.into(),
            name: id.into(),
            kind,
            source: MediaSource::External {
                absolute_path: format!("/{id}"),
            },
            duration: 1.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: Some(has_audio),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
        }
    }

    #[test]
    fn request_dto_deserializes_camelcase() {
        // The Captions tab sends camelCase; every multi-word field must decode.
        let req: CaptionRequestDto = serde_json::from_str(
            r#"{"source":{"kind":"clips","clipIds":["c1","c2"]},
                "centerX":0.5,"centerY":0.9,"textCase":"upper",
                "censorProfanity":true,"language":"es"}"#,
        )
        .expect("camelCase request");
        assert_eq!(
            req.source,
            CaptionSource::Clips {
                clip_ids: vec!["c1".into(), "c2".into()]
            }
        );
        assert_eq!(req.center_y, Some(0.9));
        assert_eq!(req.text_case, CaptionCaseDto::Upper);
        assert!(req.censor_profanity);
        assert_eq!(req.language.as_deref(), Some("es"));
    }

    #[test]
    fn request_dto_defaults_to_auto_source() {
        let req: CaptionRequestDto = serde_json::from_str("{}").expect("empty request");
        assert_eq!(req.source, CaptionSource::Auto);
        assert_eq!(req.text_case, CaptionCaseDto::Auto);
        assert!(!req.censor_profanity);
    }

    #[test]
    fn result_serializes_camelcase() {
        let r = GenerateCaptionsResult {
            edit: unchanged_edit(&3),
            caption_count: 2,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"captionCount\":2"));
        assert!(json.contains("\"timelineVersion\":3"));
    }

    fn tl_with_audio() -> Timeline {
        let mut tl = Timeline::new();
        let mut vt = Track::new("v", ClipType::Video);
        // A silent video clip (has_audio=false asset) — not eligible.
        vt.clips.push(Clip::new("v-silent", "vid", 0, 60));
        tl.tracks.push(vt);
        let mut at = Track::new("a", ClipType::Audio);
        let mut ac = Clip::new("a1", "aud", 0, 60);
        ac.media_type = ClipType::Audio;
        at.clips.push(ac);
        tl.tracks.push(at);
        tl
    }

    fn manifest_with_audio() -> MediaManifest {
        let mut m = MediaManifest::new();
        m.entries.push(entry("vid", ClipType::Video, false));
        m.entries.push(entry("aud", ClipType::Audio, true));
        m
    }

    #[test]
    fn eligible_auto_keeps_audio_drops_silent_video() {
        let tl = tl_with_audio();
        let m = manifest_with_audio();
        let targets = eligible_targets(&tl, &m, &CaptionSource::Auto);
        let ids: Vec<&str> = targets.iter().map(|t| t.clip.id.as_str()).collect();
        assert_eq!(ids, vec!["a1"]);
        assert_eq!(targets[0].track_id, "a");
    }

    #[test]
    fn eligible_track_scopes_to_one_track() {
        let tl = tl_with_audio();
        let m = manifest_with_audio();
        let targets = eligible_targets(
            &tl,
            &m,
            &CaptionSource::Track {
                track_id: "a".into(),
            },
        );
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].clip.id, "a1");
        // The (silent) video track is excluded by the track filter.
        let none = eligible_targets(
            &tl,
            &m,
            &CaptionSource::Track {
                track_id: "v".into(),
            },
        );
        assert!(none.is_empty());
    }

    #[test]
    fn eligible_clips_scopes_to_selection() {
        let tl = tl_with_audio();
        let m = manifest_with_audio();
        let targets = eligible_targets(
            &tl,
            &m,
            &CaptionSource::Clips {
                clip_ids: vec!["a1".into()],
            },
        );
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].clip.id, "a1");
    }

    #[test]
    fn eligible_drops_video_with_linked_audio() {
        let mut tl = Timeline::new();
        let mut vt = Track::new("v", ClipType::Video);
        let mut vc = Clip::new("v1", "vid_a", 0, 60);
        vc.link_group_id = Some("grp".into());
        vt.clips.push(vc);
        tl.tracks.push(vt);
        let mut at = Track::new("a", ClipType::Audio);
        let mut ac = Clip::new("a1", "aud", 0, 60);
        ac.media_type = ClipType::Audio;
        ac.link_group_id = Some("grp".into());
        at.clips.push(ac);
        tl.tracks.push(at);
        let mut m = MediaManifest::new();
        m.entries.push(entry("vid_a", ClipType::Video, true));
        m.entries.push(entry("aud", ClipType::Audio, true));
        let targets = eligible_targets(&tl, &m, &CaptionSource::Auto);
        let ids: Vec<&str> = targets.iter().map(|t| t.clip.id.as_str()).collect();
        assert!(!ids.contains(&"v1"), "linked video should be dropped");
        assert!(ids.contains(&"a1"));
    }
}
