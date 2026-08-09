//! Pure domain model and bounded parser for project-managed 3D `.cube` LUTs.
//!
//! File I/O belongs to the desktop/project layers. This module accepts an
//! already-bounded byte slice, validates the complete table, and exposes only
//! finite data suitable for GPU upload.

use serde::{Deserialize, Serialize};

/// Authored reference persisted on a clip. The content hash is also the only
/// allowed storage key; no ambient source path is retained in project JSON.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LutReference {
    pub id: String,
    pub name: String,
    pub intensity: f64,
}

impl LutReference {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        intensity: f64,
    ) -> Result<Self, LutReferenceValidationError> {
        let reference = Self {
            id: id.into(),
            name: name.into(),
            intensity,
        };
        reference.validate()?;
        Ok(reference)
    }

    pub fn validate(&self) -> Result<(), LutReferenceValidationError> {
        if self.id.len() != 64
            || !self
                .id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(LutReferenceValidationError::InvalidId);
        }
        if self.name.is_empty() || self.name.len() > 128 || self.name.chars().any(char::is_control)
        {
            return Err(LutReferenceValidationError::InvalidName);
        }
        if !self.intensity.is_finite() || !(0.0..=1.0).contains(&self.intensity) {
            return Err(LutReferenceValidationError::InvalidIntensity);
        }
        Ok(())
    }

    /// Canonical bundle-relative location. It is derived, never deserialized.
    pub fn relative_path(&self) -> String {
        format!("media/luts/{}.cube", self.id)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LutReferenceValidationError {
    InvalidId,
    InvalidName,
    InvalidIntensity,
}

impl std::fmt::Display for LutReferenceValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidId => "id must be a lowercase 64-character SHA-256 digest",
            Self::InvalidName => "name must contain 1..=128 bytes without control characters",
            Self::InvalidIntensity => "intensity must be finite and within [0, 1]",
        })
    }
}

impl std::error::Error for LutReferenceValidationError {}

/// Fully validated 3D table in `.cube` red-fastest order.
#[derive(Clone, PartialEq, Debug)]
pub struct CubeLut {
    title: Option<String>,
    size: u32,
    domain_min: [f32; 3],
    domain_max: [f32; 3],
    table: Vec<[f32; 3]>,
}

impl CubeLut {
    /// Hard read/parse ceiling for an untrusted input file.
    pub const MAX_BYTES: usize = 4 * 1024 * 1024;

    pub fn parse(bytes: &[u8]) -> Result<Self, CubeLutError> {
        if bytes.len() > Self::MAX_BYTES {
            return Err(CubeLutError::TooLarge {
                actual: bytes.len(),
                maximum: Self::MAX_BYTES,
            });
        }
        let text = std::str::from_utf8(bytes).map_err(|_| CubeLutError::InvalidUtf8)?;
        let mut title = None;
        let mut size = None;
        let mut domain_min = None;
        let mut domain_max = None;
        let mut table = Vec::new();

        for (zero_line, raw) in text.lines().enumerate() {
            let line_number = zero_line + 1;
            let line = raw.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            let fields = line.split_whitespace().collect::<Vec<_>>();
            match fields[0] {
                "TITLE" => {
                    if title.is_some() {
                        return Err(CubeLutError::DuplicateDirective { directive: "TITLE" });
                    }
                    let value = line["TITLE".len()..].trim().trim_matches('"');
                    if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control)
                    {
                        return Err(CubeLutError::InvalidDirective {
                            line: line_number,
                            directive: "TITLE",
                        });
                    }
                    title = Some(value.to_owned());
                }
                "LUT_3D_SIZE" => {
                    if size.is_some() {
                        return Err(CubeLutError::DuplicateDirective {
                            directive: "LUT_3D_SIZE",
                        });
                    }
                    if fields.len() != 2 {
                        return Err(CubeLutError::InvalidDirective {
                            line: line_number,
                            directive: "LUT_3D_SIZE",
                        });
                    }
                    let parsed =
                        fields[1]
                            .parse::<u32>()
                            .map_err(|_| CubeLutError::InvalidDirective {
                                line: line_number,
                                directive: "LUT_3D_SIZE",
                            })?;
                    if !matches!(parsed, 17 | 33) {
                        return Err(CubeLutError::UnsupportedSize(parsed));
                    }
                    size = Some(parsed);
                    table.reserve(parsed as usize * parsed as usize * parsed as usize);
                }
                "DOMAIN_MIN" => {
                    if domain_min.is_some() {
                        return Err(CubeLutError::DuplicateDirective {
                            directive: "DOMAIN_MIN",
                        });
                    }
                    domain_min = Some(parse_triplet(&fields, line_number, "DOMAIN_MIN")?);
                }
                "DOMAIN_MAX" => {
                    if domain_max.is_some() {
                        return Err(CubeLutError::DuplicateDirective {
                            directive: "DOMAIN_MAX",
                        });
                    }
                    domain_max = Some(parse_triplet(&fields, line_number, "DOMAIN_MAX")?);
                }
                directive if directive.as_bytes()[0].is_ascii_alphabetic() => {
                    return Err(CubeLutError::UnsupportedDirective {
                        line: line_number,
                        directive: directive.to_owned(),
                    });
                }
                _ => {
                    if size.is_none() {
                        return Err(CubeLutError::TableBeforeSize { line: line_number });
                    }
                    let value = parse_triplet(&fields, line_number, "table row")?;
                    if value.iter().any(|channel| channel.abs() > 16.0) {
                        return Err(CubeLutError::OutOfRangeValue { line: line_number });
                    }
                    table.push(value);
                }
            }
        }

