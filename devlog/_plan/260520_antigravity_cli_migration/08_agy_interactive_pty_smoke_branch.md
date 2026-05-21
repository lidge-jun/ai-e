---
created: 2026-05-21
status: planning
tags: [ai-e, agy, antigravity, interactive-pty, smoke, tmux]
---
# AGY Interactive PTY Smoke Branch

## Branch Boundary

This document reopens AGY only for a separate experimental branch:

```text
branch: agent/agy-interactive-pty-smokes
scope: devlog + smoke contract first, code later
main contract: unchanged; AGY remains excluded from released ai-e providers
```

The earlier exclusion remains correct for the shipped `ai-e` contract:

```text
AGY does not fit ai-e prompt-mode provider execution.
Do not route AGY through `agy -p`, `--print`, `--prompt`, or any one-shot prompt
surface as the final ai-e integration.
```

The new question is narrower:

```text
Can ai-e drive AGY as a full interactive TUI by spawning `agy` in a PTY,
detecting the input box, pasting the prompt, pressing Enter, and transcribing
the resulting answer?
```

That is technically possible enough to investigate, but it is a new provider
class, not the existing Codex/Gemini/Grok/Copilot prompt-mode path.

## Tooling Decision

Local harness availability:

```text
tmux: available at /opt/homebrew/bin/tmux
cmux: not currently on PATH
```

Use `tmux` as the first smoke harness. `cmux` can be added later only if it is
installed and gives better pane capture or session orchestration.

## Why AGY Was Removed

AGY was removed because the installed `agy` command does not currently behave
like the other one-shot provider CLIs:

```text
codex   -> accepts prompt through `exec ... <prompt>`
gemini  -> accepts prompt through `--prompt`
grok    -> accepts prompt through `--single`
copilot -> accepts prompt through `--prompt`
agy     -> opens a full TUI; one-shot prompt mode is not reliable enough
```

So the rejected design remains rejected:

```text
ai-e agy p "prompt"
  -> agy -p "prompt"
  -> wait for stdout
```

The reopened design is:

```text
ai-e agy p "prompt"
  -> spawn `agy` in a PTY
  -> observe vt100 screen until the TUI input row is ready
  -> bracketed-paste or raw-type the prompt
  -> send Enter
  -> transcribe PTY bytes / screen / AGY logs
  -> synthesize text/json/stream-json output
  -> terminate or preserve the session without orphaning AGY
```

## Implementation Shape To Validate

Do not start with production code. First prove these contracts in tmux:

```text
1. AGY startup readiness can be detected from screen text.
2. Prompt injection is accepted without corrupting multiline text.
3. A final answer can be captured from PTY bytes, screen text, or AGY logs.
4. Completion can be detected without hanging forever.
5. Cleanup can exit AGY without leaving orphan processes.
6. Resume or steer behavior can be defined from an observable session id or a
   live TUI session, not guessed.
```

If those pass, the likely code shape is:

```text
src/providers/mod.rs
  Add ProviderKind::Agy only on this branch after smoke proof.

src/headless.rs
  Route ProviderKind::Agy away from prompt-mode build_provider_args().

src/interactive_tui.rs
  Shared PTY TUI runner:
  - open PTY
  - spawn binary
  - keep writer
  - capture raw bytes
  - capture vt100 screen snapshots
  - wait for readiness predicate
  - paste/type prompt
  - wait for completion predicate
  - graceful cleanup

src/providers/agy.rs
  AGY predicates:
  - readiness screen matcher
  - thinking/busy matcher
  - answer extractor
  - log/session-id extractor if available
```

Do not modify cli-jaw to select `ai-e` AGY until this branch has deterministic
smoke evidence.

## Smoke Matrix

All smokes should write artifacts under a timestamped root:

```text
/tmp/ai-e-agy-interactive-pty-YYMMDD-HHMMSS/
```

Required artifacts:

