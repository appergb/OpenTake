/**
 * Chat store. Rust remains the persisted authority. This store only holds
 * addressable in-flight drafts so event delivery cannot merge nearby turns.
 */

import { create } from "zustand";
import {
  MAX_CHAT_BLOCK_INDEX as BLOCK_INDEX_LIMIT,
  MAX_CHAT_DELTA_CHARS,
  MAX_CHAT_STREAM_ID_LENGTH,
  type AgentContentBlock,
  type ChatMessage,
  type ChatToolCall,
} from "../lib/types";

export const MAX_CHAT_BLOCK_INDEX = BLOCK_INDEX_LIMIT;

export type ChatHistoryResyncReason =
  | "block_gap"
  | "invalid_block"
  | "invalid_block_index"
  | "invalid_identity"
  | "missing_draft"
  | "message_identity_mismatch";

export interface ChatHistoryResyncRequest {
  sessionId: string;
  messageId?: string;
  reason: ChatHistoryResyncReason;
}

interface ChatDraft {
  sessionId: string;
  messageId: string;
  /** Fingerprints make transport retries idempotent without changing order. */
  seenDeltas: Record<string, string[]>;
}

interface ChatStoreState {
  sessionId: string;
  /** Derived mirror for the currently selected session only. */
  messages: ChatMessage[];
  streaming: boolean;
  streamingId: string | null;
  sessionMessages: Record<string, ChatMessage[]>;
  drafts: Record<string, ChatDraft>;
  blockedMessageKeys: Record<string, true>;
  historyResyncRequests: Record<string, ChatHistoryResyncRequest>;
  resyncingSessionIds: Record<string, true>;
  deletedSessionIds: Record<string, true>;
  composerDraft: string | null;
  setComposerDraft: (draft: string | null) => void;
  pushUser: (text: string) => void;
  beginMessage: (sessionId: string, messageId: string) => void;
  appendBlockDelta: (sessionId: string, messageId: string, blockIndex: number, delta: string) => void;
  upsertBlock: (
    sessionId: string,
    messageId: string,
    blockIndex: number,
    block: AgentContentBlock,
  ) => void;
  finalize: {
    (sessionId: string, messageId: string, message: ChatMessage): void;
    /** Beta 4 compatibility for an already-open window. New callers use IDs. */
    (message: ChatMessage): void;
  };
  requestHistoryResync: (
    sessionId: string,
    messageId: string | undefined,
    reason: ChatHistoryResyncReason,
  ) => void;
  takeHistoryResyncRequest: () => ChatHistoryResyncRequest | null;
  setMessages: (messages: ChatMessage[]) => void;
  setMessagesForSession: (sessionId: string, messages: ChatMessage[]) => void;
  deleteSession: (sessionId: string) => void;
  reset: (sessionId: string) => void;
  /** Beta 4 compatibility aliases. New event handlers use the methods above. */
  beginStream: (id: string) => void;
  appendDelta: (delta: string) => void;
  upsertToolCall: (toolCall: ChatToolCall) => void;
}

