import { useEffect } from "react";
import { useT } from "../../i18n";
import { useEditorUiStore } from "../../store/uiStore";

function describeSettings(fps: number, width: number, height: number): string {
  return `${width} × ${height} · ${fps} fps`;
}

export function ProjectSettingsMismatchDialog() {
  const t = useT();
  const prompt = useEditorUiStore((state) => state.projectSettingsPrompt);
  const resolve = useEditorUiStore((state) => state.resolveProjectSettingsPrompt);

  useEffect(() => {
    if (!prompt) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      resolve(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [prompt, resolve]);

  if (!prompt) return null;
  return (
    <div
      role="presentation"
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
        role="dialog"
        aria-modal="true"
        aria-labelledby="project-settings-mismatch-title"
        aria-describedby="project-settings-mismatch-description"
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
          <button type="button" onClick={() => resolve(false)}>
            {t("projectSettingsMismatch.keep")}
          </button>
          <button type="button" autoFocus onClick={() => resolve(true)}>
            {t("projectSettingsMismatch.match")}
          </button>
        </div>
      </section>
    </div>
  );
}
