# CI/CD Anti-Patterns

## AP-01: Hardcoded Registry Credentials

```yaml
# BAD: Credentials in plaintext in workflow file
- name: Login
  run: docker login -u myuser -p mypassword123

# BAD: Credentials in environment variables defined in the workflow
env:
  DOCKER_PASSWORD: supersecret
```

**Why it fails**: Workflow files are committed to the repository. Anyone with read access sees the credentials. Credentials in logs are partially masked but not reliably.

**ALWAYS do this instead**:

```yaml
# GOOD: Use GitHub Secrets
- uses: docker/login-action@v3
  with:
    username: ${{ vars.DOCKERHUB_USERNAME }}
    password: ${{ secrets.DOCKERHUB_TOKEN }}
```

---

## AP-02: No Cache Configuration

```yaml
# BAD: Every build starts from scratch
- uses: docker/build-push-action@v6
  with:
    context: .
    push: true
    tags: user/myapp:latest
    # No cache-from or cache-to
```

**Why it fails**: Without cache, every CI build downloads all base image layers, reinstalls all dependencies, and recompiles everything. A 2-minute cached build becomes a 15-minute full build.

**ALWAYS do this instead**:

```yaml
# GOOD: GitHub Actions cache with mode=max
- uses: docker/build-push-action@v6
  with:
    context: .
    push: true
    tags: user/myapp:latest
    cache-from: type=gha
    cache-to: type=gha,mode=max
```

---

## AP-03: Using `mode=min` Cache (the Default)

```yaml
# BAD: mode=min only caches final exported layers
cache-to: type=gha
# This is equivalent to:
cache-to: type=gha,mode=min
```

**Why it fails**: `mode=min` only caches layers that appear in the final image. Multi-stage build intermediate layers (dependency installation, compilation) are NOT cached. The most expensive steps are rebuilt every time.

**ALWAYS do this instead**:

```yaml
# GOOD: mode=max caches ALL intermediate layers
cache-to: type=gha,mode=max
```

---

## AP-04: Pushing Images from Pull Requests

```yaml
# BAD: Pushes from any event, including PRs
- uses: docker/build-push-action@v6
  with:
    push: true
    tags: user/myapp:latest
```

**Why it fails**: Pull requests from forks can inject malicious code. Pushing from PRs allows anyone to overwrite your production image tags by opening a PR.

**ALWAYS do this instead**:

```yaml
# GOOD: Only push on non-PR events
- uses: docker/build-push-action@v6
  with:
    push: ${{ github.event_name != 'pull_request' }}
    tags: ${{ steps.meta.outputs.tags }}
```

---

## AP-05: Using `docker build` Instead of `buildx`

```yaml
# BAD: Legacy builder, no cache export, no multi-platform
- run: docker build -t user/myapp:latest .
- run: docker push user/myapp:latest
```

**Why it fails**: The legacy `docker build` command does not support cache backends, multi-platform builds, build secrets, or SBOM/provenance attestations. It is functionally inferior in every CI scenario.

**ALWAYS do this instead**:

```yaml
# GOOD: Use setup-buildx-action + build-push-action
- uses: docker/setup-buildx-action@v3
- uses: docker/build-push-action@v6
  with:
    context: .
    push: true
    tags: user/myapp:latest
    cache-from: type=gha
    cache-to: type=gha,mode=max
```

---

## AP-06: Multi-Platform Build with `--load`

```yaml
# BAD: --load does not work with multi-platform
- uses: docker/build-push-action@v6
  with:
    platforms: linux/amd64,linux/arm64
    load: true  # ERROR: multi-platform build cannot be loaded
```

**Why it fails**: The local Docker image store only supports a single platform per tag. Multi-platform manifests (manifest lists) can only exist in a registry.

**Do this instead**:

```yaml
# GOOD: Push multi-platform to registry
- uses: docker/build-push-action@v6
  with:
    platforms: linux/amd64,linux/arm64
    push: true
    tags: user/myapp:latest

# GOOD: Load single platform locally (for testing)
- uses: docker/build-push-action@v6
  with:
    load: true
    tags: myapp:test
```

---

## AP-07: Missing QEMU Setup for Multi-Platform

```yaml
# BAD: No QEMU — arm64 build fails
- uses: docker/setup-buildx-action@v3
- uses: docker/build-push-action@v6
  with:
    platforms: linux/amd64,linux/arm64
    # Fails: arm64 emulation not available
```

