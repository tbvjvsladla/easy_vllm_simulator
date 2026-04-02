---
name: docker-core-networking
description: >
  Use when configuring Docker networks, debugging container connectivity, or
  setting up service discovery between containers.
  Prevents using the default bridge network in production, which lacks DNS
  resolution and automatic service discovery.
  Covers bridge, host, overlay, macvlan, ipvlan, and none drivers, DNS
  resolution, port mapping, network isolation, and user-defined networks.
  Keywords: docker network create, --network, -p, --publish, bridge, overlay,
  macvlan, docker-compose networks, DNS, service discovery, containers can't
  talk to each other, network not working, connect containers.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-core-networking

## Quick Reference

### Network Drivers

| Driver | Isolation | Multi-Host | Use Case |
|--------|-----------|------------|----------|
| **bridge** | Container-level | No | Default single-host container communication |
| **host** | None (shares host) | No | Performance-critical apps needing direct host network |
| **overlay** | Container-level | Yes (Swarm) | Cross-host service communication |
| **macvlan** | Container-level | No | Containers appear as physical LAN devices |
| **ipvlan** | Container-level | No | VLAN integration without MAC-per-container |
| **none** | Complete | No | Fully isolated containers with no networking |

### Default Bridge vs User-Defined Bridge

| Feature | Default Bridge | User-Defined Bridge |
|---------|---------------|-------------------|
| DNS resolution | IP only (no name resolution) | Automatic by container name |
| Isolation | ALL containers join by default | Only explicitly connected containers |
| Live connect/disconnect | Requires container recreation | On-the-fly via `docker network connect` |
| Configuration | Shared, daemon restart needed | Per-network, independent |
| Recommended | NEVER for production | ALWAYS use this |

### Port Mapping Syntax

| Syntax | Meaning |
|--------|---------|
| `-p 8080:80` | Host port 8080 to container port 80 |
| `-p 127.0.0.1:8080:80` | Bind to localhost only |
| `-p 80:8080/tcp` | TCP only (default) |
| `-p 80:8080/udp` | UDP only |
| `-p 80:8080/tcp -p 80:8080/udp` | Both TCP and UDP |
| `-p 8000-8010:8000-8010` | Port range mapping |
| `-P` | All EXPOSE ports to random host ports |

### CLI Command Reference

| Command | Purpose |
|---------|---------|
| `docker network create` | Create a network |
| `docker network connect` | Connect running container to network |
| `docker network disconnect` | Disconnect container from network |
| `docker network ls` | List networks |
| `docker network inspect` | Show network details |
| `docker network rm` | Remove network |
| `docker network prune` | Remove all unused networks |

### Critical Warnings

**ALWAYS** use user-defined bridge networks instead of the default bridge. The default bridge lacks DNS resolution, proper isolation, and per-network configuration.

**NEVER** use `--link` for container communication -- it is legacy and deprecated. Use user-defined networks with DNS-based service discovery instead.

**NEVER** publish ports with `-p 0.0.0.0:PORT:PORT` on production hosts unless external access is intended. Use `-p 127.0.0.1:PORT:PORT` to restrict to localhost.

**ALWAYS** use `--internal` flag when creating networks that should have no external (internet) access. This prevents accidental data exfiltration.

**NEVER** rely on container IP addresses for communication -- IPs change on container restart. ALWAYS use container names or network aliases for DNS-based discovery.

---

## Network Driver Decision Tree

```
Need container networking?
├── No → use --network none
└── Yes
    ├── Need host-level performance (no NAT overhead)?
    │   └── Yes → use --network host
    ├── Need multi-host communication (Swarm)?
    │   └── Yes → use overlay driver
    │       ├── Need standalone container access? → --attachable
    │       └── Need encryption? → --opt encrypted
    ├── Need container to appear as physical device on LAN?
    │   ├── Yes, one MAC per container → use macvlan
    │   └── Yes, shared MAC (VLAN) → use ipvlan
    └── Single-host container communication
        └── ALWAYS use user-defined bridge
            └── docker network create mynet
```

---

## DNS Resolution

### How DNS Works in User-Defined Networks

Docker runs an embedded DNS server at `127.0.0.11` for all user-defined networks. Containers resolve each other by:

1. **Container name** -- The `--name` value becomes a DNS hostname
2. **Network alias** -- Additional DNS names via `--network-alias`
3. **Service name** -- In Compose, the service key is the DNS name

```
Container A (name: web)          Container B (name: api)
    |                                |
    |--- DNS query: "api" ---------> |
    |          127.0.0.11            |
    |<-- Response: 172.20.0.3 -------|
    |                                |
    |--- HTTP GET api:8080 --------->|  (resolved via DNS)
```

### DNS Configuration

```bash
# Custom DNS server for external resolution
docker run --dns 8.8.8.8 nginx

# Custom search domain
docker run --dns-search example.com nginx

# Custom DNS options
docker run --dns-option ndots:2 nginx
```

### Key DNS Rules

- **Default bridge**: Containers inherit host `/etc/resolv.conf` -- NO container name resolution
- **User-defined networks**: Docker DNS at `127.0.0.11` -- FULL container name resolution
- **Multiple networks**: A container resolves names ONLY for containers on the same network
- **External DNS**: Queries not matching container names forward to configured upstream DNS

