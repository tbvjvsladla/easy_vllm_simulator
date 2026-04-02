# Docker Compose Templates for Common Stacks

Complete, valid Compose templates for common application architectures. Every template follows best practices: health checks, named volumes, env_file, depends_on conditions, and no deprecated `version:` field.

---

## Web + PostgreSQL

```yaml
services:
  app:
    build:
      context: .
      target: runtime
    ports:
      - "127.0.0.1:3000:3000"
    env_file: .env
    depends_on:
      db:
        condition: service_healthy
    restart: unless-stopped

  db:
    image: postgres:16-alpine
    volumes:
      - db-data:/var/lib/postgresql/data
    env_file: .env
    environment:
      POSTGRES_USER: ${POSTGRES_USER:-app}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:?Database password is required}
      POSTGRES_DB: ${POSTGRES_DB:-appdb}
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U $${POSTGRES_USER:-app}"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 10s
    restart: unless-stopped

volumes:
  db-data:
```

---

## Web + MySQL

```yaml
services:
  app:
    build:
      context: .
      target: runtime
    ports:
      - "127.0.0.1:3000:3000"
    env_file: .env
    depends_on:
      db:
        condition: service_healthy
    restart: unless-stopped

  db:
    image: mysql:8.4
    volumes:
      - db-data:/var/lib/mysql
    env_file: .env
    environment:
      MYSQL_ROOT_PASSWORD: ${MYSQL_ROOT_PASSWORD:?Root password is required}
      MYSQL_DATABASE: ${MYSQL_DATABASE:-appdb}
      MYSQL_USER: ${MYSQL_USER:-app}
      MYSQL_PASSWORD: ${MYSQL_PASSWORD:?Database password is required}
    healthcheck:
      test: ["CMD", "mysqladmin", "ping", "-h", "localhost"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 30s
    restart: unless-stopped

volumes:
  db-data:
```

---

## Web + PostgreSQL + Redis

```yaml
services:
  app:
    build:
      context: .
      target: runtime
    ports:
      - "127.0.0.1:3000:3000"
    env_file: .env
    environment:
      DATABASE_URL: postgres://${POSTGRES_USER:-app}:${POSTGRES_PASSWORD}@db:5432/${POSTGRES_DB:-appdb}
      REDIS_URL: redis://:${REDIS_PASSWORD}@cache:6379/0
    depends_on:
      db:
        condition: service_healthy
      cache:
        condition: service_healthy
    restart: unless-stopped

  db:
    image: postgres:16-alpine
    volumes:
      - db-data:/var/lib/postgresql/data
    env_file: .env
    environment:
      POSTGRES_USER: ${POSTGRES_USER:-app}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:?Database password is required}
      POSTGRES_DB: ${POSTGRES_DB:-appdb}
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U $${POSTGRES_USER:-app}"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 10s
    restart: unless-stopped

  cache:
    image: redis:7-alpine
    command: redis-server --requirepass ${REDIS_PASSWORD:?Redis password is required}
    volumes:
      - cache-data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "-a", "${REDIS_PASSWORD}", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5
    restart: unless-stopped

volumes:
  db-data:
  cache-data:
```

---

## Full Stack (Frontend + Backend + DB + Cache)

```yaml
services:
  frontend:
    build:
      context: ./frontend
      target: runtime
    ports:
      - "127.0.0.1:80:80"
    depends_on:
      backend:
        condition: service_healthy
    restart: unless-stopped
    networks:
      - frontend

  backend:
    build:
      context: ./backend
      target: runtime
    expose:
      - "8080"
    env_file: .env
    environment:
      DATABASE_URL: postgres://${POSTGRES_USER:-app}:${POSTGRES_PASSWORD}@db:5432/${POSTGRES_DB:-appdb}
      REDIS_URL: redis://:${REDIS_PASSWORD}@cache:6379/0
    depends_on:
      db:
        condition: service_healthy
      cache:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:8080/health"]
      interval: 30s
      timeout: 3s
      retries: 3
      start_period: 10s
    restart: unless-stopped
    networks:
      - frontend
      - backend

  db:
    image: postgres:16-alpine
    volumes:
      - db-data:/var/lib/postgresql/data
    env_file: .env
    environment:
      POSTGRES_USER: ${POSTGRES_USER:-app}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:?Database password is required}
      POSTGRES_DB: ${POSTGRES_DB:-appdb}
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U $${POSTGRES_USER:-app}"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 10s
    restart: unless-stopped
    networks:
      - backend

  cache:
    image: redis:7-alpine
    command: redis-server --requirepass ${REDIS_PASSWORD:?Redis password is required}
    volumes:
      - cache-data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "-a", "${REDIS_PASSWORD}", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5
    restart: unless-stopped
    networks:
      - backend

networks:
  frontend:
  backend:

volumes:
  db-data:
  cache-data:
```

