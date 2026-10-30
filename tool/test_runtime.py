"""Run the plain Dart suite with this repository's compiled media worker."""
import os
import argparse
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--bundle', type=Path, default=ROOT / 'output/native/bundle')
args, test_args = parser.parse_known_args()
bundle = args.bundle.resolve()
worker = Path(os.environ.get('EXPERT_RUST_MEDIA_WORKER', str(
    bundle / 'expert-media-worker/expert-media-worker.exe'))).resolve()
pdf = Path(os.environ.get('EXPERT_PDF_WORKER', str(
    bundle / 'expert-pdf-worker/expert-pdf-worker.exe'))).resolve()
if not worker.is_file() or not pdf.is_file():
    raise SystemExit('Build the complete native bundle first; both Rust media and Python PDF workers are required.')
version = subprocess.check_output([worker, '--version'], text=True)
if '(Rust)' not in version:
    raise SystemExit('Stale Python media worker; rebuild into a fresh output directory.')
env = {**os.environ, 'EXPERT_RUST_MEDIA_WORKER': str(worker), 'EXPERT_PDF_WORKER': str(pdf)}
raise SystemExit(subprocess.call(
    [shutil.which('dart') or 'dart', 'test', *test_args], cwd=ROOT, env=env))
