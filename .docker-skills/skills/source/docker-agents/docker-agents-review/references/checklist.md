# Docker Review Checklist -- Complete Reference

This checklist expands on the SKILL.md checklists with detailed verification steps, expected states, and common failure modes for each check.

---

## Area 1: Dockerfile

### Base Image Checks

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| D-01 | Base image has specific version tag | `FROM node:20.11-bookworm-slim` | Using `latest` or no tag |
| D-02 | Final stage uses minimal base | alpine, slim, distroless, or scratch | Full OS image (ubuntu, debian) in production |
| D-03 | Multi-stage build separates build from runtime | Build tools only in build stage | Compiler/SDK in final image |
| D-04 | Build stages named with AS | `FROM golang:1.22 AS build` | `COPY --from=0` (numeric index) |
| D-05 | Digest pinning for supply chain security | `FROM alpine@sha256:abc...` | Tag-only reference in CI/CD |

### Instruction Checks

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| D-06 | syntax directive at top | `# syntax=docker/dockerfile:1` | Missing (no BuildKit features) |
| D-07 | COPY over ADD for local files | `COPY config.json /app/` | `ADD config.json /app/` without extraction need |
| D-08 | Combined apt-get update and install | Single RUN with `&&` | Separate RUN layers |
| D-09 | apt cache cleaned | `&& rm -rf /var/lib/apt/lists/*` | Cache left in image (+30-100MB) |
| D-10 | --no-install-recommends | `apt-get install -y --no-install-recommends` | Extra packages installed |
| D-11 | Package versions pinned | `curl=7.88.1-10+deb12u5` | Unversioned `curl` |
| D-12 | Related commands combined | Single RUN per logical operation | One RUN per command |
| D-13 | WORKDIR over RUN cd | `WORKDIR /app` | `RUN cd /app && ...` |
| D-14 | .dockerignore exists | Excludes node_modules, .git, build/ | Missing or incomplete |
| D-15 | COPY ordered for cache | Dependencies first, source code last | `COPY . .` before `npm install` |

### Signal & Process Checks

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| D-16 | ENTRYPOINT exec form | `ENTRYPOINT ["/usr/bin/app"]` | `ENTRYPOINT /usr/bin/app` (shell form) |
| D-17 | CMD exec form | `CMD ["node", "server.js"]` | `CMD node server.js` |
| D-18 | Entrypoint script uses exec | `exec "$@"` at end of script | Shell wraps process (not PID 1) |
| D-19 | Pipe failure safety | `set -o pipefail` before pipes | Pipe failure masked by last command |

### Metadata Checks

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| D-20 | EXPOSE documents ports | `EXPOSE 8080/tcp` | Missing port documentation |
| D-21 | OCI labels present | `LABEL org.opencontainers.image.*` | No metadata or deprecated MAINTAINER |
| D-22 | HEALTHCHECK defined | `HEALTHCHECK CMD curl -f http://localhost/` | No health check in Dockerfile |

---

## Area 2: Compose

### Structure Checks

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| C-01 | No version field | Omitted entirely | `version: "3.8"` present |
| C-02 | Preferred filename | `compose.yaml` | `docker-compose.yml` (legacy) |
| C-03 | Specific image tags | `image: postgres:16` | `image: postgres` or `postgres:latest` |
| C-04 | Project name defined | `name: my-project` | Relies on directory name |

### Dependency Checks

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| C-05 | depends_on with health condition | `condition: service_healthy` | Bare `depends_on: [db]` |
| C-06 | Target has healthcheck | Healthcheck defined on dependency | `service_healthy` without healthcheck on target |
| C-07 | Database healthcheck appropriate | `pg_isready`, `mysqladmin ping`, etc. | Generic or missing healthcheck |

### Volume Checks

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| C-08 | Named volumes for data | `db-data:/var/lib/postgresql/data` | Anonymous volume `/var/lib/postgresql/data` |
| C-09 | Top-level volumes declared | `volumes: { db-data: }` | Named volume used but not declared |
| C-10 | Bind mounts use absolute paths | `/host/path:/container/path` | Relative path without `./` prefix |

### Environment Checks

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| C-11 | No hardcoded secrets | `${DB_PASS:?Required}` | `DB_PASS: "plaintext-secret"` |
| C-12 | env_file for config | `env_file: [.env]` | All values inline in compose.yaml |
| C-13 | Compose secrets for sensitive data | `secrets:` section used | Passwords in environment variables |
| C-14 | Required vars use error syntax | `${VAR:?Error message}` | `${VAR}` silently empty |

