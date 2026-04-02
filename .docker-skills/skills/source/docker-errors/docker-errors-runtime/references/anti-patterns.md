# Runtime Anti-Patterns

Configuration mistakes that cause runtime failures, with explanations and correct alternatives.

---

## Memory & Resource Anti-Patterns

### AP-1: No Memory Limit in Production

```bash
# WRONG — container can consume ALL host memory
docker run -d myapp:v1

# CORRECT — ALWAYS set memory limits in production
docker run -d -m 512m myapp:v1
```

**Why**: Without limits, a memory leak in one container can trigger the host OOM killer, taking down ALL containers on the host.

### AP-2: OOM Kill Disable Without Memory Limit

```bash
# WRONG — container can consume infinite memory, crash the host
docker run -d --oom-kill-disable myapp:v1

# CORRECT — ALWAYS pair with a memory limit
docker run -d -m 512m --oom-kill-disable myapp:v1
```

**Why**: `--oom-kill-disable` without `-m` means the kernel cannot reclaim memory from this container. If the application leaks memory, the host kernel will eventually kill random processes (including other containers) to free memory.

### AP-3: Setting Memory Too Low Without Profiling

```bash
# WRONG — arbitrary memory limit without profiling
docker run -d -m 64m java-app:v1

# CORRECT — profile first, then set limit with headroom
docker run -d --name test java-app:v1
docker stats test --no-stream
# NAME  MEM USAGE / LIMIT    MEM %
# test  487.2MiB / 7.775GiB  6.12%
# Now set limit with ~30% headroom
docker run -d -m 650m java-app:v1
```

**Why**: Setting memory limits too low causes OOM kills that look like application bugs. ALWAYS measure actual usage before setting limits.

### AP-4: No PID Limit (Fork Bomb Vulnerability)

```bash
# WRONG — no PID limit allows fork bombs to crash the host
docker run -d myapp:v1

# CORRECT — set PID limit to prevent fork bombs
docker run -d --pids-limit 200 myapp:v1
```

**Why**: A malicious or buggy process can fork infinitely, consuming all PIDs on the host and making it unresponsive.

---

## Process & Entrypoint Anti-Patterns

### AP-5: Daemonizing the Main Process

```dockerfile
# WRONG — nginx forks to background, PID 1 exits, container stops
CMD ["nginx"]

# CORRECT — run in foreground
CMD ["nginx", "-g", "daemon off;"]
```

**Why**: Docker monitors PID 1. If it exits (even with code 0), the container stops. Background daemons fork and let PID 1 exit.

### AP-6: Shell Form CMD/ENTRYPOINT

```dockerfile
# WRONG — wraps in /bin/sh -c, signals not forwarded to app
CMD node server.js

# CORRECT — exec form, app is PID 1, receives signals directly
CMD ["node", "server.js"]
```

**Why**: Shell form runs the process as a child of `/bin/sh`. SIGTERM goes to the shell, not the application. The app never gets a chance to shut down gracefully, and Docker force-kills it after the stop timeout.

### AP-7: Not Using exec in Entrypoint Scripts

```bash
# WRONG — entrypoint.sh
#!/bin/sh
echo "Starting app..."
node server.js
# Shell remains as PID 1, node is PID 2

# CORRECT — entrypoint.sh
#!/bin/sh
echo "Starting app..."
exec node server.js
# node replaces shell as PID 1
```

**Why**: Without `exec`, the shell process stays as PID 1. Signals are not forwarded to the actual application. `docker stop` kills the shell, leaving the app as an orphan that gets SIGKILL after the timeout.

### AP-8: CRLF Line Endings in Scripts

```dockerfile
# WRONG — script has Windows line endings, causes "exec format error"
COPY entrypoint.sh /entrypoint.sh

# CORRECT — convert line endings in Dockerfile
COPY entrypoint.sh /entrypoint.sh
RUN sed -i 's/\r$//' /entrypoint.sh && chmod +x /entrypoint.sh
```

