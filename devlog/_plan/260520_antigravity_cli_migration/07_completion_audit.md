---
created: 2026-05-20
status: evidence
tags: [ai-e, pty, completion-audit, antigravity, providers]
---
# Completion Audit - PTY Implementation Slice

## Scope

This audit was originally created for the non-implementation preparation pass.
It is now updated with the implementation slice completed on 2026-05-20:

```text
AGY is excluded. Claude keeps the Claude-e run lifecycle. Codex, Gemini, Grok,
and Copilot run provider-native prompt/p mode inside a PTY, with provider JSONL
or text relayed as the transcribe/projection source.
```

## Implementation Evidence

Changed ai-e files:

```text
src/headless.rs
src/lib.rs
src/providers/mod.rs
README.md
structure/INDEX.md
structure/provider_adapter.md
structure/runtime_contract.md
```

Key implemented behavior:

```text
ProviderKind::is_pty_provider() is true for Claude/Codex/Gemini/Grok/Copilot.
Non-Claude providers no longer spawn via plain stdio Command; they spawn inside
portable_pty and relay PTY bytes to stdout.
Codex/Gemini/Grok/Copilot still use their native p/prompt command surfaces.
Copilot projection strips reasoningOpaque/encryptedContent/reasoningId fields
and drops empty assistant.reasoning events before stdout.
No AGY/Antigravity provider enum, parser alias, env var, or adapter was added.
```

## Requirement Audit

| Requirement | Evidence | Status |
|---|---|---|
| Runtime implementation slice | `src/headless.rs`, `src/lib.rs`, and `src/providers/mod.rs` implement PTY prompt-mode for Codex/Gemini/Grok/Copilot | Satisfied |
| Repeated PTY smoke tests | `/tmp/ai-e-pty-smoke-260520` and `/tmp/ai-e-pty-smoke-260520-repeat` | Satisfied |
| All providers covered | `03_provider_pty_smoke_matrix.md` covers Claude, Codex, Gemini, Grok, and Copilot as PTY targets; AGY evidence is recorded as excluded/manual | Satisfied |
| Claude-e-style all-provider target | `04_claude_style_pty_contract.md` defines shared PTY supervisor plus provider adapters | Satisfied |
| `transcribe` is the single runtime event boundary | `04_claude_style_pty_contract.md` and `02_pty_first_refactor_plan.md` define PTY -> transcribe -> thin projection | Satisfied |
| No final headless fallback | `00_goal.md`, `02_pty_first_refactor_plan.md`, and `04_claude_style_pty_contract.md` forbid no-PTY final architecture for remaining providers and reject AGY `--print` fallback | Satisfied |
| Codex JSONL handled correctly | `03_provider_pty_smoke_matrix.md` and `04_claude_style_pty_contract.md` state Codex JSONL is a PTY child `transcribe` source, not a separate cli-jaw API | Satisfied |
| AGY exclusion documented | First and repeat smokes plus later TUI evidence show AGY does not fit the ai-e one-shot provider contract | Satisfied |
| Tool/subagent limitations documented | `03_provider_pty_smoke_matrix.md` keeps AGY tool/subagent work as future/manual research only | Satisfied |
| Steer/resume semantics documented | `05_steer_resume_contract.md` chooses interrupt-and-resume first, live input-box steering only after provider proof | Satisfied |
| External Grok Expert review | `06_external_review_pty_all.md` records Grok Expert session, URL, trace, and findings | Satisfied |
| External GPT Pro review | `06_external_review_pty_all.md` records both corrected plan review and post-implementation GPT Pro session, URL, trace, blockers, and local closure | Satisfied |
| Official/docs source citations | `06_external_review_pty_all.md` cites Gemini, Grok, Copilot, Codex, Claude, Google transition, and Antigravity sources | Satisfied |
| Implementation locations documented | `00_goal.md`, `01_smoke_contract_and_inventory.md`, and `02_pty_first_refactor_plan.md` list ai-e modules and follow-up structure docs | Satisfied |
| No orphan smoke processes | `pgrep` check returned no matching smoke, provider, or agbrowse processes | Satisfied |

## Smoke Evidence

Implementation smoke root:

```text
/tmp/ai-e-pty-impl-smoke-260520
```

