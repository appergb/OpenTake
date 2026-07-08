import { beforeEach, describe, expect, it } from "vitest";
import type { ChatMessage } from "../lib/types";
import { useChatStore } from "./chatStore";

function resetStore() {
  useChatStore.setState({
    sessionId: "chat-test",
    messages: [],
    streaming: false,
    streamingId: null,
  });
}

function assistantMessage(overrides: Partial<ChatMessage> = {}): ChatMessage {
  return {
    id: "assistant-final",
    role: "assistant",
    content: "done",
    toolCalls: [],
    createdAt: 1,
    ...overrides,
  };
}

describe("chatStore", () => {
  beforeEach(() => {
    resetStore();
  });

  it("merges streaming deltas into the active assistant placeholder", () => {
    const store = useChatStore.getState();
    store.beginStream("assistant-stream");
    store.appendDelta("hello");
    store.appendDelta(" world");

    const state = useChatStore.getState();
    expect(state.streaming).toBe(true);
    expect(state.messages).toHaveLength(1);
    expect(state.messages[0].content).toBe("hello world");
  });

  it("upserts tool calls by id instead of duplicating them", () => {
    const store = useChatStore.getState();
    store.beginStream("assistant-stream");
    store.upsertToolCall({
      id: "tool-1",
      name: "get_timeline",
      args: { startFrame: 0 },
    });
    store.upsertToolCall({
      id: "tool-1",
      name: "get_timeline",
      args: { startFrame: 0 },
      result: { summary: "ok" },
      isError: false,
    });

    const toolCalls = useChatStore.getState().messages[0].toolCalls;
    expect(toolCalls).toHaveLength(1);
    expect(toolCalls[0].result).toEqual({ summary: "ok" });
    expect(toolCalls[0].isError).toBe(false);
  });

  it("finalize replaces the placeholder and clears streaming state", () => {
    const store = useChatStore.getState();
    store.beginStream("assistant-stream");
    store.appendDelta("draft");
    store.finalize(
      assistantMessage({
        toolCalls: [
          {
            id: "tool-1",
            name: "tighten_silences",
            args: {},
            result: { summary: "trimmed" },
          },
        ],
      }),
    );

    const state = useChatStore.getState();
    expect(state.streaming).toBe(false);
    expect(state.streamingId).toBeNull();
    expect(state.messages).toHaveLength(1);
    expect(state.messages[0].id).toBe("assistant-final");
    expect(state.messages[0].content).toBe("done");
    expect(state.messages[0].toolCalls[0].result).toEqual({ summary: "trimmed" });
  });

  it("finalize preserves streamed tool cards when final backend message omits toolCalls", () => {
    const store = useChatStore.getState();
    store.beginStream("assistant-stream");
    store.upsertToolCall({
      id: "tool-1",
      name: "get_timeline",
      args: {},
    });
    store.upsertToolCall({
      id: "tool-1",
      name: "get_timeline",
      args: {},
      result: { summary: "ok" },
      isError: false,
    });
    store.finalize(
      assistantMessage({
        id: "assistant-final",
        content: "done without cards",
        toolCalls: [],
      }),
    );

    const state = useChatStore.getState();
    expect(state.streaming).toBe(false);
    expect(state.messages).toHaveLength(1);
    expect(state.messages[0].content).toBe("done without cards");
    expect(state.messages[0].toolCalls).toHaveLength(1);
    expect(state.messages[0].toolCalls[0].result).toEqual({ summary: "ok" });
  });
});
