from __future__ import annotations

import importlib.util
import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import check_release_workflow as contract


REPOSITORY_ROOT = Path(
    os.environ.get("OPENTAKE_REPOSITORY_ROOT", Path(__file__).resolve().parents[1])
).resolve()
WORKFLOW_PATH = Path(
    os.environ.get(
        "OPENTAKE_RELEASE_WORKFLOW_PATH",
        REPOSITORY_ROOT / ".github" / "workflows" / "release.yml",
    )
).resolve()
RELEASE_NOTES_PATH = Path(
    os.environ.get(
        "OPENTAKE_RELEASE_NOTES_PATH",
        REPOSITORY_ROOT / "docs" / "releases" / "1.0.0-beta.4.md",
    )
).resolve()
WORKFLOW = WORKFLOW_PATH.read_text(encoding="utf-8") if WORKFLOW_PATH.is_file() else ""
CHECKOUT_SHA = "11d5960a326750d5838078e36cf38b85af677262"
BETA4_FAILED_RUN_ID = 31412976593
BETA4_SOURCE_SHA = "2c4efdff9d2587c90cbcac0919f9d1d333d67d6a"
BETA4_RECOVERY2_RUN_ID = 31427093503
BETA4_RECOVERY1_TOOLING_SHA = "6162466834bbabb8a16a2c08808e03a53c2b22b6"
BETA4_NEXT_TOOLING_SHA = "d90c87df0c0b4194635cd20de5e5816b6797d0c0"
RECOVERY_WINDOWS_STEP_OUTCOMES = (
    ("Set up job", "success"),
    (f"Run actions/checkout@{CHECKOUT_SHA}", "success"),
    ("Assert exact checked-out SHA", "success"),
    ("Require updater signing secrets", "success"),
    ("Install Rust toolchain", "success"),
    (
        "Run pnpm/action-setup@b906affcce14559ad1aafd4ab0e942779e9f58b1",
        "success",
    ),
    (
        "Run actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020",
        "success",
    ),
    (
        "Run ruby/setup-ruby@95ef2b042f9d7a56d8268cba8559e2842e2ad01b",
        "success",
    ),
    ("Provision checksum-pinned Windows FFmpeg sidecars", "failure"),
    ("Verify pinned sidecar supply", "skipped"),
    ("Cache Cargo dependencies", "skipped"),
    ("Install locked Web dependencies", "skipped"),
    ("Rust workspace clippy", "skipped"),
    ("Rust workspace tests", "skipped"),
    ("Web editor behavior suite", "skipped"),
    ("Minimal-feature Tauri clippy", "skipped"),
    ("Web production build", "skipped"),
    ("Reassert exact source before Windows build", "skipped"),
    ("Build native MSI, NSIS, and signed updater artifacts", "skipped"),
    (
        "Install NSIS and smoke installed app, sidecars, and updater artifacts",
        "skipped",
    ),
    ("Reassert exact source after Windows packaging", "skipped"),
    ("Create and sign Windows updater attestations", "skipped"),
    ("Create Windows exact-SHA receipt", "skipped"),
    ("Upload exact-SHA Windows packages and updater signatures", "skipped"),
    (
        "Post Run actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020",
        "skipped",
    ),
    (
        "Post Run pnpm/action-setup@b906affcce14559ad1aafd4ab0e942779e9f58b1",
        "success",
    ),
    (f"Post Run actions/checkout@{CHECKOUT_SHA}", "success"),
    ("Complete job", "success"),
)
RECOVERY_WINDOWS_STEP_NUMBERS = (*range(1, 25), 46, 47, 48, 49)
RECOVERY2_WINDOWS_STEP_OUTCOMES = (
    *RECOVERY_WINDOWS_STEP_OUTCOMES[:8],
    ("Provision checksum-pinned Windows FFmpeg sidecars", "success"),
    ("Verify pinned sidecar supply", "success"),
    ("Cache Cargo dependencies", "success"),
    ("Install locked Web dependencies", "success"),
    ("Rust workspace clippy", "success"),
    ("Rust workspace tests", "success"),
    ("Web editor behavior suite", "success"),
    ("Minimal-feature Tauri clippy", "success"),
    ("Web production build", "success"),
    ("Reassert exact source before Windows build", "success"),
    ("Build native MSI, NSIS, and signed updater artifacts", "failure"),
    ("Install NSIS and smoke installed app, sidecars, and updater artifacts", "skipped"),
    ("Reassert exact source after Windows packaging", "skipped"),
    ("Create and sign Windows updater attestations", "skipped"),
    ("Create Windows exact-SHA receipt", "skipped"),
    ("Upload exact-SHA Windows packages and updater signatures", "skipped"),
    ("Post Cache Cargo dependencies", "skipped"),
    (
        "Post Run actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020",
        "skipped",
    ),
    (
        "Post Run pnpm/action-setup@b906affcce14559ad1aafd4ab0e942779e9f58b1",
        "success",
    ),
    (f"Post Run actions/checkout@{CHECKOUT_SHA}", "success"),
    ("Complete job", "success"),
)
RECOVERY2_WINDOWS_STEP_NUMBERS = (*range(1, 25), 45, 46, 47, 48, 49)


def recovery_windows_steps() -> list[dict[str, object]]:
    return [
        {
            "number": number,
            "name": name,
            "status": "completed",
            "conclusion": conclusion,
        }
        for number, (name, conclusion) in zip(
            RECOVERY_WINDOWS_STEP_NUMBERS,
            RECOVERY_WINDOWS_STEP_OUTCOMES,
            strict=True,
        )
    ]


def recovery2_windows_steps() -> list[dict[str, object]]:
    return [
        {
            "number": number,
            "name": name,
            "status": "completed",
            "conclusion": conclusion,
        }
        for number, (name, conclusion) in zip(
            RECOVERY2_WINDOWS_STEP_NUMBERS,
            RECOVERY2_WINDOWS_STEP_OUTCOMES,
            strict=True,
        )
    ]


def recovery_artifacts(
    run_id: int, head_branch: str, head_sha: str
) -> dict[str, object]:
    return {
        "total_count": 1,
        "artifacts": [
            {
                "id": 9077851536,
                "name": f"opentake-macos-arm64-{BETA4_SOURCE_SHA}",
                "size_in_bytes": 142001290,
                "expired": False,
                "digest": "sha256:b62a8270268087d91bc4f8d2c8aac5d2ae2fe2cf32ec21d95bd2fd46787df612",
                "workflow_run": {
                    "id": run_id,
                    "repository_id": 1275692189,
                    "head_repository_id": 1275692189,
                    "head_branch": head_branch,
                    "head_sha": head_sha,
                },
            }
        ],
    }


def recovery_comparison(
    base_sha: str,
    head_sha: str = BETA4_RECOVERY1_TOOLING_SHA,
) -> dict[str, object]:
    return {
        "status": "ahead",
        "ahead_by": 1,
        "behind_by": 0,
        "total_commits": 1,
        "base_commit": {"sha": base_sha},
        "merge_base_commit": {"sha": base_sha},
        "commits": [{"sha": head_sha}],
    }


