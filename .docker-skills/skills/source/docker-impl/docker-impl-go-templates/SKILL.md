---
name: docker-impl-go-templates
description: >
  Use when formatting Docker CLI output with --format flags or extracting
  specific fields from docker inspect. Prevents broken template syntax
  from missing braces, incorrect field paths, and unquoted json calls.
  Covers Go template syntax, {{ .Field }}, json, table, range, if/else,
  and ready-to-use format strings for inspect, ps, images, and stats.
  Keywords: docker inspect, --format, Go template, json, table, range,
  docker ps format, format Docker output, get container IP, extract info,
  custom output.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-impl-go-templates

## Quick Reference

### Go Template Syntax Basics

| Syntax | Purpose | Example |
|--------|---------|---------|
| `{{ .Field }}` | Access top-level field | `{{ .State.Status }}` |
| `{{ .Field.SubField }}` | Access nested field | `{{ .NetworkSettings.IPAddress }}` |
| `{{ json . }}` | Full JSON output | `docker inspect --format='{{json .}}'` |
| `{{ json .Field }}` | JSON for specific field | `docker inspect --format='{{json .Config}}'` |
| `table` | Table with headers | `docker ps --format "table {{.Names}}\t{{.Status}}"` |
| `\t` | Tab separator in tables | Used between columns |
| `{{ println }}` | Newline in output | Used inside range loops |

### Template Functions

| Function | Purpose | Example |
|----------|---------|---------|
| `json` | JSON-encode a value | `{{ json .Config.Env }}` |
| `join` | Join string slice with separator | `{{ join .Config.Cmd " " }}` |
| `upper` | Uppercase string | `{{ upper .State.Status }}` |
| `lower` | Lowercase string | `{{ lower .State.Status }}` |
| `title` | Title-case string | `{{ title .State.Status }}` |
| `split` | Split string by separator | `{{ split .Image ":" }}` |
| `println` | Print with newline | `{{ println .Name }}` |
| `index` | Access array/map element | `{{ index .Config.Env 0 }}` |
| `len` | Get length of array/map | `{{ len .Config.Env }}` |

### Critical Warnings

**NEVER** use double quotes around the entire `--format` value when it contains Go template double quotes inside -- ALWAYS use single quotes for the outer wrapper on Linux/macOS. On Windows PowerShell, use escaped double quotes or backticks.

**NEVER** omit `{{ end }}` when using `{{ if }}` or `{{ range }}` -- every conditional and loop block MUST be closed with `{{ end }}`.

**NEVER** assume a field exists without checking -- use `{{ if .Field }}` to guard against nil values that cause template execution errors.

**ALWAYS** use `{{ json .Field }}` instead of trying to manually format complex nested objects -- JSON output is reliable and pipeable to `jq`.

**ALWAYS** use `table` prefix with `\t` separators for readable multi-column output -- omitting `table` removes column headers.

---

## Conditionals

### if / else / end

```bash
# Simple conditional
docker inspect --format='{{if .State.Running}}UP{{else}}DOWN{{end}}' myapp

# Check for empty value
docker inspect --format='{{if .State.Health}}{{.State.Health.Status}}{{else}}no healthcheck{{end}}' myapp

# Nested conditional
docker inspect --format='{{if eq .State.Status "running"}}HEALTHY{{else if eq .State.Status "exited"}}STOPPED{{else}}UNKNOWN{{end}}' myapp
```

### Comparison Functions

| Function | Purpose | Example |
|----------|---------|---------|
| `eq` | Equal | `{{ if eq .State.Status "running" }}` |
| `ne` | Not equal | `{{ if ne .State.ExitCode 0 }}` |
| `lt` | Less than | `{{ if lt .State.ExitCode 1 }}` |
| `le` | Less or equal | `{{ if le .State.Pid 0 }}` |
| `gt` | Greater than | `{{ if gt .State.ExitCode 0 }}` |
| `ge` | Greater or equal | `{{ if ge .SizeRw 1000000 }}` |
| `not` | Boolean negation | `{{ if not .State.Running }}` |
| `and` | Logical AND | `{{ if and .State.Running .State.Health }}` |
| `or` | Logical OR | `{{ if or .State.Running .State.Paused }}` |

