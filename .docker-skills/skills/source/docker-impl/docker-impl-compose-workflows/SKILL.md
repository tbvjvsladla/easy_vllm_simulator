---
name: docker-impl-compose-workflows
description: >
  Use when setting up multi-environment Compose workflows or merging
  override files. Prevents environment variable shadowing from incorrect
  precedence and broken extends from circular dependencies.
  Covers profiles, extends, include, compose.override.yaml, .env file
  precedence, compose watch, and multi-file merge patterns.
  Keywords: docker compose, profiles, extends, include, override, .env,
  compose watch, environment, dev vs production compose, multiple environments,
  compose file organization, hot reload.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Compose v2."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-impl-compose-workflows

## Quick Reference

### Compose File Search Order

Compose searches for files in this order:
1. `compose.yaml` (preferred)
2. `compose.yml`
3. `docker-compose.yaml`
4. `docker-compose.yml`

**ALWAYS** use `compose.yaml` for new projects. The `docker-compose.*` names exist only for backward compatibility.

**NEVER** include a `version:` field. It is deprecated and ignored by modern Compose.

### Environment Variable Precedence (Highest to Lowest)

| Priority | Source | Example |
|----------|--------|---------|
| 1 (highest) | `docker compose run -e` CLI flag | `docker compose run -e DEBUG=1 web` |
| 2 | Shell/interpolation in `environment` or `env_file` | `${DEBUG}` resolved from host shell |
| 3 | `environment` attribute in compose.yaml | `environment: DEBUG: "true"` |
| 4 | `env_file` attribute files | `env_file: .env.local` |
| 5 (lowest) | Image `ENV` directive | `ENV DEBUG=false` in Dockerfile |

### Variable Interpolation Syntax

| Syntax | Behavior |
|--------|----------|
| `$VAR` or `${VAR}` | Direct substitution |
| `${VAR:-default}` | Use `default` if VAR is unset **or empty** |
| `${VAR-default}` | Use `default` only if VAR is **unset** |
| `${VAR:?error}` | Error if VAR is unset **or empty** |
| `${VAR?error}` | Error if VAR is **unset** |
| `${VAR:+replacement}` | Use `replacement` if VAR is set **and non-empty** |
| `${VAR+replacement}` | Use `replacement` if VAR is **set** |

Use `$$` to produce a literal `$` sign. Interpolation applies to unquoted and double-quoted values only. Single-quoted values in `.env` files are literal.

### Merge Rules (Multiple Compose Files)

| Field Type | Behavior | Examples |
|------------|----------|----------|
| Scalar (single-value) | Later file **replaces** earlier | `image`, `command`, `mem_limit` |
| Sequence (multi-value) | Values **concatenated** | `ports`, `expose`, `dns`, `tmpfs` |
| Mapping (key-value) | **Merge by key**; later overrides matching keys | `environment`, `labels`, `volumes` |

### Critical Warnings

**NEVER** use environment variables for secrets (passwords, tokens, API keys). ALWAYS use Docker secrets or mounted secret files instead.

**NEVER** assume `depends_on` waits for service readiness. ALWAYS combine with `condition: service_healthy` and a healthcheck.

**ALWAYS** use `docker compose config` to verify the resolved configuration after merging multiple files or applying overrides.

**NEVER** use `container_name` on services you intend to scale. It prevents scaling because names must be unique.

---

## Profiles

### Assignment and Activation

Services WITHOUT a `profiles` attribute are ALWAYS started. Services WITH profiles start only when their profile is activated.

```yaml
services:
  app:                              # No profile = ALWAYS enabled
    image: myapp
  phpmyadmin:
    image: phpmyadmin
    profiles: [debug]               # Only with debug profile
  frontend:
    image: node
    profiles: [frontend, dev]       # Multiple profiles
```

### Activation Methods

```bash
# CLI flag (repeatable)
docker compose --profile debug up
docker compose --profile frontend --profile debug up
docker compose --profile "*" up          # Enable ALL profiles

# Environment variable (comma-separated)
COMPOSE_PROFILES=debug docker compose up
COMPOSE_PROFILES=frontend,debug docker compose up
```

### Auto-Activation

When you explicitly target a profiled service, Compose runs it regardless of profile activation:

```bash
docker compose run db-migrations         # Runs even without --profile tools
```

**Critical constraint**: If the targeted service has profiled dependencies, those dependencies MUST either share the same profile, be started separately, or have no profile assignment.

---

## Extends and Include

### Extends Directive

Inherits configuration from another service without including that service in the final project.

```yaml
# From another file
services:
  web:
    extends:
      file: common-services.yml
      service: webapp
    ports:
      - "8080:80"

# Within the same file
services:
  web:
    extends: webapp
```

Locally-defined attributes ALWAYS override extended values. Relative paths in extended files are automatically converted.

### Include Directive

Imports entire Compose files as independent application models:

