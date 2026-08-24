use std::io::Write as _;

use flate2::{Compression, write::ZlibEncoder};
use next_domain::{AssetId, AssetPayload, Document, Element, ElementKind, Rect};
use render_plan::RenderPlan;

use super::{RasterAssetIssue, SvgDiagnostic, SvgRenderOutput, num, rotation_attribute};

pub(super) fn apply_images(
    document: &Document,
    plan: &RenderPlan<'_>,
    output: &mut SvgRenderOutput,
) {
    let mut retired = Vec::new();
    let mut rendered = Vec::new();

    // The Phase-1 core skips images. Work backwards so inserting before the
    // nearest later materialized element preserves render-plan z-order, including
    // relative ordering with other Phase-2 facade primitives such as polygons.
    for index in (0..plan.items.len()).rev() {
        let item = &plan.items[index];
        let ElementKind::Image { asset_id } = &item.element.kind else {
            continue;
        };
        let asset_id = *asset_id;

        // The core already owns invalid element-geometry diagnostics and does not
        // emit UnsupportedPrimitive for those elements, so leave them untouched.
        if !element_geometry_is_finite(item.element) {
            continue;
        }

        let Some(asset) = document.assets.iter().find(|asset| asset.id == asset_id) else {
            push_diagnostic_once(
                output,
                SvgDiagnostic::MissingAsset {
                    element_id: item.element.id,
                    asset_id,
                },
            );
            retired.push(item.element.id);
            continue;
        };

        let fragment = match &asset.payload {
            AssetPayload::Raster {
                width,
                height,
                bits_per_pixel,
                palette,
                pixels,
                alpha,
                alpha_value,
            } => match render_raster_image(
                item.element,
                asset.id,
                *width,
                *height,
                *bits_per_pixel,
                palette.as_deref(),
                pixels,
                alpha.as_deref(),
                *alpha_value,
            ) {
                Ok(fragment) => fragment,
                Err(issue) => {
                    push_diagnostic_once(
                        output,
                        SvgDiagnostic::InvalidRasterAsset {
                            element_id: item.element.id,
                            asset_id,
                            issue,
                        },
                    );
                    retired.push(item.element.id);
                    continue;
                }
            },
            AssetPayload::Binary { .. } => {
                push_diagnostic_once(
                    output,
                    SvgDiagnostic::UnsupportedAssetPayload {
                        element_id: item.element.id,
                        asset_id,
                    },
                );
                retired.push(item.element.id);
                continue;
            }
        };

        if inject_fragment_in_plan_order(&mut output.svg, plan, index, &fragment) {
            retired.push(item.element.id);
            rendered.push(item.element.id);
        }
    }

    if retired.is_empty() {
        return;
    }

    output.diagnostics.retain(|diagnostic| {
        !matches!(
            diagnostic,
            SvgDiagnostic::UnsupportedPrimitive { element_id, .. }
                if retired.contains(element_id)
        )
    });
    output.rendered_elements += rendered.len();
    output.skipped_elements = output.skipped_elements.saturating_sub(rendered.len());
}

#[allow(clippy::too_many_arguments)]
fn render_raster_image(
    element: &Element,
    asset_id: AssetId,
    width: i32,
    height: i32,
    bits_per_pixel: u8,
    palette: Option<&[u8]>,
    pixels: &[u8],
    alpha: Option<&[u8]>,
    alpha_value: u8,
) -> Result<String, RasterAssetIssue> {
    let rgba = raster_to_rgba(
        width,
        height,
        bits_per_pixel,
        palette,
        pixels,
        alpha,
        alpha_value,
    )?;
    let png = encode_rgba_png(width, height, &rgba)?;
    let encoded = base64_encode(&png);
    let bounds = normalize_rect(element.bounds_mm);
    let mut fragment = format!(
        "<image data-element-id=\"{}\" data-ddn-asset-id=\"{}\" data-ddn-raster-bpp=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"data:image/png;base64,{}\"",
        element.id.0,
        asset_id.0,
        bits_per_pixel,
        num(bounds.x),
        num(bounds.y),
        num(bounds.width),
        num(bounds.height),
        encoded,
    );
    fragment.push_str(&rotation_attribute(element));
    fragment.push_str("/>");
    Ok(fragment)
}