```text
env.txt                 command paths, AGY help/version where available
tmux-start.capture      first visible AGY screen
tmux-ready.capture      screen when input row is detected
tmux-after-paste.capture screen after prompt injection
tmux-answer.capture     answer-visible screen
raw-pane.log            periodic `tmux capture-pane -p` samples
agy-log-paths.txt       discovered AGY log/transcript paths
agy-transcript.*        copied transcript/log excerpts when available
summary.md              pass/fail with exact timestamps and exit/cleanup state
```

### S0 - Environment And Negative One-Shot Baseline

Purpose:

```text
Record why this branch is not allowed to use `agy -p`.
```

Commands:

```bash
command -v agy
agy --help
agy -p "Reply exactly: AGY_ONESHOT_NEGATIVE_0521"
agy --print-timeout 45s -p "Reply exactly: AGY_ONESHOT_NEGATIVE_0521"
```

Expected result:

```text
Document exit code, stdout, stderr, and timeout behavior.
If one-shot succeeds later, keep the result as evidence but do not silently
switch this branch to one-shot mode. The branch goal is interactive PTY.
```

### S1 - TUI Startup Readiness

Purpose:

```text
Prove that the input-ready state can be detected from a terminal pane.
```

Harness:

```bash
tmux new-session -d -s ai-e-agy-smoke -c /tmp/ai-e-agy-smoke 'agy'
tmux capture-pane -t ai-e-agy-smoke:0.0 -p
```

Candidate readiness signals:

```text
Antigravity CLI
? for shortcuts
Gemini 3.5 Flash
line beginning with ">"
visible input cursor on the prompt row
```

Acceptance:

```text
Readiness detected within 20s.
The matcher must not depend on a specific signed-in email address.
The matcher must tolerate model/account text changing.
```

### S2 - Single-Line Prompt Injection

Purpose:

```text
Prove `tmux send-keys` style prompt entry works before implementing ai-e writer
injection.
```

Harness:

```bash
tmux send-keys -t ai-e-agy-smoke:0.0 -l 'Reply exactly: AGY_TMUX_SINGLE_OK_0521'
tmux send-keys -t ai-e-agy-smoke:0.0 Enter
tmux capture-pane -t ai-e-agy-smoke:0.0 -p
```

Acceptance:

```text
The visible input row contains the prompt before Enter, or a log/transcript
records the user prompt after Enter.
AGY begins processing without requiring manual clicks.
```

### S3 - Final Answer Capture

Purpose:

```text
Prove the final assistant answer can be captured from at least one source.
```

Sources to check, in order:

```text
1. raw PTY bytes / pane captures
2. AGY log file from `--log-file` if usable in TUI mode
3. ~/.gemini/antigravity-cli brain transcript JSONL
4. screen snapshot after quiescence
```

Acceptance:

```text
`AGY_TMUX_SINGLE_OK_0521` appears in an artifact without manual copy/paste.
The source is stable enough to implement an extractor.
```

### S4 - Multiline Prompt Injection

Purpose:

```text
Prove bracketed paste or tmux buffer paste handles multiline prompts.
```

Harness:

```bash
tmux set-buffer -t ai-e-agy-smoke 'Reply exactly:
AGY_TMUX_MULTILINE_OK_0521'
tmux paste-buffer -t ai-e-agy-smoke:0.0
tmux send-keys -t ai-e-agy-smoke:0.0 Enter
```

Acceptance:

```text
No premature submit on embedded newline.
The full multiline prompt reaches AGY.
The answer sentinel is captured.
```

If tmux buffer paste cannot preserve the needed semantics, test ai-e's existing
bracketed paste bytes in a small fake-TUI fixture before live AGY.

### S5 - Tool Permission / Auto-Approve Behavior

Purpose:

```text
Discover whether `--dangerously-skip-permissions` makes AGY tool prompts
non-blocking in TUI mode.
```

Harness:

```bash
tmux new-session -d -s ai-e-agy-tool-smoke -c /tmp/ai-e-agy-tool-smoke \
  'agy --dangerously-skip-permissions'
```

