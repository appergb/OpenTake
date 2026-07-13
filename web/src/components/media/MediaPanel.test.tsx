// @vitest-environment happy-dom

import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../../lib/api", () => ({
  getWaveform: vi.fn(),
}));

import { AudioWaveform, MediaFavoriteButton } from "./MediaPanel";

afterEach(() => {
  document.body.replaceChildren();
});

describe("AudioWaveform", () => {
  const fallback = <span data-testid="audio-fallback">fallback</span>;

  it("renders waveform bars when normalized buckets are available", () => {
    const html = renderToStaticMarkup(
      <AudioWaveform mediaRef="audio-1" fallback={fallback} bucketsOverride={[0, 0.25, 0.5, 0.75, 1]} />,
    );

    expect(html).toContain('data-testid="audio-waveform"');
    expect(html).not.toContain('data-testid="audio-fallback"');
  });

  it("renders the fallback when waveform loading resolves to null", () => {
    const html = renderToStaticMarkup(
      <AudioWaveform mediaRef="audio-1" fallback={fallback} bucketsOverride={null} />,
    );

    expect(html).not.toContain('data-testid="audio-waveform"');
    expect(html).toContain('data-testid="audio-fallback"');
  });

  it("renders the fallback when waveform loading resolves to an empty array", () => {
    const html = renderToStaticMarkup(
      <AudioWaveform mediaRef="audio-1" fallback={fallback} bucketsOverride={[]} />,
    );

    expect(html).not.toContain('data-testid="audio-waveform"');
    expect(html).toContain('data-testid="audio-fallback"');
  });
});

describe("MediaFavoriteButton", () => {
  it("disables while pending and keeps the rendered star unchanged after rejection", async () => {
    let rejectToggle: (reason: unknown) => void = () => undefined;
    const pendingToggle = new Promise<never>((_resolve, reject) => {
      rejectToggle = reject;
    });
    const performToggle = vi.fn(() => pendingToggle);
    const onSuccess = vi.fn();

    function Harness() {
      const [feedback, setFeedback] = React.useState<string | null>(null);
      return (
        <>
          <MediaFavoriteButton
            assetId="asset-1"
            favorite
            title="Unfavorite"
            onSuccess={onSuccess}
            onError={setFeedback}
            performToggle={performToggle}
          />
          {feedback && <span role="alert">{feedback}</span>}
        </>
      );
    }

    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    await act(async () => root.render(<Harness />));
    const button = container.querySelector<HTMLButtonElement>("button")!;

    await act(async () => button.dispatchEvent(new MouseEvent("click", { bubbles: true })));

    expect(performToggle).toHaveBeenCalledWith("asset-1", false);
    expect(button.disabled).toBe(true);
    expect(button.getAttribute("aria-busy")).toBe("true");
    expect(button.getAttribute("aria-pressed")).toBe("true");

    await act(async () => rejectToggle(new Error("backend rejected")));

    expect(button.disabled).toBe(false);
    expect(button.getAttribute("aria-busy")).toBe("false");
    expect(button.getAttribute("aria-pressed")).toBe("true");
    expect(container.querySelector('[role="alert"]')?.textContent).toContain("backend rejected");
    expect(onSuccess).not.toHaveBeenCalled();
    await act(async () => root.unmount());
  });
});
