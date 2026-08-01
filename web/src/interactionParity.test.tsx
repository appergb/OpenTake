import { expect, it } from "vitest";
import { timelineInteractionCursor } from "./components/timeline/TimelineContainer";
import { CORNER_CURSOR } from "./components/preview/TransformOverlay";

it("complete_hover_cursor_matrix", () => {
  expect(CORNER_CURSOR).toEqual({
    topLeft: "nwse-resize",
    topRight: "nesw-resize",
    bottomLeft: "nesw-resize",
    bottomRight: "nwse-resize",
  });

  expect(timelineInteractionCursor({ toolMode: "pointer", inRuler: true })).toBe("pointer");
  expect(
    timelineInteractionCursor({ toolMode: "pointer", inRuler: true, shiftKey: true }),
  ).toBe("crosshair");
  expect(
    timelineInteractionCursor({ toolMode: "pointer", hitRegion: "trimLeft" }),
  ).toBe("ew-resize");
  expect(
    timelineInteractionCursor({ toolMode: "pointer", hitRegion: "trimRight" }),
  ).toBe("ew-resize");
  expect(timelineInteractionCursor({ toolMode: "pointer", hitRegion: "body" })).toBe("grab");
  expect(timelineInteractionCursor({ toolMode: "razor", hitRegion: "body" })).toBe("crosshair");
  expect(timelineInteractionCursor({ toolMode: "pointer", dragKind: "move" })).toBe("grabbing");
  expect(timelineInteractionCursor({ toolMode: "pointer", dragKind: "trimLeft" })).toBe("ew-resize");
  expect(timelineInteractionCursor({ toolMode: "pointer", dragKind: "marquee" })).toBe("crosshair");
  expect(timelineInteractionCursor({ toolMode: "pointer", disabled: true })).toBe("not-allowed");
  expect(timelineInteractionCursor({ toolMode: "pointer" })).toBe("default");
});
