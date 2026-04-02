# Runtime Error Diagnostics Reference

Complete error → cause → solution mapping for Docker container runtime errors.

---

## Exit Code Reference (Extended)

| Exit Code | Signal | Linux Name | Meaning | Docker Context |
|-----------|--------|------------|---------|----------------|
| 0 | — | — | Success | Process completed normally. For services, this often means the process daemonized instead of running in foreground |
| 1 | — | — | General error | Application threw an unhandled exception or returned error |
| 2 | — | — | Misuse of shell command | Wrong arguments to shell built-in, syntax error in script |
| 125 | — | — | Docker daemon error | Container failed to start — invalid config, missing image, bad mount |
| 126 | — | — | Permission problem | Binary found but not executable (wrong permissions or format) |
| 127 | — | — | Command not found | Binary does not exist at the specified path |
| 128+n | Signal n | — | Killed by signal n | Process received fatal signal |
| 130 | SIGINT (2) | Interrupt | Ctrl+C | User interrupted the process |
| 137 | SIGKILL (9) | Kill | Forced kill | OOM killer, `docker kill`, or `docker stop` grace period exceeded |
| 139 | SIGSEGV (11) | Segfault | Memory violation | Null pointer dereference, buffer overflow, library incompatibility |
| 141 | SIGPIPE (13) | Broken pipe | Write to closed pipe | Output consumer disconnected |
| 143 | SIGTERM (15) | Terminate | Graceful shutdown | `docker stop` sent SIGTERM and process exited cleanly |

### How to Read Exit Codes

```bash
# Get exit code from stopped container
docker inspect --format='{{.State.ExitCode}}' <container>

# Get exit code from docker wait (blocking)
EXIT_CODE=$(docker wait <container>)
echo "Exit code: $EXIT_CODE"

# List all stopped containers with exit codes
docker ps -a --filter status=exited --format "table {{.Names}}\t{{.Status}}"
```

---

## OOM (Out of Memory) Diagnostics

### Confirming OOM Kill

```bash
# Check OOMKilled flag
docker inspect --format='{{.State.OOMKilled}}' <container>

# Check system logs for OOM events
dmesg | grep -i "oom\|killed process"

# Docker events for OOM
docker events --filter event=oom --since 1h

# Full memory config
docker inspect --format='Memory={{.HostConfig.Memory}} MemorySwap={{.HostConfig.MemorySwap}} MemoryReservation={{.HostConfig.MemoryReservation}}' <container>
```

### OOM Scenario Matrix

| Memory Flag | Swap Flag | Behavior |
|-------------|-----------|----------|
| `-m 512m` (no swap flag) | — | Container can use 512MB RAM + 512MB swap (total 1GB) |
| `-m 512m --memory-swap 512m` | — | Container can use 512MB total (no swap) |
| `-m 512m --memory-swap 1g` | — | Container can use 512MB RAM + 512MB swap (1GB total) |
| `-m 512m --memory-swap -1` | — | Container can use 512MB RAM + unlimited swap |
| (no memory flag) | — | No limit — container can use all host memory |

### OOM Prevention Checklist

1. ALWAYS set memory limits in production: `docker run -m 512m`
2. Profile actual usage first: `docker stats --no-stream`
3. Set memory reservation for soft limit: `--memory-reservation 256m`
4. Monitor with: `docker stats --format "table {{.Name}}\t{{.MemUsage}}\t{{.MemPerc}}"`
5. NEVER use `--oom-kill-disable` without `-m` — the container can crash the host

---

## Permission Denied Diagnostics

### File Permission Issues

| Context | Diagnostic Command | Root Cause | Solution |
|---------|-------------------|------------|----------|
| Volume mount | `docker exec <c> ls -la /data` | Host UID ≠ container UID | `docker run -u $(id -u):$(id -g)` or `chown` in Dockerfile |
| Entrypoint script | `docker exec <c> ls -la /entrypoint.sh` | Missing +x permission | `RUN chmod +x /entrypoint.sh` in Dockerfile |
| Application data dir | `docker exec <c> stat /app/data` | Directory owned by root, app runs as non-root | `RUN chown -R appuser:appuser /app/data` in Dockerfile |
| Bind mount on Linux | `ls -la /host/path` | SELinux blocking access | Add `:z` or `:Z` suffix: `-v /host/path:/data:z` |

### Capability Issues

| Error | Missing Capability | Fix |
|-------|-------------------|-----|
| `Operation not permitted` on ptrace | `SYS_PTRACE` | `--cap-add SYS_PTRACE` (for debuggers like strace, gdb) |
| `Operation not permitted` on mount | `SYS_ADMIN` | `--cap-add SYS_ADMIN` (use sparingly) |
| `Operation not permitted` on network | `NET_ADMIN` | `--cap-add NET_ADMIN` (for iptables, tc, ip commands) |
| `Permission denied` on raw socket | `NET_RAW` | `--cap-add NET_RAW` (for ping, tcpdump) |
| `Permission denied` binding port < 1024 | `NET_BIND_SERVICE` | `--cap-add NET_BIND_SERVICE` or use port > 1024 |

---

## Port Conflict Diagnostics

### Finding What Uses a Port

