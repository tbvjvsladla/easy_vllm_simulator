---
name: docker-impl-storage
description: >
  Use when persisting container data, mounting host directories, or
  configuring database storage volumes. Prevents data loss from anonymous
  volumes on container removal and silent mount failures with -v syntax.
  Covers named volumes, bind mounts, tmpfs, --mount vs -v, volume drivers,
  NFS, CIFS, backup/restore, and Compose volume integration.
  Keywords: docker volume, bind mount, tmpfs, --mount, -v, NFS, docker
  volume create, volumes-from, data loss, persist data, database volume,
  where is my data, files disappear after restart.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-impl-storage

## Quick Reference

### Storage Types

| Type | Persistence | Managed By | Location | Use Case |
|------|-------------|------------|----------|----------|
| **Named volume** | Yes | Docker (`/var/lib/docker/volumes/`) | Docker-managed | Databases, shared data, backups |
| **Anonymous volume** | Until container removed | Docker | Docker-managed | Temporary per-container data |
| **Bind mount** | Yes | Host filesystem | Any host path | Development, config injection |
| **tmpfs** | No (RAM only) | Kernel | Memory | Secrets, temp files, performance |

### Critical Warnings

**NEVER** use anonymous volumes for database data -- data is lost when the container is removed with `--rm`. ALWAYS use named volumes for any data that must survive container recreation.

**NEVER** use `-v` syntax with volume drivers or driver options -- `-v` does not support them. ALWAYS use `--mount` when configuring volume drivers, NFS, or CIFS mounts.

**NEVER** run `docker system prune -a --volumes` on production systems without first checking `docker volume ls` -- this removes ALL unused volumes including database data.

**NEVER** mount `/var/lib/docker/` as a bind mount inside a container -- this causes filesystem handle conflicts and `Unable to remove filesystem` errors.

**ALWAYS** use `--mount` syntax for production workloads and documentation -- it is explicit, self-documenting, and supports all mount options.

**ALWAYS** verify mount destination paths match the application's data directory -- mounting to the wrong path silently obscures existing container data.

---

## Mount Type Decision Tree

```
Need to persist data?
├── NO → tmpfs mount (RAM-only, fastest, lost on stop)
│       docker run --mount type=tmpfs,dst=/tmp,tmpfs-size=64m IMAGE
│
└── YES → Need Docker to manage the storage?
    ├── YES → Named volume (portable, backupable, driver support)
    │       docker run --mount type=volume,src=mydata,dst=/data IMAGE
    │
    └── NO → Need host filesystem access?
        ├── YES → Bind mount (direct host path access)
        │       docker run --mount type=bind,src=/host/path,dst=/app IMAGE
        │
        └── NO → Named volume (default choice for persistence)
```

### When to Use Each Type

| Scenario | Mount Type | Reason |
|----------|-----------|--------|
| Database storage | Named volume | Survives container lifecycle, portable |
| Development source code | Bind mount | Live editing from host |
| Config file injection | Bind mount (read-only) | Host-managed configuration |
| Temporary build artifacts | tmpfs | Fast, no disk I/O, auto-cleaned |
| Secrets at runtime | tmpfs | Never written to disk |
| Shared data between containers | Named volume | Multiple containers mount same volume |
| NFS/CIFS network storage | Named volume + driver | Volume drivers handle network mounts |
| CI/CD build cache | Named volume | Persists between pipeline runs |

---

## --mount vs -v Syntax Comparison

### --mount Syntax (Preferred)

Key-value pairs, explicit and self-documenting:

```bash
# Named volume
docker run --mount type=volume,src=mydata,dst=/data nginx

# Bind mount
docker run --mount type=bind,src=/host/path,dst=/container/path nginx

# tmpfs
docker run --mount type=tmpfs,dst=/tmp,tmpfs-size=64m nginx

# Read-only volume
docker run --mount source=data,destination=/data,readonly nginx

# Volume with subdirectory
docker run --mount src=logs,dst=/var/log/app1,volume-subpath=app1 app1
```

### -v Syntax (Quick Development Only)

Three colon-separated fields: `[name:]container-path[:options]`

```bash
docker run -v mydata:/data nginx          # Named volume
docker run -v /host/path:/app nginx       # Bind mount
docker run -v mydata:/data:ro nginx       # Read-only
```

### Key Differences

| Feature | `--mount` | `-v` |
|---------|-----------|------|
| Syntax | Key-value pairs | Colon-separated positional |
| Volume drivers | Supported | NOT supported |
| Volume options | Supported | NOT supported |
| Missing host dir (bind) | Error (safe) | Auto-creates (silent) |
| Clarity | Self-documenting | Position-dependent |
| Subpath mounting | Supported | NOT supported |

### --mount Option Reference

