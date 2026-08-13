// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const codex = vi.hoisted(() => ({
  status: vi.fn(),
  login: vi.fn(),
  cancel: vi.fn(),
  logout: vi.fn(),
}));

const mcp = vi.hoisted(() => ({
  status: vi.fn(),
  setEnabled: vi.fn(),
  pair: vi.fn(),
  regenerate: vi.fn(),
  revoke: vi.fn(),
  subscribe: vi.fn(),
  unlisten: vi.fn(),
}));

vi.mock("../../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/api")>()),
  codexAuthStatus: codex.status,
  codexLoginStart: codex.login,
  codexLoginCancel: codex.cancel,
  codexLogout: codex.logout,
  externalMcpStatus: mcp.status,
  externalMcpSetEnabled: mcp.setEnabled,
  externalMcpPair: mcp.pair,
  externalMcpRegenerate: mcp.regenerate,
  externalMcpRevoke: mcp.revoke,
  onExternalMcpStatusChanged: mcp.subscribe,
}));

import { useEditorUiStore } from "../../store/uiStore";
import { useSettingsStore } from "../../store/settingsStore";
import { t } from "../../i18n";
import { SettingsView } from "./SettingsView";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let root: Root;

const mcpEndpoint = "http://127.0.0.1:19789/mcp";

function mcpClient() {
  return {
    id: "client-1",
    name: "Cursor",
    tokenDigest: "abc123def456",
    createdAt: 1_700_000_000,
    lastUsedAt: null,
    revokedAt: null,
  };
}

function mcpStatus(clients: ReturnType<typeof mcpClient>[] = []) {
  return {
    revision: 1,
    enabled: clients.length > 0,
    state: clients.length > 0 ? "listening" as const : "disabled" as const,
    endpoint: mcpEndpoint,
    clients,
    error: null,
  };
}

