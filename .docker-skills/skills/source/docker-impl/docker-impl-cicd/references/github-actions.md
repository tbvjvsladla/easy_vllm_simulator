# GitHub Actions Workflow Examples

## Docker Hub: Build and Push

```yaml
name: Docker Hub CI

on:
  push:
    branches: [main]
    tags: ["v*.*.*"]
  pull_request:
    branches: [main]

jobs:
  docker:
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

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: user/myapp
          tags: |
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
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

### Required Secrets for Docker Hub

| Secret/Variable | Where to Set | Value |
|----------------|-------------|-------|
| `vars.DOCKERHUB_USERNAME` | Repository Variables | Docker Hub username |
| `secrets.DOCKERHUB_TOKEN` | Repository Secrets | Docker Hub Personal Access Token (NOT password) |

ALWAYS use a Personal Access Token with the minimum required scope (`Read & Write` for push). NEVER use your Docker Hub password.

---

## GHCR (GitHub Container Registry): Build and Push

```yaml
name: GHCR CI

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
  docker:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to GHCR
        if: github.event_name != 'pull_request'
        uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ghcr.io/${{ github.repository }}
          tags: |
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=ref,event=branch
            type=sha,prefix=sha-

      - name: Build and push
        uses: docker/build-push-action@v6
        with:
          context: .
          push: ${{ github.event_name != 'pull_request' }}
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

GHCR uses `GITHUB_TOKEN` which is automatically available. No manual secret creation needed. ALWAYS set `permissions: packages: write` at the job or workflow level.

---

## Multi-Registry Push (Docker Hub + GHCR)

```yaml
name: Multi-Registry CI

on:
  push:
    branches: [main]
    tags: ["v*.*.*"]

permissions:
  contents: read
  packages: write

jobs:
  docker:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Set up QEMU
        uses: docker/setup-qemu-action@v3

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to Docker Hub
        uses: docker/login-action@v3
        with:
          username: ${{ vars.DOCKERHUB_USERNAME }}
          password: ${{ secrets.DOCKERHUB_TOKEN }}

      - name: Log in to GHCR
        uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract metadata
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
            type=sha,prefix=sha-

      - name: Build and push
        uses: docker/build-push-action@v6
        with:
          context: .
          platforms: linux/amd64,linux/arm64
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

When pushing to multiple registries, list ALL image names in the `metadata-action` `images` input. The action generates tags for each registry automatically.

---

## AWS ECR: Build and Push

```yaml
name: AWS ECR CI

on:
  push:
    branches: [main]
    tags: ["v*.*.*"]

permissions:
  id-token: write
  contents: read

jobs:
  docker:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Configure AWS credentials (OIDC)
        uses: aws-actions/configure-aws-credentials@v4
        with:
          role-to-assume: arn:aws:iam::123456789012:role/GitHubActions
          aws-region: us-east-1

      - name: Log in to Amazon ECR
        id: ecr-login
        uses: aws-actions/amazon-ecr-login@v2

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ steps.ecr-login.outputs.registry }}/myapp
          tags: |
            type=semver,pattern={{version}}
            type=ref,event=branch
            type=sha,prefix=sha-

      - name: Build and push
        uses: docker/build-push-action@v6
        with:
          context: .
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

ALWAYS prefer OIDC authentication (`role-to-assume`) over static access keys for AWS. OIDC provides short-lived credentials and eliminates the need to store long-lived secrets.

---

## Build Matrix: Multiple Dockerfiles

```yaml
name: Matrix Build

on:
  push:
    branches: [main]

jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        include:
          - dockerfile: Dockerfile
            image: user/myapp
            platforms: linux/amd64,linux/arm64
          - dockerfile: Dockerfile.worker
            image: user/myapp-worker
            platforms: linux/amd64
          - dockerfile: Dockerfile.migrations
            image: user/myapp-migrations
            platforms: linux/amd64
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Set up QEMU
        if: contains(matrix.platforms, 'arm')
        uses: docker/setup-qemu-action@v3

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to Docker Hub
        uses: docker/login-action@v3
        with:
          username: ${{ vars.DOCKERHUB_USERNAME }}
          password: ${{ secrets.DOCKERHUB_TOKEN }}

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ matrix.image }}

      - name: Build and push
        uses: docker/build-push-action@v6
        with:
          context: .
          file: ${{ matrix.dockerfile }}
          platforms: ${{ matrix.platforms }}
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha,scope=${{ matrix.dockerfile }}
          cache-to: type=gha,scope=${{ matrix.dockerfile }},mode=max
```

