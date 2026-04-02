# Dockerfile Templates per Language

Complete, buildable Dockerfile templates for each supported language. Every template follows the same structure: multi-stage build, cache mounts, non-root user, health check, exec-form entrypoint.

---

## Node.js — Server Application

```dockerfile
# syntax=docker/dockerfile:1

# ---- Build Stage ----
FROM node:22-bookworm-slim AS build
WORKDIR /src

COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci --omit=dev

COPY . .
RUN npm run build

# ---- Runtime Stage ----
FROM node:22-bookworm-slim AS runtime
WORKDIR /app

RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser

COPY --from=build --chown=appuser:appuser /src/dist ./dist
COPY --from=build --chown=appuser:appuser /src/node_modules ./node_modules
COPY --from=build --chown=appuser:appuser /src/package.json ./

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD node -e "require('http').get('http://localhost:3000/health', (r) => { process.exit(r.statusCode === 200 ? 0 : 1) })" || exit 1

USER appuser
EXPOSE 3000

ENTRYPOINT ["node"]
CMD ["dist/index.js"]
```

### Node.js — Static Frontend (React, Vue, Angular)

```dockerfile
# syntax=docker/dockerfile:1

# ---- Build Stage ----
FROM node:22-bookworm-slim AS build
WORKDIR /src

COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci

COPY . .
RUN npm run build

# ---- Runtime Stage ----
FROM nginx:1.27-alpine AS runtime

RUN addgroup -S appuser && adduser -S appuser -G appuser

COPY --from=build /src/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost/ || exit 1

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
```

---

## Python — pip

```dockerfile
# syntax=docker/dockerfile:1

# ---- Build Stage ----
FROM python:3.12-slim-bookworm AS build
WORKDIR /src

COPY requirements.txt .
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install --no-compile --prefix=/install -r requirements.txt

COPY . .

# ---- Runtime Stage ----
FROM python:3.12-slim-bookworm AS runtime
WORKDIR /app

RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser

COPY --from=build /install /usr/local
COPY --from=build --chown=appuser:appuser /src .

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD python -c "import urllib.request; urllib.request.urlopen('http://localhost:8000/health')" || exit 1

USER appuser
EXPOSE 8000

ENTRYPOINT ["python"]
CMD ["-m", "uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]
```

### Python — Poetry

```dockerfile
# syntax=docker/dockerfile:1

# ---- Build Stage ----
FROM python:3.12-slim-bookworm AS build
WORKDIR /src

ENV POETRY_NO_INTERACTION=1 \
    POETRY_VIRTUALENVS_IN_PROJECT=1 \
    POETRY_VIRTUALENVS_CREATE=1

RUN --mount=type=cache,target=/root/.cache/pip \
    pip install poetry

COPY pyproject.toml poetry.lock ./
RUN --mount=type=cache,target=/root/.cache/pypoetry \
    poetry install --without dev --no-root

COPY . .
RUN poetry install --without dev

# ---- Runtime Stage ----
FROM python:3.12-slim-bookworm AS runtime
WORKDIR /app

RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser

COPY --from=build /src/.venv ./.venv
COPY --from=build --chown=appuser:appuser /src .

ENV PATH="/app/.venv/bin:$PATH"

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD python -c "import urllib.request; urllib.request.urlopen('http://localhost:8000/health')" || exit 1

USER appuser
EXPOSE 8000

ENTRYPOINT ["python"]
CMD ["-m", "uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]
```

---

## Go

```dockerfile
# syntax=docker/dockerfile:1

# ---- Build Stage ----
FROM --platform=$BUILDPLATFORM golang:1.22-alpine AS build

ARG TARGETOS TARGETARCH

WORKDIR /src

COPY go.mod go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod \
    go mod download

COPY . .
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=0 GOOS=$TARGETOS GOARCH=$TARGETARCH \
    go build -ldflags="-s -w" -o /bin/app ./cmd/server

# ---- Runtime Stage ----
FROM scratch AS runtime

COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=build /bin/app /app

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD ["/app", "healthcheck"]

USER 65534:65534

EXPOSE 8080

ENTRYPOINT ["/app"]
```

**Notes:**
- `scratch` has no shell — health check MUST use the binary itself (implement a `healthcheck` subcommand) or use `alpine` as runtime instead.
- `CGO_ENABLED=0` produces a fully static binary.
- `USER 65534:65534` is the `nobody` user on Linux.
- `-ldflags="-s -w"` strips debug info, reducing binary size.

### Go with Alpine Runtime (when shell is needed)

```dockerfile
# syntax=docker/dockerfile:1

FROM --platform=$BUILDPLATFORM golang:1.22-alpine AS build

ARG TARGETOS TARGETARCH

WORKDIR /src
COPY go.mod go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod \
    go mod download

COPY . .
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=0 GOOS=$TARGETOS GOARCH=$TARGETARCH \
    go build -ldflags="-s -w" -o /bin/app ./cmd/server

FROM alpine:3.21 AS runtime

RUN addgroup -S appuser && adduser -S appuser -G appuser
RUN apk --no-cache add ca-certificates wget

COPY --from=build /bin/app /usr/bin/app

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

USER appuser
EXPOSE 8080

ENTRYPOINT ["/usr/bin/app"]
```

