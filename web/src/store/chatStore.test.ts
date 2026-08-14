import { beforeEach, describe, expect, it } from "vitest";
import {
  decodeChatHistorySnapshot,
  decodeChatSessionsSnapshot,
  decodeChatStreamEvent,
} from "../lib/api";
import {
  MAX_CHAT_IMAGE_BASE64_CHARS,
  type AgentContentBlock,
  type ChatMessage,
} from "../lib/types";
import {
  MAX_CHAT_BLOCK_INDEX,
  MAX_CHAT_DELETED_SESSIONS,
  MAX_CHAT_DRAFTS,
  MAX_CHAT_RETAINED_SESSIONS,
  useChatStore,
} from "./chatStore";

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
  const toolCalls = blocks.flatMap((block) =>
    block.type === "toolUse"
      ? [{
          id: block.id,
          name: block.name,
          args: block.input,
          result: block.result,
          isError: block.isError,
        }]
      : [],
  );
  return {
    id,
    role: "assistant",
    content: blocks
      .filter((block): block is Extract<AgentContentBlock, { type: "text" }> => block.type === "text")
      .map((block) => block.text)
      .join(""),
    toolCalls,
    blocks,
    createdAt: 1,
    ...overrides,
  };
}

function toolResultMessage(
  id: string,
  toolUseId = "tool-1",
  overrides: Partial<ChatMessage> = {},
): ChatMessage {
  return {
    id,
    role: "tool",
    content: "{\"ok\":true}",
    toolCalls: [],
    blocks: [{
      type: "toolResult",
      toolUseId,
      content: [{ kind: "text", text: "ok" }],
    }],
    createdAt: 1,
    toolCallId: toolUseId,
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
    sessionOrder: [],
    drafts: {},
    draftOrder: [],
    blockedMessageKeys: {},
    blockedMessageOrder: [],
    historyResyncRequests: {},
    resyncingSessionIds: {},
    resyncSessionOrder: [],
    deletedSessionIds: {},
    deletedSessionOrder: [],
    projectEpoch: null,
    projectPath: null,
    projectGeneration: 0,
    sessionVersions: {},
    composerDraft: null,
  });
}

function deltaPayload(overrides: Record<string, unknown> = {}) {
  return {
    projectEpoch: 7,
    projectPath: "/tmp/project.opentake",
    sessionId: sessionA,
    messageId: "message-a",
    sequence: 0,
    blockIndex: 0,
    delta: "hello",
    ...overrides,
  };
}

