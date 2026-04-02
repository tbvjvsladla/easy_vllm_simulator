# docker-errors-build: Complete Diagnostics Reference

## Error Message Index

This reference provides an exhaustive mapping from exact Docker build error messages to their root causes and solutions. Organized by error category.

---

## 1. COPY/ADD File Resolution Errors

### Error: `COPY failed: file not found in build context`

**Exact output:**
```
------
 > [stage 3/5] COPY config.json /app/:
------
ERROR: failed to solve: failed to compute cache key: failed to calculate checksum of ref
moby::randomhash: "/config.json": not found
```

**Possible causes (check in order):**

1. **File is outside the build context directory**
   - The build context is the directory passed to `docker build`. Files above or outside it are NEVER accessible.
   - Fix: Move the file into the context, or change the context path:
     ```bash
     # If Dockerfile is in ./docker/ but files are in ./
     docker build -f docker/Dockerfile .
     ```

2. **File is excluded by `.dockerignore`**
   - Check `.dockerignore` for patterns matching the file.
   - Fix: Remove or adjust the pattern. Add an exception:
     ```
     *.json
     !config.json
     ```

3. **Path is wrong relative to build context**
   - COPY paths are relative to the build context root, NOT the Dockerfile location.
   - Fix: Use `ls` from the context directory to verify the file exists at the expected relative path.

4. **Case sensitivity mismatch (Linux builds)**
   - Linux filesystems are case-sensitive. `Config.json` and `config.json` are different files.
   - Fix: Match the exact case in the COPY instruction.

### Error: `failed to compute cache key: failed to calculate checksum of ref`

**Exact output:**
```
ERROR: failed to solve: failed to compute cache key: failed to calculate checksum of ref
"randomhash::src/app": "/src/app": not found
```

**Cause:** The source path in COPY or ADD does not exist in the build context.

**Diagnostic steps:**
```bash
# 1. Check what files are in the build context
ls -la <context-directory>/src/app

# 2. Check .dockerignore
cat .dockerignore | grep -i "src"

# 3. Rebuild with verbose output
docker build --progress=plain . 2>&1 | head -20
```

### Error: `COPY requires at least two arguments`

**Cause:** Missing destination path or malformed COPY instruction.

**Common triggers:**
```dockerfile
# BAD: Missing destination
COPY package.json

# GOOD: Include destination
COPY package.json .
COPY package.json /app/
```

### Error: `When using COPY with more than one source file, the destination must be a directory and end with /`

**Cause:** Multiple source files but destination lacks trailing slash.

**Fix:**
```dockerfile
# BAD
COPY file1.txt file2.txt /app

# GOOD
COPY file1.txt file2.txt /app/
```

---

## 2. Build Context Errors

### Symptom: `sending build context to Docker daemon` takes >30 seconds

**Diagnostic:**
```bash
# Check context size
du -sh --exclude=.git .

# Check what .dockerignore excludes
# (no built-in Docker command -- compare manually)
tar -czf /dev/null -C <context> . 2>&1
```

**Root causes and fixes:**

| Size indicator | Likely culprit | Fix |
|---------------|---------------|-----|
| >500 MB | `node_modules/` included | Add `node_modules` to `.dockerignore` |
| >100 MB | `.git/` included | Add `.git` to `.dockerignore` |
| >50 MB | Build artifacts (dist/, build/) | Add build output dirs to `.dockerignore` |
| Variable | Large data files, logs, media | Add `*.log`, `*.mp4`, data dirs to `.dockerignore` |

**Minimal `.dockerignore` template:**
```
.git
node_modules
dist
build
*.log
.env
.env.*
```

---

## 3. Cache Invalidation Errors

### Symptom: Dependencies reinstall on every build

**Diagnostic:**
```bash
# Check layer cache hits
docker build --progress=plain . 2>&1 | grep -E "CACHED|RUN"
```

**Root cause:** Source code COPY before dependency installation.

**Fix pattern (Node.js):**
```dockerfile
# WRONG ORDER -- any file change invalidates npm install
COPY . .
RUN npm ci

# CORRECT ORDER -- only package.json changes trigger npm install
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
```

**Fix pattern (Python):**
```dockerfile
# WRONG ORDER
COPY . .
RUN pip install -r requirements.txt

# CORRECT ORDER
COPY requirements.txt .
RUN pip install -r requirements.txt
COPY . .
```

**Fix pattern (Go):**
```dockerfile
# WRONG ORDER
COPY . .
RUN go build -o /app

# CORRECT ORDER
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN go build -o /app
```

### Symptom: `apt-get install` installs stale or missing packages

**Root cause:** `apt-get update` and `apt-get install` in separate RUN layers.

