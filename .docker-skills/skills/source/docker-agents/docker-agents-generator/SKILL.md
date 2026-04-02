---
name: docker-agents-generator
description: >
  Use when containerizing an application from scratch or generating
  Dockerfile and Compose configurations for a new project.
  Prevents insecure defaults by enforcing non-root users, multi-stage
  builds, health checks, and proper .dockerignore from the start.
  Covers Dockerfile generation for Node.js, Python, Go, Java, Rust,
  .NET, dev/prod Compose stacks, .dockerignore, .env templates.
  Keywords: Dockerfile, docker-compose.yml, multi-stage build,
  .dockerignore, HEALTHCHECK, non-root, FROM, COPY, ENTRYPOINT,
  containerize my app, dockerize, create Dockerfile for my project,
  how to Docker, new container setup.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+ and Docker Compose v2."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-agents-generator

## Generation Workflow

Execute these steps in order when containerizing an application:

```
1. Gather requirements (language, framework, database, cache)
2. Select Dockerfile template (language-specific)
3. Generate Dockerfile with multi-stage build
4. Generate .dockerignore for the language
5. Generate Compose configuration (dev and/or prod)
6. Generate .env template
7. Verify completeness
```

---

## Step 1: Requirements Gathering

**ALWAYS** determine these before generating any files:

```
[ ] Language/runtime: Node.js | Python | Go | Java | Rust | .NET
[ ] Framework: Express, FastAPI, Gin, Spring Boot, Actix, ASP.NET, etc.
[ ] Target environment: development | production | both
[ ] Database: PostgreSQL | MySQL | MongoDB | Redis | none
[ ] Cache layer: Redis | Memcached | none
[ ] Message queue: RabbitMQ | Kafka | none
[ ] Reverse proxy: Nginx | Traefik | none
[ ] Needs file watch (dev): yes | no
[ ] Ports: application port(s)
[ ] Persistent data: volume requirements
```

---

## Step 2: Dockerfile Template Decision Tree

```
Language?
├─ Node.js
│  ├─ Static frontend only? → multi-stage with nginx (see templates)
│  └─ Server app? → node:22-bookworm-slim runtime
├─ Python
│  ├─ ML/Data Science? → python:3.12-bookworm (full image)
│  └─ Web app? → python:3.12-slim-bookworm runtime
├─ Go
│  └─ ALWAYS → scratch or alpine runtime (static binary)
├─ Java
│  ├─ Spring Boot? → eclipse-temurin:21-jre-jammy runtime
│  └─ Other? → eclipse-temurin:21-jre-jammy runtime
├─ Rust
│  └─ ALWAYS → scratch or alpine runtime (static binary)
└─ .NET
   └─ ALWAYS → mcr.microsoft.com/dotnet/aspnet runtime
```

### Base Image Selection Rules

- **ALWAYS** use specific version tags, NEVER `latest`
- **ALWAYS** use `-slim` or `-bookworm-slim` variants for runtime stages
- **ALWAYS** use the full SDK image for build stages only
- **NEVER** ship compilers, SDKs, or build tools in production images
- **ALWAYS** include `# syntax=docker/dockerfile:1` as the first line

---

## Step 3: Generate Dockerfile

**ALWAYS** follow this structure for every generated Dockerfile:

```dockerfile
# syntax=docker/dockerfile:1

# ---- Build Stage ----
FROM <sdk-image> AS build
WORKDIR /src
# 1. Copy dependency manifests first (cache optimization)
# 2. Install dependencies with cache mounts
# 3. Copy source code
# 4. Build application

# ---- Runtime Stage ----
FROM <minimal-image> AS runtime
# 1. Create non-root user
# 2. Copy built artifacts from build stage
# 3. Set ownership and permissions
# 4. Configure health check
# 5. Switch to non-root user
# 6. Expose port
# 7. Set ENTRYPOINT and CMD
```

### Security Defaults (ALWAYS Apply)

```dockerfile
# Non-root user — ALWAYS include
RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser

# Health check — ALWAYS include
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD <health-check-command> || exit 1

# Non-root execution — ALWAYS the last USER instruction
USER appuser

# Exec form — ALWAYS use for ENTRYPOINT
ENTRYPOINT ["/app/binary"]
```

### Language-Specific Templates

See [references/templates.md](references/templates.md) for complete, buildable templates for:
- Node.js (server app + static frontend)
- Python (pip + Poetry)
- Go (static binary)
- Java (Maven + Gradle)
- Rust (cargo)
- .NET (dotnet)

---

## Step 4: Generate .dockerignore

**ALWAYS** generate a `.dockerignore` file. Select patterns based on language:

### Universal Patterns (ALWAYS include)

```
.git
.gitignore
.dockerignore
Dockerfile
docker-compose*.yml
compose*.yaml
*.md
LICENSE
.env
.env.*
*.pem
*.key
.vscode
.idea
*.swp
.DS_Store
Thumbs.db
```

### Language-Specific Additions

| Language | Additional Patterns |
|----------|-------------------|
| Node.js | `node_modules/`, `dist/`, `build/`, `.next/`, `coverage/`, `.npm/` |
| Python | `__pycache__/`, `*.pyc`, `.venv/`, `venv/`, `.pytest_cache/`, `*.egg-info/` |
| Go | `vendor/` (if not vendoring), `*.test`, `*.out` |
| Java | `target/`, `build/`, `.gradle/`, `*.class`, `*.jar` (built in container) |
| Rust | `target/`, `*.rs.bk` |
| .NET | `bin/`, `obj/`, `*.user`, `*.suo`, `packages/` |

