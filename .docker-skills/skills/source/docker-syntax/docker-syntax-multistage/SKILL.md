---
name: docker-syntax-multistage
description: >
  Use when optimizing Docker image size, separating build-time and runtime
  dependencies, or creating minimal production images.
  Prevents shipping compilers, build tools, and source code in production
  images by failing to use COPY --from to extract only final artifacts.
  Covers FROM AS stage naming, COPY --from, parallel stages, --target for
  partial builds, builder pattern, test stages, and shared dependency stages.
  Keywords: FROM AS, COPY --from, --target, docker build --target,
  multi-stage, builder pattern, distroless, alpine, scratch,
  reduce image size, Docker image too big, smaller container,
  separate build and runtime.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+ with BuildKit."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-syntax-multistage

## Quick Reference

### Multi-Stage Build Concept

A multi-stage build uses multiple `FROM` instructions in a single Dockerfile. Each `FROM` starts a new stage. Only the final stage (or the `--target` stage) produces the output image. Earlier stages exist solely to generate artifacts that are copied into later stages.

### Stage Types

| Stage Type | Purpose | Final Image? |
|-----------|---------|-------------|
| Builder | Compile code, run bundlers, generate artifacts | No |
| Test | Run test suites, linting, static analysis | No |
| Dependencies | Install and cache shared dependencies | No |
| Production | Minimal runtime with only required artifacts | Yes |
| Debug | Production + debugging tools | Yes (dev only) |

### Image Size Impact

| Language | Without Multi-Stage | With Multi-Stage | Reduction |
|----------|-------------------|-----------------|-----------|
| Go | ~800 MB (golang:1.22) | ~0 MB (scratch) or ~7 MB (alpine) | 99% |
| Node.js | ~1.1 GB (node:20) | ~180 MB (node:20-slim) | 83% |
| Python | ~1.0 GB (python:3.12) | ~150 MB (python:3.12-slim) | 85% |
| Java | ~700 MB (eclipse-temurin:21-jdk) | ~220 MB (eclipse-temurin:21-jre) | 69% |
| Rust | ~1.4 GB (rust:1.77) | ~0 MB (scratch) or ~7 MB (alpine) | 99% |
| .NET | ~900 MB (mcr.microsoft.com/dotnet/sdk:8.0) | ~220 MB (mcr.microsoft.com/dotnet/aspnet:8.0) | 76% |

### Final Stage Base Image Selection

| Base Image | Size | Use When |
|-----------|------|----------|
| `scratch` | 0 MB | Statically compiled binaries (Go, Rust with musl) |
| `alpine:3.19` | ~7 MB | Need a shell and minimal OS utilities |
| `gcr.io/distroless/static-debian12` | ~2 MB | Static binaries without shell access |
| `gcr.io/distroless/base-debian12` | ~20 MB | Binaries needing glibc but no shell |
| `*-slim` variants | 30-80 MB | Need package manager for runtime deps |

### Critical Warnings

**ALWAYS** use multi-stage builds for production images. Single-stage builds ship compilers, source code, and build tools to production -- a security risk and a waste of space.

**ALWAYS** name stages with `AS <name>`. NEVER reference stages by numeric index (`--from=0`) because indexes break when stages are added or removed.

**NEVER** install build tools (gcc, make, npm devDependencies) in the final production stage. Build tools belong in the builder stage only.

**NEVER** copy the entire build stage filesystem into the production stage. ALWAYS copy only the specific artifacts needed (binaries, compiled assets, config files).

**ALWAYS** include `# syntax=docker/dockerfile:1` as the first line to enable BuildKit features including parallel stage execution.

---

## Core Syntax

### FROM ... AS (Stage Naming)

```dockerfile
FROM <image>[:<tag>] AS <stage-name>
```

Every stage MUST have a descriptive name. Names are case-insensitive but ALWAYS use lowercase by convention.

```dockerfile
FROM golang:1.22 AS build
FROM alpine:3.19 AS runtime
FROM build AS test
```

### COPY --from (Artifact Extraction)

```dockerfile
# Copy from a named stage
COPY --from=<stage-name> <src> <dest>

# Copy from an external image (auto-pulled)
COPY --from=<image>:<tag> <src> <dest>
```

Examples:

```dockerfile
# From a build stage
COPY --from=build /app/binary /usr/local/bin/app

# From an external image
COPY --from=nginx:1.25 /etc/nginx/nginx.conf /etc/nginx/nginx.conf

# Multiple artifacts from different stages
COPY --from=build-backend /app/server /usr/local/bin/server
COPY --from=build-frontend /app/dist /var/www/html
```

### --target (Partial Builds)

Build only a specific stage and its dependencies:

```bash
# Build only the test stage
docker build --target test -t myapp:test .

# Build only the debug stage
docker build --target debug -t myapp:debug .

# Build the default (last) stage
docker build -t myapp:latest .
```

BuildKit optimization: when using `--target`, BuildKit ONLY builds the target stage and stages it depends on. Unrelated stages are skipped entirely.

---

## Decision Tree

### When to Use Multi-Stage

