/**
 * Real-time timeline preview surface (issue #142). The VIEW half of the playback
 * engine: it mounts the `<video>`/`<audio>` elements for the clips under the
 * playhead and registers them with the engine — it owns NO clock. The single
 * clock lives in `previewEngine.ts` (useTimelinePlaybackEngine, mounted in App),
 * mirroring upstream's split of an app-level VideoEngine driving a passive
 * PreviewView.
 *
 * This surface stays visible for both play and pause: while PLAYING/SCRUBBING it
 * advances live media elements, and while PAUSED it holds those same elements
 * frozen on the pause frame. That mirrors upstream's AVPlayerLayer model and
 * avoids color/size changes from swapping to a separate ffmpeg PNG composite.
 */

import { useEditorUiStore } from "../../store/uiStore";
import { useMediaStore } from "../../store/mediaStore";
import { assetUrl } from "../../lib/asset";
import { previewElementKey, previewElements } from "./previewEngine";
import {
  activeAudioClips,
  activeVisualClip,
  activeVisualClips,
  playbackFrameFromActiveFrame,
  sourceTimeSec,
} from "./timelinePlayback";
import type { ActiveMedia } from "./timelinePlayback";
import {
  timelinePreviewClipStyle,
  timelinePreviewCropMaskStyle,
  timelinePreviewCroppedMediaStyle,
  timelinePreviewLayerStyle,
} from "./previewLayerStyles";
import type { Clip, Timeline } from "../../lib/types";
import { useRef } from "react";

