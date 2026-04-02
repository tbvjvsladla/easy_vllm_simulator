# docker-errors-build: Build Configuration Anti-Patterns

## Overview

This reference documents build configuration mistakes that cause errors, slow builds, or security vulnerabilities. Each anti-pattern includes the problem, why it fails, and the correct approach.

---

## 1. No .dockerignore File

**Problem:**
```
# No .dockerignore exists
docker build .
# "Sending build context to Docker daemon  1.2GB"
```

**Why it fails:** Without `.dockerignore`, Docker sends the ENTIRE directory tree to the daemon. This includes `node_modules/` (500MB+), `.git/` (full history), IDE configs, test data, and secrets like `.env` files.

**Consequences:**
- Build takes minutes just to transfer context
- COPY instructions may accidentally include sensitive files
- Build cache invalidated by irrelevant file changes

**Fix:** ALWAYS create a `.dockerignore` file:
```
.git
node_modules
dist
build
*.log
.env
.env.*
.vscode
.idea
__pycache__
*.pyc
.DS_Store
Thumbs.db
```

---

## 2. COPY . . Before Dependency Installation

**Problem:**
```dockerfile
FROM node:20-alpine
WORKDIR /app
COPY . .
RUN npm ci
CMD ["node", "server.js"]
```

**Why it fails:** ANY file change (even editing a comment) invalidates the `COPY . .` layer, which cascades to invalidate `npm ci`. Dependencies are reinstalled on EVERY build.

**Fix:**
```dockerfile
FROM node:20-alpine
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
CMD ["node", "server.js"]
```

**Rule:** ALWAYS copy dependency manifests first, install dependencies, THEN copy source code.

---

## 3. Separating apt-get update and install

**Problem:**
```dockerfile
RUN apt-get update
RUN apt-get install -y curl
```

**Why it fails:** The `apt-get update` layer gets cached. When you later add packages, Docker uses the cached (stale) package index. Installation may fail with "Unable to locate package" or install outdated versions with known vulnerabilities.

**Fix:**
```dockerfile
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*
```

**Rule:** ALWAYS combine `apt-get update` with `apt-get install` in a SINGLE RUN instruction.

---

## 4. Not Cleaning Package Manager Cache

**Problem:**
```dockerfile
RUN apt-get update && apt-get install -y curl git wget
```

**Why it fails:** The apt cache (`/var/lib/apt/lists/`) remains in the layer, adding 30-100MB of unnecessary data to every image.

**Fix:**
```dockerfile
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    git \
    wget \
    && rm -rf /var/lib/apt/lists/*
```

**For Alpine:**
```dockerfile
RUN apk add --no-cache curl git wget
```

The `--no-cache` flag for `apk` avoids storing the index locally.

---

## 5. Using latest Tag for Base Images

**Problem:**
```dockerfile
FROM node:latest
```

**Why it fails:** `latest` is a moving target. The same Dockerfile produces different images on different days. Builds are non-reproducible, and a base image update can silently break your application.

**Fix:**
```dockerfile
# Good: Pin major.minor version
FROM node:20.11-bookworm-slim

# Best: Pin to digest for full reproducibility
FROM node:20.11-bookworm-slim@sha256:abc123...
```

**Rule:** NEVER use `:latest` in production Dockerfiles. ALWAYS pin to a specific version.

---

## 6. Secrets in ENV or ARG

**Problem:**
```dockerfile
ENV API_KEY=sk-1234567890abcdef
ARG DATABASE_PASSWORD=supersecret
RUN connect-to-db --password=$DATABASE_PASSWORD
```

**Why it fails:**
- ENV values persist in the final image, visible via `docker inspect`
- ARG values appear in `docker history`
- Both are stored in image layers and can be extracted

**Fix:**
```dockerfile
# syntax=docker/dockerfile:1
FROM alpine:3.21
RUN --mount=type=secret,id=api_key,env=API_KEY \
    --mount=type=secret,id=db_pass,env=DATABASE_PASSWORD \
    connect-to-db --password=$DATABASE_PASSWORD
```

```bash
docker build \
    --secret id=api_key,src=./api_key.txt \
    --secret id=db_pass,src=./db_pass.txt .
```

