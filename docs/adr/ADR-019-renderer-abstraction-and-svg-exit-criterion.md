# ADR-019 — Renderer abstraction and SVG exit criterion

- Status: Accepted for Phase 1
- Date: 2026-08-20
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

Full snapshot rebuild is the Phase-1 correctness baseline. Incremental prepared-scene updates for geometry/style/structural commands may be added later behind the same API if target measurements show rebuild latency to be material; renderers must not become coupled to that implementation detail.

### Invalid scene structure fails visibly, not silently

Although a valid `next-domain` document should already satisfy graph invariants, render planning remains defensive. Missing roots/children, group cycles, duplicate traversal and invalid geometry are surfaced as diagnostics. Renderers must not silently reinterpret broken graph structure.

Prepared-scene construction is derived from the same cold planner, so it inherits the same traversal order and diagnostics contract. Prepared viewport tests must remain equivalent to cold-plan results for the same document and viewport.

### Viewport culling is a global renderer capability

The renderer contract assumes that large documents are not represented by a DOM node for every document element at all times. `render-plan` therefore accepts a document-space viewport plus culling margin and produces only intersecting leaf elements.

Rotated elements are culled using a conservative axis-aligned bounding box. This may retain extra near-edge elements but must never clip a valid rotated element solely because its unrotated bounds lie outside the viewport.

### SVG is first candidate, not an irreversible platform choice

SVG is the first concrete renderer to benchmark. Product/application code must not depend on SVG-specific DOM structure. Renderer-specific code consumes the prepared render plan and transient interaction overlays through a narrow adapter.

If SVG does not satisfy the benchmark gate, the same planning/domain/editor layers remain valid while Canvas2D/WebGL or the Qt fallback is evaluated.

### Two benchmark layers are required

The benchmark is deliberately split:

1. `render-plan-bench` measures both the cold Rust traversal/snapshot path and repeated `PreparedPage` viewport queries at 5,000 and 20,000 mixed elements. CI records these timings as regression trend data but does not use hosted-runner wall-clock values as a release gate.
2. `benchmarks/svg-dom` measures actual browser/WebView SVG DOM behavior at a 3840 × 2160 target surface, including frame times, DOM population and Long Tasks.

A fast prepared planner alone does not prove SVG viability.

### Target-WebView acceptance criterion

The renderer decision is made on representative Windows target hardware and the intended WebView2/Tauri stack at native 3840 × 2160 output.

SVG remains the primary renderer only if viewport-culling mode passes both 5k and 20k mixed-element cases after warm-up:

- p95 camera-motion frame time ≤ 16.67 ms;
- no recurring Long Tasks during pan/zoom;
- visible SVG DOM population remains bounded by the viewport/culling margin instead of growing proportionally with total document size;
- no correctness loss in z-order, master-layer ordering or primitive visibility.

The full-DOM case is diagnostic and is not required to pass.

A material miss of the 20k culled target is an architecture exit signal, not a request for page-specific CSS/DOM workarounds.

## Consequences

### Positive

- domain and editor logic stay independent of renderer technology;
- master-layer/page ordering and group traversal are tested once globally;
- viewport culling is explicit before SVG DOM work begins;
- camera planning scales with the spatial candidate set instead of requiring full scene traversal;
- transient gestures do not invalidate persistent render snapshots;
- performance decisions are based on target-stack measurement rather than intuition;
- switching to Canvas/WebGL/Qt does not require rewriting commands, DDNX or legacy conversion;
- no per-page renderer hacks are accepted as a substitute for meeting the benchmark.

### Cost

- there is an additional prepared render-scene representation between domain and renderer;
- prepared snapshots duplicate renderable leaf state and need explicit invalidation/rebuild after persistent edits;
- culling, spatial indexing and group traversal require their own tests/diagnostics;
- a real Windows/WebView benchmark must be performed before SVG is declared final;
- SVG DOM and Rust planning need separate measurement tooling.

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
- a deterministic 5k/20k cold + prepared planning benchmark.

`benchmarks/svg-dom` establishes:

- dependency-free browser/WebView benchmark shell;
- 5k and 20k mixed-element scenes;
- culled and full-DOM comparison modes;
- native-4K target metadata;
- p50/p95/p99/max update and frame timing;
- DOM-node range and Long Task reporting.

### Hosted-CI trend checkpoint — 2026-08-20

On one GitHub-hosted Ubuntu runner, using the same 120 viewports and visible range of 826..918 elements:

- 5k cold: p50 16.588 ms / p95 16.841 ms;
- 5k prepared: p50 0.543 ms / p95 0.678 ms; snapshot build 21.860 ms;
- 20k cold: p50 77.391 ms / p95 80.929 ms;
- 20k prepared: p50 0.559 ms / p95 0.694 ms; snapshot build 97.970 ms.

The 20k prepared query therefore remains in approximately the same sub-millisecond range as 5k for this synthetic viewport, while the cold path scales strongly with total elements. This validates the prepared-scene architecture direction, **not** the final SVG renderer. Hosted-runner timing is non-deterministic trend data and must not replace the Windows/WebView2 acceptance measurement above.

## Follow-on requirements

- bind prepared-scene cache invalidation to the editor persistent history state/page identity at the application boundary;
- evaluate whether full snapshot rebuild latency after persistent commands requires incremental prepared-scene patching on target hardware;
- run the SVG benchmark on representative Windows target hardware/WebView2 and record the result;
- implement the first production SVG renderer adapter only after the exit criterion is measured;
- add transient interaction-overlay rendering without persistent document mutation;
- add connector/text/image-specific rendering parity tests;
- keep Canvas2D/WebGL and Qt as explicit fallback paths until the SVG gate is passed.

Tracks #11.
