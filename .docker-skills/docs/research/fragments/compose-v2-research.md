# Docker Compose v2 — Comprehensive Research

> **Source**: Official Docker Documentation (docs.docker.com)
> **Date**: 2026-03-19
> **Scope**: Docker Compose v2 specification, CLI, and patterns
> **Verification**: All content fetched via WebFetch from official docs

---

## 1. Compose File Structure

**Source**: https://docs.docker.com/compose/compose-file/

### Top-Level Elements

The Compose Specification is the current recommended format. Legacy versions 2.x and 3.x were merged into the unified Compose Specification, implemented in Docker Compose CLI v1.27.0+ (Compose v2).

Top-level elements:
- `name` — Project name
- `services` — **Required**. Map of service definitions
- `networks` — Network definitions
- `volumes` — Volume definitions
- `configs` — Configuration data definitions
- `secrets` — Sensitive data definitions
- `include` — Include other Compose files

The `version` field is **deprecated** and ignored by modern Compose. It exists only for backward compatibility.

### File Naming

Compose searches for files in this order:
1. `compose.yaml` (preferred)
2. `compose.yml`
3. `docker-compose.yaml`
4. `docker-compose.yml`

### Schema and Specification

The full Compose Specification is maintained at https://github.com/compose-spec/compose-spec. VS Code users can use the Docker DX extension for linting, code navigation, and vulnerability scanning.

### Remote Compose Files

Compose supports loading files from remote sources:
- OCI Registry: `docker compose -f oci://registry.example.com/project:latest up`
- Git Repository (HTTPS): `docker compose -f https://github.com/user/repo.git up`
- Git Repository (SSH): `docker compose -f git@github.com:user/repo.git up`
- Git branches/tags/commits: `@main`, `@v1.0.0`, `@abc123`
- Git subdirectories: `#main:path/to/compose.yaml`

---

## 2. Service Configuration (COMPLETE)

**Source**: https://docs.docker.com/compose/compose-file/05-services/

Services are the fundamental building blocks. A service represents a deployable unit backed by container definitions. The `services` top-level element is a map where each key is a service name.

### 2.1 Image

Specifies the container image in Open Container Specification format:

```yaml
services:
  web:
    image: redis
    # or
    image: redis:5
    # or with digest
    image: redis@sha256:0ed5d5928d4737458944eb604cc8509e245c3e19d02ad83935398bc4b991aac7
    # or from private registry
    image: my_private.registry:5000/redis
```

### 2.2 Build

**Source**: https://docs.docker.com/compose/compose-file/build/

Defines how to create Docker images from source code. Can be a string (context path) or object with granular control.

#### Context and Dockerfile

```yaml
build:
  context: ./dir
  dockerfile: custom.Dockerfile
```

```yaml
build:
  context: .
  dockerfile_inline: |
    FROM baseimage
    RUN some command
```

- `context`: Directory or Git repository containing the Dockerfile. Relative paths resolve to Compose file's directory.
- `dockerfile`: Alternate Dockerfile location, resolved relative to context. Cannot coexist with `dockerfile_inline`.
- `dockerfile_inline`: Embeds Dockerfile content directly (mutually exclusive with `dockerfile`).

#### Build Arguments

```yaml
build:
  args:
    GIT_COMMIT: cdc3b19
# or list syntax:
  args:
    - GIT_COMMIT=cdc3b19
```

Omitted values require runtime input.

#### Target (Multi-Stage Builds)

```yaml
build:
  target: prod
```

#### Cache Configuration

```yaml
build:
  cache_from:
    - alpine:latest
    - type=local,src=path/to/cache
    - type=gha
  cache_to:
    - user/app:cache
    - type=local,dest=path/to/cache
  no_cache: false  # Set true to disable builder cache
```

#### Build Secrets

Short syntax:
```yaml
services:
  frontend:
    build:
      secrets:
        - server-certificate
secrets:
  server-certificate:
    file: ./server.cert
```

Long syntax:
```yaml
build:
  secrets:
    - source: server-certificate
      target: cert
      uid: "103"
      gid: "103"
      mode: 0440
```

#### SSH Authentication

```yaml
build:
  ssh:
    - default
    - myproject=~/.ssh/myproject.pem
```

Dockerfile usage: `RUN --mount=type=ssh,id=myproject git clone ...`

#### Platforms

```yaml
build:
  platforms:
    - "linux/amd64"
    - "linux/arm64"
```

#### Additional Contexts

```yaml
build:
  additional_contexts:
    resources: /path/to/resources
    app: docker-image://my-app:latest
    source: https://github.com/myuser/project.git
```

Can reference services: `additional_contexts: base: service:base`

#### Network, SHM, Ulimits

```yaml
build:
  network: host   # or "none"
  shm_size: "2gb"
  ulimits:
    nproc: 65535
    nofile:
      soft: 20000
      hard: 40000
```

#### Extra Hosts in Build

```yaml
build:
  extra_hosts:
    - "somehost=162.242.195.82"
```

#### Labels and Tags

```yaml
build:
  labels:
    com.example.description: "Accounting webapp"
  tags:
    - "myimage:mytag"
    - "registry/username/myrepos:my-other-tag"
```

#### Privileged Build

```yaml
build:
  privileged: true
```

#### Provenance and SBOM

```yaml
build:
  provenance: true   # or mode=max
  sbom: true          # or generator=docker/scout-sbom-indexer:latest
```

#### Entitlements

```yaml
build:
  entitlements:
    - network.host
    - security.insecure
```

#### Pull and Image Publishing

When both `build` and `image` exist, `pull_policy` determines precedence. Without `pull_policy`, Compose attempts pulling before building. Images need an `image` attribute for registry publishing.

