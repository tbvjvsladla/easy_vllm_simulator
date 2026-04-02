# Container Command Reference

Complete flag reference for all Docker container CLI commands. Docker Engine 24+.

---

## docker run

Syntax: `docker run [OPTIONS] IMAGE [COMMAND] [ARG...]`

Creates and starts a container in one step.

### Execution

| Flag | Description | Default | Example |
|------|-------------|---------|---------|
| `-d, --detach` | Run in background, print container ID | foreground | `docker run -d nginx` |
| `-i, --interactive` | Keep STDIN open | closed | `docker run -i ubuntu cat` |
| `-t, --tty` | Allocate pseudo-TTY | none | `docker run -it ubuntu bash` |
| `--rm` | Auto-remove container on exit | persist | `docker run --rm ubuntu echo hello` |
| `--name` | Assign container name | auto-generated | `docker run --name web nginx` |
| `--init` | Run init process (forwards signals, reaps zombies) | disabled | `docker run --init node app.js` |

### Ports & Network

| Flag | Description | Example |
|------|-------------|---------|
| `-p, --publish` | Publish port `[host-ip:]host-port:container-port[/proto]` | `-p 8080:80` |
| `-P, --publish-all` | Publish all EXPOSE ports to random host ports | `-P` |
| `--network` | Connect to network (bridge, host, none, custom) | `--network mynet` |
| `--network-alias` | Add DNS alias on the network | `--network-alias web` |
| `--ip` | Static IPv4 address (user-defined networks only) | `--ip 172.20.0.5` |
| `--ip6` | Static IPv6 address | `--ip6 2001:db8::33` |
| `--dns` | Custom DNS server | `--dns 8.8.8.8` |
| `--dns-search` | Custom DNS search domain | `--dns-search example.com` |
| `-h, --hostname` | Set container hostname | `-h myhost` |
| `--add-host` | Add host-to-IP mapping to /etc/hosts | `--add-host myhost=8.8.8.8` |
| `--mac-address` | Set MAC address | `--mac-address 92:d0:c6:0a:29:33` |
| `--expose` | Expose port (documentation only, no host binding) | `--expose 80` |
| `--link` | **LEGACY** -- NEVER use. Use `--network` instead | — |

### Storage

| Flag | Description | Example |
|------|-------------|---------|
| `-v, --volume` | Bind mount or named volume (`name:path[:opts]`) | `-v mydata:/data` |
| `--mount` | Declarative mount (preferred for production) | `--mount type=volume,src=mydata,dst=/data` |
| `--volumes-from` | Mount volumes from another container | `--volumes-from web:ro` |
| `--read-only` | Read-only root filesystem | `--read-only` |
| `--tmpfs` | Mount tmpfs (in-memory filesystem) | `--tmpfs /run:size=64k` |

### Environment

| Flag | Description | Example |
|------|-------------|---------|
| `-e, --env` | Set environment variable | `-e DB_HOST=db` |
| `--env-file` | Read env vars from file | `--env-file .env` |
| `-w, --workdir` | Set working directory inside container | `-w /app` |
| `--entrypoint` | Override image ENTRYPOINT | `--entrypoint /bin/sh` |
| `-u, --user` | Run as user (name or UID[:GID]) | `-u 1000:1000` |
| `-l, --label` | Set metadata label | `-l app=web` |
| `--label-file` | Read labels from file | `--label-file labels.txt` |

### Resources