        let size = size.ok_or(CubeLutError::MissingSize)?;
        let expected = size as usize * size as usize * size as usize;
        if table.len() != expected {
            return Err(CubeLutError::WrongTableLength {
                expected,
                actual: table.len(),
            });
        }
        let domain_min = domain_min.unwrap_or([0.0; 3]);
        let domain_max = domain_max.unwrap_or([1.0; 3]);
        if (0..3).any(|channel| domain_min[channel] >= domain_max[channel]) {
            return Err(CubeLutError::InvalidDomain);
        }
        Ok(Self {
            title,
            size,
            domain_min,
            domain_max,
            table,
        })
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn domain_min(&self) -> [f32; 3] {
        self.domain_min
    }

    pub fn domain_max(&self) -> [f32; 3] {
        self.domain_max
    }

    pub fn table(&self) -> &[[f32; 3]] {
        &self.table
    }
}

fn parse_triplet(
    fields: &[&str],
    line: usize,
    directive: &'static str,
) -> Result<[f32; 3], CubeLutError> {
    if fields.len() != 4 && directive != "table row"
        || fields.len() != 3 && directive == "table row"
    {
        return Err(CubeLutError::InvalidDirective { line, directive });
    }
    let offset = usize::from(directive != "table row");
    let mut value = [0.0; 3];
    for channel in 0..3 {
        value[channel] = fields[channel + offset]
            .parse::<f32>()
            .map_err(|_| CubeLutError::InvalidNumber { line })?;
        if !value[channel].is_finite() {
            return Err(CubeLutError::InvalidNumber { line });
        }
    }
    Ok(value)
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CubeLutError {
    TooLarge {
        actual: usize,
        maximum: usize,
    },
    InvalidUtf8,
    DuplicateDirective {
        directive: &'static str,
    },
    InvalidDirective {
        line: usize,
        directive: &'static str,
    },
    UnsupportedDirective {
        line: usize,
        directive: String,
    },
    UnsupportedSize(u32),
    TableBeforeSize {
        line: usize,
    },
    MissingSize,
    InvalidNumber {
        line: usize,
    },
    OutOfRangeValue {
        line: usize,
    },
    InvalidDomain,
    WrongTableLength {
        expected: usize,
        actual: usize,
    },
}

impl std::fmt::Display for CubeLutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { actual, maximum } => {
                write!(formatter, "LUT is {actual} bytes; maximum is {maximum}")
            }
            Self::InvalidUtf8 => formatter.write_str("LUT is not valid UTF-8 text"),
            Self::DuplicateDirective { directive } => {
                write!(formatter, "duplicate {directive} directive")
            }
            Self::InvalidDirective { line, directive } => {
                write!(formatter, "invalid {directive} on line {line}")
            }
            Self::UnsupportedDirective { line, directive } => write!(
                formatter,
                "unsupported directive {directive} on line {line}"
            ),
            Self::UnsupportedSize(size) => {
                write!(formatter, "unsupported LUT size {size}; expected 17 or 33")
            }
            Self::TableBeforeSize { line } => {
                write!(formatter, "table row before LUT_3D_SIZE on line {line}")
            }
            Self::MissingSize => formatter.write_str("missing LUT_3D_SIZE"),
            Self::InvalidNumber { line } => {
                write!(formatter, "invalid finite number on line {line}")
            }
            Self::OutOfRangeValue { line } => {
                write!(formatter, "table value outside [-16, 16] on line {line}")
            }
            Self::InvalidDomain => {
                formatter.write_str("each DOMAIN_MIN channel must be less than DOMAIN_MAX")
            }
            Self::WrongTableLength { expected, actual } => write!(
                formatter,
                "LUT table has {actual} rows; expected {expected}"
            ),
        }
    }
}

impl std::error::Error for CubeLutError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicate_metadata_and_non_finite_domain() {
        let duplicate = b"LUT_3D_SIZE 17\nLUT_3D_SIZE 17\n";
        assert!(matches!(
            CubeLut::parse(duplicate),
            Err(CubeLutError::DuplicateDirective { .. })
        ));
        let non_finite = b"LUT_3D_SIZE 17\nDOMAIN_MIN NaN 0 0\n";
        assert!(matches!(
            CubeLut::parse(non_finite),
            Err(CubeLutError::InvalidNumber { .. })
        ));
    }
}
