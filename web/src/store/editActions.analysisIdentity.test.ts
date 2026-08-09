import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  AudioDenoise,
  LoudnessNormalization,
  StabilizationTrack,
} from "../lib/types";

const mocks = vi.hoisted(() => ({
  analyzeLoudness: vi.fn(),
  prepareDenoise: vi.fn(),
  analyzeStabilization: vi.fn(),
  editApply: vi.fn(),
}));

vi.mock("../lib/api", () => ({
  isTauri: true,
  analyzeLoudness: mocks.analyzeLoudness,
  prepareDenoise: mocks.prepareDenoise,
  analyzeStabilization: mocks.analyzeStabilization,
  editApply: mocks.editApply,
  cancelLoudnessAnalysis: vi.fn(),
  cancelDenoiseAnalysis: vi.fn(),
  cancelStabilizationAnalysis: vi.fn(),
}));

import {
  analyzeAndApplyLoudness,
  analyzeAndApplyStabilization,
  prepareAndApplyAudioDenoise,
} from "./editActions";
import { useEditorUiStore } from "./uiStore";
import { useProjectStore } from "./projectStore";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

const identityA = {
  projectEpoch: 7,
  projectPath: "/tmp/A.opentake",
  timelineVersion: 11,
};

describe("long media analysis edit authority", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.editApply.mockResolvedValue({
      changed: true,
      actionName: "Apply analysis",
      affectedClipIds: ["clip"],
      timelineVersion: 12,
      summary: "",
    });
    useProjectStore.setState(identityA);
    useEditorUiStore.setState({ activeNestedSequenceId: "sequence-a" });
  });

  it("keeps loudness bound to the project, version, and nested sequence captured before analysis", async () => {
    const pending = deferred<LoudnessNormalization>();
    const normalization = {
      targetLufs: -16,
      measuredLufs: -22,
      gainDb: 6,
      truePeakCeilingDbtp: -1,
    } satisfies LoudnessNormalization;
    mocks.analyzeLoudness.mockReturnValueOnce(pending.promise);

    const run = analyzeAndApplyLoudness("clip", -16, -1);
    useProjectStore.setState({
      projectEpoch: 8,
      projectPath: "/tmp/B.opentake",
      timelineVersion: 11,
    });
    useEditorUiStore.setState({ activeNestedSequenceId: "sequence-b" });
    pending.resolve(normalization);
    await run;

    expect(mocks.editApply).toHaveBeenCalledWith(
      {
        type: "editNestedSequence",
        sequenceId: "sequence-a",
        command: { type: "setLoudnessNormalization", clipId: "clip", normalization },
      },
      identityA,
    );
  });

  it("does not re-authorize denoise after the source clip changes", async () => {
    const pending = deferred<AudioDenoise>();
    const denoise = {
      mode: "voice",
      strength: 0.8,
      previewEnabled: true,
    } satisfies AudioDenoise;
    mocks.prepareDenoise.mockReturnValueOnce(pending.promise);

    const run = prepareAndApplyAudioDenoise("clip", "voice", 0.8, true);
    useProjectStore.setState({ timelineVersion: 12 });
    pending.resolve(denoise);
    await run;

    expect(mocks.editApply).toHaveBeenCalledWith(
      {
        type: "editNestedSequence",
        sequenceId: "sequence-a",
        command: { type: "setAudioDenoise", clipId: "clip", denoise },
      },
      identityA,
    );
  });

  it("keeps stabilization bound to the authority captured before analysis", async () => {
    const pending = deferred<StabilizationTrack>();
    const solution = {
      sourceIdentity: "source-a",
      strength: 1,
      cropMargin: 0,
      samples: [],
    } satisfies StabilizationTrack;
    mocks.analyzeStabilization.mockReturnValueOnce(pending.promise);

    const run = analyzeAndApplyStabilization("clip");
    useProjectStore.setState({
      projectEpoch: 8,
      projectPath: "/tmp/B.opentake",
      timelineVersion: 11,
    });
    pending.resolve(solution);
    await run;

    expect(mocks.editApply).toHaveBeenCalledWith(
      {
        type: "editNestedSequence",
        sequenceId: "sequence-a",
        command: { type: "applyStabilization", clipId: "clip", solution },
      },
      identityA,
    );
  });
});
