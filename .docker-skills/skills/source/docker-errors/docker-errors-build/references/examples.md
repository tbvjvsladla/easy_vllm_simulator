# docker-errors-build: Error Reproduction and Fix Examples

## 1. COPY File Not Found -- .dockerignore Conflict

### Reproducing the Error

**Project structure:**
```
myapp/
  src/
    app.js
  config.json
  Dockerfile
  .dockerignore
```

**.dockerignore:**
```
*.json
```

**Dockerfile:**
```dockerfile
FROM node:20-alpine
WORKDIR /app
COPY config.json .
COPY src/ ./src/
CMD ["node", "src/app.js"]
```

**Error:**
```
ERROR: failed to solve: failed to compute cache key: failed to calculate checksum
of ref moby::randomhash: "/config.json": not found
```

### Fix

**.dockerignore (corrected):**
```
*.json
!config.json
!package.json
!package-lock.json
```

The `!` prefix creates an exception to the exclusion pattern.

---

## 2. COPY File Not Found -- Wrong Build Context

### Reproducing the Error

**Project structure:**
```
project/
  docker/
    Dockerfile
  src/
    app.py
  requirements.txt
```

**Build command (wrong):**
```bash
cd project/docker
docker build .
```

**Dockerfile:**
```dockerfile
FROM python:3.12-slim
WORKDIR /app
COPY requirements.txt .
RUN pip install -r requirements.txt
COPY src/ ./src/
CMD ["python", "src/app.py"]
```

**Error:**
```
ERROR: failed to solve: failed to compute cache key: "/requirements.txt": not found
```

### Fix

**Build command (correct):**
```bash
cd project
docker build -f docker/Dockerfile .
```

The build context is `.` (project root), while the Dockerfile is at `docker/Dockerfile`. COPY paths are ALWAYS relative to the build context, not the Dockerfile.

---

## 3. Cache Invalidation -- Wrong Layer Order

### Reproducing the Error

**Dockerfile (inefficient):**
```dockerfile
FROM node:20-alpine
WORKDIR /app
COPY . .
RUN npm ci --production
CMD ["node", "server.js"]
```

**Symptom:** Every change to ANY file (even a comment in `server.js`) triggers a full `npm ci`, downloading all dependencies from scratch.

### Fix

**Dockerfile (optimized):**
```dockerfile
FROM node:20-alpine
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --production
COPY . .
CMD ["node", "server.js"]
```

**Why it works:** `package.json` and `package-lock.json` change infrequently. By copying them first and running `npm ci`, the dependency layer is cached. Source code changes (the second `COPY . .`) only invalidate layers below the second COPY.

---

## 4. Stale apt Packages -- Separated update and install

### Reproducing the Error

**Dockerfile (broken):**
```dockerfile
FROM ubuntu:22.04
RUN apt-get update
RUN apt-get install -y curl
# Later addition:
RUN apt-get install -y nginx
```

**Symptom:** After initial build succeeds, adding `nginx` later may fail with:
```
E: Unable to locate package nginx
```

**Cause:** The `apt-get update` layer is cached. The package index is stale.

### Fix

**Dockerfile (correct):**
```dockerfile
FROM ubuntu:22.04
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    nginx \
    && rm -rf /var/lib/apt/lists/*
```

**Rules:**
- ALWAYS combine `apt-get update` and `apt-get install` in a single RUN
- ALWAYS use `--no-install-recommends` to avoid unnecessary packages
- ALWAYS clean up with `rm -rf /var/lib/apt/lists/*`

---

## 5. BuildKit Mount Error -- Missing Syntax Directive

### Reproducing the Error

**Dockerfile (broken):**
```dockerfile
FROM python:3.12-slim
WORKDIR /app
COPY requirements.txt .
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install -r requirements.txt
COPY . .
CMD ["python", "app.py"]
```

**Error:**
```
ERROR: failed to create LLB definition: rpc error: code = Unknown
desc = unknown flag: --mount
```

### Fix

**Dockerfile (correct):**
```dockerfile
# syntax=docker/dockerfile:1
FROM python:3.12-slim
WORKDIR /app
COPY requirements.txt .
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install -r requirements.txt
COPY . .
CMD ["python", "app.py"]
```

