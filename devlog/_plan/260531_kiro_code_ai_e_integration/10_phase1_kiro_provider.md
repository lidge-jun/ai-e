---
created: 2026-05-31
status: implemented
tags: [ai-e, kiro, phase1]
---

# 10 — ai-e Kiro Provider (Phase 1)

## Scope

Add `kiro` / `kiro-code` as a headless PTY provider in ai-e with:

- `kiro-cli chat --no-interactive` arg builder
- `--resume` → `--resume-id` mapping (ai-e surface compatibility)
- Session footer via stdout parse + sqlite v2 store diff
- Unit tests (no live model)

## Files

| File | Change |
|------|--------|
| `Cargo.toml` | `rusqlite` bundled for session store reads |
| `src/providers/mod.rs` | `ProviderKind::Kiro` |
| `src/providers/kiro_session.rs` | **NEW** sqlite + stdout session capture |
| `src/headless.rs` | `build_kiro_args`, capture + footer emission |
| `src/lib.rs` | help text + run_provider match arm |

## Verification

```bash
cd /Users/jun/Developer/new/700_projects/ai-e
cargo test          # 81 passed
cargo build --release
```

## Next (Phase 20)

cli-jaw `ai-e` provider routing for kiro + session persistence enablement.
