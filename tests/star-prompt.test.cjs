'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { mkdtemp, rm } = require('node:fs/promises');
const { tmpdir } = require('node:os');
const { join } = require('node:path');

const {
  maybePromptGithubStar,
  shouldSkipStarPrompt,
  starPromptStatePath,
  starRepo,
  isGhInstalled,
} = require('../scripts/star-prompt.cjs');

test('starPromptStatePath honors AI_E_HOME', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'ai-e-star-home-'));
  try {
    assert.equal(starPromptStatePath({ AI_E_HOME: dir }), join(dir, 'state', 'star-prompt.json'));
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('shouldSkipStarPrompt skips CI and explicit postinstall opt-out', () => {
  assert.equal(shouldSkipStarPrompt({ CI: 'true' }), true);
  assert.equal(shouldSkipStarPrompt({ AI_E_SKIP_STAR_PROMPT: '1' }), true);
  assert.equal(shouldSkipStarPrompt({ npm_config_ai_e_skip_star_prompt: 'true' }), true);
  assert.equal(shouldSkipStarPrompt({}), false);
});

test('starRepo calls gh starred API with hidden Windows console', () => {
  let seenCommand = '';
  let seenArgs = [];
  let seenOptions;
  const result = starRepo((command, args, options) => {
    seenCommand = command;
    seenArgs = args;
    seenOptions = options;
    return {
      status: 0,
      signal: null,
      error: undefined,
      stdout: '',
      stderr: '',
      output: [],
      pid: 1,
    };
  });

  assert.deepEqual(result, { ok: true });
  assert.equal(seenCommand, 'gh');
  assert.deepEqual(seenArgs, ['api', '-X', 'PUT', '/user/starred/lidge-jun/ai-e']);
  assert.equal(seenOptions.windowsHide, true);
});

test('maybePromptGithubStar prints install-time URL for non-TTY sessions', async () => {
  const logs = [];
  let marked = false;

  await maybePromptGithubStar({
    env: {},
    stdinIsTTY: false,
    stdoutIsTTY: false,
    hasBeenPromptedFn: () => false,
    isGhInstalledFn: () => false,
    markPromptedFn: () => { marked = true; },
    logFn: (message) => logs.push(message),
  });

  assert.equal(marked, true);
  assert.ok(logs.some((line) => line.includes('https://github.com/lidge-jun/ai-e')));
  assert.ok(logs.some((line) => line.includes('non-interactive')));
});

test('maybePromptGithubStar asks and stars in interactive gh sessions', async () => {
  const logs = [];
  let marked = false;
  let starred = false;

  await maybePromptGithubStar({
    env: {},
    stdinIsTTY: true,
    stdoutIsTTY: true,
    hasBeenPromptedFn: () => false,
    isGhInstalledFn: () => true,
    markPromptedFn: () => { marked = true; },
    askYesNoFn: async () => true,
    starRepoFn: () => {
      starred = true;
      return { ok: true };
    },
    logFn: (message) => logs.push(message),
  });

  assert.equal(marked, true);
  assert.equal(starred, true);
  assert.deepEqual(logs, ['Thanks for the star!']);
});

test('maybePromptGithubStar defers to the user when an agent drives the install', async () => {
  const logs = [];
  let marked = false;
  let asked = false;
  let starred = false;

  await maybePromptGithubStar({
    env: { CODEX_THREAD_ID: '019fa50b' },
    stdinIsTTY: true,
    stdoutIsTTY: true,
    hasBeenPromptedFn: () => false,
    isGhInstalledFn: () => true,
    markPromptedFn: () => { marked = true; },
    askYesNoFn: async () => { asked = true; return true; },
    starRepoFn: () => { starred = true; return { ok: true }; },
    logFn: (message) => logs.push(message),
  });

  // The agent must not answer, and must not spend the user's GitHub identity.
  assert.equal(asked, false);
  assert.equal(starred, false);
  // The one-time state stays unwritten so the user still sees the real prompt.
  assert.equal(marked, false);
  assert.ok(logs.some((line) => line.includes('do not answer this yourself')));
  assert.ok(logs.some((line) => line.includes('Ask the user whether to star')));
});

test('isGhInstalled requires an authenticated gh, not just an installed one', () => {
  const calls = [];
  const spawnSyncFn = (_command, args) => {
    calls.push(args.join(' '));
    // `gh --version` succeeds, `gh auth status` reports logged out.
    return args[0] === '--version' ? { status: 0 } : { status: 1 };
  };

  assert.equal(isGhInstalled(spawnSyncFn), false);
  assert.deepEqual(calls, ['--version', 'auth status']);
});

test('interactiveConfirm answers on arrow keys, y/n, and a bare enter', async () => {
  const { PassThrough } = require('node:stream');
  const { interactiveConfirm } = require('../scripts/interactive-confirm.cjs');

  const ask = async (keys, defaultYes = true) => {
    const input = new PassThrough();
    input.isRaw = false;
    input.setRawMode = (mode) => { input.isRaw = mode; return input; };
    const output = new PassThrough();
    const painted = [];
    const write = output.write.bind(output);
    output.write = (chunk, ...rest) => {
      painted.push(typeof chunk === 'string' ? chunk : Buffer.from(chunk).toString('utf8'));
      return write(chunk, ...rest);
    };

    const pending = interactiveConfirm({ question: 'Star it?', defaultYes, input, output });
    for (const key of keys) input.write(key);
    return { answer: await pending, painted: painted.join(''), raw: input.isRaw };
  };

  assert.equal((await ask(['\r'], true)).answer, true);
  assert.equal((await ask(['\r'], false)).answer, false);
  assert.equal((await ask(['\x1b[C', '\r'])).answer, false); // right → No
  assert.equal((await ask(['n'])).answer, false);
  assert.equal((await ask(['y'], false)).answer, true);
  assert.equal((await ask(['\x1b'], true)).answer, false); // escape declines

  const shown = await ask(['\r']);
  assert.ok(shown.painted.includes('Yes'));
  assert.ok(shown.painted.includes('No'));
  assert.equal(shown.raw, false);
});