**Why it fails**: GitHub Actions runners are x86-64. Building for arm64 or other architectures requires QEMU user-space emulation, which must be installed explicitly.

**ALWAYS do this instead**:

```yaml
# GOOD: Install QEMU before buildx
- uses: docker/setup-qemu-action@v3
- uses: docker/setup-buildx-action@v3
- uses: docker/build-push-action@v6
  with:
    platforms: linux/amd64,linux/arm64
```

---

## AP-08: Using QEMU When Cross-Compilation is Possible

```yaml
# BAD: QEMU emulates entire Go compilation — extremely slow
# Dockerfile:
FROM golang:1.22
COPY . .
RUN go build -o /app
```

**Why it fails**: QEMU emulation is 5-10x slower than native execution. For compiled languages (Go, Rust, C/C++), the compiler can target other architectures natively without emulation.

**ALWAYS do this instead for compiled languages**:

```dockerfile
# GOOD: Cross-compile on native platform
FROM --platform=$BUILDPLATFORM golang:1.22 AS build
ARG TARGETOS TARGETARCH
WORKDIR /src
COPY . .
RUN GOOS=$TARGETOS GOARCH=$TARGETARCH CGO_ENABLED=0 go build -o /app

FROM alpine:3.21
COPY --from=build /app /usr/bin/app
```

---

## AP-09: Manual Tag Management

```yaml
# BAD: Manually construct tags — error-prone and incomplete
- run: |
    VERSION=${GITHUB_REF#refs/tags/v}
    docker tag myapp user/myapp:$VERSION
    docker tag myapp user/myapp:latest
    docker push user/myapp:$VERSION
    docker push user/myapp:latest
```

**Why it fails**: Manual tag logic is fragile, misses edge cases (PRs, branches, SHA tags), and does not generate OCI-compliant labels. Different workflows implement tagging differently, causing inconsistency.

**ALWAYS do this instead**:

```yaml
# GOOD: Use metadata-action for consistent, automatic tagging
- uses: docker/metadata-action@v5
  id: meta
  with:
    images: user/myapp
    tags: |
      type=semver,pattern={{version}}
      type=semver,pattern={{major}}.{{minor}}
      type=ref,event=branch
      type=ref,event=pr
      type=sha,prefix=sha-

- uses: docker/build-push-action@v6
  with:
    tags: ${{ steps.meta.outputs.tags }}
    labels: ${{ steps.meta.outputs.labels }}
```

---

## AP-10: Secrets via Build Arguments

```yaml
# BAD: Secret visible in docker history
- uses: docker/build-push-action@v6
  with:
    build-args: |
      NPM_TOKEN=${{ secrets.NPM_TOKEN }}
      DATABASE_URL=${{ secrets.DATABASE_URL }}
```

**Why it fails**: Build arguments are recorded in image metadata and visible via `docker history --no-trunc`. Anyone who pulls the image can extract the secrets.

**ALWAYS do this instead**:

```yaml
# GOOD: Use secret mounts — not recorded in image layers
- uses: docker/build-push-action@v6
  with:
    secrets: |
      "npm_token=${{ secrets.NPM_TOKEN }}"
```

```dockerfile
RUN --mount=type=secret,id=npm_token \
    NPM_TOKEN=$(cat /run/secrets/npm_token) npm ci
```

---

## AP-11: No Vulnerability Scanning

```yaml
# BAD: Build and push without scanning
- uses: docker/build-push-action@v6
  with:
    push: true
    tags: user/myapp:latest
    # No security scan before deployment
```

**Why it fails**: Vulnerable base images and dependencies are silently promoted to production. Known CVEs with available patches go undetected.

**ALWAYS do this instead**:

```yaml
# GOOD: Scan before push
- uses: docker/build-push-action@v6
  with:
    load: true
    tags: myapp:scan

- uses: docker/scout-action@v1
  with:
    command: cves
    image: local://myapp:scan
    only-severities: critical,high
    exit-code: true

- uses: docker/build-push-action@v6
  if: success()
  with:
    push: true
    tags: user/myapp:latest
```

---

## AP-12: No Cache Scope in Matrix Builds

