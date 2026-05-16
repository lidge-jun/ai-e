# ai-e

`ai-e` is a standalone npm-distributed Rust runtime wrapper for interactive AI CLIs.

## Source Rules

- Keep generated build output out of git. `target/` must stay ignored.
- Prefer `cargo fmt --check`, `cargo test --locked`, and `cargo build --release --locked` before publishing runtime changes.
- Keep the public npm package and command name `ai-e`.
- Do not expose provider-specific bins such as `claude-e` from this package; those remain separate packages or future thin aliases.
- Keep provider-specific behavior behind `src/providers/` boundaries before adding Codex, Gemini, Grok, or other adapters.
- Preserve stdout JSONL compatibility for cli-jaw unless `structure/runtime_contract.md` and `devlog/` describe the migration.

## Documentation

- `structure/` is the architecture and runtime surface reference.
- `devlog/_plan/` holds active planning work. New durable plan docs use numbered prefixes such as `00_overview.md`.
- Update `README.md`, `structure/INDEX.md`, and the relevant devlog plan when command flags, protocol events, exit codes, packaging, or cli-jaw integration behavior changes.
