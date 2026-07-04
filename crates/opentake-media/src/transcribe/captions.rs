//! Pure caption **building** — the heart of the Captions tab. Verbatim port of
//! `MediaPanel/CaptionsTab/CaptionBuilder.swift` plus the caption-spec
//! orchestration in `Editor/ViewModel/EditorViewModel+Captions.swift`
//! (`captionSpecs` / `bestClip` / `dominantSpeechTrack`).
//!
//! The flow, per upstream:
//!   1. Each [`TranscriptionSegment`] is split into screen-ready **phrases** on
//!      the best available boundary (sentence → clause → mid-word), each phrase
//!      packed so it *fits* a caller-supplied width predicate ([`phrases`],
//!      port of `CaptionBuilder.phrases`).
//!   2. The segment's time span is shared across its phrases by character count,
//!      back-to-back ([`distribute`]); each phrase is then given a floor display
//!      duration, shifting later phrases so they never overlap
//!      ([`enforce_min_duration`], port of `enforceMinDuration`).
//!   3. Each phrase is attributed to the timeline clip whose visible source
//!      window overlaps it most ([`best_clip`], port of `bestClip`), cased
//!      (auto/upper/lower), then mapped to PROJECT frames through that clip's
//!      trim/speed/placement ([`specs`], port of `CaptionBuilder.specs`), reusing
//!      the same `Clip::timeline_frame` mapping the live-transcript path uses.
//!
//! **Everything here is pure.** Text measurement (whether a line fits, and a
//! phrase's natural box for the caption transform) is a CoreText/cosmic-text
//! concern that lives in the render/UI layer, so it is injected as two closures
//! (`fits` and `transform_for`). Transcription (whisper + cache) is likewise
//! injected as resolved [`TranscriptionResult`]s per source. This mirrors how
//! `timeline.rs` keeps the word→frame mapping pure while the caller supplies the
//! transcripts.
//!
//! **Profanity note:** upstream's `censorProfanity` is a *transcription* option
//! (Apple `.etiquetteReplacements`); `CaptionBuilder` never masks text itself.
//! So this module has no masking pass either — masking, when enabled, happens in
//! the backend transcript the caller passes in (`TranscribeOptions.censor_profanity`),
//! keeping the 1:1 boundary. See `EditorViewModel+Captions.swift:127-134`.
//!
//! **Constants** (`UI/AppTheme.swift` `Caption` enum, quoted at their use sites):
//!   * `minDisplayDuration = 0.7` s — the per-phrase floor.
//!   * `defaultFontSize = 48`, `defaultCenter = (0.5, 0.9)` — style/placement
//!     defaults, owned by the caller (the tab / tool), not this module.
//!   * `captionPreviewMaxTextWidthRatio = 0.9` — the fraction of canvas width a
//!     line may occupy before it must wrap; used by the caller's `fits`/transform.

use opentake_domain::Clip;

use super::{TranscriptionResult, TranscriptionSegment};

/// Per-phrase floor display duration, in **seconds**. 1:1 with upstream
/// `AppTheme.Caption.minDisplayDuration = 0.7` (`AppTheme.swift:249`), the
/// `minDuration` passed into `CaptionBuilder.phrases`
/// (`EditorViewModel+Captions.swift:170`).
pub const MIN_DISPLAY_DURATION_SECS: f64 = 0.7;

/// Letter-case transform applied to each phrase before placement. 1:1 port of
/// `EditorViewModel.CaptionCase` (`EditorViewModel+Captions.swift:15-33`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CaptionCase {
    /// Leave the transcript's own casing.
    #[default]
    Auto,
    /// Force UPPERCASE.
    Upper,
    /// Force lowercase.
    Lower,
}

impl CaptionCase {
    /// Apply the case to a line (`auto` is the identity). Port of `apply(_:)`.
    pub fn apply(self, s: &str) -> String {
        match self {
            CaptionCase::Auto => s.to_string(),
            CaptionCase::Upper => s.to_uppercase(),
            CaptionCase::Lower => s.to_lowercase(),
        }
    }

    /// Parse the wire value (`"auto"`/`"upper"`/`"lower"`), matching upstream's
    /// `CaptionCase(rawValue:)` used by the `add_captions` tool and the tab.
    /// Named `parse` (not `from_str`) to avoid the `FromStr` trait confusion.
    pub fn parse(raw: &str) -> Option<CaptionCase> {
        match raw {
            "auto" => Some(CaptionCase::Auto),
            "upper" => Some(CaptionCase::Upper),
            "lower" => Some(CaptionCase::Lower),
            _ => None,
        }
    }
}

