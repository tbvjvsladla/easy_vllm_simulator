# Docker Anti-Patterns -- Consolidated Reference

All anti-patterns from Docker research, organized by domain. Each entry includes detection criteria, severity, and the correct pattern.

---

## Dockerfile Anti-Patterns

### AP-D01: Using `latest` Tag

- **Severity**: Critical
- **Detection**: `FROM <image>` without tag, or `FROM <image>:latest`
- **Risk**: Non-deterministic builds -- different image on each build
- **Fix**: Pin to specific version tag or digest

```dockerfile
# BAD
FROM node:latest
FROM node

# GOOD
FROM node:20.11-bookworm-slim

# BEST (supply chain security)
FROM node:20.11-bookworm-slim@sha256:abc123...
```

### AP-D02: Running as Root

- **Severity**: Critical
- **Detection**: No `USER` instruction in Dockerfile
- **Risk**: Container process has root privileges, escalation attack vector
- **Fix**: Create non-root user and switch to it

```dockerfile
# BAD
FROM node:20
COPY . /app
CMD ["node", "app.js"]

# GOOD
FROM node:20
RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser
WORKDIR /app
COPY --chown=appuser:appuser . .
USER appuser
CMD ["node", "app.js"]
```

### AP-D03: Secrets in ENV or ARG

- **Severity**: Critical
- **Detection**: `ENV` or `ARG` containing passwords, tokens, keys, credentials
- **Risk**: Secrets visible in `docker history` and image layers
- **Fix**: Use `--mount=type=secret` for build-time secrets, runtime env vars for runtime secrets

```dockerfile
# BAD
ENV API_KEY=sk-1234567890
ARG DATABASE_PASSWORD=secret123

# GOOD (build-time)
RUN --mount=type=secret,id=api_key \
    cat /run/secrets/api_key | some-command

# GOOD (runtime)
# Pass via: docker run -e API_KEY="$(cat key.txt)" myapp
```

### AP-D04: Shell Form ENTRYPOINT

- **Severity**: Warning
- **Detection**: `ENTRYPOINT` without JSON array syntax
- **Risk**: Application is NOT PID 1, SIGTERM not forwarded, no graceful shutdown
- **Fix**: Use exec form (JSON array)

```dockerfile
# BAD -- application is NOT PID 1
ENTRYPOINT /usr/bin/myapp

# GOOD -- application IS PID 1
ENTRYPOINT ["/usr/bin/myapp"]
```

### AP-D05: Separate apt-get update and install

- **Severity**: Warning
- **Detection**: `RUN apt-get update` and `RUN apt-get install` as separate instructions
- **Risk**: Cached update layer becomes stale, install uses outdated package index
- **Fix**: ALWAYS combine in single RUN

```dockerfile
# BAD
RUN apt-get update
RUN apt-get install -y curl

# GOOD
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*
```

### AP-D06: Not Cleaning apt Cache

- **Severity**: Warning
- **Detection**: `apt-get install` without `rm -rf /var/lib/apt/lists/*` in same RUN
- **Risk**: 30-100MB wasted per install in image size
- **Fix**: Clean in same RUN layer

```dockerfile
# BAD
RUN apt-get update && apt-get install -y curl

# GOOD
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*
```

### AP-D07: ADD When COPY Suffices

- **Severity**: Warning
- **Detection**: `ADD` used for local file copy (no tar extraction, no URL download)
- **Risk**: Implicit behavior (auto-extraction, URL download) causes surprises
- **Fix**: Use COPY for local files, ADD only for tar extraction or URLs

```dockerfile
# BAD
ADD config.json /app/config.json

# GOOD
COPY config.json /app/config.json
```

### AP-D08: Using cd Instead of WORKDIR

- **Severity**: Warning
- **Detection**: `RUN cd /path && command`
- **Risk**: cd does not persist across layers, fragile
- **Fix**: Use WORKDIR instruction

```dockerfile
# BAD
RUN cd /app && npm install

# GOOD
WORKDIR /app
RUN npm install
```

### AP-D09: ENV Persistence Leak

- **Severity**: Warning
- **Detection**: `ENV VAR=value` followed by `RUN unset VAR`
- **Risk**: ENV persists in image despite unset in later RUN (unset only affects that layer)
- **Fix**: Use shell variable within single RUN

```dockerfile
# BAD -- ADMIN_USER persists in final image
ENV ADMIN_USER="mark"
RUN echo $ADMIN_USER > ./mark
RUN unset ADMIN_USER

# GOOD -- variable is temporary
RUN export ADMIN_USER="mark" \
    && echo $ADMIN_USER > ./mark \
    && unset ADMIN_USER
```

### AP-D10: Missing .dockerignore

- **Severity**: Warning
- **Detection**: No `.dockerignore` file in build context root
- **Risk**: Sends entire directory to builder (node_modules 500MB+, .git history, IDE files)
- **Fix**: Create .dockerignore excluding non-essential files

```
# .dockerignore
.git
node_modules
dist
build
*.md
.env
.env.*
.vscode
.idea
Dockerfile
docker-compose*.yml
```

