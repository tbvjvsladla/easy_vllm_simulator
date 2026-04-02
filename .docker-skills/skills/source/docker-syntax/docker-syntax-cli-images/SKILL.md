---
name: docker-syntax-cli-images
description: >
  Use when building, tagging, pushing, or cleaning up Docker images.
  Prevents dangling image accumulation from missing prune commands and
  broken deployments from incorrect tag or push sequences.
  Covers docker buildx build, pull, push, tag, images, rmi, image prune,
  save, load, history, manifest inspect, system df, and system prune.
  Keywords: docker build, docker pull, docker push, docker tag, docker images,
  docker save, docker system prune, disk space full, cleanup, dangling images,
  free space, delete old images.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-syntax-cli-images

## Quick Reference

### Image Lifecycle Commands

| Command | Purpose | Example |
|---------|---------|---------|
| `docker buildx build` | Build image from Dockerfile (default builder) | `docker buildx build -t myapp:v1 .` |
| `docker pull` | Download image from registry | `docker pull nginx:1.25` |
| `docker push` | Upload image to registry | `docker push myregistry.com/myapp:v1` |
| `docker tag` | Create new tag for existing image | `docker tag myapp:v1 myregistry.com/myapp:v1` |
| `docker images` | List local images | `docker images --filter dangling=true` |
| `docker rmi` | Remove image(s) | `docker rmi nginx:old` |
| `docker image prune` | Remove unused images | `docker image prune -a -f` |
| `docker save` | Export image to tar archive | `docker save -o backup.tar myapp:v1` |
| `docker load` | Import image from tar archive | `docker load -i backup.tar` |
| `docker history` | Show image layer history | `docker history --no-trunc myapp:v1` |
| `docker manifest inspect` | Inspect multi-platform manifest | `docker manifest inspect nginx:latest` |

### System Management Commands

| Command | Purpose | Example |
|---------|---------|---------|
| `docker system df` | Show disk usage breakdown | `docker system df -v` |
| `docker system prune` | Remove all unused resources | `docker system prune -a --volumes -f` |
| `docker info` | Show system-wide information | `docker info --format '{{.ServerVersion}}'` |
| `docker version` | Show client/server versions | `docker version --format json` |
| `docker context` | Manage remote Docker hosts | `docker context use remote` |

### Critical Warnings

**NEVER** run `docker system prune -a --volumes` on production systems without first running `docker system df` -- it removes ALL unused volumes including database data.

**NEVER** use `docker build` (legacy) for new projects -- ALWAYS use `docker buildx build` which is the default BuildKit-based builder since Docker Engine 23+.

**NEVER** push images without verifying the tag -- `docker push myapp` pushes ALL tags of that repository. ALWAYS specify the exact tag: `docker push myapp:v1`.

**ALWAYS** use `--platform` when building for a different architecture -- omitting it silently builds for the host platform, causing `exec format error` at runtime on the target.

**ALWAYS** run `docker system df` before any cleanup operation to understand what is consuming disk space.

---

## Buildx Build Flag Reference

### Core Build Flags

| Flag | Description | Example |
|------|-------------|---------|
| `-f, --file` | Dockerfile path | `-f Dockerfile.prod` |
| `-t, --tag` | Tag the image (repeatable) | `-t myapp:v1 -t myapp:latest` |
| `--target` | Build specific stage | `--target build-env` |
| `--build-arg` | Build-time variable | `--build-arg NODE_ENV=prod` |
| `--no-cache` | Disable build cache entirely | `--no-cache` |
| `--pull` | Always pull base images | `--pull` |
| `--progress` | Output format: auto/plain/tty/quiet | `--progress=plain` |

### Output Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--load` | Load into local Docker images | `--load` |
| `--push` | Push to registry after build | `--push` |
| `-o, --output` | Custom output destination | `-o type=local,dest=./out` |

Output types: `docker` (local), `registry` (push), `local` (filesystem), `tar`, `oci`, `image`.

### Multi-Platform Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--platform` | Target platform(s) | `--platform linux/amd64,linux/arm64` |
| `--builder` | Use specific builder instance | `--builder mybuilder` |

**ALWAYS** use `--push` or `-o` (not `--load`) when building for multiple platforms -- `--load` only supports single-platform images.

### Cache Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--cache-from` | Import cache source | `--cache-from type=gha` |
| `--cache-to` | Export cache destination | `--cache-to type=gha,mode=max` |

Cache types: `registry`, `local`, `inline`, `gha` (GitHub Actions), `s3`, `azblob`.

### Security & Secrets Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--secret` | Expose secret to build (never baked in) | `--secret id=aws,src=$HOME/.aws/credentials` |
| `--ssh` | Expose SSH agent/keys to build | `--ssh default=$SSH_AUTH_SOCK` |

### Attestation Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--provenance` | SLSA provenance attestation | `--provenance=mode=max` |
| `--sbom` | Software Bill of Materials | `--sbom` |
| `--metadata-file` | Write build metadata as JSON | `--metadata-file meta.json` |

---

## Buildx Builder Management

```bash
# Create a new builder instance
docker buildx create --name mybuilder --use

# List all builders
docker buildx ls

# Inspect current builder
docker buildx inspect

# Switch to a builder
docker buildx use mybuilder

# Remove a builder
docker buildx rm mybuilder
```

