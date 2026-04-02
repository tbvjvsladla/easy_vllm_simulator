---
name: docker-core-architecture
description: >
  Use when designing Docker container architecture or explaining how Docker
  Engine components interact.
  Prevents misconceptions about container isolation, image layering, and the
  relationship between daemon, containerd, and runc.
  Covers Docker Engine architecture, OCI standards, image layers, container
  lifecycle, build context model, and Docker object types.
  Keywords: dockerd, containerd, runc, OCI, image layers, docker build,
  docker run, BuildKit, container lifecycle, how Docker works, what is a container,
  Docker internals, image vs container.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-core-architecture

## Quick Reference

### Architecture Components

| Component | Role | Process |
|-----------|------|---------|
| Docker CLI | User-facing command interface | `docker` |
| Docker Daemon | API server, image management, orchestration | `dockerd` |
| containerd | Container runtime supervision, image pull/push | `containerd` |
| runc | OCI-compliant container spawner | `runc` (exits after spawn) |
| BuildKit | Image build engine (default since Engine 23+) | `buildkitd` (within dockerd) |

### Docker Object Types

| Object | Description | Key Command |
|--------|-------------|-------------|
| Image | Immutable, layered filesystem template | `docker build`, `docker pull` |
| Container | Runnable instance of an image with writable layer | `docker run`, `docker create` |
| Network | Isolated communication channel between containers | `docker network create` |
| Volume | Persistent storage managed by Docker | `docker volume create` |

### OCI Standards

| Standard | Purpose | Governs |
|----------|---------|---------|
| OCI Image Spec | Portable image format | Layer format, manifest, config |
| OCI Runtime Spec | Container execution contract | Filesystem bundle, lifecycle, environment |
| OCI Distribution Spec | Image registry API | Pull, push, content discovery |

### Critical Warnings

**NEVER** assume a container has persistent storage -- the writable container layer is deleted when the container is removed. ALWAYS use volumes or bind mounts for data that must survive container removal.

**NEVER** treat images as mutable -- images are immutable stacks of read-only layers. To change an image, ALWAYS build a new one. Using `docker commit` in production creates unreproducible, undocumented images.

**NEVER** send unnecessary files in the build context -- ALWAYS create a `.dockerignore` file. The entire build context directory is sent to the daemon before any build instruction executes.

**NEVER** confuse `docker export`/`import` with `docker save`/`load` -- `export` flattens all layers into a single filesystem tar (loses history and metadata). `save` preserves the full image structure with all layers, tags, and history.

**NEVER** run production workloads without resource limits -- containers without memory or CPU limits can consume all host resources. ALWAYS set `-m` and `--cpus` flags.

---

## Architecture Diagram

```
                         Docker Architecture (Engine 24+)

 +------------------+
 |   Docker CLI     |  User runs: docker build / run / pull / push
 +--------+---------+
          |
          | REST API (Unix socket or TCP)
          v
 +--------+---------+
 |   Docker Daemon  |  dockerd
 |                  |  - Serves Docker API
 |  +------------+  |  - Manages images, networks, volumes
 |  |  BuildKit  |  |  - Orchestrates container lifecycle
 |  +------------+  |
 +--------+---------+
          |
          | gRPC API
          v
 +--------+---------+
 |   containerd     |  Container runtime supervisor
 |                  |  - Manages container lifecycle
 |                  |  - Pulls/pushes images (OCI compliant)
 |                  |  - Manages snapshots (layer storage)
 +--------+---------+
          |
          | OCI Runtime Spec
          v
 +--------+---------+
 |      runc        |  OCI reference runtime
 |                  |  - Creates namespaces & cgroups
 |                  |  - Starts container process
 |                  |  - Exits after spawn (container runs independently)
 +------------------+

          |
          v
 +------------------+
 |   Container      |  Isolated process(es) with:
 |                  |  - Own PID, network, mount, UTS, IPC namespaces
 |                  |  - Resource limits via cgroups
 |                  |  - Union filesystem (read-only layers + writable layer)
 +------------------+
```

