mod clipboard;
mod grouping;
mod legacy_import;
mod renderer_benchmark_evidence;
mod save_policy;

use std::{
    fs,
    io::ErrorKind,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};

use app_core::{
    ApplicationSession, ArrangeOperation as AppArrangeOperation,
    ConnectorEndpointSide as AppConnectorEndpointSide,
    ConnectorEndpointState as AppConnectorEndpointState,
    ConnectorEndpoints as AppConnectorEndpoints, ConnectorGeometryKind as AppConnectorGeometryKind,
    ElementAppearanceUpdate, StructuralGroupCreation, ZOrderOperation as AppZOrderOperation,
};
use ddnx::PackageLimits;
use editor_runtime::RecoveryPlan;
use next_domain::{
    AnchorSet, Color, Connection, Connector, ConnectorLabelStyle, Document, DocumentDefaults,
    DocumentId, Element, ElementId, ElementKind, Endpoint, FillStyle, GradientAxis, Layer, LayerId,
    LineStyle, LinearGradient, MarkerStyle, NextArtifact, NormalizedPoint, Page, PageId, Point,
    Port, PortId, Rect, RichTextDocument, RichTextToken, Scene, ScriptPosition, Size, StrokeStyle,
    StyleId, TextBlock, TextHorizontalAlignment, TextLayout, TextStyle, TextVerticalAlignment,
};
use platform_fs::{AtomicSaveError, CommitMode, DurabilityLevel, atomic_save};
use render_plan::{RenderPlanOptions, build_page_plan};
use render_svg::{SvgRenderOptions, render_plan_to_svg};
use renderer_benchmark_evidence::{
    RendererBenchmarkEvidenceRequest, RendererBenchmarkEvidenceResultDto, build_source_dirty,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use save_policy::SaveIntent;

const UNTITLED_DOCUMENT_NAME: &str = "Untitled";
const MAX_MOVE_ELEMENTS: usize = 100_000;
const MAX_SELECTION_ELEMENTS: usize = 100_000;
const RECOVERY_DIRECTORY_NAME: &str = "recovery";
const RECOVERY_FILE_NAME: &str = "current.ddnx";
const RENDERER_BENCHMARK_WINDOW_LABEL: &str = "renderer-benchmark";

struct DesktopDocument {
    session: ApplicationSession,
    path: Option<PathBuf>,
    /// Original legacy source path retained only for provenance/display. It is
    /// never a save destination.
    source_path: Option<PathBuf>,
    /// A migrated legacy source starts as an unsaved Next copy even though its
    /// freshly-created editor history has no edits yet.
    imported_dirty: bool,
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
            source_path: None,
            imported_dirty: false,
            recovered_dirty: false,
        })
    }
}

#[derive(Debug, Clone)]
struct ClipboardAppearanceSnapshot {
    stroke: Option<StrokeStyle>,
    fill: Option<FillStyle>,
    text_color: Option<Color>,
}

struct DesktopClipboard {
    payload: clipboard::ClipboardPayload,
    appearance: std::collections::BTreeMap<ElementId, ClipboardAppearanceSnapshot>,
    paste_count: u32,
}

struct DesktopState {
    document: Mutex<DesktopDocument>,
    clipboard: Mutex<Option<DesktopClipboard>>,
}

