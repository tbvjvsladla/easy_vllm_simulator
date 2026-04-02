# Network Debugging Sessions — Worked Examples

> Step-by-step debugging sessions showing how to diagnose and resolve Docker networking issues.
> Docker Engine 24+, Docker Compose v2.

---

## Session 1: Containers Cannot Communicate by Name

### Scenario
Two containers started with `docker run`. Web app tries to connect to `redis:6379` but gets `dial tcp: lookup redis: no such host`.

### Diagnosis

```bash
# Step 1: Check what network each container is on
docker inspect --format='{{range $k, $v := .NetworkSettings.Networks}}{{$k}}{{end}}' webapp
# Output: bridge

docker inspect --format='{{range $k, $v := .NetworkSettings.Networks}}{{$k}}{{end}}' redis
# Output: bridge

# Step 2: Both on "bridge" (default) — this is the problem
# Default bridge does NOT provide DNS resolution

# Step 3: Verify DNS is broken
docker exec webapp nslookup redis
# Output: ** server can't find redis: NXDOMAIN
```

### Fix

```bash
# Create user-defined network
docker network create app-net

# Stop and recreate containers on the new network
docker stop webapp redis
docker rm webapp redis

docker run -d --name redis --network app-net redis:7-alpine
docker run -d --name webapp --network app-net -p 8080:8080 mywebapp

# Verify DNS works
docker exec webapp nslookup redis
# Output: Name: redis  Address: 172.20.0.2

docker exec webapp ping -c 1 redis
# Output: PING redis (172.20.0.2): 56 data bytes — 64 bytes from 172.20.0.2
```

### Key Lesson
**ALWAYS use user-defined networks.** The default bridge is legacy and lacks DNS resolution between containers.

---

## Session 2: Port Already Allocated

### Scenario
Starting a container with `-p 3000:3000` fails with `Error: port is already allocated`.

### Diagnosis

```bash
# Step 1: Find what is using port 3000
# On Linux:
sudo lsof -i :3000
# Output: node  12345  user  TCP *:3000 (LISTEN)

# Or:
sudo ss -tlnp | grep 3000
# Output: LISTEN  0  128  *:3000  *:*  users:(("node",pid=12345,fd=19))

# Step 2: It's a local Node.js dev server using the same port
```

### Fix

```bash
# Option A: Stop the conflicting process
kill 12345

# Option B: Use a different host port
docker run -d -p 3001:3000 --name myapp myimage
# Access via localhost:3001, container still listens on 3000

# Option C: Check for orphaned Docker containers
docker ps -a --filter "publish=3000"
# If found, remove: docker rm -f <container-id>
```

### Key Lesson
ALWAYS check for port conflicts before starting containers. Use `lsof` or `ss` to identify the blocking process.

---

## Session 3: Service Binds to 127.0.0.1 Inside Container

### Scenario
Container A can ping Container B by name, but `curl http://api:8080` returns `connection refused`.

### Diagnosis

```bash
# Step 1: Verify DNS works
docker exec web nslookup api
# Output: Name: api  Address: 172.20.0.3  — DNS is fine

# Step 2: Verify ping works
docker exec web ping -c 1 api
# Output: 64 bytes from 172.20.0.3 — Network layer is fine

# Step 3: Check what the target service is listening on
docker exec api ss -tlnp
# Output: LISTEN  0  128  127.0.0.1:8080  *:*  — PROBLEM FOUND
# The service binds to 127.0.0.1, only reachable from inside its own container

# Step 4: Verify from inside the target container
docker exec api curl -s http://127.0.0.1:8080
# Output: OK — works locally but not from other containers
```

### Fix

```bash
# Change application config to bind to 0.0.0.0:8080

# For Node.js:
# app.listen(8080, '0.0.0.0')

# For Python Flask:
# app.run(host='0.0.0.0', port=8080)

# For Go:
# http.ListenAndServe(":8080", handler)  — empty host = all interfaces

# After fix:
docker exec api ss -tlnp
# Output: LISTEN  0  128  0.0.0.0:8080  *:*  — correct

docker exec web curl -s http://api:8080
# Output: OK
```

