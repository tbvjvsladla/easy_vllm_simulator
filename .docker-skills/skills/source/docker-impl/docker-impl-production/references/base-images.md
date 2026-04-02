# Base Image Comparison

## Overview Table

| Image | Compressed Size | Shell | Package Mgr | libc | Best For |
|-------|----------------|-------|-------------|------|----------|
| `scratch` | 0 MB | No | No | None | Statically compiled Go, Rust binaries |
| `alpine:3.21` | ~3.5 MB | ash | apk | musl | Minimal containers needing a package manager |
| `debian:bookworm-slim` | ~30 MB | bash | apt | glibc | Applications requiring glibc compatibility |
| `ubuntu:24.04` | ~30 MB | bash | apt | glibc | Applications needing Ubuntu-specific packages |
| `gcr.io/distroless/static-debian12` | ~2 MB | No | No | None | Static binaries with CA certs and timezone data |
| `gcr.io/distroless/base-debian12` | ~20 MB | No | No | glibc | Dynamic binaries needing glibc |
| `gcr.io/distroless/cc-debian12` | ~22 MB | No | No | glibc + libstdc++ | C++ applications |
| `gcr.io/distroless/java21-debian12` | ~90 MB | No | No | glibc + JRE | Java 21 applications |
| `gcr.io/distroless/python3-debian12` | ~50 MB | No | No | glibc + Python | Python applications |
| `gcr.io/distroless/nodejs22-debian12` | ~60 MB | No | No | glibc + Node | Node.js 22 applications |

---

## scratch

The empty image. Contains absolutely nothing -- no filesystem, no shell, no libraries.

### When to Use
- Statically compiled Go binaries (`CGO_ENABLED=0`)
- Statically compiled Rust binaries (`target x86_64-unknown-linux-musl`)
- Any binary with zero runtime dependencies

### Pros
- Smallest possible image (0 bytes base)
- Zero attack surface -- nothing to exploit
- No shell means no shell-based attacks

### Cons
- No shell for debugging (`docker exec` is useless)
- No CA certificates (must copy `/etc/ssl/certs/ca-certificates.crt`)
- No timezone data (must copy `/usr/share/zoneinfo/`)
- No user database (must copy `/etc/passwd` or use numeric UID)
- No DNS resolution config (must copy `/etc/nsswitch.conf` for some apps)

### Example

```dockerfile
FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN CGO_ENABLED=0 go build -ldflags="-s -w" -o /app

FROM scratch
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=build /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=build /etc/passwd /etc/passwd
COPY --from=build /app /app
USER 65534:65534
ENTRYPOINT ["/app"]
```

---

## alpine

Minimal Linux distribution using musl libc and BusyBox.

### When to Use
- Applications that need a package manager at build or runtime
- When you need a shell for entrypoint scripts
- Interpreted languages (Python, Ruby, Node) where distroless is not available or practical

### Pros
- Very small (~6 MB uncompressed)
- Has `apk` package manager for runtime dependencies
- Has shell (ash) for entrypoint scripts and debugging
- Frequent security updates

### Cons
- Uses musl libc instead of glibc -- some applications have compatibility issues
- DNS resolution differs from glibc (musl uses different resolver)
- Python packages with C extensions may need recompilation
- Performance differences in some workloads (musl vs glibc string operations)

### musl Compatibility Issues

Applications known to have issues with musl:
- Python packages using NumPy/SciPy (need to compile from source or use `*-musllinux` wheels)
- Applications using `nsswitch.conf` for name resolution
- JVM applications (older versions, modern JVMs work fine on musl)
- Applications using `gethostbyname` with certain DNS configurations

ALWAYS test thoroughly when migrating from glibc-based images to alpine.

### Example

```dockerfile
FROM alpine:3.21
RUN apk add --no-cache tini
RUN addgroup -S -g 1001 appuser && adduser -S -u 1001 -G appuser appuser
COPY --from=build /app /usr/bin/app
USER 1001:1001
ENTRYPOINT ["/sbin/tini", "--"]
CMD ["/usr/bin/app"]
```

---

## slim (Debian/Ubuntu Slim Variants)

Reduced versions of full Debian/Ubuntu images with documentation, man pages, and locale data removed.

### When to Use
- Applications requiring glibc compatibility
- When alpine/musl causes issues
- Python applications with native C extensions
- Applications needing `apt` for runtime dependencies

