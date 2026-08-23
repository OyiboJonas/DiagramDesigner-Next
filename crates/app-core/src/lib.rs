use ddnx::{
    HydrationError, PackageIoError, PackageLimits, PersistenceComparisonError, PreparationError,
    compare_persistence, prepare_package, read_package, write_package_to_vec,
};
use editor_core::{EditCommand, EditorError, EditorSession, HistoryStateId};
use editor_runtime::{EditorRuntime, RecoveryCheckpointKey, RecoveryPlan};
use next_domain::{ElementId, NextArtifact, Point};
use thiserror::Error;

const INITIAL_DOCUMENT_GENERATION: u64 = 1;

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

    /// Commit the final document-space delta from a completed frontend move
    /// gesture. Raw pointer updates must never call this method.
    pub fn commit_move_elements(
        &mut self,
        element_ids: Vec<ElementId>,
        delta_mm: Point,
    ) -> Result<bool, ApplicationError> {
        let changed = self
            .runtime
            .session_mut()
            .execute(EditCommand::MoveElements {
                element_ids,
                delta_mm,
            })?;
        self.sync_editor_saved_marker();
        Ok(changed)
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
        AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element,
        ElementKind, Layer, LayerId, Page, PageId, Rect, Scene, Size,
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
    fn prepared_save_is_verified_but_does_not_mark_dirty_state_clean_before_ack() {
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
}
