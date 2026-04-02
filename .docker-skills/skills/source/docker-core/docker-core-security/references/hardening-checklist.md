# Docker Security Hardening Checklist

Use this checklist before deploying containers to production. Items are ordered by priority.

---

## 1. Image Supply Chain

- [ ] **Base image pinned to digest or specific version tag** -- NEVER use `:latest` in production
  ```dockerfile
  FROM node:20.11-alpine@sha256:abc123...
  ```
- [ ] **Base image is an official or verified publisher image** -- ALWAYS prefer Docker Official Images
- [ ] **Image scanned for vulnerabilities** -- Run `docker scout cves --only-severity critical,high` or `trivy image --severity CRITICAL,HIGH`
- [ ] **No critical/high fixable vulnerabilities** -- Address all fixable CVEs before deployment
- [ ] **SBOM generated and stored** -- `docker buildx build --sbom=true` or `trivy image --format spdx-json`
- [ ] **Content trust enabled for pulls** -- `export DOCKER_CONTENT_TRUST=1`
- [ ] **Provenance attestation enabled** -- `docker buildx build --provenance=mode=max`
- [ ] **Minimal base image used** -- Alpine (<6 MB) or distroless for production

---

## 2. Build-Time Security

- [ ] **No secrets in image layers** -- NEVER use `ENV`, `ARG`, or `COPY` for secrets
  ```dockerfile
  # CORRECT
  RUN --mount=type=secret,id=api_key cat /run/secrets/api_key
  # WRONG
  ENV API_KEY=supersecret
  ```
- [ ] **Multi-stage build used** -- Build tools and dependencies NOT present in final image
- [ ] **`.dockerignore` configured** -- Excludes `.git`, `.env`, `node_modules`, credentials files
- [ ] **Package manager cache cleaned** -- `rm -rf /var/lib/apt/lists/*` after install
- [ ] **SSH agent forwarding for private repos** -- `--mount=type=ssh` instead of copying keys
- [ ] **Build arguments validated** -- No sensitive defaults in `ARG` instructions

---

## 3. Runtime User

- [ ] **Container runs as non-root** -- `USER 1001:1001` in Dockerfile
  ```dockerfile
  RUN addgroup -g 1001 -S appgroup && adduser -u 1001 -S appuser -G appgroup
  USER 1001:1001
  ```
- [ ] **Numeric UID/GID used** -- ALWAYS use numeric IDs, not names (deterministic across images)
- [ ] **File ownership set** -- `COPY --chown=1001:1001` or `RUN chown` before `USER` instruction
- [ ] **No setuid/setgid binaries** -- Remove or verify necessity: `find / -perm /6000 -type f`

---

## 4. Linux Capabilities

- [ ] **ALL capabilities dropped** -- `--cap-drop ALL` ALWAYS as the starting point
- [ ] **Only required capabilities added** -- `--cap-add NET_BIND_SERVICE` (document each addition)
- [ ] **`--privileged` NOT used** -- NEVER in production, use specific `--cap-add` instead
- [ ] **No-new-privileges enabled** -- `--security-opt no-new-privileges=true`

Compose equivalent:
```yaml
services:
  app:
    cap_drop: [ALL]
    cap_add: [NET_BIND_SERVICE]
    security_opt: [no-new-privileges=true]
```

---

## 5. Filesystem

- [ ] **Read-only root filesystem** -- `--read-only`
- [ ] **Writable dirs via tmpfs** -- `--tmpfs /tmp:size=64m` for each required writable path
- [ ] **No host filesystem mounts in production** -- Avoid bind mounts to sensitive host paths
- [ ] **Volumes use named volumes** -- NEVER anonymous volumes for persistent data

---

## 6. Network

- [ ] **User-defined bridge network** -- NEVER use the default bridge
- [ ] **Minimal port exposure** -- Only publish ports that external clients need
- [ ] **Internal networks for backend services** -- `docker network create --internal backend`
- [ ] **No `--network host`** -- Unless required for performance (document the reason)
- [ ] **DNS configured** -- Custom DNS if needed, not relying on host resolv.conf

---

## 7. Resource Limits

- [ ] **Memory limit set** -- `-m 512m` (based on application profiling)
- [ ] **Memory-swap limit set** -- `--memory-swap` equal to or 2x memory limit
- [ ] **CPU limit set** -- `--cpus 1.5` (based on workload requirements)
- [ ] **PID limit set** -- `--pids-limit 200` (fork bomb prevention)
- [ ] **File descriptor limit set** -- `--ulimit nofile=1024:2048`

Compose equivalent:
```yaml
services:
  app:
    deploy:
      resources:
        limits:
          memory: 512m
          cpus: "1.5"
          pids: 200
    ulimits:
      nofile:
        soft: 1024
        hard: 2048
```

---

## 8. Seccomp and AppArmor

- [ ] **Default seccomp profile active** -- Do NOT disable with `seccomp=unconfined`
- [ ] **Custom seccomp profile for sensitive workloads** -- Restrict to exact needed syscalls
- [ ] **AppArmor profile active** (Linux only) -- Default `docker-default` or custom profile
- [ ] **SELinux labels set** (if applicable) -- `--security-opt label=type:svirt_apache_t`

---

## 9. Daemon Security

- [ ] **Docker daemon not exposed over TCP** -- Use Unix socket or SSH tunneling
- [ ] **TLS enabled if remote API is required** -- `--tlsverify --tlscacert --tlscert --tlskey`
- [ ] **Docker group membership restricted** -- Only trusted users in the `docker` group
- [ ] **Rootless Docker considered** -- Evaluate for environments where daemon-level isolation matters
- [ ] **Logging configured** -- `--log-driver json-file --log-opt max-size=10m --log-opt max-file=3`

---

## 10. Monitoring and Auditing

- [ ] **Docker Bench for Security run** -- Address all WARN findings in sections 1-5
  ```bash
  docker run --rm --net host --pid host --userns host --cap-add audit_control \
    -v /var/lib:/var/lib:ro -v /var/run/docker.sock:/var/run/docker.sock:ro \
    -v /usr/lib/systemd:/usr/lib/systemd:ro -v /etc:/etc:ro \
    docker/docker-bench-security
  ```
- [ ] **Health checks configured** -- `HEALTHCHECK` in Dockerfile or `--health-cmd` at runtime
- [ ] **Container resource usage monitored** -- `docker stats` or Prometheus/Grafana
- [ ] **Image scanning in CI/CD pipeline** -- Automated on every build
- [ ] **Regular re-scanning of deployed images** -- Weekly or on new CVE database updates

---

## Quick Compliance Summary

| CIS Benchmark Area | Key Controls |
|---------------------|-------------|
| 1 - Host Configuration | Rootless Docker, audit logging, separate partition for Docker |
| 2 - Docker Daemon | TLS, restricted API access, logging configured |
| 3 - Docker Daemon Config | user namespace remapping, seccomp, AppArmor |
| 4 - Container Images | No secrets in layers, non-root user, health checks |
| 5 - Container Runtime | Read-only FS, cap-drop ALL, resource limits, no-new-privileges |
| 6 - Docker Security Operations | Image scanning, bench audit, monitoring |
