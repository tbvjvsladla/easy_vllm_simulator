---
name: docker-errors-runtime
description: >
  Use when a running container crashes, exits unexpectedly, or behaves
  incorrectly at runtime.
  Prevents misdiagnosis of OOM kills, permission denied errors, and
  silent container exits by following the logs-exec-inspect-events workflow.
  Covers exit codes, OOMKilled, port conflicts, exec format error,
  read-only filesystem, PID limits, resource exhaustion debugging.
  Keywords: docker logs, docker inspect, OOMKilled, exit code 137,
  permission denied, port already in use, SIGTERM, SIGKILL, docker exec,
  exits immediately, container won't start, restart loop, out of memory,
  container crashes, why did my container stop.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-errors-runtime

## Quick Reference

### Exit Code Reference

| Exit Code | Signal | Meaning | Common Cause |
|-----------|--------|---------|--------------|
| 0 | — | Success | Container completed normally |
| 1 | — | Application error | Uncaught exception, failed assertion, general error |
| 125 | — | Docker daemon error | Container failed to start (invalid config, missing image) |
| 126 | — | Command not executable | Permission denied on entrypoint/cmd binary |
| 127 | — | Command not found | Binary missing in image, wrong PATH, typo in CMD |
| 137 | SIGKILL (9) | Killed | OOM killer, `docker kill`, or `docker stop` timeout |
| 139 | SIGSEGV (11) | Segmentation fault | Native library crash, memory corruption |
| 143 | SIGTERM (15) | Graceful termination | `docker stop` (process handled SIGTERM) |

### Critical Warnings

**NEVER** ignore exit code 137 — it ALWAYS indicates the container was forcefully killed. Check OOM events with `docker inspect` and `dmesg` before increasing memory limits blindly.

**NEVER** use `--oom-kill-disable` without setting a memory limit (`-m`) — the container can consume ALL host memory and crash the entire system.

**NEVER** assume a container that exits with code 0 is healthy — it may have completed a one-shot command instead of running as a long-lived service. ALWAYS verify the process runs in the foreground.

**ALWAYS** check `docker logs` before any other debugging step — 90% of runtime issues are explained in the application output.

**ALWAYS** use `docker inspect --format='{{.State.ExitCode}}'` to get the exact exit code — `docker ps -a` truncates status information.

---

## Debugging Workflow

### Step 1: Check Logs

```bash
# Last 100 lines
docker logs --tail 100 <container>

# Follow live output with timestamps
docker logs -f -t <container>

# Logs from last 5 minutes
docker logs --since 5m <container>
```

### Step 2: Inspect Container State

```bash
# Exit code and error message
docker inspect --format='{{.State.ExitCode}}' <container>
docker inspect --format='{{.State.Error}}' <container>

# OOM killed?
docker inspect --format='{{.State.OOMKilled}}' <container>

# Full state as JSON
docker inspect --format='{{json .State}}' <container> | jq .
```

### Step 3: Exec Into Running Container

```bash
# Interactive shell (if container is still running)
docker exec -it <container> sh
docker exec -it <container> bash

# Check filesystem, processes, network
docker exec <container> ps aux
docker exec <container> df -h
docker exec <container> cat /etc/resolv.conf
```

### Step 4: Check System Events

```bash
# Events for specific container in last 10 minutes
docker events --since 10m --filter container=<container>

# OOM events specifically
docker events --filter event=oom --since 1h

# All die events
docker events --filter event=die --since 1h
```

### Step 5: Resource Usage

```bash
# Live resource stats
docker stats <container>

# Single snapshot
docker stats --no-stream <container>

# System-wide disk usage
docker system df -v
```

---

## Runtime Error Diagnostic Table

### Container Exits Immediately (Exit Code 0 or 1)

