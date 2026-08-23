param(
    [switch]$WindowsConfiguration
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path

function Assert-ScriptParses([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Required Phase-1 tooling script is missing: $Path"
    }
    $tokens = $null
    $errors = $null
    [void][System.Management.Automation.Language.Parser]::ParseFile(
        $Path,
        [ref]$tokens,
        [ref]$errors
    )
    if (@($errors).Count -ne 0) {
        $messages = @($errors | ForEach-Object { $_.Message }) -join "; "
        throw "PowerShell syntax validation failed for ${Path}: $messages"
    }
}

$privateCorpusHarness = Join-Path $repoRoot "benchmarks\private-ddd\prepare-private-corpus.ps1"
$toolchainPreflight = Join-Path $PSScriptRoot "check-windows-toolchain.ps1"
$phase1Scripts = @(
    (Join-Path $PSScriptRoot "evidence-common.ps1"),
    $toolchainPreflight,
    (Join-Path $PSScriptRoot "verify-target-evidence.ps1"),
    (Join-Path $PSScriptRoot "test-evidence-verifier.ps1"),
    (Join-Path $PSScriptRoot "verify-fidelity-evidence.ps1"),
    (Join-Path $PSScriptRoot "test-fidelity-evidence-verifier.ps1"),
    (Join-Path $PSScriptRoot "export-fidelity-scene-windows.ps1"),
    (Join-Path $PSScriptRoot "prepare-renderer-decision-review.ps1"),
    (Join-Path $PSScriptRoot "run-target-evidence-windows.ps1"),
    (Join-Path $repoRoot "benchmarks\prepared-cache\run-windows.ps1"),
    (Join-Path $repoRoot "benchmarks\adr-019\run-windows.ps1"),
    $privateCorpusHarness
)

foreach ($script in $phase1Scripts) {
    Assert-ScriptParses $script
}
Write-Host "PowerShell syntax validation passed for $($phase1Scripts.Count) Phase-1/evidence scripts."

& (Join-Path $PSScriptRoot "test-fidelity-evidence-verifier.ps1")
& (Join-Path $PSScriptRoot "test-evidence-verifier.ps1")
Write-Host "Synthetic Phase-1 archive, fidelity and renderer-review tests passed."

if ($WindowsConfiguration) {
    if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
        throw "-WindowsConfiguration requires Windows."
    }

    # Fail fast on missing Rust/MSVC prerequisites before entering the longer
    # target-runner configuration checks. The preflight maps the native rustc
    # host to the matching Visual Studio component, including ARM64.
    & $toolchainPreflight

    $validationRoot = Join-Path ([System.IO.Path]::GetTempPath()) "ddn-phase1-validate-$([Guid]::NewGuid().ToString('N'))"
    try {
        & (Join-Path $PSScriptRoot "run-target-evidence-windows.ps1") `
            -ValidateOnly `
            -OutputDirectory (Join-Path $validationRoot "target")

        & $privateCorpusHarness `
            -ValidateOnly `
            -OutputDirectory (Join-Path $validationRoot "private-ddd")

        Write-Host "Windows target-runner, fidelity and private-corpus configuration validation passed."
    }
    finally {
        Remove-Item -LiteralPath $validationRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Write-Host "Phase-1 tooling validation completed successfully."
Write-Host "This harness does not create representative target-hardware performance evidence and does not select a renderer."
