---
name: docker-agents-review
description: >
  Use when reviewing a Dockerfile or Compose file before merging,
  deploying, or auditing container security posture.
  Prevents shipping containers that run as root, lack health checks,
  expose secrets in layers, or use mutable tags like latest.
  Covers Dockerfile best practices, Compose validation, security audit,
  production readiness checks, anti-pattern detection.
  Keywords: Dockerfile review, docker compose config, HEALTHCHECK,
  USER, --no-cache, .dockerignore, capabilities, secrets, latest tag,
  is my Dockerfile secure, check my container, audit container, best practices.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+ and Docker Compose v2."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-agents-review

## Review Workflow

Execute these checklists in order. Each section is independent -- report all findings, do not stop at the first issue.

```
Review Order:
1. Dockerfile Validation     --> Base image, instructions, layers, signals
2. Compose Validation        --> Structure, depends_on, volumes, env
3. Security Audit            --> Non-root, capabilities, secrets, scanning
4. Production Readiness      --> Health checks, restart, logging, limits
5. Anti-Pattern Scan         --> Known mistakes across all areas
```

---

## Checklist 1: Dockerfile Validation

### 1A: Base Image

```
[ ] Base image uses a specific version tag (NEVER `latest`)
    FAIL: FROM node:latest | FROM nginx
    PASS: FROM node:20.11-bookworm-slim
[ ] Production stage uses minimal base (alpine, slim, distroless, or scratch)
    FAIL: Full OS image in final stage (ubuntu:22.04, debian:bookworm)
[ ] Multi-stage build separates build dependencies from runtime
    FAIL: Compiler/SDK present in final image
[ ] Build stage named with AS keyword
    FAIL: COPY --from=0 (fragile numeric index)
    PASS: FROM golang:1.22 AS build ... COPY --from=build
[ ] For reproducible builds, digest pinning used where required
    BEST: FROM alpine:3.21@sha256:abc123...
```

### 1B: Instructions & Layer Optimization

```
[ ] syntax directive present at top of Dockerfile
    EXPECTED: # syntax=docker/dockerfile:1
[ ] COPY used instead of ADD for local files
    FAIL: ADD config.json /app/ (when no extraction needed)
[ ] apt-get update && install combined in single RUN
    FAIL: Separate RUN apt-get update and RUN apt-get install
[ ] apt cache cleaned after install
    EXPECTED: && rm -rf /var/lib/apt/lists/*
[ ] --no-install-recommends used with apt-get install
[ ] Package versions pinned where determinism required
[ ] Related commands combined with && to minimize layers
    FAIL: Separate RUN for each apt-get install package
[ ] WORKDIR used instead of RUN cd
    FAIL: RUN cd /app && npm install
[ ] .dockerignore file exists and excludes node_modules, .git, build artifacts
[ ] COPY instruction ordered for cache efficiency
    EXPECTED: COPY package*.json first, then RUN npm install, then COPY . .
```

### 1C: Entrypoint & Signals

```
[ ] ENTRYPOINT uses exec form (JSON array)
    FAIL: ENTRYPOINT /usr/bin/myapp (shell form)
    PASS: ENTRYPOINT ["/usr/bin/myapp"]
[ ] CMD uses exec form (JSON array)
    FAIL: CMD node server.js
    PASS: CMD ["node", "server.js"]
[ ] Entrypoint scripts use exec "$@" to replace shell process
[ ] Application runs as PID 1 for proper signal handling
[ ] Shell form NOT used for ENTRYPOINT (prevents SIGTERM delivery)
[ ] Pipe commands use set -o pipefail
    FAIL: RUN wget -O - https://url | wc -l > /count
    PASS: RUN set -o pipefail && wget -O - https://url | wc -l > /count
```

### 1D: Metadata & Documentation

```
[ ] EXPOSE documents all listening ports
[ ] OCI standard labels present (org.opencontainers.image.*)
[ ] HEALTHCHECK defined in Dockerfile
[ ] No deprecated MAINTAINER instruction (use LABEL instead)
```

---

## Checklist 2: Compose Validation

### 2A: File Structure

```
[ ] No `version:` field (deprecated and ignored by modern Compose)
    FAIL: version: "3.8"
[ ] File named compose.yaml (preferred) or docker-compose.yml
[ ] Services use specific image tags (NEVER latest)
[ ] Project name defined via `name:` field or CLI flag
```

### 2B: Service Dependencies

