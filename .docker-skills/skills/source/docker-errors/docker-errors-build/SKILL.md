---
name: docker-errors-build
description: >
  Use when debugging Docker build failures or unexpected cache behavior.
  Prevents wasted hours from misunderstanding COPY context paths, cache
  invalidation rules, and BuildKit mount syntax.
  Covers build errors, COPY/ADD not found, context too large, cache misses,
  ARG/ENV scope, multi-stage reference errors, platform mismatch.
  Keywords: docker build, COPY failed, cache invalidation, BuildKit,
  multi-stage, ARG, --mount, exec format error, build context,
  build fails, file not found during build, wrong platform, slow build.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-errors-build

## Quick Reference

### Build Error Debugging Workflow

```
Build fails
  |
  +-- Read the FULL error message (use --progress=plain for details)
  |
  +-- Identify error category:
  |     |
  |     +-- "file not found" / "checksum" --> COPY/ADD path issue (see 1.1)
  |     +-- "context" / "sending build context" slow --> Context too large (see 1.2)
  |     +-- Rebuilds unexpectedly --> Cache invalidation (see 1.3)
  |     +-- "mount" errors --> BuildKit mount config (see 1.4)
  |     +-- "invalid stage" / "not found" --> Multi-stage reference (see 1.5)
  |     +-- "pull access denied" / "manifest unknown" --> Base image (see 1.6)
  |     +-- "exec format error" --> Platform mismatch (see 1.7)
  |     +-- Variable empty / missing --> ARG scope issue (see 1.8)
  |     +-- "permission denied" --> Build permission (see 1.9)
  |     +-- "parse error" / "unknown instruction" --> Syntax error (see 1.10)
  |
  +-- Apply fix from diagnostic table
  |
  +-- Rebuild with: docker build --progress=plain --no-cache .
```

### Critical Warnings

**ALWAYS** use `--progress=plain` when debugging build failures -- the default TTY output hides important error details.

**ALWAYS** check `.dockerignore` first when files appear missing during COPY or ADD -- this is the #1 cause of "file not found" errors.

**NEVER** use `--no-cache` as a permanent fix for cache problems -- find and fix the root cause of cache invalidation instead.

**NEVER** put secrets in ARG or ENV instructions -- they persist in image history. ALWAYS use `RUN --mount=type=secret`.

---

## 1. Diagnostic Tables

### 1.1 COPY/ADD File Not Found

| Error Message | Cause | Fix |
|--------------|-------|-----|
| `COPY failed: file not found in build context` | File is outside the build context directory | Move file into the build context or restructure with `docker build -f path/Dockerfile context/` |
| `COPY failed: file not found in build context` | File is excluded by `.dockerignore` | Remove or adjust the `.dockerignore` pattern |
| `failed to compute cache key: failed to calculate checksum of ref` | COPY source path does not exist relative to build context root | Verify path with `ls` from the build context directory. Paths are relative to context, NOT Dockerfile location |
| `COPY failed: no source files were specified` | Glob pattern matches zero files | Check wildcard pattern. Verify files exist: `ls <pattern>` from context root |
| `ADD failed: file not found in build context` | Same causes as COPY | Same fixes as COPY. ALWAYS prefer COPY over ADD for local files |

**Debugging command:**
```bash
# List what the builder actually sees in the context
docker build --progress=plain -f Dockerfile . 2>&1 | head -5
# Shows: "sending build context to Docker daemon  X.XXkB"

# Check .dockerignore effect
cat .dockerignore
```

### 1.2 Build Context Too Large

| Error Message | Cause | Fix |
|--------------|-------|-----|
| `sending build context to Docker daemon` takes minutes | No `.dockerignore` or large files in context | Create `.dockerignore` excluding `node_modules/`, `.git/`, build artifacts |
| Context exceeds available memory | Extremely large context (multi-GB) | Use `.dockerignore`. Use `--file` with a smaller context path. Use multi-stage builds with bind mounts |

**ALWAYS create a `.dockerignore` file.** Without it, the ENTIRE directory tree is sent to the daemon, including `node_modules/` (500MB+), `.git/` (entire history), and build artifacts.

### 1.3 Unexpected Cache Invalidation

