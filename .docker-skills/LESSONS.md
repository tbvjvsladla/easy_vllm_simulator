# Docker Claude Skill Package — Lessons Learned

> Numbered lessons (L-XXX) discovered during development.

---

## L-001: Phase 4 (Topic Research) Can Be Skipped
**Date:** 2026-03-19
**Context:** Vooronderzoek produced 3 comprehensive research fragments from 52 official documentation pages.
**Lesson:** When Phase 2 research is thorough enough (fetching complete API references, not just overviews), Phase 4 per-skill topic research adds minimal value. The research fragments already contain the detail needed for skill creation.
**Applicable to:** All future skill packages using this methodology.

## L-002: 3-Agent Batches Are Optimal for Skill Creation
**Date:** 2026-03-19
**Context:** Created 22 skills across 8 batches of 2-3 agents each.
**Lesson:** 3 parallel agents with separated file scopes produces consistent quality. Agents complete in 3-5 minutes each. Quality is high when prompts include: exact output directory, YAML frontmatter, scope bullets, research sections to read, and quality rules.
**Applicable to:** All future skill packages.

## L-003: Research Fragment Architecture Works Well
**Date:** 2026-03-19
**Context:** Split Docker research into 3 domain-specific fragments instead of one monolithic vooronderzoek.
**Lesson:** Domain-specific research fragments (Dockerfile, Compose, CLI+Ops) are easier for skill-creation agents to consume than a single large file. Each agent only needs to read the relevant fragment.
**Applicable to:** Future packages with broad technology scope.

## L-004: Line Count Sweet Spot Is 250-400
**Date:** 2026-03-19
**Context:** Skills ranged from 214 to 457 lines. Core-security hit 457 (91% of limit).
**Lesson:** The most useful skills land in the 250-400 line range. Skills under 250 feel thin. Skills approaching 500 should offload more to references. The 500-line limit is effective for forcing concise SKILL.md with detailed references/.
**Applicable to:** Skill structure guidelines.

## L-005: Agent Prompts Need Reference Skill Path
**Date:** 2026-03-19
**Context:** Including a path to a reference skill (Tauri core-architecture SKILL.md) in every agent prompt.
**Lesson:** Agents produce more consistent formatting when given a concrete reference skill to follow. The Tauri architecture skill works well as a universal format reference.
**Applicable to:** Masterplan template, agent prompt design.

## L-006: YAML Frontmatter Descriptions Need Folded Block Scalar Format
**Date:** 2026-03-19
**Context:** Audit of all 22 skills revealed YAML frontmatter descriptions used quoted strings and lacked trigger-optimized descriptions.
**Lesson:** YAML frontmatter descriptions must use folded block scalar (`>`) format with 'Use when...' triggers, anti-pattern warnings, and Keywords. The initial 22 skills all used quoted strings and lacked trigger-optimized descriptions, requiring a full migration during audit.
**Applicable to:** All skill packages, WAY_OF_WORK.md skill standards.

## L-007: Per-Phase Commits Are Important for Audit Traceability
**Date:** 2026-03-19
**Context:** Phases 2-7 were committed in bulk rather than per-phase.
**Lesson:** Per-phase commits are important for audit traceability. Bulk-committing multiple phases makes it impossible to verify phase boundaries in git history.
**Applicable to:** All future skill packages, workflow template.
