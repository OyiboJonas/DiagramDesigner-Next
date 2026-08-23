use std::io::{Cursor, Write};

use ddnx::{MANIFEST_PATH, ManifestError, PackageIoError, PackageLimits, read_package};
use zip::{CompressionMethod, DateTime, ZipWriter, write::SimpleFileOptions};

fn options() -> SimpleFileOptions {
    SimpleFileOptions::DEFAULT
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644)
}

fn zip_with_entries(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    for (name, bytes) in entries {
        writer.start_file(*name, options()).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

// Generated once with Python's standard-library zipfile writer, not with the
// Rust `zip` crate used by production code. It contains two stored regular-file
// entries both named `manifest.json`, each containing `{}`. Keeping the foreign
// bytes inline makes this adversarial input deterministic and independent of
// `ZipWriter`, which deliberately refuses duplicate names itself.
const FOREIGN_DUPLICATE_NAME_ZIP: &[u8] = &[
    80, 75, 3, 4, 20, 0, 0, 0, 0, 0, 9, 129, 19, 93, 67, 191, 166, 163, 2, 0, 0, 0, 2, 0, 0, 0, 13,
    0, 0, 0, 109, 97, 110, 105, 102, 101, 115, 116, 46, 106, 115, 111, 110, 123, 125, 80, 75, 3, 4,
    20, 0, 0, 0, 0, 0, 9, 129, 19, 93, 67, 191, 166, 163, 2, 0, 0, 0, 2, 0, 0, 0, 13, 0, 0, 0, 109,
    97, 110, 105, 102, 101, 115, 116, 46, 106, 115, 111, 110, 123, 125, 80, 75, 1, 2, 20, 3, 20, 0,
    0, 0, 0, 0, 9, 129, 19, 93, 67, 191, 166, 163, 2, 0, 0, 0, 2, 0, 0, 0, 13, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 128, 1, 0, 0, 0, 0, 109, 97, 110, 105, 102, 101, 115, 116, 46, 106, 115, 111, 110,
    80, 75, 1, 2, 20, 3, 20, 0, 0, 0, 0, 0, 9, 129, 19, 93, 67, 191, 166, 163, 2, 0, 0, 0, 2, 0, 0,
    0, 13, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 1, 45, 0, 0, 0, 109, 97, 110, 105, 102, 101, 115,
    116, 46, 106, 115, 111, 110, 80, 75, 5, 6, 0, 0, 0, 0, 2, 0, 2, 0, 118, 0, 0, 0, 90, 0, 0, 0,
    0, 0,
];

#[test]
fn rejects_malformed_manifest_json_before_document_loading() {
    let archive = zip_with_entries(&[
        (MANIFEST_PATH, br#"{"format":"ddnx""#),
        ("document.json", b"{}"),
    ]);
    let error = read_package(&archive, PackageLimits::default()).unwrap_err();
    assert!(matches!(error, PackageIoError::ManifestJson(_)));
}

#[test]
fn zip_writer_refuses_duplicate_logical_names() {
    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    writer.start_file(MANIFEST_PATH, options()).unwrap();
    writer.write_all(b"{}").unwrap();
    let error = writer.start_file(MANIFEST_PATH, options()).unwrap_err();
    assert!(matches!(error, zip::result::ZipError::InvalidArchive(_)));
}

#[test]
fn reader_rejects_duplicate_names_in_foreign_archive() {
    let error = read_package(FOREIGN_DUPLICATE_NAME_ZIP, PackageLimits::default()).unwrap_err();
    assert!(matches!(
        error,
        PackageIoError::ArchiveEntryCountMismatch {
            declared: 2,
            visible: 1
        }
    ));
}

#[test]
fn rejects_path_traversal_during_archive_scan() {
    let archive = zip_with_entries(&[("../manifest.json", b"{}")]);
    let error = read_package(&archive, PackageLimits::default()).unwrap_err();
    assert!(matches!(
        error,
        PackageIoError::Manifest(ManifestError::InvalidPackagePath(path))
            if path == "../manifest.json"
    ));
}

#[test]
fn rejects_manifest_entry_over_its_specific_limit() {
    let archive = zip_with_entries(&[(MANIFEST_PATH, &[b'x'; 32])]);
    let limits = PackageLimits {
        max_manifest_bytes: 8,
        ..PackageLimits::default()
    };
    let error = read_package(&archive, limits).unwrap_err();
    assert!(matches!(
        error,
        PackageIoError::EntryTooLarge {
            path,
            actual: 32,
            limit: 8
        } if path == MANIFEST_PATH
    ));
}

#[test]
fn rejects_archive_over_total_uncompressed_limit_before_payload_read() {
    let archive = zip_with_entries(&[(MANIFEST_PATH, &[b'a'; 8]), ("document.json", &[b'b'; 8])]);
    let limits = PackageLimits {
        max_total_uncompressed_bytes: 10,
        ..PackageLimits::default()
    };
    let error = read_package(&archive, limits).unwrap_err();
    assert!(matches!(
        error,
        PackageIoError::ArchiveTooLarge {
            actual: 16,
            limit: 10
        }
    ));
}

#[test]
fn rejects_archive_over_entry_count_limit() {
    let archive = zip_with_entries(&[(MANIFEST_PATH, b"{}"), ("document.json", b"{}")]);
    let limits = PackageLimits {
        max_entries: 1,
        ..PackageLimits::default()
    };
    let error = read_package(&archive, limits).unwrap_err();
    assert!(matches!(
        error,
        PackageIoError::TooManyEntries {
            actual: 2,
            limit: 1
        }
    ));
}
