# Image Scanning Reference

## Docker Scout

### Command Reference

| Command | Purpose |
|---------|---------|
| `docker scout quickview IMAGE` | Summary of vulnerability counts by severity |
| `docker scout cves IMAGE` | Detailed CVE listing with package info |
| `docker scout recommendations IMAGE` | Base image upgrade suggestions |
| `docker scout compare IMAGE --to IMAGE` | Diff vulnerabilities between two images |
| `docker scout sbom IMAGE` | View Software Bill of Materials |

### Filtering Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--only-severity` | Filter by severity level | `--only-severity critical,high` |
| `--only-fixed` | Only show fixable CVEs | `--only-fixed` |
| `--only-unfixed` | Only show unfixable CVEs | `--only-unfixed` |
| `--only-cve-id` | Target specific CVE IDs | `--only-cve-id CVE-2024-1234` |
| `--only-cisa-kev` | CISA Known Exploited Vulnerabilities | `--only-cisa-kev` |
| `--only-package-type` | Filter by package manager | `--only-package-type apk,npm` |
| `--only-package` | Regex match on package name | `--only-package "openssl.*"` |
| `--ignore-base` | Exclude base image CVEs | `--ignore-base` |
| `--only-base` | Only base image CVEs | `--only-base` |
| `--epss` | Include EPSS exploit probability scores | `--epss` |
| `--epss-score` | Filter by minimum EPSS score | `--epss-score 0.5` |
| `--epss-percentile` | Filter by EPSS percentile | `--epss-percentile 0.9` |
| `--ignore-suppressed` | Exclude Scout exceptions | `--ignore-suppressed` |

### Artifact URI Prefixes

| Prefix | Source |
|--------|--------|
| `image://` | Local image with registry fallback (default) |
| `local://` | Local image store only |
| `registry://` | Registry only |
| `oci-dir://` | OCI layout directory |
| `archive://` | Docker save tarball |
| `fs://` | Local directory or file |
| `sbom://` | SPDX/in-toto/syft JSON SBOM |

### Output Formats

| Flag | Format | Use Case |
|------|--------|----------|
| `--format sarif` | SARIF JSON | IDE integration, GitHub Code Scanning |
| `--format markdown` | Markdown table | Documentation, PR comments |
| `--format spdx` | SPDX JSON | SBOM exchange |
| `--format json` | Raw JSON | Custom tooling |

```bash
# Generate SARIF for GitHub Code Scanning
docker scout cves --format sarif -o scout-report.sarif myapp:v1

# Generate markdown for PR comments
docker scout cves --format markdown -o report.md myapp:v1
```

### CI/CD Integration Pattern

```bash
#!/bin/bash
# CI vulnerability gate script
set -e

IMAGE="${1:?Usage: scan.sh IMAGE:TAG}"

echo "=== Scanning $IMAGE for vulnerabilities ==="

# Quick overview
docker scout quickview "$IMAGE"

# Fail pipeline on critical/high fixable vulnerabilities
docker scout cves -e --only-severity critical,high --only-fixed "$IMAGE"
EXIT_CODE=$?

if [ $EXIT_CODE -eq 2 ]; then
  echo "FAIL: Fixable critical/high vulnerabilities found"
  docker scout recommendations "$IMAGE"
  exit 1
fi

echo "PASS: No fixable critical/high vulnerabilities"
```

### GitHub Actions Integration

```yaml
name: Docker Scout Scan
on:
  push:
    branches: [main]
  pull_request:

jobs:
  scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: docker/login-action@v3
        with:
          username: ${{ secrets.DOCKERHUB_USERNAME }}
          password: ${{ secrets.DOCKERHUB_TOKEN }}
      - uses: docker/build-push-action@v5
        with:
          context: .
          load: true
          tags: myapp:scan
      - uses: docker/scout-action@v1
        with:
          command: cves
          image: myapp:scan
          only-severities: critical,high
          only-fixed: true
          exit-code: true
```

---

## Trivy

### Command Reference

| Command | Purpose |
|---------|---------|
| `trivy image IMAGE` | Scan container image |
| `trivy fs PATH` | Scan filesystem / source code |
| `trivy config PATH` | Scan IaC files (Dockerfile, Compose, K8s) |
| `trivy sbom IMAGE` | Generate SBOM |
| `trivy repo URL` | Scan Git repository |

### Common Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--severity` | Filter by severity | `--severity CRITICAL,HIGH` |
| `--exit-code` | Exit code when vulns found | `--exit-code 1` |
| `--ignore-unfixed` | Skip unfixable CVEs | `--ignore-unfixed` |
| `--format` | Output format | `--format json`, `--format sarif`, `--format table` |
| `-o` | Output file | `-o report.json` |
| `--timeout` | Scan timeout | `--timeout 10m` |
| `--skip-dirs` | Directories to skip | `--skip-dirs node_modules` |
| `--skip-files` | Files to skip | `--skip-files package-lock.json` |

### Trivy CI/CD Pattern

```bash
#!/bin/bash
set -e

IMAGE="${1:?Usage: trivy-scan.sh IMAGE:TAG}"

# Scan for critical vulnerabilities, fail if found
trivy image \
  --exit-code 1 \
  --severity CRITICAL \
  --ignore-unfixed \
  --format table \
  "$IMAGE"

echo "PASS: No critical fixable vulnerabilities"
```

### Trivy GitHub Actions

```yaml
- name: Trivy Scan
  uses: aquasecurity/trivy-action@master
  with:
    image-ref: myapp:v1
    format: sarif
    output: trivy-results.sarif
    severity: CRITICAL,HIGH
    exit-code: 1

- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: trivy-results.sarif
```

### Trivy Dockerfile/Compose Scanning

```bash
# Scan Dockerfile for misconfigurations
trivy config Dockerfile

# Scan docker-compose.yml
trivy config docker-compose.yml

# Scan entire project IaC
trivy config .
```

---

## Snyk Container

### Command Reference

| Command | Purpose |
|---------|---------|
| `snyk container test IMAGE` | One-time vulnerability scan |
| `snyk container monitor IMAGE` | Continuous monitoring |
| `snyk container test IMAGE --file=Dockerfile` | Scan with remediation advice |

### Snyk CI/CD Pattern

```bash
#!/bin/bash
set -e

IMAGE="${1:?Usage: snyk-scan.sh IMAGE:TAG}"

# Test with Dockerfile context for remediation
snyk container test "$IMAGE" \
  --file=Dockerfile \
  --severity-threshold=high

echo "PASS: No high/critical vulnerabilities"
```

---

## Scanning Strategy

### When to Use Each Scanner

| Scanner | Best For | Cost |
|---------|----------|------|
| Docker Scout | Docker Hub images, quick triage, base image recommendations | Free tier available |
| Trivy | CI/CD pipelines, IaC scanning, SBOM generation, air-gapped environments | Free / open source |
| Snyk | Enterprise workflows, continuous monitoring, developer-first remediation | Free tier + paid |

### Recommended Pipeline

1. **Build stage**: Trivy scans Dockerfile for misconfigurations (`trivy config`)
2. **Post-build**: Docker Scout or Trivy scans built image for CVEs
3. **Registry**: Continuous monitoring via Snyk or registry-native scanning
4. **Runtime**: Periodic re-scanning of deployed images for new CVEs

### SBOM Generation

ALWAYS generate an SBOM for production images:

```bash
# Via BuildKit (at build time)
docker buildx build --sbom=true --push -t myapp:v1 .

# Via Trivy (post-build)
trivy image --format spdx-json -o sbom.json myapp:v1

# Via Docker Scout
docker scout sbom --format spdx myapp:v1
```