### 2.3 Command and Entrypoint

```yaml
# String form
command: bundle exec thin -p 3000
# Shell form with variable expansion
command: /bin/sh -c 'echo "hello $$HOSTNAME"'
# Exec form (list)
command: ["php", "-d", "zend_extension=/path"]
```

Setting to `null` uses image default; `[]` or `''` clears the command.

```yaml
entrypoint: /code/entrypoint.sh
# or exec form
entrypoint: ["php", "-d", "memory_limit=-1", "vendor/bin/phpunit"]
```

### 2.4 Ports

#### Short Syntax (`[HOST:]CONTAINER[/PROTOCOL]`)

```yaml
ports:
  - "3000"                    # Container port only (random host port)
  - "8000:8000"               # HOST:CONTAINER
  - "9090-9091:8080-8081"     # Port range
  - "127.0.0.1:8001:8001"    # Bind to specific interface
  - "6060:6060/udp"           # UDP protocol
  - "[::1]:6001:6001"         # IPv6
```

#### Long Syntax

```yaml
ports:
  - name: web
    target: 80
    published: "8080"
    host_ip: 127.0.0.1
    protocol: tcp
    app_protocol: http
    mode: host
```

### 2.5 Expose

Internal port exposure without host publishing:

```yaml
expose:
  - "3000"
  - "8080-8085/tcp"
```

### 2.6 Environment Variables

#### Map Syntax

```yaml
environment:
  RACK_ENV: development
  SHOW: "true"
```

#### List Syntax

```yaml
environment:
  - RACK_ENV=development
  - USER_INPUT    # Pass-through from host shell
```

### 2.7 env_file

```yaml
# Single file
env_file: .env

# Multiple files
env_file:
  - ./default.env
  - ./override.env

# With options (Compose 2.24.0+)
env_file:
  - path: ./default.env
    required: false      # Don't error if missing
    format: raw          # Literal values, no interpolation (Compose 2.30.0+)
```

#### .env File Parsing Rules
- Lines format: `VAR[=[VAL]]`
- Comments: lines starting with `#`
- Values support interpolation unless single-quoted
- Escape sequences in double-quoted values: `\n`, `\r`, `\t`, `\\`
- Later files override earlier files

### 2.8 Volumes

#### Short Syntax (`VOLUME:CONTAINER_PATH[:ACCESS_MODE]`)

```yaml
volumes:
  - /host/path:/container/path
  - volume-name:/data
  - /host/path:/container/path:ro
  - /host/path:/container/path:rw
```

#### Long Syntax

```yaml
volumes:
  - type: volume
    source: db-data
    target: /data
    volume:
      nocopy: true
      subpath: sub
  - type: bind
    source: /var/run/postgres.sock
    target: /var/run/postgres.sock
    bind:
      propagation: rprivate
      create_host_path: true
  - type: tmpfs
    target: /temp
    tmpfs:
      size: 1G
      mode: 0755
```

#### tmpfs

```yaml
tmpfs:
  - /data:mode=755,uid=1009,gid=1009
  - /run
```

#### volumes_from

```yaml
volumes_from:
  - service_name
  - service_name:ro
  - container:container_name:rw
```

### 2.9 Networks (Service-Level)

```yaml
services:
  app:
    networks:
      - network1
      - network2
```

Services connect to `default` network unless explicitly configured.

#### Network Aliases

```yaml
networks:
  backend:
    aliases:
      - db-alias
```

#### Static IP

```yaml
networks:
  front-tier:
    ipv4_address: 172.16.238.10
    ipv6_address: 2001:3984:3989::10
```

#### Additional Network Options

- `interface_name`: Specifies network interface name
- `link_local_ips`: Link-local IP assignments
- `mac_address`: MAC address for the network connection
- `driver_opts`: Driver-specific options
- `gw_priority`: Default gateway selection (higher value wins)
- `priority`: Connection order

### 2.10 network_mode

```yaml
network_mode: "host"
network_mode: "none"
network_mode: "service:other_service"
network_mode: "container:container_id"
```

### 2.11 depends_on

#### Short Syntax

```yaml
depends_on:
  - db
  - redis
```

#### Long Syntax with Conditions

```yaml
depends_on:
  db:
    condition: service_healthy
    restart: true            # Restart when dependency updates
  redis:
    condition: service_started
  migration:
    condition: service_completed_successfully
    required: false          # Warning instead of error if missing
```

**Conditions**:
- `service_started` — Service has started (default)
- `service_healthy` — Healthcheck passes
- `service_completed_successfully` — Service exits with code 0

### 2.12 Healthcheck

```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost"]
  interval: 1m30s
  timeout: 10s
  retries: 3
  start_period: 40s
  start_interval: 5s
```

**Test formats**:
- `["CMD", "command", "arg"]` — Execute command directly
- `["CMD-SHELL", "command"]` or string — Run via shell
- `NONE` — Disable healthcheck

### 2.13 Deploy

**Source**: https://docs.docker.com/compose/compose-file/deploy/

#### Replicas and Mode

```yaml
deploy:
  mode: replicated      # replicated (default), global, replicated-job, global-job
  replicas: 6
```

#### Resources

```yaml
deploy:
  resources:
    limits:
      cpus: '0.50'
      memory: 50M
      pids: 1
    reservations:
      cpus: '0.25'
      memory: 20M
```

#### Device Reservations

```yaml
deploy:
  resources:
    reservations:
      devices:
        - capabilities: ["nvidia-compute"]
          driver: nvidia
          count: 2   # or "all"
```

Device attributes: `capabilities`, `driver`, `count`, `device_ids` (mutually exclusive with count), `options`.

#### Restart Policy

