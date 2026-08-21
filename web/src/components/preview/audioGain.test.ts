import { describe, expect, it, vi } from "vitest";
import { createPreviewAudioGainController } from "./audioGain";

interface FakeElement {
  muted: boolean;
  volume: number;
}

function fakeElement(): HTMLMediaElement {
  return { muted: false, volume: 1 } as unknown as HTMLMediaElement;
}

function fakeAudioContext() {
  const gain = { gain: { value: 1 }, connect: vi.fn(), disconnect: vi.fn() };
  const source = { connect: vi.fn(), disconnect: vi.fn() };
  const context = {
    state: "suspended" as AudioContextState,
    destination: {},
    createMediaElementSource: vi.fn(() => source),
    createGain: vi.fn(() => gain),
    resume: vi.fn(async () => undefined),
    close: vi.fn(async () => undefined),
  };
  return { context, gain, source };
}

describe("preview audio gain controller", () => {
  it("routes boosted gain through one reusable Web Audio graph", () => {
    const { context, gain, source } = fakeAudioContext();
    const element = fakeElement();
    const controller = createPreviewAudioGainController(() => context as unknown as AudioContext);

    controller.setGain(element, 4, false);
    controller.setGain(element, 2, false);

    expect(context.createMediaElementSource).toHaveBeenCalledTimes(1);
    expect(context.createGain).toHaveBeenCalledTimes(1);
    expect(source.connect).toHaveBeenCalledWith(gain);
    expect(gain.connect).toHaveBeenCalledWith(context.destination);
    expect(gain.gain.value).toBe(2);
    expect(element.muted).toBe(true);
  });

  it("temporarily mutes and restores the requested gain without rebuilding nodes", () => {
    const { context, gain } = fakeAudioContext();
    const element = fakeElement();
    const controller = createPreviewAudioGainController(() => context as unknown as AudioContext);

    controller.setGain(element, 3, false);
    controller.setMuted(element, true);
    expect(gain.gain.value).toBe(0);
    controller.setMuted(element, false);
    expect(gain.gain.value).toBe(3);
    expect(context.createMediaElementSource).toHaveBeenCalledTimes(1);
  });

  it("falls back to the media element volume when Web Audio cannot start", () => {
    const element = fakeElement();
    const controller = createPreviewAudioGainController(() => {
      throw new Error("AudioContext unavailable");
    });

    controller.setGain(element, 4, false);

    expect(element.volume).toBe(1);
    expect(element.muted).toBe(false);
  });

  it("disconnects the graph when an element leaves the preview", () => {
    const { context, gain, source } = fakeAudioContext();
    const element = fakeElement();
    const controller = createPreviewAudioGainController(() => context as unknown as AudioContext);

    controller.setGain(element, 2, false);
    controller.remove(element);

    expect(source.disconnect).toHaveBeenCalledTimes(1);
    expect(gain.disconnect).toHaveBeenCalledTimes(1);
  });
});