**Rule:** NEVER use ENV or ARG for secrets. ALWAYS use `--mount=type=secret`.

---

## 7. Running as Root

**Problem:**
```dockerfile
FROM node:20
WORKDIR /app
COPY . .
RUN npm ci
CMD ["node", "server.js"]
```

**Why it fails:** Container runs as root by default. If the application is compromised, the attacker has root privileges inside the container, which can facilitate container escape attacks.

**Fix:**
```dockerfile
FROM node:20
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY --chown=node:node . .
USER node
CMD ["node", "server.js"]
```

**Rule:** ALWAYS switch to a non-root USER before CMD/ENTRYPOINT.

---

## 8. Using ADD When COPY Suffices

**Problem:**
```dockerfile
ADD config.json /app/config.json
ADD src/ /app/src/
```

**Why it fails:** ADD has implicit behaviors that COPY does not:
- Auto-extracts tar archives (may unpack unexpectedly)
- Can download URLs (unexpected network calls)
- Less predictable than COPY

**Fix:**
```dockerfile
COPY config.json /app/config.json
COPY src/ /app/src/
```

**Rule:** ALWAYS use COPY for local files. Use ADD ONLY when you specifically need tar extraction, URL download, or Git clone.

---

## 9. Too Many Layers

**Problem:**
```dockerfile
RUN apt-get update
RUN apt-get install -y curl
RUN apt-get install -y git
RUN apt-get install -y nginx
RUN rm -rf /var/lib/apt/lists/*
```

**Why it fails:**
- Each RUN creates a separate layer
- The `rm` in the last layer does NOT reduce image size -- files are still in earlier layers
- More layers means larger images and slower pulls

**Fix:**
```dockerfile
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    git \
    nginx \
    && rm -rf /var/lib/apt/lists/*
```

**Rule:** Combine related operations (especially install + cleanup) into a SINGLE RUN layer.

---

## 10. Shell Form ENTRYPOINT

**Problem:**
```dockerfile
ENTRYPOINT /usr/bin/myapp --config /etc/app.conf
```

**Why it fails:** Shell form wraps the command in `/bin/sh -c`, making the shell PID 1 instead of the application. Consequences:
- `docker stop` sends SIGTERM to the shell, not the app
- Application does not shut down gracefully
- Zombie processes can accumulate

**Fix:**
```dockerfile
ENTRYPOINT ["/usr/bin/myapp", "--config", "/etc/app.conf"]
```

**For entrypoint scripts:** End with `exec "$@"` to replace the shell process:
```bash
#!/bin/sh
set -e
# setup logic here
exec "$@"
```

**Rule:** ALWAYS use exec form `["executable", "args"]` for ENTRYPOINT.

---

## 11. Using cd Instead of WORKDIR

**Problem:**
```dockerfile
RUN cd /app && npm install
RUN cd /app && npm build
```

**Why it fails:** `cd` only affects the current RUN layer's shell. The next RUN starts from `/` again. Each `cd` must be repeated, which is fragile and error-prone.

**Fix:**
```dockerfile
WORKDIR /app
RUN npm install
RUN npm build
```

**Rule:** ALWAYS use WORKDIR to set the working directory. NEVER use `cd` in RUN.

---

## 12. Not Using Multi-Stage Builds

**Problem:**
```dockerfile
FROM golang:1.22
WORKDIR /src
COPY . .
RUN go build -o /app
CMD ["/app"]
```

**Why it fails:** The final image includes the entire Go toolchain (~800MB), source code, and build artifacts. The actual binary is only a few MB.

**Fix:**
```dockerfile
FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN CGO_ENABLED=0 go build -o /app

FROM alpine:3.21
COPY --from=build /app /usr/bin/app
USER nobody:nobody
CMD ["/usr/bin/app"]
```

**Result:** Image drops from ~800MB to ~10MB.

**Rule:** ALWAYS use multi-stage builds for compiled languages. Separate build tools from runtime.

---

## 13. Ignoring .dockerignore for Secrets

**Problem:**
```
# .dockerignore does NOT exclude:
.env
.env.local
credentials.json
*.pem
```

**Why it fails:** Without explicit exclusion, `COPY . .` includes secrets in the image. Even if they are later deleted, they persist in earlier layers and can be extracted.

