import { describe, expect, it } from "vitest";
import { shouldCommitTimelineFrameResult } from "./useTimelineFrame";

describe("timeline composite frame request freshness", () => {
  it("rejects an older composite result after a newer pause frame has been requested", () => {
    expect(
      shouldCommitTimelineFrameResult({
        enabled: true,
        resultFrame: 12,
        requestId: 1,
        latestRequestId: 2,
        latestRequestedFrame: 42,
      }),
    ).toBe(false);
  });

  it("accepts the latest composite result for the currently requested pause frame", () => {
    expect(
      shouldCommitTimelineFrameResult({
        enabled: true,
        resultFrame: 42,
        requestId: 2,
        latestRequestId: 2,
        latestRequestedFrame: 42,
      }),
    ).toBe(true);
  });
});
