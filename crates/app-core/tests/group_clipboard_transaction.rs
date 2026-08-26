use app_core::{ApplicationSession, StructuralGroupCreation};
use ddnx::PackageLimits;
use editor_core::LayerTarget;
use next_domain::{
    AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element, ElementId,
    ElementKind, Layer, LayerId, NextArtifact, Page, PageId, Rect, Scene, Size,
};
fn rectangle(id: ElementId, x: f64) -> Element {
    Element {
        id,
        name: "Leaf".to_owned(),
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
fn group(id: ElementId, name: &str, children: Vec<ElementId>, x: f64) -> Element {
    Element {
        id,
        name: name.to_owned(),
        bounds_mm: Rect {
            x,
            y: 12.0,
            width: 19.0,
            height: 13.0,
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
fn fixture() -> (NextArtifact, LayerTarget) {
    let page_id = PageId::new();
    let layer_id = LayerId::new();
    let d = Document {
        id: DocumentId::new(),
        name: "Clipboard groups".to_owned(),
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
                scene: Scene::default(),
            }],
        }],
        styles: Vec::new(),
        assets: Vec::new(),
        import: None,
    };
    (
        NextArtifact::document(d),
        LayerTarget::Page { page_id, layer_id },
    )
}
fn roots(a: &ApplicationSession) -> Vec<ElementId> {
    a.session().document().pages[0].layers[0]
        .scene
        .roots
        .clone()
}
fn children(a: &ApplicationSession, id: ElementId) -> Vec<ElementId> {
    let e = a.session().document().pages[0].layers[0]
        .scene
        .elements
        .iter()
        .find(|e| e.id == id)
        .unwrap();
    let ElementKind::Group { children } = &e.kind else {
        panic!("group")
    };
    children.clone()
}
#[test]
fn clipboard_hierarchy_is_one_transaction_and_round_trips_through_ddnx() {
    let (artifact, target) = fixture();
    let mut a = ApplicationSession::from_artifact(artifact).unwrap();
    let initial = a.session().current_history_state();
    let f = ElementId::new();
    let s = ElementId::new();
    let t = ElementId::new();
    let o = ElementId::new();
    let inner = ElementId::new();
    let outer = ElementId::new();
    assert!(
        a.create_elements_with_groups(
            target,
            vec![
                rectangle(f, 15.0),
                rectangle(s, 40.0),
                rectangle(t, 65.0),
                rectangle(o, 100.0)
            ],
            vec![
                StructuralGroupCreation {
                    element: group(inner, "Inner", vec![f, s], 15.0),
                    z_index: None
                },
                StructuralGroupCreation {
                    element: group(outer, "Outer", vec![inner, t], 15.0),
                    z_index: None
                }
            ],
            Vec::new()
        )
        .unwrap()
    );
    assert_eq!(roots(&a), vec![outer, o]);
    assert_eq!(children(&a, inner), vec![f, s]);
    assert_eq!(children(&a, outer), vec![inner, t]);
    let created = a.session().current_history_state();
    assert_ne!(created, initial);
    let bytes = a.prepare_document_save(PackageLimits::default()).unwrap();
    let r = ApplicationSession::from_ddnx_bytes(bytes.bytes(), PackageLimits::default()).unwrap();
    assert_eq!(roots(&r), vec![outer, o]);
    assert_eq!(children(&r, outer), vec![inner, t]);
    assert!(a.undo().unwrap());
    assert_eq!(a.session().current_history_state(), initial);
    assert!(roots(&a).is_empty());
    assert!(a.redo().unwrap());
    assert_eq!(a.session().current_history_state(), created)
}
#[test]
fn clipboard_preserves_empty_and_singleton_group_snapshots() {
    let (artifact, target) = fixture();
    let mut a = ApplicationSession::from_artifact(artifact).unwrap();
    let leaf = ElementId::new();
    let empty = ElementId::new();
    let single = ElementId::new();
    let outer = ElementId::new();
    let ordinary = ElementId::new();
    let e = group(empty, "Empty", Vec::new(), 4.0);
    let s = group(single, "Singleton", vec![leaf], 20.0);
    let o = group(outer, "Outer", vec![empty, single], 4.0);
    assert!(
        a.create_elements_with_groups(
            target,
            vec![rectangle(leaf, 20.0), rectangle(ordinary, 100.0)],
            vec![
                StructuralGroupCreation {
                    element: e.clone(),
                    z_index: Some(0)
                },
                StructuralGroupCreation {
                    element: s.clone(),
                    z_index: None
                },
                StructuralGroupCreation {
                    element: o.clone(),
                    z_index: None
                }
            ],
            Vec::new()
        )
        .unwrap()
    );
    assert_eq!(roots(&a), vec![outer, ordinary]);
    assert_eq!(children(&a, empty), Vec::<ElementId>::new());
    assert_eq!(children(&a, single), vec![leaf]);
    assert_eq!(children(&a, outer), vec![empty, single]);
    let scene = &a.session().document().pages[0].layers[0].scene;
    assert_eq!(scene.elements.iter().find(|x| x.id == empty).unwrap(), &e);
    assert_eq!(scene.elements.iter().find(|x| x.id == single).unwrap(), &s);
    let bytes = a.prepare_document_save(PackageLimits::default()).unwrap();
    let r = ApplicationSession::from_ddnx_bytes(bytes.bytes(), PackageLimits::default()).unwrap();
    assert_eq!(roots(&r), vec![outer, ordinary]);
    assert_eq!(children(&r, empty), Vec::<ElementId>::new());
    assert_eq!(children(&r, single), vec![leaf])
}
