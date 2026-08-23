use std::collections::VecDeque;

use editor_core::{EditorError, EditorSession, HistoryStateId, PageStateKey};
use next_domain::{NextArtifact, PageId};
use render_plan::{PreparedPage, RenderPlanError};
use thiserror::Error;

const DEFAULT_PREPARED_PAGE_CACHE_CAPACITY: usize = 4;
const INITIAL_SESSION_GENERATION: u64 = 1;

#[derive(Debug, Error)]
pub enum EditorRuntimeError {
    #[error("prepared page cache capacity must be greater than zero")]
    InvalidCacheCapacity,
    #[error(transparent)]
    Editor(#[from] EditorError),
    #[error(transparent)]
    RenderPlan(#[from] RenderPlanError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedPageCacheStats {
    pub capacity: usize,
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub builds: u64,
    pub evictions: u64,
}

/// Identity of one recovery file payload produced by one editor-session lifetime.
///
/// `HistoryStateId` is intentionally session-local, so the runtime adds a session
/// generation before handing recovery work to the platform layer. A delayed write
/// acknowledgement from an older opened document can therefore never make the
/// current document look recovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryCheckpointKey {
    session_generation: u64,
    history_state: HistoryStateId,
}

impl RecoveryCheckpointKey {
    pub fn session_generation(self) -> u64 {
        self.session_generation
    }

    pub fn history_state(self) -> HistoryStateId {
        self.history_state
    }
}

/// Immutable native-document snapshot for crash/session recovery.
///
/// Creating or persisting this snapshot does not mark the document saved and does
/// not add an undo/redo entry. The platform adapter may serialize the contained
/// `NextArtifact` as DDNX and acknowledge the exact key afterwards.
#[derive(Debug, Clone)]
pub struct RecoverySnapshot {
    key: RecoveryCheckpointKey,
    artifact: NextArtifact,
}

impl RecoverySnapshot {
    pub fn key(&self) -> RecoveryCheckpointKey {
        self.key
    }

    pub fn artifact(&self) -> &NextArtifact {
        &self.artifact
    }

    pub fn into_artifact(self) -> NextArtifact {
        self.artifact
    }
}

/// Recovery work requested from the application/platform layer.
///
/// The runtime never performs filesystem I/O. It only states whether the current
/// recovery file should be left alone, replaced with a fresh DDNX snapshot, or
/// removed because the user-visible document is clean again.
#[derive(Debug, Clone)]
pub enum RecoveryPlan {
    None,
    Write(Box<RecoverySnapshot>),
    Remove(RecoveryCheckpointKey),
}

#[derive(Debug, Clone)]
struct CachedPreparedPage {
    key: PageStateKey,
    page: PreparedPage,
}

/// Small bounded cache of immutable prepared renderer snapshots.
///
/// The cache intentionally belongs to one `EditorRuntime` / `EditorSession`
/// lifetime. `PageStateKey` is session-local persistent-state identity, so a
/// replacement session clears the cache even when a reopened document happens
/// to contain the same persistent page IDs.
#[derive(Debug, Clone)]
struct PreparedPageCache {
    capacity: usize,
    entries: VecDeque<CachedPreparedPage>,
    hits: u64,
    misses: u64,
    builds: u64,
    evictions: u64,
}

impl PreparedPageCache {
    fn new(capacity: usize) -> Result<Self, EditorRuntimeError> {
        if capacity == 0 {
            return Err(EditorRuntimeError::InvalidCacheCapacity);
        }
        Ok(Self {
            capacity,
            entries: VecDeque::with_capacity(capacity),
            hits: 0,
            misses: 0,
            builds: 0,
            evictions: 0,
        })
    }

    fn stats(&self) -> PreparedPageCacheStats {
        PreparedPageCacheStats {
            capacity: self.capacity,
            entries: self.entries.len(),
            hits: self.hits,
            misses: self.misses,
            builds: self.builds,
            evictions: self.evictions,
        }
    }

    fn reset(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
        self.builds = 0;
        self.evictions = 0;
    }

