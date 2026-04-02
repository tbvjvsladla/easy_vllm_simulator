---
name: docker-impl-production
description: >
  Use when hardening Dockerfiles for production deployment or choosing
  base images. Prevents containers running as root, missing HEALTHCHECK
  definitions, and zombie processes from shell-form ENTRYPOINT.
  Covers base image selection, USER, HEALTHCHECK, exec form, init,
  entrypoint scripts, OCI labels, digest pinning, and distroless images.
  Keywords: USER, HEALTHCHECK, ENTRYPOINT, scratch, alpine, distroless,
  tini, exec form, production ready, deploy container, secure container,
  optimize for production, smaller image, harden Dockerfile.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-impl-production

## Quick Reference

### Base Image Selection

| Image Type | Size | Shell | Package Mgr | Use Case |
|-----------|------|-------|-------------|----------|
| `scratch` | 0 MB | No | No | Statically compiled binaries (Go, Rust) |
| `alpine` | ~6 MB | Yes (ash) | apk | Minimal Linux with package management |
| `*-slim` | ~30-80 MB | Yes (bash) | apt | Reduced Debian without extras |
| `distroless` | ~20 MB | No | No | Google's minimal runtime images |
| Full (e.g., `ubuntu`) | ~75-200 MB | Yes (bash) | apt | Development, debugging, complex dependencies |

ALWAYS use a full image for the build stage and a minimal image for the runtime stage.

See [references/base-images.md](references/base-images.md) for detailed comparison with pros, cons, and language-specific recommendations.

### Production Checklist

| Requirement | Implementation | Priority |
|------------|---------------|----------|
| Non-root user | `USER` instruction with explicit UID/GID | MUST |
| Signal handling | Exec form ENTRYPOINT + `exec "$@"` in scripts | MUST |
| Health check | `HEALTHCHECK` instruction | MUST |
| Pinned base image | Tag + digest (`image:tag@sha256:...`) | MUST |
| OCI labels | `LABEL org.opencontainers.image.*` | SHOULD |
| Minimal attack surface | Multi-stage build, no shell in final image if possible | SHOULD |
| Read-only filesystem | `--read-only` flag at runtime | SHOULD |
| No secrets in layers | `--mount=type=secret` for build-time secrets | MUST |

### Critical Warnings

**NEVER** use shell form for ENTRYPOINT in production -- the application runs under `/bin/sh -c` and does NOT receive signals. ALWAYS use exec form: `ENTRYPOINT ["executable"]`.

**NEVER** run production containers as root -- a compromised process with root inside the container can escalate to host root. ALWAYS add a `USER` instruction.

**NEVER** use the `latest` tag in production -- builds become non-reproducible and may break without warning. ALWAYS pin to a specific version tag and digest.

**NEVER** store secrets in ENV, ARG, or COPY layers -- they persist in image history and can be extracted. ALWAYS use `--mount=type=secret` during build and runtime secrets management (Docker secrets, env injection).

**NEVER** install debugging tools (curl, wget, vim, strace) in production images -- they increase attack surface. Keep them in a separate debug stage built with `--target debug`.

---

## Non-Root USER Configuration

### Standard Pattern (Debian/Ubuntu)

```dockerfile
RUN groupadd -r -g 1001 appuser && \
    useradd --no-log-init -r -u 1001 -g appuser appuser
USER 1001:1001
```

ALWAYS assign explicit UID/GID for deterministic behavior across rebuilds.
ALWAYS use `--no-log-init` to prevent `/var/log/faillog` from filling with NULL characters.
ALWAYS reference UID/GID numbers in the USER instruction for clarity in `ps` and log output.

### Alpine Pattern

```dockerfile
RUN addgroup -S -g 1001 appuser && \
    adduser -S -u 1001 -G appuser -h /app appuser
USER 1001:1001
```

### Distroless Pattern

Distroless images include a `nonroot` user (UID 65534):

```dockerfile
FROM gcr.io/distroless/static-debian12:nonroot
```

No `RUN` needed -- the user is pre-configured.

