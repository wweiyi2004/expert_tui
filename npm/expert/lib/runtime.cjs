'use strict';
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');

const VERSION = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const NAME = /^@[a-z0-9][a-z0-9._-]*\/expert-tui$/;
const ROOT_FILES = ['bin', 'lib', 'native', 'expert.cmd', 'package.json', 'release-manifest.json', 'README.md', 'LICENSE'];
function readJson(file) { return JSON.parse(fs.readFileSync(file, 'utf8').replace(/^\uFEFF/, '')); }
function metadata(root) {
  const pkg = readJson(path.join(root, 'package.json'));
  if (!NAME.test(pkg.name) || !VERSION.test(pkg.version)) throw new Error('Invalid Expert package identity');
  return pkg;
}
function compareVersions(a, b) {
  if (!VERSION.test(a) || !VERSION.test(b)) throw new Error('Expected a stable semantic version');
  const aa = a.split('.').map(Number), bb = b.split('.').map(Number);
  for (let i = 0; i < 3; i++) if (aa[i] !== bb[i]) return aa[i] > bb[i] ? 1 : -1;
  return 0;
}
function home(name) {
  const explicit = process.env.EXPERT_CLI_HOME;
  if (explicit && !path.isAbsolute(explicit)) throw new Error('EXPERT_CLI_HOME must be absolute');
  const base = explicit || path.join(process.env.LOCALAPPDATA || os.homedir(), 'expert-tui', 'cli');
  if (!path.isAbsolute(base)) throw new Error('Expert cache directory must be absolute');
  return path.join(base, crypto.createHash('sha256').update(name).digest('hex').slice(0, 16));
}
function within(root, relative) {
  if (typeof relative !== 'string' || relative.includes('\\') || relative.includes(':') ||
      relative.split('/').some(p => !p || p === '.' || p === '..') || path.isAbsolute(relative)) {
    throw new Error('Invalid package path');
  }
  const target = path.resolve(root, relative);
  if (!path.relative(root, target) || path.relative(root, target).startsWith('..')) throw new Error('Package path escaped root');
  return target;
}
function walk(root, relative = '') {
  const result = [];
  for (const entry of fs.readdirSync(path.join(root, relative), { withFileTypes: true })) {
    const rel = relative ? `${relative}/${entry.name}` : entry.name;
    if (entry.isSymbolicLink()) throw new Error('Package links are not allowed');
    if (entry.isDirectory()) result.push(...walk(root, rel));
    else if (entry.isFile()) result.push(rel);
    else throw new Error('Unsupported package entry');
  }
  return result;
}
function validatePackage(root, expectedName, expectedVersion, full = true) {
  const pkg = metadata(root);
  if (pkg.name !== expectedName || pkg.version !== expectedVersion) throw new Error('Downloaded package identity mismatch');
  if (Object.keys(pkg.dependencies || {}).length || Object.keys(pkg.optionalDependencies || {}).length) {
    throw new Error('This updater requires a self-contained release without npm runtime dependencies');
  }
  const manifest = readJson(path.join(root, 'release-manifest.json'));
  if (manifest.schema !== 1 || manifest.name !== pkg.name || manifest.version !== pkg.version ||
      manifest.platform !== 'win32-x64' || typeof manifest.files !== 'object' || !manifest.files) {
    throw new Error('Invalid release manifest');
  }
  for (const rel of ['native/expert-tui.exe', 'native/expert-desktop.exe', 'native/host/bin/expert-host.exe',
                     'native/host/lib/sqlite3.dll', 'native/expert-media-worker/expert-media-worker.exe',
                     'bin/expert.cjs', 'lib/cli.cjs', 'lib/update.cjs', 'lib/runtime.cjs', 'expert.cmd', 'package.json']) {
    if (!manifest.files[rel] || !fs.statSync(within(root, rel)).isFile()) throw new Error(`Incomplete release: ${rel}`);
  }
  if (full) {
    const actual = walk(root).filter(rel => rel !== 'release-manifest.json').sort();
    const declared = Object.keys(manifest.files).sort();
    if (JSON.stringify(actual) !== JSON.stringify(declared)) throw new Error('Release file inventory mismatch');
    for (const rel of declared) {
      const bytes = fs.readFileSync(within(root, rel)), item = manifest.files[rel];
      if (bytes.length !== item.bytes || crypto.createHash('sha256').update(bytes).digest('hex') !== item.sha256) {
        throw new Error(`Release integrity check failed: ${rel}`);
      }
    }
  }
  return manifest;
}
function atomicJson(file, value) {
  const temp = `${file}.${process.pid}.${crypto.randomUUID()}.tmp`;
  try {
    const fd = fs.openSync(temp, 'wx');
    try { fs.writeFileSync(fd, JSON.stringify(value, null, 2) + '\n'); fs.fsyncSync(fd); }
    finally { fs.closeSync(fd); }
    fs.renameSync(temp, file);
  } finally { if (fs.existsSync(temp)) fs.unlinkSync(temp); }
}
function lock(cache) {
  fs.mkdirSync(cache, { recursive: true });
  const file = path.join(cache, 'update.lock');
  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      const fd = fs.openSync(file, 'wx');
      fs.writeFileSync(fd, String(process.pid));
      return () => { fs.closeSync(fd); fs.unlinkSync(file); };
    } catch (error) {
      if (error.code !== 'EEXIST') throw error;
      const owner = Number(fs.readFileSync(file, 'utf8'));
      if (Number.isSafeInteger(owner) && owner > 0) {
        try { process.kill(owner, 0); }
        catch (check) { if (check.code === 'ESRCH') { fs.unlinkSync(file); continue; } }
      }
      throw new Error('Another Expert install/update is running; retry after it finishes');
    }
  }
  throw new Error('Unable to acquire update lock');
}
function releaseKey(root, version) {
  return `${version}-${crypto.createHash('sha256').update(fs.readFileSync(path.join(root, 'release-manifest.json'))).digest('hex').slice(0, 12)}`;
}
function removeTemporary(parent, temp, prefix) {
  const target = path.resolve(temp);
  if (path.dirname(target) !== path.resolve(parent) || !path.basename(target).startsWith(prefix) ||
      fs.lstatSync(target).isSymbolicLink()) throw new Error('Refusing to remove an unexpected temporary directory');
  fs.rmSync(target, { recursive: true });
}
function selectPackage(installedRoot) {
  const installed = metadata(installedRoot), cache = home(installed.name);
  try {
    const active = readJson(path.join(cache, 'active.json'));
    if (active.name !== installed.name || !VERSION.test(active.version) ||
        !/^[0-9]+\.[0-9]+\.[0-9]+-[a-f0-9]{12}$/.test(active.directory) ||
        compareVersions(active.version, installed.version) < 0) return installedRoot;
    const root = path.join(cache, 'releases', active.directory);
    validatePackage(root, installed.name, active.version, false);
    return root;
  } catch { return installedRoot; }
}
function stagePackage(source, cache) {
  const pkg = metadata(source);
  validatePackage(source, pkg.name, pkg.version);
  const key = releaseKey(source, pkg.version), releases = path.join(cache, 'releases');
  fs.mkdirSync(releases, { recursive: true });
  const target = path.join(releases, key);
  if (path.resolve(source) === path.resolve(target)) return target;
  if (fs.existsSync(target)) { validatePackage(target, pkg.name, pkg.version); return target; }
  const temp = fs.mkdtempSync(path.join(releases, '.staging-'));
  try {
    for (const rel of ROOT_FILES) fs.cpSync(path.join(source, rel), path.join(temp, rel), { recursive: true, dereference: false });
    validatePackage(temp, pkg.name, pkg.version);
    fs.renameSync(temp, target);
    return target;
  } finally { if (fs.existsSync(temp)) removeTemporary(releases, temp, '.staging-'); }
}
function activate(root, cache) {
  const pkg = metadata(root);
  atomicJson(path.join(cache, 'active.json'), { name: pkg.name, version: pkg.version, directory: path.basename(root) });
}
function cachedPackage(root) {
  const pkg = metadata(root), cache = home(pkg.name);
  if (path.dirname(path.dirname(root)) === cache) return root;
  const unlock = lock(cache);
  try { const target = stagePackage(root, cache); activate(target, cache); return target; }
  finally { unlock(); }
}
module.exports = { VERSION, NAME, metadata, compareVersions, home, within, walk, validatePackage,
  atomicJson, lock, stagePackage, activate, selectPackage, cachedPackage, removeTemporary };
