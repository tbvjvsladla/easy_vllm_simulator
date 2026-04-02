# Runtime Debugging Examples

Step-by-step debugging sessions with real commands for common Docker runtime failures.

---

## Example 1: Container Exits Immediately

### Scenario
A web application container starts and immediately exits with code 0.

### Debugging Session

```bash
# Step 1: Check the container status
docker ps -a --filter name=webapp
# CONTAINER ID  IMAGE      STATUS                    NAMES
# abc123        myapp:v1   Exited (0) 2 seconds ago  webapp

# Step 2: Check logs
docker logs webapp
# (empty output or daemon startup message)

# Step 3: Inspect the CMD
docker inspect --format='Entrypoint={{.Config.Entrypoint}} Cmd={{.Config.Cmd}}' webapp
# Entrypoint=[] Cmd=[nginx]

# Step 4: The problem — nginx daemonizes by default
# Fix: Run nginx in foreground mode
docker run -d --name webapp myapp:v1 nginx -g "daemon off;"

# Or fix in Dockerfile:
# CMD ["nginx", "-g", "daemon off;"]
```

### Root Cause
nginx (and many other services like Apache, sshd) daemonize by default. The container's main process (PID 1) forks a background process and exits. Docker sees PID 1 exit with code 0 and stops the container.

### Prevention
ALWAYS ensure the main process runs in the foreground:
- nginx: `nginx -g "daemon off;"`
- Apache: `httpd -D FOREGROUND`
- sshd: `/usr/sbin/sshd -D`
- Custom scripts: Use `exec` to replace shell with the actual process

---

## Example 2: OOM Killed Container

### Scenario
A Java application container randomly crashes after running for a while.

### Debugging Session

```bash
# Step 1: Check exit code and OOM status
docker inspect --format='ExitCode={{.State.ExitCode}} OOM={{.State.OOMKilled}}' myapp
# ExitCode=137 OOM=true

# Step 2: Check current memory limit
docker inspect --format='Memory={{.HostConfig.Memory}}' myapp
# Memory=268435456  (256MB)

# Step 3: Check actual memory usage before it crashed
docker events --filter container=myapp --filter event=oom --since 1h
# 2026-03-19T14:22:33 container oom abc123 (name=myapp, image=myapp:v1)

# Step 4: Profile memory usage with a fresh container
docker run -d --name myapp-test -m 1g myapp:v1
docker stats myapp-test --no-stream
# NAME        CPU%  MEM USAGE / LIMIT    MEM %
# myapp-test  2.3%  487.2MiB / 1GiB      47.58%

# Step 5: The problem — 256MB is too low for a JVM application
# Fix: Set appropriate memory limit based on profiling
docker run -d --name myapp -m 1g \
  -e JAVA_OPTS="-Xmx512m -Xms256m" \
  myapp:v1
```

### Root Cause
The Java application's default heap size exceeded the 256MB container memory limit. The Linux OOM killer terminated the process (SIGKILL = exit code 137).

### Prevention
- ALWAYS profile actual memory usage with `docker stats` before setting limits
- For JVM apps: Set `-Xmx` to ~75% of the container memory limit
- Set `--memory-reservation` as a soft limit for orchestrator awareness

---

## Example 3: Permission Denied on Volume

### Scenario
An application cannot write to a mounted volume directory.

### Debugging Session

```bash
# Step 1: Check logs for the error
docker logs myapp
# Error: EACCES: permission denied, open '/data/output.json'

# Step 2: Check the user the container runs as
docker inspect --format='{{.Config.User}}' myapp
# 1001

# Step 3: Check file ownership on the volume
docker exec myapp ls -la /data
# drwxr-xr-x 2 root root 4096 Mar 19 12:00 .
# -rw-r--r-- 1 root root  128 Mar 19 12:00 config.json

# Step 4: The problem — container runs as UID 1001 but /data is owned by root
# Fix option A: Run as root (NOT recommended for production)
docker run -u 0 -v mydata:/data myapp:v1

# Fix option B: Match UIDs (preferred)
docker run -u $(id -u):$(id -g) -v mydata:/data myapp:v1

# Fix option C: Fix ownership in Dockerfile (best)
# In Dockerfile:
# RUN mkdir -p /data && chown -R 1001:1001 /data
# USER 1001
```

### Root Cause
The container process runs as UID 1001 (non-root) but the volume directory is owned by root (UID 0). Linux file permissions prevent the write.

### Prevention
- ALWAYS set ownership of data directories in the Dockerfile before switching to non-root user
- Use named volumes (Docker manages permissions) instead of bind mounts where possible
- On Linux with bind mounts and SELinux: add `:z` or `:Z` suffix

---

## Example 4: Port Already in Use

### Scenario
Starting a container fails with a port conflict error.

### Debugging Session

```bash
# Step 1: See the error
docker run -d -p 8080:80 --name web nginx
# Error: driver failed programming external connectivity:
# Bind for 0.0.0.0:8080 failed: port is already allocated

# Step 2: Check what Docker container uses that port
docker ps --format "{{.Names}}: {{.Ports}}" | grep 8080
# old-web: 0.0.0.0:8080->80/tcp

# Step 3a: If it's a Docker container — stop it
docker stop old-web && docker rm old-web

# Step 3b: If no Docker container found — check host processes
lsof -i :8080
# COMMAND  PID   USER  FD  TYPE DEVICE SIZE NODE NAME
# node     1234  user  12u IPv4 54321  TCP *:8080 (LISTEN)

# Step 3c: Use a different host port instead
docker run -d -p 8081:80 --name web nginx

# Step 4: Start the container
docker run -d -p 8080:80 --name web nginx
```

