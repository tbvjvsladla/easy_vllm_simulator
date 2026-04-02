# Docker Architecture Examples

## Component Interaction: docker run

When you execute `docker run -d --name web -p 8080:80 nginx:1.25`, the following sequence occurs:

```
Step 1: CLI --> Daemon (REST API)
   docker run -d --name web -p 8080:80 nginx:1.25
   POST /v1.45/containers/create
   POST /v1.45/containers/{id}/start

Step 2: Daemon checks local image store
   Image nginx:1.25 present?
     NO  --> Pull from registry (Steps 2a-2c)
     YES --> Skip to Step 3

Step 2a: Daemon --> containerd (gRPC)
   Pull image nginx:1.25

Step 2b: containerd --> Registry (HTTPS)
   GET /v2/library/nginx/manifests/1.25
   GET /v2/library/nginx/blobs/sha256:...  (for each layer)

Step 2c: containerd stores layers
   Snapshotter prepares overlay2 mount points

Step 3: Daemon creates container metadata
   - Assigns container ID
   - Registers name "web"
   - Configures port mapping 8080:80
   - Allocates writable layer

Step 4: Daemon --> containerd (gRPC)
   Create container with OCI bundle

Step 5: containerd --> runc
   Create namespaces, cgroups, rootfs
   Start nginx master process

Step 6: runc exits, shim supervises
   nginx is PID 1 inside the container
   shim is the parent process on the host
```

## Component Interaction: docker build

When you execute `docker build -t myapp:v1 .`:

```
Step 1: CLI packages build context
   - Reads .dockerignore
   - Creates tar archive of context directory
   - Sends tar to daemon via REST API

Step 2: Daemon delegates to BuildKit
   - Parses Dockerfile
   - Creates build graph (DAG of instructions)
   - Identifies parallelizable stages

Step 3: BuildKit executes instructions
   For each instruction:
   - Check layer cache (content-addressable)
   - If cache hit --> reuse layer
   - If cache miss --> execute instruction, create new layer

Step 4: BuildKit produces image
   - Assembles layers into image manifest
   - Stores in local image store
   - Tags as myapp:v1
```

## Layer Creation Walkthrough

Given this Dockerfile:

```dockerfile
FROM alpine:3.21                          # Layer 1: Base image layers
RUN apk add --no-cache curl              # Layer 2: +curl binary and libs
COPY config.json /app/config.json         # Layer 3: +config file
RUN mkdir -p /data && chown 1001 /data    # Layer 4: +data directory
USER 1001                                 # No layer (metadata only)
CMD ["myapp"]                             # No layer (metadata only)
```

Layer analysis:
```
Layer 1: alpine base     [~6 MB]  -- shared with all alpine-based images
Layer 2: +curl           [~2 MB]  -- added files from apk install
Layer 3: +config.json    [~1 KB]  -- single file added
Layer 4: +/data dir      [~0 KB]  -- empty directory + ownership change

Image config (not a layer):
  - USER: 1001
  - CMD: ["myapp"]
  - ENV: inherited from alpine
```

**Instructions that do NOT create layers:**
- `FROM` (references existing layers)
- `CMD`, `ENTRYPOINT` (metadata)
- `ENV`, `ARG` (metadata, though ENV creates a cache checkpoint)
- `EXPOSE`, `VOLUME`, `LABEL` (metadata)
- `USER`, `WORKDIR` (metadata)
- `STOPSIGNAL`, `SHELL`, `HEALTHCHECK` (metadata)

**Instructions that create layers:**
- `RUN` (executes command, captures filesystem diff)
- `COPY` (adds files from build context)
- `ADD` (adds files from context, URL, or Git repo)

## Container Isolation Example

Two containers from the same image have completely isolated environments:

```
Host System
├── dockerd (daemon)
├── containerd
│
├── Container A (from nginx:1.25)
│   ├── PID namespace:  PID 1 = nginx master
│   ├── Network:        172.17.0.2, port 80
│   ├── Mount:
│   │   ├── [read-only] Image layers (shared with B)
│   │   └── [writable]  Container A's own layer
│   ├── Hostname:       container-a-id
│   └── Cgroups:        512MB memory, 1.0 CPU
│
├── Container B (from nginx:1.25)
│   ├── PID namespace:  PID 1 = nginx master (different process!)
│   ├── Network:        172.17.0.3, port 80
│   ├── Mount:
│   │   ├── [read-only] Image layers (shared with A)
│   │   └── [writable]  Container B's own layer
│   ├── Hostname:       container-b-id
│   └── Cgroups:        256MB memory, 0.5 CPU
│
└── Shared Resources
    └── Image layers on disk (stored once, mounted read-only by both)
```