### File Ownership

```dockerfile
COPY --chown=1001:1001 --from=build /app/binary /usr/bin/app
WORKDIR /app
RUN chown -R 1001:1001 /app
USER 1001:1001
```

ALWAYS set file ownership BEFORE switching to the non-root user.

---

## Signal Handling

### The PID 1 Problem

The first process in a container (PID 1) receives all signals. If PID 1 is a shell (`/bin/sh`), it does NOT forward signals to child processes. The application never receives SIGTERM and cannot shut down gracefully -- Docker kills it after the timeout (default 10s).

### Exec Form (Required)

```dockerfile
# CORRECT: app is PID 1, receives SIGTERM directly
ENTRYPOINT ["/usr/bin/app"]

# WRONG: /bin/sh is PID 1, app never receives signals
ENTRYPOINT /usr/bin/app
```

### Init Process (--init / tini / dumb-init)

When your application spawns child processes, use an init process to reap zombies and forward signals:

```dockerfile
# Option 1: Docker --init flag (uses tini)
# docker run --init myimage

# Option 2: Tini embedded in image
RUN apk add --no-cache tini
ENTRYPOINT ["/sbin/tini", "--"]
CMD ["/usr/bin/app"]

# Option 3: dumb-init
COPY --from=build /usr/bin/dumb-init /usr/bin/dumb-init
ENTRYPOINT ["/usr/bin/dumb-init", "--"]
CMD ["/usr/bin/app"]
```

ALWAYS use an init process when the application forks child processes.
NEVER rely on the default Docker behavior for zombie reaping -- PID 1 must handle SIGCHLD.

### Custom STOPSIGNAL

```dockerfile
# Default is SIGTERM; override if your app uses a different signal
STOPSIGNAL SIGQUIT   # e.g., Nginx uses SIGQUIT for graceful shutdown
```

---

## HEALTHCHECK Patterns

### HTTP Health Check

```dockerfile
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1
```

### TCP Health Check (No HTTP)

```dockerfile
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD nc -z localhost 5432 || exit 1
```

### File-Based Health Check (No Network)

```dockerfile
HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
  CMD test -f /tmp/healthy || exit 1
```

### Minimal Image Health Check (No curl/wget)

For distroless or scratch images, compile a static health check binary:

```dockerfile
FROM golang:1.22 AS healthcheck
WORKDIR /src
COPY <<'EOF' main.go
package main
import ("net/http"; "os")
func main() {
    _, err := http.Get("http://localhost:8080/health")
    if err != nil { os.Exit(1) }
}
EOF
RUN CGO_ENABLED=0 go build -o /healthcheck main.go

FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=build /app /app
COPY --from=healthcheck /healthcheck /healthcheck
HEALTHCHECK --interval=30s --timeout=3s --retries=3 CMD ["/healthcheck"]
```

ALWAYS set `--start-period` to allow time for application initialization.
ALWAYS use `|| exit 1` with shell-form health checks -- the exit code determines health status.
NEVER use `curl` in health checks for production images -- it adds unnecessary attack surface. Use `wget` (included in alpine) or a compiled binary.

---

## Entrypoint Scripts

### Standard Pattern

```dockerfile
COPY --chmod=755 docker-entrypoint.sh /usr/local/bin/
ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["app", "--serve"]
```

```bash
#!/bin/sh
set -e

# Pre-flight: run migrations, wait for dependencies, etc.
if [ "$1" = 'app' ]; then
    echo "Running database migrations..."
    /usr/bin/app migrate
fi

# CRITICAL: exec replaces shell with app, making app PID 1
exec "$@"
```

ALWAYS end entrypoint scripts with `exec "$@"` -- this replaces the shell process with the application, ensuring proper signal handling.
ALWAYS use `set -e` to exit on any error during initialization.
NEVER use `#!/bin/bash` unless bash features are required -- prefer `#!/bin/sh` for portability and smaller images.

### Wait-for-Dependencies Pattern

