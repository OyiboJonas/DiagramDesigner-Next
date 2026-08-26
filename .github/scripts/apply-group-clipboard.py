from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file_path = Path(path)
    text = file_path.read_text()
    if old not in text:
        raise SystemExit(f"expected patch anchor missing in {path}: {old[:120]!r}")
    file_path.write_text(text.replace(old, new, 1))


clipboard_rs = r'''use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use next_domain::{Connection, Document, Element, ElementId, ElementKind, Endpoint, Point, PortId};

pub const PASTE_OFFSET_MM: f64 = 5.0;

#[derive(Debug, Clone)]
struct ClipboardGroupPayload {
    id: ElementId,
    name: String,
    children: Vec<ElementId>,
}

#[derive(Debug, Clone)]
pub struct ClipboardPayload {
    elements: Vec<Element>,
    groups: Vec<ClipboardGroupPayload>,
    root_ids: Vec<ElementId>,
    source_ids: Vec<ElementId>,
}

#[derive(Debug)]
pub struct ClipboardGroupInstantiation {
    pub group_id: ElementId,
    pub child_ids: Vec<ElementId>,
    pub name: String,
}

#[derive(Debug)]
pub struct ClipboardInstantiation {
    pub elements: Vec<Element>,
    pub groups: Vec<ClipboardGroupInstantiation>,
    pub element_ids: Vec<ElementId>,
    pub source_element_ids: BTreeMap<ElementId, ElementId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardError {
    EmptySelection,
    MissingElement(ElementId),
    GroupRequiresTopLevel(ElementId),
    GroupCycle(ElementId),
    GroupRequiresAtLeastTwoChildren(ElementId),
    OverlappingSelection(ElementId),
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySelection => formatter.write_str("Select at least one element to copy."),
            Self::MissingElement(element_id) => {
                write!(
                    formatter,
                    "Selected or grouped element {element_id:?} no longer exists."
                )
            }
            Self::GroupRequiresTopLevel(element_id) => write!(
                formatter,
                "Structural group {element_id:?} must be a top-level scene root before it can be copied."
            ),
            Self::GroupCycle(element_id) => write!(
                formatter,
                "Structural group hierarchy contains a cycle at {element_id:?}."
            ),
            Self::GroupRequiresAtLeastTwoChildren(element_id) => write!(
                formatter,
                "Structural group {element_id:?} cannot be reconstructed because it has fewer than two children."
            ),
            Self::OverlappingSelection(element_id) => write!(
                formatter,
                "Clipboard selection overlaps an already captured group subtree at {element_id:?}."
            ),
        }
    }
}

impl std::error::Error for ClipboardError {}

impl ClipboardPayload {
    pub fn len(&self) -> usize {
        self.root_ids.len()
    }

    pub fn source_element_ids(&self) -> &[ElementId] {
        &self.source_ids
    }

    pub fn instantiate(&self, cascade_step: u32) -> ClipboardInstantiation {
        let step = cascade_step.max(1) as f64;
        let delta = Point {
            x: PASTE_OFFSET_MM * step,
            y: PASTE_OFFSET_MM * step,
        };

        let mut element_ids = BTreeMap::new();
        let mut port_ids = BTreeMap::new();
        for element in &self.elements {
            element_ids.insert(element.id, ElementId::new());
            for port in &element.ports {
                port_ids.insert(port.id, PortId::new());
            }
        }
        for group in &self.groups {
            element_ids.insert(group.id, ElementId::new());
        }

        let mut instantiated = Vec::with_capacity(self.elements.len());
        for source in &self.elements {
            let mut element = source.clone();
            element.id = element_ids[&source.id];
            element.import = None;
            element.bounds_mm.x += delta.x;
            element.bounds_mm.y += delta.y;

            for port in &mut element.ports {
                port.id = port_ids[&port.id];
            }

            match &mut element.kind {
                ElementKind::StraightConnector { connector }
                | ElementKind::OrthogonalConnector { connector, .. } => {
                    remap_connector(connector, delta, &element_ids, &port_ids);
                }
                ElementKind::Curve {
                    connector,
                    control_points_mm,
                    ..
                } => {
                    for point in control_points_mm {
                        point.x += delta.x;
                        point.y += delta.y;
                    }
                    if let Some(connector) = connector {
                        remap_connector(connector, delta, &element_ids, &port_ids);
                    }
                }
                ElementKind::Group { .. } => {
                    unreachable!("clipboard leaf storage never contains structural groups")
                }
                _ => {}
            }
            instantiated.push(element);
        }

        let groups = self
            .groups
            .iter()
            .map(|group| ClipboardGroupInstantiation {
                group_id: element_ids[&group.id],
                child_ids: group
                    .children
                    .iter()
                    .map(|child_id| element_ids[child_id])
                    .collect(),
                name: group.name.clone(),
            })
            .collect();
        let selected = self
            .root_ids
            .iter()
            .map(|root_id| element_ids[root_id])
            .collect();

        ClipboardInstantiation {
            elements: instantiated,
            groups,
            element_ids: selected,
            source_element_ids: element_ids,
        }
    }
}

pub fn capture_selection(
    document: &Document,
    selected: &[ElementId],
) -> Result<ClipboardPayload, ClipboardError> {
    if selected.is_empty() {
        return Err(ClipboardError::EmptySelection);
    }

    let selected_set: BTreeSet<_> = selected.iter().copied().collect();
    let mut by_id = BTreeMap::new();
    let mut scene_roots = BTreeSet::new();
    for layer in &document.master_layers {
        scene_roots.extend(layer.scene.roots.iter().copied());
        for element in &layer.scene.elements {
            by_id.insert(element.id, element);
        }
    }
    for page in &document.pages {
        for layer in &page.layers {
            scene_roots.extend(layer.scene.roots.iter().copied());
            for element in &layer.scene.elements {
                by_id.insert(element.id, element);
            }
        }
    }

    for element_id in &selected_set {
        let element = by_id
            .get(element_id)
            .copied()
            .ok_or(ClipboardError::MissingElement(*element_id))?;
        if matches!(&element.kind, ElementKind::Group { .. }) && !scene_roots.contains(element_id) {
            return Err(ClipboardError::GroupRequiresTopLevel(*element_id));
        }
    }

    // Preserve visible scene z-order wherever possible instead of UUID order.
    let mut ordered_ids = Vec::with_capacity(selected_set.len());
    for layer in &document.master_layers {
        for element_id in &layer.scene.roots {
            if selected_set.contains(element_id) {
                ordered_ids.push(*element_id);
            }
        }
    }
    for page in &document.pages {
        for layer in &page.layers {
            for element_id in &layer.scene.roots {
                if selected_set.contains(element_id) {
                    ordered_ids.push(*element_id);
                }
            }
        }
    }
    for element_id in &selected_set {
        if !ordered_ids.contains(element_id) {
            ordered_ids.push(*element_id);
        }
    }

    let mut captured = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    let mut elements = Vec::new();
    let mut groups = Vec::new();
    let mut source_ids = Vec::new();
    for element_id in &ordered_ids {
        collect_subtree(
            *element_id,
            &by_id,
            &mut captured,
            &mut visiting,
            &mut elements,
            &mut groups,
            &mut source_ids,
        )?;
    }

    Ok(ClipboardPayload {
        elements,
        groups,
        root_ids: ordered_ids,
        source_ids,
    })
}

fn collect_subtree(
    element_id: ElementId,
    by_id: &BTreeMap<ElementId, &Element>,
    captured: &mut BTreeSet<ElementId>,
    visiting: &mut BTreeSet<ElementId>,
    elements: &mut Vec<Element>,
    groups: &mut Vec<ClipboardGroupPayload>,
    source_ids: &mut Vec<ElementId>,
) -> Result<(), ClipboardError> {
    if captured.contains(&element_id) {
        return Err(ClipboardError::OverlappingSelection(element_id));
    }
    if !visiting.insert(element_id) {
        return Err(ClipboardError::GroupCycle(element_id));
    }

    let element = by_id
        .get(&element_id)
        .copied()
        .ok_or(ClipboardError::MissingElement(element_id))?;
    source_ids.push(element_id);
    match &element.kind {
        ElementKind::Group { children } => {
            if children.len() < 2 {
                return Err(ClipboardError::GroupRequiresAtLeastTwoChildren(element_id));
            }
            for child_id in children {
                collect_subtree(
                    *child_id,
                    by_id,
                    captured,
                    visiting,
                    elements,
                    groups,
                    source_ids,
                )?;
            }
            groups.push(ClipboardGroupPayload {
                id: element.id,
                name: element.name.clone(),
                children: children.clone(),
            });
        }
        _ => elements.push(element.clone()),
    }

    visiting.remove(&element_id);
    captured.insert(element_id);
    Ok(())
}

fn remap_connector(
    connector: &mut next_domain::Connector,
    delta: Point,
    element_ids: &BTreeMap<ElementId, ElementId>,
    port_ids: &BTreeMap<PortId, PortId>,
) {
    remap_endpoint(&mut connector.start, delta, element_ids, port_ids);
    remap_endpoint(&mut connector.end, delta, element_ids, port_ids);
}

fn remap_endpoint(
    endpoint: &mut Endpoint,
    delta: Point,
    element_ids: &BTreeMap<ElementId, ElementId>,
    port_ids: &BTreeMap<PortId, PortId>,
) {
    endpoint.position_mm.x += delta.x;
    endpoint.position_mm.y += delta.y;
    endpoint.connection = endpoint.connection.and_then(|connection| {
        let element_id = *element_ids.get(&connection.element_id)?;
        let port_id = *port_ids.get(&connection.port_id)?;
        Some(Connection {
            element_id,
            port_id,
        })
    });
}

#[cfg(test)]
mod tests {
    use next_domain::{
        AnchorSet, Connector, ConnectorLabelStyle, DocumentDefaults, DocumentId, Layer, LayerId,
        LineStyle, MarkerStyle, NormalizedPoint, Page, PageId, Port, Rect, Scene, Size,
    };

    use super::*;

    fn shape(element_id: ElementId, port_id: PortId, x: f64) -> Element {
        Element {
            id: element_id,
            name: "Shape".to_owned(),
            bounds_mm: Rect {
                x,
                y: 20.0,
                width: 30.0,
                height: 15.0,
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
        }
    }

    fn connector(
        element_id: ElementId,
        target_id: ElementId,
        target_port: PortId,
        external_id: ElementId,
        external_port: PortId,
    ) -> Element {
        Element {
            id: element_id,
            name: "Connector".to_owned(),
            bounds_mm: Rect {
                x: 40.0,
                y: 20.0,
                width: 30.0,
                height: 20.0,
            },
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports: Vec::new(),
            style_id: None,
            text: None,
            kind: ElementKind::StraightConnector {
                connector: Connector {
                    start: Endpoint {
                        position_mm: Point { x: 40.0, y: 25.0 },
                        connection: Some(Connection {
                            element_id: target_id,
                            port_id: target_port,
                        }),
                    },
                    end: Endpoint {
                        position_mm: Point { x: 70.0, y: 40.0 },
                        connection: Some(Connection {
                            element_id: external_id,
                            port_id: external_port,
                        }),
                    },
                    start_marker: MarkerStyle::None,
                    end_marker: MarkerStyle::None,
                    line_style: LineStyle::Solid,
                    secondary_color: None,
                },
            },
            import: None,
        }
    }

    fn group(id: ElementId, name: &str, children: Vec<ElementId>) -> Element {
        Element {
            id,
            name: name.to_owned(),
            bounds_mm: Rect {
                x: 10.0,
                y: 20.0,
                width: 90.0,
                height: 30.0,
            },
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports: Vec::new(),
            style_id: None,
            text: None,
            kind: ElementKind::Group { children },
            import: None,
        }
    }

    fn document_with_elements(roots: Vec<ElementId>, elements: Vec<Element>) -> Document {
        Document {
            id: DocumentId::new(),
            name: "Clipboard fixture".to_owned(),
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
                id: PageId::new(),
                name: "Page".to_owned(),
                size_mm: Size {
                    width: 210.0,
                    height: 297.0,
                },
                layers: vec![Layer {
                    id: LayerId::new(),
                    name: "Layer".to_owned(),
                    visible: true,
                    locked: false,
                    draw_color: None,
                    scene: Scene { roots, elements },
                }],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        }
    }

    #[test]
    fn instantiate_remaps_internal_connections_and_detaches_external_targets() {
        let shape_id = ElementId::new();
        let shape_port = PortId::new();
        let connector_id = ElementId::new();
        let external_id = ElementId::new();
        let external_port = PortId::new();
        let payload = ClipboardPayload {
            elements: vec![
                shape(shape_id, shape_port, 10.0),
                connector(
                    connector_id,
                    shape_id,
                    shape_port,
                    external_id,
                    external_port,
                ),
            ],
            groups: Vec::new(),
            root_ids: vec![shape_id, connector_id],
            source_ids: vec![shape_id, connector_id],
        };

        let first = payload.instantiate(1);
        assert_eq!(first.elements.len(), 2);
        assert_eq!(first.groups.len(), 0);
        let copied_shape = &first.elements[0];
        let copied_connector = &first.elements[1];
        assert_ne!(copied_shape.id, shape_id);
        assert_ne!(copied_shape.ports[0].id, shape_port);
        assert_eq!(copied_shape.bounds_mm.x, 15.0);
        assert_eq!(copied_shape.bounds_mm.y, 25.0);

        let ElementKind::StraightConnector { connector } = &copied_connector.kind else {
            panic!("expected copied straight connector");
        };
        assert_eq!(connector.start.position_mm, Point { x: 45.0, y: 30.0 });
        assert_eq!(connector.end.position_mm, Point { x: 75.0, y: 45.0 });
        assert_eq!(
            connector.start.connection,
            Some(Connection {
                element_id: copied_shape.id,
                port_id: copied_shape.ports[0].id,
            })
        );
        assert_eq!(connector.end.connection, None);

        let second = payload.instantiate(2);
        assert_eq!(second.elements[0].bounds_mm.x, 20.0);
        assert_ne!(second.elements[0].id, copied_shape.id);
        assert_ne!(second.elements[0].ports[0].id, copied_shape.ports[0].id);
    }

    #[test]
    fn nested_group_capture_builds_post_order_group_plan_and_fresh_root_selection() {
        let first_id = ElementId::new();
        let first_port = PortId::new();
        let second_id = ElementId::new();
        let second_port = PortId::new();
        let connector_id = ElementId::new();
        let inner_group = ElementId::new();
        let outer_group = ElementId::new();
        let external_id = ElementId::new();
        let external_port = PortId::new();
        let document = document_with_elements(
            vec![outer_group, external_id],
            vec![
                shape(first_id, first_port, 10.0),
                connector(
                    connector_id,
                    first_id,
                    first_port,
                    external_id,
                    external_port,
                ),
                group(inner_group, "Inner", vec![first_id, connector_id]),
                shape(second_id, second_port, 80.0),
                group(outer_group, "Outer", vec![inner_group, second_id]),
                shape(external_id, external_port, 150.0),
            ],
        );

        let payload = capture_selection(&document, &[outer_group]).unwrap();
        assert_eq!(payload.len(), 1);
        assert_eq!(payload.elements.len(), 3);
        assert_eq!(payload.groups.len(), 2);
        assert_eq!(payload.groups[0].id, inner_group);
        assert_eq!(payload.groups[1].id, outer_group);
        assert_eq!(
            payload.source_element_ids(),
            &[outer_group, inner_group, first_id, connector_id, second_id]
        );

        let instantiated = payload.instantiate(1);
        let copied_first = instantiated.source_element_ids[&first_id];
        let copied_connector_id = instantiated.source_element_ids[&connector_id];
        let copied_inner = instantiated.source_element_ids[&inner_group];
        let copied_second = instantiated.source_element_ids[&second_id];
        let copied_outer = instantiated.source_element_ids[&outer_group];
        assert_eq!(instantiated.element_ids, vec![copied_outer]);
        assert_eq!(instantiated.groups[0].group_id, copied_inner);
        assert_eq!(
            instantiated.groups[0].child_ids,
            vec![copied_first, copied_connector_id]
        );
        assert_eq!(instantiated.groups[0].name, "Inner");
        assert_eq!(instantiated.groups[1].group_id, copied_outer);
        assert_eq!(
            instantiated.groups[1].child_ids,
            vec![copied_inner, copied_second]
        );
        assert_eq!(instantiated.groups[1].name, "Outer");

        let copied_connector = instantiated
            .elements
            .iter()
            .find(|element| element.id == copied_connector_id)
            .unwrap();
        let copied_shape = instantiated
            .elements
            .iter()
            .find(|element| element.id == copied_first)
            .unwrap();
        let ElementKind::StraightConnector { connector } = &copied_connector.kind else {
            panic!("expected copied connector")
        };
        assert_eq!(
            connector.start.connection,
            Some(Connection {
                element_id: copied_first,
                port_id: copied_shape.ports[0].id,
            })
        );
        assert_eq!(connector.end.connection, None);
    }

    #[test]
    fn nested_group_selection_is_rejected_until_its_top_level_owner_is_selected() {
        let first = ElementId::new();
        let second = ElementId::new();
        let inner = ElementId::new();
        let outer = ElementId::new();
        let document = document_with_elements(
            vec![outer],
            vec![
                shape(first, PortId::new(), 10.0),
                shape(second, PortId::new(), 50.0),
                group(inner, "Inner", vec![first, second]),
                group(outer, "Outer", vec![inner, ElementId::new()]),
            ],
        );

        assert!(matches!(
            capture_selection(&document, &[inner]),
            Err(ClipboardError::GroupRequiresTopLevel(id)) if id == inner
        ));
    }
}
'''
Path("apps/desktop/src-tauri/src/clipboard.rs").write_text(clipboard_rs)

