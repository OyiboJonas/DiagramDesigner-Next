use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    io::{Cursor, Read, Write},
};

use sha2::{Digest, Sha256};
use thiserror::Error;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{
    DOCUMENT_PATH, MANIFEST_PATH, Manifest, ManifestError, PackageLimits, PreparedAssetBlob,
    PreparedPackage, validate_package_file_path,
};

const EOCD_SIGNATURE: u32 = 0x0605_4b50;
const ZIP64_EOCD_SIGNATURE: u32 = 0x0606_4b50;
const ZIP64_LOCATOR_SIGNATURE: u32 = 0x0706_4b50;
const EOCD_MIN_BYTES: usize = 22;
const MAX_ZIP_COMMENT_BYTES: usize = u16::MAX as usize;
const ZIP64_LOCATOR_BYTES: usize = 20;
const ZIP64_EOCD_MIN_BYTES: usize = 56;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadPackage {
    pub manifest: Manifest,
    pub document_json: Vec<u8>,
    pub asset_blobs: Vec<PreparedAssetBlob>,
}

#[derive(Debug, Error)]
pub enum PackageIoError {
    #[error("invalid prepared package: {0}")]
    InvalidPreparedPackage(String),
    #[error("ZIP archive contains {actual} entries; limit is {limit}")]
    TooManyEntries { actual: usize, limit: usize },
    #[error(
        "ZIP central directory declares {declared} entries but the ZIP reader exposes {visible}; hidden or collapsed entries are not allowed"
    )]
    ArchiveEntryCountMismatch { declared: usize, visible: usize },
    #[error("ZIP central-directory metadata is invalid: {0}")]
    InvalidCentralDirectory(String),
    #[error("multi-disk ZIP archives are not supported")]
    MultiDiskArchive,
    #[error("ZIP archive contains overlapping file payloads")]
    OverlappingEntries,
    #[error("ZIP archive contains duplicate logical entry {0:?}")]
    DuplicateEntry(String),
    #[error("ZIP archive contains unexpected entry {0:?}")]
    UnexpectedEntry(String),
    #[error("DDNX package is missing required entry {0:?}")]
    MissingEntry(String),
    #[error("DDNX entry {path:?} uses unsupported compression method {method:?}")]
    UnsupportedCompression {
        path: String,
        method: CompressionMethod,
    },
    #[error("DDNX entry {0:?} is encrypted; encrypted packages are not supported")]
    EncryptedEntry(String),
    #[error("DDNX entry {0:?} is a directory or symlink; only regular file entries are allowed")]
    NonFileEntry(String),
    #[error("DDNX entry {path:?} is {actual} bytes; limit is {limit}")]
    EntryTooLarge {
        path: String,
        actual: u64,
        limit: u64,
    },
    #[error("DDNX archive expands to {actual} bytes; limit is {limit}")]
    ArchiveTooLarge { actual: u64, limit: u64 },
    #[error("DDNX entry {path:?} size mismatch: expected {expected}, found {actual}")]
    SizeMismatch {
        path: String,
        expected: u64,
        actual: u64,
    },
    #[error("DDNX entry {path:?} SHA-256 mismatch: expected {expected}, found {actual}")]
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("DDNX manifest JSON failed to parse: {0}")]
    ManifestJson(#[source] serde_json::Error),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("ZIP I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("ZIP format error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn u64_len(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, PackageIoError> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| PackageIoError::InvalidCentralDirectory("truncated u16 field".to_owned()))?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, PackageIoError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| PackageIoError::InvalidCentralDirectory("truncated u32 field".to_owned()))?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, PackageIoError> {
    let value = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| PackageIoError::InvalidCentralDirectory("truncated u64 field".to_owned()))?;
    Ok(u64::from_le_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]))
}

