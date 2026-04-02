# Generation Anti-Patterns

Mistakes to avoid when generating Dockerfiles and Compose configurations. Each anti-pattern includes the wrong approach, why it fails, and the correct alternative.

---

## Dockerfile Anti-Patterns

### AP-001: Single-Stage Dockerfile

**Wrong:**
```dockerfile
FROM node:22
WORKDIR /app
COPY . .
RUN npm ci
RUN npm run build
CMD ["node", "dist/index.js"]
```

**Why it fails:** The final image contains the full Node.js SDK, all dev dependencies, source code, and build tools. Image size can exceed 1 GB.

**Correct:** ALWAYS use multi-stage builds. Build in one stage, copy only artifacts to a minimal runtime stage.

---

### AP-002: Running as Root

**Wrong:**
```dockerfile
FROM python:3.12-slim
WORKDIR /app
COPY . .
RUN pip install -r requirements.txt
CMD ["python", "main.py"]
```

**Why it fails:** Container runs as root by default. A compromised application has full root access inside the container, and potentially to mounted volumes.

**Correct:** ALWAYS create a non-root user and switch to it before CMD/ENTRYPOINT.

---

### AP-003: No Health Check

**Wrong:**
```dockerfile
FROM node:22-slim
WORKDIR /app
COPY . .
CMD ["node", "index.js"]
```

**Why it fails:** Docker and Compose cannot determine if the application is actually healthy. `depends_on: condition: service_healthy` will not work. Orchestrators cannot detect unresponsive containers.

**Correct:** ALWAYS include a HEALTHCHECK instruction that tests the application's actual health endpoint.

---

### AP-004: Copying Everything Before Installing Dependencies

**Wrong:**
```dockerfile
FROM node:22-slim AS build
WORKDIR /src
COPY . .
RUN npm ci
RUN npm run build
```

**Why it fails:** Any source code change invalidates the `npm ci` cache. Dependencies are reinstalled on every build, even when `package.json` has not changed.

**Correct:** ALWAYS copy dependency manifests first, install dependencies, then copy source code.

```dockerfile
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build
```

---

### AP-005: No Cache Mounts

**Wrong:**
```dockerfile
RUN pip install -r requirements.txt
```

**Why it fails:** Package manager cache is discarded after each build. Rebuilds download all packages from scratch every time.

**Correct:** ALWAYS use `--mount=type=cache` for package manager caches.

```dockerfile
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install -r requirements.txt
```

---

### AP-006: Using latest Tag

**Wrong:**
```dockerfile
FROM node:latest
FROM python:latest
FROM golang:latest
```

**Why it fails:** Non-deterministic builds. The base image changes silently, potentially breaking the application. Two developers building the same Dockerfile may get different results.

**Correct:** ALWAYS pin to a specific version tag.

```dockerfile
FROM node:22-bookworm-slim
FROM python:3.12-slim-bookworm
FROM golang:1.22-alpine
```

---

### AP-007: Secrets in ENV or ARG

**Wrong:**
```dockerfile
ENV API_KEY=sk-1234567890
ARG DATABASE_PASSWORD=secret123
RUN some-command --password=$DATABASE_PASSWORD
```

**Why it fails:** ENV values persist in the final image and are visible via `docker inspect`. ARG values are visible in `docker history`. Both are baked into image layers permanently.

**Correct:** ALWAYS use secret mounts for sensitive data.

```dockerfile
RUN --mount=type=secret,id=api_key \
    cat /run/secrets/api_key | some-command
```

---

### AP-008: Shell-Form ENTRYPOINT

**Wrong:**
```dockerfile
ENTRYPOINT /usr/bin/myapp --config /etc/config.yaml
```

**Why it fails:** Shell form wraps the command in `/bin/sh -c`, making `/bin/sh` PID 1 instead of the application. SIGTERM is not forwarded to the application, preventing graceful shutdown.

**Correct:** ALWAYS use exec form for ENTRYPOINT.

```dockerfile
ENTRYPOINT ["/usr/bin/myapp"]
CMD ["--config", "/etc/config.yaml"]
```

---

### AP-009: Missing .dockerignore

**Wrong:** No `.dockerignore` file in the project.

**Why it fails:** The entire project directory is sent as build context, including `node_modules/` (500+ MB), `.git/` (entire history), test data, IDE configs, and potentially secret files.

**Correct:** ALWAYS generate a `.dockerignore` file with language-appropriate patterns.

---

### AP-010: Using ADD Instead of COPY

**Wrong:**
```dockerfile
ADD app.js /app/
ADD config.json /app/
```

**Why it fails:** ADD has implicit behaviors (auto-extracting tarballs, downloading URLs) that make the Dockerfile less predictable.

**Correct:** ALWAYS use COPY for local file operations. Only use ADD when you specifically need tar extraction, URL download, or Git clone.

---

### AP-011: Missing syntax Directive

**Wrong:**
```dockerfile
FROM node:22-slim AS build
RUN --mount=type=cache,target=/root/.npm npm ci
```

**Why it fails:** Without `# syntax=docker/dockerfile:1`, BuildKit features like `--mount`, heredocs, and `--chmod` on COPY may not be available or may behave inconsistently.

**Correct:** ALWAYS include `# syntax=docker/dockerfile:1` as the very first line.

---