replace_once(
    "crates/app-core/src/lib.rs",
    '''#[derive(Debug, Clone, PartialEq)]
pub struct ElementAppearanceUpdate {
    pub element_id: ElementId,
    pub stroke: Option<StrokeStyle>,
    pub fill: Option<FillStyle>,
    pub text_color: Option<Color>,
}
''',
    '''#[derive(Debug, Clone, PartialEq)]
pub struct ElementAppearanceUpdate {
    pub element_id: ElementId,
    pub stroke: Option<StrokeStyle>,
    pub fill: Option<FillStyle>,
    pub text_color: Option<Color>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralGroupCreation {
    pub group_id: ElementId,
    pub element_ids: Vec<ElementId>,
    pub name: String,
}
''',
)

replace_once(
    "crates/app-core/src/lib.rs",
    '''    /// Create several top-level elements as one semantic history step.
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
''',
    '''    /// Create several top-level elements as one semantic history step.
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
        self.create_elements_with_groups(target, elements, Vec::new(), appearance_updates)
    }

    /// Create clipboard leaves, rebuild structural groups from inner to outer and
    /// materialize dedicated appearance snapshots as one semantic history step.
    pub fn create_elements_with_groups(
        &mut self,
        target: LayerTarget,
        elements: Vec<Element>,
        groups: Vec<StructuralGroupCreation>,
        appearance_updates: Vec<ElementAppearanceUpdate>,
    ) -> Result<bool, ApplicationError> {
        let mut transaction = EditTransaction::new(elements.into_iter().map(|element| {
            EditCommand::CreateElement {
                target,
                element,
                z_index: None,
            }
        }));
        for group in groups {
            transaction.push(EditCommand::GroupElements {
                group_id: group.group_id,
                element_ids: group.element_ids,
                name: group.name,
            });
        }
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
''',
)

replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    '''    ConnectorEndpointState as AppConnectorEndpointState,
    ConnectorEndpoints as AppConnectorEndpoints, ConnectorGeometryKind as AppConnectorGeometryKind,
    ElementAppearanceUpdate, ZOrderOperation as AppZOrderOperation,
''',
    '''    ConnectorEndpointState as AppConnectorEndpointState,
    ConnectorEndpoints as AppConnectorEndpoints, ConnectorGeometryKind as AppConnectorGeometryKind,
    ElementAppearanceUpdate, StructuralGroupCreation, ZOrderOperation as AppZOrderOperation,
''',
)

text_path = Path("apps/desktop/src-tauri/src/lib.rs")
text = text_path.read_text()
old = '''    let appearance =
        capture_clipboard_appearance(document.session.session().document(), &selected)?;'''
if text.count(old) != 2:
    raise SystemExit(f"expected two clipboard appearance anchors, found {text.count(old)}")
text = text.replace(
    old,
    '''    let appearance = capture_clipboard_appearance(
        document.session.session().document(),
        payload.source_element_ids(),
    )?;''',
)
text_path.write_text(text)

replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    '''    let appearance_updates =
        prepare_clipboard_appearance_updates(&clipboard.appearance, &mut instantiated)?;
    document
        .session
        .create_elements(target, instantiated.elements, appearance_updates)
        .map_err(|error| CommandError::new("clipboard_paste_failed", error.to_string()))?;
''',
    '''    let appearance_updates =
        prepare_clipboard_appearance_updates(&clipboard.appearance, &mut instantiated)?;
    let groups = instantiated
        .groups
        .into_iter()
        .map(|group| StructuralGroupCreation {
            group_id: group.group_id,
            element_ids: group.child_ids,
            name: group.name,
        })
        .collect();
    document
        .session
        .create_elements_with_groups(target, instantiated.elements, groups, appearance_updates)
        .map_err(|error| CommandError::new("clipboard_paste_failed", error.to_string()))?;
''',
)

