# Phase 2 raster image compatibility contract

This delivery adds raster image rendering to the production SVG facade without moving image bytes or renderer-specific state into editor commands or render-plan items.

## Public compatibility basis

The compatibility reference is the pinned public Diagram Designer source commit `12188325704b559c211addf82f26183098b0e201`, primarily `Application/PictureObject.pas` (`TBitmapObject`) together with the public linear/fast-bitmap helpers used by that implementation.

The legacy contract relevant to this delivery is:

- serialized bitmap dimensions are positive integer width/height values;
- supported serialized pixel formats are 8-bit indexed and 24-bit colour;
- 8-bit images carry a 256-entry packed BGR palette;
- 24-bit linear bitmap pixels use Windows-native BGR byte order;
- an optional 8-bit alpha plane may accompany the image;
- `alpha_value` is a second, global alpha multiplier;
- the image is stretched to the object rectangle.

The existing migration/domain boundary already preserves these values in `AssetPayload::Raster`; `ElementKind::Image` retains only the stable `AssetId` reference. This delivery does not change that boundary or the deterministic migration asset identity.

## SVG materialization

Supported raster assets are converted to straight-alpha RGBA in top-to-bottom linear row order and embedded as deterministic PNG data URLs in SVG `<image>` elements.

Conversion rules:

- 8-bit palette entries are converted from BGR to RGB;
- 24-bit pixels are converted from BGR to RGB;
- missing per-pixel alpha is treated as 255;
- final alpha is `round(per_pixel_alpha * alpha_value / 255)`;
- the resulting PNG uses 8-bit RGBA, non-interlaced scanlines and PNG filter type 0;
- the `<image>` element uses `preserveAspectRatio="none"` so its pixels fill the element bounds like the legacy stretched draw path;
- existing element rotation is preserved by the shared SVG transform contract.

The PNG encoding is an implementation detail of the SVG facade. It is regenerated deterministically from the immutable raster asset bytes when the SVG is materialized; no PNG copy is introduced into document state. DDNX and `next-domain` continue to store the original raster representation rather than renderer output.

## Diagnostics and failure behaviour

Once image semantics are understood, generic `UnsupportedPrimitive` is retired for that element. Unsupported or malformed assets remain skipped with explicit typed diagnostics instead:

- `MissingAsset` when the referenced asset does not exist;
- `UnsupportedAssetPayload` when an image element points at a non-raster payload;
- `InvalidRasterAsset` with a typed `RasterAssetIssue` for invalid dimensions, unsupported bits-per-pixel values, missing/invalid palettes, wrong pixel/alpha lengths, overflow or PNG encoding failure.

Invalid element geometry remains owned by the Phase-1 core `InvalidGeometry` diagnostic.

## Verification contract

Public regression coverage must verify at least:

- 24-bit BGR channel conversion;
- 8-bit palette conversion;
- per-pixel plus global alpha composition;
- render-plan z-order with other facade/core primitives;
- typed missing/unsupported/malformed asset diagnostics;
- cold render-plan versus `PreparedPage` viewport equivalence;
- deterministic fidelity-scene materialization of a small synthetic raster image.

No private `.ddd` fixture or private-corpus metadata is required or published for this work.

## Deliberate boundary

Legacy `FHalftoneStretch` interpolation policy is not represented by the current `AssetPayload::Raster` contract and is therefore not silently invented here. This delivery covers raster pixel/alpha fidelity and object-rectangle placement. A future semantic interpolation setting, if required, must be introduced as renderer-independent domain state rather than inferred from SVG/browser-specific behaviour.

Metafiles remain a separate Phase-2 policy/rendering path and continue to be explicitly unsupported by this delivery.
