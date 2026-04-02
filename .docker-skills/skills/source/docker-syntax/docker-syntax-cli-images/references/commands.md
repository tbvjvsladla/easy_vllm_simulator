# Image & System Command Reference

> Complete flag reference for Docker image management and system commands.
> Source: https://docs.docker.com/reference/cli/docker/image/ and https://docs.docker.com/reference/cli/docker/system/

---

## docker buildx build

Syntax: `docker buildx build [OPTIONS] PATH | URL | -`

Default builder since Docker Engine 23+. ALWAYS use this instead of legacy `docker build`.

### All Flags

#### Core Build Flags

| Flag | Description | Default | Example |
|------|-------------|---------|---------|
| `-f, --file` | Dockerfile path | `Dockerfile` | `-f Dockerfile.prod` |
| `-t, --tag` | Name and optional tag (repeatable) | — | `-t myapp:v1` |
| `--target` | Build up to specific stage | — | `--target builder` |
| `--build-arg` | Build-time variable (repeatable) | — | `--build-arg NODE_ENV=prod` |
| `--no-cache` | Do not use cache | false | `--no-cache` |
| `--pull` | Always pull newer base images | false | `--pull` |
| `--build-context` | Additional named build contexts | — | `--build-context base=docker-image://alpine` |

#### Output Flags

| Flag | Description | Default | Example |
|------|-------------|---------|---------|
| `--load` | Load result into Docker images | — | `--load` |
| `--push` | Push result to registry | — | `--push` |
| `-o, --output` | Output destination | — | `-o type=local,dest=./out` |

**Output type reference:**

| Type | Description | Example |
|------|-------------|---------|
| `docker` | Load into local Docker image store | `-o type=docker` |
| `registry` | Push to container registry | `-o type=registry` |
| `local` | Export to local filesystem | `-o type=local,dest=./out` |
| `tar` | Export as tar archive | `-o type=tar,dest=image.tar` |
| `oci` | Export as OCI layout | `-o type=oci,dest=image.tar` |
| `image` | Build image (default) | `-o type=image` |

#### Multi-Platform Flags

| Flag | Description | Default | Example |
|------|-------------|---------|---------|
| `--platform` | Target platform(s), comma-separated | Host platform | `--platform linux/amd64,linux/arm64` |
| `--builder` | Override default builder instance | — | `--builder mybuilder` |

#### Cache Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--cache-from` | External cache source (repeatable) | `--cache-from type=local,src=./cache` |
| `--cache-to` | Cache export destination | `--cache-to type=local,dest=./cache` |

**Cache type reference:**

| Type | Description | Example |
|------|-------------|---------|
| `registry` | Cache in registry image | `type=registry,ref=user/app:cache` |
| `local` | Cache on local filesystem | `type=local,src=/tmp/cache` |
| `inline` | Embed cache in output image | `type=inline` |
| `gha` | GitHub Actions cache | `type=gha,mode=max` |
| `s3` | Amazon S3 cache | `type=s3,bucket=mybucket` |
| `azblob` | Azure Blob Storage cache | `type=azblob,name=mycache` |

#### Security & Secrets Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--secret` | Secret to expose to build (repeatable) | `--secret id=aws,src=$HOME/.aws/credentials` |
| `--ssh` | SSH agent socket/keys (repeatable) | `--ssh default=$SSH_AUTH_SOCK` |

Access in Dockerfile:
```dockerfile
RUN --mount=type=secret,id=mysecret cat /run/secrets/mysecret
RUN --mount=type=ssh git clone git@github.com:org/repo.git
```

#### Progress & Metadata Flags

| Flag | Description | Default | Example |
|------|-------------|---------|---------|
| `--progress` | Output type: auto/plain/tty/quiet/rawjson | auto | `--progress=plain` |
| `--metadata-file` | Write build result metadata to file | — | `--metadata-file meta.json` |

#### Attestation Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--provenance` | SLSA provenance attestation | `--provenance=mode=max` |
| `--sbom` | Software Bill of Materials | `--sbom` |
| `--attest` | Generic attestation | `--attest type=sbom` |

