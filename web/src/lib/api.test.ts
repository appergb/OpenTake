import { describe, expect, it } from "vitest";
import {
  accountGetBackendUrl,
  accountGetStatus,
  accountLogin,
  accountLogout,
  accountSetBackendUrl,
  decodePlaybackCommandError,
  decodePlaybackFrameEvent,
  decodePrewarmResult,
  externalMcpPair,
  externalMcpRegenerate,
  externalMcpRevoke,
  externalMcpSetEnabled,
  externalMcpStatus,
  getTimeline,
  onExternalMcpStatusChanged,
  projectNew,
  projectOpen,
} from "./api";

describe("browser external MCP safety defaults", () => {
  it("reports the endpoint disabled without inventing clients or credentials", async () => {
    await expect(externalMcpStatus()).resolves.toEqual({
      revision: 0,
      enabled: false,
      state: "disabled",
      endpoint: "http://127.0.0.1:19789/mcp",
      clients: [],
      error: null,
    });
  });

  it("rejects every durable pairing mutation outside the desktop shell", async () => {
    await expect(externalMcpSetEnabled(true)).rejects.toThrow("desktop app");
    await expect(externalMcpPair("Cursor")).rejects.toThrow("desktop app");
    await expect(externalMcpRegenerate("client-1")).rejects.toThrow("desktop app");
    await expect(externalMcpRevoke("client-1")).rejects.toThrow("desktop app");
  });

  it("returns a harmless listener disposer outside the desktop shell", async () => {
    let calls = 0;
    const dispose = await onExternalMcpStatusChanged(() => {
      calls += 1;
    });
    dispose();
    expect(calls).toBe(0);
  });
});

describe("browser account scaffold defaults", () => {
  it("stays offline and performs no login outside the desktop shell", async () => {
    await expect(accountGetBackendUrl()).resolves.toBeNull();
    await expect(accountGetStatus()).resolves.toEqual({ type: "offline" });
    await expect(accountSetBackendUrl("https://accounts.example.com")).resolves.toBeUndefined();
    await expect(accountLogout()).resolves.toBeUndefined();
    await expect(accountLogin("token")).rejects.toThrow(
      "account login requires the desktop app",
    );
  });
});

describe("browser project snapshot compatibility defaults", () => {
  it("marks every fallback snapshot as a known writable in-memory project", async () => {
    for (const snapshot of [
      await getTimeline(),
      await projectNew(),
      await projectOpen("/tmp/browser.opentake"),
    ]) {
      expect(snapshot.projectPath).toBeNull();
      expect(snapshot.compatibilityReadOnly).toBe(false);
      expect(snapshot.compatibilityBlockers).toEqual([]);
    }
  });
});

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

describe("media prewarm IPC decoding", () => {
  it("accepts only the five structured prewarm admission results", () => {
    for (const result of ["queued", "duplicate", "cached", "busy", "staleProject"] as const) {
      expect(decodePrewarmResult?.(result)).toBe(result);
    }
    expect(decodePrewarmResult?.("stale_project")).toBeNull();
    expect(decodePrewarmResult?.({ result: "queued" })).toBeNull();
  });
});
