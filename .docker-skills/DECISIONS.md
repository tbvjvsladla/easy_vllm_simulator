# Docker Claude Skill Package — Architectural Decisions

> Numbered decisions with rationale. Immutable once recorded.

---

## D-001: 7-Phase Development Methodology
**Date:** 2026-03-19
**Decision:** Follow the proven 7-phase research-first development methodology.
**Rationale:** Successfully applied in ERPNext, Blender-Bonsai, and Tauri 2 skill packages. Ensures systematic coverage and quality.
**Phases:** Infrastructure → Research → Masterplan → Topic Research → Skill Development → Validation → Publication

## D-002: ROADMAP.md as Single Source of Truth
**Date:** 2026-03-19
**Decision:** ROADMAP.md is the single source of truth for project status, progress percentages, and next steps.
**Rationale:** Prevents status fragmentation across multiple files. Every session starts by reading ROADMAP.md.

## D-003: English-Only Content
**Date:** 2026-03-19
**Decision:** All skill content, code comments, and documentation within skills MUST be in English.
**Rationale:** Skills are published internationally via OpenAEC Foundation. English ensures maximum accessibility. Project management files (PROMPTS.md) may contain Dutch per user preference.

## D-004: MIT License
**Date:** 2026-03-19
**Decision:** Project is licensed under MIT License, copyright OpenAEC Foundation.
**Rationale:** Consistent with all OpenAEC Foundation skill packages. Maximum permissiveness for adoption.

## D-005: Docker Engine 24+ and Compose v2 Only
**Date:** 2026-03-19
**Decision:** Target Docker Engine 24+ and Docker Compose v2 exclusively. No legacy docker-compose v1 support.
**Rationale:** Engine 24+ is the current stable line with BuildKit as default. Compose v2 is the maintained version integrated into the Docker CLI. Legacy v1 is EOL.

## D-006: SKILL.md Under 500 Lines
**Date:** 2026-03-19
**Decision:** Every SKILL.md file MUST be under 500 lines. Heavy content goes in references/ subdirectory.
**Rationale:** Keeps skills scannable and fast to load. Proven effective in other skill packages.

## D-007: WebFetch for Source Verification
**Date:** 2026-03-19
**Decision:** Use WebFetch tool during research phases to verify documentation is current.
**Rationale:** Docker documentation updates frequently. WebFetch ensures we consult the latest version rather than relying on training data.

## D-008: Split Dockerfile Into 3 Skills
**Date:** 2026-03-19
**Decision:** Split Dockerfile into 3 skills: `dockerfile`, `buildkit`, `multistage`.
**Rationale:** Dockerfile reference alone is 17 instructions. BuildKit adds 5 mount types + heredocs. Multi-stage has enough patterns for its own skill. Each would exceed 500 lines combined.

## D-009: Split Compose Into 2 Skills
**Date:** 2026-03-19
**Decision:** Split Compose into 2 skills: `compose-services`, `compose-resources`.
**Rationale:** Service configuration has 33+ attributes. Networks/volumes/configs/secrets are distinct infrastructure concerns.

## D-010: Split CLI Into 2 Skills
**Date:** 2026-03-19
**Decision:** Split CLI into 2 skills: `cli-containers`, `cli-images`.
**Rationale:** Container lifecycle (run, exec, logs) and image management (build, pull, prune) are distinct workflows with different mental models.

## D-011: Add docker-impl-go-templates Skill
**Date:** 2026-03-19
**Decision:** Added `docker-impl-go-templates` as a dedicated skill.
**Rationale:** Go template syntax for --format flags is a cross-cutting concern used in 10+ commands. Users frequently search for format patterns. Warrants its own skill.

## D-012: Add docker-impl-production Skill
**Date:** 2026-03-19
**Decision:** Added `docker-impl-production` as a dedicated skill.
**Rationale:** Production hardening (base images, non-root, signals, health checks, entrypoints) is a cross-cutting pattern not fitting any single syntax skill.

## D-013: Merge Networking Concepts Into core-networking
**Date:** 2026-03-19
**Decision:** Merged networking concepts into `docker-core-networking`.
**Rationale:** Network drivers, DNS, and isolation are conceptual. Practical troubleshooting goes in `docker-errors-networking`.

## D-014: Batch Order — Core First, Agents Last
**Date:** 2026-03-19
**Decision:** Reordered batches: core first, then syntax, impl, errors, agents last.
**Rationale:** Ensures dependency chain: core -> syntax -> impl -> errors -> agents.

## D-015: Bulk Commit Approach for Phases 2-7
**Date:** 2026-03-19
**Decision:** Phases 2-7 were executed and committed in a single session as bulk commits rather than per-phase commits. This was a pragmatic choice for a single-session build but deviates from the Phase X.Y commit convention.
**Rationale:** Single-session execution made per-phase commits impractical. All work was validated before committing.

## D-016: Vooronderzoek Fragment Architecture
**Date:** 2026-03-19
**Decision:** Phase 2 research uses a fragment architecture (3 domain-specific fragments totaling 5577 lines) instead of a single monolithic vooronderzoek document. The vooronderzoek-docker.md serves as an index to these fragments.
**Rationale:** Domain-specific fragments (Dockerfile, Compose, CLI+Ops) are easier for skill-creation agents to consume than a single large file. Each agent only needs to read the relevant fragment.
