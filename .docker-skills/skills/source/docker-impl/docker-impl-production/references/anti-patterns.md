# Production Anti-Patterns

## AP-01: Running as Root

**Problem:** Containers run as root by default. A container escape vulnerability combined with root gives the attacker root on the host.

```dockerfile
# BAD: No USER instruction -- container runs as root
FROM node:20-slim
WORKDIR /app
COPY . .
CMD ["node", "index.js"]
```

**Fix:**

```dockerfile
FROM node:20-slim
RUN groupadd -r -g 1001 appuser && \
    useradd --no-log-init -r -u 1001 -g appuser appuser
WORKDIR /app
COPY --chown=1001:1001 . .
USER 1001:1001
CMD ["node", "index.js"]
```

NEVER run production containers as root. ALWAYS add a USER instruction with explicit UID/GID.

---

## AP-02: Shell Form ENTRYPOINT

**Problem:** Shell form wraps the command in `/bin/sh -c`, making the shell PID 1 instead of the application. SIGTERM goes to the shell, not the app. The app never shuts down gracefully -- Docker kills it after the stop timeout.

```dockerfile
# BAD: /bin/sh is PID 1, app never receives SIGTERM
ENTRYPOINT /usr/bin/myapp --serve
```

**Fix:**

```dockerfile
# GOOD: app is PID 1, receives all signals
ENTRYPOINT ["/usr/bin/myapp", "--serve"]
```

ALWAYS use exec form for ENTRYPOINT and CMD in production.

---

## AP-03: No HEALTHCHECK

**Problem:** Without HEALTHCHECK, Docker and orchestrators cannot distinguish between a running container and a healthy one. A deadlocked application that consumes no CPU appears "running" but serves no requests.

```dockerfile
# BAD: No health check -- Docker only knows if the process is running
FROM node:20-slim
CMD ["node", "server.js"]
```

**Fix:**

```dockerfile
FROM node:20-slim
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:3000/health || exit 1
CMD ["node", "server.js"]
```

ALWAYS include a HEALTHCHECK for production services.

---

## AP-04: Using `latest` Tag

**Problem:** The `latest` tag is a mutable pointer. It can change between builds, making deployments non-reproducible. A working build today may fail tomorrow because the base image changed.

```dockerfile
# BAD: Non-deterministic
FROM python:latest
```

**Fix:**

```dockerfile
# GOOD: Pinned tag
FROM python:3.12-slim-bookworm

# BEST: Pinned tag + digest
FROM python:3.12-slim-bookworm@sha256:abc123...
```

NEVER use `latest` in production Dockerfiles. ALWAYS pin version tags. Pin digests for critical production workloads.

---

## AP-05: Secrets in Image Layers

**Problem:** ENV, ARG, and COPY instructions persist in image layers and `docker history`. Anyone who pulls the image can extract secrets.

```dockerfile
# BAD: Secret visible in docker history
ENV API_KEY=sk-production-secret-key
ARG DB_PASSWORD=supersecret
COPY credentials.json /app/
```

**Fix:**

```dockerfile
# GOOD: Build-time secrets via mount (not persisted)
RUN --mount=type=secret,id=api_key,env=API_KEY \
    some-command-that-needs-api-key

# GOOD: Runtime secrets via orchestrator
# docker run -e API_KEY_FILE=/run/secrets/api_key myapp
# Or: docker service create --secret api_key myapp
```

NEVER put secrets in ENV, ARG, or COPY. ALWAYS use `--mount=type=secret` for build-time and runtime secret management for run-time.

---

## AP-06: Installing Unnecessary Packages

**Problem:** Every package increases image size, attack surface, and CVE exposure. Debugging tools like curl, wget, vim, and strace have no place in production.

```dockerfile
# BAD: Build tools and debug utilities in production image
FROM python:3.12-slim
RUN apt-get update && apt-get install -y \
    build-essential \
    curl \
    vim \
    strace \
    net-tools \
    && rm -rf /var/lib/apt/lists/*
```

**Fix:**

```dockerfile
# GOOD: Multi-stage -- build tools stay in build stage
FROM python:3.12 AS build
RUN apt-get update && apt-get install -y build-essential
COPY requirements.txt .
RUN pip install --prefix=/install -r requirements.txt

FROM python:3.12-slim
COPY --from=build /install /usr/local
COPY . /app
```

ALWAYS use multi-stage builds to keep build tools out of the runtime image. If debugging tools are needed, create a separate debug stage with `--target debug`.

---

## AP-07: Missing Entrypoint Script exec

**Problem:** Entrypoint scripts that forget `exec "$@"` leave the shell as PID 1. The application runs as a child of the shell, receiving no signals.

```bash
# BAD: Shell remains PID 1, application is a child process
#!/bin/sh
echo "Starting..."
/usr/bin/myapp --serve
# Shell stays alive as PID 1, myapp is a child
```

**Fix:**

```bash
# GOOD: exec replaces shell with application
#!/bin/sh
set -e
echo "Starting..."
exec "$@"
```

ALWAYS end entrypoint scripts with `exec "$@"`. This replaces the shell process with the application, making it PID 1.

---

## AP-08: Single-Stage Production Build

**Problem:** Building and running in a single stage includes compilers, build tools, source code, and intermediate artifacts in the production image.

```dockerfile
# BAD: 850MB image with Go compiler, source code, test files
FROM golang:1.22
WORKDIR /app
COPY . .
RUN go build -o server
CMD ["./server"]
```

**Fix:**

```dockerfile
# GOOD: 15MB image with only the binary
FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN CGO_ENABLED=0 go build -ldflags="-s -w" -o /server

FROM alpine:3.21
COPY --from=build /server /usr/bin/server
USER 65534:65534
ENTRYPOINT ["/usr/bin/server"]
```

