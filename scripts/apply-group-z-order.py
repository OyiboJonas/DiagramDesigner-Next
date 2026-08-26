from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one marker, found {count}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


def append_once(path: str, marker: str, content: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    if content.strip() in text:
        raise SystemExit(f"{path}: content already present")
    if text.count(marker) != 1:
        raise SystemExit(f"{path}: append marker count={text.count(marker)}")
    file.write_text(text.replace(marker, content + marker, 1), encoding="utf-8")


# editor-core: a structural group is eligible when it is itself a direct scene root.
replace_once(
    "crates/editor-core/src/lib.rs",
    '''    #[error("structural group {0:?} requires the dedicated grouping workflow for z-order changes")]
    GroupZOrderUnsupported(ElementId),
''',
    '',
)
replace_once(
    "crates/editor-core/src/lib.rs",
    '''        target = Some(element_target);
        let element =
            find_element(document, *element_id).ok_or(EditorError::ElementNotFound(*element_id))?;
        if matches!(&element.kind, ElementKind::Group { .. }) {
            return Err(EditorError::GroupZOrderUnsupported(*element_id));
        }
''',
    '''        target = Some(element_target);
''',
)

old_test = '''    #[test]
    fn z_order_rejects_cross_layer_and_group_owned_mutation() {
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
        assert!(matches!(
            session.execute(EditCommand::ReorderElements {
                element_ids: vec![group_id],
                operation: ZOrderOperation::SendToBack,
            }),
            Err(EditorError::GroupZOrderUnsupported(id)) if id == group_id
        ));
        assert!(matches!(
            session.execute(EditCommand::ReorderElements {
                element_ids: vec![first],
                operation: ZOrderOperation::SendToBack,
            }),
            Err(EditorError::ZOrderRequiresTopLevelElement(id)) if id == first
        ));
        assert_eq!(session.current_history_state(), grouped_history);
    }
'''
new_test = '''    #[test]
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
'''
replace_once("crates/editor-core/src/lib.rs", old_test, new_test)

# app-core regression: group structure survives reorder/history/DDNX.
app_test = '''
#[test]
fn structural_group_z_order_round_trips_without_changing_children() {
    let (artifact, ids) = fixture();
    let mut app = ApplicationSession::from_artifact(artifact).unwrap();
    let group_id = ElementId::new();
    assert!(
        app.group_elements(group_id, vec![ids[0], ids[1]], "Pair".to_owned())
            .unwrap()
    );
    assert_eq!(roots(&app), vec![group_id, ids[2]]);

    let group_children = |app: &ApplicationSession| {
        let group = app.session().document().pages[0].layers[0]
            .scene
            .elements
            .iter()
            .find(|element| element.id == group_id)
            .unwrap();
        let ElementKind::Group { children } = &group.kind else {
            panic!("expected structural group")
        };
        children.clone()
    };
    assert_eq!(group_children(&app), vec![ids[0], ids[1]]);
    let grouped_history = app.session().current_history_state();

    assert!(
        app.reorder_elements(vec![group_id], ZOrderOperation::BringToFront)
            .unwrap()
    );
    assert_eq!(roots(&app), vec![ids[2], group_id]);
    assert_eq!(group_children(&app), vec![ids[0], ids[1]]);
    let reordered_history = app.session().current_history_state();
    assert_ne!(reordered_history, grouped_history);

    let prepared = app.prepare_document_save(PackageLimits::default()).unwrap();
    let reopened =
        ApplicationSession::from_ddnx_bytes(prepared.bytes(), PackageLimits::default()).unwrap();
    assert_eq!(roots(&reopened), vec![ids[2], group_id]);
    assert_eq!(group_children(&reopened), vec![ids[0], ids[1]]);

    assert!(app.undo().unwrap());
    assert_eq!(app.session().current_history_state(), grouped_history);
    assert_eq!(roots(&app), vec![group_id, ids[2]]);
    assert_eq!(group_children(&app), vec![ids[0], ids[1]]);
    assert!(app.redo().unwrap());
    assert_eq!(app.session().current_history_state(), reordered_history);
    assert_eq!(roots(&app), vec![ids[2], group_id]);
    assert_eq!(group_children(&app), vec![ids[0], ids[1]]);

    let before_noop = app.session().current_history_state();
    assert!(
        !app.reorder_elements(vec![group_id], ZOrderOperation::BringToFront)
            .unwrap()
    );
    assert_eq!(app.session().current_history_state(), before_noop);
}

'''
append_once(
    "crates/app-core/tests/z_order_application.rs",
    "#[test]\nfn z_order_round_trips_through_application_history_and_ddnx()",
    app_test,
)

# Desktop policy: group identity no longer suppresses root z-order actions.
replace_once(
    "apps/desktop/ui/app.js",
    '''function updateZOrderActionState() {
  const selectionCount = Number(currentSelectionProperties?.count ?? 0);
  const activeLayer = activeLayerForZOrder();
  const containsGroup = currentSelectionProperties?.containsGroup === true;
  const enabled =
    !containsGroup &&
    isZOrderActionEnabled({
      selectionCount,
      layerVisible: activeLayer?.visible === true,
      layerLocked: activeLayer?.locked !== false,
      busy: isBusy,
    });
  const reason = isBusy
    ? 'Finish the current action first'
    : selectionCount === 0
      ? 'Select one or more elements to arrange them'
      : containsGroup
        ? 'Structural groups keep their current z-order in this slice; ungroup before arranging'
        : !activeLayer?.visible
        ? 'Show the active layer before arranging elements'
        : activeLayer?.locked
          ? 'Unlock the active layer before arranging elements'
          : 'Arrange the current selection';
''',
    '''function updateZOrderActionState() {
  const selectionCount = Number(currentSelectionProperties?.count ?? 0);
  const activeLayer = activeLayerForZOrder();
  const enabled = isZOrderActionEnabled({
    selectionCount,
    layerVisible: activeLayer?.visible === true,
    layerLocked: activeLayer?.locked !== false,
    busy: isBusy,
  });
  const reason = isBusy
    ? 'Finish the current action first'
    : selectionCount === 0
      ? 'Select one or more elements to arrange them'
      : !activeLayer?.visible
        ? 'Show the active layer before arranging elements'
        : activeLayer?.locked
          ? 'Unlock the active layer before arranging elements'
          : 'Arrange the current selection';
''',
)

# Frontend policy regression: an eligible structural-group selection is still enabled.
replace_once(
    "web/editor-interaction/z-order-actions.test.mjs",
    '''  assert.equal(
    isZOrderActionEnabled({ selectionCount: 1, layerVisible: true, layerLocked: false, busy: false }),
    true,
  );
''',
    '''  assert.equal(
    isZOrderActionEnabled({ selectionCount: 1, layerVisible: true, layerLocked: false, busy: false }),
    true,
  );
  assert.equal(
    isZOrderActionEnabled({
      selectionCount: 1,
      layerVisible: true,
      layerLocked: false,
      busy: false,
      containsGroup: true,
    }),
    true,
  );
''',
)
