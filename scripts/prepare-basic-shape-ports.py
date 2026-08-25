from pathlib import Path

path = Path("apps/desktop/src-tauri/src/lib.rs")
text = path.read_text(encoding="utf-8")

old = """    Element, ElementId, ElementKind, Endpoint, Layer, LayerId, LineStyle, MarkerStyle, NextArtifact,\n    Page, PageId, Point, PortId, Rect, RichTextDocument, RichTextToken, Scene, Size, TextBlock,\n"""
new = """    Element, ElementId, ElementKind, Endpoint, Layer, LayerId, LineStyle, MarkerStyle, NextArtifact,\n    NormalizedPoint, Page, PageId, Point, Port, PortId, Rect, RichTextDocument, RichTextToken, Scene,\n    Size, TextBlock,\n"""
if old not in text:
    raise SystemExit("domain import anchor missing")
text = text.replace(old, new, 1)

old = """        anchors: AnchorSet::default(),\n        ports: Vec::new(),\n        style_id: None,\n        text,\n        kind,\n"""
new = """        anchors: AnchorSet::default(),\n        ports: default_shape_ports(),\n        style_id: None,\n        text,\n        kind,\n"""
if old not in text:
    raise SystemExit("basic element ports anchor missing")
text = text.replace(old, new, 1)

anchor = """fn connector_bounds(start_mm: Point, end_mm: Point) -> Rect {\n    Rect {\n        x: start_mm.x.min(end_mm.x),\n        y: start_mm.y.min(end_mm.y),\n        width: (start_mm.x - end_mm.x).abs().max(0.1),\n        height: (start_mm.y - end_mm.y).abs().max(0.1),\n    }\n}\n\n"""
addition = """fn default_shape_ports() -> Vec<Port> {\n    [\n        (0_u16, 0.5, 0.0),\n        (1_u16, 1.0, 0.5),\n        (2_u16, 0.5, 1.0),\n        (3_u16, 0.0, 0.5),\n    ]\n    .into_iter()\n    .map(|(index, x, y)| Port {\n        id: PortId::new(),\n        index,\n        position: NormalizedPoint { x, y },\n    })\n    .collect()\n}\n\n"""
if anchor not in text:
    raise SystemExit("connector_bounds anchor missing")
text = text.replace(anchor, anchor + addition, 1)
path.write_text(text, encoding="utf-8")
