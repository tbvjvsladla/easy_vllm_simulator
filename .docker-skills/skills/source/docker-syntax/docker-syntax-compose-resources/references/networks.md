# Networks Reference

## Top-Level Network Attributes

| Attribute | Type | Default | Description |
|-----------|------|---------|-------------|
| `driver` | string | `bridge` | Network driver: `bridge`, `host`, `overlay`, `macvlan`, `none` |
| `driver_opts` | map | — | Driver-specific options as key-value pairs |
| `ipam` | object | — | IP Address Management configuration |
| `external` | boolean | `false` | Network exists outside Compose lifecycle |
| `internal` | boolean | `false` | Restrict external access (no internet) |
| `attachable` | boolean | `false` | Allow standalone containers to attach |
| `enable_ipv4` | boolean | `true` | Enable IPv4 networking |
| `enable_ipv6` | boolean | `false` | Enable IPv6 networking |
| `labels` | map/list | — | Metadata labels (reverse-DNS notation recommended) |
| `name` | string | `<project>_<key>` | Custom network name (supports interpolation) |

## Network Drivers

### bridge (Default)

Creates an isolated network on a single Docker host. Containers on the same bridge network can communicate by service name. This is the default driver when no driver is specified.

```yaml
networks:
  app-net:
    driver: bridge
    driver_opts:
      com.docker.network.bridge.host_binding_ipv4: "127.0.0.1"
      com.docker.network.bridge.enable_icc: "true"
      com.docker.network.bridge.enable_ip_masquerade: "true"
      com.docker.network.bridge.name: "br-custom"
      com.docker.network.driver.mtu: "1500"
```

**ALWAYS** use `bridge` for single-host development and production setups where containers run on the same machine.

### host

Removes network isolation -- the container shares the host's networking namespace directly. No port mapping is needed or possible.

```yaml
networks:
  hostnet:
    driver: host
```

**ALWAYS** use `host` only when container needs maximum network performance or must bind to host ports directly. **NEVER** use `host` driver when network isolation between containers is required.

### overlay

Enables multi-host networking for Docker Swarm services. Containers on different hosts can communicate as if on the same network.

```yaml
networks:
  swarm-net:
    driver: overlay
    attachable: true    # Allow non-Swarm containers to attach
```

**ALWAYS** use `overlay` when services span multiple Docker hosts in a Swarm cluster.

### macvlan

Assigns a MAC address to each container, making it appear as a physical device on the network. Containers get their own IP on the physical network.

```yaml
networks:
  physical-net:
    driver: macvlan
    driver_opts:
      parent: eth0
    ipam:
      config:
        - subnet: 192.168.1.0/24
          gateway: 192.168.1.1
```

**ALWAYS** use `macvlan` when containers need to appear as physical devices on the LAN (e.g., DHCP servers, network appliances).

### none

Completely disables networking for the container.

```yaml
networks:
  no-net:
    driver: none
```

**ALWAYS** use `none` when a container must have zero network access for security isolation.

## IPAM Configuration

IPAM (IP Address Management) controls how IP addresses are assigned to containers on a network.

### Full IPAM Structure

```yaml
networks:
  custom-net:
    ipam:
      driver: default
      config:
        - subnet: 172.28.0.0/16
          ip_range: 172.28.5.0/24
          gateway: 172.28.5.254
          aux_addresses:
            host1: 172.28.1.5
            host2: 172.28.1.6
```

### IPAM Elements

| Element | Type | Description |
|---------|------|-------------|
| `driver` | string | IPAM driver (default: `default`) |
| `config` | list | List of IPAM configuration blocks |
| `config[].subnet` | string | CIDR-formatted network segment |
| `config[].ip_range` | string | Allocatable container IP range within the subnet |
| `config[].gateway` | string | IPv4 or IPv6 gateway for master subnet |
| `config[].aux_addresses` | map | Auxiliary addresses mapped to hostnames (reserved IPs) |

### Dual-Stack (IPv4 + IPv6) IPAM

```yaml
networks:
  dual-stack:
    enable_ipv6: true
    ipam:
      config:
        - subnet: 172.28.0.0/16
          gateway: 172.28.0.1
        - subnet: 2001:db8::/64
          gateway: 2001:db8::1
```

### IPv6-Only Network

