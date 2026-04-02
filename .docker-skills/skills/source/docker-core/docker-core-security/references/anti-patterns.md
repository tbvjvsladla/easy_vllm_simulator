# Docker Security Anti-Patterns

Each anti-pattern includes: the mistake, why it is dangerous, and the correct alternative.

---

## AP-01: Running as Root by Default

**Anti-pattern:**
```dockerfile
FROM node:20
WORKDIR /app
COPY . .
RUN npm ci
CMD ["node", "server.js"]
# No USER instruction -- runs as root (UID 0)
```

**Why dangerous:** If an attacker exploits a vulnerability in the application, they gain root access inside the container. Combined with a container escape vulnerability, this means root on the host.

**Correct:**
```dockerfile
FROM node:20-alpine
WORKDIR /app
COPY --chown=1001:1001 . .
RUN addgroup -g 1001 -S appgroup && adduser -u 1001 -S appuser -G appgroup
RUN npm ci --production
USER 1001:1001
CMD ["node", "server.js"]
```

---

## AP-02: Secrets in Image Layers

**Anti-pattern:**
```dockerfile
# Secrets persist in image history even if deleted later
COPY credentials.json /app/
RUN ./setup.sh --config /app/credentials.json
RUN rm /app/credentials.json  # Still in previous layer!
```

```dockerfile
# Secrets visible via docker inspect
ENV API_KEY=sk-live-abc123def456
ARG DB_PASSWORD=mysecretpassword
```

**Why dangerous:** Image layers are immutable. Anyone with `docker pull` access can run `docker history --no-trunc` to see `ENV`/`ARG` values or extract deleted files from earlier layers.

**Correct:**
```dockerfile
# syntax=docker/dockerfile:1
# Build-time secret (not stored in any layer)
RUN --mount=type=secret,id=api_key \
    cat /run/secrets/api_key | ./setup.sh --api-key-stdin

# Runtime: pass via environment variable or Docker secret
# docker run -e API_KEY="$(cat ~/.api_key)" myapp
```

---

## AP-03: Using --privileged

**Anti-pattern:**
```bash
# "It works with --privileged" is not a valid solution
docker run --privileged myapp
```

**Why dangerous:** `--privileged` grants ALL Linux capabilities, access to ALL host devices (`/dev/*`), and disables seccomp, AppArmor, and SELinux. The container effectively has full root access to the host kernel.

**Correct:**
```bash
# Identify the exact capability needed and grant only that
docker run --cap-drop ALL --cap-add SYS_PTRACE myapp

# If device access is needed, mount the specific device
docker run --device /dev/snd myapp
```

---

## AP-04: Using :latest Tag in Production

**Anti-pattern:**
```dockerfile
FROM python:latest
```
```yaml
services:
  app:
    image: nginx:latest
```

**Why dangerous:** `:latest` is a mutable tag. It points to different images over time. A `docker pull` on Monday and Tuesday may yield different images with different vulnerabilities or breaking changes. Builds are not reproducible.

**Correct:**
```dockerfile
# Pin to specific version
FROM python:3.12.1-slim

# Best: pin to digest for immutable reference
FROM python@sha256:abc123def456...
```

---

## AP-05: Exposing Docker Daemon Over TCP Without TLS

**Anti-pattern:**
```json
// daemon.json
{
  "hosts": ["tcp://0.0.0.0:2375"]
}
```

**Why dangerous:** The Docker API is equivalent to root access on the host. Anyone who can reach port 2375 can create privileged containers, mount the host filesystem, or deploy cryptocurrency miners. This is actively scanned for by botnets.

**Correct:**
```bash
# Use SSH tunneling (recommended)
export DOCKER_HOST=ssh://user@remote-host

# Or TLS with client certificates
dockerd --tlsverify --tlscacert=ca.pem --tlscert=server-cert.pem --tlskey=server-key.pem -H=0.0.0.0:2376
```

---

## AP-06: No Resource Limits

**Anti-pattern:**
```bash
# No memory, CPU, or PID limits
docker run -d myapp
```

**Why dangerous:** A single container can consume all host memory (triggering OOM kills of other containers), saturate all CPU cores, or spawn unlimited processes (fork bomb). This is a denial-of-service risk for all containers on the host.

**Correct:**
```bash
docker run -d \
  -m 512m --memory-swap 1g \
  --cpus 1.5 \
  --pids-limit 200 \
  myapp
```

---

## AP-07: Using Default Bridge Network

**Anti-pattern:**
```bash
# Containers on default bridge cannot resolve each other by name
docker run -d --name db postgres
docker run -d --name app --link db:db myapp  # --link is legacy
```

