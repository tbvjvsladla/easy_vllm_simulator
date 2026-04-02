# Multi-Stage Build Patterns

> Complete Dockerfiles for every major multi-stage pattern.
> All patterns verified against Docker Engine 24+ with BuildKit.

---

## 1. Minimal Builder Pattern (Scratch Final)

The smallest possible production image. ONLY works with statically compiled binaries.

```dockerfile
# syntax=docker/dockerfile:1

FROM golang:1.22-alpine AS build
WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-s -w" -o /app/server ./cmd/server

FROM scratch AS production
COPY --from=build /app/server /server
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
ENTRYPOINT ["/server"]
```

Key points:
- `CGO_ENABLED=0` produces a fully static binary (no glibc dependency).
- `-ldflags="-s -w"` strips debug info and symbol tables, reducing binary size by ~30%.
- `scratch` has NO shell, NO filesystem, NO user database -- the binary must be completely self-contained.
- ALWAYS copy CA certificates if the binary makes HTTPS requests.
- ALWAYS copy timezone data (`/usr/share/zoneinfo`) if the binary uses `time.LoadLocation`.

---

## 2. Distroless Final Stage

More secure than alpine (no shell for attackers to use) but provides glibc and CA certs.

```dockerfile
# syntax=docker/dockerfile:1

FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN CGO_ENABLED=0 go build -o /app/server ./cmd/server

FROM gcr.io/distroless/static-debian12:nonroot AS production
COPY --from=build /app/server /server
ENTRYPOINT ["/server"]
```

Distroless variants:

| Image | Size | Includes |
|-------|------|----------|
| `gcr.io/distroless/static-debian12` | ~2 MB | CA certs, tzdata, `/etc/passwd` |
| `gcr.io/distroless/base-debian12` | ~20 MB | Above + glibc |
| `gcr.io/distroless/cc-debian12` | ~25 MB | Above + libgcc |
| `gcr.io/distroless/java21-debian12` | ~220 MB | Above + OpenJDK 21 JRE |
| `gcr.io/distroless/nodejs22-debian12` | ~130 MB | Above + Node.js 22 |
| `gcr.io/distroless/python3-debian12` | ~50 MB | Above + Python 3 |

ALWAYS use the `:nonroot` tag variant to run as UID 65534 without explicit `USER` instruction.

---

## 3. Build + Test + Lint Pipeline

Three-stage pipeline where each stage serves a CI/CD purpose.

```dockerfile
# syntax=docker/dockerfile:1

# ---- Stage 1: Build ----
FROM golang:1.22 AS build
WORKDIR /src

# Cache dependencies separately
COPY go.mod go.sum ./
RUN go mod download

COPY . .
RUN CGO_ENABLED=0 go build -o /app/server ./cmd/server

# ---- Stage 2: Lint ----
FROM golangci/golangci-lint:v1.57 AS lint
WORKDIR /src
COPY --from=build /src /src
RUN golangci-lint run ./...

# ---- Stage 3: Test ----
FROM build AS test
RUN go test -v -race -coverprofile=/coverage.out ./...

# ---- Stage 4: Production ----
FROM alpine:3.19 AS production
RUN addgroup -S app && adduser -S app -G app
COPY --from=build /app/server /usr/local/bin/server
USER app
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1
ENTRYPOINT ["/usr/local/bin/server"]
```

CI pipeline usage:

```bash
# Step 1: Run lint (fails fast)
docker build --target lint .

# Step 2: Run tests
docker build --target test .

# Step 3: Build production image
docker build --target production -t myapp:latest .
```

BuildKit parallelism: `lint` and `test` stages run in parallel because both depend only on `build`, not on each other.

---

## 4. Shared Dependency Base Pattern

When multiple stages need identical dependencies, create a shared base stage. This avoids downloading and installing packages multiple times.