```yaml
networks:
  ipv6-only:
    enable_ipv4: false
    enable_ipv6: true
    ipam:
      config:
        - subnet: 2001:db8::/64
```

## External Networks

External networks are NOT created or destroyed by Compose. They must exist before running `docker compose up`.

```yaml
networks:
  # Simple external reference
  existing-net:
    external: true

  # External with custom name
  app-net:
    external: true
    name: "production-network"

  # External with variable interpolation
  dynamic-net:
    external: true
    name: "${NETWORK_ID}"
```

**ALWAYS** create external networks before running Compose:
```bash
docker network create production-network
```

**NEVER** use `driver`, `driver_opts`, `ipam`, `internal`, `attachable`, or `labels` alongside `external: true` -- only `name` is valid with external networks.

## Customizing the Default Network

Every Compose project gets an implicit `default` network. Override it by defining a network named `default`:

```yaml
networks:
  default:
    name: my-project-network
    driver: bridge
    driver_opts:
      com.docker.network.bridge.host_binding_ipv4: "127.0.0.1"
    ipam:
      config:
        - subnet: 172.30.0.0/16
```

## Service-Level Network Options

### Full Service Network Syntax

```yaml
services:
  app:
    networks:
      frontend:
        aliases:
          - webapp
          - api-server
        ipv4_address: 172.16.238.10
        ipv6_address: 2001:3984:3989::10
        link_local_ips:
          - 169.254.0.10
        mac_address: "02:42:ac:11:65:43"
        interface_name: eth1
        priority: 1000
        gw_priority: 100
        driver_opts:
          com.example.custom: "value"
```

### Service Network Attributes

| Attribute | Type | Description |
|-----------|------|-------------|
| `aliases` | list | Additional hostnames for DNS resolution on this network |
| `ipv4_address` | string | Static IPv4 address (requires IPAM subnet config) |
| `ipv6_address` | string | Static IPv6 address (requires IPAM subnet + enable_ipv6) |
| `link_local_ips` | list | Link-local IP assignments |
| `mac_address` | string | MAC address for this network connection |
| `interface_name` | string | Name of the network interface in the container |
| `priority` | integer | Connection order (higher connects first) |
| `gw_priority` | integer | Default gateway selection (higher value wins) |
| `driver_opts` | map | Driver-specific options for this connection |

### Network Aliases

Aliases provide additional DNS names for a service on a specific network. Other containers on the same network can reach the service using any of its aliases.

```yaml
services:
  database:
    image: postgres
    networks:
      backend:
        aliases:
          - db
          - postgres
          - primary-db
```

**ALWAYS** use aliases when multiple services need to reference another service by different names, or when migrating from one service name to another.

### Static IP Assignment

Static IPs require a matching IPAM subnet configuration:

```yaml
services:
  dns:
    image: coredns
    networks:
      infra:
        ipv4_address: 172.20.0.53

networks:
  infra:
    ipam:
      config:
        - subnet: 172.20.0.0/16
```

**NEVER** assign static IPs without defining the subnet in IPAM -- Compose cannot validate the address without a known subnet range.

## Network Isolation Pattern

```yaml
services:
  proxy:
    networks: [frontend]
  app:
    networks: [frontend, backend]
  db:
    networks: [backend]
  cache:
    networks: [backend]

networks:
  frontend:
    driver: bridge
  backend:
    driver: bridge
    internal: true
```

- `proxy` can ONLY reach `app` (shared `frontend` network)
- `db` and `cache` can ONLY reach `app` (shared `backend` network)
- `proxy` CANNOT reach `db` or `cache` (no shared network)
- `backend` has `internal: true` -- containers on it cannot reach the internet

## network_mode (Service-Level)

`network_mode` overrides the default network assignment entirely:

```yaml
services:
  monitor:
    network_mode: "host"           # Share host network
  isolated:
    network_mode: "none"           # No networking
  sidecar:
    network_mode: "service:app"    # Share network namespace with another service
  legacy:
    network_mode: "container:abc123"  # Share with specific container
```

**NEVER** combine `network_mode` with `networks` -- they are mutually exclusive. Using both causes a Compose validation error.

## Official Sources

- https://docs.docker.com/compose/compose-file/06-networks/
- https://docs.docker.com/engine/network/
- https://docs.docker.com/engine/network/drivers/
