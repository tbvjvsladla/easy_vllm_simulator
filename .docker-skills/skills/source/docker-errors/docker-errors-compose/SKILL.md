---
name: docker-errors-compose
description: >
  Use when docker compose up fails, services refuse to start, or
  dependency health checks time out.
  Prevents cascading failures from unvalidated Compose files, incorrect
  depends_on conditions, and unescaped environment variable interpolation.
  Covers service dependency errors, orphan containers, port conflicts,
  volume permission issues, profile resolution, build context errors.
  Keywords: docker compose up, docker compose config, depends_on,
  service_healthy, orphan containers, variable interpolation, profiles,
  compose won't start, service keeps restarting, port already in use,
  services not connecting.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Compose v2."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-errors-compose

## Quick Reference

### First Response: Validate the Compose File

**ALWAYS** run `docker compose config` before debugging any Compose error. This command parses, resolves variables, and renders the final configuration. If it fails, the Compose file itself is broken.

```bash
# Validate and render resolved config
docker compose config

# Validate without output (exit code only)
docker compose config -q

# Show resolved environment variables
docker compose config --environment
```

### Critical Warnings

**NEVER** ignore `docker compose config` validation errors — they indicate structural problems that ALWAYS cause runtime failures.

**NEVER** use `depends_on` without `condition: service_healthy` when the dependent service needs initialization time (databases, message brokers). The default `service_started` condition only waits for the container to start, NOT for the application to be ready.

**NEVER** use the deprecated `version:` field — it is ignored by Compose v2 and generates warnings.

**ALWAYS** use named volumes for persistent data. Anonymous volumes are lost on `docker compose down`.

**ALWAYS** quote port mappings that start with numbers below 60 to prevent YAML parsing as sexagesimal (base-60) numbers.

---

## Diagnostic Decision Tree

### Compose File Won't Parse

```
docker compose config fails
├── YAML syntax error?
│   ├── Check indentation (spaces only, NEVER tabs)
│   ├── Check colons have space after them in mappings
│   └── Check strings with special chars are quoted
├── "services" missing?
│   └── The `services` key is REQUIRED — add it
├── Unknown attribute error?
│   ├── Check spelling of Compose directives
│   └── Check indentation level (attribute under wrong parent)
└── Variable interpolation error?
    ├── Unset variable? → Use ${VAR:-default} or set in .env
    ├── Dollar sign in value? → Escape with $$ (e.g., $$HOME)
    └── Single-quoted in .env? → Values are literal, no interpolation
```

### Services Won't Start

```
docker compose up fails
├── Port conflict?
│   ├── "port is already allocated" → Find process: lsof -i :PORT
│   └── Two services using same host port → Change one
├── Dependency failure?
│   ├── "dependency failed to start" → Check dependent service logs
│   ├── Healthcheck timeout → Increase interval/retries/start_period
│   └── service_completed_successfully never exits 0 → Fix init service
├── Build failure?
│   ├── "build path ... does not exist" → Check context path
│   ├── Dockerfile not found → Check dockerfile path relative to context
│   └── Build context too large → Add .dockerignore
├── Image not found?
│   ├── "pull access denied" → docker login or check image name
│   └── "manifest unknown" → Verify tag exists in registry
├── Volume mount error?
│   ├── "permission denied" → Match UID/GID or fix ownership
│   ├── Named volume not declared → Add to top-level volumes:
│   └── External volume missing → Create it first
└── Environment variable error?
    ├── "variable is not set" → Define in .env or environment
    └── Wrong value resolved → Check precedence order
```

### Orphan Container Warnings

```
"Found orphan containers"
├── Service removed from compose.yaml → docker compose down --remove-orphans
├── Project name changed → Use consistent -p flag or COMPOSE_PROJECT_NAME
└── Suppress warning → Set COMPOSE_IGNORE_ORPHANS=true
```

---

## Error Diagnostic Table