### Request Flow

1. CLI sends REST API request to `dockerd` (via Unix socket `/var/run/docker.sock`)
2. `dockerd` validates request, manages high-level logic (networking, volumes, images)
3. `dockerd` delegates container operations to `containerd` via gRPC
4. `containerd` prepares the OCI bundle (rootfs + config.json)
5. `containerd` calls `runc` to create and start the container
6. `runc` sets up namespaces, cgroups, and rootfs, then starts the process
7. `runc` exits -- the container process runs directly under `containerd`

---

## Image and Layer Model

### Union Filesystem

Docker images use a **union filesystem** (typically overlay2) that stacks read-only layers on top of each other:

```
 Container (running)
 +---------------------------+
 | Writable Container Layer  |  <-- Changes (writes, deletes) go here
 +---------------------------+
 | Layer 4: COPY app.js      |  \
 +---------------------------+   |
 | Layer 3: RUN npm install   |   > Read-only image layers
 +---------------------------+   |
 | Layer 2: COPY package.json |   |
 +---------------------------+   |
 | Layer 1: FROM node:20-slim |  /
 +---------------------------+
```

### Key Layer Behaviors

- Each Dockerfile instruction that modifies the filesystem (RUN, COPY, ADD) creates one layer
- Layers are **content-addressable** -- identified by SHA256 digest of their contents
- Layers are **shared** across images -- if two images use the same base, the base layers exist only once on disk
- The writable container layer uses **copy-on-write** -- a file is copied from a lower layer to the writable layer only when modified
- Deleting a file in a higher layer creates a **whiteout marker** -- the file still exists in the lower layer but is hidden
- ALWAYS minimize layer count by combining related RUN commands with `&&`

### Image Identification

| Identifier | Format | Example |
|------------|--------|---------|
| Repository + Tag | `name:tag` | `nginx:1.25-alpine` |
| Digest | `name@sha256:...` | `nginx@sha256:a8560b...` |
| Image ID | Short SHA256 | `d1a364dc548d` |

- Tags are **mutable** pointers -- `nginx:latest` can point to different images over time
- Digests are **immutable** -- ALWAYS use digests in production for reproducibility
- An image can have multiple tags pointing to the same digest

---

## Build Context

The **build context** is the set of files sent to the Docker daemon when you run `docker build`.

### How Build Context Works

1. CLI packages the build context directory into a tar archive
2. Tar archive is sent to the daemon (even if daemon is local)
3. COPY and ADD instructions reference files relative to the build context root
4. Files outside the build context are **not accessible** to the build

### Build Context Rules

- The `.` in `docker build .` specifies the build context directory
- The Dockerfile location (`-f`) is independent of the build context
- `.dockerignore` filters files BEFORE sending to the daemon
- ALWAYS exclude unnecessary files via `.dockerignore` to reduce context size and build time
- Large contexts (>100MB) significantly slow down builds

### Build Context Sources

| Source | Example | Notes |
|--------|---------|-------|
| Local directory | `docker build .` | Most common |
| Git URL | `docker build https://github.com/user/repo.git` | Cloned by daemon |
| Tar archive | `docker build - < archive.tar.gz` | Extracted as context |
| stdin (Dockerfile only) | `docker build - <<< "FROM alpine"` | No file context available |

---

## Container Lifecycle

### State Diagram

```
                    docker create
                         |
                         v
                   +-----------+
                   |  Created  |
                   +-----------+
                         |
                    docker start
                         |
                         v
  docker unpause   +-----------+   docker pause
  +--------------->|  Running  |<--------------+
  |                +-----------+               |
  |                   |     |                  |
  |              docker|    |docker         +--------+
  |               stop |    | pause         | Paused |
  |                    |    +-------------->+--------+
  |                    v
  |              +-----------+
  |              |  Stopped  |  (Exited)
  |              +-----------+
  |                    |
  |               docker rm
  |                    |
  |                    v
  |              +-----------+
  |              |  Removed  |
  |              +-----------+
```

