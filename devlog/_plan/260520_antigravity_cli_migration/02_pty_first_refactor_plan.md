---
created: 2026-05-20
status: planning
tags: [ai-e, pty, provider-runtime, antigravity, refactor]
---
# PTY-First Provider Runtime Refactor

## Current Correction

AGY / Antigravity is excluded from this ai-e provider refactor. The PTY-first
plan still applies to Claude, Codex, Gemini, Grok, and Copilot. Their
provider-native `-p` / `--prompt` / `--single` / JSONL modes remain useful only
inside the ai-e PTY supervisor as prompt-submission and `transcribe` sources.

The AGY notes below are retained as negative/manual evidence: AGY behaves like a
full terminal app and did not prove a reliable one-shot ai-e provider boundary.

## Why This Exists

The current ai-e architecture is mixed:

```text
claude  -> PTY-backed runtime
codex   -> headless provider adapter
gemini  -> headless provider adapter
grok    -> headless provider adapter
copilot -> headless provider adapter
```

This was acceptable while Claude was the only provider that needed interactive
TUI handling. The new AGY evidence shows a different boundary: AGY should not
be forced into ai-e provider execution. The remaining design pressure is still
to move existing turn-oriented providers away from no-PTY headless execution.

Therefore ai-e should become PTY-backed for every migrated provider, not
Claude-only PTY. The current `-p`/`--prompt` JSONL paths are already the
replacement/compatibility layer; they are not the final migration target.
Every provider must run under the shared PTY spine and then declare which
sources feed `transcribe`: PTY stdout/stderr JSONL, transcript JSONL, PTY screen
text, logs, process status, or another provider-specific source.

## Current Code Evidence

```text
src/providers/mod.rs
  ProviderKind::is_pty_provider() returns true only for ClaudeCode.

src/lib.rs
  main_entry() routes non-PTY providers to headless::run_provider().
  run_provider() only executes the PTY runtime for ClaudeCode.

src/lib.rs
  The PTY runtime is currently Claude-specific:
  - builds Claude args,
  - injects Claude hook settings,
  - waits for SessionStart,
  - reads Claude transcript path,
  - waits for Stop/StopFailure sentinels.

src/transcript.rs
  Tool display comes from transcript JSONL content blocks:
  - tool_use increments active tool count,
  - tool_result decrements active tool count,
  - terminal tool events are emitted from transcript-derived events.
```

The important correction: ai-e does not get Claude tool visibility merely
because the child is in a PTY. It gets visibility because PTY execution is
combined with a provider-specific transcript contract.

## Target Architecture

Introduce a shared PTY runtime spine with provider-specific adapters.

```text
src/pty_runtime/mod.rs
  Shared:
  - open PTY
  - spawn provider binary
  - set cwd, TERM, cols, rows
  - capture vt100 screen snapshots
  - bracketed paste prompt injection
  - signal handling
  - idle/hard timeout
  - process cleanup

src/pty_runtime/provider.rs
  Trait/interface:
  - build_start_command()
  - wait_until_ready()
  - inject_prompt_strategy()
  - discover_transcript()
  - verify_prompt_accepted()
  - run_transcribe()
  - detect_completion()
  - project_transport_event()
  - graceful_shutdown()

src/pty_runtime/claude.rs
  Existing Claude behavior:
  - hook settings
  - SessionStart
  - transcript JSONL
  - Stop/StopFailure

AGY / Antigravity
  Excluded from ai-e provider adapter target for now.
  Keep smoke notes as negative/manual evidence only.
```

## Provider Modes

Each provider should declare its available execution modes:

```text
preferred_mode: pty | headless
secondary_mode: screen_observer | none
native_structured_output: true | false
transcript_source: claude_hooks | agy_brain | provider_jsonl | screen_only | none
tool_visibility: native | observed | screen_hint | none
resume_support: native | experimental | disabled
```

Initial matrix:

| Provider | Current | Target Decision |
|---|---|---|
| Claude | PTY + hook transcript | Keep; extract into adapter |
| Antigravity | Not implemented | Excluded from ai-e provider target; keep as manual/direct TUI evidence only |
| Gemini | Headless `gemini --prompt` | Move to PTY; feed PTY stdout stream-json into `transcribe` |
| Codex | Headless | Move to PTY; feed Codex JSONL and screen/text sources into `transcribe` |
| Grok | Headless | Move to PTY; parse PTY stdout streaming-json and backfill trace if needed |
| Copilot | Headless | Move to PTY; parse PTY stdout JSONL and redact bulky opaque fields |

