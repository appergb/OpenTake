/**
 * Track zone helpers — port of upstream `zones` + `timelineTrackDisplayLabel`
 * (SPEC §5.5). Visual tracks render above audio tracks; the first audio index
 * marks the region divider. Labels are V1/A1/I1/... by kind.
 */

import type { ClipType, Timeline } from "./types";

export function firstAudioIndex(timeline: Timeline): number {
  return timeline.tracks.findIndex((t) => t.type === "audio");
}

const PREFIX: Record<ClipType, string> = {
  video: "V",
  audio: "A",
  image: "I",
  text: "T",
  lottie: "L",
};

/**
 * "V1"/"A1"/... label for a track (1:1 with `timelineTrackDisplayLabel`).
 * Audio counts top-down (top audio = A1). Visual tracks count from this track
 * DOWN to the first audio track, so the TOPMOST visual track gets the highest
 * number and the bottom one gets 1 — matching upstream's stacking order.
 */
export function trackDisplayLabel(timeline: Timeline, i: number): string {
  if (i < 0 || i >= timeline.tracks.length) return "";
  const kind = timeline.tracks[i].type;
  let n = 0;
  if (kind === "audio") {
    for (let k = 0; k <= i; k++) {
      if (timeline.tracks[k].type === kind) n++;
    }
  } else {
    const fa = firstAudioIndex(timeline);
    const visualEnd = fa >= 0 ? fa : timeline.tracks.length;
    const end = Math.max(i + 1, visualEnd);
    for (let k = i; k < end; k++) {
      if (timeline.tracks[k].type === kind) n++;
    }
  }
  return `${PREFIX[kind]}${n}`;
}