---

## Network Creation and IPAM

### Basic Network Creation

```bash
# Simple user-defined bridge
docker network create mynet

# Bridge with custom subnet
docker network create --driver bridge \
  --subnet=172.28.0.0/16 \
  --gateway=172.28.0.1 \
  mynet

# Bridge with custom IP allocation range
docker network create --driver bridge \
  --subnet=172.28.0.0/16 \
  --ip-range=172.28.5.0/24 \
  --gateway=172.28.5.254 \
  mynet

# IPv6-enabled network
docker network create --ipv6 --subnet 2001:db8::/64 v6net

# Internal network (no external access)
docker network create --internal isolated
```

### IPAM Configuration

| Option | Purpose | Example |
|--------|---------|---------|
| `--subnet` | Network address range | `--subnet=172.28.0.0/16` |
| `--gateway` | Default gateway address | `--gateway=172.28.0.1` |
| `--ip-range` | Allocatable IP range within subnet | `--ip-range=172.28.5.0/24` |
| `--ipv6` | Enable IPv6 | `--ipv6` |
| `--aux-address` | Reserve addresses | `--aux-address="switch=172.28.0.2"` |

### Static IP Assignment

```bash
# Assign static IP to container (requires user-defined network with subnet)
docker network connect --ip 172.28.5.10 mynet myapp
docker run --network mynet --ip 172.28.5.10 nginx
```

---

## Container-to-Container Communication

### Same Network (Recommended)

```bash
# Create network
docker network create app-net

# Run containers on the same network
docker run -d --name db --network app-net postgres:16
docker run -d --name api --network app-net \
  -e DATABASE_URL=postgresql://db:5432/mydb myapp

# api can reach db by name "db" via DNS
```

### Multiple Networks for Isolation

```bash
# Frontend network (web + api)
docker network create frontend

# Backend network (api + db)
docker network create backend

# Web server -- only frontend
docker run -d --name web --network frontend -p 80:80 nginx

# API server -- both networks (bridge between frontend and backend)
docker run -d --name api --network frontend myapi
docker network connect backend api

# Database -- only backend (unreachable from web)
docker run -d --name db --network backend postgres:16
```

### Network Aliases

```bash
# Multiple containers behind one DNS name (client-side load balancing)
docker run -d --network mynet --network-alias search elasticsearch:8
docker run -d --network mynet --network-alias search elasticsearch:8

# Both containers resolve via "search" -- Docker round-robins responses
```

### Port Exposure Rules

- Containers on the SAME user-defined network expose ALL ports to each other automatically
- `-p` flag is ONLY needed for access from outside the Docker network (host or external)
- The `EXPOSE` instruction in Dockerfile is documentation only -- it does NOT publish ports

---

## Docker Compose Networking

### Default Behavior

Compose automatically creates a network named `{project}_default` and connects all services:

```yaml
# All services can reach each other by service name
services:
  web:
    image: nginx
    ports:
      - "80:80"      # Published to host
  api:
    image: myapi
    # Reaches db via hostname "db" automatically
  db:
    image: postgres:16
    # No ports published -- only accessible within Compose network
```

### Custom Networks in Compose

```yaml
services:
  web:
    image: nginx
    networks:
      - frontend
  api:
    image: myapi
    networks:
      - frontend
      - backend
  db:
    image: postgres:16
    networks:
      - backend

networks:
  frontend:
    driver: bridge
  backend:
    driver: bridge
    internal: true    # No external access
```

### External Networks

```yaml
# Use pre-existing network (MUST exist before docker compose up)
networks:
  existing-net:
    external: true
```

---

## Network Inspection and Debugging

### Inspect Commands

```bash
# List all networks
docker network ls
docker network ls --filter driver=bridge

# Inspect network details (containers, IPAM, options)
docker network inspect mynet

# Get containers on a network
docker network inspect --format='{{range .Containers}}{{.Name}} {{end}}' mynet

# Get container IP address
docker inspect --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' myapp
```

### Debug Connectivity

```bash
# Test DNS resolution from inside container
docker exec myapp nslookup other-container

# Test connectivity
docker exec myapp ping -c 2 other-container

# Check container's DNS config
docker exec myapp cat /etc/resolv.conf

# Check which networks a container belongs to
docker inspect --format='{{range $k, $v := .NetworkSettings.Networks}}{{$k}} {{end}}' myapp
```

---

## Reference Links

- [references/drivers.md](references/drivers.md) -- All network driver details, options, and use cases
- [references/examples.md](references/examples.md) -- Network creation, multi-container networking, isolation patterns
- [references/anti-patterns.md](references/anti-patterns.md) -- Common networking mistakes and how to avoid them

### Official Sources

- https://docs.docker.com/engine/network/
- https://docs.docker.com/engine/network/drivers/bridge/
- https://docs.docker.com/engine/network/drivers/overlay/
- https://docs.docker.com/engine/network/drivers/host/
- https://docs.docker.com/engine/network/drivers/macvlan/
- https://docs.docker.com/engine/network/drivers/ipvlan/
- https://docs.docker.com/compose/how-tos/networking/
