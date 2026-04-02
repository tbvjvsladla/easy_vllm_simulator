# Docker Compose Anti-Patterns

## AP-01: Using depends_on Without Healthchecks

**What happens**: App starts before the database is ready, causing connection errors and crash loops.

```yaml
# WRONG
services:
  app:
    depends_on:
      - db
  db:
    image: postgres:16
```

```yaml
# CORRECT
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

**Rule**: ALWAYS use `condition: service_healthy` with a healthcheck for services that need initialization time (databases, message brokers, caches).

---

## AP-02: Using the Deprecated version Field

**What happens**: Compose v2 ignores it and emits a warning. Developers waste time picking version numbers.

```yaml
# WRONG
version: "3.8"
services:
  web:
    image: nginx
```

```yaml
# CORRECT
services:
  web:
    image: nginx
```

**Rule**: NEVER include `version:` in Compose files. The Compose Specification is the only format since Compose v2.

---

## AP-03: Anonymous Volumes for Persistent Data

**What happens**: Data is lost when running `docker compose down` because anonymous volumes are not preserved.

```yaml
# WRONG — anonymous volume
services:
  db:
    image: postgres:16
    volumes:
      - /var/lib/postgresql/data
```

```yaml
# CORRECT — named volume
services:
  db:
    image: postgres:16
    volumes:
      - pgdata:/var/lib/postgresql/data

volumes:
  pgdata:
```

**Rule**: ALWAYS use named volumes for any data that must survive container recreation.

---

## AP-04: Hardcoding Secrets in Compose Files

**What happens**: Secrets committed to version control. Anyone with repo access sees credentials.

```yaml
# WRONG
services:
  db:
    environment:
      POSTGRES_PASSWORD: "super-secret-password"
```

```yaml
# CORRECT — interpolation from .env (add .env to .gitignore)
services:
  db:
    environment:
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:?Set POSTGRES_PASSWORD in .env}
```

```yaml
# BEST — use Docker secrets
services:
  db:
    environment:
      POSTGRES_PASSWORD_FILE: /run/secrets/db_password
    secrets:
      - db_password

secrets:
  db_password:
    file: ./secrets/db_password.txt
```

**Rule**: NEVER hardcode passwords or API keys in Compose files. Use `.env` files (gitignored) or Docker secrets.

---

## AP-05: Exposing Ports to All Interfaces

**What happens**: Services are accessible from any network interface, including public networks.

```yaml
# WRONG — accessible from any interface
ports:
  - "5432:5432"
```

```yaml
# CORRECT — bound to localhost only
ports:
  - "127.0.0.1:5432:5432"
```

**Rule**: ALWAYS bind development ports to `127.0.0.1` unless external access is explicitly required. Database ports (5432, 3306, 27017, 6379) should NEVER be exposed to all interfaces.

---

## AP-06: Using container_name with Scalable Services

**What happens**: `docker compose up --scale web=3` fails because container names must be unique.

```yaml
# WRONG — prevents scaling
services:
  web:
    image: nginx
    container_name: my-nginx
```

```yaml
# CORRECT — let Compose manage names
services:
  web:
    image: nginx
```

**Rule**: NEVER use `container_name` on services you might need to scale. Compose generates unique names automatically.

---

## AP-07: Unquoted Port Mappings

**What happens**: YAML interprets numbers containing colons as sexagesimal (base-60) values. `56:56` becomes `3396`.

```yaml
# WRONG — potential YAML parsing surprise
ports:
  - 56:56
  - 80:80
```

```yaml
# CORRECT — always quote
ports:
  - "56:56"
  - "80:80"
```

**Rule**: ALWAYS quote port mappings in Compose files to prevent YAML sexagesimal interpretation.

---

## AP-08: restart: always Without Resource Limits

**What happens**: A crashing service restarts infinitely, consuming CPU and memory without bound.

```yaml
# WRONG — crash loop with no limits
services:
  app:
    image: myapp
    restart: always
```

```yaml
# CORRECT — restart with safety limits
services:
  app:
    image: myapp
    restart: unless-stopped
    deploy:
      resources:
        limits:
          cpus: "0.50"
          memory: 512M
