# ADR-019 renderer correctness and fidelity review

## Purpose

ADR-019 does not allow the native performance benchmark to select the production renderer by itself. This document defines the separate correctness/fidelity review that must be completed before SVG can be promoted from candidate to production renderer.

The review is intentionally renderer-facing and does not move rendering semantics into `next-domain`, editor commands or legacy migration code.

## Decision boundary

A renderer candidate is acceptable only when all three evidence groups agree:

1. **renderer-independent correctness** — the same document state produces the correct render plan, ordering and viewport membership;
2. **candidate output fidelity** — the concrete renderer represents the currently required primitive/style/text semantics without silent loss;
3. **representative target performance** — ADR-019 native physical-4K evidence passes the mechanical gate.

A performance pass cannot override a fidelity failure. Likewise, fidelity alone cannot override a material target-performance failure.

## Phase-1 fidelity scope

The Phase-1 production-renderer decision is limited to the visible baseline already implemented by the candidate adapter:

- rectangle and rounded rectangle;
- ellipse;
- element rotation;
- typed rich text and page fields;
- straight connectors;
- connector dash styles in the supported baseline;
- stroke, fill, alpha and linear-gradient style baseline;
- master-layer-before-page-layer z-order;
- group expansion order;
- viewport culling without losing visible rotated elements;
- renderer diagnostics for any unsupported or approximated semantics.

Features that are explicitly deferred by the domain/renderer contract are not silently treated as supported merely because an SVG approximation can be drawn.

## Automated evidence matrix

### Render-plan invariants

The renderer-independent planning layer must demonstrate:

- visible master layers render before page-local layers;
- hidden layers do not render;
- scene roots preserve declared order;
- group children preserve declared order;
- structural groups expand without becoming drawing primitives;
- invalid scene relationships surface diagnostics instead of being silently reinterpreted;
- cold and `PreparedPage` viewport results are equivalent for the same persistent document state;
- rotated elements use conservative document-space culling bounds.

These invariants belong in `render-plan` / `editor-runtime` tests and remain valid if SVG is replaced.

### SVG candidate invariants

The candidate adapter must demonstrate through deterministic tests:

- rectangle, rounded-rectangle, ellipse, text and straight-connector emission;
- element output order follows `RenderPlan` order;
- rotation is represented from document geometry rather than DOM-derived geometry;
- stroke/fill colors and alpha are preserved within the candidate's declared baseline;
- linear gradients use the declared axis and colors;
- supported line styles produce deterministic dash semantics;
- rich text is escaped as inert text and legacy action/hint tails are never executed;
- page name/number/count fields resolve from the current document/page context;
- invalid geometry is skipped with an explicit diagnostic;
- missing style references fail explicitly;
- unsupported primitives, deferred connector markers, system-palette fallback and approximated line styles remain explicit diagnostics.

A renderer test must never convert one of those diagnostics into an implicit success claim.

### Frontend presentation invariants

The Tauri/WebView presentation layer must demonstrate:

- active-content SVG nodes and event-handler attributes are rejected before insertion;
- transient move previews do not mutate persistent document/history state;
- final pointer-up produces at most one semantic move command;
- snap guides, focus styling and selection overlays do not alter renderer-owned document geometry;
- candidate presentation refresh preserves keyboard focus where the interaction contract requires it.

## Source-bound fidelity fixture

The repository contains `crates/render-svg/examples/fidelity_scene.rs`. It constructs a valid `next-domain` document and sends it through the real `render-plan → render-svg` candidate path. It is not a hand-authored SVG mock.

The fixture contains:

- an overlapping master-layer rectangle and page-local rounded rectangle;
- positive and negative rotations;
- RGBA/alpha styles;
- linear gradients along both supported axes;
- rich text with Unicode, XML-sensitive characters and page fields;
- representative dotted/dash-dot connectors;
- a connector marker that must remain an explicit deferred diagnostic;
- an unsupported polygon sentinel that must remain an explicit diagnostic;
- four partially clipped rotated page-edge sentinels, one on each page edge.

