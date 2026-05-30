# 00 — Interactive Bypass for All 4 Providers (Plan)

## Goal

Add a **second execution path** (interactive PTY bypass) for codex, gemini, grok, and copilot — alongside the existing headless one-shot path. This mirrors the claude-e pattern: spawn the provider's TUI in a PTY, inject prompt via bracketed paste, detect turn completion via session file tailing, and support resume.

## Architecture

```
ai-e <provider> run --interactive [--resume <ID>] -- [extra args]
     │
     ├── existing headless path (unchanged)
     │   └── headless.rs → spawn_pty_with_timeout (one-shot, no TUI)
     │
     └── NEW interactive path
         └── interactive.rs → run_interactive()
             ├── spawn TUI in PTY (portable-pty)
             ├── wait for TUI ready (quiescence)
             ├── inject prompt via bracketed paste
             ├── tail session file for completion
             ├── graceful exit (/exit or Ctrl-C)
             └── emit session footer for resume
```

## Routing

In `lib.rs::main_entry()`, when provider is headless AND `--interactive` flag is present:
- Route to `interactive::run_interactive(provider, config)` instead of `headless::run_provider()`

## Per-Provider Contracts

| Provider | TUI Entry | Resume Entry | Session File | Completion Signal | Graceful Exit |
|----------|-----------|--------------|--------------|-------------------|---------------|
| codex | `codex [PROMPT]` | `codex resume <ID>` | `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` | New JSONL line with `"type":"assistant"` after last user turn, then quiescence | Ctrl-C (SIGINT) |
| gemini | `gemini [query]` | `gemini -r latest` / `--session-id <UUID>` | `~/.gemini/tmp/<project>/chats/session-*.jsonl` | New JSONL line after user turn, then quiescence | Ctrl-C |
| grok | `grok` (prompt via paste) | `grok -r <ID>` / `grok -c` | `~/.grok/sessions/<cwd>/<uuid>/chat_history.jsonl` | New JSONL line, then quiescence | Ctrl-C |
| copilot | `copilot` (prompt via paste) | `copilot --resume=<ID>` / `--continue` | `~/.copilot/session-store.db` (sqlite) + `events.jsonl` in session-state | Process returns to input prompt (quiescence-based) | Ctrl-C |

## File Changes

### NEW: `src/interactive.rs` (~300 lines)

Core interactive bypass engine, provider-agnostic:

```rust
pub struct InteractiveConfig {
    pub provider: ProviderKind,
    pub provider_bin: String,
    pub prompt: String,
    pub cwd: PathBuf,
    pub cols: u16,
    pub rows: u16,
    pub idle_timeout_ms: u64,
    pub hard_timeout_ms: u64,
    pub output_format: String,
    pub resume_session: Option<String>,
    pub extra_args: Vec<String>,
    pub show_session_footer: bool,
}

pub fn run_interactive(config: InteractiveConfig) -> i32 {
    // 1. Build provider-specific TUI args
    // 2. Spawn in PTY via child::PtyChild::spawn()
    // 3. Wait for TUI quiescence (reuse child.wait_quiescence)
    // 4. Inject prompt via bracketed paste (reuse sanitize::bracketed_paste)
    // 5. Tail session file for completion (provider-specific path resolution)
    // 6. Wait for completion (quiescence + session file growth stop)
    // 7. Graceful exit (Ctrl-C to PTY)
    // 8. Emit session footer
}
```

### NEW: `src/interactive_providers.rs` (~200 lines)

Per-provider specifics:

```rust
pub fn build_interactive_args(provider: ProviderKind, config: &InteractiveConfig) -> Vec<String>;
pub fn resolve_session_path(provider: ProviderKind, cwd: &Path) -> Option<PathBuf>;
pub fn detect_session_id(provider: ProviderKind, session_path: &Path) -> Option<String>;
pub fn is_turn_complete(provider: ProviderKind, session_path: &Path, offset: u64) -> bool;
```

### MODIFY: `src/lib.rs`

- Add `mod interactive; mod interactive_providers;`
- In `main_entry()`: detect `--interactive` flag → route to interactive path

### MODIFY: `src/args.rs`

- Add `--interactive` flag to the `Run` command (or detect it in print_mode args)

### MODIFY: `src/providers/mod.rs`

- Add `pub fn supports_interactive(self) -> bool` (all 4 return true, Kiro returns false)

### MODIFY: `src/headless.rs`

- Add `--interactive` to the flag parser so it's consumed (not passed to provider)

## Resume Flow

```
ai-e codex --interactive --resume <SESSION_ID> "continue from here"
  → codex resume <SESSION_ID>
  → wait TUI ready
  → bracketed paste "continue from here"
  → tail session file
  → completion → exit → footer
```

## Completion Detection Strategy

All 4 providers use a **dual signal**:
1. **Session file growth** — tail the JSONL/sqlite for new assistant content
2. **PTY quiescence** — no new PTY output for N ms after session file shows assistant turn

This avoids needing hooks (which only Claude supports).

## Testing

- Unit tests for arg building per provider
- Integration: `ai-e codex --interactive "echo hello"` should complete and emit session footer
- Resume: verify session ID extraction and footer emission

## Phases

- Phase 1: `interactive.rs` + `interactive_providers.rs` scaffolding + codex support
- Phase 2: gemini + grok support
- Phase 3: copilot support (sqlite session store is different)
- Phase 4: resume flow for all 4
