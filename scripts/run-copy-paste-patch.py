from pathlib import Path
import base64
import re
import zlib

bootstrap = Path('scripts/apply-copy-paste-duplicate.py').read_text(encoding='utf-8')
match = re.search(r"b64decode\('([^']+)'\)", bootstrap)
if match is None:
    raise RuntimeError('Could not locate compressed clipboard patch payload')
source = zlib.decompress(base64.b64decode(match.group(1))).decode('utf-8')
source = source.replace(
    'if matches!(element.kind, ElementKind::Group { .. }) {',
    'if matches!(&element.kind, ElementKind::Group { .. }) {',
)
source = source.replace(
    '''        assert_eq!(\n            capture_selection(&document, &[group_id]),\n            Err(ClipboardError::GroupUnsupported(group_id))\n        );''',
    '''        assert!(matches!(\n            capture_selection(&document, &[group_id]),\n            Err(ClipboardError::GroupUnsupported(id)) if id == group_id\n        ));''',
)
exec(compile(source, '<clipboard-patch>', 'exec'))
