# Image Management Anti-Patterns

> Common mistakes in Docker image management and how to avoid them.
> Source: https://docs.docker.com/reference/cli/docker/image/ and https://docs.docker.com/build/building/best-practices/

---

## Build Anti-Patterns

### AP-01: Using Legacy docker build

```bash
# WRONG -- legacy pre-BuildKit builder
docker build -t myapp:v1 .

# CORRECT -- BuildKit-based builder (default since Engine 23+)
docker buildx build -t myapp:v1 .
```

**Why**: Legacy `docker build` lacks BuildKit features: build secrets, SSH forwarding, cache exports, multi-platform builds, and parallel stage execution. ALWAYS use `docker buildx build`.

### AP-02: Building Without --platform for Cross-Architecture

```bash
# WRONG -- silently builds for host platform
docker buildx build -t myapp:v1 --push .
# Fails with "exec format error" when run on different architecture

# CORRECT -- explicitly specify target platform(s)
docker buildx build --platform linux/amd64,linux/arm64 -t myapp:v1 --push .
```

**Why**: Without `--platform`, Docker builds for the host architecture. The image works on the build machine but fails on any different architecture. ALWAYS specify `--platform` when the target environment differs from the build environment.

### AP-03: Using --load with Multi-Platform Builds

```bash
# WRONG -- --load only supports single platform
docker buildx build --platform linux/amd64,linux/arm64 --load -t myapp:v1 .
# Error: docker exporter does not support exporting manifest lists

# CORRECT -- use --push for multi-platform
docker buildx build --platform linux/amd64,linux/arm64 --push -t myregistry.com/myapp:v1 .

# CORRECT -- use --load for single platform only
docker buildx build --platform linux/amd64 --load -t myapp:v1 .
```

**Why**: The local Docker image store does not support manifest lists. Multi-platform builds MUST be pushed to a registry or exported to a directory.

### AP-04: Not Using Build Cache in CI/CD

```bash
# WRONG -- every CI build starts from scratch
docker buildx build -t myapp:v1 .

# CORRECT -- use GitHub Actions cache
docker buildx build \
  --cache-from type=gha \
  --cache-to type=gha,mode=max \
  -t myapp:v1 .

# CORRECT -- use registry cache
docker buildx build \
  --cache-from type=registry,ref=myregistry.com/myapp:cache \
  --cache-to type=registry,ref=myregistry.com/myapp:cache,mode=max \
  -t myapp:v1 .
```

**Why**: Without cache configuration, CI builds download and rebuild every layer every time. This wastes minutes per build. ALWAYS configure `--cache-from` and `--cache-to` in CI environments.

### AP-05: Embedding Secrets in Build Arguments

```bash
# WRONG -- build args are visible in image history
docker buildx build --build-arg DB_PASSWORD=secret123 -t myapp:v1 .
# Anyone can see: docker history --no-trunc myapp:v1

# CORRECT -- use build secrets (never stored in layers)
docker buildx build --secret id=DB_PASSWORD -t myapp:v1 .
```

In Dockerfile:
```dockerfile
# WRONG
ARG DB_PASSWORD
RUN echo $DB_PASSWORD > /tmp/setup && setup.sh

# CORRECT
RUN --mount=type=secret,id=DB_PASSWORD cat /run/secrets/DB_PASSWORD | setup.sh
```

**Why**: Build arguments are stored in image layer metadata and visible to anyone with `docker history`. Build secrets are mounted only during the RUN instruction and NEVER stored in any layer.

---

## Registry Anti-Patterns

### AP-06: Pushing Without Explicit Tag

```bash
# WRONG -- pushes ALL local tags for the repository
docker push myregistry.com/myapp

# CORRECT -- push specific tag only
docker push myregistry.com/myapp:v1.0
```

**Why**: Omitting the tag pushes every local tag of that repository. This can accidentally overwrite production tags or push development images.

### AP-07: Using :latest in Production

```bash
# WRONG -- "latest" is mutable and non-deterministic
docker pull myapp:latest

# CORRECT -- pin to specific version
docker pull myapp:1.25.3

# BEST -- pin to digest for immutability
docker pull myapp@sha256:abc123def456...
```

**Why**: The `latest` tag is mutable. It changes every time a new image is pushed. Production deployments MUST use specific version tags or digests to ensure reproducibility.

### AP-08: Not Tagging Before Push

```bash
# WRONG -- image not tagged for registry
docker buildx build -t myapp:v1 .
docker push myapp:v1
# Error: push refers to repository [docker.io/library/myapp]

# CORRECT -- tag with full registry path first
docker tag myapp:v1 myregistry.com/myapp:v1
docker push myregistry.com/myapp:v1

# BEST -- tag during build
docker buildx build -t myregistry.com/myapp:v1 .
docker push myregistry.com/myapp:v1
```

**Why**: Without a registry prefix, Docker defaults to Docker Hub's `library/` namespace. ALWAYS include the full registry path in the tag.

---

## Cleanup Anti-Patterns

### AP-09: Blind Nuclear Cleanup on Production

```bash
# WRONG -- removes ALL unused volumes (database data!)
docker system prune -a --volumes -f

# CORRECT -- assess first, then clean selectively
docker system df -v
docker container prune -f
docker image prune -f
# Only prune volumes after verifying none contain data
docker volume ls -f dangling=true
```

**Why**: `docker system prune -a --volumes` removes all unused volumes, including database data volumes that are not currently mounted. ALWAYS run `docker system df -v` first and NEVER prune volumes blindly on production.