The `# syntax=docker/dockerfile:1` directive MUST be the FIRST line. It enables BuildKit-specific features including `--mount`.

---

## 6. Secret Not Found -- Missing Build Flag

### Reproducing the Error

**Dockerfile:**
```dockerfile
# syntax=docker/dockerfile:1
FROM alpine:3.21
RUN --mount=type=secret,id=api_key \
    cat /run/secrets/api_key > /dev/null
```

**Build command (wrong):**
```bash
docker build .
```

**Error:**
```
ERROR: failed to solve: could not find secret "api_key"
```

### Fix

**Build command (correct):**
```bash
docker build --secret id=api_key,src=./api_key.txt .
```

**For environment variable secrets:**
```bash
export API_KEY=my-secret-value
docker build --secret id=API_KEY .
```

**Dockerfile for env var approach:**
```dockerfile
# syntax=docker/dockerfile:1
FROM alpine:3.21
RUN --mount=type=secret,id=API_KEY,env=API_KEY \
    echo "Secret is available as $API_KEY"
```

---

## 7. Multi-Stage Reference Error -- Case Sensitivity

### Reproducing the Error

**Dockerfile (broken):**
```dockerfile
FROM golang:1.22 AS Builder
WORKDIR /src
COPY . .
RUN go build -o /app

FROM alpine:3.21
COPY --from=Builder /app /usr/bin/app
CMD ["/usr/bin/app"]
```

**Error:**
```
ERROR: failed to solve: invalid from flag value "Builder": invalid reference format
```

### Fix

**Dockerfile (correct):**
```dockerfile
FROM golang:1.22 AS builder
WORKDIR /src
COPY . .
RUN go build -o /app

FROM alpine:3.21
COPY --from=builder /app /usr/bin/app
CMD ["/usr/bin/app"]
```

Stage names MUST be lowercase. Use only lowercase letters, digits, and hyphens.

---

## 8. ARG Scope Reset After FROM

### Reproducing the Error

**Dockerfile (broken):**
```dockerfile
ARG APP_VERSION=1.0.0
FROM alpine:3.21
RUN echo "Version: $APP_VERSION" > /version
```

**Result:** `/version` contains `Version: ` (empty variable).

### Fix

**Dockerfile (correct):**
```dockerfile
ARG APP_VERSION=1.0.0
FROM alpine:3.21
ARG APP_VERSION
RUN echo "Version: $APP_VERSION" > /version
```

**Explanation:** ARG values declared before FROM are available in FROM expressions but MUST be re-declared (without default value) inside each stage that needs them. The re-declared ARG inherits the value from the outer scope.

---

## 9. Platform Mismatch -- Building on Apple Silicon for Linux/amd64

### Reproducing the Error

**Build on M1/M2 Mac (arm64):**
```bash
docker build -t myapp .
```

**Running on amd64 server:**
```
standard_init_linux.go:228: exec user process caused: exec format error
```

### Fix

**Option 1: Specify platform at build time:**
```bash
docker build --platform linux/amd64 -t myapp .
```

**Option 2: Multi-platform build:**
```bash
docker buildx create --use
docker buildx build --platform linux/amd64,linux/arm64 -t myapp --push .
```

**Option 3: Specify in Dockerfile for cross-compilation (Go example):**
```dockerfile
# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM golang:1.22-alpine AS build
ARG TARGETOS TARGETARCH
WORKDIR /src
COPY . .
RUN GOOS=$TARGETOS GOARCH=$TARGETARCH go build -o /app

FROM alpine:3.21
COPY --from=build /app /usr/bin/app
CMD ["/usr/bin/app"]
```

This builds the Go binary using the host's native architecture (fast, no emulation) but cross-compiles for the target platform.

---

## 10. Permission Error -- USER Before Package Install

### Reproducing the Error

**Dockerfile (broken):**
```dockerfile
FROM node:20-alpine
RUN addgroup -S app && adduser -S app -G app
USER app
WORKDIR /app
COPY package*.json ./
RUN npm ci --production
COPY . .
CMD ["node", "server.js"]
```

**Error:**
```
npm ERR! Error: EACCES: permission denied, mkdir '/app/node_modules'
```

