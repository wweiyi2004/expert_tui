"""Publication guards must reject version drift, wrong tags and damaged artifacts."""
import base64
import hashlib
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

import release


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='expert-release-guard-')
        self.root = Path(self.temp.name)
        self.expected = {'name': release.NAME, 'version': '0.2.0', 'repository': release.REPOSITORY}
        self.package = {'name': release.NAME, 'version': '0.2.0', 'private': False,
                        'bin': {'expert': 'bin/expert.cjs'}, 'repository': {'url': release.REPOSITORY}}

    def tearDown(self):
        self.temp.cleanup()

    def write_metadata(self, rust_version='0.2.0'):
        (self.root / 'npm/expert').mkdir(parents=True)
        (self.root / 'next/tui').mkdir(parents=True)
        (self.root / 'npm/expert/package.json').write_text(json.dumps(self.package), encoding='utf-8')
        (self.root / 'next/tui/Cargo.toml').write_text(f'[package]\nversion = "{rust_version}"\n', encoding='utf-8')

    def artifact(self, *, tamper=False, extra=False, missing=False, unsafe=False, draft=False):
        package = {**self.package, 'private': draft}
        files = {name: b'fixture' for name in release.REQUIRED_FILES}
        if missing:
            files.pop('native/host/lib/sqlite3.dll')
        files['package.json'] = json.dumps(package).encode()
        manifest = {**self.expected, 'schema': 1, 'platform': 'win32-x64', 'files': {
            name: {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}
            for name, data in files.items()}}
        files['release-manifest.json'] = json.dumps(manifest).encode()
        if tamper:
            files['native/expert-tui.exe'] = b'changed'
        if extra:
            files['unexpected.txt'] = b'unexpected'
        if unsafe:
            files['../outside.txt'] = b'outside'
        tarball = self.root / 'candidate.tgz'
        with tarfile.open(tarball, 'w:gz') as archive:
            for name, data in files.items():
                member = tarfile.TarInfo('package/' + name)
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))
        payload = tarball.read_bytes()
        receipt = {**self.expected, 'private_draft': False,
                   'tgz_sha256': hashlib.sha256(payload).hexdigest(),
                   'npm_integrity': 'sha512-' + base64.b64encode(hashlib.sha512(payload).digest()).decode()}
        (self.root / 'release.json').write_text(json.dumps(receipt), encoding='utf-8')

    def test_matching_tag_and_original_package_are_accepted(self):
        self.write_metadata()
        self.assertEqual(release.metadata(self.root, require_tag=True, ref='refs/tags/v0.2.0'), self.expected)

    def test_branch_cannot_publish(self):
        self.write_metadata()
        with self.assertRaisesRegex(ValueError, 'branch builds cannot publish'):
            release.metadata(self.root, require_tag=True, ref='refs/heads/main')

    def test_tag_version_must_match(self):
        self.write_metadata()
        with self.assertRaises(ValueError):
            release.metadata(self.root, require_tag=True, ref='refs/tags/v0.2.1')

    def test_rust_and_npm_version_drift_is_rejected(self):
        self.write_metadata(rust_version='0.2.1')
        with self.assertRaisesRegex(ValueError, 'versions differ'):
            release.metadata(self.root)

    def test_valid_tarball_passes_without_extracting_or_executing_it(self):
        self.artifact()
        self.assertEqual(release.verify_artifacts(self.root, self.expected)['name'], release.NAME)

    def test_payload_tampering_is_rejected_even_with_valid_outer_checksums(self):
        self.artifact(tamper=True)
        with self.assertRaisesRegex(ValueError, 'payload mismatch'):
            release.verify_artifacts(self.root, self.expected)

    def test_missing_native_runtime_is_rejected(self):
        self.artifact(missing=True)
        with self.assertRaisesRegex(ValueError, 'runtime or launcher'):
            release.verify_artifacts(self.root, self.expected)

    def test_undeclared_payload_is_rejected(self):
        self.artifact(extra=True)
        with self.assertRaisesRegex(ValueError, 'inventory mismatch'):
            release.verify_artifacts(self.root, self.expected)

    def test_unsafe_tar_paths_are_rejected(self):
        self.artifact(unsafe=True)
        with self.assertRaisesRegex(ValueError, 'Unsafe tar entry'):
            release.verify_artifacts(self.root, self.expected)

    def test_draft_cannot_be_published(self):
        self.artifact(draft=True)
        with self.assertRaisesRegex(ValueError, 'publishable'):
            release.verify_artifacts(self.root, self.expected)


if __name__ == '__main__':
    unittest.main()
