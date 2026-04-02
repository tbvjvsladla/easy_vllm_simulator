# Network Drivers Reference

## Bridge Driver (Default)

### Overview

The bridge driver creates an isolated network on a single Docker host. Containers on the same bridge can communicate; containers on different bridges cannot (unless connected to both).

Docker creates a default bridge network (`bridge`) at startup. **ALWAYS create user-defined bridge networks** instead of using the default.

### User-Defined Bridge Creation

```bash
# Basic bridge
docker network create mynet

# Bridge with full IPAM configuration
docker network create --driver bridge \
  --subnet=172.28.0.0/16 \
  --ip-range=172.28.5.0/24 \
  --gateway=172.28.5.254 \
  mynet

# Bridge with custom options
docker network create --driver bridge \
  -o com.docker.network.bridge.name=my-bridge0 \
  -o com.docker.network.bridge.enable_icc=true \
  -o com.docker.network.driver.mtu=1500 \
  mynet
```

### Bridge Driver Options (`-o`)

| Option | Default | Description |
|--------|---------|-------------|
| `com.docker.network.bridge.name` | auto | Linux bridge interface name |
| `com.docker.network.bridge.enable_ip_masquerade` | `true` | Enable NAT for outbound traffic |
| `com.docker.network.bridge.enable_icc` | `true` | Inter-container connectivity on this bridge |
| `com.docker.network.bridge.host_binding_ipv4` | `0.0.0.0` | Default IP for port binding |
| `com.docker.network.driver.mtu` | `0` (no limit) | Maximum Transmission Unit |
| `com.docker.network.container_iface_prefix` | `eth` | Container interface prefix |
| `com.docker.network.bridge.inhibit_ipv4` | `false` | Skip IPv4 gateway assignment |

### Default Bridge vs User-Defined Bridge -- Detailed Comparison

#### DNS Resolution

```bash
# Default bridge: NO DNS resolution -- must use IP or --link (deprecated)
docker run -d --name db postgres:16
docker run -it --rm alpine ping db
# ping: bad address 'db'  <-- FAILS

# User-defined bridge: AUTOMATIC DNS resolution by container name
docker network create mynet
docker run -d --name db --network mynet postgres:16
docker run -it --rm --network mynet alpine ping db
# PING db (172.20.0.2): 56 data bytes  <-- WORKS
```

#### Isolation

- **Default bridge**: Every container without `--network` joins the default bridge. ALL such containers can communicate with each other -- no isolation between unrelated applications.
- **User-defined bridge**: Only containers explicitly connected can communicate. Different applications use different networks for isolation.

#### Live Connect/Disconnect

```bash
# User-defined networks support hot-plugging
docker network connect mynet running-container
docker network disconnect mynet running-container

# Default bridge requires stopping and recreating the container
```

#### Configuration

- **Default bridge**: Configured via `daemon.json`, requires daemon restart to change
- **User-defined bridge**: Configured per-network at creation time, each network independent

### Scalability

Bridge networks become unstable at **1000+ containers per network** due to Linux kernel limitations. For large deployments, distribute containers across multiple networks.

---

## Host Driver

### Overview

The host driver removes network isolation entirely. The container shares the host's network namespace -- it uses the host's IP address and port space directly.

### Usage

```bash
docker run --network host nginx
# Nginx binds to host port 80 directly -- no NAT, no port mapping needed
```

### Characteristics

| Property | Value |
|----------|-------|
| Port mapping (`-p`) | NOT needed and NOT supported |
| Performance | Best (no NAT overhead) |
| Isolation | None -- container sees all host interfaces |
| DNS | Uses host's DNS directly |
| Platform | Linux only (on Docker Desktop, "host" means the VM, not your machine) |

### When to Use Host Networking

- **Performance-critical** applications where NAT overhead matters
- Applications that need to **bind many ports dynamically** (port mapping impractical)
- Network monitoring tools that need to see all host traffic
- Applications that **must advertise their real host IP** to external systems

### When NOT to Use Host Networking

- **NEVER** use in production if isolation is a security requirement
- **NEVER** use when multiple containers need the same port
- **NEVER** use on Docker Desktop expecting to reach the physical host network

---

## Overlay Driver

### Overview

Overlay networks span multiple Docker hosts using Swarm mode. They use VXLAN encapsulation to create a virtual Layer 2 network across hosts.

### Prerequisites

- Docker Swarm mode initialized (`docker swarm init`)
- Required ports open between hosts:
  - **2377/tcp** -- Swarm management
  - **4789/udp** -- VXLAN overlay traffic
  - **7946/tcp+udp** -- Node discovery and gossip

### Creation

```bash
# Basic overlay (Swarm services only)
docker network create -d overlay my-overlay

# Attachable overlay (standalone containers + Swarm services)
docker network create -d overlay --attachable my-overlay

# Encrypted overlay (IPsec encryption on VXLAN)
docker network create -d overlay --opt encrypted --attachable secure-overlay
```

### Overlay Driver Options

| Option | Default | Description |
|--------|---------|-------------|
| `encrypted` | `false` | Enable IPSEC encryption on VXLAN |
| `com.docker.network.driver.mtu` | `1450` | VXLAN MTU (lower than bridge due to encapsulation) |

### Characteristics

