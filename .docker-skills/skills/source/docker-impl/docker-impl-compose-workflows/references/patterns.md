# Compose Workflow Patterns

## Profile Patterns

### Categorized Profile Assignment

Organize profiles by purpose. Services without profiles ALWAYS start.

```yaml
services:
  # Core services — no profile, ALWAYS running
  app:
    image: myapp:latest
    depends_on:
      db:
        condition: service_healthy

  db:
    image: postgres:16
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5

  # Development tools — only when needed
  phpmyadmin:
    image: phpmyadmin
    profiles: [dev]
    ports:
      - "127.0.0.1:8081:80"

  mailhog:
    image: mailhog/mailhog
    profiles: [dev]
    ports:
      - "127.0.0.1:8025:8025"

  # Debug/monitoring — heavy tools behind profile
  prometheus:
    image: prom/prometheus
    profiles: [monitoring]

  grafana:
    image: grafana/grafana
    profiles: [monitoring]

  # One-off tasks — run explicitly
  db-migrations:
    image: myapp:latest
    profiles: [tools]
    command: python manage.py migrate
    depends_on:
      db:
        condition: service_healthy
```

### Profile Activation Patterns

```bash
# Development with debug tools
docker compose --profile dev up -d

# Production monitoring
docker compose --profile monitoring up -d

# Run one-off migration (auto-activates service + dependencies)
docker compose run db-migrations

# Enable everything
docker compose --profile "*" up -d

# Via environment variable (useful in CI)
COMPOSE_PROFILES=dev,monitoring docker compose up -d
```

---

## Merge and Override Patterns

### Merge Rules Reference

#### Scalar Fields (Replacement)

Later file completely replaces earlier value:

```yaml
# compose.yaml
services:
  web:
    image: myapp:latest
    command: python app.py

# compose.prod.yaml
services:
  web:
    image: myapp:v2.1.0
    command: gunicorn app:app

# Result: image=myapp:v2.1.0, command=gunicorn app:app
```

#### Sequence Fields (Concatenation)

Values from all files are concatenated:

```yaml
# compose.yaml
services:
  web:
    expose:
      - "3000"
    dns:
      - 8.8.8.8

# compose.override.yaml
services:
  web:
    expose:
      - "4000"
      - "5000"
    dns:
      - 9.9.9.9

# Result: expose=["3000","4000","5000"], dns=["8.8.8.8","9.9.9.9"]
```

Sequence fields: `ports`, `expose`, `external_links`, `dns`, `dns_search`, `tmpfs`

#### Mapping Fields (Smart Merge)

Merge by key; later files override matching keys while preserving unmatched:

```yaml
# compose.yaml
services:
  web:
    environment:
      FOO: original
      BAR: original
    volumes:
      - ./src:/app/src
      - data:/app/data

# compose.override.yaml
services:
  web:
    environment:
      BAR: overridden
      BAZ: new
    volumes:
      - ./src:/app/src:cached

# Result environment: FOO=original, BAR=overridden, BAZ=new
# Result volumes: ./src:/app/src:cached (overridden by mount path), data:/app/data (preserved)
```

Mapping fields: `environment`, `labels`, `volumes`, `devices`

### compose.override.yaml Convention

Compose ALWAYS loads `compose.override.yaml` automatically alongside `compose.yaml`. No flag needed:

```yaml
# compose.yaml — base configuration
services:
  web:
    image: myapp:latest
    ports:
      - "80:80"

# compose.override.yaml — development overrides (auto-loaded)
services:
  web:
    build: .
    ports:
      - "127.0.0.1:8080:80"
    volumes:
      - ./src:/app/src
    environment:
      DEBUG: "true"
```

**ALWAYS** add `compose.override.yaml` to `.gitignore` if it contains developer-specific settings. Provide `compose.override.yaml.example` as a template.

---

## Extends Patterns

### Base Service Inheritance

