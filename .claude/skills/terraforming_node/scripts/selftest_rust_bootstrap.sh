#!/bin/bash
# Install-routing fixtures only; the separate real check compiles native Rust.
set -eu
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/rust-bootstrap-selftest.XXXXXX")"
trap 'rm -rf -- "$T"' EXIT
mkdir -p "$T/new checkout/.claude/skills/terraforming_node/scripts" "$T/bin" "$T/elsewhere"
cp "$SDIR/rust_bootstrap.sh" "$SDIR/rust_toolchain.pin" "$T/new checkout/.claude/skills/terraforming_node/scripts/"
HELPER="$T/new checkout/.claude/skills/terraforming_node/scripts/rust_bootstrap.sh"
export RUST_TEST_TRACE="$T/trace" RUST_TEST_COMMAND="$T/command" RUST_TEST_INIT="$T/installer"
: > "$RUST_TEST_TRACE"
cat > "$RUST_TEST_COMMAND" <<'SH'
#!/bin/bash
set -eu
name="${0##*/}"
printf '%s %s auto=%s cargo=%s rustup=%s\n' "$name" "$*" "${RUSTUP_AUTO_INSTALL:-unset}" "$CARGO_HOME" "$RUSTUP_HOME" >> "$RUST_TEST_TRACE"
case "$name" in
  rustup)
    if [ "$1" = which ]; then
      printf '%s/toolchains/%s/bin/%s\n' "$RUSTUP_HOME" "$RUSTUP_TOOLCHAIN" "${!#}"
    elif [ "$1 $2" = 'toolchain install' ]; then
      /bin/bash "$RUST_TEST_INIT" "$@"
    else exit 9; fi ;;
  rustc)
    if [ "$1" = --print ]; then printf '%s/toolchains/%s\n' "$RUSTUP_HOME" "$RUSTUP_TOOLCHAIN"
    else printf 'rustc %s (fixture)\n' "${RUSTUP_TOOLCHAIN%%-*}"; fi ;;
  cargo)
    if [ "$1" = build ]; then
      [ "${CARGO_NET_OFFLINE:-}" = true ]
      mkdir -p "$CARGO_TARGET_DIR/debug"
      printf '#!/bin/sh\nprintf "rust-bootstrap-probe-ok 385\\n"\n' > "$CARGO_TARGET_DIR/debug/rust-bootstrap-probe"
      chmod +x "$CARGO_TARGET_DIR/debug/rust-bootstrap-probe"
    elif [ "$1" = --version ]; then printf 'cargo fixture\n'
    elif [ "$1" = fmt ] || [ "$1" = clippy ]; then :
    else exit 9; fi ;;
  rustfmt|clippy-driver) : ;;
esac
SH
cat > "$RUST_TEST_INIT" <<'SH'
#!/bin/bash
set -eu
printf 'installer %s\n' "$*" >> "$RUST_TEST_TRACE"
[ "${RUST_TEST_INSTALL_FAIL:-0}" = 0 ] || exit 42
mkdir -p "$CARGO_HOME/bin" "$RUSTUP_HOME/toolchains/$RUSTUP_TOOLCHAIN/bin"
for name in rustup cargo rustc rustfmt clippy-driver cargo-fmt cargo-clippy; do
  cp "$RUST_TEST_COMMAND" "$CARGO_HOME/bin/$name"
  chmod +x "$CARGO_HOME/bin/$name"
  [ "$name" = rustup ] || cp "$CARGO_HOME/bin/$name" "$RUSTUP_HOME/toolchains/$RUSTUP_TOOLCHAIN/bin/$name"
done
SH
cat > "$T/bin/curl" <<'SH'
#!/bin/bash
set -eu
printf 'curl %s\n' "${!#}" >> "$RUST_TEST_TRACE"
[ "${RUST_TEST_CURL_FAIL:-0}" = 0 ] || exit 22
dest=''
while [ "$#" -gt 0 ]; do
  case "$1" in --output) dest="$2"; shift 2 ;; *) url="$1"; shift ;; esac
done
case "$url" in
  *.sha256)
    if [ "${RUST_TEST_BAD_SHA:-0}" = 1 ]; then printf 'invalid-checksum\n' > "$dest"
    else sha256sum "$RUST_TEST_INIT" > "$dest"; fi ;;
  */rustup-init) cp "$RUST_TEST_INIT" "$dest" ;;
  *) exit 23 ;;
esac
SH
cat > "$T/bin/uname" <<'SH'
#!/bin/sh
case "$1" in
  -s) printf '%s\n' "${RUST_TEST_OS:-Linux}" ;;
  -m) printf '%s\n' "${RUST_TEST_ARCH:-aarch64}" ;;
  *) exit 2 ;;
