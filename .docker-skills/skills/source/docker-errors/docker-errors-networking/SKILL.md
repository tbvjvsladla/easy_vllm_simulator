---
name: docker-errors-networking
description: >
  Use when containers cannot reach each other, DNS resolution fails
  between services, or published ports are not accessible from the host.
  Prevents connectivity failures from using the default bridge network,
  which lacks automatic DNS resolution and proper isolation.
  Covers DNS failures, connection refused, port mapping issues, iptables
  conflicts, overlay network problems, subnet collisions.
  Keywords: docker network, DNS resolution, connection refused, bridge,
  overlay, iptables, port mapping, docker network inspect, subnet,
  can't reach container, port not accessible, network error, DNS not resolving.
license: MIT
compatibility: "Designed for Claude Code. Requires Docker Engine 24+."
metadata:
  author: OpenAEC-Foundation
  version: "1.0"
---

# docker-errors-networking

## Quick Reference

### Rule #1: ALWAYS Use User-Defined Networks

**NEVER use the default bridge network.** It lacks DNS resolution, proper isolation, and configuration flexibility. ALWAYS create a user-defined bridge network:

```bash
docker network create mynet
docker run --network mynet --name web nginx
docker run --network mynet --name api node
# api can reach web via hostname "web" — automatic DNS
```

### Network Debugging Flowchart

```
Container cannot communicate
        |
        v
[Are containers on the SAME network?]
  |                    |
  NO                   YES
  |                    |
  v                    v
docker network     [Can they ping by IP?]
connect mynet        |              |
container            NO             YES
  |                  |              |
  v                  v              v
Retry            [Check firewall/  [DNS issue — check
                  iptables]        container name and
                  See §Firewall    /etc/resolv.conf]
                                   See §DNS
        |
        v
[Can container reach internet?]
  |              |
  NO             YES
  |              |
  v              v
Check ip_forward Port mapping issue
and DNS config   See §Port Mapping
See §No Internet
```

---

## Diagnostic Table: Symptom > Cause > Fix

### DNS Resolution Failures

| Symptom | Cause | Fix |
|---------|-------|-----|
| `dial tcp: lookup <hostname>: no such host` | Containers on default bridge (no DNS) | ALWAYS use user-defined network: `docker network create mynet` |
| `dial tcp: lookup <hostname>: no such host` on custom network | Target container name misspelled or not running | Verify: `docker ps --filter network=mynet`. Use exact container name or network alias |
| DNS works by container name but not by service name | Using `docker run` instead of Compose | Use `--network-alias` for custom aliases: `docker run --network mynet --network-alias db postgres` |
| `Could not resolve host` for external domains | Container DNS misconfigured | Check: `docker exec <ctr> cat /etc/resolv.conf`. Fix: `docker run --dns 8.8.8.8` or set in daemon.json |
| `WARNING: Local (127.0.0.1) DNS resolver found in resolv.conf` | Host uses loopback DNS (systemd-resolved/dnsmasq) | Set DNS in `/etc/docker/daemon.json`: `{"dns": ["8.8.8.8", "8.8.4.4"]}` and restart Docker |

### Connection Refused

| Symptom | Cause | Fix |
|---------|-------|-----|
| `connection refused` between containers on same network | Target service not listening on 0.0.0.0 | NEVER bind to 127.0.0.1 inside container. ALWAYS bind to 0.0.0.0 |
| `connection refused` from host to container | Port not published or wrong port | Verify: `docker port <ctr>`. Publish: `docker run -p 8080:80` |
| `connection refused` — service starting slowly | Container healthy but service not ready | Add health check with `--health-cmd`. Use `depends_on` with `condition: service_healthy` in Compose |
| `connection refused` after container restart | IP address changed | NEVER hardcode container IPs. ALWAYS use container names or network aliases for DNS |

### Port Mapping Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| `port is already allocated` / `bind: address already in use` | Host port in use by another process | Find: `lsof -i :PORT` or `ss -tlnp \| grep PORT`. Stop process or use different host port |
| Published port not accessible from outside host | Binding to localhost only | Change `-p 127.0.0.1:8080:80` to `-p 8080:80` to bind all interfaces |
| Port published but no response | Container process crashed or not listening | Check: `docker logs <ctr>` and `docker exec <ctr> ss -tlnp` |
| `-P` maps to unexpected ports | `EXPOSE` in Dockerfile not matching actual service port | ALWAYS use explicit `-p host:container` instead of `-P` in production |

### Default Bridge Limitations

| Symptom | Cause | Fix |
|---------|-------|-----|
| Containers cannot reach each other by name | Default bridge lacks embedded DNS | Migrate to user-defined bridge: `docker network create mynet` |
| All containers see each other (no isolation) | Default bridge connects all unspecified containers | Use separate user-defined networks per application stack |
| Cannot connect/disconnect without restart | Default bridge does not support live operations | User-defined bridges support: `docker network connect/disconnect` |

### No Internet from Container

| Symptom | Cause | Fix |
|---------|-------|-----|
| `ping: bad address` or no route to host | IP forwarding disabled on host | Enable: `sysctl -w net.ipv4.ip_forward=1`. Persist in `/etc/sysctl.conf` |
| DNS works but HTTP times out | Firewall blocking outbound traffic | Check iptables FORWARD chain. Docker needs ACCEPT for its bridge subnets |
| `--network host` works but bridge does not | NAT/masquerade not working | Verify: `iptables -t nat -L POSTROUTING`. Restart Docker to rebuild rules |
| No connectivity after Docker upgrade | iptables rules lost | `sudo systemctl restart docker` to regenerate network rules |

