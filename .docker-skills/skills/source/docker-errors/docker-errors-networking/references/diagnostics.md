# Network Error Diagnostics — Complete Reference

> Full error-to-cause-to-solution lookup table for Docker networking issues.
> Docker Engine 24+, Docker Compose v2.

---

## DNS Resolution Errors

| # | Error / Symptom | Root Cause | Solution | Verify Fix |
|---|----------------|------------|----------|------------|
| N-001 | `dial tcp: lookup <hostname>: no such host` | Containers on default bridge network (no embedded DNS) | ALWAYS use user-defined network: `docker network create mynet && docker run --network mynet` | `docker exec <ctr> nslookup <target>` returns IP |
| N-002 | `dial tcp: lookup <hostname>: no such host` on user-defined network | Target container not running or name misspelled | Check: `docker ps --filter network=mynet`. Verify exact container name matches lookup target | `docker network inspect mynet` shows both containers |
| N-003 | DNS resolves but wrong IP returned | Stale DNS cache or container recreated with new IP | Restart client container. NEVER cache IPs — Docker DNS handles resolution | `docker exec <ctr> nslookup <target>` shows correct IP |
| N-004 | `Could not resolve host: github.com` | Container cannot reach external DNS | Check `docker exec <ctr> cat /etc/resolv.conf`. Set DNS: `docker run --dns 8.8.8.8` | `docker exec <ctr> nslookup github.com` succeeds |
| N-005 | `WARNING: Local (127.0.0.1) DNS resolver found in resolv.conf` | Host uses loopback DNS resolver (systemd-resolved, dnsmasq) | Set in `/etc/docker/daemon.json`: `{"dns": ["8.8.8.8", "8.8.4.4"]}`. Restart Docker | Warning disappears from `docker run` output |
| N-006 | DNS works with `--network host` but not bridge | Docker DNS server (127.0.0.11) not reachable | Check iptables NAT rules. Restart Docker to regenerate: `sudo systemctl restart docker` | `docker exec <ctr> cat /etc/resolv.conf` shows 127.0.0.11 |
| N-007 | Service name resolution fails in `docker run` | `docker run` does not support Compose service names | Use `--network-alias`: `docker run --network mynet --network-alias db postgres` | `docker exec <client> nslookup db` resolves |
| N-008 | DNS resolution intermittently fails | Docker embedded DNS overwhelmed or host DNS flaky | Add fallback DNS: `docker run --dns 8.8.8.8 --dns 1.1.1.1`. Check host DNS stability | Monitor with repeated `nslookup` from inside container |

---

## Connection Refused Errors

| # | Error / Symptom | Root Cause | Solution | Verify Fix |
|---|----------------|------------|----------|------------|
| N-010 | `connection refused` between containers on same network | Service binding to 127.0.0.1 instead of 0.0.0.0 | Configure service to listen on `0.0.0.0`. Example: `node --host 0.0.0.0` | `docker exec <target> ss -tlnp` shows `0.0.0.0:<port>` |
| N-011 | `connection refused` from host to published port | Service not started yet or crashed | Check: `docker logs <ctr>`. Verify service health: `docker inspect --format='{{.State.Health.Status}}' <ctr>` | `curl localhost:<host-port>` succeeds |
| N-012 | `connection refused` after container restart | Client caching old IP address | NEVER hardcode IPs. ALWAYS use DNS names. Restart client if it cached the IP | Connection succeeds using hostname |
| N-013 | `connection refused` — health check passes but app unreachable | Health check URL differs from application endpoint | Verify health check tests the actual service port. Check: `docker inspect --format='{{json .State.Health}}' <ctr>` | Application responds on expected port |
| N-014 | `connection refused` to database on startup | Database not ready when app starts | Use `depends_on` with `condition: service_healthy` in Compose. Add retry logic in application | App connects after DB health check passes |
| N-015 | `connection refused` between containers on DIFFERENT networks | Networks are isolated by design | Connect containers to a shared network: `docker network connect shared-net <ctr>` | Both containers visible in `docker network inspect shared-net` |

---

## Port Mapping Errors

