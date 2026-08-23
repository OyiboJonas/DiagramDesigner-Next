use ddnx::{
    ArtifactKind, DOCUMENT_VERSION, Manifest, ManifestError, PACKAGE_VERSION, PackageLimits,
};
use next_domain::SCHEMA_VERSION;

const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn manifest() -> Manifest {
    Manifest::new(ArtifactKind::Document, HASH.to_owned(), 0)
}

#[test]
fn accepts_exact_current_version_triple() {
    let manifest = manifest();
    assert_eq!(manifest.package_version, PACKAGE_VERSION);
    assert_eq!(manifest.document_version, DOCUMENT_VERSION);
    assert_eq!(manifest.next_schema_version, SCHEMA_VERSION);
    assert!(manifest.validate(PackageLimits::default()).is_ok());
}

#[test]
fn rejects_older_and_newer_package_versions() {
    for actual in [PACKAGE_VERSION.saturating_sub(1), PACKAGE_VERSION + 1] {
        if actual == PACKAGE_VERSION {
            continue;
        }
        let mut manifest = manifest();
        manifest.package_version = actual;
        assert!(matches!(
            manifest.validate(PackageLimits::default()),
            Err(ManifestError::UnsupportedPackageVersion {
                expected,
                actual: found,
            }) if expected == PACKAGE_VERSION && found == actual
        ));
    }
}

#[test]
fn rejects_older_and_newer_document_projection_versions() {
    for actual in [DOCUMENT_VERSION.saturating_sub(1), DOCUMENT_VERSION + 1] {
        if actual == DOCUMENT_VERSION {
            continue;
        }
        let mut manifest = manifest();
        manifest.document_version = actual;
        assert!(matches!(
            manifest.validate(PackageLimits::default()),
            Err(ManifestError::UnsupportedDocumentVersion {
                expected,
                actual: found,
            }) if expected == DOCUMENT_VERSION && found == actual
        ));
    }
}

#[test]
fn rejects_older_and_newer_next_schema_versions() {
    for actual in [SCHEMA_VERSION.saturating_sub(1), SCHEMA_VERSION + 1] {
        if actual == SCHEMA_VERSION {
            continue;
        }
        let mut manifest = manifest();
        manifest.next_schema_version = actual;
        assert!(matches!(
            manifest.validate(PackageLimits::default()),
            Err(ManifestError::UnsupportedNextSchemaVersion {
                expected,
                actual: found,
            }) if expected == SCHEMA_VERSION && found == actual
        ));
    }
}