replace_once(
    "apps/desktop/src-tauri/src/lib.rs",
    '''    let appearance_updates = prepare_clipboard_appearance_updates(&appearance, &mut instantiated)?;
    document
        .session
        .create_elements(target, instantiated.elements, appearance_updates)
        .map_err(|error| CommandError::new("duplicate_failed", error.to_string()))?;
''',
    '''    let appearance_updates = prepare_clipboard_appearance_updates(&appearance, &mut instantiated)?;
    let groups = instantiated
        .groups
        .into_iter()
        .map(|group| StructuralGroupCreation {
            group_id: group.group_id,
            element_ids: group.child_ids,
            name: group.name,
        })
        .collect();
    document
        .session
        .create_elements_with_groups(target, instantiated.elements, groups, appearance_updates)
        .map_err(|error| CommandError::new("duplicate_failed", error.to_string()))?;
''',
)

replace_once(
    "apps/desktop/ui/app.js",
    "import { isGroupActionEnabled, isUngroupActionEnabled } from './editor-interaction/group-actions.mjs';\n",
    "import { isGroupActionEnabled, isUngroupActionEnabled } from './editor-interaction/group-actions.mjs';\nimport { isClipboardSelectionActionEnabled } from './editor-interaction/clipboard-actions.mjs';\n",
)

