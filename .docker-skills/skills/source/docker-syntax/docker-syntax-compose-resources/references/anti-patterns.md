# Anti-Patterns: Resource Configuration Mistakes

## Network Anti-Patterns

### Using flat networking (single default network for everything)

```yaml
# WRONG -- all services on one network, no isolation
services:
  proxy:
    image: nginx
  app:
    image: myapp
  db:
    image: postgres
  redis:
    image: redis
```

```yaml
# CORRECT -- network segmentation isolates database tier
services:
  proxy:
    image: nginx
    networks: [frontend]
  app:
    image: myapp
    networks: [frontend, backend]
  db:
    image: postgres
    networks: [backend]
  redis:
    image: redis
    networks: [backend]

networks:
  frontend:
  backend:
    internal: true
```

**Why**: Without explicit network segmentation, every service can communicate with every other service. A compromised proxy container can directly access the database. ALWAYS use separate networks to enforce least-privilege communication.

### Assigning static IPs without IPAM configuration

```yaml
# WRONG -- static IP without subnet definition
services:
  dns:
    networks:
      infra:
        ipv4_address: 172.20.0.53

networks:
  infra:
```

```yaml
# CORRECT -- IPAM subnet defined for static IP range
services:
  dns:
    networks:
      infra:
        ipv4_address: 172.20.0.53

networks:
  infra:
    ipam:
      config:
        - subnet: 172.20.0.0/16
```

**Why**: Without IPAM configuration, Compose cannot validate the static IP or ensure it falls within an allocatable range. ALWAYS define `ipam.config` with a `subnet` when using static IP addresses.

### Combining network_mode with networks

```yaml
# WRONG -- network_mode and networks are mutually exclusive
services:
  monitor:
    network_mode: host
    networks:
      - monitoring
```

```yaml
# CORRECT -- use one or the other
services:
  monitor:
    network_mode: host
```

**Why**: `network_mode` and `networks` are mutually exclusive. Using both causes a Compose validation error. NEVER combine them in the same service.

### Not marking backend networks as internal

```yaml
# WRONG -- database network has internet access
networks:
  backend:
    driver: bridge
```

```yaml
# CORRECT -- internal network prevents internet access
networks:
  backend:
    driver: bridge
    internal: true
```

**Why**: Without `internal: true`, containers on the network can reach the internet. Database and cache containers NEVER need internet access. ALWAYS set `internal: true` on networks that should not have external connectivity.

---

## Volume Anti-Patterns

### Using anonymous volumes for persistent data

```yaml
# WRONG -- anonymous volume, data lost on docker compose down
services:
  db:
    image: postgres
    volumes:
      - /var/lib/postgresql/data
```

```yaml
# CORRECT -- named volume persists across lifecycle operations
services:
  db:
    image: postgres
    volumes:
      - db-data:/var/lib/postgresql/data

volumes:
  db-data:
```

**Why**: Anonymous volumes are NOT preserved by `docker compose down`. Named volumes persist across container recreation. ALWAYS use named volumes for any data that must survive container lifecycle operations.

### Referencing undeclared named volumes

```yaml
# WRONG -- volume used in service but not declared at top level
services:
  db:
    volumes:
      - db-data:/var/lib/postgresql/data
```

```yaml
# CORRECT -- volume declared at top level
services:
  db:
    volumes:
      - db-data:/var/lib/postgresql/data

volumes:
  db-data:
```

**Why**: Compose requires every named volume used in a service to be declared in the top-level `volumes` section. Omitting the declaration causes a validation error. ALWAYS declare every named volume.

### Using bind mounts for database storage in production

```yaml
# WRONG -- bind mount ties data to specific host path
services:
  db:
    image: postgres
    volumes:
      - ./data/postgres:/var/lib/postgresql/data
```

```yaml
# CORRECT -- named volume with appropriate driver
services:
  db:
    image: postgres
    volumes:
      - db-data:/var/lib/postgresql/data

volumes:
  db-data:
```

