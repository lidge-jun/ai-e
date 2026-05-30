# Phase 30 — Live smoke + hang fixes

## Problem

Live `ai-e kiro p` hung >11 min under cli-jaw-like conditions (piped stdout/stderr, non-TTY stdin).

Root causes:

1. **PTY wrapper** — `kiro-cli chat --no-interactive` detects TTY and stalls. Fixed by pipe spawn for `ProviderKind::Kiro` only.
2. **Child stdin inherit** — kiro waited on open inherited stdin. Fixed with `Stdio::null()` on child spawn.
3. **`read_stdin_if_piped` blocking** — non-TTY stdin from orchestrators (Cursor/jaw) is an open pipe with no EOF; `read_to_string()` blocked forever. Fixed with non-blocking `O_NONBLOCK` drain (read until `WouldBlock`, never wait for EOF).

## Changes (ai-e)

| File | Change |
|------|--------|
| `src/headless.rs` | `spawn_pipe_with_timeout()` for Kiro; child stdin null |
| `src/print_mode.rs` | Non-blocking stdin read for piped orchestrator stdin |
| `structure/provider_adapter.md` | Document Kiro as pipe-backed, not PTY |

## Smoke matrix (2026-05-31)

| Case | Command | Result |
|------|---------|--------|
| Fresh | `ai-e kiro p --cwd /tmp/... --model auto "Reply exactly: KIRO_AI_E_SMOKE_OK2" \| tee out` | exit 0, ~5s |
| Session footer | stderr | `[ai-e] session: 5a793bc5-07b9-48fe-adb1-d5601b3cf6c3` |
| Resume | `--resume 5a793bc5-... "Reply exactly: KIRO_AI_E_RESUME_OK"` | exit 0, same session id |
| Open stdin pipe | `( sleep 60 ) \| ai-e kiro p ...` | exit 0, no hang |
| Unit tests | `cargo test` | 81 pass |

## Local cli-jaw wiring

```bash
ln -sfn /Users/jun/Developer/new/700_projects/ai-e \
  /Users/jun/Developer/new/700_projects/cli-jaw/node_modules/@bitkyc08/ai-e
cargo build --release -C /Users/jun/Developer/new/700_projects/ai-e
```

Deploy: publish **ai-e only** (`@bitkyc08/ai-e`); cli-jaw picks up via npm dep or symlink.

## Verification

Backend employee audit after commit.