---

## docker buildx Management

### docker buildx create

```bash
docker buildx create --name mybuilder           # Create builder
docker buildx create --name mybuilder --use      # Create and switch to it
docker buildx create --driver docker-container   # Use docker-container driver
docker buildx create --platform linux/amd64,linux/arm64  # Set platforms
```

### docker buildx ls

```bash
docker buildx ls                                 # List all builders with status
```

### docker buildx use

```bash
docker buildx use mybuilder                      # Switch active builder
docker buildx use default                        # Switch back to default
```

### docker buildx inspect

```bash
docker buildx inspect                            # Inspect current builder
docker buildx inspect mybuilder                  # Inspect specific builder
docker buildx inspect --bootstrap                # Start builder if stopped
```

### docker buildx rm

```bash
docker buildx rm mybuilder                       # Remove builder
```

---

## docker pull

Syntax: `docker pull [OPTIONS] NAME[:TAG|@DIGEST]`

| Flag | Description | Example |
|------|-------------|---------|
| `--platform` | Target platform | `--platform linux/arm64` |
| `--all-tags` | Pull all tagged images | `--all-tags` |
| `-q, --quiet` | Suppress verbose output | `-q` |

```bash
docker pull nginx                      # Latest tag
docker pull nginx:1.25                 # Specific tag
docker pull nginx@sha256:abc123...     # Specific digest (immutable)
docker pull --platform linux/arm64 nginx  # Specific platform
docker pull --all-tags nginx           # All tags of repository
```

---

## docker push

Syntax: `docker push [OPTIONS] NAME[:TAG]`

| Flag | Description | Example |
|------|-------------|---------|
| `--all-tags` | Push all tags of repository | `--all-tags` |
| `-q, --quiet` | Suppress verbose output | `-q` |

```bash
docker push myregistry.com/myapp:v1      # Push specific tag
docker push --all-tags myregistry.com/myapp  # Push all tags
```

**ALWAYS** tag with full registry path before pushing: `docker tag myapp:v1 myregistry.com/myapp:v1`

---

## docker tag

Syntax: `docker tag SOURCE_IMAGE[:TAG] TARGET_IMAGE[:TAG]`

```bash
docker tag nginx:latest myregistry.com/nginx:v1
docker tag abc123 myregistry.com/myapp:latest
docker tag myapp:v1 myapp:latest
```

---

## docker images / docker image ls

Syntax: `docker images [OPTIONS] [REPOSITORY[:TAG]]`

| Flag | Description |
|------|-------------|
| `-a, --all` | Show all images including intermediate layers |
| `--digests` | Show content digests |
| `-f, --filter` | Filter output (see filter table in SKILL.md) |
| `--format` | Custom output (Go template, `table`, `json`) |
| `--no-trunc` | Full image IDs |
| `-q, --quiet` | Image IDs only |
| `--tree` | Multi-platform tree view (experimental, API 1.47+) |

---

## docker rmi

Syntax: `docker rmi [OPTIONS] IMAGE [IMAGE...]`

| Flag | Description |
|------|-------------|
| `-f, --force` | Force remove even if containers use the image |
| `--no-prune` | Do not delete untagged parent images |

```bash
docker rmi nginx:old                   # Remove by tag
docker rmi -f abc123                   # Force remove by ID
docker rmi $(docker images -q -f dangling=true)  # Remove all dangling
```

---

## docker image prune

Syntax: `docker image prune [OPTIONS]`

| Flag | Description |
|------|-------------|
| `-a, --all` | Remove all unused images, not just dangling |
| `-f, --force` | Skip confirmation prompt |
| `--filter` | Filter (until, label) |

```bash
docker image prune                     # Dangling only
docker image prune -a                  # All unused
docker image prune -f --filter "until=24h"  # Older than 24h
docker image prune --filter "label!=keep"   # Without "keep" label
```

---

## docker save

Syntax: `docker save [OPTIONS] IMAGE [IMAGE...]`

Exports full image with all layers, tags, and history. Use for offline transfer.

