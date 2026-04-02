---
name: docker-impl-build-optimization
description: >
  Use when optimizing Docker build times or fixing unexpected cache
  invalidation. Prevents full rebuilds from incorrect instruction ordering
  and bloated images from missing .dockerignore entries.
  Covers layer caching rules, cache invalidation triggers, instruction
  ordering, .dockerignore, --mount=type=cache, bind mounts, and CI/CD
  cache backends with BuildKit.
  Keywords: docker build, cache, .dockerignore, BuildKit, --mount=type=cache,
  layer, COPY, RUN, slow build, build takes too long, rebuild every time,
  speed up Docker build, cache not working.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+ with BuildKit."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-impl-build-optimization

## Quick Reference

### Layer Caching Rules

Docker checks each instruction against its cache before executing. If the instruction and its inputs match a cached layer, the cached version is reused.

**Critical rule:** Once ANY layer's cache is invalidated, ALL subsequent layers MUST rebuild.

### Cache Invalidation Triggers

| Instruction | Cache Key | Invalidation Trigger |
|-------------|-----------|---------------------|
| `FROM` | Image reference | Base image tag/digest changed |
| `RUN` | Command string only | Command text changed (NOT external resources) |
| `COPY` | File content checksums | File content changed (mtime is NOT checked) |
| `ADD` | File checksums + URL content | File content or URL content changed |
| `ENV` | Key=Value pair | Value changed |
| `ARG` | Name=Value pair | Value changed |
| `WORKDIR` | Path + `SOURCE_DATE_EPOCH` | Path or epoch changed |

### Critical Warnings

**NEVER** separate `apt-get update` and `apt-get install` into different RUN instructions -- the cached `update` layer becomes stale and subsequent installs may fail or use outdated packages.

**NEVER** use `COPY . .` before dependency installation -- ANY file change invalidates the COPY layer and forces a full reinstall of all dependencies.

**NEVER** rely on RUN cache for external resources -- Docker only checks the command string, not what `apt-get install` or `curl` fetches. Use `--no-cache` or `--no-cache-filter` to force fresh downloads.

**ALWAYS** include a `.dockerignore` file -- without it, the entire build context (including `.git/`, `node_modules/`, test data) is sent to the builder.

**ALWAYS** use `# syntax=docker/dockerfile:1` at the top of every Dockerfile to enable BuildKit cache mounts and other optimizations.

---

## Instruction Ordering Strategy

Order instructions from LEAST frequently changed to MOST frequently changed:

```
+--------------------------------------------------+
| FROM base-image                    (rarely changes) |
+--------------------------------------------------+
| RUN install system packages         (rarely changes) |
+--------------------------------------------------+
| COPY package.json / go.mod / *.csproj (dep changes) |
+--------------------------------------------------+
| RUN install dependencies            (dep changes)   |
+--------------------------------------------------+
| COPY . .                            (every commit)  |
+--------------------------------------------------+
| RUN build application               (every commit)  |
+--------------------------------------------------+
| CMD / ENTRYPOINT                    (rarely changes) |
+--------------------------------------------------+
         CACHE FLOWS TOP-DOWN
   First invalidation breaks ALL below
```

**Principle:** Expensive, slow-changing operations go at the top. Frequently changing source code goes at the bottom.

---

## .dockerignore Template

ALWAYS create a `.dockerignore` in the project root:

```
# Version control
.git
.gitignore
.gitattributes

# Dependencies (rebuilt inside container)
node_modules
vendor
__pycache__
*.pyc
.venv

# Build artifacts
dist
build
target
*.o
*.exe

# IDE and OS files
.vscode
.idea
*.swp
*.swo
.DS_Store
Thumbs.db

# Docker files (not needed in build context)
Dockerfile*
docker-compose*.yml
.dockerignore

# Documentation and non-essential files
*.md
LICENSE
docs/

# Environment and secrets
.env
.env.*
*.pem
*.key
*.cert

# Test and CI files
.github
.gitlab-ci.yml
tests/
coverage/
```

**Negation syntax:** Use `!` to re-include files excluded by a broader pattern:

```
*.md
!README.md
```

---

## Cache Mount Patterns

Cache mounts (`--mount=type=cache`) persist package manager caches across builds. Even when a layer rebuilds, only new or changed packages are downloaded.

### apt-get (Debian/Ubuntu)

```dockerfile
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
    curl git
```

**ALWAYS** use `sharing=locked` for apt -- concurrent access corrupts the cache.

### npm

```dockerfile
COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci
```

### yarn

