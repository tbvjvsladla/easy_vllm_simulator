# Docker Skill Package — Definitive Masterplan

## Status

Phase 3 complete. Finalized from raw masterplan after vooronderzoek review.
Date: 2026-03-19

---

## Decisions Made During Refinement

| # | Decision | Rationale |
|---|----------|-----------|
| D-01 | **Split** Dockerfile into 3 skills: `dockerfile`, `buildkit`, `multistage` | Dockerfile reference alone is 17 instructions. BuildKit adds 5 mount types + heredocs. Multi-stage has enough patterns for its own skill. Each would exceed 500 lines combined. |
| D-02 | **Split** Compose into 2 skills: `compose-services`, `compose-resources` | Service configuration has 33+ attributes. Networks/volumes/configs/secrets are distinct infrastructure concerns. |
| D-03 | **Split** CLI into 2 skills: `cli-containers`, `cli-images` | Container lifecycle (run, exec, logs) and image management (build, pull, prune) are distinct workflows with different mental models. |
| D-04 | **Added** `docker-impl-go-templates` | Go template syntax for --format flags is a cross-cutting concern used in 10+ commands. Users frequently search for format patterns. Warrants its own skill. |
| D-05 | **Added** `docker-impl-production` | Production hardening (base images, non-root, signals, health checks, entrypoints) is a cross-cutting pattern not fitting any single syntax skill. |
| D-06 | **Merged** networking concepts into `docker-core-networking` | Network drivers, DNS, and isolation are conceptual. Practical troubleshooting goes in `docker-errors-networking`. |
| D-07 | **Reordered** batches: core first, then syntax, impl, errors, agents last | Ensures dependency chain: core → syntax → impl → errors → agents. |

**Result**: 22 raw skills → **22 definitive skills** (3 splits, 2 additions, 0 removals).

---

## Definitive Skill Inventory (22 skills)

### docker-core/ (3 skills)

| Name | Scope | Key Topics | Research Input | Complexity | Dependencies |
|------|-------|------------|----------------|------------|-------------|
| `docker-core-architecture` | Docker Engine architecture; daemon, containerd, runc; OCI standards; images vs containers; layers and union filesystem; build context; Docker objects lifecycle | Engine components, OCI spec, image layers | Fragment 1 §1-2, Fragment 3 §1.1 | M | None |
| `docker-core-security` | Image scanning (Scout, Trivy); rootless Docker; non-root containers; capabilities (cap_add/cap_drop); read-only filesystem; seccomp/AppArmor; content trust; resource limits; Docker Bench | Security hardening, scanning tools | Fragment 3 §5 | L | None |
| `docker-core-networking` | Network drivers (bridge, host, overlay, macvlan, ipvlan, none); DNS resolution; service discovery; port mapping; network isolation; user-defined vs default bridge | Drivers, DNS, isolation | Fragment 3 §3 | M | None |

### docker-syntax/ (7 skills)

| Name | Scope | Key Topics | Research Input | Complexity | Dependencies |
|------|-------|------------|----------------|------------|-------------|
| `docker-syntax-dockerfile` | All 17 Dockerfile instructions; parser directives; FROM, RUN, CMD, ENTRYPOINT, COPY, ADD, ENV, ARG, EXPOSE, VOLUME, WORKDIR, USER, HEALTHCHECK, LABEL, SHELL, STOPSIGNAL, ONBUILD; CMD vs ENTRYPOINT; COPY vs ADD | Instruction reference | Fragment 1 §1-2 | L | core-architecture |
| `docker-syntax-buildkit` | BuildKit syntax directive; heredoc syntax; cache mounts; secret mounts; SSH mounts; bind mounts; tmpfs mounts; cache backends (local, registry, s3, gha, azblob); inline cache; platform ARGs | Mount types, cache backends | Fragment 1 §3 | M | syntax-dockerfile |
| `docker-syntax-multistage` | Multi-stage build patterns; FROM ... AS naming; COPY --from; parallel stages; --target selection; builder pattern; minimal production images; test stages; shared dependency stages | Stage patterns | Fragment 1 §4 | M | syntax-dockerfile |
| `docker-syntax-compose-services` | Compose service configuration; image, build, command, entrypoint; ports (short/long); environment, env_file; depends_on (conditions); healthcheck; deploy; restart; logging; profiles; extends; platform; pull_policy | 33+ service attributes | Fragment 2 §2 | L | core-architecture |
| `docker-syntax-compose-resources` | Compose networks (drivers, IPAM, external); volumes (named, drivers, external); configs (file/env/content); secrets (file/env/external); mounting syntax; network aliases; volume labels | Infrastructure resources | Fragment 2 §3-5 | M | syntax-compose-services |
| `docker-syntax-cli-containers` | Container lifecycle; docker run (all flags); create, start, stop, restart, kill; rm, prune; exec; attach; logs; inspect; stats; top; events; wait; cp; diff; ps (filters + format) | Container commands | Fragment 3 §1 | L | core-architecture |
| `docker-syntax-cli-images` | Image management; docker build/buildx; pull, push, tag; images/ls (filter/format); rmi, prune; system prune; save/load; history; manifest; docker system df/info/version; docker context | Image + system commands | Fragment 3 §2, §8 | M | core-architecture |

### docker-impl/ (6 skills)

| Name | Scope | Key Topics | Research Input | Complexity | Dependencies |
|------|-------|------------|----------------|------------|-------------|
| `docker-impl-build-optimization` | Layer caching rules; cache invalidation triggers; instruction ordering; .dockerignore patterns; build context optimization; cache mount patterns for apt/npm/pip/go/maven/cargo; bind mounts for contexts; cache backends in CI | Caching strategies | Fragment 1 §5 | M | syntax-dockerfile, syntax-buildkit |
| `docker-impl-compose-workflows` | Compose development workflows; profiles (assignment, activation); extends and include; merge and override patterns; compose.override.yaml; environment variable precedence; .env file rules; compose watch (develop section); remote Compose files | Dev workflow patterns | Fragment 2 §6-10 | M | syntax-compose-services |
| `docker-impl-cicd` | CI/CD integration; GitHub Actions workflows; registry authentication; image pushing; multi-platform builds (buildx, QEMU); cache sharing in CI (gha backend); build matrix; Docker Hub/GHCR publishing | CI/CD patterns | Fragment 3 §2 (buildx), Fragment 1 §5 (cache backends) | M | syntax-cli-images, impl-build-optimization |
| `docker-impl-production` | Production Dockerfile patterns; base image selection (scratch, alpine, slim, distroless); non-root USER patterns; signal handling (STOPSIGNAL, exec form, tini/dumb-init); HEALTHCHECK patterns; entrypoint scripts; OCI labels; reproducible builds (digest pinning) | Production hardening | Fragment 1 §6 | M | syntax-dockerfile, syntax-multistage |
| `docker-impl-storage` | Volume management; named volumes vs bind mounts vs tmpfs; --mount vs -v syntax; volume drivers (local, NFS, CIFS); backup and restore procedures; read-only volumes; database persistence patterns; Compose volume integration; storage cleanup strategies | Storage patterns | Fragment 3 §4 | M | core-networking |
| `docker-impl-go-templates` | Go template syntax for --format; template functions; conditional expressions; range loops; table formatting; JSON output; common format strings for inspect, ps, images, stats, network, volume; custom output patterns | Format patterns | Fragment 3 §6 | S | syntax-cli-containers |

