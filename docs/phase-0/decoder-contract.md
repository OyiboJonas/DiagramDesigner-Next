# Phase 0 decoder contract

The legacy decoder is a compatibility boundary, not the DiagramDesigner Next domain model.

## Guarantees

- Legacy files are treated as untrusted input.
- Decompression, strings, blobs, bitmap allocation and group recursion are bounded.
- Binary strings are retained as raw bytes until an explicit charset stage.
- Version-dependent fields follow source-defined gates.
- Object boundaries are advanced only by source-defined codecs.
- All 12 currently defined legacy object type IDs have explicit codecs.
- Malformed/unknown type IDs or unsupported field values produce deterministic errors with offsets; the decoder never searches forward for a plausible next object boundary.
- Both DDD and DDT top-level formats use the same bounded object codec.
- Successful complete traversals report trailing bytes; the expected value for a known complete format variant is zero.
- Connector references remain raw during byte decoding and are materialized/validated in a separate graph pass.
- Raw legacy index pairs are retained alongside resolved importer identities for migration traceability.

## Validation baseline

- Private real-world DDD references are exercised through the external/private manifest boundary; their identifying metadata is intentionally not published.
- CI checks out the original upstream repository at a pinned commit and traverses all 30 public `.ddt` palettes through the compatibility path.
- The upstream palette corpus independently exercises BitmapObject, MetafileObject and CurveLine in addition to common shape/group/line types.
- `InheritedLayer` (type 12) is source-derived and covered by a synthetic payload-boundary test; a redistributable real DDD fixture remains desirable.

## Deliberate boundaries

- Charset/text normalization remains explicit because historical files may depend on legacy code pages/font charsets.
- The resolved importer graph is not the DDNX domain graph; permanent Next IDs are assigned only at the conversion boundary.
- Historical versions outside the current corpus may reveal additional version-specific behavior and remain diagnosable rather than silently coerced.
- Asset portability (especially EMF/WMF conversion) is separate from the bounded compatibility codec; raw legacy bytes are preserved first.
- Private fixture names, paths, source fingerprints, decoded contents and document-specific structural fingerprints do not belong in public documentation.

No unsupported or ambiguous data is silently dropped merely to produce a superficially successful import.
