// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const apiMocks = vi.hoisted(() => ({
  chatHistory: vi.fn(),
  chatHistoryAuthoritative: vi.fn(),
  chatSessionSetOpen: vi.fn(),
  chatSessions: vi.fn(),
  chatSend: vi.fn(),
  onChatDelta: vi.fn(),
  onChatToolCall: vi.fn(),
  onChatDone: vi.fn(),
  deltaHandler: undefined as undefined | ((event: Record<string, unknown>) => void),
  toolHandler: undefined as undefined | ((event: Record<string, unknown>) => void),
  doneHandler: undefined as undefined | ((event: Record<string, unknown>) => void),
  deltaMalformed: undefined as undefined | ((failure: Record<string, unknown>) => void),
  toolMalformed: undefined as undefined | ((failure: Record<string, unknown>) => void),
  doneMalformed: undefined as undefined | ((failure: Record<string, unknown>) => void),
  unDelta: vi.fn(),
  unTool: vi.fn(),
  unDone: vi.fn(),
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: string) => key,
}));

vi.mock("../../lib/api", () => ({
  isTauri: true,
  chatCancel: vi.fn(async () => {}),
  chatHistory: apiMocks.chatHistory,
  chatHistoryAuthoritative: apiMocks.chatHistoryAuthoritative,
  chatSend: apiMocks.chatSend,
  chatSessionSetOpen: apiMocks.chatSessionSetOpen,
  chatSessions: apiMocks.chatSessions,
  onChatDelta: apiMocks.onChatDelta,
  onChatToolCall: apiMocks.onChatToolCall,
  onChatDone: apiMocks.onChatDone,
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
  useChatStore.setState({
    sessionId: "stale-session",
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
  localStorage.clear();
  apiMocks.deltaHandler = undefined;
  apiMocks.toolHandler = undefined;
  apiMocks.doneHandler = undefined;
  apiMocks.deltaMalformed = undefined;
  apiMocks.toolMalformed = undefined;
  apiMocks.doneMalformed = undefined;
  apiMocks.unDelta.mockReset();
  apiMocks.unTool.mockReset();
  apiMocks.unDone.mockReset();
  apiMocks.onChatDelta.mockReset();
  apiMocks.onChatToolCall.mockReset();
  apiMocks.onChatDone.mockReset();
  apiMocks.onChatDelta.mockImplementation(async (
    handler: (event: Record<string, unknown>) => void,
    malformed: (failure: Record<string, unknown>) => void,
  ) => {
    apiMocks.deltaHandler = handler;
    apiMocks.deltaMalformed = malformed;
    return apiMocks.unDelta;
  });
  apiMocks.onChatToolCall.mockImplementation(async (
    handler: (event: Record<string, unknown>) => void,
    malformed: (failure: Record<string, unknown>) => void,
  ) => {
    apiMocks.toolHandler = handler;
    apiMocks.toolMalformed = malformed;
    return apiMocks.unTool;
  });
  apiMocks.onChatDone.mockImplementation(async (
    handler: (event: Record<string, unknown>) => void,
    malformed: (failure: Record<string, unknown>) => void,
  ) => {
    apiMocks.doneHandler = handler;
    apiMocks.doneMalformed = malformed;
    return apiMocks.unDone;
  });
  apiMocks.chatHistory.mockReset();
  apiMocks.chatHistoryAuthoritative.mockReset();
  apiMocks.chatSessionSetOpen.mockReset();
  apiMocks.chatSessions.mockReset();
  apiMocks.chatSend.mockReset();
  apiMocks.chatSend.mockResolvedValue(undefined);
  apiMocks.chatHistory.mockResolvedValue([]);
  apiMocks.chatHistoryAuthoritative.mockResolvedValue([]);
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
        messageId: "stale",
        sequence: 0,
        message: {
          id: "stale",
          role: "assistant",
          content: "stale",
          toolCalls: [],
          blocks: [{ type: "text", text: "stale" }],
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
        messageId: "stale-path",
        sequence: 0,
        message: {
          id: "stale-path",
          role: "assistant",
          content: "stale path",
          toolCalls: [],
          blocks: [{ type: "text", text: "stale path" }],
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
        messageId: "current",
        sequence: 0,
        message: {
          id: "current",
          role: "assistant",
          content: "current",
          toolCalls: [],
          blocks: [{ type: "text", text: "current" }],
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
      apiMocks.deltaHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-second",
        messageId: "assistant-background",
        sequence: 0,
        blockIndex: 0,
        delta: "Background edit",
      });
      apiMocks.doneHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-second",
        messageId: "assistant-background",
        sequence: 1,
        message: {
          id: "assistant-background",
          role: "assistant",
          content: "Background edit",
          toolCalls: [],
          blocks: [{ type: "text", text: "Background edit" }],
          createdAt: 4,
        },
      });
    });

    await act(async () => {
      (tabs[1] as HTMLButtonElement).click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(apiMocks.chatHistoryAuthoritative).not.toHaveBeenCalled();
    expect(useChatStore.getState().sessionId).toBe("chat-second");
    expect(useChatStore.getState().messages.map((message) => message.content)).toEqual([
      "Second edit",
      "Background edit",
    ]);
  });

  it("retains the selected session and composer across top-level panel navigation", async () => {
    apiMocks.chatSessions.mockResolvedValue([
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
    ]);
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });
    const secondTab = Array.from(container?.querySelectorAll('[role="tab"]') ?? [])[1] as
      | HTMLButtonElement
      | undefined;
    await act(async () => secondTab?.click());
    const textarea = container?.querySelector<HTMLTextAreaElement>("textarea")!;
    await act(async () => {
      const valueSetter = Object.getOwnPropertyDescriptor(
        HTMLTextAreaElement.prototype,
        "value",
      )?.set;
      valueSetter?.call(textarea, "keep this draft");
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
    });

    await act(async () => root?.unmount());
    root = createRoot(container!);
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(useChatStore.getState().sessionId).toBe("chat-second");
    expect(container?.querySelector<HTMLTextAreaElement>("textarea")?.value).toBe(
      "keep this draft",
    );
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
    const composer = container?.querySelector<HTMLTextAreaElement>("textarea");
    expect(composer?.getAttribute("aria-label")).toBe(
      "agent.inputPlaceholder",
    );
    expect(composer?.classList.contains("agent-composer__input")).toBe(true);
    expect(composer?.style.outline).not.toBe("none");
    expect(container?.querySelector('button[aria-label="agent.send"]')).not.toBeNull();
  });

  it("waits for an authoritative terminal history after a gap and installs it once", async () => {
    let resolveAuthoritative: (messages: Array<Record<string, unknown>>) => void = () => {};
    apiMocks.chatHistory.mockResolvedValueOnce([
      { id: "m1", role: "user", content: "persisted", toolCalls: [], createdAt: 1 },
    ]);
    apiMocks.chatHistoryAuthoritative.mockReturnValueOnce(new Promise((resolve) => {
      resolveAuthoritative = resolve;
    }));
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });

    await act(async () => {
      apiMocks.deltaHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-restored",
        messageId: "assistant-live",
        sequence: 0,
        blockIndex: 0,
        delta: "partial live reply",
      });
      apiMocks.toolHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-restored",
        messageId: "assistant-live",
        sequence: 2,
        blockIndex: 1,
        block: { type: "toolUse", id: "gap", name: "inspect_timeline", input: {} },
      });
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(apiMocks.chatHistory).not.toHaveBeenCalled();
    expect(apiMocks.chatHistoryAuthoritative).toHaveBeenCalledOnce();
    expect(useChatStore.getState().messages.at(-1)?.content).toBe("partial live reply");

    const terminal = {
      id: "assistant-live",
      role: "assistant",
      content: "final persisted reply",
      toolCalls: [],
      blocks: [{ type: "text", text: "final persisted reply" }],
      createdAt: 9,
    };
    await act(async () => {
      apiMocks.doneHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-restored",
        messageId: "assistant-live",
        sequence: 1,
        message: terminal,
      });
      resolveAuthoritative([
        { id: "m1", role: "user", content: "persisted", toolCalls: [], createdAt: 1 },
        terminal,
      ]);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(useChatStore.getState().messages.at(-1)).toEqual(terminal);
    expect(apiMocks.chatHistoryAuthoritative).toHaveBeenCalledOnce();
    expect(useChatStore.getState().takeHistoryResyncRequest()).toBeNull();
  });

  it("installs an in-flight authoritative snapshot after the panel unmounts", async () => {
    let resolveAuthoritative: (messages: Array<Record<string, unknown>>) => void = () => {};
    apiMocks.chatHistoryAuthoritative.mockReturnValueOnce(new Promise((resolve) => {
      resolveAuthoritative = resolve;
    }));
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });
    await act(async () => {
      apiMocks.deltaHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-restored",
        messageId: "assistant-gap",
        sequence: 0,
        blockIndex: 0,
        delta: "partial",
      });
      apiMocks.deltaHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-restored",
        messageId: "assistant-gap",
        sequence: 2,
        blockIndex: 0,
        delta: "gap",
      });
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(useChatStore.getState().resyncingSessionIds["chat-restored"]).toBe(true);
    expect(container?.querySelector<HTMLTextAreaElement>("textarea")?.disabled).toBe(true);
    expect(apiMocks.chatHistoryAuthoritative).toHaveBeenCalledOnce();

    await act(async () => root?.unmount());
    root = null;
    const authoritative = [{
      id: "final-after-unmount",
      role: "assistant",
      content: "terminal snapshot",
      toolCalls: [],
      blocks: [{ type: "text", text: "terminal snapshot" }],
      createdAt: 9,
    }];
    await act(async () => {
      resolveAuthoritative(authoritative);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(useChatStore.getState().sessionMessages["chat-restored"]).toEqual(authoritative);
    expect(useChatStore.getState().resyncingSessionIds["chat-restored"]).toBeUndefined();
  });

  it("retries a rejected authoritative request after unmount and remount without spinning", async () => {
    vi.useFakeTimers();
    try {
      const authoritative = [{
        id: "final-after-retry",
        role: "assistant",
        content: "terminal retry snapshot",
        toolCalls: [],
        blocks: [{ type: "text", text: "terminal retry snapshot" }],
        createdAt: 10,
      }];
      apiMocks.chatHistoryAuthoritative
        .mockRejectedValueOnce(new Error("authoritative history unavailable"))
        .mockResolvedValueOnce(authoritative);
      await act(async () => {
        root?.render(<AgentPanel />);
        await Promise.resolve();
        await Promise.resolve();
      });
      await act(async () => {
        apiMocks.deltaHandler?.({
          projectEpoch: 41,
          projectPath: "/tmp/Current.opentake",
          sessionId: "chat-restored",
          messageId: "assistant-gap",
          sequence: 0,
          blockIndex: 0,
          delta: "partial",
        });
        apiMocks.deltaHandler?.({
          projectEpoch: 41,
          projectPath: "/tmp/Current.opentake",
          sessionId: "chat-restored",
          messageId: "assistant-gap",
          sequence: 2,
          blockIndex: 0,
          delta: "gap",
        });
        await Promise.resolve();
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(apiMocks.chatHistoryAuthoritative).toHaveBeenCalledOnce();
      expect(useChatStore.getState().resyncingSessionIds["chat-restored"]).toBe(true);
      expect(container?.querySelector<HTMLTextAreaElement>("textarea")?.disabled).toBe(true);

      await act(async () => root?.unmount());
      root = null;
      apiMocks.chatSessions.mockReturnValueOnce(new Promise(() => {}));
      root = createRoot(container!);
      await act(async () => {
        root?.render(<AgentPanel />);
        await Promise.resolve();
        await Promise.resolve();
      });
      expect(
        apiMocks.chatHistoryAuthoritative,
        "a permanent failure must not cause an immediate retry loop",
      ).toHaveBeenCalledOnce();

      await act(async () => {
        await vi.advanceTimersByTimeAsync(5_000);
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(apiMocks.chatHistoryAuthoritative).toHaveBeenCalledTimes(2);
      expect(useChatStore.getState().sessionMessages["chat-restored"]).toEqual(authoritative);
      expect(useChatStore.getState().resyncingSessionIds["chat-restored"]).toBeUndefined();
    } finally {
      vi.useRealTimers();
    }
  });

  it("requeues a CAS-rejected re-sync and resumes it after remount", async () => {
    let resolveStale: (messages: Array<Record<string, unknown>>) => void = () => {};
    apiMocks.chatHistoryAuthoritative.mockReturnValueOnce(new Promise((resolve) => {
      resolveStale = resolve;
    }));
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });
    await act(async () => {
      apiMocks.deltaHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-restored",
        messageId: "assistant-gap",
        sequence: 0,
        blockIndex: 0,
        delta: "partial",
      });
      apiMocks.deltaHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-restored",
        messageId: "assistant-gap",
        sequence: 2,
        blockIndex: 0,
        delta: "gap",
      });
      await Promise.resolve();
      await Promise.resolve();
    });
    await act(async () => root?.unmount());
    root = null;
    useChatStore.getState().beginMessage("chat-restored", "assistant-newer");
    await act(async () => {
      resolveStale([{
        id: "stale",
        role: "assistant",
        content: "stale snapshot",
        toolCalls: [],
        blocks: [{ type: "text", text: "stale snapshot" }],
        createdAt: 8,
      }]);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(useChatStore.getState().historyResyncRequests["chat-restored"]).toBeDefined();

    const final = [{
      id: "authoritative-newer",
      role: "assistant",
      content: "new terminal snapshot",
      toolCalls: [],
      blocks: [{ type: "text", text: "new terminal snapshot" }],
      createdAt: 10,
    }];
    apiMocks.chatHistoryAuthoritative.mockResolvedValueOnce(final);
    apiMocks.chatSessions.mockReturnValueOnce(new Promise(() => {}));
    root = createRoot(container!);
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(apiMocks.chatHistoryAuthoritative).toHaveBeenCalledTimes(2);
    expect(useChatStore.getState().sessionMessages["chat-restored"]).toEqual(final);
    expect(useChatStore.getState().resyncingSessionIds["chat-restored"]).toBeUndefined();
  });

  it("does not let a late startup snapshot overwrite a touched inactive session", async () => {
    let resolveSessions: (sessions: Array<Record<string, unknown>>) => void = () => {};
    apiMocks.chatSessions.mockReturnValueOnce(new Promise((resolve) => {
      resolveSessions = resolve;
    }));
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
    });

    await act(async () => {
      apiMocks.deltaHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-second",
        messageId: "assistant-new",
        sequence: 0,
        blockIndex: 0,
        delta: "new inactive reply",
      });
      resolveSessions([
        {
          id: "chat-restored",
          messages: [{ id: "m1", role: "user", content: "persisted", toolCalls: [], createdAt: 1 }],
          createdAt: 1,
          isOpen: true,
        },
        {
          id: "chat-second",
          messages: [{ id: "old", role: "assistant", content: "old snapshot", toolCalls: [], createdAt: 1 }],
          createdAt: 2,
          isOpen: true,
        },
      ]);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(useChatStore.getState().sessionMessages["chat-second"].at(-1)?.content).toBe(
      "new inactive reply",
    );
    const secondTab = Array.from(container?.querySelectorAll('[role="tab"]') ?? [])[1] as
      | HTMLButtonElement
      | undefined;
    await act(async () => secondTab?.click());
    expect(useChatStore.getState().messages.at(-1)?.content).toBe("new inactive reply");
  });

  it("atomically clears prior project chat state before a same-id replacement snapshot arrives", async () => {
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(useChatStore.getState().sessionMessages["chat-restored"]).toBeDefined();

    let resolveReplacement: (sessions: Array<Record<string, unknown>>) => void = () => {};
    apiMocks.chatSessions.mockReturnValueOnce(new Promise((resolve) => {
      resolveReplacement = resolve;
    }));
    await act(async () => {
      useProjectStore.setState({
        projectEpoch: 42,
        projectPath: "/tmp/Replacement.opentake",
      });
      await Promise.resolve();
    });

    expect(useChatStore.getState().sessionMessages).toEqual({});
    await act(async () => {
      resolveReplacement([{
        id: "chat-restored",
        messages: [{ id: "b1", role: "user", content: "project B", toolCalls: [], createdAt: 1 }],
        createdAt: 1,
        isOpen: true,
      }]);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(useChatStore.getState().messages.map((message) => message.content)).toEqual([
      "project B",
    ]);
  });

  it("applies server identities and sequences, then reloads malformed or gapped history once", async () => {
    let resolveHistory: (messages: Array<Record<string, unknown>>) => void = () => {};
    apiMocks.chatHistoryAuthoritative.mockReturnValueOnce(new Promise((resolve) => {
      resolveHistory = resolve;
    }));
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });

    await act(async () => {
      apiMocks.deltaHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-restored",
        messageId: "assistant-server",
        sequence: 0,
        blockIndex: 0,
        delta: "Text A",
      });
      apiMocks.toolHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-restored",
        messageId: "assistant-server",
        sequence: 2,
        blockIndex: 1,
        block: {
          type: "toolUse",
          id: "tool-gap",
          name: "inspect_timeline",
          input: {},
        },
      });
      apiMocks.toolHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-restored",
        messageId: "assistant-server",
        sequence: 2,
        blockIndex: 1,
        block: {
          type: "toolUse",
          id: "tool-gap",
          name: "inspect_timeline",
          input: {},
        },
      });
      await Promise.resolve();
    });

    expect(apiMocks.chatHistoryAuthoritative).toHaveBeenCalledTimes(1);
    expect(apiMocks.chatHistoryAuthoritative).toHaveBeenCalledWith(
      "chat-restored",
      41,
      "/tmp/Current.opentake",
    );
    expect(useChatStore.getState().streaming).toBe(false);

    await act(async () => {
      apiMocks.deltaMalformed?.({
        eventName: "chat_delta",
        reason: "invalid_block",
        sessionId: "chat-restored",
        messageId: "assistant-server",
      });
      await Promise.resolve();
    });
    expect(apiMocks.chatHistoryAuthoritative).toHaveBeenCalledTimes(1);

    const authoritative = [
      { id: "m1", role: "user", content: "persisted", toolCalls: [], createdAt: 1 },
      {
        id: "assistant-server",
        role: "assistant",
        content: "Authoritative reply",
        toolCalls: [],
        blocks: [{ type: "text", text: "Authoritative reply" }],
        createdAt: 5,
      },
    ];
    await act(async () => {
      resolveHistory(authoritative);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(useChatStore.getState().messages).toEqual(authoritative);
  });

  it("finalizes a sequenced tool-result message without leaving an assistant draft streaming", async () => {
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });
    const toolMessage = {
      id: "tool-message",
      role: "tool",
      content: "{\"ok\":true}",
      toolCalls: [],
      blocks: [{
        type: "toolResult",
        toolUseId: "tool-1",
        content: [{ kind: "text", text: "ok" }],
      }],
      createdAt: 5,
      toolCallId: "tool-1",
    };

    await act(async () => {
      apiMocks.toolHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-restored",
        messageId: "tool-message",
        sequence: 0,
        blockIndex: 0,
        block: toolMessage.blocks[0],
      });
      apiMocks.doneHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-restored",
        messageId: "tool-message",
        sequence: 1,
        message: toolMessage,
      });
      await Promise.resolve();
    });

    expect(useChatStore.getState().messages.at(-1)).toEqual(toolMessage);
    expect(useChatStore.getState().streaming).toBe(false);
    expect(container?.querySelector('[data-agent-block-type="toolResult"]')).not.toBeNull();
  });

  it("releases a pending composer when the first server event is malformed and history is replaced", async () => {
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
      valueSetter?.call(textarea, "pending request");
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      textarea.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
      await Promise.resolve();
    });
    expect(textarea.disabled).toBe(true);
    expect(container?.querySelector('button[aria-label="agent.cancel"]')).not.toBeNull();

    await act(async () => {
      apiMocks.deltaMalformed?.({
        eventName: "chat_delta",
        reason: "invalid_block",
        sessionId: "chat-restored",
        messageId: "assistant-malformed",
      });
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(apiMocks.chatHistoryAuthoritative).toHaveBeenCalledOnce();
    expect(textarea.disabled).toBe(false);
  });

  it("never reloads an old session against a replacement project after a sequence gap", async () => {
    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });

    await act(async () => {
      apiMocks.deltaHandler?.({
        projectEpoch: 41,
        projectPath: "/tmp/Current.opentake",
        sessionId: "chat-restored",
        messageId: "assistant-old-project",
        sequence: 1,
        blockIndex: 0,
        delta: "gap",
      });
      useProjectStore.setState({
        projectEpoch: 42,
        projectPath: "/tmp/Replacement.opentake",
      });
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(apiMocks.chatHistoryAuthoritative).not.toHaveBeenCalled();
  });

  it("removes the Agent-local Chat/Motion switch without consulting a stored mode", async () => {
    const getItem = vi.spyOn(Storage.prototype, "getItem");
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    localStorage.setItem("opentake.agent.mode", "motion");
    getItem.mockClear();
    setItem.mockClear();

    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container?.querySelector('[aria-label="agent.modes"]')).toBeNull();
    expect(container?.textContent).not.toContain("agent.motionMode");
    expect(getItem.mock.calls.some(([key]) => String(key).includes("agent.mode"))).toBe(false);
    expect(setItem.mock.calls.some(([key]) => String(key).includes("agent.mode"))).toBe(false);
    getItem.mockRestore();
    setItem.mockRestore();
  });

  it("disposes listeners that resolve after the panel has already unmounted", async () => {
    let resolveDelta: (unsubscribe: () => void) => void = () => {};
    let resolveTool: (unsubscribe: () => void) => void = () => {};
    let resolveDone: (unsubscribe: () => void) => void = () => {};
    apiMocks.onChatDelta.mockReturnValueOnce(new Promise((resolve) => { resolveDelta = resolve; }));
    apiMocks.onChatToolCall.mockReturnValueOnce(new Promise((resolve) => { resolveTool = resolve; }));
    apiMocks.onChatDone.mockReturnValueOnce(new Promise((resolve) => { resolveDone = resolve; }));

    await act(async () => {
      root?.render(<AgentPanel />);
      await Promise.resolve();
    });
    await act(async () => root?.unmount());
    root = null;
    await act(async () => {
      resolveDelta(apiMocks.unDelta);
      resolveTool(apiMocks.unTool);
      resolveDone(apiMocks.unDone);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(apiMocks.unDelta).toHaveBeenCalledOnce();
    expect(apiMocks.unTool).toHaveBeenCalledOnce();
    expect(apiMocks.unDone).toHaveBeenCalledOnce();
  });
});