```yaml
deploy:
  restart_policy:
    condition: on-failure    # none, on-failure, any (default)
    delay: 5s                # Default: 0
    max_attempts: 3          # Default: unlimited
    window: 120s             # Default: immediate
```

#### Placement

```yaml
deploy:
  placement:
    constraints:
      - node.labels.disktype==ssd
    preferences:
      - spread: node.labels.zone
```

#### Update Config

```yaml
deploy:
  update_config:
    parallelism: 2
    delay: 10s
    failure_action: pause    # continue, rollback, pause (default)
    monitor: 30s
    max_failure_ratio: 0.1
    order: stop-first        # stop-first (default), start-first
```

#### Rollback Config

```yaml
deploy:
  rollback_config:
    parallelism: 0           # 0 = all at once
    delay: 0s
    failure_action: pause    # continue, pause (default)
    monitor: 0s
    max_failure_ratio: 0
    order: stop-first
```

#### Endpoint Mode

```yaml
deploy:
  endpoint_mode: vip     # vip (virtual IP), dnsrr (DNS round-robin)
```

#### Deploy Labels

```yaml
deploy:
  labels:
    com.example.description: "This label appears on the service, not container"
```

### 2.14 Restart (Service-Level)

```yaml
restart: "no"
restart: always
restart: on-failure
restart: on-failure:3
restart: unless-stopped
```

### 2.15 Logging

```yaml
logging:
  driver: syslog
  options:
    syslog-address: "tcp://192.168.0.42:123"
```

### 2.16 Labels and Annotations

```yaml
labels:
  com.example.description: "Accounting webapp"
  com.example.department: "Finance"
# or list syntax:
labels:
  - "com.example.description=Accounting webapp"

# Load from files
label_file:
  - ./app.labels
  - ./additional.labels

annotations:
  com.example.foo: bar
```

### 2.17 Container Identity

```yaml
container_name: my-web-container    # Prevents scaling!
hostname: my-host                   # RFC 1123 compliant
domainname: example.com
```

`container_name` follows regex `[a-zA-Z0-9][a-zA-Z0-9_.-]+`.

### 2.18 User and Working Directory

```yaml
user: "1000:1000"
working_dir: /app
```

### 2.19 Terminal and Input

```yaml
stdin_open: true    # -i flag equivalent
tty: true           # Allocate pseudo-TTY
init: true          # Run init process (PID 1) for signal forwarding
```

### 2.20 Security

```yaml
privileged: true

cap_add:
  - ALL
cap_drop:
  - NET_ADMIN

security_opt:
  - label=user:USER
  - label=role:ROLE

group_add:
  - mail
  - root

read_only: true     # Read-only root filesystem
```

### 2.21 Sysctls and Ulimits

```yaml
sysctls:
  net.core.somaxconn: 1024
  net.ipv4.tcp_syncookies: 0

ulimits:
  nproc: 65535
  nofile:
    soft: 20000
    hard: 40000

shm_size: "2gb"
```

### 2.22 Process Namespacing

```yaml
pid: "host"
ipc: "shareable"              # or "service:other_service"
uts: "host"
userns_mode: "host"
cgroup: "host"                # or "private"
cgroup_parent: /custom/cgroup
pids_limit: 100               # -1 for unlimited
```

### 2.23 DNS and Hosts

```yaml
extra_hosts:
  - "somehost=162.242.195.82"
  - "myhostv6=[::1]"

dns:
  - 8.8.8.8
  - 9.9.9.9

dns_search:
  - example.com

dns_opt:
  - use-vc
  - no-tld-query
```

### 2.24 Devices

```yaml
devices:
  - "/dev/ttyUSB0:/dev/ttyUSB0"
  - "/dev/sda:/dev/xvda:rwm"
  - "vendor1.com/device=gpu"

device_cgroup_rules:
  - 'c 1:3 mr'
  - 'a 7:* rmw'

gpus:
  - driver: 3dfx
    count: 2
# or
gpus: all
```

### 2.25 Profiles

```yaml
profiles: ["frontend", "debug"]
```

Services without profiles are ALWAYS enabled. See Section 7 for full details.

### 2.26 Extends

```yaml
extends:
  file: common.yml
  service: webapp
```

Merges configurations with precedence rules. See Section 8 for full details.

### 2.27 Platform

```yaml
platform: darwin
platform: windows/amd64
platform: linux/arm64/v8
```

Format: `os[/arch[/variant]]`

### 2.28 Pull Policy

```yaml
pull_policy: always     # always, never, missing (default), build, daily, weekly, every_<duration>
```

- `always`: Pull from registry every time
- `never`: Use only cached images
- `missing`: Pull if unavailable locally (default)
- `build`: Always rebuild
- `daily`/`weekly`/`every_<duration>`: Time-based refresh

### 2.29 Stop and Lifecycle

```yaml
stop_signal: SIGUSR1            # Default: SIGTERM
stop_grace_period: 1m30s        # Grace period before SIGKILL
```

#### Lifecycle Hooks

```yaml
post_start:
  - command: ./startup.sh
    user: root
    privileged: true
    working_dir: /app
    environment:
      - VAR=value

pre_stop:
  - command: ./cleanup.sh
```

`pre_stop` runs before container shutdown (not triggered by sudden termination).

### 2.30 Resource Configuration (Service-Level)

#### CPU

- `cpus`: Fractional CPU allocation (0.000 = unlimited)
- `cpu_count`: Number of usable CPUs
- `cpu_percent`: Percentage of available CPUs
- `cpu_shares`: Relative weight vs. other containers
- `cpu_period`: CFS period
- `cpu_quota`: CFS quota
- `cpu_rt_runtime`: Real-time scheduler allocation
- `cpu_rt_period`: Real-time scheduler period
- `cpuset`: Explicit CPU ranges (`0-3` or `0,1,2`)