### AP-D11: Too Many Layers

- **Severity**: Info
- **Detection**: Multiple consecutive RUN instructions for related operations
- **Risk**: Unnecessary layers increase image size and pull time
- **Fix**: Combine related operations

```dockerfile
# BAD
RUN apt-get update
RUN apt-get install -y curl
RUN apt-get install -y git
RUN rm -rf /var/lib/apt/lists/*

# GOOD
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    git \
    && rm -rf /var/lib/apt/lists/*
```

### AP-D12: Missing Syntax Directive

- **Severity**: Info
- **Detection**: No `# syntax=docker/dockerfile:1` at top of Dockerfile
- **Risk**: Cannot use BuildKit features (heredocs, cache mounts, secret mounts)
- **Fix**: Add as first line

```dockerfile
# syntax=docker/dockerfile:1
FROM alpine:3.21
```

### AP-D13: Poor COPY Ordering (Cache Bust)

- **Severity**: Warning
- **Detection**: `COPY . .` before `RUN npm install` or similar dependency install
- **Risk**: Any source code change invalidates dependency cache
- **Fix**: Copy dependency manifest first, install, then copy source

```dockerfile
# BAD -- any change busts npm install cache
COPY . .
RUN npm install

# GOOD -- only package.json change triggers reinstall
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
```

### AP-D14: Missing Pipe Failure Handling

- **Severity**: Warning
- **Detection**: Piped commands in RUN without `set -o pipefail`
- **Risk**: Intermediate pipe failures masked by last command success
- **Fix**: Add `set -o pipefail` before piped commands

```dockerfile
# BAD -- wget failure masked by wc success
RUN wget -O - https://example.com | wc -l > /count

# GOOD
RUN set -o pipefail && wget -O - https://example.com | wc -l > /count
```

### AP-D15: No Multi-Stage Build

- **Severity**: Warning
- **Detection**: Build tools (gcc, make, go, npm) present in single-stage Dockerfile
- **Risk**: Final image contains compilers, source code, build artifacts (100MB-1GB waste)
- **Fix**: Separate build and runtime stages

```dockerfile
# BAD -- Go SDK in production (800MB+)
FROM golang:1.22
COPY . .
RUN go build -o server .
CMD ["./server"]

# GOOD -- only binary in production
FROM golang:1.22 AS build
COPY . .
RUN go build -o /server .

FROM scratch
COPY --from=build /server /server
CMD ["/server"]
```

---

## Compose Anti-Patterns

### AP-C01: Using `version:` Field

- **Severity**: Warning
- **Detection**: `version:` key present in compose.yaml
- **Risk**: Deprecated, ignored by modern Compose, misleading
- **Fix**: Remove entirely

```yaml
# BAD
version: "3.8"
services:
  web:
    image: nginx

# GOOD
services:
  web:
    image: nginx
```

### AP-C02: depends_on Without Healthcheck

- **Severity**: Warning
- **Detection**: `depends_on: [service]` without `condition: service_healthy`
- **Risk**: Dependent service starts before dependency is actually ready
- **Fix**: Add healthcheck to dependency, use condition

```yaml
# BAD
depends_on:
  - db

# GOOD
depends_on:
  db:
    condition: service_healthy
```

### AP-C03: Anonymous Volumes for Data

- **Severity**: Critical
- **Detection**: Volume path without name (e.g., `volumes: [/var/lib/data]`)
- **Risk**: Data lost on `docker compose down`, cannot easily backup or share
- **Fix**: Use named volumes

```yaml
# BAD
volumes:
  - /var/lib/postgresql/data

# GOOD
volumes:
  - db-data:/var/lib/postgresql/data
```

### AP-C04: Hardcoded Secrets

- **Severity**: Critical
- **Detection**: Plain-text passwords/tokens in environment values
- **Risk**: Secrets committed to version control, visible to all
- **Fix**: Use variable interpolation with .env file or Compose secrets

```yaml
# BAD
environment:
  DATABASE_PASSWORD: "my-secret-password"

# GOOD
environment:
  DATABASE_PASSWORD: ${DATABASE_PASSWORD:?Password required}
```

### AP-C05: container_name on Scalable Services

- **Severity**: Warning
- **Detection**: `container_name:` attribute on services that may need scaling
- **Risk**: Container names must be unique -- prevents `docker compose scale`
- **Fix**: Let Compose manage container names

```yaml
# BAD
services:
  web:
    image: nginx
    container_name: my-nginx

# GOOD
services:
  web:
    image: nginx
```

### AP-C06: restart: always Without Resource Limits

- **Severity**: Warning
- **Detection**: `restart: always` without `deploy.resources.limits`
- **Risk**: Crashing container in infinite restart loop consumes all system resources
- **Fix**: Combine restart policy with resource limits

```yaml
# BAD
restart: always

# GOOD
restart: unless-stopped
deploy:
  resources:
    limits:
      cpus: '0.50'
      memory: 512M
```

### AP-C07: Ports Exposed to All Interfaces

