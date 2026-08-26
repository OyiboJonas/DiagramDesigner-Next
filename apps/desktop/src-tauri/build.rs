use std::{env, process::Command};

fn main() {
    println!("cargo:rerun-if-env-changed=DDN_SOURCE_COMMIT");
    println!("cargo:rerun-if-env-changed=DDN_SOURCE_DIRTY");
    println!("cargo:rerun-if-env-changed=DDN_SOURCE_LOCK_BLOB");

    let source_commit = resolve_source_commit();
    let source_dirty = resolve_source_dirty();
    let source_lock_blob = resolve_source_lock_blob();
    println!("cargo:rustc-env=DDN_BUILD_SOURCE_COMMIT={source_commit}");
    println!("cargo:rustc-env=DDN_BUILD_SOURCE_DIRTY={source_dirty}");
    println!("cargo:rustc-env=DDN_BUILD_SOURCE_LOCK_BLOB={source_lock_blob}");

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "document_state",
            "document_navigation",
            "activate_page",
            "activate_layer",
            "create_page",
            "delete_page",
            "update_page_properties",
            "create_layer",
            "delete_layer",
            "update_layer_properties",
            "candidate_page_presentation",
            "set_selection",
            "selection_properties",
            "group_selection",
            "ungroup_selection",
            "reorder_selection",
            "copy_selection",
            "paste_selection",
            "duplicate_selection",
            "create_basic_element",
            "create_connector",
            "set_connector_endpoint",
            "delete_selection",
            "update_element_properties",
            "update_element_appearance",
            "new_document",
            "open_document",
            "save_document",
            "recovery_status",
            "restore_recovery",
            "discard_recovery",
            "sync_recovery",
            "undo",
            "redo",
            "commit_move_elements",
            "open_renderer_benchmark",
            "renderer_benchmark_environment",
            "persist_renderer_benchmark_evidence",
            "close_renderer_benchmark",
        ]),
    ))
    .expect("failed to build DiagramDesigner Next Tauri application metadata");
}

fn resolve_source_commit() -> String {
    if let Ok(value) = env::var("DDN_SOURCE_COMMIT") {
        return validate_git_oid(value.trim()).unwrap_or_else(|| {
            panic!("DDN_SOURCE_COMMIT must contain exactly 40 hexadecimal characters")
        });
    }

    git_stdout(&["rev-parse", "--verify", "HEAD"])
        .and_then(|value| validate_git_oid(value.trim()))
        .unwrap_or_else(|| "unknown".to_owned())
}

fn resolve_source_dirty() -> &'static str {
    if let Ok(value) = env::var("DDN_SOURCE_DIRTY") {
        return match value.trim() {
            "true" => "true",
            "false" => "false",
            _ => panic!("DDN_SOURCE_DIRTY must be either true or false"),
        };
    }

    match git_stdout(&["status", "--porcelain", "--untracked-files=normal"]) {
        Some(status) if status.trim().is_empty() => "false",
        Some(_) => "true",
        None => "unknown",
    }
}

fn resolve_source_lock_blob() -> String {
    if let Ok(value) = env::var("DDN_SOURCE_LOCK_BLOB") {
        return validate_git_oid(value.trim()).unwrap_or_else(|| {
            panic!("DDN_SOURCE_LOCK_BLOB must contain exactly 40 hexadecimal characters")
        });
    }

    git_stdout(&["rev-parse", "HEAD:apps/desktop/src-tauri/Cargo.lock"])
        .and_then(|value| validate_git_oid(value.trim()))
        .unwrap_or_else(|| "unknown".to_owned())
}

fn validate_git_oid(value: &str) -> Option<String> {
    if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(value.to_ascii_lowercase())
    } else {
        None
    }
}

fn git_stdout(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}
