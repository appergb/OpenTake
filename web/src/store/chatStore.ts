/**
 * Chat store. Rust remains the persisted authority. This store only holds
 * addressable, sequenced drafts so one malformed message cannot corrupt a
 * sibling stream or a different session.
 */

import { create } from "zustand";
import {
  MAX_CHAT_BLOCK_INDEX as BLOCK_INDEX_LIMIT,
  MAX_CHAT_DELTA_CHARS,
  MAX_CHAT_EVENT_BYTES,
  MAX_CHAT_EVENT_SEQUENCE,
  canonicalizeBoundedChatEvent,
  isBoundedAgentContentBlock,
  isBoundedChatId,
  isBoundedTerminalChatMessage,
  type AgentContentBlock,
  type ChatMessage,
  type ChatToolCall,
} from "../lib/types";

export const MAX_CHAT_BLOCK_INDEX = BLOCK_INDEX_LIMIT;
export const MAX_CHAT_RETAINED_SESSIONS = 32;
export const MAX_CHAT_DRAFTS = 64;
export const MAX_CHAT_DELETED_SESSIONS = 128;
const MAX_CHAT_BLOCKED_MESSAGES = 128;
const MAX_CHAT_RESYNC_SESSIONS = 32;
const MAX_CHAT_REPLAY_FINGERPRINTS = 64;
const MAX_CHAT_REPLAY_CANONICAL_CHARS = MAX_CHAT_EVENT_BYTES;

export type ChatHistoryResyncReason =
  | "block_gap"
  | "block_identity_mismatch"
  | "invalid_block"
  | "invalid_block_index"
  | "invalid_identity"
  | "invalid_message"
  | "invalid_sequence"
  | "missing_draft"
  | "message_identity_mismatch"
  | "sequence_conflict"
  | "sequence_gap"
  | "sequence_out_of_order";

export interface ChatHistoryResyncRequest {
  sessionId: string;
  messageId?: string;
  reason: ChatHistoryResyncReason;
}

interface ChatDraft {
  sessionId: string;
  messageId: string;
  nextSequence: number;
  finalized: boolean;
  eventFingerprints: Record<string, string>;
  fingerprintOrder: number[];
}

