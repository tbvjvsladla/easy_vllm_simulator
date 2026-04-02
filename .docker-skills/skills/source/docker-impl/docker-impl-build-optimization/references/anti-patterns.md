# Build Optimization Anti-Patterns

> Reference file for docker-impl-build-optimization.
> Each anti-pattern includes what goes wrong and the correct approach.

---

## AP-001: COPY Everything Before Installing Dependencies

**The mistake:**
```dockerfile
FROM node:20
WORKDIR /app
COPY . .              # Copies ALL source files
RUN npm install       # Reinstalls on EVERY source change
```

**Why it fails:** `COPY . .` creates a cache key based on ALL files in the build context. Any change to any file -- even editing a comment in a source file -- invalidates the COPY layer and forces a complete `npm install` from scratch.

**The fix:**
```dockerfile
FROM node:20
WORKDIR /app
COPY package.json package-lock.json ./   # Only dependency files
RUN npm ci                                # Cached until deps change
COPY . .                                  # Source changes only affect this layer
```

---

## AP-002: Separate apt-get update and install

**The mistake:**
```dockerfile
RUN apt-get update
RUN apt-get install -y curl
```

**Why it fails:** The `apt-get update` layer is cached based on the command string. Days later, when you add `nginx` to the install line, Docker reuses the stale `apt-get update` cache. The package index is outdated, and `apt-get install` may fail or install old versions.

**The fix:**
```dockerfile
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    nginx \
    && rm -rf /var/lib/apt/lists/*
```

ALWAYS combine update and install in a single RUN.

---

## AP-003: Missing .dockerignore

**The mistake:** No `.dockerignore` file in the project.

**Why it fails:** The entire project directory becomes the build context, including:
- `node_modules/` (often 500MB+)
- `.git/` (entire repository history, can be gigabytes)
- Test data, documentation, IDE configs
- `.env` files with secrets

This slows down every build because the entire context must be sent to the Docker daemon. It also causes unnecessary cache invalidation -- any file change in any ignored directory triggers `COPY . .` to rebuild.

**The fix:** ALWAYS create a `.dockerignore` file. See the SKILL.md template for a comprehensive starter.

---

## AP-004: Not Using Cache Mounts

**The mistake:**
```dockerfile
COPY requirements.txt ./
RUN pip install -r requirements.txt
```

**Why it fails:** When the requirements change, pip downloads ALL packages from scratch, even those already downloaded in a previous build. Without a cache mount, the pip download cache is discarded with the old layer.

**The fix:**
```dockerfile
COPY requirements.txt ./
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install -r requirements.txt
```

The cache mount persists across builds. Even when the RUN layer rebuilds, pip reuses previously downloaded packages and only fetches what changed.

---

## AP-005: Expecting RUN Cache to Detect External Changes

**The mistake:**
```dockerfile
RUN curl -sL https://example.com/latest-release.tar.gz | tar xz
```

**Why it fails:** Docker caches the RUN layer based ONLY on the command string. If the URL content changes (new release uploaded), Docker still uses the cached layer because the command text is identical. The build silently uses stale content.

**The fix:**
```bash
# Option 1: Use --no-cache for the specific stage
docker build --no-cache-filter download .

# Option 2: Use a build arg as cache buster
ARG RELEASE_VERSION=1.0.0
RUN curl -sL "https://example.com/release-${RELEASE_VERSION}.tar.gz" | tar xz

# Option 3: Force full rebuild
docker build --no-cache .
```

---

## AP-006: Cache Mounts Without Correct Sharing Mode for apt

**The mistake:**
```dockerfile
RUN --mount=type=cache,target=/var/cache/apt \
    --mount=type=cache,target=/var/lib/apt \
    apt-get update && apt-get install -y curl
```

**Why it fails:** The default sharing mode is `shared`, which allows concurrent read/write. apt's lock mechanism conflicts with this, causing corruption when parallel builds access the same cache.

**The fix:**
```dockerfile
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y curl
```

ALWAYS use `sharing=locked` for apt caches.

---

## AP-007: Using min Cache Mode in CI/CD

**The mistake:**
```bash
docker buildx build \
  --cache-to type=registry,ref=myapp:cache \
  --cache-from type=registry,ref=myapp:cache .
```

**Why it fails:** The default cache mode is `min`, which only exports the layers present in the final image. Intermediate build stage layers (dependency downloads, compilation steps) are NOT cached. On the next CI run, those expensive intermediate steps must be repeated.

**The fix:**
```bash
docker buildx build \
  --cache-to type=registry,ref=myapp:cache,mode=max \
  --cache-from type=registry,ref=myapp:cache .
```

ALWAYS use `mode=max` in CI/CD to cache all intermediate layers.

---

## AP-008: Ignoring Build Context Size

**The mistake:** Running `docker build .` in a directory with large files, vendor directories, or data sets without measuring context size.

