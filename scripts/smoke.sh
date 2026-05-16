#!/usr/bin/env sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CLAUDE_BIN=${CLAUDE_BIN:-$(command -v claude)}
AI_E_MODEL=${AI_E_MODEL:-claude-opus-4-6}
AI_E_PROMPT=${AI_E_PROMPT:-Say hello in one short sentence.}

printf '%s\n' "$AI_E_PROMPT" \
  | cargo run --quiet --manifest-path "$ROOT/Cargo.toml" --bin ai-e -- claude run \
      --jsonl \
      --output-format stream-json \
      --timeout-ms 600000 \
      --claude-bin "$CLAUDE_BIN" \
      -- \
      --model "$AI_E_MODEL" \
      --dangerously-skip-permissions
