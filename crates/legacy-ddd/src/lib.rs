//! Read-only compatibility layer for legacy Diagram Designer `.ddd` and `.ddt` files.
//!
//! Phase 0 deliberately keeps this crate independent from the future editor domain model.
//! Its job is to decode legacy bytes into explicit, inspectable intermediate structures.

pub mod container;
pub mod encoding;
pub mod object;
pub mod reader;
pub mod reference;
pub mod template;
pub mod text_markup;
pub mod text_normalization;

use std::io::{Cursor, Read};

use container::{ContainerError, LegacyContainer, LegacyContainerSummary, parse_container};
use flate2::read::DeflateDecoder;
use reference::{ReferenceResolutionSummary, resolve_container_references};
use serde::Serialize;
use template::{LegacyTemplate, LegacyTemplateSummary, TemplateError, parse_template};
use thiserror::Error;

pub const HEADER_SIZE: usize = 6;
pub const DEFAULT_MAX_INFLATED_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LegacyFormat {
    Ddd,
    Ddt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyHeader {
    pub format: LegacyFormat,
    pub version: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyEnvelope {
    pub header: LegacyHeader,
    pub compressed_bytes: usize,
    pub inflated_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "format", content = "payload", rename_all = "lowercase")]
pub enum LegacyDecoded {
    Ddd(LegacyContainer),
    Ddt(LegacyTemplate),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyInspection {
    pub envelope: LegacyEnvelope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_summary: Option<LegacyContainerSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_summary: Option<LegacyTemplateSummary>,
    /// Second-pass validation of DDD legacy owner-list object/link indices.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_resolution: Option<ReferenceResolutionSummary>,
}

#[derive(Debug, Error)]
pub enum LegacyError {
    #[error("file is too short to contain a Diagram Designer header")]
    TruncatedHeader,
    #[error("invalid Delphi ShortString marker in legacy header: expected 3, found {0}")]
    InvalidShortStringLength(u8),
    #[error("unsupported legacy signature {0:?}")]
    InvalidSignature([u8; 3]),
    #[error("expected DDD document, found {0:?}")]
    ExpectedDdd(LegacyFormat),
    #[error("legacy payload decompression failed: {0}")]
    Inflate(#[source] std::io::Error),
    #[error("legacy payload exceeds safety limit of {limit} bytes")]
    InflateLimitExceeded { limit: usize },
    #[error(transparent)]
    Container(#[from] ContainerError),
    #[error(transparent)]
    Template(#[from] TemplateError),
}

/// Parse the packed Delphi header used by Diagram Designer.
///
/// The legacy source defines a packed record containing `string[3]` followed by
/// a `Word`. Delphi 7 ShortString storage includes one length byte, therefore
/// the on-disk header handled here is six bytes:
/// `[3, 'D', 'D', 'd'|'t', version_lo, version_hi]`.
pub fn parse_header(bytes: &[u8]) -> Result<LegacyHeader, LegacyError> {
    if bytes.len() < HEADER_SIZE {
        return Err(LegacyError::TruncatedHeader);
    }

    if bytes[0] != 3 {
        return Err(LegacyError::InvalidShortStringLength(bytes[0]));
    }

    let signature = [bytes[1], bytes[2], bytes[3]];
    let format = match &signature {
        b"DDd" => LegacyFormat::Ddd,
        b"DDt" => LegacyFormat::Ddt,
        _ => return Err(LegacyError::InvalidSignature(signature)),
    };

    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    Ok(LegacyHeader { format, version })
}

/// Validate the header and bounded-decompress the raw DEFLATE payload.
pub fn inflate_payload(bytes: &[u8], max_inflated_bytes: usize) -> Result<Vec<u8>, LegacyError> {
    parse_header(bytes)?;
    let payload = &bytes[HEADER_SIZE..];
    let decoder = DeflateDecoder::new(Cursor::new(payload));
    let mut limited = decoder.take(max_inflated_bytes as u64 + 1);
    let mut inflated = Vec::new();
    limited
        .read_to_end(&mut inflated)
        .map_err(LegacyError::Inflate)?;

    if inflated.len() > max_inflated_bytes {
        return Err(LegacyError::InflateLimitExceeded {
            limit: max_inflated_bytes,
        });
    }

    Ok(inflated)
}

pub fn inspect(bytes: &[u8], max_inflated_bytes: usize) -> Result<LegacyEnvelope, LegacyError> {
    let header = parse_header(bytes)?;
    let inflated = inflate_payload(bytes, max_inflated_bytes)?;
    Ok(LegacyEnvelope {
        header,
        compressed_bytes: bytes.len().saturating_sub(HEADER_SIZE),
        inflated_bytes: inflated.len(),
    })
}

/// Fully decode a DDD file into the Phase 0 legacy intermediate model.
pub fn decode_ddd(bytes: &[u8], max_inflated_bytes: usize) -> Result<LegacyContainer, LegacyError> {
    let header = parse_header(bytes)?;
    if header.format != LegacyFormat::Ddd {
        return Err(LegacyError::ExpectedDdd(header.format));
    }
    let inflated = inflate_payload(bytes, max_inflated_bytes)?;
    Ok(parse_container(&inflated, header.version)?)
}

/// Decode either legacy top-level format while reusing the same object codec.
pub fn decode_document(
    bytes: &[u8],
    max_inflated_bytes: usize,
) -> Result<LegacyDecoded, LegacyError> {
    let header = parse_header(bytes)?;
    let inflated = inflate_payload(bytes, max_inflated_bytes)?;
    match header.format {
        LegacyFormat::Ddd => Ok(LegacyDecoded::Ddd(parse_container(
            &inflated,
            header.version,
        )?)),
        LegacyFormat::Ddt => Ok(LegacyDecoded::Ddt(parse_template(
            &inflated,
            header.version,
        )?)),
    }
}

/// Inspect the envelope and fully traverse either DDD or DDT. DDD additionally
/// validates connector references in a separate second pass.
pub fn inspect_document(
    bytes: &[u8],
    max_inflated_bytes: usize,
) -> Result<LegacyInspection, LegacyError> {
    let header = parse_header(bytes)?;
    let inflated = inflate_payload(bytes, max_inflated_bytes)?;
    let envelope = LegacyEnvelope {
        header: header.clone(),
        compressed_bytes: bytes.len().saturating_sub(HEADER_SIZE),
        inflated_bytes: inflated.len(),
    };

    let (container_summary, template_summary, reference_resolution) = match header.format {
        LegacyFormat::Ddd => {
            let container = parse_container(&inflated, header.version)?;
            let summary = container.summary();
            let references = resolve_container_references(&container);
            (Some(summary), None, Some(references))
        }
        LegacyFormat::Ddt => {
            let template = parse_template(&inflated, header.version)?;
            (None, Some(template.summary()), None)
        }
    };

    Ok(LegacyInspection {
        envelope,
        container_summary,
        template_summary,
        reference_resolution,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::DeflateEncoder};
    use std::io::Write;

    fn fixture(signature: &[u8; 3], version: u16, payload: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(payload).unwrap();
        let compressed = encoder.finish().unwrap();

        let mut bytes = vec![3, signature[0], signature[1], signature[2]];
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&compressed);
        bytes
    }

    #[test]
    fn parses_ddd_header() {
        let bytes = fixture(b"DDd", 28, b"payload");
        assert_eq!(
            parse_header(&bytes).unwrap(),
            LegacyHeader {
                format: LegacyFormat::Ddd,
                version: 28,
            }
        );
    }

    #[test]
    fn parses_ddt_header() {
        let bytes = fixture(b"DDt", 28, b"template");
        assert_eq!(parse_header(&bytes).unwrap().format, LegacyFormat::Ddt);
    }

    #[test]
    fn inflates_raw_deflate_payload() {
        let bytes = fixture(b"DDd", 28, b"hello legacy world");
        assert_eq!(
            inflate_payload(&bytes, 1024).unwrap(),
            b"hello legacy world"
        );
    }

    #[test]
    fn enforces_inflate_limit() {
        let bytes = fixture(b"DDd", 28, &[42; 64]);
        assert!(matches!(
            inflate_payload(&bytes, 16),
            Err(LegacyError::InflateLimitExceeded { limit: 16 })
        ));
    }

    #[test]
    fn rejects_unknown_signature() {
        let bytes = fixture(b"XYZ", 28, b"payload");
        assert!(matches!(
            parse_header(&bytes),
            Err(LegacyError::InvalidSignature(_))
        ));
    }
}