/// One timed, screen-ready caption phrase in **source seconds**. Port of
/// `CaptionBuilder.Phrase` (`CaptionBuilder.swift:4-8`).
#[derive(Clone, Debug, PartialEq)]
pub struct Phrase {
    /// The phrase text (already packed to fit; not yet cased).
    pub text: String,
    /// Start time in source seconds.
    pub start: f64,
    /// End time in source seconds (`>= start`).
    pub end: f64,
}

/// One built caption clip: a text clip spec in **project frames**, ready for the
/// command layer to place on a fresh caption track. Mirrors upstream
/// `EditorViewModel.TextClipSpec` for the caption path — plus the
/// `caption_group_id` every caption clip carries (so subtitle export and
/// caption-group style sync recognize it).
#[derive(Clone, Debug, PartialEq)]
pub struct CaptionClipSpec {
    /// The (final, cased) caption text.
    pub content: String,
    /// Clip start on the timeline, in project frames (inclusive).
    pub start_frame: i32,
    /// Clip length in frames (`>= 1`).
    pub duration_frames: i32,
    /// The shared caption-group id all clips from one Generate share.
    pub caption_group_id: String,
}

// MARK: - Phrase building (CaptionBuilder.swift)

/// Split a transcript `segment` into screen-ready [`Phrase`]s and time them.
/// Verbatim port of `CaptionBuilder.phrases(for:fits:minDuration:)`
/// (`CaptionBuilder.swift:11-19`).
///
/// `fits(line)` returns whether `line` fits on screen at the chosen style — a
/// caller-injected text-measurement predicate (CoreText/cosmic-text), kept out
/// of this pure module. `min_duration` is the per-phrase floor in seconds
/// (upstream passes [`MIN_DISPLAY_DURATION_SECS`]).
pub fn phrases<F: Fn(&str) -> bool>(
    segment: &TranscriptionSegment,
    fits: &F,
    min_duration: f64,
) -> Vec<Phrase> {
    let pieces = split(&segment.text, fits);
    let timed = distribute(&pieces, segment.start, segment.end);
    enforce_min_duration(timed, min_duration)
}

/// Recursively break `text` until every piece `fits`. A single over-long word
/// that can't be broken is kept whole. Port of `split(_:fits:)`
/// (`CaptionBuilder.swift:21-28`).
fn split<F: Fn(&str) -> bool>(text: &str, fits: &F) -> Vec<String> {
    let t = text.trim();
    if t.is_empty() {
        return Vec::new();
    }
    if fits(t) {
        return vec![t.to_string()];
    }
    let parts = break_once(t);
    if parts.len() <= 1 {
        // A single over-long word: keep it (matches upstream's guard).
        return vec![t.to_string()];
    }
    parts.iter().flat_map(|p| split(p, fits)).collect()
}

/// Break once at the best boundary present: sentence (`.!?`), then clause
/// (`,;:`), then the midpoint word. Port of `breakOnce(_:)`
/// (`CaptionBuilder.swift:31-33`).
fn break_once(text: &str) -> Vec<String> {
    break_on(text, ".!?")
        .or_else(|| break_on(text, ",;:"))
        .unwrap_or_else(|| break_at_mid_word(text))
}

/// Split after any delimiter that is followed by a space (or end of string), so
/// `"U.S."` and `"3.14"` stay intact. Returns `None` when it produced only one
/// piece. Verbatim port of `breakOn(_:delimiters:)` (`CaptionBuilder.swift:36-53`).
fn break_on(text: &str, delimiters: &str) -> Option<Vec<String>> {
    let chars: Vec<char> = text.chars().collect();
    let mut pieces: Vec<String> = Vec::new();
    let mut current = String::new();
    for (i, c) in chars.iter().enumerate() {
        current.push(*c);
        let next_is_break = i + 1 >= chars.len() || chars[i + 1] == ' ';
        if delimiters.contains(*c) && next_is_break {
            let piece = current.trim();
            if !piece.is_empty() {
                pieces.push(piece.to_string());
            }
            current.clear();
        }
    }
    let tail = current.trim();
    if !tail.is_empty() {
        pieces.push(tail.to_string());
    }
    if pieces.len() > 1 {
        Some(pieces)
    } else {
        None
    }
}

/// Break at the midpoint word boundary. A single word (no spaces) is returned
/// unchanged. Port of `breakAtMidWord(_:)` (`CaptionBuilder.swift:55-60`).
fn break_at_mid_word(text: &str) -> Vec<String> {
    let words: Vec<&str> = text.split(' ').filter(|w| !w.is_empty()).collect();
    if words.len() <= 1 {
        return vec![text.to_string()];
    }
    let mid = words.len() / 2;
    vec![words[..mid].join(" "), words[mid..].join(" ")]
}

