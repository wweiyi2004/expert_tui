"""Check the compiled bundle's offline UI and default isolated storage."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--bundle', type=Path, default=Path('output/native/bundle'))
    parser.add_argument('--report', type=Path, default=Path('output/validation/standalone-smoke.json'))
    args = parser.parse_args()
    bundle = args.bundle.resolve()
    checks = []
    with tempfile.TemporaryDirectory(prefix='expert-tui-standalone-') as scratch:
        root = Path(scratch)
        env = {k: v for k, v in os.environ.items() if not k.startswith('EXPERT_')}
        env.update(LOCALAPPDATA=str(root / 'local'), APPDATA=str(root / 'roaming'))
        version = subprocess.run([str(bundle / 'expert-tui.exe'), '--version'], env=env,
                                 capture_output=True, text=True, check=True, timeout=15)
        assert version.stdout.strip() == 'expert 0.2.0', version.stdout
        scene = subprocess.run([str(bundle / 'expert-tui.exe'), '--snapshot', 'welcome', '--ascii'],
                               env=env, capture_output=True, check=True, timeout=15)
        assert scene.stdout.strip(), 'Empty terminal snapshot'
        checks.append('compiled version and offline welcome snapshot succeed')
        host = subprocess.Popen([str(bundle / 'host/bin/expert-host.exe'), '--idle-seconds', '2'],
                                env=env, cwd=root, stdin=subprocess.DEVNULL,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            stdout, stderr = host.communicate(timeout=45)
            assert host.returncode == 0, (stdout.decode(errors='replace'), stderr.decode(errors='replace'))
            assert b'Expert Host ready' in stdout, stdout
            state = root / 'local/expert-tui/state'
            assert (state / 'documents/expert_chat.sqlite').is_file(), list(root.rglob('*'))
            assert (state / 'host/discovery').is_dir()
            assert not (root / 'local/expert-chat').exists()
            assert not (root / 'roaming/com.wweiyi').exists()
            checks.append('default Host startup uses expert-tui/state, creates its own database, and exits when idle')
            checks.append('legacy expert-chat and GUI storage namespaces remain untouched')
        finally:
            if host.poll() is None:
                host.terminate()
                host.communicate(timeout=10)
    report = {'ok': True, 'bundle': str(bundle), 'checks': checks, 'paid_model_calls': 0}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
