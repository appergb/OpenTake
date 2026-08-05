import json
import unittest
from pathlib import Path

import check_windows_product_ci as contract


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = (REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml").read_text(
    encoding="utf-8"
)


class WindowsProductCiContractTests(unittest.TestCase):
    def assert_rejected(self, workflow: str, expected: str) -> None:
        self.assertIn(expected, contract.validate_workflow(workflow))

    def test_repository_workflow_satisfies_contract(self) -> None:
        self.assertEqual([], contract.validate_workflow(WORKFLOW))

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
        self.assert_rejected(mutated, "pinned FFmpeg cancellation lifecycle")

    def test_windows_process_tree_gate_must_cover_fast_parent_exit(self) -> None:
        mutated = WORKFLOW.replace(
            "          process_tree::tests::windows_suspended_job_contains_fast_exit_descendant\n",
            "",
            1,
        )
        self.assert_rejected(mutated, "race-free Windows process-tree containment")

    def test_windows_reparse_gate_must_cover_retained_external_source(self) -> None:
        mutated = WORKFLOW.replace(
            "          cargo test -p opentake-tauri --lib retained_external_source_rejects_windows_reparse_contract\n",
            "",
            1,
        )
        self.assert_rejected(mutated, "complete Windows retained-path reparse safety")

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


if __name__ == "__main__":
    unittest.main()
