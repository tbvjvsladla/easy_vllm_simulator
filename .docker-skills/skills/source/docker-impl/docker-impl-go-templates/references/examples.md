# Go Template Complex Examples

> Advanced Go template patterns for Docker CLI. Each example is verified against Docker Engine 24+.
> ALWAYS test complex templates on a single container before using in scripts.

---

## Conditional Output Examples

### Container Status Dashboard

```bash
# Color-coded status (for terminal output)
docker inspect --format='{{.Name}}: {{if eq .State.Status "running"}}RUNNING{{else if eq .State.Status "exited"}}EXITED (code {{.State.ExitCode}}){{else if eq .State.Status "paused"}}PAUSED{{else}}{{.State.Status}}{{end}}' CONTAINER
```

### Health Check with Fallback

```bash
# Show health status or "no healthcheck configured"
docker inspect --format='{{if .State.Health}}Health: {{.State.Health.Status}} ({{len .State.Health.Log}} checks logged){{else}}Health: no healthcheck configured{{end}}' CONTAINER
```

### Conditional Port Display

```bash
# Show port mappings only if they exist
docker inspect --format='{{if .NetworkSettings.Ports}}Ports: {{range $p, $conf := .NetworkSettings.Ports}}{{$p}}->{{if $conf}}{{(index $conf 0).HostPort}}{{else}}unmapped{{end}} {{end}}{{else}}No ports exposed{{end}}' CONTAINER
```

### Memory Limit Check

```bash
# Show memory limit or "unlimited"
docker inspect --format='Memory: {{if eq .HostConfig.Memory 0}}unlimited{{else}}{{.HostConfig.Memory}} bytes{{end}}' CONTAINER
```

### Non-Zero Exit Code Alert

```bash
# Alert on non-zero exit codes
docker inspect --format='{{if and (eq .State.Status "exited") (ne .State.ExitCode 0)}}ALERT: {{.Name}} exited with code {{.State.ExitCode}}{{end}}' CONTAINER
```

---

## Range Loop Examples

### Environment Variable Extraction

```bash
# All environment variables, one per line
docker inspect --format='{{range .Config.Env}}{{println .}}{{end}}' CONTAINER

# Filter-like: show env vars as KEY=VALUE (already in that format)
docker inspect --format='Environment:{{range .Config.Env}}
  {{.}}{{end}}' CONTAINER
```

### Multi-Network Container Report

```bash
# Full network report for a container
docker inspect --format='Networks:{{range $name, $conf := .NetworkSettings.Networks}}
  {{$name}}:
    IP:      {{$conf.IPAddress}}
    Gateway: {{$conf.Gateway}}
    MAC:     {{$conf.MacAddress}}{{end}}' CONTAINER
```

### Mount Summary

```bash
# All mounts with type, source, destination, and mode
docker inspect --format='Mounts:{{range .Mounts}}
  [{{.Type}}] {{.Source}} -> {{.Destination}} ({{if .RW}}rw{{else}}ro{{end}}){{end}}' CONTAINER
```

### Label Enumeration

```bash
# All labels formatted as key=value
docker inspect --format='Labels:{{range $k, $v := .Config.Labels}}
  {{$k}} = {{$v}}{{end}}' CONTAINER
```

### Container List on a Network

```bash
# All containers on a network with their IPs
docker network inspect --format='Containers on {{.Name}}:{{range .Containers}}
  {{.Name}} ({{.IPv4Address}}){{end}}' NETWORK
```

### IPAM Configuration

```bash
# All subnet/gateway pairs for a network
docker network inspect --format='IPAM:{{range .IPAM.Config}}
  Subnet:  {{.Subnet}}
  Gateway: {{.Gateway}}{{end}}' NETWORK
```

---

## Nested Map Access with index

### Access Label with Dots in Key Name

```bash
# Labels with dots MUST use index function
docker inspect --format='{{index .Config.Labels "com.docker.compose.project"}}' CONTAINER
docker inspect --format='{{index .Config.Labels "org.opencontainers.image.version"}}' CONTAINER
```

### Access Specific Port Binding

```bash
# Get host port for a specific container port
docker inspect --format='{{(index (index .NetworkSettings.Ports "8080/tcp") 0).HostPort}}' CONTAINER

# With nil guard (port may not be mapped)
docker inspect --format='{{if index .NetworkSettings.Ports "80/tcp"}}{{(index (index .NetworkSettings.Ports "80/tcp") 0).HostPort}}{{else}}not mapped{{end}}' CONTAINER
```

### Access Specific Network Configuration