```dockerfile
# syntax=docker/dockerfile:1

# ---- Shared base with system dependencies ----
FROM python:3.12-slim AS base
RUN apt-get update && apt-get install -y --no-install-recommends \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# ---- Dependencies stage ----
FROM base AS deps
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# ---- Test stage ----
FROM deps AS test
COPY requirements-dev.txt .
RUN pip install --no-cache-dir -r requirements-dev.txt
COPY . .
RUN pytest --tb=short

# ---- Production stage ----
FROM base AS production
COPY --from=deps /usr/local/lib/python3.12/site-packages /usr/local/lib/python3.12/site-packages
COPY --from=deps /usr/local/bin /usr/local/bin
COPY . .
RUN groupadd -r app && useradd -r -g app app
USER app
CMD ["gunicorn", "app:create_app()", "--bind", "0.0.0.0:8000"]
```

Stage dependency graph:
```
base --> deps --> test
  |        |
  +--------+--> production
```

---

## 5. Parallel Frontend + Backend Pattern

Build independent components simultaneously. BuildKit detects that `frontend` and `backend` have no dependency relationship and runs them in parallel.

```dockerfile
# syntax=docker/dockerfile:1

# ---- Frontend Build (runs in parallel with backend) ----
FROM node:20-slim AS frontend
WORKDIR /frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ .
RUN npm run build

# ---- Backend Build (runs in parallel with frontend) ----
FROM golang:1.22-alpine AS backend
WORKDIR /backend
COPY backend/go.mod backend/go.sum ./
RUN go mod download
COPY backend/ .
RUN CGO_ENABLED=0 go build -o /server ./cmd/server

# ---- Production (depends on both, waits for both to finish) ----
FROM alpine:3.19 AS production
RUN addgroup -S app && adduser -S app -G app

COPY --from=backend /server /usr/local/bin/server
COPY --from=frontend /frontend/dist /var/www/html

USER app
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/server"]
```

---

## 6. Multi-Architecture Build Pattern

Use BuildKit platform ARGs for cross-compilation. The build stage runs on the BUILD platform for speed, while the output targets the specified platform.

```dockerfile
# syntax=docker/dockerfile:1

# Build on host architecture for speed
FROM --platform=$BUILDPLATFORM golang:1.22-alpine AS build

# Target architecture variables (auto-set by BuildKit)
ARG TARGETOS TARGETARCH

WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY . .

# Cross-compile for the target platform
RUN CGO_ENABLED=0 GOOS=$TARGETOS GOARCH=$TARGETARCH \
    go build -ldflags="-s -w" -o /app/server ./cmd/server

# Runtime image uses the target platform automatically
FROM alpine:3.19
COPY --from=build /app/server /usr/local/bin/server
USER nobody:nobody
ENTRYPOINT ["/usr/local/bin/server"]
```

Build for multiple platforms:

```bash
docker buildx build \
  --platform linux/amd64,linux/arm64,linux/arm/v7 \
  -t myregistry/myapp:latest \
  --push .
```

Key: `--platform=$BUILDPLATFORM` on the build stage means compilation runs natively (fast), while cross-compiling for the target architecture. Without this flag, Docker would emulate the target architecture for the entire build (slow).

---

## 7. Cache-Optimized Builder Pattern

Maximize cache hits by separating dependency installation from code compilation.

```dockerfile
# syntax=docker/dockerfile:1

FROM node:20-slim AS deps
WORKDIR /app

# Step 1: Copy ONLY dependency manifests (changes rarely)
COPY package.json package-lock.json ./

# Step 2: Install dependencies (cached unless manifests change)
RUN npm ci

# ---- Build stage ----
FROM deps AS build
# Step 3: Copy source code (changes frequently)
COPY . .
# Step 4: Build (only reruns when source changes)
RUN npm run build

# ---- Production ----
FROM node:20-slim AS production
WORKDIR /app
ENV NODE_ENV=production

COPY --from=deps /app/node_modules ./node_modules
COPY --from=build /app/dist ./dist
COPY package.json ./

USER node
CMD ["node", "dist/index.js"]
```