| Property | Value |
|----------|-------|
| Multi-host | Yes (Swarm required) |
| Encryption | Optional via `--opt encrypted` |
| Service discovery | Automatic via Swarm DNS |
| Load balancing | Built-in via Swarm routing mesh |
| Platform | Linux only for encryption |

### Limitations

- Windows containers CANNOT use encrypted overlay networks
- Same 1000-container-per-host scalability limit applies
- Encryption adds performance overhead (IPsec)
- `--attachable` is required for standalone containers (non-service)

---

## Macvlan Driver

### Overview

Macvlan assigns a unique MAC address to each container, making it appear as a physical device on the network. Containers get IP addresses from the physical network's DHCP server or static assignment.

### Creation

```bash
# Macvlan with static subnet
docker network create -d macvlan \
  --subnet=192.168.1.0/24 \
  --gateway=192.168.1.1 \
  -o parent=eth0 \
  my-macvlan

# Macvlan with VLAN tagging (802.1Q trunk)
docker network create -d macvlan \
  --subnet=192.168.50.0/24 \
  --gateway=192.168.50.1 \
  -o parent=eth0.50 \
  my-macvlan-vlan50
```

### Macvlan Modes

| Mode | Description |
|------|-------------|
| **bridge** (default) | Containers can communicate with each other and external network |
| **passthru** | Single container directly attached to parent interface |

### Macvlan Driver Options

| Option | Description |
|--------|-------------|
| `parent` | Host interface to attach to (REQUIRED) |
| `macvlan_mode` | `bridge` (default) or `passthru` |

### Characteristics

| Property | Value |
|----------|-------|
| MAC address | Unique per container |
| IP address | From physical network range |
| Host communication | NOT possible by default (requires macvlan on host interface too) |
| Promiscuous mode | Required on parent interface |
| Use case | Legacy apps that need LAN presence, IoT, network appliances |

### Limitations

- **Host cannot communicate with macvlan containers** without additional configuration (create a macvlan sub-interface on the host)
- Requires **promiscuous mode** on the parent interface
- Many cloud providers and wireless interfaces **block promiscuous mode**
- Each container consumes a MAC address -- some switches limit MAC addresses per port

---

## IPvlan Driver

### Overview

IPvlan is similar to macvlan but ALL containers share the parent interface's MAC address. Each container gets its own IP address. This avoids the MAC-per-container overhead and works where promiscuous mode is blocked.

### Creation

```bash
# IPvlan L2 mode (default)
docker network create -d ipvlan \
  --subnet=192.168.1.0/24 \
  --gateway=192.168.1.1 \
  -o parent=eth0 \
  my-ipvlan

# IPvlan L3 mode (routing, no bridge)
docker network create -d ipvlan \
  --subnet=192.168.100.0/24 \
  -o parent=eth0 \
  -o ipvlan_mode=l3 \
  my-ipvlan-l3
```

### IPvlan Modes

| Mode | Layer | Description |
|------|-------|-------------|
| **l2** (default) | Layer 2 | Behaves like macvlan but with shared MAC |
| **l3** | Layer 3 | Pure routing mode -- no broadcast, no ARP |
| **l3s** | Layer 3 + source | L3 with source-based routing and iptables integration |

### IPvlan vs Macvlan

| Feature | Macvlan | IPvlan |
|---------|---------|--------|
| MAC address | Unique per container | Shared (parent's MAC) |
| Promiscuous mode | Required | NOT required |
| Cloud compatibility | Often blocked | Generally works |
| L3 routing | No | Yes (l3 mode) |
| Broadcast/multicast | Yes | No (in l3 mode) |

### When to Choose IPvlan over Macvlan

- Cloud environments that **block promiscuous mode**
- Switches with **MAC address table limits**
- When L3 routing mode is needed
- Wireless interfaces (no promiscuous mode support)

---

## None Driver

### Overview

The none driver provides complete network isolation. The container has only a loopback interface -- no external connectivity.

### Usage

```bash
docker run --network none alpine ip addr
# Only shows lo (127.0.0.1)
```

### When to Use

- **Batch processing** containers that need no network access
- **Security-sensitive** workloads that must be completely isolated
- Containers that communicate ONLY through volumes or shared memory
- Testing network failure scenarios

---

## Subnet Allocation

### Default Address Pools

Docker allocates subnets from built-in pools:
- `172.17.0.0/16` through `172.28.0.0/14`
- `192.168.0.0/16`

### Custom Address Pools

Configure in `/etc/docker/daemon.json`:

```json
{
  "default-address-pools": [
    { "base": "10.10.0.0/16", "size": 24 },
    { "base": "172.30.0.0/16", "size": 24 }
  ]
}
```

Each new network gets a `/24` (or configured `size`) from the pool automatically.

### Gateway Priority

When a container connects to multiple networks, the default gateway is selected by `gw-priority` (highest value wins, default: 0):

```bash
docker run --network name=primary,gw-priority=1 --network secondary myimage
# Default gateway comes from "primary" network
```

---

## Official Sources

- https://docs.docker.com/engine/network/
- https://docs.docker.com/engine/network/drivers/bridge/
- https://docs.docker.com/engine/network/drivers/overlay/
- https://docs.docker.com/engine/network/drivers/host/
- https://docs.docker.com/engine/network/drivers/macvlan/
- https://docs.docker.com/engine/network/drivers/ipvlan/
- https://docs.docker.com/engine/network/drivers/none/
