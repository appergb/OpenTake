import { beforeEach, describe, expect, it } from "vitest";
import { decodeChatStreamEvent } from "../lib/api";
import type { AgentContentBlock, ChatMessage } from "../lib/types";
import { MAX_CHAT_BLOCK_INDEX, useChatStore } from "./chatStore";

const sessionA = "session-a";
const sessionB = "session-b";

function tool(id: string, name = "get_timeline"): AgentContentBlock {
  return { type: "toolUse", id, name, input: { frame: 0 } };
}

function assistantMessage(
  id: string,
  blocks: AgentContentBlock[],
  overrides: Partial<ChatMessage> = {},
): ChatMessage {
  return {
    id,
    role: "assistant",
    content: blocks
      .filter((block): block is Extract<AgentContentBlock, { type: "text" }> => block.type === "text")
      .map((block) => block.text)
      .join(""),
    toolCalls: [],
    blocks,
    createdAt: 1,
    ...overrides,
  };
}

function resetStore() {
  useChatStore.setState({
    sessionId: sessionA,
    messages: [],
    streaming: false,
    streamingId: null,
    sessionMessages: {},
    drafts: {},
    blockedMessageKeys: {},
    historyResyncRequests: {},
    resyncingSessionIds: {},
    deletedSessionIds: {},
    composerDraft: null,
  });
}

