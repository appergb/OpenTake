//! Fail-closed compatibility handling for tolerant timeline wire fields.
//!
//! The domain model owns the persisted field descriptors. This module only
//! walks raw JSON with those descriptors to retain future/unknown data guards
//! that serde cannot see after a Swift-compatible `try?` fallback.

use opentake_domain::{
    AnimPair, Clip, Crop, Fill, Keyframe, KeyframeTrack, KeyframeValueWireShape, Rgba, Shadow,
    TextStyle, Timeline, Track, Transform,
};
use serde_json::Value;
use uuid::Uuid;

/// Timeline prepared for the narrow Swift-compatible `Track.clips` fallback.
///
/// This value exists only after the formal decode failed. Each present
/// `Track.clips` value has been probed exactly once, and the document clone is
/// created only when at least one probe failed.
pub(crate) struct TimelineFallback {
    pub(crate) normalized: Value,
    pub(crate) failed_tracks: Vec<bool>,
}

/// Restore upstream UUID fallback semantics at the persistence boundary, where
/// the raw document still distinguishes an explicit empty string from a
/// missing, null, or non-string ID. The decoded Timeline keeps identical track
/// and clip ordering; a Track.clips fallback yields an empty decoded vector and
/// is therefore skipped safely.
pub(crate) fn repair_timeline_ids(timeline: &mut Timeline, document: &Value) {
    let Some(raw_tracks) = document
        .get(Timeline::TRACKS_WIRE_FIELD)
        .and_then(Value::as_array)
    else {
        return;
    };

    for (track_index, track) in timeline.tracks.iter_mut().enumerate() {
        let Some(raw_track) = raw_tracks.get(track_index) else {
            break;
        };
        repair_id(&mut track.id, raw_track.get(Track::ID_WIRE_FIELD));

        let Some(raw_clips) = raw_track
            .get(Track::CLIPS_WIRE_FIELD)
            .and_then(Value::as_array)
        else {
            continue;
        };
        for (clip_index, clip) in track.clips.iter_mut().enumerate() {
            let Some(raw_clip) = raw_clips.get(clip_index) else {
                break;
            };
            repair_id(&mut clip.id, raw_clip.get(Clip::ID_WIRE_FIELD));
        }
    }
}

fn repair_id(id: &mut String, raw: Option<&Value>) {
    if let Some(Value::String(raw)) = raw {
        id.clone_from(raw);
    } else {
        *id = Uuid::new_v4().to_string();
    }
}

/// Probe only the fallback that upstream explicitly tolerates:
/// `(try? decode([Clip])) ?? []` on each Track.
pub(crate) fn prepare_timeline_fallback(document: &Value) -> Option<TimelineFallback> {
    let tracks = document
        .get(Timeline::TRACKS_WIRE_FIELD)
        .and_then(Value::as_array)?;
    let mut failed_tracks = vec![false; tracks.len()];

    for (index, track) in tracks.iter().enumerate() {
        let Some(clips) = track.get(Track::CLIPS_WIRE_FIELD) else {
            continue;
        };
        if serde_json::from_value::<Vec<Clip>>(clips.clone()).is_err() {
            failed_tracks[index] = true;
        }
    }

    if !failed_tracks.iter().any(|failed| *failed) {
        return None;
    }

    let mut normalized = document.clone();
    let normalized_tracks = normalized
        .get_mut(Timeline::TRACKS_WIRE_FIELD)?
        .as_array_mut()?;
    for (index, failed) in failed_tracks.iter().copied().enumerate() {
        if !failed {
            continue;
        }
        if let Some(clips) = normalized_tracks[index].get_mut(Track::CLIPS_WIRE_FIELD) {
            *clips = Value::Array(Vec::new());
        }
    }

    Some(TimelineFallback {
        normalized,
        failed_tracks,
    })
}

