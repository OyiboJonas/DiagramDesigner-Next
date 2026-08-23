param(
    [Parameter(Mandatory = $true)]
    [string]$SessionDirectory,
    [string]$OutputFile = "",
    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$verifier = Join-Path $PSScriptRoot "verify-target-evidence.ps1"
if (-not (Test-Path -LiteralPath $verifier -PathType Leaf)) {
    throw "Phase-1 decision-review preparation is missing its archive verifier: $verifier"
}

function Resolve-ArchivedPath([string]$Root, [string]$RelativePath, [string]$Label) {
    if ([string]::IsNullOrWhiteSpace($RelativePath) -or [System.IO.Path]::IsPathRooted($RelativePath)) {
        throw "$Label must be a relative path inside the Phase-1 target session."
    }
    $rootFull = [System.IO.Path]::GetFullPath($Root)
    $candidate = [System.IO.Path]::GetFullPath((Join-Path $rootFull $RelativePath.Replace('/', [System.IO.Path]::DirectorySeparatorChar)))
    $rootPrefix = $rootFull.TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    ) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $candidate.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "$Label escapes the Phase-1 target session."
    }
    if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
        throw "$Label is missing: $candidate"
    }
    return $candidate
}

function Format-NumberInvariant([object]$Value, [int]$Decimals = 3) {
    if ($null -eq $Value) { return "n/a" }
    $number = 0.0
    if (-not [double]::TryParse(
        [string]$Value,
        [System.Globalization.NumberStyles]::Float,
        [System.Globalization.CultureInfo]::InvariantCulture,
        [ref]$number
    )) {
        return [string]$Value
    }
    return $number.ToString("F$Decimals", [System.Globalization.CultureInfo]::InvariantCulture)
}

function Find-CulledCase([object[]]$Measurements, [int]$Count) {
    $matches = @($Measurements | Where-Object {
        [int]$_.nodes_requested -eq $Count -and [string]$_.mode -eq "culled"
    })
    if ($matches.Count -ne 1) {
        throw "Expected exactly one culled $Count ADR-019 measurement, found $($matches.Count)."
    }
    return $matches[0]
}

$SessionDirectory = [System.IO.Path]::GetFullPath($SessionDirectory)
if (-not (Test-Path -LiteralPath $SessionDirectory -PathType Container)) {
    throw "Phase-1 target session does not exist: $SessionDirectory"
}

& $verifier -SessionDirectory $SessionDirectory

$manifestPath = Join-Path $SessionDirectory "phase-1-target-evidence.json"
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$fidelityProperty = $manifest.PSObject.Properties["fidelity"]
if ($null -eq $fidelityProperty) {
    throw "This target session predates integrated fidelity evidence. Run the current combined target runner before preparing a renderer decision review."
}

$rendererPath = Resolve-ArchivedPath $SessionDirectory ([string]$manifest.renderer.evidenceFile) "ADR-019 renderer evidence"
$fidelityManifestPath = Resolve-ArchivedPath $SessionDirectory ([string]$manifest.fidelity.evidenceManifest) "ADR-019 fidelity manifest"
$renderer = Get-Content -LiteralPath $rendererPath -Raw | ConvertFrom-Json
$fidelity = Get-Content -LiteralPath $fidelityManifestPath -Raw | ConvertFrom-Json

$measurements = @($renderer.measurements)
$culled5k = Find-CulledCase $measurements 5000
$culled20k = Find-CulledCase $measurements 20000
$preparedCases = @($manifest.preparedPage.metrics.cases)
$prepared5k = @($preparedCases | Where-Object { [int]$_.nodes -eq 5000 })
$prepared20k = @($preparedCases | Where-Object { [int]$_.nodes -eq 20000 })
if ($prepared5k.Count -ne 1 -or $prepared20k.Count -ne 1) {
    throw "Combined manifest does not contain exactly one PreparedPage 5k and 20k metric case."
}
$prepared5k = $prepared5k[0]
$prepared20k = $prepared20k[0]

$fidelitySvgPath = Resolve-ArchivedPath (Split-Path -Parent $fidelityManifestPath) ([string]$fidelity.renderer.svgFile) "ADR-019 fidelity SVG"
$fidelityDiagnosticsPath = Resolve-ArchivedPath (Split-Path -Parent $fidelityManifestPath) ([string]$fidelity.renderer.diagnosticsFile) "ADR-019 fidelity diagnostics"

