# Cleanup-safe local Windows Phase-1 runs

Local Rust/Tauri builds can consume multiple gigabytes even though the retained Phase-1 evidence files are small. The cleanup-safe wrappers keep those generated build artifacts out of the repository by setting `CARGO_TARGET_DIR` to a unique temporary directory and removing that directory in a `finally` block.

## Tooling validation

Prefer:

```powershell
.\benchmarks\phase-1\validate-tooling-clean.ps1 -WindowsConfiguration
```

The wrapper requires at least 3 GB free on the build drive by default. Override the build root when another drive should carry the temporary compilation:

```powershell
.\benchmarks\phase-1\validate-tooling-clean.ps1 `
  -WindowsConfiguration `
  -CargoBuildRoot "D:\DiagramDesigner-Next-builds"
```

## Representative target evidence

Prefer:

```powershell
.\benchmarks\phase-1\run-target-evidence-windows-clean.ps1
```

For a real target run the wrapper requires at least 8 GB free on the build drive before compilation starts. A configuration-only run uses a maximum 3 GB threshold:

```powershell
.\benchmarks\phase-1\run-target-evidence-windows-clean.ps1 -ValidateOnly
```

If another drive has more free space, redirect only the disposable build artifacts there while keeping the evidence output in its normal repository location:

```powershell
.\benchmarks\phase-1\run-target-evidence-windows-clean.ps1 `
  -CargoBuildRoot "D:\DiagramDesigner-Next-builds"
```

The wrapper creates a unique `ddn-cargo-target-*` child directory under the selected build root. It never deletes the build root itself. The unique target directory is removed after success or failure.

Use `-KeepBuildArtifacts` only when compiler outputs are needed for diagnostics. The wrapper then prints the retained target path explicitly.

## Fidelity review viewer

After a successful real target run the cleanup-safe wrapper creates `adr-019-fidelity-review.html` in the new target-session directory. The viewer is generated only after the combined evidence archive has passed verification.

The viewer:

- embeds the retained fidelity SVG as passive image data rather than inline SVG markup;
- shows source/evidence hashes and decision eligibility;
- shows PreparedPage and native SVG performance metrics beside the scene;
- exposes 50%, 100%, 150% and 200% viewer zoom controls;
- presents the complete Phase-1 manual fidelity checklist;
- keeps checklist selections as local browser review state only;
- can build/copy a text summary for the human decision record;
- cannot modify evidence files or select a renderer.

Open it automatically after the target run with:

```powershell
.\benchmarks\phase-1\run-target-evidence-windows-clean.ps1 -OpenReviewViewer
```

For an existing completed target session, regenerate the derived viewer independently with:

```powershell
.\benchmarks\phase-1\prepare-fidelity-review-viewer.ps1 `
  -SessionDirectory .\benchmark-results\phase-1-target\target-<timestamp>-<commit> `
  -Open
```

`-Force` is required to replace an existing derived viewer. This never changes the immutable target, renderer or fidelity evidence archives.

## What is retained

The cleanup-safe wrappers remove Cargo/Tauri build products only. They do not remove:

- `benchmark-results` evidence or derived local review files;
- source files or Git metadata;
- Rust toolchains under `.rustup`;
- Cargo tools/cache under `.cargo`;
- Visual Studio Build Tools or Windows SDKs.

The underlying original runners remain available for CI and low-level diagnostics. For interactive local Phase-1 work, use the cleanup-safe wrappers by default.
