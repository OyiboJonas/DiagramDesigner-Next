Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$verifier = Join-Path $PSScriptRoot "verify-fidelity-evidence.ps1"
if (-not (Test-Path -LiteralPath $verifier -PathType Leaf)) {
    throw "ADR-019 fidelity verifier test fixture is missing: $verifier"
}

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
$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) "ddn-fidelity-verifier-$([Guid]::NewGuid().ToString('N'))"
$null = New-Item -ItemType Directory -Force -Path $tempRoot

try {
    $svgPath = Join-Path $tempRoot "adr-019-fidelity-scene.svg"
    $diagnosticsPath = Join-Path $tempRoot "adr-019-fidelity-diagnostics.txt"
    $manifestPath = Join-Path $tempRoot "adr-019-fidelity-evidence.json"

    Write-Utf8NoBom $svgPath "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect /></svg>`n"
    Write-Utf8NoBom $diagnosticsPath @"
SVG-DIAGNOSTIC ConnectorMarkerDeferred { synthetic }
SVG-DIAGNOSTIC UnsupportedPrimitive { synthetic }
FIDELITY-SUMMARY rendered=12 skipped=1 plan_diagnostics=0 svg_diagnostics=2
"@

    $svgHash = (Get-FileHash -LiteralPath $svgPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $diagnosticsHash = (Get-FileHash -LiteralPath $diagnosticsPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $manifest = [ordered]@{
        manifest = "diagramdesigner-next-adr-019-fidelity-v1"
        generatedAt = "2026-08-22T19:30:00.0000000Z"
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
            svgSha256 = $svgHash
            diagnosticsFile = "adr-019-fidelity-diagnostics.txt"
            diagnosticsSha256 = $diagnosticsHash
        }
        expectedDiagnostics = @("ConnectorMarkerDeferred", "UnsupportedPrimitive")
        manualReview = [ordered]@{
            status = "not-reviewed-by-runner"
            contract = "docs/architecture/adr-019-fidelity-review.md"
        }
        finalRendererDecision = "not-made-by-runner"
    }
    $manifestJson = $manifest | ConvertTo-Json -Depth 10
    Write-Utf8NoBom $manifestPath ($manifestJson + "`n")

    & $verifier -SessionDirectory $tempRoot

    $diagnosticManifest = $manifestJson | ConvertFrom-Json
    $diagnosticManifest.source.workingTreeCleanAtStart = $false
    $diagnosticManifest.source.eligibleForPhase1Decision = $false
    $diagnosticManifest.source.diagnosticOnly = $true
    Write-Utf8NoBom $manifestPath (($diagnosticManifest | ConvertTo-Json -Depth 10) + "`n")
    & $verifier -SessionDirectory $tempRoot

    Write-Utf8NoBom $manifestPath ($manifestJson + "`n")
    $tamperedEligibility = $manifestJson | ConvertFrom-Json
    $tamperedEligibility.source.eligibleForPhase1Decision = $false
    Write-Utf8NoBom $manifestPath (($tamperedEligibility | ConvertTo-Json -Depth 10) + "`n")
    Assert-Throws { & $verifier -SessionDirectory $tempRoot } "decision eligibility does not match"

    Write-Utf8NoBom $manifestPath ($manifestJson + "`n")
    $tamperedReview = $manifestJson | ConvertFrom-Json
    $tamperedReview.manualReview.status = "passed"
    Write-Utf8NoBom $manifestPath (($tamperedReview | ConvertTo-Json -Depth 10) + "`n")
    Assert-Throws { & $verifier -SessionDirectory $tempRoot } "must not claim completion"

    Write-Utf8NoBom $manifestPath ($manifestJson + "`n")
    Add-Content -LiteralPath $svgPath -Value "<!-- tampered -->"
    Assert-Throws { & $verifier -SessionDirectory $tempRoot } "fidelity SVG SHA-256 does not match"

    Write-Host "ADR-019 fidelity verifier synthetic fixture: clean and diagnostic classifications accepted; eligibility, review and raw-SVG tampering rejected."
}
finally {
    Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
}