Implementation smoke results:

```text
Codex:   codex.out -> thread.started + agent_message AI_E_CODEX_PTY_OK_260520, exit 0
Gemini:  gemini.out -> init/message/result + assistant deltas AI_E_GEMINI_PTY_OK_260520, exit 0
Grok:    grok.out -> thought/text/end + AI_E_GROK_PTY_OK_260520, exit 0
Copilot: copilot.out -> assistant.message AI_E_COPILOT_PTY_OK_260520, exit 0
Copilot filtered: copilot-filter2.out -> assistant.message AI_E_COPILOT_FILTER2_OK_260520,
                  no reasoningOpaque/encryptedContent field names in projected output, exit 0
```

First PTY smoke root:

```text
/tmp/ai-e-pty-smoke-260520
```

Repeat PTY smoke root:

```text
/tmp/ai-e-pty-smoke-260520-repeat
```

First-pass provider evidence:

```text
Claude:  claude-prompt-stream-verbose.log, claude-tool-stream.log
Codex:   codex-prompt.log, codex-tool.log
Gemini:  gemini-prompt-stream.log, gemini-tool-stream.log
Grok:    grok-prompt-stream.log, grok-tool-stream.log
Copilot: copilot-prompt-json.log, copilot-tool-json.log
AGY excluded/manual: agy-prompt-interactive.log, agy-prompt-interactive-cli.log,
         ~/.gemini/antigravity-cli/brain/2ec328fe-6c6d-4dc3-89fe-d6d104181738/.system_generated/logs/transcript.jsonl
```

Repeat provider evidence:

```text
Claude:  claude-repeat.log -> CLAUDE_REPEAT_OK_260520
Codex:   codex-repeat.log -> CODEX_REPEAT_OK_260520, exit 0
Gemini:  gemini-repeat.log -> GEMINI_REPEAT_OK_260520
Grok:    grok-repeat.log -> GROK_REPEAT_OK_260520
Copilot: copilot-repeat.log -> COPILOT_REPEAT_OK_260520
AGY excluded/manual: agy-repeat.log -> timeout 124,
         ~/.gemini/antigravity-cli/brain/7d0e5f2c-2c46-4615-a590-87391154a453/.system_generated/logs/transcript.jsonl
         -> AGY_REPEAT_OK_260520
```

## Final Contract

The prepared architecture is:

```text
provider CLI
  -> ai-e PTY supervisor
  -> provider-specific source readers
  -> one transcribe pipeline
  -> transcribed event stream
  -> optional thin cli-jaw projection
```

Do not implement:

```text
provider-specific no-PTY headless fallback as final architecture
AGY `--print` runtime fallback
AGY ai-e provider adapter
ai-e antigravity
cli-jaw direct parsing of provider JSONL when ai-e is selected
forced provider-wide semantic normalization
```

## Verification Commands

Latest verification run:

```text
ai-e: cargo fmt --check
ai-e: cargo test --locked
ai-e: cargo build --locked
ai-e: cargo build --release --locked
cli-jaw: bash structure/verify-counts.sh
cli-jaw: npm run typecheck:frontend
cli-jaw: npm test -- --runInBand tests/unit/agent-args.test.ts tests/unit/steer-flow.test.ts
```

Expected result:

```text
ai-e cargo fmt/test/build/release build: PASS
verify-counts: PASS
typecheck: PASS
cli-jaw npm test command: PASS, 2887 tests, 2873 pass, 0 fail, 14 skipped
```

## Post-Implementation External Verification

GPT Pro implementation review:

```text
vendor: chatgpt
model: pro
effort: standard
sessionId: 01KS2K3X2GH1419XE2GAYNBAM1
url: https://chatgpt.com/c/6a0d9e33-5ccc-83a6-9a14-7620b0ca5963
traceId: 92439e5ba4ea5882
trace: /tmp/ai-e-pty-impl-smoke-260520/agbrowse-gptpro-trace
```

Local response to release blockers:

```text
The public/local contract is now reconciled in ai-e README and structure docs:
non-Claude providers use `ai-e <provider> p ...` under PTY prompt-mode.
Copilot projection redaction is covered by unit tests for reasoningOpaque,
encryptedContent, reasoningId, and empty assistant.reasoning events.
```
