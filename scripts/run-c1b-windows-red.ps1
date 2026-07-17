param(
  [Parameter(Mandatory = $true)][ValidateSet('6b', '7a', '7b', '7c')][string]$Task,
  [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-f]{40}$')][string]$TestSha,
  [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-f]{40}$')][string]$ParentSha,
  [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-f]{16}$')][string]$Nonce,
  [Parameter(Mandatory = $true)][string]$EvidenceRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$actual = (git rev-parse HEAD).Trim().ToLowerInvariant()
$actualParent = (git rev-parse 'HEAD^').Trim().ToLowerInvariant()
$commitRow = @((git rev-list --parents -n 1 HEAD).Trim().Split(' '))
$changedPaths = @(git diff-tree --no-commit-id --name-only -r HEAD)
if ($actual -cne $TestSha) { throw 'expected-RED checkout does not match TestSha' }
if ($actualParent -cne $ParentSha) { throw 'expected-RED parent does not match ParentSha' }
if ($commitRow.Count -ne 2) { throw 'expected-RED commit must have exactly one parent' }
if ($changedPaths.Count -ne 1 -or $changedPaths[0] -cne 'crates/opentake-project/src/safe_fs/windows.rs') {
  throw 'expected-RED commit changed paths outside windows.rs'
}
New-Item -ItemType Directory -Path $EvidenceRoot -ErrorAction Stop | Out-Null
$Evidence = Join-Path $EvidenceRoot "c1b-task-$Task-$TestSha-$Nonce"
New-Item -ItemType Directory -Path $Evidence -ErrorAction Stop | Out-Null
$startedAt = (Get-Date).ToUniversalTime().ToString('o')
$redRows = [System.Collections.Generic.List[object]]::new()
$parentPassTests = @()

function Invoke-ExpectedRed {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string]$Log,
    [Parameter(Mandatory = $true)][string]$ExpectedPattern
  )
  & cargo test -p opentake-project --lib $Name -- --exact --test-threads=1 2>&1 |
    Tee-Object -FilePath $Log | Out-Host
  $code = $LASTEXITCODE
  if ($code -eq 0) { throw "RED unexpectedly passed: $Name" }
  $text = Get-Content -Raw -Path $Log
  if ([regex]::Matches($text, '(?m)^running 1 test\r?$').Count -ne 1) {
    throw "RED did not execute exactly one test: $Name"
  }
  if ([regex]::Matches($text, '(?m)^test result: FAILED\. 0 passed; 1 failed;').Count -ne 1) {
    throw "RED did not report exactly one failed test: $Name"
  }
  $escaped = [regex]::Escape($Name)
  if ([regex]::Matches($text, "(?m)^test $escaped \.\.\. FAILED\r?$").Count -ne 1) {
    throw "RED failure was not the selected test: $Name"
  }
  if (-not (Select-String -Quiet -Path $Log -Pattern $ExpectedPattern)) {
    throw "RED did not fail for the required typed refusal: $Name / $ExpectedPattern"
  }
  return $code
}

function Invoke-ExpectedPass {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string]$Log
  )
  & cargo test -p opentake-project --lib $Name -- --exact --test-threads=1 2>&1 |
    Tee-Object -FilePath $Log | Out-Host
  if ($LASTEXITCODE -ne 0) { throw "required parent-contract test failed: $Name" }
  $text = Get-Content -Raw -Path $Log
  if ([regex]::Matches($text, '(?m)^running 1 test\r?$').Count -ne 1 -or
      [regex]::Matches($text, '(?m)^test result: ok\. 1 passed; 0 failed;').Count -ne 1) {
    throw "PASS did not execute exactly one successful test: $Name"
  }
}

function Invoke-RedCase {
  param([string]$Name, [string]$LogName, [string]$ExpectedPattern)
  $exit = Invoke-ExpectedRed -Name $Name -Log (Join-Path $Evidence $LogName) `
    -ExpectedPattern $ExpectedPattern
  $redRows.Add([ordered]@{
    name = $Name
    command = "cargo test -p opentake-project --lib $Name -- --exact --test-threads=1"
    exit = $exit
    expected = $ExpectedPattern
    log = $LogName
  })
}

switch ($Task) {
  '6b' {
    Invoke-RedCase 'safe_fs::windows::tests::nested_retained_io_roundtrip' `
      'windows-io-red.log' 'UnsupportedSecureFilesystem|UnsupportedTarget'
    Invoke-RedCase 'safe_fs::windows::tests::windows_post_create_metadata_failure_rolls_back_same_handle' `
      'windows-create-rollback-red.log' 'UnsupportedSecureFilesystem|UnsupportedTarget'
  }
  '7a' {
    $parentPassTests = @(
      'component_utf16_and_rejections', 'unicode_and_object_attribute_lifetimes',
      'operation_contract_spy_all_rows', 'volume_root_contract_is_access_dependent',
      'synchronous_nt_completion_rejects_pending_buffer_small_and_warnings',
      'query_reports_reparse_as_present_and_open_rejects', 'reparse_parser_bounds_every_field',
      'directory_parser_bounds_and_requery', 'metadata_types_and_hardlinks',
      'ten_thousand_handles_return_to_baseline', 'ancestor_mapping_cannot_rebind',
      'every_volume_field_is_bound', 'create_new_preserves_every_existing_kind',
      'ntstatus_mapping_is_operation_specific', 'production_capabilities_own_drop_resources'
    )
    foreach ($shortName in $parentPassTests) {
      $fullName = "safe_fs::windows::tests::$shortName"
      Invoke-ExpectedPass -Name $fullName -Log (Join-Path $Evidence "$shortName.pass.log")
    }
    Invoke-RedCase 'safe_fs::windows::tests::owner_only_file_directory_stage_succeed_and_rollback' `
      'windows-owner-only-red.log' 'VerifySecurityDescriptor|UnsupportedSecureFilesystem|UnsupportedTarget'
  }
  '7b' {
    Invoke-RedCase 'safe_fs::windows::tests::quarantine_and_publish_success_do_not_self_conflict' `
      'windows-rename-red.log' 'QuarantineNoReplace|UnsupportedAtomicPublish|PrimitiveUnavailable'
  }
  '7c' {
    Invoke-RedCase 'safe_fs::windows::tests::cleanup_quarantined_tree_deletes_nested_reparse_without_traversal' `
      'windows-cleanup-red.log' 'OpenCleanupEntry|UnsupportedSecureFilesystem|UnsupportedTarget'
  }
}

[ordered]@{
  schema = 'opentake-c1b-windows-red-v1'
  repository = $env:GITHUB_REPOSITORY
  workflow = $env:GITHUB_WORKFLOW
  run_id = $env:GITHUB_RUN_ID
  run_attempt = $env:GITHUB_RUN_ATTEMPT
  job_id = $env:GITHUB_JOB
  event_name = $env:GITHUB_EVENT_NAME
  runner_os = $env:RUNNER_OS
  runner_arch = $env:RUNNER_ARCH
  task = $Task
  test_sha = $TestSha
  parent_sha = $ParentSha
  nonce = $Nonce
  changed_paths = @($changedPaths)
  red = @($redRows)
  parent_pass_tests = @($parentPassTests)
  parent_pass_count = $parentPassTests.Count
  started_at_utc = $startedAt
  finished_at_utc = (Get-Date).ToUniversalTime().ToString('o')
} | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8NoBOM `
  (Join-Path $Evidence 'red-receipt.json')
