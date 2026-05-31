# cli-jaw Migration Target

`ai-e` is cli-jaw's common external runtime for interactive AI CLIs.

## Target Shape

```text
cli-jaw provider id -> ai-e provider command

claude   -> ai-e claude ...   (PTY interactive, hook-based)
codex    -> ai-e codex ...    (interactive, session file tail + resume)
gemini   -> ai-e gemini ...   (interactive, session file tail + resume)
grok     -> ai-e grok ...     (interactive, session file tail + resume)
copilot  -> ai-e copilot ...  (interactive, session file tail + resume)
kiro     -> ai-e kiro ...     (pipe, session footer + resume)
agy      -> ai-e agy ...      (headless -p, session footer + resume)
```

## Detection

cli-jaw resolves one external runtime (`ai-e`) and selects providers through
explicit command arguments:

1. `AI_E_BIN` environment variable.
2. Embedded npm `ai-e` in cli-jaw's node_modules.
3. PATH `ai-e`.
4. Provider-specific legacy helpers only as fallbacks (`claude-e`, `jaw-claude-i`).

Provider binaries are resolved separately via provider env or flags:

| Provider | Preferred Binary Override |
|---|---|
| Claude | `AI_E_CLAUDE_BIN` or `--claude-bin` |
| Codex | `AI_E_CODEX_BIN` or `--provider-bin` |
| Gemini | `AI_E_GEMINI_BIN` or `--provider-bin` |
| Grok | `AI_E_GROK_BIN` or `--provider-bin` |
| Copilot | `AI_E_COPILOT_BIN` or `--provider-bin` |
| Kiro | `AI_E_KIRO_BIN` or `--provider-bin` |
| Agy | `AI_E_AGY_BIN` or `--provider-bin` |

## Current Status

`ai-e` is the active runtime for all 7 providers. The `claude-e` standalone
package remains published but is no longer the preferred path for new
integrations.

## Session Contract

cli-jaw expects the session footer on stderr:

```text
[ai-e] session: <session-id>
[ai-e] resume: ai-e <provider> --resume <session-id> "your next prompt"
```

cli-jaw parses this footer to persist session IDs for multi-turn conversations.
All 7 providers emit this footer.

## Required cli-jaw Work (remaining)

- Add `ai-e` binary detection and doctor output.
- Add provider config fields for provider binary overrides.
- Add spawn builders for `ai-e <provider> ...`.
- Add stream handling for interactive bypass stdout (session JSONL projection).
- Add UI provider/runtime/status fields.
- Preserve existing Claude behavior during rollout.

## UI Implications

The frontend should show provider identity separately from runtime identity:

| UI Concept | Example |
|---|---|
| Provider | Claude, Codex, Gemini, Grok, Copilot, Kiro, Agy |
| Runtime | ai-e, claude-e legacy |
| Mode | interactive, headless, PTY+hook |
| Permission state | bypass/yolo/allow-all/approval policy |
