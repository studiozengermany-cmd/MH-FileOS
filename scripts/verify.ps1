[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot

function Invoke-VerificationStage {
  param(
    [Parameter(Mandatory)]
    [string] $Name,

    [Parameter(Mandatory)]
    [scriptblock] $Action
  )

  $stopwatch = [Diagnostics.Stopwatch]::StartNew()
  Write-Host "[verify] START $Name"
  & $Action
  $stopwatch.Stop()
  Write-Host "[verify] PASS  $Name ($($stopwatch.Elapsed.TotalSeconds.ToString('0.00'))s)"
}

function Test-MarkdownLocalLinks {
  $missing = @()
  Get-ChildItem -LiteralPath $repoRoot -Recurse -File -Filter '*.md' | ForEach-Object {
    $document = $_
    $content = Get-Content -LiteralPath $document.FullName -Raw
    foreach ($match in [regex]::Matches($content, '\[[^\]]+\]\(([^)]+)\)')) {
      $target = $match.Groups[1].Value.Trim()
      if ($target -match '^(https?://|mailto:|#)') {
        continue
      }

      $pathPart = ($target -split '#', 2)[0].Trim('<', '>')
      if ([string]::IsNullOrWhiteSpace($pathPart)) {
        continue
      }

      $resolved = Join-Path $document.DirectoryName $pathPart
      if (-not (Test-Path -LiteralPath $resolved)) {
        $missing += "$($document.FullName) -> $target"
      }
    }
  }

  if ($missing.Count -ne 0) {
    throw "Missing local Markdown targets:`n$($missing -join [Environment]::NewLine)"
  }
}

function Test-NoFixtureRunResidue {
  $runtimeRoot = Join-Path $repoRoot 'fixtures/sandbox/runtime'
  if (-not (Test-Path -LiteralPath $runtimeRoot -PathType Container)) {
    return
  }

  $residue = @(Get-ChildItem -LiteralPath $runtimeRoot -Force)
  if ($residue.Count -ne 0) {
    throw "Fixture tests left $($residue.Count) runtime item(s) under '$runtimeRoot'."
  }
}

function Test-FixtureManifestJson {
  $toolchain = '1.97.0-x86_64-pc-windows-msvc'
  $output = & rustup run $toolchain cargo test -p fileos-testkit `
    'manifest::tests::manifest_json_probe_emits_deterministic_document' `
    --locked --offline -- --exact --nocapture 2>&1
  if ($LASTEXITCODE -ne 0) {
    throw "Manifest JSON probe exited with code $LASTEXITCODE."
  }

  $text = ($output -join [Environment]::NewLine)
  $match = [regex]::Match(
    $text,
    '(?s)MH_FILEOS_MANIFEST_JSON_BEGIN\r?\n(.*?)MH_FILEOS_MANIFEST_JSON_END'
  )
  if (-not $match.Success) {
    throw 'Manifest JSON probe markers were not found in Cargo test output.'
  }

  $manifest = $match.Groups[1].Value | ConvertFrom-Json
  if ($manifest.schema_version -ne 1 -or
      $manifest.seed -ne 42 -or
      $manifest.disposable -ne $true -or
      @($manifest.entries).Count -eq 0) {
    throw 'Manifest JSON parsed but required identity/evidence fields were missing.'
  }
}

function Test-ScanSummaryJson {
  $toolchain = '1.97.0-x86_64-pc-windows-msvc'
  $output = & rustup run $toolchain cargo test -p fileos-cli `
    'tests::fixture_scan_emits_path_free_json_and_preserves_source' `
    --locked --offline -- --exact --nocapture 2>&1
  if ($LASTEXITCODE -ne 0) {
    throw "Scan summary JSON probe exited with code $LASTEXITCODE."
  }

  $text = ($output -join [Environment]::NewLine)
  $match = [regex]::Match(
    $text,
    '(?s)MH_FILEOS_SCAN_JSON_BEGIN\r?\n(.*?)MH_FILEOS_SCAN_JSON_END'
  )
  if (-not $match.Success) {
    throw 'Scan summary JSON probe markers were not found in Cargo test output.'
  }

  $json = $match.Groups[1].Value
  $summary = $json | ConvertFrom-Json
  if ($summary.schema_version -ne 1 -or
      $summary.status -ne 'completed' -or
      $summary.counters.files -le 0 -or
      $summary.counters.partial_issues -ne 0 -or
      $summary.resource_limits.metadata_workers -le 0 -or
      $summary.resource_usage.open_directories_high_water -le 0) {
    throw 'Scan summary JSON parsed but required deterministic evidence fields were missing.'
  }

  if ($json -match '"(root|path|filename|timestamp|duration|run_id|raw_os_code)"\s*:') {
    throw 'Scan summary JSON exposed a forbidden path, identity, time, or raw OS field.'
  }
}

function Test-M6VerticalSliceJson {
  $toolchain = '1.97.0-x86_64-pc-windows-msvc'
  $output = & rustup run $toolchain cargo run --quiet -p fileos-cli `
    --example fixture-catalog-demo --locked --offline 2>&1
  if ($LASTEXITCODE -ne 0) {
    throw "M6 fixture catalog demo exited with code $LASTEXITCODE."
  }

  $json = ($output -join [Environment]::NewLine).Trim()
  $evidence = $json | ConvertFrom-Json
  if ($evidence.schema_version -ne 1 -or
      $evidence.status -ne 'verified' -or
      $evidence.scan_status -ne 'completed' -or
      $evidence.catalog_schema_version -ne 1 -or
      $evidence.catalog_summary.entries -le 0 -or
      $evidence.catalog_summary.files -le 0 -or
      $evidence.catalog_summary.directories -le 0 -or
      $evidence.catalog_summary.bytes -le 0 -or
      $evidence.catalog_reopened -ne $true -or
      $evidence.integrity_check_passed -ne $true -or
      $evidence.snapshot_matches_manifest -ne $true -or
      $evidence.summary_matches_manifest -ne $true -or
      $evidence.source_unchanged -ne $true -or
      $evidence.sandbox_cleaned -ne $true) {
    throw 'M6 fixture catalog JSON parsed but required vertical-slice evidence was missing.'
  }

  if ($json -match '"(root|path|filename|timestamp|duration|run_id|raw_os_code)"\s*:') {
    throw 'M6 fixture catalog JSON exposed a forbidden path, identity, time, or raw OS field.'
  }
}

Push-Location -LiteralPath $repoRoot
try {
  Invoke-VerificationStage -Name 'workspace/toolchain/fixture tests' -Action {
    & (Join-Path $PSScriptRoot 'verify-workspace.ps1')
  }
  Invoke-VerificationStage -Name 'Markdown local links' -Action {
    Test-MarkdownLocalLinks
  }
  Invoke-VerificationStage -Name 'fixture manifest JSON' -Action {
    Test-FixtureManifestJson
  }
  Invoke-VerificationStage -Name 'scan summary JSON' -Action {
    Test-ScanSummaryJson
  }
  Invoke-VerificationStage -Name 'M6 fixture catalog demo JSON' -Action {
    Test-M6VerticalSliceJson
  }
  Invoke-VerificationStage -Name 'fixture runtime residue' -Action {
    Test-NoFixtureRunResidue
  }
  Invoke-VerificationStage -Name 'Git whitespace errors' -Action {
    & git diff --check
    if ($LASTEXITCODE -ne 0) {
      throw "git diff --check exited with code $LASTEXITCODE."
    }
  }
  Write-Host '[verify] ALL LOCAL GATES PASSED'
}
finally {
  Pop-Location
}
