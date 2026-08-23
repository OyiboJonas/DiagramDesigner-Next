param(
    [Parameter(Mandatory = $true)]
    [string]$SessionDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$evidenceCommon = Join-Path $PSScriptRoot "evidence-common.ps1"
$fidelityVerifier = Join-Path $PSScriptRoot "verify-fidelity-evidence.ps1"
foreach ($requiredPath in @($evidenceCommon, $fidelityVerifier)) {
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
        throw "Phase-1 evidence verifier is missing required parser/verifier: $requiredPath"
    }
}
. $evidenceCommon

function Read-EvidenceValue([string[]]$Lines, [string]$Key) {
    $prefix = "$Key="
    $matches = @($Lines | Where-Object { $_.StartsWith($prefix, [System.StringComparison]::Ordinal) })
    if ($matches.Count -ne 1) {
        throw "PreparedPage evidence must contain exactly one '$Key=' line."
    }
    return $matches[0].Substring($prefix.Length)
}

function Read-EvidenceBoolean([string[]]$Lines, [string]$Key) {
    $rawValue = Read-EvidenceValue $Lines $Key
    $parsedValue = $false
    if (-not [bool]::TryParse($rawValue, [ref]$parsedValue)) {
        throw "PreparedPage evidence '$Key' must be a boolean value."
    }
    return $parsedValue
}

function Read-RequiredBooleanProperty([object]$Object, [string]$Name, [string]$Label) {
    if ($null -eq $Object) {
        throw "$Label is missing."
    }
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property -or $property.Value -isnot [bool]) {
        throw "$Label must be a JSON boolean."
    }
    return [bool]$property.Value
}

function Assert-GitObjectId([string]$Value, [string]$Label) {
    if ($Value -notmatch '^[0-9a-fA-F]{40}$') {
        throw "$Label must be a 40-character Git object id."
    }
}

function Resolve-EvidencePath([string]$Root, [string]$RelativePath, [string]$Label) {
    if ([string]::IsNullOrWhiteSpace($RelativePath)) {
        throw "$Label evidence path is missing from the manifest."
    }
    if ([System.IO.Path]::IsPathRooted($RelativePath)) {
        throw "$Label evidence path must be relative to the archived session directory."
    }

    $normalized = $RelativePath.Replace('/', [System.IO.Path]::DirectorySeparatorChar)
    $rootFull = [System.IO.Path]::GetFullPath($Root)
    $candidate = [System.IO.Path]::GetFullPath((Join-Path $rootFull $normalized))
    $rootPrefix = $rootFull.TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    ) + [System.IO.Path]::DirectorySeparatorChar

    if (-not $candidate.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "$Label evidence path escapes the archived session directory."
    }
    if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
        throw "$Label evidence file is missing: $candidate"
    }
    return $candidate
}

$SessionDirectory = [System.IO.Path]::GetFullPath($SessionDirectory)
if (-not (Test-Path -LiteralPath $SessionDirectory -PathType Container)) {
    throw "Phase-1 evidence session directory does not exist: $SessionDirectory"
}

$manifestPath = Join-Path $SessionDirectory "phase-1-target-evidence.json"
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "Phase-1 evidence manifest is missing: $manifestPath"
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json

if ($manifest.manifest -ne "diagramdesigner-next-phase-1-target-evidence-v1") {
    throw "Unexpected Phase-1 target evidence manifest schema."
}
if ($manifest.phase1RendererDecision -ne "not-made-by-runner") {
    throw "The combined target runner must not make the final renderer decision."
}

$commitSha = [string]$manifest.source.commit
$rootLockBlob = [string]$manifest.source.rootCargoLockGitBlob
$desktopLockBlob = [string]$manifest.source.desktopCargoLockGitBlob
$workingTreeCleanAtStart = Read-RequiredBooleanProperty $manifest.source "workingTreeCleanAtStart" "Combined manifest source workingTreeCleanAtStart"
$eligibleForPhase1Decision = Read-RequiredBooleanProperty $manifest.source "eligibleForPhase1Decision" "Combined manifest source eligibleForPhase1Decision"
$diagnosticOnly = Read-RequiredBooleanProperty $manifest.source "diagnosticOnly" "Combined manifest source diagnosticOnly"
Assert-GitObjectId $commitSha "Source commit"
Assert-GitObjectId $rootLockBlob "Root Cargo.lock blob"
Assert-GitObjectId $desktopLockBlob "Desktop Cargo.lock blob"

$preparedPath = Resolve-EvidencePath $SessionDirectory ([string]$manifest.preparedPage.evidenceFile) "PreparedPage"
$rendererPath = Resolve-EvidencePath $SessionDirectory ([string]$manifest.renderer.evidenceFile) "ADR-019 renderer"

