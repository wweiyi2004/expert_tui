"""Build a GUI-free Windows x64 npm package and matching portable ZIP.

No login, publish, production-host shutdown, settings or credential copies.
"""
import argparse
import base64
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def digest(file):
    return hashlib.sha256(file.read_bytes()).hexdigest()


def run(command, cwd=ROOT):
    command = [shutil.which(str(command[0])) or str(command[0]), *command[1:]]
    result = subprocess.run([str(c) for c in command], cwd=cwd, capture_output=True,
                            text=True, encoding='utf-8', errors='replace')
    if result.returncode:
        raise RuntimeError(f'{command[0]} failed ({result.returncode}):\n{result.stdout}\n{result.stderr}')
    return result.stdout


def npm_cli():
    node = Path(shutil.which('node'))
    candidate = node.parent / 'node_modules/npm/bin/npm-cli.js'
    if candidate.is_file():
        return [node, candidate]
    raise RuntimeError('npm-cli.js not found next to Node.js')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--scope', default='wweiyi', help='npm account or organization name, with or without @')
    parser.add_argument('--draft', action='store_true', help='Build a private local preview before npm login')
    # All components are built locally; no GUI release is required.
    parser.add_argument('--skip-build', action='store_true', help='Reuse already compiled native artifacts')
    args = parser.parse_args()
    template = ROOT / 'npm/expert'
    version = re.search(r'^version\s*=\s*"([^"]+)"', (ROOT / 'next/tui/Cargo.toml').read_text(), re.M)[1]
    if version != json.loads((template / 'package.json').read_text())['version']:
        raise RuntimeError('Rust and npm release versions must match')
    scope = args.scope
    if not scope and not args.draft:
        try:
            scope = run(npm_cli() + ['whoami', '--registry=https://registry.npmjs.org']).strip()
        except RuntimeError:
            raise RuntimeError('Log into npm first, or pass --scope ACCOUNT; --draft builds an unpublished local preview') from None
    scope = (scope or 'expert-tui-preview').removeprefix('@')
    if not re.fullmatch(r'[a-z0-9][a-z0-9._-]*', scope):
        raise ValueError('Invalid npm scope')
    name = f'@{scope}/expert'
    if not args.skip_build:
        run(['python', 'tool/build_native.py'])
    media = ROOT / 'output/native/expert-media-worker'
    if not (media / 'expert-media-worker.exe').is_file():
        raise RuntimeError('Run python tool/build_native.py first')
    stamp = datetime.datetime.now().strftime('%Y%m%d-%H%M%S')
    out = ROOT / 'output/releases' / f'expert-npm-{version}-{stamp}'
    out.mkdir(parents=True, exist_ok=False)
    package = out / 'package'
    package.mkdir()
    host_build = ROOT / 'output/native/dart-build'
    for folder in ['bin', 'lib']:
        shutil.copytree(template / folder, package / folder)
    pkg = json.loads((template / 'package.json').read_text())
    pkg.update(name=name, version=version, private=args.draft)
    pkg.pop('scripts', None)
    (package / 'package.json').write_text(json.dumps(pkg, indent=2) + '\n', encoding='utf-8')
    (package / 'README.md').write_text((template / 'README.md').read_text(encoding='utf-8').replace('@ACCOUNT/expert', name), encoding='utf-8')
    shutil.copy2(ROOT / 'LICENSE', package / 'LICENSE')
    shutil.copy2(template / 'expert.cmd', package / 'expert.cmd')
    native = package / 'native'
    native.mkdir()
    for exe in ['expert-tui.exe', 'expert-desktop.exe']:
        shutil.copy2(ROOT / 'next/target/release' / exe, native / exe)
    shutil.copytree(host_build / 'bundle', native / 'host')
    (native / 'host/bin/expert_host.exe').rename(native / 'host/bin/expert-host.exe')
    shutil.copytree(media, native / 'expert-media-worker')
    math = native / 'data/flutter_assets/assets/math'
    math.mkdir(parents=True)
    shutil.copy2(ROOT / 'assets/math/toolkit.py', math / 'toolkit.py')
    manifest = {'schema': 1, 'name': name, 'version': version, 'platform': 'win32-x64', 'files': {}}
    forbidden = {'expert_chat.exe', 'flutter_windows.dll', 'app.so', 'bridge.json', 'gui.json',
                 'owner.lock', 'operations.jsonl', '.npmrc', 'key.properties'}
    for file in sorted(package.rglob('*')):
        if file.is_symlink():
            raise RuntimeError('Package must not contain symbolic links')
        if not file.is_file():
            continue
        if file.name in forbidden or file.suffix.lower() in {'.sqlite', '.keystore', '.jks'}:
            raise RuntimeError(f'Unexpected GUI/user-data file: {file.name}')
        rel = file.relative_to(package).as_posix()
        manifest['files'][rel] = {'sha256': digest(file), 'bytes': file.stat().st_size}
    (package / 'release-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    expected = run([native / 'expert-tui.exe', '--version']).strip()
    if expected != f'expert {version}':
        raise RuntimeError(f'Unexpected native version: {expected}')
    validation = 'const r=require(process.argv[1]);r.validatePackage(process.argv[2],process.argv[3],process.argv[4]);console.log("verified")'
    run(['node', '-e', validation, package / 'lib/runtime.cjs', package, name, version])
    print('Packing npm tarball...', flush=True)
    packed = json.loads(run(npm_cli() + ['pack', str(package), '--json', '--ignore-scripts', '--pack-destination', str(out)]))[0]
    actual_files = {entry['path'] for entry in packed['files']}
    if actual_files != set(manifest['files']) | {'release-manifest.json'}:
        raise RuntimeError('npm pack file inventory does not match the verified release')
    tarball = out / packed['filename']
    if 'sha512-' + base64.b64encode(hashlib.sha512(tarball.read_bytes()).digest()).decode() != packed['integrity']:
        raise RuntimeError('Independent npm integrity check failed')
    with tarfile.open(tarball, 'r:gz') as tar:
        for member in tar.getmembers():
            if not member.isfile() or not member.name.startswith('package/'):
                raise RuntimeError('Unexpected npm tar entry')
            rel = member.name.removeprefix('package/')
            if rel == 'release-manifest.json':
                continue
            data = tar.extractfile(member).read()
            if hashlib.sha256(data).hexdigest() != manifest['files'][rel]['sha256']:
                raise RuntimeError(f'Independent npm payload validation failed: {rel}')
    archive = out / f'expert-{version}-windows-x64.zip'
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED, compresslevel=6) as z:
        for file in sorted(package.rglob('*')):
            if file.is_file():
                z.write(file, Path('Expert-TUI') / file.relative_to(package))
    result = {'name': name, 'version': version, 'private_draft': args.draft, 'published': False,
              'directory': str(out), 'package': str(package), 'tgz': str(out / packed['filename']),
              'zip': str(archive), 'file_count': len(manifest['files']) + 1,
              'npm_integrity': packed['integrity'], 'npm_bytes': packed['size'],
              'tgz_sha256': digest(out / packed['filename']), 'zip_sha256': digest(archive),
              'gui_bundled': False, 'native_version_verified': True}
    (out / 'release.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
    latest = ROOT / 'output/tui-npm-build/latest.json'
    latest.parent.mkdir(parents=True, exist_ok=True)
    latest.write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result, indent=2), flush=True)


if __name__ == '__main__':
    main()
