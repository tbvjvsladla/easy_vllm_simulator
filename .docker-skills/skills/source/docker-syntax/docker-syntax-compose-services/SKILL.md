---
name: docker-syntax-compose-services
description: >
  Use when writing compose.yaml service definitions, configuring service
  dependencies, or setting up health checks and resource limits.
  Prevents depends_on without condition: service_healthy, which starts
  dependent services before the upstream is actually ready.
  Covers image, build, command, ports, environment, volumes, networks,
  depends_on with health conditions, healthcheck, deploy, restart, logging,
  profiles, and extends directives.
  Keywords: services, depends_on, condition: service_healthy, healthcheck,
  ports, volumes, networks, deploy, restart, profiles, compose.yaml,
  write compose file, service definition, start order, wait for database.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Compose v2."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-syntax-compose-services

## Quick Reference

### Service Definition Structure

```yaml
services:
  service-name:
    image: registry/image:tag        # Container image
    build: ./path                    # Build from Dockerfile
    command: ["executable", "arg"]   # Override CMD
    entrypoint: ["executable"]       # Override ENTRYPOINT
    ports:                           # Port mappings
      - "8080:80"
    environment:                     # Environment variables
      KEY: value
    volumes:                         # Data mounts
      - data:/app/data
    depends_on:                      # Service dependencies
      db:
        condition: service_healthy
    healthcheck:                     # Health monitoring
      test: ["CMD", "curl", "-f", "http://localhost"]
    deploy:                          # Resource limits and replicas
      resources:
        limits:
          memory: 512M
    restart: unless-stopped          # Restart policy
```

### Critical Warnings

**NEVER** use `depends_on` without `condition: service_healthy` when your service requires the dependency to be fully ready. The default `service_started` condition only waits for the container to start, NOT for the application inside to be ready.

**NEVER** set `container_name` on services you intend to scale. Container names must be unique -- setting a fixed name prevents `docker compose up --scale`.

**NEVER** hardcode secrets in `environment`. ALWAYS use Docker secrets or `.env` files with interpolation for sensitive values.

**NEVER** expose ports to all interfaces (`"8080:80"`) in production. ALWAYS bind to a specific interface (`"127.0.0.1:8080:80"`) unless the service must be publicly accessible.

**ALWAYS** combine `restart: always` or `restart: unless-stopped` with `deploy.resources.limits` to prevent a crash-looping container from consuming all system resources.

**ALWAYS** declare named volumes in the top-level `volumes:` section. Anonymous volumes are destroyed on `docker compose down`.

---

## Image and Build

### Image Source

```yaml
services:
  web:
    image: nginx:1.25-alpine              # Tag-based
    image: redis@sha256:0ed5d592...       # Digest-pinned
    image: registry.example.com:5000/app  # Private registry
```

### Build Configuration

```yaml
services:
  app:
    build:
      context: .                    # Build context directory
      dockerfile: prod.Dockerfile   # Custom Dockerfile path
      target: production            # Multi-stage target
      args:
        GIT_COMMIT: ${GIT_COMMIT}   # Build arguments
      cache_from:
        - type=gha                  # GitHub Actions cache
      secrets:
        - db_password               # Build-time secrets
      platforms:
        - linux/amd64
        - linux/arm64
```

When both `build` and `image` are set, `pull_policy` determines precedence. Without `pull_policy`, Compose attempts pulling before building.

---

## Command and Entrypoint

| Form | Syntax | Shell Processing |
|------|--------|-----------------|
| String | `command: bundle exec thin -p 3000` | Passed to `/bin/sh -c` |
| List (exec) | `command: ["php", "-d", "memory=-1"]` | Executed directly |

Set to `null` to use image default. Set to `[]` or `''` to clear.

---

## Ports

### Short vs Long Syntax Comparison

| Feature | Short Syntax | Long Syntax |
|---------|-------------|-------------|
| Basic mapping | `"8080:80"` | `target: 80, published: "8080"` |
| Interface bind | `"127.0.0.1:8080:80"` | `host_ip: 127.0.0.1` |
| Protocol | `"6060:6060/udp"` | `protocol: udp` |
| Port range | `"9090-9091:8080-8081"` | Not supported |
| Named port | Not supported | `name: web` |
| App protocol | Not supported | `app_protocol: http` |
| Random host port | `"3000"` | Omit `published` |

### Short Syntax

```yaml
ports:
  - "8080:80"                  # HOST:CONTAINER
  - "127.0.0.1:8001:8001"     # Bind to localhost
  - "9090-9091:8080-8081"     # Port range
  - "6060:6060/udp"           # UDP protocol
  - "3000"                     # Random host port
```

### Long Syntax

```yaml
ports:
  - name: web
    target: 80
    published: "8080"
    host_ip: 127.0.0.1
    protocol: tcp
    app_protocol: http
    mode: host
```

ALWAYS use long syntax when you need named ports or explicit interface binding for clarity.

---

## Environment Variables

### Precedence (Highest to Lowest)

1. `docker compose run -e` CLI flag
2. Shell interpolation in `environment`/`env_file`
3. `environment` attribute (static values)
4. `env_file` attribute
5. Dockerfile `ENV` directive

### Configuration

```yaml
environment:
  RACK_ENV: development           # Map syntax
  DB_PASSWORD: ${DB_PASSWORD:?Required}  # Fail if unset

env_file:
  - path: ./default.env
    required: false               # Don't error if missing
```

ALWAYS use `${VAR:?message}` for required variables to fail fast with a clear error.

---

## Volumes

### Short vs Long Syntax

