# Container CLI Anti-Patterns

Common Docker container CLI misuse patterns with corrections. Docker Engine 24+.

---

## 1. Execution Anti-Patterns

### AP-001: Using --privileged Instead of Specific Capabilities

```bash
# WRONG -- grants full host access
docker run --privileged myapp

# CORRECT -- grant only what is needed
docker run --cap-drop ALL --cap-add NET_BIND_SERVICE --cap-add SYS_PTRACE myapp
```

**Why**: `--privileged` disables ALL security isolation. The container can access ALL host devices, bypass AppArmor/SELinux, load kernel modules, and mount the host filesystem. ALWAYS drop all capabilities and add back only the specific ones required.

### AP-002: Running as Root by Default

```bash
# WRONG -- runs as root inside container
docker run nginx

# CORRECT -- specify non-root user
docker run -u 1000:1000 nginx

# BEST -- define USER in Dockerfile
# USER 1001:1001
```

**Why**: Root inside a container maps to root on the host (unless user namespaces are configured). A container escape vulnerability with root gives full host access. ALWAYS run as non-root.

### AP-003: Not Using --rm for Throwaway Containers

```bash
# WRONG -- leaves stopped container behind
docker run ubuntu echo "hello"
docker run node:20 npm test

# CORRECT -- auto-cleanup
docker run --rm ubuntu echo "hello"
docker run --rm node:20 npm test
```

**Why**: Every `docker run` without `--rm` creates a stopped container that consumes disk space. Over time, hundreds of stopped containers accumulate. ALWAYS use `--rm` for one-off commands, tests, and debug sessions.

### AP-004: Not Using --init for Signal Handling

```bash
# WRONG -- shell scripts and Node.js do not forward SIGTERM
docker run -d --name app node:20 node server.js

# CORRECT -- tini init process handles signals and reaps zombies
docker run -d --name app --init node:20 node server.js
```

**Why**: Without `--init`, the application runs as PID 1 and must handle signals itself. Most applications (Node.js, Python, shell scripts) do NOT handle SIGTERM by default, causing `docker stop` to always wait the full grace period and then SIGKILL. `--init` adds tini as PID 1 which properly forwards signals and reaps zombie processes.

---

## 2. Networking Anti-Patterns

### AP-005: Using --link for Container Communication

```bash
# WRONG -- legacy, deprecated
docker run --link redis:db myapp

# CORRECT -- use user-defined bridge network
docker network create mynet
docker run -d --name redis --network mynet redis
docker run -d --name myapp --network mynet myapp
# myapp can reach redis at hostname "redis"
```

**Why**: `--link` is legacy and only works on the default bridge network. It does NOT support automatic DNS resolution, cannot be changed without recreating containers, and provides no network isolation. User-defined networks provide DNS, isolation, and live connect/disconnect.

### AP-006: Using Default Bridge Network

```bash
# WRONG -- default bridge lacks DNS resolution
docker run -d --name redis redis
docker run -d --name myapp myapp
# myapp CANNOT reach redis by name

# CORRECT -- user-defined network with automatic DNS
docker network create mynet
docker run -d --name redis --network mynet redis
docker run -d --name myapp --network mynet myapp
# myapp reaches redis at hostname "redis"
```

**Why**: The default bridge network does NOT provide DNS resolution between containers. Containers can only communicate by IP address, which changes on restart. ALWAYS create and use user-defined bridge networks.

### AP-007: Exposing Ports on All Interfaces

```bash
# WRONG -- accessible from any network interface
docker run -p 8080:80 nginx

# CORRECT -- bind to localhost only (use reverse proxy for external access)
docker run -p 127.0.0.1:8080:80 nginx
```

**Why**: `-p 8080:80` binds to `0.0.0.0`, making the service accessible from any network interface, including the public internet. For services that should only be accessed via a reverse proxy or locally, ALWAYS bind to `127.0.0.1`.

---

## 3. Exec Anti-Patterns

### AP-008: Chained Commands Without Shell Wrapper

```bash
# WRONG -- && is interpreted by the HOST shell
docker exec myapp echo "step 1" && echo "step 2"
# "step 2" runs on the HOST, not in the container

# CORRECT -- wrap in shell
docker exec myapp sh -c 'echo "step 1" && echo "step 2"'
```

**Why**: The host shell interprets `&&`, `||`, `|`, `;`, and redirections before Docker sees them. The second command runs on the host. ALWAYS wrap compound commands in `sh -c "..."`.

### AP-009: Exec Into Paused Container

```bash
# WRONG -- fails silently or with error
docker pause myapp
docker exec -it myapp bash  # Will fail

# CORRECT -- unpause first
docker unpause myapp
docker exec -it myapp bash
```

**Why**: A paused container's processes are frozen by the cgroup freezer. Docker cannot execute new processes in a frozen cgroup. ALWAYS unpause before exec.

