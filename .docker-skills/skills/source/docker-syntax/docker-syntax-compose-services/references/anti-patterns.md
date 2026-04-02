# Service Configuration Anti-Patterns

Common mistakes in Docker Compose service configuration with explanations and corrections.

---

## AP-001: depends_on Without Health Conditions

**Problem**: `depends_on` in short form only waits for the container to start, NOT for the application to be ready. A database container starts in milliseconds, but PostgreSQL may need seconds to initialize.

```yaml
# WRONG -- app starts before database accepts connections
services:
  app:
    depends_on:
      - db
  db:
    image: postgres:16
```

```yaml
# CORRECT -- app waits for database to pass healthcheck
services:
  app:
    depends_on:
      db:
        condition: service_healthy
  db:
    image: postgres:16
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 30s
```

**Why**: Without `condition: service_healthy`, race conditions cause intermittent startup failures. The app connects before the database is ready, crashes, and must be manually restarted.

---

## AP-002: Hardcoded Secrets in Environment

**Problem**: Secrets in plain text in `compose.yaml` get committed to version control.

```yaml
# WRONG -- credentials in source control
services:
  db:
    environment:
      POSTGRES_PASSWORD: "super-secret-password"
```

```yaml
# CORRECT -- use interpolation with required check
services:
  db:
    environment:
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:?Database password is required}
```

Or use Docker secrets:

```yaml
# BEST -- secrets mounted as files, never in environment
services:
  db:
    secrets:
      - db_password
    environment:
      POSTGRES_PASSWORD_FILE: /run/secrets/db_password

secrets:
  db_password:
    file: ./secrets/db_password.txt
```

**Why**: Environment variables appear in `docker inspect`, process listings, and logs. Secrets are mounted as files with restricted permissions.

---

## AP-003: container_name on Scalable Services

**Problem**: `container_name` sets a fixed name. Container names must be unique, so scaling fails.

```yaml
# WRONG -- cannot run docker compose up --scale web=3
services:
  web:
    image: nginx
    container_name: my-nginx
```

```yaml
# CORRECT -- let Compose manage container names
services:
  web:
    image: nginx
```

**Why**: Compose generates unique names like `project-web-1`, `project-web-2`. A fixed `container_name` makes the second instance fail with a name conflict.

---

## AP-004: No Resource Limits

**Problem**: Without memory limits, a single container with a memory leak can consume all host memory and crash other containers or the host itself.

```yaml
# WRONG -- no limits, can consume unlimited resources
services:
  app:
    image: myapp
    restart: always
```

```yaml
# CORRECT -- explicit limits prevent resource exhaustion
services:
  app:
    image: myapp
    restart: unless-stopped
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 512M
        reservations:
          cpus: '0.25'
          memory: 128M
```

**Why**: `restart: always` combined with no limits creates a crash loop that consumes increasing resources. Each restart may leak more memory until the host is unresponsive.

---

## AP-005: Anonymous Volumes for Persistent Data

**Problem**: Anonymous volumes are recreated on `docker compose down` and data is permanently lost.

```yaml
# WRONG -- data lost when containers are removed
services:
  db:
    image: postgres
    volumes:
      - /var/lib/postgresql/data
```

```yaml
# CORRECT -- named volume persists across compose down/up
services:
  db:
    image: postgres
    volumes:
      - db-data:/var/lib/postgresql/data

volumes:
  db-data:
```

**Why**: Anonymous volumes have random names and are not tracked. `docker compose down` removes them. Named volumes declared in the top-level `volumes:` section survive `docker compose down` (unless `--volumes` flag is used).

---

## AP-006: Exposing Ports to All Interfaces

**Problem**: Default port mapping binds to `0.0.0.0`, making the service accessible from any network interface including public ones.

```yaml
# WRONG -- accessible from any network interface
ports:
  - "8080:80"
  - "5432:5432"
```

```yaml
# CORRECT -- bind to localhost for local-only access
ports:
  - "127.0.0.1:8080:80"
  - "127.0.0.1:5432:5432"
```

**Why**: On a server with a public IP, `"5432:5432"` exposes PostgreSQL to the internet. ALWAYS bind to `127.0.0.1` for services that should only be accessed locally or through a reverse proxy.

---

## AP-007: Using `version:` Field

**Problem**: The `version` field is deprecated and ignored by modern Compose. It provides no value and confuses users about compatibility.

```yaml
# WRONG -- deprecated, provides no functionality
version: "3.8"
services:
  web:
    image: nginx
```

```yaml
# CORRECT -- version field is not needed
services:
  web:
    image: nginx
```

**Why**: The unified Compose Specification replaced legacy versions 2.x and 3.x. Modern Compose ignores this field entirely.

---

## AP-008: Running as Root Without Necessity

**Problem**: Containers run as root by default, giving a compromised process unnecessary privileges.

```yaml
# WRONG -- runs as root
services:
  app:
    image: myapp
```

