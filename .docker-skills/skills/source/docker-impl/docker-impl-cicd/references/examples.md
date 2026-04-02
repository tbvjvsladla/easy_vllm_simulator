# CI/CD Examples: Multi-Platform, Registry Auth, Cache Strategies

## Multi-Platform Build Examples

### Basic Multi-Platform (amd64 + arm64)

```yaml
- name: Set up QEMU
  uses: docker/setup-qemu-action@v3

- name: Set up Docker Buildx
  uses: docker/setup-buildx-action@v3

- name: Build and push
  uses: docker/build-push-action@v6
  with:
    context: .
    platforms: linux/amd64,linux/arm64
    push: true
    tags: user/myapp:latest
```

QEMU is required for any platform that does not match the runner's native architecture. GitHub Actions runners are `linux/amd64`, so `linux/arm64` and all other platforms require QEMU.

### Cross-Compilation Dockerfile (Go)

Cross-compilation is 5-10x faster than QEMU emulation for compiled languages.

```dockerfile
# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM golang:1.22-alpine AS build

# These ARGs are automatically set by BuildKit
ARG TARGETOS TARGETARCH

WORKDIR /src
COPY go.mod go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod go mod download

COPY . .
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    GOOS=$TARGETOS GOARCH=$TARGETARCH CGO_ENABLED=0 \
    go build -ldflags="-s -w" -o /app ./cmd/server

FROM alpine:3.21
RUN apk --no-cache add ca-certificates
COPY --from=build /app /usr/bin/app
USER 1001
ENTRYPOINT ["/usr/bin/app"]
```

Key points:
- `--platform=$BUILDPLATFORM` runs the build stage on the CI runner's native arch.
- `TARGETOS` and `TARGETARCH` are set automatically by BuildKit for each target platform.
- `CGO_ENABLED=0` is required for static cross-compilation without C dependencies.
- The runtime stage runs on the target platform natively.

### Cross-Compilation Dockerfile (Rust)

```dockerfile
# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM rust:1.77-alpine AS build

ARG TARGETARCH
RUN apk add --no-cache musl-dev

# Install cross-compilation target
RUN case "$TARGETARCH" in \
      amd64) RUST_TARGET="x86_64-unknown-linux-musl" ;; \
      arm64) RUST_TARGET="aarch64-unknown-linux-musl" ;; \
      *) echo "Unsupported: $TARGETARCH" && exit 1 ;; \
    esac && \
    rustup target add "$RUST_TARGET" && \
    echo "$RUST_TARGET" > /rust-target.txt

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release --target $(cat /rust-target.txt) && \
    rm -rf src

COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --target $(cat /rust-target.txt) && \
    cp target/$(cat /rust-target.txt)/release/myapp /app

FROM alpine:3.21
COPY --from=build /app /usr/bin/app
USER 1001
ENTRYPOINT ["/usr/bin/app"]
```

### Node.js Multi-Platform (Interpreted Language)

Interpreted languages do not need cross-compilation. QEMU emulates the target platform for native dependency installation.

```dockerfile
# syntax=docker/dockerfile:1
FROM node:20-alpine AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci --production

FROM node:20-alpine
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .
USER 1001
EXPOSE 3000
CMD ["node", "server.js"]
```

For interpreted languages, the entire build runs under QEMU emulation for non-native platforms. This is slower but unavoidable when native extensions (e.g., `bcrypt`, `sharp`) must be compiled for the target architecture.

### Extended Platform Matrix

```yaml
- name: Build and push (all platforms)
  uses: docker/build-push-action@v6
  with:
    context: .
    platforms: |
      linux/amd64
      linux/arm64
      linux/arm/v7
      linux/arm/v6
      linux/386
      linux/ppc64le
      linux/s390x
    push: true
    tags: user/myapp:latest
```

NEVER include platforms your application does not support. Test on each platform before adding it to the matrix.

---

## Registry Authentication Examples

### Docker Hub with Personal Access Token

```yaml
- name: Log in to Docker Hub
  uses: docker/login-action@v3
  with:
    username: ${{ vars.DOCKERHUB_USERNAME }}
    password: ${{ secrets.DOCKERHUB_TOKEN }}
```

Create the token at https://hub.docker.com/settings/security. Select "Read & Write" scope for push access.

### GHCR with GITHUB_TOKEN

```yaml
- name: Log in to GHCR
  uses: docker/login-action@v3
  with:
    registry: ghcr.io
    username: ${{ github.actor }}
    password: ${{ secrets.GITHUB_TOKEN }}
```

Requires `permissions: packages: write` at the job or workflow level. No manual secret creation needed.

### AWS ECR with OIDC (Recommended)

```yaml
permissions:
  id-token: write
  contents: read

steps:
  - name: Configure AWS credentials
    uses: aws-actions/configure-aws-credentials@v4
    with:
      role-to-assume: arn:aws:iam::123456789012:role/GitHubActionsRole
      aws-region: us-east-1

  - name: Log in to Amazon ECR
    id: ecr-login
    uses: aws-actions/amazon-ecr-login@v2
```

OIDC eliminates long-lived access keys. The IAM role must trust the GitHub OIDC provider and allow `ecr:GetAuthorizationToken` plus `ecr:BatchCheckLayerAvailability`, `ecr:PutImage`, etc.

### AWS ECR with Access Keys (Legacy)

```yaml
- name: Log in to Amazon ECR
  uses: docker/login-action@v3
  with:
    registry: 123456789012.dkr.ecr.us-east-1.amazonaws.com
    username: ${{ secrets.AWS_ACCESS_KEY_ID }}
    password: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
```

ALWAYS prefer OIDC over static access keys. If using access keys, rotate them regularly and use the minimum required IAM permissions.

### Azure ACR

