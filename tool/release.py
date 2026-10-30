"""Check release identity, tags and portable npm artifact integrity. Never publishes."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import tarfile
import urllib.error
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
NAME = '@wweiyi/expert'
REPOSITORY = 'git+https://github.com/wweiyi2004/expert_tui.git'
REQUIRED_FILES = {
    'native/expert-tui.exe', 'native/expert-desktop.exe',
    'native/host/bin/expert-host.exe', 'native/host/lib/sqlite3.dll',
    'native/expert-media-worker/expert-media-worker.exe',
    'bin/expert.cjs', 'lib/cli.cjs', 'lib/update.cjs', 'lib/runtime.cjs', 'expert.cmd',
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def metadata(root=ROOT, *, require_tag=False, ref=None):
    package = json.loads((root / 'npm/expert/package.json').read_text(encoding='utf-8'))
    version = package['version']
    rust = re.search(r'^version\s*=\s*"([^"]+)"',
                     (root / 'next/tui/Cargo.toml').read_text(encoding='utf-8'), re.M)[1]
    require(package['name'] == NAME, 'Release must retain the existing npm package identity')
    require(package.get('bin') == {'expert': 'bin/expert.cjs'}, 'The existing expert command must be retained')
    require(re.fullmatch(r'(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)', version), 'Stable semver required')
    require(version == rust, 'Rust and npm versions differ')
    require(package.get('repository', {}).get('url') == REPOSITORY, 'npm repository must match this GitHub repository')
    if require_tag:
        require((ref if ref is not None else os.environ.get('GITHUB_REF')) == f'refs/tags/v{version}',
                f'Publishing requires the exact v{version} tag; branch builds cannot publish')
    return {'name': NAME, 'version': version, 'repository': REPOSITORY}


def registry_version(name, version):
    url = 'https://registry.npmjs.org/' + urllib.parse.quote(name, safe='@') + '/' + version
    try:
        with urllib.request.urlopen(url, timeout=30) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise


def verify_artifacts(base, expected):
    tarballs = list(base.rglob('*.tgz'))
    receipts = list(base.rglob('release.json'))
    require(len(tarballs) == len(receipts) == 1, 'Expected exactly one npm tarball and release receipt')
    tarball = tarballs[0]
    receipt = json.loads(receipts[0].read_text(encoding='utf-8'))
    payload = tarball.read_bytes()
    require(receipt['name'] == expected['name'] and receipt['version'] == expected['version'], 'Release receipt identity mismatch')
    require(not receipt['private_draft'], 'Draft packages cannot be published')
    require(hashlib.sha256(payload).hexdigest() == receipt['tgz_sha256'], 'Tarball SHA-256 mismatch')
    integrity = 'sha512-' + base64.b64encode(hashlib.sha512(payload).digest()).decode()
    require(integrity == receipt['npm_integrity'], 'Tarball SHA-512 integrity mismatch')
    with tarfile.open(tarball, 'r:gz') as archive:
        members = archive.getmembers()
        files = {}
        for member in members:
            path = PurePosixPath(member.name)
            require(member.isfile() and not path.is_absolute() and '..' not in path.parts
                    and len(path.parts) > 1 and path.parts[0] == 'package', 'Unsafe tar entry')
            relative = path.relative_to('package').as_posix()
            require(relative not in files, 'Duplicate tar entry')
            files[relative] = member
        require('package.json' in files and 'release-manifest.json' in files, 'Package metadata missing')
        package = json.load(archive.extractfile(files['package.json']))
        manifest = json.load(archive.extractfile(files['release-manifest.json']))
        for key in ['name', 'version']:
            require(package[key] == manifest[key] == expected[key], f'Package {key} mismatch')
        require(package.get('private') is False, 'Package must be publishable')
        require(package.get('repository', {}).get('url') == expected['repository'], 'Source repository mismatch')
        require(package.get('bin') == {'expert': 'bin/expert.cjs'}, 'Command shim mismatch')
        require(manifest.get('schema') == 1 and manifest.get('platform') == 'win32-x64', 'Unsupported manifest')
        require(set(files) == set(manifest['files']) | {'release-manifest.json'}, 'File inventory mismatch')
        require(REQUIRED_FILES <= set(files), 'Native runtime or launcher files missing')
        for relative, info in manifest['files'].items():
            data = archive.extractfile(files[relative]).read()
            require(len(data) == info['bytes'] and hashlib.sha256(data).hexdigest() == info['sha256'],
                    f'Package payload mismatch: {relative}')
    return {**expected, 'tarball': str(tarball.resolve()), 'integrity': integrity, 'files': len(files)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('action', choices=['metadata', 'verify'])
    parser.add_argument('directory', nargs='?', type=Path)
    parser.add_argument('--require-tag', action='store_true')
    parser.add_argument('--check-unpublished', action='store_true')
    parser.add_argument('--check-published', action='store_true')
    args = parser.parse_args()
    expected = metadata(require_tag=args.require_tag)
    if args.check_unpublished:
        require(registry_version(expected['name'], expected['version']) is None,
                'This version is already published; increase both Rust and npm versions before publishing')
    result = expected
    if args.action == 'verify':
        require(args.directory is not None, 'Artifact directory required')
        result = verify_artifacts(args.directory.resolve(), expected)
    if args.check_published:
        require(args.action == 'verify', 'Verify artifacts before checking the published package')
        published = registry_version(expected['name'], expected['version'])
        require(published is not None and published['dist']['integrity'] == result['integrity'],
                'Registry does not expose the expected release integrity')
    if os.environ.get('GITHUB_OUTPUT'):
        with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as output:
            for key in ['name', 'version', 'tarball']:
                if key in result:
                    output.write(f'{key}={result[key]}\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
