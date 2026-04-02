# Service Attributes Reference

Complete reference for all Docker Compose service attributes. Organized by category.

---

## Image and Build

### image

Specifies the container image in OCI format.

```yaml
image: redis                                          # Latest tag
image: redis:7.2-alpine                               # Specific tag
image: redis@sha256:0ed5d5928d47...                   # Digest-pinned
image: registry.example.com:5000/myapp:latest         # Private registry
```

### build

Defines how to create Docker images from source. String shorthand or object with granular control.

```yaml
# String shorthand
build: ./dir

# Full object
build:
  context: ./dir                          # Build context (required)
  dockerfile: custom.Dockerfile           # Alternate Dockerfile (relative to context)
  dockerfile_inline: |                    # Inline Dockerfile (mutually exclusive with dockerfile)
    FROM baseimage
    RUN some command
  args:                                   # Build arguments
    GIT_COMMIT: cdc3b19
    # or list:
    # - GIT_COMMIT=cdc3b19
  target: production                      # Multi-stage build target
  cache_from:                             # Cache sources
    - alpine:latest
    - type=local,src=path/to/cache
    - type=gha
  cache_to:                               # Cache destinations
    - user/app:cache
    - type=local,dest=path/to/cache
  no_cache: false                         # Disable builder cache
  secrets:                                # Build-time secrets
    - server-certificate                  # Short syntax
    - source: server-certificate          # Long syntax
      target: cert
      uid: "103"
      gid: "103"
      mode: 0440
  ssh:                                    # SSH agent forwarding
    - default
    - myproject=~/.ssh/myproject.pem
  platforms:                              # Multi-platform builds
    - "linux/amd64"
    - "linux/arm64"
  additional_contexts:                    # Named contexts
    resources: /path/to/resources
    app: docker-image://my-app:latest
    source: https://github.com/user/repo.git
  network: host                           # Build network mode (host, none)
  shm_size: "2gb"                         # Shared memory size
  ulimits:                                # Build-time ulimits
    nproc: 65535
    nofile:
      soft: 20000
      hard: 40000
  extra_hosts:                            # /etc/hosts entries during build
    - "somehost=162.242.195.82"
  labels:                                 # Image labels
    com.example.description: "Accounting webapp"
  tags:                                   # Additional image tags
    - "myimage:mytag"
    - "registry/username/myrepos:my-other-tag"
  privileged: true                        # Privileged build mode
  provenance: true                        # Build provenance attestation
  sbom: true                              # Software Bill of Materials
  entitlements:                           # Build entitlements
    - network.host
    - security.insecure
```

---

## Command and Entrypoint

### command

Overrides the image CMD.

```yaml
command: bundle exec thin -p 3000                     # String (shell form)
command: /bin/sh -c 'echo "hello $$HOSTNAME"'         # Shell with escaping
command: ["php", "-d", "zend_extension=/path"]         # List (exec form)
```

- `null` uses image default
- `[]` or `''` clears the command

### entrypoint

Overrides the image ENTRYPOINT.

```yaml
entrypoint: /code/entrypoint.sh                       # String form
entrypoint: ["php", "-d", "memory_limit=-1", "vendor/bin/phpunit"]  # List form
```

---

## Networking

### ports

#### Short Syntax: `[HOST:]CONTAINER[/PROTOCOL]`

```yaml
ports:
  - "3000"                         # Container port only (random host port)
  - "8000:8000"                    # HOST:CONTAINER
  - "9090-9091:8080-8081"          # Port range
  - "127.0.0.1:8001:8001"         # Bind to specific interface
  - "6060:6060/udp"                # UDP protocol
  - "[::1]:6001:6001"              # IPv6
```

#### Long Syntax

```yaml
ports:
  - name: web                      # Port name (optional)
    target: 80                     # Container port (required)
    published: "8080"              # Host port
    host_ip: 127.0.0.1            # Interface to bind
    protocol: tcp                  # tcp or udp
    app_protocol: http             # Application protocol hint
    mode: host                     # host or ingress
```

### expose

Internal port exposure without host publishing.

```yaml
expose:
  - "3000"
  - "8080-8085/tcp"
```

### networks (service-level)

```yaml
networks:
  - frontend
  - backend

# With options
networks:
  backend:
    aliases:
      - db-alias
    ipv4_address: 172.16.238.10
    ipv6_address: 2001:3984:3989::10
    interface_name: eth1
    mac_address: "02:42:ac:11:00:02"
    driver_opts:
      com.docker.network.endpoint.dnsnames: myservice
    gw_priority: 100
    priority: 1000
    link_local_ips:
      - 169.254.0.10
```

