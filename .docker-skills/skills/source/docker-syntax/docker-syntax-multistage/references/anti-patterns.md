# Multi-Stage Anti-Patterns

> Common mistakes in multi-stage Dockerfiles, why they are wrong, and how to fix them.
> All corrections verified against Docker Engine 24+ with BuildKit.

---

## 1. Referencing Stages by Numeric Index

**Problem:** Numeric indexes break when stages are added, removed, or reordered.

```dockerfile
# BAD: Fragile numeric reference
FROM golang:1.22
WORKDIR /src
COPY . .
RUN go build -o /app/server

FROM alpine:3.19
COPY --from=0 /app/server /usr/local/bin/server
```

```dockerfile
# GOOD: Named stage reference
FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN go build -o /app/server

FROM alpine:3.19 AS production
COPY --from=build /app/server /usr/local/bin/server
```

**Why:** Adding a new stage before the build stage changes `--from=0` to point to the wrong stage. Named references survive any reordering. ALWAYS name every stage.

---

## 2. Copying the Entire Build Stage

**Problem:** Copying everything from the build stage defeats the purpose of multi-stage builds.

```dockerfile
# BAD: Copies source code, compilers, caches, and temp files
FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN go build -o /app/server

FROM alpine:3.19
COPY --from=build /src /src
```

```dockerfile
# GOOD: Copy only the compiled artifact
FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN go build -o /app/server

FROM alpine:3.19
COPY --from=build /app/server /usr/local/bin/server
```

**Why:** The whole point of multi-stage is to exclude build artifacts. Copying the entire working directory brings compilers, source code, intermediate objects, and package caches into the production image. ALWAYS copy only the specific files needed at runtime.

---

## 3. Installing Build Tools in the Final Stage

**Problem:** Build tools in the production image waste space and create attack surface.

```dockerfile
# BAD: gcc and build-essential in production
FROM python:3.12-slim AS production
RUN apt-get update && apt-get install -y build-essential libpq-dev
COPY requirements.txt .
RUN pip install -r requirements.txt
COPY . .
CMD ["python", "app.py"]
```

```dockerfile
# GOOD: Build tools in builder stage only
FROM python:3.12-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential libpq-dev \
    && rm -rf /var/lib/apt/lists/*
RUN python -m venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

FROM python:3.12-slim AS production
RUN apt-get update && apt-get install -y --no-install-recommends \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /opt/venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"
COPY . .
CMD ["python", "app.py"]
```

**Why:** `build-essential` adds ~200 MB. `libpq-dev` (headers) is ~15 MB while `libpq5` (runtime) is ~1 MB. Build tools are NEVER needed at runtime and provide attackers with compilers.

---

## 4. Not Separating Dependency Install from Code Copy

**Problem:** Copying all source code before installing dependencies invalidates the dependency cache on every code change.

```dockerfile
# BAD: Any source change reruns npm ci
FROM node:20-slim AS build
WORKDIR /app
COPY . .
RUN npm ci
RUN npm run build
```

```dockerfile
# GOOD: Dependencies cached separately from source
FROM node:20-slim AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci

FROM deps AS build
COPY . .
RUN npm run build
```

**Why:** `npm ci` takes 30-120 seconds. By copying only `package.json` and `package-lock.json` first, the install step is cached until dependencies actually change. Source code changes only trigger the build step.

---

## 5. Using `latest` Tag on Build Stage Base Images

**Problem:** `latest` resolves to a different image over time, breaking reproducibility.

```dockerfile
# BAD: Non-deterministic base images
FROM golang:latest AS build
# ...
FROM alpine:latest AS production
```

```dockerfile
# GOOD: Pinned versions
FROM golang:1.22-alpine AS build
# ...
FROM alpine:3.19 AS production
```

```dockerfile
# BEST: Pinned by digest
FROM golang:1.22-alpine@sha256:abc123... AS build
# ...
FROM alpine:3.19@sha256:def456... AS production
```

**Why:** A build that works today may fail tomorrow when `latest` points to a new major version. Pin at least the major.minor version. For critical production images, pin by digest for full reproducibility.

---

## 6. Forgetting CA Certificates with Scratch

**Problem:** Applications making HTTPS requests fail silently on `scratch` because no CA certificates exist.

```dockerfile
# BAD: HTTPS requests will fail with x509 certificate errors
FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN CGO_ENABLED=0 go build -o /server

FROM scratch
COPY --from=build /server /server
ENTRYPOINT ["/server"]
```

```dockerfile
# GOOD: Include CA certificates for HTTPS
FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN CGO_ENABLED=0 go build -o /server

FROM scratch
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=build /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=build /server /server
ENTRYPOINT ["/server"]
```

**Why:** `scratch` is completely empty -- no certificates, no timezone data, no user database. ALWAYS copy CA certificates if the application makes HTTPS calls. ALWAYS copy zoneinfo if the application uses time zones.

---

## 7. Not Using CGO_ENABLED=0 for Scratch Targets

**Problem:** Go binaries link against glibc by default, which does not exist on `scratch` or `alpine` (which uses musl).

