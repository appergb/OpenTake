import { useEffect, useRef, type CSSProperties } from "react";
import { useT } from "../../i18n";
import { useEditorUiStore } from "../../store/uiStore";

function describeSettings(fps: number, width: number, height: number): string {
  return `${width} × ${height} · ${fps} fps`;
}

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function focusableElements(container: HTMLElement): HTMLElement[] {
  return [...container.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (element) => element.tabIndex >= 0 && !element.hidden,
  );
}

const choiceButtonStyle: CSSProperties = {
  minHeight: 28,
  padding: "0 12px",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  border: "var(--bw-thin) solid var(--border-primary)",
  borderRadius: "var(--radius-sm)",
  background: "var(--bg-raised)",
};

export function ProjectSettingsMismatchDialog() {
  const t = useT();
  const prompt = useEditorUiStore((state) => state.projectSettingsPrompt);
  const resolve = useEditorUiStore((state) => state.resolveProjectSettingsPrompt);
  const dialogRef = useRef<HTMLElement>(null);
  const primaryButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!prompt) return;
    const previousFocus =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const dialog = dialogRef.current;
    primaryButtonRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      if (event.key === "Escape") {
        event.preventDefault();
        resolve(false);
        return;
      }
      if (event.key !== "Tab" || !dialog) return;
      const focusables = focusableElements(dialog);
      if (focusables.length === 0) {
        event.preventDefault();
        dialog.focus();
        return;
      }
      const first = focusables[0]!;
      const last = focusables[focusables.length - 1]!;
      const active = document.activeElement;
      if (event.shiftKey && (active === first || !dialog.contains(active))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (active === last || !dialog.contains(active))) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      if (previousFocus?.isConnected) previousFocus.focus();
    };
  }, [prompt, resolve]);

  if (!prompt) return null;
  return (
    <div
      role="presentation"
      className="app-dialog-backdrop"
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 10020,
        display: "grid",
        placeItems: "center",
        padding: 24,
        background: "rgba(0, 0, 0, 0.55)",
      }}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) resolve(false);
      }}
    >
      <section
        ref={dialogRef}
        role="dialog"
        tabIndex={-1}
        aria-modal="true"
        aria-labelledby="project-settings-mismatch-title"
        aria-describedby="project-settings-mismatch-description"
        className="app-dialog-surface"
        style={{
          width: "min(460px, 100%)",
          border: "var(--bw-thin) solid var(--border-primary)",
          borderRadius: 12,
          padding: 20,
          background: "var(--bg-elevated)",
          boxShadow: "0 20px 60px rgba(0,0,0,0.45)",
          color: "var(--text-primary)",
        }}
      >
        <h2 id="project-settings-mismatch-title" style={{ margin: 0, fontSize: 18 }}>
          {t("projectSettingsMismatch.title")}
        </h2>
        <p
          id="project-settings-mismatch-description"
          style={{ margin: "10px 0 18px", color: "var(--text-secondary)", lineHeight: 1.5 }}
        >
          {t("projectSettingsMismatch.description")}
        </p>
        <dl style={{ margin: 0, display: "grid", gap: 8 }}>
          <div style={{ display: "flex", justifyContent: "space-between", gap: 16 }}>
            <dt>{t("projectSettingsMismatch.current")}</dt>
            <dd style={{ margin: 0 }}>{describeSettings(prompt.current.fps, prompt.current.width, prompt.current.height)}</dd>
          </div>
          <div style={{ display: "flex", justifyContent: "space-between", gap: 16 }}>
            <dt>{t("projectSettingsMismatch.source")}</dt>
            <dd style={{ margin: 0 }}>{describeSettings(prompt.suggested.fps, prompt.suggested.width, prompt.suggested.height)}</dd>
          </div>
        </dl>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 10, marginTop: 22 }}>
          <button type="button" style={choiceButtonStyle} onClick={() => resolve(false)}>
            {t("projectSettingsMismatch.keep")}
          </button>
          <button
            ref={primaryButtonRef}
            type="button"
            style={choiceButtonStyle}
            onClick={() => resolve(true)}
          >
            {t("projectSettingsMismatch.match")}
          </button>
        </div>
      </section>
    </div>
  );
}
