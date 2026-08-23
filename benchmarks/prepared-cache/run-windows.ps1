param(
    [int[]]$Counts = @(5000, 20000),
    [int]$RebuildSamples = 20,
    [int]$HitSamples = 2000,
    [string]$OutputDirectory = "",
    [switch]$DebugBuild,
    [switch]$AllowDirtyTree
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw "The prepared-cache target evidence runner must be executed on Windows."
}
if ($RebuildSamples -le 0 -or $HitSamples -le 0) {
    throw "RebuildSamples and HitSamples must both be greater than zero."
}
$invalidCounts = @($Counts | Where-Object { $_ -le 0 })
if ($Counts.Count -eq 0 -or $invalidCounts.Count -ne 0) {
    throw "Counts must contain at least one positive element count."
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$lockfilePath = Join-Path $repoRoot "Cargo.lock"
Push-Location $repoRoot
try {
    $commitSha = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $commitSha -notmatch '^[0-9a-fA-F]{40}$') {
        throw "Unable to resolve the current Git commit."
    }
    $commitSha = $commitSha.ToLowerInvariant()

    if (-not (Test-Path -LiteralPath $lockfilePath -PathType Leaf)) {
        throw "Prepared-cache target evidence requires the committed root Cargo.lock."
    }
    $lockfileBlob = (& git rev-parse "HEAD:Cargo.lock").Trim()
    if ($LASTEXITCODE -ne 0 -or $lockfileBlob -notmatch '^[0-9a-fA-F]{40}$') {
        throw "Unable to resolve the committed root Cargo.lock Git blob."
    }
    $lockfileBlob = $lockfileBlob.ToLowerInvariant()

    $workingTree = (& git status --porcelain --untracked-files=normal | Out-String).TrimEnd()
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to inspect the Git working tree."
    }
    if (-not $AllowDirtyTree -and -not [string]::IsNullOrWhiteSpace($workingTree)) {
        throw "Target evidence requires a clean Git working tree. Commit/stash changes or use -AllowDirtyTree for diagnostics only."
    }

    if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
        $OutputDirectory = Join-Path $repoRoot "benchmark-results\prepared-cache"
    }
    elseif (-not [System.IO.Path]::IsPathRooted($OutputDirectory)) {
        $OutputDirectory = Join-Path $repoRoot $OutputDirectory
    }
    $null = New-Item -ItemType Directory -Force -Path $OutputDirectory

    $timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $outputPath = Join-Path $OutputDirectory "prepared-cache-$timestamp.txt"
    $profile = if ($DebugBuild) { "debug" } else { "release" }

    $os = Get-CimInstance Win32_OperatingSystem |
        Select-Object Caption, Version, BuildNumber, OSArchitecture
    $cpu = Get-CimInstance Win32_Processor |
        Select-Object Name, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed
    $computer = Get-CimInstance Win32_ComputerSystem |
        Select-Object Manufacturer, Model, TotalPhysicalMemory
    $gpu = Get-CimInstance Win32_VideoController |
        Select-Object Name, DriverVersion, AdapterRAM
    $battery = Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue |
        Select-Object BatteryStatus, EstimatedChargeRemaining
    $powerScheme = (& powercfg /getactivescheme 2>&1 | Out-String).Trim()

    $rustc = (& rustc --version --verbose 2>&1 | Out-String).TrimEnd()
    if ($LASTEXITCODE -ne 0) {
        throw "rustc is unavailable or failed."
    }
    $cargo = (& cargo --version 2>&1 | Out-String).TrimEnd()
    if ($LASTEXITCODE -ne 0) {
        throw "cargo is unavailable or failed."
    }

    Write-Host "Prepared-cache source commit: $commitSha"
    Write-Host "Prepared-cache root Cargo.lock Git blob: $lockfileBlob"

    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add("DIAGRAMDESIGNER_NEXT_PREPARED_CACHE_EVIDENCE_V1")
    $lines.Add("captured_local=$((Get-Date).ToString('o'))")
    $lines.Add("captured_utc=$([DateTime]::UtcNow.ToString('o'))")
    $lines.Add("commit_sha=$commitSha")
    $lines.Add("cargo_lock_blob=$lockfileBlob")
    $lines.Add("working_tree_clean=$([string]::IsNullOrWhiteSpace($workingTree))")
    $lines.Add("build_profile=$profile")
    $lines.Add("rebuild_samples=$RebuildSamples")
    $lines.Add("hit_samples=$HitSamples")
    $lines.Add("counts=$($Counts -join ',')")
    $lines.Add("powershell=$($PSVersionTable.PSVersion.ToString())")
    $lines.Add("META os=$($os | ConvertTo-Json -Compress)")
    $lines.Add("META cpu=$($cpu | ConvertTo-Json -Compress)")
    $lines.Add("META computer=$($computer | ConvertTo-Json -Compress)")
    $lines.Add("META gpu=$($gpu | ConvertTo-Json -Compress)")
    if ($null -eq $battery) {
        $lines.Add("META battery=null")
    }
    else {
        $lines.Add("META battery=$($battery | ConvertTo-Json -Compress)")
    }
    $lines.Add("META power_scheme=$powerScheme")
    $lines.Add("META rustc_begin")
    foreach ($line in ($rustc -split "`r?`n")) { $lines.Add($line) }
    $lines.Add("META rustc_end")
    $lines.Add("META cargo=$cargo")
    if (-not [string]::IsNullOrWhiteSpace($workingTree)) {
        $lines.Add("META git_status_begin")
        foreach ($line in ($workingTree -split "`r?`n")) { $lines.Add($line) }
        $lines.Add("META git_status_end")
    }
    $lines.Add("BENCHMARK stdout_begin")

    $cargoArgs = [System.Collections.Generic.List[string]]::new()
    $cargoArgs.Add("run")
    if (-not $DebugBuild) { $cargoArgs.Add("--release") }
    $cargoArgs.Add("--locked")
    $cargoArgs.Add("--quiet")
    $cargoArgs.Add("-p")
    $cargoArgs.Add("editor-runtime")
    $cargoArgs.Add("--bin")
    $cargoArgs.Add("prepared-cache-bench")
    $cargoArgs.Add("--")
    $cargoArgs.Add("--rebuild-samples")
    $cargoArgs.Add($RebuildSamples.ToString())
    $cargoArgs.Add("--hit-samples")
    $cargoArgs.Add($HitSamples.ToString())
    foreach ($count in $Counts) { $cargoArgs.Add($count.ToString()) }

    $benchmarkOutput = @(& cargo @cargoArgs 2>&1 | ForEach-Object { $_.ToString() })
    $cargoExitCode = $LASTEXITCODE
    foreach ($line in $benchmarkOutput) {
        $lines.Add($line)
        Write-Host $line
    }
    $lines.Add("BENCHMARK stdout_end")
    $lines.Add("cargo_exit_code=$cargoExitCode")

    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllLines($outputPath, [string[]]$lines, $utf8)

    if ($cargoExitCode -ne 0) {
        throw "Prepared-cache benchmark failed with exit code $cargoExitCode. Evidence was retained at $outputPath"
    }

    $metaLines = @($benchmarkOutput | Where-Object { $_ -like "BENCH prepared-cache-meta *" })
    if ($metaLines.Count -ne 1) {
        throw "Benchmark output did not contain exactly one prepared-cache metadata line. Evidence: $outputPath"
    }
    foreach ($count in $Counts) {
        $pattern = "BENCH prepared-cache nodes=$count *"
        $matching = @($benchmarkOutput | Where-Object { $_ -like $pattern })
        if ($matching.Count -ne 1) {
            throw "Benchmark output did not contain exactly one result for nodes=$count. Evidence: $outputPath"
        }
    }

    Write-Host "Prepared-cache evidence written to: $outputPath"
    if ($DebugBuild) {
        Write-Warning "DebugBuild is for CI/diagnostics only and is not Phase-1 target-hardware evidence."
    }
}
finally {
    Pop-Location
}
