// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ExternalMcpStatus } from "../../lib/types";

const api = vi.hoisted(() => ({
  status: vi.fn(),
  setEnabled: vi.fn(),
  pair: vi.fn(),
  regenerate: vi.fn(),
  revoke: vi.fn(),
  subscribe: vi.fn(),
  unlisten: vi.fn(),
}));

vi.mock("../../lib/api", () => ({
  externalMcpStatus: api.status,
  externalMcpSetEnabled: api.setEnabled,
  externalMcpPair: api.pair,
  externalMcpRegenerate: api.regenerate,
  externalMcpRevoke: api.revoke,
  onExternalMcpStatusChanged: api.subscribe,
}));

import { t } from "../../i18n";
import { ExternalMcpPane } from "./ExternalMcpPane";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

const endpoint = "http://127.0.0.1:19789/mcp";

function client(name = "Cursor") {
  return {
    id: "client-1",
    name,
    tokenDigest: "abc123def456",
    createdAt: 1_700_000_000,
    lastUsedAt: null,
    revokedAt: null,
  };
}

function status(overrides: Partial<ExternalMcpStatus> = {}): ExternalMcpStatus {
  return {
    revision: 1,
    enabled: false,
    state: "disabled",
    endpoint,
    clients: [],
    error: null,
    ...overrides,
  };
}

function receipt(name = "Cursor", bearerToken = oneTimeToken) {
  return {
    client: client(name),
    endpoint,
    bearerToken,
  };
}

let latestStatus = status();
let emitStatus: ((next: ExternalMcpStatus) => void) | null = null;
let oneTimeToken = "";
let container: HTMLDivElement;
let root: Root;

function button(label: string) {
  return [...container.querySelectorAll<HTMLButtonElement>("button")].find(
    (candidate) => candidate.textContent?.trim() === label,
  )!;
}