### AP-010: Using Exec for Persistent Changes

```bash
# WRONG -- changes are lost when container is recreated
docker exec myapp apt-get update && apt-get install -y curl
docker exec myapp pip install requests

# CORRECT -- add to Dockerfile
# RUN apt-get update && apt-get install -y --no-install-recommends curl
# RUN pip install requests
```

**Why**: `docker exec` changes exist only in the container's writable layer. When the container is removed and recreated (deployment, scaling, restart), all changes are lost. ALWAYS put required packages and configuration in the Dockerfile.

---

## 4. Lifecycle Anti-Patterns

### AP-011: Using docker kill Instead of docker stop

```bash
# WRONG -- no graceful shutdown, data corruption risk
docker kill myapp

# CORRECT -- graceful shutdown with SIGTERM
docker stop myapp

# CORRECT -- with custom grace period for slow shutdown
docker stop -t 30 myapp
```

**Why**: `docker kill` sends SIGKILL by default, which immediately terminates the process. Applications cannot flush buffers, close database connections, or finish in-flight requests. ALWAYS use `docker stop` for graceful shutdown. Use `docker kill` only for hung or unresponsive containers.

### AP-012: Not Setting Restart Policy for Services

```bash
# WRONG -- container stays stopped after crash or host reboot
docker run -d --name api myapp

# CORRECT -- auto-restart on failure
docker run -d --name api --restart unless-stopped myapp

# CORRECT -- with retry limit for debugging
docker run -d --name api --restart on-failure:5 myapp
```

**Why**: Without a restart policy, crashed containers stay stopped until manually restarted. After a host reboot, all containers remain stopped. ALWAYS set `--restart unless-stopped` or `--restart on-failure:N` for production services.

### AP-013: Force-Removing Running Containers in Production

```bash
# WRONG -- immediate kill, no graceful shutdown
docker rm -f production-db

# CORRECT -- stop gracefully, then remove
docker stop -t 30 production-db
docker rm -v production-db
```

**Why**: `docker rm -f` sends SIGKILL immediately, identical to `docker kill` followed by `docker rm`. For databases and stateful services, this risks data corruption. ALWAYS `docker stop` first with an appropriate grace period.

---

## 5. Resource Anti-Patterns

### AP-014: Running Without Memory Limits

```bash
# WRONG -- container can consume all host memory
docker run -d myapp

# CORRECT -- set memory limit
docker run -d -m 512m myapp

# CORRECT -- with swap limit
docker run -d -m 512m --memory-swap 1g myapp
```

**Why**: Without memory limits, a single container with a memory leak can consume all host memory, causing the OOM killer to terminate random processes including other containers. ALWAYS set `-m` for production containers.

### AP-015: Running Without CPU Limits

```bash
# WRONG -- container can use 100% of all CPU cores
docker run -d myapp

# CORRECT -- limit CPU usage
docker run -d --cpus 1.5 myapp

# CORRECT -- with PID limit for fork bomb prevention
docker run -d --cpus 1.5 --pids-limit 200 myapp
```

**Why**: Without CPU limits, a runaway process in one container can starve all other containers of CPU time. ALWAYS set `--cpus` and `--pids-limit` for production workloads.

### AP-016: Disabling OOM Killer Without Memory Limit

```bash
# WRONG -- can hang the entire host
docker run --oom-kill-disable myapp

# CORRECT -- disable OOM kill only WITH a memory limit
docker run -m 512m --oom-kill-disable myapp
```

**Why**: `--oom-kill-disable` without a memory limit means a container can consume unlimited memory. When the host runs out, the kernel has no container to OOM-kill, potentially freezing the entire system. NEVER use `--oom-kill-disable` without `-m`.

---

## 6. Storage Anti-Patterns

### AP-017: Using -v Instead of --mount for Production

```bash
# WRONG -- -v silently creates host directories if they do not exist
docker run -v /nonexistent/path:/app/config myapp
# Creates /nonexistent/path as root-owned empty directory

# CORRECT -- --mount fails explicitly if source does not exist
docker run --mount type=bind,src=/etc/myapp/config,dst=/app/config myapp
# Error: source path does not exist
```

**Why**: `-v` auto-creates missing host directories as root-owned empty directories, masking configuration errors. `--mount` fails immediately if the source does not exist, catching misconfiguration before the container starts. ALWAYS use `--mount` for production bind mounts.

### AP-018: Not Using Named Volumes for Database Data

```bash
# WRONG -- anonymous volume, hard to manage
docker run -d postgres:16
# Volume gets random ID, lost if container is removed with --rm

# CORRECT -- named volume, persistent and manageable
docker run -d --mount source=pgdata,target=/var/lib/postgresql/data postgres:16
```

**Why**: Anonymous volumes get random IDs that are difficult to identify and manage. They are automatically removed when the container is removed with `--rm`. Named volumes persist independently of container lifecycle and are easy to backup, restore, and share.

