# Provider Adapter Contract

`ai-e` has one implemented provider class: PTY-supervised providers.

## PTY Provider

PTY providers need a terminal lifecycle because their value comes from the
provider's authenticated local runtime. Claude Code uses interactive prompt
injection; Codex, Gemini, Grok, and Copilot use native prompt flags while still
running inside the same PTY supervisor.

Required responsibilities:

- Resolve the provider binary and provider-specific env vars.
- Build provider args, including session/resume and settings/hook wiring.
- Spawn through a PTY.
- Respond to terminal capability queries.
- Detect startup, trust, permission, and stop conditions.
- Normalize transcript/log output into the shared output contract.
- Clean up the process group without losing session state.

## Current Provider Map

| Provider | Class | Entry point | Key defaults |
|---|---|---|---|
| Claude Code | PTY | `claude` | `--dangerously-skip-permissions`, workspace trust auto-accept |
| Codex CLI | PTY prompt-mode | `codex exec` | `--dangerously-bypass-approvals-and-sandbox --skip-git-repo-check` |
| Gemini CLI | PTY prompt-mode | `gemini --prompt` | `--skip-trust --approval-mode yolo` plus home-root `--include-directories` |
| Grok CLI | PTY prompt-mode | `grok --single` | `--no-alt-screen --always-approve --permission-mode bypassPermissions` |
| Copilot CLI | PTY prompt-mode | `copilot --prompt` | `--allow-all --stream off` |

AGY/Antigravity is excluded from the provider map. Do not add `ProviderKind::Antigravity`,
`ai-e antigravity`, or `agy` aliases until a non-TUI prompt contract exists.

## Future Hardening

- Move Claude-specific hook/transcript code into `src/providers/claude_code/`
  once the shared PTY prompt-mode path is stable.
- Add captured-output normalization for prompt-mode providers when their native
  JSON events are stable enough to project uniformly.
- Add provider-level smoke tests that use fake provider binaries so CI never
  calls live models.
- Add optional `agbrowse web-ai render` validation for GPT Pro plans without
  sending prompts to a live browser provider.
