---
name: docker-syntax-buildkit
description: >
  Use when optimizing Dockerfile builds with cache mounts, mounting secrets
  during build, or writing multi-line RUN with heredoc syntax.
  Prevents baking credentials into image layers, cache-busting on every build
  due to missing --mount=type=cache, and using outdated non-BuildKit syntax.
  Covers syntax directive, heredoc RUN, --mount=type=cache for apt/npm/pip/go,
  --mount=type=secret, --mount=type=ssh, bind mounts, and platform ARGs.
  Keywords: # syntax=docker/dockerfile:1, --mount=type=cache, --mount=type=secret,
  --mount=type=ssh, heredoc, BUILDKIT_INLINE_CACHE, TARGETPLATFORM,
  build secrets, SSH in build, cache npm pip, modern Dockerfile syntax.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+ with BuildKit (default)."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-syntax-buildkit

## Quick Reference

### Syntax Directive

ALWAYS include at the very top of every Dockerfile, before any other instruction:

```dockerfile
# syntax=docker/dockerfile:1
```

This enables heredoc syntax, `--mount` flags, `--chmod`/`--link`/`--parents`/`--exclude` on COPY/ADD, the `# check` directive, and all other BuildKit extensions. ALWAYS use `docker/dockerfile:1` (not `1.0` or a fixed minor version) to get the latest stable features.

### Mount Types Overview

| Mount Type | Purpose | Key Use Case |
|------------|---------|-------------|
| `cache` | Persistent cache directories across builds | Package manager caches (apt, npm, pip, go) |
| `secret` | Access credentials without baking into layers | API keys, tokens, registry auth |
| `ssh` | Forward host SSH agent during build | Cloning private Git repositories |
| `bind` | Mount context or stage files (read-only default) | Large source trees, cross-stage file access |
| `tmpfs` | Temporary in-memory filesystem | Scratch space for compilation, tests |

### Platform ARGs (Automatic in BuildKit)

| ARG | Example Value | Purpose |
|-----|---------------|---------|
| `TARGETPLATFORM` | `linux/amd64` | Full target platform string |
| `TARGETOS` | `linux` | Target operating system |
| `TARGETARCH` | `amd64` | Target architecture |
| `TARGETVARIANT` | `v7` | Target variant (e.g., ARM version) |
| `BUILDPLATFORM` | `linux/amd64` | Host platform running the build |
| `BUILDOS` | `linux` | Host operating system |
| `BUILDARCH` | `amd64` | Host architecture |

These ARGs are available automatically without explicit `ARG` declaration. ALWAYS declare them with `ARG TARGETOS TARGETARCH` inside a stage to use them in `RUN` instructions.

### Critical Warnings

**NEVER** put secrets in `ENV` or `ARG` instructions -- they are visible in `docker history` and image layers. ALWAYS use `--mount=type=secret` instead.

**NEVER** omit the syntax directive when using BuildKit features -- without `# syntax=docker/dockerfile:1`, mount flags and heredoc syntax cause parse errors.

**NEVER** use `sharing=shared` (the default) for `apt` cache mounts -- apt requires exclusive access. ALWAYS use `sharing=locked` for apt caches.

**NEVER** assume secret mount contents trigger cache invalidation -- they do NOT. If a secret changes and the build must reflect that change, pass a `CACHEBUST` build arg.

**ALWAYS** use `set -e` in heredoc RUN blocks -- without it, individual command failures are silently ignored and the build continues.

---

## Mount Type Decision Tree

```
Need to mount something during RUN?
|
+-- Persisting package downloads between builds?
|   --> type=cache (see Cache Mount Patterns below)
|
+-- Accessing credentials/tokens during build?
|   --> type=secret (file or env mode)
|
+-- Cloning private Git repos via SSH?
|   --> type=ssh
|
+-- Reading source files without creating a COPY layer?
|   +-- From build context? --> type=bind,target=.
|   +-- From another stage? --> type=bind,from=<stage>,target=<path>
|
+-- Need temporary scratch space (not persisted)?
    --> type=tmpfs
```

