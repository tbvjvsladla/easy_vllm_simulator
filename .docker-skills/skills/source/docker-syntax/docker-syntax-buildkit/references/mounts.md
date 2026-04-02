# BuildKit Mount Types -- Complete Reference

## Cache Mount (`--mount=type=cache`)

Persists a directory across builds. Contents survive layer rebuilds, enabling incremental package downloads.

### Full Syntax

```
RUN --mount=type=cache,target=<path>[,id=<id>][,sharing=<mode>][,from=<stage>][,source=<path>][,mode=<perms>][,uid=<uid>][,gid=<gid>] <command>
```

### All Options

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `target` | YES | -- | Absolute path to the cache directory inside the build container |
| `id` | no | value of `target` | Unique identifier for the cache. Mounts with the same `id` share cache storage across stages and builds |
| `sharing` | no | `shared` | Concurrency mode: `shared` (concurrent read/write), `locked` (exclusive access, one build at a time), `private` (each build gets a fresh copy) |
| `from` | no | (empty) | Build stage or image to initialize cache contents from |
| `source` | no | (empty) | Path within `from` to use as initial cache seed |
| `mode` | no | `0755` | Directory permissions (octal) |
| `uid` | no | `0` | Owner user ID of the cache directory |
| `gid` | no | `0` | Owner group ID of the cache directory |

### Sharing Mode Details

| Mode | Behavior | When to Use |
|------|----------|-------------|
| `shared` | Multiple concurrent builds can read and write simultaneously | MOST package managers (npm, pip, go, cargo) |
| `locked` | Only one build can access the cache at a time; others wait | apt/dpkg (REQUIRED -- apt uses lock files internally) |
| `private` | Each build gets its own copy of the cache; changes are discarded | Test runs that modify cache contents destructively |

### Cache Identity Rules

- Caches with the same `id` are shared across stages and builds on the same builder.
- If `id` is omitted, `target` is used as the identity.
- To namespace caches (e.g., per-project), set explicit `id` values: `id=myproject-npm`.
- The `BUILDKIT_CACHE_MOUNT_NS` ARG can prefix all cache IDs globally.

### Cache Lifecycle

- Cache mounts are NOT cleared between builds by default.
- `docker builder prune` removes all build cache including mount caches.
- `docker builder prune --filter type=exec.cachemount` removes only mount caches.
- Cache content is NOT part of any image layer -- it exists only on the builder.

---

## Secret Mount (`--mount=type=secret`)

Exposes sensitive data during build without persisting in any layer. Secret contents are NEVER written to the image or build cache.

### Full Syntax

```
RUN --mount=type=secret,id=<id>[,target=<path>][,required=<bool>][,env=<name>][,mode=<perms>][,uid=<uid>][,gid=<gid>] <command>
```

### All Options

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `id` | YES | -- | Identifier matching the `--secret id=` flag in the build command |
| `target` | no | `/run/secrets/<id>` | File path where the secret is mounted inside the container |
| `required` | no | `false` | If `true`, the build fails when the secret is not provided |
| `env` | no | (none) | Expose the secret as an environment variable with this name instead of a file |
| `mode` | no | `0400` | File permissions (octal). Default is owner-read-only |
| `uid` | no | `0` | Owner user ID |
| `gid` | no | `0` | Owner group ID |

### Build Command Syntax

```bash
# From file
docker build --secret id=mytoken,src=./token.txt .

# From environment variable
docker build --secret id=mytoken,env=MY_TOKEN .
```

### Security Properties

- Secret contents are NEVER part of any image layer.
- Secret contents are NEVER in the build cache.
- Secret contents do NOT appear in `docker history`.
- Secret files are mounted read-only and exist only during that single RUN instruction.
- Secret changes do NOT trigger cache invalidation -- the RUN layer is cached based on the command string only.

### Cache Invalidation Workaround

```dockerfile
ARG CACHEBUST
RUN --mount=type=secret,id=TOKEN,env=TOKEN some-command
```

```bash
docker build --secret id=TOKEN,src=./token.txt --build-arg CACHEBUST=$(date +%s) .
```

---

## SSH Mount (`--mount=type=ssh`)

Forwards the host SSH agent socket into the build container for Git authentication and other SSH operations.

### Full Syntax

```
RUN --mount=type=ssh[,id=<id>][,target=<path>][,required=<bool>][,mode=<perms>][,uid=<uid>][,gid=<gid>] <command>
```

