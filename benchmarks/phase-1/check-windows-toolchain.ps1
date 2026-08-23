param(
    [switch]$SkipCompileCheck
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw "The Windows native toolchain preflight must be executed on Windows."
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path

function Require-Command([string]$Name) {
    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if ($null -eq $command) {
        throw "Required command '$Name' was not found in PATH."
    }
    return $command
}

$null = Require-Command "git"
$null = Require-Command "rustc"
$null = Require-Command "cargo"

$rustcVerbose = @(& rustc -vV 2>&1 | ForEach-Object { $_.ToString() })
if ($LASTEXITCODE -ne 0) {
    throw "rustc -vV failed."
}
$hostLines = @($rustcVerbose | Where-Object { $_.StartsWith("host: ", [System.StringComparison]::Ordinal) })
if ($hostLines.Count -ne 1) {
    throw "rustc -vV did not report exactly one host triple."
}
$rustHost = $hostLines[0].Substring("host: ".Length).Trim()

$requiredVisualStudioComponent = $null
$architectureLabel = $null
switch ($rustHost) {
    "aarch64-pc-windows-msvc" {
        $requiredVisualStudioComponent = "Microsoft.VisualStudio.Component.VC.Tools.ARM64"
        $architectureLabel = "ARM64"
    }
    "x86_64-pc-windows-msvc" {
        $requiredVisualStudioComponent = "Microsoft.VisualStudio.Component.VC.Tools.x86.x64"
        $architectureLabel = "x64"
    }
    "i686-pc-windows-msvc" {
        $requiredVisualStudioComponent = "Microsoft.VisualStudio.Component.VC.Tools.x86.x64"
        $architectureLabel = "x86"
    }
    default {
        throw "Phase-1 native Windows evidence requires an MSVC Rust host. Unsupported rustc host: $rustHost"
    }
}

$vswhereCandidates = [System.Collections.Generic.List[string]]::new()
if (-not [string]::IsNullOrWhiteSpace(${env:ProgramFiles(x86)})) {
    $vswhereCandidates.Add((Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"))
}
if (-not [string]::IsNullOrWhiteSpace($env:ProgramFiles)) {
    $vswhereCandidates.Add((Join-Path $env:ProgramFiles "Microsoft Visual Studio\Installer\vswhere.exe"))
}
$vswhere = $vswhereCandidates | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
if ([string]::IsNullOrWhiteSpace([string]$vswhere)) {
    throw "Visual Studio Installer/vswhere was not found. Install Visual Studio Build Tools 2022 with the native C++ tools required for $architectureLabel."
}

$installationOutput = @(& $vswhere -latest -products * -requires $requiredVisualStudioComponent -property installationPath 2>&1 | ForEach-Object { $_.ToString() })
if ($LASTEXITCODE -ne 0) {
    throw "vswhere failed while checking Visual Studio component '$requiredVisualStudioComponent': $($installationOutput -join '; ')"
}
$installationPaths = @($installationOutput | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
if ($installationPaths.Count -ne 1) {
    $componentHint = if ($architectureLabel -eq "ARM64") {
        "Install 'MSVC v143 - VS 2022 C++ ARM64/ARM64EC Build Tools (Latest)' plus a current Windows SDK."
    }
    else {
        "Install the VS 2022 Desktop development with C++ workload including MSVC v143 x64/x86 Build Tools plus a current Windows SDK."
    }
    throw "Visual Studio Build Tools 2022 does not expose the required $architectureLabel component '$requiredVisualStudioComponent'. $componentHint"
}
$installationPath = $installationPaths[0].Trim()
$vcToolsRoot = Join-Path $installationPath "VC\Tools\MSVC"
if (-not (Test-Path -LiteralPath $vcToolsRoot -PathType Container)) {
    throw "Visual C++ tools directory is missing below '$installationPath'."
}
$linkers = @(Get-ChildItem -LiteralPath $vcToolsRoot -Recurse -Filter "link.exe" -File -ErrorAction SilentlyContinue)
if ($linkers.Count -eq 0) {
    throw "No MSVC link.exe was found below '$vcToolsRoot'. Repair the Visual Studio Build Tools C++ installation."
}

Write-Host "Windows native toolchain discovery passed."
Write-Host "Rust host: $rustHost"
Write-Host "Visual Studio architecture: $architectureLabel"
Write-Host "Visual Studio installation: $installationPath"
Write-Host "Detected MSVC linkers: $($linkers.Count)"

if (-not $SkipCompileCheck) {
    Push-Location $repoRoot
    try {
        Write-Host "Compiling the ADR-019 fidelity scene with the locked native toolchain..."
        & cargo check --locked --quiet -p render-svg --example fidelity_scene
        if ($LASTEXITCODE -ne 0) {
            throw "Native render-svg fidelity scene compile check failed for rustc host $rustHost."
        }
    }
    finally {
        Pop-Location
    }
    Write-Host "Native render-svg compile check passed."
}

Write-Host "Windows native toolchain preflight completed successfully."
