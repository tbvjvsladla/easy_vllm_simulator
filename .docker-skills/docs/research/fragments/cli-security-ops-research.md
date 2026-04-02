# Docker CLI, Security, Networking, Storage & Errors Research

> **Research Date**: 2026-03-19
> **Sources**: Official Docker documentation (docs.docker.com), verified via WebFetch
> **Scope**: Docker Engine 24+, Docker Compose v2
> **Purpose**: Vooronderzoek Part 3 for Docker Skill Package

---

## Table of Contents

1. [Container Lifecycle Commands](#1-container-lifecycle-commands)
2. [Image Management Commands](#2-image-management-commands)
3. [Network Management](#3-network-management)
4. [Volume & Storage Management](#4-volume--storage-management)
5. [Security](#5-security)
6. [Go Template Formatting](#6-go-template-formatting)
7. [Common Errors & Troubleshooting](#7-common-errors--troubleshooting)
8. [System & Maintenance](#8-system--maintenance)

---

## 1. Container Lifecycle Commands

**Source**: https://docs.docker.com/reference/cli/docker/container/
**Source**: https://docs.docker.com/reference/cli/docker/container/run/

### 1.1 docker run — Create and Run a Container

Full syntax: `docker run [OPTIONS] IMAGE [COMMAND] [ARG...]`

#### Core Execution Flags

| Flag | Description | Default | Example |
|------|-------------|---------|---------|
| `-d, --detach` | Run in background, print container ID | — | `docker run -d nginx` |
| `-i, --interactive` | Keep STDIN open | — | `docker run -i ubuntu cat` |
| `-t, --tty` | Allocate pseudo-TTY | — | `docker run -it ubuntu bash` |
| `--rm` | Auto-remove container on exit | — | `docker run --rm ubuntu echo hello` |
| `--name` | Assign container name | Auto-generated | `docker run --name web nginx` |
| `--init` | Run init process (forwards signals, reaps zombies) | — | `docker run --init nginx` |

#### Port & Network Flags

| Flag | Description | Example |
|------|-------------|---------|
| `-p, --publish` | Publish port `[host-ip:]host-port:container-port[/proto]` | `docker run -p 8080:80 nginx` |
| `-P, --publish-all` | Publish all exposed ports to random host ports | `docker run -P nginx` |
| `--network` | Connect to network (bridge, host, none, custom) | `docker run --network mynet nginx` |
| `--network-alias` | Add DNS alias on the network | `docker run --network mynet --network-alias web nginx` |
| `--ip` | Static IPv4 address | `docker run --ip 172.20.0.5 nginx` |
| `--ip6` | Static IPv6 address | `docker run --ip6 2001:db8::33 nginx` |
| `--dns` | Custom DNS server | `docker run --dns 8.8.8.8 nginx` |
| `--dns-search` | Custom DNS search domain | `docker run --dns-search example.com nginx` |
| `--hostname, -h` | Set container hostname | `docker run -h myhost nginx` |
| `--add-host` | Add host-to-IP mapping to /etc/hosts | `docker run --add-host myhost=8.8.8.8 nginx` |
| `--mac-address` | Set MAC address | `docker run --mac-address 92:d0:c6:0a:29:33 nginx` |
| `--expose` | Expose port (documentation only, no host binding) | `docker run --expose 80 nginx` |
| `--link` | **LEGACY** — Link to another container | `docker run --link redis:db nginx` |

#### Storage & Volume Flags

| Flag | Description | Example |
|------|-------------|---------|
| `-v, --volume` | Bind mount or named volume | `docker run -v mydata:/data nginx` |
| `--mount` | Declarative mount (preferred) | `docker run --mount type=volume,src=mydata,dst=/data nginx` |
| `--volumes-from` | Mount volumes from another container | `docker run --volumes-from web:ro app` |
| `--read-only` | Read-only root filesystem | `docker run --read-only nginx` |
| `--tmpfs` | Mount tmpfs (in-memory) | `docker run --tmpfs /run:size=64k nginx` |

#### Environment & Configuration Flags

| Flag | Description | Example |
|------|-------------|---------|
| `-e, --env` | Set environment variable | `docker run -e DB_HOST=db nginx` |
| `--env-file` | Read env vars from file | `docker run --env-file .env nginx` |
| `-w, --workdir` | Set working directory | `docker run -w /app node npm start` |
| `--entrypoint` | Override ENTRYPOINT | `docker run --entrypoint /bin/sh nginx` |
| `-u, --user` | Run as user (name or UID[:GID]) | `docker run -u 1000:1000 ubuntu whoami` |
| `-l, --label` | Set metadata label | `docker run -l app=web nginx` |
| `--label-file` | Read labels from file | `docker run --label-file labels.txt nginx` |

#### Resource Limit Flags

| Flag | Description | Example |
|------|-------------|---------|
| `-m, --memory` | Memory limit | `docker run -m 512m nginx` |
| `--memory-reservation` | Memory soft limit | `docker run --memory-reservation 256m nginx` |
| `--memory-swap` | Total memory + swap limit | `docker run --memory-swap 1g nginx` |
| `--cpus` | Number of CPUs | `docker run --cpus 1.5 nginx` |
| `-c, --cpu-shares` | CPU shares (relative weight) | `docker run -c 1024 nginx` |
| `--cpuset-cpus` | Pin to specific CPUs | `docker run --cpuset-cpus 0-3 nginx` |
| `--pids-limit` | Max PIDs in container | `docker run --pids-limit 200 nginx` |
| `--ulimit` | Set ulimit | `docker run --ulimit nofile=1024:2048 nginx` |
| `--shm-size` | /dev/shm size | `docker run --shm-size 1gb nginx` |
| `--blkio-weight` | Block I/O weight (10-1000) | `docker run --blkio-weight 300 nginx` |
| `--device-read-bps` | Limit device read rate | `docker run --device-read-bps /dev/sda:1mb nginx` |
| `--device-write-bps` | Limit device write rate | `docker run --device-write-bps /dev/sda:1mb nginx` |
| `--oom-kill-disable` | Disable OOM killer | `docker run --oom-kill-disable nginx` |

#### Security & Privilege Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--privileged` | Full host privileges (**dangerous**) | `docker run --privileged ubuntu` |
| `--cap-add` | Add Linux capability | `docker run --cap-add SYS_PTRACE ubuntu` |
| `--cap-drop` | Drop Linux capability | `docker run --cap-drop NET_RAW ubuntu` |
| `--security-opt` | Security options (AppArmor, SELinux, seccomp) | `docker run --security-opt no-new-privileges=true ubuntu` |
| `--device` | Add host device | `docker run --device /dev/sda:/dev/xvdc nginx` |
| `--gpus` | Add GPU devices | `docker run --gpus all ubuntu nvidia-smi` |
| `--pid` | PID namespace (host or container) | `docker run --pid=host ubuntu ps aux` |
| `--ipc` | IPC namespace mode | `docker run --ipc host ubuntu` |
| `--userns` | User namespace mode | `docker run --userns host hello-world` |
| `--cgroupns` | Cgroup namespace (host or private) | `docker run --cgroupns private ubuntu` |

#### Health Check Flags

| Flag | Description | Default | Example |
|------|-------------|---------|---------|
| `--health-cmd` | Health check command | — | `docker run --health-cmd='curl -f http://localhost/' nginx` |
| `--health-interval` | Check interval | 0s | `docker run --health-interval 30s nginx` |
| `--health-timeout` | Check timeout | 0s | `docker run --health-timeout 10s nginx` |
| `--health-retries` | Consecutive failures for unhealthy | — | `docker run --health-retries 3 nginx` |
| `--health-start-period` | Grace period for init | 0s | `docker run --health-start-period 40s nginx` |
| `--health-start-interval` | Check interval during start period | 0s | `docker run --health-start-interval 5s nginx` |
| `--no-healthcheck` | Disable health check | — | `docker run --no-healthcheck nginx` |

#### Restart Policy Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--restart no` | Never restart (default) | `docker run --restart no nginx` |
| `--restart always` | Always restart | `docker run --restart always nginx` |
| `--restart unless-stopped` | Restart unless manually stopped | `docker run --restart unless-stopped nginx` |
| `--restart on-failure[:N]` | Restart on non-zero exit, optional max retries | `docker run --restart on-failure:5 nginx` |

#### Logging Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--log-driver` | Logging driver | `docker run --log-driver json-file nginx` |
| `--log-opt` | Log driver options | `docker run --log-opt max-size=10m --log-opt max-file=3 nginx` |

#### Image & Pull Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--pull` | Pull policy: `missing` (default), `always`, `never` | `docker run --pull always nginx` |
| `--platform` | Target platform | `docker run --platform linux/amd64 nginx` |

#### Signal Flags

| Flag | Description | Default | Example |
|------|-------------|---------|---------|
| `--stop-signal` | Signal to stop container | SIGTERM | `docker run --stop-signal SIGKILL nginx` |
| `--stop-timeout` | Timeout before force kill | 10s | `docker run --stop-timeout 30 nginx` |
| `--sig-proxy` | Proxy signals to process | true | `docker run --sig-proxy=false nginx` |

### 1.2 docker create

Syntax: `docker create [OPTIONS] IMAGE [COMMAND] [ARG...]`

Same flags as `docker run`, but creates the container without starting it. Use `docker start` to begin execution.

```bash
docker create --name myapp -p 8080:80 nginx
docker start myapp
```

### 1.3 docker start / stop / restart / kill

```bash
# Start one or more stopped containers
docker start [OPTIONS] CONTAINER [CONTAINER...]
docker start -a myapp          # Attach STDOUT/STDERR and forward signals
docker start -i myapp          # Attach container's STDIN

# Stop (sends SIGTERM, then SIGKILL after grace period)
docker stop [OPTIONS] CONTAINER [CONTAINER...]
docker stop -t 30 myapp        # 30-second grace period (default 10s)

# Restart (stop then start)
docker restart [OPTIONS] CONTAINER [CONTAINER...]
docker restart -t 5 myapp      # 5-second grace period

# Kill (sends signal immediately, default SIGKILL)
docker kill [OPTIONS] CONTAINER [CONTAINER...]
docker kill -s SIGTERM myapp   # Send specific signal
```

### 1.4 docker rm / container prune

```bash
# Remove one or more containers
docker rm [OPTIONS] CONTAINER [CONTAINER...]
docker rm myapp                 # Remove stopped container
docker rm -f myapp              # Force remove (sends SIGKILL first)
docker rm -v myapp              # Also remove anonymous volumes
docker rm -l /webapp/db         # Remove link only

# Remove ALL stopped containers
docker container prune [OPTIONS]
docker container prune -f       # No confirmation prompt
docker container prune --filter "until=24h"   # Only containers older than 24h
docker container prune --filter "label=temp"  # Only containers with label
```

### 1.5 docker exec — Execute Command in Running Container

Syntax: `docker exec [OPTIONS] CONTAINER COMMAND [ARG...]`

| Flag | Description |
|------|-------------|
| `-d, --detach` | Run in background |
| `-e, --env` | Set environment variables (API 1.25+) |
| `--env-file` | Load env vars from file (API 1.25+) |
| `-i, --interactive` | Keep STDIN open |
| `-t, --tty` | Allocate pseudo-TTY |
| `-u, --user` | Run as username/UID |
| `-w, --workdir` | Working directory (API 1.35+) |
| `--privileged` | Extended privileges |

```bash
# Interactive shell
docker exec -it myapp bash
docker exec -it myapp sh

# Run command
docker exec myapp ls /app

# Background task
docker exec -d myapp touch /tmp/marker

# With environment
docker exec -e DEBUG=1 myapp env

# As different user
docker exec -u root myapp apt-get update

# Different working directory
docker exec -it -w /var/log myapp ls
```

**Important**: The command must be an executable. Chained commands require shell:
```bash
# ❌ WRONG — chained commands fail
docker exec myapp echo a && echo b

# ✅ CORRECT — wrap in shell
docker exec myapp sh -c "echo a && echo b"
```

**Limitation**: Cannot exec into a paused container — unpause first.

### 1.6 docker attach

```bash
# Attach to running container's STDIN/STDOUT/STDERR
docker attach [OPTIONS] CONTAINER
docker attach myapp

# Detach with key sequence (default: Ctrl+P, Ctrl+Q)
docker attach --detach-keys="ctrl-c" myapp

# Without signal proxy
docker attach --sig-proxy=false myapp
```

### 1.7 docker logs

Syntax: `docker logs [OPTIONS] CONTAINER`

| Flag | Default | Description |
|------|---------|-------------|
| `-f, --follow` | — | Stream live output |
| `-n, --tail` | all | Number of lines from end |
| `-t, --timestamps` | — | Show RFC3339Nano timestamps |
| `--since` | — | Show logs after timestamp/duration |
| `--until` | — | Show logs before timestamp/duration (API 1.35+) |
| `--details` | — | Show extra metadata (env vars) |

```bash
# Last 100 lines
docker logs --tail 100 myapp

# Follow live output
docker logs -f myapp

# Logs from last 30 minutes
docker logs --since 30m myapp

# Logs with timestamps
docker logs -t myapp

# Logs in a time range
docker logs --since 2024-01-01T00:00:00Z --until 2024-01-02T00:00:00Z myapp

# Follow with tail
docker logs -f --tail 50 myapp
```

**Timestamp formats**: RFC 3339 dates, Unix timestamps, Go duration strings (`1m30s`, `3h`).

### 1.8 docker inspect

```bash
# Inspect any Docker object
docker inspect [OPTIONS] NAME|ID [NAME|ID...]
docker inspect myapp
docker inspect --type container myapp     # Restrict to containers
docker inspect --type image nginx         # Restrict to images
docker inspect -s myapp                   # Include size info

# With Go template formatting (see Section 6)
docker inspect --format='{{.State.Status}}' myapp
```

### 1.9 docker stats

```bash
# Live resource usage for all running containers
docker stats

# Specific containers
docker stats myapp db redis

# Single snapshot (no streaming)
docker stats --no-stream

# Custom format
docker stats --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}"
```

### 1.10 docker top

```bash
# Display running processes in a container
docker top CONTAINER [ps OPTIONS]
docker top myapp
docker top myapp aux
```

### 1.11 docker events (system-wide)

```bash
# Real-time events from Docker daemon
docker events
docker events --since '2024-01-01'
docker events --until '10m'
docker events --filter 'event=start'
docker events --filter 'type=container'
docker events --filter 'container=myapp'
docker events --format '{{json .}}'
```

### 1.12 docker wait

```bash
# Block until container stops, print exit code
docker wait CONTAINER [CONTAINER...]
docker wait myapp
EXIT_CODE=$(docker wait myapp)
```

### 1.13 docker cp

```bash
# Copy files between container and host
docker cp [OPTIONS] CONTAINER:SRC_PATH DEST_PATH
docker cp [OPTIONS] SRC_PATH CONTAINER:DEST_PATH

# Copy from container to host
docker cp myapp:/app/log.txt ./log.txt

# Copy from host to container
docker cp ./config.yml myapp:/app/config.yml

# Copy with archive mode (preserves UID/GID)
docker cp -a myapp:/app/data ./backup/
```

### 1.14 docker diff

```bash
# Show filesystem changes in container
docker diff CONTAINER
docker diff myapp
# Output: A = Added, C = Changed, D = Deleted
```

### 1.15 docker commit

```bash
# Create image from container changes
docker commit [OPTIONS] CONTAINER [REPOSITORY[:TAG]]
docker commit myapp myimage:v1
docker commit -m "Added config" -a "author" myapp myimage:v1
docker commit --change='CMD ["nginx"]' myapp myimage:v1
docker commit --change='EXPOSE 80' myapp myimage:v1
```

### 1.16 docker export / import

```bash
# Export container filesystem as tar
docker export CONTAINER > container.tar
docker export -o container.tar myapp

# Import tar as image (NOT a full Docker image — no history/metadata)
docker import container.tar myimage:v1
docker import https://example.com/image.tar myimage:v1
cat container.tar | docker import - myimage:v1
```

### 1.17 docker rename

```bash
docker rename CONTAINER NEW_NAME
docker rename old_name new_name
```

### 1.18 docker update

```bash
# Update container configuration (resource limits, restart policy)
docker update [OPTIONS] CONTAINER [CONTAINER...]
docker update --memory 512m myapp
docker update --cpus 2 myapp
docker update --restart always myapp
docker update --memory 256m --memory-swap 512m myapp
docker update --pids-limit 100 myapp
```

### 1.19 docker container ls / docker ps

Syntax: `docker ps [OPTIONS]` or `docker container ls [OPTIONS]`

| Flag | Default | Description |
|------|---------|-------------|
| `-a, --all` | — | Show all containers (not just running) |
| `-f, --filter` | — | Filter output |
| `--format` | — | Custom output format |
| `-n, --last` | -1 | Show n last created |
| `-l, --latest` | — | Show latest created |
| `--no-trunc` | — | Full output (no truncation) |
| `-q, --quiet` | — | Container IDs only |
| `-s, --size` | — | Display file sizes |

#### Filter Options

| Filter | Description | Example |
|--------|-------------|---------|
| `id` | Container ID | `--filter id=abc123` |
| `name` | Container name (substring match) | `--filter name=web` |
| `label` | Label key or key=value | `--filter label=app=web` |
| `exited` | Exit code (use with `-a`) | `--filter exited=0` |
| `status` | created/restarting/running/removing/paused/exited/dead | `--filter status=running` |
| `ancestor` | Image (name, tag, digest, ID) | `--filter ancestor=nginx:latest` |
| `before` | Created before container | `--filter before=myapp` |
| `since` | Created since container | `--filter since=myapp` |
| `volume` | Volume name or mount point | `--filter volume=mydata` |
| `network` | Network name or ID | `--filter network=mynet` |
| `publish` | Published port | `--filter publish=80/tcp` |
| `expose` | Exposed port | `--filter expose=8080` |
| `health` | starting/healthy/unhealthy/none | `--filter health=healthy` |
| `is-task` | Service task (true/false) | `--filter is-task=false` |

#### Format Placeholders

| Placeholder | Output |
|-------------|--------|
| `{{.ID}}` | Container ID |
| `{{.Image}}` | Image ID |
| `{{.Command}}` | Command |
| `{{.CreatedAt}}` | Creation time |
| `{{.RunningFor}}` | Elapsed time |
| `{{.Ports}}` | Port mappings |
| `{{.State}}` | Status (created/running/exited) |
| `{{.Status}}` | Detailed status with health |
| `{{.Size}}` | Disk usage |
| `{{.Names}}` | Container names |
| `{{.Labels}}` | All labels |
| `{{.Label "key"}}` | Specific label value |
| `{{.Mounts}}` | Volume names |
| `{{.Networks}}` | Network names |

```bash
# Running containers
docker ps

# All containers
docker ps -a

# Custom format
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

# JSON output
docker ps --format json

# Filter by status
docker ps -a --filter status=exited

# Only IDs (useful for scripting)
docker ps -q

# Remove all stopped containers
docker rm $(docker ps -aq --filter status=exited)
```

### 1.20 docker port

```bash
# List port mappings for a container
docker port CONTAINER [PRIVATE_PORT[/PROTO]]
docker port myapp
docker port myapp 80/tcp
```

### 1.21 docker pause / unpause

```bash
docker pause CONTAINER [CONTAINER...]
docker unpause CONTAINER [CONTAINER...]
```

---

## 2. Image Management Commands

**Source**: https://docs.docker.com/reference/cli/docker/image/
**Source**: https://docs.docker.com/reference/cli/docker/buildx/build/

### 2.1 docker buildx build (Modern BuildKit Builder)

Syntax: `docker buildx build [OPTIONS] PATH | URL | -`

This is the DEFAULT builder since Docker Engine 23+. The legacy `docker build` uses pre-BuildKit backend and is only for Windows container mode or when `DOCKER_BUILDKIT=0`.

#### Core Build Flags

| Flag | Description | Example |
|------|-------------|---------|
| `-f, --file` | Dockerfile path | `docker buildx build -f Dockerfile.prod .` |
| `-t, --tag` | Tag the image | `docker buildx build -t myapp:v1 .` |
| `--target` | Build specific stage | `docker buildx build --target build-env .` |
| `--build-arg` | Build-time variable | `docker buildx build --build-arg NODE_ENV=prod .` |
| `--no-cache` | Disable build cache | `docker buildx build --no-cache .` |
| `--pull` | Always pull base images | `docker buildx build --pull .` |

#### Output Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--push` | Push to registry (shorthand for `--output=type=registry`) | `docker buildx build -t user/app --push .` |
| `--load` | Load into local Docker (shorthand for `--output=type=docker`) | `docker buildx build --load .` |
| `-o, --output` | Custom output destination | `docker buildx build -o type=local,dest=./out .` |

**Output types**: `docker` (local store), `registry` (push), `local` (filesystem), `tar`, `oci`, `image`.

```bash
# Export to local directory
docker buildx build -o . .

# Export as tar
docker buildx build -o - . > image.tar

# Export as OCI
docker buildx build -o type=oci,dest=image.tar .
```

#### Multi-Platform Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--platform` | Target platform(s) | `docker buildx build --platform linux/amd64,linux/arm64 .` |
| `--builder` | Use specific builder instance | `docker buildx build --builder mybuilder .` |

```bash
# Multi-platform build (requires push or output, not load)
docker buildx build --platform linux/amd64,linux/arm64,linux/arm/v7 -t user/app --push .

# Single platform (can use --load)
docker buildx build --platform linux/arm64 --load .
```

#### Cache Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--cache-from` | Import cache | `docker buildx build --cache-from type=local,src=./cache .` |
| `--cache-to` | Export cache | `docker buildx build --cache-to type=local,dest=./cache .` |

**Cache types**: `registry`, `local`, `inline`, `gha` (GitHub Actions), `s3`, `azblob`.

```bash
# Registry cache
docker buildx build --cache-from=user/app:cache --cache-to=user/app:cache .

# Local cache
docker buildx build --cache-from=type=local,src=/tmp/cache --cache-to=type=local,dest=/tmp/cache .

# GitHub Actions cache
docker buildx build --cache-from=type=gha --cache-to=type=gha,mode=max .

# Inline cache (embedded in image)
docker buildx build --cache-to=type=inline .
```

#### Security & Secrets Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--secret` | Expose secret to build | `docker buildx build --secret id=aws,src=$HOME/.aws/credentials .` |
| `--ssh` | Expose SSH agent/keys | `docker buildx build --ssh default=$SSH_AUTH_SOCK .` |

```bash
# Secret from file
docker buildx build --secret id=mysecret,src=./secret.txt .

# Secret from environment variable
SECRET_TOKEN=abc123 docker buildx build --secret id=SECRET_TOKEN .

# SSH for private repos
eval $(ssh-agent)
ssh-add ~/.ssh/id_rsa
docker buildx build --ssh default .
```

In Dockerfile, access secrets and SSH:
```dockerfile
# Access secret
RUN --mount=type=secret,id=mysecret cat /run/secrets/mysecret

# Access SSH
RUN --mount=type=ssh git clone git@github.com:org/repo.git
```

#### Progress & Metadata Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--progress` | Output format: auto/plain/tty/quiet/rawjson | `docker buildx build --progress=plain .` |
| `--metadata-file` | Write build metadata as JSON | `docker buildx build --metadata-file meta.json .` |
| `--build-context` | Additional named build contexts | `docker buildx build --build-context base=docker-image://alpine .` |

#### Attestation Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--provenance` | SLSA provenance attestation | `docker buildx build --provenance=mode=max --push .` |
| `--sbom` | Software Bill of Materials | `docker buildx build --sbom --push .` |
| `--attest` | Generic attestation | `docker buildx build --attest type=sbom .` |

### 2.2 docker pull / push / tag

```bash
# Pull image from registry
docker pull [OPTIONS] NAME[:TAG|@DIGEST]
docker pull nginx                     # Latest tag
docker pull nginx:1.25                # Specific tag
docker pull nginx@sha256:abc123...    # Specific digest
docker pull --platform linux/arm64 nginx  # Specific platform
docker pull --all-tags nginx          # All tags

# Push image to registry
docker push [OPTIONS] NAME[:TAG]
docker push myregistry.com/myapp:v1
docker push --all-tags myregistry.com/myapp

# Tag image
docker tag SOURCE_IMAGE[:TAG] TARGET_IMAGE[:TAG]
docker tag nginx:latest myregistry.com/nginx:v1
docker tag abc123 myregistry.com/myapp:latest
```

### 2.3 docker image ls / docker images

Syntax: `docker images [OPTIONS] [REPOSITORY[:TAG]]`

| Flag | Description |
|------|-------------|
| `-a, --all` | Show all images (including intermediate layers) |
| `--digests` | Show content digests |
| `-f, --filter` | Filter output |
| `--format` | Custom output (table/json/Go template) |
| `--no-trunc` | Full image IDs |
| `-q, --quiet` | Image IDs only |
| `--tree` | Multi-platform tree view (experimental, API 1.47+) |

#### Filter Options

| Filter | Description | Example |
|--------|-------------|---------|
| `dangling` | Untagged images (true/false) | `--filter dangling=true` |
| `label` | By label key or key=value | `--filter label=maintainer=me` |
| `before` | Created before image | `--filter before=nginx:1.24` |
| `since` | Created since image | `--filter since=nginx:1.24` |
| `reference` | Wildcard reference pattern | `--filter reference="ngin*:lat*"` |

#### Format Placeholders

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
# List all images
docker images

# Filter dangling images
docker images -f dangling=true

# Custom format
docker images --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}"

# JSON output
docker images --format json

# Only IDs (for scripting)
docker images -q

# Images from specific repo
docker images nginx

# Remove all dangling images
docker rmi $(docker images -q -f dangling=true)
```

### 2.4 docker rmi / image prune

```bash
# Remove one or more images
docker rmi [OPTIONS] IMAGE [IMAGE...]
docker rmi nginx:latest
docker rmi -f abc123              # Force remove (even if tagged)
docker rmi --no-prune abc123      # Don't remove untagged parents

# Remove unused images
docker image prune [OPTIONS]
docker image prune                # Remove dangling images only
docker image prune -a             # Remove ALL unused images
docker image prune -f             # No confirmation
docker image prune --filter "until=24h"   # Older than 24h
docker image prune --filter "label!=keep" # Without "keep" label
```

### 2.5 docker save / load

```bash
# Save image(s) to tar archive (preserves layers, tags, history)
docker save [OPTIONS] IMAGE [IMAGE...]
docker save -o backup.tar nginx:latest
docker save nginx:latest > backup.tar
docker save nginx:latest redis:latest > multi.tar

# Load image from tar archive
docker load [OPTIONS]
docker load -i backup.tar
docker load < backup.tar
docker load -q -i backup.tar     # Quiet mode
```

### 2.6 docker image history

```bash
# Show image layer history
docker image history [OPTIONS] IMAGE
docker history nginx
docker history --no-trunc nginx   # Full commands
docker history --format "{{.CreatedBy}}" nginx
docker history -q nginx           # Layer IDs only
```

### 2.7 docker manifest (Multi-platform)

```bash
# Inspect manifest list
docker manifest inspect nginx:latest

# Create manifest list
docker manifest create myapp:latest myapp:amd64 myapp:arm64

# Push manifest list
docker manifest push myapp:latest
```

---

## 3. Network Management

**Source**: https://docs.docker.com/engine/network/
**Source**: https://docs.docker.com/engine/network/drivers/bridge/
**Source**: https://docs.docker.com/engine/network/drivers/overlay/

### 3.1 Network Drivers Overview

| Driver | Description | Use Case |
|--------|-------------|----------|
| **bridge** | Default. Isolated network on single host | Single-host container communication |
| **host** | Remove network isolation, share host network | Performance-critical applications |
| **none** | No networking | Fully isolated containers |
| **overlay** | Multi-host networking via Swarm | Cross-host service communication |
| **macvlan** | Containers appear as physical devices on LAN | Direct LAN integration |
| **ipvlan** | Connect to external VLANs | VLAN integration |

### 3.2 Bridge Networking

#### Default Bridge vs User-Defined Bridge

| Feature | Default Bridge | User-Defined Bridge |
|---------|---------------|-------------------|
| DNS resolution | ❌ IP only (or legacy `--link`) | ✅ Automatic by container name |
| Isolation | ❌ All unspecified containers join | ✅ Only explicitly connected containers |
| Live connect/disconnect | ❌ Requires container recreation | ✅ On-the-fly with `docker network connect` |
| Configuration | ❌ Shared settings, daemon restart needed | ✅ Per-network, independent config |
| Env var sharing | Via `--link` only | Via volumes, compose, or secrets |

> **ALWAYS use user-defined bridge networks.** The default bridge is legacy and lacks DNS resolution, proper isolation, and configuration flexibility.

#### Bridge Driver Options (`-o`)

| Option | Default | Description |
|--------|---------|-------------|
| `com.docker.network.bridge.name` | — | Linux bridge name |
| `com.docker.network.bridge.enable_ip_masquerade` | true | Enable NAT for outbound traffic |
| `com.docker.network.bridge.enable_icc` | true | Inter-container connectivity |
| `com.docker.network.bridge.host_binding_ipv4` | all | Default IP for port binding |
| `com.docker.network.driver.mtu` | 0 (no limit) | Maximum Transmission Unit |
| `com.docker.network.container_iface_prefix` | eth | Container interface prefix |
| `com.docker.network.bridge.inhibit_ipv4` | false | Skip IPv4 gateway |

**Scalability limit**: Bridge networks become unstable at 1000+ containers per network (Linux kernel limitation).

### 3.3 Overlay Networking

Requires Docker Swarm mode. Spans multiple Docker hosts.

```bash
# Create overlay network
docker network create -d overlay my-overlay

# Attachable overlay (standalone containers + services)
docker network create -d overlay --attachable my-overlay

# Encrypted overlay (IPsec at VXLAN level)
docker network create --opt encrypted -d overlay --attachable secure-overlay
```

**Required ports** between Swarm hosts:
- **2377/tcp** — Swarm control plane
- **4789/udp** — Overlay traffic (VXLAN)
- **7946/tcp+udp** — Node communication

**Limitation**: Windows containers cannot use encrypted overlay networks.
**Scalability**: Same 1000-container limit per single host applies.

### 3.4 Host Networking

```bash
# Container shares host's network namespace
docker run --network host nginx
```

No port mapping needed — container binds directly to host ports.

### 3.5 None Network

```bash
# Completely isolated — no network
docker run --network none alpine
```

### 3.6 Macvlan Networking

```bash
# Container appears as physical device on LAN
docker network create -d macvlan \
  --subnet=192.168.1.0/24 \
  --gateway=192.168.1.1 \
  -o parent=eth0 \
  my-macvlan
```

### 3.7 Network Management Commands

```bash
# Create network
docker network create [OPTIONS] NETWORK
docker network create -d bridge --subnet=172.28.0.0/16 --gateway=172.28.0.1 mynet
docker network create --ipv6 --subnet 2001:db8::/64 v6net
docker network create --ipv4=false --ipv6 v6only     # IPv6-only

# Advanced bridge with custom IP range
docker network create --driver=bridge \
  --subnet=172.28.0.0/16 \
  --ip-range=172.28.5.0/24 \
  --gateway=172.28.5.254 \
  mynet

# Internal network (no external access)
docker network create --internal isolated

# List networks
docker network ls
docker network ls --filter driver=bridge
docker network ls --format "{{.Name}}: {{.Driver}}"

# Inspect network
docker network inspect mynet
docker network inspect --format='{{range .Containers}}{{.Name}} {{end}}' mynet

# Connect container to network
docker network connect [OPTIONS] NETWORK CONTAINER
docker network connect mynet myapp
docker network connect --ip 172.28.5.10 mynet myapp
docker network connect --alias db mynet postgres

# Disconnect container from network
docker network disconnect [OPTIONS] NETWORK CONTAINER
docker network disconnect mynet myapp
docker network disconnect -f mynet myapp   # Force

# Remove network
docker network rm NETWORK [NETWORK...]
docker network rm mynet

# Remove all unused networks
docker network prune
docker network prune -f
docker network prune --filter "until=24h"
```

### 3.8 DNS Resolution

- **Default bridge**: Containers inherit host's `/etc/resolv.conf`
- **User-defined networks**: Docker's embedded DNS server at 127.0.0.11

DNS configuration flags:
```bash
docker run --dns 8.8.8.8 --dns-search example.com --dns-option ndots:2 nginx
```

### 3.9 Port Publishing

```bash
# Syntax: -p [host-ip:]host-port:container-port[/protocol]
docker run -p 80:8080 nginx                    # Map host:80 to container:8080
docker run -p 127.0.0.1:80:8080 nginx         # Bind to localhost only
docker run -p 80:8080/tcp -p 80:8080/udp nginx # Both TCP and UDP
docker run -p 8000-8010:8000-8010 nginx        # Port range
docker run -P nginx                             # All exposed ports to random host ports
```

**Key rule**: Containers on the same user-defined bridge expose ALL ports to each other automatically. `-p` is only needed for access from outside the network.

### 3.10 Container Network Stack Sharing

```bash
# Share network namespace with another container
docker run -d --name redis redis --bind 127.0.0.1
docker run --rm -it --network container:redis redis redis-cli -h 127.0.0.1
```

### 3.11 Gateway Priority

When a container is connected to multiple networks, the default gateway is selected by `gw-priority` (default: 0, highest wins):

```bash
docker run --network name=gwnet,gw-priority=1 --network anet1 myimage
```

### 3.12 Subnet Allocation

Docker selects from default pools. Custom pools in `/etc/docker/daemon.json`:

```json
{
  "default-address-pools": [
    { "base": "172.17.0.0/16", "size": 24 }
  ]
}
```

Built-in default pools: `172.17.0.0/16` through `172.28.0.0/14`, `192.168.0.0/16`.

---

## 4. Volume & Storage Management

**Source**: https://docs.docker.com/engine/storage/
**Source**: https://docs.docker.com/engine/storage/volumes/

### 4.1 Storage Types Overview

| Type | Persistence | Management | Use Case |
|------|-------------|------------|----------|
| **Volumes** | Yes | Docker-managed (`/var/lib/docker/volumes/`) | Databases, shared data, backups |
| **Bind mounts** | Yes | Host filesystem (any path) | Development, config files, host access |
| **tmpfs** | No (memory only) | Kernel | Secrets, temp data, performance |
| **Named pipes** | N/A | Host communication | Docker API access |

### 4.2 --mount vs -v Syntax

#### --mount (Preferred — explicit, supports all options)

```bash
docker run --mount type=volume,src=mydata,dst=/data nginx
docker run --mount type=bind,src=/host/path,dst=/container/path nginx
docker run --mount type=tmpfs,dst=/tmp,tmpfs-size=64m nginx
```

**Volume --mount options:**

| Option | Description |
|--------|-------------|
| `type` | `volume`, `bind`, or `tmpfs` |
| `source` / `src` | Volume name (omit for anonymous) |
| `destination` / `dst` / `target` | Container mount path |
| `readonly` / `ro` | Read-only mount |
| `volume-subpath` | Mount subdirectory within volume |
| `volume-nocopy` | Don't copy container data into empty volume |
| `volume-opt` | Driver-specific options (repeatable) |
| `volume-driver` | Volume driver name |

#### -v (Simpler — three colon-separated fields)

```bash
docker run -v mydata:/data nginx                    # Named volume
docker run -v /host/path:/container/path nginx      # Bind mount
docker run -v mydata:/data:ro nginx                 # Read-only
```

Format: `[name:]container-path[:options]`

Options: `ro` (read-only), `volume-nocopy`

#### Key Differences

| Feature | `--mount` | `-v` |
|---------|-----------|------|
| Syntax | Key-value pairs | Colon-separated |
| Volume drivers | ✅ Supported | ❌ Not supported |
| Volume-opt | ✅ Supported | ❌ Not supported |
| Creates host dir if missing | ❌ Error for bind mounts | ✅ Auto-creates |
| Clarity | ✅ Self-documenting | ❌ Position-dependent |

> **ALWAYS use `--mount` for production and documentation.** Use `-v` only for quick local development.

### 4.3 Volume Management Commands

```bash
# Create volume
docker volume create [OPTIONS] [VOLUME]
docker volume create mydata
docker volume create --label project=web mydata
docker volume create -d local --opt type=nfs \
  --opt device=:/nfs/share --opt o=addr=10.0.0.1 nfs-vol

# List volumes
docker volume ls
docker volume ls --filter dangling=true
docker volume ls --filter label=project=web
docker volume ls --format "{{.Name}}: {{.Driver}}"
docker volume ls -q                              # Names only

# Inspect volume
docker volume inspect mydata
docker volume inspect --format '{{.Mountpoint}}' mydata

# Remove volume
docker volume rm mydata
docker volume rm -f mydata                       # Force

# Remove ALL unused volumes
docker volume prune
docker volume prune -f                           # No confirmation
docker volume prune --filter "label!=keep"       # Except labeled
```

### 4.4 Volume Lifecycle & Behavior

**Persistence**: Volumes exist outside container lifecycle. Data survives container deletion.

**Simultaneous mounting**: Multiple containers can mount the same volume simultaneously.

**Empty volume auto-population**: When mounting an empty volume to a directory with existing files, Docker copies those files into the volume.

```bash
# Nginx HTML gets copied into the volume
docker run -d --mount source=nginx-vol,destination=/usr/share/nginx/html nginx
```

**Non-empty volume**: Pre-existing container data is obscured (not merged).

**Prevent auto-copy**: `volume-nocopy` stops Docker from copying container data into empty volumes.

### 4.5 Named vs Anonymous Volumes

| | Named | Anonymous |
|--|-------|-----------|
| Creation | Explicit name | Auto-generated ID |
| Reuse | Easy to reference | Must use random ID |
| Cleanup | Persist until explicitly removed | Auto-removed with `--rm` flag |
| Sharing | Easy between containers | Difficult |

### 4.6 Read-Only Volumes

```bash
# Via --mount
docker run --mount source=data,destination=/data,readonly nginx

# Via -v
docker run -v data:/data:ro nginx
```

Enables write for some containers, read-only for others on the same volume.

### 4.7 Volume Subdirectories

```bash
# Mount specific subdirectory (must exist in volume)
docker run --mount src=logs,dst=/var/log/app1,volume-subpath=app1 app1
docker run --mount src=logs,dst=/var/log/app2,volume-subpath=app2 app2
```

### 4.8 Volume Drivers

#### Local Driver (Default)

Host filesystem storage. Supports NFS, CIFS, and block devices via options.

#### NFS Volumes

```bash
# NFSv3
docker volume create --driver local \
  --opt type=nfs \
  --opt device=:/var/docker-nfs \
  --opt o=addr=10.0.0.10 \
  nfs-vol

# NFSv4
docker volume create --driver local \
  --opt type=nfs \
  --opt device=:/var/docker-nfs \
  --opt "o=addr=10.0.0.10,rw,nfsvers=4,async" \
  nfs-vol
```

#### CIFS/Samba Volumes

```bash
docker volume create --driver local \
  --opt type=cifs \
  --opt device=//server.de/backup \
  --opt o=addr=server.de,username=user,password=pass,file_mode=0777,dir_mode=0777 \
  --name cifs-vol
```

> The `addr` option is REQUIRED when using a hostname instead of IP.

#### Third-Party Volume Drivers

```bash
# Install plugin
docker plugin install --grant-all-permissions rclone/docker-volume-rclone

# Create volume with plugin
docker volume create -d rclone --name remote-vol -o type=sftp -o path=remote

# Use volume (MUST use --mount when volume-driver needs options)
docker run --mount type=volume,volume-driver=rclone,src=remote-vol,target=/app nginx
```

#### Block Storage Devices

```bash
# Mount raw block device
docker run --mount='type=volume,dst=/external-drive,volume-driver=local,volume-opt=device=/dev/loop5,volume-opt=type=ext4' ubuntu
```

### 4.9 Backup & Restore

```bash
# Backup: mount volume from data container, tar to host
docker run --rm --volumes-from dbstore -v $(pwd):/backup ubuntu \
  tar cvf /backup/backup.tar /dbdata

# Restore: mount volume in new container, extract tar
docker run --rm --volumes-from dbstore2 -v $(pwd):/backup ubuntu \
  bash -c "cd /dbdata && tar xvf /backup/backup.tar --strip 1"
```

### 4.10 tmpfs Mounts

```bash
# Via --mount
docker run --mount type=tmpfs,dst=/tmp,tmpfs-size=64m nginx

# Via --tmpfs
docker run --tmpfs /tmp:size=64k nginx
```

**Characteristics**: Memory-only, lost on container stop/restart/host reboot. Cannot be shared between containers. Linux-only.

### 4.11 Docker Compose Volume Integration

```yaml
# Basic volume
services:
  frontend:
    image: node:lts
    volumes:
      - myapp:/home/node/app
volumes:
  myapp:

# External volume (must exist before compose up)
volumes:
  myapp:
    external: true

# Named volume with driver options
volumes:
  dbdata:
    driver: local
    driver_opts:
      type: nfs
      device: ":/nfs/share"
      o: "addr=10.0.0.1,rw"
```

### 4.12 Database Persistence Pattern

```bash
# ✅ Named volume for database data
docker run -d --name postgres \
  --mount source=pgdata,target=/var/lib/postgresql/data \
  -e POSTGRES_PASSWORD=secret \
  postgres:16

# ❌ NEVER use anonymous volumes for database data
docker run -d postgres:16  # Data lost when container removed with --rm
```

---

## 5. Security

**Source**: https://docs.docker.com/engine/security/
**Source**: https://docs.docker.com/engine/security/rootless/
**Source**: https://docs.docker.com/scout/

### 5.1 Kernel-Level Isolation

#### Namespaces

Docker uses kernel namespaces for process isolation:
- **PID namespace**: Process trees isolated between containers
- **Network namespace**: Separate network stacks
- **Mount namespace**: Separate filesystem views
- **UTS namespace**: Separate hostname/domain
- **IPC namespace**: Separate inter-process communication
- **User namespace**: UID/GID mapping (optional)

Namespace technology is production-tested since 2008, evolved from OpenVZ (2005).

#### Control Groups (cgroups)

Resource accounting and limits. Prevent single containers from consuming all system resources (DoS prevention). Merged into Linux kernel 2.6.24.

### 5.2 Linux Capabilities

Docker starts containers with a **restricted allowlist** of capabilities (not full root). Default capabilities are minimal — most traditional root operations are denied:

- ❌ Mount operations
- ❌ Raw socket access (prevents packet spoofing)
- ❌ Device node creation
- ❌ Kernel module loading

```bash
# Drop ALL capabilities, add only needed ones
docker run --cap-drop ALL --cap-add NET_BIND_SERVICE nginx

# Common capabilities
docker run --cap-add SYS_PTRACE ubuntu    # For debugging
docker run --cap-add NET_ADMIN ubuntu     # For network admin
docker run --cap-add SYS_ADMIN ubuntu     # For mount (use sparingly)
```

> **ALWAYS drop all capabilities and add back only what's needed.** NEVER use `--privileged` in production.

### 5.3 Docker Daemon Security

**Attack surface**: The daemon runs as root (unless rootless mode). Only trusted users should have Docker access.

**Critical risks**:
- Container can mount host root filesystem: `docker run -v /:/host ubuntu`
- API access equals root access on the host

**Mitigations**:
- REST API uses Unix socket (not TCP) — prevents CSRF
- HTTPS with certificates mandatory for remote API
- SSH tunneling: `DOCKER_HOST=ssh://USER@HOST`
- **NEVER** expose daemon API over HTTP without TLS

### 5.4 Non-Root Containers

```dockerfile
# ✅ Create non-root user in Dockerfile
RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser
USER appuser

# ✅ Assign explicit UID/GID for determinism
RUN addgroup -g 1001 -S appgroup && adduser -u 1001 -S appuser -G appgroup
USER 1001:1001
```

```bash
# Run as non-root via CLI
docker run -u 1000:1000 nginx
docker run --user nobody nginx
```

> **ALWAYS run containers as non-root unless there's a specific technical requirement for root.**

### 5.5 Read-Only Root Filesystem

```bash
# Read-only root with writable tmpfs for specific paths
docker run --read-only --tmpfs /tmp --tmpfs /run nginx
```

### 5.6 Security Options

```bash
# Prevent privilege escalation
docker run --security-opt no-new-privileges=true nginx

# AppArmor profile
docker run --security-opt apparmor=docker-default nginx

# Seccomp profile
docker run --security-opt seccomp=default nginx              # Use default
docker run --security-opt seccomp=custom-profile.json nginx   # Custom profile
docker run --security-opt seccomp=unconfined nginx            # Disable (dangerous)

# SELinux labels
docker run --security-opt label=type:svirt_apache_t nginx
docker run --security-opt label=disable nginx                 # Disable labeling
```

### 5.7 Content Trust (Image Signing)

Docker Content Trust verifies image integrity and publisher identity.

```bash
# Enable content trust
export DOCKER_CONTENT_TRUST=1

# Now pull/push only works with signed images
docker pull nginx          # Fails if not signed
docker push myapp:v1       # Automatically signs
```

Configure in `daemon.json` via `trust-pinning` for restricting to specific root keys.

### 5.8 Rootless Docker

Runs both daemon and containers without root privileges.

**Prerequisites**:
- `newuidmap` and `newgidmap` (package: `uidmap`)
- `/etc/subuid` and `/etc/subgid` with 65,536+ subordinate UIDs/GIDs

**Installation**:
```bash
# Install via setup tool (non-root)
dockerd-rootless-setuptool.sh install

# Configure environment
export PATH=/usr/bin:$PATH
export DOCKER_HOST=unix:///run/user/$(id -u)/docker.sock

# Enable lingering for auto-start
sudo loginctl enable-linger $(whoami)

# Manage with systemd user units
systemctl --user start docker
systemctl --user stop docker
systemctl --user restart docker
```

**Key difference from userns-remap**: In rootless mode, BOTH daemon and containers run without root. In userns-remap, the daemon still runs as root.

### 5.9 Resource Limits (DoS Prevention)

```bash
# Memory limits
docker run -m 512m --memory-swap 1g nginx

# CPU limits
docker run --cpus 1.5 nginx

# PID limits (fork bomb prevention)
docker run --pids-limit 200 nginx

# Ulimits
docker run --ulimit nofile=1024:2048 nginx

# Disable OOM killer (use with memory limits)
docker run -m 512m --oom-kill-disable nginx
```

### 5.10 Docker Scout (Image Scanning)

Docker Scout analyzes images for vulnerabilities by creating an SBOM and matching against vulnerability databases.

```bash
# Quick vulnerability overview
docker scout quickview nginx

# Detailed CVE listing
docker scout cves nginx

# Critical/high only
docker scout cves --only-severity critical,high nginx

# Only fixable vulnerabilities
docker scout cves --only-fixed nginx

# With EPSS scores (exploit probability)
docker scout cves --epss --epss-score 0.5 nginx

# Specific output format
docker scout cves --format sarif -o report.json nginx
docker scout cves --format markdown -o report.md nginx

# Scan local directory
docker scout cves fs://.

# Scan tarball
docker scout cves archive://image.tar

# Compare two images
docker scout compare nginx:1.24 --to nginx:1.25

# Get recommendations
docker scout recommendations nginx

# Exit code for CI/CD (returns 2 if vulnerabilities found)
docker scout cves -e nginx
```

**Artifact URI prefixes**:
| Prefix | Source |
|--------|--------|
| `image://` | Local image with registry fallback (default) |
| `local://` | Local image store only |
| `registry://` | Registry only |
| `oci-dir://` | OCI layout directory |
| `archive://` | Docker save tarball |
| `fs://` | Local directory/file |
| `sbom://` | SPDX/in-toto/syft JSON SBOM |

**CVE filtering flags**:
- `--only-severity` — Filter by severity level
- `--only-fixed` / `--only-unfixed` — Fixability filter
- `--only-cve-id` — Target specific CVEs
- `--only-cisa-kev` — CISA Known Exploited Vulnerabilities
- `--only-package-type` — By package manager (apk, deb, rpm, npm, pypi, golang)
- `--only-package` — Regex package filter
- `--ignore-base` / `--only-base` — Base image CVE filtering
- `--ignore-suppressed` — Exclude Scout exceptions
- `--epss-score` / `--epss-percentile` — Exploit probability filter

### 5.11 Security Best Practices Summary

| Practice | Implementation |
|----------|---------------|
| ✅ Non-root containers | `USER 1001` in Dockerfile |
| ✅ Minimal capabilities | `--cap-drop ALL --cap-add <needed>` |
| ✅ Read-only root FS | `--read-only --tmpfs /tmp` |
| ✅ No privilege escalation | `--security-opt no-new-privileges=true` |
| ✅ Resource limits | `-m 512m --cpus 1.5 --pids-limit 200` |
| ✅ Image scanning | `docker scout cves --only-severity critical,high` |
| ✅ Content trust | `DOCKER_CONTENT_TRUST=1` |
| ✅ Minimal base images | Alpine (<6 MB) or distroless |
| ✅ Pin image digests | `FROM alpine@sha256:abc123...` |
| ✅ No secrets in image | Use `--secret` in build, never `COPY`/`ENV` |
| ❌ NEVER `--privileged` | Use specific `--cap-add` instead |
| ❌ NEVER expose daemon API without TLS | Use SSH tunnel or Unix socket |
| ❌ NEVER run as root unless required | Create and use non-root user |
| ❌ NEVER use `latest` tag in production | Pin specific versions |

---

## 6. Go Template Formatting

**Source**: https://docs.docker.com/reference/cli/docker/inspect/

Docker CLI supports Go templates via `--format` for `inspect`, `ps`, `images`, `stats`, `network ls`, `volume ls`, and more.

### 6.1 Basic Syntax

```bash
# Access field
docker inspect --format='{{.State.Status}}' myapp

# Range over arrays/maps
docker inspect --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' myapp

# JSON output
docker inspect --format='{{json .Config}}' myapp

# Conditional
docker inspect --format='{{if .State.Running}}UP{{else}}DOWN{{end}}' myapp

# Table format (for ls commands)
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

# JSON output mode
docker ps --format json
```

### 6.2 Common Inspect Patterns

#### Network Information

```bash
# Get IP address
docker inspect --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' myapp

# Get MAC address
docker inspect --format='{{range .NetworkSettings.Networks}}{{.MacAddress}}{{end}}' myapp

# Get all port bindings
docker inspect --format='{{range $p, $conf := .NetworkSettings.Ports}}{{$p}} -> {{(index $conf 0).HostPort}}{{end}}' myapp

# Get specific port
docker inspect --format='{{(index (index .NetworkSettings.Ports "80/tcp") 0).HostPort}}' myapp

# Get network name
docker inspect --format='{{range $k, $v := .NetworkSettings.Networks}}{{$k}}{{end}}' myapp
```

#### Container State

```bash
# Status
docker inspect --format='{{.State.Status}}' myapp

# PID
docker inspect --format='{{.State.Pid}}' myapp

# Start time
docker inspect --format='{{.State.StartedAt}}' myapp

# Exit code
docker inspect --format='{{.State.ExitCode}}' myapp

# Running check
docker inspect --format='{{.State.Running}}' myapp
```

#### Configuration

```bash
# Image name
docker inspect --format='{{.Config.Image}}' myapp

# Entrypoint
docker inspect --format='{{.Config.Entrypoint}}' myapp

# Command
docker inspect --format='{{json .Config.Cmd}}' myapp

# Environment variables
docker inspect --format='{{range .Config.Env}}{{println .}}{{end}}' myapp

# Labels
docker inspect --format='{{json .Config.Labels}}' myapp

# Specific label
docker inspect --format='{{index .Config.Labels "com.example.version"}}' myapp
```

#### Mounts & Volumes

```bash
# All mounts
docker inspect --format='{{json .Mounts}}' myapp

# Mount sources
docker inspect --format='{{range .Mounts}}{{.Source}} -> {{.Destination}}{{println}}{{end}}' myapp
```

#### Size

```bash
# Total filesystem size
docker inspect --size -f '{{.SizeRootFs}}' myapp

# Writable layer size
docker inspect --size -f '{{.SizeRw}}' myapp

# Log path
docker inspect --format='{{.LogPath}}' myapp
```

### 6.3 Common PS Format Patterns

```bash
# Compact running containers
docker ps --format "table {{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}"

# Container names only
docker ps --format "{{.Names}}"

# With custom headers
docker ps --format "table {{.ID}}\t{{.Names}}\t{{.State}}\t{{.RunningFor}}"

# JSON (one per line)
docker ps --format json

# With size
docker ps -s --format "table {{.Names}}\t{{.Size}}"

# Specific label
docker ps --format "{{.Names}}: {{.Label \"app\"}}"
```

### 6.4 Image Format Patterns

```bash
# Compact image list
docker images --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}"

# Repository:tag
docker images --format "{{.Repository}}:{{.Tag}}"

# JSON
docker images --format json

# With digest
docker images --digests --format "table {{.Repository}}\t{{.Tag}}\t{{.Digest}}"
```

### 6.5 Stats Format Patterns

```bash
docker stats --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}\t{{.BlockIO}}"
```

---

## 7. Common Errors & Troubleshooting

**Source**: https://docs.docker.com/engine/daemon/troubleshoot/
**Source**: Synthesized from official documentation patterns

### 7.1 Daemon & Connection Errors

| # | Error Message | Cause | Solution |
|---|--------------|-------|----------|
| 1 | `Cannot connect to the Docker daemon. Is 'docker daemon' running on this host?` | Docker daemon not running or client misconfigured | Start daemon: `sudo systemctl start docker`. Check `DOCKER_HOST`: `env \| grep DOCKER_HOST`. Unset if incorrect: `unset DOCKER_HOST` |
| 2 | `permission denied while trying to connect to the Docker daemon socket` | User not in `docker` group | `sudo usermod -aG docker $USER` then log out/in. Or use `sudo` |
| 3 | `unable to configure the Docker daemon with file /etc/docker/daemon.json` | Conflicting options between daemon.json and CLI flags | Remove duplicate options from either daemon.json or systemd unit. Create `/etc/systemd/system/docker.service.d/docker.conf` to override ExecStart |
| 4 | `Error starting daemon: error initializing graphdriver` | Storage driver issue or corrupted state | Check storage driver compatibility. May need `docker system prune` or clean `/var/lib/docker` |

### 7.2 Build Errors

| # | Error Message | Cause | Solution |
|---|--------------|-------|----------|
| 5 | `COPY failed: file not found in build context` | File outside build context or in `.dockerignore` | Verify file is in build context directory. Check `.dockerignore` patterns. Cannot COPY files from parent directories |
| 6 | `failed to solve: dockerfile parse error` | Syntax error in Dockerfile | Check for typos, missing backslashes in multi-line RUN, wrong instruction names |
| 7 | `failed to compute cache key: failed to calculate checksum of ref` | File referenced in COPY/ADD doesn't exist | Verify file path relative to build context. Check `.dockerignore` |
| 8 | `apt-get install` installs stale packages | `apt-get update` and `apt-get install` in separate RUN layers | ALWAYS combine: `RUN apt-get update && apt-get install -y package` |
| 9 | `ERROR: failed to solve: process "/bin/sh -c ..." did not complete successfully: exit code: 1` | Command in RUN instruction failed | Check the specific command. Add `set -e` for visibility. Use `--progress=plain` for full output |
| 10 | `max depth exceeded` | Too many image layers or recursive builds | Reduce number of layers by combining RUN instructions. Check for circular FROM dependencies |

### 7.3 Runtime Errors

| # | Error Message | Cause | Solution |
|---|--------------|-------|----------|
| 11 | Container killed by OOM (exit code 137) | Out of memory | Increase `-m` limit: `docker run -m 1g`. Check for memory leaks. Use `docker stats` to monitor |
| 12 | `exec format error` | Binary architecture mismatch (e.g., ARM image on AMD64) | Use `--platform` flag or build for correct architecture |
| 13 | `standard_init_linux.go: exec user process caused: no such file or directory` | Wrong line endings (CRLF) in shell scripts, or missing binary | Convert scripts to LF: `dos2unix script.sh`. Check ENTRYPOINT/CMD binary exists |
| 14 | `OCI runtime create failed: container_linux.go: starting container process caused` | Various runtime issues | Check logs: `journalctl -u docker`. Common: SELinux, cgroup v2 issues, corrupted image |
| 15 | Exit code 139 (SIGSEGV) | Segmentation fault in application | Debug application. Check for native library compatibility issues |
| 16 | Container immediately exits | No foreground process | Ensure CMD/ENTRYPOINT runs in foreground (not daemonized). Use `docker logs` to check |
| 17 | `Error response from daemon: Conflict. The container name "X" is already in use` | Container name already taken | Remove old container: `docker rm old_container`. Or use `--rm` flag. Or use unique names |

### 7.4 Network Errors

| # | Error Message | Cause | Solution |
|---|--------------|-------|----------|
| 18 | `port is already allocated` / `bind: address already in use` | Another process using the host port | Find process: `lsof -i :PORT` or `netstat -tlnp \| grep PORT`. Stop conflicting process or use different port |
| 19 | `dial tcp: lookup <hostname>: no such host` | DNS resolution failure in container | Check `--dns` settings. For default bridge: DNS inherits from host. For custom networks: Docker DNS at 127.0.0.11 |
| 20 | `WARNING: Local (127.0.0.1) DNS resolver found in resolv.conf` | Container can't reach host's loopback DNS | Configure DNS in daemon.json: `{"dns": ["8.8.8.8", "8.8.4.4"]}`. Or disable dnsmasq |
| 21 | Containers can't reach each other by name on default bridge | Default bridge lacks DNS resolution | Use user-defined bridge: `docker network create mynet` and `--network mynet` |
| 22 | `docker0` bridge disappears | Other network services interfering (netscript, NetworkManager, systemd-networkd) | Configure interface as unmanaged. Create `/etc/systemd/network/docker.network` with `Unmanaged=yes` |
| 23 | Container can't access internet | IP forwarding disabled | Enable: `sysctl net.ipv4.ip_forward=1`. For systemd-networkd: add `IPForward=true` to network config |

### 7.5 Storage & Volume Errors

| # | Error Message | Cause | Solution |
|---|--------------|-------|----------|
| 24 | `Unable to remove filesystem` | Container bind-mounting `/var/lib/docker/` holds open handles | Stop containers holding mounts first. Use `lsof` to find busy processes. Avoid mounting `/var/lib/docker/` |
| 25 | `no space left on device` | Docker disk full | Run `docker system prune -a --volumes`. Check `docker system df`. Clean build cache: `docker builder prune` |
| 26 | `error creating overlay mount` | Corrupted storage or incompatible storage driver | Check storage driver. May need to remove `/var/lib/docker` and reinitialize |
| 27 | Volume data not persisting | Using anonymous volume with `--rm`, or wrong mount path | Use named volumes. Verify mount destination matches application data path |
| 28 | Permission denied accessing mounted volume | UID/GID mismatch between host and container | Match UIDs: `docker run -u $(id -u):$(id -g)`. Or `chown` in Dockerfile. Or use `--userns-remap` |

### 7.6 Image Errors

| # | Error Message | Cause | Solution |
|---|--------------|-------|----------|
| 29 | `manifest unknown: manifest unknown` | Image tag doesn't exist in registry | Verify tag exists: `docker manifest inspect image:tag`. Check for typos |
| 30 | `unauthorized: authentication required` | Not logged in or insufficient permissions | Run `docker login`. Check credentials and repository permissions |
| 31 | `image with reference X was found but does not match the specified platform` | Platform mismatch | Use `--platform linux/amd64` (or correct platform). Build for target platform |

### 7.7 Kernel & System Errors

| # | Error Message | Cause | Solution |
|---|--------------|-------|----------|
| 32 | `Your kernel does not support swap limit capabilities` | Kernel lacks swap accounting | Edit `/etc/default/grub`: `GRUB_CMDLINE_LINUX="cgroup_enable=memory swapaccount=1"`. Run `sudo update-grub` and reboot |
| 33 | `Error response from daemon: driver failed programming external connectivity` | iptables/firewall issue | Restart Docker: `sudo systemctl restart docker`. Check iptables rules. May need `--iptables=true` in daemon.json |

### 7.8 Diagnostic Commands

```bash
# Check daemon status
docker info
systemctl status docker
journalctl -u docker --no-pager -n 50

# Check container health
docker inspect --format='{{.State.Health.Status}}' myapp
docker inspect --format='{{json .State.Health}}' myapp

# Resource usage
docker stats --no-stream
docker system df -v

# Container logs
docker logs --tail 100 myapp
docker logs --since 5m myapp

# Process list
docker top myapp

# Filesystem changes
docker diff myapp

# Events
docker events --since 10m

# Network debugging
docker exec myapp ping -c 2 other-container
docker exec myapp nslookup other-container
docker exec myapp cat /etc/resolv.conf
docker network inspect mynet

# Inspect everything
docker inspect myapp | jq '.[0].State'
docker inspect myapp | jq '.[0].NetworkSettings.Networks'
docker inspect myapp | jq '.[0].Mounts'
```

---

## 8. System & Maintenance

**Source**: https://docs.docker.com/reference/cli/docker/system/

### 8.1 docker system df — Disk Usage

```bash
# Overview
docker system df

# Verbose (per-resource breakdown)
docker system df -v

# Custom format
docker system df --format "table {{.Type}}\t{{.TotalCount}}\t{{.Size}}\t{{.Reclaimable}}"

# JSON
docker system df --format json
```

**Output fields**:
- **TYPE**: Images, Containers, Local Volumes, Build Cache
- **TOTAL**: Count of resources
- **ACTIVE**: Currently in-use
- **SIZE**: Disk space consumed
- **RECLAIMABLE**: Recoverable space + percentage

**Verbose image fields**: Repository, Tag, Image ID, Created, Size, Shared Size, Unique Size, Containers

### 8.2 docker system prune — Clean Up

```bash
# Remove stopped containers, unused networks, dangling images, build cache
docker system prune

# Also remove ALL unused images (not just dangling)
docker system prune -a

# Also remove anonymous volumes
docker system prune --volumes

# Nuclear option: everything unused
docker system prune -a --volumes -f

# With time filter
docker system prune --filter "until=24h"

# With label filter
docker system prune --filter "label=temp"
docker system prune --filter "label!=keep"
```

**What each prune removes**:

| Command | Removes |
|---------|---------|
| `docker container prune` | All stopped containers |
| `docker image prune` | Dangling images only |
| `docker image prune -a` | All unused images |
| `docker network prune` | All unused networks |
| `docker volume prune` | All unused anonymous volumes |
| `docker builder prune` | Build cache |
| `docker system prune` | Stopped containers + unused networks + dangling images + build cache |
| `docker system prune -a --volumes` | All of the above + all unused images + anonymous volumes |

### 8.3 docker system info

```bash
# Full system information
docker info

# Specific field via format
docker info --format '{{.ServerVersion}}'
docker info --format '{{.OperatingSystem}}'
docker info --format '{{.Driver}}'
docker info --format '{{json .Plugins}}'
```

Shows: server version, storage driver, logging driver, cgroup driver, kernel version, OS, CPU/memory, registry config, security options, runtime info.

### 8.4 docker system events

```bash
# Real-time daemon events
docker events

# Filter by type
docker events --filter type=container
docker events --filter type=image
docker events --filter type=volume
docker events --filter type=network

# Filter by event
docker events --filter event=start
docker events --filter event=stop
docker events --filter event=die

# Filter by container
docker events --filter container=myapp

# Time range
docker events --since '2024-01-01T00:00:00'
docker events --until '10m'

# JSON format
docker events --format '{{json .}}'

# Custom format
docker events --format '{{.Time}} {{.Type}} {{.Action}} {{.Actor.Attributes.name}}'
```

### 8.5 docker version

```bash
# Full version info (client + server)
docker version

# Short version
docker version --format '{{.Server.Version}}'

# JSON
docker version --format json
```

### 8.6 docker context

```bash
# List contexts
docker context ls

# Create context for remote host
docker context create remote --docker "host=ssh://user@remote-host"

# Switch context
docker context use remote

# Switch back to default
docker context use default

# Inspect context
docker context inspect remote

# Remove context
docker context rm remote
```

### 8.7 Disk Space Management Strategy

```bash
# 1. Assess current usage
docker system df -v

# 2. Remove stopped containers
docker container prune -f

# 3. Remove dangling images
docker image prune -f

# 4. Remove unused images (aggressive)
docker image prune -a -f

# 5. Remove unused volumes (careful — check first!)
docker volume ls -f dangling=true
docker volume prune -f

# 6. Remove build cache
docker builder prune -f
docker builder prune -a -f   # All build cache

# 7. Nuclear cleanup
docker system prune -a --volumes -f
```

> **ALWAYS run `docker system df` first to understand what's consuming space before pruning.** NEVER blindly run `docker system prune -a --volumes` on production systems — it removes ALL unused volumes including database data.

### 8.8 Scheduled Cleanup (Cron)

```bash
# Daily cleanup of containers older than 24h and dangling images
0 2 * * * docker container prune -f --filter "until=24h" && docker image prune -f
```

---

## Dockerfile Best Practices (Bonus Section)

**Source**: https://docs.docker.com/build/building/best-practices/

### Layer Ordering for Cache Optimization

Order instructions from least to most frequently changed:

```dockerfile
# ✅ CORRECT — dependencies change less often than source code
FROM node:20-alpine
WORKDIR /app
COPY package*.json ./
RUN npm ci --production
COPY . .
CMD ["node", "server.js"]
```

```dockerfile
# ❌ WRONG — any source change invalidates npm install cache
FROM node:20-alpine
WORKDIR /app
COPY . .
RUN npm ci --production
CMD ["node", "server.js"]
```

### Package Manager Best Practices

```dockerfile
# ✅ CORRECT — combined update+install, cleanup, sorted packages
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    git \
    nginx \
  && rm -rf /var/lib/apt/lists/*
```

```dockerfile
# ❌ WRONG — separate update and install (stale cache risk)
RUN apt-get update
RUN apt-get install -y curl
```

### Multi-Stage Builds

```dockerfile
# Build stage
FROM golang:1.22 AS builder
WORKDIR /app
COPY go.* ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 go build -o /server

# Production stage (minimal image)
FROM alpine:3.21
RUN apk --no-cache add ca-certificates
COPY --from=builder /server /server
USER 1001
CMD ["/server"]
```

### Base Image Selection

- ✅ Use Docker Official Images
- ✅ Use Alpine (<6 MB) or distroless for production
- ✅ Pin to digest for supply chain security: `FROM alpine@sha256:abc123...`
- ✅ Use `--pull --no-cache` in CI to get latest patches

### CMD vs ENTRYPOINT

```dockerfile
# ENTRYPOINT for the main command, CMD for default arguments
ENTRYPOINT ["python"]
CMD ["--help"]

# ENTRYPOINT with helper script
COPY docker-entrypoint.sh /
ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["postgres"]
```

### COPY vs ADD

| Instruction | Use When |
|-------------|----------|
| `COPY` | ✅ Default choice — simple file copying |
| `ADD` | Remote URLs, Git repos, auto-tar extraction |

```dockerfile
# ✅ Use COPY for local files
COPY requirements.txt .

# ✅ Use bind mount for temporary build files
RUN --mount=type=bind,source=requirements.txt,target=/tmp/requirements.txt \
    pip install -r /tmp/requirements.txt
```

### Pipe Failure Safety

```dockerfile
# ✅ CORRECT — fails on intermediate pipe errors
RUN set -o pipefail && wget -O - https://example.com | wc -l > /count

# ❌ WRONG — only reports last command's exit code
RUN wget -O - https://example.com | wc -l > /count
```

### WORKDIR

```dockerfile
# ✅ CORRECT — use absolute WORKDIR
WORKDIR /app

# ❌ WRONG — cd in RUN (hard to track)
RUN cd /app && do-something
```

### Environment Variables for Version Pinning

```dockerfile
ENV NODE_VERSION=20.11.0
ENV PG_MAJOR=16
RUN curl -SL https://nodejs.org/dist/v$NODE_VERSION/node-v$NODE_VERSION.tar.xz | tar -xJ
```

---

## Source Verification Table

| Source URL | Fetched | Content Quality |
|-----------|---------|-----------------|
| https://docs.docker.com/reference/cli/docker/ | ✅ 2026-03-19 | Overview only |
| https://docs.docker.com/reference/cli/docker/container/ | ✅ 2026-03-19 | Command list |
| https://docs.docker.com/reference/cli/docker/container/run/ | ✅ 2026-03-19 | Comprehensive — all flags |
| https://docs.docker.com/reference/cli/docker/container/exec/ | ✅ 2026-03-19 | Complete |
| https://docs.docker.com/reference/cli/docker/container/logs/ | ✅ 2026-03-19 | Complete |
| https://docs.docker.com/reference/cli/docker/container/ls/ | ✅ 2026-03-19 | Complete — filters + format |
| https://docs.docker.com/reference/cli/docker/image/ | ✅ 2026-03-19 | Command list |
| https://docs.docker.com/reference/cli/docker/image/ls/ | ✅ 2026-03-19 | Complete — filters + format |
| https://docs.docker.com/reference/cli/docker/image/build/ | ✅ 2026-03-19 | Legacy builder only |
| https://docs.docker.com/reference/cli/docker/buildx/build/ | ✅ 2026-03-19 | Comprehensive — all flags |
| https://docs.docker.com/reference/cli/docker/inspect/ | ✅ 2026-03-19 | Good — Go templates |
| https://docs.docker.com/reference/cli/docker/network/create/ | ✅ 2026-03-19 | Comprehensive |
| https://docs.docker.com/reference/cli/docker/system/ | ✅ 2026-03-19 | Overview only |
| https://docs.docker.com/reference/cli/docker/system/prune/ | ✅ 2026-03-19 | Complete |
| https://docs.docker.com/reference/cli/docker/system/df/ | ✅ 2026-03-19 | Complete |
| https://docs.docker.com/engine/security/ | ✅ 2026-03-19 | Good — namespaces, cgroups, capabilities |
| https://docs.docker.com/engine/security/rootless/ | ✅ 2026-03-19 | Good — setup only |
| https://docs.docker.com/engine/network/ | ✅ 2026-03-19 | Comprehensive |
| https://docs.docker.com/engine/network/drivers/bridge/ | ✅ 2026-03-19 | Comprehensive |
| https://docs.docker.com/engine/network/drivers/overlay/ | ✅ 2026-03-19 | Good |
| https://docs.docker.com/engine/storage/ | ✅ 2026-03-19 | Overview only |
| https://docs.docker.com/engine/storage/volumes/ | ✅ 2026-03-19 | Comprehensive |
| https://docs.docker.com/scout/ | ✅ 2026-03-19 | Overview only |
| https://docs.docker.com/reference/cli/docker/scout/cves/ | ✅ 2026-03-19 | Comprehensive |
| https://docs.docker.com/build/building/best-practices/ | ✅ 2026-03-19 | Comprehensive |
| https://docs.docker.com/engine/daemon/troubleshoot/ | ✅ 2026-03-19 | Good — daemon/network/storage errors |