```dockerfile
# BAD: apt-get update layer is cached, install uses stale package list
RUN apt-get update
RUN apt-get install -y curl nginx

# GOOD: Combined in single layer
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    nginx \
    && rm -rf /var/lib/apt/lists/*
```

### Symptom: RUN layer rebuilds despite identical command

**Cause:** A preceding layer was invalidated, causing ALL subsequent layers to rebuild.

**Diagnostic:** Run with `--progress=plain` and look for the FIRST non-CACHED layer. That is where invalidation started.

---

## 4. BuildKit-Specific Errors

### Error: `failed to create LLB definition: rpc error: unknown flag: --mount`

**Causes (check in order):**

1. Missing syntax directive:
   ```dockerfile
   # MUST be the very first line
   # syntax=docker/dockerfile:1
   ```

2. BuildKit not enabled (Docker Engine < 23):
   ```bash
   DOCKER_BUILDKIT=1 docker build .
   ```

3. Using legacy builder explicitly:
   ```bash
   # If DOCKER_BUILDKIT=0 is set, unset it
   unset DOCKER_BUILDKIT
   ```

### Error: `error: secret "X" not found`

**Cause:** Build command does not pass the required secret.

**Fix:**
```bash
# Must pass --secret flag
docker build --secret id=X,src=./secret-file.txt .

# For environment variable secrets
SECRET_VALUE=mytoken docker build --secret id=SECRET_VALUE .
```

### Error: `could not parse ssh: [default]: stat /path: no such file or directory`

**Cause:** SSH agent not running or key not loaded.

**Fix:**
```bash
eval $(ssh-agent)
ssh-add ~/.ssh/id_rsa
docker build --ssh default .
```

### Error: Cache mount produces inconsistent results with apt

**Cause:** Multiple concurrent builds writing to the same apt cache.

**Fix:** Use `sharing=locked` for apt cache mounts:
```dockerfile
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends curl
```

---

## 5. Multi-Stage Build Errors

### Error: `invalid from flag value "Build": invalid reference format`

**Cause:** Stage name contains uppercase letters.

**Fix:** Stage names MUST be lowercase:
```dockerfile
# BAD
FROM golang:1.22 AS Build

# GOOD
FROM golang:1.22 AS build
```

### Error: `failed to solve: <stage>: not found`

**Possible causes:**

1. Stage name misspelled in `COPY --from=`:
   ```dockerfile
   FROM golang:1.22 AS builder
   # ...
   COPY --from=buider /app /app  # TYPO: "buider" instead of "builder"
   ```

2. Stage defined AFTER the COPY that references it:
   - Stages MUST be defined before they are referenced (order matters).

3. `--target` flag skips the stage:
   - When using `--target`, only the target stage and its dependencies are built.

### Error: `circular dependency detected`

**Cause:** Stage A copies from stage B, and stage B copies from stage A.

**Fix:** Restructure to break the cycle. Introduce an intermediate stage:
```dockerfile
FROM alpine AS shared-base
COPY shared-files /shared/

FROM shared-base AS stage-a
# ...

FROM shared-base AS stage-b
COPY --from=stage-a /output /input
```

---

## 6. Base Image Pull Errors

### Error: `pull access denied for X, repository does not exist`

**Diagnostic:**
```bash
# Verify the image exists
docker manifest inspect <image>:<tag>

# Check for typos in common images
# nginx vs ngnix, postgres vs postgresql, python vs pythoon
```

**Causes:**
1. Image name misspelled
2. Private image without authentication
3. Image genuinely does not exist

### Error: `manifest unknown: manifest unknown`

**Cause:** The specified tag does not exist for this image.

**Fix:**
```bash
# Check available tags
docker manifest inspect <image>:<tag>

# Common mistake: using OS-specific tags that don't exist
# e.g., node:20-alpine3.19 when only node:20-alpine exists
```

### Error: `toomanyrequests: You have reached your pull rate limit`

**Docker Hub rate limits:**
- Anonymous: 100 pulls per 6 hours
- Authenticated (free): 200 pulls per 6 hours
- Paid: Higher limits

**Fix:**
```bash
# Authenticate to increase limits
docker login

# Use a registry mirror
# In /etc/docker/daemon.json:
# { "registry-mirrors": ["https://mirror.example.com"] }
```

---

## 7. Platform Mismatch Errors

### Error: `exec format error`

**Full output:**
```
standard_init_linux.go:228: exec user process caused: exec format error
```

**Cause:** Binary in the image was compiled for a different CPU architecture.

**Diagnostic:**
```bash
# Check image platform
docker inspect --format='{{.Os}}/{{.Architecture}}' <image>

# Check host platform
uname -m
```

