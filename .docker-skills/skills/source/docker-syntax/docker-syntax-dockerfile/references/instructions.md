# Dockerfile Instructions -- Complete Reference

> All syntax verified against https://docs.docker.com/reference/dockerfile/
> Requires: `# syntax=docker/dockerfile:1` parser directive for BuildKit features.

---

## FROM

Initializes a new build stage and sets the base image. MUST be the first instruction after parser directives and global ARGs.

### Syntax

```dockerfile
FROM [--platform=<platform>] <image> [AS <name>]
FROM [--platform=<platform>] <image>[:<tag>] [AS <name>]
FROM [--platform=<platform>] <image>[@<digest>] [AS <name>]
```

### Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `<image>` | Yes | Base image name |
| `:<tag>` | No | Image tag (defaults to `latest`) |
| `@<digest>` | No | Pin to exact image digest |
| `--platform=<platform>` | No | Target platform (`linux/amd64`, `linux/arm64`, etc.) |
| `AS <name>` | No | Name the build stage for `COPY --from=<name>` |

### Behaviors

- Multiple FROM instructions create multi-stage builds.
- Each FROM clears all prior state (layers, ENV, ARG within stage).
- ARG instructions before FROM are available in the FROM line but NOT in subsequent instructions unless re-declared with `ARG <name>` (no default needed).
- Tag defaults to `latest` when omitted -- ALWAYS specify an explicit tag.

### Examples

```dockerfile
# Pinned version
FROM ubuntu:22.04

# Named stage for multi-stage
FROM golang:1.22 AS builder

# Platform-specific
FROM --platform=linux/arm64 alpine:3.19

# Digest-pinned for reproducibility
FROM alpine:3.21@sha256:a8560b36e8b8210634f77d9f7f9efd7ffa463e380b75e2e74aff4511df3ef88c

# Dynamic base via global ARG
ARG BASE_IMAGE=ubuntu:22.04
FROM ${BASE_IMAGE} AS runtime
```

---

## RUN

Executes commands during build in a new layer.

### Syntax Forms

```dockerfile
# Shell form (processed by /bin/sh -c)
RUN <command>

# Exec form (no shell processing)
RUN ["executable", "param1", "param2"]

# Heredoc form (BuildKit)
RUN <<EOF
commands here
EOF
```

### Mount Options (BuildKit)

#### Cache Mount

Persists package manager caches across builds.

```dockerfile
RUN --mount=type=cache,target=<path>[,id=<id>][,sharing=<shared|private|locked>][,from=<stage>][,source=<path>][,mode=<mode>][,uid=<uid>][,gid=<gid>] <command>
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| `target` | Required | Directory to cache |
| `id` | `target` value | Cache identifier |
| `sharing` | `shared` | `shared`, `private`, or `locked` |
| `from` | -- | Source stage for initial cache content |
| `source` | -- | Source path within the from stage |
| `mode` | `0755` | Permissions on cache directory |
| `uid` | `0` | Owner user ID |
| `gid` | `0` | Owner group ID |

**Package manager cache targets:**

| Package Manager | Cache Target(s) | Sharing |
|----------------|-----------------|---------|
| apt | `/var/cache/apt` + `/var/lib/apt` | `locked` (required) |
| npm | `/root/.npm` | `shared` |
| pip | `/root/.cache/pip` | `shared` |
| Go | `/go/pkg/mod` + `/root/.cache/go-build` | `shared` |
| Cargo (Rust) | `/app/target/` + `/usr/local/cargo/git/db` + `/usr/local/cargo/registry/` | `shared` |
| Bundler (Ruby) | `/root/.gem` | `shared` |
| NuGet (.NET) | `/root/.nuget/packages` | `shared` |
| Composer (PHP) | `/tmp/cache` | `shared` |

#### Bind Mount

Mount context files without creating a COPY layer. Read-only by default.

```dockerfile
RUN --mount=type=bind,target=<path>[,source=<path>][,from=<stage|image>][,rw] <command>
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| `target` | Required | Mount point inside the build container |
| `source` | `.` | Source path in the context or stage |
| `from` | Build context | Source stage or external image |
| `rw` | -- | Make mount read-write |

#### Secret Mount

Access secrets without baking into any layer.