```yaml
volumes:
  # Short syntax
  - db-data:/var/lib/postgresql/data        # Named volume
  - ./config:/app/config:ro                 # Bind mount, read-only

  # Long syntax
  - type: volume
    source: db-data
    target: /var/lib/data
    volume:
      nocopy: true
  - type: bind
    source: ./config
    target: /app/config
    read_only: true
    bind:
      create_host_path: true
  - type: tmpfs
    target: /tmp
    tmpfs:
      size: 100M
```

ALWAYS use long syntax for production configurations -- it makes mount type, access mode, and options explicit.

---

## depends_on

### Condition Comparison Table

| Condition | Waits For | Requires | Use Case |
|-----------|-----------|----------|----------|
| `service_started` | Container started | Nothing | Non-critical dependencies |
| `service_healthy` | Healthcheck passes | `healthcheck` on target | Databases, APIs that need warmup |
| `service_completed_successfully` | Exit code 0 | Service exits | Migrations, seed scripts |

### Configuration

```yaml
depends_on:
  db:
    condition: service_healthy
    restart: true              # Restart when dependency updates
  migration:
    condition: service_completed_successfully
    required: false            # Warning instead of error if missing
  redis:
    condition: service_started
```

ALWAYS use `condition: service_healthy` for database dependencies. A started container does NOT mean the database is accepting connections.

---

## Healthcheck

### Pattern Template

```yaml
healthcheck:
  test: ["CMD-SHELL", "<check-command>"]
  interval: 30s          # Time between checks
  timeout: 10s           # Max time for single check
  retries: 3             # Failures before unhealthy
  start_period: 30s      # Grace period at startup
  start_interval: 5s     # Interval during start_period
```

### Common Healthcheck Commands

| Service | Test Command |
|---------|-------------|
| PostgreSQL | `pg_isready -U postgres` |
| MySQL | `mysqladmin ping -h localhost` |
| Redis | `redis-cli ping` |
| HTTP API | `curl -f http://localhost:8080/health` |
| TCP port | `nc -z localhost 5432` |

Set `test: NONE` to disable a healthcheck inherited from the image.

---

## Deploy and Resource Limits

```yaml
deploy:
  replicas: 3
  resources:
    limits:
      cpus: '0.50'
      memory: 512M
      pids: 100
    reservations:
      cpus: '0.25'
      memory: 256M
      devices:
        - capabilities: [gpu]
          driver: nvidia
          count: 1
  restart_policy:
    condition: on-failure
    delay: 5s
    max_attempts: 3
    window: 120s
```

ALWAYS set `resources.limits.memory` for every production service. Without limits, a memory leak in one container can crash the entire host.

---

## Restart Policies

| Policy | Behavior |
|--------|----------|
| `"no"` | Never restart (default). ALWAYS quote -- unquoted `no` is YAML boolean `false` |
| `always` | Restart unconditionally, including after daemon restart |
| `on-failure[:max]` | Restart only on non-zero exit. Optional max retries (`on-failure:3`) |
| `unless-stopped` | Like `always`, but NOT after manual `docker stop` |

ALWAYS use `unless-stopped` for production services -- it respects manual stops while surviving daemon restarts.

---

## Security Configuration

```yaml
services:
  app:
    read_only: true              # Read-only root filesystem
    user: "1000:1000"            # Non-root user
    cap_drop:
      - ALL                      # Drop all capabilities
    cap_add:
      - NET_BIND_SERVICE         # Add back only what's needed
    security_opt:
      - no-new-privileges:true
```

ALWAYS drop all capabilities with `cap_drop: [ALL]` and add back only what the service requires. NEVER use `privileged: true` unless absolutely necessary.

---

## Profiles

```yaml
services:
  app:                           # No profile = ALWAYS enabled
    image: myapp
  debug-tools:
    image: debug-toolkit
    profiles: [debug]            # Only with --profile debug
  monitoring:
    profiles: [monitoring, production]
```

Activate with: `docker compose --profile debug up` or `COMPOSE_PROFILES=debug`.

Services WITHOUT profiles are ALWAYS started. ALWAYS assign profiles to development-only or optional services.

---

## Extends

```yaml
services:
  web:
    extends:
      file: common-services.yml
      service: webapp
    environment:
      API_KEY: ${API_KEY}        # Local values override extended
```

Local attributes ALWAYS override extended values. Relative paths in extended files are automatically converted.

---

## Additional Attributes

| Attribute | Purpose | Key Constraint |
|-----------|---------|---------------|
| `container_name` | Fixed container name | Prevents scaling |
| `hostname` | Container hostname | RFC 1123 compliant |
| `platform` | Target platform | Format: `os[/arch[/variant]]` |
| `pull_policy` | Image pull strategy | `always`, `never`, `missing`, `build` |
| `logging` | Log driver and options | Driver must be available |
| `labels` | Container metadata | Reverse-DNS notation recommended |
| `init: true` | PID 1 init process | Proper signal forwarding |
| `stop_grace_period` | Time before SIGKILL | Default: 10s |

---

## Reference Links

- [references/attributes.md](references/attributes.md) -- Complete service attribute reference with all options and syntax variants
- [references/examples.md](references/examples.md) -- Common service configurations for web, database, cache, and worker services
- [references/anti-patterns.md](references/anti-patterns.md) -- Service configuration mistakes with explanations and corrections

### Official Sources

- https://docs.docker.com/compose/compose-file/05-services/
- https://docs.docker.com/compose/compose-file/build/
- https://docs.docker.com/compose/compose-file/deploy/
- https://docs.docker.com/compose/how-tos/environment-variables/
