/**
 * Tiny ownership flag so the two playback clocks never both advance the
 * playhead. While `<TimelinePlayback>` is mounted it drives `activeFrame` from
 * the real media elements (audio/video) and CLAIMS the clock; the fallback
 * `usePlaybackTicker` (a plain dt-based rAF, used when no preview/media element
 * is available) then YIELDS instead of advancing. If the media clock releases
 * mid-playback (e.g. the preview panel unmounts), the fallback resumes.
 */

// Reference-counted so overlapping claim/release (StrictMode double-invoke, a
// brief mount/unmount overlap) can't leave the clock stuck owned or released.
let refCount = 0;

export const mediaClock = {
  claim(): void {
    refCount += 1;
  },
  release(): void {
    refCount = Math.max(0, refCount - 1);
  },
  get active(): boolean {
    return refCount > 0;
  },
};
