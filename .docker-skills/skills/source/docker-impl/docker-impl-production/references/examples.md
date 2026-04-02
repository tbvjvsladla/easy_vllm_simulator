# Production Examples

## Go (Static Binary)

```dockerfile
# syntax=docker/dockerfile:1

FROM golang:1.22-bookworm AS build

ARG VERSION=dev
WORKDIR /src

# Cache dependencies
COPY go.mod go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod \
    go mod download

# Build static binary
COPY . .
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=0 go build \
    -ldflags="-s -w -X main.version=${VERSION}" \
    -o /app ./cmd/server

FROM scratch

# Copy CA certs and timezone data
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=build /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=build /etc/passwd /etc/passwd

COPY --from=build /app /app

LABEL org.opencontainers.image.title="Go App" \
      org.opencontainers.image.version="${VERSION}"

USER 65534:65534

ENTRYPOINT ["/app"]
```

---

## Python (Django / Flask / FastAPI)

```dockerfile
# syntax=docker/dockerfile:1

FROM python:3.12-bookworm AS build

WORKDIR /app

# Install build dependencies
COPY requirements.txt .
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install --prefix=/install -r requirements.txt

FROM python:3.12-slim-bookworm@sha256:<pin-digest-here>

# OCI metadata
LABEL org.opencontainers.image.title="Python App"

# Non-root user
RUN groupadd -r -g 1001 appuser && \
    useradd --no-log-init -r -u 1001 -g appuser -d /app appuser

WORKDIR /app

# Copy installed packages
COPY --from=build /install /usr/local

# Copy application
COPY --chown=1001:1001 . .

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=15s --retries=3 \
  CMD python -c "import urllib.request; urllib.request.urlopen('http://localhost:8000/health')" || exit 1

USER 1001:1001

EXPOSE 8000

ENTRYPOINT ["python", "-m", "gunicorn"]
CMD ["app:application", "--bind", "0.0.0.0:8000", "--workers", "4"]
```

---

## Node.js (Express / NestJS / Fastify)

```dockerfile
# syntax=docker/dockerfile:1

FROM node:20-bookworm AS build

WORKDIR /app

# Install dependencies (production + dev for build)
COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci

# Build application
COPY . .
RUN npm run build

# Prune dev dependencies
RUN npm prune --production

FROM node:20-bookworm-slim@sha256:<pin-digest-here>

LABEL org.opencontainers.image.title="Node.js App"

# Non-root user (node user UID 1000 exists in official images)
# Use it or create a custom one
RUN groupadd -r -g 1001 appuser && \
    useradd --no-log-init -r -u 1001 -g appuser appuser

WORKDIR /app

COPY --chown=1001:1001 --from=build /app/dist ./dist
COPY --chown=1001:1001 --from=build /app/node_modules ./node_modules
COPY --chown=1001:1001 --from=build /app/package.json ./

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:3000/health || exit 1

USER 1001:1001

EXPOSE 3000

# Use --init for proper signal handling with Node.js
ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["node", "dist/index.js"]
```

---

## Rust (Static Binary with musl)

```dockerfile
# syntax=docker/dockerfile:1

FROM rust:1.77-bookworm AS build

# Install musl target for static linking
RUN rustup target add x86_64-unknown-linux-musl
RUN apt-get update && apt-get install -y --no-install-recommends \
    musl-tools \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

# Cache dependencies via cargo-chef pattern
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --target x86_64-unknown-linux-musl

# Build real application
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --target x86_64-unknown-linux-musl && \
    cp target/x86_64-unknown-linux-musl/release/myapp /app

FROM scratch

COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=build /app /app

LABEL org.opencontainers.image.title="Rust App"

USER 65534:65534

ENTRYPOINT ["/app"]
```

---

## Java (Spring Boot / Quarkus)

```dockerfile
# syntax=docker/dockerfile:1

FROM eclipse-temurin:21-jdk-bookworm AS build

WORKDIR /src

# Cache Gradle/Maven dependencies
COPY gradle/ gradle/
COPY gradlew build.gradle.kts settings.gradle.kts ./
RUN --mount=type=cache,target=/root/.gradle \
    ./gradlew dependencies --no-daemon

COPY . .
RUN --mount=type=cache,target=/root/.gradle \
    ./gradlew bootJar --no-daemon

# Extract Spring Boot layered JAR for optimal Docker caching
RUN java -Djarmode=tools -jar build/libs/*.jar extract --destination /extracted

FROM eclipse-temurin:21-jre-alpine@sha256:<pin-digest-here>

LABEL org.opencontainers.image.title="Java App"

RUN addgroup -S -g 1001 appuser && adduser -S -u 1001 -G appuser appuser

WORKDIR /app

# Copy extracted layers (most stable first for cache efficiency)
COPY --from=build /extracted/dependencies/ ./
COPY --from=build /extracted/spring-boot-loader/ ./
COPY --from=build /extracted/snapshot-dependencies/ ./
COPY --from=build /extracted/application/ ./

HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:8080/actuator/health || exit 1

USER 1001:1001

EXPOSE 8080

ENTRYPOINT ["java", "-XX:+UseContainerSupport", "-XX:MaxRAMPercentage=75.0", "org.springframework.boot.loader.launch.JarLauncher"]
```

