---
created: 2026-05-20
status: evidence
tags: [ai-e, pty, smoke, providers, antigravity]
---
# Provider PTY Smoke Matrix

## Scope

This is implementation evidence only. No source code was changed for these
smokes. The goal was to verify whether ai-e can prepare a Claude-e-level PTY
pipeline for providers without relying on an Antigravity `--print` runtime
fallback.

Smoke artifact directory:

```text
/tmp/ai-e-pty-smoke-260520
```

PTY wrapper:

```text
/usr/bin/script -q <log> /bin/sh -lc '<provider command>'
```

Self-test:

```text
/tmp/ai-e-pty-smoke-260520/script-selftest.log
  SCRIPT_PTY_OK TTY
```

## Contract Conclusions

1. PTY is the execution contract. `transcribe` is the single capture and event
   contract. Each provider contributes capture sources into that pipeline:
   - Claude: native stream-json plus Claude transcript/hook path.
   - Gemini: native stream-json emitted by the PTY child.
   - Grok: native streaming-json for thought/text/end, but tool visibility is
     not full structured parity in the observed smoke.
   - Copilot: native JSONL emitted by the PTY child, but `transcribe` must
     redact bulky opaque fields before any user-facing projection.
   - Codex: PTY child JSONL when `codex exec --json` is selected, plus
     terminal transcript/screen as debug and fallback evidence.
   - Antigravity: excluded from ai-e provider target; PTY/TUI evidence is kept
     as negative/manual evidence only.
2. Existing non-PTY `-p`/`--prompt` parsing remains compatibility evidence. It
   is not the target replacement architecture.
3. AGY `--print` remains probe evidence only. It is not a runtime fallback.
4. Do not implement an AGY ai-e provider in this migration. The current evidence
   shows a full TUI boundary, not a reliable one-shot provider boundary.
5. ai-e should expose transcribed events and only a thin cli-jaw projection.
   Provider-native evidence should remain available as source artifacts with
   provenance, not be forced into a provider-wide semantic schema.

## Repeat Smoke Addendum

Repeat artifact directory:

```text
/tmp/ai-e-pty-smoke-260520-repeat
```

Repeat self-test:

```text
script-selftest.log
  REPEAT_SCRIPT_PTY_OK TTY
```

| Provider | Log | Exit | Repeat evidence |
|---|---:|---:|---|
| Claude | `claude-repeat.log` | 0 | emitted `CLAUDE_REPEAT_OK_260520` |
| Gemini | `gemini-repeat.log` | 0 | emitted `GEMINI_REPEAT_OK_260520` |
| Grok | `grok-repeat.log` | 0 | emitted `GROK_REPEAT_OK_260520` |
| Copilot | `copilot-repeat.log` | 0 | emitted `COPILOT_REPEAT_OK_260520`; opaque fields still require redaction |
| Codex | `codex-repeat.log` | 0 | emitted `CODEX_REPEAT_OK_260520` under PTY |
| AGY | `agy-repeat.log` | 124 | TUI/stdout did not naturally complete before timeout |
| AGY | `~/.gemini/antigravity-cli/brain/7d0e5f2c-2c46-4615-a590-87391154a453/.system_generated/logs/transcript.jsonl` | n/a | contained `USER_INPUT` and `PLANNER_RESPONSE` with `AGY_REPEAT_OK_260520` |

The repeat AGY result confirms the earlier negative finding: AGY transcript
evidence can appear quickly while the launched TUI process remains open. That is
not sufficient for the current ai-e provider contract, so AGY is excluded rather
than adapted.

## Help Capability Probe

