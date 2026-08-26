use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use next_domain::{Connection, Document, Element, ElementId, ElementKind, Endpoint, Point, PortId};

pub const PASTE_OFFSET_MM: f64 = 5.0;

#[derive(Debug, Clone)]
struct ClipboardGroupPayload {
    element: Element,
    z_index: Option<usize>,
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
    pub element: Element,
    pub z_index: Option<usize>,
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
    SelectionRequiresTopLevel(ElementId),
    SelectionSpansScenes,
    GroupCycle(ElementId),
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
            Self::SelectionRequiresTopLevel(element_id) => write!(
                formatter,
                "Selected element {element_id:?} must be a top-level scene root before it can be copied."
            ),
            Self::SelectionSpansScenes => formatter.write_str(
                "Clipboard selection must stay within one scene/layer."
            ),
            Self::GroupCycle(element_id) => write!(
                formatter,
                "Structural group hierarchy contains a cycle at {element_id:?}."
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
            element_ids.insert(group.element.id, ElementId::new());
            for port in &group.element.ports {
                port_ids.insert(port.id, PortId::new());
            }
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
            .map(|group| {
                let source = &group.element;
                let mut element = source.clone();
                element.id = element_ids[&source.id];
                element.import = None;
                element.bounds_mm.x += delta.x;
                element.bounds_mm.y += delta.y;
                for port in &mut element.ports {
                    port.id = port_ids[&port.id];
                }
                let ElementKind::Group { children } = &mut element.kind else {
                    unreachable!("clipboard group storage contains only structural groups")
                };
                for child_id in children {
                    *child_id = element_ids[child_id];
                }
                ClipboardGroupInstantiation {
                    element,
                    z_index: group.z_index,
                }
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
    let mut scene_by_id = BTreeMap::new();
    let mut roots_by_scene = Vec::new();

    for layer in &document.master_layers {
        let scene_index = roots_by_scene.len();
        roots_by_scene.push(layer.scene.roots.clone());
        for element in &layer.scene.elements {
            by_id.insert(element.id, element);
            scene_by_id.insert(element.id, scene_index);
        }
    }
    for page in &document.pages {
        for layer in &page.layers {
            let scene_index = roots_by_scene.len();
            roots_by_scene.push(layer.scene.roots.clone());
            for element in &layer.scene.elements {
                by_id.insert(element.id, element);
                scene_by_id.insert(element.id, scene_index);
            }
        }
    }

    let mut selected_scene = None;
    for element_id in &selected_set {
        by_id
            .get(element_id)
            .copied()
            .ok_or(ClipboardError::MissingElement(*element_id))?;
        let scene_index = *scene_by_id
            .get(element_id)
            .ok_or(ClipboardError::MissingElement(*element_id))?;
        if let Some(expected_scene) = selected_scene {
            if expected_scene != scene_index {
                return Err(ClipboardError::SelectionSpansScenes);
            }
        } else {
            selected_scene = Some(scene_index);
        }
        if !roots_by_scene[scene_index].contains(element_id) {
            return Err(ClipboardError::SelectionRequiresTopLevel(*element_id));
        }
    }

    // Logical clipboard selections are direct roots from exactly one scene. Preserve
    // that scene's canonical back-to-front root order instead of UUID order.
    let scene_index = selected_scene.expect("non-empty selection has a scene");
    let ordered_ids: Vec<_> = roots_by_scene[scene_index]
        .iter()
        .copied()
        .filter(|element_id| selected_set.contains(element_id))
        .collect();

    let mut captured = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    let mut elements = Vec::new();
    let mut groups = Vec::new();
    let mut source_ids = Vec::new();
    let mut initial_order = Vec::new();
    for element_id in &ordered_ids {
        collect_subtree(
            *element_id,
            &by_id,
            &mut captured,
            &mut visiting,
            &mut elements,
            &mut groups,
            &mut source_ids,
            &mut initial_order,
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
    initial_order: &mut Vec<ElementId>,
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
            let z_index = if children.is_empty() {
                let index = initial_order.len();
                initial_order.push(element_id);
                Some(index)
            } else {
                for child_id in children {
                    collect_subtree(
                        *child_id,
                        by_id,
                        captured,
                        visiting,
                        elements,
                        groups,
                        source_ids,
                        initial_order,
                    )?;
                }
                None
            };
            groups.push(ClipboardGroupPayload {
                element: element.clone(),
                z_index,
            });
        }
        _ => {
            elements.push(element.clone());
            initial_order.push(element_id);
        }
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
        assert_eq!(payload.groups[0].element.id, inner_group);
        assert_eq!(payload.groups[1].element.id, outer_group);
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
        assert_eq!(instantiated.groups[0].element.id, copied_inner);
        let ElementKind::Group { children } = &instantiated.groups[0].element.kind else {
            panic!("inner group")
        };
        assert_eq!(children, &vec![copied_first, copied_connector_id]);
        assert_eq!(instantiated.groups[0].element.name, "Inner");
        assert_eq!(instantiated.groups[0].z_index, None);
        assert_eq!(instantiated.groups[1].element.id, copied_outer);
        let ElementKind::Group { children } = &instantiated.groups[1].element.kind else {
            panic!("outer group")
        };
        assert_eq!(children, &vec![copied_inner, copied_second]);
        assert_eq!(instantiated.groups[1].element.name, "Outer");
        assert_eq!(instantiated.groups[1].z_index, None);

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
    fn empty_and_singleton_groups_are_captured_without_flattening() {
        let leaf = ElementId::new();
        let empty = ElementId::new();
        let singleton = ElementId::new();
        let outer = ElementId::new();
        let document = document_with_elements(
            vec![outer],
            vec![
                group(empty, "Empty", Vec::new()),
                shape(leaf, PortId::new(), 50.0),
                group(singleton, "Singleton", vec![leaf]),
                group(outer, "Outer", vec![empty, singleton]),
            ],
        );
        let payload = capture_selection(&document, &[outer]).unwrap();
        assert_eq!(payload.elements.len(), 1);
        assert_eq!(payload.groups.len(), 3);
        assert_eq!(payload.groups[0].element.id, empty);
        assert_eq!(payload.groups[0].z_index, Some(0));
        assert_eq!(payload.groups[1].element.id, singleton);
        assert_eq!(payload.groups[2].element.id, outer);
        let instantiated = payload.instantiate(1);
        let copied_empty = instantiated.source_element_ids[&empty];
        let copied_leaf = instantiated.source_element_ids[&leaf];
        let copied_singleton = instantiated.source_element_ids[&singleton];
        let copied_outer = instantiated.source_element_ids[&outer];
        assert_eq!(instantiated.element_ids, vec![copied_outer]);
        assert_eq!(instantiated.groups[0].element.id, copied_empty);
        assert_eq!(instantiated.groups[0].z_index, Some(0));
        let ElementKind::Group { children } = &instantiated.groups[1].element.kind else {
            panic!("singleton")
        };
        assert_eq!(children, &vec![copied_leaf]);
        let ElementKind::Group { children } = &instantiated.groups[2].element.kind else {
            panic!("outer")
        };
        assert_eq!(children, &vec![copied_empty, copied_singleton]);
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
            Err(ClipboardError::SelectionRequiresTopLevel(id)) if id == inner
        ));
    }

    #[test]
    fn ordinary_group_child_selection_is_rejected_as_non_top_level() {
        let child = ElementId::new();
        let owner = ElementId::new();
        let document = document_with_elements(
            vec![owner],
            vec![
                shape(child, PortId::new(), 10.0),
                group(owner, "Owner", vec![child]),
            ],
        );

        assert!(matches!(
            capture_selection(&document, &[child]),
            Err(ClipboardError::SelectionRequiresTopLevel(id)) if id == child
        ));
    }

    #[test]
    fn selection_spanning_two_scenes_is_rejected_explicitly() {
        let first = ElementId::new();
        let second = ElementId::new();
        let mut document = document_with_elements(
            vec![first],
            vec![shape(first, PortId::new(), 10.0)],
        );
        document.pages.push(Page {
            id: PageId::new(),
            name: "Second page".to_owned(),
            size_mm: Size {
                width: 210.0,
                height: 297.0,
            },
            layers: vec![Layer {
                id: LayerId::new(),
                name: "Second layer".to_owned(),
                visible: true,
                locked: false,
                draw_color: None,
                scene: Scene {
                    roots: vec![second],
                    elements: vec![shape(second, PortId::new(), 80.0)],
                },
            }],
        });

        assert!(matches!(
            capture_selection(&document, &[first, second]),
            Err(ClipboardError::SelectionSpansScenes)
        ));
    }
}
