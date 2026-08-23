param(
    [string]$ForbiddenTermsFile = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $repoRoot
try {
    $tracked = @(& git ls-files)
    if ($LASTEXITCODE -ne 0) {
        throw "git ls-files failed."
    }

    $findings = [System.Collections.Generic.List[string]]::new()

    $forbiddenTrackedPatterns = @(
        '(^|/)benchmark-results/',
        '(^|/)private-fixtures/',
        '(^|/)\.env($|\.)',
        '\.ddd$',
        '\.(pem|p12|pfx|key)$',
        'private-corpus\.local\.json$',
        '\.private\.json$'
    )

    foreach ($path in $tracked) {
        foreach ($pattern in $forbiddenTrackedPatterns) {
            if ($path -match $pattern) {
                $findings.Add("forbidden tracked path: $path")
                break
            }
        }
    }

    $secretPatterns = @(
        @{ Name = 'GitHub token'; Pattern = 'gh[pousr]_[A-Za-z0-9_]{20,}' },
        @{ Name = 'AWS access key'; Pattern = 'AKIA[0-9A-Z]{16}' },
        @{ Name = 'private key'; Pattern = '-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----' },
        @{ Name = 'credential assignment'; Pattern = '(?i)(password|passwd|api[_-]?key|access[_-]?token|secret)\s*[:=]\s*["''][^"'']{6,}["'']' },
        @{ Name = 'personal email'; Pattern = '(?i)[A-Z0-9._%+-]+@(gmail|outlook|hotmail|yahoo)\.[A-Z]{2,}' }
    )

    $forbiddenTerms = @()
    if (-not [string]::IsNullOrWhiteSpace($ForbiddenTermsFile)) {
        $ForbiddenTermsFile = [System.IO.Path]::GetFullPath($ForbiddenTermsFile)
        if (-not (Test-Path -LiteralPath $ForbiddenTermsFile -PathType Leaf)) {
            throw "Forbidden terms file does not exist: $ForbiddenTermsFile"
        }
        $forbiddenTerms = @(
            Get-Content -LiteralPath $ForbiddenTermsFile -Encoding UTF8 |
                ForEach-Object { $_.Trim() } |
                Where-Object { -not [string]::IsNullOrWhiteSpace($_) -and -not $_.StartsWith('#') }
        )
    }

    foreach ($relativePath in $tracked) {
        $path = Join-Path $repoRoot $relativePath
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }

        $bytes = [System.IO.File]::ReadAllBytes($path)
        if ($bytes.Length -gt 5MB -or ($bytes -contains [byte]0)) { continue }

        $text = [System.Text.Encoding]::UTF8.GetString($bytes)
        foreach ($secretPattern in $secretPatterns) {
            if ([regex]::IsMatch($text, $secretPattern.Pattern)) {
                $findings.Add("$($secretPattern.Name) pattern in $relativePath")
            }
        }

        foreach ($term in $forbiddenTerms) {
            if ($text.IndexOf($term, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
                $findings.Add("local forbidden term found in $relativePath")
            }
        }
    }

    if ($findings.Count -gt 0) {
        $message = "Public preflight failed:`n - " + (($findings | Sort-Object -Unique) -join "`n - ")
        throw $message
    }

    Write-Host "Public preflight passed for $($tracked.Count) tracked files."
    if ($forbiddenTerms.Count -eq 0) {
        Write-Host "No local forbidden-terms file was supplied; run again with -ForbiddenTermsFile for private organization/file/hash checks."
    }
}
finally {
    Pop-Location
}
