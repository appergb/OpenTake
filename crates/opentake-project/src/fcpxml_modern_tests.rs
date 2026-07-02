//! Unit tests for [`crate::fcpxml_modern`], split out to keep the exporter file
//! within the project's per-file line budget. Included via
//! `#[cfg(test)] #[path = "fcpxml_modern_tests.rs"] mod tests;`, so `super::*`
//! resolves to the `fcpxml_modern` module's private items.

use super::*;
use opentake_domain::{MediaManifestEntry, MediaSource, TextStyle, Transform};

fn entry(id: &str, name: &str, kind: ClipType, duration: f64) -> MediaManifestEntry {
    MediaManifestEntry {
        id: id.into(),
        name: name.into(),
        kind,
        source: MediaSource::External {
            absolute_path: format!("/media/{name}"),
        },
        duration,
        generation_input: None,
        source_width: Some(1920),
        source_height: Some(1080),
        source_fps: Some(30.0),
        has_audio: Some(kind == ClipType::Video || kind == ClipType::Audio),
        folder_id: None,
        cached_remote_url: None,
        cached_remote_url_expires_at: None,
    }
}

fn manifest(entries: Vec<MediaManifestEntry>) -> MediaManifest {
    let mut m = MediaManifest::new();
    m.entries = entries;
    m
}

// --- time values ---

#[test]
fn time_value_reduces_rational() {
    assert_eq!(time_value(0, 30), "0s");
    assert_eq!(time_value(30, 30), "1s"); // 30/30 = 1
    assert_eq!(time_value(1, 30), "1/30s");
    assert_eq!(time_value(45, 30), "3/2s"); // gcd 15
    assert_eq!(time_value(60, 24), "5/2s"); // gcd 12
}

#[test]
fn tc_format_drop_for_30_60() {
    assert_eq!(tc_format(30), "DF");
    assert_eq!(tc_format(60), "DF");
    assert_eq!(tc_format(24), "NDF");
    assert_eq!(tc_format(25), "NDF");
}

#[test]
fn linear_to_db_maps_unity_to_zero() {
    assert!((linear_to_db(1.0)).abs() < 1e-9);
    assert!(linear_to_db(0.0) <= -96.0);
    // -6 dB ~ 0.5
    assert!((linear_to_db(0.5) - (20.0 * 0.5f64.log10())).abs() < 1e-9);
}

// --- document shell ---

#[test]
fn document_has_fcpxml_header_and_version() {
    let tl = Timeline::new();
    let xml = export_fcpxml(&tl, &manifest(vec![]), None);
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE fcpxml>\n"));
    assert!(xml.contains("<fcpxml version=\"1.10\">"));
    assert!(xml.contains("<resources>"));
    assert!(xml.contains("<library>"));
    assert!(xml.contains("<event name=\"OpenTake\">"));
    assert!(xml.contains("<project name=\"Timeline Export\">"));
    // Empty timeline → self-closing spine.
    assert!(xml.contains("<spine/>") || xml.contains("<spine>"));
    // Timeline format r1 with 1/30s frame duration (default 30fps).
    assert!(xml.contains("<format id=\"r1\""));
    assert!(xml.contains("frameDuration=\"1/30s\""));
}

#[test]
fn sequence_carries_format_and_tcformat() {
    let mut tl = Timeline::new();
    tl.fps = 24;
    let xml = export_fcpxml(&tl, &manifest(vec![]), None);
    assert!(xml.contains("<sequence format=\"r1\""));
    assert!(xml.contains("tcFormat=\"NDF\""));
    assert!(xml.contains("frameDuration=\"1/24s\""));
}

// --- assets ---

#[test]
fn video_clip_emits_asset_and_asset_clip() {
    let mut tl = Timeline::new();
    let mut vt = Track::new("v", ClipType::Video);
    vt.clips.push(Clip::new("c1", "v1", 0, 60)); // 2s @30
    tl.tracks.push(vt);
    let xml = export_fcpxml(
        &tl,
        &manifest(vec![entry("v1", "shot.mp4", ClipType::Video, 4.0)]),
        None,
    );
    // Asset r2 with file:// src + per-source format r3.
    assert!(xml.contains("<asset id=\"r2\""));
    assert!(xml.contains("src=\"file:///media/shot.mp4\""));
    assert!(xml.contains("hasVideo=\"1\""));
    assert!(xml.contains("hasAudio=\"1\""));
    assert!(xml.contains("<format id=\"r3\""));
    // asset-clip references r2; offset 0, duration 2s (60/30).
    assert!(xml.contains("<asset-clip ref=\"r2\""));
    assert!(xml.contains("offset=\"0s\""));
    assert!(xml.contains("duration=\"2s\""));
}

