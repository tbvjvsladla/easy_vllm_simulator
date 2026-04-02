# Cache Invalidation Rules Per Instruction Type

> Reference file for docker-impl-build-optimization.
> Source: https://docs.docker.com/build/cache/invalidation/

---

## How Docker Layer Caching Works

1. Docker processes each instruction in order, top to bottom.
2. For each instruction, Docker checks whether a cached layer exists.
3. If the cache key matches, the cached layer is reused (cache HIT).
4. If the cache key does NOT match, the layer is rebuilt (cache MISS).
5. **Once a cache miss occurs, ALL subsequent layers MUST rebuild** -- even if their own cache keys have not changed.

This cascade behavior is the single most important rule for build optimization.

---

## Instruction-by-Instruction Cache Rules

### FROM

**Cache key:** Image reference (name + tag or digest).

**Invalidation triggers:**
- The image tag resolves to a different digest (e.g., `node:20` was updated on Docker Hub)
- The digest is explicitly different
- `--pull` flag is used (forces fresh pull and re-evaluation)

**Behavior:**
- With tag only: Docker checks if the local image matches the remote. If not pulled recently, uses local cache.
- With digest: Exact match required -- fully deterministic.
- `docker build --pull` forces a fresh pull, which may invalidate the FROM cache.

**Best practice:** Pin to digest for reproducible builds. Use tags for development convenience.

---

### RUN

**Cache key:** The command string (the exact text after `RUN`).

**Invalidation triggers:**
- The command text changes (even whitespace or comments)
- A preceding layer was invalidated (cascade)

**NOT an invalidation trigger:**
- External resource changes (package repository updates, remote file changes)
- Different output from the same command on a different day
- Environment variables set outside the Dockerfile

**Important details:**
- `RUN apt-get update` caches the layer. Running the same command days later still uses the cache, even though the package index is stale. This is why `apt-get update && apt-get install` MUST be in a single RUN.
- `RUN --mount=type=cache` does NOT affect cache key computation -- the mount target is separate from the layer cache.
- `RUN --mount=type=secret` does NOT invalidate cache when the secret content changes.

**Example -- same cache key:**
```dockerfile
# These two produce the SAME cache key:
RUN echo "hello"
RUN echo "hello"

# This produces a DIFFERENT cache key:
RUN echo "hello "  # trailing space
```

---

### COPY

**Cache key:** File content checksums of all source files.

**Invalidation triggers:**
- Any source file's content changed (even one byte)
- Files were added or removed from the source glob pattern
- `--chmod` or `--chown` values changed

**NOT an invalidation trigger:**
- File modification timestamp (mtime) changed without content change
- File access time changed
- File ownership changed on the host (only the content matters)

**Important details:**
- Docker computes a checksum of every file matching the source pattern.
- For directories, checksums include all files recursively.
- The `.dockerignore` file affects which files are in the build context, which indirectly affects COPY cache.
- `COPY --link` creates an independent layer that can be cached separately from preceding layers.

**Example -- understanding COPY cache:**
```dockerfile
# This caches based on ONLY package.json and lock file content:
COPY package.json package-lock.json ./

# This caches based on ALL files in the build context:
COPY . .
```

The first pattern is dramatically better for caching because it only invalidates when dependencies change.

---

### ADD

**Cache key:** File content checksums + URL response content.

**Invalidation triggers:**
- Local file content changed (same as COPY)
- Remote URL content changed (HTTP response body differs)
- Git repository changed (for Git URL sources)
- `--checksum` value changed

**NOT an invalidation trigger:**
- HTTP headers changing without body change
- Remote server returning different headers with same content

**Important details:**
- For remote URLs, Docker checks the actual response content, not just the URL string.
- For Git sources, Docker checks the commit hash at the specified ref.
- ADD has auto-extraction behavior for tar archives -- the extracted content is what gets cached.

---

### ENV

**Cache key:** The key=value pair.

**Invalidation triggers:**
- The value changed
- The key name changed

