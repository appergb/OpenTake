// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  MAX_CHAT_IMAGE_BASE64_CHARS,
  type ChatMessage,
} from "../../lib/types";
import { AssistantTurn, ConversationMessage } from "./AgentPanel";

vi.mock("../../i18n", () => ({
  useT: () => (key: string, values?: Record<string, string>) =>
    values ? `${key}:${Object.values(values).join(":")}` : key,
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: query === "(prefers-reduced-motion: reduce)",
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
  vi.unstubAllGlobals();
});

function assistant(overrides: Partial<ChatMessage> = {}): ChatMessage {
  return {
    id: "assistant-ordered",
    role: "assistant",
    content: "legacy content must not render",
    toolCalls: [
      { id: "legacy-tool", name: "legacy_tool", args: { ignored: true } },
    ],
    blocks: [
      { type: "text", text: "Text A" },
      {
        type: "toolUse",
        id: "tool-1",
        name: "inspect_timeline",
        input: { frame: 42 },
      },
      {
        type: "toolResult",
        toolUseId: "tool-1",
        content: [
          { kind: "image", mediaType: "image/png", base64: "iVBORw0KGgo=" },
          { kind: "text", text: "Timeline inspected" },
        ],
      },
      { type: "text", text: "Text B" },
    ],
    createdAt: 1,
    ...overrides,
  };
}

async function render(message: ChatMessage) {
  await act(async () => root.render(<AssistantTurn message={message} />));
}

describe("AssistantTurn", () => {
  it("renders authoritative blocks in exact order without assistant bubble or tool-card chrome", async () => {
    await render(assistant());

    const turn = container.querySelector<HTMLElement>("[data-assistant-turn]");
    expect(turn).not.toBeNull();
    expect(
      Array.from(turn?.querySelectorAll<HTMLElement>("[data-agent-block-index]") ?? []).map(
        (block) => [block.dataset.agentBlockIndex, block.dataset.agentBlockType],
      ),
    ).toEqual([
      ["0", "text"],
      ["1", "toolUse"],
      ["2", "toolResult"],
      ["3", "text"],
    ]);
    expect(turn?.textContent).toContain("Text A");
    expect(turn?.textContent).toContain("Text B");
    expect(turn?.textContent).not.toContain("legacy content must not render");
    expect(turn?.textContent).not.toContain("legacy_tool");
    expect(turn?.classList.contains("agent-message__bubble")).toBe(false);
    expect(container.querySelector(".agent-tool-card")).toBeNull();
    expect(
      Array.from(container.querySelectorAll<HTMLElement>("[data-tool-activity]")).every(
        (activity) => activity.style.border === "",
      ),
    ).toBe(true);
  });

  it("exposes arguments, result text, and a bounded raster image through accessible disclosures", async () => {
    await render(assistant());
    const triggers = Array.from(
      container.querySelectorAll<HTMLButtonElement>("[data-tool-activity-trigger]"),
    );
    expect(triggers).toHaveLength(2);
    expect(triggers[0].getAttribute("aria-expanded")).toBe("false");
    expect(triggers[0].getAttribute("aria-controls")).toBeTruthy();
    expect(document.getElementById(triggers[0].getAttribute("aria-describedby")!)?.textContent)
      .toBe("agent.toolRunning");

    await act(async () => triggers[0].click());
    expect(triggers[0].getAttribute("aria-expanded")).toBe("true");
    const argsRegion = document.getElementById(triggers[0].getAttribute("aria-controls")!);
    expect(argsRegion?.textContent).toContain('"frame": 42');

    await act(async () => triggers[1].click());
    const resultRegion = document.getElementById(triggers[1].getAttribute("aria-controls")!);
    expect(resultRegion?.textContent).toContain("Timeline inspected");
    const image = resultRegion?.querySelector("img");
    expect(image?.getAttribute("src")).toBe("data:image/png;base64,iVBORw0KGgo=");
    expect(image?.getAttribute("alt")).toBe("agent.toolImageAlt:inspect_timeline");
    expect(image?.classList.contains("agent-tool-activity__image")).toBe(true);
  });

  it("announces failed tool activity and reveals its error result", async () => {
    await render(assistant({
      content: "",
      toolCalls: [],
      blocks: [{
        type: "toolUse",
        id: "tool-error",
        name: "apply_edit",
        input: { clipId: "clip-1" },
        result: { error: "timeline locked" },
        isError: true,
      }],
    }));

    const activity = container.querySelector<HTMLElement>("[data-tool-activity]");
    const trigger = activity?.querySelector<HTMLButtonElement>("button");
    expect(activity?.dataset.status).toBe("error");
    expect(document.getElementById(trigger?.getAttribute("aria-describedby") ?? "")?.textContent)
      .toBe("agent.toolFailed");
    await act(async () => trigger?.click());
    expect(container.textContent).toContain("timeline locked");
  });

  it("rejects active-content and malformed image data instead of constructing a data URI", async () => {
    await render(assistant({
      content: "",
      toolCalls: [],
      blocks: [{
        type: "toolResult",
        toolUseId: "unsafe-tool",
        content: [
          { kind: "image", mediaType: "image/svg+xml", base64: "PHN2Zz48L3N2Zz4=" },
          { kind: "image", mediaType: "image/png", base64: "not base64<script>" },
        ],
      }],
    }));

    const trigger = container.querySelector<HTMLButtonElement>("button");
    await act(async () => trigger?.click());
    expect(container.querySelector("img")).toBeNull();
    expect(container.textContent?.match(/agent\.toolImageUnavailable/g)).toHaveLength(2);
    expect(container.innerHTML).not.toContain("data:image/svg+xml");
  });

  it("opens and closes disclosure details immediately when reduced motion is requested", async () => {
    await render(assistant());
    const trigger = container.querySelector<HTMLButtonElement>("[data-tool-activity-trigger]")!;
    trigger.focus();
    expect(document.activeElement).toBe(trigger);

    await act(async () => trigger.click());
    const detail = document.getElementById(trigger.getAttribute("aria-controls")!);
    expect(detail?.classList.contains("reveal")).toBe(true);
    expect(detail?.getAttribute("data-state")).toBe("open");

    await act(async () => trigger.click());
    expect(document.getElementById(trigger.getAttribute("aria-controls")!)).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it("keeps live status outside the disclosure button and closes on Escape without bubbling", async () => {
    const parentKeyDown = vi.fn();
    await act(async () => root.render(
      <div onKeyDown={parentKeyDown}>
        <AssistantTurn message={assistant()} />
      </div>,
    ));
    const trigger = container.querySelector<HTMLButtonElement>("[data-tool-activity-trigger]")!;
    const statusId = trigger.getAttribute("aria-describedby");

    expect(statusId).toBeTruthy();
    expect(trigger.querySelector('[role="status"]')).toBeNull();
    expect(document.getElementById(statusId!)?.getAttribute("role")).toBe("status");
    await act(async () => trigger.click());
    trigger.focus();
    await act(async () => {
      trigger.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    });

    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    expect(document.activeElement).toBe(trigger);
    expect(parentKeyDown).not.toHaveBeenCalled();
  });

  it("rejects raster image payloads above the shared chat image ceiling", async () => {
    await render(assistant({
      content: "",
      toolCalls: [],
      blocks: [{
        type: "toolResult",
        toolUseId: "oversized-tool",
        content: [{
          kind: "image",
          mediaType: "image/png",
          base64: "A".repeat(MAX_CHAT_IMAGE_BASE64_CHARS + 4),
        }],
      }],
    }));

    const trigger = container.querySelector<HTMLButtonElement>("button")!;
    await act(async () => trigger.click());
    expect(container.querySelector("img")).toBeNull();
    expect(container.textContent).toContain("agent.toolImageUnavailable");
  });

  it("renders native tool-message blocks instead of their flattened compatibility content", async () => {
    const message: ChatMessage = {
      id: "tool-message",
      role: "tool",
      content: "flattened compatibility result",
      toolCalls: [],
      blocks: [{
        type: "toolResult",
        toolUseId: "tool-1",
        content: [{ kind: "image", mediaType: "image/png", base64: "iVBORw0KGgo=" }],
      }],
      createdAt: 2,
      toolCallId: "tool-1",
    };
    await act(async () => root.render(
      <ConversationMessage message={message} onOpenSettings={vi.fn()} />,
    ));

    expect(container.textContent).not.toContain("flattened compatibility result");
    expect(container.querySelector('[data-agent-block-type="toolResult"]')).not.toBeNull();
    const trigger = container.querySelector<HTMLButtonElement>("[data-tool-activity-trigger]")!;
    await act(async () => trigger.click());
    expect(container.querySelector("img")?.getAttribute("src")).toBe(
      "data:image/png;base64,iVBORw0KGgo=",
    );
  });

  it("keeps an assistant tool round, native tool result, and follow-up text in one turn", async () => {
    const messages: ChatMessage[] = [
      {
        id: "assistant-tool-use",
        role: "assistant",
        content: "",
        toolCalls: [{ id: "tool-1", name: "inspect_timeline", args: {} }],
        blocks: [{
          type: "toolUse",
          id: "tool-1",
          name: "inspect_timeline",
          input: {},
        }],
        createdAt: 1,
      },
      {
        id: "tool-result",
        role: "tool",
        content: "flattened result",
        toolCalls: [],
        blocks: [{
          type: "toolResult",
          toolUseId: "tool-1",
          content: [{ kind: "text", text: "Inspected 3 tracks" }],
        }],
        createdAt: 2,
        toolCallId: "tool-1",
      },
      {
        id: "assistant-follow-up",
        role: "assistant",
        content: "Finished",
        toolCalls: [],
        blocks: [{ type: "text", text: "Finished" }],
        createdAt: 3,
      },
    ];
    await act(async () => root.render(<AssistantTurn messages={messages} />));

    expect(container.querySelectorAll("[data-assistant-turn]")).toHaveLength(1);
    expect(
      Array.from(container.querySelectorAll<HTMLElement>("[data-agent-block-index]")).map(
        (block) => [block.dataset.agentBlockIndex, block.dataset.agentBlockType],
      ),
    ).toEqual([
      ["0", "toolUse"],
      ["1", "toolResult"],
      ["2", "text"],
    ]);
    expect(container.textContent).not.toContain("flattened result");
    expect(container.textContent).toContain("Finished");
  });

  it("uses authoritative user text blocks on a quiet user surface", async () => {
    const message: ChatMessage = {
      id: "user-blocks",
      role: "user",
      content: "legacy user text",
      toolCalls: [],
      blocks: [{ type: "text", text: "authoritative user text" }],
      createdAt: 4,
    };
    await act(async () => root.render(
      <ConversationMessage message={message} onOpenSettings={vi.fn()} />,
    ));

    const surface = container.querySelector(".agent-message__user-surface");
    expect(surface?.textContent).toBe("authoritative user text");
    expect(surface?.classList.contains("agent-message__bubble")).toBe(false);
    expect(surface?.getAttribute("style")).toBeNull();
  });
});
