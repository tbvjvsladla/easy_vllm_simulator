# Networking Examples

## 1. Basic User-Defined Bridge Network

```bash
# Create network
docker network create app-net

# Run database
docker run -d \
  --name postgres \
  --network app-net \
  -e POSTGRES_PASSWORD=secret \
  postgres:16

# Run application -- connects to postgres by name
docker run -d \
  --name api \
  --network app-net \
  -e DATABASE_URL=postgresql://postgres:secret@postgres:5432/mydb \
  -p 3000:3000 \
  myapi:latest

# api resolves "postgres" via Docker DNS automatically
```

---

## 2. Multi-Tier Network Isolation

Separate frontend, backend, and database tiers. Only the API can reach both the web tier and the database tier.

```bash
# Create isolated networks
docker network create frontend
docker network create backend --internal  # No internet access

# Web server -- frontend only, published to host
docker run -d --name web --network frontend -p 80:80 nginx

# API server -- connected to BOTH networks
docker run -d --name api --network frontend myapi
docker network connect backend api

# Database -- backend only (unreachable from web, no internet)
docker run -d --name db --network backend \
  -e POSTGRES_PASSWORD=secret \
  postgres:16

# Result:
# web -> api  (via frontend network)  OK
# api -> db   (via backend network)   OK
# web -> db   (no shared network)     BLOCKED
# db -> internet (--internal)         BLOCKED
```

---

## 3. Docker Compose Multi-Network Setup

```yaml
services:
  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    networks:
      - frontend
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro

  api:
    image: myapi:latest
    networks:
      - frontend
      - backend
    environment:
      - DATABASE_URL=postgresql://user:pass@db:5432/mydb
      - REDIS_URL=redis://cache:6379

  db:
    image: postgres:16
    networks:
      - backend
    volumes:
      - pgdata:/var/lib/postgresql/data
    environment:
      - POSTGRES_PASSWORD=pass

  cache:
    image: redis:7-alpine
    networks:
      - backend

networks:
  frontend:
    driver: bridge
  backend:
    driver: bridge
    internal: true

volumes:
  pgdata:
```

---

## 4. Network Aliases for Service Discovery

Multiple containers behind a single DNS name for client-side round-robin:

```bash
docker network create search-net

# Launch multiple Elasticsearch instances with same alias
docker run -d --name es1 --network search-net --network-alias search elasticsearch:8
docker run -d --name es2 --network search-net --network-alias search elasticsearch:8
docker run -d --name es3 --network search-net --network-alias search elasticsearch:8

# Client resolves "search" -- Docker rotates IP responses
docker run --rm --network search-net alpine nslookup search
# Returns all three IPs in round-robin order
```

In Compose:

```yaml
services:
  worker:
    image: myworker
    deploy:
      replicas: 3
    networks:
      app-net:
        aliases:
          - workers
```

---

## 5. Static IP Assignment

```bash
# Network MUST have a subnet defined for static IPs
docker network create --subnet=172.28.0.0/16 static-net

# Assign specific IPs
docker run -d --name dns-server \
  --network static-net \
  --ip 172.28.0.53 \
  coredns/coredns

docker run -d --name gateway \
  --network static-net \
  --ip 172.28.0.1 \
  mygateway
```

---

## 6. Container Sharing Network Namespace

Two containers share the same network namespace -- they communicate via localhost:

```bash
# Redis binds to localhost only
docker run -d --name redis redis:7 --bind 127.0.0.1

# Second container shares redis's network stack
docker run --rm -it --network container:redis redis:7 redis-cli -h 127.0.0.1
# Connects successfully via shared loopback
```

---

## 7. Host Network for Performance

```bash
# No port mapping needed -- binds directly to host ports
docker run -d --name perf-app --network host myapp

# Application at host-ip:8080 directly
# No NAT overhead, maximum throughput
```

---

## 8. Internal Network (No External Access)

```bash
# Create network with no internet access
docker network create --internal secure-net

# Containers can communicate with each other but NOT the internet
docker run -d --name app1 --network secure-net alpine sleep 3600
docker run -d --name app2 --network secure-net alpine sleep 3600

# app1 can reach app2
docker exec app1 ping -c 1 app2  # WORKS

# app1 cannot reach the internet
docker exec app1 ping -c 1 8.8.8.8  # FAILS (no route)
```