| # | Error / Symptom | Root Cause | Solution | Verify Fix |
|---|----------------|------------|----------|------------|
| N-020 | `port is already allocated` | Another process (or container) already bound to that host port | Find: `lsof -i :<port>` or `ss -tlnp \| grep <port>`. Stop conflicting process or choose different port | `docker run -p <new-port>:<ctr-port>` succeeds |
| N-021 | `bind: address already in use` | Same as N-020 — different OS error message | Same as N-020 | Same as N-020 |
| N-022 | Published port not reachable from other machines | Port bound to localhost only (`-p 127.0.0.1:8080:80`) | Remove IP binding: `-p 8080:80` binds to all interfaces | `curl <host-ip>:8080` from remote machine succeeds |
| N-023 | `-P` maps to random high ports | `-P` publishes all EXPOSE ports to random host ports | ALWAYS use explicit `-p host:container` in production. Use `docker port <ctr>` to find mappings | `docker port <ctr>` shows expected mapping |
| N-024 | Port mapping exists but no response | Container process not listening on the container port | Check: `docker exec <ctr> ss -tlnp`. Verify EXPOSE matches actual service port | `ss -tlnp` output shows service on expected port |
| N-025 | UDP port not working | Default protocol is TCP | Specify UDP: `-p 5060:5060/udp` or both: `-p 5060:5060/tcp -p 5060:5060/udp` | `docker port <ctr>` shows UDP mapping |
| N-026 | Port range mapping fails | Ranges must match in size | Ensure equal ranges: `-p 8000-8010:8000-8010` (same count of ports) | `docker port <ctr>` lists all range mappings |

---

## Firewall and iptables Errors

| # | Error / Symptom | Root Cause | Solution | Verify Fix |
|---|----------------|------------|----------|------------|
| N-030 | `driver failed programming external connectivity on endpoint` | iptables rules corrupted or conflicting | Restart Docker: `sudo systemctl restart docker` | Container starts with port mapping |
| N-031 | Containers lose network after `firewall-cmd --reload` or `ufw reload` | Firewall flush removes Docker iptables chains | ALWAYS restart Docker after firewall changes: `sudo systemctl restart docker` | `iptables -L DOCKER` shows rules |
| N-032 | ufw blocking container traffic despite allowing port | ufw FORWARD chain drops Docker traffic | Add ufw rule for Docker subnet or edit `/etc/ufw/after.rules` to allow Docker bridge | Container traffic flows through firewall |
| N-033 | firewalld blocking Docker traffic | Docker zone not configured in firewalld | Add Docker interface to trusted zone: `firewall-cmd --zone=trusted --add-interface=docker0 --permanent` | `firewall-cmd --get-active-zones` shows docker0 |
| N-034 | `docker0` bridge interface disappears | NetworkManager or systemd-networkd managing Docker interfaces | Mark unmanaged. For NM: `[keyfile]\nunmanaged-devices=interface-name:docker*` in `/etc/NetworkManager/conf.d/` | `ip link show docker0` exists after restart |
| N-035 | Container-to-container blocked despite same network | ICC (inter-container connectivity) disabled | Check bridge option: `docker network inspect mynet`. Create with ICC enabled (default) | Containers can ping each other |

---

## Internet Connectivity Errors

| # | Error / Symptom | Root Cause | Solution | Verify Fix |
|---|----------------|------------|----------|------------|
| N-040 | Container cannot reach any external IP | IP forwarding disabled on host | Enable: `sysctl -w net.ipv4.ip_forward=1`. Persist: add `net.ipv4.ip_forward=1` to `/etc/sysctl.conf` | `docker exec <ctr> ping -c 1 8.8.8.8` succeeds |
| N-041 | DNS resolves but HTTP/HTTPS times out | Firewall blocking outbound traffic on FORWARD chain | Check: `iptables -L FORWARD -n -v`. Allow Docker subnet outbound traffic | `docker exec <ctr> wget -qO- http://example.com` succeeds |
| N-042 | `--network host` has internet but bridge does not | NAT/masquerade rules missing | Restart Docker to rebuild: `sudo systemctl restart docker`. Check `iptables -t nat -L POSTROUTING` | MASQUERADE rule exists for Docker subnet |
| N-043 | Internet works for some containers, not others | Container on `--internal` network | Check: `docker network inspect <net>`. Internal networks block all external traffic by design | Move container to non-internal network |
| N-044 | No connectivity after Docker/system upgrade | iptables rules not regenerated | `sudo systemctl restart docker` | `iptables -L DOCKER -n` shows populated chain |
| N-045 | Proxy-related connection failures | Container needs HTTP_PROXY set | Set via env: `docker run -e HTTP_PROXY=http://proxy:3128 -e HTTPS_PROXY=http://proxy:3128` | `docker exec <ctr> wget http://example.com` through proxy |

---

## Overlay Network Errors

