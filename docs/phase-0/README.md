# Phase 0 — Legacy Compatibility Lab

## Goal

Prove that DiagramDesigner Next can identify, decompress and progressively decode legacy `.ddd` and `.ddt` documents in a deterministic, testable and bounded way before editor UI development begins.

## Phase 0 invariants

1. Legacy files are untrusted input.
2. Parsing is read-only.
3. Parsing has explicit size limits.
4. No parser type may depend on editor UI or rendering code.
5. Unknown or unsupported data must produce diagnostics rather than silent loss.
6. Source-derived assumptions become compatibility guarantees only after fixture verification.

## Compatibility pipeline

```text
legacy bytes
  → bounded decoder
  → validated reference/text normalization
  → legacy-migrate
  → next-domain
  → DDNX persistence
```

## Decoder stages

1. Header + bounded DEFLATE
2. Bounded byte reader and string primitives
3. Container defaults and document metadata
4. Pages and layers
5. Object-list registry and 12 legacy object type IDs
6. Object payloads and version gates
7. Two-pass connector/reference resolution
8. Legacy text markup and codepage conversion
9. Bitmap/metafile extraction
10. Canonical migration diagnostics
11. Public/private fixture regression
12. Next-domain conversion and DDNX writer

## Fixture policy

Synthetic fixtures are useful for parser unit tests, but real legacy files are also valuable for compatibility evidence.

Target fixture categories include:

- minimal text;
- rectangle / ellipse / flowchart;
- straight line / connector / curve;
- linked connector with ports;
- polygon;
- group / nested group;
- bitmap / metafile;
- multi-page / multi-layer;
- inherited layer;
- template `.ddt`;
- non-ASCII text and historical file versions.

Redistributable public fixtures may be documented and pinned in the repository. Private/restricted fixtures must remain outside the public Git history, including their identifying names, local paths, source fingerprints and decoded contents. They are exercised through a local external manifest; see `private-corpus-verification.md`.