```
[ ] depends_on uses condition: service_healthy (not bare dependency)
    FAIL: depends_on: [db] (only waits for start, not ready)
    PASS: depends_on: { db: { condition: service_healthy } }
[ ] Services with depends_on: service_healthy have healthcheck defined
    FAIL: condition: service_healthy without healthcheck on target
[ ] Database services have appropriate healthcheck
    EXAMPLE: test: ["CMD-SHELL", "pg_isready -U postgres"]
```

### 2C: Volumes & Storage

```
[ ] Named volumes used for persistent data (NEVER anonymous)
    FAIL: volumes: [/var/lib/postgresql/data]
    PASS: volumes: [db-data:/var/lib/postgresql/data]
[ ] Named volumes declared in top-level volumes: section
[ ] Bind mounts use absolute paths or named volumes
[ ] Database data directories use named volumes
```

### 2D: Environment & Secrets

```
[ ] Secrets NOT hardcoded in environment values
    FAIL: DATABASE_PASSWORD: "my-secret"
    PASS: DATABASE_PASSWORD: ${DATABASE_PASSWORD:?Required}
[ ] env_file used for environment-specific configuration
[ ] Sensitive values use Compose secrets (not environment)
[ ] Variable interpolation uses error syntax for required vars
    EXPECTED: ${VAR:?Error message}
```

### 2E: Networking & Ports

```
[ ] Development ports bound to localhost
    FAIL: ports: ["8080:80"]
    PASS: ports: ["127.0.0.1:8080:80"]
[ ] Services that do not need external access use expose (not ports)
[ ] Network isolation implemented where services should not communicate
    EXAMPLE: frontend/backend network separation
[ ] container_name NOT used on scalable services
    FAIL: container_name: my-nginx (prevents scaling)
```

### 2F: Resource Management

```
[ ] Resource limits defined (deploy.resources.limits)
    EXPECTED: cpus and memory limits set
[ ] restart policy appropriate (unless-stopped or on-failure, NOT always without limits)
    FAIL: restart: always without deploy.resources.limits
[ ] Optional/debug services use profiles
    FAIL: phpmyadmin always running in production
    PASS: phpmyadmin with profiles: [debug]
```

---

## Checklist 3: Security Audit

### 3A: Container User

```
[ ] Dockerfile creates and switches to non-root USER
    FAIL: No USER instruction (runs as root)
    PASS: RUN groupadd -r app && useradd -r -g app app ... USER app
[ ] USER instruction uses explicit UID/GID for determinism
[ ] --no-log-init flag used with useradd
[ ] gosu used instead of sudo in entrypoint scripts
```

### 3B: Capabilities & Privileges

```
[ ] --privileged NOT used in production
    FAIL: privileged: true in compose.yaml
[ ] Capabilities dropped and only needed ones added
    BEST: cap_drop: [ALL] + cap_add: [NET_BIND_SERVICE]
[ ] no-new-privileges security option set
    EXPECTED: security_opt: [no-new-privileges:true]
[ ] Read-only root filesystem where possible
    EXPECTED: read_only: true with tmpfs for /tmp, /run
```

### 3C: Secrets & Sensitive Data

```
[ ] No secrets in ENV or ARG instructions
    FAIL: ENV API_KEY=sk-123 or ARG DATABASE_PASSWORD=secret
[ ] Build secrets use --mount=type=secret
    PASS: RUN --mount=type=secret,id=token cat /run/secrets/token
[ ] No secrets committed to .dockerignore-excluded files
[ ] No sensitive data in docker history (check with docker history)
```

### 3D: Image Security

```
[ ] Images scanned for vulnerabilities (docker scout cves)
[ ] Base images from official/trusted sources
[ ] No unnecessary packages installed in final image
[ ] Build tools NOT present in production image (use multi-stage)
```

### 3E: Network Security

```
[ ] Internal services NOT exposed to host network
[ ] Ports bound to specific interfaces where possible
[ ] Network isolation between frontend and backend tiers
[ ] No host network mode without justification
```

---

## Checklist 4: Production Readiness

### 4A: Health Checks

```
[ ] HEALTHCHECK defined in Dockerfile or Compose healthcheck
[ ] Health check interval, timeout, retries configured
    EXPECTED: interval=30s, timeout=5s, start_period=10s, retries=3
[ ] Health check command tests actual service readiness
    FAIL: CMD true (always passes)
    PASS: CMD curl -f http://localhost/health || exit 1
[ ] start_period allows for application startup time
```

### 4B: Restart & Recovery

