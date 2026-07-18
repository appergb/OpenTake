//! Private serde compatibility adapters for [`TextStyle`](crate::TextStyle).

use serde::Deserialize;

use crate::text::{
    default_background_fill, default_border_fill, default_font_name, default_font_scale,
    default_font_size, Fill, Rgba, Shadow,
};

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

pub(crate) fn deserialize_default_on_error<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    deserialize_or(deserializer, T::default())
}

#[derive(Deserialize)]
struct StrictRgba {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

impl From<Rgba> for StrictRgba {
    fn from(value: Rgba) -> Self {
        Self {
            r: value.r,
            g: value.g,
            b: value.b,
            a: value.a,
        }
    }
}

impl From<StrictRgba> for Rgba {
    fn from(value: StrictRgba) -> Self {
        Rgba::new(value.r, value.g, value.b, value.a)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StrictShadow {
    enabled: bool,
    color: StrictRgba,
    offset_x: f64,
    offset_y: f64,
    blur: f64,
}

impl From<Shadow> for StrictShadow {
    fn from(value: Shadow) -> Self {
        Self {
            enabled: value.enabled,
            color: value.color.into(),
            offset_x: value.offset_x,
            offset_y: value.offset_y,
            blur: value.blur,
        }
    }
}

impl From<StrictShadow> for Shadow {
    fn from(value: StrictShadow) -> Self {
        Shadow {
            enabled: value.enabled,
            color: value.color.into(),
            offset_x: value.offset_x,
            offset_y: value.offset_y,
            blur: value.blur,
        }
    }
}

#[derive(Deserialize)]
struct StrictFill {
    enabled: bool,
    color: StrictRgba,
}

impl From<Fill> for StrictFill {
    fn from(value: Fill) -> Self {
        Self {
            enabled: value.enabled,
            color: value.color.into(),
        }
    }
}

impl From<StrictFill> for Fill {
    fn from(value: StrictFill) -> Self {
        Fill::new(value.enabled, value.color.into())
    }
}

pub(crate) fn deserialize_font_name_on_error<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_or(deserializer, default_font_name())
}

pub(crate) fn deserialize_font_size_on_error<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_or(deserializer, default_font_size())
}

pub(crate) fn deserialize_font_scale_on_error<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_or(deserializer, default_font_scale())
}

pub(crate) fn deserialize_color_on_error<'de, D>(deserializer: D) -> Result<Rgba, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_or(deserializer, StrictRgba::from(Rgba::default())).map(Into::into)
}

pub(crate) fn deserialize_shadow_on_error<'de, D>(deserializer: D) -> Result<Shadow, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_or(deserializer, StrictShadow::from(Shadow::default())).map(Into::into)
}

pub(crate) fn deserialize_background_on_error<'de, D>(deserializer: D) -> Result<Fill, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_or(deserializer, StrictFill::from(default_background_fill())).map(Into::into)
}

pub(crate) fn deserialize_border_on_error<'de, D>(deserializer: D) -> Result<Fill, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_or(deserializer, StrictFill::from(default_border_fill())).map(Into::into)
}
