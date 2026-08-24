from pathlib import Path

path = Path("crates/render-svg/src/image.rs")
text = path.read_text(encoding="utf-8")
text = text.replace(
    "use next_domain::{AssetId, AssetPayload, Document, Element, ElementId, ElementKind, Rect};",
    "use next_domain::{AssetId, AssetPayload, Document, Element, ElementKind, Rect};",
    1,
)
old = """    if let Some(alpha) = alpha
        && alpha.len() != pixel_count
    {
        return Err(RasterAssetIssue::InvalidAlphaLength {
            expected: pixel_count,
            actual: alpha.len(),
        });
    }
"""
new = """    if let Some(alpha) = alpha {
        if alpha.len() != pixel_count {
            return Err(RasterAssetIssue::InvalidAlphaLength {
                expected: pixel_count,
                actual: alpha.len(),
            });
        }
    }
"""
if old not in text:
    raise SystemExit("expected Rust 1.85 alpha let-chain not found")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
