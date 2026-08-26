from pathlib import Path

helper = Path('apps/desktop/ui/editor-interaction/clipboard-actions.mjs')
helper.write_text('''export function isClipboardSelectionActionEnabled({ selectionCount = 0, busy = false } = {}) {\n  return !busy && Number(selectionCount) > 0;\n}\n\nexport function isClipboardShortcutActionEnabled(\n  { shortcut, selectionCount = 0, clipboardAvailable = false } = {},\n) {\n  if (shortcut === 'copy-selection' || shortcut === 'duplicate-selection') {\n    return Number(selectionCount) > 0;\n  }\n  if (shortcut === 'paste-selection') {\n    return clipboardAvailable === true;\n  }\n  return true;\n}\n''', encoding='utf-8')

reexport = Path('web/editor-interaction/clipboard-actions.mjs')
reexport.write_text("export { isClipboardSelectionActionEnabled, isClipboardShortcutActionEnabled } from '../../apps/desktop/ui/editor-interaction/clipboard-actions.mjs';\n", encoding='utf-8')

test = Path('web/editor-interaction/clipboard-actions.test.mjs')
test.write_text('''import test from 'node:test';\nimport assert from 'node:assert/strict';\n\nimport {\n  isClipboardSelectionActionEnabled,\n  isClipboardShortcutActionEnabled,\n} from './clipboard-actions.mjs';\n\ntest('clipboard selection actions allow structural group selections', () => {\n  assert.equal(isClipboardSelectionActionEnabled(), false);\n  assert.equal(isClipboardSelectionActionEnabled({ selectionCount: 0, busy: false }), false);\n  assert.equal(isClipboardSelectionActionEnabled({ selectionCount: 1, busy: true }), false);\n  assert.equal(isClipboardSelectionActionEnabled({ selectionCount: 1, busy: false }), true);\n  assert.equal(\n    isClipboardSelectionActionEnabled({ selectionCount: 1, busy: false, containsGroup: true }),\n    true,\n  );\n});\n\ntest('clipboard keyboard actions do not special-case structural groups', () => {\n  assert.equal(\n    isClipboardShortcutActionEnabled({\n      shortcut: 'copy-selection',\n      selectionCount: 1,\n      containsGroup: true,\n    }),\n    true,\n  );\n  assert.equal(\n    isClipboardShortcutActionEnabled({\n      shortcut: 'duplicate-selection',\n      selectionCount: 1,\n      containsGroup: true,\n    }),\n    true,\n  );\n  assert.equal(\n    isClipboardShortcutActionEnabled({ shortcut: 'copy-selection', selectionCount: 0 }),\n    false,\n  );\n  assert.equal(\n    isClipboardShortcutActionEnabled({ shortcut: 'paste-selection', clipboardAvailable: false }),\n    false,\n  );\n  assert.equal(\n    isClipboardShortcutActionEnabled({ shortcut: 'paste-selection', clipboardAvailable: true }),\n    true,\n  );\n  assert.equal(isClipboardShortcutActionEnabled({ shortcut: 'save' }), true);\n});\n''', encoding='utf-8')

app = Path('apps/desktop/ui/app.js')
text = app.read_text(encoding='utf-8')
text = text.replace(
"import { isClipboardSelectionActionEnabled } from './editor-interaction/clipboard-actions.mjs';",
"import { isClipboardSelectionActionEnabled, isClipboardShortcutActionEnabled } from './editor-interaction/clipboard-actions.mjs';",
)
old = '''        const selectionCount = Number(currentSelectionProperties?.count ?? 0);\n        const containsGroup = currentSelectionProperties?.containsGroup === true;\n        if (\n          ((shortcut === 'delete-selection' ||\n            shortcut === 'copy-selection' ||\n            shortcut === 'duplicate-selection') &&\n            selectionCount === 0) ||\n          ((shortcut === 'copy-selection' || shortcut === 'duplicate-selection') && containsGroup) ||\n          (shortcut === 'paste-selection' && !clipboardAvailable)\n        ) {\n          return;\n        }\n'''
new = '''        const selectionCount = Number(currentSelectionProperties?.count ?? 0);\n        if (\n          (shortcut === 'delete-selection' && selectionCount === 0) ||\n          !isClipboardShortcutActionEnabled({ shortcut, selectionCount, clipboardAvailable })\n        ) {\n          return;\n        }\n'''
if old not in text:
    raise SystemExit('keyboard shortcut guard block not found')
text = text.replace(old, new)
app.write_text(text, encoding='utf-8')
