# Language-Specific Multi-Stage Examples

> Production-ready multi-stage Dockerfiles for six major languages.
> All examples verified against Docker Engine 24+ with BuildKit.

---

## Go

Go is the ideal language for multi-stage builds because it produces statically linked binaries that run on `scratch`.

```dockerfile
# syntax=docker/dockerfile:1

FROM --platform=$BUILDPLATFORM golang:1.22-alpine AS build

ARG TARGETOS TARGETARCH
ARG VERSION=dev

WORKDIR /src

# Cache module download (only reruns when go.mod/go.sum change)
COPY go.mod go.sum ./
RUN go mod download -x

COPY . .

# Cross-compile static binary
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=0 GOOS=$TARGETOS GOARCH=$TARGETARCH \
    go build -ldflags="-s -w -X main.version=$VERSION" \
    -o /app/server ./cmd/server

# ---- Test (optional CI target) ----
FROM build AS test
RUN CGO_ENABLED=0 go test -v ./...

# ---- Production ----
FROM scratch AS production
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=build /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=build /app/server /server
USER 65534:65534
ENTRYPOINT ["/server"]
```

Key Go decisions:
- `CGO_ENABLED=0` -- ALWAYS set for scratch/distroless targets. Without it, Go links against glibc.
- `-ldflags="-s -w"` -- Strips debug symbols, reduces binary size by ~30%.
- `--platform=$BUILDPLATFORM` on build stage -- Compiles natively, cross-compiles for target. 10x faster than QEMU emulation.
- Copy CA certs and timezone data from the build stage -- `scratch` has nothing.

**Final image size:** ~10-20 MB (binary only, no OS).

---

## Node.js

Node.js requires a runtime, so the final image uses `node:*-slim` instead of `scratch`.

```dockerfile
# syntax=docker/dockerfile:1

# ---- Dependencies ----
FROM node:20-slim AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --ignore-scripts

# ---- Build ----
FROM deps AS build
COPY . .
RUN npm run build
# Remove devDependencies after build
RUN npm prune --production

# ---- Test (optional CI target) ----
FROM deps AS test
COPY . .
RUN npm run lint && npm run test

# ---- Production ----
FROM node:20-slim AS production
WORKDIR /app
ENV NODE_ENV=production

# Copy production node_modules (no devDependencies)
COPY --from=build /app/node_modules ./node_modules
# Copy built application
COPY --from=build /app/dist ./dist
COPY package.json ./

USER node
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
  CMD node -e "require('http').get('http://localhost:3000/health', (r) => { process.exit(r.statusCode === 200 ? 0 : 1) })"

CMD ["node", "dist/index.js"]
```

Key Node.js decisions:
- `npm ci` in deps stage -- Installs exact versions from lockfile. NEVER use `npm install` in Docker builds.
- `npm prune --production` -- Removes devDependencies after build, before copying to production.
- `--ignore-scripts` -- Prevents postinstall scripts from running during dependency install (security).
- `node:20-slim` for production -- ~180 MB vs ~1.1 GB for full `node:20`.
- Built-in `node` user -- Node images include a non-root `node` user. ALWAYS use it.

**Final image size:** ~180-250 MB.

---

## Python

Python multi-stage builds use virtual environments to isolate dependencies for clean copying.

```dockerfile
# syntax=docker/dockerfile:1

FROM python:3.12-slim AS base
RUN apt-get update && apt-get install -y --no-install-recommends \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

# ---- Dependencies ----
FROM base AS deps
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Use a virtual environment for clean copying
RUN python -m venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

COPY requirements.txt .
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install --no-compile -r requirements.txt

# ---- Test (optional CI target) ----
FROM deps AS test
COPY requirements-dev.txt .
RUN pip install --no-compile -r requirements-dev.txt
COPY . /app
WORKDIR /app
RUN pytest --tb=short -q

# ---- Production ----
FROM base AS production
WORKDIR /app

# Copy the entire virtual environment (includes all installed packages)
COPY --from=deps /opt/venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

COPY . .

RUN groupadd -r app && useradd -r -g app app
USER app

EXPOSE 8000
CMD ["gunicorn", "app:create_app()", "--bind", "0.0.0.0:8000", "--workers", "4"]
```

Key Python decisions:
- Virtual environment (`/opt/venv`) -- ALWAYS use a venv so dependencies can be copied as a single directory. Without it, packages scatter across system directories.
- `build-essential` and `libpq-dev` in deps stage only -- Needed to compile C extensions (psycopg2, numpy). NEVER include in production.
- `libpq5` in base and production -- Runtime library for PostgreSQL. Build headers (`libpq-dev`) stay in deps.
- `--no-compile` -- Skip `.pyc` generation during install (happens at runtime). Smaller image.
- Cache mount on pip -- Avoids re-downloading packages on rebuild.

**Final image size:** ~150-250 MB.

---

## Java (Spring Boot / Maven)

Java uses a JDK for building and a JRE for running.

