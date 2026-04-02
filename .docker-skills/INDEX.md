# Docker Claude Skill Package — Skill Index

> Complete catalog of all 22 skills in the Docker & Docker Compose skill package.

## Quick Navigation

| Category | Skills | Description |
|----------|--------|-------------|
| core/ | 3 | Architecture, security, networking fundamentals |
| syntax/ | 7 | Dockerfile, BuildKit, Compose, CLI reference |
| impl/ | 6 | Build optimization, CI/CD, production, storage |
| errors/ | 4 | Build, runtime, networking, Compose troubleshooting |
| agents/ | 2 | Validation and generation agents |

---

## docker-core/ (3 skills)

### [docker-core-architecture](skills/source/docker-core/docker-core-architecture/SKILL.md)
> Guides Docker Engine architecture including daemon, containerd, runc, OCI standards, image and container concepts, layer filesystem, build context model, and Docker object lifecycle. Activates when explaining Docker concepts, understanding image layers, reasoning about container isolation, or setting up Docker environments.

### [docker-core-security](skills/source/docker-core/docker-core-security/SKILL.md)
> Guides Docker security including image scanning with Docker Scout and Trivy, rootless Docker, non-root containers, Linux capabilities management, read-only filesystems, seccomp and AppArmor profiles, content trust, resource limits, and Docker Bench for Security. Activates when hardening containers, scanning images for vulnerabilities, implementing least-privilege containers, or auditing Docker security.

### [docker-core-networking](skills/source/docker-core/docker-core-networking/SKILL.md)
> Guides Docker networking including bridge, host, overlay, macvlan, ipvlan, and none network drivers, DNS resolution and service discovery, port mapping and publishing, network isolation patterns, user-defined vs default bridge networks, and container-to-container communication. Activates when configuring Docker networks, debugging connectivity, setting up service discovery, or designing network architecture.

---

## docker-syntax/ (7 skills)

### [docker-syntax-dockerfile](skills/source/docker-syntax/docker-syntax-dockerfile/SKILL.md)
> Complete Dockerfile instruction reference including FROM, RUN, CMD, ENTRYPOINT, COPY, ADD, ENV, ARG, EXPOSE, VOLUME, WORKDIR, USER, HEALTHCHECK, LABEL, SHELL, STOPSIGNAL, ONBUILD, and parser directives. Activates when writing Dockerfiles, choosing between CMD and ENTRYPOINT, configuring COPY vs ADD, setting health checks, or understanding instruction syntax and behavior.

### [docker-syntax-buildkit](skills/source/docker-syntax/docker-syntax-buildkit/SKILL.md)
> Guides BuildKit-specific Dockerfile features including syntax directive, heredoc syntax for multi-line RUN, cache mounts for package managers, secret mounts for credentials, SSH mounts for private repos, bind mounts, tmpfs mounts, cache backends, and platform-specific ARGs. Activates when optimizing Dockerfile builds, mounting secrets during build, caching package manager downloads, or using heredoc syntax in RUN instructions.

### [docker-syntax-multistage](skills/source/docker-syntax/docker-syntax-multistage/SKILL.md)
> Guides Docker multi-stage build patterns including stage naming with FROM AS, COPY --from for artifact extraction, parallel build stages, --target for partial builds, builder pattern for compiled languages, test stages, shared dependency stages, and minimal production images. Activates when optimizing Docker image size, separating build and runtime dependencies, creating production-ready images, or implementing CI/CD build pipelines.

### [docker-syntax-compose-services](skills/source/docker-syntax/docker-syntax-compose-services/SKILL.md)
> Complete Docker Compose service configuration reference including image, build, command, ports, environment, volumes, networks, depends_on with health conditions, healthcheck, deploy with resource limits, restart policies, logging, profiles, extends, and all other service attributes. Activates when writing compose.yaml services, configuring service dependencies, setting up health checks, managing port mappings, or configuring resource limits.

### [docker-syntax-compose-resources](skills/source/docker-syntax/docker-syntax-compose-resources/SKILL.md)
> Guides Docker Compose infrastructure resources including top-level networks, volumes, configs, and secrets definitions, network drivers and IPAM configuration, external resources, mounting configs and secrets in services, volume drivers and options, and network aliases. Activates when defining Compose networks, creating named volumes, managing configs or secrets, or configuring network infrastructure in compose.yaml.

### [docker-syntax-cli-containers](skills/source/docker-syntax/docker-syntax-cli-containers/SKILL.md)
> Complete Docker container CLI reference including docker run with all major flags, exec, logs, inspect with Go templates, stats, top, events, container lifecycle commands (create, start, stop, restart, kill, rm, prune), cp, diff, and ps with filters and format. Activates when running containers, executing commands in containers, reading logs, inspecting container state, or managing container lifecycle.

### [docker-syntax-cli-images](skills/source/docker-syntax/docker-syntax-cli-images/SKILL.md)
> Complete Docker image and system CLI reference including docker build and buildx, pull, push, tag, image listing with filters and format, image and system pruning, save/load for offline transfer, history, manifest for multi-platform, and system management commands (df, info, version, context). Activates when building images, managing registries, cleaning up disk space, transferring images, or checking system status.

---

## docker-impl/ (6 skills)

