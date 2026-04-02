# Docker & Docker Compose — Vooronderzoek

> Broad research across all Docker domains for the Docker Claude Skill Package.
> Date: 2026-03-19

---

## Research Methodology

Research was conducted in 3 parallel streams, each fetching and analyzing official Docker documentation via WebFetch. All content verified against docs.docker.com (Tier 1 source per SOURCES.md).

## Research Fragments

The complete research is split across 3 fragments:

### Fragment 1: Dockerfile & BuildKit
**File**: [fragments/dockerfile-buildkit-research.md](fragments/dockerfile-buildkit-research.md)
**Sources fetched**: 8 official documentation pages
**Sections**:
1. Parser Directives (syntax, escape, check)
2. Dockerfile Instructions Reference (all 17 instructions)
3. BuildKit Features (mounts, heredocs, cache backends)
4. Multi-Stage Build Patterns (builder pattern, parallel stages, target)
5. Layer Optimization & Caching (invalidation rules, cache mounts for 8 package managers)
6. Best Practices (base images, non-root, signals, labels, reproducibility)
7. Common Anti-Patterns (12 documented anti-patterns)

### Fragment 2: Docker Compose v2
**File**: [fragments/compose-v2-research.md](fragments/compose-v2-research.md)
**Sources fetched**: 19 official documentation pages
**Sections**:
1. Compose File Structure (top-level elements, naming, remote files)
2. Service Configuration — COMPLETE (33+ attributes)
3. Networks Configuration (drivers, IPAM, external, aliases)
4. Volumes Configuration (named, drivers, external)
5. Configs & Secrets (file/env/content/external, mounting)
6. Environment Variables (interpolation, .env, precedence)
7. Profiles (assignment, activation, dependency resolution)
8. Merge & Override Patterns (merge rules, extends, include)
9. Compose CLI Commands (30+ subcommands)
10. Compose Watch (develop section, 3 action types)
11. Common Anti-Patterns (8 anti-patterns)

### Fragment 3: Docker CLI, Security, Networking, Storage & Errors
**File**: [fragments/cli-security-ops-research.md](fragments/cli-security-ops-research.md)
**Sources fetched**: 25 official documentation pages
**Sections**:
1. Container Lifecycle Commands (docker run 70+ flags, all lifecycle commands)
2. Image Management Commands (buildx, pull/push, prune, save/load, manifest)
3. Network Management (6 drivers, DNS, port publishing, subnet allocation)
4. Volume & Storage Management (4 mount types, NFS/CIFS, backup/restore)
5. Security (namespaces, cgroups, capabilities, rootless, Scout, content trust)
6. Go Template Formatting (30+ practical format patterns)
7. Common Errors & Troubleshooting (33 specific errors with solutions)
8. System & Maintenance (df, prune, info, context)

---

## Key Findings for Skill Design

### Coverage Breadth
- **Dockerfile**: 17 instructions + parser directives + BuildKit extensions
- **BuildKit**: 5 mount types, heredocs, cache backends (6 types), inline cache
- **Compose v2**: 33+ service attributes, 5 top-level elements, remote files, watch
- **CLI**: 70+ `docker run` flags, 30+ compose subcommands, Go templates
- **Security**: Docker Scout, rootless, seccomp, capabilities, content trust
- **Networking**: 6 drivers, DNS resolution, port mapping, isolation
- **Storage**: 4 mount types, volume drivers, backup patterns
- **Errors**: 33 documented error patterns with solutions

### Skill Candidate Summary
Based on research volume and topic boundaries, the following skill inventory is recommended:
- **core/**: 3 skills (architecture, security, networking)
- **syntax/**: 7 skills (dockerfile, buildkit, multistage, compose-services, compose-resources, cli-containers, cli-images)
- **impl/**: 6 skills (build-optimization, compose-workflows, cicd, production, storage, go-templates)
- **errors/**: 4 skills (build, runtime, networking, compose)
- **agents/**: 2 skills (review, generator)
- **Total**: 22 skills across 8 batches

### Decision Points
1. Dockerfile instructions are too large for one skill → split into dockerfile (instructions) + buildkit (extensions) + multistage (patterns)
2. Compose is too large for one skill → split into services (config) + resources (networks/volumes/secrets)
3. CLI is too large for one skill → split into containers (lifecycle) + images (management)
4. Go templates warrant their own skill — distinct syntax used across many commands
5. Production hardening combines base images, non-root, signals, health checks — cross-cutting concern
