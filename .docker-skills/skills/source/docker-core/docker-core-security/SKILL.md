---
name: docker-core-security
description: >
  Use when hardening containers, scanning images for vulnerabilities, or
  implementing least-privilege Docker deployments.
  Prevents running containers as root, exposing secrets in image layers, and
  skipping vulnerability scanning before production deployment.
  Covers Docker Scout, Trivy, rootless Docker, USER instruction, cap-drop,
  read-only filesystems, seccomp, AppArmor, content trust, and resource limits.
  Keywords: docker scout, trivy, USER, cap-drop, --read-only, seccomp,
  AppArmor, --no-new-privileges, DOCKER_CONTENT_TRUST, docker-bench-security,
  environment variable, password, secrets, credential leak, running as root,
  container security, harden container.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-core-security

## Quick Reference

### Security Layers Overview

| Layer | Mechanism | Purpose |
|-------|-----------|---------|
| Image Supply Chain | Content trust, scanning, pinned digests | Verify image integrity and known vulnerabilities |
| Build-Time | Secrets mounts, multi-stage builds, .dockerignore | Prevent secrets leaking into image layers |
| Runtime Isolation | Namespaces, cgroups, seccomp, AppArmor | Kernel-level process and resource isolation |
| Least Privilege | Non-root USER, cap-drop ALL, read-only FS | Minimize attack surface inside the container |
| Resource Limits | Memory, CPU, PID limits | Prevent denial-of-service via resource exhaustion |
| Host Protection | Rootless Docker, no-new-privileges | Reduce daemon and container escape impact |

### Minimum Viable Security (Quick-Start)

Apply these five settings to EVERY production container:

```dockerfile
# Dockerfile
FROM node:20-alpine
RUN addgroup -g 1001 -S appgroup && adduser -u 1001 -S appuser -G appgroup
WORKDIR /app
COPY --chown=1001:1001 . .
USER 1001:1001
CMD ["node", "server.js"]
```

```bash
# Runtime
docker run -d \
  --read-only \
  --tmpfs /tmp:size=64m \
  --cap-drop ALL \
  --security-opt no-new-privileges=true \
  -m 512m --cpus 1.5 --pids-limit 200 \
  myapp:v1
```

```yaml
# Docker Compose equivalent
services:
  app:
    image: myapp:v1
    read_only: true
    tmpfs:
      - /tmp:size=64m
    cap_drop:
      - ALL
    security_opt:
      - no-new-privileges=true
    deploy:
      resources:
        limits:
          memory: 512m
          cpus: "1.5"
          pids: 200
    user: "1001:1001"
```

### Critical Warnings

**NEVER** use `--privileged` in production -- it grants the container ALL host capabilities and access to ALL host devices. ALWAYS use specific `--cap-add` flags for the exact capabilities needed.

**NEVER** store secrets in Dockerfile `ENV`, `ARG`, or `COPY` instructions -- they persist in image layers and are visible via `docker history`. ALWAYS use `--mount=type=secret` for build-time secrets and Docker secrets or environment variables for runtime.

**NEVER** run containers as root unless there is a specific technical requirement -- most application workloads run correctly as non-root. ALWAYS add a `USER` instruction to your Dockerfile.

**NEVER** use `latest` tag in production -- it is mutable and prevents reproducible deployments. ALWAYS pin to a specific version tag or image digest.

**NEVER** expose the Docker daemon API over TCP without TLS -- API access equals root access on the host. ALWAYS use Unix socket (default) or SSH tunneling for remote access.

**NEVER** disable seccomp (`--security-opt seccomp=unconfined`) in production -- it removes syscall filtering that blocks container escapes. ALWAYS use the default seccomp profile or a custom restricted profile.

---

## Security Hardening Decision Tree

```
Container needs hardening?
|
+-- Is the image from a trusted source?
|   +-- NO --> Pin to verified digest: FROM image@sha256:...
|   +-- YES --> Pin version tag, scan with Docker Scout
|
+-- Does the app need root?
|   +-- NO --> Add USER 1001:1001 to Dockerfile
|   +-- YES --> Document WHY, use --cap-drop ALL --cap-add <specific>
|
+-- Does the app write to the filesystem?
|   +-- NO --> Use --read-only
|   +-- YES to specific dirs --> Use --read-only --tmpfs /path
|   +-- YES broadly --> Skip --read-only, log a security debt ticket
|
+-- Does the app need special kernel access?
|   +-- NO --> Use --cap-drop ALL (add nothing)
|   +-- YES --> --cap-drop ALL --cap-add <EXACT_CAP> (see capabilities table)
|
+-- Is this exposed to the internet?
|   +-- YES --> Add resource limits, no-new-privileges, seccomp default
|   +-- NO --> Still apply resource limits (defense in depth)
```

