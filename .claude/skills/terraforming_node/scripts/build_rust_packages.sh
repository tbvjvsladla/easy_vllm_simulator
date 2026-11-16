#!/bin/bash
# Explicit node-local Q/T/P/U source build. No install, fetch fallback or evaluator hook.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd)"
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
      echo 'After explicit Terraform install/check: build delivered Q/T/P/U normal release targets and smoke native CLIs.'
      exit 0 ;;
    *) printf 'Unknown argument: %s\n' "$1" >&2; exit 2 ;;
  esac
done
q="$REPO/.claude/rust/runtime-support"
t="$REPO/.claude/rust/role-contract"
p="$REPO/.claude/skills/vllm-recipe-explorer/rust/runtime"
u="$REPO/.claude/skills/upstream-version-watch/rust/runtime"
x="$REPO/.claude/rust/parity"
# terraforming_node programs ported to Rust (R): built first, its `rust_package_check` verifies every crate after its build.
r="$REPO/.claude/skills/terraforming_node/rust"
# py 경계 가드(plan_26111615 D5 · bin 전용).
bg="$REPO/.claude/rust/py-boundary-guard"
# Policies bundle (.claude/policies/**): completion gate+doc naming, evidence publisher, registry audit, runtime selftest, distribution check, claim predicates.
g="$REPO/.claude/rust/policy-gate"
pp="$REPO/.claude/rust/policy-publisher"
pa="$REPO/.claude/rust/policy-audit"
ps="$REPO/.claude/rust/policy-selftest"
pd="$REPO/.claude/rust/policy-distribution"
pf="$REPO/.claude/rust/policy-predicates"
# hint-publisher Rust port (plan_26111615 · Q path dependency) — built where the skill is delivered.
h="$REPO/.claude/skills/hint-publisher/rust"
# Every role consumer must carry its library and Q path dependency.
for consumer in "$REPO/.claude/skills/adversarial-benchmark/rust" "$u"; do
  if [ -f "$consumer/Cargo.toml" ]; then
    [ -f "$t/Cargo.toml" ] && [ -f "$q/Cargo.toml" ] || {
      echo "Delivered role-contract/Q path dependency missing: $consumer" >&2; exit 2;
    }
  fi
done
for skill in vllm-recipe-explorer upstream-version-watch; do
  if [ -d "$REPO/.claude/skills/$skill" ]; then
    [ -f "$REPO/.claude/skills/$skill/rust/runtime/Cargo.toml" ] && [ -f "$q/Cargo.toml" ] || {
      echo "Delivered Rust package/path dependency missing for $skill" >&2; exit 2;
    }
  fi
done
# mktemp (임시 metadata 디렉터리)는 이 helper 가 실제로 쓰는 호스트 도구다 — 없으면 원인을 말하고 멈춘다(Python 은 쓰지 않는다).
for tool in mktemp; do
  command -v "$tool" >/dev/null 2>&1 || { printf '[build_rust_packages] ERROR 필요한 도구가 PATH 에 없다: %s (임시 디렉터리에 사용)\n' "$tool" >&2; exit 4; }
done
tmp="$(mktemp -d "${TMPDIR:-/tmp}/rust-packages.XXXXXX")"
trap 'rm -f -- "$tmp/metadata.json"; rmdir -- "$tmp"' EXIT
applicable=0
for crate in "$r" "$q" "$t" "$p" "$u" "$x" "$g" "$pp" "$pa" "$ps" "$pd" "$pf" "$bg" "$h"; do
  [ -f "$crate/Cargo.toml" ] || continue
  applicable=$((applicable+1))
  [ -f "$crate/Cargo.lock" ] || { echo "Missing lock: $crate" >&2; exit 2; }
  # Clear only the child source override; source must resolve within this delivered node.
  env -u EASY_VLLM_SOURCE_ROOT bash "$BOOTSTRAP" exec "${runtime_args[@]}" -- cargo metadata \
    --no-deps --format-version 1 --locked --manifest-path "$crate/Cargo.toml" \
    "${cargo_args[@]}" > "$tmp/metadata.json"
  target_args=(--lib --bins)
  if [ "$crate" = "$q" ] || [ "$crate" = "$t" ]; then target_args=(--lib); fi
  # bin 전용 패키지(parity)는 lib 대상이 없다 — `--lib` 를 요구하면 "no library targets" 로 101.
  if [ "$crate" = "$x" ] || [ "$crate" = "$bg" ]; then target_args=(--bins); fi
  env -u EASY_VLLM_SOURCE_ROOT bash "$BOOTSTRAP" exec "${runtime_args[@]}" -- cargo build \
    --release --locked "${target_args[@]}" --manifest-path "$crate/Cargo.toml" \
    --target-dir "$crate/target" "${cargo_args[@]}"
  # Post-build check (Rust: terraforming_node/rust `rust_package_check`, built first in this loop): unique root package,
  # every normal binary an executable ELF, every library its .rlib; prints the package summary JSON line.
  [ -x "$r/target/release/rust_package_check" ] || {
    echo "Post-build checker missing (terraforming_node/rust not delivered or not built): $r/target/release/rust_package_check" >&2; exit 2; }
  "$r/target/release/rust_package_check" "$tmp/metadata.json" "$crate"
done
if [ "$applicable" -eq 0 ]; then
  echo '[build_rust_packages] NOT_APPLICABLE: no Q/T/P/U packages delivered to this plane'
  exit 0
fi
if [ -f "$p/Cargo.toml" ]; then
  env -i PATH= LC_ALL=C.UTF-8 COLUMNS=80 "$p/target/release/quant_table" --dtype float16 --json
fi
if [ -f "$h/Cargo.toml" ]; then
  env -i PATH= LC_ALL=C.UTF-8 COLUMNS=80 "$h/target/release/hint" --help >/dev/null
fi
if [ -f "$u/Cargo.toml" ]; then
  env -i PATH= LC_ALL=C.UTF-8 COLUMNS=80 "$u/target/release/node_role_contract" --help
  env -i PATH= LC_ALL=C.UTF-8 COLUMNS=80 "$u/target/release/validate_runtime_patch" -h
  set +e
  env -i PATH= LC_ALL=C.UTF-8 COLUMNS=80 "$u/target/release/validate_runtime_patch"
  status=$?
  set -e
  [ "$status" -eq 2 ] || { echo "validate_runtime_patch missing-args smoke: expected2, actual$status" >&2; exit 1; }
fi
if [ -f "$r/Cargo.toml" ]; then
  # terraforming_node ports: deterministic self-tests (no network, no Python; temporary files only under TMPDIR).
  for prog in audit_session_ssh agent_card_contract library_access logs_lifecycle adversarial_stress seed_from_journal; do
    env -i PATH=/usr/bin:/bin LC_ALL=C.UTF-8 TMPDIR="${TMPDIR:-/tmp}" "$r/target/release/$prog" --self-test >/dev/null || {
      echo "$prog --self-test smoke failed" >&2; exit 1; }
  done
fi
echo '[build_rust_packages] PASS: delivered normal Q/T/P/U build and applicable native startup smokes'
