# Provider Adapter Contract

`ai-e` supports 7 providers across 3 execution paths: Claude PTY with hooks,
interactive bypass (session file tailing), and legacy headless one-shot.

## Provider Map

| Provider | Kind | Default Mode | Entry Point | Key Defaults |
|---|---|---|---|---|
| Claude Code | PTY + hook | interactive (hook) | `claude` | `--dangerously-skip-permissions`, workspace trust auto-accept |
| Codex CLI | interactive bypass | interactive | `codex` | `--no-alt-screen --dangerously-bypass-approvals-and-sandbox` |
| Gemini CLI | interactive bypass | interactive | `gemini` | `--skip-trust --approval-mode yolo` + home-root `--include-directories` |
| Grok CLI | interactive bypass | interactive | `grok` | `--no-alt-screen --always-approve --permission-mode bypassPermissions` |
| Copilot CLI | interactive bypass | interactive | `copilot` | `--yolo` (interactive), `--allow-all --stream off` (headless) |
| Kiro CLI | pipe bypass | interactive (pipe) | `kiro-cli` | `--trust-all-tools`; pipe spawn (PTY hangs kiro-cli) |
| Antigravity | headless / interactive | headless | `agy` | `--dangerously-skip-permissions --print-timeout 10m` |

## Execution Paths

### Claude PTY + Hook (claude only)

Full PTY lifecycle with hook-based completion detection:

1. Spawn Claude Code in PTY with hook settings directory.
2. Auto-accept workspace/folder trust prompt.
3. Wait for SessionStart hook sentinel.
4. Inject prompt via bracketed paste (up to 3 retries).
5. Tail Claude transcript JSONL for output streaming.
6. Detect completion via Stop/StopFailure hook sentinels.
7. Emit session footer with resume command.

### Interactive Bypass (codex, gemini, grok, copilot, kiro, agy)

Session-file-tailing completion detection without hooks:

1. Spawn provider TUI in PTY (or pipe for kiro).
2. Build provider-specific args with resume/model/hardening flags.
3. Inject prompt (positional arg or bracketed paste depending on provider).
4. Tail provider session file (JSONL/sqlite) for new assistant content.
5. Detect completion via quiescence (3s stable after assistant response).
6. Graceful exit (Ctrl-C to child, wait, kill process group).
7. Extract session ID and emit session footer.

### Headless (legacy one-shot)

Direct provider invocation with timeout. Activated by `--headless` or `-p` flag
(or default for agy):

1. Parse prompt from args/stdin.
2. Build provider-native one-shot command.
3. Spawn in PTY with timeout.
4. Relay stdout directly to caller.
5. Emit session footer when session ID is extractable.

## Provider Binary Resolution

Each provider resolves its binary through environment variables with fallback:

| Provider | Env Vars | Default |
|---|---|---|
| Claude | `AI_E_CLAUDE_BIN`, `CLAUDE_BIN` | `claude` |
| Codex | `AI_E_CODEX_BIN`, `CODEX_BIN` | `codex` |
| Gemini | `AI_E_GEMINI_BIN`, `GEMINI_BIN` | `gemini` |
| Grok | `AI_E_GROK_BIN`, `GROK_BIN` | `grok` |
| Copilot | `AI_E_COPILOT_BIN`, `COPILOT_BIN` | `copilot` |
| Kiro | `AI_E_KIRO_BIN`, `KIRO_BIN`, `KIRO_CLI_BIN` | `kiro-cli` |
| Agy | `AI_E_AGY_BIN`, `AGY_BIN` | `agy` |

`--provider-bin <path>` overrides the binary for a single headless run.

## Session & Resume

All providers support session persistence and resume:

| Provider | Session Source | Resume Mapping |
|---|---|---|
| Claude | SessionStart hook payload / generated UUID | `--resume <id>` → PTY resume path |
| Codex | `thread.started.thread_id` from JSONL / rollout filename | `codex resume <id> <prompt>` |
| Gemini | First line of session JSONL (`sessionId`) | `gemini --resume <id>` |
| Grok | `sessionId` from JSON output / session dir UUID | `grok --resume <id>` |
| Copilot | `result.sessionId` from JSONL / session-state dir | `copilot --resume=<id>` |
| Kiro | sqlite query / stdout parse | `kiro-cli chat --resume-id <id>` |
| Agy | `last_conversations.json` (cwd → uuid) | `agy --conversation <id>` |

## Session File Locations (Interactive Bypass)

| Provider | Session Dir | File Pattern |
|---|---|---|
| Codex | `~/.codex/sessions/` | `rollout-*.jsonl` (newest by mtime) |
| Gemini | `~/.gemini/tmp/<project>/chats/` | `session-*.jsonl` (newest by mtime) |
| Grok | `~/.grok/sessions/<url-encoded-cwd>/` | `*/chat_history.jsonl` |
| Copilot | `~/.copilot/session-state/` | `*/events.jsonl` + `session-store.db` |
| Kiro | resolved via sqlite | N/A (pipe-based, not file-tailed) |
| Agy | `~/.gemini/antigravity-cli/conversations/` | protobuf (PTY quiescence, no JSONL) |

## Completion Detection

| Provider | Strategy |
|---|---|
| Claude | Hook sentinel files (Stop/StopFailure) |
| Codex | Session JSONL stable for 3s + has assistant response |
| Gemini | Session JSONL stable for 3s + has assistant response |
| Grok | Session JSONL stable for 3s + has assistant response |
| Copilot | Session JSONL stable for 3s + has assistant response |
| Kiro | Process exit (pipe mode) |
| Agy | PTY quiescence (no JSONL — protobuf format) |

## JSONL Normalization (Interactive Bypass)

Each provider's session JSONL is projected into a common shape for output:

- **Codex**: `response_item` → assistant messages, function calls, function call outputs
- **Gemini**: `type:"user"/"gemini"` → user/assistant turns with `thoughts` array
- **Grok**: `role:"user"/"assistant"` → turns with `thought` field; filters system-injected messages
- **Copilot**: `assistant.message`/`assistant.reasoning` → turns; strips `reasoningOpaque`, `encryptedContent`

## Future Hardening

- Move Claude-specific hook/transcript code into `src/providers/claude_code/` module.
- Map `--json-schema` to codex `--output-schema` temp file.
- Add provider-level smoke tests with fake provider binaries.
- Add gemini UUID-based resume (currently index-based only).