fn find_eocd(bytes: &[u8]) -> Result<usize, PackageIoError> {
    if bytes.len() < EOCD_MIN_BYTES {
        return Err(PackageIoError::InvalidCentralDirectory(
            "archive is too short for an end-of-central-directory record".to_owned(),
        ));
    }

    let search_start = bytes
        .len()
        .saturating_sub(EOCD_MIN_BYTES + MAX_ZIP_COMMENT_BYTES);
    let latest_start = bytes.len() - EOCD_MIN_BYTES;

    for offset in (search_start..=latest_start).rev() {
        if read_u32(bytes, offset)? != EOCD_SIGNATURE {
            continue;
        }
        let comment_len = usize::from(read_u16(bytes, offset + 20)?);
        if offset
            .checked_add(EOCD_MIN_BYTES)
            .and_then(|end| end.checked_add(comment_len))
            == Some(bytes.len())
        {
            return Ok(offset);
        }
    }

    Err(PackageIoError::InvalidCentralDirectory(
        "end-of-central-directory record was not found at the end of the archive".to_owned(),
    ))
}

fn zip64_declared_entries(bytes: &[u8], eocd_offset: usize) -> Result<usize, PackageIoError> {
    let locator_offset = eocd_offset
        .checked_sub(ZIP64_LOCATOR_BYTES)
        .ok_or_else(|| {
            PackageIoError::InvalidCentralDirectory(
                "ZIP64 end-of-central-directory locator is missing".to_owned(),
            )
        })?;
    if read_u32(bytes, locator_offset)? != ZIP64_LOCATOR_SIGNATURE {
        return Err(PackageIoError::InvalidCentralDirectory(
            "ZIP64 end-of-central-directory locator signature is missing".to_owned(),
        ));
    }

    let zip64_disk = read_u32(bytes, locator_offset + 4)?;
    let zip64_offset = read_u64(bytes, locator_offset + 8)?;
    let total_disks = read_u32(bytes, locator_offset + 16)?;
    if zip64_disk != 0 || total_disks != 1 {
        return Err(PackageIoError::MultiDiskArchive);
    }

    let zip64_offset = usize::try_from(zip64_offset).map_err(|_| {
        PackageIoError::InvalidCentralDirectory(
            "ZIP64 end-of-central-directory offset does not fit this platform".to_owned(),
        )
    })?;
    let minimum_end = zip64_offset
        .checked_add(ZIP64_EOCD_MIN_BYTES)
        .ok_or_else(|| {
            PackageIoError::InvalidCentralDirectory(
                "ZIP64 end-of-central-directory bounds overflow".to_owned(),
            )
        })?;
    if minimum_end > bytes.len() || read_u32(bytes, zip64_offset)? != ZIP64_EOCD_SIGNATURE {
        return Err(PackageIoError::InvalidCentralDirectory(
            "ZIP64 end-of-central-directory record is missing or truncated".to_owned(),
        ));
    }

    let record_size = read_u64(bytes, zip64_offset + 4)?;
    if record_size < 44 {
        return Err(PackageIoError::InvalidCentralDirectory(
            "ZIP64 end-of-central-directory record is shorter than the fixed fields".to_owned(),
        ));
    }
    let record_size = usize::try_from(record_size).map_err(|_| {
        PackageIoError::InvalidCentralDirectory(
            "ZIP64 end-of-central-directory size does not fit this platform".to_owned(),
        )
    })?;
    let record_end = zip64_offset
        .checked_add(12)
        .and_then(|value| value.checked_add(record_size))
        .ok_or_else(|| {
            PackageIoError::InvalidCentralDirectory(
                "ZIP64 end-of-central-directory record size overflows".to_owned(),
            )
        })?;
    if record_end > locator_offset {
        return Err(PackageIoError::InvalidCentralDirectory(
            "ZIP64 end-of-central-directory record overlaps its locator".to_owned(),
        ));
    }

    let disk = read_u32(bytes, zip64_offset + 16)?;
    let central_disk = read_u32(bytes, zip64_offset + 20)?;
    let entries_on_disk = read_u64(bytes, zip64_offset + 24)?;
    let total_entries = read_u64(bytes, zip64_offset + 32)?;
    if disk != 0 || central_disk != 0 || entries_on_disk != total_entries {
        return Err(PackageIoError::MultiDiskArchive);
    }

    usize::try_from(total_entries).map_err(|_| {
        PackageIoError::InvalidCentralDirectory(
            "ZIP64 entry count does not fit this platform".to_owned(),
        )
    })
}

