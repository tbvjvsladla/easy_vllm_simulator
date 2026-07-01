# __llm-wiki Schema (path-reference)

- **Mode**: path-reference. **Raw copy**: disabled. The library stores metadata,
  path references, deterministic relationship edges, and concise summaries —
  never raw source bodies.

## Files

```text
__llm-wiki/
├── SCHEMA.md
├── config.yaml
├── index.md                                   # orientation entrypoint (read FIRST)
├── log.md                                     # append-only (re)index log
├── sources/
│   ├── source-registry.md                     # human table (authority-ranked)
│   └── registry.json                          # machine source of truth (scripts read this)
├── relationships/
│   └── edge-graph.md                          # typed, grep-grounded edges
├── concepts/
│   ├── _index.md                              # derived cache of synthesis nodes
│   └── document-universe-hierarchy-map.md     # authority + query routing
└── warm-start/
    └── <thread>.md                            # durable per-thread context packets
```

## Source metadata (per entry, in registry.json)

```yaml
source_id:       string            # rel path with /_. → -
workspace:       string
source_path:     string            # project-relative path reference (NEVER a copy)
document_type:   devlog|testlog|sub-doc|plan|simlog|seed|reference
role:            string
authority_rank:  number            # from authority_order[document_type]
copy_policy:     path_reference
sha256:          hex               # of the referenced file (dir-node: of the anchor)
bytes:           number
lines:           number
title_hint:      string
topical_summary: string
stem:            string|null       # <type>_<YYYYMMDDHH>_<seq>  (simlog: <YYYYMMDDHH>_<seq>)
topic_slug:      string|null
confidence:      high|medium|low   # lightweight: edge connectivity
lifecycle_state: active|archived
indexed_at:      ISO-8601
out_edges:       [ {to, edge_type, evidence} ]
```

## Relationship edges (deterministic, in registry.json + edge-graph.md)

```yaml
from:       source_id
to:         source_id
edge_type:  cites | realizes | evidences | part-of | same-thread | superseded-by (문서 헤더의 소급 배너 literal grep — docs.md §3 · 판정 반전 표시, 여전히 결정론)
evidence:   string   # the literal token/path that grounds this edge (no inference)
```

- `realizes`  — a devlog/testlog that cites a plan → work realizes plan.
- `evidences` — a simlog/testlog cited by a higher tier → evidence supports it
  (the docs chain: simlog → testlog → devlog).
- `cites`     — generic explicit cross-reference.
- `part-of`   — a file nested under a directory-node (e.g. a simlog run).
- `same-thread` — identical topic slug (best-effort, secondary).

## Rules

- Store metadata + path references + edges, **not** raw bodies.
- Authority before recency for completion ("did it work") questions.
- Configured `intent_first` roots before low-rank notes for intent ("what approach") questions.
- **Negative honesty**: if nothing clears the authority/score gate, answer "no
  evidence" — never fabricate a path.
- The project ROOT **constitution** (`/CLAUDE.md`, `/.claude/`) is **not indexed**
  (by-root; vendored subtrees under `seed/` are excluded too) — the library is a
  neutral evidence base (anti-confirmation-bias); the constitution is the
  human-authored *output* of reviewing the logs, not an input.
- `simlog` is indexed at run-DIRECTORY granularity (run-summary level), not per trial log.
