---
created: 2026-05-20
status: planning
tags: [ai-e, pty, claude-style, provider-runtime]
---
# Claude-Style PTY Contract For All Providers

## Decision

All ai-e providers should move toward the Claude-style runtime model:

```text
ai-e provider command
  -> portable_pty child
  -> prompt injection or provider-native prompt submission inside the PTY
  -> provider-specific source capture
  -> one ai-e transcribe pipeline
  -> transcribed event stream
  -> optional thin cli-jaw projection
```

The existing `-p`/`--prompt`/headless JSONL paths are useful evidence and
compatibility shims, but they are not the target replacement architecture.
They already exist. The migration goal is Claude-e-level PTY control for every
provider.

## What "Claude-Style" Means In Current ai-e

The current Claude path in `src/lib.rs`, `src/child.rs`, `src/hook.rs`,
`src/transcript.rs`, and `src/normalize.rs` has these hard requirements:

1. Open a real PTY with `portable_pty::native_pty_system()`.
2. Spawn the provider binary in that PTY with a fixed cwd, terminal size, and
   `TERM=xterm-256color`.
3. Drain the PTY continuously.
4. Maintain a vt100 screen snapshot for readiness, diagnostics, and fallback
   prompt detection.
5. Answer terminal capability queries when the TUI asks them.
6. Wait for a provider-specific "session started" signal.
7. Discover a source path or stream from that signal.
8. Take an initial transcript offset.
9. Wait for PTY quiescence before prompt injection.
10. Inject the prompt through bracketed paste and submit it.
11. Verify prompt acceptance through `transcribe`.
12. Tail source readers live through `transcribe`.
13. Normalize provider-native records into ai-e stream-json.
14. Track activity and active tools for idle timeout decisions.
15. Wait for provider-specific completion signals.
16. Emit a final synthesized `result`.
17. Gracefully exit or kill the provider process group.
18. Preserve a session/conversation id for interrupt-and-resume steering.

This is the contract to generalize. A provider is not considered migrated just
because `provider -p --output-format stream-json` can be parsed.

## Single Transcribe Boundary

The runtime should not grow separate "jsonl mode", "screen mode", and
"transcript mode" execution contracts. Codex already demonstrates why: when a
provider has structured JSONL, that is a capture source from the PTY child, not
a second runner.

Therefore the provider-neutral boundary is:

```text
PTY child bytes
PTY screen snapshots
provider log files
provider transcript JSONL
provider stdout/stderr JSONL
provider exit/status
  -> transcribe
  -> TranscribedEvent stream
  -> synthesized Result
```

`transcribe` owns source precedence, redaction, offset/watermark handling,
partial-line buffering, final text extraction, and debug artifact references.
Provider adapters may contribute source readers, but they must not expose a
parallel public contract. This keeps all providers on the Claude-e shape:
supervise in PTY first, transcribe everything second, and emit only a thin
projection when cli-jaw needs common transport fields.

## Current Claude Implementation Shape

Concrete current locations:

```text
src/lib.rs
  main_entry()
    splits provider
    routes only ProviderKind::ClaudeCode to PTY

  run()
    creates HookDir
    builds Claude args with --settings hook JSON
    spawns child::PtyChild
    waits for SessionStart sentinel
    extracts transcript path/session id
    bracketed-pastes prompt
    verifies transcript activity after initial offset
    starts transcript tail thread
    waits for Stop/StopFailure/child exit/timeouts
    synthesizes final result

src/child.rs
  PtyChild::spawn()
    opens portable_pty
    spawns command
    drains master reader
    keeps vt100 screen snapshot
    answers terminal queries

src/hook.rs
  HookDir
    builds temporary relay script
    writes SessionStart/Stop/StopFailure payloads atomically
    exposes sentinel and payload paths

src/transcript.rs
  tail_transcript()
    tails JSONL after offset
    retries partial lines
    emits mapped stream lines
    tracks tool_use/tool_result activity

src/normalize.rs
  normalize_transcript_line()
    filters internal records
    maps assistant/user records for the current Claude contract
    passes known stream events
    synthesizes final result
```

## Provider Adapter Target