```
Need to build/compile code in the container?
  YES --> Use multi-stage (builder + runtime)
  NO  --> Does the image include dev dependencies or build tools?
    YES --> Use multi-stage to separate them
    NO  --> Single stage MAY be acceptable for simple runtime images

Is the final image > 500 MB?
  YES --> Multi-stage with a minimal base will likely reduce it significantly
  NO  --> Still consider multi-stage for security (no build tools in production)
```

### Choosing the Final Stage Base

```
Is the binary statically compiled? (Go with CGO_ENABLED=0, Rust with musl)
  YES --> Use `scratch` or `gcr.io/distroless/static-debian12`
  NO  --> Does it need glibc?
    YES --> Does it need a shell for debugging?
      YES --> Use `alpine` or `*-slim` variant
      NO  --> Use `gcr.io/distroless/base-debian12`
    NO  --> Use `alpine`

Does the runtime need a language runtime? (Node, Python, Java, .NET)
  YES --> Use the slim/JRE variant of that runtime
  NO  --> Use alpine or distroless
```

---

## Patterns

### 1. Basic Builder Pattern

The foundation of all multi-stage builds. Build in a full SDK image, run in a minimal image.

```dockerfile
# syntax=docker/dockerfile:1

FROM golang:1.22 AS build
WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 go build -o /app/server ./cmd/server

FROM alpine:3.19 AS production
RUN addgroup -S app && adduser -S app -G app
COPY --from=build /app/server /usr/local/bin/server
USER app
ENTRYPOINT ["/usr/local/bin/server"]
```

### 2. Build + Test + Production Pipeline

Run tests as a build gate. The production stage only depends on the build stage, so test failures do not affect the production image -- but CI pipelines MUST target the test stage first.

```dockerfile
# syntax=docker/dockerfile:1

FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN go build -o /app/server ./cmd/server

FROM build AS test
RUN go test -v -race ./...

FROM alpine:3.19 AS production
COPY --from=build /app/server /usr/local/bin/server
USER nobody:nobody
ENTRYPOINT ["/usr/local/bin/server"]
```

CI usage -- ALWAYS build test target before production:

```bash
docker build --target test .
docker build --target production -t myapp:latest .
```

### 3. Shared Dependency Stage

When multiple stages need the same dependencies, create a shared base to avoid duplication.

```dockerfile
# syntax=docker/dockerfile:1

FROM node:20-slim AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --ignore-scripts

FROM deps AS build
COPY . .
RUN npm run build

FROM deps AS test
COPY . .
RUN npm run test

FROM node:20-slim AS production
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY --from=build /app/dist ./dist
COPY package.json ./
USER node
CMD ["node", "dist/index.js"]
```

### 4. Parallel Build Stages

BuildKit automatically parallelizes independent stages. Design your Dockerfile to maximize parallelism.

```dockerfile
# syntax=docker/dockerfile:1

FROM node:20-slim AS frontend-build
WORKDIR /frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ .
RUN npm run build

FROM golang:1.22 AS backend-build
WORKDIR /backend
COPY backend/go.mod backend/go.sum ./
RUN go mod download
COPY backend/ .
RUN CGO_ENABLED=0 go build -o /server ./cmd/server

FROM alpine:3.19 AS production
COPY --from=backend-build /server /usr/local/bin/server
COPY --from=frontend-build /frontend/dist /var/www/html
USER nobody:nobody
ENTRYPOINT ["/usr/local/bin/server"]
```

`frontend-build` and `backend-build` execute simultaneously because neither depends on the other.

### 5. Debug Stage Pattern

Add debugging tools without polluting the production image:

```dockerfile
# syntax=docker/dockerfile:1

FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN CGO_ENABLED=0 go build -o /app/server ./cmd/server

FROM alpine:3.19 AS production
COPY --from=build /app/server /usr/local/bin/server
USER nobody:nobody
ENTRYPOINT ["/usr/local/bin/server"]

FROM production AS debug
USER root
RUN apk add --no-cache curl strace busybox-extras
USER nobody:nobody
```

```bash
# Production: lean
docker build -t myapp:latest .

# Debug: with tools
docker build --target debug -t myapp:debug .
```

### 6. Cross-Platform Build Pattern

Use BuildKit platform ARGs for multi-architecture builds:

```dockerfile
# syntax=docker/dockerfile:1

FROM --platform=$BUILDPLATFORM golang:1.22 AS build
ARG TARGETOS TARGETARCH
WORKDIR /src
COPY . .
RUN GOOS=$TARGETOS GOARCH=$TARGETARCH CGO_ENABLED=0 \
    go build -o /app/server ./cmd/server

FROM alpine:3.19
COPY --from=build /app/server /usr/local/bin/server
ENTRYPOINT ["/usr/local/bin/server"]
```

```bash
docker buildx build --platform linux/amd64,linux/arm64 -t myapp:latest .
```

---

## Reference Links

- [references/patterns.md](references/patterns.md) -- Complete multi-stage patterns with full Dockerfiles
- [references/examples.md](references/examples.md) -- Language-specific multi-stage examples (Node, Python, Go, Java, Rust, .NET)
- [references/anti-patterns.md](references/anti-patterns.md) -- Multi-stage mistakes and how to fix them

### Official Sources

- https://docs.docker.com/build/building/multi-stage/
- https://docs.docker.com/reference/dockerfile/
- https://docs.docker.com/build/building/best-practices/
- https://docs.docker.com/build/buildkit/
