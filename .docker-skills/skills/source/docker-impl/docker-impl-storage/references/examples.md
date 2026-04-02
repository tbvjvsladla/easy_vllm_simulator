# Storage Examples

Working examples for Docker storage patterns. All examples verified against Docker Engine 24+ and Docker Compose v2.

---

## Database Persistence Patterns

### PostgreSQL with Named Volume

```bash
# Create and run with named volume
docker run -d --name postgres \
  --mount source=pgdata,target=/var/lib/postgresql/data \
  -e POSTGRES_PASSWORD=secret \
  -e POSTGRES_DB=myapp \
  -p 5432:5432 \
  postgres:16

# Verify volume created
docker volume inspect pgdata

# Stop and remove container -- data persists
docker stop postgres && docker rm postgres

# Start new container with same volume -- data intact
docker run -d --name postgres \
  --mount source=pgdata,target=/var/lib/postgresql/data \
  -e POSTGRES_PASSWORD=secret \
  -p 5432:5432 \
  postgres:16
```

### MySQL with Named Volume

```bash
docker run -d --name mysql \
  --mount source=mysqldata,target=/var/lib/mysql \
  -e MYSQL_ROOT_PASSWORD=secret \
  -e MYSQL_DATABASE=myapp \
  -p 3306:3306 \
  mysql:8
```

### MongoDB with Named Volume

```bash
docker run -d --name mongo \
  --mount source=mongodata,target=/data/db \
  -e MONGO_INITDB_ROOT_USERNAME=admin \
  -e MONGO_INITDB_ROOT_PASSWORD=secret \
  -p 27017:27017 \
  mongo:7
```

### Redis with Named Volume

```bash
docker run -d --name redis \
  --mount source=redisdata,target=/data \
  redis:7 redis-server --appendonly yes
```

### Database Data Directories Reference

| Database | Data Directory | Volume Target |
|----------|---------------|---------------|
| PostgreSQL | `/var/lib/postgresql/data` | `/var/lib/postgresql/data` |
| MySQL | `/var/lib/mysql` | `/var/lib/mysql` |
| MongoDB | `/data/db` | `/data/db` |
| Redis | `/data` | `/data` |
| Elasticsearch | `/usr/share/elasticsearch/data` | `/usr/share/elasticsearch/data` |
| MariaDB | `/var/lib/mysql` | `/var/lib/mysql` |

---

## Backup and Restore Procedures

### Backup a Named Volume

```bash
# Method 1: tar backup via helper container
docker run --rm \
  --mount source=pgdata,target=/data,readonly \
  -v $(pwd):/backup \
  alpine tar czf /backup/pgdata-backup.tar.gz -C /data .

# Method 2: Backup from a running database container
docker run --rm \
  --volumes-from postgres:ro \
  -v $(pwd):/backup \
  alpine tar czf /backup/pgdata-backup.tar.gz -C /var/lib/postgresql/data .
```

### Restore a Named Volume

```bash
# Create fresh volume
docker volume create pgdata-restored

# Restore from backup
docker run --rm \
  --mount source=pgdata-restored,target=/data \
  -v $(pwd):/backup \
  alpine sh -c "cd /data && tar xzf /backup/pgdata-backup.tar.gz"

# Use restored volume
docker run -d --name postgres \
  --mount source=pgdata-restored,target=/var/lib/postgresql/data \
  -e POSTGRES_PASSWORD=secret \
  postgres:16
```

### PostgreSQL-Native Backup (pg_dump)

```bash
# Logical backup (SQL dump)
docker exec postgres pg_dump -U postgres myapp > myapp-backup.sql

# Restore from SQL dump
docker exec -i postgres psql -U postgres myapp < myapp-backup.sql

# Compressed backup
docker exec postgres pg_dump -U postgres -Fc myapp > myapp-backup.dump

# Restore compressed
docker exec -i postgres pg_restore -U postgres -d myapp < myapp-backup.dump
```

### MySQL-Native Backup (mysqldump)

```bash
# Backup
docker exec mysql mysqldump -u root -psecret myapp > myapp-backup.sql

# Restore
docker exec -i mysql mysql -u root -psecret myapp < myapp-backup.sql
```

### Automated Backup Script

```bash
#!/bin/bash
# backup-volumes.sh — Backup all named volumes
BACKUP_DIR="/backups/$(date +%Y%m%d)"
mkdir -p "$BACKUP_DIR"

for volume in $(docker volume ls -q --filter "label=backup=true"); do
  echo "Backing up volume: $volume"
  docker run --rm \
    --mount source="$volume",target=/data,readonly \
    -v "$BACKUP_DIR":/backup \
    alpine tar czf "/backup/${volume}.tar.gz" -C /data .
done

# Remove backups older than 30 days
find /backups -type f -name "*.tar.gz" -mtime +30 -delete
```

---

## NFS Volume Examples

### NFSv3 Volume

```bash
docker volume create --driver local \
  --opt type=nfs \
  --opt device=:/var/docker-nfs \
  --opt o=addr=10.0.0.10 \
  nfs-data
```