---

## Non-Root Containers

### Dockerfile USER Instruction

ALWAYS use numeric UID/GID for deterministic behavior across environments:

```dockerfile
# Alpine-based
RUN addgroup -g 1001 -S appgroup && adduser -u 1001 -S appuser -G appgroup
USER 1001:1001

# Debian/Ubuntu-based
RUN groupadd -r -g 1001 appgroup && useradd --no-log-init -r -u 1001 -g appgroup appuser
USER 1001:1001
```

### Runtime Override

```bash
docker run -u 1001:1001 nginx
docker run --user nobody nginx
```

### Compose

```yaml
services:
  app:
    image: myapp:v1
    user: "1001:1001"
```

---

## Linux Capabilities

### Drop-All-Add-Specific Pattern

ALWAYS start with `--cap-drop ALL` and add back only what the application requires:

```bash
docker run --cap-drop ALL --cap-add NET_BIND_SERVICE nginx
```

### Common Capabilities Reference

| Capability | Purpose | When Needed |
|------------|---------|-------------|
| `NET_BIND_SERVICE` | Bind to ports < 1024 | Web servers on port 80/443 |
| `CHOWN` | Change file ownership | Apps managing file permissions |
| `SETUID` / `SETGID` | Change process UID/GID | Apps switching users at runtime |
| `SYS_PTRACE` | Process tracing | Debugging, profiling tools |
| `NET_ADMIN` | Network configuration | VPN containers, network tools |
| `SYS_ADMIN` | Mount operations, broad access | **Avoid** -- use specific caps instead |
| `DAC_OVERRIDE` | Bypass file permission checks | **Avoid** -- fix file permissions instead |

### Compose Syntax

```yaml
services:
  app:
    cap_drop:
      - ALL
    cap_add:
      - NET_BIND_SERVICE
```

---

## Read-Only Root Filesystem

ALWAYS use `--read-only` with `--tmpfs` for directories that require writes:

```bash
docker run --read-only \
  --tmpfs /tmp:size=64m \
  --tmpfs /run:size=16m \
  --tmpfs /var/cache/nginx:size=32m \
  nginx
```

### Compose

```yaml
services:
  app:
    read_only: true
    tmpfs:
      - /tmp:size=64m
      - /run:size=16m
```

---

## Seccomp and AppArmor Profiles

### Seccomp (Syscall Filtering)

Docker applies a default seccomp profile that blocks ~44 dangerous syscalls. ALWAYS keep this enabled.

```bash
# Default profile (applied automatically)
docker run --security-opt seccomp=default nginx

# Custom restrictive profile
docker run --security-opt seccomp=custom-profile.json nginx
```

### AppArmor (Mandatory Access Control)

```bash
# Default Docker AppArmor profile
docker run --security-opt apparmor=docker-default nginx

# Custom profile
docker run --security-opt apparmor=my-custom-profile nginx
```

### No-New-Privileges

ALWAYS enable this to prevent processes inside the container from gaining additional privileges via setuid/setgid binaries:

```bash
docker run --security-opt no-new-privileges=true nginx
```

---

## Content Trust and Image Signing

### DOCKER_CONTENT_TRUST

```bash
# Enable globally
export DOCKER_CONTENT_TRUST=1

# With content trust enabled:
docker pull nginx          # Fails if image is not signed
docker push myapp:v1       # Automatically signs the image
```

### Digest Pinning

ALWAYS pin production base images to a digest for supply chain security:

```dockerfile
FROM alpine@sha256:c5b1261d6d3e43071626931fc004f70149baed4c52b3b3d4f8d72af0a7e2d708
```

---

## Image Scanning

### Docker Scout

```bash
# Quick vulnerability overview
docker scout quickview myapp:v1

# Detailed CVE listing
docker scout cves myapp:v1

# Critical and high severity only
docker scout cves --only-severity critical,high myapp:v1

# Only fixable vulnerabilities
docker scout cves --only-fixed myapp:v1

# Compare versions for upgrade decisions
docker scout compare myapp:v1 --to myapp:v2

# Get base image upgrade recommendations
docker scout recommendations myapp:v1

# CI/CD gate (exit code 2 if vulnerabilities found)
docker scout cves -e myapp:v1
```

