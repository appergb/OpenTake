/**
 * Preview (SPEC §8). Tab bar + aspect-fit canvas area + scrub bar + transport
 * bar with project-setting badges. Transport drives the local playhead.
 */

import { useEffect, useRef, useState } from "react";
import {
  SkipBack,
  SkipForward,
  StepBack,
  StepForward,
  Play,
  Pause,
  Camera,
} from "lucide-react";
import { PanelHeaderBar } from "../ui/PanelShell";
import { HoverButton } from "../ui/HoverButton";
import { Icon } from "../ui/Icon";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import { useMediaStore } from "../../store/mediaStore";
import { formatTimecode, totalFrames } from "../../lib/geometry";
import { assetUrl } from "../../lib/asset";
import { TimelinePlayback } from "./TimelinePlaybackLayer";
import { TransformOverlay } from "./TransformOverlay";
import { aspectFitBox, timelinePreviewCanvasStyle } from "./previewLayerStyles";
import { useT } from "../../i18n";
import {
  compositeFrame,
  getPreviewEndpoint,
  isTauri,
  type CompositeFrame,
} from "../../lib/api";
import { rustEngineEnabled } from "./rustEngine";
import { shouldUseRustEngine } from "./timelinePlayback";
import { findSelectedVisualClip, mediaCanvasAspect } from "../../lib/clip";
import type { MediaItem } from "../../lib/types";