/// Inspect tolerant/buffered timeline fields that `serde_ignored` cannot see.
/// `failed_tracks` is the cached result from [`prepare_timeline_fallback`]; this
/// visitor never repeats a complete `Vec<Clip>` decode.
pub(crate) fn scan_timeline(
    document: &Value,
    file: &str,
    failed_tracks: &[bool],
    ignored: &mut Vec<String>,
) {
    let Some(tracks) = document
        .get(Timeline::TRACKS_WIRE_FIELD)
        .and_then(Value::as_array)
    else {
        return;
    };

    for (track_index, track) in tracks.iter().enumerate() {
        let track_path = format!("{}.{track_index}", Timeline::TRACKS_WIRE_FIELD);
        for field in Track::TOLERANT_SCALAR_WIRE_FIELDS {
            scan_future_scalar_shape(
                track.get(*field),
                &format!("{track_path}.{field}"),
                file,
                ignored,
            );
        }

        let clips_path = format!("{track_path}.{}", Track::CLIPS_WIRE_FIELD);
        let clips = match track.get(Track::CLIPS_WIRE_FIELD) {
            Some(Value::Array(clips)) => clips,
            Some(Value::Object(fields)) => {
                if fields.is_empty() {
                    ignored.push(format!("{file}:{clips_path}"));
                } else {
                    ignored.extend(
                        fields
                            .keys()
                            .map(|key| format!("{file}:{clips_path}.{key}")),
                    );
                }
                continue;
            }
            Some(_) | None => continue,
        };

        let clips_failed = failed_tracks.get(track_index).copied().unwrap_or(false);
        if clips_failed {
            ignored.push(format!("{file}:{clips_path}:invalid-or-unreadable"));
        }

        for (clip_index, clip) in clips.iter().enumerate() {
            let base = format!("{clips_path}.{clip_index}");
            if clips_failed {
                scan_decodable_clip_unknowns(clip, &base, file, ignored);
            }
            scan_object_keys(Some(clip), &base, Clip::WIRE_FIELDS, file, ignored);
            for field in Clip::TOLERANT_SCALAR_WIRE_FIELDS {
                scan_future_scalar_shape(
                    clip.get(*field),
                    &format!("{base}.{field}"),
                    file,
                    ignored,
                );
            }

            scan_future_object_shape(
                clip.get(Clip::TRANSFORM_WIRE_FIELD),
                &format!("{base}.{}", Clip::TRANSFORM_WIRE_FIELD),
                Transform::COMPATIBLE_WIRE_FIELDS,
                file,
                ignored,
            );
            scan_object_scalar_fields(
                clip.get(Clip::TRANSFORM_WIRE_FIELD),
                &format!("{base}.{}", Clip::TRANSFORM_WIRE_FIELD),
                Transform::SCALAR_WIRE_FIELDS,
                file,
                ignored,
            );
            scan_future_object_shape(
                clip.get(Clip::CROP_WIRE_FIELD),
                &format!("{base}.{}", Clip::CROP_WIRE_FIELD),
                Crop::WIRE_FIELDS,
                file,
                ignored,
            );
            scan_object_scalar_fields(
                clip.get(Clip::CROP_WIRE_FIELD),
                &format!("{base}.{}", Clip::CROP_WIRE_FIELD),
                Crop::SCALAR_WIRE_FIELDS,
                file,
                ignored,
            );
            if let Some(style) = clip.get(Clip::TEXT_STYLE_WIRE_FIELD) {
                scan_text_style(
                    style,
                    &format!("{base}.{}", Clip::TEXT_STYLE_WIRE_FIELD),
                    file,
                    ignored,
                );
            }
            for descriptor in Clip::KEYFRAME_TRACK_WIRE_FIELDS {
                if let Some(track) = clip.get(descriptor.name) {
                    scan_keyframe_track(
                        track,
                        &format!("{base}.{}", descriptor.name),
                        descriptor.value_shape,
                        file,
                        ignored,
                    );
                }
            }
        }
    }
}

fn scan_decodable_clip_unknowns(clip: &Value, path: &str, file: &str, ignored: &mut Vec<String>) {
    let Ok(bytes) = serde_json::to_vec(clip) else {
        return;
    };
    let mut decoder = serde_json::Deserializer::from_slice(&bytes);
    let _: std::result::Result<Clip, _> = serde_ignored::deserialize(&mut decoder, |unknown| {
        let suffix = canonical_ignored_path(&unknown, clip);
        ignored.push(format!("{file}:{path}.{suffix}"));
    });
}

