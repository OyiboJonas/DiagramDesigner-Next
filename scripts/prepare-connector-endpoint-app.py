from pathlib import Path


def read(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    Path(path).write_text(text, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"Expected one anchor in {path}, found {count}: {old[:120]!r}")
    write(path, text.replace(old, new, 1))


path = "crates/app-core/src/lib.rs"
replace_once(
    path,
    """use editor_core::{\n    EditCommand, EditTransaction, EditorError, EditorSession, HistoryStateId, LayerScope,\n    LayerTarget,\n};\n""",
    """use editor_core::{\n    ConnectorEndpointSide as CoreConnectorEndpointSide, EditCommand, EditTransaction, EditorError,\n    EditorSession, HistoryStateId, LayerScope, LayerTarget,\n};\n""",
)
replace_once(
    path,
    """use next_domain::{\n    Color, Element, ElementId, Layer, LayerId, NextArtifact, Page, PageId, Point, Rect, Size,\n    TextBlock,\n};\n""",
    """use next_domain::{\n    Color, Connection, Element, ElementId, Layer, LayerId, NextArtifact, Page, PageId, Point, Rect,\n    Size, TextBlock,\n};\n""",
)
replace_once(
    path,
    """const INITIAL_DOCUMENT_GENERATION: u64 = 1;\n\n#[derive(Debug, Error)]\npub enum ApplicationError {\n""",
    """const INITIAL_DOCUMENT_GENERATION: u64 = 1;\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ConnectorEndpointSide {\n    Start,\n    End,\n}\n\nimpl From<ConnectorEndpointSide> for CoreConnectorEndpointSide {\n    fn from(value: ConnectorEndpointSide) -> Self {\n        match value {\n            ConnectorEndpointSide::Start => Self::Start,\n            ConnectorEndpointSide::End => Self::End,\n        }\n    }\n}\n\n#[derive(Debug, Error)]\npub enum ApplicationError {\n""",
)
replace_once(
    path,
    """    /// Delete a selection as one semantic history step.\n    pub fn delete_elements(\n""",
    """    /// Commit one connector endpoint as either a free point or a durable\n    /// target-port reference. Connected coordinates are resolved by editor-core.\n    pub fn set_connector_endpoint(\n        &mut self,\n        element_id: ElementId,\n        side: ConnectorEndpointSide,\n        position_mm: Point,\n        connection: Option<Connection>,\n    ) -> Result<bool, ApplicationError> {\n        self.execute_edit(EditCommand::SetConnectorEndpoint {\n            element_id,\n            side: side.into(),\n            position_mm,\n            connection,\n        })\n    }\n\n    /// Delete a selection as one semantic history step.\n    pub fn delete_elements(\n""",
)

# Extend test imports and add one focused application-boundary test.
replace_once(
    path,
    """    use next_domain::{\n        AnchorSet, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element,\n        ElementKind, Layer, LayerId, Page, PageId, Rect, Scene, Size,\n    };\n""",
    """    use next_domain::{\n        AnchorSet, Connection, Connector, ConnectorLabelStyle, Document, DocumentDefaults,\n        DocumentId, Element, ElementKind, Endpoint, Layer, LayerId, LineStyle, MarkerStyle,\n        NormalizedPoint, Page, PageId, Port, PortId, Rect, Scene, Size,\n    };\n""",
)
replace_once(
    path,
    """    #[test]\n    fn move_commit_marks_document_dirty_and_drives_recovery() {\n""",
    """    #[test]\n    fn connector_endpoint_edit_uses_application_semantic_boundary() {\n        let source_id = ElementId::new();\n        let target_id = ElementId::new();\n        let port_id = PortId::new();\n        let page_id = PageId::new();\n        let layer_id = LayerId::new();\n        let source = Element {\n            id: source_id,\n            name: \"Connector\".to_owned(),\n            bounds_mm: Rect {\n                x: 0.0,\n                y: 0.0,\n                width: 10.0,\n                height: 10.0,\n            },\n            rotation_deg: 0.0,\n            anchors: AnchorSet::default(),\n            ports: Vec::new(),\n            style_id: None,\n            text: None,\n            kind: ElementKind::StraightConnector {\n                connector: Connector {\n                    start: Endpoint {\n                        position_mm: Point { x: 1.0, y: 1.0 },\n                        connection: None,\n                    },\n                    end: Endpoint {\n                        position_mm: Point { x: 9.0, y: 9.0 },\n                        connection: None,\n                    },\n                    start_marker: MarkerStyle::None,\n                    end_marker: MarkerStyle::None,\n                    line_style: LineStyle::Solid,\n                    secondary_color: None,\n                },\n            },\n            import: None,\n        };\n        let target = Element {\n            id: target_id,\n            name: \"Target\".to_owned(),\n            bounds_mm: Rect {\n                x: 20.0,\n                y: 30.0,\n                width: 10.0,\n                height: 10.0,\n            },\n            rotation_deg: 0.0,\n            anchors: AnchorSet::default(),\n            ports: vec![Port {\n                id: port_id,\n                index: 0,\n                position: NormalizedPoint { x: 1.0, y: 0.5 },\n            }],\n            style_id: None,\n            text: None,\n            kind: ElementKind::Rectangle {\n                corner_radius_mm: 0.0,\n            },\n            import: None,\n        };\n        let document = Document {\n            id: DocumentId::new(),\n            name: \"Connector application test\".to_owned(),\n            defaults: defaults(),\n            master_layers: Vec::new(),\n            pages: vec![Page {\n                id: page_id,\n                name: \"Page\".to_owned(),\n                size_mm: Size {\n                    width: 210.0,\n                    height: 297.0,\n                },\n                layers: vec![Layer {\n                    id: layer_id,\n                    name: \"Layer\".to_owned(),\n                    visible: true,\n                    locked: false,\n                    draw_color: None,\n                    scene: Scene {\n                        roots: vec![source_id, target_id],\n                        elements: vec![source, target],\n                    },\n                }],\n            }],\n            styles: Vec::new(),\n            assets: Vec::new(),\n            import: None,\n        };\n        let mut application =\n            ApplicationSession::from_artifact(NextArtifact::document(document)).unwrap();\n        application\n            .set_connector_endpoint(\n                source_id,\n                ConnectorEndpointSide::Start,\n                Point { x: -1.0, y: -1.0 },\n                Some(Connection {\n                    element_id: target_id,\n                    port_id,\n                }),\n            )\n            .unwrap();\n\n        let source = &application.session().document().pages[0].layers[0].scene.elements[0];\n        let ElementKind::StraightConnector { connector } = &source.kind else {\n            panic!(\"expected straight connector\")\n        };\n        assert_eq!(connector.start.connection.unwrap().element_id, target_id);\n        assert_eq!(connector.start.position_mm, Point { x: 30.0, y: 35.0 });\n        assert!(application.is_dirty());\n    }\n\n    #[test]\n    fn move_commit_marks_document_dirty_and_drives_recovery() {\n""",
)