```dockerfile
RUN --mount=type=secret,id=<id>[,target=<path>][,env=<varname>][,required][,mode=<mode>][,uid=<uid>][,gid=<gid>] <command>
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| `id` | Required | Secret identifier (matches `--secret id=` in build command) |
| `target` | `/run/secrets/<id>` | File path for the secret |
| `env` | -- | Expose as environment variable instead of file |
| `required` | `false` | Fail build if secret not provided |
| `mode` | `0400` | File permissions |

Build command: `docker build --secret id=mytoken,src=./token.txt .`

#### SSH Mount

Forward the host SSH agent for Git operations.

```dockerfile
RUN --mount=type=ssh[,id=<id>][,target=<path>][,required][,mode=<mode>][,uid=<uid>][,gid=<gid>] <command>
```

Build command: `docker build --ssh default .`

#### Tmpfs Mount

Temporary filesystem, discarded after RUN completes.

```dockerfile
RUN --mount=type=tmpfs,target=<path>[,size=<bytes>] <command>
```

### Other RUN Options

| Option | Values | Min Version | Description |
|--------|--------|-------------|-------------|
| `--network` | `default`, `none`, `host` | 1.3 | Control network access during build |
| `--security` | `sandbox`, `insecure` | 1.20 | Security mode for the build step |

### Key Behaviors

- Shell form uses `/bin/sh -c` by default (configurable via SHELL).
- Exec form does NOT invoke a shell -- no variable expansion, no pipes, no glob.
- Each RUN creates a new layer -- combine commands with `&&` to minimize layers.
- Cache invalidation checks the command string only, NOT external resources.

---

## CMD

Default command when a container starts. Does NOT execute during build.

### Syntax Forms

```dockerfile
# Exec form (PREFERRED)
CMD ["executable", "param1", "param2"]

# Default parameters for ENTRYPOINT
CMD ["param1", "param2"]

# Shell form
CMD command param1 param2
```

### Behaviors

- Only the LAST CMD in a Dockerfile takes effect.
- Overridden entirely by arguments passed to `docker run`.
- When combined with exec-form ENTRYPOINT, CMD provides default arguments.
- Shell form wraps in `/bin/sh -c` -- the shell becomes PID 1, not the application.

---

## ENTRYPOINT

Configures the container to run as an executable.

### Syntax Forms

```dockerfile
# Exec form (PREFERRED)
ENTRYPOINT ["executable", "param1", "param2"]

