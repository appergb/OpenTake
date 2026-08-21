import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const homeSource = readFileSync(new URL("./HomeView.tsx", import.meta.url), "utf8").replace(/\r\n?/g, "\n");
const projectLauncherSource = homeSource.slice(
  homeSource.indexOf("function ProjectLauncher"),
  homeSource.indexOf("function ProjectHero"),
);
const projectGridCardSource = homeSource.slice(homeSource.indexOf("function ProjectGridCard"));
const tokenSource = readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8");
const globalSource = readFileSync(new URL("../../styles/global.css", import.meta.url), "utf8");
const componentSource = readFileSync(new URL("../../styles/components.css", import.meta.url), "utf8");

describe("HomeView Vercel embedded visual direction", () => {
  it("uses homepage-specific Vercel tokens without replacing editor tokens", () => {
    expect(tokenSource).toContain("--home-bg: #0a0a0a");
    expect(tokenSource).toContain("--home-card: #171717");
    expect(tokenSource).toContain("--home-border: #282828");
    expect(tokenSource).toContain("--bg-base: rgb(10, 10, 10)");
  });

  it("keeps the sidebar on the background layer and floats the right workspace", () => {
    expect(homeSource).toContain("homeSidebarStyle");
    expect(homeSource).toContain("homeWorkspaceStyle");
    expect(homeSource).toContain("minWidth: 0");
    expect(homeSource).toContain("minHeight: 0");
    expect(homeSource).toContain("background: \"transparent\"");
    expect(homeSource).toContain("overflowY: \"auto\"");
    expect(homeSource).not.toContain("borderRight");
  });

  it("removes decorative tilt and radial hover effects from project cards", () => {
    expect(homeSource).not.toContain("perspective(1000px)");
    expect(homeSource).not.toContain("rotateX");
    expect(homeSource).not.toContain("rotateY");
    expect(homeSource).not.toContain("radial-gradient(circle 100px");
    expect(homeSource).not.toContain("scale3d");
  });

  it("uses restrained card hover styling instead of inline motion effects", () => {
    expect(homeSource).toContain("home-project-card");
    expect(globalSource).toContain(".home-project-card:hover");
    expect(globalSource).toContain("border-color: rgba(255, 255, 255, 0.24)");
  });

  it("uses equal inset spacing around the embedded stage", () => {
    expect(homeSource).toContain("padding: \"var(--home-stage-inset)\"");
    expect(tokenSource).toContain("--home-stage-inset:");
    expect(tokenSource).toContain("--titlebar-safe-top: clamp(");
  });

  it("uses separate empty and project-first home states", () => {
    expect(homeSource).toContain("EmptyLauncher");
    expect(homeSource).toContain("ProjectLauncher");
    expect(homeSource).toContain("recents.length === 0");
    expect(homeSource).not.toContain("gridTemplateColumns: \"repeat(auto-fill");
    expect(homeSource).not.toContain("minHeight: 132");
  });

  it("does not pin recent projects to a bottom rail when projects exist", () => {
    expect(homeSource).not.toContain("RecentProjectsRail");
    expect(homeSource).not.toContain("recents.slice(0, 3)");
  });

  it("keeps a full left-aligned promotional hero when projects exist", () => {
    expect(homeSource).toContain("ProjectHero");
    expect(homeSource).not.toContain("CompactHero");
    expect(homeSource).toContain("{t(\"home.welcome\")}");
    expect(homeSource).toContain("{t(\"app.tagline\")}");
    expect(homeSource).toContain("alignItems: \"flex-start\"");
    expect(homeSource).toContain("textAlign: \"left\"");
  });

  it("places project-mode content near the top-left in responsive project columns", () => {
    expect(homeSource).toContain("padding: \"var(--titlebar-safe-top) var(--space-xl-xxl) var(--space-xl-xxl)\"");
    expect(homeSource).toContain("gridTemplateColumns: \"repeat(auto-fit, minmax(min(100%, 220px), 1fr))\"");
    expect(homeSource).toContain("ProjectGridCard");
    expect(homeSource).not.toContain("width: \"min(720px, 100%)\"");
  });

  it("wraps home action rows instead of forcing fixed-width button rails on small windows", () => {
    expect(homeSource).toContain("function SampleProjectsStrip");
    expect(homeSource).toContain("function EmptyLauncher");
    expect(homeSource).toContain("function ProjectHero");
    expect(homeSource).toContain("flexWrap: \"wrap\"");
  });

  it("keeps long recent project names from widening the project grid", () => {
    expect(projectLauncherSource).toContain("flex: 1,\n        minWidth: 0,\n        minHeight: 0");
    expect(projectLauncherSource).toContain("overflowX: \"hidden\"");
    expect(projectGridCardSource).toContain("minWidth: 0");
    expect(projectGridCardSource).toContain("textOverflow: \"ellipsis\"");
    expect(projectGridCardSource).toContain("whiteSpace: \"nowrap\"");
  });

  it("uses a semantic 16:9 figure preview with covered thumbnails", () => {
    expect(projectGridCardSource).toContain("<figure");
    expect(projectGridCardSource).toContain("home-project-preview");
    expect(projectGridCardSource).toContain('className="home-project-preview__image"');
    expect(componentSource).toContain(".home-project-preview {");
    expect(componentSource).toContain("aspect-ratio: 16 / 9");
    expect(componentSource).toContain(".home-project-preview__image {");
    expect(componentSource).toContain("object-fit: cover");
  });

  it("removes the Home generation activity region", () => {
    expect(homeSource).not.toContain("GenerationActivity");
    expect(homeSource).not.toContain("home-generation-heading");
  });

  it("enlarges the sidebar logo to a prominent size", () => {
    // Logo 块特征：紧邻 t("app.name") 的容器，字号从 --fs-sm-md 放大到 --fs-xl
    expect(homeSource).toContain("fontSize: \"var(--fs-xl)\"");
    expect(homeSource).toContain("{t(\"app.name\")}");
  });
});