### Fix

**Dockerfile (correct):**
```dockerfile
FROM node:20-alpine
RUN addgroup -S app && adduser -S app -G app
WORKDIR /app
COPY --chown=app:app package*.json ./
RUN npm ci --production
COPY --chown=app:app . .
USER app
CMD ["node", "server.js"]
```

**Key principle:** Install packages and set up the application BEFORE switching to the non-root USER. Use `COPY --chown` to ensure files are owned by the correct user.

---

## 11. Exec Format Error -- CRLF Line Endings in Scripts

### Reproducing the Error

**entrypoint.sh (created on Windows with CRLF endings):**
```bash
#!/bin/bash
echo "Starting app"
exec "$@"
```

**Dockerfile:**
```dockerfile
FROM alpine:3.21
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh
ENTRYPOINT ["/entrypoint.sh"]
CMD ["echo", "hello"]
```

**Error:**
```
standard_init_linux.go:228: exec user process caused: no such file or directory
```

**Cause:** The `\r\n` (CRLF) line endings make the kernel unable to find the interpreter (`/bin/bash\r` does not exist).

### Fix

**Option 1: Convert in Dockerfile:**
```dockerfile
FROM alpine:3.21
RUN apk add --no-cache dos2unix
COPY entrypoint.sh /entrypoint.sh
RUN dos2unix /entrypoint.sh && chmod +x /entrypoint.sh
ENTRYPOINT ["/entrypoint.sh"]
CMD ["echo", "hello"]
```

**Option 2: Fix at source (preferred):**
```bash
# Convert before building
dos2unix entrypoint.sh

# Or configure Git to auto-convert
echo "*.sh text eol=lf" >> .gitattributes
```

**Option 3: Use heredoc to create the script inline (avoids line ending issues entirely):**
```dockerfile
# syntax=docker/dockerfile:1
FROM alpine:3.21
COPY <<'EOF' /entrypoint.sh
#!/bin/sh
set -e
echo "Starting app"
exec "$@"
EOF
RUN chmod +x /entrypoint.sh
ENTRYPOINT ["/entrypoint.sh"]
CMD ["echo", "hello"]
```

---

## 12. Build Context Too Large -- Missing .dockerignore

### Reproducing the Error

**Project structure:**
```
myapp/
  node_modules/     (500 MB)
  .git/             (200 MB)
  src/              (2 MB)
  Dockerfile
```

**Build output:**
```
Sending build context to Docker daemon  702.3MB
```

Build takes several minutes just to transfer context.

### Fix

**Create `.dockerignore`:**
```
node_modules
.git
dist
build
*.log
.env
.env.*
.vscode
.idea
```

**Result:**
```
Sending build context to Docker daemon  2.1MB
```

Build context transfer drops from minutes to under a second.

---

## 13. ENV Persistence Leak

### Reproducing the Error

**Dockerfile (broken):**
```dockerfile
FROM alpine:3.21
ENV SECRET_TOKEN=abc123
RUN some-build-command --token=$SECRET_TOKEN
RUN unset SECRET_TOKEN
```

**Problem:** `SECRET_TOKEN` is STILL in the final image:
```bash
docker inspect --format='{{range .Config.Env}}{{println .}}{{end}}' myimage
# Output includes: SECRET_TOKEN=abc123
```

### Fix

**Option 1: Use ARG instead of ENV (if only needed at build time):**
```dockerfile
FROM alpine:3.21
ARG SECRET_TOKEN
RUN some-build-command --token=$SECRET_TOKEN
```

**Option 2: Use secret mount (preferred for actual secrets):**
```dockerfile
# syntax=docker/dockerfile:1
FROM alpine:3.21
RUN --mount=type=secret,id=token,env=SECRET_TOKEN \
    some-build-command --token=$SECRET_TOKEN
```

**Option 3: Use inline export (if ENV is needed temporarily):**
```dockerfile
FROM alpine:3.21
RUN export SECRET_TOKEN=abc123 \
    && some-build-command --token=$SECRET_TOKEN \
    && unset SECRET_TOKEN
```

NEVER use ENV for secrets. ENV values persist in the final image and are visible via `docker inspect` and `docker history`.
