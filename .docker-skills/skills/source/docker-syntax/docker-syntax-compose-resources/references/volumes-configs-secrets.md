# Volumes, Configs, and Secrets Reference

## Top-Level Volumes

### Volume Attributes

| Attribute | Type | Default | Description |
|-----------|------|---------|-------------|
| `driver` | string | `local` | Volume driver |
| `driver_opts` | map | — | Driver-specific options as key-value pairs |
| `external` | boolean | `false` | Volume exists outside Compose lifecycle |
| `labels` | map/list | — | Metadata labels (reverse-DNS notation recommended) |
| `name` | string | `<project>_<key>` | Custom volume name (supports interpolation) |

### Basic Named Volume

```yaml
volumes:
  db-data:
```

An empty declaration creates a volume using the default `local` driver. The volume persists across `docker compose down` and is reused on subsequent `docker compose up`.

**ALWAYS** declare named volumes at the top level -- referencing an undeclared volume name in a service causes a validation error.

### Volume with Custom Driver

```yaml
volumes:
  db-data:
    driver: local
    driver_opts:
      type: "none"
      o: "bind"
      device: "/data/db"
```

### NFS Volume

```yaml
volumes:
  nfs-data:
    driver_opts:
      type: "nfs"
      o: "addr=10.40.0.199,nolock,soft,rw"
      device: ":/docker/example"
```

### CIFS/SMB Volume

```yaml
volumes:
  smb-data:
    driver_opts:
      type: "cifs"
      o: "addr=10.40.0.199,username=user,password=pass"
      device: "//10.40.0.199/share"
```

### tmpfs Volume

```yaml
volumes:
  tmp-data:
    driver_opts:
      type: "tmpfs"
      device: "tmpfs"
      o: "size=100m,uid=1000"
```

### External Volume

```yaml
volumes:
  shared-data:
    external: true

  # With custom name
  db-data:
    external: true
    name: actual-volume-name

  # With variable interpolation
  dynamic-vol:
    external: true
    name: "${VOLUME_NAME}"
```

**NEVER** use `driver`, `driver_opts`, or `labels` alongside `external: true` -- only `name` is valid with external volumes.

**ALWAYS** create external volumes before running Compose:
```bash
docker volume create actual-volume-name
```

### Volume Labels

```yaml
volumes:
  db-data:
    labels:
      com.example.description: "Database volume"
      com.example.department: "IT/Ops"
      com.example.backup: "daily"
```

Labels apply to named volumes ONLY, NOT to bind mounts. Visible via `docker volume inspect`.

### Custom Volume Name

```yaml
volumes:
  db-data:
    name: "my-app-data"

  # With interpolation
  cache:
    name: "${PROJECT_NAME}_cache"
```

## Service-Level Volume Mounting

### Short Syntax

Format: `[SOURCE:]TARGET[:ACCESS_MODE]`

```yaml
services:
  app:
    volumes:
      - db-data:/var/lib/postgresql/data        # Named volume
      - /host/path:/container/path               # Bind mount
      - ./relative:/container/path               # Relative bind mount
      - ~/home/path:/container/path              # Home directory
      - /container/anonymous                      # Anonymous volume
      - db-data:/data:ro                          # Read-only
      - db-data:/data:rw                          # Read-write (default)
```

### Long Syntax

```yaml
services:
  app:
    volumes:
      # Named volume
      - type: volume
        source: db-data
        target: /var/lib/postgresql/data
        volume:
          nocopy: true       # Do not copy data from container on creation
          subpath: sub       # Mount a subdirectory of the volume

      # Bind mount
      - type: bind
        source: /host/data
        target: /container/data
        read_only: true
        bind:
          propagation: rprivate       # Mount propagation
          create_host_path: true      # Create host path if missing
          selinux: z                  # SELinux relabeling (z=shared, Z=private)

      # tmpfs mount
      - type: tmpfs
        target: /tmp
        tmpfs:
          size: 1073741824    # 1GB in bytes
          mode: 0755

      # Named pipe (Windows)
      - type: npipe
        source: \\.\pipe\docker_engine
        target: \\.\pipe\docker_engine
```

### Volume Mount Attributes (Long Syntax)

| Attribute | Type | Description |
|-----------|------|-------------|
| `type` | string | `volume`, `bind`, `tmpfs`, `npipe`, `cluster` |
| `source` | string | Volume name or host path |
| `target` | string | Container mount path |
| `read_only` | boolean | Mount as read-only |
| `volume.nocopy` | boolean | Do not copy container data on first mount |
| `volume.subpath` | string | Mount subdirectory of a volume |
| `bind.propagation` | string | `rprivate`, `private`, `rshared`, `shared`, `rslave`, `slave` |
| `bind.create_host_path` | boolean | Create host path if it does not exist |
| `bind.selinux` | string | `z` (shared) or `Z` (private) SELinux relabeling |
| `tmpfs.size` | integer | Size in bytes |
| `tmpfs.mode` | integer | File mode (e.g., `0755`) |

### volumes_from

Mount all volumes from another service or container:

```yaml
services:
  backup:
    volumes_from:
      - db                        # All volumes from db service
      - db:ro                     # Read-only
      - container:legacy_db:rw    # From external container
```

### Sharing Volumes Between Services

```yaml
services:
  writer:
    image: myapp
    volumes:
      - shared-data:/data

  reader:
    image: backup
    volumes:
      - shared-data:/var/lib/backup/data:ro

volumes:
  shared-data:
```

