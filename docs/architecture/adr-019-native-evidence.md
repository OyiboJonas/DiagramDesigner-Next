# ADR-019 native renderer evidence contract

## Purpose

ADR-019 deliberately keeps SVG as a renderer candidate until the actual Windows/Tauri/WebView2 path is measured on representative hardware with a physical 3840×2160 client area and reviewed for correctness/fidelity.

The native benchmark measures the renderer candidate inside an isolated fullscreen Tauri WebView. The Phase-1 combined target runner binds that evidence to the PreparedPage release benchmark **and** the deterministic fidelity-scene archive under one source-bound session.

This contract makes the resulting evidence durable and reviewable without granting the WebView generic filesystem access.

## Evidence ownership

The benchmark WebView has only three benchmark-specific native capabilities:

1. inspect its own native environment and build provenance;
2. submit one completed ADR-019 report to the Rust backend for validated atomic evidence storage;
3. close itself.

It does **not** receive a generic filesystem API, a path picker, shell access, or arbitrary command execution.

The Rust backend validates the report schema and build-source identity before persisting it. The destination is selected by the native process, never by WebView input:

- normal application runs: app-local data under `benchmarks/adr-019`;
- controlled target runs: the process owner may set `DDN_ADR019_EVIDENCE_DIR` to an absolute directory before launch.

Evidence is written through the existing `platform-fs::atomic_save` boundary.

## Source provenance

The desktop build records:

- application version;
- source commit;
- whether the source tree was dirty when the build provenance was established;
- the committed standalone desktop `Cargo.lock` Git blob.

For controlled target measurements, `benchmarks/adr-019/run-windows.ps1` supplies the exact checkout commit through build-time environment variables and forces a clean desktop rebuild so stale build provenance cannot survive from an earlier compilation.

The native environment object embedded into every report therefore includes:

- `appVersion`;
- `sourceCommit`;
- `sourceDirty`;
- Tauri/WebView runtime;
- platform;
- physical WebView client dimensions;
- scale factor;
- fullscreen state;
- monitor dimensions and name.

The combined Phase-1 runner additionally binds PreparedPage and fidelity evidence to the same commit and the committed root `Cargo.lock` Git blob.

## Report persistence validation

Rust accepts a benchmark report only when all of these structural checks pass:

- report schema is `diagramdesigner-next-adr-019-native-v1`;
- `finalRendererDecision` remains `not-made-by-benchmark`;
- report source commit exactly matches the running build;
- report dirty-source flag matches the running build when known;
- exactly four benchmark measurements are present;
- a non-empty mechanical performance verdict status exists;
- a generation timestamp exists;
- serialized evidence remains below the native size limit.

A failed performance verdict is still valid evidence and is persisted. The persistence boundary must never turn the mechanical benchmark result into the final renderer decision.

## Preferred Phase-1 representative run

From a clean checkout on the representative Windows machine, use the combined runner:

```powershell
.\benchmarks\phase-1\run-target-evidence-windows.ps1
```

The combined runner performs the full automated Phase-1 evidence sequence:

1. verifies Windows, Git source identity, lockfile provenance and required benchmark wiring;
2. runs the PreparedPage 5k/20k benchmark in **release** mode;
3. launches the native fullscreen ADR-019 Tauri/WebView2 benchmark;
4. requires a physical WebView client area of at least 3840×2160 and the complete four-case renderer report;
5. generates the deterministic ADR-019 fidelity scene through the real `render-plan → render-svg` path;
6. verifies PreparedPage, renderer and fidelity evidence use the same source provenance and eligibility classification;
7. hashes and archives all retained evidence;
8. writes `phase-1-target-evidence.json` with structured PreparedPage metrics, renderer summary and a hash-bound reference to the fidelity manifest;
9. verifies the complete archive chain again through `benchmarks/phase-1/verify-target-evidence.ps1`, which also invokes the fidelity archive verifier;
10. generates a prefilled `adr-019-renderer-decision-review.md` from the now-verified archive.

The final review draft is a derived, human-editable aid. It is intentionally **not** part of the immutable evidence hash chain and cannot change the archived benchmark/fidelity evidence it summarizes.

A clean source tree produces evidence marked:

```text
eligibleForPhase1Decision = true
diagnosticOnly = false
```

`-AllowDirtyTree` exists only for diagnostics. Dirty sessions are still source-bound and verifiable, but the combined manifest must mark them:

```text
eligibleForPhase1Decision = false
diagnosticOnly = true
```

The archived-evidence verifiers check that this classification agrees with raw PreparedPage/renderer provenance and with the nested fidelity manifest. Editing only a manifest cannot promote a diagnostic run into decision evidence.

## Fidelity archive

The fidelity evidence is generated automatically during the combined target run. A standalone diagnostic/review export remains available:

```powershell
.\benchmarks\phase-1\export-fidelity-scene-windows.ps1
```

The fidelity manifest is `diagramdesigner-next-adr-019-fidelity-v1` and records:

