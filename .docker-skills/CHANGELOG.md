# Changelog

All notable changes to the Docker Claude Skill Package will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [1.0.0] - 2026-03-19

### Added
- 22 Docker skills across 5 categories (core, syntax, impl, errors, agents)
- 3 core skills: architecture, security, networking
- 7 syntax skills: dockerfile, buildkit, multistage, compose-services, compose-resources, cli-containers, cli-images
- 6 implementation skills: build-optimization, production, storage, cicd, compose-workflows, go-templates
- 4 error skills: build, runtime, networking, compose
- 2 agent skills: review (validation checklist), generator (Dockerfile/Compose generation)
- 66 reference files (3 per skill: methods/patterns, examples, anti-patterns)
- Comprehensive research: 52 official Docker documentation pages fetched and analyzed
- INDEX.md with complete skill catalog
- README.md with installation instructions and skill overview
- Validation report: all 22 skills pass quality criteria
- docs/masterplan/docker-masterplan.md with 22-skill execution plan
- docs/research/ with vooronderzoek and 3 research fragments

### Infrastructure
- Repository initialized with 7-phase research-first methodology
- Core files: CLAUDE.md, ROADMAP.md, REQUIREMENTS.md, DECISIONS.md, SOURCES.md, WAY_OF_WORK.md, LESSONS.md
- MIT License (OpenAEC Foundation)
- .gitignore configured
