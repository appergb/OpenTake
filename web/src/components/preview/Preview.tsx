/**
 * Preview (SPEC §8). Tab bar + aspect-fit canvas area + scrub bar + transport
 * bar with project-setting badges. The canvas displays Rust composite frames via
 * the `preview_frame` event (SPEC §11.2) — not yet wired, so it shows the canvas
 * background + a centered placeholder. Transport drives the local playhead.
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
import { useTimelineFrame } from "./useTimelineFrame";
import { TimelinePlayback } from "./TimelinePlaybackLayer";
import { useT } from "../../i18n";
import type { MediaItem } from "../../lib/types";

/** A value that only updates after it has been stable for `ms` (trailing
 *  debounce). Used to defer the expensive timeline composite until the playhead
 *  stops moving, so scrubbing doesn't spawn a per-frame ffmpeg/GPU storm. */
function useDebounced<T>(value: T, ms: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const id = window.setTimeout(() => setDebounced(value), ms);
    return () => window.clearTimeout(id);
  }, [value, ms]);
  return debounced;
}

export function Preview() {
  const t = useT();
  const timeline = useProjectStore((s) => s.timeline);
  const activeFrame = useEditorUiStore((s) => s.activeFrame);
  const setCurrentFrame = useEditorUiStore((s) => s.setCurrentFrame);
  const isPlaying = useEditorUiStore((s) => s.isPlaying);
  const togglePlayTimeline = useEditorUiStore((s) => s.togglePlay);
  const previewMediaId = useEditorUiStore((s) => s.previewMediaId);
  const previewItem = useMediaStore((s) =>
    previewMediaId ? s.items.find((m) => m.id === previewMediaId) ?? null : null,
  );

  // Media-preview playback is driven by the app transport (more capable than the
  // <video>'s native controls), so the <video>/<audio> renders WITHOUT controls
  // and this ref + state mirror its time/duration into the shared transport.
  const mediaRef = useRef<HTMLMediaElement | null>(null);
  const [mediaTime, setMediaTime] = useState(0);
  const [mediaDuration, setMediaDuration] = useState(0);
  const [mediaPlaying, setMediaPlaying] = useState(false);
  useEffect(() => {
    setMediaTime(0);
    setMediaDuration(0);
    setMediaPlaying(false);
  }, [previewMediaId]);

  // Space bar during media preview → toggle the media element.
  const mediaToggleCount = useEditorUiStore((s) => s.mediaPreviewToggleRequest);
  useEffect(() => {
    if (mediaToggleCount > 0 && previewing) {
      togglePlay();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mediaToggleCount]);

  const previewing = previewItem !== null;
  // Timeline composite preview (#47): on the Timeline tab, paint the GPU-
  // composited frame for the current playhead (replacing the black placeholder).
  // `timeline` identity changes on every `timeline_changed`, forcing a refetch.
  // The frame is clamped to the last DRAWABLE frame (total-1; clips are half-open
  // [start,end)) so parking at the very end doesn't composite to black. During
  // playback the request rate is capped (~11fps) to bound ffmpeg/PNG churn until
  // the streaming engine (#53) lands; paused/scrub stays immediate.
  const timelineTotal = totalFrames(timeline);
  // During playback `<TimelinePlayback>` plays the real media elements, so the
  // GPU composite is fetched only when PAUSED/scrubbing (accurate text/effects,
  // and no per-frame ffmpeg/PNG churn while playing).
  const timelineHasContent = !previewing && timeline.tracks.length > 0;
  // Debounce the composited frame: each composite is a separate ffmpeg decode +
  // wgpu pass + PNG + base64, so firing one per scrubbed frame spawns a storm of
  // subprocess + GPU work that can lock up the whole machine (reported freeze).
  // The playhead still moves instantly; the composite is fetched only once the
  // playhead has settled for ~140ms (i.e. you stop scrubbing). See the perf
  // issue for the proper fix (a streaming playback/scrub engine).
  const composeFrame = useDebounced(
    Math.min(Math.round(activeFrame), Math.max(0, timelineTotal - 1)),
    140,
  );
  const timelineFrameUrl = useTimelineFrame(
    composeFrame,
    !previewing && timeline.tracks.length > 0 && !isPlaying,
    timeline,
    0,
  );
  const fps = timeline.fps;
  const total = previewing
    ? Math.max(0, Math.round(mediaDuration * fps))
    : totalFrames(timeline);
  const activeShownFrame = previewing ? Math.round(mediaTime * fps) : activeFrame;
  const playing = previewing ? mediaPlaying : isPlaying;
  const aspect = timeline.width / timeline.height;

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

  // Aspect-fit is done in pure CSS (intrinsic media size + max-width/height,
  // centered by the stage's flexbox) — no JS measurement, so there's no stale /
  // zero-size race that could render the frame tiny or off-center.
  void aspect;

  return (
    <>
      <PanelHeaderBar>
        <PreviewTabs item={previewItem} />
      </PanelHeaderBar>

      {/* Canvas stage: a flex-centered area; the media inside aspect-fits via
          intrinsic size + max-width/height, so it always fills the largest 16:9
          box and stays centered. */}
      <div
        style={{
          flex: 1,
          minHeight: 0,
          background: "var(--bg-surface)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          overflow: "hidden",
          padding: 8,
        }}
      >
        {/* Layer: TimelinePlayback lives here when there are tracks —
             it stays mounted even when paused so audio/video elements
             survive the pause→play transition (upstream VideoEngine model). */}
        {!previewItem && timelineHasContent && (
          <TimelinePlayback timeline={timeline} fps={fps} playing={isPlaying} />
        )}
        {previewItem ? (
          <MediaPreview
            item={previewItem}
            mediaRef={mediaRef}
            onTime={setMediaTime}
            onDuration={setMediaDuration}
            onPlayingChange={setMediaPlaying}
          />
        ) : isPlaying ? null : timelineFrameUrl ? (
          // Rust GPU composite of the timeline at the current playhead (#47).
          <img
            src={timelineFrameUrl}
            alt=""
            draggable={false}
            style={{
              position: "absolute",
              inset: 0,
              width: "100%",
              height: "100%",
              objectFit: "contain",
              display: "block",
            }}
          />
        ) : (
          // Empty / no-frame: a framed 16:9 canvas surface placeholder.
          <div
            style={{
              aspectRatio: `${timeline.width} / ${timeline.height}`,
              height: "100%",
              maxWidth: "100%",
              maxHeight: "100%",
              background: "var(--bg-preview-canvas)",
              border: "1px solid rgba(255,255,255,0.08)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: "var(--text-muted)",
              fontSize: "var(--fs-xs)",
            }}
          >
            {timeline.tracks.length === 0 ? t("preview.noMedia") : `${timeline.width}×${timeline.height}`}
          </div>
        )}
      </div>

      {/* The app's scrub + transport are the single control surface — they drive
          both the timeline composite and (via mediaRef) single-media preview, so
          the <video>/<audio> renders without its native controls. */}
      <ScrubBar frame={activeShownFrame} total={total} onSeek={seekTo} />

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
        <HoverButton title={t("preview.captureFrame")}>
          <Icon icon={Camera} size={13} />
        </HoverButton>
        <ProjectSettingsBadges fps={timeline.fps} width={timeline.width} height={timeline.height} />
      </div>
    </>
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

function ScrubBar({ frame, total, onSeek }: { frame: number; total: number; onSeek: (f: number) => void }) {
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
        seekFromEvent(e.clientX);
      }}
      onPointerMove={(e) => {
        if (e.buttons === 1) seekFromEvent(e.clientX);
      }}
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
