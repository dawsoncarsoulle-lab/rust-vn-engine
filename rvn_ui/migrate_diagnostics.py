"""Mechanical, reviewed diagnostic-only migration. Never translates game content."""
import json
import re
from pathlib import Path

root = Path(__file__).resolve().parent
catalog = json.loads((root / "diagnostic_translations.json").read_text())
token = re.compile(r'//[^\n]*|/\*.*?\*/|"(?:\\.|[^"\\])*"', re.S)
for path in (root / "src").glob("*.rs"):
    text = path.read_text()
    source = text
    masked = token.sub(lambda m: ' ' * len(m[0]), source)
    tests = []
    for match in re.finditer(r'#\[cfg\(test\)\]', masked):
        opening = masked.index('{', match.end())
        depth, closing = 1, opening + 1
        while depth:
            depth += (masked[closing] == '{') - (masked[closing] == '}')
            closing += 1
        tests.append((match.start(), closing))
    edits = []
    for match in token.finditer(source):
        if any(a <= match.start() < b for a, b in tests):
            continue
        if not match[0].startswith('"'):
            continue
        try:
            value = json.loads(match[0])
        except ValueError:
            continue
        if value not in catalog:
            continue
        before = source[:match.start()]
        if re.search(r'diagnostic!\(\s*$', before):
            continue
        formatted = re.search(r'format!\(\s*$', before)
        pair = match[0] + ', ' + json.dumps(catalog[value], ensure_ascii=False)
        if formatted:
            edits.append((formatted.start(), match.end(), 'diagnostic!(' + pair))
        else:
            edits.append((match.start(), match.end(), 'diagnostic!(' + pair + ')'))
    for left, right, replacement in reversed(edits):
        source = source[:left] + replacement + source[right:]
    path.write_text(source)
