---
name: docker-impl-cicd
description: >
  Use when setting up Docker CI/CD pipelines, pushing images to registries,
  or configuring multi-platform builds with GitHub Actions.
  Prevents broken workflows from incorrect docker/build-push-action usage,
  missing registry authentication, or misconfigured buildx cache backends.
  Covers GitHub Actions workflows, GHCR and Docker Hub auth, buildx QEMU
  multi-platform builds, cache-to/cache-from, image tagging conventions.
  Keywords: docker/build-push-action, docker/login-action, buildx, QEMU,
  GHCR, ghcr.io, docker push, CI/CD, GitHub Actions, matrix builds,
  automate Docker build, push to registry, deploy container, pipeline.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+ with BuildKit."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-impl-cicd

## Quick Reference

### GitHub Actions Docker Toolkit

| Action | Purpose | Required |
|--------|---------|----------|
| `docker/setup-buildx-action@v3` | Install and configure buildx builder | ALWAYS |
| `docker/setup-qemu-action@v3` | Install QEMU for multi-platform builds | Only for multi-arch |
| `docker/login-action@v3` | Authenticate to container registries | ALWAYS before push |
| `docker/build-push-action@v6` | Build and push images with BuildKit | ALWAYS |
| `docker/metadata-action@v5` | Generate tags and labels from Git context | ALWAYS |

### Registry Authentication Comparison

| Registry | Login Server | Username Secret | Password Secret |
|----------|-------------|-----------------|-----------------|
| Docker Hub | (default) | `DOCKERHUB_USERNAME` | `DOCKERHUB_TOKEN` |
| GHCR | `ghcr.io` | `github.actor` | `secrets.GITHUB_TOKEN` |
| AWS ECR | `<account>.dkr.ecr.<region>.amazonaws.com` | `AWS_ACCESS_KEY_ID` | `AWS_SECRET_ACCESS_KEY` |
| Azure ACR | `<name>.azurecr.io` | `ACR_USERNAME` | `ACR_PASSWORD` |
| Google GAR | `<region>-docker.pkg.dev` | `_json_key` | Service account JSON |

### Cache Strategy Decision Tree

```
Is this a GitHub Actions workflow?
├── YES → Use type=gha (fastest, no registry auth needed)
│   └── ALWAYS set mode=max for full intermediate layer caching
├── NO → Is a container registry available?
│   ├── YES → Use type=registry with a dedicated cache tag
│   │   └── ALWAYS use mode=max for CI builds
│   └── NO → Use type=local with mounted volume
└── Need to share cache across forks/PRs?
    └── Use type=registry (gha cache is scoped to branch)
```

### Image Tagging Conventions

| Trigger | Tag Pattern | Example |
|---------|------------|---------|
| Push to main | `latest`, `main` | `myapp:latest` |
| Git tag (semver) | `v1.2.3`, `1.2.3`, `1.2`, `1` | `myapp:1.2.3` |
| Pull request | `pr-<number>` | `myapp:pr-42` |
| Branch push | `<branch-name>` | `myapp:feature-auth` |
| Git SHA | `sha-<short-sha>` | `myapp:sha-a1b2c3d` |

### Critical Warnings

**NEVER** hardcode registry credentials in workflow files or Dockerfiles. ALWAYS use GitHub Secrets or OIDC for authentication.

**NEVER** use `docker login` with plaintext passwords in CI. ALWAYS use `docker/login-action` which handles credential storage securely.

**NEVER** push images without cache configuration in CI. Without `--cache-from`/`--cache-to`, every CI build starts from scratch, wasting minutes.

**NEVER** use `type=gha` cache with `mode=min` (the default). ALWAYS set `mode=max` to cache all intermediate layers, not just exported layers.

**NEVER** build multi-platform images with `--load`. Multi-platform manifests can only be pushed to a registry (`--push`) or exported to a file.

**ALWAYS** pin GitHub Action versions to major tags (e.g., `@v3`) at minimum. Pin to SHA for maximum security in production workflows.

---

## Complete Build-and-Push Workflow

```yaml
name: Build and Push Docker Image

on:
  push:
    branches: [main]
    tags: ["v*.*.*"]
  pull_request:
    branches: [main]

permissions:
  contents: read
  packages: write

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Set up QEMU
        uses: docker/setup-qemu-action@v3

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to Docker Hub
        if: github.event_name != 'pull_request'
        uses: docker/login-action@v3
        with:
          username: ${{ vars.DOCKERHUB_USERNAME }}
          password: ${{ secrets.DOCKERHUB_TOKEN }}

      - name: Log in to GHCR
        if: github.event_name != 'pull_request'
        uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract metadata (tags, labels)
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: |
            user/myapp
            ghcr.io/${{ github.repository }}
          tags: |
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=semver,pattern={{major}}
            type=ref,event=branch
            type=ref,event=pr
            type=sha,prefix=sha-

      - name: Build and push
        uses: docker/build-push-action@v6
        with:
          context: .
          platforms: linux/amd64,linux/arm64
          push: ${{ github.event_name != 'pull_request' }}
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

---

## Multi-Platform Build Setup

### Platform Support Matrix

| Platform | QEMU Required | Common Use Case |
|----------|--------------|-----------------|
| `linux/amd64` | No (native on most CI) | Standard x86-64 servers |
| `linux/arm64` | Yes | AWS Graviton, Apple Silicon, Raspberry Pi 4+ |
| `linux/arm/v7` | Yes | Raspberry Pi 3, older ARM devices |
| `linux/arm/v6` | Yes | Raspberry Pi Zero/1 |
| `linux/386` | Yes | Legacy 32-bit systems |
| `linux/s390x` | Yes | IBM mainframes |
| `linux/ppc64le` | Yes | IBM POWER systems |

### Cross-Compilation Pattern (Faster than QEMU)

For compiled languages, cross-compile on the native platform instead of emulating:

```dockerfile
# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM golang:1.22 AS build
ARG TARGETOS TARGETARCH
WORKDIR /src
COPY go.* ./
RUN go mod download
COPY . .
RUN GOOS=$TARGETOS GOARCH=$TARGETARCH CGO_ENABLED=0 \
    go build -o /app ./cmd