```yaml
include:
  - my-compose-include.yaml
  - path:
      - third-party/compose.yaml
      - override.yaml                    # Paired override
  - oci://docker.io/user/app:latest      # Remote OCI source
```

Each included file resolves paths relative to its own directory. Direct resource conflicts between included files cause an error. Works recursively.

---

## Multiple Compose Files

### Override Convention

By default, Compose loads `compose.yaml` THEN `compose.override.yaml` automatically. No `-f` flag needed.

```bash
# Explicit multi-file (processed left-to-right, later overrides earlier)
docker compose -f compose.yaml -f compose.prod.yaml up
docker compose -f compose.yaml -f compose.admin.yaml run backup_db
```

Override files need NOT be complete or valid standalone Compose files. They can be fragments.

**ALWAYS** resolve paths relative to the first (base) file.

### Remote Compose Files

```bash
# OCI Registry
docker compose -f oci://registry.example.com/project:latest up

# Git Repository
docker compose -f https://github.com/user/repo.git up
docker compose -f https://github.com/user/repo.git@v1.0.0 up
docker compose -f git@github.com:user/repo.git#main:path/to/compose.yaml up
```

---

## Compose Watch

### Configuration

```yaml
services:
  web:
    build: .
    develop:
      watch:
        - action: sync
          path: ./web
          target: /src/web
          initial_sync: true
          ignore:
            - node_modules/
        - action: rebuild
          path: package.json
        - action: sync+restart
          path: ./proxy/nginx.conf
          target: /etc/nginx/conf.d/default.conf
```

### Action Types

| Action | Behavior | Use Case |
|--------|----------|----------|
| `sync` | Syncs host files to container path | Hot reload frameworks (React, Vue) |
| `rebuild` | Builds new image, replaces container | Dependency changes, compiled languages |
| `sync+restart` | Syncs files then restarts container | Config file changes (nginx.conf, .ini) |

### Usage

```bash
docker compose up --watch              # Combined with logs
docker compose watch                   # Separate from logs
```

### Constraints

- Works ONLY with services that have a `build` attribute, NEVER with pre-built `image`-only services
- Container image MUST contain `stat`, `mkdir`, `rmdir` utilities
- Container `USER` MUST have write permissions to target paths
- ALWAYS ignore large directories (e.g., `node_modules/`) for performance
- Does NOT support glob patterns in path definitions

---

## Compose CLI Workflow

### Essential Commands

| Command | Purpose |
|---------|---------|
| `docker compose up -d` | Create and start containers (detached) |
| `docker compose down` | Stop and remove containers and networks |
| `docker compose build` | Build or rebuild service images |
| `docker compose ps` | List running containers |
| `docker compose logs -f` | Follow container log output |
| `docker compose exec <svc> <cmd>` | Run command in running container |
| `docker compose run <svc> <cmd>` | Run one-off command on a service |
| `docker compose config` | Validate and display resolved config |
| `docker compose --dry-run up` | Preview changes without executing |

### .env File Rules

- Default location: `.env` next to `compose.yaml`
- Override with: `docker compose --env-file ./config/.env.dev up`
- Multiple files: `docker compose --env-file .env --env-file .env.override up`
- Later files override earlier files
- Lines starting with `#` are comments
- Single-quoted values are literal (no interpolation)
- Double-quoted values support escape sequences: `\n`, `\r`, `\t`, `\\`

---

## Decision Trees

### Which Multi-File Strategy to Use?

```
Need to customize a third-party Compose file?
├── YES → Use `include` with paired override file
└── NO
    ├── Need to share base service config across services?
    │   └── YES → Use `extends` directive
    └── Need environment-specific overrides (dev/prod)?
        ├── YES, two environments → Use compose.override.yaml (auto-loaded)
        └── YES, multiple → Use explicit -f flag with per-environment files
```

### Which Watch Action to Use?

```
What changed?
├── Source code (interpreted language) → sync
├── Config file (needs process restart) → sync+restart
└── Dependencies or compiled code → rebuild
```

---

## Reference Links

- [references/patterns.md](references/patterns.md) -- Profile patterns, merge rules, extend patterns, watch configuration
- [references/examples.md](references/examples.md) -- Dev/prod workflow, multi-file setup, profile-based optional services
- [references/anti-patterns.md](references/anti-patterns.md) -- Workflow mistakes and corrections

### Official Sources

- https://docs.docker.com/compose/how-tos/environment-variables/
- https://docs.docker.com/compose/how-tos/environment-variables/variable-interpolation/
- https://docs.docker.com/compose/how-tos/environment-variables/envvars-precedence/
- https://docs.docker.com/compose/how-tos/profiles/
- https://docs.docker.com/compose/how-tos/multiple-compose-files/merge/
- https://docs.docker.com/compose/how-tos/multiple-compose-files/extends/
- https://docs.docker.com/compose/how-tos/multiple-compose-files/include/
- https://docs.docker.com/compose/how-tos/file-watch/
- https://docs.docker.com/reference/cli/docker/compose/
