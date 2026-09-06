---
name: wiki-desk
description: >-
  Path-reference librarian over the project's docs/ work-history (devlog, testlog, plan,
  simlog) and the sub-doc mirror. Builds a deterministic relationship graph
  (cites/realizes/evidences edges) and surfaces authority-ranked, edge-traversable context
  WITHOUT copying source bodies. Use when starting an Ouroboros interview, writing a plan,
  devising a model serving strategy, bumping a vLLM version, or adapting topology — whenever
  grounded prior context or faster Seed convergence helps; also when shelving new docs into
  the library or asking "what does this project already know about X". Also triggers on
  Korean instructions like "위키 조회", "이전에 뭐 했는지/뭐였지", "근거 문서 찾아줘",
  "관련 plan/devlog 찾아줘", "문서 등록/갱신/입고". Run init once to bootstrap __llm-wiki,
  then warm-start incrementally on each invocation.
---

# wiki-desk — path-reference librarian (self-improvement loop)

`wiki-desk` is the **librarian** for a project-local `__llm-wiki/`: a metadata-first library of
**path references + sha256 + roles + authority ranks + deterministic relationship edges +
concise summaries**. It is **not** a raw source archive — original docs stay in place; the
library only points at them. This closes the self-improvement loop: **relationship building →
refined context surfacing → faster Ouroboros interview/Seed convergence** for version/topology/model
adaptation.

> Building-block, main-only (one central librarian; sub-node knowledge is recovered via the
> doc-mirror, not a second library). The live `__llm-wiki/` is untracked; this skill is tracked.

## Contract

- **Goal** — index the project's own work-history as path references + a deterministic edge graph, and surface authority-ranked prior evidence without copying source bodies.
- **When to invoke** — as a **sidecar** at three loop points only: work-start **discovery**, a new **failure**, and evidence **publication** (shelving). Never as a step of the serving chain.
- **Inputs** — `docs/{devlog,testlog,plan,simlog}` + `sync_staging/sub_docs` + `seed` (graceful skip if absent) · a question or a doc stem.
- **Outputs** — `__llm-wiki/` registry + edge graph (untracked) · authority-ranked answers with source paths · lint/scan reports.
- **Mandatory procedural spine** — see §Mandatory procedural spine below (orientation before answering).
- **State transitions** — advances **none**. The librarian informs `execution-approved` planning and `evidence-complete` shelving, but never judges a state; `.claude/policies/runtime/completion_gate.py` owns every state verdict.
- **HITL/safety boundaries** — no raw-body copies · no constitution/`.claude/` indexing (anti-confirmation-bias) · no web research · no cron/background ingestion · **negative honesty**: answer "no evidence" rather than fabricate a path.
- **Failure → reference routing** — see §Failure → reference routing below.
- **Deterministic commands** — `scripts/init_wiki_desk.py`(init/`--incremental`) · `scripts/smoke_query.py` · `scripts/lint_wiki.py` · `scripts/scan_raw_copy.py`.
- **Handoff contract** — **sidecar / non-owner**: it *cites* what other skills own. Benchmark report, certificate and verdict are owned by `adversarial-benchmark`; manifest/Flag by `terraforming_node`; image identity by `upstream-version-watch`; serving triplet by `vllm-recipe-explorer`. wiki-desk shelves and points at them, and never (re)publishes or judges them.
- **Owns (state)** — `wiki-index` · `doc-edge-graph`

## Mandatory procedural spine

1. **Orient** — read `__llm-wiki/index.md` → `SCHEMA.md` → `sources/source-registry.md` → `relationships/edge-graph.md` → `concepts/document-universe-hierarchy-map.md` (in that order; scripts read `sources/registry.json`).
2. **Warm-start** — run the incremental shelving pass so the registry matches disk before answering.
3. **Query** — resolve intent (completion vs intent) → search metadata + edges first, never raw bodies → rank by intent → authority → relevance.
4. **Traverse** — follow `realizes`/`evidences`/`same-thread` edges to surface supporting plan/testlog/simlog; open a body only when the summary is insufficient.
5. **Answer or refuse** — if nothing clears the authority/score gate, return the negative verdict; never fabricate a path.
6. **Shelve** — after new docs are published, re-run the incremental pass and lint the graph.

## Failure → reference routing

