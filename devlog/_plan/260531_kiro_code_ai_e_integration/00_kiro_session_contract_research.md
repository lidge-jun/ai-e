---
created: 2026-05-31
status: research
tags: [ai-e, kiro-code, kiro-cli, session, resume, cli-jaw]
---

# 00 — Kiro CLI Session Contract Research

## Goal

Integrate `kiro-code` into `ai-e` (like `claude-e` → `ai-e claude`) with correct
session id capture/resume, then wire `cli-jaw` `ai-e` provider routing + symlink
dist so only `ai-e` deploy is needed.

## Empirical Evidence (Jun, 2026-05-31)

Interactive TUI (`kiro-cli chat hi`):

```text
Session ended.
Resume with: kiro-cli --resume-id 79eee8a5-7c00-4cd9-8385-c534a2f8b814
```

Official flags (`kiro-cli chat --help`):

| Flag | Purpose |
|------|---------|
| `--resume-id <SESSION_ID>` | Resume specific conversation |
| `-r, --resume` | Resume most recent for cwd |
| `--no-interactive` | Non-TUI batch mode (cli-jaw / ai-e path) |
| `-a, --trust-all-tools` | Auto-approve tools |
| `--model <MODEL>` | Model selection |

**Key insight:** Resume surface is `--resume-id`, not `--resume <id>` (Claude-style).

## Headless vs TUI Session Surfaces

| Mode | Session id source | Notes |
|------|-------------------|-------|
| TUI (`chat` default) | stdout footer `Resume with: kiro-cli --resume-id …` | May also print `Session ID: …` |
| Headless (`chat --no-interactive`) | **No reliable stdout footer** | Persists to sqlite v2 store |

### Sqlite v2 store (authoritative for headless)

Path (macOS): `~/Library/Application Support/kiro-cli/data.sqlite3`

Table: `conversations_v2`

- `key` = canonical cwd (realpath)
- `conversation_id` = session uuid
- `updated_at` = epoch ms

Legacy `~/.kiro/sessions/cli/*.json` is **not** used by `--no-interactive`.

cli-jaw WIP (uncommitted) already implements Node-side capture:

- `listKiroConversationIdsForCwd()` — snapshot before spawn
- `resolveKiroSessionIdAfterSpawn()` — set-diff after fresh spawn
- `extractKiroSessionIdFromV2Store()` — fallback by `updated_at`
- `parseKiroSessionIdFromStdout()` — TUI `Session ID:` line

## Current Architecture Gap

| Layer | kiro today | target |
|-------|------------|--------|
| ai-e | **No kiro provider** | `ai-e kiro p …` headless PTY adapter |
| cli-jaw | Top-level `kiro-code` → direct `kiro-cli` | `ai-e` provider `kiro` (keep `kiro-code` alias) |
| session persist | Works for top-level kiro WIP | Must work for `ai-e` + provider `kiro` |
| `shouldPersistMainSession` | N/A | Currently **blocks** all non-claude ai-e providers |

## ai-e Integration Pattern (reference)

Existing headless PTY providers: `codex`, `gemini`, `grok`, `copilot`.

Each adds to:

1. `src/providers/mod.rs` — `ProviderKind::Kiro`, parse alias `kiro|kiro-code`
2. `src/headless.rs` — `build_kiro_args()`:
   ```text
   kiro-cli chat --no-interactive [--trust-all-tools] [--model M]
                 [--resume-id ID]  # map ai-e --resume → --resume-id
                 <prompt>
   ```
3. `src/lib.rs` — route `ProviderKind::Kiro` through `headless::run_provider`
4. Tests — fake binary smoke (no live model)
5. `structure/` docs — provider map + cli-jaw migration row

### Session id in ai-e (design choice)

Option A — **Rust sqlite reader** in ai-e (parity with cli-jaw, self-contained)

Option B — **stdout parse only** + delegate sqlite to cli-jaw (insufficient for headless)

Option C — **Emit `[ai-e] session:` footer** after run by reading sqlite in ai-e (recommended)

Recommended: **Option C** — ai-e reads sqlite post-run, emits:

```text
[ai-e] session: <uuid>
[ai-e] resume: ai-e kiro --resume <uuid> "next prompt"
```

cli-jaw already parses `[ai-e] session:` / jaw_runtime for claude path; extend for kiro.

## cli-jaw Integration (phase 2)

Files (planned):

| File | Change |
|------|--------|
| `src/agent/args.ts` | `ai-e` + provider `kiro` spawn/resume args |
| `src/types/cli-engine.ts` | provider enum |
| `src/cli/registry.ts` | ai-e kiro models/status |
| `src/agent/spawn.ts` | route kiro stdout via ai-e or keep plain-text parser |
| `src/agent/session-persistence.ts` | enable persist for `ai-e` + `kiro` |
| `resolveAiEProvider()` | map `kiro-code` model → `kiro` provider |

WIP session capture (5 files, uncommitted in cli-jaw) merges into phase 2 or
commits separately if kept for direct `kiro-code` fallback.

## Symlink / Deploy Contract

Per Jun:

- Commit all changes in **ai-e** repo
- Symlink built `ai-e` binary into cli-jaw `node_modules/@bitkyc08/ai-e` dist
- cli-jaw picks up via existing detection (`../ai-e/target/release/ai-e`)
- **Deploy ai-e only** — Jun runs deploy when asked

## Smoke Matrix

| # | Command | Expect |
|---|---------|--------|
| 1 | `ai-e kiro p "say kiro-smoke-ok"` | exit 0, response contains smoke token |
| 2 | capture session id from footer or sqlite | non-empty uuid |
| 3 | `ai-e kiro p --resume <id> "say resumed"` | exit 0, same session |
| 4 | cli-jaw dispatch with `ai-e` + provider kiro | session persisted in jaw.db |
| 5 | fake-binary unit tests in ai-e | CI green, no network |

## Phase Plan (jawdev)

| Phase | Scope | Employee verify |
|-------|-------|-----------------|
| 10 | ai-e kiro provider + session footer + tests | Backend audit |
| 20 | cli-jaw ai-e routing + persistence + WIP merge | Backend verify |
| 30 | smoke (live kiro-cli) + symlink dist | Testing verify |
| 40 | docs/structure + commit ai-e, deploy handoff | Docs verify |

## Open Questions (resolved by Jun's autonomy grant)

1. **Keep top-level `kiro-code`?** Yes — fallback until ai-e kiro stable; primary path becomes ai-e.
2. **Deploy now?** No — commit + symlink; Jun deploys ai-e on request.
3. **Session capture owner?** ai-e Rust sqlite reader + cli-jaw fallback for direct kiro-code.
