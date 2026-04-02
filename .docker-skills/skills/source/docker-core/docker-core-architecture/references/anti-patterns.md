# Docker Architecture Anti-Patterns

## AP-001: Treating Containers as Persistent VMs

**Problem:** Treating containers like virtual machines -- logging in via `docker exec`, installing packages manually, making configuration changes, and expecting them to persist.

**Why it fails:** The writable container layer is ephemeral. When the container is removed, ALL changes are lost. Manual changes are unreproducible and invisible to other team members.

**Correction:**
- ALWAYS define the entire environment in a Dockerfile
- ALWAYS use `docker build` to create reproducible images
- NEVER rely on manual `docker exec` changes in production
- Use volumes for data that must persist beyond the container lifecycle

---

## AP-002: Using docker commit for Production Images

**Problem:** Making changes inside a running container and using `docker commit` to create new images.

```bash
# BAD
docker exec myapp apt-get install -y curl
docker commit myapp myapp:with-curl
```

**Why it fails:**
- No Dockerfile means no audit trail and no reproducibility
- Cannot rebuild the image automatically in CI/CD
- Layer content is opaque -- impossible to review what changed
- Accumulates unnecessary files and metadata over time

**Correction:**
- ALWAYS use a Dockerfile to define image contents
- ALWAYS build images via `docker build` or `docker buildx build`
- Treat images as build artifacts, not mutable state

---

## AP-003: Storing Data in the Container Layer

**Problem:** Writing application data (databases, uploads, logs) to the container's writable layer instead of a volume.

```bash
# BAD: Database data stored in container layer
docker run -d --name postgres postgres:16
# All data lost when container is removed
```

**Why it fails:**
- Container removal deletes the writable layer and all data
- Copy-on-write overhead degrades I/O performance for write-heavy workloads
- Data cannot be shared between containers
- Backup and migration become extremely difficult

**Correction:**
```bash
# GOOD: Named volume for persistent data
docker run -d --name postgres \
  --mount source=pgdata,target=/var/lib/postgresql/data \
  postgres:16
```

- ALWAYS use named volumes for database data, uploads, and other persistent state
- ALWAYS use tmpfs mounts for sensitive temporary data (secrets, session files)
- NEVER store important data in the container writable layer

---

## AP-004: Ignoring the Build Context

**Problem:** Running `docker build` without a `.dockerignore` file, sending gigabytes of unnecessary files to the daemon.

**Why it fails:**
- The entire build context directory is packaged as a tar and sent to the daemon
- `node_modules/` (500MB+), `.git/` (50MB+), and build artifacts waste time and bandwidth
- Large contexts cause multi-second delays on EVERY build, even with full cache hits
- Sensitive files (`.env`, private keys) can accidentally end up in the image

**Correction:**
- ALWAYS create a `.dockerignore` file in every project with a Dockerfile
- Exclude: `.git`, `node_modules`, `dist`, `build`, `*.md`, `.env`, `*.pem`, IDE configs
- Monitor context size with `docker build` output: "Sending build context to Docker daemon  XX.XXB"

---

## AP-005: Running All Processes as Root

**Problem:** Running the container process as root (the default) when root privileges are not required.

**Why it fails:**
- Container root maps to host root by default (unless user namespaces are enabled)
- A container escape vulnerability gives the attacker root access to the host
- File permission issues when mounting volumes -- files created by root in the container are owned by root on the host

**Correction:**
```dockerfile
# ALWAYS create a non-root user
RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser
USER appuser
```

- ALWAYS add a `USER` instruction in the Dockerfile
- ALWAYS assign explicit UID/GID for deterministic behavior
- NEVER use `--privileged` in production -- use specific `--cap-add` instead

---

## AP-006: Not Setting Resource Limits

**Problem:** Running containers without memory or CPU limits, allowing a single container to consume all host resources.

```bash
# BAD: No resource limits
docker run -d nginx
```

**Why it fails:**
- A memory leak in one container can trigger the OOM killer on the host, affecting ALL containers
- A CPU-intensive container can starve other containers of processing time
- A fork bomb can exhaust the PID table for the entire host

**Correction:**
```bash
# GOOD: Explicit resource limits
docker run -d \
  -m 512m \
  --cpus 1.0 \
  --pids-limit 200 \
  nginx
```

