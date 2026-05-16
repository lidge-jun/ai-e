# cli-jaw Migration Target

`ai-e` is intended to become cli-jaw's common external runtime for interactive
and headless AI CLIs.

## Target Shape

```text
cli-jaw provider id -> ai-e provider command

claude   -> ai-e claude ...
codex    -> ai-e codex ...
gemini   -> ai-e gemini ...
grok     -> ai-e grok ...
copilot  -> ai-e copilot ...
```

## Detection

cli-jaw should eventually detect:

1. explicit `AI_E_BIN`;
2. embedded npm `ai-e`;
3. PATH `ai-e`;
4. provider-specific legacy helpers only as fallbacks.

Provider binaries should be resolved separately and passed through provider env
or flags:

| Provider | Preferred binary override |
|---|---|
| Claude | `AI_E_CLAUDE_BIN` or `--claude-bin` |
| Codex | `AI_E_CODEX_BIN` or `--provider-bin` |
| Gemini | `AI_E_GEMINI_BIN` or `--provider-bin` |
| Grok | `AI_E_GROK_BIN` or `--provider-bin` |
| Copilot | `AI_E_COPILOT_BIN` or `--provider-bin` |

## Rollout Recommendation

Do not replace `claude-e` immediately.

Safer sequence:

1. Keep existing `claude-e` support for the current Claude provider.
2. Add `ai-e` as a new optional runtime in cli-jaw.
3. Use `ai-e` first for non-Claude providers because those do not need the
   Claude PTY contract.
4. Once `ai-e claude` matches the required Claude behavior, promote `ai-e` to
   the preferred all-provider runtime.
5. Keep `claude-e` as a compatibility fallback until saved settings and docs no
   longer refer to it.

## UI Implications

The frontend should show provider identity separately from runtime identity:

| UI Concept | Example |
|---|---|
| Provider | Claude, Codex, Gemini, Grok, Copilot |
| Runtime | native, ai-e, claude-e legacy |
| Mode | PTY, headless |
| Permission state | bypass/yolo/allow-all/approval policy |

Avoid provider labels such as `Codex (OpenAI)` unless product ownership is
explicitly part of the requested UI copy. Keep labels short and operational.

## Required cli-jaw Work

- Add `ai-e` binary detection and doctor output.
- Add provider config fields for provider binary overrides.
- Add spawn builders for `ai-e <provider> ...`.
- Add stream handling for direct provider stdout when headless providers do not
  emit `jaw_runtime`.
- Add UI provider/runtime/status fields.
- Preserve existing Claude behavior during rollout.
