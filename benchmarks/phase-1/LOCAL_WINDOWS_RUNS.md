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

## What is retained

The cleanup-safe wrappers remove Cargo/Tauri build products only. They do not remove:

- `benchmark-results` evidence;
- source files or Git metadata;
- Rust toolchains under `.rustup`;
- Cargo tools/cache under `.cargo`;
- Visual Studio Build Tools or Windows SDKs.

The underlying original runners remain available for CI and low-level diagnostics. For interactive local Phase-1 work, use the cleanup-safe wrappers by default.