### Key Lesson
**ALWAYS bind to 0.0.0.0 inside containers.** Binding to 127.0.0.1 makes the service unreachable from any other container, even on the same network.

---

## Session 4: Docker Breaks VPN Connectivity

### Scenario
After installing Docker, VPN connections to `10.0.0.0/8` range stop working. Some corporate hosts become unreachable.

### Diagnosis

```bash
# Step 1: Check Docker network subnets
docker network ls
docker network inspect bridge --format='{{range .IPAM.Config}}{{.Subnet}}{{end}}'
# Output: 172.17.0.0/16

# Step 2: Check other Docker networks
docker network inspect $(docker network ls -q) --format='{{.Name}}: {{range .IPAM.Config}}{{.Subnet}}{{end}}'
# Output:
# bridge: 172.17.0.0/16
# mynet1: 172.18.0.0/16
# mynet2: 172.19.0.0/16

# Step 3: Check routing table
ip route | grep 172
# Docker routes override VPN routes for overlapping ranges

# Step 4: Check if Docker's default pool conflicts
# Default pools: 172.17.0.0/16 through 172.28.0.0/14, 192.168.0.0/16
# If VPN uses any 172.x.x.x or 192.168.x.x — conflict
```

### Fix

```bash
# Configure Docker to use non-conflicting address pools
# Edit /etc/docker/daemon.json:
{
  "default-address-pools": [
    {
      "base": "10.99.0.0/16",
      "size": 24
    }
  ]
}

# Restart Docker
sudo systemctl restart docker

# Remove old networks and recreate
docker network prune -f
docker network create mynet
docker network inspect mynet --format='{{range .IPAM.Config}}{{.Subnet}}{{end}}'
# Output: 10.99.0.0/24 — no longer conflicts
```

### Key Lesson
**ALWAYS configure custom address pools** when Docker runs alongside VPNs or in environments with specific network requirements. The default pools (172.17-28.x.x, 192.168.x.x) conflict with many corporate networks.

---

## Session 5: Containers Lose Network After Firewall Reload

### Scenario
After running `firewall-cmd --reload` (or `ufw reload`), all Docker containers lose internet connectivity.

### Diagnosis

```bash
# Step 1: Test connectivity
docker exec webapp ping -c 1 8.8.8.8
# Output: ping: sendto: Operation not permitted

# Step 2: Check Docker iptables chains
sudo iptables -L DOCKER -n
# Output: Chain DOCKER (0 references)  — chain exists but no references

sudo iptables -L FORWARD -n
# Output: FORWARD chain shows DROP default, no Docker rules

# Step 3: The firewall reload flushed all iptables rules including Docker's NAT and filter chains
```

### Fix

```bash
# Restart Docker to regenerate all iptables rules
sudo systemctl restart docker

# Verify rules are back
sudo iptables -L DOCKER -n -v
# Output: Chain DOCKER with forwarding rules

sudo iptables -t nat -L POSTROUTING -n
# Output: MASQUERADE rule for Docker subnet

# Test connectivity
docker exec webapp ping -c 1 8.8.8.8
# Output: 64 bytes from 8.8.8.8
```

### Prevention

```bash
# Create a systemd override to restart Docker after firewall reload
# For firewalld:
sudo mkdir -p /etc/systemd/system/firewalld.service.d
cat <<'EOF' | sudo tee /etc/systemd/system/firewalld.service.d/docker.conf
[Service]
ExecStartPost=/usr/bin/systemctl restart docker
EOF
sudo systemctl daemon-reload
```

### Key Lesson
**ALWAYS restart Docker after any firewall changes.** Firewall reloads flush iptables rules, destroying Docker's network configuration. Automate this with systemd dependencies.

---

## Session 6: Compose Cross-Project Communication

### Scenario
Two Compose projects need to communicate. Project A (`frontend`) needs to reach Project B's (`backend`) API service, but `nslookup api` fails.