    fn get_or_prepare<'a>(
        &'a mut self,
        session: &EditorSession,
        page_id: PageId,
    ) -> Result<&'a PreparedPage, EditorRuntimeError> {
        let key = session.page_state_key(page_id)?;
        if let Some(position) = self.entries.iter().position(|entry| entry.key == key) {
            self.hits = self.hits.saturating_add(1);
            if position + 1 != self.entries.len() {
                let entry = self
                    .entries
                    .remove(position)
                    .expect("located prepared-page cache entry must exist");
                self.entries.push_back(entry);
            }
            return Ok(&self
                .entries
                .back()
                .expect("prepared-page cache hit must leave one entry")
                .page);
        }

        self.misses = self.misses.saturating_add(1);
        let page = PreparedPage::build(session.document(), page_id)?;
        self.builds = self.builds.saturating_add(1);

        if self.entries.len() == self.capacity {
            self.entries.pop_front();
            self.evictions = self.evictions.saturating_add(1);
        }
        self.entries.push_back(CachedPreparedPage { key, page });

        Ok(&self
            .entries
            .back()
            .expect("prepared-page cache insert must leave one entry")
            .page)
    }
}

/// Application-layer composition of editor state and renderer-derived caches.
///
/// `editor-core` remains renderer independent and `render-plan` remains unaware
/// of editor history. This runtime is the first layer allowed to combine both.
/// Persistent editor commands naturally select a new `PageStateKey`; transient
/// selection/view state continues to reuse the same prepared page.
///
/// Recovery tracking also lives here rather than in editor history. A recovery
/// checkpoint is an external persistence fact, not a user edit and not a successful
/// user-visible save.
#[derive(Debug, Clone)]
pub struct EditorRuntime {
    session: EditorSession,
    prepared_pages: PreparedPageCache,
    session_generation: u64,
    persisted_recovery: Option<RecoveryCheckpointKey>,
}

impl EditorRuntime {
    pub fn new(session: EditorSession) -> Self {
        Self::with_cache_capacity(session, DEFAULT_PREPARED_PAGE_CACHE_CAPACITY)
            .expect("default prepared-page cache capacity is non-zero")
    }

    pub fn with_cache_capacity(
        session: EditorSession,
        capacity: usize,
    ) -> Result<Self, EditorRuntimeError> {
        Ok(Self {
            session,
            prepared_pages: PreparedPageCache::new(capacity)?,
            session_generation: INITIAL_SESSION_GENERATION,
            persisted_recovery: None,
        })
    }

    pub fn session(&self) -> &EditorSession {
        &self.session
    }

    /// Mutable access still cannot bypass the editor command boundary because
    /// `EditorSession` does not expose mutable document access.
    pub fn session_mut(&mut self) -> &mut EditorSession {
        &mut self.session
    }

    /// Replace the complete editor session, for example after opening another
    /// document. Derived renderer snapshots are never reused across sessions.
    ///
    /// A previously acknowledged recovery checkpoint is deliberately retained
    /// until the platform layer removes or replaces it. The generation changes,
    /// so stale acknowledgements cannot be confused with the new document.
    pub fn replace_session(&mut self, session: EditorSession) {
        self.session = session;
        self.prepared_pages.reset();
        self.session_generation = self.session_generation.wrapping_add(1);
        if self.session_generation == 0 {
            self.session_generation = INITIAL_SESSION_GENERATION;
        }
    }

    pub fn prepared_page(&mut self, page_id: PageId) -> Result<&PreparedPage, EditorRuntimeError> {
        let session = &self.session;
        self.prepared_pages.get_or_prepare(session, page_id)
    }

    pub fn prepared_active_page(&mut self) -> Result<Option<&PreparedPage>, EditorRuntimeError> {
        let Some(page_id) = self.session.active_page_id() else {
            return Ok(None);
        };
        self.prepared_page(page_id).map(Some)
    }

    pub fn prepared_page_cache_stats(&self) -> PreparedPageCacheStats {
        self.prepared_pages.stats()
    }