```bash
#!/bin/sh
set -e

# Wait for database
until nc -z "$DB_HOST" "$DB_PORT" 2>/dev/null; do
    echo "Waiting for database at $DB_HOST:$DB_PORT..."
    sleep 1
done

exec "$@"
```

---

## OCI Metadata Labels

```dockerfile
LABEL org.opencontainers.image.title="My Application" \
      org.opencontainers.image.description="Production API server" \
      org.opencontainers.image.version="1.2.3" \
      org.opencontainers.image.authors="team@example.com" \
      org.opencontainers.image.url="https://example.com" \
      org.opencontainers.image.source="https://github.com/org/repo" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.created="2024-01-15T10:30:00Z" \
      org.opencontainers.image.revision="abc123def"
```

ALWAYS use OCI standard keys (`org.opencontainers.image.*`) -- they are recognized by registries, scanners, and orchestrators.
ALWAYS inject `created` and `revision` via build args for accuracy:

```dockerfile
ARG BUILD_DATE
ARG VCS_REF
LABEL org.opencontainers.image.created="${BUILD_DATE}" \
      org.opencontainers.image.revision="${VCS_REF}"
```

```bash
docker build \
  --build-arg BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ") \
  --build-arg VCS_REF=$(git rev-parse --short HEAD) .
```

---

## Reproducible Builds

### Digest Pinning

```dockerfile
# Tag alone is mutable -- the same tag can point to different images
FROM node:20-slim

# Tag + digest is immutable -- guarantees exact same image
FROM node:20-slim@sha256:4b19478e60dfe3a05c3ca13d822e40c45a3cdc633b4c63da8ef0ac2c01feee84
```

ALWAYS pin production base images by digest. Tags are mutable pointers -- a registry push can change what `node:20-slim` resolves to.

### Get the Current Digest

```bash
docker pull node:20-slim
docker inspect --format='{{index .RepoDigests 0}}' node:20-slim
```

### Pin Package Versions

```dockerfile
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl=7.88.1-10+deb12u5 \
    && rm -rf /var/lib/apt/lists/*
```

### Reproducible Timestamps

```bash
docker build --build-arg SOURCE_DATE_EPOCH=0 .
```

---

## Production Dockerfile Template

```dockerfile
# syntax=docker/dockerfile:1

# ---- Build Stage ----
FROM node:20-bookworm AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci --production=false
COPY . .
RUN npm run build

# ---- Runtime Stage ----
FROM node:20-bookworm-slim@sha256:<pin-digest-here> AS runtime

# OCI metadata
LABEL org.opencontainers.image.title="My App" \
      org.opencontainers.image.version="1.0.0" \
      org.opencontainers.image.licenses="MIT"

# Non-root user
RUN groupadd -r -g 1001 appuser && \
    useradd --no-log-init -r -u 1001 -g appuser appuser

WORKDIR /app

# Copy build artifacts with correct ownership
COPY --chown=1001:1001 --from=build /app/dist ./dist
COPY --chown=1001:1001 --from=build /app/node_modules ./node_modules
COPY --chown=1001:1001 --from=build /app/package.json ./

# Entrypoint script
COPY --chmod=755 docker-entrypoint.sh /usr/local/bin/

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:3000/health || exit 1

# Switch to non-root
USER 1001:1001

EXPOSE 3000

ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["node", "dist/index.js"]
```

See [references/examples.md](references/examples.md) for production Dockerfiles per language (Go, Python, Node.js, Rust, Java, .NET).

---

## Reference Links

- [references/base-images.md](references/base-images.md) -- Base image comparison with pros, cons, size, and language recommendations
- [references/examples.md](references/examples.md) -- Production Dockerfiles per language, entrypoint scripts, health checks
- [references/anti-patterns.md](references/anti-patterns.md) -- Production deployment mistakes and corrections

### Official Sources

- https://docs.docker.com/build/building/best-practices/
- https://docs.docker.com/reference/dockerfile/
- https://docs.docker.com/build/building/multi-stage/
- https://github.com/GoogleContainerTools/distroless
- https://github.com/opencontainers/image-spec/blob/main/annotations.md