### Diagnosis

```bash
# Step 1: Check networks created by each project
docker network ls --filter "label=com.docker.compose.project=frontend"
# Output: frontend_default

docker network ls --filter "label=com.docker.compose.project=backend"
# Output: backend_default

# Step 2: Each project has its own isolated network — they cannot see each other
docker network inspect frontend_default --format='{{range .Containers}}{{.Name}} {{end}}'
# Output: frontend-web-1

docker network inspect backend_default --format='{{range .Containers}}{{.Name}} {{end}}'
# Output: backend-api-1 backend-db-1
```

### Fix

```bash
# Step 1: Create a shared external network
docker network create shared-services

# Step 2: Update frontend docker-compose.yml
```

```yaml
# frontend/docker-compose.yml
services:
  web:
    image: nginx
    networks:
      - default
      - shared

networks:
  shared:
    external: true
    name: shared-services
```

```yaml
# backend/docker-compose.yml
services:
  api:
    image: node:20-alpine
    networks:
      - default
      - shared

  db:
    image: postgres:16
    networks:
      - default  # NOT on shared network — stays isolated

networks:
  shared:
    external: true
    name: shared-services
```

```bash
# Step 3: Restart both projects
docker compose -f backend/docker-compose.yml up -d
docker compose -f frontend/docker-compose.yml up -d

# Step 4: Verify cross-project DNS
docker compose -f frontend/docker-compose.yml exec web nslookup api
# Output: Name: api  Address: 10.99.1.3
```

### Key Lesson
Compose projects are network-isolated by default. For cross-project communication, ALWAYS create a shared external network and add only the services that need to communicate.

---

## Session 7: Container Cannot Reach the Internet

### Scenario
A freshly installed Docker host. Containers can ping each other but cannot reach external hosts.

### Diagnosis

```bash
# Step 1: Test external connectivity
docker run --rm alpine ping -c 1 8.8.8.8
# Output: PING 8.8.8.8: sendto: Network is unreachable

# Step 2: Check IP forwarding
sysctl net.ipv4.ip_forward
# Output: net.ipv4.ip_forward = 0  — DISABLED

# Step 3: Check NAT rules
sudo iptables -t nat -L POSTROUTING -n
# Output: (empty or no MASQUERADE rule)
```

### Fix

```bash
# Step 1: Enable IP forwarding
sudo sysctl -w net.ipv4.ip_forward=1

# Step 2: Make persistent
echo "net.ipv4.ip_forward=1" | sudo tee -a /etc/sysctl.conf

# Step 3: Restart Docker to create NAT rules
sudo systemctl restart docker

# Step 4: Verify
docker run --rm alpine ping -c 1 8.8.8.8
# Output: 64 bytes from 8.8.8.8

sudo iptables -t nat -L POSTROUTING -n
# Output: MASQUERADE  all  --  172.17.0.0/16  !172.17.0.0/16
```

### Key Lesson
Docker requires `net.ipv4.ip_forward=1` for containers to reach the internet via NAT. ALWAYS verify this setting on fresh installations.

---

## Session 8: Health Check with Networking Dependency

### Scenario
Web application connects to database on startup. Compose starts both, but web crashes because database is not ready.

### Diagnosis

```bash
docker compose up
# webapp exits with: connection refused to db:5432
# Database is still initializing
```

### Fix

```yaml
# docker-compose.yml
services:
  db:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: secret
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 5s
      retries: 5
      start_period: 10s

  webapp:
    image: mywebapp
    depends_on:
      db:
        condition: service_healthy
    environment:
      DATABASE_URL: "postgres://postgres:secret@db:5432/app"
```

```bash
docker compose up -d
# webapp waits until db health check passes before starting

# Verify health
docker compose ps
# NAME    STATUS
# db      running (healthy)
# webapp  running
```

### Key Lesson
**ALWAYS use health checks with `condition: service_healthy`** for service dependencies. The `depends_on` without condition only waits for the container to start, NOT for the service to be ready.
