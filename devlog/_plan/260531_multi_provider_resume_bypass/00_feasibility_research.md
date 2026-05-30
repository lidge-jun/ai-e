---
created: 2026-05-31
status: research
tags: [ai-e, codex, gemini, grok, copilot, interactive-bypass, resume, claude-e]
---

# 00 — Multi-Provider Interactive-Bypass & Resume Feasibility

## Goal

Jun's hypothesis: migrate `codex` / `gemini` / `grok` / `copilot` to the
`claude-e`-grade **interactive bypass** pattern (launch the provider's
interactive TUI in a PTY, inject prompt via bracketed paste, capture the
response from a side-channel, detect completion), including **full resume
logic** — the same model `ai-e claude` already uses for Claude Code.

This doc records the **empirical** investigation (4 parallel sub-agents,
live CLI inspection on 2026-05-31) that confirms feasibility & difficulty
**before** any code is written.

## The claude-e pattern (reference)

Claude is the ONLY provider in `ai-e` that uses interactive bypass, because
its `-p`/print mode is not used. The 3 pillars:

- **(A) Interactive TUI in PTY** — `claude` launched without `-p`; prompt
  injected via `sanitize::bracketed_paste` + Enter.
- **(B) Side-channel capture** — tail the Claude transcript **JSONL file**
  (path provided by the SessionStart hook). The PTY screen ANSI is never
  parsed for content.
- **(C) Completion detection** — Claude's **SessionStart/Stop hook sentinels**
  (`--settings` hook JSON). No other CLI has an equivalent hook.

Key realization: claude-e never parses the full-screen TUI ANSI. It reads a
clean side-channel. Any port to other providers must do the same.

## Empirical findings (2026-05-31, live CLI inspection)

| Provider | Ver | Interactive bypass NEEDED? | Resume in one-shot? | JSON out | Reasoning/effort | Side-channel | Difficulty |
|---|---|---|---|---|---|---|---|
| **codex** | 0.135.0 | ❌ NO | ✅ `codex exec resume <uuid>` / `--last` | ✅ `--json` | ✅ `-c model_reasoning_effort="high"` | rollout JSONL + `thread.started` on stdout | 1/5 |
| **grok** | 0.2.11 | ❌ NO | ✅ `--resume <id>` / `--continue` w/ `--single` | ✅ `json`/`streaming-json` | ✅ `thought` field (`--effort` model-dependent) | `~/.grok/sessions/<cwd>/<id>/` + `sessionId` on stdout | 1/5 |
| **copilot** | 1.0.56 | ❌ NO | ✅ `--resume=<id>` / `--continue` / `--session-id` w/ `--prompt` | ✅ `--output-format json` | ✅ `--effort none..max` + `reasoningText` | `~/.copilot/session-state/<uuid>/events.jsonl` + `session-store.db` | 1/5 |
| **gemini** | 0.42.0 | ⚠️ ONLY for resume | ❌ `--prompt` makes a NEW session each run; resume only in interactive | ✅ `--output-format json` | ✅ `thoughts` array | `~/.gemini/tmp/<proj>/chats/session-*.jsonl` | 2/5 |

### codex — one-shot fully sufficient
- `codex exec --json` emits structured JSONL: `thread.started` (→ `thread_id`),
  `item.completed` (agent_message/command_execution), `turn.completed` (usage).
- Resume: `codex exec resume <UUID> "<prompt>"` or `--last` (VERIFIED live).
- Session id surfaced on stdout (`thread.started.thread_id`), in rollout
  filename, and `~/.codex/session_index.jsonl`.
- Reasoning effort: `-c 'model_reasoning_effort="high"'` (VERIFIED).
- Completion: process exit + `turn.completed`.
- **Interactive bypass would be a downgrade.**

### grok — one-shot fully sufficient
- `grok --single --output-format json` → `{text, stopReason, sessionId, requestId, thought}`.
- Resume: `--resume <id>` and `--continue` both work WITH `--single` (VERIFIED).
- Rich session store `~/.grok/sessions/<cwd>/<uuid>/` (summary.json, chat_history.jsonl,
  events.jsonl with `turn_ended`).
- Completion: process exit + `stopReason:"EndTurn"` + `{"type":"end",...}` in streaming.
- `--effort` exists but `grok-build` model rejects `reasoningEffort` (model limit, not CLI).
- **Interactive bypass would be a downgrade.**

