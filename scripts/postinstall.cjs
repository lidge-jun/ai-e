#!/usr/bin/env node
'use strict';

const { spawnSync } = require('node:child_process');
const { existsSync, mkdirSync, copyFileSync, chmodSync } = require('node:fs');
const { join, dirname } = require('node:path');
const { maybePromptGithubStar } = require('./star-prompt.cjs');

const root = join(__dirname, '..');
const ext = process.platform === 'win32' ? '.exe' : '';
const built = join(root, 'target', 'release', `ai-e${ext}`);
const BINARIES = ['ai-e'];

function log(message) {
  console.log(`[ai-e:postinstall] ${message}`);
}

function platformPackageName() {
  const platform = process.platform;
  const arch = process.arch;
  const key = `${platform}-${arch}`;
  const supported = ['darwin-arm64', 'darwin-x64', 'linux-x64', 'linux-arm64'];
  if (!supported.includes(key)) return null;
  return `@bitkyc08/ai-e-${key}`;
}

function tryResolvePrebuilt() {
  const pkg = platformPackageName();
  if (!pkg) return false;

  let pkgDir;
  try {
    const pkgJson = require.resolve(`${pkg}/package.json`);
    pkgDir = dirname(pkgJson);
  } catch {
    return false;
  }

  const srcBin = join(pkgDir, 'bin', `ai-e${ext}`);
  if (!existsSync(srcBin)) return false;

  const targetDir = join(root, 'target', 'release');
  mkdirSync(targetDir, { recursive: true });

  let copied = 0;
  for (const name of BINARIES) {
    const src = join(pkgDir, 'bin', `${name}${ext}`);
    const dst = join(targetDir, `${name}${ext}`);
    if (existsSync(src)) {
      copyFileSync(src, dst);
      chmodSync(dst, 0o755);
      copied++;
    }
  }

  if (copied > 0) {
    log(`resolved ${copied} prebuilt binaries from ${pkg}`);
    return true;
  }
  return false;
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

  try {
    if (tryResolvePrebuilt()) {
      return promptForStar(0);
    }
  } catch (err) {
    log(`prebuilt resolution failed: ${err instanceof Error ? err.message : err}`);
  }

  const cargo = spawnSync('cargo', ['--version'], { encoding: 'utf8', stdio: 'pipe' });
  if (cargo.status !== 0) {
    log('cargo not found — skipping native build.');
    log('  The ai-e binary will not be available until Rust is installed.');
    log('  Install Rust: https://rustup.rs');
    return promptForStar(0);
  }

  log(`using ${cargo.stdout.trim()}`);
  const build = spawnSync('cargo', ['build', '--release', '--locked'], {
    cwd: root,
    stdio: 'inherit',
    env: process.env,
  });

  if (build.status !== 0) {
    log(`cargo build failed (status ${build.status}) — skipping native build.`);
    log('  The ai-e binary will not be available. Re-run: npm rebuild @bitkyc08/ai-e');
    return promptForStar(0);
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
    process.exit(0);
  });
