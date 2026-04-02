# Docker Compose Error Diagnostics — Complete Reference

## YAML & File Parsing Errors

| # | Error Message | Cause | Fix |
|---|---|---|---|
| 1 | `yaml: line N: did not find expected key` | Indentation error, tab characters, or misplaced colon | Use spaces only (2-space indent standard). Check line N and surrounding lines for tabs or misaligned keys |
| 2 | `yaml: line N: mapping values are not allowed here` | Missing space after colon, or value on wrong line | Add space after colon: `key: value` not `key:value`. Check for unquoted strings containing colons |
| 3 | `yaml: line N: could not find expected ':'` | Missing colon in mapping, or incorrect list format | Ensure all mapping keys end with `: `. Check for missing `-` in list items |
| 4 | `yaml: line N: found character that cannot start any token` | Tab character in YAML | Replace all tabs with spaces. YAML forbids tabs for indentation |
| 5 | `Compose file not found` | No compose file in current directory | ALWAYS check you are in the correct directory. Create `compose.yaml` or specify with `-f path/to/compose.yaml` |
| 6 | `services is required` | Top-level `services:` key missing or empty | Add `services:` as a required top-level element with at least one service |
| 7 | `Additional property X is not allowed` | Misspelled or unsupported Compose directive | Check spelling against the Compose Specification. Verify indentation level is correct |
| 8 | `"version" is obsolete` | Deprecated `version:` field present | Remove the `version:` line. Compose v2 uses the unified Compose Specification and ignores this field |

## Service Definition Errors

| # | Error Message | Cause | Fix |
|---|---|---|---|
| 9 | `service "X" has neither an image nor a build context` | Service missing both `image:` and `build:` | Add either `image: <name>` to pull an image or `build: <path>` to build from Dockerfile |
| 10 | `invalid service name "X"` | Service name contains invalid characters | Use only lowercase/uppercase letters, digits, hyphens, underscores, and dots. Must start with letter or digit |
| 11 | `service "X" depends on undefined service "Y"` | `depends_on` references a nonexistent service | Check spelling of service name in `depends_on`. Ensure the target service is defined in the same Compose file or an included file |
| 12 | `container name "X" is already in use by container Y` | A stopped or running container with the same `container_name` exists | Run `docker rm X` to remove the stale container, or run `docker compose down` first |
| 13 | `service "X" refers to undefined network "Y"` | Service uses a network not declared in top-level `networks:` | Add the network to the top-level `networks:` section, or fix the spelling |
| 14 | `service "X" refers to undefined volume "Y"` | Named volume used in service but not declared at top level | Add the volume to the top-level `volumes:` section |
| 15 | `service "X" refers to undefined config "Y"` | Config not declared in top-level `configs:` | Add the config to the top-level `configs:` section |
| 16 | `service "X" refers to undefined secret "Y"` | Secret not declared in top-level `secrets:` | Add the secret to the top-level `secrets:` section |

## Port & Network Errors

| # | Error Message | Cause | Fix |
|---|---|---|---|
| 17 | `port is already allocated` | Another container or host process occupies the port | Find the process: `lsof -i :PORT` or `ss -tlnp \| grep PORT`. Stop it or use a different host port |
| 18 | `Bind for 0.0.0.0:PORT failed: port is already allocated` | Same as above, explicit bind address | Same fix. Also check other Compose services for duplicate host port mappings |
| 19 | `driver failed programming external connectivity on endpoint` | iptables or firewall conflict | Restart Docker: `sudo systemctl restart docker`. Check firewall rules. Ensure `net.ipv4.ip_forward=1` |
| 20 | `network X declared as external, but could not be found` | External network does not exist | Create it: `docker network create X`. Or remove `external: true` to let Compose manage it |
| 21 | Containers cannot reach each other by service name | Using default bridge network or misconfigured networks | Ensure services share a common user-defined network. NEVER rely on the default bridge for DNS resolution |
| 22 | `network_mode and networks cannot be combined` | Service has both `network_mode:` and `networks:` | Use one or the other. `network_mode: host` excludes custom networks |

## Volume & Storage Errors

| # | Error Message | Cause | Fix |
|---|---|---|---|
| 23 | `volume X declared as external, but could not be found` | External volume does not exist | Create it: `docker volume create X`. Or remove `external: true` |
| 24 | Permission denied on mounted volume | UID/GID mismatch between host and container user | Match UIDs: set `user: "1000:1000"` in service. Or `chown` the host directory. Or adjust Dockerfile USER |
| 25 | Volume data lost after `docker compose down` | Using anonymous volume instead of named volume | ALWAYS declare named volumes in the top-level `volumes:` section and reference by name |
| 26 | `Mounts denied: the path /host/path is not shared from the host` | Docker Desktop file sharing restriction (macOS/Windows) | Add the path to Docker Desktop Settings > Resources > File Sharing |
| 27 | Volume mount overwrites container files | Bind mount or empty named volume replacing container directory | Use named volumes (they auto-populate from container). Or use `docker cp` to seed bind mounts |

## Environment Variable Errors

