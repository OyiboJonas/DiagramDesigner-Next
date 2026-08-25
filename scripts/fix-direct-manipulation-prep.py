from pathlib import Path

path = Path('scripts/prepare-direct-manipulation.py')
text = path.read_text(encoding='utf-8')
old = '''candidate = replace_once(
    candidate,
    '''      removeMoveOverlay(svg);\n      removeSnapGuides(svg);\n      removeConnectorPreview(svg);\n''',
    '''      removeMoveOverlay(svg);\n      removeSnapGuides(svg);\n      removeTransformPreview(svg);\n      removeTransformEditor(svg);\n      removeConnectorPreview(svg);\n''',
    "candidate presentation transform cleanup",
)
'''
new = '''candidate = replace_once(
    candidate,
    '''      const previousSelection = preserveSelection ? [...selectedElementIds] : [];\n      disposePointerBinding?.();\n      disposePointerBinding = null;\n      removeMoveOverlay(svg);\n      removeSnapGuides(svg);\n      removeConnectorPreview(svg);\n''',
    '''      const previousSelection = preserveSelection ? [...selectedElementIds] : [];\n      disposePointerBinding?.();\n      disposePointerBinding = null;\n      removeMoveOverlay(svg);\n      removeSnapGuides(svg);\n      removeTransformPreview(svg);\n      removeTransformEditor(svg);\n      removeConnectorPreview(svg);\n''',
    "candidate presentation transform cleanup",
)
'''
if old not in text:
    raise RuntimeError('preparation cleanup block not found')
path.write_text(text.replace(old, new, 1), encoding='utf-8')