if ([string]::IsNullOrWhiteSpace($OutputFile)) {
    $OutputFile = Join-Path $SessionDirectory "adr-019-renderer-decision-review.md"
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputFile)) {
    $OutputFile = Join-Path $SessionDirectory $OutputFile
}
$OutputFile = [System.IO.Path]::GetFullPath($OutputFile)

if ((Test-Path -LiteralPath $OutputFile) -and -not $Force) {
    throw "Decision-review output already exists: $OutputFile. Use -Force only if replacing the local review draft is intentional."
}

$manifestSha256 = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
$rendererSha256 = (Get-FileHash -LiteralPath $rendererPath -Algorithm SHA256).Hash.ToLowerInvariant()
$fidelityManifestSha256 = (Get-FileHash -LiteralPath $fidelityManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
$fidelitySvgSha256 = (Get-FileHash -LiteralPath $fidelitySvgPath -Algorithm SHA256).Hash.ToLowerInvariant()
$fidelityDiagnosticsSha256 = (Get-FileHash -LiteralPath $fidelityDiagnosticsPath -Algorithm SHA256).Hash.ToLowerInvariant()

$sourceCommit = [string]$manifest.source.commit
$eligible = [bool]$manifest.source.eligibleForPhase1Decision
$diagnosticOnly = [bool]$manifest.source.diagnosticOnly
$workingTreeClean = [bool]$manifest.source.workingTreeCleanAtStart
$performanceVerdict = [string]$renderer.performanceVerdict.status
$verdictReasons = @($renderer.performanceVerdict.reasons | ForEach-Object { "- $_" }) -join "`n"

$review = @"
# ADR-019 renderer decision review — $($sourceCommit.Substring(0, 12))

> Generated from a verified Phase-1 target evidence archive. This document is **prefilled evidence, not an automated renderer decision**. Complete the human-review fields before changing ADR-019 or promoting a production renderer.

## Decision identity

- Source commit: $sourceCommit
- Decision date: <complete after review>
- Reviewer(s): <complete after review>
- Combined target evidence session: $SessionDirectory
- Combined manifest SHA-256: $manifestSha256
- GitHub Actions validation run for the same commit: <record run id / URL>

## Evidence eligibility

- workingTreeCleanAtStart: $workingTreeClean
- eligibleForPhase1Decision: $eligible
- diagnosticOnly: $diagnosticOnly
- root Cargo.lock Git blob: $($manifest.source.rootCargoLockGitBlob)
- desktop Cargo.lock Git blob: $($manifest.source.desktopCargoLockGitBlob)

Decision eligibility conclusion: <eligible / do not decide>

## PreparedPage release evidence

| Scene | Rebuild p95 | Cache-hit p95 | Eviction rebuild | Reviewer conclusion |
| --- | ---: | ---: | ---: | --- |
| 5,000 | $($prepared5k.rebuildUs.p95) µs | $($prepared5k.cacheHitNs.p95) ns | $($prepared5k.eviction.rebuildUs) µs | <acceptable / investigate> |
| 20,000 | $($prepared20k.rebuildUs.p95) µs | $($prepared20k.cacheHitNs.p95) ns | $($prepared20k.eviction.rebuildUs) µs | <acceptable / investigate> |

PreparedPage architecture conclusion:

- [ ] immutable rebuild strategy accepted for Phase 1;
- [ ] incremental patching required before renderer promotion;
- [ ] measurement invalid/not representative — repeat target run.

Rationale: <complete from representative evidence>

## ADR-019 native SVG performance evidence

- Renderer report SHA-256: $rendererSha256
- Runtime: $($renderer.environment.runtime)
- Platform: $($renderer.environment.platform)
- Physical client area: $($renderer.environment.clientWidthPx)x$($renderer.environment.clientHeightPx)
- Monitor: $($renderer.environment.monitorName) / $($renderer.environment.monitorWidthPx)x$($renderer.environment.monitorHeightPx)
- Mechanical verdict: $performanceVerdict

| Case | Frame p95 | Long Tasks | DOM max | Physical stage | Reviewer conclusion |
| --- | ---: | ---: | ---: | --- | --- |
| 5,000 culled | $(Format-NumberInvariant $culled5k.frame_ms_p95) ms | $($culled5k.long_tasks_observed) | $($culled5k.dom_nodes_max) | $($culled5k.stage_physical_px.width)x$($culled5k.stage_physical_px.height) | <pass / fail / repeat> |
| 20,000 culled | $(Format-NumberInvariant $culled20k.frame_ms_p95) ms | $($culled20k.long_tasks_observed) | $($culled20k.dom_nodes_max) | $($culled20k.stage_physical_px.width)x$($culled20k.stage_physical_px.height) | <pass / fail / repeat> |

Mechanical verdict reasons:

$verdictReasons

Performance conclusion: <pass / fail / repeat measurement>

## Fidelity evidence identity

- Fidelity manifest: $($manifest.fidelity.evidenceManifest)
- Fidelity manifest SHA-256: $fidelityManifestSha256
- Fidelity SVG SHA-256: $fidelitySvgSha256
- Fidelity diagnostics SHA-256: $fidelityDiagnosticsSha256
- Candidate: $($fidelity.renderer.candidate)
- Fidelity eligibility: $($fidelity.source.eligibleForPhase1Decision)
- Automated review status: $($fidelity.manualReview.status)
- Fidelity SVG: $fidelitySvgPath
- Fidelity diagnostics: $fidelityDiagnosticsPath

Expected diagnostics:

- [ ] ConnectorMarkerDeferred confirmed in retained diagnostics;
- [ ] UnsupportedPrimitive confirmed in retained diagnostics.

Unexpected diagnostics: <none or list>

## Manual correctness/fidelity review

Use docs/architecture/adr-019-fidelity-review.md as the review contract.

| Check | Result | Notes |
| --- | --- | --- |
| Master layer remains behind page-local layer | <correct / blocking / out-of-scope> | <notes> |
| Rounded rectangle and rotations | <...> | <notes> |
| Ellipse stroke/fill/alpha | <...> | <notes> |
| Horizontal and vertical gradients | <...> | <notes> |
| Unicode/XML-sensitive text and page fields | <...> | <notes> |
| Supported connector dash styles | <...> | <notes> |
| Four rotated page-edge sentinels remain visible/clipped correctly | <...> | <notes> |
| Deferred marker remains explicit diagnostic | <...> | <notes> |
| Unsupported polygon remains explicit diagnostic | <...> | <notes> |
| Review at 100% and representative zoom levels | <...> | <notes> |

Blocking fidelity defects: <none or list>

Accepted typed approximations/deferred semantics: <none or list with rationale>

Manual fidelity conclusion: <acceptable / blocking / repeat review>

## Renderer decision

Select exactly one **after** the evidence and manual review above are complete:

- [ ] **SVG selected as Phase-1 production renderer.**
- [ ] **SVG rejected for Phase 1; evaluate Canvas2D fallback.**
- [ ] **SVG rejected for Phase 1; evaluate WebGL fallback.**
- [ ] **SVG rejected for Phase 1; evaluate Qt/native fallback.**
- [ ] **No decision — evidence must be repeated or architecture work remains open.**

Final rationale: <complete after review>

## Phase-1 closure

- [ ] combined target evidence is decision-eligible;
- [ ] PreparedPage conclusion is recorded;
- [ ] native renderer performance conclusion is recorded;
- [ ] manual fidelity review is complete with no unresolved blocking defect;
- [ ] final renderer decision is recorded;
- [ ] public Phase-1 tracking issue #2 is updated to this source/evidence checkpoint;
- [ ] any renderer-promotion pull request references that same evidence checkpoint.
"@

$parent = Split-Path -Parent $OutputFile
if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
    $null = New-Item -ItemType Directory -Force -Path $parent
}
$utf8 = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($OutputFile, ($review.TrimEnd() + "`n"), $utf8)

Write-Host "ADR-019 renderer decision review draft created: $OutputFile"
Write-Host "Source commit: $sourceCommit"
Write-Host "Evidence eligible for Phase-1 decision: $eligible"
Write-Host "Mechanical renderer verdict: $performanceVerdict"
Write-Host "The script did not make a PreparedPage architecture decision, complete the visual fidelity review, or select the renderer."
