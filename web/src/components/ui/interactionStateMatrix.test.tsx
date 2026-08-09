// @vitest-environment happy-dom

import { renderToStaticMarkup } from "react-dom/server";
import { expect, it, vi } from "vitest";
import { ScrubbableNumberField } from "../inspector/ScrubbableNumberField";
import { SplitPane } from "../shell/SplitPane";
import { HoverButton } from "./HoverButton";

vi.mock("../../i18n", () => ({ useT: () => (key: string) => key }));

it("all_enabled_disabled_hover_focus_and_cursor_modes", () => {
  const enabledButton = renderToStaticMarkup(<HoverButton title="Play">P</HoverButton>);
  const disabledButton = renderToStaticMarkup(<HoverButton title="Play" disabled>P</HoverButton>);
  expect(enabledButton).toContain("cursor:pointer");
  expect(enabledButton).toContain('data-interaction-state="enabled"');
  expect(disabledButton).toContain("cursor:not-allowed");
  expect(disabledButton).toContain('data-interaction-state="disabled"');

  const field = renderToStaticMarkup(
    <ScrubbableNumberField
      ariaLabel="Opacity"
      value={50}
      min={0}
      max={100}
      sensitivity={1}
      format={String}
      onCommit={() => undefined}
    />,
  );
  expect(field).toContain('role="spinbutton"');
  expect(field).toContain('aria-label="Opacity"');
  expect(field).toContain('tabindex="0"');
  expect(field).toContain("cursor:ew-resize");

  const disabledField = renderToStaticMarkup(
    <ScrubbableNumberField
      ariaLabel="Opacity"
      disabled
      value={50}
      min={0}
      max={100}
      sensitivity={1}
      format={String}
      onCommit={() => undefined}
    />,
  );
  expect(disabledField).toContain('aria-disabled="true"');
  expect(disabledField).toContain('tabindex="-1"');
  expect(disabledField).toContain("cursor:not-allowed");

  const horizontal = renderToStaticMarkup(
    <SplitPane mode="horizontal" initial={300} first={<div />} second={<div />} />,
  );
  const vertical = renderToStaticMarkup(
    <SplitPane mode="vertical" initial={300} first={<div />} second={<div />} />,
  );
  expect(horizontal).toContain('role="separator"');
  expect(horizontal).toContain("cursor:col-resize");
  expect(vertical).toContain("cursor:row-resize");
});