ALWAYS use multi-stage builds for production. The runtime image should contain ONLY the application binary and its runtime dependencies.

---

## AP-09: Not Setting start-period on HEALTHCHECK

**Problem:** Without `--start-period`, health checks run immediately. If the application takes 15 seconds to start, the first few checks fail and Docker may mark the container as unhealthy before it is ready.

```dockerfile
# BAD: No start period -- fails during startup
HEALTHCHECK --interval=5s --retries=3 CMD curl -f http://localhost/ || exit 1
```

**Fix:**

```dockerfile
# GOOD: 30s grace period for Java/heavy apps
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1
```

ALWAYS set `--start-period` to at least the application's expected startup time.

---

## AP-10: Writable Root Filesystem

**Problem:** A writable root filesystem allows attackers to modify binaries, install tools, or write malicious scripts. If the application does not need to write to the filesystem, the root filesystem should be read-only.

```bash
# BAD: Default writable filesystem
docker run myapp

# GOOD: Read-only root filesystem
docker run --read-only --tmpfs /tmp myapp
```

```yaml
# docker-compose.yml
services:
  app:
    image: myapp
    read_only: true
    tmpfs:
      - /tmp
    volumes:
      - app-data:/app/data  # Only mount writable where needed
```

ALWAYS run production containers with `--read-only` when possible. Use `tmpfs` for temporary files and named volumes for persistent data.

---

## AP-11: No Resource Limits

**Problem:** Without resource limits, a container can consume all host memory or CPU, affecting other containers and the host itself.

```yaml
# BAD: No limits
services:
  app:
    image: myapp

# GOOD: Explicit limits
services:
  app:
    image: myapp
    deploy:
      resources:
        limits:
          cpus: "2.0"
          memory: 512M
        reservations:
          cpus: "0.5"
          memory: 256M
```

ALWAYS set memory and CPU limits in production. ALWAYS set reservations to guarantee minimum resources.

---

## AP-12: Using VOLUME in Production Dockerfiles

**Problem:** The VOLUME instruction creates anonymous volumes that are hard to manage and can lead to data loss. It also prevents changes to the specified directory in subsequent Dockerfile layers.

```dockerfile
# BAD: Anonymous volume, cannot be easily backed up or managed
FROM postgres:16
VOLUME /var/lib/postgresql/data
```

**Fix:** Define volumes in `docker-compose.yml` or `docker run`, not in the Dockerfile:

```yaml
services:
  db:
    image: postgres:16
    volumes:
      - pgdata:/var/lib/postgresql/data

volumes:
  pgdata:
```

NEVER use VOLUME in application Dockerfiles. ALWAYS define volumes at the orchestration layer (compose or run command).

---

## AP-13: Ignoring Multi-Platform Builds

**Problem:** Building only for `linux/amd64` means the image will not run natively on ARM servers (Graviton, Apple Silicon dev machines), requiring slow emulation.

```bash
# BAD: Only builds for the host platform
docker build -t myapp .
```

**Fix:**

```dockerfile
FROM --platform=$BUILDPLATFORM golang:1.22 AS build
ARG TARGETOS TARGETARCH
RUN GOOS=$TARGETOS GOARCH=$TARGETARCH go build -o /app

FROM alpine:3.21
COPY --from=build /app /app
```

```bash
docker buildx build --platform linux/amd64,linux/arm64 -t myapp .
```

ALWAYS consider multi-platform builds if your application runs on diverse infrastructure (cloud, edge, developer machines).

---

## AP-14: No Graceful Shutdown Handling

**Problem:** The application does not handle SIGTERM. When Docker stops the container, in-flight requests are dropped and database connections are not closed cleanly.

```javascript
// BAD: No signal handling
const server = app.listen(3000);
```

**Fix:**

```javascript
// GOOD: Graceful shutdown
const server = app.listen(3000);

process.on('SIGTERM', () => {
  console.log('SIGTERM received, shutting down gracefully...');
  server.close(() => {
    console.log('Server closed.');
    process.exit(0);
  });
  // Force shutdown after timeout
  setTimeout(() => process.exit(1), 10000);
});
```

ALWAYS implement SIGTERM handling in the application. ALWAYS close connections and finish in-flight requests before exiting.

---

## Summary Table

| # | Anti-Pattern | Risk | Fix |
|---|-------------|------|-----|
| AP-01 | Running as root | Host compromise | `USER` instruction |
| AP-02 | Shell form ENTRYPOINT | No graceful shutdown | Exec form `["..."]` |
| AP-03 | No HEALTHCHECK | Silent failures | Add HEALTHCHECK |
| AP-04 | Using `latest` tag | Non-reproducible | Pin version + digest |
| AP-05 | Secrets in layers | Credential leak | `--mount=type=secret` |
| AP-06 | Unnecessary packages | Large attack surface | Multi-stage builds |
| AP-07 | Missing exec in entrypoint | Signal handling broken | `exec "$@"` |
| AP-08 | Single-stage build | Bloated image | Multi-stage |
| AP-09 | No start-period | False unhealthy | Set `--start-period` |
| AP-10 | Writable root FS | Tampering risk | `--read-only` |
| AP-11 | No resource limits | Resource exhaustion | Set limits + reservations |
| AP-12 | VOLUME in Dockerfile | Unmanaged data | Orchestration-level volumes |
| AP-13 | Single-platform only | ARM incompatibility | Multi-platform builds |
| AP-14 | No graceful shutdown | Dropped requests | Handle SIGTERM |
