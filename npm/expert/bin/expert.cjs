#!/usr/bin/env node
'use strict';
const path = require('node:path');
const { selectPackage } = require('../lib/runtime.cjs');
try {
  const selected = selectPackage(path.resolve(__dirname, '..'));
  Promise.resolve(require(path.join(selected, 'lib', 'cli.cjs')).main(selected, process.argv.slice(2)))
    .catch(error => { console.error(`expert: ${error.message}`); process.exitCode = 1; });
} catch (error) {
  console.error(`expert: ${error.message}`);
  process.exitCode = 1;
}