/// Share `[start, end]` across `texts` by character count, back-to-back. Port of
/// `distribute(_:start:end:)` (`CaptionBuilder.swift:63-75`). An empty input
/// yields no phrases; each piece counts at least one char so an all-empty set
/// still divides evenly.
fn distribute(texts: &[String], start: f64, end: f64) -> Vec<Phrase> {
    if texts.is_empty() {
        return Vec::new();
    }
    let total: f64 = texts.iter().map(|t| t.chars().count().max(1) as f64).sum();
    let span = (end - start).max(0.0);
    let mut phrases = Vec::with_capacity(texts.len());
    let mut t = start;
    for text in texts {
        let dur = span * (text.chars().count().max(1) as f64) / total;
        phrases.push(Phrase {
            text: text.clone(),
            start: t,
            end: t + dur,
        });
        t += dur;
    }
    phrases
}

/// Give each phrase a floor duration, shifting later ones so they don't overlap.
/// Verbatim port of `enforceMinDuration(_:minDuration:)`
/// (`CaptionBuilder.swift:78-91`).
fn enforce_min_duration(mut phrases: Vec<Phrase>, min_duration: f64) -> Vec<Phrase> {
    for i in 0..phrases.len() {
        if phrases[i].end - phrases[i].start < min_duration {
            phrases[i].end = phrases[i].start + min_duration;
        }
        if i + 1 < phrases.len() && phrases[i + 1].start < phrases[i].end {
            let shift = phrases[i].end - phrases[i + 1].start;
            phrases[i + 1].start += shift;
            phrases[i + 1].end += shift;
        }
    }
    phrases
}

// MARK: - Spec building (CaptionBuilder.specs)

/// Map cased phrases through `source_clip`'s trim/speed/placement into
/// PROJECT-frame caption clip specs. Verbatim port of
/// `CaptionBuilder.specs(...)` (`CaptionBuilder.swift:93-124`).
///
/// A phrase whose source range doesn't intersect the clip's visible window is
/// dropped. Each clip is clamped so it stays inside the owner clip's timeline
/// span, and given at least `min_duration_frames` (upstream default 1).
fn specs(
    cased: &[Phrase],
    source_clip: &Clip,
    fps: i32,
    caption_group_id: &str,
    min_duration_frames: i32,
) -> Vec<CaptionClipSpec> {
    let fps_d = fps as f64;
    let visible_start_source = source_clip.trim_start_frame as f64;
    let visible_end_source = visible_start_source
        + source_clip.duration_frames as f64 * source_clip.speed.max(SPEED_FLOOR);

    let mut out = Vec::new();
    for p in cased {
        let phrase_start_source = p.start * fps_d;
        let phrase_end_source = p.end * fps_d;
        // Skip phrases that fall entirely outside the clip's visible window.
        if phrase_end_source <= visible_start_source || phrase_start_source >= visible_end_source {
            continue;
        }
        let s = source_clip
            .timeline_frame(p.start, fps)
            .unwrap_or(source_clip.start_frame);
        let e = source_clip
            .timeline_frame(p.end, fps)
            .unwrap_or_else(|| source_clip.end_frame());
        // duration = clamp(e,end) - clamp(s,start), floored at min_duration_frames.
        let clamped_end = source_clip.end_frame().min(e);
        let clamped_start = source_clip.start_frame.max(s);
        let duration = (clamped_end - clamped_start).max(min_duration_frames);
        out.push(CaptionClipSpec {
            content: p.text.clone(),
            start_frame: s,
            duration_frames: duration,
            caption_group_id: caption_group_id.to_string(),
        });
    }
    out
}

/// Lower bound on `speed` in the frame math, matching upstream `max(speed, 0.0001)`.
const SPEED_FLOOR: f64 = 0.0001;

// MARK: - Orchestration (EditorViewModel+Captions.swift)

/// One caption target: a timeline clip plus its resolved source transcript.
/// Mirrors upstream `CaptionTarget` (`EditorViewModel+Captions.swift:91-95`)
/// joined to its transcript. The caller (the bridge / tool) has already filtered
/// to caption-eligible clips (see `caption_target_fragments`), transcribed each
/// unique source (cached), and grouped clips by track.
pub struct CaptionTarget<'a> {
    /// The clip id (echoed back in [`dominant_speech_track`]'s accounting).
    pub clip_id: String,
    /// The track id the clip lives on (drives auto-detect winner selection).
    pub track_id: String,
    /// The clip geometry (start/trim/duration/speed) for the frame mapping.
    pub clip: &'a Clip,
    /// The clip's source transcript (source-seconds timings). `None` when that
    /// source failed to transcribe — the clip contributes nothing, not an error.
    pub transcript: Option<&'a TranscriptionResult>,
}

