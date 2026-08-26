from pathlib import Path

path = Path('apps/desktop/ui/candidate-svg-keyboard.mjs')
text = path.read_text(encoding='utf-8')
old = """      const elementId = event.target?.getAttribute?.('data-element-id');
      if (elementId && currentElements.some((entry) => entry.id === elementId)) {
        controller.activate(elementId);
"""
new = """      const rawElementId = event.target?.getAttribute?.('data-element-id');
      const elementId = rawElementId ? resolveElementId(rawElementId) : null;
      if (elementId && currentElements.some((entry) => entry.id === elementId)) {
        controller.activate(elementId);
"""
count = text.count(old)
if count != 1:
    raise SystemExit(f'keyboard focus marker count={count}')
path.write_text(text.replace(old, new, 1), encoding='utf-8')