### NFSv4 Volume

```bash
docker volume create --driver local \
  --opt type=nfs \
  --opt device=:/var/docker-nfs \
  --opt "o=addr=10.0.0.10,rw,nfsvers=4,async" \
  nfs-data
```

### NFS Volume in Compose

```yaml
services:
  app:
    image: myapp:latest
    volumes:
      - nfs-data:/app/data

volumes:
  nfs-data:
    driver: local
    driver_opts:
      type: nfs
      device: ":/var/docker-nfs"
      o: "addr=10.0.0.10,rw,nfsvers=4,async"
```

### CIFS/SMB Volume

```bash
docker volume create --driver local \
  --opt type=cifs \
  --opt device=//fileserver.example.com/shared \
  --opt o=addr=fileserver.example.com,username=dockeruser,password=secret,file_mode=0777,dir_mode=0777 \
  --name cifs-data
```

### CIFS Volume in Compose

```yaml
volumes:
  cifs-data:
    driver: local
    driver_opts:
      type: cifs
      device: "//fileserver.example.com/shared"
      o: "addr=fileserver.example.com,username=dockeruser,password=secret"
```

---

## Docker Compose Volume Patterns

### Basic Named Volume

```yaml
services:
  db:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: secret
    volumes:
      - pgdata:/var/lib/postgresql/data

volumes:
  pgdata:
```

### Shared Volume Between Services

```yaml
services:
  writer:
    image: mywriter:latest
    volumes:
      - shared-data:/data

  reader:
    image: myreader:latest
    volumes:
      - shared-data:/data:ro    # Read-only access

volumes:
  shared-data:
```

### External Volume (Pre-Created)

```yaml
# Volume MUST exist before running docker compose up
# Create it first: docker volume create pgdata-prod

services:
  db:
    image: postgres:16
    volumes:
      - pgdata-prod:/var/lib/postgresql/data

volumes:
  pgdata-prod:
    external: true
```

### Bind Mount for Development

```yaml
services:
  app:
    image: node:20-alpine
    working_dir: /app
    volumes:
      - ./src:/app/src            # Bind mount for live reload
      - node_modules:/app/node_modules  # Named volume for deps
    command: npm run dev

volumes:
  node_modules:
```

### Full-Stack Application with Volumes

```yaml
services:
  web:
    image: nginx:alpine
    ports:
      - "80:80"
    volumes:
      - static-files:/usr/share/nginx/html:ro
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
    depends_on:
      - app

  app:
    build: .
    volumes:
      - static-files:/app/static
      - uploads:/app/uploads
    depends_on:
      - db

  db:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: secret
    volumes:
      - pgdata:/var/lib/postgresql/data

volumes:
  static-files:
  uploads:
  pgdata:
```

### Compose Volume with Labels

```yaml
volumes:
  pgdata:
    labels:
      backup: "true"
      project: "myapp"
      environment: "production"
```

### tmpfs in Compose

```yaml
services:
  app:
    image: myapp:latest
    read_only: true
    tmpfs:
      - /tmp
      - /run
    volumes:
      - appdata:/data
```

---

## Development Workflow Examples

### Node.js Development with Volume Optimization

```yaml
services:
  app:
    image: node:20-alpine
    working_dir: /app
    volumes:
      - .:/app                          # Source code bind mount
      - node_modules:/app/node_modules  # Preserve node_modules in volume
    command: npm run dev
    ports:
      - "3000:3000"

volumes:
  node_modules:
```

Using a named volume for `node_modules` prevents host OS native module conflicts and improves performance.

### Python Development with pip Cache

```yaml
services:
  app:
    build: .
    volumes:
      - .:/app
      - pip-cache:/root/.cache/pip
    command: python manage.py runserver 0.0.0.0:8000

volumes:
  pip-cache:
```

---

## Read-Only Container with Selective Write Access

```bash
# Production hardened container
docker run -d --name secure-app \
  --read-only \
  --tmpfs /tmp:size=64m \
  --tmpfs /run \
  --mount source=appdata,dst=/app/data \
  --mount type=bind,src=/etc/myapp/config.yml,dst=/app/config.yml,readonly \
  myapp:latest
```

Compose equivalent:

```yaml
services:
  app:
    image: myapp:latest
    read_only: true
    tmpfs:
      - /tmp:size=64m
      - /run
    volumes:
      - appdata:/app/data
      - ./config.yml:/app/config.yml:ro

volumes:
  appdata:
```

---

## Volume Inspection and Debugging

```bash
# List all volumes with details
docker volume ls --format "table {{.Name}}\t{{.Driver}}\t{{.Labels}}"

# Check volume mount point on host
docker volume inspect --format '{{.Mountpoint}}' mydata

# See what volumes a container uses
docker inspect --format='{{range .Mounts}}{{.Name}} -> {{.Destination}} ({{.Type}}){{println}}{{end}}' myapp

# Check volume size (via container)
docker run --rm --mount source=mydata,dst=/data,readonly alpine du -sh /data

# Find containers using a volume
docker ps -a --filter volume=mydata --format "{{.Names}}"
```