| Error Message | Cause | Fix |
|---|---|---|
| `yaml: line N: did not find expected key` | YAML indentation or syntax error | Check line N for tabs (use spaces), missing colons, or unquoted special characters |
| `services is required` | Missing `services:` top-level key | Add `services:` as the top-level element |
| `service "X" refers to undefined network "Y"` | Network used but not declared | Add network Y to top-level `networks:` section |
| `service "X" refers to undefined volume "Y"` | Named volume used but not declared | Add volume Y to top-level `volumes:` section |
| `port is already allocated` | Host port in use by another container or process | Find occupant: `lsof -i :PORT` or `ss -tlnp \| grep PORT`. Use a different host port |
| `Bind for 0.0.0.0:PORT failed` | Same as above — port conflict | Same fix as above |
| `dependency "X" is not a valid service` | depends_on references nonexistent service | Check service name spelling in depends_on |
| `Found orphan containers` | Services removed or project name changed | Run `docker compose down --remove-orphans` |
| `variable "X" is not set` | Interpolation variable undefined | Define in `.env` file, or use `${X:-default}` syntax |
| `invalid interpolation format` | Malformed variable syntax | Check for unescaped `$`. Use `$$` for literal dollar sign |
| `build path /path does not exist` | Build context directory missing | Verify `context:` path exists relative to compose.yaml |
| `Cannot locate specified Dockerfile` | Dockerfile path wrong | Check `dockerfile:` is relative to `context:`, not compose.yaml |
| `pull access denied for X` | Image not found or authentication required | Run `docker login`. Verify image name and tag |
| `no matching manifest for linux/amd64` | Image not available for platform | Add `platform:` to service or use compatible image |
| `service "X" has neither an image nor a build context` | Service needs image or build | Add `image:` or `build:` to the service definition |
| `"version" is obsolete` | Deprecated version field | Remove the `version:` line entirely |
| `invalid service name "X"` | Service name contains invalid characters | Use only `[a-zA-Z0-9._-]`, start with letter or digit |
| `Compose file not found` | No compose.yaml in directory | Create `compose.yaml` or use `-f` flag to specify path |
| `container name "X" is already in use` | Stale container with same name exists | Run `docker compose down` first, or `docker rm X` |
| `network X declared as external but could not be found` | External network does not exist | Create it: `docker network create X` |
| `volume X declared as external but not found` | External volume does not exist | Create it: `docker volume create X` |

---

## Common Patterns

### Proper Service Dependencies with Healthcheck

```yaml
services:
  app:
    image: myapp:latest
    depends_on:
      db:
        condition: service_healthy
      redis:
        condition: service_healthy

  db:
    image: postgres:16
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 30s

  redis:
    image: redis:7-alpine
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 3
```

### Safe Port Mapping

```yaml
ports:
  # ALWAYS quote port mappings to prevent YAML parsing issues
  - "8080:80"
  # Bind to localhost for development
  - "127.0.0.1:5432:5432"
```

### Environment Variable with Required Check

```yaml
environment:
  DATABASE_URL: ${DATABASE_URL:?DATABASE_URL must be set}
  LOG_LEVEL: ${LOG_LEVEL:-info}
```

### Validation Workflow

```bash
# Step 1: Validate syntax and variable resolution
docker compose config -q

# Step 2: Start with build and force recreation
docker compose up --build --force-recreate -d

# Step 3: Check service status
docker compose ps

# Step 4: Check logs for failing services
docker compose logs --tail 50 <service-name>
```

---

## Profile Dependency Resolution

When a profiled service depends on another profiled service, both profiles must be active or the dependency must have no profile:

```yaml
services:
  app:
    image: myapp

  debug-tools:
    image: debug:latest
    profiles: [debug]
    depends_on:
      - app           # OK: app has no profile, always available

  test-runner:
    image: test:latest
    profiles: [test]
    depends_on:
      - debug-tools   # FAILS unless debug profile is also active
```

**Fix**: Either activate both profiles (`--profile test --profile debug`), remove the cross-profile dependency, or remove the profile from the dependency.

---

## Reference Links

- [references/diagnostics.md](references/diagnostics.md) -- Complete error message to cause to solution mapping
- [references/examples.md](references/examples.md) -- Common Compose error scenarios with step-by-step fixes
- [references/anti-patterns.md](references/anti-patterns.md) -- Compose configuration mistakes and corrections

### Official Sources

- https://docs.docker.com/compose/compose-file/
- https://docs.docker.com/compose/compose-file/05-services/
- https://docs.docker.com/compose/how-tos/environment-variables/variable-interpolation/
- https://docs.docker.com/compose/how-tos/profiles/
- https://docs.docker.com/reference/cli/docker/compose/
