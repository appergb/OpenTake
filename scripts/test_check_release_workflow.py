from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import check_release_workflow as contract


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_PATH = REPOSITORY_ROOT / ".github" / "workflows" / "release.yml"
WORKFLOW = WORKFLOW_PATH.read_text(encoding="utf-8") if WORKFLOW_PATH.is_file() else ""
CHECKOUT_SHA = "11d5960a326750d5838078e36cf38b85af677262"


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
            "          --config '{\"bundle\":{\"macOS\":{\"signingIdentity\":\"-\"}}}'\n"
        )
        decoy = (
            "        run: >-\n"
            "          echo './web/node_modules/.bin/tauri build --ci\n"
            "          --target aarch64-apple-darwin --bundles app,dmg\n"
            "          --config {\"bundle\":{\"macOS\":{\"signingIdentity\":\"-\"}}}'\n"
        )
        self.assert_rejected(
            self.mutate(original, decoy), "complete ad-hoc macOS ARM64 bundle gate"
        )

    def test_windows_package_command_cannot_be_faked_by_a_string(self) -> None:
        mutated = self.mutate(
            "          & .\\web\\node_modules\\.bin\\tauri.cmd build --ci --bundles msi,nsis\n",
            "          $decoy = '& .\\web\\node_modules\\.bin\\tauri.cmd build --ci --bundles msi,nsis'\n",
        )
        self.assert_rejected(mutated, "complete Windows x64 installer gate")

    def test_publish_command_cannot_be_faked_by_echo(self) -> None:
        mutated = self.mutate(
            '        run: gh release edit "$RELEASE_TAG" --draft=false --prerelease --latest=false\n',
            '        run: echo \'gh release edit "$RELEASE_TAG" --draft=false --prerelease --latest=false\'\n',
        )
        self.assert_rejected(mutated, "verified prerelease publication")

    def test_repository_metadata_is_beta2_and_release_notes_exist(self) -> None:
        self.assertEqual([], contract.validate_repository_metadata(REPOSITORY_ROOT))


class ReleaseRepositoryMetadataTests(unittest.TestCase):
    def make_repository(self) -> tuple[tempfile.TemporaryDirectory[str], Path]:
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        (root / "src-tauri").mkdir()
        (root / "web").mkdir()
        (root / "docs" / "releases").mkdir(parents=True)
        (root / "Cargo.toml").write_text(
            '[workspace.package]\nversion = "1.0.0-beta.2"\n', encoding="utf-8"
        )
        (root / "src-tauri" / "tauri.conf.json").write_text(
            json.dumps({"version": "1.0.0-beta.2"}), encoding="utf-8"
        )
        (root / "web" / "package.json").write_text(
            json.dumps({"version": "1.0.0-beta.2"}), encoding="utf-8"
        )
        (root / "docs" / "releases" / "1.0.0-beta.2.md").write_text(
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
        notes = root / "docs" / "releases" / "1.0.0-beta.2.md"
        real_read_text = Path.read_text

        def fail_notes(path: Path, *args, **kwargs):
            if path == notes:
                raise OSError("synthetic notes read failure")
            return real_read_text(path, *args, **kwargs)

        with mock.patch.object(Path, "read_text", autospec=True, side_effect=fail_notes):
            self.assertEqual(
                ["Beta 2 release notes exist"],
                contract.validate_repository_metadata(root),
            )


class ReleaseDraftStateMachineTests(unittest.TestCase):
    def release_payload(self, *, draft: bool) -> dict[str, object]:
        return {
            "data": {
                "repository": {
                    "release": {
                        "databaseId": 123,
                        "tagName": "v1.0.0-beta.2",
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
        plan = self.resolver()(payload, "v1.0.0-beta.2", "a" * 40)
        self.assertEqual("create", plan["action"])
        self.assertEqual([], plan["asset_node_ids"])

    def test_same_tag_draft_plans_refresh_with_existing_assets(self) -> None:
        plan = self.resolver()(
            self.release_payload(draft=True), "v1.0.0-beta.2", "a" * 40
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
                self.release_payload(draft=False), "v1.0.0-beta.2", "a" * 40
            )


class RemoteTagResolutionTests(unittest.TestCase):
    def resolver(self):
        resolver = getattr(contract, "resolve_remote_tag_refs", None)
        self.assertIsNotNone(resolver, "remote tag ref resolver must exist")
        return resolver

    def test_lightweight_tag_resolves_direct_commit(self) -> None:
        sha = "a" * 40
        refs = f"{sha}\trefs/tags/v1.0.0-beta.2\n"
        self.assertEqual(
            sha, self.resolver()(refs, "v1.0.0-beta.2")
        )

    def test_annotated_tag_resolves_peeled_commit(self) -> None:
        tag_object = "b" * 40
        commit = "c" * 40
        refs = (
            f"{tag_object}\trefs/tags/v1.0.0-beta.2\n"
            f"{commit}\trefs/tags/v1.0.0-beta.2^{{}}\n"
        )
        self.assertEqual(
            commit, self.resolver()(refs, "v1.0.0-beta.2")
        )


if __name__ == "__main__":
    unittest.main()
