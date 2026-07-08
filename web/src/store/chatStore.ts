/**
 * Chat store. The store holds the message list + streaming state; it never
 * edits the timeline directly. Every tool call a chat turn makes goes through
 * the Rust `EditCommand` authority, and the resulting timeline/media events
 * re-sync the project stores independently.
 */

import { create } from "zustand";
import type { ChatMessage, ChatToolCall } from "../lib/types";

interface ChatStoreState {
  sessionId: string;
  messages: ChatMessage[];
  streaming: boolean;
  streamingId: string | null;
  pushUser: (text: string) => void;
  beginStream: (id: string) => void;
  appendDelta: (delta: string) => void;
  upsertToolCall: (toolCall: ChatToolCall) => void;
  finalize: (message: ChatMessage) => void;
  setMessages: (messages: ChatMessage[]) => void;
  reset: (sessionId: string) => void;
}

function newSessionId(): string {
  return `chat-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function assistantTarget(messages: ChatMessage[], streamingId: string | null): number {
  if (streamingId) {
    const i = messages.findIndex((message) => message.id === streamingId);
    if (i >= 0) return i;
  }
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    if (messages[i].role === "assistant") return i;
  }
  return -1;
}

function mergeToolCall(message: ChatMessage, toolCall: ChatToolCall): ChatMessage {
  const index = message.toolCalls.findIndex((existing) => existing.id === toolCall.id);
  const toolCalls =
    index < 0
      ? [...message.toolCalls, toolCall]
      : message.toolCalls.map((existing, i) =>
          i === index ? { ...existing, ...toolCall } : existing,
        );
  return { ...message, toolCalls };
}

export const useChatStore = create<ChatStoreState>((set) => ({
  sessionId: newSessionId(),
  messages: [],
  streaming: false,
  streamingId: null,

  pushUser: (text) =>
    set((state) => ({
      messages: [
        ...state.messages,
        {
          id: `u-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
          role: "user",
          content: text,
          toolCalls: [],
          createdAt: Date.now(),
        },
      ],
    })),

  beginStream: (id) =>
    set((state) => ({
      streaming: true,
      streamingId: id,
      messages: [
        ...state.messages,
        {
          id,
          role: "assistant",
          content: "",
          toolCalls: [],
          createdAt: Date.now(),
        },
      ],
    })),

  appendDelta: (delta) =>
    set((state) => {
      const id = state.streamingId;
      if (!id) return state;
      return {
        messages: state.messages.map((message) =>
          message.id === id ? { ...message, content: message.content + delta } : message,
        ),
      };
    }),

  upsertToolCall: (toolCall) =>
    set((state) => {
      const target = assistantTarget(state.messages, state.streamingId);
      if (target < 0) return state;
      const messages = state.messages.slice();
      messages[target] = mergeToolCall(messages[target], toolCall);
      return { messages };
    }),

  finalize: (message) =>
    set((state) => {
      const id = state.streamingId;
      const finalizeMessage = (existing?: ChatMessage): ChatMessage => {
        if (!existing) return message;
        return {
          ...message,
          content: message.content.length > 0 ? message.content : existing.content,
          toolCalls:
            existing.toolCalls.length > 0 && message.toolCalls.length === 0
              ? existing.toolCalls
              : message.toolCalls,
        };
      };
      const messages =
        id != null
          ? state.messages.map((existing) =>
              existing.id === id ? finalizeMessage(existing) : existing,
            )
          : [...state.messages, message];
      return { messages, streaming: false, streamingId: null };
    }),

  setMessages: (messages) => set({ messages }),

  reset: (sessionId) =>
    set({
      sessionId,
      messages: [],
      streaming: false,
      streamingId: null,
    }),
}));

export function mintSessionId(): string {
  return newSessionId();
}