export function Preview() {
  const t = useT();
  const timeline = useProjectStore((s) => s.timeline);
  const activeFrame = useEditorUiStore((s) => s.activeFrame);
  const setCurrentFrame = useEditorUiStore((s) => s.setCurrentFrame);
  const isPlaying = useEditorUiStore((s) => s.isPlaying);
  const setScrubbing = useEditorUiStore((s) => s.setScrubbing);
  const togglePlayTimeline = useEditorUiStore((s) => s.togglePlay);
  const previewMediaId = useEditorUiStore((s) => s.previewMediaId);
  const selectedClipIds = useEditorUiStore((s) => s.selectedClipIds);
  const pushToast = useEditorUiStore((s) => s.pushToast);
  const previewItem = useMediaStore((s) =>
    previewMediaId ? s.items.find((m) => m.id === previewMediaId) ?? null : null,
  );
  // The Transform overlay's target clip + media aspect (Inspector.tsx:295-301's
  // same mediaCanvasAspect lookup pattern, reused so both surfaces agree on
  // aspect-preserving resize). `transformClip` is null whenever upstream's
  // TransformOverlayView.selectedClip would also be nil (see clip.ts's
  // findSelectedVisualClip doc comment) — resolved unconditionally here (cheap)
  // and gated at render time alongside the timeline-tab / has-content checks.
  const transformClip = findSelectedVisualClip(timeline, selectedClipIds);
  const transformMediaItem = useMediaStore((s) =>
    transformClip ? s.items.find((m) => m.id === transformClip.mediaRef) ?? null : null,
  );
  const transformMediaAspect = mediaCanvasAspect(
    transformMediaItem?.width,
    transformMediaItem?.height,
    timeline.width,
    timeline.height,
  );

  // Media-preview playback is driven by the app transport (more capable than the
  // <video>'s native controls), so the <video>/<audio> renders WITHOUT controls
  // and this ref + state mirror its time/duration into the shared transport.
  const mediaRef = useRef<HTMLMediaElement | null>(null);
  const [mediaTime, setMediaTime] = useState(0);
  const [mediaDuration, setMediaDuration] = useState(0);
  const [mediaPlaying, setMediaPlaying] = useState(false);
  const stageRef = useRef<HTMLDivElement | null>(null);
  const [stageSize, setStageSize] = useState({ width: 0, height: 0 });
  useEffect(() => {
    setMediaTime(0);
    setMediaDuration(0);
    setMediaPlaying(false);
  }, [previewMediaId]);
  useEffect(() => {
    const el = stageRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(([entry]) => {
      if (!entry) return;
      const { width, height } = entry.contentRect;
      setStageSize((prev) =>
        Math.abs(prev.width - width) < 0.5 && Math.abs(prev.height - height) < 0.5
          ? prev
          : { width, height },
      );
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Space bar during media preview → toggle the media element.
  const mediaToggleCount = useEditorUiStore((s) => s.mediaPreviewToggleRequest);
  useEffect(() => {
    if (mediaToggleCount > 0 && previewing) {
      togglePlay();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mediaToggleCount]);

  const previewing = previewItem !== null;
  const timelineHasContent = !previewing && timeline.tracks.length > 0;
  const fps = timeline.fps;
  const total = previewing
    ? Math.max(0, Math.round(mediaDuration * fps))
    : totalFrames(timeline);
  const activeShownFrame = previewing ? Math.round(mediaTime * fps) : activeFrame;
  const playing = previewing ? mediaPlaying : isPlaying;

  const seekTo = (frame: number) => {
    const clamped = Math.max(0, Math.min(total, frame));
    if (previewing) {
      if (mediaRef.current) mediaRef.current.currentTime = clamped / fps;
    } else {
      setCurrentFrame(clamped);
    }
  };

  const togglePlay = () => {
    if (previewing) {
      const el = mediaRef.current;
      if (!el) return;
      if (el.paused) void el.play();
      else el.pause();
    } else {
      // Rewinds from the parked end frame on replay (see store togglePlay).
      togglePlayTimeline();
    }
  };

  const captureTimelineFrame = async () => {
    if (previewing || !timelineHasContent) return;
    const frame = Math.max(0, Math.floor(activeFrame));
    try {
      const image: CompositeFrame | null = await compositeFrame(frame, 0);
      if (!image) {
        pushToast(t("preview.captureFrameUnavailable"));
        return;
      }
      downloadDataUrl(image.dataUrl, `opentake-frame-${String(frame).padStart(6, "0")}.png`);
      pushToast(t("preview.captureFrameSaved"));
    } catch (error) {
      console.warn("capture frame failed:", error);
      pushToast(t("preview.captureFrameFailed"));
    }
  };

  const fittedCanvas = aspectFitBox(stageSize.width, stageSize.height, timeline.width, timeline.height);
  const timelineCanvasStyle = {
    ...timelinePreviewCanvasStyle(timeline.width, timeline.height),
    ...(fittedCanvas
      ? { width: fittedCanvas.width, height: fittedCanvas.height, flex: "0 0 auto" }
      : {}),
  };

  return (
    <>
      <PanelHeaderBar>
        <PreviewTabs item={previewItem} />
      </PanelHeaderBar>

      {/* Canvas stage: a flex-centered area; the media inside aspect-fits via
          intrinsic size + max-width/height, so it always fills the largest 16:9
          box and stays centered. */}
      <div
        ref={stageRef}
        style={{
          flex: 1,
          minHeight: 0,
          background: "var(--bg-surface)",
          position: "relative",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          overflow: "hidden",
          padding: 8,
        }}
      >
        {previewItem ? (
          <MediaPreview
            item={previewItem}
            mediaRef={mediaRef}
            onTime={setMediaTime}
            onDuration={setMediaDuration}
            onPlayingChange={setMediaPlaying}
          />
        ) : (
          <div style={{ ...timelineCanvasStyle, position: "relative" }}>
            {timelineHasContent ? (
              <>
                <TimelinePlayback timeline={timeline} fps={fps} />
                <TimelineRustOverlay />
                {transformClip && fittedCanvas && (
                  <TransformOverlay
                    clip={transformClip}
                    canvasPx={fittedCanvas}
                    mediaAspect={transformMediaAspect}
                  />
                )}
              </>
            ) : (
              // Empty timeline: a framed 16:9 canvas surface placeholder.
              <div
                style={{
                  width: "100%",
                  height: "100%",
                  border: "1px solid rgba(255,255,255,0.08)",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  color: "var(--text-muted)",
                  fontSize: "var(--fs-xs)",
                }}
              >
                {t("preview.noMedia")}
              </div>
            )}
          </div>
        )}
      </div>

      {/* The app's scrub + transport are the single control surface — they drive
          both the timeline composite and (via mediaRef) single-media preview, so
          the <video>/<audio> renders without its native controls. */}
      <ScrubBar
        frame={activeShownFrame}
        total={total}
        onSeek={seekTo}
        onScrubbingChange={previewing ? undefined : setScrubbing}
      />

      {/* Transport bar */}
      <div
        style={{
          height: 36,
          flex: "0 0 auto",
          display: "flex",
          alignItems: "center",
          gap: "var(--space-sm)",
          padding: "0 var(--space-sm)",
          background: "var(--bg-surface)",
          borderTop: "var(--bw-thin) solid var(--border-primary)",
        }}
      >
        <span className="tabular" style={{ fontSize: "var(--fs-xs)", color: "var(--accent-timecode)" }}>
          {formatTimecode(activeShownFrame, fps)} / {formatTimecode(total, fps)}
        </span>
        <div style={{ flex: 1 }} />
        <div style={{ display: "flex", alignItems: "center", gap: "var(--space-md)" }}>
          <HoverButton title={t("preview.jumpStart")} onClick={() => seekTo(0)}>
            <Icon icon={SkipBack} size={13} />
          </HoverButton>
          <HoverButton title={t("preview.stepBack")} onClick={() => seekTo(activeShownFrame - 1)}>
            <Icon icon={StepBack} size={13} />
          </HoverButton>
          <HoverButton title={t("preview.playPause")} onClick={togglePlay}>
            <Icon icon={playing ? Pause : Play} size={14} />
          </HoverButton>
          <HoverButton title={t("preview.stepForward")} onClick={() => seekTo(activeShownFrame + 1)}>
            <Icon icon={StepForward} size={13} />
          </HoverButton>
          <HoverButton title={t("preview.jumpEnd")} onClick={() => seekTo(total)}>
            <Icon icon={SkipForward} size={13} />
          </HoverButton>
        </div>
        <div style={{ flex: 1 }} />
        <HoverButton
          title={t("preview.captureFrame")}
          disabled={previewing || !timelineHasContent}
          onClick={() => void captureTimelineFrame()}
        >
          <Icon icon={Camera} size={13} />
        </HoverButton>
        <ProjectSettingsBadges fps={timeline.fps} width={timeline.width} height={timeline.height} />
      </div>
    </>
  );
}

function downloadDataUrl(dataUrl: string, filename: string): void {
  const link = document.createElement("a");
  link.href = dataUrl;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
}

/**
 * The Rust streaming-playback surface: an `<img>` pointed at the loopback MJPEG
 * stream, shown ONLY during PLAY when the Rust engine flag is on (under Tauri).
 * It overlays `<TimelinePlayback>` (whose `<video>` elements are paused during
 * Rust play) and fills the aspect-fit canvas. Scrub/pause unmount it, so the
 * legacy `<video>`/composite surface shows again. No-op (renders nothing) outside
 * Tauri or with the flag off — the legacy path is untouched.
 */
function TimelineRustOverlay() {
  const isPlaying = useEditorUiStore((s) => s.isPlaying);
  const isScrubbing = useEditorUiStore((s) => s.isScrubbing);
  const [endpoint, setEndpoint] = useState<string | null>(null);

  useEffect(() => {
    if (!rustEngineEnabled() || !isTauri) return;
    let cancelled = false;
    void getPreviewEndpoint().then((url) => {
      // Guard against a null/undefined endpoint leaking into state (which would
      // otherwise activate the overlay with a broken <img>).
      if (!cancelled && typeof url === "string") setEndpoint(url);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const active =
    shouldUseRustEngine({ rustEnabled: rustEngineEnabled(), isTauri, isPlaying, isScrubbing }) &&
    endpoint !== null;
  if (!active) return null;
  return (
    <img
      src={endpoint ?? undefined}
      alt=""
      draggable={false}
      style={{
        position: "absolute",
        inset: 0,
        width: "100%",
        height: "100%",
        objectFit: "fill",
        pointerEvents: "none",
        zIndex: 2,
      }}
    />
  );
}

/** Renders a single media asset straight from disk via the asset protocol —
 *  `<video>`/`<audio>` (NO native controls; the app transport drives them via
 *  `mediaRef`), `<img>` for stills. The pragmatic preview path (WebView decodes
 *  the original file); timeline composite preview is a later batch. */
function MediaPreview({
  item,
  mediaRef,
  onTime,
  onDuration,
  onPlayingChange,
}: {
  item: MediaItem;
  mediaRef: React.MutableRefObject<HTMLMediaElement | null>;
  onTime: (time: number) => void;
  onDuration: (duration: number) => void;
  onPlayingChange: (playing: boolean) => void;
}) {
  const t = useT();
  const url = assetUrl(item.path);
  const box: React.CSSProperties = {
    maxWidth: "100%",
    maxHeight: "100%",
    objectFit: "contain",
    display: "block",
    pointerEvents: "none",
  };

  if (!url) {
    return <span>{t("preview.unavailable")}</span>;
  }
  if (item.type === "image") {
    return <img src={url} alt={item.name} draggable={false} style={box} />;
  }
  if (item.type === "audio") {
    return (
      <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: "var(--space-md)", padding: "var(--space-xl)" }}>
        <Icon icon={Play} size={28} />
        <audio
          ref={(el) => {
            mediaRef.current = el;
          }}
          src={url}
          onTimeUpdate={(e) => onTime(e.currentTarget.currentTime)}
          onLoadedMetadata={(e) => onDuration(e.currentTarget.duration || 0)}
          onDurationChange={(e) => onDuration(e.currentTarget.duration || 0)}
          onPlay={() => onPlayingChange(true)}
          onPause={() => onPlayingChange(false)}
          onEnded={() => onPlayingChange(false)}
          style={{ width: "80%" }}
        />
      </div>
    );
  }
  // video (and any other visual): app transport drives it (no native controls).
  return (
    <video
      ref={(el) => {
        mediaRef.current = el;
      }}
      src={url}
      playsInline
      onTimeUpdate={(e) => onTime(e.currentTarget.currentTime)}
      onLoadedMetadata={(e) => onDuration(e.currentTarget.duration || 0)}
      onDurationChange={(e) => onDuration(e.currentTarget.duration || 0)}
      onPlay={() => onPlayingChange(true)}
      onPause={() => onPlayingChange(false)}
      onEnded={() => onPlayingChange(false)}
      style={box}
    />
  );
}

function PreviewTabs({ item }: { item: MediaItem | null }) {
  const t = useT();
  const setPreviewMedia = useEditorUiStore((s) => s.setPreviewMedia);
  const onTimeline = item === null;
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "var(--space-md)" }}>
      <button
        type="button"
        onClick={() => setPreviewMedia(null)}
        style={{
          paddingBottom: 4,
          fontSize: "var(--fs-sm-md)",
          fontWeight: "var(--fw-semibold)",
          color: onTimeline ? "var(--text-primary)" : "var(--text-tertiary)",
          borderBottom: onTimeline ? "var(--bw-medium) solid var(--accent-primary)" : "none",
        }}
      >
        {t("preview.timelineTab")}
      </button>
      {item && (
        <div
          style={{
            paddingBottom: 4,
            maxWidth: 180,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            fontSize: "var(--fs-sm-md)",
            fontWeight: "var(--fw-semibold)",
            color: "var(--text-primary)",
            borderBottom: "var(--bw-medium) solid var(--accent-primary)",
          }}
        >
          {item.name}
        </div>
      )}
    </div>
  );
}

function ScrubBar({
  frame,
  total,
  onSeek,
  onScrubbingChange,
}: {
  frame: number;
  total: number;
  onSeek: (f: number) => void;
  /** Toggled while the user drags the bar, so the engine drives the live
   *  <video> scrub (issue #142) and the GPU composite stays settled-only. */
  onScrubbingChange?: (scrubbing: boolean) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState(false);
  const progress = total > 0 ? frame / total : 0;

  const seekFromEvent = (clientX: number) => {
    const el = ref.current;
    if (!el || total <= 0) return;
    const rect = el.getBoundingClientRect();
    const t = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    onSeek(Math.round(t * total));
  };

  return (
    <div
      ref={ref}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      onPointerDown={(e) => {
        (e.target as HTMLElement).setPointerCapture(e.pointerId);
        onScrubbingChange?.(true);
        seekFromEvent(e.clientX);
      }}
      onPointerMove={(e) => {
        if (e.buttons === 1) seekFromEvent(e.clientX);
      }}
      onPointerUp={() => onScrubbingChange?.(false)}
      onLostPointerCapture={() => onScrubbingChange?.(false)}
      style={{
        height: 18,
        flex: "0 0 auto",
        display: "flex",
        alignItems: "center",
        padding: "0 var(--space-sm)",
        background: "var(--bg-surface)",
        cursor: "pointer",
      }}
    >
      <div
        style={{
          // position:relative confines the absolute progress fill + handle below.
          // Without it they escape to the nearest positioned ancestor (the preview
          // panel) and render as a tall cream bar down the left edge.
          position: "relative",
          flex: 1,
          height: hover ? 4 : 3,
          background: "rgba(255,255,255,0.1)",
          borderRadius: 2,
        }}
      >
        <div
          style={{
            position: "absolute",
            left: 0,
            top: 0,
            bottom: 0,
            width: `${progress * 100}%`,
            background: "var(--accent-primary)",
            borderRadius: 2,
          }}
        />
        <div
          style={{
            position: "absolute",
            left: `${progress * 100}%`,
            top: "50%",
            transform: "translate(-50%, -50%)",
            width: hover ? 10 : 6,
            height: hover ? 10 : 6,
            borderRadius: "50%",
            background: "var(--accent-primary)",
          }}
        />
      </div>
    </div>
  );
}

function Badge({ children }: { children: React.ReactNode }) {
  return (
    <span
      style={{
        fontSize: "var(--fs-xxs)",
        fontWeight: "var(--fw-bold)",
        color: "var(--text-secondary)",
        height: "var(--icon-md-lg)",
        display: "inline-flex",
        alignItems: "center",
        padding: "0 var(--space-sm)",
        borderRadius: "var(--radius-xs-sm)",
      }}
      className="hover-area tabular"
    >
      {children}
    </span>
  );
}

function ProjectSettingsBadges({ fps, width, height }: { fps: number; width: number; height: number }) {
  const t = useT();
  const g = gcd(width, height) || 1;
  const quality = height >= 2160 ? "4K" : height >= 1440 ? "2K" : height >= 1080 ? "FHD" : "HD";
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "var(--space-xs)" }}>
      <Badge>{`${width / g}:${height / g}`}</Badge>
      <Badge>{fps}</Badge>
      <Badge>{quality}</Badge>
      <Badge>{t("preview.fit")}</Badge>
    </div>
  );
}

function gcd(a: number, b: number): number {
  return b === 0 ? a : gcd(b, a % b);
}