### network_mode

```yaml
network_mode: "host"
network_mode: "none"
network_mode: "service:other_service"
network_mode: "container:container_id"
```

### dns, dns_search, dns_opt

```yaml
dns:
  - 8.8.8.8
  - 9.9.9.9
dns_search:
  - example.com
dns_opt:
  - use-vc
  - no-tld-query
```

### extra_hosts

```yaml
extra_hosts:
  - "somehost=162.242.195.82"
  - "myhostv6=[::1]"
```

### links and external_links

```yaml
links:
  - db
  - db:database                    # With alias
external_links:
  - redis
  - database:mysql
```

---

## Environment

### environment

```yaml
# Map syntax
environment:
  RACK_ENV: development
  SHOW: "true"

# List syntax
environment:
  - RACK_ENV=development
  - USER_INPUT                     # Pass-through from host shell
```

### env_file

```yaml
env_file: .env                     # Single file

env_file:                          # Multiple files (later overrides earlier)
  - ./default.env
  - ./override.env

env_file:                          # With options (Compose 2.24.0+)
  - path: ./default.env
    required: false                # Don't error if missing
    format: raw                    # No interpolation (Compose 2.30.0+)
```

#### .env File Parsing Rules

- Format: `VAR[=[VAL]]`
- Comments: lines starting with `#`
- Double-quoted values support interpolation and escape sequences (`\n`, `\r`, `\t`, `\\`)
- Single-quoted values are literal (no interpolation)
- Later files override earlier files for matching keys

---

## Volumes and Storage

### volumes (service-level)

#### Short Syntax: `SOURCE:TARGET[:ACCESS_MODE]`

```yaml
volumes:
  - /host/path:/container/path              # Bind mount
  - volume-name:/data                       # Named volume
  - /host/path:/container/path:ro           # Read-only
  - /host/path:/container/path:rw           # Read-write (default)
```

#### Long Syntax

```yaml
volumes:
  - type: volume                            # Named volume
    source: db-data
    target: /data
    volume:
      nocopy: true                          # Don't copy container data to volume
      subpath: sub                          # Mount subdirectory
  - type: bind                              # Bind mount
    source: /var/run/postgres.sock
    target: /var/run/postgres.sock
    bind:
      propagation: rprivate
      create_host_path: true                # Create path if missing
    read_only: true
  - type: tmpfs                             # Temporary filesystem
    target: /temp
    tmpfs:
      size: 1G
      mode: 0755
```

### tmpfs

```yaml
tmpfs:
  - /data:mode=755,uid=1009,gid=1009
  - /run
```

### volumes_from

```yaml
volumes_from:
  - service_name
  - service_name:ro
  - container:container_name:rw
```

---

## Dependencies and Health

### depends_on

#### Short Syntax

```yaml
depends_on:
  - db
  - redis
```

#### Long Syntax

```yaml
depends_on:
  db:
    condition: service_healthy              # Wait for healthcheck
    restart: true                           # Restart when dependency updates
  redis:
    condition: service_started              # Wait for start only
  migration:
    condition: service_completed_successfully  # Wait for exit code 0
    required: false                         # Warning if service missing
```

### healthcheck

```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost"]       # Exec form
  test: ["CMD-SHELL", "pg_isready -U postgres"]         # Shell form
  test: curl -f http://localhost || exit 1               # String (shell)
  interval: 1m30s                  # Time between checks (default: 30s)
  timeout: 10s                     # Max time for check (default: 30s)
  retries: 3                       # Failures before unhealthy (default: 3)
  start_period: 40s                # Grace period at startup (default: 0s)
  start_interval: 5s               # Interval during start_period (default: 5s)
```

Disable inherited healthcheck: `test: NONE`

---

## Deploy

```yaml
deploy:
  mode: replicated                 # replicated (default), global
  replicas: 6
  resources:
    limits:
      cpus: '0.50'
      memory: 50M
      pids: 1
    reservations:
      cpus: '0.25'
      memory: 20M
      devices:
        - capabilities: ["gpu"]    # or "nvidia-compute"
          driver: nvidia
          count: 2                 # or "all"
          # device_ids: ["0", "1"]  # Mutually exclusive with count
  restart_policy:
    condition: on-failure          # none, on-failure, any (default)
    delay: 5s                      # Default: 0
    max_attempts: 3                # Default: unlimited
    window: 120s                   # Default: immediate
  placement:
    constraints:
      - node.labels.disktype==ssd
    preferences:
      - spread: node.labels.zone
  update_config:
    parallelism: 2
    delay: 10s
    failure_action: pause          # continue, rollback, pause (default)
    monitor: 30s
    max_failure_ratio: 0.1
    order: stop-first              # stop-first (default), start-first
  rollback_config:
    parallelism: 0                 # 0 = all at once
    delay: 0s
    failure_action: pause
    monitor: 0s
    max_failure_ratio: 0
    order: stop-first
  endpoint_mode: vip               # vip (virtual IP), dnsrr (round-robin DNS)
  labels:
    com.example.description: "Service label (not container)"
```