**Better**: Add `.gitattributes` to the repository:
```
*.sh text eol=lf
Dockerfile text eol=lf
*.yml text eol=lf
*.yaml text eol=lf
```

**Why**: The kernel reads `#!/bin/sh\r` as the interpreter path. The `\r` (carriage return) is part of the path, and `/bin/sh\r` does not exist.

### AP-9: Missing Shebang in Entrypoint Scripts

```bash
# WRONG — no shebang, kernel doesn't know how to execute
echo "Starting..."
node server.js

# CORRECT — always include shebang
#!/bin/sh
echo "Starting..."
exec node server.js
```

**Why**: Without a shebang, the kernel cannot determine the interpreter. The exec system call fails with "exec format error."

---

## Networking Anti-Patterns

### AP-10: Using Default Bridge Network

```bash
# WRONG — containers can't resolve each other by name
docker run -d --name api myapi:v1
docker run -d --name web myweb:v1
docker exec web curl http://api:3000  # FAILS: name resolution error

# CORRECT — use a user-defined network
docker network create mynet
docker run -d --name api --network mynet myapi:v1
docker run -d --name web --network mynet myweb:v1
docker exec web curl http://api:3000  # WORKS
```

**Why**: The default bridge network does not provide DNS resolution between containers. User-defined bridge networks include an embedded DNS server.

### AP-11: Exposing Ports on 0.0.0.0 in Production

```bash
# WRONG — binds to all interfaces, accessible from any network
docker run -d -p 5432:5432 postgres:16

# CORRECT — bind to localhost or specific interface
docker run -d -p 127.0.0.1:5432:5432 postgres:16
```

**Why**: Binding to `0.0.0.0` exposes the port on all network interfaces, including public-facing ones. Database and internal services should NEVER be directly accessible from the internet.

### AP-12: Hardcoded Container IPs

```bash
# WRONG — IPs change on container restart
docker exec web curl http://172.18.0.3:3000

# CORRECT — use DNS names (container names or aliases)
docker exec web curl http://api:3000
```

**Why**: Container IP addresses are dynamic and change on restart, network reconnect, or redeployment. DNS names are stable.

---

## Volume & Storage Anti-Patterns

### AP-13: Writing Application Data to Container Layer

```dockerfile
# WRONG — data stored in container layer, lost on container removal
CMD ["node", "server.js"]
# Application writes to /app/data/ inside the container

# CORRECT — use a volume for persistent data
# docker run -d --mount type=volume,src=appdata,dst=/app/data myapp:v1
```

**Why**: The container's writable layer is ephemeral. When the container is removed (`docker rm`), all data in the writable layer is permanently lost.

### AP-14: Anonymous Volumes with --rm

```bash
# WRONG — anonymous volume deleted when container stops
docker run --rm -v /data mydb:v1

# CORRECT — use a named volume
docker run --rm --mount type=volume,src=dbdata,dst=/data mydb:v1
```

**Why**: The `--rm` flag removes anonymous volumes when the container exits. Named volumes persist regardless of `--rm`.

### AP-15: Bind Mounts in Production Without Understanding -v Auto-Create

```bash
# WRONG — -v auto-creates missing host directory as root-owned empty dir
docker run -v /host/config:/app/config myapp:v1
# If /host/config doesn't exist, Docker creates it as empty root-owned directory

# CORRECT — use --mount which errors on missing source
docker run --mount type=bind,src=/host/config,dst=/app/config myapp:v1
# Error: bind source path does not exist: /host/config
```

**Why**: The `-v` flag silently creates missing host directories, leading to containers starting with empty config directories instead of failing fast. `--mount` provides explicit error handling.

---

## Security Anti-Patterns

### AP-16: Running as Root

