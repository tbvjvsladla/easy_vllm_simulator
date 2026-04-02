---
name: docker-syntax-dockerfile
description: >
  Use when writing or reviewing Dockerfiles, choosing between CMD and
  ENTRYPOINT, or selecting COPY vs ADD.
  Prevents shell-form CMD that blocks signal propagation, ADD for local files
  where COPY suffices, and missing HEALTHCHECK in production images.
  Covers all 17 Dockerfile instructions: FROM, RUN, CMD, ENTRYPOINT, COPY,
  ADD, ENV, ARG, EXPOSE, VOLUME, WORKDIR, USER, HEALTHCHECK, LABEL, SHELL,
  STOPSIGNAL, ONBUILD, and parser directives.
  Keywords: FROM, RUN, CMD, ENTRYPOINT, COPY, ADD, HEALTHCHECK, USER,
  WORKDIR, ARG, ENV, # syntax=docker/dockerfile:1, CMD vs ENTRYPOINT,
  when to use, Dockerfile reference, Dockerfile best practices,
  write a Dockerfile.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+ with BuildKit."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-syntax-dockerfile

## Quick Reference

### Parser Directives

Parser directives MUST appear at the very top of the Dockerfile, before any instructions, blank lines, or comments.

| Directive | Purpose | Example |
|-----------|---------|---------|
| `# syntax=docker/dockerfile:1` | Enable BuildKit features (heredocs, mounts, etc.) | ALWAYS include this |
| `# escape=\`` | Change escape character (useful on Windows) | Optional |
| `# check=error=true` | Enable build-time lint checks (v1.8.0+) | Optional |

**ALWAYS** start every Dockerfile with `# syntax=docker/dockerfile:1` to enable BuildKit extensions.

### All 17 Instructions at a Glance

| Instruction | Purpose | Creates Layer? |
|-------------|---------|---------------|
| `FROM` | Set base image, start build stage | Yes (base) |
| `RUN` | Execute command during build | Yes |
| `CMD` | Default container command (overridable) | No (metadata) |
| `ENTRYPOINT` | Container executable (persistent) | No (metadata) |
| `COPY` | Copy files from context or stage | Yes |
| `ADD` | Copy with URL download and tar extraction | Yes |
| `ENV` | Set persistent environment variable | Yes |
| `ARG` | Set build-time variable (not persisted) | No |
| `WORKDIR` | Set working directory | Yes |
| `EXPOSE` | Document container port | No (metadata) |
| `VOLUME` | Declare mount point | No (metadata) |
| `USER` | Set user for subsequent instructions | No (metadata) |
| `HEALTHCHECK` | Define container health test | No (metadata) |
| `LABEL` | Add image metadata | Yes |
| `SHELL` | Override default shell | No (metadata) |
| `STOPSIGNAL` | Set container stop signal | No (metadata) |
| `ONBUILD` | Deferred instruction for child images | No (metadata) |

### Critical Warnings

**NEVER** use `latest` tag in FROM -- ALWAYS pin to a specific version (`node:20.11-bookworm-slim`) or digest for reproducibility.

**NEVER** store secrets in ENV or ARG -- they are visible in `docker history`. ALWAYS use `RUN --mount=type=secret` instead.

**NEVER** use ADD when COPY suffices -- ADD has implicit behaviors (auto-extraction, URL download) that make builds less predictable.

**NEVER** use shell form for ENTRYPOINT -- the application will NOT be PID 1 and will NOT receive signals for graceful shutdown.

**NEVER** separate `apt-get update` and `apt-get install` into different RUN instructions -- the update layer gets cached and becomes stale.

**ALWAYS** combine related RUN commands with `&&` to minimize layers.

**ALWAYS** clean up package manager caches in the same RUN layer as the install.

---

## Shell Form vs Exec Form

Three instructions support both forms: `RUN`, `CMD`, `ENTRYPOINT`.

| Form | Syntax | Shell Processing | Variable Expansion | Signal Handling |
|------|--------|-----------------|-------------------|----------------|
| **Shell** | `CMD command arg1` | Yes (`/bin/sh -c`) | Yes (`$VAR` works) | App is NOT PID 1 |
| **Exec** | `CMD ["command", "arg1"]` | No (direct exec) | No (use `ENV` for vars) | App IS PID 1 |

**ALWAYS** use exec form for `CMD` and `ENTRYPOINT` in production images.

Use shell form for `RUN` when you need variable expansion, pipes, or command chaining.

---

## CMD vs ENTRYPOINT Interaction Matrix

| | No ENTRYPOINT | ENTRYPOINT (shell form) | ENTRYPOINT (exec form) |
|---|---|---|---|
| **No CMD** | Error -- no command | `/bin/sh -c entrypoint_cmd` | `entrypoint_cmd` |
| **CMD (exec form)** | `cmd_executable args` | `/bin/sh -c entrypoint_cmd` (CMD ignored) | `entrypoint_cmd cmd_args` |
| **CMD (shell form)** | `/bin/sh -c cmd_string` | `/bin/sh -c entrypoint_cmd` (CMD ignored) | `entrypoint_cmd /bin/sh -c cmd_string` |