```bash
# Get IP for a specific network by name
docker inspect --format='{{(index .NetworkSettings.Networks "bridge").IPAddress}}' CONTAINER

# Get IP for a named network
docker inspect --format='{{(index .NetworkSettings.Networks "mynet").IPAddress}}' CONTAINER
```

---

## Scripting Integration Examples

### Container IP Lookup Script

```bash
#!/bin/bash
# Get IP addresses for all running containers
for id in $(docker ps -q); do
  name=$(docker inspect --format='{{.Name}}' "$id" | sed 's/^\///')
  ip=$(docker inspect --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$id")
  echo "$name: $ip"
done
```

### Health Monitoring Script

```bash
#!/bin/bash
# Check health of all containers with healthchecks
docker ps --format '{{.Names}}' | while read name; do
  health=$(docker inspect --format='{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "$name")
  if [ "$health" = "unhealthy" ]; then
    echo "ALERT: $name is unhealthy"
    docker inspect --format='{{json .State.Health}}' "$name" | jq '.Log[-1]'
  fi
done
```

### Port Mapping Report

```bash
#!/bin/bash
# Report all port mappings across running containers
docker ps --format '{{.Names}}' | while read name; do
  ports=$(docker inspect --format='{{range $p, $conf := .NetworkSettings.Ports}}{{if $conf}}{{$p}}->{{(index $conf 0).HostPort}} {{end}}{{end}}' "$name")
  if [ -n "$ports" ]; then
    echo "$name: $ports"
  fi
done
```

### Resource Limit Audit

```bash
#!/bin/bash
# Audit containers without memory limits
docker ps -q | while read id; do
  mem=$(docker inspect --format='{{.HostConfig.Memory}}' "$id")
  name=$(docker inspect --format='{{.Name}}' "$id" | sed 's/^\///')
  if [ "$mem" = "0" ]; then
    echo "WARNING: $name has no memory limit"
  else
    echo "OK: $name limited to $((mem / 1024 / 1024))MB"
  fi
done
```

### Cleanup Script Using Format Output

```bash
#!/bin/bash
# Remove containers that exited with non-zero status
docker ps -a --filter status=exited --format '{{.ID}} {{.Names}} {{.Status}}' | while read id name status; do
  exitcode=$(docker inspect --format='{{.State.ExitCode}}' "$id")
  if [ "$exitcode" -ne 0 ]; then
    echo "Removing $name (exit code: $exitcode)"
    docker rm "$id"
  fi
done
```

---

## Combined Format with jq

### Pretty-Print Specific Sections

```bash
# Pretty-print network settings
docker inspect --format='{{json .NetworkSettings}}' CONTAINER | jq .

# Pretty-print environment as object
docker inspect --format='{{json .Config.Env}}' CONTAINER | jq '.[] | split("=") | {(.[0]): .[1]}'

# Pretty-print mounts
docker inspect --format='{{json .Mounts}}' CONTAINER | jq '.[] | {type, source: .Source, dest: .Destination, rw: .RW}'

# Pretty-print labels
docker inspect --format='{{json .Config.Labels}}' CONTAINER | jq .
```

### Cross-Container Comparison

```bash
# Compare images across all running containers
docker ps --format json | jq -r '[.Names, .Image] | @tsv' | sort -k2

# Find containers using the same image
docker ps --format json | jq -s 'group_by(.Image) | .[] | select(length > 1) | {image: .[0].Image, containers: [.[].Names]}'
```

---

## Table Format Best Practices

### Custom Table with Headers

```bash
# table keyword ALWAYS adds headers automatically
docker ps --format "table {{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}"

# Output:
# NAMES    IMAGE    STATUS          PORTS
# web      nginx    Up 2 hours      0.0.0.0:80->80/tcp
# db       postgres Up 2 hours      5432/tcp
```

### Without Headers (for Scripting)

```bash
# Omit "table" prefix to get raw values without headers
docker ps --format "{{.Names}}\t{{.Image}}\t{{.Status}}"

# Output:
# web    nginx    Up 2 hours
# db     postgres Up 2 hours
```

### Multi-Line Per Entry

```bash
# Use println for multi-line output per container
docker ps --format "Container: {{.Names}}\n  Image: {{.Image}}\n  Status: {{.Status}}\n"
```

---

## Windows PowerShell Considerations

```powershell
# On PowerShell, use double quotes with escaped inner quotes
docker inspect --format="{{.State.Status}}" myapp

# For complex templates, use single quotes (works in PowerShell 7+)
docker inspect --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' myapp

# Alternative: use backtick escaping
docker inspect --format="{{.State.Status}}" myapp
```

**ALWAYS** prefer single-quoted `--format='...'` on Linux/macOS. On Windows CMD, use double quotes. On PowerShell, test both; single quotes work in most cases.
