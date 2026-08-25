from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"missing patch anchor: {label}")
    return text.replace(old, new, 1)


# editor-core: canonical read queries plus connector bounds coherence.
path = Path("crates/editor-core/src/lib.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    """#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub enum ConnectorEndpointSide {\n    Start,\n    End,\n}\n\n""",
    """#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub enum ConnectorEndpointSide {\n    Start,\n    End,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ConnectorGeometryKind {\n    Straight,\n    Orthogonal,\n    Curve,\n}\n\n#[derive(Debug, Clone, PartialEq)]\npub struct ConnectorEndpointSnapshot {\n    pub kind: ConnectorGeometryKind,\n    pub start: Endpoint,\n    pub end: Endpoint,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq)]\npub struct ResolvedPortPosition {\n    pub element_id: ElementId,\n    pub port_id: PortId,\n    pub position_mm: Point,\n}\n\n""",
    "editor public connector query types",
)
text = replace_once(
    text,
    """    pub fn clear_selection(&mut self) {\n        self.selection.clear();\n    }\n\n    pub fn current_history_state(&self) -> HistoryStateId {\n""",
    """    pub fn clear_selection(&mut self) {\n        self.selection.clear();\n    }\n\n    /// Read canonical connector endpoint state without exposing mutable document access.\n    pub fn connector_endpoint_snapshot(\n        &self,\n        element_id: ElementId,\n    ) -> Result<Option<ConnectorEndpointSnapshot>, EditorError> {\n        let element = find_element(&self.document, element_id)\n            .ok_or(EditorError::ElementNotFound(element_id))?;\n        let (kind, connector) = match &element.kind {\n            ElementKind::StraightConnector { connector } => {\n                (ConnectorGeometryKind::Straight, connector)\n            }\n            ElementKind::OrthogonalConnector { connector, .. } => {\n                (ConnectorGeometryKind::Orthogonal, connector)\n            }\n            ElementKind::Curve {\n                connector: Some(connector),\n                ..\n            } => (ConnectorGeometryKind::Curve, connector),\n            _ => return Ok(None),\n        };\n        Ok(Some(ConnectorEndpointSnapshot {\n            kind,\n            start: connector.start.clone(),\n            end: connector.end.clone(),\n        }))\n    }\n\n    /// Resolve every port in one scene to its canonical document-space position.\n    pub fn resolved_ports(\n        &self,\n        target: LayerTarget,\n    ) -> Result<Vec<ResolvedPortPosition>, EditorError> {\n        let layer = find_layer(&self.document, target)\n            .ok_or(EditorError::LayerNotFound(layer_id_of(target)))?;\n        let mut ports = Vec::new();\n        for element in &layer.scene.elements {\n            for port in &element.ports {\n                ports.push(ResolvedPortPosition {\n                    element_id: element.id,\n                    port_id: port.id,\n                    position_mm: port_document_position(element, port.id)?,\n                });\n            }\n        }\n        Ok(ports)\n    }\n\n    pub fn current_history_state(&self) -> HistoryStateId {\n""",
    "editor read query methods",
)
text = replace_once(
    text,
    """    *endpoint = next;\n\n    Ok(Some(AppliedCommand {\n""",
    """    *endpoint = next;\n    refresh_connector_bounds(document, element_id)?;\n\n    Ok(Some(AppliedCommand {\n""",
    "endpoint edit refreshes bounds",
)
text = replace_once(
    text,
    """            *connector_endpoint_mut(\n                connector_mut(element).ok_or(EditorError::HistoryInvariantViolation)?,\n                *side,\n            ) = endpoint.clone();\n            synchronize_connected_endpoints(document)?;\n""",
    """            *connector_endpoint_mut(\n                connector_mut(element).ok_or(EditorError::HistoryInvariantViolation)?,\n                *side,\n            ) = endpoint.clone();\n            refresh_connector_bounds(document, *element_id)?;\n            synchronize_connected_endpoints(document)?;\n""",
    "endpoint undo refreshes bounds",
)
text = replace_once(
    text,
    """    for (element_id, side, position) in updates {\n        let element =\n            find_element_mut(document, element_id).ok_or(EditorError::HistoryInvariantViolation)?;\n        connector_endpoint_mut(\n            connector_mut(element).ok_or(EditorError::HistoryInvariantViolation)?,\n            side,\n        )\n        .position_mm = position;\n    }\n    Ok(())\n}\n\nfn connector_mut(element: &mut Element) -> Option<&mut Connector> {\n""",
    """    let mut touched = BTreeSet::new();\n    for (element_id, side, position) in updates {\n        let element =\n            find_element_mut(document, element_id).ok_or(EditorError::HistoryInvariantViolation)?;\n        connector_endpoint_mut(\n            connector_mut(element).ok_or(EditorError::HistoryInvariantViolation)?,\n            side,\n        )\n        .position_mm = position;\n        touched.insert(element_id);\n    }\n    for element_id in touched {\n        refresh_connector_bounds(document, element_id)?;\n    }\n    Ok(())\n}\n\nfn refresh_connector_bounds(\n    document: &mut Document,\n    element_id: ElementId,\n) -> Result<(), EditorError> {\n    let element =\n        find_element_mut(document, element_id).ok_or(EditorError::HistoryInvariantViolation)?;\n    let (start, end) = {\n        let connector = connector_mut(element).ok_or(EditorError::HistoryInvariantViolation)?;\n        (connector.start.position_mm, connector.end.position_mm)\n    };\n    element.bounds_mm = Rect {\n        x: start.x.min(end.x),\n        y: start.y.min(end.y),\n        width: (start.x - end.x).abs().max(0.1),\n        height: (start.y - end.y).abs().max(0.1),\n    };\n    Ok(())\n}\n\nfn connector_mut(element: &mut Element) -> Option<&mut Connector> {\n""",
    "connected endpoint bounds synchronization",
)
text = replace_once(
    text,
    """        assert_eq!(connected.connection, Some(connection));\n        assert_eq!(connected.position_mm, Point { x: 30.0, y: 32.5 });\n\n        session.undo().unwrap();\n""",
    """        assert_eq!(connected.connection, Some(connection));\n        assert_eq!(connected.position_mm, Point { x: 30.0, y: 32.5 });\n        assert_eq!(\n            bounds(&session, source),\n            Rect {\n                x: 11.0,\n                y: 7.0,\n                width: 19.0,\n                height: 25.5,\n            }\n        );\n\n        let snapshot = session.connector_endpoint_snapshot(source).unwrap().unwrap();\n        assert_eq!(snapshot.kind, ConnectorGeometryKind::Straight);\n        assert_eq!(snapshot.start, connected);\n        let ports = session.resolved_ports(session.active_layer().unwrap()).unwrap();\n        assert!(ports.iter().any(|port|\n            port.element_id == target\n                && port.port_id == port_id\n                && port.position_mm == Point { x: 30.0, y: 32.5 }\n        ));\n\n        session.undo().unwrap();\n""",
    "editor connector read query test",
)
text = replace_once(
    text,
    """        assert_eq!(\n            connector_endpoint_value(&session, source, ConnectorEndpointSide::Start).position_mm,\n            Point { x: 35.0, y: 30.5 }\n        );\n        session.undo().unwrap();\n""",
    """        assert_eq!(\n            connector_endpoint_value(&session, source, ConnectorEndpointSide::Start).position_mm,\n            Point { x: 35.0, y: 30.5 }\n        );\n        assert_eq!(\n            bounds(&session, source),\n            Rect {\n                x: 11.0,\n                y: 7.0,\n                width: 24.0,\n                height: 23.5,\n            }\n        );\n        session.undo().unwrap();\n""",
    "target move refreshes connector bounds test",
)
path.write_text(text, encoding="utf-8")


