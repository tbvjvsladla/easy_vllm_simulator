# Container Management Workflows

Practical workflows for common Docker container operations. Docker Engine 24+.

---

## 1. Development Workflows

### Start Interactive Development Container

```bash
# Node.js development with live code mount
docker run -it --rm \
  --name dev \
  -v "$(pwd)":/app \
  -w /app \
  -p 3000:3000 \
  node:20-alpine \
  sh

# Python development with pip cache
docker run -it --rm \
  --name pydev \
  -v "$(pwd)":/app \
  -v pipcache:/root/.cache/pip \
  -w /app \
  -p 8000:8000 \
  python:3.12-slim \
  bash
```

### Run One-Off Commands

```bash
# Run tests
docker run --rm -v "$(pwd)":/app -w /app node:20-alpine npm test

# Run database migration
docker run --rm \
  --network mynet \
  -e DATABASE_URL=postgres://user:pass@db:5432/mydb \
  myapp:latest \
  python manage.py migrate

# Generate build artifact
docker run --rm -v "$(pwd)":/app -w /app golang:1.22 go build -o /app/server
```

### Debug a Failing Container

```bash
# 1. Check why it exited
docker ps -a --filter name=myapp
docker logs --tail 50 myapp

# 2. Inspect the state
docker inspect --format='{{.State.ExitCode}}' myapp
docker inspect --format='{{.State.Error}}' myapp

# 3. Check filesystem changes
docker diff myapp

# 4. Start a shell in the stopped container's image
docker run -it --rm --entrypoint sh myapp:latest

# 5. Copy logs out before removing
docker cp myapp:/var/log/app.log ./debug-log.txt
docker rm myapp
```

---

## 2. Production Workflows

### Launch Production Service

```bash
docker run -d \
  --name api \
  --restart unless-stopped \
  --init \
  -u 1000:1000 \
  --read-only \
  --tmpfs /tmp \
  --cap-drop ALL \
  --security-opt no-new-privileges=true \
  -m 512m --cpus 1.5 --pids-limit 200 \
  -p 127.0.0.1:8080:8080 \
  --network production \
  --mount source=api-data,target=/data \
  -e NODE_ENV=production \
  --env-file /etc/myapp/env \
  --log-opt max-size=10m --log-opt max-file=5 \
  --health-cmd='curl -sf http://localhost:8080/health || exit 1' \
  --health-interval=30s \
  --health-timeout=5s \
  --health-retries=3 \
  --health-start-period=60s \
  myapp:v2.1.0
```

### Launch Database with Persistent Storage

```bash
# PostgreSQL
docker run -d \
  --name postgres \
  --restart unless-stopped \
  -u 999:999 \
  --network production \
  --mount source=pgdata,target=/var/lib/postgresql/data \
  -e POSTGRES_PASSWORD_FILE=/run/secrets/pg_password \
  --mount type=bind,src=/etc/secrets/pg_password,dst=/run/secrets/pg_password,readonly \
  -m 1g --cpus 2 \
  --shm-size 256m \
  --log-opt max-size=10m --log-opt max-file=3 \
  postgres:16-alpine

# Redis with memory limit
docker run -d \
  --name redis \
  --restart unless-stopped \
  --network production \
  --mount source=redisdata,target=/data \
  -m 256m \
  redis:7-alpine \
  redis-server --maxmemory 200mb --maxmemory-policy allkeys-lru
```

### Graceful Deployment (Blue-Green)

```bash
# 1. Start new version alongside old
docker run -d --name api-v2 --network production \
  -p 127.0.0.1:8081:8080 myapp:v2.0.0

# 2. Wait for health check
until docker inspect --format='{{.State.Health.Status}}' api-v2 | grep -q healthy; do
  sleep 2
done

# 3. Switch traffic (update reverse proxy, then)
docker stop api-v1
docker rm api-v1

# 4. Rename new container
docker rename api-v2 api
```

---

## 3. Monitoring Workflows

### Live Resource Monitoring

```bash
# All running containers
docker stats

# Specific containers with custom format
docker stats --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.MemPerc}}\t{{.NetIO}}\t{{.PIDs}}" \
  api postgres redis

# Single snapshot for scripts
docker stats --no-stream --format "{{.Name}}: CPU={{.CPUPerc}} MEM={{.MemUsage}}"
```

### Log Analysis

```bash
# Last 100 lines with timestamps
docker logs -t --tail 100 myapp

# Logs from the last hour
docker logs --since 1h myapp

# Follow live logs
docker logs -f --tail 0 myapp

# Logs between two timestamps
docker logs --since 2024-06-01T10:00:00 --until 2024-06-01T11:00:00 myapp

# Search logs (pipe to grep)
docker logs myapp 2>&1 | grep -i error
```

### Health Check Monitoring

```bash
# Current health status
docker inspect --format='{{.State.Health.Status}}' myapp

# Health check log (last 5 results)
docker inspect --format='{{range .State.Health.Log}}{{.ExitCode}} {{.Output}}{{end}}' myapp

# Full health JSON
docker inspect --format='{{json .State.Health}}' myapp | jq .

# List all unhealthy containers
docker ps --filter health=unhealthy --format "{{.Names}}: {{.Status}}"
```

### Event Monitoring