### Trivy

```bash
# Scan local image
trivy image myapp:v1

# Critical/high only
trivy image --severity CRITICAL,HIGH myapp:v1

# Exit code for CI (1 if vulns found)
trivy image --exit-code 1 --severity CRITICAL myapp:v1

# Scan filesystem
trivy fs .

# Generate SBOM
trivy image --format spdx-json -o sbom.json myapp:v1
```

### Snyk

```bash
# Scan image
snyk container test myapp:v1

# With Dockerfile for remediation advice
snyk container test myapp:v1 --file=Dockerfile

# Monitor for new vulnerabilities
snyk container monitor myapp:v1
```

See [references/scanning.md](references/scanning.md) for complete scanning integration details.

---

## Resource Limits (DoS Prevention)

ALWAYS set resource limits on production containers:

```bash
docker run -d \
  -m 512m --memory-swap 1g \
  --cpus 1.5 \
  --pids-limit 200 \
  --ulimit nofile=1024:2048 \
  myapp:v1
```

| Flag | Purpose | Recommended |
|------|---------|-------------|
| `-m, --memory` | Hard memory limit | Set based on application profiling |
| `--memory-swap` | Memory + swap limit | Set to 2x memory or equal to prevent swap |
| `--cpus` | CPU quota | Match to workload, start conservative |
| `--pids-limit` | Max processes (fork bomb prevention) | 200 for most apps, 100 for simple services |
| `--ulimit nofile` | File descriptor limit | 1024:2048 for most apps |

---

## Rootless Docker

Runs BOTH the daemon and containers entirely without root privileges.

```bash
# Install (as non-root user)
dockerd-rootless-setuptool.sh install

# Configure environment
export DOCKER_HOST=unix:///run/user/$(id -u)/docker.sock

# Enable auto-start with lingering
sudo loginctl enable-linger $(whoami)

# Manage via systemd user units
systemctl --user start docker
systemctl --user enable docker
```

**Key limitation**: Rootless mode does NOT support AppArmor, overlay network drivers on older kernels, or `--net=host` on all configurations. ALWAYS test your workload in rootless mode before committing to it.

---

## Build-Time Secret Management

NEVER bake secrets into image layers. ALWAYS use BuildKit secret mounts:

```dockerfile
# syntax=docker/dockerfile:1
RUN --mount=type=secret,id=aws_creds,target=/root/.aws/credentials \
    aws s3 cp s3://bucket/file /app/file
```

```bash
docker buildx build --secret id=aws_creds,src=$HOME/.aws/credentials .
```

### SSH Agent Forwarding

```bash
docker buildx build --ssh default=$SSH_AUTH_SOCK .
```

```dockerfile
RUN --mount=type=ssh git clone git@github.com:org/private-repo.git
```

---

## Docker Bench for Security

Automated audit against CIS Docker Benchmark:

```bash
docker run --rm --net host --pid host \
  --userns host --cap-add audit_control \
  -e DOCKER_CONTENT_TRUST=$DOCKER_CONTENT_TRUST \
  -v /var/lib:/var/lib:ro \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -v /usr/lib/systemd:/usr/lib/systemd:ro \
  -v /etc:/etc:ro \
  docker/docker-bench-security
```

ALWAYS run Docker Bench before deploying to production. Address all WARN findings in sections 1-5.

---

## Supply Chain Security

### SBOM (Software Bill of Materials)

```bash
# Generate SBOM during build
docker buildx build --sbom=true --push -t myapp:v1 .

# Scan existing image SBOM
docker scout sbom myapp:v1
```

### Provenance Attestations

```bash
# Enable SLSA provenance
docker buildx build --provenance=mode=max --push -t myapp:v1 .
```

---

## Reference Links

- [references/scanning.md](references/scanning.md) -- Docker Scout, Trivy, Snyk integration and CI/CD pipeline patterns
- [references/hardening-checklist.md](references/hardening-checklist.md) -- Complete security hardening checklist for audits
- [references/anti-patterns.md](references/anti-patterns.md) -- Security anti-patterns with exploit scenarios

### Official Sources

- https://docs.docker.com/engine/security/
- https://docs.docker.com/engine/security/rootless/
- https://docs.docker.com/scout/
- https://docs.docker.com/reference/cli/docker/scout/cves/
- https://docs.docker.com/build/building/best-practices/
- https://docs.docker.com/engine/security/seccomp/
- https://docs.docker.com/engine/security/apparmor/