### Prevention
- Use `--rm` flag so containers auto-remove on stop
- Use Docker Compose with unique project names for automatic port management
- Use `docker compose down` before `docker compose up` to clean previous state

---

## Example 5: Exec Format Error

### Scenario
A container fails to start with an exec format error.

### Debugging Session

```bash
# Step 1: See the error
docker run myapp:v1
# exec /entrypoint.sh: exec format error

# Step 2: Check the entrypoint script
docker run --entrypoint cat myapp:v1 /entrypoint.sh | head -1
# #!/bin/bash^M

# Step 3: The problem — CRLF line endings (^M = \r)
# The kernel cannot find interpreter "#!/bin/bash\r"

# Fix option A: In Dockerfile, convert line endings
# RUN sed -i 's/\r$//' /entrypoint.sh

# Fix option B: Configure Git on the build machine
# git config core.autocrlf input

# Fix option C: Add .gitattributes to the repo
# *.sh text eol=lf
# Dockerfile text eol=lf
```

### Alternative Scenario: Architecture Mismatch

```bash
# Step 1: See the error
docker run myapp:v1
# exec /app/server: exec format error

# Step 2: Check image and host architecture
docker inspect --format='{{.Architecture}}' myapp:v1
# arm64

uname -m
# x86_64

# Step 3: The problem — ARM image on x86 host
# Fix: Pull or build for the correct platform
docker pull --platform linux/amd64 myapp:v1
# Or build for the correct platform
docker buildx build --platform linux/amd64 -t myapp:v1 .
```

### Prevention
- ALWAYS add `.gitattributes` with `*.sh text eol=lf` to repositories
- ALWAYS add shebang (`#!/bin/sh`) as the first line of entrypoint scripts
- ALWAYS verify target architecture matches build architecture
- Use multi-platform builds: `docker buildx build --platform linux/amd64,linux/arm64`

---

## Example 6: Read-Only Filesystem Failures

### Scenario
An application fails to write temporary files in a security-hardened container.

### Debugging Session

```bash
# Step 1: Check logs
docker logs myapp
# Error: EROFS: read-only file system, open '/tmp/cache.json'

# Step 2: Confirm read-only mode
docker inspect --format='{{.HostConfig.ReadonlyRootfs}}' myapp
# true

# Step 3: Check what paths the app needs to write to
docker run --rm myapp:v1 sh -c "find / -writable 2>/dev/null"
# (no output — everything is read-only)

# Step 4: Fix — add tmpfs mounts for writable paths
docker run -d --read-only \
  --tmpfs /tmp:size=64m \
  --tmpfs /var/cache:size=32m \
  --mount type=volume,src=appdata,dst=/app/data \
  myapp:v1

# Step 5: Verify the fix
docker exec myapp touch /tmp/test-file && echo "OK"
# OK
```

### Prevention
- When using `--read-only`, ALWAYS map writable paths with tmpfs or volumes
- Common writable paths: `/tmp`, `/var/tmp`, `/run`, `/var/run`, `/var/cache`
- Application data ALWAYS goes on a volume, NEVER in the container layer

---

## Example 7: Container Restart Loop

### Scenario
A container keeps restarting and never becomes stable.

### Debugging Session

```bash
# Step 1: Check restart count
docker inspect --format='RestartCount={{.RestartCount}} Policy={{.HostConfig.RestartPolicy.Name}}' myapp
# RestartCount=47 Policy=always

# Step 2: Stop the restart loop
docker update --restart no myapp
docker stop myapp

# Step 3: Check what's going wrong
docker logs --tail 50 myapp
# Error: Connection refused to database at db:5432
# Retry 1/5... failed
# Retry 5/5... failed
# Exiting with error

# Step 4: The problem — database dependency not available
# Check if the database is running
docker ps --filter name=db
# (no results — db container is not running)

# Step 5: Fix — start the dependency first
docker start db
# Wait for it to be ready
docker exec db pg_isready
# /var/run/postgresql:5432 - accepting connections

# Step 6: Restart the application
docker update --restart unless-stopped myapp
docker start myapp

# Step 7: Verify
docker ps --filter name=myapp
# STATUS: Up 30 seconds
```

### Prevention
- Use Docker Compose with `depends_on` and health checks for service ordering
- Implement proper health checks in application containers
- Use exponential backoff in application retry logic
- NEVER rely on container restart policy alone for dependency management

---

## Example 8: Debugging Network Connectivity

### Scenario
A container cannot reach another container by hostname.

### Debugging Session

```bash
# Step 1: Check what network the containers are on
docker inspect --format='{{range $k, $v := .NetworkSettings.Networks}}{{$k}} {{end}}' webapp
# bridge

docker inspect --format='{{range $k, $v := .NetworkSettings.Networks}}{{$k}} {{end}}' api
# bridge

# Step 2: Test DNS resolution
docker exec webapp nslookup api
# ** server can't find api: NXDOMAIN

# Step 3: The problem — default bridge network has no DNS resolution
# Fix: Create a user-defined network
docker network create mynet
docker network connect mynet webapp
docker network connect mynet api

# Step 4: Test again
docker exec webapp nslookup api
# Name:    api
# Address: 172.18.0.3

docker exec webapp curl http://api:3000/health
# {"status":"ok"}
```

### Prevention
- ALWAYS use user-defined bridge networks, NEVER the default bridge
- Use Docker Compose (automatically creates a user-defined network)
- Use `--network-alias` for additional DNS names