---

## Range Loops

### Iterating Over Arrays

```bash
# Environment variables (one per line)
docker inspect --format='{{range .Config.Env}}{{println .}}{{end}}' myapp

# Mounts with source and destination
docker inspect --format='{{range .Mounts}}{{.Source}} -> {{.Destination}}{{println}}{{end}}' myapp
```

### Iterating Over Maps

```bash
# Network names and IPs
docker inspect --format='{{range $net, $conf := .NetworkSettings.Networks}}{{$net}}: {{$conf.IPAddress}}{{println}}{{end}}' myapp

# Port bindings
docker inspect --format='{{range $port, $bindings := .NetworkSettings.Ports}}{{$port}} -> {{(index $bindings 0).HostPort}}{{println}}{{end}}' myapp

# All labels
docker inspect --format='{{range $k, $v := .Config.Labels}}{{$k}}={{$v}}{{println}}{{end}}' myapp
```

### Index Function

```bash
# First element of an array
docker inspect --format='{{index .Config.Cmd 0}}' myapp

# Specific port binding
docker inspect --format='{{(index (index .NetworkSettings.Ports "80/tcp") 0).HostPort}}' myapp

# Label by key (with dot in name)
docker inspect --format='{{index .Config.Labels "com.example.version"}}' myapp
```

---

## Format Patterns by Command

### docker inspect -- Container State (6 patterns)

```bash
# 1. Container status
docker inspect --format='{{.State.Status}}' CONTAINER

# 2. Container PID
docker inspect --format='{{.State.Pid}}' CONTAINER

# 3. Container start time
docker inspect --format='{{.State.StartedAt}}' CONTAINER

# 4. Container exit code
docker inspect --format='{{.State.ExitCode}}' CONTAINER

# 5. Running boolean
docker inspect --format='{{.State.Running}}' CONTAINER

# 6. Health check status
docker inspect --format='{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' CONTAINER
```

### docker inspect -- Network Info (6 patterns)

```bash
# 7. IP address (first network)
docker inspect --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' CONTAINER

# 8. MAC address
docker inspect --format='{{range .NetworkSettings.Networks}}{{.MacAddress}}{{end}}' CONTAINER

# 9. All networks with IPs
docker inspect --format='{{range $k,$v := .NetworkSettings.Networks}}{{$k}}={{$v.IPAddress}} {{end}}' CONTAINER

# 10. Gateway
docker inspect --format='{{range .NetworkSettings.Networks}}{{.Gateway}}{{end}}' CONTAINER

# 11. Port bindings (all)
docker inspect --format='{{range $p,$conf := .NetworkSettings.Ports}}{{$p}}->{{(index $conf 0).HostPort}} {{end}}' CONTAINER

# 12. Specific port mapping
docker inspect --format='{{(index (index .NetworkSettings.Ports "80/tcp") 0).HostPort}}' CONTAINER
```

### docker inspect -- Configuration (6 patterns)

```bash
# 13. Image name
docker inspect --format='{{.Config.Image}}' CONTAINER

# 14. Entrypoint
docker inspect --format='{{json .Config.Entrypoint}}' CONTAINER

# 15. Command
docker inspect --format='{{json .Config.Cmd}}' CONTAINER

# 16. Environment variables
docker inspect --format='{{range .Config.Env}}{{println .}}{{end}}' CONTAINER

# 17. All labels as JSON
docker inspect --format='{{json .Config.Labels}}' CONTAINER

# 18. Specific label
docker inspect --format='{{index .Config.Labels "com.example.version"}}' CONTAINER
```

### docker inspect -- Mounts & Size (4 patterns)

