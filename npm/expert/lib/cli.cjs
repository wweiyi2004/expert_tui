'use strict';
const path = require('node:path');
const { spawn } = require('node:child_process');
const runtime = require('./runtime.cjs');
async function main(root, args) {
  const pkg = runtime.metadata(root);
  if (['--version', '-V', 'version'].includes(args[0])) {
    const json = args.includes('--json');
    console.log(json ? JSON.stringify({ name: pkg.name, version: pkg.version, platform: 'win32-x64' }) : `expert ${pkg.version}`);
    return;
  }
  if (process.platform !== 'win32' || process.arch !== 'x64') throw new Error('This release supports Windows x64 only');
  if (args[0] === 'update') {
    process.exitCode = await require('./update.cjs').main(root, args.slice(1));
    return;
  }
  // Running EXEs live outside node_modules, so npm can replace its package while
  // a TUI or shared Host keeps using an older, immutable release directory.
  const cached = runtime.cachedPackage(root), native = path.join(cached, 'native');
  const child = spawn(path.join(native, 'expert-tui.exe'), args, {
    stdio: 'inherit', cwd: process.cwd(), windowsHide: false,
    env: { ...process.env, EXPERT_PACKAGE_ROOT: cached,
      EXPERT_UPDATE_SCRIPT: path.join(cached, 'lib', 'update.cjs'), EXPERT_NODE_EXE: process.execPath,
      EXPERT_CHAT_HOST: process.env.EXPERT_CHAT_HOST || path.join(native, 'host', 'bin', 'expert-host.exe') },
  });
  child.on('error', error => { console.error(`expert: ${error.message}`); process.exitCode = 1; });
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => { /* foreground native client handles terminal signals */ });
  child.on('exit', (code, signal) => { process.exitCode = code ?? (signal ? 1 : 0); });
}
module.exports = { main };