### copilot — one-shot fully sufficient
- `copilot --prompt --output-format json` → JSONL events; final line is
  `{"type":"result","sessionId":"<uuid>","exitCode":0,"usage":{...}}`.
- Resume: `--resume=<id>` / `--continue` / `--session-id` ALL work with `--prompt` (VERIFIED).
- Reasoning: `reasoningText` in `assistant.message`; effort via `--effort none..max`.
- Side-channels: per-session `events.jsonl` + global `~/.copilot/session-store.db` (sqlite).
- ai-e's `project_copilot_jsonl_line` / `strip_copilot_opaque_fields` already filter noise.
- **Interactive bypass would be a downgrade.**

### gemini — the ONE partial case
- `gemini --prompt --output-format json` works (json + `thoughts` + tokens), BUT
  **`--prompt` does not resume** — it starts a NEW session each run.
- Resume only via interactive: `--resume latest` / `--resume <index>` (NOT by UUID),
  or `-i/--prompt-interactive "<prompt>"` (inject prompt + stay interactive).
- Side-channel: `~/.gemini/tmp/<cwd-dirname>/chats/session-<date>-<shortid>.jsonl`
  (first line has `sessionId`; turns end with `type:"gemini"` then `{"$set":{...}}`).
- So gemini is the ONLY provider where interactive bypass adds real value, and
  ONLY for multi-turn/resume. `-i` makes paste-injection unnecessary.

## Critical conclusion — the premise inverts

**"Migrate ALL to interactive bypass" is the WRONG direction for 3.5 of 4 providers.**

- codex / grok / copilot have **first-class headless APIs** with native resume +
  JSON + reasoning/effort. Forcing interactive TUI injection would be MORE
  fragile (TUI input quirks, paste handling, no clean completion hook) with ZERO
  benefit.
- gemini one-shot covers single-turn fully; only **resume** needs interactive.
- claude is special because `claude-e` deliberately avoided `-p` (legacy design);
  the other CLIs ship a proper non-interactive contract that already does what
  claude-e had to hand-roll.

## Recommended implementation (honors the underlying intent)

The user's real intent = "every provider should have complete, claude-e-grade
capability (resume + session capture + reasoning)." The optimal way to deliver
that is NOT interactive bypass, but:

### For codex / grok / copilot (one-shot + resume wiring) — difficulty 1/5 each
1. Parse the session id from native JSON/JSONL stdout
   (codex `thread.started.thread_id`, grok `sessionId`, copilot `result.sessionId`).
2. Emit the `[ai-e] session: <id>` / `[ai-e] resume: ...` footer (parity with kiro).
3. Map ai-e `--resume <id>` → provider-native resume:
   - codex → `codex exec resume <id> <prompt>`
   - grok → `grok --single <prompt> --resume <id>`
   - copilot → `copilot --prompt <prompt> --resume=<id>`
4. (codex) switch text path to `--json` projection for clean extraction.

### For gemini — difficulty 2/5
- Single-turn: keep `--prompt --output-format json` (already works).
- Resume/multi-turn (optional): interactive `gemini -i "<prompt>"` in PTY, tail
  `~/.gemini/tmp/<proj>/chats/session-*.jsonl`, completion = `type:"gemini"` + `$set`,
  session id from JSONL first line; map ai-e `--resume <uuid>` by resolving UUID →
  index via `--list-sessions` (or hold for a later phase).

### Where interactive bypass IS justified
- Only gemini-resume, and only if multi-turn gemini is required in ai-e.
- Keep the claude-e interactive harness claude-only for now.

## Difficulty matrix (resume + footer wiring, not bypass)

| Provider | Resume wiring | Session footer | Interactive bypass | Net effort |
|---|---|---|---|---|
| codex | low | low | unneeded | 1/5 |
| grok | low | low | unneeded | 1/5 |
| copilot | low | low | unneeded | 1/5 |
| gemini | medium (UUID→index) | low | optional, for resume only | 2/5 |

## Open decision for Jun

The literal goal ("전부 claude-e 급 인터랙티브 우회") conflicts with the evidence.
Two ways to proceed:

- **Plan R (recommended)** — implement native resume + session footer for all 4
  (claude-e-grade capability without the fragile bypass). gemini resume via
  interactive only if needed.
- **Plan B (literal)** — force interactive bypass for all 4 anyway. Higher cost,
  more fragile, no capability gain over Plan R. Not recommended.

Verification (sub-agent) and PABCD phasing follow once direction is confirmed.