replace_once(
    "apps/desktop/ui/app.js",
    '''function updateClipboardActionState() {
  const selectionCount = Number(currentSelectionProperties?.count ?? 0);
  const containsGroup = currentSelectionProperties?.containsGroup === true;
  elements.copySelection.disabled = isBusy || selectionCount === 0 || containsGroup;
  elements.duplicateSelection.disabled = isBusy || selectionCount === 0 || containsGroup;
  elements.pasteSelection.disabled = isBusy || !clipboardAvailable;
  const groupReason = containsGroup
    ? 'Structural groups are not copied or duplicated in this slice; ungroup first'
    : null;
  if (groupReason) {
    elements.copySelection.title = groupReason;
    elements.duplicateSelection.title = groupReason;
  } else {
    elements.copySelection.title = 'Copy the current selection (Ctrl/Cmd+C)';
    elements.duplicateSelection.title = 'Duplicate the current selection (Ctrl/Cmd+D)';
  }
}
''',
    '''function updateClipboardActionState() {
  const selectionCount = Number(currentSelectionProperties?.count ?? 0);
  const selectionEnabled = isClipboardSelectionActionEnabled({
    selectionCount,
    busy: isBusy,
  });
  elements.copySelection.disabled = !selectionEnabled;
  elements.duplicateSelection.disabled = !selectionEnabled;
  elements.pasteSelection.disabled = isBusy || !clipboardAvailable;
  elements.copySelection.title = 'Copy the current selection (Ctrl/Cmd+C)';
  elements.duplicateSelection.title = 'Duplicate the current selection (Ctrl/Cmd+D)';
}
''',
)