Key observations:
- Both containers share the same read-only image layers (zero disk duplication)
- Each container has its own writable layer (isolated writes)
- Each container has its own PID 1 (different nginx processes)
- Each container has its own IP address and network stack
- Resource limits are independent per container

## Network Architecture Example

```
Host Network Stack
│
├── docker0 (default bridge: 172.17.0.0/16)
│   ├── veth-a ←→ Container A eth0 (172.17.0.2)
│   └── veth-b ←→ Container B eth0 (172.17.0.3)
│       (NO DNS resolution between A and B)
│
├── br-mynet (user-defined bridge: 172.18.0.0/16)
│   ├── veth-c ←→ Container C eth0 (172.18.0.2)
│   └── veth-d ←→ Container D eth0 (172.18.0.3)
│       (DNS: C can reach D as "container-d-name")
│
└── iptables rules
    ├── NAT: Container outbound traffic masqueraded as host IP
    ├── FORWARD: Inter-container traffic on same bridge allowed
    └── DNAT: Published ports (-p 8080:80) forwarded to container
```

## Volume Architecture Example

```
Host Filesystem
│
├── /var/lib/docker/volumes/
│   ├── pgdata/
│   │   └── _data/           <-- Named volume "pgdata"
│   │       ├── base/
│   │       ├── global/
│   │       └── pg_wal/
│   └── app-logs/
│       └── _data/           <-- Named volume "app-logs"
│           └── app.log
│
├── Container: postgres
│   └── /var/lib/postgresql/data  --> mounted from pgdata volume
│       (reads/writes go directly to host, bypassing overlay2)
│
└── Container: app
    ├── /var/log/app              --> mounted from app-logs volume
    └── /app/                     --> overlay2 (container writable layer)
```

Key observations:
- Volume data bypasses the union filesystem entirely (no copy-on-write overhead)
- Volume data persists when the container is removed
- Multiple containers can mount the same volume simultaneously

## Build Context Transfer

```
Project Directory
├── src/
│   ├── main.go          (10 KB)  -- included
│   └── utils.go         (5 KB)   -- included
├── tests/
│   └── main_test.go     (8 KB)   -- excluded by .dockerignore
├── node_modules/         (500 MB) -- excluded by .dockerignore
├── .git/                 (50 MB)  -- excluded by .dockerignore
├── Dockerfile            (1 KB)   -- excluded by .dockerignore
├── .dockerignore         (1 KB)   -- processed first, not sent
└── go.mod               (1 KB)   -- included

Without .dockerignore: ~560 MB sent to daemon
With .dockerignore:    ~16 KB sent to daemon
```

The `.dockerignore` file:
```
.git
node_modules
tests
Dockerfile
.dockerignore
*.md
```

ALWAYS create a `.dockerignore` file. The build context is sent as a tar archive over the Docker API before any instruction executes. A 500MB context adds seconds to every build, even with full cache hits.

## Container Lifecycle Practical Example

```bash
# 1. CREATE: Allocate writable layer, set config, do not start
docker create --name myapp -p 8080:80 nginx:1.25
# State: Created | PID: none | Network: allocated but inactive

# 2. START: Execute the main process
docker start myapp
# State: Running | PID: active | Network: active, port 8080 mapped

# 3. PAUSE: Freeze all processes via cgroup freezer
docker pause myapp
# State: Paused | PID: frozen | Network: connections stall, no new responses

# 4. UNPAUSE: Resume all processes
docker unpause myapp
# State: Running | PID: active | Network: resumes normally

# 5. STOP: Send SIGTERM, wait 10s, then SIGKILL
docker stop myapp
# State: Stopped (Exited 0) | PID: none | Network: released
# Writable layer: PRESERVED (can be started again)

# 6. START again: Resume from stopped state
docker start myapp
# State: Running | PID: new process | Network: new IP possible

# 7. STOP and REMOVE: Delete the container
docker stop myapp
docker rm myapp
# State: Removed | Writable layer: DELETED | Anonymous volumes: DELETED
```

## OCI Bundle Structure

When containerd prepares a container for runc, it creates an OCI bundle:

```
/run/containerd/io.containerd.runtime.v2.task/<namespace>/<container-id>/
├── config.json          <-- OCI Runtime Spec configuration
│   ├── process          (command, args, env, cwd, user)
│   ├── root             (path to rootfs, readonly flag)
│   ├── mounts           (procfs, sysfs, devpts, tmpfs, volumes)
│   ├── linux
│   │   ├── namespaces   (pid, network, mount, uts, ipc)
│   │   ├── resources    (cgroups: memory, cpu, pids)
│   │   └── seccomp      (syscall filter profile)
│   └── hooks            (prestart, poststart, poststop)
└── rootfs/              <-- Union mount of image layers + writable layer
    ├── bin/
    ├── etc/
    ├── usr/
    └── ...
```
