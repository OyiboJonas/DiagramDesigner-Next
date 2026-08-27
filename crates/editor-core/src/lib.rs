use std::collections::BTreeSet;

use next_domain::{
    Artifact, Color, Connection, Connector, Document, Element, ElementId, ElementKind,
    ElementStyle, Endpoint, FillStyle, Layer, LayerId, LineStyle, MarkerStyle, NextArtifact, Page,
    PageId, Point, PortId, Rect, Scene, Size, StrokeStyle, StyleId, TextBlock, ValidationReport,
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HistoryStateId(u64);

impl HistoryStateId {
    pub const INITIAL: Self = Self(0);

    pub fn value(self) -> u64 {
        self.0
    }
}

/// Stable key for derived data that belongs to one page of one persistent
/// document history state. Transient selection/viewport state is intentionally
/// excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageStateKey {
    history_state: HistoryStateId,
    page_id: PageId,
}

impl PageStateKey {
    pub fn history_state(self) -> HistoryStateId {
        self.history_state
    }

    pub fn page_id(self) -> PageId {
        self.page_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LayerScope {
    Master,
    Page { page_id: PageId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LayerTarget {
    Master { layer_id: LayerId },
    Page { page_id: PageId, layer_id: LayerId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConnectorEndpointSide {
    Start,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorGeometryKind {
    Straight,
    Orthogonal,
    Curve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZOrderOperation {
    BringToFront,
    SendToBack,
    BringForward,
    SendBackward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrangeOperation {
    AlignLeft,
    AlignHorizontalCenter,
    AlignRight,
    AlignTop,
    AlignVerticalCenter,
    AlignBottom,
    DistributeHorizontal,
    DistributeVertical,
}

impl ArrangeOperation {
    fn minimum_selection(self) -> usize {
        match self {
            Self::DistributeHorizontal | Self::DistributeVertical => 3,
            _ => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorEndpointSnapshot {
    pub kind: ConnectorGeometryKind,
    pub start: Endpoint,
    pub end: Endpoint,
    pub start_marker: MarkerStyle,
    pub end_marker: MarkerStyle,
    pub line_style: LineStyle,
    pub secondary_color: Option<Color>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedPortPosition {
    pub element_id: ElementId,
    pub port_id: PortId,
    pub position_mm: Point,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditCommand {
    CreatePage {
        page: Page,
        index: Option<usize>,
    },
    DeletePage {
        page_id: PageId,
    },
    SetPageProperties {
        page_id: PageId,
        name: String,
        size_mm: Size,
    },
    CreateLayer {
        scope: LayerScope,
        layer: Layer,
        index: Option<usize>,
    },
    DeleteLayer {
        target: LayerTarget,
    },
    SetLayerProperties {
        target: LayerTarget,
        name: String,
        visible: bool,
        locked: bool,
        draw_color: Option<Color>,
    },
    MoveElements {
        element_ids: Vec<ElementId>,
        delta_mm: Point,
    },
    /// Align or distribute direct scene roots from canonical document geometry.
    /// Structural groups participate as one logical object and are expanded only
    /// when the computed movement is committed.
    ArrangeElements {
        element_ids: Vec<ElementId>,
        operation: ArrangeOperation,
    },
    ReorderElements {
        element_ids: Vec<ElementId>,
        operation: ZOrderOperation,
    },
    SetBounds {
        element_id: ElementId,
        bounds_mm: Rect,
    },
    SetRotation {
        element_id: ElementId,
        rotation_deg: f64,
    },
    /// Edit one connector endpoint. `position_mm` is the free endpoint position.
    /// When `connection` is present, editor-core resolves the canonical document
    /// position from the referenced target port instead of trusting view state.
    SetConnectorEndpoint {
        element_id: ElementId,
        side: ConnectorEndpointSide,
        position_mm: Point,
        connection: Option<Connection>,
    },
    /// Replace persisted connector paint semantics as one history command.
    SetConnectorStyle {
        element_id: ElementId,
        start_marker: MarkerStyle,
        end_marker: MarkerStyle,
        line_style: LineStyle,
        secondary_color: Option<Color>,
    },
    SetElementStyle {
        element_ids: Vec<ElementId>,
        style_id: Option<StyleId>,
    },
    /// Replace the visual appearance of one element with an element-owned style.
    /// The deterministic style ID prevents repeated edits from accumulating style records
    /// and guarantees that imported/shared style records are never mutated in place.
    SetElementAppearance {
        element_id: ElementId,
        stroke: Option<StrokeStyle>,
        fill: Option<FillStyle>,
        text_color: Option<Color>,
    },
    SetText {
        element_id: ElementId,
        text: Option<TextBlock>,
    },
    /// Replace a contiguous set of direct siblings with one structural group.
    /// Child geometry remains in document coordinates; grouping itself never
    /// applies a hidden transform.
    GroupElements {
        group_id: ElementId,
        element_ids: Vec<ElementId>,
        name: String,
    },
    /// Reconstruct an exact structural group snapshot. This supports singleton and
    /// empty groups used by imported documents and structured clipboard paste.
    CreateStructuralGroup {
        target: LayerTarget,
        group: Element,
        z_index: Option<usize>,
    },
    /// Remove one structural group while promoting its children into the group's
    /// exact sibling position.
    Ungroup {
        group_id: ElementId,
    },
    /// Create one top-level element in a scene.
    ///
    /// Group construction is intentionally a separate future semantic command.
    /// A non-empty `Group` cannot be smuggled through this primitive because that
    /// would leave child/root ownership ambiguous.
    CreateElement {
        target: LayerTarget,
        element: Element,
        /// Root z-order insertion index. `None` appends at the frontmost end.
        z_index: Option<usize>,
    },
    /// Delete elements as one semantic operation.
    ///
    /// Selecting a group expands to its complete descendant closure. Remaining
    /// connectors are detached from deleted targets while preserving their free
    /// endpoint positions. Deleting a child while its group remains is rejected.
    DeleteElements {
        element_ids: Vec<ElementId>,
    },
}

/// One persistent editor history step.
///
/// Tools may collect several semantic commands and commit them atomically. Adjacent
/// geometric updates for the same target are coalesced inside the transaction before
/// the document is mutated. Transactions do not contain raw pointer events.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EditTransaction {
    commands: Vec<EditCommand>,
}

impl EditTransaction {
    pub fn new<I>(commands: I) -> Self
    where
        I: IntoIterator<Item = EditCommand>,
    {
        let mut transaction = Self::default();
        for command in commands {
            transaction.push(command);
        }
        transaction
    }

    pub fn single(command: EditCommand) -> Self {
        Self::new([command])
    }

    pub fn commands(&self) -> &[EditCommand] {
        &self.commands
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn push(&mut self, command: EditCommand) {
        let mut coalesced = false;
        let mut remove_last = false;

        if let Some(previous) = self.commands.last_mut() {
            match (previous, &command) {
                (
                    EditCommand::MoveElements {
                        element_ids: previous_ids,
                        delta_mm: previous_delta,
                    },
                    EditCommand::MoveElements {
                        element_ids,
                        delta_mm,
                    },
                ) if previous_ids == element_ids => {
                    previous_delta.x += delta_mm.x;
                    previous_delta.y += delta_mm.y;
                    remove_last = previous_delta.x == 0.0 && previous_delta.y == 0.0;
                    coalesced = true;
                }
                (
                    EditCommand::SetBounds {
                        element_id: previous_id,
                        bounds_mm: previous_bounds,
                    },
                    EditCommand::SetBounds {
                        element_id,
                        bounds_mm,
                    },
                ) if previous_id == element_id => {
                    *previous_bounds = *bounds_mm;
                    coalesced = true;
                }
                (
                    EditCommand::SetRotation {
                        element_id: previous_id,
                        rotation_deg: previous_rotation,
                    },
                    EditCommand::SetRotation {
                        element_id,
                        rotation_deg,
                    },
                ) if previous_id == element_id => {
                    *previous_rotation = *rotation_deg;
                    coalesced = true;
                }
                (
                    EditCommand::SetConnectorEndpoint {
                        element_id: previous_id,
                        side: previous_side,
                        position_mm: previous_position,
                        connection: previous_connection,
                    },
                    EditCommand::SetConnectorEndpoint {
                        element_id,
                        side,
                        position_mm,
                        connection,
                    },
                ) if previous_id == element_id && previous_side == side => {
                    *previous_position = *position_mm;
                    *previous_connection = *connection;
                    coalesced = true;
                }
                (
                    EditCommand::SetElementStyle {
                        element_ids: previous_ids,
                        style_id: previous_style_id,
                    },
                    EditCommand::SetElementStyle {
                        element_ids,
                        style_id,
                    },
                ) if previous_ids == element_ids => {
                    *previous_style_id = *style_id;
                    coalesced = true;
                }
                (
                    EditCommand::SetText {
                        element_id: previous_id,
                        text: previous_text,
                    },
                    EditCommand::SetText { element_id, text },
                ) if previous_id == element_id => {
                    *previous_text = text.clone();
                    coalesced = true;
                }
                (
                    EditCommand::SetPageProperties {
                        page_id: previous_id,
                        name: previous_name,
                        size_mm: previous_size,
                    },
                    EditCommand::SetPageProperties {
                        page_id,
                        name,
                        size_mm,
                    },
                ) if previous_id == page_id => {
                    *previous_name = name.clone();
                    *previous_size = *size_mm;
                    coalesced = true;
                }
                (
                    EditCommand::SetLayerProperties {
                        target: previous_target,
                        name: previous_name,
                        visible: previous_visible,
                        locked: previous_locked,
                        draw_color: previous_draw_color,
                    },
                    EditCommand::SetLayerProperties {
                        target,
                        name,
                        visible,
                        locked,
                        draw_color,
                    },
                ) if previous_target == target => {
                    *previous_name = name.clone();
                    *previous_visible = *visible;
                    *previous_locked = *locked;
                    *previous_draw_color = *draw_color;
                    coalesced = true;
                }
                _ => {}
            }
        }

        if remove_last {
            self.commands.pop();
        } else if !coalesced {
            self.commands.push(command);
        }
    }
}

#[derive(Debug, Error)]
pub enum EditorError {
    #[error("editor can open only document artifacts")]
    UnsupportedArtifact,
    #[error("document failed next-domain validation: {0:?}")]
    InvalidDocument(ValidationReport),
    #[error("page {0:?} does not exist")]
    PageNotFound(PageId),
    #[error("page {0:?} already exists")]
    PageAlreadyExists(PageId),
    #[error("page insertion index {index} is outside 0..={len}")]
    InvalidPageIndex { index: usize, len: usize },
    #[error("page size must contain finite positive dimensions")]
    InvalidPageSize,
    #[error("layer {0:?} does not exist in the requested scope")]
    LayerNotFound(LayerId),
    #[error("layer {0:?} already exists in the document")]
    LayerAlreadyExists(LayerId),
    #[error("layer insertion index {index} is outside 0..={len}")]
    InvalidLayerIndex { index: usize, len: usize },
    #[error("element {0:?} does not exist")]
    ElementNotFound(ElementId),
    #[error("element {0:?} already exists")]
    ElementAlreadyExists(ElementId),
    #[error("port {0:?} already exists")]
    PortAlreadyExists(PortId),
    #[error("port {port_id:?} does not exist on element {element_id:?}")]
    PortNotFound {
        element_id: ElementId,
        port_id: PortId,
    },
    #[error("element {0:?} does not expose connector endpoint geometry")]
    ElementIsNotConnector(ElementId),
    #[error(
        "connector {source_element_id:?} and target {target_element_id:?} are not in the same scene"
    )]
    ConnectionTargetDifferentScene {
        source_element_id: ElementId,
        target_element_id: ElementId,
    },
    #[error("layer {0:?} is locked")]
    LayerLocked(LayerId),
    #[error("style {0:?} does not exist")]
    StyleNotFound(StyleId),
    #[error("appearance contains an invalid stroke width")]
    InvalidAppearance,
    #[error("element-owned appearance style {0:?} collides with existing document state")]
    AppearanceStyleCollision(StyleId),
    #[error("text layout contains a non-finite or negative margin")]
    InvalidTextLayout,
    #[error("command contains duplicate element {0:?}")]
    DuplicateCommandElement(ElementId),
    #[error("z-order selection spans more than one layer")]
    ZOrderDifferentLayers,
    #[error("z-order editing currently requires top-level element {0:?}")]
    ZOrderRequiresTopLevelElement(ElementId),
    #[error("arrange selection spans more than one layer")]
    ArrangeDifferentLayers,
    #[error("arrange editing currently requires top-level element {0:?}")]
    ArrangeRequiresTopLevelElement(ElementId),
    #[error("arrange operation requires at least {required} elements; got {actual}")]
    ArrangeRequiresAtLeast { required: usize, actual: usize },
    #[error("z-order index {index} is outside 0..={len}")]
    InvalidZOrderIndex { index: usize, len: usize },
    #[error("non-empty group creation requires the dedicated grouping command")]
    GroupCreationRequiresDedicatedCommand,
    #[error("grouping requires at least two elements")]
    GroupRequiresAtLeastTwoElements,
    #[error("group members must be contiguous direct siblings with the same owner")]
    GroupMembersHaveDifferentOwners,
    #[error("grouping non-contiguous siblings would change z-order")]
    NonContiguousGroupSelection,
    #[error("element {0:?} has ambiguous structural ownership")]
    AmbiguousElementOwnership(ElementId),
    #[error("element {0:?} is not a group")]
    ElementIsNotGroup(ElementId),
    #[error("group hierarchy contains a cycle at {0:?}")]
    GroupHierarchyCycle(ElementId),
    #[error("group {0:?} requires a dedicated affine transform command for resize/rotation")]
    GroupTransformRequiresDedicatedCommand(ElementId),
    #[error("element {element_id:?} is still owned by group {group_id:?}")]
    ElementReferencedByGroup {
        element_id: ElementId,
        group_id: ElementId,
    },
    #[error("command contains non-finite or otherwise invalid geometry")]
    InvalidGeometry,
    #[error("editor history invariant was violated")]
    HistoryInvariantViolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveState {
    page_id: Option<PageId>,
    layer: Option<LayerTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SiblingOwner {
    Roots,
    Group(ElementId),
}

#[derive(Debug, Clone)]
struct DetachedConnection {
    source_element_id: ElementId,
    side: ConnectorEndpointSide,
    connection: Connection,
}

#[derive(Debug, Clone)]
struct RemovedElement {
    target: LayerTarget,
    element: Element,
    element_index: usize,
    root_index: Option<usize>,
}

#[derive(Debug, Clone)]
enum UndoStep {
    RemoveCreatedPage {
        page_id: PageId,
    },
    RestoreDeletedPage {
        page: Page,
        page_index: usize,
        detached: Vec<DetachedConnection>,
    },
    SetPageProperties {
        page_id: PageId,
        name: String,
        size_mm: Size,
    },
    RemoveCreatedLayer {
        scope: LayerScope,
        layer_id: LayerId,
    },
    RestoreDeletedLayer {
        scope: LayerScope,
        layer: Layer,
        layer_index: usize,
        detached: Vec<DetachedConnection>,
    },
    SetLayerProperties {
        target: LayerTarget,
        name: String,
        visible: bool,
        locked: bool,
        draw_color: Option<Color>,
    },
    MoveElements {
        element_ids: Vec<ElementId>,
        delta_mm: Point,
    },
    ArrangeElements {
        movements: Vec<(Vec<ElementId>, Point)>,
    },
    RestoreZOrder {
        target: LayerTarget,
        roots: Vec<ElementId>,
    },
    SetBounds {
        element_id: ElementId,
        bounds_mm: Rect,
    },
    SetRotation {
        element_id: ElementId,
        rotation_deg: f64,
    },
    SetConnectorEndpoint {
        element_id: ElementId,
        side: ConnectorEndpointSide,
        endpoint: Endpoint,
    },
    SetConnectorStyle {
        element_id: ElementId,
        start_marker: MarkerStyle,
        end_marker: MarkerStyle,
        line_style: LineStyle,
        secondary_color: Option<Color>,
    },
    SetElementStyles {
        previous: Vec<(ElementId, Option<StyleId>)>,
    },
    RestoreElementAppearance {
        element_id: ElementId,
        previous_style_id: Option<StyleId>,
        dedicated_style_id: StyleId,
        previous_dedicated_style: Option<ElementStyle>,
    },
    SetText {
        element_id: ElementId,
        text: Option<TextBlock>,
    },
    RemoveCreatedGroup {
        target: LayerTarget,
        owner: SiblingOwner,
        previous_siblings: Vec<ElementId>,
        group_id: ElementId,
    },
    RestoreUngrouped {
        target: LayerTarget,
        owner: SiblingOwner,
        previous_siblings: Vec<ElementId>,
        group: Element,
        element_index: usize,
        detached: Vec<DetachedConnection>,
    },
    RemoveCreated {
        element_id: ElementId,
    },
    RestoreDeleted {
        removed: Vec<RemovedElement>,
        detached: Vec<DetachedConnection>,
    },
}

#[derive(Debug, Clone)]
struct AppliedCommand {
    undo: UndoStep,
    structural: bool,
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    forward: EditTransaction,
    inverse: Vec<UndoStep>,
    before: HistoryStateId,
    after: HistoryStateId,
    active_before: ActiveState,
    active_after: ActiveState,
    topology_changed: bool,
}

#[derive(Debug, Clone)]
struct History {
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    current: HistoryStateId,
    saved: HistoryStateId,
    next_state: u64,
}

impl Default for History {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            current: HistoryStateId::INITIAL,
            saved: HistoryStateId::INITIAL,
            next_state: 1,
        }
    }
}

impl History {
    fn allocate_state(&mut self) -> HistoryStateId {
        let state = HistoryStateId(self.next_state);
        self.next_state = self.next_state.saturating_add(1);
        state
    }
}

/// Renderer- and platform-independent editable document session.
///
/// The document is intentionally not exposed mutably. Persistent mutations go
/// through `EditCommand` / `EditTransaction`, which makes undo/redo and dirty
/// tracking a single global concern instead of a property of individual tools or
/// views.
#[derive(Debug, Clone)]
pub struct EditorSession {
    document: Document,
    active_page_id: Option<PageId>,
    active_layer: Option<LayerTarget>,
    selection: BTreeSet<ElementId>,
    history: History,
}

impl EditorSession {
    pub fn from_artifact(artifact: NextArtifact) -> Result<Self, EditorError> {
        let validation = artifact.validate();
        if !validation.is_valid() {
            return Err(EditorError::InvalidDocument(validation));
        }

        let Artifact::Document(document) = artifact.artifact else {
            return Err(EditorError::UnsupportedArtifact);
        };

        let active_page_id = document.pages.first().map(|page| page.id);
        let active_layer = document
            .pages
            .first()
            .and_then(|page| {
                page.layers.first().map(|layer| LayerTarget::Page {
                    page_id: page.id,
                    layer_id: layer.id,
                })
            })
            .or_else(|| {
                document
                    .master_layers
                    .first()
                    .map(|layer| LayerTarget::Master { layer_id: layer.id })
            });

        Ok(Self {
            document,
            active_page_id,
            active_layer,
            selection: BTreeSet::new(),
            history: History::default(),
        })
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn into_artifact(self) -> NextArtifact {
        NextArtifact::document(self.document)
    }

    pub fn active_page_id(&self) -> Option<PageId> {
        self.active_page_id
    }

    pub fn active_layer(&self) -> Option<LayerTarget> {
        self.active_layer
    }

    pub fn set_active_page(&mut self, page_id: PageId) -> Result<(), EditorError> {
        let page = self
            .document
            .pages
            .iter()
            .find(|page| page.id == page_id)
            .ok_or(EditorError::PageNotFound(page_id))?;

        self.active_page_id = Some(page_id);
        self.active_layer = match self.active_layer {
            Some(LayerTarget::Page {
                page_id: active_page,
                layer_id,
            }) if active_page == page_id
                && page.layers.iter().any(|layer| layer.id == layer_id) =>
            {
                self.active_layer
            }
            _ => page
                .layers
                .first()
                .map(|layer| LayerTarget::Page {
                    page_id,
                    layer_id: layer.id,
                })
                .or_else(|| {
                    self.document
                        .master_layers
                        .first()
                        .map(|layer| LayerTarget::Master { layer_id: layer.id })
                }),
        };
        Ok(())
    }

    pub fn set_active_layer(&mut self, target: LayerTarget) -> Result<(), EditorError> {
        if find_layer(&self.document, target).is_none() {
            return Err(EditorError::LayerNotFound(layer_id_of(target)));
        }
        if let LayerTarget::Page { page_id, .. } = target {
            self.active_page_id = Some(page_id);
        }
        self.active_layer = Some(target);
        Ok(())
    }

    pub fn selection(&self) -> &BTreeSet<ElementId> {
        &self.selection
    }

    pub fn set_selection<I>(&mut self, element_ids: I) -> Result<(), EditorError>
    where
        I: IntoIterator<Item = ElementId>,
    {
        let selection: BTreeSet<_> = element_ids.into_iter().collect();
        for element_id in &selection {
            if find_element_layer(&self.document, *element_id).is_none() {
                return Err(EditorError::ElementNotFound(*element_id));
            }
        }
        self.selection = selection;
        Ok(())
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    /// Read canonical connector endpoint state without exposing mutable document access.
    pub fn connector_endpoint_snapshot(
        &self,
        element_id: ElementId,
    ) -> Result<Option<ConnectorEndpointSnapshot>, EditorError> {
        let element = find_element(&self.document, element_id)
            .ok_or(EditorError::ElementNotFound(element_id))?;
        let (kind, connector) = match &element.kind {
            ElementKind::StraightConnector { connector } => {
                (ConnectorGeometryKind::Straight, connector)
            }
            ElementKind::OrthogonalConnector { connector, .. } => {
                (ConnectorGeometryKind::Orthogonal, connector)
            }
            ElementKind::Curve {
                connector: Some(connector),
                ..
            } => (ConnectorGeometryKind::Curve, connector),
            _ => return Ok(None),
        };
        Ok(Some(ConnectorEndpointSnapshot {
            kind,
            start: connector.start.clone(),
            end: connector.end.clone(),
            start_marker: connector.start_marker,
            end_marker: connector.end_marker,
            line_style: connector.line_style,
            secondary_color: connector.secondary_color,
        }))
    }

    /// Resolve every port in one scene to its canonical document-space position.
    pub fn resolved_ports(
        &self,
        target: LayerTarget,
    ) -> Result<Vec<ResolvedPortPosition>, EditorError> {
        let layer = find_layer(&self.document, target)
            .ok_or(EditorError::LayerNotFound(layer_id_of(target)))?;
        let mut ports = Vec::new();
        for element in &layer.scene.elements {
            for port in &element.ports {
                ports.push(ResolvedPortPosition {
                    element_id: element.id,
                    port_id: port.id,
                    position_mm: port_document_position(element, port.id)?,
                });
            }
        }
        Ok(ports)
    }

    pub fn current_history_state(&self) -> HistoryStateId {
        self.history.current
    }

    /// Return the persistent-state key for a page. Derived caches may reuse data
    /// while this key is unchanged; commands, undo and redo change/restore the
    /// history component automatically.
    pub fn page_state_key(&self, page_id: PageId) -> Result<PageStateKey, EditorError> {
        if !self.document.pages.iter().any(|page| page.id == page_id) {
            return Err(EditorError::PageNotFound(page_id));
        }
        Ok(PageStateKey {
            history_state: self.history.current,
            page_id,
        })
    }

    pub fn active_page_state_key(&self) -> Option<PageStateKey> {
        self.active_page_id.map(|page_id| PageStateKey {
            history_state: self.history.current,
            page_id,
        })
    }

    pub fn saved_history_state(&self) -> HistoryStateId {
        self.history.saved
    }

    pub fn is_dirty(&self) -> bool {
        self.history.current != self.history.saved
    }

    pub fn mark_saved(&mut self) {
        self.history.saved = self.history.current;
    }

    /// Execute one persistent editor mutation.
    ///
    /// Pointer tools should normally keep drag/resize/rotate previews in transient
    /// view state and submit a single command at gesture commit.
    pub fn execute(&mut self, command: EditCommand) -> Result<bool, EditorError> {
        self.execute_transaction(EditTransaction::single(command))
    }

    /// Execute several semantic commands atomically as one undo/redo history step.
    pub fn execute_transaction(
        &mut self,
        transaction: EditTransaction,
    ) -> Result<bool, EditorError> {
        let active_before = self.active_state();
        let (forward, inverse) = apply_transaction(&mut self.document, transaction.commands())?;
        if forward.is_empty() {
            return Ok(false);
        }

        let topology_changed = forward.iter().any(command_changes_session_topology);
        self.repair_active_state();
        let active_after = self.active_state();
        let before = self.history.current;
        let after = self.history.allocate_state();
        self.history.undo.push(HistoryEntry {
            forward: EditTransaction { commands: forward },
            inverse,
            before,
            after,
            active_before,
            active_after,
            topology_changed,
        });
        self.history.redo.clear();
        self.history.current = after;
        self.prune_selection();
        Ok(true)
    }

    pub fn can_undo(&self) -> bool {
        !self.history.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.history.redo.is_empty()
    }

    pub fn undo(&mut self) -> Result<bool, EditorError> {
        let Some(entry) = self.history.undo.pop() else {
            return Ok(false);
        };
        if apply_undo_steps(&mut self.document, &entry.inverse).is_err() {
            self.history.undo.push(entry);
            return Err(EditorError::HistoryInvariantViolation);
        }
        let active_before = entry.active_before;
        let topology_changed = entry.topology_changed;
        self.history.current = entry.before;
        self.history.redo.push(entry);
        if topology_changed {
            self.restore_active_state(active_before);
        } else {
            self.repair_active_state();
        }
        self.prune_selection();
        Ok(true)
    }

    pub fn redo(&mut self) -> Result<bool, EditorError> {
        let Some(mut entry) = self.history.redo.pop() else {
            return Ok(false);
        };
        let applied = apply_transaction(&mut self.document, entry.forward.commands());
        let Ok((forward, inverse)) = applied else {
            self.history.redo.push(entry);
            return Err(EditorError::HistoryInvariantViolation);
        };
        if forward != entry.forward.commands {
            if apply_undo_steps(&mut self.document, &inverse).is_err() {
                return Err(EditorError::HistoryInvariantViolation);
            }
            self.history.redo.push(entry);
            return Err(EditorError::HistoryInvariantViolation);
        }
        entry.inverse = inverse;
        let active_after = entry.active_after;
        let topology_changed = entry.topology_changed;
        self.history.current = entry.after;
        self.history.undo.push(entry);
        if topology_changed {
            self.restore_active_state(active_after);
        } else {
            self.repair_active_state();
        }
        self.prune_selection();
        Ok(true)
    }

    fn active_state(&self) -> ActiveState {
        ActiveState {
            page_id: self.active_page_id,
            layer: self.active_layer,
        }
    }

    fn restore_active_state(&mut self, state: ActiveState) {
        self.active_page_id = state.page_id;
        self.active_layer = state.layer;
        self.repair_active_state();
    }

    fn repair_active_state(&mut self) {
        if self
            .active_page_id
            .map(|page_id| self.document.pages.iter().any(|page| page.id == page_id))
            != Some(true)
        {
            self.active_page_id = self.document.pages.first().map(|page| page.id);
        }

        let active_layer_valid = self.active_layer.is_some_and(|target| {
            find_layer(&self.document, target).is_some()
                && match target {
                    LayerTarget::Master { .. } => true,
                    LayerTarget::Page { page_id, .. } => self.active_page_id == Some(page_id),
                }
        });
        if active_layer_valid {
            return;
        }

        self.active_layer = self
            .active_page_id
            .and_then(|page_id| {
                self.document
                    .pages
                    .iter()
                    .find(|page| page.id == page_id)
                    .and_then(|page| {
                        page.layers.first().map(|layer| LayerTarget::Page {
                            page_id,
                            layer_id: layer.id,
                        })
                    })
            })
            .or_else(|| {
                self.document
                    .master_layers
                    .first()
                    .map(|layer| LayerTarget::Master { layer_id: layer.id })
            });
    }

    fn prune_selection(&mut self) {
        self.selection
            .retain(|element_id| find_element_layer(&self.document, *element_id).is_some());
    }
}

fn command_changes_session_topology(command: &EditCommand) -> bool {
    matches!(
        command,
        EditCommand::CreatePage { .. }
            | EditCommand::DeletePage { .. }
            | EditCommand::CreateLayer { .. }
            | EditCommand::DeleteLayer { .. }
    )
}

fn apply_transaction(
    document: &mut Document,
    commands: &[EditCommand],
) -> Result<(Vec<EditCommand>, Vec<UndoStep>), EditorError> {
    let mut forward = Vec::new();
    let mut inverse = Vec::new();
    let mut structural = false;

    for command in commands {
        match apply_command(document, command) {
            Ok(Some(applied)) => {
                forward.push(command.clone());
                structural |= applied.structural;
                inverse.push(applied.undo);
            }
            Ok(None) => {}
            Err(error) => {
                if apply_undo_steps(document, &inverse).is_err() {
                    return Err(EditorError::HistoryInvariantViolation);
                }
                return Err(error);
            }
        }
    }

    if structural {
        let validation = NextArtifact::document(document.clone()).validate();
        if !validation.is_valid() {
            if apply_undo_steps(document, &inverse).is_err() {
                return Err(EditorError::HistoryInvariantViolation);
            }
            return Err(EditorError::InvalidDocument(validation));
        }
    }

    Ok((forward, inverse))
}

fn apply_command(
    document: &mut Document,
    command: &EditCommand,
) -> Result<Option<AppliedCommand>, EditorError> {
    match command {
        EditCommand::CreatePage { page, index } => apply_create_page(document, page, *index),
        EditCommand::DeletePage { page_id } => apply_delete_page(document, *page_id),
        EditCommand::SetPageProperties {
            page_id,
            name,
            size_mm,
        } => apply_set_page_properties(document, *page_id, name, *size_mm),
        EditCommand::CreateLayer {
            scope,
            layer,
            index,
        } => apply_create_layer(document, *scope, layer, *index),
        EditCommand::DeleteLayer { target } => apply_delete_layer(document, *target),
        EditCommand::SetLayerProperties {
            target,
            name,
            visible,
            locked,
            draw_color,
        } => apply_set_layer_properties(document, *target, name, *visible, *locked, *draw_color),
        EditCommand::MoveElements {
            element_ids,
            delta_mm,
        } => apply_move(document, element_ids, *delta_mm),
        EditCommand::ArrangeElements {
            element_ids,
            operation,
        } => apply_arrange_elements(document, element_ids, *operation),
        EditCommand::ReorderElements {
            element_ids,
            operation,
        } => apply_reorder_elements(document, element_ids, *operation),
        EditCommand::SetBounds {
            element_id,
            bounds_mm,
        } => apply_set_bounds(document, *element_id, *bounds_mm),
        EditCommand::SetRotation {
            element_id,
            rotation_deg,
        } => apply_set_rotation(document, *element_id, *rotation_deg),
        EditCommand::SetConnectorEndpoint {
            element_id,
            side,
            position_mm,
            connection,
        } => apply_set_connector_endpoint(document, *element_id, *side, *position_mm, *connection),
        EditCommand::SetConnectorStyle {
            element_id,
            start_marker,
            end_marker,
            line_style,
            secondary_color,
        } => apply_set_connector_style(
            document,
            *element_id,
            *start_marker,
            *end_marker,
            *line_style,
            *secondary_color,
        ),
        EditCommand::SetElementStyle {
            element_ids,
            style_id,
        } => apply_set_element_style(document, element_ids, *style_id),
        EditCommand::SetElementAppearance {
            element_id,
            stroke,
            fill,
            text_color,
        } => apply_set_element_appearance(document, *element_id, stroke, fill, *text_color),
        EditCommand::SetText { element_id, text } => apply_set_text(document, *element_id, text),
        EditCommand::GroupElements {
            group_id,
            element_ids,
            name,
        } => apply_group_elements(document, *group_id, element_ids, name),
        EditCommand::CreateStructuralGroup {
            target,
            group,
            z_index,
        } => apply_create_structural_group(document, *target, group, *z_index),
        EditCommand::Ungroup { group_id } => apply_ungroup(document, *group_id),
        EditCommand::CreateElement {
            target,
            element,
            z_index,
        } => apply_create(document, *target, element, *z_index),
        EditCommand::DeleteElements { element_ids } => apply_delete(document, element_ids),
    }
}

fn apply_create_page(
    document: &mut Document,
    page: &Page,
    index: Option<usize>,
) -> Result<Option<AppliedCommand>, EditorError> {
    if document.pages.iter().any(|existing| existing.id == page.id) {
        return Err(EditorError::PageAlreadyExists(page.id));
    }
    if !size_is_valid(page.size_mm) {
        return Err(EditorError::InvalidPageSize);
    }

    let mut page_layer_ids = BTreeSet::new();
    for layer in &page.layers {
        if !page_layer_ids.insert(layer.id) || layer_id_exists(document, layer.id) {
            return Err(EditorError::LayerAlreadyExists(layer.id));
        }
    }

    let insertion = index.unwrap_or(document.pages.len());
    if insertion > document.pages.len() {
        return Err(EditorError::InvalidPageIndex {
            index: insertion,
            len: document.pages.len(),
        });
    }
    document.pages.insert(insertion, page.clone());
    Ok(Some(AppliedCommand {
        undo: UndoStep::RemoveCreatedPage { page_id: page.id },
        structural: true,
    }))
}

fn apply_delete_page(
    document: &mut Document,
    page_id: PageId,
) -> Result<Option<AppliedCommand>, EditorError> {
    let page_index = document
        .pages
        .iter()
        .position(|page| page.id == page_id)
        .ok_or(EditorError::PageNotFound(page_id))?;
    let page = document.pages[page_index].clone();
    if let Some(layer) = page.layers.iter().find(|layer| layer.locked) {
        return Err(EditorError::LayerLocked(layer.id));
    }

    let delete_ids = page_element_ids(&page);
    let mut detached = Vec::new();
    detach_document_connections(document, &delete_ids, &mut detached);
    document.pages.remove(page_index);

    Ok(Some(AppliedCommand {
        undo: UndoStep::RestoreDeletedPage {
            page,
            page_index,
            detached,
        },
        structural: true,
    }))
}

fn apply_set_page_properties(
    document: &mut Document,
    page_id: PageId,
    name: &str,
    size_mm: Size,
) -> Result<Option<AppliedCommand>, EditorError> {
    if !size_is_valid(size_mm) {
        return Err(EditorError::InvalidPageSize);
    }
    let page = document
        .pages
        .iter_mut()
        .find(|page| page.id == page_id)
        .ok_or(EditorError::PageNotFound(page_id))?;
    if page.name == name && page.size_mm == size_mm {
        return Ok(None);
    }
    let previous_name = page.name.clone();
    let previous_size = page.size_mm;
    page.name = name.to_owned();
    page.size_mm = size_mm;
    Ok(Some(AppliedCommand {
        undo: UndoStep::SetPageProperties {
            page_id,
            name: previous_name,
            size_mm: previous_size,
        },
        structural: false,
    }))
}

fn apply_create_layer(
    document: &mut Document,
    scope: LayerScope,
    layer: &Layer,
    index: Option<usize>,
) -> Result<Option<AppliedCommand>, EditorError> {
    if layer_id_exists(document, layer.id) {
        return Err(EditorError::LayerAlreadyExists(layer.id));
    }
    let len = layers(document, scope)?.len();
    let insertion = index.unwrap_or(len);
    if insertion > len {
        return Err(EditorError::InvalidLayerIndex {
            index: insertion,
            len,
        });
    }
    layers_mut(document, scope)?.insert(insertion, layer.clone());
    Ok(Some(AppliedCommand {
        undo: UndoStep::RemoveCreatedLayer {
            scope,
            layer_id: layer.id,
        },
        structural: true,
    }))
}

fn apply_delete_layer(
    document: &mut Document,
    target: LayerTarget,
) -> Result<Option<AppliedCommand>, EditorError> {
    let scope = layer_scope_of(target);
    let layer_id = layer_id_of(target);
    let (layer_index, layer) = {
        let collection = layers(document, scope)?;
        let layer_index = collection
            .iter()
            .position(|layer| layer.id == layer_id)
            .ok_or(EditorError::LayerNotFound(layer_id))?;
        (layer_index, collection[layer_index].clone())
    };
    if layer.locked {
        return Err(EditorError::LayerLocked(layer.id));
    }

    let delete_ids = layer_element_ids(&layer);
    let mut detached = Vec::new();
    detach_document_connections(document, &delete_ids, &mut detached);
    layers_mut(document, scope)?.remove(layer_index);

    Ok(Some(AppliedCommand {
        undo: UndoStep::RestoreDeletedLayer {
            scope,
            layer,
            layer_index,
            detached,
        },
        structural: true,
    }))
}

fn apply_set_layer_properties(
    document: &mut Document,
    target: LayerTarget,
    name: &str,
    visible: bool,
    locked: bool,
    draw_color: Option<Color>,
) -> Result<Option<AppliedCommand>, EditorError> {
    let layer =
        find_layer_mut(document, target).ok_or(EditorError::LayerNotFound(layer_id_of(target)))?;
    if layer.name == name
        && layer.visible == visible
        && layer.locked == locked
        && layer.draw_color == draw_color
    {
        return Ok(None);
    }
    let previous_name = layer.name.clone();
    let previous_visible = layer.visible;
    let previous_locked = layer.locked;
    let previous_draw_color = layer.draw_color;
    layer.name = name.to_owned();
    layer.visible = visible;
    layer.locked = locked;
    layer.draw_color = draw_color;
    Ok(Some(AppliedCommand {
        undo: UndoStep::SetLayerProperties {
            target,
            name: previous_name,
            visible: previous_visible,
            locked: previous_locked,
            draw_color: previous_draw_color,
        },
        structural: false,
    }))
}

fn apply_move(
    document: &mut Document,
    element_ids: &[ElementId],
    delta_mm: Point,
) -> Result<Option<AppliedCommand>, EditorError> {
    if !delta_mm.x.is_finite() || !delta_mm.y.is_finite() {
        return Err(EditorError::InvalidGeometry);
    }
    if element_ids.is_empty() || (delta_mm.x == 0.0 && delta_mm.y == 0.0) {
        return Ok(None);
    }

    preflight_elements(document, element_ids)?;
    let expanded_ids = expand_move_targets(document, element_ids)?;
    for element_id in &expanded_ids {
        let element = find_element_mut(document, *element_id)
            .ok_or(EditorError::ElementNotFound(*element_id))?;
        translate_element_geometry(element, delta_mm);
    }
    synchronize_connected_endpoints(document)?;

    Ok(Some(AppliedCommand {
        undo: UndoStep::MoveElements {
            element_ids: expanded_ids,
            delta_mm: Point {
                x: -delta_mm.x,
                y: -delta_mm.y,
            },
        },
        structural: false,
    }))
}

#[derive(Debug, Clone, Copy)]
struct ArrangeItem {
    element_id: ElementId,
    bounds: Rect,
}

fn apply_arrange_elements(
    document: &mut Document,
    element_ids: &[ElementId],
    operation: ArrangeOperation,
) -> Result<Option<AppliedCommand>, EditorError> {
    let required = operation.minimum_selection();
    if element_ids.len() < required {
        return Err(EditorError::ArrangeRequiresAtLeast {
            required,
            actual: element_ids.len(),
        });
    }

    let mut selected = BTreeSet::new();
    let mut target = None;
    for element_id in element_ids {
        if !selected.insert(*element_id) {
            return Err(EditorError::DuplicateCommandElement(*element_id));
        }
        ensure_element_editable(document, *element_id)?;
        let element_target = layer_target_for_element(document, *element_id)
            .ok_or(EditorError::ElementNotFound(*element_id))?;
        if target.is_some_and(|existing| existing != element_target) {
            return Err(EditorError::ArrangeDifferentLayers);
        }
        target = Some(element_target);
    }

    let target = target.expect("minimum arrange selection has a target layer");
    let layer =
        find_layer(document, target).ok_or(EditorError::LayerNotFound(layer_id_of(target)))?;
    for element_id in &selected {
        if layer
            .scene
            .roots
            .iter()
            .filter(|root| **root == *element_id)
            .count()
            != 1
        {
            return Err(EditorError::ArrangeRequiresTopLevelElement(*element_id));
        }
    }

    let mut items = Vec::with_capacity(selected.len());
    for element_id in selected {
        let mut recursion_stack = BTreeSet::new();
        let bounds = subtree_visual_bounds(&layer.scene, element_id, &mut recursion_stack)?;
        items.push(ArrangeItem { element_id, bounds });
    }

    let left = items
        .iter()
        .map(|item| item.bounds.x)
        .fold(f64::INFINITY, f64::min);
    let top = items
        .iter()
        .map(|item| item.bounds.y)
        .fold(f64::INFINITY, f64::min);
    let right = items
        .iter()
        .map(|item| item.bounds.x + item.bounds.width)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = items
        .iter()
        .map(|item| item.bounds.y + item.bounds.height)
        .fold(f64::NEG_INFINITY, f64::max);
    let horizontal_center = (left + right) / 2.0;
    let vertical_center = (top + bottom) / 2.0;

    let mut logical_movements: Vec<(ElementId, Point)> = Vec::new();
    match operation {
        ArrangeOperation::AlignLeft => {
            logical_movements.extend(items.iter().map(|item| {
                (
                    item.element_id,
                    Point {
                        x: left - item.bounds.x,
                        y: 0.0,
                    },
                )
            }));
        }
        ArrangeOperation::AlignHorizontalCenter => {
            logical_movements.extend(items.iter().map(|item| {
                (
                    item.element_id,
                    Point {
                        x: horizontal_center - (item.bounds.x + item.bounds.width / 2.0),
                        y: 0.0,
                    },
                )
            }));
        }
        ArrangeOperation::AlignRight => {
            logical_movements.extend(items.iter().map(|item| {
                (
                    item.element_id,
                    Point {
                        x: right - (item.bounds.x + item.bounds.width),
                        y: 0.0,
                    },
                )
            }));
        }
        ArrangeOperation::AlignTop => {
            logical_movements.extend(items.iter().map(|item| {
                (
                    item.element_id,
                    Point {
                        x: 0.0,
                        y: top - item.bounds.y,
                    },
                )
            }));
        }
        ArrangeOperation::AlignVerticalCenter => {
            logical_movements.extend(items.iter().map(|item| {
                (
                    item.element_id,
                    Point {
                        x: 0.0,
                        y: vertical_center - (item.bounds.y + item.bounds.height / 2.0),
                    },
                )
            }));
        }
        ArrangeOperation::AlignBottom => {
            logical_movements.extend(items.iter().map(|item| {
                (
                    item.element_id,
                    Point {
                        x: 0.0,
                        y: bottom - (item.bounds.y + item.bounds.height),
                    },
                )
            }));
        }
        ArrangeOperation::DistributeHorizontal => {
            items.sort_by(|a, b| {
                a.bounds
                    .x
                    .total_cmp(&b.bounds.x)
                    .then_with(|| a.element_id.cmp(&b.element_id))
            });
            let first_left = items[0].bounds.x;
            let last_right = items.last().expect("distribution has items").bounds.x
                + items.last().expect("distribution has items").bounds.width;
            let total_width: f64 = items.iter().map(|item| item.bounds.width).sum();
            let gap = (last_right - first_left - total_width) / (items.len() - 1) as f64;
            let last_index = items.len() - 1;
            let mut cursor = first_left;
            for (index, item) in items.iter().enumerate() {
                let delta_x = if index == 0 || index == last_index {
                    0.0
                } else {
                    cursor - item.bounds.x
                };
                logical_movements.push((item.element_id, Point { x: delta_x, y: 0.0 }));
                cursor += item.bounds.width + gap;
            }
        }
        ArrangeOperation::DistributeVertical => {
            items.sort_by(|a, b| {
                a.bounds
                    .y
                    .total_cmp(&b.bounds.y)
                    .then_with(|| a.element_id.cmp(&b.element_id))
            });
            let first_top = items[0].bounds.y;
            let last_bottom = items.last().expect("distribution has items").bounds.y
                + items.last().expect("distribution has items").bounds.height;
            let total_height: f64 = items.iter().map(|item| item.bounds.height).sum();
            let gap = (last_bottom - first_top - total_height) / (items.len() - 1) as f64;
            let last_index = items.len() - 1;
            let mut cursor = first_top;
            for (index, item) in items.iter().enumerate() {
                let delta_y = if index == 0 || index == last_index {
                    0.0
                } else {
                    cursor - item.bounds.y
                };
                logical_movements.push((item.element_id, Point { x: 0.0, y: delta_y }));
                cursor += item.bounds.height + gap;
            }
        }
    }

    let mut undo_movements = Vec::new();
    let mut expanded_seen = BTreeSet::new();
    for (element_id, delta_mm) in logical_movements {
        if delta_mm.x == 0.0 && delta_mm.y == 0.0 {
            continue;
        }
        let expanded_ids = expand_move_targets(document, &[element_id])?;
        for expanded_id in &expanded_ids {
            if !expanded_seen.insert(*expanded_id) {
                return Err(EditorError::HistoryInvariantViolation);
            }
        }
        for expanded_id in &expanded_ids {
            let element = find_element_mut(document, *expanded_id)
                .ok_or(EditorError::HistoryInvariantViolation)?;
            translate_element_geometry(element, delta_mm);
        }
        undo_movements.push((
            expanded_ids,
            Point {
                x: -delta_mm.x,
                y: -delta_mm.y,
            },
        ));
    }

    if undo_movements.is_empty() {
        return Ok(None);
    }
    synchronize_connected_endpoints(document)?;
    Ok(Some(AppliedCommand {
        undo: UndoStep::ArrangeElements {
            movements: undo_movements,
        },
        structural: false,
    }))
}

fn apply_reorder_elements(
    document: &mut Document,
    element_ids: &[ElementId],
    operation: ZOrderOperation,
) -> Result<Option<AppliedCommand>, EditorError> {
    if element_ids.is_empty() {
        return Ok(None);
    }

    let mut selected = BTreeSet::new();
    let mut target = None;
    for element_id in element_ids {
        if !selected.insert(*element_id) {
            return Err(EditorError::DuplicateCommandElement(*element_id));
        }
        ensure_element_editable(document, *element_id)?;
        let element_target = layer_target_for_element(document, *element_id)
            .ok_or(EditorError::ElementNotFound(*element_id))?;
        if target.is_some_and(|existing| existing != element_target) {
            return Err(EditorError::ZOrderDifferentLayers);
        }
        target = Some(element_target);
    }

    let target = target.expect("non-empty z-order selection has a target layer");
    let layer =
        find_layer(document, target).ok_or(EditorError::LayerNotFound(layer_id_of(target)))?;
    for element_id in &selected {
        if layer
            .scene
            .roots
            .iter()
            .filter(|root| **root == *element_id)
            .count()
            != 1
        {
            return Err(EditorError::ZOrderRequiresTopLevelElement(*element_id));
        }
    }

    let previous = layer.scene.roots.clone();
    let mut reordered = previous.clone();
    match operation {
        ZOrderOperation::BringToFront => {
            reordered.retain(|element_id| !selected.contains(element_id));
            reordered.extend(
                previous
                    .iter()
                    .copied()
                    .filter(|element_id| selected.contains(element_id)),
            );
        }
        ZOrderOperation::SendToBack => {
            reordered = previous
                .iter()
                .copied()
                .filter(|element_id| selected.contains(element_id))
                .chain(
                    previous
                        .iter()
                        .copied()
                        .filter(|element_id| !selected.contains(element_id)),
                )
                .collect();
        }
        ZOrderOperation::BringForward => {
            for index in (0..reordered.len().saturating_sub(1)).rev() {
                if selected.contains(&reordered[index]) && !selected.contains(&reordered[index + 1])
                {
                    reordered.swap(index, index + 1);
                }
            }
        }
        ZOrderOperation::SendBackward => {
            for index in 1..reordered.len() {
                if selected.contains(&reordered[index]) && !selected.contains(&reordered[index - 1])
                {
                    reordered.swap(index, index - 1);
                }
            }
        }
    }

    if reordered == previous {
        return Ok(None);
    }

    let layer = find_layer_mut(document, target).ok_or(EditorError::HistoryInvariantViolation)?;
    layer.scene.roots = reordered;
    Ok(Some(AppliedCommand {
        undo: UndoStep::RestoreZOrder {
            target,
            roots: previous,
        },
        structural: false,
    }))
}

fn apply_set_bounds(
    document: &mut Document,
    element_id: ElementId,
    bounds_mm: Rect,
) -> Result<Option<AppliedCommand>, EditorError> {
    if !rect_is_valid(bounds_mm) {
        return Err(EditorError::InvalidGeometry);
    }
    ensure_element_editable(document, element_id)?;
    if matches!(
        &find_element(document, element_id)
            .ok_or(EditorError::ElementNotFound(element_id))?
            .kind,
        ElementKind::Group { .. }
    ) {
        return Err(EditorError::GroupTransformRequiresDedicatedCommand(
            element_id,
        ));
    }
    let element =
        find_element_mut(document, element_id).ok_or(EditorError::ElementNotFound(element_id))?;
    if element.bounds_mm == bounds_mm {
        return Ok(None);
    }
    let previous = element.bounds_mm;
    element.bounds_mm = bounds_mm;
    synchronize_connected_endpoints(document)?;
    Ok(Some(AppliedCommand {
        undo: UndoStep::SetBounds {
            element_id,
            bounds_mm: previous,
        },
        structural: false,
    }))
}

fn apply_set_rotation(
    document: &mut Document,
    element_id: ElementId,
    rotation_deg: f64,
) -> Result<Option<AppliedCommand>, EditorError> {
    if !rotation_deg.is_finite() {
        return Err(EditorError::InvalidGeometry);
    }
    ensure_element_editable(document, element_id)?;
    if matches!(
        &find_element(document, element_id)
            .ok_or(EditorError::ElementNotFound(element_id))?
            .kind,
        ElementKind::Group { .. }
    ) {
        return Err(EditorError::GroupTransformRequiresDedicatedCommand(
            element_id,
        ));
    }
    let element =
        find_element_mut(document, element_id).ok_or(EditorError::ElementNotFound(element_id))?;
    if element.rotation_deg == rotation_deg {
        return Ok(None);
    }
    let previous = element.rotation_deg;
    element.rotation_deg = rotation_deg;
    synchronize_connected_endpoints(document)?;
    Ok(Some(AppliedCommand {
        undo: UndoStep::SetRotation {
            element_id,
            rotation_deg: previous,
        },
        structural: false,
    }))
}

fn apply_set_connector_endpoint(
    document: &mut Document,
    element_id: ElementId,
    side: ConnectorEndpointSide,
    position_mm: Point,
    connection: Option<Connection>,
) -> Result<Option<AppliedCommand>, EditorError> {
    if !point_is_finite(position_mm) {
        return Err(EditorError::InvalidGeometry);
    }
    ensure_element_editable(document, element_id)?;
    if connector(
        find_element(document, element_id).ok_or(EditorError::ElementNotFound(element_id))?,
    )
    .is_none()
    {
        return Err(EditorError::ElementIsNotConnector(element_id));
    }

    let resolved_position = match connection {
        Some(connection) => resolve_connection_position(document, element_id, connection)?,
        None => position_mm,
    };
    let next = Endpoint {
        position_mm: resolved_position,
        connection,
    };
    let previous = connector_endpoint(
        connector(
            find_element(document, element_id).ok_or(EditorError::ElementNotFound(element_id))?,
        )
        .ok_or(EditorError::ElementIsNotConnector(element_id))?,
        side,
    )
    .clone();
    if previous == next {
        return Ok(None);
    }

    let element =
        find_element_mut(document, element_id).ok_or(EditorError::HistoryInvariantViolation)?;
    let endpoint = connector_endpoint_mut(
        connector_mut(element).ok_or(EditorError::HistoryInvariantViolation)?,
        side,
    );
    *endpoint = next;
    refresh_connector_bounds(document, element_id)?;

    Ok(Some(AppliedCommand {
        undo: UndoStep::SetConnectorEndpoint {
            element_id,
            side,
            endpoint: previous,
        },
        // Connection references participate in Next-domain structural validation.
        structural: true,
    }))
}

fn apply_set_connector_style(
    document: &mut Document,
    element_id: ElementId,
    start_marker: MarkerStyle,
    end_marker: MarkerStyle,
    line_style: LineStyle,
    secondary_color: Option<Color>,
) -> Result<Option<AppliedCommand>, EditorError> {
    ensure_element_editable(document, element_id)?;
    let (previous_start_marker, previous_end_marker, previous_line_style, previous_secondary_color) = {
        let element =
            find_element(document, element_id).ok_or(EditorError::ElementNotFound(element_id))?;
        let connector = connector(element).ok_or(EditorError::ElementIsNotConnector(element_id))?;
        (
            connector.start_marker,
            connector.end_marker,
            connector.line_style,
            connector.secondary_color,
        )
    };

    if previous_start_marker == start_marker
        && previous_end_marker == end_marker
        && previous_line_style == line_style
        && previous_secondary_color == secondary_color
    {
        return Ok(None);
    }

    let element =
        find_element_mut(document, element_id).ok_or(EditorError::HistoryInvariantViolation)?;
    let connector = connector_mut(element).ok_or(EditorError::HistoryInvariantViolation)?;
    connector.start_marker = start_marker;
    connector.end_marker = end_marker;
    connector.line_style = line_style;
    connector.secondary_color = secondary_color;

    Ok(Some(AppliedCommand {
        undo: UndoStep::SetConnectorStyle {
            element_id,
            start_marker: previous_start_marker,
            end_marker: previous_end_marker,
            line_style: previous_line_style,
            secondary_color: previous_secondary_color,
        },
        structural: false,
    }))
}

fn apply_set_element_style(
    document: &mut Document,
    element_ids: &[ElementId],
    style_id: Option<StyleId>,
) -> Result<Option<AppliedCommand>, EditorError> {
    if element_ids.is_empty() {
        return Ok(None);
    }
    if let Some(style_id) = style_id {
        if !document.styles.iter().any(|style| style.id == style_id) {
            return Err(EditorError::StyleNotFound(style_id));
        }
    }

    preflight_elements(document, element_ids)?;
    let previous: Vec<_> = element_ids
        .iter()
        .map(|element_id| {
            let style_id = find_element(document, *element_id)
                .ok_or(EditorError::ElementNotFound(*element_id))?
                .style_id;
            Ok((*element_id, style_id))
        })
        .collect::<Result<_, EditorError>>()?;

    if previous
        .iter()
        .all(|(_, previous_style_id)| *previous_style_id == style_id)
    {
        return Ok(None);
    }

    for element_id in element_ids {
        let element = find_element_mut(document, *element_id)
            .ok_or(EditorError::HistoryInvariantViolation)?;
        element.style_id = style_id;
    }

    Ok(Some(AppliedCommand {
        undo: UndoStep::SetElementStyles { previous },
        structural: false,
    }))
}

fn apply_set_element_appearance(
    document: &mut Document,
    element_id: ElementId,
    stroke: &Option<StrokeStyle>,
    fill: &Option<FillStyle>,
    text_color: Option<Color>,
) -> Result<Option<AppliedCommand>, EditorError> {
    if stroke
        .as_ref()
        .is_some_and(|stroke| !stroke.width_mm.is_finite() || stroke.width_mm <= 0.0)
    {
        return Err(EditorError::InvalidAppearance);
    }
    ensure_element_editable(document, element_id)?;

    let dedicated_style_id = StyleId::v5(element_id.0, "diagramdesigner-next:element-appearance");
    let previous_style_id = find_element(document, element_id)
        .ok_or(EditorError::ElementNotFound(element_id))?
        .style_id;
    let previous_dedicated_style = document
        .styles
        .iter()
        .find(|style| style.id == dedicated_style_id)
        .cloned();

    let referenced_by_other =
        all_layers(document).any(|layer| {
            layer.scene.elements.iter().any(|element| {
                element.id != element_id && element.style_id == Some(dedicated_style_id)
            })
        });
    if referenced_by_other
        || (previous_dedicated_style.is_some() && previous_style_id != Some(dedicated_style_id))
    {
        return Err(EditorError::AppearanceStyleCollision(dedicated_style_id));
    }

    let next_style = ElementStyle {
        id: dedicated_style_id,
        stroke: stroke.clone(),
        fill: fill.clone(),
        text_color,
    };
    if previous_style_id == Some(dedicated_style_id)
        && previous_dedicated_style.as_ref() == Some(&next_style)
    {
        return Ok(None);
    }

    if let Some(existing) = document
        .styles
        .iter_mut()
        .find(|style| style.id == dedicated_style_id)
    {
        *existing = next_style;
    } else {
        document.styles.push(next_style);
    }
    find_element_mut(document, element_id)
        .ok_or(EditorError::HistoryInvariantViolation)?
        .style_id = Some(dedicated_style_id);

    Ok(Some(AppliedCommand {
        undo: UndoStep::RestoreElementAppearance {
            element_id,
            previous_style_id,
            dedicated_style_id,
            previous_dedicated_style,
        },
        // The command creates a style reference and therefore participates in domain validation.
        structural: true,
    }))
}

fn apply_set_text(
    document: &mut Document,
    element_id: ElementId,
    text: &Option<TextBlock>,
) -> Result<Option<AppliedCommand>, EditorError> {
    if let Some(text) = text {
        if !text_block_is_valid(text) {
            return Err(EditorError::InvalidTextLayout);
        }
    }

    ensure_element_editable(document, element_id)?;
    let element =
        find_element_mut(document, element_id).ok_or(EditorError::ElementNotFound(element_id))?;
    if element.text.as_ref() == text.as_ref() {
        return Ok(None);
    }

    let previous = element.text.clone();
    element.text = text.clone();
    Ok(Some(AppliedCommand {
        undo: UndoStep::SetText {
            element_id,
            text: previous,
        },
        structural: false,
    }))
}

fn apply_group_elements(
    document: &mut Document,
    group_id: ElementId,
    element_ids: &[ElementId],
    name: &str,
) -> Result<Option<AppliedCommand>, EditorError> {
    if element_ids.len() < 2 {
        return Err(EditorError::GroupRequiresAtLeastTwoElements);
    }
    if find_element_layer(document, group_id).is_some() {
        return Err(EditorError::ElementAlreadyExists(group_id));
    }
    preflight_elements(document, element_ids)?;

    let target = layer_target_for_element(document, element_ids[0])
        .ok_or(EditorError::ElementNotFound(element_ids[0]))?;
    if element_ids
        .iter()
        .copied()
        .any(|element_id| layer_target_for_element(document, element_id) != Some(target))
    {
        return Err(EditorError::GroupMembersHaveDifferentOwners);
    }

    let layer = find_layer(document, target)
        .ok_or_else(|| EditorError::LayerNotFound(layer_id_of(target)))?;
    let scene = &layer.scene;
    let owner = direct_sibling_owner(scene, element_ids[0])?;
    for element_id in element_ids.iter().copied().skip(1) {
        if direct_sibling_owner(scene, element_id)? != owner {
            return Err(EditorError::GroupMembersHaveDifferentOwners);
        }
    }

    let siblings = owner_siblings(scene, owner)
        .ok_or(EditorError::AmbiguousElementOwnership(element_ids[0]))?;
    let selected: BTreeSet<_> = element_ids.iter().copied().collect();
    if selected.len() != element_ids.len() {
        let duplicate = element_ids
            .iter()
            .copied()
            .find(|id| {
                element_ids
                    .iter()
                    .filter(|candidate| **candidate == *id)
                    .count()
                    > 1
            })
            .expect("duplicate set cardinality implies a duplicate element");
        return Err(EditorError::DuplicateCommandElement(duplicate));
    }

    let positions: Vec<_> = siblings
        .iter()
        .enumerate()
        .filter_map(|(index, id)| selected.contains(id).then_some(index))
        .collect();
    if positions.len() != selected.len() {
        return Err(EditorError::GroupMembersHaveDifferentOwners);
    }
    let first = positions[0];
    let last = *positions
        .last()
        .expect("grouping requires at least two members");
    if last - first + 1 != positions.len() {
        return Err(EditorError::NonContiguousGroupSelection);
    }

    let children = siblings[first..=last].to_vec();
    let bounds_mm = subtree_union_bounds(scene, &children)?;
    let group = Element {
        id: group_id,
        name: if name.is_empty() {
            "Group".to_owned()
        } else {
            name.to_owned()
        },
        bounds_mm,
        rotation_deg: 0.0,
        anchors: Default::default(),
        ports: Vec::new(),
        style_id: None,
        text: None,
        kind: ElementKind::Group { children },
        import: None,
    };

    let previous_siblings = siblings.clone();
    let mut replacement = previous_siblings.clone();
    replacement.splice(first..=last, [group_id]);

    let layer = find_layer_mut(document, target).ok_or(EditorError::HistoryInvariantViolation)?;
    *owner_siblings_mut(&mut layer.scene, owner).ok_or(EditorError::HistoryInvariantViolation)? =
        replacement;
    layer.scene.elements.push(group);

    Ok(Some(AppliedCommand {
        undo: UndoStep::RemoveCreatedGroup {
            target,
            owner,
            previous_siblings,
            group_id,
        },
        structural: true,
    }))
}

fn apply_create_structural_group(
    document: &mut Document,
    target: LayerTarget,
    group: &Element,
    z_index: Option<usize>,
) -> Result<Option<AppliedCommand>, EditorError> {
    if find_element_layer(document, group.id).is_some() {
        return Err(EditorError::ElementAlreadyExists(group.id));
    }
    if !element_geometry_is_valid(group) {
        return Err(EditorError::InvalidGeometry);
    }
    let ElementKind::Group { children } = &group.kind else {
        return Err(EditorError::ElementIsNotGroup(group.id));
    };
    let mut command_ports = BTreeSet::new();
    for port in &group.ports {
        if !command_ports.insert(port.id) || port_exists(document, port.id) {
            return Err(EditorError::PortAlreadyExists(port.id));
        }
    }
    let layer = find_layer(document, target)
        .ok_or_else(|| EditorError::LayerNotFound(layer_id_of(target)))?;
    if layer.locked {
        return Err(EditorError::LayerLocked(layer.id));
    }

    if children.is_empty() {
        let insertion = z_index.unwrap_or(layer.scene.roots.len());
        if insertion > layer.scene.roots.len() {
            return Err(EditorError::InvalidZOrderIndex {
                index: insertion,
                len: layer.scene.roots.len(),
            });
        }
        let previous_siblings = layer.scene.roots.clone();
        let mut replacement = previous_siblings.clone();
        replacement.insert(insertion, group.id);
        let layer =
            find_layer_mut(document, target).ok_or(EditorError::HistoryInvariantViolation)?;
        layer.scene.roots = replacement;
        layer.scene.elements.push(group.clone());
        return Ok(Some(AppliedCommand {
            undo: UndoStep::RemoveCreatedGroup {
                target,
                owner: SiblingOwner::Roots,
                previous_siblings,
                group_id: group.id,
            },
            structural: true,
        }));
    }

    preflight_elements(document, children)?;
    if children
        .iter()
        .copied()
        .any(|child_id| layer_target_for_element(document, child_id) != Some(target))
    {
        return Err(EditorError::GroupMembersHaveDifferentOwners);
    }
    let layer = find_layer(document, target).ok_or(EditorError::HistoryInvariantViolation)?;
    let scene = &layer.scene;
    let owner = direct_sibling_owner(scene, children[0])?;
    for child_id in children.iter().copied().skip(1) {
        if direct_sibling_owner(scene, child_id)? != owner {
            return Err(EditorError::GroupMembersHaveDifferentOwners);
        }
    }
    let siblings =
        owner_siblings(scene, owner).ok_or(EditorError::AmbiguousElementOwnership(children[0]))?;
    let selected: BTreeSet<_> = children.iter().copied().collect();
    if selected.len() != children.len() {
        let duplicate = children
            .iter()
            .copied()
            .find(|id| {
                children
                    .iter()
                    .filter(|candidate| **candidate == *id)
                    .count()
                    > 1
            })
            .expect("duplicate group child must exist");
        return Err(EditorError::DuplicateCommandElement(duplicate));
    }
    let positions: Vec<_> = siblings
        .iter()
        .enumerate()
        .filter_map(|(index, id)| selected.contains(id).then_some(index))
        .collect();
    if positions.len() != children.len() {
        return Err(EditorError::GroupMembersHaveDifferentOwners);
    }
    let first = positions[0];
    let last = *positions.last().expect("non-empty group has last child");
    if last - first + 1 != positions.len() || siblings[first..=last] != children[..] {
        return Err(EditorError::NonContiguousGroupSelection);
    }
    let previous_siblings = siblings.clone();
    let mut replacement = previous_siblings.clone();
    replacement.splice(first..=last, [group.id]);
    let layer = find_layer_mut(document, target).ok_or(EditorError::HistoryInvariantViolation)?;
    *owner_siblings_mut(&mut layer.scene, owner).ok_or(EditorError::HistoryInvariantViolation)? =
        replacement;
    layer.scene.elements.push(group.clone());
    Ok(Some(AppliedCommand {
        undo: UndoStep::RemoveCreatedGroup {
            target,
            owner,
            previous_siblings,
            group_id: group.id,
        },
        structural: true,
    }))
}

fn apply_ungroup(
    document: &mut Document,
    group_id: ElementId,
) -> Result<Option<AppliedCommand>, EditorError> {
    ensure_element_editable(document, group_id)?;
    let target = layer_target_for_element(document, group_id)
        .ok_or(EditorError::ElementNotFound(group_id))?;
    let layer = find_layer(document, target).ok_or(EditorError::HistoryInvariantViolation)?;
    let scene = &layer.scene;
    let group = scene_element(scene, group_id)
        .ok_or(EditorError::ElementNotFound(group_id))?
        .clone();
    let ElementKind::Group { children } = &group.kind else {
        return Err(EditorError::ElementIsNotGroup(group_id));
    };
    let children = children.clone();
    let owner = direct_sibling_owner(scene, group_id)?;
    let previous_siblings = owner_siblings(scene, owner)
        .ok_or(EditorError::AmbiguousElementOwnership(group_id))?
        .clone();
    let sibling_index = previous_siblings
        .iter()
        .position(|id| *id == group_id)
        .ok_or(EditorError::AmbiguousElementOwnership(group_id))?;
    if previous_siblings
        .iter()
        .filter(|id| **id == group_id)
        .count()
        != 1
    {
        return Err(EditorError::AmbiguousElementOwnership(group_id));
    }
    let element_index = scene
        .elements
        .iter()
        .position(|element| element.id == group_id)
        .ok_or(EditorError::HistoryInvariantViolation)?;

    let mut replacement = previous_siblings.clone();
    replacement.splice(sibling_index..=sibling_index, children);

    let mut detached = Vec::new();
    let delete_ids = BTreeSet::from([group_id]);
    for layer in &mut document.master_layers {
        detach_deleted_connections(&mut layer.scene, &delete_ids, &mut detached);
    }
    for page in &mut document.pages {
        for layer in &mut page.layers {
            detach_deleted_connections(&mut layer.scene, &delete_ids, &mut detached);
        }
    }

    let layer = find_layer_mut(document, target).ok_or(EditorError::HistoryInvariantViolation)?;
    *owner_siblings_mut(&mut layer.scene, owner).ok_or(EditorError::HistoryInvariantViolation)? =
        replacement;
    let position = layer
        .scene
        .elements
        .iter()
        .position(|element| element.id == group_id)
        .ok_or(EditorError::HistoryInvariantViolation)?;
    layer.scene.elements.remove(position);

    Ok(Some(AppliedCommand {
        undo: UndoStep::RestoreUngrouped {
            target,
            owner,
            previous_siblings,
            group,
            element_index,
            detached,
        },
        structural: true,
    }))
}

fn apply_create(
    document: &mut Document,
    target: LayerTarget,
    element: &Element,
    z_index: Option<usize>,
) -> Result<Option<AppliedCommand>, EditorError> {
    if find_element_layer(document, element.id).is_some() {
        return Err(EditorError::ElementAlreadyExists(element.id));
    }
    if !element_geometry_is_valid(element) {
        return Err(EditorError::InvalidGeometry);
    }
    if matches!(
        &element.kind,
        ElementKind::Group { children } if !children.is_empty()
    ) {
        return Err(EditorError::GroupCreationRequiresDedicatedCommand);
    }

    let mut command_ports = BTreeSet::new();
    for port in &element.ports {
        if !command_ports.insert(port.id) || port_exists(document, port.id) {
            return Err(EditorError::PortAlreadyExists(port.id));
        }
    }

    let layer = find_layer(document, target)
        .ok_or_else(|| EditorError::LayerNotFound(layer_id_of(target)))?;
    if layer.locked {
        return Err(EditorError::LayerLocked(layer.id));
    }
    let insertion = z_index.unwrap_or(layer.scene.roots.len());
    if insertion > layer.scene.roots.len() {
        return Err(EditorError::InvalidZOrderIndex {
            index: insertion,
            len: layer.scene.roots.len(),
        });
    }

    let layer = find_layer_mut(document, target).ok_or(EditorError::HistoryInvariantViolation)?;
    layer.scene.elements.push(element.clone());
    layer.scene.roots.insert(insertion, element.id);

    Ok(Some(AppliedCommand {
        undo: UndoStep::RemoveCreated {
            element_id: element.id,
        },
        structural: true,
    }))
}

fn apply_delete(
    document: &mut Document,
    element_ids: &[ElementId],
) -> Result<Option<AppliedCommand>, EditorError> {
    if element_ids.is_empty() {
        return Ok(None);
    }

    let mut delete_ids = BTreeSet::new();
    for element_id in element_ids {
        if !delete_ids.insert(*element_id) {
            return Err(EditorError::DuplicateCommandElement(*element_id));
        }
        ensure_element_editable(document, *element_id)?;
    }

    // Deleting a group means deleting its complete owned subtree. This mirrors the
    // visible editor object and prevents hidden orphan children.
    let mut pending: Vec<_> = delete_ids.iter().copied().collect();
    while let Some(element_id) = pending.pop() {
        let element =
            find_element(document, element_id).ok_or(EditorError::ElementNotFound(element_id))?;
        if let ElementKind::Group { children } = &element.kind {
            for child in children {
                if delete_ids.insert(*child) {
                    ensure_element_editable(document, *child)?;
                    pending.push(*child);
                }
            }
        }
    }

    // A child cannot be removed while a surviving group still owns it. Ungrouping
    // is a separate semantic operation and must not happen implicitly during delete.
    for layer in all_layers(document) {
        for element in &layer.scene.elements {
            if delete_ids.contains(&element.id) {
                continue;
            }
            if let ElementKind::Group { children } = &element.kind {
                if let Some(child) = children.iter().find(|child| delete_ids.contains(child)) {
                    return Err(EditorError::ElementReferencedByGroup {
                        element_id: *child,
                        group_id: element.id,
                    });
                }
            }
        }
    }

    let mut removed = Vec::with_capacity(delete_ids.len());
    for element_id in &delete_ids {
        let target = layer_target_for_element(document, *element_id)
            .ok_or(EditorError::ElementNotFound(*element_id))?;
        let layer = find_layer(document, target).ok_or(EditorError::HistoryInvariantViolation)?;
        if layer.locked {
            return Err(EditorError::LayerLocked(layer.id));
        }
        let element_index = layer
            .scene
            .elements
            .iter()
            .position(|element| element.id == *element_id)
            .ok_or(EditorError::HistoryInvariantViolation)?;
        let root_index = layer.scene.roots.iter().position(|root| root == element_id);
        removed.push(RemovedElement {
            target,
            element: layer.scene.elements[element_index].clone(),
            element_index,
            root_index,
        });
    }

    let mut detached = Vec::new();
    for layer in &mut document.master_layers {
        detach_deleted_connections(&mut layer.scene, &delete_ids, &mut detached);
    }
    for page in &mut document.pages {
        for layer in &mut page.layers {
            detach_deleted_connections(&mut layer.scene, &delete_ids, &mut detached);
        }
    }

    for layer in &mut document.master_layers {
        remove_deleted_from_scene(&mut layer.scene, &delete_ids);
    }
    for page in &mut document.pages {
        for layer in &mut page.layers {
            remove_deleted_from_scene(&mut layer.scene, &delete_ids);
        }
    }

    Ok(Some(AppliedCommand {
        undo: UndoStep::RestoreDeleted { removed, detached },
        structural: true,
    }))
}

fn apply_undo_steps(document: &mut Document, inverse: &[UndoStep]) -> Result<(), EditorError> {
    for step in inverse.iter().rev() {
        apply_undo_step(document, step)?;
    }
    Ok(())
}

fn apply_undo_step(document: &mut Document, step: &UndoStep) -> Result<(), EditorError> {
    match step {
        UndoStep::RemoveCreatedPage { page_id } => {
            let index = document
                .pages
                .iter()
                .position(|page| page.id == *page_id)
                .ok_or(EditorError::HistoryInvariantViolation)?;
            document.pages.remove(index);
        }
        UndoStep::RestoreDeletedPage {
            page,
            page_index,
            detached,
        } => {
            if *page_index > document.pages.len() {
                return Err(EditorError::HistoryInvariantViolation);
            }
            document.pages.insert(*page_index, page.clone());
            restore_detached_connections(document, detached)?;
        }
        UndoStep::SetPageProperties {
            page_id,
            name,
            size_mm,
        } => {
            let page = document
                .pages
                .iter_mut()
                .find(|page| page.id == *page_id)
                .ok_or(EditorError::HistoryInvariantViolation)?;
            page.name = name.clone();
            page.size_mm = *size_mm;
        }
        UndoStep::RemoveCreatedLayer { scope, layer_id } => {
            let collection = layers_mut(document, *scope)?;
            let index = collection
                .iter()
                .position(|layer| layer.id == *layer_id)
                .ok_or(EditorError::HistoryInvariantViolation)?;
            collection.remove(index);
        }
        UndoStep::RestoreDeletedLayer {
            scope,
            layer,
            layer_index,
            detached,
        } => {
            let collection = layers_mut(document, *scope)?;
            if *layer_index > collection.len() {
                return Err(EditorError::HistoryInvariantViolation);
            }
            collection.insert(*layer_index, layer.clone());
            restore_detached_connections(document, detached)?;
        }
        UndoStep::SetLayerProperties {
            target,
            name,
            visible,
            locked,
            draw_color,
        } => {
            let layer =
                find_layer_mut(document, *target).ok_or(EditorError::HistoryInvariantViolation)?;
            layer.name = name.clone();
            layer.visible = *visible;
            layer.locked = *locked;
            layer.draw_color = *draw_color;
        }
        UndoStep::MoveElements {
            element_ids,
            delta_mm,
        } => {
            for element_id in element_ids {
                let element = find_element_mut(document, *element_id)
                    .ok_or(EditorError::HistoryInvariantViolation)?;
                translate_element_geometry(element, *delta_mm);
            }
            synchronize_connected_endpoints(document)?;
        }
        UndoStep::ArrangeElements { movements } => {
            for (element_ids, delta_mm) in movements {
                for element_id in element_ids {
                    let element = find_element_mut(document, *element_id)
                        .ok_or(EditorError::HistoryInvariantViolation)?;
                    translate_element_geometry(element, *delta_mm);
                }
            }
            synchronize_connected_endpoints(document)?;
        }
        UndoStep::RestoreZOrder { target, roots } => {
            let layer =
                find_layer_mut(document, *target).ok_or(EditorError::HistoryInvariantViolation)?;
            layer.scene.roots = roots.clone();
        }
        UndoStep::SetBounds {
            element_id,
            bounds_mm,
        } => {
            let element = find_element_mut(document, *element_id)
                .ok_or(EditorError::HistoryInvariantViolation)?;
            element.bounds_mm = *bounds_mm;
            synchronize_connected_endpoints(document)?;
        }
        UndoStep::SetRotation {
            element_id,
            rotation_deg,
        } => {
            let element = find_element_mut(document, *element_id)
                .ok_or(EditorError::HistoryInvariantViolation)?;
            element.rotation_deg = *rotation_deg;
            synchronize_connected_endpoints(document)?;
        }
        UndoStep::SetConnectorEndpoint {
            element_id,
            side,
            endpoint,
        } => {
            let element = find_element_mut(document, *element_id)
                .ok_or(EditorError::HistoryInvariantViolation)?;
            *connector_endpoint_mut(
                connector_mut(element).ok_or(EditorError::HistoryInvariantViolation)?,
                *side,
            ) = endpoint.clone();
            refresh_connector_bounds(document, *element_id)?;
            synchronize_connected_endpoints(document)?;
        }
        UndoStep::SetConnectorStyle {
            element_id,
            start_marker,
            end_marker,
            line_style,
            secondary_color,
        } => {
            let element = find_element_mut(document, *element_id)
                .ok_or(EditorError::HistoryInvariantViolation)?;
            let connector = connector_mut(element).ok_or(EditorError::HistoryInvariantViolation)?;
            connector.start_marker = *start_marker;
            connector.end_marker = *end_marker;
            connector.line_style = *line_style;
            connector.secondary_color = *secondary_color;
        }
        UndoStep::SetElementStyles { previous } => {
            for (element_id, style_id) in previous {
                let element = find_element_mut(document, *element_id)
                    .ok_or(EditorError::HistoryInvariantViolation)?;
                element.style_id = *style_id;
            }
        }
        UndoStep::RestoreElementAppearance {
            element_id,
            previous_style_id,
            dedicated_style_id,
            previous_dedicated_style,
        } => {
            find_element_mut(document, *element_id)
                .ok_or(EditorError::HistoryInvariantViolation)?
                .style_id = *previous_style_id;
            if let Some(previous) = previous_dedicated_style {
                if let Some(existing) = document
                    .styles
                    .iter_mut()
                    .find(|style| style.id == *dedicated_style_id)
                {
                    *existing = previous.clone();
                } else {
                    document.styles.push(previous.clone());
                }
            } else {
                document
                    .styles
                    .retain(|style| style.id != *dedicated_style_id);
            }
        }
        UndoStep::SetText { element_id, text } => {
            let element = find_element_mut(document, *element_id)
                .ok_or(EditorError::HistoryInvariantViolation)?;
            element.text = text.clone();
        }
        UndoStep::RemoveCreatedGroup {
            target,
            owner,
            previous_siblings,
            group_id,
        } => {
            let layer =
                find_layer_mut(document, *target).ok_or(EditorError::HistoryInvariantViolation)?;
            *owner_siblings_mut(&mut layer.scene, *owner)
                .ok_or(EditorError::HistoryInvariantViolation)? = previous_siblings.clone();
            let before = layer.scene.elements.len();
            layer
                .scene
                .elements
                .retain(|element| element.id != *group_id);
            if layer.scene.elements.len() + 1 != before {
                return Err(EditorError::HistoryInvariantViolation);
            }
        }
        UndoStep::RestoreUngrouped {
            target,
            owner,
            previous_siblings,
            group,
            element_index,
            detached,
        } => {
            let layer =
                find_layer_mut(document, *target).ok_or(EditorError::HistoryInvariantViolation)?;
            if *element_index > layer.scene.elements.len() {
                return Err(EditorError::HistoryInvariantViolation);
            }
            layer.scene.elements.insert(*element_index, group.clone());
            *owner_siblings_mut(&mut layer.scene, *owner)
                .ok_or(EditorError::HistoryInvariantViolation)? = previous_siblings.clone();
            restore_detached_connections(document, detached)?;
        }
        UndoStep::RemoveCreated { element_id } => {
            let target = layer_target_for_element(document, *element_id)
                .ok_or(EditorError::HistoryInvariantViolation)?;
            let layer =
                find_layer_mut(document, target).ok_or(EditorError::HistoryInvariantViolation)?;
            layer.scene.roots.retain(|root| root != element_id);
            let before = layer.scene.elements.len();
            layer
                .scene
                .elements
                .retain(|element| element.id != *element_id);
            if layer.scene.elements.len() + 1 != before {
                return Err(EditorError::HistoryInvariantViolation);
            }
        }
        UndoStep::RestoreDeleted { removed, detached } => {
            restore_deleted(document, removed, detached)?;
        }
    }
    Ok(())
}

fn restore_deleted(
    document: &mut Document,
    removed: &[RemovedElement],
    detached: &[DetachedConnection],
) -> Result<(), EditorError> {
    let mut records = removed.to_vec();
    records.sort_by_key(|record| (record.target, record.element_index));

    for record in &records {
        let layer = find_layer_mut(document, record.target)
            .ok_or(EditorError::HistoryInvariantViolation)?;
        if record.element_index > layer.scene.elements.len() {
            return Err(EditorError::HistoryInvariantViolation);
        }
        layer
            .scene
            .elements
            .insert(record.element_index, record.element.clone());
    }

    let mut roots: Vec<_> = records
        .iter()
        .filter_map(|record| {
            record
                .root_index
                .map(|index| (record.target, index, record.element.id))
        })
        .collect();
    roots.sort_by_key(|(target, index, _)| (*target, *index));
    for (target, index, element_id) in roots {
        let layer =
            find_layer_mut(document, target).ok_or(EditorError::HistoryInvariantViolation)?;
        if index > layer.scene.roots.len() {
            return Err(EditorError::HistoryInvariantViolation);
        }
        layer.scene.roots.insert(index, element_id);
    }

    restore_detached_connections(document, detached)?;

    Ok(())
}

fn restore_detached_connections(
    document: &mut Document,
    detached: &[DetachedConnection],
) -> Result<(), EditorError> {
    for connection in detached {
        let element = find_element_mut(document, connection.source_element_id)
            .ok_or(EditorError::HistoryInvariantViolation)?;
        let connector = connector_mut(element).ok_or(EditorError::HistoryInvariantViolation)?;
        match connection.side {
            ConnectorEndpointSide::Start => {
                connector.start.connection = Some(connection.connection)
            }
            ConnectorEndpointSide::End => connector.end.connection = Some(connection.connection),
        }
    }
    Ok(())
}

fn detach_deleted_connections(
    scene: &mut Scene,
    delete_ids: &BTreeSet<ElementId>,
    detached: &mut Vec<DetachedConnection>,
) {
    for element in &mut scene.elements {
        if delete_ids.contains(&element.id) {
            continue;
        }
        let source_element_id = element.id;
        let Some(connector) = connector_mut(element) else {
            continue;
        };

        if let Some(connection) = connector.start.connection {
            if delete_ids.contains(&connection.element_id) {
                detached.push(DetachedConnection {
                    source_element_id,
                    side: ConnectorEndpointSide::Start,
                    connection,
                });
                connector.start.connection = None;
            }
        }
        if let Some(connection) = connector.end.connection {
            if delete_ids.contains(&connection.element_id) {
                detached.push(DetachedConnection {
                    source_element_id,
                    side: ConnectorEndpointSide::End,
                    connection,
                });
                connector.end.connection = None;
            }
        }
    }
}

fn remove_deleted_from_scene(scene: &mut Scene, delete_ids: &BTreeSet<ElementId>) {
    scene.roots.retain(|root| !delete_ids.contains(root));
    scene
        .elements
        .retain(|element| !delete_ids.contains(&element.id));
}

fn expand_move_targets(
    document: &Document,
    element_ids: &[ElementId],
) -> Result<Vec<ElementId>, EditorError> {
    let mut seen = BTreeSet::new();
    let mut recursion_stack = BTreeSet::new();
    let mut expanded = Vec::new();
    for element_id in element_ids {
        collect_move_subtree(
            document,
            *element_id,
            &mut seen,
            &mut recursion_stack,
            &mut expanded,
        )?;
    }
    for element_id in &expanded {
        ensure_element_editable(document, *element_id)?;
    }
    Ok(expanded)
}

fn collect_move_subtree(
    document: &Document,
    element_id: ElementId,
    seen: &mut BTreeSet<ElementId>,
    recursion_stack: &mut BTreeSet<ElementId>,
    expanded: &mut Vec<ElementId>,
) -> Result<(), EditorError> {
    if recursion_stack.contains(&element_id) {
        return Err(EditorError::GroupHierarchyCycle(element_id));
    }
    if !seen.insert(element_id) {
        return Ok(());
    }
    let element =
        find_element(document, element_id).ok_or(EditorError::ElementNotFound(element_id))?;
    expanded.push(element_id);
    if let ElementKind::Group { children } = &element.kind {
        recursion_stack.insert(element_id);
        for child in children {
            collect_move_subtree(document, *child, seen, recursion_stack, expanded)?;
        }
        recursion_stack.remove(&element_id);
    }
    Ok(())
}

fn translate_point(point: &mut Point, delta_mm: Point) {
    point.x += delta_mm.x;
    point.y += delta_mm.y;
}

/// Translate every absolute document-space geometry field owned by an element.
/// Bounds are common to every element. Connector endpoint positions and curve
/// control points are also absolute document coordinates and must move with the
/// element; normalized polygon/port geometry remains relative to the translated
/// bounds. Keeping this in one helper prevents tools and future group operations
/// from inventing incomplete element-specific move paths.
fn translate_element_geometry(element: &mut Element, delta_mm: Point) {
    element.bounds_mm.x += delta_mm.x;
    element.bounds_mm.y += delta_mm.y;

    match &mut element.kind {
        ElementKind::StraightConnector { connector }
        | ElementKind::OrthogonalConnector { connector, .. } => {
            translate_free_connector_geometry(connector, delta_mm);
        }
        ElementKind::Curve {
            connector,
            control_points_mm,
            ..
        } => {
            for point in control_points_mm {
                translate_point(point, delta_mm);
            }
            if let Some(connector) = connector {
                translate_free_connector_geometry(connector, delta_mm);
            }
        }
        _ => {}
    }
}

fn translate_free_connector_geometry(connector: &mut Connector, delta_mm: Point) {
    if connector.start.connection.is_none() {
        translate_point(&mut connector.start.position_mm, delta_mm);
    }
    if connector.end.connection.is_none() {
        translate_point(&mut connector.end.position_mm, delta_mm);
    }
}

fn connector(element: &Element) -> Option<&Connector> {
    match &element.kind {
        ElementKind::StraightConnector { connector }
        | ElementKind::OrthogonalConnector { connector, .. } => Some(connector),
        ElementKind::Curve {
            connector: Some(connector),
            ..
        } => Some(connector),
        _ => None,
    }
}

fn connector_endpoint(connector: &Connector, side: ConnectorEndpointSide) -> &Endpoint {
    match side {
        ConnectorEndpointSide::Start => &connector.start,
        ConnectorEndpointSide::End => &connector.end,
    }
}

fn connector_endpoint_mut(connector: &mut Connector, side: ConnectorEndpointSide) -> &mut Endpoint {
    match side {
        ConnectorEndpointSide::Start => &mut connector.start,
        ConnectorEndpointSide::End => &mut connector.end,
    }
}

fn point_is_finite(point: Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn resolve_connection_position(
    document: &Document,
    source_element_id: ElementId,
    connection: Connection,
) -> Result<Point, EditorError> {
    let source_target = layer_target_for_element(document, source_element_id)
        .ok_or(EditorError::ElementNotFound(source_element_id))?;
    let target_target = layer_target_for_element(document, connection.element_id)
        .ok_or(EditorError::ElementNotFound(connection.element_id))?;
    if source_target != target_target {
        return Err(EditorError::ConnectionTargetDifferentScene {
            source_element_id,
            target_element_id: connection.element_id,
        });
    }
    let target = find_element(document, connection.element_id)
        .ok_or(EditorError::ElementNotFound(connection.element_id))?;
    port_document_position(target, connection.port_id)
}

fn port_document_position(target: &Element, port_id: PortId) -> Result<Point, EditorError> {
    let port =
        target
            .ports
            .iter()
            .find(|port| port.id == port_id)
            .ok_or(EditorError::PortNotFound {
                element_id: target.id,
                port_id,
            })?;
    let center = Point {
        x: target.bounds_mm.x + target.bounds_mm.width / 2.0,
        y: target.bounds_mm.y + target.bounds_mm.height / 2.0,
    };
    let unrotated = Point {
        x: target.bounds_mm.x + port.position.x * target.bounds_mm.width,
        y: target.bounds_mm.y + port.position.y * target.bounds_mm.height,
    };
    if target.rotation_deg == 0.0 {
        return Ok(unrotated);
    }
    let radians = target.rotation_deg.to_radians();
    let dx = unrotated.x - center.x;
    let dy = unrotated.y - center.y;
    Ok(Point {
        x: center.x + dx * radians.cos() - dy * radians.sin(),
        y: center.y + dx * radians.sin() + dy * radians.cos(),
    })
}

fn synchronize_connected_endpoints(document: &mut Document) -> Result<(), EditorError> {
    let mut updates = Vec::new();
    for layer in all_layers(document) {
        for source in &layer.scene.elements {
            let Some(connector) = connector(source) else {
                continue;
            };
            for side in [ConnectorEndpointSide::Start, ConnectorEndpointSide::End] {
                let Some(connection) = connector_endpoint(connector, side).connection else {
                    continue;
                };
                let target = layer
                    .scene
                    .elements
                    .iter()
                    .find(|element| element.id == connection.element_id)
                    .ok_or(EditorError::HistoryInvariantViolation)?;
                let position = port_document_position(target, connection.port_id)
                    .map_err(|_| EditorError::HistoryInvariantViolation)?;
                updates.push((source.id, side, position));
            }
        }
    }

    let mut touched = BTreeSet::new();
    for (element_id, side, position) in updates {
        let element =
            find_element_mut(document, element_id).ok_or(EditorError::HistoryInvariantViolation)?;
        connector_endpoint_mut(
            connector_mut(element).ok_or(EditorError::HistoryInvariantViolation)?,
            side,
        )
        .position_mm = position;
        touched.insert(element_id);
    }
    for element_id in touched {
        refresh_connector_bounds(document, element_id)?;
    }
    Ok(())
}

fn refresh_connector_bounds(
    document: &mut Document,
    element_id: ElementId,
) -> Result<(), EditorError> {
    let element =
        find_element_mut(document, element_id).ok_or(EditorError::HistoryInvariantViolation)?;
    let (start, end) = {
        let connector = connector_mut(element).ok_or(EditorError::HistoryInvariantViolation)?;
        (connector.start.position_mm, connector.end.position_mm)
    };
    element.bounds_mm = Rect {
        x: start.x.min(end.x),
        y: start.y.min(end.y),
        width: (start.x - end.x).abs().max(0.1),
        height: (start.y - end.y).abs().max(0.1),
    };
    Ok(())
}

fn connector_mut(element: &mut Element) -> Option<&mut Connector> {
    match &mut element.kind {
        ElementKind::StraightConnector { connector }
        | ElementKind::OrthogonalConnector { connector, .. } => Some(connector),
        ElementKind::Curve {
            connector: Some(connector),
            ..
        } => Some(connector),
        _ => None,
    }
}

fn element_geometry_is_valid(element: &Element) -> bool {
    if !rect_is_valid(element.bounds_mm) || !element.rotation_deg.is_finite() {
        return false;
    }
    if element
        .ports
        .iter()
        .any(|port| !port.position.x.is_finite() || !port.position.y.is_finite())
    {
        return false;
    }
    match &element.kind {
        ElementKind::Polygon { vertices } => vertices
            .iter()
            .all(|point| point.x.is_finite() && point.y.is_finite()),
        ElementKind::Curve {
            connector,
            control_points_mm,
            ..
        } => {
            control_points_mm
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
                && connector
                    .as_ref()
                    .map(connector_geometry_is_valid)
                    .unwrap_or(true)
        }
        ElementKind::StraightConnector { connector }
        | ElementKind::OrthogonalConnector { connector, .. } => {
            connector_geometry_is_valid(connector)
        }
        _ => true,
    }
}

fn connector_geometry_is_valid(connector: &Connector) -> bool {
    [connector.start.position_mm, connector.end.position_mm]
        .into_iter()
        .all(|point| point.x.is_finite() && point.y.is_finite())
}

fn rect_is_valid(rect: Rect) -> bool {
    rect.x.is_finite()
        && rect.y.is_finite()
        && rect.width.is_finite()
        && rect.height.is_finite()
        && rect.width >= 0.0
        && rect.height >= 0.0
}

fn text_block_is_valid(text: &TextBlock) -> bool {
    text.layout.margin_mm.is_finite() && text.layout.margin_mm >= 0.0
}

fn scene_element(scene: &Scene, element_id: ElementId) -> Option<&Element> {
    scene
        .elements
        .iter()
        .find(|element| element.id == element_id)
}

fn direct_sibling_owner(scene: &Scene, element_id: ElementId) -> Result<SiblingOwner, EditorError> {
    let root_count = scene.roots.iter().filter(|id| **id == element_id).count();
    let parents: Vec<_> = scene
        .elements
        .iter()
        .filter_map(|element| match &element.kind {
            ElementKind::Group { children } if children.contains(&element_id) => Some(element.id),
            _ => None,
        })
        .collect();

    match (root_count, parents.as_slice()) {
        (1, []) => Ok(SiblingOwner::Roots),
        (0, [parent]) => Ok(SiblingOwner::Group(*parent)),
        _ => Err(EditorError::AmbiguousElementOwnership(element_id)),
    }
}

fn owner_siblings(scene: &Scene, owner: SiblingOwner) -> Option<&Vec<ElementId>> {
    match owner {
        SiblingOwner::Roots => Some(&scene.roots),
        SiblingOwner::Group(group_id) => {
            let group = scene_element(scene, group_id)?;
            let ElementKind::Group { children } = &group.kind else {
                return None;
            };
            Some(children)
        }
    }
}

fn owner_siblings_mut(scene: &mut Scene, owner: SiblingOwner) -> Option<&mut Vec<ElementId>> {
    match owner {
        SiblingOwner::Roots => Some(&mut scene.roots),
        SiblingOwner::Group(group_id) => {
            let group = scene
                .elements
                .iter_mut()
                .find(|element| element.id == group_id)?;
            let ElementKind::Group { children } = &mut group.kind else {
                return None;
            };
            Some(children)
        }
    }
}

fn subtree_union_bounds(scene: &Scene, element_ids: &[ElementId]) -> Result<Rect, EditorError> {
    let mut bounds = None;
    let mut recursion_stack = BTreeSet::new();
    for element_id in element_ids {
        let next = subtree_visual_bounds(scene, *element_id, &mut recursion_stack)?;
        bounds = Some(match bounds {
            Some(current) => union_rect(current, next),
            None => next,
        });
    }
    bounds.ok_or(EditorError::GroupRequiresAtLeastTwoElements)
}

fn subtree_visual_bounds(
    scene: &Scene,
    element_id: ElementId,
    recursion_stack: &mut BTreeSet<ElementId>,
) -> Result<Rect, EditorError> {
    if !recursion_stack.insert(element_id) {
        return Err(EditorError::GroupHierarchyCycle(element_id));
    }
    let element =
        scene_element(scene, element_id).ok_or(EditorError::ElementNotFound(element_id))?;
    let result = match &element.kind {
        ElementKind::Group { children } if !children.is_empty() => {
            let mut bounds = None;
            for child in children {
                let child_bounds = subtree_visual_bounds(scene, *child, recursion_stack)?;
                bounds = Some(match bounds {
                    Some(current) => union_rect(current, child_bounds),
                    None => child_bounds,
                });
            }
            bounds.expect("non-empty group must produce child bounds")
        }
        _ => rotated_aabb(element.bounds_mm, element.rotation_deg),
    };
    recursion_stack.remove(&element_id);
    Ok(result)
}

fn union_rect(left: Rect, right: Rect) -> Rect {
    let min_x = left.x.min(right.x);
    let min_y = left.y.min(right.y);
    let max_x = (left.x + left.width).max(right.x + right.width);
    let max_y = (left.y + left.height).max(right.y + right.height);
    Rect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

fn rotated_aabb(rect: Rect, rotation_deg: f64) -> Rect {
    if rotation_deg == 0.0 {
        return rect;
    }
    let radians = rotation_deg.to_radians();
    let cos = radians.cos().abs();
    let sin = radians.sin().abs();
    let width = rect.width * cos + rect.height * sin;
    let height = rect.width * sin + rect.height * cos;
    let center_x = rect.x + rect.width / 2.0;
    let center_y = rect.y + rect.height / 2.0;
    Rect {
        x: center_x - width / 2.0,
        y: center_y - height / 2.0,
        width,
        height,
    }
}

fn preflight_elements(document: &Document, element_ids: &[ElementId]) -> Result<(), EditorError> {
    let mut unique = BTreeSet::new();
    for element_id in element_ids {
        if !unique.insert(*element_id) {
            return Err(EditorError::DuplicateCommandElement(*element_id));
        }
        ensure_element_editable(document, *element_id)?;
    }
    Ok(())
}

fn ensure_element_editable(document: &Document, element_id: ElementId) -> Result<(), EditorError> {
    let layer =
        find_element_layer(document, element_id).ok_or(EditorError::ElementNotFound(element_id))?;
    if layer.locked {
        return Err(EditorError::LayerLocked(layer.id));
    }
    Ok(())
}

fn size_is_valid(size: Size) -> bool {
    size.width.is_finite() && size.height.is_finite() && size.width > 0.0 && size.height > 0.0
}

fn layer_id_exists(document: &Document, layer_id: LayerId) -> bool {
    all_layers(document).any(|layer| layer.id == layer_id)
}

fn layer_scope_of(target: LayerTarget) -> LayerScope {
    match target {
        LayerTarget::Master { .. } => LayerScope::Master,
        LayerTarget::Page { page_id, .. } => LayerScope::Page { page_id },
    }
}

fn layers(document: &Document, scope: LayerScope) -> Result<&Vec<Layer>, EditorError> {
    match scope {
        LayerScope::Master => Ok(&document.master_layers),
        LayerScope::Page { page_id } => document
            .pages
            .iter()
            .find(|page| page.id == page_id)
            .map(|page| &page.layers)
            .ok_or(EditorError::PageNotFound(page_id)),
    }
}

fn layers_mut(document: &mut Document, scope: LayerScope) -> Result<&mut Vec<Layer>, EditorError> {
    match scope {
        LayerScope::Master => Ok(&mut document.master_layers),
        LayerScope::Page { page_id } => document
            .pages
            .iter_mut()
            .find(|page| page.id == page_id)
            .map(|page| &mut page.layers)
            .ok_or(EditorError::PageNotFound(page_id)),
    }
}

fn layer_element_ids(layer: &Layer) -> BTreeSet<ElementId> {
    layer
        .scene
        .elements
        .iter()
        .map(|element| element.id)
        .collect()
}

fn page_element_ids(page: &Page) -> BTreeSet<ElementId> {
    page.layers
        .iter()
        .flat_map(|layer| layer.scene.elements.iter().map(|element| element.id))
        .collect()
}

fn detach_document_connections(
    document: &mut Document,
    delete_ids: &BTreeSet<ElementId>,
    detached: &mut Vec<DetachedConnection>,
) {
    for layer in &mut document.master_layers {
        detach_deleted_connections(&mut layer.scene, delete_ids, detached);
    }
    for page in &mut document.pages {
        for layer in &mut page.layers {
            detach_deleted_connections(&mut layer.scene, delete_ids, detached);
        }
    }
}

fn layer_id_of(target: LayerTarget) -> LayerId {
    match target {
        LayerTarget::Master { layer_id } | LayerTarget::Page { layer_id, .. } => layer_id,
    }
}

fn find_layer(document: &Document, target: LayerTarget) -> Option<&Layer> {
    match target {
        LayerTarget::Master { layer_id } => document
            .master_layers
            .iter()
            .find(|layer| layer.id == layer_id),
        LayerTarget::Page { page_id, layer_id } => document
            .pages
            .iter()
            .find(|page| page.id == page_id)?
            .layers
            .iter()
            .find(|layer| layer.id == layer_id),
    }
}

fn find_layer_mut(document: &mut Document, target: LayerTarget) -> Option<&mut Layer> {
    match target {
        LayerTarget::Master { layer_id } => document
            .master_layers
            .iter_mut()
            .find(|layer| layer.id == layer_id),
        LayerTarget::Page { page_id, layer_id } => document
            .pages
            .iter_mut()
            .find(|page| page.id == page_id)?
            .layers
            .iter_mut()
            .find(|layer| layer.id == layer_id),
    }
}

fn all_layers(document: &Document) -> impl Iterator<Item = &Layer> {
    document
        .master_layers
        .iter()
        .chain(document.pages.iter().flat_map(|page| page.layers.iter()))
}

fn find_element_layer(document: &Document, element_id: ElementId) -> Option<&Layer> {
    all_layers(document).find(|layer| {
        layer
            .scene
            .elements
            .iter()
            .any(|element| element.id == element_id)
    })
}

fn layer_target_for_element(document: &Document, element_id: ElementId) -> Option<LayerTarget> {
    for layer in &document.master_layers {
        if layer
            .scene
            .elements
            .iter()
            .any(|element| element.id == element_id)
        {
            return Some(LayerTarget::Master { layer_id: layer.id });
        }
    }
    for page in &document.pages {
        for layer in &page.layers {
            if layer
                .scene
                .elements
                .iter()
                .any(|element| element.id == element_id)
            {
                return Some(LayerTarget::Page {
                    page_id: page.id,
                    layer_id: layer.id,
                });
            }
        }
    }
    None
}

fn find_element(document: &Document, element_id: ElementId) -> Option<&Element> {
    find_element_layer(document, element_id)?
        .scene
        .elements
        .iter()
        .find(|element| element.id == element_id)
}

fn find_element_mut(document: &mut Document, element_id: ElementId) -> Option<&mut Element> {
    for layer in &mut document.master_layers {
        if let Some(element) = layer
            .scene
            .elements
            .iter_mut()
            .find(|element| element.id == element_id)
        {
            return Some(element);
        }
    }
    for page in &mut document.pages {
        for layer in &mut page.layers {
            if let Some(element) = layer
                .scene
                .elements
                .iter_mut()
                .find(|element| element.id == element_id)
            {
                return Some(element);
            }
        }
    }
    None
}

fn port_exists(document: &Document, port_id: PortId) -> bool {
    all_layers(document).any(|layer| {
        layer
            .scene
            .elements
            .iter()
            .flat_map(|element| element.ports.iter())
            .any(|port| port.id == port_id)
    })
}

#[cfg(test)]
mod tests {
    use next_domain::{
        AnchorSet, AssetId, Connection, ConnectorLabelStyle, CurveKind, DocumentDefaults,
        DocumentId, ElementKind, ElementStyle, Endpoint, LayerId, LineStyle, MarkerStyle,
        NormalizedPoint, Page, PageId, Port, RichTextDocument, Scene, Size,
        TextHorizontalAlignment, TextLayout, TextVerticalAlignment,
    };

    use super::*;

    fn element(id: ElementId, x: f64, y: f64) -> Element {
        Element {
            id,
            name: String::new(),
            bounds_mm: Rect {
                x,
                y,
                width: 10.0,
                height: 5.0,
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

    fn fixture(locked: bool) -> (EditorSession, ElementId, ElementId, ElementId, LayerId) {
        let first = ElementId::new();
        let second = ElementId::new();
        let master = ElementId::new();
        let page_id = PageId::new();
        let layer_id = LayerId::new();
        let master_layer_id = LayerId::new();

        let document = Document {
            id: DocumentId::new(),
            name: "Editor test".to_owned(),
            defaults: defaults(),
            master_layers: vec![Layer {
                id: master_layer_id,
                name: "Master".to_owned(),
                visible: true,
                locked: false,
                draw_color: None,
                scene: Scene {
                    roots: vec![master],
                    elements: vec![element(master, 100.0, 100.0)],
                },
            }],
            pages: vec![Page {
                id: page_id,
                name: "Page".to_owned(),
                size_mm: next_domain::Size {
                    width: 210.0,
                    height: 297.0,
                },
                layers: vec![Layer {
                    id: layer_id,
                    name: "Layer".to_owned(),
                    visible: true,
                    locked,
                    draw_color: None,
                    scene: Scene {
                        roots: vec![first, second],
                        elements: vec![element(first, 1.0, 2.0), element(second, 20.0, 30.0)],
                    },
                }],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        };
        (
            EditorSession::from_artifact(NextArtifact::document(document)).unwrap(),
            first,
            second,
            master,
            layer_id,
        )
    }

    fn bounds(session: &EditorSession, id: ElementId) -> Rect {
        find_element(session.document(), id).unwrap().bounds_mm
    }

    fn roots(session: &EditorSession, target: LayerTarget) -> Vec<ElementId> {
        find_layer(session.document(), target)
            .unwrap()
            .scene
            .roots
            .clone()
    }

    fn z_order_fixture() -> (EditorSession, Vec<ElementId>, LayerTarget) {
        let (base, first, second, _, _) = fixture(false);
        let third = ElementId::new();
        let fourth = ElementId::new();
        let mut document = base.document().clone();
        let layer = &mut document.pages[0].layers[0];
        layer.scene.roots.extend([third, fourth]);
        layer.scene.elements.push(element(third, 40.0, 50.0));
        layer.scene.elements.push(element(fourth, 60.0, 70.0));
        let session = EditorSession::from_artifact(NextArtifact::document(document)).unwrap();
        let target = session.active_layer().unwrap();
        (session, vec![first, second, third, fourth], target)
    }

    fn with_styles(session: EditorSession, style_ids: &[StyleId]) -> EditorSession {
        let mut document = session.document().clone();
        document
            .styles
            .extend(style_ids.iter().map(|style_id| ElementStyle {
                id: *style_id,
                stroke: None,
                fill: None,
                text_color: None,
            }));
        EditorSession::from_artifact(NextArtifact::document(document)).unwrap()
    }

    fn style_ref(session: &EditorSession, id: ElementId) -> Option<StyleId> {
        find_element(session.document(), id).unwrap().style_id
    }

    fn text_value(session: &EditorSession, id: ElementId) -> Option<TextBlock> {
        find_element(session.document(), id).unwrap().text.clone()
    }

    fn text_block(margin_mm: f64) -> TextBlock {
        TextBlock {
            content: RichTextDocument::default(),
            layout: TextLayout {
                horizontal: TextHorizontalAlignment::Left,
                vertical: TextVerticalAlignment::Top,
                margin_mm,
            },
        }
    }

    fn endpoint_connection_fixture(
        locked: bool,
    ) -> (EditorSession, ElementId, ElementId, ElementId, PortId) {
        let (session, source_id, target_id, master_id, _) = fixture(locked);
        let port_id = PortId::new();
        let mut document = session.document().clone();
        {
            let target = find_element_mut(&mut document, target_id).unwrap();
            target.ports.push(Port {
                id: port_id,
                index: 0,
                position: NormalizedPoint { x: 1.0, y: 0.5 },
            });
        }
        {
            let source = find_element_mut(&mut document, source_id).unwrap();
            source.kind = ElementKind::StraightConnector {
                connector: Connector {
                    start: Endpoint {
                        position_mm: Point { x: 1.0, y: 2.0 },
                        connection: None,
                    },
                    end: Endpoint {
                        position_mm: Point { x: 11.0, y: 7.0 },
                        connection: None,
                    },
                    start_marker: MarkerStyle::None,
                    end_marker: MarkerStyle::None,
                    line_style: LineStyle::Solid,
                    secondary_color: None,
                },
            };
        }
        (
            EditorSession::from_artifact(NextArtifact::document(document)).unwrap(),
            source_id,
            target_id,
            master_id,
            port_id,
        )
    }

    fn connector_endpoint_value(
        session: &EditorSession,
        element_id: ElementId,
        side: ConnectorEndpointSide,
    ) -> Endpoint {
        connector_endpoint(
            connector(find_element(session.document(), element_id).unwrap()).unwrap(),
            side,
        )
        .clone()
    }

    #[test]
    fn z_order_preserves_multi_selection_order_and_round_trips_history() {
        let (mut session, ids, target) = z_order_fixture();
        let [first, second, third, fourth]: [ElementId; 4] = ids.try_into().unwrap();
        let before = session.current_history_state();

        assert!(
            session
                .execute(EditCommand::ReorderElements {
                    element_ids: vec![second, third],
                    operation: ZOrderOperation::BringToFront,
                })
                .unwrap()
        );
        let after = session.current_history_state();
        assert_ne!(after, before);
        assert_eq!(roots(&session, target), vec![first, fourth, second, third]);

        assert!(session.undo().unwrap());
        assert_eq!(session.current_history_state(), before);
        assert_eq!(roots(&session, target), vec![first, second, third, fourth]);

        assert!(session.redo().unwrap());
        assert_eq!(session.current_history_state(), after);
        assert_eq!(roots(&session, target), vec![first, fourth, second, third]);
    }

    #[test]
    fn z_order_one_step_moves_are_stable_and_boundaries_are_noops() {
        let (mut session, ids, target) = z_order_fixture();
        let [first, second, third, fourth]: [ElementId; 4] = ids.try_into().unwrap();

        assert!(
            session
                .execute(EditCommand::ReorderElements {
                    element_ids: vec![first, third],
                    operation: ZOrderOperation::BringForward,
                })
                .unwrap()
        );
        assert_eq!(roots(&session, target), vec![second, first, fourth, third]);
        assert!(session.undo().unwrap());

        assert!(
            session
                .execute(EditCommand::ReorderElements {
                    element_ids: vec![second, fourth],
                    operation: ZOrderOperation::SendBackward,
                })
                .unwrap()
        );
        assert_eq!(roots(&session, target), vec![second, first, fourth, third]);
        assert!(session.undo().unwrap());

        let history = session.current_history_state();
        assert!(
            !session
                .execute(EditCommand::ReorderElements {
                    element_ids: vec![fourth],
                    operation: ZOrderOperation::BringToFront,
                })
                .unwrap()
        );
        assert!(
            !session
                .execute(EditCommand::ReorderElements {
                    element_ids: vec![first],
                    operation: ZOrderOperation::SendToBack,
                })
                .unwrap()
        );
        assert_eq!(session.current_history_state(), history);
    }

    #[test]
    fn z_order_rejects_cross_layer_and_group_children_but_allows_top_level_group_noops() {
        let (mut session, first, second, master, _) = fixture(false);
        let history = session.current_history_state();
        assert!(matches!(
            session.execute(EditCommand::ReorderElements {
                element_ids: vec![first, master],
                operation: ZOrderOperation::BringToFront,
            }),
            Err(EditorError::ZOrderDifferentLayers)
        ));
        assert_eq!(session.current_history_state(), history);

        let group_id = ElementId::new();
        assert!(
            session
                .execute(EditCommand::GroupElements {
                    group_id,
                    element_ids: vec![first, second],
                    name: "Group".to_owned(),
                })
                .unwrap()
        );
        let grouped_history = session.current_history_state();
        assert!(
            !session
                .execute(EditCommand::ReorderElements {
                    element_ids: vec![group_id],
                    operation: ZOrderOperation::SendToBack,
                })
                .unwrap()
        );
        assert_eq!(session.current_history_state(), grouped_history);
        assert!(matches!(
            session.execute(EditCommand::ReorderElements {
                element_ids: vec![first],
                operation: ZOrderOperation::SendToBack,
            }),
            Err(EditorError::ZOrderRequiresTopLevelElement(id)) if id == first
        ));
        assert_eq!(session.current_history_state(), grouped_history);
    }

    #[test]
    fn z_order_moves_top_level_groups_without_mutating_child_structure() {
        let (mut session, ids, target) = z_order_fixture();
        let [first, second, third, fourth]: [ElementId; 4] = ids.try_into().unwrap();
        let group_id = ElementId::new();
        session
            .execute(EditCommand::GroupElements {
                group_id,
                element_ids: vec![first, second],
                name: "Pair".to_owned(),
            })
            .unwrap();
        assert_eq!(roots(&session, target), vec![group_id, third, fourth]);
        let children = |session: &EditorSession| {
            let group = find_element(session.document(), group_id).unwrap();
            let ElementKind::Group { children } = &group.kind else {
                panic!("expected group")
            };
            children.clone()
        };
        assert_eq!(children(&session), vec![first, second]);
        let grouped_history = session.current_history_state();

        assert!(
            session
                .execute(EditCommand::ReorderElements {
                    element_ids: vec![group_id],
                    operation: ZOrderOperation::BringToFront,
                })
                .unwrap()
        );
        let front_history = session.current_history_state();
        assert_eq!(roots(&session, target), vec![third, fourth, group_id]);
        assert_eq!(children(&session), vec![first, second]);

        assert!(session.undo().unwrap());
        assert_eq!(session.current_history_state(), grouped_history);
        assert_eq!(roots(&session, target), vec![group_id, third, fourth]);
        assert_eq!(children(&session), vec![first, second]);
        assert!(session.redo().unwrap());
        assert_eq!(session.current_history_state(), front_history);
        assert_eq!(roots(&session, target), vec![third, fourth, group_id]);
        assert_eq!(children(&session), vec![first, second]);

        // Caller order does not override the selected roots' existing relative order.
        assert!(
            session
                .execute(EditCommand::ReorderElements {
                    element_ids: vec![group_id, third],
                    operation: ZOrderOperation::SendToBack,
                })
                .unwrap()
        );
        assert_eq!(roots(&session, target), vec![third, group_id, fourth]);
        assert_eq!(children(&session), vec![first, second]);
        assert!(session.undo().unwrap());
        assert_eq!(roots(&session, target), vec![third, fourth, group_id]);
        assert_eq!(children(&session), vec![first, second]);
    }

    #[test]
    fn structural_group_reconstruction_supports_singleton_and_empty_snapshots() {
        let (mut session, first, second, _, _) = fixture(false);
        let target = session.active_layer().unwrap();
        let singleton_id = ElementId::new();
        let mut singleton = element(singleton_id, 3.0, 4.0);
        singleton.name = "Imported singleton".to_owned();
        singleton.bounds_mm = Rect {
            x: 2.0,
            y: 1.0,
            width: 17.0,
            height: 9.0,
        };
        singleton.kind = ElementKind::Group {
            children: vec![first],
        };
        assert!(
            session
                .execute(EditCommand::CreateStructuralGroup {
                    target,
                    group: singleton.clone(),
                    z_index: None,
                })
                .unwrap()
        );
        assert_eq!(roots(&session, target), vec![singleton_id, second]);
        assert_eq!(
            find_element(session.document(), singleton_id).unwrap(),
            &singleton
        );

        let empty_id = ElementId::new();
        let mut empty = element(empty_id, 9.0, 11.0);
        empty.name = "Imported empty".to_owned();
        empty.kind = ElementKind::Group {
            children: Vec::new(),
        };
        assert!(
            session
                .execute(EditCommand::CreateStructuralGroup {
                    target,
                    group: empty.clone(),
                    z_index: Some(0),
                })
                .unwrap()
        );
        assert_eq!(
            roots(&session, target),
            vec![empty_id, singleton_id, second]
        );
        assert_eq!(find_element(session.document(), empty_id).unwrap(), &empty);
        assert!(session.undo().unwrap());
        assert!(session.undo().unwrap());
        assert_eq!(roots(&session, target), vec![first, second]);
        assert!(session.redo().unwrap());
        assert!(session.redo().unwrap());
        assert_eq!(
            roots(&session, target),
            vec![empty_id, singleton_id, second]
        );
    }

    #[test]
    fn appearance_edit_uses_element_owned_style_and_is_one_undoable_step() {
        let (base, first, second, _, _) = fixture(false);
        let shared = StyleId::new();
        let mut document = base.document().clone();
        document.styles.push(ElementStyle {
            id: shared,
            stroke: Some(StrokeStyle {
                width_mm: 0.4,
                color: Color::SystemPalette { index: 7 },
            }),
            fill: None,
            text_color: None,
        });
        find_element_mut(&mut document, first).unwrap().style_id = Some(shared);
        find_element_mut(&mut document, second).unwrap().style_id = Some(shared);
        let mut session = EditorSession::from_artifact(NextArtifact::document(document)).unwrap();
        let before = session.current_history_state();

        assert!(
            session
                .execute(EditCommand::SetElementAppearance {
                    element_id: first,
                    stroke: Some(StrokeStyle {
                        width_mm: 0.8,
                        color: Color::Rgba {
                            r: 10,
                            g: 20,
                            b: 30,
                            a: 255
                        },
                    }),
                    fill: Some(FillStyle {
                        color: Color::Rgba {
                            r: 240,
                            g: 230,
                            b: 220,
                            a: 255
                        },
                        gradient: None,
                    }),
                    text_color: None,
                })
                .unwrap()
        );
        let after = session.current_history_state();
        assert_ne!(after, before);
        let dedicated = style_ref(&session, first).unwrap();
        assert_ne!(dedicated, shared);
        assert_eq!(style_ref(&session, second), Some(shared));
        assert_eq!(session.document().styles.len(), 2);
        assert_eq!(
            session
                .document()
                .styles
                .iter()
                .find(|style| style.id == shared)
                .unwrap()
                .stroke
                .as_ref()
                .unwrap()
                .color,
            Color::SystemPalette { index: 7 }
        );

        assert!(session.undo().unwrap());
        assert_eq!(session.current_history_state(), before);
        assert_eq!(style_ref(&session, first), Some(shared));
        assert!(
            session
                .document()
                .styles
                .iter()
                .all(|style| style.id != dedicated)
        );

        assert!(session.redo().unwrap());
        assert_eq!(session.current_history_state(), after);
        assert_eq!(style_ref(&session, first), Some(dedicated));
        assert_eq!(session.document().styles.len(), 2);
    }

    #[test]
    fn connector_endpoint_connection_is_canonical_and_undoable() {
        let (mut session, source, target, _, port_id) = endpoint_connection_fixture(false);
        let connection = Connection {
            element_id: target,
            port_id,
        };
        session
            .execute(EditCommand::SetConnectorEndpoint {
                element_id: source,
                side: ConnectorEndpointSide::Start,
                position_mm: Point {
                    x: -999.0,
                    y: -999.0,
                },
                connection: Some(connection),
            })
            .unwrap();

        let connected = connector_endpoint_value(&session, source, ConnectorEndpointSide::Start);
        assert_eq!(connected.connection, Some(connection));
        assert_eq!(connected.position_mm, Point { x: 30.0, y: 32.5 });
        assert_eq!(
            bounds(&session, source),
            Rect {
                x: 11.0,
                y: 7.0,
                width: 19.0,
                height: 25.5,
            }
        );

        let snapshot = session
            .connector_endpoint_snapshot(source)
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.kind, ConnectorGeometryKind::Straight);
        assert_eq!(snapshot.start, connected);
        let ports = session
            .resolved_ports(session.active_layer().unwrap())
            .unwrap();
        assert!(ports.iter().any(|port| port.element_id == target
            && port.port_id == port_id
            && port.position_mm == Point { x: 30.0, y: 32.5 }));

        session.undo().unwrap();
        let free = connector_endpoint_value(&session, source, ConnectorEndpointSide::Start);
        assert_eq!(free.connection, None);
        assert_eq!(free.position_mm, Point { x: 1.0, y: 2.0 });

        session.redo().unwrap();
        assert_eq!(
            connector_endpoint_value(&session, source, ConnectorEndpointSide::Start).position_mm,
            Point { x: 30.0, y: 32.5 }
        );
    }

    #[test]
    fn connector_endpoint_connection_validates_source_scene_port_and_lock() {
        let (mut session, source, target, master, port_id) = endpoint_connection_fixture(false);
        let missing_port = PortId::new();
        let error = session
            .execute(EditCommand::SetConnectorEndpoint {
                element_id: source,
                side: ConnectorEndpointSide::Start,
                position_mm: Point::default(),
                connection: Some(Connection {
                    element_id: target,
                    port_id: missing_port,
                }),
            })
            .unwrap_err();
        assert!(matches!(
            error,
            EditorError::PortNotFound { element_id, port_id }
                if element_id == target && port_id == missing_port
        ));

        {
            let document = &mut session.document;
            find_element_mut(document, master)
                .unwrap()
                .ports
                .push(Port {
                    id: PortId::new(),
                    index: 0,
                    position: NormalizedPoint { x: 0.5, y: 0.5 },
                });
        }
        let master_port = find_element(session.document(), master).unwrap().ports[0].id;
        let error = session
            .execute(EditCommand::SetConnectorEndpoint {
                element_id: source,
                side: ConnectorEndpointSide::Start,
                position_mm: Point::default(),
                connection: Some(Connection {
                    element_id: master,
                    port_id: master_port,
                }),
            })
            .unwrap_err();
        assert!(matches!(
            error,
            EditorError::ConnectionTargetDifferentScene {
                source_element_id,
                target_element_id
            } if source_element_id == source && target_element_id == master
        ));

        let (mut locked, locked_source, locked_target, _, locked_port) =
            endpoint_connection_fixture(true);
        let error = locked
            .execute(EditCommand::SetConnectorEndpoint {
                element_id: locked_source,
                side: ConnectorEndpointSide::Start,
                position_mm: Point::default(),
                connection: Some(Connection {
                    element_id: locked_target,
                    port_id: locked_port,
                }),
            })
            .unwrap_err();
        assert!(matches!(error, EditorError::LayerLocked(_)));

        // A non-connector can never acquire hidden endpoint state.
        let error = session
            .execute(EditCommand::SetConnectorEndpoint {
                element_id: target,
                side: ConnectorEndpointSide::Start,
                position_mm: Point { x: 1.0, y: 1.0 },
                connection: None,
            })
            .unwrap_err();
        assert!(matches!(error, EditorError::ElementIsNotConnector(id) if id == target));

        // Keep the valid port referenced so the fixture remains meaningful.
        assert_eq!(
            port_id,
            find_element(session.document(), target).unwrap().ports[0].id
        );
    }

    #[test]
    fn connected_endpoint_tracks_target_move_without_double_translation() {
        let (mut session, source, target, _, port_id) = endpoint_connection_fixture(false);
        session
            .execute(EditCommand::SetConnectorEndpoint {
                element_id: source,
                side: ConnectorEndpointSide::Start,
                position_mm: Point::default(),
                connection: Some(Connection {
                    element_id: target,
                    port_id,
                }),
            })
            .unwrap();

        session
            .execute(EditCommand::MoveElements {
                element_ids: vec![target],
                delta_mm: Point { x: 5.0, y: -2.0 },
            })
            .unwrap();
        assert_eq!(
            connector_endpoint_value(&session, source, ConnectorEndpointSide::Start).position_mm,
            Point { x: 35.0, y: 30.5 }
        );
        assert_eq!(
            bounds(&session, source),
            Rect {
                x: 11.0,
                y: 7.0,
                width: 24.0,
                height: 23.5,
            }
        );
        session.undo().unwrap();
        assert_eq!(
            connector_endpoint_value(&session, source, ConnectorEndpointSide::Start).position_mm,
            Point { x: 30.0, y: 32.5 }
        );

        session
            .execute(EditCommand::MoveElements {
                element_ids: vec![source],
                delta_mm: Point { x: 4.0, y: 3.0 },
            })
            .unwrap();
        let start = connector_endpoint_value(&session, source, ConnectorEndpointSide::Start);
        let end = connector_endpoint_value(&session, source, ConnectorEndpointSide::End);
        assert_eq!(start.position_mm, Point { x: 30.0, y: 32.5 });
        assert_eq!(end.position_mm, Point { x: 15.0, y: 10.0 });
        session.undo().unwrap();

        session
            .execute(EditCommand::MoveElements {
                element_ids: vec![source, target],
                delta_mm: Point { x: 4.0, y: 3.0 },
            })
            .unwrap();
        let start = connector_endpoint_value(&session, source, ConnectorEndpointSide::Start);
        let end = connector_endpoint_value(&session, source, ConnectorEndpointSide::End);
        assert_eq!(start.position_mm, Point { x: 34.0, y: 35.5 });
        assert_eq!(end.position_mm, Point { x: 15.0, y: 10.0 });
    }

    #[test]
    fn connected_endpoint_tracks_target_bounds_and_rotation_with_undo_redo() {
        let (mut session, source, target, _, port_id) = endpoint_connection_fixture(false);
        session
            .execute(EditCommand::SetConnectorEndpoint {
                element_id: source,
                side: ConnectorEndpointSide::Start,
                position_mm: Point::default(),
                connection: Some(Connection {
                    element_id: target,
                    port_id,
                }),
            })
            .unwrap();
        session
            .execute(EditCommand::SetBounds {
                element_id: target,
                bounds_mm: Rect {
                    x: 40.0,
                    y: 50.0,
                    width: 20.0,
                    height: 10.0,
                },
            })
            .unwrap();
        assert_eq!(
            connector_endpoint_value(&session, source, ConnectorEndpointSide::Start).position_mm,
            Point { x: 60.0, y: 55.0 }
        );
        session
            .execute(EditCommand::SetRotation {
                element_id: target,
                rotation_deg: 90.0,
            })
            .unwrap();
        let rotated = connector_endpoint_value(&session, source, ConnectorEndpointSide::Start);
        assert!((rotated.position_mm.x - 50.0).abs() < 1e-9);
        assert!((rotated.position_mm.y - 65.0).abs() < 1e-9);

        session.undo().unwrap();
        assert_eq!(
            connector_endpoint_value(&session, source, ConnectorEndpointSide::Start).position_mm,
            Point { x: 60.0, y: 55.0 }
        );
        session.undo().unwrap();
        assert_eq!(
            connector_endpoint_value(&session, source, ConnectorEndpointSide::Start).position_mm,
            Point { x: 30.0, y: 32.5 }
        );
        session.redo().unwrap();
        session.redo().unwrap();
        let rotated = connector_endpoint_value(&session, source, ConnectorEndpointSide::Start);
        assert!((rotated.position_mm.x - 50.0).abs() < 1e-9);
        assert!((rotated.position_mm.y - 65.0).abs() < 1e-9);
    }

    #[test]
    fn move_is_atomic_and_undoable() {
        let (mut session, first, second, _, _) = fixture(false);
        session
            .execute(EditCommand::MoveElements {
                element_ids: vec![first, second],
                delta_mm: Point { x: 4.0, y: -3.0 },
            })
            .unwrap();
        assert_eq!(bounds(&session, first).x, 5.0);
        assert_eq!(bounds(&session, first).y, -1.0);
        assert_eq!(bounds(&session, second).x, 24.0);
        assert!(session.can_undo());

        session.undo().unwrap();
        assert_eq!(bounds(&session, first).x, 1.0);
        assert_eq!(bounds(&session, first).y, 2.0);
        assert_eq!(bounds(&session, second).x, 20.0);

        session.redo().unwrap();
        assert_eq!(bounds(&session, first).x, 5.0);
        assert_eq!(bounds(&session, second).x, 24.0);
    }

    #[test]
    fn move_translates_connector_endpoint_geometry_and_undo_redo() {
        let (mut session, first, _, _, _) = fixture(false);
        {
            let document = &mut session.document;
            let element = find_element_mut(document, first).unwrap();
            element.kind = ElementKind::StraightConnector {
                connector: Connector {
                    start: Endpoint {
                        position_mm: Point { x: 1.0, y: 2.0 },
                        connection: None,
                    },
                    end: Endpoint {
                        position_mm: Point { x: 11.0, y: 7.0 },
                        connection: None,
                    },
                    start_marker: MarkerStyle::None,
                    end_marker: MarkerStyle::None,
                    line_style: LineStyle::Solid,
                    secondary_color: None,
                },
            };
        }

        session
            .execute(EditCommand::MoveElements {
                element_ids: vec![first],
                delta_mm: Point { x: 4.0, y: -3.0 },
            })
            .unwrap();
        let moved = find_element(session.document(), first).unwrap();
        let ElementKind::StraightConnector { connector } = &moved.kind else {
            panic!("expected connector")
        };
        assert_eq!(moved.bounds_mm.x, 5.0);
        assert_eq!(moved.bounds_mm.y, -1.0);
        assert_eq!(connector.start.position_mm, Point { x: 5.0, y: -1.0 });
        assert_eq!(connector.end.position_mm, Point { x: 15.0, y: 4.0 });

        session.undo().unwrap();
        let restored = find_element(session.document(), first).unwrap();
        let ElementKind::StraightConnector { connector } = &restored.kind else {
            panic!("expected connector")
        };
        assert_eq!(restored.bounds_mm.x, 1.0);
        assert_eq!(restored.bounds_mm.y, 2.0);
        assert_eq!(connector.start.position_mm, Point { x: 1.0, y: 2.0 });
        assert_eq!(connector.end.position_mm, Point { x: 11.0, y: 7.0 });

        session.redo().unwrap();
        let redone = find_element(session.document(), first).unwrap();
        let ElementKind::StraightConnector { connector } = &redone.kind else {
            panic!("expected connector")
        };
        assert_eq!(connector.start.position_mm, Point { x: 5.0, y: -1.0 });
        assert_eq!(connector.end.position_mm, Point { x: 15.0, y: 4.0 });
    }

    #[test]
    fn move_translates_curve_control_points_and_optional_connector() {
        let (mut session, first, _, _, _) = fixture(false);
        {
            let document = &mut session.document;
            let element = find_element_mut(document, first).unwrap();
            element.kind = ElementKind::Curve {
                curve_kind: CurveKind::Bezier,
                connector: Some(Connector {
                    start: Endpoint {
                        position_mm: Point { x: 2.0, y: 3.0 },
                        connection: None,
                    },
                    end: Endpoint {
                        position_mm: Point { x: 8.0, y: 9.0 },
                        connection: None,
                    },
                    start_marker: MarkerStyle::None,
                    end_marker: MarkerStyle::None,
                    line_style: LineStyle::Solid,
                    secondary_color: None,
                }),
                control_points_mm: vec![Point { x: 3.0, y: 4.0 }, Point { x: 6.0, y: 7.0 }],
            };
        }

        session
            .execute(EditCommand::MoveElements {
                element_ids: vec![first],
                delta_mm: Point { x: -2.0, y: 5.0 },
            })
            .unwrap();
        let moved = find_element(session.document(), first).unwrap();
        let ElementKind::Curve {
            connector,
            control_points_mm,
            ..
        } = &moved.kind
        else {
            panic!("expected curve")
        };
        assert_eq!(
            control_points_mm,
            &vec![Point { x: 1.0, y: 9.0 }, Point { x: 4.0, y: 12.0 }]
        );
        let connector = connector.as_ref().unwrap();
        assert_eq!(connector.start.position_mm, Point { x: 0.0, y: 8.0 });
        assert_eq!(connector.end.position_mm, Point { x: 6.0, y: 14.0 });

        session.undo().unwrap();
        let restored = find_element(session.document(), first).unwrap();
        let ElementKind::Curve {
            connector,
            control_points_mm,
            ..
        } = &restored.kind
        else {
            panic!("expected curve")
        };
        assert_eq!(
            control_points_mm,
            &vec![Point { x: 3.0, y: 4.0 }, Point { x: 6.0, y: 7.0 }]
        );
        let connector = connector.as_ref().unwrap();
        assert_eq!(connector.start.position_mm, Point { x: 2.0, y: 3.0 });
        assert_eq!(connector.end.position_mm, Point { x: 8.0, y: 9.0 });
    }

    #[test]
    fn grouping_preserves_order_moves_subtree_and_round_trips_history() {
        let (mut session, first, second, _, layer_id) = fixture(false);
        let page_id = session.active_page_id().unwrap();
        let target = LayerTarget::Page { page_id, layer_id };
        let group_id = ElementId::new();

        session
            .execute(EditCommand::GroupElements {
                group_id,
                element_ids: vec![first, second],
                name: "Pair".to_owned(),
            })
            .unwrap();
        assert_eq!(roots(&session, target), vec![group_id]);
        let group = find_element(session.document(), group_id).unwrap();
        let ElementKind::Group { children } = &group.kind else {
            panic!("expected group")
        };
        assert_eq!(children, &vec![first, second]);
        assert_eq!(
            group.bounds_mm,
            Rect {
                x: 1.0,
                y: 2.0,
                width: 29.0,
                height: 33.0,
            }
        );

        session
            .execute(EditCommand::MoveElements {
                element_ids: vec![group_id, first],
                delta_mm: Point { x: 5.0, y: -2.0 },
            })
            .unwrap();
        assert_eq!(bounds(&session, group_id).x, 6.0);
        assert_eq!(bounds(&session, first).x, 6.0);
        assert_eq!(bounds(&session, second).x, 25.0);
        // Selecting both the group and one descendant must not double-translate it.
        assert_eq!(bounds(&session, first).y, 0.0);

        session.undo().unwrap();
        assert_eq!(bounds(&session, first).x, 1.0);
        assert_eq!(bounds(&session, second).x, 20.0);
        session.undo().unwrap();
        assert_eq!(roots(&session, target), vec![first, second]);
        assert!(find_element(session.document(), group_id).is_none());

        session.redo().unwrap();
        session.redo().unwrap();
        assert_eq!(roots(&session, target), vec![group_id]);
        assert_eq!(bounds(&session, first).x, 6.0);
        assert_eq!(bounds(&session, second).x, 25.0);
    }

    #[test]
    fn grouping_supports_nested_direct_siblings_and_rejects_z_order_changes() {
        let (session, first, second, _, layer_id) = fixture(false);
        let page_id = session.active_page_id().unwrap();
        let parent_id = ElementId::new();
        let nested_id = ElementId::new();
        let mut document = session.document().clone();
        let target = LayerTarget::Page { page_id, layer_id };
        let layer = find_layer_mut(&mut document, target).unwrap();
        layer.scene.roots = vec![parent_id];
        let mut parent = element(parent_id, 0.0, 0.0);
        parent.kind = ElementKind::Group {
            children: vec![first, second],
        };
        layer.scene.elements.push(parent);
        let mut session = EditorSession::from_artifact(NextArtifact::document(document)).unwrap();

        session
            .execute(EditCommand::GroupElements {
                group_id: nested_id,
                element_ids: vec![second, first],
                name: "Nested".to_owned(),
            })
            .unwrap();
        let parent = find_element(session.document(), parent_id).unwrap();
        let ElementKind::Group { children } = &parent.kind else {
            panic!("expected parent group")
        };
        assert_eq!(children, &vec![nested_id]);
        let nested = find_element(session.document(), nested_id).unwrap();
        let ElementKind::Group { children } = &nested.kind else {
            panic!("expected nested group")
        };
        // Source sibling order wins over caller selection order.
        assert_eq!(children, &vec![first, second]);
        session.undo().unwrap();
        let parent = find_element(session.document(), parent_id).unwrap();
        let ElementKind::Group { children } = &parent.kind else {
            panic!("expected parent group")
        };
        assert_eq!(children, &vec![first, second]);

        let middle_id = ElementId::new();
        let mut document = session.document().clone();
        let layer = find_layer_mut(&mut document, target).unwrap();
        layer.scene.roots = vec![first, middle_id, second];
        layer
            .scene
            .elements
            .retain(|element| element.id != parent_id);
        layer.scene.elements.push(element(middle_id, 12.0, 12.0));
        let mut session = EditorSession::from_artifact(NextArtifact::document(document)).unwrap();
        assert!(matches!(
            session.execute(EditCommand::GroupElements {
                group_id: ElementId::new(),
                element_ids: vec![first, second],
                name: String::new(),
            }),
            Err(EditorError::NonContiguousGroupSelection)
        ));
        assert_eq!(roots(&session, target), vec![first, middle_id, second]);
    }

    #[test]
    fn generic_resize_and_rotation_reject_structural_groups() {
        let (mut session, first, second, _, _) = fixture(false);
        let group_id = ElementId::new();
        session
            .execute(EditCommand::GroupElements {
                group_id,
                element_ids: vec![first, second],
                name: String::new(),
            })
            .unwrap();
        let original = bounds(&session, group_id);
        assert!(matches!(
            session.execute(EditCommand::SetBounds {
                element_id: group_id,
                bounds_mm: Rect {
                    x: original.x,
                    y: original.y,
                    width: original.width * 2.0,
                    height: original.height * 2.0,
                },
            }),
            Err(EditorError::GroupTransformRequiresDedicatedCommand(id)) if id == group_id
        ));
        assert!(matches!(
            session.execute(EditCommand::SetRotation {
                element_id: group_id,
                rotation_deg: 45.0,
            }),
            Err(EditorError::GroupTransformRequiresDedicatedCommand(id)) if id == group_id
        ));
        assert_eq!(bounds(&session, group_id), original);
    }

    #[test]
    fn invalid_multi_move_does_not_partially_mutate() {
        let (mut session, first, _, _, _) = fixture(false);
        let before = bounds(&session, first);
        let missing = ElementId::new();
        assert!(matches!(
            session.execute(EditCommand::MoveElements {
                element_ids: vec![first, missing],
                delta_mm: Point { x: 8.0, y: 2.0 },
            }),
            Err(EditorError::ElementNotFound(id)) if id == missing
        ));
        assert_eq!(bounds(&session, first), before);
        assert!(!session.can_undo());
    }

    #[test]
    fn locked_layer_rejects_document_mutation() {
        let (mut session, first, _, _, layer_id) = fixture(true);
        assert!(matches!(
            session.execute(EditCommand::MoveElements {
                element_ids: vec![first],
                delta_mm: Point { x: 1.0, y: 1.0 },
            }),
            Err(EditorError::LayerLocked(id)) if id == layer_id
        ));
    }

    #[test]
    fn master_layer_uses_same_command_path() {
        let (mut session, _, _, master, _) = fixture(false);
        session
            .execute(EditCommand::MoveElements {
                element_ids: vec![master],
                delta_mm: Point { x: -10.0, y: 5.0 },
            })
            .unwrap();
        assert_eq!(bounds(&session, master).x, 90.0);
        assert_eq!(bounds(&session, master).y, 105.0);
    }

    #[test]
    fn page_state_key_tracks_persistent_state_not_transient_state() {
        let (mut session, first, second, _, _) = fixture(false);
        let initial = session.active_page_state_key().unwrap();
        assert_eq!(initial.history_state(), HistoryStateId::INITIAL);
        assert_eq!(initial.page_id(), session.active_page_id().unwrap());

        session.set_selection([first, second]).unwrap();
        session.mark_saved();
        assert_eq!(session.active_page_state_key().unwrap(), initial);

        session
            .execute(EditCommand::MoveElements {
                element_ids: vec![first],
                delta_mm: Point { x: 2.0, y: 3.0 },
            })
            .unwrap();
        let edited = session.active_page_state_key().unwrap();
        assert_ne!(edited, initial);
        assert_eq!(edited.page_id(), initial.page_id());

        session.undo().unwrap();
        assert_eq!(session.active_page_state_key().unwrap(), initial);
        session.redo().unwrap();
        assert_eq!(session.active_page_state_key().unwrap(), edited);
    }

    #[test]
    fn element_style_assignment_is_atomic_reversible_and_validated() {
        let (session, first, second, _, _) = fixture(false);
        let style_id = StyleId::new();
        let mut session = with_styles(session, &[style_id]);

        assert!(
            session
                .execute(EditCommand::SetElementStyle {
                    element_ids: vec![first, second],
                    style_id: Some(style_id),
                })
                .unwrap()
        );
        assert_eq!(style_ref(&session, first), Some(style_id));
        assert_eq!(style_ref(&session, second), Some(style_id));

        session.undo().unwrap();
        assert_eq!(style_ref(&session, first), None);
        assert_eq!(style_ref(&session, second), None);
        session.redo().unwrap();
        assert_eq!(style_ref(&session, first), Some(style_id));
        assert_eq!(style_ref(&session, second), Some(style_id));

        let state = session.current_history_state();
        let missing = StyleId::new();
        assert!(matches!(
            session.execute(EditCommand::SetElementStyle {
                element_ids: vec![first, second],
                style_id: Some(missing),
            }),
            Err(EditorError::StyleNotFound(id)) if id == missing
        ));
        assert_eq!(session.current_history_state(), state);
        assert_eq!(style_ref(&session, first), Some(style_id));
        assert_eq!(style_ref(&session, second), Some(style_id));
    }

    #[test]
    fn text_edit_coalesces_reverses_and_rejects_invalid_layout() {
        let (mut session, first, _, _, _) = fixture(false);
        let first_text = text_block(1.0);
        let final_text = text_block(2.0);
        let transaction = EditTransaction::new([
            EditCommand::SetText {
                element_id: first,
                text: Some(first_text),
            },
            EditCommand::SetText {
                element_id: first,
                text: Some(final_text.clone()),
            },
        ]);
        assert_eq!(transaction.commands().len(), 1);
        assert!(session.execute_transaction(transaction).unwrap());
        assert_eq!(text_value(&session, first), Some(final_text.clone()));

        session.undo().unwrap();
        assert_eq!(text_value(&session, first), None);
        session.redo().unwrap();
        assert_eq!(text_value(&session, first), Some(final_text.clone()));

        let state = session.current_history_state();
        assert!(matches!(
            session.execute(EditCommand::SetText {
                element_id: first,
                text: Some(text_block(-1.0)),
            }),
            Err(EditorError::InvalidTextLayout)
        ));
        assert_eq!(session.current_history_state(), state);
        assert_eq!(text_value(&session, first), Some(final_text));
    }

    #[test]
    fn dirty_state_tracks_history_state_not_command_count() {
        let (mut session, first, _, _, _) = fixture(false);
        assert!(!session.is_dirty());
        session
            .execute(EditCommand::SetRotation {
                element_id: first,
                rotation_deg: 45.0,
            })
            .unwrap();
        assert!(session.is_dirty());
        session.mark_saved();
        assert!(!session.is_dirty());

        session
            .execute(EditCommand::SetRotation {
                element_id: first,
                rotation_deg: 90.0,
            })
            .unwrap();
        assert!(session.is_dirty());
        session.undo().unwrap();
        assert!(!session.is_dirty());
        session.redo().unwrap();
        assert!(session.is_dirty());
    }

    #[test]
    fn no_op_does_not_create_history_state() {
        let (mut session, first, _, _, _) = fixture(false);
        let state = session.current_history_state();
        assert!(
            !session
                .execute(EditCommand::MoveElements {
                    element_ids: vec![first],
                    delta_mm: Point { x: 0.0, y: 0.0 },
                })
                .unwrap()
        );
        assert_eq!(session.current_history_state(), state);
        assert!(!session.can_undo());
    }

    #[test]
    fn selection_is_validated_but_not_part_of_undo_history() {
        let (mut session, first, second, _, _) = fixture(false);
        session.set_selection([first, second]).unwrap();
        assert_eq!(session.selection().len(), 2);
        assert!(!session.can_undo());

        let missing = ElementId::new();
        assert!(matches!(
            session.set_selection([missing]),
            Err(EditorError::ElementNotFound(id)) if id == missing
        ));
        assert_eq!(session.selection().len(), 2);
    }

    #[test]
    fn bounds_and_rotation_are_reversible() {
        let (mut session, first, _, _, _) = fixture(false);
        let original = bounds(&session, first);
        let updated = Rect {
            x: 2.0,
            y: 3.0,
            width: 40.0,
            height: 25.0,
        };
        session
            .execute(EditCommand::SetBounds {
                element_id: first,
                bounds_mm: updated,
            })
            .unwrap();
        session
            .execute(EditCommand::SetRotation {
                element_id: first,
                rotation_deg: 30.0,
            })
            .unwrap();
        assert_eq!(bounds(&session, first), updated);

        session.undo().unwrap();
        session.undo().unwrap();
        assert_eq!(bounds(&session, first), original);
    }

    #[test]
    fn transaction_coalesces_and_undoes_as_one_history_step() {
        let (mut session, first, _, _, _) = fixture(false);
        let original = bounds(&session, first);
        let mut transaction = EditTransaction::default();
        transaction.push(EditCommand::MoveElements {
            element_ids: vec![first],
            delta_mm: Point { x: 1.0, y: 2.0 },
        });
        transaction.push(EditCommand::MoveElements {
            element_ids: vec![first],
            delta_mm: Point { x: 3.0, y: -1.0 },
        });
        transaction.push(EditCommand::SetRotation {
            element_id: first,
            rotation_deg: 10.0,
        });
        transaction.push(EditCommand::SetRotation {
            element_id: first,
            rotation_deg: 30.0,
        });
        assert_eq!(transaction.commands().len(), 2);

        session.execute_transaction(transaction).unwrap();
        assert_eq!(bounds(&session, first).x, original.x + 4.0);
        assert_eq!(bounds(&session, first).y, original.y + 1.0);
        assert_eq!(
            find_element(session.document(), first)
                .unwrap()
                .rotation_deg,
            30.0
        );
        session.undo().unwrap();
        assert_eq!(bounds(&session, first), original);
        assert_eq!(
            find_element(session.document(), first)
                .unwrap()
                .rotation_deg,
            0.0
        );
        assert!(!session.can_undo());
        session.redo().unwrap();
        assert_eq!(bounds(&session, first).x, original.x + 4.0);
        assert_eq!(
            find_element(session.document(), first)
                .unwrap()
                .rotation_deg,
            30.0
        );
    }

    #[test]
    fn failed_transaction_rolls_back_all_prior_commands() {
        let (mut session, first, _, _, _) = fixture(false);
        let original = bounds(&session, first);
        let missing = ElementId::new();
        let transaction = EditTransaction::new([
            EditCommand::MoveElements {
                element_ids: vec![first],
                delta_mm: Point { x: 50.0, y: 50.0 },
            },
            EditCommand::SetRotation {
                element_id: missing,
                rotation_deg: 15.0,
            },
        ]);
        assert!(matches!(
            session.execute_transaction(transaction),
            Err(EditorError::ElementNotFound(id)) if id == missing
        ));
        assert_eq!(bounds(&session, first), original);
        assert!(!session.can_undo());
    }

    #[test]
    fn create_element_preserves_requested_z_order_across_undo_redo() {
        let (mut session, first, second, _, layer_id) = fixture(false);
        let page_id = session.active_page_id().unwrap();
        let target = LayerTarget::Page { page_id, layer_id };
        let created = ElementId::new();
        session
            .execute(EditCommand::CreateElement {
                target,
                element: element(created, 8.0, 9.0),
                z_index: Some(1),
            })
            .unwrap();
        assert_eq!(roots(&session, target), vec![first, created, second]);
        assert!(find_element(session.document(), created).is_some());

        session.undo().unwrap();
        assert_eq!(roots(&session, target), vec![first, second]);
        assert!(find_element(session.document(), created).is_none());

        session.redo().unwrap();
        assert_eq!(roots(&session, target), vec![first, created, second]);
    }

    #[test]
    fn duplicate_create_is_rejected_without_history() {
        let (mut session, first, _, _, layer_id) = fixture(false);
        let target = LayerTarget::Page {
            page_id: session.active_page_id().unwrap(),
            layer_id,
        };
        assert!(matches!(
            session.execute(EditCommand::CreateElement {
                target,
                element: element(first, 0.0, 0.0),
                z_index: None,
            }),
            Err(EditorError::ElementAlreadyExists(id)) if id == first
        ));
        assert!(!session.can_undo());
    }

    fn relationship_fixture() -> (EditorSession, LayerTarget, ElementId, ElementId, PortId) {
        let page_id = PageId::new();
        let layer_id = LayerId::new();
        let target_id = ElementId::new();
        let connector_id = ElementId::new();
        let port_id = PortId::new();
        let mut target = element(target_id, 10.0, 10.0);
        target.ports.push(Port {
            id: port_id,
            index: 0,
            position: NormalizedPoint { x: 0.5, y: 0.5 },
        });
        let connector = Element {
            id: connector_id,
            name: "Connector".to_owned(),
            bounds_mm: Rect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            },
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports: Vec::new(),
            style_id: None,
            text: None,
            kind: ElementKind::StraightConnector {
                connector: next_domain::Connector {
                    start: Endpoint {
                        position_mm: Point { x: 15.0, y: 15.0 },
                        connection: Some(Connection {
                            element_id: target_id,
                            port_id,
                        }),
                    },
                    end: Endpoint {
                        position_mm: Point { x: 30.0, y: 30.0 },
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
        let document = Document {
            id: DocumentId::new(),
            name: "Relationships".to_owned(),
            defaults: defaults(),
            master_layers: Vec::new(),
            pages: vec![Page {
                id: page_id,
                name: "Page".to_owned(),
                size_mm: next_domain::Size {
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
                        roots: vec![target_id, connector_id],
                        elements: vec![target, connector],
                    },
                }],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        };
        (
            EditorSession::from_artifact(NextArtifact::document(document)).unwrap(),
            LayerTarget::Page { page_id, layer_id },
            target_id,
            connector_id,
            port_id,
        )
    }

    fn start_connection(session: &EditorSession, connector_id: ElementId) -> Option<Connection> {
        let connector = find_element(session.document(), connector_id).unwrap();
        match &connector.kind {
            ElementKind::StraightConnector { connector } => connector.start.connection,
            _ => panic!("expected straight connector"),
        }
    }

    #[test]
    fn delete_detaches_remaining_connector_and_undo_restores_connection() {
        let (mut session, target, target_id, connector_id, port_id) = relationship_fixture();
        session.set_selection([target_id]).unwrap();
        session
            .execute(EditCommand::DeleteElements {
                element_ids: vec![target_id],
            })
            .unwrap();
        assert!(find_element(session.document(), target_id).is_none());
        assert_eq!(start_connection(&session, connector_id), None);
        assert!(session.selection().is_empty());
        assert_eq!(roots(&session, target), vec![connector_id]);

        session.undo().unwrap();
        assert!(find_element(session.document(), target_id).is_some());
        assert_eq!(
            start_connection(&session, connector_id),
            Some(Connection {
                element_id: target_id,
                port_id,
            })
        );
        assert_eq!(roots(&session, target), vec![target_id, connector_id]);
    }

    #[test]
    fn ungroup_promotes_children_detaches_group_connections_and_undo_restores() {
        let (session, target, group_id, connector_id, port_id) = relationship_fixture();
        let child_id = ElementId::new();
        let mut document = session.document().clone();
        let layer = find_layer_mut(&mut document, target).unwrap();
        let group = layer
            .scene
            .elements
            .iter_mut()
            .find(|element| element.id == group_id)
            .unwrap();
        group.kind = ElementKind::Group {
            children: vec![child_id],
        };
        layer.scene.elements.push(element(child_id, 4.0, 4.0));
        let mut session = EditorSession::from_artifact(NextArtifact::document(document)).unwrap();

        session.execute(EditCommand::Ungroup { group_id }).unwrap();
        assert!(find_element(session.document(), group_id).is_none());
        assert!(find_element(session.document(), child_id).is_some());
        assert_eq!(roots(&session, target), vec![child_id, connector_id]);
        assert_eq!(start_connection(&session, connector_id), None);

        session.undo().unwrap();
        assert!(find_element(session.document(), group_id).is_some());
        assert_eq!(roots(&session, target), vec![group_id, connector_id]);
        assert_eq!(
            start_connection(&session, connector_id),
            Some(Connection {
                element_id: group_id,
                port_id,
            })
        );

        session.redo().unwrap();
        assert!(find_element(session.document(), group_id).is_none());
        assert_eq!(roots(&session, target), vec![child_id, connector_id]);
        assert_eq!(start_connection(&session, connector_id), None);
    }

    #[test]
    fn deleting_group_cascades_children_and_restores_storage_and_z_order() {
        let page_id = PageId::new();
        let layer_id = LayerId::new();
        let child_id = ElementId::new();
        let group_id = ElementId::new();
        let other_id = ElementId::new();
        let child = element(child_id, 1.0, 1.0);
        let mut group = element(group_id, 0.0, 0.0);
        group.kind = ElementKind::Group {
            children: vec![child_id],
        };
        let other = element(other_id, 20.0, 20.0);
        let document = Document {
            id: DocumentId::new(),
            name: "Group delete".to_owned(),
            defaults: defaults(),
            master_layers: Vec::new(),
            pages: vec![Page {
                id: page_id,
                name: "Page".to_owned(),
                size_mm: next_domain::Size::default(),
                layers: vec![Layer {
                    id: layer_id,
                    name: "Layer".to_owned(),
                    visible: true,
                    locked: false,
                    draw_color: None,
                    scene: Scene {
                        roots: vec![group_id, other_id],
                        elements: vec![child, group, other],
                    },
                }],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        };
        let mut session = EditorSession::from_artifact(NextArtifact::document(document)).unwrap();
        let target = LayerTarget::Page { page_id, layer_id };

        session
            .execute(EditCommand::DeleteElements {
                element_ids: vec![group_id],
            })
            .unwrap();
        assert!(find_element(session.document(), group_id).is_none());
        assert!(find_element(session.document(), child_id).is_none());
        assert_eq!(roots(&session, target), vec![other_id]);

        session.undo().unwrap();
        assert_eq!(roots(&session, target), vec![group_id, other_id]);
        let ids: Vec<_> = find_layer(session.document(), target)
            .unwrap()
            .scene
            .elements
            .iter()
            .map(|element| element.id)
            .collect();
        assert_eq!(ids, vec![child_id, group_id, other_id]);
    }

    #[test]
    fn directly_deleting_group_child_is_rejected_atomically() {
        let page_id = PageId::new();
        let layer_id = LayerId::new();
        let child_id = ElementId::new();
        let group_id = ElementId::new();
        let child = element(child_id, 1.0, 1.0);
        let mut group = element(group_id, 0.0, 0.0);
        group.kind = ElementKind::Group {
            children: vec![child_id],
        };
        let document = Document {
            id: DocumentId::new(),
            name: "Grouped child".to_owned(),
            defaults: defaults(),
            master_layers: Vec::new(),
            pages: vec![Page {
                id: page_id,
                name: "Page".to_owned(),
                size_mm: next_domain::Size::default(),
                layers: vec![Layer {
                    id: layer_id,
                    name: "Layer".to_owned(),
                    visible: true,
                    locked: false,
                    draw_color: None,
                    scene: Scene {
                        roots: vec![group_id],
                        elements: vec![child, group],
                    },
                }],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        };
        let mut session = EditorSession::from_artifact(NextArtifact::document(document)).unwrap();
        assert!(matches!(
            session.execute(EditCommand::DeleteElements {
                element_ids: vec![child_id],
            }),
            Err(EditorError::ElementReferencedByGroup {
                element_id,
                group_id: parent,
            }) if element_id == child_id && parent == group_id
        ));
        assert!(find_element(session.document(), child_id).is_some());
        assert!(find_element(session.document(), group_id).is_some());
        assert!(!session.can_undo());
    }

    #[test]
    fn invalid_structural_create_rolls_back() {
        let (mut session, _, _, _, layer_id) = fixture(false);
        let target = LayerTarget::Page {
            page_id: session.active_page_id().unwrap(),
            layer_id,
        };
        let image_id = ElementId::new();
        let mut image = element(image_id, 0.0, 0.0);
        image.kind = ElementKind::Image {
            asset_id: AssetId::new(),
        };
        assert!(matches!(
            session.execute(EditCommand::CreateElement {
                target,
                element: image,
                z_index: None,
            }),
            Err(EditorError::InvalidDocument(_))
        ));
        assert!(find_element(session.document(), image_id).is_none());
        assert!(!session.can_undo());
    }

    #[test]
    fn page_delete_repairs_active_state_and_undo_redo_restores_navigation() {
        let (mut session, _, _, _, _) = fixture(false);
        let first_page = session.active_page_id().unwrap();
        let first_layer = session.active_layer().unwrap();
        let second_page = PageId::new();
        let second_layer = LayerId::new();
        session
            .execute(EditCommand::CreatePage {
                page: Page {
                    id: second_page,
                    name: "Second".to_owned(),
                    size_mm: Size {
                        width: 297.0,
                        height: 210.0,
                    },
                    layers: vec![Layer {
                        id: second_layer,
                        name: "Second layer".to_owned(),
                        visible: true,
                        locked: false,
                        draw_color: None,
                        scene: Scene::default(),
                    }],
                },
                index: None,
            })
            .unwrap();
        assert_eq!(session.active_page_id(), Some(first_page));
        assert_eq!(session.active_layer(), Some(first_layer));

        session
            .execute(EditCommand::DeletePage {
                page_id: first_page,
            })
            .unwrap();
        assert_eq!(session.active_page_id(), Some(second_page));
        assert_eq!(
            session.active_layer(),
            Some(LayerTarget::Page {
                page_id: second_page,
                layer_id: second_layer,
            })
        );
        assert_eq!(session.document().pages.len(), 1);

        session.undo().unwrap();
        assert_eq!(session.active_page_id(), Some(first_page));
        assert_eq!(session.active_layer(), Some(first_layer));
        assert_eq!(session.document().pages[0].id, first_page);
        assert_eq!(session.document().pages[1].id, second_page);

        session.redo().unwrap();
        assert_eq!(session.active_page_id(), Some(second_page));
        assert_eq!(
            session.active_layer(),
            Some(LayerTarget::Page {
                page_id: second_page,
                layer_id: second_layer,
            })
        );
    }

    #[test]
    fn non_topology_undo_preserves_current_navigation() {
        let (mut session, first, _, _, _) = fixture(false);
        let first_page = session.active_page_id().unwrap();
        let second_page = PageId::new();
        let second_layer = LayerId::new();
        session
            .execute(EditCommand::CreatePage {
                page: Page {
                    id: second_page,
                    name: "Second".to_owned(),
                    size_mm: Size {
                        width: 210.0,
                        height: 297.0,
                    },
                    layers: vec![Layer {
                        id: second_layer,
                        name: "Second layer".to_owned(),
                        visible: true,
                        locked: false,
                        draw_color: None,
                        scene: Scene::default(),
                    }],
                },
                index: None,
            })
            .unwrap();
        session.set_active_page(first_page).unwrap();
        session
            .execute(EditCommand::MoveElements {
                element_ids: vec![first],
                delta_mm: Point { x: 3.0, y: 0.0 },
            })
            .unwrap();
        session.set_active_page(second_page).unwrap();

        session.undo().unwrap();
        assert_eq!(session.active_page_id(), Some(second_page));
        assert_eq!(
            session.active_layer(),
            Some(LayerTarget::Page {
                page_id: second_page,
                layer_id: second_layer,
            })
        );
        assert_eq!(bounds(&session, first).x, 1.0);
    }

    #[test]
    fn layer_delete_repairs_selection_and_active_layer_across_history() {
        let (mut session, first, _, _, first_layer_id) = fixture(false);
        let page_id = session.active_page_id().unwrap();
        let second_layer_id = LayerId::new();
        let first_target = LayerTarget::Page {
            page_id,
            layer_id: first_layer_id,
        };
        let second_target = LayerTarget::Page {
            page_id,
            layer_id: second_layer_id,
        };

        session
            .execute(EditCommand::CreateLayer {
                scope: LayerScope::Page { page_id },
                layer: Layer {
                    id: second_layer_id,
                    name: "Second".to_owned(),
                    visible: true,
                    locked: false,
                    draw_color: None,
                    scene: Scene::default(),
                },
                index: None,
            })
            .unwrap();
        session.set_active_layer(first_target).unwrap();
        session.set_selection([first]).unwrap();

        session
            .execute(EditCommand::DeleteLayer {
                target: first_target,
            })
            .unwrap();
        assert!(session.selection().is_empty());
        assert_eq!(session.active_page_id(), Some(page_id));
        assert_eq!(session.active_layer(), Some(second_target));
        assert!(find_layer(session.document(), first_target).is_none());

        session.undo().unwrap();
        assert_eq!(session.active_page_id(), Some(page_id));
        assert_eq!(session.active_layer(), Some(first_target));
        assert!(find_layer(session.document(), first_target).is_some());

        session.redo().unwrap();
        assert_eq!(session.active_page_id(), Some(page_id));
        assert_eq!(session.active_layer(), Some(second_target));
        assert!(find_layer(session.document(), first_target).is_none());
    }

    #[test]
    fn page_and_layer_properties_are_coalesced_validated_and_reversible() {
        let (mut session, _, _, _, layer_id) = fixture(false);
        let page_id = session.active_page_id().unwrap();
        let target = LayerTarget::Page { page_id, layer_id };
        let original_page = session.document().pages[0].clone();
        let original_layer = find_layer(session.document(), target).unwrap().clone();

        let mut transaction = EditTransaction::default();
        transaction.push(EditCommand::SetPageProperties {
            page_id,
            name: "Draft".to_owned(),
            size_mm: Size {
                width: 200.0,
                height: 200.0,
            },
        });
        transaction.push(EditCommand::SetPageProperties {
            page_id,
            name: "Final".to_owned(),
            size_mm: Size {
                width: 420.0,
                height: 297.0,
            },
        });
        transaction.push(EditCommand::SetLayerProperties {
            target,
            name: "Hidden locked".to_owned(),
            visible: false,
            locked: true,
            draw_color: Some(Color::SystemPalette { index: 3 }),
        });
        session.execute_transaction(transaction).unwrap();

        assert_eq!(session.document().pages[0].name, "Final");
        assert_eq!(session.document().pages[0].size_mm.width, 420.0);
        let layer = find_layer(session.document(), target).unwrap();
        assert_eq!(layer.name, "Hidden locked");
        assert!(!layer.visible);
        assert!(layer.locked);
        assert_eq!(layer.draw_color, Some(Color::SystemPalette { index: 3 }));

        session.undo().unwrap();
        assert_eq!(session.document().pages[0].name, original_page.name);
        assert_eq!(session.document().pages[0].size_mm, original_page.size_mm);
        assert_eq!(
            find_layer(session.document(), target).unwrap(),
            &original_layer
        );

        let state = session.current_history_state();
        let error = session
            .execute(EditCommand::SetPageProperties {
                page_id,
                name: "Invalid".to_owned(),
                size_mm: Size {
                    width: f64::NAN,
                    height: 297.0,
                },
            })
            .unwrap_err();
        assert!(matches!(error, EditorError::InvalidPageSize));
        assert_eq!(session.current_history_state(), state);
    }

    #[test]
    fn page_and_layer_creation_reject_duplicate_ids_without_history() {
        let (mut session, _, _, _, layer_id) = fixture(false);
        let page_id = session.active_page_id().unwrap();
        let initial_state = session.current_history_state();
        let duplicate_page = session.document().pages[0].clone();
        let error = session
            .execute(EditCommand::CreatePage {
                page: duplicate_page,
                index: None,
            })
            .unwrap_err();
        assert!(matches!(error, EditorError::PageAlreadyExists(id) if id == page_id));
        assert_eq!(session.current_history_state(), initial_state);

        let error = session
            .execute(EditCommand::CreateLayer {
                scope: LayerScope::Page { page_id },
                layer: Layer {
                    id: layer_id,
                    name: "Duplicate".to_owned(),
                    visible: true,
                    locked: false,
                    draw_color: None,
                    scene: Scene::default(),
                },
                index: None,
            })
            .unwrap_err();
        assert!(matches!(error, EditorError::LayerAlreadyExists(id) if id == layer_id));
        assert_eq!(session.current_history_state(), initial_state);
    }
}
