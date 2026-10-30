"""Run the plain Dart suite with this repository's compiled media worker."""
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
worker = Path(os.environ.get('EXPERT_MEDIA_WORKER', str(
    ROOT / 'output/native/bundle/expert-media-worker/expert-media-worker.exe'))).resolve()
if not worker.is_file():
    raise SystemExit('Build the native bundle first: python tool/build_native.py')
env = {**os.environ, 'EXPERT_MEDIA_WORKER': str(worker)}
raise SystemExit(subprocess.call(
    [shutil.which('dart') or 'dart', 'test', *sys.argv[1:]], cwd=ROOT, env=env))
