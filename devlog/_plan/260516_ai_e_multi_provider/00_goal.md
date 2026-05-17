---
created: 2026-05-16
status: active
tags: [ai-e, multi-provider, cli-jaw, rust, npm]
---
# ai-e Multi-Provider Runtime Goal

## Objective

Build `ai-e` as a new standalone repository copied from `claude-e` but reshaped
into a modular runtime for multiple AI CLIs.

The repository must keep `claude-e` untouched and independently usable.

## Providers

Initial provider set:

- `claude`: PTY-backed Claude Code provider copied from `claude-e`.
- `codex`: headless provider using `codex exec`; tests must cover
  `--model gpt-5-mini`.
- `gemini`: headless provider using `gemini --prompt`.
- `grok`: headless provider using `grok --single`.
- `copilot`: headless provider using `copilot --prompt`.

## Hardening Requirements

- One public npm binary: `ai-e`.
- No `claude-e` bin collision from this package.
- Provider-explicit command shape: `ai-e <provider> ...`.
- Wrapper timeout for headless providers.
- Activity-aware timeout for the Claude PTY provider: transcript activity and
  active tools keep the idle timeout from killing a live run, while a separate
  hard cap prevents orphaned processes.
- Conservative unattended defaults for each provider.
- Provider binary env overrides.
- Tests for arg builders and postinstall behavior.
- CI with Rust verification and npm publish dry-run.
- Documentation for `agbrowse --help` and `agbrowse web-ai --help` as the
  validation surface for GPT Pro browser workflows.

## cli-jaw Target

cli-jaw should eventually call:

```text
ai-e claude ...
ai-e codex ...
ai-e gemini ...
ai-e grok ...
ai-e copilot ...
```

The migration should add `ai-e` alongside `claude-e` first, then decide whether
`ai-e claude` can replace `claude-e` after behavior parity is proven.
