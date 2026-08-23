# PreparedPage rebuild and cache benchmark contract

## Purpose

Phase 1 already has an immutable `PreparedPage` plus a bounded application cache keyed by the persistent `PageStateKey = (HistoryStateId, PageId)`. The remaining question is empirical: whether rebuilding a prepared snapshot after a persistent edit is cheap enough on representative target hardware, or whether a later incremental update strategy is justified.

This benchmark exists to answer that question without weakening the current architecture. It does **not** change renderer ownership, history semantics, or the cache key.

The benchmark measures the real `EditorRuntime::prepared_page()` path. It does not use a mock cache or a synthetic cache implementation.

## Architecture invariants under test

The measurement assumes and verifies all of the following:

- a persistent history state owns one immutable prepared page snapshot;
- transient selection/view state reuses the same snapshot;
- a cache miss builds exactly one `PreparedPage`;
- a same-state lookup is a cache hit and performs no rebuild;
- retained historical states are reused by Undo/Redo without rebuilding;
- the cache remains bounded;
- an evicted historical state is rebuilt exactly once when revisited;
- no incremental mutation of a cached `PreparedPage` is introduced unless target-hardware evidence later justifies a dedicated design.

The benchmark must fail rather than silently report timings if these invariants are violated.

## Benchmark executable

The executable lives at:

`crates/editor-runtime/src/bin/prepared-cache-bench.rs`

Default target-sized run:

```text
cargo run --release --locked -p editor-runtime --bin prepared-cache-bench -- 5000 20000
```

The default measurement uses:

- scene sizes: 5,000 and 20,000 elements;
- prepared-page cache capacity: 4 historical page states;
- rebuild samples: 20;
- same-state hit samples: 2,000;
- three retained edited history states for Undo/Redo reuse;
- four edits for the forced-eviction scenario.

Sample counts can be changed for diagnostics:

```text
cargo run --release --locked -p editor-runtime --bin prepared-cache-bench -- --rebuild-samples 40 --hit-samples 5000 5000 20000
```

Do not use reduced sample counts as Phase-1 target-hardware evidence.

## Preferred Windows evidence runner

For the representative Windows machine, prefer the project runner instead of manually assembling metadata and stdout:

```powershell
.\benchmarks\prepared-cache\run-windows.ps1
```

The runner defaults to the required release build, 20 rebuild samples, 2,000 cache-hit samples and 5k/20k scenes. It writes one UTF-8 evidence record under `benchmark-results\prepared-cache` containing the raw benchmark output together with the exact source commit and environment metadata.

Important runner behavior:

- it refuses non-Windows systems;
- it refuses a dirty Git working tree by default, so source identity stays reproducible;
- `-AllowDirtyTree` exists only for diagnostics and records the dirty status in the evidence file;
- `-DebugBuild` exists only for CI/diagnostics and emits a warning; it is **not** valid Phase-1 target evidence;
- reduced `-RebuildSamples`, `-HitSamples` or changed `-Counts` are diagnostics unless explicitly reviewed as a new benchmark contract;
- benchmark stdout is retained even when the cargo process fails;
- the result is accepted only if exactly one schema metadata line and exactly one result line for every requested scene are present.

A custom evidence directory can be supplied without changing the measurement:

```powershell
.\benchmarks\prepared-cache\run-windows.ps1 -OutputDirectory D:\DiagramDesignerBenchmarks\prepared-cache
```

## Required target-hardware context

The Phase-1 measurement must be run on the representative Windows machine used for the editor/renderer decision, not on a hosted CI runner. Record the exact source commit and environment before running the release benchmark.

The preferred runner captures:

- exact Git commit and clean/dirty state;
- local and UTC capture time;
- release/debug profile and requested sample counts;
- Windows version/build/architecture;
- CPU model, core/logical-processor counts and reported maximum clock;
- machine manufacturer/model and physical memory;
- GPU/driver metadata (useful because the same machine is used for ADR-019);
- battery status when Windows exposes one;
- active Windows power scheme;
- PowerShell, `rustc --version --verbose`, and Cargo version;
- complete benchmark stdout and Cargo exit code.

For manual cross-checking, the equivalent core commands are:

