---
name: docker-syntax-compose-resources
description: >
  Use when defining top-level networks, volumes, configs, or secrets in
  compose.yaml. Prevents misconfigured network drivers, orphaned volumes,
  and secrets mounted with wrong permissions.
  Covers networks, volumes, configs, secrets, IPAM, external resources,
  network aliases, volume drivers, and overlay networks.
  Keywords: docker compose, networks, volumes, configs, secrets, IPAM,
  overlay, bridge, shared network, shared volume, compose resources,
  service communication.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Compose v2."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-syntax-compose-resources

## Quick Reference

### Top-Level Resource Elements

| Element | Purpose | Default Behavior |
|---------|---------|-----------------|
| `networks` | Define named networks for service communication | Compose creates implicit `default` network |
| `volumes` | Define named volumes for persistent data | Created on `docker compose up` if missing |
| `configs` | Define non-sensitive configuration data | Mounted at `/<config-name>` with mode `0444` |
| `secrets` | Define sensitive data (passwords, certificates) | Mounted at `/run/secrets/<secret-name>` |

### Network Driver Comparison

| Driver | Scope | Use Case | Multi-Host |
|--------|-------|----------|-----------|
| `bridge` | Single host | Default. Isolated network between containers | No |
| `host` | Single host | Container shares host network stack directly | No |
| `overlay` | Multi-host | Swarm service communication across nodes | Yes |
| `macvlan` | Single host | Container gets own MAC address on physical network | No |
| `none` | Single host | Completely disable networking | No |

### Config vs Secret Comparison

| Attribute | Config | Secret |
|-----------|--------|--------|
| Purpose | Non-sensitive configuration | Sensitive credentials |
| Default mount path | `/<config-name>` | `/run/secrets/<secret-name>` |
| Default permissions | `0444` (world-readable) | `0444` (world-readable) |
| Source options | `file`, `environment`, `content` | `file`, `environment` |
| Customizable mount | Yes (`target`, `uid`, `gid`, `mode`) | Yes (`target`, `uid`, `gid`, `mode`) |
| `content` inline | Yes (Compose 2.23.1+) | No |

### Critical Warnings

**NEVER** use anonymous volumes for data that must persist -- anonymous volumes are recreated on `docker compose down` and all data is lost. ALWAYS define named volumes in the top-level `volumes` section.

**NEVER** omit the top-level declaration for a named volume, config, or secret -- referencing an undeclared resource in a service causes a Compose validation error. ALWAYS declare every resource at the top level.

**NEVER** set `external: true` on a resource without ensuring it exists before running `docker compose up` -- Compose does NOT create external resources and errors immediately if they are missing.

**NEVER** combine `external: true` with `driver`, `driver_opts`, `file`, `content`, or other creation attributes -- when `external` is set, only `name` is relevant alongside it. Compose rejects files with additional fields on external resources.

**ALWAYS** use reverse-DNS notation for resource labels (e.g., `com.example.description`) -- this prevents naming collisions with labels from other tools.

---

## Decision Trees

### Which Resource Type to Use

```
Need to store data persistently across container restarts?
├─ Yes → Use a VOLUME (top-level `volumes`)
│   ├─ Data owned by this Compose project? → Define normally
│   └─ Data shared across projects? → Use `external: true`
└─ No → Need to inject file-based configuration?
    ├─ Contains sensitive data (passwords, keys, certs)?
    │   └─ Yes → Use a SECRET (top-level `secrets`)
    └─ Non-sensitive configuration?
        └─ Yes → Use a CONFIG (top-level `configs`)
```

### Which Network Driver to Use

```
Need containers to communicate?
├─ Single Docker host?
│   ├─ Standard container isolation → driver: bridge (default)
│   ├─ Container needs host network performance → driver: host
│   └─ Container needs own MAC on physical LAN → driver: macvlan
├─ Multiple Docker hosts (Swarm)?
│   └─ driver: overlay
└─ Container must have no network access?
    └─ driver: none
```

### External vs Managed Resources

```
Is the resource created outside this Compose project?
├─ Yes → external: true
│   ├─ Name matches Compose key? → Just set external: true
│   └─ Different name? → Add name: "actual-name"
└─ No → Let Compose manage creation and lifecycle
    ├─ Need custom driver? → Set driver + driver_opts
    ├─ Need custom subnet? → Set ipam.config
    └─ Default behavior sufficient? → Declare with empty body
```

---

## Top-Level Networks

### Basic Network Definition

```yaml
networks:
  frontend:
  backend:
    driver: bridge
```

An empty declaration uses the default `bridge` driver.

### Network with IPAM Configuration

```yaml
networks:
  app-net:
    driver: bridge
    ipam:
      driver: default
      config:
        - subnet: 172.28.0.0/16
          ip_range: 172.28.5.0/24
          gateway: 172.28.5.254
          aux_addresses:
            host1: 172.28.1.5
```

### Network Attributes

