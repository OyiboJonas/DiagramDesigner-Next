use next_domain::{
    AnchorSet, Connector, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element,
    ElementId, ElementKind, Endpoint, Layer, LayerId, LineStyle, MarkerStyle, Page, Point, Rect,
    Scene, Size,
};
use render_plan::{PreparedPage, RenderPlanOptions, build_page_plan};

fn orthogonal(id: ElementId) -> Element {
    Element {
        id,
        name: String::new(),
        bounds_mm: Rect {
            x: 20.0,
            y: 30.0,
            width: 0.0,
            height: 40.0,
        },
        rotation_deg: 0.0,
        anchors: AnchorSet::default(),
        ports: Vec::new(),
        style_id: None,
        text: None,
        kind: ElementKind::OrthogonalConnector {
            connector: Connector {
                start: Endpoint {
                    position_mm: Point { x: 20.0, y: 30.0 },
                    connection: None,
                },
                end: Endpoint {
                    position_mm: Point { x: 20.0, y: 70.0 },
                    connection: None,
                },
                start_marker: MarkerStyle::Arrow3,
                end_marker: MarkerStyle::Arrow3,
                line_style: LineStyle::Solid,
                secondary_color: None,
            },
            corner_radius_mm: 0.0,
        },
        import: None,
    }
}

fn document(element: Element) -> (Document, next_domain::PageId) {
    let page_id = next_domain::PageId::new();
    (
        Document {
            id: DocumentId::new(),
            name: "Orthogonal culling regression".to_owned(),
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
                    id: LayerId::new(),
                    name: "Layer".to_owned(),
                    visible: true,
                    locked: false,
                    draw_color: None,
                    scene: Scene {
                        roots: vec![element.id],
                        elements: vec![element],
                    },
                }],
            }],
            styles: Vec::new(),
            assets: Vec::new(),
            import: None,
        },
        page_id,
    )
}

#[test]
fn cold_and_prepared_plans_keep_marker_clearance_outside_endpoint_bounds() {
    let id = ElementId::new();
    let (document, page_id) = document(orthogonal(id));
    let options = RenderPlanOptions {
        viewport_mm: Some(Rect {
            x: 19.0,
            y: 26.5,
            width: 2.0,
            height: 1.0,
        }),
        cull_margin_mm: 0.0,
    };

    let cold = build_page_plan(&document, page_id, options).unwrap();
    let prepared = PreparedPage::build(&document, page_id).unwrap();
    let hot = prepared.query(options).unwrap();

    assert_eq!(cold.items.len(), 1);
    assert_eq!(cold.items[0].element.id, id);
    assert_eq!(hot.items.len(), 1);
    assert_eq!(hot.items[0].element.id, id);
    assert_eq!(cold.culled_elements, hot.culled_elements);
}