### docker-errors/ (4 skills)

| Name | Scope | Key Topics | Research Input | Complexity | Dependencies |
|------|-------|------------|----------------|------------|-------------|
| `docker-errors-build` | Dockerfile build failures; COPY/ADD file not found; context too large; cache invalidation unexpected; BuildKit mount errors; multi-stage reference errors; base image pull failures; platform mismatch; ARG/ENV scope issues; permission during build | Build error diagnostics | Fragment 3 §7 (build errors), Fragment 1 §7 | M | syntax-dockerfile, syntax-buildkit |
| `docker-errors-runtime` | Container runtime errors; OOM killed; permission denied; port already in use; container exits immediately; exec format error; read-only filesystem writes; PID limit; resource exhaustion; debugging workflow (logs → exec → inspect → events) | Runtime diagnostics | Fragment 3 §7 (runtime errors) | M | syntax-cli-containers |
| `docker-errors-networking` | Network troubleshooting; DNS resolution failures; connection refused between containers; port mapping not working; default bridge limitations; overlay network issues; firewall/iptables conflicts; container cannot reach internet | Network diagnostics | Fragment 3 §7 (network errors), §3 | M | core-networking |
| `docker-errors-compose` | Compose-specific errors; service dependency failures; orphan containers; port conflicts between services; volume mount permission issues; environment variable interpolation errors; profile dependency resolution; build context errors; compose config validation | Compose diagnostics | Fragment 3 §7, Fragment 2 §11 | M | syntax-compose-services |

### docker-agents/ (2 skills)

| Name | Scope | Key Topics | Research Input | Complexity | Dependencies |
|------|-------|------------|----------------|------------|-------------|
| `docker-agents-review` | Validation checklist for Dockerfiles and Compose files; anti-pattern detection; security audit; best practice compliance; layer optimization check; Compose config validation; health check verification; resource limit verification | All anti-patterns from research | Fragment 1 §7, Fragment 2 §11, Fragment 3 §7 | M | ALL syntax + impl skills |
| `docker-agents-generator` | Generate Dockerfiles from app requirements; generate Compose files for common stacks; language-specific templates (Node, Python, Go, Java, .NET); development vs production configs; proper security defaults; health checks; .dockerignore generation | All patterns from research | All fragments | L | ALL core + syntax skills |

---

## Batch Execution Plan (DEFINITIVE)

| Batch | Skills | Count | Dependencies | Notes |
|-------|--------|-------|-------------|-------|
| 1 | `core-architecture`, `core-security` | 2 | None | Foundation skills, no dependencies |
| 2 | `core-networking`, `syntax-dockerfile`, `syntax-buildkit` | 3 | Batch 1 | Core networking + primary Dockerfile syntax |
| 3 | `syntax-multistage`, `syntax-compose-services`, `syntax-compose-resources` | 3 | Batch 1-2 | Multi-stage + Compose syntax |
| 4 | `syntax-cli-containers`, `syntax-cli-images`, `impl-go-templates` | 3 | Batch 1-2 | CLI skills + Go templates |
| 5 | `impl-build-optimization`, `impl-production`, `impl-storage` | 3 | Batch 2-3 | Core implementation patterns |
| 6 | `impl-compose-workflows`, `impl-cicd`, `errors-build` | 3 | Batch 2-5 | Compose workflows + CI/CD + first error skill |
| 7 | `errors-runtime`, `errors-networking`, `errors-compose` | 3 | Batch 2-6 | Remaining error skills |
| 8 | `agents-review`, `agents-generator` | 2 | ALL above | Agent skills reference all other skills |

**Total**: 22 skills across 8 batches.

---

## Per-Skill Agent Prompts

### Constants

```
PROJECT_ROOT = C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package
RESEARCH_DIR = C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\docs\research\fragments
REQUIREMENTS_FILE = C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\REQUIREMENTS.md
REFERENCE_SKILL = C:\Users\Freek Heijting\Documents\GitHub\Tauri-2-Claude-Skill-Package\skills\source\tauri-core\tauri-core-architecture\SKILL.md
```

---

### Batch 1

#### Prompt: docker-core-architecture

```
## Task: Create the docker-core-architecture skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-core\docker-core-architecture\

### Files to Create
1. SKILL.md (main skill file, <500 lines)
2. references/concepts.md (Docker object types, image layers, union filesystem)
3. references/examples.md (architecture diagrams in text, component interaction examples)
4. references/anti-patterns.md (architectural mistakes)

### Reference Format
Read and follow the structure of:
C:\Users\Freek Heijting\Documents\GitHub\Tauri-2-Claude-Skill-Package\skills\source\tauri-core\tauri-core-architecture\SKILL.md

### YAML Frontmatter
---
name: docker-core-architecture
description: "Guides Docker Engine architecture including daemon, containerd, runc, OCI standards, image and container concepts, layer filesystem, build context model, and Docker object lifecycle. Activates when explaining Docker concepts, understanding image layers, reasoning about container isolation, or setting up Docker environments."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT — do not exceed)
- Docker Engine architecture: dockerd daemon, containerd, runc
- OCI standards: image spec, runtime spec
- Images vs containers: immutable layers, writable container layer
- Union filesystem and layer model
- Build context: what it is, how it's sent to daemon
- Docker objects: images, containers, networks, volumes
- Container lifecycle: created → running → paused → stopped → removed
- Key commands mapping to architecture (build → image, run → container)

### Research Sections to Read
- Fragment 1 (dockerfile-buildkit-research.md): §1-2 for parser/instruction concepts
- Fragment 3 (cli-security-ops-research.md): §1.1 for container lifecycle, §8 for system info

### Quality Rules
- English only
- SKILL.md < 500 lines; heavy content goes in references/
- Use ALWAYS/NEVER deterministic language, not "you should" or "consider"
- Include architecture component diagram (ASCII art)
- Include container lifecycle state diagram
- Include Critical Warnings section with NEVER rules
```

#### Prompt: docker-core-security