### AP-10: Not Cleaning Build Cache

```bash
# WRONG -- only cleaning images and containers
docker container prune -f
docker image prune -a -f
# Build cache still consuming gigabytes

# CORRECT -- include build cache in cleanup
docker builder prune -f
# Or for all build cache:
docker builder prune -a -f
```

**Why**: Build cache can grow to tens of gigabytes and is NOT removed by `docker image prune`. ALWAYS include `docker builder prune` in cleanup routines.

### AP-11: Removing Images by Short ID Without Verification

```bash
# WRONG -- short ID might match multiple images
docker rmi abc123

# CORRECT -- verify first, then remove by full tag
docker images --no-trunc | grep abc123
docker rmi myapp:old-version
```

**Why**: Short IDs can be ambiguous. ALWAYS verify which image you are removing by listing with `--no-trunc` or removing by tag name.

---

## Image Listing Anti-Patterns

### AP-12: Ignoring Dangling Images

```bash
# WRONG -- never checking for dangling images
docker images

# CORRECT -- regularly check and clean dangling images
docker images -f dangling=true
docker image prune -f
```

**Why**: Every rebuild creates dangling images (old layers without tags). These accumulate silently and waste disk space. ALWAYS check `docker images -f dangling=true` regularly.

### AP-13: Parsing docker images Output with Text Tools

```bash
# WRONG -- fragile text parsing
docker images | grep "myapp" | awk '{print $3}' | xargs docker rmi

# CORRECT -- use --format and --filter
docker images --filter reference="myapp*" -q | xargs docker rmi

# BEST -- use built-in filter
docker images -f reference="myapp:v1*" --format "{{.ID}}" | xargs docker rmi
```

**Why**: Text parsing of docker output is fragile and breaks with format changes. ALWAYS use `--filter` and `--format` for programmatic access.

---

## Transfer Anti-Patterns

### AP-14: Using docker export Instead of docker save

```bash
# WRONG -- export loses layers, history, tags, and metadata
docker export mycontainer > backup.tar
docker import backup.tar myapp:restored
# Result: single-layer image, no CMD, no ENTRYPOINT, no history

# CORRECT -- save preserves everything
docker save -o backup.tar myapp:v1.0
docker load -i backup.tar
# Result: exact same image with all layers, tags, and metadata
```

**Why**: `docker export` exports a container's filesystem as a flat tarball -- it loses ALL image metadata including CMD, ENTRYPOINT, ENV, EXPOSE, and layer history. ALWAYS use `docker save`/`docker load` for image transfer.

### AP-15: Transferring Uncompressed Image Archives

```bash
# WRONG -- uncompressed tar can be very large
docker save -o huge-image.tar myapp:v1.0
# Result: 2GB file to transfer

# CORRECT -- compress for transfer
docker save myapp:v1.0 | gzip > myapp-v1.tar.gz
# Result: 800MB file

# BEST -- use zstd for better compression ratio and speed
docker save myapp:v1.0 | zstd > myapp-v1.tar.zst
```

**Why**: Docker images are already layer-compressed internally but the tar archive itself is not compressed. Compressing the archive can reduce transfer size by 50-70%.

---

## System Management Anti-Patterns

### AP-16: Not Monitoring Disk Usage

```bash
# WRONG -- wait until "no space left on device" error
# Then panic-run: docker system prune -a --volumes -f

# CORRECT -- proactive monitoring
docker system df              # Quick overview
docker system df -v           # Detailed breakdown

# Set up alerts on reclaimable space percentage
```

**Why**: Docker resource consumption grows silently. By the time you get a disk space error, the system may be unresponsive. ALWAYS monitor `docker system df` regularly or via scheduled checks.

### AP-17: Using docker context Without Verification

```bash
# WRONG -- switch context and immediately run destructive commands
docker context use production
docker system prune -a --volumes -f

# CORRECT -- always verify which context is active
docker context ls
docker info --format '{{.Name}}'
# Then proceed with caution
```

**Why**: Docker contexts switch the target daemon. Running cleanup or destructive commands on the wrong context can destroy production data. ALWAYS verify the active context before running any modifying command.

### AP-18: Ignoring docker system events for Debugging

```bash
# WRONG -- guessing why containers fail
docker logs myapp  # Not enough info

# CORRECT -- use system events for full picture
docker events --filter container=myapp --since 10m
docker events --filter type=image --since 1h
```

**Why**: `docker events` provides daemon-level visibility into container lifecycle, image pulls, network changes, and volume operations. ALWAYS check events when debugging unexpected behavior.

---

## Summary: Quick Rules

| Rule | Do | Don't |
|------|-----|-------|
| Builder | `docker buildx build` | `docker build` |
| Multi-platform | `--platform` + `--push` | `--load` with multiple platforms |
| Secrets | `--secret` | `--build-arg` for passwords |
| Tags | Explicit version tags | `:latest` in production |
| Push | `docker push repo:tag` | `docker push repo` (all tags) |
| Cleanup | `docker system df` first | `docker system prune -a --volumes -f` blindly |
| Transfer | `docker save` / `docker load` | `docker export` / `docker import` |
| CI cache | `--cache-from` + `--cache-to` | Uncached builds |
| Build cache | `docker builder prune` | Forget about build cache |
| Context | Verify active context | Run commands without checking |

---

## Official Sources

- https://docs.docker.com/reference/cli/docker/image/
- https://docs.docker.com/reference/cli/docker/buildx/build/
- https://docs.docker.com/build/building/best-practices/
- https://docs.docker.com/reference/cli/docker/system/
