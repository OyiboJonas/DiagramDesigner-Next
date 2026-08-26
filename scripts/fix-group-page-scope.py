from pathlib import Path


def replace_once(path, old, new):
    file = Path(path)
    text = file.read_text(encoding='utf-8')
    count = text.count(old)
    if count != 1:
        raise SystemExit(f'{path}: marker count={count}')
    file.write_text(text.replace(old, new, 1), encoding='utf-8')

replace_once(
    'apps/desktop/src-tauri/src/grouping.rs',
    '''pub(crate) fn selection_groups(document: &Document) -> Vec<SelectionGroupSnapshot> {
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
''',
    '''pub(crate) fn selection_groups(
    document: &Document,
    page_id: PageId,
) -> Vec<SelectionGroupSnapshot> {
    let mut groups = Vec::new();
    for layer in document.master_layers.iter().filter(|layer| layer.visible) {
        collect_layer_selection_groups(layer, &mut groups);
    }
    if let Some(page) = document.pages.iter().find(|page| page.id == page_id) {
        for layer in page.layers.iter().filter(|layer| layer.visible) {
            collect_layer_selection_groups(layer, &mut groups);
        }
    }
    groups
}
''',
)

replace_once(
    'apps/desktop/src-tauri/src/grouping.rs',
    '''    fn top_level_group_maps_to_rendered_leaf_descendants() {
        let (document, _, _, ids) = fixture(false);
        let groups = selection_groups(&document);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].group_id, ids[0]);
        assert_eq!(groups[0].leaf_element_ids, vec![ids[1], ids[2]]);
    }

    #[test]
    fn capabilities_require_mutable_contiguous_top_level_selection() {
''',
    '''    fn top_level_group_maps_to_rendered_leaf_descendants() {
        let (document, page_id, _, ids) = fixture(false);
        let groups = selection_groups(&document, page_id);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].group_id, ids[0]);
        assert_eq!(groups[0].leaf_element_ids, vec![ids[1], ids[2]]);
    }

    #[test]
    fn selection_groups_are_scoped_to_master_and_requested_page() {
        let (mut document, page_id, _, ids) = fixture(false);
        let other_page_id = PageId::new();
        let other_group_id = ElementId::new();
        let other_leaf_id = ElementId::new();
        document.pages.push(Page {
            id: other_page_id,
            name: "Other page".to_owned(),
            size_mm: Size {
                width: 210.0,
                height: 297.0,
            },
            layers: vec![Layer {
                id: LayerId::new(),
                name: "Other layer".to_owned(),
                visible: true,
                locked: false,
                draw_color: None,
                scene: Scene {
                    roots: vec![other_group_id],
                    elements: vec![
                        element(
                            other_group_id,
                            80.0,
                            ElementKind::Group {
                                children: vec![other_leaf_id],
                            },
                        ),
                        element(other_leaf_id, 80.0, ElementKind::Ellipse),
                    ],
                },
            }],
        });

        let active_groups = selection_groups(&document, page_id);
        assert_eq!(active_groups.len(), 1);
        assert_eq!(active_groups[0].group_id, ids[0]);
        assert!(active_groups.iter().all(|group| group.group_id != other_group_id));

        let other_groups = selection_groups(&document, other_page_id);
        assert_eq!(other_groups.len(), 1);
        assert_eq!(other_groups[0].group_id, other_group_id);
    }

    #[test]
    fn capabilities_require_mutable_contiguous_top_level_selection() {
''',
)

replace_once(
    'apps/desktop/src-tauri/src/lib.rs',
    'let selection_groups: Vec<_> = grouping::selection_groups(session.document())',
    'let selection_groups: Vec<_> = grouping::selection_groups(session.document(), page_id)',
)

print('group selection presentation scoped to active page')