#### Memory

- `mem_limit`: Maximum memory
- `mem_reservation`: Memory guarantee
- `mem_swappiness`: Swap priority (0-100%)
- `memswap_limit`: Total memory + swap limit
- `oom_kill_disable`: Prevent OOM kills
- `oom_score_adj`: OOM kill preference (-1000 to 1000)

#### Block I/O

```yaml
blkio_config:
  weight: 300
  weight_device:
    - path: /dev/sda
      weight: 400
  device_read_bps:
    - path: /dev/sdb
      rate: '12mb'
  device_read_iops:
    - path: /dev/sdb
      rate: 120
  device_write_bps:
    - path: /dev/sdb
      rate: '1024k'
  device_write_iops:
    - path: /dev/sdb
      rate: 30
```

### 2.31 Secrets (Service-Level)

Short syntax:
```yaml
secrets:
  - server-certificate
```

Long syntax:
```yaml
secrets:
  - source: server-certificate
    target: server.cert
    uid: "103"
    gid: "103"
    mode: 0o440
```

### 2.32 Configs (Service-Level)

Similar syntax to secrets — short form (name only) or long form with `source`, `target`, `uid`, `gid`, `mode`.

### 2.33 Additional Service Attributes

- `attach`: Control log collection (default `true`)
- `runtime`: OCI runtime (e.g., `runc`)
- `scale`: Default container count (consistent with deploy replicas)
- `isolation`: Container isolation technology (platform-specific)
- `mac_address`: MAC address assignment
- `storage_opt`: Storage driver options
- `credential_spec`: Windows credential spec (`file://` or `registry://`)
- `use_api_socket`: Access container engine API within container

#### External and Service Links

```yaml
external_links:
  - redis
  - database:mysql

links:
  - db
  - db:database    # with alias
```

#### Provider Services

```yaml
provider:
  type: awesomecloud
  options:
    type: mysql
    foo: bar
```

#### AI Models

```yaml
# Short syntax
models:
  - my_model

# Long syntax
models:
  my_model:
    endpoint_var: MODEL_URL
    model_var: MODEL
```

---

## 3. Networks Configuration

**Source**: https://docs.docker.com/compose/compose-file/06-networks/

### Default Network

Compose creates an implicit `default` network. All services connect to it unless explicitly configured otherwise. Services on the same network can communicate; services on different networks are isolated.

### Network Drivers

```yaml
networks:
  db-data:
    driver: bridge    # bridge, host, overlay, macvlan, none, or custom
```

Available drivers:
- `bridge` — Default. Isolated network on a single host
- `host` — Use host's networking directly
- `overlay` — Multi-host networking (Swarm)
- `macvlan` — Assign MAC address to container
- `none` — No networking

Compose returns an error if the driver is not available on the platform.

### Driver Options

```yaml
networks:
  mynet:
    driver: bridge
    driver_opts:
      com.docker.network.bridge.host_binding_ipv4: "127.0.0.1"
```

### IPAM Configuration

```yaml
networks:
  mynet:
    ipam:
      driver: default
      config:
        - subnet: 172.28.0.0/16
          ip_range: 172.28.5.0/24
          gateway: 172.28.5.254
          aux_addresses:
            host1: 172.28.1.5
```

IPAM elements:
- `driver`: IPAM driver (default: `default`)
- `config`: List of configuration blocks
  - `subnet`: CIDR-formatted network segment
  - `ip_range`: Allocatable container IP range
  - `gateway`: IPv4/IPv6 gateway for master subnet
  - `aux_addresses`: Auxiliary addresses mapped to hostnames

### IPv6 and IPv4

```yaml
networks:
  mynet:
    enable_ipv6: true
    enable_ipv4: false    # IPv6-only network
```

### Network Attributes

- `attachable`: When `true`, standalone containers can attach (not just services)
- `internal`: When `true`, creates externally isolated network
- `labels`: Metadata in dictionary or array format (reverse-DNS notation recommended)

### External Networks

```yaml
networks:
  outside:
    external: true
    name: "${NETWORK_ID}"    # Supports variable interpolation
```

When `external: true`, Compose does NOT create the network and errors if it doesn't exist. Only `name` is relevant alongside `external`.

### Custom Network Name

```yaml
networks:
  db-data:
    name: "my-custom-network"
```

### Customizing the Default Network

```yaml
networks:
  default:
    name: a_network
    driver_opts:
      com.docker.network.bridge.host_binding_ipv4: 127.0.0.1
```

### Network Isolation Pattern

```yaml
services:
  proxy:
    networks:
      - frontend
  app:
    networks:
      - frontend
      - backend
  db:
    networks:
      - backend

networks:
  frontend:
  backend:
```

In this pattern, `proxy` is isolated from `db` — only `app` can talk to both.

---

## 4. Volumes Configuration

**Source**: https://docs.docker.com/compose/compose-file/07-volumes/

Volumes are persistent data stores implemented by the container engine.

### Named Volumes

```yaml
services:
  backend:
    image: example/database
    volumes:
      - db-data:/etc/data

  backup:
    image: backup-service
    volumes:
      - db-data:/var/lib/backup/data

volumes:
  db-data:
```

When running `docker compose up`, volumes are created automatically if they don't exist, or reused if they do.

### Volume Driver

```yaml
volumes:
  db-data:
    driver: foobar
```

### Driver Options

```yaml
volumes:
  example:
    driver_opts:
      type: "nfs"
      o: "addr=10.40.0.199,nolock,soft,rw"
      device: ":/docker/example"
```

### External Volumes

```yaml
volumes:
  db-data:
    external: true
```