### [docker-impl-build-optimization](skills/source/docker-impl/docker-impl-build-optimization/SKILL.md)
> Guides Docker build optimization including layer caching rules, cache invalidation triggers, instruction ordering for cache efficiency, .dockerignore patterns, build context optimization, cache mount patterns for all major package managers, bind mounts for large contexts, and CI/CD cache sharing with cache backends. Activates when optimizing Docker build times, reducing image sizes, fixing cache invalidation issues, or configuring build caching in CI/CD pipelines.

### [docker-impl-production](skills/source/docker-impl/docker-impl-production/SKILL.md)
> Guides production-ready Docker patterns including base image selection (scratch, alpine, slim, distroless), non-root USER configuration, signal handling with exec form and init processes, HEALTHCHECK patterns, entrypoint scripts with exec, OCI metadata labels, reproducible builds with digest pinning, and minimal attack surface. Activates when preparing containers for production, choosing base images, configuring health checks, writing entrypoint scripts, or hardening Dockerfiles.

### [docker-impl-storage](skills/source/docker-impl/docker-impl-storage/SKILL.md)
> Guides Docker storage implementation including named volumes vs bind mounts vs tmpfs, --mount vs -v syntax comparison, volume drivers (local, NFS, CIFS), backup and restore procedures, read-only volumes, database persistence patterns, Compose volume integration, and storage cleanup strategies. Activates when persisting data, mounting host directories, configuring database storage, backing up volumes, or managing disk space.

### [docker-impl-cicd](skills/source/docker-impl/docker-impl-cicd/SKILL.md)
> Guides Docker CI/CD integration including GitHub Actions workflows with docker/build-push-action, registry authentication for Docker Hub and GHCR, multi-platform builds with buildx and QEMU, cache sharing in CI using GitHub Actions cache backend, build matrix strategies, image tagging conventions, and automated image publishing. Activates when setting up Docker CI/CD pipelines, pushing images to registries, building multi-platform images, or optimizing CI build times.

### [docker-impl-compose-workflows](skills/source/docker-impl/docker-impl-compose-workflows/SKILL.md)
> Guides Docker Compose development workflows including profiles for optional services, extends and include directives, merge and override patterns with multiple Compose files, compose.override.yaml conventions, environment variable precedence and .env file rules, compose watch for live file sync, and remote Compose file loading. Activates when setting up development workflows, managing environment-specific configs, using profiles, merging Compose files, or configuring live reload with compose watch.

### [docker-impl-go-templates](skills/source/docker-impl/docker-impl-go-templates/SKILL.md)
> Guides Go template syntax for Docker --format flags including template functions, conditional expressions, range loops, table formatting, JSON output, and 30+ ready-to-use format patterns for inspect, ps, images, stats, network, and volume commands. Activates when formatting Docker command output, extracting specific fields from inspect, creating custom table output, or scripting Docker commands.

---

## docker-errors/ (4 skills)

### [docker-errors-build](skills/source/docker-errors/docker-errors-build/SKILL.md)
> Diagnoses and resolves Docker build errors including COPY/ADD file not found, build context too large, unexpected cache invalidation, BuildKit mount errors, multi-stage reference errors, base image pull failures, platform mismatch, ARG/ENV scope issues, permission errors during build, and Dockerfile syntax errors. Activates when encountering docker build failures, debugging cache issues, fixing COPY errors, or troubleshooting BuildKit problems.

### [docker-errors-runtime](skills/source/docker-errors/docker-errors-runtime/SKILL.md)
> Diagnoses and resolves Docker container runtime errors including OOM killed, permission denied, port already in use, container exits immediately, exec format error, read-only filesystem write attempts, PID limit exceeded, resource exhaustion, and provides the container debugging workflow (logs, exec, inspect, events). Activates when containers crash, exit unexpectedly, run out of memory, have permission issues, or need debugging.

### [docker-errors-compose](skills/source/docker-errors/docker-errors-compose/SKILL.md)
> Diagnoses and resolves Docker Compose errors including service dependency failures, orphan container warnings, port conflicts between services, volume mount permission issues, environment variable interpolation errors, profile dependency resolution, build context errors, and compose config validation failures. Activates when docker compose up fails, services won't start, dependencies time out, or Compose file has syntax errors.

### [docker-errors-networking](skills/source/docker-errors/docker-errors-networking/SKILL.md)
> Diagnoses and resolves Docker networking errors including DNS resolution failures between containers, connection refused, port mapping not working, default bridge communication issues, overlay network problems, firewall and iptables conflicts, container cannot reach internet, and network subnet conflicts. Activates when containers cannot communicate, DNS resolution fails, ports are not accessible, or network connectivity is broken.

---

## docker-agents/ (2 skills)

### [docker-agents-generator](skills/source/docker-agents/docker-agents-generator/SKILL.md)
> Generates production-ready Dockerfiles and Docker Compose configurations from application requirements. Provides language-specific templates for Node.js, Python, Go, Java, Rust, and .NET, development and production Compose stacks, proper security defaults, health checks, and .dockerignore generation. Activates when generating Dockerfiles from scratch, containerizing an existing application, creating Compose configurations, or scaffolding Docker infrastructure for a project.

### [docker-agents-review](skills/source/docker-agents/docker-agents-review/SKILL.md)
> Validates Docker configurations by checking Dockerfiles for best practices, Compose files for correctness, security compliance, build optimization, and known anti-patterns. Provides structured validation checklists for Dockerfile review, Compose review, security audit, and production readiness assessment. Activates when reviewing Dockerfiles, validating Compose files, auditing container security, or checking Docker configurations before deployment.
