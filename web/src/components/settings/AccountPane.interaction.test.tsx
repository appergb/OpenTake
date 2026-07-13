// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AccountInfo, AccountStatus } from "../../lib/types";

const accountApi = vi.hoisted(() => ({
  getBackendUrl: vi.fn(),
  getStatus: vi.fn(),
  login: vi.fn(),
  logout: vi.fn(),
  setBackendUrl: vi.fn(),
}));

vi.mock("../../lib/api", () => ({
  accountGetBackendUrl: accountApi.getBackendUrl,
  accountGetStatus: accountApi.getStatus,
  accountLogin: accountApi.login,
  accountLogout: accountApi.logout,
  accountSetBackendUrl: accountApi.setBackendUrl,
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: string, vars?: Record<string, string | number>) =>
    vars ? `${key} ${Object.values(vars).join(" ")}` : key,
}));

import { AccountPane } from "./AccountPane";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let root: Root | null = null;
let container: HTMLDivElement | null = null;
let savedBackend: string | null;
let status: AccountStatus;

async function flush(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

async function renderPane(): Promise<HTMLDivElement> {
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  await act(async () => {
    root?.render(<AccountPane />);
  });
  await flush();
  return container;
}

function input(id: string): HTMLInputElement {
  const element = container?.querySelector<HTMLInputElement>(`#${id}`);
  if (!element) throw new Error(`missing input ${id}`);
  return element;
}

async function setInputValue(element: HTMLInputElement, value: string): Promise<void> {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
  if (!setter) throw new Error("missing native input value setter");
  await act(async () => {
    setter.call(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

function button(label: string): HTMLButtonElement {
  const element = [...(container?.querySelectorAll<HTMLButtonElement>("button") ?? [])].find(
    (candidate) => candidate.textContent?.trim() === label,
  );
  if (!element) throw new Error(`missing button ${label}`);
  return element;
}

async function click(element: HTMLElement): Promise<void> {
  await act(async () => {
    element.click();
    await Promise.resolve();
  });
  await flush();
}

async function pressEnter(element: HTMLInputElement): Promise<void> {
  await act(async () => {
    element.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    await Promise.resolve();
  });
  await flush();
}

function deferred(): { promise: Promise<void>; resolve: () => void } {
  let resolvePromise: (() => void) | undefined;
  const promise = new Promise<void>((resolve) => {
    resolvePromise = resolve;
  });
  return {
    promise,
    resolve: () => resolvePromise?.(),
  };
}

beforeEach(() => {
  savedBackend = null;
  status = { type: "offline" };
  accountApi.getBackendUrl.mockReset().mockImplementation(async () => savedBackend);
  accountApi.getStatus.mockReset().mockImplementation(async () => status);
  accountApi.setBackendUrl.mockReset().mockImplementation(async (url: string | null) => {
    savedBackend = url;
    status = { type: "offline" };
  });
  accountApi.login.mockReset().mockImplementation(async () => {
    const info = { userId: "verified-user" };
    status = { type: "online", info };
    return info;
  });
  accountApi.logout.mockReset().mockImplementation(async () => {
    status = { type: "offline" };
  });
});

afterEach(async () => {
  if (root) {
    await act(async () => root?.unmount());
  }
  container?.remove();
  root = null;
  container = null;
});

describe("AccountPane interactions", () => {
  it("accepts null optional fields from the Rust account DTO", () => {
    const info: AccountInfo = { userId: "minimal-user", email: null, plan: null };

    expect(info.email).toBeNull();
    expect(info.plan).toBeNull();
  });

  it("loads a stored credential without network login and keeps logout available", async () => {
    savedBackend = "https://accounts.example.com";
    status = { type: "stored" };

    const view = await renderPane();

    expect(input("account-backend-url").value).toBe(savedBackend);
    expect(view.textContent).toContain("account.status.stored");
    expect(view.querySelector('[aria-label="account.logout"]')).not.toBeNull();
    expect(accountApi.login).not.toHaveBeenCalled();
  });

  it("blocks login for a dirty URL, coalesces double-save, then logs in on Enter", async () => {
    savedBackend = "https://old.example.com";
    await renderPane();
    await setInputValue(input("account-token"), "  secret-token  ");
    await setInputValue(input("account-backend-url"), "https://new.example.com");

    const loginButton = button("account.login");
    expect(loginButton.disabled).toBe(true);
    expect(container?.textContent).toContain("account.backendUrlUnsaved");
    await click(loginButton);
    expect(accountApi.login).not.toHaveBeenCalled();

    const gate = deferred();
    accountApi.setBackendUrl.mockImplementationOnce(async (url: string | null) => {
      await gate.promise;
      savedBackend = url;
    });
    const saveButton = button("account.saveBackendUrl");
    await act(async () => {
      saveButton.click();
      saveButton.click();
      await Promise.resolve();
    });
    expect(accountApi.setBackendUrl).toHaveBeenCalledTimes(1);
    gate.resolve();
    await flush();

    expect(loginButton.disabled).toBe(false);
    await pressEnter(input("account-token"));
    expect(accountApi.login).toHaveBeenCalledWith("secret-token");
    expect(input("account-token").value).toBe("");
    expect(container?.textContent).toContain("account.status.online verified-user");
  });

  it("surfaces save failure and leaves the old destination protected", async () => {
    savedBackend = "https://old.example.com";
    accountApi.setBackendUrl.mockRejectedValueOnce(new Error("keychain unavailable"));
    await renderPane();
    await setInputValue(input("account-backend-url"), "https://new.example.com");
    await setInputValue(input("account-token"), "secret-token");

    await click(button("account.saveBackendUrl"));

    expect(container?.querySelector('[role="alert"]')?.textContent).toContain(
      "account.backendUrlSaveFailed keychain unavailable",
    );
    expect(button("account.login").disabled).toBe(true);
    expect(accountApi.login).not.toHaveBeenCalled();
  });

  it("surfaces login failure and re-enables the controls", async () => {
    savedBackend = "https://accounts.example.com";
    accountApi.login.mockRejectedValueOnce(new Error("bad token"));
    await renderPane();
    await setInputValue(input("account-token"), "bad-token");

    await pressEnter(input("account-token"));

    expect(container?.querySelector('[role="alert"]')?.textContent).toContain(
      "account.loginFailed bad token",
    );
    expect(button("account.login").disabled).toBe(false);
  });

  it("clears the backend and can report a clear failure", async () => {
    savedBackend = "https://accounts.example.com";
    status = { type: "stored" };
    await renderPane();
    await setInputValue(input("account-token"), "draft-token");

    await click(button("account.clearBackendUrl"));
    expect(accountApi.setBackendUrl).toHaveBeenCalledWith(null);
    expect(input("account-backend-url").value).toBe("");
    expect(input("account-token").value).toBe("");
    expect(button("account.login").disabled).toBe(true);

    savedBackend = "https://accounts.example.com";
    accountApi.getBackendUrl.mockResolvedValueOnce(savedBackend);
    await act(async () => root?.unmount());
    root = null;
    container?.remove();
    container = null;
    await renderPane();
    accountApi.setBackendUrl.mockRejectedValueOnce(new Error("delete failed"));

    await click(button("account.clearBackendUrl"));
    expect(container?.querySelector('[role="alert"]')?.textContent).toContain(
      "account.backendUrlSaveFailed delete failed",
    );
    expect(input("account-backend-url").value).toBe(savedBackend);
  });

  it("logs out a stored credential without exposing it", async () => {
    savedBackend = "https://accounts.example.com";
    status = { type: "stored" };
    const view = await renderPane();
    const logoutButton = view.querySelector<HTMLButtonElement>('[aria-label="account.logout"]');
    if (!logoutButton) throw new Error("missing logout button");

    await click(logoutButton);

    expect(accountApi.logout).toHaveBeenCalledTimes(1);
    expect(view.textContent).toContain("account.status.offline");
  });
});