### Lifecycle States

| State | Description | Key Behavior |
|-------|-------------|--------------|
| Created | Container exists but process has not started | Writable layer allocated, config set |
| Running | Main process is executing | Has PID, consumes resources, network active |
| Paused | Process suspended via cgroup freezer | Memory preserved, CPU released, no I/O |
| Stopped | Main process exited (exit code preserved) | Writable layer preserved, no resource usage |
| Removed | Container deleted | Writable layer deleted, anonymous volumes removed if `--rm` |

### Command-to-Architecture Mapping

| Command | Docker Object Affected | What Happens |
|---------|----------------------|--------------|
| `docker build` | Image | BuildKit executes Dockerfile, produces layered image |
| `docker pull` | Image | containerd fetches layers from registry via OCI Distribution |
| `docker run` | Container + (Image) | Pull if needed, create container, allocate writable layer, start process |
| `docker create` | Container | Allocate writable layer, set config, do NOT start |
| `docker start` | Container | containerd calls runc to start process |
| `docker stop` | Container | Send SIGTERM, wait grace period, then SIGKILL |
| `docker kill` | Container | Send signal immediately (default SIGKILL) |
| `docker rm` | Container | Remove writable layer and metadata |
| `docker rmi` | Image | Remove image layers (if not referenced by other images/containers) |

---

## Container Isolation Model

Docker containers are isolated using Linux kernel primitives:

### Namespaces (What the container can see)

| Namespace | Isolates | Effect |
|-----------|----------|--------|
| PID | Process IDs | Container sees only its own processes; PID 1 is the main process |
| Network | Network stack | Own IP address, ports, routing table, firewall rules |
| Mount | Filesystem | Own root filesystem via union mount |
| UTS | Hostname | Own hostname and domain name |
| IPC | Inter-process communication | Own shared memory, semaphores, message queues |
| User | UID/GID mapping | Root inside container maps to unprivileged user on host (optional) |

### Cgroups (What the container can use)

| Resource | Control | CLI Flag |
|----------|---------|----------|
| Memory | Hard limit, soft limit, swap | `-m 512m`, `--memory-swap 1g` |
| CPU | Share weight, core pinning, quota | `--cpus 1.5`, `--cpuset-cpus 0-3` |
| PIDs | Maximum process count | `--pids-limit 200` |
| Block I/O | Read/write bandwidth | `--device-read-bps /dev/sda:1mb` |

---

## Decision Trees

### When to Use Which Docker Object

```
Need persistent data?
  YES --> Use a Volume (docker volume create)
    Shared between containers? --> Named volume
    Single container temp data? --> tmpfs mount
    Host file access needed? --> Bind mount
  NO --> Container writable layer is sufficient

Need container communication?
  YES --> Use a Network (docker network create)
    Same host? --> User-defined bridge
    Multi-host? --> Overlay (requires Swarm)
    Direct LAN? --> Macvlan
  NO --> Use --network none

Need a reusable environment?
  YES --> Build an Image (Dockerfile + docker build)
  NO --> Use docker run with existing image
```

### Image vs Container Decision

```
Is it a template (immutable, shareable, versioned)?
  --> IMAGE: Build it, tag it, push it

Is it a running instance (has state, has PID, consumes resources)?
  --> CONTAINER: Run it, stop it, remove it
```

---

## Reference Links

- [references/concepts.md](references/concepts.md) -- Docker object types, image layers, union filesystem details
- [references/examples.md](references/examples.md) -- Architecture interaction examples, component diagrams
- [references/anti-patterns.md](references/anti-patterns.md) -- Architectural mistakes and corrections

### Official Sources

- https://docs.docker.com/get-started/docker-overview/
- https://docs.docker.com/engine/
- https://docs.docker.com/build/buildkit/
- https://docs.docker.com/engine/storage/
- https://docs.docker.com/engine/network/
- https://docs.docker.com/engine/security/
- https://opencontainers.org/
