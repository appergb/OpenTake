import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { HoverButton } from "./HoverButton";

describe("HoverButton", () => {
  it("does not opt icon buttons out of normal keyboard focus", () => {
    const html = renderToStaticMarkup(<HoverButton title="Play">P</HoverButton>);

    expect(html).not.toContain('tabindex="-1"');
  });
});
