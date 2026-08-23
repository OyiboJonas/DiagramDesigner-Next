# DiagramDesigner Next

Modern, migration-safe successor to Diagram Designer.

> Status: Phase 1 — editor foundation and renderer decision in progress

DiagramDesigner Next is an independent architectural rewrite with explicit compatibility tooling for legacy `.ddd` and `.ddt` documents. The original `meesoft/DiagramDesigner` project is used as the functional and legacy-format reference; see `THIRD_PARTY_NOTICES.md`.

Phase 0 established the bounded legacy decoding/migration boundary, renderer-independent `next-domain` model and deterministic DDNX persistence. Phase 1 builds the editor foundation, desktop persistence/recovery, interaction model and replaceable renderer candidate on top of that boundary.

## Phase-1 renderer gate

The editor/application foundations, atomic desktop persistence, recovery, command history, viewport interaction, snapping/rulers, keyboard accessibility, PreparedPage caching and the SVG renderer candidate are implemented.

SVG is **not** yet the selected production renderer. The remaining Phase-1 exit gate is representative target-hardware evidence plus correctness/fidelity review under ADR-019.

On representative Windows hardware with a physical client area of at least 3840×2160, run:

```powershell
.\benchmarks\phase-1\run-target-evidence-windows.ps1
```

The combined target run archives:

- PreparedPage 5k/20k **release** rebuild/cache evidence;
- native fullscreen Tauri/WebView2 SVG evidence;
- deterministic ADR-019 fidelity-scene evidence through `render-plan → render-svg`;
- source/lockfile provenance and evidence hashes;
- a prefilled `adr-019-renderer-decision-review.md` for human review.

A clean source tree is required for evidence that may participate in the renderer decision. Diagnostic dirty-tree runs cannot be promoted into decision evidence.

## Local tooling validation

Phase-1 evidence/review tooling can be checked independently of GitHub Actions:

```powershell
.\benchmarks\phase-1\validate-tooling.ps1
```

On Windows, use:

```powershell
.\benchmarks\phase-1\validate-tooling.ps1 -WindowsConfiguration
```

This validates the target-runner/fidelity/private-corpus configuration without reading private fixture files.

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
