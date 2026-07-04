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
  Check,
  ChevronDown,
} from "lucide-react";
import { PanelHeaderBar } from "../ui/PanelShell";
import { HoverButton } from "../ui/HoverButton";
import { Icon } from "../ui/Icon";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import { useMediaStore, refreshMedia } from "../../store/mediaStore";
import { formatTimecode, totalFrames } from "../../lib/geometry";
import { snapFrameToEdge } from "../../lib/snap";
import { maybeSnapFeedback } from "../../lib/haptic";
import { assetUrl } from "../../lib/asset";
import { TimelinePlayback } from "./TimelinePlaybackLayer";
import { TransformOverlay } from "./TransformOverlay";
import { CropOverlay } from "./CropOverlay";
import {
  CANVAS_OUTLINE_COLOR,
  aspectFitBox,
  timelinePreviewCanvasStyle,
} from "./previewLayerStyles";
import { useT } from "../../i18n";
import {
  captureFrameToMedia,
  getPreviewEndpoint,
  isTauri,
  previewPoster,
} from "../../lib/api";
import { rustEngineEnabled } from "./rustEngine";
import { shouldUseRustEngine } from "./timelinePlayback";
import { findCropEditingClip, findSelectedVisualClip, mediaCanvasAspect } from "../../lib/clip";
import { setTimelineSettings } from "../../store/editActions";
import { applyScrollZoom, type CanvasOffset } from "../../lib/previewZoom";
import {
  ASPECT_PRESETS,
  QUALITY_PRESETS,
  ZOOM_PRESETS,
  isAspectPresetActive,
  isQualityPresetActive,
  isZoomPresetActive,
  qualityBadgeLabel,
  zoomBadgeLabel,
  type AspectPreset,
  type QualityPreset,
  type ZoomPreset,
} from "../../lib/previewPresets";
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
  const mediaPanelCurrentFolderId = useEditorUiStore((s) => s.mediaPanelCurrentFolderId);
  // Preview canvas zoom + pan (Item 1). Read here and applied to the timeline
  // stage transform below; the scroll-zoom gesture writes them via a native
  // (non-passive) wheel listener (see the effect below).
  const canvasZoom = useEditorUiStore((s) => s.canvasZoom);
  const canvasOffset = useEditorUiStore((s) => s.canvasOffset);
  const setCanvasZoom = useEditorUiStore((s) => s.setCanvasZoom);
  const setCanvasOffset = useEditorUiStore((s) => s.setCanvasOffset);
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

  // The Crop overlay's target clip (T3-11). Mutually exclusive with the
  // Transform overlay — `PreviewContainerView.swift:37-41`'s
  // `if cropEditingActive { CropOverlayView() } else { TransformOverlayView() }`
  // — gated additionally on `cropEditingActive` at render time below.
  // `findCropEditingClip` (unlike `findSelectedVisualClip`) excludes text
  // clips and hides on an ambiguous match, per `CropOverlayView.selectedClip`'s
  // exact rule (clip.ts doc comment).
  const cropEditingActive = useEditorUiStore((s) => s.cropEditingActive);
  const cropClip = cropEditingActive ? findCropEditingClip(timeline, selectedClipIds) : null;
  const cropMediaItem = useMediaStore((s) =>
    cropClip ? s.items.find((m) => m.id === cropClip.mediaRef) ?? null : null,
  );
  // Raw SOURCE pixel aspect (sourceWidth / sourceHeight) — distinct from
  // `mediaCanvasAspect` above (which normalizes against the timeline canvas).
  // 1:1 with upstream `sourcePixelAspect(for:)` (CropOverlayView.swift:207-212).
  const cropSourcePixelAspect =
    cropMediaItem?.width && cropMediaItem?.height && cropMediaItem.height > 0
      ? cropMediaItem.width / cropMediaItem.height
      : null;

  // Media-preview playback is driven by the app transport (more capable than the
  // <video>'s native controls), so the <video>/<audio> renders WITHOUT controls
  // and this ref + state mirror its time/duration into the shared transport.
  const mediaRef = useRef<HTMLMediaElement | null>(null);
  const [mediaTime, setMediaTime] = useState(0);
  const [mediaDuration, setMediaDuration] = useState(0);
  const [mediaPlaying, setMediaPlaying] = useState(false);
  const stageRef = useRef<HTMLDivElement | null>(null);
  // The zoomed canvas box element — the wheel handler measures its rect so the
  // zoom anchors on the cursor's position within the canvas (not the padded stage).
  const canvasBoxRef = useRef<HTMLDivElement | null>(null);
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

  // Attach the scroll-zoom wheel handler natively with { passive: false } (see
  // the closure above for why). A latest-ref keeps the listener stable while
  // always running the current closure — the exact pattern TimelineContainer
  // uses for its own non-passive wheel listener.
  useEffect(() => {
    const el = stageRef.current;
    if (!el) return;
    const handler = (e: WheelEvent) => scrollZoomRef.current(e);
    el.addEventListener("wheel", handler, { passive: false });
    return () => el.removeEventListener("wheel", handler);
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
      // A single media item has no clip edges to snap to.
      if (mediaRef.current) mediaRef.current.currentTime = clamped / fps;
    } else {
      // Magnetize the scrub bar to clip start/end edges (~0.25s threshold) and
      // tick on engage, like dragging the playhead in the timeline.
      const snapped = snapFrameToEdge(timeline, clamped, Math.max(2, Math.round(fps * 0.25)));
      setCurrentFrame(snapped.frame);
      maybeSnapFeedback(snapped.snappedTo);
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

  // Whether the capture button is available: the timeline tab with content, or a
  // single-clip VIDEO preview tab (upstream shows it on both —
  // `isTimeline || activePreviewTab.clipType == .video`,
  // PreviewContainerView.swift:95).
  const canCaptureVideoTab = previewing && previewItem?.type === "video";
  const canCapture = (timelineHasContent && !previewing) || canCaptureVideoTab;

  // Capture the current frame INTO the media library as a new still (upstream
  // `captureCurrentFrameToMedia`): composite the timeline (timeline tab) or decode
  // the source asset's own frame (video tab), import it named "{nameBase} frame",
  // place it in the current media folder, then refresh the panel and toast. This
  // REPLACES the old PNG download — upstream imports, it does not save to disk.
  const captureFrame = async () => {
    if (!canCapture) return;
    const onVideoTab = canCaptureVideoTab;
    const frame = Math.max(0, Math.floor(onVideoTab ? activeShownFrame : activeFrame));
    // nameBase: "Frame" on the timeline tab, the asset's name on the video tab.
    const nameBase = onVideoTab ? (previewItem?.name ?? "Frame") : "Frame";
    const sourceMediaId = onVideoTab ? (previewItem?.id ?? null) : null;
    try {
      const list = await captureFrameToMedia(
        frame,
        nameBase,
        mediaPanelCurrentFolderId,
        sourceMediaId,
      );
      if (!list) {
        pushToast(t("preview.captureFrameUnavailable"));
        return;
      }
      // The command emits media_changed too, but refresh explicitly so the panel
      // reflects the rename/folder-move (which fire after that event) immediately.
      await refreshMedia();
      pushToast(t("preview.captureFrameSaved"));
    } catch (error) {
      console.warn("capture frame failed:", error);
      pushToast(t("preview.captureFrameFailed"));
    }
  };

  const fittedCanvas = aspectFitBox(stageSize.width, stageSize.height, timeline.width, timeline.height);
  // The zoomed canvas box: the aspect-fit box physically resized by `canvasZoom`
  // (upstream `.frame(fitSize * zoom)`, PreviewContainerView.swift:20-22,43). The
  // box stays flex-centered in the stage; `canvasOffset` then translates it
  // (upstream `.offset(canvasOffset)`, :49). Overlays receive THIS scaled box as
  // their `canvasPx` so their fraction→px placement and their pointer deltas
  // (which divide by canvasPx) both track the zoomed canvas 1:1 — the alignment
  // invariant. `null` (degenerate stage) leaves everything unzoomed.
  const scaledCanvas =
    fittedCanvas && canvasZoom > 0
      ? { width: fittedCanvas.width * canvasZoom, height: fittedCanvas.height * canvasZoom }
      : fittedCanvas;
  const canvasTransform = `translate(${canvasOffset.width}px, ${canvasOffset.height}px)`;
  const timelineCanvasStyle = {
    ...timelinePreviewCanvasStyle(timeline.width, timeline.height),
    ...(scaledCanvas
      ? {
          width: scaledCanvas.width,
          height: scaledCanvas.height,
          // Clear the base style's max-100% clamp so a zoomed-in box (bigger than
          // the stage) actually enlarges — the stage's overflow:hidden then crops
          // it, showing a magnified region (upstream's `.frame(fitSize*zoom)`
          // grows past the container and the parent `.clipped()`s it).
          maxWidth: "none",
          maxHeight: "none",
          flex: "0 0 auto",
          transform: canvasTransform,
        }
      : {}),
  };

  // Cursor-anchored scroll-to-zoom (Item 1). Mirrors upstream
  // PreviewView.swift:115-126 (Cmd+scroll only) and the anchor math in
  // :14-34. A trackpad pinch arrives as ctrl+wheel and the webview would
  // otherwise page-zoom, so — exactly like TimelineContainer's wheel handler —
  // the listener is attached natively with `{ passive: false }` (React's onWheel
  // is passive, so preventDefault there silently no-ops). Only Cmd/Ctrl+wheel
  // zooms; a bare scroll is left alone. Guarded on a live scaled canvas.
  const scrollZoomRef = useRef<(e: WheelEvent) => void>(() => {});
  scrollZoomRef.current = (e: WheelEvent) => {
    if (!(e.ctrlKey || e.metaKey)) return;
    const boxEl = canvasBoxRef.current;
    if (!boxEl || previewing || !timelineHasContent) return;
    e.preventDefault();
    const rect = boxEl.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;
    // Upstream sensitivity: 0.005 for precise (trackpad) deltas, 0.05 otherwise
    // (PreviewView.swift:122). deltaMode 0 = pixel (trackpad), 1 = line (mouse).
    const sensitivity = e.deltaMode === 0 ? 0.005 : 0.05;
    // Upstream negates: scrolling up (negative deltaY) zooms IN. Browser wheel
    // deltaY is positive scrolling down, so negate to match.
    const deltaZoom = -e.deltaY * sensitivity;
    if (deltaZoom === 0) return;
    const next = applyScrollZoom({
      oldZoom: canvasZoom,
      deltaZoom,
      pointTopDown: { x: e.clientX - rect.left, y: e.clientY - rect.top },
      viewSize: { width: rect.width, height: rect.height },
      offset: canvasOffset as CanvasOffset,
    });
    setCanvasOffset(next.offset);
    setCanvasZoom(next.zoom);
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
          <div ref={canvasBoxRef} style={{ ...timelineCanvasStyle, position: "relative" }}>
            {timelineHasContent ? (
              <>
                <TimelinePlayback timeline={timeline} fps={fps} />
                <TimelineRustOverlay />
                {/* Below-fit canvas outline (upstream PreviewContainerView.swift:
                    44-47: Rectangle stroke white @ Opacity.moderate=0.25 when
                    canvasZoom < 1.0, else invisible). pointer-events:none so it
                    never intercepts overlay drags. */}
                {canvasZoom < 1.0 && (
                  <div
                    style={{
                      position: "absolute",
                      inset: 0,
                      border: `1px solid ${CANVAS_OUTLINE_COLOR}`,
                      pointerEvents: "none",
                      zIndex: 4,
                    }}
                  />
                )}
                {/* Mutually exclusive per `PreviewContainerView.swift:37-41`: while
                    crop-editing is active, CropOverlay replaces TransformOverlay
                    entirely (even if no clip resolves for it — matching upstream's
                    unconditional `if editor.cropEditingActive` swap). Overlays get
                    the SCALED canvas box (fittedCanvas × canvasZoom) so their
                    placement + pointer math track the zoomed canvas (invariant). */}
                {cropEditingActive
                  ? cropClip &&
                    scaledCanvas && (
                      <CropOverlay
                        clip={cropClip}
                        canvasPx={scaledCanvas}
                        sourcePixelAspect={cropSourcePixelAspect}
                      />
                    )
                  : transformClip &&
                    scaledCanvas && (
                      <TransformOverlay
                        clip={transformClip}
                        canvasPx={scaledCanvas}
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
          disabled={!canCapture}
          onClick={() => void captureFrame()}
        >
          <Icon icon={Camera} size={13} />
        </HoverButton>
        <ProjectSettingsBadges fps={timeline.fps} width={timeline.width} height={timeline.height} />
      </div>
    </>
  );
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
  // Runtime fallback flag (previewEngine.ts trips it when the engine can't start):
  // unmount the MJPEG <img> so the legacy <video> underneath shows through instead
  // of a frozen/black stream frame.
  const engineFailed = useEditorUiStore((s) => s.rustEngineFailed);
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
    shouldUseRustEngine({ rustEnabled: rustEngineEnabled(), isTauri, isPlaying, isScrubbing, engineFailed }) &&
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
  // Hi-res first-frame poster, painted INSTANTLY behind the <video> so a cold
  // click shows a sharp frame with no blank/spinner. Decoded (and cached) by the
  // backend on select; the asset protocol then streams the real video
  // progressively (it honors HTTP Range, so `preload="metadata"` below does not
  // download the whole file). Only fetched for video; cleared between items so a
  // stale poster never flashes on the next clip.
  const [posterUrl, setPosterUrl] = useState<string | null>(null);
  useEffect(() => {
    if (item.type !== "video" || item.missing) {
      setPosterUrl(null);
      return;
    }
    let cancelled = false;
    setPosterUrl(null);
    void previewPoster(item.id).then((path) => {
      if (!cancelled && path) setPosterUrl(assetUrl(path));
    });
    return () => {
      cancelled = true;
    };
  }, [item.id, item.type, item.missing]);

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
  // `preload="metadata"` (not the default "auto") + an instant hi-res `poster`
  // make a cold click near-instant: the first frame shows immediately and the
  // asset protocol streams the rest progressively via HTTP Range, instead of
  // eagerly buffering the whole file behind a blank frame.
  return (
    <video
      ref={(el) => {
        mediaRef.current = el;
      }}
      src={url}
      poster={posterUrl ?? undefined}
      preload="metadata"
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

interface BadgeMenuOption {
  key: string;
  label: string;
  active: boolean;
  onSelect: () => void;
}

/**
 * Compact borderless badge that opens a popup menu — the port of upstream's
 * `settingsMenuButton` (`.menuStyle(.borderlessButton)`,
 * PreviewContainerView.swift:253-268). Keeps the Badge's compact look for the
 * trigger and reuses the app Dropdown's raised-popup + checked-row styling for
 * the menu. Closes on outside click / Escape.
 */
function BadgeMenu({
  label,
  ariaLabel,
  options,
}: {
  label: string;
  ariaLabel: string;
  options: BadgeMenuOption[];
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={rootRef} style={{ position: "relative", display: "inline-block" }}>
      <button
        type="button"
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="hover-area tabular"
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: 2,
          height: "var(--icon-md-lg)",
          padding: "0 var(--space-sm)",
          borderRadius: "var(--radius-xs-sm)",
          background: "transparent",
          border: "none",
          color: "var(--text-secondary)",
          fontSize: "var(--fs-xxs)",
          fontWeight: "var(--fw-bold)",
          cursor: "pointer",
        }}
      >
        <span>{label}</span>
        <span style={{ display: "inline-flex", opacity: 0.6 }}>
          <Icon icon={ChevronDown} size={10} />
        </span>
      </button>

      {open && (
        <div
          role="listbox"
          style={{
            position: "absolute",
            bottom: "calc(100% + var(--space-xs))",
            right: 0,
            minWidth: 96,
            padding: "var(--space-xxs)",
            background: "var(--bg-raised)",
            border: "var(--bw-thin) solid var(--border-primary)",
            borderRadius: "var(--radius-md)",
            boxShadow: "var(--shadow-lg)",
            zIndex: 50,
            display: "flex",
            flexDirection: "column",
            gap: 1,
          }}
        >
          {options.map((opt) => (
            <button
              key={opt.key}
              type="button"
              role="option"
              aria-selected={opt.active}
              onClick={() => {
                opt.onSelect();
                setOpen(false);
              }}
              className="hover-area"
              style={{
                display: "flex",
                alignItems: "center",
                gap: "var(--space-sm)",
                height: 26,
                padding: "0 var(--space-sm)",
                borderRadius: "var(--radius-xs-sm)",
                background: opt.active ? "var(--bg-prominent)" : "transparent",
                color: opt.active ? "var(--text-primary)" : "var(--text-secondary)",
                fontSize: "var(--fs-sm)",
                fontWeight: "var(--fw-medium)",
                textAlign: "left",
                cursor: "pointer",
              }}
            >
              <span style={{ width: 12, display: "inline-flex", justifyContent: "center", flex: "0 0 auto" }}>
                {opt.active && <Icon icon={Check} size={11} />}
              </span>
              <span style={{ flex: 1 }}>{opt.label}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * Interactive project-setting badges (Items 1 + 2). Aspect applies an
 * AspectPreset via SetTimelineSettings (changes timeline W/H). Quality selects a
 * preview render quality (short-edge cap, uiStore.previewQualityShortEdge — does
 * NOT change timeline dims). Zoom sets the canvas zoom preset (resetting the pan
 * offset, upstream zoomMenu PreviewContainerView.swift:209-224). FPS stays a
 * read-only badge (the FPS menu is out of scope for this pass). Mirrors
 * upstream's `projectSettingsGroup` (:131-154).
 */
function ProjectSettingsBadges({ fps, width, height }: { fps: number; width: number; height: number }) {
  const t = useT();
  const canvasZoom = useEditorUiStore((s) => s.canvasZoom);
  const setCanvasZoom = useEditorUiStore((s) => s.setCanvasZoom);
  const setCanvasOffset = useEditorUiStore((s) => s.setCanvasOffset);
  const previewQualityShortEdge = useEditorUiStore((s) => s.previewQualityShortEdge);
  const setPreviewQualityShortEdge = useEditorUiStore((s) => s.setPreviewQualityShortEdge);

  const g = gcd(width, height) || 1;

  const applyZoom = (preset: ZoomPreset) => {
    // Upstream zoomMenu resets offset to zero, then sets zoom
    // (PreviewContainerView.swift:212-213).
    setCanvasOffset({ width: 0, height: 0 });
    setCanvasZoom(preset.value);
  };

  const applyAspect = (preset: AspectPreset) => {
    void setTimelineSettings(fps, preset.width, preset.height);
  };

  const applyQuality = (preset: QualityPreset) => {
    // Toggle off if re-picking the active one (back to backend default cap);
    // otherwise store the chosen short edge.
    setPreviewQualityShortEdge(previewQualityShortEdge === preset.shortEdge ? null : preset.shortEdge);
  };

  return (
    <div style={{ display: "flex", alignItems: "center", gap: "var(--space-xs)" }}>
      <BadgeMenu
        label={`${width / g}:${height / g}`}
        ariaLabel={t("preview.aspectRatio")}
        options={ASPECT_PRESETS.map((p) => ({
          key: p.label,
          label: p.label,
          active: isAspectPresetActive(p, width, height),
          onSelect: () => applyAspect(p),
        }))}
      />
      <Badge>{fps}</Badge>
      <BadgeMenu
        label={qualityBadgeLabel(width, height)}
        ariaLabel={t("preview.quality")}
        options={QUALITY_PRESETS.map((p) => ({
          key: p.label,
          label: p.label,
          // Active when it matches the timeline's native short edge OR the
          // chosen preview-quality cap.
          active:
            previewQualityShortEdge === p.shortEdge ||
            (previewQualityShortEdge === null && isQualityPresetActive(p, width, height)),
          onSelect: () => applyQuality(p),
        }))}
      />
      <BadgeMenu
        label={zoomBadgeLabel(canvasZoom)}
        ariaLabel={t("preview.canvasZoom")}
        options={ZOOM_PRESETS.map((p) => ({
          key: p.label,
          label: p.label,
          active: isZoomPresetActive(p, canvasZoom),
          onSelect: () => applyZoom(p),
        }))}
      />
    </div>
  );
}

function gcd(a: number, b: number): number {
  return b === 0 ? a : gcd(b, a % b);
}