```bash
# On Linux
lsof -i :8080
ss -tlnp | grep 8080
netstat -tlnp | grep 8080

# On macOS
lsof -i :8080

# Find Docker container using port
docker ps --format "{{.Names}}: {{.Ports}}" | grep 8080

# Check all port mappings for a container
docker port <container>
```

### Port Conflict Resolution

| Scenario | Diagnostic | Fix |
|----------|-----------|-----|
| Another container on same port | `docker ps` shows port in use | Stop old container or map to different host port |
| Host process on port | `lsof -i :PORT` shows non-Docker process | Stop host process or change container port mapping |
| Container restart with same name | `docker ps -a` shows stopped container | `docker rm <old>` then start new, or use `--rm` |
| Docker proxy process holding port | Port busy after container stopped | Restart Docker daemon: `sudo systemctl restart docker` |

---

## Exec Format Error Diagnostics

### Architecture Mismatch

```bash
# Check image architecture
docker inspect --format='{{.Architecture}}' <image>

# Check host architecture
uname -m

# Pull for specific platform
docker pull --platform linux/amd64 <image>

# Build for specific platform
docker buildx build --platform linux/amd64 -t <image> .
```

### Script Issues

| Error | Cause | Fix |
|-------|-------|-----|
| `exec format error` on .sh file | No shebang line | Add `#!/bin/sh` or `#!/bin/bash` as first line |
| `no such file or directory` on .sh file | CRLF line endings | `RUN sed -i 's/\r$//' /script.sh` or configure Git: `git config core.autocrlf input` |
| `no such file or directory` on binary | Dynamic linking in minimal image | Build static: `CGO_ENABLED=0 go build`. Or use alpine instead of scratch |
| `exec format error` on binary | Wrong CPU architecture | Cross-compile for target or use multi-platform build |

---

## Read-Only Filesystem Diagnostics

### Common Write Paths That Need tmpfs

| Application | Writable Paths Needed |
|-------------|----------------------|
| nginx | `/var/cache/nginx`, `/var/run`, `/tmp` |
| Node.js | `/tmp`, application log directory |
| Python | `/tmp`, `__pycache__` directories |
| PostgreSQL | `/var/run/postgresql`, data directory (use volume) |
| Redis | `/data` (use volume), `/tmp` |
| Generic | `/tmp`, `/var/tmp`, `/run` |

### Read-Only Configuration Pattern

```bash
# Read-only root with specific writable paths
docker run --read-only \
  --tmpfs /tmp:size=64m \
  --tmpfs /run:size=64m \
  --mount type=volume,src=appdata,dst=/app/data \
  <image>
```

---

## PID Limit Diagnostics

### Detecting PID Exhaustion

```bash
# Check PID limit setting
docker inspect --format='{{.HostConfig.PidsLimit}}' <container>

# Count processes in container
docker top <container> | wc -l

# Check from inside container
docker exec <container> cat /sys/fs/cgroup/pids.max
docker exec <container> cat /sys/fs/cgroup/pids.current
```

### PID Limit Guidelines

| Application Type | Recommended `--pids-limit` |
|-----------------|---------------------------|
| Single-process service (nginx, redis) | 50-100 |
| Multi-worker application (gunicorn, pm2) | 200-500 |
| Build tools (make, gradle) | 500-1000 |
| Development container | 1000+ or unlimited |

---

## Resource Exhaustion Diagnostics

### Disk Space

```bash
# Docker disk usage overview
docker system df

# Detailed breakdown
docker system df -v

# Container writable layer size
docker ps -s --format "table {{.Names}}\t{{.Size}}"

# Find large files in container
docker exec <container> du -sh /* 2>/dev/null | sort -rh | head -10

# Clean unused resources
docker system prune          # Safe: stopped containers, unused networks, dangling images
docker builder prune          # Build cache
docker volume prune           # CAREFUL: removes unused volumes including data
```

### CPU Throttling

```bash
# Check CPU limits
docker inspect --format='CPUs={{.HostConfig.NanoCpus}} CPUShares={{.HostConfig.CpuShares}}' <container>

# Monitor CPU usage
docker stats --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}" --no-stream

# Update CPU limit on running container
docker update --cpus 2 <container>
```

---

## Container Restart Loop Diagnostics

### Detecting Restart Loops

```bash
# Check restart count
docker inspect --format='RestartCount={{.RestartCount}} Policy={{.HostConfig.RestartPolicy.Name}}' <container>

# Watch for rapid restarts
docker events --filter container=<container> --filter event=start

# Check last exit info
docker inspect --format='ExitCode={{.State.ExitCode}} FinishedAt={{.State.FinishedAt}}' <container>
```

### Restart Policy Reference

| Policy | Behavior | When to Use |
|--------|----------|-------------|
| `no` | Never restart (default) | One-shot tasks, debugging |
| `on-failure[:N]` | Restart on non-zero exit, optional max N times | Applications that may crash but should recover |
| `always` | Always restart, even on clean exit | Core infrastructure services |
| `unless-stopped` | Like `always`, but respects `docker stop` | Production services that should survive host reboot |

### Breaking a Restart Loop

```bash
# Stop the container (overrides restart policy)
docker stop <container>

# Update restart policy to prevent restarts
docker update --restart no <container>

# Check logs from the last crash
docker logs --tail 50 <container>

# Fix the issue, then re-enable restart policy
docker update --restart unless-stopped <container>
docker start <container>
```
