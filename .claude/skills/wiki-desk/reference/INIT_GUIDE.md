# Adaptive Init Guide

Init fits `__llm-wiki` to a project through an interview, then deterministically indexes the
chosen roots. **Auto-scan proposes evidence; the user's decisions are the contract.** If the
project already has a mature document system (this one does — `docs/` 4-type convention), init
just indexes it from `fixtures/project_init_answers.yaml`; otherwise lead the interview below first.

## Interview slots

1. **Project identity** — name; what the library serves (here: closing the self-improvement loop).
2. **Document roots** — which folders hold human-authored work history. Be SPECIFIC per-folder so
   `simlog` can be handled as run-directory nodes and so the constitution / `.claude/` / `output/`
   are never pulled in.
   - This project: `docs/devlog`, `docs/testlog`, `docs/plan`, `docs/simlog`, `sync_staging/sub_docs`, `seed`.
3. **Authority order** — the precedence weight per document type. This project encodes
   **execution-truth > plan-intent**:
   `devlog:100 > testlog:85 > sub-doc:70 > plan:55 > simlog:40 > seed:25`.
4. **Registry rules** — `include_extensions`, `exclude_dirs` (add `output`, `__llm-wiki`, the wiki
   source trees), `exclude_basenames` (`example.md` skeleton), `dir_node_roots` (`docs/simlog`).
   (A `dir_node_anchor` key was documented until 2026-09-03; no code ever read it — a dir-node is
   hashed over its whole sorted file set, so no in-directory anchor file is needed.)
5. **Relationship/edge slot** — confirm the deterministic edge types extractable from the project's
   cross-reference convention: `cites`, `realizes`, `evidences`, `same-thread`, `superseded-by`. (Semantic
   `contradicts`/`supersedes` are parked — they need judgment.)
6. **Retrieval policy** — `completion_first` (did it work) vs `intent_first` (what approach), plus the
   negative-honesty gates `min_authority` / `min_score`.

## Constitution exclusion (non-negotiable)

`CLAUDE.md` and `.claude/` are **never** indexed. The library is a neutral evidence base; the
constitution is the human-authored output of reviewing the logs (anti-confirmation-bias). It is also
always loaded each session, so indexing it would be redundant.

## Output contract (all under `__llm-wiki/`, untracked)

`config.yaml` · `SCHEMA.md` · `index.md` · `log.md` · `sources/source-registry.md` +
`sources/registry.json` · `relationships/edge-graph.md` · `concepts/_index.md` +
`concepts/document-universe-hierarchy-map.md` · `warm-start/<thread>.md`.
**No raw source copy is created.**