export function TimelinePlayback({ timeline, fps }: { timeline: Timeline; fps: number }) {
  // Subscribe to activeFrame so the right clips stay mounted as the playhead moves.
  const frame = useEditorUiStore((s) => playbackFrameFromActiveFrame(s.activeFrame));
  const items = useMediaStore((s) => s.items);

  const visuals = activeVisualClips(timeline, frame);
  const audios = activeAudioClips(timeline, frame);
  const visual = visuals.length > 0 ? visuals[visuals.length - 1] : null;

  const urlFor = (mediaRef: string): string | null =>
    assetUrl(items.find((m) => m.id === mediaRef)?.path);

  // Stable ref callback per playback key (cached) so a same-source split clip's
  // element isn't detached/re-attached at the edit boundary. Only a changing
  // function identity would do that, so we keep one callback per key. Detaching
  // pauses the element first: React detaches refs (commit phase, synchronous),
  // and a DOM media element removed from the tree keeps playing unless paused here.
  const cbCache = useRef<Map<string, (el: HTMLMediaElement | null) => void>>(new Map());
  const register = (id: string) => {
    let cb = cbCache.current.get(id);
    if (!cb) {
      cb = (el: HTMLMediaElement | null) => {
        if (el) previewElements.set(id, el);
        else previewElements.remove(id);
      };
      cbCache.current.set(id, cb);
    }
    return cb;
  };

  const fpsRef = useRef(fps);
  fpsRef.current = fps;

  // Seek a freshly-mounted element to the right source position immediately, so
  // entering a clip (or starting playback mid-timeline) shows the correct frame
  // instead of the source's frame 0.
  const seekOnLoad = (clip: Clip) => (e: React.SyntheticEvent<HTMLMediaElement>) => {
    const f = Math.max(0, Math.floor(useEditorUiStore.getState().activeFrame));
    e.currentTarget.currentTime = sourceTimeSec(clip, f, fpsRef.current > 0 ? fpsRef.current : 30);
  };

  // --- Clip boundary preload + outgoing-clip retention (#131) ---
  // When the playhead crosses a clip boundary the old <video> unmounts and a
  // new one mounts, causing a load/decode gap (black frame + audio dropout).
  // Two mechanisms prevent this:
  //
  // 1. PRELOAD: 10 frames before the current clip ends, mount the NEXT clip's
  //    <video> (hidden, opacity 0) so it loads + decodes before the boundary.
  //    The unique `key` per clip id means React REUSES the preloaded element
  //    when it becomes the current clip — no remount, no gap.
  // 2. OUTGOING: keep the previous clip's <video> mounted for one extra frame
  //    (opacity 0) as a safety net in case the incoming clip's first frame
  //    isn't decoded yet. The element is paused when it unmounts (see
  //    `register`), and one frame of muted audio bleed (~16ms) is inaudible.
  const PRELOAD_FRAMES = 10;

  // The next visual clip after the current one ends (null if same or none).
  let nextVisual: ActiveMedia | null = null;
  if (visual) {
    const endFrame = visual.clip.startFrame + visual.clip.durationFrames;
    const candidate = activeVisualClip(timeline, endFrame);
    if (candidate && candidate.clip.id !== visual.clip.id) nextVisual = candidate;
  }
  const shouldPreloadNext =
    visual != null &&
    nextVisual != null &&
    nextVisual.clip.mediaType === "video" &&
    visual.clip.startFrame + visual.clip.durationFrames - frame <= PRELOAD_FRAMES;

  // Track the outgoing clip (previous visual) to keep it for one frame.
  // Ref-during-render follows the existing `tlRef`/`fpsRef` pattern above.
  const prevVisualRef = useRef<ActiveMedia | null>(null);
  const outgoing =
    prevVisualRef.current != null &&
    visual != null &&
    prevVisualRef.current.clip.id !== visual.clip.id
      ? prevVisualRef.current
      : null;
  prevVisualRef.current = visual;

  return (
    <div data-playback-surface="webkit" style={timelinePreviewLayerStyle}>
      {visuals.map((visual) => {
        const key = previewElementKey(visual);
        const url = urlFor(visual.clip.mediaRef);
        if (!url) return null;
        const cropMaskStyle = timelinePreviewCropMaskStyle(visual.clip, frame);
        const mediaStyle = timelinePreviewCroppedMediaStyle(visual.clip, frame);
        return (
          <div
            key={key}
            // Explicit z-order so the preview composites in the SAME order as the
            // final render (opentake-render keeps visual track 0 topmost): lower
            // track index = higher layer. Without this the order relied on DOM
            // paint order, which React reconciliation could shuffle as clips
            // enter/leave during scrub — making the preview disagree with the
            // exported frame. Track indices are small, so 1000 is a safe base.
            style={{ ...timelinePreviewClipStyle(visual.clip, frame), zIndex: 1000 - visual.trackIndex }}
          >
            <div style={cropMaskStyle}>
              {visual.clip.mediaType === "video" ? (
                <video
                  ref={register(key)}
                  src={url}
                  playsInline
                  preload="auto"
                  onLoadedData={seekOnLoad(visual.clip)}
                  style={mediaStyle}
                />
              ) : (
                <img src={url} alt="" draggable={false} style={mediaStyle} />
              )}
            </div>
          </div>
        );
      })}
      {/* Clip boundary preload: mount the next clip's <video> hidden so it
          decodes before the boundary (#131). The unique key means React
          reuses this element when the playhead crosses — no remount gap. */}
      {shouldPreloadNext && nextVisual && nextVisual.clip.mediaType === "video" &&
        urlFor(nextVisual.clip.mediaRef) && (
        <div
          key={previewElementKey(nextVisual)}
          style={{ ...timelinePreviewClipStyle(nextVisual.clip, frame), opacity: 0, pointerEvents: "none" }}
        >
          <div style={timelinePreviewCropMaskStyle(nextVisual.clip, frame)}>
            <video
              ref={register(previewElementKey(nextVisual))}
              src={urlFor(nextVisual.clip.mediaRef)!}
              playsInline
              preload="auto"
              onLoadedData={seekOnLoad(nextVisual.clip)}
              style={timelinePreviewCroppedMediaStyle(nextVisual.clip, frame)}
            />
          </div>
        </div>
      )}
      {/* Outgoing clip retention: keep the previous clip for one frame as a
          safety net in case the incoming clip's first frame isn't decoded
          yet (#131). Hidden (opacity 0); paused when it unmounts. */}
      {outgoing && outgoing.clip.mediaType === "video" &&
        urlFor(outgoing.clip.mediaRef) && (
        <div
          key={previewElementKey(outgoing)}
          style={{ ...timelinePreviewClipStyle(outgoing.clip, frame), opacity: 0, pointerEvents: "none" }}
        >
          <div style={timelinePreviewCropMaskStyle(outgoing.clip, frame)}>
            <video
              ref={register(previewElementKey(outgoing))}
              src={urlFor(outgoing.clip.mediaRef)!}
              playsInline
              preload="auto"
              style={timelinePreviewCroppedMediaStyle(outgoing.clip, frame)}
            />
          </div>
        </div>
      )}
      {audios.map((a) => {
        const key = previewElementKey(a);
        const url = urlFor(a.clip.mediaRef);
        return url ? (
          <audio
            key={key}
            ref={register(key)}
            src={url}
            preload="auto"
            onLoadedData={seekOnLoad(a.clip)}
            style={{ display: "none" }}
          />
        ) : null;
      })}
    </div>
  );
}
