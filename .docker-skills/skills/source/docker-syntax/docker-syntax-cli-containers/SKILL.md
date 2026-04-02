---
name: docker-syntax-cli-containers
description: >
  Use when running, inspecting, or managing Docker containers from the CLI.
  Prevents silent data loss from missing -v on rm, zombie containers from
  skipped stop commands, and incorrect docker run flag combinations.
  Covers docker run, exec, logs, inspect, stats, ps, cp, stop, kill, rm,
  prune, and container lifecycle management.
  Keywords: docker run, docker exec, docker logs, docker inspect, docker ps,
  docker stop, docker rm, how to run container, check container logs,
  enter running container, list containers, manage containers.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-syntax-cli-containers

## Quick Reference

### Container Lifecycle Overview

| Command | Purpose | Key Flags |
|---------|---------|-----------|
| `docker run` | Create and start container | `-d`, `-it`, `--rm`, `--name`, `-p`, `-v` |
| `docker create` | Create without starting | Same as `run` |
| `docker start` | Start stopped container | `-a` (attach), `-i` (interactive) |
| `docker stop` | Graceful stop (SIGTERM) | `-t` (grace period, default 10s) |
| `docker restart` | Stop then start | `-t` (grace period) |
| `docker kill` | Immediate signal | `-s` (signal, default SIGKILL) |
| `docker rm` | Remove container | `-f` (force), `-v` (volumes) |
| `docker container prune` | Remove all stopped | `--filter` |
| `docker exec` | Run command in container | `-it`, `-u`, `-w`, `-e` |
| `docker logs` | Read container output | `-f`, `--tail`, `--since` |
| `docker inspect` | Container metadata | `--format` (Go templates) |
| `docker ps` | List containers | `-a`, `--filter`, `--format` |
| `docker stats` | Live resource usage | `--no-stream`, `--format` |
| `docker top` | Container processes | Accepts `ps` options |
| `docker events` | Real-time daemon events | `--filter`, `--since` |
| `docker cp` | Copy files in/out | `-a` (archive mode) |
| `docker diff` | Filesystem changes | A=Added, C=Changed, D=Deleted |
| `docker rename` | Rename container | — |
| `docker update` | Change resource limits | `--memory`, `--cpus`, `--restart` |
| `docker pause` | Freeze container | — |
| `docker unpause` | Resume container | — |
| `docker wait` | Block until exit | Returns exit code |
| `docker port` | Show port mappings | — |
| `docker attach` | Attach to STDIN/STDOUT | `--detach-keys` |

### docker run Flag Categories

