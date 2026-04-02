# Dockerfile & BuildKit Research

> Research fragment for the Docker Claude Skill Package.
> Sources: Official Docker documentation, fetched via WebFetch on 2026-03-19.
> All content verified against docs.docker.com.

---

## Table of Contents

1. [Parser Directives](#1-parser-directives)
2. [Dockerfile Instructions Reference](#2-dockerfile-instructions-reference)
3. [BuildKit Features (Engine 24+)](#3-buildkit-features-engine-24)
4. [Multi-Stage Build Patterns](#4-multi-stage-build-patterns)
5. [Layer Optimization & Caching](#5-layer-optimization--caching)
6. [Best Practices](#6-best-practices)
7. [Common Anti-Patterns](#7-common-anti-patterns)

---

## 1. Parser Directives

Parser directives are optional comments that MUST appear at the very top of the Dockerfile, before any builder instructions, blank lines, or comments.

Source: https://docs.docker.com/reference/dockerfile/

### syntax

Declares the Dockerfile syntax version. Enables BuildKit frontend features and automatic updates.

```dockerfile
# syntax=docker/dockerfile:1
```

- ALWAYS use `docker/dockerfile:1` (not `1.0` or a fixed minor) to get the latest stable features.
- Required for heredoc syntax, `--mount` flags, `# check` directive, and other BuildKit extensions.

### escape

Sets the escape character (default `\`). Useful on Windows where `\` is the path separator.

```dockerfile
# escape=`
```

### check (Dockerfile v1.8.0+)

Configures build check evaluation — lint rules for your Dockerfile.

```dockerfile
# check=skip=JSONArgsRecommended,StageNameCasing
# check=error=true
# check=skip=all
# check=skip=JSONArgsRecommended;error=true
```

---

## 2. Dockerfile Instructions Reference

Source: https://docs.docker.com/reference/dockerfile/

### FROM

Initializes a new build stage and sets the base image. MUST be the first instruction (after parser directives and global ARGs).

**Syntax:**
```dockerfile
FROM [--platform=<platform>] <image> [AS <name>]
FROM [--platform=<platform>] <image>[:<tag>] [AS <name>]
FROM [--platform=<platform>] <image>[@<digest>] [AS <name>]
```

**Parameters:**
| Parameter | Description |
|-----------|-------------|
| `--platform` | Target platform, e.g., `linux/amd64`, `linux/arm64` |
| `AS <name>` | Name the build stage for later `COPY --from=<name>` |
| `<tag>` | Defaults to `latest` if omitted |
| `@<digest>` | Pin to exact image digest for reproducibility |

**Key behaviors:**
- Multiple `FROM` instructions create multi-stage builds.
- Each `FROM` clears all prior state (layers, ENV, ARG within stage).
- ARG instructions before FROM are available in FROM but NOT in subsequent instructions unless re-declared.

**Examples:**
```dockerfile
# Basic
FROM ubuntu:22.04

# Named stage
FROM golang:1.22 AS builder

# Platform-specific
FROM --platform=linux/arm64 alpine:3.19

# Digest-pinned
FROM alpine:3.21@sha256:a8560b36e8b8210634f77d9f7f9efd7ffa463e380b75e2e74aff4511df3ef88c

# With global ARG
ARG BASE_IMAGE=ubuntu:22.04
FROM ${BASE_IMAGE} AS builder
```

---

### RUN

Executes commands in a new layer on top of the current image. The most complex instruction with many BuildKit-specific features.

**Syntax forms:**

```dockerfile
# Shell form (runs in /bin/sh -c by default)
RUN apt-get update && apt-get install -y curl

# Exec form (no shell processing — no variable expansion)
RUN ["apt-get", "install", "-y", "curl"]

# Heredoc form (BuildKit, # syntax=docker/dockerfile:1)
RUN <<EOF
#!/usr/bin/env bash
set -e
apt-get update
apt-get install -y curl
EOF
```

**Mount options (BuildKit, Dockerfile >= 1.2):**

| Mount Type | Purpose | Min Dockerfile Version |
|------------|---------|----------------------|
| `--mount=type=bind` | Bind-mount context files (read-only by default) | 1.2 |
| `--mount=type=cache` | Persistent cache directories across builds | 1.2 |
| `--mount=type=tmpfs` | Temporary filesystem mount | 1.2 |
| `--mount=type=secret` | Access secrets without baking into layers | 1.2 |
| `--mount=type=ssh` | Forward SSH agent for Git auth etc. | 1.2 |

**Other RUN options:**

| Option | Purpose | Min Version |
|--------|---------|-------------|
| `--network=default\|none\|host` | Control network access during build | 1.3 |
| `--security=sandbox\|insecure` | Security mode | 1.20 |

**Cache mount examples:**

```dockerfile
# Go build cache
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    go build -o /app/hello

# apt cache (sharing=locked required for apt)
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt update && apt-get --no-install-recommends install -y gcc

# npm cache
RUN --mount=type=cache,target=/root/.npm \
    npm install

# pip cache
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install -r requirements.txt

# Ruby gem cache
RUN --mount=type=cache,target=/root/.gem \
    bundle install

# Rust/Cargo cache
RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build

# .NET NuGet cache
RUN --mount=type=cache,target=/root/.nuget/packages \
    dotnet restore

# PHP Composer cache
RUN --mount=type=cache,target=/tmp/cache \
    composer install
```

**Secret mount examples:**

```dockerfile
# Mount a secret file
RUN --mount=type=secret,id=aws,target=/root/.aws/credentials \
    aws s3 cp s3://bucket/file /dest

# Build command:
# docker build --secret id=aws,src=$HOME/.aws/credentials .
```

**SSH mount examples:**

```dockerfile
RUN --mount=type=ssh \
    ssh -q -T git@gitlab.com

# Build command:
# docker build --ssh default .
```

**Bind mount examples:**

```dockerfile
# Mount source from another stage
RUN --mount=type=bind,target=/source,from=build go build ...

# Mount build context read-only (avoids COPY layer)
RUN --mount=type=bind,target=. go build -o /app/hello
```

**Tmpfs mount:**

```dockerfile
RUN --mount=type=tmpfs,target=/tmp gcc -o /app/binary source.c
```

**Key behaviors:**
- Shell form uses `/bin/sh -c` by default (configurable via SHELL).
- Exec form does NOT invoke a shell — no variable expansion, no pipes.
- Each RUN creates a new layer (combine commands with `&&` to minimize layers).
- Cache for RUN is invalidated only when the command string changes, NOT when external resources change.

---

### CMD

Specifies the default command to run when a container starts. Does NOT execute during build.

**Syntax forms:**

```dockerfile
# Exec form (PREFERRED)
CMD ["executable", "param1", "param2"]

# As default parameters to ENTRYPOINT
CMD ["param1", "param2"]

# Shell form
CMD command param1 param2
```

**Key behaviors:**
- Only the LAST CMD in a Dockerfile takes effect.
- Overridden by arguments passed to `docker run`.
- When combined with ENTRYPOINT (exec form), CMD provides default arguments.

---

### ENTRYPOINT

Configures the container to run as an executable.

**Syntax forms:**

```dockerfile
# Exec form (PREFERRED)
ENTRYPOINT ["executable", "param1", "param2"]

# Shell form
ENTRYPOINT command param1 param2
```

**Key behaviors:**
- Exec form: `docker run` arguments are APPENDED to ENTRYPOINT.
- Shell form: `docker run` arguments are IGNORED; runs under `/bin/sh -c`.
- Only the LAST ENTRYPOINT takes effect.

### CMD + ENTRYPOINT Interaction Table

| | No ENTRYPOINT | `ENTRYPOINT exec_entry p1` (shell) | `ENTRYPOINT ["exec_entry", "p1"]` (exec) |
|---|---|---|---|
| **No CMD** | Error | `/bin/sh -c exec_entry p1` | `exec_entry p1` |
| **`CMD ["exec_cmd", "p1"]`** | `exec_cmd p1` | `/bin/sh -c exec_entry p1` | `exec_entry p1 exec_cmd p1` |
| **`CMD exec_cmd p1`** | `/bin/sh -c exec_cmd p1` | `/bin/sh -c exec_entry p1` | `exec_entry p1 /bin/sh -c exec_cmd p1` |

**Best practice pattern — entrypoint script:**

```dockerfile
COPY ./docker-entrypoint.sh /
ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["postgres"]
```

```bash
#!/bin/bash
set -e

if [ "$1" = 'postgres' ]; then
    chown -R postgres "$PGDATA"
    if [ -z "$(ls -A "$PGDATA")" ]; then
        gosu postgres initdb
    fi
    exec gosu postgres "$@"
fi

exec "$@"
```

The `exec` command replaces the shell process, making the application PID 1 for proper signal handling.

---

### COPY

Copies files from build context or earlier build stages.

**Syntax:**
```dockerfile
COPY [OPTIONS] <src> ... <dest>
COPY [OPTIONS] ["<src>", ... "<dest>"]
```

**Options:**

| Option | Purpose | Min Version |
|--------|---------|-------------|
| `--from=<stage\|image\|context>` | Copy from other stage, external image, or named context | - |
| `--chown=<user>:<group>` | Set ownership (Linux only) | - |
| `--chmod=<perms>` | Set permissions (octal or symbolic) | 1.2 |
| `--link[=<boolean>]` | Enhanced layer reuse across rebuilds | 1.4 |
| `--parents[=<boolean>]` | Preserve parent directory structure | 1.7 |
| `--exclude=<pattern>` | Exclude matching paths | 1.7 |

**Examples:**

```dockerfile
# Basic copy
COPY file1.txt file2.txt /usr/src/things/
COPY *.png /dest/

# Multi-stage copy
COPY --from=build /myapp /usr/bin/

# Copy from external image
COPY --from=nginx:latest /etc/nginx/nginx.conf /nginx.conf

# Permissions and ownership
COPY --chmod=755 app.sh /app/
COPY --chown=myuser:mygroup --chmod=644 files* /somedir/

# Preserve parent directories
COPY --parents ./x/a.txt ./y/a.txt /parents/
# Creates /parents/x/a.txt and /parents/y/a.txt

# Exclude patterns
COPY --exclude=*.txt --exclude=*.md hom* /mydir/

# Link mode (better layer reuse)
COPY --link /foo /bar
```

**Key behaviors:**
- Cache invalidation uses file content checksums (NOT mtime).
- Destination with trailing `/` is treated as a directory.
- Relative paths resolve against WORKDIR.
- Default permissions: 644 for files, 755 for directories.

---

### ADD

Like COPY but with additional capabilities: URL downloads, Git repos, auto-extraction.

**Syntax:**
```dockerfile
ADD [OPTIONS] <src> ... <dest>
ADD [OPTIONS] ["<src>", ... "<dest>"]
```

**Additional options beyond COPY:**

| Option | Purpose | Min Version |
|--------|---------|-------------|
| `--keep-git-dir=<boolean>` | Preserve `.git` directory | 1.1 |
| `--checksum=<hash>` | Verify integrity of remote sources | 1.6 |
| `--unpack=<bool>` | Control auto-extraction of archives | 1.17 |

**Sources supported:** Local files, HTTP/HTTPS URLs, Git repositories, local tar archives.

**Examples:**

```dockerfile
# Remote file download
ADD https://example.com/archive.zip /usr/src/things/

# Git repository
ADD https://github.com/moby/buildkit.git#v0.14.1:docs /buildkit-docs

# Checksum verification
ADD --checksum=sha256:24454f... https://mirrors.example.com/archive.tar.gz /

# Control auto-extraction
ADD --unpack=false my-archive.tar.gz .
```

### COPY vs ADD — When to Use Which

| Use Case | Instruction |
|----------|-------------|
| Copy local files from build context | COPY (preferred) |
| Copy from another build stage | COPY --from |
| Download remote file with checksum | ADD --checksum |
| Clone a Git repository | ADD (Git URL) |
| Auto-extract a local tar archive | ADD |
| General-purpose local file copy | COPY (more explicit, preferred) |

---

### ENV

Sets environment variables that persist in the final image and are available to all subsequent instructions and at container runtime.

**Syntax:**
```dockerfile
ENV <key>=<value> [<key>=<value>...]
```

**Examples:**
```dockerfile
ENV MY_NAME="John Doe"
ENV MY_VAR=hello MY_OTHER_VAR=world
ENV PATH=/usr/local/nginx/bin:$PATH
ENV PG_MAJOR=9.3 PG_VERSION=9.3.4
```

**Key behaviors:**
- Persists in the final image (visible via `docker inspect`).
- Each ENV instruction creates a new layer.
- Overridable at runtime via `docker run --env KEY=VALUE`.
- Values inherit into child stages in multi-stage builds.

**Variable precedence:**
```dockerfile
ENV abc=hello
ENV abc=bye def=$abc   # def=hello (uses value BEFORE this line)
ENV ghi=$abc           # ghi=bye
```

---

### ARG

Defines build-time variables. NOT persisted in the final image.

**Syntax:**
```dockerfile
ARG <name>[=<default value>]
```

**Examples:**
```dockerfile
ARG VERSION=latest
FROM busybox:$VERSION
ARG VERSION           # Must re-declare after FROM to use within stage
RUN echo $VERSION > image_version
```

**Predefined platform ARGs (automatic in BuildKit):**
- `TARGETPLATFORM` (e.g., `linux/amd64`)
- `TARGETOS`, `TARGETARCH`, `TARGETVARIANT`
- `BUILDPLATFORM`, `BUILDOS`, `BUILDARCH`, `BUILDVARIANT`

**Predefined proxy ARGs (excluded from `docker history` by default):**
- `HTTP_PROXY`, `HTTPS_PROXY`, `FTP_PROXY`, `NO_PROXY`, `ALL_PROXY`
- Lowercase variants also supported

**BuildKit built-in ARGs:**
- `BUILDKIT_INLINE_CACHE` — Enable inline cache metadata
- `BUILDKIT_MULTI_PLATFORM` — Deterministic multi-platform output
- `BUILDKIT_SANDBOX_HOSTNAME` — Set build hostname
- `BUILDKIT_CONTEXT_KEEP_GIT_DIR` — Preserve `.git` in context
- `BUILDKIT_CACHE_MOUNT_NS` — Cache ID namespace

**Security warning:** ARG values are visible in `docker history`. NEVER use ARG for secrets. Use `RUN --mount=type=secret` instead.

---

### WORKDIR

Sets the working directory for RUN, CMD, ENTRYPOINT, COPY, ADD.

```dockerfile
WORKDIR /path/to/workdir
```

- Created automatically if it doesn't exist.
- Relative paths stack: `WORKDIR /a` then `WORKDIR b` then `WORKDIR c` = `/a/b/c`.
- Supports environment variable expansion: `WORKDIR $DIRPATH`.
- ALWAYS use WORKDIR instead of `RUN cd /some/path && ...`.

---

### EXPOSE

Documents which ports the container listens on. Does NOT publish them.

```dockerfile
EXPOSE 80/tcp
EXPOSE 80/udp
EXPOSE 80 443 8080/udp
```

- Defaults to TCP if protocol is omitted.
- Ports are published at runtime with `docker run -p` or `-P`.

---

### VOLUME

Creates a mount point for externally mounted volumes.

```dockerfile
VOLUME ["/data"]
VOLUME /var/log /var/db
```

- Marks directories as externally mountable.
- Host directory specified at container runtime, NOT in Dockerfile.
- STRONGLY recommended for mutable or user-serviceable data (databases, logs, config).

---

### USER

Sets the user (and optionally group) for subsequent RUN, CMD, ENTRYPOINT instructions and the running container.

```dockerfile
USER <user>[:<group>]
USER <UID>[:<GID>]
```

**Example:**
```dockerfile
RUN groupadd -r appuser && useradd -r -g appuser appuser
USER appuser
```

---

### HEALTHCHECK

Defines how Docker tests whether the container is still working.

```dockerfile
HEALTHCHECK [OPTIONS] CMD <command>
HEALTHCHECK NONE
```

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--interval=DURATION` | 30s | Time between checks |
| `--timeout=DURATION` | 30s | Max time for check to complete |
| `--start-period=DURATION` | 0s | Grace period on startup |
| `--retries=N` | 3 | Consecutive failures to mark unhealthy |

**Example:**
```dockerfile
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost/ || exit 1
```

**Exit codes:** 0 = healthy, 1 = unhealthy, 2 = reserved.

---

### LABEL

Adds metadata key-value pairs to the image.

```dockerfile
LABEL "com.example.vendor"="ACME Incorporated"
LABEL version="1.0" description="My app"
LABEL org.opencontainers.image.authors="author@example.com"
```

- Use OCI standard label keys where possible (`org.opencontainers.image.*`).
- View with `docker image inspect --format='{{json .Config.Labels}}' <image>`.
- Replaces deprecated MAINTAINER instruction.

---

### SHELL

Overrides the default shell used for shell-form commands.

```dockerfile
SHELL ["/bin/bash", "-c"]
RUN echo hello   # Now uses bash instead of sh

# Windows
SHELL ["powershell", "-Command"]
RUN Write-Host 'Hello'
```

---

### STOPSIGNAL

Sets the system call signal sent to the container to exit.

```dockerfile
STOPSIGNAL SIGTERM
```

---

### ONBUILD

Adds a trigger instruction executed when the image is used as a base for another build.

```dockerfile
ONBUILD ADD . /app/src
ONBUILD RUN /app/src/compile.sh
```

- NOT executed in the current image build.
- Fires in child images that use `FROM <this-image>`.
- Useful for language-stack base images.
- Images with ONBUILD should get dedicated tags (e.g., `ruby:2.0-onbuild`).

---

### Environment Variable Substitution

Variables (`$variable` or `${variable}`) are supported in: ADD, COPY, ENV, EXPOSE, FROM, LABEL, STOPSIGNAL, USER, VOLUME, WORKDIR, ONBUILD.

**Modifiers (bash-style):**

```dockerfile
${variable:-default}         # Use default if unset
${variable:+alternate}       # Use alternate if set
${variable#pattern}          # Remove shortest prefix match
${variable##pattern}         # Remove longest prefix match
${variable%pattern}          # Remove shortest suffix match
${variable%%pattern}         # Remove longest suffix match
${variable/find/replace}     # Replace first occurrence
${variable//find/replace}    # Replace all occurrences
```

**Escaping:** `\$foo` or `\${foo}` for literal dollar signs.

---

## 3. BuildKit Features (Engine 24+)

Sources:
- https://docs.docker.com/build/buildkit/
- https://docs.docker.com/reference/dockerfile/

### Enabling BuildKit

BuildKit is the **default builder** since Docker Engine v23.0 and Docker Desktop.

For older engines:
```bash
DOCKER_BUILDKIT=1 docker build .
```

Or in `/etc/docker/daemon.json`:
```json
{
  "features": {
    "buildkit": true
  }
}
```

### Syntax Directive

ALWAYS include at the top of every Dockerfile:

```dockerfile
# syntax=docker/dockerfile:1
```

This enables:
- Heredoc syntax
- `--mount` flags on RUN
- `--chmod` on COPY/ADD
- `--link` on COPY/ADD
- `--parents` on COPY
- `--exclude` on COPY/ADD
- `# check` directive
- Network modes for RUN
- And all other BuildKit extensions

### Heredoc Syntax

Run multi-line scripts without `&&` chaining:

```dockerfile
# syntax=docker/dockerfile:1

RUN <<EOF
#!/usr/bin/env bash
set -e
apt-get update
apt-get install -y curl git
rm -rf /var/lib/apt/lists/*
EOF
```

Create files inline:

```dockerfile
COPY <<EOF /usr/share/nginx/html/index.html
<!DOCTYPE html>
<html><body><h1>Hello</h1></body></html>
EOF
```

Multiple heredocs:

```dockerfile
RUN <<SCRIPT1 && <<SCRIPT2
echo "First script"
SCRIPT1
echo "Second script"
SCRIPT2
```

### Cache Mounts (`--mount=type=cache`)

Persist package manager caches across builds. The cache is cumulative — even when a layer rebuilds, only new/changed packages are downloaded.

**Full syntax:**
```dockerfile
RUN --mount=type=cache,target=<path>[,id=<id>][,sharing=<shared|private|locked>][,from=<stage>][,source=<path>][,mode=<mode>][,uid=<uid>][,gid=<gid>] <command>
```

**Sharing modes:**
- `shared` (default) — Multiple builds can read/write concurrently.
- `private` — Each build gets its own cache instance.
- `locked` — Only one build can access at a time. Required for `apt`.

**Package manager cache targets (comprehensive list):**

| Package Manager | Cache Target(s) |
|----------------|-----------------|
| apt | `/var/cache/apt` + `/var/lib/apt` (both with `sharing=locked`) |
| npm | `/root/.npm` |
| pip | `/root/.cache/pip` |
| Go modules | `/go/pkg/mod` + `/root/.cache/go-build` |
| Cargo (Rust) | `/app/target/` + `/usr/local/cargo/git/db` + `/usr/local/cargo/registry/` |
| Bundler (Ruby) | `/root/.gem` |
| NuGet (.NET) | `/root/.nuget/packages` |
| Composer (PHP) | `/tmp/cache` |

### Secret Mounts (`--mount=type=secret`)

Access sensitive data during build without persisting in any layer.

```dockerfile
RUN --mount=type=secret,id=mytoken,target=/run/secrets/mytoken \
    cat /run/secrets/mytoken | some-command

# Or as environment variable
RUN --mount=type=secret,id=TOKEN,env=TOKEN \
    some-command  # $TOKEN available
```

Build command:
```bash
docker build --secret id=mytoken,src=./token.txt .
```

**Important:** Secret contents do NOT trigger cache invalidation when they change. To force rebuild when a secret changes, pass a `CACHEBUST` build arg:
```dockerfile
ARG CACHEBUST
RUN --mount=type=secret,id=TOKEN,env=TOKEN some-command
```
```bash
docker build --secret id=TOKEN --build-arg CACHEBUST=$(date +%s) .
```

### SSH Mounts (`--mount=type=ssh`)

Forward the host SSH agent for Git operations during build.

```dockerfile
RUN --mount=type=ssh \
    git clone git@github.com:private/repo.git /app
```

Build command:
```bash
docker build --ssh default .
```

### Bind Mounts (`--mount=type=bind`)

Mount context files without creating a COPY layer. Read-only by default.

```dockerfile
# Mount entire build context
RUN --mount=type=bind,target=. go build -o /app/hello

# Mount from another stage
RUN --mount=type=bind,target=/source,from=build go build ...

# Mount specific file (avoids COPY layer for requirements)
RUN --mount=type=bind,source=requirements.txt,target=/tmp/requirements.txt \
    pip install --requirement /tmp/requirements.txt
```

**Key advantage:** Mounted files are NOT persisted in the final image. Only the RUN output is kept.

### Tmpfs Mounts

Temporary filesystem for build operations:

```dockerfile
RUN --mount=type=tmpfs,target=/tmp gcc -o /app/binary source.c
```

### BuildKit Inline Cache

Embed cache metadata into the image itself:

```dockerfile
ARG BUILDKIT_INLINE_CACHE=1
```

Or via build command:
```bash
docker build --build-arg BUILDKIT_INLINE_CACHE=1 -t myapp:latest .
```

### Parallel Build Stages

BuildKit automatically detects independent stages and builds them in parallel. It uses a fully concurrent build graph solver.

Key behaviors:
- Stages without dependencies on each other run simultaneously.
- `--target` only builds the target stage and its dependencies (BuildKit skips unneeded stages).
- The legacy builder processes ALL stages sequentially up to the target.

### Predefined Platform ARGs

Automatically available in BuildKit without explicit declaration:

```dockerfile
FROM --platform=$BUILDPLATFORM golang:1.22 AS build
ARG TARGETOS TARGETARCH
RUN GOOS=$TARGETOS GOARCH=$TARGETARCH go build -o /app

FROM alpine:3.19
COPY --from=build /app /usr/bin/app
```

---

## 4. Multi-Stage Build Patterns

Source: https://docs.docker.com/build/building/multi-stage/

### Basic Builder Pattern

Separate build dependencies from the final runtime image:

```dockerfile
# syntax=docker/dockerfile:1

# Stage 1: Build
FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN go build -o /bin/hello ./cmd

# Stage 2: Runtime (minimal image)
FROM scratch
COPY --from=build /bin/hello /bin/hello
CMD ["/bin/hello"]
```

Result: a tiny production image containing only the binary. No compilers, SDKs, or source code.

### Named Stages (`AS`)

ALWAYS name stages for maintainability:

```dockerfile
FROM golang:1.22 AS build
# ...

FROM alpine:3.19 AS runtime
COPY --from=build /app /usr/bin/app
```

Named references survive instruction reordering — numeric indexes (`--from=0`) break when stages are added/removed.

### COPY --from Sources

```dockerfile
# From a named stage
COPY --from=build /app /usr/bin/app

# From an external image (auto-pulled)
COPY --from=nginx:latest /etc/nginx/nginx.conf /nginx.conf

# From a numbered stage (fragile, avoid)
COPY --from=0 /bin/hello /bin/hello
```

### Parallel Build Stages

Stages that share a common base but are independent run in parallel:

```dockerfile
FROM alpine:latest AS builder
RUN apk --no-cache add build-base

FROM builder AS build1
COPY source1.cpp source.cpp
RUN g++ -o /binary source.cpp

FROM builder AS build2
COPY source2.cpp source.cpp
RUN g++ -o /binary source.cpp
```

`build1` and `build2` run simultaneously because neither depends on the other.

### Target Stage Selection (`--target`)

Build only a specific stage (and its dependencies):

```bash
docker build --target build -t my-build-stage .
```

Use cases:
- **Debug stage** with debugging tools (not shipped to production).
- **Test stage** with test data and test runners.
- **Production stage** — lean, minimal, secure.

**BuildKit optimization:** When targeting `stage2`, BuildKit skips `stage1` if `stage2` doesn't depend on it. The legacy builder would process all stages up to the target.

### Common Multi-Stage Patterns

**Build + Test + Production:**
```dockerfile
# syntax=docker/dockerfile:1

FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN go build -o /app ./cmd

FROM build AS test
RUN go test ./...

FROM alpine:3.19 AS production
COPY --from=build /app /usr/bin/app
USER nobody:nobody
ENTRYPOINT ["/usr/bin/app"]
```

**Shared dependency base:**
```dockerfile
FROM node:20 AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci

FROM deps AS build
COPY . .
RUN npm run build

FROM node:20-slim AS production
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY --from=build /app/dist ./dist
CMD ["node", "dist/index.js"]
```

---

## 5. Layer Optimization & Caching

Sources:
- https://docs.docker.com/build/cache/
- https://docs.docker.com/build/cache/invalidation/
- https://docs.docker.com/build/cache/optimize/
- https://docs.docker.com/build/cache/backends/

### How Layer Caching Works

- Each Dockerfile instruction creates a layer.
- Docker checks each instruction against its cache before executing.
- If the instruction string (and relevant inputs) match a cached layer, the cached version is used.
- **Critical rule:** Once a layer's cache is invalidated, ALL subsequent layers must rebuild.

### Cache Invalidation Triggers

| Instruction | Cache Key | Invalidation Trigger |
|-------------|-----------|---------------------|
| FROM | Image reference | Base image changed |
| RUN | Command string | Command text changed, or preceding layer invalidated |
| COPY | File checksums | File content changed (mtime is NOT checked) |
| ADD | File checksums + URL content | File content changed, URL content changed |
| ENV | Key=Value | Value changed |
| ARG | Name=Value | Value changed |
| WORKDIR | Path + `SOURCE_DATE_EPOCH` | Path changed, or `SOURCE_DATE_EPOCH` changed |

**Important:** For COPY/ADD, Docker uses file content checksums, NOT modification timestamps. Touching a file without changing its content does NOT invalidate the cache.

**Important:** For RUN, Docker only checks the command string. External resources (e.g., `apt-get install` fetching new package versions) do NOT trigger invalidation.

### Instruction Ordering for Cache Efficiency

**Principle:** Order instructions from LEAST frequently changed to MOST frequently changed. Put expensive operations early.

**Before (poor cache usage):**
```dockerfile
FROM node
WORKDIR /app
COPY . .                  # ANY file change invalidates everything below
RUN npm install           # Reinstalls on every code change
RUN npm build
```

**After (optimized):**
```dockerfile
FROM node
WORKDIR /app
COPY package.json yarn.lock .   # Only dependency changes trigger install
RUN npm install
COPY . .                        # Code changes only affect build step
RUN npm build
```

### .dockerignore Patterns

Exclude files from the build context. Smaller context = faster builds.

```
# Version control
.git
.gitignore

# Dependencies (rebuilt in container)
node_modules
vendor

# Build artifacts
dist
build
*.o
*.pyc
__pycache__

# IDE and OS files
.vscode
.idea
*.swp
.DS_Store
Thumbs.db

# Docker files (not needed in context)
Dockerfile
docker-compose*.yml
.dockerignore

# Documentation
*.md
LICENSE

# Environment and secrets
.env
.env.*
*.pem
*.key
```

### Bind Mounts for Large Build Contexts

When source code is only needed to produce an artifact, use bind mounts instead of COPY:

```dockerfile
FROM golang:latest
WORKDIR /build
RUN --mount=type=bind,target=. go build -o /app/hello
```

Advantages:
- Mounted files are NOT persisted in any layer.
- Only the RUN output is kept.
- Avoids bloating the build cache with source files.

### Forcing Cache Invalidation

```bash
# Invalidate ALL cache
docker build --no-cache .

# Invalidate specific stage cache
docker build --no-cache-filter install .

# Clear builder cache
docker builder prune

# Pull fresh base image
docker build --pull .

# Both fresh base + no cache
docker build --pull --no-cache .
```

### Cache Backends

Docker Buildx supports multiple cache storage backends:

| Backend | Description | Use Case |
|---------|-------------|----------|
| **Inline** | Embedded in the output image | Simple, works with any registry |
| **Registry** | Separate image at dedicated tag | CI/CD, team sharing |
| **Local** | Filesystem directory | Local development |
| **GitHub Actions** (gha) | GitHub Actions cache service | GitHub CI/CD (beta) |
| **AWS S3** | S3 bucket storage | AWS environments (unreleased) |
| **Azure Blob** | Azure Blob Storage | Azure environments (unreleased) |

**Cache modes:**
- `min` (default) — Only caches exported layers. Smaller, faster.
- `max` — Caches ALL layers including intermediates. More cache hits.

**Registry cache example:**
```bash
# Export cache
docker buildx build --push -t registry/app:latest \
  --cache-to type=registry,ref=registry/app:buildcache,mode=max \
  --cache-from type=registry,ref=registry/app:buildcache .

# Multiple cache sources (branch + main)
docker buildx build --push -t registry/app:latest \
  --cache-to type=registry,ref=registry/app:buildcache:feature \
  --cache-from type=registry,ref=registry/app:buildcache:feature \
  --cache-from type=registry,ref=registry/app:buildcache:main .
```

**GitHub Actions CI/CD cache example:**
```yaml
name: ci
on: push
jobs:
  docker:
    runs-on: ubuntu-latest
    steps:
      - uses: docker/login-action@v4
        with:
          username: ${{ vars.DOCKERHUB_USERNAME }}
          password: ${{ secrets.DOCKERHUB_TOKEN }}
      - uses: docker/setup-buildx-action@v4
      - uses: docker/build-push-action@v7
        with:
          push: true
          tags: user/app:latest
          cache-from: type=registry,ref=user/app:buildcache
          cache-to: type=registry,ref=user/app:buildcache,mode=max
```

---

## 6. Best Practices

Source: https://docs.docker.com/build/building/best-practices/

### Base Image Selection

| Image Type | Size | Use Case |
|-----------|------|----------|
| `scratch` | 0 MB | Statically compiled binaries (Go, Rust) |
| `alpine` | ~6 MB | Minimal Linux with apk package manager |
| `*-slim` (e.g., `debian:bookworm-slim`) | ~30-80 MB | Reduced Debian/Ubuntu without extras |
| `distroless` | ~20 MB | Google's minimal images, no shell, no package manager |
| Full images (e.g., `ubuntu:22.04`) | ~75-200 MB | When you need a full OS (development, debugging) |

**Best practice:** Use a full image for the build stage and a minimal image for the runtime stage.

### Non-Root User Pattern

```dockerfile
RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser
USER appuser
```

- ALWAYS assign explicit UID/GID for deterministic behavior across rebuilds.
- Use `--no-log-init` to prevent `/var/log/faillog` from filling with NULL characters (Go archive/tar sparse file bug with large UIDs).
- Use `gosu` instead of `sudo` for proper signal forwarding and TTY behavior.

### Signal Handling & PID 1

- ALWAYS use exec form for ENTRYPOINT: `ENTRYPOINT ["executable"]`
- Shell form wraps in `/bin/sh -c`, preventing proper signal delivery to the application.
- In entrypoint scripts, use `exec "$@"` to replace the shell process with the application.
- The application MUST be PID 1 to receive SIGTERM and other signals for graceful shutdown.

### Metadata Labels (OCI Standard)

```dockerfile
LABEL org.opencontainers.image.title="My App"
LABEL org.opencontainers.image.description="Application description"
LABEL org.opencontainers.image.version="1.0.0"
LABEL org.opencontainers.image.authors="author@example.com"
LABEL org.opencontainers.image.url="https://example.com"
LABEL org.opencontainers.image.source="https://github.com/user/repo"
LABEL org.opencontainers.image.licenses="MIT"
LABEL org.opencontainers.image.created="2024-01-01T00:00:00Z"
```

### Reproducible Builds

**Pin base image by digest:**
```dockerfile
FROM alpine:3.21@sha256:a8560b36e8b8210634f77d9f7f9efd7ffa463e380b75e2e74aff4511df3ef88c
```

**Pin package versions:**
```dockerfile
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl=7.88.1-10+deb12u5 \
    git=1:2.39.2-1.1
```

**Pin with wildcard for patch versions:**
```dockerfile
RUN apt-get install -y --no-install-recommends s3cmd=1.1.*
```

**Fixed timestamps:**
```bash
docker build --build-arg SOURCE_DATE_EPOCH=0 .
```

### Pipe Error Handling

```dockerfile
# BAD: wget failure is masked by wc success
RUN wget -O - https://some.site | wc -l > /number

# GOOD: set pipefail so any command failure is caught
RUN set -o pipefail && wget -O - https://some.site | wc -l > /number

# GOOD: explicit bash when default shell is dash
RUN ["/bin/bash", "-c", "set -o pipefail && wget -O - https://some.site | wc -l > /number"]
```

### Ephemeral Container Design

- Containers should be stoppable, destroyable, and rebuildable with minimal setup.
- Follow the Twelve-Factor App methodology — processes are stateless.
- Each container should have a single concern.
- Use Docker networks for inter-container communication.

---

## 7. Common Anti-Patterns

### Running as Root

```dockerfile
# BAD: Container runs as root by default
FROM node:20
COPY . /app
CMD ["node", "app.js"]

# GOOD: Create and switch to non-root user
FROM node:20
RUN groupadd -r appuser && useradd -r -g appuser appuser
WORKDIR /app
COPY --chown=appuser:appuser . .
USER appuser
CMD ["node", "app.js"]
```

### Using `latest` Tag

```dockerfile
# BAD: Non-deterministic, different image on each build
FROM node:latest

# GOOD: Pin to specific version
FROM node:20.11-bookworm-slim

# BEST: Pin to digest for full reproducibility
FROM node:20.11-bookworm-slim@sha256:abc123...
```

### Not Cleaning apt Cache

```dockerfile
# BAD: Cache left in image, wastes ~30-100MB per install
RUN apt-get update
RUN apt-get install -y curl

# GOOD: Combined + cleaned
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*
```

Note: Official Debian/Ubuntu images run `apt-get clean` automatically, but `/var/lib/apt/lists/*` must be removed manually.

### Separating apt-get update and install

```dockerfile
# BAD: Cached update layer becomes stale
RUN apt-get update
RUN apt-get install -y curl

# Later, adding a package:
RUN apt-get update         # Uses cache - doesn't actually update!
RUN apt-get install -y curl nginx  # May fail or use outdated packages

# GOOD: Always combine
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    nginx \
    && rm -rf /var/lib/apt/lists/*
```

### Large Build Contexts

```dockerfile
# BAD: Sends entire project directory (node_modules, .git, etc.)
# No .dockerignore file

# GOOD: Use .dockerignore
# .dockerignore:
# node_modules
# .git
# dist
# *.md
```

### Secrets in ENV/ARG

```dockerfile
# BAD: Secret visible in docker history and image layers
ENV API_KEY=sk-1234567890
ARG DATABASE_PASSWORD=secret123

# GOOD: Use secret mounts
RUN --mount=type=secret,id=api_key \
    cat /run/secrets/api_key | some-command
```

Build: `docker build --secret id=api_key,src=./api_key.txt .`

### Not Using .dockerignore

Without `.dockerignore`, the entire build context (often gigabytes) is sent to the builder, including:
- `node_modules/` (can be 500MB+)
- `.git/` (entire history)
- IDE configs, OS files
- Test data, documentation

ALWAYS create a `.dockerignore` file.

### Too Many Layers

```dockerfile
# BAD: Each RUN creates a separate layer
RUN apt-get update
RUN apt-get install -y curl
RUN apt-get install -y git
RUN rm -rf /var/lib/apt/lists/*

# GOOD: Single layer for related operations
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    git \
    && rm -rf /var/lib/apt/lists/*
```

### ENV Persistence Leak

```dockerfile
# BAD: Variable persists in final image even after unset
ENV ADMIN_USER="mark"
RUN echo $ADMIN_USER > ./mark
RUN unset ADMIN_USER  # Does NOT remove from image!

# GOOD: Use temporary variable within single RUN
RUN export ADMIN_USER="mark" \
    && echo $ADMIN_USER > ./mark \
    && unset ADMIN_USER
```

### Using ADD When COPY Suffices

```dockerfile
# BAD: ADD has implicit behaviors (auto-extraction, URL download)
ADD config.json /app/config.json

# GOOD: COPY is explicit and predictable
COPY config.json /app/config.json
```

Use ADD only when you specifically need its extra features (tar extraction, URL download, Git clone).

### Shell Form ENTRYPOINT

```dockerfile
# BAD: Application is NOT PID 1, signals not forwarded
ENTRYPOINT /usr/bin/myapp

# GOOD: Application IS PID 1, proper signal handling
ENTRYPOINT ["/usr/bin/myapp"]
```

### Using cd Instead of WORKDIR

```dockerfile
# BAD: cd in RUN doesn't persist, fragile
RUN cd /app && npm install

# GOOD: WORKDIR persists and self-documents
WORKDIR /app
RUN npm install
```

---

## Appendix: Complete BuildKit Example

```dockerfile
# syntax=docker/dockerfile:1

# ---- Build Stage ----
FROM --platform=$BUILDPLATFORM golang:1.22-alpine AS build

ARG TARGETOS TARGETARCH
ARG VERSION=dev

WORKDIR /src

# Cache dependency download
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=bind,source=go.sum,target=go.sum \
    --mount=type=bind,source=go.mod,target=go.mod \
    go mod download

# Build with cached build artifacts
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    --mount=type=bind,target=. \
    GOOS=$TARGETOS GOARCH=$TARGETARCH go build \
    -ldflags "-X main.version=$VERSION" \
    -o /bin/app ./cmd

# ---- Runtime Stage ----
FROM alpine:3.19 AS runtime

# Security: non-root user
RUN addgroup -S appgroup && adduser -S appuser -G appgroup

# Metadata
LABEL org.opencontainers.image.title="My App" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.authors="team@example.com"

# Copy only the binary
COPY --from=build /bin/app /usr/bin/app

# Health check
HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

# Non-root execution
USER appuser:appgroup

EXPOSE 8080

ENTRYPOINT ["/usr/bin/app"]
CMD ["--config", "/etc/app/config.yaml"]
```

Build command:
```bash
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --build-arg VERSION=1.0.0 \
  --secret id=api_key,src=./api_key.txt \
  --cache-from type=registry,ref=registry/app:buildcache \
  --cache-to type=registry,ref=registry/app:buildcache,mode=max \
  --push \
  -t registry/app:1.0.0 \
  -t registry/app:latest \
  .
```
