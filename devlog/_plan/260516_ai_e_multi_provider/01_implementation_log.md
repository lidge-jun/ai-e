---
created: 2026-05-16
status: active
tags: [implementation, providers, verification]
---
# Implementation Log

## Scaffold

- Copied `/Users/jun/Developer/new/700_projects/claude-exec` to
  `/Users/jun/Developer/new/700_projects/ai-e`.
- Excluded `.git`, `target`, and unrelated local untracked files from the copy.
- Renamed npm package to `ai-e`.
- Renamed Rust package to `ai-exec`.
- Reduced package bins to the single public command `ai-e`.

## Provider Architecture

- Added `src/providers/` as the provider registry.
- Added Claude binary metadata in `src/providers/claude_code.rs`.
- Added `src/headless.rs` for non-PTY providers.
- Kept Claude on the copied PTY path.
- Routed Codex, Gemini, Grok, and Copilot through native headless CLI commands.

## Local CLI Help Findings

Safe help probes confirmed installed CLI surfaces:

- `codex exec` supports non-interactive execution, `--model`, `--json`,
  `--cd`, and `--dangerously-bypass-approvals-and-sandbox`.
- `gemini --prompt` supports headless mode, `--model`, `--output-format`, and
  `--approval-mode yolo`.
- `grok --single` supports single-turn mode, `--model`, `--output-format`, and
  `--always-approve`.
- `copilot --prompt` supports non-interactive mode, `--model`,
  `--output-format`, `--allow-all`, and `--stream`.
- `agbrowse --help` documents `web-ai`, `--vendor chatgpt`, `--model pro`, and
  GPT Pro effort aliases. `web-ai render` is the safe non-mutating validation
  surface.

## Verification Notes

- `Cargo.lock` was stale after renaming from `claude-exec` to `ai-exec`.
- Regenerated the lockfile with `cargo check`.
- Added builder tests for Codex `gpt-5-mini`, Gemini, Grok, and Copilot
  argument mapping.
- Removed copied `CLAUDE_EXEC_*` compatibility environment variables from the
  new package surface. `ai-e` now uses `AI_E_*` names plus generic
  `CLAUDE_BIN` for Claude binary resolution.
- Verified the full local hardening path:
  - `npm run verify`
  - `npm run publish:dry-run`
  - `npm run pack:dry`
  - `target/release/ai-e --help`
- Verified provider command construction with `/bin/echo`:
  - `ai-e codex --provider-bin /bin/echo --model gpt-5-mini --output-format json`
  - `ai-e gemini --provider-bin /bin/echo --model gemini-2.5-pro --output-format stream-json`
  - `ai-e grok --provider-bin /bin/echo --model auto --output-format stream-json`
  - `ai-e copilot --provider-bin /bin/echo --model gpt-5-mini --output-format stream-json`

## 2026-05-17 cli-jaw Hardening Alignment

Re-audited headless defaults against cli-jaw's live `src/agent/args.ts` launch
policy instead of relying on provider help alone.

Changes:

- Codex now adds `--skip-git-repo-check` in addition to
  `--dangerously-bypass-approvals-and-sandbox`.
- Gemini now uses `--approval-mode yolo` instead of the shorter `--yolo`
  spelling and injects home-root `--include-directories` values, matching
  cli-jaw's external-path access guard.
- Grok now adds `--no-alt-screen` alongside `--always-approve` and
  `--permission-mode bypassPermissions`.
- Copilot keeps `--allow-all --stream off` and the smoke/default model remains
  `gpt-5-mini`.
- Explicit caller-provided hardening flags are not duplicated; ai-e only fills
  missing unattended defaults.

Live smoke notes:

- All provider binaries were present locally:
  `codex`, `gemini`, `grok`, and `copilot`.
- Controlled `/bin/echo` smoke confirmed the exact provider args:
  - Codex: `exec --model gpt-5-mini --json --dangerously-bypass-approvals-and-sandbox --skip-git-repo-check ...`
  - Gemini: `--model gemini-3-flash-preview --prompt ... --output-format stream-json --skip-trust --approval-mode yolo --include-directories /Users/jun`
  - Grok: `--model grok-build --single ... --output-format streaming-json --no-alt-screen --always-approve --permission-mode bypassPermissions`
  - Copilot: `--model gpt-5-mini --prompt ... --output-format json --allow-all --stream off`
- Codex real smoke:
  - `gpt-5-mini` returned a provider-side unsupported-model error for the
    current ChatGPT-backed Codex account.
  - `gpt-5.4` succeeded with `SMOKE_AI_E_CODEX_REAL_2`.
- Gemini real smoke succeeded with `SMOKE_AI_E_GEMINI_REAL_1`; the CLI printed
  YOLO mode enabled and accepted the prompt non-interactively.
- Grok real smoke succeeded with `SMOKE_AI_E_GROK_REAL_1`.
- Copilot real smoke succeeded with `gpt-5-mini` and
  `SMOKE_AI_E_COPILOT_REAL_1`.

Shared Claude fix:

- `rate_limit_event` is now passed through by the transcript normalizer so
  Claude 429 wait events can reach cli-jaw instead of being discarded.

## 2026-05-17 Grok Shutdown Timing Comparison

Compared direct Grok CLI against the Rust `target/release/ai-e grok` wrapper
after a cli-jaw shutdown-delay report.

Environment:

- `grok 0.1.211 (2f2cd6d5c2)`
- model: `grok-build`
- direct output: `--output-format streaming-json`
- ai-e output: `--output-format stream-json`

Simple one-line answer prompt, 5 runs each:

| Path | `text -> end` avg | `end -> close` avg | `last event -> close` avg |
|------|-------------------|--------------------|---------------------------|
| direct `grok` | 0.045s | 0.375s | 0.375s |
| `ai-e grok` | 0.018s | 0.333s | 0.333s |

Tool-use prompt (`pwd` only, no file edits), 2 runs each:

| Path | `text -> end` range | `end -> close` range |
|------|---------------------|----------------------|
| direct `grok` | 0.003-0.028s | 0.215-0.342s |
| `ai-e grok` | 0.032-0.036s | 0.248-0.349s |

Conclusion:

- The current Grok CLI does not reproduce the earlier 3-9s shutdown delay.
- Rust `ai-e grok` does not add meaningful shutdown delay over direct `grok`.
- No ai-e Grok shutdown watchdog is justified now; use the existing headless
  hard timeout for hung processes and revisit only with new trace data.