| Category | Key Flags | Details |
|----------|-----------|---------|
| Execution | `-d`, `-it`, `--rm`, `--name`, `--init` | [references/commands.md#execution](references/commands.md) |
| Ports & Network | `-p`, `--network`, `--hostname`, `--dns` | [references/commands.md#ports--network](references/commands.md) |
| Storage | `-v`, `--mount`, `--read-only`, `--tmpfs` | [references/commands.md#storage](references/commands.md) |
| Environment | `-e`, `--env-file`, `-w`, `--entrypoint`, `-u` | [references/commands.md#environment](references/commands.md) |
| Resources | `-m`, `--cpus`, `--pids-limit`, `--ulimit` | [references/commands.md#resources](references/commands.md) |
| Security | `--cap-add`, `--cap-drop`, `--security-opt`, `--read-only` | [references/commands.md#security](references/commands.md) |
| Health | `--health-cmd`, `--health-interval`, `--health-retries` | [references/commands.md#health](references/commands.md) |
| Restart | `--restart no\|always\|unless-stopped\|on-failure[:N]` | [references/commands.md#restart](references/commands.md) |
| Logging | `--log-driver`, `--log-opt` | [references/commands.md#logging](references/commands.md) |
| Pull & Platform | `--pull`, `--platform` | [references/commands.md#pull--platform](references/commands.md) |

### Critical Warnings

**NEVER** use `--privileged` in production -- it grants the container full host access. ALWAYS use specific `--cap-add` flags for the exact capabilities needed.

**NEVER** run containers as root unless technically required. ALWAYS use `-u 1000:1000` or define `USER` in the Dockerfile.

**NEVER** use `--link` for container communication -- it is legacy. ALWAYS use user-defined bridge networks with `--network`.

**NEVER** use `docker exec` with chained commands directly -- the shell interprets `&&` on the host. ALWAYS wrap in `sh -c "cmd1 && cmd2"`.

**NEVER** run `docker rm -f` on production databases without confirming backups. ALWAYS stop gracefully with `docker stop` first.

**ALWAYS** use `--rm` for one-off containers (tests, migrations, debug shells) to prevent stopped container accumulation.

**ALWAYS** set `--init` when running applications that do not handle signals or reap zombie processes (e.g., shell scripts, Node.js without signal handlers).

---

## docker ps -- Filter Cheat Sheet

### Filter Options

| Filter | Match Type | Example |
|--------|-----------|---------|
| `name` | Substring | `--filter name=web` |
| `status` | Exact | `--filter status=running` |
| `ancestor` | Image name/tag/ID | `--filter ancestor=nginx:latest` |
| `label` | Key or key=value | `--filter label=app=web` |
| `exited` | Exit code (with `-a`) | `--filter exited=0` |
| `health` | Health status | `--filter health=healthy` |
| `network` | Network name/ID | `--filter network=mynet` |
| `volume` | Volume name/mount | `--filter volume=mydata` |
| `publish` | Published port | `--filter publish=80/tcp` |
| `before`/`since` | Relative to container | `--filter before=myapp` |

### Status Values

`created` | `restarting` | `running` | `removing` | `paused` | `exited` | `dead`

### Format Placeholders

| Placeholder | Output |
|-------------|--------|
| `{{.ID}}` | Container ID |
| `{{.Names}}` | Container name |
| `{{.Image}}` | Image name |
| `{{.Status}}` | Detailed status with health |
| `{{.State}}` | State (running/exited) |
| `{{.Ports}}` | Port mappings |
| `{{.RunningFor}}` | Uptime |
| `{{.Size}}` | Disk usage (with `-s`) |
| `{{.Label "key"}}` | Specific label value |
| `{{.Networks}}` | Network names |
| `{{.Mounts}}` | Volume names |

```bash
# Useful one-liners
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
docker ps -a --filter status=exited --format "{{.Names}}: exited {{.Status}}"
docker ps -q --filter status=exited | xargs docker rm
docker ps --format json
```

---

## docker inspect -- Common Format Strings

### Container State

```bash
docker inspect --format='{{.State.Status}}' CONTAINER
docker inspect --format='{{.State.Running}}' CONTAINER
docker inspect --format='{{.State.ExitCode}}' CONTAINER
docker inspect --format='{{.State.Pid}}' CONTAINER
docker inspect --format='{{.State.StartedAt}}' CONTAINER
```

### Network Information

```bash
# IP address
docker inspect --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' CONTAINER

# Port bindings
docker inspect --format='{{range $p, $conf := .NetworkSettings.Ports}}{{$p}} -> {{(index $conf 0).HostPort}}{{end}}' CONTAINER

# Network name
docker inspect --format='{{range $k, $v := .NetworkSettings.Networks}}{{$k}}{{end}}' CONTAINER
```

### Configuration

```bash
docker inspect --format='{{.Config.Image}}' CONTAINER
docker inspect --format='{{json .Config.Cmd}}' CONTAINER
docker inspect --format='{{range .Config.Env}}{{println .}}{{end}}' CONTAINER
docker inspect --format='{{json .Config.Labels}}' CONTAINER
docker inspect --format='{{index .Config.Labels "com.example.version"}}' CONTAINER
```

### Mounts

```bash
docker inspect --format='{{range .Mounts}}{{.Source}} -> {{.Destination}}{{println}}{{end}}' CONTAINER
docker inspect --format='{{json .Mounts}}' CONTAINER
```

### Size

```bash
docker inspect --size -f '{{.SizeRootFs}}' CONTAINER
docker inspect --size -f '{{.SizeRw}}' CONTAINER
```

### Health Check

```bash
docker inspect --format='{{.State.Health.Status}}' CONTAINER
docker inspect --format='{{json .State.Health}}' CONTAINER
```

---

## Decision Trees

### Which Run Mode?

```
Need interactive shell?
├── YES → docker run -it --rm IMAGE bash
└── NO → Need background daemon?
    ├── YES → docker run -d --name NAME IMAGE
    └── NO → One-off command?
        ├── YES → docker run --rm IMAGE COMMAND
        └── NO → docker create + docker start
```

### Which Stop Method?

```
Need graceful shutdown?
├── YES → docker stop [-t SECONDS] CONTAINER
│         (sends SIGTERM, waits grace period, then SIGKILL)
└── NO → Need immediate termination?
    ├── YES → docker kill CONTAINER
    └── NO → Need to send specific signal?
        └── YES → docker kill -s SIGNAL CONTAINER
```

### Exec or Attach?

```
Need to run a NEW command in running container?
├── YES → docker exec -it CONTAINER COMMAND
└── NO → Need to connect to the MAIN process?
    ├── YES → docker attach CONTAINER
    │         (Ctrl+P, Ctrl+Q to detach without stopping)
    └── NO → Just need logs?
        └── YES → docker logs -f CONTAINER
```

---

## Common Patterns

### Production Container Launch

```bash
docker run -d \
  --name myapp \
  --restart unless-stopped \
  --init \
  -u 1000:1000 \
  --read-only \
  --tmpfs /tmp \
  --cap-drop ALL \
  --security-opt no-new-privileges=true \
  -m 512m --cpus 1.5 --pids-limit 200 \
  -p 127.0.0.1:8080:8080 \
  --network mynet \
  --mount source=appdata,target=/data \
  -e NODE_ENV=production \
  --log-opt max-size=10m --log-opt max-file=3 \
  --health-cmd='curl -f http://localhost:8080/health || exit 1' \
  --health-interval=30s \
  myapp:v1.2.3
```

### Debug a Running Container

```bash
# Shell into container
docker exec -it myapp sh

# Check processes
docker top myapp

# Stream logs
docker logs -f --tail 100 myapp

# Resource usage
docker stats myapp --no-stream

# Filesystem changes
docker diff myapp

# Full metadata
docker inspect myapp | jq '.[0].State'
```

### Container Cleanup

```bash
# Remove specific stopped container and its anonymous volumes
docker rm -v myapp

# Remove ALL stopped containers
docker container prune -f

# Remove containers older than 24 hours
docker container prune -f --filter "until=24h"

# Force remove running container (emergency only)
docker rm -f myapp
```

### Copy Files

```bash
# Extract logs from container
docker cp myapp:/var/log/app.log ./app.log

# Inject config into running container
docker cp ./config.yml myapp:/app/config.yml

# Archive mode (preserves UID/GID)
docker cp -a myapp:/app/data ./backup/
```

### Update Running Container Resources

```bash
docker update --memory 1g --cpus 2 myapp
docker update --restart always myapp
docker update --pids-limit 300 myapp
```

---

## Reference Links

- [references/commands.md](references/commands.md) -- Complete flag reference for all container commands
- [references/examples.md](references/examples.md) -- Common container management workflows
- [references/anti-patterns.md](references/anti-patterns.md) -- CLI misuse patterns and corrections

### Official Sources

- https://docs.docker.com/reference/cli/docker/container/
- https://docs.docker.com/reference/cli/docker/container/run/
- https://docs.docker.com/reference/cli/docker/container/exec/
- https://docs.docker.com/reference/cli/docker/container/logs/
- https://docs.docker.com/reference/cli/docker/container/ls/
- https://docs.docker.com/reference/cli/docker/inspect/