---

## Step 5: Generate Compose Configuration

### Compose Template Decision Tree

```
What stack?
├─ Web + Database
│  └─ compose.yaml with app + db + named volume
├─ Web + Database + Cache
│  └─ compose.yaml with app + db + redis + named volumes
├─ Full Stack (frontend + backend + db + cache)
│  └─ compose.yaml with frontend + backend + db + redis + networks
├─ Development only
│  └─ compose.yaml with watch, bind mounts, debug ports
└─ Production only
   └─ compose.yaml with resource limits, restart policy, no bind mounts
```

### Compose Generation Rules

- **NEVER** include the `version:` field (deprecated)
- **ALWAYS** use `depends_on` with `condition: service_healthy`
- **ALWAYS** define health checks for database and cache services
- **ALWAYS** use named volumes for persistent data
- **ALWAYS** use `env_file` instead of hardcoded environment values
- **ALWAYS** bind ports to `127.0.0.1` in development configurations
- **ALWAYS** include resource limits in production configurations
- **ALWAYS** use `restart: unless-stopped` in production
- **NEVER** use `container_name` for services that may need scaling

See [references/compose-templates.md](references/compose-templates.md) for complete templates.

---

## Step 6: Generate .env Template

**ALWAYS** generate a `.env.example` file alongside Compose configurations:

```env
# Application
APP_PORT=3000
NODE_ENV=production

# Database
POSTGRES_USER=app
POSTGRES_PASSWORD=changeme
POSTGRES_DB=appdb

# Redis (if applicable)
REDIS_PASSWORD=changeme

# Secrets — NEVER commit actual values
# Copy this file to .env and fill in real values
```

### Rules

- **ALWAYS** include placeholder values, NEVER real secrets
- **ALWAYS** add a comment warning not to commit `.env`
- **ALWAYS** name the template `.env.example` (not `.env`)
- **ALWAYS** reference it from Compose via `env_file: .env`

---

## Step 7: Verification Checklist

After generating all files, verify:

```
[ ] Dockerfile: starts with # syntax=docker/dockerfile:1
[ ] Dockerfile: uses multi-stage build (build + runtime stages)
[ ] Dockerfile: dependency manifests copied before source code
[ ] Dockerfile: uses cache mounts for package managers
[ ] Dockerfile: non-root user created and activated
[ ] Dockerfile: HEALTHCHECK instruction present
[ ] Dockerfile: ENTRYPOINT uses exec form
[ ] Dockerfile: no secrets in ENV or ARG
[ ] Dockerfile: pinned base image versions (no :latest)
[ ] .dockerignore: exists and covers language-specific patterns
[ ] Compose: no version: field
[ ] Compose: depends_on uses condition: service_healthy
[ ] Compose: all databases have healthcheck defined
[ ] Compose: named volumes for persistent data
[ ] Compose: env_file used instead of hardcoded secrets
[ ] Compose: resource limits set (production configs)
[ ] .env.example: exists with placeholder values
[ ] .env.example: no real secrets committed
```

---

## Common Generation Patterns

### Development Compose with Watch

```yaml
services:
  app:
    build:
      context: .
      target: build
    ports:
      - "127.0.0.1:3000:3000"
    env_file: .env
    develop:
      watch:
        - action: sync
          path: ./src
          target: /src/src
          ignore:
            - node_modules/
        - action: rebuild
          path: package.json
```

### Production Compose Additions

```yaml
services:
  app:
    build:
      context: .
      target: runtime
    restart: unless-stopped
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 512M
        reservations:
          cpus: '0.25'
          memory: 128M
```

### Database Health Checks (Copy-Paste Ready)

| Database | Health Check |
|----------|-------------|
| PostgreSQL | `["CMD-SHELL", "pg_isready -U $${POSTGRES_USER:-postgres}"]` |
| MySQL | `["CMD", "mysqladmin", "ping", "-h", "localhost"]` |
| MongoDB | `["CMD", "mongosh", "--eval", "db.adminCommand('ping')"]` |
| Redis | `["CMD", "redis-cli", "ping"]` |

---

## Anti-Patterns to Avoid

See [references/anti-patterns.md](references/anti-patterns.md) for the complete list. Critical ones:

- **NEVER** generate a Dockerfile without multi-stage builds
- **NEVER** generate Compose without health checks on databases
- **NEVER** hardcode secrets in Dockerfiles or Compose files
- **NEVER** omit .dockerignore when generating Docker infrastructure
- **NEVER** use `ADD` when `COPY` suffices
- **NEVER** run containers as root in generated configurations

---

## Reference Links

- [references/templates.md](references/templates.md) -- Dockerfile templates per language
- [references/compose-templates.md](references/compose-templates.md) -- Compose templates for common stacks
- [references/anti-patterns.md](references/anti-patterns.md) -- Generation mistakes to avoid

### Official Sources

- https://docs.docker.com/reference/dockerfile/
- https://docs.docker.com/build/building/best-practices/
- https://docs.docker.com/build/building/multi-stage/
- https://docs.docker.com/compose/compose-file/
- https://docs.docker.com/compose/compose-file/05-services/