**Network isolation:** The `db` and `cache` services are on the `backend` network only. The `frontend` service cannot reach them directly — only `backend` bridges both networks.

---

## Development Configuration

Use alongside a base `compose.yaml` via `docker compose -f compose.yaml -f compose.dev.yaml up`:

```yaml
services:
  app:
    build:
      context: .
      target: build
    ports:
      - "127.0.0.1:3000:3000"
      - "127.0.0.1:9229:9229"
    env_file: .env
    environment:
      NODE_ENV: development
    develop:
      watch:
        - action: sync
          path: ./src
          target: /src/src
          ignore:
            - node_modules/
        - action: rebuild
          path: package.json

  db:
    ports:
      - "127.0.0.1:5432:5432"

  adminer:
    image: adminer:latest
    ports:
      - "127.0.0.1:8080:8080"
    depends_on:
      db:
        condition: service_healthy
    profiles:
      - debug
```

### Development Configuration Rules

- **ALWAYS** bind ports to `127.0.0.1` to prevent external access
- **ALWAYS** expose debug ports (9229 for Node.js, 5005 for Java, etc.)
- **ALWAYS** expose database ports for direct access with local tools
- **ALWAYS** use the `build` target (not `runtime`) for hot reload support
- **ALWAYS** use `develop.watch` for file synchronization
- **ALWAYS** put admin tools (Adminer, phpMyAdmin) behind a `debug` profile

---

## Production Configuration

Use alongside a base `compose.yaml` via `docker compose -f compose.yaml -f compose.prod.yaml up -d`:

```yaml
services:
  app:
    build:
      context: .
      target: runtime
    ports:
      - "3000:3000"
    restart: unless-stopped
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 512M
        reservations:
          cpus: '0.25'
          memory: 128M
    logging:
      driver: json-file
      options:
        max-size: "10m"
        max-file: "3"

  db:
    restart: unless-stopped
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 1G
        reservations:
          cpus: '0.5'
          memory: 256M
    logging:
      driver: json-file
      options:
        max-size: "10m"
        max-file: "5"

  cache:
    restart: unless-stopped
    deploy:
      resources:
        limits:
          cpus: '0.5'
          memory: 256M
        reservations:
          cpus: '0.1'
          memory: 64M
```

### Production Configuration Rules

- **ALWAYS** set `restart: unless-stopped`
- **ALWAYS** set resource limits and reservations
- **ALWAYS** configure log rotation (`max-size`, `max-file`)
- **NEVER** expose database ports to the host
- **NEVER** bind ports to `127.0.0.1` if the service must be publicly reachable
- **NEVER** include admin tools or debug profiles
- **NEVER** use `develop.watch` in production

---

## Web + MongoDB

```yaml
services:
  app:
    build:
      context: .
      target: runtime
    ports:
      - "127.0.0.1:3000:3000"
    env_file: .env
    environment:
      MONGODB_URI: mongodb://${MONGO_USER:-app}:${MONGO_PASSWORD}@db:27017/${MONGO_DB:-appdb}?authSource=admin
    depends_on:
      db:
        condition: service_healthy
    restart: unless-stopped

  db:
    image: mongo:7
    volumes:
      - db-data:/data/db
    env_file: .env
    environment:
      MONGO_INITDB_ROOT_USERNAME: ${MONGO_USER:-app}
      MONGO_INITDB_ROOT_PASSWORD: ${MONGO_PASSWORD:?MongoDB password is required}
      MONGO_INITDB_DATABASE: ${MONGO_DB:-appdb}
    healthcheck:
      test: ["CMD", "mongosh", "--eval", "db.adminCommand('ping')"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 20s
    restart: unless-stopped

volumes:
  db-data:
```

---

## .env.example Template

**ALWAYS** generate this alongside any Compose configuration:

```env
# ============================================
# Application Configuration
# ============================================
# Copy this file to .env and fill in real values.
# NEVER commit the .env file to version control.
# ============================================

# Application
APP_PORT=3000
NODE_ENV=production

# PostgreSQL
POSTGRES_USER=app
POSTGRES_PASSWORD=changeme
POSTGRES_DB=appdb

# Redis (if applicable)
REDIS_PASSWORD=changeme

# MongoDB (if applicable)
MONGO_USER=app
MONGO_PASSWORD=changeme
MONGO_DB=appdb
```

---

## Template Customization Rules

1. **ALWAYS** remove unused services (do not include Redis if the app does not use caching)
2. **ALWAYS** update port numbers to match the actual application
3. **ALWAYS** update health check commands to match the actual health endpoint
4. **ALWAYS** update environment variable names to match the application's expected configuration
5. **ALWAYS** use `${VAR:?error}` for required variables to fail fast on missing config
6. **ALWAYS** use `${VAR:-default}` for optional variables with sensible defaults
7. **NEVER** include services the application does not need
8. **NEVER** hardcode passwords or secrets directly in Compose files