```bash
# 19. All mounts as JSON
docker inspect --format='{{json .Mounts}}' CONTAINER

# 20. Mount sources and destinations
docker inspect --format='{{range .Mounts}}{{.Source}} -> {{.Destination}}{{println}}{{end}}' CONTAINER

# 21. Log file path
docker inspect --format='{{.LogPath}}' CONTAINER

# 22. Root filesystem size (requires -s flag)
docker inspect --size --format='{{.SizeRootFs}}' CONTAINER
```

### docker ps (4 patterns)

```bash
# 23. Compact table
docker ps --format "table {{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}"

# 24. Names only
docker ps --format "{{.Names}}"

# 25. With specific label
docker ps --format "table {{.Names}}\t{{.Label \"app\"}}\t{{.Status}}"

# 26. JSON output (one object per line)
docker ps --format json
```

### docker images (3 patterns)

```bash
# 27. Compact image list
docker images --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}"

# 28. Repository:tag pairs
docker images --format "{{.Repository}}:{{.Tag}}"

# 29. With digest
docker images --digests --format "table {{.Repository}}\t{{.Tag}}\t{{.Digest}}"
```

### docker stats / network / volume / system (5 patterns)

```bash
# 30. Stats with custom columns
docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}"

# 31. Network listing
docker network ls --format "table {{.Name}}\t{{.Driver}}\t{{.Scope}}"

# 32. Volume listing
docker volume ls --format "table {{.Name}}\t{{.Driver}}\t{{.Mountpoint}}"

# 33. System disk usage
docker system df --format "table {{.Type}}\t{{.TotalCount}}\t{{.Size}}\t{{.Reclaimable}}"

# 34. Docker info field
docker info --format '{{.ServerVersion}}'
```

---

## Scripting Patterns

### Extract Values for Shell Scripts

```bash
# Get container ID by name
CID=$(docker ps -qf "name=myapp")

# Get IP address into variable
IP=$(docker inspect --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' myapp)

# Get host port for container port 80
PORT=$(docker inspect --format='{{(index (index .NetworkSettings.Ports "80/tcp") 0).HostPort}}' myapp)

# Get all running container names
docker ps --format "{{.Names}}" | while read name; do
  echo "Container: $name"
done

# Get exit codes for all stopped containers
docker ps -a --filter status=exited --format "{{.Names}}: exit {{.Status}}"
```

### Bulk Operations with Format Output

```bash
# Stop all containers on a specific network
docker network inspect --format='{{range .Containers}}{{.Name}} {{end}}' mynet | xargs docker stop

# Remove all images from a specific repository
docker images myrepo --format "{{.ID}}" | xargs docker rmi

# Get IPs of all running containers
docker ps -q | xargs -I {} docker inspect --format='{{.Name}}: {{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' {}
```

---

## Decision Tree: Choosing Output Format

```
Need Docker command output?
├── Need full object details?
│   └── Use: docker inspect --format='{{json .}}' | jq
├── Need specific single field?
│   └── Use: --format='{{.Field.SubField}}'
├── Need readable multi-column table?
│   └── Use: --format "table {{.Col1}}\t{{.Col2}}"
├── Need machine-parseable output?
│   └── Use: --format json  OR  --format='{{json .Field}}'
├── Need to iterate nested data?
│   └── Use: --format='{{range .Items}}{{.Field}}{{println}}{{end}}'
└── Need conditional output?
    └── Use: --format='{{if .Cond}}yes{{else}}no{{end}}'
```

---

## Reference Links

- [references/patterns.md](references/patterns.md) -- 30+ format patterns organized by Docker command
- [references/examples.md](references/examples.md) -- Complex template examples with conditionals, range loops, and scripting
- [references/anti-patterns.md](references/anti-patterns.md) -- Common Go template mistakes and how to fix them

### Official Sources

- https://docs.docker.com/reference/cli/docker/inspect/
- https://docs.docker.com/reference/cli/docker/container/ls/
- https://docs.docker.com/reference/cli/docker/image/ls/
- https://pkg.go.dev/text/template