Path("apps/desktop/ui/editor-interaction/clipboard-actions.mjs").write_text(
    '''export function isClipboardSelectionActionEnabled({ selectionCount = 0, busy = false } = {}) {\n  return !busy && Number(selectionCount) > 0;\n}\n'''
)
Path("web/editor-interaction/clipboard-actions.mjs").write_text(
    "export { isClipboardSelectionActionEnabled } from '../../apps/desktop/ui/editor-interaction/clipboard-actions.mjs';\n"
)
Path("web/editor-interaction/clipboard-actions.test.mjs").write_text(
    '''import test from 'node:test';\nimport assert from 'node:assert/strict';\n\nimport { isClipboardSelectionActionEnabled } from './clipboard-actions.mjs';\n\ntest('clipboard selection actions allow structural group selections', () => {\n  assert.equal(isClipboardSelectionActionEnabled(), false);\n  assert.equal(isClipboardSelectionActionEnabled({ selectionCount: 0, busy: false }), false);\n  assert.equal(isClipboardSelectionActionEnabled({ selectionCount: 1, busy: true }), false);\n  assert.equal(isClipboardSelectionActionEnabled({ selectionCount: 1, busy: false }), true);\n  assert.equal(\n    isClipboardSelectionActionEnabled({ selectionCount: 1, busy: false, containsGroup: true }),\n    true,\n  );\n});\n'''
)