| Flag | Description | Example |
|------|-------------|---------|
| `-m, --memory` | Memory limit (bytes, k, m, g) | `-m 512m` |
| `--memory-reservation` | Memory soft limit | `--memory-reservation 256m` |
| `--memory-swap` | Total memory + swap limit (-1 = unlimited) | `--memory-swap 1g` |
| `--cpus` | Number of CPUs (decimal) | `--cpus 1.5` |
| `-c, --cpu-shares` | CPU shares (relative weight, default 1024) | `-c 2048` |
| `--cpuset-cpus` | Pin to specific CPUs | `--cpuset-cpus 0-3` |
| `--pids-limit` | Max PIDs in container (fork bomb prevention) | `--pids-limit 200` |
| `--ulimit` | Set ulimit values | `--ulimit nofile=1024:2048` |
| `--shm-size` | /dev/shm size | `--shm-size 1g` |
| `--blkio-weight` | Block I/O weight (10-1000) | `--blkio-weight 300` |
| `--device-read-bps` | Limit device read rate | `--device-read-bps /dev/sda:1mb` |
| `--device-write-bps` | Limit device write rate | `--device-write-bps /dev/sda:1mb` |
| `--oom-kill-disable` | Disable OOM killer (use with -m) | `--oom-kill-disable` |

### Security

| Flag | Description | Example |
|------|-------------|---------|
| `--privileged` | Full host privileges (**NEVER use in production**) | `--privileged` |
| `--cap-add` | Add Linux capability | `--cap-add SYS_PTRACE` |
| `--cap-drop` | Drop Linux capability | `--cap-drop ALL` |
| `--security-opt` | Security options (AppArmor, SELinux, seccomp) | `--security-opt no-new-privileges=true` |
| `--device` | Add host device to container | `--device /dev/sda:/dev/xvdc` |
| `--gpus` | Add GPU devices | `--gpus all` |
| `--pid` | PID namespace (host or container:NAME) | `--pid=host` |
| `--ipc` | IPC namespace mode | `--ipc host` |
| `--userns` | User namespace mode | `--userns host` |
| `--cgroupns` | Cgroup namespace (host or private) | `--cgroupns private` |

### Health

| Flag | Description | Default | Example |
|------|-------------|---------|---------|
| `--health-cmd` | Health check command | none | `--health-cmd='curl -f http://localhost/'` |
| `--health-interval` | Time between checks | 30s | `--health-interval 30s` |
| `--health-timeout` | Max time for single check | 30s | `--health-timeout 10s` |
| `--health-retries` | Consecutive failures for unhealthy | 3 | `--health-retries 3` |
| `--health-start-period` | Grace period during init | 0s | `--health-start-period 40s` |
| `--health-start-interval` | Check interval during start period | 5s | `--health-start-interval 5s` |
| `--no-healthcheck` | Disable health check from image | — | `--no-healthcheck` |

### Restart

| Policy | Behavior | Example |
|--------|----------|---------|
| `no` | Never restart (default) | `--restart no` |
| `always` | Always restart, including on daemon startup | `--restart always` |
| `unless-stopped` | Like `always` but NOT after manual `docker stop` | `--restart unless-stopped` |
| `on-failure[:N]` | Restart on non-zero exit, optional max retries | `--restart on-failure:5` |

### Logging

| Flag | Description | Example |
|------|-------------|---------|
| `--log-driver` | Logging driver (json-file, syslog, journald, etc.) | `--log-driver json-file` |
| `--log-opt` | Log driver options (repeatable) | `--log-opt max-size=10m --log-opt max-file=3` |

### Pull & Platform

| Flag | Description | Example |
|------|-------------|---------|
| `--pull` | Pull policy: `missing` (default), `always`, `never` | `--pull always` |
| `--platform` | Target platform | `--platform linux/amd64` |

### Signals

| Flag | Description | Default | Example |
|------|-------------|---------|---------|
| `--stop-signal` | Signal to stop container | SIGTERM | `--stop-signal SIGKILL` |
| `--stop-timeout` | Seconds before force kill after stop signal | 10 | `--stop-timeout 30` |
| `--sig-proxy` | Proxy signals to the process | true | `--sig-proxy=false` |

---

## docker create

Syntax: `docker create [OPTIONS] IMAGE [COMMAND] [ARG...]`

Accepts ALL the same flags as `docker run`. Creates the container without starting it. Use `docker start` afterward.

```bash
docker create --name myapp -p 8080:80 nginx
# Returns container ID
docker start myapp
```

---

## docker start

Syntax: `docker start [OPTIONS] CONTAINER [CONTAINER...]`