- **Severity**: Warning
- **Detection**: Port mapping without host IP (e.g., `"8080:80"`)
- **Risk**: Service accessible from all network interfaces (default 0.0.0.0)
- **Fix**: Bind to localhost for development

```yaml
# BAD -- exposed to all interfaces
ports:
  - "8080:80"

# GOOD -- localhost only
ports:
  - "127.0.0.1:8080:80"
```

### AP-C08: Debug Services Without Profiles

- **Severity**: Info
- **Detection**: Debug/admin tools (phpmyadmin, adminer, mailhog) without profiles
- **Risk**: Unnecessary resource consumption, potential security exposure
- **Fix**: Assign to debug profile

```yaml
# BAD
services:
  phpmyadmin:
    image: phpmyadmin

# GOOD
services:
  phpmyadmin:
    image: phpmyadmin
    profiles: [debug]
```

---

## Security Anti-Patterns

### AP-S01: Using --privileged

- **Severity**: Critical
- **Detection**: `privileged: true` in Compose or `--privileged` in run command
- **Risk**: Container has full host access -- equivalent to running on bare metal as root
- **Fix**: Use specific capabilities

```yaml
# BAD
privileged: true

# GOOD
cap_drop:
  - ALL
cap_add:
  - NET_BIND_SERVICE
```

### AP-S02: No Capability Management

- **Severity**: Warning
- **Detection**: No `cap_drop` or `cap_add` configuration
- **Risk**: Container runs with default capability set (broader than needed)
- **Fix**: Drop all, add back only what is needed

```yaml
# BAD -- default capabilities
services:
  app:
    image: myapp

# GOOD -- minimal capabilities
services:
  app:
    image: myapp
    cap_drop:
      - ALL
    cap_add:
      - NET_BIND_SERVICE
    security_opt:
      - no-new-privileges:true
```

### AP-S03: Host Network Without Justification

- **Severity**: Warning
- **Detection**: `network_mode: host` or `--network host`
- **Risk**: No network isolation -- container shares host's network namespace
- **Fix**: Use bridge network with port mapping unless performance requires host mode

### AP-S04: Mounting Host Root

- **Severity**: Critical
- **Detection**: Volume mount of `/` or `/etc` or `/var/run/docker.sock`
- **Risk**: Container can read/write host filesystem, docker socket gives root access
- **Fix**: Mount only specific needed directories with minimal permissions

```yaml
# BAD
volumes:
  - /:/host
  - /var/run/docker.sock:/var/run/docker.sock

# GOOD -- mount only what is needed
volumes:
  - ./config:/app/config:ro
```

### AP-S05: No Image Scanning

- **Severity**: Warning
- **Detection**: No `docker scout cves` in CI/CD pipeline
- **Risk**: Vulnerable base images or dependencies deployed to production
- **Fix**: Add scanning to CI/CD

```bash
# Add to CI pipeline
docker scout cves --only-severity critical,high --exit-code myapp:latest
```

---

## Build Performance Anti-Patterns

### AP-B01: Large Build Context

- **Severity**: Warning
- **Detection**: No .dockerignore, slow `docker build` startup
- **Risk**: Sends GB of files to builder (node_modules, .git, test data)
- **Fix**: Create comprehensive .dockerignore

### AP-B02: No Cache Mounts

- **Severity**: Info
- **Detection**: `RUN pip install` or `RUN npm install` without `--mount=type=cache`
- **Risk**: Full package download on every build
- **Fix**: Use BuildKit cache mounts for package managers

```dockerfile
# BAD -- downloads all packages every build
RUN pip install -r requirements.txt

# GOOD -- reuses cached packages
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install -r requirements.txt
```

### AP-B03: No Build Cache Strategy in CI

- **Severity**: Info
- **Detection**: No `--cache-from` / `--cache-to` in CI build commands
- **Risk**: Full rebuild on every CI run (slow, wasteful)
- **Fix**: Configure registry or GHA cache backend

```bash
# GOOD -- registry cache
docker buildx build \
  --cache-from type=registry,ref=registry/app:buildcache \
  --cache-to type=registry,ref=registry/app:buildcache,mode=max \
  -t registry/app:latest .
```

---

## Quick Detection Checklist

Use this for rapid scanning of a Docker project:

```
Dockerfile:
[ ] grep -c "FROM.*latest\|FROM [a-z]*/[a-z]*$" Dockerfile     # AP-D01
[ ] grep -c "^USER " Dockerfile                                   # AP-D02 (should be >0)
[ ] grep -c "ENV.*KEY\|ENV.*SECRET\|ENV.*PASSWORD" Dockerfile     # AP-D03
[ ] grep -c "^ENTRYPOINT [^[]" Dockerfile                         # AP-D04
[ ] grep -c "^ADD " Dockerfile                                    # AP-D07 (review each)
[ ] test -f .dockerignore                                          # AP-D10

Compose:
[ ] grep -c "^version:" compose.yaml                              # AP-C01
[ ] grep -c "container_name:" compose.yaml                        # AP-C05
[ ] grep -c "privileged: true" compose.yaml                       # AP-S01
[ ] grep -c "restart: always" compose.yaml                        # AP-C06 (check limits)
```
