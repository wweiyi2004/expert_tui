"""Build the complete Windows terminal runtime from this repository only."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'output/native'


def run(args):
    args = [shutil.which(str(args[0])) or str(args[0]), *map(str, args[1:])]
    print(' '.join(args), flush=True)
    subprocess.run(args, cwd=ROOT, check=True)


def main():
    if os.name != 'nt':
        raise SystemExit('The bundled host currently supports Windows x64 only.')
    OUT.mkdir(parents=True, exist_ok=True)
    run(['dart', 'pub', 'get', '--enforce-lockfile'])
    run(['cargo', 'build', '--manifest-path', 'next/Cargo.toml', '--release', '--locked'])
    run(['dart', 'build', 'cli', '--target=bin/expert_host.dart', f'--output={OUT / "dart-build"}'])
    run(['uv', 'sync', '--project', 'server', '--frozen'])
    python = ROOT / 'server/.venv/Scripts/python.exe'
    run([python, '-m', 'PyInstaller', '--noconfirm', '--onedir', '--console',
         '--name', 'expert-media-worker', '--distpath', OUT,
         '--workpath', OUT / 'media-build', '--specpath', OUT / 'media-build',
         '--collect-submodules', 'pypdf', '--collect-submodules', 'PIL',
         '--collect-submodules', 'zstandard', 'server/desktop_media_worker.py'])
    bundle = OUT / 'bundle'
    bundle.mkdir(exist_ok=True)
    for name in ['expert-tui.exe', 'expert-desktop.exe']:
        shutil.copy2(ROOT / 'next/target/release' / name, bundle / name)
    shutil.copytree(OUT / 'dart-build/bundle', bundle / 'host', dirs_exist_ok=True)
    original = bundle / 'host/bin/expert_host.exe'
    original.replace(original.with_name('expert-host.exe'))
    shutil.copytree(OUT / 'expert-media-worker', bundle / 'expert-media-worker', dirs_exist_ok=True)
    assets = bundle / 'data/flutter_assets/assets/math'
    assets.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / 'assets/math/toolkit.py', assets / 'toolkit.py')
    print(json.dumps({'bundle': str(bundle), 'source': str(ROOT)}), flush=True)


if __name__ == '__main__':
    main()