| Flag | Description |
|------|-------------|
| `-a, --attach` | Attach STDOUT/STDERR and forward signals |
| `-i, --interactive` | Attach container's STDIN |
| `--detach-keys` | Override detach key sequence |

---

## docker stop

Syntax: `docker stop [OPTIONS] CONTAINER [CONTAINER...]`

Sends SIGTERM, waits for grace period, then sends SIGKILL.

| Flag | Description | Default |
|------|-------------|---------|
| `-t, --time` | Grace period in seconds before SIGKILL | 10 |
| `-s, --signal` | Signal to send instead of SIGTERM | SIGTERM |

---

## docker restart

Syntax: `docker restart [OPTIONS] CONTAINER [CONTAINER...]`

Equivalent to `docker stop` followed by `docker start`.

| Flag | Description | Default |
|------|-------------|---------|
| `-t, --time` | Grace period in seconds | 10 |
| `-s, --signal` | Signal to send | SIGTERM |

---

## docker kill

Syntax: `docker kill [OPTIONS] CONTAINER [CONTAINER...]`

Sends a signal immediately (no grace period).

| Flag | Description | Default |
|------|-------------|---------|
| `-s, --signal` | Signal to send | SIGKILL |

---

## docker rm

Syntax: `docker rm [OPTIONS] CONTAINER [CONTAINER...]`

| Flag | Description |
|------|-------------|
| `-f, --force` | Force remove running container (sends SIGKILL first) |
| `-v, --volumes` | Remove anonymous volumes attached to the container |
| `-l, --link` | Remove the specified link only |

---

## docker container prune

Syntax: `docker container prune [OPTIONS]`

Removes ALL stopped containers.

| Flag | Description |
|------|-------------|
| `-f, --force` | No confirmation prompt |
| `--filter` | Filter (e.g., `until=24h`, `label=temp`) |

---

## docker exec

Syntax: `docker exec [OPTIONS] CONTAINER COMMAND [ARG...]`

| Flag | Description |
|------|-------------|
| `-d, --detach` | Run in background |
| `-e, --env` | Set environment variables |
| `--env-file` | Load env vars from file |
| `-i, --interactive` | Keep STDIN open |
| `-t, --tty` | Allocate pseudo-TTY |
| `-u, --user` | Run as username or UID |
| `-w, --workdir` | Working directory inside container |
| `--privileged` | Extended privileges |

**ALWAYS** wrap chained commands in a shell:
```bash
# CORRECT
docker exec myapp sh -c "echo a && echo b"

# WRONG -- && is interpreted by host shell
docker exec myapp echo a && echo b
```

**Cannot** exec into a paused container -- unpause first.

---

## docker attach

Syntax: `docker attach [OPTIONS] CONTAINER`

Connects to the container's MAIN process (PID 1) STDIN/STDOUT/STDERR.

| Flag | Description | Default |
|------|-------------|---------|
| `--detach-keys` | Override detach key sequence | Ctrl+P, Ctrl+Q |
| `--no-stdin` | Do not attach STDIN | false |
| `--sig-proxy` | Proxy all signals to the process | true |

---

## docker logs

Syntax: `docker logs [OPTIONS] CONTAINER`

| Flag | Description | Default |
|------|-------------|---------|
| `-f, --follow` | Stream live output | — |
| `-n, --tail` | Number of lines from end | all |
| `-t, --timestamps` | Show RFC3339Nano timestamps | — |
| `--since` | Logs after timestamp or duration | — |
| `--until` | Logs before timestamp or duration | — |
| `--details` | Show extra metadata | — |

Timestamp formats: RFC 3339 (`2024-01-01T00:00:00Z`), Unix timestamps, Go duration strings (`30m`, `3h`).

---

## docker inspect

Syntax: `docker inspect [OPTIONS] NAME|ID [NAME|ID...]`

| Flag | Description |
|------|-------------|
| `--format, -f` | Go template format string |
| `--type` | Restrict to type: `container`, `image`, `network`, `volume` |
| `-s, --size` | Include size information |

---

## docker stats