---

## Heredoc Syntax

### Multi-Line RUN

Run multi-line scripts without `&&` chaining:

```dockerfile
# syntax=docker/dockerfile:1

RUN <<EOF
#!/usr/bin/env bash
set -e
apt-get update
apt-get install -y --no-install-recommends curl git
rm -rf /var/lib/apt/lists/*
EOF
```

**ALWAYS** include `set -e` in heredoc RUN blocks. Without it, only the exit code of the LAST command determines success.

### Inline File Creation

Create files without a separate COPY:

```dockerfile
COPY <<EOF /etc/nginx/conf.d/default.conf
server {
    listen 80;
    server_name localhost;
    location / {
        root /usr/share/nginx/html;
    }
}
EOF
```

### Multiple Heredocs

```dockerfile
RUN <<INSTALL && <<CONFIGURE
apt-get update && apt-get install -y nginx
INSTALL
echo "daemon off;" >> /etc/nginx/nginx.conf
CONFIGURE
```

---

## Cache Mount Patterns

ALWAYS use cache mounts for package managers. The cache is cumulative -- even when a layer rebuilds, only new/changed packages are downloaded.

| Package Manager | Cache Target(s) | Sharing Mode |
|----------------|-----------------|--------------|
| apt | `/var/cache/apt` + `/var/lib/apt` | `locked` (required) |
| npm | `/root/.npm` | `shared` (default) |
| yarn | `/usr/local/share/.cache/yarn` | `shared` |
| pnpm | `/root/.local/share/pnpm/store` | `shared` |
| pip | `/root/.cache/pip` | `shared` |
| Go | `/go/pkg/mod` + `/root/.cache/go-build` | `shared` |
| Cargo (Rust) | `/app/target/` + `/usr/local/cargo/git/db` + `/usr/local/cargo/registry/` | `shared` |
| Maven | `/root/.m2/repository` | `shared` |
| Bundler (Ruby) | `/root/.gem` | `shared` |
| NuGet (.NET) | `/root/.nuget/packages` | `shared` |
| Composer (PHP) | `/tmp/cache` | `shared` |

See [references/examples.md](references/examples.md) for complete patterns per package manager.

### Cache Mount Full Syntax

```
--mount=type=cache,target=<path>[,id=<id>][,sharing=<shared|private|locked>][,from=<stage>][,source=<path>][,mode=<mode>][,uid=<uid>][,gid=<gid>]
```

| Option | Default | Purpose |
|--------|---------|---------|
| `target` | (required) | Directory to cache |
| `id` | value of `target` | Cache identity (share across stages with same id) |
| `sharing` | `shared` | `shared`: concurrent access; `locked`: exclusive; `private`: per-build copy |
| `from` | (none) | Initialize cache from a build stage |
| `source` | (none) | Path within `from` to seed cache |
| `mode` | `0755` | Directory permissions |
| `uid` | `0` | Owner user ID |
| `gid` | `0` | Owner group ID |

---

## Secret Mounts

### As File (default)

```dockerfile
RUN --mount=type=secret,id=aws,target=/root/.aws/credentials \
    aws s3 cp s3://bucket/file /dest
```

Build: `docker build --secret id=aws,src=$HOME/.aws/credentials .`

### As Environment Variable

```dockerfile
RUN --mount=type=secret,id=TOKEN,env=TOKEN \
    some-command  # $TOKEN is available
```

Build: `docker build --secret id=TOKEN,src=./token.txt .`

### Secret Mount Options

| Option | Default | Purpose |
|--------|---------|---------|
| `id` | (required) | Secret identifier matching `--secret id=` |
| `target` | `/run/secrets/<id>` | Mount path inside the container |
| `required` | `false` | Fail build if secret is not provided |
| `env` | (none) | Expose as environment variable instead of file |
| `mode` | `0400` | File permissions |
| `uid` | `0` | Owner user ID |
| `gid` | `0` | Owner group ID |

---

## SSH Mounts

```dockerfile
RUN --mount=type=ssh \
    git clone git@github.com:org/private-repo.git /app
```

