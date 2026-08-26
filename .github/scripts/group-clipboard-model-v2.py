from pathlib import Path

p = Path("apps/desktop/src-tauri/src/clipboard.rs")
t = p.read_text()
replacements = [
("struct ClipboardGroupPayload {\n    id: ElementId,\n    name: String,\n    children: Vec<ElementId>,\n}", "struct ClipboardGroupPayload {\n    element: Element,\n    z_index: Option<usize>,\n}"),
("pub struct ClipboardGroupInstantiation {\n    pub group_id: ElementId,\n    pub child_ids: Vec<ElementId>,\n    pub name: String,\n}", "pub struct ClipboardGroupInstantiation {\n    pub element: Element,\n    pub z_index: Option<usize>,\n}"),
("    GroupRequiresAtLeastTwoChildren(ElementId),\n", ""),
('''            Self::GroupRequiresAtLeastTwoChildren(element_id) => write!(
                formatter,
                "Structural group {element_id:?} cannot be reconstructed because it has fewer than two children."
            ),
''', ''),
('''        for group in &self.groups {
            element_ids.insert(group.id, ElementId::new());
        }
''', '''        for group in &self.groups {
            element_ids.insert(group.element.id, ElementId::new());
            for port in &group.element.ports {
                port_ids.insert(port.id, PortId::new());
            }
        }
'''),
('''        let groups = self
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
''', '''        let groups = self
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
'''),
('''    let mut source_ids = Vec::new();
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
''', '''    let mut source_ids = Vec::new();
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
'''),
('''    source_ids: &mut Vec<ElementId>,
) -> Result<(), ClipboardError> {''', '''    source_ids: &mut Vec<ElementId>,
    initial_order: &mut Vec<ElementId>,
) -> Result<(), ClipboardError> {'''),
('''        ElementKind::Group { children } => {
            if children.len() < 2 {
                return Err(ClipboardError::GroupRequiresAtLeastTwoChildren(element_id));
            }
            for child_id in children {
                collect_subtree(
                    *child_id, by_id, captured, visiting, elements, groups, source_ids,
                )?;
            }
            groups.push(ClipboardGroupPayload {
                id: element.id,
                name: element.name.clone(),
                children: children.clone(),
            });
        }
        _ => elements.push(element.clone()),
''', '''        ElementKind::Group { children } => {
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
'''),
("assert_eq!(payload.groups[0].id, inner_group);", "assert_eq!(payload.groups[0].element.id, inner_group);"),
("assert_eq!(payload.groups[1].id, outer_group);", "assert_eq!(payload.groups[1].element.id, outer_group);"),
]
for old,new in replacements:
    if old not in t:
        raise SystemExit(f"clipboard anchor missing: {old[:100]!r}")
    t=t.replace(old,new,1)
old='''        assert_eq!(instantiated.groups[0].group_id, copied_inner);
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
'''
new='''        assert_eq!(instantiated.groups[0].element.id, copied_inner);
        let ElementKind::Group { children } = &instantiated.groups[0].element.kind else { panic!("inner group") };
        assert_eq!(children, &vec![copied_first, copied_connector_id]);
        assert_eq!(instantiated.groups[0].element.name, "Inner");
        assert_eq!(instantiated.groups[0].z_index, None);
        assert_eq!(instantiated.groups[1].element.id, copied_outer);
        let ElementKind::Group { children } = &instantiated.groups[1].element.kind else { panic!("outer group") };
        assert_eq!(children, &vec![copied_inner, copied_second]);
        assert_eq!(instantiated.groups[1].element.name, "Outer");
        assert_eq!(instantiated.groups[1].z_index, None);
'''
if old not in t: raise SystemExit("group test mapping anchor missing")
t=t.replace(old,new,1)
anchor="    #[test]\n    fn nested_group_selection_is_rejected_until_its_top_level_owner_is_selected() {"
extra=r'''    #[test]
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
        let ElementKind::Group { children } = &instantiated.groups[1].element.kind else { panic!("singleton") };
        assert_eq!(children, &vec![copied_leaf]);
        let ElementKind::Group { children } = &instantiated.groups[2].element.kind else { panic!("outer") };
        assert_eq!(children, &vec![copied_empty, copied_singleton]);
    }

'''
if anchor not in t: raise SystemExit("clipboard test insertion anchor missing")
p.write_text(t.replace(anchor,extra+anchor,1))