When `external: true`, Compose does NOT create the volume and errors if it doesn't exist. Only `name` is relevant alongside `external`.

### Volume Labels

```yaml
volumes:
  db-data:
    labels:
      com.example.description: "Database volume"
      com.example.department: "IT/Ops"
# or array:
    labels:
      - "com.example.description=Database volume"
```

Labels apply to named volumes only, NOT bind mounts. Visible via `docker volume inspect`.

### Custom Volume Name

```yaml
volumes:
  db-data:
    name: "my-app-data"

# With variable interpolation
volumes:
  db-data:
    name: ${DATABASE_VOLUME}

# Combined with external
volumes:
  db-data:
    external: true
    name: actual-name-of-volume
```

---

## 5. Configs & Secrets

### Configs

**Source**: https://docs.docker.com/compose/compose-file/08-configs/

Configs enable services to adapt behavior without rebuilding images. Mounted as files at `/<config-name>` (Linux) or `C:\<config-name>` (Windows) by default.

Default permissions:
- Owned by the container command executor (overridable)
- World-readable (mode `0444`) unless overridden
- Only accessible to services explicitly granted via `configs` attribute

#### File-Based Config

```yaml
configs:
  http_config:
    file: ./httpd.conf
```

#### Environment Config (Compose 2.23.1+)

```yaml
configs:
  simple_config:
    environment: "SIMPLE_CONFIG_VALUE"
```

#### Content Config (Compose 2.23.1+)

```yaml
configs:
  app_config:
    content: |
      debug=${DEBUG}
      spring.application.name=${COMPOSE_PROJECT_NAME}
```

Supports variable interpolation.

#### External Config

```yaml
configs:
  http_config:
    external: true
```

When `external: true`, all other attributes are irrelevant. Compose rejects files with additional fields.

#### Custom Config Name

```yaml
configs:
  http_config:
    external: true
    name: "${HTTP_CONFIG_KEY}"    # Dynamic lookup
```

### Secrets

**Source**: https://docs.docker.com/compose/compose-file/09-secrets/

Secrets are a flavor of Configs focusing on sensitive data. Services access secrets only when explicitly granted via `secrets` attribute.

#### File-Based Secret

```yaml
secrets:
  server-certificate:
    file: ./server.cert
```

#### Environment-Based Secret

```yaml
secrets:
  token:
    environment: "OAUTH_TOKEN"
```

Secrets are created with naming convention: `<project_name>_secret-name` when deployed.

#### Mounting in Services

Short syntax:
```yaml
services:
  web:
    secrets:
      - server-certificate
```

Long syntax:
```yaml
services:
  web:
    secrets:
      - source: server-certificate
        target: server.cert
        uid: "103"
        gid: "103"
        mode: 0o440
```

---

## 6. Environment Variables

**Source**: https://docs.docker.com/compose/how-tos/environment-variables/

### Variable Substitution Syntax

**Source**: https://docs.docker.com/compose/how-tos/environment-variables/variable-interpolation/

#### Braced Expressions

| Syntax | Behavior |
|--------|----------|
| `${VAR}` | Direct substitution |
| `${VAR:-default}` | Use `default` if VAR is unset **or empty** |
| `${VAR-default}` | Use `default` only if VAR is **unset** |
| `${VAR:?error}` | Error if VAR is unset **or empty** |
| `${VAR?error}` | Error if VAR is **unset** |
| `${VAR:+replacement}` | Use `replacement` if VAR is set **and non-empty** |
| `${VAR+replacement}` | Use `replacement` if VAR is **set** |

Unbraced syntax `$VAR` is also supported.

Interpolation is applied for **unquoted and double-quoted values** only.

#### Escaping

Use `$$` to produce a literal `$` sign:
```yaml
command: /bin/sh -c 'echo "hello $$HOSTNAME"'
```

### .env File

**Default location**: `.env` file at project root, next to `compose.yaml`.

**Syntax rules**:
- Comments: lines starting with `#`
- Blank lines ignored
- Key-value pairs use `=` or `:` as delimiters
- Whitespace around values is trimmed
- Unquoted and double-quoted values support interpolation
- Single-quoted values are literal: `'${VAR}'` stays as `${VAR}`
- Inline comments: for unquoted values, require preceding space (`VAR=VAL # comment`)
- Inline comments for quoted values follow the closing quote (`VAR="VAL" # comment`)
- Escaped quotes: `VAR='Let\'s'` produces `Let's`
- Escape sequences in double-quoted values: `\n`, `\r`, `\t`, `\\`
- Single-quoted values can span multiple lines

**Alternative location**:
```bash
docker compose --env-file ./config/.env.dev up
```

**Multiple .env files** (later files override earlier):
```bash
docker compose --env-file .env --env-file .env.override up
```

Invalid path returns an error.

### Precedence Order (Highest to Lowest)

**Source**: https://docs.docker.com/compose/how-tos/environment-variables/envvars-precedence/

1. **`docker compose run -e` CLI flag** — Explicit command-line values override all
2. **`environment` or `env_file` with shell/interpolation** — Values from shell or `.env` files
3. **`environment` attribute** — Static values in compose.yaml
4. **`env_file` attribute** — Values from referenced external files
5. **Image `ENV` directive** — Dockerfile ENV applies only when Compose doesn't override

Key principle: "Having any ARG or ENV setting in a Dockerfile evaluates only if there is no Docker Compose entry for `environment`, `env_file` or `run --env`."

#### Important Edge Cases

- `--env VALUE=1.8` (explicit) ALWAYS wins regardless of other sources
- `--env VALUE` (reference without assignment) looks up from Host OS env, then `.env` file
- Host OS environment takes precedence over `.env` file for interpolation
- Local environment variables do NOT automatically propagate into containers without explicit Compose config