Do not confuse provider-native JSONL with non-PTY/headless execution.
Provider-native JSONL can remain a `transcribe` source only after the provider
has been launched inside the shared PTY supervisor. AGY is not included in that
provider set.

Current smoke evidence is captured in `03_provider_pty_smoke_matrix.md`.
The most important result is that AGY stdout did not produce a final answer
before timeout, but AGY brain transcript JSONL did contain the final
`PLANNER_RESPONSE`. With the interactive TUI evidence, this is now negative
evidence for ai-e provider integration:

```text
do not add AGY provider adapter
do not add ai-e antigravity
do not fall back to agy --print
```

not:

```text
PTY stdout-only
agy --print fallback
```

The provider-wide Claude-style contract is captured in
`04_claude_style_pty_contract.md`.

## AGY PTY Smoke Result

The AGY PTY smoke is now closed as negative evidence for this integration.
Future AGY work needs a different boundary than ai-e's current one-shot provider
contract.

Observed probes:

```text
1. agy --prompt-interactive "<prompt>"
2. agy with prompt injected by bracketed paste
3. NO_COLOR=1 agy ...
4. --add-dir <workspace> as first AGY workspace arg
5. --dangerously-skip-permissions
6. terminal tool prompt: "Run pwd, then answer DONE."
7. subagent prompt: "Delegate a bounded review task, then answer DONE."
8. interrupt/timeout cleanup
```

Observed artifacts:

```text
/tmp/ai-e-agy-pty/raw.log
/tmp/ai-e-agy-pty/screen.txt
/tmp/ai-e-agy-pty/agy.log
/tmp/ai-e-agy-pty/brain-transcripts.txt
/tmp/ai-e-agy-pty/summary.json
```

Conclusions:

```text
can_find_transcript: true
transcript_updates_live: true for simple planner response
process_completion_contract: false
ai_e_provider_fit: false
next_action: exclude AGY from ai-e provider target
```

## Transcribe Output Contract

ai-e should own the output contract, but it does not need to force every
provider into a Claude-shaped semantic schema. Provider-native evidence should
be preserved as transcribed source artifacts, then exposed to cli-jaw through a
thin transport projection.

Do not implement AGY transcribe/projection events in ai-e in this migration.
The old candidate shape is retained only as a rejected design:

```json
{"type":"runtime_started","provider":"antigravity","mode":"pty"}
{"type":"provider_spawned","provider":"antigravity","pid":12345}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"..."}]}}
{"type":"tool_observed","provider":"antigravity","source":"agy_transcript","name":"LIST_DIRECTORY","confidence":"observed"}
{"type":"subagent_observed","provider":"antigravity","source":"agy_transcript","id":"...","confidence":"observed"}
{"type":"result","is_error":false,"provider":"antigravity"}
```

Forbidden until proven:

```text
thinking_delta
native tool_use/tool_result labels for AGY
token usage
model id
resume event replay
```

Reason: AGY has not yet provided those as a stable public CLI output contract.

## Implementation Order

1. Extract shared PTY child/spawn/screen utilities without changing Claude
   behavior.
2. Add provider adapter trait and implement Claude adapter by moving existing
   hook/transcript logic behind the trait.
3. Add regression tests proving `ai-e claude` output and tool events are
   unchanged.
4. Do not build AGY provider code.
5. Do not add `ProviderKind::Antigravity`.
6. Move Codex/Gemini/Grok/Copilot to PTY adapters one provider at a time.
   Their native JSONL parsers may be reused only as `transcribe` source readers
   for the PTY child, not as no-PTY replacement runtimes.

## cli-jaw Contract Impact

cli-jaw should stop assuming ai-e provider output quality is tied only to
provider name. It should consume explicit ai-e capability metadata:

```text
provider: claude | codex | gemini | grok | copilot
execution_mode: pty | headless
tool_visibility: native | observed | screen_hint | none
structured_output: transcribed | thin-projection | provider-native-debug | none
resume: disabled | experimental | stable
```

AGY should not be exposed as an ai-e-backed cli-jaw canary in this migration.