| Symptom | Cause | Fix |
|---------|-------|-----|
| Container exits with code 0 instantly | Main process runs in background (daemonizes) | ALWAYS run the process in foreground mode. For nginx: `CMD ["nginx", "-g", "daemon off;"]` |
| Container exits with code 0 instantly | CMD is a shell command that completes | Use a long-running process. For shell scripts: end with `exec` or `tail -f /dev/null` for debugging |
| Container exits with code 1 | Application startup failure | Check `docker logs`. Fix config, missing env vars, or dependency issues |
| Container exits with code 1 | Missing environment variables | ALWAYS pass required env vars: `docker run -e DB_HOST=db -e DB_PORT=5432` |

### OOM Killed (Exit Code 137)

| Symptom | Cause | Fix |
|---------|-------|-----|
| `OOMKilled: true` in inspect output | Container exceeded memory limit | Increase limit: `docker run -m 1g`. Profile actual usage with `docker stats` first |
| Exit 137 but `OOMKilled: false` | `docker stop` timeout exceeded (SIGKILL after grace period) | Increase stop timeout: `docker stop -t 30`. Or fix application to handle SIGTERM faster |
| Exit 137 but `OOMKilled: false` | Manual `docker kill` | Check who/what killed the container via `docker events` |
| Host OOM killer triggers | No memory limit set, host runs out of RAM | ALWAYS set memory limits in production: `-m 512m` |

### Permission Denied

| Symptom | Cause | Fix |
|---------|-------|-----|
| `Permission denied` on volume files | UID/GID mismatch between host and container | Match UIDs: `docker run -u $(id -u):$(id -g)`. Or `chown` in Dockerfile |
| `Permission denied` executing entrypoint | Script lacks execute permission | Add in Dockerfile: `RUN chmod +x /entrypoint.sh` |
| `Permission denied` binding to port < 1024 | Non-root user cannot bind privileged ports | Use port > 1024, or add `--cap-add NET_BIND_SERVICE` |
| `Operation not permitted` on system call | Missing Linux capability | Add specific capability: `--cap-add SYS_PTRACE` for debugging. NEVER use `--privileged` |

### Port Already in Use

| Symptom | Cause | Fix |
|---------|-------|-----|
| `port is already allocated` | Another container using the same host port | Find it: `docker ps --format "{{.Names}}: {{.Ports}}"`. Stop or remap |
| `bind: address already in use` | Host process using the port | Find process: `lsof -i :PORT` or `ss -tlnp \| grep PORT`. Stop it or use different port |
| Port conflict after restart | Old container not removed | Use `--rm` flag, or `docker rm -f <old-container>` before starting |

### Exec Format Error

| Symptom | Cause | Fix |
|---------|-------|-----|
| `exec format error` | Architecture mismatch (e.g., ARM image on x86) | Build for correct platform: `docker buildx build --platform linux/amd64`. Or pull correct image: `docker pull --platform linux/amd64 nginx` |
| `exec format error` on shell script | Missing shebang (`#!/bin/sh`) in entrypoint script | ALWAYS add shebang as first line of entrypoint scripts |
| `exec user process caused: no such file or directory` | CRLF line endings in shell script | Convert to LF: `RUN sed -i 's/\r$//' /entrypoint.sh` or use `dos2unix`. ALWAYS use LF in Dockerfiles and scripts |
| `exec user process caused: no such file or directory` | Dynamically linked binary in scratch/distroless image | Build with `CGO_ENABLED=0` for static linking, or use alpine base |

### Read-Only Filesystem

| Symptom | Cause | Fix |
|---------|-------|-----|
| `Read-only file system` write error | Container started with `--read-only` | Add tmpfs for writable paths: `--tmpfs /tmp --tmpfs /run`. Or mount a volume for data directories |
| Application fails to write temp files | Read-only root FS without tmpfs | Map writable paths: `--read-only --tmpfs /tmp:size=64m --mount type=volume,src=data,dst=/app/data` |
| Log file write failure | Read-only FS, app writes to file instead of stdout | Redirect logs to stdout, or mount a volume for log directory |

### PID Limit and Resource Exhaustion