FROM alpine:3.21
COPY --from=build /app /usr/bin/app
USER 1001
ENTRYPOINT ["/usr/bin/app"]
```

**ALWAYS** use `--platform=$BUILDPLATFORM` on the build stage and cross-compile with `TARGETOS`/`TARGETARCH`. This avoids QEMU emulation for the compilation step, reducing build time by 5-10x.

---

## Cache Strategies in CI

### GitHub Actions Cache (`type=gha`)

```yaml
- uses: docker/build-push-action@v6
  with:
    cache-from: type=gha
    cache-to: type=gha,mode=max
```

- Uses GitHub Actions cache service (same as `actions/cache`).
- Scoped to the current branch; falls back to the default branch.
- 10 GB limit per repository. Old entries are evicted automatically.
- ALWAYS use `mode=max` to cache all intermediate layers.

### Registry Cache (`type=registry`)

```yaml
- uses: docker/build-push-action@v6
  with:
    cache-from: type=registry,ref=user/myapp:buildcache
    cache-to: type=registry,ref=user/myapp:buildcache,mode=max
```

- Stored as a separate image manifest in the registry.
- Shared across all branches, PRs, forks, and CI providers.
- Requires registry authentication.
- ALWAYS use a dedicated cache tag (e.g., `:buildcache`), not `:latest`.

### Multi-Branch Cache Strategy

```yaml
cache-from: |
  type=registry,ref=user/myapp:cache-${{ github.ref_name }}
  type=registry,ref=user/myapp:cache-main
cache-to: type=registry,ref=user/myapp:cache-${{ github.ref_name }},mode=max
```

This pattern caches per-branch with a fallback to main, ensuring feature branches benefit from the main branch cache.

---

## Docker Scout in CI

```yaml
- name: Docker Scout CVE scan
  uses: docker/scout-action@v1
  with:
    command: cves
    image: ${{ steps.meta.outputs.tags }}
    only-severities: critical,high
    exit-code: true
```

- `exit-code: true` fails the workflow if critical/high vulnerabilities are found.
- ALWAYS run Scout after building but before deploying to production.
- Use `sarif` output format for GitHub Security tab integration.

---

## metadata-action Tag Types

| Type | Input | Output Tag |
|------|-------|-----------|
| `type=semver,pattern={{version}}` | Tag `v1.2.3` | `1.2.3` |
| `type=semver,pattern={{major}}.{{minor}}` | Tag `v1.2.3` | `1.2` |
| `type=semver,pattern={{major}}` | Tag `v1.2.3` | `1` |
| `type=ref,event=branch` | Push to `main` | `main` |
| `type=ref,event=pr` | PR #42 | `pr-42` |
| `type=sha,prefix=sha-` | Any commit | `sha-a1b2c3d` |
| `type=schedule` | Cron trigger | `nightly` |
| `type=raw,value=latest` | Manual | `latest` |
| `type=edge` | Default branch push | `edge` |

---

## Security Best Practices for CI/CD

| Practice | Implementation |
|----------|---------------|
| NEVER hardcode credentials | Use `secrets.*` in GitHub Actions |
| ALWAYS use access tokens | Docker Hub: Personal Access Token, not password |
| ALWAYS scope permissions | `permissions: packages: write` only when needed |
| NEVER push from PRs | Guard with `if: github.event_name != 'pull_request'` |
| ALWAYS pin action versions | Use `@v3` or full SHA for supply chain security |
| ALWAYS scan images | Run Docker Scout or Trivy before deployment |
| NEVER store secrets in ARG/ENV | Use `--secret` flag in build |
| ALWAYS use OIDC when possible | Keyless auth for AWS ECR, GCP GAR |

---

## Reference Links

- [references/github-actions.md](references/github-actions.md) -- Complete GitHub Actions workflow examples for all registry types
- [references/examples.md](references/examples.md) -- Multi-platform builds, registry auth, cache strategies
- [references/anti-patterns.md](references/anti-patterns.md) -- CI/CD mistakes and how to avoid them

### Official Sources

- https://docs.docker.com/build/ci/github-actions/
- https://github.com/docker/build-push-action
- https://github.com/docker/metadata-action
- https://github.com/docker/login-action
- https://github.com/docker/setup-buildx-action
- https://docs.docker.com/build/cache/backends/gha/
- https://docs.docker.com/build/building/multi-platform/
- https://docs.docker.com/scout/integrations/ci/gha/
