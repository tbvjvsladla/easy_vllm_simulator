# Docker Compose Error Scenarios — Examples with Fixes

## Scenario 1: Database Not Ready When App Starts

### Symptom
Application crashes on startup with "connection refused" to database, even though `depends_on` is set.

### Broken Configuration
```yaml
services:
  app:
    image: myapp:latest
    depends_on:
      - db
    environment:
      DATABASE_URL: postgres://user:pass@db:5432/mydb

  db:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: pass
      POSTGRES_USER: user
      POSTGRES_DB: mydb
```

### Why It Fails
`depends_on` with default behavior (`service_started`) only waits for the container to start. PostgreSQL needs 5-15 seconds to initialize before accepting connections.

### Fixed Configuration
```yaml
services:
  app:
    image: myapp:latest
    depends_on:
      db:
        condition: service_healthy
    environment:
      DATABASE_URL: postgres://user:pass@db:5432/mydb

  db:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: pass
      POSTGRES_USER: user
      POSTGRES_DB: mydb
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U user -d mydb"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 30s
```

---

## Scenario 2: Port Conflict Between Services

### Symptom
```
Error response from daemon: driver failed programming external connectivity
on endpoint project-web-1: Bind for 0.0.0.0:8080 failed: port is already allocated
```

### Broken Configuration
```yaml
services:
  web:
    image: nginx
    ports:
      - "8080:80"

  api:
    image: myapi
    ports:
      - "8080:3000"    # Duplicate host port!
```

### Fix
```yaml
services:
  web:
    image: nginx
    ports:
      - "8080:80"

  api:
    image: myapi
    ports:
      - "8081:3000"    # Different host port
```

### External Port Conflict Fix
If the port is used by a process outside Docker:
```bash
# Find what's using the port
lsof -i :8080
# or
ss -tlnp | grep 8080

# Stop the conflicting process, then retry
docker compose up -d
```

---

## Scenario 3: Environment Variable Not Set

### Symptom
```
WARNING: The DATABASE_PASSWORD variable is not set. Defaulting to a blank string.
```
Or with strict syntax:
```
variable "DATABASE_PASSWORD" is not set and is required
```

### Broken Configuration
```yaml
services:
  app:
    image: myapp
    environment:
      DATABASE_PASSWORD: ${DATABASE_PASSWORD}
```

### Fix Option A: Create .env file
```bash
# Create .env in same directory as compose.yaml
echo "DATABASE_PASSWORD=my-secure-password" > .env
```

### Fix Option B: Use default value
```yaml
environment:
  DATABASE_PASSWORD: ${DATABASE_PASSWORD:-default-dev-password}
```

### Fix Option C: Require with error message
```yaml
environment:
  DATABASE_PASSWORD: ${DATABASE_PASSWORD:?DATABASE_PASSWORD must be set in .env or environment}
```

### Verify Resolution
```bash
docker compose config --environment
```

---

## Scenario 4: Orphan Container Warning

### Symptom
```
WARN[0000] Found orphan containers ([project-old-service-1]) for this project.
Use 'docker compose down --remove-orphans' to clean them up.
```

### Cause
A service was removed from `compose.yaml` but its container still exists from a previous run.

### Fix
```bash
# Remove orphan containers
docker compose down --remove-orphans

# Or suppress the warning permanently
export COMPOSE_IGNORE_ORPHANS=true
```

---

## Scenario 5: Volume Permission Denied

### Symptom
Application log shows:
```
Error: EACCES: permission denied, open '/data/app.db'
```

### Broken Configuration
```yaml
services:
  app:
    image: myapp    # Runs as UID 1000
    volumes:
      - ./data:/data    # Host dir owned by root
```

### Fix Option A: Match UID in Dockerfile
```dockerfile
FROM node:20-alpine
RUN mkdir -p /data && chown -R node:node /data
USER node
```

### Fix Option B: Set user in Compose
```yaml
services:
  app:
    image: myapp
    user: "${UID:-1000}:${GID:-1000}"
    volumes:
      - ./data:/data
```

### Fix Option C: Fix host directory ownership
```bash
sudo chown -R 1000:1000 ./data
```

---

## Scenario 6: Build Context Not Found

### Symptom
```
failed to solve: failed to read dockerfile: open Dockerfile: no such file or directory
```

