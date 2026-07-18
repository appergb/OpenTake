// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const apiMocks = vi.hoisted(() => ({
  chatSessions: vi.fn(),
  chatSend: vi.fn(),
  doneHandler: undefined as undefined | ((event: Record<string, unknown>) => void),
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: string) => key,
}));

vi.mock("../../lib/api", () => ({
  isTauri: true,
  chatCancel: vi.fn(async () => {}),
  chatHistory: vi.fn(async () => []),
  chatSend: apiMocks.chatSend,
  chatSessions: apiMocks.chatSessions,
  onChatDelta: vi.fn(async () => () => {}),
  onChatToolCall: vi.fn(async () => () => {}),
  onChatDone: vi.fn(async (handler: (event: Record<string, unknown>) => void) => {
    apiMocks.doneHandler = handler;
    return () => {};
  }),
}));

import { useChatStore } from "../../store/chatStore";
import { useProjectStore } from "../../store/projectStore";
import { AgentPanel } from "./AgentPanel";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let root: Root | null = null;
let container: HTMLDivElement | null = null;

beforeEach(() => {
  useProjectStore.setState({ projectEpoch: 41, projectPath: "/tmp/Current.opentake" });
  useChatStore.getState().reset("stale-session");
  apiMocks.doneHandler = undefined;
  apiMocks.chatSessions.mockReset();
  apiMocks.chatSend.mockReset();
  apiMocks.chatSend.mockResolvedValue(undefined);
  apiMocks.chatSessions.mockResolvedValue([
    {
      id: "chat-restored",
      messages: [
        {
          id: "m1",
          role: "user",
          content: "persisted",
          toolCalls: [],
          createdAt: 1,
        },
      ],
      createdAt: 1,
    },
  ]);
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  if (root) await act(async () => root?.unmount());
  container?.remove();
  root = null;
  container = null;
});

describe("AgentPanel project sessions", () => {
  it("restores the current project and rejects stale-project events", async () => {
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(apiMocks.chatSessions).toHaveBeenCalledTimes(1);
    expect(useChatStore.getState().sessionId).toBe("chat-restored");
    expect(useChatStore.getState().messages.map((message) => message.content)).toEqual([
      "persisted",
    ]);

    await act(async () => {
      apiMocks.doneHandler?.({
        projectEpoch: 40,
        projectPath: "/tmp/Previous.opentake",
        sessionId: "chat-restored",
        message: {
          id: "stale",
          role: "assistant",
          content: "stale",
          toolCalls: [],
          createdAt: 2,
        },
      });
    });
    expect(useChatStore.getState().messages).toHaveLength(1);

    await act(async () => {
      apiMocks.doneHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/SaveAsTarget.opentake",
        sessionId: "chat-restored",
        message: {
          id: "stale-path",
          role: "assistant",
          content: "stale path",
          toolCalls: [],
          createdAt: 2,
        },
      });
    });
    expect(useChatStore.getState().messages).toHaveLength(1);

    await act(async () => {
      apiMocks.doneHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-restored",
        message: {
          id: "current",
          role: "assistant",
          content: "current",
          toolCalls: [],
          createdAt: 3,
        },
      });
    });
    expect(useChatStore.getState().messages.map((message) => message.content)).toEqual([
      "persisted",
      "current",
    ]);
  });

  it("clears the previous project when the replacement has no saved path", async () => {
    useChatStore.setState({
      sessionId: "chat-old-project",
      messages: [
        {
          id: "old",
          role: "assistant",
          content: "must not leak",
          toolCalls: [],
          createdAt: 1,
        },
      ],
      streaming: true,
      streamingId: "old",
    });
    useProjectStore.setState({ projectEpoch: 42, projectPath: null });

    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
    });

    expect(apiMocks.chatSessions).not.toHaveBeenCalled();
    expect(useChatStore.getState().sessionId).not.toBe("chat-old-project");
    expect(useChatStore.getState().messages).toEqual([]);
    expect(useChatStore.getState().streaming).toBe(false);
  });

  it("does not finalize a rejected send into a replacement project", async () => {
    let rejectSend: (reason: Error) => void = () => {};
    apiMocks.chatSend.mockReturnValueOnce(
      new Promise<void>((_resolve, reject) => {
        rejectSend = reject;
      }),
    );
    apiMocks.chatSessions.mockResolvedValue([]);
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
    });

    const textarea = container?.querySelector("textarea");
    expect(textarea).not.toBeNull();
    await act(async () => {
      if (!textarea) return;
      const valueSetter = Object.getOwnPropertyDescriptor(
        HTMLTextAreaElement.prototype,
        "value",
      )?.set;
      valueSetter?.call(textarea, "old project request");
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      await Promise.resolve();
      textarea.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
      await Promise.resolve();
    });
    expect(apiMocks.chatSend).toHaveBeenCalledTimes(1);

    await act(async () => {
      useProjectStore.setState({ projectEpoch: 42, projectPath: null });
      rejectSend(new Error("old project failed"));
      await Promise.resolve();
    });

    expect(useChatStore.getState().messages).toEqual([]);
    expect(useChatStore.getState().streaming).toBe(false);
  });
});
