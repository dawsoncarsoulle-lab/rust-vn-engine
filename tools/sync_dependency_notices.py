#!/usr/bin/env python3
"""Append missing registry notices from the locked Cargo graph, without discarding
previously distributed notices. Run for the engine and editor before packaging.
Existing notices are retained: this is an inventory aid, not a legal audit.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess


def missing_notices(metadata, existing, roots):
    packages = {package['id']: package for package in metadata['packages']}
    nodes = {node['id']: node for node in metadata['resolve']['nodes']}
    pending = [key for key, package in packages.items() if package['name'] in roots]
    if len(pending) != len(roots):
        raise ValueError('Expected release packages are missing from Cargo metadata')
    selected = set()
    while pending:
        key = pending.pop()
        if key in selected:
            continue
        selected.add(key)
        pending.extend(nodes.get(key, {}).get('dependencies', []))
    known = set(re.findall(r'^={10,}\n([^\n]+)\nLicense:', existing, re.M))
    additions = []
    for package in sorted((packages[key] for key in selected), key=lambda package: (package['name'], package['version'])):
        title = f"{package['name']} {package['version']}"
        if title in known or not str(package['source'] or '').startswith('registry+'):
            continue
        folder = Path(package['manifest_path']).parent
        paths = sorted(path for path in folder.iterdir() if path.is_file() and re.match(r'^(LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE)(?:[.\-_]|$)', path.name, re.I))
        if package['license_file']:
            license_path = folder / package['license_file']
            if license_path not in paths:
                paths.append(license_path)
        license_texts = []
        for path in paths:
            if not path.resolve().is_relative_to(folder.resolve()) or not 0 < path.stat().st_size <= 1024 * 1024:
                raise ValueError(f'Invalid dependency notice: {path}')
            license_texts.append(f'\n--- {path.name} ---\n{path.read_text(encoding="utf-8")}\n')
        # This crate declares WTFPL in its manifest but omits a separate LICENSE
        # file in its source archive. Use the identical license text supplied by
        # its companion wrapper; do not invent an upstream copyright statement.
        if not license_texts and package['name'] == 'ffmpeg-sys-next' and package['license'] == 'WTFPL':
            wrapper = next(package for package in packages.values() if package['name'] == 'ffmpeg-next')
            text = (Path(wrapper['manifest_path']).parent / 'LICENSE').read_text(encoding='utf-8')
            license_texts.append('\n--- Declared WTFPL v2 (text from ffmpeg-next LICENSE) ---\n' + text + '\n')
        if not license_texts or not package['license']:
            raise ValueError(f'Manual license review required for {title}; original notices were not changed')
        authors = f"Authors: {', '.join(package['authors'])}".rstrip()
        header = f"\n{'=' * 70}\n{title}\nLicense: {package['license']}\n{package.get('repository') or ''}\n{authors}\nOriginal source archive: https://crates.io/api/v1/crates/{package['name']}/{package['version']}/download\n"
        additions.append(header + ''.join(license_texts))
    return additions


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workspace', type=Path, required=True)
    parser.add_argument('--notices', type=Path, required=True)
    parser.add_argument('--roots', nargs='+', required=True)
    parser.add_argument('--features', required=True)
    args = parser.parse_args()
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--offline', '--format-version', '1', '--features', args.features], cwd=args.workspace, text=True))
    existing = args.notices.read_text(encoding='utf-8')
    additions = missing_notices(metadata, existing, args.roots)
    if additions:
        args.notices.write_text((existing + ''.join(additions)).rstrip() + '\n', encoding='utf-8')
    print(f'Added {len(additions)} registry notices; existing notices retained: {args.notices}')


if __name__ == '__main__':
    main()
