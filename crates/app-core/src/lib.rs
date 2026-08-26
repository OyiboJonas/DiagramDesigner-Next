use ddnx::{
    HydrationError, PackageIoError, PackageLimits, PersistenceComparisonError, PreparationError,
    compare_persistence, prepare_package, read_package, write_package_to_vec,
};
use editor_core::{
    ConnectorEndpointSide as CoreConnectorEndpointSide,
    ConnectorEndpointSnapshot as CoreConnectorEndpointSnapshot,
    ConnectorGeometryKind as CoreConnectorGeometryKind, EditCommand, EditTransaction, EditorError,
    EditorSession, HistoryStateId, LayerScope, LayerTarget,
    ResolvedPortPosition as CoreResolvedPortPosition,
};
use editor_runtime::{EditorRuntime, RecoveryCheckpointKey, RecoveryPlan};
use next_domain::{
    Color, Connection, Element, ElementId, FillStyle, Layer, LayerId, NextArtifact, Page, PageId,
    Point, PortId, Rect, Size, StrokeStyle, TextBlock,
};
use thiserror::Error;

const INITIAL_DOCUMENT_GENERATION: u64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorEndpointSide {
    Start,
    End,
}

impl From<ConnectorEndpointSide> for CoreConnectorEndpointSide {
    fn from(value: ConnectorEndpointSide) -> Self {
        match value {
            ConnectorEndpointSide::Start => Self::Start,
            ConnectorEndpointSide::End => Self::End,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorGeometryKind {
    Straight,
    Orthogonal,
    Curve,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorEndpointState {
    pub position_mm: Point,
    pub connection: Option<Connection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorEndpoints {
    pub kind: ConnectorGeometryKind,
    pub start: ConnectorEndpointState,
    pub end: ConnectorEndpointState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConnectorPortPosition {
    pub element_id: ElementId,
    pub port_id: PortId,
    pub position_mm: Point,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElementAppearanceUpdate {
    pub element_id: ElementId,
    pub stroke: Option<StrokeStyle>,
    pub fill: Option<FillStyle>,
    pub text_color: Option<Color>,
}

impl From<CoreConnectorEndpointSnapshot> for ConnectorEndpoints {
    fn from(value: CoreConnectorEndpointSnapshot) -> Self {
        let kind = match value.kind {
            CoreConnectorGeometryKind::Straight => ConnectorGeometryKind::Straight,
            CoreConnectorGeometryKind::Orthogonal => ConnectorGeometryKind::Orthogonal,
            CoreConnectorGeometryKind::Curve => ConnectorGeometryKind::Curve,
        };
        Self {
            kind,
            start: ConnectorEndpointState {
                position_mm: value.start.position_mm,
                connection: value.start.connection,
            },
            end: ConnectorEndpointState {
                position_mm: value.end.position_mm,
                connection: value.end.connection,
            },
        }
    }
}

impl From<CoreResolvedPortPosition> for ConnectorPortPosition {
    fn from(value: CoreResolvedPortPosition) -> Self {
        Self {
            element_id: value.element_id,
            port_id: value.port_id,
            position_mm: value.position_mm,
        }
    }
}

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error(transparent)]
    PackageIo(#[from] PackageIoError),
    #[error(transparent)]
    Hydration(#[from] HydrationError),
    #[error(transparent)]
    Preparation(#[from] PreparationError),
    #[error(transparent)]
    PersistenceComparison(#[from] PersistenceComparisonError),
    #[error(transparent)]
    Editor(#[from] EditorError),
    #[error("verified DDNX round-trip changed persistent document semantics: {first_difference:?}")]
    PersistenceMismatch { first_difference: Option<String> },
}

/// Identifies the exact editor state whose bytes were prepared for a user-visible save.
///
/// `HistoryStateId` is local to one `EditorSession`, so the application adds a
/// generation that changes whenever a different document session is installed.
/// A delayed filesystem acknowledgement from a previously opened document can
/// therefore never mark the current document clean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentSaveKey {
    document_generation: u64,
    history_state: HistoryStateId,
}

impl DocumentSaveKey {
    pub fn document_generation(self) -> u64 {
        self.document_generation
    }

    pub fn history_state(self) -> HistoryStateId {
        self.history_state
    }
}

/// DDNX bytes prepared from one immutable persistent editor state.
///
/// Creating this value is not a successful save. The platform/filesystem layer
/// must persist `bytes` through the atomic replacement boundary from ADR-015 and
/// acknowledge `key` only after that operation succeeds.
#[derive(Debug, Clone)]
pub struct PreparedDocumentSave {
    key: DocumentSaveKey,
    bytes: Vec<u8>,
}

impl PreparedDocumentSave {
    pub fn key(&self) -> DocumentSaveKey {
        self.key
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Application-facing document composition layer.
///
/// This is deliberately narrower than exposing `EditorRuntime` mutably to a
/// WebView IPC layer. Frontend gesture code emits typed semantic intents and the
/// desktop adapter calls dedicated methods here. DDNX byte preparation is kept
/// separate from filesystem durability and from the user-visible saved marker.
#[derive(Debug, Clone)]
pub struct ApplicationSession {
    runtime: EditorRuntime,
    document_generation: u64,
    persisted_save_state: HistoryStateId,
}

impl ApplicationSession {
    pub fn from_artifact(artifact: NextArtifact) -> Result<Self, ApplicationError> {
        let session = EditorSession::from_artifact(artifact)?;
        let persisted_save_state = session.current_history_state();
        Ok(Self {
            runtime: EditorRuntime::new(session),
            document_generation: INITIAL_DOCUMENT_GENERATION,
            persisted_save_state,
        })
    }

    pub fn from_ddnx_bytes(bytes: &[u8], limits: PackageLimits) -> Result<Self, ApplicationError> {
        Self::from_artifact(decode_ddnx(bytes, limits)?)
    }

    pub fn session(&self) -> &EditorSession {
        self.runtime.session()
    }

    pub fn document_generation(&self) -> u64 {
        self.document_generation
    }

    /// Application-level dirty state follows the exact history state known to be
    /// on disk, including the case where a save finishes after newer edits exist.
    pub fn is_dirty(&self) -> bool {
        self.runtime.session().current_history_state() != self.persisted_save_state
    }

    pub fn persisted_save_state(&self) -> HistoryStateId {
        self.persisted_save_state
    }

    pub fn replace_from_ddnx_bytes(
        &mut self,
        bytes: &[u8],
        limits: PackageLimits,
    ) -> Result<(), ApplicationError> {
        self.replace_artifact(decode_ddnx(bytes, limits)?)
    }

    pub fn replace_artifact(&mut self, artifact: NextArtifact) -> Result<(), ApplicationError> {
        let session = EditorSession::from_artifact(artifact)?;
        self.runtime.replace_session(session);
        self.document_generation = self.document_generation.wrapping_add(1);
        if self.document_generation == 0 {
            self.document_generation = INITIAL_DOCUMENT_GENERATION;
        }
        self.persisted_save_state = self.runtime.session().current_history_state();
        self.sync_editor_saved_marker();
        Ok(())
    }

    fn execute_edit(&mut self, command: EditCommand) -> Result<bool, ApplicationError> {
        let changed = self.runtime.session_mut().execute(command)?;
        self.sync_editor_saved_marker();
        Ok(changed)
    }

    fn execute_edit_transaction(
        &mut self,
        transaction: EditTransaction,
    ) -> Result<bool, ApplicationError> {
        let changed = self
            .runtime
            .session_mut()
            .execute_transaction(transaction)?;
        self.sync_editor_saved_marker();
        Ok(changed)
    }

    /// Commit the final document-space delta from a completed frontend move
    /// gesture. Raw pointer updates must never call this method.
    pub fn commit_move_elements(
        &mut self,
        element_ids: Vec<ElementId>,
        delta_mm: Point,
    ) -> Result<bool, ApplicationError> {
        self.execute_edit(EditCommand::MoveElements {
            element_ids,
            delta_mm,
        })
    }

    /// Create one element through the editor-core semantic command boundary.
    pub fn create_element(
        &mut self,
        target: LayerTarget,
        element: Element,
        z_index: Option<usize>,
    ) -> Result<bool, ApplicationError> {
        self.execute_edit(EditCommand::CreateElement {
            target,
            element,
            z_index,
        })
    }

    /// Create several top-level elements as one semantic history step.
    ///
    /// Clipboard/paste callers prepare fresh stable identities before crossing this
    /// boundary. editor-core still owns structural validation, atomic rollback and
    /// undo/redo for the complete transaction.
    pub fn create_elements(
        &mut self,
        target: LayerTarget,
        elements: Vec<Element>,
        appearance_updates: Vec<ElementAppearanceUpdate>,
    ) -> Result<bool, ApplicationError> {
        let mut transaction =
            EditTransaction::new(
                elements
                    .into_iter()
                    .map(|element| EditCommand::CreateElement {
                        target,
                        element,
                        z_index: None,
                    }),
            );
        for update in appearance_updates {
            transaction.push(EditCommand::SetElementAppearance {
                element_id: update.element_id,
                stroke: update.stroke,
                fill: update.fill,
                text_color: update.text_color,
            });
        }
        self.execute_edit_transaction(transaction)
    }

    /// Commit one connector endpoint as either a free point or a durable
    /// target-port reference. Connected coordinates are resolved by editor-core.
    pub fn set_connector_endpoint(
        &mut self,
        element_id: ElementId,
        side: ConnectorEndpointSide,
        position_mm: Point,
        connection: Option<Connection>,
    ) -> Result<bool, ApplicationError> {
        self.execute_edit(EditCommand::SetConnectorEndpoint {
            element_id,
            side: side.into(),
            position_mm,
            connection,
        })
    }

    /// Read one connector's canonical endpoint state without exposing editor-core types.
    pub fn connector_endpoints(
        &self,
        element_id: ElementId,
    ) -> Result<Option<ConnectorEndpoints>, ApplicationError> {
        Ok(self
            .runtime
            .session()
            .connector_endpoint_snapshot(element_id)?
            .map(ConnectorEndpoints::from))
    }

    /// Return hit-testable ports only for the active visible, unlocked page-local layer.
    pub fn active_page_layer_ports(&self) -> Result<Vec<ConnectorPortPosition>, ApplicationError> {
        let session = self.runtime.session();
        let Some(page_id) = session.active_page_id() else {
            return Ok(Vec::new());
        };
        let Some(LayerTarget::Page {
            page_id: layer_page_id,
            layer_id,
        }) = session.active_layer()
        else {
            return Ok(Vec::new());
        };
        if layer_page_id != page_id {
            return Ok(Vec::new());
        }
        let page = session
            .document()
            .pages
            .iter()
            .find(|page| page.id == page_id)
            .ok_or(EditorError::PageNotFound(page_id))?;
        let layer = page
            .layers
            .iter()
            .find(|layer| layer.id == layer_id)
            .ok_or(EditorError::LayerNotFound(layer_id))?;
        if !layer.visible || layer.locked {
            return Ok(Vec::new());
        }
        Ok(session
            .resolved_ports(LayerTarget::Page { page_id, layer_id })?
            .into_iter()
            .map(ConnectorPortPosition::from)
            .collect())
    }

    /// Delete a selection as one semantic history step.
    pub fn delete_elements(
        &mut self,
        element_ids: Vec<ElementId>,
    ) -> Result<bool, ApplicationError> {
        self.execute_edit(EditCommand::DeleteElements { element_ids })
    }

    /// Commit bounds, rotation and an optional text replacement atomically.
    ///
    /// `text_update == None` leaves text untouched. `Some(None)` removes the text
    /// block, while `Some(Some(_))` replaces it.
    pub fn commit_element_properties(
        &mut self,
        element_id: ElementId,
        bounds_mm: Rect,
        rotation_deg: f64,
        text_update: Option<Option<TextBlock>>,
    ) -> Result<bool, ApplicationError> {
        let mut transaction = EditTransaction::default();
        transaction.push(EditCommand::SetBounds {
            element_id,
            bounds_mm,
        });
        transaction.push(EditCommand::SetRotation {
            element_id,
            rotation_deg,
        });
        if let Some(text) = text_update {
            transaction.push(EditCommand::SetText { element_id, text });
        }
        self.execute_edit_transaction(transaction)
    }

    /// Apply stroke/fill/text colour as one semantic history step. editor-core owns
    /// the deterministic per-element style identity and never mutates shared styles.
    pub fn set_element_appearance(
        &mut self,
        element_id: ElementId,
        stroke: Option<StrokeStyle>,
        fill: Option<FillStyle>,
        text_color: Option<Color>,
    ) -> Result<bool, ApplicationError> {
        self.execute_edit(EditCommand::SetElementAppearance {
            element_id,
            stroke,
            fill,
            text_color,
        })
    }

    /// Switch the active page without creating a persistent history step.
    pub fn set_active_page(&mut self, page_id: PageId) -> Result<(), ApplicationError> {
        self.runtime.session_mut().set_active_page(page_id)?;
        Ok(())
    }

    /// Return the active page-local layer ID while keeping `LayerTarget` private to app-core.
    pub fn active_page_layer_id(&self) -> Option<LayerId> {
        match self.runtime.session().active_layer()? {
            LayerTarget::Page { layer_id, .. } => Some(layer_id),
            LayerTarget::Master { .. } => None,
        }
    }

    /// Switch the active page-local layer without creating a persistent history step.
    pub fn set_active_page_layer(
        &mut self,
        page_id: PageId,
        layer_id: LayerId,
    ) -> Result<(), ApplicationError> {
        self.runtime
            .session_mut()
            .set_active_layer(LayerTarget::Page { page_id, layer_id })?;
        Ok(())
    }

    pub fn create_page(&mut self, page: Page) -> Result<bool, ApplicationError> {
        self.execute_edit(EditCommand::CreatePage { page, index: None })
    }

    pub fn delete_page(&mut self, page_id: PageId) -> Result<bool, ApplicationError> {
        self.execute_edit(EditCommand::DeletePage { page_id })
    }

    pub fn set_page_properties(
        &mut self,
        page_id: PageId,
        name: String,
        size_mm: Size,
    ) -> Result<bool, ApplicationError> {
        self.execute_edit(EditCommand::SetPageProperties {
            page_id,
            name,
            size_mm,
        })
    }

    pub fn create_page_layer(
        &mut self,
        page_id: PageId,
        layer: Layer,
    ) -> Result<bool, ApplicationError> {
        self.execute_edit(EditCommand::CreateLayer {
            scope: LayerScope::Page { page_id },
            layer,
            index: None,
        })
    }

    pub fn delete_page_layer(
        &mut self,
        page_id: PageId,
        layer_id: LayerId,
    ) -> Result<bool, ApplicationError> {
        self.execute_edit(EditCommand::DeleteLayer {
            target: LayerTarget::Page { page_id, layer_id },
        })
    }

    pub fn set_page_layer_properties(
        &mut self,
        page_id: PageId,
        layer_id: LayerId,
        name: String,
        visible: bool,
        locked: bool,
        draw_color: Option<Color>,
    ) -> Result<bool, ApplicationError> {
        self.execute_edit(EditCommand::SetLayerProperties {
            target: LayerTarget::Page { page_id, layer_id },
            name,
            visible,
            locked,
            draw_color,
        })
    }

    pub fn set_selection<I>(&mut self, element_ids: I) -> Result<(), ApplicationError>
    where
        I: IntoIterator<Item = ElementId>,
    {
        self.runtime.session_mut().set_selection(element_ids)?;
        Ok(())
    }

    pub fn clear_selection(&mut self) {
        self.runtime.session_mut().clear_selection();
    }

    pub fn undo(&mut self) -> Result<bool, ApplicationError> {
        let changed = self.runtime.session_mut().undo()?;
        self.sync_editor_saved_marker();
        Ok(changed)
    }

    pub fn redo(&mut self) -> Result<bool, ApplicationError> {
        let changed = self.runtime.session_mut().redo()?;
        self.sync_editor_saved_marker();
        Ok(changed)
    }

    /// Build and verify a native DDNX payload for the exact current history state.
    ///
    /// The generated bytes are read and hydrated again and compared through the
    /// established persistence-equivalence contract before they are allowed to
    /// reach the platform filesystem service.
    pub fn prepare_document_save(
        &self,
        limits: PackageLimits,
    ) -> Result<PreparedDocumentSave, ApplicationError> {
        let key = DocumentSaveKey {
            document_generation: self.document_generation,
            history_state: self.runtime.session().current_history_state(),
        };
        let artifact = NextArtifact::document(self.runtime.session().document().clone());
        let prepared = prepare_package(&artifact, limits)?;
        let bytes = write_package_to_vec(&prepared, limits)?;
        let hydrated = read_package(&bytes, limits)?.into_artifact()?;
        let comparison = compare_persistence(&artifact, &hydrated)?;
        if !comparison.equivalent {
            return Err(ApplicationError::PersistenceMismatch {
                first_difference: comparison.first_difference,
            });
        }

        Ok(PreparedDocumentSave { key, bytes })
    }

    /// Acknowledge successful atomic persistence of a previously prepared save.
    ///
    /// Same-generation stale acknowledgements are meaningful: they update which
    /// exact history state is on disk but do not mark newer current edits clean.
    /// Cross-document acknowledgements are rejected.
    pub fn acknowledge_document_saved(&mut self, key: DocumentSaveKey) -> bool {
        if key.document_generation != self.document_generation {
            return false;
        }
        self.persisted_save_state = key.history_state;
        self.sync_editor_saved_marker();
        true
    }

    pub fn recovery_plan(&self) -> RecoveryPlan {
        self.runtime.recovery_plan()
    }

    pub fn acknowledge_recovery_written(&mut self, key: RecoveryCheckpointKey) -> bool {
        self.runtime.acknowledge_recovery_written(key)
    }

    pub fn acknowledge_recovery_removed(&mut self, key: RecoveryCheckpointKey) -> bool {
        self.runtime.acknowledge_recovery_removed(key)
    }

    fn sync_editor_saved_marker(&mut self) {
        if self.runtime.session().current_history_state() == self.persisted_save_state
            && self.runtime.session().saved_history_state() != self.persisted_save_state
        {
            self.runtime.session_mut().mark_saved();
        }
    }
}

fn decode_ddnx(bytes: &[u8], limits: PackageLimits) -> Result<NextArtifact, ApplicationError> {
    let package = read_package(bytes, limits)?;
    Ok(package.into_artifact()?)
}

#[cfg(test)]
mod tests {
    use editor_runtime::RecoveryPlan;
    use next_domain::{
        AnchorSet, Connection, Connector, ConnectorLabelStyle, Document, DocumentDefaults,
        DocumentId, Element, ElementKind, Endpoint, Layer, LayerId, LineStyle, MarkerStyle,
        NormalizedPoint, Page, PageId, Port, PortId, Rect, Scene, Size,
    };

    use super::*;

    fn fixture() -> (NextArtifact, ElementId) {
        let element_id = ElementId::new();
        let page_id = PageId::new();
        let layer_id = LayerId::new();
        let rectangle = Element {
            id: element_id,
            name: "Rectangle".to_owned(),
            bounds_mm: Rect {
                x: 10.0,
                y: 20.0,
                width: 30.0,
                height: 15.0,
            },
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports: Vec::new(),
            style_id: None,
            text: None,
            kind: ElementKind::Rectangle {
                corner_radius_mm: 0.0,
            },
            import: None,
        };
        let document = Document {
            id: DocumentId::new(),
            name: "Application fixture".to_owned(),
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
                        roots: vec![element_id],
                        elements: vec![rectangle],
                    },
                }],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        };
        (NextArtifact::document(document), element_id)
    }

    fn encode(artifact: &NextArtifact) -> Vec<u8> {
        let limits = PackageLimits::default();
        let prepared = prepare_package(artifact, limits).unwrap();
        write_package_to_vec(&prepared, limits).unwrap()
    }

    #[test]
    fn connector_endpoint_edit_uses_application_semantic_boundary() {
        let source_id = ElementId::new();
        let target_id = ElementId::new();
        let port_id = PortId::new();
        let page_id = PageId::new();
        let layer_id = LayerId::new();
        let source = Element {
            id: source_id,
            name: "Connector".to_owned(),
            bounds_mm: Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports: Vec::new(),
            style_id: None,
            text: None,
            kind: ElementKind::StraightConnector {
                connector: Connector {
                    start: Endpoint {
                        position_mm: Point { x: 1.0, y: 1.0 },
                        connection: None,
                    },
                    end: Endpoint {
                        position_mm: Point { x: 9.0, y: 9.0 },
                        connection: None,
                    },
                    start_marker: MarkerStyle::None,
                    end_marker: MarkerStyle::None,
                    line_style: LineStyle::Solid,
                    secondary_color: None,
                },
            },
            import: None,
        };
        let target = Element {
            id: target_id,
            name: "Target".to_owned(),
            bounds_mm: Rect {
                x: 20.0,
                y: 30.0,
                width: 10.0,
                height: 10.0,
            },
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports: vec![Port {
                id: port_id,
                index: 0,
                position: NormalizedPoint { x: 1.0, y: 0.5 },
            }],
            style_id: None,
            text: None,
            kind: ElementKind::Rectangle {
                corner_radius_mm: 0.0,
            },
            import: None,
        };
        let document = Document {
            id: DocumentId::new(),
            name: "Connector application test".to_owned(),
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
                name: "Page".to_owned(),
                size_mm: Size {
                    width: 210.0,
                    height: 297.0,
                },
                layers: vec![Layer {
                    id: layer_id,
                    name: "Layer".to_owned(),
                    visible: true,
                    locked: false,
                    draw_color: None,
                    scene: Scene {
                        roots: vec![source_id, target_id],
                        elements: vec![source, target],
                    },
                }],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        };
        let mut application =
            ApplicationSession::from_artifact(NextArtifact::document(document)).unwrap();
        application
            .set_connector_endpoint(
                source_id,
                ConnectorEndpointSide::Start,
                Point { x: -1.0, y: -1.0 },
                Some(Connection {
                    element_id: target_id,
                    port_id,
                }),
            )
            .unwrap();

        let source = &application.session().document().pages[0].layers[0]
            .scene
            .elements[0];
        let ElementKind::StraightConnector { connector } = &source.kind else {
            panic!("expected straight connector")
        };
        assert_eq!(connector.start.connection.unwrap().element_id, target_id);
        assert_eq!(connector.start.position_mm, Point { x: 30.0, y: 35.0 });
        let endpoints = application.connector_endpoints(source_id).unwrap().unwrap();
        assert_eq!(endpoints.kind, ConnectorGeometryKind::Straight);
        assert_eq!(endpoints.start.position_mm, Point { x: 30.0, y: 35.0 });
        assert_eq!(endpoints.start.connection.unwrap().port_id, port_id);
        let ports = application.active_page_layer_ports().unwrap();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].element_id, target_id);
        assert_eq!(ports[0].port_id, port_id);
        assert_eq!(ports[0].position_mm, Point { x: 30.0, y: 35.0 });
        application
            .set_page_layer_properties(page_id, layer_id, "Layer".to_owned(), false, false, None)
            .unwrap();
        assert!(application.active_page_layer_ports().unwrap().is_empty());
        assert!(application.is_dirty());
    }

    #[test]
    fn move_commit_marks_document_dirty_and_drives_recovery() {
        let (artifact, element_id) = fixture();
        let mut app = ApplicationSession::from_artifact(artifact).unwrap();
        app.commit_move_elements(vec![element_id], Point { x: 5.0, y: 0.0 })
            .unwrap();
        assert!(app.is_dirty());

        let prepared = app.prepare_document_save(PackageLimits::default()).unwrap();
        assert!(!prepared.bytes().is_empty());
        assert!(app.is_dirty());
        assert_ne!(
            app.session().current_history_state(),
            app.session().saved_history_state()
        );

        assert!(app.acknowledge_document_saved(prepared.key()));
        assert!(!app.is_dirty());
        assert_eq!(
            app.session().current_history_state(),
            app.session().saved_history_state()
        );
    }

    #[test]
    fn late_save_ack_tracks_exact_disk_state_without_masking_newer_edits() {
        let (artifact, element_id) = fixture();
        let mut app = ApplicationSession::from_artifact(artifact).unwrap();
        app.commit_move_elements(vec![element_id], Point { x: 5.0, y: 0.0 })
            .unwrap();
        let saved_state = app.session().current_history_state();
        let prepared = app.prepare_document_save(PackageLimits::default()).unwrap();

        app.commit_move_elements(vec![element_id], Point { x: 2.0, y: 0.0 })
            .unwrap();
        assert_ne!(app.session().current_history_state(), saved_state);
        assert!(app.acknowledge_document_saved(prepared.key()));
        assert_eq!(app.persisted_save_state(), saved_state);
        assert!(app.is_dirty());

        assert!(app.undo().unwrap());
        assert_eq!(app.session().current_history_state(), saved_state);
        assert!(!app.is_dirty());
        assert_eq!(app.session().saved_history_state(), saved_state);

        assert!(app.redo().unwrap());
        assert!(app.is_dirty());
    }

    #[test]
    fn save_ack_from_replaced_document_generation_is_rejected() {
        let (first, element_id) = fixture();
        let mut app = ApplicationSession::from_artifact(first).unwrap();
        app.commit_move_elements(vec![element_id], Point { x: 1.0, y: 0.0 })
            .unwrap();
        let stale = app
            .prepare_document_save(PackageLimits::default())
            .unwrap()
            .key();
        let generation = app.document_generation();

        let (second, _) = fixture();
        app.replace_artifact(second).unwrap();
        assert_ne!(app.document_generation(), generation);
        assert!(!app.acknowledge_document_saved(stale));
        assert!(!app.is_dirty());
    }

    #[test]
    fn ddnx_open_and_replace_use_verified_document_packages() {
        let (artifact, _) = fixture();
        let bytes = encode(&artifact);
        let mut app =
            ApplicationSession::from_ddnx_bytes(&bytes, PackageLimits::default()).unwrap();
        assert_eq!(app.session().document().name, "Application fixture");

        let (replacement, _) = fixture();
        let replacement_bytes = encode(&replacement);
        app.replace_from_ddnx_bytes(&replacement_bytes, PackageLimits::default())
            .unwrap();
        assert_eq!(app.session().document().name, "Application fixture");
        assert!(!app.is_dirty());
    }

    #[test]
    fn semantic_move_is_one_history_step_and_recovery_remains_separate() {
        let (artifact, element_id) = fixture();
        let mut app = ApplicationSession::from_artifact(artifact).unwrap();
        app.set_selection([element_id]).unwrap();
        let before = app.session().current_history_state();

        assert!(
            app.commit_move_elements(vec![element_id], Point { x: 3.0, y: -2.0 })
                .unwrap()
        );
        let after = app.session().current_history_state();
        assert_ne!(after, before);
        assert_eq!(app.session().selection().len(), 1);
        assert!(matches!(app.recovery_plan(), RecoveryPlan::Write(_)));

        assert!(app.undo().unwrap());
        assert_eq!(app.session().current_history_state(), before);
        assert!(!app.is_dirty());
    }

    #[test]
    fn corrupted_ddnx_is_rejected_before_editor_session_creation() {
        let (artifact, _) = fixture();
        let mut bytes = encode(&artifact);
        let index = bytes.len() / 2;
        bytes[index] ^= 0x7f;

        assert!(ApplicationSession::from_ddnx_bytes(&bytes, PackageLimits::default()).is_err());
    }

    #[test]
    fn constructive_application_commands_share_editor_history_and_dirty_state() {
        let (artifact, _) = fixture();
        let mut app = ApplicationSession::from_artifact(artifact).unwrap();
        let target = app.session().active_layer().unwrap();
        let created_id = ElementId::new();
        let created = Element {
            id: created_id,
            name: "Created rectangle".to_owned(),
            bounds_mm: Rect {
                x: 15.0,
                y: 25.0,
                width: 40.0,
                height: 20.0,
            },
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports: Vec::new(),
            style_id: None,
            text: None,
            kind: ElementKind::Rectangle {
                corner_radius_mm: 0.0,
            },
            import: None,
        };

        let initial = app.session().current_history_state();
        assert!(app.create_element(target, created, None).unwrap());
        let after_create = app.session().current_history_state();
        assert_ne!(after_create, initial);
        assert!(app.is_dirty());
        assert!(
            app.session().document().pages[0].layers[0]
                .scene
                .elements
                .iter()
                .any(|element| element.id == created_id)
        );

        let updated_bounds = Rect {
            x: 30.0,
            y: 35.0,
            width: 55.0,
            height: 28.0,
        };
        assert!(
            app.commit_element_properties(created_id, updated_bounds, 22.5, None)
                .unwrap()
        );
        let created = app.session().document().pages[0].layers[0]
            .scene
            .elements
            .iter()
            .find(|element| element.id == created_id)
            .unwrap();
        assert_eq!(created.bounds_mm, updated_bounds);
        assert_eq!(created.rotation_deg, 22.5);

        assert!(app.delete_elements(vec![created_id]).unwrap());
        assert!(
            !app.session().document().pages[0].layers[0]
                .scene
                .elements
                .iter()
                .any(|element| element.id == created_id)
        );
        assert!(app.undo().unwrap());
        assert!(
            app.session().document().pages[0].layers[0]
                .scene
                .elements
                .iter()
                .any(|element| element.id == created_id)
        );
        assert!(app.undo().unwrap());
        assert!(app.undo().unwrap());
        assert_eq!(app.session().current_history_state(), initial);
        assert!(!app.is_dirty());
    }

    #[test]
    fn appearance_commit_is_one_application_history_step() {
        let (artifact, element_id) = fixture();
        let mut app = ApplicationSession::from_artifact(artifact).unwrap();
        let before = app.session().current_history_state();
        assert!(
            app.set_element_appearance(
                element_id,
                Some(StrokeStyle {
                    width_mm: 0.6,
                    color: Color::Rgba {
                        r: 12,
                        g: 34,
                        b: 56,
                        a: 255
                    },
                }),
                Some(FillStyle {
                    color: Color::Rgba {
                        r: 200,
                        g: 210,
                        b: 220,
                        a: 255
                    },
                    gradient: None,
                }),
                None,
            )
            .unwrap()
        );
        let after = app.session().current_history_state();
        assert_ne!(after, before);
        assert!(app.is_dirty());
        assert!(app.undo().unwrap());
        assert_eq!(app.session().current_history_state(), before);
        assert!(!app.is_dirty());
        assert!(app.redo().unwrap());
        assert_eq!(app.session().current_history_state(), after);
    }

    #[test]
    fn page_and_layer_commands_keep_navigation_transient_and_structure_in_history() {
        let (artifact, _) = fixture();
        let mut app = ApplicationSession::from_artifact(artifact).unwrap();
        let first_page = app.session().active_page_id().unwrap();
        let first_layer = app.active_page_layer_id().unwrap();
        let initial = app.session().current_history_state();

        let second_page = PageId::new();
        let second_layer = LayerId::new();
        assert!(
            app.create_page(Page {
                id: second_page,
                name: "Page 2".to_owned(),
                size_mm: Size {
                    width: 297.0,
                    height: 210.0,
                },
                layers: vec![Layer {
                    id: second_layer,
                    name: "Layer 1".to_owned(),
                    visible: true,
                    locked: false,
                    draw_color: None,
                    scene: Scene::default(),
                }],
            })
            .unwrap()
        );
        let after_page_create = app.session().current_history_state();
        assert_ne!(after_page_create, initial);

        app.set_active_page(second_page).unwrap();
        app.set_active_page_layer(second_page, second_layer)
            .unwrap();
        assert_eq!(app.session().active_page_id(), Some(second_page));
        assert_eq!(app.active_page_layer_id(), Some(second_layer));
        assert_eq!(app.session().current_history_state(), after_page_create);

        let extra_layer = LayerId::new();
        assert!(
            app.create_page_layer(
                second_page,
                Layer {
                    id: extra_layer,
                    name: "Layer 2".to_owned(),
                    visible: true,
                    locked: false,
                    draw_color: None,
                    scene: Scene::default(),
                },
            )
            .unwrap()
        );
        app.set_active_page_layer(second_page, extra_layer).unwrap();
        assert_eq!(app.active_page_layer_id(), Some(extra_layer));

        assert!(
            app.set_page_properties(
                second_page,
                "Landscape".to_owned(),
                Size {
                    width: 320.0,
                    height: 180.0,
                },
            )
            .unwrap()
        );
        assert!(
            app.set_page_layer_properties(
                second_page,
                extra_layer,
                "Annotations".to_owned(),
                false,
                true,
                None,
            )
            .unwrap()
        );
        let page = app
            .session()
            .document()
            .pages
            .iter()
            .find(|page| page.id == second_page)
            .unwrap();
        assert_eq!(page.name, "Landscape");
        let layer = page
            .layers
            .iter()
            .find(|layer| layer.id == extra_layer)
            .unwrap();
        assert_eq!(layer.name, "Annotations");
        assert!(!layer.visible);
        assert!(layer.locked);

        assert!(
            app.set_page_layer_properties(
                second_page,
                extra_layer,
                "Annotations".to_owned(),
                true,
                false,
                None,
            )
            .unwrap()
        );
        assert!(app.delete_page_layer(second_page, extra_layer).unwrap());
        assert!(app.undo().unwrap());
        assert!(
            app.session()
                .document()
                .pages
                .iter()
                .find(|page| page.id == second_page)
                .unwrap()
                .layers
                .iter()
                .any(|layer| layer.id == extra_layer)
        );

        app.set_active_page(first_page).unwrap();
        app.set_active_page_layer(first_page, first_layer).unwrap();
        let before_page_delete = app.session().current_history_state();
        assert!(app.delete_page(second_page).unwrap());
        assert_eq!(app.session().document().pages.len(), 1);
        assert!(app.undo().unwrap());
        assert_eq!(app.session().document().pages.len(), 2);
        assert_eq!(app.session().current_history_state(), before_page_delete);
    }
}
