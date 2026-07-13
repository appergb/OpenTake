import { describe, expect, it, vi } from "vitest";
import { rangeContextMenuItems } from "./TimelineRangeContextMenu";

describe("rangeContextMenuItems", () => {
  it("offers only save and clear for the captured range", () => {
    const range = { startFrame: 10, endFrame: 20 };
    const onSave = vi.fn();
    const onClear = vi.fn();
    const items = rangeContextMenuItems({
      range,
      labels: { save: "Save Range as Media", clear: "Clear Range" },
      onSave,
      onClear,
    });

    expect(items.map((item) => item.label)).toEqual([
      "Save Range as Media",
      "Clear Range",
    ]);
    items[0].action();
    items[1].action();
    expect(onSave).toHaveBeenCalledWith(range);
    expect(onClear).toHaveBeenCalledTimes(1);
  });
});
