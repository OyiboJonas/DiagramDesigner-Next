# ADR-019 — Renderer abstraction and SVG exit criterion

- Status: Accepted — SVG selected for Phase 1
- Date: 2026-08-20; renderer selection recorded 2026-08-23
- Scope: render planning, prepared render scenes, SVG renderer, viewport culling, desktop renderer selection

## Context

DiagramDesigner Next needs a renderer that preserves diagram semantics while remaining responsive with large documents on 4K displays. The migration study selected SVG as the first renderer candidate because it maps well to vector diagrams, text, accessibility and browser tooling, but explicitly required an early exit benchmark before the application becomes coupled to SVG DOM behavior.

The editor hot path is already separated from persistence and IPC by ADR-018. The same separation is required between the domain model and the concrete renderer.

Initial Phase-1 measurements also showed that reconstructing scene lookup maps and traversing every document root for every camera movement scales with total document size even when only a small viewport subset is visible. That cold planning path is useful for validation and snapshot construction, but it is not the production camera hot path.

## Decision

### Renderer-independent planning precedes concrete drawing

`next-domain` does not emit SVG, Canvas calls or platform drawing commands.

A dedicated `render-plan` layer converts one document page plus viewport options into a back-to-front list of renderable leaf elements and diagnostics. It owns traversal and viewport culling but does not own styling, DOM mutation or interaction state.

The ordering contract is:

```text
visible master layers
  → visible page-local layers
    → scene root order
      → group child order
        → renderable leaf primitives
```

Groups are structural and are expanded by the planner. They are not themselves a renderer primitive.

### Cold traversal and camera-hot queries are separate paths

`build_page_plan` remains the canonical cold path for full scene traversal, graph diagnostics and one-off planning.

Repeated camera queries use `PreparedPage` instead:

1. traverse and validate the page once;
2. clone renderable leaf elements into an immutable renderer-owned snapshot;
3. assign each leaf a stable draw ordinal that already encodes master/page/root/group ordering;
4. compute conservative culling bounds once;
5. insert those bounds into a document-space spatial grid;
6. query only cells intersecting the viewport plus culling margin;
7. sort/deduplicate candidates by the stable draw ordinal before exact intersection testing.

The prepared query therefore avoids rebuilding per-layer maps and walking all roots on every pan/zoom frame.

Very large elements are kept in a bounded global bucket rather than copied into an unbounded number of cells. Likewise, pathologically large viewport queries fall back to the prepared leaf list instead of iterating an unbounded grid range. These are global index-safety rules, not page-specific optimizations.

### A prepared scene is a snapshot of persistent document state

`PreparedPage` owns cloned renderable leaf elements and does not silently track mutations of the editor document.

A persistent document mutation — command transaction, undo or redo — invalidates a prepared snapshot for the affected document state. The application/renderer cache must therefore associate a prepared page with the editor's stable persistent history state plus page identity, or rebuild/update the snapshot after the state changes.

Transient pointer previews do **not** invalidate the prepared scene. They are rendered as overlays on top of the immutable prepared/base state, consistent with ADR-018.

Full snapshot rebuild is the Phase-1 correctness baseline. The representative target measurement recorded below shows no material reason to introduce incremental prepared-scene mutation before renderer promotion. Incremental geometry/style/structural patching remains a later optimization option behind the same API if future real workloads demonstrate a user-visible need.

### Invalid scene structure fails visibly, not silently

Although a valid `next-domain` document should already satisfy graph invariants, render planning remains defensive. Missing roots/children, group cycles, duplicate traversal and invalid geometry are surfaced as diagnostics. Renderers must not silently reinterpret broken graph structure.

Prepared-scene construction is derived from the same cold planner, so it inherits the same traversal order and diagnostics contract. Prepared viewport tests must remain equivalent to cold-plan results for the same document and viewport.

### Viewport culling is a global renderer capability