fn declared_archive_entries(bytes: &[u8]) -> Result<usize, PackageIoError> {
    let eocd_offset = find_eocd(bytes)?;
    let disk = read_u16(bytes, eocd_offset + 4)?;
    let central_disk = read_u16(bytes, eocd_offset + 6)?;
    let entries_on_disk = read_u16(bytes, eocd_offset + 8)?;
    let total_entries = read_u16(bytes, eocd_offset + 10)?;
    let central_size = read_u32(bytes, eocd_offset + 12)?;
    let central_offset = read_u32(bytes, eocd_offset + 16)?;

    if disk != 0 || central_disk != 0 {
        return Err(PackageIoError::MultiDiskArchive);
    }

    let uses_zip64 = entries_on_disk == u16::MAX
        || total_entries == u16::MAX
        || central_size == u32::MAX
        || central_offset == u32::MAX;
    if uses_zip64 {
        return zip64_declared_entries(bytes, eocd_offset);
    }
    if entries_on_disk != total_entries {
        return Err(PackageIoError::MultiDiskArchive);
    }

    Ok(usize::from(total_entries))
}

fn validate_prepared_package(
    prepared: &PreparedPackage,
    limits: PackageLimits,
) -> Result<(), PackageIoError> {
    prepared.manifest.validate(limits)?;

    let canonical_manifest =
        serde_json::to_vec(&prepared.manifest).map_err(PackageIoError::ManifestJson)?;
    if canonical_manifest != prepared.manifest_json {
        return Err(PackageIoError::InvalidPreparedPackage(
            "manifest_json does not match the manifest value".to_owned(),
        ));
    }
    if u64_len(prepared.manifest_json.len()) > limits.max_manifest_bytes {
        return Err(PackageIoError::EntryTooLarge {
            path: MANIFEST_PATH.to_owned(),
            actual: u64_len(prepared.manifest_json.len()),
            limit: limits.max_manifest_bytes,
        });
    }

    let document_bytes = u64_len(prepared.document_json.len());
    if document_bytes != prepared.manifest.document_bytes {
        return Err(PackageIoError::SizeMismatch {
            path: DOCUMENT_PATH.to_owned(),
            expected: prepared.manifest.document_bytes,
            actual: document_bytes,
        });
    }
    let document_hash = sha256_hex(&prepared.document_json);
    if document_hash != prepared.manifest.document_sha256 {
        return Err(PackageIoError::HashMismatch {
            path: DOCUMENT_PATH.to_owned(),
            expected: prepared.manifest.document_sha256.clone(),
            actual: document_hash,
        });
    }

    if prepared.asset_blobs.len() != prepared.manifest.assets.len() {
        return Err(PackageIoError::InvalidPreparedPackage(format!(
            "manifest describes {} assets but {} blobs were prepared",
            prepared.manifest.assets.len(),
            prepared.asset_blobs.len()
        )));
    }

    let blobs_by_content_hash: HashMap<_, _> = prepared
        .asset_blobs
        .iter()
        .map(|blob| (blob.content_sha256.as_str(), blob))
        .collect();
    if blobs_by_content_hash.len() != prepared.asset_blobs.len() {
        return Err(PackageIoError::InvalidPreparedPackage(
            "prepared asset blobs contain duplicate content hashes".to_owned(),
        ));
    }

    for asset in &prepared.manifest.assets {
        let blob = blobs_by_content_hash
            .get(asset.content_sha256.as_str())
            .ok_or_else(|| {
                PackageIoError::InvalidPreparedPackage(format!(
                    "missing prepared blob for asset {}",
                    asset.content_sha256
                ))
            })?;
        if blob.id != asset.id
            || blob.path != asset.path
            || blob.media_type != asset.media_type
            || blob.blob_sha256 != asset.blob_sha256
        {
            return Err(PackageIoError::InvalidPreparedPackage(format!(
                "prepared blob metadata does not match manifest asset {}",
                asset.content_sha256
            )));
        }
        let actual_bytes = u64_len(blob.bytes.len());
        if actual_bytes != asset.bytes {
            return Err(PackageIoError::SizeMismatch {
                path: asset.path.clone(),
                expected: asset.bytes,
                actual: actual_bytes,
            });
        }
        let actual_hash = sha256_hex(&blob.bytes);
        if actual_hash != asset.blob_sha256 {
            return Err(PackageIoError::HashMismatch {
                path: asset.path.clone(),
                expected: asset.blob_sha256.clone(),
                actual: actual_hash,
            });
        }
    }

    let total = prepared.asset_blobs.iter().try_fold(
        u64_len(prepared.manifest_json.len())
            .checked_add(document_bytes)
            .ok_or(PackageIoError::ArchiveTooLarge {
                actual: u64::MAX,
                limit: limits.max_total_uncompressed_bytes,
            })?,
        |total, blob| {
            total
                .checked_add(u64_len(blob.bytes.len()))
                .ok_or(PackageIoError::ArchiveTooLarge {
                    actual: u64::MAX,
                    limit: limits.max_total_uncompressed_bytes,
                })
        },
    )?;
    if total > limits.max_total_uncompressed_bytes {
        return Err(PackageIoError::ArchiveTooLarge {
            actual: total,
            limit: limits.max_total_uncompressed_bytes,
        });
    }

    Ok(())
}

