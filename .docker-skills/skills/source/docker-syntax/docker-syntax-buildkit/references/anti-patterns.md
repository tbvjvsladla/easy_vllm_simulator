# BuildKit Anti-Patterns

## Cache Mount Misuse

### Using shared mode for apt

```dockerfile
# BAD: apt uses lock files internally -- concurrent access causes failures
RUN --mount=type=cache,target=/var/cache/apt \
    --mount=type=cache,target=/var/lib/apt \
    apt-get update && apt-get install -y curl
```

```dockerfile
# GOOD: ALWAYS use sharing=locked for apt
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y curl
```

**Why:** apt/dpkg uses internal lock files. Without `sharing=locked`, parallel builds corrupt the cache or fail with lock errors.

---

### Removing cache in the same RUN with cache mount

```dockerfile
# BAD: Cleaning the cache defeats the purpose of the cache mount
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y curl \
    && rm -rf /var/lib/apt/lists/*
```

```dockerfile
# GOOD: Let the cache mount handle persistence -- no cleanup needed
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y curl
```

**Why:** Cache mount contents are NOT part of the layer. Cleaning them is unnecessary and actually clears the persistent cache that saves time on the next build.

---

### Forgetting to copy artifacts out of cache-mounted directories

```dockerfile
# BAD: Binary is in cache-mounted target/ -- it disappears after RUN
RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release
# The binary at /app/target/release/myapp is GONE
```

```dockerfile
# GOOD: Copy the artifact to a non-cached path within the same RUN
RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release \
    && cp /app/target/release/myapp /usr/local/bin/myapp
```

**Why:** Cache mount directories exist outside the layer. Files written there do NOT become part of the image. ALWAYS copy build artifacts to a non-mounted path before the RUN ends.

---

### Not specifying cache id for multi-project builders

```dockerfile
# BAD: Projects sharing a builder collide on /root/.npm cache
# Project A
RUN --mount=type=cache,target=/root/.npm npm ci

# Project B (different project, same builder)
RUN --mount=type=cache,target=/root/.npm npm ci
```

```dockerfile
# GOOD: Use explicit id to namespace caches
# Project A
RUN --mount=type=cache,target=/root/.npm,id=project-a-npm npm ci

# Project B
RUN --mount=type=cache,target=/root/.npm,id=project-b-npm npm ci
```

**Why:** Cache identity defaults to `target` path. On shared builders (CI/CD), unrelated projects share and potentially corrupt each other's caches.

---

## Secret Mount Misuse

### Using ARG or ENV for secrets

```dockerfile
# BAD: Secret visible in docker history and image metadata
ARG DATABASE_PASSWORD=secret123
ENV API_KEY=sk-1234567890
RUN curl -H "Authorization: Bearer $API_KEY" https://api.example.com
```

```dockerfile
# GOOD: Secret available only during RUN, never in any layer
RUN --mount=type=secret,id=api_key,env=API_KEY \
    curl -H "Authorization: Bearer $API_KEY" https://api.example.com
```

**Why:** ARG values appear in `docker history`. ENV values persist in the image and are visible via `docker inspect`. Secret mounts exist only during the RUN instruction and leave zero trace.

---

### Assuming secret changes invalidate cache

```dockerfile
# BAD: Secret changed but build uses cached layer with old secret's output
RUN --mount=type=secret,id=TOKEN,env=TOKEN \
    curl -H "Authorization: $TOKEN" https://api.example.com/data > /data.json
```

```dockerfile
# GOOD: Use CACHEBUST arg to force rebuild when secret changes
ARG CACHEBUST
RUN --mount=type=secret,id=TOKEN,env=TOKEN \
    curl -H "Authorization: $TOKEN" https://api.example.com/data > /data.json
```

```bash
docker build --secret id=TOKEN,src=./token.txt \
  --build-arg CACHEBUST=$(date +%s) .
```

**Why:** Secret contents are deliberately excluded from cache keys for security. The RUN layer is cached based on the command string only.

---

### Copying secret to a file in the image

```dockerfile
# BAD: Secret ends up in a layer
RUN --mount=type=secret,id=creds,target=/tmp/creds \
    cp /tmp/creds /app/credentials.json
```

```dockerfile
# GOOD: Use secret in-place, never copy it
RUN --mount=type=secret,id=creds,target=/tmp/creds \
    my-tool --config /tmp/creds
```

**Why:** The whole point of secret mounts is that they leave no trace. Copying the secret to a regular file path bakes it into the layer permanently.

---

## SSH Mount Misuse

### Not adding known hosts

```dockerfile
# BAD: SSH prompts for host key verification -- build hangs indefinitely
RUN --mount=type=ssh \
    git clone git@github.com:org/repo.git /app
```

```dockerfile
# GOOD: Add known hosts before any SSH operation
RUN --mount=type=ssh \
    mkdir -p ~/.ssh \
    && ssh-keyscan github.com >> ~/.ssh/known_hosts \
    && git clone git@github.com:org/repo.git /app
```

**Why:** Without known hosts, SSH prompts for interactive confirmation. Docker builds are non-interactive -- the build hangs until timeout.

