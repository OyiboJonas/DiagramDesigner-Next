param(
    [switch]$WindowsConfiguration,
    [string]$CargoBuildRoot = "",
    [double]$MinimumFreeSpaceGB = 3,
    [switch]$KeepBuildArtifacts
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$validator = Join-Path $PSScriptRoot "validate-tooling.ps1"
$ephemeralHelper = Join-Path $PSScriptRoot "ephemeral-cargo-target.ps1"

foreach ($requiredPath in @($validator, $ephemeralHelper)) {
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
        throw "Required cleanup-safe validation file is missing: $requiredPath"
    }
}

. $ephemeralHelper

Push-Location $repoRoot
try {
    Invoke-DdnWithEphemeralCargoTarget `
        -BuildRoot $CargoBuildRoot `
        -MinimumFreeSpaceGB $MinimumFreeSpaceGB `
        -KeepBuildArtifacts:$KeepBuildArtifacts `
        -Script {
            & $validator -WindowsConfiguration:$WindowsConfiguration
        }
}
finally {
    Pop-Location
}
