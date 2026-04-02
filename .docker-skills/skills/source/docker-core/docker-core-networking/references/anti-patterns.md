# Networking Anti-Patterns

## AP-1: Using the Default Bridge Network

### Problem

```bash
# Containers on the default bridge -- NO DNS resolution
docker run -d --name db postgres:16
docker run -d --name api -e DB_HOST=db myapi
# api CANNOT resolve "db" by name -- connection fails
```

### Why It Fails

The default bridge network does NOT provide automatic DNS resolution. Containers can only reach each other by IP address, which changes on every container restart.

### Correct Approach

```bash
# ALWAYS create a user-defined bridge network
docker network create app-net
docker run -d --name db --network app-net postgres:16
docker run -d --name api --network app-net -e DB_HOST=db myapi
# api resolves "db" automatically via Docker's embedded DNS
```

---

## AP-2: Using --link for Container Communication

### Problem

```bash
# Legacy --link flag -- deprecated and will be removed
docker run -d --name db postgres:16
docker run -d --link db:database myapi
```

### Why It Fails

`--link` is a legacy feature that only works on the default bridge. It does NOT work with user-defined networks, does NOT support dynamic discovery, and will be removed in a future Docker release.

### Correct Approach

```bash
docker network create app-net
docker run -d --name db --network app-net postgres:16
docker run -d --name api --network app-net myapi
# Use container name "db" as hostname -- works with DNS
```

---

## AP-3: Hardcoding Container IP Addresses

### Problem

```bash
docker run -d --name db --network app-net postgres:16
# Get IP and hardcode it
DB_IP=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' db)
docker run -d --name api --network app-net -e DB_HOST=$DB_IP myapi
```

### Why It Fails

Container IP addresses are **ephemeral**. They change when containers restart, are recreated, or when the network is recreated. Hardcoded IPs break silently.

### Correct Approach

```bash
# ALWAYS use container names or network aliases for discovery
docker run -d --name api --network app-net -e DB_HOST=db myapi
# "db" resolves dynamically via DNS -- survives container restarts
```

---

## AP-4: Publishing Ports on All Interfaces

### Problem

```bash
# Exposes database to entire network and internet
docker run -d -p 5432:5432 postgres:16
# Equivalent to -p 0.0.0.0:5432:5432
```

### Why It Fails

Publishing on `0.0.0.0` makes the service accessible from any network interface, including public interfaces. Database ports, admin panels, and internal services become exposed to attackers.

### Correct Approach

```bash
# Bind to localhost only for internal services
docker run -d -p 127.0.0.1:5432:5432 postgres:16

# For services that only need container-to-container access,
# do NOT publish ports at all -- use a shared network instead
docker network create backend
docker run -d --name db --network backend postgres:16
docker run -d --name api --network backend myapi
# api reaches db:5432 directly -- no host port exposure
```

---

## AP-5: Publishing Ports for Container-to-Container Communication

### Problem

```bash
# Publishing ports when only containers need to communicate
docker run -d --name db --network app-net -p 5432:5432 postgres:16
docker run -d --name cache --network app-net -p 6379:6379 redis:7
docker run -d --name api --network app-net myapi
```

### Why It Fails

Containers on the same user-defined network can reach ALL ports on other containers automatically. Publishing ports (`-p`) is ONLY needed for access from outside the Docker network. Unnecessary port publishing increases the attack surface.

### Correct Approach

```bash
# No -p needed for inter-container communication
docker run -d --name db --network app-net postgres:16
docker run -d --name cache --network app-net redis:7
docker run -d --name api --network app-net -p 3000:3000 myapi
# Only api port 3000 is published -- the one that needs host/external access
```

---

## AP-6: All Containers on a Single Network

### Problem

```yaml
services:
  web:
    image: nginx
  api:
    image: myapi
  db:
    image: postgres:16
  cache:
    image: redis:7
  monitoring:
    image: prometheus
# All on the default Compose network -- every service can reach every other service
```

### Why It Fails

No network segmentation means the web server can directly access the database, monitoring can reach application internals, and a compromise of any container gives network access to all others.

### Correct Approach

```yaml
services:
  web:
    networks: [frontend]
  api:
    networks: [frontend, backend]
  db:
    networks: [backend]
  cache:
    networks: [backend]
  monitoring:
    networks: [monitoring, backend]

networks:
  frontend:
  backend:
    internal: true
  monitoring:
```