impl DesktopState {
    fn new() -> Result<Self, CommandError> {
        Ok(Self {
            document: Mutex::new(DesktopDocument::blank()?),
            clipboard: Mutex::new(None),
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
    source_path: Option<String>,
    imported: bool,
    dirty: bool,
    recovered: bool,
    version: &'static str,
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
    selection_groups: Vec<SelectionGroupDto>,
    port_targets: Vec<PortTargetDto>,
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
struct SelectionGroupDto {
    group_id: ElementId,
    bounds_mm: Rect,
    leaf_element_ids: Vec<ElementId>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortTargetDto {
    element_id: ElementId,
    port_id: PortId,
    position_mm: Point,
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
struct ClipboardCopyDto {
    count: usize,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ZOrderOperationRequest {
    BringToFront,
    SendToBack,
    BringForward,
    SendBackward,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReorderSelectionRequest {
    operation: ZOrderOperationRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ArrangeOperationRequest {
    AlignLeft,
    AlignHorizontalCenter,
    AlignRight,
    AlignTop,
    AlignVerticalCenter,
    AlignBottom,
    DistributeHorizontal,
    DistributeVertical,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArrangeSelectionRequest {
    operation: ArrangeOperationRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BasicElementKind {
    Rectangle,
    Ellipse,
    Text,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBasicElementRequest {
    kind: BasicElementKind,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ConnectorKind {
    Straight,
    Orthogonal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateConnectorRequest {
    kind: ConnectorKind,
    start_mm: Point,
    end_mm: Point,
    start_connection: Option<Connection>,
    end_connection: Option<Connection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ConnectorEndpointSideRequest {
    Start,
    End,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetConnectorEndpointRequest {
    element_id: ElementId,
    side: ConnectorEndpointSideRequest,
    position_mm: Point,
    connection: Option<Connection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateConnectorStyleRequest {
    element_id: ElementId,
    start_marker: MarkerStyle,
    end_marker: MarkerStyle,
    line_style: LineStyle,
    secondary_color: Option<Color>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateElementPropertiesRequest {
    element_id: ElementId,
    bounds_mm: Rect,
    rotation_deg: f64,
    text: Option<String>,
    text_style: Option<TextStyleDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextStyleDto {
    bold: bool,
    italic: bool,
    underline: bool,
    strikeout: bool,
    script: ScriptPosition,
    overline: bool,
    symbol_font: bool,
    font_family: Option<String>,
    font_size_pt: Option<u16>,
    color: Option<Color>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateElementAppearanceRequest {
    element_id: ElementId,
    stroke_enabled: Option<bool>,
    stroke_color: Option<String>,
    stroke_width_mm: Option<f64>,
    fill_enabled: Option<bool>,
    fill_color: Option<String>,
    fill_gradient_enabled: Option<bool>,
    fill_gradient_end_color: Option<String>,
    fill_gradient_axis: Option<GradientAxis>,
    text_color: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ElementAppearanceDto {
    stroke_applicable: bool,
    stroke_enabled: bool,
    stroke_color: String,
    stroke_width_mm: f64,
    fill_applicable: bool,
    fill_enabled: bool,
    fill_color: String,
    fill_gradient_enabled: bool,
    fill_gradient_end_color: String,
    fill_gradient_axis: GradientAxis,
    text_color_applicable: bool,
    text_color: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ElementEditResultDto {
    state: DocumentStateDto,
    selected_element_ids: Vec<ElementId>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectionPropertiesDto {
    count: usize,
    primary: Option<ElementPropertiesDto>,
    can_group: bool,
    can_ungroup: bool,
    contains_group: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ElementPropertiesDto {
    element_id: ElementId,
    name: String,
    element_type: &'static str,
    bounds_mm: Rect,
    rotation_deg: f64,
    text: Option<String>,
    text_editable: bool,
    text_style: Option<TextStyleDto>,
    geometry_editable: bool,
    appearance: ElementAppearanceDto,
    connector: Option<ConnectorPropertiesDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectorPropertiesDto {
    kind: &'static str,
    start: ConnectorEndpointDto,
    end: ConnectorEndpointDto,
    start_marker: MarkerStyle,
    end_marker: MarkerStyle,
    line_style: LineStyle,
    secondary_color: Option<Color>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectorEndpointDto {
    position_mm: Point,
    connection: Option<ConnectorConnectionDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectorConnectionDto {
    element_id: ElementId,
    port_id: PortId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageIdRequest {
    page_id: PageId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LayerIdRequest {
    page_id: PageId,
    layer_id: LayerId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdatePagePropertiesRequest {
    page_id: PageId,
    name: String,
    size_mm: Size,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateLayerPropertiesRequest {
    page_id: PageId,
    layer_id: LayerId,
    name: String,
    visible: bool,
    locked: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentNavigationDto {
    state: DocumentStateDto,
    pages: Vec<PageNavigationDto>,
    active_page_id: Option<PageId>,
    active_layer_id: Option<LayerId>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PageNavigationDto {
    page_id: PageId,
    name: String,
    size_mm: Size,
    layers: Vec<LayerNavigationDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LayerNavigationDto {
    layer_id: LayerId,
    name: String,
    visible: bool,
    locked: bool,
    element_count: usize,
}

#[tauri::command]
fn document_state(state: State<'_, DesktopState>) -> Result<DocumentStateDto, CommandError> {
    let document = lock_document(&state)?;
    Ok(document_state_dto(&document))
}

#[tauri::command]
fn document_navigation(
    state: State<'_, DesktopState>,
) -> Result<DocumentNavigationDto, CommandError> {
    let document = lock_document(&state)?;
    Ok(document_navigation_dto(&document))
}

#[tauri::command]
fn activate_page(
    request: PageIdRequest,
    state: State<'_, DesktopState>,
) -> Result<DocumentNavigationDto, CommandError> {
    let mut document = lock_document(&state)?;
    document
        .session
        .set_active_page(request.page_id)
        .map_err(|error| CommandError::new("activate_page_failed", error.to_string()))?;
    document.session.clear_selection();
    Ok(document_navigation_dto(&document))
}

#[tauri::command]
fn activate_layer(
    request: LayerIdRequest,
    state: State<'_, DesktopState>,
) -> Result<DocumentNavigationDto, CommandError> {
    let mut document = lock_document(&state)?;
    document
        .session
        .set_active_page_layer(request.page_id, request.layer_id)
        .map_err(|error| CommandError::new("activate_layer_failed", error.to_string()))?;
    document.session.clear_selection();
    Ok(document_navigation_dto(&document))
}

#[tauri::command]
fn create_page(state: State<'_, DesktopState>) -> Result<DocumentNavigationDto, CommandError> {
    let mut document = lock_document(&state)?;
    let (number, size_mm) = {
        let session = document.session.session();
        let size_mm = session
            .active_page_id()
            .and_then(|page_id| {
                session
                    .document()
                    .pages
                    .iter()
                    .find(|page| page.id == page_id)
                    .map(|page| page.size_mm)
            })
            .unwrap_or(Size {
                width: 210.0,
                height: 297.0,
            });
        (session.document().pages.len() + 1, size_mm)
    };
    let page = empty_page(format!("Page {number}"), size_mm);
    let page_id = page.id;
    let layer_id = page.layers[0].id;
    document
        .session
        .create_page(page)
        .map_err(|error| CommandError::new("page_create_failed", error.to_string()))?;
    document
        .session
        .set_active_page(page_id)
        .map_err(|error| CommandError::new("activate_page_failed", error.to_string()))?;
    document
        .session
        .set_active_page_layer(page_id, layer_id)
        .map_err(|error| CommandError::new("activate_layer_failed", error.to_string()))?;
    document.session.clear_selection();
    Ok(document_navigation_dto(&document))
}

#[tauri::command]
fn delete_page(
    request: PageIdRequest,
    state: State<'_, DesktopState>,
) -> Result<DocumentNavigationDto, CommandError> {
    let mut document = lock_document(&state)?;
    if document.session.session().document().pages.len() <= 1 {
        return Err(CommandError::new(
            "last_page_delete_blocked",
            "A document must keep at least one page.",
        ));
    }
    document
        .session
        .delete_page(request.page_id)
        .map_err(|error| CommandError::new("page_delete_failed", error.to_string()))?;
    document.session.clear_selection();
    Ok(document_navigation_dto(&document))
}

#[tauri::command]
fn update_page_properties(
    request: UpdatePagePropertiesRequest,
    state: State<'_, DesktopState>,
) -> Result<DocumentNavigationDto, CommandError> {
    let mut document = lock_document(&state)?;
    let name = structure_name(&request.name)?;
    document
        .session
        .set_page_properties(request.page_id, name, request.size_mm)
        .map_err(|error| CommandError::new("page_properties_failed", error.to_string()))?;
    Ok(document_navigation_dto(&document))
}

#[tauri::command]
fn create_layer(
    request: PageIdRequest,
    state: State<'_, DesktopState>,
) -> Result<DocumentNavigationDto, CommandError> {
    let mut document = lock_document(&state)?;
    let number = document
        .session
        .session()
        .document()
        .pages
        .iter()
        .find(|page| page.id == request.page_id)
        .ok_or_else(|| CommandError::new("page_missing", "The requested page no longer exists."))?
        .layers
        .len()
        + 1;
    let layer = empty_layer(format!("Layer {number}"));
    let layer_id = layer.id;
    document
        .session
        .create_page_layer(request.page_id, layer)
        .map_err(|error| CommandError::new("layer_create_failed", error.to_string()))?;
    document
        .session
        .set_active_page_layer(request.page_id, layer_id)
        .map_err(|error| CommandError::new("activate_layer_failed", error.to_string()))?;
    document.session.clear_selection();
    Ok(document_navigation_dto(&document))
}

#[tauri::command]
fn delete_layer(
    request: LayerIdRequest,
    state: State<'_, DesktopState>,
) -> Result<DocumentNavigationDto, CommandError> {
    let mut document = lock_document(&state)?;
    let layer_count = document
        .session
        .session()
        .document()
        .pages
        .iter()
        .find(|page| page.id == request.page_id)
        .ok_or_else(|| CommandError::new("page_missing", "The requested page no longer exists."))?
        .layers
        .len();
    if layer_count <= 1 {
        return Err(CommandError::new(
            "last_layer_delete_blocked",
            "A page must keep at least one local layer.",
        ));
    }
    document
        .session
        .delete_page_layer(request.page_id, request.layer_id)
        .map_err(|error| CommandError::new("layer_delete_failed", error.to_string()))?;
    document.session.clear_selection();
    Ok(document_navigation_dto(&document))
}

#[tauri::command]
fn update_layer_properties(
    request: UpdateLayerPropertiesRequest,
    state: State<'_, DesktopState>,
) -> Result<DocumentNavigationDto, CommandError> {
    let mut document = lock_document(&state)?;
    let name = structure_name(&request.name)?;
    let draw_color = document
        .session
        .session()
        .document()
        .pages
        .iter()
        .find(|page| page.id == request.page_id)
        .and_then(|page| {
            page.layers
                .iter()
                .find(|layer| layer.id == request.layer_id)
        })
        .ok_or_else(|| CommandError::new("layer_missing", "The requested layer no longer exists."))?
        .draw_color;
    document
        .session
        .set_page_layer_properties(
            request.page_id,
            request.layer_id,
            name,
            request.visible,
            request.locked,
            draw_color,
        )
        .map_err(|error| CommandError::new("layer_properties_failed", error.to_string()))?;
    if !request.visible || request.locked {
        document.session.clear_selection();
    }
    Ok(document_navigation_dto(&document))
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
    let selection_groups: Vec<_> = grouping::selection_groups(session.document(), page_id)
        .into_iter()
        .map(|group| SelectionGroupDto {
            group_id: group.group_id,
            bounds_mm: group.bounds_mm,
            leaf_element_ids: group.leaf_element_ids,
        })
        .collect();
    let mut snap_elements: Vec<SnapElementDto> = plan
        .items
        .iter()
        .map(|item| SnapElementDto {
            element_id: item.element.id,
            bounds_mm: item.element.bounds_mm,
            rotation_deg: item.element.rotation_deg,
        })
        .collect();
    snap_elements.extend(selection_groups.iter().map(|group| SnapElementDto {
        element_id: group.group_id,
        bounds_mm: group.bounds_mm,
        rotation_deg: 0.0,
    }));
    let port_targets = document
        .session
        .active_page_layer_ports()
        .map_err(|error| CommandError::new("connector_ports_failed", error.to_string()))?
        .into_iter()
        .map(|port| PortTargetDto {
            element_id: port.element_id,
            port_id: port.port_id,
            position_mm: port.position_mm,
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
        selection_groups,
        port_targets,
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
fn selection_properties(
    state: State<'_, DesktopState>,
) -> Result<SelectionPropertiesDto, CommandError> {
    let document = lock_document(&state)?;
    let session = document.session.session();
    let selected: Vec<_> = session.selection().iter().copied().collect();
    let primary = if selected.len() == 1 {
        let element = find_element(session.document(), selected[0]).ok_or_else(|| {
            CommandError::new(
                "selection_element_missing",
                "The selected element no longer exists in the current document.",
            )
        })?;
        let connector = document
            .session
            .connector_endpoints(element.id)
            .map_err(|error| CommandError::new("connector_query_failed", error.to_string()))?;
        Some(element_properties_dto(
            element,
            connector,
            session.document(),
        ))
    } else {
        None
    };
    let grouping_state = grouping::selection_capabilities(
        session.document(),
        session.active_page_id(),
        document.session.active_page_layer_id(),
        &selected,
    );
    Ok(SelectionPropertiesDto {
        count: selected.len(),
        primary,
        can_group: grouping_state.can_group,
        can_ungroup: grouping_state.can_ungroup,
        contains_group: grouping_state.contains_group,
    })
}

#[tauri::command]
fn group_selection(state: State<'_, DesktopState>) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let selected: Vec<_> = document
        .session
        .session()
        .selection()
        .iter()
        .copied()
        .collect();
    let capabilities = grouping::selection_capabilities(
        document.session.session().document(),
        document.session.session().active_page_id(),
        document.session.active_page_layer_id(),
        &selected,
    );
    if !capabilities.can_group {
        return Err(CommandError::new(
            "group_selection_invalid",
            "Select at least two adjacent top-level elements on the visible, unlocked active layer.",
        ));
    }

    let group_id = ElementId::new();
    document
        .session
        .group_elements(group_id, selected, "Group".to_owned())
        .map_err(|error| CommandError::new("group_selection_failed", error.to_string()))?;
    document
        .session
        .set_selection([group_id])
        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

#[tauri::command]
fn ungroup_selection(state: State<'_, DesktopState>) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let selected: Vec<_> = document
        .session
        .session()
        .selection()
        .iter()
        .copied()
        .collect();
    let page_id = document.session.session().active_page_id();
    let layer_id = document.session.active_page_layer_id();
    let children = grouping::selected_group_children(
        document.session.session().document(),
        page_id,
        layer_id,
        &selected,
    )
    .ok_or_else(|| {
        CommandError::new(
            "ungroup_selection_invalid",
            "Select one top-level group on the visible, unlocked active layer.",
        )
    })?;
    let group_id = selected[0];

    document
        .session
        .ungroup(group_id)
        .map_err(|error| CommandError::new("ungroup_selection_failed", error.to_string()))?;
    document
        .session
        .set_selection(children)
        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

#[tauri::command]
fn create_basic_element(
    request: CreateBasicElementRequest,
    state: State<'_, DesktopState>,
) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let (target, page_size) = {
        let session = document.session.session();
        let target = session.active_layer().ok_or_else(|| {
            CommandError::new(
                "no_active_layer",
                "The current document has no active layer for element creation.",
            )
        })?;
        let page_size = session
            .active_page_id()
            .and_then(|page_id| {
                session
                    .document()
                    .pages
                    .iter()
                    .find(|page| page.id == page_id)
                    .map(|page| page.size_mm)
            })
            .unwrap_or(Size {
                width: 210.0,
                height: 297.0,
            });
        (target, page_size)
    };

    let element_id = ElementId::new();
    let (name, width, height, kind, text) = match request.kind {
        BasicElementKind::Rectangle => (
            "Rectangle".to_owned(),
            40.0,
            25.0,
            ElementKind::Rectangle {
                corner_radius_mm: 0.0,
            },
            None,
        ),
        BasicElementKind::Ellipse => ("Ellipse".to_owned(), 40.0, 25.0, ElementKind::Ellipse, None),
        BasicElementKind::Text => (
            "Text".to_owned(),
            60.0,
            20.0,
            ElementKind::Text,
            Some(simple_text_block("Text", TextStyle::default(), None)),
        ),
    };
    let bounds_mm = Rect {
        x: ((page_size.width - width) / 2.0).max(0.0),
        y: ((page_size.height - height) / 2.0).max(0.0),
        width,
        height,
    };
    let element = Element {
        id: element_id,
        name,
        bounds_mm,
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: default_shape_ports(),
        style_id: None,
        text,
        kind,
        import: None,
    };

    document
        .session
        .create_element(target, element, None)
        .map_err(|error| CommandError::new("element_create_failed", error.to_string()))?;
    document
        .session
        .set_selection([element_id])
        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

#[tauri::command]
fn create_connector(
    request: CreateConnectorRequest,
    state: State<'_, DesktopState>,
) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let (target, page_size) = {
        let session = document.session.session();
        let page_id = session.active_page_id().ok_or_else(|| {
            CommandError::new("no_active_page", "The current document has no active page.")
        })?;
        let layer_id = document.session.active_page_layer_id().ok_or_else(|| {
            CommandError::new(
                "no_active_page_layer",
                "Choose a page-local layer before drawing a connector.",
            )
        })?;
        let page = session
            .document()
            .pages
            .iter()
            .find(|page| page.id == page_id)
            .ok_or_else(|| {
                CommandError::new("page_missing", "The active page no longer exists.")
            })?;
        let layer = page
            .layers
            .iter()
            .find(|layer| layer.id == layer_id)
            .ok_or_else(|| {
                CommandError::new("layer_missing", "The active layer no longer exists.")
            })?;
        if !layer.visible {
            return Err(CommandError::new(
                "connector_layer_hidden",
                "Connectors can be drawn only on a visible layer.",
            ));
        }
        if layer.locked {
            return Err(CommandError::new(
                "connector_layer_locked",
                "Unlock the active layer before drawing a connector.",
            ));
        }
        let target = session.active_layer().ok_or_else(|| {
            CommandError::new(
                "no_active_layer",
                "The current document has no active layer.",
            )
        })?;
        (target, page.size_mm)
    };

    let ports = document
        .session
        .active_page_layer_ports()
        .map_err(|error| CommandError::new("connector_ports_failed", error.to_string()))?;
    let start = connector_creation_endpoint(
        request.start_mm,
        request.start_connection,
        page_size,
        &ports,
    )?;
    let end =
        connector_creation_endpoint(request.end_mm, request.end_connection, page_size, &ports)?;
    let start_mm = start.position_mm;
    let end_mm = end.position_mm;
    let distance_mm = (end_mm.x - start_mm.x).hypot(end_mm.y - start_mm.y);
    if distance_mm < 0.5 {
        return Err(CommandError::new(
            "connector_too_short",
            "Drag at least 0.5 mm to create a connector.",
        ));
    }

    let element_id = ElementId::new();
    let connector = Connector {
        start,
        end,
        start_marker: MarkerStyle::None,
        end_marker: MarkerStyle::None,
        line_style: LineStyle::Solid,
        secondary_color: None,
    };
    let (name, kind) = match request.kind {
        ConnectorKind::Straight => (
            "Connector".to_owned(),
            ElementKind::StraightConnector { connector },
        ),
        ConnectorKind::Orthogonal => (
            "Orthogonal connector".to_owned(),
            ElementKind::OrthogonalConnector {
                connector,
                corner_radius_mm: 0.0,
            },
        ),
    };
    let element = Element {
        id: element_id,
        name,
        bounds_mm: connector_bounds(start_mm, end_mm),
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text: None,
        kind,
        import: None,
    };

    document
        .session
        .create_element(target, element, None)
        .map_err(|error| CommandError::new("connector_create_failed", error.to_string()))?;
    document
        .session
        .set_selection([element_id])
        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

#[tauri::command]
fn set_connector_endpoint(
    request: SetConnectorEndpointRequest,
    state: State<'_, DesktopState>,
) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let page_size = {
        let session = document.session.session();
        let page_id = session.active_page_id().ok_or_else(|| {
            CommandError::new("no_active_page", "The current document has no active page.")
        })?;
        let layer_id = document.session.active_page_layer_id().ok_or_else(|| {
            CommandError::new(
                "no_active_page_layer",
                "Choose a page-local layer before editing connector endpoints.",
            )
        })?;
        let page = session
            .document()
            .pages
            .iter()
            .find(|page| page.id == page_id)
            .ok_or_else(|| {
                CommandError::new("page_missing", "The active page no longer exists.")
            })?;
        let layer = page
            .layers
            .iter()
            .find(|layer| layer.id == layer_id)
            .ok_or_else(|| {
                CommandError::new("layer_missing", "The active layer no longer exists.")
            })?;
        if !layer.visible {
            return Err(CommandError::new(
                "connector_layer_hidden",
                "Connector endpoints can be edited only on a visible layer.",
            ));
        }
        if layer.locked {
            return Err(CommandError::new(
                "connector_layer_locked",
                "Unlock the active layer before editing connector endpoints.",
            ));
        }
        if !layer
            .scene
            .elements
            .iter()
            .any(|element| element.id == request.element_id)
        {
            return Err(CommandError::new(
                "connector_not_on_active_layer",
                "The connector must belong to the active page-local layer.",
            ));
        }
        page.size_mm
    };
    let position_mm = clamp_connector_point(request.position_mm, page_size)?;
    let side = match request.side {
        ConnectorEndpointSideRequest::Start => AppConnectorEndpointSide::Start,
        ConnectorEndpointSideRequest::End => AppConnectorEndpointSide::End,
    };
    document
        .session
        .set_connector_endpoint(request.element_id, side, position_mm, request.connection)
        .map_err(|error| CommandError::new("connector_endpoint_failed", error.to_string()))?;
    document
        .session
        .set_selection([request.element_id])
        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

#[tauri::command]
fn update_connector_style(
    request: UpdateConnectorStyleRequest,
    state: State<'_, DesktopState>,
) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    document
        .session
        .set_connector_style(
            request.element_id,
            request.start_marker,
            request.end_marker,
            request.line_style,
            request.secondary_color,
        )
        .map_err(|error| CommandError::new("connector_style_failed", error.to_string()))?;
    document
        .session
        .set_selection([request.element_id])
        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

#[tauri::command]
fn delete_selection(state: State<'_, DesktopState>) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let selected: Vec<_> = document
        .session
        .session()
        .selection()
        .iter()
        .copied()
        .collect();
    if !selected.is_empty() {
        document
            .session
            .delete_elements(selected)
            .map_err(|error| CommandError::new("element_delete_failed", error.to_string()))?;
        document.session.clear_selection();
    }
    Ok(element_edit_result_dto(&document))
}

#[tauri::command]
fn reorder_selection(
    request: ReorderSelectionRequest,
    state: State<'_, DesktopState>,
) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let selected: Vec<_> = document
        .session
        .session()
        .selection()
        .iter()
        .copied()
        .collect();
    if selected.is_empty() {
        return Ok(element_edit_result_dto(&document));
    }

    {
        let session = document.session.session();
        let page_id = session.active_page_id().ok_or_else(|| {
            CommandError::new(
                "arrange_no_active_page",
                "Choose an active page before arranging elements.",
            )
        })?;
        let layer_id = document.session.active_page_layer_id().ok_or_else(|| {
            CommandError::new(
                "arrange_no_active_layer",
                "Choose a page-local layer before arranging elements.",
            )
        })?;
        let page = session
            .document()
            .pages
            .iter()
            .find(|page| page.id == page_id)
            .ok_or_else(|| {
                CommandError::new("arrange_page_missing", "The active page no longer exists.")
            })?;
        let layer = page
            .layers
            .iter()
            .find(|layer| layer.id == layer_id)
            .ok_or_else(|| {
                CommandError::new(
                    "arrange_layer_missing",
                    "The active layer no longer exists.",
                )
            })?;
        if !layer.visible {
            return Err(CommandError::new(
                "arrange_layer_hidden",
                "Elements can be arranged only on a visible active layer.",
            ));
        }
        if layer.locked {
            return Err(CommandError::new(
                "arrange_layer_locked",
                "Unlock the active layer before arranging elements.",
            ));
        }
        if selected.iter().any(|element_id| {
            !layer
                .scene
                .elements
                .iter()
                .any(|element| element.id == *element_id)
        }) {
            return Err(CommandError::new(
                "arrange_not_on_active_layer",
                "Every selected element must belong to the active layer.",
            ));
        }
    }

    let operation = match request.operation {
        ZOrderOperationRequest::BringToFront => AppZOrderOperation::BringToFront,
        ZOrderOperationRequest::SendToBack => AppZOrderOperation::SendToBack,
        ZOrderOperationRequest::BringForward => AppZOrderOperation::BringForward,
        ZOrderOperationRequest::SendBackward => AppZOrderOperation::SendBackward,
    };
    document
        .session
        .reorder_elements(selected, operation)
        .map_err(|error| CommandError::new("arrange_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

#[tauri::command]
fn arrange_selection(
    request: ArrangeSelectionRequest,
    state: State<'_, DesktopState>,
) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let selected: Vec<_> = document
        .session
        .session()
        .selection()
        .iter()
        .copied()
        .collect();

    {
        let session = document.session.session();
        let page_id = session.active_page_id().ok_or_else(|| {
            CommandError::new(
                "layout_no_active_page",
                "Choose an active page before aligning or distributing elements.",
            )
        })?;
        let layer_id = document.session.active_page_layer_id().ok_or_else(|| {
            CommandError::new(
                "layout_no_active_layer",
                "Choose a page-local layer before aligning or distributing elements.",
            )
        })?;
        let page = session
            .document()
            .pages
            .iter()
            .find(|page| page.id == page_id)
            .ok_or_else(|| {
                CommandError::new("layout_page_missing", "The active page no longer exists.")
            })?;
        let layer = page
            .layers
            .iter()
            .find(|layer| layer.id == layer_id)
            .ok_or_else(|| {
                CommandError::new("layout_layer_missing", "The active layer no longer exists.")
            })?;
        if !layer.visible {
            return Err(CommandError::new(
                "layout_layer_hidden",
                "Elements can be aligned or distributed only on a visible active layer.",
            ));
        }
        if layer.locked {
            return Err(CommandError::new(
                "layout_layer_locked",
                "Unlock the active layer before aligning or distributing elements.",
            ));
        }
        if selected.iter().any(|element_id| {
            !layer
                .scene
                .elements
                .iter()
                .any(|element| element.id == *element_id)
        }) {
            return Err(CommandError::new(
                "layout_not_on_active_layer",
                "Every selected element must belong to the active layer.",
            ));
        }
    }

    let operation = match request.operation {
        ArrangeOperationRequest::AlignLeft => AppArrangeOperation::AlignLeft,
        ArrangeOperationRequest::AlignHorizontalCenter => {
            AppArrangeOperation::AlignHorizontalCenter
        }
        ArrangeOperationRequest::AlignRight => AppArrangeOperation::AlignRight,
        ArrangeOperationRequest::AlignTop => AppArrangeOperation::AlignTop,
        ArrangeOperationRequest::AlignVerticalCenter => AppArrangeOperation::AlignVerticalCenter,
        ArrangeOperationRequest::AlignBottom => AppArrangeOperation::AlignBottom,
        ArrangeOperationRequest::DistributeHorizontal => AppArrangeOperation::DistributeHorizontal,
        ArrangeOperationRequest::DistributeVertical => AppArrangeOperation::DistributeVertical,
    };
    document
        .session
        .arrange_elements(selected, operation)
        .map_err(|error| CommandError::new("layout_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

#[tauri::command]
fn copy_selection(state: State<'_, DesktopState>) -> Result<ClipboardCopyDto, CommandError> {
    let document = lock_document(&state)?;
    let selected: Vec<_> = document
        .session
        .session()
        .selection()
        .iter()
        .copied()
        .collect();
    let payload = clipboard::capture_selection(document.session.session().document(), &selected)
        .map_err(|error| CommandError::new("clipboard_copy_failed", error.to_string()))?;
    let count = payload.len();
    let appearance = capture_clipboard_appearance(
        document.session.session().document(),
        payload.source_element_ids(),
    )?;
    let mut application_clipboard = state.clipboard.lock().map_err(|_| {
        CommandError::new(
            "clipboard_lock_failed",
            "The application clipboard lock is poisoned.",
        )
    })?;
    *application_clipboard = Some(DesktopClipboard {
        payload,
        appearance,
        paste_count: 0,
    });
    Ok(ClipboardCopyDto { count })
}

#[tauri::command]
fn paste_selection(state: State<'_, DesktopState>) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let target = document.session.session().active_layer().ok_or_else(|| {
        CommandError::new(
            "clipboard_no_active_layer",
            "Choose an active layer before pasting elements.",
        )
    })?;
    let mut application_clipboard = state.clipboard.lock().map_err(|_| {
        CommandError::new(
            "clipboard_lock_failed",
            "The application clipboard lock is poisoned.",
        )
    })?;
    let clipboard = application_clipboard
        .as_mut()
        .ok_or_else(|| CommandError::new("clipboard_empty", "Copy a selection before pasting."))?;
    let next_step = clipboard.paste_count.saturating_add(1).max(1);
    let mut instantiated = clipboard.payload.instantiate(next_step);
    let selected = instantiated.element_ids.clone();
    let appearance_updates =
        prepare_clipboard_appearance_updates(&clipboard.appearance, &mut instantiated)?;
    let groups = instantiated
        .groups
        .into_iter()
        .map(|group| StructuralGroupCreation {
            element: group.element,
            z_index: group.z_index,
        })
        .collect();
    document
        .session
        .create_elements_with_groups(target, instantiated.elements, groups, appearance_updates)
        .map_err(|error| CommandError::new("clipboard_paste_failed", error.to_string()))?;
    document
        .session
        .set_selection(selected)
        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;
    clipboard.paste_count = next_step;
    Ok(element_edit_result_dto(&document))
}

#[tauri::command]
fn duplicate_selection(
    state: State<'_, DesktopState>,
) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let target = document.session.session().active_layer().ok_or_else(|| {
        CommandError::new(
            "duplicate_no_active_layer",
            "Choose an active layer before duplicating elements.",
        )
    })?;
    let selected: Vec<_> = document
        .session
        .session()
        .selection()
        .iter()
        .copied()
        .collect();
    let payload = clipboard::capture_selection(document.session.session().document(), &selected)
        .map_err(|error| CommandError::new("duplicate_failed", error.to_string()))?;
    let appearance = capture_clipboard_appearance(
        document.session.session().document(),
        payload.source_element_ids(),
    )?;
    let mut instantiated = payload.instantiate(1);
    let duplicated_ids = instantiated.element_ids.clone();
    let appearance_updates = prepare_clipboard_appearance_updates(&appearance, &mut instantiated)?;
    let groups = instantiated
        .groups
        .into_iter()
        .map(|group| StructuralGroupCreation {
            element: group.element,
            z_index: group.z_index,
        })
        .collect();
    document
        .session
        .create_elements_with_groups(target, instantiated.elements, groups, appearance_updates)
        .map_err(|error| CommandError::new("duplicate_failed", error.to_string()))?;
    document
        .session
        .set_selection(duplicated_ids)
        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

fn clear_application_clipboard(state: &State<'_, DesktopState>) -> Result<(), CommandError> {
    let mut application_clipboard = state.clipboard.lock().map_err(|_| {
        CommandError::new(
            "clipboard_lock_failed",
            "The application clipboard lock is poisoned.",
        )
    })?;
    *application_clipboard = None;
    Ok(())
}

fn capture_clipboard_appearance(
    document: &Document,
    selected: &[ElementId],
) -> Result<std::collections::BTreeMap<ElementId, ClipboardAppearanceSnapshot>, CommandError> {
    const APPEARANCE_STYLE_NAMESPACE: &str = "diagramdesigner-next:element-appearance";
    let mut snapshots = std::collections::BTreeMap::new();

    for source_id in selected {
        let source = find_element(document, *source_id).ok_or_else(|| {
            CommandError::new(
                "clipboard_source_missing",
                "A copied source element no longer exists in the current document.",
            )
        })?;
        let dedicated_style_id = StyleId::v5(source_id.0, APPEARANCE_STYLE_NAMESPACE);
        if source.style_id != Some(dedicated_style_id) {
            continue;
        }
        let style = document
            .styles
            .iter()
            .find(|style| style.id == dedicated_style_id)
            .ok_or_else(|| {
                CommandError::new(
                    "clipboard_appearance_missing",
                    "The copied element's dedicated appearance style is missing.",
                )
            })?;
        snapshots.insert(
            *source_id,
            ClipboardAppearanceSnapshot {
                stroke: style.stroke.clone(),
                fill: style.fill.clone(),
                text_color: style.text_color,
            },
        );
    }

    Ok(snapshots)
}

fn prepare_clipboard_appearance_updates(
    snapshots: &std::collections::BTreeMap<ElementId, ClipboardAppearanceSnapshot>,
    instantiated: &mut clipboard::ClipboardInstantiation,
) -> Result<Vec<ElementAppearanceUpdate>, CommandError> {
    let mut updates = Vec::new();

    for (source_id, copied_id) in &instantiated.source_element_ids {
        let Some(style) = snapshots.get(source_id) else {
            continue;
        };
        let copied = if let Some(element) = instantiated
            .elements
            .iter_mut()
            .find(|element| element.id == *copied_id)
        {
            element
        } else if let Some(group) = instantiated
            .groups
            .iter_mut()
            .find(|group| group.element.id == *copied_id)
        {
            &mut group.element
        } else {
            return Err(CommandError::new(
                "clipboard_copy_missing",
                "The instantiated clipboard element could not be resolved.",
            ));
        };

        copied.style_id = None;
        updates.push(ElementAppearanceUpdate {
            element_id: *copied_id,
            stroke: style.stroke.clone(),
            fill: style.fill.clone(),
            text_color: style.text_color,
        });
    }

    Ok(updates)
}

#[tauri::command]
fn update_element_properties(
    request: UpdateElementPropertiesRequest,
    state: State<'_, DesktopState>,
) -> Result<ElementEditResultDto, CommandError> {
    let UpdateElementPropertiesRequest {
        element_id,
        bounds_mm,
        rotation_deg,
        text,
        text_style,
    } = request;
    let mut document = lock_document(&state)?;
    let existing =
        find_element(document.session.session().document(), element_id).ok_or_else(|| {
            CommandError::new(
                "element_properties_missing",
                "The selected element no longer exists in the current document.",
            )
        })?;
    if !element_geometry_editable(&existing.kind) {
        return Err(CommandError::new(
            "element_geometry_requires_dedicated_tool",
            "This element uses a dedicated geometry tool and cannot be resized in the basic inspector.",
        ));
    }

    let text_update = if text.is_some() || text_style.is_some() {
        let existing =
            find_element(document.session.session().document(), element_id).ok_or_else(|| {
                CommandError::new(
                    "element_properties_missing",
                    "The selected element no longer exists in the current document.",
                )
            })?;
        let Some(existing_text) = existing.text.as_ref() else {
            return Err(CommandError::new(
                "element_text_not_editable",
                "This element does not contain editable text.",
            ));
        };
        let (preview, editable, common_style) = text_preview(existing_text);
        if !editable {
            return Err(CommandError::new(
                "element_text_not_editable",
                "This rich-text element cannot be flattened safely by the basic text editor.",
            ));
        }
        let next_style = match text_style {
            Some(style) => text_style_from_dto(style)?,
            None => common_style.unwrap_or_default(),
        };
        let next_text = text.as_deref().unwrap_or(&preview);
        Some(Some(simple_text_block(
            next_text,
            next_style,
            Some(existing_text.layout),
        )))
    } else {
        None
    };

    document
        .session
        .commit_element_properties(element_id, bounds_mm, rotation_deg, text_update)
        .map_err(|error| CommandError::new("element_properties_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

#[tauri::command]
fn update_element_appearance(
    request: UpdateElementAppearanceRequest,
    state: State<'_, DesktopState>,
) -> Result<ElementEditResultDto, CommandError> {
    let mut document = lock_document(&state)?;
    let (
        stroke_applicable,
        fill_applicable,
        text_color_applicable,
        mut stroke,
        mut fill,
        mut text_color,
    ) = {
        let session = document.session.session();
        let element = find_element(session.document(), request.element_id).ok_or_else(|| {
            CommandError::new(
                "element_appearance_missing",
                "The selected element no longer exists in the current document.",
            )
        })?;
        let (stroke_applicable, fill_applicable, text_color_applicable) =
            appearance_applicability(&element.kind);
        let (stroke, fill, text_color) =
            materialized_element_appearance(element, session.document());
        (
            stroke_applicable,
            fill_applicable,
            text_color_applicable,
            stroke,
            fill,
            text_color,
        )
    };

    if (!stroke_applicable
        && (request.stroke_enabled.is_some()
            || request.stroke_color.is_some()
            || request.stroke_width_mm.is_some()))
        || (!fill_applicable
            && (request.fill_enabled.is_some()
                || request.fill_color.is_some()
                || request.fill_gradient_enabled.is_some()
                || request.fill_gradient_end_color.is_some()
                || request.fill_gradient_axis.is_some()))
        || (!text_color_applicable && request.text_color.is_some())
    {
        return Err(CommandError::new(
            "appearance_not_applicable",
            "The requested appearance field does not apply to this element type.",
        ));
    }

    if request.fill_enabled == Some(false)
        && (request.fill_color.is_some()
            || request.fill_gradient_enabled.is_some()
            || request.fill_gradient_end_color.is_some()
            || request.fill_gradient_axis.is_some())
    {
        return Err(CommandError::new(
            "appearance_fill_disabled_details",
            "Fill detail fields cannot be changed while fill is being disabled.",
        ));
    }
    if request.fill_gradient_enabled == Some(false)
        && (request.fill_gradient_end_color.is_some() || request.fill_gradient_axis.is_some())
    {
        return Err(CommandError::new(
            "appearance_gradient_disabled_details",
            "Gradient detail fields cannot be changed while the gradient is being disabled.",
        ));
    }

    if let Some(enabled) = request.stroke_enabled {
        if enabled {
            stroke.get_or_insert_with(default_stroke);
        } else {
            stroke = None;
        }
    }
    if let Some(width) = request.stroke_width_mm {
        if !width.is_finite() || width <= 0.0 {
            return Err(CommandError::new(
                "invalid_stroke_width",
                "Stroke width must be a finite positive value.",
            ));
        }
        stroke.get_or_insert_with(default_stroke).width_mm = width;
    }
    if let Some(color) = request.stroke_color.as_deref() {
        stroke.get_or_insert_with(default_stroke).color = parse_rgb_color(color)?;
    }

    if let Some(enabled) = request.fill_enabled {
        if enabled {
            fill.get_or_insert_with(default_fill);
        } else {
            fill = None;
        }
    }
    if let Some(color) = request.fill_color.as_deref() {
        fill.get_or_insert_with(default_fill).color = parse_rgb_color(color)?;
    }
    if let Some(enabled) = request.fill_gradient_enabled {
        if enabled {
            let fill = fill.get_or_insert_with(default_fill);
            if fill.gradient.is_none() {
                fill.gradient = Some(default_linear_gradient(fill.color));
            }
        } else if let Some(fill) = fill.as_mut() {
            fill.gradient = None;
        }
    }
    if let Some(color) = request.fill_gradient_end_color.as_deref() {
        let gradient = fill
            .as_mut()
            .and_then(|fill| fill.gradient.as_mut())
            .ok_or_else(|| {
                CommandError::new(
                    "appearance_gradient_missing",
                    "Enable fill and its linear gradient before changing the gradient end colour.",
                )
            })?;
        gradient.end_color = parse_rgb_color(color)?;
    }
    if let Some(axis) = request.fill_gradient_axis {
        let gradient = fill
            .as_mut()
            .and_then(|fill| fill.gradient.as_mut())
            .ok_or_else(|| {
                CommandError::new(
                    "appearance_gradient_missing",
                    "Enable fill and its linear gradient before changing the gradient axis.",
                )
            })?;
        gradient.axis = axis;
    }
    if let Some(color) = request.text_color.as_deref() {
        text_color = Some(parse_rgb_color(color)?);
    }

    document
        .session
        .set_element_appearance(request.element_id, stroke, fill, text_color)
        .map_err(|error| CommandError::new("element_appearance_failed", error.to_string()))?;
    document
        .session
        .set_selection([request.element_id])
        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;
    Ok(element_edit_result_dto(&document))
}

#[tauri::command]
fn new_document(state: State<'_, DesktopState>) -> Result<DocumentActionDto, CommandError> {
    let mut document = lock_document(&state)?;
    reject_if_dirty(&document)?;
    *document = DesktopDocument::blank()?;
    clear_application_clipboard(&state)?;
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
        .add_filter("DiagramDesigner files", &["ddnx", "ddd", "ddt"])
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
    let source_kind = legacy_import::classify_path(&path)
        .map_err(|error| CommandError::new("unsupported_document_type", error))?;

    let bytes = fs::read(&path)
        .map_err(|error| CommandError::new("document_read_failed", error.to_string()))?;
    let (opened, persistent_path, source_path, imported_dirty) = match source_kind {
        legacy_import::DocumentSourceKind::NativeDdnx => {
            let opened = ApplicationSession::from_ddnx_bytes(&bytes, PackageLimits::default())
                .map_err(|error| CommandError::new("document_open_failed", error.to_string()))?;
            (opened, Some(path.clone()), None, false)
        }
        legacy_import::DocumentSourceKind::LegacyDdd
        | legacy_import::DocumentSourceKind::LegacyDdt => {
            let artifact = legacy_import::migrate_legacy_bytes(
                &bytes,
                source_kind,
                desktop_document_defaults(),
            )
            .map_err(|error| CommandError::new("legacy_import_failed", error))?;
            let opened = ApplicationSession::from_artifact(artifact)
                .map_err(|error| CommandError::new("legacy_import_failed", error.to_string()))?;
            // The legacy source is deliberately detached from persistence. Save
            // therefore enters the existing first-save .ddnx picker instead of
            // ever replacing the .ddd/.ddt source file.
            (opened, None, Some(path.clone()), true)
        }
    };

    let mut document = lock_document(&state)?;
    // A second check makes replacement safe even if another command edited the
    // document while the native picker or migration was running.
    reject_if_dirty(&document)?;
    *document = DesktopDocument {
        session: opened,
        path: persistent_path,
        source_path,
        imported_dirty,
        recovered_dirty: false,
    };
    clear_application_clipboard(&state)?;

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
    save_document_with_intent(app, state, SaveIntent::Save).await
}

#[tauri::command]
async fn save_as_document(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<SaveResultDto, CommandError> {
    save_document_with_intent(app, state, SaveIntent::SaveAs).await
}

async fn save_document_with_intent(
    app: AppHandle,
    state: State<'_, DesktopState>,
    intent: SaveIntent,
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

    let has_existing_path = existing_path.is_some();
    let destination = if intent.uses_existing_path(has_existing_path) {
        existing_path.clone().ok_or_else(|| {
            CommandError::new(
                "save_path_missing",
                "The existing document save path is no longer available.",
            )
        })?
    } else {
        let suggested_name = existing_path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled.ddnx".to_owned());
        let selected = app
            .dialog()
            .file()
            .add_filter("DiagramDesigner Next", &["ddnx"])
            .set_file_name(&suggested_name)
            .blocking_save_file();
        let Some(selected) = selected else {
            let document = lock_document(&state)?;
            return Ok(cancelled_save_result(&document));
        };
        normalize_ddnx_save_path(
            selected
                .into_path()
                .map_err(|error| CommandError::new("invalid_selected_path", error.to_string()))?,
        )?
    };

    if intent.requires_overwrite_confirmation(has_existing_path, destination.exists()) {
        // This command is async, matching the existing native file-picker boundary;
        // the blocking native dialog therefore does not execute in a synchronous
        // main-thread Tauri command context.
        let confirmed = app
            .dialog()
            .message(format!(
                "{} already exists. Replace it with the current DiagramDesigner Next document?",
                destination.display()
            ))
            .title("Replace existing DDNX file?")
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::YesNo)
            .blocking_show();
        if !confirmed {
            let document = lock_document(&state)?;
            return Ok(cancelled_save_result(&document));
        }
    }

    let report = atomic_save(&destination, prepared.bytes()).map_err(map_atomic_save_error)?;

    let mut document = lock_document(&state)?;
    if !document.session.acknowledge_document_saved(prepared.key()) {
        return Err(CommandError::committed(
            "stale_save_completion",
            "The file was written, but the document session was replaced before the save completed.",
        ));
    }
    if intent.updates_persistent_path(has_existing_path) {
        document.path = Some(destination);
    }
    document.imported_dirty = false;
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

fn cancelled_save_result(document: &DesktopDocument) -> SaveResultDto {
    SaveResultDto {
        cancelled: true,
        state: document_state_dto(document),
        commit_mode: None,
        durability: None,
        cleanup_warning: None,
    }
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
        source_path: None,
        imported_dirty: false,
        recovered_dirty: true,
    };
    clear_application_clipboard(&state)?;

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
    let mut document = lock_document(&state)?;
    sync_recovery_for_document(&app, &mut document)
}

fn sync_recovery_for_document(
    app: &AppHandle,
    document: &mut DesktopDocument,
) -> Result<RecoverySyncDto, CommandError> {
    let path = recovery_path(app)?;

    // A restored copy intentionally has no user-visible persisted baseline. While
    // it is in that state, always checkpoint the exact current document directly.
    // This also handles Edit -> Undo back to the recovered initial history state,
    // which editor-core correctly considers clean relative to its decoded baseline
    // but which is still unsaved from the user's perspective.
    if document.recovered_dirty || document.imported_dirty {
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

fn document_navigation_dto(document: &DesktopDocument) -> DocumentNavigationDto {
    let session = document.session.session();
    let pages = session
        .document()
        .pages
        .iter()
        .map(|page| PageNavigationDto {
            page_id: page.id,
            name: page.name.clone(),
            size_mm: page.size_mm,
            layers: page
                .layers
                .iter()
                .map(|layer| LayerNavigationDto {
                    layer_id: layer.id,
                    name: layer.name.clone(),
                    visible: layer.visible,
                    locked: layer.locked,
                    element_count: layer.scene.elements.len(),
                })
                .collect(),
        })
        .collect();
    DocumentNavigationDto {
        state: document_state_dto(document),
        pages,
        active_page_id: session.active_page_id(),
        active_layer_id: document.session.active_page_layer_id(),
    }
}

fn structure_name(value: &str) -> Result<String, CommandError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CommandError::new(
            "invalid_structure_name",
            "Page and layer names must not be empty.",
        ));
    }
    Ok(trimmed.to_owned())
}

fn element_edit_result_dto(document: &DesktopDocument) -> ElementEditResultDto {
    ElementEditResultDto {
        state: document_state_dto(document),
        selected_element_ids: document
            .session
            .session()
            .selection()
            .iter()
            .copied()
            .collect(),
    }
}

fn find_element(document: &Document, element_id: ElementId) -> Option<&Element> {
    document
        .master_layers
        .iter()
        .chain(document.pages.iter().flat_map(|page| page.layers.iter()))
        .find_map(|layer| {
            layer
                .scene
                .elements
                .iter()
                .find(|element| element.id == element_id)
        })
}

fn element_properties_dto(
    element: &Element,
    connector: Option<AppConnectorEndpoints>,
    document: &Document,
) -> ElementPropertiesDto {
    let (text, text_editable, text_style) = match element.text.as_ref() {
        Some(block) => {
            let (preview, editable, common_style) = text_preview(block);
            let style = editable.then(|| text_style_dto(common_style.unwrap_or_default()));
            (Some(preview), editable, style)
        }
        None => (None, false, None),
    };
    ElementPropertiesDto {
        element_id: element.id,
        name: element.name.clone(),
        element_type: element_type_name(&element.kind),
        bounds_mm: element.bounds_mm,
        rotation_deg: element.rotation_deg,
        text,
        text_editable,
        text_style,
        geometry_editable: element_geometry_editable(&element.kind),
        appearance: element_appearance_dto(element, document),
        connector: connector.and_then(connector_properties_dto),
    }
}

fn appearance_applicability(kind: &ElementKind) -> (bool, bool, bool) {
    let shape = matches!(
        kind,
        ElementKind::Rectangle { .. }
            | ElementKind::Ellipse
            | ElementKind::Polygon { .. }
            | ElementKind::Flowchart { .. }
    );
    let text = matches!(kind, ElementKind::Text);
    (shape, shape, text)
}

fn materialized_element_appearance(
    element: &Element,
    document: &Document,
) -> (Option<StrokeStyle>, Option<FillStyle>, Option<Color>) {
    if let Some(style) = element
        .style_id
        .and_then(|style_id| document.styles.iter().find(|style| style.id == style_id))
    {
        return (style.stroke.clone(), style.fill.clone(), style.text_color);
    }
    let (stroke_applicable, _, text_applicable) = appearance_applicability(&element.kind);
    (
        stroke_applicable.then(default_stroke),
        None,
        text_applicable.then(default_black),
    )
}

fn element_appearance_dto(element: &Element, document: &Document) -> ElementAppearanceDto {
    let (stroke_applicable, fill_applicable, text_color_applicable) =
        appearance_applicability(&element.kind);
    let (stroke, fill, text_color) = materialized_element_appearance(element, document);
    let fallback_fill_color = Color::Rgba {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    let displayed_fill_color = fill
        .as_ref()
        .map(|fill| fill.color)
        .unwrap_or(fallback_fill_color);
    let (fill_gradient_enabled, fill_gradient_end_color, fill_gradient_axis) = fill
        .as_ref()
        .and_then(|fill| fill.gradient.as_ref())
        .map(|gradient| (true, color_to_hex(gradient.end_color), gradient.axis))
        .unwrap_or_else(|| {
            (
                false,
                color_to_hex(displayed_fill_color),
                GradientAxis::AlongX,
            )
        });
    ElementAppearanceDto {
        stroke_applicable,
        stroke_enabled: stroke.is_some(),
        stroke_color: color_to_hex(
            stroke
                .as_ref()
                .map(|stroke| stroke.color)
                .unwrap_or_else(default_black),
        ),
        stroke_width_mm: stroke
            .as_ref()
            .map(|stroke| stroke.width_mm)
            .unwrap_or(0.25),
        fill_applicable,
        fill_enabled: fill.is_some(),
        fill_color: color_to_hex(displayed_fill_color),
        fill_gradient_enabled,
        fill_gradient_end_color,
        fill_gradient_axis,
        text_color_applicable,
        text_color: color_to_hex(text_color.unwrap_or_else(default_black)),
    }
}

fn default_black() -> Color {
    Color::Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    }
}

fn default_stroke() -> StrokeStyle {
    StrokeStyle {
        width_mm: 0.25,
        color: default_black(),
    }
}

fn default_fill() -> FillStyle {
    FillStyle {
        color: Color::Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        },
        gradient: None,
    }
}

fn default_linear_gradient(start_color: Color) -> LinearGradient {
    LinearGradient {
        end_color: start_color,
        axis: GradientAxis::AlongX,
    }
}

fn color_to_hex(color: Color) -> String {
    match color {
        Color::Rgba { r, g, b, .. } => format!("#{r:02x}{g:02x}{b:02x}"),
        // System colours are intentionally kept in the domain until the user changes
        // that field. The picker shows the renderer's neutral fallback only.
        Color::SystemPalette { .. } => "#808080".to_owned(),
    }
}

fn parse_rgb_color(value: &str) -> Result<Color, CommandError> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CommandError::new(
            "invalid_color",
            "Colours must use six-digit RGB notation such as #336699.",
        ));
    }
    let parse = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&hex[range], 16)
            .map_err(|_| CommandError::new("invalid_color", "Colour could not be parsed."))
    };
    Ok(Color::Rgba {
        r: parse(0..2)?,
        g: parse(2..4)?,
        b: parse(4..6)?,
        a: 255,
    })
}

fn connector_properties_dto(connector: AppConnectorEndpoints) -> Option<ConnectorPropertiesDto> {
    let kind = match connector.kind {
        AppConnectorGeometryKind::Straight => "straight",
        AppConnectorGeometryKind::Orthogonal => "orthogonal",
        AppConnectorGeometryKind::Curve => return None,
    };
    Some(ConnectorPropertiesDto {
        kind,
        start: connector_endpoint_dto(connector.start),
        end: connector_endpoint_dto(connector.end),
        start_marker: connector.start_marker,
        end_marker: connector.end_marker,
        line_style: connector.line_style,
        secondary_color: connector.secondary_color,
    })
}

fn connector_endpoint_dto(endpoint: AppConnectorEndpointState) -> ConnectorEndpointDto {
    ConnectorEndpointDto {
        position_mm: endpoint.position_mm,
        connection: endpoint
            .connection
            .map(|connection| ConnectorConnectionDto {
                element_id: connection.element_id,
                port_id: connection.port_id,
            }),
    }
}

fn element_geometry_editable(kind: &ElementKind) -> bool {
    !matches!(
        kind,
        ElementKind::StraightConnector { .. }
            | ElementKind::OrthogonalConnector { .. }
            | ElementKind::Group { .. }
    )
}

fn element_type_name(kind: &ElementKind) -> &'static str {
    match kind {
        ElementKind::Text => "Text",
        ElementKind::Rectangle { .. } => "Rectangle",
        ElementKind::Ellipse => "Ellipse",
        ElementKind::StraightConnector { .. } => "Straight connector",
        ElementKind::OrthogonalConnector { .. } => "Orthogonal connector",
        ElementKind::Image { .. } => "Image",
        ElementKind::Metafile { .. } => "Metafile",
        ElementKind::Group { .. } => "Group",
        ElementKind::Polygon { .. } => "Polygon",
        ElementKind::Flowchart { .. } => "Flowchart",
        ElementKind::Curve { .. } => "Curve",
        ElementKind::LayerReference { .. } => "Layer reference",
    }
}

fn text_style_dto(style: TextStyle) -> TextStyleDto {
    TextStyleDto {
        bold: style.bold,
        italic: style.italic,
        underline: style.underline,
        strikeout: style.strikeout,
        script: style.script,
        overline: style.overline,
        symbol_font: style.symbol_font,
        font_family: style.font_family,
        font_size_pt: style.font_size_pt,
        color: style.color,
    }
}

fn text_style_from_dto(style: TextStyleDto) -> Result<TextStyle, CommandError> {
    if style.font_size_pt == Some(0) {
        return Err(CommandError::new(
            "invalid_text_font_size",
            "Text font size must be a positive whole number of points.",
        ));
    }
    let font_family = style.font_family.and_then(|family| {
        let trimmed = family.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    });
    Ok(TextStyle {
        bold: style.bold,
        italic: style.italic,
        underline: style.underline,
        strikeout: style.strikeout,
        script: style.script,
        overline: style.overline,
        symbol_font: style.symbol_font,
        font_family,
        font_size_pt: style.font_size_pt,
        color: style.color,
    })
}

fn text_preview(block: &TextBlock) -> (String, bool, Option<TextStyle>) {
    let mut preview = String::new();
    let mut common_style: Option<TextStyle> = None;
    let mut editable = block.content.tail.is_none() && block.content.diagnostics.is_empty();
    for token in &block.content.tokens {
        match token {
            RichTextToken::Text { text, style } => {
                preview.push_str(text);
                if let Some(existing) = common_style.as_ref() {
                    if existing != style {
                        editable = false;
                    }
                } else {
                    common_style = Some(style.clone());
                }
            }
            RichTextToken::NewLine => preview.push('\n'),
            RichTextToken::PageNumber { .. } => {
                preview.push_str("{page}");
                editable = false;
            }
            RichTextToken::PageCount { .. } => {
                preview.push_str("{pages}");
                editable = false;
            }
            RichTextToken::PageName { .. } => {
                preview.push_str("{page name}");
                editable = false;
            }
            RichTextToken::SymbolGlyph { legacy_glyph, .. } => {
                preview.push(*legacy_glyph);
                editable = false;
            }
        }
    }
    (preview, editable, common_style)
}

fn clamp_connector_point(point: Point, page_size: Size) -> Result<Point, CommandError> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return Err(CommandError::new(
            "invalid_connector_geometry",
            "Connector endpoints must contain finite coordinates.",
        ));
    }
    Ok(Point {
        x: point.x.clamp(0.0, page_size.width),
        y: point.y.clamp(0.0, page_size.height),
    })
}

fn connector_creation_endpoint(
    position_mm: Point,
    connection: Option<Connection>,
    page_size: Size,
    ports: &[app_core::ConnectorPortPosition],
) -> Result<Endpoint, CommandError> {
    let Some(connection) = connection else {
        return Ok(Endpoint {
            position_mm: clamp_connector_point(position_mm, page_size)?,
            connection: None,
        });
    };
    let port = ports
        .iter()
        .find(|port| port.element_id == connection.element_id && port.port_id == connection.port_id)
        .ok_or_else(|| {
            CommandError::new(
                "connector_port_missing",
                "The requested connector port is no longer available on the active editable layer.",
            )
        })?;
    Ok(Endpoint {
        position_mm: port.position_mm,
        connection: Some(connection),
    })
}

fn connector_bounds(start_mm: Point, end_mm: Point) -> Rect {
    Rect {
        x: start_mm.x.min(end_mm.x),
        y: start_mm.y.min(end_mm.y),
        width: (start_mm.x - end_mm.x).abs().max(0.1),
        height: (start_mm.y - end_mm.y).abs().max(0.1),
    }
}

fn default_shape_ports() -> Vec<Port> {
    [
        (0_u16, 0.5, 0.0),
        (1_u16, 1.0, 0.5),
        (2_u16, 0.5, 1.0),
        (3_u16, 0.0, 0.5),
    ]
    .into_iter()
    .map(|(index, x, y)| Port {
        id: PortId::new(),
        index,
        position: NormalizedPoint { x, y },
    })
    .collect()
}

fn simple_text_block(text: &str, style: TextStyle, layout: Option<TextLayout>) -> TextBlock {
    let mut tokens = Vec::new();
    let mut lines = text.split('\n').peekable();
    while let Some(line) = lines.next() {
        tokens.push(RichTextToken::Text {
            text: line.to_owned(),
            style: style.clone(),
        });
        if lines.peek().is_some() {
            tokens.push(RichTextToken::NewLine);
        }
    }
    TextBlock {
        content: RichTextDocument {
            tokens,
            tail: None,
            diagnostics: Vec::new(),
        },
        layout: layout.unwrap_or(TextLayout {
            horizontal: TextHorizontalAlignment::Left,
            vertical: TextVerticalAlignment::Top,
            margin_mm: 1.0,
        }),
    }
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

fn document_is_dirty(document: &DesktopDocument) -> bool {
    document.recovered_dirty || document.imported_dirty || document.session.is_dirty()
}

fn reject_if_dirty(document: &DesktopDocument) -> Result<(), CommandError> {
    if document_is_dirty(document) {
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
        source_path: document
            .source_path
            .as_deref()
            .map(|path| path.to_string_lossy().into_owned()),
        imported: document.source_path.is_some(),
        dirty: document_is_dirty(document),
        recovered: document.recovered_dirty,
        version: env!("CARGO_PKG_VERSION"),
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

fn report_close_checkpoint_error(app: &AppHandle, message: &str) {
    eprintln!("DiagramDesigner Next close blocked: {message}");
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let payload = serde_json::to_string(message)
        .unwrap_or_else(|_| "\"Unknown recovery checkpoint error\"".to_owned());
    let _ = window.eval(format!(
        "window.diagramDesignerNext?.reportCloseCheckpointError({payload});"
    ));
}

fn checkpoint_dirty_document_before_close(window: &tauri::Window, api: &tauri::CloseRequestApi) {
    if window.label() != "main" {
        return;
    }
    let app = window.app_handle();
    let state = app.state::<DesktopState>();
    let mut document = match state.document.lock() {
        Ok(document) => document,
        Err(_) => {
            api.prevent_close();
            report_close_checkpoint_error(
                app,
                "The document state could not be locked for the final recovery checkpoint.",
            );
            return;
        }
    };
    if !document_is_dirty(&document) {
        return;
    }
    if let Err(error) = sync_recovery_for_document(app, &mut document) {
        api.prevent_close();
        report_close_checkpoint_error(app, &error.message);
    }
}

fn desktop_document_defaults() -> DocumentDefaults {
    DocumentDefaults {
        font_family: "Arial".to_owned(),
        font_size_pt: 10.0,
        font_style_bits: 0,
        object_shadows: false,
        auto_line_break: true,
        connector_label_style: ConnectorLabelStyle::Transparent,
    }
}

fn empty_layer(name: String) -> Layer {
    Layer {
        id: LayerId::new(),
        name,
        visible: true,
        locked: false,
        draw_color: None,
        scene: Scene::default(),
    }
}

fn empty_page(name: String, size_mm: Size) -> Page {
    Page {
        id: PageId::new(),
        name,
        size_mm,
        layers: vec![empty_layer("Layer 1".to_owned())],
    }
}

fn blank_document_artifact() -> NextArtifact {
    NextArtifact::document(Document {
        id: DocumentId::new(),
        name: UNTITLED_DOCUMENT_NAME.to_owned(),
        defaults: desktop_document_defaults(),
        master_layers: Vec::new(),
        pages: vec![empty_page(
            "Page 1".to_owned(),
            Size {
                width: 210.0,
                height: 297.0,
            },
        )],
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
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                checkpoint_dirty_document_before_close(window, api);
            }
        })
        .invoke_handler(tauri::generate_handler![
            document_state,
            document_navigation,
            activate_page,
            activate_layer,
            create_page,
            delete_page,
            update_page_properties,
            create_layer,
            delete_layer,
            update_layer_properties,
            candidate_page_presentation,
            set_selection,
            selection_properties,
            group_selection,
            ungroup_selection,
            reorder_selection,
            arrange_selection,
            copy_selection,
            paste_selection,
            duplicate_selection,
            create_basic_element,
            create_connector,
            set_connector_endpoint,
            update_connector_style,
            delete_selection,
            update_element_properties,
            update_element_appearance,
            new_document,
            open_document,
            save_document,
            save_as_document,
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