Path("crates/app-core/tests/group_clipboard_transaction.rs").write_text(
    r'''use app_core::{ApplicationSession, StructuralGroupCreation};
use ddnx::PackageLimits;
use editor_core::LayerTarget;
use next_domain::{
    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
    ElementKind, Layer, LayerId, NextArtifact, Page, PageId, Rect, Scene, Size,
};

fn rectangle(id: ElementId, name: &str, x: f64) -> Element {
    Element {
        id,
        name: name.to_owned(),
        bounds_mm: Rect {
            x,
            y: 20.0,
            width: 20.0,
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

fn fixture() -> (NextArtifact, LayerTarget) {
    let page_id = PageId::new();
    let layer_id = LayerId::new();
    let document = Document {
        id: DocumentId::new(),
        name: "Clipboard group transaction".to_owned(),
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
                    roots: Vec::new(),
                    elements: Vec::new(),
                },
            }],
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    };
    (
        NextArtifact::document(document),
        LayerTarget::Page { page_id, layer_id },
    )
}

fn roots(app: &ApplicationSession) -> Vec<ElementId> {
    app.session().document().pages[0].layers[0]
        .scene
        .roots
        .clone()
}

fn group_children(app: &ApplicationSession, group_id: ElementId) -> Vec<ElementId> {
    let element = app.session().document().pages[0].layers[0]
        .scene
        .elements
        .iter()
        .find(|element| element.id == group_id)
        .unwrap();
    let ElementKind::Group { children } = &element.kind else {
        panic!("expected structural group")
    };
    children.clone()
}

#[test]
fn clipboard_hierarchy_is_one_transaction_and_round_trips_through_ddnx() {
    let (artifact, target) = fixture();
    let mut app = ApplicationSession::from_artifact(artifact).unwrap();
    let initial_history = app.session().current_history_state();
    let first = ElementId::new();
    let second = ElementId::new();
    let third = ElementId::new();
    let ordinary = ElementId::new();
    let inner = ElementId::new();
    let outer = ElementId::new();

    assert!(
        app.create_elements_with_groups(
            target,
            vec![
                rectangle(first, "First", 15.0),
                rectangle(second, "Second", 40.0),
                rectangle(third, "Third", 65.0),
                rectangle(ordinary, "Ordinary", 100.0),
            ],
            vec![
                StructuralGroupCreation {
                    group_id: inner,
                    element_ids: vec![first, second],
                    name: "Inner".to_owned(),
                },
                StructuralGroupCreation {
                    group_id: outer,
                    element_ids: vec![inner, third],
                    name: "Outer".to_owned(),
                },
            ],
            Vec::new(),
        )
        .unwrap()
    );
    assert_eq!(roots(&app), vec![outer, ordinary]);
    assert_eq!(group_children(&app, inner), vec![first, second]);
    assert_eq!(group_children(&app, outer), vec![inner, third]);
    let created_history = app.session().current_history_state();
    assert_ne!(created_history, initial_history);

    let prepared = app.prepare_document_save(PackageLimits::default()).unwrap();
    let reopened =
        ApplicationSession::from_ddnx_bytes(prepared.bytes(), PackageLimits::default()).unwrap();
    assert_eq!(roots(&reopened), vec![outer, ordinary]);
    assert_eq!(group_children(&reopened, inner), vec![first, second]);
    assert_eq!(group_children(&reopened, outer), vec![inner, third]);

    assert!(app.undo().unwrap());
    assert_eq!(app.session().current_history_state(), initial_history);
    assert!(roots(&app).is_empty());
    assert!(app.session().document().pages[0].layers[0]
        .scene
        .elements
        .is_empty());

    assert!(app.redo().unwrap());
    assert_eq!(app.session().current_history_state(), created_history);
    assert_eq!(roots(&app), vec![outer, ordinary]);
    assert_eq!(group_children(&app, inner), vec![first, second]);
    assert_eq!(group_children(&app, outer), vec![inner, third]);
}
'''
)
