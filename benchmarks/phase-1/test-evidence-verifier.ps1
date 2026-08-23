Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$common = Join-Path $PSScriptRoot "evidence-common.ps1"
$verifier = Join-Path $PSScriptRoot "verify-target-evidence.ps1"
$decisionPreparer = Join-Path $PSScriptRoot "prepare-renderer-decision-review.ps1"
foreach ($path in @($common, $verifier, $decisionPreparer)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Phase-1 verifier test fixture is missing required script: $path"
    }
}
. $common

function Write-Utf8NoBom([string]$Path, [string]$Content) {
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $utf8)
}

function Assert-Throws([scriptblock]$Action, [string]$ExpectedFragment) {
    try {
        & $Action
    }
    catch {
        if ($_.Exception.Message -notlike "*$ExpectedFragment*") {
            throw "Expected failure containing '$ExpectedFragment', got: $($_.Exception.Message)"
        }
        return
    }
    throw "Expected action to fail with '$ExpectedFragment', but it succeeded."
}

$commit = "1111111111111111111111111111111111111111"
$rootLock = "2222222222222222222222222222222222222222"
$desktopLock = "3333333333333333333333333333333333333333"
$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) "ddn-phase1-verifier-$([Guid]::NewGuid().ToString('N'))"
$preparedDirectory = Join-Path $tempRoot "prepared-cache"
$rendererDirectory = Join-Path $tempRoot "adr-019"
$null = New-Item -ItemType Directory -Force -Path $preparedDirectory
$null = New-Item -ItemType Directory -Force -Path $rendererDirectory

