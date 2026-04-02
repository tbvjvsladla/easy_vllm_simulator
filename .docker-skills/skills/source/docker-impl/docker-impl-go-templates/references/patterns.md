# Go Template Format Patterns Reference

> Complete catalog of 40+ ready-to-use `--format` patterns for Docker CLI commands.
> ALWAYS copy these patterns verbatim -- they are verified against Docker Engine 24+.

---

## docker inspect -- Container Patterns

### State & Lifecycle

```bash
# P-01: Container status (running, exited, paused, etc.)
docker inspect --format='{{.State.Status}}' CONTAINER

# P-02: Container PID on host
docker inspect --format='{{.State.Pid}}' CONTAINER

# P-03: Running boolean
docker inspect --format='{{.State.Running}}' CONTAINER

# P-04: Exit code
docker inspect --format='{{.State.ExitCode}}' CONTAINER

# P-05: Start time (RFC 3339)
docker inspect --format='{{.State.StartedAt}}' CONTAINER

# P-06: Finish time
docker inspect --format='{{.State.FinishedAt}}' CONTAINER

# P-07: Health check status (with nil guard)
docker inspect --format='{{if .State.Health}}{{.State.Health.Status}}{{else}}no-healthcheck{{end}}' CONTAINER

# P-08: OOM killed boolean
docker inspect --format='{{.State.OOMKilled}}' CONTAINER

# P-09: Restart count
docker inspect --format='{{.RestartCount}}' CONTAINER

# P-10: Restart policy
docker inspect --format='{{.HostConfig.RestartPolicy.Name}}' CONTAINER
```

### Network Information

```bash
# P-11: IP address (first network)
docker inspect --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' CONTAINER

# P-12: All networks with IPs
docker inspect --format='{{range $net, $conf := .NetworkSettings.Networks}}{{$net}}={{$conf.IPAddress}} {{end}}' CONTAINER

# P-13: MAC address
docker inspect --format='{{range .NetworkSettings.Networks}}{{.MacAddress}}{{end}}' CONTAINER

# P-14: Gateway
docker inspect --format='{{range .NetworkSettings.Networks}}{{.Gateway}}{{end}}' CONTAINER

# P-15: All port bindings
docker inspect --format='{{range $p, $conf := .NetworkSettings.Ports}}{{$p}} -> {{(index $conf 0).HostPort}}{{println}}{{end}}' CONTAINER

# P-16: Specific port (80/tcp) host mapping
docker inspect --format='{{(index (index .NetworkSettings.Ports "80/tcp") 0).HostPort}}' CONTAINER

# P-17: Network mode
docker inspect --format='{{.HostConfig.NetworkMode}}' CONTAINER

# P-18: DNS servers
docker inspect --format='{{json .HostConfig.Dns}}' CONTAINER
```

### Configuration

```bash
# P-19: Image name
docker inspect --format='{{.Config.Image}}' CONTAINER

# P-20: Entrypoint (JSON array)
docker inspect --format='{{json .Config.Entrypoint}}' CONTAINER

# P-21: Command (JSON array)
docker inspect --format='{{json .Config.Cmd}}' CONTAINER

# P-22: Working directory
docker inspect --format='{{.Config.WorkingDir}}' CONTAINER

# P-23: Hostname
docker inspect --format='{{.Config.Hostname}}' CONTAINER

# P-24: Environment variables (one per line)
docker inspect --format='{{range .Config.Env}}{{println .}}{{end}}' CONTAINER

# P-25: All labels (JSON)
docker inspect --format='{{json .Config.Labels}}' CONTAINER

# P-26: Specific label by key
docker inspect --format='{{index .Config.Labels "com.example.version"}}' CONTAINER

# P-27: Exposed ports
docker inspect --format='{{json .Config.ExposedPorts}}' CONTAINER

# P-28: User
docker inspect --format='{{.Config.User}}' CONTAINER
```

### Mounts & Storage

```bash
# P-29: All mounts (JSON)
docker inspect --format='{{json .Mounts}}' CONTAINER

# P-30: Mount sources and destinations
docker inspect --format='{{range .Mounts}}{{.Source}} -> {{.Destination}}{{println}}{{end}}' CONTAINER

# P-31: Mount types and names
docker inspect --format='{{range .Mounts}}{{.Type}}: {{.Name}} @ {{.Destination}}{{println}}{{end}}' CONTAINER

# P-32: Log file path
docker inspect --format='{{.LogPath}}' CONTAINER

# P-33: Root filesystem size (use with docker inspect -s)
docker inspect --size --format='{{.SizeRootFs}}' CONTAINER

# P-34: Writable layer size (use with docker inspect -s)
docker inspect --size --format='{{.SizeRw}}' CONTAINER
```

### Resource Limits

```bash
# P-35: Memory limit (bytes, 0 = unlimited)
docker inspect --format='{{.HostConfig.Memory}}' CONTAINER

# P-36: CPU shares
docker inspect --format='{{.HostConfig.CpuShares}}' CONTAINER

# P-37: CPU quota and period (NanoCpus)
docker inspect --format='{{.HostConfig.NanoCpus}}' CONTAINER

# P-38: PID limit
docker inspect --format='{{.HostConfig.PidsLimit}}' CONTAINER
```

