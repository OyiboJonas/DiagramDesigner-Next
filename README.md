# DiagramDesigner Next

Modern, migration-safe successor to Diagram Designer.

> Status: 0.1.0-alpha.1 candidate — Windows desktop alpha

DiagramDesigner Next is an independent architectural rewrite with explicit compatibility tooling for legacy `.ddd` and `.ddt` documents. The original `meesoft/DiagramDesigner` project is used as the functional and legacy-format reference; see `THIRD_PARTY_NOTICES.md`.

Phase 0 established the bounded legacy decoding/migration boundary, renderer-independent `next-domain` model and deterministic DDNX persistence. Phase 1 builds the editor foundation, desktop persistence/recovery, interaction model and replaceable renderer boundary on top of that foundation.


## 0.1 Windows alpha

The first alpha packages the current editor foundation into one functional Windows workflow: native DDNX open/save, migration-safe `.ddd`/`.ddt` import, crash recovery, pages/layers, basic shapes and text, straight/orthogonal connectors with ports, selection/move/snapping, direct resize/rotation, basic appearance, and Undo/Redo. Current post-baseline builds also include structured clipboard operations, grouping/z-order, alignment/distribution, Save As with overwrite confirmation, and editable connector markers/line styles.

The alpha remains intentionally conservative around persistence: legacy sources are import-only, first Save and Save As require explicit native confirmation before replacing a newly selected existing DDNX target, and dirty window close writes a fresh atomic recovery checkpoint before the native window is allowed to close. The WebView still receives no broad filesystem or shell capability.

The unsigned portable Windows artifact and the exact alpha smoke path/known limitations are documented in [`docs/testing/alpha-0.1.md`](docs/testing/alpha-0.1.md).

## Phase-1 renderer decision

The editor/application foundations, atomic desktop persistence, recovery, command history, viewport interaction, snapping/rulers, keyboard accessibility, PreparedPage caching and SVG renderer path are implemented.

ADR-019 selected SVG as the Phase-1 production renderer after representative Windows target evidence and manual fidelity review. The decision checkpoint is source commit `6c5595d62ddb905ed864203230c5a7786b36f860` and is recorded in `docs/architecture/adr-019-renderer-decision-record.md`.

The decision evidence established:

- decision-eligible, non-diagnostic combined target evidence;
- acceptable immutable PreparedPage rebuild/cache behavior at 5k and 20k;
- native fullscreen Tauri/WebView2 `performance_gate_pass` at physical 3840×2160;
- viewport-bounded SVG DOM and no recurring Long Tasks under the mechanical gate;
- all required manual fidelity checks correct with zero blocking defects;
- explicit typed diagnostics for deferred marker and unsupported primitive semantics.

SVG remains behind the renderer abstraction. `next-domain`, editor history/commands and renderer-neutral geometry do not own SVG DOM state, so a future backend change remains possible without rewriting the document/editor model.

## Reproducing Phase-1 target evidence

On representative Windows hardware with a physical client area of at least 3840×2160, use the cleanup-safe runner:

```powershell
.\benchmarks\phase-1\run-target-evidence-windows-clean.ps1 -OpenReviewViewer
```

The combined target run archives:

- PreparedPage 5k/20k **release** rebuild/cache evidence;
- native fullscreen Tauri/WebView2 SVG evidence;
- deterministic ADR-019 fidelity-scene evidence through `render-plan → render-svg`;
- source/lockfile provenance and evidence hashes;
- a prefilled `adr-019-renderer-decision-review.md` and local fidelity-review viewer.

A clean source tree is required for evidence that may participate in an architecture decision. Diagnostic dirty-tree runs cannot be promoted into decision evidence. Large Cargo/Tauri build products are isolated and cleaned by the wrapper; retained evidence remains under `benchmark-results/` and is ignored by Git.

## Local tooling validation

Phase-1 evidence/review tooling can be checked independently of GitHub Actions:

```powershell
.\benchmarks\phase-1\validate-tooling-clean.ps1
```

For lower-level configuration checks, the underlying validator remains available:

```powershell
.\benchmarks\phase-1\validate-tooling.ps1 -WindowsConfiguration
```

This validates the target-runner/fidelity/private-corpus configuration without reading private fixture files and does not itself create representative target evidence.

## Private legacy compatibility evidence

Private/company legacy documents are **not** part of this repository. The public code contains only a generic manifest example and a local verification harness.

Create a private manifest outside the repository from `fixtures/private-corpus.example.json`, point it at locally controlled fixtures, and run:

```powershell
.\benchmarks\private-ddd\prepare-private-corpus.ps1 `
  -ManifestPath <path-to-private-manifest.json>
```

Private fixture names, paths, source fingerprints, decoded contents and review outputs must remain outside the public repository unless explicitly approved for publication. `benchmark-results/` and local private fixture locations are ignored by Git.

See `docs/phase-0/private-corpus-verification.md` for the compatibility contract.

## Architecture rules

- Persistent document mutation goes through typed commands/transactions.
- `next-domain` remains renderer-, platform- and legacy-storage-independent.
- Renderer adapters consume renderer-independent plans rather than owning editor geometry/state.
- Pointer-move hot paths remain frontend/transient; persistent mutation occurs only on semantic commit.
- DDNX is the package codec; filesystem durability belongs to the platform adapter.
- WebViews receive no generic shell, arbitrary-command or broad filesystem surface.

## Upstream compatibility reference

The CI compatibility gate checks out a pinned revision of `meesoft/DiagramDesigner` and exercises its public DDT template palettes. DiagramDesigner Next does not claim ownership of upstream Diagram Designer source code or assets.

## License

DiagramDesigner Next is released under the MIT License. See `LICENSE` and `THIRD_PARTY_NOTICES.md`.
