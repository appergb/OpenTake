use std::collections::BTreeSet;

use opentake_domain::{
    AnimPair, ChromaKey, Clip, ClipType, ColorGrade, Crop, Effect, Fill, Interpolation, Keyframe,
    KeyframeTrack, LutReference, Mask, Rgba, Shadow, StabilizationKeyframe, StabilizationTrack,
    TextAlignment, TextStyle, Track, Transform, Transition, TransitionKind,
};
use serde::Serialize;

fn serialized_keys(value: &impl Serialize) -> BTreeSet<String> {
    serde_json::to_value(value)
        .expect("wire value serializes")
        .as_object()
        .expect("wire value is an object")
        .keys()
        .cloned()
        .collect()
}

fn declared_keys(fields: &[&str]) -> BTreeSet<String> {
    fields.iter().map(|field| (*field).to_owned()).collect()
}

fn assert_schema(value: &impl Serialize, fields: &[&str]) {
    assert_eq!(serialized_keys(value), declared_keys(fields));
}

fn full_text_style() -> TextStyle {
    TextStyle {
        font_name: "WireGuard".to_owned(),
        font_size: 72.0,
        font_scale: 1.25,
        color: Rgba::new(0.1, 0.2, 0.3, 0.4),
        alignment: TextAlignment::Right,
        shadow: Shadow {
            enabled: true,
            color: Rgba::new(0.2, 0.3, 0.4, 0.5),
            offset_x: 2.0,
            offset_y: 3.0,
            blur: 4.0,
        },
        background: Fill::new(true, Rgba::new(0.3, 0.4, 0.5, 0.6)),
        border: Fill::new(true, Rgba::new(0.4, 0.5, 0.6, 0.7)),
    }
}

fn full_clip() -> Clip {
    let mut clip = Clip::new("clip", "asset", 10, 20);
    clip.media_type = ClipType::Audio;
    clip.source_clip_type = ClipType::Text;
    clip.trim_start_frame = 1;
    clip.trim_end_frame = 2;
    clip.speed = 1.25;
    clip.volume = 0.75;
    clip.fade_in_frames = 3;
    clip.fade_out_frames = 4;
    clip.fade_in_interpolation = Interpolation::Hold;
    clip.fade_out_interpolation = Interpolation::Smooth;
    clip.opacity = 0.8;
    clip.transform = Transform {
        center_x: 0.25,
        center_y: 0.75,
        width: 0.5,
        height: 0.4,
        rotation: 15.0,
        flip_horizontal: true,
        flip_vertical: true,
    };
    clip.crop = Crop {
        left: 0.1,
        top: 0.2,
        right: 0.3,
        bottom: 0.4,
    };
    clip.link_group_id = Some("link".to_owned());
    clip.caption_group_id = Some("caption".to_owned());
    clip.nested_sequence_id = Some("sequence".to_owned());
    clip.text_content = Some("text".to_owned());
    clip.text_style = Some(full_text_style());
    clip.opacity_track = Some(KeyframeTrack::from_keyframes(vec![
        Keyframe::with_interpolation(0, 0.5, Interpolation::Linear),
    ]));
    clip.position_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(
        0,
        AnimPair::new(0.1, 0.2),
    )]));
    clip.scale_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(
        0,
        AnimPair::new(1.1, 1.2),
    )]));
    clip.rotation_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(0, 12.0)]));
    clip.crop_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(
        0,
        Crop {
            left: 0.1,
            top: 0.1,
            right: 0.1,
            bottom: 0.1,
        },
    )]));
    clip.volume_track = Some(KeyframeTrack::from_keyframes(vec![Keyframe::new(0, 0.9)]));
    clip.color_grade = Some(ColorGrade::default());
    clip.lut = Some(
        LutReference::new("0123456789abcdef".repeat(4), "Wire schema LUT", 0.75)
            .expect("wire LUT reference is valid"),
    );
    clip.chroma_key = Some(ChromaKey::default());
    clip.masks = vec![Mask::default()];
    clip.effects = vec![Effect::new("wire").with_param("amount", 1.0)];
    clip.stabilization = Some(StabilizationTrack {
        model: "wire".to_owned(),
        model_version: 1,
        source_identity: "asset".to_owned(),
        strength: 0.75,
        crop_margin: 0.02,
        keyframes: vec![
            StabilizationKeyframe::default(),
            StabilizationKeyframe {
                frame: 1,
                translation_x: 0.01,
                translation_y: -0.01,
                rotation_degrees: 0.5,
            },
        ],
    });
    clip.transition_out = Some(Transition {
        from_clip_id: "clip".to_owned(),
        to_clip_id: "next".to_owned(),
        kind: TransitionKind::CrossDissolve,
        duration_frames: 5,
    });
    clip.reversed = true;
    clip
}

#[test]
fn wire_field_descriptors_match_fully_populated_serialization() {
    let clip = full_clip();
    let mut track = Track::new("track", ClipType::Video);
    track.muted = true;
    track.hidden = true;
    track.sync_locked = false;
    track.clips.push(clip.clone());

    assert_schema(&clip, Clip::WIRE_FIELDS);
    assert_schema(&track, Track::WIRE_FIELDS);
    assert_schema(clip.text_style.as_ref().unwrap(), TextStyle::WIRE_FIELDS);
    assert_schema(&clip.transform, Transform::WIRE_FIELDS);
    assert_schema(&clip.crop, Crop::WIRE_FIELDS);
    assert_schema(
        &clip.opacity_track.as_ref().unwrap().keyframes[0],
        Keyframe::<f64>::WIRE_FIELDS,
    );
    assert_schema(
        &clip.position_track.as_ref().unwrap().keyframes[0].value,
        AnimPair::WIRE_FIELDS,
    );
    assert_schema(
        clip.opacity_track.as_ref().unwrap(),
        KeyframeTrack::<f64>::WIRE_FIELDS,
    );
    assert_schema(&clip.text_style.as_ref().unwrap().color, Rgba::WIRE_FIELDS);
    assert_schema(
        &clip.text_style.as_ref().unwrap().shadow,
        Shadow::WIRE_FIELDS,
    );
    assert_schema(
        &clip.text_style.as_ref().unwrap().background,
        Fill::WIRE_FIELDS,
    );

    assert_eq!(
        declared_keys(Transform::LEGACY_WIRE_FIELDS),
        declared_keys(&["x", "y"]),
    );
    let mut current_and_legacy = declared_keys(Transform::WIRE_FIELDS);
    current_and_legacy.extend(declared_keys(Transform::LEGACY_WIRE_FIELDS));
    assert_eq!(
        current_and_legacy,
        declared_keys(Transform::COMPATIBLE_WIRE_FIELDS),
    );
}
