# ai-e Structure

`ai-e` is the multi-provider PTY/pipe-backed exec layer for interactive AI CLIs.
It evolved from the standalone `claude-e` runtime into a unified command that
drives Claude Code, Codex, Gemini, Grok, Copilot, Kiro, and Antigravity through
the same process contract.

## Execution Paths

| Path | Providers | Description |
|---|---|---|
| Claude PTY + hook | claude | Interactive prompt injection, transcript tailing, hook-based lifecycle |
| Interactive bypass | codex, gemini, grok, copilot, kiro, agy | Spawn TUI/pipe, inject prompt, tail session file for completion |
| Headless (legacy) | codex, gemini, grok, copilot, kiro, agy | One-shot native CLI flags; deprecated for most providers |

Interactive bypass is the default for codex/gemini/grok/copilot/kiro. Agy
defaults to headless (TUI output is not parseable) with `--interactive` opt-in.

## Modules

| Area | Files | Responsibility |
|---|---|---|
| CLI entrypoint | `src/lib.rs`, `src/args.rs`, `src/bin/ai-e.rs`, `bin/ai-e` | `ai-e <provider> ...` parsing, mode routing, npm binary wrapper |
| Provider registry | `src/providers/mod.rs`, `src/providers/claude_code.rs` | Provider ids, binary resolution, PTY/pipe classification |
| Interactive bypass | `src/interactive.rs`, `src/interactive_providers.rs` | PTY/pipe TUI spawn, session file tailing, completion detection, resume |
| Headless providers | `src/headless.rs` | Legacy one-shot PTY spawn with timeout for all non-claude providers |
| Claude PTY provider | `src/child.rs`, `src/hook.rs`, `src/transcript.rs`, `src/normalize.rs` | Hook-based lifecycle, transcript replay, stream-json normalization |
| Kiro session | `src/providers/kiro_session.rs` | Kiro sqlite session resolution, conversation ID extraction |
| Runtime config | `src/config.rs`, `src/protocol.rs` | Run ids, session ids, runtime JSONL envelope |
| Terminal handling | `src/terminal.rs`, `src/cleanup.rs`, `src/sanitize.rs` | PTY terminal responses, prompt safety, process cleanup |
| Print compatibility | `src/print_mode.rs` | `-p`-style prompt/stdin/output parsing for the Claude provider |
| Packaging | `Cargo.toml`, `package.json`, `bin/`, `scripts/`, `.github/workflows/`, `platform-packages/`, `tests/` | Rust build, prebuilt platform packages, npm install, release scripts |

## Documents

- `cli_surface.md` — supported commands, provider syntax, interactive/headless modes, npm packaging.
- `provider_adapter.md` — adapter contract for all 7 providers.
- `runtime_contract.md` — JSONL lifecycle, output formats, exit codes, session management.
- `cli_jaw_migration.md` — target cli-jaw integration path.

## Current Invariants

- Public npm command: `ai-e`.
- Provider-explicit shape: `ai-e <provider> ...`.
- 7 providers: `claude`, `codex`, `gemini`, `grok`, `copilot`, `kiro`, `agy`.
- Default mode: interactive bypass (except agy → headless, claude → PTY+hook).
- Session footer emitted for all providers with resume support.
- `jaw_runtime` envelope remains for cli-jaw compatibility.
- Main package `optionalDependencies` and `platform-packages/*` versions must match before release.
