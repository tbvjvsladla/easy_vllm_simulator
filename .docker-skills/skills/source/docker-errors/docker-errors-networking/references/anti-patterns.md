# Networking Anti-Patterns — What NOT to Do

> Common Docker networking configuration mistakes, why they fail, and the correct alternative.
> Docker Engine 24+, Docker Compose v2.

---

## AP-001: Using the Default Bridge Network

### The Mistake

```bash
# Relying on the default bridge for container communication
docker run -d --name redis redis
docker run -d --name webapp myapp
# webapp tries to connect to "redis" by hostname — FAILS
```

### Why It Fails

The default `bridge` network does NOT provide embedded DNS resolution. Containers can only reach each other by IP address, which changes on restart. The legacy `--link` flag provides name resolution but is deprecated and not maintained.

### The Correct Way

```bash
# ALWAYS create and use a user-defined bridge network
docker network create app-net
docker run -d --name redis --network app-net redis
docker run -d --name webapp --network app-net myapp
# webapp can now reach "redis" by hostname via Docker's embedded DNS
```

---

## AP-002: Hardcoding Container IP Addresses

### The Mistake

```python
# Application config
DATABASE_HOST = "172.18.0.3"  # Hardcoded container IP
REDIS_HOST = "172.18.0.4"
```

### Why It Fails

Container IP addresses are assigned dynamically and change every time a container is recreated, restarted, or moved to a different network. Hardcoded IPs break silently when infrastructure changes.

### The Correct Way

```python
# ALWAYS use container names or network aliases
DATABASE_HOST = "db"      # Docker DNS resolves this
REDIS_HOST = "redis"      # Docker DNS resolves this
```

```bash
# Or use network aliases for more descriptive names
docker run --network app-net --network-alias database postgres
```

---

## AP-003: Binding Services to 127.0.0.1 Inside Containers

### The Mistake

```javascript
// Node.js server inside container
app.listen(3000, '127.0.0.1');  // Only accessible from inside this container
```

```python
# Flask inside container
app.run(host='127.0.0.1', port=5000)  # Only accessible from inside this container
```

### Why It Fails

`127.0.0.1` (localhost) inside a container refers to the container's own loopback interface. Other containers — even on the same Docker network — cannot reach this address. It is container-private.

### The Correct Way

```javascript
// ALWAYS bind to 0.0.0.0 inside containers
app.listen(3000, '0.0.0.0');  // Accessible from all network interfaces
```

```python
# ALWAYS bind to 0.0.0.0 inside containers
app.run(host='0.0.0.0', port=5000)
```

---

## AP-004: Using --link for Container Communication

### The Mistake

```bash
docker run -d --name redis redis
docker run -d --name webapp --link redis:redis myapp
```

### Why It Fails

`--link` is legacy and deprecated. It only works on the default bridge, does not support user-defined networks, cannot be updated without container recreation, and creates fragile coupling between containers.

### The Correct Way

```bash
docker network create app-net
docker run -d --name redis --network app-net redis
docker run -d --name webapp --network app-net myapp
# DNS-based discovery — no --link needed
```

---

## AP-005: Publishing Ports for Container-to-Container Communication

### The Mistake

```yaml
# docker-compose.yml
services:
  api:
    image: myapi
    ports:
      - "3000:3000"  # Published to host — unnecessary for inter-service traffic

  web:
    image: myweb
    environment:
      API_URL: "http://localhost:3000"  # Tries to reach via host — fragile
```

### Why It Fails

Port publishing (`-p` / `ports:`) maps a container port to a host port. This is only needed for access from outside the Docker network. Containers on the same network can reach ALL ports of other containers directly — no publishing needed. Using `localhost` from one container does NOT reach another container.

### The Correct Way

```yaml
services:
  api:
    image: myapi
    # NO ports: needed for inter-service communication
    # Only add ports: if external access is required

  web:
    image: myweb
    ports:
      - "8080:80"  # Only web needs external access
    environment:
      API_URL: "http://api:3000"  # Use service name, not localhost
```

---

## AP-006: Using depends_on Without Health Checks

### The Mistake

```yaml
services:
  db:
    image: postgres:16
    # No healthcheck defined

  webapp:
    depends_on:
      - db  # Only waits for container START, not service READY
```

### Why It Fails

`depends_on` without `condition: service_healthy` only ensures the database container has started. PostgreSQL may take several seconds to initialize. The webapp connects immediately and gets `connection refused` because the database is not accepting connections yet.

### The Correct Way

```yaml
services:
  db:
    image: postgres:16
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 5s
      retries: 5
      start_period: 10s

  webapp:
    depends_on:
      db:
        condition: service_healthy  # Waits for health check to pass
```

---

## AP-007: Ignoring Network Isolation Between Compose Projects

### The Mistake

```bash
# Project A
cd frontend && docker compose up -d

# Project B
cd backend && docker compose up -d

# Expecting frontend to reach backend by service name — FAILS
```

### Why It Fails

Each Compose project creates its own isolated default network (`frontend_default`, `backend_default`). Services in different projects cannot see each other because they are on separate networks with separate DNS scopes.

### The Correct Way

```bash
# Create shared network first
docker network create shared-services
```

```yaml
# Both projects reference the external network
networks:
  shared:
    external: true
    name: shared-services

services:
  myservice:
    networks:
      - default  # Keep internal communication
      - shared   # Add cross-project access
```

