#!/usr/bin/env python3
"""Inventory maintained Markdown and check local links, without dependencies.

Uses Git's tracked and non-ignored untracked file inventory. Generated media,
marketing work and runtime caches are excluded. Historical missing evidence is
reported, never restored. --json emits the complete machine-readable inventory.
This is a file-link audit, not a heading-anchor or external-URL validator.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
EXCLUDED = ("marketing/", "output/", ".playwright-cli/", "target/", "web/node_modules/")
OWNED_ELSEWHERE = {
    "docs/releases/1.0.0-beta.6.md",
    "docs/plans/active/2026-09-06-public-beta.md",
    "docs/capabilities/CAPABILITY-LEDGER.md",
    "docs/knowledge/2026-09-06-semantic-search-model.md",
}
PENDING: set[str] = set()
LINK = re.compile(r'!?\[[^\]\n]*\]\((<[^>\n]+>|[^\s)]+)(?:\s+["\'][^\n]*?["\'])?\)')
REFERENCE = re.compile(r'^\s{0,3}\[[^\]]+\]:\s*(<[^>]+>|\S+)')
HTML_LINK = re.compile(r'\b(?:href|src)=["\']([^"\']+)["\']')


def git_files(*args: str) -> list[str]:
    result = subprocess.check_output(["git", *args, "-z"], cwd=ROOT)
    return [name for name in result.decode().split("\0") if name]


def classification(name: str) -> str:
    if name == "docs/documentation-sync-2026-09-06.md":
        return "生成的同步报告"
    if name in OWNED_ELSEWHERE or name.startswith("docs/audit/2026-09-06/"):
        return "主代理维护"
    if (
        name == "CLAUDE.md"
        or name.startswith(("docs/audit/", "docs/releases/", "docs/superpowers/archive/", "update-summary/", ".superpowers/"))
        or name.startswith("docs/superpowers/plans/")
        or Path(name).name in {"HANDOFF-2026-07.md", "PORT-1TO1-GAP.md", "FULL_PROJECT_SCAN_REPORT.md", "BUGS.md", "ROADMAP.md", "CAPCUT-GAP.md"}
    ):
        return "历史保留"
    if name.startswith(("docs/specs/", "docs/port-map/", "docs/upstream-analysis/", "docs/superpowers/specs/")) or Path(name).name == "SPEC.md":
        return "设计与来源参考"
    if name.startswith("crates/") or "assets/" in name or name.endswith("THIRD_PARTY_NOTICES.md"):
        return "运行资源或归属记录"
    return "维护文档"


def local_targets(content: str):
    fence: str | None = None
    for line_number, line in enumerate(content.splitlines(), 1):
        marker = re.match(r"^\s{0,3}(`{3,}|~{3,})", line)
        if marker:
            token = marker.group(1)
            if fence is None:
                fence = token
            elif token[0] == fence[0] and len(token) >= len(fence):
                fence = None
            continue
        if fence is not None:
            continue
        # Backticked paths describe code/history, not clickable Markdown links.
        line = re.sub(r"(`+).*?\1", "", line)
        matches = list(LINK.finditer(line)) + list(HTML_LINK.finditer(line))
        reference = REFERENCE.match(line)
        if reference:
            matches.append(reference)
        for match in matches:
            target = match.group(1).strip("<>")
            parsed = urlsplit(target)
            if parsed.scheme or parsed.netloc or not parsed.path:
                continue
            yield line_number, target, unquote(parsed.path)


def audit() -> dict:
    names = sorted(set(git_files("ls-files", "--cached", "--others", "--exclude-standard")))
    deleted = set(git_files("diff", "--name-only", "--diff-filter=D"))
    changed = set(git_files("diff", "--name-only"))
    tracked = set(git_files("ls-files", "--cached"))
    inventory = []
    issues = []
    link_count = 0
    for name in names:
        path = ROOT / name
        if path.suffix.lower() != ".md" or name.startswith(EXCLUDED) or not path.is_file():
            continue
        category = classification(name)
        content = path.read_text(encoding="utf-8")
        count = 0
        for line, target, destination in local_targets(content):
            count += 1
            resolved = (path.parent / destination).resolve()
            if resolved.exists():
                continue
            try:
                relative = resolved.relative_to(ROOT).as_posix()
            except ValueError:
                relative = str(resolved)
            if relative in deleted:
                kind = "保留用户删除"
            elif relative in PENDING:
                kind = "候选文档待主代理生成"
            elif relative.startswith("docs/audit/2026-09-06/"):
                kind = "主代理范围待处理"
            elif category == "主代理维护":
                kind = "主代理范围待处理"
            elif category == "历史保留" and not relative.endswith(('.md', '.MD')):
                kind = "历史证据或外部路径缺失"
            else:
                kind = "断链"
            issues.append(dict(file=name, line=line, target=target, resolved=relative, kind=kind))
        link_count += count
        inventory.append(dict(path=name, category=category, changed=name in changed or name not in tracked, local_links=count))
    entries = [row["path"] for row in inventory if Path(row["path"]).name.lower() in {"agent.md", "agents.md"}]
    errors = sum(issue["kind"] == "断链" for issue in issues)
    if entries != ["AGENTS.md"]:
        errors += 1
    return dict(markdown_count=len(inventory), local_link_count=link_count, errors=errors,
                agent_entries=entries, inventory=inventory, issues=issues)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="emit complete audit JSON to stdout")
    args = parser.parse_args()
    result = audit()
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(f"Markdown: {result['markdown_count']}; local links: {result['local_link_count']}; errors: {result['errors']}")
        print("Agent entries: " + ", ".join(result["agent_entries"]))
        for issue in result["issues"]:
            print(f"{issue['kind']}: {issue['file']}:{issue['line']} -> {issue['target']}")
    return 1 if result["errors"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
