import json
import re
import unittest
from dataclasses import FrozenInstanceError, is_dataclass
from pathlib import Path

import check_windows_product_ci as contract


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = (REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml").read_text(
    encoding="utf-8"
)


def _named_job_body(workflow: str, job_name: str) -> str | None:
    match = re.search(
        rf"(?ms)^  {re.escape(job_name)}:\n(?P<body>.*?)(?=^  [a-zA-Z0-9_-]+:\n|\Z)",
        workflow,
    )
    return None if match is None else match.group("body")


def _steps(job: str) -> list[str]:
    starts = [
        match.start()
        for match in re.finditer(r"(?m)^      - (?=(?:name|uses):)", job)
    ]
    return [
        job[start : starts[index + 1] if index + 1 < len(starts) else len(job)]
        for index, start in enumerate(starts)
    ]


def _named_step(steps: list[str], name: str) -> str | None:
    prefix = f"      - name: {name}\n"
    return next((step for step in steps if step.startswith(prefix)), None)


class WindowsProductCiContractTests(unittest.TestCase):
    def assert_rejected(self, workflow: str, expected: str) -> None:
        self.assertIn(expected, contract.validate_workflow(workflow))

    def assert_job_mutation_rejected(
        self, job_name: str, before: str, after: str, expected: str
    ) -> None:
        job = _named_job_body(WORKFLOW, job_name)
        self.assertIsNotNone(job)
        assert job is not None
        self.assertIn(before, job, f"canonical {job_name} mutation token missing")
        mutated_job = job.replace(before, after, 1)
        self.assertNotEqual(job, mutated_job)
        self.assert_rejected(WORKFLOW.replace(job, mutated_job, 1), expected)

    def native_import_script(self) -> str:
        document = contract.parse_workflow_yaml(WORKFLOW)
        job = document["jobs"]["windows-security"]
        step = next(
            step
            for step in job["steps"]
            if step.get("name") == "Verify portable Tauri test image imports"
        )
        return step["run"]

    def test_repository_workflow_satisfies_contract(self) -> None:
        self.assertEqual([], contract.validate_workflow(WORKFLOW))

    def test_tauri_manifest_is_checked_out_with_lf_for_cli_rewrites(self) -> None:
        attributes_path = REPOSITORY_ROOT / ".gitattributes"
        attributes = (
            attributes_path.read_text(encoding="utf-8")
            if attributes_path.exists()
            else ""
        )
        self.assertEqual([], contract.validate_git_attributes(attributes))

    def test_rejects_missing_tauri_manifest_lf_contract(self) -> None:
        self.assertIn(
            "LF-stable Tauri Cargo manifest checkout",
            contract.validate_git_attributes(""),
        )

    def test_rejects_later_tauri_manifest_eol_override(self) -> None:
        attributes = (
            "src-tauri/Cargo.toml text eol=lf\n"
            "src-tauri/Cargo.toml text eol=crlf\n"
        )
        self.assertIn(
            "LF-stable Tauri Cargo manifest checkout",
            contract.validate_git_attributes(attributes),
        )

    def test_rejects_nested_tauri_attributes_override(self) -> None:
        attributes = "src-tauri/Cargo.toml text eol=lf\n"
        self.assertIn(
            "LF-stable Tauri Cargo manifest checkout",
            contract.validate_git_attributes(
                attributes, "Cargo.toml text eol=crlf\n"
            ),
        )

    def test_accepts_beta_app_version_with_numeric_msi_mapping(self) -> None:
        config = json.dumps(
            {
                "version": "1.0.0-beta.1",
                "bundle": {"windows": {"wix": {"version": "1.0.0.1"}}},
            }
        )
        self.assertEqual([], contract.validate_tauri_config(config))

    def test_rejects_missing_numeric_msi_mapping(self) -> None:
        config = json.dumps({"version": "1.0.0-beta.1", "bundle": {}})
        self.assertIn(
            "MSI-compatible numeric Beta version",
            contract.validate_tauri_config(config),
        )

    def test_rejects_msi_mapping_for_a_different_release(self) -> None:
        config = json.dumps(
            {
                "version": "1.1.0-beta.1",
                "bundle": {"windows": {"wix": {"version": "1.0.0.1"}}},
            }
        )
        self.assertIn(
            "MSI version tracks public app version",
            contract.validate_tauri_config(config),
        )

    def test_comment_cannot_replace_workspace_test_command(self) -> None:
        mutated = WORKFLOW.replace(
            "      - name: Rust workspace tests\n"
            "        env:\n"
            "          OPENTAKE_MOTION_TRACE: '1'\n"
            "        run: cargo test --workspace -- --test-threads=1",
            "      - name: Rust workspace tests\n"
            "        env:\n"
            "          OPENTAKE_MOTION_TRACE: '1'\n"
            "        run: echo skipped # cargo test --workspace -- --test-threads=1",
            1,
        )
        self.assert_rejected(mutated, "Rust workspace tests command")

    def test_chromium_gate_must_include_the_live_security_integration(self) -> None:
        mutated = WORKFLOW.replace(
            "            virtual_time_network_csp_timeout_cleanup_and_frame_identity \\\n",
            "",
            1,
        )
        self.assert_rejected(mutated, "complete Windows Chromium motion regression")

    def test_cancellation_gate_must_use_the_pinned_packaged_ffmpeg(self) -> None:
        mutated = WORKFLOW.replace(
            "          OPENTAKE_FFMPEG: ${{ github.workspace }}\\src-tauri\\binaries\\ffmpeg-x86_64-pc-windows-msvc.exe\n",
            "",
            1,
        )
        self.assert_rejected(mutated, "exact security gate contracts")

    def test_windows_process_tree_gate_must_cover_fast_parent_exit(self) -> None:
        mutated = WORKFLOW.replace(
            "          tests::windows_suspended_job_contains_fast_exit_descendant\n",
            "",
            1,
        )
        self.assert_rejected(mutated, "exact security gate contracts")

    def test_windows_process_tree_gate_cannot_fall_back_to_old_zero_test_package(
        self,
    ) -> None:
        self.assert_job_mutation_rejected(
            "windows-security",
            "cargo test -p opentake-process-tree --lib\n"
            "          tests::windows_suspended_job_contains_fast_exit_descendant",
            "cargo test -p opentake-media --lib\n"
            "          process_tree::tests::windows_suspended_job_contains_fast_exit_descendant",
            "exact security gate contracts",
        )

    def test_windows_reparse_gate_must_cover_retained_external_source(self) -> None:
        mutated = WORKFLOW.replace(
            "          cargo test -p opentake-tauri --lib retained_external_source_rejects_windows_reparse_contract\n",
            "",
            1,
        )
        self.assert_rejected(mutated, "exact security gate contracts")

    def test_native_import_parser_rejects_empty_output_fixture(self) -> None:
        self.assertIn(
            "Assert-DumpbinImportFixtureRejected -Label 'empty output' -Lines @()",
            self.native_import_script(),
        )

    def test_native_import_parser_rejects_localized_header_fixture(self) -> None:
        script = self.native_import_script()
        self.assertIn("-Label 'localized header'", script)
        self.assertIn("'  La sección contiene las importaciones siguientes:'", script)

    def test_native_import_parser_rejects_unknown_layout_fixture(self) -> None:
        script = self.native_import_script()
        self.assertIn("-Label 'unknown import layout'", script)
        self.assertIn("'      localized or changed layout'", script)

    def test_native_import_parser_requires_modules_and_symbols(self) -> None:
        script = self.native_import_script()
        self.assertIn("if ($moduleNames.Count -eq 0)", script)
        self.assertIn("if ($importsToProbe.Count -eq 0)", script)

    def test_native_import_parser_rejects_unknown_nonempty_section_lines(self) -> None:
        self.assertIn(
            "throw \"unrecognized non-empty dumpbin import line: $line\"",
            self.native_import_script(),
        )

    def test_native_import_parser_accepts_canonical_dumpbin_layout_fixture(self) -> None:
        script = self.native_import_script()
        self.assertIn("-Label 'canonical import layout'", script)
        for metadata in (
            "Import Address Table",
            "Import Name Table",
            "time date stamp",
            "Index of first forwarder reference",
        ):
            with self.subTest(metadata=metadata):
                self.assertIn(metadata, script)

    def test_product_cache_cannot_restore_target_outputs(self) -> None:
        mutated = WORKFLOW.replace(
            "            ~/.cargo/git\n          key: windows-product-",
            "            ~/.cargo/git\n            target\n          key: windows-product-",
            1,
        )
        self.assert_rejected(mutated, "product cache excludes target")

    def test_installed_product_must_launch(self) -> None:
        without_launch = WORKFLOW.replace(
            "          $app = Start-Process -FilePath $application -PassThru\n",
            "          $app = $null\n",
            1,
        )
        self.assert_rejected(without_launch, "installed product launch smoke test")

    def test_receipt_must_hash_installers_and_bind_source_sha(self) -> None:
        without_hash = WORKFLOW.replace(
            "sha256 = (Get-FileHash", "digest = (Get-FileHash", 1
        )
        self.assert_rejected(without_hash, "receipt records SHA-256")

        without_source = WORKFLOW.replace(
            "source_sha = $env:RECEIPT_SHA", "unbound_sha = $env:RECEIPT_SHA", 1
        )
        self.assert_rejected(without_source, "receipt binds source SHA")

    def test_upload_requires_both_installer_families(self) -> None:
        mutated = WORKFLOW.replace(
            "            target/release/bundle/nsis/*.exe\n", "", 1
        )
        self.assert_rejected(mutated, "upload includes NSIS")

    def test_required_steps_cannot_ignore_failures(self) -> None:
        mutated = WORKFLOW.replace(
            "      - name: Rust workspace tests\n",
            "      - name: Rust workspace tests\n        continue-on-error: true\n",
            1,
        )
        self.assert_rejected(mutated, "no ignored failures")

    def test_checkout_assertion_must_publish_bind_output(self) -> None:
        mutated = WORKFLOW.replace("        id: bind\n", "", 1)
        self.assert_rejected(mutated, "checkout assertion publishes bind output")

    def test_receipt_env_must_consume_bind_output(self) -> None:
        mutated = WORKFLOW.replace(
            "RECEIPT_SHA: ${{ steps.bind.outputs.sha }}", "RECEIPT_SHA: deadbeef", 1
        )
        self.assert_rejected(mutated, "receipt consumes bind output")

    def test_receipt_must_be_written_and_uploaded(self) -> None:
        without_write = WORKFLOW.replace(
            "$receipt | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8NoBOM windows-product-receipt.json",
            "$receipt | ConvertTo-Json -Depth 6 | Out-Null",
            1,
        )
        self.assert_rejected(without_write, "receipt file is written")

        without_upload = WORKFLOW.replace(
            "            windows-product-receipt.json\n", "", 1
        )
        self.assert_rejected(without_upload, "upload includes receipt")

    def test_required_step_cannot_be_conditionally_skipped(self) -> None:
        mutated = WORKFLOW.replace(
            "      - name: Rust workspace tests\n",
            "      - name: Rust workspace tests\n        if: false\n",
            1,
        )
        self.assert_rejected(mutated, "required steps are unconditional")

    def test_product_job_condition_cannot_skip_normal_runs(self) -> None:
        mutated = WORKFLOW.replace(
            "    name: Windows (full product / bundle)\n"
            "    if: github.event_name != 'workflow_dispatch' || inputs.red_task == 'none'\n",
            "    name: Windows (full product / bundle)\n    if: false\n",
            1,
        )
        self.assert_rejected(mutated, "product job condition")

    def test_security_job_target_must_bind_dispatch_pr_head_and_push_sha(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-security",
            contract.TARGET_SHA_EXPRESSION,
            "TARGET_SHA: ${{ github.sha }}",
            "security exact target SHA",
        )

    def test_library_job_target_must_bind_dispatch_pr_head_and_push_sha(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-library-security",
            contract.TARGET_SHA_EXPRESSION,
            "TARGET_SHA: ${{ github.sha }}",
            "library exact target SHA",
        )

    def test_security_checkout_must_be_exact_and_noncredentialed(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-security",
            "ref: ${{ env.TARGET_SHA }}\n          fetch-depth: 0\n          persist-credentials: false",
            "ref: main\n          fetch-depth: 1\n          persist-credentials: true",
            "security exact-SHA checkout",
        )

    def test_library_checkout_must_be_exact_and_noncredentialed(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-library-security",
            "ref: ${{ env.TARGET_SHA }}\n          fetch-depth: 0\n          persist-credentials: false",
            "ref: main\n          fetch-depth: 1\n          persist-credentials: true",
            "library exact-SHA checkout",
        )

    def test_security_job_rejects_a_second_default_checkout(self) -> None:
        exact_checkout = (
            f"      - uses: {contract._pinned_action('actions/checkout')}\n"
            "        with:\n"
            "          ref: ${{ env.TARGET_SHA }}\n"
            "          fetch-depth: 0\n"
            "          persist-credentials: false\n"
        )
        self.assert_job_mutation_rejected(
            "windows-security",
            exact_checkout,
            exact_checkout
            + f"\n      - uses: {contract._pinned_action('actions/checkout')}\n",
            "security exact-SHA checkout",
        )

    def test_library_job_rejects_a_second_default_checkout(self) -> None:
        exact_checkout = (
            f"      - uses: {contract._pinned_action('actions/checkout')}\n"
            "        with:\n"
            "          ref: ${{ env.TARGET_SHA }}\n"
            "          fetch-depth: 0\n"
            "          persist-credentials: false\n"
        )
        self.assert_job_mutation_rejected(
            "windows-library-security",
            exact_checkout,
            exact_checkout
            + f"\n      - uses: {contract._pinned_action('actions/checkout')}\n",
            "library exact-SHA checkout",
        )

    def test_security_checkout_assertion_must_compare_tree_and_cleanliness(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-security",
            "test \"$(git rev-parse 'HEAD^{tree}')\" = \"$(git rev-parse \"${expected}^{tree}\")\"\n          test -z \"$(git status --porcelain=v1 --untracked-files=all)\"",
            "true\n          true",
            "security immutable checkout assertion",
        )

    def test_library_checkout_assertion_must_compare_tree_and_cleanliness(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-library-security",
            "test \"$(git rev-parse 'HEAD^{tree}')\" = \"$(git rev-parse \"${expected}^{tree}\")\"\n          test -z \"$(git status --porcelain=v1 --untracked-files=all)\"",
            "true\n          true",
            "library immutable checkout assertion",
        )

    def test_security_receipt_must_bind_requested_and_checked_out_sha(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-security",
            "checked_out_sha = $env:RECEIPT_SHA.ToLowerInvariant()",
            "checked_out_sha = ('0' * 40)",
            "security SHA-bound receipt",
        )

    def test_library_receipt_must_bind_requested_and_checked_out_sha(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-library-security",
            "requested_sha = $env:TARGET_SHA.ToLowerInvariant()",
            "requested_sha = ('0' * 40)",
            "library SHA-bound receipt",
        )

    def test_security_receipt_must_run_even_after_a_gate_failure(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-security",
            "name: Build Windows security JSON receipt\n        if: always()",
            "name: Build Windows security JSON receipt\n        if: success()",
            "security receipt always runs",
        )

    def test_library_receipt_artifact_name_must_bind_checked_out_sha(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-library-security",
            "name: windows-library-security-${{ steps.bind-library.outputs.sha }}",
            "name: windows-library-security-mutable",
            "library SHA-bound receipt artifact",
        )

    def test_security_final_aggregate_must_be_enforced(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-security",
            "name: Enforce Windows security aggregate",
            "name: Ignore Windows security aggregate",
            "security aggregate enforcement",
        )

    def test_library_final_aggregate_must_be_enforced(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-library-security",
            "name: Enforce Windows library security aggregate",
            "name: Ignore Windows library security aggregate",
            "library aggregate enforcement",
        )

    def test_security_job_cannot_ignore_gate_failures(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-security",
            "      - name: Portable FFmpeg cancellation lifecycle\n",
            "      - name: Portable FFmpeg cancellation lifecycle\n        continue-on-error: true\n",
            "security no ignored failures",
        )

    def test_library_job_keeps_all_capability_gates(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-library-security",
            "run: cargo test -p opentake-project -- --test-threads=1",
            "run: echo project tests skipped",
            "exact library gate contracts",
        )

    def test_security_gate_rejects_a_write_host_command_lure(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-security",
            "run: >-\n          cargo test -p opentake-process-tree --lib\n          tests::windows_suspended_job_contains_fast_exit_descendant\n          -- --exact --nocapture --test-threads=1",
            "run: >-\n          Write-Host 'cargo test -p opentake-process-tree --lib\n          tests::windows_suspended_job_contains_fast_exit_descendant\n          -- --exact --nocapture --test-threads=1'",
            "exact security gate contracts",
        )

    def test_security_gate_rejects_echo_and_comment_command_lures(self) -> None:
        before = (
            "        run: |\n"
            "          cargo test -p opentake-tauri --lib windows_project_media_junction_is_rejected_without_writing_target\n"
            "          cargo test -p opentake-tauri --lib windows_directory_handoff_blocks_junction_replacement_before_child_create\n"
            "          cargo test -p opentake-tauri --lib windows_retained_output_handle_blocks_final_name_replacement\n"
            "          cargo test -p opentake-tauri --lib retained_external_source_rejects_windows_reparse_contract\n"
        )
        after = (
            "        run: |\n"
            "          Write-Host 'cargo test -p opentake-tauri --lib windows_project_media_junction_is_rejected_without_writing_target'\n"
            "          Write-Host 'cargo test -p opentake-tauri --lib windows_directory_handoff_blocks_junction_replacement_before_child_create'\n"
            "          Write-Host 'cargo test -p opentake-tauri --lib windows_retained_output_handle_blocks_final_name_replacement'\n"
            "          # cargo test -p opentake-tauri --lib retained_external_source_rejects_windows_reparse_contract\n"
        )
        self.assert_job_mutation_rejected(
            "windows-security", before, after, "exact security gate contracts"
        )

    def test_library_gate_id_is_part_of_the_exact_contract(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-library-security",
            "id: library-project",
            "id: decoy-library-project",
            "exact library gate contracts",
        )

    def test_library_gate_rejects_a_noop_string_lure(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-library-security",
            "run: cargo test -p opentake-project -- --test-threads=1",
            "run: Write-Host 'cargo test -p opentake-project -- --test-threads=1'",
            "exact library gate contracts",
        )

    def test_security_receipt_outcomes_cannot_be_swapped_between_gates(self) -> None:
        job = _named_job_body(WORKFLOW, "windows-security")
        self.assertIsNotNone(job)
        assert job is not None
        first = "steps.security-cancellation.outcome"
        second = "steps.security-process-tree.outcome"
        self.assertIn(first, job)
        self.assertIn(second, job)
        mutated_job = job.replace(first, "steps.contract-swap.outcome", 1)
        mutated_job = mutated_job.replace(second, first, 1)
        mutated_job = mutated_job.replace("steps.contract-swap.outcome", second, 1)
        self.assert_rejected(
            WORKFLOW.replace(job, mutated_job, 1),
            "security receipt gate binding",
        )

    def test_security_receipt_command_cannot_drift_from_gate_contract(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-security",
            "command = 'packaged FFmpeg -version; cargo test opentake-media Windows cancellation lifecycle'",
            "command = 'Write-Host cancellation tests skipped'",
            "security receipt gate binding",
        )

    def test_security_receipt_cannot_claim_old_zero_test_process_tree_package(
        self,
    ) -> None:
        self.assert_job_mutation_rejected(
            "windows-security",
            "command = 'cargo test -p opentake-process-tree --lib tests::windows_suspended_job_contains_fast_exit_descendant -- --exact --nocapture --test-threads=1'",
            "command = 'cargo test -p opentake-media --lib process_tree::tests::windows_suspended_job_contains_fast_exit_descendant -- --exact --nocapture --test-threads=1'",
            "security receipt gate binding",
        )

    def test_specialist_gate_order_is_immutable(self) -> None:
        job = _named_job_body(WORKFLOW, "windows-library-security")
        self.assertIsNotNone(job)
        assert job is not None
        steps = _steps(job)
        media = _named_step(steps, "Test retained-handle and junction defenses")
        project = _named_step(steps, "Test complete bundle publication and recovery")
        self.assertIsNotNone(media)
        self.assertIsNotNone(project)
        assert media is not None and project is not None
        mutated_job = job.replace(media, "__MEDIA_GATE__", 1)
        mutated_job = mutated_job.replace(project, media, 1)
        mutated_job = mutated_job.replace("__MEDIA_GATE__", project, 1)
        self.assert_rejected(
            WORKFLOW.replace(job, mutated_job, 1),
            "library gate order",
        )

    def test_specialist_contracts_are_frozen_and_bind_each_gate_once(self) -> None:
        specialist_contracts = contract.SPECIALIST_CONTRACTS
        self.assertEqual(2, len(specialist_contracts))
        for job_contract in specialist_contracts:
            self.assertTrue(is_dataclass(job_contract))
            self.assertTrue(job_contract.__dataclass_params__.frozen)
            self.assertEqual(4, len(job_contract.gates))
            self.assertRegex(job_contract.receipt_step_sha256, r"^[0-9a-f]{64}$")
            self.assertRegex(job_contract.enforce_step_sha256, r"^[0-9a-f]{64}$")
            step_ids = [gate.step_id for gate in job_contract.gates]
            receipt_ids = [gate.receipt_id for gate in job_contract.gates]
            self.assertEqual(len(step_ids), len(set(step_ids)))
            self.assertEqual(len(receipt_ids), len(set(receipt_ids)))
            gate = job_contract.gates[0]
            self.assertTrue(is_dataclass(gate))
            self.assertTrue(gate.__dataclass_params__.frozen)
            with self.assertRaises(FrozenInstanceError):
                gate.step_id = "mutable"  # type: ignore[misc]

    def test_all_external_actions_are_pinned_to_central_verified_shas(self) -> None:
        pins = contract.ACTION_PINS
        self.assertEqual(
            {
                "actions/checkout": "11d5960a326750d5838078e36cf38b85af677262",
                "actions/setup-node": "49933ea5288caeca8642d1e84afbd3f7d6820020",
                "actions/cache": "0057852bfaa89a56745cba8c7296529d2fc39830",
                "actions/upload-artifact": "ea165f8d65b6e75b540449e92b4886f43607fa02",
                "pnpm/action-setup": "b906affcce14559ad1aafd4ab0e942779e9f58b1",
                "ruby/setup-ruby": "95ef2b042f9d7a56d8268cba8559e2842e2ad01b",
            },
            dict(pins),
        )
        uses = re.findall(
            r"(?m)^\s*(?:-\s+)?uses:\s*([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+)@([^\s#]+)",
            WORKFLOW,
        )
        self.assertGreater(len(uses), 0)
        for action, revision in uses:
            with self.subTest(action=action, revision=revision):
                self.assertRegex(revision, r"^[0-9a-f]{40}$")
                self.assertEqual(pins[action], revision)

    def test_floating_major_action_tags_are_rejected_everywhere(self) -> None:
        for action, sha in contract.ACTION_PINS.items():
            with self.subTest(action=action):
                pinned = f"{action}@{sha}"
                self.assertIn(pinned, WORKFLOW)
                mutated = WORKFLOW.replace(pinned, f"{action}@v4", 1)
                self.assert_rejected(mutated, "all external actions pinned")

    def test_unreviewed_40hex_action_revision_is_rejected(self) -> None:
        action = "actions/checkout"
        sha = contract.ACTION_PINS[action]
        mutated = WORKFLOW.replace(f"{action}@{sha}", f"{action}@{'0' * 40}", 1)
        self.assert_rejected(mutated, "all external actions pinned")

    def test_action_pin_ownership_verifier_binds_each_sha_to_its_repo(self) -> None:
        calls: list[tuple[str, str]] = []

        def resolve(repo: str, sha: str) -> str:
            calls.append((repo, sha))
            return sha

        self.assertEqual([], contract.validate_action_pin_ownership(resolve))
        self.assertEqual(list(contract.ACTION_PINS.items()), calls)

    def test_action_pin_ownership_verifier_rejects_cross_repo_resolution(self) -> None:
        wrong_repo = "ruby/setup-ruby"

        def resolve(repo: str, sha: str) -> str:
            return "0" * 40 if repo == wrong_repo else sha

        self.assertEqual(
            [f"{wrong_repo} pin is not a commit in its repository"],
            contract.validate_action_pin_ownership(resolve),
        )

    def test_quoted_unapproved_external_action_is_rejected(self) -> None:
        mutated = WORKFLOW.replace(
            "      - name: Install Rust toolchain\n",
            "      - uses: \"attacker/unapproved-action@"
            + ("a" * 40)
            + "\"\n\n      - name: Install Rust toolchain\n",
            1,
        )
        self.assert_rejected(mutated, "all external actions pinned")

    def test_quoted_second_checkout_is_counted_by_action_repository(self) -> None:
        checkout = contract._pinned_action("actions/checkout")
        job = _named_job_body(WORKFLOW, "windows-security")
        self.assertIsNotNone(job)
        assert job is not None
        marker = "      - name: Assert exact Windows security checkout\n"
        self.assertIn(marker, job)
        injected = (
            f"      - uses: \"{checkout}\"\n"
            "        with:\n"
            "          ref: main\n\n"
            + marker
        )
        mutated_job = job.replace(marker, injected, 1)
        self.assert_rejected(
            WORKFLOW.replace(job, mutated_job, 1),
            "checkout action counts",
        )

    def test_quoted_checkout_main_cannot_hide_after_product_sha_binding(self) -> None:
        checkout = contract._pinned_action("actions/checkout")
        job = _named_job_body(WORKFLOW, "windows-product")
        self.assertIsNotNone(job)
        assert job is not None
        marker = "      - name: Assert exact checked-out SHA\n"
        self.assertIn(marker, job)
        injected = (
            f"      - uses: '{checkout}'\n"
            "        with:\n"
            "          ref: main\n\n"
            + marker
        )
        mutated_job = job.replace(marker, injected, 1)
        self.assert_rejected(
            WORKFLOW.replace(job, mutated_job, 1),
            "checkout action counts",
        )

    def test_receipt_requested_sha_rejects_trailing_comment_lure(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-security",
            "requested_sha = $env:TARGET_SHA.ToLowerInvariant()",
            "requested_sha = ('0' * 40) # requested_sha = $env:TARGET_SHA.ToLowerInvariant()",
            "exact security receipt build contract",
        )

    def test_receipt_checked_out_sha_rejects_trailing_comment_lure(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-library-security",
            "checked_out_sha = $env:RECEIPT_SHA.ToLowerInvariant()",
            "checked_out_sha = ('0' * 40) # checked_out_sha = $env:RECEIPT_SHA.ToLowerInvariant()",
            "exact library receipt build contract",
        )

    def test_receipt_enforce_rejects_noop_comment_lure(self) -> None:
        self.assert_job_mutation_rejected(
            "windows-security",
            "if ([int]$receipt.aggregate_exit -ne 0) { throw 'Windows security aggregate failed' }",
            "Write-Host 'aggregate skipped' # if ([int]$receipt.aggregate_exit -ne 0) { throw 'Windows security aggregate failed' }",
            "exact security receipt enforce contract",
        )

    def test_duplicate_windows_security_job_key_is_rejected(self) -> None:
        start = WORKFLOW.index("  windows-security:\n")
        end = WORKFLOW.index("\n  web:\n", start)
        duplicate = WORKFLOW[start:end]
        mutated = WORKFLOW[:end] + "\n" + duplicate + WORKFLOW[end:]
        self.assert_rejected(mutated, "valid unique-key YAML 1.2 workflow")

    def test_anchor_alias_and_tagged_nodes_are_rejected_by_shared_parser(self) -> None:
        mutations = {
            "anchor-alias": WORKFLOW.replace(
                "permissions:\n  contents: read\n",
                "permissions: &read_permissions\n"
                "  contents: read\n"
                "permissions-copy: *read_permissions\n",
                1,
            ),
            "tag": WORKFLOW.replace("permissions:\n", "permissions: !reviewed\n", 1),
        }
        for label, mutated in mutations.items():
            with self.subTest(label=label):
                self.assert_rejected(mutated, "valid unique-key YAML 1.2 workflow")

    def test_git_checkout_main_after_security_bind_is_rejected(self) -> None:
        marker = "      - name: Portable FFmpeg cancellation lifecycle\n"
        injected = (
            "      - name: Mutate trusted source after bind\n"
            "        shell: bash\n"
            "        run: git checkout main\n\n"
            + marker
        )
        self.assert_job_mutation_rejected(
            "windows-security",
            marker,
            injected,
            "no source mutation after security SHA bind",
        )

    def test_all_sha_bound_windows_jobs_have_frozen_identity_and_digest(self) -> None:
        contracts = contract.SHA_BOUND_JOB_CONTRACTS
        self.assertEqual(5, len(contracts))
        for job_contract in contracts:
            self.assertTrue(is_dataclass(job_contract))
            self.assertTrue(job_contract.__dataclass_params__.frozen)
            self.assertRegex(job_contract.job_sha256, r"^[0-9a-f]{64}$")
            self.assertEqual(
                len(job_contract.step_identities),
                len(set(job_contract.step_identities)),
            )

    def test_all_pre_and_post_reasserts_reject_untracked_source_files(self) -> None:
        document = contract.parse_workflow_yaml(WORKFLOW)
        jobs = document["jobs"]
        bash_guard = 'test -z "$(git status --porcelain=v1 --untracked-files=all)"'
        for job_contract in contract.SHA_BOUND_JOB_CONTRACTS:
            job = jobs[job_contract.job_name]
            steps = job["steps"]
            for step_name in (
                job_contract.before_reassert_name,
                job_contract.after_reassert_name,
            ):
                step = next(step for step in steps if step.get("name") == step_name)
                script = step["run"]
                with self.subTest(job=job_contract.job_name, step=step_name):
                    if job_contract.job_name == "windows-red-evidence":
                        for checkout in ("c1b-dispatcher", "c1b-target"):
                            self.assertIn(
                                f"git -C {checkout} status --porcelain=v1 --untracked-files=all",
                                script,
                            )
                    else:
                        self.assertIn(bash_guard, script)

    def test_untracked_status_lure_cannot_replace_security_guard(self) -> None:
        strict_guard = (
            'test -z "$(git status --porcelain=v1 --untracked-files=all)"'
        )
        self.assert_job_mutation_rejected(
            "windows-security",
            strict_guard,
            "printf '%s\\n' '?? untracked-security-source.rs' # " + strict_guard,
            "complete SHA-bound Windows untracked source guards",
        )


if __name__ == "__main__":
    unittest.main()
