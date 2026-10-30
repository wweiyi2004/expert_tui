"""Optional read-only comparison against the source checkout; not a build input."""
import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
GUI_ONLY = {
    'bridge_host.dart': 'Flutter Widget wrapper selecting the GUI bridge host',
    'bridge_host_io.dart': 'Flutter StatefulWidget / platform channel integration',
    'bridge_host_stub.dart': 'Flutter Widget stub',
    'gui_actions_flutter.dart': 'Flutter page navigation and package-info adapter',
    'gui_navigation.dart': 'GUI navigation state; no terminal execution service',
}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    args = parser.parse_args()
    source = args.source.resolve()
    problems, groups = [], {}
    for folder in ['next/tui', 'next/desktop', 'design/tui', 'npm/expert']:
        files = [p for p in (source / folder).rglob('*')
                 if p.is_file() and not {'target', 'node_modules', '__pycache__'}.intersection(p.parts)]
        missing = [p.relative_to(source).as_posix() for p in files
                   if not (ROOT / p.relative_to(source)).is_file()]
        changed = [p.relative_to(source).as_posix() for p in files
                   if (ROOT / p.relative_to(source)).is_file()
                   and p.read_bytes() != (ROOT / p.relative_to(source)).read_bytes()]
        groups[folder] = {'source_files': len(files), 'missing': missing, 'adapted': changed}
        problems.extend(missing)
    bridge = {}
    for p in (source / 'lib/features/tui_bridge').glob('*.dart'):
        rel = p.relative_to(source)
        if (ROOT / rel).is_file():
            bridge[p.name] = 'included'
        elif p.name in GUI_ONLY:
            bridge[p.name] = GUI_ONLY[p.name]
        else:
            problems.append(f'Unclassified omitted bridge source: {rel}')
    tests = {}
    for p in (source / 'test').glob('tui*test.dart'):
        if (ROOT / 'test' / p.name).is_file():
            tests[p.name] = 'active plain Dart test'
        elif (ROOT / 'archive/gui-integration' / (p.name + '.txt')).is_file():
            tests[p.name] = 'archived: Flutter preferences/path-provider integration'
        else:
            problems.append(f'Unclassified omitted terminal test: {p.name}')
    imports_checked = 0
    for p in (ROOT / 'lib').rglob('*.dart'):
        text = p.read_text(encoding='utf-8-sig')
        for uri in re.findall(r"^\s*(?:import|export|part)\s+'([^']+)'", text, re.M):
            imports_checked += 1
            if uri.startswith(('package:flutter', 'package:expert_chat/')):
                problems.append(f'Legacy/Flutter dependency: {p.relative_to(ROOT)} -> {uri}')
            if uri.startswith('package:expert_tui/'):
                target = ROOT / 'lib' / uri.split('/', 1)[1]
            elif not uri.startswith(('package:', 'dart:')):
                target = (p.parent / uri).resolve()
            else:
                continue
            if not target.is_relative_to(ROOT) or not target.is_file():
                problems.append(f'Unresolved/outside import: {p.relative_to(ROOT)} -> {uri}')
    for p in (ROOT / 'next').rglob('*.rs'):
        if 'target' in p.parts:
            continue
        for uri in re.findall(r'include_(?:str|bytes)!\("([^"]+)"\)', p.read_text(encoding='utf-8')):
            target = (p.parent / uri).resolve()
            if not target.is_relative_to(ROOT) or not target.is_file():
                problems.append(f'Unresolved/outside Rust asset: {p.relative_to(ROOT)} -> {uri}')
    manifest = json.loads((ROOT / 'docs/extraction-manifest.json').read_text(encoding='utf-8'))
    source_changed = [rel for rel, digest in manifest['source_sha256'].items()
                      if hashlib.sha256((source / rel).read_bytes()).hexdigest() != digest]
    problems.extend(f'Source changed after extraction: {p}' for p in source_changed)
    config = json.loads((ROOT / '.dart_tool/package_config.json').read_text(encoding='utf-8'))
    for package in config['packages']:
        if package['name'].startswith('flutter') or 'expert_chat' in package['rootUri']:
            problems.append(f'Unexpected resolved package: {package["name"]}')
    report = {'passed': not problems, 'scope': 'standalone terminal runtime and supporting source artifacts',
              'groups': groups, 'bridge_sources': bridge, 'terminal_tests': tests,
              'dart_imports_checked': imports_checked,
              'original_source_files_unchanged': len(manifest['source_sha256']),
              'problems': problems}
    out = ROOT / 'output/validation/completeness-audit.json'
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    print(json.dumps(report, indent=2, ensure_ascii=False))
    if problems:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
