# Provider Adapter Contract

`ai-e` has two provider classes.

## PTY Provider

PTY providers need a terminal lifecycle because their value comes from the
interactive runtime. Claude Code is the first PTY provider.

Required responsibilities:

- Resolve the provider binary and provider-specific env vars.
- Build provider args, including session/resume and settings/hook wiring.
- Spawn through a PTY.
- Respond to terminal capability queries.
- Detect startup, trust, permission, and stop conditions.
- Normalize transcript/log output into the shared output contract.
- Clean up the process group without losing session state.

## Headless Provider

Headless providers already expose a non-interactive command. Codex, Gemini,
Grok, and Copilot currently use this class.

Required responsibilities:

- Resolve the provider binary.
- Map common `ai-e` flags into provider flags.
- Add conservative unattended defaults.
- Forward explicit provider args after `--`.
- Enforce wrapper timeout.
- Preserve provider stdout/stderr without inventing unsupported semantics.

## Current Provider Map

| Provider | Class | Entry point | Key defaults |
|---|---|---|---|
| Claude Code | PTY | `claude` | `--dangerously-skip-permissions`, workspace trust auto-accept |
| Codex CLI | Headless | `codex exec` | `--dangerously-bypass-approvals-and-sandbox --skip-git-repo-check` |
| Gemini CLI | Headless | `gemini --prompt` | `--skip-trust --approval-mode yolo` plus home-root `--include-directories` |
| Grok CLI | Headless | `grok --single` | `--no-alt-screen --always-approve --permission-mode bypassPermissions` |
| Copilot CLI | Headless | `copilot --prompt` | `--allow-all --stream off` |

## Future Hardening

- Move Claude-specific hook/transcript code into `src/providers/claude_code/`
  once more PTY providers exist.
- Add captured-output normalization for headless providers when their native
  JSON events are stable enough.
- Add provider-level smoke tests that use fake provider binaries so CI never
  calls live models.
- Add optional `agbrowse web-ai render` validation for GPT Pro plans without
  sending prompts to a live browser provider.