---

## 9. Macvlan -- Container on Physical LAN

```bash
# Create macvlan network attached to physical interface
docker network create -d macvlan \
  --subnet=192.168.1.0/24 \
  --gateway=192.168.1.1 \
  -o parent=eth0 \
  lan-net

# Container appears as device 192.168.1.100 on the physical network
docker run -d --name iot-bridge \
  --network lan-net \
  --ip 192.168.1.100 \
  my-iot-app

# Other devices on 192.168.1.0/24 can reach this container directly
```

---

## 10. Overlay Network (Multi-Host Swarm)

```bash
# On manager node
docker swarm init

# Create overlay network
docker network create -d overlay --attachable my-overlay

# Deploy service across multiple hosts
docker service create --name web \
  --network my-overlay \
  --replicas 3 \
  -p 80:80 \
  nginx

# Standalone container can also join (because --attachable)
docker run -d --name debug --network my-overlay alpine sleep 3600
```

---

## 11. Encrypted Overlay Network

```bash
# Create encrypted overlay (IPsec between hosts)
docker network create -d overlay \
  --opt encrypted \
  --attachable \
  secure-overlay

# All VXLAN traffic between hosts is encrypted
docker service create --name secure-web \
  --network secure-overlay \
  nginx
```

---

## 12. Custom IPAM Configuration

```bash
# Network with specific subnet, allocation range, and gateway
docker network create \
  --driver bridge \
  --subnet=10.100.0.0/16 \
  --ip-range=10.100.1.0/24 \
  --gateway=10.100.0.1 \
  --aux-address="reserved1=10.100.1.1" \
  custom-net

# Containers get IPs from 10.100.1.0/24 range
# 10.100.1.1 is reserved and won't be assigned
```

---

## 13. IPv6 Networking

```bash
# Dual-stack network (IPv4 + IPv6)
docker network create --ipv6 \
  --subnet=172.28.0.0/16 \
  --subnet=2001:db8::/64 \
  dual-stack-net

# IPv6-only network
docker network create --ipv4=false --ipv6 \
  --subnet=2001:db8:1::/64 \
  v6only-net
```

---

## 14. Connecting a Running Container to Additional Networks

```bash
# Container starts on one network
docker run -d --name multi-net-app --network frontend myapp

# Hot-plug additional network
docker network connect backend multi-net-app

# Optionally with static IP and alias
docker network connect --ip 172.28.5.10 --alias myservice backend multi-net-app

# Disconnect when no longer needed
docker network disconnect frontend multi-net-app
```

---

## 15. Network Debugging Session

```bash
# 1. Check which networks a container belongs to
docker inspect --format='{{range $k, $v := .NetworkSettings.Networks}}{{$k}}: {{$v.IPAddress}}{{"\n"}}{{end}}' myapp

# 2. Check DNS resolution from inside the container
docker exec myapp nslookup target-container
docker exec myapp cat /etc/resolv.conf

# 3. Test connectivity
docker exec myapp ping -c 2 target-container
docker exec myapp wget -qO- http://target-container:8080/health

# 4. Check which containers are on a network
docker network inspect mynet --format='{{range .Containers}}{{.Name}} ({{.IPv4Address}}){{"\n"}}{{end}}'

# 5. Check port bindings
docker port myapp

# 6. Check if ports are exposed to host
docker inspect --format='{{range $p, $conf := .NetworkSettings.Ports}}{{$p}} -> {{if $conf}}{{(index $conf 0).HostPort}}{{else}}not published{{end}}{{"\n"}}{{end}}' myapp
```

---

## 16. Localhost Port Binding (Security)

```bash
# INSECURE: Binds to all interfaces (0.0.0.0)
docker run -p 5432:5432 postgres:16

# SECURE: Binds to localhost only
docker run -p 127.0.0.1:5432:5432 postgres:16

# Use specific interface IP
docker run -p 10.0.0.5:8080:80 nginx
```

---

## Official Sources

- https://docs.docker.com/engine/network/
- https://docs.docker.com/engine/network/drivers/bridge/
- https://docs.docker.com/engine/network/drivers/overlay/
- https://docs.docker.com/engine/network/drivers/macvlan/
- https://docs.docker.com/compose/how-tos/networking/
