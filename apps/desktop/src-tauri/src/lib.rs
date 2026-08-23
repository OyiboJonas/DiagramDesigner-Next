mod renderer_benchmark_evidence;

use std::{
    fs,
    io::ErrorKind,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};

use app_core::ApplicationSession;
use ddnx::PackageLimits;
use editor_runtime::RecoveryPlan;
use next_domain::{
    ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, ElementId, Layer, LayerId,
    NextArtifact, Page, PageId, Point, Rect, Scene, Size,
};
use platform_fs::{AtomicSaveError, CommitMode, DurabilityLevel, atomic_save};
use render_plan::{RenderPlanOptions, build_page_plan};
use render_svg::{SvgRenderOptions, render_plan_to_svg};
use renderer_benchmark_evidence::{
    RendererBenchmarkEvidenceRequest, RendererBenchmarkEvidenceResultDto, build_source_dirty,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;

const UNTITLED_DOCUMENT_NAME: &str = "Untitled";
const MAX_MOVE_ELEMENTS: usize = 100_000;
const MAX_SELECTION_ELEMENTS: usize = 100_000;
const RECOVERY_DIRECTORY_NAME: &str = "recovery";
const RECOVERY_FILE_NAME: &str = "current.ddnx";
const RENDERER_BENCHMARK_WINDOW_LABEL: &str = "renderer-benchmark";

struct DesktopDocument {
    session: ApplicationSession,
    path: Option<PathBuf>,
    /// A restored recovery snapshot is intentionally detached from any user file.
    /// `ApplicationSession` correctly sees the decoded snapshot as its initial
    /// history state, so this desktop-owned flag keeps the recovered copy dirty
    /// until the user explicitly saves it.
    recovered_dirty: bool,
}

impl DesktopDocument {
    fn blank() -> Result<Self, CommandError> {
        Ok(Self {
            session: ApplicationSession::from_artifact(blank_document_artifact())
                .map_err(|error| CommandError::new("new_document_failed", error.to_string()))?,
            path: None,
            recovered_dirty: false,
        })
    }
}

struct DesktopState {
    document: Mutex<DesktopDocument>,
}

impl DesktopState {
    fn new() -> Result<Self, CommandError> {
        Ok(Self {
            document: Mutex::new(DesktopDocument::blank()?),
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandError {
    code: &'static str,
    message: String,
    committed: bool,
}

impl CommandError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            committed: false,
        }
    }

    fn committed(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            committed: true,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentStateDto {
    name: String,
    path: Option<String>,
    dirty: bool,
    recovered: bool,
    page_count: usize,
    active_page_id: Option<PageId>,
    document_generation: u64,
    history_state: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidatePagePresentationDto {
    page_id: PageId,
    page_name: String,
    width_mm: f64,
    height_mm: f64,
    snap_elements: Vec<SnapElementDto>,
    svg: String,
    rendered_elements: usize,
    skipped_elements: usize,
    diagnostic_count: usize,
    document_generation: u64,
    history_state: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapElementDto {
    element_id: ElementId,
    bounds_mm: Rect,
    rotation_deg: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentActionDto {
    cancelled: bool,
    state: DocumentStateDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SaveResultDto {
    cancelled: bool,
    state: DocumentStateDto,
    commit_mode: Option<&'static str>,
    durability: Option<&'static str>,
    cleanup_warning: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryStatusDto {
    available: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoverySyncDto {
    action: &'static str,
    state: DocumentStateDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RendererBenchmarkEnvironmentDto {
    runtime: &'static str,
    platform: &'static str,
    client_width_px: u32,
    client_height_px: u32,
    scale_factor: f64,
    fullscreen: bool,
    monitor_width_px: Option<u32>,
    monitor_height_px: Option<u32>,
    monitor_name: Option<String>,
    app_version: &'static str,
    source_commit: &'static str,
    source_dirty: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SelectionRequest {
    element_ids: Vec<ElementId>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveElementsRequest {
    element_ids: Vec<ElementId>,
    delta_mm: Point,
}

#[tauri::command]
fn document_state(state: State<'_, DesktopState>) -> Result<DocumentStateDto, CommandError> {
    let document = lock_document(&state)?;
    Ok(document_state_dto(&document))
}

#[tauri::command]
async fn open_renderer_benchmark(app: AppHandle) -> Result<(), CommandError> {
    if let Some(window) = app.get_webview_window(RENDERER_BENCHMARK_WINDOW_LABEL) {
        window.show().map_err(|error| {
            CommandError::new("benchmark_window_show_failed", error.to_string())
        })?;
        window.set_focus().map_err(|error| {
            CommandError::new("benchmark_window_focus_failed", error.to_string())
        })?;
        return Ok(());
    }

    // Window creation is deliberately performed from an async command. Tauri's
    // Windows/WebView2 implementation documents a synchronous-command deadlock
    // hazard for WebviewWindowBuilder::new. The benchmark is a separate fullscreen
    // WebView so inner_size() represents the physical client area used by ADR-019.
    WebviewWindowBuilder::new(
        &app,
        RENDERER_BENCHMARK_WINDOW_LABEL,
        WebviewUrl::App("renderer-benchmark.html".into()),
    )
    .title("DiagramDesigner Next — ADR-019 Renderer Benchmark")
    .inner_size(1280.0, 800.0)
    .min_inner_size(960.0, 640.0)
    .resizable(true)
    .fullscreen(true)
    .build()
    .map_err(|error| CommandError::new("benchmark_window_create_failed", error.to_string()))?;

    Ok(())
}

#[tauri::command]
fn renderer_benchmark_environment(
    window: WebviewWindow,
) -> Result<RendererBenchmarkEnvironmentDto, CommandError> {
    ensure_renderer_benchmark_window(&window)?;

    let client = window
        .inner_size()
        .map_err(|error| CommandError::new("benchmark_client_size_failed", error.to_string()))?;
    let scale_factor = window
        .scale_factor()
        .map_err(|error| CommandError::new("benchmark_scale_factor_failed", error.to_string()))?;
    let fullscreen = window.is_fullscreen().map_err(|error| {
        CommandError::new("benchmark_fullscreen_state_failed", error.to_string())
    })?;
    let monitor = window
        .current_monitor()
        .map_err(|error| CommandError::new("benchmark_monitor_query_failed", error.to_string()))?;

    let (monitor_width_px, monitor_height_px, monitor_name) = if let Some(monitor) = monitor {
        (
            Some(monitor.size().width),
            Some(monitor.size().height),
            monitor.name().cloned(),
        )
    } else {
        (None, None, None)
    };

    Ok(RendererBenchmarkEnvironmentDto {
        runtime: if cfg!(target_os = "windows") {
            "tauri-webview2"
        } else {
            "tauri-wry"
        },
        platform: std::env::consts::OS,
        client_width_px: client.width,
        client_height_px: client.height,
        scale_factor,
        fullscreen,
        monitor_width_px,
        monitor_height_px,
        monitor_name,
        app_version: env!("CARGO_PKG_VERSION"),
        source_commit: env!("DDN_BUILD_SOURCE_COMMIT"),
        source_dirty: build_source_dirty(),
    })
}

#[tauri::command]
fn persist_renderer_benchmark_evidence(
    app: AppHandle,
    window: WebviewWindow,
    request: RendererBenchmarkEvidenceRequest,
) -> Result<RendererBenchmarkEvidenceResultDto, CommandError> {
    ensure_renderer_benchmark_window(&window)?;
    renderer_benchmark_evidence::persist(&app, request.report)
}

#[tauri::command]
fn close_renderer_benchmark(window: WebviewWindow) -> Result<(), CommandError> {
    ensure_renderer_benchmark_window(&window)?;
    window
        .close()
        .map_err(|error| CommandError::new("benchmark_window_close_failed", error.to_string()))
}

fn ensure_renderer_benchmark_window(window: &WebviewWindow) -> Result<(), CommandError> {
    if window.label() != RENDERER_BENCHMARK_WINDOW_LABEL {
        return Err(CommandError::new(
            "benchmark_window_scope_violation",
            "Renderer benchmark commands are only available to the dedicated benchmark window.",
        ));
    }
    Ok(())
}

#[tauri::command]
fn candidate_page_presentation(
    state: State<'_, DesktopState>,
) -> Result<Option<CandidatePagePresentationDto>, CommandError> {
    let document = lock_document(&state)?;
    let session = document.session.session();
    let Some(page_id) = session.active_page_id() else {
        return Ok(None);
    };
    let page = session
        .document()
        .pages
        .iter()
        .find(|page| page.id == page_id)
        .ok_or_else(|| {
            CommandError::new(
                "candidate_page_missing",
                "The active page no longer exists in the current document.",
            )
        })?;

    // The desktop presentation intentionally consumes only the public
    // renderer-independent planning boundary. SVG remains a replaceable candidate
    // adapter until ADR-019's native Windows/WebView2 gate chooses the production
    // backend. The UI contract does not expose SVG-specific editor state.
    let plan = build_page_plan(session.document(), page_id, RenderPlanOptions::default())
        .map_err(|error| CommandError::new("candidate_render_plan_failed", error.to_string()))?;
    let plan_diagnostics = plan.diagnostics.len();
    // Snapping consumes renderer-neutral document geometry rather than SVG DOM
    // measurements. The candidate adapter can therefore be replaced after ADR-019
    // without changing the movement or snapping contract.
    let snap_elements = plan
        .items
        .iter()
        .map(|item| SnapElementDto {
            element_id: item.element.id,
            bounds_mm: item.element.bounds_mm,
            rotation_deg: item.element.rotation_deg,
        })
        .collect();
    let rendered = render_plan_to_svg(
        session.document(),
        page_id,
        &plan,
        SvgRenderOptions::default(),
    )
    .map_err(|error| CommandError::new("candidate_svg_render_failed", error.to_string()))?;

    Ok(Some(CandidatePagePresentationDto {
        page_id,
        page_name: page.name.clone(),
        width_mm: page.size_mm.width,
        height_mm: page.size_mm.height,
        snap_elements,
        svg: rendered.svg,
        rendered_elements: rendered.rendered_elements,
        skipped_elements: rendered.skipped_elements,
        diagnostic_count: plan_diagnostics.saturating_add(rendered.diagnostics.len()),
        document_generation: document.session.document_generation(),
        history_state: session.current_history_state().value(),
    }))
}

#[tauri::command]
fn set_selection(
    request: SelectionRequest,
    state: State<'_, DesktopState>,
) -> Result<DocumentStateDto, CommandError> {
    if request.element_ids.len() > MAX_SELECTION_ELEMENTS {
        return Err(CommandError::new(
            "selection_too_large",
            format!(
                "Selection contains {} elements; maximum is {MAX_SELECTION_ELEMENTS}.",
                request.element_ids.len()
            ),
        ));
    }

    let mut document = lock_document(&state)?;
    document
        .session
        .set_selection(request.element_ids)
        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;
    Ok(document_state_dto(&document))
}

#[tauri::command]
fn new_document(state: State<'_, DesktopState>) -> Result<DocumentActionDto, CommandError> {
    let mut document = lock_document(&state)?;
    reject_if_dirty(&document)?;
    *document = DesktopDocument::blank()?;
    Ok(DocumentActionDto {
        cancelled: false,
        state: document_state_dto(&document),
    })
}

#[tauri::command]
async fn open_document(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<DocumentActionDto, CommandError> {
    {
        let document = lock_document(&state)?;
        reject_if_dirty(&document)?;
    }

    let selected = app
        .dialog()
        .file()
        .add_filter("DiagramDesigner Next", &["ddnx"])
        .blocking_pick_file();
    let Some(selected) = selected else {
        let document = lock_document(&state)?;
        return Ok(DocumentActionDto {
            cancelled: true,
            state: document_state_dto(&document),
        });
    };
    let path = selected
        .into_path()
        .map_err(|error| CommandError::new("invalid_selected_path", error.to_string()))?;

    let bytes = fs::read(&path)
        .map_err(|error| CommandError::new("document_read_failed", error.to_string()))?;
    let opened = ApplicationSession::from_ddnx_bytes(&bytes, PackageLimits::default())
        .map_err(|error| CommandError::new("document_open_failed", error.to_string()))?;

    let mut document = lock_document(&state)?;
    // A second check makes the replacement safe even if another command edited the
    // document while the native picker or package verification was running.
    reject_if_dirty(&document)?;
    *document = DesktopDocument {
        session: opened,
        path: Some(path),
        recovered_dirty: false,
    };

    Ok(DocumentActionDto {
        cancelled: false,
        state: document_state_dto(&document),
    })
}

#[tauri::command]
async fn save_document(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<SaveResultDto, CommandError> {
    let (prepared, existing_path) = {
        let document = lock_document(&state)?;
        let prepared = document
            .session
            .prepare_document_save(PackageLimits::default())
            .map_err(|error| {
                CommandError::new("document_prepare_save_failed", error.to_string())
            })?;
        (prepared, document.path.clone())
    };

    let first_save = existing_path.is_none();
    let destination = if let Some(path) = existing_path {
        path
    } else {
        let selected = app
            .dialog()
            .file()
            .add_filter("DiagramDesigner Next", &["ddnx"])
            .set_file_name("Untitled.ddnx")
            .blocking_save_file();
        let Some(selected) = selected else {
            let document = lock_document(&state)?;
            return Ok(SaveResultDto {
                cancelled: true,
                state: document_state_dto(&document),
                commit_mode: None,
                durability: None,
                cleanup_warning: None,
            });
        };
        normalize_ddnx_save_path(
            selected
                .into_path()
                .map_err(|error| CommandError::new("invalid_selected_path", error.to_string()))?,
        )?
    };

    // Overwrite confirmation remains application policy rather than a low-level
    // filesystem-adapter flag. Until a dedicated confirmation flow exists, the
    // first Save of a new document refuses an already-existing target.
    if first_save && destination.exists() {
        return Err(CommandError::new(
            "save_target_exists",
            "The selected file already exists. Overwrite confirmation is not implemented yet; choose a new file name.",
        ));
    }

    let report = atomic_save(&destination, prepared.bytes()).map_err(map_atomic_save_error)?;

    let mut document = lock_document(&state)?;
    if !document.session.acknowledge_document_saved(prepared.key()) {
        return Err(CommandError::committed(
            "stale_save_completion",
            "The file was written, but the document session was replaced before the save completed.",
        ));
    }
    if first_save {
        document.path = Some(destination);
    }
    document.recovered_dirty = false;

    Ok(SaveResultDto {
        cancelled: false,
        state: document_state_dto(&document),
        commit_mode: Some(match report.mode {
            CommitMode::Created => "created",
            CommitMode::Replaced => "replaced",
        }),
        durability: Some(match report.durability {
            DurabilityLevel::FileAndDirectorySynced => "file-and-directory-synced",
            DurabilityLevel::FileSyncedAndPlatformCommitFlushed => {
                "file-synced-platform-commit-flushed"
            }
        }),
        cleanup_warning: report.cleanup_warning,
    })
}

#[tauri::command]
fn recovery_status(app: AppHandle) -> Result<RecoveryStatusDto, CommandError> {
    Ok(RecoveryStatusDto {
        available: recovery_path(&app)?.is_file(),
    })
}

#[tauri::command]
fn restore_recovery(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<DocumentActionDto, CommandError> {
    {
        let document = lock_document(&state)?;
        reject_if_dirty(&document)?;
    }

    let path = recovery_path(&app)?;
    let bytes = fs::read(&path).map_err(|error| {
        CommandError::new(
            "recovery_read_failed",
            format!("Could not read recovery snapshot: {error}"),
        )
    })?;
    let recovered = ApplicationSession::from_ddnx_bytes(&bytes, PackageLimits::default())
        .map_err(|error| CommandError::new("recovery_decode_failed", error.to_string()))?;

    let mut document = lock_document(&state)?;
    reject_if_dirty(&document)?;
    *document = DesktopDocument {
        session: recovered,
        // A recovery snapshot never inherits a user-visible save target. The user
        // must explicitly choose where to save the recovered copy.
        path: None,
        recovered_dirty: true,
    };

    Ok(DocumentActionDto {
        cancelled: false,
        state: document_state_dto(&document),
    })
}

#[tauri::command]
fn discard_recovery(app: AppHandle) -> Result<RecoveryStatusDto, CommandError> {
    remove_recovery_file(&app)?;
    Ok(RecoveryStatusDto { available: false })
}

#[tauri::command]
fn sync_recovery(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<RecoverySyncDto, CommandError> {
    let path = recovery_path(&app)?;
    let mut document = lock_document(&state)?;

    // A restored copy intentionally has no user-visible persisted baseline. While
    // it is in that state, always checkpoint the exact current document directly.
    // This also handles Edit -> Undo back to the recovered initial history state,
    // which editor-core correctly considers clean relative to its decoded baseline
    // but which is still unsaved from the user's perspective.
    if document.recovered_dirty {
        let prepared = document
            .session
            .prepare_document_save(PackageLimits::default())
            .map_err(|error| CommandError::new("recovery_prepare_failed", error.to_string()))?;
        write_recovery_file(&path, prepared.bytes())?;
        return Ok(RecoverySyncDto {
            action: "written",
            state: document_state_dto(&document),
        });
    }

    match document.session.recovery_plan() {
        RecoveryPlan::None => {
            // The runtime can legitimately know of no recovery payload after a
            // restored copy was explicitly saved. Remove any remaining startup
            // snapshot in that clean state.
            if !document.session.is_dirty() && path.is_file() {
                remove_recovery_path(&path)?;
                return Ok(RecoverySyncDto {
                    action: "removed",
                    state: document_state_dto(&document),
                });
            }
            Ok(RecoverySyncDto {
                action: "none",
                state: document_state_dto(&document),
            })
        }
        RecoveryPlan::Write(snapshot) => {
            let key = snapshot.key();
            let prepared = document
                .session
                .prepare_document_save(PackageLimits::default())
                .map_err(|error| CommandError::new("recovery_prepare_failed", error.to_string()))?;
            if prepared.key().history_state() != key.history_state() {
                return Err(CommandError::new(
                    "recovery_state_mismatch",
                    "Recovery snapshot and verified DDNX bytes refer to different history states.",
                ));
            }
            write_recovery_file(&path, prepared.bytes())?;
            if !document.session.acknowledge_recovery_written(key) {
                return Err(CommandError::committed(
                    "stale_recovery_completion",
                    "The recovery snapshot was written, but the editor session changed before acknowledgement.",
                ));
            }
            Ok(RecoverySyncDto {
                action: "written",
                state: document_state_dto(&document),
            })
        }
        RecoveryPlan::Remove(key) => {
            remove_recovery_path(&path)?;
            if !document.session.acknowledge_recovery_removed(key) {
                return Err(CommandError::new(
                    "stale_recovery_remove",
                    "Recovery cleanup no longer matches the editor runtime checkpoint state.",
                ));
            }
            Ok(RecoverySyncDto {
                action: "removed",
                state: document_state_dto(&document),
            })
        }
    }
}

#[tauri::command]
fn undo(state: State<'_, DesktopState>) -> Result<DocumentStateDto, CommandError> {
    let mut document = lock_document(&state)?;
    document
        .session
        .undo()
        .map_err(|error| CommandError::new("undo_failed", error.to_string()))?;
    Ok(document_state_dto(&document))
}

#[tauri::command]
fn redo(state: State<'_, DesktopState>) -> Result<DocumentStateDto, CommandError> {
    let mut document = lock_document(&state)?;
    document
        .session
        .redo()
        .map_err(|error| CommandError::new("redo_failed", error.to_string()))?;
    Ok(document_state_dto(&document))
}

#[tauri::command]
fn commit_move_elements(
    request: MoveElementsRequest,
    state: State<'_, DesktopState>,
) -> Result<DocumentStateDto, CommandError> {
    if request.element_ids.len() > MAX_MOVE_ELEMENTS {
        return Err(CommandError::new(
            "move_selection_too_large",
            format!(
                "Move request contains {} elements; maximum is {MAX_MOVE_ELEMENTS}.",
                request.element_ids.len()
            ),
        ));
    }

    let mut document = lock_document(&state)?;
    document
        .session
        .commit_move_elements(request.element_ids, request.delta_mm)
        .map_err(|error| CommandError::new("move_commit_failed", error.to_string()))?;
    Ok(document_state_dto(&document))
}

fn lock_document<'a>(
    state: &'a State<'_, DesktopState>,
) -> Result<MutexGuard<'a, DesktopDocument>, CommandError> {
    state.document.lock().map_err(|_| {
        CommandError::new(
            "desktop_state_poisoned",
            "Desktop document state lock was poisoned.",
        )
    })
}

fn reject_if_dirty(document: &DesktopDocument) -> Result<(), CommandError> {
    if document.recovered_dirty || document.session.is_dirty() {
        return Err(CommandError::new(
            "unsaved_changes",
            "The current document has unsaved changes. Save it before replacing the document session.",
        ));
    }
    Ok(())
}

fn document_state_dto(document: &DesktopDocument) -> DocumentStateDto {
    let session = document.session.session();
    DocumentStateDto {
        name: session.document().name.clone(),
        path: document
            .path
            .as_deref()
            .map(|path| path.to_string_lossy().into_owned()),
        dirty: document.recovered_dirty || document.session.is_dirty(),
        recovered: document.recovered_dirty,
        page_count: session.document().pages.len(),
        active_page_id: session.active_page_id(),
        document_generation: document.session.document_generation(),
        history_state: session.current_history_state().value(),
    }
}

fn normalize_ddnx_save_path(mut path: PathBuf) -> Result<PathBuf, CommandError> {
    match path.extension().and_then(|extension| extension.to_str()) {
        None => {
            path.set_extension("ddnx");
            Ok(path)
        }
        Some(extension) if extension.eq_ignore_ascii_case("ddnx") => Ok(path),
        Some(_) => Err(CommandError::new(
            "invalid_save_extension",
            "DiagramDesigner Next documents must use the .ddnx extension.",
        )),
    }
}

fn recovery_path(app: &AppHandle) -> Result<PathBuf, CommandError> {
    let root = app.path().app_local_data_dir().map_err(|error| {
        CommandError::new(
            "recovery_directory_unavailable",
            format!("Could not resolve the application recovery directory: {error}"),
        )
    })?;
    Ok(root.join(RECOVERY_DIRECTORY_NAME).join(RECOVERY_FILE_NAME))
}

fn write_recovery_file(path: &PathBuf, bytes: &[u8]) -> Result<(), CommandError> {
    let parent = path.parent().ok_or_else(|| {
        CommandError::new(
            "recovery_directory_unavailable",
            "Recovery path has no parent directory.",
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        CommandError::new(
            "recovery_directory_create_failed",
            format!("Could not create the recovery directory: {error}"),
        )
    })?;
    atomic_save(path, bytes).map_err(map_recovery_save_error)?;
    Ok(())
}

fn remove_recovery_file(app: &AppHandle) -> Result<(), CommandError> {
    let path = recovery_path(app)?;
    remove_recovery_path(&path)
}

fn remove_recovery_path(path: &PathBuf) -> Result<(), CommandError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CommandError::new(
            "recovery_remove_failed",
            format!("Could not remove the recovery snapshot: {error}"),
        )),
    }
}

fn map_atomic_save_error(error: AtomicSaveError) -> CommandError {
    let code = if error.committed {
        "save_durability_failed_after_commit"
    } else {
        "atomic_save_failed"
    };
    if error.committed {
        CommandError::committed(code, error.to_string())
    } else {
        CommandError::new(code, error.to_string())
    }
}

fn map_recovery_save_error(error: AtomicSaveError) -> CommandError {
    let code = if error.committed {
        "recovery_durability_failed_after_commit"
    } else {
        "recovery_write_failed"
    };
    if error.committed {
        CommandError::committed(code, error.to_string())
    } else {
        CommandError::new(code, error.to_string())
    }
}

fn blank_document_artifact() -> NextArtifact {
    let page_id = PageId::new();
    let layer_id = LayerId::new();
    NextArtifact::document(Document {
        id: DocumentId::new(),
        name: UNTITLED_DOCUMENT_NAME.to_owned(),
        defaults: DocumentDefaults {
            font_family: "Arial".to_owned(),
            font_size_pt: 10.0,
            font_style_bits: 0,
            object_shadows: false,
            auto_line_break: true,
            connector_label_style: ConnectorLabelStyle::Transparent,
        },
        master_layers: Vec::new(),
        pages: vec![Page {
            id: page_id,
            name: "Page 1".to_owned(),
            size_mm: Size {
                width: 210.0,
                height: 297.0,
            },
            layers: vec![Layer {
                id: layer_id,
                name: "Layer 1".to_owned(),
                visible: true,
                locked: false,
                draw_color: None,
                scene: Scene {
                    roots: Vec::new(),
                    elements: Vec::new(),
                },
            }],
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = DesktopState::new().expect("failed to create initial desktop document state");
    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            document_state,
            candidate_page_presentation,
            set_selection,
            new_document,
            open_document,
            save_document,
            recovery_status,
            restore_recovery,
            discard_recovery,
            sync_recovery,
            undo,
            redo,
            commit_move_elements,
            open_renderer_benchmark,
            renderer_benchmark_environment,
            persist_renderer_benchmark_evidence,
            close_renderer_benchmark,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DiagramDesigner Next");
}
