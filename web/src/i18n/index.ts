/**
 * Lightweight i18n runtime. A Zustand store holds the active locale (persisted
 * to localStorage, default `zh-CN`); `useT()` returns a memoized translator that
 * re-renders consumers on locale change. `{placeholders}` are interpolated from
 * the `vars` argument. No external i18n dependency — the dictionaries live in
 * `dict.ts` and the surface is tiny (translate + switch locale).
 */

import { useMemo } from "react";
import { create } from "zustand";
import { DICTS, type Dict, type Locale } from "./dict";

const LS_LOCALE = "locale";
const DEFAULT_LOCALE: Locale = "zh-CN";

function loadLocale(): Locale {
  if (typeof localStorage === "undefined") return DEFAULT_LOCALE;
  const v = localStorage.getItem(LS_LOCALE);
  return v === "en" || v === "zh-CN" ? v : DEFAULT_LOCALE;
}

interface I18nState {
  locale: Locale;
  setLocale: (locale: Locale) => void;
}

export const useI18nStore = create<I18nState>((set) => ({
  locale: loadLocale(),
  setLocale: (locale) => {
    if (typeof localStorage !== "undefined") localStorage.setItem(LS_LOCALE, locale);
    if (typeof document !== "undefined") {
      document.documentElement.lang = locale === "zh-CN" ? "zh-CN" : "en";
    }
    set({ locale });
  },
}));

/** Translate `key` in `dict`, falling back to the key itself when missing. */
function translate(dict: Dict, key: string, vars?: Record<string, string | number>): string {
  const template = dict[key] ?? key;
  if (!vars) return template;
  return template.replace(/\{(\w+)\}/g, (_m, name: string) =>
    name in vars ? String(vars[name]) : `{${name}}`,
  );
}

export type TFunction = (key: string, vars?: Record<string, string | number>) => string;

/** Hook returning a translator bound to the active locale. */
export function useT(): TFunction {
  const locale = useI18nStore((s) => s.locale);
  return useMemo(() => {
    const dict = DICTS[locale];
    return (key: string, vars?: Record<string, string | number>) => translate(dict, key, vars);
  }, [locale]);
}

/** Imperative translator for non-component contexts (e.g. action labels). */
export function t(key: string, vars?: Record<string, string | number>): string {
  const dict = DICTS[useI18nStore.getState().locale];
  return translate(dict, key, vars);
}

export { type Locale, LOCALES } from "./dict";

/** Apply the persisted locale to <html lang> at startup. */
export function initI18n(): void {
  if (typeof document !== "undefined") {
    const locale = useI18nStore.getState().locale;
    document.documentElement.lang = locale === "zh-CN" ? "zh-CN" : "en";
  }
}