ALWAYS use the `scope` parameter on `type=gha` cache when building multiple images. Without scope, cache entries from different Dockerfiles overwrite each other.

---

## Build with Secrets

```yaml
name: Build with Secrets

on:
  push:
    branches: [main]

jobs:
  docker:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to GHCR
        uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Build and push
        uses: docker/build-push-action@v6
        with:
          context: .
          push: true
          tags: ghcr.io/${{ github.repository }}:latest
          secrets: |
            "npm_token=${{ secrets.NPM_TOKEN }}"
            "github_token=${{ secrets.GITHUB_TOKEN }}"
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

In the Dockerfile, access secrets via mount:

```dockerfile
# syntax=docker/dockerfile:1
FROM node:20-alpine
WORKDIR /app
COPY package*.json ./
RUN --mount=type=secret,id=npm_token \
    NPM_TOKEN=$(cat /run/secrets/npm_token) \
    npm ci --registry https://npm.pkg.github.com
COPY . .
RUN npm run build
```

NEVER pass secrets via `build-args`. They appear in `docker history` and are cached in image layers.

---

## Docker Scout Integration

```yaml
name: Build and Scan

on:
  push:
    branches: [main]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to Docker Hub
        uses: docker/login-action@v3
        with:
          username: ${{ vars.DOCKERHUB_USERNAME }}
          password: ${{ secrets.DOCKERHUB_TOKEN }}

      - name: Build and load locally
        uses: docker/build-push-action@v6
        with:
          context: .
          load: true
          tags: myapp:local

      - name: Docker Scout CVE scan
        uses: docker/scout-action@v1
        with:
          command: cves
          image: local://myapp:local
          only-severities: critical,high
          exit-code: true

      - name: Docker Scout recommendations
        if: always()
        uses: docker/scout-action@v1
        with:
          command: recommendations
          image: local://myapp:local

      - name: Push if scan passes
        if: success()
        uses: docker/build-push-action@v6
        with:
          context: .
          push: true
          tags: user/myapp:latest
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

ALWAYS build and scan locally first, then push only if the scan passes. This prevents vulnerable images from reaching the registry.

---

## Scheduled Rebuild for Base Image Updates

```yaml
name: Scheduled Rebuild

on:
  schedule:
    - cron: "0 4 * * 1"  # Every Monday at 04:00 UTC
  workflow_dispatch:

jobs:
  rebuild:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to Docker Hub
        uses: docker/login-action@v3
        with:
          username: ${{ vars.DOCKERHUB_USERNAME }}
          password: ${{ secrets.DOCKERHUB_TOKEN }}

      - name: Build and push (fresh base)
        uses: docker/build-push-action@v6
        with:
          context: .
          push: true
          tags: user/myapp:latest
          build-args: |
            BUILDKIT_INLINE_CACHE=1
          no-cache: true
          pull: true
```

Use `no-cache: true` and `pull: true` together to ensure the scheduled rebuild picks up all base image security patches. ALWAYS combine scheduled rebuilds with vulnerability scanning.

---

## Provenance and SBOM Attestations

```yaml
- name: Build and push with attestations
  uses: docker/build-push-action@v6
  with:
    context: .
    push: true
    tags: ${{ steps.meta.outputs.tags }}
    labels: ${{ steps.meta.outputs.labels }}
    provenance: mode=max
    sbom: true
    cache-from: type=gha
    cache-to: type=gha,mode=max
```

- `provenance: mode=max` generates SLSA provenance attestations with full build metadata.
- `sbom: true` generates a Software Bill of Materials attached to the image.
- Attestations are stored as OCI image manifests alongside the image.
- ALWAYS enable provenance and SBOM for production images to support supply chain security verification.

---

## Official Sources

- https://docs.docker.com/build/ci/github-actions/
- https://github.com/docker/build-push-action
- https://github.com/docker/metadata-action
- https://github.com/docker/login-action
- https://github.com/docker/setup-buildx-action
- https://github.com/docker/setup-qemu-action
- https://github.com/docker/scout-action
