# CLI Surface

## Binary Names

`ai-e` is the only public binary exposed by this package.

## Provider Form

```text
ai-e <provider> [options] <prompt>
ai-e <provider> p [options] <prompt>
ai-e <provider> run [wrapper flags] -- [provider args]
```

Providers:

| Provider | Aliases | Default Mode | Command |
|---|---|---|---|
| Claude Code | `claude`, `claude-code` | PTY + hook | `ai-e claude ...` |
| Codex CLI | `codex` | interactive | `ai-e codex ...` |
| Gemini CLI | `gemini` | interactive | `ai-e gemini ...` |
| Grok CLI | `grok` | interactive | `ai-e grok ...` |
| Copilot CLI | `copilot`, `github-copilot` | interactive | `ai-e copilot ...` |
| Kiro CLI | `kiro`, `kiro-code` | interactive (pipe) | `ai-e kiro ...` |
| Antigravity | `agy`, `antigravity` | headless | `ai-e agy ...` |

If the provider is omitted, `ai-e` defaults to `claude` for bootstrap
compatibility. New cli-jaw integration should pass the provider explicitly.

## Mode Selection

| Provider | Default | `--headless` / `-p` | `--interactive` |
|---|---|---|---|
| claude | PTY + hook | N/A | N/A |
| codex, gemini, grok, copilot, kiro | interactive bypass | headless one-shot | (already default) |
| agy | headless | (already default) | interactive bypass |

## Claude Provider

The Claude provider preserves the `claude -p`-style command surface while
driving the interactive Claude Code runtime via PTY:

```bash
ai-e claude "your prompt here"
ai-e claude p --model opus "explain quicksort"
ai-e claude --tool "use 10 tools and summarize the results"
ai-e claude --output-format json "summarize this commit" < commit.diff
ai-e claude --output-format stream-json "audit src/" --verbose | jq .
ai-e claude --resume 6a304357-e92d-47e7-a56d-c54065e12be1 "continue"
ai-e claude run --claude-bin "$(command -v claude)" -- --model opus
```

Claude binary resolution:

1. `AI_E_CLAUDE_BIN`
2. `CLAUDE_BIN`
3. `claude`

### Wrapper-Owned Flags (Claude print-compatible mode)

| Flag | Behavior |
|---|---|
| `--input-format text\|stream-json` | Reads plain stdin or extracts user text from JSONL messages. |
| `--output-format text\|json\|stream-json` | Normalizes transcript output. |
| `--idle-timeout-ms` | Idle timeout (resets on activity, suppressed during tool use). Default 600s. |
| `--hard-timeout-ms` | Absolute runtime cap. Default 3600s. |
| `--timeout-ms` | Backward-compatible alias for `--idle-timeout-ms`. |
| `--claude-bin` | Claude binary path. |
| `--cwd` | Working directory for the Claude PTY process. |
| `--cols`, `--rows` | PTY dimensions (default 120×40). |
| `--session-id` | Use a specific session id. |
| `--resume` / `-r` | Resume a Claude session. |
| `--no-session-persistence` | Suppress generated session id. |
| `--auto-accept-workspace-trust` | Accept workspace trust prompt (default: enabled). |
| `--no-auto-accept-workspace-trust` | Disable workspace trust auto-accept. |
| `--tool`, `--t`, `-t` | Print compact tool progress to stderr. |
| `--no-session-footer` | Hide the final stderr resume footer. |
| `--json-schema` | Append JSON-only schema instruction to prompt. |

### Accepted Print-Only Compatibility Flags

| Flag | Behavior |
|---|---|
| `--verbose`, `--include-partial-messages`, `--include-hook-events`, `--replay-user-messages` | Accepted and consumed. |
| `--fallback-model`, `--max-budget-usd` | Accepted and consumed (PTY path cannot enforce). |

### Forwarded Claude Flags

Boolean: `--allow-dangerously-skip-permissions`, `--bare`, `--brief`, `--chrome`,
`--continue`/`-c`, `--dangerously-skip-permissions`, `--disable-slash-commands`,
`--exclude-dynamic-system-prompt-sections`, `--fork-session`, `--ide`,
`--mcp-debug`, `--no-chrome`, `--strict-mcp-config`.

Single-value: `--agent`, `--agents`, `--append-system-prompt`,
`--append-system-prompt-file`, `--debug-file`, `--effort`, `--mcp-config`,
`--model`, `--name`/`-n`, `--permission-mode`, `--plugin-dir`, `--plugin-url`,
`--remote-control-session-name-prefix`, `--setting-sources`, `--settings`,
`--system-prompt`, `--system-prompt-file`.

Optional-value: `--debug`/`-d`, `--from-pr`, `--remote-control`, `--tmux`,
`--worktree`/`-w`.

Variadic: `--add-dir`, `--allowedTools`/`--allowed-tools`, `--betas`,
`--disallowedTools`/`--disallowed-tools`, `--file`, `--tools`.

Unknown flags starting with `-` are treated as boolean Claude flags for forward
compatibility.

Unless the caller supplies `--permission-mode`, `--permission-mode=...`,
`--dangerously-skip-permissions`, or `--allow-dangerously-skip-permissions`,
the Claude provider appends `--dangerously-skip-permissions`.

## Interactive Bypass Providers

Default mode for codex, gemini, grok, copilot, kiro. Opt-in for agy via
`--interactive`.