fn deterministic_options() -> SimpleFileOptions {
    SimpleFileOptions::DEFAULT
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644)
}

/// Write a fully prepared DDNX package to deterministic ZIP bytes.
///
/// DDNX v1 deliberately uses ZIP `Stored` entries only. This avoids hidden
/// compressor variability and keeps the package byte stream reproducible.
pub fn write_package_to_vec(
    prepared: &PreparedPackage,
    limits: PackageLimits,
) -> Result<Vec<u8>, PackageIoError> {
    validate_prepared_package(prepared, limits)?;

    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    let options = deterministic_options();

    writer.start_file(MANIFEST_PATH, options)?;
    writer.write_all(&prepared.manifest_json)?;
    writer.start_file(DOCUMENT_PATH, options)?;
    writer.write_all(&prepared.document_json)?;

    let mut blobs: Vec<_> = prepared.asset_blobs.iter().collect();
    blobs.sort_by(|left, right| left.path.cmp(&right.path));
    for blob in blobs {
        writer.start_file(&blob.path, options)?;
        writer.write_all(&blob.bytes)?;
    }

    Ok(writer.finish()?.into_inner())
}

fn scan_archive(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    limits: PackageLimits,
) -> Result<BTreeMap<String, usize>, PackageIoError> {
    if archive.len() > limits.max_entries {
        return Err(PackageIoError::TooManyEntries {
            actual: archive.len(),
            limit: limits.max_entries,
        });
    }
    if archive.has_overlapping_files()? {
        return Err(PackageIoError::OverlappingEntries);
    }

    let mut indices = BTreeMap::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        let name = file.name().to_owned();
        validate_package_file_path(&name)?;
        if file.compression() != CompressionMethod::Stored {
            return Err(PackageIoError::UnsupportedCompression {
                path: name,
                method: file.compression(),
            });
        }
        if file.encrypted() {
            return Err(PackageIoError::EncryptedEntry(name));
        }
        if file.is_dir() || file.is_symlink() {
            return Err(PackageIoError::NonFileEntry(name));
        }
        total = total
            .checked_add(file.size())
            .ok_or(PackageIoError::ArchiveTooLarge {
                actual: u64::MAX,
                limit: limits.max_total_uncompressed_bytes,
            })?;
        if total > limits.max_total_uncompressed_bytes {
            return Err(PackageIoError::ArchiveTooLarge {
                actual: total,
                limit: limits.max_total_uncompressed_bytes,
            });
        }
        if indices.insert(name.clone(), index).is_some() {
            return Err(PackageIoError::DuplicateEntry(name));
        }
    }
    Ok(indices)
}

