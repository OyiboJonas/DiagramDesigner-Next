use std::collections::{BTreeMap, BTreeSet};

use next_domain::{Document, Element, ElementId, ElementKind, Layer, LayerId, PageId, Rect, Scene};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SelectionGroupSnapshot {
    pub group_id: ElementId,
    pub bounds_mm: Rect,
    pub leaf_element_ids: Vec<ElementId>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct SelectionCapabilities {
    pub can_group: bool,
    pub can_ungroup: bool,
    pub contains_group: bool,
}

pub(crate) fn selection_groups(document: &Document) -> Vec<SelectionGroupSnapshot> {
    let mut groups = Vec::new();
    for layer in document.master_layers.iter().filter(|layer| layer.visible) {
        collect_layer_selection_groups(layer, &mut groups);
    }
    for page in &document.pages {
        for layer in page.layers.iter().filter(|layer| layer.visible) {
            collect_layer_selection_groups(layer, &mut groups);
        }
    }
    groups
}

pub(crate) fn selection_capabilities(
    document: &Document,
    active_page_id: Option<PageId>,
    active_layer_id: Option<LayerId>,
    selected: &[ElementId],
) -> SelectionCapabilities {
    let contains_group = selected.iter().any(|element_id| {
        find_element(document, *element_id)
            .is_some_and(|element| matches!(element.kind, ElementKind::Group { .. }))
    });
    let Some(layer) = active_page_layer(document, active_page_id, active_layer_id) else {
        return SelectionCapabilities {
            contains_group,
            ..SelectionCapabilities::default()
        };
    };
    if !layer.visible || layer.locked {
        return SelectionCapabilities {
            contains_group,
            ..SelectionCapabilities::default()
        };
    }

    let selected_set: BTreeSet<_> = selected.iter().copied().collect();
    let mut positions: Vec<_> = layer
        .scene
        .roots
        .iter()
        .enumerate()
        .filter_map(|(index, element_id)| selected_set.contains(element_id).then_some(index))
        .collect();
    positions.sort_unstable();
    let all_selected_are_roots = positions.len() == selected_set.len();
    let contiguous = positions
        .windows(2)
        .all(|window| window[1] == window[0].saturating_add(1));
    let can_group = selected_set.len() >= 2 && all_selected_are_roots && contiguous;

    let can_ungroup = selected_set.len() == 1
        && positions.len() == 1
        && find_element_in_scene(&layer.scene, *selected_set.iter().next().unwrap())
            .is_some_and(|element| matches!(element.kind, ElementKind::Group { .. }));

    SelectionCapabilities {
        can_group,
        can_ungroup,
        contains_group,
    }
}

pub(crate) fn selected_group_children(
    document: &Document,
    active_page_id: Option<PageId>,
    active_layer_id: Option<LayerId>,
    selected: &[ElementId],
) -> Option<Vec<ElementId>> {
    let capabilities = selection_capabilities(document, active_page_id, active_layer_id, selected);
    if !capabilities.can_ungroup {
        return None;
    }
    let layer = active_page_layer(document, active_page_id, active_layer_id)?;
    let element = find_element_in_scene(&layer.scene, selected[0])?;
    let ElementKind::Group { children } = &element.kind else {
        return None;
    };
    Some(children.clone())
}

fn active_page_layer(
    document: &Document,
    active_page_id: Option<PageId>,
    active_layer_id: Option<LayerId>,
) -> Option<&Layer> {
    let page_id = active_page_id?;
    let layer_id = active_layer_id?;
    document
        .pages
        .iter()
        .find(|page| page.id == page_id)?
        .layers
        .iter()
        .find(|layer| layer.id == layer_id)
}

fn collect_layer_selection_groups(layer: &Layer, output: &mut Vec<SelectionGroupSnapshot>) {
    let elements: BTreeMap<_, _> = layer
        .scene
        .elements
        .iter()
        .map(|element| (element.id, element))
        .collect();
    for root_id in &layer.scene.roots {
        let Some(root) = elements.get(root_id).copied() else {
            continue;
        };
        if !matches!(root.kind, ElementKind::Group { .. }) {
            continue;
        }
        let mut leaves = Vec::new();
        let mut visiting = BTreeSet::new();
        if collect_leaf_ids(*root_id, &elements, &mut visiting, &mut leaves) && !leaves.is_empty() {
            output.push(SelectionGroupSnapshot {
                group_id: *root_id,
                bounds_mm: root.bounds_mm,
                leaf_element_ids: leaves,
            });
        }
    }
}

fn collect_leaf_ids(
    element_id: ElementId,
    elements: &BTreeMap<ElementId, &Element>,
    visiting: &mut BTreeSet<ElementId>,
    output: &mut Vec<ElementId>,
) -> bool {
    if !visiting.insert(element_id) {
        return false;
    }
    let Some(element) = elements.get(&element_id).copied() else {
        visiting.remove(&element_id);
        return false;
    };
    let valid = match &element.kind {
        ElementKind::Group { children } => children
            .iter()
            .all(|child_id| collect_leaf_ids(*child_id, elements, visiting, output)),
        _ => {
            output.push(element_id);
            true
        }
    };
    visiting.remove(&element_id);
    valid
}

fn find_element(document: &Document, element_id: ElementId) -> Option<&Element> {
    document
        .master_layers
        .iter()
        .flat_map(|layer| layer.scene.elements.iter())
        .chain(
            document
                .pages
                .iter()
                .flat_map(|page| page.layers.iter())
                .flat_map(|layer| layer.scene.elements.iter()),
        )
        .find(|element| element.id == element_id)
}

fn find_element_in_scene(scene: &Scene, element_id: ElementId) -> Option<&Element> {
    scene
        .elements
        .iter()
        .find(|element| element.id == element_id)
}

#[cfg(test)]
mod tests {
    use next_domain::{
        AnchorSet, ConnectorLabelStyle, DocumentDefaults, DocumentId, LayerId, Page, Rect, Size,
    };

    use super::*;

    fn element(id: ElementId, x: f64, kind: ElementKind) -> Element {
        Element {
            id,
            name: String::new(),
            bounds_mm: Rect {
                x,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            rotation_deg: 0.0,
            anchors: AnchorSet::default(),
            ports: Vec::new(),
            style_id: None,
            text: None,
            kind,
            import: None,
        }
    }

    fn fixture(locked: bool) -> (Document, PageId, LayerId, [ElementId; 4]) {
        let ids = [
            ElementId::new(),
            ElementId::new(),
            ElementId::new(),
            ElementId::new(),
        ];
        let nested = ElementId::new();
        let group = ids[0];
        let page_id = PageId::new();
        let layer_id = LayerId::new();
        let document = Document {
            id: DocumentId::new(),
            name: String::new(),
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
                name: String::new(),
                size_mm: Size {
                    width: 210.0,
                    height: 297.0,
                },
                layers: vec![Layer {
                    id: layer_id,
                    name: String::new(),
                    visible: true,
                    locked,
                    draw_color: None,
                    scene: Scene {
                        roots: vec![group, ids[3]],
                        elements: vec![
                            element(
                                group,
                                0.0,
                                ElementKind::Group {
                                    children: vec![nested, ids[2]],
                                },
                            ),
                            element(
                                nested,
                                0.0,
                                ElementKind::Group {
                                    children: vec![ids[1]],
                                },
                            ),
                            element(ids[1], 0.0, ElementKind::Ellipse),
                            element(
                                ids[2],
                                12.0,
                                ElementKind::Rectangle {
                                    corner_radius_mm: 0.0,
                                },
                            ),
                            element(ids[3], 30.0, ElementKind::Ellipse),
                        ],
                    },
                }],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        };
        (document, page_id, layer_id, ids)
    }

    #[test]
    fn top_level_group_maps_to_rendered_leaf_descendants() {
        let (document, _, _, ids) = fixture(false);
        let groups = selection_groups(&document);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].group_id, ids[0]);
        assert_eq!(groups[0].leaf_element_ids, vec![ids[1], ids[2]]);
    }

    #[test]
    fn capabilities_require_mutable_contiguous_top_level_selection() {
        let (document, page_id, layer_id, ids) = fixture(false);
        let group_only =
            selection_capabilities(&document, Some(page_id), Some(layer_id), &[ids[0]]);
        assert!(group_only.can_ungroup);
        assert!(group_only.contains_group);

        let adjacent =
            selection_capabilities(&document, Some(page_id), Some(layer_id), &[ids[0], ids[3]]);
        assert!(adjacent.can_group);

        let (locked, page_id, layer_id, ids) = fixture(true);
        let blocked =
            selection_capabilities(&locked, Some(page_id), Some(layer_id), &[ids[0], ids[3]]);
        assert!(!blocked.can_group);
        assert!(!blocked.can_ungroup);
    }
}