```
## Task: Create the docker-core-security skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-core\docker-core-security\

### Files to Create
1. SKILL.md (main skill file, <500 lines)
2. references/scanning.md (Docker Scout, Trivy, Snyk integration details)
3. references/hardening-checklist.md (complete security hardening checklist)
4. references/anti-patterns.md (security anti-patterns)

### YAML Frontmatter
---
name: docker-core-security
description: "Guides Docker security including image scanning with Docker Scout and Trivy, rootless Docker, non-root containers, Linux capabilities management, read-only filesystems, seccomp and AppArmor profiles, content trust, resource limits, and Docker Bench for Security. Activates when hardening containers, scanning images for vulnerabilities, implementing least-privilege containers, or auditing Docker security."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT — do not exceed)
- Image scanning: Docker Scout (docker scout cves, quickview, recommendations), Trivy, Snyk
- Content trust and image signing (DOCKER_CONTENT_TRUST)
- Rootless Docker setup and limitations
- Non-root containers: USER instruction, numeric UID/GID
- Capabilities: default set, cap_add, cap_drop, --cap-drop=ALL pattern
- Read-only root filesystem (--read-only + tmpfs for writable dirs)
- Seccomp profiles (default, custom)
- AppArmor profiles
- Resource limits: --memory, --cpus, --pids-limit
- Secret management overview (build secrets, runtime secrets)
- Docker Bench for Security
- Supply chain security (SBOM, provenance)

### Research Sections to Read
- Fragment 3 (cli-security-ops-research.md): §5 Security (complete section)

### Quality Rules
- English only
- SKILL.md < 500 lines; heavy content goes in references/
- Use ALWAYS/NEVER deterministic language
- Include security hardening decision tree
- Include "minimum viable security" quick-start pattern
- Include Critical Warnings section
```

---

### Batch 2

#### Prompt: docker-core-networking

```
## Task: Create the docker-core-networking skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-core\docker-core-networking\

### Files to Create
1. SKILL.md (<500 lines)
2. references/drivers.md (all network driver details, options, use cases)
3. references/examples.md (network creation, multi-container networking, isolation)
4. references/anti-patterns.md (networking mistakes)

### YAML Frontmatter
---
name: docker-core-networking
description: "Guides Docker networking including bridge, host, overlay, macvlan, ipvlan, and none network drivers, DNS resolution and service discovery, port mapping and publishing, network isolation patterns, user-defined vs default bridge networks, and container-to-container communication. Activates when configuring Docker networks, debugging connectivity, setting up service discovery, or designing network architecture."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- Network drivers: bridge, host, overlay, macvlan, ipvlan, none
- Default bridge vs user-defined bridge (why user-defined is ALWAYS preferred)
- DNS resolution in user-defined networks (automatic by container name)
- Port mapping: -p host:container, -p ip:host:container, -P
- Network isolation: --internal networks, no inter-network communication
- Container-to-container communication patterns
- docker network create/connect/disconnect/ls/inspect/rm/prune
- IPAM configuration (subnet, gateway, ip-range)
- Overlay network specifics (Swarm, encryption)
- Macvlan/ipvlan use cases

### Research Sections to Read
- Fragment 3 (cli-security-ops-research.md): §3 Network Management (complete)

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Include network driver decision tree
- Include DNS resolution diagram
- Include port mapping syntax table
```

#### Prompt: docker-syntax-dockerfile

```
## Task: Create the docker-syntax-dockerfile skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-syntax\docker-syntax-dockerfile\

### Files to Create
1. SKILL.md (<500 lines)
2. references/instructions.md (complete reference for all 17 instructions with all parameters)
3. references/examples.md (complete Dockerfile examples for common scenarios)
4. references/anti-patterns.md (instruction misuse patterns)

### YAML Frontmatter
---
name: docker-syntax-dockerfile
description: "Complete Dockerfile instruction reference including FROM, RUN, CMD, ENTRYPOINT, COPY, ADD, ENV, ARG, EXPOSE, VOLUME, WORKDIR, USER, HEALTHCHECK, LABEL, SHELL, STOPSIGNAL, ONBUILD, and parser directives. Activates when writing Dockerfiles, choosing between CMD and ENTRYPOINT, configuring COPY vs ADD, setting health checks, or understanding instruction syntax and behavior."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+ with BuildKit."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- Parser directives: syntax, escape, check
- All 17 instructions with complete syntax, parameters, and behavior
- CMD vs ENTRYPOINT interaction matrix (exec form vs shell form)
- COPY vs ADD comparison (when to use each — ALWAYS prefer COPY)
- ENV vs ARG scope and persistence differences
- HEALTHCHECK syntax and all parameters
- Shell form vs exec form for RUN, CMD, ENTRYPOINT
- ONBUILD triggers and restrictions
- Instruction ordering impact on caching (covered briefly, detail in impl-build-optimization)

### Research Sections to Read
- Fragment 1 (dockerfile-buildkit-research.md): §1 Parser Directives, §2 ALL instructions

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Include CMD vs ENTRYPOINT matrix table (4 combinations)
- Include COPY vs ADD decision guide
- Include shell form vs exec form comparison
- Heavy instruction detail goes in references/instructions.md
```

#### Prompt: docker-syntax-buildkit

```
## Task: Create the docker-syntax-buildkit skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-syntax\docker-syntax-buildkit\

### Files to Create
1. SKILL.md (<500 lines)
2. references/mounts.md (complete reference for all 5 mount types with all options)
3. references/examples.md (cache mount patterns per package manager, secret patterns, SSH patterns)
4. references/anti-patterns.md (BuildKit feature misuse)

### YAML Frontmatter
---
name: docker-syntax-buildkit
description: "Guides BuildKit-specific Dockerfile features including syntax directive, heredoc syntax for multi-line RUN, cache mounts for package managers, secret mounts for credentials, SSH mounts for private repos, bind mounts, tmpfs mounts, cache backends, and platform-specific ARGs. Activates when optimizing Dockerfile builds, mounting secrets during build, caching package manager downloads, or using heredoc syntax in RUN instructions."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+ with BuildKit (default)."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- Syntax directive: # syntax=docker/dockerfile:1
- Heredoc syntax: RUN <<EOF, COPY <<EOF, inline files
- Cache mounts: --mount=type=cache (target, id, sharing, from)
- Secret mounts: --mount=type=secret (id, target, required, env)
- SSH mounts: --mount=type=ssh (id, target, required)
- Bind mounts: --mount=type=bind (source, target, from, rw)
- Tmpfs mounts: --mount=type=tmpfs (target, size)
- Cache mount patterns: apt, npm, pip, go, maven, cargo, yarn, pnpm
- Platform-specific ARGs: TARGETPLATFORM, TARGETOS, TARGETARCH, BUILDPLATFORM
- Cache backends: local, registry, s3, gha, azblob, inline

### Research Sections to Read
- Fragment 1 (dockerfile-buildkit-research.md): §3 BuildKit Features, §5 Cache backends

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Include mount type decision tree
- Include cache mount pattern for EACH package manager (apt, npm, pip, go, maven, cargo)
- Include secret mount security warnings
```