/// Pick the track with the most spoken words across `targets`, or `None` when no
/// target has any timed words. 1:1 port of `dominantSpeechTrack`
/// (`EditorViewModel+Captions.swift:151-158`) + `spokenWordCount`
/// (`:197-205`). A word counts for a clip when its timing **midpoint** lands in
/// the clip's visible source window `[trim_start, trim_start + dur*speed)`.
///
/// Ties resolve to the *last* track visited with the max count (Swift's
/// `max(by:)` keeps the later element on `<`); iteration order follows `targets`.
pub fn dominant_speech_track(targets: &[CaptionTarget<'_>], fps: i32) -> Option<String> {
    let fps_d = fps as f64;
    // Accumulate per track in first-seen order (a Vec of (track_id, count) keeps
    // the deterministic tie behavior a hash map would lose).
    let mut counts: Vec<(String, i64)> = Vec::new();
    for t in targets {
        let Some(result) = t.transcript else { continue };
        let (vis_start, vis_end) = visible_source_span(t.clip);
        let mut spoken = 0i64;
        for w in &result.words {
            let (Some(s), Some(e)) = (w.start, w.end) else {
                continue;
            };
            let mid = (s + e) / 2.0 * fps_d;
            if vis_start <= mid && mid < vis_end {
                spoken += 1;
            }
        }
        match counts.iter_mut().find(|(id, _)| *id == t.track_id) {
            Some(entry) => entry.1 += spoken,
            None => counts.push((t.track_id.clone(), spoken)),
        }
    }
    // `wordsByTrack.filter { $0.value > 0 }.max { $0.value < $1.value }` — keep the
    // last track reaching the running max (matches Swift `max(by:)` on ties).
    let mut best: Option<(&str, i64)> = None;
    for (id, count) in &counts {
        if *count > 0 && best.is_none_or(|(_, b)| b <= *count) {
            best = Some((id.as_str(), *count));
        }
    }
    best.map(|(id, _)| id.to_string())
}

/// Build every caption clip spec for `targets`, in project frames, sharing one
/// `caption_group_id`. 1:1 port of `captionSpecs(...)`
/// (`EditorViewModel+Captions.swift:160-183`):
///
///   * Each source's segments → phrases (`phrases`, packed by `fits`).
///   * Each phrase is attributed to the clip it overlaps most ([`best_clip`]),
///     so a phrase spanning a cut is emitted once.
///   * Per clip: phrases are cased then mapped to frames ([`specs`]).
///
/// `fits(line)` and `case` come from the caller (the tab/tool's style +
/// text-measurement). The returned specs are in the same order upstream places
/// them: grouped by target clip, in the caller's `targets` order. The caller
/// mints `caption_group_id` (upstream `UUID().uuidString`).
pub fn caption_specs<F: Fn(&str) -> bool>(
    targets: &[CaptionTarget<'_>],
    fps: i32,
    case: CaptionCase,
    caption_group_id: &str,
    fits: &F,
) -> Vec<CaptionClipSpec> {
    // Group phrases by owning clip id (matches `phrasesByClip`).
    // Distinct source refs, first-seen: iterate transcripts once per source.
    let mut phrases_by_clip: Vec<(String, Vec<Phrase>)> = Vec::new();
    let mut seen_refs: Vec<&str> = Vec::new();
    for t in targets {
        let media_ref = t.clip.media_ref.as_str();
        if seen_refs.contains(&media_ref) {
            continue;
        }
        seen_refs.push(media_ref);
        let Some(result) = t.transcript else { continue };
        // Clips sharing this source (upstream `targets.filter { mediaRef == ref }`).
        let clips: Vec<&CaptionTarget<'_>> = targets
            .iter()
            .filter(|c| c.clip.media_ref == media_ref)
            .collect();
        if clips.is_empty() {
            continue;
        }
        let seg_phrases: Vec<Phrase> = result
            .segments
            .iter()
            .flat_map(|seg| phrases(seg, fits, MIN_DISPLAY_DURATION_SECS))
            .collect();
        for p in seg_phrases {
            let Some(owner) = best_clip(&p, &clips, fps) else {
                continue;
            };
            match phrases_by_clip
                .iter_mut()
                .find(|(id, _)| *id == owner.clip_id)
            {
                Some(entry) => entry.1.push(p),
                None => phrases_by_clip.push((owner.clip_id.clone(), vec![p])),
            }
        }
    }

    // Place per target, in `targets` order (upstream `targets.flatMap`).
    let mut out = Vec::new();
    for t in targets {
        let Some((_, clip_phrases)) = phrases_by_clip.iter().find(|(id, _)| *id == t.clip_id)
        else {
            continue;
        };
        let cased: Vec<Phrase> = clip_phrases
            .iter()
            .map(|p| Phrase {
                text: case.apply(&p.text),
                start: p.start,
                end: p.end,
            })
            .collect();
        out.extend(specs(&cased, t.clip, fps, caption_group_id, 1));
    }
    out
}

/// The clip whose visible source window overlaps phrase `p` the most, but only
/// when the overlap is real (`> 0`) and covers at least half the phrase. 1:1 port
/// of `bestClip(for:among:)` (`EditorViewModel+Captions.swift:186-195`).
fn best_clip<'a>(
    p: &Phrase,
    clips: &[&'a CaptionTarget<'a>],
    fps: i32,
) -> Option<&'a CaptionTarget<'a>> {
    let fps_d = fps as f64;
    let ps = p.start * fps_d;
    let pe = p.end * fps_d;
    let overlap = |c: &Clip| -> f64 {
        let (vs, ve) = visible_source_span(c);
        (pe.min(ve) - ps.max(vs)).max(0.0)
    };
    // `clips.max(by: { overlap($0) < overlap($1) })` — last max on ties.
    let mut best: Option<&&CaptionTarget<'_>> = None;
    for c in clips {
        match best {
            Some(b) if overlap(b.clip) > overlap(c.clip) => {}
            _ => best = Some(c),
        }
    }
    let best = best?;
    let o = overlap(best.clip);
    if o > 0.0 && o >= (pe - ps) / 2.0 {
        Some(best)
    } else {
        None
    }
}

/// A clip's visible source-frame window `[trim_start, trim_start + dur*speed)`.
/// Port of the inlined `visibleSource(_:)` (`EditorViewModel+Captions.swift:207-210`).
fn visible_source_span(clip: &Clip) -> (f64, f64) {
    let start = clip.trim_start_frame as f64;
    (
        start,
        start + clip.duration_frames as f64 * clip.speed.max(SPEED_FLOOR),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcribe::{TranscriptionResult, TranscriptionWord};

    /// A word-count-based fits predicate — a line "fits" when it has at most
    /// `max_words` whitespace-separated words. Lets the packing tests be
    /// deterministic without a real text engine (mirrors what the width
    /// predicate does, just on word count).
    fn fits_words(max_words: usize) -> impl Fn(&str) -> bool {
        move |line: &str| line.split_whitespace().count() <= max_words
    }

    /// A fits predicate keyed on character length (for punctuation-boundary tests).
    fn fits_chars(max: usize) -> impl Fn(&str) -> bool {
        move |line: &str| line.chars().count() <= max
    }

    fn seg(text: &str, start: f64, end: f64) -> TranscriptionSegment {
        TranscriptionSegment {
            text: text.into(),
            start,
            end,
        }
    }

    fn clip(id: &str, start: i32, duration: i32, trim_start: i32, speed: f64) -> Clip {
        let mut c = Clip::new(id, "media", start, duration);
        c.trim_start_frame = trim_start;
        c.speed = speed;
        c
    }

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    // --- CaptionCase --------------------------------------------------------

    #[test]
    fn caption_case_apply_and_parse() {
        assert_eq!(CaptionCase::Auto.apply("Hello"), "Hello");
        assert_eq!(CaptionCase::Upper.apply("Hello"), "HELLO");
        assert_eq!(CaptionCase::Lower.apply("Hello"), "hello");
        assert_eq!(CaptionCase::parse("upper"), Some(CaptionCase::Upper));
        assert_eq!(CaptionCase::parse("nope"), None);
    }

    // --- split / break boundaries ------------------------------------------

    #[test]
    fn fitting_line_is_kept_whole() {
        // Fits (<=5 words) → single phrase spanning the segment.
        let s = seg("a short line here", 0.0, 2.0);
        let out = phrases(&s, &fits_words(5), MIN_DISPLAY_DURATION_SECS);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "a short line here");
        approx(out[0].start, 0.0);
        approx(out[0].end, 2.0);
    }

    #[test]
    fn breaks_on_sentence_boundary_first() {
        // Two sentences; each fits once split. Break must land on ". ".
        let s = seg("First one. Second two.", 0.0, 10.0);
        let out = phrases(&s, &fits_words(2), MIN_DISPLAY_DURATION_SECS);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].text, "First one.");
        assert_eq!(out[1].text, "Second two.");
    }

    #[test]
    fn abbreviation_period_is_not_a_break() {
        // "U.S." has no space after the internal dots, so it stays intact; the
        // sentence break is the final period (end of string). One phrase.
        let s = seg("the U.S. economy", 0.0, 3.0);
        let out = phrases(&s, &fits_words(5), MIN_DISPLAY_DURATION_SECS);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "the U.S. economy");
    }

    #[test]
    fn decimal_number_stays_intact() {
        let s = seg("pi is 3.14 today", 0.0, 3.0);
        // Force wrapping by char budget; the decimal must not split at "3.".
        let out = phrases(&s, &fits_chars(10), MIN_DISPLAY_DURATION_SECS);
        // Every emitted piece keeps "3.14" whole (never a lone "3." or ".14").
        assert!(out.iter().all(|p| !p.text.ends_with("3.")));
        assert!(out.iter().any(|p| p.text.contains("3.14")));
    }

    #[test]
    fn falls_back_to_clause_then_midword() {
        // No sentence punctuation; a comma clause break is used.
        let s = seg("apples, oranges and pears", 0.0, 4.0);
        let out = phrases(&s, &fits_words(2), MIN_DISPLAY_DURATION_SECS);
        assert_eq!(out[0].text, "apples,");
        // "oranges and pears" is 3 words > 2 → mid-word split (no punctuation).
        assert!(out.len() >= 2);
    }

    #[test]
    fn single_overlong_word_is_kept() {
        // One token that can't be broken and doesn't fit: kept as-is (no crash,
        // no infinite recursion) — the upstream `parts.count > 1` guard.
        let s = seg("supercalifragilisticexpialidocious", 0.0, 1.0);
        let out = phrases(&s, &fits_chars(5), MIN_DISPLAY_DURATION_SECS);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "supercalifragilisticexpialidocious");
    }

    #[test]
    fn empty_segment_yields_no_phrases() {
        let s = seg("   ", 0.0, 2.0);
        assert!(phrases(&s, &fits_words(5), MIN_DISPLAY_DURATION_SECS).is_empty());
    }

    // --- distribute (time sharing) -----------------------------------------

    #[test]
    fn time_is_shared_by_char_count_back_to_back() {
        // "aa" (2) then "bbbb" (4): total 6 chars over a 6s span (min-dur 0 so
        // the raw distribution is observable). "aa" gets 2s, "bbbb" 4s.
        let parts = vec!["aa".to_string(), "bbbb".to_string()];
        let out = enforce_min_duration(distribute(&parts, 0.0, 6.0), 0.0);
        approx(out[0].start, 0.0);
        approx(out[0].end, 2.0);
        approx(out[1].start, 2.0);
        approx(out[1].end, 6.0);
    }

    #[test]
    fn distribute_zero_span_gives_zero_length_phrases() {
        let parts = vec!["a".to_string(), "b".to_string()];
        let out = distribute(&parts, 5.0, 5.0);
        approx(out[0].start, 5.0);
        approx(out[0].end, 5.0);
        approx(out[1].start, 5.0);
    }

    // --- enforce_min_duration ----------------------------------------------

    #[test]
    fn min_duration_floors_and_shifts_followers() {
        // Two 0.2s phrases back to back; floor 0.7 pushes the second forward so
        // they never overlap. Verbatim behavior of enforceMinDuration.
        let raw = vec![
            Phrase {
                text: "a".into(),
                start: 0.0,
                end: 0.2,
            },
            Phrase {
                text: "b".into(),
                start: 0.2,
                end: 0.4,
            },
        ];
        let out = enforce_min_duration(raw, 0.7);
        approx(out[0].start, 0.0);
        approx(out[0].end, 0.7);
        // second shifted by (0.7 - 0.2) = 0.5 → [0.7, 0.9], then floored? Its
        // length is 0.2 < 0.7 so it is floored to 0.7 as well BEFORE the shift of
        // the (non-existent) next. Upstream order: clamp i, then shift i+1.
        approx(out[1].start, 0.7);
        // i=1: its own floor already applied in its own iteration → end = start+0.7
        approx(out[1].end, 1.4);
    }

    #[test]
    fn min_duration_leaves_long_phrases_untouched() {
        let raw = vec![
            Phrase {
                text: "a".into(),
                start: 0.0,
                end: 2.0,
            },
            Phrase {
                text: "b".into(),
                start: 2.0,
                end: 4.0,
            },
        ];
        let out = enforce_min_duration(raw, 0.7);
        approx(out[0].end, 2.0);
        approx(out[1].start, 2.0);
        approx(out[1].end, 4.0);
    }

    // --- specs (phrase -> project frames) ----------------------------------

    #[test]
    fn specs_map_identity_clip_to_frames() {
        // clip at frame 0, no trim, speed 1, 30 fps. Phrase 0..1s → start 0,
        // end frame 30 → duration 30.
        let c = clip("c", 0, 300, 0, 1.0);
        let cased = vec![Phrase {
            text: "hi".into(),
            start: 0.0,
            end: 1.0,
        }];
        let out = specs(&cased, &c, 30, "g1", 1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].content, "hi");
        assert_eq!(out[0].start_frame, 0);
        assert_eq!(out[0].duration_frames, 30);
        assert_eq!(out[0].caption_group_id, "g1");
    }

    #[test]
    fn specs_offset_by_clip_start_and_trim() {
        // clip starts at timeline 100, trims 30 source frames (=1.0s). A phrase
        // at 1.0..1.5s maps to timeline 100..115 → start 100, duration 15.
        let c = clip("c", 100, 300, 30, 1.0);
        let cased = vec![Phrase {
            text: "x".into(),
            start: 1.0,
            end: 1.5,
        }];
        let out = specs(&cased, &c, 30, "g", 1);
        assert_eq!(out[0].start_frame, 100);
        assert_eq!(out[0].duration_frames, 15);
    }

    #[test]
    fn specs_drop_phrase_outside_visible_window() {
        // trim 30 → visible source starts at 1.0s. A phrase entirely at 0..0.5s
        // is dropped (upstream `phraseEndSource > visibleStartSource` guard).
        let c = clip("c", 0, 300, 30, 1.0);
        let cased = vec![Phrase {
            text: "gone".into(),
            start: 0.0,
            end: 0.5,
        }];
        assert!(specs(&cased, &c, 30, "g", 1).is_empty());
    }

    #[test]
    fn specs_clamp_duration_to_clip_and_floor() {
        // A phrase that runs past the clip end is clamped to the clip's end, with
        // a floor of min_duration_frames. Clip [0,30) at 30fps; phrase 0.9..5.0s.
        let c = clip("c", 0, 30, 0, 1.0);
        let cased = vec![Phrase {
            text: "long".into(),
            start: 0.9,
            end: 5.0,
        }];
        let out = specs(&cased, &c, 30, "g", 1);
        assert_eq!(out.len(), 1);
        // start maps to 27; end clamps to clip end 30 → duration 3.
        assert_eq!(out[0].start_frame, 27);
        assert_eq!(out[0].duration_frames, 3);
    }

    #[test]
    fn specs_speed_compresses_span() {
        // speed 2 → a 1s (30-frame) source span occupies 15 timeline frames.
        let c = clip("c", 0, 300, 0, 2.0);
        let cased = vec![Phrase {
            text: "s".into(),
            start: 1.0,
            end: 2.0,
        }];
        let out = specs(&cased, &c, 30, "g", 1);
        assert_eq!(out[0].start_frame, 15);
        assert_eq!(out[0].duration_frames, 15);
    }

    // --- caption_specs orchestration ---------------------------------------

    fn result(
        words: Vec<TranscriptionWord>,
        segments: Vec<TranscriptionSegment>,
    ) -> TranscriptionResult {
        TranscriptionResult {
            text: String::new(),
            language: Some("en".into()),
            words,
            segments,
        }
    }

    fn word(text: &str, start: f64, end: f64) -> TranscriptionWord {
        TranscriptionWord {
            text: text.into(),
            start: Some(start),
            end: Some(end),
        }
    }

    #[test]
    fn caption_specs_builds_and_cases_clips() {
        let c = clip("c1", 0, 300, 0, 1.0);
        let t = result(
            vec![word("hello", 0.0, 0.5), word("world", 0.5, 1.0)],
            vec![seg("hello world", 0.0, 1.0)],
        );
        let targets = vec![CaptionTarget {
            clip_id: "c1".into(),
            track_id: "t1".into(),
            clip: &c,
            transcript: Some(&t),
        }];
        let out = caption_specs(&targets, 30, CaptionCase::Upper, "grp", &fits_words(5));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].content, "HELLO WORLD");
        assert_eq!(out[0].caption_group_id, "grp");
        assert_eq!(out[0].start_frame, 0);
    }

    #[test]
    fn caption_specs_empty_transcript_yields_nothing() {
        let c = clip("c1", 0, 300, 0, 1.0);
        let targets = vec![CaptionTarget {
            clip_id: "c1".into(),
            track_id: "t1".into(),
            clip: &c,
            transcript: None,
        }];
        assert!(caption_specs(&targets, 30, CaptionCase::Auto, "g", &fits_words(5)).is_empty());
    }

    #[test]
    fn caption_specs_no_overlap_prevention_across_phrases() {
        // Two sentences forced apart by the min-duration floor stay non-overlapping
        // after mapping (each maps to a distinct frame window).
        let c = clip("c1", 0, 3000, 0, 1.0);
        let t = result(
            vec![],
            vec![seg("One. Two.", 0.0, 0.4)], // 0.4s span, two phrases → floored to 0.7 each
        );
        let targets = vec![CaptionTarget {
            clip_id: "c1".into(),
            track_id: "t1".into(),
            clip: &c,
            transcript: Some(&t),
        }];
        let out = caption_specs(&targets, 30, CaptionCase::Auto, "g", &fits_words(1));
        assert_eq!(out.len(), 2);
        // Second clip starts at/after the first clip's end (no overlap).
        let first_end = out[0].start_frame + out[0].duration_frames;
        assert!(out[1].start_frame >= first_end, "{:?}", out);
    }

    #[test]
    fn seam_phrase_attributed_to_one_clip_by_overlap() {
        // Two clips from the SAME source split at 1.0s. A phrase 0.9..1.1s overlaps
        // both but more than half sits in exactly one; it's emitted once total.
        let a = clip("A", 0, 30, 0, 1.0); // visible [0,30) source frames = [0,1)s
        let b = clip("B", 30, 30, 30, 1.0); // visible [30,60) = [1,2)s
                                            // Both targets carry the same source transcript (upstream dedups by ref).
        let t = result(vec![], vec![seg("seam", 0.9, 1.5)]);
        let targets = vec![
            CaptionTarget {
                clip_id: "A".into(),
                track_id: "t".into(),
                clip: &a,
                transcript: Some(&t),
            },
            CaptionTarget {
                clip_id: "B".into(),
                track_id: "t".into(),
                clip: &b,
                transcript: Some(&t),
            },
        ];
        let out = caption_specs(&targets, 30, CaptionCase::Auto, "g", &fits_words(5));
        // The single phrase [0.9,1.5]s overlaps B for 0.5s and A for 0.1s → B owns it.
        assert_eq!(out.len(), 1);
    }

    // --- dominant_speech_track ---------------------------------------------

    #[test]
    fn dominant_track_picks_most_words() {
        let ca = clip("a", 0, 300, 0, 1.0);
        let cb = clip("b", 0, 300, 0, 1.0);
        let ta = result(vec![word("one", 0.0, 0.3)], vec![]);
        let tb = result(
            vec![
                word("a", 0.0, 0.2),
                word("b", 0.2, 0.4),
                word("c", 0.4, 0.6),
            ],
            vec![],
        );
        let targets = vec![
            CaptionTarget {
                clip_id: "a".into(),
                track_id: "TA".into(),
                clip: &ca,
                transcript: Some(&ta),
            },
            CaptionTarget {
                clip_id: "b".into(),
                track_id: "TB".into(),
                clip: &cb,
                transcript: Some(&tb),
            },
        ];
        assert_eq!(dominant_speech_track(&targets, 30).as_deref(), Some("TB"));
    }

    #[test]
    fn dominant_track_none_when_no_words() {
        let c = clip("a", 0, 300, 0, 1.0);
        let t = result(vec![], vec![]);
        let targets = vec![CaptionTarget {
            clip_id: "a".into(),
            track_id: "TA".into(),
            clip: &c,
            transcript: Some(&t),
        }];
        assert_eq!(dominant_speech_track(&targets, 30), None);
    }

    #[test]
    fn dominant_track_ignores_words_outside_visible_window() {
        // trim 60 → visible source [2.0s, ...). Words before 2.0s don't count.
        let c = clip("a", 0, 300, 60, 1.0);
        let t = result(
            vec![word("early", 0.0, 0.3), word("late", 2.1, 2.4)],
            vec![],
        );
        let targets = vec![CaptionTarget {
            clip_id: "a".into(),
            track_id: "TA".into(),
            clip: &c,
            transcript: Some(&t),
        }];
        // Only "late" counts → the track still wins (1 > 0).
        assert_eq!(dominant_speech_track(&targets, 30).as_deref(), Some("TA"));
    }
}
