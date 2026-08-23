use std::{
    fmt::Write as _,
    fs::{self, OpenOptions},
    io::Write as IoWrite,
    path::{Path, PathBuf},
    process,
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use ddnx::{
    PackageLimits, compare_persistence, prepare_package, read_package, write_package_to_vec,
};
use legacy_ddd::{
    DEFAULT_MAX_INFLATED_BYTES, decode_document,
    encoding::LegacyEncoding,
    inspect_document,
    text_normalization::{TextNormalizationSummary, normalize_document_text},
};
use legacy_migrate::{MigrationOptions, migrate_bytes};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Parser)]
#[command(name = "dd-migrate")]
#[command(about = "DiagramDesigner Next legacy migration laboratory")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Fully traverse a DDD/DDT file and print compact deterministic diagnostics.
    Inspect {
        file: PathBuf,
        #[arg(long, default_value_t = DEFAULT_MAX_INFLATED_BYTES)]
        max_inflated_bytes: usize,
    },
    /// Decode a DDD/DDT file into the explicit Phase 0 legacy intermediate JSON.
    Decode {
        file: PathBuf,
        #[arg(long, default_value_t = DEFAULT_MAX_INFLATED_BYTES)]
        max_inflated_bytes: usize,
    },
    /// Decode all textual fields to Unicode and parse legacy text markup.
    Text {
        file: PathBuf,
        /// Explicit fallback for DEFAULT/SYMBOL/OEM/unknown legacy charsets and DDT files.
        #[arg(long, default_value = "windows-1252")]
        fallback_encoding: LegacyEncoding,
        #[arg(long, default_value_t = DEFAULT_MAX_INFLATED_BYTES)]
        max_inflated_bytes: usize,
    },
    /// Convert a legacy DDD/DDT file into Next JSON or a native DDNX package.
    Convert {
        file: PathBuf,
        /// Write a native `.ddnx` package instead of printing Next JSON to stdout.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Explicit fallback for DEFAULT/SYMBOL/OEM/unknown legacy charsets and DDT files.
        #[arg(long, default_value = "windows-1252")]
        fallback_encoding: LegacyEncoding,
        #[arg(long, default_value_t = DEFAULT_MAX_INFLATED_BYTES)]
        max_inflated_bytes: usize,
    },
    /// Read, integrity-check and hydrate a native DDNX package.
    VerifyDdnx { file: PathBuf },
    /// Verify external/private fixtures without committing their binaries to GitHub.
    VerifyCorpus {
        /// JSON manifest containing relative/absolute fixture paths and pinned hashes.
        manifest: PathBuf,
        /// Explicit fallback for DEFAULT/SYMBOL/OEM/unknown legacy charsets and DDT files.
        #[arg(long, default_value = "windows-1252")]
        fallback_encoding: LegacyEncoding,
        #[arg(long, default_value_t = DEFAULT_MAX_INFLATED_BYTES)]
        max_inflated_bytes: usize,
    },
}

#[derive(Debug, Deserialize)]
struct CorpusManifest {
    entries: Vec<CorpusEntry>,
}

#[derive(Debug, Deserialize)]
struct CorpusEntry {
    name: String,
    path: PathBuf,
    source_sha256: String,
    #[serde(default)]
    next_sha256: Option<String>,
    /// Optional reviewed text-normalization fingerprint. This lets private files
    /// participate in deterministic text regression without entering the repository.
    #[serde(default)]
    text: Option<CorpusTextExpectation>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct CorpusTextExpectation {
    entries: usize,
    object_text_entries: usize,
    decode_error_entries: usize,
    markup_diagnostics: usize,
    symbol_glyphs: usize,
    action_tails: usize,
    hint_tails: usize,
}

fn read_file(file: &Path) -> Result<Vec<u8>> {
    fs::read(file).with_context(|| format!("failed to read {}", file.display()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn atomic_write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        bail!(
            "refusing to overwrite existing output file {}",
            path.display()
        );
    }

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        bail!("output directory does not exist: {}", parent.display());
    }
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow::anyhow!("output path has no valid UTF-8 file name"))?;

    let mut opened = None;
    for nonce in 0..100u32 {
        let candidate = parent.join(format!(".{file_name}.{}.{}.tmp", process::id(), nonce));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                opened = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to create temporary file in {}", parent.display())
                });
            }
        }
    }

    let Some((temporary_path, mut temporary_file)) = opened else {
        bail!(
            "failed to allocate a unique temporary file in {}",
            parent.display()
        );
    };

    if let Err(error) = temporary_file
        .write_all(bytes)
        .and_then(|()| temporary_file.sync_all())
    {
        drop(temporary_file);
        let _ = fs::remove_file(&temporary_path);
        return Err(error).with_context(|| {
            format!(
                "failed to write temporary DDNX file {}",
                temporary_path.display()
            )
        });
    }
    drop(temporary_file);

    if path.exists() {
        let _ = fs::remove_file(&temporary_path);
        bail!(
            "output file appeared while writing; refusing to overwrite {}",
            path.display()
        );
    }
    if let Err(error) = fs::rename(&temporary_path, path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(error).with_context(|| {
            format!(
                "failed to atomically move temporary DDNX file to {}",
                path.display()
            )
        });
    }

    Ok(())
}