Prompt:

```text
In this temporary directory only, run pwd and then answer exactly:
AGY_TMUX_TOOL_OK_0521
```

Acceptance:

```text
No blocking permission dialog remains.
If AGY shows a permission UI, record the screen text and required key sequence.
Do not implement blind auto-key approval until this is stable.
```

### S6 - Interrupt And Steer

Purpose:

```text
Decide whether AGY steering should use Ctrl-C + resumed prompt, or live input
after the TUI returns to ready state.
```

Harness:

```bash
tmux send-keys -t ai-e-agy-smoke:0.0 -l 'Write a long numbered list from 1 to 200.'
tmux send-keys -t ai-e-agy-smoke:0.0 Enter
tmux send-keys -t ai-e-agy-smoke:0.0 C-c
tmux capture-pane -t ai-e-agy-smoke:0.0 -p
```

Acceptance:

```text
Ctrl-C must either return to a ready input row or emit a resumable session hint.
If Ctrl-C exits the process or leaves the TUI in an ambiguous state, steer is
not accepted for the first integration.
```

### S7 - Resume Contract

Purpose:

```text
Find the exact session id and native resume surface for AGY interactive runs.
```

Sources:

```text
visible resume hint
`--conversation <id>` in logs
`Created conversation <id>` in logs
brain transcript directory UUID
```

Acceptance:

```text
A second AGY process can resume the same conversation using only machine-read
state from the first run.
If this cannot be proven, ai-e AGY resume remains disabled and cli-jaw must
treat AGY as fresh-only.
```

### S8 - Structured/Tool Event Limits

Purpose:

```text
Avoid overclaiming Claude-like structured events from a TUI-only source.
```

Acceptance:

```text
First accepted output shape is text and synthetic `assistant`/`result` events.
No native `thinking_delta`, `tool_use`, `tool_result`, token usage, or model id
is claimed unless AGY exposes them in a stable log/transcript artifact.
```

### S9 - Cleanup / Orphan Check

Purpose:

```text
Ensure the harness and future ai-e provider do not leave live AGY processes.
```

Harness:

```bash
tmux kill-session -t ai-e-agy-smoke
tmux kill-session -t ai-e-agy-tool-smoke
```

Follow-up process check:

```text
Use the local process listing available in the execution environment and record
any remaining agy/Antigravity child processes in summary.md.
```

Acceptance:

```text
No orphaned AGY process remains after cleanup.
If AGY keeps helper/background processes by design, identify which are expected
and which are smoke failures.
```

## First Integration Gate

Do not write production AGY provider code until these are true:

```text
S1 readiness PASS
S2 single-line injection PASS
S3 final answer capture PASS
S9 cleanup PASS
```

Do not wire AGY into cli-jaw until these are also true:

```text
S4 multiline injection PASS
S5 permission/tool behavior classified
S6 steering classified
S7 resume classified or explicitly disabled
S8 output limits documented in README + structure
fake-TUI unit tests exist for readiness, paste, completion, timeout, cleanup
```

## Expected ai-e Public Contract If Proven

The public shape can stay:

```bash
ai-e agy p "your prompt"
ai-e agy --output-format stream-json "your prompt"
```

But the implementation must be described differently:

```text
AGY provider class: interactive TUI PTY
prompt submission: input-row detection + PTY paste + Enter
output source: AGY logs/transcript if available, else bounded PTY transcript
structured output: synthetic wrapper-side only
resume: disabled until exact machine-readable conversation id proof
```

## Rejected Shortcuts

Do not accept these as solutions on this branch:

```text
Use `agy -p` as fallback.
Assume the first `>` character means input-ready without surrounding screen
context.
Assume final answer is complete because stdout went quiet once.
Parse model/quota/tool events from visible text and call them native structured
events.
Kill all matching `agy` processes globally as cleanup.
Merge to main before smoke artifacts exist.
```
