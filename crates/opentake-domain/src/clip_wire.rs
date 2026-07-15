//! Private serde compatibility adapters for [`Clip`](crate::Clip).
//!
//! These types mirror upstream's tolerant `try? decode` wire behavior. They
//! are intentionally kept out of `clip.rs`, which owns the domain model and
//! editing logic.

use serde::Deserialize;

use crate::keyframe::{AnimPair, Interpolation, Keyframe, KeyframeTrack};
use crate::transform::Crop;

pub(crate) fn deserialize_default_on_error<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    deserialize_or(deserializer, T::default())
}

fn deserialize_or<'de, D, T>(deserializer: D, fallback: T) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    #[allow(dead_code)]
    enum Tolerant<T> {
        Value(T),
        Invalid(serde::de::IgnoredAny),
    }

    Ok(match Tolerant::deserialize(deserializer)? {
        Tolerant::Value(value) => value,
        Tolerant::Invalid(_) => fallback,
    })
}

pub(crate) fn deserialize_one_on_error<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_or(deserializer, 1.0)
}

pub(crate) fn deserialize_linear_on_error<'de, D>(
    deserializer: D,
) -> Result<Interpolation, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_or(deserializer, Interpolation::Linear)
}

#[derive(Deserialize)]
struct StrictCrop {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl From<Crop> for StrictCrop {
    fn from(value: Crop) -> Self {
        Self {
            left: value.left,
            top: value.top,
            right: value.right,
            bottom: value.bottom,
        }
    }
}

impl From<StrictCrop> for Crop {
    fn from(value: StrictCrop) -> Self {
        Crop {
            left: value.left,
            top: value.top,
            right: value.right,
            bottom: value.bottom,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StrictKeyframe<V> {
    frame: i32,
    value: V,
    interpolation_out: Interpolation,
}

#[derive(Deserialize)]
struct StrictKeyframeTrack<V> {
    keyframes: Vec<StrictKeyframe<V>>,
}

fn into_keyframe_track<V, W>(wire: StrictKeyframeTrack<V>) -> KeyframeTrack<W>
where
    W: From<V>,
{
    KeyframeTrack::from_keyframes(
        wire.keyframes
            .into_iter()
            .map(|keyframe| Keyframe {
                frame: keyframe.frame,
                value: keyframe.value.into(),
                interpolation_out: keyframe.interpolation_out,
            })
            .collect(),
    )
}

pub(crate) fn deserialize_crop_on_error<'de, D>(deserializer: D) -> Result<Crop, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_or(deserializer, StrictCrop::from(Crop::default())).map(Into::into)
}

pub(crate) fn deserialize_optional_f64_track_on_error<'de, D>(
    deserializer: D,
) -> Result<Option<KeyframeTrack<f64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let wire: Option<StrictKeyframeTrack<f64>> = deserialize_or(deserializer, None)?;
    Ok(wire.map(into_keyframe_track))
}

pub(crate) fn deserialize_optional_pair_track_on_error<'de, D>(
    deserializer: D,
) -> Result<Option<KeyframeTrack<AnimPair>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let wire: Option<StrictKeyframeTrack<AnimPair>> = deserialize_or(deserializer, None)?;
    Ok(wire.map(into_keyframe_track))
}

pub(crate) fn deserialize_optional_crop_track_on_error<'de, D>(
    deserializer: D,
) -> Result<Option<KeyframeTrack<Crop>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let wire: Option<StrictKeyframeTrack<StrictCrop>> = deserialize_or(deserializer, None)?;
    Ok(wire.map(into_keyframe_track))
}