### Network Checks

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| C-15 | Dev ports bound to localhost | `127.0.0.1:8080:80` | `8080:80` (all interfaces) |
| C-16 | Internal services use expose | `expose: ["3000"]` | `ports` on internal-only services |
| C-17 | Network isolation | Separate frontend/backend networks | All services on default network |
| C-18 | No container_name on scalable services | Let Compose manage names | `container_name: my-web` |

### Resource Checks

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| C-19 | Resource limits set | `deploy.resources.limits.memory: 512M` | No limits (unbounded) |
| C-20 | Restart with limits | `restart: unless-stopped` + limits | `restart: always` without limits |
| C-21 | Debug services profiled | `profiles: [debug]` | phpmyadmin always enabled |

---

## Area 3: Security

### User Checks

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| S-01 | Non-root USER | `USER appuser` or `USER 1001` | No USER instruction |
| S-02 | Explicit UID/GID | `USER 1001:1001` | `USER appuser` without known UID |
| S-03 | --no-log-init flag | `useradd --no-log-init -r` | faillog fills with NULLs |
| S-04 | gosu over sudo | `exec gosu appuser "$@"` | `sudo -u appuser` in entrypoint |

### Privilege Checks

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| S-05 | No --privileged | Absent from Compose and run commands | `privileged: true` |
| S-06 | Capabilities managed | `cap_drop: [ALL]` + specific adds | Default capabilities (too broad) |
| S-07 | no-new-privileges | `security_opt: [no-new-privileges:true]` | Missing security option |
| S-08 | Read-only root FS | `read_only: true` + tmpfs mounts | Writable root filesystem |

### Secret Checks

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| S-09 | No secrets in ENV/ARG | Build secrets via `--mount=type=secret` | `ENV API_KEY=sk-123` |
| S-10 | Secret mount in build | `RUN --mount=type=secret,id=token` | `COPY secrets.txt /app/` |
| S-11 | Clean docker history | No sensitive data in layer history | ARG with default secret value |

### Image Security Checks

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| S-12 | Vulnerability scan | `docker scout cves` run, no critical | Never scanned |
| S-13 | Official base images | Docker Official Images or verified | Unverified third-party images |
| S-14 | Minimal packages | Only runtime dependencies in final image | curl, vim, build-essential in production |
| S-15 | No build tools in prod | Multi-stage separates build from runtime | gcc, make in final image |

---

## Area 4: Production Readiness

### Health Check Configuration

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| P-01 | HEALTHCHECK defined | Present in Dockerfile or Compose | No health monitoring |
| P-02 | Appropriate intervals | interval=30s, timeout=5s, retries=3 | Default 30s timeout (too long) |
| P-03 | Meaningful test command | Tests actual service endpoint | `CMD true` or `CMD exit 0` |
| P-04 | start_period configured | Allows for startup time | Marked unhealthy during startup |

### Recovery Configuration

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| P-05 | Restart policy set | `unless-stopped` or `on-failure` | `restart: "no"` in production |
| P-06 | Max restart attempts | `on-failure:5` or `max_attempts: 5` | Infinite restart loop |
| P-07 | Resource limits | Memory and CPU limits set | Runaway container consumes all RAM |
| P-08 | Log rotation | `max-size: 10m`, `max-file: 3` | Logs fill disk |

### Observability Configuration

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| P-09 | Logs to stdout/stderr | Application logs to standard streams | Logs to /var/log/app.log inside container |
| P-10 | Log rotation active | Logging driver options set | No rotation (disk fills up) |
| P-11 | Metrics accessible | `docker stats` shows useful data | No resource monitoring |

### Build Configuration

| # | Check | Expected State | Common Failure |
|---|-------|---------------|----------------|
| P-12 | Pinned base images | Specific version tag or digest | `latest` tag in CI/CD |
| P-13 | Pinned packages | Version constraints on critical packages | `apt-get install curl` (any version) |
| P-14 | Cache strategy | `cache-from` / `cache-to` configured | Full rebuild every CI run |
| P-15 | .dockerignore complete | Excludes .git, node_modules, docs, tests | Sends GB of context to builder |
| P-16 | Multi-platform ready | `--platform` configured if needed | amd64-only in mixed environments |
