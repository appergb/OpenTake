import { beforeEach, describe, expect, it } from "vitest";
import { projectNew } from "../lib/api";
import type { MediaItem } from "../lib/types";
import { addMediaToTimelineAt, insertTrack } from "./editActions";
import { useEditorUiStore } from "./uiStore";
import { useProjectStore } from "./projectStore";
import { forceRefresh } from "./sync";

function video(id: string): MediaItem {
  return {
    id,
    name: id,
    type: "video",
    duration: 2,
    width: 1920,
    height: 1080,
    hasAudio: false,
  };
}

describe("browser fallback edit actions", () => {
  beforeEach(async () => {
    await projectNew();
    await forceRefresh();
    useEditorUiStore.setState({ activeFrame: 0, currentFrame: 0, selectedClipIds: new Set() });
  });

  it("places media drops on the newly inserted zone-clamped track", async () => {
    await insertTrack("video");
    await forceRefresh();
    await insertTrack("audio");
    await forceRefresh();

    await addMediaToTimelineAt(video("drop"), 12, null, 2);

    const tracks = useProjectStore.getState().timeline.tracks;
    expect(tracks.map((track) => track.type)).toEqual(["video", "video", "audio"]);
    expect(tracks[0].clips).toHaveLength(0);
    expect(tracks[1].clips.map((clip) => clip.mediaRef)).toEqual(["drop"]);
  });
});