```dockerfile
# syntax=docker/dockerfile:1

# ---- Build ----
FROM eclipse-temurin:21-jdk AS build
WORKDIR /src

# Cache Maven dependencies
COPY pom.xml .
COPY .mvn .mvn
COPY mvnw .
RUN chmod +x mvnw
RUN --mount=type=cache,target=/root/.m2/repository \
    ./mvnw dependency:resolve dependency:resolve-plugins

COPY src ./src
RUN --mount=type=cache,target=/root/.m2/repository \
    ./mvnw package -DskipTests -Dmaven.javadoc.skip=true

# Extract layers for optimized Docker layering (Spring Boot 3+)
RUN java -Djarmode=layertools -jar target/*.jar extract --destination /extracted

# ---- Test (optional CI target) ----
FROM build AS test
RUN --mount=type=cache,target=/root/.m2/repository \
    ./mvnw test

# ---- Production ----
FROM eclipse-temurin:21-jre AS production
WORKDIR /app

# Spring Boot layered extraction (most stable layers first)
COPY --from=build /extracted/dependencies/ ./
COPY --from=build /extracted/spring-boot-loader/ ./
COPY --from=build /extracted/snapshot-dependencies/ ./
COPY --from=build /extracted/application/ ./

RUN groupadd -r app && useradd -r -g app app
USER app

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
  CMD curl -f http://localhost:8080/actuator/health || exit 1

ENTRYPOINT ["java", "org.springframework.boot.loader.launch.JarLauncher"]
```

Key Java decisions:
- `eclipse-temurin:21-jdk` for build, `eclipse-temurin:21-jre` for production -- JDK has compiler tools (~700 MB), JRE has runtime only (~220 MB).
- Spring Boot layered extraction -- Splits the JAR into layers ordered by change frequency. Dependencies (rarely change) cache separately from application code (changes often).
- Maven wrapper (`mvnw`) -- Ensures consistent Maven version. ALWAYS commit the wrapper to version control.
- `dependency:resolve` in separate step -- Caches Maven downloads independently from compilation.

**Final image size:** ~220-300 MB.

---

## Rust

Rust produces statically linked binaries (with musl) that run on `scratch`, similar to Go.

```dockerfile
# syntax=docker/dockerfile:1

FROM rust:1.77-alpine AS build
RUN apk add --no-cache musl-dev

WORKDIR /src

# Cache dependency compilation (Rust's biggest bottleneck)
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    --mount=type=cache,target=/src/target \
    cargo build --release
# Remove the dummy build artifact
RUN rm -rf src target/release/deps/$(echo "${PWD##*/}" | tr '-' '_')*

# Build the real application
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    --mount=type=cache,target=/src/target \
    cargo build --release \
    && cp target/release/myapp /usr/local/bin/myapp

# ---- Test (optional CI target) ----
FROM build AS test
RUN --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    --mount=type=cache,target=/src/target \
    cargo test

# ---- Production ----
FROM scratch AS production
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=build /usr/local/bin/myapp /myapp
USER 65534:65534
ENTRYPOINT ["/myapp"]
```

Key Rust decisions:
- `rust:1.77-alpine` + `musl-dev` -- Alpine uses musl libc, producing fully static binaries. ALWAYS use alpine for scratch targets.
- Dummy `main.rs` trick -- Compiles dependencies first. When only source code changes, dependencies are cached. This saves 5-15 minutes on large projects.
- Cache mounts on cargo registry and target -- Rust compilation is slow. Cache mounts persist compiled dependencies across builds.
- `cp` from cache mount target -- Cache mounts are not included in the layer, so ALWAYS copy the binary out before the RUN ends.

**Final image size:** ~5-15 MB (binary only, no OS).

---

## .NET

.NET uses the SDK for building and the ASP.NET runtime for running.

```dockerfile
# syntax=docker/dockerfile:1

FROM mcr.microsoft.com/dotnet/sdk:8.0 AS build
WORKDIR /src

# Restore dependencies (cached unless .csproj files change)
COPY *.sln .
COPY src/MyApp/*.csproj src/MyApp/
COPY src/MyApp.Tests/*.csproj src/MyApp.Tests/
RUN --mount=type=cache,target=/root/.nuget/packages \
    dotnet restore

# Build
COPY src/ src/
RUN --mount=type=cache,target=/root/.nuget/packages \
    dotnet publish src/MyApp/MyApp.csproj \
    -c Release \
    -o /app/publish \
    --no-restore

# ---- Test (optional CI target) ----
FROM build AS test
RUN --mount=type=cache,target=/root/.nuget/packages \
    dotnet test --no-restore --verbosity normal

# ---- Production ----
FROM mcr.microsoft.com/dotnet/aspnet:8.0 AS production
WORKDIR /app

COPY --from=build /app/publish .

RUN groupadd -r app && useradd -r -g app app
USER app

EXPOSE 8080
ENV ASPNETCORE_URLS=http://+:8080

HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
  CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["dotnet", "MyApp.dll"]
```

Key .NET decisions:
- `dotnet/sdk:8.0` for build, `dotnet/aspnet:8.0` for production -- SDK is ~900 MB (includes compiler, NuGet, analyzers). ASP.NET runtime is ~220 MB.
- Copy `.csproj` files first, then `dotnet restore` -- NuGet restore is cached until project references change.
- `dotnet publish -c Release` -- Produces optimized, ready-to-deploy output.
- `--no-restore` on publish -- Skips redundant restore since it was done in a prior step.
- `ASPNETCORE_URLS` -- Configures Kestrel to listen on the correct port.

For self-contained deployments (no runtime needed):

```dockerfile
RUN dotnet publish src/MyApp/MyApp.csproj \
    -c Release \
    -r linux-x64 \
    --self-contained true \
    -p:PublishSingleFile=true \
    -p:PublishTrimmed=true \
    -o /app/publish
```

With self-contained + trimmed, the final image can use `mcr.microsoft.com/dotnet/runtime-deps:8.0` (~30 MB) or even `alpine`.

**Final image size:** ~220-300 MB (framework-dependent) or ~80-120 MB (self-contained + trimmed).
