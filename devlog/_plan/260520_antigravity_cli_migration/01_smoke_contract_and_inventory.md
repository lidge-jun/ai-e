---
created: 2026-05-20
status: active
tags: [ai-e, antigravity, agy, gemini, smoke, inventory]
---
# AGY Contract And Gemini Inventory

## Local AGY Contract For ai-e

The local `agy` binary is installed and reports:

```bash
agy --version
# 1.0.0
```

Smoke results from `/Users/jun/Developer/new/700_projects/cli-jaw` define the
initial ai-e provider contract:

| Behavior | Result |
|---|---|
| `agy --print "<prompt>"` | Works; answer is stdout-only |
| `agy --prompt "<prompt>"` | Works as alias |
| `agy -p "<prompt>"` | Works as alias |
| `--print-timeout 30s` | Accepted |
| `--print-timeout 1ms` | Prints `Error: timed out waiting for response` to stdout and exits `0` |
| `--dangerously-skip-permissions` | Allows tool-use prompts without interactive approval |
| `--add-dir <path>` | First repeated value becomes observed tool cwd |
| no `--add-dir` | Tool `pwd` returned `~/.gemini/antigravity-cli/scratch` |
| `--model` | Unsupported; exit `2` |
| `--output-format` | Unsupported; exit `2` |
| `--include-directories` | Unsupported; exit `2` |
| `--conversation <uuid>` | Works, but stdout includes previous assistant text plus new answer |
| `--continue` | Works, but stdout includes previous assistant text plus new answer |

The AGY CLI settings file is documented and locally observed at
`~/.gemini/antigravity-cli/settings.json`.
> 출처: [Using AGY CLI](https://antigravity.google/docs/cli-using)

## ai-e Provider Decision

Do not add `ProviderKind::Antigravity` in this migration. Current ai-e code uses
PTY only for Claude; Codex, Gemini, Grok, and Copilot should still move toward
the Claude-style PTY spine. AGY was evaluated against that model and is now
excluded because it behaves as a full interactive TUI and did not prove a
one-shot provider completion contract.

Non-PTY probe shape, for reference only:

```text
agy --print <prompt>
```

Probe/manual command shape:

```text
agy --print <prompt>
    --print-timeout <derived duration>
    --dangerously-skip-permissions
    --add-dir <cwd/home include root>
```

Do not pass Gemini CLI flags:

```text
--model
--output-format
--skip-trust
--approval-mode
--include-directories
```

Do not implement `stream-json` by forwarding to AGY. Do not create an ai-e AGY
wrapper-level event envelope in this migration.

Do not implement resume in phase 1. AGY resumed print mode emits historical
assistant text before the new answer, so ai-e cannot expose a clean
latest-turn contract without additional trimming fixtures.

Rejected implementation target:

```text
ai-e antigravity --output-format stream-json <prompt>
```

Do not implement this target. No headless or PTY fallback should be wired into
ai-e for AGY.

PTY smoke result:

| Contract | Result |
|---|---|
| AGY excluded | Brain transcript appears for simple responses, but process/TUI behavior does not fit the ai-e one-shot provider contract |

The headless `--print` result is therefore only a probe contract, not a runtime
fallback.

## ai-e Gemini Inventory

Inventory command:

```bash
rg -i --count-matches \
  "gemini|google-generative|generativeai|@google/genai|@google/generative-ai|GEMINI|GOOGLE_API_KEY|vertex|bard" \
  --glob '!target/**' --glob '!node_modules/**'
```

Each entry is `path:match_count`.

```text
AGENTS.md:1
README.md:15
devlog/_plan/260516_ai_e_multi_provider/00_goal.md:3
devlog/_plan/260516_ai_e_multi_provider/01_implementation_log.md:11
devlog/_plan/260520_antigravity_cli_migration/00_goal.md:16
package.json:1
src/headless.rs:11
src/lib.rs:6
src/providers/mod.rs:12
structure/INDEX.md:5
structure/cli_jaw_migration.md:5
structure/cli_surface.md:9
structure/provider_adapter.md:3
structure/runtime_contract.md:4
```

## ai-e Edit Targets

First implementation files for the remaining PTY provider migration:

```text
src/providers/mod.rs
src/pty_runtime/mod.rs
src/pty_runtime/claude.rs
src/headless.rs
src/lib.rs
README.md
structure/runtime_contract.md
structure/cli_surface.md
structure/provider_adapter.md
structure/INDEX.md
structure/cli_jaw_migration.md
```

Keep the current no-PTY `ai-e gemini` path intact only as a legacy/manual
safety path until the Claude-style PTY adapters have smoke coverage and
cli-jaw can consume their PTY-transcribed output contracts.

Broader follow-up: once the PTY runtime is generalized from the current Claude
path, move Codex, Gemini, Grok, and Copilot from the headless path to
provider-specific PTY adapters one at a time. This is not a blind bulk switch:
each provider needs a separate transcript/stdout-jsonl/screen/completion/steer
smoke. AGY should not be added as an ai-e runtime path.
