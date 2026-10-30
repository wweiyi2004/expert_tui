#!/usr/bin/env node
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { execFile } = require('node:child_process');
const { promisify } = require('node:util');
const runtime = require('./runtime.cjs');
const exec = promisify(execFile);
function registry() {
  const url = new URL(process.env.EXPERT_UPDATE_REGISTRY || 'https://registry.npmjs.org/');
  if (url.username || url.password || url.search || url.hash ||
      (url.protocol !== 'https:' && !(url.protocol === 'http:' && ['127.0.0.1', '[::1]', 'localhost'].includes(url.hostname)))) {
    throw new Error('Update registry must be HTTPS (HTTP allowed only for loopback tests)');
  }
  return url.href.replace(/\/?$/, '/');
}
function npmCli() {
  const candidates = [process.env.npm_execpath,
    path.join(path.dirname(process.execPath), 'node_modules', 'npm', 'bin', 'npm-cli.js')];
  for (const dir of (process.env.PATH || '').split(path.delimiter)) {
    if (dir) candidates.push(path.join(dir, 'node_modules', 'npm', 'bin', 'npm-cli.js'));
  }
  const found = candidates.find(file => file && path.isAbsolute(file) && path.basename(file) === 'npm-cli.js' && fs.existsSync(file));
  if (!found) throw new Error('npm CLI not found; install Node.js with npm and retry');
  return found;
}
async function latestVersion(name, base = registry()) {
  const response = await fetch(`${base}${encodeURIComponent(name)}/latest`, { signal: AbortSignal.timeout(15000), redirect: 'error' });
  if (response.status === 404) throw new Error(`${name} has not been published to this registry yet`);
  if (!response.ok) throw new Error(`Update registry returned HTTP ${response.status}`);
  const body = await response.text();
  if (body.length > 1024 * 1024) throw new Error('Registry response exceeds limit');
  const info = JSON.parse(body);
  if (info.name !== name || !runtime.VERSION.test(info.version)) throw new Error('Invalid npm release metadata');
  if (info.os && !info.os.includes('win32') || info.cpu && !info.cpu.includes('x64')) throw new Error('Release does not support Windows x64');
  return info.version;
}
async function update(root, { install = false, getLatest = latestVersion, runNpm } = {}) {
  const pkg = runtime.metadata(root), cache = runtime.home(pkg.name), base = registry();
  const currentRoot = runtime.selectPackage(root), current = runtime.metadata(currentRoot).version;
  const latest = await getLatest(pkg.name, base);
  const available = runtime.compareVersions(latest, current) > 0;
  const result = { name: pkg.name, current, latest, available, installed: false,
    message: available ? `发现新版 ${latest}；/update 安装，或运行 expert update` : `当前已是最新版本 ${current}` };
  if (!available || !install) return result;
  const unlock = runtime.lock(cache);
  let temp;
  try {
    temp = fs.mkdtempSync(path.join(cache, '.download-'));
    const args = ['install', '--prefix', temp, '--ignore-scripts', '--no-audit', '--no-fund', '--no-bin-links',
      '--package-lock=false', '--save=false', '--omit=dev', '--fetch-retries=1', '--fetch-timeout=30000',
      '--cache', path.join(temp, 'npm-cache'),
      '--registry', base, `--${pkg.name.split('/')[0]}:registry=${base}`, `${pkg.name}@${latest}`];
    if (runNpm) await runNpm(args, temp);
    else await exec(process.execPath, [npmCli(), ...args], { cwd: temp, windowsHide: true, timeout: 240000,
      maxBuffer: 2 * 1024 * 1024, env: { ...process.env, npm_config_update_notifier: 'false' } });
    const downloaded = path.join(temp, 'node_modules', ...pkg.name.split('/'));
    runtime.validatePackage(downloaded, pkg.name, latest);
    const target = runtime.stagePackage(downloaded, cache);
    runtime.activate(target, cache);
    return { ...result, installed: true, message: `已安装 ${latest}，下次启动 expert 使用新版。当前会话和 Host 继续运行。` };
  } catch (error) {
    if (error.killed || error.code === 'ETIMEDOUT') throw new Error('Update timed out; current version is unchanged');
    if (typeof error.stderr === 'string' && /Z_DATA_ERROR|incorrect data check/.test(error.stderr)) {
      throw new Error('npm 解压校验失败，当前版本保留。可切换 Node.js 22 LTS 后重试');
    }
    // npm stderr can contain registry credentials; keep raw command output private.
    if (error.cmd) throw new Error('npm download failed; current version is unchanged. Check registry/network access and retry');
    throw error;
  } finally {
    try { if (temp && fs.existsSync(temp)) runtime.removeTemporary(cache, temp, '.download-'); }
    finally { unlock(); }
  }
}
async function main(root, args) {
  const json = args.includes('--json');
  try {
    if (args.some(arg => !['check', 'install', '--check', '--json'].includes(arg))) throw new Error('Usage: expert update [--check] [--json]');
    const result = await update(root, { install: args.includes('install') || !args.includes('check') && !args.includes('--check') });
    console.log(json ? JSON.stringify({ ok: true, ...result }) : result.message);
    return 0;
  } catch (error) {
    if (json) console.log(JSON.stringify({ ok: false, message: error.message }));
    else console.error(`expert update: ${error.message}`);
    return 1;
  }
}
module.exports = { registry, npmCli, latestVersion, update, main };
if (require.main === module) {
  const root = process.env.EXPERT_PACKAGE_ROOT || path.resolve(__dirname, '..');
  main(root, process.argv.slice(2)).then(code => { process.exitCode = code; });
}