fn read_entry(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    index: usize,
    path: &str,
    limit: u64,
    expected_size: Option<u64>,
) -> Result<Vec<u8>, PackageIoError> {
    let file = archive.by_index(index)?;
    let declared_size = file.size();
    if declared_size > limit {
        return Err(PackageIoError::EntryTooLarge {
            path: path.to_owned(),
            actual: declared_size,
            limit,
        });
    }
    if let Some(expected) = expected_size {
        if declared_size != expected {
            return Err(PackageIoError::SizeMismatch {
                path: path.to_owned(),
                expected,
                actual: declared_size,
            });
        }
    }

    let mut limited = file.take(limit.saturating_add(1));
    let mut bytes = Vec::with_capacity(usize::try_from(declared_size).unwrap_or(0));
    limited.read_to_end(&mut bytes)?;
    let actual = u64_len(bytes.len());
    if actual > limit {
        return Err(PackageIoError::EntryTooLarge {
            path: path.to_owned(),
            actual,
            limit,
        });
    }
    if actual != declared_size {
        return Err(PackageIoError::SizeMismatch {
            path: path.to_owned(),
            expected: declared_size,
            actual,
        });
    }
    Ok(bytes)
}

fn require_index(indices: &BTreeMap<String, usize>, path: &str) -> Result<usize, PackageIoError> {
    indices
        .get(path)
        .copied()
        .ok_or_else(|| PackageIoError::MissingEntry(path.to_owned()))
}

fn verify_hash(path: &str, bytes: &[u8], expected: &str) -> Result<(), PackageIoError> {
    let actual = sha256_hex(bytes);
    if actual != expected {
        return Err(PackageIoError::HashMismatch {
            path: path.to_owned(),
            expected: expected.to_owned(),
            actual,
        });
    }
    Ok(())
}

/// Read and integrity-check a DDNX ZIP package without hydrating it into the
/// in-memory editor domain yet. No archive entry is extracted to the filesystem.
pub fn read_package(bytes: &[u8], limits: PackageLimits) -> Result<ReadPackage, PackageIoError> {
    let declared_entries = declared_archive_entries(bytes)?;
    if declared_entries > limits.max_entries {
        return Err(PackageIoError::TooManyEntries {
            actual: declared_entries,
            limit: limits.max_entries,
        });
    }

    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    let visible_entries = archive.len();
    if visible_entries != declared_entries {
        return Err(PackageIoError::ArchiveEntryCountMismatch {
            declared: declared_entries,
            visible: visible_entries,
        });
    }
    let indices = scan_archive(&mut archive, limits)?;

    let manifest_index = require_index(&indices, MANIFEST_PATH)?;
    let manifest_bytes = read_entry(
        &mut archive,
        manifest_index,
        MANIFEST_PATH,
        limits.max_manifest_bytes,
        None,
    )?;
    let manifest: Manifest =
        serde_json::from_slice(&manifest_bytes).map_err(PackageIoError::ManifestJson)?;
    manifest.validate(limits)?;

    let mut expected_paths =
        BTreeSet::from([MANIFEST_PATH.to_owned(), manifest.document_path.clone()]);
    expected_paths.extend(manifest.assets.iter().map(|asset| asset.path.clone()));
    expected_paths.extend(manifest.previews.iter().map(|preview| preview.path.clone()));
    let actual_paths: BTreeSet<_> = indices.keys().cloned().collect();

    if let Some(path) = actual_paths.difference(&expected_paths).next() {
        return Err(PackageIoError::UnexpectedEntry(path.clone()));
    }
    if let Some(path) = expected_paths.difference(&actual_paths).next() {
        return Err(PackageIoError::MissingEntry(path.clone()));
    }

    let document_index = require_index(&indices, &manifest.document_path)?;
    let document_json = read_entry(
        &mut archive,
        document_index,
        &manifest.document_path,
        limits.max_document_bytes,
        Some(manifest.document_bytes),
    )?;
    verify_hash(
        &manifest.document_path,
        &document_json,
        &manifest.document_sha256,
    )?;

    let mut asset_blobs = Vec::with_capacity(manifest.assets.len());
    for asset in &manifest.assets {
        let index = require_index(&indices, &asset.path)?;
        let blob_bytes = read_entry(
            &mut archive,
            index,
            &asset.path,
            limits.max_asset_bytes,
            Some(asset.bytes),
        )?;
        verify_hash(&asset.path, &blob_bytes, &asset.blob_sha256)?;
        asset_blobs.push(PreparedAssetBlob {
            id: asset.id,
            content_sha256: asset.content_sha256.clone(),
            blob_sha256: asset.blob_sha256.clone(),
            path: asset.path.clone(),
            media_type: asset.media_type.clone(),
            bytes: blob_bytes,
        });
    }

    for preview in &manifest.previews {
        let index = require_index(&indices, &preview.path)?;
        let preview_bytes = read_entry(
            &mut archive,
            index,
            &preview.path,
            limits.max_asset_bytes,
            Some(preview.bytes),
        )?;
        verify_hash(&preview.path, &preview_bytes, &preview.sha256)?;
    }

    Ok(ReadPackage {
        manifest,
        document_json,
        asset_blobs,
    })
}

