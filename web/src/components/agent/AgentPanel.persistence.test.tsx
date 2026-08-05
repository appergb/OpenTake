// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const apiMocks = vi.hoisted(() => ({
  chatHistory: vi.fn(),
  chatSessionSetOpen: vi.fn(),
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
  chatHistory: apiMocks.chatHistory,
  chatSend: apiMocks.chatSend,
  chatSessionSetOpen: apiMocks.chatSessionSetOpen,
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
import { useSettingsStore } from "../../store/settingsStore";
import { AgentPanel } from "./AgentPanel";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let root: Root | null = null;
let container: HTMLDivElement | null = null;

beforeEach(() => {
  useProjectStore.setState({ projectEpoch: 41, projectPath: "/tmp/Current.opentake" });
  useSettingsStore.setState({ byokProvider: "anthropic" });
  useChatStore.getState().reset("stale-session");
  apiMocks.doneHandler = undefined;
  apiMocks.chatHistory.mockReset();
  apiMocks.chatSessionSetOpen.mockReset();
  apiMocks.chatSessions.mockReset();
  apiMocks.chatSend.mockReset();
  apiMocks.chatSend.mockResolvedValue(undefined);
  apiMocks.chatHistory.mockResolvedValue([]);
  apiMocks.chatSessionSetOpen.mockImplementation(
    async (sessionId: string, isOpen: boolean) => ({
      id: sessionId,
      messages: [],
      createdAt: Date.now(),
      isOpen,
    }),
  );
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
      isOpen: true,
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
  it("routes Agent chat directly through the selected official Codex provider", async () => {
    useSettingsStore.setState({ byokProvider: "codex" });
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });

    const textarea = container?.querySelector<HTMLTextAreaElement>("textarea")!;
    await act(async () => {
      const valueSetter = Object.getOwnPropertyDescriptor(
        HTMLTextAreaElement.prototype,
        "value",
      )?.set;
      valueSetter?.call(textarea, "用官方 Codex 调整时间线");
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      textarea.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
      await Promise.resolve();
    });

    expect(apiMocks.chatSend).toHaveBeenCalledWith(
      "chat-restored",
      "用官方 Codex 调整时间线",
      "codex",
      41,
      "/tmp/Current.opentake",
    );
  });

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

  it("restores only open tabs and switches using the loaded project snapshot", async () => {
    apiMocks.chatSessions.mockResolvedValueOnce([
      {
        id: "chat-first",
        messages: [
          { id: "u1", role: "user", content: "First edit", toolCalls: [], createdAt: 1 },
        ],
        createdAt: 1,
        isOpen: true,
      },
      {
        id: "chat-second",
        messages: [
          { id: "u2", role: "user", content: "Second edit", toolCalls: [], createdAt: 2 },
        ],
        createdAt: 2,
        isOpen: true,
      },
      {
        id: "chat-closed",
        messages: [
          { id: "u3", role: "user", content: "Closed edit", toolCalls: [], createdAt: 3 },
        ],
        createdAt: 3,
        isOpen: false,
      },
    ]);
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });

    const tabs = Array.from(container?.querySelectorAll('[role="tab"]') ?? []);
    expect(tabs.map((tab) => tab.getAttribute("aria-label"))).toEqual([
      "First edit",
      "Second edit",
    ]);

    await act(async () => {
      (tabs[1] as HTMLButtonElement).click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(apiMocks.chatHistory).not.toHaveBeenCalled();
    expect(useChatStore.getState().sessionId).toBe("chat-second");
    expect(useChatStore.getState().messages[0].content).toBe("Second edit");
  });

  it("persists new and closed tab state", async () => {
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });

    const newTab = container?.querySelector(
      'button[aria-label="agent.newTab"]',
    ) as HTMLButtonElement | null;
    expect(newTab).not.toBeNull();
    await act(async () => {
      newTab?.click();
      await Promise.resolve();
      await Promise.resolve();
    });
    const createCall = apiMocks.chatSessionSetOpen.mock.calls.find((call) => call[1] === true);
    expect(createCall).toBeDefined();
    const createdId = createCall?.[0] as string;
    expect(useChatStore.getState().sessionId).toBe(createdId);

    const selected = container?.querySelector(
      '[role="tab"][aria-selected="true"]',
    ) as HTMLButtonElement | null;
    const close = selected?.nextElementSibling as HTMLButtonElement | null;
    await act(async () => {
      close?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(apiMocks.chatSessionSetOpen).toHaveBeenCalledWith(
      createdId,
      false,
      41,
      "/tmp/Current.opentake",
    );
    expect(useChatStore.getState().sessionId).toBe("chat-restored");
  });

  it("serializes a new-tab open before an immediate close", async () => {
    let resolveOpen: () => void = () => {};
    let resolveClose: () => void = () => {};
    apiMocks.chatSessionSetOpen.mockImplementation(
      (sessionId: string, isOpen: boolean) =>
        new Promise((resolve) => {
          const finish = () =>
            resolve({ id: sessionId, messages: [], createdAt: Date.now(), isOpen });
          if (isOpen) resolveOpen = finish;
          else resolveClose = finish;
        }),
    );
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });

    const newTab = container?.querySelector(
      'button[aria-label="agent.newTab"]',
    ) as HTMLButtonElement | null;
    await act(async () => {
      newTab?.click();
      await Promise.resolve();
    });
    const createdId = apiMocks.chatSessionSetOpen.mock.calls[0][0] as string;
    const selected = container?.querySelector(
      '[role="tab"][aria-selected="true"]',
    ) as HTMLButtonElement | null;
    const close = selected?.nextElementSibling as HTMLButtonElement | null;
    await act(async () => {
      close?.click();
      await Promise.resolve();
    });
    expect(apiMocks.chatSessionSetOpen.mock.calls).toEqual([
      [createdId, true, 41, "/tmp/Current.opentake"],
    ]);

    await act(async () => {
      resolveOpen();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(apiMocks.chatSessionSetOpen.mock.calls[1]).toEqual([
      createdId,
      false,
      41,
      "/tmp/Current.opentake",
    ]);

    await act(async () => {
      resolveClose();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(useChatStore.getState().sessionId).toBe("chat-restored");
    expect(
      Array.from(container?.querySelectorAll('[role="tab"]') ?? []).map((tab) =>
        tab.getAttribute("aria-label"),
      ),
    ).toEqual(["persisted"]);
  });

  it("serializes the automatic first-tab open before an immediate close", async () => {
    apiMocks.chatSessions.mockResolvedValueOnce([]);
    let resolveInitialOpen: () => void = () => {};
    let resolveInitialClose: () => void = () => {};
    let openCount = 0;
    apiMocks.chatSessionSetOpen.mockImplementation(
      (sessionId: string, isOpen: boolean) =>
        new Promise((resolve) => {
          const finish = () =>
            resolve({ id: sessionId, messages: [], createdAt: Date.now(), isOpen });
          if (isOpen && openCount++ === 0) resolveInitialOpen = finish;
          else if (!isOpen) resolveInitialClose = finish;
          else finish();
        }),
    );
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });

    const initialId = apiMocks.chatSessionSetOpen.mock.calls[0][0] as string;
    const selected = container?.querySelector(
      '[role="tab"][aria-selected="true"]',
    ) as HTMLButtonElement | null;
    const close = selected?.nextElementSibling as HTMLButtonElement | null;
    await act(async () => {
      close?.click();
      await Promise.resolve();
    });
    expect(apiMocks.chatSessionSetOpen.mock.calls).toEqual([
      [initialId, true, 41, "/tmp/Current.opentake"],
    ]);

    await act(async () => {
      resolveInitialOpen();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(apiMocks.chatSessionSetOpen.mock.calls[1]).toEqual([
      initialId,
      false,
      41,
      "/tmp/Current.opentake",
    ]);

    await act(async () => {
      resolveInitialClose();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });
    const replacementCall = apiMocks.chatSessionSetOpen.mock.calls[2];
    expect(replacementCall[1]).toBe(true);
    expect(replacementCall[0]).not.toBe(initialId);
    expect(useChatStore.getState().sessionId).toBe(replacementCall[0]);
  });

  it("keeps the session close-tab button on a 24px hit target", async () => {
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });
    const selected = container?.querySelector(
      '[role="tab"][aria-selected="true"]',
    ) as HTMLButtonElement | null;
    const close = selected?.nextElementSibling as HTMLButtonElement | null;
    expect(close).not.toBeNull();
    expect(close?.style.width).toBe("24px");
    expect(close?.style.height).toBe("24px");
  });
});
