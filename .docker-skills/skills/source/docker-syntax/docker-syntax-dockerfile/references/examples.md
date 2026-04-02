# Dockerfile Examples -- Production-Ready Templates

> All examples verified against Docker Engine 24+ with BuildKit.
> ALWAYS start with `# syntax=docker/dockerfile:1`.

---

## Go Application (Multi-Stage, Multi-Platform)

```dockerfile
# syntax=docker/dockerfile:1

FROM --platform=$BUILDPLATFORM golang:1.22-alpine AS build

ARG TARGETOS TARGETARCH
ARG VERSION=dev

WORKDIR /src

# Cache dependency download separately from build
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=bind,source=go.sum,target=go.sum \
    --mount=type=bind,source=go.mod,target=go.mod \
    go mod download

# Build with cached artifacts
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    --mount=type=bind,target=. \
    GOOS=$TARGETOS GOARCH=$TARGETARCH go build \
    -ldflags "-X main.version=$VERSION" \
    -o /bin/app ./cmd

FROM alpine:3.19 AS runtime

RUN addgroup -S appgroup && adduser -S appuser -G appgroup

COPY --from=build /bin/app /usr/bin/app

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

USER appuser:appgroup
EXPOSE 8080

ENTRYPOINT ["/usr/bin/app"]
CMD ["--config", "/etc/app/config.yaml"]
```

---

## Node.js Application

```dockerfile
# syntax=docker/dockerfile:1

FROM node:20-bookworm-slim AS deps

WORKDIR /app

COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci --production=false

FROM deps AS build

COPY . .
RUN npm run build

FROM node:20-bookworm-slim AS production

RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser

WORKDIR /app

COPY --from=deps /app/node_modules ./node_modules
COPY --from=build /app/dist ./dist
COPY package.json ./

ENV NODE_ENV=production

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD node -e "require('http').get('http://localhost:3000/health', (r) => { process.exit(r.statusCode === 200 ? 0 : 1) })"

USER appuser:appuser
EXPOSE 3000

ENTRYPOINT ["node"]
CMD ["dist/index.js"]
```

---

## Python Application

```dockerfile
# syntax=docker/dockerfile:1

FROM python:3.12-slim-bookworm AS base

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1

WORKDIR /app

FROM base AS deps

COPY requirements.txt .
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install --no-compile -r requirements.txt

FROM base AS production

RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser

COPY --from=deps /usr/local/lib/python3.12/site-packages /usr/local/lib/python3.12/site-packages
COPY --from=deps /usr/local/bin /usr/local/bin
COPY . .

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD python -c "import urllib.request; urllib.request.urlopen('http://localhost:8000/health')" || exit 1

USER appuser:appuser
EXPOSE 8000

ENTRYPOINT ["python"]
CMD ["-m", "uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]
```

---

## Rust Application

```dockerfile
# syntax=docker/dockerfile:1

FROM rust:1.77-bookworm AS build

WORKDIR /app

# Cache dependencies by building a dummy project first
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release

# Build actual application
COPY src ./src
RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release && \
    cp target/release/myapp /usr/local/bin/

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser

COPY --from=build /usr/local/bin/myapp /usr/local/bin/myapp

USER appuser:appuser
EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/myapp"]
```

---

## .NET Application

```dockerfile
# syntax=docker/dockerfile:1

FROM mcr.microsoft.com/dotnet/sdk:8.0 AS build

WORKDIR /src

COPY *.csproj ./
RUN --mount=type=cache,target=/root/.nuget/packages \
    dotnet restore

COPY . .
RUN dotnet publish -c Release -o /app/publish --no-restore

FROM mcr.microsoft.com/dotnet/aspnet:8.0 AS runtime

RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser

WORKDIR /app
COPY --from=build /app/publish .

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD curl -f http://localhost:8080/health || exit 1

USER appuser:appuser
EXPOSE 8080

ENTRYPOINT ["dotnet", "MyApp.dll"]
```

---

## Nginx Static Site