#[test]
fn trim_sets_clip_start() {
    let mut tl = Timeline::new();
    let mut vt = Track::new("v", ClipType::Video);
    let mut clip = Clip::new("c1", "v1", 30, 30); // offset 1s
    clip.trim_start_frame = 15; // start 0.5s
    vt.clips.push(clip);
    tl.tracks.push(vt);
    let xml = export_fcpxml(
        &tl,
        &manifest(vec![entry("v1", "shot.mp4", ClipType::Video, 4.0)]),
        None,
    );
    assert!(xml.contains("offset=\"1s\""));
    assert!(xml.contains("start=\"1/2s\"")); // 15/30
}

#[test]
fn duplicate_media_ref_emits_single_asset() {
    let mut tl = Timeline::new();
    let mut vt = Track::new("v", ClipType::Video);
    vt.clips.push(Clip::new("c1", "v1", 0, 30));
    vt.clips.push(Clip::new("c2", "v1", 30, 30));
    tl.tracks.push(vt);
    let xml = export_fcpxml(
        &tl,
        &manifest(vec![entry("v1", "shot.mp4", ClipType::Video, 4.0)]),
        None,
    );
    // Only one asset r2 (the file:// src appears once).
    assert_eq!(xml.matches("src=\"file:///media/shot.mp4\"").count(), 1);
    // Two asset-clips both referencing r2.
    assert_eq!(xml.matches("<asset-clip ref=\"r2\"").count(), 2);
}

#[test]
fn audio_asset_has_audio_flags_no_video() {
    let mut tl = Timeline::new();
    let mut at = Track::new("a", ClipType::Audio);
    let mut clip = Clip::new("c1", "a1", 0, 60);
    clip.media_type = ClipType::Audio;
    at.clips.push(clip);
    tl.tracks.push(at);
    let xml = export_fcpxml(
        &tl,
        &manifest(vec![entry("a1", "song.mp3", ClipType::Audio, 10.0)]),
        None,
    );
    assert!(xml.contains("hasVideo=\"0\""));
    assert!(xml.contains("hasAudio=\"1\""));
    assert!(xml.contains("audioChannels=\"2\""));
    // Audio on a negative connected lane (no primary visual track here, so
    // the audio track itself becomes primary — lane omitted).
    assert!(xml.contains("<asset-clip ref=\"r2\""));
}

// --- text → title ---

#[test]
fn text_clip_emits_title_not_asset() {
    let mut tl = Timeline::new();
    let mut vt = Track::new("v", ClipType::Video);
    vt.clips.push(Clip::new("v1", "vid", 0, 60));
    let mut tt = Track::new("t", ClipType::Text);
    let mut text = Clip::new("t1", "text-asset", 0, 90);
    text.media_type = ClipType::Text;
    text.text_content = Some("Hello World".to_string());
    text.text_style = Some(TextStyle::default());
    tt.clips.push(text);
    tl.tracks.push(vt);
    tl.tracks.push(tt);
    let xml = export_fcpxml(
        &tl,
        &manifest(vec![entry("vid", "vid.mp4", ClipType::Video, 4.0)]),
        None,
    );
    assert!(xml.contains("<title"));
    assert!(xml.contains("Hello World"));
    // The text clip's media-ref must NOT appear as an asset.
    assert!(!xml.contains("text-asset"));
    // Title is a connected clip (lane 1) since video track is primary.
    assert!(xml.contains("lane=\"1\""));
}

// --- adjustments ---