class ReleaseWorkflowContractTests(unittest.TestCase):
    def assert_rejected(self, workflow: str, expected: str) -> None:
        self.assertIn(expected, contract.validate_workflow(workflow))

    def mutate(self, old: str, new: str) -> str:
        self.assertIn(old, WORKFLOW, f"canonical workflow is missing fixture: {old}")
        return WORKFLOW.replace(old, new, 1)

    def mutate_last(self, old: str, new: str) -> str:
        self.assertIn(old, WORKFLOW, f"canonical workflow is missing fixture: {old}")
        prefix, match, suffix = WORKFLOW.rpartition(old)
        self.assertEqual(old, match)
        return prefix + new + suffix

    def test_repository_workflow_satisfies_contract(self) -> None:
        self.assertTrue(WORKFLOW_PATH.is_file(), "release.yml must exist")
        self.assertEqual([], contract.validate_workflow(WORKFLOW))

    def test_trigger_must_remain_tag_only_with_existing_tag_dispatch(self) -> None:
        mutated = self.mutate("    tags: ['v*']\n", "    branches: [main]\n")
        self.assert_rejected(mutated, "tag-only release trigger")

    def test_failed_run_recovery_binds_the_original_tag_push_and_source(self) -> None:
        source_sha = BETA4_SOURCE_SHA
        run_id = BETA4_FAILED_RUN_ID
        run = {
            "id": run_id,
            "name": "Release",
            "path": ".github/workflows/release.yml",
            "event": "push",
            "status": "completed",
            "conclusion": "failure",
            "head_branch": "v1.0.0-beta.4",
            "head_sha": source_sha,
            "run_attempt": 1,
        }
        jobs = {
            "total_count": 5,
            "jobs": [
                {
                    "name": "Validate immutable release source",
                    "run_id": run_id,
                    "run_attempt": 1,
                    "head_sha": source_sha,
                    "status": "completed",
                    "conclusion": "success",
                },
                {
                    "name": "Release quality gates",
                    "run_id": run_id,
                    "run_attempt": 1,
                    "head_sha": source_sha,
                    "status": "completed",
                    "conclusion": "success",
                },
                {
                    "name": "macOS ARM64 app and DMG",
                    "run_id": run_id,
                    "run_attempt": 1,
                    "head_sha": source_sha,
                    "status": "completed",
                    "conclusion": "success",
                },
                {
                    "name": "Windows x64 MSI and NSIS",
                    "run_id": run_id,
                    "run_attempt": 1,
                    "head_sha": source_sha,
                    "status": "completed",
                    "conclusion": "failure",
                    "steps": recovery_windows_steps(),
                },
                {
                    "name": "Publish verified GitHub prerelease",
                    "run_id": run_id,
                    "run_attempt": 1,
                    "head_sha": source_sha,
                    "status": "completed",
                    "conclusion": "skipped",
                },
            ],
        }
        compare = {
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "base_commit": {"sha": source_sha},
            "merge_base_commit": {"sha": source_sha},
            "commits": [{"sha": BETA4_RECOVERY1_TOOLING_SHA}],
        }

        contract.validate_recovery_run(
            run,
            jobs,
            compare,
            expected_run_id=run_id,
            expected_tag="v1.0.0-beta.4",
            expected_sha=source_sha,
            expected_comparison_head_sha=BETA4_RECOVERY1_TOOLING_SHA,
        )

        mutations = (
            ("run event", {**run, "event": "workflow_dispatch"}, jobs, compare),
            ("run source", {**run, "head_sha": "2" * 40}, jobs, compare),
            ("published result", {**run, "conclusion": "success"}, jobs, compare),
            (
                "validation gate",
                run,
                {
                    **jobs,
                    "jobs": [
                        {
                            "name": "Validate immutable release source",
                            "run_id": run_id,
                            "run_attempt": 1,
                            "head_sha": source_sha,
                            "status": "completed",
                            "conclusion": "failure",
                        },
                        *jobs["jobs"][1:],
                    ],
                },
                compare,
            ),
            (
                "main ancestry",
                run,
                jobs,
                {**compare, "merge_base_commit": {"sha": "3" * 40}},
            ),
            (
                "comparison head",
                run,
                jobs,
                {**compare, "commits": [{"sha": "4" * 40}]},
            ),
            (
                "missing comparison head",
                run,
                jobs,
                {key: value for key, value in compare.items() if key != "commits"},
            ),
            (
                "truncated comparison commits",
                run,
                jobs,
                {**compare, "ahead_by": 2, "total_commits": 2},
            ),
        )
        for name, mutated_run, mutated_jobs, mutated_compare in mutations:
            with self.subTest(name=name):
                with self.assertRaises(contract.RecoveryRunError):
                    contract.validate_recovery_run(
                        mutated_run,
                        mutated_jobs,
                        mutated_compare,
                        expected_run_id=run_id,
                        expected_tag="v1.0.0-beta.4",
                        expected_sha=source_sha,
                        expected_comparison_head_sha=BETA4_RECOVERY1_TOOLING_SHA,
                    )

    def test_failed_run_recovery_requires_the_exact_windows_failure_job_set(
        self,
    ) -> None:
        source_sha = BETA4_SOURCE_SHA
        run_id = BETA4_FAILED_RUN_ID
        run = {
            "id": run_id,
            "name": "Release",
            "path": ".github/workflows/release.yml",
            "event": "push",
            "status": "completed",
            "conclusion": "failure",
            "head_branch": "v1.0.0-beta.4",
            "head_sha": source_sha,
            "run_attempt": 1,
        }
        outcomes = {
            "Validate immutable release source": "success",
            "Release quality gates": "success",
            "macOS ARM64 app and DMG": "success",
            "Windows x64 MSI and NSIS": "failure",
            "Publish verified GitHub prerelease": "skipped",
        }

        def entry(name: str, conclusion: str) -> dict[str, object]:
            result: dict[str, object] = {
                "name": name,
                "run_id": run_id,
                "run_attempt": 1,
                "head_sha": source_sha,
                "status": "completed",
                "conclusion": conclusion,
            }
            if name == "Windows x64 MSI and NSIS":
                result["steps"] = recovery_windows_steps()
            return result

        exact = [entry(name, conclusion) for name, conclusion in outcomes.items()]
        compare = {
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "base_commit": {"sha": source_sha},
            "merge_base_commit": {"sha": source_sha},
            "commits": [{"sha": BETA4_RECOVERY1_TOOLING_SHA}],
        }
        invalid_job_sets = {
            "missing": exact[:-1],
            "extra": [*exact, entry("unexpected arbitrary job", "failure")],
            "duplicate": [*exact, exact[-1]],
            "quality failure": [
                entry(
                    name,
                    "failure" if name == "Release quality gates" else conclusion,
                )
                for name, conclusion in outcomes.items()
            ],
            "publish failure": [
                entry(
                    name,
                    "failure"
                    if name == "Publish verified GitHub prerelease"
                    else conclusion,
                )
                for name, conclusion in outcomes.items()
            ],
            "wrong run": [
                {**exact[0], "run_id": run_id + 1},
                *exact[1:],
            ],
            "wrong attempt": [
                {**exact[0], "run_attempt": 2},
                *exact[1:],
            ],
            "wrong source": [
                {**exact[0], "head_sha": "2" * 40},
                *exact[1:],
            ],
        }

        for name, entries in invalid_job_sets.items():
            with self.subTest(name=name):
                with self.assertRaises(contract.RecoveryRunError):
                    contract.validate_recovery_run(
                        run,
                        {"total_count": len(entries), "jobs": entries},
                        compare,
                        expected_run_id=run_id,
                        expected_tag="v1.0.0-beta.4",
                        expected_sha=source_sha,
                        expected_comparison_head_sha=BETA4_RECOVERY1_TOOLING_SHA,
                    )

    def test_failed_run_recovery_requires_the_exact_sidecar_failure_step(
        self,
    ) -> None:
        source_sha = BETA4_SOURCE_SHA
        run_id = BETA4_FAILED_RUN_ID
        run = {
            "id": run_id,
            "name": "Release",
            "path": ".github/workflows/release.yml",
            "event": "push",
            "status": "completed",
            "conclusion": "failure",
            "head_branch": "v1.0.0-beta.4",
            "head_sha": source_sha,
            "run_attempt": 1,
        }
        outcomes = (
            ("Validate immutable release source", "success"),
            ("Release quality gates", "success"),
            ("macOS ARM64 app and DMG", "success"),
            ("Windows x64 MSI and NSIS", "failure"),
            ("Publish verified GitHub prerelease", "skipped"),
        )

        def jobs_with_steps(steps: list[dict[str, object]]) -> dict[str, object]:
            entries = []
            for name, conclusion in outcomes:
                entry: dict[str, object] = {
                    "name": name,
                    "run_id": run_id,
                    "run_attempt": 1,
                    "head_sha": source_sha,
                    "status": "completed",
                    "conclusion": conclusion,
                }
                if name == "Windows x64 MSI and NSIS":
                    entry["steps"] = steps
                entries.append(entry)
            return {"total_count": len(entries), "jobs": entries}

        compare = {
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "base_commit": {"sha": source_sha},
            "merge_base_commit": {"sha": source_sha},
            "commits": [{"sha": BETA4_RECOVERY1_TOOLING_SHA}],
        }
        exact_steps = recovery_windows_steps()
        mutations: dict[str, list[dict[str, object]]] = {
            "missing failure step": [
                step
                for step in exact_steps
                if step["name"]
                != "Provision checksum-pinned Windows FFmpeg sidecars"
            ],
            "extra step": [
                *recovery_windows_steps(),
                {
                    "name": "unexpected arbitrary step",
                    "status": "completed",
                    "conclusion": "failure",
                },
            ],
            "wrong failure step number": [
                {
                    **step,
                    "number": 10
                    if step["name"]
                    == "Provision checksum-pinned Windows FFmpeg sidecars"
                    else step["number"],
                }
                for step in recovery_windows_steps()
            ],
            "workspace test failure": [
                {
                    **step,
                    "status": "completed",
                    "conclusion": (
                        "success"
                        if step["name"]
                        == "Provision checksum-pinned Windows FFmpeg sidecars"
                        else "failure"
                        if step["name"] == "Rust workspace tests"
                        else step["conclusion"]
                    ),
                }
                for step in recovery_windows_steps()
            ],
            "installer failure": [
                {
                    **step,
                    "status": "completed",
                    "conclusion": (
                        "success"
                        if step["name"]
                        == "Provision checksum-pinned Windows FFmpeg sidecars"
                        else "failure"
                        if step["name"]
                        == "Build native MSI, NSIS, and signed updater artifacts"
                        else step["conclusion"]
                    ),
                }
                for step in recovery_windows_steps()
            ],
        }

        for name, steps in mutations.items():
            with self.subTest(name=name):
                with self.assertRaises(contract.RecoveryRunError):
                    contract.validate_recovery_run(
                        run,
                        jobs_with_steps(steps),
                        compare,
                        expected_run_id=run_id,
                        expected_tag="v1.0.0-beta.4",
                        expected_sha=source_sha,
                        expected_comparison_head_sha=BETA4_RECOVERY1_TOOLING_SHA,
                    )

    def test_second_recovery_run_is_an_exact_fail_closed_chain_link(self) -> None:
        run = {
            "id": BETA4_RECOVERY2_RUN_ID,
            "workflow_id": 330325373,
            "name": "Release",
            "path": ".github/workflows/release.yml",
            "event": "workflow_dispatch",
            "status": "completed",
            "conclusion": "failure",
            "head_branch": "main",
            "head_sha": BETA4_RECOVERY1_TOOLING_SHA,
            "run_attempt": 1,
            "repository": {
                "id": 1275692189,
                "full_name": "appergb/OpenTake",
                "private": False,
            },
            "head_repository": {
                "id": 1275692189,
                "full_name": "appergb/OpenTake",
                "private": False,
            },
        }
        outcomes = (
            ("Validate immutable release source", "success"),
            ("Release quality gates", "success"),
            ("macOS ARM64 app and DMG", "success"),
            ("Windows x64 MSI and NSIS", "failure"),
            ("Publish verified GitHub prerelease", "skipped"),
        )

        def jobs_with_steps(steps: list[dict[str, object]]) -> dict[str, object]:
            entries = []
            for name, conclusion in outcomes:
                entry: dict[str, object] = {
                    "name": name,
                    "run_id": BETA4_RECOVERY2_RUN_ID,
                    "run_attempt": 1,
                    "head_sha": BETA4_RECOVERY1_TOOLING_SHA,
                    "status": "completed",
                    "conclusion": conclusion,
                }
                if name == "Windows x64 MSI and NSIS":
                    entry["steps"] = steps
                entries.append(entry)
            return {"total_count": len(entries), "jobs": entries}

        exact_jobs = jobs_with_steps(recovery2_windows_steps())
        exact_artifacts = recovery_artifacts(
            BETA4_RECOVERY2_RUN_ID, "main", BETA4_RECOVERY1_TOOLING_SHA
        )
        run_comparison = recovery_comparison(
            BETA4_RECOVERY1_TOOLING_SHA,
            BETA4_NEXT_TOOLING_SHA,
        )

        contract.validate_failed_recovery_run(
            run,
            exact_jobs,
            exact_artifacts,
            run_comparison,
            expected_run_id=BETA4_RECOVERY2_RUN_ID,
            expected_tag="v1.0.0-beta.4",
            expected_source_sha=BETA4_SOURCE_SHA,
            expected_tooling_sha=BETA4_RECOVERY1_TOOLING_SHA,
            expected_current_tooling_sha=BETA4_NEXT_TOOLING_SHA,
        )

        moved_failure_steps = [
            {
                **step,
                "conclusion": (
                    "success"
                    if step["name"]
                    == "Build native MSI, NSIS, and signed updater artifacts"
                    else "failure"
                    if step["name"] == "Rust workspace tests"
                    else step["conclusion"]
                ),
            }
            for step in recovery2_windows_steps()
        ]
        artifact = exact_artifacts["artifacts"][0]
        assert isinstance(artifact, dict)
        artifact_run = artifact["workflow_run"]
        assert isinstance(artifact_run, dict)
        job_entries = exact_jobs["jobs"]
        assert isinstance(job_entries, list)
        run_repository = run["repository"]
        run_head_repository = run["head_repository"]
        assert isinstance(run_repository, dict)
        assert isinstance(run_head_repository, dict)
        mutations = (
            (
                "wrong dispatcher head",
                {**run, "head_sha": "3" * 40},
                exact_jobs,
                exact_artifacts,
                run_comparison,
            ),
            (
                "wrong workflow ID",
                {**run, "workflow_id": 1},
                exact_jobs,
                exact_artifacts,
                run_comparison,
            ),
            (
                "wrong repository",
                {**run, "repository": {**run_repository, "id": 1}},
                exact_jobs,
                exact_artifacts,
                run_comparison,
            ),
            (
                "non-boolean repository visibility",
                {**run, "repository": {**run_repository, "private": 0}},
                exact_jobs,
                exact_artifacts,
                run_comparison,
            ),
            (
                "wrong head repository",
                {**run, "head_repository": {**run_head_repository, "id": 1}},
                exact_jobs,
                exact_artifacts,
                run_comparison,
            ),
            (
                "wrong run attempt",
                {**run, "run_attempt": 2},
                exact_jobs,
                exact_artifacts,
                run_comparison,
            ),
            (
                "boolean run attempt",
                {**run, "run_attempt": True},
                exact_jobs,
                exact_artifacts,
                run_comparison,
            ),
            (
                "missing job",
                run,
                {"total_count": 4, "jobs": job_entries[:-1]},
                exact_artifacts,
                run_comparison,
            ),
            (
                "extra job",
                run,
                {
                    "total_count": 6,
                    "jobs": [
                        *job_entries,
                        {
                            "name": "unexpected job",
                            "run_id": BETA4_RECOVERY2_RUN_ID,
                            "run_attempt": 1,
                            "head_sha": BETA4_RECOVERY1_TOOLING_SHA,
                            "status": "completed",
                            "conclusion": "failure",
                        },
                    ],
                },
                exact_artifacts,
                run_comparison,
            ),
            (
                "boolean job attempt",
                run,
                {
                    **exact_jobs,
                    "jobs": [
                        {**job_entries[0], "run_attempt": True},
                        *job_entries[1:],
                    ],
                },
                exact_artifacts,
                run_comparison,
            ),
            (
                "failure moved to workspace tests",
                run,
                jobs_with_steps(moved_failure_steps),
                exact_artifacts,
                run_comparison,
            ),
            (
                "boolean Windows step number",
                run,
                jobs_with_steps(
                    [
                        {**recovery2_windows_steps()[0], "number": True},
                        *recovery2_windows_steps()[1:],
                    ]
                ),
                exact_artifacts,
                run_comparison,
            ),
            (
                "artifact ID",
                run,
                exact_jobs,
                {
                    **exact_artifacts,
                    "artifacts": [{**artifact, "id": 9077851537}],
                },
                run_comparison,
            ),
            (
                "artifact name",
                run,
                exact_jobs,
                {
                    **exact_artifacts,
                    "artifacts": [{**artifact, "name": "unbound-artifact"}],
                },
                run_comparison,
            ),
            (
                "artifact size drift",
                run,
                exact_jobs,
                {
                    **exact_artifacts,
                    "artifacts": [{**artifact, "size_in_bytes": 142001291}],
                },
                run_comparison,
            ),
            (
                "artifact digest",
                run,
                exact_jobs,
                {
                    **exact_artifacts,
                    "artifacts": [{**artifact, "digest": "sha256:" + "0" * 64}],
                },
                run_comparison,
            ),
            (
                "artifact repository",
                run,
                exact_jobs,
                {
                    **exact_artifacts,
                    "artifacts": [
                        {
                            **artifact,
                            "workflow_run": {
                                **artifact_run,
                                "repository_id": 1,
                            },
                        }
                    ],
                },
                run_comparison,
            ),
            (
                "artifact head",
                run,
                exact_jobs,
                {
                    **exact_artifacts,
                    "artifacts": [
                        {
                            **artifact,
                            "workflow_run": {
                                **artifact_run,
                                "head_sha": BETA4_SOURCE_SHA,
                            },
                        }
                    ],
                },
                run_comparison,
            ),
            (
                "missing artifact payload",
                run,
                exact_jobs,
                {"total_count": 0, "artifacts": []},
                run_comparison,
            ),
            (
                "boolean artifact count",
                run,
                exact_jobs,
                {**exact_artifacts, "total_count": True},
                run_comparison,
            ),
            (
                "malformed artifact payload",
                run,
                exact_jobs,
                {"total_count": 1, "artifacts": ["not-an-object"]},
                run_comparison,
            ),
            (
                "expired artifact",
                run,
                exact_jobs,
                {
                    **exact_artifacts,
                    "artifacts": [{**artifact, "expired": True}],
                },
                run_comparison,
            ),
            (
                "non-boolean artifact expiry",
                run,
                exact_jobs,
                {
                    **exact_artifacts,
                    "artifacts": [{**artifact, "expired": 0}],
                },
                run_comparison,
            ),
            (
                "empty artifact",
                run,
                exact_jobs,
                {
                    **exact_artifacts,
                    "artifacts": [{**artifact, "size_in_bytes": 0}],
                },
                run_comparison,
            ),
            (
                "artifact run",
                run,
                exact_jobs,
                {
                    **exact_artifacts,
                    "artifacts": [
                        {
                            **artifact,
                            "workflow_run": {
                                **artifact_run,
                                "id": BETA4_FAILED_RUN_ID,
                            },
                        }
                    ],
                },
                run_comparison,
            ),
            (
                "prior tooling ancestry",
                run,
                exact_jobs,
                exact_artifacts,
                {
                    **run_comparison,
                    "merge_base_commit": {"sha": BETA4_SOURCE_SHA},
                },
            ),
            (
                "current tooling comparison head",
                run,
                exact_jobs,
                exact_artifacts,
                {
                    **run_comparison,
                    "commits": [{"sha": "4" * 40}],
                },
            ),
            (
                "truncated tooling comparison commits",
                run,
                exact_jobs,
                exact_artifacts,
                {
                    **run_comparison,
                    "ahead_by": 2,
                    "total_commits": 2,
                },
            ),
            (
                "boolean comparison counts",
                run,
                exact_jobs,
                exact_artifacts,
                {
                    **run_comparison,
                    "ahead_by": True,
                    "behind_by": False,
                },
            ),
        )
        for name, mutated_run, mutated_jobs, mutated_artifacts, mutated_compare in mutations:
            with self.subTest(name=name):
                with self.assertRaises(contract.RecoveryRunError):
                    contract.validate_failed_recovery_run(
                        mutated_run,
                        mutated_jobs,
                        mutated_artifacts,
                        mutated_compare,
                        expected_run_id=BETA4_RECOVERY2_RUN_ID,
                        expected_tag="v1.0.0-beta.4",
                        expected_source_sha=BETA4_SOURCE_SHA,
                        expected_tooling_sha=BETA4_RECOVERY1_TOOLING_SHA,
                        expected_current_tooling_sha=BETA4_NEXT_TOOLING_SHA,
                    )

    def test_recovery_rejects_an_unlisted_failed_run(self) -> None:
        with self.assertRaises(contract.RecoveryRunError):
            contract.validate_failed_recovery_run(
                {
                    "id": 99999999999,
                    "name": "Release",
                    "path": ".github/workflows/release.yml",
                    "event": "workflow_dispatch",
                    "status": "completed",
                    "conclusion": "failure",
                    "head_branch": "main",
                    "head_sha": BETA4_RECOVERY1_TOOLING_SHA,
                    "run_attempt": 1,
                },
                {"total_count": 0, "jobs": []},
                {"total_count": 0, "artifacts": []},
                recovery_comparison(
                    BETA4_RECOVERY1_TOOLING_SHA,
                    BETA4_NEXT_TOOLING_SHA,
                ),
                expected_run_id=99999999999,
                expected_tag="v1.0.0-beta.4",
                expected_source_sha=BETA4_SOURCE_SHA,
                expected_tooling_sha=BETA4_RECOVERY1_TOOLING_SHA,
                expected_current_tooling_sha=BETA4_NEXT_TOOLING_SHA,
            )

    def test_dispatch_contract_requires_root_and_predecessor_run_ids(self) -> None:
        document = contract._parse_workflow_yaml(WORKFLOW)
        inputs = document["on"]["workflow_dispatch"]["inputs"]
        self.assertEqual(
            {"tag", "failed_run_id", "failed_recovery_run_id"},
            set(inputs),
        )
        self.assertTrue(inputs["failed_run_id"]["required"])
        self.assertEqual(
            "Root failed tag-push Release run ID (31412976593) for the immutable source",
            inputs["failed_run_id"]["description"],
        )
        self.assertEqual(
            {
                "description": "Previous failed workflow_dispatch recovery Release run ID (31427093503) chained to the same immutable source",
                "required": True,
                "type": "string",
            },
            inputs["failed_recovery_run_id"],
        )

        mutated = self.mutate(
            "      failed_recovery_run_id:\n"
            "        description: Previous failed workflow_dispatch recovery Release run ID (31427093503) chained to the same immutable source\n"
            "        required: true\n"
            "        type: string\n",
            "      failed_recovery_run_id:\n"
            "        description: Optional arbitrary run\n"
            "        required: false\n"
            "        type: string\n",
        )
        self.assert_rejected(mutated, "tag-only release trigger")

    def test_contract_paths_can_be_bound_to_an_external_workflow_copy(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            workflow = root / "release.yml"
            notes = root / "release-notes.md"
            with mock.patch.dict(
                os.environ,
                {
                    "OPENTAKE_REPOSITORY_ROOT": str(root),
                    "OPENTAKE_RELEASE_WORKFLOW_PATH": str(workflow),
                    "OPENTAKE_RELEASE_NOTES_PATH": str(notes),
                },
            ):
                isolated_spec = importlib.util.spec_from_file_location(
                    "isolated_check_release_workflow",
                    Path(contract.__file__).resolve(),
                )
                assert isolated_spec is not None and isolated_spec.loader is not None
                isolated = importlib.util.module_from_spec(isolated_spec)
                isolated_spec.loader.exec_module(isolated)

            self.assertEqual(isolated.REPOSITORY_ROOT, root)
            self.assertEqual(isolated.WORKFLOW_PATH, workflow)
            self.assertEqual(
                isolated.RELEASE_NOTES_PATH,
                notes,
            )

    def test_dispatch_recovery_cannot_bypass_failed_run_provenance(self) -> None:
        mutations = (
            (
                '      RELEASE_TOOLING_SHA: ${{ github.workflow_sha }}\n',
                '      RELEASE_TOOLING_SHA: ${{ github.sha }}\n',
            ),
            (
                '            test "$tooling_sha" = "$remote_main"\n',
                '            test -n "$tooling_sha"\n',
            ),
            (
                '            test "$FAILED_RUN_ID" = "31412976593"\n',
                '            test -n "$FAILED_RUN_ID"\n',
            ),
            (
                '            test "$FAILED_RECOVERY_RUN_ID" = "31427093503"\n',
                '            test -n "$FAILED_RECOVERY_RUN_ID"\n',
            ),
            (
                '              --run-id "$FAILED_RUN_ID" \\\n',
                '              --run-id 31412976593 \\\n',
            ),
            (
                '            gh api "repos/$GITHUB_REPOSITORY/compare/$source_sha...$predecessor_tooling_sha" \\\n',
                '            gh api "repos/$GITHUB_REPOSITORY/compare/$source_sha...$remote_main" \\\n',
            ),
            (
                '            gh api "repos/$GITHUB_REPOSITORY/compare/$predecessor_tooling_sha...$remote_main" \\\n',
                '            gh api "repos/$GITHUB_REPOSITORY/compare/$remote_main...$remote_main" \\\n',
            ),
            (
                '            gh api "repos/$GITHUB_REPOSITORY/actions/runs/$FAILED_RECOVERY_RUN_ID/artifacts?per_page=100" \\\n',
                '            gh api "repos/$GITHUB_REPOSITORY/actions/runs/$FAILED_RECOVERY_RUN_ID/artifacts?per_page=1" \\\n',
            ),
            (
                '      FAILED_RECOVERY_RUN_ID: ${{ github.event_name == \'workflow_dispatch\' && inputs.failed_recovery_run_id || \'\' }}\n',
                '      FAILED_RECOVERY_RUN_ID: 31427093503\n',
            ),
        )
        for old, new in mutations:
            with self.subTest(mutation=old.strip()):
                self.assert_rejected(
                    self.mutate(old, new), "failed-run recovery provenance"
                )

    def test_windows_recovery_tooling_is_bound_to_the_workflow_commit(self) -> None:
        mutations = (
            (
                'git cat-file blob "$($env:RELEASE_TOOLING_SHA):scripts/provision_ffmpeg_sidecars.py" > $provisioner',
                'Get-Content scripts/provision_ffmpeg_sidecars.py | Set-Content $provisioner',
            ),
            (
                'git cat-file -e "$($env:RELEASE_TOOLING_SHA)^{commit}" 2>$null',
                '$true',
            ),
            (
                'git fetch --no-tags --depth=1 origin $env:RELEASE_TOOLING_SHA',
                'git fetch --no-tags --depth=1 origin main',
            ),
        )
        for old, new in mutations:
            with self.subTest(mutation=old):
                self.assert_rejected(
                    self.mutate(old, new), "exact release tooling provenance"
                )

    def test_recovery_tooling_never_trusts_raw_github_downloads(self) -> None:
        mutations = (
            (
                'git cat-file blob "$tooling_sha:scripts/check_release_workflow.py" \\\n',
                'curl --fail "https://raw.githubusercontent.com/appergb/OpenTake/$tooling_sha/scripts/check_release_workflow.py" \\\n',
            ),
            (
                'git cat-file blob "$RELEASE_TOOLING_SHA:scripts/test_check_release_workflow.py" \\\n',
                'curl --fail "https://raw.githubusercontent.com/appergb/OpenTake/$RELEASE_TOOLING_SHA/scripts/test_check_release_workflow.py" \\\n',
            ),
            (
                'git cat-file blob "$($env:RELEASE_TOOLING_SHA):scripts/provision_ffmpeg_sidecars.py" > $provisioner',
                'Invoke-WebRequest "https://raw.githubusercontent.com/appergb/OpenTake/$env:RELEASE_TOOLING_SHA/scripts/provision_ffmpeg_sidecars.py" -OutFile $provisioner',
            ),
        )
        for old, new in mutations:
            with self.subTest(mutation=old):
                self.assert_rejected(
                    self.mutate(old, new),
                    "release tooling never trusts raw HTTP downloads",
                )

    def test_recovery_runs_the_exact_tooling_provisioner_tests(self) -> None:
        mutated = self.mutate(
            '            -s "$tooling_root" -p \'test_provision_ffmpeg_sidecars.py\'\n',
            '            -s scripts/tests -p \'test_*.py\'\n',
        )
        self.assert_rejected(mutated, "recovery provisioner tests")

    def test_recovery_release_notes_are_loaded_from_exact_tooling_commit(
        self,
    ) -> None:
        mutations = (
            (
                'git cat-file blob "$RELEASE_TOOLING_SHA:docs/releases/1.0.0-beta.4.md" \\\n',
                'cp docs/releases/1.0.0-beta.4.md \\\n',
                "exact release tooling provenance",
            ),
            (
                'git cat-file blob "$RELEASE_TOOLING_SHA:$NOTES_PATH" \\\n',
                'cp "$NOTES_PATH" \\\n',
                "recovery release notes use exact tooling commit",
            ),
        )
        for old, new, expected in mutations:
            with self.subTest(mutation=old):
                self.assert_rejected(self.mutate(old, new), expected)

    def test_trigger_rejects_any_additional_event(self) -> None:
        mutated = self.mutate(
            "  workflow_dispatch:\n",
            "  pull_request:\n  workflow_dispatch:\n",
        )
        self.assert_rejected(mutated, "exact release event set")

    def test_validation_must_bind_tag_to_current_remote_main(self) -> None:
        mutated = self.mutate(
            "git ls-remote --exit-code origin refs/heads/main",
            "git rev-parse origin/main",
        )
        self.assert_rejected(mutated, "tag commit equals current remote main HEAD")

    def test_validation_must_compare_all_public_versions(self) -> None:
        mutated = self.mutate(
            'web = json.loads(Path("web/package.json").read_text(encoding="utf-8"))',
            'web = {"version": version}',
        )
        self.assert_rejected(mutated, "Cargo, Tauri, and Web versions match tag")

    def test_validation_must_bind_windows_installer_version(self) -> None:
        mutated = self.mutate(
            'if wix_version != "1.0.0.4":',
            'if wix_version != "1.0.0.3":',
        )
        self.assert_rejected(mutated, "Cargo, Tauri, and Web versions match tag")

    def test_release_tag_must_reject_semver_build_metadata(self) -> None:
        guard = '          if "+" in tag:\n'
        self.assertEqual(1, WORKFLOW.count(guard))
        mutated = self.mutate(guard, '          if False:\n')
        self.assert_rejected(mutated, "SemVer build metadata is unsupported")

    def test_publish_must_depend_on_every_gate(self) -> None:
        mutated = self.mutate(
            "needs: [validate, quality, macos_arm64, windows_x64]",
            "needs: [validate, macos_arm64, windows_x64]",
        )
        self.assert_rejected(mutated, "publish depends on every required job")

    def test_release_assets_must_have_exact_platform_counts(self) -> None:
        mutated = self.mutate(
            'test "${#dmgs[@]}" -eq 1',
            'test "${#dmgs[@]}" -ge 1',
        )
        self.assert_rejected(mutated, "strict release asset counts")

    def test_both_build_jobs_require_private_key_and_password_secrets(self) -> None:
        private_key = (
            "          TAURI_SIGNING_PRIVATE_KEY: "
            "${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}\n"
        )
        password = (
            "          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "
            "${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}\n"
        )
        self.assertEqual(6, WORKFLOW.count(private_key))
        self.assertEqual(6, WORKFLOW.count(password))
        for old, new in (
            (private_key, "          TAURI_SIGNING_PRIVATE_KEY: ''\n"),
            (password, "          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ''\n"),
        ):
            self.assert_rejected(
                self.mutate(old, new), "updater signing secrets fail closed"
            )

    def test_updater_signing_secret_guard_checks_nonempty_values(self) -> None:
        guard = (
            '          test -n "${TAURI_SIGNING_PRIVATE_KEY:-}"\n'
            '          test -n "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"\n'
        )
        self.assertEqual(2, WORKFLOW.count(guard))
        mutated = self.mutate(
            '          test -n "${TAURI_SIGNING_PRIVATE_KEY:-}"\n',
            '          test -z "${TAURI_SIGNING_PRIVATE_KEY:-}"\n',
        )
        self.assert_rejected(mutated, "updater signing secrets fail closed")

    def test_bundlers_cannot_disable_signed_tauri_v2_updater_artifacts(self) -> None:
        self.assertEqual(2, WORKFLOW.count('"createUpdaterArtifacts":true'))
        mutated = self.mutate(
            '"createUpdaterArtifacts":true', '"createUpdaterArtifacts":false'
        )
        self.assert_rejected(mutated, "signed Tauri v2 updater bundles")

    def test_platform_uploads_require_exact_updater_packages_and_signatures(self) -> None:
        for path in (
            "target/aarch64-apple-darwin/release/bundle/macos/*.app.tar.gz\n",
            "target/aarch64-apple-darwin/release/bundle/macos/*.app.tar.gz.sig\n",
            "target/aarch64-apple-darwin/release/bundle/macos/*.app.tar.gz.attestation.json\n",
            "target/aarch64-apple-darwin/release/bundle/macos/*.app.tar.gz.attestation.json.sig\n",
            "target/release/bundle/msi/*.msi.sig\n",
            "target/release/bundle/msi/*.msi.attestation.json\n",
            "target/release/bundle/msi/*.msi.attestation.json.sig\n",
            "target/release/bundle/nsis/*.exe.sig\n",
            "target/release/bundle/nsis/*.exe.attestation.json\n",
            "target/release/bundle/nsis/*.exe.attestation.json.sig\n",
        ):
            with self.subTest(path=path.strip()):
                self.assert_rejected(
                    self.mutate(f"            {path}", ""),
                    "exact signed updater artifact uploads",
                )

    def test_publish_generates_only_the_fixed_repo_exact_tag_manifest(self) -> None:
        mutations = (
            ("--repository appergb/OpenTake", "--repository attacker/OpenTake"),
            (
                '--tag "$RELEASE_TAG" \\\n            --version "$RELEASE_VERSION"',
                '--tag "v1.0.0-beta.2" \\\n            --version "$RELEASE_VERSION"',
            ),
            ('--version "$RELEASE_VERSION"', '--version "1.0.0-beta.2"'),
            (
                '--source-sha "$RELEASE_SHA"',
                '--source-sha "0000000000000000000000000000000000000000"',
            ),
            (
                '--output "$PUBLISH_ROOT/assets/updater-$RELEASE_TAG.json"',
                '--output "$PUBLISH_ROOT/assets/latest.json"',
            ),
            ("--darwin-artifact \"${mac_updaters[0]}\"", "--darwin-artifact \"${dmgs[0]}\""),
            ("--windows-msi-artifact \"${msi_installers[0]}\"", "--windows-msi-artifact \"${nsis_installers[0]}\""),
            ("--windows-nsis-artifact \"${nsis_installers[0]}\"", "--windows-nsis-artifact \"${msi_installers[0]}\""),
            (
                '--darwin-attestation "${mac_attestations[0]}"',
                '--darwin-attestation "${mac_updaters[0]}"',
            ),
            (
                '--windows-msi-attestation "${msi_attestations[0]}"',
                '--windows-msi-attestation "${msi_installers[0]}"',
            ),
            (
                '--windows-nsis-attestation "${nsis_attestations[0]}"',
                '--windows-nsis-attestation "${nsis_installers[0]}"',
            ),
        )
        for old, new in mutations:
            with self.subTest(mutation=old):
                self.assert_rejected(
                    self.mutate_last(old, new),
                    "fixed HTTPS exact-tag updater manifest",
                )

    def test_staged_payload_requires_exact_updater_asset_and_signature_counts(self) -> None:
        mutations = (
            ('test "${#mac_updaters[@]}" -eq 1', 'test "${#mac_updaters[@]}" -ge 1'),
            ('test "${#mac_signatures[@]}" -eq 1', 'test "${#mac_signatures[@]}" -ge 1'),
            ('test "${#windows_signatures[@]}" -eq 2', 'test "${#windows_signatures[@]}" -ge 1'),
            ('test "${#mac_attestations[@]}" -eq 1', 'test "${#mac_attestations[@]}" -ge 1'),
            ('test "${#msi_attestations[@]}" -eq 1', 'test "${#msi_attestations[@]}" -ge 1'),
            ('test "${#nsis_attestations[@]}" -eq 1', 'test "${#nsis_attestations[@]}" -ge 1'),
            ('test "${#manifests[@]}" -eq 1', 'test "${#manifests[@]}" -ge 1'),
        )
        for old, new in mutations:
            with self.subTest(mutation=old):
                self.assert_rejected(
                    self.mutate(old, new), "strict signed updater asset counts"
                )

    def test_updater_private_key_cannot_reach_receipts_or_artifacts(self) -> None:
        marker = "          RECEIPT_SHA: ${{ needs.validate.outputs.source_sha }}\n"
        leaked = marker + (
            "          LEAKED_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}\n"
        )
        self.assert_rejected(
            self.mutate(marker, leaked),
            "updater signing secrets limited to guard and build steps",
        )
        upload_path = "            macos-arm64-receipt.json\n"
        self.assert_rejected(
            self.mutate(
                upload_path,
                upload_path + "            target/**/*.key\n",
            ),
            "private updater signing material is never uploaded",
        )

    def test_receipts_and_manifest_cross_check_every_updater_file(self) -> None:
        mutated = self.mutate(
            'if entry.get("sha256") != sha256(artifact):',
            'if entry.get("sha256") == "":',
        )
        self.assert_rejected(
            mutated, "updater receipts, digests, sizes, and signatures agree"
        )

    def test_every_updater_signature_is_verified_with_the_embedded_public_key(self) -> None:
        mutations = (
            (
                "sudo apt-get install --yes --no-install-recommends minisign",
                "true # minisign verifier bypassed",
            ),
            (
                'pubkey = config["plugins"]["updater"]["pubkey"]',
                'pubkey = "attacker-controlled-key"',
            ),
            (
                'minisign -Vm "${mac_updaters[0]}"',
                'true # macOS updater signature bypassed',
            ),
            (
                'minisign -Vm "${msis[0]}"',
                'true # MSI updater signature bypassed',
            ),
            (
                'minisign -Vm "${exes[0]}"',
                'true # NSIS updater signature bypassed',
            ),
            (
                'minisign -Vm "${mac_attestations[0]}"',
                'true # macOS attestation signature bypassed',
            ),
            (
                'minisign -Vm "${msi_attestations[0]}"',
                'true # MSI attestation signature bypassed',
            ),
            (
                'minisign -Vm "${nsis_attestations[0]}"',
                'true # NSIS attestation signature bypassed',
            ),
        )
        for old, new in mutations:
            with self.subTest(mutation=old):
                self.assert_rejected(
                    self.mutate(old, new),
                    "updater signatures verify against embedded public key",
                )

    def test_build_jobs_create_and_sign_release_bound_attestations(self) -> None:
        mutations = (
            ("--platform darwin-aarch64", "--platform darwin-x86_64"),
            ("--platform windows-x86_64-msi", "--platform windows-aarch64-msi"),
            ("--platform windows-x86_64-nsis", "--platform windows-aarch64-nsis"),
            (
                '--source-sha "$TARGET_SHA"',
                '--source-sha "0000000000000000000000000000000000000000"',
            ),
            (
                './web/node_modules/.bin/tauri signer sign "$attestation"',
                'echo "attestation signing bypassed"',
            ),
            (
                '& .\\web\\node_modules\\.bin\\tauri.cmd signer sign $msiAttestation',
                "Write-Output 'MSI attestation signing bypassed'",
            ),
            (
                '& .\\web\\node_modules\\.bin\\tauri.cmd signer sign $nsisAttestation',
                "Write-Output 'NSIS attestation signing bypassed'",
            ),
        )
        for old, new in mutations:
            with self.subTest(mutation=old):
                self.assert_rejected(
                    self.mutate(old, new),
                    "signed updater attestations bind release identity and payload",
                )

    def test_publish_rejects_forged_attestation_fields_or_payload_hash(self) -> None:
        mutations = (
            ('"schemaVersion": 1,', '"schemaVersion": 2,'),
            (
                'if attestation.get("sha256") != sha256(artifact):',
                'if attestation.get("sha256") == "":',
            ),
            (
                'if attestation.get("sourceSha") != source_sha:',
                'if not attestation.get("sourceSha"):',
            ),
        )
        for old, new in mutations:
            with self.subTest(mutation=old):
                self.assert_rejected(
                    self.mutate_last(old, new),
                    "attestations bind exact release identity and updater bytes",
                )

    def test_release_must_be_draft_before_upload_and_publish(self) -> None:
        mutated = self.mutate(
            "--draft --prerelease --latest=false",
            "--prerelease --latest=false",
        )
        self.assert_rejected(mutated, "draft exists before release upload")

    def test_release_must_generate_and_verify_sha256sums(self) -> None:
        mutated = self.mutate(
            'sha256sum "${payload_names[@]}" > SHA256SUMS',
            'md5sum "${payload_names[@]}" > SHA256SUMS',
        )
        self.assert_rejected(mutated, "SHA256SUMS covers and verifies every payload")

    def test_draft_assets_are_downloaded_and_verified_before_publication(self) -> None:
        mutations = (
            (
                'gh release download "$RELEASE_TAG" --dir "$PUBLISH_ROOT/verified-draft"',
                'true # draft download bypassed',
            ),
            (
                'if remote_sizes != local_sizes:',
                'if set(remote_sizes) != set(local_sizes):',
            ),
            (
                'cmp "$PUBLISH_ROOT/assets/SHA256SUMS" "$PUBLISH_ROOT/verified-draft/SHA256SUMS"',
                'true # draft checksum asset comparison bypassed',
            ),
            (
                'cd "$PUBLISH_ROOT/verified-draft"\n'
                '          sha256sum --check SHA256SUMS',
                'cd "$PUBLISH_ROOT/verified-draft"\n'
                '          sha256sum --status SHA256SUMS || true',
            ),
        )
        for old, new in mutations:
            with self.subTest(mutation=old):
                self.assert_rejected(
                    self.mutate(old, new),
                    "draft assets verified by exact name, size, and SHA-256",
                )

    def test_required_work_cannot_ignore_failures(self) -> None:
        mutated = self.mutate(
            "      - name: Rust workspace clippy\n",
            "      - name: Rust workspace clippy\n        continue-on-error: true\n",
        )
        self.assert_rejected(mutated, "no ignored failures")

    def test_required_protection_step_cannot_be_skipped(self) -> None:
        mutated = self.mutate(
            "      - name: Create or refresh draft prerelease\n",
            "      - name: Create or refresh draft prerelease\n        if: false\n",
        )
        self.assert_rejected(mutated, "required steps are unconditional")

    def test_build_jobs_must_checkout_the_validated_exact_sha(self) -> None:
        mutated = self.mutate(
            "          ref: ${{ needs.validate.outputs.source_sha }}\n",
            "          ref: main\n",
        )
        self.assert_rejected(mutated, "every gate checks out the validated SHA")

    def test_every_release_job_must_have_exactly_one_checkout(self) -> None:
        jobs = ("validate", "quality", "macos_arm64", "windows_x64", "publish")
        for index, job_name in enumerate(jobs):
            with self.subTest(job=job_name):
                start = WORKFLOW.index(f"  {job_name}:\n")
                stop = (
                    WORKFLOW.index(f"\n  {jobs[index + 1]}:\n", start)
                    if index + 1 < len(jobs)
                    else len(WORKFLOW)
                )
                duplicate = (
                    "\n      - uses: actions/checkout@"
                    f"{CHECKOUT_SHA}\n"
                    "        with:\n"
                    "          ref: main\n"
                    "          fetch-depth: 0\n"
                    "          persist-credentials: false\n"
                )
                mutated = WORKFLOW[:stop] + duplicate + WORKFLOW[stop:]
                self.assert_rejected(
                    mutated,
                    "exactly one ordered checkout and SHA assertion per release job",
                )

    def test_checkout_cannot_run_after_the_sha_assertion(self) -> None:
        quality_start = WORKFLOW.index("  quality:\n")
        checkout_start = WORKFLOW.index(
            f"      - uses: actions/checkout@{CHECKOUT_SHA} # v4\n", quality_start
        )
        assertion_start = WORKFLOW.index("      - name: Assert exact checked-out SHA\n", checkout_start)
        assertion_stop = WORKFLOW.index("      - name: Free disk space\n", assertion_start)
        checkout = WORKFLOW[checkout_start:assertion_start]
        assertion = WORKFLOW[assertion_start:assertion_stop]
        mutated = (
            WORKFLOW[:checkout_start]
            + assertion
            + checkout
            + WORKFLOW[assertion_stop:]
        )
        self.assert_rejected(
            mutated, "exactly one ordered checkout and SHA assertion per release job"
        )

    def test_published_release_cannot_be_mutated_by_rerun(self) -> None:
        mutated = self.mutate(
            "resolve-release-state",
            "trust-release-state",
        )
        self.assert_rejected(mutated, "published same-tag release is immutable")

    def test_final_api_verification_must_bind_target_sha(self) -> None:
        mutated = self.mutate_last(
            'release.get("target_commitish") != expected_sha',
            'release.get("target_commitish") == ""',
        )
        self.assert_rejected(mutated, "final API verification binds target SHA")

    def test_draft_lookup_uses_graphql_not_the_rest_tag_endpoint(self) -> None:
        draft_start = WORKFLOW.index("      - name: Create or refresh draft prerelease\n")
        upload_start = WORKFLOW.index("      - name: Upload the exact payload to the draft\n")
        draft_step = WORKFLOW[draft_start:upload_start]
        self.assertIn("gh api graphql", draft_step)
        self.assertNotIn("releases/tags/$RELEASE_TAG", draft_step)
        self.assertIn("resolve-release-state", draft_step)

    def test_remote_tag_is_revalidated_before_draft_and_publication(self) -> None:
        first = self.mutate(
            "      - name: Revalidate remote tag before draft mutation\n",
            "      - name: Remote tag check removed before draft mutation\n",
        )
        self.assert_rejected(first, "remote tag rebound before draft mutation")
        second = self.mutate(
            "      - name: Revalidate remote tag before publication\n",
            "      - name: Remote tag check removed before publication\n",
        )
        self.assert_rejected(second, "remote tag rebound before publication")

    def test_permissions_are_read_by_default_and_write_only_for_publish(self) -> None:
        document = contract._parse_workflow_yaml(WORKFLOW)
        self.assertEqual(
            {"actions": "read", "contents": "read"},
            document["jobs"]["validate"]["permissions"],
        )
        validate_without_actions = self.mutate(
            "    permissions:\n      actions: read\n      contents: read\n",
            "    permissions:\n      contents: read\n",
        )
        self.assert_rejected(
            validate_without_actions,
            "validate-only Actions read permission",
        )
        top_level_write = self.mutate(
            "permissions:\n  contents: read\n",
            "permissions:\n  contents: write\n",
        )
        self.assert_rejected(top_level_write, "top-level contents read permission")
        publish_read = self.mutate(
            "  publish:\n    name: Publish verified GitHub prerelease\n    permissions:\n      contents: write\n",
            "  publish:\n    name: Publish verified GitHub prerelease\n    permissions:\n      contents: read\n",
        )
        self.assert_rejected(publish_read, "publish-only contents write permission")

        quality_write = self.mutate(
            "  quality:\n    name: Release quality gates\n",
            "  quality:\n    name: Release quality gates\n    permissions:\n      contents: write\n",
        )
        self.assert_rejected(quality_write, "only publish may elevate contents permission")

    def test_external_actions_are_pinned_to_full_commit_shas(self) -> None:
        mutated = self.mutate(
            f"uses: actions/checkout@{CHECKOUT_SHA}",
            "uses: actions/checkout@v4",
        )
        self.assert_rejected(mutated, "external actions pinned to full commit SHA")

    def test_quality_must_pin_ruby_before_the_python_validator(self) -> None:
        ruby_setup = (
            "      - uses: ruby/setup-ruby@95ef2b042f9d7a56d8268cba8559e2842e2ad01b # v1\n"
            "        with:\n"
            "          ruby-version: '3.3'\n\n"
        )
        self.assertIn(ruby_setup, WORKFLOW)
        mutated = WORKFLOW.replace(ruby_setup, "", 1).replace(
            "      - name: Provisioner unit tests\n",
            ruby_setup + "      - name: Provisioner unit tests\n",
            1,
        )
        self.assert_rejected(
            mutated, "quality pins Ruby Psych before the release validator"
        )

    def test_duplicate_yaml_mapping_key_is_rejected(self) -> None:
        mutated = self.mutate(
            "name: Release\n\n",
            "name: Release\nname: Shadow release\n\n",
        )
        self.assert_rejected(mutated, "valid unique-key YAML 1.2 workflow")

    def test_quality_command_cannot_be_faked_by_a_trailing_comment(self) -> None:
        mutated = self.mutate(
            "        run: cargo clippy --workspace --all-targets -- -D warnings\n",
            "        run: echo skipped # cargo clippy --workspace --all-targets -- -D warnings\n",
        )
        self.assert_rejected(mutated, "complete Ubuntu release quality gates")

    def test_quality_command_cannot_be_faked_by_a_heredoc_string(self) -> None:
        mutated = self.mutate(
            "        run: cargo clippy --workspace --all-targets -- -D warnings\n",
            "        run: |\n"
            "          cat <<'DECOY'\n"
            "          cargo clippy --workspace --all-targets -- -D warnings\n"
            "          DECOY\n",
        )
        self.assert_rejected(mutated, "complete Ubuntu release quality gates")

    def test_simple_gate_cannot_be_wrapped_in_false_shell_control_flow(self) -> None:
        mutated = self.mutate(
            "        run: cargo clippy --workspace --all-targets -- -D warnings\n",
            "        run: |\n"
            "          if false; then\n"
            "            cargo clippy --workspace --all-targets -- -D warnings\n"
            "          fi\n",
        )
        self.assert_rejected(mutated, "approved release run templates")

    def test_publish_cannot_be_wrapped_in_false_shell_control_flow(self) -> None:
        mutated = self.mutate(
            '        run: gh release edit "$RELEASE_TAG" --draft=false --prerelease --latest=false\n',
            "        run: |\n"
            "          if false; then\n"
            '            gh release edit "$RELEASE_TAG" --draft=false --prerelease --latest=false\n'
            "          fi\n",
        )
        self.assert_rejected(mutated, "approved release run templates")

    def test_git_source_mutation_after_assertion_is_rejected(self) -> None:
        mutated = self.mutate(
            "      - name: Free disk space\n",
            "      - name: Mutate source after assertion\n"
            "        run: git checkout main\n\n"
            "      - name: Free disk space\n",
        )
        self.assert_rejected(mutated, "no git source mutation after SHA assertion")

    def test_reassertion_step_cannot_be_a_decoy(self) -> None:
        mutated = self.mutate(
            "\n  macos_arm64:\n",
            "\n      - name: Reassert exact source after quality gates\n"
            "        run: echo clean\n"
            "\n  macos_arm64:\n",
        )
        self.assert_rejected(mutated, "approved release step sets")

    def test_every_reassertion_rejects_an_untracked_source_file(self) -> None:
        names = (
            "Reassert exact source after validation",
            "Reassert exact source after quality gates",
            "Reassert exact source before macOS build",
            "Reassert exact source after macOS packaging",
            "Reassert exact source before Windows build",
            "Reassert exact source after Windows packaging",
            "Reassert exact source before draft mutation",
            "Reassert exact source before publication",
        )
        strict = '          test -z "$(git status --porcelain=v1 --untracked-files=all)"\n'
        permissive = (
            '          status="$(git status --porcelain=v1 --untracked-files=all)"\n'
            '          test "$status" = "?? untracked-release-source.rs" || test -z "$status"\n'
        )
        for name in names:
            with self.subTest(step=name):
                start = WORKFLOW.index(f"      - name: {name}\n")
                stop = WORKFLOW.find("\n      - ", start + 1)
                stop = len(WORKFLOW) if stop < 0 else stop
                body = WORKFLOW[start:stop]
                if strict in body:
                    mutated_body = body.replace(strict, permissive, 1)
                else:
                    marker = "remote-tag-before-publication.txt) ;;"
                    if marker not in body:
                        marker = "remote-tag-before-draft.txt|remote-tag-before-publication.txt) ;;"
                    self.assertIn(marker, body)
                    mutated_body = body.replace(
                        marker,
                        marker.removesuffix(") ;;")
                        + "|untracked-release-source.rs) ;;",
                        1,
                    )
                mutated = WORKFLOW[:start] + mutated_body + WORKFLOW[stop:]
                self.assert_rejected(
                    mutated, "every source reassert rejects untracked files"
                )

    def test_publish_outputs_must_stay_outside_the_worktree(self) -> None:
        mutated = self.mutate(
            '          publish_root="$RUNNER_TEMP/opentake-release-$GITHUB_RUN_ID-$GITHUB_RUN_ATTEMPT"\n',
            '          publish_root="$GITHUB_WORKSPACE/release-assets"\n',
        )
        self.assert_rejected(mutated, "publish outputs stay outside the worktree")

    def test_publish_python_helpers_cannot_write_bytecode_into_checkout(self) -> None:
        protected_env = "      PYTHONDONTWRITEBYTECODE: '1'\n"
        if protected_env in WORKFLOW:
            protected = WORKFLOW
        else:
            protected = self.mutate(
                "      NOTES_PATH: ${{ needs.validate.outputs.notes_path }}\n",
                "      NOTES_PATH: ${{ needs.validate.outputs.notes_path }}\n"
                + protected_env,
            )
        unprotected = protected.replace(protected_env, "", 1)

        self.assertNotIn(
            "publish Python helpers cannot write bytecode into the checkout",
            contract.validate_workflow(protected),
        )
        self.assert_rejected(
            unprotected,
            "publish Python helpers cannot write bytecode into the checkout",
        )

    def test_macos_package_command_cannot_be_faked_by_an_echo_string(self) -> None:
        original = (
            "        run: >-\n"
            "          ./web/node_modules/.bin/tauri build --ci\n"
            "          --target aarch64-apple-darwin --bundles app,dmg\n"
            "          --config '{\"bundle\":{\"createUpdaterArtifacts\":true,\"macOS\":{\"signingIdentity\":\"-\"}}}'\n"
        )
        decoy = (
            "        run: >-\n"
            "          echo './web/node_modules/.bin/tauri build --ci\n"
            "          --target aarch64-apple-darwin --bundles app,dmg\n"
            "          --config {\"bundle\":{\"createUpdaterArtifacts\":true,\"macOS\":{\"signingIdentity\":\"-\"}}}'\n"
        )
        self.assert_rejected(
            self.mutate(original, decoy), "complete ad-hoc macOS ARM64 bundle gate"
        )

    def test_windows_package_command_cannot_be_faked_by_a_string(self) -> None:
        mutated = self.mutate(
            "            & .\\web\\node_modules\\.bin\\tauri.cmd @tauriArguments\n",
            "            $decoy = '& .\\web\\node_modules\\.bin\\tauri.cmd @tauriArguments'\n",
        )
        self.assert_rejected(mutated, "complete Windows x64 installer gate")

    def test_windows_build_config_is_one_runner_temp_file_argument(self) -> None:
        document = contract._parse_workflow_yaml(WORKFLOW)
        windows = document["jobs"]["windows_x64"]
        build = next(
            step
            for step in windows["steps"]
            if step.get("name")
            == "Build native MSI, NSIS, and signed updater artifacts"
        )
        script = build["run"]
        required = (
            '$configPath = Join-Path $env:RUNNER_TEMP "opentake-windows-updater-$env:GITHUB_RUN_ID-$env:GITHUB_RUN_ATTEMPT.json"',
            "$configJson = '{\"bundle\":{\"createUpdaterArtifacts\":true}}'",
            "$utf8NoBom = [System.Text.UTF8Encoding]::new($false)",
            "[System.IO.File]::WriteAllText($configPath, $configJson, $utf8NoBom)",
            "$parsedConfig = [System.IO.File]::ReadAllText($configPath, [System.Text.Encoding]::UTF8) | ConvertFrom-Json -AsHashtable",
            "$tauriArguments = @(",
            "'--config'",
            "$configPath",
            "& .\\web\\node_modules\\.bin\\tauri.cmd @tauriArguments",
        )
        for marker in required:
            with self.subTest(marker=marker):
                self.assertIn(marker, script)
        self.assertNotIn("--config '{", script)

    def test_windows_build_config_file_contract_is_fail_closed(self) -> None:
        mutations = (
            (
                '$configPath = Join-Path $env:RUNNER_TEMP "opentake-windows-updater-$env:GITHUB_RUN_ID-$env:GITHUB_RUN_ATTEMPT.json"',
                '$configPath = Join-Path $env:GITHUB_WORKSPACE "tauri-release.json"',
            ),
            (
                "$utf8NoBom = [System.Text.UTF8Encoding]::new($false)",
                "$utf8NoBom = [System.Text.UTF8Encoding]::new($true)",
            ),
            (
                "$configJson = '{\"bundle\":{\"createUpdaterArtifacts\":true}}'",
                "$configJson = '{\"bundle\":{\"createUpdaterArtifacts\":false}}'",
            ),
            (
                "if ($configItem.PSIsContainer -or (($configItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) {",
                "if ($configItem.PSIsContainer) {",
            ),
            (
                "            '--config'\n            $configPath\n",
                "            '--config'\n            $configJson\n",
            ),
            (
                "& .\\web\\node_modules\\.bin\\tauri.cmd @tauriArguments",
                "Write-Host '& .\\web\\node_modules\\.bin\\tauri.cmd @tauriArguments'",
            ),
        )
        for before, after in mutations:
            with self.subTest(before=before):
                self.assert_rejected(
                    self.mutate(before, after),
                    "complete Windows x64 installer gate",
                )

    def test_publish_command_cannot_be_faked_by_echo(self) -> None:
        mutated = self.mutate(
            '        run: gh release edit "$RELEASE_TAG" --draft=false --prerelease --latest=false\n',
            '        run: echo \'gh release edit "$RELEASE_TAG" --draft=false --prerelease --latest=false\'\n',
        )
        self.assert_rejected(mutated, "verified prerelease publication")

    def test_repository_metadata_is_beta4_and_release_notes_exist(self) -> None:
        self.assertEqual([], contract.validate_repository_metadata(REPOSITORY_ROOT))

    def test_release_notes_document_normal_push_and_dual_sha_recovery(self) -> None:
        self.assertTrue(RELEASE_NOTES_PATH.is_file())
        self.assertEqual(
            [], contract.validate_release_notes_contract(RELEASE_NOTES_PATH)
        )
        with tempfile.TemporaryDirectory() as directory:
            notes = Path(directory) / "notes.md"
            notes.write_text(
                "tag must always equal current main\n", encoding="utf-8"
            )
            self.assertEqual(
                ["Beta 4 release notes document dual-SHA recovery provenance"],
                contract.validate_release_notes_contract(notes),
            )
        canonical = RELEASE_NOTES_PATH.read_text(encoding="utf-8")
        for marker in (
            "`failed_run_id=31412976593`",
            "`failed_recovery_run_id=31427093503`",
            "`2c4efdff9d2587c90cbcac0919f9d1d333d67d6a`",
            "`6162466834bbabb8a16a2c08808e03a53c2b22b6`",
        ):
            with self.subTest(marker=marker):
                with tempfile.TemporaryDirectory() as directory:
                    notes = Path(directory) / "notes.md"
                    notes.write_text(
                        canonical.replace(marker, "`redacted`"),
                        encoding="utf-8",
                    )
                    self.assertEqual(
                        [
                            "Beta 4 release notes document dual-SHA recovery provenance"
                        ],
                        contract.validate_release_notes_contract(notes),
                    )


class ReleaseRepositoryMetadataTests(unittest.TestCase):
    def make_repository(self) -> tuple[tempfile.TemporaryDirectory[str], Path]:
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        (root / "src-tauri").mkdir()
        (root / "web").mkdir()
        (root / "docs" / "releases").mkdir(parents=True)
        (root / "Cargo.toml").write_text(
            '[workspace.package]\nversion = "1.0.0-beta.4"\n', encoding="utf-8"
        )
        (root / "src-tauri" / "tauri.conf.json").write_text(
            json.dumps(
                {
                    "version": "1.0.0-beta.4",
                    "bundle": {"windows": {"wix": {"version": "1.0.0.4"}}},
                }
            ),
            encoding="utf-8",
        )
        (root / "web" / "package.json").write_text(
            json.dumps({"version": "1.0.0-beta.4"}), encoding="utf-8"
        )
        (root / "docs" / "releases" / "1.0.0-beta.4.md").write_text(
            "release notes\n", encoding="utf-8"
        )
        return temporary, root

    def test_non_object_json_metadata_returns_a_contract_error(self) -> None:
        for relative_path in ("src-tauri/tauri.conf.json", "web/package.json"):
            with self.subTest(path=relative_path):
                temporary, root = self.make_repository()
                self.addCleanup(temporary.cleanup)
                (root / relative_path).write_text("[]\n", encoding="utf-8")
                self.assertEqual(
                    ["readable Cargo, Tauri, and Web version metadata"],
                    contract.validate_repository_metadata(root),
                )

    def test_release_notes_io_error_returns_a_contract_error(self) -> None:
        temporary, root = self.make_repository()
        self.addCleanup(temporary.cleanup)
        notes = root / "docs" / "releases" / "1.0.0-beta.4.md"
        real_read_text = Path.read_text

        def fail_notes(path: Path, *args, **kwargs):
            if path == notes:
                raise OSError("synthetic notes read failure")
            return real_read_text(path, *args, **kwargs)

        with mock.patch.object(Path, "read_text", autospec=True, side_effect=fail_notes):
            self.assertEqual(
                ["Beta 4 release notes exist"],
                contract.validate_repository_metadata(root),
            )

    def test_wrong_windows_installer_version_returns_a_contract_error(self) -> None:
        temporary, root = self.make_repository()
        self.addCleanup(temporary.cleanup)
        (root / "src-tauri" / "tauri.conf.json").write_text(
            json.dumps(
                {
                    "version": "1.0.0-beta.4",
                    "bundle": {"windows": {"wix": {"version": "1.0.0.3"}}},
                }
            ),
            encoding="utf-8",
        )
        self.assertEqual(
            ["Windows installer version is 1.0.0.4"],
            contract.validate_repository_metadata(root),
        )


class ReleaseDraftStateMachineTests(unittest.TestCase):
    def release_payload(self, *, draft: bool) -> dict[str, object]:
        return {
            "data": {
                "repository": {
                    "release": {
                        "databaseId": 123,
                        "tagName": "v1.0.0-beta.3",
                        "tagCommit": {"oid": "a" * 40},
                        "isDraft": draft,
                        "isPrerelease": True,
                        "releaseAssets": {
                            "nodes": [
                                {"id": "RA_old", "name": "old.dmg", "size": 10}
                            ],
                            "pageInfo": {"hasNextPage": False},
                        },
                    }
                }
            }
        }

    def resolver(self):
        resolver = getattr(contract, "resolve_release_state", None)
        self.assertIsNotNone(resolver, "release draft state resolver must exist")
        return resolver

    def test_missing_release_plans_draft_creation(self) -> None:
        payload = {"data": {"repository": {"release": None}}}
        plan = self.resolver()(payload, "v1.0.0-beta.3", "a" * 40)
        self.assertEqual("create", plan["action"])
        self.assertEqual([], plan["asset_node_ids"])

    def test_same_tag_draft_plans_refresh_with_existing_assets(self) -> None:
        plan = self.resolver()(
            self.release_payload(draft=True), "v1.0.0-beta.3", "a" * 40
        )
        self.assertEqual("refresh", plan["action"])
        self.assertEqual(["RA_old"], plan["asset_node_ids"])

    def test_published_release_fails_before_any_asset_delete_plan(self) -> None:
        error_type = getattr(contract, "ReleaseStateError", None)
        self.assertIsNotNone(error_type, "published release error type must exist")
        with self.assertRaisesRegex(
            error_type, "release is already published; refusing to mutate it"
        ):
            self.resolver()(
                self.release_payload(draft=False), "v1.0.0-beta.3", "a" * 40
            )


class RemoteTagResolutionTests(unittest.TestCase):
    def resolver(self):
        resolver = getattr(contract, "resolve_remote_tag_refs", None)
        self.assertIsNotNone(resolver, "remote tag ref resolver must exist")
        return resolver

    def test_lightweight_tag_resolves_direct_commit(self) -> None:
        sha = "a" * 40
        refs = f"{sha}\trefs/tags/v1.0.0-beta.3\n"
        self.assertEqual(
            sha, self.resolver()(refs, "v1.0.0-beta.3")
        )

    def test_annotated_tag_resolves_peeled_commit(self) -> None:
        tag_object = "b" * 40
        commit = "c" * 40
        refs = (
            f"{tag_object}\trefs/tags/v1.0.0-beta.3\n"
            f"{commit}\trefs/tags/v1.0.0-beta.3^{{}}\n"
        )
        self.assertEqual(
            commit, self.resolver()(refs, "v1.0.0-beta.3")
        )


if __name__ == "__main__":
    unittest.main()