- ALWAYS set memory limits (`-m`) in production
- ALWAYS set CPU limits (`--cpus`) in production
- ALWAYS set PID limits (`--pids-limit`) to prevent fork bombs

---

## AP-007: Using the Default Bridge Network

**Problem:** Relying on the default `bridge` network for container communication.

**Why it fails:**
- No automatic DNS resolution -- containers can only reach each other by IP address
- IP addresses change on container restart -- hardcoded IPs break
- ALL containers on the default bridge can reach each other -- no isolation
- The legacy `--link` flag is deprecated and unreliable

**Correction:**
```bash
# GOOD: User-defined bridge with automatic DNS
docker network create mynet
docker run -d --name web --network mynet nginx
docker run -d --name api --network mynet myapi
# api can reach web as "web" via DNS
```

- ALWAYS create user-defined bridge networks
- NEVER use `--link` for container communication
- NEVER hardcode container IP addresses

---

## AP-008: Monolithic Containers

**Problem:** Running multiple services (web server + database + cache) inside a single container.

**Why it fails:**
- Cannot scale services independently
- Failure of one service kills all services in the container
- Cannot update one service without restarting all others
- Signal handling becomes complex with multiple PID 1 candidates
- Violates the single-responsibility principle for containers

**Correction:**
- ALWAYS run one primary process per container
- Use Docker Compose or orchestration to manage multi-service applications
- Connect services via Docker networks
- Use shared volumes for inter-service data when needed

---

## AP-009: Conflating Image Tags with Immutability

**Problem:** Assuming that `nginx:1.25` will always refer to the exact same image.

**Why it fails:**
- Tags are **mutable pointers** -- the registry maintainer can push a new image under the same tag
- `latest` is especially dangerous -- it changes with every new release
- Security patches often update tagged images without changing the tag
- Two `docker pull` commands at different times can return different images for the same tag

**Correction:**
```dockerfile
# Pin by digest for production
FROM nginx:1.25@sha256:a8560b36e8b8210634f77d9f7f9efd7ffa463e380b75e2e74aff4511df3ef88c
```

- ALWAYS use digest pinning for production Dockerfiles
- NEVER use `latest` in production
- Use tags for development convenience, digests for production determinism

---

## AP-010: Exposing the Docker Daemon API Without TLS

**Problem:** Binding the Docker daemon to a TCP port without TLS encryption and authentication.

```json
{
  "hosts": ["tcp://0.0.0.0:2375"]
}
```

**Why it fails:**
- Docker API access is equivalent to root access on the host
- Anyone who can reach the TCP port can create privileged containers
- Attackers can mount the host root filesystem and gain full control
- Cryptocurrency mining botnets actively scan for exposed Docker APIs

**Correction:**
- NEVER expose the Docker daemon on TCP without TLS mutual authentication
- ALWAYS use Unix socket (`/var/run/docker.sock`) for local access
- ALWAYS use SSH tunneling for remote access: `DOCKER_HOST=ssh://user@host`
- If TCP is required, ALWAYS configure TLS with client certificates

---

## AP-011: Misunderstanding Layer Caching

**Problem:** Placing frequently-changing files before expensive operations in the Dockerfile.

```dockerfile
# BAD: Any source code change invalidates the npm install cache
FROM node:20
WORKDIR /app
COPY . .
RUN npm install
```

**Why it fails:**
- Docker layer caching is sequential -- once a layer's cache is invalidated, ALL subsequent layers must rebuild
- Copying source code (which changes frequently) before installing dependencies (which change rarely) means `npm install` runs on every build

**Correction:**
```dockerfile
# GOOD: Dependencies cached separately from source code
FROM node:20
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
```

- ALWAYS order Dockerfile instructions from least to most frequently changed
- ALWAYS separate dependency installation from source code copying
- Use `--mount=type=cache` for package manager caches

---

## AP-012: Using docker save/load Instead of a Registry

**Problem:** Distributing images by saving to tar files and loading on target machines.

**Why it fails:**
- No version control or tagging at the distribution level
- No layer sharing between images (full image transferred every time)
- No security scanning or vulnerability assessment in the pipeline
- Manual process prone to errors and stale images

**Correction:**
- ALWAYS use a container registry (Docker Hub, GitHub Container Registry, AWS ECR) for image distribution
- Use `docker push` and `docker pull` for image transfer
- Implement CI/CD pipelines that build, scan, and push images automatically
- Reserve `docker save`/`load` for air-gapped environments only