| Provider | Log | Exit | Relevant result |
|---|---:|---:|---|
| Claude | `claude-help.log` | 0 | interactive default, `-p/--print`, `--output-format text/json/stream-json`, `--include-partial-messages`, `--agents` |
| Codex | `codex-help.log` | 0 | interactive default, `exec`, `--no-alt-screen`, `--cd`, `--add-dir`, approvals/sandbox flags |
| Gemini | `gemini-help.log` | 0 | interactive default, `-p/--prompt`, `-i/--prompt-interactive`, `--output-format text/json/stream-json`, approval mode flags |
| Grok | `grok-help.log` | 0 | TUI, `--single`, `--output-format plain/json/streaming-json`, `--no-alt-screen`, agents/subagents, `--cwd` |
| Copilot | `copilot-help.log` | 0 | interactive default, `-p/--prompt`, `-i/--interactive`, `--output-format text/json`, `--stream on/off` |
| AGY | `agy-help.log` | 0 | `--print`, `--prompt-interactive`, `--continue`, `--conversation`, `--dangerously-skip-permissions`, `--add-dir`, `--sandbox`, `--print-timeout`; no `--output-format` observed |

## Prompt Smoke Results

| Provider | Log | Exit | Result | Normalization note |
|---|---:|---:|---|---|
| Claude | `claude-prompt-stream.log` | 1 | `--output-format stream-json` failed without `--verbose` | wrapper must add `--verbose` for Claude print stream-json |
| Claude | `claude-prompt-stream-verbose.log` | 0 | emitted `CLAUDE_PTY_OK_260520` | JSONL includes `stream_event`, `assistant`, `result` |
| Codex | `codex-prompt.log` | 0 | emitted `CODEX_PTY_OK_260520` | terminal transcript captured; Codex JSONL should be added as a `transcribe` source |
| Gemini | `gemini-prompt-stream.log` | 0 | emitted `GEMINI_PTY_OK_260520` | JSONL includes `init`, `message`, `result` |
| Grok | `grok-prompt-stream.log` | 0 | emitted `GROK_PTY_OK_260520` | JSONL token stream with `thought`, `text`, `end` |
| Copilot | `copilot-prompt-json.log` | 0 | emitted `COPILOT_PTY_OK_260520` | JSONL includes session/tool metadata plus bulky opaque fields |
| AGY | `agy-prompt-interactive.log` | 124 | no stdout final before timeout | stdout/TUI alone is not enough |
| AGY | `agy-prompt-interactive-cli.log` | 124 | conversation id discovered | log pointed to brain conversation `2ec328fe-6c6d-4dc3-89fe-d6d104181738` |
| AGY | `~/.gemini/antigravity-cli/brain/2ec328fe-6c6d-4dc3-89fe-d6d104181738/.system_generated/logs/transcript.jsonl` | n/a | contained `PLANNER_RESPONSE` with `AGY_PTY_OK_260520` | transcript tailing is the observed data path |

AGY caveat:

```text
agy-prompt-interactive-cli.log reported "You are not logged into Antigravity"
for several cache/model polls, while the brain transcript still recorded a
model response. Implementation must surface login/model warnings separately
from transcript success.
```

## Tool-Use Smoke Results

Prompt shape:

```text
Run pwd using your shell/terminal/Bash tool, then reply exactly <PROVIDER>_TOOL_OK_260520
```

| Provider | Log | Exit | Tool evidence | Final evidence |
|---|---:|---:|---|---|
| Claude | `claude-tool-stream.log` | 0 | `content_block_start` `tool_use` name `Bash`; later `tool_result` with cwd | `CLAUDE_TOOL_OK_260520` |
| Codex | `codex-tool.log` | 0 | terminal transcript contained `exec /bin/zsh -lc pwd ... succeeded` | `CODEX_TOOL_OK_260520` |
| Gemini | `gemini-tool-stream.log` | 0 | `tool_use` `run_shell_command`, `tool_result` status `success` | `GEMINI_TOOL_OK_260520` |
| Grok | `grok-tool-stream.log` | 0 | no separate structured tool event observed; `thought` text described terminal execution | `GROK_TOOL_OK_260520` |
| Copilot | `copilot-tool-json.log` | 0 | `assistant.message.toolRequests`, `tool.execution_start`, `tool.execution_complete` | `COPILOT_TOOL_OK_260520` |

AGY tool-use smoke is not required for this ai-e provider migration because AGY
is excluded. Existing AGY transcript evidence remains useful for manual notes.

## Provider-Specific Parser Implications

### Claude

Feed native JSONL and existing transcript logic into `transcribe`. The PTY
refactor should preserve current Claude output byte-for-byte where possible.
The smoke confirms that Claude print stream-json requires `--verbose`.

