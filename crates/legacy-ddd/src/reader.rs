use thiserror::Error;

#[derive(Debug, Clone, Copy)]
pub struct ReaderLimits {
    pub max_string_bytes: usize,
    pub max_blob_bytes: usize,
}

impl Default for ReaderLimits {
    fn default() -> Self {
        Self {
            max_string_bytes: 16 * 1024 * 1024,
            max_blob_bytes: 128 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReaderError {
    #[error(
        "unexpected end of legacy payload at offset {offset}: requested {requested} bytes, {remaining} remain"
    )]
    UnexpectedEof {
        offset: usize,
        requested: usize,
        remaining: usize,
    },
    #[error("legacy string length {length} exceeds limit of {limit} bytes at offset {offset}")]
    StringLimitExceeded {
        offset: usize,
        length: usize,
        limit: usize,
    },
    #[error("legacy blob length {length} exceeds limit of {limit} bytes at offset {offset}")]
    BlobLimitExceeded {
        offset: usize,
        length: usize,
        limit: usize,
    },
}

/// Small, deterministic reader for the inflated legacy byte stream.
///
/// It intentionally exposes raw string bytes. Character decoding belongs to a
/// later compatibility stage because Delphi 7 documents are not intrinsically
/// UTF-8 and may depend on historical ANSI code pages/font charsets.
pub struct LegacyReader<'a> {
    bytes: &'a [u8],
    offset: usize,
    limits: ReaderLimits,
}

impl<'a> LegacyReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self::with_limits(bytes, ReaderLimits::default())
    }

    pub fn with_limits(bytes: &'a [u8], limits: ReaderLimits) -> Self {
        Self {
            bytes,
            offset: 0,
            limits,
        }
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    pub fn is_eof(&self) -> bool {
        self.offset == self.bytes.len()
    }

    pub fn read_exact(&mut self, len: usize) -> Result<&'a [u8], ReaderError> {
        let remaining = self.remaining();
        if len > remaining {
            return Err(ReaderError::UnexpectedEof {
                offset: self.offset,
                requested: len,
                remaining,
            });
        }

        let start = self.offset;
        self.offset += len;
        Ok(&self.bytes[start..self.offset])
    }

    pub fn read_u8(&mut self) -> Result<u8, ReaderError> {
        Ok(self.read_exact(1)?[0])
    }

    pub fn read_i8(&mut self) -> Result<i8, ReaderError> {
        Ok(self.read_u8()? as i8)
    }

    pub fn read_u16_le(&mut self) -> Result<u16, ReaderError> {
        let bytes: [u8; 2] = self.read_exact(2)?.try_into().expect("exact length");
        Ok(u16::from_le_bytes(bytes))
    }

    pub fn read_i16_le(&mut self) -> Result<i16, ReaderError> {
        let bytes: [u8; 2] = self.read_exact(2)?.try_into().expect("exact length");
        Ok(i16::from_le_bytes(bytes))
    }

    pub fn read_u32_le(&mut self) -> Result<u32, ReaderError> {
        let bytes: [u8; 4] = self.read_exact(4)?.try_into().expect("exact length");
        Ok(u32::from_le_bytes(bytes))
    }

    pub fn read_i32_le(&mut self) -> Result<i32, ReaderError> {
        let bytes: [u8; 4] = self.read_exact(4)?.try_into().expect("exact length");
        Ok(i32::from_le_bytes(bytes))
    }

    pub fn read_f32_le(&mut self) -> Result<f32, ReaderError> {
        let bytes: [u8; 4] = self.read_exact(4)?.try_into().expect("exact length");
        Ok(f32::from_le_bytes(bytes))
    }

    pub fn read_f64_le(&mut self) -> Result<f64, ReaderError> {
        let bytes: [u8; 8] = self.read_exact(8)?.try_into().expect("exact length");
        Ok(f64::from_le_bytes(bytes))
    }

    /// Read the raw byte representation written by legacy `SaveString`, whose
    /// source implementation prefixes the byte sequence with a 32-bit length.
    pub fn read_string32_raw(&mut self) -> Result<Vec<u8>, ReaderError> {
        let length_offset = self.offset;
        let length = self.read_u32_le()? as usize;
        if length > self.limits.max_string_bytes {
            return Err(ReaderError::StringLimitExceeded {
                offset: length_offset,
                length,
                limit: self.limits.max_string_bytes,
            });
        }
        Ok(self.read_exact(length)?.to_vec())
    }

    /// Read the raw byte representation written by legacy `SaveString16`.
    pub fn read_string16_raw(&mut self) -> Result<Vec<u8>, ReaderError> {
        let length_offset = self.offset;
        let length = self.read_u16_le()? as usize;
        if length > self.limits.max_string_bytes {
            return Err(ReaderError::StringLimitExceeded {
                offset: length_offset,
                length,
                limit: self.limits.max_string_bytes,
            });
        }
        Ok(self.read_exact(length)?.to_vec())
    }

    pub fn read_blob(&mut self, length: usize) -> Result<Vec<u8>, ReaderError> {
        let offset = self.offset;
        if length > self.limits.max_blob_bytes {
            return Err(ReaderError::BlobLimitExceeded {
                offset,
                length,
                limit: self.limits.max_blob_bytes,
            });
        }
        Ok(self.read_exact(length)?.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian_primitives() {
        let bytes = [
            0x80, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12, 0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0xf0, 0x3f,
        ];
        let mut reader = LegacyReader::new(&bytes);
        assert_eq!(reader.read_i8().unwrap(), -128);
        assert_eq!(reader.read_u16_le().unwrap(), 0x1234);
        assert_eq!(reader.read_u32_le().unwrap(), 0x1234_5678);
        assert_eq!(reader.read_f32_le().unwrap(), 1.0);
        assert_eq!(reader.read_f64_le().unwrap(), 1.0);
        assert!(reader.is_eof());
    }

    #[test]
    fn preserves_raw_ansi_string_bytes() {
        let bytes = [3, 0, 0, 0, 0x41, 0x80, 0xff];
        let mut reader = LegacyReader::new(&bytes);
        assert_eq!(reader.read_string32_raw().unwrap(), vec![0x41, 0x80, 0xff]);
    }

    #[test]
    fn reports_truncation_with_offset() {
        let mut reader = LegacyReader::new(&[1, 2]);
        assert_eq!(
            reader.read_u32_le().unwrap_err(),
            ReaderError::UnexpectedEof {
                offset: 0,
                requested: 4,
                remaining: 2,
            }
        );
    }

    #[test]
    fn rejects_oversized_string_before_allocation() {
        let limits = ReaderLimits {
            max_string_bytes: 4,
            max_blob_bytes: 4,
        };
        let mut reader = LegacyReader::with_limits(&[5, 0, 0, 0], limits);
        assert!(matches!(
            reader.read_string32_raw(),
            Err(ReaderError::StringLimitExceeded { length: 5, .. })
        ));
    }
}