    /// Compute the next recovery action without mutating editor history or saved
    /// state. Call this on an autosave cadence or before lifecycle suspension.
    pub fn recovery_plan(&self) -> RecoveryPlan {
        let current_key = RecoveryCheckpointKey {
            session_generation: self.session_generation,
            history_state: self.session.current_history_state(),
        };

        if self.session.is_dirty() {
            if self.persisted_recovery == Some(current_key) {
                return RecoveryPlan::None;
            }

            return RecoveryPlan::Write(Box::new(RecoverySnapshot {
                key: current_key,
                artifact: NextArtifact::document(self.session.document().clone()),
            }));
        }

        self.persisted_recovery
            .map(RecoveryPlan::Remove)
            .unwrap_or(RecoveryPlan::None)
    }

    /// Acknowledge that the platform layer durably persisted the supplied recovery
    /// snapshot. Stale acknowledgements from a previous opened document are ignored.
    /// A same-session stale history acknowledgement is accepted intentionally: it
    /// records what is actually on disk, and `recovery_plan()` will immediately
    /// request a newer snapshot when the editor has advanced meanwhile.
    pub fn acknowledge_recovery_written(&mut self, key: RecoveryCheckpointKey) -> bool {
        if key.session_generation != self.session_generation {
            return false;
        }
        self.persisted_recovery = Some(key);
        true
    }

    /// Acknowledge successful deletion of the exact recovery payload currently
    /// believed to be on disk. Mismatched/stale deletes do not clear newer state.
    pub fn acknowledge_recovery_removed(&mut self, key: RecoveryCheckpointKey) -> bool {
        if self.persisted_recovery != Some(key) {
            return false;
        }
        self.persisted_recovery = None;
        true
    }

    pub fn persisted_recovery_key(&self) -> Option<RecoveryCheckpointKey> {
        self.persisted_recovery
    }
}

#[cfg(test)]
mod tests {
    use editor_core::EditCommand;
    use next_domain::{
        AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
        ElementKind, Layer, LayerId, NextArtifact, Page, PageId, Point, Rect, Scene, Size,
    };

    use super::*;

    fn defaults() -> DocumentDefaults {
        DocumentDefaults {
            font_family: "Arial".to_owned(),
            font_size_pt: 10.0,
            font_style_bits: 0,
            object_shadows: false,
            auto_line_break: true,
            connector_label_style: ConnectorLabelStyle::Transparent,
        }
    }

    fn rectangle(id: ElementId) -> Element {
        Element {
            id,
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
        }
    }

