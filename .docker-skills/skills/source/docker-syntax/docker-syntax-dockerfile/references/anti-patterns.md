# Dockerfile Anti-Patterns

> Common instruction misuse patterns with corrections.
> Every anti-pattern includes WHY it is wrong and the correct alternative.

---

## AP-001: Using `latest` Tag

**Problem:** Non-deterministic builds -- different image on each build.

```dockerfile
# BAD
FROM node:latest
FROM python
```

```dockerfile
# GOOD -- pin to specific version
FROM node:20.11-bookworm-slim

# BEST -- pin to digest for full reproducibility
FROM node:20.11-bookworm-slim@sha256:abc123...
```

**Why:** `latest` resolves to a different image after every upstream push. Builds become unreproducible and may break without any Dockerfile change.

---

## AP-002: Secrets in ENV or ARG

**Problem:** Secrets are baked into image layers and visible in `docker history`.

```dockerfile
# BAD -- secret persists in image metadata
ENV API_KEY=sk-1234567890
ARG DATABASE_PASSWORD=secret123
RUN curl -H "Authorization: Bearer $API_KEY" https://api.example.com
```

```dockerfile
# GOOD -- secret exists only during RUN, never in any layer
RUN --mount=type=secret,id=api_key,env=API_KEY \
    curl -H "Authorization: Bearer $API_KEY" https://api.example.com
```

**Build:** `docker build --secret id=api_key,src=./api_key.txt .`

**Why:** ENV values persist in the final image (`docker inspect`). ARG values appear in `docker history`. Both are extractable by anyone with access to the image.

---

## AP-003: ADD When COPY Suffices

**Problem:** ADD has implicit behaviors that make builds less predictable.

```dockerfile
# BAD -- ADD auto-extracts tars, downloads URLs, adds magic
ADD config.json /app/config.json
ADD src/ /app/src/
```

```dockerfile
# GOOD -- COPY is explicit with no side effects
COPY config.json /app/config.json
COPY src/ /app/src/
```

**Why:** ADD auto-extracts tar archives and downloads URLs. When you only need to copy local files, these implicit behaviors create confusion and risk unexpected extraction.

**When ADD is correct:**
- Downloading a remote file with `ADD --checksum=sha256:...`
- Cloning a Git repository: `ADD https://github.com/user/repo.git#v1.0 /src`
- Intentionally extracting a tar archive: `ADD archive.tar.gz /dest/`

---

## AP-004: Shell Form ENTRYPOINT

**Problem:** Application is NOT PID 1 and does not receive signals.

```dockerfile
# BAD -- runs as /bin/sh -c "/usr/bin/myapp", app is NOT PID 1
ENTRYPOINT /usr/bin/myapp
```

```dockerfile
# GOOD -- app IS PID 1, receives SIGTERM for graceful shutdown
ENTRYPOINT ["/usr/bin/myapp"]
```

**Why:** Shell form wraps the command in `/bin/sh -c`, making `sh` PID 1 instead of the application. The `sh` process does NOT forward signals. `docker stop` sends SIGTERM, but the application never receives it, leading to a 10-second timeout and forced SIGKILL.

---

## AP-005: Separate apt-get update and install

**Problem:** Cached update layer becomes stale, causing install failures.

```dockerfile
# BAD -- apt-get update is cached, install uses stale package list
RUN apt-get update
RUN apt-get install -y curl
```

```dockerfile
# GOOD -- always combine in one layer
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*
```

**Why:** Docker caches each RUN layer independently. When you add a new package later, the `apt-get update` layer is still cached from weeks/months ago. The install fails because package URLs have changed.

---

## AP-006: Not Cleaning Package Manager Cache

**Problem:** Cache files bloat the image by 30-100MB per install.

```dockerfile
# BAD -- cache left in layer
RUN apt-get update && apt-get install -y curl git
```

```dockerfile
# GOOD -- clean in same layer
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    git \
    && rm -rf /var/lib/apt/lists/*
```

