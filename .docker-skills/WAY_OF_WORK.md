# Docker Claude Skill Package — Way of Work

> 7-phase methodology, skill structure, content standards, and naming conventions.

---

## 7-Phase Methodology

### Phase 1: Infrastructure & Planning
- Initialize repository with core files
- Create directory structure
- Establish protocols and standards

### Phase 2: Research (Vooronderzoek)
- Broad research across all Docker domains
- Identify skill candidates and coverage gaps
- Document findings in `docs/research/vooronderzoek-docker.md`
- Verify sources against SOURCES.md approved URLs
- Pause point: evaluate if deeper research is needed before proceeding

### Phase 3: Masterplan & Skill Design
- Create complete skill inventory with descriptions
- Map dependencies between skills
- Plan development batches (3 skills per batch)
- Output: `docs/masterplan/docker-masterplan.md`

### Phase 4: Topic Research
- Deep-dive research per skill topic
- Collect code examples, anti-patterns, edge cases
- Output: `docs/research/topic-research/{skill-name}-research.md`
- Pause point: is the research sufficient for high-quality skill creation?

### Phase 5: Skill Development
- Create skills in batches of 3 (per P-002 batch strategy)
- Each skill follows the Skill Structure below
- Validate each batch before proceeding (P-003)

### Phase 6: Validation & Quality
- Full validation pass on all skills (P-003 checklist)
- Cross-reference verification
- Source traceability audit

### Phase 7: Publication
- GitHub repository setup (P-009)
- README finalization
- Social preview and tags
- Final CHANGELOG update

---

## Skill Structure

### Directory Layout
```
skills/source/docker-{category}/{skill-name}/
├── SKILL.md              # Main skill file (< 500 lines)
└── references/           # Supplementary content
    ├── examples.md       # Extended code examples
    ├── anti-patterns.md  # What NOT to do
    └── {topic}.md        # Additional reference material
```

### SKILL.md Template
```markdown
---
name: docker-{category}-{topic}
description: |
  Trigger words and description. When the user asks about {topic},
  Docker {specific area}, or {related concepts}, use this skill.
---

# Docker {Topic Title}

## Quick Reference
> Concise summary of the most critical information.

## Decision Tree
> When to use what. Flowchart-style guidance.

## Patterns

### Pattern 1: {Name}
> When: {condition}
> Use: {approach}

```dockerfile
# Code example
```

### Pattern 2: {Name}
...

## Anti-Patterns
> NEVER do X because Y. ALWAYS do Z instead.

## Reference Links
- [references/examples.md](references/examples.md) — Extended examples
- [Official Docs](https://docs.docker.com/...) — Source documentation
```

---

## Naming Conventions

### Skill Names
Format: `docker-{category}-{topic}`

| Category | Examples |
|----------|----------|
| syntax | docker-syntax-dockerfile, docker-syntax-compose, docker-syntax-cli |
| impl | docker-impl-multistage, docker-impl-cicd, docker-impl-registry |
| errors | docker-errors-build, docker-errors-runtime, docker-errors-networking |
| core | docker-core-security, docker-core-networking, docker-core-storage |
| agents | docker-agent-validator, docker-agent-generator |

### File Names
- Skill directories: lowercase, hyphenated (`docker-syntax-dockerfile`)
- Reference files: lowercase, hyphenated (`multi-stage-examples.md`)
- Research files: `{skill-name}-research.md`

---

## Content Standards

### Language
- English only (D-003)
- Deterministic phrasing:
  - ALWAYS: "ALWAYS use multi-stage builds for production images"
  - NEVER: "NEVER run containers as root in production"
  - DO NOT: "DO NOT use `latest` tag in production Dockerfiles"
- Avoid hedging: not "you might want to consider" but "USE X when Y"

### Code Examples
- MUST be complete and runnable
- MUST include comments explaining key lines
- MUST show both Dockerfile and Compose YAML where applicable
- MUST use current syntax (BuildKit, Compose v2)
- MUST annotate version-specific features

### Cross-References
- Link to related skills within the package
- Link to official documentation via SOURCES.md URLs
- Link to references/ files from SKILL.md

---

## Quality Gates

### Per-Skill Gate (before accepting)
1. YAML frontmatter valid
2. Line count < 500
3. English-only
4. Deterministic language
5. Code examples verified
6. References exist and are linked
7. Sources traceable

### Per-Batch Gate (after 3 skills)
1. All 3 skills pass per-skill gate
2. No conflicting advice between skills
3. Cross-references valid
4. ROADMAP.md updated

### Per-Phase Gate (before next phase)
1. All batches pass
2. Core files updated (P-006)
3. CHANGELOG entry added
4. Commit with phase message