describe("chatStore ordered session streams", () => {
  beforeEach(resetStore);

  it("keeps text, tool, and following text in authoritative block order", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", 0, "I will inspect ");
    store.upsertBlock(sessionA, "message-a", 1, tool("tool-1"));
    store.appendBlockDelta(sessionA, "message-a", 2, "and then edit.");

    expect(useChatStore.getState().messages[0].blocks).toEqual([
      { type: "text", text: "I will inspect " },
      tool("tool-1"),
      { type: "text", text: "and then edit." },
    ]);
    expect(useChatStore.getState().messages[0].content).toBe("I will inspect and then edit.");
  });

  it("keeps multiple tool rounds and duplicate retry events idempotent", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", 0, "first ");
    store.appendBlockDelta(sessionA, "message-a", 0, "first ");
    store.upsertBlock(sessionA, "message-a", 1, tool("tool-1"));
    store.upsertBlock(sessionA, "message-a", 1, tool("tool-1"));
    store.appendBlockDelta(sessionA, "message-a", 2, "second ");
    store.upsertBlock(sessionA, "message-a", 3, tool("tool-2", "split_clip"));
    store.appendBlockDelta(sessionA, "message-a", 4, "complete");

    expect(useChatStore.getState().messages[0].blocks).toEqual([
      { type: "text", text: "first " },
      tool("tool-1"),
      { type: "text", text: "second " },
      tool("tool-2", "split_clip"),
      { type: "text", text: "complete" },
    ]);
  });

  it("stops a message and requests one authoritative history reload after a block gap", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.upsertBlock(sessionA, "message-a", 2, tool("tool-2"));
    store.appendBlockDelta(sessionA, "message-a", 0, "must not apply");
    store.upsertBlock(sessionA, "message-a", 2, tool("tool-2"));

    const state = useChatStore.getState();
    expect(state.messages[0].blocks).toEqual([]);
    expect(state.takeHistoryResyncRequest()).toEqual({
      sessionId: sessionA,
      messageId: "message-a",
      reason: "block_gap",
    });
    expect(state.takeHistoryResyncRequest()).toBeNull();
  });

  it("rejects negative and huge block indices without mutating a draft", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", -1, "bad");
    store.upsertBlock(sessionA, "message-a", MAX_CHAT_BLOCK_INDEX + 1, tool("bad"));

    expect(useChatStore.getState().messages[0].blocks).toEqual([]);
    expect(useChatStore.getState().takeHistoryResyncRequest()).toMatchObject({
      sessionId: sessionA,
      messageId: "message-a",
      reason: "invalid_block_index",
    });
  });

  it("replaces a streaming draft with the final authoritative message without merging fields", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", 0, "stale draft");
    const final = assistantMessage("message-a", [
      { type: "text", text: "final before " },
      tool("tool-final"),
      { type: "text", text: "final after" },
    ]);
    store.finalize(sessionA, "message-a", final);

    const state = useChatStore.getState();
    expect(state.streaming).toBe(false);
    expect(state.streamingId).toBeNull();
    expect(state.messages).toEqual([final]);
  });

  it("keeps an inactive session draft isolated until that session is selected", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", 0, "draft A");
    store.reset(sessionB);
    store.setMessages([
      assistantMessage("persisted-b", [{ type: "text", text: "persisted B" }]),
    ]);
    store.appendBlockDelta(sessionA, "message-a", 0, " after switch");

    expect(useChatStore.getState().sessionId).toBe(sessionB);
    expect(useChatStore.getState().messages.map((message) => message.content)).toEqual([
      "persisted B",
    ]);
    expect(useChatStore.getState().streaming).toBe(false);

    store.reset(sessionA);
    expect(useChatStore.getState().messages.map((message) => message.content)).toEqual([
      "draft A after switch",
    ]);
    expect(useChatStore.getState().streamingId).toBe("message-a");
  });

  it("ignores late streams for a deleted session instead of merging into a nearby assistant", () => {
    const store = useChatStore.getState();
    store.reset(sessionB);
    store.setMessages([
      assistantMessage("unrelated-b", [{ type: "text", text: "unrelated B" }]),
    ]);
    store.deleteSession(sessionA);
    store.beginMessage(sessionA, "message-a");
    store.upsertBlock(sessionA, "message-a", 0, tool("tool-a"));
    store.finalize(
      sessionA,
      "message-a",
      assistantMessage("message-a", [tool("tool-a")]),
    );

    const state = useChatStore.getState();
    expect(state.messages).toHaveLength(1);
    expect(state.messages[0].id).toBe("unrelated-b");
    expect(state.messages[0].blocks).toEqual([{ type: "text", text: "unrelated B" }]);
  });

  it("never merges an unaddressed tool call into the nearest assistant message", () => {
    const store = useChatStore.getState();
    store.setMessages([
      assistantMessage("unrelated-a", [{ type: "text", text: "existing reply" }]),
    ]);
    store.upsertBlock(sessionA, "missing-message", 0, tool("tool-missing"));

    const state = useChatStore.getState();
    expect(state.messages).toEqual([
      assistantMessage("unrelated-a", [{ type: "text", text: "existing reply" }]),
    ]);
    expect(state.takeHistoryResyncRequest()).toEqual({
      sessionId: sessionA,
      messageId: "missing-message",
      reason: "missing_draft",
    });
  });

  it("strictly decodes typed stream payloads and reports malformed identities for re-sync", () => {
    expect(
      decodeChatStreamEvent("chat_delta", {
        projectEpoch: 7,
        projectPath: "/tmp/project.opentake",
        sessionId: sessionA,
        messageId: "message-a",
        blockIndex: 0,
        delta: "hello",
      }),
    ).toEqual({
      ok: true,
      event: {
        type: "blockDelta",
        projectEpoch: 7,
        projectPath: "/tmp/project.opentake",
        sessionId: sessionA,
        messageId: "message-a",
        blockIndex: 0,
        delta: "hello",
      },
    });
    expect(
      decodeChatStreamEvent("chat_tool_call", {
        projectEpoch: 7,
        projectPath: "/tmp/project.opentake",
        sessionId: sessionA,
        messageId: "message-a",
        blockIndex: -1,
        block: tool("tool-a"),
      }),
    ).toEqual({
      ok: false,
      failure: {
        eventName: "chat_tool_call",
        reason: "invalid_block_index",
        sessionId: sessionA,
        messageId: "message-a",
      },
    });
  });

});