fn write_ddnx(path: &Path, artifact: &next_domain::NextArtifact) -> Result<()> {
    let is_ddnx = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("ddnx"));
    if !is_ddnx {
        bail!("native output path must use the .ddnx extension");
    }

    let limits = PackageLimits::default();
    let prepared = prepare_package(artifact, limits).context("failed to prepare DDNX package")?;
    let package_bytes =
        write_package_to_vec(&prepared, limits).context("failed to serialize DDNX package")?;

    // Verify the exact bytes that will be committed to disk before the atomic rename.
    let hydrated = read_package(&package_bytes, limits)
        .context("self-verification could not read generated DDNX package")?
        .into_artifact()
        .context("self-verification could not hydrate generated DDNX package")?;
    let comparison = compare_persistence(artifact, &hydrated)?;
    if !comparison.equivalent {
        let detail = comparison
            .first_difference
            .unwrap_or_else(|| "unknown persistence divergence".to_owned());
        bail!("generated DDNX package did not round-trip safely: {detail}");
    }

    atomic_write_new(path, &package_bytes)?;
    println!(
        "WROTE\t{}\tbytes={}\tsha256={}",
        path.display(),
        package_bytes.len(),
        sha256_hex(&package_bytes)
    );
    Ok(())
}

fn verify_ddnx(path: &Path) -> Result<()> {
    let bytes = read_file(path)?;
    let limits = PackageLimits::default();
    let package = read_package(&bytes, limits)
        .with_context(|| format!("failed to verify DDNX container {}", path.display()))?;
    let manifest = package.manifest.clone();
    let artifact = package
        .into_artifact()
        .with_context(|| format!("failed to hydrate DDNX package {}", path.display()))?;
    let canonical_next = serde_json::to_vec(&artifact)?;

    println!(
        "PASS\t{}\tpackage_sha256={}\tnext_sha256={}\tassets={}",
        path.display(),
        sha256_hex(&bytes),
        sha256_hex(&canonical_next),
        manifest.assets.len()
    );
    Ok(())
}

fn resolve_fixture_path(manifest_path: &Path, fixture_path: &Path) -> PathBuf {
    if fixture_path.is_absolute() {
        fixture_path.to_owned()
    } else {
        manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(fixture_path)
    }
}

fn text_expectation_matches(
    expected: &CorpusTextExpectation,
    actual: &TextNormalizationSummary,
) -> bool {
    expected.entries == actual.entries
        && expected.object_text_entries == actual.object_text_entries
        && expected.decode_error_entries == actual.decode_error_entries
        && expected.markup_diagnostics == actual.markup_diagnostics
        && expected.symbol_glyphs == actual.symbol_glyphs
        && expected.action_tails == actual.action_tails
        && expected.hint_tails == actual.hint_tails
}

fn format_text_summary(summary: &TextNormalizationSummary) -> String {
    format!(
        "entries={} object-text={} decode-errors={} markup-diagnostics={} symbol-glyphs={} action-tails={} hint-tails={}",
        summary.entries,
        summary.object_text_entries,
        summary.decode_error_entries,
        summary.markup_diagnostics,
        summary.symbol_glyphs,
        summary.action_tails,
        summary.hint_tails
    )
}

