---
created: 2026-05-20
status: complete
tags: [ai-e, pty, external-review, grok, gpt-pro]
---
# External Review - All-Provider Claude-Style PTY

## Review Prompt Correction

Earlier review framing allowed the reviewer to recommend keeping non-PTY
native JSONL paths for some providers. That was the wrong premise.

Correct premise sent for this review:

```text
All providers should move under ai-e's Claude-style PTY supervisor.
-p/--prompt/--single is only a prompt submission mode.
JSONL/stream-json/streaming-json is only a `transcribe` source.
Provider-native JSONL may be parsed only from the PTY child.
No-PTY headless is not the final architecture.
```

## Grok Expert Capture

```text
vendor: grok
model: expert
sessionId: 01KS1NKPC60YN6E1VHR56SC4VM
url: https://grok.com/c/10753ce8-b40c-433d-80fb-e9af64cd618e?rid=88ada703-b6d8-4584-9e29-371825435eae
trace: /tmp/ai-e-pty-smoke-260520/agbrowse-trace-pty-all
```

The review accepted the corrected all-provider PTY premise.

It identified these mandatory copied parts from the current Claude path:

```text
portable_pty spawn as the execution/supervision boundary
process-group cleanup
structured completion/failure signaling
tool-activity tracking
synthesized result emission
observer tailing
initial observer offset
prompt-acceptance verification
```

It also warned that some Claude details are provider-specific, not universal:

```text
vt100 screen snapshot
bracketed-paste injection
terminal query answers
Stop/StopFailure hooks
```

Local interpretation:

```text
These details remain in the shared PTY spine as capabilities, but each adapter
must decide whether it uses hook sentinels, stdout JSONL, transcript tailing,
screen detection, or provider-specific commands.
```

## Adapter Shape From Review

The reviewed adapter surface:

```text
spawn_flags(mode) -> Vec<String>
parse_source(record) -> TranscribedEvent
transcript_locator() -> Option<PathBuf>
redact_sensitive(output)
fallback_screen_parser()
stop_handler(stop_reason)
```

This aligns with the local `04_claude_style_pty_contract.md` adapter target.

## Missing Smoke Matrix From Review

AGY follow-up is now outside this ai-e provider migration:

```text
AGY transcript id/path discovery under PTY
AGY brain transcript tailing under PTY
prompt-interactive multi-turn behavior
permission/sandbox flow
auth/cache-poll handling
resume via --conversation or --continue
interrupt cleanup and resume correctness
```

Local update after later TUI evidence:

```text
Do not code AGY as an ai-e provider in this migration.
Keep AGY review items as future/manual research only.
```

Before converting other providers:

```text
Gemini: stream-json fidelity under ai-e Rust PTY, alt-screen interference
Grok: whether tool events can be recovered separately from thought/text/end
Copilot: redaction of reasoningOpaque/encryptedContent and toolRequests coverage
Codex: structured JSONL/screen/text transcribe proof
```

## Risks From Review

```text
TTY emulation variance
mixed TUI and JSONL output
partial JSONL writes
AGY out-of-band IDE/agent activity escaping a single PTY stream
CLI update brittleness
resource/timeout overhead
no headless escape hatch
```

Local decision:

```text
These are accepted risks for Claude/Codex/Gemini/Grok/Copilot. For AGY, the
risk is high enough that AGY is excluded from the ai-e provider target.
```

## cli-jaw Prohibitions Confirmed

The review matched the local cli-jaw contract:

```text
Do not call provider binaries directly once ai-e is selected.
Do not parse raw provider stdout/JSONL in cli-jaw for ai-e providers.
Treat ai-e transcribed events plus thin projection fields as the cli-jaw-facing
contract.
Do not use AGY --print.
```

## Source Notes

