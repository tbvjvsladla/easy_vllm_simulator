# Build Optimization Examples

> Reference file for docker-impl-build-optimization.
> All examples verified against Docker Engine 24+ with BuildKit.

---

## Before/After: Node.js Application

### Before (poor cache usage)

```dockerfile
FROM node:20
WORKDIR /app
COPY . .
RUN npm install
RUN npm run build
EXPOSE 3000
CMD ["node", "dist/index.js"]
```

**Problems:**
- `COPY . .` before `npm install` -- ANY file change triggers full dependency reinstall
- No `.dockerignore` -- sends `node_modules/`, `.git/`, test files to builder
- No cache mounts -- npm downloads everything from scratch on rebuild
- No multi-stage -- build tools and devDependencies remain in final image
- Using full `node:20` image (~1GB) for runtime

### After (optimized)

```dockerfile
# syntax=docker/dockerfile:1

# Build stage
FROM node:20-bookworm-slim AS build
WORKDIR /app

# Install dependencies first (changes less often than source code)
COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci --production=false

# Copy source and build (changes every commit)
COPY . .
RUN npm run build

# Runtime stage
FROM node:20-bookworm-slim AS runtime
WORKDIR /app

# Copy only production dependencies and built output
COPY --from=build /app/node_modules ./node_modules
COPY --from=build /app/dist ./dist
COPY package.json ./

USER node
EXPOSE 3000
CMD ["node", "dist/index.js"]
```

**Improvements:**
- Dependency files copied separately -- `npm ci` only reruns when `package.json` or lockfile changes
- Cache mount on `/root/.npm` -- npm reuses downloaded packages across builds
- Multi-stage build -- build tools excluded from final image
- Slim base image -- ~200MB smaller than full image
- Non-root user for security

---

## Before/After: Python Application

### Before

```dockerfile
FROM python:3.12
WORKDIR /app
COPY . .
RUN pip install -r requirements.txt
CMD ["python", "app.py"]
```

### After

```dockerfile
# syntax=docker/dockerfile:1

FROM python:3.12-slim AS runtime
WORKDIR /app

# Install dependencies first
COPY requirements.txt ./
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install --no-compile -r requirements.txt

# Copy application source
COPY . .

RUN groupadd -r appuser && useradd -r -g appuser appuser
USER appuser

CMD ["python", "app.py"]
```

---

## Before/After: Go Application

### Before

```dockerfile
FROM golang:1.22
WORKDIR /app
COPY . .
RUN go build -o server ./cmd/server
EXPOSE 8080
CMD ["./server"]
```

### After

```dockerfile
# syntax=docker/dockerfile:1

FROM --platform=$BUILDPLATFORM golang:1.22-alpine AS build

ARG TARGETOS TARGETARCH

WORKDIR /src

# Download dependencies (cached separately from source)
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=bind,source=go.sum,target=go.sum \
    --mount=type=bind,source=go.mod,target=go.mod \
    go mod download

# Build with bind mount (source not persisted in layers)
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    --mount=type=bind,target=. \
    GOOS=$TARGETOS GOARCH=$TARGETARCH CGO_ENABLED=0 \
    go build -ldflags="-s -w" -o /bin/server ./cmd/server

# Minimal runtime
FROM alpine:3.19 AS runtime
RUN addgroup -S app && adduser -S app -G app
COPY --from=build /bin/server /usr/bin/server
USER app
EXPOSE 8080
ENTRYPOINT ["/usr/bin/server"]
```

**Key techniques:**
- Bind mount for source -- no COPY layer, source not in any image layer
- Separate cache mounts for Go modules and build cache
- Cross-compilation with BUILDPLATFORM and TARGETARCH
- `CGO_ENABLED=0` for static binary -- can run on `scratch` or `alpine`
- `-ldflags="-s -w"` strips debug info, reducing binary size ~30%

---

## Before/After: Java (Maven) Application

### Before

```dockerfile
FROM maven:3.9-eclipse-temurin-21
WORKDIR /app
COPY . .
RUN mvn package
CMD ["java", "-jar", "target/app.jar"]
```

### After

```dockerfile
# syntax=docker/dockerfile:1

FROM maven:3.9-eclipse-temurin-21 AS build
WORKDIR /app

# Resolve dependencies first
COPY pom.xml ./
RUN --mount=type=cache,target=/root/.m2/repository \
    mvn dependency:resolve dependency:resolve-plugins

# Build application
COPY src ./src
RUN --mount=type=cache,target=/root/.m2/repository \
    mvn package -DskipTests -o

# Runtime with JRE only
FROM eclipse-temurin:21-jre-alpine AS runtime
WORKDIR /app
COPY --from=build /app/target/app.jar ./app.jar
RUN addgroup -S app && adduser -S app -G app
USER app
EXPOSE 8080
ENTRYPOINT ["java", "-jar", "app.jar"]
```