### All Options

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `id` | no | `default` | SSH agent identity, matching `--ssh <id>=<path>` in the build command |
| `target` | no | `/run/buildkit/ssh_agent.${N}` | Mount path for the SSH agent socket |
| `required` | no | `false` | If `true`, the build fails when the SSH agent is not available |
| `mode` | no | `0600` | Socket file permissions |
| `uid` | no | `0` | Owner user ID |
| `gid` | no | `0` | Owner group ID |

### Build Command Syntax

```bash
# Forward default SSH agent
docker build --ssh default .

# Forward specific key
docker build --ssh default=$HOME/.ssh/id_ed25519 .

# Multiple SSH identities
docker build --ssh default --ssh deploy=$HOME/.ssh/deploy_key .
```

### Security Properties

- The SSH agent socket is forwarded, NOT the private key itself.
- The socket exists only during the RUN instruction execution.
- SSH agent access is NEVER persisted in any layer.

### Host Key Verification

ALWAYS add known hosts before using SSH to avoid interactive prompts that hang the build:

```dockerfile
RUN --mount=type=ssh \
    mkdir -p ~/.ssh \
    && ssh-keyscan github.com >> ~/.ssh/known_hosts \
    && ssh-keyscan gitlab.com >> ~/.ssh/known_hosts \
    && git clone git@github.com:org/repo.git /app
```

---

## Bind Mount (`--mount=type=bind`)

Mounts files from the build context or another stage directly into the build container. Read-only by default. Mounted files are NOT persisted in any layer.

### Full Syntax

```
RUN --mount=type=bind[,target=<path>][,source=<path>][,from=<stage|image>][,rw=<bool>] <command>
```

### All Options

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `target` | YES | -- | Mount destination path inside the build container |
| `source` | no | `.` (root of context or stage) | Source path within the build context or the `from` stage |
| `from` | no | build context | Named build stage or external image to mount from |
| `rw` | no | `false` | If `true`, the mount is read-write. Changes are NOT persisted in any layer or back to the source |

### Key Behaviors

- **Read-only by default.** Write attempts fail unless `rw=true` is set.
- **Changes with `rw=true` are discarded** after the RUN instruction completes. They do NOT modify the source and are NOT part of any layer.
- **Avoids creating COPY layers.** Only the RUN output is kept. This reduces image size when source files are only needed to produce artifacts.
- **Cross-stage mounting** with `from=<stage>` enables reading files from other stages without COPY.

### Common Patterns

```dockerfile
# Mount entire build context (Go compilation without COPY)
RUN --mount=type=bind,target=. go build -o /app/hello

# Mount from another stage
FROM builder AS compile
RUN --mount=type=bind,from=source,source=/src,target=/build/src \
    make -C /build/src

# Mount single file (e.g., requirements without COPY layer)
RUN --mount=type=bind,source=requirements.txt,target=/tmp/requirements.txt \
    pip install -r /tmp/requirements.txt
```

---

## Tmpfs Mount (`--mount=type=tmpfs`)

Creates a temporary in-memory filesystem. Contents are discarded after the RUN instruction completes. NEVER persisted in any layer.

### Full Syntax

```
RUN --mount=type=tmpfs,target=<path>[,size=<bytes>] <command>
```

### All Options

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `target` | YES | -- | Mount path inside the build container |
| `size` | no | unlimited (limited by available memory) | Maximum size in bytes |

### Use Cases

| Scenario | Why Tmpfs |
|----------|----------|
| Compilation scratch space | Avoids writing temp files to a layer |
| Test execution temp data | Automatically cleaned up, no layer bloat |
| Sensitive intermediate files | Guaranteed not persisted anywhere |

### Example

```dockerfile
# Compilation with tmpfs for intermediate objects
RUN --mount=type=tmpfs,target=/tmp \
    gcc -o /app/binary -O2 source.c

# Test execution with tmpfs for test artifacts
RUN --mount=type=tmpfs,target=/test-output \
    pytest --junitxml=/test-output/results.xml tests/
```

---

## Mount Comparison Table

| Property | cache | secret | ssh | bind | tmpfs |
|----------|-------|--------|-----|------|-------|
| Persists between builds | YES | no | no | no | no |
| Part of image layer | no | no | no | no | no |
| Read-write by default | YES | no | n/a | no | YES |
| Supports `from` stage | YES | no | no | YES | no |
| Supports `required` | no | YES | YES | no | no |
| Supports `env` mode | no | YES | no | no | no |
| Triggers cache invalidation | no | no | no | YES (content changes) | no |
