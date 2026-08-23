param(
    [Parameter(Mandatory = $true)]
    [string]$SessionDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

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
    throw "ADR-019 fidelity evidence session directory does not exist: $SessionDirectory"
}

$manifestPath = Join-Path $SessionDirectory "adr-019-fidelity-evidence.json"
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "ADR-019 fidelity evidence manifest is missing: $manifestPath"
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json

if ($manifest.manifest -ne "diagramdesigner-next-adr-019-fidelity-v1") {
    throw "Unexpected ADR-019 fidelity evidence manifest schema."
}
if ($manifest.finalRendererDecision -ne "not-made-by-runner") {
    throw "The fidelity evidence runner must not make the final renderer decision."
}
if ($manifest.manualReview.status -ne "not-reviewed-by-runner") {
    throw "The fidelity evidence runner must not claim completion of the manual review."
}
if ($manifest.manualReview.contract -ne "docs/architecture/adr-019-fidelity-review.md") {
    throw "Unexpected ADR-019 fidelity manual-review contract."
}

$commitSha = [string]$manifest.source.commit
$rootLockBlob = [string]$manifest.source.rootCargoLockGitBlob
$workingTreeCleanAtStart = Read-RequiredBooleanProperty $manifest.source "workingTreeCleanAtStart" "Fidelity manifest source workingTreeCleanAtStart"
$eligibleForPhase1Decision = Read-RequiredBooleanProperty $manifest.source "eligibleForPhase1Decision" "Fidelity manifest source eligibleForPhase1Decision"
$diagnosticOnly = Read-RequiredBooleanProperty $manifest.source "diagnosticOnly" "Fidelity manifest source diagnosticOnly"
Assert-GitObjectId $commitSha "Fidelity source commit"
Assert-GitObjectId $rootLockBlob "Fidelity root Cargo.lock blob"

if ($eligibleForPhase1Decision -ne $workingTreeCleanAtStart) {
    throw "Fidelity evidence decision eligibility does not match the recorded working-tree state."
}
if ($diagnosticOnly -ne (-not $workingTreeCleanAtStart)) {
    throw "Fidelity evidence diagnostic-only classification does not match the recorded working-tree state."
}

if ($manifest.renderer.candidate -ne "render-svg" -or $manifest.renderer.example -ne "fidelity_scene") {
    throw "Unexpected ADR-019 fidelity renderer candidate or fixture."
}
if ([int]$manifest.renderer.renderedElements -ne 12 -or
    [int]$manifest.renderer.skippedElements -ne 1 -or
    [int]$manifest.renderer.planDiagnostics -ne 0 -or
    [int]$manifest.renderer.svgDiagnostics -ne 2) {
    throw "ADR-019 fidelity fixture summary does not match the Phase-1 contract."
}

$expectedDiagnostics = @($manifest.expectedDiagnostics)
if ($expectedDiagnostics.Count -ne 2 -or
    [string]$expectedDiagnostics[0] -ne "ConnectorMarkerDeferred" -or
    [string]$expectedDiagnostics[1] -ne "UnsupportedPrimitive") {
    throw "ADR-019 fidelity expected-diagnostic contract changed."
}

$svgPath = Resolve-EvidencePath $SessionDirectory ([string]$manifest.renderer.svgFile) "Fidelity SVG"
$diagnosticsPath = Resolve-EvidencePath $SessionDirectory ([string]$manifest.renderer.diagnosticsFile) "Fidelity diagnostics"

$svgHash = (Get-FileHash -LiteralPath $svgPath -Algorithm SHA256).Hash.ToLowerInvariant()
$diagnosticsHash = (Get-FileHash -LiteralPath $diagnosticsPath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($svgHash -ne ([string]$manifest.renderer.svgSha256).ToLowerInvariant()) {
    throw "ADR-019 fidelity SVG SHA-256 does not match the archived manifest."
}
if ($diagnosticsHash -ne ([string]$manifest.renderer.diagnosticsSha256).ToLowerInvariant()) {
    throw "ADR-019 fidelity diagnostics SHA-256 does not match the archived manifest."
}

$svgText = Get-Content -LiteralPath $svgPath -Raw
if (-not $svgText.StartsWith("<svg ", [System.StringComparison]::Ordinal)) {
    throw "ADR-019 fidelity output is not an SVG document."
}
if ($svgText -match '<script\b|<foreignObject\b|\son[a-z]+\s*=') {
    throw "ADR-019 fidelity SVG contains active SVG content."
}

$diagnostics = Get-Content -LiteralPath $diagnosticsPath -Raw
$expectedSummary = 'FIDELITY-SUMMARY rendered=12 skipped=1 plan_diagnostics=0 svg_diagnostics=2'
if ($diagnostics -notmatch [regex]::Escape($expectedSummary)) {
    throw "ADR-019 fidelity diagnostics do not contain the expected renderer summary."
}
foreach ($diagnostic in $expectedDiagnostics) {
    if ($diagnostics -notmatch [regex]::Escape([string]$diagnostic)) {
        throw "ADR-019 fidelity diagnostics are missing expected diagnostic '$diagnostic'."
    }
}

Write-Host "ADR-019 fidelity evidence verified successfully: $SessionDirectory"
Write-Host "Source commit: $commitSha"
Write-Host "Eligible for Phase-1 decision: $eligibleForPhase1Decision"
Write-Host "Diagnostic only: $diagnosticOnly"
Write-Host "SVG SHA-256: $svgHash"
Write-Host "Diagnostics SHA-256: $diagnosticsHash"
Write-Host "Manual correctness/fidelity review and the final renderer decision remain outside this verifier."
