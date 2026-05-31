# Runtime Contract

## Execution Modes

`ai-e` has three execution paths selected by provider and flags:

| Mode | Providers | Selection |
|---|---|---|
| Claude PTY + hook | claude | Default for claude provider |
| Interactive bypass | codex, gemini, grok, copilot, kiro, agy | Default for non-claude (except agy) |
| Headless (legacy) | codex, gemini, grok, copilot, kiro, agy | `--headless` or `-p` flag; default for agy |

## Command Input

Provider-explicit shape:

```text
ai-e <provider> [args] <prompt>
ai-e <provider> p [args] <prompt>
ai-e <provider> run [args] -- [provider args]
```

If the provider is omitted, the current bootstrap default is `claude`. cli-jaw
should pass the provider explicitly.

Prompt handling:

- Positional prompt text is accepted.
- Piped stdin is read non-blocking and appended to positional prompt text.
- `--input-format stream-json` extracts user text from JSONL messages.
- Empty prompt input is rejected with exit code `16`.
- Prompt input is sanitized before provider execution.
- Maximum prompt size: 10 MB.

## Claude PTY Runtime

The Claude provider keeps the `claude-e` behavior:

- Spawn Claude Code in a PTY.
- Handle terminal capability probes.
- Auto-accept workspace/folder trust when enabled.
- Add permission bypass unless the caller provided a permission policy.
- Inject the prompt via bracketed paste (up to 3 retries, 5s per attempt).
- Tail Claude transcript JSONL.
- Synthesize text/json/stream-json output.

Claude PTY timeout behavior is activity-aware:

- `--idle-timeout-ms` expires only after no transcript activity for the window.
- Assistant/user transcript records, tool use, and tool result records refresh
  the activity clock.
- Active tool calls suppress idle timeout until tool results arrive.
- `--hard-timeout-ms` remains the absolute process cap.
- `--timeout-ms` is a backward-compatible alias for `--idle-timeout-ms`.

## Interactive Bypass Runtime

Non-claude providers (default mode) use session-file tailing for completion:

- Spawn provider TUI in PTY (or pipe for kiro).
- Inject prompt via positional arg or bracketed paste.
- Tail provider session JSONL for new assistant content.
- Completion = session file stable for 3s after assistant response.
- Agy: no JSONL (protobuf) — uses PTY quiescence for completion.
- Graceful exit: Ctrl-C to child, wait 3s, kill process group.
- Extract session ID and emit session footer.

Interactive bypass timeout:

- `--idle-timeout-ms` (default 600s): no new session file content.
- `--hard-timeout-ms` (default 3600s): absolute cap.

## Headless Runtime (Legacy)

One-shot provider invocation with timeout. Provider-native output relayed
directly to stdout.

| Provider | Spawn Shape |
|---|---|
| Codex | `codex exec --json ... <prompt>` |
| Gemini | `gemini --prompt <prompt> --output-format ...` |
| Grok | `grok --single <prompt> --output-format ...` |
| Copilot | `copilot --prompt <prompt> --output-format ...` |
| Kiro | `kiro-cli chat --no-interactive ... <prompt>` |
| Agy | `agy -p <prompt> --dangerously-skip-permissions` |

Headless hardening defaults:

- Codex: `--dangerously-bypass-approvals-and-sandbox --skip-git-repo-check`
- Gemini: `--skip-trust --approval-mode yolo` + home-root `--include-directories`
- Grok: `--no-alt-screen --always-approve --permission-mode bypassPermissions`
- Copilot: `--allow-all --stream off`
- Kiro: `--trust-all-tools`
- Agy: `--dangerously-skip-permissions --print-timeout 10m`

## Output Formats

| Format | Behavior |
|---|---|
| `text` | Assistant text content only to stdout |
| `json` | Final result JSON object at end |
| `stream-json` | JSONL events as they arrive + final result |

Claude provider normalizes transcript replay into these formats. Interactive
bypass providers project their native session JSONL into the same shapes.
Headless providers relay native output directly.

## Runtime Events (JSONL)

Claude `run`/`exec` mode emits structured JSONL events on stdout:

```json
{"type":"jaw_runtime","event":"runtime_started","run_id":"run_12345678"}
{"type":"jaw_runtime","event":"provider_spawned","provider":"claude","pid":12345}
{"type":"jaw_runtime","event":"session_started","session_id":"..."}
{"type":"jaw_runtime","event":"prompt_injected"}
{"type":"jaw_runtime","event":"stop_received"}
```

Interactive bypass and headless providers do not emit `jaw_runtime` events.
They relay provider-native output directly.

## Session Footer

All providers emit a stderr footer by default:

```text
[ai-e] session: <session-id>
[ai-e] resume: ai-e <provider> --resume <session-id> "your next prompt"
```

Interactive bypass uses `--interactive` in the resume hint:

```text
[ai-e] resume: ai-e <provider> --interactive --resume <session-id> "your next prompt"
```

Suppress with `--no-session-footer`.

## Structured Output

Claude Code `-p --json-schema` creates a `structured_output` field via its
internal print-mode behavior. The Claude PTY provider does not receive that
tool automatically. Current behavior: `--json-schema` appends a schema
instruction to the prompt. Future: parse/validate final output and attach
`structured_output` at the wrapper layer.

Codex has native `--output-schema <FILE>` in `codex exec`; `ai-e` does not yet
map `--json-schema` into a temp schema file.

## Exit Codes

| Code | Meaning |
|---:|---|
| `0` | Normal completion |
| `1` | Underlying provider exited unsuccessfully |
| `2` | Graceful interrupt (SIGINT/SIGTERM); session may be resumable |
| `4` | Provider spawn or PTY write failure |
| `5` | Claude SessionStart hook failure/timeout or early exit before SessionStart |
| `6` | Wrapper timeout (idle or hard) |
| `7` | Claude prompt injection transcript verification failure |
| `11` | Claude StopFailure hook |
| `13` | Claude hook temp dir or settings generation failure |
| `16` | stdin read, size, empty prompt, argument parse, or prompt sanitization failure |
| `64` | Unsupported provider in current runtime path |

## Compatibility Guarantees

- Public binary: `ai-e`.
- Public provider shape: `ai-e <provider> ...`.
- 7 providers: claude, codex, gemini, grok, copilot, kiro, agy.
- `claude-e` remains a separate standalone repository and package.
- `jaw_runtime` envelope remains for cli-jaw compatibility.
- Provider-specific bins are not exposed from this package.
