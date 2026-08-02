//! Explicit source-color policy for the current SDR compositor.
//!
//! OpenTake retains the source signalling in the media manifest. PQ/HLG video
//! is converted to BT.709 before it becomes RGBA8 so seek-preview, continuous
//! playback and export all see the same display-referred pixels. This is an SDR
//! delivery policy, not an HDR passthrough claim.

use opentake_domain::MediaColorMetadata;
use std::process::Command;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HdrDecodeBackend {
    VideoToolbox,
    Zscale,
    Unsupported,
}

fn reports_filter(output: &str, expected: &str) -> bool {
    output.lines().any(|line| {
        let mut fields = line.split_whitespace();
        let _flags = fields.next();
        fields.next() == Some(expected)
    })
}

fn backend_from_filter_listing(output: &str) -> HdrDecodeBackend {
    if cfg!(target_os = "macos") && reports_filter(output, "scale_vt") {
        HdrDecodeBackend::VideoToolbox
    } else if reports_filter(output, "zscale") {
        HdrDecodeBackend::Zscale
    } else {
        HdrDecodeBackend::Unsupported
    }
}

fn hdr_decode_backend() -> HdrDecodeBackend {
    static BACKEND: OnceLock<HdrDecodeBackend> = OnceLock::new();
    *BACKEND.get_or_init(|| {
        let output = Command::new(crate::ff::ffmpeg_path())
            .args(["-hide_banner", "-filters"])
            .output();
        let Ok(output) = output else {
            return HdrDecodeBackend::Unsupported;
        };
        let mut listing = String::from_utf8_lossy(&output.stdout).into_owned();
        listing.push_str(&String::from_utf8_lossy(&output.stderr));
        backend_from_filter_listing(&listing)
    })
}

/// FFmpeg filter chain for an HDR source entering the SDR RGBA compositor.
/// Tokens are selected from a fixed allowlist; untrusted probe strings are
/// never interpolated into a filter expression.
pub fn hdr_tonemap_filter(color: &MediaColorMetadata) -> Option<String> {
    let transfer = color.transfer.as_deref()?.to_ascii_lowercase();
    let input_transfer = match transfer.as_str() {
        "smpte2084" | "pq" => "smpte2084",
        "arib-std-b67" | "hlg" => "arib-std-b67",
        _ => return None,
    };
    match hdr_decode_backend() {
        HdrDecodeBackend::VideoToolbox => {
            // When the active macOS FFmpeg exposes scale_vt, VideoToolbox
            // performs the metadata-driven EDR→SDR conversion in the hardware
            // scaler. p010le is the supported hwdownload bridge before the
            // ordinary software RGBA pipeline resumes.
            Some(
                "scale_vt=w=iw:h=ih:color_matrix=bt709:color_primaries=bt709:color_transfer=bt709,hwdownload,format=p010le"
                    .to_string(),
            )
        }
        HdrDecodeBackend::Zscale => Some(format!(
            "zscale=pin=bt2020:tin={input_transfer}:min=bt2020nc:rin=limited:t=linear:npl=100,format=gbrpf32le,tonemap=mobius:param=0.3:desat=2,zscale=p=bt709:t=bt709:m=bt709:r=limited"
        )),
        HdrDecodeBackend::Unsupported => None,
    }
}

/// Decoder input arguments required by the platform HDR conversion path.
pub fn hdr_decode_input_args(color: &MediaColorMetadata) -> Vec<String> {
    if color.is_hdr() && hdr_decode_backend() == HdrDecodeBackend::VideoToolbox {
        vec![
            "-hwaccel".into(),
            "videotoolbox".into(),
            "-hwaccel_output_format".into(),
            "videotoolbox_vld".into(),
        ]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_zscale_listing_never_selects_unavailable_videotoolbox_filter() {
        let listing = " .S. tonemap V->V Conversion\n .SC zscale V->V Apply resizing";
        assert_eq!(
            backend_from_filter_listing(listing),
            HdrDecodeBackend::Zscale
        );
    }

    #[test]
    fn missing_hdr_filters_are_reported_as_unsupported() {
        assert_eq!(
            backend_from_filter_listing(" .. scale V->V Scale video"),
            HdrDecodeBackend::Unsupported
        );
    }
}