fn verify_corpus(
    manifest_path: &Path,
    fallback_encoding: LegacyEncoding,
    max_inflated_bytes: usize,
) -> Result<()> {
    let manifest_bytes = read_file(manifest_path)?;
    let manifest: CorpusManifest = serde_json::from_slice(&manifest_bytes).with_context(|| {
        format!(
            "failed to parse corpus manifest {}",
            manifest_path.display()
        )
    })?;

    if manifest.entries.is_empty() {
        bail!("corpus manifest contains no entries");
    }

    let mut failures = Vec::new();
    for entry in manifest.entries {
        let fixture_path = resolve_fixture_path(manifest_path, &entry.path);
        let bytes = match read_file(&fixture_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                failures.push(format!("{}: {error:#}", entry.name));
                continue;
            }
        };

        let source_sha256 = sha256_hex(&bytes);
        if !source_sha256.eq_ignore_ascii_case(&entry.source_sha256) {
            failures.push(format!(
                "{}: source SHA-256 mismatch: expected {}, found {}",
                entry.name, entry.source_sha256, source_sha256
            ));
            continue;
        }

        if let Err(error) = inspect_document(&bytes, max_inflated_bytes) {
            failures.push(format!("{}: inspection failed: {error:#}", entry.name));
            continue;
        }

        let decoded = match decode_document(&bytes, max_inflated_bytes) {
            Ok(decoded) => decoded,
            Err(error) => {
                failures.push(format!("{}: text decode failed: {error:#}", entry.name));
                continue;
            }
        };
        let text_report = normalize_document_text(&decoded, fallback_encoding);
        if let Some(expected) = &entry.text {
            if !text_expectation_matches(expected, &text_report.summary) {
                failures.push(format!(
                    "{}: text normalization summary mismatch: found {}",
                    entry.name,
                    format_text_summary(&text_report.summary)
                ));
                continue;
            }
        }

        let artifact = match migrate_bytes(
            &bytes,
            max_inflated_bytes,
            MigrationOptions { fallback_encoding },
        ) {
            Ok(artifact) => artifact,
            Err(error) => {
                failures.push(format!("{}: conversion failed: {error:#}", entry.name));
                continue;
            }
        };

        let canonical_json = serde_json::to_vec(&artifact).with_context(|| {
            format!("failed to serialize converted artifact for {}", entry.name)
        })?;
        let next_sha256 = sha256_hex(&canonical_json);

        if let Some(expected) = &entry.next_sha256 {
            if !next_sha256.eq_ignore_ascii_case(expected) {
                failures.push(format!(
                    "{}: Next JSON SHA-256 mismatch: expected {}, found {}",
                    entry.name, expected, next_sha256
                ));
                continue;
            }
        }

        println!(
            "PASS\t{}\tsource={}\tnext={}\ttext={}",
            entry.name,
            source_sha256,
            next_sha256,
            format_text_summary(&text_report.summary)
        );
    }

    if !failures.is_empty() {
        for failure in &failures {
            eprintln!("FAIL\t{failure}");
        }
        bail!("{} corpus fixture(s) failed verification", failures.len());
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Inspect {
            file,
            max_inflated_bytes,
        } => {
            let bytes = read_file(&file)?;
            let report = inspect_document(&bytes, max_inflated_bytes)
                .with_context(|| format!("failed to inspect {}", file.display()))?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Decode {
            file,
            max_inflated_bytes,
        } => {
            let bytes = read_file(&file)?;
            let document = decode_document(&bytes, max_inflated_bytes)
                .with_context(|| format!("failed to decode {}", file.display()))?;
            println!("{}", serde_json::to_string_pretty(&document)?);
        }
        Command::Text {
            file,
            fallback_encoding,
            max_inflated_bytes,
        } => {
            let bytes = read_file(&file)?;
            let document = decode_document(&bytes, max_inflated_bytes)
                .with_context(|| format!("failed to decode {}", file.display()))?;
            let report = normalize_document_text(&document, fallback_encoding);
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Convert {
            file,
            output,
            fallback_encoding,
            max_inflated_bytes,
        } => {
            let bytes = read_file(&file)?;
            let artifact = migrate_bytes(
                &bytes,
                max_inflated_bytes,
                MigrationOptions { fallback_encoding },
            )
            .with_context(|| format!("failed to convert {}", file.display()))?;
            if let Some(output) = output {
                write_ddnx(&output, &artifact)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&artifact)?);
            }
        }
        Command::VerifyDdnx { file } => verify_ddnx(&file)?,
        Command::VerifyCorpus {
            manifest,
            fallback_encoding,
            max_inflated_bytes,
        } => verify_corpus(&manifest, fallback_encoding, max_inflated_bytes)?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary() -> TextNormalizationSummary {
        TextNormalizationSummary {
            fallback_encoding: LegacyEncoding::Windows1252,
            entries: 10,
            object_text_entries: 4,
            decode_error_entries: 0,
            markup_diagnostics: 0,
            symbol_glyphs: 2,
            action_tails: 1,
            hint_tails: 0,
        }
    }

    #[test]
    fn corpus_text_expectation_matches_all_semantic_counts() {
        let expected = CorpusTextExpectation {
            entries: 10,
            object_text_entries: 4,
            decode_error_entries: 0,
            markup_diagnostics: 0,
            symbol_glyphs: 2,
            action_tails: 1,
            hint_tails: 0,
        };
        assert!(text_expectation_matches(&expected, &summary()));
    }

    #[test]
    fn corpus_text_expectation_rejects_a_single_changed_count() {
        let expected = CorpusTextExpectation {
            entries: 10,
            object_text_entries: 4,
            decode_error_entries: 0,
            markup_diagnostics: 0,
            symbol_glyphs: 1,
            action_tails: 1,
            hint_tails: 0,
        };
        assert!(!text_expectation_matches(&expected, &summary()));
    }
}
