---
created: 2026-05-20
status: superseded
tags: [ai-e, antigravity, gemini, provider, migration]
---
# Antigravity Provider Migration Goal - Superseded

## Objective

Do not add `antigravity` / `agy` as an ai-e provider in this migration. Later
interactive evidence showed `agy hi` entering a full TUI session with account
and model status rendered in the terminal. Combined with the earlier PTY smokes
where AGY prompt-interactive stayed open until timeout, AGY is not a fit for the
current ai-e one-shot PTY provider contract.

The remaining implementation target is still Claude-style PTY for ai-e
providers that can be supervised as turn-oriented provider children: Claude,
Codex, Gemini, Grok, and Copilot. Each provider runs under the shared Rust PTY
supervisor, then one `transcribe` pipeline captures transcript, stdout/stderr
JSONL, logs, process status, and screen evidence. Any common cli-jaw shape
should be a thin projection over that transcript, not a forced semantic mapping
layer.

Keep the existing `gemini` provider intact as a legacy/manual safety path until
the new ai-e PTY runtime proves parity in production-like smoke tests. That is
not the final architecture. The final architecture is not no-PTY `-p` parsing;
it is ai-e PTY supervision with provider-specific sources feeding `transcribe`.

Google is transitioning Gemini CLI into Antigravity CLI, with consumer Gemini
CLI / Gemini Code Assist individual serving changes scheduled for June 18,
2026. ai-e should therefore support Antigravity directly instead of depending
only on the old Gemini CLI command surface.
> 출처: [An important update: Transitioning Gemini CLI to Antigravity CLI](https://developers.googleblog.com/an-important-update-transitioning-gemini-cli-to-antigravity-cli/)

Antigravity CLI is not guaranteed to be a 1:1 Gemini CLI replacement at launch,
so ai-e must not treat it as a drop-in provider implementation.
> 출처: [An important update: Transitioning Gemini CLI to Antigravity CLI](https://developers.googleblog.com/an-important-update-transitioning-gemini-cli-to-antigravity-cli/)

## Local Command Surface

Confirmed locally:

```bash
agy --version
# 1.0.0
```

Relevant `agy --help` flags:

```text
--print / --prompt              run a single prompt non-interactively
--prompt-interactive            seed an interactive session
--continue                      continue the most recent conversation
--conversation                  resume a previous conversation by ID
--dangerously-skip-permissions  auto-approve tool permission requests
--add-dir                       add a workspace directory, repeatable
--sandbox                       enable terminal restrictions
--print-timeout                 print-mode wait timeout
```

Antigravity CLI settings live under
`~/.gemini/antigravity-cli/settings.json`, and command-line flags can override
persistent settings for the current session.
> 출처: [Using AGY CLI](https://antigravity.google/docs/cli-using)

## Excluded Provider Shape

Do not implement this command in ai-e for now:

```bash
ai-e antigravity "summarize this repo"
```

Do not add this provider metadata in ai-e for now:

```text
ProviderKind::Antigravity
id: antigravity
label: Antigravity CLI
default binary: agy
env override order: AI_E_ANTIGRAVITY_BIN, AGY_BIN, ANTIGRAVITY_BIN
```

Observed non-PTY probe command shape:

```bash
agy --print "<prompt>" \
  --print-timeout 10m \
  --dangerously-skip-permissions \
  --add-dir "$HOME"
```

This is probe/manual evidence only. It must not become an ai-e implementation
fallback or a hidden runtime retry path.

Provider arg rules:

- Do not forward `--model` until `agy` documents or exposes model selection.
- Do not forward `--effort`; the local help surface does not expose it.
- Map ai-e `--timeout-ms` to wrapper process timeout as today, and map a
  reasonable provider timeout to `--print-timeout`.
- Use repeated `--add-dir` for home-root and explicit extra directory access.
- Do not forward `--output-format` to AGY. ai-e owns the transcribed event
  stream and any thin cli-jaw projection.

`--print` is not an ai-e integration shape and is not a fallback path. AGY is
excluded from the current ai-e provider migration rather than forced through
the Claude-style path.

Current code evidence:

```text
src/providers/mod.rs: is_pty_provider() returns true only for ClaudeCode.
src/lib.rs: non-PTY providers are routed to headless::run_provider().
src/lib.rs: the PTY path currently waits for Claude SessionStart hooks,
            extracts a transcript path, tails transcript JSONL, and emits
            tool_use/tool_result-derived terminal events.
```

Therefore the Antigravity implementation plan must not be "copy Gemini
headless and swap binary." More broadly, the provider migration plan is not
"keep parsing `-p` output." AGY is removed from the ai-e provider target, and
Claude/Codex/Gemini/Grok/Copilot should be moved to Claude-style PTY adapters
one by one.

The provider-wide contract is documented in:

```text
04_claude_style_pty_contract.md
```

## Implementation Plan

### Phase 0 - PTY Viability Probe Result

The probe result is negative for ai-e provider integration:

- `agy --prompt-interactive` created brain transcript records but did not
  naturally produce a one-shot stdout/process completion contract.
- Repeat PTY smoke also stayed open until timeout.
- Interactive `agy hi` enters a full TUI prompt, which is a user-facing terminal
  app rather than the current ai-e provider child contract.

Decision outcomes:

| Outcome | Meaning | Implementation |
|---|---|---|
| AGY excluded | TUI and transcript behavior do not fit current ai-e one-shot PTY provider contract | Do not add `ProviderKind::Antigravity`; keep AGY docs as negative/manual evidence |

### Phase 1 - PTY Runtime Generalization

- Split the current Claude-specific PTY path into a shared PTY spine and
  provider adapters:

```text
src/pty_runtime/mod.rs          shared PTY spawn, prompt injection, screen, timeout, kill
src/pty_runtime/claude.rs       Claude hook settings, SessionStart, Stop/StopFailure
src/providers/claude_code.rs    Claude provider metadata
```

- Keep Claude behavior byte-for-byte compatible before moving other providers.
- Model provider output as three layers:
  - `source`: provider's own transcript/log/stdout/stderr/screen evidence.
  - `transcribe`: ai-e's append-only event stream with provenance and offsets.
  - `projection`: minimal cli-jaw transport fields such as final text, warning,
    process status, and observed activity.

### Phase 2 - Antigravity Provider Skeleton

Skipped. Do not add `ProviderKind::Antigravity`, `antigravity`/`agy` aliases,
binary env resolution, or `ai-e antigravity` help examples in this migration.

### Phase 3 - Transcribe Contract

- For `output_format=text`, print final assistant text only.
- For `output_format=json|stream-json`, ai-e emits transcribed events plus a
  minimal transport projection. Do not forward `--output-format` to AGY.
- Do not model AGY transcript metadata inside ai-e in this migration. The earlier
  record shape remains useful as manual evidence only:

```json
{"type":"tool_observed","provider":"antigravity","source":"agy_transcript","confidence":"observed"}
```

- Do not label AGY transcript records as first-party `tool_use` / `tool_result`.
- Never infer hidden reasoning from AGY records.

### Phase 4 - Tests

- Unit test provider parsing does not include `antigravity` or `agy`.
- No fake AGY provider fixtures are required unless AGY is reconsidered later
  with a different runtime boundary.

### Phase 5 - cli-jaw Integration Readiness

cli-jaw should not call `ai-e antigravity`. Direct `agy` probes can stay in
documentation and manual smoke notes, but not in the ai-e runtime path.

## Replacement Gate

Do not remove `ai-e gemini` or direct Gemini CLI runtime until:

- a non-AGY replacement path is chosen, or AGY exposes a different supported
  non-TUI automation contract in future;
- docs distinguish Gemini API/STT/browser features from the old Gemini CLI
  runtime.
- postinstall and PATH detection do not prefer `agy` as an ai-e provider.
