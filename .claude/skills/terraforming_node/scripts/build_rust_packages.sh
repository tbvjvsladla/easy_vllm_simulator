#!/bin/bash
# Explicit node-local Q/P/U source build. No install, fetch fallback or evaluator hook.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SDIR/../../../.." && pwd)"
BOOTSTRAP="$SDIR/rust_bootstrap.sh"
runtime_args=()
cargo_args=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --offline) cargo_args+=(--offline); shift ;;
    --runtime-home)
      [ "$#" -ge 2 ] && [ -n "$2" ] || { echo '--runtime-home requires DIR' >&2; exit 2; }
      runtime_args+=(--runtime-home "$2"); shift 2 ;;
    --help|-h)
      echo 'Usage: build_rust_packages.sh [--offline] [--runtime-home DIR]'
      echo 'After explicit Terraform install/check: build delivered Q/P/U normal release targets and smoke native CLIs.'
      exit 0 ;;
    *) printf 'Unknown argument: %s\n' "$1" >&2; exit 2 ;;
  esac
done
q="$REPO/.claude/rust/runtime-support"
p="$REPO/.claude/skills/vllm-recipe-explorer/rust/runtime"
u="$REPO/.claude/skills/upstream-version-watch/rust/runtime"
for skill in vllm-recipe-explorer upstream-version-watch; do
  if [ -d "$REPO/.claude/skills/$skill" ]; then
    [ -f "$REPO/.claude/skills/$skill/rust/runtime/Cargo.toml" ] && [ -f "$q/Cargo.toml" ] || {
      echo "Delivered Rust package/path dependency missing for $skill" >&2; exit 2;
    }
  fi
done
tmp="$(mktemp -d "${TMPDIR:-/tmp}/rust-packages.XXXXXX")"
trap 'rm -f -- "$tmp/metadata.json"; rmdir -- "$tmp"' EXIT
applicable=0
for crate in "$q" "$p" "$u"; do
  [ -f "$crate/Cargo.toml" ] || continue
  applicable=$((applicable+1))
  [ -f "$crate/Cargo.lock" ] || { echo "Missing lock: $crate" >&2; exit 2; }
  # Clear only the child source override; source must resolve within this delivered node.
  env -u EASY_VLLM_SOURCE_ROOT bash "$BOOTSTRAP" exec "${runtime_args[@]}" -- cargo metadata \
    --no-deps --format-version 1 --locked --manifest-path "$crate/Cargo.toml" \
    "${cargo_args[@]}" > "$tmp/metadata.json"
  target_args=(--lib --bins)
  [ "$crate" != "$q" ] || target_args=(--lib)
  env -u EASY_VLLM_SOURCE_ROOT bash "$BOOTSTRAP" exec "${runtime_args[@]}" -- cargo build \
    --release --locked "${target_args[@]}" --manifest-path "$crate/Cargo.toml" \
    --target-dir "$crate/target" "${cargo_args[@]}"
  python3 - "$tmp/metadata.json" "$crate" <<'PY'
from pathlib import Path
import json, os, sys
crate=Path(sys.argv[2]).resolve()
data=json.loads(Path(sys.argv[1]).read_text())
packages=[p for p in data['packages']if Path(p['manifest_path']).resolve()==crate/'Cargo.toml']
if len(packages)!=1:raise SystemExit('metadata root package is ambiguous')
package=packages[0]
bins=sorted(t['name']for t in package['targets']if 'bin'in t['kind']and not t.get('required-features'))
for name in bins:
    binary=crate/'target/release'/name
    if not binary.is_file()or not os.access(binary,os.X_OK):raise SystemExit('normal binary missing/non-executable: '+str(binary))
    with binary.open('rb')as stream:
        if stream.read(4)!=b'\x7fELF':raise SystemExit('normal binary is not ELF: '+str(binary))
libraries=[t['name']for t in package['targets']if 'lib'in t['kind']]
for name in libraries:
    if not(crate/'target/release'/('lib'+name+'.rlib')).is_file():raise SystemExit('library missing: '+name)
print(json.dumps(dict(package=package['name'],normal_bins=bins,normal_bin_count=len(bins),libraries=libraries)))
PY
done
if [ "$applicable" -eq 0 ]; then
  echo '[build_rust_packages] NOT_APPLICABLE: no Q/P/U packages delivered to this plane'
  exit 0
fi
if [ -f "$p/Cargo.toml" ]; then
  env -i PATH= LC_ALL=C.UTF-8 COLUMNS=80 "$p/target/release/quant_table" --dtype float16 --json
fi
if [ -f "$u/Cargo.toml" ]; then
  env -i PATH= LC_ALL=C.UTF-8 COLUMNS=80 "$u/target/release/validate_runtime_patch" -h
  set +e
  env -i PATH= LC_ALL=C.UTF-8 COLUMNS=80 "$u/target/release/validate_runtime_patch"
  status=$?
  set -e
  [ "$status" -eq 2 ] || { echo "validate_runtime_patch missing-args smoke: expected2, actual$status" >&2; exit 1; }
fi
echo '[build_rust_packages] PASS: delivered normal Q/P/U build and applicable native startup smokes'
