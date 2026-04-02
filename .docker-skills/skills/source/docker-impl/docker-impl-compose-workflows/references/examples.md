# Compose Workflow Examples

## Dev/Prod Workflow with Override Files

### Base Configuration (compose.yaml)

```yaml
services:
  app:
    image: myapp:latest
    ports:
      - "80:80"
    environment:
      NODE_ENV: production
      LOG_LEVEL: warn
    depends_on:
      db:
        condition: service_healthy
    restart: unless-stopped
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 512M

  db:
    image: postgres:16
    volumes:
      - db-data:/var/lib/postgresql/data
    environment:
      POSTGRES_DB: ${POSTGRES_DB:?Database name required}
      POSTGRES_USER: ${POSTGRES_USER:?Database user required}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:?Database password required}
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${POSTGRES_USER}"]
      interval: 10s
      timeout: 5s
      retries: 5
    restart: unless-stopped

volumes:
  db-data:
```

### Development Override (compose.override.yaml — auto-loaded)

```yaml
services:
  app:
    build:
      context: .
      target: development
    ports:
      - "127.0.0.1:3000:80"
    environment:
      NODE_ENV: development
      LOG_LEVEL: debug
      DEBUG: "true"
    volumes:
      - ./src:/app/src
    develop:
      watch:
        - action: sync
          path: ./src
          target: /app/src
          ignore:
            - node_modules/
        - action: rebuild
          path: package.json
    restart: "no"

  db:
    ports:
      - "127.0.0.1:5432:5432"
```

### Production Override (compose.prod.yaml — explicit -f)

```yaml
services:
  app:
    image: registry.example.com/myapp:${APP_VERSION:?Version required}
    deploy:
      replicas: 3
      resources:
        limits:
          cpus: '2.0'
          memory: 1G
        reservations:
          cpus: '0.5'
          memory: 256M

  db:
    volumes:
      - db-data:/var/lib/postgresql/data
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 2G
```

### Usage

```bash
# Development (compose.yaml + compose.override.yaml auto-loaded)
docker compose up --watch

# Production (compose.yaml + compose.prod.yaml, override skipped)
docker compose -f compose.yaml -f compose.prod.yaml up -d

# Verify resolved config before deploying
docker compose -f compose.yaml -f compose.prod.yaml config
```

---

## Multi-File Setup with Include

### Project Structure

```
project/
├── compose.yaml                 # Main orchestration
├── compose.override.yaml        # Dev overrides (gitignored)
├── compose.override.yaml.example
├── .env                         # Default env vars
├── .env.example                 # Template (committed)
├── infra/
│   └── compose.yaml             # Database, cache, queue
├── monitoring/
│   └── compose.yaml             # Prometheus, Grafana
└── app/
    ├── Dockerfile
    └── src/
```

### Main Compose File (compose.yaml)

```yaml
include:
  - infra/compose.yaml
  - monitoring/compose.yaml

services:
  app:
    build:
      context: ./app
    ports:
      - "8080:8080"
    environment:
      DATABASE_URL: postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@db:5432/${POSTGRES_DB}
      REDIS_URL: redis://cache:6379
    depends_on:
      db:
        condition: service_healthy
      cache:
        condition: service_started
```

### Infrastructure File (infra/compose.yaml)

```yaml
services:
  db:
    image: postgres:16
    volumes:
      - db-data:/var/lib/postgresql/data
    environment:
      POSTGRES_DB: ${POSTGRES_DB:-myapp}
      POSTGRES_USER: ${POSTGRES_USER:-postgres}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:?Required}
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${POSTGRES_USER:-postgres}"]
      interval: 10s
      timeout: 5s
      retries: 5

  cache:
    image: redis:7-alpine
    volumes:
      - cache-data:/data

  queue:
    image: rabbitmq:3-management
    profiles: [messaging]
    ports:
      - "127.0.0.1:15672:15672"

volumes:
  db-data:
  cache-data:
```

### Monitoring File (monitoring/compose.yaml)

```yaml
services:
  prometheus:
    image: prom/prometheus:latest
    profiles: [monitoring]
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml:ro
    ports:
      - "127.0.0.1:9090:9090"

  grafana:
    image: grafana/grafana:latest
    profiles: [monitoring]
    volumes:
      - grafana-data:/var/lib/grafana
    ports:
      - "127.0.0.1:3001:3000"
    depends_on:
      - prometheus

volumes:
  grafana-data:
```

### Usage

```bash
# Core services only
docker compose up -d

# Core + monitoring
docker compose --profile monitoring up -d

# Core + messaging + monitoring
docker compose --profile messaging --profile monitoring up -d
```

---

## Profile-Based Optional Services

### Debug and Admin Tools

