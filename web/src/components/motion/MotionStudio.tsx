import { useT } from "../../i18n";

const panelStyle = {
  minWidth: 0,
  minHeight: 0,
  background: "color-mix(in srgb, var(--bg-elevated) 82%, black)",
  border: "var(--bw-thin) solid var(--border-primary)",
  borderRadius: "var(--radius-sm)",
  overflow: "hidden",
} as const;

const headingStyle = {
  margin: 0,
  padding: "10px 12px 8px",
  color: "var(--text-secondary)",
  fontSize: "var(--fs-xs)",
  fontWeight: "var(--fw-semibold)",
  letterSpacing: "0.04em",
  textTransform: "uppercase",
} as const;

export function MotionStudio() {
  const t = useT();

  return (
    <main
      aria-label={t("motionStudio.workspace")}
      style={{
        width: "100%",
        height: "100%",
        minWidth: 0,
        minHeight: 0,
        display: "grid",
        gridTemplateColumns:
          "minmax(148px, 0.72fr) minmax(360px, 2.4fr) minmax(176px, 0.86fr)",
        gridTemplateRows: "minmax(0, 1fr) minmax(104px, 0.3fr)",
        gap: 6,
        padding: 6,
        background: "#0b0c0f",
        color: "var(--text-primary)",
      }}
    >
      <aside aria-label={t("motionStudio.files")} style={panelStyle}>
        <h2 style={headingStyle}>{t("motionStudio.files")}</h2>
        {["motionStudio.fileIndex", "motionStudio.fileStyles"].map((key, index) => (
          <div
            key={key}
            style={{
              margin: "0 6px 3px",
              padding: "7px 8px",
              borderRadius: "var(--radius-xs-sm)",
              background: index === 0 ? "rgba(255,255,255,.08)" : "transparent",
              color: index === 0 ? "var(--text-primary)" : "var(--text-secondary)",
              fontFamily: "var(--font-mono)",
              fontSize: "var(--fs-xs)",
            }}
          >
            {t(key)}
          </div>
        ))}
      </aside>

      <section
        aria-label={t("motionStudio.editor")}
        style={{ ...panelStyle, display: "grid", gridTemplateRows: "minmax(0, 1fr) auto" }}
      >
        <div style={{ minHeight: 0, display: "grid", placeItems: "center", padding: 24 }}>
          <div style={{ maxWidth: 520 }}>
            <p
              style={{
                margin: "0 0 8px",
                fontSize: "clamp(24px, 3.3vw, 48px)",
                fontWeight: 650,
                letterSpacing: "-0.035em",
              }}
            >
              {t("motionStudio.visibleStarterTitle")}
            </p>
            <p style={{ margin: 0, color: "var(--text-secondary)", fontSize: "var(--fs-sm)" }}>
              {t("motionStudio.visibleStarterSubtitle")}
            </p>
          </div>
        </div>
        <div
          style={{
            padding: "8px 12px",
            borderTop: "var(--bw-thin) solid var(--border-primary)",
            color: "var(--text-tertiary)",
            fontFamily: "var(--font-mono)",
            fontSize: "var(--fs-xs)",
          }}
        >
          &lt;main class=&quot;motion&quot;&gt;…&lt;/main&gt;
        </div>
      </section>

      <aside aria-label={t("motionStudio.inspector")} style={panelStyle}>
        <h2 style={headingStyle}>{t("motionStudio.inspector")}</h2>
      </aside>

      <figure
        role="region"
        aria-label={t("motionStudio.preview")}
        style={{
          ...panelStyle,
          gridColumn: "2",
          gridRow: "1",
          alignSelf: "end",
          justifySelf: "end",
          width: "min(42%, 420px)",
          aspectRatio: "16 / 9",
          margin: 12,
          display: "grid",
          placeItems: "center",
          background: "#050506",
          boxShadow: "0 16px 50px rgba(0,0,0,.34)",
          color: "var(--text-tertiary)",
          fontSize: "var(--fs-xs)",
        }}
      >
        {t("motionStudio.previewPending")}
      </figure>

      <section
        aria-label={t("motionStudio.timeline")}
        style={{
          ...panelStyle,
          gridColumn: "1 / -1",
          display: "grid",
          gridTemplateRows: "auto 1fr",
        }}
      >
        <h2 style={headingStyle}>{t("motionStudio.timeline")}</h2>
        <div
          aria-hidden="true"
          style={{
            margin: "0 12px 12px",
            borderTop: "var(--bw-thin) solid var(--border-subtle)",
            background:
              "repeating-linear-gradient(90deg, transparent 0 31px, rgba(255,255,255,.09) 31px 32px)",
          }}
        />
      </section>
    </main>
  );
}
