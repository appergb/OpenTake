import { readFileSync } from "node:fs";

const { engines } = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
);
const version = process.versions.node;
const [major, minor] = version.split(".").map(Number);
// Match the intersection of the locked Vite and Vitest Node requirements.
const supported = /^\d+\.\d+\.\d+$/.test(version) && (
  (major === 20 && minor >= 19) ||
  (major === 22 && minor >= 12) ||
  major >= 24
);

if (!supported) {
  console.error(`Unsupported Node.js ${version}. Required: ${engines.node}.`);
  console.error(
    "Install a supported Node.js version (recommended: Node.js 22.x >=22.12), " +
    "then run pnpm --dir web install --frozen-lockfile from the repository root.",
  );
  process.exit(1);
}
