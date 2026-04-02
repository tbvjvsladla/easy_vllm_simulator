# Compose Workflow Anti-Patterns

## AP-1: Using compose.override.yaml for Production Settings

**Problem**: `compose.override.yaml` is auto-loaded. If production settings are placed there, they apply during development too, or worse, development overrides leak into production.

```yaml
# WRONG — compose.override.yaml with production config
services:
  app:
    image: registry.example.com/myapp:v2.1.0
    deploy:
      replicas: 3
```

**Fix**: ALWAYS use explicit `-f` flags for non-development environments. Reserve `compose.override.yaml` exclusively for local development overrides.

```bash
# Development (auto-loads compose.override.yaml)
docker compose up

# Production (explicitly skips compose.override.yaml)
docker compose -f compose.yaml -f compose.prod.yaml up -d
```

---

## AP-2: Duplicating Configuration Instead of Using Extends

**Problem**: Copy-pasting identical configuration across services leads to drift and maintenance burden.

```yaml
# WRONG — duplicated config
services:
  web:
    image: python:3.12
    environment:
      PYTHONUNBUFFERED: "1"
      LOG_LEVEL: info
    working_dir: /app

  worker:
    image: python:3.12
    environment:
      PYTHONUNBUFFERED: "1"
      LOG_LEVEL: info
    working_dir: /app
```

**Fix**: ALWAYS use `extends` to share common configuration.

```yaml
services:
  base:
    image: python:3.12
    environment:
      PYTHONUNBUFFERED: "1"
      LOG_LEVEL: info
    working_dir: /app

  web:
    extends: base
    command: gunicorn app:app

  worker:
    extends: base
    command: celery -A tasks worker
```

---

## AP-3: Hardcoding Environment Values in compose.yaml

**Problem**: Hardcoded values prevent environment-specific configuration and risk leaking secrets into version control.

```yaml
# WRONG — hardcoded secret
services:
  db:
    environment:
      POSTGRES_PASSWORD: "my-secret-password"
```

**Fix**: ALWAYS use interpolation with required-variable syntax for secrets and environment-specific values.

```yaml
services:
  db:
    environment:
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:?Database password is required}
```

Commit `.env.example` with empty values. Add `.env` to `.gitignore`.

---

## AP-4: Not Verifying Merged Configuration

**Problem**: Multiple Compose files can produce unexpected merge results. Deploying without verification leads to runtime failures.

```bash
# WRONG — deploy without checking
docker compose -f compose.yaml -f compose.prod.yaml -f compose.monitoring.yaml up -d
```

**Fix**: ALWAYS run `docker compose config` before deploying with multiple files.

```bash
# Verify first
docker compose -f compose.yaml -f compose.prod.yaml config
# Then deploy
docker compose -f compose.yaml -f compose.prod.yaml up -d
```

---

## AP-5: Running Debug Tools Without Profiles

**Problem**: Development and debugging tools running in production waste resources and create security exposure.

```yaml
# WRONG — phpmyadmin always running
services:
  app:
    image: myapp
  phpmyadmin:
    image: phpmyadmin
    ports:
      - "8080:80"
```

**Fix**: ALWAYS place optional/debug services behind profiles.

```yaml
services:
  app:
    image: myapp
  phpmyadmin:
    image: phpmyadmin
    profiles: [dev]
    ports:
      - "127.0.0.1:8080:80"
```

---

## AP-6: Using Bind Mounts Instead of Compose Watch

**Problem**: Bind mounts in production configs create host-dependency. Mixing development bind mounts with production configs causes confusion.

```yaml
# WRONG — bind mount in base compose.yaml
services:
  app:
    image: myapp
    volumes:
      - ./src:/app/src    # Only useful in development
```

**Fix**: Use `compose watch` for development file syncing. Keep bind mounts in `compose.override.yaml` only if watch is not suitable.

```yaml
# compose.yaml — clean base
services:
  app:
    build: .

# compose.override.yaml — development only
services:
  app:
    develop:
      watch:
        - action: sync
          path: ./src
          target: /app/src
          ignore:
            - node_modules/
```

---

## AP-7: Ignoring .env File Precedence Rules

**Problem**: Not understanding that host shell variables override `.env` file values leads to "it works on my machine" issues.

```bash
# Developer A has LOG_LEVEL=debug in their shell
# Developer B does not
# Both use the same .env file with LOG_LEVEL=info
# They get different behavior — confusing
```

**Fix**: ALWAYS document the precedence chain. Use `docker compose config --environment` to verify resolved values. For critical variables, use the `${VAR:?error}` syntax to fail fast on missing values.

```bash
# Verify what Compose sees
docker compose config --environment

# Force a specific value regardless of shell
docker compose run -e LOG_LEVEL=debug app
```

---

## AP-8: Not Ignoring Large Directories in Watch

**Problem**: Watching directories like `node_modules/` causes excessive CPU usage, slow sync, and thousands of unnecessary file events.

```yaml
# WRONG — no ignore list
develop:
  watch:
    - action: sync
      path: ./frontend
      target: /app
```

**Fix**: ALWAYS ignore dependency directories, build outputs, and test artifacts.

```yaml
develop:
  watch:
    - action: sync
      path: ./frontend
      target: /app
      ignore:
        - node_modules/
        - .next/
        - dist/
        - coverage/
        - "*.test.js"
```

---

## AP-9: Using Watch with Image-Only Services

**Problem**: `compose watch` only works with services that have a `build` attribute. Using it with `image`-only services silently does nothing.

```yaml
# WRONG — watch does nothing without build
services:
  app:
    image: myapp:latest
    develop:
      watch:
        - action: sync
          path: ./src
          target: /app/src
```

**Fix**: ALWAYS ensure watched services have a `build` attribute.

```yaml
services:
  app:
    build: .
    develop:
      watch:
        - action: sync
          path: ./src
          target: /app/src
```

---

## AP-10: Conflicting Resources in Include Files

**Problem**: Two included Compose files defining the same service name or volume name causes a hard error.

```yaml
# WRONG — both files define "db" service
include:
  - team-a/compose.yaml    # has services.db
  - team-b/compose.yaml    # also has services.db
# Result: ERROR — conflicting service names
```

**Fix**: Use unique service names across included files. If you need to customize an included service, use the paired override pattern.

```yaml
include:
  - path:
      - team-a/compose.yaml
      - team-a/overrides.yaml    # Customizes team-a services
  - path:
      - team-b/compose.yaml
```

---

## AP-11: Missing Healthchecks on Profile Dependencies

**Problem**: A profiled service depends on a core service without a healthcheck, causing startup race conditions.

```yaml
# WRONG — migration starts before db is ready
services:
  db:
    image: postgres:16

  migrate:
    profiles: [tools]
    command: python manage.py migrate
    depends_on:
      - db
```

**Fix**: ALWAYS add healthchecks to services that other services depend on, and use `condition: service_healthy`.

```yaml
services:
  db:
    image: postgres:16
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5

  migrate:
    profiles: [tools]
    command: python manage.py migrate
    depends_on:
      db:
        condition: service_healthy
```

---

## AP-12: Exposing Ports to All Interfaces in Development

**Problem**: Default port mapping `"8080:80"` binds to `0.0.0.0`, exposing development services to the entire network.

```yaml
# WRONG — exposed to all interfaces
services:
  adminer:
    profiles: [dev]
    ports:
      - "8080:8080"
```

**Fix**: ALWAYS bind development service ports to `127.0.0.1`.

```yaml
services:
  adminer:
    profiles: [dev]
    ports:
      - "127.0.0.1:8080:8080"
```