fn scan_keyframe_track(
    value: &Value,
    path: &str,
    value_shape: KeyframeValueWireShape,
    file: &str,
    ignored: &mut Vec<String>,
) {
    if matches!(value, Value::Array(_)) {
        ignored.push(format!("{file}:{path}"));
        return;
    }
    if !value.is_object() {
        return;
    }

    scan_object_keys(
        Some(value),
        path,
        KeyframeTrack::<f64>::WIRE_FIELDS,
        file,
        ignored,
    );
    let keyframes_field = KeyframeTrack::<f64>::KEYFRAMES_WIRE_FIELD;
    let keyframes_path = format!("{path}.{keyframes_field}");
    let keyframes = match value.get(keyframes_field) {
        Some(Value::Array(keyframes)) => keyframes,
        Some(Value::Object(fields)) => {
            if fields.is_empty() {
                ignored.push(format!("{file}:{keyframes_path}"));
            } else {
                ignored.extend(
                    fields
                        .keys()
                        .map(|key| format!("{file}:{keyframes_path}.{key}")),
                );
            }
            return;
        }
        _ => return,
    };

    for (index, keyframe) in keyframes.iter().enumerate() {
        let keyframe_path = format!("{keyframes_path}.{index}");
        if matches!(keyframe, Value::Array(_)) {
            ignored.push(format!("{file}:{keyframe_path}"));
            continue;
        }
        if !keyframe.is_object() {
            continue;
        }
        scan_object_keys(
            Some(keyframe),
            &keyframe_path,
            Keyframe::<f64>::WIRE_FIELDS,
            file,
            ignored,
        );
        scan_object_scalar_fields(
            Some(keyframe),
            &keyframe_path,
            Keyframe::<f64>::SCALAR_WIRE_FIELDS,
            file,
            ignored,
        );
        let value_field = Keyframe::<f64>::VALUE_WIRE_FIELD;
        let value_path = format!("{keyframe_path}.{value_field}");
        match (value_shape, keyframe.get(value_field)) {
            (KeyframeValueWireShape::Pair, Some(Value::Object(_))) => {
                scan_object_keys(
                    keyframe.get(value_field),
                    &value_path,
                    AnimPair::WIRE_FIELDS,
                    file,
                    ignored,
                );
                scan_object_scalar_fields(
                    keyframe.get(value_field),
                    &value_path,
                    AnimPair::SCALAR_WIRE_FIELDS,
                    file,
                    ignored,
                );
            }
            (KeyframeValueWireShape::Crop, Some(Value::Object(_))) => {
                scan_object_keys(
                    keyframe.get(value_field),
                    &value_path,
                    Crop::WIRE_FIELDS,
                    file,
                    ignored,
                );
                scan_object_scalar_fields(
                    keyframe.get(value_field),
                    &value_path,
                    Crop::SCALAR_WIRE_FIELDS,
                    file,
                    ignored,
                );
            }
            (
                KeyframeValueWireShape::Pair | KeyframeValueWireShape::Crop,
                Some(Value::Array(_)),
            ) => ignored.push(format!("{file}:{value_path}")),
            (KeyframeValueWireShape::Scalar, Some(Value::Object(fields))) => {
                if fields.is_empty() {
                    ignored.push(format!("{file}:{value_path}"));
                } else {
                    ignored.extend(
                        fields
                            .keys()
                            .map(|key| format!("{file}:{value_path}.{key}")),
                    );
                }
            }
            (KeyframeValueWireShape::Scalar, Some(Value::Array(_))) => {
                ignored.push(format!("{file}:{value_path}"));
            }
            _ => {}
        }
    }
}

fn scan_text_style(value: &Value, path: &str, file: &str, ignored: &mut Vec<String>) {
    if matches!(value, Value::Array(_)) {
        ignored.push(format!("{file}:{path}"));
        return;
    }
    if !value.is_object() {
        return;
    }

    scan_object_keys(Some(value), path, TextStyle::WIRE_FIELDS, file, ignored);
    for field in TextStyle::TOLERANT_SCALAR_WIRE_FIELDS {
        scan_future_scalar_shape(value.get(*field), &format!("{path}.{field}"), file, ignored);
    }
    scan_rgba(
        value.get(TextStyle::COLOR_WIRE_FIELD),
        &format!("{path}.{}", TextStyle::COLOR_WIRE_FIELD),
        file,
        ignored,
    );

    if let Some(shadow) = value.get(TextStyle::SHADOW_WIRE_FIELD) {
        let shadow_path = format!("{path}.{}", TextStyle::SHADOW_WIRE_FIELD);
        scan_future_object_shape(
            Some(shadow),
            &shadow_path,
            Shadow::WIRE_FIELDS,
            file,
            ignored,
        );
        for field in Shadow::TOLERANT_SCALAR_WIRE_FIELDS {
            scan_future_scalar_shape(
                shadow.get(*field),
                &format!("{shadow_path}.{field}"),
                file,
                ignored,
            );
        }
        scan_rgba(
            shadow.get(Shadow::COLOR_WIRE_FIELD),
            &format!("{shadow_path}.{}", Shadow::COLOR_WIRE_FIELD),
            file,
            ignored,
        );
    }

    for field in TextStyle::FILL_WIRE_FIELDS {
        if let Some(fill) = value.get(*field) {
            let fill_path = format!("{path}.{field}");
            scan_future_object_shape(Some(fill), &fill_path, Fill::WIRE_FIELDS, file, ignored);
            for scalar in Fill::TOLERANT_SCALAR_WIRE_FIELDS {
                scan_future_scalar_shape(
                    fill.get(*scalar),
                    &format!("{fill_path}.{scalar}"),
                    file,
                    ignored,
                );
            }
            scan_rgba(
                fill.get(Fill::COLOR_WIRE_FIELD),
                &format!("{fill_path}.{}", Fill::COLOR_WIRE_FIELD),
                file,
                ignored,
            );
        }
    }
}

