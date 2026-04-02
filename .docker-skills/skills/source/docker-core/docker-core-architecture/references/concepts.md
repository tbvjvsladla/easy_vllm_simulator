# Docker Architecture Concepts

## Docker Object Types in Detail

### Images

An image is an **immutable, ordered collection of read-only filesystem layers** plus metadata (config, environment, entrypoint, labels).

**Key properties:**
- Images are built from a Dockerfile using `docker build`
- Each layer represents one filesystem change (file added, modified, or deleted)
- Layers are **content-addressable** -- identified by the SHA256 hash of their contents
- Layers are **shared** between images -- common base layers exist only once on disk
- Images are distributed via registries using the OCI Distribution Specification
- An image manifest lists all layers and the image config

**Image manifest structure:**
```
Image Manifest
├── Config (JSON)
│   ├── Environment variables
│   ├── Entrypoint / CMD
│   ├── Working directory
│   ├── User
│   ├── Exposed ports
│   ├── Labels
│   └── Layer diff IDs (ordered)
├── Layers (ordered list)
│   ├── Layer 1 digest + size
│   ├── Layer 2 digest + size
│   └── Layer N digest + size
└── Metadata
    ├── Schema version
    ├── Media type
    └── Platform (os/arch)
```

**Multi-platform images** use a manifest list (also called an index) that points to platform-specific manifests:
```
Manifest List (Index)
├── linux/amd64 --> Manifest A
├── linux/arm64 --> Manifest B
└── linux/arm/v7 --> Manifest C
```

### Containers

A container is a **runnable instance of an image** with its own writable layer, network stack, process space, and configuration.

**Key properties:**
- Created from an image using `docker run` or `docker create`
- Has a thin writable layer on top of the image's read-only layers
- The writable layer is deleted when the container is removed
- Multiple containers can share the same image without duplication
- Each container has its own isolated namespaces and cgroup limits
- Container state (created/running/paused/stopped) is tracked by the daemon

**Container filesystem model:**
```
+-----------------------------+
| Container Writable Layer    |  <-- Copy-on-write: files copied here only when modified
+-----------------------------+
| Image Layer N (read-only)   |
+-----------------------------+
| ...                         |
+-----------------------------+
| Image Layer 1 (read-only)   |
+-----------------------------+
```

When a container reads a file, Docker searches layers top-down and returns the first match. When a container modifies a file, the file is first copied from the read-only layer to the writable layer (copy-on-write), then modified in place.

### Networks

A Docker network provides **isolated communication channels** between containers.

**Key properties:**
- Containers on the same user-defined network can resolve each other by name (automatic DNS)
- Containers on the default bridge network can ONLY communicate via IP address
- A container can connect to multiple networks simultaneously
- Network isolation means containers on different networks cannot communicate
- ALWAYS use user-defined bridge networks, not the default bridge

**Network types:**
| Type | Isolation | DNS | Use Case |
|------|-----------|-----|----------|
| bridge (user-defined) | Per-network | Automatic | Standard container communication |
| bridge (default) | Weak | None | Legacy, avoid |
| host | None | Host DNS | Performance-critical, no port mapping needed |
| none | Complete | None | Fully isolated containers |
| overlay | Multi-host | Automatic | Swarm services across hosts |
| macvlan | Appears as physical device | Network-level | Direct LAN integration |

### Volumes

A volume is a **Docker-managed persistent storage** mechanism that exists outside the container's union filesystem.

**Key properties:**
- Volumes persist independently of any container lifecycle
- Stored in `/var/lib/docker/volumes/` on the host (by default)
- Can be shared between multiple containers simultaneously
- Support different drivers (local, NFS, cloud storage plugins)
- Named volumes are explicitly created and referenced by name
- Anonymous volumes are created automatically and identified by a random hash

**Storage comparison:**
| Storage Type | Managed By | Persists After `docker rm` | Shared Between Containers | Performance |
|-------------|------------|---------------------------|--------------------------|-------------|
| Container writable layer | Docker (overlay2) | No | No | Good (copy-on-write overhead) |
| Named volume | Docker | Yes | Yes | Best (direct mount) |
| Bind mount | Host filesystem | Yes (host file) | Yes | Best (direct access) |
| tmpfs | Kernel (memory) | No | No | Fastest (RAM) |

---

## Union Filesystem (overlay2)

Docker Engine 24+ uses **overlay2** as the default storage driver. overlay2 implements a union filesystem that merges multiple directories into a single coherent view.

### overlay2 Architecture

