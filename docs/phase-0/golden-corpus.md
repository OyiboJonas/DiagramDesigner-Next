# Phase 0 golden corpus

This document records the **public** compatibility fixtures and the publication policy for private legacy references.

## Private legacy references

Private/company `.ddd` documents are used locally to strengthen compatibility testing, but identifying metadata is intentionally excluded from the public repository.

The public repository must not contain, unless explicitly approved for publication:

- private document names or local paths;
- source-file SHA-256 fingerprints;
- decoded private text;
- company-specific page names or labels;
- document-specific object/reference counts that could fingerprint a private document;
- generated private Next JSON fingerprints or review manifests.

Private fixtures are replayed through the generic `dd-migrate verify-corpus` path using a local manifest outside Git. See `private-corpus-verification.md`.

Private evidence may establish that multiple independently produced legacy documents traverse the bounded decoder, reference resolver, text normalizer and Next-domain conversion successfully. Public documentation records only the resulting **capability boundary**, not identifying fixture details.

## Public upstream DDT reference

Source: `meesoft/DiagramDesigner`, `TemplatePalettes/AV_Flowchart.ddt`, pinned to upstream commit:

```text
12188325704b559c211addf82f26183098b0e201
```

The fixture confirms the source-defined `TTemplateSheet` top-level layout: `DDt` header + file version + raw DEFLATE payload containing width, height and the inherited `TBaseObjectList` stream.

## Full pinned upstream DDT regression

CI checks out the original `meesoft/DiagramDesigner` repository at the pinned commit and executes the compatibility path against **all 30 public `.ddt` files** in `TemplatePalettes`.

The public corpus exercises the ordinary shape families plus bitmap, metafile, nested/grouped and curve-heavy palettes. Type 12 (`InheritedLayer`) is source-derived and synthetically boundary-tested but does not occur in the pinned public DDT corpus; a redistributable real fixture remains desirable.

The public compatibility path is:

```text
legacy bytes
  → bounded inspect/decode
  → text normalization
  → Next conversion/validation
  → DDNX write
  → DDNX integrity read
  → Next hydration
```

The pinned public text-normalization regression contract is:

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

These values are derived solely from the public upstream palette corpus.

## Confirmed public compatibility assumptions

The public upstream corpus and source-derived tests support the following format rules:

1. DDD/DDT signatures and version fields are decoded explicitly.
2. Legacy payloads use bounded raw-DEFLATE decoding.
3. Legacy strings remain byte-oriented until an explicit charset decision is made.
4. DDD pages/layers and DDT template object lists use source-defined ordering.
5. Nested groups use the same bounded object-list codec recursively.
6. Connector object/link indices are retained raw during decoding and resolved in a second pass.
7. Unknown type IDs or malformed values fail diagnostically rather than being guessed.
8. All 12 currently defined legacy object type IDs have source-derived codecs; types not present in the public corpus retain synthetic boundary coverage.

## Future corpus work

- obtain redistributable real DDD fixtures for non-empty Stencil/master-layer behavior and type-12 `InheritedLayer`;
- add trustworthy redistributable non-Western text fixtures;
- add additional historical versions when redistributable fixtures are available;
- keep all non-public fixture identities and fingerprints in external/private manifests only.