---

## docker ps Patterns

```bash
# P-39: Compact status table
docker ps --format "table {{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}"

# P-40: Names only (for scripting)
docker ps --format "{{.Names}}"

# P-41: IDs only (equivalent to -q)
docker ps --format "{{.ID}}"

# P-42: Name with state and uptime
docker ps --format "table {{.Names}}\t{{.State}}\t{{.RunningFor}}"

# P-43: With specific label column
docker ps --format "table {{.Names}}\t{{.Label \"app\"}}\t{{.Status}}"

# P-44: With size info (requires -s flag)
docker ps -s --format "table {{.Names}}\t{{.Size}}"

# P-45: With network and mount info
docker ps --format "table {{.Names}}\t{{.Networks}}\t{{.Mounts}}"

# P-46: JSON output (one object per line, native)
docker ps --format json

# P-47: All containers with exit info
docker ps -a --format "table {{.Names}}\t{{.Status}}\t{{.Command}}"
```

---

## docker images Patterns

```bash
# P-48: Compact image table
docker images --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}"

# P-49: Repository:tag pairs (for scripting)
docker images --format "{{.Repository}}:{{.Tag}}"

# P-50: With creation time
docker images --format "table {{.Repository}}\t{{.Tag}}\t{{.CreatedSince}}\t{{.Size}}"

# P-51: With digest
docker images --digests --format "table {{.Repository}}\t{{.Tag}}\t{{.Digest}}"

# P-52: IDs only (equivalent to -q)
docker images --format "{{.ID}}"

# P-53: JSON output
docker images --format json
```

---

## docker stats Patterns

```bash
# P-54: CPU and memory overview
docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.MemPerc}}"

# P-55: Full resource view
docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}\t{{.BlockIO}}\t{{.PIDs}}"

# P-56: Names and CPU only
docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}"

# P-57: Container ID with stats
docker stats --no-stream --format "table {{.Container}}\t{{.Name}}\t{{.CPUPerc}}\t{{.MemPerc}}"
```

---

## docker network Patterns

```bash
# P-58: Network listing with driver
docker network ls --format "table {{.Name}}\t{{.Driver}}\t{{.Scope}}"

# P-59: Network IDs only
docker network ls --format "{{.ID}}"

# P-60: Network names only
docker network ls --format "{{.Name}}"

# P-61: Containers on a specific network
docker network inspect --format='{{range .Containers}}{{.Name}} ({{.IPv4Address}}){{println}}{{end}}' NETWORK

# P-62: Network subnet
docker network inspect --format='{{range .IPAM.Config}}{{.Subnet}}{{end}}' NETWORK

# P-63: Network gateway
docker network inspect --format='{{range .IPAM.Config}}{{.Gateway}}{{end}}' NETWORK
```

---

## docker volume Patterns

```bash
# P-64: Volume listing with driver
docker volume ls --format "table {{.Name}}\t{{.Driver}}\t{{.Mountpoint}}"

# P-65: Volume names only
docker volume ls --format "{{.Name}}"

# P-66: Volume mount point
docker volume inspect --format='{{.Mountpoint}}' VOLUME

# P-67: Volume labels (JSON)
docker volume inspect --format='{{json .Labels}}' VOLUME

# P-68: Volume creation time
docker volume inspect --format='{{.CreatedAt}}' VOLUME
```

---

## docker system Patterns

```bash
# P-69: Disk usage table
docker system df --format "table {{.Type}}\t{{.TotalCount}}\t{{.Size}}\t{{.Reclaimable}}"

# P-70: Server version
docker info --format '{{.ServerVersion}}'

# P-71: Storage driver
docker info --format '{{.Driver}}'

# P-72: Operating system
docker info --format '{{.OperatingSystem}}'

# P-73: Total memory
docker info --format '{{.MemTotal}}'

# P-74: Number of containers
docker info --format 'Running: {{.ContainersRunning}}, Stopped: {{.ContainersStopped}}'

# P-75: Plugins (JSON)
docker info --format '{{json .Plugins}}'
```

---

## docker events Patterns

```bash
# P-76: Custom event format
docker events --format '{{.Time}} {{.Type}} {{.Action}} {{.Actor.Attributes.name}}'

# P-77: JSON events
docker events --format '{{json .}}'
```

---

## Pattern Index by Use Case

| Use Case | Pattern IDs |
|----------|-------------|
| Container health monitoring | P-01, P-07, P-08, P-09 |
| Network debugging | P-11 through P-18, P-61 through P-63 |
| Security auditing | P-10, P-19, P-25, P-28, P-35 through P-38 |
| Scripting / automation | P-40, P-41, P-46, P-49, P-52, P-59, P-65 |
| Resource monitoring | P-33 through P-38, P-54 through P-57, P-69 |
| Configuration inspection | P-19 through P-28 |
| Storage / volume management | P-29 through P-34, P-64 through P-68 |
| System overview | P-69 through P-75 |
