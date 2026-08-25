from pathlib import Path

path = Path("scripts/prepare-direct-manipulation.py")
text = path.read_text(encoding="utf-8")
label = '    "candidate presentation transform cleanup",\n)'
label_index = text.find(label)
if label_index < 0:
    raise RuntimeError("presentation cleanup patch label not found")
block_start = text.rfind("candidate = replace_once(\n", 0, label_index)
if block_start < 0:
    raise RuntimeError("presentation cleanup patch start not found")
block_end = label_index + len(label)
replacement = (
    "candidate = replace_once(\n"
    "    candidate,\n"
    "    '''      const previousSelection = preserveSelection ? [...selectedElementIds] : [];\\n"
    "      disposePointerBinding?.();\\n"
    "      disposePointerBinding = null;\\n"
    "      removeMoveOverlay(svg);\\n"
    "      removeSnapGuides(svg);\\n"
    "      removeConnectorPreview(svg);\\n''',\n"
    "    '''      const previousSelection = preserveSelection ? [...selectedElementIds] : [];\\n"
    "      disposePointerBinding?.();\\n"
    "      disposePointerBinding = null;\\n"
    "      removeMoveOverlay(svg);\\n"
    "      removeSnapGuides(svg);\\n"
    "      removeTransformPreview(svg);\\n"
    "      removeTransformEditor(svg);\\n"
    "      removeConnectorPreview(svg);\\n''',\n"
    "    \"candidate presentation transform cleanup\",\n"
    ")"
)
text = text[:block_start] + replacement + text[block_end:]
path.write_text(text, encoding="utf-8")
