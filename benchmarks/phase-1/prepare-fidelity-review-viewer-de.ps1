param(
    [string]$SessionDirectory = "",
    [switch]$Open,
    [switch]$ValidateOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$generator = Join-Path $PSScriptRoot "prepare-fidelity-review-viewer.ps1"

function New-GermanReviewPatch {
    return @'
<script id="ddn-review-de-patch">
(() => {
  document.documentElement.lang = 'de';

  const scene = document.getElementById('scene');
  const viewport = document.getElementById('viewport');
  const rows = [...document.querySelectorAll('#review-table tbody tr')];
  const progress = document.getElementById('progress');
  const summary = document.getElementById('summary');

  const t = {
    'ADR-019 fidelity review': 'ADR-019 Fidelity-Pr\u00fcfung',
    'Deterministic fidelity scene': 'Deterministische Fidelity-Pr\u00fcfszene',
    'Manual correctness and fidelity checklist': 'Manuelle Korrektheits- und Fidelity-Pr\u00fcfliste',
    'Evidence identity': 'Nachweis-Identit\u00e4t',
    'Native renderer performance': 'Native Renderer-Performance',
    'PreparedPage release evidence': 'PreparedPage-Release-Nachweis',
    'Retained renderer diagnostics': 'Beibehaltene Renderer-Diagnosen'
  };

  const checklist = {
    'Master layer remains behind page-local layer': 'Masterebene bleibt hinter der seitenlokalen Ebene.',
    'Rounded rectangle and positive/negative rotations are geometrically correct': 'Abgerundetes Rechteck sowie positive und negative Rotationen sind geometrisch korrekt.',
    'Ellipse stroke, fill and alpha are correct': 'Kontur, F\u00fcllung und Transparenz der Ellipse sind korrekt.',
    'Horizontal and vertical gradients, including alpha, are correct': 'Horizontale und vertikale Farbverl\u00e4ufe einschlie\u00dflich Transparenz sind korrekt.',
    'Unicode, XML-sensitive text and page fields are legible and correct': 'Unicode-Zeichen, XML-Sonderzeichen und Seitenfelder sind lesbar und korrekt.',
    'Supported dotted and dash-dot connector styles are correct': 'Unterst\u00fctzte gepunktete und Strich-Punkt-Verbindungslinien werden korrekt dargestellt.',
    'All four rotated page-edge sentinels remain visible and clipped correctly': 'Alle vier gedrehten Pr\u00fcfelemente an den Seitenr\u00e4ndern sind sichtbar und korrekt abgeschnitten.',
    'Deferred connector marker remains an explicit diagnostic': 'Die noch nicht umgesetzte Pfeil-/Marker-Darstellung wird weiterhin ausdr\u00fccklich als Diagnose ausgewiesen.',
    'Unsupported polygon remains an explicit diagnostic': 'Das nicht unterst\u00fctzte Polygon wird weiterhin ausdr\u00fccklich als Diagnose ausgewiesen.',
    'Scene reviewed at intrinsic 100% and representative viewer zoom levels': 'Szene wurde bei echten 100 % sowie bei repr\u00e4sentativen Zoomstufen gepr\u00fcft.'
  };

  const hints = {
    'layer-order': 'Achte darauf, dass die graue Master-Fl\u00e4che hinter dem lokalen Inhalt liegt und nichts ungewollt \u00fcberdeckt.',
    'rect-rotation': 'Pr\u00fcfe Rundungen, Kanten und beide Drehrichtungen auf saubere Geometrie ohne Spr\u00fcnge oder Verzerrungen.',
    'ellipse-style': 'Pr\u00fcfe Konturbreite, F\u00fcllung und halbtransparente Darstellung der gedrehten Ellipse.',
    'gradients': 'Pr\u00fcfe, ob horizontale und vertikale Verl\u00e4ufe gleichm\u00e4\u00dfig und mit korrekter Transparenz erscheinen.',
    'rich-text': 'Pr\u00fcfe Umlaute, Sonderzeichen wie < & >, Pfeil/Unicode sowie Seitenname und Seitenzahl.',
    'connectors': 'Pr\u00fcfe Punkt- und Strich-Punkt-Linien auf klar erkennbare, gleichm\u00e4\u00dfige Muster.',
    'edge-sentinels': 'An jeder Blattkante muss ein teilweise abgeschnittenes, gedrehtes Pr\u00fcfelement sichtbar bleiben.',
    'marker-diagnostic': 'Der noch nicht unterst\u00fctzte Pfeilmarker darf nicht stillschweigend falsch gerendert werden; er muss in den Diagnosen auftauchen.',
    'polygon-diagnostic': 'Das nicht unterst\u00fctzte Polygon darf nicht stillschweigend falsch gerendert werden; es muss in den Diagnosen auftauchen.',
    'zoom-review': 'Vergleiche mindestens 50 %, 100 %, 150 % und 200 %. Formen und Text m\u00fcssen proportional skalieren; 150 % und 200 % m\u00fcssen sichtbar unterschiedlich sein.'
  };

  const statusText = {
    '': 'Nicht gepr\u00fcft',
    'correct': 'Korrekt',
    'acceptable-approximation': 'Akzeptable Ann\u00e4herung',
    'blocking': 'Blockierender Fidelity-Fehler',
    'out-of-scope': 'Au\u00dferhalb von Phase 1'
  };

  const headings = document.querySelectorAll('h1, .card > h2');
  for (const heading of headings) {
    const key = heading.textContent.trim();
    if (t[key]) heading.textContent = t[key];
  }

  const meta = document.querySelector('header .meta');
  if (meta && meta.textContent.startsWith('Source ')) {
    meta.textContent = meta.textContent.replace(/^Source /, 'Quell-Commit ');
  }

  const topBadge = document.querySelector('header .badge');
  if (topBadge) {
    if (topBadge.textContent.trim() === 'Decision eligible') topBadge.textContent = 'F\u00fcr Phase-1-Entscheidung geeignet';
    if (topBadge.textContent.trim() === 'Diagnostic only') topBadge.textContent = 'Nur diagnostisch';
  }

  const strong = document.querySelector('.viewer-toolbar strong');
  if (strong) strong.textContent = 'Darstellungszoom';

  const notice = document.querySelector('.notice');
  if (notice) {
    notice.textContent = 'Dies ist eine abgeleitete lokale Pr\u00fcfhilfe und kein Nachweis. 100 % entspricht der aus dem SVG berechneten Browser-Intrinsikgr\u00f6\u00dfe; 50 %, 150 % und 200 % werden exakt davon abgeleitet. Die Anzeige ist eine Rendering-/Fidelity-Pr\u00fcfung und keine Kalibrierung einer physischen Druckgr\u00f6\u00dfe. Pr\u00fcfe insbesondere, dass sich 150 % und 200 % sichtbar unterscheiden. Die Nachweisdateien werden durch Eingaben in dieser Seite nicht ver\u00e4ndert und es wird keine Rendererentscheidung getroffen.';
  }

  for (const row of rows) {
    const labelCell = row.querySelector('.check-label');
    const english = labelCell.textContent.trim();
    if (checklist[english]) labelCell.textContent = checklist[english];
    if (hints[row.dataset.reviewId]) labelCell.title = hints[row.dataset.reviewId];
    const select = row.querySelector('.status');
    for (const option of select.options) {
      if (Object.prototype.hasOwnProperty.call(statusText, option.value)) option.textContent = statusText[option.value];
    }
    select.setAttribute('aria-label', `Ergebnis f\u00fcr ${labelCell.textContent.trim()}`);
    const notes = row.querySelector('.notes');
    notes.placeholder = 'Optionale Notiz';
    notes.setAttribute('aria-label', `Notiz zu ${labelCell.textContent.trim()}`);
  }

  const headerMap = {
    'Check': 'Pr\u00fcfpunkt',
    'Result': 'Ergebnis',
    'Notes': 'Notiz',
    'Case': 'Fall',
    'Long tasks': 'Lange Tasks',
    'Stage': 'Testfl\u00e4che',
    'Scene': 'Szene',
    'Cache-hit p95': 'Cache-Treffer p95',
    'Eviction rebuild': 'Neuaufbau nach Verdr\u00e4ngung'
  };
  for (const th of document.querySelectorAll('th')) {
    const key = th.textContent.trim();
    if (headerMap[key]) th.textContent = headerMap[key];
  }

  const dtMap = {
    'Source commit': 'Quell-Commit',
    'Decision eligible': 'F\u00fcr Phase-1-Entscheidung geeignet',
    'Diagnostic only': 'Nur diagnostisch',
    'Manifest SHA-256': 'Manifest SHA-256',
    'Fidelity manifest SHA-256': 'Fidelity-Manifest SHA-256',
    'Fidelity SVG SHA-256': 'Fidelity-SVG SHA-256',
    'Diagnostics SHA-256': 'Diagnosen SHA-256'
  };
  for (const dt of document.querySelectorAll('dt')) {
    const key = dt.textContent.trim();
    if (dtMap[key]) dt.textContent = dtMap[key];
  }

  const perfNote = [...document.querySelectorAll('.card-body p')].find(p => p.textContent.includes('Timing contract:'));
  if (perfNote) perfNote.textContent = 'Grenzwerte: Aktualisierung p95 <= 16,667 ms; rAF-Frame p95 <= 17,500 ms. Die Performance ist nur ein Nachweis und ersetzt die manuelle Fidelity-Pr\u00fcfung nicht.';

  const perfBadge = [...document.querySelectorAll('.card .badge')].find(b => b.textContent.trim() === 'performance_gate_pass');
  if (perfBadge) perfBadge.textContent = 'Performance-Grenzwerte bestanden (performance_gate_pass)';

  function decodedSvgText() {
    const src = scene ? (scene.getAttribute('src') || '') : '';
    const prefix = 'data:image/svg+xml;base64,';
    if (!src.startsWith(prefix)) return '';
    try {
      const binary = atob(src.slice(prefix.length));
      const bytes = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
      return new TextDecoder('utf-8').decode(bytes);
    } catch (_) {
      return '';
    }
  }

  function attribute(tag, name) {
    const doubleQuoted = tag.match(new RegExp('\\b' + name + '\\s*=\\s*"([^"]+)"', 'i'));
    if (doubleQuoted) return doubleQuoted[1];
    const singleQuoted = tag.match(new RegExp("\\b" + name + "\\s*=\\s*'([^']+)'", 'i'));
    return singleQuoted ? singleQuoted[1] : '';
  }

  function cssPixels(raw) {
    const match = String(raw || '').trim().match(/^([0-9.+-]+)\s*(mm|cm|in|pt|px)?$/i);
    if (!match) return NaN;
    const value = Number(match[1]);
    const unit = (match[2] || 'px').toLowerCase();
    if (!Number.isFinite(value) || value <= 0) return NaN;
    if (unit === 'mm') return value * 96 / 25.4;
    if (unit === 'cm') return value * 96 / 2.54;
    if (unit === 'in') return value * 96;
    if (unit === 'pt') return value * 96 / 72;
    return value;
  }

  function resolveBaseSize() {
    const svgText = decodedSvgText();
    const tagMatch = svgText.match(/<svg\b[^>]*>/i);
    if (tagMatch) {
      const tag = tagMatch[0];
      const width = cssPixels(attribute(tag, 'width'));
      const height = cssPixels(attribute(tag, 'height'));
      if (Number.isFinite(width) && Number.isFinite(height)) return { width, height, source: 'SVG' };
      const viewBox = attribute(tag, 'viewBox').trim().split(/[\s,]+/).map(Number);
      if (viewBox.length === 4 && Number.isFinite(viewBox[2]) && Number.isFinite(viewBox[3]) && viewBox[2] > 0 && viewBox[3] > 0) {
        return { width: viewBox[2], height: viewBox[3], source: 'viewBox' };
      }
    }
    if (scene && scene.naturalWidth > 0 && scene.naturalHeight > 0) {
      return { width: scene.naturalWidth, height: scene.naturalHeight, source: 'natural' };
    }
    const box = scene ? scene.getBoundingClientRect() : { width: 0, height: 0 };
    return { width: Math.max(1, box.width), height: Math.max(1, box.height), source: 'layout' };
  }

  let baseSize = null;
  const zoomReadout = document.createElement('span');
  zoomReadout.id = 'zoom-readout';
  zoomReadout.style.fontSize = '12px';
  zoomReadout.style.fontWeight = '600';
  zoomReadout.style.opacity = '.78';
  if (progress && progress.parentNode) progress.parentNode.insertBefore(zoomReadout, progress);

  function applyExactZoom(percent) {
    if (!scene) return;
    if (!baseSize) baseSize = resolveBaseSize();
    const width = baseSize.width * percent / 100;
    const height = baseSize.height * percent / 100;
    scene.style.width = `${width}px`;
    scene.style.height = `${height}px`;
    scene.style.maxWidth = 'none';
    zoomReadout.textContent = `${percent} % | ${Math.round(width)} x ${Math.round(height)} CSS-Px`;
    if (viewport) {
      viewport.scrollLeft = 0;
      viewport.scrollTop = 0;
    }
  }

  for (const button of document.querySelectorAll('[data-zoom]')) {
    const replacement = button.cloneNode(true);
    button.replaceWith(replacement);
    replacement.addEventListener('click', () => {
      const zoom = Number(replacement.dataset.zoom);
      applyExactZoom(zoom);
      for (const other of document.querySelectorAll('[data-zoom]')) other.removeAttribute('aria-pressed');
      replacement.setAttribute('aria-pressed', 'true');
    });
  }

  function updateGermanProgress() {
    if (!progress) return;
    const reviewed = rows.filter(row => row.querySelector('.status').value).length;
    progress.textContent = `${reviewed} / ${rows.length} gepr\u00fcft`;
  }
  for (const row of rows) row.querySelector('.status').addEventListener('change', updateGermanProgress);
  updateGermanProgress();

  function reviewState() {
    return rows.map(row => ({
      label: row.querySelector('.check-label').textContent.trim(),
      status: row.querySelector('.status').value,
      notes: row.querySelector('.notes').value.trim()
    }));
  }

  function replaceButton(id, text, handler) {
    const original = document.getElementById(id);
    if (!original) return null;
    const replacement = original.cloneNode(true);
    replacement.textContent = text;
    original.replaceWith(replacement);
    replacement.addEventListener('click', handler);
    return replacement;
  }

  const buildButton = replaceButton('build-summary', 'Pr\u00fcfergebnis erstellen', () => {
    const eligibleText = document.querySelector('header .badge')?.textContent || '';
    const mechanical = document.querySelector('.card .badge.good, .card .badge.warn, .card .badge.bad')?.textContent || '';
    const lines = [
      '# ADR-019 - Zusammenfassung der manuellen Fidelity-Pr\u00fcfung',
      '',
      `Quell-Commit: ${document.querySelector('header .meta')?.textContent.replace(/^Quell-Commit\s*/, '') || ''}`,
      `Phase-1-Eignung: ${eligibleText}`,
      `Mechanisches Renderer-Ergebnis: ${mechanical}`,
      ''
    ];
    let blocking = 0;
    for (const item of reviewState()) {
      const label = statusText[item.status] || statusText[''];
      if (item.status === 'blocking') blocking += 1;
      lines.push(`- ${item.label}: ${label}${item.notes ? ` - ${item.notes}` : ''}`);
    }
    lines.push('', `Blockierende Fidelity-Fehler: ${blocking}`);
    lines.push('Finale Renderer-Entscheidung: WIRD NICHT DURCH DIESE PRUEFHILFE GETROFFEN');
    summary.value = lines.join('\n');
  });

  replaceButton('copy-summary', 'Zusammenfassung kopieren', async () => {
    if (!summary.value && buildButton) buildButton.click();
    try {
      await navigator.clipboard.writeText(summary.value);
    } catch (_) {
      summary.focus();
      summary.select();
      document.execCommand('copy');
    }
  });

  replaceButton('reset-review', 'Lokale Pr\u00fcfung zur\u00fccksetzen', () => {
    if (!confirm('Lokale Pr\u00fcfergebnisse und Notizen wirklich l\u00f6schen? Die Nachweisdateien bleiben unver\u00e4ndert.')) return;
    const storageKey = `ddn-adr019-review-${document.querySelector('header .meta')?.textContent.replace(/^Quell-Commit\s*/, '') || ''}`;
    try { localStorage.removeItem(storageKey); } catch (_) {}
    for (const row of rows) {
      row.querySelector('.status').value = '';
      row.querySelector('.notes').value = '';
    }
    summary.value = '';
    updateGermanProgress();
  });

  if (summary) {
    summary.setAttribute('aria-label', 'Zusammenfassung der Fidelity-Pr\u00fcfung');
    summary.placeholder = 'Nach Abschluss der Pr\u00fcfung auf "Pr\u00fcfergebnis erstellen" klicken.';
  }

  document.title = document.title.replace('ADR-019 fidelity review', 'ADR-019 Fidelity-Pr\u00fcfung');

  const initializeZoom = () => {
    baseSize = resolveBaseSize();
    applyExactZoom(100);
  };
  if (scene && scene.complete) initializeZoom();
  else if (scene) scene.addEventListener('load', initializeZoom, { once: true });
})();
</script>
'@
}

if ($ValidateOnly) {
    $patch = New-GermanReviewPatch
    foreach ($required in @(
        'ddn-review-de-patch',
        'resolveBaseSize',
        'value * 96 / 25.4',
        'applyExactZoom',
        '150 % und 200 %',
        'Pr\u00fcfergebnis erstellen'
    )) {
        if (-not $patch.Contains($required)) {
            throw "German fidelity review patch validation is missing required content: $required"
        }
    }
    Write-Host "German ADR-019 review patch validation passed. No evidence was read or written."
    return
}

if ([string]::IsNullOrWhiteSpace($SessionDirectory)) {
    throw "SessionDirectory is required unless -ValidateOnly is used."
}
if (-not (Test-Path -LiteralPath $generator -PathType Leaf)) {
    throw "Base fidelity review viewer generator is missing: $generator"
}

$SessionDirectory = [System.IO.Path]::GetFullPath($SessionDirectory)
if (-not (Test-Path -LiteralPath $SessionDirectory -PathType Container)) {
    throw "Phase-1 target session does not exist: $SessionDirectory"
}

$outputFile = Join-Path $SessionDirectory "adr-019-fidelity-review.html"
& $generator -SessionDirectory $SessionDirectory -OutputFile $outputFile -Force

$html = Get-Content -LiteralPath $outputFile -Raw -Encoding UTF8
$patchPattern = '(?s)\s*<script id="ddn-review-de-patch">.*?</script>\s*'
$html = [regex]::Replace($html, $patchPattern, "`r`n")
$patch = New-GermanReviewPatch
if (-not $html.Contains('</body>')) {
    throw "Generated fidelity review viewer has no closing body element."
}
$html = $html.Replace('</body>', "$patch`r`n</body>")

$utf8 = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($outputFile, $html, $utf8)

Write-Host "German ADR-019 fidelity review viewer prepared: $outputFile"
Write-Host "The underlying Phase-1 target evidence was verified but not modified."
Write-Host "Zoom uses explicit SVG-derived CSS pixel dimensions; 150% and 200% are distinct sizes."

if ($Open) {
    Start-Process -FilePath $outputFile
}
