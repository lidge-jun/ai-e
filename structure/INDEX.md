# ai-e Structure

`ai-e` is the multi-provider successor scaffold to the standalone `claude-e`
runtime. Claude Code is PTY-backed with interactive prompt injection; Codex,
Gemini, Grok, and Copilot are PTY prompt-mode adapters over their native
one-shot CLI surfaces.

## Modules

| Area | Files | Responsibility |
|---|---|---|
| CLI entrypoint | `src/lib.rs`, `src/args.rs`, `src/bin/ai-e.rs`, `bin/ai-e` | `ai-e <provider> ...` parsing and npm binary wrapper |
| Provider registry | `src/providers/` | provider ids, binary resolution metadata, PTY routing |
| PTY prompt-mode providers | `src/headless.rs` | Codex/Gemini/Grok/Copilot option parsing, arg construction, timeout-bound PTY spawn |
| Claude PTY provider | `src/child.rs`, `src/hook.rs`, `src/transcript.rs`, `src/normalize.rs` | current runnable provider path copied from `claude-e` |
| Runtime config | `src/config.rs`, `src/protocol.rs` | run ids, session ids, runtime JSONL envelope |
| Terminal handling | `src/terminal.rs`, `src/cleanup.rs`, `src/sanitize.rs` | PTY terminal responses, prompt safety, process cleanup |
| Print compatibility | `src/print_mode.rs` | `-p`-style prompt/stdin/output parsing for the Claude provider |
| Packaging | `Cargo.toml`, `package.json`, `bin/`, `scripts/`, `.github/workflows/` | Rust build, npm install, dry-run/publish/release scripts |

## Documents

- `cli_surface.md` - supported commands, provider syntax, and npm packaging.
- `provider_adapter.md` - adapter contract for Codex, Gemini, Grok, Copilot, and future CLIs.
- `runtime_contract.md` - JSONL lifecycle and result shape.
- `cli_jaw_migration.md` - target cli-jaw integration path.

## Current Invariants

- Public npm command: `ai-e`.
- Provider-explicit shape: `ai-e claude ...`, `ai-e codex ...`, `ai-e gemini ...`, `ai-e grok ...`.
- PTY providers: `claude`, `codex`, `gemini`, `grok`, `copilot`.
- AGY/Antigravity is intentionally not a provider id.
- No provider-specific bins are exposed by this npm package yet.
- `jaw_runtime` envelope remains for cli-jaw compatibility, with generic `provider_spawned` events added for multi-provider use.
