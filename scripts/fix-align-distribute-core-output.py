from pathlib import Path

path = Path("crates/editor-core/src/lib.rs")
text = path.read_text(encoding="utf-8")

old = '''    let mut items = Vec::with_capacity(selected.len());
    for element_id in selected {
        let element = find_element(document, element_id)
            .ok_or(EditorError::ElementNotFound(element_id))?;
        items.push(ArrangeItem {
            element_id,
            bounds: element_visual_bounds(element),
        });
    }
'''
new = '''    let mut items = Vec::with_capacity(selected.len());
    for element_id in selected {
        let mut recursion_stack = BTreeSet::new();
        let bounds = subtree_visual_bounds(&layer.scene, element_id, &mut recursion_stack)?;
        items.push(ArrangeItem { element_id, bounds });
    }
'''
if text.count(old) != 1:
    raise SystemExit(f"expected one arrange item block, got {text.count(old)}")
text = text.replace(old, new, 1)

start = text.find("fn element_visual_bounds(element: &Element) -> Rect {")
if start == -1:
    raise SystemExit("generated element_visual_bounds helper not found")
end_marker = "/// Bounds are common to every element. Connector endpoint positions and curve\n"
end = text.find(end_marker, start)
if end == -1:
    raise SystemExit("following geometry documentation marker not found")
# The pre-existing source has an empty doc-comment separator immediately before
# the insertion point. Remove it together with the now-redundant helper.
prefix = text[:start]
if prefix.endswith("///\n\n"):
    prefix = prefix[:-5]
text = prefix + text[end:]

path.write_text(text, encoding="utf-8")