**Why**: Bind mounts create tight coupling to the host filesystem, have permission issues across platforms, and do not benefit from Docker's volume driver capabilities (snapshots, replication, backup). ALWAYS use named volumes for database storage. Reserve bind mounts for development-time source code mounting.

### Not using read-only mounts where appropriate

```yaml
# WRONG -- backup service has write access to source data
services:
  backup:
    volumes:
      - app-data:/data
```

```yaml
# CORRECT -- read-only mount prevents accidental writes
services:
  backup:
    volumes:
      - app-data:/data:ro
```

**Why**: Without `:ro`, every mounted service has full write access. A bug in the backup service could corrupt the data. ALWAYS mount volumes as read-only when the service only needs to read.

---

## Config Anti-Patterns

### Using environment variables for multi-line configuration

```yaml
# WRONG -- complex config crammed into environment variable
services:
  web:
    environment:
      NGINX_CONFIG: |
        server {
          listen 80;
          location / { proxy_pass http://app:8080; }
        }
```

```yaml
# CORRECT -- use a config for structured configuration files
services:
  web:
    configs:
      - source: nginx-config
        target: /etc/nginx/conf.d/default.conf

configs:
  nginx-config:
    file: ./nginx/default.conf
```

**Why**: Environment variables are designed for simple key-value pairs, not structured file content. Multi-line values in environment variables are fragile, hard to debug, and do not support proper file permissions. ALWAYS use configs for configuration files.

### Not specifying target path for configs

```yaml
# WRONG -- config mounted at root as /<config-name>
services:
  web:
    configs:
      - nginx-config
```

```yaml
# CORRECT -- explicit target path where application expects it
services:
  web:
    configs:
      - source: nginx-config
        target: /etc/nginx/nginx.conf
        mode: 0440
```

**Why**: The default mount path (`/<config-name>`) is rarely where an application expects its configuration file. ALWAYS specify `target` to mount the config at the correct application path.

### Combining mutually exclusive config sources

```yaml
# WRONG -- file and content cannot coexist
configs:
  app-config:
    file: ./config.json
    content: |
      {"key": "value"}
```

```yaml
# CORRECT -- use only one source per config
configs:
  app-config:
    file: ./config.json

  inline-config:
    content: |
      {"key": "value"}
```

**Why**: Each config definition accepts exactly ONE source: `file`, `environment`, `content`, or `external`. Specifying multiple sources causes a validation error. ALWAYS use a single source per config.

---

## Secret Anti-Patterns

### Using environment variables for sensitive data

```yaml
# WRONG -- password visible in docker inspect, logs, process listing
services:
  db:
    environment:
      POSTGRES_PASSWORD: "super-secret-password"
```

```yaml
# CORRECT -- secret mounted as file, not visible in inspect/logs
services:
  db:
    environment:
      POSTGRES_PASSWORD_FILE: /run/secrets/db-password
    secrets:
      - db-password

secrets:
  db-password:
    file: ./secrets/db_password.txt
```

**Why**: Environment variables are visible via `docker inspect`, process listings, and debug logs. Secrets are mounted as files with controlled permissions and are NOT exposed through container metadata. ALWAYS use secrets for passwords, API keys, certificates, and tokens.

### Using default permissions on secret files

```yaml
# WRONG -- default 0444 is world-readable inside container
services:
  web:
    secrets:
      - source: tls-key
        target: /etc/ssl/private/server.key
```

```yaml
# CORRECT -- restrictive permissions on private key
services:
  web:
    secrets:
      - source: tls-key
        target: /etc/ssl/private/server.key
        uid: "103"
        gid: "103"
        mode: 0400
```

**Why**: The default mode `0444` means any process in the container can read the secret. For private keys and credentials, ALWAYS set `mode: 0400` (owner-read-only) or `mode: 0440` (owner+group read) with explicit `uid`/`gid`.