```dockerfile
# syntax=docker/dockerfile:1

FROM node:20-bookworm-slim AS build

WORKDIR /app
COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci
COPY . .
RUN npm run build

FROM nginx:1.25-alpine AS production

# Remove default config
RUN rm /etc/nginx/conf.d/default.conf

COPY --from=build /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/app.conf

HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost/ || exit 1

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
```

---

## Multi-Stage with Tests

```dockerfile
# syntax=docker/dockerfile:1

FROM golang:1.22 AS build

WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN go build -o /bin/app ./cmd

FROM build AS test

RUN go test -v -race ./...

FROM alpine:3.19 AS production

RUN addgroup -S appgroup && adduser -S appuser -G appgroup

COPY --from=build /bin/app /usr/bin/app

USER appuser:appgroup
EXPOSE 8080

ENTRYPOINT ["/usr/bin/app"]
```

Build just the test stage: `docker build --target test .`
Build production: `docker build --target production .`

---

## Entrypoint Script Pattern

```dockerfile
# syntax=docker/dockerfile:1

FROM postgres:16-bookworm

COPY --chmod=755 docker-entrypoint-initdb.d/ /docker-entrypoint-initdb.d/
COPY --chmod=755 docker-entrypoint.sh /usr/local/bin/

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["postgres"]
```

**docker-entrypoint.sh:**
```bash
#!/bin/bash
set -e

# Run initialization tasks
if [ "$1" = 'postgres' ]; then
    echo "Initializing database..."
    chown -R postgres "$PGDATA"
fi

# ALWAYS end with exec "$@" to replace shell with the CMD arguments
# This makes the application PID 1 for proper signal handling
exec "$@"
```

---

## Secret and SSH Usage

```dockerfile
# syntax=docker/dockerfile:1

FROM node:20-bookworm-slim AS build

WORKDIR /app

# Clone private repository using SSH
RUN --mount=type=ssh \
    git clone git@github.com:company/private-lib.git /app/lib

# Use API token without baking into layer
RUN --mount=type=secret,id=npm_token,env=NPM_TOKEN \
    echo "//registry.npmjs.org/:_authToken=${NPM_TOKEN}" > .npmrc && \
    npm ci && \
    rm .npmrc

COPY . .
RUN npm run build

FROM node:20-bookworm-slim AS production

WORKDIR /app
COPY --from=build /app/dist ./dist
COPY --from=build /app/node_modules ./node_modules

USER node
EXPOSE 3000
CMD ["node", "dist/index.js"]
```

Build command:
```bash
docker build \
  --ssh default \
  --secret id=npm_token,src=$HOME/.npmrc_token \
  -t myapp:latest .
```

---

## Minimal Image from Scratch

```dockerfile
# syntax=docker/dockerfile:1

FROM golang:1.22 AS build

WORKDIR /src
COPY . .

# Static binary with no CGO dependencies
RUN CGO_ENABLED=0 go build -ldflags="-s -w" -o /bin/app ./cmd

FROM scratch

# Copy CA certificates for HTTPS
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

COPY --from=build /bin/app /bin/app

USER 65534:65534
EXPOSE 8080

ENTRYPOINT ["/bin/app"]
```

The `scratch` image has zero bytes -- no shell, no package manager, no OS. ONLY use with statically compiled binaries.

---

## PHP/Composer with Cache Mounts

```dockerfile
# syntax=docker/dockerfile:1

FROM composer:2 AS deps

WORKDIR /app
COPY composer.json composer.lock ./
RUN --mount=type=cache,target=/tmp/cache \
    composer install --no-dev --no-scripts --no-autoloader

FROM php:8.3-fpm-bookworm AS production

RUN apt-get update && apt-get install -y --no-install-recommends \
    libpq-dev \
    && docker-php-ext-install pdo_pgsql opcache \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=deps /app/vendor ./vendor
COPY . .
RUN composer dump-autoload --optimize --no-dev

USER www-data
EXPOSE 9000

CMD ["php-fpm"]
```