**Important details:**
- ENV values persist across layers and into the final image.
- Changing an ENV value invalidates that layer AND all subsequent layers.
- ENV set in a parent image (FROM) is inherited and does NOT trigger invalidation unless overridden.

---

### ARG

**Cache key:** The name=value pair (only when the ARG is USED in subsequent instructions).

**Invalidation triggers:**
- The build-arg value changed AND the ARG is referenced in a subsequent instruction
- An unused ARG does NOT invalidate any cache

**Important details:**
- ARG declared before FROM is only available in FROM itself, not in subsequent instructions.
- ARG must be re-declared after FROM to be used within a stage.
- `--build-arg` values that differ from the default trigger cache invalidation.
- Predefined ARGs (like HTTP_PROXY) do NOT cause cache invalidation unless explicitly referenced.

**Example -- ARG cache behavior:**
```dockerfile
ARG VERSION=1.0
FROM alpine:3.19
ARG VERSION         # Re-declare to use within stage
RUN echo $VERSION   # Cache key includes VERSION value
```

Changing `--build-arg VERSION=2.0` invalidates the RUN layer because it references VERSION.

---

### WORKDIR

**Cache key:** The directory path + `SOURCE_DATE_EPOCH` value.

**Invalidation triggers:**
- The path changed
- `SOURCE_DATE_EPOCH` build-arg changed (affects directory creation timestamp)

**Important details:**
- WORKDIR creates the directory if it does not exist.
- Multiple WORKDIR instructions stack (relative paths accumulate).
- WORKDIR itself rarely causes cache issues -- it is the instructions AFTER it that matter.

---

### EXPOSE, LABEL, USER, VOLUME, STOPSIGNAL, SHELL

**Cache key:** The instruction arguments.

**Invalidation triggers:**
- The arguments changed.

These metadata instructions have simple cache behavior. They rarely cause optimization issues because they are typically static.

---

### HEALTHCHECK

**Cache key:** The full instruction including options and command.

**Invalidation triggers:**
- Any option changed (interval, timeout, retries, start-period)
- The command changed

---

## Cache Cascade Visualization

```
Layer 1: FROM node:20           [CACHE HIT]
Layer 2: WORKDIR /app           [CACHE HIT]
Layer 3: COPY package.json .    [CACHE HIT]  -- file unchanged
Layer 4: RUN npm ci             [CACHE HIT]  -- command unchanged, no prior miss
Layer 5: COPY . .               [CACHE MISS] -- source file changed
Layer 6: RUN npm run build      [MUST REBUILD] -- cascade from Layer 5
Layer 7: CMD ["node", "dist/"]  [MUST REBUILD] -- cascade from Layer 5
```

In this example, only layers 5-7 rebuild. Layers 1-4 are reused from cache, saving the expensive `npm ci` step.

---

## Cache Key Summary Table

| Instruction | What Docker Checks | What Docker Ignores |
|-------------|-------------------|---------------------|
| `FROM` | Image reference + digest | Pull frequency |
| `RUN` | Command string text | External resource state |
| `COPY` | File content checksums | mtime, permissions on host |
| `ADD` | File checksums + URL body | HTTP headers, URL string alone |
| `ENV` | Key=Value text | Runtime overrides |
| `ARG` | Name=Value (if used) | Unused ARGs |
| `WORKDIR` | Path + SOURCE_DATE_EPOCH | Directory contents |

---

## Practical Rules for Cache Optimization

1. **ALWAYS** put instructions that change least at the top of the Dockerfile.
2. **ALWAYS** separate dependency file copying (package.json, go.mod) from source code copying.
3. **ALWAYS** combine `apt-get update` and `apt-get install` in a single RUN to prevent stale index cache.
4. **NEVER** rely on RUN cache for fetching latest versions -- the command string is the only cache key.
5. **ALWAYS** use `.dockerignore` to exclude files that cause unnecessary COPY cache invalidation.
6. **ALWAYS** use `--mount=type=cache` for package manager caches -- they survive layer rebuilds.
7. **NEVER** put `COPY . .` before `RUN install-dependencies` -- any source file change triggers a full dependency reinstall.