---

## AP-008: Not Configuring Docker Address Pools with VPN

### The Mistake

```bash
# Install Docker with default settings on a machine with corporate VPN
# Docker creates networks in 172.17-28.x.x and 192.168.x.x ranges
# VPN routes to 172.16.0.0/12 stop working
```

### Why It Fails

Docker's default address pools (`172.17.0.0/16` through `172.28.0.0/14`, `192.168.0.0/16`) overlap with many corporate VPN ranges. Docker's local routes take precedence, breaking VPN connectivity to those subnets.

### The Correct Way

```json
// /etc/docker/daemon.json — configure BEFORE creating any networks
{
  "default-address-pools": [
    {
      "base": "10.99.0.0/16",
      "size": 24
    }
  ]
}
```

```bash
sudo systemctl restart docker
# All new networks will use 10.99.x.x — no VPN conflict
```

---

## AP-009: Forgetting to Restart Docker After Firewall Changes

### The Mistake

```bash
# Reload firewall rules
sudo firewall-cmd --reload
# or: sudo ufw reload
# or: sudo iptables -F

# All Docker containers lose network connectivity
# Admin does not restart Docker
```

### Why It Fails

Docker creates iptables chains (DOCKER, DOCKER-ISOLATION, DOCKER-USER) and NAT MASQUERADE rules at startup. When the firewall flushes iptables, these chains are destroyed. Without them, Docker cannot route traffic between containers or to the internet.

### The Correct Way

```bash
# ALWAYS restart Docker after any firewall change
sudo firewall-cmd --reload && sudo systemctl restart docker

# Or automate with systemd:
# /etc/systemd/system/firewalld.service.d/docker.conf
[Service]
ExecStartPost=/usr/bin/systemctl restart docker
```

---

## AP-010: Using host.docker.internal in Production

### The Mistake

```yaml
services:
  webapp:
    environment:
      DB_HOST: "host.docker.internal"  # Relies on Docker Desktop feature
```

### Why It Fails

`host.docker.internal` is a Docker Desktop convenience feature. It does NOT work reliably on Linux Docker Engine in production. It is not a standard DNS name and its behavior varies across platforms and Docker versions.

### The Correct Way

```yaml
# For container-to-container: use Docker service names
services:
  webapp:
    environment:
      DB_HOST: "db"  # Another container on the same network

# For container-to-host on Linux:
# Use the host gateway IP or --network host
services:
  webapp:
    extra_hosts:
      - "host.docker.internal:host-gateway"  # Explicit and portable
```

---

## AP-011: Exposing Database Ports to the Host

### The Mistake

```yaml
services:
  db:
    image: postgres:16
    ports:
      - "5432:5432"  # Database accessible from entire network
```

### Why It Fails

Publishing the database port to the host makes it accessible from any machine that can reach the host. This is a security risk — databases should only be accessible from application containers, not from the network.

### The Correct Way

```yaml
services:
  db:
    image: postgres:16
    # NO ports: — only accessible from containers on the same network
    networks:
      - backend

  webapp:
    image: myapp
    networks:
      - backend
    environment:
      DB_HOST: db  # Reaches database via internal network

networks:
  backend:
    internal: true  # Optional: also blocks internet access from db
```

```bash
# If you need temporary host access for debugging:
# Use a one-off port-forward instead of permanent publishing
docker exec -it db psql -U postgres
```

---

## AP-012: Not Using internal Networks for Backend Services

### The Mistake

```yaml
services:
  web:
    networks:
      - app-net
  db:
    networks:
      - app-net  # Database has full internet access — unnecessary risk
```

### Why It Fails

By default, all Docker bridge networks allow outbound internet access. A compromised database container could exfiltrate data or download malware. Backend services (databases, caches, message queues) rarely need internet access.

### The Correct Way

```yaml
services:
  web:
    networks:
      - frontend
      - backend

  db:
    networks:
      - backend  # Cannot reach internet

networks:
  frontend:
    # Normal network — internet access allowed
  backend:
    internal: true  # No outbound internet — maximum isolation
```

---

## Summary Table

| # | Anti-Pattern | Risk | Correct Approach |
|---|-------------|------|-----------------|
| AP-001 | Default bridge network | No DNS, no isolation | User-defined bridge networks |
| AP-002 | Hardcoded container IPs | Breaks on restart | DNS names or network aliases |
| AP-003 | Bind to 127.0.0.1 in container | Unreachable from other containers | Bind to 0.0.0.0 |
| AP-004 | --link flag | Deprecated, fragile | User-defined networks |
| AP-005 | Publish ports for inter-container traffic | Unnecessary host exposure | Direct container-to-container on same network |
| AP-006 | depends_on without health check | Race condition on startup | condition: service_healthy |
| AP-007 | Assume cross-project DNS works | Projects are network-isolated | Shared external network |
| AP-008 | Default address pools with VPN | VPN routes broken | Custom address pools in daemon.json |
| AP-009 | Skip Docker restart after firewall change | All containers lose network | ALWAYS restart Docker after firewall changes |
| AP-010 | host.docker.internal in production | Platform-dependent, unreliable | Service names or explicit host mapping |
| AP-011 | Publish database ports | Security exposure | Internal network, no port publishing |
| AP-012 | Internet access for backend services | Data exfiltration risk | internal: true networks |