```
Container View (merged)     What the container process sees
        |
        +-- merged/         Unified view of all layers
        |
        +-- diff/           Container's writable layer (upperdir)
        |
        +-- work/           Internal overlay2 work directory
        |
        +-- lower           Pointer to read-only image layers (lowerdir)
             |
             +-- Layer N
             +-- ...
             +-- Layer 1
```

### Copy-on-Write (CoW) Mechanics

| Operation | What Happens |
|-----------|-------------|
| Read file | Search layers top-down, return first match |
| Create new file | Write directly to the writable (upper) layer |
| Modify existing file | Copy entire file from lower layer to upper layer, then modify |
| Delete file | Create a **whiteout** file in upper layer (character device `0:0`) |
| Delete directory | Create an **opaque whiteout** by setting `trusted.overlay.opaque=y` xattr |

### Performance Implications

- **First write** to an existing file incurs a copy-up cost (entire file copied to writable layer)
- Large files (databases, logs) should ALWAYS use volumes to avoid copy-on-write overhead
- The more layers an image has, the deeper the search path for file lookups
- overlay2 supports up to 128 lower layers

### Layer Sharing

```
Image A: [Base] [Layer 1] [Layer 2]
Image B: [Base] [Layer 1] [Layer 3]

On disk:  [Base] [Layer 1] [Layer 2] [Layer 3]
                                      ^
                     Layer 1 and Base stored only once
```

This sharing means pulling a new image that shares a base with an existing image downloads only the new layers. `docker system df -v` shows the "Shared Size" for each image.

---

## Docker Daemon (dockerd)

The Docker daemon is the **central management process** that:

1. **Exposes the Docker API** -- REST API over Unix socket (`/var/run/docker.sock`) or TCP
2. **Manages images** -- Build, pull, push, tag, inspect, remove
3. **Manages containers** -- Create, start, stop, remove, inspect
4. **Manages networks** -- Create, connect, disconnect, remove
5. **Manages volumes** -- Create, inspect, remove
6. **Delegates runtime operations** to containerd via gRPC

### Daemon Configuration

Configuration is set in `/etc/docker/daemon.json`:
```json
{
  "storage-driver": "overlay2",
  "log-driver": "json-file",
  "log-opts": {
    "max-size": "10m",
    "max-file": "3"
  },
  "default-address-pools": [
    {"base": "172.17.0.0/16", "size": 24}
  ]
}
```

NEVER set the same option in both `daemon.json` and CLI flags -- the daemon will fail to start with a conflict error.

---

## containerd

containerd is the **container runtime supervisor** that sits between the Docker daemon and runc:

1. **Pulls and pushes images** using the OCI Distribution Specification
2. **Manages image layer snapshots** using the overlay2 snapshotter
3. **Prepares the OCI bundle** (rootfs + runtime config) for runc
4. **Supervises container processes** after runc exits
5. **Provides the shim process** that keeps the container running independently of containerd restarts

### The Shim Process

When containerd starts a container, it creates a **shim** process (`containerd-shim-runc-v2`) that:
- Becomes the parent of the container process
- Keeps STDIO open for the container
- Reports exit status back to containerd
- Allows containerd to restart without affecting running containers

---

## runc

runc is the **OCI reference runtime** -- a lightweight binary that creates and starts containers:

1. Receives an OCI bundle (rootfs directory + `config.json`)
2. Creates Linux namespaces (PID, network, mount, UTS, IPC, user)
3. Configures cgroups for resource limits
4. Sets up the root filesystem using pivot_root or chroot
5. Starts the container process
6. **Exits** -- runc does NOT supervise the container; the shim takes over

runc is replaceable with any OCI-compliant runtime (e.g., crun, kata-containers, gVisor/runsc).

---

## OCI Standards

The **Open Container Initiative (OCI)** defines three specifications that Docker follows:

### OCI Image Specification
- Defines the format for container images
- An image is a manifest, a config, and an ordered set of filesystem layers
- Layers are tar archives, optionally compressed (gzip, zstd)
- Content-addressable storage using SHA256 digests

### OCI Runtime Specification
- Defines how to run a "filesystem bundle"
- A bundle is a directory containing a `config.json` and a `rootfs/` directory
- `config.json` specifies: process, root filesystem, mounts, namespaces, cgroups, capabilities
- Defines lifecycle operations: create, start, kill, delete
- Defines standard hooks: prestart, poststart, poststop

### OCI Distribution Specification
- Defines the API for distributing container images
- Registry endpoints: `/v2/`, `/v2/<name>/manifests/<ref>`, `/v2/<name>/blobs/<digest>`
- Supports content discovery, pull, push, and deletion
- Used by Docker Hub, GitHub Container Registry, AWS ECR, and all OCI-compliant registries