**Best practice pattern:**
```dockerfile
ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["default-command"]
```

- `ENTRYPOINT` (exec form) sets the fixed executable.
- `CMD` (exec form) provides default arguments, overridable via `docker run`.
- Shell form ENTRYPOINT ALWAYS ignores CMD -- NEVER combine them.

---

## COPY vs ADD Decision Guide

| Use Case | Instruction | Why |
|----------|-------------|-----|
| Copy local files | **COPY** | Explicit, predictable, no side effects |
| Copy from build stage | **COPY --from** | Only option for multi-stage copies |
| Download remote file with checksum | **ADD --checksum** | Integrity verification built in |
| Clone a Git repository | **ADD** (Git URL) | Supports branch/tag/commit references |
| Extract a local tar archive | **ADD** | Auto-extracts tar, tar.gz, tar.bz2, tar.xz |
| Everything else | **COPY** | ALWAYS prefer COPY by default |

**ALWAYS** prefer COPY unless you specifically need ADD's extra features.

---

## ENV vs ARG Comparison

| Property | ENV | ARG |
|----------|-----|-----|
| Available during build | Yes | Yes |
| Available at runtime | Yes | **No** |
| Visible in final image | Yes (`docker inspect`) | **No** |
| Visible in history | Yes | Yes (NEVER put secrets here) |
| Overridable | `docker run --env` | `docker build --build-arg` |
| Scope | Current + subsequent stages | Current stage only |
| Creates layer | Yes | No |
| Survives FROM | Yes (inherited) | **No** (must re-declare) |

**ALWAYS** use ARG for build-time-only values (version numbers, build flags).
**ALWAYS** use ENV for values needed at container runtime (PATH, config).

---

## HEALTHCHECK Syntax

```dockerfile
HEALTHCHECK [OPTIONS] CMD <command>
HEALTHCHECK NONE
```

| Option | Default | Description |
|--------|---------|-------------|
| `--interval=DURATION` | 30s | Time between checks |
| `--timeout=DURATION` | 30s | Max time for single check |
| `--start-period=DURATION` | 0s | Grace period on startup |
| `--retries=N` | 3 | Consecutive failures before unhealthy |

**Exit codes:** 0 = healthy, 1 = unhealthy, 2 = reserved (NEVER use).

```dockerfile
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:8080/health || exit 1
```

**ALWAYS** set `--start-period` for applications with slow startup.

---

## ONBUILD Triggers

```dockerfile
ONBUILD ADD . /app/src
ONBUILD RUN /app/src/compile.sh
```

- NOT executed in the current build -- fires in child images using `FROM <this-image>`.
- Useful for language-stack base images.
- ONBUILD ONBUILD is NOT allowed (no chaining).
- ONBUILD FROM and ONBUILD MAINTAINER are NOT allowed.

---

## RUN Mount Types (BuildKit)

| Mount Type | Purpose | Key Flags |
|------------|---------|-----------|
| `--mount=type=cache` | Persist package manager caches | `target`, `sharing`, `id` |
| `--mount=type=bind` | Mount context without COPY layer | `target`, `from`, `source` |
| `--mount=type=secret` | Access secrets without baking in | `id`, `target`, `env` |
| `--mount=type=ssh` | Forward SSH agent | `id` |
| `--mount=type=tmpfs` | Temporary filesystem | `target` |

See [references/instructions.md](references/instructions.md) for complete mount syntax and examples.

---

## Variable Substitution

Supported in: `ADD`, `COPY`, `ENV`, `EXPOSE`, `FROM`, `LABEL`, `STOPSIGNAL`, `USER`, `VOLUME`, `WORKDIR`, `ONBUILD`.

**NOT** supported in: `RUN` exec form, `CMD` exec form, `ENTRYPOINT` exec form (use shell form or ENV).

| Modifier | Example | Result |
|----------|---------|--------|
| Default value | `${VAR:-default}` | Use `default` if VAR unset |
| Alternate value | `${VAR:+alternate}` | Use `alternate` if VAR is set |
| Remove prefix | `${VAR#pattern}` | Remove shortest prefix match |
| Remove suffix | `${VAR%pattern}` | Remove shortest suffix match |

---

## Reference Links

- [references/instructions.md](references/instructions.md) -- Complete syntax and parameters for all 17 instructions
- [references/examples.md](references/examples.md) -- Production-ready Dockerfile examples for common scenarios
- [references/anti-patterns.md](references/anti-patterns.md) -- Instruction misuse patterns with corrections

### Official Sources

- https://docs.docker.com/reference/dockerfile/
- https://docs.docker.com/build/building/best-practices/
- https://docs.docker.com/build/building/multi-stage/
- https://docs.docker.com/build/buildkit/
