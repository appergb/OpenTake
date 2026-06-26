import { describe, expect, it } from "vitest";
import { createFallbackStore } from "./fallback";

describe("browser fallback edit store", () => {
  it("supports insertTrack and addClips for media drops", () => {
    const fallback = createFallbackStore();
    fallback.reset();

    const trackResult = fallback.editApply({ type: "insertTrack", kind: "video" });
    const addResult = fallback.editApply({
      type: "addClips",
      entries: [
        {
          mediaRef: "m1",
          mediaType: "video",
          sourceClipType: "video",
          trackIndex: 0,
          startFrame: 12,
          durationFrames: 30,
          hasAudio: false,
          addLinkedAudio: false,
        },
      ],
    });

    const timeline = fallback.getTimeline().timeline;

    expect(trackResult.changed).toBe(true);
    expect(addResult.changed).toBe(true);
    expect(addResult.affectedClipIds).toHaveLength(1);
    expect(timeline.tracks).toHaveLength(1);
    expect(timeline.tracks[0].type).toBe("video");
    expect(timeline.tracks[0].clips[0]).toMatchObject({
      id: addResult.affectedClipIds[0],
      mediaRef: "m1",
      mediaType: "video",
      sourceClipType: "video",
      startFrame: 12,
      durationFrames: 30,
    });
  });

  it("inserts tracks at a requested index", () => {
    const fallback = createFallbackStore();
    fallback.reset();

    fallback.editApply({ type: "insertTrack", kind: "video" });
    fallback.editApply({ type: "insertTrack", kind: "audio" });
    const result = fallback.editApply({ type: "insertTrack", kind: "video", at: 0 });

    expect(result.affectedClipIds).toEqual(["t102"]);
    expect(fallback.getTimeline().timeline.tracks.map((track) => track.id)).toEqual(["t102", "t100", "t101"]);
  });

  it("adds linked audio when dropping a video asset with audio", () => {
    const fallback = createFallbackStore();
    fallback.reset();
    fallback.editApply({ type: "insertTrack", kind: "video" });

    const addResult = fallback.editApply({
      type: "addClips",
      entries: [
        {
          mediaRef: "m1",
          mediaType: "video",
          sourceClipType: "video",
          trackIndex: 0,
          startFrame: 12,
          durationFrames: 30,
          hasAudio: true,
          addLinkedAudio: true,
        },
      ],
    });
    const timeline = fallback.getTimeline().timeline;

    expect(addResult.affectedClipIds).toHaveLength(2);
    expect(timeline.tracks.map((track) => track.type)).toEqual(["video", "audio"]);
    const video = timeline.tracks[0].clips[0];
    const audio = timeline.tracks[1].clips[0];
    expect(video.linkGroupId).toBeTruthy();
    expect(audio.linkGroupId).toBe(video.linkGroupId);
    expect(audio).toMatchObject({
      id: addResult.affectedClipIds[1],
      mediaRef: "m1",
      mediaType: "audio",
      sourceClipType: "video",
      startFrame: 12,
      durationFrames: 30,
    });
  });

  it("trims and splits overwritten regions instead of swallowing entire clips", () => {
    const fallback = createFallbackStore();
    fallback.reset();
    fallback.editApply({ type: "insertTrack", kind: "video" });
    const first = fallback.editApply({
      type: "addClips",
      entries: [
        {
          mediaRef: "base",
          mediaType: "video",
          sourceClipType: "video",
          trackIndex: 0,
          startFrame: 0,
          durationFrames: 100,
          hasAudio: false,
          addLinkedAudio: false,
        },
      ],
    });

    fallback.editApply({
      type: "addClips",
      entries: [
        {
          mediaRef: "overlay",
          mediaType: "video",
          sourceClipType: "video",
          trackIndex: 0,
          startFrame: 40,
          durationFrames: 20,
          hasAudio: false,
          addLinkedAudio: false,
        },
      ],
    });
    const clips = fallback.getTimeline().timeline.tracks[0].clips;

    expect(clips).toEqual([
      expect.objectContaining({ id: first.affectedClipIds[0], mediaRef: "base", startFrame: 0, durationFrames: 40 }),
      expect.objectContaining({ mediaRef: "overlay", startFrame: 40, durationFrames: 20 }),
      expect.objectContaining({ mediaRef: "base", startFrame: 60, durationFrames: 40 }),
    ]);
  });

  it("supports duplicateClips for Option-drag duplicate previews", () => {
    const fallback = createFallbackStore();
    fallback.reset();
    fallback.editApply({ type: "insertTrack", kind: "video" });
    fallback.editApply({ type: "insertTrack", kind: "video" });
    const addResult = fallback.editApply({
      type: "addClips",
      entries: [
        {
          mediaRef: "m1",
          mediaType: "video",
          sourceClipType: "video",
          trackIndex: 0,
          startFrame: 10,
          durationFrames: 20,
          hasAudio: false,
          addLinkedAudio: false,
        },
      ],
    });
    const sourceId = addResult.affectedClipIds[0];

    const duplicateResult = fallback.editApply({
      type: "duplicateClips",
      clipIds: [sourceId],
      offsetFrames: 15,
      targetTrackIndexes: [1],
    });
    const timeline = fallback.getTimeline().timeline;

    expect(duplicateResult.changed).toBe(true);
    expect(duplicateResult.affectedClipIds).toHaveLength(1);
    expect(timeline.tracks[0].clips.map((clip) => clip.id)).toEqual([sourceId]);
    expect(timeline.tracks[1].clips[0]).toMatchObject({
      id: duplicateResult.affectedClipIds[0],
      mediaRef: "m1",
      startFrame: 25,
      durationFrames: 20,
    });
  });

  it("plans multi-clip duplicates before clearing destination ranges", () => {
    const fallback = createFallbackStore();
    fallback.reset();
    fallback.editApply({ type: "insertTrack", kind: "video" });
    const addResult = fallback.editApply({
      type: "addClips",
      entries: [
        {
          mediaRef: "a",
          mediaType: "video",
          sourceClipType: "video",
          trackIndex: 0,
          startFrame: 0,
          durationFrames: 30,
          hasAudio: false,
          addLinkedAudio: false,
        },
        {
          mediaRef: "b",
          mediaType: "video",
          sourceClipType: "video",
          trackIndex: 0,
          startFrame: 30,
          durationFrames: 30,
          hasAudio: false,
          addLinkedAudio: false,
        },
      ],
    });

    const duplicateResult = fallback.editApply({
      type: "duplicateClips",
      clipIds: addResult.affectedClipIds,
      offsetFrames: 15,
      targetTrackIndexes: [0, 0],
    });

    expect(duplicateResult.affectedClipIds).toHaveLength(2);
    expect(fallback.getTimeline().timeline.tracks[0].clips.map((clip) => clip.mediaRef)).toEqual([
      "a",
      "a",
      "b",
    ]);
  });

  it("remaps linked duplicate groups to a fresh shared link", () => {
    const fallback = createFallbackStore();
    fallback.reset();
    fallback.editApply({ type: "insertTrack", kind: "video" });
    const addResult = fallback.editApply({
      type: "addClips",
      entries: [
        {
          mediaRef: "linked-av",
          mediaType: "video",
          sourceClipType: "video",
          trackIndex: 0,
          startFrame: 0,
          durationFrames: 30,
          hasAudio: true,
          addLinkedAudio: true,
        },
      ],
    });
    const originalVideoId = addResult.affectedClipIds[0];
    const originalAudioId = addResult.affectedClipIds[1];
    const originalClips = fallback.getTimeline().timeline.tracks.flatMap((track) => track.clips);
    const originalVideo = originalClips.find((clip) => clip.id === originalVideoId);
    const originalAudio = originalClips.find((clip) => clip.id === originalAudioId);

    const duplicateResult = fallback.editApply({
      type: "duplicateClips",
      clipIds: [originalVideoId, originalAudioId],
      offsetFrames: 200,
      targetTrackIndexes: [0, 1],
    });
    const clips = fallback.getTimeline().timeline.tracks.flatMap((track) => track.clips);
    const videoCopy = clips.find((clip) => clip.id === duplicateResult.affectedClipIds[0]);
    const audioCopy = clips.find((clip) => clip.id === duplicateResult.affectedClipIds[1]);

    expect(duplicateResult.affectedClipIds).toHaveLength(2);
    expect(videoCopy?.linkGroupId).toBeTruthy();
    expect(audioCopy?.linkGroupId).toBe(videoCopy?.linkGroupId);
    expect(videoCopy?.linkGroupId).not.toBe(originalVideo?.linkGroupId);
    expect(originalVideo?.linkGroupId).toBe(originalAudio?.linkGroupId);
  });

  it("persists fade length and interpolation properties", () => {
    const fallback = createFallbackStore();

    const result = fallback.editApply({
      type: "setClipProperties",
      clipIds: ["c1"],
      properties: {
        fadeInFrames: 7,
        fadeOutFrames: 9,
        fadeInInterpolation: "smooth",
        fadeOutInterpolation: "smooth",
      },
    });

    const clip = fallback
      .getTimeline()
      .timeline.tracks.flatMap((track) => track.clips)
      .find((candidate) => candidate.id === "c1");

    expect(result.changed).toBe(true);
    expect(clip?.fadeInFrames).toBe(7);
    expect(clip?.fadeOutFrames).toBe(9);
    expect(clip?.fadeInInterpolation).toBe("smooth");
    expect(clip?.fadeOutInterpolation).toBe("smooth");
  });
});
