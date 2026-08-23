param(
    [string]$OutputDirectory = "",
    [switch]$AllowDirtyTree,
    [switch]$ValidateOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw "The ADR-019 fidelity evidence runner must be executed on Windows."
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$examplePath = Join-Path $repoRoot "crates\render-svg\examples\fidelity_scene.rs"
$rootLockfile = Join-Path $repoRoot "Cargo.lock"
$verifier = Join-Path $PSScriptRoot "verify-fidelity-evidence.ps1"
$verifierTest = Join-Path $PSScriptRoot "test-fidelity-evidence-verifier.ps1"

foreach ($requiredPath in @($examplePath, $rootLockfile, $verifier, $verifierTest)) {
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
        throw "Required ADR-019 fidelity evidence file is missing: $requiredPath"
    }
}

function Get-GitBlob([string]$RepoPath, [string]$Label) {
    $blob = (& git rev-parse "HEAD:$RepoPath").Trim()
    if ($LASTEXITCODE -ne 0 -or $blob -notmatch '^[0-9a-fA-F]{40}$') {
        throw "Unable to resolve the committed $Label Git blob."
    }
    return $blob.ToLowerInvariant()
}

Push-Location $repoRoot
try {
    $commitSha = (& git rev-parse --verify HEAD).Trim().ToLowerInvariant()
    if ($LASTEXITCODE -ne 0 -or $commitSha -notmatch '^[0-9a-f]{40}$') {
        throw "Unable to resolve a 40-character Git commit for ADR-019 fidelity evidence."
    }

    $workingTree = (& git status --porcelain --untracked-files=normal | Out-String).TrimEnd()
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to inspect the Git working tree."
    }
    $workingTreeClean = [string]::IsNullOrWhiteSpace($workingTree)
    if (-not $workingTreeClean -and -not $AllowDirtyTree) {
        throw "ADR-019 fidelity decision evidence requires a clean Git working tree. Commit/stash changes or use -AllowDirtyTree for diagnostics only."
    }

    $rootLockBlob = Get-GitBlob "Cargo.lock" "root Cargo.lock"

    if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
        $OutputDirectory = Join-Path $repoRoot "benchmark-results\phase-1-fidelity"
    }
    elseif (-not [System.IO.Path]::IsPathRooted($OutputDirectory)) {
        $OutputDirectory = Join-Path $repoRoot $OutputDirectory
    }
    $OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)

    Write-Host "ADR-019 fidelity source commit: $commitSha"
    Write-Host "ADR-019 fidelity root Cargo.lock Git blob: $rootLockBlob"
    Write-Host "ADR-019 fidelity output root: $OutputDirectory"

    if ($ValidateOnly) {
        & cargo check --locked --quiet -p render-svg --example fidelity_scene
        if ($LASTEXITCODE -ne 0) {
            throw "The ADR-019 fidelity scene example did not compile."
        }
        & $verifierTest
        Write-Host "ADR-019 fidelity evidence configuration and archive verifier are valid. No target scene evidence was generated."
        return
    }

    $timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $sessionName = "fidelity-$timestamp-$($commitSha.Substring(0, 12))"
    $sessionDirectory = Join-Path $OutputDirectory $sessionName
    $null = New-Item -ItemType Directory -Force -Path $sessionDirectory

    $svgPath = Join-Path $sessionDirectory "adr-019-fidelity-scene.svg"
    $diagnosticsPath = Join-Path $sessionDirectory "adr-019-fidelity-diagnostics.txt"

    # Start-Process redirects the native process streams directly to files. This
    # avoids Windows PowerShell text-redirection transcoding the SVG output.
    $process = Start-Process -FilePath "cargo" -ArgumentList @(
        "run",
        "--locked",
        "--quiet",
        "-p",
        "render-svg",
        "--example",
        "fidelity_scene"
    ) -NoNewWindow -Wait -PassThru -RedirectStandardOutput $svgPath -RedirectStandardError $diagnosticsPath
    if ($process.ExitCode -ne 0) {
        throw "ADR-019 fidelity scene generation failed. See: $diagnosticsPath"
    }

    $svgText = Get-Content -LiteralPath $svgPath -Raw
    if (-not $svgText.StartsWith("<svg ", [System.StringComparison]::Ordinal)) {
        throw "ADR-019 fidelity output is not an SVG document."
    }
    if ($svgText -match '<script\b|<foreignObject\b|\son[a-z]+\s*=') {
        throw "ADR-019 fidelity SVG unexpectedly contains active SVG content."
    }

    $diagnostics = Get-Content -LiteralPath $diagnosticsPath -Raw
    $expectedSummary = 'FIDELITY-SUMMARY rendered=12 skipped=1 plan_diagnostics=0 svg_diagnostics=2'
    if ($diagnostics -notmatch [regex]::Escape($expectedSummary)) {
        throw "ADR-019 fidelity renderer summary changed. Expected '$expectedSummary'."
    }
    if ($diagnostics -notmatch 'ConnectorMarkerDeferred') {
        throw "ADR-019 fidelity fixture did not surface the expected deferred connector-marker diagnostic."
    }
    if ($diagnostics -notmatch 'UnsupportedPrimitive') {
        throw "ADR-019 fidelity fixture did not surface the expected unsupported-primitive diagnostic."
    }

    $svgSha256 = (Get-FileHash -LiteralPath $svgPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $diagnosticsSha256 = (Get-FileHash -LiteralPath $diagnosticsPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $eligibleForPhase1Decision = $workingTreeClean

    $manifest = [ordered]@{
        manifest = "diagramdesigner-next-adr-019-fidelity-v1"
        generatedAt = [DateTime]::UtcNow.ToString("o")
        source = [ordered]@{
            commit = $commitSha
            workingTreeCleanAtStart = $workingTreeClean
            rootCargoLockGitBlob = $rootLockBlob
            eligibleForPhase1Decision = $eligibleForPhase1Decision
            diagnosticOnly = -not $eligibleForPhase1Decision
        }
        renderer = [ordered]@{
            candidate = "render-svg"
            example = "fidelity_scene"
            renderedElements = 12
            skippedElements = 1
            planDiagnostics = 0
            svgDiagnostics = 2
            svgFile = [System.IO.Path]::GetFileName($svgPath)
            svgSha256 = $svgSha256
            diagnosticsFile = [System.IO.Path]::GetFileName($diagnosticsPath)
            diagnosticsSha256 = $diagnosticsSha256
        }
        expectedDiagnostics = @(
            "ConnectorMarkerDeferred",
            "UnsupportedPrimitive"
        )
        manualReview = [ordered]@{
            status = "not-reviewed-by-runner"
            contract = "docs/architecture/adr-019-fidelity-review.md"
        }
        finalRendererDecision = "not-made-by-runner"
    }

    $manifestPath = Join-Path $sessionDirectory "adr-019-fidelity-evidence.json"
    $manifestJson = $manifest | ConvertTo-Json -Depth 10
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($manifestPath, "$manifestJson`n", $utf8)

    & $verifier -SessionDirectory $sessionDirectory

    Write-Host "ADR-019 fidelity evidence retained at: $sessionDirectory"
    Write-Host "SVG SHA-256: $svgSha256"
    Write-Host "Diagnostics SHA-256: $diagnosticsSha256"
    Write-Host "Eligible for Phase-1 decision: $eligibleForPhase1Decision"
    Write-Host "Manual correctness/fidelity review remains required; this runner does not select the renderer."
}
finally {
    Pop-Location
}
