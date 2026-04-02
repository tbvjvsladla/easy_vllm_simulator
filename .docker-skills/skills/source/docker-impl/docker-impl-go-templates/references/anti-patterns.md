# Go Template Anti-Patterns

> Common mistakes when using Docker `--format` flags. Each anti-pattern includes the error,
> WHY it fails, and the CORRECT alternative. Verified against Docker Engine 24+.

---

## AP-01: Missing {{ end }} Closure

```bash
# WRONG -- missing {{ end }} causes parse error
docker inspect --format='{{if .State.Running}}UP' myapp
# Error: template: :1: unexpected EOF

# CORRECT -- ALWAYS close if/range blocks
docker inspect --format='{{if .State.Running}}UP{{end}}' myapp
```

**WHY**: Go templates require explicit block closure. Every `{{ if }}` and `{{ range }}` MUST have a matching `{{ end }}`.

---

## AP-02: Quoting Conflict on Linux/macOS

```bash
# WRONG -- double quotes conflict with Go template syntax
docker inspect --format="{{.State.Status}}" myapp
# May work on some shells but breaks with nested quotes

# CORRECT -- ALWAYS use single quotes on Linux/macOS
docker inspect --format='{{.State.Status}}' myapp
```

**WHY**: The shell interprets double-quoted strings before Docker sees them. Backticks, dollar signs, and braces inside double quotes can trigger shell expansion. Single quotes pass the template verbatim.

---

## AP-03: Accessing Nil/Missing Fields Without Guard

```bash
# WRONG -- crashes if no healthcheck is configured
docker inspect --format='{{.State.Health.Status}}' myapp
# Error: template: :1:... executing "..." at <.State.Health.Status>: nil pointer

# CORRECT -- guard with if
docker inspect --format='{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' myapp
```

**WHY**: Not all containers have a healthcheck configured. `.State.Health` is nil when no HEALTHCHECK instruction exists. Accessing a field on a nil pointer causes a template execution error.

---

## AP-04: Using Dot Notation for Labels with Dots

```bash
# WRONG -- Go interprets dots as field separators
docker inspect --format='{{.Config.Labels.com.docker.compose.project}}' myapp
# Error: template: :1:... can't evaluate field com in type ...

# CORRECT -- use index function for dotted key names
docker inspect --format='{{index .Config.Labels "com.docker.compose.project"}}' myapp
```

**WHY**: Go template dot notation (`.field.subfield`) treats each segment as a struct field. Label keys containing dots are map keys, not nested structs. ALWAYS use `index` to access map keys that contain dots.

---

## AP-05: Forgetting table Keyword for Headers

```bash
# WRONG -- no headers, hard to read
docker ps --format "{{.Names}}\t{{.Status}}\t{{.Ports}}"
# Output:
# web    Up 2 hours    80/tcp
# db     Up 2 hours    5432/tcp

# CORRECT -- table prefix adds column headers
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
# Output:
# NAMES  STATUS        PORTS
# web    Up 2 hours    80/tcp
# db     Up 2 hours    5432/tcp
```

**WHY**: The `table` keyword is a Docker-specific extension (not standard Go templates). Without it, you get raw values only. ALWAYS use `table` when output is for human reading; omit it only for machine parsing.

---

## AP-06: Using \t Without table for Alignment

```bash
# WRONG -- \t without table produces inconsistent alignment
docker ps --format "{{.Names}}\t{{.Image}}\t{{.Status}}"
# Output may not align because tab stops depend on terminal

# CORRECT for humans -- use table keyword
docker ps --format "table {{.Names}}\t{{.Image}}\t{{.Status}}"

# CORRECT for scripting -- use a fixed separator
docker ps --format "{{.Names}}|{{.Image}}|{{.Status}}"
```

**WHY**: Tab characters without the `table` directive rely on terminal tab stop settings. Different terminals and pipes handle tabs differently. For reliable alignment, ALWAYS use `table`. For machine parsing, use a deterministic separator like `|` or use `--format json`.

---

## AP-07: Range Without Variable Assignment on Maps

```bash
# WRONG -- cannot access both key and value
docker inspect --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' myapp
# Works but you lose the network name

# CORRECT -- assign key and value variables
docker inspect --format='{{range $name, $conf := .NetworkSettings.Networks}}{{$name}}: {{$conf.IPAddress}}{{println}}{{end}}' myapp
```

**WHY**: When iterating over maps, the simple `{{ range }}` form only gives you the value. To access both key and value, ALWAYS use the `$key, $value := .Map` assignment form.

---

## AP-08: Assuming Port Bindings Always Exist

```bash
# WRONG -- crashes when port has no host binding
docker inspect --format='{{(index (index .NetworkSettings.Ports "80/tcp") 0).HostPort}}' myapp
# Error: index out of range when port is exposed but not published

# CORRECT -- guard against nil binding list
docker inspect --format='{{with index .NetworkSettings.Ports "80/tcp"}}{{(index . 0).HostPort}}{{else}}not published{{end}}' myapp

# ALSO CORRECT -- use if
docker inspect --format='{{if index .NetworkSettings.Ports "80/tcp"}}{{(index (index .NetworkSettings.Ports "80/tcp") 0).HostPort}}{{else}}not published{{end}}' myapp
```

**WHY**: A port can be EXPOSE'd in the Dockerfile without being published (`-p`). The binding list is nil or empty for unexposed ports. ALWAYS check before indexing into port binding arrays.

---

## AP-09: Using json on Entire Inspect Without jq

