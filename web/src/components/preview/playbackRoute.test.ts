import { describe, expect, it } from "vitest";
import type { Clip, ClipType, Timeline, Track } from "../../lib/types";
import {
  isRetryableRustPlaybackFailure,
  resolveTimelinePlaybackRoute,
} from "./playbackRoute";

const runtime = { rustAvailable: true, rustEnabled: true };

function clip(overrides: Partial<Clip> = {}): Clip {
  return {
    id: "clip-1",
    mediaRef: "media-1",
    mediaType: "video",
    sourceClipType: "video",
    startFrame: 0,
    durationFrames: 90,
    trimStartFrame: 0,
    trimEndFrame: 0,
    speed: 1,
    volume: 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: "linear",
    fadeOutInterpolation: "linear",
    opacity: 1,
    transform: {
      centerX: 0.5,
      centerY: 0.5,
      width: 1,
      height: 1,
      rotation: 0,
      flipHorizontal: false,
      flipVertical: false,
    },
    crop: { left: 0, top: 0, right: 0, bottom: 0 },
    ...overrides,
  };
}

function track(type: ClipType, clips: Clip[]): Track {
  return {
    id: `track-${type}-${clips[0]?.id ?? "empty"}`,
    type,
    muted: false,
    hidden: false,
    syncLocked: true,
    clips,
  };
}

function timeline(...clips: Clip[]): Timeline {
  return {
    fps: 30,
    width: 1920,
    height: 1080,
    settingsConfigured: true,
    tracks: clips.map((item) => track(item.mediaType, [item])),
  };
}

function reasonCodes(result: ReturnType<typeof resolveTimelinePlaybackRoute>): string[] {
  return result.reasons.map((reason) => reason.code);
}

