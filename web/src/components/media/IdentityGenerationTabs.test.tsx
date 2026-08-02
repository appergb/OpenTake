// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useI18nStore } from "../../i18n";
import type { AvatarGenerationResult, MediaItem, VoiceCloneResult } from "../../lib/types";
import { useMediaStore } from "../../store/mediaStore";
import { useProjectStore } from "../../store/projectStore";
import { AvatarGenerationTab, type AvatarDependencies, VoiceCloneTab, type VoiceDependencies } from "./IdentityGenerationTabs";

vi.mock("../../lib/asset", () => ({ assetUrl: (path: string | null | undefined) => (path ? `asset://${path}` : null) }));

function setValue(element: HTMLInputElement | HTMLTextAreaElement, value: string): void {
  const prototype = element instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
  Object.getOwnPropertyDescriptor(prototype, "value")?.set?.call(element, value);
  element.dispatchEvent(new Event("input", { bubbles: true }));
}

function media(id: string, type: "image" | "audio"): MediaItem {
  return { id, name: id, type, duration: type === "audio" ? 1 : 0, hasAudio: type === "audio", favorite: false };
}

function checkbox(container: HTMLElement, index: number): HTMLInputElement {
  return Array.from(container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'))[index];
}

function button(container: HTMLElement, label: string): HTMLButtonElement {
  return Array.from(container.querySelectorAll<HTMLButtonElement>("button")).find((candidate) => candidate.textContent?.includes(label))!;
}

describe("identity generation workflows", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useI18nStore.setState({ locale: "en" });
    useMediaStore.setState({ items: [media("portrait", "image"), media("voice-ref", "audio")], folders: [], importing: false, error: null });
    useProjectStore.setState({ timeline: { fps: 30, width: 1920, height: 1080, settingsConfigured: true, tracks: [], voiceModels: [] } });
    container = document.createElement("div"); document.body.append(container); root = createRoot(container);
  });

  afterEach(async () => { await act(async () => root.unmount()); container.remove(); });

  it("requires recorded consent and cost, imports a preview, supports undo, and isolates a cancelled stale avatar result", async () => {
    let resolve: ((value: AvatarGenerationResult) => void) | undefined;
    const run = vi.fn(() => new Promise<AvatarGenerationResult>((done) => { resolve = done; }));
    const cancel = vi.fn().mockResolvedValue(true);
    const undo = vi.fn().mockResolvedValue(undefined);
    const dependencies: AvatarDependencies = { run, cancel, undo };
    await act(async () => root.render(<AvatarGenerationTab dependencies={dependencies} />));
    expect(button(container, "Generate & Import Avatar").disabled).toBe(true);
    await act(async () => { checkbox(container, 0).click(); checkbox(container, 1).click(); });
    await act(async () => button(container, "Generate & Import Avatar").click());
    expect(run).toHaveBeenCalledWith(expect.objectContaining({ portraitMediaRef: "portrait", audioMediaRef: "voice-ref", costAuthorized: true, consentId: expect.stringMatching(/^consent-avatar-/) }));
    await act(async () => button(container, "Cancel").click());
    expect(cancel).toHaveBeenCalledOnce();
    await act(async () => resolve?.({ result: { assetId: "avatar", clipIds: ["clip"], previewPath: "/tmp/stale.mp4", provider: "fal", model: "sync", providerRequestId: "request", requestHash: "a".repeat(64), consentId: "consent", portraitMediaRef: "portrait", audioMediaRef: "voice-ref", durationFrames: 30, mediaType: "video/mp4", imported: true }, actionName: "Generate Avatar" }));
    expect(container.querySelector("video")).toBeNull();

    run.mockResolvedValueOnce({ result: { assetId: "avatar", clipIds: ["clip"], previewPath: "/tmp/avatar.mp4", provider: "fal", model: "sync", providerRequestId: "request", requestHash: "a".repeat(64), consentId: "consent", portraitMediaRef: "portrait", audioMediaRef: "voice-ref", durationFrames: 30, mediaType: "video/mp4", imported: true }, actionName: "Generate Avatar" });
    await act(async () => button(container, "Generate & Import Avatar").click());
    expect(container.querySelector("video")?.getAttribute("src")).toContain("avatar.mp4");
    expect(container.textContent).toContain("30 frames");
    await act(async () => button(container, "Undo Avatar").click());
    expect(undo).toHaveBeenCalledOnce();
  });

  it("opens voice cloning for legacy timelines that omit voiceModels", async () => {
    useProjectStore.setState({
      timeline: {
        fps: 30,
        width: 1920,
        height: 1080,
        settingsConfigured: true,
        tracks: [],
      },
    });
    const dependencies: VoiceDependencies = {
      run: vi.fn(),
      cancel: vi.fn().mockResolvedValue(true),
      undo: vi.fn().mockResolvedValue(undefined),
    };

    await act(async () => root.render(<VoiceCloneTab dependencies={dependencies} />));

    expect(container.querySelector('[data-testid="voice-clone-tab"]')).not.toBeNull();
    expect(button(container, "Enroll Voice Model").disabled).toBe(true);
  });

  it("enrolls, generates, auditions, undoes, and revokes a consented voice", async () => {
    const run = vi.fn(async (request: Parameters<VoiceDependencies["run"]>[0]): Promise<VoiceCloneResult> => {
      if (request.action === "enroll") return { result: { action: "enroll", voiceId: "voice-model-1", voiceName: request.voiceName, provider: "elevenlabs", model: "eleven_multilingual_v2", consentId: request.consentId }, actionName: "Enroll Voice Clone" };
      if (request.action === "generate") return { result: { action: "generate", voiceId: request.voiceId!, assetId: "audio-out", clipIds: ["clip"], previewPath: "/tmp/voice.mp3", provider: "elevenlabs", model: "eleven_multilingual_v2", consentId: request.consentId, durationFrames: 30, imported: true }, actionName: "Generate Cloned Voice" };
      return { result: { action: "revoke", voiceId: request.voiceId!, provider: "elevenlabs", consentId: request.consentId, revoked: true }, actionName: "Revoke Voice Clone" };
    });
    const dependencies: VoiceDependencies = { run, cancel: vi.fn().mockResolvedValue(true), undo: vi.fn().mockResolvedValue(undefined) };
    await act(async () => root.render(<VoiceCloneTab dependencies={dependencies} />));
    const name = container.querySelector<HTMLInputElement>('input[aria-label="Voice name"]')!;
    await act(async () => { setValue(name, "Narrator"); checkbox(container, 0).click(); checkbox(container, 1).click(); });
    await act(async () => button(container, "Enroll Voice Model").click());
    expect(run).toHaveBeenLastCalledWith(expect.objectContaining({ action: "enroll", referenceAudioMediaRef: "voice-ref", voiceName: "Narrator", costAuthorized: true, consentId: expect.stringMatching(/^consent-voice-/) }));
    const prompt = container.querySelector<HTMLTextAreaElement>('textarea[aria-label="Text to speak"]')!;
    await act(async () => setValue(prompt, "Hello"));
    await act(async () => button(container, "Generate, Audition & Import").click());
    expect(container.querySelector("audio")?.getAttribute("src")).toContain("voice.mp3");
    await act(async () => button(container, "Undo Generated Audio").click());
    expect(dependencies.undo).toHaveBeenCalledOnce();
    await act(async () => button(container, "Permanently Revoke Voice").click());
    expect(run).toHaveBeenLastCalledWith(expect.objectContaining({ action: "revoke", voiceId: "voice-model-1" }));
    expect(container.textContent).toContain("cannot generate again");
  });
});