| 실패 신호 | 라우팅 대상 (정확 경로) |
|---|---|
| Dead path-ref · stale sha256 · orphan node · authority out of range | `.claude/skills/wiki-desk/scripts/lint_wiki.py` |
| Query returns nothing / library looks unbuilt or drifted | `.claude/skills/wiki-desk/reference/MANUAL.md` |
| Target environment has no document system yet (init interview) | `.claude/skills/wiki-desk/reference/INIT_GUIDE.md` |
| Raw source bodies suspected inside the library | `.claude/skills/wiki-desk/scripts/scan_raw_copy.py` |
| Reporting a field observation about the librarian itself | `.claude/skills/wiki-desk/reference/FIELD_REPORT_TEMPLATE.md` |

## When to use (invocation timing)

Invoke the librarian — read its metadata first, then act — when:

- **starting an Ouroboros interview / writing a `docs/plan/`** → warm-start the relevant thread,
  surface prior devlog/testlog evidence so the Seed converges faster;
- **devising a model serving strategy / bumping a vLLM version / changing topology** → query the
  library for what already worked (and what failed) on that model/version;
- **a new doc is written** (plan/devlog/testlog/simlog/benchmark) → shelve it (warm-start incremental);
- **asked "what do we know about X / where is the evidence for Y"** → topical query with edges.
- **a sub node asked for grounding** (`library_request[]` in its task report) → resolve it and hand
  back an export. **This is the librarian's job, and until 2026-09-04 this file never said so** —
  the exchange protocol lived only in `terraforming_node`, so a session that loaded *this* skill
  had no idea it was the export party (audit finding). See §Serving a sub node's request below.

## Serving a sub node's request (main-only · 2026-09-04)

The sub cannot read the library — that asymmetry is deliberate (the library is **not replicated**;
what crosses is a *reference plus excerpt*, never a copy). So the sub sends a request and you answer.

- **Where the request arrives**: the sub's task report carries `library_request[]`; the relay
  surfaces it into `campaigns/<camp-id>/relay/pending_hitl.json`. The three-message contract (`request` → `export` →
  `attestation`) and its judge live in `terraforming_node` (`SKILL.md` §2.7.8 ·
  `scripts/library_exchange.py`); the mechanics are `scripts/library_relay.py`, which calls this
  skill's `smoke_query.py` to resolve terms.
- **★ Priority is not first-come**: a request carrying `blocking: true` means the sub has *stopped*
  and will not decide without it. Serve those **before** everything else — including before your
  own in-flight query work. Non-blocking requests wait. (Before 2026-09-04 nothing read that field;
  ordering was directory-name sort, i.e. effectively arbitrary.)
- **Excerpt budget is owned by the judge** (`library_exchange.EXCERPT_MAX_CHARS`). Do not re-declare
  it here or in the relay — it was declared in two places once, they diverged, and the gate then
  rejected exports that followed the rule.
- **An unanswered blocking request is a stalled node.** The sub is behaving correctly by waiting:
  its honest deferral is the designed outcome of insufficient grounding, not a failure to route
  around. What is missing in that state is *evidence*, and supplying it is this desk's work.

## Operating model

- **`__llm-wiki/` lives at the project root, untracked.** It indexes `docs/{devlog,testlog,plan,simlog}`
  + `sync_staging/sub_docs` + `seed` (graceful skip if absent). The **project ROOT constitution
  (`/CLAUDE.md`, `/.claude/`) is deliberately NOT indexed** (exclusion is by-root — those paths are
  simply not document_roots; vendored subtrees under `seed/` are excluded too) — the library is a
  neutral evidence base that can surface patterns the constitution has not yet codified
  (anti-confirmation-bias); the constitution is the human-authored *output* of reviewing the logs, not an input.
- **Authority = execution-truth > plan-intent**: `devlog:100 > testlog:85 > sub-doc:70 > plan:55 >
  simlog:40 > seed:25`. A devlog of what actually ran outranks the plan that was only intended.
- **Cycle**: `init` (once, adaptive bootstrap) → loop (new plan/serving/version) → `warm-start`
  (incremental shelving on invocation). Detection is inline (no cron). The script's contract is the
  fixed `document_roots`: a doc under those roots auto-shelves (deterministic); a doc OUTSIDE them is
  not emitted by the script — proposing a contract amendment for it is an **agent/HITL judgment**, not
  a script behavior.

## Mandatory orientation

Before answering a project question or updating the library, read **in this order**:

1. `__llm-wiki/index.md` (entrypoint) 2. `__llm-wiki/SCHEMA.md` 3. `__llm-wiki/sources/source-registry.md`
4. `__llm-wiki/relationships/edge-graph.md` 5. `__llm-wiki/concepts/document-universe-hierarchy-map.md`.