The renderer contract assumes that large documents are not represented by a DOM node for every document element at all times. `render-plan` therefore accepts a document-space viewport plus culling margin and produces only intersecting leaf elements.

Rotated elements are culled using a conservative axis-aligned bounding box. This may retain extra near-edge elements but must never clip a valid rotated element solely because its unrotated bounds lie outside the viewport.

### SVG is the Phase-1 production renderer, not an irreversible platform choice

SVG was the first concrete renderer candidate and has now passed the representative Phase-1 target gate. It is therefore selected as the Phase-1 production renderer.

Product/application code must still not depend on SVG-specific DOM structure. Renderer-specific code consumes the prepared render plan and transient interaction overlays through a narrow adapter. The production desktop wiring uses a stable SVG facade over the evidence-tested adapter rather than moving SVG state into editor/domain ownership.

Canvas2D/WebGL and Qt remain possible future backends behind the same planning/domain/editor boundaries if later workloads produce evidence for a change. Their evaluation is no longer a Phase-1 prerequisite.

### Two benchmark layers are required

The benchmark is deliberately split:

1. `render-plan-bench` and the PreparedPage benchmark measure the cold Rust traversal/snapshot path, immutable rebuild/cache behavior and repeated viewport queries at 5,000 and 20,000 mixed elements. CI records smoke/trend data but hosted-runner wall-clock values are not a release gate.
2. the native ADR-019 benchmark measures actual Tauri/WebView2 SVG behavior at a 3840 × 2160 target surface, including renderer update time, frame cadence, DOM population and Long Tasks.

A fast prepared planner alone does not prove SVG viability.

### Target-WebView acceptance criterion

The renderer decision is made on representative Windows target hardware and the intended WebView2/Tauri stack at native 3840 × 2160 output.

SVG remains the primary renderer only if viewport-culling mode passes both 5k and 20k mixed-element cases after warm-up:

- renderer update p95 ≤ `1000 / 60 = 16.667 ms`;
- rAF frame p95 ≤ `17.500 ms`, allowing only a bounded 5% VSync/timestamp-quantization tolerance while keeping the renderer-work budget strict;
- no recurring Long Tasks during pan/zoom;
- visible SVG DOM population remains bounded by the viewport/culling margin instead of growing proportionally with total document size;
- no correctness loss in z-order, master-layer ordering or primitive visibility.

The rAF allowance is **not** additional renderer work budget. It prevents normal 60 Hz callback quantization (for example 16.7–16.8 ms) from being misclassified as renderer work above budget.

The full-DOM case is diagnostic and is not required to pass.

A material miss of the 20k culled target remains an architecture exit signal, not a request for page-specific CSS/DOM workarounds.

## Consequences

### Positive

- domain and editor logic stay independent of renderer technology;
- master-layer/page ordering and group traversal are tested once globally;
- viewport culling is explicit before SVG DOM work begins;
- camera planning scales with the spatial candidate set instead of requiring full scene traversal;
- transient gestures do not invalidate persistent render snapshots;
- performance decisions are based on target-stack measurement rather than intuition;
- a future switch to Canvas/WebGL/Qt does not require rewriting commands, DDNX or legacy conversion;
- no per-page renderer hacks are accepted as a substitute for meeting the benchmark.

### Cost

- there is an additional prepared render-scene representation between domain and renderer;
- prepared snapshots duplicate renderable leaf state and need explicit invalidation/rebuild after persistent edits;
- culling, spatial indexing and group traversal require their own tests/diagnostics;
- SVG DOM and Rust planning require separate measurement tooling;
- the selected SVG adapter must retain explicit diagnostics for deferred/unsupported semantics rather than silently approximating them.

## Phase-1 implementation checkpoint

`render-plan` establishes:

- renderer-independent page planning;
- master-layer-before-page-layer ordering;
- visible-layer filtering;
- group expansion in declared child order;
- defensive graph diagnostics;
- document-space viewport culling with margin;
- conservative rotated-element AABB handling;
- immutable `PreparedPage` snapshots;
- bounded spatial-grid indexing with stable draw-order restoration;
- full-view and viewport equivalence tests between cold and prepared planning;
- deterministic 5k/20k planning and PreparedPage benchmarks.

The native SVG benchmark establishes:

- dependency-free benchmark logic inside the Tauri/WebView2 target stack;
- 5k and 20k mixed-element scenes;
- culled and full-DOM comparison modes;
- native-4K target metadata;
- renderer-update and rAF p50/p95/p99/max timing;
- DOM-node range and Long Task reporting;
- provenance-bound retained evidence.

### Hosted-CI trend checkpoint — 2026-08-20

On one GitHub-hosted Ubuntu runner, using the same 120 viewports and visible range of 826..918 elements:

- 5k cold: p50 16.588 ms / p95 16.841 ms;
- 5k prepared: p50 0.543 ms / p95 0.678 ms; snapshot build 21.860 ms;
- 20k cold: p50 77.391 ms / p95 80.929 ms;
- 20k prepared: p50 0.559 ms / p95 0.694 ms; snapshot build 97.970 ms.

The 20k prepared query therefore remains in approximately the same sub-millisecond range as 5k for this synthetic viewport, while the cold path scales strongly with total elements. This validated the prepared-scene architecture direction, not the renderer selection. Hosted-runner timing remains non-deterministic trend data and does not replace target evidence.

### Representative Windows target checkpoint — 2026-08-23

Decision source commit: `6c5595d62ddb905ed864203230c5a7786b36f860`.

The combined archive was verified with `eligibleForPhase1Decision=true` and `diagnosticOnly=false`.

PreparedPage release evidence:

- 5k rebuild p95 `2001 us`, cache-hit p95 `100 ns`, forced-eviction rebuild `1318 us`;
- 20k rebuild p95 `8967 us`, cache-hit p95 `100 ns`, forced-eviction rebuild `8980 us`.

Conclusion: keep immutable PreparedPage rebuilds for Phase 1. The measurements do not justify incremental patching before renderer promotion.

Native SVG evidence:

- physical target client: `3840x2160`;
- mechanical verdict: `performance_gate_pass`;
- Long Task requirement: pass;
- viewport-bounded DOM requirement: pass.

Manual fidelity review:

- all ten required review checks: correct;
- blocking fidelity defects: `0`;
- `ConnectorMarkerDeferred` remains an explicit typed diagnostic;
- `UnsupportedPrimitive` remains an explicit typed diagnostic.

Final Phase-1 renderer decision: **SVG selected as the production renderer**. Full traceability is recorded in `docs/architecture/adr-019-renderer-decision-record.md`.

The measured squash-merge commit and the fully green PR #7 head share the exact Git tree `f788f8757678d8ec3cefeb7e7283a34540451fc8`; the PR validations therefore exercised the same source tree used for the representative evidence.

## Follow-on requirements

Completed for the Phase-1 decision:

- [x] bind prepared-scene cache invalidation to the editor persistent history state/page identity at the application boundary;
- [x] measure full snapshot rebuild latency on representative target hardware;
- [x] run the native SVG benchmark on representative Windows target hardware/WebView2;
- [x] complete deterministic fidelity evidence and manual review;
- [x] select the Phase-1 renderer from evidence;
- [x] promote the measured SVG path through a stable production adapter/facade.

Continuing work after Phase 1:

- keep transient interaction overlays free of persistent document mutation;
- expand connector/text/image rendering parity as those semantics are implemented;
- retain typed diagnostics for deferred features;
- preserve Canvas2D/WebGL/Qt as renderer-abstraction options rather than parallel Phase-1 implementations;
- reopen incremental PreparedPage design only if future representative workloads demonstrate material user-visible rebuild latency.

Tracks #2.