```bash
# WRONG -- unreadable wall of JSON
docker inspect --format='{{json .}}' myapp
# Outputs thousands of characters on one line

# CORRECT -- pipe to jq for readability
docker inspect --format='{{json .}}' myapp | jq .

# BETTER -- target specific sections
docker inspect --format='{{json .State}}' myapp | jq .
docker inspect --format='{{json .NetworkSettings}}' myapp | jq .
```

**WHY**: `{{ json . }}` dumps the entire object as a single unformatted line. ALWAYS pipe to `jq` for human-readable output. Better yet, target a specific sub-object to reduce noise.

---

## AP-10: Mixing Go Template and jq in --format

```bash
# WRONG -- jq syntax inside Go template
docker inspect --format='{{.Config.Labels | keys}}' myapp
# Error: function "keys" not defined

# CORRECT -- use json then pipe to jq
docker inspect --format='{{json .Config.Labels}}' myapp | jq 'keys'
```

**WHY**: Go templates and jq are separate languages. Go templates have a limited set of built-in functions (`json`, `join`, `split`, `upper`, `lower`, `title`, `println`, `index`, `len`). For complex JSON transformations, ALWAYS output JSON from Docker and process with jq.

---

## AP-11: Forgetting println in Range Loops

```bash
# WRONG -- all values concatenated on one line
docker inspect --format='{{range .Config.Env}}{{.}}{{end}}' myapp
# Output: PATH=/usr/binHOME=/rootTERM=xterm

# CORRECT -- use println for line breaks
docker inspect --format='{{range .Config.Env}}{{println .}}{{end}}' myapp
# Output:
# PATH=/usr/bin
# HOME=/root
# TERM=xterm
```

**WHY**: Go templates do not insert newlines between range iterations. ALWAYS use `{{ println . }}` or `{{ println }}` at the end of range blocks to separate items.

---

## AP-12: Using --format with docker inspect on Wrong Object Type

```bash
# WRONG -- using container fields on an image
docker inspect --format='{{.State.Status}}' nginx:latest
# Error: template: :1:... can't evaluate field State in type...

# CORRECT -- use image-specific fields
docker inspect --format='{{.Config.Cmd}}' nginx:latest
docker inspect --format='{{json .Config.ExposedPorts}}' nginx:latest

# CORRECT -- restrict type explicitly
docker inspect --type image --format='{{.RepoTags}}' nginx:latest
docker inspect --type container --format='{{.State.Status}}' myapp
```

**WHY**: `docker inspect` auto-detects object type, but container-specific fields (`.State`, `.NetworkSettings`, `.HostConfig`) do not exist on images. ALWAYS use `--type` when the object type is ambiguous, or know which fields belong to which object type.

---

## AP-13: Hardcoding Container Names in Scripts

```bash
# WRONG -- breaks when container name changes
IP=$(docker inspect --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' my-hardcoded-name)

# CORRECT -- use variables or docker ps filters
CONTAINER=$(docker ps -qf "label=app=web" | head -1)
IP=$(docker inspect --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$CONTAINER")

# ALSO CORRECT -- use Compose service names
IP=$(docker inspect --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$(docker compose ps -q web)")
```

**WHY**: Container names may change between deployments. ALWAYS use labels, filters, or Compose service lookups to find containers dynamically in scripts.

---

## AP-14: Not Using --no-stream with docker stats in Scripts

```bash
# WRONG -- hangs forever in script (stats is a live stream)
CPU=$(docker stats --format "{{.CPUPerc}}" myapp)

# CORRECT -- use --no-stream for single snapshot
CPU=$(docker stats --no-stream --format "{{.CPUPerc}}" myapp)
```

**WHY**: `docker stats` continuously streams live data by default. In a script, this blocks indefinitely. ALWAYS use `--no-stream` when capturing stats output in variables or pipelines.

---

## AP-15: Ignoring Platform Differences in Quoting

```bash
# Linux/macOS -- use single quotes
docker inspect --format='{{.State.Status}}' myapp

# Windows CMD -- use double quotes
docker inspect --format="{{.State.Status}}" myapp

# Windows PowerShell -- single quotes usually work
docker inspect --format='{{.State.Status}}' myapp

# WRONG -- escaping that works on one platform but not another
docker inspect --format=\"{{.State.Status}}\" myapp
```

**WHY**: Shell quoting rules differ across platforms. Single quotes on Linux/macOS prevent all shell interpolation. Windows CMD requires double quotes. PowerShell has its own rules. ALWAYS test format strings on your target platform.

---

## Summary Table

| Anti-Pattern | Risk | Fix |
|-------------|------|-----|
| AP-01: Missing `{{ end }}` | Template parse error | ALWAYS close if/range blocks |
| AP-02: Double-quote wrapper | Shell expansion corrupts template | Use single quotes on Linux/macOS |
| AP-03: Nil field access | Template execution panic | Guard with `{{ if }}` |
| AP-04: Dots in label keys | Field resolution error | Use `index` function |
| AP-05: No `table` keyword | Missing headers | Add `table` prefix |
| AP-06: `\t` without `table` | Misaligned output | Use `table` or fixed separator |
| AP-07: Range without key var | Lost map keys | Use `$k, $v := .Map` form |
| AP-08: Unguarded port index | Index out of range panic | Check binding exists first |
| AP-09: Full JSON without jq | Unreadable output | Pipe to jq, target sub-objects |
| AP-10: jq syntax in template | Undefined function error | Separate Go template from jq |
| AP-11: No println in range | Concatenated output | Add `{{ println }}` |
| AP-12: Wrong object type | Field not found error | Use `--type` flag |
| AP-13: Hardcoded names | Brittle scripts | Use labels/filters |
| AP-14: No --no-stream | Script hangs | ALWAYS use --no-stream in scripts |
| AP-15: Platform quoting | Cross-platform failures | Test on target platform |
