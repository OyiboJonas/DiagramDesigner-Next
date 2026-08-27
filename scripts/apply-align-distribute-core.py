from pathlib import Path


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    if text.count(old) != 1:
        raise SystemExit(f"expected exactly one match in {path}: {old[:80]!r}, got {text.count(old)}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


core = Path("crates/editor-core/src/lib.rs")

replace_once(
    core,
    """pub enum ZOrderOperation {\n    BringToFront,\n    SendToBack,\n    BringForward,\n    SendBackward,\n}\n\n#[derive(Debug, Clone, PartialEq)]\npub struct ConnectorEndpointSnapshot""",
    """pub enum ZOrderOperation {\n    BringToFront,\n    SendToBack,\n    BringForward,\n    SendBackward,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ArrangeOperation {\n    AlignLeft,\n    AlignHorizontalCenter,\n    AlignRight,\n    AlignTop,\n    AlignVerticalCenter,\n    AlignBottom,\n    DistributeHorizontal,\n    DistributeVertical,\n}\n\nimpl ArrangeOperation {\n    fn minimum_selection(self) -> usize {\n        match self {\n            Self::DistributeHorizontal | Self::DistributeVertical => 3,\n            _ => 2,\n        }\n    }\n}\n\n#[derive(Debug, Clone, PartialEq)]\npub struct ConnectorEndpointSnapshot""",
)

replace_once(
    core,
    """    MoveElements {\n        element_ids: Vec<ElementId>,\n        delta_mm: Point,\n    },\n    ReorderElements {""",
    """    MoveElements {\n        element_ids: Vec<ElementId>,\n        delta_mm: Point,\n    },\n    /// Align or distribute direct scene roots from canonical document geometry.\n    /// Structural groups participate as one logical object and are expanded only\n    /// when the computed movement is committed.\n    ArrangeElements {\n        element_ids: Vec<ElementId>,\n        operation: ArrangeOperation,\n    },\n    ReorderElements {""",
)

replace_once(
    core,
    """    #[error(\"z-order selection spans more than one layer\")]\n    ZOrderDifferentLayers,\n    #[error(\"z-order editing currently requires top-level element {0:?}\")]\n    ZOrderRequiresTopLevelElement(ElementId),\n    #[error(\"z-order index {index} is outside 0..={len}\")]""",
    """    #[error(\"z-order selection spans more than one layer\")]\n    ZOrderDifferentLayers,\n    #[error(\"z-order editing currently requires top-level element {0:?}\")]\n    ZOrderRequiresTopLevelElement(ElementId),\n    #[error(\"arrange selection spans more than one layer\")]\n    ArrangeDifferentLayers,\n    #[error(\"arrange editing currently requires top-level element {0:?}\")]\n    ArrangeRequiresTopLevelElement(ElementId),\n    #[error(\"arrange operation requires at least {required} elements; got {actual}\")]\n    ArrangeRequiresAtLeast { required: usize, actual: usize },\n    #[error(\"z-order index {index} is outside 0..={len}\")]""",
)

replace_once(
    core,
    """    MoveElements {\n        element_ids: Vec<ElementId>,\n        delta_mm: Point,\n    },\n    RestoreZOrder {""",
    """    MoveElements {\n        element_ids: Vec<ElementId>,\n        delta_mm: Point,\n    },\n    ArrangeElements {\n        movements: Vec<(Vec<ElementId>, Point)>,\n    },\n    RestoreZOrder {""",
)

replace_once(
    core,
    """        EditCommand::MoveElements {\n            element_ids,\n            delta_mm,\n        } => apply_move(document, element_ids, *delta_mm),\n        EditCommand::ReorderElements {""",
    """        EditCommand::MoveElements {\n            element_ids,\n            delta_mm,\n        } => apply_move(document, element_ids, *delta_mm),\n        EditCommand::ArrangeElements {\n            element_ids,\n            operation,\n        } => apply_arrange_elements(document, element_ids, *operation),\n        EditCommand::ReorderElements {""",
)

arrange_impl = r'''
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
        let element = find_element(document, element_id)
            .ok_or(EditorError::ElementNotFound(element_id))?;
        items.push(ArrangeItem {
            element_id,
            bounds: element_visual_bounds(element),
        });
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
                logical_movements.push((
                    item.element_id,
                    Point {
                        x: delta_x,
                        y: 0.0,
                    },
                ));
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
                logical_movements.push((
                    item.element_id,
                    Point {
                        x: 0.0,
                        y: delta_y,
                    },
                ));
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

'''
replace_once(
    core,
    """fn apply_reorder_elements(\n    document: &mut Document,""",
    arrange_impl + "fn apply_reorder_elements(\n    document: &mut Document,",
)

replace_once(
    core,
    """        UndoStep::MoveElements {\n            element_ids,\n            delta_mm,\n        } => {\n            for element_id in element_ids {\n                let element = find_element_mut(document, *element_id)\n                    .ok_or(EditorError::HistoryInvariantViolation)?;\n                translate_element_geometry(element, *delta_mm);\n            }\n            synchronize_connected_endpoints(document)?;\n        }\n        UndoStep::RestoreZOrder""",
    """        UndoStep::MoveElements {\n            element_ids,\n            delta_mm,\n        } => {\n            for element_id in element_ids {\n                let element = find_element_mut(document, *element_id)\n                    .ok_or(EditorError::HistoryInvariantViolation)?;\n                translate_element_geometry(element, *delta_mm);\n            }\n            synchronize_connected_endpoints(document)?;\n        }\n        UndoStep::ArrangeElements { movements } => {\n            for (element_ids, delta_mm) in movements {\n                for element_id in element_ids {\n                    let element = find_element_mut(document, *element_id)\n                        .ok_or(EditorError::HistoryInvariantViolation)?;\n                    translate_element_geometry(element, *delta_mm);\n                }\n            }\n            synchronize_connected_endpoints(document)?;\n        }\n        UndoStep::RestoreZOrder""",
)

visual_helper = r'''
fn element_visual_bounds(element: &Element) -> Rect {
    if element.rotation_deg == 0.0 {
        return element.bounds_mm;
    }
    let center_x = element.bounds_mm.x + element.bounds_mm.width / 2.0;
    let center_y = element.bounds_mm.y + element.bounds_mm.height / 2.0;
    let radians = element.rotation_deg.to_radians();
    let cos = radians.cos().abs();
    let sin = radians.sin().abs();
    let width = element.bounds_mm.width * cos + element.bounds_mm.height * sin;
    let height = element.bounds_mm.width * sin + element.bounds_mm.height * cos;
    Rect {
        x: center_x - width / 2.0,
        y: center_y - height / 2.0,
        width,
        height,
    }
}

'''
replace_once(
    core,
    """/// Bounds are common to every element. Connector endpoint positions and curve\n/// control points are also absolute document coordinates""",
    visual_helper + "/// Bounds are common to every element. Connector endpoint positions and curve\n/// control points are also absolute document coordinates",
)

app = Path("crates/app-core/src/lib.rs")
replace_once(
    app,
    """    ConnectorEndpointSide as CoreConnectorEndpointSide,\n    ConnectorEndpointSnapshot as CoreConnectorEndpointSnapshot,\n    ConnectorGeometryKind as CoreConnectorGeometryKind, EditCommand, EditTransaction, EditorError,""",
    """    ArrangeOperation as CoreArrangeOperation, ConnectorEndpointSide as CoreConnectorEndpointSide,\n    ConnectorEndpointSnapshot as CoreConnectorEndpointSnapshot,\n    ConnectorGeometryKind as CoreConnectorGeometryKind, EditCommand, EditTransaction, EditorError,""",
)
replace_once(
    app,
    """pub enum ZOrderOperation {\n    BringToFront,\n    SendToBack,\n    BringForward,\n    SendBackward,\n}\n\nimpl From<ZOrderOperation> for CoreZOrderOperation""",
    """pub enum ZOrderOperation {\n    BringToFront,\n    SendToBack,\n    BringForward,\n    SendBackward,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ArrangeOperation {\n    AlignLeft,\n    AlignHorizontalCenter,\n    AlignRight,\n    AlignTop,\n    AlignVerticalCenter,\n    AlignBottom,\n    DistributeHorizontal,\n    DistributeVertical,\n}\n\nimpl From<ArrangeOperation> for CoreArrangeOperation {\n    fn from(value: ArrangeOperation) -> Self {\n        match value {\n            ArrangeOperation::AlignLeft => Self::AlignLeft,\n            ArrangeOperation::AlignHorizontalCenter => Self::AlignHorizontalCenter,\n            ArrangeOperation::AlignRight => Self::AlignRight,\n            ArrangeOperation::AlignTop => Self::AlignTop,\n            ArrangeOperation::AlignVerticalCenter => Self::AlignVerticalCenter,\n            ArrangeOperation::AlignBottom => Self::AlignBottom,\n            ArrangeOperation::DistributeHorizontal => Self::DistributeHorizontal,\n            ArrangeOperation::DistributeVertical => Self::DistributeVertical,\n        }\n    }\n}\n\nimpl From<ZOrderOperation> for CoreZOrderOperation""",
)
replace_once(
    app,
    """    /// Reorder top-level elements through editor-core's canonical scene-root order.\n    pub fn reorder_elements(""",
    """    /// Align or distribute direct scene roots from canonical document geometry.\n    pub fn arrange_elements(\n        &mut self,\n        element_ids: Vec<ElementId>,\n        operation: ArrangeOperation,\n    ) -> Result<bool, ApplicationError> {\n        self.execute_edit(EditCommand::ArrangeElements {\n            element_ids,\n            operation: operation.into(),\n        })\n    }\n\n    /// Reorder top-level elements through editor-core's canonical scene-root order.\n    pub fn reorder_elements(""",
)

core_test = Path("crates/editor-core/tests/arrange_elements.rs")
core_test.parent.mkdir(parents=True, exist_ok=True)
core_test.write_text(r'''use editor_core::{ArrangeOperation, EditCommand, EditorError, EditorSession};
use next_domain::{
    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
    ElementKind, Layer, LayerId, NextArtifact, Page, PageId, Rect, Scene, Size,
};

fn element(id: ElementId, x: f64, y: f64, width: f64, height: f64) -> Element {
    Element {
        id,
        name: String::new(),
        bounds_mm: Rect { x, y, width, height },
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text: None,
        kind: ElementKind::Rectangle { corner_radius_mm: 0.0 },
        import: None,
    }
}

fn fixture() -> (EditorSession, [ElementId; 4]) {
    let ids = [ElementId::new(), ElementId::new(), ElementId::new(), ElementId::new()];
    let page_id = PageId::new();
    let layer_id = LayerId::new();
    let document = Document {
        id: DocumentId::new(),
        name: "Arrange".to_owned(),
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
            size_mm: Size { width: 210.0, height: 297.0 },
            layers: vec![Layer {
                id: layer_id,
                name: "Layer 1".to_owned(),
                visible: true,
                locked: false,
                draw_color: None,
                scene: Scene {
                    roots: ids.to_vec(),
                    elements: vec![
                        element(ids[0], 10.0, 10.0, 10.0, 10.0),
                        element(ids[1], 35.0, 25.0, 20.0, 10.0),
                        element(ids[2], 80.0, 45.0, 10.0, 20.0),
                        element(ids[3], 130.0, 80.0, 20.0, 20.0),
                    ],
                },
            }],
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    };
    (EditorSession::from_artifact(NextArtifact::document(document)).unwrap(), ids)
}

fn bounds(session: &EditorSession, id: ElementId) -> Rect {
    session.document().pages[0].layers[0]
        .scene
        .elements
        .iter()
        .find(|element| element.id == id)
        .unwrap()
        .bounds_mm
}

#[test]
fn alignment_is_one_history_step_and_noop_is_not_history() {
    let (mut session, ids) = fixture();
    let initial = session.current_history_state();
    assert!(session.execute(EditCommand::ArrangeElements {
        element_ids: vec![ids[2], ids[0], ids[1]],
        operation: ArrangeOperation::AlignLeft,
    }).unwrap());
    assert_eq!(bounds(&session, ids[0]).x, 10.0);
    assert_eq!(bounds(&session, ids[1]).x, 10.0);
    assert_eq!(bounds(&session, ids[2]).x, 10.0);
    let aligned = session.current_history_state();
    assert_ne!(aligned, initial);
    assert!(session.undo().unwrap());
    assert_eq!(session.current_history_state(), initial);
    assert_eq!(bounds(&session, ids[1]).x, 35.0);
    assert!(session.redo().unwrap());
    assert_eq!(session.current_history_state(), aligned);

    let before_noop = session.current_history_state();
    assert!(!session.execute(EditCommand::ArrangeElements {
        element_ids: vec![ids[1], ids[0], ids[2]],
        operation: ArrangeOperation::AlignLeft,
    }).unwrap());
    assert_eq!(session.current_history_state(), before_noop);
}

#[test]
fn distribution_keeps_outer_items_fixed_and_ignores_caller_order() {
    let (mut session, ids) = fixture();
    assert!(session.execute(EditCommand::ArrangeElements {
        element_ids: vec![ids[3], ids[1], ids[0], ids[2]],
        operation: ArrangeOperation::DistributeHorizontal,
    }).unwrap());
    assert_eq!(bounds(&session, ids[0]).x, 10.0);
    assert_eq!(bounds(&session, ids[3]).x, 130.0);
    let gap1 = bounds(&session, ids[1]).x - (bounds(&session, ids[0]).x + bounds(&session, ids[0]).width);
    let gap2 = bounds(&session, ids[2]).x - (bounds(&session, ids[1]).x + bounds(&session, ids[1]).width);
    let gap3 = bounds(&session, ids[3]).x - (bounds(&session, ids[2]).x + bounds(&session, ids[2]).width);
    assert!((gap1 - gap2).abs() < 1e-9);
    assert!((gap2 - gap3).abs() < 1e-9);
}

#[test]
fn structural_group_moves_as_one_logical_arrange_item_and_child_ids_are_rejected() {
    let (mut session, ids) = fixture();
    let group_id = ElementId::new();
    assert!(session.execute(EditCommand::GroupElements {
        group_id,
        element_ids: vec![ids[0], ids[1]],
        name: "Pair".to_owned(),
    }).unwrap());
    let child_before = bounds(&session, ids[0]);
    assert!(session.execute(EditCommand::ArrangeElements {
        element_ids: vec![group_id, ids[3]],
        operation: ArrangeOperation::AlignRight,
    }).unwrap());
    let group = bounds(&session, group_id);
    let last = bounds(&session, ids[3]);
    assert!(((group.x + group.width) - (last.x + last.width)).abs() < 1e-9);
    assert!(bounds(&session, ids[0]).x > child_before.x);

    let history = session.current_history_state();
    let error = session.execute(EditCommand::ArrangeElements {
        element_ids: vec![ids[0], ids[3]],
        operation: ArrangeOperation::AlignTop,
    }).unwrap_err();
    assert!(matches!(error, EditorError::ArrangeRequiresTopLevelElement(id) if id == ids[0]));
    assert_eq!(session.current_history_state(), history);
}

#[test]
fn arrange_enforces_operation_selection_minimum() {
    let (mut session, ids) = fixture();
    let error = session.execute(EditCommand::ArrangeElements {
        element_ids: vec![ids[0], ids[1]],
        operation: ArrangeOperation::DistributeVertical,
    }).unwrap_err();
    assert!(matches!(error, EditorError::ArrangeRequiresAtLeast { required: 3, actual: 2 }));
}
''', encoding="utf-8")

app_test = Path("crates/app-core/tests/arrange_application.rs")
app_test.parent.mkdir(parents=True, exist_ok=True)
app_test.write_text(r'''use app_core::{ApplicationSession, ArrangeOperation};
use ddnx::PackageLimits;
use next_domain::{
    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
    ElementKind, Layer, LayerId, NextArtifact, Page, PageId, Rect, Scene, Size,
};

fn rectangle(id: ElementId, x: f64, y: f64, width: f64) -> Element {
    Element {
        id,
        name: String::new(),
        bounds_mm: Rect { x, y, width, height: 10.0 },
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text: None,
        kind: ElementKind::Rectangle { corner_radius_mm: 0.0 },
        import: None,
    }
}

fn fixture() -> (ApplicationSession, [ElementId; 3]) {
    let ids = [ElementId::new(), ElementId::new(), ElementId::new()];
    let document = Document {
        id: DocumentId::new(),
        name: "Arrange application".to_owned(),
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
            name: "Page 1".to_owned(),
            size_mm: Size { width: 210.0, height: 297.0 },
            layers: vec![Layer {
                id: LayerId::new(),
                name: "Layer 1".to_owned(),
                visible: true,
                locked: false,
                draw_color: None,
                scene: Scene {
                    roots: ids.to_vec(),
                    elements: vec![
                        rectangle(ids[0], 10.0, 10.0, 10.0),
                        rectangle(ids[1], 50.0, 30.0, 20.0),
                        rectangle(ids[2], 100.0, 60.0, 10.0),
                    ],
                },
            }],
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    };
    (ApplicationSession::from_artifact(NextArtifact::document(document)).unwrap(), ids)
}

fn x(app: &ApplicationSession, id: ElementId) -> f64 {
    app.session().document().pages[0].layers[0]
        .scene
        .elements
        .iter()
        .find(|element| element.id == id)
        .unwrap()
        .bounds_mm.x
}

#[test]
fn arrange_round_trips_through_application_history_and_ddnx() {
    let (mut app, ids) = fixture();
    let initial = app.session().current_history_state();
    assert!(app.arrange_elements(ids.to_vec(), ArrangeOperation::AlignHorizontalCenter).unwrap());
    let center0 = x(&app, ids[0]) + 5.0;
    let center1 = x(&app, ids[1]) + 10.0;
    let center2 = x(&app, ids[2]) + 5.0;
    assert!((center0 - center1).abs() < 1e-9);
    assert!((center1 - center2).abs() < 1e-9);
    let arranged = app.session().current_history_state();
    assert_ne!(arranged, initial);

    let prepared = app.prepare_document_save(PackageLimits::default()).unwrap();
    let reopened = ApplicationSession::from_ddnx_bytes(prepared.bytes(), PackageLimits::default()).unwrap();
    assert_eq!(x(&reopened, ids[0]), x(&app, ids[0]));
    assert_eq!(x(&reopened, ids[1]), x(&app, ids[1]));
    assert_eq!(x(&reopened, ids[2]), x(&app, ids[2]));

    assert!(app.undo().unwrap());
    assert_eq!(app.session().current_history_state(), initial);
    assert_eq!(x(&app, ids[0]), 10.0);
    assert_eq!(x(&app, ids[1]), 50.0);
    assert!(app.redo().unwrap());
    assert_eq!(app.session().current_history_state(), arranged);
}
''', encoding="utf-8")