| Symptom | Cause | Fix |
|---------|-------|-----|
| `npm install` reruns on every build | `COPY . .` before `RUN npm install` | Copy `package.json` and lockfile FIRST, install, THEN copy source |
| `apt-get install` gets stale packages | `apt-get update` in a separate RUN layer | ALWAYS combine: `RUN apt-get update && apt-get install -y pkg` |
| Layer rebuilds after unrelated file change | COPY instruction too broad | Copy only the files needed for each step. Order from least to most frequently changed |
| Cache never hits in CI | No cache backend configured | Use `--cache-from` and `--cache-to` with registry or GHA backend |
| RUN layer rebuilds despite identical command | Previous layer was invalidated | Check ALL preceding layers -- cache invalidation cascades downward |

### 1.4 BuildKit Mount Errors

| Error Message | Cause | Fix |
|--------------|-------|-----|
| `failed to create LLB definition: rpc error: unknown flag: --mount` | Missing `# syntax=docker/dockerfile:1` directive | Add `# syntax=docker/dockerfile:1` as the FIRST line of the Dockerfile |
| `failed to create LLB definition: rpc error: unknown flag: --mount` | BuildKit not enabled (pre-Engine 23) | Set `DOCKER_BUILDKIT=1` or upgrade Docker Engine to 23+ |
| `failed to solve: failed to mount` cache target | Cache directory permissions or path issue | Verify target path. Add `uid` and `gid` options if running as non-root |
| `error: secret "X" not found` | Secret not passed to build command | Add `--secret id=X,src=path` to `docker build` command |
| `could not parse ssh: [default]: stat /nonexistent: no such file` | SSH agent not running or key not added | Run `eval $(ssh-agent)` and `ssh-add ~/.ssh/id_rsa` before build |
| `inconsistent result from cache mount` | Concurrent builds with `sharing=shared` on apt | Use `sharing=locked` for `apt` cache mounts |

### 1.5 Multi-Stage Reference Errors

| Error Message | Cause | Fix |
|--------------|-------|-----|
| `invalid from flag value X: invalid reference format` | Stage name contains uppercase or invalid characters | Use lowercase alphanumeric names with hyphens only |
| `failed to solve: X: not found` in `COPY --from=X` | Stage name does not exist or is misspelled | Verify the `AS name` in the FROM instruction matches exactly |
| `invalid stage index: N` | Numeric `--from=N` references a non-existent stage | Use named stages (`AS build`) instead of numeric indexes |
| `circular dependency detected` | Stage A copies from stage B which copies from A | Restructure stages to eliminate circular references |

### 1.6 Base Image Pull Failures

| Error Message | Cause | Fix |
|--------------|-------|-----|
| `pull access denied for X, repository does not exist` | Image name misspelled or does not exist | Verify image name on Docker Hub or registry. Check for typos |
| `manifest unknown: manifest unknown` | Tag does not exist for this image | Verify tag with `docker manifest inspect image:tag` |
| `unauthorized: authentication required` | Private registry requires login | Run `docker login <registry>` before building |
| `error pulling image configuration: download failed after attempts` | Network issue or registry timeout | Check network connectivity. Retry. Use `--pull` to force fresh pull |
| `toomanyrequests: You have reached your pull rate limit` | Docker Hub rate limit exceeded | Authenticate with `docker login` (free accounts get higher limits). Use a registry mirror |

### 1.7 Platform Mismatch

| Error Message | Cause | Fix |
|--------------|-------|-----|
| `exec format error` | Image built for wrong CPU architecture | Add `--platform linux/amd64` (or target arch) to build command |
| `image with reference X does not match the specified platform` | Pulled image has no manifest for requested platform | Check available platforms: `docker manifest inspect image:tag`. Build for available platform |
| `no match for platform in manifest list` | Multi-platform image lacks requested platform | Use a different base image that supports your platform |

### 1.8 ARG Scope Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| ARG value is empty inside build stage | ARG declared before FROM but not re-declared after | Re-declare `ARG varname` (without default) after each FROM that needs it |
| ARG value not available at runtime | ARG is build-time only, not persisted | Convert to ENV: `ARG VAR` then `ENV VAR=$VAR` if needed at runtime |
| Variable expansion not working in RUN | Using exec form `RUN ["echo", "$VAR"]` | Exec form does NOT expand variables. Use shell form: `RUN echo $VAR` |
| ENV set incorrectly using prior ENV value | Same-line ENV precedence | `ENV a=bye b=$a` uses `a`'s value from BEFORE this line. Split into separate ENV instructions if needed |