describe("resolveTimelinePlaybackRoute", () => {
  it("routes plain forward video to WebKit", () => {
    const result = resolveTimelinePlaybackRoute(
      timeline(
        clip({ id: "video", mediaType: "video" }),
        clip({ id: "image", mediaType: "image", sourceClipType: "image" }),
        clip({ id: "audio", mediaType: "audio", sourceClipType: "audio" }),
      ),
      runtime,
    );
    expect(result).toEqual({ kind: "webkit", reasons: [] });
  });

  it("routes two ordinary video tracks through the native compositor", () => {
    const result = resolveTimelinePlaybackRoute(
      timeline(
        clip({ id: "upper-video", mediaType: "video" }),
        clip({ id: "lower-video", mediaType: "video" }),
      ),
      runtime,
    );

    expect(result).toEqual({ kind: "rust", reasons: [] });
  });

  it("routes temporally remapped video stacks through the native compositor", () => {
    const result = resolveTimelinePlaybackRoute(
      timeline(
        clip({ id: "upper-video", mediaType: "video", speed: 1.5 }),
        clip({ id: "lower-video", mediaType: "video" }),
      ),
      runtime,
    );

    expect(result).toEqual({ kind: "rust", reasons: [] });
  });

  it("retries a failed WebKit video revision through the native decoder", () => {
    expect(
      resolveTimelinePlaybackRoute(timeline(clip()), {
        ...runtime,
        forceRust: true,
      }),
    ).toEqual({ kind: "rust", reasons: [] });
  });

  it("keeps failed WebKit video on WebKit when native playback is unavailable", () => {
    expect(
      resolveTimelinePlaybackRoute(timeline(clip()), {
        rustAvailable: false,
        rustEnabled: true,
        forceRust: true,
      }),
    ).toEqual({ kind: "webkit", reasons: [] });
  });

  it("routes reverse only and speed only timelines to WebKit", () => {
    expect(
      resolveTimelinePlaybackRoute(timeline(clip({ reversed: true })), runtime).kind,
    ).toBe("webkit");
    expect(
      resolveTimelinePlaybackRoute(timeline(clip({ speed: 1.75 })), runtime).kind,
    ).toBe("webkit");
    expect(
      resolveTimelinePlaybackRoute(timeline(clip({ speed: 0 })), runtime).kind,
    ).toBe("webkit");
    expect(
      resolveTimelinePlaybackRoute(
        timeline(clip({ speed: 1 + Number.EPSILON })),
        runtime,
      ).kind,
    ).toBe("webkit");
  });

  it("routes text color chroma stabilization and supported masks to Rust", () => {
    const text = clip({ id: "text", mediaType: "text", sourceClipType: "text" });
    const color = clip({
      id: "color",
      colorGrade: {
        exposure: 0.25,
        temperature: 0,
        tint: 0,
        liftGammaGain: {
          lift: { r: 0, g: 0, b: 0 },
          gamma: { r: 1, g: 1, b: 1 },
          gain: { r: 1, g: 1, b: 1 },
        },
        contrast: 1,
        saturation: 1,
      },
    });
    const chroma = clip({
      id: "chroma",
      chromaKey: {
        keyColor: { r: 0, g: 1, b: 0 },
        similarity: 0.2,
        smoothness: 0.1,
        spill: 0.1,
      },
    });
    const masks = clip({
      id: "masks",
      masks: [
        {
          shape: { kind: "circle", center: { x: 0.5, y: 0.5 }, radius: { x: 0.2, y: 0.2 } },
          feather: 0,
          invert: false,
        },
        {
          shape: { kind: "linear", point: { x: 0.5, y: 0.5 }, normal: { x: 1, y: 0 } },
          feather: 0,
          invert: false,
        },
      ],
    });
    const stabilization = clip({
      id: "stabilization",
      stabilization: {
        model: "opentake.motion-smoothing",
        modelVersion: 1,
        sourceIdentity: "media-1",
        strength: 1,
        cropMargin: 0,
        keyframes: [
          { frame: 0, translationX: 0, translationY: 0, rotationDegrees: 0 },
          { frame: 89, translationX: 0.01, translationY: 0, rotationDegrees: 0 },
        ],
      },
    });
    for (const item of [text, color, chroma, stabilization, masks]) {
      expect(resolveTimelinePlaybackRoute(timeline(item), runtime)).toEqual({
        kind: "rust",
        reasons: [],
      });
    }
  });

  it("routes temporal compositor timelines through Rust when native playback is available", () => {
    const temporalCompositorCases = [
      clip({ id: "text-reversed", mediaType: "text", sourceClipType: "text", reversed: true }),
      clip({
        id: "color-speed",
        speed: 1.5,
        colorGrade: {
          exposure: 0.25,
          temperature: 0,
          tint: 0,
          liftGammaGain: {
            lift: { r: 0, g: 0, b: 0 },
            gamma: { r: 1, g: 1, b: 1 },
            gain: { r: 1, g: 1, b: 1 },
          },
          contrast: 1,
          saturation: 1,
        },
      }),
      clip({
        id: "mask-speed",
        speed: 1.5,
        masks: [
          {
            shape: {
              kind: "circle",
              center: { x: 0.5, y: 0.5 },
              radius: { x: 0.2, y: 0.2 },
            },
            feather: 0,
            invert: false,
          },
        ],
      }),
    ];

    for (const item of temporalCompositorCases) {
      expect(resolveTimelinePlaybackRoute(timeline(item), runtime)).toEqual({
        kind: "rust",
        reasons: [],
      });
    }
  });

  it("treats rust-disabled temporal compositor routes as retryable", () => {
    const nativeStartupFailure = resolveTimelinePlaybackRoute(
      timeline(clip({ mediaType: "text", sourceClipType: "text" })),
      { rustAvailable: true, rustEnabled: false },
    );
    const temporalCompositorNativeFallback = resolveTimelinePlaybackRoute(
      timeline(
        clip({ mediaType: "text", sourceClipType: "text", reversed: true }),
      ),
      { rustAvailable: true, rustEnabled: false },
    );

    expect(isRetryableRustPlaybackFailure(nativeStartupFailure, true)).toBe(true);
    expect(isRetryableRustPlaybackFailure(nativeStartupFailure, false)).toBe(false);
    expect(
      isRetryableRustPlaybackFailure(temporalCompositorNativeFallback, true),
    ).toBe(true);
  });

  it("returns rust-unavailable for temporal compositor timelines when native playback is absent", () => {
    const rustUnavailableRuntime = { rustAvailable: false, rustEnabled: true };
    const temporalCompositorCases = [
      clip({ id: "text-reversed", mediaType: "text", sourceClipType: "text", reversed: true }),
      clip({
        id: "color-speed",
        speed: 1.5,
        colorGrade: {
          exposure: 0.25,
          temperature: 0,
          tint: 0,
          liftGammaGain: {
            lift: { r: 0, g: 0, b: 0 },
            gamma: { r: 1, g: 1, b: 1 },
            gain: { r: 1, g: 1, b: 1 },
          },
          contrast: 1,
          saturation: 1,
        },
      }),
      clip({
        id: "mask-speed",
        speed: 1.5,
        masks: [
          {
            shape: {
              kind: "circle",
              center: { x: 0.5, y: 0.5 },
              radius: { x: 0.2, y: 0.2 },
            },
            feather: 0,
            invert: false,
          },
        ],
      }),
    ];

    for (const item of temporalCompositorCases) {
      expect(resolveTimelinePlaybackRoute(timeline(item), rustUnavailableRuntime)).toEqual({
        kind: "unsupported",
        reasons: [{ code: "rust-unavailable" }],
      });
    }
  });

  it("routes polygon masks through the native compositor", () => {
    const polygon = clip({
      masks: [
        {
          shape: {
            kind: "poly",
            points: [
              { x: 0.2, y: 0.2 },
              { x: 0.8, y: 0.2 },
              { x: 0.5, y: 0.8 },
            ],
          },
          feather: 0.1,
          invert: false,
        },
      ],
    });

    expect(resolveTimelinePlaybackRoute(timeline(polygon), runtime)).toEqual({
      kind: "rust",
      reasons: [],
    });
  });

  it("routes advertised effects through Rust and rejects unknown persisted effects", () => {
    expect(
      resolveTimelinePlaybackRoute(
        timeline(clip({ effects: [{ name: "sepia", params: {}, enabled: true }] })),
        runtime,
      ),
    ).toEqual({ kind: "rust", reasons: [] });

    const cases: Array<[Clip, string]> = [
      [clip({ mediaType: "lottie", sourceClipType: "lottie" }), "lottie"],
      [clip({ effects: [{ name: "blur", params: {}, enabled: true }] }), "unknown-effect"],
      [
        clip({
          masks: Array.from({ length: 5 }, () => ({
            shape: { kind: "circle" as const, center: { x: 0.5, y: 0.5 }, radius: { x: 0.1, y: 0.1 } },
            feather: 0,
            invert: false,
          })),
        }),
        "mask-overflow",
      ],
    ];
    for (const [item, code] of cases) {
      const result = resolveTimelinePlaybackRoute(timeline(item), runtime);
      expect(result.kind).toBe("unsupported");
      expect(reasonCodes(result)).toContain(code);
    }
  });

  it("does not select an incomplete renderer because of a runtime preference", () => {
    const preferRust = { rustAvailable: true, rustEnabled: true };
    expect(
      resolveTimelinePlaybackRoute(timeline(clip({ reversed: true })), preferRust).kind,
    ).toBe("webkit");
    expect(
      resolveTimelinePlaybackRoute(
        timeline(clip({ mediaType: "lottie", sourceClipType: "lottie" })),
        preferRust,
      ).kind,
    ).toBe("unsupported");
  });

  it("returns Unsupported when Rust is unavailable and WebKit lacks parity", () => {
    const text = timeline(clip({ mediaType: "text", sourceClipType: "text" }));
    const unavailable = resolveTimelinePlaybackRoute(text, {
      rustAvailable: false,
      rustEnabled: true,
    });
    expect(unavailable.kind).toBe("unsupported");
    expect(reasonCodes(unavailable)).toContain("rust-unavailable");

    const disabled = resolveTimelinePlaybackRoute(text, {
      rustAvailable: true,
      rustEnabled: false,
    });
    expect(disabled.kind).toBe("unsupported");
    expect(reasonCodes(disabled)).toContain("rust-disabled");
  });
});
