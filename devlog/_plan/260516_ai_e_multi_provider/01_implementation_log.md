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
  `--yolo`.
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
