# ADR-016 — DDNX preview and import-audit policy

- Status: Accepted for DDNX v1 format freeze
- Date: 2026-08-19
- Scope: DDNX package contents, renderer integration, legacy migration metadata

## Context

The DDNX manifest already allows optional `previews/...` entries, and early package design reserved the possibility of an `import/legacy.json` audit sidecar.

Without a clear policy, derived previews can accidentally become required document state, while a legacy sidecar can duplicate import metadata, inflate files, leak source content and pull legacy-specific concepts back into the native format.

## Decision

## Preview entries

Previews are optional **derived cache/artifact data**. They are never authoritative document state.

Rules for DDNX v1:

- a valid document/package must open and hydrate completely with zero preview entries;
- previews may be omitted from normal saves;
- if present, every preview is explicitly declared by the manifest and covered by byte length and SHA-256 integrity checks;
- preview corruption causes package verification failure while the entry is present; readers do not silently consume unverified preview bytes;
- no editor/domain behavior, geometry, style, text, relationship or asset identity may be reconstructed from a preview;
- previews may be dropped and regenerated from the authoritative current document by a renderer;
- the core `ddnx` crate does not render previews;
- the initial v1 writer may continue to emit no previews until a renderer/export pipeline defines the actual production encoding/size policy.

A future preview encoding convention can be added without changing domain semantics. If it changes package-level requirements, that change follows ADR-013 versioning rules.

## Legacy import audit sidecar

DDNX v1 does **not** define or emit `import/legacy.json`.

The authoritative migration trace required by current compatibility work already exists in typed Next state:

- artifact-level `ImportMetadata` records source format/version/hash/importer and diagnostics;
- `ElementImportMetadata` records source path/type ID/anchor bits and selected raw values required for migration auditing.

This typed metadata is sufficient for normal provenance, diagnostics and repair-tool correlation without adding a second native representation of legacy state.

### Original legacy bytes

Original `.ddd` / `.ddt` source bytes are not embedded into DDNX by default.

Reasons:

- potentially sensitive/company source data should not be copied invisibly into every native file;
- original files may contain large bitmap/metafile payloads already represented as Next assets;
- bundling the full source creates duplicate storage and a second attack surface;
- retention/licensing/privacy policy for original files belongs to the user's source-file workflow, not the native package format.

`ImportMetadata.source_sha256` provides stable provenance without embedding the source.

### Future deep-audit archive

If a future repair/compliance workflow genuinely requires a richer legacy audit sidecar or embedded original source, it must be an explicit optional feature with:

- a versioned schema;
- manifest declaration and integrity hashes;
- explicit user/application policy;
- size/security limits;
- privacy/export behavior;
- no effect on normal domain hydration.

It must not be introduced as an undocumented extra ZIP entry because the DDNX reader deliberately rejects undeclared entries.

## Consequences

### Positive

- `document.json` + assets remain the authoritative native document;
- previews can never become a hidden persistence dependency;
- native files do not silently retain whole legacy source documents;
- import provenance remains typed and queryable rather than duplicated in a parallel JSON format;
- future richer audit storage requires an explicit reviewed format decision.

### Cost

- initial files may have no instant thumbnail until the renderer/desktop layer implements preview generation;
- deep forensic reconstruction may still require retaining the original legacy file externally.

## DDNX v1 freeze result

For v1, the package core is therefore:

```text
manifest.json             required
document.json             required
assets/...                optional, manifest-declared
previews/...              optional derived data, manifest-declared
import/legacy.json        not part of v1
embedded original DDD/DDT not part of v1
```