**ALWAYS** create a dedicated builder for multi-platform builds -- the default builder does not support multi-platform output.

---

## Image Filter Cheat Sheet

### docker images --filter

| Filter | Description | Example |
|--------|-------------|---------|
| `dangling=true` | Untagged images (no repo:tag) | `docker images -f dangling=true` |
| `label=key` | Images with specific label | `docker images -f label=maintainer` |
| `label=key=value` | Images with label matching value | `docker images -f label=app=web` |
| `before=image` | Created before given image | `docker images -f before=nginx:1.24` |
| `since=image` | Created after given image | `docker images -f since=nginx:1.24` |
| `reference=pattern` | Wildcard match on repo:tag | `docker images -f reference="ngin*:lat*"` |

### Image Format Placeholders

| Placeholder | Output |
|-------------|--------|
| `{{.ID}}` | Image ID |
| `{{.Repository}}` | Repository name |
| `{{.Tag}}` | Tag |
| `{{.Digest}}` | Content digest |
| `{{.CreatedSince}}` | Time since creation |
| `{{.CreatedAt}}` | Creation timestamp |
| `{{.Size}}` | Disk size |

```bash
# Compact image list
docker images --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}"

# JSON output (one per line)
docker images --format json

# Only IDs for scripting
docker images -q

# Remove all dangling images
docker rmi $(docker images -q -f dangling=true)
```

---

## Disk Cleanup Strategy

**ALWAYS follow this order -- from safest to most aggressive:**

1. **Assess** -- `docker system df -v`
2. **Containers** -- `docker container prune -f`
3. **Dangling images** -- `docker image prune -f`
4. **All unused images** -- `docker image prune -a -f`
5. **Build cache** -- `docker builder prune -f`
6. **Volumes** -- `docker volume ls -f dangling=true` then `docker volume prune -f`
7. **Nuclear** -- `docker system prune -a --volumes -f`

### What Each Prune Removes

| Command | Removes |
|---------|---------|
| `docker container prune` | All stopped containers |
| `docker image prune` | Dangling images only |
| `docker image prune -a` | ALL unused images |
| `docker network prune` | All unused networks |
| `docker volume prune` | All unused anonymous volumes |
| `docker builder prune` | Build cache |
| `docker system prune` | Containers + networks + dangling images + build cache |
| `docker system prune -a --volumes` | All of the above + all unused images + volumes |

### Prune Filter Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--filter "until=24h"` | Resources older than duration | `docker image prune -f --filter "until=24h"` |
| `--filter "label=temp"` | Resources with label | `docker system prune --filter "label=temp"` |
| `--filter "label!=keep"` | Resources without label | `docker image prune --filter "label!=keep"` |
| `-f` | Skip confirmation prompt | `docker system prune -f` |

---

## Decision Tree: Which Cleanup Command?

```
Need to free disk space?
├── Know what's using space? → docker system df -v
├── Just dangling/untagged images? → docker image prune -f
├── All unused images? → docker image prune -a -f
├── Stopped containers? → docker container prune -f
├── Build cache growing? → docker builder prune -f
├── Unused volumes? → docker volume prune -f (CHECK FIRST!)
└── Everything unused? → docker system prune -a --volumes -f (DANGEROUS on prod)
```

```
Need to transfer images offline?
├── Full image with layers + tags + history → docker save / docker load
├── Container filesystem snapshot (no metadata) → docker export / docker import
└── Multi-platform manifest → docker manifest create + push
```

```
Need multi-platform builds?
├── Single platform, local use → docker buildx build --platform linux/amd64 --load .
├── Multiple platforms, push to registry → docker buildx build --platform linux/amd64,linux/arm64 --push .
└── Need a builder first? → docker buildx create --name mp --use
```

---

## System Information Commands

### docker system df

```bash
docker system df              # Summary: images, containers, volumes, build cache
docker system df -v           # Verbose per-resource breakdown
docker system df --format "table {{.Type}}\t{{.TotalCount}}\t{{.Size}}\t{{.Reclaimable}}"
```

### docker info

```bash
docker info                                    # Full system information
docker info --format '{{.ServerVersion}}'      # Server version only
docker info --format '{{.Driver}}'             # Storage driver
docker info --format '{{json .Plugins}}'       # Available plugins
```

### docker version

```bash
docker version                                 # Client + server versions
docker version --format '{{.Server.Version}}'  # Server version only
docker version --format json                   # JSON output
```

### docker context

```bash
docker context ls                                                    # List all contexts
docker context create remote --docker "host=ssh://user@remote-host"  # Create SSH context
docker context use remote                                            # Switch to context
docker context use default                                           # Switch back
docker context inspect remote                                        # Show details
docker context rm remote                                             # Remove context
```

---

## Reference Links

- [references/commands.md](references/commands.md) -- Complete image and system command reference with all flags
- [references/examples.md](references/examples.md) -- Image management workflows and cleanup scripts
- [references/anti-patterns.md](references/anti-patterns.md) -- Common image management mistakes and how to avoid them

### Official Sources

- https://docs.docker.com/reference/cli/docker/image/
- https://docs.docker.com/reference/cli/docker/buildx/build/
- https://docs.docker.com/reference/cli/docker/image/ls/
- https://docs.docker.com/reference/cli/docker/system/
- https://docs.docker.com/reference/cli/docker/system/prune/
- https://docs.docker.com/reference/cli/docker/system/df/
