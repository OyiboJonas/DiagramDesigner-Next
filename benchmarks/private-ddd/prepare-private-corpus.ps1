param(
    [string]$ManifestPath = "",
    [string]$OutputDirectory = "",
    [string]$FallbackEncoding = "windows-1252",
    [switch]$ValidateOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$rootLockfile = Join-Path $repoRoot "Cargo.lock"
$ddMigrateManifest = Join-Path $repoRoot "crates\dd-migrate\Cargo.toml"

foreach ($requiredPath in @($rootLockfile, $ddMigrateManifest)) {
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
        throw "Required private-corpus harness file is missing: $requiredPath"
    }
}

function Write-Utf8NoBom([string]$Path, [string]$Content) {
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $utf8)
}

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot "benchmark-results\private-ddd"
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot $OutputDirectory
}
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)

Push-Location $repoRoot
try {
    & cargo build --locked --quiet -p dd-migrate
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to build the locked dd-migrate corpus verifier."
    }

    $binaryName = if ([System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT) {
        "dd-migrate.exe"
    }
    else {
        "dd-migrate"
    }
    $inspector = Join-Path (Join-Path $repoRoot "target") (Join-Path "debug" $binaryName)
    if (-not (Test-Path -LiteralPath $inspector -PathType Leaf)) {
        throw "Built dd-migrate binary is missing: $inspector"
    }

    if ($ValidateOnly) {
        Write-Host "Private corpus harness configuration is valid. No private manifest or fixture was read."
        return
    }

    if ([string]::IsNullOrWhiteSpace($ManifestPath)) {
        throw "-ManifestPath is required unless -ValidateOnly is used."
    }
    $ManifestPath = [System.IO.Path]::GetFullPath($ManifestPath)
    if (-not (Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
        throw "Private corpus manifest does not exist: $ManifestPath"
    }

    $null = New-Item -ItemType Directory -Force -Path $OutputDirectory
    $timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $sessionDirectory = Join-Path $OutputDirectory "private-ddd-$timestamp"
    $null = New-Item -ItemType Directory -Force -Path $sessionDirectory

    $verificationLines = @(& $inspector verify-corpus $ManifestPath --fallback-encoding $FallbackEncoding)
    if ($LASTEXITCODE -ne 0) {
        throw "Private corpus verification failed."
    }

    $summaryPath = Join-Path $sessionDirectory "private-corpus-summary.txt"
    $summary = @(
        "DIAGRAMDESIGNER_NEXT_PRIVATE_CORPUS_REVIEW_V1",
        "fallback_encoding=$FallbackEncoding",
        "This output is local review material and must not be committed unless every field has been approved for publication.",
        "No legacy document bytes or decoded document contents are copied by this harness.",
        ""
    ) + $verificationLines
    Write-Utf8NoBom $summaryPath (($summary -join "`n") + "`n")

    Write-Host "Private corpus verification completed: $sessionDirectory"
    Write-Host "Local review summary: $summaryPath"
    Write-Host "Keep private manifests, fixture paths, document names and fingerprints outside the public repository unless explicitly approved."
}
finally {
    Pop-Location
}