```yaml
networks:
  internal-net:
    internal: true        # Externally isolated -- no internet access
    attachable: true       # Standalone containers can attach
    enable_ipv6: true      # Enable IPv6
    labels:
      com.example.project: "myapp"
    name: "custom-net-name"
    driver_opts:
      com.docker.network.bridge.host_binding_ipv4: "127.0.0.1"
```

### External Network

```yaml
networks:
  shared:
    external: true
    name: "${NETWORK_ID}"    # Variable interpolation supported
```

### Customizing the Default Network

```yaml
networks:
  default:
    name: my-app-network
    driver: bridge
    driver_opts:
      com.docker.network.bridge.host_binding_ipv4: "127.0.0.1"
```

### Service-Level Network Configuration

```yaml
services:
  app:
    networks:
      backend:
        aliases:
          - app-alias
          - api
        ipv4_address: 172.16.238.10
        ipv6_address: 2001:3984:3989::10
        priority: 1000
```

---

## Top-Level Volumes

### Basic Volume Definition

```yaml
volumes:
  db-data:
  cache:
    driver: local
```

### Volume with NFS Driver

```yaml
volumes:
  nfs-data:
    driver_opts:
      type: "nfs"
      o: "addr=10.40.0.199,nolock,soft,rw"
      device: ":/docker/example"
```

### External Volume

```yaml
volumes:
  shared-data:
    external: true
    name: actual-volume-name
```

### Volume with Labels

```yaml
volumes:
  db-data:
    labels:
      com.example.description: "Database volume"
      com.example.department: "IT/Ops"
    name: "${DATABASE_VOLUME}"
```

---

## Top-Level Configs and Secrets

### Config Sources

```yaml
configs:
  from-file:
    file: ./httpd.conf
  from-env:
    environment: "CONFIG_VALUE"       # Compose 2.23.1+
  from-inline:
    content: |                         # Compose 2.23.1+
      debug=${DEBUG}
      app.name=${COMPOSE_PROJECT_NAME}
  from-external:
    external: true
    name: "${HTTP_CONFIG_KEY}"
```

### Secret Sources

```yaml
secrets:
  from-file:
    file: ./server.cert
  from-env:
    environment: "OAUTH_TOKEN"
  from-external:
    external: true
    name: "${SECRET_KEY}"
```

### Mounting in Services

```yaml
services:
  web:
    configs:
      - from-file                            # Short: mounts at /<config-name>
      - source: from-inline
        target: /etc/app/config.properties
        uid: "1000"
        gid: "1000"
        mode: 0440
    secrets:
      - from-file                            # Short: mounts at /run/secrets/<name>
      - source: from-env
        target: oauth-token
        uid: "103"
        gid: "103"
        mode: 0440
```

---

## Network Isolation Pattern

```yaml
services:
  proxy:
    image: nginx
    networks:
      - frontend
  app:
    image: myapp
    networks:
      - frontend
      - backend
  db:
    image: postgres
    networks:
      - backend

networks:
  frontend:
  backend:
    internal: true    # No external access for database network
```

In this pattern, `proxy` CANNOT reach `db` -- only `app` bridges both networks. Setting `internal: true` on `backend` prevents containers on that network from reaching the internet.

---

## Complete Resource Example

```yaml
services:
  web:
    image: nginx
    configs:
      - source: nginx-config
        target: /etc/nginx/nginx.conf
        mode: 0440
    secrets:
      - tls-cert
      - source: tls-key
        target: /etc/ssl/private/server.key
        mode: 0400
    volumes:
      - static-files:/usr/share/nginx/html:ro
    networks:
      frontend:
        aliases:
          - webserver

  app:
    image: myapp
    volumes:
      - app-data:/data
    networks:
      - frontend
      - backend

  db:
    image: postgres
    volumes:
      - db-data:/var/lib/postgresql/data
    secrets:
      - db-password
    networks:
      - backend

networks:
  frontend:
    driver: bridge
  backend:
    driver: bridge
    internal: true

volumes:
  db-data:
  app-data:
  static-files:

configs:
  nginx-config:
    file: ./nginx/nginx.conf

secrets:
  tls-cert:
    file: ./certs/server.crt
  tls-key:
    file: ./certs/server.key
  db-password:
    environment: POSTGRES_PASSWORD
```

---

## Reference Links

- [references/networks.md](references/networks.md) -- All network options, drivers, and IPAM configuration
- [references/volumes-configs-secrets.md](references/volumes-configs-secrets.md) -- Volume, config, and secret definitions and mounting
- [references/anti-patterns.md](references/anti-patterns.md) -- Resource configuration mistakes and corrections

### Official Sources

- https://docs.docker.com/compose/compose-file/06-networks/
- https://docs.docker.com/compose/compose-file/07-volumes/
- https://docs.docker.com/compose/compose-file/08-configs/
- https://docs.docker.com/compose/compose-file/09-secrets/
