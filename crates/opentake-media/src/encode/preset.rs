//! Export presets — the codec/resolution → ffmpeg-args mapping consumed by the
//! encoder. Mirrors upstream `ExportService` preset selection
//! (`docs/_analysis/02` §1.3). `opentake-render` owns the wgpu frame compositing
//! and the even-size decision; this crate only encodes already-even RGBA frames.

/// Output video codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    H265,
    ProRes422,
    /// ProRes 4444 with an alpha plane for local generated derivatives.
    ProRes4444,
}

/// Short-edge target resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportResolution {
    P720,
    P1080,
    P2160,
}

impl ExportResolution {
    /// The short-edge pixel count.
    pub fn short_edge(self) -> u32 {
        match self {
            ExportResolution::P720 => 720,
            ExportResolution::P1080 => 1080,
            ExportResolution::P2160 => 2160,
        }
    }
}

/// An export preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportPreset {
    pub codec: VideoCodec,
    pub resolution: ExportResolution,
}

impl ExportPreset {
    pub fn new(codec: VideoCodec, resolution: ExportResolution) -> Self {
        ExportPreset { codec, resolution }
    }

    /// ffmpeg `-c:v` codec token.
    pub fn vcodec_arg(&self) -> &'static str {
        match self.codec {
            VideoCodec::H264 => "libx264",
            VideoCodec::H265 => "libx265",
            VideoCodec::ProRes422 => "prores_ks",
            VideoCodec::ProRes4444 => "prores_ks",
        }
    }

    /// ffmpeg `-c:a` audio codec token. ProRes pairs with LPCM; H.264/H.265 use
    /// AAC (upstream presets).
    pub fn acodec_arg(&self) -> &'static str {
        match self.codec {
            VideoCodec::ProRes422 | VideoCodec::ProRes4444 => "pcm_s16le",
            _ => "aac",
        }
    }

    /// Output pixel format. ProRes uses a 10-bit 422 format; H.264/H.265 use
    /// yuv420p for broad compatibility.
    pub fn pix_fmt_arg(&self) -> &'static str {
        match self.codec {
            VideoCodec::ProRes422 => "yuv422p10le",
            VideoCodec::ProRes4444 => "yuva444p10le",
            _ => "yuv420p",
        }
    }

    /// BT.709 delivery tagging. `setparams` writes all three properties onto
    /// every frame before the encoder sees it; the stream flags are retained as
    /// an explicit container/codec request. This combination is required by
    /// current FFmpeg/libx264, where stream flags alone leave primaries and
    /// transfer as `unknown` in the produced bitstream.
    pub fn color_args(&self) -> Vec<String> {
        vec![
            "-vf".into(),
            "setparams=color_primaries=bt709:color_trc=bt709:colorspace=bt709".into(),
            "-colorspace".into(),
            "bt709".into(),
            "-color_primaries".into(),
            "bt709".into(),
            "-color_trc".into(),
            "bt709".into(),
        ]
    }
}

/// Round a dimension down to the nearest non-zero even value: `max(2, n - n%2)`.
/// Verbatim port of `ImageVideoGenerator.encoderDimension` (`:68-72`) /
/// `TimelineRenderer.even` (`:85`). The render layer applies this before calling
/// the encoder; exposed here for parity tests and as a guard.
pub fn even_dimension(n: u32) -> u32 {
    (n - n % 2).max(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_short_edges() {
        assert_eq!(ExportResolution::P720.short_edge(), 720);
        assert_eq!(ExportResolution::P1080.short_edge(), 1080);
        assert_eq!(ExportResolution::P2160.short_edge(), 2160);
    }

    #[test]
    fn codec_tokens() {
        let h264 = ExportPreset::new(VideoCodec::H264, ExportResolution::P1080);
        assert_eq!(h264.vcodec_arg(), "libx264");
        assert_eq!(h264.acodec_arg(), "aac");
        assert_eq!(h264.pix_fmt_arg(), "yuv420p");

        let prores = ExportPreset::new(VideoCodec::ProRes422, ExportResolution::P2160);
        assert_eq!(prores.vcodec_arg(), "prores_ks");
        assert_eq!(prores.acodec_arg(), "pcm_s16le"); // LPCM
        assert_eq!(prores.pix_fmt_arg(), "yuv422p10le");

        let alpha = ExportPreset::new(VideoCodec::ProRes4444, ExportResolution::P1080);
        assert_eq!(alpha.vcodec_arg(), "prores_ks");
        assert_eq!(alpha.acodec_arg(), "pcm_s16le");
        assert_eq!(alpha.pix_fmt_arg(), "yuva444p10le");
    }

    #[test]
    fn every_delivery_codec_gets_bt709_frame_and_stream_tags() {
        let h265 = ExportPreset::new(VideoCodec::H265, ExportResolution::P720);
        let args = h265.color_args();
        assert!(args.windows(2).any(|w| w == ["-colorspace", "bt709"]));
        assert!(args.windows(2).any(|w| w == ["-color_primaries", "bt709"]));
        assert!(args.windows(2).any(|w| w == ["-color_trc", "bt709"]));

        let prores = ExportPreset::new(VideoCodec::ProRes422, ExportResolution::P720);
        assert!(prores
            .color_args()
            .iter()
            .any(|arg| arg.contains("setparams=color_primaries=bt709")));
    }

    #[test]
    fn even_dimension_rounds_down_to_even() {
        assert_eq!(even_dimension(1920), 1920);
        assert_eq!(even_dimension(1921), 1920);
        assert_eq!(even_dimension(1), 2); // min 2
        assert_eq!(even_dimension(0), 2);
        assert_eq!(even_dimension(3), 2);
        assert_eq!(even_dimension(101), 100);
    }
}
