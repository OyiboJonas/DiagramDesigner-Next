param(
    [string]$OutputDirectory = "",
    [int]$PreparedRebuildSamples = 20,
    [int]$PreparedHitSamples = 2000,
    [switch]$AllowDirtyTree,
    [switch]$ValidateOnly,
    [string]$CargoBuildRoot = "",
    [double]$MinimumFreeSpaceGB = 8,
    [switch]$KeepBuildArtifacts,
    [switch]$OpenReviewViewer
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw "The cleanup-safe Phase-1 target runner must be executed on Windows."
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$runner = Join-Path $PSScriptRoot "run-target-evidence-windows.ps1"
$ephemeralHelper = Join-Path $PSScriptRoot "ephemeral-cargo-target.ps1"
$reviewViewer = Join-Path $PSScriptRoot "prepare-fidelity-review-viewer.ps1"

foreach ($requiredPath in @($runner, $ephemeralHelper, $reviewViewer)) {
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
        throw "Required cleanup-safe target runner file is missing: $requiredPath"
    }
}

. $ephemeralHelper

$requiredFreeSpaceGB = if ($ValidateOnly) {
    [Math]::Min($MinimumFreeSpaceGB, 3)
}
else {
    $MinimumFreeSpaceGB
}

$effectiveOutputRoot = if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    Join-Path $repoRoot "benchmark-results\phase-1-target"
}
elseif ([System.IO.Path]::IsPathRooted($OutputDirectory)) {
    $OutputDirectory
}
else {
    Join-Path $repoRoot $OutputDirectory
}
$effectiveOutputRoot = [System.IO.Path]::GetFullPath($effectiveOutputRoot)

$beforeSessions = @{}
if (Test-Path -LiteralPath $effectiveOutputRoot -PathType Container) {
    Get-ChildItem -LiteralPath $effectiveOutputRoot -Directory -Filter "target-*" -ErrorAction SilentlyContinue |
        ForEach-Object { $beforeSessions[$_.FullName] = $true }
}

Push-Location $repoRoot
try {
    Invoke-DdnWithEphemeralCargoTarget `
        -BuildRoot $CargoBuildRoot `
        -MinimumFreeSpaceGB $requiredFreeSpaceGB `
        -KeepBuildArtifacts:$KeepBuildArtifacts `
        -Script {
            & $runner `
                -OutputDirectory $OutputDirectory `
                -PreparedRebuildSamples $PreparedRebuildSamples `
                -PreparedHitSamples $PreparedHitSamples `
                -AllowDirtyTree:$AllowDirtyTree `
                -ValidateOnly:$ValidateOnly
        }

    if (-not $ValidateOnly) {
        $newSessions = @(
            Get-ChildItem -LiteralPath $effectiveOutputRoot -Directory -Filter "target-*" -ErrorAction SilentlyContinue |
                Where-Object {
                    -not $beforeSessions.ContainsKey($_.FullName) -and
                    (Test-Path -LiteralPath (Join-Path $_.FullName "phase-1-target-evidence.json") -PathType Leaf)
                }
        )
        if ($newSessions.Count -ne 1) {
            throw "Expected exactly one newly completed Phase-1 target session for review-viewer generation, found $($newSessions.Count)."
        }

        & $reviewViewer `
            -SessionDirectory $newSessions[0].FullName `
            -Open:$OpenReviewViewer
    }
}
finally {
    Pop-Location
}