describe("chatStore ordered session streams", () => {
  beforeEach(resetStore);

  it("clears every project-scoped namespace when the same session id moves to a new project", () => {
    const store = useChatStore.getState();
    store.resetProject(sessionA, 7, "/tmp/A.opentake");
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", 1, 0, "gap");
    store.deleteSession(sessionB);
    store.setComposerDraft("project A draft");

    expect(store.resetProject(sessionA, 8, "/tmp/B.opentake")).toBe(true);
    const moved = useChatStore.getState();
    expect(moved.sessionId).toBe(sessionA);
    expect(moved.projectEpoch).toBe(8);
    expect(moved.projectPath).toBe("/tmp/B.opentake");
    expect(moved.sessionMessages).toEqual({});
    expect(moved.drafts).toEqual({});
    expect(moved.blockedMessageKeys).toEqual({});
    expect(moved.historyResyncRequests).toEqual({});
    expect(moved.resyncingSessionIds).toEqual({});
    expect(moved.deletedSessionIds).toEqual({});
    expect(moved.sessionVersions).toEqual({});
    expect(moved.composerDraft).toBeNull();
  });

  it("preserves selected session and composer when resetProject sees an ordinary same-project remount", () => {
    const store = useChatStore.getState();
    store.resetProject(sessionA, 7, "/tmp/A.opentake");
    store.setMessagesForSession(sessionB, [assistantMessage("saved", [{ type: "text", text: "keep" }])]);
    store.reset(sessionB);
    store.setComposerDraft("keep this draft");

    expect(store.resetProject("replacement-id", 7, "/tmp/A.opentake")).toBe(false);
    const remounted = useChatStore.getState();
    expect(remounted.sessionId).toBe(sessionB);
    expect(remounted.messages[0].content).toBe("keep");
    expect(remounted.composerDraft).toBe("keep this draft");
  });

  it("installs a startup snapshot only when that session was untouched in the same load generation", () => {
    const store = useChatStore.getState();
    store.resetProject(sessionA, 7, "/tmp/A.opentake");
    const generation = useChatStore.getState().projectGeneration;
    const versions = { ...useChatStore.getState().sessionVersions };
    store.beginMessage(sessionB, "live-inactive");
    store.appendBlockDelta(sessionB, "live-inactive", 0, 0, "new live reply");

    expect(store.installSessionSnapshot(
      sessionB,
      [assistantMessage("stale", [{ type: "text", text: "old persisted reply" }])],
      generation,
      versions[sessionB] ?? 0,
    )).toBe(false);
    expect(useChatStore.getState().sessionMessages[sessionB][0].content).toBe("new live reply");
    expect(store.installSessionSnapshot(
      sessionA,
      [assistantMessage("saved", [{ type: "text", text: "safe snapshot" }])],
      generation,
      versions[sessionA] ?? 0,
    )).toBe(true);
    expect(useChatStore.getState().sessionMessages[sessionA][0].content).toBe("safe snapshot");
  });

  it("validates bounded history and session snapshots before they can reach the store", () => {
    const valid = assistantMessage("saved", [{ type: "text", text: "safe" }]);
    const oversized = toolResultMessage("tool-image", "tool-1", {
      content: "image",
      blocks: [{
        type: "toolResult",
        toolUseId: "tool-1",
        content: [{
          kind: "image",
          mediaType: "image/png",
          base64: "A".repeat(MAX_CHAT_IMAGE_BASE64_CHARS + 4),
        }],
      }],
    });

    expect(decodeChatHistorySnapshot([valid])).toEqual([valid]);
    expect(decodeChatHistorySnapshot([oversized])).toBeNull();
    expect(decodeChatSessionsSnapshot([{
      id: sessionA,
      messages: [oversized],
      createdAt: 1,
      isOpen: true,
    }])).toBeNull();
    expect(decodeChatSessionsSnapshot([{
      id: sessionA,
      messages: [valid],
      createdAt: 1,
      isOpen: true,
    }])?.[0].messages).toEqual([valid]);
  });

  it("keeps text, tool, and following text in authoritative sequence order", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", 0, 0, "I will inspect ");
    store.upsertBlock(sessionA, "message-a", 1, 1, tool("tool-1"));
    store.appendBlockDelta(sessionA, "message-a", 2, 2, "and then edit.");

    expect(useChatStore.getState().messages[0].blocks).toEqual([
      { type: "text", text: "I will inspect " },
      tool("tool-1"),
      { type: "text", text: "and then edit." },
    ]);
    expect(useChatStore.getState().messages[0].content).toBe("I will inspect and then edit.");
  });

  it("ignores an exact delayed retry while preserving identical later deltas", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", 0, 0, "same");
    store.appendBlockDelta(sessionA, "message-a", 1, 0, "same");
    store.appendBlockDelta(sessionA, "message-a", 0, 0, "same");
    store.appendBlockDelta(sessionA, "message-a", 2, 0, "later");

    expect(useChatStore.getState().messages[0].blocks).toEqual([
      { type: "text", text: "samesamelater" },
    ]);
    expect(useChatStore.getState().takeHistoryResyncRequest()).toBeNull();
  });

  it("canonicalizes distinct Unicode keys by code unit instead of locale collation", () => {
    const firstInput = { "é": 1, "e\u0301": 2 };
    const reorderedInput = { "e\u0301": 2, "é": 1 };
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.upsertBlock(sessionA, "message-a", 0, 0, {
      type: "toolUse",
      id: "tool-a",
      name: "inspect_timeline",
      input: firstInput,
    });
    store.appendBlockDelta(sessionA, "message-a", 1, 1, "later");
    store.upsertBlock(sessionA, "message-a", 0, 0, {
      type: "toolUse",
      id: "tool-a",
      name: "inspect_timeline",
      input: reorderedInput,
    });

    expect(useChatStore.getState().takeHistoryResyncRequest()).toBeNull();
    expect(useChatStore.getState().messages[0].blocks?.at(-1)).toEqual({
      type: "text",
      text: "later",
    });
  });

  it("rejects a sparse retry that collided in the old canonical hash", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.upsertBlock(sessionA, "message-a", 0, 0, {
      type: "toolResult",
      toolUseId: "tool-a",
      content: [],
    });
    store.upsertBlock(sessionA, "message-a", 0, 0, {
      type: "toolResult",
      toolUseId: "tool-a",
      content: new Array(1),
    });

    expect(useChatStore.getState().takeHistoryResyncRequest()).toMatchObject({
      sessionId: sessionA,
      messageId: "message-a",
      reason: "invalid_block",
    });
  });

  it("rejects a sequence gap before reading the terminal payload", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    let payloadReads = 0;
    const terminal = {
      id: "message-a",
      get role() {
        payloadReads += 1;
        throw new Error("terminal payload should not be read for a sequence gap");
      },
    } as unknown as ChatMessage;

    expect(() => store.finalize(sessionA, "message-a", 2, terminal)).not.toThrow();
    expect(payloadReads).toBe(0);
    expect(useChatStore.getState().takeHistoryResyncRequest()).toMatchObject({
      reason: "sequence_gap",
    });
  });

  it("poisons a reused sequence when its event kind, block index, or payload conflicts", () => {
    const conflicts: Array<(store: ReturnType<typeof useChatStore.getState>) => void> = [
      (store) => store.appendBlockDelta(sessionA, "message-a", 0, 0, "changed"),
      (store) => store.appendBlockDelta(sessionA, "message-a", 0, 1, "original"),
      (store) => store.upsertBlock(sessionA, "message-a", 0, 0, tool("tool-a")),
    ];

    conflicts.forEach((conflict) => {
      resetStore();
      const store = useChatStore.getState();
      store.beginMessage(sessionA, "message-a");
      store.appendBlockDelta(sessionA, "message-a", 0, 0, "original");
      conflict(useChatStore.getState());

      expect(useChatStore.getState().messages[0].content).toBe("original");
      expect(useChatStore.getState().takeHistoryResyncRequest()).toMatchObject({
        sessionId: sessionA,
        messageId: "message-a",
        reason: "sequence_conflict",
      });
    });
  });

  it("poisons an exact retry after it falls outside the bounded replay window", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", 0, 0, "first");
    for (let sequence = 1; sequence <= 64; sequence += 1) {
      store.appendBlockDelta(sessionA, "message-a", sequence, 0, "x");
    }
    store.appendBlockDelta(sessionA, "message-a", 0, 0, "first");

    expect(useChatStore.getState().messages[0].content).toBe(`first${"x".repeat(64)}`);
    expect(useChatStore.getState().takeHistoryResyncRequest()).toMatchObject({
      sessionId: sessionA,
      messageId: "message-a",
      reason: "sequence_out_of_order",
    });
  });

  it("keeps multiple tool rounds addressable without duplicate upserts", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", 0, 0, "first ");
    store.upsertBlock(sessionA, "message-a", 1, 1, tool("tool-1"));
    store.upsertBlock(sessionA, "message-a", 1, 1, tool("tool-1"));
    store.appendBlockDelta(sessionA, "message-a", 2, 2, "second ");
    store.upsertBlock(sessionA, "message-a", 3, 3, tool("tool-2", "split_clip"));
    store.appendBlockDelta(sessionA, "message-a", 4, 4, "complete");

    expect(useChatStore.getState().messages[0].blocks).toEqual([
      { type: "text", text: "first " },
      tool("tool-1"),
      { type: "text", text: "second " },
      tool("tool-2", "split_clip"),
      { type: "text", text: "complete" },
    ]);
  });

  it("poisons only the gapped message while another message in the session proceeds", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.upsertBlock(sessionA, "message-a", 2, 0, tool("tool-a"));
    store.beginMessage(sessionA, "message-b");
    store.appendBlockDelta(sessionA, "message-b", 0, 0, "healthy");

    const state = useChatStore.getState();
    expect(state.messages.find((message) => message.id === "message-a")?.blocks).toEqual([]);
    expect(state.messages.find((message) => message.id === "message-b")?.content).toBe("healthy");
    expect(state.streamingId).toBe("message-b");
    expect(state.takeHistoryResyncRequest()).toEqual({
      sessionId: sessionA,
      messageId: "message-a",
      reason: "sequence_gap",
    });
  });

  it("deduplicates session history re-sync while blocking each malformed message separately", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", 1, 0, "gap");
    store.beginMessage(sessionA, "message-b");
    store.appendBlockDelta(sessionA, "message-b", 3, 0, "gap");

    const state = useChatStore.getState();
    expect(Object.keys(state.blockedMessageKeys)).toHaveLength(2);
    expect(state.takeHistoryResyncRequest()).toMatchObject({ sessionId: sessionA });
    expect(state.takeHistoryResyncRequest()).toBeNull();
  });

  it("poisons stale non-duplicate sequence delivery but leaves sibling messages active", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", 0, 0, "A");
    store.appendBlockDelta(sessionA, "message-a", 1, 0, "B");
    store.appendBlockDelta(sessionA, "message-a", 0, 0, "old");
    store.beginMessage(sessionA, "message-b");
    store.appendBlockDelta(sessionA, "message-b", 0, 0, "safe");

    expect(useChatStore.getState().messages.find((message) => message.id === "message-a")?.content).toBe("AB");
    expect(useChatStore.getState().messages.find((message) => message.id === "message-b")?.content).toBe("safe");
    expect(useChatStore.getState().takeHistoryResyncRequest()).toMatchObject({
      messageId: "message-a",
      reason: "sequence_conflict",
    });
  });

  it("rejects negative and huge indices or sequences without mutating a draft", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", -1, 0, "bad");
    store.upsertBlock(sessionA, "message-a", 0, MAX_CHAT_BLOCK_INDEX + 1, tool("bad"));

    expect(useChatStore.getState().messages[0].blocks).toEqual([]);
    expect(useChatStore.getState().takeHistoryResyncRequest()).toMatchObject({
      sessionId: sessionA,
      messageId: "message-a",
    });
  });

  it("poisons an upsert that changes a block discriminant or tool identity", () => {
    const cases: Array<[AgentContentBlock, AgentContentBlock]> = [
      [tool("tool-1"), { ...tool("tool-1"), type: "toolUse", name: "split_clip" }],
      [tool("tool-1"), tool("tool-2")],
      [tool("tool-1"), { type: "text", text: "replacement" }],
      [
        { type: "toolResult", toolUseId: "tool-1", content: [{ kind: "text", text: "ok" }] },
        { type: "toolResult", toolUseId: "tool-2", content: [{ kind: "text", text: "ok" }] },
      ],
    ];

    cases.forEach(([initial, replacement], index) => {
      resetStore();
      const store = useChatStore.getState();
      store.beginMessage(sessionA, `message-${index}`);
      store.upsertBlock(sessionA, `message-${index}`, 0, 0, initial);
      store.upsertBlock(sessionA, `message-${index}`, 1, 0, replacement);
      expect(useChatStore.getState().messages[0].blocks).toEqual([initial]);
      expect(useChatStore.getState().takeHistoryResyncRequest()).toMatchObject({
        messageId: `message-${index}`,
        reason: "block_identity_mismatch",
      });
    });
  });

  it("replaces a streaming draft with the exact sequenced final message", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", 0, 0, "stale draft");
    const final = assistantMessage("message-a", [
      { type: "text", text: "final before " },
      tool("tool-final"),
      { type: "text", text: "final after" },
    ]);
    store.finalize(sessionA, "message-a", 1, final);

    const state = useChatStore.getState();
    expect(state.streaming).toBe(false);
    expect(state.streamingId).toBeNull();
    expect(state.messages).toEqual([final]);
  });

  it("replaces a tool-result draft with its exact terminal tool message", () => {
    const store = useChatStore.getState();
    const final = toolResultMessage("tool-message", "tool-a");
    store.beginMessage(sessionA, "tool-message");
    store.upsertBlock(sessionA, "tool-message", 0, 0, final.blocks![0]);
    store.finalize(sessionA, "tool-message", 1, final);

    expect(useChatStore.getState().messages).toEqual([final]);
    expect(useChatStore.getState().streaming).toBe(false);
  });

  it("fails the legacy finalize overload closed when the message id is not exact", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.finalize(assistantMessage("message-b", [{ type: "text", text: "wrong" }]));

    expect(useChatStore.getState().messages).toEqual([
      expect.objectContaining({ id: "message-a", content: "" }),
    ]);
    expect(useChatStore.getState().takeHistoryResyncRequest()).toMatchObject({
      messageId: "message-a",
      reason: "message_identity_mismatch",
    });
  });

  it("keeps an inactive session draft isolated until that session is selected", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", 0, 0, "draft A");
    store.reset(sessionB);
    store.setMessages([
      assistantMessage("persisted-b", [{ type: "text", text: "persisted B" }]),
    ]);
    store.appendBlockDelta(sessionA, "message-a", 1, 0, " after switch");

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
    store.upsertBlock(sessionA, "message-a", 0, 0, tool("tool-a"));
    store.finalize(
      sessionA,
      "message-a",
      1,
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
    store.upsertBlock(sessionA, "missing-message", 0, 0, tool("tool-missing"));

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

  it("clears all message poison after authoritative history is installed", () => {
    const store = useChatStore.getState();
    store.beginMessage(sessionA, "message-a");
    store.appendBlockDelta(sessionA, "message-a", 2, 0, "gap");
    store.setMessagesForSession(sessionA, [
      assistantMessage("message-a", [{ type: "text", text: "authoritative" }]),
    ]);

    expect(Object.keys(useChatStore.getState().blockedMessageKeys)).toEqual([]);
    expect(useChatStore.getState().takeHistoryResyncRequest()).toBeNull();
    store.beginMessage(sessionA, "message-b");
    store.appendBlockDelta(sessionA, "message-b", 0, 0, "new stream");
    expect(useChatStore.getState().messages.at(-1)?.content).toBe("new stream");
  });

  it("bounds deleted-session tombstones, retained histories, and in-flight drafts", () => {
    const store = useChatStore.getState();
    for (let index = 0; index < MAX_CHAT_DELETED_SESSIONS + 12; index += 1) {
      store.deleteSession(`deleted-${index}`);
    }
    expect(Object.keys(useChatStore.getState().deletedSessionIds).length).toBeLessThanOrEqual(
      MAX_CHAT_DELETED_SESSIONS,
    );
    expect(useChatStore.getState().deletedSessionIds[`deleted-${MAX_CHAT_DELETED_SESSIONS + 11}`]).toBe(true);

    for (let index = 0; index < MAX_CHAT_RETAINED_SESSIONS + 12; index += 1) {
      const sessionId = `history-${index}`;
      store.reset(sessionId);
      store.setMessages([assistantMessage(`saved-${index}`, [{ type: "text", text: "saved" }])]);
    }
    expect(Object.keys(useChatStore.getState().sessionMessages).length).toBeLessThanOrEqual(
      MAX_CHAT_RETAINED_SESSIONS,
    );
    expect(useChatStore.getState().messages[0].id).toBe(
      `saved-${MAX_CHAT_RETAINED_SESSIONS + 11}`,
    );

    for (let index = 0; index < MAX_CHAT_DRAFTS + 12; index += 1) {
      const sessionId = `draft-session-${index}`;
      store.reset(sessionId);
      store.beginMessage(sessionId, `draft-${index}`);
    }
    expect(Object.keys(useChatStore.getState().drafts).length).toBeLessThanOrEqual(
      MAX_CHAT_DRAFTS,
    );
    expect(useChatStore.getState().streamingId).toBe(`draft-${MAX_CHAT_DRAFTS + 11}`);
  });

  it("strictly decodes a bounded sequenced delta", () => {
    expect(decodeChatStreamEvent("chat_delta", deltaPayload())).toEqual({
      ok: true,
      event: {
        type: "blockDelta",
        projectEpoch: 7,
        projectPath: "/tmp/project.opentake",
        sessionId: sessionA,
        messageId: "message-a",
        sequence: 0,
        blockIndex: 0,
        delta: "hello",
      },
    });
  });

  it("rejects sparse arrays and other non-JSON tool payloads", () => {
    const sparseContent = new Array(1);
    const invalidBlocks: unknown[] = [
      { type: "toolResult", toolUseId: "tool-a", content: sparseContent },
      { type: "toolUse", id: "tool-a", name: "inspect_timeline", input: new Array(1) },
      { type: "toolUse", id: "tool-a", name: "inspect_timeline", input: new Date(0) },
    ];

    invalidBlocks.forEach((block) => {
      expect(decodeChatStreamEvent("chat_tool_call", {
        ...deltaPayload(),
        delta: undefined,
        block,
      })).toEqual({
        ok: false,
        failure: {
          eventName: "chat_tool_call",
          reason: "invalid_block",
          sessionId: sessionA,
          messageId: "message-a",
        },
      });
    });
  });

  it("rejects image blocks whose aggregate event payload exceeds one MiB", () => {
    const image = { kind: "image" as const, base64: "x".repeat(600_000), mediaType: "image/png" };
    expect(decodeChatStreamEvent("chat_tool_call", {
      ...deltaPayload(),
      delta: undefined,
      block: {
        type: "toolResult",
        toolUseId: "tool-a",
        content: [image, image],
      },
    })).toEqual({
      ok: false,
      failure: {
        eventName: "chat_tool_call",
        reason: "invalid_block",
        sessionId: sessionA,
        messageId: "message-a",
      },
    });
  });

  it("strictly decodes a terminal tool-result message", () => {
    const messages = [
      toolResultMessage("message-a", "tool-a"),
      toolResultMessage("message-a", "tool-a", {
        blocks: [{
          type: "toolResult",
          toolUseId: "tool-a",
          content: [{ kind: "text", text: "ok" }],
          isError: false,
        }],
        toolIsError: false,
      }),
    ];

    messages.forEach((message) => {
      expect(decodeChatStreamEvent("chat_done", {
        ...deltaPayload(),
        delta: undefined,
        blockIndex: undefined,
        sequence: 1,
        message,
      })).toEqual({
        ok: true,
        event: {
          type: "done",
          projectEpoch: 7,
          projectPath: "/tmp/project.opentake",
          sessionId: sessionA,
          messageId: "message-a",
          sequence: 1,
          message,
        },
      });
    });
  });

  it("rejects a tool terminal when only the block explicitly marks success", () => {
    const message = toolResultMessage("message-a", "tool-a", {
      blocks: [{
        type: "toolResult",
        toolUseId: "tool-a",
        content: [{ kind: "text", text: "ok" }],
        isError: false,
      }],
    });

    expect(decodeChatStreamEvent("chat_done", {
      ...deltaPayload(),
      delta: undefined,
      blockIndex: undefined,
      sequence: 1,
      message,
    })).toEqual({
      ok: false,
      failure: {
        eventName: "chat_done",
        reason: "invalid_message",
        sessionId: sessionA,
        messageId: "message-a",
      },
    });
  });

  it("rejects a tool terminal when only the message explicitly marks success", () => {
    const message = toolResultMessage("message-a", "tool-a", {
      toolIsError: false,
    });

    expect(decodeChatStreamEvent("chat_done", {
      ...deltaPayload(),
      delta: undefined,
      blockIndex: undefined,
      sequence: 1,
      message,
    })).toEqual({
      ok: false,
      failure: {
        eventName: "chat_done",
        reason: "invalid_message",
        sessionId: sessionA,
        messageId: "message-a",
      },
    });
  });

  it("rejects malformed tool terminals and tool-only fields on assistants", () => {
    const validTool = toolResultMessage("message-a", "tool-a");
    const validAssistant = assistantMessage("message-a", [{ type: "text", text: "done" }]);
    const invalidMessages = [
      { ...validTool, blocks: [] },
      { ...validTool, blocks: [{ type: "text", text: "wrong role" }] },
      { ...validTool, toolCallId: "tool-b" },
      { ...validTool, toolCalls: [{ id: "tool-a", name: "inspect_timeline", args: {} }] },
      { ...validTool, toolIsError: true },
      { ...validAssistant, toolCallId: "tool-a" },
      { ...validAssistant, blocks: [{ type: "toolResult", toolUseId: "tool-a", content: [{ kind: "text", text: "wrong role" }] }] },
    ];

    invalidMessages.forEach((message) => {
      expect(decodeChatStreamEvent("chat_done", {
        ...deltaPayload(),
        delta: undefined,
        blockIndex: undefined,
        sequence: 1,
        message,
      })).toEqual({
        ok: false,
        failure: {
          eventName: "chat_done",
          reason: "invalid_message",
          sessionId: sessionA,
          messageId: "message-a",
        },
      });
    });
  });

  it("rejects missing, negative, fractional, and huge event sequences", () => {
    [undefined, -1, 0.5, Number.MAX_SAFE_INTEGER + 1].forEach((sequence) => {
      const payload = deltaPayload({ sequence });
      expect(decodeChatStreamEvent("chat_delta", payload)).toEqual({
        ok: false,
        failure: {
          eventName: "chat_delta",
          reason: "invalid_sequence",
          sessionId: sessionA,
          messageId: "message-a",
        },
      });
    });
  });

  it("deeply rejects malformed terminal assistant messages", () => {
    const valid = assistantMessage("message-a", [
      { type: "text", text: "done" },
      tool("tool-a"),
    ]);
    const invalidMessages = [
      { ...valid, role: "user" },
      { ...valid, createdAt: Number.POSITIVE_INFINITY },
      { ...valid, toolCalls: [{ id: "", name: "get_timeline", args: {} }] },
      { ...valid, toolCalls: [{ id: "tool-a", name: "", args: {} }] },
      { ...valid, toolCalls: [{ id: "tool-a", name: "get_timeline", args: { n: Infinity } }] },
      { ...valid, blocks: [{ type: "toolUse", id: "", name: "get_timeline", input: {} }] },
      { ...valid, blocks: [{ type: "toolResult", toolUseId: "", content: [] }] },
      { ...valid, blocks: [{ type: "toolResult", toolUseId: "tool-a", content: [{ kind: "image", base64: 7, mediaType: "image/png" }] }] },
      { ...valid, blocks: [{ type: "unknown" }] },
      { ...valid, toolIsError: "yes" },
    ];

    invalidMessages.forEach((message) => {
      expect(decodeChatStreamEvent("chat_done", {
        ...deltaPayload(),
        delta: undefined,
        blockIndex: undefined,
        sequence: 2,
        message,
      })).toEqual({
        ok: false,
        failure: {
          eventName: "chat_done",
          reason: "invalid_message",
          sessionId: sessionA,
          messageId: "message-a",
        },
      });
    });
  });
});
