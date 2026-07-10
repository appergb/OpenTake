import { describe, expect, it } from "vitest";
import { decodePlaybackCommandError, decodePlaybackFrameEvent } from "./api";

describe("playback IPC decoding", () => {
  it("decodes the full playback frame identity instead of accepting frame only", () => {
    expect(
      decodePlaybackFrameEvent({
        projectEpoch: 9,
        timelineVersion: 14,
        sessionId: "session-9",
        frame: 22,
        sequence: 7,
        terminal: false,
      }),
    ).toEqual({
      projectEpoch: 9,
      timelineVersion: 14,
      sessionId: "session-9",
      frame: 22,
      sequence: 7,
      terminal: false,
    });
    expect(decodePlaybackFrameEvent({ frame: 22 })).toBeNull();
  });

  it("rejects malformed frame fields instead of inferring defaults", () => {
    const valid = {
      projectEpoch: 9,
      timelineVersion: 14,
      sessionId: "session-9",
      frame: 22,
      sequence: 7,
      terminal: false,
    };
    for (const key of Object.keys(valid)) {
      expect(
        decodePlaybackFrameEvent({ ...valid, [key]: undefined }),
        key,
      ).toBeNull();
    }
  });

  it("decodes only the four structured playback command errors", () => {
    for (const code of ["superseded", "cancelled", "busy", "engine"] as const) {
      expect(decodePlaybackCommandError({ code, message: "detail" })).toEqual({
        code,
        message: "detail",
      });
    }
    expect(decodePlaybackCommandError("plain string")).toBeNull();
    expect(decodePlaybackCommandError({ code: "unknown", message: "detail" })).toBeNull();
  });
});
