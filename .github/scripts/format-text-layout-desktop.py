from pathlib import Path

path = Path('apps/desktop/src-tauri/src/lib.rs')
text = path.read_text(encoding='utf-8')
old1 = '''        if let Some(layout_update) = text_layout {\n            next_text_block.layout = apply_text_layout_update(next_text_block.layout, layout_update)?;\n        }'''
new1 = '''        if let Some(layout_update) = text_layout {\n            next_text_block.layout =\n                apply_text_layout_update(next_text_block.layout, layout_update)?;\n        }'''
old2 = '''            (Some(preview), editable, style, Some(text_layout_dto(block.layout)))'''
new2 = '''            (\n                Some(preview),\n                editable,\n                style,\n                Some(text_layout_dto(block.layout)),\n            )'''
for old, new in [(old1, new1), (old2, new2)]:
    if text.count(old) != 1:
        raise SystemExit(f'expected exactly one rustfmt anchor, found {text.count(old)}')
    text = text.replace(old, new, 1)
path.write_text(text, encoding='utf-8')