```dockerfile
# WRONG — process runs as root inside container
FROM node:20
COPY . /app
CMD ["node", "/app/server.js"]

# CORRECT — create and use non-root user
FROM node:20
RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser
COPY --chown=appuser:appuser . /app
USER appuser
CMD ["node", "/app/server.js"]
```

**Why**: If an attacker exploits the application, they have root access inside the container. Combined with misconfigurations (privileged mode, host mounts), this can lead to host compromise.

### AP-17: Using --privileged for Specific Capabilities

```bash
# WRONG — grants ALL capabilities + device access + disables security
docker run --privileged myapp:v1

# CORRECT — add only the specific capability needed
docker run --cap-add SYS_PTRACE myapp:v1
```

**Why**: `--privileged` disables ALL security isolation: capabilities, seccomp, AppArmor, and device restrictions. It is equivalent to running directly on the host.

### AP-18: Ignoring no-new-privileges

```bash
# WRONG — process can escalate privileges via setuid binaries
docker run myapp:v1

# CORRECT — prevent privilege escalation
docker run --security-opt no-new-privileges=true myapp:v1
```

**Why**: Without `no-new-privileges`, a process can gain additional privileges through setuid/setgid binaries. This is a common container escape vector.

---

## Restart & Lifecycle Anti-Patterns

### AP-19: Using restart=always for Everything

```bash
# WRONG — restarts even on clean exit, masks bugs
docker run -d --restart always debug-task:v1

# CORRECT — use on-failure for tasks that should only restart on errors
docker run -d --restart on-failure:5 debug-task:v1

# CORRECT — use unless-stopped for long-running services
docker run -d --restart unless-stopped nginx
```

**Why**: `restart=always` restarts the container even when it exits cleanly (code 0). This hides bugs where the container should stay stopped and wastes resources in restart loops.

### AP-20: No Health Check

```bash
# WRONG — Docker only knows if PID 1 is running, not if app is healthy
docker run -d myapp:v1

# CORRECT — add health check for actual application health
docker run -d \
  --health-cmd='curl -f http://localhost:8080/health || exit 1' \
  --health-interval=30s \
  --health-timeout=10s \
  --health-retries=3 \
  --health-start-period=40s \
  myapp:v1
```

**Why**: Without a health check, Docker considers a container "healthy" as long as PID 1 is running. The application could be deadlocked, out of connections, or returning errors — Docker won't know. Orchestrators (Compose, Swarm) use health status for dependency ordering and rolling updates.

### AP-21: Relying on restart=always Instead of Fixing Root Cause

```bash
# WRONG — masking a recurring crash with restart policy
docker run -d --restart always myapp:v1
# Container keeps crashing and restarting every 30 seconds

# CORRECT — investigate the root cause
docker update --restart no myapp
docker stop myapp
docker logs --tail 100 myapp
# Fix the bug, then deploy with appropriate restart policy
```

**Why**: Restart policies are a safety net, not a substitute for fixing bugs. A container in a restart loop consumes CPU, generates logs, and may leave corrupt state.

---

## Compose-Specific Anti-Patterns

### AP-22: depends_on Without Health Checks

```yaml
# WRONG — depends_on only waits for container start, not readiness
services:
  web:
    depends_on:
      - db
  db:
    image: postgres:16

# CORRECT — use depends_on with condition
services:
  web:
    depends_on:
      db:
        condition: service_healthy
  db:
    image: postgres:16
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 5s
      retries: 5
```

**Why**: `depends_on` without a health check condition only ensures the dependency container has started. The database may not be ready to accept connections yet, causing the web service to crash on startup.

### AP-23: Using container_name in Compose

```yaml
# WRONG — prevents scaling, causes name conflicts across projects
services:
  web:
    container_name: web
    image: nginx

# CORRECT — let Compose manage names (project_service_N)
services:
  web:
    image: nginx
```

**Why**: Fixed container names prevent `docker compose up --scale web=3` and cause conflicts if multiple Compose projects use the same name.