async function setClientName(name: string) {
  const input = container.querySelector<HTMLInputElement>("input[name='external-mcp-client-name']")!;
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
  if (!setter) throw new Error("missing native input value setter");
  await act(async () => {
    setter.call(input, name);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

async function render(next = latestStatus) {
  latestStatus = next;
  api.subscribe.mockImplementation(async (handler: (value: ExternalMcpStatus) => void) => {
    emitStatus = handler;
    handler(latestStatus);
    return api.unlisten;
  });
  await act(async () => root.render(<ExternalMcpPane />));
  await act(async () => undefined);
}

beforeEach(() => {
  vi.clearAllMocks();
  latestStatus = status();
  emitStatus = null;
  oneTimeToken = `test-bearer-${crypto.randomUUID()}`;
  api.status.mockImplementation(async () => latestStatus);
  api.setEnabled.mockImplementation(async (enabled: boolean) => {
    latestStatus = status({ revision: 2, enabled, state: enabled ? "listening" : "disabled" });
    return latestStatus;
  });
  api.pair.mockResolvedValue(receipt());
  api.regenerate.mockResolvedValue(receipt());
  api.revoke.mockImplementation(async () => status({ revision: 2 }));
  vi.stubGlobal("navigator", {
    clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
  vi.unstubAllGlobals();
});

describe("ExternalMcpPane", () => {
  it.each([
    ["disabled", false, t("mcp.status.disabled")],
    ["listening", true, t("mcp.status.listening")],
    ["portConflict", true, t("mcp.status.portConflict")],
    ["authFailure", true, t("mcp.status.authFailure")],
  ] as const)("renders the authoritative %s listener state", async (state, enabled, label) => {
    await render(status({ enabled, state }));

    const row = container.querySelector<HTMLElement>(`[data-external-mcp-status='${state}']`);
    const toggle = container.querySelector<HTMLInputElement>("input[role='switch']")!;
    expect(row?.textContent).toContain(label);
    expect(toggle.checked).toBe(state === "listening");
    expect(container.textContent).toContain(endpoint);
  });

  it("keeps the switch off and surfaces an error when enable is rejected", async () => {
    api.setEnabled.mockRejectedValueOnce(new Error("port is busy"));
    await render();

    const toggle = container.querySelector<HTMLInputElement>("input[role='switch']")!;
    await act(async () => toggle.click());

    expect(toggle.checked).toBe(false);
    expect(container.querySelector("[role='alert']")?.textContent).toContain("port is busy");
  });

  it("retries enable from an authoritative port-conflict state instead of disabling the preference", async () => {
    await render(status({ enabled: true, state: "portConflict" }));

    const toggle = container.querySelector<HTMLInputElement>("input[role='switch']")!;
    await act(async () => toggle.click());

    expect(api.setEnabled).toHaveBeenCalledWith(true);
  });

  it("subscribes before status delivery and disposes the listener when unmounted", async () => {
    await render();

    expect(api.subscribe).toHaveBeenCalledOnce();
    expect(container.querySelector("[data-external-mcp-status='disabled']")).not.toBeNull();
    await act(async () => root.unmount());
    expect(api.unlisten).toHaveBeenCalledOnce();
  });

  it("does not let an older status command overwrite a newer listener state", async () => {
    let resolveStatus!: (value: ExternalMcpStatus) => void;
    api.status.mockReturnValueOnce(new Promise<ExternalMcpStatus>((resolve) => {
      resolveStatus = resolve;
    }));
    await render(status({ revision: 1, enabled: false, state: "disabled" }));

    await act(async () => {
      emitStatus?.(status({ revision: 3, enabled: true, state: "listening" }));
      resolveStatus(status({ revision: 2, enabled: false, state: "disabled" }));
    });

    expect(container.querySelector("[data-external-mcp-status='listening']")).not.toBeNull();
  });

  it("reveals a newly paired bearer once and removes it from the DOM when dismissed", async () => {
    await render();
    await setClientName("Claude Desktop");
    await act(async () => button(t("mcp.pair")).click());

    expect(api.pair).toHaveBeenCalledWith("Claude Desktop");
    expect(container.textContent).toContain(oneTimeToken);

    await act(async () => button(t("mcp.tokenDismiss")).click());
    expect(container.textContent).not.toContain(oneTimeToken);
  });

  it("copies the streamable endpoint and Authorization header from the one-time receipt", async () => {
    await render();
    await setClientName("Claude Desktop");
    await act(async () => button(t("mcp.pair")).click());
    await act(async () => button(t("mcp.copyConfig")).click());

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(expect.stringContaining(endpoint));
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
      expect.stringContaining(`"Authorization": "Bearer ${oneTimeToken}"`),
    );
  });

  it("reports clipboard failures without hiding the one-time receipt", async () => {
    (navigator.clipboard.writeText as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error("clipboard denied"),
    );
    await render();
    await setClientName("Cursor");
    await act(async () => button(t("mcp.pair")).click());
    await act(async () => button(t("mcp.copyConfig")).click());

    expect(container.querySelector("[role='alert']")?.textContent).toContain("clipboard denied");
    expect(container.textContent).toContain(oneTimeToken);
  });

  it("requires confirmation before regenerating or revoking a client", async () => {
    await render(status({ enabled: true, state: "listening", clients: [client()] }));

    await act(async () => button(t("mcp.regenerate")).click());
    expect(api.regenerate).not.toHaveBeenCalled();
    await act(async () => button(t("mcp.confirmRegenerate")).click());
    expect(api.regenerate).toHaveBeenCalledWith("client-1");

    await act(async () => button(t("mcp.tokenDismiss")).click());
    await act(async () => button(t("mcp.revoke")).click());
    expect(api.revoke).not.toHaveBeenCalled();
    await act(async () => button(t("mcp.confirmRevoke")).click());
    expect(api.revoke).toHaveBeenCalledWith("client-1");
  });

  it("focuses an inline confirmation when it opens", async () => {
    await render(status({ enabled: true, state: "listening", clients: [client()] }));
    const regenerate = button(t("mcp.regenerate"));
    await act(async () => regenerate.click());

    const confirm = button(t("mcp.confirmRegenerate"));
    expect(document.activeElement).toBe(confirm);
  });

  it("returns Escape focus to the confirmation trigger", async () => {
    await render(status({ enabled: true, state: "listening", clients: [client()] }));
    const regenerate = button(t("mcp.regenerate"));
    await act(async () => regenerate.click());

    const confirm = button(t("mcp.confirmRegenerate"));
    await act(async () => confirm.dispatchEvent(new KeyboardEvent("keydown", {
      key: "Escape",
      bubbles: true,
      cancelable: true,
    })));

    expect(container.querySelector(`[aria-controls='external-mcp-confirm-client-1'][aria-expanded='true']`)).toBeNull();
    expect(document.activeElement).toBe(regenerate);
  });

  it("returns cancel focus to the destructive trigger that opened confirmation", async () => {
    await render(status({ enabled: true, state: "listening", clients: [client()] }));
    const revoke = button(t("mcp.revoke"));
    await act(async () => revoke.click());
    await act(async () => button(t("mcp.cancel")).click());

    expect(document.activeElement).toBe(revoke);
  });

  it("drops a bearer from the DOM on navigation and does not persist it in browser storage", async () => {
    await render();
    await setClientName("Cursor");
    await act(async () => button(t("mcp.pair")).click());
    expect(container.textContent).toContain(oneTimeToken);

    await act(async () => root.unmount());
    expect(container.textContent).not.toContain(oneTimeToken);
    expect(localStorage.getItem("external-mcp")).toBeNull();
  });
});