---

### Batch 3

#### Prompt: docker-syntax-multistage

```
## Task: Create the docker-syntax-multistage skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-syntax\docker-syntax-multistage\

### Files to Create
1. SKILL.md (<500 lines)
2. references/patterns.md (all multi-stage patterns with complete Dockerfiles)
3. references/examples.md (language-specific multi-stage examples: Node, Python, Go, Java)
4. references/anti-patterns.md (multi-stage mistakes)

### YAML Frontmatter
---
name: docker-syntax-multistage
description: "Guides Docker multi-stage build patterns including stage naming with FROM AS, COPY --from for artifact extraction, parallel build stages, --target for partial builds, builder pattern for compiled languages, test stages, shared dependency stages, and minimal production images. Activates when optimizing Docker image size, separating build and runtime dependencies, creating production-ready images, or implementing CI/CD build pipelines."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+ with BuildKit."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- FROM ... AS <name> stage naming
- COPY --from=<stage> artifact extraction
- COPY --from=<image> (external image reference)
- Parallel build stages (BuildKit automatic parallelization)
- --target flag for partial builds
- Builder pattern: build stage → production stage
- Test stage pattern: build → test → production
- Shared dependency stage pattern
- Language-specific patterns: Go, Node.js, Python, Java, Rust, .NET
- Image size optimization through stage selection
- Distroless and scratch final stages

### Research Sections to Read
- Fragment 1 (dockerfile-buildkit-research.md): §4 Multi-Stage Build Patterns

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Include at least 4 language-specific multi-stage examples
- Include size comparison table (with vs without multi-stage)
- ALWAYS use multi-stage for production images
```

#### Prompt: docker-syntax-compose-services

```
## Task: Create the docker-syntax-compose-services skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-syntax\docker-syntax-compose-services\

### Files to Create
1. SKILL.md (<500 lines)
2. references/attributes.md (complete service attribute reference with all options)
3. references/examples.md (common service configurations: web, db, cache, worker)
4. references/anti-patterns.md (service configuration mistakes)

### YAML Frontmatter
---
name: docker-syntax-compose-services
description: "Complete Docker Compose service configuration reference including image, build, command, ports, environment, volumes, networks, depends_on with health conditions, healthcheck, deploy with resource limits, restart policies, logging, profiles, extends, and all other service attributes. Activates when writing compose.yaml services, configuring service dependencies, setting up health checks, managing port mappings, or configuring resource limits."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Compose v2."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- image, build (context, dockerfile, args, target, cache_from, secrets)
- command, entrypoint (string and list forms)
- ports: short syntax, long syntax
- expose
- environment, env_file (precedence rules)
- volumes: short syntax, long syntax (type, source, target, read_only)
- tmpfs
- networks, network_mode
- depends_on: simple form, condition form (service_healthy, service_started, service_completed_successfully)
- healthcheck: test, interval, timeout, retries, start_period, start_interval
- deploy: replicas, resources (limits/reservations), restart_policy, placement
- restart: no, always, on-failure, unless-stopped
- logging: driver, options
- container_name, hostname, user, working_dir
- privileged, cap_add, cap_drop, security_opt, read_only
- sysctls, ulimits, init, pid, ipc
- extra_hosts, dns
- profiles, extends, platform, pull_policy

### Research Sections to Read
- Fragment 2 (compose-v2-research.md): §2 Service Configuration (COMPLETE section)

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Include depends_on condition comparison table
- Include ports syntax comparison (short vs long)
- Include healthcheck pattern template
- Heavy attribute details go in references/attributes.md
```

#### Prompt: docker-syntax-compose-resources

```
## Task: Create the docker-syntax-compose-resources skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-syntax\docker-syntax-compose-resources\

### Files to Create
1. SKILL.md (<500 lines)
2. references/networks.md (all network options, drivers, IPAM config)
3. references/volumes-configs-secrets.md (volume, config, secret definitions and mounting)
4. references/anti-patterns.md (resource configuration mistakes)

### YAML Frontmatter
---
name: docker-syntax-compose-resources
description: "Guides Docker Compose infrastructure resources including top-level networks, volumes, configs, and secrets definitions, network drivers and IPAM configuration, external resources, mounting configs and secrets in services, volume drivers and options, and network aliases. Activates when defining Compose networks, creating named volumes, managing configs or secrets, or configuring network infrastructure in compose.yaml."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Compose v2."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- Top-level networks: driver (bridge, overlay, host, none), driver_opts, IPAM (driver, config: subnet, gateway, ip_range), external, internal, attachable, labels, name
- Network aliases in services
- Top-level volumes: driver, driver_opts, external, labels, name
- Volume mounting in services (short/long syntax reference)
- Top-level configs: file, environment, content, external, name
- Top-level secrets: file, environment, external, name
- Mounting configs in services: source, target, uid, gid, mode
- Mounting secrets in services: source, target, uid, gid, mode
- External resource pattern (pre-created outside Compose)
- Resource naming conventions

### Research Sections to Read
- Fragment 2 (compose-v2-research.md): §3 Networks, §4 Volumes, §5 Configs & Secrets

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Include network driver comparison table
- Include config vs secret usage comparison
- Include external resource pattern example
```

---

### Batch 4

#### Prompt: docker-syntax-cli-containers

```
## Task: Create the docker-syntax-cli-containers skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-syntax\docker-syntax-cli-containers\

### Files to Create
1. SKILL.md (<500 lines)
2. references/commands.md (complete container command reference with all flags)
3. references/examples.md (common container management workflows)
4. references/anti-patterns.md (CLI misuse patterns)

### YAML Frontmatter
---
name: docker-syntax-cli-containers
description: "Complete Docker container CLI reference including docker run with all major flags, exec, logs, inspect with Go templates, stats, top, events, container lifecycle commands (create, start, stop, restart, kill, rm, prune), cp, diff, and ps with filters and format. Activates when running containers, executing commands in containers, reading logs, inspecting container state, or managing container lifecycle."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- docker run: ALL major flag categories (execution, ports, storage, env, security, resources, logging, restart)
- docker create, start, stop, restart, kill (signals)
- docker rm, docker container prune
- docker exec (-it, -u, -w, -e, --privileged)
- docker attach (vs exec)
- docker logs (-f, --tail, --since, --until, -t, --details)
- docker inspect (--format with Go template basics)
- docker stats (--no-stream, --format)
- docker top, docker events (--filter)
- docker wait (exit code)
- docker cp (container↔host)
- docker diff (filesystem changes)
- docker ps (--all, --filter, --format, common filters)
- docker rename, docker update
- docker pause/unpause

### Research Sections to Read
- Fragment 3 (cli-security-ops-research.md): §1 Container Lifecycle Commands (complete)

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Include docker run flag category table (quick reference)
- Include docker ps --filter cheat sheet
- Include common docker inspect format strings
- Heavy command details go in references/commands.md
```