Generalize the current Claude path into a provider adapter interface. The
shared spine should remain one PTY engine; the provider-specific pieces should
be isolated.

```text
ProviderPtyAdapter
  id()
  binary()
  build_args(config)
  install_sources(config)
  wait_session_started(child, sources)
  source_start_offsets(sources)
  prepare_prompt(prompt)
  inject_prompt(child, prepared_prompt)
  verify_prompt_accepted(transcribe, start_offsets)
  run_transcribe(sources, output_format)
  completion_state(child, transcribe)
  transcribe_source_record(record)
  shutdown(child)
```

Provider source types:

```text
Claude
  source: hook-discovered transcript JSONL
  start: SessionStart hook
  complete: Stop/StopFailure hook

Gemini
  source: PTY stdout stream-json first, optional workspace/session files if discovered
  start: stdout init event or TUI ready detector
  complete: stdout result event

Grok
  source: PTY stdout streaming-json plus post-run trace backfill if needed
  start: first thought/text event or TUI ready detector
  complete: stdout end event

Copilot
  source: PTY stdout JSONL
  start: session/tool metadata event or first assistant.turn_start
  complete: result event

Codex
  source: PTY stdout JSONL when `codex exec --json` is selected, plus PTY screen/transcript text as debug/fallback
  start: JSONL session/event start or screen readiness
  complete: JSONL result marker or final answer plus process exit/result marker
```

## Role Of `-p` And JSONL

`-p`, `--prompt`, `--single`, and provider-native JSONL are not the target
runtime boundary. They may be used inside a PTY adapter only when they improve
source capture for `transcribe`.

Allowed:

```text
spawn provider in PTY
pass provider-native prompt flag inside the PTY command
transcribe provider-native JSONL emitted from that PTY process
emit transcribed events plus optional thin projection
```

Not allowed as final migration target:

```text
spawn provider without PTY
parse provider -p JSONL as the replacement runtime
let cli-jaw call provider CLIs directly when ai-e is selected
```

This distinction matters because `-p` parsing is already the current
replacement/compatibility path. The new work is PTY supervision, session
contracting, prompt acceptance verification, completion detection, and
transcribed replay.

## AGY Exclusion Under This Model

AGY / Antigravity is excluded from the ai-e provider target for this migration.
The local `agy hi` evidence shows a full terminal app with account/model status
and an input prompt. Earlier PTY smokes also showed `agy --prompt-interactive`
remaining open until timeout even when the brain transcript had a response.

Therefore do not implement:

```text
AntigravityPtyAdapter
ProviderKind::Antigravity
ai-e antigravity
agy --print fallback
```

The AGY logs/transcripts remain negative/manual evidence only.

## Refactor Order

1. Rename `claude_bin`-specific configuration into provider-neutral binary
   config while preserving current behavior.
2. Extract `PtyChild` as the common process/screen engine.
3. Extract current Claude run flow into `ClaudePtyAdapter` with no behavior
   changes.
4. Add adapter-level smoke fixtures for prompt acceptance, tool activity,
   idle timeout with active tools, stop, failure, and interrupt.
5. Add `GeminiPtyAdapter`, `GrokPtyAdapter`, `CopilotPtyAdapter`, and
   `CodexPtyAdapter` one by one, each still running under PTY and feeding every
   provider-native JSONL/text/screen source into `transcribe`.
6. Remove or demote headless provider paths after parity fixtures exist.

## Acceptance Criteria

A provider reaches Claude-style readiness only when ai-e can prove:

```text
spawned_in_pty: true
prompt_injected_or_submitted_under_pty: true
prompt_acceptance_verified: true
transcribe_sources_documented: true
live_activity_updates_timeout: true
tool_activity_blocks_idle_timeout_when_supported: true
completion_detected_without_process_guessing: true
final_text_or_result_derived_from_evidence: true
process_group_cleanup_verified: true
cli-jaw_consumes_ai_e_only: true
```

For AGY, the ai-e contract is:

```text
provider_target: false
probe_only_docs: true
runtime_fallback: false
```

Steering semantics are documented separately in
`05_steer_resume_contract.md`. The short version: all providers must support
interrupt-and-resume before any provider is allowed to claim live input-box
steering.