### Gemini

Gemini already gives a clean stream-json event stream when run under PTY.
`transcribe` can read `tool_use`, `tool_result`, `message`, and `result`
directly from that PTY child source. Gemini does not need to be forced through
a screen parser.

### Grok

Grok gives useful thought/text/end streaming, but the observed tool-use smoke
did not expose a structured tool boundary. ai-e should not label Grok terminal
work as native tool_use/tool_result unless a later smoke proves a stable event.

### Copilot

Copilot JSONL is usable but noisy. `transcribe` must drop or redact:

```text
reasoningOpaque
encryptedContent
large ephemeral session fields unless explicitly requested by debug mode
```

Keep:

```text
assistant.message.content
assistant.message.toolRequests
tool.execution_start
tool.execution_complete
assistant.turn_start
assistant.turn_end
result.usage
```

### Codex

Codex should be treated like the other providers: the process still runs inside
the PTY supervisor, and `transcribe` owns every source. When `codex exec
--json` is selected, Codex JSONL is the preferred source; terminal
screen/transcript text remains debug/fallback evidence for readiness,
diagnostics, and unexpected TUI output. Do not create a separate public Codex
JSONL path outside `transcribe`.

### Antigravity

AGY is excluded from this ai-e provider plan. The rejected path was:

```text
spawn agy in PTY
pass --prompt-interactive or inject prompt interactively
write --log-file to a known path
parse the log line "Created conversation <uuid>"
tail ~/.gemini/antigravity-cli/brain/<uuid>/.system_generated/logs/transcript.jsonl
transcribe USER_INPUT / PLANNER_RESPONSE / tool records if present
emit only thin transport projection fields for cli-jaw
terminate on completion, timeout, or explicit error
```

Do not implement AGY transcribe/projection events in this migration. The rejected
candidate event set was:

```text
runtime_started
provider_spawned
provider_warning
assistant_text_delta or assistant_final
tool_observed only if transcript proves tool vocabulary
subagent_observed only if transcript proves subagent vocabulary
result
```

Forbidden for AGY in this migration:

```text
native tool_use/tool_result
thinking_delta
token usage
model id as stable contract
resume replay
stdout-only completion
agy --print fallback
```

## AGY Follow-Up If Reconsidered Later

Only reopen AGY if a supported non-TUI automation boundary appears. If that
happens, start a new plan and re-run:

1. Login reconciliation:
   - run `agy` status/help/account-related probes if available;
   - document whether CLI and Antigravity app share login state;
   - fail with `provider_warning` when cache/model polls say not logged in.
2. AGY tool prompt:
   - ask AGY to run `pwd`;
   - capture PTY log, CLI `--log-file`, and brain transcript;
   - identify exact transcript event types for tool start/end.
3. AGY subagent prompt:
   - ask for a bounded delegate/review action;
   - identify whether transcript contains subagent identity, lifecycle, and
     final text.
4. AGY completion probe:
   - determine whether `PLANNER_RESPONSE` status `DONE` is enough;
   - check if multi-step responses append later records after first
     `PLANNER_RESPONSE`.
5. AGY cleanup probe:
   - verify timeout kills child process and no orphaned `agy` child remains;
   - preserve pre-existing user `agy` processes.

## Raw Log Inventory

```text
agy-help.log                         1351 bytes
agy-prompt-interactive-cli.log      10213 bytes
agy-prompt-interactive.log            157 bytes
claude-help.log                     10018 bytes
claude-prompt-stream-verbose.log     7078 bytes
claude-prompt-stream.log              168 bytes
claude-tool-stream.log              11915 bytes
codex-help.log                       5929 bytes
codex-prompt.log                      378 bytes
codex-tool.log                        568 bytes
copilot-help.log                    10203 bytes
copilot-prompt-json.log             13522 bytes
copilot-tool-json.log               27380 bytes
gemini-help.log                      3961 bytes
gemini-prompt-stream.log             1310 bytes
gemini-tool-stream.log               1431 bytes
grok-help.log                        3818 bytes
grok-prompt-stream.log               1008 bytes
grok-tool-stream.log                 1860 bytes
script-selftest.log                    21 bytes
```