#[cfg(test)]
mod tests {
    use next_domain::{
        Asset, AssetId, AssetPayload, NextArtifact, Scene, Size, TemplateId, TemplatePalette,
    };

    use super::*;
    use crate::prepare_package;

    const CONTENT_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn prepared_fixture() -> PreparedPackage {
        let artifact = NextArtifact::template_palette(TemplatePalette {
            id: TemplateId::new(),
            name: "DDNX test".to_owned(),
            size_mm: Size {
                width: 10.0,
                height: 20.0,
            },
            scene: Scene::default(),
            styles: Vec::new(),
            assets: vec![Asset {
                id: AssetId::new(),
                sha256: CONTENT_HASH.to_owned(),
                media_type: "application/octet-stream".to_owned(),
                payload: AssetPayload::Binary {
                    bytes: vec![1, 2, 3, 4],
                },
            }],
            import: None,
        });
        prepare_package(&artifact, PackageLimits::default()).unwrap()
    }

    #[test]
    fn deterministic_writer_produces_identical_zip_bytes() {
        let prepared = prepared_fixture();
        let first = write_package_to_vec(&prepared, PackageLimits::default()).unwrap();
        let second = write_package_to_vec(&prepared, PackageLimits::default()).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn reads_and_verifies_written_package() {
        let prepared = prepared_fixture();
        let zip = write_package_to_vec(&prepared, PackageLimits::default()).unwrap();
        let read = read_package(&zip, PackageLimits::default()).unwrap();
        assert_eq!(read.manifest, prepared.manifest);
        assert_eq!(read.document_json, prepared.document_json);
        assert_eq!(read.asset_blobs, prepared.asset_blobs);
    }

    #[test]
    fn rejects_document_hash_corruption_declared_by_manifest() {
        let mut prepared = prepared_fixture();
        prepared.manifest.document_sha256 =
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned();
        prepared.manifest_json = serde_json::to_vec(&prepared.manifest).unwrap();
        let error = write_package_to_vec(&prepared, PackageLimits::default()).unwrap_err();
        assert!(matches!(error, PackageIoError::HashMismatch { .. }));
    }

    #[test]
    fn rejects_unexpected_zip_entry() {
        let prepared = prepared_fixture();
        let zip = write_package_to_vec(&prepared, PackageLimits::default()).unwrap();
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = deterministic_options();
        writer.start_file(MANIFEST_PATH, options).unwrap();
        writer.write_all(&prepared.manifest_json).unwrap();
        writer.start_file(DOCUMENT_PATH, options).unwrap();
        writer.write_all(&prepared.document_json).unwrap();
        for blob in &prepared.asset_blobs {
            writer.start_file(&blob.path, options).unwrap();
            writer.write_all(&blob.bytes).unwrap();
        }
        writer.start_file("unexpected.bin", options).unwrap();
        writer.write_all(b"unexpected").unwrap();
        let modified = writer.finish().unwrap().into_inner();
        assert_ne!(zip, modified);
        let error = read_package(&modified, PackageLimits::default()).unwrap_err();
        assert!(matches!(error, PackageIoError::UnexpectedEntry(_)));
    }
}
