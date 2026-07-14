[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot

function Invoke-VersionCommand {
  param(
    [Parameter(Mandatory)]
    [string] $Command,

    [string[]] $Arguments = @()
  )

  $resolved = @(Get-Command -Name $Command -CommandType Application, ExternalScript -ErrorAction Stop)[0]
  $output = & $resolved.Source @Arguments 2>&1
  $exitCode = $LASTEXITCODE

  if ($exitCode -ne 0) {
    throw "$Command exited with code $exitCode."
  }

  return ($output | Out-String).Trim()
}

function Assert-ExactValue {
  param(
    [Parameter(Mandatory)]
    [string] $Label,

    [Parameter(Mandatory)]
    [string] $Actual,

    [Parameter(Mandatory)]
    [string] $Expected
  )

  if ($Actual -cne $Expected) {
    throw "$Label mismatch. Expected '$Expected', observed '$Actual'."
  }
}

function Assert-VersionPattern {
  param(
    [Parameter(Mandatory)]
    [string] $Label,

    [Parameter(Mandatory)]
    [string] $Actual,

    [Parameter(Mandatory)]
    [string] $Pattern
  )

  if ($Actual -notmatch $Pattern) {
    throw "$Label does not match the pinned baseline. Observed '$Actual'."
  }
}

$nodePin = (Get-Content -LiteralPath (Join-Path $repoRoot '.node-version') -Raw).Trim()
$manifest = Get-Content -LiteralPath (Join-Path $repoRoot 'package.json') -Raw | ConvertFrom-Json
$rustToolchain = Get-Content -LiteralPath (Join-Path $repoRoot 'rust-toolchain.toml') -Raw
$rustToolchainName = '1.97.0-x86_64-pc-windows-msvc'

Assert-ExactValue -Label '.node-version' -Actual $nodePin -Expected '24.17.0'
Assert-ExactValue -Label 'packageManager' -Actual $manifest.packageManager -Expected 'pnpm@10.11.0'
Assert-ExactValue -Label 'engines.node' -Actual $manifest.engines.node -Expected '24.17.0'
Assert-ExactValue -Label 'engines.pnpm' -Actual $manifest.engines.pnpm -Expected '10.11.0'

if ($rustToolchain -notmatch '(?m)^channel = "1\.97\.0"\r?$') {
  throw 'rust-toolchain.toml does not pin Rust 1.97.0.'
}

$observed = [ordered]@{
  rustc = Invoke-VersionCommand -Command 'rustup' -Arguments @('run', $rustToolchainName, 'rustc', '--version')
  cargo = Invoke-VersionCommand -Command 'rustup' -Arguments @('run', $rustToolchainName, 'cargo', '--version')
  rustfmt = Invoke-VersionCommand -Command 'rustup' -Arguments @('run', $rustToolchainName, 'rustfmt', '--version')
  clippy = Invoke-VersionCommand -Command 'rustup' -Arguments @('run', $rustToolchainName, 'clippy-driver', '--version')
  node = Invoke-VersionCommand -Command 'node' -Arguments @('--version')
  pnpm = Invoke-VersionCommand -Command 'pnpm' -Arguments @('--version')
}

Assert-VersionPattern -Label 'rustc' -Actual $observed.rustc -Pattern '^rustc 1\.97\.0 '
Assert-VersionPattern -Label 'cargo' -Actual $observed.cargo -Pattern '^cargo 1\.97\.0 '
Assert-VersionPattern -Label 'rustfmt' -Actual $observed.rustfmt -Pattern '^rustfmt 1\.9\.0-stable '
Assert-VersionPattern -Label 'clippy' -Actual $observed.clippy -Pattern '^clippy 0\.1\.97 '
Assert-ExactValue -Label 'node' -Actual $observed.node -Expected 'v24.17.0'
Assert-ExactValue -Label 'pnpm' -Actual $observed.pnpm -Expected '10.11.0'

Write-Output 'Toolchain baseline verified:'
$observed.GetEnumerator() | ForEach-Object {
  Write-Output "  $($_.Key): $($_.Value)"
}