---

## Before/After: Rust Application

### Before

```dockerfile
FROM rust:1.77
WORKDIR /app
COPY . .
RUN cargo build --release
CMD ["./target/release/myapp"]
```

### After

```dockerfile
# syntax=docker/dockerfile:1

FROM rust:1.77-slim AS build
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release && \
    cp /app/target/release/myapp /usr/local/bin/myapp

FROM debian:bookworm-slim AS runtime
RUN groupadd -r app && useradd -r -g app app
COPY --from=build /usr/local/bin/myapp /usr/local/bin/myapp
USER app
ENTRYPOINT ["/usr/local/bin/myapp"]
```

---

## .dockerignore Pattern Examples

### Node.js Project

```
.git
.gitignore
node_modules
npm-debug.log*
dist
build
coverage
.nyc_output
.env
.env.*
*.md
LICENSE
.vscode
.idea
Dockerfile*
docker-compose*.yml
.dockerignore
tests/
__tests__/
*.test.js
*.spec.js
.eslintrc*
.prettierrc*
jest.config.*
```

### Python Project

```
.git
.gitignore
__pycache__
*.pyc
*.pyo
.venv
venv
env
.env
.env.*
*.egg-info
dist
build
.pytest_cache
.mypy_cache
.tox
coverage.xml
htmlcov
*.md
LICENSE
.vscode
.idea
Dockerfile*
docker-compose*.yml
.dockerignore
tests/
docs/
```

### Go Project

```
.git
.gitignore
bin/
vendor/
*.test
*.out
coverage.txt
.env
*.md
LICENSE
.vscode
.idea
Dockerfile*
docker-compose*.yml
.dockerignore
*_test.go
testdata/
docs/
```

### Monorepo / General

```
.git
.github
.gitlab-ci.yml
.circleci
.travis.yml

# IDE
.vscode
.idea
*.swp
*.swo

# OS
.DS_Store
Thumbs.db

# Environment
.env
.env.*
*.pem
*.key

# Docker
Dockerfile*
docker-compose*.yml
.dockerignore

# Documentation
*.md
LICENSE
docs/

# Tests
tests/
test/
__tests__
coverage/
.nyc_output
```

---

## CI/CD Cache Configuration Examples

### GitHub Actions with Registry Cache

```yaml
name: Build and Push
on: push

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: docker/login-action@v3
        with:
          username: ${{ vars.DOCKERHUB_USERNAME }}
          password: ${{ secrets.DOCKERHUB_TOKEN }}

      - uses: docker/setup-buildx-action@v3

      - uses: docker/build-push-action@v6
        with:
          context: .
          push: true
          tags: user/app:${{ github.sha }}
          cache-from: type=registry,ref=user/app:buildcache
          cache-to: type=registry,ref=user/app:buildcache,mode=max
```

### GitHub Actions with GHA Cache

```yaml
      - uses: docker/build-push-action@v6
        with:
          context: .
          push: true
          tags: user/app:latest
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

### GitLab CI with Registry Cache

```yaml
build:
  image: docker:24
  services:
    - docker:24-dind
  variables:
    DOCKER_BUILDKIT: 1
  script:
    - docker buildx create --use
    - docker buildx build
        --push
        --tag $CI_REGISTRY_IMAGE:$CI_COMMIT_SHA
        --cache-from type=registry,ref=$CI_REGISTRY_IMAGE:buildcache
        --cache-to type=registry,ref=$CI_REGISTRY_IMAGE:buildcache,mode=max
        .
```

### Multi-Branch Cache Strategy

```bash
#!/bin/bash
BRANCH=$(git rev-parse --abbrev-ref HEAD | tr '/' '-')

docker buildx build \
  --push \
  --tag registry/app:${BRANCH}-${GITHUB_SHA:0:7} \
  --cache-from type=registry,ref=registry/app:cache-${BRANCH} \
  --cache-from type=registry,ref=registry/app:cache-main \
  --cache-to type=registry,ref=registry/app:cache-${BRANCH},mode=max \
  .
```

ALWAYS include `cache-main` as a fallback source -- new branches immediately benefit from the main branch cache.
