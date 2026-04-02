# Service Configuration Examples

Production-ready service configurations for common use cases. All examples follow best practices: healthchecks, resource limits, security hardening, and named volumes.

---

## Web Server (Nginx Reverse Proxy)

```yaml
services:
  nginx:
    image: nginx:1.25-alpine
    ports:
      - "127.0.0.1:80:80"
      - "127.0.0.1:443:443"
    volumes:
      - type: bind
        source: ./nginx/conf.d
        target: /etc/nginx/conf.d
        read_only: true
      - type: bind
        source: ./nginx/ssl
        target: /etc/nginx/ssl
        read_only: true
      - nginx-cache:/var/cache/nginx
    healthcheck:
      test: ["CMD-SHELL", "curl -f http://localhost/health || exit 1"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s
    deploy:
      resources:
        limits:
          cpus: '0.50'
          memory: 256M
    restart: unless-stopped
    read_only: true
    tmpfs:
      - /var/run
      - /tmp
    cap_drop:
      - ALL
    cap_add:
      - NET_BIND_SERVICE
      - CHOWN
      - SETGID
      - SETUID
    depends_on:
      app:
        condition: service_healthy
    networks:
      - frontend

volumes:
  nginx-cache:

networks:
  frontend:
```

---

## Application Server (Node.js)

```yaml
services:
  app:
    build:
      context: .
      dockerfile: Dockerfile
      target: production
      args:
        NODE_ENV: production
    ports:
      - "127.0.0.1:3000:3000"
    environment:
      NODE_ENV: production
      DATABASE_URL: postgresql://${DB_USER}:${DB_PASSWORD}@db:5432/${DB_NAME}
      REDIS_URL: redis://redis:6379
    env_file:
      - path: .env
        required: true
    healthcheck:
      test: ["CMD-SHELL", "curl -f http://localhost:3000/health || exit 1"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 30s
      start_interval: 5s
    deploy:
      replicas: 2
      resources:
        limits:
          cpus: '1.0'
          memory: 512M
        reservations:
          cpus: '0.25'
          memory: 128M
    restart: unless-stopped
    init: true
    read_only: true
    tmpfs:
      - /tmp
    cap_drop:
      - ALL
    user: "1000:1000"
    depends_on:
      db:
        condition: service_healthy
      redis:
        condition: service_healthy
      migration:
        condition: service_completed_successfully
    networks:
      - frontend
      - backend

networks:
  frontend:
  backend:
```

---

## Database (PostgreSQL)

```yaml
services:
  db:
    image: postgres:16-alpine
    volumes:
      - db-data:/var/lib/postgresql/data
      - type: bind
        source: ./init-scripts
        target: /docker-entrypoint-initdb.d
        read_only: true
    environment:
      POSTGRES_DB: ${DB_NAME:?Database name required}
      POSTGRES_USER: ${DB_USER:?Database user required}
      POSTGRES_PASSWORD: ${DB_PASSWORD:?Database password required}
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${DB_USER} -d ${DB_NAME}"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 30s
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 1G
        reservations:
          cpus: '0.50'
          memory: 256M
    restart: unless-stopped
    shm_size: 256M
    cap_drop:
      - ALL
    cap_add:
      - CHOWN
      - FOWNER
      - SETGID
      - SETUID
      - DAC_READ_SEARCH
    networks:
      - backend

volumes:
  db-data:

networks:
  backend:
```

---

## Database (MySQL/MariaDB)

```yaml
services:
  mysql:
    image: mysql:8.0
    volumes:
      - mysql-data:/var/lib/mysql
    environment:
      MYSQL_ROOT_PASSWORD: ${MYSQL_ROOT_PASSWORD:?Root password required}
      MYSQL_DATABASE: ${DB_NAME}
      MYSQL_USER: ${DB_USER}
      MYSQL_PASSWORD: ${DB_PASSWORD}
    healthcheck:
      test: ["CMD-SHELL", "mysqladmin ping -h localhost -u root -p${MYSQL_ROOT_PASSWORD}"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 30s
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 1G
    restart: unless-stopped
    cap_drop:
      - ALL
    cap_add:
      - CHOWN
      - FOWNER
      - SETGID
      - SETUID
      - DAC_OVERRIDE
    networks:
      - backend

volumes:
  mysql-data:

networks:
  backend:
```

---

## Cache (Redis)

```yaml
services:
  redis:
    image: redis:7-alpine
    command: ["redis-server", "--maxmemory", "256mb", "--maxmemory-policy", "allkeys-lru"]
    volumes:
      - redis-data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 3
      start_period: 10s
    deploy:
      resources:
        limits:
          cpus: '0.50'
          memory: 300M
    restart: unless-stopped
    read_only: true
    cap_drop:
      - ALL
    networks:
      - backend

volumes:
  redis-data:

networks:
  backend:
```

---

## Background Worker (Celery-style)

```yaml
services:
  worker:
    build:
      context: .
      target: production
    command: ["celery", "-A", "app", "worker", "--loglevel=info", "--concurrency=4"]
    environment:
      CELERY_BROKER_URL: redis://redis:6379/0
      CELERY_RESULT_BACKEND: redis://redis:6379/1
      DATABASE_URL: postgresql://${DB_USER}:${DB_PASSWORD}@db:5432/${DB_NAME}
    env_file:
      - .env
    deploy:
      replicas: 2
      resources:
        limits:
          cpus: '1.0'
          memory: 512M
        reservations:
          cpus: '0.25'
          memory: 128M
    restart: unless-stopped
    init: true
    read_only: true
    tmpfs:
      - /tmp
    cap_drop:
      - ALL
    user: "1000:1000"
    depends_on:
      redis:
        condition: service_healthy
      db:
        condition: service_healthy
    networks:
      - backend

networks:
  backend:
```