**Why:** Cleanup in a SEPARATE RUN instruction does NOT reduce image size. Docker layers are additive -- deleting files in a new layer adds a "whiteout" entry but the original data remains in the previous layer.

---

## AP-007: Running as Root

**Problem:** Container processes run as root, creating a security risk.

```dockerfile
# BAD -- no USER instruction, runs as root
FROM node:20
COPY . /app
CMD ["node", "app.js"]
```

```dockerfile
# GOOD -- create and switch to non-root user
FROM node:20
RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser
WORKDIR /app
COPY --chown=appuser:appuser . .
USER appuser
CMD ["node", "app.js"]
```

**Why:** If an attacker exploits the application, root access inside the container can lead to container escape, host filesystem access, or privilege escalation.

---

## AP-008: COPY . . Before Dependency Install

**Problem:** Every source code change invalidates the dependency cache.

```dockerfile
# BAD -- any file change triggers full npm install
FROM node:20
WORKDIR /app
COPY . .
RUN npm install
RUN npm run build
```

```dockerfile
# GOOD -- dependency files copied first, code changes only affect build
FROM node:20
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build
```

**Why:** Docker invalidates a layer's cache when ANY input file changes. Copying everything before `npm install` means even a one-line code change triggers a full dependency reinstall.

---

## AP-009: Using cd Instead of WORKDIR

**Problem:** `cd` in RUN does not persist to the next instruction.

```dockerfile
# BAD -- cd only lasts within the same RUN
RUN cd /app && npm install
RUN npm run build  # FAILS: still in / not /app
```

```dockerfile
# GOOD -- WORKDIR persists across instructions
WORKDIR /app
RUN npm install
RUN npm run build
```

**Why:** Each RUN starts from the WORKDIR, not from where the previous RUN ended. `cd` within a RUN only affects that single instruction.

---

## AP-010: Too Many Layers

**Problem:** Each RUN creates a separate layer, bloating the image.

```dockerfile
# BAD -- 4 layers for one logical operation
RUN apt-get update
RUN apt-get install -y curl
RUN apt-get install -y git
RUN rm -rf /var/lib/apt/lists/*
```

```dockerfile
# GOOD -- one layer for the entire operation
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    git \
    && rm -rf /var/lib/apt/lists/*
```

**Why:** More layers mean larger images and slower pulls. Cleanup in a later layer does NOT reclaim space from earlier layers.

---

## AP-011: ENV Persistence Leak

**Problem:** Environment variables set with ENV persist in the final image even after `unset`.

```dockerfile
# BAD -- ADMIN_USER persists in the image despite unset
ENV ADMIN_USER="mark"
RUN echo $ADMIN_USER > ./mark
RUN unset ADMIN_USER  # Does NOT remove from image metadata!
```

```dockerfile
# GOOD -- use shell variable within single RUN
RUN export ADMIN_USER="mark" \
    && echo $ADMIN_USER > ./mark \
    && unset ADMIN_USER
```

**Why:** ENV writes to image metadata. `unset` in a RUN only affects that shell session. The variable remains in `docker inspect` output and is available at container runtime.

---

## AP-012: No .dockerignore

**Problem:** Entire project directory (including node_modules, .git) is sent as build context.

```
# BAD -- no .dockerignore, sends everything
project/
├── .git/              (500MB+ of history)
├── node_modules/      (500MB+ of dependencies)
├── dist/              (rebuilt in container)
└── src/               (what you actually need)
```

```
# GOOD -- .dockerignore excludes irrelevant files
.git
node_modules
dist
*.md
.env
.env.*
```

**Why:** Build context is sent to the Docker daemon before build starts. Without `.dockerignore`, gigabytes of unnecessary data are transferred, slowing every build.

---

## AP-013: No HEALTHCHECK

**Problem:** Docker has no way to detect if the application inside the container is actually working.

