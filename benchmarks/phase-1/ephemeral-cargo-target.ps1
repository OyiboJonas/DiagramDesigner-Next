Set-StrictMode -Version Latest

function Invoke-DdnWithEphemeralCargoTarget {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [scriptblock]$Script,
        [string]$BuildRoot = "",
        [double]$MinimumFreeSpaceGB = 0,
        [switch]$KeepBuildArtifacts
    )

    if ($MinimumFreeSpaceGB -lt 0) {
        throw "MinimumFreeSpaceGB cannot be negative."
    }

    if ([string]::IsNullOrWhiteSpace($BuildRoot)) {
        $BuildRoot = [System.IO.Path]::GetTempPath()
    }
    elseif (-not [System.IO.Path]::IsPathRooted($BuildRoot)) {
        $BuildRoot = [System.IO.Path]::GetFullPath((Join-Path (Get-Location).Path $BuildRoot))
    }
    else {
        $BuildRoot = [System.IO.Path]::GetFullPath($BuildRoot)
    }

    $pathRoot = [System.IO.Path]::GetPathRoot($BuildRoot)
    if (-not [string]::IsNullOrWhiteSpace($pathRoot) -and $pathRoot -match '^[A-Za-z]:\\$') {
        $driveName = $pathRoot.Substring(0, 1)
        $drive = Get-PSDrive -Name $driveName -PSProvider FileSystem -ErrorAction SilentlyContinue
        if ($null -ne $drive -and $MinimumFreeSpaceGB -gt 0) {
            $freeGB = [double]$drive.Free / 1GB
            if ($freeGB -lt $MinimumFreeSpaceGB) {
                throw ("Insufficient free space on {0}: {1:N2} GB available, at least {2:N2} GB required for this isolated Cargo build. Free space or use -CargoBuildRoot on another drive." -f $pathRoot, $freeGB, $MinimumFreeSpaceGB)
            }
        }
    }

    $null = New-Item -ItemType Directory -Force -Path $BuildRoot
    $targetDirectory = Join-Path $BuildRoot "ddn-cargo-target-$([Guid]::NewGuid().ToString('N'))"
    $null = New-Item -ItemType Directory -Force -Path $targetDirectory

    $previousCargoTargetDirectory = $env:CARGO_TARGET_DIR
    try {
        $env:CARGO_TARGET_DIR = $targetDirectory
        Write-Host "Using isolated Cargo target directory: $targetDirectory"
        & $Script
    }
    finally {
        $env:CARGO_TARGET_DIR = $previousCargoTargetDirectory

        if ($KeepBuildArtifacts) {
            Write-Warning "Cargo build artifacts were retained for diagnostics at: $targetDirectory"
        }
        else {
            $removed = $false
            for ($attempt = 1; $attempt -le 3 -and -not $removed; $attempt++) {
                try {
                    if (Test-Path -LiteralPath $targetDirectory) {
                        Remove-Item -LiteralPath $targetDirectory -Recurse -Force -ErrorAction Stop
                    }
                    $removed = $true
                }
                catch {
                    if ($attempt -lt 3) {
                        Start-Sleep -Milliseconds 500
                    }
                }
            }

            if (Test-Path -LiteralPath $targetDirectory) {
                Write-Warning "Unable to remove the isolated Cargo target directory automatically. Remove it manually when no build process is using it: $targetDirectory"
            }
            else {
                Write-Host "Removed isolated Cargo build artifacts."
            }
        }
    }
}
