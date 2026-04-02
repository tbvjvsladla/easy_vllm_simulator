# Docker Claude Skill Package — Requirements

> Quality guarantees and per-area requirements for all Docker skills.

---

## Global Requirements

### Format
- YAML frontmatter with `name` and `description` (including trigger words)
- SKILL.md as entry point, < 500 lines
- Heavy content offloaded to `references/` directory
- Structure: Quick Reference > Decision Trees > Patterns > Reference Links

### Language
- English-only content (no Dutch or other languages in skills)
- Deterministic phrasing: ALWAYS/NEVER, not "you might consider"
- Actionable instructions, not explanations

### Quality
- All code examples verified against official Docker documentation
- All sources traceable to SOURCES.md approved URLs
- No deprecated features or outdated syntax
- Version-specific annotations where behavior differs between Engine versions

---

## Per-Area Requirements

### Dockerfile Skills (docker-syntax-*)
- MUST cover all commonly used instructions (FROM, RUN, COPY, ADD, CMD, ENTRYPOINT, ENV, ARG, EXPOSE, VOLUME, WORKDIR, USER, HEALTHCHECK, LABEL)
- MUST demonstrate multi-stage build patterns
- MUST include security best practices (non-root users, minimal base images, .dockerignore)
- MUST show BuildKit features and syntax (--mount, heredocs)
- MUST cover layer optimization and caching strategies

### Docker Compose Skills (docker-syntax-compose-*, docker-impl-*)
- MUST target Compose v2 specification (compose.yaml, not docker-compose.yml)
- MUST cover service definitions, networks, volumes, secrets, configs
- MUST show environment variable handling (env_file, variable substitution)
- MUST demonstrate dependency management (depends_on with healthchecks)
- MUST include profiles and extend/merge patterns

### CLI Skills (docker-syntax-cli-*)
- MUST cover container lifecycle commands (run, exec, stop, rm, logs)
- MUST cover image management (build, pull, push, tag, prune)
- MUST cover network and volume management
- MUST show filtering and formatting (--filter, --format with Go templates)

### Build & CI/CD Skills (docker-impl-*)
- MUST demonstrate multi-stage builds for production optimization
- MUST show BuildKit cache mounts and secret mounts
- MUST cover registry authentication and image pushing
- MUST include GitHub Actions / CI pipeline integration patterns
- MUST show build context optimization (.dockerignore, minimal contexts)

### Security Skills (docker-core-security-*)
- MUST cover image scanning (Docker Scout, Trivy, Snyk)
- MUST demonstrate rootless containers and user namespace mapping
- MUST show secret management (build secrets, runtime secrets, Compose secrets)
- MUST cover read-only filesystems and capability dropping
- MUST include supply chain security (image signing, SBOM)

### Networking Skills (docker-core-networking-*)
- MUST cover bridge, host, overlay, and macvlan network drivers
- MUST demonstrate service discovery and DNS resolution
- MUST show port mapping and exposure patterns
- MUST include network isolation and security groups

### Storage Skills (docker-core-storage-*)
- MUST cover volumes, bind mounts, and tmpfs mounts
- MUST demonstrate volume drivers and backup strategies
- MUST show data persistence patterns for databases
- MUST include storage cleanup and pruning

### Error & Debugging Skills (docker-errors-*)
- MUST cover common build failures and their solutions
- MUST demonstrate container debugging techniques (exec, logs, inspect, events)
- MUST show resource constraint issues (OOM, disk space, PID limits)
- MUST include networking troubleshooting
- MUST cover permission and filesystem errors

### Agent Skills (docker-agents-*)
- MUST provide Dockerfile generation from application requirements
- MUST include Compose file generation and validation
- MUST offer security audit capabilities
- MUST support migration assistance (v1 to v2 Compose, deprecated features)

---

## Cross-Cutting Requirements

### Version Coverage
- Docker Engine 24+ as minimum supported version
- Docker Compose v2 (the `docker compose` subcommand, not standalone `docker-compose`)
- BuildKit as default builder
- Note deprecated features with migration paths

### Multi-Language Coverage
- Dockerfile syntax with full instruction reference
- YAML for Compose files with schema validation awareness
- Shell scripting for entrypoint scripts and health checks
- Go template syntax for `--format` flags

### Testing & Validation
- All Dockerfile examples MUST be buildable
- All Compose files MUST pass `docker compose config` validation
- All CLI commands MUST be verified on Docker Engine 24+
- Error examples MUST show real error messages and their solutions
