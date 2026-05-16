#!/usr/bin/env node
'use strict';

const { spawnSync } = require('node:child_process');
const { existsSync } = require('node:fs');
const { join } = require('node:path');
const { maybePromptGithubStar } = require('./star-prompt.cjs');

const root = join(__dirname, '..');
const built = join(root, 'target', 'release', process.platform === 'win32' ? 'ai-e.exe' : 'ai-e');

function log(message) {
  console.log(`[ai-e:postinstall] ${message}`);
}

async function main() {
  if (process.env.AI_E_SKIP_POSTINSTALL === '1') {
    log('skipping postinstall because AI_E_SKIP_POSTINSTALL is set');
    return 0;
  }

  if (process.env.AI_E_SKIP_BUILD === '1') {
    log('skipping native build because AI_E_SKIP_BUILD is set');
    return promptForStar(0);
  }

  if (existsSync(built)) {
    log(`native binary already exists: ${built}`);
    return promptForStar(0);
  }

  const cargo = spawnSync('cargo', ['--version'], { encoding: 'utf8', stdio: 'pipe' });
  if (cargo.status !== 0) {
    console.error('[ai-e:postinstall] cargo is required to build the npm package from source.');
    console.error('[ai-e:postinstall] Install Rust from https://rustup.rs, or set AI_E_SKIP_BUILD=1 and provide a built binary separately.');
    return 1;
  }

  log(`using ${cargo.stdout.trim()}`);
  const build = spawnSync('cargo', ['build', '--release', '--locked'], {
    cwd: root,
    stdio: 'inherit',
    env: process.env,
  });

  if (build.status !== 0) {
    console.error(`[ai-e:postinstall] cargo build failed with status ${build.status}`);
    return build.status || 1;
  }

  log(`native binary built: ${built}`);
  return promptForStar(0);
}

async function promptForStar(code) {
  try {
    await maybePromptGithubStar();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.warn(`[ai-e:postinstall] GitHub star prompt skipped: ${message}`);
  }
  return code;
}

main()
  .then((code) => process.exit(code))
  .catch((error) => {
    const message = error instanceof Error ? error.stack || error.message : String(error);
    console.error(`[ai-e:postinstall] ${message}`);
    process.exit(1);
  });
