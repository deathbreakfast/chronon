#!/usr/bin/env bash
# Reject non-authoritative (local/smoke) hardware and zero-claim drain-only multibench reports.
set -euo pipefail

REPORTS="${1:-profiling/chronon-bench/reports}"
fail=0

for f in "$REPORTS"/*-aws-*.json; do
  [[ -f "$f" ]] || continue
  if grep -qE '"hardware": "(local|dev-wsl)"' "$f"; then
    echo "REJECT: $f has non-authoritative (local) hardware" >&2
    fail=1
  fi
done

if command -v python3 >/dev/null 2>&1; then
  while IFS= read -r -d '' f; do
    python3 - "$f" <<'PY' || fail=1
import json, sys
path = sys.argv[1]
with open(path) as fh:
    rep = json.load(fh)
dims = rep.get("sweep_dimensions") or {}
bc = dims.get("bench_client_count") or 1
idx = dims.get("bench_client_index") or 0
if bc > 1 and idx > 0:
    ops = rep.get("ops") or 0
    if ops <= 0:
        print(f"REJECT: {path} drain-only client {idx} has ops={ops}", file=sys.stderr)
        sys.exit(1)
PY
  done < <(find "$REPORTS" -maxdepth 1 -name '*-aws-*.json' -print0 2>/dev/null)
fi

exit "$fail"
