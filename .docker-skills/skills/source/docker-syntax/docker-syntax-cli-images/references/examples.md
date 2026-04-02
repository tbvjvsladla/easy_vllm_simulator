# Image Management Workflows & Cleanup Scripts

> Practical workflows for building, transferring, and cleaning up Docker images.
> Source: https://docs.docker.com/reference/cli/docker/image/ and https://docs.docker.com/reference/cli/docker/system/

---

## Build Workflows

### Basic Build and Tag

```bash
# Build with tag
docker buildx build -t myapp:v1.0 .

# Build with multiple tags
docker buildx build -t myapp:v1.0 -t myapp:latest .

# Build from specific Dockerfile
docker buildx build -f Dockerfile.prod -t myapp:prod .

# Build specific stage from multi-stage Dockerfile
docker buildx build --target builder -t myapp:build-stage .

# Build with build arguments
docker buildx build --build-arg NODE_ENV=production --build-arg APP_VERSION=1.0 -t myapp:v1.0 .
```

### Build with Full Output (Debugging)

```bash
# Plain text progress (shows all build output)
docker buildx build --progress=plain -t myapp:v1.0 .

# No cache (force full rebuild)
docker buildx build --no-cache --pull -t myapp:v1.0 .

# Write metadata to file for CI inspection
docker buildx build --metadata-file build-meta.json -t myapp:v1.0 .
```

### Build with Secrets

```bash
# Secret from file (NEVER baked into image layers)
docker buildx build --secret id=npmrc,src=$HOME/.npmrc -t myapp:v1.0 .

# Secret from environment variable
DB_PASSWORD=secret123 docker buildx build --secret id=DB_PASSWORD -t myapp:v1.0 .

# SSH agent for private Git repos
docker buildx build --ssh default=$SSH_AUTH_SOCK -t myapp:v1.0 .
```

### Build with Cache (CI/CD)

```bash
# GitHub Actions cache
docker buildx build \
  --cache-from type=gha \
  --cache-to type=gha,mode=max \
  -t myapp:v1.0 .

# Registry-based cache
docker buildx build \
  --cache-from type=registry,ref=myregistry.com/myapp:cache \
  --cache-to type=registry,ref=myregistry.com/myapp:cache,mode=max \
  -t myregistry.com/myapp:v1.0 \
  --push .

# Local directory cache
docker buildx build \
  --cache-from type=local,src=/tmp/docker-cache \
  --cache-to type=local,dest=/tmp/docker-cache \
  -t myapp:v1.0 .
```

---

## Multi-Platform Build Workflows

### Setup Multi-Platform Builder

```bash
# Create builder with docker-container driver (required for multi-platform)
docker buildx create --name multiplatform --driver docker-container --use

# Verify platforms supported
docker buildx inspect --bootstrap
```

### Build for Multiple Platforms

```bash
# Build and push for amd64 + arm64
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t myregistry.com/myapp:v1.0 \
  --push .

# Build and push for all common platforms
docker buildx build \
  --platform linux/amd64,linux/arm64,linux/arm/v7 \
  -t myregistry.com/myapp:v1.0 \
  --push .

# Build single platform for local testing
docker buildx build --platform linux/arm64 --load -t myapp:arm64-test .
```

### Create Manifest List Manually

```bash
# Build per-platform images
docker buildx build --platform linux/amd64 --load -t myapp:amd64 .
docker buildx build --platform linux/arm64 --load -t myapp:arm64 .

# Tag and push each
docker tag myapp:amd64 myregistry.com/myapp:amd64
docker tag myapp:arm64 myregistry.com/myapp:arm64
docker push myregistry.com/myapp:amd64
docker push myregistry.com/myapp:arm64

# Create and push manifest list
docker manifest create myregistry.com/myapp:latest \
  myregistry.com/myapp:amd64 \
  myregistry.com/myapp:arm64
docker manifest push myregistry.com/myapp:latest
```

---

## Registry Workflows

### Tag and Push

```bash
# Tag for registry
docker tag myapp:v1.0 myregistry.com/myapp:v1.0
docker tag myapp:v1.0 myregistry.com/myapp:latest

# Push specific tag
docker push myregistry.com/myapp:v1.0

# Push all tags at once
docker push --all-tags myregistry.com/myapp
```

### Pull Strategies

```bash
# Pull by tag (mutable -- may change)
docker pull nginx:1.25

# Pull by digest (immutable -- ALWAYS gets exact same image)
docker pull nginx@sha256:abc123def456...

# Pull for specific platform
docker pull --platform linux/arm64 nginx:1.25

# Inspect remote manifest without pulling
docker manifest inspect nginx:latest
```

---

## Offline Transfer Workflows

### Transfer Image Between Hosts

```bash
# On source host: save image to tar
docker save -o myapp-v1.tar myapp:v1.0

# Transfer file (scp, usb, etc.)
scp myapp-v1.tar user@target-host:/tmp/

# On target host: load image from tar
docker load -i /tmp/myapp-v1.tar

# Verify
docker images myapp
```

### Transfer Multiple Images

```bash
# Save multiple images into one archive
docker save -o all-images.tar nginx:1.25 redis:7 postgres:16

# Load all images at once
docker load -i all-images.tar
```

### Compressed Transfer (Save Bandwidth)

