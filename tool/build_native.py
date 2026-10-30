"""Build the complete Windows terminal runtime from this repository only."""
import json
import argparse
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
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, default=OUT,
                        help='Fresh build directory inside this project output/ directory')
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to((ROOT / 'output').resolve()):
        raise SystemExit('Build output must stay inside this repository output/ directory.')
    if (output / 'bundle').exists():
        raise SystemExit('Bundle already exists; choose a fresh --output directory to preserve running versions.')
    if os.name != 'nt':
        raise SystemExit('The bundled host currently supports Windows x64 only.')
    output.mkdir(parents=True, exist_ok=True)
    run(['dart', 'pub', 'get', '--enforce-lockfile'])
    run(['cargo', 'build', '--manifest-path', 'next/Cargo.toml', '--release', '--locked'])
    run(['dart', 'build', 'cli', '--target=bin/expert_host.dart', f'--output={output / "dart-build"}'])
    run(['uv', 'sync', '--project', 'server', '--frozen'])
    python = ROOT / 'server/.venv/Scripts/python.exe'
    run([python, '-m', 'PyInstaller', '--noconfirm', '--onedir', '--console',
         '--name', 'expert-pdf-worker', '--distpath', output,
         '--workpath', output / 'pdf-build', '--specpath', output / 'pdf-build',
         '--collect-submodules', 'pypdf', '--exclude-module', 'PIL',
         '--exclude-module', 'zstandard', 'server/desktop_pdf_worker.py'])
    bundle = output / 'bundle'
    bundle.mkdir(exist_ok=True)
    for name in ['expert-tui.exe', 'expert-desktop.exe']:
        shutil.copy2(ROOT / 'next/target/release' / name, bundle / name)
    shutil.copytree(output / 'dart-build/bundle', bundle / 'host')
    original = bundle / 'host/bin/expert_host.exe'
    original.replace(original.with_name('expert-host.exe'))
    media = bundle / 'expert-media-worker'
    media.mkdir()
    shutil.copy2(ROOT / 'next/target/release/expert-media-worker.exe', media / 'expert-media-worker.exe')
    shutil.copytree(output / 'expert-pdf-worker', bundle / 'expert-pdf-worker')
    assets = bundle / 'data/flutter_assets/assets/math'
    assets.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / 'assets/math/toolkit.py', assets / 'toolkit.py')
    print(json.dumps({'bundle': str(bundle), 'source': str(ROOT)}), flush=True)


if __name__ == '__main__':
    main()
