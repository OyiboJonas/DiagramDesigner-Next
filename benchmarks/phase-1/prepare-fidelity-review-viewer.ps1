param(
    [string]$SessionDirectory = "",
    [string]$OutputFile = "",
    [switch]$Force,
    [switch]$Open,
    [switch]$ValidateOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$verifier = Join-Path $PSScriptRoot "verify-target-evidence.ps1"

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

function Html([object]$Value) {
    if ($null -eq $Value) { return "n/a" }
    return [System.Net.WebUtility]::HtmlEncode([string]$Value)
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

function Get-OptionalPropertyValue([object]$Object, [string]$Name) {
    if ($null -eq $Object) { return $null }
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) { return $null }
    return $property.Value
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

function New-ReviewViewerHtml([hashtable]$Data) {
    $checkRows = @(
        @("layer-order", "Master layer remains behind page-local layer"),
        @("rect-rotation", "Rounded rectangle and positive/negative rotations are geometrically correct"),
        @("ellipse-style", "Ellipse stroke, fill and alpha are correct"),
        @("gradients", "Horizontal and vertical gradients, including alpha, are correct"),
        @("rich-text", "Unicode, XML-sensitive text and page fields are legible and correct"),
        @("connectors", "Supported dotted and dash-dot connector styles are correct"),
        @("edge-sentinels", "All four rotated page-edge sentinels remain visible and clipped correctly"),
        @("marker-diagnostic", "Deferred connector marker remains an explicit diagnostic"),
        @("polygon-diagnostic", "Unsupported polygon remains an explicit diagnostic"),
        @("zoom-review", "Scene reviewed at 100% and representative viewer zoom levels")
    )

    $rows = [System.Text.StringBuilder]::new()
    foreach ($item in $checkRows) {
        $id = Html $item[0]
        $label = Html $item[1]
        [void]$rows.AppendLine(@"
<tr data-review-id="$id">
  <td class="check-label">$label</td>
  <td>
    <select class="status" aria-label="Result for $label">
      <option value="">Not reviewed</option>
      <option value="correct">Correct</option>
      <option value="acceptable-approximation">Acceptable approximation</option>
      <option value="blocking">Blocking fidelity defect</option>
      <option value="out-of-scope">Out of Phase-1 scope</option>
    </select>
  </td>
  <td><input class="notes" type="text" aria-label="Notes for $label" placeholder="Optional notes"></td>
</tr>
"@)
    }

    $template = @'
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>ADR-019 fidelity review - __SHORT_COMMIT__</title>
<style>
:root { color-scheme: light dark; font-family: Segoe UI, system-ui, sans-serif; }
* { box-sizing: border-box; }
body { margin: 0; background: Canvas; color: CanvasText; }
header { position: sticky; top: 0; z-index: 20; display: flex; gap: 16px; align-items: center; justify-content: space-between; padding: 14px 20px; border-bottom: 1px solid color-mix(in srgb, CanvasText 18%, transparent); background: color-mix(in srgb, Canvas 94%, transparent); backdrop-filter: blur(10px); }
header h1 { margin: 0; font-size: 18px; }
header .meta { font-size: 12px; opacity: .72; }
main { display: grid; grid-template-columns: minmax(0, 1.5fr) minmax(360px, .8fr); gap: 18px; padding: 18px; }
.card { border: 1px solid color-mix(in srgb, CanvasText 16%, transparent); border-radius: 12px; background: color-mix(in srgb, Canvas 97%, CanvasText 3%); overflow: hidden; }
.card > h2 { margin: 0; padding: 13px 15px; font-size: 15px; border-bottom: 1px solid color-mix(in srgb, CanvasText 13%, transparent); }
.card-body { padding: 14px 15px; }
.viewer-toolbar { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin-bottom: 10px; }
button { border: 1px solid color-mix(in srgb, CanvasText 24%, transparent); border-radius: 8px; background: ButtonFace; color: ButtonText; padding: 7px 10px; cursor: pointer; }
button[aria-pressed="true"] { outline: 2px solid Highlight; outline-offset: 1px; }
.scene-viewport { height: min(66vh, 780px); overflow: auto; background: white; border: 1px solid #bbb; border-radius: 8px; }
.scene-wrap { min-width: 100%; min-height: 100%; display: flex; align-items: flex-start; justify-content: flex-start; padding: 16px; }
#scene { display: block; width: 100%; max-width: none; height: auto; transform-origin: top left; }
.badge { display: inline-flex; align-items: center; gap: 6px; border-radius: 999px; padding: 4px 9px; font-size: 12px; font-weight: 600; }
.badge.good { background: #dff4e4; color: #174d25; }
.badge.warn { background: #fff0c7; color: #6b4b00; }
.badge.bad { background: #ffdede; color: #6d1616; }
.kv { display: grid; grid-template-columns: max-content 1fr; gap: 6px 12px; font-size: 13px; }
.kv dt { opacity: .68; }
.kv dd { margin: 0; overflow-wrap: anywhere; }
table { width: 100%; border-collapse: collapse; font-size: 12.5px; }
th, td { border-bottom: 1px solid color-mix(in srgb, CanvasText 12%, transparent); padding: 8px; text-align: left; vertical-align: top; }
th { font-weight: 600; }
.status { width: 190px; max-width: 100%; padding: 6px; }
.notes { width: 100%; min-width: 180px; padding: 6px; }
.review-progress { margin-left: auto; font-size: 12px; font-weight: 600; }
pre { white-space: pre-wrap; word-break: break-word; max-height: 270px; overflow: auto; padding: 10px; border-radius: 8px; background: color-mix(in srgb, Canvas 88%, CanvasText 12%); font-size: 12px; }
textarea { width: 100%; min-height: 160px; resize: vertical; padding: 9px; font-family: Consolas, monospace; font-size: 12px; }
.notice { padding: 10px 12px; border-left: 4px solid Highlight; background: color-mix(in srgb, Highlight 9%, Canvas); font-size: 12.5px; line-height: 1.45; }
.stack { display: grid; gap: 18px; align-content: start; }
@media (max-width: 1050px) { main { grid-template-columns: 1fr; } .scene-viewport { height: 60vh; } }
@media print { header, .viewer-toolbar, .actions { display: none; } main { display: block; padding: 0; } .card { break-inside: avoid; margin-bottom: 12px; } .scene-viewport { height: auto; overflow: visible; } }
</style>
</head>
<body>
<header>
  <div>
    <h1>ADR-019 fidelity review</h1>
    <div class="meta">Source __SOURCE_COMMIT__</div>
  </div>
  <span class="badge __ELIGIBILITY_CLASS__">__ELIGIBILITY_LABEL__</span>
</header>
<main>
  <section class="stack">
    <article class="card">
      <h2>Deterministic fidelity scene</h2>
      <div class="card-body">
        <div class="viewer-toolbar" role="toolbar" aria-label="Viewer zoom">
          <strong>Viewer zoom</strong>
          <button type="button" data-zoom="50">50%</button>
          <button type="button" data-zoom="100" aria-pressed="true">100%</button>
          <button type="button" data-zoom="150">150%</button>
          <button type="button" data-zoom="200">200%</button>
          <span class="review-progress" id="progress">0 / 10 reviewed</span>
        </div>
        <div class="notice">This is a derived local review aid, not evidence. The SVG is embedded as an image from the already verified archive; changing review selections does not modify the evidence files or select a renderer.</div>
        <div class="scene-viewport" id="viewport">
          <div class="scene-wrap"><img id="scene" alt="ADR-019 deterministic fidelity scene" src="data:image/svg+xml;base64,__SVG_BASE64__"></div>
        </div>
      </div>
    </article>

    <article class="card">
      <h2>Manual correctness and fidelity checklist</h2>
      <div class="card-body">
        <table id="review-table">
          <thead><tr><th>Check</th><th>Result</th><th>Notes</th></tr></thead>
          <tbody>__CHECK_ROWS__</tbody>
        </table>
        <div class="actions" style="display:flex; gap:8px; margin-top:12px; flex-wrap:wrap">
          <button type="button" id="build-summary">Build review summary</button>
          <button type="button" id="copy-summary">Copy summary</button>
          <button type="button" id="reset-review">Reset local review</button>
        </div>
        <textarea id="summary" aria-label="Review summary" placeholder="Use Build review summary after completing the checklist."></textarea>
      </div>
    </article>
  </section>

  <aside class="stack">
    <article class="card">
      <h2>Evidence identity</h2>
      <div class="card-body">
        <dl class="kv">
          <dt>Source commit</dt><dd>__SOURCE_COMMIT__</dd>
          <dt>Decision eligible</dt><dd>__ELIGIBLE__</dd>
          <dt>Diagnostic only</dt><dd>__DIAGNOSTIC_ONLY__</dd>
          <dt>Manifest SHA-256</dt><dd>__MANIFEST_SHA__</dd>
          <dt>Fidelity manifest SHA-256</dt><dd>__FIDELITY_MANIFEST_SHA__</dd>
          <dt>Fidelity SVG SHA-256</dt><dd>__SVG_SHA__</dd>
          <dt>Diagnostics SHA-256</dt><dd>__DIAGNOSTICS_SHA__</dd>
        </dl>
      </div>
    </article>

    <article class="card">
      <h2>Native renderer performance</h2>
      <div class="card-body">
        <p><span class="badge __PERF_CLASS__">__PERF_VERDICT__</span></p>
        <table>
          <thead><tr><th>Case</th><th>Update p95</th><th>Frame p95</th><th>Long tasks</th><th>DOM max</th><th>Stage</th></tr></thead>
          <tbody>
            <tr><td>5,000 culled</td><td>__UPDATE_5K__ ms</td><td>__FRAME_5K__ ms</td><td>__LONG_5K__</td><td>__DOM_5K__</td><td>__STAGE_5K__</td></tr>
            <tr><td>20,000 culled</td><td>__UPDATE_20K__ ms</td><td>__FRAME_20K__ ms</td><td>__LONG_20K__</td><td>__DOM_20K__</td><td>__STAGE_20K__</td></tr>
          </tbody>
        </table>
        <p style="font-size:12px; opacity:.72">Timing contract: update p95 <= 16.667 ms; rAF frame p95 <= 17.500 ms. Performance remains evidence only and cannot replace the manual fidelity decision.</p>
      </div>
    </article>

    <article class="card">
      <h2>PreparedPage release evidence</h2>
      <div class="card-body">
        <table>
          <thead><tr><th>Scene</th><th>Rebuild p95</th><th>Cache-hit p95</th><th>Eviction rebuild</th></tr></thead>
          <tbody>
            <tr><td>5,000</td><td>__PREP_REBUILD_5K__ us</td><td>__PREP_HIT_5K__ ns</td><td>__PREP_EVICT_5K__ us</td></tr>
            <tr><td>20,000</td><td>__PREP_REBUILD_20K__ us</td><td>__PREP_HIT_20K__ ns</td><td>__PREP_EVICT_20K__ us</td></tr>
          </tbody>
        </table>
      </div>
    </article>

    <article class="card">
      <h2>Retained renderer diagnostics</h2>
      <div class="card-body"><pre>__DIAGNOSTICS_TEXT__</pre></div>
    </article>
  </aside>
</main>
<script>
(() => {
  const storageKey = 'ddn-adr019-review-__SOURCE_COMMIT__';
  const rows = [...document.querySelectorAll('#review-table tbody tr')];
  const progress = document.getElementById('progress');
  const summary = document.getElementById('summary');

  function load() {
    let saved = {};
    try { saved = JSON.parse(localStorage.getItem(storageKey) || '{}'); } catch (_) {}
    for (const row of rows) {
      const id = row.dataset.reviewId;
      const item = saved[id];
      if (!item) continue;
      row.querySelector('.status').value = item.status || '';
      row.querySelector('.notes').value = item.notes || '';
    }
    updateProgress();
  }

  function state() {
    const value = {};
    for (const row of rows) {
      const id = row.dataset.reviewId;
      value[id] = {
        label: row.querySelector('.check-label').textContent.trim(),
        status: row.querySelector('.status').value,
        notes: row.querySelector('.notes').value.trim()
      };
    }
    return value;
  }

  function save() {
    try { localStorage.setItem(storageKey, JSON.stringify(state())); } catch (_) {}
    updateProgress();
  }

  function updateProgress() {
    const reviewed = rows.filter(row => row.querySelector('.status').value).length;
    progress.textContent = `${reviewed} / ${rows.length} reviewed`;
  }

  for (const row of rows) {
    row.querySelector('.status').addEventListener('change', save);
    row.querySelector('.notes').addEventListener('input', save);
  }

  for (const button of document.querySelectorAll('[data-zoom]')) {
    button.addEventListener('click', () => {
      const zoom = Number(button.dataset.zoom);
      document.getElementById('scene').style.width = `${zoom}%`;
      for (const other of document.querySelectorAll('[data-zoom]')) other.removeAttribute('aria-pressed');
      button.setAttribute('aria-pressed', 'true');
    });
  }

  document.getElementById('build-summary').addEventListener('click', () => {
    const lines = [
      '# ADR-019 manual fidelity review summary',
      '',
      'Source commit: __SOURCE_COMMIT__',
      'Evidence eligible: __ELIGIBLE__',
      'Mechanical renderer verdict: __PERF_VERDICT__',
      ''
    ];
    let blocking = 0;
    for (const item of Object.values(state())) {
      const status = item.status || 'not-reviewed';
      if (status === 'blocking') blocking += 1;
      lines.push(`- ${item.label}: ${status}${item.notes ? ` - ${item.notes}` : ''}`);
    }
    lines.push('', `Blocking fidelity defects: ${blocking}`);
    lines.push('Final renderer decision: NOT MADE BY THIS REVIEW AID');
    summary.value = lines.join('\n');
  });

  document.getElementById('copy-summary').addEventListener('click', async () => {
    if (!summary.value) document.getElementById('build-summary').click();
    try {
      await navigator.clipboard.writeText(summary.value);
    } catch (_) {
      summary.focus();
      summary.select();
      document.execCommand('copy');
    }
  });

  document.getElementById('reset-review').addEventListener('click', () => {
    if (!confirm('Clear the local review selections and notes? Evidence files are not affected.')) return;
    try { localStorage.removeItem(storageKey); } catch (_) {}
    for (const row of rows) {
      row.querySelector('.status').value = '';
      row.querySelector('.notes').value = '';
    }
    summary.value = '';
    updateProgress();
  });

  load();
})();
</script>
</body>
</html>
'@

    $replacements = [ordered]@{
        "__SHORT_COMMIT__" = Html $Data.ShortCommit
        "__SOURCE_COMMIT__" = Html $Data.SourceCommit
        "__ELIGIBILITY_CLASS__" = Html $Data.EligibilityClass
        "__ELIGIBILITY_LABEL__" = Html $Data.EligibilityLabel
        "__ELIGIBLE__" = Html $Data.Eligible
        "__DIAGNOSTIC_ONLY__" = Html $Data.DiagnosticOnly
        "__MANIFEST_SHA__" = Html $Data.ManifestSha
        "__FIDELITY_MANIFEST_SHA__" = Html $Data.FidelityManifestSha
        "__SVG_SHA__" = Html $Data.SvgSha
        "__DIAGNOSTICS_SHA__" = Html $Data.DiagnosticsSha
        "__PERF_CLASS__" = Html $Data.PerformanceClass
        "__PERF_VERDICT__" = Html $Data.PerformanceVerdict
        "__UPDATE_5K__" = Html $Data.Update5k
        "__FRAME_5K__" = Html $Data.Frame5k
        "__LONG_5K__" = Html $Data.Long5k
        "__DOM_5K__" = Html $Data.Dom5k
        "__STAGE_5K__" = Html $Data.Stage5k
        "__UPDATE_20K__" = Html $Data.Update20k
        "__FRAME_20K__" = Html $Data.Frame20k
        "__LONG_20K__" = Html $Data.Long20k
        "__DOM_20K__" = Html $Data.Dom20k
        "__STAGE_20K__" = Html $Data.Stage20k
        "__PREP_REBUILD_5K__" = Html $Data.PreparedRebuild5k
        "__PREP_HIT_5K__" = Html $Data.PreparedHit5k
        "__PREP_EVICT_5K__" = Html $Data.PreparedEvict5k
        "__PREP_REBUILD_20K__" = Html $Data.PreparedRebuild20k
        "__PREP_HIT_20K__" = Html $Data.PreparedHit20k
        "__PREP_EVICT_20K__" = Html $Data.PreparedEvict20k
        "__DIAGNOSTICS_TEXT__" = $Data.DiagnosticsText
        "__SVG_BASE64__" = $Data.SvgBase64
        "__CHECK_ROWS__" = $rows.ToString()
    }

    foreach ($entry in $replacements.GetEnumerator()) {
        $template = $template.Replace([string]$entry.Key, [string]$entry.Value)
    }
    return $template
}

if ($ValidateOnly) {
    $sampleSvg = '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10"/></svg>'
    $sampleData = @{
        ShortCommit = "111111111111"
        SourceCommit = "1111111111111111111111111111111111111111"
        EligibilityClass = "good"
        EligibilityLabel = "Decision eligible"
        Eligible = "True"
        DiagnosticOnly = "False"
        ManifestSha = ("1" * 64)
        FidelityManifestSha = ("2" * 64)
        SvgSha = ("3" * 64)
        DiagnosticsSha = ("4" * 64)
        PerformanceClass = "good"
        PerformanceVerdict = "performance_gate_pass"
        Update5k = "1.000"
        Frame5k = "16.800"
        Long5k = "0"
        Dom5k = "900"
        Stage5k = "3840x2160"
        Update20k = "2.000"
        Frame20k = "16.800"
        Long20k = "0"
        Dom20k = "900"
        Stage20k = "3840x2160"
        PreparedRebuild5k = "100"
        PreparedHit5k = "200"
        PreparedEvict5k = "120"
        PreparedRebuild20k = "400"
        PreparedHit20k = "500"
        PreparedEvict20k = "450"
        DiagnosticsText = Html "ConnectorMarkerDeferred`nUnsupportedPrimitive"
        SvgBase64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($sampleSvg))
    }
    $sampleHtml = New-ReviewViewerHtml $sampleData
    foreach ($required in @(
        "data:image/svg+xml;base64,",
        "Master layer remains behind page-local layer",
        "ConnectorMarkerDeferred",
        "Final renderer decision: NOT MADE BY THIS REVIEW AID",
        "performance_gate_pass"
    )) {
        if (-not $sampleHtml.Contains($required)) {
            throw "Fidelity review viewer validation is missing required content: $required"
        }
    }
    if ($sampleHtml.Contains($sampleSvg)) {
        throw "Fidelity review viewer must embed the SVG as image data rather than inline active markup."
    }
    Write-Host "ADR-019 fidelity review viewer template validation passed. No evidence was read or written."
    return
}

if ([string]::IsNullOrWhiteSpace($SessionDirectory)) {
    throw "SessionDirectory is required unless -ValidateOnly is used."
}
if (-not (Test-Path -LiteralPath $verifier -PathType Leaf)) {
    throw "Fidelity review viewer is missing the Phase-1 archive verifier: $verifier"
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
    throw "This target session predates integrated fidelity evidence. Run the current combined target runner first."
}

$rendererPath = Resolve-ArchivedPath $SessionDirectory ([string]$manifest.renderer.evidenceFile) "ADR-019 renderer evidence"
$fidelityManifestPath = Resolve-ArchivedPath $SessionDirectory ([string]$manifest.fidelity.evidenceManifest) "ADR-019 fidelity manifest"
$renderer = Get-Content -LiteralPath $rendererPath -Raw | ConvertFrom-Json
$fidelity = Get-Content -LiteralPath $fidelityManifestPath -Raw | ConvertFrom-Json
$fidelityRoot = Split-Path -Parent $fidelityManifestPath
$svgPath = Resolve-ArchivedPath $fidelityRoot ([string]$fidelity.renderer.svgFile) "ADR-019 fidelity SVG"
$diagnosticsPath = Resolve-ArchivedPath $fidelityRoot ([string]$fidelity.renderer.diagnosticsFile) "ADR-019 fidelity diagnostics"

$measurements = @($renderer.measurements)
$culled5k = Find-CulledCase $measurements 5000
$culled20k = Find-CulledCase $measurements 20000
$preparedCases = @($manifest.preparedPage.metrics.cases)
$prepared5k = @($preparedCases | Where-Object { [int]$_.nodes -eq 5000 })
$prepared20k = @($preparedCases | Where-Object { [int]$_.nodes -eq 20000 })
if ($prepared5k.Count -ne 1 -or $prepared20k.Count -ne 1) {
    throw "Combined manifest does not contain exactly one PreparedPage 5k and 20k case."
}
$prepared5k = $prepared5k[0]
$prepared20k = $prepared20k[0]

$sourceCommit = [string]$manifest.source.commit
$eligible = [bool]$manifest.source.eligibleForPhase1Decision
$diagnosticOnly = [bool]$manifest.source.diagnosticOnly
$performanceVerdict = [string]$renderer.performanceVerdict.status
$performanceClass = if ($performanceVerdict -eq "performance_gate_pass") { "good" } elseif ($performanceVerdict -eq "measurement_incomplete") { "warn" } else { "bad" }
$eligibilityClass = if ($eligible) { "good" } else { "warn" }
$eligibilityLabel = if ($eligible) { "Decision eligible" } else { "Diagnostic only" }

$manifestSha = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
$fidelityManifestSha = (Get-FileHash -LiteralPath $fidelityManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
$svgSha = (Get-FileHash -LiteralPath $svgPath -Algorithm SHA256).Hash.ToLowerInvariant()
$diagnosticsSha = (Get-FileHash -LiteralPath $diagnosticsPath -Algorithm SHA256).Hash.ToLowerInvariant()
$svgBase64 = [Convert]::ToBase64String([System.IO.File]::ReadAllBytes($svgPath))
$diagnosticsText = Html (Get-Content -LiteralPath $diagnosticsPath -Raw)

$data = @{
    ShortCommit = $sourceCommit.Substring(0, 12)
    SourceCommit = $sourceCommit
    EligibilityClass = $eligibilityClass
    EligibilityLabel = $eligibilityLabel
    Eligible = $eligible
    DiagnosticOnly = $diagnosticOnly
    ManifestSha = $manifestSha
    FidelityManifestSha = $fidelityManifestSha
    SvgSha = $svgSha
    DiagnosticsSha = $diagnosticsSha
    PerformanceClass = $performanceClass
    PerformanceVerdict = $performanceVerdict
    Update5k = Format-NumberInvariant (Get-OptionalPropertyValue $culled5k "update_ms_p95")
    Frame5k = Format-NumberInvariant $culled5k.frame_ms_p95
    Long5k = $culled5k.long_tasks_observed
    Dom5k = $culled5k.dom_nodes_max
    Stage5k = "$($culled5k.stage_physical_px.width)x$($culled5k.stage_physical_px.height)"
    Update20k = Format-NumberInvariant (Get-OptionalPropertyValue $culled20k "update_ms_p95")
    Frame20k = Format-NumberInvariant $culled20k.frame_ms_p95
    Long20k = $culled20k.long_tasks_observed
    Dom20k = $culled20k.dom_nodes_max
    Stage20k = "$($culled20k.stage_physical_px.width)x$($culled20k.stage_physical_px.height)"
    PreparedRebuild5k = $prepared5k.rebuildUs.p95
    PreparedHit5k = $prepared5k.cacheHitNs.p95
    PreparedEvict5k = $prepared5k.eviction.rebuildUs
    PreparedRebuild20k = $prepared20k.rebuildUs.p95
    PreparedHit20k = $prepared20k.cacheHitNs.p95
    PreparedEvict20k = $prepared20k.eviction.rebuildUs
    DiagnosticsText = $diagnosticsText
    SvgBase64 = $svgBase64
}

if ([string]::IsNullOrWhiteSpace($OutputFile)) {
    $OutputFile = Join-Path $SessionDirectory "adr-019-fidelity-review.html"
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputFile)) {
    $OutputFile = Join-Path $SessionDirectory $OutputFile
}
$OutputFile = [System.IO.Path]::GetFullPath($OutputFile)

if ((Test-Path -LiteralPath $OutputFile) -and -not $Force) {
    throw "Fidelity review viewer already exists: $OutputFile. Use -Force only if replacing the derived local viewer is intentional."
}

$html = New-ReviewViewerHtml $data
$parent = Split-Path -Parent $OutputFile
if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
    $null = New-Item -ItemType Directory -Force -Path $parent
}
$utf8 = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($OutputFile, $html, $utf8)

Write-Host "ADR-019 local fidelity review viewer created: $OutputFile"
Write-Host "Source commit: $sourceCommit"
Write-Host "Evidence eligible for Phase-1 decision: $eligible"
Write-Host "Mechanical renderer verdict: $performanceVerdict"
Write-Host "The viewer is derived review state only. It does not modify archived evidence or select a renderer."

if ($Open) {
    Start-Process -FilePath $OutputFile
}