---

### Copying SSH keys into the image

```dockerfile
# BAD: Private key baked into image layer
COPY id_rsa /root/.ssh/id_rsa
RUN chmod 600 /root/.ssh/id_rsa \
    && git clone git@github.com:org/repo.git /app
```

```dockerfile
# GOOD: Forward SSH agent -- key never touches the image
RUN --mount=type=ssh \
    mkdir -p ~/.ssh \
    && ssh-keyscan github.com >> ~/.ssh/known_hosts \
    && git clone git@github.com:org/repo.git /app
```

**Why:** Even if you delete the key in a later RUN, it remains in the COPY layer and can be extracted. SSH mounts forward the agent socket -- the private key never enters the build.

---

## Heredoc Misuse

### Missing set -e in heredoc

```dockerfile
# BAD: If apt-get update fails, install still runs (and may use stale packages)
RUN <<EOF
apt-get update
apt-get install -y curl
rm -rf /var/lib/apt/lists/*
EOF
```

```dockerfile
# GOOD: set -e ensures any failure stops the build
RUN <<EOF
#!/usr/bin/env bash
set -e
apt-get update
apt-get install -y curl
rm -rf /var/lib/apt/lists/*
EOF
```

**Why:** Without `set -e`, heredoc scripts report only the exit code of the LAST command. Earlier failures are silently ignored, leading to broken images.

---

### Using heredoc for single commands

```dockerfile
# BAD: Unnecessary complexity for a single command
RUN <<EOF
npm install
EOF
```

```dockerfile
# GOOD: Use standard RUN for simple commands
RUN npm install
```

**Why:** Heredoc syntax is for multi-line scripts that benefit from avoiding `&&` chains. Single commands gain nothing from it and lose readability.

---

## Syntax Directive Misuse

### Placing syntax directive after comments or blank lines

```dockerfile
# This is my Dockerfile
# syntax=docker/dockerfile:1

FROM alpine
RUN --mount=type=cache,target=/tmp echo hello
```

```dockerfile
# syntax=docker/dockerfile:1
# This is my Dockerfile

FROM alpine
RUN --mount=type=cache,target=/tmp echo hello
```

**Why:** Parser directives MUST be at the very top of the file, before any comments, blank lines, or instructions. A syntax directive after any other line is treated as a regular comment and ignored -- mount flags then cause parse errors.

---

### Pinning to a specific minor version

```dockerfile
# BAD: Misses bug fixes and new features
# syntax=docker/dockerfile:1.4
```

```dockerfile
# GOOD: Gets latest stable features within major version 1
# syntax=docker/dockerfile:1
```

**Why:** `docker/dockerfile:1` resolves to the latest `1.x` release. Pinning to `1.4` misses improvements like `--parents`, `--exclude`, `# check`, and security fixes.

---

## Bind Mount Misuse

### Expecting bind mount writes to persist

```dockerfile
# BAD: Writes to read-write bind mount are discarded after RUN
RUN --mount=type=bind,target=/src,rw=true \
    echo "modified" > /src/file.txt
# /src/file.txt is NOT modified in the build context or any layer
```

```dockerfile
# GOOD: Write output to a non-mounted path
RUN --mount=type=bind,target=/src \
    cp /src/template.txt /app/config.txt \
    && sed -i 's/PLACEHOLDER/value/' /app/config.txt
```

**Why:** Bind mount changes (even with `rw=true`) are never persisted. They do not modify the build context and are not part of any layer. ALWAYS write results to a non-mounted path.

---

## Cache Backend Misuse

### Using min mode in CI/CD

```bash
# BAD: Only caches exported layers -- intermediate stages are rebuilt every time
docker buildx build \
  --cache-to type=registry,ref=registry/app:cache \
  --cache-from type=registry,ref=registry/app:cache .
```

```bash
# GOOD: Cache ALL layers including intermediates
docker buildx build \
  --cache-to type=registry,ref=registry/app:cache,mode=max \
  --cache-from type=registry,ref=registry/app:cache .
```

**Why:** The default `min` mode only caches layers that end up in the final image. Multi-stage builds have many intermediate layers (dependency download, compilation) that provide the biggest cache benefit. ALWAYS use `mode=max` in CI/CD.

---

### Not providing cache-from on first build

```bash
# BAD: First build has no cache source -- works but logs warnings
docker buildx build \
  --cache-from type=registry,ref=registry/app:cache .
```

This is actually fine -- BuildKit gracefully handles missing cache sources. No change needed. The `--cache-from` is silently ignored if the cache image does not exist. This is NOT an error.

---

### Single cache source without branch fallback

```bash
# BAD: Feature branch has no cache -- full rebuild
docker buildx build \
  --cache-from type=registry,ref=registry/app:cache-feature .
```

```bash
# GOOD: Fall back to main branch cache
docker buildx build \
  --cache-from type=registry,ref=registry/app:cache-feature \
  --cache-from type=registry,ref=registry/app:cache-main .
```

**Why:** Feature branches diverge from main. Without a fallback, the first build on a new branch starts from scratch. Multiple `--cache-from` sources let BuildKit find the best match.
