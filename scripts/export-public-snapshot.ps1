param(
    [string]$OutputPath = "",
    [string]$ForbiddenTermsFile = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$invocationDirectory = (Get-Location).Path
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

if (-not [string]::IsNullOrWhiteSpace($ForbiddenTermsFile) -and -not [System.IO.Path]::IsPathRooted($ForbiddenTermsFile)) {
    $ForbiddenTermsFile = Join-Path $invocationDirectory $ForbiddenTermsFile
}

Push-Location $repoRoot
try {
    $status = @(& git status --porcelain)
    if ($LASTEXITCODE -ne 0) {
        throw "git status failed."
    }
    if ($status.Count -ne 0) {
        throw "Public snapshot export requires a clean working tree."
    }

    $preflight = Join-Path $PSScriptRoot "public-preflight.ps1"
    if ([string]::IsNullOrWhiteSpace($ForbiddenTermsFile)) {
        & $preflight
    }
    else {
        & $preflight -ForbiddenTermsFile $ForbiddenTermsFile
    }

    if ([string]::IsNullOrWhiteSpace($OutputPath)) {
        $OutputPath = Join-Path (Split-Path $repoRoot -Parent) "DiagramDesigner-Next-public-snapshot.zip"
    }
    elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
        $OutputPath = Join-Path $invocationDirectory $OutputPath
    }
    $OutputPath = [System.IO.Path]::GetFullPath($OutputPath)

    if ([System.IO.Path]::GetExtension($OutputPath) -ne '.zip') {
        throw "OutputPath must end in .zip"
    }

    $separatorChars = [char[]]@(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    $repoPrefix = $repoRoot.TrimEnd($separatorChars) + [System.IO.Path]::DirectorySeparatorChar
    if ($OutputPath.StartsWith($repoPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Write the public snapshot outside the repository working tree."
    }

    if (Test-Path -LiteralPath $OutputPath) {
        Remove-Item -LiteralPath $OutputPath -Force
    }

    & git archive --format=zip "--output=$OutputPath" HEAD
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $OutputPath -PathType Leaf)) {
        throw "git archive failed."
    }

    $hash = (Get-FileHash -LiteralPath $OutputPath -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Host "History-free public snapshot created: $OutputPath"
    Write-Host "Snapshot SHA-256: $hash"
    Write-Host "The ZIP contains tracked HEAD content only; it contains no .git history, issues or pull-request metadata."
}
finally {
    Pop-Location
}