**ALWAYS** use `:ro` (read-only) on the consumer service when it only needs to read the data -- this prevents accidental writes.

---

## Top-Level Configs

### Config Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `file` | string | Path to file containing config data |
| `environment` | string | Environment variable name holding config value (Compose 2.23.1+) |
| `content` | string | Inline config content with interpolation (Compose 2.23.1+) |
| `external` | boolean | Config exists outside Compose lifecycle |
| `name` | string | Custom config name in the platform |

### Config Sources

Only ONE source attribute (`file`, `environment`, `content`, or `external`) can be used per config definition.

#### File-Based Config

```yaml
configs:
  nginx-config:
    file: ./nginx/nginx.conf
```

#### Environment-Based Config (Compose 2.23.1+)

```yaml
configs:
  app-config:
    environment: "APP_CONFIG_JSON"
```

The value of the environment variable becomes the config content.

#### Inline Content Config (Compose 2.23.1+)

```yaml
configs:
  app-properties:
    content: |
      debug=${DEBUG}
      spring.application.name=${COMPOSE_PROJECT_NAME}
      server.port=8080
```

Supports Compose variable interpolation within the content.

#### External Config

```yaml
configs:
  shared-config:
    external: true
    name: "${HTTP_CONFIG_KEY}"
```

### Mounting Configs in Services

#### Short Syntax

```yaml
services:
  web:
    configs:
      - nginx-config        # Mounts at /<config-name>
      - app-properties       # Mounts at /app-properties
```

#### Long Syntax

```yaml
services:
  web:
    configs:
      - source: nginx-config
        target: /etc/nginx/nginx.conf
        uid: "1000"
        gid: "1000"
        mode: 0440
```

#### Config Mount Attributes

| Attribute | Type | Default | Description |
|-----------|------|---------|-------------|
| `source` | string | — | Config name as declared in top-level `configs` |
| `target` | string | `/<config-name>` | Mount path inside the container |
| `uid` | string | Container command user | File owner UID |
| `gid` | string | Container command user | File group GID |
| `mode` | integer | `0444` | File permissions |

---

## Top-Level Secrets

### Secret Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `file` | string | Path to file containing secret data |
| `environment` | string | Environment variable name holding secret value |
| `external` | boolean | Secret exists outside Compose lifecycle |
| `name` | string | Custom secret name in the platform |

### Secret Sources

Only ONE source attribute (`file`, `environment`, or `external`) can be used per secret definition.

#### File-Based Secret

```yaml
secrets:
  server-cert:
    file: ./certs/server.crt
  server-key:
    file: ./certs/server.key
  db-password:
    file: ./secrets/db_password.txt
```

#### Environment-Based Secret

```yaml
secrets:
  oauth-token:
    environment: "OAUTH_TOKEN"
  api-key:
    environment: "API_KEY"
```

**ALWAYS** prefer `environment` source for CI/CD pipelines where secrets are injected as environment variables. **ALWAYS** prefer `file` source for local development with secret files excluded from version control.

#### External Secret

```yaml
secrets:
  production-cert:
    external: true
    name: "${CERT_SECRET_NAME}"
```

### Mounting Secrets in Services

#### Short Syntax

```yaml
services:
  web:
    secrets:
      - server-cert        # Mounts at /run/secrets/server-cert
      - db-password          # Mounts at /run/secrets/db-password
```

#### Long Syntax

```yaml
services:
  web:
    secrets:
      - source: server-cert
        target: /etc/ssl/certs/server.crt
        uid: "103"
        gid: "103"
        mode: 0440
      - source: server-key
        target: /etc/ssl/private/server.key
        uid: "103"
        gid: "103"
        mode: 0400
```

#### Secret Mount Attributes

| Attribute | Type | Default | Description |
|-----------|------|---------|-------------|
| `source` | string | — | Secret name as declared in top-level `secrets` |
| `target` | string | `/run/secrets/<name>` | Mount path inside the container |
| `uid` | string | Container command user | File owner UID |
| `gid` | string | Container command user | File group GID |
| `mode` | integer | `0444` | File permissions |

**ALWAYS** set restrictive `mode` on secret files (e.g., `0400` or `0440`) -- the default `0444` is world-readable inside the container.

---

## Resource Naming Conventions

### Default Naming

Compose prefixes resource names with the project name:

| Resource | Default Name Pattern |
|----------|---------------------|
| Network | `<project>_<network-key>` |
| Volume | `<project>_<volume-key>` |
| Config | `<project>_<config-key>` |
| Secret | `<project>_<secret-key>` |

### Custom Naming with `name`

```yaml
networks:
  app-net:
    name: "production-network"       # Exact name, no project prefix

volumes:
  db-data:
    name: "postgres-data-v2"         # Exact name, no project prefix

configs:
  app-config:
    name: "app-config-${ENV}"        # Interpolation supported

secrets:
  tls-cert:
    name: "tls-cert-${DOMAIN}"       # Interpolation supported
```

**ALWAYS** use the `name` attribute when resources must have predictable names (e.g., for external references from other projects or scripts).

## Official Sources

- https://docs.docker.com/compose/compose-file/07-volumes/
- https://docs.docker.com/compose/compose-file/08-configs/
- https://docs.docker.com/compose/compose-file/09-secrets/
- https://docs.docker.com/compose/compose-file/05-services/