### Pros
- glibc compatibility -- works with all Linux binaries
- `apt` package manager available
- Familiar Debian/Ubuntu ecosystem
- Smaller than full images by 50-70%

### Cons
- Larger than alpine (~30-80 MB vs ~6 MB)
- More packages installed than strictly necessary
- Slower security update cycle than alpine

### Available Variants

| Variant | Base | Example |
|---------|------|---------|
| `debian:bookworm-slim` | Debian 12 | General purpose |
| `node:20-bookworm-slim` | Debian 12 + Node | Node.js applications |
| `python:3.12-slim-bookworm` | Debian 12 + Python | Python applications |
| `openjdk:21-slim-bookworm` | Debian 12 + JDK | Java applications |
| `ruby:3.3-slim-bookworm` | Debian 12 + Ruby | Ruby applications |

---

## distroless (Google Container Tools)

Minimal images containing ONLY the application runtime and its dependencies. No shell, no package manager, no utilities.

### When to Use
- Production deployments where security is critical
- When you do not need runtime debugging via shell
- Language-specific runtimes (Java, Python, Node.js, .NET)
- Static binaries that need CA certs and timezone data

### Pros
- Minimal attack surface -- no shell, no package manager
- Reduced CVE exposure -- fewer packages to scan
- Smaller than slim images
- Pre-configured `nonroot` user (UID 65534)
- Includes CA certificates and timezone data (unlike scratch)

### Cons
- No shell -- cannot `docker exec` into container for debugging
- No package manager -- cannot install tools at runtime
- Debugging requires a separate debug image (`*:debug` tags include busybox)
- Limited to Google's supported runtimes

### Image Tags

| Tag | Contents |
|-----|----------|
| `latest` | Runs as root |
| `nonroot` | Runs as UID 65534 |
| `debug` | Includes busybox shell for debugging |
| `debug-nonroot` | Debug + nonroot |

ALWAYS use the `nonroot` tag in production.

### Example

```dockerfile
FROM golang:1.22 AS build
WORKDIR /src
COPY . .
RUN CGO_ENABLED=0 go build -ldflags="-s -w" -o /app

FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=build /app /app
ENTRYPOINT ["/app"]
```

---

## Full Images (ubuntu, debian)

Complete OS images with all standard utilities.

### When to Use
- Development and CI/CD stages
- Applications with complex system dependencies
- When debugging tools are needed
- Base for custom organization images

### Pros
- Full toolchain available
- Maximum compatibility
- Easy debugging

### Cons
- Large image size (75-200 MB)
- Large attack surface
- Many unnecessary packages
- Slower pull times

NEVER use full images as production runtime images. ALWAYS use them only in build stages and use a minimal image for the runtime stage.

---

## Language-Specific Recommendations

| Language | Build Stage | Runtime Stage |
|----------|-----------|--------------|
| Go (static) | `golang:1.22` | `scratch` or `distroless/static` |
| Go (CGO) | `golang:1.22` | `distroless/base` or `alpine` |
| Rust (static) | `rust:1.77` | `scratch` or `distroless/static` |
| Rust (dynamic) | `rust:1.77` | `distroless/cc` or `debian:slim` |
| Node.js | `node:20` | `node:20-slim` or `distroless/nodejs22` |
| Python | `python:3.12` | `python:3.12-slim` or `distroless/python3` |
| Java | `eclipse-temurin:21-jdk` | `eclipse-temurin:21-jre-alpine` or `distroless/java21` |
| .NET | `mcr.microsoft.com/dotnet/sdk:8.0` | `mcr.microsoft.com/dotnet/runtime:8.0-alpine` |
| Ruby | `ruby:3.3` | `ruby:3.3-slim` |
| PHP | `php:8.3-cli` or `php:8.3-fpm` | `php:8.3-fpm-alpine` |

---

## Size Comparison Example (Go Application)

| Runtime Image | Final Image Size |
|--------------|-----------------|
| `golang:1.22` (full, no multi-stage) | ~850 MB |
| `ubuntu:24.04` | ~85 MB |
| `debian:bookworm-slim` | ~40 MB |
| `alpine:3.21` | ~15 MB |
| `gcr.io/distroless/static` | ~8 MB |
| `scratch` | ~7 MB |

The binary itself is ~7 MB. Everything above that is OS overhead.