### Setting Environment Variables

**Source**: https://docs.docker.com/compose/how-tos/environment-variables/set-environment-variables/

#### In compose.yaml

```yaml
environment:
  DEBUG: "true"
# or
environment:
  - DEBUG=true
```

#### Shell Pass-Through

```yaml
environment:
  - DEBUG    # Inherits from host shell, no warning if undefined
```

#### With Interpolation

```yaml
environment:
  - DEBUG=${DEBUG}    # Generates warning if unset
```

#### Via CLI

```bash
docker compose run -e DEBUG=1 web python console.py
docker compose run -e DEBUG web python console.py    # Pass from shell
```

#### Verification

```bash
docker compose config                    # View resolved config
docker compose config --environment      # View interpolation variables
docker compose -e DATABASE_URL=value up  # Override specific variable
```

> **Security note**: Docker documentation explicitly cautions against using environment variables for sensitive data like passwords — use secrets instead.

> **Limitation**: Substitution from `.env` files is a Docker Compose CLI feature. NOT supported by Swarm with `docker stack deploy`.

---

## 7. Profiles

**Source**: https://docs.docker.com/compose/how-tos/profiles/

### Profile Assignment

```yaml
services:
  frontend:
    profiles: [frontend]
  phpmyadmin:
    profiles: [debug]
  backend:    # No profile = ALWAYS enabled
```

Valid profile names: `[a-zA-Z0-9][a-zA-Z0-9_.-]+`

Services WITHOUT profile assignments are **always enabled** and start by default.

### Activation Methods

#### Command-line Flag

```bash
docker compose --profile debug up
docker compose --profile frontend --profile debug up
docker compose --profile "*" up    # Enable ALL profiles
```

#### Environment Variable

```bash
COMPOSE_PROFILES=debug docker compose up
COMPOSE_PROFILES=frontend,debug docker compose up
```

### Dependency Resolution and Auto-Activation

When explicitly targeting a service with assigned profiles, Compose runs that service **regardless of whether its profile is activated**:

```yaml
services:
  db:
    image: postgres
  db-migrations:
    profiles: [tools]
    depends_on: [db]
```

Running `docker compose run db-migrations` activates the service and its dependencies automatically, even though the `tools` profile isn't explicitly enabled.

**Critical constraint**: If a targeted service has profiled dependencies, those dependencies must either share the same profile, be started separately, or have no profile assignment.

### Stopping Profiled Services

```bash
docker compose --profile debug down    # Stops debug-profiled + unprofiled services
docker compose down phpmyadmin         # Stop single service
docker compose stop phpmyadmin         # Stop without removing
```

---

## 8. Merge & Override Patterns

### Multiple Compose Files with -f

**Source**: https://docs.docker.com/compose/how-tos/multiple-compose-files/merge/

Default behavior: Compose loads `compose.yaml` followed by optional `compose.override.yaml`.

```bash
docker compose -f compose.yaml -f compose.admin.yaml run backup_db
```

Files are processed left-to-right. Later files override and extend predecessors.

#### Merge Rules by Field Type

**Single-Value Fields (Replacement)**:
Fields like `image`, `command`, `mem_limit` — later file completely replaces earlier value.

**Multi-Value Sequence Fields (Concatenation)**:
`ports`, `expose`, `external_links`, `dns`, `dns_search`, `tmpfs` — values from all files are concatenated.

Example: Base `expose: ["3000"]` + Override `expose: ["4000", "5000"]` = Result `expose: ["3000", "4000", "5000"]`

**Key-Value Mapping Fields (Smart Merge)**:
`environment`, `labels`, `volumes`, `devices` — merge by key/mount-path.

- **Environment & Labels**: Key determines precedence. Later files override matching keys while preserving unmatched entries.
- **Volumes & Devices**: Container mount path is the identifier. Later files override by target path.

Example: Base `environment: [FOO=original, BAR=original]` + Override `environment: [BAR=local, BAZ=local]` = Result: `FOO=original, BAR=local, BAZ=local`

#### Path Resolution

All paths are relative to the **first (base) file**. Use `docker compose config` to verify merged configuration.

Override files need NOT be complete or valid standalone Compose files — they can be fragments.

### Extends Directive

**Source**: https://docs.docker.com/compose/how-tos/multiple-compose-files/extends/

#### Syntax

From another file:
```yaml
services:
  web:
    extends:
      file: common-services.yml
      service: webapp
```

Within the same file:
```yaml
services:
  web:
    extends: webapp
```

#### Behavior
- Inherits properties from a specified service without including that service in final project (unless explicitly defined)
- Locally-defined attributes override extended values
- Supports multiple levels of extension
- Relative paths in extended files are automatically converted

#### Path Resolution

All paths are relative to the base Compose file. When extending from different folders, relative paths in the extended service are converted automatically (e.g., `./container.env` becomes `../commons/container.env`).

### Include Directive

**Source**: https://docs.docker.com/compose/how-tos/multiple-compose-files/include/

```yaml
include:
  - my-compose-include.yaml
  - oci://docker.io/username/my-compose-app:latest
```

#### Behavior
- Each included file loads as an **individual Compose application model** with its own project directory
- Relative paths resolve within each file's directory scope
- All resources are merged into the current application model
- Works recursively — included files can have their own `include` sections
- Supports remote sources (OCI artifacts, Git repositories)

#### Conflict Resolution

Direct conflicts between resources are rejected with an error. Customization through:

1. **Paired override files**:
```yaml
include:
  - path:
      - third-party/compose.yaml
      - override.yaml
```

2. **Global override file**: `compose.override.yaml` modifies the merged model.

