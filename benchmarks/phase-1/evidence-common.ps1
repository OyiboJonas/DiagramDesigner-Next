Set-StrictMode -Version Latest

function Convert-BenchmarkKeyValueLine {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Line,
        [Parameter(Mandatory = $true)]
        [string]$Prefix
    )

    if (-not $Line.StartsWith($Prefix, [System.StringComparison]::Ordinal)) {
        throw "Benchmark line does not start with expected prefix '$Prefix'."
    }

    $values = [ordered]@{}
    $payload = $Line.Substring($Prefix.Length).Trim()
    foreach ($token in ($payload -split '\s+')) {
        if ([string]::IsNullOrWhiteSpace($token)) { continue }
        $separator = $token.IndexOf('=')
        if ($separator -le 0 -or $separator -eq ($token.Length - 1)) {
            throw "Malformed benchmark token '$token' in line '$Line'."
        }
        $key = $token.Substring(0, $separator)
        $value = $token.Substring($separator + 1)
        if ($values.Contains($key)) {
            throw "Duplicate benchmark key '$key' in line '$Line'."
        }
        $values[$key] = $value
    }
    return $values
}

function Read-RequiredInt64 {
    param(
        [Parameter(Mandatory = $true)]
        [System.Collections.IDictionary]$Values,
        [Parameter(Mandatory = $true)]
        [string]$Key,
        [Parameter(Mandatory = $true)]
        [string]$Context
    )

    if (-not $Values.Contains($Key)) {
        throw "$Context is missing required numeric key '$Key'."
    }
    $parsed = 0L
    if (-not [Int64]::TryParse(
        [string]$Values[$Key],
        [System.Globalization.NumberStyles]::Integer,
        [System.Globalization.CultureInfo]::InvariantCulture,
        [ref]$parsed
    )) {
        throw "$Context key '$Key' is not a valid Int64: '$($Values[$Key])'."
    }
    if ($parsed -lt 0) {
        throw "$Context key '$Key' must not be negative."
    }
    return $parsed
}

function Get-PreparedCacheMetrics {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Lines
    )

    $metaLines = @($Lines | Where-Object { $_.StartsWith('BENCH prepared-cache-meta ', [System.StringComparison]::Ordinal) })
    if ($metaLines.Count -ne 1) {
        throw "PreparedPage evidence must contain exactly one prepared-cache metadata line."
    }
    $meta = Convert-BenchmarkKeyValueLine $metaLines[0] 'BENCH prepared-cache-meta '
    if (-not $meta.Contains('schema') -or [string]$meta['schema'] -ne 'diagramdesigner-next-prepared-cache-v1') {
        throw "PreparedPage evidence has an unexpected benchmark schema."
    }

    $cacheCapacity = Read-RequiredInt64 $meta 'cache_capacity' 'PreparedPage benchmark metadata'
    $rebuildSamples = Read-RequiredInt64 $meta 'rebuild_samples' 'PreparedPage benchmark metadata'
    $hitSamples = Read-RequiredInt64 $meta 'hit_samples' 'PreparedPage benchmark metadata'
    if ($cacheCapacity -le 0 -or $rebuildSamples -le 0 -or $hitSamples -le 0) {
        throw "PreparedPage benchmark metadata capacity/sample counts must be greater than zero."
    }

    $caseLines = @($Lines | Where-Object { $_.StartsWith('BENCH prepared-cache nodes=', [System.StringComparison]::Ordinal) })
    if ($caseLines.Count -ne 2) {
        throw "Phase-1 PreparedPage evidence must contain exactly two benchmark result lines."
    }

    $cases = [System.Collections.Generic.List[object]]::new()
    foreach ($line in $caseLines) {
        $values = Convert-BenchmarkKeyValueLine $line 'BENCH prepared-cache '
        $nodes = Read-RequiredInt64 $values 'nodes' 'PreparedPage benchmark result'
        $caseRebuildSamples = Read-RequiredInt64 $values 'rebuild_samples' "PreparedPage $nodes result"
        $caseHitSamples = Read-RequiredInt64 $values 'hit_samples' "PreparedPage $nodes result"
        if ($caseRebuildSamples -ne $rebuildSamples -or $caseHitSamples -ne $hitSamples) {
            throw "PreparedPage $nodes result sample counts do not match the benchmark metadata."
        }

        $cases.Add([ordered]@{
            nodes = $nodes
            rebuildSamples = $caseRebuildSamples
            rebuildUs = [ordered]@{
                p50 = Read-RequiredInt64 $values 'rebuild_p50_us' "PreparedPage $nodes result"
                p95 = Read-RequiredInt64 $values 'rebuild_p95_us' "PreparedPage $nodes result"
                p99 = Read-RequiredInt64 $values 'rebuild_p99_us' "PreparedPage $nodes result"
                max = Read-RequiredInt64 $values 'rebuild_max_us' "PreparedPage $nodes result"
            }
            hitSamples = $caseHitSamples
            cacheHitNs = [ordered]@{
                p50 = Read-RequiredInt64 $values 'hit_p50_ns' "PreparedPage $nodes result"
                p95 = Read-RequiredInt64 $values 'hit_p95_ns' "PreparedPage $nodes result"
                p99 = Read-RequiredInt64 $values 'hit_p99_ns' "PreparedPage $nodes result"
                max = Read-RequiredInt64 $values 'hit_max_ns' "PreparedPage $nodes result"
            }
            history = [ordered]@{
                hits = Read-RequiredInt64 $values 'history_hits' "PreparedPage $nodes result"
                p50Ns = Read-RequiredInt64 $values 'history_p50_ns' "PreparedPage $nodes result"
                p95Ns = Read-RequiredInt64 $values 'history_p95_ns' "PreparedPage $nodes result"
                maxNs = Read-RequiredInt64 $values 'history_max_ns' "PreparedPage $nodes result"
                builds = Read-RequiredInt64 $values 'history_builds' "PreparedPage $nodes result"
            }
            eviction = [ordered]@{
                rebuildUs = Read-RequiredInt64 $values 'eviction_rebuild_us' "PreparedPage $nodes result"
                builds = Read-RequiredInt64 $values 'eviction_builds' "PreparedPage $nodes result"
                evictions = Read-RequiredInt64 $values 'evictions' "PreparedPage $nodes result"
            }
        })
    }

    $orderedCases = @($cases | Sort-Object { [Int64]$_.nodes })
    if ($orderedCases.Count -ne 2 -or [Int64]$orderedCases[0].nodes -ne 5000 -or [Int64]$orderedCases[1].nodes -ne 20000) {
        throw "Phase-1 PreparedPage evidence must contain exactly the 5k and 20k benchmark cases."
    }

    return [ordered]@{
        schema = [string]$meta['schema']
        cacheCapacity = $cacheCapacity
        rebuildSamples = $rebuildSamples
        hitSamples = $hitSamples
        cases = $orderedCases
    }
}