---

## Scheduled Tasks (Cron Worker)

```yaml
services:
  scheduler:
    build:
      context: .
      target: production
    command: ["celery", "-A", "app", "beat", "--loglevel=info"]
    environment:
      CELERY_BROKER_URL: redis://redis:6379/0
    deploy:
      replicas: 1
      resources:
        limits:
          cpus: '0.25'
          memory: 128M
    restart: unless-stopped
    init: true
    cap_drop:
      - ALL
    user: "1000:1000"
    depends_on:
      redis:
        condition: service_healthy
    networks:
      - backend

networks:
  backend:
```

---

## Database Migration (One-Shot)

```yaml
services:
  migration:
    build:
      context: .
      target: production
    command: ["python", "manage.py", "migrate", "--noinput"]
    environment:
      DATABASE_URL: postgresql://${DB_USER}:${DB_PASSWORD}@db:5432/${DB_NAME}
    depends_on:
      db:
        condition: service_healthy
    restart: "no"
    cap_drop:
      - ALL
    user: "1000:1000"
    networks:
      - backend

networks:
  backend:
```

Use `depends_on: migration: condition: service_completed_successfully` in the application service to wait for migrations.

---

## Debug Tools (Profile-Gated)

```yaml
services:
  pgadmin:
    image: dpage/pgadmin4:latest
    profiles: [debug]
    ports:
      - "127.0.0.1:5050:80"
    environment:
      PGADMIN_DEFAULT_EMAIL: admin@local.dev
      PGADMIN_DEFAULT_PASSWORD: ${PGADMIN_PASSWORD:-admin}
    volumes:
      - pgadmin-data:/var/lib/pgadmin
    deploy:
      resources:
        limits:
          cpus: '0.50'
          memory: 512M
    restart: unless-stopped
    depends_on:
      db:
        condition: service_healthy
    networks:
      - backend

  mailhog:
    image: mailhog/mailhog:latest
    profiles: [debug]
    ports:
      - "127.0.0.1:8025:8025"
    deploy:
      resources:
        limits:
          cpus: '0.25'
          memory: 128M
    restart: unless-stopped
    networks:
      - backend

volumes:
  pgadmin-data:

networks:
  backend:
```

Activate with: `docker compose --profile debug up`

---

## GPU-Enabled Service (Machine Learning)

```yaml
services:
  ml-worker:
    build:
      context: .
      dockerfile: Dockerfile.gpu
    deploy:
      resources:
        limits:
          cpus: '4.0'
          memory: 8G
        reservations:
          cpus: '2.0'
          memory: 4G
          devices:
            - capabilities: [gpu]
              driver: nvidia
              count: 1
    environment:
      NVIDIA_VISIBLE_DEVICES: all
      CUDA_VISIBLE_DEVICES: "0"
    volumes:
      - model-data:/app/models
    restart: unless-stopped
    networks:
      - backend

volumes:
  model-data:

networks:
  backend:
```

---

## Full-Stack Composition

Complete example combining web, app, database, cache, worker, and migration services.

```yaml
name: myapp

services:
  nginx:
    image: nginx:1.25-alpine
    ports:
      - "127.0.0.1:80:80"
    volumes:
      - ./nginx/conf.d:/etc/nginx/conf.d:ro
    healthcheck:
      test: ["CMD-SHELL", "curl -f http://localhost/health || exit 1"]
      interval: 30s
      timeout: 5s
      retries: 3
    deploy:
      resources:
        limits:
          memory: 256M
    restart: unless-stopped
    depends_on:
      app:
        condition: service_healthy
    networks:
      - frontend

  app:
    build:
      context: .
      target: production
    healthcheck:
      test: ["CMD-SHELL", "curl -f http://localhost:3000/health || exit 1"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 30s
    deploy:
      replicas: 2
      resources:
        limits:
          cpus: '1.0'
          memory: 512M
    restart: unless-stopped
    init: true
    env_file: [.env]
    depends_on:
      db:
        condition: service_healthy
      redis:
        condition: service_healthy
      migration:
        condition: service_completed_successfully
    networks:
      - frontend
      - backend

  worker:
    build:
      context: .
      target: production
    command: ["celery", "-A", "app", "worker", "-l", "info"]
    deploy:
      replicas: 2
      resources:
        limits:
          memory: 512M
    restart: unless-stopped
    init: true
    env_file: [.env]
    depends_on:
      db:
        condition: service_healthy
      redis:
        condition: service_healthy
    networks:
      - backend

  migration:
    build:
      context: .
      target: production
    command: ["python", "manage.py", "migrate", "--noinput"]
    env_file: [.env]
    depends_on:
      db:
        condition: service_healthy
    restart: "no"
    networks:
      - backend

  db:
    image: postgres:16-alpine
    volumes:
      - db-data:/var/lib/postgresql/data
    env_file: [.env]
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 30s
    deploy:
      resources:
        limits:
          memory: 1G
    restart: unless-stopped
    shm_size: 256M
    networks:
      - backend

  redis:
    image: redis:7-alpine
    command: ["redis-server", "--maxmemory", "256mb", "--maxmemory-policy", "allkeys-lru"]
    volumes:
      - redis-data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 3
    deploy:
      resources:
        limits:
          memory: 300M
    restart: unless-stopped
    networks:
      - backend

volumes:
  db-data:
  redis-data:

networks:
  frontend:
  backend:
```

---

## Official Sources

- https://docs.docker.com/compose/compose-file/05-services/
- https://docs.docker.com/compose/compose-file/build/
- https://docs.docker.com/compose/compose-file/deploy/