---

## Java — Maven

```dockerfile
# syntax=docker/dockerfile:1

# ---- Build Stage ----
FROM eclipse-temurin:21-jdk-jammy AS build
WORKDIR /src

COPY pom.xml .
COPY .mvn .mvn
COPY mvnw .
RUN chmod +x mvnw
RUN --mount=type=cache,target=/root/.m2 \
    ./mvnw dependency:go-offline

COPY src ./src
RUN --mount=type=cache,target=/root/.m2 \
    ./mvnw package -DskipTests

# ---- Runtime Stage ----
FROM eclipse-temurin:21-jre-jammy AS runtime
WORKDIR /app

RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser

COPY --from=build --chown=appuser:appuser /src/target/*.jar app.jar

HEALTHCHECK --interval=30s --timeout=3s --start-period=30s --retries=3 \
  CMD curl -f http://localhost:8080/actuator/health || exit 1

USER appuser
EXPOSE 8080

ENTRYPOINT ["java"]
CMD ["-jar", "app.jar"]
```

### Java — Gradle

```dockerfile
# syntax=docker/dockerfile:1

# ---- Build Stage ----
FROM eclipse-temurin:21-jdk-jammy AS build
WORKDIR /src

COPY build.gradle settings.gradle gradlew ./
COPY gradle ./gradle
RUN chmod +x gradlew
RUN --mount=type=cache,target=/root/.gradle \
    ./gradlew dependencies --no-daemon

COPY src ./src
RUN --mount=type=cache,target=/root/.gradle \
    ./gradlew bootJar --no-daemon -x test

# ---- Runtime Stage ----
FROM eclipse-temurin:21-jre-jammy AS runtime
WORKDIR /app

RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser

COPY --from=build --chown=appuser:appuser /src/build/libs/*.jar app.jar

HEALTHCHECK --interval=30s --timeout=3s --start-period=30s --retries=3 \
  CMD curl -f http://localhost:8080/actuator/health || exit 1

USER appuser
EXPOSE 8080

ENTRYPOINT ["java"]
CMD ["-jar", "app.jar"]
```

---

## Rust

```dockerfile
# syntax=docker/dockerfile:1

# ---- Build Stage ----
FROM rust:1.77-bookworm AS build
WORKDIR /src

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release

COPY src ./src
RUN touch src/main.rs && \
    --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release && \
    cp /app/target/release/myapp /bin/app

# ---- Runtime Stage ----
FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser

COPY --from=build /bin/app /usr/bin/app

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

USER appuser
EXPOSE 8080

ENTRYPOINT ["/usr/bin/app"]
```

**Notes:**
- The dummy `main.rs` trick caches dependency compilation separately from source changes.
- Use `debian:bookworm-slim` for Rust apps that link dynamically. For static builds (`RUSTFLAGS='-C target-feature=+crt-static'`), use `scratch`.
- Replace `myapp` with the actual binary name from `Cargo.toml`.

---

## .NET

```dockerfile
# syntax=docker/dockerfile:1

# ---- Build Stage ----
FROM mcr.microsoft.com/dotnet/sdk:8.0 AS build
WORKDIR /src

COPY *.csproj ./
RUN --mount=type=cache,target=/root/.nuget/packages \
    dotnet restore

COPY . .
RUN dotnet publish -c Release -o /app/publish --no-restore

# ---- Runtime Stage ----
FROM mcr.microsoft.com/dotnet/aspnet:8.0 AS runtime
WORKDIR /app

RUN groupadd -r appuser && useradd --no-log-init -r -g appuser appuser

COPY --from=build --chown=appuser:appuser /app/publish .

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD curl -f http://localhost:8080/health || exit 1

USER appuser
EXPOSE 8080

ENTRYPOINT ["dotnet", "MyApp.dll"]
```

**Notes:**
- Replace `MyApp.dll` with the actual assembly name.
- For self-contained deployments, add `--self-contained true -r linux-x64` to `dotnet publish` and use `mcr.microsoft.com/dotnet/runtime-deps:8.0` as runtime base.
- The .NET 8+ default port is 8080 (changed from 80 in earlier versions).

---

## Template Customization Rules

When adapting these templates:

1. **ALWAYS** keep the `# syntax=docker/dockerfile:1` directive
2. **ALWAYS** keep multi-stage structure (build + runtime)
3. **ALWAYS** keep cache mounts for the language's package manager
4. **ALWAYS** keep the non-root user pattern
5. **ALWAYS** keep the HEALTHCHECK instruction
6. **ALWAYS** update port numbers to match the application
7. **ALWAYS** update health check URLs to match the application's health endpoint
8. **NEVER** add secrets via ENV or ARG — use `--mount=type=secret` if needed
9. **NEVER** remove the WORKDIR instruction
10. **NEVER** use shell-form ENTRYPOINT