Cache behavior:
- Change `package.json` --> Steps 2, 3, 4 all rerun.
- Change source code only --> Step 2 is cached, only steps 3 and 4 rerun.
- Change nothing --> Everything is cached.

---

## 8. COPY --from External Image Pattern

Pull files from published images without building them. Useful for including third-party tools.

```dockerfile
# syntax=docker/dockerfile:1

FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN go build -o /app/server ./cmd/server

FROM alpine:3.19 AS production

# Copy tools from external images
COPY --from=busybox:uclibc /bin/wget /usr/local/bin/wget
COPY --from=nginx:1.25-alpine /etc/nginx/nginx.conf /etc/nginx/default.conf

COPY --from=build /app/server /usr/local/bin/server
ENTRYPOINT ["/usr/local/bin/server"]
```

NEVER use `COPY --from` with `:latest` tag on external images. ALWAYS pin to a specific version for reproducibility.

---

## 9. Monorepo Multi-Service Pattern

Build multiple services from a single Dockerfile using `--target`.

```dockerfile
# syntax=docker/dockerfile:1

# ---- Shared dependency base ----
FROM golang:1.22 AS base
WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY . .

# ---- API Service ----
FROM base AS build-api
RUN CGO_ENABLED=0 go build -o /bin/api ./cmd/api

FROM alpine:3.19 AS api
COPY --from=build-api /bin/api /usr/local/bin/api
USER nobody:nobody
ENTRYPOINT ["/usr/local/bin/api"]

# ---- Worker Service ----
FROM base AS build-worker
RUN CGO_ENABLED=0 go build -o /bin/worker ./cmd/worker

FROM alpine:3.19 AS worker
COPY --from=build-worker /bin/worker /usr/local/bin/worker
USER nobody:nobody
ENTRYPOINT ["/usr/local/bin/worker"]

# ---- Migration Tool ----
FROM base AS build-migrate
RUN CGO_ENABLED=0 go build -o /bin/migrate ./cmd/migrate

FROM alpine:3.19 AS migrate
COPY --from=build-migrate /bin/migrate /usr/local/bin/migrate
USER nobody:nobody
ENTRYPOINT ["/usr/local/bin/migrate"]
```

Build each service independently:

```bash
docker build --target api -t myapp-api:latest .
docker build --target worker -t myapp-worker:latest .
docker build --target migrate -t myapp-migrate:latest .
```

---

## 10. Security-Hardened Production Pattern

Production image with all security best practices applied.

```dockerfile
# syntax=docker/dockerfile:1

FROM golang:1.22-alpine AS build
WORKDIR /src

# Use cache mounts for faster rebuilds
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=bind,source=go.sum,target=go.sum \
    --mount=type=bind,source=go.mod,target=go.mod \
    go mod download

RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    --mount=type=bind,target=. \
    CGO_ENABLED=0 go build -ldflags="-s -w" -o /app/server ./cmd/server

FROM alpine:3.19 AS production

# Remove unnecessary packages and caches
RUN apk --no-cache add ca-certificates tzdata \
    && rm -rf /var/cache/apk/*

# Create non-root user with explicit UID/GID
RUN addgroup -g 10001 -S app \
    && adduser -u 10001 -S app -G app -h /app -s /sbin/nologin

# Copy only the binary
COPY --from=build /app/server /usr/local/bin/server

# Make filesystem read-only friendly
RUN chmod 555 /usr/local/bin/server

# Metadata
LABEL org.opencontainers.image.title="My App" \
      org.opencontainers.image.source="https://github.com/org/repo" \
      org.opencontainers.image.licenses="MIT"

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

# Switch to non-root user
USER app:app
WORKDIR /app

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/server"]
```

Security checklist for multi-stage production images:
- Non-root user with explicit UID/GID
- No build tools or compilers in final image
- No source code in final image
- No package manager caches in final image
- CA certificates present for HTTPS
- Read-only filesystem compatible
- HEALTHCHECK defined
- OCI metadata labels