interface ChatStoreState {
  projectEpoch: number | null;
  projectPath: string | null;
  projectGeneration: number;
  sessionId: string;
  /** Derived mirror for the currently selected session only. */
  messages: ChatMessage[];
  streaming: boolean;
  streamingId: string | null;
  sessionMessages: Record<string, ChatMessage[]>;
  sessionOrder: string[];
  drafts: Record<string, ChatDraft>;
  draftOrder: string[];
  blockedMessageKeys: Record<string, true>;
  blockedMessageOrder: string[];
  historyResyncRequests: Record<string, ChatHistoryResyncRequest>;
  resyncingSessionIds: Record<string, true>;
  resyncSessionOrder: string[];
  deletedSessionIds: Record<string, true>;
  deletedSessionOrder: string[];
  sessionVersions: Record<string, number>;
  composerDraft: string | null;
  setComposerDraft: (draft: string | null) => void;
  pushUser: (text: string) => void;
  beginMessage: (sessionId: string, messageId: string) => void;
  appendBlockDelta: (
    sessionId: string,
    messageId: string,
    sequence: number,
    blockIndex: number,
    delta: string,
  ) => void;
  upsertBlock: (
    sessionId: string,
    messageId: string,
    sequence: number,
    blockIndex: number,
    block: AgentContentBlock,
  ) => void;
  finalize: {
    (sessionId: string, messageId: string, sequence: number, message: ChatMessage): void;
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
  installSessionSnapshot: (
    sessionId: string,
    messages: ChatMessage[],
    projectGeneration: number,
    expectedSessionVersion: number,
  ) => boolean;
  deleteSession: (sessionId: string) => void;
  resetProject: (sessionId: string, projectEpoch: number, projectPath: string | null) => boolean;
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

function isBlockIndex(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 && value <= MAX_CHAT_BLOCK_INDEX;
}

function isSequence(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 && value <= MAX_CHAT_EVENT_SEQUENCE;
}

function touch(order: string[], value: string): string[] {
  return [...order.filter((candidate) => candidate !== value), value];
}

function touchedSessionVersions(state: ChatStoreState, sessionId: string): Record<string, number> {
  return {
    ...state.sessionVersions,
    [sessionId]: (state.sessionVersions[sessionId] ?? 0) + 1,
  };
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

function sameBlockIdentity(left: AgentContentBlock, right: AgentContentBlock): boolean {
  if (left.type !== right.type) return false;
  if (left.type === "text" && right.type === "text") return true;
  if (left.type === "toolUse" && right.type === "toolUse") {
    return left.id === right.id && left.name === right.name;
  }
  return left.type === "toolResult" && right.type === "toolResult" && left.toolUseId === right.toolUseId;
}

function activeDraft(state: ChatStoreState, sessionId: string): ChatDraft | undefined {
  for (let index = state.draftOrder.length - 1; index >= 0; index -= 1) {
    const key = state.draftOrder[index];
    const draft = state.drafts[key];
    if (
      draft?.sessionId === sessionId &&
      !draft.finalized &&
      !state.blockedMessageKeys[key]
    ) {
      return draft;
    }
  }
  return undefined;
}

function selectedView(state: ChatStoreState) {
  const draft = activeDraft(state, state.sessionId);
  return {
    messages: state.sessionMessages[state.sessionId] ?? [],
    streaming: draft !== undefined,
    streamingId: draft?.messageId ?? null,
  };
}

function removeSessionState(state: ChatStoreState, sessionId: string): void {
  delete state.sessionMessages[sessionId];
  delete state.sessionVersions[sessionId];
  delete state.historyResyncRequests[sessionId];
  delete state.resyncingSessionIds[sessionId];
  state.sessionOrder = state.sessionOrder.filter((candidate) => candidate !== sessionId);
  state.resyncSessionOrder = state.resyncSessionOrder.filter((candidate) => candidate !== sessionId);
  for (const [key, draft] of Object.entries(state.drafts)) {
    if (draft.sessionId === sessionId) delete state.drafts[key];
  }
  state.draftOrder = state.draftOrder.filter((key) => state.drafts[key] !== undefined);
  for (const key of Object.keys(state.blockedMessageKeys)) {
    if (key.startsWith(`${sessionId}\u0000`)) delete state.blockedMessageKeys[key];
  }
  state.blockedMessageOrder = state.blockedMessageOrder.filter(
    (key) => state.blockedMessageKeys[key] !== undefined,
  );
}

/** Clone mutable collections, enforce all retention limits, then recompute the selected view. */
function reconcile(input: ChatStoreState): ChatStoreState {
  const state: ChatStoreState = {
    ...input,
    sessionMessages: { ...input.sessionMessages },
    sessionOrder: [...input.sessionOrder],
    drafts: { ...input.drafts },
    draftOrder: [...input.draftOrder],
    blockedMessageKeys: { ...input.blockedMessageKeys },
    blockedMessageOrder: [...input.blockedMessageOrder],
    historyResyncRequests: { ...input.historyResyncRequests },
    resyncingSessionIds: { ...input.resyncingSessionIds },
    resyncSessionOrder: [...input.resyncSessionOrder],
    deletedSessionIds: { ...input.deletedSessionIds },
    deletedSessionOrder: [...input.deletedSessionOrder],
    sessionVersions: { ...input.sessionVersions },
  };

  state.sessionOrder = state.sessionOrder.filter(
    (sessionId, index, order) => state.sessionMessages[sessionId] !== undefined && order.indexOf(sessionId) === index,
  );
  for (const sessionId of Object.keys(state.sessionMessages)) {
    if (!state.sessionOrder.includes(sessionId)) state.sessionOrder.push(sessionId);
  }
  while (Object.keys(state.sessionMessages).length > MAX_CHAT_RETAINED_SESSIONS) {
    const victim = state.sessionOrder.find((sessionId) => sessionId !== state.sessionId);
    if (!victim) break;
    removeSessionState(state, victim);
  }

  state.draftOrder = state.draftOrder.filter(
    (key, index, order) => state.drafts[key] !== undefined && order.indexOf(key) === index,
  );
  for (const key of Object.keys(state.drafts)) {
    if (!state.draftOrder.includes(key)) state.draftOrder.push(key);
  }
  while (Object.keys(state.drafts).length > MAX_CHAT_DRAFTS) {
    const key = state.draftOrder.shift();
    if (!key) break;
    const draft = state.drafts[key];
    delete state.drafts[key];
    delete state.blockedMessageKeys[key];
    state.blockedMessageOrder = state.blockedMessageOrder.filter((candidate) => candidate !== key);
    if (draft && !draft.finalized) {
      const messages = state.sessionMessages[draft.sessionId];
      if (messages) {
        state.sessionMessages[draft.sessionId] = messages.filter((message) => message.id !== draft.messageId);
      }
    }
  }

  state.blockedMessageOrder = state.blockedMessageOrder.filter(
    (key, index, order) => state.blockedMessageKeys[key] !== undefined && order.indexOf(key) === index,
  );
  while (Object.keys(state.blockedMessageKeys).length > MAX_CHAT_BLOCKED_MESSAGES) {
    const key = state.blockedMessageOrder.shift();
    if (!key) break;
    delete state.blockedMessageKeys[key];
    delete state.drafts[key];
    state.draftOrder = state.draftOrder.filter((candidate) => candidate !== key);
  }

  state.deletedSessionOrder = state.deletedSessionOrder.filter(
    (sessionId, index, order) => state.deletedSessionIds[sessionId] !== undefined && order.indexOf(sessionId) === index,
  );
  while (Object.keys(state.deletedSessionIds).length > MAX_CHAT_DELETED_SESSIONS) {
    const sessionId = state.deletedSessionOrder.shift();
    if (!sessionId) break;
    delete state.deletedSessionIds[sessionId];
  }

  state.resyncSessionOrder = state.resyncSessionOrder.filter(
    (sessionId, index, order) => state.resyncingSessionIds[sessionId] !== undefined && order.indexOf(sessionId) === index,
  );
  while (Object.keys(state.resyncingSessionIds).length > MAX_CHAT_RESYNC_SESSIONS) {
    const sessionId = state.resyncSessionOrder.shift();
    if (!sessionId) break;
    delete state.resyncingSessionIds[sessionId];
    delete state.historyResyncRequests[sessionId];
  }

  return { ...state, ...selectedView(state) };
}

function stopMessage(
  state: ChatStoreState,
  sessionId: string,
  messageId: string | undefined,
  reason: ChatHistoryResyncReason,
): ChatStoreState {
  if (!isBoundedChatId(sessionId) || state.deletedSessionIds[sessionId]) return state;
  const key = messageId && isBoundedChatId(messageId) ? draftKey(sessionId, messageId) : undefined;
  if (key && state.blockedMessageKeys[key]) return state;

  const blockedMessageKeys = key
    ? { ...state.blockedMessageKeys, [key]: true as const }
    : state.blockedMessageKeys;
  const blockedMessageOrder = key ? touch(state.blockedMessageOrder, key) : state.blockedMessageOrder;
  const firstResyncForSession = !state.resyncingSessionIds[sessionId];
  return reconcile({
    ...state,
    sessionVersions: touchedSessionVersions(state, sessionId),
    blockedMessageKeys,
    blockedMessageOrder,
    resyncingSessionIds: firstResyncForSession
      ? { ...state.resyncingSessionIds, [sessionId]: true as const }
      : state.resyncingSessionIds,
    resyncSessionOrder: firstResyncForSession
      ? touch(state.resyncSessionOrder, sessionId)
      : state.resyncSessionOrder,
    historyResyncRequests: firstResyncForSession
      ? {
          ...state.historyResyncRequests,
          [sessionId]: { sessionId, ...(messageId ? { messageId } : {}), reason },
        }
      : state.historyResyncRequests,
  });
}

type SequenceAddressDecision = "apply" | "compare" | ChatHistoryResyncReason;

function sequenceAddressDecision(
  draft: ChatDraft,
  sequence: number,
): SequenceAddressDecision {
  if (!draft.finalized && sequence === draft.nextSequence) return "apply";
  if (sequence > draft.nextSequence) return "sequence_gap";
  const previous = draft.eventFingerprints[String(sequence)];
  if (previous === undefined) return "sequence_out_of_order";
  return "compare";
}

function sequencePayloadDecision(
  draft: ChatDraft,
  sequence: number,
  fingerprint: string,
  addressDecision: "apply" | "compare",
): "apply" | "retry" | "sequence_conflict" {
  if (addressDecision === "apply") return "apply";
  return draft.eventFingerprints[String(sequence)] === fingerprint ? "retry" : "sequence_conflict";
}

function advanceDraft(
  draft: ChatDraft,
  sequence: number,
  fingerprint: string,
  finalized = false,
): ChatDraft {
  const eventFingerprints: Record<string, string> = {
    ...draft.eventFingerprints,
    [sequence]: fingerprint,
  };
  const fingerprintOrder = [...draft.fingerprintOrder.filter((value) => value !== sequence), sequence];
  let retainedChars = fingerprintOrder.reduce(
    (total, retainedSequence) => total + (eventFingerprints[String(retainedSequence)]?.length ?? 0),
    0,
  );
  while (
    fingerprintOrder.length > MAX_CHAT_REPLAY_FINGERPRINTS ||
    retainedChars > MAX_CHAT_REPLAY_CANONICAL_CHARS
  ) {
    const expired = fingerprintOrder.shift();
    if (expired !== undefined) {
      retainedChars -= eventFingerprints[String(expired)]?.length ?? 0;
      delete eventFingerprints[String(expired)];
    }
  }
  return {
    ...draft,
    nextSequence: sequence + 1,
    finalized,
    eventFingerprints,
    fingerprintOrder,
  };
}

function withUpdatedMessage(
  state: ChatStoreState,
  sessionId: string,
  messageId: string,
  replacement: ChatMessage,
  draft: ChatDraft,
): ChatStoreState {
  const messages = state.sessionMessages[sessionId];
  const index = messages?.findIndex((message) => message.id === messageId) ?? -1;
  if (!messages || index < 0) return stopMessage(state, sessionId, messageId, "missing_draft");
  const nextMessages = messages.slice();
  nextMessages[index] = replacement;
  const key = draftKey(sessionId, messageId);
  return reconcile({
    ...state,
    sessionVersions: touchedSessionVersions(state, sessionId),
    sessionMessages: { ...state.sessionMessages, [sessionId]: nextMessages },
    sessionOrder: touch(state.sessionOrder, sessionId),
    drafts: { ...state.drafts, [key]: draft },
    draftOrder: touch(state.draftOrder, key),
  });
}

function installSessionMessages(
  state: ChatStoreState,
  sessionId: string,
  messages: ChatMessage[],
): ChatStoreState {
  const drafts = Object.fromEntries(
    Object.entries(state.drafts).filter(([, draft]) => draft.sessionId !== sessionId),
  );
  const draftOrder = state.draftOrder.filter((key) => drafts[key] !== undefined);
  const blockedMessageKeys = Object.fromEntries(
    Object.entries(state.blockedMessageKeys).filter(([key]) => !key.startsWith(`${sessionId}\u0000`)),
  ) as Record<string, true>;
  const blockedMessageOrder = state.blockedMessageOrder.filter(
    (key) => blockedMessageKeys[key] !== undefined,
  );
  const resyncingSessionIds = { ...state.resyncingSessionIds };
  delete resyncingSessionIds[sessionId];
  const historyResyncRequests = { ...state.historyResyncRequests };
  delete historyResyncRequests[sessionId];
  return reconcile({
    ...state,
    sessionMessages: { ...state.sessionMessages, [sessionId]: messages },
    sessionOrder: touch(state.sessionOrder, sessionId),
    sessionVersions: touchedSessionVersions(state, sessionId),
    drafts,
    draftOrder,
    blockedMessageKeys,
    blockedMessageOrder,
    resyncingSessionIds,
    resyncSessionOrder: state.resyncSessionOrder.filter((candidate) => candidate !== sessionId),
    historyResyncRequests,
  });
}

export const useChatStore = create<ChatStoreState>((set, get) => ({
  projectEpoch: null,
  projectPath: null,
  projectGeneration: 0,
  sessionId: newSessionId(),
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
  sessionVersions: {},
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
    return reconcile({
      ...state,
      sessionVersions: touchedSessionVersions(state, state.sessionId),
      sessionMessages: {
        ...state.sessionMessages,
        [state.sessionId]: [...(state.sessionMessages[state.sessionId] ?? []), message],
      },
      sessionOrder: touch(state.sessionOrder, state.sessionId),
    });
  }),

  beginMessage: (sessionId, messageId) => set((state) => {
    if (!isBoundedChatId(sessionId) || !isBoundedChatId(messageId)) {
      return stopMessage(state, sessionId, isBoundedChatId(messageId) ? messageId : undefined, "invalid_identity");
    }
    if (state.deletedSessionIds[sessionId]) return state;
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
    return reconcile({
      ...state,
      sessionVersions: touchedSessionVersions(state, sessionId),
      sessionMessages: { ...state.sessionMessages, [sessionId]: [...messages, message] },
      sessionOrder: touch(state.sessionOrder, sessionId),
      drafts: {
        ...state.drafts,
        [key]: {
          sessionId,
          messageId,
          nextSequence: 0,
          finalized: false,
          eventFingerprints: {},
          fingerprintOrder: [],
        },
      },
      draftOrder: touch(state.draftOrder, key),
    });
  }),

  appendBlockDelta: (sessionId, messageId, sequence, blockIndex, delta) => set((state) => {
    if (!isBoundedChatId(sessionId) || !isBoundedChatId(messageId)) {
      return stopMessage(state, sessionId, isBoundedChatId(messageId) ? messageId : undefined, "invalid_identity");
    }
    if (!isSequence(sequence)) return stopMessage(state, sessionId, messageId, "invalid_sequence");
    if (!isBlockIndex(blockIndex)) return stopMessage(state, sessionId, messageId, "invalid_block_index");
    if (typeof delta !== "string" || delta.length > MAX_CHAT_DELTA_CHARS) {
      return stopMessage(state, sessionId, messageId, "invalid_block");
    }
    const key = draftKey(sessionId, messageId);
    if (state.deletedSessionIds[sessionId] || state.blockedMessageKeys[key]) return state;
    const draft = state.drafts[key];
    if (!draft) return stopMessage(state, sessionId, messageId, "missing_draft");
    const addressDecision = sequenceAddressDecision(draft, sequence);
    if (addressDecision !== "apply" && addressDecision !== "compare") {
      return stopMessage(state, sessionId, messageId, addressDecision);
    }
    const fingerprint = canonicalizeBoundedChatEvent({
      type: "blockDelta",
      sessionId,
      messageId,
      blockIndex,
      delta,
    });
    if (fingerprint === null) return stopMessage(state, sessionId, messageId, "invalid_block");
    const decision = sequencePayloadDecision(draft, sequence, fingerprint, addressDecision);
    if (decision === "retry") return state;
    if (decision !== "apply") return stopMessage(state, sessionId, messageId, decision);
    const messages = state.sessionMessages[sessionId];
    const index = messages?.findIndex((message) => message.id === messageId) ?? -1;
    if (!messages || index < 0) return stopMessage(state, sessionId, messageId, "missing_draft");
    const message = messages[index];
    const blocks = [...(message.blocks ?? [])];
    if (blockIndex > blocks.length) return stopMessage(state, sessionId, messageId, "block_gap");
    if (blockIndex === blocks.length) {
      blocks.push({ type: "text", text: delta });
    } else if (blocks[blockIndex].type === "text") {
      blocks[blockIndex] = { type: "text", text: blocks[blockIndex].text + delta };
    } else {
      return stopMessage(state, sessionId, messageId, "block_identity_mismatch");
    }
    return withUpdatedMessage(
      state,
      sessionId,
      messageId,
      deriveAssistantFields(message, blocks),
      advanceDraft(draft, sequence, fingerprint),
    );
  }),

  upsertBlock: (sessionId, messageId, sequence, blockIndex, block) => set((state) => {
    if (!isBoundedChatId(sessionId) || !isBoundedChatId(messageId)) {
      return stopMessage(state, sessionId, isBoundedChatId(messageId) ? messageId : undefined, "invalid_identity");
    }
    if (!isSequence(sequence)) return stopMessage(state, sessionId, messageId, "invalid_sequence");
    if (!isBlockIndex(blockIndex)) return stopMessage(state, sessionId, messageId, "invalid_block_index");
    const key = draftKey(sessionId, messageId);
    if (state.deletedSessionIds[sessionId] || state.blockedMessageKeys[key]) return state;
    const draft = state.drafts[key];
    if (!draft) return stopMessage(state, sessionId, messageId, "missing_draft");
    const addressDecision = sequenceAddressDecision(draft, sequence);
    if (addressDecision !== "apply" && addressDecision !== "compare") {
      return stopMessage(state, sessionId, messageId, addressDecision);
    }
    if (!isBoundedAgentContentBlock(block)) return stopMessage(state, sessionId, messageId, "invalid_block");
    const fingerprint = canonicalizeBoundedChatEvent({
      type: "blockUpsert",
      sessionId,
      messageId,
      blockIndex,
      block,
    });
    if (fingerprint === null) return stopMessage(state, sessionId, messageId, "invalid_block");
    const decision = sequencePayloadDecision(draft, sequence, fingerprint, addressDecision);
    if (decision === "retry") return state;
    if (decision !== "apply") return stopMessage(state, sessionId, messageId, decision);
    const messages = state.sessionMessages[sessionId];
    const index = messages?.findIndex((message) => message.id === messageId) ?? -1;
    if (!messages || index < 0) return stopMessage(state, sessionId, messageId, "missing_draft");
    const message = messages[index];
    const blocks = [...(message.blocks ?? [])];
    if (blockIndex > blocks.length) return stopMessage(state, sessionId, messageId, "block_gap");
    if (blockIndex === blocks.length) {
      blocks.push(block);
    } else if (!sameBlockIdentity(blocks[blockIndex], block)) {
      return stopMessage(state, sessionId, messageId, "block_identity_mismatch");
    } else {
      blocks[blockIndex] = block;
    }
    return withUpdatedMessage(
      state,
      sessionId,
      messageId,
      deriveAssistantFields(message, blocks),
      advanceDraft(draft, sequence, fingerprint),
    );
  }),

  finalize: ((first: string | ChatMessage, second?: string, third?: number, fourth?: ChatMessage) => {
    if (typeof first !== "string") {
      set((state) => {
        const messageId = state.streamingId;
        if (!messageId || first.id !== messageId) {
          return messageId
            ? stopMessage(state, state.sessionId, messageId, "message_identity_mismatch")
            : state;
        }
        const key = draftKey(state.sessionId, messageId);
        const draft = state.drafts[key];
        if (!draft || !isBoundedTerminalChatMessage(first, messageId)) {
          return stopMessage(state, state.sessionId, messageId, draft ? "invalid_message" : "missing_draft");
        }
        const messages = state.sessionMessages[state.sessionId];
        const index = messages?.findIndex((message) => message.id === messageId) ?? -1;
        if (!messages || index < 0) return stopMessage(state, state.sessionId, messageId, "missing_draft");
        const sequence = draft.nextSequence;
        const fingerprint = canonicalizeBoundedChatEvent({
          type: "done",
          sessionId: state.sessionId,
          messageId,
          message: first,
        });
        if (fingerprint === null) return stopMessage(state, state.sessionId, messageId, "invalid_message");
        const nextMessages = messages.slice();
        nextMessages[index] = first;
        return reconcile({
          ...state,
          sessionVersions: touchedSessionVersions(state, state.sessionId),
          sessionMessages: { ...state.sessionMessages, [state.sessionId]: nextMessages },
          sessionOrder: touch(state.sessionOrder, state.sessionId),
          drafts: {
            ...state.drafts,
            [key]: advanceDraft(draft, sequence, fingerprint, true),
          },
          draftOrder: touch(state.draftOrder, key),
        });
      });
      return;
    }
    const sessionId = first;
    const messageId = second;
    const sequence = third;
    const message = fourth;
    set((state) => {
      if (!isBoundedChatId(sessionId) || !isBoundedChatId(messageId)) {
        return stopMessage(state, sessionId, isBoundedChatId(messageId) ? messageId : undefined, "invalid_identity");
      }
      if (!message || message.id !== messageId) {
        return stopMessage(state, sessionId, messageId, "message_identity_mismatch");
      }
      if (!isSequence(sequence)) return stopMessage(state, sessionId, messageId, "invalid_sequence");
      const key = draftKey(sessionId, messageId);
      if (state.deletedSessionIds[sessionId] || state.blockedMessageKeys[key]) return state;
      const draft = state.drafts[key];
      if (!draft) return stopMessage(state, sessionId, messageId, "missing_draft");
      const addressDecision = sequenceAddressDecision(draft, sequence);
      if (addressDecision !== "apply" && addressDecision !== "compare") {
        return stopMessage(state, sessionId, messageId, addressDecision);
      }
      if (!isBoundedTerminalChatMessage(message, messageId)) {
        return stopMessage(state, sessionId, messageId, "invalid_message");
      }
      const fingerprint = canonicalizeBoundedChatEvent({ type: "done", sessionId, messageId, message });
      if (fingerprint === null) return stopMessage(state, sessionId, messageId, "invalid_message");
      const decision = sequencePayloadDecision(draft, sequence, fingerprint, addressDecision);
      if (decision === "retry") return state;
      if (decision !== "apply") return stopMessage(state, sessionId, messageId, decision);
      const messages = state.sessionMessages[sessionId];
      const index = messages?.findIndex((candidate) => candidate.id === messageId) ?? -1;
      if (!messages || index < 0) return stopMessage(state, sessionId, messageId, "missing_draft");
      const nextMessages = messages.slice();
      nextMessages[index] = message;
      return reconcile({
        ...state,
        sessionVersions: touchedSessionVersions(state, sessionId),
        sessionMessages: { ...state.sessionMessages, [sessionId]: nextMessages },
        sessionOrder: touch(state.sessionOrder, sessionId),
        drafts: {
          ...state.drafts,
          [key]: advanceDraft(draft, sequence, fingerprint, true),
        },
        draftOrder: touch(state.draftOrder, key),
      });
    });
  }) as ChatStoreState["finalize"],

  requestHistoryResync: (sessionId, messageId, reason) => set((state) =>
    stopMessage(state, sessionId, messageId, reason),
  ),

  takeHistoryResyncRequest: () => {
    const state = get();
    const sessionId = state.resyncSessionOrder.find(
      (candidate) => state.historyResyncRequests[candidate] !== undefined,
    );
    if (!sessionId) return null;
    const request = state.historyResyncRequests[sessionId];
    set((current) => {
      const historyResyncRequests = { ...current.historyResyncRequests };
      delete historyResyncRequests[sessionId];
      return { historyResyncRequests };
    });
    return request;
  },

  setMessages: (messages) => get().setMessagesForSession(get().sessionId, messages),

  setMessagesForSession: (sessionId, messages) => set((state) => {
    if (!isBoundedChatId(sessionId) || state.deletedSessionIds[sessionId]) return state;
    return installSessionMessages(state, sessionId, messages);
  }),

  installSessionSnapshot: (sessionId, messages, projectGeneration, expectedSessionVersion) => {
    let installed = false;
    set((state) => {
      if (
        !isBoundedChatId(sessionId) ||
        state.deletedSessionIds[sessionId] ||
        state.projectGeneration !== projectGeneration ||
        (state.sessionVersions[sessionId] ?? 0) !== expectedSessionVersion
      ) {
        return state;
      }
      installed = true;
      return installSessionMessages(state, sessionId, messages);
    });
    return installed;
  },

  deleteSession: (sessionId) => set((state) => {
    if (!isBoundedChatId(sessionId)) return state;
    const next = reconcile({
      ...state,
      deletedSessionIds: { ...state.deletedSessionIds, [sessionId]: true as const },
      deletedSessionOrder: touch(state.deletedSessionOrder, sessionId),
    });
    removeSessionState(next, sessionId);
    return reconcile(next);
  }),

  resetProject: (sessionId, projectEpoch, projectPath) => {
    if (
      !isBoundedChatId(sessionId) ||
      !Number.isSafeInteger(projectEpoch) ||
      projectEpoch < 0 ||
      (projectPath !== null && (projectPath.length === 0 || projectPath.length > 4096))
    ) {
      return false;
    }
    let changed = false;
    set((state) => {
      if (state.projectEpoch === projectEpoch && state.projectPath === projectPath) return state;
      changed = true;
      const projectGeneration = state.projectGeneration >= Number.MAX_SAFE_INTEGER
        ? 1
        : state.projectGeneration + 1;
      return reconcile({
        ...state,
        projectEpoch,
        projectPath,
        projectGeneration,
        sessionId,
        sessionMessages: {},
        sessionOrder: [],
        sessionVersions: {},
        drafts: {},
        draftOrder: [],
        blockedMessageKeys: {},
        blockedMessageOrder: [],
        historyResyncRequests: {},
        resyncingSessionIds: {},
        resyncSessionOrder: [],
        deletedSessionIds: {},
        deletedSessionOrder: [],
        composerDraft: null,
      });
    });
    return changed;
  },

  reset: (sessionId) => set((state) => {
    if (!isBoundedChatId(sessionId)) return state;
    return reconcile({ ...state, sessionId });
  }),

  beginStream: (id) => get().beginMessage(get().sessionId, id),
  appendDelta: (delta) => {
    const state = get();
    if (!state.streamingId) return;
    const draft = state.drafts[draftKey(state.sessionId, state.streamingId)];
    if (draft) state.appendBlockDelta(state.sessionId, state.streamingId, draft.nextSequence, 0, delta);
  },
  upsertToolCall: (toolCall) => {
    const state = get();
    if (!state.streamingId) return;
    const draft = state.drafts[draftKey(state.sessionId, state.streamingId)];
    if (!draft) return;
    const message = state.messages.find((candidate) => candidate.id === state.streamingId);
    const existing = message?.blocks?.findIndex(
      (block) => block.type === "toolUse" && block.id === toolCall.id,
    ) ?? -1;
    const blockIndex = existing >= 0 ? existing : (message?.blocks?.length ?? 0);
    state.upsertBlock(state.sessionId, state.streamingId, draft.nextSequence, blockIndex, {
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