```bash
ai-e codex --model gpt-5-mini "summarize this repo"
ai-e gemini --model gemini-2.5-pro "summarize this repo"
ai-e grok --model auto "summarize this repo"
ai-e copilot --model gpt-5-mini "summarize this repo"
ai-e kiro --model auto "summarize this repo"
ai-e agy --interactive "respond with hello world"
```

Resume:

```bash
ai-e codex --resume <session-id> "continue"
ai-e gemini --resume <session-id> "continue"
ai-e grok --resume <session-id> "continue"
ai-e copilot --resume <session-id> "continue"
ai-e kiro --resume <session-id> "continue"
ai-e agy --interactive --resume <session-id> "continue"
```

### Interactive Flags

| Flag | Behavior |
|---|---|
| `--model` / `-m` | Forward model selection to provider. |
| `--output-format text\|json\|stream-json` | Output format (projected from session JSONL). |
| `--idle-timeout-ms` | Idle timeout. Default 600s. |
| `--hard-timeout-ms` | Hard timeout. Default 3600s. |
| `--cwd` | Working directory. |
| `--resume` / `-r` | Resume a provider session. |
| `--no-session-footer` | Hide session footer. |
| `--provider-bin` | Override provider binary for one run. |

### Provider Hardening Defaults (Interactive)

| Provider | Defaults |
|---|---|
| Codex | `--no-alt-screen --dangerously-bypass-approvals-and-sandbox` |
| Gemini | `--skip-trust --approval-mode yolo` + home-root `--include-directories` |
| Grok | `--no-alt-screen --always-approve --permission-mode bypassPermissions` |
| Copilot | `--yolo` |
| Kiro | `--trust-all-tools` |
| Agy | `--dangerously-skip-permissions` |

## Headless Providers (Legacy)

Activated by `--headless` or `-p` flag. Default for agy.

```bash
ai-e codex --headless --model gpt-5-mini "summarize this repo"
ai-e agy "respond with hello world"
```

### Headless Flags

| Flag | Behavior |
|---|---|
| `--provider-bin <path>` | Override provider binary. |
| `--model` / `-m` | Forward model selection. |
| `--output-format text\|json\|stream-json` | Provider output mode. |
| `--cwd`, `-C`, `--cd` | Working directory. |
| `--timeout-ms` | Kill provider on timeout (exit code 6). Default 600s. |
| `--no-session-footer` | Hide session footer. |
| `--` | Forward remaining args directly to provider. |

### Headless Hardening Defaults

| Provider | Underlying Command | Defaults |
|---|---|---|
| Codex | `codex exec --json <prompt>` | `--dangerously-bypass-approvals-and-sandbox --skip-git-repo-check` |
| Gemini | `gemini --prompt <prompt>` | `--skip-trust --approval-mode yolo` + `--include-directories` |
| Grok | `grok --single <prompt>` | `--no-alt-screen --always-approve --permission-mode bypassPermissions` |
| Copilot | `copilot --prompt <prompt>` | `--allow-all --stream off` |
| Kiro | `kiro-cli chat --no-interactive <prompt>` | `--trust-all-tools` |
| Agy | `agy -p <prompt>` | `--dangerously-skip-permissions --print-timeout 10m` |

## Provider Binary Resolution

| Provider | Env Vars | Default |
|---|---|---|
| Claude | `AI_E_CLAUDE_BIN`, `CLAUDE_BIN` | `claude` |
| Codex | `AI_E_CODEX_BIN`, `CODEX_BIN` | `codex` |
| Gemini | `AI_E_GEMINI_BIN`, `GEMINI_BIN` | `gemini` |
| Grok | `AI_E_GROK_BIN`, `GROK_BIN` | `grok` |
| Copilot | `AI_E_COPILOT_BIN`, `COPILOT_BIN` | `copilot` |
| Kiro | `AI_E_KIRO_BIN`, `KIRO_BIN`, `KIRO_CLI_BIN` | `kiro-cli` |
| Agy | `AI_E_AGY_BIN`, `AGY_BIN` | `agy` |

## npm Packaging

The `@bitkyc08/ai-e` npm package exposes:

```json
{
  "bin": {
    "ai-e": "bin/ai-e"
  }
}
```

`npm install -g @bitkyc08/ai-e` runs `scripts/postinstall.cjs`, which attempts
to copy a prebuilt binary from the matching optional platform package (e.g.,
`@bitkyc08/ai-e-darwin-arm64`), falling back to `cargo build --release --locked`.

Skip controls:

| Env Var | Behavior |
|---|---|
| `AI_E_SKIP_STAR_PROMPT=1` | Suppress GitHub star prompt. |
| `AI_E_SKIP_POSTINSTALL=1` | Skip all postinstall work. |
| `AI_E_SKIP_BUILD=1` | Skip native build (use when another layer provides binary). |
| `AI_E_BIN=/path/to/ai-e` | Make the npm wrapper execute a prebuilt binary. |

Release helpers:

```bash
npm run verify          # fmt:check + test + build
npm run pack:dry        # npm pack --dry-run
npm run publish:dry-run # full publish simulation
npm run release:check   # local release dry run
npm run release:npm     # publish current version
npm run release:patch   # bump patch, commit, tag, publish, GitHub release
npm run release:minor   # bump minor
npm run release:major   # bump major
npm run release:preview # publish preview dist-tag
```

GitHub Actions only verifies and dry-runs. Actual npm publishing is local-only.