function newSessionId(): string {
  return `chat-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function draftKey(sessionId: string, messageId: string): string {
  return `${sessionId}\u0000${messageId}`;
}

function isId(value: unknown): value is string {
  return typeof value === "string" && value.length > 0 && value.length <= MAX_CHAT_STREAM_ID_LENGTH;
}

function isBlockIndex(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 && value <= MAX_CHAT_BLOCK_INDEX;
}

function hasOwn(value: object, property: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, property);
}

function isBlock(block: unknown): block is AgentContentBlock {
  if (typeof block !== "object" || block === null || !hasOwn(block, "type")) return false;
  const value = block as Record<string, unknown>;
  if (value.type === "text") return typeof value.text === "string";
  if (value.type === "toolUse") {
    return isId(value.id) && isId(value.name) && hasOwn(value, "input") &&
      (value.result === undefined || hasOwn(value, "result")) &&
      (value.isError === undefined || typeof value.isError === "boolean");
  }
  if (value.type !== "toolResult" || !isId(value.toolUseId) || !Array.isArray(value.content)) {
    return false;
  }
  return value.content.every((item) => {
    if (typeof item !== "object" || item === null) return false;
    const content = item as Record<string, unknown>;
    return (content.kind === "text" && typeof content.text === "string") ||
      (content.kind === "image" && typeof content.base64 === "string" && typeof content.mediaType === "string");
  }) && (value.isError === undefined || typeof value.isError === "boolean");
}

function deriveAssistantFields(message: ChatMessage, blocks: AgentContentBlock[]): ChatMessage {
  return {
    ...message,
    blocks,
    content: blocks
      .filter((block): block is Extract<AgentContentBlock, { type: "text" }> => block.type === "text")
      .map((block) => block.text)
      .join(""),
    toolCalls: blocks.flatMap((block) =>
      block.type === "toolUse"
        ? [{ id: block.id, name: block.name, args: block.input, result: block.result, isError: block.isError }]
        : [],
    ),
  };
}

function activeDraft(state: Pick<ChatStoreState, "drafts" | "blockedMessageKeys">, sessionId: string): ChatDraft | undefined {
  return Object.values(state.drafts).find(
    (draft) => draft.sessionId === sessionId && !state.blockedMessageKeys[draftKey(sessionId, draft.messageId)],
  );
}

function selectedView(
  state: Pick<ChatStoreState, "sessionMessages" | "drafts" | "blockedMessageKeys">,
  sessionId: string,
) {
  const draft = activeDraft(state, sessionId);
  return {
    messages: state.sessionMessages[sessionId] ?? [],
    streaming: draft !== undefined,
    streamingId: draft?.messageId ?? null,
  };
}

function withSessionMessages(
  state: ChatStoreState,
  sessionId: string,
  messages: ChatMessage[],
  extras: Partial<ChatStoreState> = {},
): Partial<ChatStoreState> {
  const sessionMessages = { ...state.sessionMessages, [sessionId]: messages };
  const next = { ...state, ...extras, sessionMessages };
  return {
    ...extras,
    sessionMessages,
    ...(state.sessionId === sessionId ? selectedView(next, sessionId) : {}),
  };
}

function stopMessage(
  state: ChatStoreState,
  sessionId: string,
  messageId: string | undefined,
  reason: ChatHistoryResyncReason,
): Partial<ChatStoreState> {
  if (!isId(sessionId) || state.deletedSessionIds[sessionId]) return {};
  const key = messageId && isId(messageId) ? draftKey(sessionId, messageId) : undefined;
  if (key && state.blockedMessageKeys[key]) return {};
  const blockedMessageKeys = key
    ? { ...state.blockedMessageKeys, [key]: true as const }
    : state.blockedMessageKeys;
  const resyncingSessionIds = state.resyncingSessionIds[sessionId]
    ? state.resyncingSessionIds
    : { ...state.resyncingSessionIds, [sessionId]: true as const };
  const historyResyncRequests = state.resyncingSessionIds[sessionId]
    ? state.historyResyncRequests
    : {
        ...state.historyResyncRequests,
        [sessionId]: { sessionId, ...(messageId ? { messageId } : {}), reason },
      };
  const next = { ...state, blockedMessageKeys, resyncingSessionIds };
  return {
    blockedMessageKeys,
    resyncingSessionIds,
    historyResyncRequests,
    ...(state.sessionId === sessionId ? selectedView(next, sessionId) : {}),
  };
}

export const useChatStore = create<ChatStoreState>((set, get) => ({
  sessionId: newSessionId(),
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
  setComposerDraft: (composerDraft) => set({ composerDraft }),

  pushUser: (text) => set((state) => {
    if (state.deletedSessionIds[state.sessionId]) return state;
    const message: ChatMessage = {
      id: `u-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      role: "user",
      content: text,
      toolCalls: [],
      createdAt: Date.now(),
    };
    return withSessionMessages(state, state.sessionId, [...(state.sessionMessages[state.sessionId] ?? []), message]);
  }),

  beginMessage: (sessionId, messageId) => set((state) => {
    if (!isId(sessionId) || !isId(messageId)) return stopMessage(state, sessionId, messageId, "invalid_identity");
    if (state.deletedSessionIds[sessionId] || state.resyncingSessionIds[sessionId]) return state;
    const key = draftKey(sessionId, messageId);
    if (state.drafts[key] || state.blockedMessageKeys[key]) return state;
    const messages = state.sessionMessages[sessionId] ?? [];
    if (messages.some((message) => message.id === messageId)) return state;
    const message: ChatMessage = {
      id: messageId,
      role: "assistant",
      content: "",
      toolCalls: [],
      blocks: [],
      createdAt: Date.now(),
    };
    return withSessionMessages(state, sessionId, [...messages, message], {
      drafts: { ...state.drafts, [key]: { sessionId, messageId, seenDeltas: {} } },
    });
  }),

  appendBlockDelta: (sessionId, messageId, blockIndex, delta) => set((state) => {
    if (!isId(sessionId) || !isId(messageId)) return stopMessage(state, sessionId, messageId, "invalid_identity");
    if (!isBlockIndex(blockIndex)) return stopMessage(state, sessionId, messageId, "invalid_block_index");
    if (typeof delta !== "string" || delta.length > MAX_CHAT_DELTA_CHARS) {
      return stopMessage(state, sessionId, messageId, "invalid_block");
    }
    const key = draftKey(sessionId, messageId);
    if (state.deletedSessionIds[sessionId] || state.blockedMessageKeys[key] || state.resyncingSessionIds[sessionId]) return state;
    const draft = state.drafts[key];
    const messages = state.sessionMessages[sessionId];
    const index = messages?.findIndex((message) => message.id === messageId) ?? -1;
    if (!draft || !messages || index < 0) return stopMessage(state, sessionId, messageId, "missing_draft");
    const message = messages[index];
    const blocks = [...(message.blocks ?? [])];
    if (blockIndex > blocks.length) return stopMessage(state, sessionId, messageId, "block_gap");
    const seen = draft.seenDeltas[String(blockIndex)] ?? [];
    if (seen.includes(delta)) return state;
    if (blockIndex === blocks.length) {
      blocks.push({ type: "text", text: delta });
    } else if (blocks[blockIndex].type === "text") {
      blocks[blockIndex] = { type: "text", text: blocks[blockIndex].text + delta };
    } else {
      return stopMessage(state, sessionId, messageId, "invalid_block");
    }
    const nextMessages = messages.slice();
    nextMessages[index] = deriveAssistantFields(message, blocks);
    return withSessionMessages(state, sessionId, nextMessages, {
      drafts: {
        ...state.drafts,
        [key]: {
          ...draft,
          seenDeltas: { ...draft.seenDeltas, [blockIndex]: [...seen, delta].slice(-128) },
        },
      },
    });
  }),

  upsertBlock: (sessionId, messageId, blockIndex, block) => set((state) => {
    if (!isId(sessionId) || !isId(messageId)) return stopMessage(state, sessionId, messageId, "invalid_identity");
    if (!isBlockIndex(blockIndex)) return stopMessage(state, sessionId, messageId, "invalid_block_index");
    if (!isBlock(block)) return stopMessage(state, sessionId, messageId, "invalid_block");
    const key = draftKey(sessionId, messageId);
    if (state.deletedSessionIds[sessionId] || state.blockedMessageKeys[key] || state.resyncingSessionIds[sessionId]) return state;
    const draft = state.drafts[key];
    const messages = state.sessionMessages[sessionId];
    const index = messages?.findIndex((message) => message.id === messageId) ?? -1;
    if (!draft || !messages || index < 0) return stopMessage(state, sessionId, messageId, "missing_draft");
    const message = messages[index];
    const blocks = [...(message.blocks ?? [])];
    if (blockIndex > blocks.length) return stopMessage(state, sessionId, messageId, "block_gap");
    if (blockIndex === blocks.length) blocks.push(block);
    else blocks[blockIndex] = block;
    const nextMessages = messages.slice();
    nextMessages[index] = deriveAssistantFields(message, blocks);
    return withSessionMessages(state, sessionId, nextMessages);
  }),

  finalize: ((first: string | ChatMessage, second?: string, third?: ChatMessage) => {
    if (typeof first !== "string") {
      const state = get();
      const messageId = state.streamingId;
      if (!messageId) {
        set((current) => withSessionMessages(
          current,
          current.sessionId,
          [...(current.sessionMessages[current.sessionId] ?? []), first],
        ));
        return;
      }
      set((current) => {
        const messages = current.sessionMessages[current.sessionId] ?? [];
        const index = messages.findIndex((message) => message.id === messageId);
        if (index < 0) return current;
        const key = draftKey(current.sessionId, messageId);
        const nextMessages = messages.slice();
        nextMessages[index] = first;
        const drafts = { ...current.drafts };
        delete drafts[key];
        return withSessionMessages(current, current.sessionId, nextMessages, { drafts });
      });
      return;
    }
    const sessionId = first;
    const messageId = second;
    const message = third;
    set((state) => {
      if (!isId(sessionId) || !isId(messageId) || !message || message.id !== messageId) {
        return stopMessage(state, sessionId, messageId, "message_identity_mismatch");
      }
      const key = draftKey(sessionId, messageId);
      if (state.deletedSessionIds[sessionId] || state.blockedMessageKeys[key] || state.resyncingSessionIds[sessionId]) return state;
      const messages = state.sessionMessages[sessionId];
      const index = messages?.findIndex((candidate) => candidate.id === messageId) ?? -1;
      if (!state.drafts[key] || !messages || index < 0) return stopMessage(state, sessionId, messageId, "missing_draft");
      const nextMessages = messages.slice();
      nextMessages[index] = message;
      const drafts = { ...state.drafts };
      delete drafts[key];
      return withSessionMessages(state, sessionId, nextMessages, { drafts });
    });
  }) as ChatStoreState["finalize"],

  requestHistoryResync: (sessionId, messageId, reason) => set((state) =>
    stopMessage(state, sessionId, messageId, reason),
  ),

  takeHistoryResyncRequest: () => {
    const state = get();
    const [sessionId, request] = Object.entries(state.historyResyncRequests)[0] ?? [];
    if (!sessionId || !request) return null;
    set(({ historyResyncRequests }) => {
      const next = { ...historyResyncRequests };
      delete next[sessionId];
      return { historyResyncRequests: next };
    });
    return request;
  },

  setMessages: (messages) => get().setMessagesForSession(get().sessionId, messages),

  setMessagesForSession: (sessionId, messages) => set((state) => {
    if (!isId(sessionId) || state.deletedSessionIds[sessionId]) return state;
    const drafts = Object.fromEntries(
      Object.entries(state.drafts).filter(([, draft]) => draft.sessionId !== sessionId),
    );
    const resyncingSessionIds = { ...state.resyncingSessionIds };
    delete resyncingSessionIds[sessionId];
    const historyResyncRequests = { ...state.historyResyncRequests };
    delete historyResyncRequests[sessionId];
    return withSessionMessages(state, sessionId, messages, { drafts, resyncingSessionIds, historyResyncRequests });
  }),

  deleteSession: (sessionId) => set((state) => {
    if (!isId(sessionId)) return state;
    const sessionMessages = { ...state.sessionMessages };
    delete sessionMessages[sessionId];
    const drafts = Object.fromEntries(
      Object.entries(state.drafts).filter(([, draft]) => draft.sessionId !== sessionId),
    );
    const blockedMessageKeys = Object.fromEntries(
      Object.entries(state.blockedMessageKeys).filter(([key]) => !key.startsWith(`${sessionId}\u0000`)),
    ) as Record<string, true>;
    const historyResyncRequests = { ...state.historyResyncRequests };
    delete historyResyncRequests[sessionId];
    const resyncingSessionIds = { ...state.resyncingSessionIds };
    delete resyncingSessionIds[sessionId];
    const next = {
      ...state,
      sessionMessages,
      drafts,
      blockedMessageKeys,
      historyResyncRequests,
      resyncingSessionIds,
      deletedSessionIds: { ...state.deletedSessionIds, [sessionId]: true as const },
    };
    return {
      sessionMessages,
      drafts,
      blockedMessageKeys,
      historyResyncRequests,
      resyncingSessionIds,
      deletedSessionIds: next.deletedSessionIds,
      ...(state.sessionId === sessionId ? { messages: [], streaming: false, streamingId: null } : {}),
    };
  }),

  reset: (sessionId) => set((state) => {
    const next = { ...state, sessionId };
    return { sessionId, ...selectedView(next, sessionId) };
  }),

  beginStream: (id) => get().beginMessage(get().sessionId, id),
  appendDelta: (delta) => {
    const state = get();
    if (state.streamingId) state.appendBlockDelta(state.sessionId, state.streamingId, 0, delta);
  },
  upsertToolCall: (toolCall) => {
    const state = get();
    if (!state.streamingId) return;
    const message = state.messages.find((candidate) => candidate.id === state.streamingId);
    const existing = message?.blocks?.findIndex(
      (block) => block.type === "toolUse" && block.id === toolCall.id,
    ) ?? -1;
    const blockIndex = existing >= 0 ? existing : (message?.blocks?.length ?? 0);
    state.upsertBlock(state.sessionId, state.streamingId, blockIndex, {
      type: "toolUse",
      id: toolCall.id,
      name: toolCall.name,
      input: toolCall.args,
      result: toolCall.result,
      isError: toolCall.isError,
    });
  },
}));

export function mintSessionId(): string {
  return newSessionId();
}
