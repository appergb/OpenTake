/**
 * Chat store (SPEC §10.1 read-only mirror discipline, applied to chat). The
 * store holds the message list + streaming state; it never edits the timeline
 * directly — every tool call a chat turn makes goes through the Rust
 * `EditCommand` authority, and the resulting `timeline_changed` event re-syncs
 * the project store. The chat store only tracks what the chat backend reported.
 *
 * Streaming: a single "in-flight" assistant message id accumulates `chat_delta`
 * chunks; on `chat_tool_call` the card is upserted by id (so the second emit,
 * with the result filled, replaces the placeholder); on `chat_done` the
 * in-flight message is finalized and the streaming flag clears.
 */

import { create } from "zustand";
import type { ChatMessage, ChatToolCall } from "../lib/types";

interface ChatStoreState {
  /** Stable session id (one conversation per editor open; minted on first send). */
  sessionId: string;
  /** The full message log (user / assistant / tool). */
  messages: ChatMessage[];
  /** True while a turn is streaming (disables the input + shows a cancel button). */
  streaming: boolean;
  /** The in-flight assistant message id (accumulates deltas until `chat_done`). */
  streamingId: string | null;

  /** Append a user message (sentimential; the backend also records it, but the
   *  UI mirrors it immediately for responsiveness). */
  pushUser: (text: string) => void;
  /** Start streaming a new assistant message; returns the id to attach deltas to. */
  beginStream: (id: string) => void;
  /** Append a text delta to the in-flight assistant message. */
  appendDelta: (delta: string) => void;
  /** Upsert a tool-call card on the in-flight (or last) assistant message. */
  upsertToolCall: (tc: ChatToolCall) => void;
  /** Finalize the in-flight message from `chat_done` and clear streaming. */
  finalize: (message: ChatMessage) => void;
  /** Replace the whole log (after `chat_history` on mount / reconnect). */
  setMessages: (messages: ChatMessage[]) => void;
  /** Reset the session (new conversation id; empty log). */
  reset: (sessionId: string) => void;
}

function newSessionId(): string {
  return `chat-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

/** Find the assistant message a tool call should attach to: the in-flight one,
 *  or the last assistant message. Returns its index (or -1 to append a new one). */
function assistantTarget(messages: ChatMessage[], streamingId: string | null): number {
  if (streamingId) {
    const i = messages.findIndex((m) => m.id === streamingId);
    if (i >= 0) return i;
  }
  for (let i = messages.length - 1; i >= 0; i--) {
    if (messages[i].role === "assistant") return i;
  }
  return -1;
}

/** Upsert a tool call into an assistant message by tool-call id (so the second
 *  emit — result-filled — replaces the placeholder, not appends). */
function mergeToolCall(msg: ChatMessage, tc: ChatToolCall): ChatMessage {
  const idx = msg.toolCalls.findIndex((c) => c.id === tc.id);
  const toolCalls =
    idx < 0
      ? [...msg.toolCalls, tc]
      : msg.toolCalls.map((c, i) => (i === idx ? { ...c, ...tc } : c));
  return { ...msg, toolCalls };
}

export const useChatStore = create<ChatStoreState>((set) => ({
  sessionId: newSessionId(),
  messages: [],
  streaming: false,
  streamingId: null,

  pushUser: (text) =>
    set((s) => ({
      messages: [
        ...s.messages,
        {
          id: `u-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
          role: "user" as const,
          content: text,
          toolCalls: [],
          createdAt: Date.now(),
        },
      ],
    })),

  beginStream: (id) =>
    set((s) => ({
      streaming: true,
      streamingId: id,
      messages: [
        ...s.messages,
        {
          id,
          role: "assistant" as const,
          content: "",
          toolCalls: [],
          createdAt: Date.now(),
        },
      ],
    })),

  appendDelta: (delta) =>
    set((s) => {
      const id = s.streamingId;
      if (!id) return s;
      return {
        messages: s.messages.map((m) =>
          m.id === id ? { ...m, content: m.content + delta } : m,
        ),
      };
    }),

  upsertToolCall: (tc) =>
    set((s) => {
      const target = assistantTarget(s.messages, s.streamingId);
      if (target < 0) return s;
      const messages = s.messages.slice();
      messages[target] = mergeToolCall(messages[target], tc);
      return { messages };
    }),

  finalize: (message) =>
    set((s) => {
      // The done message is the source of truth (full content + all tool calls
      // resolved); replace the in-flight placeholder by id.
      const id = s.streamingId;
      const messages =
        id != null
          ? s.messages.map((m) => (m.id === id ? message : m))
          : [...s.messages, message];
      return { messages, streaming: false, streamingId: null };
    }),

  setMessages: (messages) => set({ messages }),

  reset: (sessionId) =>
    set({ sessionId, messages: [], streaming: false, streamingId: null }),
}));

/** Mint a fresh session id (used when the panel mounts or the user clears). */
export function mintSessionId(): string {
  return newSessionId();
}
