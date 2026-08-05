import { describe, expect, it } from "vitest";
import { createFallbackStore } from "./fallback";
import type { EditRequest } from "./types";

describe("browser fallback edit store", () => {
  it("applies reversed through setClipProperties", async () => {
    const fallback = createFallbackStore();
    const id = fallback.getTimeline().timeline.tracks[0].clips[0].id;

    await fallback.editApply({ type: "setClipProperties", clipIds: [id], properties: { reversed: true } });

    const clip = fallback.getTimeline().timeline.tracks[0].clips[0];
    expect(clip.reversed).toBe(true);
  });

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

  it("swaps same-kind tracks without allowing cross-kind swaps", () => {
    const fallback = createFallbackStore();
    fallback.reset();
    fallback.editApply({ type: "insertTrack", kind: "video" });
    fallback.editApply({ type: "insertTrack", kind: "video" });
    fallback.editApply({ type: "insertTrack", kind: "audio" });

    const sameKind = fallback.editApply({ type: "swapTracks", a: 0, b: 1 });
    const crossKind = fallback.editApply({ type: "swapTracks", a: 1, b: 2 });

    expect(sameKind.changed).toBe(true);
    expect(crossKind.changed).toBe(false);
    expect(fallback.getTimeline().timeline.tracks.map((track) => track.id)).toEqual(["t101", "t100", "t102"]);
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

  it("keeps effect setters atomic when any clip id is missing", () => {
    const fallback = createFallbackStore();

    const result = fallback.editApply({
      type: "setEffects",
      clipIds: ["c1", "missing"],
      effects: [{ name: "grayscale", params: { amount: 0.4 }, enabled: true }],
    });
    const clip = fallback
      .getTimeline()
      .timeline.tracks.flatMap((track) => track.clips)
      .find((candidate) => candidate.id === "c1");

    expect(result.changed).toBe(false);
    expect(clip?.effects).toBeUndefined();
  });

  it("rejects an invalid color grade before browser fallback mutation", () => {
    const fallback = createFallbackStore();

    const result = fallback.editApply({
      type: "setColorGrade",
      clipIds: ["c1"],
      grade: { liftGammaGain: { gamma: { r: 0, g: 1, b: 1 } } },
    });
    const clip = fallback
      .getTimeline()
      .timeline.tracks.flatMap((track) => track.clips)
      .find((candidate) => candidate.id === "c1");

    expect(result.changed).toBe(false);
    expect(clip?.colorGrade).toBeUndefined();
  });

  it("normalizes and validates HSL secondary authored state", () => {
    const fallback = createFallbackStore();
    const applied = fallback.editApply({
      type: "setColorGrade",
      clipIds: ["c1"],
      grade: { hslSecondary: { hueCenter: 0.98, hueShift: 0.2 } },
    });
    const clip = fallback
      .getTimeline()
      .timeline.tracks.flatMap((track) => track.clips)
      .find((candidate) => candidate.id === "c1");
    expect(applied.changed).toBe(true);
    expect(clip?.colorGrade?.hslSecondary).toEqual({
      hueCenter: 0.98,
      hueWidth: 0.24,
      feather: 0.08,
      hueShift: 0.2,
      saturation: 0,
      lightness: 0,
    });

    const rejected = fallback.editApply({
      type: "setColorGrade",
      clipIds: ["c1"],
      grade: { hslSecondary: { hueWidth: 0 } },
    });
    expect(rejected.changed).toBe(false);
    expect(clip?.colorGrade?.hslSecondary?.hueCenter).toBe(0.98);
  });

  it("sets, adjusts, and removes a path-free managed LUT reference", () => {
    const fallback = createFallbackStore();
    const id = "0123456789abcdef".repeat(4);
    const applied = fallback.editApply({
      type: "setLut",
      clipIds: ["c1"],
      lut: { id, name: "Known Transform", intensity: 1 },
    });
    expect(applied.changed).toBe(true);
    expect(fallback.getTimeline().timeline.tracks[0].clips[0].lut).toEqual({
      id,
      name: "Known Transform",
      intensity: 1,
    });

    expect(
      fallback.editApply({
        type: "setLut",
        clipIds: ["c1"],
        lut: { id, name: "Known Transform", intensity: 1.5 },
      }).changed,
    ).toBe(false);
    expect(
      fallback.editApply({
        type: "setLut",
        clipIds: ["c1"],
        lut: { id, name: "bad\u0000name", intensity: 0.5 },
      }).changed,
    ).toBe(false);
    expect(
      fallback.editApply({
        type: "setLut",
        clipIds: ["c1"],
        lut: { id, name: "界".repeat(43), intensity: 0.5 },
      }).changed,
    ).toBe(false);
    expect(fallback.editApply({ type: "setLut", clipIds: ["c1"], lut: null }).changed).toBe(true);
    expect(fallback.getTimeline().timeline.tracks[0].clips[0].lut).toBeUndefined();
  });

  it("stores both pair ids and rejects an oversized adjacent cross dissolve", () => {
    const fallback = createFallbackStore();
    fallback.reset();
    fallback.editApply({ type: "insertTrack", kind: "video" });
    const first = fallback.editApply({
      type: "addClips",
      entries: [{ mediaRef: "a", mediaType: "video", sourceClipType: "video", trackIndex: 0, startFrame: 0, durationFrames: 60 }],
    }).affectedClipIds[0];
    const second = fallback.editApply({
      type: "addClips",
      entries: [{ mediaRef: "b", mediaType: "video", sourceClipType: "video", trackIndex: 0, startFrame: 60, durationFrames: 30 }],
    }).affectedClipIds[0];

    const result = fallback.editApply({
      type: "setTransition",
      fromClipId: first,
      toClipId: second,
      kind: "crossDissolve",
      durationFrames: 15,
    });

    expect(result.changed).toBe(true);
    expect(fallback.getTimeline().timeline.tracks[0].clips[0].transitionOut).toEqual({
      fromClipId: first,
      toClipId: second,
      kind: "crossDissolve",
      durationFrames: 15,
    });
    expect(fallback.editApply({
      type: "setTransition",
      fromClipId: first,
      toClipId: second,
      kind: "crossDissolve",
      durationFrames: 16,
    }).changed).toBe(false);
  });

  it("rejects an invalid or non-successor fallback transition", () => {
    const fallback = createFallbackStore();
    fallback.reset();
    fallback.editApply({ type: "insertTrack", kind: "video" });
    const first = fallback.editApply({
      type: "addClips",
      entries: [{ mediaRef: "a", mediaType: "video", sourceClipType: "video", trackIndex: 0, startFrame: 0, durationFrames: 60 }],
    }).affectedClipIds[0];
    const blocker = fallback.editApply({
      type: "addClips",
      entries: [{ mediaRef: "b", mediaType: "video", sourceClipType: "video", trackIndex: 0, startFrame: 30, durationFrames: 30 }],
    }).affectedClipIds[0];
    const successor = fallback.editApply({
      type: "addClips",
      entries: [{ mediaRef: "c", mediaType: "video", sourceClipType: "video", trackIndex: 0, startFrame: 60, durationFrames: 30 }],
    }).affectedClipIds[0];

    expect(fallback.editApply({
      type: "setTransition",
      fromClipId: first,
      toClipId: successor,
      kind: "crossDissolve",
      durationFrames: 15,
    }).changed).toBe(false);
    expect(fallback.editApply({
      type: "setTransition",
      fromClipId: blocker,
      toClipId: successor,
      kind: "crossDissolve",
      durationFrames: 0,
    }).changed).toBe(false);
  });

  it("places media with project settings and linked audio as one version", () => {
    const fallback = createFallbackStore();
    fallback.reset();
    const beforeVersion = fallback.getTimeline().version;

    const result = fallback.editApply({
      type: "placeMedia",
      settings: { fps: 60, width: 3840, height: 2160 },
      target: { kind: "newTrack", trackType: "video", at: 0 },
      entry: {
        mediaRef: "av-source",
        mediaType: "video",
        sourceClipType: "video",
        startFrame: 12,
        durationFrames: 48,
        hasAudio: true,
        addLinkedAudio: true,
      },
    });
    const snapshot = fallback.getTimeline();

    expect(result.changed).toBe(true);
    expect(result.timelineVersion).toBe(beforeVersion + 1);
    expect(snapshot.version).toBe(beforeVersion + 1);
    expect(snapshot.timeline).toMatchObject({
      fps: 60,
      width: 3840,
      height: 2160,
      settingsConfigured: true,
    });
    expect(snapshot.timeline.tracks.map((track) => track.type)).toEqual(["video", "audio"]);
    const [video, audio] = snapshot.timeline.tracks.flatMap((track) => track.clips);
    expect(result.affectedClipIds).toEqual([video.id, audio.id]);
    expect(video.linkGroupId).toBeTruthy();
    expect(audio.linkGroupId).toBe(video.linkGroupId);
  });

  it("rejects an invalid placeMedia target before applying its settings", () => {
    const fallback = createFallbackStore();
    fallback.reset();
    const before = fallback.getTimeline();

    const result = fallback.editApply({
      type: "placeMedia",
      settings: { fps: 60, width: 3840, height: 2160 },
      target: { kind: "newTrack", trackType: "audio" },
      entry: {
        mediaRef: "video-source",
        mediaType: "video",
        sourceClipType: "video",
        startFrame: 0,
        durationFrames: 30,
        hasAudio: false,
        addLinkedAudio: false,
      },
    });

    expect(result.changed).toBe(false);
    expect(fallback.getTimeline()).toEqual(before);
  });

  it("moves linked A/V to a new visual track in one command while pinning audio", () => {
    const fallback = createFallbackStore();
    fallback.reset();
    const placed = fallback.editApply({
      type: "placeMedia",
      target: { kind: "newTrack", trackType: "video" },
      entry: {
        mediaRef: "av-source",
        mediaType: "video",
        sourceClipType: "video",
        startFrame: 10,
        durationFrames: 20,
        hasAudio: true,
        addLinkedAudio: true,
      },
    });
    const before = fallback.getTimeline();
    const [videoId, audioId] = placed.affectedClipIds;

    const result = fallback.editApply({
      type: "moveOrDuplicateClipsToNewTrack",
      clipIds: [videoId, audioId],
      leadClipId: videoId,
      requestedFrameDelta: 15,
      insertAt: 0,
      mode: "move",
    });
    const after = fallback.getTimeline();
    const videoTrack = after.timeline.tracks.find((track) =>
      track.clips.some((clip) => clip.id === videoId),
    );
    const audioTrack = after.timeline.tracks.find((track) =>
      track.clips.some((clip) => clip.id === audioId),
    );

    expect(result.changed).toBe(true);
    expect(result.affectedClipIds).toEqual([videoId, audioId]);
    expect(after.version).toBe(before.version + 1);
    expect(videoTrack?.type).toBe("video");
    expect(audioTrack?.type).toBe("audio");
    expect(videoTrack?.clips[0].startFrame).toBe(25);
    expect(audioTrack?.clips[0].startFrame).toBe(25);
  });

  it("duplicates linked A/V to a new track with fresh shared identity", () => {
    const fallback = createFallbackStore();
    fallback.reset();
    const placed = fallback.editApply({
      type: "placeMedia",
      target: { kind: "newTrack", trackType: "video" },
      entry: {
        mediaRef: "av-source",
        mediaType: "video",
        sourceClipType: "video",
        startFrame: 0,
        durationFrames: 20,
        hasAudio: true,
        addLinkedAudio: true,
      },
    });
    const original = fallback.getTimeline();
    const [videoId, audioId] = placed.affectedClipIds;
    const originalGroup = original.timeline.tracks[0].clips[0].linkGroupId;

    const result = fallback.editApply({
      type: "moveOrDuplicateClipsToNewTrack",
      clipIds: [videoId, audioId],
      leadClipId: videoId,
      requestedFrameDelta: 40,
      insertAt: 0,
      mode: "duplicate",
    });
    const after = fallback.getTimeline();
    const copies = result.affectedClipIds.map((id) =>
      after.timeline.tracks.flatMap((track) => track.clips).find((clip) => clip.id === id),
    );

    expect(result.changed).toBe(true);
    expect(result.affectedClipIds).toHaveLength(2);
    expect(after.version).toBe(original.version + 1);
    expect(after.timeline.tracks.flatMap((track) => track.clips).map((clip) => clip.id)).toEqual(
      expect.arrayContaining([videoId, audioId, ...result.affectedClipIds]),
    );
    expect(copies[0]?.linkGroupId).toBeTruthy();
    expect(copies[1]?.linkGroupId).toBe(copies[0]?.linkGroupId);
    expect(copies[0]?.linkGroupId).not.toBe(originalGroup);
    expect(copies.map((clip) => clip?.startFrame)).toEqual([40, 40]);
  });

  it.each([0, 5])(
    "preserves linked A/V sources when a new-track duplicate overlaps by %i frames",
    (frameDelta) => {
      const fallback = createFallbackStore();
      fallback.reset();
      const placed = fallback.editApply({
        type: "placeMedia",
        target: { kind: "newTrack", trackType: "video" },
        entry: {
          mediaRef: "av-source",
          mediaType: "video",
          sourceClipType: "video",
          startFrame: 10,
          durationFrames: 20,
          hasAudio: true,
          addLinkedAudio: true,
        },
      });
      const [videoId, audioId] = placed.affectedClipIds;
      const before = fallback.getTimeline();
      const originalVideo = before.timeline.tracks
        .flatMap((track) => track.clips)
        .find((clip) => clip.id === videoId);
      const originalAudio = before.timeline.tracks
        .flatMap((track) => track.clips)
        .find((clip) => clip.id === audioId);
      const originalAudioTrackId = before.timeline.tracks.find((track) =>
        track.clips.some((clip) => clip.id === audioId),
      )?.id;

      const result = fallback.editApply({
        type: "moveOrDuplicateClipsToNewTrack",
        clipIds: [videoId, audioId],
        leadClipId: videoId,
        requestedFrameDelta: frameDelta,
        insertAt: 0,
        mode: "duplicate",
      });
      const after = fallback.getTimeline();
      const allClips = after.timeline.tracks.flatMap((track) => track.clips);
      const videoCopy = allClips.find((clip) => clip.id === result.affectedClipIds[0]);
      const audioCopy = allClips.find((clip) => clip.id === result.affectedClipIds[1]);
      const audioCopyTrackId = after.timeline.tracks.find((track) =>
        track.clips.some((clip) => clip.id === audioCopy?.id),
      )?.id;

      expect(allClips.find((clip) => clip.id === videoId)).toEqual(originalVideo);
      expect(allClips.find((clip) => clip.id === audioId)).toEqual(originalAudio);
      expect(videoCopy?.startFrame).toBe(10 + frameDelta);
      expect(audioCopy?.startFrame).toBe(10 + frameDelta);
      expect(videoCopy?.linkGroupId).toBe(audioCopy?.linkGroupId);
      expect(videoCopy?.linkGroupId).not.toBe(originalVideo?.linkGroupId);
      expect(audioCopyTrackId).not.toBe(originalAudioTrackId);
      expect(after.version).toBe(before.version + 1);
    },
  );

  it("pastes complete snapshots with fresh clip/group ids and bounded transitions", () => {
    const fallback = createFallbackStore();
    const before = fallback.getTimeline();
    const targetTrack = before.timeline.tracks[0];
    const first = structuredClone(targetTrack.clips[0]);
    const second = structuredClone(targetTrack.clips[1]);
    first.id = "clipboard-a";
    second.id = "clipboard-b";
    first.linkGroupId = second.linkGroupId = "clipboard-link";
    first.captionGroupId = second.captionGroupId = "clipboard-caption";
    first.reversed = true;
    first.speed = 1.25;
    first.effects = [{ name: "blur", params: { amount: 0.5 }, enabled: true }];
    first.opacityTrack = {
      keyframes: [{ frame: 0, value: 0.4, interpolationOut: "smooth" }],
    };
    first.transitionOut = {
      fromClipId: first.id,
      toClipId: second.id,
      kind: "crossDissolve",
      durationFrames: 8,
    };
    second.transitionOut = {
      fromClipId: second.id,
      toClipId: "outside-selection",
      kind: "crossDissolve",
      durationFrames: 8,
    };

    const result = fallback.editApply({
      type: "pasteClips",
      entries: [
        { clip: first, targetTrackId: targetTrack.id, startFrame: 200 },
        { clip: second, targetTrackId: targetTrack.id, startFrame: 230 },
      ],
    });
    const after = fallback.getTimeline();
    const copyA = after.timeline.tracks
      .flatMap((track) => track.clips)
      .find((clip) => clip.id === result.affectedClipIds[0]);
    const copyB = after.timeline.tracks
      .flatMap((track) => track.clips)
      .find((clip) => clip.id === result.affectedClipIds[1]);

    expect(result.affectedClipIds).toHaveLength(2);
    expect(after.version).toBe(before.version + 1);
    expect(copyA).toEqual({
      ...first,
      id: result.affectedClipIds[0],
      startFrame: 200,
      linkGroupId: copyA?.linkGroupId,
      captionGroupId: copyA?.captionGroupId,
      transitionOut: {
        ...first.transitionOut,
        fromClipId: result.affectedClipIds[0],
        toClipId: result.affectedClipIds[1],
      },
    });
    expect(copyB).toEqual({
      ...second,
      id: result.affectedClipIds[1],
      startFrame: 230,
      linkGroupId: copyB?.linkGroupId,
      captionGroupId: copyB?.captionGroupId,
      transitionOut: undefined,
    });
    expect(copyA?.linkGroupId).toBe(copyB?.linkGroupId);
    expect(copyA?.linkGroupId).not.toBe("clipboard-link");
    expect(copyA?.captionGroupId).toBe(copyB?.captionGroupId);
    expect(copyA?.captionGroupId).not.toBe("clipboard-caption");
  });

  it("rejects invalid paste destinations without consuming a version", () => {
    const fallback = createFallbackStore();
    const before = fallback.getTimeline();

    const result = fallback.editApply({
      type: "pasteClips",
      entries: [{
        clip: before.timeline.tracks[0].clips[0],
        targetTrackId: "missing-track",
        startFrame: 0,
      }],
    });

    expect(result.changed).toBe(false);
    expect(fallback.getTimeline()).toEqual(before);
  });

  it("rejects i32-boundary timeline gestures atomically without consuming ids", () => {
    const fallback = createFallbackStore();
    const before = fallback.getTimeline();

    const duplicate = fallback.editApply({
      type: "duplicateClips",
      clipIds: ["c2"],
      offsetFrames: 2_147_483_647,
      targetTrackIndexes: [0],
    });
    const move = fallback.editApply({
      type: "moveClips",
      moves: [{ clipId: "c1", toTrack: 0, toFrame: 2_147_483_647 }],
    });
    const insert = fallback.editApply({
      type: "insertClips",
      trackIndex: 0,
      atFrame: 2_147_483_647,
      entries: [{
        mediaRef: "overflow",
        mediaType: "video",
        sourceClipType: "video",
        trackIndex: 0,
        startFrame: 0,
        durationFrames: 1,
      }],
    });

    expect([duplicate.changed, move.changed, insert.changed]).toEqual([false, false, false]);
    expect(fallback.getTimeline()).toEqual(before);
    expect(fallback.editApply({ type: "insertTrack", kind: "video" }).affectedClipIds).toEqual(["t100"]);
  });

  it("rejects overflowing swaps, property retimes, and FPS projections atomically", () => {
    const base = createFallbackStore().getTimeline().timeline;
    const first = structuredClone(base.tracks[0].clips[0]);
    const second = structuredClone(base.tracks[0].clips[1]);
    first.id = "first";
    first.startFrame = 0;
    first.durationFrames = 10;
    second.id = "second";
    second.mediaType = "video";
    second.sourceClipType = "video";
    second.startFrame = 2_147_483_646;
    second.durationFrames = 1;
    const swapTimeline = structuredClone(base);
    swapTimeline.tracks = [{ ...swapTimeline.tracks[0], clips: [first, second] }];
    const swapStore = createFallbackStore(swapTimeline);
    const beforeSwap = swapStore.getTimeline();
    expect(swapStore.editApply({ type: "swapClips", clipA: "first", clipB: "second" }).changed).toBe(false);
    expect(swapStore.getTimeline()).toEqual(beforeSwap);

    const propertiesStore = createFallbackStore();
    const beforeProperties = propertiesStore.getTimeline();
    expect(propertiesStore.editApply({
      type: "setClipProperties",
      clipIds: ["c1"],
      properties: { speed: Number.MIN_VALUE },
    }).changed).toBe(false);
    expect(propertiesStore.getTimeline()).toEqual(beforeProperties);

    const projected = structuredClone(base);
    projected.tracks = [{
      ...projected.tracks[0],
      clips: [{ ...first, startFrame: 1_073_741_824, durationFrames: 1 }],
    }];
    const settingsStore = createFallbackStore(projected);
    const beforeSettings = settingsStore.getTimeline();
    expect(settingsStore.editApply({
      type: "setTimelineSettings",
      fps: 60,
      width: 1920,
      height: 1080,
    }).changed).toBe(false);
    expect(settingsStore.getTimeline()).toEqual(beforeSettings);
  });

  it("rejects malformed root or nested loaded clips before any edit", () => {
    for (const nested of [false, true]) {
      const timeline = createFallbackStore().getTimeline().timeline;
      const malformed = structuredClone(timeline.tracks[0].clips[0]);
      malformed.startFrame = 2_147_483_647;
      malformed.durationFrames = 1;
      if (nested) {
        const child = structuredClone(timeline);
        child.tracks = [{ ...child.tracks[0], clips: [malformed] }];
        timeline.nestedSequences = [{ id: "bad-child", name: "Bad", timeline: child }];
      } else {
        timeline.tracks[0].clips.push(malformed);
      }
      const fallback = createFallbackStore(timeline);
      const before = fallback.getTimeline();
      expect(fallback.editApply({ type: "insertTrack", kind: "video" }).changed).toBe(false);
      expect(fallback.getTimeline()).toEqual(before);
    }
  });

  it.each(["image", "text"] as const)(
    "keeps negative %s trims editable while rejecting per-edge overflow",
    (mediaType) => {
      const timeline = createFallbackStore().getTimeline().timeline;
      const extended = structuredClone(timeline.tracks[0].clips[0]);
      extended.id = "extended";
      extended.mediaType = mediaType;
      extended.sourceClipType = mediaType;
      extended.startFrame = 0;
      extended.durationFrames = 30;
      extended.trimStartFrame = -10;
      extended.trimEndFrame = -5;
      timeline.tracks = [{ ...timeline.tracks[0], clips: [extended] }];
      const fallback = createFallbackStore(timeline);
      const duplicate = fallback.editApply({
        type: "duplicateClips",
        clipIds: ["extended"],
        offsetFrames: 50,
        targetTrackIndexes: [0],
      });
      const copy = fallback.getTimeline().timeline.tracks[0].clips.find(
        (clip) => clip.id === duplicate.affectedClipIds[0],
      );
      expect(duplicate.changed).toBe(true);
      expect([copy?.trimStartFrame, copy?.trimEndFrame]).toEqual([-10, -5]);

      const unsafeTimeline = structuredClone(timeline);
      unsafeTimeline.tracks[0].clips[0].trimStartFrame = -100;
      unsafeTimeline.tracks[0].clips[0].trimEndFrame = 2_147_483_642;
      const unsafe = createFallbackStore(unsafeTimeline);
      const before = unsafe.getTimeline();
      expect(unsafe.editApply({ type: "insertTrack", kind: "video" }).changed).toBe(false);
      expect(unsafe.getTimeline()).toEqual(before);
    },
  );

  it("remaps duplicate transition endpoints for direct and new-track gestures", () => {
    const timeline = createFallbackStore().getTimeline().timeline;
    const first = { ...structuredClone(timeline.tracks[0].clips[0]), id: "first", startFrame: 0, durationFrames: 20 };
    const second = { ...structuredClone(timeline.tracks[0].clips[0]), id: "second", startFrame: 20, durationFrames: 20 };
    first.transitionOut = {
      fromClipId: "first",
      toClipId: "second",
      kind: "crossDissolve",
      durationFrames: 8,
    };
    timeline.tracks = [{ ...timeline.tracks[0], clips: [first, second] }];

    for (const newTrack of [false, true]) {
      const fallback = createFallbackStore(timeline);
      const result = newTrack
        ? fallback.editApply({
            type: "moveOrDuplicateClipsToNewTrack",
            clipIds: ["first", "second"],
            leadClipId: "first",
            requestedFrameDelta: 100,
            insertAt: 0,
            mode: "duplicate",
          })
        : fallback.editApply({
            type: "duplicateClips",
            clipIds: ["first", "second"],
            offsetFrames: 100,
            targetTrackIndexes: [0, 0],
          });
      const clips = fallback.getTimeline().timeline.tracks.flatMap((track) => track.clips);
      const firstCopy = clips.find((clip) => clip.id === result.affectedClipIds[0]);
      expect(firstCopy?.transitionOut).toEqual({
        fromClipId: result.affectedClipIds[0],
        toClipId: result.affectedClipIds[1],
        kind: "crossDissolve",
        durationFrames: 8,
      });
    }
  });

  it("splits linked partners and an independent seed as one deduplicated batch", () => {
    const timeline = createFallbackStore().getTimeline().timeline;
    const video = structuredClone(timeline.tracks[0].clips[0]);
    video.id = "video";
    video.startFrame = 100;
    video.durationFrames = 60;
    video.trimStartFrame = 5;
    video.trimEndFrame = 7;
    video.fadeInFrames = 4;
    video.fadeOutFrames = 6;
    video.linkGroupId = "original-link";
    video.opacityTrack = {
      keyframes: [
        { frame: 0, value: 0, interpolationOut: "linear" },
        { frame: 60, value: 1, interpolationOut: "smooth" },
      ],
    };
    video.transitionOut = {
      fromClipId: "video",
      toClipId: "later",
      kind: "crossDissolve",
      durationFrames: 5,
    };

    const audio = structuredClone(timeline.tracks[1].clips[0]);
    audio.id = "audio";
    audio.startFrame = 100;
    audio.durationFrames = 60;
    audio.linkGroupId = "original-link";

    const solo = structuredClone(timeline.tracks[0].clips[0]);
    solo.id = "solo";
    solo.startFrame = 90;
    solo.durationFrames = 80;
    solo.linkGroupId = undefined;

    const outsidePartner = structuredClone(timeline.tracks[0].clips[0]);
    outsidePartner.id = "outside-partner";
    outsidePartner.startFrame = 200;
    outsidePartner.durationFrames = 30;
    outsidePartner.linkGroupId = "original-link";

    timeline.tracks = [
      { ...timeline.tracks[0], id: "video-track", clips: [video] },
      { ...timeline.tracks[1], id: "audio-track", clips: [audio] },
      { ...timeline.tracks[0], id: "overlay-track", clips: [solo, outsidePartner] },
    ];
    const fallback = createFallbackStore(timeline);

    const result = fallback.editApply({
      type: "splitClips",
      clipIds: ["video", "audio", "video", "solo"],
      atFrame: 130,
    });
    const snapshot = fallback.getTimeline();
    const clips = snapshot.timeline.tracks.flatMap((track) => track.clips);
    const videoLeft = clips.find((clip) => clip.id === "video")!;
    const audioLeft = clips.find((clip) => clip.id === "audio")!;
    const videoRight = clips.find(
      (clip) => result.affectedClipIds.includes(clip.id) && clip.mediaRef === video.mediaRef,
    )!;
    const audioRight = clips.find(
      (clip) => result.affectedClipIds.includes(clip.id) && clip.mediaType === "audio",
    )!;

    expect(result.changed).toBe(true);
    expect(result.actionName).toBe("Split Clips");
    expect(result.affectedClipIds).toHaveLength(3);
    expect(snapshot.version).toBe(1);
    expect(videoLeft).toMatchObject({
      startFrame: 100,
      durationFrames: 30,
      trimStartFrame: 5,
      trimEndFrame: 37,
      fadeInFrames: 4,
      fadeOutFrames: 0,
      linkGroupId: "original-link",
      transitionOut: undefined,
    });
    expect(videoRight).toMatchObject({
      startFrame: 130,
      durationFrames: 30,
      trimStartFrame: 35,
      trimEndFrame: 7,
      fadeInFrames: 0,
      fadeOutFrames: 6,
      transitionOut: undefined,
    });
    expect(videoRight.linkGroupId).toBeTruthy();
    expect(videoRight.linkGroupId).toBe(audioRight.linkGroupId);
    expect(videoRight.linkGroupId).not.toBe("original-link");
    expect(audioLeft.linkGroupId).toBe("original-link");
    const videoLeftOpacityKeyframes = videoLeft.opacityTrack?.keyframes ?? [];
    expect(videoLeftOpacityKeyframes[videoLeftOpacityKeyframes.length - 1]).toMatchObject({
      frame: 30,
      value: 0.5,
    });
    expect(videoRight.opacityTrack?.keyframes[0]).toMatchObject({ frame: 0, value: 0.5 });
    expect(clips.filter((clip) => clip.id === "outside-partner")).toEqual([outsidePartner]);
    expect(clips.find((clip) => clip.id === "solo")?.durationFrames).toBe(40);
  });

  it("rejects an invalid split batch without mutation or id consumption", () => {
    const fallback = createFallbackStore();
    const before = fallback.getTimeline();

    const result = fallback.editApply({
      type: "splitClips",
      clipIds: ["c1", "missing", "c1"],
      atFrame: 30,
    });

    expect(result.changed).toBe(false);
    expect(fallback.getTimeline()).toEqual(before);
    expect(fallback.editApply({ type: "insertTrack", kind: "video" }).affectedClipIds).toEqual(["t100"]);
  });

  it("sets static transform fields while keyframing only active lanes", () => {
    const timeline = createFallbackStore().getTimeline().timeline;
    const animated = structuredClone(timeline.tracks[0].clips[0]);
    animated.id = "animated";
    animated.startFrame = 100;
    animated.durationFrames = 60;
    animated.transform = {
      centerX: 0.25,
      centerY: 0.35,
      width: 0.8,
      height: 0.6,
      rotation: 5,
      flipHorizontal: false,
      flipVertical: true,
    };
    animated.positionTrack = {
      keyframes: [{ frame: 0, value: { a: 0, b: 0.05 }, interpolationOut: "smooth" }],
    };
    animated.scaleTrack = { keyframes: [] };
    animated.rotationTrack = {
      keyframes: [{ frame: 0, value: 10, interpolationOut: "smooth" }],
    };
    timeline.tracks = [{ ...timeline.tracks[0], clips: [animated] }];
    const fallback = createFallbackStore(timeline);

    const result = fallback.editApply({
      type: "setTransformAtFrame",
      clipId: "animated",
      frame: 130,
      transform: {
        centerX: 0.7,
        centerY: 0.6,
        width: 0.4,
        height: 0.25,
        rotation: 33,
        flipHorizontal: true,
        flipVertical: false,
      },
    });
    const changed = fallback.getTimeline().timeline.tracks[0].clips[0];

    expect(result).toMatchObject({ changed: true, actionName: "Change Transform" });
    const positionKeyframes = changed.positionTrack?.keyframes ?? [];
    const positionKeyframe = positionKeyframes[positionKeyframes.length - 1];
    expect(positionKeyframe).toMatchObject({ frame: 30, interpolationOut: "smooth" });
    expect(positionKeyframe?.value.a).toBeCloseTo(0.5);
    expect(positionKeyframe?.value.b).toBeCloseTo(0.475);
    const rotationKeyframes = changed.rotationTrack?.keyframes ?? [];
    expect(rotationKeyframes[rotationKeyframes.length - 1]).toEqual({
      frame: 30,
      value: 33,
      interpolationOut: "smooth",
    });
    expect(changed.scaleTrack).toEqual({ keyframes: [] });
    expect(changed.transform).toEqual({
      centerX: 0.25,
      centerY: 0.35,
      width: 0.4,
      height: 0.25,
      rotation: 5,
      flipHorizontal: true,
      flipVertical: false,
    });
  });

  it("rejects invalid animated transform frames and non-finite values atomically", () => {
    const timeline = createFallbackStore().getTimeline().timeline;
    timeline.tracks[0].clips[0].positionTrack = {
      keyframes: [{ frame: 0, value: { a: 0, b: 0 }, interpolationOut: "smooth" }],
    };
    const fallback = createFallbackStore(timeline);
    const target = {
      centerX: 0.5,
      centerY: 0.5,
      width: 1,
      height: 1,
      rotation: 0,
      flipHorizontal: false,
      flipVertical: false,
    };

    const commands: EditRequest[] = [
      { type: "setTransformAtFrame", clipId: "c1", frame: 90, transform: target },
      { type: "setTransformAtFrame", clipId: "c1", frame: Number.NaN, transform: target },
      {
        type: "setTransformAtFrame",
        clipId: "c1",
        frame: 30,
        transform: { ...target, centerX: Number.NaN },
      },
      {
        type: "setTransformAtFrame",
        clipId: "c1",
        frame: 30,
        transform: { ...target, centerX: -Number.MAX_VALUE, width: Number.MAX_VALUE },
      },
    ];
    for (const command of commands) {
      const before = fallback.getTimeline();
      expect(fallback.editApply(command).changed).toBe(false);
      expect(fallback.getTimeline()).toEqual(before);
    }
  });

  it("does not emulate swapMedia without the Tauri media manifest", () => {
    const fallback = createFallbackStore();

    const result = fallback.editApply({
      type: "swapMedia",
      clipId: "c1",
      mediaRef: "replacement",
    });
    const clip = fallback
      .getTimeline()
      .timeline.tracks.flatMap((track) => track.clips)
      .find((candidate) => candidate.id === "c1");

    expect(result.changed).toBe(false);
    expect(clip?.mediaRef).toBe("demo-video");
  });
});
