# Storage Anti-Patterns

Common Docker storage mistakes, why they fail, and the correct approach.

---

## AP-01: Anonymous Volumes for Database Data

### The Mistake

```bash
# Anonymous volume -- no name specified
docker run -d --rm postgres:16
```

### Why It Fails

Anonymous volumes receive a random hash as their name. When the container is removed (especially with `--rm`), the anonymous volume is also removed. All database data is permanently lost.

### Correct Approach

```bash
# ALWAYS use named volumes for database data
docker run -d --name postgres \
  --mount source=pgdata,target=/var/lib/postgresql/data \
  -e POSTGRES_PASSWORD=secret \
  postgres:16
```

---

## AP-02: Using -v Syntax for Volume Drivers

### The Mistake

```bash
# -v does NOT support volume driver options
docker run -v nfs-data:/data nginx
# No way to specify NFS options with -v
```

### Why It Fails

The `-v` flag only supports three colon-separated fields: `name:path:options`. It cannot pass `volume-driver`, `volume-opt`, or other advanced mount options.

### Correct Approach

```bash
# ALWAYS use --mount for volume drivers
docker run --mount type=volume,volume-driver=local,src=nfs-data,dst=/data,\
volume-opt=type=nfs,volume-opt=device=:/nfs/share,volume-opt=o=addr=10.0.0.1 nginx
```

---

## AP-03: Wrong Mount Path for Database

### The Mistake

```bash
# Wrong path -- PostgreSQL data is NOT at /data
docker run -d \
  --mount source=pgdata,target=/data \
  postgres:16
```

### Why It Fails

Each database stores data at a specific path. Mounting a volume to the wrong path means the database writes to the container's ephemeral filesystem instead, and data is lost on container removal.

### Correct Approach

ALWAYS verify the data directory for each database:

| Database | Correct Path |
|----------|-------------|
| PostgreSQL | `/var/lib/postgresql/data` |
| MySQL/MariaDB | `/var/lib/mysql` |
| MongoDB | `/data/db` |
| Redis | `/data` |
| Elasticsearch | `/usr/share/elasticsearch/data` |

---

## AP-04: Bind Mount Auto-Creates Missing Directories

### The Mistake

```bash
# -v silently creates /nonexistent/path as an empty directory
docker run -v /nonexistent/path:/data myapp
```

### Why It Fails

With `-v` syntax, Docker auto-creates missing host directories. The application receives an empty directory instead of the expected data, leading to silent data loss or startup errors that are difficult to diagnose.

### Correct Approach

```bash
# --mount raises an error if the host path does not exist
docker run --mount type=bind,src=/host/data,dst=/data myapp
# Error: bind mount source path does not exist: /host/data
```

ALWAYS use `--mount` for bind mounts. The explicit error prevents silent failures.

---

## AP-05: Mounting /var/lib/docker Inside a Container

### The Mistake

```bash
docker run -v /var/lib/docker:/var/lib/docker myapp
```

### Why It Fails

Mounting Docker's internal storage directory causes filesystem handle conflicts. Containers that bind-mount this path hold open handles that prevent Docker from cleaning up resources, producing `Unable to remove filesystem` errors.

### Correct Approach

NEVER mount `/var/lib/docker` into containers. If you need Docker access inside a container (Docker-in-Docker), use the Docker socket:

```bash
# Docker socket (for Docker CLI access, not storage)
docker run -v /var/run/docker.sock:/var/run/docker.sock myapp
```

---

## AP-06: No Backup Strategy for Volumes

### The Mistake

Relying on Docker volumes as the sole copy of important data without any backup procedure.

### Why It Fails

`docker volume prune` removes all unused volumes. `docker system prune --volumes` does the same. A single accidental command can destroy all database data.

### Correct Approach

```bash
# Regular volume backup
docker run --rm \
  --mount source=pgdata,target=/data,readonly \
  -v /backups:/backup \
  alpine tar czf /backup/pgdata-$(date +%Y%m%d).tar.gz -C /data .

# Label volumes that need backup
docker volume create --label backup=true pgdata
```

ALWAYS implement automated backup for production volumes. ALWAYS label volumes to distinguish critical from disposable data.

---

## AP-07: Blind docker system prune --volumes on Production

### The Mistake

```bash
# Removes ALL unused volumes including database data
docker system prune -a --volumes -f
```

### Why It Fails

If a database container is stopped (not running), its volume is considered "unused" and is removed. This permanently destroys production data.

### Correct Approach

