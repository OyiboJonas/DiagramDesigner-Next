use std::{
    env, fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use platform_fs::{AtomicSaveError, CommitMode, DurabilityLevel, atomic_save};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tauri::{AppHandle, Manager};

use super::CommandError;

const REPORT_SCHEMA: &str = "diagramdesigner-next-adr-019-native-v1";
const EVIDENCE_DIRECTORY_NAME: &str = "benchmarks";
const ADR_DIRECTORY_NAME: &str = "adr-019";
const MAX_EVIDENCE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RendererBenchmarkEvidenceRequest {
    pub(crate) report: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RendererBenchmarkEvidenceResultDto {
    path: String,
    commit_mode: &'static str,
    durability: &'static str,
    cleanup_warning: Option<String>,
}

pub(crate) fn build_source_dirty() -> Option<bool> {
    match env!("DDN_BUILD_SOURCE_DIRTY") {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn build_source_lock_blob() -> Option<&'static str> {
    let value = env!("DDN_BUILD_SOURCE_LOCK_BLOB");
    if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(value)
    } else {
        None
    }
}

pub(crate) fn persist(
    app: &AppHandle,
    mut report: Value,
) -> Result<RendererBenchmarkEvidenceResultDto, CommandError> {
    stamp_build_provenance(&mut report)?;
    validate_report(&report)?;

    let mut bytes = serde_json::to_vec_pretty(&report).map_err(|error| {
        CommandError::new(
            "benchmark_evidence_serialize_failed",
            format!("Could not serialize ADR-019 evidence: {error}"),
        )
    })?;
    bytes.push(b'\n');
    if bytes.len() > MAX_EVIDENCE_BYTES {
        return Err(CommandError::new(
            "benchmark_evidence_too_large",
            format!(
                "ADR-019 evidence is {} bytes; maximum is {MAX_EVIDENCE_BYTES} bytes.",
                bytes.len()
            ),
        ));
    }

    let directory = evidence_directory(app)?;
    fs::create_dir_all(&directory).map_err(|error| {
        CommandError::new(
            "benchmark_evidence_directory_create_failed",
            format!("Could not create ADR-019 evidence directory: {error}"),
        )
    })?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            CommandError::new(
                "benchmark_evidence_clock_failed",
                format!("Could not timestamp ADR-019 evidence: {error}"),
            )
        })?
        .as_nanos();
    let short_commit: String = env!("DDN_BUILD_SOURCE_COMMIT").chars().take(12).collect();
    let destination = directory.join(format!("renderer-{timestamp}-{short_commit}.json"));

    let save = atomic_save(&destination, &bytes).map_err(map_evidence_save_error)?;
    Ok(RendererBenchmarkEvidenceResultDto {
        path: destination.to_string_lossy().into_owned(),
        commit_mode: match save.mode {
            CommitMode::Created => "created",
            CommitMode::Replaced => "replaced",
        },
        durability: match save.durability {
            DurabilityLevel::FileAndDirectorySynced => "file-and-directory-synced",
            DurabilityLevel::FileSyncedAndPlatformCommitFlushed => {
                "file-synced-platform-commit-flushed"
            }
        },
        cleanup_warning: save.cleanup_warning,
    })
}

fn stamp_build_provenance(report: &mut Value) -> Result<(), CommandError> {
    let root = report
        .as_object_mut()
        .ok_or_else(|| invalid_report("ADR-019 evidence root must be a JSON object."))?;

    let mut provenance = Map::new();
    provenance.insert(
        "desktopCargoLockGitBlob".to_owned(),
        build_source_lock_blob()
            .map(|value| Value::String(value.to_owned()))
            .unwrap_or(Value::Null),
    );
    root.insert("buildProvenance".to_owned(), Value::Object(provenance));
    Ok(())
}