- source commit and root `Cargo.lock` Git blob;
- clean/diagnostic eligibility classification;
- candidate renderer and fixture identity;
- rendered/skipped element and diagnostic counts;
- SHA-256 hashes for the generated SVG and diagnostic log;
- the expected deferred/unsupported diagnostics;
- `manualReview.status = not-reviewed-by-runner`;
- `finalRendererDecision = not-made-by-runner`.

This is intentional. Automated fidelity evidence can prove source identity, deterministic fixture structure and explicit diagnostics, but it cannot replace the visual correctness review.

## Prefilled renderer review

The combined runner creates the review draft automatically after the archive has passed verification. An existing integrated target session can recreate the draft with:

```powershell
.\benchmarks\phase-1\prepare-renderer-decision-review.ps1 `
  -SessionDirectory .\benchmark-results\phase-1-target\target-<timestamp>-<commit>
```

The preparer re-verifies the target archive first, then prefills:

- source commit and both lockfile Git blobs;
- clean/decision eligibility;
- PreparedPage 5k/20k release rebuild p95, cache-hit p95 and eviction rebuild values;
- 5k/20k culled native frame p95, Long Task counts, DOM maxima and measured physical-stage dimensions;
- the mechanical renderer verdict and reasons;
- fidelity manifest, SVG and diagnostic hashes and local paths.

The preparer refuses to overwrite an existing review draft unless `-Force` is supplied. It never marks the manual fidelity review complete, never makes the PreparedPage incremental-patching decision and never selects SVG or a fallback renderer.

## Standalone ADR-019 run

The renderer-only runner remains useful for focused diagnostics:

```powershell
.\benchmarks\adr-019\run-windows.ps1
```

It:

1. verifies Windows, Git source identity, clean-tree state and benchmark capability wiring;
2. selects `benchmark-results\adr-019` as the controlled evidence directory unless another directory is supplied;
3. forces a clean standalone desktop rebuild;
4. launches the release Tauri application with exact source provenance and the controlled evidence directory;
5. requires the newly written report to match the measured commit, Tauri/WebView2 runtime, physical 3840×2160 client minimum and the four-case report contract;
6. retains and reports the raw JSON regardless of whether the mechanical SVG performance gate passes or fails.

For the Phase-1 renderer decision, prefer the combined runner so all automated evidence is archived under one provenance contract.

## Archived evidence verification

A retained combined session can be checked independently later:

```powershell
.\benchmarks\phase-1\verify-target-evidence.ps1 `
  -SessionDirectory .\benchmark-results\phase-1-target\target-<timestamp>-<commit>
```

The verifier checks:

- combined manifest schema and renderer-decision non-ownership;
- commit and both Cargo.lock Git object IDs;
- SHA-256 hashes of PreparedPage and renderer raw evidence;
- PreparedPage release profile, 5k/20k counts and structured metrics against raw output;
- working-tree cleanliness against PreparedPage raw evidence;
- renderer `sourceDirty` against the same source state;
- clean/diagnostic decision eligibility classification;
- Windows/Tauri/WebView2 and physical-4K requirements;
- renderer summary values against the raw ADR-019 report;
- nested fidelity-manifest hash and source provenance;
- fidelity eligibility classification against the combined session;
- fidelity summary/non-ownership values against the nested manifest;
- nested SVG and diagnostic hashes, passive SVG content and required typed diagnostics through `verify-fidelity-evidence.ps1`.

Older retained Phase-1 v1 archives that predate nested fidelity evidence remain verifiable; new combined target runs always include the fidelity section.

## CI configuration smoke

Hosted Windows CI must not be treated as ADR-019 or PreparedPage target-hardware performance evidence. It runs configuration and synthetic verification only, including:

```powershell
.\benchmarks\phase-1\run-target-evidence-windows.ps1 `
  -ValidateOnly `
  -OutputDirectory "$env:RUNNER_TEMP\phase-1-target-evidence"

.\benchmarks\phase-1\test-evidence-verifier.ps1
```

Current Windows CI also validates the standalone fidelity exporter/verifier and the generic private-corpus harness in `-ValidateOnly` mode without reading any private manifest or fixture.

The synthetic combined verifier test covers clean decision-eligible sessions, dirty diagnostic-only sessions and nested fidelity archives, creates a prefilled renderer-review draft from realistic culled/full measurement fields, and then rejects tampering of eligibility, dirty-state summaries, structured PreparedPage metrics, nested fidelity hashes and raw evidence.

## Decision rule

The raw native JSON, PreparedPage release result and deterministic fidelity archive are inputs to ADR-019. SVG can become the production renderer only after:

- representative physical-4K native evidence exists;
- the mandatory culled 5k and 20k cases satisfy the mechanical gate;
- DOM/Long Task constraints are satisfied;
- PreparedPage release rebuild latency is acceptable or measured evidence justifies an incremental-patching implementation;
- the manual correctness/fidelity review is acceptable and recorded.

Otherwise the documented Canvas2D/WebGL/Qt fallback path must be evaluated. Neither benchmark runner nor archived-evidence verifier records the final renderer choice.
