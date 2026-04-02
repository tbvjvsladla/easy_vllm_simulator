# Docker Review Examples

Real-world review scenarios showing good configurations, bad configurations, and the fixes needed.

---

## Scenario 1: Node.js Application Dockerfile

### Bad Dockerfile

```dockerfile
FROM node:latest
COPY . /app
WORKDIR /app
RUN npm install
EXPOSE 3000
CMD npm start
```

### Issues Found

| ID | Severity | Issue | Location |
|----|----------|-------|----------|
| CRIT-001 | Critical | Running as root (no USER instruction) | Entire Dockerfile |
| CRIT-002 | Critical | Using `latest` tag (non-deterministic) | FROM line |
| WARN-001 | Warning | Missing syntax directive | Top of file |
| WARN-002 | Warning | CMD in shell form (no signal handling) | CMD line |
| WARN-003 | Warning | COPY . . before npm install (cache bust) | COPY line |
| WARN-004 | Warning | No .dockerignore (node_modules sent to context) | Project root |
| WARN-005 | Warning | No HEALTHCHECK defined | Entire Dockerfile |
| WARN-006 | Warning | No multi-stage build (dev deps in prod) | Entire Dockerfile |
| INFO-001 | Info | No OCI labels | Entire Dockerfile |

### Fixed Dockerfile

```dockerfile
# syntax=docker/dockerfile:1

# ---- Build Stage ----
FROM node:20.11-bookworm-slim AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --production
COPY . .

# ---- Production Stage ----
FROM node:20.11-bookworm-slim AS production

LABEL org.opencontainers.image.title="My Node App" \
      org.opencontainers.image.version="1.0.0"

RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser

WORKDIR /app
COPY --from=build --chown=appuser:appuser /app ./

USER appuser

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD node -e "require('http').get('http://localhost:3000/health', (r) => { process.exit(r.statusCode === 200 ? 0 : 1) })" || exit 1

CMD ["node", "server.js"]
```

---

## Scenario 2: Python API Dockerfile

### Bad Dockerfile

```dockerfile
FROM python:3.12
ADD . /app
WORKDIR /app
RUN pip install -r requirements.txt
ENV SECRET_KEY=my-super-secret-key
ENTRYPOINT python app.py
```

### Issues Found

| ID | Severity | Issue | Location |
|----|----------|-------|----------|
| CRIT-001 | Critical | Secret in ENV (visible in docker history) | ENV SECRET_KEY line |
| CRIT-002 | Critical | Running as root | Entire Dockerfile |
| CRIT-003 | Critical | ENTRYPOINT shell form (signals not forwarded) | ENTRYPOINT line |
| WARN-001 | Warning | ADD used instead of COPY | ADD line |
| WARN-002 | Warning | Full Python image (900MB+) | FROM line |
| WARN-003 | Warning | No cache mount for pip | RUN pip install |
| WARN-004 | Warning | Missing syntax directive | Top of file |
| WARN-005 | Warning | No HEALTHCHECK | Entire Dockerfile |

### Fixed Dockerfile

```dockerfile
# syntax=docker/dockerfile:1

FROM python:3.12-slim AS production

LABEL org.opencontainers.image.title="My Python API"

RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser

WORKDIR /app

COPY requirements.txt .
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install --no-compile -r requirements.txt

COPY --chown=appuser:appuser . .

USER appuser

EXPOSE 8000

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD python -c "import urllib.request; urllib.request.urlopen('http://localhost:8000/health')" || exit 1

ENTRYPOINT ["python", "app.py"]
```

Secret handling at runtime:
```bash
# Pass secret via environment at runtime (NOT baked into image)
docker run -e SECRET_KEY="$(cat secret.txt)" myapp

# Or use Docker secrets in Compose
```

---

## Scenario 3: Go Application with Multi-Stage Build

### Bad Dockerfile

```dockerfile
FROM golang:1.22
WORKDIR /app
COPY . .
RUN go build -o server .
CMD ["./server"]
```

### Issues Found

| ID | Severity | Issue | Location |
|----|----------|-------|----------|
| WARN-001 | Warning | No multi-stage (Go SDK in production ~800MB) | FROM line |
| WARN-002 | Warning | No cache mounts for Go modules | RUN go build |
| WARN-003 | Warning | Running as root | Entire Dockerfile |
| WARN-004 | Warning | Missing syntax directive | Top of file |
| INFO-001 | Info | Could use scratch for static binary | FROM line |

### Fixed Dockerfile

```dockerfile
# syntax=docker/dockerfile:1

FROM golang:1.22-alpine AS build
WORKDIR /src
COPY go.mod go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod \
    go mod download
COPY . .
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=0 go build -o /bin/server .

FROM scratch
COPY --from=build /bin/server /server
USER 65534:65534
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
  CMD ["/server", "healthcheck"]
ENTRYPOINT ["/server"]
```

---

## Scenario 4: Compose File Review

### Bad Compose File

```yaml
version: "3.8"

services:
  web:
    build: .
    ports:
      - "8080:80"
    depends_on:
      - db
    environment:
      DATABASE_URL: "postgres://admin:secret123@db:5432/myapp"
    restart: always
    container_name: my-web

  db:
    image: postgres
    volumes:
      - /var/lib/postgresql/data
    environment:
      POSTGRES_PASSWORD: secret123

  adminer:
    image: adminer
    ports:
      - "9090:8080"
```