#### Prompt: docker-syntax-cli-images

```
## Task: Create the docker-syntax-cli-images skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-syntax\docker-syntax-cli-images\

### Files to Create
1. SKILL.md (<500 lines)
2. references/commands.md (complete image and system command reference)
3. references/examples.md (image management workflows, cleanup scripts)
4. references/anti-patterns.md (image management mistakes)

### YAML Frontmatter
---
name: docker-syntax-cli-images
description: "Complete Docker image and system CLI reference including docker build and buildx, pull, push, tag, image listing with filters and format, image and system pruning, save/load for offline transfer, history, manifest for multi-platform, and system management commands (df, info, version, context). Activates when building images, managing registries, cleaning up disk space, transferring images, or checking system status."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- docker build / docker buildx build (--tag, --file, --target, --build-arg, --secret, --ssh, --cache-from, --cache-to, --platform, --progress, --no-cache, --pull, --push, --load)
- docker buildx create, use, inspect, ls
- docker pull, push, tag
- docker images / docker image ls (--filter: dangling, label, before, since, reference; --format)
- docker rmi / docker image rm
- docker image prune (-a, --filter until)
- docker system prune (-a, --volumes, --filter)
- docker save / docker load (offline transfer)
- docker history (--no-trunc)
- docker manifest (inspect, create, push — multi-platform)
- docker system df (disk usage)
- docker system info
- docker version
- docker context (create, use, ls — remote Docker)

### Research Sections to Read
- Fragment 3 (cli-security-ops-research.md): §2 Image Management, §8 System & Maintenance

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Include buildx build flag table
- Include image filter cheat sheet
- Include disk cleanup strategy (prune levels)
```

#### Prompt: docker-impl-go-templates

```
## Task: Create the docker-impl-go-templates skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-impl\docker-impl-go-templates\

### Files to Create
1. SKILL.md (<500 lines)
2. references/patterns.md (30+ format patterns organized by command)
3. references/examples.md (complex template examples, conditional/range)
4. references/anti-patterns.md (template mistakes)

### YAML Frontmatter
---
name: docker-impl-go-templates
description: "Guides Go template syntax for Docker --format flags including template functions, conditional expressions, range loops, table formatting, JSON output, and 30+ ready-to-use format patterns for inspect, ps, images, stats, network, and volume commands. Activates when formatting Docker command output, extracting specific fields from inspect, creating custom table output, or scripting Docker commands."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- Go template syntax basics: {{ }}, {{ .Field }}, {{ .Field.SubField }}
- Template functions: json, join, upper, lower, title, split, println
- Conditional: {{ if }}, {{ else }}, {{ end }}
- Range/loops: {{ range }}, {{ .Key }}, {{ .Value }}
- Table formatting: table directive
- JSON output: {{ json . }}
- Index access: {{ index .Field 0 }}
- docker inspect format patterns (container, image, network, volume)
- docker ps format patterns (all placeholders)
- docker images format patterns
- docker stats format patterns
- docker network ls format patterns
- docker volume ls format patterns
- Scripting patterns: extracting IDs, IPs, ports for shell scripts

### Research Sections to Read
- Fragment 3 (cli-security-ops-research.md): §6 Go Template Formatting (complete)

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Include 30+ ready-to-use format patterns
- Organize by command (inspect, ps, images, etc.)
- Include both simple and complex template examples
```

---

### Batch 5

#### Prompt: docker-impl-build-optimization

```
## Task: Create the docker-impl-build-optimization skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-impl\docker-impl-build-optimization\

### Files to Create
1. SKILL.md (<500 lines)
2. references/caching-rules.md (complete cache invalidation rules per instruction type)
3. references/examples.md (optimized Dockerfiles before/after, .dockerignore patterns)
4. references/anti-patterns.md (caching and optimization mistakes)

### YAML Frontmatter
---
name: docker-impl-build-optimization
description: "Guides Docker build optimization including layer caching rules, cache invalidation triggers, instruction ordering for cache efficiency, .dockerignore patterns, build context optimization, cache mount patterns for all major package managers (apt, npm, pip, go, maven, cargo), bind mounts for large contexts, and CI/CD cache sharing with cache backends. Activates when optimizing Docker build times, reducing image sizes, fixing cache invalidation issues, or configuring build caching in CI/CD pipelines."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+ with BuildKit."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- Layer caching rules: when cache is used vs invalidated
- Cache invalidation triggers per instruction type (COPY checksum, RUN command string, ARG values)
- Instruction ordering strategy (least-changing first)
- .dockerignore: syntax, patterns, common entries, negation
- Build context optimization: keep context small, use .dockerignore
- Cache mount patterns: apt-get, npm/yarn/pnpm, pip, go modules, maven, cargo, composer
- Bind mount pattern for large build contexts
- Cache backends for CI: gha (GitHub Actions), s3, azblob, registry
- Cache sharing between CI runs
- Layer squashing considerations
- BuildKit garbage collection

### Research Sections to Read
- Fragment 1 (dockerfile-buildkit-research.md): §5 Layer Optimization & Caching (complete)
- Fragment 1: §3 BuildKit Features (cache mount patterns)

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Include instruction ordering diagram (most stable → least stable)
- Include .dockerignore template
- Include cache mount pattern for each package manager
- Include before/after comparison for optimized builds
```

#### Prompt: docker-impl-production

