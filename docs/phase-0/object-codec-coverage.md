# Legacy object codec coverage

Phase 0 reads legacy objects into an explicit intermediate model. A codec is considered complete only when its byte layout is derived from the legacy Pascal source, bounded by parser safety rules, covered by unit tests, and validated against a real or deliberately generated fixture.

| ID | Legacy object | Codec status | Validation |
|---:|---|---|---|
| 1 | TextObject | implemented | private DDD references + upstream DDT corpus |
| 2 | RectangleObject | implemented | private DDD references + upstream DDT corpus |
| 3 | EllipseObject | implemented | private DDD references + upstream DDT corpus |
| 4 | StraightLine | implemented | private DDD references + upstream DDT corpus |
| 5 | ConnectorLine | implemented | private DDD references + upstream DDT corpus |
| 6 | BitmapObject | implemented, bounded image allocation | upstream DDT corpus + synthetic boundary test |
| 7 | MetafileObject | implemented, bounded blob allocation | upstream DDT corpus + synthetic boundary test |
| 8 | GroupObject | implemented recursively | private DDD references + upstream DDT corpus |
| 9 | PolygonObject | implemented | private DDD references + upstream DDT corpus |
| 10 | FlowchartObject | implemented | private DDD references + upstream DDT corpus |
| 11 | CurveLine | implemented with pre-v28/v28 inheritance split | upstream DDT corpus + v26/v28 synthetic tests |
| 12 | InheritedLayer | implemented | source-derived synthetic boundary test; redistributable real DDD fixture still desirable |

Private fixture identities, fingerprints and document-specific counts are intentionally excluded from the public repository. They are exercised only through the generic external-corpus boundary.

## Full upstream palette regression

CI checks out the original `meesoft/DiagramDesigner` repository at pinned commit `12188325704b559c211addf82f26183098b0e201` and traverses **all 30 upstream `.ddt` palettes** with `dd-migrate inspect`.

All 30 palettes currently parse successfully with **zero trailing bytes**. This independently exercises:

- BitmapObject (type 6), including palettes such as `Dokumente.ddt`, `Electronic Display Devices.ddt`, `Electronic Symbols 3.ddt` and `GUI design 2.ddt`;
- MetafileObject (type 7), including the large Cisco topology palette and many AutoRealm/flowchart palettes;
- CurveLine (type 11), including `Electronic Symbols 1.ddt`, `Electronic Symbols 2.ddt`, `GeneralShapes.ddt` and `Genogram.ddt`.

Type 12 (`InheritedLayer`) does not occur in the pinned upstream DDT palette corpus. Its codec is source-derived and covered synthetically, but a redistributable real DDD example remains useful as an additional golden reference.

## Compatibility invariant

Unknown object type IDs remain an explicit parser error. The decoder never searches forward for a plausible next type byte and never guesses a payload length. Every one of the 12 currently defined type IDs has a source-defined codec; malformed or unsupported field values fail diagnostically.

## Reference resolution

Connector `ObjectIndex`/`LinkIndex` pairs are decoded raw during pass one. Pass two assigns deterministic importer-local object IDs, validates owner-list object/link indices, preserves the raw pair for traceability and materializes resolved endpoint records. The Next/DDNX model translates those importer IDs into stable document element/port IDs rather than importing legacy list positions as identity.
