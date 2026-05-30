# ai-e

[![Tests](https://github.com/lidge-jun/ai-e/actions/workflows/test.yml/badge.svg)](https://github.com/lidge-jun/ai-e/actions/workflows/test.yml)
[![npm version](https://img.shields.io/npm/v/@bitkyc08/ai-e.svg)](https://www.npmjs.com/package/@bitkyc08/ai-e)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/runtime-Rust-orange.svg)](Cargo.toml)

`ai-e` is a modular PTY exec layer for AI CLIs.

The goal is one installable command that can drive Claude Code, Codex, Gemini,
Grok, Antigravity (agy), and future interactive agent CLIs through the same
process contract:

```bash
npm install -g @bitkyc08/ai-e

ai-e claude "your prompt here"
ai-e codex --model gpt-5-mini "summarize this repo"
ai-e grok --model auto "explain quicksort"
ai-e agy "respond with hello world"
ai-e kiro --model auto "summarize this repo"
```

Providers run in two modes:

- **Interactive (default for codex/grok)**: Spawn the provider's TUI in a PTY,
  inject prompt via bracketed paste, tail the session file for structured JSONL
  output, detect completion via quiescence, and emit a session footer for resume.
- **Headless**: One-shot prompt execution via native CLI flags (legacy `-p` path,
  still default for agy/kiro).

## Why This Exists

Interactive AI CLIs are useful because they carry auth, tools, local settings,
and provider-specific behavior. Agent systems need a different shape: spawn one
process, send one prompt, stream progress, classify failures, and return a
stable result object.

`ai-e` keeps those concerns separate:

| Layer | Responsibility |
|---|---|
| Core PTY runtime | Spawn, prompt injection, terminal responses, timeouts, cleanup. |
| Provider adapter | Binary resolution, args, trust/permission handling, transcript source. |
| Normalizer | Convert provider transcript/log output into common text/json/stream-json. |
| CLI surface | `ai-e <provider> ...` command parsing for cli-jaw and humans. |
| Packaging | npm install, native Rust build, local release scripts, CI dry-runs. |

## Provider Status

| Provider | Command | Mode | Notes |
|---|---|---|---|
| Claude Code | `ai-e claude ...` | PTY interactive | Copied from the hardened `claude-e` runtime. Hook-based completion. |
| Codex CLI | `ai-e codex ...` | Interactive (default) | TUI in PTY, session file tail, resume via session ID. |
| Gemini CLI | `ai-e gemini ...` | Interactive | Uses `gemini` TUI with session JSONL tail. |
| Grok CLI | `ai-e grok ...` | Interactive (default) | TUI in PTY, `chat_history.jsonl` tail, auto-completion. |
| Copilot CLI | `ai-e copilot ...` | Interactive | Uses `copilot` TUI with sqlite session store. |
| Kiro CLI | `ai-e kiro ...` | Pipe (headless) | `kiro-cli chat --no-interactive`; PTY hangs for kiro. |
| Antigravity | `ai-e agy ...` | Headless (default) | `agy -p`; interactive opt-in via `--interactive`. |

## Install

```bash
npm install -g @bitkyc08/ai-e
```

Optional one-shot usage:

```bash
npx @bitkyc08/ai-e claude "your prompt here"
```

From source:

```bash
git clone https://github.com/lidge-jun/ai-e.git
cd ai-e
npm install
cargo build --release --locked
```

The npm package builds the Rust release binary during `postinstall`. It also
asks once for a GitHub star when npm is running interactively with an
authenticated `gh` CLI; non-interactive installs print the repository URL
instead. Set `AI_E_SKIP_STAR_PROMPT=1` to suppress the star request.
Set `AI_E_SKIP_POSTINSTALL=1` to skip all postinstall work. Set
`AI_E_SKIP_BUILD=1` only when another packaging layer provides the binary; the
star request can still run in that mode unless it is skipped separately.

## Command Surface

Provider-explicit form:

```bash
ai-e claude "your prompt here"
ai-e claude p "your prompt here"
ai-e claude print "your prompt here"
ai-e claude -p "your prompt here"
ai-e claude run --claude-bin "$(command -v claude)" -- --model opus
ai-e codex --model gpt-5-mini "summarize this repo"
ai-e gemini --model gemini-2.5-pro "summarize this repo"
ai-e grok --model auto "summarize this repo"
ai-e copilot --model gpt-5-mini "summarize this repo"
```

Bootstrap shorthand:

```bash
ai-e "your prompt here"
```

The shorthand currently defaults to `claude` so the copied runtime remains easy
to smoke-test. New cli-jaw integration should pass the provider explicitly.

## Claude Provider

The Claude provider is PTY-backed. It preserves the useful parts of
`claude -p` command shape while driving the interactive Claude Code runtime.

Examples:

```bash
ai-e claude "write a two-line commit summary"
ai-e claude --tool "use 10 tools and summarize what happened"
ai-e claude --output-format json "summarize this staged diff" < diff.patch
ai-e claude --output-format stream-json "audit src/" --verbose | jq .
```

Claude binary resolution:

1. `AI_E_CLAUDE_BIN`
2. `CLAUDE_BIN`
3. `claude`

Unless the caller already supplied `--permission-mode`,
`--permission-mode=...`, `--dangerously-skip-permissions`, or
`--allow-dangerously-skip-permissions`, the Claude provider appends
`--dangerously-skip-permissions` to avoid unattended tool runs hanging on
permission prompts.

The Claude provider uses activity-aware PTY timeouts. `--idle-timeout-ms`
expires only after no transcript activity is observed for the configured
window, and active Claude tool calls suppress idle timeout until tool results
arrive. `--hard-timeout-ms` remains the absolute process cap. The legacy
`--timeout-ms` flag is treated as an idle-timeout alias.

## PTY Prompt-Mode Providers

Codex, Gemini, Grok, and Copilot route through the wrapper PTY supervisor but
submit the prompt through each provider's native one-shot/prompt flag.
They use the safest documented non-interactive surfaces from their installed
CLI help:

| Provider | Underlying command shape | Default hardening |
|---|---|---|
| Codex | `codex exec ... <prompt>` | Adds `--dangerously-bypass-approvals-and-sandbox` unless the caller supplied sandbox/approval flags, plus `--skip-git-repo-check`. |
| Gemini | `gemini --prompt <prompt>` | Adds `--skip-trust`, `--approval-mode yolo`, and home-root `--include-directories` values. |
| Grok | `grok --single <prompt>` | Adds `--no-alt-screen`, `--always-approve`, and `--permission-mode bypassPermissions`. |
| Copilot | `copilot --prompt <prompt>` | Adds `--allow-all --stream off`. |

Provider binary overrides:

| Provider | Env vars |
|---|---|
| Codex | `AI_E_CODEX_BIN`, then `CODEX_BIN`, then `codex` |
| Gemini | `AI_E_GEMINI_BIN`, then `GEMINI_BIN`, then `gemini` |
| Grok | `AI_E_GROK_BIN`, then `GROK_BIN`, then `grok` |
| Copilot | `AI_E_COPILOT_BIN`, then `COPILOT_BIN`, then `copilot` |
| Kiro | `AI_E_KIRO_BIN`, then `KIRO_BIN`, then `kiro-cli` |
| Antigravity | `AI_E_AGY_BIN`, then `AGY_BIN`, then `agy` |

`--provider-bin <path>` overrides the binary for a single run.

For Codex validation, use `--model gpt-5-mini`:

```bash
ai-e codex --model gpt-5-mini --output-format json "reply with OK"
```

Other providers can use any locally available model alias.

## GPT Pro Browser Validation

`ai-e` itself does not automate browser-hosted ChatGPT. When GPT Pro validation
is needed, use the locally installed `agbrowse` help surface as the source of
truth:

```bash
agbrowse --help
agbrowse web-ai --help
```

For non-mutating validation, render the prompt envelope instead of sending to a
live provider:

```bash
agbrowse web-ai render \
  --vendor chatgpt \
  --model pro \
  --effort extended \
  --prompt "Validate this ai-e provider plan" \
  --json
```

Do not use `agbrowse web-ai send`, `query`, or browser mutation commands for
help-surface validation because those are live provider workflows that can call
GPT Pro.

## Output Contract

Print-compatible mode suppresses runtime diagnostics from stdout and returns the
requested user-facing format:

```bash
ai-e claude --output-format text "hello"
ai-e claude --output-format json "hello"
ai-e claude --output-format stream-json "hello" --verbose
```

Explicit runtime mode emits JSONL. Runtime lifecycle records use a generic
provider event envelope:

```json
{"type":"jaw_runtime","event":"provider_spawned","provider":"claude","pid":12345}
```

`jaw_runtime` stays for cli-jaw compatibility while this is still the first
external runtime migration target.

## Structured Output Position

Claude Code `-p --json-schema` has native print-mode behavior that creates a
separate `structured_output` field. The current Claude provider is interactive
PTY-backed, so it does not get Claude Code's internal print-mode
`StructuredOutput` tool automatically.

The intended `ai-e` implementation path is wrapper-side result shaping:

1. Parse final assistant text when `--json-schema` is provided.
2. Validate against JSON Schema.
3. Attach `structured_output` to the synthesized result.
4. Later add one repair turn if validation fails.

That keeps the user-facing result shape useful without claiming byte-for-byte
equivalence with Claude Code internals.

## Development

```bash
npm run fmt:check
npm run test
npm run build
npm run verify
npm run pack:dry
npm run publish:dry-run
```

### Prebuilt Package Contract

The main `@bitkyc08/ai-e` package declares one optional platform package per
supported OS/architecture. The main package version, every
`optionalDependencies` version, and every `platform-packages/*/package.json`
version must match before a release is considered publishable.
`npm run test:postinstall` checks this contract, and
`scripts/sync-package-versions.mjs <version>` updates all package metadata for a
workflow release version.

The GitHub release workflow also syncs Cargo metadata to the requested workflow
version before building platform packages and before publishing the main package.
This prevents the main package from pointing at platform package versions that
were never built.

Manual Claude smoke when auth is available:

```bash
bash scripts/smoke.sh
```

## Repository Map

- `src/lib.rs` - CLI entry, routing (interactive vs headless), Claude provider dispatch.
- `src/interactive.rs` - Interactive bypass engine: PTY spawn, paste inject, session file tail, normalize, completion detect.
- `src/interactive_providers.rs` - Per-provider TUI args, session path resolution, session ID extraction.
- `src/headless.rs` - Headless one-shot execution for all providers.
- `src/providers/` - Provider registry, adapter metadata, kiro session capture.
- `src/child.rs` - PTY child process wrapper.
- `src/hook.rs` - Claude hook relay used by the Claude provider.
- `src/transcript.rs` / `src/normalize.rs` - Claude transcript replay and stream-json normalization.
- `structure/` - Command surface, runtime contract, adapter architecture.
- `devlog/_plan/` - Active migration and provider expansion plans.

## cli-jaw Goal

The target integration is:

```text
cli-jaw
  -> ai-e claude ...   (PTY interactive, hook-based)
  -> ai-e codex ...    (interactive, session file tail + resume)
  -> ai-e grok ...     (interactive, session file tail + resume)
  -> ai-e kiro ...     (pipe, session footer + resume)
  -> ai-e agy ...      (headless -p, session footer + resume)
  -> ai-e gemini ...   (interactive)
  -> ai-e copilot ...  (interactive)
```

cli-jaw resolves one external runtime (`ai-e`) and selects providers through
explicit command arguments. Resume is supported for claude, codex, grok, kiro,
and agy via session ID persistence.
