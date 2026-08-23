# SVG DOM benchmark

This benchmark is the browser/WebView half of the Phase-1 renderer exit criterion. It is intentionally dependency-free and remains separate from the normal editor rendering path.

## What it measures

The synthetic scene uses the same 5,000 / 20,000 mixed-node scale as `render-plan-bench` and a 960 × 540 document-space viewport corresponding to a native 3840 × 2160 target surface at 4 px per document unit.

Two modes are measured:

- `viewport-culling`: only elements intersecting the current viewport plus a 12-unit margin exist in the SVG DOM. The benchmark rebuilds the visible set for each camera position, so the number includes a deliberately conservative DOM-update cost.
- `full DOM`: all requested elements stay mounted and panning changes only the SVG `viewBox`. This is diagnostic; it is not the preferred architecture and never substitutes for the culled acceptance cases.

For every case the benchmark records DOM-node range, JavaScript/update p50/p95/p99/max, frame p50/p95/p99/max, frames over 16.67 ms, browser/GPU metadata, the measured physical SVG surface, and Long Tasks where the API is available.

## Diagnostic browser run

Serve the repository root with any local static server, for example:

```sh
python -m http.server 8000
```

Then open:

```text
http://localhost:8000/benchmarks/svg-dom/
```

The browser page is useful for development and regression diagnostics only. It **cannot** satisfy ADR-019 because the acceptance evaluator requires the Windows Tauri/WebView2 runtime.

## Native ADR-019 run

Run the desktop application on representative Windows target hardware and choose **4K benchmark** from the editor toolbar. The Rust desktop boundary opens an isolated fullscreen Tauri window loading `renderer-benchmark.html`.

Security and measurement properties:

- the main editor window can only request that the benchmark window be opened/focused;
- the benchmark window can only read its own physical client/monitor metadata and close itself;
- no generic filesystem, shell, arbitrary-command or document-mutation capability is granted to the benchmark window;
- window creation is performed from an async Tauri command to avoid the documented Windows/WebView2 synchronous creation deadlock hazard;
- the native environment is eligible only when the physical WebView client is at least **3840 × 2160**;
- during measurement all benchmark controls/results are hidden and the SVG occupies the entire WebView client area;
- each culled acceptance result must itself record a physical SVG stage of at least **3840 × 2160**. A 4K window containing a smaller measured stage is rejected as incomplete evidence.

Enter the machine model and useful hardware notes before the run. The generated JSON report contains native client/monitor metadata, browser/WebView user-agent data, GPU information when exposed by WebGL, all four benchmark cases, and the mechanical performance verdict. Copy that JSON into the renderer benchmark record together with any additional Windows/GPU-driver/power-mode information needed for the representative-hardware review.

## Acceptance gate

CI timings and ordinary browser runs are trend/diagnostic data only. The renderer decision must be made on representative Windows hardware in the intended Tauri/WebView2 stack at native 3840 × 2160 output.

SVG passes the measured performance portion of ADR-019 only if **both culled 5k and culled 20k** cases demonstrate:

- physical measured SVG surface at least 3840 × 2160;
- p95 frame time at or below **16.67 ms**;
- no observed recurring Long Tasks during pan;
- viewport-bounded DOM population (current guard: at most 1,500 scene nodes), rather than growth proportional to total document size.

Missing culled evidence, unavailable Long Task evidence, invalid timings, or a sub-4K measured stage produces `measurement_incomplete`. Material p95/Long-Task/DOM misses produce `fallback_required`. A `performance_gate_pass` is **not by itself the final renderer decision**: correctness/fidelity evidence and representative-hardware review still have to be accepted.

If the culled 20k case misses materially or behaves inconsistently across representative Windows hardware, the renderer abstraction remains in place and Canvas2D/WebGL or the Qt fallback is benchmarked before SVG is promoted to production.

## Relationship to Rust planning

`cargo run --locked -p render-plan --bin render-plan-bench -- 5000 20000` measures document traversal and viewport-culling plan construction without browser DOM cost. A fast Rust plan does not prove SVG viability; both layers must be measured independently.

The remaining Phase-1 target-hardware task also includes measuring `PreparedPage` rebuild/cache behavior on representative hardware. Incremental prepared-scene patching is only added if that measurement justifies the complexity.