---

## Restart Policy (Service-Level)

```yaml
restart: "no"                      # ALWAYS quote -- unquoted no = YAML false
restart: always
restart: on-failure
restart: on-failure:3              # With max retries
restart: unless-stopped
```

---

## Logging

```yaml
logging:
  driver: syslog                   # json-file (default), syslog, journald, etc.
  options:
    syslog-address: "tcp://192.168.0.42:123"
    max-size: "10m"
    max-file: "3"
```

---

## Labels and Annotations

```yaml
labels:
  com.example.description: "Accounting webapp"
  com.example.department: "Finance"
# or list syntax:
labels:
  - "com.example.description=Accounting webapp"

label_file:
  - ./app.labels

annotations:
  com.example.foo: bar
```

---

## Container Identity

```yaml
container_name: my-web-container   # Prevents scaling
hostname: my-host                  # RFC 1123 compliant
domainname: example.com
```

---

## User and Working Directory

```yaml
user: "1000:1000"
working_dir: /app
```

---

## Terminal and Init

```yaml
stdin_open: true                   # Equivalent to -i flag
tty: true                          # Allocate pseudo-TTY
init: true                         # Run init process for signal forwarding
```

---

## Security

```yaml
privileged: true                   # Full host access (AVOID in production)

cap_add:
  - ALL
cap_drop:
  - NET_ADMIN
  - SYS_ADMIN

security_opt:
  - label=user:USER
  - label=role:ROLE
  - no-new-privileges:true

group_add:
  - mail
  - root

read_only: true                    # Read-only root filesystem
```

---

## Sysctls, Ulimits, and Shared Memory

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

---

## Process Namespacing

```yaml
pid: "host"
ipc: "shareable"                   # or "service:other_service"
uts: "host"
userns_mode: "host"
cgroup: "host"                     # or "private"
cgroup_parent: /custom/cgroup
pids_limit: 100                    # -1 for unlimited
```

---

## Devices

```yaml
devices:
  - "/dev/ttyUSB0:/dev/ttyUSB0"
  - "/dev/sda:/dev/xvda:rwm"

device_cgroup_rules:
  - 'c 1:3 mr'
  - 'a 7:* rmw'

gpus:
  - driver: nvidia
    count: 2
# or
gpus: all
```

---

## Profiles

```yaml
profiles: ["frontend", "debug"]
```

Valid names: `[a-zA-Z0-9][a-zA-Z0-9_.-]+`. Services without profiles are ALWAYS enabled.

---

## Extends

```yaml
extends:
  file: common.yml                 # From another file
  service: webapp

# Within same file
extends: webapp
```

---

## Platform and Pull Policy

```yaml
platform: linux/arm64/v8           # Format: os[/arch[/variant]]

pull_policy: always                # always, never, missing (default), build
                                   # daily, weekly, every_<duration>
```

---

## Stop and Lifecycle

```yaml
stop_signal: SIGUSR1               # Default: SIGTERM
stop_grace_period: 1m30s           # Grace period before SIGKILL

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

---

## Secrets and Configs (Service-Level)

### secrets

```yaml
secrets:
  - server-certificate             # Short syntax

secrets:
  - source: server-certificate     # Long syntax
    target: server.cert
    uid: "103"
    gid: "103"
    mode: 0o440
```

### configs

Same syntax as secrets -- short form (name only) or long form with `source`, `target`, `uid`, `gid`, `mode`.

---

## Miscellaneous Attributes

| Attribute | Type | Purpose |
|-----------|------|---------|
| `attach` | bool | Control log collection (default `true`) |
| `runtime` | string | OCI runtime (e.g., `runc`) |
| `scale` | int | Default container count |
| `isolation` | string | Container isolation technology |
| `mac_address` | string | MAC address assignment |
| `storage_opt` | map | Storage driver options |
| `credential_spec` | string | Windows credential spec |
| `use_api_socket` | bool | Access container engine API |

---

## Official Sources

- https://docs.docker.com/compose/compose-file/05-services/
- https://docs.docker.com/compose/compose-file/build/
- https://docs.docker.com/compose/compose-file/deploy/