---

## 7. Logging Anti-Patterns

### AP-019: Not Setting Log Rotation

```bash
# WRONG -- logs grow unbounded
docker run -d --name api myapp

# CORRECT -- set max log size and file count
docker run -d --name api \
  --log-opt max-size=10m \
  --log-opt max-file=3 \
  myapp
```

**Why**: The default `json-file` log driver has no size limit. A busy container can fill the entire disk with logs. ALWAYS set `max-size` and `max-file` log options, or configure defaults in `/etc/docker/daemon.json`.

### AP-020: Searching Logs Without --since or --tail

```bash
# WRONG -- reads ALL logs (can be gigabytes)
docker logs myapp | grep error

# CORRECT -- limit the log window
docker logs --since 1h myapp 2>&1 | grep error
docker logs --tail 1000 myapp 2>&1 | grep error
```

**Why**: `docker logs` without `--since` or `--tail` reads the entire log history. For long-running containers, this can be gigabytes of data, causing high memory usage and slow results. ALWAYS limit the scope.

---

## 8. Security Anti-Patterns

### AP-021: Mounting Docker Socket Into Containers

```bash
# WRONG -- container gets full Docker API access (effectively root on host)
docker run -v /var/run/docker.sock:/var/run/docker.sock myapp

# CORRECT -- use Docker-in-Docker with limited permissions, or
# use a Docker API proxy that restricts operations
```

**Why**: Mounting the Docker socket gives the container full control over the Docker daemon, which is equivalent to root access on the host. The container can create privileged containers, mount host filesystems, and escape completely. NEVER mount the Docker socket unless absolutely necessary (CI/CD runners), and ALWAYS pair with additional restrictions.

### AP-022: Hardcoding Secrets in Environment Variables

```bash
# WRONG -- visible in docker inspect, docker history, process list
docker run -e DB_PASSWORD=mysecretpass myapp

# CORRECT -- use Docker secrets or file-based secrets
docker run --mount type=bind,src=/etc/secrets/db_pass,dst=/run/secrets/db_pass,readonly myapp
# Application reads /run/secrets/db_pass
```

**Why**: Environment variables set via `-e` are visible in `docker inspect`, `docker exec env`, `/proc/1/environ`, and process listing. ALWAYS use file-based secrets or Docker Swarm secrets for sensitive data.

### AP-023: Using latest Tag in Production

```bash
# WRONG -- unpredictable, different version on each pull
docker run -d myapp:latest

# CORRECT -- pin to specific version
docker run -d myapp:v2.1.0

# BEST -- pin to digest for supply chain security
docker run -d myapp@sha256:abc123...
```

**Why**: The `latest` tag is mutable and can point to a different image at any time. Deployments become non-reproducible. ALWAYS pin to a specific version tag, and use digest pinning for critical production workloads.

---

## Summary Table

| # | Anti-Pattern | Severity | Correction |
|---|-------------|----------|------------|
| AP-001 | `--privileged` in production | Critical | `--cap-drop ALL --cap-add <specific>` |
| AP-002 | Running as root | High | `-u 1000:1000` or `USER` in Dockerfile |
| AP-003 | No `--rm` for throwaway containers | Low | ALWAYS `--rm` for one-off commands |
| AP-004 | No `--init` for signal handling | Medium | ALWAYS `--init` for non-signal-aware apps |
| AP-005 | Using `--link` | Medium | User-defined bridge network |
| AP-006 | Using default bridge | Medium | `docker network create` + `--network` |
| AP-007 | Ports on all interfaces | High | `-p 127.0.0.1:port:port` |
| AP-008 | Chained commands without shell | Medium | `sh -c "cmd1 && cmd2"` |
| AP-009 | Exec into paused container | Low | `docker unpause` first |
| AP-010 | Exec for persistent changes | Medium | Put in Dockerfile |
| AP-011 | `docker kill` for normal shutdown | High | `docker stop` with grace period |
| AP-012 | No restart policy | Medium | `--restart unless-stopped` |
| AP-013 | `docker rm -f` on production | High | `docker stop` then `docker rm` |
| AP-014 | No memory limit | High | `-m 512m` |
| AP-015 | No CPU limit | Medium | `--cpus 1.5 --pids-limit 200` |
| AP-016 | OOM kill disabled without -m | Critical | ALWAYS pair with `-m` |
| AP-017 | `-v` for production bind mounts | Medium | `--mount` for explicit errors |
| AP-018 | Anonymous volumes for databases | High | Named volumes |
| AP-019 | No log rotation | High | `--log-opt max-size=10m` |
| AP-020 | Unbounded log reads | Low | `--since` or `--tail` |
| AP-021 | Mounting Docker socket | Critical | Avoid or use API proxy |
| AP-022 | Secrets in env vars | High | File-based secrets |
| AP-023 | `latest` tag in production | High | Pin specific version or digest |