### AP-012: Separate apt-get update and install

**Wrong:**
```dockerfile
RUN apt-get update
RUN apt-get install -y curl
```

**Why it fails:** The `apt-get update` layer gets cached. When adding new packages later, the cached update layer may reference stale package indexes, causing install failures.

**Correct:** ALWAYS combine update and install in one RUN, and clean up after.

```dockerfile
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*
```

---

## Compose Anti-Patterns

### AP-013: Using version: Field

**Wrong:**
```yaml
version: "3.8"
services:
  web:
    image: nginx
```

**Why it fails:** The `version` field is deprecated and ignored by modern Compose. It adds confusion and provides no benefit.

**Correct:** NEVER include the `version:` field.

---

### AP-014: depends_on Without Health Check

**Wrong:**
```yaml
services:
  app:
    depends_on:
      - db
  db:
    image: postgres
```

**Why it fails:** Without a health check, `depends_on` only waits for the container to start, not for PostgreSQL to be ready to accept connections. The application may crash on startup.

**Correct:** ALWAYS use `condition: service_healthy` and define health checks.

```yaml
services:
  app:
    depends_on:
      db:
        condition: service_healthy
  db:
    image: postgres
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5
```

---

### AP-015: Hardcoded Secrets in Compose

**Wrong:**
```yaml
services:
  db:
    environment:
      POSTGRES_PASSWORD: "my-super-secret-password"
```

**Why it fails:** Secrets are committed to version control in plain text. Anyone with repository access can read them.

**Correct:** ALWAYS use `env_file` with `.env` (gitignored) or variable interpolation with `${VAR:?error}`.

```yaml
services:
  db:
    env_file: .env
    environment:
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:?Database password is required}
```

---

### AP-016: Anonymous Volumes

**Wrong:**
```yaml
services:
  db:
    image: postgres
    volumes:
      - /var/lib/postgresql/data
```

**Why it fails:** Anonymous volumes are recreated on `docker compose down`. All database data is lost.

**Correct:** ALWAYS use named volumes for persistent data.

```yaml
services:
  db:
    volumes:
      - db-data:/var/lib/postgresql/data
volumes:
  db-data:
```

---

### AP-017: No Resource Limits in Production

**Wrong:**
```yaml
services:
  app:
    restart: always
```

**Why it fails:** A crashing container with `restart: always` and no resource limits can consume all system CPU and memory in a restart loop, affecting all other containers on the host.

**Correct:** ALWAYS set resource limits in production configurations.

```yaml
services:
  app:
    restart: unless-stopped
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 512M
```

---

### AP-018: Exposing Ports to All Interfaces

**Wrong:**
```yaml
ports:
  - "5432:5432"
  - "6379:6379"
```

**Why it fails:** Database and cache ports are exposed to all network interfaces, making them accessible from outside the host. This is a significant security risk.

**Correct:** ALWAYS bind development ports to `127.0.0.1`. In production, do NOT expose database ports at all.

```yaml
# Development only
ports:
  - "127.0.0.1:5432:5432"
```

---

### AP-019: Using container_name for Scalable Services

**Wrong:**
```yaml
services:
  web:
    image: nginx
    container_name: my-nginx
```

**Why it fails:** Container names must be unique. Setting `container_name` prevents `docker compose up --scale web=3`.

**Correct:** NEVER use `container_name` for services that may need scaling. Let Compose manage container names.

---

### AP-020: No Log Rotation

**Wrong:**
```yaml
services:
  app:
    image: myapp
    restart: always
```

**Why it fails:** Default json-file logging has no size limit. A busy application can fill the disk with log data, crashing the entire host.

**Correct:** ALWAYS configure log rotation in production.

```yaml
services:
  app:
    logging:
      driver: json-file
      options:
        max-size: "10m"
        max-file: "3"
```

---

## Summary Checklist

Before delivering generated Docker infrastructure, verify NONE of these anti-patterns are present:

| ID | Anti-Pattern | Check |
|----|-------------|-------|
| AP-001 | Single-stage Dockerfile | Multi-stage build present? |
| AP-002 | Running as root | Non-root USER set? |
| AP-003 | No health check | HEALTHCHECK instruction present? |
| AP-004 | COPY before dependencies | Manifests copied first? |
| AP-005 | No cache mounts | --mount=type=cache used? |
| AP-006 | latest tag | Pinned version tags? |
| AP-007 | Secrets in ENV/ARG | No secrets in layers? |
| AP-008 | Shell-form ENTRYPOINT | Exec form used? |
| AP-009 | Missing .dockerignore | .dockerignore exists? |
| AP-010 | ADD instead of COPY | COPY used for local files? |
| AP-011 | Missing syntax directive | First line is # syntax=...? |
| AP-012 | Separate apt update/install | Combined in one RUN? |
| AP-013 | version: field | No version: in Compose? |
| AP-014 | depends_on without health | condition: service_healthy? |
| AP-015 | Hardcoded secrets | env_file or interpolation? |
| AP-016 | Anonymous volumes | Named volumes declared? |
| AP-017 | No resource limits | deploy.resources set? |
| AP-018 | Ports on all interfaces | 127.0.0.1 binding in dev? |
| AP-019 | container_name on scalable | No container_name? |
| AP-020 | No log rotation | Logging configured? |