# app-core: hide editor-core query types and expose desktop-safe semantic snapshots.
path = Path("crates/app-core/src/lib.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    """use editor_core::{\n    ConnectorEndpointSide as CoreConnectorEndpointSide, EditCommand, EditTransaction, EditorError,\n    EditorSession, HistoryStateId, LayerScope, LayerTarget,\n};\n""",
    """use editor_core::{\n    ConnectorEndpointSide as CoreConnectorEndpointSide,\n    ConnectorEndpointSnapshot as CoreConnectorEndpointSnapshot,\n    ConnectorGeometryKind as CoreConnectorGeometryKind, EditCommand, EditTransaction, EditorError,\n    EditorSession, HistoryStateId, LayerScope, LayerTarget,\n    ResolvedPortPosition as CoreResolvedPortPosition,\n};\n""",
    "app-core editor imports",
)
text = replace_once(
    text,
    """    Color, Connection, Element, ElementId, Layer, LayerId, NextArtifact, Page, PageId, Point, Rect,\n    Size, TextBlock,\n""",
    """    Color, Connection, Element, ElementId, Layer, LayerId, NextArtifact, Page, PageId, Point, PortId,\n    Rect, Size, TextBlock,\n""",
    "app-core PortId import",
)
text = replace_once(
    text,
    """impl From<ConnectorEndpointSide> for CoreConnectorEndpointSide {\n    fn from(value: ConnectorEndpointSide) -> Self {\n        match value {\n            ConnectorEndpointSide::Start => Self::Start,\n            ConnectorEndpointSide::End => Self::End,\n        }\n    }\n}\n\n""",
    """impl From<ConnectorEndpointSide> for CoreConnectorEndpointSide {\n    fn from(value: ConnectorEndpointSide) -> Self {\n        match value {\n            ConnectorEndpointSide::Start => Self::Start,\n            ConnectorEndpointSide::End => Self::End,\n        }\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ConnectorGeometryKind {\n    Straight,\n    Orthogonal,\n    Curve,\n}\n\n#[derive(Debug, Clone, PartialEq)]\npub struct ConnectorEndpointState {\n    pub position_mm: Point,\n    pub connection: Option<Connection>,\n}\n\n#[derive(Debug, Clone, PartialEq)]\npub struct ConnectorEndpoints {\n    pub kind: ConnectorGeometryKind,\n    pub start: ConnectorEndpointState,\n    pub end: ConnectorEndpointState,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq)]\npub struct ConnectorPortPosition {\n    pub element_id: ElementId,\n    pub port_id: PortId,\n    pub position_mm: Point,\n}\n\nimpl From<CoreConnectorEndpointSnapshot> for ConnectorEndpoints {\n    fn from(value: CoreConnectorEndpointSnapshot) -> Self {\n        let kind = match value.kind {\n            CoreConnectorGeometryKind::Straight => ConnectorGeometryKind::Straight,\n            CoreConnectorGeometryKind::Orthogonal => ConnectorGeometryKind::Orthogonal,\n            CoreConnectorGeometryKind::Curve => ConnectorGeometryKind::Curve,\n        };\n        Self {\n            kind,\n            start: ConnectorEndpointState {\n                position_mm: value.start.position_mm,\n                connection: value.start.connection,\n            },\n            end: ConnectorEndpointState {\n                position_mm: value.end.position_mm,\n                connection: value.end.connection,\n            },\n        }\n    }\n}\n\nimpl From<CoreResolvedPortPosition> for ConnectorPortPosition {\n    fn from(value: CoreResolvedPortPosition) -> Self {\n        Self {\n            element_id: value.element_id,\n            port_id: value.port_id,\n            position_mm: value.position_mm,\n        }\n    }\n}\n\n""",
    "app-core connector query DTOs",
)
text = replace_once(
    text,
    """    pub fn set_connector_endpoint(\n        &mut self,\n        element_id: ElementId,\n        side: ConnectorEndpointSide,\n        position_mm: Point,\n        connection: Option<Connection>,\n    ) -> Result<bool, ApplicationError> {\n        self.execute_edit(EditCommand::SetConnectorEndpoint {\n            element_id,\n            side: side.into(),\n            position_mm,\n            connection,\n        })\n    }\n\n    /// Delete a selection as one semantic history step.\n""",
    """    pub fn set_connector_endpoint(\n        &mut self,\n        element_id: ElementId,\n        side: ConnectorEndpointSide,\n        position_mm: Point,\n        connection: Option<Connection>,\n    ) -> Result<bool, ApplicationError> {\n        self.execute_edit(EditCommand::SetConnectorEndpoint {\n            element_id,\n            side: side.into(),\n            position_mm,\n            connection,\n        })\n    }\n\n    /// Read one connector's canonical endpoint state without exposing editor-core types.\n    pub fn connector_endpoints(\n        &self,\n        element_id: ElementId,\n    ) -> Result<Option<ConnectorEndpoints>, ApplicationError> {\n        Ok(self\n            .runtime\n            .session()\n            .connector_endpoint_snapshot(element_id)?\n            .map(ConnectorEndpoints::from))\n    }\n\n    /// Return hit-testable ports only for the active visible, unlocked page-local layer.\n    pub fn active_page_layer_ports(\n        &self,\n    ) -> Result<Vec<ConnectorPortPosition>, ApplicationError> {\n        let session = self.runtime.session();\n        let Some(page_id) = session.active_page_id() else {\n            return Ok(Vec::new());\n        };\n        let Some(LayerTarget::Page {\n            page_id: layer_page_id,\n            layer_id,\n        }) = session.active_layer()\n        else {\n            return Ok(Vec::new());\n        };\n        if layer_page_id != page_id {\n            return Ok(Vec::new());\n        }\n        let page = session\n            .document()\n            .pages\n            .iter()\n            .find(|page| page.id == page_id)\n            .ok_or(EditorError::PageNotFound(page_id))?;\n        let layer = page\n            .layers\n            .iter()\n            .find(|layer| layer.id == layer_id)\n            .ok_or(EditorError::LayerNotFound(layer_id))?;\n        if !layer.visible || layer.locked {\n            return Ok(Vec::new());\n        }\n        Ok(session\n            .resolved_ports(LayerTarget::Page { page_id, layer_id })?\n            .into_iter()\n            .map(ConnectorPortPosition::from)\n            .collect())\n    }\n\n    /// Delete a selection as one semantic history step.\n""",
    "app-core connector read methods",
)
text = replace_once(
    text,
    """        assert_eq!(connector.start.connection.unwrap().element_id, target_id);\n        assert_eq!(connector.start.position_mm, Point { x: 30.0, y: 35.0 });\n        assert!(application.is_dirty());\n""",
    """        assert_eq!(connector.start.connection.unwrap().element_id, target_id);\n        assert_eq!(connector.start.position_mm, Point { x: 30.0, y: 35.0 });\n        let endpoints = application.connector_endpoints(source_id).unwrap().unwrap();\n        assert_eq!(endpoints.kind, ConnectorGeometryKind::Straight);\n        assert_eq!(endpoints.start.position_mm, Point { x: 30.0, y: 35.0 });\n        assert_eq!(endpoints.start.connection.unwrap().port_id, port_id);\n        let ports = application.active_page_layer_ports().unwrap();\n        assert_eq!(ports.len(), 1);\n        assert_eq!(ports[0].element_id, target_id);\n        assert_eq!(ports[0].port_id, port_id);\n        assert_eq!(ports[0].position_mm, Point { x: 30.0, y: 35.0 });\n        application\n            .set_page_layer_properties(\n                page_id,\n                layer_id,\n                "Layer".to_owned(),\n                false,\n                false,\n                None,\n            )\n            .unwrap();\n        assert!(application.active_page_layer_ports().unwrap().is_empty());\n        assert!(application.is_dirty());\n""",
    "app-core connector read test",
)
path.write_text(text, encoding="utf-8")


# Tauri desktop: presentation port targets, selected connector endpoint DTOs, and semantic commit.
path = Path("apps/desktop/src-tauri/src/lib.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    "use app_core::ApplicationSession;\n",
    """use app_core::{\n    ApplicationSession, ConnectorEndpointSide as AppConnectorEndpointSide,\n    ConnectorEndpointState as AppConnectorEndpointState, ConnectorEndpoints as AppConnectorEndpoints,\n    ConnectorGeometryKind as AppConnectorGeometryKind,\n};\n""",
    "desktop app-core imports",
)
text = replace_once(
    text,
    """    AnchorSet, Connector, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId, Element,\n    ElementId, ElementKind, Endpoint, Layer, LayerId, LineStyle, MarkerStyle, NextArtifact, Page,\n    PageId, Point, Rect, RichTextDocument, RichTextToken, Scene, Size, TextBlock,\n""",
    """    AnchorSet, Connection, Connector, ConnectorLabelStyle, Document, DocumentDefaults, DocumentId,\n    Element, ElementId, ElementKind, Endpoint, Layer, LayerId, LineStyle, MarkerStyle, NextArtifact,\n    Page, PageId, Point, PortId, Rect, RichTextDocument, RichTextToken, Scene, Size, TextBlock,\n""",
    "desktop domain imports",
)
text = replace_once(
    text,
    """    snap_elements: Vec<SnapElementDto>,\n    svg: String,\n""",
    """    snap_elements: Vec<SnapElementDto>,\n    port_targets: Vec<PortTargetDto>,\n    svg: String,\n""",
    "presentation port targets field",
)
text = replace_once(
    text,
    """struct SnapElementDto {\n    element_id: ElementId,\n    bounds_mm: Rect,\n    rotation_deg: f64,\n}\n\n""",
    """struct SnapElementDto {\n    element_id: ElementId,\n    bounds_mm: Rect,\n    rotation_deg: f64,\n}\n\n#[derive(Debug, Serialize)]\n#[serde(rename_all = "camelCase")]\nstruct PortTargetDto {\n    element_id: ElementId,\n    port_id: PortId,\n    position_mm: Point,\n}\n\n""",
    "port target DTO",
)
text = replace_once(
    text,
    """struct CreateConnectorRequest {\n    kind: ConnectorKind,\n    start_mm: Point,\n    end_mm: Point,\n}\n\n""",
    """struct CreateConnectorRequest {\n    kind: ConnectorKind,\n    start_mm: Point,\n    end_mm: Point,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = "snake_case")]\nenum ConnectorEndpointSideRequest {\n    Start,\n    End,\n}\n\n#[derive(Debug, Deserialize)]\n#[serde(rename_all = "camelCase")]\nstruct SetConnectorEndpointRequest {\n    element_id: ElementId,\n    side: ConnectorEndpointSideRequest,\n    position_mm: Point,\n    connection: Option<Connection>,\n}\n\n""",
    "endpoint request DTO",
)
text = replace_once(
    text,
    """    geometry_editable: bool,\n}\n\n""",
    """    geometry_editable: bool,\n    connector: Option<ConnectorPropertiesDto>,\n}\n\n#[derive(Debug, Serialize)]\n#[serde(rename_all = "camelCase")]\nstruct ConnectorPropertiesDto {\n    kind: &'static str,\n    start: ConnectorEndpointDto,\n    end: ConnectorEndpointDto,\n}\n\n#[derive(Debug, Serialize)]\n#[serde(rename_all = "camelCase")]\nstruct ConnectorEndpointDto {\n    position_mm: Point,\n    connection: Option<ConnectorConnectionDto>,\n}\n\n#[derive(Debug, Serialize)]\n#[serde(rename_all = "camelCase")]\nstruct ConnectorConnectionDto {\n    element_id: ElementId,\n    port_id: PortId,\n}\n\n""",
    "selection connector DTOs",
)
text = replace_once(
    text,
    """    document\n        .session\n        .set_selection([element_id])\n        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;\n    Ok(element_edit_result_dto(&document))\n}\n\n#[tauri::command]\nfn delete_selection(state: State<'_, DesktopState>) -> Result<ElementEditResultDto, CommandError> {\n""",
    """    document\n        .session\n        .set_selection([element_id])\n        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;\n    Ok(element_edit_result_dto(&document))\n}\n\n#[tauri::command]\nfn set_connector_endpoint(\n    request: SetConnectorEndpointRequest,\n    state: State<'_, DesktopState>,\n) -> Result<ElementEditResultDto, CommandError> {\n    let mut document = lock_document(&state)?;\n    let page_size = {\n        let session = document.session.session();\n        let page_id = session.active_page_id().ok_or_else(|| {\n            CommandError::new("no_active_page", "The current document has no active page.")\n        })?;\n        let layer_id = document.session.active_page_layer_id().ok_or_else(|| {\n            CommandError::new(\n                "no_active_page_layer",\n                "Choose a page-local layer before editing connector endpoints.",\n            )\n        })?;\n        let page = session\n            .document()\n            .pages\n            .iter()\n            .find(|page| page.id == page_id)\n            .ok_or_else(|| CommandError::new("page_missing", "The active page no longer exists."))?;\n        let layer = page\n            .layers\n            .iter()\n            .find(|layer| layer.id == layer_id)\n            .ok_or_else(|| CommandError::new("layer_missing", "The active layer no longer exists."))?;\n        if !layer.visible {\n            return Err(CommandError::new(\n                "connector_layer_hidden",\n                "Connector endpoints can be edited only on a visible layer.",\n            ));\n        }\n        if layer.locked {\n            return Err(CommandError::new(\n                "connector_layer_locked",\n                "Unlock the active layer before editing connector endpoints.",\n            ));\n        }\n        if !layer\n            .scene\n            .elements\n            .iter()\n            .any(|element| element.id == request.element_id)\n        {\n            return Err(CommandError::new(\n                "connector_not_on_active_layer",\n                "The connector must belong to the active page-local layer.",\n            ));\n        }\n        page.size_mm\n    };\n    let position_mm = clamp_connector_point(request.position_mm, page_size)?;\n    let side = match request.side {\n        ConnectorEndpointSideRequest::Start => AppConnectorEndpointSide::Start,\n        ConnectorEndpointSideRequest::End => AppConnectorEndpointSide::End,\n    };\n    document\n        .session\n        .set_connector_endpoint(request.element_id, side, position_mm, request.connection)\n        .map_err(|error| CommandError::new("connector_endpoint_failed", error.to_string()))?;\n    document\n        .session\n        .set_selection([request.element_id])\n        .map_err(|error| CommandError::new("selection_failed", error.to_string()))?;\n    Ok(element_edit_result_dto(&document))\n}\n\n#[tauri::command]\nfn delete_selection(state: State<'_, DesktopState>) -> Result<ElementEditResultDto, CommandError> {\n""",
    "semantic endpoint Tauri command",
)
text = replace_once(
    text,
    """    let snap_elements = plan\n        .items\n        .iter()\n        .map(|item| SnapElementDto {\n            element_id: item.element.id,\n            bounds_mm: item.element.bounds_mm,\n            rotation_deg: item.element.rotation_deg,\n        })\n        .collect();\n    let rendered = render_plan_to_svg(\n""",
    """    let snap_elements = plan\n        .items\n        .iter()\n        .map(|item| SnapElementDto {\n            element_id: item.element.id,\n            bounds_mm: item.element.bounds_mm,\n            rotation_deg: item.element.rotation_deg,\n        })\n        .collect();\n    let port_targets = document\n        .session\n        .active_page_layer_ports()\n        .map_err(|error| CommandError::new("connector_ports_failed", error.to_string()))?\n        .into_iter()\n        .map(|port| PortTargetDto {\n            element_id: port.element_id,\n            port_id: port.port_id,\n            position_mm: port.position_mm,\n        })\n        .collect();\n    let rendered = render_plan_to_svg(\n""",
    "presentation port query",
)
text = replace_once(
    text,
    """        height_mm: page.size_mm.height,\n        snap_elements,\n        svg: rendered.svg,\n""",
    """        height_mm: page.size_mm.height,\n        snap_elements,\n        port_targets,\n        svg: rendered.svg,\n""",
    "presentation port DTO assignment",
)
text = replace_once(
    text,
    """        Some(element_properties_dto(element))\n    } else {\n""",
    """        let connector = document\n            .session\n            .connector_endpoints(element.id)\n            .map_err(|error| CommandError::new("connector_query_failed", error.to_string()))?;\n        Some(element_properties_dto(element, connector))\n    } else {\n""",
    "selection connector query",
)
text = replace_once(
    text,
    """fn element_properties_dto(element: &Element) -> ElementPropertiesDto {\n""",
    """fn element_properties_dto(\n    element: &Element,\n    connector: Option<AppConnectorEndpoints>,\n) -> ElementPropertiesDto {\n""",
    "element properties connector argument",
)
text = replace_once(
    text,
    """        text_editable,\n        geometry_editable: element_geometry_editable(&element.kind),\n    }\n}\n\nfn element_geometry_editable(kind: &ElementKind) -> bool {\n""",
    """        text_editable,\n        geometry_editable: element_geometry_editable(&element.kind),\n        connector: connector.and_then(connector_properties_dto),\n    }\n}\n\nfn connector_properties_dto(connector: AppConnectorEndpoints) -> Option<ConnectorPropertiesDto> {\n    let kind = match connector.kind {\n        AppConnectorGeometryKind::Straight => "straight",\n        AppConnectorGeometryKind::Orthogonal => "orthogonal",\n        AppConnectorGeometryKind::Curve => return None,\n    };\n    Some(ConnectorPropertiesDto {\n        kind,\n        start: connector_endpoint_dto(connector.start),\n        end: connector_endpoint_dto(connector.end),\n    })\n}\n\nfn connector_endpoint_dto(endpoint: AppConnectorEndpointState) -> ConnectorEndpointDto {\n    ConnectorEndpointDto {\n        position_mm: endpoint.position_mm,\n        connection: endpoint.connection.map(|connection| ConnectorConnectionDto {\n            element_id: connection.element_id,\n            port_id: connection.port_id,\n        }),\n    }\n}\n\nfn element_geometry_editable(kind: &ElementKind) -> bool {\n""",
    "connector selection DTO conversion",
)
path.write_text(text, encoding="utf-8")


# Tauri build manifest must include the new command.
path = Path("apps/desktop/src-tauri/build.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    '            "create_connector",\n            "delete_selection",\n',
    '            "create_connector",\n            "set_connector_endpoint",\n            "delete_selection",\n',
    "Tauri app manifest endpoint command",
)
path.write_text(text, encoding="utf-8")

# Runtime handler mirrors the build manifest.
path = Path("apps/desktop/src-tauri/src/lib.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    """            create_basic_element,\n            create_connector,\n            delete_selection,\n""",
    """            create_basic_element,\n            create_connector,\n            set_connector_endpoint,\n            delete_selection,\n""",
    "Tauri runtime endpoint command",
)
path.write_text(text, encoding="utf-8")