fn scan_rgba(value: Option<&Value>, path: &str, file: &str, ignored: &mut Vec<String>) {
    scan_future_object_shape(value, path, Rgba::WIRE_FIELDS, file, ignored);
    if let Some(value) = value {
        for field in Rgba::TOLERANT_SCALAR_WIRE_FIELDS {
            scan_future_scalar_shape(value.get(*field), &format!("{path}.{field}"), file, ignored);
        }
    }
}

fn scan_future_object_shape(
    value: Option<&Value>,
    path: &str,
    known: &[&str],
    file: &str,
    ignored: &mut Vec<String>,
) {
    if matches!(value, Some(Value::Array(_))) {
        ignored.push(format!("{file}:{path}"));
        return;
    }
    scan_object_keys(value, path, known, file, ignored);
}

fn scan_future_scalar_shape(
    value: Option<&Value>,
    path: &str,
    file: &str,
    ignored: &mut Vec<String>,
) {
    match value {
        Some(Value::Array(_)) => ignored.push(format!("{file}:{path}")),
        Some(Value::Object(fields)) if fields.is_empty() => {
            ignored.push(format!("{file}:{path}"));
        }
        Some(Value::Object(fields)) => {
            ignored.extend(fields.keys().map(|key| format!("{file}:{path}.{key}")))
        }
        _ => {}
    }
}

fn scan_object_scalar_fields(
    value: Option<&Value>,
    path: &str,
    fields: &[&str],
    file: &str,
    ignored: &mut Vec<String>,
) {
    let Some(value) = value.filter(|value| value.is_object()) else {
        return;
    };
    for field in fields {
        scan_future_scalar_shape(value.get(*field), &format!("{path}.{field}"), file, ignored);
    }
}

fn scan_object_keys(
    value: Option<&Value>,
    path: &str,
    known: &[&str],
    file: &str,
    ignored: &mut Vec<String>,
) {
    let Some(object) = value.and_then(Value::as_object) else {
        return;
    };
    for key in object.keys() {
        if !known.contains(&key.as_str()) {
            ignored.push(format!("{file}:{path}.{key}"));
        }
    }
}

enum IgnoredSegment {
    Map(String),
    Seq(usize),
}

pub(crate) fn canonical_ignored_path(path: &serde_ignored::Path<'_>, document: &Value) -> String {
    fn collect(path: &serde_ignored::Path<'_>, segments: &mut Vec<IgnoredSegment>) {
        match path {
            serde_ignored::Path::Root => {}
            serde_ignored::Path::Seq { parent, index } => {
                collect(parent, segments);
                segments.push(IgnoredSegment::Seq(*index));
            }
            serde_ignored::Path::Map { parent, key } => {
                collect(parent, segments);
                segments.push(IgnoredSegment::Map(key.clone()));
            }
            serde_ignored::Path::Some { parent }
            | serde_ignored::Path::NewtypeStruct { parent }
            | serde_ignored::Path::NewtypeVariant { parent } => collect(parent, segments),
        }
    }

    let mut segments = Vec::new();
    collect(path, &mut segments);

    let mut current = Some(document);
    let mut rendered = Vec::new();
    for segment in segments {
        match segment {
            IgnoredSegment::Seq(index) => {
                rendered.push(index.to_string());
                current = current
                    .and_then(Value::as_array)
                    .and_then(|array| array.get(index));
            }
            IgnoredSegment::Map(key) => {
                let direct = current
                    .and_then(Value::as_object)
                    .and_then(|object| object.get(&key));
                if let Some(value) = direct {
                    rendered.push(key);
                    current = Some(value);
                    continue;
                }

                let variant = current
                    .and_then(Value::as_object)
                    .filter(|object| object.len() == 1)
                    .and_then(|object| object.iter().next())
                    .filter(|(_, value)| {
                        value
                            .as_object()
                            .is_some_and(|fields| fields.contains_key(&key))
                    });
                if let Some((variant_name, variant_value)) = variant {
                    rendered.push(variant_name.clone());
                    rendered.push(key.clone());
                    current = variant_value
                        .as_object()
                        .and_then(|fields| fields.get(&key));
                } else {
                    rendered.push(key);
                    current = None;
                }
            }
        }
    }
    rendered.join(".")
}