**Fix:**
```bash
# Build for specific platform
docker build --platform linux/amd64 .

# Run with platform emulation
docker run --platform linux/amd64 <image>
```

### Error: `image with reference X does not match the specified platform`

**Cause:** Pulled image manifest does not include the requested platform.

**Fix:**
```bash
# Check available platforms
docker manifest inspect <image>:<tag> | grep -A2 '"platform"'

# Use a different base image version that supports your platform
```

---

## 8. ARG/ENV Scope Errors

### Symptom: ARG value is empty after FROM

**Root cause:** ARG scope resets at each FROM instruction.

```dockerfile
# BAD: VERSION is empty in the build stage
ARG VERSION=1.0
FROM alpine:3.21
RUN echo $VERSION > /version  # Empty!

# GOOD: Re-declare ARG after FROM
ARG VERSION=1.0
FROM alpine:3.21
ARG VERSION
RUN echo $VERSION > /version  # "1.0"
```

### Symptom: Variable not expanded in exec form

**Root cause:** Exec form does NOT invoke a shell, so no variable expansion occurs.

```dockerfile
# BAD: Literal "$HOME" string, not expanded
RUN ["echo", "$HOME"]

# GOOD: Shell form expands variables
RUN echo $HOME

# GOOD: Explicit shell in exec form
RUN ["/bin/sh", "-c", "echo $HOME"]
```

### Symptom: ARG not available at container runtime

**Root cause:** ARG is build-time only. It does NOT persist in the image.

```dockerfile
# BAD: APP_VERSION not available at runtime
ARG APP_VERSION=1.0
CMD echo $APP_VERSION  # Empty at runtime

# GOOD: Convert ARG to ENV for runtime persistence
ARG APP_VERSION=1.0
ENV APP_VERSION=$APP_VERSION
CMD echo $APP_VERSION  # "1.0" at runtime
```

---

## 9. Permission Errors During Build

### Error: `EACCES: permission denied, mkdir '/app/node_modules'`

**Cause:** USER instruction set before directory creation or package install.

**Fix:**
```dockerfile
# BAD: Non-root user can't write to /app
USER node
WORKDIR /app
RUN npm install  # Permission denied

# GOOD: Install as root, then switch user
WORKDIR /app
COPY --chown=node:node package*.json ./
RUN npm install
USER node
```

### Error: `permission denied` executing a COPY'd script

**Cause:** COPY preserves source file permissions. If source lacks +x, so does the copy.

**Fix:**
```dockerfile
# Option 1: Set permissions in COPY (BuildKit)
COPY --chmod=755 entrypoint.sh /app/

# Option 2: chmod after COPY
COPY entrypoint.sh /app/
RUN chmod +x /app/entrypoint.sh
```

### Error: `open /var/lib/apt/lists/lock: permission denied`

**Cause:** Running apt as non-root user.

**Fix:** Run ALL system package installation BEFORE the USER instruction:
```dockerfile
FROM ubuntu:22.04
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*
RUN useradd -r -s /bin/false appuser
USER appuser
```

---

## 10. Dockerfile Syntax Errors

### Error: `unknown instruction: XXX`

**Cause:** Instruction misspelled or in wrong case.

**Common mistakes:**
```dockerfile
run echo hello        # BAD: lowercase
FORM ubuntu:22.04     # BAD: typo (FORM instead of FROM)
COPY. /app/           # BAD: missing space after COPY
```

**Fix:** ALL Dockerfile instructions MUST be uppercase. Check spelling.

### Error: `Dockerfile parse error: missing FROM`

**Causes:**
1. No FROM instruction in the Dockerfile
2. Parser directive or comment is malformed, consuming the FROM line
3. BOM character at file start

**Fix:** Ensure FROM is present and is the first instruction (after optional parser directives and global ARGs).

### Error: Unexpected behavior with line continuations

**Cause:** Trailing whitespace after the `\` continuation character.

```dockerfile
# BAD: Invisible space after backslash breaks continuation
RUN apt-get update && \
    apt-get install -y curl

# GOOD: No trailing whitespace after backslash
RUN apt-get update && \
    apt-get install -y curl
```

**Detection:** Use an editor that shows trailing whitespace, or run:
```bash
grep -nP '\\\s+$' Dockerfile
```

### Error: `failed to process "Dockerfile": file is empty`

**Causes:**
1. Dockerfile is actually empty
2. BOM (Byte Order Mark) character at file start
3. Wrong file encoding

**Fix:**
```bash
# Check for BOM
file Dockerfile
# Should show "ASCII text" or "UTF-8 Unicode text"
# If it shows "UTF-8 Unicode (with BOM) text", remove BOM:
sed -i '1s/^\xEF\xBB\xBF//' Dockerfile
```
