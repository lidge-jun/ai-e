# Runtime Contract

## Provider Classes

`ai-e` supports two runtime classes:

- PTY providers: interactive CLIs that need terminal automation. Current:
  `claude`.
- Headless providers: CLIs with native non-interactive execution. Current:
  `codex`, `gemini`, `grok`, `copilot`.

## Command Input

Provider-explicit shape:

```text
ai-e <provider> [args] <prompt>
ai-e <provider> run [args] -- [provider args]
```

If the provider is omitted, the current bootstrap default is `claude`. cli-jaw
should not rely on that shorthand; it should pass the provider explicitly.

Prompt handling:

- Positional prompt text is accepted.
- Piped stdin is accepted and appended to positional prompt text.
- Empty prompt input is rejected with exit code `16`.
- Prompt input is sanitized before provider execution.

## Claude PTY Runtime

The Claude provider keeps the copied `claude-e` behavior:

- spawn Claude Code in a PTY;
- handle terminal capability probes;
- auto-accept workspace/folder trust when enabled;
- add permission bypass unless the caller provided a permission policy;
- inject the prompt via bracketed paste;
- tail Claude transcript JSONL;
- synthesize text/json/stream-json output.

Claude binary resolution:

1. `AI_E_CLAUDE_BIN`
2. `CLAUDE_BIN`
3. `claude`

## Headless Runtime

Headless providers spawn the provider CLI directly with inherited stdout/stderr
and a wrapper timeout. The provider owns its native output format.

| Provider | Spawn shape |
|---|---|
| Codex | `codex exec ... <prompt>` |
| Gemini | `gemini --prompt <prompt> ...` |
| Grok | `grok --single <prompt> ...` |
| Copilot | `copilot --prompt <prompt> ...` |

Headless timeout returns exit code `6`. Spawn failure returns exit code `4`.
Provider exit codes are otherwise propagated.

## Runtime Events

Claude runtime mode emits JSONL runtime events on stdout when `run` is used.
The event envelope remains `jaw_runtime` for cli-jaw compatibility:

```json
{"type":"jaw_runtime","event":"runtime_started","run_id":"run_12345678"}
```

Multi-provider additions use generic event names:

```json
{"type":"jaw_runtime","event":"provider_spawned","provider":"claude","pid":12345}
```

Headless providers currently relay provider stdout/stderr directly and do not
wrap provider-native JSON into `jaw_runtime` events. That is intentional until
each provider's JSON event stability is audited.

## Session Footer

Claude print-compatible mode emits a stderr footer by default:

```text
[ai-e] session: <session-id>
[ai-e] resume: ai-e claude --resume <session-id> "your next prompt"
```

Headless provider resume behavior remains provider-native.

## Structured Output

Claude Code `-p --json-schema` creates a separate `structured_output` field via
its internal print-mode behavior. The Claude PTY provider does not receive that
internal tool automatically. Current behavior is compatibility-oriented prompt
instruction; future hardening should parse/validate final output and attach
`structured_output` at the wrapper layer.

Codex has native `--output-schema <FILE>` in `codex exec`; `ai-e` does not yet
map `--json-schema` into a temp schema file. That is a planned hardening item.

## Exit Codes

| Code | Meaning |
|---:|---|
| `0` | Normal completion |
| `1` | Underlying provider exited unsuccessfully without a more specific wrapper classification |
| `2` | Graceful interrupt; session metadata can be resumable |
| `4` | Provider spawn or PTY write failure |
| `5` | Claude SessionStart hook failure, timeout, or early exit before SessionStart |
| `6` | Wrapper timeout |
| `7` | Claude prompt injection transcript verification failure |
| `11` | Claude StopFailure hook |
| `13` | Claude hook temp dir or settings generation failure |
| `16` | stdin read, size, empty prompt, argument parse, or prompt sanitization failure |
| `64` | Reserved for unsupported provider ids or future provider dispatch failures |

## Compatibility Guarantees

- Public binary: `ai-e`.
- Public provider shape: `ai-e <provider> ...`.
- `claude-e` remains a separate standalone repository and package.
- `jaw_runtime` remains until cli-jaw supports a renamed runtime envelope.
- Provider-specific bins are intentionally not exposed from this package yet.