Scripts read the machine source of truth `__llm-wiki/sources/registry.json`.

## Commands (deterministic scripts — `scripts/`)

```bash
ROOT=.; WIKI=__llm-wiki; ANS=.claude/skills/wiki-desk/fixtures/project_init_answers.yaml
# init (once) — adaptive bootstrap of the path-reference library
python3 .claude/skills/wiki-desk/scripts/init_wiki_desk.py --project-root $ROOT --wiki-root $WIKI --answers $ANS
# warm-start (ongoing) — incremental shelving: re-extract changed/new, archive removed
python3 .claude/skills/wiki-desk/scripts/init_wiki_desk.py --project-root $ROOT --wiki-root $WIKI --answers $ANS --incremental
# query — authority-ranked, edge-traversable retrieval (direct stem lookup OR topical)
python3 .claude/skills/wiki-desk/scripts/smoke_query.py --wiki-root $WIKI --query "<question or doc stem>"
# lint — graph integrity (dead path-ref / stale sha256 / authority range / orphan); --fix regenerates caches
python3 .claude/skills/wiki-desk/scripts/lint_wiki.py --wiki-root $WIKI --project-root $ROOT [--fix]
# release-gate scans
python3 .claude/skills/wiki-desk/scripts/scan_forbidden_strings.py .claude/skills/wiki-desk   # PII leak guard (tracked skill)
python3 .claude/skills/wiki-desk/scripts/scan_raw_copy.py $WIKI                                 # no raw copies
```

If the target environment has **no document system yet**, init is interview-led: settle
`document_roots` / `authority_order` / retrieval with the user first (see `reference/INIT_GUIDE.md`).

## Query loop

1. Resolve intent: completion ("did it work") vs intent ("what approach"). 2. Search registry
metadata + edges first, never raw bodies. 3. Rank by intent → authority → relevance. 4. Traverse
`realizes`/`evidences`/`same-thread` edges to surface supporting plan/testlog/simlog. 5. Open a
source body only when the summary is insufficient. **Negative honesty**: if nothing clears the
authority/score gate, answer "no evidence" — never fabricate a path.

## Relationship edges (deterministic, grep-grounded — no inference)

`cites` (explicit cross-ref) · `realizes` (devlog/testlog → plan) · `evidences` (simlog/testlog →
the higher tier it supports; the docs chain simlog→testlog→devlog) · `same-thread` (identical
topic slug) · `superseded-by` (retroactive header banner). (A `part-of` type existed until
2026-09-03 but was **unreachable**: dir-node mode registers the simlog run directory itself and
never its member files, so nothing could ever be nested under a registered dir-node.) Citations are resolved
by stem with prefix-glob, brace-glob `{1,2,3}`, and `_*` wildcard expansion. (Semantic
`contradicts`/`supersedes` edges are intentionally **not** v1 — they need judgment; parked.)

## Verification (release gate — done = all pass on real data)

1. **init** bootstraps the live library; 2. **query smoke** — direct lookup returns a node + its
`realizes`→plan edge; topical returns ≥2 authority-ranked sources with edge traversal; a no-evidence
query returns the negative verdict (exit 2); 3. **scan_forbidden_strings** + **scan_raw_copy** pass;
4. **lint** reports 0 critical. Commit only the **tracked skill** (the live `__llm-wiki/` is untracked).

## Provenance

Derived from the operator's own `wiki-desk-operating-pattern` v1.0.0-candidate and the open-source
`llm-wiki` (nvk) — adopting its `_index`-first navigation, lint, lightweight confidence, dual-link,
and `concepts/` synthesis-node ideas while keeping strict path-reference (no raw-copy / web-research /
multi-runtime packaging). MIT. Constitution-adapted for `easy_vllm_simulator`.

## Non-goals

No raw `__llm-wiki/raw/` copies · no sub-node library instance · no constitution/`.claude/` indexing ·
no cron/background ingestion · no web research · no semantic-judgment edges (v1) · no multi-runtime packaging.

## Deeper references (read on demand)

- `reference/references.md` — 외부 ID→URL 정규화, HW scope 표, 호환성/성능 공식과 부정판정 최소범위 레시피.
- `reference/INIT_GUIDE.md` — adaptive-init interview contract (slots, edge slot, authority design).
- `reference/MANUAL.md` — install, init, warm-start, query, lint, scans, field report.
- `templates/__llm-wiki/SCHEMA.md` — the metadata + edge schema (copied into the live library).