```yaml
- name: Log in to Azure ACR
  uses: docker/login-action@v3
  with:
    registry: myregistry.azurecr.io
    username: ${{ secrets.ACR_USERNAME }}
    password: ${{ secrets.ACR_PASSWORD }}
```

### Google Artifact Registry

```yaml
- name: Authenticate to Google Cloud
  uses: google-github-actions/auth@v2
  with:
    workload_identity_provider: projects/123/locations/global/workloadIdentityPools/pool/providers/provider
    service_account: ci@project.iam.gserviceaccount.com

- name: Log in to GAR
  uses: docker/login-action@v3
  with:
    registry: us-docker.pkg.dev
    username: oauth2accesstoken
    password: ${{ steps.auth.outputs.access_token }}
```

### Self-Hosted Registry

```yaml
- name: Log in to private registry
  uses: docker/login-action@v3
  with:
    registry: registry.example.com
    username: ${{ secrets.REGISTRY_USERNAME }}
    password: ${{ secrets.REGISTRY_PASSWORD }}
```

---

## Cache Strategy Examples

### GitHub Actions Cache (Recommended for GitHub CI)

```yaml
- uses: docker/build-push-action@v6
  with:
    cache-from: type=gha
    cache-to: type=gha,mode=max
```

Characteristics:
- 10 GB total cache per repository.
- Branch-scoped: feature branch cache falls back to default branch.
- Fastest option in GitHub Actions (uses the same storage as `actions/cache`).
- ALWAYS use `mode=max` — `mode=min` only caches the final exported layers.

### Registry Cache (Cross-CI, Cross-Branch)

```yaml
- uses: docker/build-push-action@v6
  with:
    cache-from: type=registry,ref=user/myapp:buildcache
    cache-to: type=registry,ref=user/myapp:buildcache,mode=max
```

Characteristics:
- Shared across all branches, forks, and CI providers.
- Requires registry authentication.
- Adds push/pull time but avoids full rebuilds.
- Use a dedicated tag (`:buildcache`) to avoid polluting image tags.

### Multi-Source Cache Fallback

```yaml
- uses: docker/build-push-action@v6
  with:
    cache-from: |
      type=registry,ref=user/myapp:cache-${{ github.ref_name }}
      type=registry,ref=user/myapp:cache-main
    cache-to: type=registry,ref=user/myapp:cache-${{ github.ref_name }},mode=max
```

BuildKit tries cache sources in order. Feature branches get their own cache but fall back to the main branch cache when their own is empty.

### Scoped Cache for Matrix Builds

```yaml
strategy:
  matrix:
    service: [api, worker, web]

steps:
  - uses: docker/build-push-action@v6
    with:
      context: .
      file: docker/${{ matrix.service }}/Dockerfile
      cache-from: type=gha,scope=build-${{ matrix.service }}
      cache-to: type=gha,scope=build-${{ matrix.service }},mode=max
```

Without `scope`, all matrix jobs share the same cache namespace and overwrite each other's entries. ALWAYS use scope when building multiple images in the same workflow.

### Inline Cache (Simple, No External Storage)

```yaml
- uses: docker/build-push-action@v6
  with:
    push: true
    tags: user/myapp:latest
    build-args: BUILDKIT_INLINE_CACHE=1
    cache-from: type=registry,ref=user/myapp:latest
```

Inline cache embeds cache metadata directly in the pushed image. Simpler than dedicated cache images but only caches exported layers (`mode=min` equivalent).

### Local Cache (Self-Hosted Runners)

```yaml
- uses: docker/build-push-action@v6
  with:
    cache-from: type=local,src=/tmp/.buildx-cache
    cache-to: type=local,dest=/tmp/.buildx-cache-new,mode=max

# Rotate cache to prevent unbounded growth
- name: Move cache
  run: |
    rm -rf /tmp/.buildx-cache
    mv /tmp/.buildx-cache-new /tmp/.buildx-cache
```

ALWAYS rotate local cache (write to new dir, then swap) to prevent the cache directory from growing indefinitely. Each build appends new layers without removing old ones.

---

## Build Argument Patterns

### Version Injection

```yaml
- name: Build and push
  uses: docker/build-push-action@v6
  with:
    context: .
    build-args: |
      VERSION=${{ github.ref_name }}
      COMMIT_SHA=${{ github.sha }}
      BUILD_DATE=${{ github.event.head_commit.timestamp }}
    push: true
    tags: user/myapp:${{ github.ref_name }}
```

```dockerfile
# syntax=docker/dockerfile:1
ARG VERSION=dev
ARG COMMIT_SHA=unknown
ARG BUILD_DATE=unknown

FROM alpine:3.21
LABEL org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${COMMIT_SHA}" \
      org.opencontainers.image.created="${BUILD_DATE}"
# ...
```

### Target Stage for Dev/Prod

```yaml
# Development build
- uses: docker/build-push-action@v6
  with:
    context: .
    target: development
    load: true
    tags: myapp:dev

# Production build
- uses: docker/build-push-action@v6
  with:
    context: .
    target: production
    push: true
    tags: user/myapp:latest
```

```dockerfile
# syntax=docker/dockerfile:1
FROM node:20-alpine AS base
WORKDIR /app
COPY package*.json ./

FROM base AS development
RUN npm install
COPY . .
CMD ["npm", "run", "dev"]

FROM base AS production
RUN npm ci --production
COPY . .
USER 1001
CMD ["node", "server.js"]
```

---

## Official Sources

- https://docs.docker.com/build/ci/github-actions/
- https://docs.docker.com/build/building/multi-platform/
- https://docs.docker.com/build/cache/backends/
- https://docs.docker.com/build/cache/backends/gha/
- https://github.com/docker/build-push-action
- https://github.com/docker/login-action
