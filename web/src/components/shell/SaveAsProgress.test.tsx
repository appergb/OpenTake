import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ cancelSaveAsMedia: vi.fn() }));
vi.mock("../../store/editActions", () => ({ cancelSaveAsMedia: mocks.cancelSaveAsMedia }));

import { SaveAsProgressView } from "./SaveAsProgress";

describe("SaveAsProgress", () => {
  it("renders visible progress and an enabled cancel button", () => {
    const progress = {
      operationId: 1,
      label: "Saving clip",
      done: 25,
      total: 100,
      cancellable: true,
      cancelling: false,
    };
    const html = renderToStaticMarkup(<SaveAsProgressView progress={progress} />);

    expect(html).toContain("Saving clip");
    expect(html).toContain("25%");
    expect(html).toContain("Cancel");
    expect(html).not.toContain("disabled");

    const view = SaveAsProgressView({ progress });
    const button = React.Children.toArray(view.props.children)[2] as React.ReactElement<{
      onClick: () => void;
    }>;
    button.props.onClick();
    expect(mocks.cancelSaveAsMedia).toHaveBeenCalledTimes(1);
  });
});
