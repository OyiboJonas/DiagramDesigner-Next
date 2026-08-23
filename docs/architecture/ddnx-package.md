# DDNX native package contract

Status: pre-v1 architecture contract for Issue #8.

## Purpose

`.ddnx` is the native editable package format for DiagramDesigner Next. Legacy `.ddd` and `.ddt` files remain import formats and must never become the storage model for the new editor.

The dependency direction is:

```text
DDD / DDT
  ↓
legacy-ddd
  ↓
legacy-migrate
  ↓
next-domain
  ↓
ddnx
  ↓
filesystem / platform adapter
```

The `ddnx` crate knows the renderer-independent Next domain, but it does not know Delphi/VCL object IDs, legacy list indices, editor tools or platform UI objects.

## Package layout

DDNX v1 is a ZIP container with a deliberately small fixed core:

```text
manifest.json
document.json
assets/<content-sha256>.<ext>
previews/<name>                 # optional, manifest-declared derived data
```

`import/legacy.json` and embedded original DDD/DDT source bytes are deliberately **not** part of DDNX v1. Migration provenance stays in typed Next import metadata. ADR-016 defines this boundary.

ZIP is only the transport/container. The normal document model is `next-domain`.

## Version boundaries

Three versions are intentionally separate:

- `PACKAGE_VERSION = 1` — ZIP/package contract;
- `DOCUMENT_VERSION = 1` — the DDNX-specific JSON projection;
- `next-domain::SCHEMA_VERSION = 1` — renderer-independent domain schema.

The manifest binds all three.

Current pre-v1 readers accept only the exact current version triple and reject both older and newer values. This is intentional, not an absence of a compatibility strategy. ADR-013 defines future backward compatibility as explicit monotonic version migrators; unknown newer files are never best-effort deserialized into current structs, and opening an older supported file migrates it in memory without rewriting the source until the user saves.

Dedicated integration tests cover current-version acceptance and older/newer rejection for all three version axes.

## `manifest.json`

The manifest is readable before document hydration and contains:

- DDNX magic/version information;
- artifact kind (`document` or `template_palette`);
- fixed `document.json` path;
- SHA-256 and byte length of `document.json`;
- asset records;
- optional preview records.

Each asset record contains:

```text
AssetId
content_sha256
blob_sha256
media_type
bytes
path
```

### Two hashes, two meanings

`content_sha256` is semantic/content identity owned by the Next asset model and is used for the content-addressed package path.

`blob_sha256` is integrity identity for the exact external byte sequence stored in the ZIP. Raster semantic metadata also lives in `document.json`, so the two hashes are not required to match.

## `document.json`

`document.json` is a DDNX persistence projection of `next-domain`.

Binary asset payloads are externalized: asset descriptors remain in JSON while bytes live exactly once under `assets/`.

Logical domain order is preserved in `document.json`, including asset order. Physical ZIP entry order is independent of document semantics.

### Master layers

`Document.master_layers` are persisted and hydrated as ordinary typed Next document state. DDNX has no `Stencil` field and no legacy-specific background-layer representation.

The legacy DDD container `Stencil` is consumed only by `legacy-migrate`, which maps its global-background semantics into `Document.master_layers` before DDNX sees the artifact. This keeps the native format independent of the source format.

An empty master-layer collection is valid and may be represented explicitly as `master_layers: []` in the DDNX projection.

### Raster assets

A raster descriptor stores:

- width / height;
- bits per pixel;
- alpha value;
- palette presence and byte length;
- pixel byte length;
- alpha presence and byte length.

The explicit presence flags preserve the difference between `None` and `Some([])`. A reader rejects contradictory combinations such as `palette_present = false` with non-zero palette length.

## Preview policy

Previews are optional derived data only. They are not authoritative document state and are never required to open/hydrate a valid DDNX document.

If present, previews are manifest-declared and integrity-checked like other package data. The editor/domain must not reconstruct geometry, text, styles, relationships or assets from them. They may be removed and regenerated from the authoritative document.

The core `ddnx` crate does not render previews, and the initial writer may emit none until the renderer/desktop layer defines an encoding/size production policy. ADR-016 records the full rule.

## Legacy import-audit policy

DDNX v1 does not duplicate the legacy source graph into a package sidecar. Artifact-level `ImportMetadata` and element-level `ElementImportMetadata` already carry the typed provenance/diagnostic fields needed by migration tooling.

Original `.ddd` / `.ddt` bytes are not embedded by default. `source_sha256` provides provenance without silently retaining potentially sensitive, large or duplicate source data. A future deep-audit sidecar would require its own explicit versioned schema, manifest declaration, security/size/privacy policy and must remain non-authoritative. See ADR-016.

## Deterministic writer

DDNX v1 writes only ZIP `Stored` entries. Compression is intentionally disabled at this stage so output does not depend on compressor implementation/version details.

The writer uses deterministic timestamps, fixed Unix file permissions, fixed core entry order, asset entries sorted by package path and no duplicate package paths.

The same prepared package therefore produces identical ZIP bytes in the current implementation.

The migration CLI performs an in-memory read/hydrate verification before generated bytes are committed to disk.

## Persistence equivalence

Rust `f64` → JSON text → Rust `f64` can differ by one IEEE-754 ULP for some finite decimal representations even though no meaningful geometry changed. This was observed in the real upstream palette corpus.

DDNX therefore defines one centralized persistence comparison:

- objects, arrays, keys, booleans, strings, IDs and integer numbers are strict;
- finite non-integer JSON numbers may differ by at most **1 ULP**;
- 2 ULP or more is an error;
- structural differences are always errors;
- string mismatch diagnostics report paths and lengths but suppress contents.

This is a persistence-boundary rule, not a general editor geometry tolerance.

## Reader safety model

DDNX files are untrusted input. The reader never extracts archive entries to the filesystem.

Before hydration it enforces:

- maximum ZIP entry count;
- maximum manifest size;
- maximum document size;
- maximum per-asset size;
- maximum total uncompressed size;
- normalized relative package paths only;
- no absolute paths, `..`, `.`, empty components, Windows drive paths, backslashes or NUL;
- no duplicate logical ZIP names;
- no overlapping ZIP file payloads;
- no encrypted entries;
- no directory/symlink entries;
- `Stored` compression only for DDNX v1;
- no undeclared entries;
- all required manifest-declared entries must exist;
- declared sizes must match actual sizes;
- `document.json` SHA-256 must match the manifest;
- asset and preview SHA-256 values must match the manifest;
- hydrated asset descriptor metadata must match verified blobs;
- reconstructed semantic asset hashes must match `content_sha256`;
- the resulting `NextArtifact` must pass normal domain validation, including document-global identity rules.

The adversarial suite covers malformed manifest JSON, traversal paths, per-entry oversize, total uncompressed oversize and entry-count limits in addition to writer/reader round-trip and hash-corruption tests.

Duplicate-entry defense is tested at two independent levels: the production Rust writer refuses duplicate names, and a fixed foreign ZIP byte fixture generated outside the Rust `zip` crate contains two `manifest.json` entries and is rejected by the DDNX reader with `DuplicateEntry`.

## Native write boundary

`dd-migrate convert <legacy-file> --output <file.ddnx>` currently:

1. converts legacy input to a validated `NextArtifact`;
2. prepares DDNX JSON and external asset blobs;
3. writes deterministic ZIP bytes in memory;
4. reads, integrity-checks and hydrates those exact bytes again;
5. verifies DDNX persistence equivalence;
6. creates a unique sibling temporary file;
7. writes and `sync_all()`s the temporary file;
8. renames it to the requested destination.

The CLI deliberately refuses to overwrite an existing destination and is not the future editor Save implementation.

ADR-015 defines the application save boundary: DDNX remains byte-oriented; a global filesystem/platform adapter writes a same-directory temporary file, syncs it, then commits it with the platform's atomic rename/replace primitive. Existing documents must never be replaced via delete-then-rename. Platform-specific replacement and parent-directory durability implementation remains work for the desktop/platform layer.

## Dependency and toolchain reproducibility

The Rust application workspace commits its root `Cargo.lock`. CI/build commands use `--locked`, so a source commit cannot silently resolve a different transitive dependency graph at a later date.

The lockfile was bootstrapped from the same Rust 1.85.0 CI workspace used by the compatibility suite, then committed and made mandatory. Dependency updates require an explicit reviewed lockfile diff. ADR-014 records the full policy.

The direct `zip = "=7.0.0"` constraint remains intentional while DDNX v1 byte-level package behavior is frozen; the committed lockfile pins the rest of the graph.

## Current compatibility gate

CI pins the original `meesoft/DiagramDesigner` repository to:

```text
12188325704b559c211addf82f26183098b0e201
```

Every upstream `.ddt` palette is run through:

```text
legacy bytes
  → inspect
  → text normalization
  → Next conversion
  → DDNX write
  → DDNX integrity read
  → Next hydration
```

The reviewed public text fingerprint is:

```text
palettes=30
entries=4031
object-text=453
decode-errors=0
markup-diagnostics=0
symbol-glyphs=11
action-tails=2
hint-tails=0
```

The established compatibility checkpoint is 30/30 original palettes passing the complete persistence path, with Format, Clippy and workspace tests green. The format-freeze branch additionally requires the committed lockfile through Cargo `--locked`, strict version-policy tests and the foreign duplicate-name ZIP regression.

Master-layer behavior has dedicated synthetic legacy-conversion and DDNX document round-trip tests because `.ddt` palettes do not contain DDD container Stencil state.

## Format-freeze decisions already made

- package/document/domain versions are separate and future backward compatibility uses explicit migrations — ADR-013;
- the executable Rust workspace commits `Cargo.lock` and CI uses locked dependency resolution — ADR-014;
- atomic overwrite belongs in a platform filesystem adapter; delete-then-rename is forbidden — ADR-015;
- previews are optional derived non-authoritative data — ADR-016;
- DDNX v1 has no `import/legacy.json` and does not embed original DDD/DDT bytes by default — ADR-016;
- foreign duplicate logical ZIP names are covered by an independent byte fixture;
- legacy DDD Stencil is not a DDNX concept; it becomes native `master_layers` before packaging.

## Deliberate remaining work

Before DDNX is frozen as a public v1 format, the remaining implementation/evidence items are:

- implement and platform-test ADR-015 when the desktop filesystem adapter exists;
- define the renderer-side preview **production** convention only when a renderer exists (preview absence remains valid v1 state);
- validate a real non-empty legacy DDD Stencil through `Stencil → master_layers → DDNX → master_layers` when such a fixture is available.

These are global format/platform decisions or external evidence. They must not be implemented as page-, renderer- or importer-specific exceptions.