#[test]
fn transform_emits_adjust_transform() {
    let mut tl = Timeline::new();
    let mut vt = Track::new("v", ClipType::Video);
    let mut clip = Clip::new("c1", "v1", 0, 30);
    clip.transform = Transform {
        center_x: 0.75, // offset right
        center_y: 0.5,
        width: 0.5, // scale 0.5
        height: 0.5,
        rotation: 10.0,
        flip_horizontal: false,
        flip_vertical: false,
    };
    vt.clips.push(clip);
    tl.tracks.push(vt);
    let xml = export_fcpxml(
        &tl,
        &manifest(vec![entry("v1", "shot.mp4", ClipType::Video, 4.0)]),
        None,
    );
    assert!(xml.contains("<adjust-transform"));
    assert!(xml.contains("scale=\"0.5000 0.5000\""));
    // rotation negated.
    assert!(xml.contains("rotation=\"-10.0000\""));
    // position x offset = (0.75-0.5)*1920 = 480.
    assert!(xml.contains("position=\"480.0000 0.0000\""));
}

#[test]
fn default_transform_emits_no_adjust_transform() {
    let mut tl = Timeline::new();
    let mut vt = Track::new("v", ClipType::Video);
    vt.clips.push(Clip::new("c1", "v1", 0, 30));
    tl.tracks.push(vt);
    let xml = export_fcpxml(
        &tl,
        &manifest(vec![entry("v1", "shot.mp4", ClipType::Video, 4.0)]),
        None,
    );
    assert!(!xml.contains("<adjust-transform"));
}

#[test]
fn audio_volume_emits_adjust_volume_in_db() {
    let mut tl = Timeline::new();
    let mut at = Track::new("a", ClipType::Audio);
    let mut clip = Clip::new("c1", "a1", 0, 60);
    clip.media_type = ClipType::Audio;
    clip.volume = 0.5; // ~ -6 dB
    at.clips.push(clip);
    tl.tracks.push(at);
    let xml = export_fcpxml(
        &tl,
        &manifest(vec![entry("a1", "song.mp3", ClipType::Audio, 10.0)]),
        None,
    );
    assert!(xml.contains("<adjust-volume"));
    assert!(xml.contains("dB"));
}

// --- lanes / spine ---

#[test]
fn first_video_track_is_primary_others_get_lanes() {
    let mut tl = Timeline::new();
    let mut v1 = Track::new("v1", ClipType::Video);
    v1.clips.push(Clip::new("c1", "m1", 0, 30));
    let mut v2 = Track::new("v2", ClipType::Video);
    v2.clips.push(Clip::new("c2", "m2", 0, 30));
    let mut a1 = Track::new("a1", ClipType::Audio);
    let mut ac = Clip::new("c3", "m3", 0, 30);
    ac.media_type = ClipType::Audio;
    a1.clips.push(ac);
    tl.tracks.push(v1);
    tl.tracks.push(v2);
    tl.tracks.push(a1);
    let xml = export_fcpxml(
        &tl,
        &manifest(vec![
            entry("m1", "a.mp4", ClipType::Video, 4.0),
            entry("m2", "b.mp4", ClipType::Video, 4.0),
            entry("m3", "c.mp3", ClipType::Audio, 4.0),
        ]),
        None,
    );
    // Second video track → lane 1; audio → lane -1.
    assert!(xml.contains("lane=\"1\""));
    assert!(xml.contains("lane=\"-1\""));
}

// --- empty / unresolved ---

#[test]
fn empty_timeline_has_empty_spine() {
    let tl = Timeline::new();
    let xml = export_fcpxml(&tl, &manifest(vec![]), None);
    assert!(xml.contains("<spine/>") || xml.contains("<spine>\n"));
}

#[test]
fn unresolved_media_uses_relative_src() {
    let mut tl = Timeline::new();
    let mut vt = Track::new("v", ClipType::Video);
    vt.clips.push(Clip::new("c1", "ghost", 0, 30));
    tl.tracks.push(vt);
    let xml = export_fcpxml(&tl, &manifest(vec![]), None);
    assert!(xml.contains("src=\"media/ghost\""));
}

#[test]
fn output_is_well_formed_escaped() {
    // Text with XML metacharacters must be escaped.
    let mut tl = Timeline::new();
    let mut tt = Track::new("t", ClipType::Text);
    let mut text = Clip::new("t1", "ta", 0, 30);
    text.media_type = ClipType::Text;
    text.text_content = Some("A & B <tag>".to_string());
    tt.clips.push(text);
    tl.tracks.push(tt);
    let xml = export_fcpxml(&tl, &manifest(vec![]), None);
    assert!(xml.contains("A &amp; B &lt;tag&gt;"));
    assert!(!xml.contains("A & B <tag>"));
}