```bash
# Save with compression
docker save myapp:v1.0 | gzip > myapp-v1.tar.gz

# Load from compressed archive
gunzip -c myapp-v1.tar.gz | docker load

# Or using zstd for better compression
docker save myapp:v1.0 | zstd > myapp-v1.tar.zst
zstd -d myapp-v1.tar.zst --stdout | docker load
```

### Export Build Output to Filesystem

```bash
# Export build result to local directory (no image created)
docker buildx build -o type=local,dest=./output .

# Export as tar to stdout
docker buildx build -o type=tar,dest=image.tar .

# Export as OCI layout
docker buildx build -o type=oci,dest=oci-image.tar .
```

---

## Image Inspection Workflows

### Analyze Image Layers

```bash
# View full layer history (see what each layer does)
docker history --no-trunc myapp:v1.0

# Layer sizes only
docker history --format "table {{.Size}}\t{{.CreatedBy}}" myapp:v1.0

# Layer IDs for debugging
docker history -q myapp:v1.0
```

### Find Large Images

```bash
# Sort images by size (largest first)
docker images --format "{{.Size}}\t{{.Repository}}:{{.Tag}}" | sort -hr

# Show images with exact sizes
docker images --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}"
```

### Find Dangling Images

```bash
# List all dangling (untagged) images
docker images -f dangling=true

# Count dangling images
docker images -f dangling=true -q | wc -l

# Show reclaimable space
docker system df
```

---

## Cleanup Scripts

### Daily Cleanup Script

```bash
#!/bin/bash
# daily-cleanup.sh -- Safe daily Docker cleanup
# ALWAYS assess before cleaning

echo "=== Docker Disk Usage ==="
docker system df

echo ""
echo "=== Removing stopped containers ==="
docker container prune -f --filter "until=24h"

echo ""
echo "=== Removing dangling images ==="
docker image prune -f

echo ""
echo "=== Removing unused networks ==="
docker network prune -f --filter "until=24h"

echo ""
echo "=== Disk Usage After Cleanup ==="
docker system df
```

### Aggressive Cleanup Script (Development Only)

```bash
#!/bin/bash
# dev-cleanup.sh -- Aggressive cleanup for development machines
# NEVER use on production systems with data volumes

echo "=== Before Cleanup ==="
docker system df

echo ""
echo "=== Full Cleanup ==="
docker container prune -f
docker image prune -a -f
docker network prune -f
docker builder prune -a -f

echo ""
echo "=== After Cleanup ==="
docker system df
```

### Production-Safe Cleanup Script

```bash
#!/bin/bash
# prod-cleanup.sh -- Conservative cleanup for production
# NEVER removes volumes or actively-used images

echo "=== Docker Disk Usage ==="
docker system df -v

echo ""
echo "=== Removing stopped containers older than 7 days ==="
docker container prune -f --filter "until=168h"

echo ""
echo "=== Removing dangling images only ==="
docker image prune -f

echo ""
echo "=== Removing build cache older than 7 days ==="
docker builder prune -f --filter "until=168h"

echo ""
echo "=== Disk Usage After Cleanup ==="
docker system df
```

### Scheduled Cleanup (Cron)

```bash
# Add to crontab: crontab -e

# Daily at 2 AM: remove containers older than 24h and dangling images
0 2 * * * docker container prune -f --filter "until=24h" && docker image prune -f

# Weekly on Sunday at 3 AM: more aggressive cleanup
0 3 * * 0 docker container prune -f && docker image prune -a -f --filter "until=168h" && docker builder prune -f --filter "until=168h"
```

---

## CI/CD Image Workflows

### Build, Test, Push Pipeline

```bash
#!/bin/bash
# ci-pipeline.sh -- Build, test, and push in CI
set -euo pipefail

IMAGE="myregistry.com/myapp"
TAG="${CI_COMMIT_SHA:-latest}"

# Build
docker buildx build \
  --cache-from type=gha \
  --cache-to type=gha,mode=max \
  --load \
  -t "${IMAGE}:${TAG}" .

# Test
docker run --rm "${IMAGE}:${TAG}" npm test

# Tag and push
docker tag "${IMAGE}:${TAG}" "${IMAGE}:latest"
docker push "${IMAGE}:${TAG}"
docker push "${IMAGE}:latest"

# Clean up CI runner
docker rmi "${IMAGE}:${TAG}" "${IMAGE}:latest" 2>/dev/null || true
```

### Build with Provenance and SBOM

```bash
# Production build with supply chain attestations
docker buildx build \
  --provenance=mode=max \
  --sbom \
  --push \
  -t myregistry.com/myapp:v1.0 .
```

---

## System Monitoring Workflow

### Quick System Health Check

```bash
#!/bin/bash
# docker-health.sh -- Quick system overview

echo "=== Docker Version ==="
docker version --format '{{.Server.Version}}'

echo ""
echo "=== Storage Driver ==="
docker info --format '{{.Driver}}'

echo ""
echo "=== Disk Usage ==="
docker system df

echo ""
echo "=== Running Containers ==="
docker ps --format "table {{.Names}}\t{{.Image}}\t{{.Status}}"

echo ""
echo "=== Image Count ==="
echo "Total: $(docker images -q | wc -l)"
echo "Dangling: $(docker images -q -f dangling=true | wc -l)"
```

---

## Official Sources

- https://docs.docker.com/reference/cli/docker/image/
- https://docs.docker.com/reference/cli/docker/buildx/build/
- https://docs.docker.com/reference/cli/docker/system/
- https://docs.docker.com/reference/cli/docker/system/prune/