---

## AP-7: Using Host Network Mode by Default

### Problem

```bash
# Using host network for convenience, not performance
docker run --network host mywebapp
docker run --network host another-app
```

### Why It Fails

Host networking removes ALL network isolation. Multiple containers compete for the same ports. A compromised container has full access to the host's network interfaces, listening sockets, and can sniff traffic.

### Correct Approach

```bash
# Use host networking ONLY when NAT overhead is measurable and unacceptable
# For normal applications, use bridge networking with port mapping
docker network create app-net
docker run -d --name web --network app-net -p 80:80 mywebapp
```

---

## AP-8: Ignoring DNS Resolution Differences Between Networks

### Problem

```bash
docker network create net-a
docker network create net-b

docker run -d --name db --network net-a postgres:16
docker run -d --name api --network net-b myapi
# api tries to resolve "db" -- FAILS because they are on different networks
```

### Why It Fails

Docker DNS resolution is scoped to each network. A container can ONLY resolve names of containers on the same network. Containers on different networks are invisible to each other.

### Correct Approach

```bash
# Option 1: Same network
docker run -d --name api --network net-a myapi

# Option 2: Connect api to both networks
docker run -d --name api --network net-b myapi
docker network connect net-a api
# Now api can resolve names on BOTH networks
```

---

## AP-9: Not Using --internal for Sensitive Backend Networks

### Problem

```bash
# Backend network has internet access by default
docker network create backend
docker run -d --name db --network backend postgres:16
# A compromised db container can exfiltrate data to the internet
```

### Why It Fails

By default, bridge networks provide outbound internet access via NAT. Backend services (databases, caches, queues) rarely need internet access. Leaving it enabled creates an exfiltration path.

### Correct Approach

```bash
# Internal network -- no outbound internet access
docker network create --internal backend
docker run -d --name db --network backend postgres:16
# db can communicate with other containers on backend, but CANNOT reach the internet
```

---

## AP-10: Forgetting Overlay Network Port Requirements

### Problem

```bash
# Swarm nodes cannot communicate -- overlay network broken
docker swarm init
docker network create -d overlay my-overlay
docker service create --network my-overlay --replicas 3 myapp
# Containers on different hosts cannot reach each other
```

### Why It Fails

Overlay networks require specific ports open between ALL Swarm nodes:
- **2377/tcp** -- Swarm management
- **4789/udp** -- VXLAN data
- **7946/tcp+udp** -- Node discovery

If firewall rules block these ports, overlay networking fails silently or intermittently.

### Correct Approach

```bash
# Verify required ports are open on ALL Swarm nodes
# On each node's firewall:
ufw allow 2377/tcp    # Swarm management
ufw allow 4789/udp    # VXLAN
ufw allow 7946/tcp    # Node discovery
ufw allow 7946/udp    # Node discovery

# Then create overlay network
docker network create -d overlay --attachable my-overlay
```

---

## AP-11: Relying on EXPOSE for Security

### Problem

```dockerfile
# Dockerfile
EXPOSE 80
# "Only port 80 is accessible" -- WRONG assumption
```

### Why It Fails

The `EXPOSE` instruction is **documentation only**. It does NOT restrict which ports are accessible. Containers on the same network can access ANY port on any other container, regardless of `EXPOSE`. The only things that control port access are:
- Network membership (which network the container is on)
- `-p` flag (which ports are published to the host)
- `--internal` flag (blocks all external access)

### Correct Approach

Use network segmentation and `-p` flags for access control. Treat `EXPOSE` as documentation for operators, not as a security mechanism.

---

## AP-12: Not Cleaning Up Unused Networks

### Problem

```bash
# Creating networks for temporary tasks and never removing them
docker network create test-1
docker network create test-2
docker network create experiment
# Months later: dozens of orphaned networks consuming address space
```

### Why It Fails

Docker has a limited default address pool. Each unused network holds a subnet allocation. Eventually, new network creation fails with "could not find an available, non-overlapping IPv4 address pool."

### Correct Approach

```bash
# Regular cleanup of unused networks
docker network prune -f

# Or with age filter
docker network prune -f --filter "until=24h"

# Check before pruning
docker network ls
```

---

## Official Sources

- https://docs.docker.com/engine/network/
- https://docs.docker.com/engine/network/drivers/bridge/
- https://docs.docker.com/engine/network/drivers/overlay/