For a source-bound Windows review session run:

```powershell
.\benchmarks\phase-1\export-fidelity-scene-windows.ps1
```

The runner requires a clean tree for decision evidence, executes the fixture through the locked dependency graph, rejects active SVG content, requires the expected typed diagnostics, hashes the SVG and diagnostic log, writes `adr-019-fidelity-evidence.json` with source commit and root `Cargo.lock` Git-blob provenance, and immediately verifies the retained archive.

A retained session can be verified again independently:

```powershell
.\benchmarks\phase-1\verify-fidelity-evidence.ps1 `
  -SessionDirectory .\benchmark-results\phase-1-fidelity\fidelity-<timestamp>-<commit>
```

The verifier rechecks source/eligibility classification, fixture summary, relative evidence paths, both SHA-256 hashes, expected typed diagnostics, passive SVG content and non-ownership of both the manual review and final renderer decision.

`-AllowDirtyTree` is diagnostic only. Such a run is retained with `eligibleForPhase1Decision = false` and `diagnosticOnly = true`. The runner deliberately records `manualReview.status = not-reviewed-by-runner` and `finalRendererDecision = not-made-by-runner`.

`-ValidateOnly` compiles the fidelity example through the locked dependency graph and runs a synthetic verifier fixture that accepts clean/diagnostic archives while rejecting eligibility, review-status and raw-SVG tampering. It does not create target evidence.

## Manual target review

After a source-bound target run has produced evidence marked `eligibleForPhase1Decision = true`, inspect the generated fidelity SVG from the same source commit used for the target measurement. Confirm at least:

1. the local foreground rectangle is visibly above the overlapping master-layer rectangle;
2. normal/rounded rectangles and positive/negative rotations are geometrically correct;
3. ellipse stroke/fill/alpha rendering is correct;
4. both gradient axes and their alpha are correct;
5. Unicode, escaped XML-sensitive characters and page fields are legible and correct;
6. representative supported dotted/dash-dot connector styles are correct;
7. all four rotated page-edge sentinels remain visibly clipped at the appropriate edge instead of disappearing;
8. the deferred marker and unsupported polygon remain visible in the diagnostic log rather than being silently treated as supported.

Review at 100% scale and at representative zoom levels. If additional line styles become part of the Phase-1 production baseline, add them to the fixture before renderer promotion rather than relying only on automated mapping tests.

Record any discrepancy as one of:

- **correct** — matches the declared Phase-1 semantics;
- **acceptable approximation** — already represented by an explicit typed diagnostic and accepted for this phase;
- **blocking fidelity defect** — incorrect geometry, ordering, visibility, text, style or silent semantic loss;
- **out of Phase-1 scope** — genuinely deferred functionality that is not required for the renderer decision.

## Promotion rule

SVG may be selected as the production renderer only when:

- the combined target evidence is decision-eligible rather than diagnostic-only;
- the PreparedPage release measurement does not justify an unresolved architecture change;
- the native culled 5k and 20k SVG cases pass the ADR-019 mechanical performance gate;
- the automated invariant suites are green for the measured source commit;
- the manual fidelity scene has no blocking fidelity defect;
- every known approximation/deferred feature remains explicit rather than silent.

If any condition fails materially, keep the renderer abstraction intact and evaluate the documented Canvas2D/WebGL/Qt fallback path instead of introducing page-specific SVG workarounds.

## Review record

The final renderer decision should record, at minimum:

- source commit;
- combined target-evidence session path/hash manifest;
- fidelity-evidence manifest and SVG/diagnostic hashes;
- automated CI run used for the same source commit;
- PreparedPage conclusion (`immutable rebuild accepted` or `incremental patching required`);
- ADR-019 mechanical performance verdict;
- fidelity-review outcome and any accepted typed diagnostics;
- final renderer choice and rationale.

The benchmark runner and evidence verifier intentionally do not write this decision record themselves.
