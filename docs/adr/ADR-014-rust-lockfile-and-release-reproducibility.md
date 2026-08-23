# ADR-014 — Rust lockfile and release reproducibility policy

- Status: Accepted for Phase 0 / executable workspace
- Date: 2026-08-19
- Scope: Rust workspace, CI, future desktop builds and releases

## Context

DiagramDesigner Next is an application workspace, not a reusable Rust library published independently to crates.io. Its compatibility parser and DDNX codec are security-sensitive and are validated against pinned real corpora.

Leaving dependency resolution floating means the same source commit can compile against different transitive dependency versions over time. That weakens CI reproducibility and makes compatibility/security regressions harder to attribute.

## Decision

### Commit the workspace `Cargo.lock`

The repository commits the root `Cargo.lock` generated for the complete Rust workspace.

All application/CI/release builds use that lockfile. Dependency updates therefore arrive as explicit reviewed source changes rather than being selected implicitly by a later crates.io index state.

### CI uses locked resolution

Once the lockfile is committed, normal CI commands use `--locked` where Cargo dependency resolution is involved:

```text
cargo build --locked
cargo clippy --locked ...
cargo test --locked ...
```

A missing or stale lockfile must fail CI rather than regenerate silently.

### Updating dependencies

Dependency changes are intentional maintenance work. The update commit/PR contains both manifest changes and the resulting `Cargo.lock` diff.

Broad unattended `cargo update` is not part of a normal build. Security or compatibility updates are reviewed with the same corpus/format gates as other parser/persistence changes.

### Exact pins versus lockfile

The lockfile is the primary transitive-resolution pin for this application workspace.

A direct manifest dependency may still use an exact version when format determinism or a known compatibility boundary requires it. `zip = "=7.0.0"` is currently such an intentional constraint while the DDNX v1 byte-level package behavior is being frozen.

Exact pins should not be added merely to duplicate what `Cargo.lock` already guarantees.

### Toolchain

The workspace declares `rust-version = "1.85"`, and CI currently installs Rust 1.85.0 explicitly. Release automation must use an explicit Rust toolchain version rather than a moving `stable` toolchain.

A future toolchain bump is a reviewed change and must pass the full compatibility/persistence corpus.

### Release provenance

Future distributable desktop releases should record at minimum:

- Git commit SHA;
- Rust toolchain version;
- committed `Cargo.lock`;
- target triple;
- application/DDNX schema versions;
- build workflow identity.

Bit-for-bit reproducible installers are desirable but are a separate packaging milestone. This ADR first guarantees deterministic Rust dependency selection from a source commit.

## Consequences

### Positive

- a source commit has one reviewed Rust dependency graph;
- CI cannot silently drift because crates.io gained newer compatible releases;
- dependency-related parser/DDNX behavior changes are visible in review;
- future security audits/SBOM generation have a stable package graph.

### Cost

- dependency updates create lockfile diffs that must be reviewed;
- CI must fail rather than auto-heal a stale lockfile.

## Verification

The repository must:

1. commit the root `Cargo.lock`;
2. run Rust CI/build commands with `--locked`;
3. keep the pinned upstream DiagramDesigner corpus gate green after lockfile introduction;
4. regenerate the lockfile only as an explicit dependency-maintenance change.