```
[ ] Restart policy set (unless-stopped or on-failure)
[ ] on-failure has max_attempts limit to prevent crash loops
[ ] Resource limits prevent runaway container resource consumption
[ ] Logging configured with size rotation
    EXPECTED: logging driver with max-size and max-file options
```

### 4C: Observability

```
[ ] Structured logging to stdout/stderr (NOT to files inside container)
[ ] Log rotation configured (max-size, max-file)
    FAIL: No log rotation (fills disk)
    PASS: logging: { options: { max-size: "10m", max-file: "3" } }
[ ] Container metrics accessible via docker stats
[ ] Health status queryable via docker inspect
```

### 4D: Build Reproducibility

```
[ ] Base images pinned to specific version (or digest)
[ ] Package versions pinned where critical
[ ] Build cache strategy defined (cache-from, cache-to)
[ ] .dockerignore prevents build context bloat
[ ] Multi-platform build configured if needed
```

---

## Checklist 5: Anti-Pattern Scan

Scan the codebase for these known issues. See [references/anti-patterns.md](references/anti-patterns.md) for full details.

### Dockerfile Anti-Patterns

```
[ ] No FROM with latest tag
[ ] No shell form ENTRYPOINT
[ ] No separate apt-get update and install
[ ] No missing apt cache cleanup
[ ] No ADD when COPY suffices
[ ] No RUN cd instead of WORKDIR
[ ] No secrets in ENV or ARG
[ ] No ENV persistence leak (unset in same RUN)
[ ] No missing .dockerignore
[ ] No running as root without justification
[ ] No too-many-layers (combine related RUN)
[ ] No missing syntax directive
```

### Compose Anti-Patterns

```
[ ] No version: field present
[ ] No depends_on without healthcheck condition
[ ] No anonymous volumes for persistent data
[ ] No hardcoded secrets in environment
[ ] No container_name on scalable services
[ ] No restart: always without resource limits
[ ] No ports exposed to all interfaces (0.0.0.0)
[ ] No debug services without profiles
```

### Security Anti-Patterns

```
[ ] No --privileged in production
[ ] No running as root without USER instruction
[ ] No host network without justification
[ ] No missing capability drops
[ ] No secrets baked into image layers
```

---

## Decision Trees

### Is the Dockerfile production-ready?

```
Does it use multi-stage build?
+-- No --> Add build + runtime stages
+-- Yes
    |
    Does the final stage use a minimal base?
    +-- No --> Switch to alpine/slim/distroless
    +-- Yes
        |
        Does it run as non-root?
        +-- No --> CRITICAL: Add USER instruction
        +-- Yes
            |
            Does it have a HEALTHCHECK?
            +-- No --> Add health check
            +-- Yes
                |
                Does it use exec form for ENTRYPOINT/CMD?
                +-- No --> Convert to JSON array format
                +-- Yes --> PASS
```

### Is the Compose file production-ready?

```
Does it have a version: field?
+-- Yes --> Remove it (deprecated)
+-- No
    |
    Do all depends_on use service_healthy?
    +-- No --> Add healthchecks and conditions
    +-- Yes
        |
        Are all persistent volumes named?
        +-- No --> CRITICAL: Replace anonymous volumes
        +-- Yes
            |
            Are resource limits set?
            +-- No --> Add deploy.resources.limits
            +-- Yes
                |
                Are secrets properly managed?
                +-- No --> Move to Compose secrets or env_file
                +-- Yes --> PASS
```

---

## Review Report Template

After completing all checklists, produce a report:

```
## Docker Configuration Review Report

### Summary
- Total issues found: X
- Critical (blocks deployment): X
- Warning (should fix): X
- Info (improvement suggestion): X

### Critical Issues
1. [CRIT-001] Description -- Location -- Fix

### Warnings
1. [WARN-001] Description -- Location -- Fix

### Passed Checks
- Dockerfile Validation: PASS/FAIL (X/Y checks passed)
- Compose Validation: PASS/FAIL
- Security Audit: PASS/FAIL
- Production Readiness: PASS/FAIL
- Anti-Pattern Scan: PASS/FAIL
```

---

## Reference Links

- [references/checklist.md](references/checklist.md) -- Complete validation checklist organized by area
- [references/examples.md](references/examples.md) -- Review scenarios with good and bad examples
- [references/anti-patterns.md](references/anti-patterns.md) -- All anti-patterns consolidated from research

### Official Sources

- https://docs.docker.com/build/building/best-practices/
- https://docs.docker.com/compose/compose-file/
- https://docs.docker.com/engine/security/
- https://docs.docker.com/reference/dockerfile/
- https://docs.docker.com/scout/
