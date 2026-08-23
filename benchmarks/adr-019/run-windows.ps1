param(
    [string]$OutputDirectory = "",
    [switch]$AllowDirtyTree,
    [switch]$ValidateOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw "The ADR-019 target evidence runner must be executed on Windows."
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$manifestPath = Join-Path $repoRoot "apps\desktop\src-tauri\Cargo.toml"
$lockfilePath = Join-Path $repoRoot "apps\desktop\src-tauri\Cargo.lock"
$lockfileRepoPath = "apps/desktop/src-tauri/Cargo.lock"
$benchmarkJsPath = Join-Path $repoRoot "apps\desktop\ui\renderer-benchmark.js"
$capabilityPath = Join-Path $repoRoot "apps\desktop\src-tauri\capabilities\renderer-benchmark.json"

Push-Location $repoRoot
try {
    $commitSha = (& git rev-parse --verify HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $commitSha -notmatch '^[0-9a-fA-F]{40}$') {
        throw "Unable to resolve a 40-character Git commit for ADR-019 evidence."
    }
    $commitSha = $commitSha.ToLowerInvariant()

    $workingTree = (& git status --porcelain --untracked-files=normal | Out-String).TrimEnd()
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to inspect the Git working tree."
    }
    $dirty = -not [string]::IsNullOrWhiteSpace($workingTree)
    if ($dirty -and -not $AllowDirtyTree) {
        throw "ADR-019 target evidence requires a clean Git working tree. Commit/stash changes or use -AllowDirtyTree for diagnostics only."
    }

    if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
        $OutputDirectory = Join-Path $repoRoot "benchmark-results\adr-019"
    }
    elseif (-not [System.IO.Path]::IsPathRooted($OutputDirectory)) {
        $OutputDirectory = Join-Path $repoRoot $OutputDirectory
    }
    $OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)

    foreach ($requiredPath in @($manifestPath, $lockfilePath, $benchmarkJsPath, $capabilityPath)) {
        if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
            throw "Required ADR-019 benchmark file is missing: $requiredPath"
        }
    }

    $lockfileBlobSha = (& git rev-parse "HEAD:$lockfileRepoPath").Trim()
    if ($LASTEXITCODE -ne 0 -or $lockfileBlobSha -notmatch '^[0-9a-fA-F]{40}$') {
        throw "Unable to resolve the committed Tauri Cargo.lock Git blob for ADR-019 evidence."
    }
    $lockfileBlobSha = $lockfileBlobSha.ToLowerInvariant()

    $capability = Get-Content -LiteralPath $capabilityPath -Raw | ConvertFrom-Json
    if (@($capability.permissions) -notcontains "allow-persist-renderer-benchmark-evidence") {
        throw "Renderer benchmark capability does not contain the dedicated evidence-persistence permission."
    }

    Write-Host "ADR-019 source commit: $commitSha"
    Write-Host "ADR-019 source dirty: $dirty"
    Write-Host "ADR-019 desktop Cargo.lock Git blob: $lockfileBlobSha"
    Write-Host "ADR-019 evidence directory: $OutputDirectory"

    if ($ValidateOnly) {
        Write-Host "ADR-019 target runner configuration is valid. No GUI benchmark was launched."
        return
    }

    $null = New-Item -ItemType Directory -Force -Path $OutputDirectory
    $before = @{}
    Get-ChildItem -LiteralPath $OutputDirectory -Filter "renderer-*.json" -File -ErrorAction SilentlyContinue |
        ForEach-Object { $before[$_.FullName] = $true }

    $previousCommit = $env:DDN_SOURCE_COMMIT
    $previousDirty = $env:DDN_SOURCE_DIRTY
    $previousLockBlob = $env:DDN_SOURCE_LOCK_BLOB
    $previousEvidenceDirectory = $env:DDN_ADR019_EVIDENCE_DIR
    try {
        $env:DDN_SOURCE_COMMIT = $commitSha
        $env:DDN_SOURCE_DIRTY = if ($dirty) { "true" } else { "false" }
        $env:DDN_SOURCE_LOCK_BLOB = $lockfileBlobSha
        $env:DDN_ADR019_EVIDENCE_DIR = $OutputDirectory

        Write-Host "Cleaning the standalone desktop target so build provenance cannot be stale…"
        & cargo clean --manifest-path $manifestPath
        if ($LASTEXITCODE -ne 0) {
            throw "cargo clean failed for the desktop application."
        }

        Write-Host "Launching the release desktop application with the committed dependency lockfile."
        Write-Host "In DiagramDesigner Next: open '4K benchmark', enter target-hardware notes, run the ADR-019 suite, then close the benchmark and editor windows."
        & cargo run --locked --release --manifest-path $manifestPath
        $cargoExitCode = $LASTEXITCODE
    }
    finally {
        $env:DDN_SOURCE_COMMIT = $previousCommit
        $env:DDN_SOURCE_DIRTY = $previousDirty
        $env:DDN_SOURCE_LOCK_BLOB = $previousLockBlob
        $env:DDN_ADR019_EVIDENCE_DIR = $previousEvidenceDirectory
    }

    $newEvidence = @(Get-ChildItem -LiteralPath $OutputDirectory -Filter "renderer-*.json" -File |
        Where-Object { -not $before.ContainsKey($_.FullName) } |
        Sort-Object LastWriteTimeUtc -Descending)

    if ($newEvidence.Count -eq 0) {
        if ($cargoExitCode -ne 0) {
            throw "Desktop application exited with code $cargoExitCode and produced no new ADR-019 evidence file."
        }
        throw "Desktop application produced no new ADR-019 evidence file. Run the native benchmark suite before closing the application."
    }

    $evidencePath = $newEvidence[0].FullName
    $report = Get-Content -LiteralPath $evidencePath -Raw | ConvertFrom-Json

    if ($report.report -ne "diagramdesigner-next-adr-019-native-v1") {
        throw "Unexpected ADR-019 report schema in $evidencePath"
    }
    if ($report.environment.sourceCommit -ne $commitSha) {
        throw "Evidence source commit does not match the measured checkout. Evidence was retained at $evidencePath"
    }
    if (-not $AllowDirtyTree -and $report.environment.sourceDirty -ne $false) {
        throw "Clean target evidence did not record sourceDirty=false. Evidence was retained at $evidencePath"
    }
    if ($report.buildProvenance.desktopCargoLockGitBlob -ne $lockfileBlobSha) {
        throw "Evidence desktop Cargo.lock provenance does not match the measured checkout. Evidence was retained at $evidencePath"
    }
    if ($report.environment.runtime -ne "tauri-webview2") {
        throw "ADR-019 target evidence must come from Tauri/WebView2. Evidence was retained at $evidencePath"
    }
    if ([int]$report.environment.clientWidthPx -lt 3840 -or [int]$report.environment.clientHeightPx -lt 2160) {
        throw "ADR-019 target evidence requires a physical client area of at least 3840x2160. Evidence was retained at $evidencePath"
    }
    if (@($report.measurements).Count -ne 4) {
        throw "ADR-019 evidence must contain exactly four benchmark cases. Evidence was retained at $evidencePath"
    }
    if ([string]::IsNullOrWhiteSpace([string]$report.performanceVerdict.status)) {
        throw "ADR-019 evidence is missing its mechanical performance verdict. Evidence was retained at $evidencePath"
    }
    if ($report.finalRendererDecision -ne "not-made-by-benchmark") {
        throw "The benchmark must not make the final renderer decision. Evidence was retained at $evidencePath"
    }
    if ($cargoExitCode -ne 0) {
        throw "Desktop application exited with code $cargoExitCode after writing evidence. Evidence was retained at $evidencePath"
    }

    Write-Host "ADR-019 native evidence validated and retained at: $evidencePath"
    Write-Host "Desktop Cargo.lock provenance: $($report.buildProvenance.desktopCargoLockGitBlob)"
    Write-Host "Performance verdict: $($report.performanceVerdict.status)"
    Write-Host "The performance verdict is evidence only; correctness/fidelity review still decides whether SVG is accepted."
}
finally {
    Pop-Location
}