| # | Error Message | Cause | Fix |
|---|---|---|---|
| 28 | `variable "X" is not set` | Variable used in interpolation but undefined | Define in `.env` file, shell environment, or use default: `${X:-default}` |
| 29 | `required variable "X" is missing a value` | Variable with `:?` syntax is empty or unset | Set the variable. `${X:?message}` errors when X is unset or empty |
| 30 | `invalid interpolation format for X` | Malformed `${}` expression | Check for unclosed braces, nested interpolation (not supported), or stray `$` signs |
| 31 | Wrong value resolved for variable | Precedence conflict between sources | Check precedence order: CLI `-e` > shell env > `environment:` > `env_file:` > Dockerfile ENV. Use `docker compose config` to verify |
| 32 | Literal `${VAR}` appears in container | Value single-quoted in `.env` file | Single-quoted values are literal. Use double quotes or no quotes for interpolation |
| 33 | Dollar sign causes parse error | Unescaped `$` in value | Escape with `$$`. Example: `command: echo "$$HOME"` |

## Build Errors

| # | Error Message | Cause | Fix |
|---|---|---|---|
| 34 | `build path /path does not exist` | Build context directory not found | Verify `context:` path is relative to the Compose file directory. Check for typos |
| 35 | `Cannot locate specified Dockerfile: Dockerfile` | Dockerfile missing in build context | Check `dockerfile:` is relative to `context:`, NOT relative to compose.yaml. Default is `Dockerfile` in context root |
| 36 | `failed to solve: dockerfile parse error` | Syntax error in referenced Dockerfile | Check Dockerfile for typos, missing backslashes, wrong instruction names |
| 37 | `COPY failed: file not found in build context` | File outside context or excluded by .dockerignore | Verify file exists within the build context. Check `.dockerignore` patterns |
| 38 | `error during connect: Get "https://...": dial tcp: lookup registry` | Network issue during image pull in build | Check internet connectivity. Check DNS. Verify registry URL |
| 39 | Build context upload is extremely slow | Context directory contains large files (node_modules, .git, data) | Add a `.dockerignore` with large directories. ALWAYS ignore `node_modules/`, `.git/`, and build artifacts |

## Dependency & Lifecycle Errors

| # | Error Message | Cause | Fix |
|---|---|---|---|
| 40 | `dependency failed to start for service "X"` | Dependent service crashed or failed healthcheck | Check dependent service logs: `docker compose logs <dep>`. Fix the underlying service error first |
| 41 | Service times out waiting for `service_healthy` | Healthcheck failing or intervals too short | Increase `start_period`, `interval`, and `retries` in healthcheck. Verify the health command works inside the container |
| 42 | `service_completed_successfully` never met | Init/migration service exits with non-zero code | Check service logs. Ensure the command exits with code 0 on success |
| 43 | `Found orphan containers for this project` | Services removed from Compose file but containers still exist | Run `docker compose down --remove-orphans`. Or set `COMPOSE_IGNORE_ORPHANS=true` to suppress |
| 44 | Service restarts in a loop | Application crash with `restart: always` | Check logs: `docker compose logs <service>`. Fix the application error. Use `restart: on-failure:5` to limit retries |

## Profile Errors

| # | Error Message | Cause | Fix |
|---|---|---|---|
| 45 | Profiled dependency not started | Service depends on a profiled service whose profile is not active | Activate required profile: `--profile X`. Or remove the profile from the dependency. Or remove the cross-profile dependency |
| 46 | `docker compose up` doesn't start profiled service | Profiles are opt-in, not default | Explicitly activate: `--profile name` or `COMPOSE_PROFILES=name`. Services with profiles are NEVER started by default |
| 47 | All services started when only profile wanted | Missing profile assignment on optional services | Add `profiles: [name]` to services that should be opt-in |

## Compose Config Validation Errors

| # | Error Message | Cause | Fix |
|---|---|---|---|
| 48 | `docker compose config` shows unexpected values | Variable precedence or override file merging | Use `docker compose config --environment` to trace variable sources. Check for `compose.override.yaml` auto-loading |
| 49 | Merge conflict between Compose files | Duplicate resource definitions in included files | Rename conflicting resources. Use `include:` with paired override files for customization |
| 50 | `no configuration file provided: not found` | Neither compose.yaml nor docker-compose.yml exists | Create `compose.yaml` (preferred name). Or specify: `docker compose -f custom.yaml up` |

## Diagnostic Commands Reference

| Command | Purpose |
|---|---|
| `docker compose config` | Validate and render resolved Compose file |
| `docker compose config -q` | Silent validation (exit code only) |
| `docker compose config --environment` | Show resolved interpolation variables |
| `docker compose ps` | List service container status |
| `docker compose ps -a` | List all containers including stopped |
| `docker compose logs <service>` | View service logs |
| `docker compose logs --tail 50 <service>` | View last 50 log lines |
| `docker compose events` | Stream real-time Compose events |
| `docker compose top` | Display running processes per service |
| `docker compose port <service> <port>` | Show public port mapping |
| `docker inspect <container>` | Full container inspection |
| `docker compose down --remove-orphans` | Clean stop with orphan removal |
| `docker compose up --force-recreate` | Recreate all containers from scratch |
| `docker compose up --build` | Rebuild images before starting |