```
## Task: Create the docker-impl-production skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-impl\docker-impl-production\

### Files to Create
1. SKILL.md (<500 lines)
2. references/base-images.md (base image comparison table: scratch, alpine, slim, distroless, full)
3. references/examples.md (production Dockerfiles per language, entrypoint scripts, health checks)
4. references/anti-patterns.md (production deployment mistakes)

### YAML Frontmatter
---
name: docker-impl-production
description: "Guides production-ready Docker patterns including base image selection (scratch, alpine, slim, distroless), non-root USER configuration, signal handling with exec form and init processes (tini, dumb-init), HEALTHCHECK patterns, entrypoint scripts with exec, OCI metadata labels, reproducible builds with digest pinning, and minimal attack surface. Activates when preparing containers for production, choosing base images, configuring health checks, writing entrypoint scripts, or hardening Dockerfiles."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- Base image selection: scratch, alpine, slim, distroless, full — comparison table with sizes, package managers, shell availability
- Non-root USER: creating user/group, numeric UID/GID, directory ownership
- Signal handling: exec form vs shell form (PID 1 problem), --init flag, tini, dumb-init
- STOPSIGNAL configuration
- HEALTHCHECK: production patterns, health check scripts, startup probes
- Entrypoint scripts: pattern with exec "$@", environment validation, secrets injection
- OCI metadata labels: org.opencontainers.image.* labels
- Reproducible builds: digest pinning (@sha256:...), version pinning
- Minimal attack surface: no unnecessary packages, no dev dependencies
- .dockerignore for production builds
- Complete production Dockerfile template

### Research Sections to Read
- Fragment 1 (dockerfile-buildkit-research.md): §6 Best Practices (complete)
- Fragment 1: §7 Common Anti-Patterns

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Include base image comparison table (image, size, shell, pkg manager, use case)
- Include complete entrypoint script example
- Include production Dockerfile template per language
```

#### Prompt: docker-impl-storage

```
## Task: Create the docker-impl-storage skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-impl\docker-impl-storage\

### Files to Create
1. SKILL.md (<500 lines)
2. references/mount-types.md (complete comparison: volumes, bind mounts, tmpfs, named pipes)
3. references/examples.md (database persistence, backup/restore, NFS volumes, Compose volumes)
4. references/anti-patterns.md (storage mistakes)

### YAML Frontmatter
---
name: docker-impl-storage
description: "Guides Docker storage implementation including named volumes vs bind mounts vs tmpfs, --mount vs -v syntax comparison, volume drivers (local, NFS, CIFS), backup and restore procedures, read-only volumes, database persistence patterns, Compose volume integration, and storage cleanup strategies. Activates when persisting data, mounting host directories, configuring database storage, backing up volumes, or managing disk space."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- Mount types: named volumes, anonymous volumes, bind mounts, tmpfs
- --mount vs -v syntax: complete comparison, when to use each
- Named volumes: creation, lifecycle, sharing between containers
- Bind mounts: host path mapping, permission considerations
- tmpfs: in-memory storage, size limits
- Volume drivers: local, NFS, CIFS/SMB
- Backup procedure: volume → tar archive
- Restore procedure: tar archive → volume
- Read-only mounts (:ro flag)
- Database persistence patterns (PostgreSQL, MySQL, MongoDB)
- Compose volume integration (top-level volumes, service volumes)
- Storage cleanup: docker volume prune, dangling volumes
- Docker system df for space analysis

### Research Sections to Read
- Fragment 3 (cli-security-ops-research.md): §4 Volume & Storage Management (complete)

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Include mount type decision tree
- Include --mount vs -v comparison table
- Include backup/restore command sequences
- Include database persistence pattern
```

---

### Batch 6

#### Prompt: docker-impl-compose-workflows

```
## Task: Create the docker-impl-compose-workflows skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-impl\docker-impl-compose-workflows\

### Files to Create
1. SKILL.md (<500 lines)
2. references/patterns.md (profile patterns, merge rules, extend patterns, watch config)
3. references/examples.md (dev/prod workflow, multi-file setup, profile-based optional services)
4. references/anti-patterns.md (workflow mistakes)

### YAML Frontmatter
---
name: docker-impl-compose-workflows
description: "Guides Docker Compose development workflows including profiles for optional services, extends and include directives, merge and override patterns with multiple Compose files, compose.override.yaml conventions, environment variable precedence and .env file rules, compose watch for live file sync, and remote Compose file loading. Activates when setting up development workflows, managing environment-specific configs, using profiles, merging Compose files, or configuring live reload with compose watch."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Compose v2."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- Profiles: assignment to services, activation (--profile, COMPOSE_PROFILES), auto-activation via depends_on, multiple profiles
- Extends: service inheritance, restrictions (depends_on, volumes_from not inherited)
- Include: importing from other Compose files
- Multiple Compose files: -f flag ordering, merge rules (scalar: override, sequence: merge, mapping: merge)
- compose.override.yaml: automatic loading, development overrides
- Environment variable precedence: 5-level priority order
- .env file: location, syntax, COMPOSE_ENV_FILES
- Variable interpolation: ${VAR}, ${VAR:-default}, ${VAR-default}, ${VAR:?error}, ${VAR?error}, ${VAR:+replacement}, ${VAR+replacement}
- Compose watch: develop section, sync action, rebuild action, sync+restart action, path/target mapping
- Remote Compose files: OCI, Git (HTTPS/SSH), branches/tags
- Compose CLI workflow: up, down, build, ps, logs, exec, run, config

### Research Sections to Read
- Fragment 2 (compose-v2-research.md): §6-10 (Env vars, Profiles, Merge, CLI, Watch)

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Include environment variable precedence table
- Include interpolation syntax reference table
- Include merge rules table (per field type)
- Include compose watch configuration example
```

#### Prompt: docker-impl-cicd

```
## Task: Create the docker-impl-cicd skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-impl\docker-impl-cicd\

### Files to Create
1. SKILL.md (<500 lines)
2. references/github-actions.md (complete GitHub Actions workflow examples)
3. references/examples.md (multi-platform builds, registry auth, cache strategies)
4. references/anti-patterns.md (CI/CD mistakes)

### YAML Frontmatter
---
name: docker-impl-cicd
description: "Guides Docker CI/CD integration including GitHub Actions workflows with docker/build-push-action, registry authentication for Docker Hub and GHCR, multi-platform builds with buildx and QEMU, cache sharing in CI using GitHub Actions cache backend, build matrix strategies, image tagging conventions, and automated image publishing. Activates when setting up Docker CI/CD pipelines, pushing images to registries, building multi-platform images, or optimizing CI build times."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+ with BuildKit."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- GitHub Actions: docker/setup-buildx-action, docker/setup-qemu-action, docker/login-action, docker/build-push-action, docker/metadata-action
- Registry authentication: Docker Hub, GHCR (ghcr.io), private registries
- Multi-platform builds: buildx, QEMU emulation, platform matrix
- Cache backends in CI: type=gha (GitHub Actions cache), type=registry
- Image tagging: semantic versioning, git SHA, branch-based, latest
- docker/metadata-action for automated tag generation
- Build and push workflow (complete)
- Build matrix: multiple platforms, multiple Dockerfiles
- Security: never hardcode credentials, use GitHub secrets
- Docker Scout in CI (vulnerability scanning)

### Research Sections to Read
- Fragment 3 (cli-security-ops-research.md): §2 Image Management (buildx, registry)
- Fragment 1 (dockerfile-buildkit-research.md): §5 Cache backends in CI

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Include complete GitHub Actions workflow (build + push + multi-platform)
- Include registry auth comparison table
- Include cache strategy decision tree (gha vs registry vs s3)
```

