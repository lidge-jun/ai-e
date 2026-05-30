---
created: 2026-05-31
status: implemented
tags: [cli-jaw, ai-e, kiro, phase2]
---

# 20 — cli-jaw ai-e Kiro Routing (Phase 2)

## Scope

Wire cli-jaw `ai-e` selector to the new ai-e `kiro` provider with session
persistence and plain-text stdout handling shared with top-level `kiro-code`.

## Changes (cli-jaw)

| Area | Detail |
|------|--------|
| `args.ts` | `kiro` in `AI_E_PROVIDERS`; `buildAiEKiroArgs`; resume via `--resume` |
| `spawn.ts` | `isKiroPlainTextCli(cli, provider)`; stderr `[ai-e] session:` capture |
| `session-persistence.ts` | persist for `ai-e` + `kiro` |
| `registry.ts` / UI constants | provider + models |
| `settings.ts` | quota delegation `kiro` |

## Local symlink (dev)

```bash
ln -sfn /Users/jun/Developer/new/700_projects/ai-e \
  /Users/jun/Developer/new/700_projects/cli-jaw/node_modules/@bitkyc08/ai-e
cargo build --release -C /Users/jun/Developer/new/700_projects/ai-e
```

## Verification

```bash
cd /Users/jun/Developer/new/700_projects/cli-jaw
npx tsc --noEmit
npm test -- tests/unit/agent-args.test.ts tests/unit/kiro-runtime.test.ts tests/unit/cli-registry.test.ts
```

## Next (Phase 30)

Live `kiro-cli` smoke + end-to-end resume via `ai-e kiro`.