### Broken Configuration
```yaml
services:
  app:
    build:
      context: ./backend
      dockerfile: docker/Dockerfile    # Relative to context!
```

### Project Structure
```
project/
  compose.yaml
  backend/
    docker/
      Dockerfile
    src/
```

### Why It Fails
`dockerfile` path is relative to `context`, so Compose looks for `./backend/docker/Dockerfile`. If the file is actually at `./docker/Dockerfile` (relative to project root), it will not be found.

### Fixed Configuration
```yaml
services:
  app:
    build:
      context: ./backend
      dockerfile: docker/Dockerfile    # Must exist at ./backend/docker/Dockerfile
```

Or if Dockerfile is at project root level:
```yaml
services:
  app:
    build:
      context: .
      dockerfile: docker/Dockerfile    # Now relative to project root
```

---

## Scenario 7: YAML Sexagesimal Port Parsing

### Symptom
Port mapping produces unexpected numbers. Port `56:56` gets interpreted as `3396`.

### Broken Configuration
```yaml
ports:
  - 56:56      # YAML parses as sexagesimal (base-60): 5*60+6 = 306
```

### Fix
```yaml
ports:
  - "56:56"    # ALWAYS quote port mappings
```

**Rule**: ALWAYS quote port mappings in Compose files to prevent YAML base-60 interpretation.

---

## Scenario 8: Dollar Sign in Environment Value

### Symptom
```
invalid interpolation format for services.app.environment.PASSWORD
```

### Broken Configuration
```yaml
services:
  app:
    environment:
      PASSWORD: pa$$word    # Compose tries to interpolate $$
```

### Fix
```yaml
services:
  app:
    environment:
      PASSWORD: "pa$$$$word"    # $$ produces literal $
```

Or use `env_file` with `format: raw` (Compose 2.30.0+):
```yaml
services:
  app:
    env_file:
      - path: ./secrets.env
        format: raw    # No interpolation
```

---

## Scenario 9: Profile Dependency Not Starting

### Symptom
```bash
docker compose --profile test up
# test-runner starts but debug-tools does not
```

### Broken Configuration
```yaml
services:
  debug-tools:
    image: debug:latest
    profiles: [debug]

  test-runner:
    image: test:latest
    profiles: [test]
    depends_on:
      - debug-tools    # debug profile not active!
```

### Fix Option A: Activate both profiles
```bash
docker compose --profile test --profile debug up
```

### Fix Option B: Remove profile from dependency
```yaml
services:
  debug-tools:
    image: debug:latest
    # No profile — always available

  test-runner:
    image: test:latest
    profiles: [test]
    depends_on:
      - debug-tools
```

### Fix Option C: Share the same profile
```yaml
services:
  debug-tools:
    image: debug:latest
    profiles: [test, debug]    # Active in both profiles

  test-runner:
    image: test:latest
    profiles: [test]
    depends_on:
      - debug-tools
```

---

## Scenario 10: Named Volume Not Declared

### Symptom
```
service "db" refers to undefined volume db-data: invalid compose project
```

### Broken Configuration
```yaml
services:
  db:
    image: postgres:16
    volumes:
      - db-data:/var/lib/postgresql/data
# Missing top-level volumes declaration!
```

### Fix
```yaml
services:
  db:
    image: postgres:16
    volumes:
      - db-data:/var/lib/postgresql/data

volumes:
  db-data:    # ALWAYS declare named volumes at top level
```

---

## Scenario 11: External Network Does Not Exist

### Symptom
```
network myshared declared as external, but could not be found
```

### Configuration
```yaml
services:
  app:
    networks:
      - myshared

networks:
  myshared:
    external: true
```

### Fix
```bash
# Create the external network first
docker network create myshared

# Then start Compose
docker compose up -d
```

---

## Scenario 12: Compose Override File Confusion

### Symptom
Services have unexpected configuration. Port mappings or environment variables do not match what is in `compose.yaml`.

### Cause
Compose automatically loads `compose.override.yaml` if it exists alongside `compose.yaml`. This is silent and can cause unexpected merging.

### Diagnosis
```bash
# See the fully merged and resolved config
docker compose config

# Check which files are being loaded
docker compose config --resolve-image-digests 2>&1 | head -5
```

### Fix
Remove or rename `compose.override.yaml` if it is not intentional. Or explicitly control which files are loaded:
```bash
docker compose -f compose.yaml up -d    # Only base file, no override
```