#### Prompt: docker-errors-build

```
## Task: Create the docker-errors-build skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-errors\docker-errors-build\

### Files to Create
1. SKILL.md (<500 lines)
2. references/diagnostics.md (complete error → cause → solution table)
3. references/examples.md (error reproduction and fix examples)
4. references/anti-patterns.md (build configuration mistakes that cause errors)

### YAML Frontmatter
---
name: docker-errors-build
description: "Diagnoses and resolves Docker build errors including COPY/ADD file not found, build context too large, unexpected cache invalidation, BuildKit mount errors, multi-stage reference errors, base image pull failures, platform mismatch, ARG/ENV scope issues, permission errors during build, and Dockerfile syntax errors. Activates when encountering docker build failures, debugging cache issues, fixing COPY errors, or troubleshooting BuildKit problems."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- COPY/ADD file not found: .dockerignore excluding files, wrong context, wrong path
- Build context too large: missing .dockerignore, unnecessary files
- Cache invalidation unexpected: instruction ordering, COPY checksum changes
- BuildKit mount errors: wrong mount type, missing secret, SSH agent not running
- Multi-stage reference: stage not found, COPY --from wrong stage name
- Base image pull failure: registry auth, image not found, platform mismatch
- Platform mismatch: building for wrong architecture, missing --platform
- ARG scope: ARG before FROM only available to FROM, ARG after FROM for build stage
- Permission errors: file ownership in COPY, running as non-root
- Dockerfile syntax: invalid instruction, wrong escape character
- Build timeout and resource issues

### Research Sections to Read
- Fragment 3 (cli-security-ops-research.md): §7 Common Errors (build section)
- Fragment 1 (dockerfile-buildkit-research.md): §7 Common Anti-Patterns

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Format as diagnostic table: Error Message → Cause → Fix
- Include exact error messages where possible
- Include debugging workflow
```

---

### Batch 7

#### Prompt: docker-errors-runtime

```
## Task: Create the docker-errors-runtime skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-errors\docker-errors-runtime\

### Files to Create
1. SKILL.md (<500 lines)
2. references/diagnostics.md (complete error → cause → solution table)
3. references/examples.md (debugging sessions with commands)
4. references/anti-patterns.md (runtime configuration mistakes)

### YAML Frontmatter
---
name: docker-errors-runtime
description: "Diagnoses and resolves Docker container runtime errors including OOM killed, permission denied, port already in use, container exits immediately, exec format error, read-only filesystem write attempts, PID limit exceeded, resource exhaustion, and provides the container debugging workflow (logs → exec → inspect → events). Activates when containers crash, exit unexpectedly, run out of memory, have permission issues, or need debugging."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- OOM killed: detecting, --memory limits, inspecting OOMKilled flag
- Permission denied: file ownership, user mismatch, volume permissions, SELinux/AppArmor
- Port already in use: identifying conflicting process, choosing different port
- Container exits immediately: missing foreground process, CMD vs ENTRYPOINT issues, exec form
- Exec format error: wrong platform, missing shebang, binary not found
- Read-only filesystem: --read-only + tmpfs for writable dirs
- PID limit exceeded: --pids-limit
- Resource exhaustion: disk space, inode exhaustion
- Debugging workflow: docker logs → docker exec → docker inspect → docker events
- Exit codes: 0 (success), 1 (app error), 125 (daemon error), 126 (not executable), 127 (not found), 137 (SIGKILL/OOM), 143 (SIGTERM)
- docker inspect for debugging (State, RestartCount, OOMKilled, ExitCode)

### Research Sections to Read
- Fragment 3 (cli-security-ops-research.md): §7 Common Errors (runtime section)

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Format as diagnostic table: Symptom → Cause → Fix
- Include exit code reference table
- Include debugging flowchart
- Include docker inspect commands for each error type
```

#### Prompt: docker-errors-networking

```
## Task: Create the docker-errors-networking skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-errors\docker-errors-networking\

### Files to Create
1. SKILL.md (<500 lines)
2. references/diagnostics.md (complete network error → cause → solution table)
3. references/examples.md (network debugging sessions)
4. references/anti-patterns.md (networking configuration mistakes)

### YAML Frontmatter
---
name: docker-errors-networking
description: "Diagnoses and resolves Docker networking errors including DNS resolution failures between containers, connection refused, port mapping not working, default bridge communication issues, overlay network problems, firewall and iptables conflicts, container cannot reach internet, and network subnet conflicts. Activates when containers cannot communicate, DNS resolution fails, ports are not accessible, or network connectivity is broken."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- DNS resolution failure: using default bridge (no DNS), container name typo, network mismatch
- Connection refused: service not ready, wrong port, wrong network
- Port mapping not working: host port conflict, firewall rules, binding to wrong interface
- Default bridge limitations: no DNS, only --link (deprecated), ALWAYS use user-defined
- Containers on different networks: cannot communicate unless connected to same network
- Overlay network issues: Swarm not initialized, port 2377/7946/4789 blocked
- Firewall/iptables: Docker modifies iptables, conflicts with ufw/firewalld
- No internet from container: DNS config, proxy settings, network mode
- Subnet conflict: Docker network overlaps with host network
- Network debugging tools: docker network inspect, docker exec ping/nslookup/curl
- Compose networking: default network, service discovery, network_mode: host

### Research Sections to Read
- Fragment 3 (cli-security-ops-research.md): §7 Common Errors (network section), §3 Network Management

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Format as diagnostic table: Symptom → Cause → Fix
- Include network debugging flowchart
- Include "ALWAYS use user-defined networks" as rule #1
```

#### Prompt: docker-errors-compose

```
## Task: Create the docker-errors-compose skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-errors\docker-errors-compose\

### Files to Create
1. SKILL.md (<500 lines)
2. references/diagnostics.md (complete Compose error → cause → solution table)
3. references/examples.md (common Compose error scenarios with fixes)
4. references/anti-patterns.md (Compose configuration mistakes)

### YAML Frontmatter
---
name: docker-errors-compose
description: "Diagnoses and resolves Docker Compose errors including service dependency failures, orphan container warnings, port conflicts between services, volume mount permission issues, environment variable interpolation errors, profile dependency resolution, build context errors, and compose config validation failures. Activates when docker compose up fails, services won't start, dependencies time out, or Compose file has syntax errors."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Compose v2."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- Service dependency failures: depends_on without healthcheck, service_healthy condition not met
- Orphan containers: --remove-orphans, renamed services
- Port conflicts: multiple services on same host port
- Volume mount permission: host directory ownership, UID/GID mismatch
- Environment variable interpolation: unset variables, wrong syntax, dollar escaping ($$)
- Profile dependency: profile-dependent service not activated
- Build context errors: context not found, Dockerfile path wrong
- docker compose config: validation errors and how to fix
- YAML syntax errors: indentation, quotes, special characters
- Version field warning: "version is obsolete"
- Service name restrictions
- Compose file not found: naming, location, -f flag
- Resource limit errors: deploy.resources format

### Research Sections to Read
- Fragment 2 (compose-v2-research.md): §11 Common Anti-Patterns
- Fragment 3 (cli-security-ops-research.md): §7 Common Errors (compose section)

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Format as diagnostic table: Error Message → Cause → Fix
- Include docker compose config as validation tool
- Include depends_on with healthcheck as pattern
```

