# Phase 0 status — Legacy Compatibility Lab

Updated: 2026-08-23

This file is a concise engineering checkpoint. Detailed format/domain decisions live in the ADRs and architecture documents.

## Implemented boundaries

```text
DDD / DDT bytes
  ↓ legacy-ddd
bounded source-defined legacy graph
  ↓ reference + text normalization
validated legacy intermediate state
  ↓ legacy-migrate
next-domain
  ↓ ddnx
native DDNX package
```

No future editor or renderer code should bypass these boundaries.

## Legacy decoding

- DDD and DDT packed header/version detection;
- raw DEFLATE with output bound;
- bounded primitive/string/blob readers;
- explicit version gates;
- all 12 current source-defined object type IDs;
- recursive groups;
- bitmap/metafile/curve payloads;
- second-pass connector object/link resolution;
- legacy charset decoding;
- Unicode rich-text tokenization with inert action/hint tails;
- source-defined fixed Symbol escapes mapped to portable Unicode;
- arbitrary free `\S ... \s` Symbol-font runs preserved as explicit unresolved glyph tokens;
- central text audit metrics reused by CI and private corpus verification.

## Next domain

- typed stable UUID identities;
- metric geometry;
- pages and page-local layers;
- document-global `master_layers` rendered before page-local layers;
- document-wide `LayerId`, `ElementId`, `PortId`, `StyleId` and `AssetId` uniqueness;
- flat scenes/groups by stable IDs;
- stable ports and connector references;
- typed styles, gradients, markers, routes and text layout;
- Unicode/RichText with unresolved legacy Symbol glyphs retained explicitly rather than guessed;
- content-addressed assets;
- structural validation.

Legacy DDD `Stencil` is source-verified as global page background content and maps to `Document.master_layers`. It is not treated as a DDT/template library.

Element/port identity is independent of layer storage location. Moving an element between layers must not mint a new identity. Group and connector relationship scope remains scene-local until explicitly broadened by a future domain decision.

## DDNX

- versioned manifest/document projection;
- deterministic Stored ZIP writer;
- bounded in-memory reader;
- strict package paths;
- entry/document/asset/total size limits;
- overlapping/encrypted/directory/symlink/unsupported-compression rejection;
- manifest-declared entry set;
- document/blob integrity hashes;
- externalized assets;
- raster presence/length semantics;
- hydration to validated `NextArtifact`;
- native persistence of `master_layers` without a legacy Stencil field;
- strict persistence comparison with maximum 1 ULP allowance only for finite JSON floating values;
- `dd-migrate convert --output <file.ddnx>` and `verify-ddnx`.

## Compatibility checkpoints

### Private DDD references

Private real-world DDD references are exercised locally through the external `verify-corpus` boundary. Their binaries, names, local paths, source fingerprints, decoded contents and document-specific structural observations are intentionally excluded from the public repository.

The private path exists to catch real-world migration regressions without turning confidential fixtures into public project metadata. CI validates only the generic harness configuration and never reads private files.

### Pinned public DDT corpus

Source: `meesoft/DiagramDesigner` pinned to commit:

```text
12188325704b559c211addf82f26183098b0e201
```

Current public compatibility gate:

- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo test --workspace`;
- 30/30 original palettes parse to exact EOF;
- 30/30 convert to validated Next artifacts;
- 30/30 write/read/hydrate through DDNX;
- bitmap, metafile, group and curve-heavy palettes included.

The reviewed public text-normalization fingerprint is:

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

The 11 unresolved free Symbol-font glyphs are intentionally retained rather than reverse-guessed from already-decoded text. Fixed source-defined `WriteSymbol(...)` escapes are handled separately and map to portable Unicode.

Master-layer-specific behavior additionally has synthetic legacy conversion and DDNX document round-trip coverage because DDT does not contain DDD container Stencil state.

## Open hardening

1. obtain a redistributable real DDD with non-empty global Stencil and replay `Stencil → master_layers → DDNX → master_layers`;
2. continue private real-world corpus replay through external manifests without publishing fixture identities;
3. obtain a redistributable real type-12 `InheritedLayer` fixture before promoting relative indices to stable references;
4. add trustworthy redistributable non-Western text fixtures; resolve additional Symbol glyphs only with proven source-byte semantics;
5. continue DDNX format/release hardening without coupling editor code back to legacy storage.

## Phase transition rule

Do not make the legacy format model the editor model. Editor/UI work consumes `next-domain`; legacy-specific repair/import behavior stays behind `legacy-migrate`.
