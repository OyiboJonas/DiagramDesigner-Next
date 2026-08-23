use serde::Serialize;
use thiserror::Error;

use crate::reader::{LegacyReader, ReaderError};

const DEFAULT_MARGIN: i32 = 1_778;
const CL_WHITE: i32 = 0x00ff_ffff;
const CL_NONE: i32 = 0x1fff_ffff;
const MAX_NESTING_DEPTH: usize = 64;
const LEGACY_PALETTE_BYTES: usize = 256 * 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LegacyRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct LegacyFloatPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LegacyPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyBaseObject {
    /// Raw Delphi 7 ANSI bytes. Character decoding is deliberately deferred.
    pub name_raw: Vec<u8>,
    pub position: LegacyRect,
    /// Serialized Delphi set `TObjectAnchors`. The current Delphi 7 build stores
    /// this six-member set in one byte; real v26/v28 files confirm the layout.
    pub anchors: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LegacyTextPayload {
    pub base: LegacyBaseObject,
    /// Raw bytes written by legacy `SaveString`.
    pub text_raw: Vec<u8>,
    pub text_x_align: i8,
    pub text_y_align: i8,
    pub text_color: i32,
    pub margin: i32,
    pub angle: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LegacyLinePayload {
    pub text: LegacyTextPayload,
    pub line_width: i32,
    pub line_color: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LegacyShapePayload {
    pub line: LegacyLinePayload,
    pub fill_color: i32,
    pub gradient_color: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LegacyLinkReference {
    pub object_index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_index: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LegacyConnectorPayload {
    pub line: LegacyLinePayload,
    pub start_marker: u16,
    pub end_marker: u16,
    pub line_style: u16,
    pub fill_color: i32,
    pub links: [LegacyLinkReference; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LegacyPicturePayload {
    pub base: LegacyBaseObject,
    pub links: Vec<LegacyFloatPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyBitmapData {
    pub halftone_stretch: bool,
    pub width: i32,
    pub height: i32,
    pub bits_per_pixel: u8,
    /// Legacy 8-bit palettes contain 256 packed BGR triples (768 bytes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette_raw: Option<Vec<u8>>,
    pub image_raw: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha_raw: Option<Vec<u8>>,
    pub alpha_value: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "base_kind", rename_all = "snake_case")]
pub enum LegacyCurveBase {
    /// Files before v28 deliberately bypass TBaseConnectorObject during load.
    Line { line: LegacyLinePayload },
    /// v28+ curve objects use the connector payload, including endpoint refs.
    Connector { connector: LegacyConnectorPayload },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LegacyObject {
    Text {
        payload: LegacyTextPayload,
    },
    Rectangle {
        shape: LegacyShapePayload,
        corner_radius: i32,
        /// `None` means the legacy standard shape links are used.
        custom_links: Option<Vec<LegacyFloatPoint>>,
    },
    Ellipse {
        shape: LegacyShapePayload,
    },
    StraightLine {
        connector: LegacyConnectorPayload,
    },
    ConnectorLine {
        connector: LegacyConnectorPayload,
        corner_radius: i32,
    },
    Bitmap {
        picture: LegacyPicturePayload,
        bitmap: LegacyBitmapData,
    },
    Metafile {
        picture: LegacyPicturePayload,
        metafile_raw: Vec<u8>,
        angle: f32,
    },
    Group {
        base: LegacyBaseObject,
        children: Vec<LegacyObject>,
        links: Vec<LegacyFloatPoint>,
    },
    Polygon {
        shape: LegacyShapePayload,
        points: Vec<LegacyFloatPoint>,
    },
    Flowchart {
        shape: LegacyShapePayload,
        flowchart_type: i32,
    },
    CurveLine {
        base: LegacyCurveBase,
        curve_type: u8,
        points: Vec<LegacyPoint>,
    },
    InheritedLayer {
        picture: LegacyPicturePayload,
        relative_page_index: i32,
        layer_index: i32,
    },
}

impl LegacyObject {
    pub fn legacy_type_id(&self) -> u8 {
        match self {
            Self::Text { .. } => 1,
            Self::Rectangle { .. } => 2,
            Self::Ellipse { .. } => 3,
            Self::StraightLine { .. } => 4,
            Self::ConnectorLine { .. } => 5,
            Self::Bitmap { .. } => 6,
            Self::Metafile { .. } => 7,
            Self::Group { .. } => 8,
            Self::Polygon { .. } => 9,
            Self::Flowchart { .. } => 10,
            Self::CurveLine { .. } => 11,
            Self::InheritedLayer { .. } => 12,
        }
    }

    pub fn recursive_count(&self) -> usize {
        match self {
            Self::Group { children, .. } => {
                1 + children.iter().map(Self::recursive_count).sum::<usize>()
            }
            _ => 1,
        }
    }

    pub fn visit_type_ids(&self, visitor: &mut impl FnMut(u8)) {
        visitor(self.legacy_type_id());
        if let Self::Group { children, .. } = self {
            for child in children {
                child.visit_type_ids(visitor);
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum ObjectError {
    #[error(transparent)]
    Reader(#[from] ReaderError),
    #[error("unsupported legacy object type {object_type} at offset {offset}")]
    UnsupportedObjectType { offset: usize, object_type: u8 },
    #[error("legacy group nesting exceeds safety limit of {limit} levels at offset {offset}")]
    NestingLimitExceeded { offset: usize, limit: usize },
    #[error("legacy bitmap dimensions {width}x{height} are invalid at offset {offset}")]
    InvalidBitmapDimensions {
        offset: usize,
        width: i32,
        height: i32,
    },
    #[error("legacy bitmap dimensions overflow byte-size calculation at offset {offset}")]
    BitmapSizeOverflow { offset: usize },
    #[error(
        "legacy bitmap bits-per-pixel value {bits_per_pixel} is unsupported at offset {offset}"
    )]
    UnsupportedBitmapBitsPerPixel { offset: usize, bits_per_pixel: u8 },
    #[error("legacy bitmap alpha format value {format} is unsupported at offset {offset}")]
    UnsupportedBitmapFormat { offset: usize, format: u8 },
    #[error("legacy boolean at offset {offset} has invalid byte value {value}")]
    InvalidBoolean { offset: usize, value: u8 },
    #[error("legacy metafile length {length} is invalid at offset {offset}")]
    InvalidMetafileLength { offset: usize, length: i32 },
    #[error("legacy curve type {curve_type} is outside the known range 0..=3 at offset {offset}")]
    InvalidCurveType { offset: usize, curve_type: u8 },
}

fn read_bool(reader: &mut LegacyReader<'_>) -> Result<bool, ObjectError> {
    let offset = reader.offset();
    match reader.read_u8()? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(ObjectError::InvalidBoolean { offset, value }),
    }
}

fn parse_base_object(
    reader: &mut LegacyReader<'_>,
    file_version: u16,
) -> Result<LegacyBaseObject, ObjectError> {
    let name_raw = reader.read_string16_raw()?;
    let position = LegacyRect {
        left: reader.read_i32_le()?,
        top: reader.read_i32_le()?,
        right: reader.read_i32_le()?,
        bottom: reader.read_i32_le()?,
    };
    let anchors = if file_version >= 13 {
        reader.read_u8()?
    } else {
        0
    };

    Ok(LegacyBaseObject {
        name_raw,
        position,
        anchors,
    })
}

fn parse_text_payload(
    reader: &mut LegacyReader<'_>,
    file_version: u16,
) -> Result<LegacyTextPayload, ObjectError> {
    let base = parse_base_object(reader, file_version)?;
    let text_raw = reader.read_string32_raw()?;
    let text_x_align = reader.read_i8()?;
    let text_y_align = reader.read_i8()?;
    let text_color = reader.read_i32_le()?;
    let mut margin = if file_version >= 11 {
        reader.read_i32_le()?
    } else {
        DEFAULT_MARGIN
    };

    // Mirror the historical compatibility adjustment in TTextObject.LoadFromStream.
    // The original source checks X alignment twice, so the effective condition is
    // simply X-align == 2 for pre-v26 files.
    if file_version < 26 && text_x_align == 2 {
        margin = 0;
    }

    let angle = if file_version >= 22 {
        reader.read_f32_le()?
    } else {
        0.0
    };

    Ok(LegacyTextPayload {
        base,
        text_raw,
        text_x_align,
        text_y_align,
        text_color,
        margin,
        angle,
    })
}

fn parse_line_payload(
    reader: &mut LegacyReader<'_>,
    file_version: u16,
) -> Result<LegacyLinePayload, ObjectError> {
    let text = parse_text_payload(reader, file_version)?;
    let line_width = reader.read_i32_le()?;
    let line_color = reader.read_i32_le()?;
    Ok(LegacyLinePayload {
        text,
        line_width,
        line_color,
    })
}

fn parse_shape_payload(
    reader: &mut LegacyReader<'_>,
    file_version: u16,
) -> Result<LegacyShapePayload, ObjectError> {
    let line = parse_line_payload(reader, file_version)?;
    let fill_color = reader.read_i32_le()?;
    let gradient_color = if file_version >= 20 {
        reader.read_i32_le()?
    } else {
        CL_NONE
    };
    Ok(LegacyShapePayload {
        line,
        fill_color,
        gradient_color,
    })
}

fn parse_link_reference(reader: &mut LegacyReader<'_>) -> Result<LegacyLinkReference, ObjectError> {
    let object_index = reader.read_i32_le()?;
    let link_index = if object_index == -1 {
        None
    } else {
        Some(reader.read_u16_le()?)
    };
    Ok(LegacyLinkReference {
        object_index,
        link_index,
    })
}

fn parse_connector_payload(
    reader: &mut LegacyReader<'_>,
    file_version: u16,
) -> Result<LegacyConnectorPayload, ObjectError> {
    let line = parse_line_payload(reader, file_version)?;
    let start_marker = reader.read_u16_le()?;
    let end_marker = reader.read_u16_le()?;
    let line_style = if file_version >= 9 {
        reader.read_u16_le()?
    } else {
        0
    };
    let fill_color = if file_version >= 18 {
        reader.read_i32_le()?
    } else {
        CL_WHITE
    };
    let links = [parse_link_reference(reader)?, parse_link_reference(reader)?];

    Ok(LegacyConnectorPayload {
        line,
        start_marker,
        end_marker,
        line_style,
        fill_color,
        links,
    })
}

fn parse_points(
    reader: &mut LegacyReader<'_>,
    count: usize,
) -> Result<Vec<LegacyFloatPoint>, ObjectError> {
    let mut points = Vec::with_capacity(count);
    for _ in 0..count {
        points.push(LegacyFloatPoint {
            x: reader.read_f64_le()?,
            y: reader.read_f64_le()?,
        });
    }
    Ok(points)
}

fn parse_integer_points(
    reader: &mut LegacyReader<'_>,
    count: usize,
) -> Result<Vec<LegacyPoint>, ObjectError> {
    let mut points = Vec::with_capacity(count);
    for _ in 0..count {
        points.push(LegacyPoint {
            x: reader.read_i32_le()?,
            y: reader.read_i32_le()?,
        });
    }
    Ok(points)
}

fn parse_picture_payload(
    reader: &mut LegacyReader<'_>,
    file_version: u16,
) -> Result<LegacyPicturePayload, ObjectError> {
    let base = parse_base_object(reader, file_version)?;
    let links = if file_version >= 2 {
        let count = reader.read_u16_le()? as usize;
        parse_points(reader, count)?
    } else {
        Vec::new()
    };
    Ok(LegacyPicturePayload { base, links })
}

fn checked_bitmap_size(
    width: i32,
    height: i32,
    bytes_per_pixel: usize,
    offset: usize,
) -> Result<usize, ObjectError> {
    if width < 1 || height < 1 {
        return Err(ObjectError::InvalidBitmapDimensions {
            offset,
            width,
            height,
        });
    }

    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(bytes_per_pixel))
        .ok_or(ObjectError::BitmapSizeOverflow { offset })
}

fn parse_bitmap_data(
    reader: &mut LegacyReader<'_>,
    file_version: u16,
) -> Result<LegacyBitmapData, ObjectError> {
    let halftone_stretch = read_bool(reader)?;
    let format_offset = reader.offset();
    let format = reader.read_u8()?;
    if format > 1 {
        return Err(ObjectError::UnsupportedBitmapFormat {
            offset: format_offset,
            format,
        });
    }

    let dimensions_offset = reader.offset();
    let width = reader.read_i32_le()?;
    let height = reader.read_i32_le()?;
    let bpp_offset = reader.offset();
    let bits_per_pixel = reader.read_u8()?;
    let bytes_per_pixel = match bits_per_pixel {
        8 => 1,
        24 => 3,
        value => {
            return Err(ObjectError::UnsupportedBitmapBitsPerPixel {
                offset: bpp_offset,
                bits_per_pixel: value,
            });
        }
    };

    let image_size = checked_bitmap_size(width, height, bytes_per_pixel, dimensions_offset)?;
    let palette_raw = if bits_per_pixel == 8 {
        Some(reader.read_blob(LEGACY_PALETTE_BYTES)?)
    } else {
        None
    };
    let image_raw = reader.read_blob(image_size)?;
    let alpha_raw = if format == 1 {
        let alpha_size = checked_bitmap_size(width, height, 1, dimensions_offset)?;
        Some(reader.read_blob(alpha_size)?)
    } else {
        None
    };
    let alpha_value = if file_version >= 17 {
        reader.read_u8()?
    } else {
        255
    };

    Ok(LegacyBitmapData {
        halftone_stretch,
        width,
        height,
        bits_per_pixel,
        palette_raw,
        image_raw,
        alpha_raw,
        alpha_value,
    })
}

pub fn parse_object_list(
    reader: &mut LegacyReader<'_>,
    file_version: u16,
) -> Result<Vec<LegacyObject>, ObjectError> {
    parse_object_list_at_depth(reader, file_version, 0)
}

fn parse_object_list_at_depth(
    reader: &mut LegacyReader<'_>,
    file_version: u16,
    depth: usize,
) -> Result<Vec<LegacyObject>, ObjectError> {
    if depth > MAX_NESTING_DEPTH {
        return Err(ObjectError::NestingLimitExceeded {
            offset: reader.offset(),
            limit: MAX_NESTING_DEPTH,
        });
    }

    let object_count = reader.read_u16_le()? as usize;
    let mut objects = Vec::with_capacity(object_count);
    for _ in 0..object_count {
        let type_offset = reader.offset();
        let object_type = reader.read_u8()?;
        objects.push(parse_object(
            reader,
            file_version,
            object_type,
            type_offset,
            depth,
        )?);
    }
    Ok(objects)
}

fn parse_object(
    reader: &mut LegacyReader<'_>,
    file_version: u16,
    object_type: u8,
    type_offset: usize,
    depth: usize,
) -> Result<LegacyObject, ObjectError> {
    match object_type {
        1 => Ok(LegacyObject::Text {
            payload: parse_text_payload(reader, file_version)?,
        }),
        2 => {
            let shape = parse_shape_payload(reader, file_version)?;
            let corner_radius = if file_version >= 15 {
                reader.read_i32_le()?
            } else {
                0
            };
            let custom_links = if file_version >= 24 {
                let count = reader.read_i16_le()?;
                if count < 0 {
                    None
                } else {
                    Some(parse_points(reader, count as usize)?)
                }
            } else {
                None
            };
            Ok(LegacyObject::Rectangle {
                shape,
                corner_radius,
                custom_links,
            })
        }
        3 => Ok(LegacyObject::Ellipse {
            shape: parse_shape_payload(reader, file_version)?,
        }),
        4 => Ok(LegacyObject::StraightLine {
            connector: parse_connector_payload(reader, file_version)?,
        }),
        5 => {
            let connector = parse_connector_payload(reader, file_version)?;
            let corner_radius = if file_version >= 14 {
                reader.read_i32_le()?
            } else {
                0
            };
            Ok(LegacyObject::ConnectorLine {
                connector,
                corner_radius,
            })
        }
        6 => Ok(LegacyObject::Bitmap {
            picture: parse_picture_payload(reader, file_version)?,
            bitmap: parse_bitmap_data(reader, file_version)?,
        }),
        7 => {
            let picture = parse_picture_payload(reader, file_version)?;
            let length_offset = reader.offset();
            let length = reader.read_i32_le()?;
            if length < 0 {
                return Err(ObjectError::InvalidMetafileLength {
                    offset: length_offset,
                    length,
                });
            }
            let metafile_raw = reader.read_blob(length as usize)?;
            let angle = if file_version >= 4 {
                reader.read_f32_le()?
            } else {
                0.0
            };
            Ok(LegacyObject::Metafile {
                picture,
                metafile_raw,
                angle,
            })
        }
        8 => {
            let base = parse_base_object(reader, file_version)?;
            let children = parse_object_list_at_depth(reader, file_version, depth + 1)?;
            let links = if file_version >= 2 {
                let count = reader.read_u16_le()? as usize;
                parse_points(reader, count)?
            } else {
                Vec::new()
            };
            Ok(LegacyObject::Group {
                base,
                children,
                links,
            })
        }
        9 => {
            let shape = parse_shape_payload(reader, file_version)?;
            let count = reader.read_u16_le()? as usize;
            Ok(LegacyObject::Polygon {
                shape,
                points: parse_points(reader, count)?,
            })
        }
        10 => Ok(LegacyObject::Flowchart {
            shape: parse_shape_payload(reader, file_version)?,
            flowchart_type: reader.read_i32_le()?,
        }),
        11 => {
            let base = if file_version < 28 {
                LegacyCurveBase::Line {
                    line: parse_line_payload(reader, file_version)?,
                }
            } else {
                LegacyCurveBase::Connector {
                    connector: parse_connector_payload(reader, file_version)?,
                }
            };
            let curve_type = if file_version >= 16 {
                let offset = reader.offset();
                let value = reader.read_u8()?;
                if value > 3 {
                    return Err(ObjectError::InvalidCurveType {
                        offset,
                        curve_type: value,
                    });
                }
                value
            } else {
                // ctLegacy
                1
            };
            let point_count = reader.read_u16_le()? as usize;
            Ok(LegacyObject::CurveLine {
                base,
                curve_type,
                points: parse_integer_points(reader, point_count)?,
            })
        }
        12 => Ok(LegacyObject::InheritedLayer {
            picture: parse_picture_payload(reader, file_version)?,
            relative_page_index: reader.read_i32_le()?,
            layer_index: reader.read_i32_le()?,
        }),
        _ => Err(ObjectError::UnsupportedObjectType {
            offset: type_offset,
            object_type,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_string16(target: &mut Vec<u8>, value: &[u8]) {
        target.extend_from_slice(&(value.len() as u16).to_le_bytes());
        target.extend_from_slice(value);
    }

    fn push_string32(target: &mut Vec<u8>, value: &[u8]) {
        target.extend_from_slice(&(value.len() as u32).to_le_bytes());
        target.extend_from_slice(value);
    }

    fn push_base(target: &mut Vec<u8>, name: &[u8]) {
        push_string16(target, name);
        for value in [10_i32, 20, 30, 40] {
            target.extend_from_slice(&value.to_le_bytes());
        }
        target.push(0);
    }

    fn push_text(target: &mut Vec<u8>, name: &[u8], text: &[u8]) {
        push_base(target, name);
        push_string32(target, text);
        target.push(0);
        target.push(0);
        target.extend_from_slice(&0_i32.to_le_bytes());
        target.extend_from_slice(&DEFAULT_MARGIN.to_le_bytes());
        target.extend_from_slice(&0_f32.to_le_bytes());
    }

    fn push_line(target: &mut Vec<u8>, name: &[u8], text: &[u8]) {
        push_text(target, name, text);
        target.extend_from_slice(&666_i32.to_le_bytes());
        target.extend_from_slice(&0_i32.to_le_bytes());
    }

    fn push_shape(target: &mut Vec<u8>, name: &[u8], text: &[u8]) {
        push_line(target, name, text);
        target.extend_from_slice(&CL_WHITE.to_le_bytes());
        target.extend_from_slice(&CL_NONE.to_le_bytes());
    }

    fn push_picture(target: &mut Vec<u8>, name: &[u8]) {
        push_base(target, name);
        target.extend_from_slice(&0_u16.to_le_bytes());
    }

    fn push_connector(target: &mut Vec<u8>, name: &[u8]) {
        push_line(target, name, b"");
        target.extend_from_slice(&0_u16.to_le_bytes());
        target.extend_from_slice(&0_u16.to_le_bytes());
        target.extend_from_slice(&0_u16.to_le_bytes());
        target.extend_from_slice(&CL_WHITE.to_le_bytes());
        target.extend_from_slice(&(-1_i32).to_le_bytes());
        target.extend_from_slice(&(-1_i32).to_le_bytes());
    }

    #[test]
    fn parses_polygon_and_straight_line_payload_boundaries() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2_u16.to_le_bytes());

        bytes.push(9);
        push_shape(&mut bytes, b"Box", b"Hello");
        bytes.extend_from_slice(&4_u16.to_le_bytes());
        for (x, y) in [(0.0_f64, 0.0_f64), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
        }

        bytes.push(4);
        push_connector(&mut bytes, b"StraightLine");

        let mut reader = LegacyReader::new(&bytes);
        let parsed = parse_object_list(&mut reader, 28).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].legacy_type_id(), 9);
        assert_eq!(parsed[1].legacy_type_id(), 4);
        assert!(reader.is_eof());
    }

    #[test]
    fn parses_nested_group_with_flowchart_and_rectangle() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.push(8);
        push_base(&mut bytes, b"Group");

        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.push(10);
        push_shape(&mut bytes, b"Flow", b"Process");
        bytes.extend_from_slice(&0x21_i32.to_le_bytes());

        bytes.push(2);
        push_shape(&mut bytes, b"Body", b"");
        bytes.extend_from_slice(&0_i32.to_le_bytes());
        bytes.extend_from_slice(&(-1_i16).to_le_bytes());

        bytes.extend_from_slice(&4_u16.to_le_bytes());
        for (x, y) in [(0.0_f64, 0.0_f64), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
        }

        let mut reader = LegacyReader::new(&bytes);
        let parsed = parse_object_list(&mut reader, 28).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].recursive_count(), 3);
        assert!(reader.is_eof());
    }

    #[test]
    fn parses_bitmap_metafile_curve_and_inherited_layer_boundaries() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&4_u16.to_le_bytes());

        bytes.push(6);
        push_picture(&mut bytes, b"Bitmap");
        bytes.push(1); // halftone
        bytes.push(0); // no alpha channel
        bytes.extend_from_slice(&1_i32.to_le_bytes());
        bytes.extend_from_slice(&1_i32.to_le_bytes());
        bytes.push(24);
        bytes.extend_from_slice(&[1, 2, 3]);
        bytes.push(255);

        bytes.push(7);
        push_picture(&mut bytes, b"Metafile");
        bytes.extend_from_slice(&3_i32.to_le_bytes());
        bytes.extend_from_slice(&[7, 8, 9]);
        bytes.extend_from_slice(&0.5_f32.to_le_bytes());

        bytes.push(11);
        push_connector(&mut bytes, b"CurveLine");
        bytes.push(2); // ctBezier
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        for (x, y) in [(10_i32, 20_i32), (30, 40)] {
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
        }

        bytes.push(12);
        push_picture(&mut bytes, b"InheritedLayer");
        bytes.extend_from_slice(&(-1_i32).to_le_bytes());
        bytes.extend_from_slice(&2_i32.to_le_bytes());

        let mut reader = LegacyReader::new(&bytes);
        let parsed = parse_object_list(&mut reader, 28).unwrap();
        assert_eq!(
            parsed
                .iter()
                .map(LegacyObject::legacy_type_id)
                .collect::<Vec<_>>(),
            vec![6, 7, 11, 12]
        );
        assert!(reader.is_eof());
    }

    #[test]
    fn parses_pre_v28_curve_without_connector_payload() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.push(11);
        push_line(&mut bytes, b"CurveLine", b"");
        bytes.push(1); // ctLegacy
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&10_i32.to_le_bytes());
        bytes.extend_from_slice(&20_i32.to_le_bytes());

        let mut reader = LegacyReader::new(&bytes);
        let parsed = parse_object_list(&mut reader, 26).unwrap();
        assert!(matches!(
            parsed.as_slice(),
            [LegacyObject::CurveLine {
                base: LegacyCurveBase::Line { .. },
                ..
            }]
        ));
        assert!(reader.is_eof());
    }

    #[test]
    fn rejects_unknown_object_type_explicitly() {
        let bytes = [1, 0, 99];
        let mut reader = LegacyReader::new(&bytes);
        assert!(matches!(
            parse_object_list(&mut reader, 28),
            Err(ObjectError::UnsupportedObjectType {
                object_type: 99,
                ..
            })
        ));
    }
}