| Option | Applies To | Description |
|--------|-----------|-------------|
| `type` | All | `volume`, `bind`, or `tmpfs` |
| `source` / `src` | volume, bind | Volume name or host path |
| `destination` / `dst` / `target` | All | Container mount path |
| `readonly` / `ro` | volume, bind | Read-only access |
| `volume-subpath` | volume | Mount subdirectory within volume |
| `volume-nocopy` | volume | Skip copying container data into empty volume |
| `volume-opt` | volume | Driver-specific options (repeatable) |
| `volume-driver` | volume | Volume driver name |
| `tmpfs-size` | tmpfs | Size limit in bytes |
| `tmpfs-mode` | tmpfs | File mode (e.g., 1770) |

---

## Volume Lifecycle

### Auto-Population Behavior

When mounting an **empty** named volume to a container directory that has existing files, Docker copies those files into the volume:

```bash
# Nginx HTML files get copied into nginx-vol on first mount
docker run -d --mount source=nginx-vol,destination=/usr/share/nginx/html nginx
```

**NEVER** rely on auto-population for production data -- it only happens once when the volume is empty. Use explicit initialization instead.

### Named vs Anonymous Volumes

| Aspect | Named Volume | Anonymous Volume |
|--------|-------------|-----------------|
| Creation | Explicit name provided | Auto-generated hash ID |
| Reuse | Easy to reference by name | Must use random ID |
| Cleanup with `--rm` | Persists | Auto-removed |
| Sharing | Easy between containers | Difficult |
| Backup | Straightforward | Error-prone |

### Simultaneous Mounting

Multiple containers can mount the same volume simultaneously. ALWAYS use read-only mounts for consumers that do not need write access:

```bash
# Writer container
docker run --mount source=shared,dst=/data mywriter

# Reader container (read-only)
docker run --mount source=shared,dst=/data,readonly myreader
```

---

## Volume Drivers

### Local Driver (Default)

Stores data on the host filesystem. Supports NFS, CIFS, and block devices via options.

### NFS Volumes

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

### CIFS/SMB Volumes

```bash
docker volume create --driver local \
  --opt type=cifs \
  --opt device=//server.example.com/backup \
  --opt o=addr=server.example.com,username=user,password=pass,file_mode=0777,dir_mode=0777 \
  --name cifs-vol
```

The `addr` option is REQUIRED when using a hostname instead of an IP address.

### Third-Party Volume Drivers

```bash
# Install plugin
docker plugin install --grant-all-permissions rclone/docker-volume-rclone

# Create volume with plugin -- MUST use --mount syntax
docker run --mount type=volume,volume-driver=rclone,src=remote-vol,target=/app nginx
```

---

## Database Persistence Patterns

### PostgreSQL

```bash
docker run -d --name postgres \
  --mount source=pgdata,target=/var/lib/postgresql/data \
  -e POSTGRES_PASSWORD=secret \
  postgres:16
```

### MySQL

```bash
docker run -d --name mysql \
  --mount source=mysqldata,target=/var/lib/mysql \
  -e MYSQL_ROOT_PASSWORD=secret \
  mysql:8
```

### MongoDB

```bash
docker run -d --name mongo \
  --mount source=mongodata,target=/data/db \
  mongo:7
```

ALWAYS use named volumes for database containers. See [references/examples.md](references/examples.md) for Compose patterns and backup procedures.

---

## Storage Cleanup Strategy

```bash
# 1. Assess current usage FIRST
docker system df -v

# 2. List dangling volumes (not attached to any container)
docker volume ls -f dangling=true

# 3. Remove specific unused volumes
docker volume rm VOLUME_NAME

# 4. Remove ALL unused anonymous volumes
docker volume prune -f

# 5. Targeted prune (exclude labeled volumes)
docker volume prune --filter "label!=keep"

# 6. Full system cleanup (DANGEROUS on production)
docker system prune -a --volumes -f
```

**ALWAYS** run `docker system df` before pruning to understand what consumes space. **NEVER** run `docker volume prune` on production without verifying which volumes are unused.

---

## Compose Volume Integration

```yaml
services:
  db:
    image: postgres:16
    volumes:
      - pgdata:/var/lib/postgresql/data

volumes:
  pgdata:                    # Named volume (managed by Compose)
```

See [references/examples.md](references/examples.md) for NFS volumes, external volumes, and multi-service patterns in Compose.

---

## Reference Links

- [references/mount-types.md](references/mount-types.md) -- Complete comparison of volumes, bind mounts, and tmpfs with all options
- [references/examples.md](references/examples.md) -- Database persistence, backup/restore, NFS volumes, Compose volume patterns
- [references/anti-patterns.md](references/anti-patterns.md) -- Common storage mistakes and how to avoid them

### Official Sources

- https://docs.docker.com/engine/storage/
- https://docs.docker.com/engine/storage/volumes/
- https://docs.docker.com/reference/cli/docker/volume/
- https://docs.docker.com/compose/how-tos/volumes/