Syntax: `docker stats [OPTIONS] [CONTAINER...]`

| Flag | Description |
|------|-------------|
| `--all, -a` | Show all containers (not just running) |
| `--no-stream` | Single snapshot instead of live stream |
| `--no-trunc` | Full container IDs |
| `--format` | Go template format |

Format placeholders: `{{.Name}}`, `{{.CPUPerc}}`, `{{.MemUsage}}`, `{{.MemPerc}}`, `{{.NetIO}}`, `{{.BlockIO}}`, `{{.PIDs}}`.

---

## docker top

Syntax: `docker top CONTAINER [ps OPTIONS]`

Displays running processes. Accepts standard `ps` options after the container name.

```bash
docker top myapp
docker top myapp aux
docker top myapp -o pid,user,%cpu,%mem,cmd
```

---

## docker events

Syntax: `docker events [OPTIONS]`

Real-time stream of Docker daemon events.

| Flag | Description |
|------|-------------|
| `--filter` | Filter by: `type`, `event`, `container`, `image`, `label`, `network`, `volume` |
| `--since` | Events after timestamp or duration |
| `--until` | Events before timestamp or duration |
| `--format` | Go template or `json` |

Event types: `container`, `image`, `volume`, `network`, `daemon`, `plugin`, `node`, `service`, `secret`, `config`.

---

## docker wait

Syntax: `docker wait CONTAINER [CONTAINER...]`

Blocks until the container stops, then prints the exit code.

```bash
EXIT_CODE=$(docker wait myapp)
```

---

## docker cp

Syntax: `docker cp [OPTIONS] SRC DEST`

Either SRC or DEST must be `CONTAINER:PATH`.

| Flag | Description |
|------|-------------|
| `-a, --archive` | Archive mode (preserves UID/GID) |
| `-L, --follow-link` | Follow symlinks in SRC |

```bash
# Container to host
docker cp myapp:/app/log.txt ./log.txt

# Host to container
docker cp ./config.yml myapp:/app/config.yml

# Archive mode
docker cp -a myapp:/app/data ./backup/
```

---

## docker diff

Syntax: `docker diff CONTAINER`

Shows filesystem changes relative to the image.

| Symbol | Meaning |
|--------|---------|
| `A` | File or directory added |
| `C` | File or directory changed |
| `D` | File or directory deleted |

---

## docker rename

Syntax: `docker rename CONTAINER NEW_NAME`

Works on running or stopped containers.

---

## docker update

Syntax: `docker update [OPTIONS] CONTAINER [CONTAINER...]`

Updates resource limits and restart policy on a running or stopped container.

| Flag | Description |
|------|-------------|
| `--memory, -m` | Memory limit |
| `--memory-reservation` | Memory soft limit |
| `--memory-swap` | Memory + swap limit |
| `--cpus` | Number of CPUs |
| `--cpu-shares, -c` | CPU shares |
| `--cpuset-cpus` | CPUs to pin to |
| `--pids-limit` | Max PIDs |
| `--restart` | Restart policy |
| `--blkio-weight` | Block I/O weight |

---

## docker pause / unpause

```bash
docker pause CONTAINER [CONTAINER...]
docker unpause CONTAINER [CONTAINER...]
```

Pauses all processes in a container using cgroup freezer. ALWAYS unpause before attempting `docker exec`.

---

## docker port

Syntax: `docker port CONTAINER [PRIVATE_PORT[/PROTO]]`

```bash
docker port myapp           # All mappings
docker port myapp 80/tcp    # Specific port
```

---

## docker ps / container ls

Syntax: `docker ps [OPTIONS]`

| Flag | Description |
|------|-------------|
| `-a, --all` | Show all containers (not just running) |
| `-f, --filter` | Filter output |
| `--format` | Go template format or `json` |
| `-n, --last` | Show n last created containers |
| `-l, --latest` | Show the latest created container |
| `--no-trunc` | Full output (no truncation) |
| `-q, --quiet` | Container IDs only |
| `-s, --size` | Display file sizes |

See SKILL.md for complete filter and format reference.