```yaml
services:
  # Core — always running
  web:
    build: .
    ports:
      - "8080:8080"
    depends_on:
      db:
        condition: service_healthy

  db:
    image: postgres:16
    healthcheck:
      test: ["CMD-SHELL", "pg_isready"]
      interval: 10s
      timeout: 5s
      retries: 5
    volumes:
      - db-data:/var/lib/postgresql/data

  # Dev tools
  adminer:
    image: adminer
    profiles: [dev]
    ports:
      - "127.0.0.1:8081:8080"

  mailpit:
    image: axllent/mailpit
    profiles: [dev]
    ports:
      - "127.0.0.1:8025:8025"
      - "127.0.0.1:1025:1025"

  # Testing
  test-runner:
    build:
      context: .
      target: test
    profiles: [test]
    command: pytest
    depends_on:
      db:
        condition: service_healthy
    environment:
      DATABASE_URL: postgres://postgres:postgres@db:5432/test

  # Seed/migration tools
  seed:
    build: .
    profiles: [tools]
    command: python manage.py seed
    depends_on:
      db:
        condition: service_healthy

  migrate:
    build: .
    profiles: [tools]
    command: python manage.py migrate
    depends_on:
      db:
        condition: service_healthy

volumes:
  db-data:
```

### Usage Patterns

```bash
# Daily development
docker compose --profile dev up -d

# Run tests (auto-starts db dependency)
docker compose run test-runner

# Run migration (auto-starts db dependency)
docker compose run migrate

# Seed database
docker compose run seed

# CI pipeline
COMPOSE_PROFILES=test docker compose up --abort-on-container-exit
```

---

## Environment Variable Workflow

### .env File (committed as .env.example, actual .env gitignored)

```bash
# .env.example — committed to repo
POSTGRES_DB=myapp
POSTGRES_USER=postgres
POSTGRES_PASSWORD=
APP_VERSION=latest
NODE_ENV=development

# .env — local overrides (gitignored)
POSTGRES_DB=myapp
POSTGRES_USER=postgres
POSTGRES_PASSWORD=supersecret
APP_VERSION=2.1.0
NODE_ENV=development
```

### Compose with Required Variables

```yaml
services:
  app:
    image: myapp:${APP_VERSION:?APP_VERSION is required}
    environment:
      # Required — fails if missing
      DATABASE_URL: postgres://${POSTGRES_USER:?}:${POSTGRES_PASSWORD:?}@db:5432/${POSTGRES_DB:?}
      # Optional with default
      LOG_LEVEL: ${LOG_LEVEL:-info}
      # Optional — only set if present
      SENTRY_DSN: ${SENTRY_DSN+${SENTRY_DSN}}
```

### Multiple .env Files for Environments

```bash
# Load base + environment-specific
docker compose --env-file .env --env-file .env.staging up -d

# Verify interpolation result
docker compose --env-file .env --env-file .env.staging config --environment
```

---

## Compose Watch Full Example

### Node.js + Python + Nginx Stack

```yaml
services:
  frontend:
    build:
      context: ./frontend
      target: development
    develop:
      watch:
        - action: sync
          path: ./frontend/src
          target: /app/src
          initial_sync: true
          ignore:
            - node_modules/
            - "*.test.tsx"
            - __tests__/
        - action: rebuild
          path: ./frontend/package.json

  api:
    build:
      context: ./api
    develop:
      watch:
        - action: sync
          path: ./api/app
          target: /app/app
          ignore:
            - __pycache__/
            - "*.pyc"
        - action: sync+restart
          path: ./api/gunicorn.conf.py
          target: /app/gunicorn.conf.py
        - action: rebuild
          path: ./api/requirements.txt

  nginx:
    build:
      context: ./nginx
    ports:
      - "127.0.0.1:8080:80"
    develop:
      watch:
        - action: sync+restart
          path: ./nginx/nginx.conf
          target: /etc/nginx/nginx.conf
    depends_on:
      - frontend
      - api
```

### Starting Watch Mode

```bash
# Start with watch (logs + watch combined)
docker compose up --watch

# Or start detached, then watch separately
docker compose up -d
docker compose watch
```

---

## Remote Compose Files

### OCI Registry

```bash
# Pull and run from OCI registry
docker compose -f oci://registry.example.com/myapp:latest up -d

# Specific version
docker compose -f oci://registry.example.com/myapp:v2.1.0 up -d
```

### Git Repository

```bash
# Default branch
docker compose -f https://github.com/myorg/infra.git up -d

# Specific branch
docker compose -f https://github.com/myorg/infra.git@staging up -d

# Specific tag
docker compose -f https://github.com/myorg/infra.git@v1.0.0 up -d

# Subdirectory in repo
docker compose -f git@github.com:myorg/infra.git#main:docker/compose.yaml up -d

# Combine remote base with local override
docker compose -f https://github.com/myorg/base.git@v1.0.0 -f compose.local.yaml up -d
```