Build: `docker build --ssh default .`

| Option | Default | Purpose |
|--------|---------|---------|
| `id` | `default` | SSH agent socket identifier |
| `target` | `/run/buildkit/ssh_agent.${N}` | Mount path for socket |
| `required` | `false` | Fail build if SSH agent is not available |

ALWAYS add GitHub/GitLab host keys before cloning:

```dockerfile
RUN --mount=type=ssh \
    mkdir -p ~/.ssh && ssh-keyscan github.com >> ~/.ssh/known_hosts \
    && git clone git@github.com:org/repo.git /app
```

---

## Bind Mounts

```dockerfile
# Mount entire build context (avoids COPY layer)
RUN --mount=type=bind,target=. go build -o /app/hello

# Mount from another stage
RUN --mount=type=bind,from=build,source=/src,target=/source ls /source

# Mount single file
RUN --mount=type=bind,source=requirements.txt,target=/tmp/requirements.txt \
    pip install -r /tmp/requirements.txt
```

| Option | Default | Purpose |
|--------|---------|---------|
| `target` | (required) | Mount destination in container |
| `source` | `.` (root of context/stage) | Source path |
| `from` | build context | Named stage or image to mount from |
| `rw` | `false` | Set `true` for read-write (changes NOT persisted) |

---

## Tmpfs Mounts

```dockerfile
RUN --mount=type=tmpfs,target=/tmp gcc -o /app/binary source.c
```

| Option | Default | Purpose |
|--------|---------|---------|
| `target` | (required) | Mount path |
| `size` | unlimited | Size limit in bytes |

---

## Cross-Compilation Pattern

```dockerfile
# syntax=docker/dockerfile:1

FROM --platform=$BUILDPLATFORM golang:1.22-alpine AS build
ARG TARGETOS TARGETARCH

WORKDIR /src
COPY go.mod go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod go mod download

COPY . .
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    GOOS=$TARGETOS GOARCH=$TARGETARCH go build -o /bin/app ./cmd

FROM alpine:3.19
COPY --from=build /bin/app /usr/bin/app
ENTRYPOINT ["/usr/bin/app"]
```

Build multi-platform: `docker buildx build --platform linux/amd64,linux/arm64 -t myapp:latest .`

---

## Cache Backends

| Backend | Flag | Use Case |
|---------|------|----------|
| Inline | `type=inline` | Embed cache metadata in output image |
| Registry | `type=registry,ref=<image>` | Store cache as separate registry image |
| Local | `type=local,dest=<path>` | Local filesystem directory |
| GitHub Actions | `type=gha` | GitHub Actions cache service |
| S3 | `type=s3,bucket=<name>,region=<region>` | AWS S3 storage |
| Azure Blob | `type=azblob,account_url=<url>` | Azure Blob storage |

### Cache Modes

- `min` (default) -- Only caches exported layers. Smaller cache, fewer hits.
- `max` -- Caches ALL intermediate layers. Larger cache, more hits. ALWAYS use `mode=max` in CI/CD.

### Registry Cache (CI/CD)

```bash
docker buildx build --push -t registry/app:latest \
  --cache-to type=registry,ref=registry/app:buildcache,mode=max \
  --cache-from type=registry,ref=registry/app:buildcache .
```

### GitHub Actions Cache

```bash
docker buildx build \
  --cache-to type=gha,mode=max \
  --cache-from type=gha .
```

---

## Reference Links

- [references/mounts.md](references/mounts.md) -- Complete reference for all 5 mount types with every option
- [references/examples.md](references/examples.md) -- Cache mount patterns per package manager, secret patterns, SSH patterns
- [references/anti-patterns.md](references/anti-patterns.md) -- BuildKit feature misuse and corrections

### Official Sources

- https://docs.docker.com/reference/dockerfile/
- https://docs.docker.com/build/buildkit/
- https://docs.docker.com/build/cache/
- https://docs.docker.com/build/cache/backends/
- https://docs.docker.com/build/building/multi-stage/
