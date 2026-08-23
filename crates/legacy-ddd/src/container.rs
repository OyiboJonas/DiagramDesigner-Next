use std::collections::BTreeMap;

use serde::Serialize;
use thiserror::Error;

use crate::{
    object::{LegacyObject, ObjectError, parse_object_list},
    reader::{LegacyReader, ReaderError},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyContainerDefaults {
    /// Raw Delphi 7 ANSI bytes. Character decoding is intentionally deferred.
    pub default_font_name_raw: Vec<u8>,
    pub default_font_size: i32,
    pub default_font_style: i32,
    pub default_font_charset: u8,
    pub object_shadows: bool,
    pub auto_line_break: bool,
    pub connector_label_style: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LegacyLayer {
    pub draw_color: i32,
    pub objects: Vec<LegacyObject>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LegacyPage {
    pub width: i32,
    pub height: i32,
    /// Raw Delphi 7 ANSI bytes. Character decoding is intentionally deferred.
    pub name_raw: Vec<u8>,
    pub layers: Vec<LegacyLayer>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LegacyContainer {
    pub defaults: LegacyContainerDefaults,
    pub pages: Vec<LegacyPage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stencil: Option<LegacyLayer>,
    /// Bytes left after parsing the source-defined container. Real golden files
    /// are expected to report zero. Keeping this explicit avoids silent loss if
    /// an historical variant carries additional data.
    pub trailing_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyPageSummary {
    pub width: i32,
    pub height: i32,
    pub name_raw: Vec<u8>,
    pub layer_count: usize,
    pub top_level_object_count: usize,
    pub recursive_object_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyContainerSummary {
    pub defaults: LegacyContainerDefaults,
    pub page_count: usize,
    pub layer_count: usize,
    pub top_level_object_count: usize,
    pub recursive_object_count: usize,
    pub object_type_counts: BTreeMap<u8, usize>,
    pub stencil_top_level_object_count: usize,
    pub trailing_bytes: usize,
    pub pages: Vec<LegacyPageSummary>,
}

impl LegacyContainer {
    pub fn summary(&self) -> LegacyContainerSummary {
        let mut object_type_counts = BTreeMap::new();
        let mut page_summaries = Vec::with_capacity(self.pages.len());
        let mut layer_count = 0;
        let mut top_level_object_count = 0;
        let mut recursive_object_count = 0;

        for page in &self.pages {
            let page_top_level = page.layers.iter().map(|layer| layer.objects.len()).sum();
            let page_recursive = page
                .layers
                .iter()
                .flat_map(|layer| &layer.objects)
                .map(LegacyObject::recursive_count)
                .sum();

            for object in page.layers.iter().flat_map(|layer| &layer.objects) {
                object.visit_type_ids(&mut |type_id| {
                    *object_type_counts.entry(type_id).or_insert(0) += 1;
                });
            }

            layer_count += page.layers.len();
            top_level_object_count += page_top_level;
            recursive_object_count += page_recursive;
            page_summaries.push(LegacyPageSummary {
                width: page.width,
                height: page.height,
                name_raw: page.name_raw.clone(),
                layer_count: page.layers.len(),
                top_level_object_count: page_top_level,
                recursive_object_count: page_recursive,
            });
        }

        let stencil_top_level_object_count =
            self.stencil.as_ref().map_or(0, |layer| layer.objects.len());
        if let Some(stencil) = &self.stencil {
            for object in &stencil.objects {
                recursive_object_count += object.recursive_count();
                object.visit_type_ids(&mut |type_id| {
                    *object_type_counts.entry(type_id).or_insert(0) += 1;
                });
            }
        }

        LegacyContainerSummary {
            defaults: self.defaults.clone(),
            page_count: self.pages.len(),
            layer_count,
            top_level_object_count,
            recursive_object_count,
            object_type_counts,
            stencil_top_level_object_count,
            trailing_bytes: self.trailing_bytes,
            pages: page_summaries,
        }
    }
}

#[derive(Debug, Error)]
pub enum ContainerError {
    #[error(transparent)]
    Reader(#[from] ReaderError),
    #[error(transparent)]
    Object(#[from] ObjectError),
    #[error("legacy boolean at offset {offset} has invalid byte value {value}")]
    InvalidBoolean { offset: usize, value: u8 },
    #[error(
        "connector label style {value} is outside the known legacy range 0..=2 at offset {offset}"
    )]
    InvalidConnectorLabelStyle { offset: usize, value: u8 },
}

fn read_bool(reader: &mut LegacyReader<'_>) -> Result<bool, ContainerError> {
    let offset = reader.offset();
    match reader.read_u8()? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(ContainerError::InvalidBoolean { offset, value }),
    }
}

fn parse_defaults(
    reader: &mut LegacyReader<'_>,
    file_version: u16,
) -> Result<LegacyContainerDefaults, ContainerError> {
    let default_font_name_raw = reader.read_string16_raw()?;
    let default_font_size = reader.read_i32_le()?;
    let default_font_style = reader.read_i32_le()?;
    let default_font_charset = if file_version >= 23 {
        reader.read_u8()?
    } else {
        1
    };
    let object_shadows = if file_version >= 19 {
        read_bool(reader)?
    } else {
        false
    };
    let auto_line_break = if file_version >= 21 {
        read_bool(reader)?
    } else {
        false
    };
    let connector_label_style = if file_version >= 27 {
        let offset = reader.offset();
        let value = reader.read_u8()?;
        if value > 2 {
            return Err(ContainerError::InvalidConnectorLabelStyle { offset, value });
        }
        value
    } else {
        // clsSolid in the original enum.
        1
    };

    Ok(LegacyContainerDefaults {
        default_font_name_raw,
        default_font_size,
        default_font_style,
        default_font_charset,
        object_shadows,
        auto_line_break,
        connector_label_style,
    })
}

fn parse_layer(
    reader: &mut LegacyReader<'_>,
    file_version: u16,
) -> Result<LegacyLayer, ContainerError> {
    let draw_color = reader.read_i32_le()?;
    let objects = parse_object_list(reader, file_version)?;
    Ok(LegacyLayer {
        draw_color,
        objects,
    })
}

fn parse_page(
    reader: &mut LegacyReader<'_>,
    file_version: u16,
) -> Result<LegacyPage, ContainerError> {
    let width = reader.read_i32_le()?;
    let height = reader.read_i32_le()?;
    let name_raw = reader.read_string16_raw()?;
    let layer_count = reader.read_u16_le()? as usize;
    let mut layers = Vec::with_capacity(layer_count);
    for _ in 0..layer_count {
        layers.push(parse_layer(reader, file_version)?);
    }

    Ok(LegacyPage {
        width,
        height,
        name_raw,
        layers,
    })
}

/// Decode a complete source-defined DDD container using the object codecs that
/// have been implemented in Phase 0.
///
/// Unsupported object type IDs fail explicitly instead of guessing their byte
/// length. This is the key compatibility invariant: traversal only advances
/// when the legacy source layout is known.
pub fn parse_container(
    inflated: &[u8],
    file_version: u16,
) -> Result<LegacyContainer, ContainerError> {
    let mut reader = LegacyReader::new(inflated);
    let defaults = parse_defaults(&mut reader, file_version)?;

    let page_count = reader.read_u16_le()? as usize;
    let mut pages = Vec::with_capacity(page_count);
    for _ in 0..page_count {
        pages.push(parse_page(&mut reader, file_version)?);
    }

    let stencil = if file_version >= 5 {
        Some(parse_layer(&mut reader, file_version)?)
    } else {
        None
    };

    Ok(LegacyContainer {
        defaults,
        pages,
        stencil,
        trailing_bytes: reader.remaining(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_string16(target: &mut Vec<u8>, value: &[u8]) {
        target.extend_from_slice(&(value.len() as u16).to_le_bytes());
        target.extend_from_slice(value);
    }

    fn push_empty_layer(target: &mut Vec<u8>) {
        target.extend_from_slice(&(-1_i32).to_le_bytes());
        target.extend_from_slice(&0_u16.to_le_bytes());
    }

    #[test]
    fn parses_complete_empty_v28_container() {
        let mut bytes = Vec::new();
        push_string16(&mut bytes, b"Arial");
        bytes.extend_from_slice(&12_i32.to_le_bytes());
        bytes.extend_from_slice(&0_i32.to_le_bytes());
        bytes.push(1); // charset
        bytes.push(0); // shadows
        bytes.push(1); // auto line break
        bytes.push(0); // connector label style
        bytes.extend_from_slice(&1_u16.to_le_bytes()); // pages

        bytes.extend_from_slice(&1496880_i32.to_le_bytes());
        bytes.extend_from_slice(&2119320_i32.to_le_bytes());
        push_string16(&mut bytes, b"Flow");
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        push_empty_layer(&mut bytes);

        push_empty_layer(&mut bytes); // stencil

        let parsed = parse_container(&bytes, 28).unwrap();
        assert_eq!(parsed.pages.len(), 1);
        assert_eq!(parsed.pages[0].layers.len(), 1);
        assert_eq!(parsed.trailing_bytes, 0);

        let summary = parsed.summary();
        assert_eq!(summary.page_count, 1);
        assert_eq!(summary.top_level_object_count, 0);
        assert_eq!(summary.recursive_object_count, 0);
    }

    #[test]
    fn applies_v26_connector_label_default() {
        let mut bytes = Vec::new();
        push_string16(&mut bytes, b"");
        bytes.extend_from_slice(&0_i32.to_le_bytes());
        bytes.extend_from_slice(&0_i32.to_le_bytes());
        bytes.push(0); // charset
        bytes.push(0); // shadows
        bytes.push(1); // auto line break
        // no connector label byte in v26
        bytes.extend_from_slice(&0_u16.to_le_bytes()); // pages
        push_empty_layer(&mut bytes); // stencil

        let parsed = parse_container(&bytes, 26).unwrap();
        assert_eq!(parsed.defaults.connector_label_style, 1);
        assert_eq!(parsed.pages.len(), 0);
        assert_eq!(parsed.trailing_bytes, 0);
    }
}