try {
    $preparedPath = Join-Path $preparedDirectory "prepared-cache-fixture.txt"
    $preparedContent = @"
DIAGRAMDESIGNER_NEXT_PREPARED_CACHE_EVIDENCE_V1
captured_local=2026-08-22T12:00:00.0000000+00:00
captured_utc=2026-08-22T12:00:00.0000000Z
commit_sha=$commit
cargo_lock_blob=$rootLock
working_tree_clean=True
build_profile=release
rebuild_samples=2
hit_samples=3
counts=5000,20000
BENCHMARK stdout_begin
BENCH prepared-cache-meta schema=diagramdesigner-next-prepared-cache-v1 cache_capacity=4 rebuild_samples=2 hit_samples=3
BENCH prepared-cache nodes=5000 rebuild_samples=2 rebuild_p50_us=100 rebuild_p95_us=110 rebuild_p99_us=110 rebuild_max_us=120 hit_samples=3 hit_p50_ns=200 hit_p95_ns=220 hit_p99_ns=220 hit_max_ns=230 history_hits=6 history_p50_ns=300 history_p95_ns=330 history_max_ns=340 history_builds=4 eviction_rebuild_us=130 eviction_builds=6 evictions=2
BENCH prepared-cache nodes=20000 rebuild_samples=2 rebuild_p50_us=400 rebuild_p95_us=440 rebuild_p99_us=440 rebuild_max_us=450 hit_samples=3 hit_p50_ns=500 hit_p95_ns=550 hit_p99_ns=550 hit_max_ns=560 history_hits=6 history_p50_ns=600 history_p95_ns=660 history_max_ns=670 history_builds=4 eviction_rebuild_us=470 eviction_builds=6 evictions=2
BENCHMARK stdout_end
cargo_exit_code=0
"@
    $cleanPreparedContent = $preparedContent.TrimStart() + "`n"
    Write-Utf8NoBom $preparedPath $cleanPreparedContent
    $preparedLines = @(Get-Content -LiteralPath $preparedPath)
    $preparedMetrics = Get-PreparedCacheMetrics -Lines $preparedLines

    if ([Int64]$preparedMetrics.cases[0].rebuildUs.p95 -ne 110 -or
        [Int64]$preparedMetrics.cases[1].cacheHitNs.p95 -ne 550) {
        throw "Synthetic PreparedPage parser fixture produced unexpected structured metrics."
    }

    $rendererPath = Join-Path $rendererDirectory "renderer-fixture.json"
    $renderer = [ordered]@{
        report = "diagramdesigner-next-adr-019-native-v1"
        environment = [ordered]@{
            runtime = "tauri-webview2"
            platform = "windows"
            clientWidthPx = 3840
            clientHeightPx = 2160
            scaleFactor = 1.0
            fullscreen = $true
            monitorWidthPx = 3840
            monitorHeightPx = 2160
            monitorName = "Synthetic 4K"
            appVersion = "0.1.0"
            sourceCommit = $commit
            sourceDirty = $false
        }
        buildProvenance = [ordered]@{
            desktopCargoLockGitBlob = $desktopLock
        }
        hardware = [ordered]@{
            machineModel = "Synthetic target"
            notes = "CI verifier fixture"
        }
        measurements = @(
            [ordered]@{
                nodes_requested = 5000
                mode = "culled"
                frame_ms_p95 = 8.25
                long_tasks_observed = 0
                dom_nodes_max = 600
                stage_physical_px = [ordered]@{ width = 3840; height = 2160 }
            },
            [ordered]@{
                nodes_requested = 5000
                mode = "full"
                frame_ms_p95 = 18.0
                long_tasks_observed = 1
                dom_nodes_max = 5000
                stage_physical_px = [ordered]@{ width = 3840; height = 2160 }
            },
            [ordered]@{
                nodes_requested = 20000
                mode = "culled"
                frame_ms_p95 = 11.5
                long_tasks_observed = 0
                dom_nodes_max = 900
                stage_physical_px = [ordered]@{ width = 3840; height = 2160 }
            },
            [ordered]@{
                nodes_requested = 20000
                mode = "full"
                frame_ms_p95 = 32.0
                long_tasks_observed = 2
                dom_nodes_max = 20000
                stage_physical_px = [ordered]@{ width = 3840; height = 2160 }
            }
        )
        performanceVerdict = [ordered]@{
            status = "performance_gate_pass"
            reasons = @(
                "synthetic culled cases satisfy the measured performance conditions",
                "manual correctness/fidelity review remains required"
            )
        }
        finalRendererDecision = "not-made-by-benchmark"
        generatedAt = "2026-08-22T12:00:00.0000000Z"
    }
    $cleanRendererJson = ($renderer | ConvertTo-Json -Depth 10) + "`n"
    Write-Utf8NoBom $rendererPath $cleanRendererJson

    $preparedHash = (Get-FileHash -LiteralPath $preparedPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $rendererHash = (Get-FileHash -LiteralPath $rendererPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $manifestPath = Join-Path $tempRoot "phase-1-target-evidence.json"
    $manifest = [ordered]@{
        manifest = "diagramdesigner-next-phase-1-target-evidence-v1"
        generatedAt = "2026-08-22T12:00:01.0000000Z"
        source = [ordered]@{
            commit = $commit
            workingTreeCleanAtStart = $true
            rootCargoLockGitBlob = $rootLock
            desktopCargoLockGitBlob = $desktopLock
            eligibleForPhase1Decision = $true
            diagnosticOnly = $false
        }
        preparedPage = [ordered]@{
            evidenceFile = "prepared-cache/prepared-cache-fixture.txt"
            sha256 = $preparedHash
            buildProfile = "release"
            rebuildSamples = 2
            hitSamples = 3
            counts = @(5000, 20000)
            metrics = $preparedMetrics
        }
        renderer = [ordered]@{
            evidenceFile = "adr-019/renderer-fixture.json"
            sha256 = $rendererHash
            runtime = "tauri-webview2"
            sourceDirty = $false
            clientWidthPx = 3840
            clientHeightPx = 2160
            performanceVerdict = "performance_gate_pass"
            finalRendererDecision = "not-made-by-benchmark"
        }
        hardware = [ordered]@{}
        phase1RendererDecision = "not-made-by-runner"
        reviewRequired = @("renderer correctness and fidelity evidence")
    }
    $manifestJson = $manifest | ConvertTo-Json -Depth 12
    Write-Utf8NoBom $manifestPath ($manifestJson + "`n")

    & $verifier -SessionDirectory $tempRoot

    # New combined target sessions may retain a separately hashed fidelity archive.
    # Exercise the entire nested verification path before target hardware is used.
    $fidelityDirectory = Join-Path (Join-Path $tempRoot "fidelity") "synthetic"
    $null = New-Item -ItemType Directory -Force -Path $fidelityDirectory
    $fidelitySvgPath = Join-Path $fidelityDirectory "adr-019-fidelity-scene.svg"
    $fidelityDiagnosticsPath = Join-Path $fidelityDirectory "adr-019-fidelity-diagnostics.txt"
    $fidelityManifestPath = Join-Path $fidelityDirectory "adr-019-fidelity-evidence.json"
    Write-Utf8NoBom $fidelitySvgPath "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect /></svg>`n"
    Write-Utf8NoBom $fidelityDiagnosticsPath @"
SVG-DIAGNOSTIC ConnectorMarkerDeferred { synthetic }
SVG-DIAGNOSTIC UnsupportedPrimitive { synthetic }
FIDELITY-SUMMARY rendered=12 skipped=1 plan_diagnostics=0 svg_diagnostics=2
"@
    $fidelitySvgHash = (Get-FileHash -LiteralPath $fidelitySvgPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $fidelityDiagnosticsHash = (Get-FileHash -LiteralPath $fidelityDiagnosticsPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $fidelityManifest = [ordered]@{
        manifest = "diagramdesigner-next-adr-019-fidelity-v1"
        generatedAt = "2026-08-22T12:00:02.0000000Z"
        source = [ordered]@{
            commit = $commit
            workingTreeCleanAtStart = $true
            rootCargoLockGitBlob = $rootLock
            eligibleForPhase1Decision = $true
            diagnosticOnly = $false
        }
        renderer = [ordered]@{
            candidate = "render-svg"
            example = "fidelity_scene"
            renderedElements = 12
            skippedElements = 1
            planDiagnostics = 0
            svgDiagnostics = 2
            svgFile = "adr-019-fidelity-scene.svg"
            svgSha256 = $fidelitySvgHash
            diagnosticsFile = "adr-019-fidelity-diagnostics.txt"
            diagnosticsSha256 = $fidelityDiagnosticsHash
        }
        expectedDiagnostics = @("ConnectorMarkerDeferred", "UnsupportedPrimitive")
        manualReview = [ordered]@{
            status = "not-reviewed-by-runner"
            contract = "docs/architecture/adr-019-fidelity-review.md"
        }
        finalRendererDecision = "not-made-by-runner"
    }
    Write-Utf8NoBom $fidelityManifestPath (($fidelityManifest | ConvertTo-Json -Depth 10) + "`n")
    $fidelityManifestHash = (Get-FileHash -LiteralPath $fidelityManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()

    $combinedWithFidelity = $manifestJson | ConvertFrom-Json
    $combinedWithFidelity | Add-Member -NotePropertyName fidelity -NotePropertyValue ([pscustomobject]@{
        evidenceManifest = "fidelity/synthetic/adr-019-fidelity-evidence.json"
        sha256 = $fidelityManifestHash
        candidate = "render-svg"
        eligibleForPhase1Decision = $true
        manualReviewStatus = "not-reviewed-by-runner"
        finalRendererDecision = "not-made-by-runner"
    })
    Write-Utf8NoBom $manifestPath (($combinedWithFidelity | ConvertTo-Json -Depth 12) + "`n")
    & $verifier -SessionDirectory $tempRoot

    $reviewPath = Join-Path $tempRoot "synthetic-renderer-decision-review.md"
    & $decisionPreparer -SessionDirectory $tempRoot -OutputFile $reviewPath
    if (-not (Test-Path -LiteralPath $reviewPath -PathType Leaf)) {
        throw "Synthetic renderer decision review was not created."
    }
    $reviewText = Get-Content -LiteralPath $reviewPath -Raw -Encoding UTF8
    foreach ($requiredFragment in @(
        $commit,
        "110 µs",
        "440 µs",
        "8.250 ms",
        "11.500 ms",
        "performance_gate_pass",
        $fidelityManifestHash,
        "not-reviewed-by-runner"
    )) {
        if ($reviewText -notlike "*$requiredFragment*") {
            throw "Synthetic renderer decision review is missing expected fragment '$requiredFragment'."
        }
    }
    Assert-Throws {
        & $decisionPreparer -SessionDirectory $tempRoot -OutputFile $reviewPath
    } "already exists"

    $tamperedFidelityHash = $combinedWithFidelity | ConvertTo-Json -Depth 12 | ConvertFrom-Json
    $tamperedFidelityHash.fidelity.sha256 = ("0" * 64)
    Write-Utf8NoBom $manifestPath (($tamperedFidelityHash | ConvertTo-Json -Depth 12) + "`n")
    Assert-Throws { & $verifier -SessionDirectory $tempRoot } "fidelity manifest SHA-256 does not match"

    # Dirty source runs remain verifiable but must be classified as diagnostic-only.
    Write-Utf8NoBom $manifestPath ($manifestJson + "`n")
    $dirtyPreparedContent = $cleanPreparedContent.Replace("working_tree_clean=True", "working_tree_clean=False")
    Write-Utf8NoBom $preparedPath $dirtyPreparedContent
    $dirtyRenderer = $renderer | ConvertTo-Json -Depth 10 | ConvertFrom-Json
    $dirtyRenderer.environment.sourceDirty = $true
    Write-Utf8NoBom $rendererPath (($dirtyRenderer | ConvertTo-Json -Depth 10) + "`n")
    $diagnosticManifest = $manifestJson | ConvertFrom-Json
    $diagnosticManifest.source.workingTreeCleanAtStart = $false
    $diagnosticManifest.source.eligibleForPhase1Decision = $false
    $diagnosticManifest.source.diagnosticOnly = $true
    $diagnosticManifest.preparedPage.sha256 = (Get-FileHash -LiteralPath $preparedPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $diagnosticManifest.renderer.sha256 = (Get-FileHash -LiteralPath $rendererPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $diagnosticManifest.renderer.sourceDirty = $true
    Write-Utf8NoBom $manifestPath (($diagnosticManifest | ConvertTo-Json -Depth 12) + "`n")
    & $verifier -SessionDirectory $tempRoot

    # Restore the clean, decision-eligible fixture for tamper rejection tests.
    Write-Utf8NoBom $preparedPath $cleanPreparedContent
    Write-Utf8NoBom $rendererPath $cleanRendererJson
    Write-Utf8NoBom $manifestPath ($manifestJson + "`n")

    $tamperedEligibility = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $tamperedEligibility.source.eligibleForPhase1Decision = $false
    Write-Utf8NoBom $manifestPath (($tamperedEligibility | ConvertTo-Json -Depth 12) + "`n")
    Assert-Throws { & $verifier -SessionDirectory $tempRoot } "decision eligibility does not match source provenance"

    Write-Utf8NoBom $manifestPath ($manifestJson + "`n")
    $tamperedClassification = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $tamperedClassification.source.diagnosticOnly = $true
    Write-Utf8NoBom $manifestPath (($tamperedClassification | ConvertTo-Json -Depth 12) + "`n")
    Assert-Throws { & $verifier -SessionDirectory $tempRoot } "diagnostic-only classification does not match"

    Write-Utf8NoBom $manifestPath ($manifestJson + "`n")
    $tamperedRendererSummary = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $tamperedRendererSummary.renderer.sourceDirty = $true
    Write-Utf8NoBom $manifestPath (($tamperedRendererSummary | ConvertTo-Json -Depth 12) + "`n")
    Assert-Throws { & $verifier -SessionDirectory $tempRoot } "renderer sourceDirty does not match raw ADR-019 evidence"

    Write-Utf8NoBom $manifestPath ($manifestJson + "`n")
    $tamperedManifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $tamperedManifest.preparedPage.metrics.cases[0].rebuildUs.p95 = 111
    Write-Utf8NoBom $manifestPath (($tamperedManifest | ConvertTo-Json -Depth 12) + "`n")
    Assert-Throws { & $verifier -SessionDirectory $tempRoot } "rebuild metric 'p95' does not match raw evidence"

    Write-Utf8NoBom $manifestPath ($manifestJson + "`n")
    Add-Content -LiteralPath $preparedPath -Value "tampered=true"
    Assert-Throws { & $verifier -SessionDirectory $tempRoot } "PreparedPage evidence SHA-256 does not match"

    Write-Host "Phase-1 evidence verifier synthetic fixture: clean, diagnostic and nested-fidelity archives accepted; prefilled review generated; eligibility, summary, nested-hash and raw-evidence tampering rejected."
}
finally {
    Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
}