| Flag | Description |
|------|-------------|
| `-o, --output` | Write to file instead of STDOUT |

```bash
docker save -o backup.tar nginx:latest
docker save nginx:latest > backup.tar
docker save nginx:latest redis:latest > multi.tar   # Multiple images
```

---

## docker load

Syntax: `docker load [OPTIONS]`

Imports image from tar archive created by `docker save`.

| Flag | Description |
|------|-------------|
| `-i, --input` | Read from file instead of STDIN |
| `-q, --quiet` | Suppress load output |

```bash
docker load -i backup.tar
docker load < backup.tar
docker load -q -i backup.tar
```

---

## docker history

Syntax: `docker image history [OPTIONS] IMAGE`

Shows the layer history of an image.

| Flag | Description |
|------|-------------|
| `--no-trunc` | Show full commands (not truncated) |
| `--format` | Custom output format |
| `-q, --quiet` | Layer IDs only |
| `-H, --human` | Human-readable sizes (default true) |

```bash
docker history nginx
docker history --no-trunc nginx
docker history --format "{{.CreatedBy}}" nginx
docker history -q nginx
```

---

## docker manifest

Multi-platform manifest list management.

### docker manifest inspect

```bash
docker manifest inspect nginx:latest
docker manifest inspect --verbose nginx:latest
```

### docker manifest create

```bash
docker manifest create myapp:latest myapp:amd64 myapp:arm64
docker manifest create --amend myapp:latest myapp:amd64  # Amend existing
```

### docker manifest annotate

```bash
docker manifest annotate myapp:latest myapp:arm64 --os linux --arch arm64
```

### docker manifest push

```bash
docker manifest push myapp:latest
docker manifest push --purge myapp:latest  # Remove local after push
```

---

## docker system df

Syntax: `docker system df [OPTIONS]`

| Flag | Description |
|------|-------------|
| `-v, --verbose` | Per-resource detailed breakdown |
| `--format` | Custom output format |

Output fields: TYPE, TOTAL, ACTIVE, SIZE, RECLAIMABLE.

Verbose mode adds per-image details: Repository, Tag, Image ID, Created, Size, Shared Size, Unique Size, Containers.

---

## docker system prune

Syntax: `docker system prune [OPTIONS]`

| Flag | Description |
|------|-------------|
| `-a, --all` | Also remove all unused images (not just dangling) |
| `--volumes` | Also remove anonymous volumes |
| `-f, --force` | Skip confirmation prompt |
| `--filter` | Provide filter values (until, label) |

---

## docker info

Syntax: `docker info [OPTIONS]`

| Flag | Description |
|------|-------------|
| `--format` | Format output using Go template |

Shows: server version, storage driver, logging driver, cgroup driver, kernel version, OS, architecture, CPUs, memory, registry config, security options, runtime.

---

## docker version

Syntax: `docker version [OPTIONS]`

| Flag | Description |
|------|-------------|
| `--format` | Format using Go template or `json` |

Shows both client and server version details including API version, Go version, OS/Arch.

---

## docker context

### docker context create

```bash
docker context create myctx --docker "host=ssh://user@host"
docker context create myctx --docker "host=tcp://host:2376,ca=ca.pem,cert=cert.pem,key=key.pem"
```

### docker context ls

```bash
docker context ls
docker context ls --format "{{.Name}}: {{.DockerEndpoint}}"
```

### docker context use

```bash
docker context use myctx
docker context use default
```

### docker context inspect

```bash
docker context inspect myctx
docker context inspect --format '{{.Endpoints.docker.Host}}' myctx
```

### docker context rm

```bash
docker context rm myctx
docker context rm -f myctx   # Force
```

---

## Official Sources

- https://docs.docker.com/reference/cli/docker/image/
- https://docs.docker.com/reference/cli/docker/image/ls/
- https://docs.docker.com/reference/cli/docker/buildx/build/
- https://docs.docker.com/reference/cli/docker/system/
- https://docs.docker.com/reference/cli/docker/system/df/
- https://docs.docker.com/reference/cli/docker/system/prune/