```yaml
# common-services.yml
services:
  base-python:
    image: python:3.12-slim
    environment:
      PYTHONUNBUFFERED: "1"
      PYTHONDONTWRITEBYTECODE: "1"
    working_dir: /app
    volumes:
      - ./requirements.txt:/app/requirements.txt

# compose.yaml
services:
  web:
    extends:
      file: common-services.yml
      service: base-python
    command: gunicorn app:app
    ports:
      - "8000:8000"

  worker:
    extends:
      file: common-services.yml
      service: base-python
    command: celery -A tasks worker
```

### Same-File Extends

```yaml
services:
  base:
    image: node:20-slim
    working_dir: /app
    environment:
      NODE_ENV: production

  frontend:
    extends: base
    command: npm run start:frontend
    ports:
      - "3000:3000"

  backend:
    extends: base
    command: npm run start:backend
    ports:
      - "4000:4000"
```

### Multi-Level Extension

```yaml
# base.yml
services:
  base:
    image: python:3.12
    environment:
      LOG_LEVEL: info

# web-base.yml
services:
  web-base:
    extends:
      file: base.yml
      service: base
    command: gunicorn app:app

# compose.yaml
services:
  web:
    extends:
      file: web-base.yml
      service: web-base
    ports:
      - "8000:8000"
```

---

## Include Patterns

### Importing Sub-Projects

```yaml
# compose.yaml
include:
  - infra/compose.yaml               # Database, cache, queue
  - monitoring/compose.yaml           # Prometheus, Grafana

services:
  app:
    build: .
    depends_on:
      db:
        condition: service_healthy
```

Each included file resolves paths relative to its own directory.

### Include with Override

```yaml
include:
  - path:
      - third-party/compose.yaml
      - third-party/compose.override.yaml
```

### Remote Include

```yaml
include:
  - oci://docker.io/myorg/infra-compose:latest
  - https://github.com/myorg/shared-compose.git@v2.0.0
```

---

## Watch Configuration Patterns

### Full-Stack Watch Setup

```yaml
services:
  frontend:
    build:
      context: ./frontend
    develop:
      watch:
        - action: sync
          path: ./frontend/src
          target: /app/src
          initial_sync: true
          ignore:
            - node_modules/
            - "*.test.js"
        - action: rebuild
          path: ./frontend/package.json

  backend:
    build:
      context: ./backend
    develop:
      watch:
        - action: sync
          path: ./backend/app
          target: /app/app
          ignore:
            - __pycache__/
            - "*.pyc"
        - action: rebuild
          path: ./backend/requirements.txt
        - action: sync+restart
          path: ./backend/config.ini
          target: /app/config.ini

  nginx:
    build:
      context: ./nginx
    develop:
      watch:
        - action: sync+restart
          path: ./nginx/nginx.conf
          target: /etc/nginx/nginx.conf
```

### Dockerfile Permissions for Watch

The container `USER` MUST have write access to sync targets. Use `COPY --chown` in the Dockerfile:

```dockerfile
FROM node:20-slim
WORKDIR /app
RUN addgroup --system app && adduser --system --ingroup app app
COPY --chown=app:app package*.json ./
RUN npm ci
COPY --chown=app:app . .
USER app
CMD ["npm", "start"]
```

### Path Mapping Behavior

For a source file change at `./app/html/index.html` with `path: ./app`:

| Target | Result in Container |
|--------|-------------------|
| `/app/html` | `/app/html/index.html` |
| `/app/static` | `/app/static/index.html` |
| `/assets` | `/assets/index.html` |

### Ignore Patterns

Patterns are relative to the `path` of the current watch action, NOT the project directory.

Default ignored (no configuration needed):
- `.dockerignore` rules
- Temporary/backup files from common IDEs (Vim, Emacs, JetBrains)
- `.git` directories

**ALWAYS** ignore dependency directories and build artifacts for performance:

```yaml
ignore:
  - node_modules/
  - __pycache__/
  - "*.pyc"
  - .next/
  - dist/
  - build/
```