function mcpReceipt(token: string) {
  return { client: mcpClient(), endpoint: mcpEndpoint, bearerToken: token };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

async function setInput(element: HTMLInputElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
  if (!setter) throw new Error("missing native input value setter");
  await act(async () => {
    setter.call(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

function button(label: string) {
  return [...container.querySelectorAll<HTMLButtonElement>("button")].find(
    (candidate) => candidate.textContent?.trim() === label,
  )!;
}

function Harness() {
  const open = useEditorUiStore((state) => state.settingsOpen);
  const setOpen = useEditorUiStore((state) => state.setSettingsOpen);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        Open settings
      </button>
      {open && <SettingsView />}
    </>
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  const initialMcpStatus = mcpStatus();
  mcp.status.mockResolvedValue(initialMcpStatus);
  mcp.subscribe.mockImplementation(async (handler: (status: typeof initialMcpStatus) => void) => {
    handler(initialMcpStatus);
    return mcp.unlisten;
  });
  useEditorUiStore.setState({ settingsOpen: false, settingsPane: "general" });
  useSettingsStore.setState({ byokProvider: "anthropic", windowSize: "standard" });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
});

it("keeps Settings modal, keyboard-contained, escapable, and focus-restoring", async () => {
  await act(async () => root.render(<Harness />));
  const trigger = container.querySelector<HTMLButtonElement>("button")!;
  trigger.focus();

  await act(async () => trigger.click());

  const dialog = container.querySelector<HTMLElement>('[role="dialog"]');
  expect(dialog).not.toBeNull();
  expect(dialog?.getAttribute("aria-modal")).toBe("true");
  expect(dialog?.getAttribute("aria-labelledby")).toBe("settings-dialog-title");
  expect(document.activeElement).toBe(dialog);

  const focusable = [...dialog!.querySelectorAll<HTMLElement>(
    'button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])',
  )];
  const first = focusable[0]!;
  const last = focusable.at(-1)!;

  await act(async () => {
    dialog!.dispatchEvent(new KeyboardEvent("keydown", {
      key: "Tab",
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    }));
  });
  expect(document.activeElement).toBe(last);

  last.focus();
  await act(async () => {
    last.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true }));
  });
  expect(document.activeElement).toBe(first);

  await act(async () => {
    first.dispatchEvent(new KeyboardEvent("keydown", {
      key: "Tab",
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    }));
  });
  expect(document.activeElement).toBe(last);

  await act(async () => {
    last.dispatchEvent(new KeyboardEvent("keydown", {
      key: "Escape",
      bubbles: true,
      cancelable: true,
    }));
  });

  expect(useEditorUiStore.getState().settingsOpen).toBe(false);
  expect(container.querySelector('[role="dialog"]')).toBeNull();
  expect(document.activeElement).toBe(trigger);
});

it("lets an open Settings dropdown consume the first Escape", async () => {
  await act(async () => root.render(<Harness />));
  const outsideTrigger = container.querySelector<HTMLButtonElement>("button")!;
  outsideTrigger.focus();
  await act(async () => outsideTrigger.click());

  const dropdownTrigger = container.querySelector<HTMLButtonElement>(
    'button[aria-haspopup="listbox"]',
  )!;
  await act(async () => dropdownTrigger.click());

  const selectedOption = container.querySelector<HTMLButtonElement>(
    '[role="option"][aria-selected="true"]',
  )!;
  expect(document.activeElement).toBe(selectedOption);

  const firstEscape = new KeyboardEvent("keydown", {
    key: "Escape",
    bubbles: true,
    cancelable: true,
  });
  await act(async () => selectedOption.dispatchEvent(firstEscape));

  expect(firstEscape.defaultPrevented).toBe(true);
  expect(useEditorUiStore.getState().settingsOpen).toBe(true);
  expect(container.querySelector('[role="listbox"]')).toBeNull();
  expect(document.activeElement).toBe(dropdownTrigger);

  await act(async () => {
    dropdownTrigger.dispatchEvent(new KeyboardEvent("keydown", {
      key: "Escape",
      bubbles: true,
      cancelable: true,
    }));
  });

  expect(useEditorUiStore.getState().settingsOpen).toBe(false);
  expect(container.querySelector('[role="dialog"]')).toBeNull();
  expect(document.activeElement).toBe(outsideTrigger);
});

it("keeps the compact proxy switch inside a 24px pointer target", async () => {
  await act(async () => root.render(<Harness />));
  await act(async () => container.querySelector<HTMLButtonElement>("button")!.click());

  const proxySwitch = container.querySelector<HTMLInputElement>('[role="switch"]')!;
  const target = proxySwitch.closest<HTMLElement>("label");
  expect(target?.style.width).toBe("24px");
  expect(target?.style.height).toBe("24px");
  expect(proxySwitch.style.width).toBe("16px");
  expect(proxySwitch.style.height).toBe("16px");
});

it("switches dark window layouts with an accessible radio group", async () => {
  useEditorUiStore.setState({ settingsPane: "appearance" });
  await act(async () => root.render(<Harness />));
  await act(async () => container.querySelector<HTMLButtonElement>("button")!.click());

  const group = container.querySelector<HTMLElement>('[role="radiogroup"]');
  const choices = [...container.querySelectorAll<HTMLButtonElement>('[role="radio"]')];
  expect(group?.getAttribute("aria-label")).toBe(t("settings.windowSize"));
  expect(choices.map((choice) => choice.textContent?.trim())).toEqual(["深色 · 标准", "深色 · 紧凑"]);
  expect(choices[0]?.getAttribute("aria-checked")).toBe("true");
  expect(choices[1]?.getAttribute("aria-checked")).toBe("false");

  await act(async () => {
    choices[0]!.dispatchEvent(new KeyboardEvent("keydown", {
      key: "ArrowRight",
      bubbles: true,
      cancelable: true,
    }));
  });

  expect(useSettingsStore.getState().windowSize).toBe("compact");
  expect(choices[1]?.getAttribute("aria-checked")).toBe("true");
  expect(document.activeElement).toBe(choices[1]);
});

it("renders external MCP pairing controls fail-closed outside the desktop shell", async () => {
  useEditorUiStore.setState({ settingsPane: "mcp" });
  await act(async () => root.render(<Harness />));
  await act(async () => container.querySelector<HTMLButtonElement>("button")!.click());

  const status = container.querySelector<HTMLElement>("[data-external-mcp-status='disabled']");
  const toggle = container.querySelector<HTMLInputElement>("input[role='switch']");
  expect(status?.getAttribute("role")).toBe("status");
  expect(status?.textContent).toContain(t("mcp.status.disabled"));
  expect(toggle?.checked).toBe(false);
  expect(container.textContent).toContain(t("mcp.note"));
  expect(container.textContent).toContain("http://127.0.0.1:19789/mcp");
  expect(container.querySelector<HTMLInputElement>("input[name='external-mcp-client-name']")).not.toBeNull();
});

it("blocks pane navigation and modal close until a paired receipt is rendered", async () => {
  const pair = deferred<ReturnType<typeof mcpReceipt>>();
  const token = `pair-receipt-${crypto.randomUUID()}`;
  mcp.pair.mockReturnValueOnce(pair.promise);
  useEditorUiStore.setState({ settingsPane: "mcp" });
  await act(async () => root.render(<Harness />));
  await act(async () => container.querySelector<HTMLButtonElement>("button")!.click());
  await act(async () => undefined);

  await setInput(
    container.querySelector<HTMLInputElement>("input[name='external-mcp-client-name']")!,
    "Cursor",
  );
  await act(async () => button(t("mcp.pair")).click());

  const done = button(t("settings.done"));
  const close = container.querySelector<HTMLButtonElement>("button[aria-label='Close']")!;
  const general = button(t("settings.section.general"));
  expect(done.disabled).toBe(true);
  expect(close.disabled).toBe(true);
  expect(general.disabled).toBe(true);
  await act(async () => {
    document.activeElement?.dispatchEvent(new KeyboardEvent("keydown", {
      key: "Escape",
      bubbles: true,
      cancelable: true,
    }));
  });
  expect(useEditorUiStore.getState()).toMatchObject({ settingsOpen: true, settingsPane: "mcp" });

  await act(async () => pair.resolve(mcpReceipt(token)));
  expect(container.textContent).toContain(token);
  expect(done.disabled).toBe(false);
  expect(close.disabled).toBe(false);
  expect(general.disabled).toBe(false);
});

it("blocks navigation until a regenerated receipt is rendered", async () => {
  const initial = mcpStatus([mcpClient()]);
  const regenerate = deferred<ReturnType<typeof mcpReceipt>>();
  const token = `regenerated-receipt-${crypto.randomUUID()}`;
  mcp.status.mockResolvedValue(initial);
  mcp.subscribe.mockImplementation(async (handler: (status: typeof initial) => void) => {
    handler(initial);
    return mcp.unlisten;
  });
  mcp.regenerate.mockReturnValueOnce(regenerate.promise);
  useEditorUiStore.setState({ settingsPane: "mcp" });
  await act(async () => root.render(<Harness />));
  await act(async () => container.querySelector<HTMLButtonElement>("button")!.click());
  await act(async () => undefined);

  await act(async () => button(t("mcp.regenerate")).click());
  await act(async () => button(t("mcp.confirmRegenerate")).click());
  const done = button(t("settings.done"));
  const general = button(t("settings.section.general"));
  expect(done.disabled).toBe(true);
  expect(general.disabled).toBe(true);

  await act(async () => regenerate.resolve(mcpReceipt(token)));
  expect(container.textContent).toContain(token);
  expect(done.disabled).toBe(false);
  expect(general.disabled).toBe(false);
});

it("drives official Codex ChatGPT login, polling, cancellation, logout, and unavailable states", async () => {
  const signedOut = {
    available: true,
    authenticated: false,
    authMethod: null,
    version: "codex-cli 0.146.0",
    loginInProgress: false,
    message: "Not logged in",
  };
  const waiting = { ...signedOut, loginInProgress: true, message: "Waiting" };
  const signedIn = {
    ...signedOut,
    authenticated: true,
    authMethod: "ChatGPT",
    message: "Logged in using ChatGPT",
  };
  let poll: (() => void) | null = null;
  const interval = vi.spyOn(window, "setInterval").mockImplementation((callback) => {
    poll = callback as () => void;
    return 77 as unknown as ReturnType<typeof setInterval>;
  });
  vi.spyOn(window, "clearInterval").mockImplementation(() => undefined);
  codex.status.mockResolvedValue(signedOut);
  codex.login.mockResolvedValue(waiting);
  codex.cancel.mockResolvedValue(signedOut);
  codex.logout.mockResolvedValue(signedOut);
  useEditorUiStore.setState({ settingsPane: "ai" });
  useSettingsStore.setState({ byokProvider: "codex" });

  await act(async () => root.render(<Harness />));
  await act(async () => container.querySelector<HTMLButtonElement>("button")!.click());
  await act(async () => undefined);
  expect(container.textContent).toContain(t("settings.codexSignedOut"));
  expect(container.querySelector('input[type="password"]')).toBeNull();

  const button = (label: string) =>
    [...container.querySelectorAll<HTMLButtonElement>("button")].find(
      (candidate) => candidate.textContent?.trim() === label,
    )!;
  await act(async () => button(t("settings.codexLogin")).click());
  expect(codex.login).toHaveBeenCalledOnce();
  expect(container.textContent).toContain(t("settings.codexWaiting"));
  expect(interval).toHaveBeenCalledOnce();

  codex.status.mockResolvedValueOnce(signedIn);
  await act(async () => {
    poll?.();
    await Promise.resolve();
  });
  expect(container.textContent).toContain(t("settings.codexSignedIn", { method: "ChatGPT" }));
  await act(async () => button(t("settings.codexLogout")).click());
  expect(codex.logout).toHaveBeenCalledOnce();
  expect(container.textContent).toContain(t("settings.codexSignedOut"));

  await act(async () => button(t("settings.codexLogin")).click());
  expect(container.textContent).toContain(t("settings.codexWaiting"));
  await act(async () => button(t("settings.codexCancel")).click());
  expect(codex.cancel).toHaveBeenCalledOnce();
  expect(container.textContent).toContain(t("settings.codexSignedOut"));

  interval.mockRestore();
});

it("keeps Codex login disabled until the authoritative initial status arrives", async () => {
  const initial = deferred<{
    available: boolean;
    authenticated: boolean;
    authMethod: string | null;
    version: string | null;
    loginInProgress: boolean;
    message: string;
  }>();
  codex.status.mockReturnValueOnce(initial.promise);
  useEditorUiStore.setState({ settingsPane: "ai" });
  useSettingsStore.setState({ byokProvider: "codex" });

  await act(async () => root.render(<Harness />));
  await act(async () => container.querySelector<HTMLButtonElement>("button")!.click());
  const login = () => [...container.querySelectorAll<HTMLButtonElement>("button")].find(
    (candidate) => candidate.textContent?.trim() === t("settings.codexLogin"),
  )!;

  expect(login().disabled).toBe(true);
  await act(async () => login().click());
  expect(codex.login).not.toHaveBeenCalled();

  initial.resolve({
    available: true,
    authenticated: false,
    authMethod: null,
    version: "codex-cli 0.146.0",
    loginInProgress: false,
    message: "Not logged in",
  });
  await act(async () => initial.promise);
  expect(login().disabled).toBe(false);
});

it("does not let an older Codex poll overwrite a completed cancellation", async () => {
  const signedOut = {
    available: true,
    authenticated: false,
    authMethod: null,
    version: "codex-cli 0.146.0",
    loginInProgress: false,
    message: "Not logged in",
  };
  const waiting = { ...signedOut, loginInProgress: true, message: "Waiting" };
  const stalePoll = deferred<typeof waiting>();
  let poll: (() => void) | null = null;
  const interval = vi.spyOn(window, "setInterval").mockImplementation((callback) => {
    poll = callback as () => void;
    return 88 as unknown as ReturnType<typeof setInterval>;
  });
  vi.spyOn(window, "clearInterval").mockImplementation(() => undefined);
  codex.status
    .mockResolvedValueOnce(signedOut)
    .mockReturnValueOnce(stalePoll.promise);
  codex.login.mockResolvedValueOnce(waiting);
  codex.cancel.mockResolvedValueOnce(signedOut);
  useEditorUiStore.setState({ settingsPane: "ai" });
  useSettingsStore.setState({ byokProvider: "codex" });

  await act(async () => root.render(<Harness />));
  await act(async () => container.querySelector<HTMLButtonElement>("button")!.click());
  await act(async () => Promise.resolve());
  const button = (label: string) =>
    [...container.querySelectorAll<HTMLButtonElement>("button")].find(
      (candidate) => candidate.textContent?.trim() === label,
    )!;
  await act(async () => button(t("settings.codexLogin")).click());
  await act(async () => {
    poll?.();
    await Promise.resolve();
  });
  await act(async () => button(t("settings.codexCancel")).click());
  expect(container.textContent).toContain(t("settings.codexSignedOut"));

  stalePoll.resolve(waiting);
  await act(async () => stalePoll.promise);
  expect(container.textContent).toContain(t("settings.codexSignedOut"));
  expect(container.textContent).not.toContain(t("settings.codexWaiting"));
  interval.mockRestore();
});

it("disables official Codex login when the CLI is unavailable", async () => {
  codex.status.mockResolvedValue({
    available: false,
    authenticated: false,
    authMethod: null,
    version: null,
    loginInProgress: false,
    message: "Official Codex CLI was not found",
  });
  useEditorUiStore.setState({ settingsPane: "ai" });
  useSettingsStore.setState({ byokProvider: "codex" });

  await act(async () => root.render(<Harness />));
  await act(async () => container.querySelector<HTMLButtonElement>("button")!.click());
  await act(async () => undefined);

  expect(container.textContent).toContain(t("settings.codexUnavailable"));
  const login = [...container.querySelectorAll<HTMLButtonElement>("button")].find(
    (candidate) => candidate.textContent?.trim() === t("settings.codexLogin"),
  )!;
  expect(login.disabled).toBe(true);
  expect(container.querySelector('input[type="password"]')).toBeNull();
});

it("surfaces official Codex status failures without exposing an API-key field", async () => {
  codex.status.mockRejectedValue(new Error("status exploded"));
  useEditorUiStore.setState({ settingsPane: "ai" });
  useSettingsStore.setState({ byokProvider: "codex" });

  await act(async () => root.render(<Harness />));
  await act(async () => container.querySelector<HTMLButtonElement>("button")!.click());
  await act(async () => undefined);

  expect(container.textContent).toContain(
    t("settings.codexActionFailed", { error: "status exploded" }),
  );
  expect(container.querySelector('input[type="password"]')).toBeNull();
});