---

## 9. Compose CLI Commands

**Source**: https://docs.docker.com/reference/cli/docker/compose/

### Global Options

| Option | Default | Purpose |
|--------|---------|---------|
| `--all-resources` | — | Include all resources, even unused by services |
| `--ansi` | `auto` | ANSI control character output (`never`/`always`/`auto`) |
| `--compatibility` | — | Backward compatibility mode |
| `--dry-run` | — | Execute without changing stack state |
| `--env-file` | — | Alternate environment file |
| `-f`, `--file` | — | Compose configuration files (supports multiple) |
| `--parallel` | `-1` | Max parallelism for concurrent engine calls (-1 = unlimited) |
| `--profile` | — | Active profiles to enable |
| `--progress` | — | Progress output type (`auto`, `tty`, `plain`, `json`, `quiet`) |
| `--project-directory` | — | Alternate working directory |
| `-p`, `--project-name` | — | Project name |

### Project Name Resolution (Precedence)

1. `-p` / `--project-name` flag
2. `COMPOSE_PROJECT_NAME` environment variable
3. `name:` in config file
4. Basename of project directory
5. Basename of current directory

Names must contain only lowercase letters, digits, dashes, underscores, and begin with lowercase letter or digit.

### Environment Variables

| Variable | Equivalent |
|----------|-----------|
| `COMPOSE_FILE` | `-f` flag |
| `COMPOSE_PROJECT_NAME` | `-p` flag |
| `COMPOSE_PROFILES` | `--profile` flag |
| `COMPOSE_PARALLEL_LIMIT` | `--parallel` flag |
| `COMPOSE_IGNORE_ORPHANS` | Set `true` to disable orphan detection |
| `COMPOSE_MENU` | Set `false` to disable helper menu |

### Lifecycle Management Commands

| Command | Purpose |
|---------|---------|
| `docker compose up` | Create and start containers |
| `docker compose down` | Stop and remove containers, networks |
| `docker compose start` | Start existing services |
| `docker compose stop` | Stop running services |
| `docker compose restart` | Restart service containers |
| `docker compose pause` | Pause services |
| `docker compose unpause` | Unpause services |
| `docker compose kill` | Force stop service containers |
| `docker compose wait` | Block until containers stop |

### Container Operations

| Command | Purpose |
|---------|---------|
| `docker compose ps` | List containers |
| `docker compose logs` | View container output |
| `docker compose top` | Display running processes |
| `docker compose events` | Receive real-time events |
| `docker compose exec` | Execute command in running container |
| `docker compose run` | Run one-off command on a service |
| `docker compose attach` | Attach stdin/stdout/stderr to running container |
| `docker compose cp` | Copy files between container and filesystem |
| `docker compose export` | Export container filesystem as tar |
| `docker compose commit` | Create image from container changes |

### Image Operations

| Command | Purpose |
|---------|---------|
| `docker compose build` | Build or rebuild services |
| `docker compose pull` | Pull service images |
| `docker compose push` | Push service images |
| `docker compose images` | List images used by containers |

### Configuration & Inspection

| Command | Purpose |
|---------|---------|
| `docker compose config` | Parse, resolve, render Compose file in canonical format |
| `docker compose convert` | Convert to platform's canonical format |
| `docker compose bridge` | Convert into another model |
| `docker compose port` | Print public port for binding |
| `docker compose volumes` | List volumes |
| `docker compose version` | Show version information |

### Resource Management

| Command | Purpose |
|---------|---------|
| `docker compose create` | Create containers without starting |
| `docker compose rm` | Remove stopped containers |
| `docker compose scale` | Scale services |
| `docker compose stats` | Live resource usage statistics |

### File Monitoring

| Command | Purpose |
|---------|---------|
| `docker compose watch` | Watch build context, rebuild/refresh on changes |
| `docker compose ls` | List running Compose projects |

### Publishing

| Command | Purpose |
|---------|---------|
| `docker compose publish` | Publish Compose application |

### Dry Run Mode

`--dry-run` shows all steps Compose would apply without executing. Works with most commands except state-query commands (`ps`, `ls`, `logs`).

---

## 10. Compose Watch (File Sync)

**Source**: https://docs.docker.com/compose/how-tos/file-watch/

### Usage

```bash
docker compose up --watch
# or separately from logs:
docker compose watch
```

### Develop Section Configuration

```yaml
services:
  web:
    build: .
    develop:
      watch:
        - action: sync
          path: ./web
          target: /src/web
          initial_sync: true
          ignore:
            - node_modules/
        - action: rebuild
          path: package.json
        - action: sync+restart
          path: ./proxy/nginx.conf
          target: /etc/nginx/conf.d/default.conf
```

### Action Types

| Action | Behavior | Use Case |
|--------|----------|----------|
| `sync` | Syncs host files to container | Hot Reload frameworks |
| `rebuild` | Builds new image, replaces container | Compiled languages, dependency changes |
| `sync+restart` | Syncs files then restarts container | Config file changes (e.g., nginx.conf) |

### Path and Target Mapping

For source file change at `./app/html/index.html`:
- `target: /app/html` results in `/app/html/index.html`
- `target: /app/static` results in `/app/static/index.html`
- `target: /assets` results in `/assets/index.html`

### Ignore Patterns

Patterns are relative to the `path` in the current watch action, NOT the project directory.

Default ignored:
- Files matching `.dockerignore` rules
- Temporary/backup files for common IDEs (Vim, Emacs, JetBrains)
- `.git` directories

### initial_sync

When using `sync` or `sync+restart`, `initial_sync: true` ensures files are up-to-date before starting a new watch session.

### Prerequisites

