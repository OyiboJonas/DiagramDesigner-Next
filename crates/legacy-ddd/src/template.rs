use std::collections::BTreeMap;

use serde::Serialize;
use thiserror::Error;

use crate::{
    object::{LegacyObject, ObjectError, parse_object_list},
    reader::{LegacyReader, ReaderError},
};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LegacyTemplate {
    pub width: i32,
    pub height: i32,
    pub objects: Vec<LegacyObject>,
    pub trailing_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyTemplateSummary {
    pub width: i32,
    pub height: i32,
    pub top_level_object_count: usize,
    pub recursive_object_count: usize,
    pub object_type_counts: BTreeMap<u8, usize>,
    pub trailing_bytes: usize,
}

impl LegacyTemplate {
    pub fn summary(&self) -> LegacyTemplateSummary {
        let mut object_type_counts = BTreeMap::new();
        let recursive_object_count = self.objects.iter().map(LegacyObject::recursive_count).sum();

        for object in &self.objects {
            object.visit_type_ids(&mut |type_id| {
                *object_type_counts.entry(type_id).or_insert(0) += 1;
            });
        }

        LegacyTemplateSummary {
            width: self.width,
            height: self.height,
            top_level_object_count: self.objects.len(),
            recursive_object_count,
            object_type_counts,
            trailing_bytes: self.trailing_bytes,
        }
    }
}

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error(transparent)]
    Reader(#[from] ReaderError),
    #[error(transparent)]
    Object(#[from] ObjectError),
}

/// Decode the inflated payload written by `TTemplateSheet.SaveToStream`:
/// width, height, then inherited `TBaseObjectList` serialization.
pub fn parse_template(inflated: &[u8], file_version: u16) -> Result<LegacyTemplate, TemplateError> {
    let mut reader = LegacyReader::new(inflated);
    let width = reader.read_i32_le()?;
    let height = reader.read_i32_le()?;
    let objects = parse_object_list(&mut reader, file_version)?;

    Ok(LegacyTemplate {
        width,
        height,
        objects,
        trailing_bytes: reader.remaining(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_empty_template_payload() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&216_720_i32.to_le_bytes());
        bytes.extend_from_slice(&726_093_i32.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());

        let parsed = parse_template(&bytes, 28).unwrap();
        assert_eq!(parsed.width, 216_720);
        assert_eq!(parsed.height, 726_093);
        assert!(parsed.objects.is_empty());
        assert_eq!(parsed.trailing_bytes, 0);
    }
}
