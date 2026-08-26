from pathlib import Path

path = Path("apps/desktop/src-tauri/src/clipboard.rs")
text = path.read_text(encoding="utf-8")

text = text.replace(
'''    MissingElement(ElementId),
    GroupRequiresTopLevel(ElementId),
    GroupCycle(ElementId),
    OverlappingSelection(ElementId),
''',
'''    MissingElement(ElementId),
    SelectionRequiresTopLevel(ElementId),
    SelectionSpansScenes,
    GroupCycle(ElementId),
    OverlappingSelection(ElementId),
''',
)
text = text.replace(
'''            Self::GroupRequiresTopLevel(element_id) => write!(
                formatter,
                "Structural group {element_id:?} must be a top-level scene root before it can be copied."
            ),
''',
'''            Self::SelectionRequiresTopLevel(element_id) => write!(
                formatter,
                "Selected element {element_id:?} must be a top-level scene root before it can be copied."
            ),
            Self::SelectionSpansScenes => formatter.write_str(
                "Clipboard selection must stay within one scene/layer."
            ),
''',
)

old_capture = '''    let selected_set: BTreeSet<_> = selected.iter().copied().collect();
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
'''

new_capture = '''    let selected_set: BTreeSet<_> = selected.iter().copied().collect();
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
'''

if old_capture not in text:
    raise SystemExit("capture_selection block not found")
text = text.replace(old_capture, new_capture)

text = text.replace(
'''            capture_selection(&document, &[inner]),
            Err(ClipboardError::GroupRequiresTopLevel(id)) if id == inner
        ));
    }
}
''',
'''            capture_selection(&document, &[inner]),
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
''',
)

if "SelectionSpansScenes" not in text or "ordinary_group_child_selection_is_rejected_as_non_top_level" not in text:
    raise SystemExit("test/error patch did not apply")

path.write_text(text, encoding="utf-8")
