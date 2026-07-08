import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

vi.mock("../../lib/api", () => ({
  getWaveform: vi.fn(),
}));

import { AudioWaveform } from "./MediaPanel";

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