esac
SH
chmod +x "$T/bin/"* "$RUST_TEST_INIT" "$RUST_TEST_COMMAND"
export PATH="$T/bin:$PATH"
pass=0
expect() {
  local expected="$1" label="$2" rc=0
  shift 2
  "$@" > "$T/stdout" 2> "$T/stderr" || rc=$?
  if [ "$rc" != "$expected" ]; then
    printf 'FAIL %s expected=%s actual=%s\n' "$label" "$expected" "$rc" >&2
    cat "$T/stdout" "$T/stderr" >&2
    exit 1
  fi
  pass=$((pass + 1)); printf 'PASS %s\n' "$label"
}
RT="$T/local runtime"
expect 3 'missing status' bash "$HELPER" status --runtime-home "$RT"
[ ! -e "$RT" ]
expect 3 'missing check does not install' bash "$HELPER" check --runtime-home "$RT"
expect 3 'missing exec does not install' bash "$HELPER" exec --runtime-home "$RT" -- rustc --version
[ ! -s "$RUST_TEST_TRACE" ]
expect 5 'download failure' env RUST_TEST_CURL_FAIL=1 bash "$HELPER" install --runtime-home "$T/network failure"
expect 5 'checksum failure prevents installer' env RUST_TEST_BAD_SHA=1 bash "$HELPER" install --runtime-home "$T/bad checksum"
! grep -q '^installer ' "$RUST_TEST_TRACE"
expect 6 'interrupted installation' env RUST_TEST_INSTALL_FAIL=1 bash "$HELPER" install --runtime-home "$T/install failure" --rustup-init-file "$RUST_TEST_INIT"
expect 0 'fresh official-download route, space path' bash "$HELPER" install --runtime-home "$RT"
grep -q -- '--no-modify-path' "$RUST_TEST_TRACE"
grep -q -- '--default-host aarch64-unknown-linux-gnu' "$RUST_TEST_TRACE"
expect 0 'ready state derives from installed tools' bash "$HELPER" status --runtime-home "$RT"
[ ! -e "$RT/.bootstrap-ready" ]
before="$(grep -c '^curl ' "$RUST_TEST_TRACE")"
expect 0 'idempotent install' bash "$HELPER" install --runtime-home "$RT"
[ "$(grep -c '^curl ' "$RUST_TEST_TRACE")" = "$before" ]
expect 0 'offline fixture build/run/fmt/clippy' bash "$HELPER" check --runtime-home "$RT"
[ "$(grep -c '^curl ' "$RUST_TEST_TRACE")" = "$before" ]
expect 0 'new cwd and inherited global selectors overridden' bash -c \
  'cd "$1"; CARGO_HOME=unused-global RUSTUP_HOME=unused-global RUSTUP_TOOLCHAIN=nightly /bin/bash "$2" exec --runtime-home "$3" -- rustc --version' \
  fixture "$T/elsewhere" "$HELPER" "$RT"
! grep -q 'cargo=unused-global\|rustup=unused-global' "$RUST_TEST_TRACE"
! grep -q 'auto=unset\|auto=1' "$RUST_TEST_TRACE"
pin="$(sed -n 's/^RUST_VERSION=//p' "$SDIR/rust_toolchain.pin")"
rm "$RT/rustup/toolchains/$pin-aarch64-unknown-linux-gnu/bin/rustfmt"
expect 3 'missing component is partial' bash "$HELPER" check --runtime-home "$RT"
expect 0 'explicit install repairs partial without installer download' bash "$HELPER" install --runtime-home "$RT"
[ "$(grep -c '^curl ' "$RUST_TEST_TRACE")" = "$before" ]
rm "$RT/cargo/bin/cargo-fmt"
expect 3 'missing Cargo subcommand proxy is partial' bash "$HELPER" status --runtime-home "$RT"
expect 0 'explicit installer repairs Cargo proxy' bash "$HELPER" install --runtime-home "$RT" --rustup-init-file "$RUST_TEST_INIT"
expect 0 'repaired Cargo proxy passes check' bash "$HELPER" check --runtime-home "$RT"
expect 5 'mirror failure does not print userinfo' env RUSTUP_UPDATE_ROOT=https://fixture-user:fixture-password@mirror.invalid RUST_TEST_CURL_FAIL=1 \
  bash "$HELPER" install --runtime-home "$T/private mirror"
! grep -q 'fixture-user\|fixture-password' "$T/stdout" "$T/stderr"
expect 3 'outside cwd retry command is absolute' bash -c 'cd "$1"; /bin/bash "$2" check --runtime-home "$3"' \
  fixture "$T/elsewhere" "$HELPER" "$T/not installed"
grep -Fq "$T/new" "$T/stderr"
mkdir -p "$T/no-cc"
for utility in sed head tr env uname grep awk realpath dirname; do ln -s "$(command -v "$utility")" "$T/no-cc/$utility"; done
expect 4 'missing linker is explicit' env PATH="$T/no-cc" /bin/bash "$HELPER" install --runtime-home "$T/no linker"
expect 0 'other supported architecture route' env RUST_TEST_ARCH=x86_64 bash "$HELPER" install --runtime-home "$T/x86" --rustup-init-file "$RUST_TEST_INIT"
expect 4 'unsupported architecture' env RUST_TEST_ARCH=riscv64 bash "$HELPER" install --runtime-home "$T/unsupported"
[ ! -e "$T/unsupported" ]
expect 4 'unsupported OS' env RUST_TEST_OS=Darwin bash "$HELPER" check --runtime-home "$RT"
expect 0 'new checkout derives its own runtime root' bash -c 'cd "$1"; /bin/bash "$2" install --rustup-init-file "$3"' \
  fixture "$T/elsewhere" "$HELPER" "$RUST_TEST_INIT"
[ -d "$T/new checkout/.claude/runtime/rust/cargo" ]
printf 'PASS rust bootstrap contract fixtures (%s commands; no Rust installation or network)\n' "$pass"