fn validate_report(report: &Value) -> Result<(), CommandError> {
    if report.get("report").and_then(Value::as_str) != Some(REPORT_SCHEMA) {
        return Err(invalid_report("Unexpected ADR-019 report schema."));
    }
    if report.get("finalRendererDecision").and_then(Value::as_str) != Some("not-made-by-benchmark")
    {
        return Err(invalid_report(
            "ADR-019 benchmark evidence must not contain a final renderer decision.",
        ));
    }

    let environment = report
        .get("environment")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_report("ADR-019 evidence is missing its environment object."))?;
    if environment.get("sourceCommit").and_then(Value::as_str)
        != Some(env!("DDN_BUILD_SOURCE_COMMIT"))
    {
        return Err(invalid_report(
            "ADR-019 evidence source commit does not match the running build.",
        ));
    }
    match build_source_dirty() {
        Some(expected) => {
            if environment.get("sourceDirty").and_then(Value::as_bool) != Some(expected) {
                return Err(invalid_report(
                    "ADR-019 evidence dirty-source state does not match the running build.",
                ));
            }
        }
        None => {
            if !environment.get("sourceDirty").is_some_and(Value::is_null) {
                return Err(invalid_report(
                    "ADR-019 evidence must record unknown dirty-source state as null.",
                ));
            }
        }
    }

    let provenance = report
        .get("buildProvenance")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_report("ADR-019 evidence is missing native build provenance."))?;
    match build_source_lock_blob() {
        Some(expected) => {
            if provenance
                .get("desktopCargoLockGitBlob")
                .and_then(Value::as_str)
                != Some(expected)
            {
                return Err(invalid_report(
                    "ADR-019 evidence desktop Cargo.lock blob does not match the running build.",
                ));
            }
        }
        None => {
            if !provenance
                .get("desktopCargoLockGitBlob")
                .is_some_and(Value::is_null)
            {
                return Err(invalid_report(
                    "ADR-019 evidence must record unknown desktop Cargo.lock provenance as null.",
                ));
            }
        }
    }

    if report
        .get("measurements")
        .and_then(Value::as_array)
        .map(Vec::len)
        != Some(4)
    {
        return Err(invalid_report(
            "ADR-019 evidence must contain exactly four benchmark measurements.",
        ));
    }
    let verdict_status = report
        .get("performanceVerdict")
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if verdict_status.trim().is_empty() {
        return Err(invalid_report(
            "ADR-019 evidence is missing its mechanical performance verdict status.",
        ));
    }
    let generated_at = report
        .get("generatedAt")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if generated_at.trim().is_empty() {
        return Err(invalid_report(
            "ADR-019 evidence is missing its generation timestamp.",
        ));
    }

    Ok(())
}

fn evidence_directory(app: &AppHandle) -> Result<PathBuf, CommandError> {
    if let Some(configured) = env::var_os("DDN_ADR019_EVIDENCE_DIR") {
        let path = PathBuf::from(configured);
        if !path.is_absolute() || path.as_os_str().is_empty() {
            return Err(CommandError::new(
                "benchmark_evidence_directory_invalid",
                "DDN_ADR019_EVIDENCE_DIR must be a non-empty absolute path.",
            ));
        }
        return Ok(path);
    }

    let root = app.path().app_local_data_dir().map_err(|error| {
        CommandError::new(
            "benchmark_evidence_directory_unavailable",
            format!("Could not resolve the application-local ADR-019 directory: {error}"),
        )
    })?;
    Ok(root.join(EVIDENCE_DIRECTORY_NAME).join(ADR_DIRECTORY_NAME))
}

fn invalid_report(message: impl Into<String>) -> CommandError {
    CommandError::new("benchmark_evidence_invalid_report", message)
}

fn map_evidence_save_error(error: AtomicSaveError) -> CommandError {
    let code = if error.committed {
        "benchmark_evidence_durability_failed_after_commit"
    } else {
        "benchmark_evidence_atomic_save_failed"
    };
    if error.committed {
        CommandError::committed(code, error.to_string())
    } else {
        CommandError::new(code, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_report() -> Value {
        json!({
            "report": REPORT_SCHEMA,
            "environment": {
                "sourceCommit": env!("DDN_BUILD_SOURCE_COMMIT"),
                "sourceDirty": build_source_dirty(),
            },
            "measurements": [{}, {}, {}, {}],
            "performanceVerdict": { "status": "performance_gate_pass" },
            "finalRendererDecision": "not-made-by-benchmark",
            "generatedAt": "2026-08-22T12:00:00.000Z"
        })
    }

    #[test]
    fn native_stamp_records_the_build_lockfile_blob_and_validates() {
        let mut report = valid_report();
        report["buildProvenance"] = json!({
            "desktopCargoLockGitBlob": "ffffffffffffffffffffffffffffffffffffffff"
        });

        stamp_build_provenance(&mut report).unwrap();

        match build_source_lock_blob() {
            Some(expected) => assert_eq!(
                report["buildProvenance"]["desktopCargoLockGitBlob"],
                Value::String(expected.to_owned())
            ),
            None => assert!(report["buildProvenance"]["desktopCargoLockGitBlob"].is_null()),
        }
        validate_report(&report).unwrap();
    }

    #[test]
    fn validation_rejects_tampered_lockfile_provenance() {
        let mut report = valid_report();
        stamp_build_provenance(&mut report).unwrap();

        report["buildProvenance"]["desktopCargoLockGitBlob"] =
            Value::String("0000000000000000000000000000000000000000".to_owned());

        if build_source_lock_blob() == Some("0000000000000000000000000000000000000000") {
            report["buildProvenance"]["desktopCargoLockGitBlob"] =
                Value::String("1111111111111111111111111111111111111111".to_owned());
        }

        assert!(validate_report(&report).is_err());
    }
}