```bash
# Watch container start/stop/die events
docker events --filter type=container --filter event=start --filter event=stop --filter event=die

# Watch with structured output
docker events --format '{{.Time}} {{.Action}} {{.Actor.Attributes.name}}'

# Events from the last 10 minutes
docker events --since 10m --until 0s
```

---

## 4. Maintenance Workflows

### Batch Container Management

```bash
# Stop all running containers
docker stop $(docker ps -q)

# Remove all stopped containers
docker container prune -f

# Remove containers by label
docker rm $(docker ps -aq --filter label=environment=staging)

# Remove containers older than 24h
docker container prune -f --filter "until=24h"

# Restart all containers on a specific network
docker ps -q --filter network=production | xargs -r docker restart
```

### Resource Limit Adjustment

```bash
# Increase memory for a running container
docker update --memory 1g --memory-swap 2g myapp

# Add CPU resources
docker update --cpus 4 myapp

# Change restart policy
docker update --restart unless-stopped myapp

# Apply limits to multiple containers
docker update --memory 256m --cpus 0.5 worker1 worker2 worker3
```

### Backup and Restore

```bash
# Backup: copy data from volume via helper container
docker run --rm \
  --mount source=pgdata,target=/data,readonly \
  -v "$(pwd)/backups":/backup \
  alpine \
  tar czf /backup/pgdata-$(date +%Y%m%d).tar.gz -C /data .

# Restore: extract backup into volume
docker run --rm \
  --mount source=pgdata,target=/data \
  -v "$(pwd)/backups":/backup \
  alpine \
  sh -c "rm -rf /data/* && tar xzf /backup/pgdata-20240601.tar.gz -C /data"

# Copy specific files from container
docker cp myapp:/app/uploads ./uploads-backup

# Copy config into running container
docker cp ./nginx.conf proxy:/etc/nginx/nginx.conf
docker exec proxy nginx -s reload
```

---

## 5. Troubleshooting Workflows

### Container Keeps Restarting

```bash
# 1. Check restart count and status
docker inspect --format='{{.RestartCount}} restarts, last exit: {{.State.ExitCode}}' myapp

# 2. Read recent logs
docker logs --tail 50 myapp

# 3. Check OOM kill
docker inspect --format='{{.State.OOMKilled}}' myapp

# 4. Check resource usage
docker stats --no-stream myapp

# 5. Temporarily disable restart to investigate
docker update --restart no myapp
docker stop myapp
docker logs --tail 200 myapp
```

### Container Cannot Reach Another Container

```bash
# 1. Verify both are on the same network
docker inspect --format='{{range $k, $v := .NetworkSettings.Networks}}{{$k}} {{end}}' container1
docker inspect --format='{{range $k, $v := .NetworkSettings.Networks}}{{$k}} {{end}}' container2

# 2. Check DNS resolution
docker exec container1 nslookup container2

# 3. Check connectivity
docker exec container1 ping -c 2 container2

# 4. Check container2 is actually listening
docker exec container2 netstat -tlnp

# 5. Check /etc/resolv.conf
docker exec container1 cat /etc/resolv.conf
```

### Port Already In Use

```bash
# Find what is using the port
# Linux:
lsof -i :8080
# or
ss -tlnp | grep 8080

# Find which Docker container holds the port
docker ps --filter publish=8080

# Use a different host port
docker run -p 8081:8080 myapp
```

### High Memory Usage Investigation

```bash
# 1. Check current memory usage
docker stats --no-stream --format "table {{.Name}}\t{{.MemUsage}}\t{{.MemPerc}}"

# 2. Check memory limit
docker inspect --format='{{.HostConfig.Memory}}' myapp

# 3. Check if OOM killed
docker inspect --format='{{.State.OOMKilled}}' myapp

# 4. Check processes inside container
docker top myapp -o pid,rss,cmd

# 5. Increase memory limit if needed
docker update --memory 1g myapp
```

---

## 6. Scripting Patterns

### Wait for Container Health

```bash
#!/bin/bash
CONTAINER=$1
TIMEOUT=60
ELAPSED=0

until [ "$(docker inspect --format='{{.State.Health.Status}}' "$CONTAINER" 2>/dev/null)" = "healthy" ]; do
  if [ $ELAPSED -ge $TIMEOUT ]; then
    echo "ERROR: $CONTAINER did not become healthy within ${TIMEOUT}s"
    docker logs --tail 20 "$CONTAINER"
    exit 1
  fi
  sleep 2
  ELAPSED=$((ELAPSED + 2))
done
echo "$CONTAINER is healthy"
```

### Get Container IP on Specific Network

```bash
docker inspect --format='{{(index .NetworkSettings.Networks "mynet").IPAddress}}' myapp
```

### List All Container Port Mappings

```bash
docker ps --format '{{.Names}}: {{.Ports}}' | grep -v "^$"
```

### Bulk Inspect with jq

```bash
# All container IPs
docker inspect $(docker ps -q) | jq -r '.[].NetworkSettings.Networks | to_entries[] | "\(.key): \(.value.IPAddress)"'

# All containers with their restart policy
docker inspect $(docker ps -aq) | jq -r '.[] | "\(.Name): \(.HostConfig.RestartPolicy.Name)"'
```

### Clean Exit with Wait

```bash
# Start container and wait for completion
docker run -d --name job myapp:latest process-data
EXIT_CODE=$(docker wait job)
docker logs job > job-output.log
docker rm job
exit $EXIT_CODE
```