**Why dangerous:** The default bridge network lacks DNS resolution, provides no isolation (all containers can communicate), and cannot be configured per-network. The `--link` flag is legacy and deprecated.

**Correct:**
```bash
docker network create myapp-net
docker run -d --name db --network myapp-net postgres
docker run -d --name app --network myapp-net myapp
# app can reach db at hostname "db" via Docker DNS
```

---

## AP-08: Disabling Seccomp

**Anti-pattern:**
```bash
docker run --security-opt seccomp=unconfined myapp
```

**Why dangerous:** The default seccomp profile blocks approximately 44 dangerous syscalls including `mount`, `reboot`, `keyctl`, and `ptrace` (in some modes). Disabling it removes a critical layer of defense that prevents container escape exploits.

**Correct:**
```bash
# Use default profile (applied automatically, just don't disable it)
docker run myapp

# Or create a custom profile that restricts further
docker run --security-opt seccomp=custom-restrictive.json myapp
```

---

## AP-09: Mounting Docker Socket into Containers

**Anti-pattern:**
```bash
docker run -v /var/run/docker.sock:/var/run/docker.sock myapp
```

**Why dangerous:** Any process in the container can use the Docker socket to create new privileged containers, mount the host filesystem, or execute commands as root on the host. This is equivalent to giving the container full root access to the host.

**Correct:**
```bash
# If Docker API access is genuinely needed (CI/CD, monitoring):
# 1. Use a socket proxy that filters allowed API calls
docker run -v /var/run/docker.sock:/var/run/docker-proxy.sock:ro \
  tecnativa/docker-socket-proxy

# 2. Or use rootless Docker where the socket has limited impact
# 3. Or use a dedicated CI runner with minimal host access
```

---

## AP-10: Ignoring .dockerignore

**Anti-pattern:**
```dockerfile
COPY . .
# Copies everything: .git, .env, node_modules, credentials, SSH keys
```

Without a `.dockerignore`, the build context includes all files in the directory, including secrets and unnecessary bulk.

**Why dangerous:** Credentials, environment files, and SSH keys get baked into image layers. Large directories like `.git` and `node_modules` bloat the image and build context.

**Correct:**
```
# .dockerignore
.git
.env
.env.*
*.pem
*.key
credentials.*
node_modules
__pycache__
.DS_Store
Dockerfile
docker-compose*.yml
```

---

## AP-11: Writable Root Filesystem

**Anti-pattern:**
```bash
# Default: container filesystem is writable
docker run -d myapp
```

**Why dangerous:** An attacker who compromises the application can write malicious binaries, modify configuration, or tamper with system files inside the container.

**Correct:**
```bash
docker run -d --read-only \
  --tmpfs /tmp:size=64m \
  --tmpfs /var/run:size=16m \
  myapp
```

---

## AP-12: Not Scanning Images

**Anti-pattern:**
```bash
# Build and deploy without any vulnerability check
docker build -t myapp:v1 .
docker push registry.example.com/myapp:v1
```

**Why dangerous:** Known CVEs in base images and dependencies go undetected. An image may ship with critical vulnerabilities that have known exploits and available fixes.

**Correct:**
```bash
# Scan as part of CI/CD pipeline
docker build -t myapp:v1 .
docker scout cves --only-severity critical,high --only-fixed -e myapp:v1
# Pipeline fails if fixable critical/high CVEs exist
```

---

## Summary Table

| # | Anti-Pattern | Risk Level | Quick Fix |
|---|-------------|------------|-----------|
| AP-01 | Running as root | Critical | Add `USER 1001:1001` |
| AP-02 | Secrets in layers | Critical | Use `--mount=type=secret` |
| AP-03 | `--privileged` | Critical | `--cap-drop ALL --cap-add <specific>` |
| AP-04 | `:latest` tag | High | Pin version or digest |
| AP-05 | TCP daemon without TLS | Critical | SSH tunnel or TLS certs |
| AP-06 | No resource limits | High | Add `-m`, `--cpus`, `--pids-limit` |
| AP-07 | Default bridge | Medium | User-defined bridge network |
| AP-08 | Seccomp disabled | High | Keep default profile |
| AP-09 | Docker socket mount | Critical | Socket proxy or rootless |
| AP-10 | No .dockerignore | High | Create comprehensive .dockerignore |
| AP-11 | Writable root FS | Medium | `--read-only` + `--tmpfs` |
| AP-12 | No image scanning | High | Docker Scout or Trivy in CI/CD |
