[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$toolchain = '1.97.0-x86_64-pc-windows-msvc'
$expectedMembers = @(
  'apps/fileos-cli',
  'crates/fileos-domain',
  'crates/fileos-app',
  'crates/fileos-testkit',
  'crates/fileos-catalog',
  'crates/fileos-scanner',
  'crates/fileos-platform-windows'
)
$expectedPackages = @($expectedMembers | ForEach-Object { Split-Path -Leaf $_ })
$docsOnlyPackages = @()
$expectedFilesByPackage = @{
  'fileos-cli' = @('Cargo.toml', 'examples/fixture_catalog_demo.rs', 'src/main.rs')
  'fileos-domain' = @('Cargo.toml', 'src/lib.rs', 'src/scan.rs')
  'fileos-app' = @('Cargo.toml', 'src/lib.rs', 'src/scan.rs')
  'fileos-testkit' = @(
    'Cargo.toml',
    'src/digest.rs',
    'src/error.rs',
    'src/lib.rs',
    'src/manifest.rs',
    'src/sandbox.rs',
    'src/snapshot.rs',
    'tests/fixture_generator.rs'
  )
  'fileos-catalog' = @(
    'Cargo.toml',
    'migrations/0001_catalog_v1.sql',
    'src/catalog.rs',
    'src/error.rs',
    'src/lib.rs',
    'src/migration.rs',
    'src/model.rs',
    'tests/catalog_v1.rs'
  )
  'fileos-scanner' = @('Cargo.toml', 'src/lib.rs', 'src/tests.rs')
  'fileos-platform-windows' = @('Cargo.toml', 'src/lib.rs')
}
$expectedDependenciesByPackage = @{
  'fileos-cli' = @(
    'fileos-app|normal|crates/fileos-app',
    'fileos-catalog|dev|crates/fileos-catalog',
    'fileos-domain|normal|crates/fileos-domain',
    'fileos-scanner|normal|crates/fileos-scanner',
    'fileos-testkit|dev|crates/fileos-testkit'
  )
  'fileos-domain' = @()
  'fileos-app' = @('fileos-domain|normal|crates/fileos-domain')
  'fileos-testkit' = @()
  'fileos-catalog' = @(
    'fileos-app|normal|crates/fileos-app',
    'fileos-domain|normal|crates/fileos-domain',
    'fileos-scanner|dev|crates/fileos-scanner',
    'fileos-testkit|dev|crates/fileos-testkit',
    'rusqlite|normal|registry|=0.40.1|bundled|False'
  )
  'fileos-scanner' = @(
    'fileos-app|normal|crates/fileos-app',
    'fileos-domain|normal|crates/fileos-domain',
    'fileos-platform-windows|normal|crates/fileos-platform-windows',
    'fileos-testkit|dev|crates/fileos-testkit'
  )
  'fileos-platform-windows' = @()
}

function Invoke-Cargo {
  param(
    [Parameter(Mandatory)]
    [string[]] $Arguments
  )

  $output = & rustup run $toolchain cargo @Arguments 2>&1
  $exitCode = $LASTEXITCODE

  if ($exitCode -ne 0) {
    throw "cargo $($Arguments -join ' ') exited with code $exitCode."
  }

  $output | ForEach-Object { Write-Output $_ }
}

function Invoke-Clippy {
  param(
    [Parameter(Mandatory)]
    [string[]] $Arguments
  )

  $clippyPathOutput = & rustup which --toolchain $toolchain cargo-clippy 2>&1
  if ($LASTEXITCODE -ne 0) {
    throw "rustup could not locate cargo-clippy for toolchain '$toolchain'."
  }

  $clippyPath = ($clippyPathOutput | Out-String).Trim()
  if (-not (Test-Path -LiteralPath $clippyPath -PathType Leaf)) {
    throw "cargo-clippy was not found at '$clippyPath'."
  }

  # Invoke the pinned toolchain binary directly. On Windows, routing this through
  # the Cargo proxy can leave a nested `cargo clippy` process waiting indefinitely.
  $output = & $clippyPath clippy @Arguments 2>&1
  $exitCode = $LASTEXITCODE

  if ($exitCode -ne 0) {
    throw "cargo clippy $($Arguments -join ' ') exited with code $exitCode."
  }

  $output | ForEach-Object { Write-Output $_ }
}

function Get-CargoMetadata {
  $output = & rustup run $toolchain cargo metadata --format-version 1 --no-deps --locked --offline
  $exitCode = $LASTEXITCODE

  if ($exitCode -ne 0) {
    throw "cargo metadata exited with code $exitCode."
  }

  return (($output -join [Environment]::NewLine) | ConvertFrom-Json)
}

function Assert-SequenceEqual {
  param(
    [Parameter(Mandatory)]
    [string] $Label,

    [Parameter(Mandatory)]
    [AllowEmptyCollection()]
    [string[]] $Actual,

    [Parameter(Mandatory)]
    [AllowEmptyCollection()]
    [string[]] $Expected
  )

  if ($Actual.Count -ne $Expected.Count) {
    throw "$Label mismatch. Expected '$($Expected -join ', ')'; observed '$($Actual -join ', ')'."
  }

  for ($index = 0; $index -lt $Expected.Count; $index++) {
    if ($Actual[$index] -cne $Expected[$index]) {
      throw "$Label order mismatch at index $index. Expected '$($Expected[$index])'; observed '$($Actual[$index])'."
    }
  }
}

Push-Location -LiteralPath $repoRoot
try {
  & (Join-Path $PSScriptRoot 'verify-toolchain.ps1')

  Invoke-Cargo -Arguments @('fmt', '--all', '--', '--check')

  $workspaceManifest = Get-Content -LiteralPath (Join-Path $repoRoot 'Cargo.toml') -Raw
  $membersMatch = [regex]::Match($workspaceManifest, '(?ms)^members\s*=\s*\[(.*?)\]')
  if (-not $membersMatch.Success) {
    throw 'Root Cargo.toml does not contain a workspace members array.'
  }

  $actualMembers = @(
    [regex]::Matches($membersMatch.Groups[1].Value, '"([^"]+)"') |
      ForEach-Object { $_.Groups[1].Value }
  )
  Assert-SequenceEqual -Label 'Workspace members' -Actual $actualMembers -Expected $expectedMembers

  $metadata = Get-CargoMetadata
  $actualPackages = @($metadata.packages.name | Sort-Object)
  $sortedExpectedPackages = @($expectedPackages | Sort-Object)
  Assert-SequenceEqual -Label 'Workspace packages' -Actual $actualPackages -Expected $sortedExpectedPackages

  $repoPrefix = $repoRoot.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar

  foreach ($package in $metadata.packages) {
    if ($null -ne $package.source) {
      throw "Package '$($package.name)' has an external source '$($package.source)'."
    }

    $manifestPath = [IO.Path]::GetFullPath($package.manifest_path)
    if (-not $manifestPath.StartsWith($repoPrefix, [StringComparison]::OrdinalIgnoreCase)) {
      throw "Package '$($package.name)' manifest is outside the repository."
    }

    $expectedMember = @($expectedMembers | Where-Object { (Split-Path -Leaf $_) -ceq $package.name })
    if ($expectedMember.Count -ne 1) {
      throw "Package '$($package.name)' does not map to exactly one approved workspace member."
    }

    $expectedManifestPath = [IO.Path]::GetFullPath(
      (Join-Path $repoRoot (Join-Path $expectedMember[0] 'Cargo.toml'))
    )
    if (-not $manifestPath.Equals($expectedManifestPath, [StringComparison]::OrdinalIgnoreCase)) {
      throw "Package '$($package.name)' manifest must be '$expectedManifestPath', observed '$manifestPath'."
    }

    if (-not $expectedDependenciesByPackage.ContainsKey($package.name)) {
      throw "Package '$($package.name)' has no dependency allowlist."
    }
    $actualDependencies = @(
      $package.dependencies | ForEach-Object {
        $dependency = $_
        $dependencyKind = if ($null -eq $dependency.kind) { 'normal' } else { [string] $dependency.kind }
        $dependencyPathValue = if ($null -ne $dependency.PSObject.Properties['path']) { $dependency.path } else { $null }
        $dependencySourceValue = if ($null -ne $dependency.PSObject.Properties['source']) { $dependency.source } else { $null }
        if ($null -ne $dependencyPathValue) {
          if ($null -ne $dependencySourceValue) {
            throw "Local dependency '$($dependency.name)' of '$($package.name)' has an external source."
          }
          $dependencyPath = [IO.Path]::GetFullPath($dependencyPathValue)
          if (-not $dependencyPath.StartsWith($repoPrefix, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Dependency '$($dependency.name)' of '$($package.name)' is outside the repository."
          }
          $relativeDependencyPath = [IO.Path]::GetRelativePath($repoRoot, $dependencyPath).Replace('\', '/')
          "$($dependency.name)|$dependencyKind|$relativeDependencyPath"
        }
        else {
          if ($dependencySourceValue -cne 'registry+https://github.com/rust-lang/crates.io-index') {
            throw "External dependency '$($dependency.name)' of '$($package.name)' is not from the approved registry."
          }
          $features = @($dependency.features | Sort-Object) -join ','
          "$($dependency.name)|$dependencyKind|registry|$($dependency.req)|$features|$($dependency.uses_default_features)"
        }
      } | Sort-Object
    )
    $expectedDependencies = @($expectedDependenciesByPackage[$package.name] | Sort-Object)
    Assert-SequenceEqual -Label "Dependencies for $($package.name)" `
      -Actual $actualDependencies -Expected $expectedDependencies

    $targets = @($package.targets)
    $libraryTargets = @($targets | Where-Object { @($_.kind) -contains 'lib' })
    $crateRoot = Split-Path -Parent $manifestPath
    if ($package.name -ceq 'fileos-cli') {
      $binaryTargets = @($targets | Where-Object { @($_.kind) -contains 'bin' })
      $exampleTargets = @($targets | Where-Object { @($_.kind) -contains 'example' })
      if ($targets.Count -ne 2 -or
          $binaryTargets.Count -ne 1 -or
          $binaryTargets[0].name -cne 'fileos-cli' -or
          $exampleTargets.Count -ne 1 -or
          $exampleTargets[0].name -cne 'fixture-catalog-demo') {
        throw "Package 'fileos-cli' must expose the approved read-only binary and M6 fixture example targets."
      }
      $mainPath = Join-Path $crateRoot 'src/main.rs'
      $targetSourcePath = [IO.Path]::GetFullPath($binaryTargets[0].src_path)
      if (-not $targetSourcePath.Equals($mainPath, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Package 'fileos-cli' binary target must be '$mainPath', observed '$targetSourcePath'."
      }
      $examplePath = Join-Path $crateRoot 'examples/fixture_catalog_demo.rs'
      $exampleSourcePath = [IO.Path]::GetFullPath($exampleTargets[0].src_path)
      if (-not $exampleSourcePath.Equals($examplePath, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Package 'fileos-cli' example target must be '$examplePath', observed '$exampleSourcePath'."
      }
    }
    else {
      if ($libraryTargets.Count -ne 1) {
        throw "Package '$($package.name)' must expose exactly one library target."
      }
      if ($package.name -ceq 'fileos-testkit') {
        $testTargets = @($targets | Where-Object { @($_.kind) -contains 'test' })
        if ($targets.Count -ne 2 -or $testTargets.Count -ne 1 -or $testTargets[0].name -cne 'fixture_generator') {
          throw "Package 'fileos-testkit' must expose one library and the approved fixture_generator integration test."
        }
      }
      elseif ($package.name -ceq 'fileos-catalog') {
        $testTargets = @($targets | Where-Object { @($_.kind) -contains 'test' })
        if ($targets.Count -ne 2 -or $testTargets.Count -ne 1 -or $testTargets[0].name -cne 'catalog_v1') {
          throw "Package 'fileos-catalog' must expose one library and the approved catalog_v1 integration test."
        }
      }
      elseif ($targets.Count -ne 1) {
        throw "Package '$($package.name)' has an unapproved Cargo target."
      }

      $libPath = Join-Path $crateRoot 'src/lib.rs'
      $targetSourcePath = [IO.Path]::GetFullPath($libraryTargets[0].src_path)
      if (-not $targetSourcePath.Equals($libPath, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Package '$($package.name)' library target must be '$libPath', observed '$targetSourcePath'."
      }
    }

    $actualFiles = @(
      Get-ChildItem -LiteralPath $crateRoot -Recurse -File |
        ForEach-Object { [IO.Path]::GetRelativePath($crateRoot, $_.FullName).Replace('\', '/') } |
        Sort-Object
    )
    if (-not $expectedFilesByPackage.ContainsKey($package.name)) {
      throw "Package '$($package.name)' has no file inventory allowlist."
    }
    $expectedFiles = @($expectedFilesByPackage[$package.name])
    Assert-SequenceEqual -Label "Files for $($package.name)" -Actual $actualFiles -Expected $expectedFiles

    if ($docsOnlyPackages -ccontains $package.name) {
      $libPath = Join-Path $crateRoot 'src/lib.rs'
      $invalidLines = @(
        Get-Content -LiteralPath $libPath |
          Where-Object {
            $_ -notmatch '^\s*$' -and
            $_ -notmatch '^//!' -and
            $_ -cne '#![forbid(unsafe_code)]'
          }
      )

      if ($invalidLines.Count -ne 0) {
        throw "Package '$($package.name)' contains behavior before its approved milestone."
      }
    }
  }

  Invoke-Cargo -Arguments @('tree', '--workspace', '--all-features', '--edges', 'all', '--locked', '--offline')
  Invoke-Cargo -Arguments @('check', '--workspace', '--all-targets', '--all-features', '--locked', '--offline')
  Invoke-Clippy -Arguments @('--workspace', '--all-targets', '--all-features', '--locked', '--offline', '--', '-D', 'warnings')
  Invoke-Cargo -Arguments @('test', '--workspace', '--all-targets', '--all-features', '--locked', '--offline')

  $previousRustdocFlags = $env:RUSTDOCFLAGS
  try {
    $env:RUSTDOCFLAGS = '-D warnings'
    Invoke-Cargo -Arguments @('doc', '--workspace', '--all-features', '--no-deps', '--locked', '--offline')
  }
  finally {
    if ($null -eq $previousRustdocFlags) {
      Remove-Item Env:RUSTDOCFLAGS -ErrorAction SilentlyContinue
    }
    else {
      $env:RUSTDOCFLAGS = $previousRustdocFlags
    }
  }

  Write-Output 'Workspace contract verified: approved M6 read-only vertical slice and exact pinned dependencies.'
}
finally {
  Pop-Location
}