**Fix:**
```
# .dockerignore must include:
.env
.env.*
*.pem
*.key
credentials.json
secrets/
```

**Rule:** ALWAYS exclude secret files in `.dockerignore`. NEVER rely on deleting secrets in a later layer.

---

## 14. Not Pinning Package Versions

**Problem:**
```dockerfile
RUN apt-get update && apt-get install -y curl git
RUN pip install flask requests
```

**Why it fails:** Package versions change over time. The same Dockerfile may install different versions on different days, leading to:
- Non-reproducible builds
- Unexpected breaking changes
- Security audit difficulty

**Fix:**
```dockerfile
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl=7.88.1-10+deb12u5 \
    git=1:2.39.2-1.1 \
    && rm -rf /var/lib/apt/lists/*

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt
```

**requirements.txt:**
```
flask==3.0.2
requests==2.31.0
```

**Rule:** ALWAYS pin package versions in production Dockerfiles. Use lockfiles (package-lock.json, poetry.lock, go.sum) where available.

---

## 15. Missing Syntax Directive for BuildKit Features

**Problem:**
```dockerfile
FROM python:3.12-slim
RUN --mount=type=cache,target=/root/.cache/pip pip install flask
```

**Why it fails:** Without `# syntax=docker/dockerfile:1`, the builder does not recognize `--mount` and other BuildKit extensions.

**Fix:**
```dockerfile
# syntax=docker/dockerfile:1
FROM python:3.12-slim
RUN --mount=type=cache,target=/root/.cache/pip pip install flask
```

**Rule:** ALWAYS include `# syntax=docker/dockerfile:1` as the FIRST line when using ANY BuildKit feature (--mount, --chmod on COPY, --link, heredocs).

---

## 16. Numeric Stage References

**Problem:**
```dockerfile
FROM golang:1.22
WORKDIR /src
COPY . .
RUN go build -o /app

FROM alpine:3.21
COPY --from=0 /app /usr/bin/app
```

**Why it fails:** Numeric references (`--from=0`) break when stages are added, removed, or reordered. This is fragile and hard to maintain.

**Fix:**
```dockerfile
FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN go build -o /app

FROM alpine:3.21
COPY --from=build /app /usr/bin/app
```

**Rule:** ALWAYS name stages with `AS`. NEVER use numeric indexes in `--from`.

---

## 17. ENV for Build-Only Variables

**Problem:**
```dockerfile
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*
```

**Why it fails:** `DEBIAN_FRONTEND=noninteractive` persists in the final image as an environment variable, which can cause unexpected behavior for interactive tools run inside the container.

**Fix:**
```dockerfile
ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*
```

Or inline:
```dockerfile
RUN DEBIAN_FRONTEND=noninteractive apt-get update && \
    apt-get install -y curl && \
    rm -rf /var/lib/apt/lists/*
```

**Rule:** Use ARG or inline variables for build-only values. ONLY use ENV for variables needed at container runtime.

---

## Summary Table

| # | Anti-Pattern | Primary Consequence | Severity |
|---|-------------|---------------------|----------|
| 1 | No .dockerignore | Slow builds, secret leaks | High |
| 2 | COPY . . before deps | Full rebuild on every change | High |
| 3 | Separate apt update/install | Stale/broken packages | High |
| 4 | No cache cleanup | Bloated images (+30-100MB) | Medium |
| 5 | Using :latest tag | Non-reproducible builds | High |
| 6 | Secrets in ENV/ARG | Credential exposure | Critical |
| 7 | Running as root | Container escape risk | High |
| 8 | ADD instead of COPY | Unpredictable behavior | Low |
| 9 | Too many layers | Bloated images | Medium |
| 10 | Shell form ENTRYPOINT | Signal handling broken | High |
| 11 | cd instead of WORKDIR | Fragile, error-prone | Low |
| 12 | No multi-stage builds | Bloated images (+hundreds MB) | High |
| 13 | Secrets not in .dockerignore | Credential exposure | Critical |
| 14 | Unpinned package versions | Non-reproducible builds | Medium |
| 15 | Missing syntax directive | BuildKit features fail | Medium |
| 16 | Numeric stage references | Fragile multi-stage builds | Low |
| 17 | ENV for build-only vars | Variable pollution in image | Low |
