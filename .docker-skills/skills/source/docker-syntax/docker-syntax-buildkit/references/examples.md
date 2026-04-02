# BuildKit Examples -- Cache, Secret, and SSH Patterns

## Cache Mount Patterns per Package Manager

### apt (Debian/Ubuntu)

```dockerfile
# syntax=docker/dockerfile:1

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
    curl \
    git \
    build-essential
```

**ALWAYS use `sharing=locked`** -- apt uses internal lock files and FAILS with concurrent access.

**Note:** When using cache mounts for apt, do NOT add `rm -rf /var/lib/apt/lists/*` -- the cache mount handles persistence, and the lists directory is the cache.

---

### npm

```dockerfile
# syntax=docker/dockerfile:1

WORKDIR /app
COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci --prefer-offline
```

For non-root builds:

```dockerfile
USER node
WORKDIR /app
COPY --chown=node:node package.json package-lock.json ./
RUN --mount=type=cache,target=/home/node/.npm,uid=1000,gid=1000 \
    npm ci --prefer-offline
```

---

### yarn (v1 Classic)

```dockerfile
# syntax=docker/dockerfile:1

WORKDIR /app
COPY package.json yarn.lock ./
RUN --mount=type=cache,target=/usr/local/share/.cache/yarn \
    yarn install --frozen-lockfile
```

---

### pnpm

```dockerfile
# syntax=docker/dockerfile:1

WORKDIR /app
COPY package.json pnpm-lock.yaml ./
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    corepack enable pnpm && pnpm install --frozen-lockfile
```

---

### pip (Python)

```dockerfile
# syntax=docker/dockerfile:1

WORKDIR /app
COPY requirements.txt ./
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install --no-compile -r requirements.txt
```

With bind mount (avoids COPY layer for requirements):

```dockerfile
RUN --mount=type=cache,target=/root/.cache/pip \
    --mount=type=bind,source=requirements.txt,target=/tmp/requirements.txt \
    pip install --no-compile -r /tmp/requirements.txt
```

---

### Go

```dockerfile
# syntax=docker/dockerfile:1

FROM golang:1.22 AS build
WORKDIR /src

# Cache module download
COPY go.mod go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod \
    go mod download

# Cache build artifacts
COPY . .
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    go build -o /bin/app ./cmd
```

Cross-compilation with cache:

```dockerfile
FROM --platform=$BUILDPLATFORM golang:1.22 AS build
ARG TARGETOS TARGETARCH
WORKDIR /src

COPY go.mod go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod go mod download

COPY . .
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    GOOS=$TARGETOS GOARCH=$TARGETARCH go build -o /bin/app ./cmd
```

---

### Cargo (Rust)

```dockerfile
# syntax=docker/dockerfile:1

FROM rust:1.77 AS build
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src/ src/

RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release \
    && cp target/release/myapp /usr/local/bin/myapp
```

**IMPORTANT:** ALWAYS copy the binary OUT of the cache-mounted `target/` directory before the RUN ends. The cache mount is not part of the layer -- if you do not copy, the binary is lost.

---

### Maven (Java)

```dockerfile
# syntax=docker/dockerfile:1

FROM maven:3.9-eclipse-temurin-21 AS build
WORKDIR /app

COPY pom.xml ./
RUN --mount=type=cache,target=/root/.m2/repository \
    mvn dependency:go-offline -B

COPY src/ src/
RUN --mount=type=cache,target=/root/.m2/repository \
    mvn package -B -DskipTests \
    && cp target/*.jar /app.jar
```

---

### Gradle (Java/Kotlin)

```dockerfile
# syntax=docker/dockerfile:1

FROM gradle:8.6-jdk21 AS build
WORKDIR /app

COPY build.gradle.kts settings.gradle.kts ./
COPY gradle/ gradle/
RUN --mount=type=cache,target=/home/gradle/.gradle/caches \
    gradle dependencies --no-daemon

COPY src/ src/
RUN --mount=type=cache,target=/home/gradle/.gradle/caches \
    gradle build --no-daemon -x test \
    && cp build/libs/*.jar /app.jar
```

---

### Bundler (Ruby)

```dockerfile
# syntax=docker/dockerfile:1

WORKDIR /app
COPY Gemfile Gemfile.lock ./
RUN --mount=type=cache,target=/root/.gem \
    bundle install --jobs 4 --retry 3
```

---

### NuGet (.NET)

```dockerfile
# syntax=docker/dockerfile:1

FROM mcr.microsoft.com/dotnet/sdk:8.0 AS build
WORKDIR /app

COPY *.csproj ./
RUN --mount=type=cache,target=/root/.nuget/packages \
    dotnet restore

COPY . .
RUN --mount=type=cache,target=/root/.nuget/packages \
    dotnet publish -c Release -o /out
```

---

### Composer (PHP)

```dockerfile
# syntax=docker/dockerfile:1

WORKDIR /app
COPY composer.json composer.lock ./
RUN --mount=type=cache,target=/tmp/cache \
    composer install --no-dev --no-scripts --prefer-dist
```

---

## Secret Mount Patterns

### Private Registry Authentication

```dockerfile
# syntax=docker/dockerfile:1

RUN --mount=type=secret,id=npmrc,target=/root/.npmrc \
    npm ci --prefer-offline
```

Build:

```bash
docker build --secret id=npmrc,src=$HOME/.npmrc .
```

---

### API Key During Build

```dockerfile
# syntax=docker/dockerfile:1

# As environment variable
RUN --mount=type=secret,id=api_key,env=API_KEY \
    curl -H "Authorization: Bearer $API_KEY" https://api.example.com/data > /app/data.json

# As file
RUN --mount=type=secret,id=api_key,target=/run/secrets/api_key \
    curl -H "Authorization: Bearer $(cat /run/secrets/api_key)" https://api.example.com/data > /app/data.json
```