```yaml
# CORRECT -- non-root user, minimal capabilities
services:
  app:
    image: myapp
    user: "1000:1000"
    read_only: true
    cap_drop:
      - ALL
    cap_add:
      - NET_BIND_SERVICE
    security_opt:
      - no-new-privileges:true
    tmpfs:
      - /tmp
```

**Why**: If an attacker exploits a vulnerability in the application, running as root gives them full control over the container. Combined with `read_only: true` and dropped capabilities, the attack surface is minimized.

---

## AP-009: No Healthcheck on Dependency Services

**Problem**: Without a healthcheck, there is no way to use `condition: service_healthy` in `depends_on`. Services that depend on this service can only use `service_started`, which is unreliable.

```yaml
# WRONG -- no healthcheck, dependents cannot wait for readiness
services:
  redis:
    image: redis:7
  app:
    depends_on:
      - redis
```

```yaml
# CORRECT -- healthcheck enables reliable dependency ordering
services:
  redis:
    image: redis:7
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 3
  app:
    depends_on:
      redis:
        condition: service_healthy
```

**Why**: EVERY service that other services depend on MUST have a healthcheck. Without it, dependent services start before the dependency is actually ready to handle requests.

---

## AP-010: Using `restart: "no"` Without Quotes

**Problem**: YAML interprets bare `no` as boolean `false`, which is not the same as the string `"no"`.

```yaml
# WRONG -- YAML parses this as boolean false
restart: no
```

```yaml
# CORRECT -- quoted string
restart: "no"
```

**Why**: Most Compose implementations handle this gracefully, but it is technically incorrect YAML. ALWAYS quote `"no"` to ensure correct parsing.

---

## AP-011: Mixing Network Isolation with network_mode

**Problem**: Using `network_mode: host` bypasses Docker networking entirely. Port mappings, service discovery, and network isolation stop working.

```yaml
# WRONG -- host networking breaks port mapping and isolation
services:
  app:
    network_mode: host
    ports:
      - "8080:80"    # Ignored with host networking
    networks:
      - backend      # Incompatible with network_mode
```

```yaml
# CORRECT -- use Docker networks for isolation
services:
  app:
    ports:
      - "127.0.0.1:8080:80"
    networks:
      - backend
```

**Why**: `network_mode: host` shares the host's network stack. The `ports` and `networks` attributes are silently ignored. Service-to-service DNS resolution stops working.

---

## AP-012: Not Using Profiles for Dev-Only Services

**Problem**: Debug and monitoring tools run in every environment, consuming resources unnecessarily.

```yaml
# WRONG -- debug tools always start
services:
  app:
    image: myapp
  phpmyadmin:
    image: phpmyadmin
  mailhog:
    image: mailhog/mailhog
```

```yaml
# CORRECT -- debug tools behind profile
services:
  app:
    image: myapp
  phpmyadmin:
    image: phpmyadmin
    profiles: [debug]
  mailhog:
    image: mailhog/mailhog
    profiles: [debug]
```

**Why**: Services without profiles ALWAYS start. In production, dev tools waste resources and create security risks. Use `docker compose --profile debug up` only when needed.

---

## AP-013: Bind Mounts With Absolute Host Paths

**Problem**: Absolute host paths make compose files non-portable across machines and operating systems.

```yaml
# WRONG -- path exists only on this specific machine
volumes:
  - /home/alice/project/data:/app/data
```

```yaml
# CORRECT -- relative path, portable
volumes:
  - ./data:/app/data
```

**Why**: Relative paths resolve from the Compose file location, making the project work on any machine. Absolute paths break when cloned to a different location or OS.

---

## AP-014: Logging Without Size Limits

**Problem**: Without log rotation, container logs grow indefinitely until the disk is full.

```yaml
# WRONG -- unlimited log growth
services:
  app:
    image: myapp
```

```yaml
# CORRECT -- bounded log size with rotation
services:
  app:
    image: myapp
    logging:
      driver: json-file
      options:
        max-size: "10m"
        max-file: "3"
```

**Why**: A busy application can generate gigabytes of logs. Without `max-size` and `max-file`, logs fill the disk, causing all services on the host to fail.

---

## AP-015: Not Using init for Worker Processes

**Problem**: Without `init: true`, the application runs as PID 1. PID 1 does not receive default signal handling, causing zombie processes and delayed shutdowns.

```yaml
# WRONG -- app is PID 1, poor signal handling
services:
  worker:
    command: ["python", "worker.py"]
```

```yaml
# CORRECT -- init process handles signals and reaps zombies
services:
  worker:
    command: ["python", "worker.py"]
    init: true
```

**Why**: The init process (tini) forwards signals properly and reaps zombie child processes. Without it, `docker compose stop` may wait the full grace period before sending SIGKILL.

---

## Official Sources

- https://docs.docker.com/compose/compose-file/05-services/
- https://docs.docker.com/compose/compose-file/deploy/
- https://docs.docker.com/compose/how-tos/environment-variables/
