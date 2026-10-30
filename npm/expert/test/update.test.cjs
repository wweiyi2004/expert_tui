'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const http = require('node:http');
const { spawnSync } = require('node:child_process');
const runtime = require('../lib/runtime.cjs');
const updater = require('../lib/update.cjs');
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'expert-npm-test-'));
process.env.EXPERT_CLI_HOME = path.join(root, 'cache');
const name = '@fixture/expert';
function fixture(version, suffix = '') {
  const dir = path.join(root, `package-${version}${suffix}`);
  fs.mkdirSync(dir);
  for (const sub of ['bin', 'lib']) fs.cpSync(path.resolve(__dirname, '..', sub), path.join(dir, sub), { recursive: true });
  const pkg = { name, version, bin: { expert: 'bin/expert.cjs' }, os: ['win32'], cpu: ['x64'] };
  fs.writeFileSync(path.join(dir, 'package.json'), JSON.stringify(pkg));
  for (const file of ['README.md', 'LICENSE', 'expert.cmd', 'native/expert-tui.exe', 'native/expert-desktop.exe',
    'native/host/bin/expert-host.exe', 'native/host/lib/sqlite3.dll', 'native/expert-media-worker/expert-media-worker.exe']) {
    fs.mkdirSync(path.dirname(path.join(dir, file)), { recursive: true });
    fs.writeFileSync(path.join(dir, file), `fixture ${version}`);
  }
  const files = {};
  for (const rel of runtime.walk(dir)) {
    const data = fs.readFileSync(path.join(dir, rel));
    files[rel] = { bytes: data.length, sha256: crypto.createHash('sha256').update(data).digest('hex') };
  }
  fs.writeFileSync(path.join(dir, 'release-manifest.json'), JSON.stringify({ schema: 1, name, version, platform: 'win32-x64', files }));
  return dir;
}
const old = fixture('0.2.0'), newer = fixture('0.2.1');

test('semantic versions, public registry and confined paths are validated', () => {
  assert.equal(runtime.compareVersions('0.2.10', '0.2.9'), 1);
  assert.throws(() => runtime.compareVersions('latest', '0.2.0'));
  for (const rel of ['../secret', '/absolute', 'C:/secret', 'lib/../../secret', 'lib\\secret']) assert.throws(() => runtime.within(root, rel));
  assert.equal(updater.registry(), 'https://registry.npmjs.org/');
});
test('version query is offline, does not start Host or initialize release cache', () => {
  const queried = spawnSync(process.execPath, [path.join(old, 'bin/expert.cjs'), 'version', '--json'], { encoding: 'utf8' });
  assert.equal(queried.status, 0, queried.stderr);
  assert.equal(JSON.parse(queried.stdout).version, '0.2.0');
  assert.equal(fs.existsSync(runtime.home(name)), false);
});
test('checking for an update changes no files or active version', async () => {
  const result = await updater.update(old, { getLatest: async () => '0.2.1' });
  assert.equal(result.available, true);
  assert.equal(result.installed, false);
  assert.equal(runtime.selectPackage(old), old);
  assert.equal(fs.existsSync(runtime.home(name)), false);
});
test('install stages a full release, preserves old files, and a fresh launcher selects new JS and native assets', async () => {
  const cachedOld = runtime.cachedPackage(old);
  const result = await updater.update(old, { install: true, getLatest: async () => '0.2.1', runNpm: async args => {
    assert.ok(args.includes('--ignore-scripts'));
    assert.ok(args.includes(`${name}@0.2.1`));
    const prefix = args[args.indexOf('--prefix') + 1];
    const destination = path.join(prefix, 'node_modules', ...name.split('/'));
    fs.cpSync(newer, destination, { recursive: true });
  } });
  assert.equal(result.installed, true);
  assert.equal(runtime.metadata(runtime.selectPackage(old)).version, '0.2.1');
  assert.equal(fs.readFileSync(path.join(cachedOld, 'native/expert-tui.exe'), 'utf8'), 'fixture 0.2.0');
  const queried = spawnSync(process.execPath, [path.join(old, 'bin/expert.cjs'), '--version'], { encoding: 'utf8' });
  assert.equal(queried.stdout.trim(), 'expert 0.2.1');
});
test('a failed or tampered download preserves the current version and clears update lock', async () => {
  const bad = fixture('0.2.2');
  fs.appendFileSync(path.join(bad, 'native/expert-tui.exe'), 'tampered');
  await assert.rejects(updater.update(old, { install: true, getLatest: async () => '0.2.2', runNpm: async args => {
    const prefix = args[args.indexOf('--prefix') + 1];
    fs.cpSync(bad, path.join(prefix, 'node_modules', ...name.split('/')), { recursive: true });
  } }), /integrity/);
  assert.equal(runtime.metadata(runtime.selectPackage(old)).version, '0.2.1');
  assert.equal(fs.existsSync(path.join(runtime.home(name), 'update.lock')), false);
  await assert.rejects(updater.update(old, { install: true, getLatest: async () => '0.2.2', runNpm: async () => { throw new Error('offline'); } }), /offline/);
  assert.equal(runtime.metadata(runtime.selectPackage(old)).version, '0.2.1');
});
test('concurrent update cannot replace a staged/active version', () => {
  const cache = runtime.home(name), unlock = runtime.lock(cache);
  try { assert.throws(() => runtime.lock(cache), /Another Expert/); }
  finally { unlock(); }
});
test('registry lookup validates package identity and reports unpublished or offline sources', async () => {
  let body = { name, version: '0.2.3' }, status = 200;
  const server = http.createServer((_, response) => { response.writeHead(status, { 'Content-Type': 'application/json' }); response.end(JSON.stringify(body)); });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    const base = `http://127.0.0.1:${server.address().port}/`;
    assert.equal(await updater.latestVersion(name, base), '0.2.3');
    body = { name: '@other/expert', version: '0.2.3' };
    await assert.rejects(updater.latestVersion(name, base), /Invalid npm/);
    status = 404;
    await assert.rejects(updater.latestVersion(name, base), /not been published/);
  } finally { await new Promise(resolve => server.close(resolve)); }
});
test.after(() => runtime.removeTemporary(os.tmpdir(), root, 'expert-npm-test-'));