### Firewall and iptables Conflicts

| Symptom | Cause | Fix |
|---------|-------|-----|
| `driver failed programming external connectivity` | iptables conflict or stale rules | Restart Docker: `sudo systemctl restart docker`. Check iptables rules |
| Firewalld/ufw blocking Docker traffic | Host firewall overriding Docker iptables | For ufw: allow Docker subnet. For firewalld: add Docker zone. Or set `"iptables": true` in daemon.json |
| `docker0` bridge disappears | NetworkManager or systemd-networkd managing Docker interfaces | Mark docker0 as unmanaged in NetworkManager or systemd-networkd config |
| Containers lose connectivity after firewall reload | Firewall flush removed Docker chains | ALWAYS restart Docker after firewall changes: `sudo systemctl restart docker` |

### Overlay Network Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| Cannot create overlay network | Swarm mode not initialized | Initialize: `docker swarm init` or join an existing swarm |
| Standalone containers cannot join overlay | Network not attachable | Create with `--attachable`: `docker network create -d overlay --attachable mynet` |
| Cross-host communication fails | Required ports blocked between hosts | Open: 2377/tcp (control), 4789/udp (VXLAN), 7946/tcp+udp (node discovery) |
| Encrypted overlay fails on Windows | Windows limitation | Encrypted overlay is NOT supported on Windows. Use unencrypted or different approach |

### Subnet Conflicts

| Symptom | Cause | Fix |
|---------|-------|-----|
| Containers cannot reach host network resources | Docker subnet overlaps with host/VPN network | Specify non-conflicting subnet: `docker network create --subnet=10.99.0.0/16 mynet` |
| VPN breaks after Docker install | Docker default pools conflict with VPN ranges | Configure in `/etc/docker/daemon.json`: `{"default-address-pools": [{"base": "10.99.0.0/16", "size": 24}]}` |
| `network X has active endpoints` when removing | Containers still connected | Disconnect all: `docker network disconnect -f mynet <ctr>` then remove |

---

## Network Debugging Commands

### Essential Diagnostic Commands

```bash
# Check which network a container is on
docker inspect --format='{{range $k, $v := .NetworkSettings.Networks}}{{$k}} {{end}}' <ctr>

# Get container IP address
docker inspect --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' <ctr>

# List all containers on a network
docker network inspect --format='{{range .Containers}}{{.Name}} {{end}}' mynet

# Check DNS resolution inside container
docker exec <ctr> nslookup <target-hostname>
docker exec <ctr> cat /etc/resolv.conf

# Test connectivity between containers
docker exec <ctr> ping -c 2 <target-hostname>
docker exec <ctr> wget -qO- http://<target-hostname>:<port>/

# Check listening ports inside container
docker exec <ctr> ss -tlnp
docker exec <ctr> netstat -tlnp

# Check published port mapping
docker port <ctr>

# Inspect full network configuration
docker network inspect mynet

# Check iptables rules (host)
sudo iptables -L -n -v
sudo iptables -t nat -L -n -v

# Check IP forwarding (host)
sysctl net.ipv4.ip_forward
```

### Compose-Specific Debugging

```bash
# Check default network created by Compose
docker network ls --filter "label=com.docker.compose.project=<project>"

# Verify service DNS names
docker compose exec <service> nslookup <other-service>

# Check Compose network config
docker compose config | grep -A 10 networks
```

---

## Compose Networking Patterns

### Correct: Services on Shared Network

```yaml
# docker-compose.yml
services:
  web:
    image: nginx
    ports:
      - "8080:80"  # Only needed for external access
    networks:
      - app-net

  api:
    image: node:20-alpine
    networks:
      - app-net  # Can reach "web" by hostname

networks:
  app-net:
    driver: bridge
```

### Correct: Isolated Backend Network

```yaml
services:
  web:
    networks:
      - frontend
      - backend

  api:
    networks:
      - backend

  db:
    networks:
      - backend  # Not accessible from frontend

networks:
  frontend:
  backend:
    internal: true  # No external internet access
```

### Anti-Pattern: Missing Network Declaration

```yaml
# NEVER rely on the default Compose network for multi-project setups
# ALWAYS declare explicit networks when services need cross-project communication
services:
  api:
    networks:
      - shared-net

networks:
  shared-net:
    external: true  # Must exist before compose up
```

---

## Critical Rules

**ALWAYS** use user-defined bridge networks -- the default bridge lacks DNS, isolation, and live connect/disconnect.

**ALWAYS** bind services to 0.0.0.0 inside containers -- binding to 127.0.0.1 makes the service unreachable from other containers.

**ALWAYS** restart Docker after firewall changes -- firewall reloads flush Docker's iptables chains.

**NEVER** hardcode container IP addresses -- IPs change on restart. Use DNS names or network aliases.

**NEVER** use `--link` -- it is legacy and deprecated. Use user-defined networks with DNS.

**NEVER** expose ports with `-p` for container-to-container communication -- containers on the same network can reach all ports directly.

---

## Reference Links

- [references/diagnostics.md](references/diagnostics.md) -- Complete error-to-cause-to-solution lookup table
- [references/examples.md](references/examples.md) -- Network debugging sessions with step-by-step resolution
- [references/anti-patterns.md](references/anti-patterns.md) -- Networking configuration mistakes and why they fail

### Official Sources

- https://docs.docker.com/engine/network/
- https://docs.docker.com/engine/network/drivers/bridge/
- https://docs.docker.com/engine/network/drivers/overlay/
- https://docs.docker.com/engine/daemon/troubleshoot/