```bash
# 1. Check what will be removed FIRST
docker system df -v
docker volume ls -f dangling=true

# 2. Remove only specific resources
docker container prune -f
docker image prune -f

# 3. Remove volumes ONLY after verifying
docker volume rm specific-volume-name

# 4. Use label-based filtering
docker volume prune --filter "label!=keep" -f
```

---

## AP-08: UID/GID Mismatch on Bind Mounts

### The Mistake

```bash
# Container runs as UID 1000, host files owned by UID 0
docker run -v /host/data:/data myapp
# Result: Permission denied
```

### Why It Fails

Bind mounts map host files directly into the container. If the container process runs as a different UID/GID than the file owner on the host, permission errors occur.

### Correct Approach

```bash
# Option 1: Match container user to host UID
docker run -u $(id -u):$(id -g) -v /host/data:/data myapp

# Option 2: Set ownership in Dockerfile
RUN chown -R 1001:1001 /data
USER 1001:1001

# Option 3: Use named volumes (Docker manages permissions)
docker run --mount source=appdata,dst=/data myapp
```

Named volumes avoid UID/GID issues because Docker manages the filesystem permissions.

---

## AP-09: Storing Secrets in Volumes

### The Mistake

```bash
# Secret file persisted in a volume
docker run -v secrets:/run/secrets myapp
echo "password123" | docker exec -i myapp tee /run/secrets/db_password
```

### Why It Fails

Volume data persists on disk and survives container removal. Secrets in volumes can be accessed by any container that mounts the volume and remain on disk even after the container is deleted.

### Correct Approach

```bash
# Use tmpfs for runtime secrets -- never written to disk
docker run --mount type=tmpfs,dst=/run/secrets myapp

# Or use Docker secrets (Swarm mode)
echo "password123" | docker secret create db_password -
docker service create --secret db_password myapp
```

---

## AP-10: Not Using read_only for Consumer Containers

### The Mistake

```yaml
services:
  writer:
    volumes:
      - shared:/data
  reader:
    volumes:
      - shared:/data      # Read-write, but only reads
```

### Why It Fails

Without explicit read-only access, any bug in the reader service can corrupt shared data. Multiple writers to the same volume without coordination causes data corruption.

### Correct Approach

```yaml
services:
  writer:
    volumes:
      - shared:/data
  reader:
    volumes:
      - shared:/data:ro    # Explicitly read-only
```

ALWAYS mount volumes as read-only for services that do not need write access.

---

## AP-11: Ignoring Volume Cleanup in CI/CD

### The Mistake

Running `docker compose up` and `docker compose down` in CI/CD pipelines without `--volumes`, leaving orphaned volumes after every build.

### Why It Fails

Each CI/CD run creates new anonymous volumes. Over time, these consume all available disk space on the CI runner, causing `no space left on device` errors.

### Correct Approach

```bash
# ALWAYS clean up volumes in CI/CD
docker compose down --volumes --remove-orphans

# Or use docker system prune in CI cleanup
docker system prune -a --volumes -f
```

In CI/CD environments (where data persistence is not needed), ALWAYS include `--volumes` in cleanup commands.

---

## AP-12: Using VOLUME in Dockerfile Without Intent

### The Mistake

```dockerfile
# Creates an anonymous volume at /data for EVERY container
VOLUME /data
```

### Why It Fails

The `VOLUME` instruction in a Dockerfile forces Docker to create an anonymous volume at that path for every container created from the image. This:
- Prevents overriding with a bind mount in some edge cases
- Creates orphaned anonymous volumes that consume disk space
- Cannot be un-declared by downstream images

### Correct Approach

Do NOT use `VOLUME` in Dockerfiles unless you have a specific technical reason. Let users choose their mount strategy at runtime:

```bash
# User decides the mount type at runtime
docker run --mount source=mydata,dst=/data myapp
```

---

## Summary: Storage Do's and Don'ts

| Do | Don't |
|----|-------|
| Use named volumes for databases | Use anonymous volumes for important data |
| Use `--mount` syntax in production | Use `-v` with volume drivers |
| Verify mount paths match app data dirs | Assume generic paths like `/data` |
| Use `--mount` for bind mounts (safe errors) | Use `-v` for bind mounts (auto-creates) |
| Implement automated volume backups | Rely on volumes as sole data copy |
| Check `docker system df` before pruning | Run `docker system prune --volumes` blindly |
| Match container UID to host file ownership | Ignore UID/GID on bind mounts |
| Use tmpfs for secrets | Store secrets in persistent volumes |
| Mount read-only for consumers | Give write access to all consumers |
| Clean volumes in CI/CD pipelines | Leave orphaned volumes after builds |
| Let users choose mounts at runtime | Use `VOLUME` in Dockerfile without intent |