Container image must contain: `stat`, `mkdir`, `rmdir`.
Container's `USER` must be able to write to the target path. Use `COPY --chown` in Dockerfile:

```dockerfile
COPY --chown=app:app . /app
```

### Constraints

- Works ONLY with services using `build` attribute, NOT pre-built `image` services
- Does NOT replace bind mount functionality
- Does NOT support glob patterns in path definitions
- Performance benefits from ignoring large directories like `node_modules/`

### Complete Example

```yaml
services:
  web:
    build: .
    command: npm start
    develop:
      watch:
        - action: sync
          path: ./web
          target: /app/web
          ignore:
            - node_modules/
        - action: sync+restart
          path: ./proxy/nginx.conf
          target: /etc/nginx/conf.d/default.conf

  backend:
    build:
      context: backend
      target: builder
```

---

## 11. Common Anti-Patterns

### ❌ Using `version:` Field

The `version` field is **deprecated** and ignored by modern Compose. Legacy versions 2.x and 3.x were merged into the unified Compose Specification.

```yaml
# ❌ Anti-pattern
version: "3.8"
services:
  web:
    image: nginx

# ✅ Best practice
services:
  web:
    image: nginx
```

### ❌ Not Using Healthchecks with depends_on

Without healthchecks, `depends_on` only waits for the container to start, not for the service to be ready.

```yaml
# ❌ Anti-pattern — db may not be ready when app starts
services:
  app:
    depends_on:
      - db
  db:
    image: postgres

# ✅ Best practice — wait for db to be healthy
services:
  app:
    depends_on:
      db:
        condition: service_healthy
  db:
    image: postgres
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5
```

### ❌ Hardcoded Environment Values

Hardcoding values makes configurations inflexible and risks leaking secrets.

```yaml
# ❌ Anti-pattern
services:
  app:
    environment:
      DATABASE_PASSWORD: "my-secret-password"

# ✅ Best practice — use interpolation + .env file
services:
  app:
    environment:
      DATABASE_PASSWORD: ${DATABASE_PASSWORD:?Database password is required}
```

### ❌ Not Using Named Volumes for Data Persistence

Anonymous volumes are recreated on `docker compose down` and data is lost.

```yaml
# ❌ Anti-pattern — anonymous volume, data lost on down
services:
  db:
    image: postgres
    volumes:
      - /var/lib/postgresql/data

# ✅ Best practice — named volume persists across restarts
services:
  db:
    image: postgres
    volumes:
      - db-data:/var/lib/postgresql/data

volumes:
  db-data:
```

### ❌ Using container_name in Scalable Services

`container_name` prevents scaling because container names must be unique.

```yaml
# ❌ Anti-pattern — cannot scale
services:
  web:
    image: nginx
    container_name: my-nginx

# ✅ Best practice — let Compose manage names
services:
  web:
    image: nginx
```

### ❌ Not Using Profiles for Optional Services

Running all services always wastes resources when some are only needed for development or debugging.

```yaml
# ❌ Anti-pattern — debug tools always running
services:
  app:
    image: myapp
  phpmyadmin:
    image: phpmyadmin

# ✅ Best practice — debug tools behind profile
services:
  app:
    image: myapp
  phpmyadmin:
    image: phpmyadmin
    profiles: [debug]
```

### ❌ Using `restart: always` Without Resource Limits

A crashing container with `restart: always` and no resource limits can consume all system resources.

```yaml
# ❌ Anti-pattern
services:
  app:
    image: myapp
    restart: always

# ✅ Best practice — combine with resource limits
services:
  app:
    image: myapp
    restart: unless-stopped
    deploy:
      resources:
        limits:
          cpus: '0.50'
          memory: 512M
```

### ❌ Exposing Ports to All Interfaces

By default, port mappings bind to `0.0.0.0`, exposing the service to all network interfaces.

```yaml
# ❌ Anti-pattern — exposed to all interfaces
ports:
  - "8080:80"

# ✅ Best practice — bind to localhost for development
ports:
  - "127.0.0.1:8080:80"
```

---

## Sources Summary

| Section | Source URL |
|---------|-----------|
| Compose file reference | https://docs.docker.com/compose/compose-file/ |
| Services | https://docs.docker.com/compose/compose-file/05-services/ |
| Networks | https://docs.docker.com/compose/compose-file/06-networks/ |
| Volumes | https://docs.docker.com/compose/compose-file/07-volumes/ |
| Configs | https://docs.docker.com/compose/compose-file/08-configs/ |
| Secrets | https://docs.docker.com/compose/compose-file/09-secrets/ |
| Environment variables | https://docs.docker.com/compose/how-tos/environment-variables/ |
| Variable interpolation | https://docs.docker.com/compose/how-tos/environment-variables/variable-interpolation/ |
| Env var precedence | https://docs.docker.com/compose/how-tos/environment-variables/envvars-precedence/ |
| Setting env vars | https://docs.docker.com/compose/how-tos/environment-variables/set-environment-variables/ |
| Profiles | https://docs.docker.com/compose/how-tos/profiles/ |
| Multiple files | https://docs.docker.com/compose/how-tos/multiple-compose-files/ |
| Merge rules | https://docs.docker.com/compose/how-tos/multiple-compose-files/merge/ |
| Extends | https://docs.docker.com/compose/how-tos/multiple-compose-files/extends/ |
| Include | https://docs.docker.com/compose/how-tos/multiple-compose-files/include/ |
| Compose CLI | https://docs.docker.com/reference/cli/docker/compose/ |
| Deploy spec | https://docs.docker.com/compose/compose-file/deploy/ |
| Build spec | https://docs.docker.com/compose/compose-file/build/ |
| File watch | https://docs.docker.com/compose/how-tos/file-watch/ |
