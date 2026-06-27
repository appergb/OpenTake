import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const homeSource = readFileSync(new URL("./HomeView.tsx", import.meta.url), "utf8");
const tokenSource = readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8");
const globalSource = readFileSync(new URL("../../styles/global.css", import.meta.url), "utf8");

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
    expect(homeSource).toContain("background: \"transparent\"");
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

  it("places project-mode content near the top-left and shows projects in four columns", () => {
    expect(homeSource).toContain("padding: \"var(--titlebar-safe-top) var(--space-xl-xxl) var(--space-xl-xxl)\"");
    expect(homeSource).toContain("gridTemplateColumns: \"repeat(4, minmax(0, 1fr))\"");
    expect(homeSource).toContain("ProjectGridCard");
    expect(homeSource).not.toContain("width: \"min(720px, 100%)\"");
  });

  it("enlarges the sidebar logo to a prominent size", () => {
    // Logo 块特征：紧邻 t("app.name") 的容器，字号从 --fs-sm-md 放大到 --fs-xl
    expect(homeSource).toContain("fontSize: \"var(--fs-xl)\"");
    expect(homeSource).toContain("{t(\"app.name\")}");
  });
});
