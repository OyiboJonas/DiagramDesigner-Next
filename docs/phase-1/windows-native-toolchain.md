# Phase 1 — Windows native toolchain

DiagramDesigner Next uses the native Rust MSVC target for Windows desktop validation and ADR-019 target-hardware evidence. The required Visual Studio C++ component depends on the active `rustc` host architecture.

## Supported native Rust hosts

The Phase-1 Windows preflight currently accepts:

- `aarch64-pc-windows-msvc` — native Windows ARM64;
- `x86_64-pc-windows-msvc` — native Windows x64;
- `i686-pc-windows-msvc` — native Windows x86.

Other Windows Rust hosts are rejected for Phase-1 native evidence until they are explicitly supported and reviewed.

## Visual Studio Build Tools requirements

Install Visual Studio Build Tools 2022 and a current Windows SDK.

For **ARM64**, install the current:

```text
MSVC v143 - VS 2022 C++ ARM64/ARM64EC Build Tools (Latest)
```

The corresponding Visual Studio component ID is:

```text
Microsoft.VisualStudio.Component.VC.Tools.ARM64
```

For **x64/x86**, install the VS 2022 Desktop development with C++ workload including the current MSVC v143 x64/x86 Build Tools. The corresponding component ID is:

```text
Microsoft.VisualStudio.Component.VC.Tools.x86.x64
```

A current Windows 10 or Windows 11 SDK is also required.

## Preflight

Run:

```powershell
.\benchmarks\phase-1\check-windows-toolchain.ps1
```

The preflight:

1. requires Windows, Git, `rustc` and `cargo`;
2. reads the exact native Rust host from `rustc -vV`;
3. maps that host to the required Visual Studio C++ component;
4. uses `vswhere` to verify the matching component is installed;
5. confirms that MSVC `link.exe` exists below the selected Build Tools installation;
6. compiles the locked ADR-019 fidelity-scene example as a native linker/toolchain smoke test.

Use `-SkipCompileCheck` only when the caller intentionally needs component discovery without compiling the fidelity scene, for example in a narrow hosted-CI preflight.

## Full Phase-1 tooling validation

After the standalone preflight passes, run:

```powershell
.\benchmarks\phase-1\validate-tooling.ps1 -WindowsConfiguration
```

`-WindowsConfiguration` invokes the native toolchain preflight first so missing Rust or architecture-specific MSVC prerequisites fail early with an actionable message. It then validates the Phase-1 target-runner, fidelity and private-corpus tooling.

Passing either command is **not** representative target-hardware evidence and does not select the renderer. The actual Phase-1 renderer decision still requires the clean combined target run, physical 4K native WebView2 measurements and the manual ADR-019 fidelity review.
