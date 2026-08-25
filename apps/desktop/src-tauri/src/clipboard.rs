use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use next_domain::{Connection, Document, Element, ElementId, ElementKind, Endpoint, Point, PortId};

pub const PASTE_OFFSET_MM: f64 = 5.0;

#[derive(Debug, Clone)]
pub struct ClipboardPayload {
    elements: Vec<Element>,
}

#[derive(Debug)]
pub struct ClipboardInstantiation {
    pub elements: Vec<Element>,
    pub element_ids: Vec<ElementId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardError {
    EmptySelection,
    MissingElement(ElementId),
    GroupUnsupported(ElementId),
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySelection => formatter.write_str("Select at least one element to copy."),
            Self::MissingElement(element_id) => {
                write!(
                    formatter,
                    "Selected element {element_id:?} no longer exists."
                )
            }
            Self::GroupUnsupported(element_id) => write!(
                formatter,
                "Copying structural group {element_id:?} is not available in this first clipboard slice; ungroup it first."
            ),
        }
    }
}

impl std::error::Error for ClipboardError {}

impl ClipboardPayload {
    pub fn len(&self) -> usize {
        self.elements.len()
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

        let mut instantiated = Vec::with_capacity(self.elements.len());
        let mut selected = Vec::with_capacity(self.elements.len());
        for source in &self.elements {
            let mut element = source.clone();
            element.id = element_ids[&source.id];
            selected.push(element.id);
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
                    unreachable!("groups are rejected while capturing the clipboard payload")
                }
                _ => {}
            }
            instantiated.push(element);
        }

        ClipboardInstantiation {
            elements: instantiated,
            element_ids: selected,
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
    for layer in &document.master_layers {
        for element in &layer.scene.elements {
            by_id.insert(element.id, element);
        }
    }
    for page in &document.pages {
        for layer in &page.layers {
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
        if matches!(&element.kind, ElementKind::Group { .. }) {
            return Err(ClipboardError::GroupUnsupported(*element_id));
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

    Ok(ClipboardPayload {
        elements: ordered_ids
            .into_iter()
            .map(|element_id| by_id[&element_id].clone())
            .collect(),
    })
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

    fn shape(element_id: ElementId, port_id: PortId) -> Element {
        Element {
            id: element_id,
            name: "Shape".to_owned(),
            bounds_mm: Rect {
                x: 10.0,
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

    #[test]
    fn instantiate_remaps_internal_connections_and_detaches_external_targets() {
        let shape_id = ElementId::new();
        let shape_port = PortId::new();
        let connector_id = ElementId::new();
        let external_id = ElementId::new();
        let external_port = PortId::new();
        let payload = ClipboardPayload {
            elements: vec![
                shape(shape_id, shape_port),
                connector(
                    connector_id,
                    shape_id,
                    shape_port,
                    external_id,
                    external_port,
                ),
            ],
        };

        let first = payload.instantiate(1);
        assert_eq!(first.elements.len(), 2);
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
    fn capture_rejects_structural_groups() {
        let group_id = ElementId::new();
        let group = Element {
            id: group_id,
            name: "Group".to_owned(),
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
            kind: ElementKind::Group {
                children: Vec::new(),
            },
            import: None,
        };
        let page_id = PageId::new();
        let layer_id = LayerId::new();
        let document = Document {
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
                        roots: vec![group_id],
                        elements: vec![group],
                    },
                }],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        };

        assert!(matches!(
            capture_selection(&document, &[group_id]),
            Err(ClipboardError::GroupUnsupported(id)) if id == group_id
        ));
    }
}
