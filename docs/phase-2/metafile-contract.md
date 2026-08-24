# Phase 2 metafile contract

## Scope

DiagramDesigner Next preserves legacy `TMetafileObject` payloads without pretending that preservation alone makes WMF/EMF renderable in the production SVG/WebView surface.

The pinned public upstream reference is `Application/PictureObject.pas` from `meesoft/DiagramDesigner` at commit `12188325704b559c211addf82f26183098b0e201`.

## Public upstream behaviour

The legacy `TMetafileObject` contract is:

- the metafile is persisted through `TMetafile.SaveToStream` as a length-prefixed binary payload;
- the serialized bytes are restored through `TMetafile.LoadFromStream`;
- the object angle is serialized separately as a 32-bit `Single` for file version 4 and later;
- drawing stretches the metafile to the complete object rectangle;
- non-zero rotation applies a world transform around the centre of that object rectangle before the stretch draw.

The legacy object therefore owns two distinct concerns: a Windows metafile payload and document-space placement/rotation semantics.

## Next-domain policy

The importer continues to preserve the original metafile bytes as a stable hash-addressed `AssetPayload::Binary` with media type:

`application/vnd.diagramdesigner-next.windows-metafile`

`ElementKind::Metafile` references that stable `AssetId`. No SVG, PNG, Windows GDI handle, temporary conversion path or platform cache is stored in `next-domain` or editor history.

This is intentional: binary preservation is lossless persistence, not a WMF/EMF rendering claim.

## Renderer policy

The normal `render_plan_to_svg` path keeps a metafile as the existing typed `UnsupportedPrimitive(Metafile)` diagnostic when no verified browser-renderable rendition exists.

A platform layer may instead call `render_plan_to_svg_with_metafile_renditions` and provide disposable renditions keyed by the source `AssetId`. The SVG renderer accepts only browser-renderable rendition media types:

- `image/png`
- `image/jpeg`
- `image/webp`
- `image/svg+xml`

A valid rendition is embedded as an SVG `<image>` element. It uses `preserveAspectRatio="none"` to match the upstream stretch-to-object-rectangle behaviour and rotates around the normalized rectangle centre using the element's document rotation.

The rendition is renderer input only. It does not replace or mutate the original binary asset.

## Fallback and validation

- missing source asset -> `MissingAsset`;
- wrong media type, non-binary source payload or empty binary source -> `UnsupportedAssetPayload`;
- valid preserved source with no usable rendition -> retain `UnsupportedPrimitive(Metafile)`;
- unsupported or empty rendition -> retain `UnsupportedPrimitive(Metafile)`;
- valid browser rendition -> materialize the element and retire its unsupported-primitive diagnostic.

This policy prevents a malformed or merely preserved payload from being counted as rendered compatibility.

## Render-plan invariants

Metafile renditions are inserted in render-plan order. Cold `build_page_plan` and `PreparedPage` queries therefore share the same culling decision and produce identical SVG for the same viewport and rendition map.

## Compatibility boundary

This checkpoint establishes the policy and renderer boundary needed for platform conversion. It does **not** claim that DiagramDesigner Next can decode arbitrary WMF or EMF payloads on every platform. Such a claim requires an actual conversion implementation plus format-specific evidence and tests.
