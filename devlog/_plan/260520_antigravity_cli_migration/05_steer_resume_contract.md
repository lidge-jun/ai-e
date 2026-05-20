---
created: 2026-05-20
status: planning
tags: [ai-e, pty, steer, resume, interrupt]
---
# Steer, Interrupt, And Resume Contract

## Decision

Claude-style PTY migration must include steering semantics, not only spawn and
parsing. The default mid-run steer contract should be:

```text
interrupt current PTY run
preserve or discover session/conversation id
exit cleanly when possible
start a new PTY run with provider-native resume/continue arguments
inject the new prompt
verify acceptance through the observer
```

Directly writing a new prompt into the provider input box and pressing Enter is
a separate capability. It can be supported only when the adapter proves the
provider is idle, the input field is focused, and the observer can confirm the
new prompt was accepted.

## Current Claude Behavior

Current ai-e Claude path:

```text
SIGINT/SIGTERM received
  -> stop flag
  -> emit interrupted { resumable: true }
  -> cleanup::graceful_exit()
  -> write "/exit\r" into PTY
  -> wait up to 5s
  -> escalate to process-group kill if needed
```

Current resume command construction:

```text
ai-e claude run ... --resume <sessionId> -- <claude args>
```

Current session source:

```text
SessionStart hook payload -> session_id/sessionId
SessionStart hook payload -> transcript_path/transcriptPath
```

Current prompt injection:

```text
sanitize prompt
strip ESC
reject Ctrl-C/Ctrl-D/Ctrl-Z/NUL
bracketed paste:
  ESC [ 200 ~ <prompt> ESC [ 201 ~
submit:
  CR
verify:
  transcript contains user/assistant activity after initial offset
```

This is not a long-lived "write anything into stdin whenever user steers"
contract. It is a controlled one-turn PTY run with a resumable session.

## Two Steering Modes

### Mode A - Interrupt And Resume

This is the mandatory mode for all providers.

Use when:

```text
provider is generating
tool is active
observer cannot prove input box readiness
provider is in alternate screen/TUI state
user asks to steer/replace current turn
```

Required adapter fields:

```text
session_id_source
interrupt_sequence
graceful_exit_sequence
resume_args
resume_acceptance_probe
resume_failure_detector
```

Provider mapping:

| Provider | Interrupt | Resume candidate | Notes |
|---|---|---|---|
| Claude | `/exit` after signal flag | `--resume <sessionId>` | Existing path |
| AGY | provider-specific graceful exit TBD | `--conversation <id>` or `--continue` | Conversation id from CLI log/brain path |
| Gemini | provider-specific graceful exit TBD | `--resume <id>` if proven | Current direct path has `--resume`, but PTY resume must be smoked |
| Grok | provider-specific graceful exit TBD | `--resume <id>` or trace/session id if proven | Current observed `end.sessionId` is not enough by itself |
| Copilot | provider-specific graceful exit TBD | `--connect`, `--resume`, or session id if proven | Must smoke actual CLI behavior |
| Codex | provider-specific graceful exit TBD | `exec resume <id>` if proven under PTY | Must smoke because current PTY output is text |

### Mode B - Live Input Injection

This is optional and must be provider-gated.

Use only when all are true:

```text
child still alive
no active tool according to observer
screen detector says provider is idle
screen detector says input box is focused or focus can be restored
adapter has a provider-specific submit sequence
observer verifies the new prompt after injection
```

Implementation shape:

```text
wait_for_idle()
focus_prompt_if_needed()
bracketed_paste(new_prompt)
enter
verify_prompt_accepted_after_offset()
continue tailing same observer
```

Do not use live injection:

```text
while a tool is active
while provider is asking permission
while provider is rendering a modal/trust prompt
when screen snapshot cannot identify readiness
when observer cannot prove prompt acceptance
```

## Why Mode A Is Default

It matches current cli-jaw behavior. `steerAgent()` currently kills the active
agent with reason `steer`, waits for process end, then starts orchestration for
the new prompt. It does not write a new prompt into the running child process.

It also matches current ai-e Claude safety behavior. Claude preserves session
identity and exits cleanly before the next resumed prompt.

Therefore provider-wide PTY steering should first implement Mode A. Mode B can
be added later for providers where it is stable and useful.

## Required Smokes

For each provider, before marking steer as supported:

1. Start a long-running response.
2. Send interrupt through the ai-e wrapper.
3. Verify cleanup event and no orphan child process.
4. Verify session/conversation id is persisted.
5. Resume with a new prompt.
6. Verify the new prompt reaches the observer after the previous transcript
   offset.
7. Verify old assistant tail does not leak into the new final result.
8. Verify active tool prevents idle timeout but does not prevent hard timeout.

For optional live injection:

1. Wait for idle screen state.
2. Inject a second prompt without process restart.
3. Verify the second prompt is accepted in the same session.
4. Verify multi-line bracketed paste works.
5. Verify Ctrl-C/Ctrl-D/Ctrl-Z/ESC sanitization still prevents terminal
   control injection.
6. Verify permission prompts are not accidentally accepted by raw prompt text.

## AGY-Specific Open Questions

AGY needs these smokes before coding live steering:

```text
Does Ctrl-C stop generation or exit the whole TUI?
Does /exit exist?
Does --continue continue the exact latest brain conversation?
Does --conversation <uuid> resume a known brain conversation?
Does prompt-interactive leave the TUI at an input box after a PLANNER_RESPONSE?
Can a second prompt be bracketed-pasted and accepted?
Does transcript append USER_INPUT/PLANNER_RESPONSE for that second prompt?
```

Until those are answered, AGY steer support should be Mode A only:

```text
interrupt/cleanup + --conversation or --continue resume
```

and only after resume smokes prove the correct conversation is selected.