Build:

```bash
# From file
docker build --secret id=api_key,src=./api-key.txt .

# From environment variable
docker build --secret id=api_key,env=API_KEY .
```

---

### Multiple Secrets

```dockerfile
# syntax=docker/dockerfile:1

RUN --mount=type=secret,id=aws_access,env=AWS_ACCESS_KEY_ID \
    --mount=type=secret,id=aws_secret,env=AWS_SECRET_ACCESS_KEY \
    aws s3 cp s3://private-bucket/model.bin /app/model.bin
```

Build:

```bash
docker build \
  --secret id=aws_access,env=AWS_ACCESS_KEY_ID \
  --secret id=aws_secret,env=AWS_SECRET_ACCESS_KEY .
```

---

### Required Secrets

```dockerfile
# syntax=docker/dockerfile:1

# Build FAILS if secret not provided (instead of silently continuing)
RUN --mount=type=secret,id=deploy_key,required=true,target=/root/.ssh/deploy_key \
    chmod 600 /root/.ssh/deploy_key \
    && git clone git@github.com:org/config.git /app/config
```

---

### Docker Compose Build Secrets

```yaml
# docker-compose.yml
services:
  app:
    build:
      context: .
      secrets:
        - npmrc
secrets:
  npmrc:
    file: ~/.npmrc
```

---

## SSH Mount Patterns

### Clone Private Repository

```dockerfile
# syntax=docker/dockerfile:1

RUN --mount=type=ssh \
    mkdir -p ~/.ssh \
    && ssh-keyscan github.com >> ~/.ssh/known_hosts \
    && git clone git@github.com:org/private-repo.git /app
```

Build:

```bash
# Forward running SSH agent
eval $(ssh-agent)
ssh-add ~/.ssh/id_ed25519
docker build --ssh default .
```

---

### Multiple SSH Identities

```dockerfile
# syntax=docker/dockerfile:1

# Clone from GitHub with default identity
RUN --mount=type=ssh,id=github \
    mkdir -p ~/.ssh \
    && ssh-keyscan github.com >> ~/.ssh/known_hosts \
    && git clone git@github.com:org/repo1.git /app/repo1

# Clone from GitLab with deploy key
RUN --mount=type=ssh,id=gitlab \
    ssh-keyscan gitlab.com >> ~/.ssh/known_hosts \
    && git clone git@gitlab.com:org/repo2.git /app/repo2
```

Build:

```bash
docker build \
  --ssh github=$HOME/.ssh/github_key \
  --ssh gitlab=$HOME/.ssh/gitlab_deploy_key .
```

---

### Go Private Modules via SSH

```dockerfile
# syntax=docker/dockerfile:1

FROM golang:1.22 AS build
WORKDIR /src

RUN --mount=type=ssh \
    mkdir -p ~/.ssh \
    && ssh-keyscan github.com >> ~/.ssh/known_hosts \
    && git config --global url."git@github.com:".insteadOf "https://github.com/"

COPY go.mod go.sum ./
RUN --mount=type=ssh \
    --mount=type=cache,target=/go/pkg/mod \
    go mod download

COPY . .
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    go build -o /bin/app ./cmd
```

---

## Bind Mount Patterns

### Build Without COPY Layer (Go)

```dockerfile
# syntax=docker/dockerfile:1

FROM golang:1.22 AS build
WORKDIR /src
RUN --mount=type=bind,target=. \
    --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    go build -o /bin/app ./cmd
```

The source code is mounted read-only. Only `/bin/app` becomes part of the layer.

---

### Cross-Stage File Access

```dockerfile
# syntax=docker/dockerfile:1

FROM alpine AS configs
COPY configs/ /configs/

FROM alpine AS app
RUN --mount=type=bind,from=configs,source=/configs,target=/tmp/configs \
    cp /tmp/configs/production.yaml /etc/app/config.yaml
```

---

### Bind + Cache Combined (Python)

```dockerfile
# syntax=docker/dockerfile:1

FROM python:3.12-slim AS build
RUN --mount=type=cache,target=/root/.cache/pip \
    --mount=type=bind,source=requirements.txt,target=/tmp/requirements.txt \
    pip install --prefix=/install -r /tmp/requirements.txt

FROM python:3.12-slim
COPY --from=build /install /usr/local
```

No `requirements.txt` COPY layer exists in the final image. The pip cache persists across builds.

---

## Cache Backend Patterns

### Inline Cache

```bash
docker build -t myapp:latest \
  --build-arg BUILDKIT_INLINE_CACHE=1 \
  --push .

# Reuse on next build
docker build -t myapp:latest \
  --cache-from myapp:latest .
```

Simple but limited -- only caches final stage layers.

---

### Registry Cache (CI/CD)

```bash
# Build with registry cache
docker buildx build --push -t registry/app:latest \
  --cache-to type=registry,ref=registry/app:buildcache,mode=max \
  --cache-from type=registry,ref=registry/app:buildcache .

# Feature branch with fallback to main
docker buildx build --push -t registry/app:feature \
  --cache-to type=registry,ref=registry/app:cache-feature \
  --cache-from type=registry,ref=registry/app:cache-feature \
  --cache-from type=registry,ref=registry/app:cache-main .
```

---

### Local Cache

```bash
docker buildx build \
  --cache-to type=local,dest=/tmp/buildcache \
  --cache-from type=local,src=/tmp/buildcache .
```

---

### GitHub Actions Cache

```yaml
# .github/workflows/build.yml
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: docker/setup-buildx-action@v4
      - uses: docker/build-push-action@v7
        with:
          push: true
          tags: ghcr.io/org/app:latest
          cache-from: type=gha
          cache-to: type=gha,mode=max
```