```dockerfile
COPY package.json yarn.lock ./
RUN --mount=type=cache,target=/usr/local/share/.cache/yarn \
    yarn install --frozen-lockfile
```

### pnpm

```dockerfile
COPY package.json pnpm-lock.yaml ./
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile
```

### pip (Python)

```dockerfile
COPY requirements.txt ./
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install -r requirements.txt
```

### Go modules

```dockerfile
COPY go.mod go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod \
    go mod download

COPY . .
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    go build -o /app/server ./cmd
```

### Cargo (Rust)

```dockerfile
COPY Cargo.toml Cargo.lock ./
RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release
```

### Maven (Java)

```dockerfile
COPY pom.xml ./
RUN --mount=type=cache,target=/root/.m2/repository \
    mvn dependency:resolve

COPY src ./src
RUN --mount=type=cache,target=/root/.m2/repository \
    mvn package -DskipTests
```

### NuGet (.NET)

```dockerfile
COPY *.csproj ./
RUN --mount=type=cache,target=/root/.nuget/packages \
    dotnet restore

COPY . ./
RUN --mount=type=cache,target=/root/.nuget/packages \
    dotnet publish -c Release -o /app
```

---

## Bind Mounts for Large Contexts

When source code is only needed to produce an artifact, use bind mounts instead of COPY to avoid persisting source files in any layer:

```dockerfile
FROM golang:1.22 AS build
WORKDIR /src
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=bind,source=go.sum,target=go.sum \
    --mount=type=bind,source=go.mod,target=go.mod \
    go mod download

RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    --mount=type=bind,target=. \
    go build -o /bin/app ./cmd
```

**Advantages:**
- Mounted files are NOT persisted in any layer
- Only the RUN output is kept in the image
- Avoids bloating the build cache with source files
- Bind mounts are read-only by default (safe)

---

## CI/CD Cache Backends

### Registry Cache (recommended for teams)

```bash
docker buildx build --push -t registry/app:latest \
  --cache-to type=registry,ref=registry/app:buildcache,mode=max \
  --cache-from type=registry,ref=registry/app:buildcache .
```

### GitHub Actions Cache

```yaml
- uses: docker/build-push-action@v7
  with:
    push: true
    tags: user/app:latest
    cache-from: type=gha
    cache-to: type=gha,mode=max
```

### Multi-Branch Cache Strategy

```bash
docker buildx build --push -t registry/app:latest \
  --cache-to type=registry,ref=registry/app:cache:$BRANCH \
  --cache-from type=registry,ref=registry/app:cache:$BRANCH \
  --cache-from type=registry,ref=registry/app:cache:main .
```

ALWAYS fall back to the main branch cache when the feature branch cache misses.

### Cache Modes

| Mode | Behavior | Use Case |
|------|----------|----------|
| `min` (default) | Caches only exported layers | Smaller cache, faster export |
| `max` | Caches ALL layers including intermediates | More cache hits, larger storage |

ALWAYS use `mode=max` in CI/CD to maximize cache reuse across builds.

---

## Layer Squashing Considerations

Docker does NOT support true layer squashing natively. Options:

| Approach | How | Trade-off |
|----------|-----|-----------|
| Multi-stage builds | Copy only final artifacts to clean stage | Best approach -- no extra tooling |
| `--squash` (experimental) | Merge all layers into one | Loses all intermediate cache |
| `docker export/import` | Flatten to single layer | Loses metadata, CMD, ENV, etc. |

**ALWAYS** prefer multi-stage builds over squashing -- they preserve caching while producing minimal final images.

---

## Forcing Cache Invalidation

```bash
# Invalidate ALL cache
docker build --no-cache .

# Invalidate a specific stage only
docker build --no-cache-filter install .

# Pull fresh base images
docker build --pull .

# Clear entire builder cache
docker builder prune

# Clear with size limit
docker builder prune --keep-storage 5GB
```

---

## Reference Links

- [references/caching-rules.md](references/caching-rules.md) -- Complete cache invalidation rules per instruction type
- [references/examples.md](references/examples.md) -- Optimized Dockerfiles before/after, .dockerignore patterns
- [references/anti-patterns.md](references/anti-patterns.md) -- Caching and optimization mistakes with explanations

### Official Sources

- https://docs.docker.com/build/cache/
- https://docs.docker.com/build/cache/invalidation/
- https://docs.docker.com/build/cache/optimize/
- https://docs.docker.com/build/cache/backends/
- https://docs.docker.com/reference/dockerfile/
- https://docs.docker.com/build/building/best-practices/