ALWAYS use `-XX:+UseContainerSupport` (default since JDK 10+) so the JVM respects container memory limits.
ALWAYS set `-XX:MaxRAMPercentage` instead of `-Xmx` for container-aware memory management.

---

## .NET

```dockerfile
# syntax=docker/dockerfile:1

FROM mcr.microsoft.com/dotnet/sdk:8.0 AS build

WORKDIR /src

# Restore dependencies (cached)
COPY *.csproj ./
RUN --mount=type=cache,target=/root/.nuget/packages \
    dotnet restore

# Build and publish
COPY . .
RUN dotnet publish -c Release -o /app --no-restore

FROM mcr.microsoft.com/dotnet/aspnet:8.0-alpine@sha256:<pin-digest-here>

LABEL org.opencontainers.image.title=".NET App"

RUN addgroup -S -g 1001 appuser && adduser -S -u 1001 -G appuser appuser

WORKDIR /app
COPY --from=build /app .

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:5000/health || exit 1

USER 1001:1001

EXPOSE 5000

ENTRYPOINT ["dotnet", "MyApp.dll"]
```

---

## Entrypoint Script Examples

### Generic Entrypoint with Migrations

```bash
#!/bin/sh
set -e

echo "Starting application..."

# Run database migrations if the command is the main app
if [ "$1" = 'serve' ] || [ "$1" = 'node' ] || [ "$1" = 'python' ]; then
    echo "Running database migrations..."
    /usr/bin/app migrate --apply 2>&1
    echo "Migrations complete."
fi

# Replace shell with the application process
exec "$@"
```

### Entrypoint with Environment Validation

```bash
#!/bin/sh
set -e

# Validate required environment variables
required_vars="DATABASE_URL SECRET_KEY"
for var in $required_vars; do
    eval value=\$$var
    if [ -z "$value" ]; then
        echo "ERROR: Required environment variable $var is not set" >&2
        exit 1
    fi
done

exec "$@"
```

### Entrypoint with Wait-for-Dependencies

```bash
#!/bin/sh
set -e

# Wait for PostgreSQL
if [ -n "$DB_HOST" ]; then
    echo "Waiting for PostgreSQL at $DB_HOST:${DB_PORT:-5432}..."
    timeout=30
    elapsed=0
    until nc -z "$DB_HOST" "${DB_PORT:-5432}" 2>/dev/null; do
        elapsed=$((elapsed + 1))
        if [ "$elapsed" -ge "$timeout" ]; then
            echo "ERROR: Timed out waiting for database" >&2
            exit 1
        fi
        sleep 1
    done
    echo "Database is ready."
fi

# Wait for Redis
if [ -n "$REDIS_HOST" ]; then
    echo "Waiting for Redis at $REDIS_HOST:${REDIS_PORT:-6379}..."
    timeout=15
    elapsed=0
    until nc -z "$REDIS_HOST" "${REDIS_PORT:-6379}" 2>/dev/null; do
        elapsed=$((elapsed + 1))
        if [ "$elapsed" -ge "$timeout" ]; then
            echo "ERROR: Timed out waiting for Redis" >&2
            exit 1
        fi
        sleep 1
    done
    echo "Redis is ready."
fi

exec "$@"
```

### Entrypoint with Config File Generation

```bash
#!/bin/sh
set -e

# Generate config from environment variables
cat > /app/config.json <<CONF
{
  "database_url": "${DATABASE_URL}",
  "port": ${PORT:-8080},
  "log_level": "${LOG_LEVEL:-info}",
  "cors_origins": "${CORS_ORIGINS:-*}"
}
CONF

exec "$@"
```

---

## Health Check Patterns by Technology

### PostgreSQL

```dockerfile
HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=5 \
  CMD pg_isready -U postgres || exit 1
```

### Redis

```dockerfile
HEALTHCHECK --interval=10s --timeout=3s --retries=3 \
  CMD redis-cli ping | grep -q PONG || exit 1
```

### Nginx

```dockerfile
HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost/ || exit 1
```

### gRPC Service

```dockerfile
# Requires grpc-health-probe binary
COPY --from=grpc-health-probe /bin/grpc_health_probe /bin/grpc_health_probe
HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD ["/bin/grpc_health_probe", "-addr=:50051"]
```

### Worker Process (No HTTP)

```dockerfile
HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
  CMD test -f /tmp/worker-healthy && \
      test $(($(date +%s) - $(stat -c %Y /tmp/worker-healthy))) -lt 60 || exit 1
```

The worker writes to `/tmp/worker-healthy` on each successful processing cycle. The health check verifies the file exists and was updated within the last 60 seconds.