### 1.9 Permission Errors During Build

| Error Message | Cause | Fix |
|--------------|-------|-----|
| `permission denied` in RUN instruction | Running as non-root USER before installing packages | Place package installation BEFORE the `USER` instruction |
| `EACCES: permission denied` writing to directory | Directory owned by root, running as non-root | Add `RUN chown -R user:group /dir` BEFORE switching to USER |
| `permission denied` on COPY'd script | Script lacks execute permission | Add `COPY --chmod=755 script.sh /app/` or `RUN chmod +x /app/script.sh` |
| `open /var/lib/apt/lists/lock: permission denied` | Running apt as non-root user | Run apt commands BEFORE `USER` instruction, or use `--mount=type=cache` with correct uid/gid |

### 1.10 Dockerfile Syntax Errors

| Error Message | Cause | Fix |
|--------------|-------|-----|
| `failed to solve: dockerfile parse error on line X: unknown instruction: XXX` | Misspelled instruction or wrong case | Instructions MUST be uppercase: `RUN`, `COPY`, `FROM`, not `run`, `copy`, `from` |
| `failed to solve: dockerfile parse error: missing FROM` | No FROM instruction or FROM not first | FROM MUST be the first instruction (after parser directives and ARG) |
| Unexpected behavior with multi-line RUN | Missing `\` continuation character | End each continued line with `\`. NEVER leave trailing spaces after `\` |
| `failed to process "Dockerfile": no parser directive, file is empty` | BOM character or wrong encoding | Save Dockerfile as UTF-8 without BOM. Remove invisible characters |
| `COPY requires at least two arguments` | Missing destination argument | COPY needs `<src> <dest>`. Add trailing `/` for directories: `COPY files/ /app/` |

---

## 2. Decision Trees

### Which Cache Strategy to Use

```
Problem: Build is slow
  |
  +-- Dependencies reinstall every time?
  |     YES --> Reorder: copy lockfile first, install, then copy source
  |
  +-- Package downloads repeat?
  |     YES --> Add --mount=type=cache for your package manager
  |
  +-- CI builds have no cache?
  |     YES --> Configure --cache-from/--cache-to with registry backend
  |
  +-- Base image pulls every time?
  |     YES --> Pin base image digest. Use --cache-from=type=registry
  |
  +-- Entire Dockerfile rebuilds?
        YES --> Check if early layer changed. Order: system deps > app deps > source
```

### When to Use --no-cache vs --no-cache-filter

```
Need to force rebuild?
  |
  +-- Entire image from scratch?
  |     --> docker build --no-cache .
  |
  +-- Only specific stage?
  |     --> docker build --no-cache-filter=<stage-name> .
  |
  +-- Fresh base image only?
  |     --> docker build --pull .
  |
  +-- Everything fresh (CI release build)?
        --> docker build --pull --no-cache .
```

---

## 3. Essential Debug Commands

```bash
# Full build output (ALWAYS use this when debugging)
docker build --progress=plain .

# Build specific stage only
docker build --target <stage-name> .

# Check what the build context contains
tar -czf - -C <context-dir> . | wc -c

# Inspect build cache usage
docker system df
docker builder prune --filter type=regular
docker builder prune -a  # Remove ALL build cache

# Check image layers and sizes
docker history <image>
docker history --no-trunc <image>

# Verify platform of an image
docker inspect --format='{{.Os}}/{{.Architecture}}' <image>

# Check .dockerignore is working
# (compare context size with and without .dockerignore)
```

---

## Reference Links

- [references/diagnostics.md](references/diagnostics.md) -- Complete error message to cause to solution mapping with exact error strings
- [references/examples.md](references/examples.md) -- Error reproduction and fix examples with before/after Dockerfiles
- [references/anti-patterns.md](references/anti-patterns.md) -- Build configuration mistakes that cause errors

### Official Sources

- https://docs.docker.com/build/building/best-practices/
- https://docs.docker.com/build/cache/
- https://docs.docker.com/build/cache/invalidation/
- https://docs.docker.com/reference/dockerfile/
- https://docs.docker.com/build/buildkit/
- https://docs.docker.com/engine/daemon/troubleshoot/
