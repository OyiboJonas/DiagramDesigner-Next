# Legacy text normalization

Diagram Designer 1.x is a Delphi 7 application. Its serialized strings are byte strings, not intrinsically UTF-8. Phase 0 therefore keeps byte decoding and rich-text interpretation as explicit migration stages.

## Pipeline

```text
legacy string bytes
        ↓
charset decision
        ↓
Unicode text + decode diagnostics
        ↓
legacy backslash-markup parser
        ↓
renderer-independent RichTextDocument
        ↓
legacy-migrate
        ↓
next-domain RichTextDocument
```

The original raw bytes remain attached to normalized import entries for traceability. Normal editor/rendering state uses the typed Next representation rather than Delphi markup.

## Charset decision

For DDD documents the stored `DefaultFontCharSet` is used when it maps to a concrete Windows/VCL charset. `DEFAULT_CHARSET`, Symbol/OEM and unknown charset values do not silently become UTF-8; they use a caller-selected fallback and record that decision.

DDT has no top-level charset in its source-defined format. Its ordinary strings therefore use an explicit caller-selected fallback.

`dd-migrate text` defaults to `windows-1252` because the validated German corpus and the legacy Delphi project configuration are Western-European, but the fallback is an explicit CLI option rather than a hidden parser assumption.

Supported decoder choices currently cover Windows-1250 through Windows-1258, Windows-874, Shift-JIS, EUC-KR, GBK and Big5.

## Markup model

The parser follows the legacy `TTextObject.Draw.ParseText` command language and converts commands into typed tokens/styles rather than preserving rendering commands as editor text.

Covered commands include:

- bold, italic, underline and strikeout toggles;
- subscript/superscript and normal baseline;
- overline;
- Symbol-font mode;
- font family, font size and RGB text color;
- explicit line breaks;
- page number, page count and page name semantic fields;
- legacy special-character escapes;
- unformatted `\@...\@` sections;
- `\A` action and `\N` hint tails.

External action/hint tails are preserved as inert typed data. The text parser does not execute them or classify them as trusted paths/URLs.

## Symbol-font boundary

The original `TextObject.pas` has two distinct Symbol-font mechanisms and they are intentionally treated differently.

### Source-defined `WriteSymbol(...)` escapes

Eight backslash escapes call `WriteSymbol(...)` with fixed legacy Symbol Encoding code points. The source literals and the Unicode Consortium's Adobe Symbol Encoding mapping make these meanings unambiguous:

| Escape | Symbol code | Portable Unicode |
|---|---:|---|
| `\#` | A8 | `♦` BLACK DIAMOND SUIT |
| `\=` | B9 | `≠` NOT EQUAL TO |
| `\~` | BB | `≈` ALMOST EQUAL TO |
| `\>` | B3 | `≥` GREATER-THAN OR EQUAL TO |
| `\<` | A3 | `≤` LESS-THAN OR EQUAL TO |
| `\/` | D6 | `√` SQUARE ROOT |
| `\-` | B8 | `÷` DIVISION SIGN |
| `\§` | A5 | `∞` INFINITY |

These eight are converted to ordinary portable Unicode text and no longer carry Symbol-font rendering semantics.

### Free `\S ... \s` runs

`\S` changes the active legacy renderer font to Windows Symbol until `\s`. Arbitrary characters inside such a run are **not** assumed to mean their Unicode code points. They are emitted as `SymbolGlyph { legacy_glyph, ... }` tokens and remain explicit unresolved glyph identities until a mapping is proven for the actual source glyph.

This distinction is deliberate. By the time markup is parsed, ordinary legacy text has already passed through the selected ANSI decoder. Reverse-interpreting a decoded character as a Symbol-Encoding byte would therefore introduce a hidden code-page assumption into a generic parser. Phase 0 preserves the unresolved glyph instead of guessing.

The pinned public corpus proves that this case is real rather than theoretical: 11 free Symbol-font glyphs occur across three upstream palettes. Their presence is now part of the regression contract. A future byte-aware Symbol normalization stage may resolve individual glyphs only when the source-byte identity and mapping are explicit.

## Stable paths

Normalization walks DDD pages/layers/stencil and nested groups with the same deterministic importer path convention as reference resolution, for example:

```text
page/0/name
page/0/layer/0/object/17/name
page/0/layer/0/object/17/text
page/0/layer/0/object/23/group/object/2/text
stencil/object/0/text
```

This lets migration diagnostics correlate decoded text, raw bytes, object identity and resolved relationships before permanent Next IDs are assigned.

## Central text audit summary

`TextNormalizationSummary` is the shared audit surface used by the CLI, CI and private corpus verifier. It reports:

- total normalized entries;
- object-text entries;
- entries with decode errors;
- markup diagnostics;
- unresolved Symbol glyph count;
- inert action-tail count;
- inert hint-tail count.

The counts are derived from the same normalized token graph that is fed into `legacy-migrate`; there is no parallel text parser for testing.

## Pinned public corpus

CI pins `meesoft/DiagramDesigner` at commit `12188325704b559c211addf82f26183098b0e201` and text-normalizes every one of the 30 upstream DDT palettes before Next conversion and DDNX round-trip.

The reviewed baseline after correcting free Symbol-font semantics is:

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

Because both the upstream commit and this repository revision are controlled, CI treats these aggregate values as a semantic regression contract. A change requires explicit review rather than silently updating the expected counts.

The 11 unresolved glyphs occur in `Electronic Symbols 1.ddt` (2), `Electronic Symbols 2.ddt` (2) and `web wireframe.ddt` (7). CI intentionally pins the count rather than embedding or guessing their rendered text meaning. Dedicated source-derived unit tests separately cover the eight portable `WriteSymbol(...)` mappings and preservation of arbitrary `\S ... \s` glyphs.

## Private / external corpus

`dd-migrate verify-corpus` always reports the same text summary for each external fixture. A corpus manifest may optionally contain a reviewed `text` expectation. When present, any count mismatch fails the fixture before its Next fingerprint is accepted.

This allows private/company DDD files to provide deterministic text regression coverage without committing the binaries to GitHub. See `docs/phase-0/private-corpus-verification.md`.

## Remaining boundary

The core text path is now integrated into `next-domain`. Remaining hardening is corpus-driven rather than architectural:

- replay the private reference DDD files through the external verifier and pin reviewed text summaries/Next hashes;
- add non-Western real documents when trustworthy fixtures become available;
- extend Symbol-font Unicode mappings only when source-byte identity and a specific legacy glyph meaning are proven.

No unknown Symbol glyph or undecodable byte sequence is silently guessed.