### Committing secret files to version control

```yaml
# WRONG -- secret file tracked in Git
secrets:
  db-password:
    file: ./db_password.txt    # This file is committed to the repo!
```

```yaml
# CORRECT -- use environment source in CI/CD
secrets:
  db-password:
    environment: "DB_PASSWORD"

# OR use file source with proper .gitignore
secrets:
  db-password:
    file: ./secrets/db_password.txt    # ./secrets/ in .gitignore
```

**Why**: Secret files committed to version control are permanently exposed in Git history. ALWAYS add secret file paths to `.gitignore`. In CI/CD pipelines, ALWAYS use the `environment` source to inject secrets from the pipeline's secret management system.

---

## External Resource Anti-Patterns

### Adding creation attributes to external resources

```yaml
# WRONG -- driver_opts ignored on external volumes
volumes:
  shared-data:
    external: true
    driver: local
    driver_opts:
      type: nfs
      o: "addr=10.0.0.1"
```

```yaml
# CORRECT -- only name is valid with external
volumes:
  shared-data:
    external: true
    name: "nfs-shared-data"
```

**Why**: When `external: true` is set, Compose does NOT create the resource. All creation attributes (`driver`, `driver_opts`, `labels`, `ipam`, `file`, `content`) are ignored or rejected. ALWAYS configure external resources outside of Compose and reference them by name only.

### Not validating external resources before deployment

```yaml
# WRONG -- Compose fails at startup if resource is missing
volumes:
  shared-data:
    external: true
```

```yaml
# CORRECT -- create external resources before running Compose
# Pre-deployment script:
# docker volume create shared-data
# docker network create shared-net

volumes:
  shared-data:
    external: true
```

**Why**: Compose does NOT create external resources and errors immediately if they are missing. ALWAYS ensure external networks, volumes, configs, and secrets exist before running `docker compose up`. Include resource creation in deployment scripts or infrastructure-as-code.

---

## General Resource Anti-Patterns

### Not using labels for resource management

```yaml
# WRONG -- no labels, difficult to identify and manage
volumes:
  db-data:
  cache-data:
  uploads:

networks:
  frontend:
  backend:
```

```yaml
# CORRECT -- labeled for identification and automation
volumes:
  db-data:
    labels:
      com.example.project: "myapp"
      com.example.component: "database"
      com.example.backup: "daily"
  cache-data:
    labels:
      com.example.project: "myapp"
      com.example.component: "cache"
      com.example.backup: "none"

networks:
  frontend:
    labels:
      com.example.project: "myapp"
      com.example.tier: "public"
  backend:
    labels:
      com.example.project: "myapp"
      com.example.tier: "private"
```

**Why**: Without labels, resources from different projects become indistinguishable. Labels enable filtering (`docker volume ls --filter label=com.example.backup=daily`), automated management, and documentation. ALWAYS use reverse-DNS labeled resources in production.

### Using project-prefixed names when sharing across projects

```yaml
# WRONG -- each project creates its own copy
# Project A:
volumes:
  shared-data:     # Creates projecta_shared-data

# Project B:
volumes:
  shared-data:     # Creates projectb_shared-data (different volume!)
```

```yaml
# CORRECT -- use name or external for cross-project sharing
# Project A:
volumes:
  shared-data:
    name: "global-shared-data"

# Project B:
volumes:
  shared-data:
    external: true
    name: "global-shared-data"
```

**Why**: Compose prefixes resource names with the project name by default. Two projects declaring the same volume key get separate volumes. ALWAYS use `name` or `external: true` with `name` when resources must be shared across Compose projects.

## Official Sources

- https://docs.docker.com/compose/compose-file/06-networks/
- https://docs.docker.com/compose/compose-file/07-volumes/
- https://docs.docker.com/compose/compose-file/08-configs/
- https://docs.docker.com/compose/compose-file/09-secrets/