| Symptom | Cause | Fix |
|---------|-------|-----|
| `cannot allocate memory` inside container | Memory limit reached | Increase `-m` limit or optimize application memory usage |
| `fork: Resource temporarily unavailable` | PID limit exceeded | Increase `--pids-limit`. Default is unlimited; set to 200-500 for most apps |
| `no space left on device` | Container writable layer full, or host disk full | Check `docker system df`. Prune unused resources: `docker system prune`. Write data to volumes, not container layer |
| Container extremely slow | CPU throttling | Check `docker stats` for CPU%. Increase `--cpus` limit |

---

## docker inspect for Debugging

### Essential Inspect Commands

```bash
# Full state overview
docker inspect --format='{{json .State}}' <container> | jq .

# Why did it stop?
docker inspect --format='ExitCode={{.State.ExitCode}} OOM={{.State.OOMKilled}} Error={{.State.Error}}' <container>

# What command is it running?
docker inspect --format='Entrypoint={{.Config.Entrypoint}} Cmd={{.Config.Cmd}}' <container>

# Environment variables
docker inspect --format='{{range .Config.Env}}{{println .}}{{end}}' <container>

# Mount points
docker inspect --format='{{range .Mounts}}{{.Type}}: {{.Source}} -> {{.Destination}} ({{if .RW}}rw{{else}}ro{{end}}){{println}}{{end}}' <container>

# Network settings
docker inspect --format='{{range $net, $config := .NetworkSettings.Networks}}{{$net}}: {{$config.IPAddress}}{{println}}{{end}}' <container>

# Resource limits
docker inspect --format='Memory={{.HostConfig.Memory}} CPUs={{.HostConfig.NanoCpus}} PidsLimit={{.HostConfig.PidsLimit}}' <container>

# Health check status
docker inspect --format='{{.State.Health.Status}}' <container>
docker inspect --format='{{json .State.Health}}' <container> | jq .

# Restart count
docker inspect --format='RestartCount={{.RestartCount}}' <container>
```

---

## Decision Trees

### Container Won't Start

```
Container won't start
├─ Exit 125 → Docker daemon error
│  ├─ "invalid reference format" → Fix image name/tag
│  ├─ "no such image" → Pull image first: docker pull <image>
│  └─ "invalid mount config" → Fix volume/mount syntax
├─ Exit 126 → Command not executable
│  ├─ Check file permissions → chmod +x
│  └─ Check binary format → file <binary>
├─ Exit 127 → Command not found
│  ├─ Typo in CMD/ENTRYPOINT → Fix spelling
│  ├─ Binary not in PATH → Use absolute path
│  └─ Binary not installed → Add to Dockerfile
└─ Exit 0/1 instantly → See "Container Exits Immediately" table
```

### Container Crashes After Running

```
Container was running, then died
├─ Exit 137 → Killed
│  ├─ OOMKilled=true → Memory limit too low (see OOM section)
│  ├─ OOMKilled=false, after docker stop → Stop timeout too short
│  └─ OOMKilled=false, unexpected → Check docker events + dmesg
├─ Exit 139 → Segfault
│  ├─ Native library issue → Check library compatibility
│  └─ Memory corruption → Debug with --cap-add SYS_PTRACE
├─ Exit 143 → Graceful SIGTERM
│  └─ Expected from docker stop → Normal shutdown
└─ Exit 1 → Application error
   └─ Check docker logs → Fix application bug
```

---

## Reference Links

- [references/diagnostics.md](references/diagnostics.md) -- Complete error-to-solution mapping for all runtime errors
- [references/examples.md](references/examples.md) -- Step-by-step debugging sessions with real commands
- [references/anti-patterns.md](references/anti-patterns.md) -- Runtime configuration mistakes and how to avoid them

### Official Sources

- https://docs.docker.com/engine/daemon/troubleshoot/
- https://docs.docker.com/reference/cli/docker/container/run/
- https://docs.docker.com/reference/cli/docker/container/logs/
- https://docs.docker.com/reference/cli/docker/inspect/
- https://docs.docker.com/reference/cli/docker/system/events/
