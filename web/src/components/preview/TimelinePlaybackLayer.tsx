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
  activeVisualClips,
  playbackFrameFromActiveFrame,
  sourceTimeSec,
} from "./timelinePlayback";
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

  return (
    <div style={timelinePreviewLayerStyle}>
      {visuals.map((visual) => {
        const key = previewElementKey(visual);
        const url = urlFor(visual.clip.mediaRef);
        if (!url) return null;
        const cropMaskStyle = timelinePreviewCropMaskStyle(visual.clip, frame);
        const mediaStyle = timelinePreviewCroppedMediaStyle(visual.clip, frame);
        return (
          <div key={key} style={timelinePreviewClipStyle(visual.clip, frame)}>
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
