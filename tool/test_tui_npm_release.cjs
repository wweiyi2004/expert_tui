'use strict';
// Actual npm install and registry update against a loopback registry. No publish,
// paid model or production profile is touched.
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');
const { execFile, spawn } = require('node:child_process');
const crypto = require('node:crypto');
const { promisify } = require('node:util');
const exec = promisify(execFile);
const repository = path.resolve(__dirname, '..');
const runtime = require('../npm/expert/lib/runtime.cjs');
const { npmCli } = require('../npm/expert/lib/update.cjs');

async function main() {
  const release = JSON.parse(fs.readFileSync(path.join(repository, 'output/tui-npm-build/latest.json'), 'utf8'));
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'expert-npm-acceptance-'));
  const prefix = path.join(root, 'global prefix');
  const source = release.package;
  const pkg = runtime.metadata(source);
  let base, tarballRequests = 0;
  const server = http.createServer((request, response) => {
    if (request.url === '/package.tgz') {
      tarballRequests++;
      response.writeHead(200, { 'Content-Type': 'application/octet-stream' });
      fs.createReadStream(release.tgz).pipe(response);
      return;
    }
    const info = { ...pkg, dist: { tarball: `${base}package.tgz`, integrity: release.npm_integrity } };
    const requested = decodeURIComponent(request.url).replace(/\/$/, '');
    response.setHeader('Content-Type', 'application/json');
    if (requested === `/${pkg.name}/latest`) response.end(JSON.stringify(info));
    else if (requested === `/${pkg.name}`) response.end(JSON.stringify({ name: pkg.name,
      'dist-tags': { latest: pkg.version }, versions: { [pkg.version]: info } }));
    else { response.writeHead(404); response.end('{"error":"not found"}'); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  base = `http://127.0.0.1:${server.address().port}/`;
  const env = { ...process.env, EXPERT_CLI_HOME: path.join(root, 'version cache'), EXPERT_UPDATE_REGISTRY: base,
    EXPERT_HOST_DATA_DIR: path.join(root, 'state'), npm_config_cache: path.join(root, 'npm cache') };
  const command = (args, extraEnv = {}) => exec(process.execPath, args, { cwd: root, env: { ...env, ...extraEnv },
    windowsHide: true, timeout: 120000, maxBuffer: 2 * 1024 * 1024 });
  const report = { passed: false, published: false, paid_requests: 0, checks: [] };
  let oldHost;
  try {
    await command([npmCli(), 'install', '--global', '--prefix', prefix, '--ignore-scripts', '--no-audit', '--no-fund', release.tgz]);
    const installed = path.join(prefix, 'node_modules', ...pkg.name.split('/'));
    runtime.validatePackage(installed, pkg.name, pkg.version);
    if (!fs.existsSync(path.join(prefix, 'expert-tui.cmd'))) throw new Error('npm did not create the expert command shim');
    const entry = path.join(installed, 'bin/expert.cjs');
    const version = await command([entry, 'version', '--json']);
    if (JSON.parse(version.stdout).version !== pkg.version) throw new Error('Installed version mismatch');
    report.checks.push('npm tarball installs globally into a prefix containing spaces and creates expert-tui.cmd');
    const snapshot = await command([entry, '--snapshot', 'welcome', '--width', '60', '--height', '24']);
    if (!snapshot.stdout.trim()) throw new Error('Native offline render failed');
    if (fs.existsSync(path.join(root, 'state/host/discovery/bridge.json'))) throw new Error('Offline rendering started Host');
    report.checks.push('installed npm launcher runs the real native TUI without GUI or a running Host');
    const fixture = path.join(root, 'previous fixture');
    fs.cpSync(installed, fixture, { recursive: true });
    fs.writeFileSync(path.join(fixture, 'package.json'), JSON.stringify({ ...pkg, version: '0.1.9' }));
    const oldManifest = JSON.parse(fs.readFileSync(path.join(fixture, 'release-manifest.json'), 'utf8'));
    oldManifest.version = '0.1.9';
    const oldPackageJson = fs.readFileSync(path.join(fixture, 'package.json'));
    oldManifest.files['package.json'] = { bytes: oldPackageJson.length, sha256: crypto.createHash('sha256').update(oldPackageJson).digest('hex') };
    fs.writeFileSync(path.join(fixture, 'release-manifest.json'), JSON.stringify(oldManifest));
    const updaterEntry = path.join(fixture, 'bin/expert.cjs');
    // Use a second cache so this fixture initially represents an older install.
    const updateEnv = { EXPERT_CLI_HOME: path.join(root, 'update cache') };
    const prepared = await command(['-e', 'console.log(require(process.argv[1]).cachedPackage(process.argv[2]))',
      path.join(fixture, 'lib/runtime.cjs'), fixture], updateEnv);
    const oldCached = prepared.stdout.trim();
    const hostState = path.join(root, 'locked-host-state');
    oldHost = spawn(path.join(oldCached, 'native/host/bin/expert-host.exe'), ['--data-dir', hostState, '--idle-seconds', '0'],
      { cwd: root, env, stdio: 'ignore', windowsHide: true });
    const readyDeadline = Date.now() + 20000;
    while (!fs.existsSync(path.join(hostState, 'host/discovery/gui.json'))) {
      if (oldHost.exitCode !== null || Date.now() > readyDeadline) throw new Error('Isolated locked Host did not start');
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    const check = JSON.parse((await command([updaterEntry, 'update', '--check', '--json'], updateEnv)).stdout);
    if (!check.available || check.installed) throw new Error('Read-only update check failed');
    const before = await command([updaterEntry, '--version'], updateEnv);
    if (before.stdout.trim() !== 'expert 0.1.9') throw new Error('Update check changed the active version');
    const update = JSON.parse((await command([updaterEntry, 'update', '--json'], updateEnv)).stdout);
    if (!update.installed || !update.ok || tarballRequests < 1) throw new Error('Real npm registry update failed');
    const after = await command([updaterEntry, '--version'], updateEnv);
    if (after.stdout.trim() !== `expert ${pkg.version}`) throw new Error('Bootstrap did not dispatch to the activated version');
    const newSnapshot = await command([updaterEntry, '--snapshot', 'welcome', '--width', '40', '--height', '24'], updateEnv);
    if (!newSnapshot.stdout.trim()) throw new Error('Updated native bundle failed to launch');
    if (oldHost.exitCode !== null || !fs.existsSync(path.join(oldCached, 'native/host/bin/expert-host.exe'))) {
      throw new Error('Update disrupted the running old Host');
    }
    report.checks.push('expert update downloads through real npm, checks integrity, activates the new JS/native bundle, and next launch uses it');
    report.checks.push('a real old Host keeps running with its EXE locked while the new version is installed');
    const activeCache = path.join(root, 'update cache');
    const cacheName = fs.readdirSync(activeCache)[0];
    const active = JSON.parse(fs.readFileSync(path.join(activeCache, cacheName, 'active.json'), 'utf8'));
    const cached = path.join(activeCache, cacheName, 'releases', active.directory);
    const result = await exec(path.join(cached, 'native/expert-tui.exe'), ['update', '--check', '--json'], {
      cwd: root, env: { ...env, ...updateEnv, EXPERT_PACKAGE_ROOT: cached, EXPERT_NODE_EXE: process.execPath,
        EXPERT_UPDATE_SCRIPT: path.join(cached, 'lib/update.cjs') }, windowsHide: true, timeout: 30000 });
    if (JSON.parse(result.stdout).latest !== pkg.version) throw new Error('Native updater bridge failed');
    report.checks.push('native expert-tui update bridge returns valid status without starting Host');
    report.passed = true;
  } finally {
    if (oldHost && oldHost.exitCode === null) {
      oldHost.kill();
      await new Promise(resolve => oldHost.once('exit', resolve));
    }
    await new Promise(resolve => server.close(resolve));
    const reportPath = path.join(path.dirname(release.tgz), 'npm-acceptance.json');
    fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
    if (report.passed) runtime.removeTemporary(os.tmpdir(), root, 'expert-npm-acceptance-');
    else console.error(`Isolated evidence retained: ${root}`);
  }
  console.log(JSON.stringify(report, null, 2));
}
main().catch(error => { console.error(error); process.exitCode = 1; });
