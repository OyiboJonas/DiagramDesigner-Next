param(
    [string]$OutputDirectory = "",
    [int]$PreparedRebuildSamples = 20,
    [int]$PreparedHitSamples = 2000,
    [switch]$AllowDirtyTree,
    [switch]$ValidateOnly,
    [string]$CargoBuildRoot = "",
    [double]$MinimumFreeSpaceGB = 8,
    [switch]$KeepBuildArtifacts
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw "The cleanup-safe Phase-1 target runner must be executed on Windows."
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$runner = Join-Path $PSScriptRoot "run-target-evidence-windows.ps1"
$ephemeralHelper = Join-Path $PSScriptRoot "ephemeral-cargo-target.ps1"

foreach ($requiredPath in @($runner, $ephemeralHelper)) {
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
}
finally {
    Pop-Location
}