The review cited Google's Gemini CLI to Antigravity transition announcement as
current context for why Antigravity must be treated as a first-class runtime.
> 출처: [An important update: Transitioning Gemini CLI to Antigravity CLI](https://developers.googleblog.com/an-important-update-transitioning-gemini-cli-to-antigravity-cli/)

It also cited the Antigravity site as the product/CLI surface entry point.
> 출처: [Google Antigravity](https://antigravity.google/)

Do not treat third-party review claims about private transcript stability as an
official guarantee. The local smoke evidence remains the concrete contract
until official AGY transcript docs are found.

## GPT Pro Status

Corrected GPT Pro query was captured with the same all-provider PTY premise:

```text
vendor: chatgpt
model: pro
effort: extended
sessionId: 01KS1NKR473FFGXF39NM6XXHKP
url: https://chatgpt.com/c/6a0d255a-087c-83a5-a96e-1cffb60f5fe6
trace: /tmp/ai-e-pty-smoke-260520/agbrowse-trace-pty-all
```

GPT Pro verdict:

```text
The corrected plan is directionally right and should be enforced strictly:
every provider should run inside ai-e's PTY supervisor.
-p / --prompt / --single is only a prompt submission mode inside PTY.
JSONL is a `transcribe` source, not an excuse to keep non-PTY architecture.
```

GPT Pro's strongest implementation correction was sequencing:

```text
Do not code AGY first unless transcript-tail completion and prompt-acceptance
semantics are proven. Gemini and Copilot are lower-risk first conversions
because their PTY JSONL behavior is already closer to Claude's transcribed
event model.
```

Local decision:

```text
AGY is no longer the migration forcing function for ai-e provider code. The
remaining provider migration is Claude/Codex/Gemini/Grok/Copilot under PTY plus
`transcribe`.
```

GPT Pro also required:

```text
provider capability probes under PTY
nonce-based prompt acceptance verification
initial observer offset/watermark
one transcribe pipeline
observer precedence: structured JSONL > transcript final marker > screen/text > process exit
redaction before user-facing projection
provider-neutral cancellation policy
cli-jaw never parses provider stdout once ai-e is selected
```

Local correction after the Codex JSONL clarification:

```text
JSONL is not a mode. Codex/Gemini/Grok/Copilot JSONL are source readers inside
the single transcribe pipeline. The public contract remains PTY supervisor plus
transcribed events plus a thin result projection for every provider.
```

## Public Source Cross-Check

Gemini CLI headless docs describe `-p` / `--prompt` as headless mode and
document streaming JSON as JSONL events including `init`, `message`,
`tool_use`, `tool_result`, `error`, and `result`.
> 출처: [Gemini CLI headless mode reference](https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/headless.md)

## GPT Pro Implementation Review

After the local implementation slice, GPT Pro was asked to validate the
implemented plan, the AGY exclusion, and the cli-jaw boundary:

```text
vendor: chatgpt
model: pro
effort: standard
sessionId: 01KS2K3X2GH1419XE2GAYNBAM1
url: https://chatgpt.com/c/6a0d9e33-5ccc-83a6-9a14-7620b0ca5963
traceId: 92439e5ba4ea5882
trace: /tmp/ai-e-pty-impl-smoke-260520/agbrowse-gptpro-trace
```

Verdict:

```text
The implemented contract is coherent after excluding AGY.
Codex, Gemini, Grok, and Copilot can move under ai-e PTY prompt-mode while
Claude keeps its existing Claude-e run lifecycle.
```

GPT Pro identified two release blockers:

```text
1. Reconcile public/local drift so ai-e and cli-jaw consistently say/test
   `ai-e <codex|gemini|grok|copilot> p <prompt>` is PTY-backed
   provider-native prompt mode.
2. Add frozen parser/redaction fixtures from smoke runs, especially Copilot
   filtered `reasoningOpaque` and `encryptedContent` behavior.
```

Local closure:

```text
README.md, structure/provider_adapter.md, structure/runtime_contract.md, and
structure/INDEX.md now describe PTY prompt-mode instead of no-PTY headless.
src/headless.rs has unit coverage for Copilot projection redaction:
reasoningOpaque, encryptedContent, reasoningId, and empty assistant.reasoning
events are removed before stdout projection.
```

Remaining caveat:

```text
Non-Claude resume remains intentionally weaker than Claude resume until ai-e
emits provider/session identifiers and can map a later turn to a provider-native
resume primitive. Current cli-jaw steering is interrupt-first via SIGINT.
```

Grok CLI docs describe `-p, --single <PROMPT>` and
`--output-format <FMT>` with `streaming-json` as newline-delimited events.
> 출처: [xAI Grok Headless & Scripting](https://docs.x.ai/build/cli/headless-scripting)

GitHub Copilot CLI docs describe `--output-format=json` as JSONL.
> 출처: [GitHub Copilot CLI command reference](https://docs.github.com/en/copilot/reference/cli-command-reference)

Codex CLI docs show `codex exec --json` writing JSONL output.
> 출처: [Codex CLI exec mode](https://www.mintlify.com/openai/codex/advanced/exec-mode)

Claude Code docs describe `--output-format stream-json`,
`--include-partial-messages`, `--resume`, and `--continue`.
> 출처: [Claude Code CLI reference](https://docs.claude.com/en/docs/claude-code/cli-reference)

Google's transition post says Gemini CLI is transitioning to Antigravity CLI
and explicitly warns against assuming immediate one-to-one parity.
> 출처: [An important update: Transitioning Gemini CLI to Antigravity CLI](https://developers.googleblog.com/an-important-update-transitioning-gemini-cli-to-antigravity-cli/)