```dockerfile
# BAD -- no health monitoring
FROM node:20
COPY . /app
CMD ["node", "app.js"]
```

```dockerfile
# GOOD -- Docker can detect application failures
FROM node:20
COPY . /app
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD node -e "require('http').get('http://localhost:3000/health', (r) => { process.exit(r.statusCode === 200 ? 0 : 1) })"
CMD ["node", "app.js"]
```

**Why:** Without HEALTHCHECK, Docker only knows if the process is running, not if it is responding correctly. Orchestrators like Docker Compose and Swarm use health status to manage container lifecycle and restarts.

---

## AP-014: Pipe Errors Silently Swallowed

**Problem:** In shell form, pipe failures are masked by the last command's exit code.

```dockerfile
# BAD -- if wget fails, wc still succeeds, RUN reports success
RUN wget -O - https://some.site | wc -l > /number
```

```dockerfile
# GOOD -- set pipefail so any command failure is caught
RUN set -o pipefail && wget -O - https://some.site | wc -l > /number
```

**Why:** By default, `/bin/sh -c` only checks the exit code of the last command in a pipe. A failed download could produce an empty file with no error. `set -o pipefail` makes the pipe return the exit code of the first failing command.

**Note:** `pipefail` is a bash feature. If the default shell is `dash` (common on Debian), use exec form: `RUN ["/bin/bash", "-c", "set -o pipefail && ..."]`.

---

## AP-015: Shell Form CMD with ENTRYPOINT

**Problem:** Shell form CMD produces unexpected process tree when combined with exec form ENTRYPOINT.

```dockerfile
# BAD -- results in: entrypoint_cmd /bin/sh -c cmd_string
ENTRYPOINT ["python"]
CMD python app.py
```

```dockerfile
# GOOD -- results in: python app.py
ENTRYPOINT ["python"]
CMD ["app.py"]
```

**Why:** Shell form CMD wraps in `/bin/sh -c`, which becomes an argument to the ENTRYPOINT. The actual command becomes `python /bin/sh -c python app.py`, which is not the intended behavior. ALWAYS use exec form for both ENTRYPOINT and CMD.

---

## AP-016: Numeric Stage References

**Problem:** Using `--from=0` instead of named stages breaks when stages are reordered.

```dockerfile
# BAD -- fragile, breaks if a stage is added before this one
FROM golang:1.22
RUN go build -o /app
FROM alpine:3.19
COPY --from=0 /app /usr/bin/app
```

```dockerfile
# GOOD -- named reference survives reordering
FROM golang:1.22 AS build
RUN go build -o /app
FROM alpine:3.19
COPY --from=build /app /usr/bin/app
```

**Why:** Numeric indexes are positional. Adding, removing, or reordering stages silently changes what `--from=0` refers to. Named stages are explicit and self-documenting.

---

## AP-017: VOLUME Before File Operations

**Problem:** Data written to a VOLUME path during build is silently discarded.

```dockerfile
# BAD -- the echo output is lost because /data is a volume
VOLUME /data
RUN echo "config" > /data/config.txt  # DISCARDED at runtime!
```

```dockerfile
# GOOD -- write files first, declare volume last
RUN mkdir -p /data && echo "config" > /data/config.txt
VOLUME /data
```

**Why:** After a VOLUME instruction, any changes to that directory in subsequent build layers are discarded. The volume mount at runtime replaces the directory contents.

---

## AP-018: No --no-install-recommends for apt

**Problem:** apt installs recommended packages by default, adding unnecessary bloat.

```dockerfile
# BAD -- installs curl plus all "recommended" packages
RUN apt-get update && apt-get install -y curl
```

```dockerfile
# GOOD -- only installs curl and its hard dependencies
RUN apt-get update && apt-get install -y --no-install-recommends curl
```

**Why:** Recommended packages can add 50-200MB of unnecessary software. In container images, you ALWAYS want the minimal set of packages.
