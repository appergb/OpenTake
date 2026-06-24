/**
 * Real-time timeline preview surface (issue #142). The VIEW half of the playback
 * engine: it mounts the `<video>`/`<audio>` elements for the clips under the
 * playhead and registers them with the engine — it owns NO clock. The single
 * clock lives in `previewEngine.ts` (useTimelinePlaybackEngine, mounted in App),
 * mirroring upstream's split of an app-level VideoEngine driving a passive
 * PreviewView.
 *
 * This surface is visible while PLAYING or SCRUBBING (the cheap, live path the
 * single-media preview already uses); when settled it goes transparent so the
 * high-fidelity Rust GPU composite shows through. We can't GPU-composite live in
 * the WebView, so transform/crop/text during playback await the streaming engine
 * (#53); this surface shows raw decoded frames (opacity + track order).
 */

import { useEditorUiStore } from "../../store/uiStore";
import { useMediaStore } from "../../store/mediaStore";
import { assetUrl } from "../../lib/asset";
import { previewElements } from "./previewEngine";
import {
  activeAudioClips,
  activeVisualClip,
  clipOpacity,
  sourceTimeSec,
} from "./timelinePlayback";
import type { Clip, Timeline } from "../../lib/types";
import { useRef } from "react";

export function TimelinePlayback({ timeline, fps }: { timeline: Timeline; fps: number }) {
  // Subscribe to activeFrame so the right clips stay mounted as the playhead
  // moves; subscribe to play/scrub state so the surface shows only then.
  const activeFrame = useEditorUiStore((s) => s.activeFrame);
  const isPlaying = useEditorUiStore((s) => s.isPlaying);
  const isScrubbing = useEditorUiStore((s) => s.isScrubbing);
  const items = useMediaStore((s) => s.items);
  const frame = Math.round(activeFrame);
  const live = isPlaying || isScrubbing;

  const visual = activeVisualClip(timeline, frame);
  const audios = activeAudioClips(timeline, frame);

  const urlFor = (mediaRef: string): string | null =>
    assetUrl(items.find((m) => m.id === mediaRef)?.path);

  // Stable ref callback per clip id (cached) so a clip's element isn't
  // detached/re-attached every re-render — only a changing function identity
  // would do that, so we keep one callback per id. Detaching pauses the element
  // first: React detaches refs (commit phase, synchronous) and a DOM media
  // element removed from the tree keeps playing unless paused here.
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

  // Aspect-fit via intrinsic media size + max-width/height; the parent stage
  // flex-centers us. No absolute positioning (which would escape an unpositioned
  // ancestor and mis-place the frame — the old "bottom-left corner" bug).
  const fill: React.CSSProperties = {
    maxWidth: "100%",
    maxHeight: "100%",
    objectFit: "contain",
    display: "block",
  };

  const visualUrl = visual ? urlFor(visual.clip.mediaRef) : null;

  // Seek a freshly-mounted element to the right source position immediately, so
  // entering a clip (or starting playback mid-timeline) shows the correct frame
  // instead of the source's frame 0.
  const seekOnLoad = (clip: Clip) => (e: React.SyntheticEvent<HTMLMediaElement>) => {
    const f = Math.round(useEditorUiStore.getState().activeFrame);
    e.currentTarget.currentTime = sourceTimeSec(clip, f, fpsRef.current > 0 ? fpsRef.current : 30);
  };

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        overflow: "hidden",
      }}
    >
      {visual && visualUrl && visual.clip.mediaType === "video" && (
        <video
          key={visual.clip.id}
          ref={register(visual.clip.id)}
          src={visualUrl}
          playsInline
          preload="auto"
          onLoadedData={seekOnLoad(visual.clip)}
          style={{ ...fill, opacity: live ? clipOpacity(visual.clip) : 0 }}
        />
      )}
      {visual && visualUrl && visual.clip.mediaType === "image" && (
        <img
          key={visual.clip.id}
          src={visualUrl}
          alt=""
          draggable={false}
          style={{ ...fill, opacity: live ? clipOpacity(visual.clip) : 0 }}
        />
      )}
      {audios.map((a) => {
        const url = urlFor(a.clip.mediaRef);
        return url ? (
          <audio
            key={a.clip.id}
            ref={register(a.clip.id)}
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