```yaml
# BAD: All matrix jobs share the same cache, overwriting each other
strategy:
  matrix:
    service: [api, worker, web]
steps:
  - uses: docker/build-push-action@v6
    with:
      file: ${{ matrix.service }}/Dockerfile
      cache-from: type=gha
      cache-to: type=gha,mode=max
```

**Why it fails**: Without scope, all three matrix jobs read and write to the same cache namespace. The last job to finish overwrites the cache of the other two. Only one service benefits from caching.

**ALWAYS do this instead**:

```yaml
# GOOD: Scoped cache per matrix entry
- uses: docker/build-push-action@v6
  with:
    file: ${{ matrix.service }}/Dockerfile
    cache-from: type=gha,scope=${{ matrix.service }}
    cache-to: type=gha,scope=${{ matrix.service }},mode=max
```

---

## AP-13: Using `latest` Tag as the Only Tag

```yaml
# BAD: Only latest — no way to pin or rollback
tags: user/myapp:latest
```

**Why it fails**: `latest` is mutable. If a bad version is pushed, there is no way to roll back to the previous version. Kubernetes deployments with `imagePullPolicy: Always` silently pick up breaking changes.

**ALWAYS do this instead**:

```yaml
# GOOD: Immutable semver tags + latest for convenience
tags: |
  type=semver,pattern={{version}}
  type=semver,pattern={{major}}.{{minor}}
  type=sha,prefix=sha-
  type=raw,value=latest,enable=${{ github.ref == format('refs/heads/{0}', 'main') }}
```

---

## AP-14: Skipping Buildx Setup

```yaml
# BAD: Relying on the default Docker builder in CI
- run: docker build -t myapp .
```

**Why it fails**: Without `docker/setup-buildx-action`, the builder does not support:
- Cache export/import (`--cache-from`/`--cache-to`)
- Multi-platform builds (`--platform`)
- Build secrets (`--secret`)
- SBOM and provenance attestations
- Parallel stage execution

**ALWAYS do this instead**:

```yaml
- uses: docker/setup-buildx-action@v3
- uses: docker/build-push-action@v6
  with:
    context: .
    # All BuildKit features now available
```

---

## AP-15: Not Using Provenance/SBOM

```yaml
# BAD: No supply chain metadata
- uses: docker/build-push-action@v6
  with:
    push: true
    tags: user/myapp:latest
    provenance: false
```

**Why it fails**: Without provenance and SBOM attestations, there is no verifiable record of how the image was built, what tools were used, or what packages it contains. This blocks compliance with SLSA and other supply chain security frameworks.

**ALWAYS do this instead for production images**:

```yaml
- uses: docker/build-push-action@v6
  with:
    push: true
    tags: user/myapp:latest
    provenance: mode=max
    sbom: true
```

---

## Summary Table

| # | Anti-Pattern | Impact | Fix |
|---|-------------|--------|-----|
| AP-01 | Hardcoded credentials | Credential leak | GitHub Secrets + login-action |
| AP-02 | No cache | Slow builds (5-15 min) | `type=gha,mode=max` |
| AP-03 | mode=min cache | Partial cache hits | `mode=max` |
| AP-04 | Push from PRs | Malicious image injection | Guard with event check |
| AP-05 | Legacy docker build | No modern features | setup-buildx + build-push-action |
| AP-06 | Multi-platform + load | Build failure | Push to registry |
| AP-07 | Missing QEMU | Cross-platform failure | setup-qemu-action |
| AP-08 | QEMU for compiled langs | 5-10x slower builds | Cross-compilation |
| AP-09 | Manual tags | Inconsistent, fragile | metadata-action |
| AP-10 | Secrets in build-args | Secret exposure | Secret mounts |
| AP-11 | No scanning | Vulnerable images in prod | Docker Scout / Trivy |
| AP-12 | No cache scope | Cache thrashing in matrix | Scoped cache |
| AP-13 | Only `latest` tag | No rollback | Semver + SHA tags |
| AP-14 | No buildx setup | Missing BuildKit features | setup-buildx-action |
| AP-15 | No provenance/SBOM | No supply chain proof | Enable attestations |

---

## Official Sources

- https://docs.docker.com/build/ci/github-actions/
- https://docs.docker.com/build/cache/backends/gha/
- https://docs.docker.com/build/building/multi-platform/
- https://github.com/docker/build-push-action
- https://github.com/docker/metadata-action
