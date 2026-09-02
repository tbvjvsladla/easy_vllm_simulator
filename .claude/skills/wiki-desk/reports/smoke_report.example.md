# Wiki-desk Smoke Report (example)

> Required sections of a passing smoke. Regenerate the live one with
> `smoke_query.py --report reports/smoke_report.generated.md`.

## Init

- sources: 70 · edges: 151 (cites/realizes/evidences/same-thread) · raw_copy: false

## A. Direct lookup (path-reference accuracy + edge)

- Query: `devlog_26062412`
- Returns: `docs/devlog/devlog_26062412_*.md` (authority 100) → **realizes** `plan_26062411`
  + ← **evidences** `testlog_26062412`. PASS.

## B. Topical traversal

- Query: `0.23.0 소스빌드 검증 증거`
- Returns: ≥2 testlogs (authority 85), each with evidences→devlog / realizes→plan / simlog
  brace-glob `{1,2,3}` expanded to 3 runs. PASS.

## C. Warm-start incremental

- New stub devlog citing a plan → `--incremental` → indexed (delta added=1), `realizes` edge
  auto-extracted; on removal → marked `archived`. PASS.

## Negative honesty

- Query with no matching high-authority evidence → `INSUFFICIENT EVIDENCE`, **no path**, exit 2. PASS.

## Safety

- scan_forbidden_strings (tracked skill): PASS 0 hits.
- scan_raw_copy (live `__llm-wiki`): PASS.
- lint: 0 critical, 0 warning (suggestions allowed).
