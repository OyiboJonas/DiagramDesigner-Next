from pathlib import Path

path = Path("scripts/prepare-connector-creation-ports.py")
text = path.read_text(encoding="utf-8")
old = '''for module_path in [
    Path("apps/desktop/ui/editor-interaction/connector-gesture.mjs"),
    Path("web/editor-interaction/connector-gesture.mjs"),
]:
    patch_connector_gesture(module_path)

# Keep the production and independently tested interaction modules byte-identical.
desktop_module = Path("apps/desktop/ui/editor-interaction/connector-gesture.mjs").read_text(encoding="utf-8")
web_module = Path("web/editor-interaction/connector-gesture.mjs").read_text(encoding="utf-8")
if desktop_module != web_module:
    raise RuntimeError("connector gesture production/test copies diverged")
'''
new = '''patch_connector_gesture(Path("apps/desktop/ui/editor-interaction/connector-gesture.mjs"))

# The web test module is deliberately a stable re-export of the production gesture.
web_facade = Path("web/editor-interaction/connector-gesture.mjs").read_text(encoding="utf-8")
expected_facade = "../../apps/desktop/ui/editor-interaction/connector-gesture.mjs"
if expected_facade not in web_facade:
    raise RuntimeError("web connector gesture facade no longer re-exports the production module")
'''
count = text.count(old)
if count != 1:
    raise RuntimeError(f"expected one old facade block, found {count}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
print("Adjusted connector creation preparation for the web re-export facade.")