---

### Batch 8

#### Prompt: docker-agents-review

```
## Task: Create the docker-agents-review skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-agents\docker-agents-review\

### Files to Create
1. SKILL.md (<500 lines)
2. references/checklist.md (complete validation checklist organized by area)
3. references/examples.md (review scenarios: good Dockerfile, bad Dockerfile, fixes)
4. references/anti-patterns.md (all anti-patterns consolidated from all skills)

### YAML Frontmatter
---
name: docker-agents-review
description: "Validates Docker configurations by checking Dockerfiles for best practices, Compose files for correctness, security compliance, build optimization, and known anti-patterns. Provides structured validation checklists for Dockerfile review, Compose review, security audit, and production readiness assessment. Activates when reviewing Dockerfiles, validating Compose files, auditing container security, or checking Docker configurations before deployment."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+ and Docker Compose v2."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- Dockerfile validation checklist:
  - Base image: pinned version (not :latest), minimal image, official/verified
  - Instructions: correct form (exec vs shell), proper ordering for cache
  - Security: non-root USER, no secrets in ENV/ARG, minimal packages
  - Optimization: multi-stage build, .dockerignore exists, layer consolidation
  - Health: HEALTHCHECK defined, proper CMD/ENTRYPOINT
  - Labels: OCI metadata labels present
- Compose validation checklist:
  - No version: field, compose.yaml naming
  - depends_on with healthcheck conditions
  - Named volumes for persistent data
  - Environment via env_file (not hardcoded)
  - Resource limits defined (deploy.resources)
  - No container_name on scalable services
- Security audit checklist:
  - Image scanning (Scout/Trivy)
  - Capability dropping
  - Read-only filesystem where possible
  - Resource limits
  - Network isolation
- Production readiness checklist:
  - Health checks, restart policy, logging, resource limits
- Anti-pattern detection: all anti-patterns from research (12 Dockerfile + 8 Compose + runtime)

### Research Sections to Read
- Fragment 1 §7 (Dockerfile anti-patterns)
- Fragment 2 §11 (Compose anti-patterns)
- Fragment 3 §7 (Error patterns)

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Structure as runnable checklists (checkbox format)
- Group by area: Dockerfile, Compose, Security, Production
- Each check: what to verify, expected state, common failure
```

#### Prompt: docker-agents-generator

```
## Task: Create the docker-agents-generator skill

### Output Directory
C:\Users\Freek Heijting\Documents\GitHub\Docker-Claude-Skill-Package\skills\source\docker-agents\docker-agents-generator\

### Files to Create
1. SKILL.md (<500 lines)
2. references/templates.md (Dockerfile templates per language: Node, Python, Go, Java, Rust, .NET)
3. references/compose-templates.md (Compose templates for common stacks: web+db, web+db+cache, full stack)
4. references/anti-patterns.md (generation mistakes to avoid)

### YAML Frontmatter
---
name: docker-agents-generator
description: "Generates production-ready Dockerfiles and Docker Compose configurations from application requirements. Provides language-specific templates for Node.js, Python, Go, Java, Rust, and .NET, development and production Compose stacks, proper security defaults, health checks, and .dockerignore generation. Activates when generating Dockerfiles from scratch, containerizing an existing application, creating Compose configurations, or scaffolding Docker infrastructure for a project."
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+ and Docker Compose v2."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

### Scope (EXACT)
- Dockerfile generation decision tree: language → base image → build strategy → production stage
- Language-specific templates:
  - Node.js: multi-stage, npm ci, non-root, signal handling
  - Python: multi-stage, pip install, venv, non-root
  - Go: multi-stage, scratch/distroless final, static binary
  - Java: multi-stage, Maven/Gradle build, JRE runtime, jlink
  - Rust: multi-stage, cargo build --release, minimal runtime
  - .NET: multi-stage, dotnet publish, aspnet runtime
- Compose generation:
  - Web + Database (PostgreSQL/MySQL)
  - Web + Database + Cache (Redis)
  - Full stack (frontend + backend + db + cache + reverse proxy)
  - Development config (with volumes, debug ports, watch)
  - Production config (with resource limits, health checks, restart)
- .dockerignore generation per language
- Security defaults: non-root, health checks, resource limits, no :latest
- Environment configuration: .env template, env_file setup

### Research Sections to Read
- Fragment 1: §2 Instructions, §4 Multi-Stage, §6 Best Practices
- Fragment 2: §2 Service Config, §3-5 Resources
- All fragments: anti-patterns sections

### Quality Rules
- English only, <500 lines SKILL.md, ALWAYS/NEVER language
- Include complete, buildable Dockerfile templates (not snippets)
- Include complete, valid Compose templates
- Include decision tree for choosing the right template
- ALL generated code MUST follow best practices from other skills
```

---

## Appendix: Skill Directory Structure

```
skills/source/
├── docker-core/
│   ├── docker-core-architecture/
│   │   ├── SKILL.md
│   │   └── references/
│   │       ├── concepts.md
│   │       ├── examples.md
│   │       └── anti-patterns.md
│   ├── docker-core-security/
│   │   ├── SKILL.md
│   │   └── references/
│   └── docker-core-networking/
│       ├── SKILL.md
│       └── references/
├── docker-syntax/
│   ├── docker-syntax-dockerfile/
│   ├── docker-syntax-buildkit/
│   ├── docker-syntax-multistage/
│   ├── docker-syntax-compose-services/
│   ├── docker-syntax-compose-resources/
│   ├── docker-syntax-cli-containers/
│   └── docker-syntax-cli-images/
├── docker-impl/
│   ├── docker-impl-build-optimization/
│   ├── docker-impl-compose-workflows/
│   ├── docker-impl-cicd/
│   ├── docker-impl-production/
│   ├── docker-impl-storage/
│   └── docker-impl-go-templates/
├── docker-errors/
│   ├── docker-errors-build/
│   ├── docker-errors-runtime/
│   ├── docker-errors-networking/
│   └── docker-errors-compose/
└── docker-agents/
    ├── docker-agents-review/
    └── docker-agents-generator/
```
