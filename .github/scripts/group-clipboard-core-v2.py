from pathlib import Path


def replace_once(path, old, new):
    p = Path(path)
    text = p.read_text()
    if old not in text:
        raise SystemExit(f"missing anchor in {path}: {old[:100]!r}")
    p.write_text(text.replace(old, new, 1))


replace_once("crates/editor-core/src/lib.rs", '''    GroupElements {
        group_id: ElementId,
        element_ids: Vec<ElementId>,
        name: String,
    },
    /// Remove one structural group''', '''    GroupElements {
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
    /// Remove one structural group''')

replace_once("crates/editor-core/src/lib.rs", '''        EditCommand::GroupElements {
            group_id,
            element_ids,
            name,
        } => apply_group_elements(document, *group_id, element_ids, name),
        EditCommand::Ungroup { group_id } => apply_ungroup(document, *group_id),''', '''        EditCommand::GroupElements {
            group_id,
            element_ids,
            name,
        } => apply_group_elements(document, *group_id, element_ids, name),
        EditCommand::CreateStructuralGroup {
            target,
            group,
            z_index,
        } => apply_create_structural_group(document, *target, group, *z_index),
        EditCommand::Ungroup { group_id } => apply_ungroup(document, *group_id),''')

p = Path("crates/editor-core/src/lib.rs")
t = p.read_text()
anchor = "fn apply_ungroup(\n    document: &mut Document,\n    group_id: ElementId,\n) -> Result<Option<AppliedCommand>, EditorError> {"
impl = r'''fn apply_create_structural_group(
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
            return Err(EditorError::InvalidZOrderIndex { index: insertion, len: layer.scene.roots.len() });
        }
        let previous_siblings = layer.scene.roots.clone();
        let mut replacement = previous_siblings.clone();
        replacement.insert(insertion, group.id);
        let layer = find_layer_mut(document, target).ok_or(EditorError::HistoryInvariantViolation)?;
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
    if children.iter().copied().any(|child_id| layer_target_for_element(document, child_id) != Some(target)) {
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
    let siblings = owner_siblings(scene, owner)
        .ok_or(EditorError::AmbiguousElementOwnership(children[0]))?;
    let selected: BTreeSet<_> = children.iter().copied().collect();
    if selected.len() != children.len() {
        let duplicate = children.iter().copied()
            .find(|id| children.iter().filter(|candidate| **candidate == *id).count() > 1)
            .expect("duplicate group child must exist");
        return Err(EditorError::DuplicateCommandElement(duplicate));
    }
    let positions: Vec<_> = siblings.iter().enumerate()
        .filter_map(|(index, id)| selected.contains(id).then_some(index)).collect();
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
    *owner_siblings_mut(&mut layer.scene, owner).ok_or(EditorError::HistoryInvariantViolation)? = replacement;
    layer.scene.elements.push(group.clone());
    Ok(Some(AppliedCommand {
        undo: UndoStep::RemoveCreatedGroup { target, owner, previous_siblings, group_id: group.id },
        structural: true,
    }))
}

'''
if anchor not in t:
    raise SystemExit("group reconstruction function anchor missing")
t = t.replace(anchor, impl + anchor, 1)

test_anchor = "    #[test]\n    fn appearance_edit_uses_element_owned_style_and_is_one_undoable_step() {"
test = r'''    #[test]
    fn structural_group_reconstruction_supports_singleton_and_empty_snapshots() {
        let (mut session, first, second, _, _) = fixture(false);
        let target = session.active_layer().unwrap();
        let singleton_id = ElementId::new();
        let mut singleton = element(singleton_id, 3.0, 4.0);
        singleton.name = "Imported singleton".to_owned();
        singleton.bounds_mm = Rect { x: 2.0, y: 1.0, width: 17.0, height: 9.0 };
        singleton.kind = ElementKind::Group { children: vec![first] };
        assert!(session.execute(EditCommand::CreateStructuralGroup {
            target,
            group: singleton.clone(),
            z_index: None,
        }).unwrap());
        assert_eq!(roots(&session, target), vec![singleton_id, second]);
        assert_eq!(find_element(session.document(), singleton_id).unwrap(), &singleton);

        let empty_id = ElementId::new();
        let mut empty = element(empty_id, 9.0, 11.0);
        empty.name = "Imported empty".to_owned();
        empty.kind = ElementKind::Group { children: Vec::new() };
        assert!(session.execute(EditCommand::CreateStructuralGroup {
            target,
            group: empty.clone(),
            z_index: Some(0),
        }).unwrap());
        assert_eq!(roots(&session, target), vec![empty_id, singleton_id, second]);
        assert_eq!(find_element(session.document(), empty_id).unwrap(), &empty);
        assert!(session.undo().unwrap());
        assert!(session.undo().unwrap());
        assert_eq!(roots(&session, target), vec![first, second]);
        assert!(session.redo().unwrap());
        assert!(session.redo().unwrap());
        assert_eq!(roots(&session, target), vec![empty_id, singleton_id, second]);
    }

'''
if test_anchor not in t:
    raise SystemExit("editor-core test anchor missing")
p.write_text(t.replace(test_anchor, test + test_anchor, 1))