**Why it fails:** The build context is transferred in its entirety to the Docker daemon before any instruction executes. A 2GB context takes significant time to transfer, even if the Dockerfile only copies a few files.

**How to detect:**
```bash
# Check context size (first line of build output)
docker build . 2>&1 | head -1
# Output: "Sending build context to Docker daemon  2.1GB"
```

**The fix:**
1. Add a `.dockerignore` file (AP-003)
2. Use bind mounts instead of COPY for large source trees
3. Use a subdirectory as the build context: `docker build -f Dockerfile ./src`

---

## AP-009: Squashing Instead of Multi-Stage

**The mistake:**
```bash
docker build --squash -t myapp .
```

**Why it fails:** Squashing merges ALL layers into one, destroying all intermediate layer cache. Every rebuild starts from scratch. The `--squash` flag is experimental and not recommended for production.

**The fix:** Use multi-stage builds to produce a clean final image while preserving layer caching:
```dockerfile
FROM golang:1.22 AS build
# ... build steps with full caching ...

FROM alpine:3.19
COPY --from=build /app/binary /usr/bin/binary
```

The final image contains only what you explicitly COPY into it, with zero build artifacts.

---

## AP-010: Not Falling Back to Main Branch Cache in CI

**The mistake:**
```bash
# Feature branch CI
docker buildx build \
  --cache-from type=registry,ref=myapp:cache-feature-123 \
  --cache-to type=registry,ref=myapp:cache-feature-123 .
```

**Why it fails:** A new feature branch has no cache yet. Every layer rebuilds from scratch on the first CI run, which can take 10-30 minutes for complex builds.

**The fix:**
```bash
docker buildx build \
  --cache-from type=registry,ref=myapp:cache-feature-123 \
  --cache-from type=registry,ref=myapp:cache-main \
  --cache-to type=registry,ref=myapp:cache-feature-123,mode=max .
```

ALWAYS include the main branch cache as a fallback source. Docker tries cache sources in order and uses the first match.

---

## AP-011: Unnecessary Layer Creation

**The mistake:**
```dockerfile
RUN apt-get update
RUN apt-get install -y curl
RUN apt-get install -y git
RUN apt-get install -y wget
RUN rm -rf /var/lib/apt/lists/*
```

**Why it fails:** Each RUN creates a separate layer. The `rm` in the last layer does NOT reduce image size -- the files still exist in the previous layers. Docker images are additive: deleting a file in a later layer only hides it, the bytes remain.

**The fix:**
```dockerfile
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    git \
    wget \
    && rm -rf /var/lib/apt/lists/*
```

ALWAYS combine related operations (install + cleanup) in a single RUN so removed files never exist in a committed layer.

---

## AP-012: Secrets in Build Args or ENV

**The mistake:**
```dockerfile
ARG DATABASE_PASSWORD=secret123
ENV API_KEY=sk-1234567890
RUN deploy.sh
```

**Why it fails:** ARG values are visible in `docker history`. ENV values persist in the image metadata and are visible via `docker inspect`. Both leak secrets.

Additionally, changing a secret value in ARG invalidates the cache, causing unnecessary rebuilds. Secret mounts do NOT affect cache keys.

**The fix:**
```dockerfile
RUN --mount=type=secret,id=db_pass,target=/run/secrets/db_pass \
    --mount=type=secret,id=api_key,env=API_KEY \
    deploy.sh
```

```bash
docker build \
  --secret id=db_pass,src=./db_pass.txt \
  --secret id=api_key,src=./api_key.txt .
```

NEVER use ARG or ENV for secrets. ALWAYS use `--mount=type=secret`.

---

## AP-013: Using COPY --link Without Understanding Its Behavior

**The mistake:** Blindly adding `--link` to all COPY instructions expecting faster builds.

**Why it fails:** `COPY --link` creates layers that are independent of preceding layers. This means:
- The layer can be cached even if a preceding layer changes
- BUT the files are placed in a new snapshot, not on top of the existing filesystem
- If the COPY destination depends on a directory created by a previous RUN, `--link` may not work as expected

**When to use `--link`:**
- Copying final artifacts into a clean runtime stage
- `COPY --link --from=build /app/binary /usr/bin/binary`

**When NOT to use `--link`:**
- When the COPY target directory is created by a preceding instruction
- When you need the copied files to interact with the existing layer filesystem

---

## Summary: Cache Optimization Checklist

1. Does the Dockerfile have `# syntax=docker/dockerfile:1`?
2. Is there a `.dockerignore` file?
3. Are dependency files copied BEFORE source code?
4. Are `apt-get update` and `install` in the SAME RUN?
5. Do package managers use `--mount=type=cache`?
6. Does CI use `mode=max` for cache export?
7. Does CI include main branch as fallback cache source?
8. Are secrets using `--mount=type=secret` (not ARG/ENV)?
9. Are install and cleanup in the same RUN layer?
10. Is the final stage using a minimal base image?
