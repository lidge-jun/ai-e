# CLI Surface

## Binary Names

`ai-e` is the only public binary exposed by this package.

Provider-specific commands such as `claude-e`, `gemini-e`, `grok-e`, or
`codex-e` are intentionally not exposed from this package yet. That avoids
colliding with standalone packages while the common runtime is still being
scaffolded.

## Provider Form

Primary shape:

```text
ai-e <provider> [print-compatible args] <prompt>
ai-e <provider> p [print-compatible args] <prompt>
ai-e <provider> print [print-compatible args] <prompt>
ai-e <provider> run [wrapper flags] -- [provider args]
```

Providers:

| Provider | Status | Command |
|---|---|---|
| Claude Code | runnable | `ai-e claude ...` |
| Codex CLI | headless | `ai-e codex ...` |
| Gemini CLI | headless | `ai-e gemini ...` |
| Grok CLI | headless | `ai-e grok ...` |
| Copilot CLI | headless | `ai-e copilot ...` |

For bootstrap compatibility, omitting `<provider>` currently defaults to
`claude`. New cli-jaw integration should always pass the provider explicitly.

## Claude Provider

The copied Claude provider preserves the `claude -p`-style command surface while
staying PTY-backed internally:

```bash
ai-e claude "your prompt here"
ai-e claude p "your prompt here"
ai-e claude --model opus "explain quicksort"
ai-e claude --output-format json "summarize this commit" < commit.diff
ai-e claude --output-format stream-json "audit src/" --verbose | jq .
ai-e claude run --claude-bin "$(command -v claude)" -- --model opus
```

Claude binary resolution:

1. `AI_E_CLAUDE_BIN`
2. `CLAUDE_BIN`
3. `claude`

Wrapper-owned flags:

| Flag | Behavior |
|---|---|
| `--input-format text|stream-json` | Reads plain stdin or extracts user text from JSONL messages. |
| `--output-format text|json|stream-json` | Normalizes transcript output to the requested print-style shape. |
| `--idle-timeout-ms`, `--hard-timeout-ms`, `--timeout-ms`, `--claude-bin`, `--cwd`, `--cols`, `--rows` | PTY wrapper controls for the Claude provider. `--timeout-ms` remains an idle-timeout compatibility alias. |
| `--session-id` | Uses the provided session id for the generated PTY session. |
| `--no-session-persistence` | Suppresses generated session id. |
| `--resume` / `-r` | Resumes the provided session id in the PTY path. |
| `--json-schema` | Current scaffold appends a JSON-only schema instruction; future work should attach `structured_output`. |
| `--auto-accept-workspace-trust`, `--no-auto-accept-workspace-trust` | Controls pre-SessionStart workspace/folder trust prompt handling. |
| `--tool`, `--t`, `-t` | Prints compact tool-use and tool-result progress to stderr. |
| `--no-session-footer` | Hides the final stderr resume footer in print-compatible mode. |

Accepted print-only compatibility flags:

| Flag | Behavior |
|---|---|
| `--verbose`, `--include-partial-messages`, `--include-hook-events`, `--replay-user-messages` | Accepted and consumed; transcript replay owns PTY output timing. |
| `--fallback-model`, `--max-budget-usd` | Accepted and consumed because the PTY path cannot enforce Claude print-mode fallback or budget policy. |

Forwarded provider flags include `--model`, `--effort`, `--permission-mode`,
`--add-dir`, `--allowed-tools`, `--tools`, `--mcp-config`, `--settings`,
`--system-prompt`, `--append-system-prompt`, `--plugin-dir`, `--plugin-url`,
browser flags, MCP debug flags, and related Claude global controls.

If the caller does not supply `--permission-mode`,
`--permission-mode=...`, `--dangerously-skip-permissions`, or
`--allow-dangerously-skip-permissions`, the Claude provider appends
`--dangerously-skip-permissions` before spawning Claude.

## Headless Provider Mapping

Non-Claude providers use native non-interactive CLI modes instead of the Claude
PTY/hook/transcript path.

| Provider | Underlying command | Output mapping | Hardening defaults |
|---|---|---|---|
| Codex | `codex exec` | `--json` for `json` and `stream-json` | `--dangerously-bypass-approvals-and-sandbox` |
| Gemini | `gemini --prompt` | `--output-format text|json|stream-json` | `--skip-trust --yolo` |
| Grok | `grok --single` | `stream-json` maps to `streaming-json` | `--always-approve --permission-mode bypassPermissions` |
| Copilot | `copilot --prompt` | `stream-json` maps to Copilot `json` JSONL | `--allow-all --stream off` |

Shared headless flags:

| Flag | Behavior |
|---|---|
| `--provider-bin <path>` | Override the provider binary for one run. |
| `--model <model>` / `-m <model>` | Forward model selection. Codex tests pin `gpt-5-mini`. |
| `--output-format text|json|stream-json` | Select provider output mode where supported. |
| `--cwd`, `-C`, `--cd` | Working directory for the provider process. |
| `--timeout-ms <ms>` | Kill provider process on timeout, returning exit code `6`. |
| `--` | Forward the rest of the args directly to the provider. |

Provider binary resolution:

| Provider | Env resolution |
|---|---|
| Codex | `AI_E_CODEX_BIN`, `CODEX_BIN`, `codex` |
| Gemini | `AI_E_GEMINI_BIN`, `GEMINI_BIN`, `gemini` |
| Grok | `AI_E_GROK_BIN`, `GROK_BIN`, `grok` |
| Copilot | `AI_E_COPILOT_BIN`, `COPILOT_BIN`, `copilot` |

`gh copilot` is not used directly by default. If needed, set
`AI_E_COPILOT_BIN` to a wrapper script that executes `gh copilot --`.

## npm Packaging

The `ai-e` npm package exposes:

```json
{
  "bin": {
    "ai-e": "bin/ai-e"
  }
}
```

`npm install -g ai-e` runs `scripts/postinstall.cjs`, which builds
`target/release/ai-e` with Cargo and performs a one-time GitHub star prompt.
When npm is non-interactive, postinstall prints the repository URL instead of
blocking the install.

Skip controls:

| Env var | Behavior |
|---|---|
| `AI_E_SKIP_STAR_PROMPT=1` | Suppress only the GitHub star prompt. |
| `AI_E_SKIP_POSTINSTALL=1` | Skip all postinstall work. |
| `AI_E_SKIP_BUILD=1` | Skip the native build. |
| `AI_E_BIN=/path/to/ai-e` | Make the npm wrapper execute a prebuilt binary. |

Release helpers:

```bash
npm run verify
npm run pack:dry
npm run publish:dry-run
npm run release:check
npm run release:npm
npm run release:patch
npm run release:minor
npm run release:major
npm run release:preview
```

GitHub Actions must not publish to npm. CI only verifies and dry-runs package
contents; the real npm publish step is intentionally local-only after
interactive npm authentication.