#[allow(clippy::too_many_arguments)]
fn raster_to_rgba(
    width: i32,
    height: i32,
    bits_per_pixel: u8,
    palette: Option<&[u8]>,
    pixels: &[u8],
    alpha: Option<&[u8]>,
    alpha_value: u8,
) -> Result<Vec<u8>, RasterAssetIssue> {
    if width <= 0 || height <= 0 {
        return Err(RasterAssetIssue::InvalidDimensions);
    }
    let width = usize::try_from(width).map_err(|_| RasterAssetIssue::InvalidDimensions)?;
    let height = usize::try_from(height).map_err(|_| RasterAssetIssue::InvalidDimensions)?;
    let pixel_count = width
        .checked_mul(height)
        .ok_or(RasterAssetIssue::SizeOverflow)?;

    let bytes_per_pixel = match bits_per_pixel {
        8 => 1usize,
        24 => 3usize,
        bits_per_pixel => {
            return Err(RasterAssetIssue::UnsupportedBitsPerPixel { bits_per_pixel });
        }
    };
    let expected_pixels = pixel_count
        .checked_mul(bytes_per_pixel)
        .ok_or(RasterAssetIssue::SizeOverflow)?;
    if pixels.len() != expected_pixels {
        return Err(RasterAssetIssue::InvalidPixelLength {
            expected: expected_pixels,
            actual: pixels.len(),
        });
    }

    if bits_per_pixel == 8 {
        let Some(palette) = palette else {
            return Err(RasterAssetIssue::MissingPalette);
        };
        if palette.len() != 256 * 3 {
            return Err(RasterAssetIssue::InvalidPaletteLength {
                expected: 256 * 3,
                actual: palette.len(),
            });
        }
    }

    if let Some(alpha) = alpha {
        if alpha.len() != pixel_count {
            return Err(RasterAssetIssue::InvalidAlphaLength {
                expected: pixel_count,
                actual: alpha.len(),
            });
        }
    }

    let rgba_len = pixel_count
        .checked_mul(4)
        .ok_or(RasterAssetIssue::SizeOverflow)?;
    let mut rgba = Vec::with_capacity(rgba_len);

    for index in 0..pixel_count {
        let (red, green, blue) = match bits_per_pixel {
            8 => {
                let palette = palette.expect("8-bit palette validated above");
                let palette_offset = usize::from(pixels[index]) * 3;
                // Diagram Designer serializes its TPalette as packed BGR triples.
                (
                    palette[palette_offset + 2],
                    palette[palette_offset + 1],
                    palette[palette_offset],
                )
            }
            24 => {
                let pixel_offset = index * 3;
                // TLinearBitmap pf24bit uses the same Windows-native BGR channel order.
                (
                    pixels[pixel_offset + 2],
                    pixels[pixel_offset + 1],
                    pixels[pixel_offset],
                )
            }
            _ => unreachable!("bits-per-pixel validated above"),
        };
        let per_pixel_alpha = alpha.map(|values| values[index]).unwrap_or(255);
        let combined_alpha =
            ((u16::from(per_pixel_alpha) * u16::from(alpha_value) + 127) / 255) as u8;
        rgba.extend_from_slice(&[red, green, blue, combined_alpha]);
    }

    Ok(rgba)
}