```powershell
git rev-parse HEAD
rustc --version --verbose
cargo --version
Get-CimInstance Win32_OperatingSystem |
  Select-Object Caption, Version, BuildNumber, OSArchitecture
Get-CimInstance Win32_Processor |
  Select-Object Name, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed
Get-CimInstance Win32_ComputerSystem |
  Select-Object Manufacturer, Model, TotalPhysicalMemory
```

Also confirm that the target machine is in its intended normal performance condition. Record any material deviation such as battery-only operation, a nonstandard vendor/Windows power profile, OS updates, antivirus full scans, thermal throttling or other heavy background activity.

## Output contract

The first benchmark line identifies the output schema and fixed cache configuration:

```text
BENCH prepared-cache-meta schema=diagramdesigner-next-prepared-cache-v1 ...
```

Each scene then emits one line containing:

- `nodes`: scene element count;
- `rebuild_samples`: number of isolated cache-miss rebuilds;
- `rebuild_p50_us`, `rebuild_p95_us`, `rebuild_p99_us`, `rebuild_max_us`: full `EditorRuntime::prepared_page()` miss plus `PreparedPage::build` cost;
- `hit_samples`: same-state lookup count;
- `hit_p50_ns`, `hit_p95_ns`, `hit_p99_ns`, `hit_max_ns`: already-prepared lookup cost;
- `history_hits`: number of measured Undo/Redo prepared-state reuses;
- `history_p50_ns`, `history_p95_ns`, `history_max_ns`: retained historical-state lookup cost;
- `history_builds`: total builds required to establish the retained history fixture; Undo/Redo itself must not increase it;
- `eviction_rebuild_us`: rebuild cost when returning to a deliberately evicted historical state;
- `eviction_builds`: total build count in the eviction scenario;
- `evictions`: total eviction count in the scenario.

The executable validates the expected hit/miss/build/eviction counters before it prints a successful result.

The Windows evidence runner wraps those unchanged `BENCH` lines in an evidence envelope headed by:

```text
DIAGRAMDESIGNER_NEXT_PREPARED_CACHE_EVIDENCE_V1
```

Do not edit or normalize the raw `BENCH` lines before retaining the target result.

## CI smoke versus target evidence

GitHub Actions runs a deliberately small Linux smoke invocation:

```text
cargo run --locked --quiet -p editor-runtime --bin prepared-cache-bench -- --rebuild-samples 3 --hit-samples 100 5000 20000
```

The Windows job invokes the same project evidence runner with a debug build and reduced sample counts. This proves that metadata capture, evidence-file creation and the real benchmark executable work together on Windows without making hosted-runner timing values decision evidence.

These jobs are **correctness and executable-path regression checks only**. Hosted-runner timing values must not be used to decide whether incremental prepared-page patching is required.

The existing `render-plan` hosted benchmark remains useful as a trend line for cold planning versus prepared viewport queries, but it has the same restriction.

## Decision policy

Phase 1 deliberately does not set an arbitrary universal millisecond threshold for snapshot rebuilds. The decision must be based on representative-hardware evidence and the actual editor interaction model.

Review the target results against these questions:

1. Does a persistent edit that requires a new prepared snapshot cause a noticeable interaction stall at the representative 5k scene size?
2. Does the 20k rebuild cost create unacceptable latency for intended large-document workflows?
3. Are same-state and Undo/Redo cache hits negligible compared with snapshot rebuilds, confirming that the cache is solving the intended historical-navigation case?
4. Does the bounded cache behave as expected under eviction, without hidden repeated rebuilds?
5. Is the rebuild cost large enough, frequent enough, and user-visible enough to justify the complexity and correctness risk of incremental prepared-page patching?

Only if the answer to the last question is supported by the target measurements should Phase 1 open a dedicated incremental-preparation design. Any such design must retain renderer independence, exact `PageStateKey` semantics, deterministic plan equivalence, and bounded historical caching.

If target rebuild behavior is acceptable, keep the simpler immutable rebuild model and proceed to the ADR-019 native 3840×2160 renderer decision.

## Evidence retention

For the representative run, retain together:

- exact DiagramDesigner Next commit SHA;
- the environment metadata above;
- the complete benchmark stdout;
- confirmation that the build used `release`;
- date of measurement;
- any material deviation from the standard test conditions.

The preferred Windows runner puts the machine-readable/raw facts in one evidence file. Attach that file to Phase-1 issue #11 or commit it as a small benchmark record if a stable project-local record is preferred. Add a human interpretation separately; do not replace the raw measurement with only a summarized conclusion.
