param(
    [string]$OutputDirectory = "",
    [int]$PreparedRebuildSamples = 20,
    [int]$PreparedHitSamples = 2000,
    [switch]$AllowDirtyTree,
    [switch]$ValidateOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw "The Phase-1 target evidence runner must be executed on Windows."
}
if ($PreparedRebuildSamples -le 0 -or $PreparedHitSamples -le 0) {
    throw "PreparedRebuildSamples and PreparedHitSamples must both be greater than zero."
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$preparedRunner = Join-Path $repoRoot "benchmarks\prepared-cache\run-windows.ps1"
$rendererRunner = Join-Path $repoRoot "benchmarks\adr-019\run-windows.ps1"
$fidelityRunner = Join-Path $repoRoot "benchmarks\phase-1\export-fidelity-scene-windows.ps1"
$evidenceVerifier = Join-Path $repoRoot "benchmarks\phase-1\verify-target-evidence.ps1"
$decisionReviewPreparer = Join-Path $repoRoot "benchmarks\phase-1\prepare-renderer-decision-review.ps1"
$evidenceCommon = Join-Path $repoRoot "benchmarks\phase-1\evidence-common.ps1"
$rootLockfile = Join-Path $repoRoot "Cargo.lock"
$desktopLockfile = Join-Path $repoRoot "apps\desktop\src-tauri\Cargo.lock"

function Assert-ScriptParses([string]$Path) {
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

function Read-EvidenceValue([string[]]$Lines, [string]$Key) {
    $prefix = "$Key="
    $matches = @($Lines | Where-Object { $_.StartsWith($prefix, [System.StringComparison]::Ordinal) })
    if ($matches.Count -ne 1) {
        throw "PreparedPage evidence must contain exactly one '$Key=' line."
    }
    return $matches[0].Substring($prefix.Length)
}

function Read-EvidenceJson([string[]]$Lines, [string]$Prefix) {
    $matches = @($Lines | Where-Object { $_.StartsWith($Prefix, [System.StringComparison]::Ordinal) })
    if ($matches.Count -ne 1) {
        throw "PreparedPage evidence must contain exactly one '$Prefix' metadata line."
    }
    return $matches[0].Substring($Prefix.Length) | ConvertFrom-Json
}

function Get-SingleEvidenceFile([string]$Directory, [string]$Filter, [string]$Label) {
    $files = @(Get-ChildItem -LiteralPath $Directory -Filter $Filter -File -ErrorAction SilentlyContinue)
    if ($files.Count -ne 1) {
        throw "Expected exactly one $Label evidence file in '$Directory', found $($files.Count)."
    }
    return $files[0]
}

function Get-GitBlob([string]$RepoPath, [string]$Label) {
    $blob = (& git rev-parse "HEAD:$RepoPath").Trim()
    if ($LASTEXITCODE -ne 0 -or $blob -notmatch '^[0-9a-fA-F]{40}$') {
        throw "Unable to resolve the committed $Label Git blob."
    }
    return $blob.ToLowerInvariant()
}

foreach ($requiredPath in @(
    $preparedRunner,
    $rendererRunner,
    $fidelityRunner,
    $evidenceVerifier,
    $decisionReviewPreparer,
    $evidenceCommon,
    $rootLockfile,
    $desktopLockfile
)) {
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
        throw "Required Phase-1 target evidence file is missing: $requiredPath"
    }
}
Assert-ScriptParses $preparedRunner
Assert-ScriptParses $rendererRunner
Assert-ScriptParses $fidelityRunner
Assert-ScriptParses $evidenceVerifier
Assert-ScriptParses $decisionReviewPreparer
Assert-ScriptParses $evidenceCommon
. $evidenceCommon

Push-Location $repoRoot
try {
    $commitSha = (& git rev-parse --verify HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $commitSha -notmatch '^[0-9a-fA-F]{40}$') {
        throw "Unable to resolve a 40-character Git commit for Phase-1 target evidence."
    }
    $commitSha = $commitSha.ToLowerInvariant()

    $workingTree = (& git status --porcelain --untracked-files=normal | Out-String).TrimEnd()
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to inspect the Git working tree."
    }
    $workingTreeClean = [string]::IsNullOrWhiteSpace($workingTree)
    if (-not $workingTreeClean -and -not $AllowDirtyTree) {
        throw "Phase-1 target evidence requires a clean Git working tree. Commit/stash changes or use -AllowDirtyTree for diagnostics only."
    }

    $rootLockBlob = Get-GitBlob "Cargo.lock" "root Cargo.lock"
    $desktopLockBlob = Get-GitBlob "apps/desktop/src-tauri/Cargo.lock" "desktop Cargo.lock"

    if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
        $OutputDirectory = Join-Path $repoRoot "benchmark-results\phase-1-target"
    }
    elseif (-not [System.IO.Path]::IsPathRooted($OutputDirectory)) {
        $OutputDirectory = Join-Path $repoRoot $OutputDirectory
    }
    $OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)

    Write-Host "Phase-1 source commit: $commitSha"
    Write-Host "Phase-1 root Cargo.lock Git blob: $rootLockBlob"
    Write-Host "Phase-1 desktop Cargo.lock Git blob: $desktopLockBlob"
    Write-Host "Phase-1 target evidence root: $OutputDirectory"

    if ($ValidateOnly) {
        if ($AllowDirtyTree) {
            & $rendererRunner -ValidateOnly -AllowDirtyTree -OutputDirectory (Join-Path $OutputDirectory "adr-019")
            & $fidelityRunner -ValidateOnly -AllowDirtyTree -OutputDirectory (Join-Path $OutputDirectory "fidelity")
        }
        else {
            & $rendererRunner -ValidateOnly -OutputDirectory (Join-Path $OutputDirectory "adr-019")
            & $fidelityRunner -ValidateOnly -OutputDirectory (Join-Path $OutputDirectory "fidelity")
        }
        Write-Host "Phase-1 combined target runner configuration is valid. No target benchmark evidence was generated."
        return
    }

    $timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $sessionName = "target-$timestamp-$($commitSha.Substring(0, 12))"
    $sessionDirectory = Join-Path $OutputDirectory $sessionName
    $preparedDirectory = Join-Path $sessionDirectory "prepared-cache"
    $rendererDirectory = Join-Path $sessionDirectory "adr-019"
    $fidelityDirectory = Join-Path $sessionDirectory "fidelity"
    $null = New-Item -ItemType Directory -Force -Path $preparedDirectory
    $null = New-Item -ItemType Directory -Force -Path $rendererDirectory
    $null = New-Item -ItemType Directory -Force -Path $fidelityDirectory

    Write-Host "Running PreparedPage target benchmark in RELEASE mode..."
    if ($AllowDirtyTree) {
        & $preparedRunner `
            -Counts @(5000, 20000) `
            -RebuildSamples $PreparedRebuildSamples `
            -HitSamples $PreparedHitSamples `
            -OutputDirectory $preparedDirectory `
            -AllowDirtyTree
    }
    else {
        & $preparedRunner `
            -Counts @(5000, 20000) `
            -RebuildSamples $PreparedRebuildSamples `
            -HitSamples $PreparedHitSamples `
            -OutputDirectory $preparedDirectory
    }

    $preparedFile = Get-SingleEvidenceFile $preparedDirectory "prepared-cache-*.txt" "PreparedPage"
    $preparedLines = @(Get-Content -LiteralPath $preparedFile.FullName)
    if ($preparedLines.Count -eq 0 -or $preparedLines[0] -ne "DIAGRAMDESIGNER_NEXT_PREPARED_CACHE_EVIDENCE_V1") {
        throw "Unexpected PreparedPage evidence schema in '$($preparedFile.FullName)'."
    }
    if ((Read-EvidenceValue $preparedLines "commit_sha") -ne $commitSha) {
        throw "PreparedPage evidence source commit does not match the combined target run."
    }
    if ((Read-EvidenceValue $preparedLines "cargo_lock_blob") -ne $rootLockBlob) {
        throw "PreparedPage evidence root Cargo.lock provenance does not match the combined target run."
    }
    if ((Read-EvidenceValue $preparedLines "working_tree_clean") -ne [string]$workingTreeClean) {
        throw "PreparedPage evidence working-tree state does not match the combined target run."
    }
    if ((Read-EvidenceValue $preparedLines "build_profile") -ne "release") {
        throw "PreparedPage evidence must use the release build profile for Phase-1 target evidence."
    }
    if ([int](Read-EvidenceValue $preparedLines "cargo_exit_code") -ne 0) {
        throw "PreparedPage evidence records a non-zero cargo exit code."
    }
    $preparedMetrics = Get-PreparedCacheMetrics -Lines $preparedLines
    if ([Int64]$preparedMetrics.rebuildSamples -ne $PreparedRebuildSamples -or
        [Int64]$preparedMetrics.hitSamples -ne $PreparedHitSamples) {
        throw "PreparedPage structured metrics do not match the requested target sample counts."
    }

    Write-Host "PreparedPage target evidence validated. Starting native ADR-019 renderer measurement..."
    if ($AllowDirtyTree) {
        & $rendererRunner -OutputDirectory $rendererDirectory -AllowDirtyTree
    }
    else {
        & $rendererRunner -OutputDirectory $rendererDirectory
    }

    $rendererFile = Get-SingleEvidenceFile $rendererDirectory "renderer-*.json" "ADR-019 renderer"
    $rendererReport = Get-Content -LiteralPath $rendererFile.FullName -Raw | ConvertFrom-Json
    if ($rendererReport.report -ne "diagramdesigner-next-adr-019-native-v1") {
        throw "Unexpected ADR-019 renderer report schema in '$($rendererFile.FullName)'."
    }
    if ($rendererReport.environment.sourceCommit -ne $commitSha) {
        throw "ADR-019 evidence source commit does not match the combined target run."
    }
    if ($rendererReport.buildProvenance.desktopCargoLockGitBlob -ne $desktopLockBlob) {
        throw "ADR-019 evidence desktop Cargo.lock provenance does not match the combined target run."
    }
    $expectedSourceDirty = -not $workingTreeClean
    if ($rendererReport.environment.sourceDirty -ne $expectedSourceDirty) {
        throw "ADR-019 evidence dirty-state provenance does not match the combined target run."
    }
    if ($rendererReport.environment.runtime -ne "tauri-webview2") {
        throw "Combined Phase-1 renderer evidence must come from Tauri/WebView2."
    }
    if ([int]$rendererReport.environment.clientWidthPx -lt 3840 -or [int]$rendererReport.environment.clientHeightPx -lt 2160) {
        throw "Combined Phase-1 renderer evidence requires a physical client area of at least 3840x2160."
    }
    if (@($rendererReport.measurements).Count -ne 4) {
        throw "Combined Phase-1 renderer evidence must contain exactly four benchmark cases."
    }
    if ([string]::IsNullOrWhiteSpace([string]$rendererReport.performanceVerdict.status)) {
        throw "Combined Phase-1 renderer evidence is missing its mechanical performance verdict."
    }
    if ($rendererReport.finalRendererDecision -ne "not-made-by-benchmark") {
        throw "ADR-019 must not make the final renderer decision."
    }

    Write-Host "ADR-019 target evidence validated. Exporting deterministic renderer fidelity evidence..."
    if ($AllowDirtyTree) {
        & $fidelityRunner -OutputDirectory $fidelityDirectory -AllowDirtyTree
    }
    else {
        & $fidelityRunner -OutputDirectory $fidelityDirectory
    }

    $fidelitySessions = @(
        Get-ChildItem -LiteralPath $fidelityDirectory -Directory -ErrorAction SilentlyContinue |
            Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName "adr-019-fidelity-evidence.json") -PathType Leaf }
    )
    if ($fidelitySessions.Count -ne 1) {
        throw "Expected exactly one ADR-019 fidelity session in '$fidelityDirectory', found $($fidelitySessions.Count)."
    }
    $fidelityManifestPath = Join-Path $fidelitySessions[0].FullName "adr-019-fidelity-evidence.json"
    $fidelityManifest = Get-Content -LiteralPath $fidelityManifestPath -Raw | ConvertFrom-Json
    if ($fidelityManifest.manifest -ne "diagramdesigner-next-adr-019-fidelity-v1") {
        throw "Unexpected ADR-019 fidelity manifest schema."
    }
    if ($fidelityManifest.source.commit -ne $commitSha) {
        throw "ADR-019 fidelity evidence source commit does not match the combined target run."
    }
    if ($fidelityManifest.source.rootCargoLockGitBlob -ne $rootLockBlob) {
        throw "ADR-019 fidelity root Cargo.lock provenance does not match the combined target run."
    }
    if ([bool]$fidelityManifest.source.workingTreeCleanAtStart -ne $workingTreeClean) {
        throw "ADR-019 fidelity working-tree state does not match the combined target run."
    }
    if ([bool]$fidelityManifest.source.eligibleForPhase1Decision -ne $workingTreeClean) {
        throw "ADR-019 fidelity decision eligibility does not match the combined target run."
    }
    if ($fidelityManifest.manualReview.status -ne "not-reviewed-by-runner" -or
        $fidelityManifest.finalRendererDecision -ne "not-made-by-runner") {
        throw "ADR-019 fidelity automation must not claim the manual review or final renderer decision."
    }

    $commitAfter = (& git rev-parse --verify HEAD).Trim().ToLowerInvariant()
    if ($LASTEXITCODE -ne 0 -or $commitAfter -ne $commitSha) {
        throw "Repository HEAD changed while Phase-1 target evidence was being collected."
    }

    $preparedSha256 = (Get-FileHash -LiteralPath $preparedFile.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    $rendererSha256 = (Get-FileHash -LiteralPath $rendererFile.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    $fidelityManifestSha256 = (Get-FileHash -LiteralPath $fidelityManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()

    $os = Read-EvidenceJson $preparedLines "META os="
    $cpu = Read-EvidenceJson $preparedLines "META cpu="
    $computer = Read-EvidenceJson $preparedLines "META computer="
    $gpu = Read-EvidenceJson $preparedLines "META gpu="
    $powerScheme = Read-EvidenceValue $preparedLines "META power_scheme"
    $eligibleForPhase1Decision = $workingTreeClean -and ($rendererReport.environment.sourceDirty -eq $false)

    $fidelityRelativeManifest = "fidelity/$($fidelitySessions[0].Name)/adr-019-fidelity-evidence.json"
    $manifest = [ordered]@{
        manifest = "diagramdesigner-next-phase-1-target-evidence-v1"
        generatedAt = [DateTime]::UtcNow.ToString("o")
        source = [ordered]@{
            commit = $commitSha
            workingTreeCleanAtStart = $workingTreeClean
            rootCargoLockGitBlob = $rootLockBlob
            desktopCargoLockGitBlob = $desktopLockBlob
            eligibleForPhase1Decision = $eligibleForPhase1Decision
            diagnosticOnly = -not $eligibleForPhase1Decision
        }
        preparedPage = [ordered]@{
            evidenceFile = "prepared-cache/$($preparedFile.Name)"
            sha256 = $preparedSha256
            buildProfile = "release"
            rebuildSamples = $PreparedRebuildSamples
            hitSamples = $PreparedHitSamples
            counts = @(5000, 20000)
            metrics = $preparedMetrics
        }
        renderer = [ordered]@{
            evidenceFile = "adr-019/$($rendererFile.Name)"
            sha256 = $rendererSha256
            runtime = $rendererReport.environment.runtime
            sourceDirty = [bool]$rendererReport.environment.sourceDirty
            clientWidthPx = [int]$rendererReport.environment.clientWidthPx
            clientHeightPx = [int]$rendererReport.environment.clientHeightPx
            performanceVerdict = [string]$rendererReport.performanceVerdict.status
            finalRendererDecision = [string]$rendererReport.finalRendererDecision
        }
        fidelity = [ordered]@{
            evidenceManifest = $fidelityRelativeManifest
            sha256 = $fidelityManifestSha256
            candidate = [string]$fidelityManifest.renderer.candidate
            eligibleForPhase1Decision = [bool]$fidelityManifest.source.eligibleForPhase1Decision
            manualReviewStatus = [string]$fidelityManifest.manualReview.status
            finalRendererDecision = [string]$fidelityManifest.finalRendererDecision
        }
        hardware = [ordered]@{
            operatingSystem = $os
            cpu = $cpu
            computer = $computer
            gpu = $gpu
            powerScheme = $powerScheme
            benchmarkMachineModel = $rendererReport.hardware.machineModel
            benchmarkNotes = $rendererReport.hardware.notes
        }
        phase1RendererDecision = "not-made-by-runner"
        reviewRequired = @(
            "PreparedPage release rebuild/cache measurements on representative hardware",
            "ADR-019 mechanical SVG performance verdict",
            "renderer correctness and fidelity evidence",
            "manual review of the retained ADR-019 fidelity scene"
        )
    }

    $manifestPath = Join-Path $sessionDirectory "phase-1-target-evidence.json"
    $manifestJson = $manifest | ConvertTo-Json -Depth 12
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($manifestPath, "$manifestJson`n", $utf8)

    & $evidenceVerifier -SessionDirectory $sessionDirectory

    $decisionReviewPath = Join-Path $sessionDirectory "adr-019-renderer-decision-review.md"
    & $decisionReviewPreparer -SessionDirectory $sessionDirectory -OutputFile $decisionReviewPath

    Write-Host "Phase-1 combined target evidence retained at: $sessionDirectory"
    Write-Host "PreparedPage SHA-256: $preparedSha256"
    Write-Host "ADR-019 SHA-256: $rendererSha256"
    Write-Host "Fidelity manifest SHA-256: $fidelityManifestSha256"
    Write-Host "Combined manifest: $manifestPath"
    Write-Host "Prefilled renderer review: $decisionReviewPath"
    Write-Host "Eligible for Phase-1 decision: $eligibleForPhase1Decision"
    Write-Host "Renderer performance verdict: $($rendererReport.performanceVerdict.status)"
    Write-Host "No renderer decision was made by this runner; manual correctness/fidelity review remains mandatory."
}
finally {
    Pop-Location
}