function Assert-PreparedCacheMetricsMatch {
    param(
        [Parameter(Mandatory = $true)]
        [object]$Expected,
        [Parameter(Mandatory = $true)]
        [object]$Actual
    )

    foreach ($property in @('schema', 'cacheCapacity', 'rebuildSamples', 'hitSamples')) {
        if ([string]$Expected.$property -ne [string]$Actual.$property) {
            throw "PreparedPage manifest metric '$property' does not match raw evidence."
        }
    }

    $expectedCases = @($Expected.cases)
    $actualCases = @($Actual.cases)
    if ($expectedCases.Count -ne $actualCases.Count) {
        throw "PreparedPage manifest case count does not match raw evidence."
    }

    for ($index = 0; $index -lt $actualCases.Count; $index += 1) {
        $expectedCase = $expectedCases[$index]
        $actualCase = $actualCases[$index]
        foreach ($property in @('nodes', 'rebuildSamples', 'hitSamples')) {
            if ([Int64]$expectedCase.$property -ne [Int64]$actualCase.$property) {
                throw "PreparedPage manifest case $index metric '$property' does not match raw evidence."
            }
        }
        foreach ($property in @('p50', 'p95', 'p99', 'max')) {
            if ([Int64]$expectedCase.rebuildUs.$property -ne [Int64]$actualCase.rebuildUs.$property) {
                throw "PreparedPage $($actualCase.nodes) rebuild metric '$property' does not match raw evidence."
            }
            if ([Int64]$expectedCase.cacheHitNs.$property -ne [Int64]$actualCase.cacheHitNs.$property) {
                throw "PreparedPage $($actualCase.nodes) cache-hit metric '$property' does not match raw evidence."
            }
        }
        foreach ($property in @('hits', 'p50Ns', 'p95Ns', 'maxNs', 'builds')) {
            if ([Int64]$expectedCase.history.$property -ne [Int64]$actualCase.history.$property) {
                throw "PreparedPage $($actualCase.nodes) history metric '$property' does not match raw evidence."
            }
        }
        foreach ($property in @('rebuildUs', 'builds', 'evictions')) {
            if ([Int64]$expectedCase.eviction.$property -ne [Int64]$actualCase.eviction.$property) {
                throw "PreparedPage $($actualCase.nodes) eviction metric '$property' does not match raw evidence."
            }
        }
    }
}
