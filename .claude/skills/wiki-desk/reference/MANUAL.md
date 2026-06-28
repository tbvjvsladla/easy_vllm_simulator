# Manual

## 1. Install

This skill lives at `.claude/skills/wiki-desk/` (tracked building-block, main node only). The live
library `__llm-wiki/` is generated at the project root and is **untracked** (`.gitignore /__llm-wiki/`).
It is never propagated to sub-nodes (it is structurally excluded from `sync_to_sub.sh`).

## 2. Initialize (once)

```bash
python3 .claude/skills/wiki-desk/scripts/init_wiki_desk.py \
  --project-root . --wiki-root __llm-wiki \
  --answers .claude/skills/wiki-desk/fixtures/project_init_answers.yaml
```

Required init decisions (see `INIT_GUIDE.md`): project identity, document roots, authority order,
registry rules, edge types, retrieval policy. Auto-scan suggests; the operator decides.

## 3. Warm-start (ongoing)

After init, shelve new/changed docs incrementally on librarian invocation:

```bash
python3 .claude/skills/wiki-desk/scripts/init_wiki_desk.py \
  --project-root . --wiki-root __llm-wiki \
  --answers .claude/skills/wiki-desk/fixtures/project_init_answers.yaml --incremental
```

Incremental preserves `indexed_at` for unchanged sources, re-extracts the edge graph from current
bodies, and marks removed sources `archived` (path reference preserved, never deleted). A per-thread
durable packet is written to `__llm-wiki/warm-start/<thread>.md`.

## 4. Query

```bash
python3 .claude/skills/wiki-desk/scripts/smoke_query.py --wiki-root __llm-wiki \
  --query "<question or a doc stem like devlog_2026062412_1>" [--report reports/smoke_report.generated.md]
```

A passing response returns referenced source paths, document types, authority ranks, confidence,
concise summaries, and traversed edges. Direct stem lookup returns the named node + its edges; a
topical query returns authority-ranked matches; a no-evidence query returns the negative verdict
(exit 2) and **no path**.

## 5. Lint (graph integrity)

```bash
python3 .claude/skills/wiki-desk/scripts/lint_wiki.py --wiki-root __llm-wiki --project-root . [--fix]
```

Rules: dead-path-ref (critical), stale-sha256 (warning → run `--incremental`), authority/confidence
range (warning), orphan (suggestion), edge-integrity (critical). Lint is hard-bound to `__llm-wiki/`:
it may READ (stat/hash) referenced project files, but only ever WRITES inside `__llm-wiki/`. `--fix`
regenerates derived caches (e.g. `concepts/_index.md`); it never moves or edits project docs.

## 6. Safety scans (release gate)

```bash
python3 .claude/skills/wiki-desk/scripts/scan_forbidden_strings.py .claude/skills/wiki-desk
python3 .claude/skills/wiki-desk/scripts/scan_raw_copy.py __llm-wiki
```

`scan_forbidden_strings` guards against environment PII (home path, account handle, node IPs,
hostnames, NAS paths) baking into the tracked skill — extend `--term` per deployment. `scan_raw_copy`
fails on a `raw/` directory or oversized wiki files that look like copied source bodies.

## 7. Release gate (done)

Done = init bootstraps the library + query smoke (direct + topical + negative) passes + both scans
pass + lint reports 0 critical. Commit only the tracked skill; the live `__llm-wiki/` stays untracked.
Record evidence in `docs/testlog/`. Use `FIELD_REPORT_TEMPLATE.md` after a real adaptation to plan v1.1+.