# Shell form
ENTRYPOINT command param1 param2
```

### Behaviors

- Exec form: `docker run` arguments are APPENDED to ENTRYPOINT.
- Shell form: `docker run` arguments are IGNORED. Runs under `/bin/sh -c`.
- Only the LAST ENTRYPOINT takes effect.
- Override at runtime with `docker run --entrypoint`.

### Best Practice -- Entrypoint Script

```dockerfile
COPY --chmod=755 docker-entrypoint.sh /
ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["postgres"]
```

```bash
#!/bin/bash
set -e
# Initialization logic here
exec "$@"  # Replace shell with CMD arguments -- app becomes PID 1
```

---

## COPY

Copies files from build context or earlier build stages.

### Syntax

```dockerfile
COPY [OPTIONS] <src> ... <dest>
COPY [OPTIONS] ["<src>", ... "<dest>"]
```

### Options

| Option | Description | Min Version |
|--------|-------------|-------------|
| `--from=<stage\|image\|context>` | Copy from another stage, external image, or named context | -- |
| `--chown=<user>:<group>` | Set ownership (Linux only) | -- |
| `--chmod=<perms>` | Set file permissions (octal or symbolic) | 1.2 |
| `--link[=<boolean>]` | Enhanced layer reuse across rebuilds | 1.4 |
| `--parents[=<boolean>]` | Preserve parent directory structure | 1.7 |
| `--exclude=<pattern>` | Exclude matching paths | 1.7 |

### Behaviors

- Cache invalidation uses file content checksums (NOT modification timestamps).
- Destination ending with `/` is treated as a directory.
- Relative paths resolve against WORKDIR.
- Default permissions: 0644 for files, 0755 for directories.
- Glob patterns (`*.txt`, `src/*.go`) are supported in source paths.

### Examples

```dockerfile
COPY file1.txt /dest/
COPY *.json /app/
COPY --from=build /app/binary /usr/bin/
COPY --from=nginx:latest /etc/nginx/nginx.conf /nginx.conf
COPY --chmod=755 entrypoint.sh /
COPY --chown=appuser:appgroup config/ /app/config/
COPY --parents src/main.go src/utils.go /app/
COPY --exclude=*.test.go --exclude=*_test.go . /app/src/
COPY --link /app /app
```

---

## ADD

Like COPY but with URL download, Git clone, and tar auto-extraction.

### Syntax

```dockerfile
ADD [OPTIONS] <src> ... <dest>
ADD [OPTIONS] ["<src>", ... "<dest>"]
```

### Additional Options (beyond COPY)

| Option | Description | Min Version |
|--------|-------------|-------------|
| `--keep-git-dir=<boolean>` | Preserve `.git` directory when cloning | 1.1 |
| `--checksum=<hash>` | Verify integrity of remote sources | 1.6 |
| `--unpack=<boolean>` | Control auto-extraction of archives | 1.17 |

### Sources Supported

- Local files and directories
- HTTP/HTTPS URLs
- Git repositories (with branch/tag/commit refs)
- Local tar archives (auto-extracted: tar, tar.gz, tar.bz2, tar.xz)

### Examples

```dockerfile
# Remote file with checksum
ADD --checksum=sha256:24454f... https://example.com/archive.tar.gz /

# Git repository at specific tag
ADD https://github.com/moby/buildkit.git#v0.14.1:docs /buildkit-docs

# Disable auto-extraction
ADD --unpack=false my-archive.tar.gz .
```

---

## ENV

Sets environment variables that persist in the final image and at container runtime.

### Syntax

```dockerfile
ENV <key>=<value> [<key>=<value>...]
```

### Behaviors

- Persists in the final image (visible via `docker inspect`).
- Each ENV instruction creates a new layer.
- Overridable at runtime via `docker run --env KEY=VALUE`.
- Values inherit into child stages in multi-stage builds.
- Multiple assignments on one line use the values from BEFORE the line:

```dockerfile
ENV abc=hello
ENV abc=bye def=$abc   # def=hello (old value of abc)
ENV ghi=$abc           # ghi=bye (new value of abc)
```

---

## ARG

Build-time variables. NOT persisted in the final image.

### Syntax

```dockerfile
ARG <name>[=<default value>]
```

### Behaviors

- Scope is limited to the current build stage.
- MUST be re-declared after FROM to use within a stage.
- Values are visible in `docker history` -- NEVER use for secrets.
- Overridable via `docker build --build-arg NAME=VALUE`.

### Predefined Platform ARGs (BuildKit)

Automatically available without declaration:

| ARG | Example Value | Description |
|-----|---------------|-------------|
| `TARGETPLATFORM` | `linux/amd64` | Target platform |
| `TARGETOS` | `linux` | Target OS |
| `TARGETARCH` | `amd64` | Target architecture |
| `TARGETVARIANT` | `v7` | Target variant (e.g., ARM) |
| `BUILDPLATFORM` | `linux/amd64` | Build machine platform |
| `BUILDOS` | `linux` | Build machine OS |
| `BUILDARCH` | `amd64` | Build machine architecture |

### Predefined Proxy ARGs

Excluded from `docker history` by default:

`HTTP_PROXY`, `HTTPS_PROXY`, `FTP_PROXY`, `NO_PROXY`, `ALL_PROXY` (and lowercase variants).

### BuildKit Built-in ARGs

| ARG | Purpose |
|-----|---------|
| `BUILDKIT_INLINE_CACHE` | Enable inline cache metadata |
| `BUILDKIT_MULTI_PLATFORM` | Deterministic multi-platform output |
| `BUILDKIT_SANDBOX_HOSTNAME` | Set build hostname |
| `BUILDKIT_CONTEXT_KEEP_GIT_DIR` | Preserve `.git` in context |
| `BUILDKIT_CACHE_MOUNT_NS` | Cache ID namespace |

---

## WORKDIR

Sets the working directory for RUN, CMD, ENTRYPOINT, COPY, ADD.

### Syntax

```dockerfile
WORKDIR /path/to/workdir
```

### Behaviors

- Created automatically if it does not exist.
- Relative paths stack: `WORKDIR /a` then `WORKDIR b` then `WORKDIR c` results in `/a/b/c`.
- Supports environment variable expansion: `WORKDIR $DIRPATH`.
- ALWAYS use WORKDIR instead of `RUN cd /some/path && ...`.

---

## EXPOSE

Documents which ports the container listens on. Does NOT publish them.

### Syntax

```dockerfile
EXPOSE <port>[/<protocol>] [<port>[/<protocol>]...]
```

- Defaults to TCP if protocol is omitted.
- Ports are published at runtime with `docker run -p` or `-P`.
- Purely informational -- has no networking effect at build time.

---

## VOLUME

Creates a mount point for externally mounted volumes.

### Syntax

```dockerfile
VOLUME ["/data"]
VOLUME /var/log /var/db
```

### Behaviors

- Marks directories as externally mountable.
- Host directory is specified at container runtime, NOT in the Dockerfile.
- Any data written to a VOLUME path after the VOLUME instruction during build is DISCARDED.
- ALWAYS declare VOLUME for mutable or user-serviceable data (databases, logs).

---

## USER

Sets the user and optionally group for subsequent RUN, CMD, ENTRYPOINT.

### Syntax

```dockerfile
USER <user>[:<group>]
USER <UID>[:<GID>]
```

### Example

```dockerfile
RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser
USER appuser:appuser
```

- ALWAYS create the user before switching to it.
- Use `--no-log-init` to prevent sparse file issues with large UIDs.

---

## HEALTHCHECK

Defines how Docker tests whether the container is working.

### Syntax

```dockerfile
HEALTHCHECK [OPTIONS] CMD <command>
HEALTHCHECK NONE
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--interval=DURATION` | 30s | Time between checks |
| `--timeout=DURATION` | 30s | Max time for single check |
| `--start-period=DURATION` | 0s | Grace period on startup (failures don't count) |
| `--retries=N` | 3 | Consecutive failures to mark unhealthy |

### Exit Codes

| Code | Status |
|------|--------|
| 0 | Healthy |
| 1 | Unhealthy |
| 2 | Reserved -- NEVER use |

### Example

```dockerfile
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:8080/health || exit 1
```

- Only the LAST HEALTHCHECK takes effect.
- `HEALTHCHECK NONE` disables any health check inherited from a base image.

---

## LABEL

Adds metadata key-value pairs to the image.

### Syntax

```dockerfile
LABEL <key>=<value> [<key>=<value>...]
```

### OCI Standard Labels

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

- Replaces deprecated MAINTAINER instruction.
- View with `docker image inspect --format='{{json .Config.Labels}}' <image>`.
- ALWAYS use OCI standard label keys where applicable.

---

## SHELL

Overrides the default shell for shell-form commands.

### Syntax

```dockerfile
SHELL ["executable", "parameters"]
```

### Examples

```dockerfile
# Linux -- switch to bash
SHELL ["/bin/bash", "-c"]
RUN echo "Now using bash"

# Windows -- switch to PowerShell
SHELL ["powershell", "-Command"]
RUN Write-Host 'Hello from PowerShell'
```

- Default on Linux: `["/bin/sh", "-c"]`
- Default on Windows: `["cmd", "/S", "/C"]`
- Affects all subsequent shell-form RUN, CMD, ENTRYPOINT.

---

## STOPSIGNAL

Sets the system call signal sent to the container to exit.

### Syntax

```dockerfile
STOPSIGNAL <signal>
```

- Signal can be a name (`SIGTERM`) or number (`15`).
- Default is `SIGTERM`.
- Override at runtime with `docker run --stop-signal`.

---

## ONBUILD

Adds a trigger instruction executed when the image is used as a base for another build.

### Syntax

```dockerfile
ONBUILD <INSTRUCTION>
```

### Behaviors

- NOT executed in the current build.
- Fires in child images that use `FROM <this-image>`.
- Useful for language-stack base images (e.g., `ruby:2.0-onbuild`).
- ONBUILD triggers execute immediately after the child's FROM instruction.

### Restrictions

- `ONBUILD ONBUILD` is NOT allowed (no chaining).
- `ONBUILD FROM` is NOT allowed.
- `ONBUILD MAINTAINER` is NOT allowed.

### Example

```dockerfile
# Base image for Node.js apps
ONBUILD COPY package.json /app/
ONBUILD RUN npm install
ONBUILD COPY . /app/
```

---

## Environment Variable Substitution

Variables (`$variable` or `${variable}`) are supported in these instructions:
`ADD`, `COPY`, `ENV`, `EXPOSE`, `FROM`, `LABEL`, `STOPSIGNAL`, `USER`, `VOLUME`, `WORKDIR`, `ONBUILD`.

### Modifiers

| Modifier | Syntax | Result |
|----------|--------|--------|
| Default value | `${variable:-default}` | Use `default` if variable is unset |
| Alternate value | `${variable:+alternate}` | Use `alternate` if variable is set |
| Remove shortest prefix | `${variable#pattern}` | Strip shortest match from start |
| Remove longest prefix | `${variable##pattern}` | Strip longest match from start |
| Remove shortest suffix | `${variable%pattern}` | Strip shortest match from end |
| Remove longest suffix | `${variable%%pattern}` | Strip longest match from end |
| Replace first | `${variable/find/replace}` | Replace first occurrence |
| Replace all | `${variable//find/replace}` | Replace all occurrences |

Escape with `\$foo` or `\${foo}` for literal dollar signs.
