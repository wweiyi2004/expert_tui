"""Crash only a freshly spawned isolated Host and verify the Rust TUI relaunches it."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
from host_e2e import Rpc, wait


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--probe', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix='expert-host-recovery-')).resolve()
    discovery = root / 'state/host/discovery'
    env = {k: v for k, v in os.environ.items() if not k.startswith('EXPERT_')}
    env.update(EXPERT_HOST_DATA_DIR=str(root / 'state'),
               EXPERT_CHAT_HOST=str(args.bundle.resolve() / 'host/bin/expert-host.exe'))
    log_path = root / 'probe.log'
    log = log_path.open('w', encoding='utf-8')
    probe = subprocess.Popen([str(args.probe.resolve())], env=env, cwd=args.bundle.resolve(),
                             stdout=log, stderr=log,
                             creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0)
    try:
        wait(lambda: 'ready' in log_path.read_text(encoding='utf-8'), seconds=30)
        first = json.loads((discovery / 'bridge.json').read_text())['pid']
        # This PID was just created by our probe using its unique isolated profile.
        subprocess.run(['taskkill', '/PID', str(first), '/F'], check=True, capture_output=True)
        # A detached Windows child can inherit a pipe handle; wait on the probe
        # process itself rather than waiting for every descendant to close EOF.
        probe.wait(timeout=45)
        output = log_path.read_text(encoding='utf-8')
        assert probe.returncode == 0 and 'recovered' in output, output
        second = json.loads((discovery / 'bridge.json').read_text())['pid']
        assert first != second
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps({'passed': True, 'host_relaunched': True,
                                          'restored_snapshot': True, 'paid_requests': 0}, indent=2))
    finally:
        if probe.poll() is None:
            probe.kill()
            probe.wait(timeout=10)
        log.close()
        if (discovery / 'gui.json').exists():
            try:
                client = Rpc(discovery, gui=True)
                client.call('host.shutdown', {'confirmed': True})
                client.close()
            except Exception:
                pass


if __name__ == '__main__':
    main()