    fn fixture_session() -> (EditorSession, ElementId, PageId) {
        let element_id = ElementId::new();
        let page_id = PageId::new();
        let layer_id = LayerId::new();
        let document = Document {
            id: DocumentId::new(),
            name: "Runtime test".to_owned(),
            defaults: defaults(),
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
                        roots: vec![element_id],
                        elements: vec![rectangle(element_id)],
                    },
                }],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        };
        (
            EditorSession::from_artifact(NextArtifact::document(document)).unwrap(),
            element_id,
            page_id,
        )
    }

    fn move_element(runtime: &mut EditorRuntime, element_id: ElementId, x: f64) {
        runtime
            .session_mut()
            .execute(EditCommand::MoveElements {
                element_ids: vec![element_id],
                delta_mm: Point { x, y: 0.0 },
            })
            .unwrap();
    }

    #[test]
    fn transient_state_reuses_snapshot_and_history_restores_cached_snapshots() {
        let (session, element_id, page_id) = fixture_session();
        let mut runtime = EditorRuntime::new(session);

        runtime.prepared_page(page_id).unwrap();
        assert_eq!(
            runtime.prepared_page_cache_stats(),
            PreparedPageCacheStats {
                capacity: DEFAULT_PREPARED_PAGE_CACHE_CAPACITY,
                entries: 1,
                hits: 0,
                misses: 1,
                builds: 1,
                evictions: 0,
            }
        );

        runtime.session_mut().set_selection([element_id]).unwrap();
        runtime.session_mut().mark_saved();
        runtime.prepared_page(page_id).unwrap();
        assert_eq!(runtime.prepared_page_cache_stats().hits, 1);
        assert_eq!(runtime.prepared_page_cache_stats().builds, 1);

        runtime
            .session_mut()
            .execute(EditCommand::MoveElements {
                element_ids: vec![element_id],
                delta_mm: Point { x: 5.0, y: 3.0 },
            })
            .unwrap();
        runtime.prepared_page(page_id).unwrap();
        assert_eq!(runtime.prepared_page_cache_stats().misses, 2);
        assert_eq!(runtime.prepared_page_cache_stats().builds, 2);
        assert_eq!(runtime.prepared_page_cache_stats().entries, 2);

        runtime.session_mut().undo().unwrap();
        runtime.prepared_page(page_id).unwrap();
        assert_eq!(runtime.prepared_page_cache_stats().hits, 2);
        assert_eq!(runtime.prepared_page_cache_stats().builds, 2);

        runtime.session_mut().redo().unwrap();
        runtime.prepared_page(page_id).unwrap();
        assert_eq!(runtime.prepared_page_cache_stats().hits, 3);
        assert_eq!(runtime.prepared_page_cache_stats().builds, 2);
    }

    #[test]
    fn cache_capacity_bounds_history_snapshots() {
        let (session, element_id, page_id) = fixture_session();
        let mut runtime = EditorRuntime::with_cache_capacity(session, 1).unwrap();

        runtime.prepared_page(page_id).unwrap();
        runtime
            .session_mut()
            .execute(EditCommand::MoveElements {
                element_ids: vec![element_id],
                delta_mm: Point { x: 1.0, y: 0.0 },
            })
            .unwrap();
        runtime.prepared_page(page_id).unwrap();
        assert_eq!(runtime.prepared_page_cache_stats().evictions, 1);
        assert_eq!(runtime.prepared_page_cache_stats().entries, 1);

        runtime.session_mut().undo().unwrap();
        runtime.prepared_page(page_id).unwrap();
        let stats = runtime.prepared_page_cache_stats();
        assert_eq!(stats.misses, 3);
        assert_eq!(stats.builds, 3);
        assert_eq!(stats.evictions, 2);
        assert_eq!(stats.entries, 1);
    }

    #[test]
    fn replacing_session_clears_even_identical_page_state_identity() {
        let (session, _, page_id) = fixture_session();
        let mut runtime = EditorRuntime::new(session);
        runtime.prepared_page(page_id).unwrap();
        assert_eq!(runtime.prepared_page_cache_stats().entries, 1);

        let document = runtime.session().document().clone();
        let replacement = EditorSession::from_artifact(NextArtifact::document(document)).unwrap();
        assert_eq!(
            replacement.page_state_key(page_id).unwrap(),
            runtime.session().page_state_key(page_id).unwrap()
        );

        runtime.replace_session(replacement);
        assert_eq!(
            runtime.prepared_page_cache_stats(),
            PreparedPageCacheStats {
                capacity: DEFAULT_PREPARED_PAGE_CACHE_CAPACITY,
                entries: 0,
                hits: 0,
                misses: 0,
                builds: 0,
                evictions: 0,
            }
        );

        runtime.prepared_page(page_id).unwrap();
        assert_eq!(runtime.prepared_page_cache_stats().misses, 1);
        assert_eq!(runtime.prepared_page_cache_stats().builds, 1);
    }

    #[test]
    fn recovery_checkpoint_does_not_change_saved_state_or_history() {
        let (session, element_id, _) = fixture_session();
        let mut runtime = EditorRuntime::new(session);
        assert!(matches!(runtime.recovery_plan(), RecoveryPlan::None));

        let saved_before = runtime.session().saved_history_state();
        move_element(&mut runtime, element_id, 2.0);
        let current = runtime.session().current_history_state();
        assert!(runtime.session().is_dirty());
        assert_ne!(current, saved_before);

        let RecoveryPlan::Write(snapshot) = runtime.recovery_plan() else {
            panic!("dirty editor state must request a recovery write");
        };
        let key = snapshot.key();
        assert_eq!(key.history_state(), current);
        assert!(snapshot.artifact().validate().is_valid());
        assert_eq!(runtime.session().saved_history_state(), saved_before);
        assert!(runtime.session().is_dirty());
        assert!(runtime.session().can_undo());

        assert!(runtime.acknowledge_recovery_written(key));
        assert!(matches!(runtime.recovery_plan(), RecoveryPlan::None));
        assert_eq!(runtime.session().saved_history_state(), saved_before);
        assert!(runtime.session().is_dirty());

        runtime.session_mut().mark_saved();
        assert!(!runtime.session().is_dirty());
        let RecoveryPlan::Remove(remove_key) = runtime.recovery_plan() else {
            panic!("clean saved state must request recovery cleanup");
        };
        assert_eq!(remove_key, key);
        assert!(runtime.acknowledge_recovery_removed(remove_key));
        assert!(matches!(runtime.recovery_plan(), RecoveryPlan::None));
    }

    #[test]
    fn stale_same_session_recovery_write_cannot_mask_newer_history() {
        let (session, element_id, _) = fixture_session();
        let mut runtime = EditorRuntime::new(session);

        move_element(&mut runtime, element_id, 1.0);
        let RecoveryPlan::Write(first) = runtime.recovery_plan() else {
            panic!("first edit must request recovery");
        };
        let first_key = first.key();

        move_element(&mut runtime, element_id, 1.0);
        let second_state = runtime.session().current_history_state();
        assert_ne!(first_key.history_state(), second_state);

        assert!(runtime.acknowledge_recovery_written(first_key));
        let RecoveryPlan::Write(second) = runtime.recovery_plan() else {
            panic!("newer editor state must supersede a stale recovery write");
        };
        assert_eq!(second.key().history_state(), second_state);
        assert_ne!(second.key(), first_key);
    }

    #[test]
    fn undo_redo_recovery_tracks_exact_persisted_history_state() {
        let (session, element_id, _) = fixture_session();
        let mut runtime = EditorRuntime::new(session);

        move_element(&mut runtime, element_id, 1.0);
        let RecoveryPlan::Write(first) = runtime.recovery_plan() else {
            panic!("first edit must request recovery");
        };
        let first_key = first.key();
        assert!(runtime.acknowledge_recovery_written(first_key));

        move_element(&mut runtime, element_id, 1.0);
        let RecoveryPlan::Write(second) = runtime.recovery_plan() else {
            panic!("second edit must request newer recovery");
        };
        let second_key = second.key();
        assert!(runtime.acknowledge_recovery_written(second_key));

        runtime.session_mut().undo().unwrap();
        assert_eq!(
            runtime.session().current_history_state(),
            first_key.history_state()
        );
        let RecoveryPlan::Write(undo_snapshot) = runtime.recovery_plan() else {
            panic!("undo to a different on-disk state must request replacement");
        };
        assert_eq!(undo_snapshot.key(), first_key);
        assert!(runtime.acknowledge_recovery_written(first_key));

        runtime.session_mut().redo().unwrap();
        assert_eq!(
            runtime.session().current_history_state(),
            second_key.history_state()
        );
        let RecoveryPlan::Write(redo_snapshot) = runtime.recovery_plan() else {
            panic!("redo must request the exact newer history state again");
        };
        assert_eq!(redo_snapshot.key(), second_key);
    }

    #[test]
    fn replacing_session_rejects_stale_write_ack_and_requests_old_cleanup() {
        let (session, element_id, _) = fixture_session();
        let mut runtime = EditorRuntime::new(session);
        move_element(&mut runtime, element_id, 1.0);
        let RecoveryPlan::Write(snapshot) = runtime.recovery_plan() else {
            panic!("dirty state must request recovery");
        };
        let old_key = snapshot.key();
        assert!(runtime.acknowledge_recovery_written(old_key));

        let (replacement, _, _) = fixture_session();
        runtime.replace_session(replacement);
        assert!(!runtime.session().is_dirty());
        assert!(!runtime.acknowledge_recovery_written(old_key));

        let RecoveryPlan::Remove(remove_key) = runtime.recovery_plan() else {
            panic!("opening a clean replacement session must clean stale recovery");
        };
        assert_eq!(remove_key, old_key);
        assert!(runtime.acknowledge_recovery_removed(remove_key));
        assert!(matches!(runtime.recovery_plan(), RecoveryPlan::None));
    }

    #[test]
    fn zero_capacity_is_rejected() {
        let (session, _, _) = fixture_session();
        assert!(matches!(
            EditorRuntime::with_cache_capacity(session, 0),
            Err(EditorRuntimeError::InvalidCacheCapacity)
        ));
    }
}
