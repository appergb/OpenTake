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

    def test_comment_cannot_replace_workspace_test_command(self) -> None:
        mutated = WORKFLOW.replace(
            "run: cargo test --workspace -- --test-threads=1",
            "run: echo skipped # cargo test --workspace -- --test-threads=1",
            1,
        )
        self.assert_rejected(mutated, "Rust workspace tests command")

    def test_product_cache_cannot_restore_target_outputs(self) -> None:
        mutated = WORKFLOW.replace(
            "            ~/.cargo/git\n          key: windows-product-",
            "            ~/.cargo/git\n            target\n          key: windows-product-",
            1,
        )
        self.assert_rejected(mutated, "product cache excludes target")

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