| # | Error / Symptom | Root Cause | Solution | Verify Fix |
|---|----------------|------------|----------|------------|
| N-050 | `network mynet not found` for overlay | Swarm mode not initialized | `docker swarm init` on manager node | `docker network create -d overlay mynet` succeeds |
| N-051 | Standalone container cannot join overlay | Network not created as attachable | Recreate: `docker network create -d overlay --attachable mynet` | `docker run --network mynet` works for standalone containers |
| N-052 | Cross-host overlay communication fails | Swarm ports blocked between hosts | Open: 2377/tcp, 4789/udp, 7946/tcp+udp between all Swarm nodes | `docker node ls` shows all nodes as Ready |
| N-053 | Overlay encryption fails on Windows | Windows does not support IPsec on overlay | Remove `--opt encrypted`. Use application-level TLS instead | Overlay works without encryption flag |
| N-054 | Overlay network performance degraded | VXLAN overhead + encryption overhead | Benchmark with/without encryption. Consider host networking for latency-critical services | `iperf3` shows acceptable throughput |

---

## Subnet Conflict Errors

| # | Error / Symptom | Root Cause | Solution | Verify Fix |
|---|----------------|------------|----------|------------|
| N-060 | VPN connection breaks after Docker starts | Docker default subnet (172.17.0.0/16) overlaps VPN range | Set custom pools in `/etc/docker/daemon.json`: `{"default-address-pools": [{"base": "10.99.0.0/16", "size": 24}]}` | VPN and Docker work simultaneously |
| N-061 | Cannot reach specific host network IPs | Docker network overlaps with host LAN subnet | Create network with non-conflicting subnet: `docker network create --subnet=10.99.0.0/16 mynet` | Container can reach LAN IPs |
| N-062 | `network X has active endpoints` when removing network | Containers still connected to network | Disconnect all: `docker network disconnect -f mynet <ctr>` for each container, then remove | `docker network rm mynet` succeeds |
| N-063 | `Pool overlaps with other one on this address space` | Another Docker network already uses requested subnet | Choose different subnet or remove conflicting network: `docker network ls` to find it | New network creates with desired subnet |
| N-064 | All Docker default subnets exhausted | Too many networks created consuming all default pool space | Increase pool or use larger subnets. Clean unused: `docker network prune` | New networks create successfully |

---

## Docker Compose Networking Errors

| # | Error / Symptom | Root Cause | Solution | Verify Fix |
|---|----------------|------------|----------|------------|
| N-070 | Service A cannot reach Service B by name | Services on different explicitly declared networks | Ensure both services list the same network in their `networks:` key | `docker compose exec A nslookup B` resolves |
| N-071 | `network X declared as external, but could not be found` | External network does not exist yet | Create first: `docker network create X` before `docker compose up` | `docker compose up` succeeds |
| N-072 | Port conflict between Compose projects | Two projects mapping same host port | Use different host ports per project or use `host_ip` to bind to different interfaces | Both projects start without port conflict |
| N-073 | Container names conflict across Compose projects | Custom `container_name` collides | Remove `container_name` — let Compose auto-generate unique names. Or use distinct names | `docker compose up` for both projects succeeds |
| N-074 | Cross-project service communication fails | Each Compose project creates its own isolated network | Create shared external network. Declare in both compose files with `external: true` | Services across projects can reach each other |
| N-075 | DNS resolution uses wrong container after scale | Compose round-robins between replicas | This is expected behavior. For sticky sessions, use application-level routing | Verify with repeated `nslookup` showing different IPs |

---

## Diagnostic Command Reference

### Quick Diagnosis Checklist

```bash
# 1. Are containers on the same network?
docker inspect --format='{{range $k, $v := .NetworkSettings.Networks}}{{$k}} {{end}}' <ctr1>
docker inspect --format='{{range $k, $v := .NetworkSettings.Networks}}{{$k}} {{end}}' <ctr2>

# 2. Can they resolve each other?
docker exec <ctr1> nslookup <ctr2-name>

# 3. Can they ping each other?
docker exec <ctr1> ping -c 2 <ctr2-name>

# 4. Is the service listening?
docker exec <target> ss -tlnp

# 5. What does the DNS config look like?
docker exec <ctr> cat /etc/resolv.conf

# 6. What is the full network config?
docker network inspect <network-name>

# 7. What ports are published?
docker port <ctr>

# 8. Is IP forwarding on?
sysctl net.ipv4.ip_forward

# 9. Are Docker iptables chains present?
sudo iptables -L DOCKER -n -v 2>/dev/null || echo "No DOCKER chain"

# 10. Docker daemon logs
journalctl -u docker --no-pager -n 50
```