```dockerfile
# BAD: Binary requires glibc, fails on scratch
FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN go build -o /server

FROM scratch
COPY --from=build /server /server
ENTRYPOINT ["/server"]
# Runtime error: not found (missing dynamic linker)
```

```dockerfile
# GOOD: Static binary with no glibc dependency
FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN CGO_ENABLED=0 go build -o /server

FROM scratch
COPY --from=build /server /server
ENTRYPOINT ["/server"]
```

**Why:** Without `CGO_ENABLED=0`, Go links against glibc for DNS resolution and other OS features. The binary appears to exist but fails with "not found" because the dynamic linker (`/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2`) is missing. ALWAYS set `CGO_ENABLED=0` when targeting `scratch` or `distroless/static`.

---

## 8. Running Tests Only in CI, Not in the Dockerfile

**Problem:** Tests that run outside the Dockerfile cannot leverage Docker's caching and may pass locally but fail in the container environment.

```dockerfile
# BAD: No test stage -- tests run outside Docker
FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN go build -o /app/server

FROM alpine:3.19
COPY --from=build /app/server /usr/local/bin/server
ENTRYPOINT ["/usr/local/bin/server"]
```

```dockerfile
# GOOD: Test stage verifies code inside the same build environment
FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN go build -o /app/server

FROM build AS test
RUN go test -v -race ./...

FROM alpine:3.19 AS production
COPY --from=build /app/server /usr/local/bin/server
ENTRYPOINT ["/usr/local/bin/server"]
```

**Why:** A test stage ensures tests run in the exact same environment as the build. CI pipelines use `docker build --target test` as a gate before building production. Tests are cached by BuildKit and only rerun when code changes.

---

## 9. Duplicating Dependency Installation Across Stages

**Problem:** Multiple stages independently install the same dependencies, wasting build time and bandwidth.

```dockerfile
# BAD: Dependencies installed twice
FROM node:20-slim AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM node:20-slim AS test
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci                              # Duplicate install!
COPY . .
RUN npm test
```

```dockerfile
# GOOD: Shared dependency stage
FROM node:20-slim AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci

FROM deps AS build
COPY . .
RUN npm run build

FROM deps AS test
COPY . .
RUN npm test
```

**Why:** The `deps` stage runs once and is reused by both `build` and `test`. BuildKit caches the deps stage result, so both downstream stages start from the same cached layer.

---

## 10. Ignoring Layer Order in the Final Stage

**Problem:** Putting frequently-changing files before stable files ruins cache efficiency in the final image.

```dockerfile
# BAD: Application code copied before stable dependencies
FROM node:20-slim AS production
WORKDIR /app
COPY --from=build /app/dist ./dist           # Changes often
COPY --from=deps /app/node_modules ./node_modules  # Changes rarely
COPY package.json ./
CMD ["node", "dist/index.js"]
```

```dockerfile
# GOOD: Stable layers first, volatile layers last
FROM node:20-slim AS production
WORKDIR /app
COPY package.json ./                                 # Almost never changes
COPY --from=deps /app/node_modules ./node_modules    # Changes rarely
COPY --from=build /app/dist ./dist                   # Changes often
CMD ["node", "dist/index.js"]
```

**Why:** Docker caches layers sequentially. When `dist` changes (which happens on every code push), all layers AFTER it are invalidated. By putting `node_modules` before `dist`, the modules layer stays cached even when application code changes.

---

## 11. Not Pruning devDependencies for Node.js Production

**Problem:** Production image includes test frameworks, linters, and build tools in `node_modules`.

```dockerfile
# BAD: devDependencies shipped to production
FROM node:20-slim AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM node:20-slim AS production
WORKDIR /app
COPY --from=build /app/node_modules ./node_modules  # Includes devDeps!
COPY --from=build /app/dist ./dist
CMD ["node", "dist/index.js"]
```

```dockerfile
# GOOD: Production-only dependencies
FROM node:20-slim AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci

FROM deps AS build
COPY . .
RUN npm run build
RUN npm prune --production

FROM node:20-slim AS production
WORKDIR /app
ENV NODE_ENV=production
COPY --from=build /app/node_modules ./node_modules
COPY --from=build /app/dist ./dist
COPY package.json ./
USER node
CMD ["node", "dist/index.js"]
```

**Why:** devDependencies can add 100-500 MB to node_modules. `npm prune --production` removes them after the build step completes. ALWAYS prune before copying to the production stage.

---

## 12. Missing --no-install-recommends on apt-get

**Problem:** apt installs recommended but unnecessary packages, adding 50-200 MB to build stages.

```dockerfile
# BAD: Installs recommended packages (man pages, docs, extra tools)
FROM python:3.12-slim AS build
RUN apt-get update && apt-get install -y build-essential
```

```dockerfile
# GOOD: Only required packages
FROM python:3.12-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    && rm -rf /var/lib/apt/lists/*
```

**Why:** `--no-install-recommends` prevents apt from pulling in suggested and recommended packages. In a build stage this is less critical (the stage is discarded), but it speeds up the build and reduces the chance of conflicting packages. In a production stage it is CRITICAL for image size.