```

**Rule**: ALWAYS combine `restart:` policies with resource limits via `deploy.resources.limits` to prevent runaway containers.

---

## AP-09: Not Using Profiles for Optional Services

**What happens**: Debug tools, admin panels, and test utilities run in all environments, wasting resources and expanding the attack surface.

```yaml
# WRONG — debug tools always running
services:
  app:
    image: myapp
  phpmyadmin:
    image: phpmyadmin
  mailhog:
    image: mailhog/mailhog
```

```yaml
# CORRECT — optional services behind profiles
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

```bash
# Start with debug tools only when needed
docker compose --profile debug up -d
```

**Rule**: ALWAYS assign profiles to services that are not needed in production (debug tools, admin panels, test services).

---

## AP-10: Large Build Context Without .dockerignore

**What happens**: Docker sends the entire project directory (including node_modules, .git, data files) as build context. Builds are slow and may include sensitive files in the image.

```yaml
# WRONG — no .dockerignore, sending everything
services:
  app:
    build: .
```

**Fix**: Create `.dockerignore` in the build context:
```
node_modules
.git
.env
*.log
data/
dist/
coverage/
```

**Rule**: ALWAYS create a `.dockerignore` file when using `build:` in Compose. At minimum, exclude `.git/`, `node_modules/`, and any data directories.

---

## AP-11: Using Default Bridge Network

**What happens**: Services cannot resolve each other by name. DNS resolution only works on user-defined networks.

```yaml
# WRONG — relying on default bridge (implicit)
services:
  app:
    image: myapp
    network_mode: bridge
  db:
    image: postgres
    network_mode: bridge
```

```yaml
# CORRECT — Compose creates a default user-defined network automatically
# Simply do not specify network_mode at all
services:
  app:
    image: myapp
  db:
    image: postgres
```

**Rule**: NEVER set `network_mode: bridge` explicitly. Compose automatically creates a user-defined bridge network with DNS resolution enabled.

---

## AP-12: Missing Named Volume Declaration

**What happens**: `docker compose config` fails with "refers to undefined volume".

```yaml
# WRONG — volume used but not declared
services:
  db:
    volumes:
      - pgdata:/var/lib/postgresql/data
```

```yaml
# CORRECT — volume declared at top level
services:
  db:
    volumes:
      - pgdata:/var/lib/postgresql/data

volumes:
  pgdata:
```

**Rule**: ALWAYS declare named volumes in the top-level `volumes:` section. This is a Compose requirement, not optional.

---

## AP-13: Ignoring docker compose config Output

**What happens**: Developers debug runtime issues that are caused by misconfigured Compose files. Hours spent on symptoms rather than root cause.

```bash
# WRONG workflow — jump straight to up and debug
docker compose up -d
docker compose logs
# ... head scratching ...
```

```bash
# CORRECT workflow — validate first
docker compose config -q          # Quick validation
docker compose config             # Inspect resolved output
docker compose up -d              # Start with confidence
```

**Rule**: ALWAYS run `docker compose config` before `docker compose up` when troubleshooting. This catches YAML errors, undefined variables, missing volumes/networks, and merge issues before they become runtime failures.

---

## AP-14: Cross-Profile Dependencies

**What happens**: Service A (profile: test) depends on Service B (profile: debug). Starting only the `test` profile leaves Service B inactive, causing dependency failure.

```yaml
# WRONG — cross-profile dependency
services:
  debug-db:
    profiles: [debug]
  test-runner:
    profiles: [test]
    depends_on:
      - debug-db    # Not started unless debug profile is active
```

```yaml
# CORRECT — shared profile or no profile on dependency
services:
  debug-db:
    profiles: [debug, test]    # Available in both profiles
  test-runner:
    profiles: [test]
    depends_on:
      - debug-db
```

**Rule**: NEVER create `depends_on` chains across different profiles unless you ALWAYS activate all required profiles together. Dependencies must share at least one profile with their dependents, or have no profile assignment.