### Issues Found

| ID | Severity | Issue | Location |
|----|----------|-------|----------|
| CRIT-001 | Critical | Hardcoded database password | web.environment, db.environment |
| CRIT-002 | Critical | Anonymous volume for database data | db.volumes |
| WARN-001 | Warning | version field present (deprecated) | Top of file |
| WARN-002 | Warning | depends_on without health condition | web.depends_on |
| WARN-003 | Warning | postgres using latest tag | db.image |
| WARN-004 | Warning | Ports exposed to all interfaces | web.ports, adminer.ports |
| WARN-005 | Warning | restart: always without resource limits | web.restart |
| WARN-006 | Warning | container_name prevents scaling | web.container_name |
| WARN-007 | Warning | No healthcheck on db | db service |
| WARN-008 | Warning | Adminer always running (no profile) | adminer service |
| WARN-009 | Warning | No resource limits | All services |
| WARN-010 | Warning | No log rotation configured | All services |
| INFO-001 | Info | No network isolation | All services on default network |

### Fixed Compose File

```yaml
name: my-project

services:
  web:
    build: .
    ports:
      - "127.0.0.1:8080:80"
    depends_on:
      db:
        condition: service_healthy
    env_file:
      - .env
    environment:
      DATABASE_URL: "postgres://${DB_USER}:${DB_PASS:?Database password required}@db:5432/${DB_NAME}"
    restart: unless-stopped
    networks:
      - frontend
      - backend
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 512M
    logging:
      options:
        max-size: "10m"
        max-file: "3"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:80/health"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s

  db:
    image: postgres:16
    volumes:
      - db-data:/var/lib/postgresql/data
    env_file:
      - .env.db
    environment:
      POSTGRES_PASSWORD: ${DB_PASS:?Database password required}
    restart: unless-stopped
    networks:
      - backend
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 1G
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5
    logging:
      options:
        max-size: "10m"
        max-file: "3"

  adminer:
    image: adminer:4
    ports:
      - "127.0.0.1:9090:8080"
    profiles:
      - debug
    networks:
      - backend
    deploy:
      resources:
        limits:
          cpus: '0.25'
          memory: 128M

networks:
  frontend:
  backend:

volumes:
  db-data:
```

---

## Scenario 5: Security Audit -- Privileged Container

### Bad Configuration

```yaml
services:
  app:
    image: myapp:latest
    privileged: true
    ports:
      - "80:80"
    volumes:
      - /:/host
    network_mode: host
```

### Issues Found

| ID | Severity | Issue | Location |
|----|----------|-------|----------|
| CRIT-001 | Critical | privileged: true (full host access) | app.privileged |
| CRIT-002 | Critical | Host root mounted into container | app.volumes |
| CRIT-003 | Critical | Host network mode (no isolation) | app.network_mode |
| CRIT-004 | Critical | Using latest tag | app.image |
| WARN-001 | Warning | No resource limits | app service |
| WARN-002 | Warning | No healthcheck | app service |

### Fixed Configuration

```yaml
services:
  app:
    image: myapp:1.5.2
    cap_drop:
      - ALL
    cap_add:
      - NET_BIND_SERVICE
    security_opt:
      - no-new-privileges:true
    read_only: true
    tmpfs:
      - /tmp
      - /run
    ports:
      - "127.0.0.1:80:80"
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 512M
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:80/health"]
      interval: 30s
      timeout: 5s
      retries: 3
```

---

## Review Report Example

```
## Docker Configuration Review Report

### Summary
- Total issues found: 12
- Critical (blocks deployment): 3
- Warning (should fix): 7
- Info (improvement suggestion): 2

### Critical Issues
1. [CRIT-001] Hardcoded database password in compose.yaml -- web.environment.DATABASE_URL -- Move to .env file with ${DB_PASS:?Required}
2. [CRIT-002] Anonymous volume for PostgreSQL data -- db.volumes -- Use named volume: db-data:/var/lib/postgresql/data
3. [CRIT-003] Running as root in Dockerfile -- No USER instruction -- Add non-root user and USER directive

### Warnings
1. [WARN-001] version field present -- compose.yaml line 1 -- Remove (deprecated)
2. [WARN-002] depends_on without health condition -- web.depends_on -- Add condition: service_healthy
3. [WARN-003] PostgreSQL using implicit latest tag -- db.image -- Pin to postgres:16
4. [WARN-004] Ports exposed to 0.0.0.0 -- web.ports -- Bind to 127.0.0.1
5. [WARN-005] No resource limits -- All services -- Add deploy.resources.limits
6. [WARN-006] No log rotation -- All services -- Add logging.options with max-size
7. [WARN-007] Adminer always running -- adminer service -- Add profiles: [debug]

### Info
1. [INFO-001] No network isolation -- All services on default -- Add frontend/backend networks
2. [INFO-002] No OCI labels in Dockerfile -- Dockerfile -- Add org.opencontainers.image.* labels

### Passed Checks
- Dockerfile Validation: FAIL (8/15 checks passed)
- Compose Validation: FAIL (3/12 checks passed)
- Security Audit: FAIL (2/11 checks passed)
- Production Readiness: FAIL (1/8 checks passed)
- Anti-Pattern Scan: FAIL (5/20 patterns detected)
```
