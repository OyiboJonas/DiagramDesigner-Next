param(
    [string]$SessionDirectory = "",
    [switch]$Open,
    [switch]$ValidateOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$viewer = Join-Path $PSScriptRoot "prepare-fidelity-review-viewer-de.ps1"

function Resolve-PowerShellFileSystemPath([string]$Path) {
    if ([string]::IsNullOrWhiteSpace($Path)) {
        throw "Path must not be empty."
    }

    # Windows PowerShell 5.1 does not keep Environment.CurrentDirectory in sync
    # with Set-Location. SessionState resolves relative paths against the actual
    # PowerShell provider location instead of the process working directory.
    return $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Path)
}

if ($ValidateOnly) {
    $relativeProbe = ".\benchmark-results\phase-1-target\probe"
    $expected = [System.IO.Path]::GetFullPath((Join-Path (Get-Location).ProviderPath "benchmark-results\phase-1-target\probe"))
    $actual = Resolve-PowerShellFileSystemPath $relativeProbe
    if (-not [string]::Equals($actual, $expected, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "PowerShell-relative path resolution mismatch. Expected '$expected', got '$actual'."
    }
    if (-not (Test-Path -LiteralPath $viewer -PathType Leaf)) {
        throw "German fidelity review viewer is missing: $viewer"
    }
    Write-Host "German fidelity review launcher path validation passed."
    return
}

if ([string]::IsNullOrWhiteSpace($SessionDirectory)) {
    throw "SessionDirectory is required unless -ValidateOnly is used."
}
if (-not (Test-Path -LiteralPath $viewer -PathType Leaf)) {
    throw "German fidelity review viewer is missing: $viewer"
}

$resolvedSession = Resolve-PowerShellFileSystemPath $SessionDirectory
& $viewer -SessionDirectory $resolvedSession -Open:$Open
