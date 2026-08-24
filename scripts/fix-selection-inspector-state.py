from pathlib import Path

path = Path("apps/desktop/ui/app.js")
text = path.read_text(encoding="utf-8")
old = '''  const primary = details?.primary ?? null;
  if (!primary) {
    elements.selectionPropertiesForm.hidden = true;
    return;
  }
'''
new = '''  const primary = details?.primary ?? null;
  elements.applyProperties.disabled = !primary;
  if (!primary) {
    elements.selectionPropertiesForm.hidden = true;
    return;
  }
'''
if text.count(old) != 1:
    raise SystemExit(f"selection inspector anchor count: {text.count(old)}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
print("Applied selection inspector state fix")