fn encode_rgba_png(width: i32, height: i32, rgba: &[u8]) -> Result<Vec<u8>, RasterAssetIssue> {
    let width_u32 = u32::try_from(width).map_err(|_| RasterAssetIssue::InvalidDimensions)?;
    let height_u32 = u32::try_from(height).map_err(|_| RasterAssetIssue::InvalidDimensions)?;
    let width = usize::try_from(width).map_err(|_| RasterAssetIssue::InvalidDimensions)?;
    let height = usize::try_from(height).map_err(|_| RasterAssetIssue::InvalidDimensions)?;
    let row_len = width.checked_mul(4).ok_or(RasterAssetIssue::SizeOverflow)?;
    let expected_rgba = row_len
        .checked_mul(height)
        .ok_or(RasterAssetIssue::SizeOverflow)?;
    if rgba.len() != expected_rgba {
        return Err(RasterAssetIssue::EncodingFailed);
    }

    let scanline_capacity = expected_rgba
        .checked_add(height)
        .ok_or(RasterAssetIssue::SizeOverflow)?;
    let mut scanlines = Vec::with_capacity(scanline_capacity);
    for row in rgba.chunks_exact(row_len) {
        scanlines.push(0); // PNG filter type: None.
        scanlines.extend_from_slice(row);
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&scanlines)
        .map_err(|_| RasterAssetIssue::EncodingFailed)?;
    let compressed = encoder
        .finish()
        .map_err(|_| RasterAssetIssue::EncodingFailed)?;

    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width_u32.to_be_bytes());
    ihdr.extend_from_slice(&height_u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA, zlib, no interlace.
    write_png_chunk(&mut png, b"IHDR", &ihdr)?;
    write_png_chunk(&mut png, b"IDAT", &compressed)?;
    write_png_chunk(&mut png, b"IEND", &[])?;
    Ok(png)
}

fn write_png_chunk(
    target: &mut Vec<u8>,
    kind: &[u8; 4],
    data: &[u8],
) -> Result<(), RasterAssetIssue> {
    let length = u32::try_from(data.len()).map_err(|_| RasterAssetIssue::EncodingFailed)?;
    target.extend_from_slice(&length.to_be_bytes());
    target.extend_from_slice(kind);
    target.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    target.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    Ok(())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut index = 0usize;
    while index < bytes.len() {
        let b0 = bytes[index];
        let b1 = bytes.get(index + 1).copied();
        let b2 = bytes.get(index + 2).copied();
        output.push(TABLE[(b0 >> 2) as usize] as char);
        output.push(TABLE[(((b0 & 0x03) << 4) | b1.unwrap_or(0) >> 4) as usize] as char);
        if let Some(b1) = b1 {
            output.push(TABLE[(((b1 & 0x0f) << 2) | b2.unwrap_or(0) >> 6) as usize] as char);
        } else {
            output.push('=');
        }
        if let Some(b2) = b2 {
            output.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
        index += 3;
    }
    output
}

fn push_diagnostic_once(output: &mut SvgRenderOutput, diagnostic: SvgDiagnostic) {
    if !output.diagnostics.contains(&diagnostic) {
        output.diagnostics.push(diagnostic);
    }
}

fn element_geometry_is_finite(element: &Element) -> bool {
    element.bounds_mm.x.is_finite()
        && element.bounds_mm.y.is_finite()
        && element.bounds_mm.width.is_finite()
        && element.bounds_mm.height.is_finite()
        && element.rotation_deg.is_finite()
}

fn inject_fragment_in_plan_order(
    svg: &mut String,
    plan: &RenderPlan<'_>,
    item_index: usize,
    fragment: &str,
) -> bool {
    for later in &plan.items[item_index + 1..] {
        let needle = format!("data-element-id=\"{}\"", later.element.id.0);
        let Some(attribute_at) = svg.find(&needle) else {
            continue;
        };
        let Some(tag_start) = svg[..attribute_at].rfind('<') else {
            continue;
        };
        svg.insert_str(tag_start, fragment);
        return true;
    }

    if let Some(end_svg) = svg.rfind("</svg>") {
        svg.insert_str(end_svg, fragment);
        return true;
    }
    false
}

fn normalize_rect(rect: Rect) -> Rect {
    let (x, width) = if rect.width >= 0.0 {
        (rect.x, rect.width)
    } else {
        (rect.x + rect.width, -rect.width)
    };
    let (y, height) = if rect.height >= 0.0 {
        (rect.y, rect.height)
    } else {
        (rect.y + rect.height, -rect.height)
    };
    Rect {
        x,
        y,
        width,
        height,
    }
}