$preparedHash = (Get-FileHash -LiteralPath $preparedPath -Algorithm SHA256).Hash.ToLowerInvariant()
$rendererHash = (Get-FileHash -LiteralPath $rendererPath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($preparedHash -ne ([string]$manifest.preparedPage.sha256).ToLowerInvariant()) {
    throw "PreparedPage evidence SHA-256 does not match the archived manifest."
}
if ($rendererHash -ne ([string]$manifest.renderer.sha256).ToLowerInvariant()) {
    throw "ADR-019 renderer evidence SHA-256 does not match the archived manifest."
}

$preparedLines = @(Get-Content -LiteralPath $preparedPath)
if ($preparedLines.Count -eq 0 -or $preparedLines[0] -ne "DIAGRAMDESIGNER_NEXT_PREPARED_CACHE_EVIDENCE_V1") {
    throw "Unexpected PreparedPage target evidence schema."
}
if ((Read-EvidenceValue $preparedLines "commit_sha") -ne $commitSha) {
    throw "PreparedPage source commit does not match the combined manifest."
}
if ((Read-EvidenceValue $preparedLines "cargo_lock_blob") -ne $rootLockBlob) {
    throw "PreparedPage root Cargo.lock provenance does not match the combined manifest."
}
$preparedWorkingTreeClean = Read-EvidenceBoolean $preparedLines "working_tree_clean"
if ($preparedWorkingTreeClean -ne $workingTreeCleanAtStart) {
    throw "PreparedPage working-tree state does not match the combined manifest."
}
if ((Read-EvidenceValue $preparedLines "build_profile") -ne "release") {
    throw "PreparedPage Phase-1 target evidence must use the release build profile."
}
if ([int](Read-EvidenceValue $preparedLines "cargo_exit_code") -ne 0) {
    throw "PreparedPage evidence records a non-zero cargo exit code."
}
if ((Read-EvidenceValue $preparedLines "counts") -ne "5000,20000") {
    throw "PreparedPage Phase-1 target evidence must contain the 5k and 20k scene counts."
}
if ([string]$manifest.preparedPage.buildProfile -ne "release") {
    throw "Combined manifest does not identify PreparedPage evidence as release profile."
}
if (@($manifest.preparedPage.counts).Count -ne 2 -or
    [int]$manifest.preparedPage.counts[0] -ne 5000 -or
    [int]$manifest.preparedPage.counts[1] -ne 20000) {
    throw "Combined manifest PreparedPage counts do not match the Phase-1 5k/20k contract."
}

$rawPreparedMetrics = Get-PreparedCacheMetrics -Lines $preparedLines
if ([Int64]$manifest.preparedPage.rebuildSamples -ne [Int64]$rawPreparedMetrics.rebuildSamples -or
    [Int64]$manifest.preparedPage.hitSamples -ne [Int64]$rawPreparedMetrics.hitSamples) {
    throw "Combined manifest PreparedPage sample counts do not match raw evidence."
}
if ($null -eq $manifest.preparedPage.metrics) {
    throw "Combined manifest is missing structured PreparedPage metrics."
}
Assert-PreparedCacheMetricsMatch -Expected $manifest.preparedPage.metrics -Actual $rawPreparedMetrics

$renderer = Get-Content -LiteralPath $rendererPath -Raw | ConvertFrom-Json
if ($renderer.report -ne "diagramdesigner-next-adr-019-native-v1") {
    throw "Unexpected ADR-019 renderer evidence schema."
}
if ($renderer.environment.sourceCommit -ne $commitSha) {
    throw "ADR-019 source commit does not match the combined manifest."
}
if ($renderer.buildProvenance.desktopCargoLockGitBlob -ne $desktopLockBlob) {
    throw "ADR-019 desktop Cargo.lock provenance does not match the combined manifest."
}
$rendererSourceDirty = Read-RequiredBooleanProperty $renderer.environment "sourceDirty" "ADR-019 renderer sourceDirty"
$manifestRendererSourceDirty = Read-RequiredBooleanProperty $manifest.renderer "sourceDirty" "Combined manifest renderer sourceDirty"
if ($rendererSourceDirty -ne (-not $workingTreeCleanAtStart)) {
    throw "ADR-019 dirty-state provenance does not match the combined manifest source state."
}
if ($manifestRendererSourceDirty -ne $rendererSourceDirty) {
    throw "Combined manifest renderer sourceDirty does not match raw ADR-019 evidence."
}
$expectedEligibility = $workingTreeCleanAtStart -and (-not $rendererSourceDirty)
if ($eligibleForPhase1Decision -ne $expectedEligibility) {
    throw "Combined manifest Phase-1 decision eligibility does not match source provenance."
}
if ($diagnosticOnly -ne (-not $expectedEligibility)) {
    throw "Combined manifest diagnostic-only classification does not match Phase-1 decision eligibility."
}
if ($renderer.environment.runtime -ne "tauri-webview2") {
    throw "ADR-019 target evidence must come from Tauri/WebView2."
}
if ($renderer.environment.platform -ne "windows") {
    throw "ADR-019 target evidence must come from Windows."
}
if ([int]$renderer.environment.clientWidthPx -lt 3840 -or [int]$renderer.environment.clientHeightPx -lt 2160) {
    throw "ADR-019 target evidence requires a physical client area of at least 3840x2160."
}
if (@($renderer.measurements).Count -ne 4) {
    throw "ADR-019 target evidence must contain exactly four benchmark cases."
}
if ([string]::IsNullOrWhiteSpace([string]$renderer.performanceVerdict.status)) {
    throw "ADR-019 target evidence is missing its mechanical performance verdict."
}
if ($renderer.finalRendererDecision -ne "not-made-by-benchmark") {
    throw "ADR-019 benchmark evidence must not make the final renderer decision."
}

if ([string]$manifest.renderer.runtime -ne [string]$renderer.environment.runtime -or
    [int]$manifest.renderer.clientWidthPx -ne [int]$renderer.environment.clientWidthPx -or
    [int]$manifest.renderer.clientHeightPx -ne [int]$renderer.environment.clientHeightPx -or
    [string]$manifest.renderer.performanceVerdict -ne [string]$renderer.performanceVerdict.status -or
    [string]$manifest.renderer.finalRendererDecision -ne [string]$renderer.finalRendererDecision) {
    throw "Combined manifest renderer summary does not match the raw ADR-019 evidence."
}

$fidelityHash = $null
$fidelityProperty = $manifest.PSObject.Properties["fidelity"]
if ($null -ne $fidelityProperty) {
    $fidelitySummary = $fidelityProperty.Value
    $fidelityManifestPath = Resolve-EvidencePath $SessionDirectory ([string]$fidelitySummary.evidenceManifest) "ADR-019 fidelity manifest"
    $fidelityHash = (Get-FileHash -LiteralPath $fidelityManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($fidelityHash -ne ([string]$fidelitySummary.sha256).ToLowerInvariant()) {
        throw "ADR-019 fidelity manifest SHA-256 does not match the combined manifest."
    }

    $fidelity = Get-Content -LiteralPath $fidelityManifestPath -Raw | ConvertFrom-Json
    if ($fidelity.manifest -ne "diagramdesigner-next-adr-019-fidelity-v1") {
        throw "Unexpected ADR-019 fidelity evidence schema in the combined target archive."
    }
    if ([string]$fidelity.source.commit -ne $commitSha -or
        [string]$fidelity.source.rootCargoLockGitBlob -ne $rootLockBlob) {
        throw "ADR-019 fidelity provenance does not match the combined target archive."
    }
    $fidelityWorkingTreeClean = Read-RequiredBooleanProperty $fidelity.source "workingTreeCleanAtStart" "ADR-019 fidelity workingTreeCleanAtStart"
    $fidelityEligibility = Read-RequiredBooleanProperty $fidelity.source "eligibleForPhase1Decision" "ADR-019 fidelity eligibleForPhase1Decision"
    $fidelityDiagnosticOnly = Read-RequiredBooleanProperty $fidelity.source "diagnosticOnly" "ADR-019 fidelity diagnosticOnly"
    if ($fidelityWorkingTreeClean -ne $workingTreeCleanAtStart -or
        $fidelityEligibility -ne $eligibleForPhase1Decision -or
        $fidelityDiagnosticOnly -ne $diagnosticOnly) {
        throw "ADR-019 fidelity eligibility classification does not match the combined target archive."
    }
    if ([string]$fidelitySummary.candidate -ne [string]$fidelity.renderer.candidate -or
        [bool]$fidelitySummary.eligibleForPhase1Decision -ne $fidelityEligibility -or
        [string]$fidelitySummary.manualReviewStatus -ne [string]$fidelity.manualReview.status -or
        [string]$fidelitySummary.finalRendererDecision -ne [string]$fidelity.finalRendererDecision) {
        throw "Combined manifest fidelity summary does not match the raw fidelity manifest."
    }
    if ([string]$fidelity.manualReview.status -ne "not-reviewed-by-runner" -or
        [string]$fidelity.finalRendererDecision -ne "not-made-by-runner") {
        throw "ADR-019 fidelity automation must not claim manual review completion or the final renderer decision."
    }

    & $fidelityVerifier -SessionDirectory (Split-Path -Parent $fidelityManifestPath)
}

Write-Host "Phase-1 target evidence verified successfully: $SessionDirectory"
Write-Host "Source commit: $commitSha"
Write-Host "Eligible for Phase-1 decision: $eligibleForPhase1Decision"
Write-Host "Diagnostic only: $diagnosticOnly"
Write-Host "PreparedPage SHA-256: $preparedHash"
foreach ($case in @($rawPreparedMetrics.cases)) {
    Write-Host "PreparedPage $($case.nodes): rebuild p95=$($case.rebuildUs.p95) us, cache-hit p95=$($case.cacheHitNs.p95) ns, eviction rebuild=$($case.eviction.rebuildUs) us"
}
Write-Host "ADR-019 SHA-256: $rendererHash"
if ($null -ne $fidelityHash) {
    Write-Host "Fidelity manifest SHA-256: $fidelityHash"
}
Write-Host "Renderer performance verdict: $($renderer.performanceVerdict.status)"
Write-Host "Final renderer decision remains outside the evidence verifier and still requires correctness/fidelity review."
