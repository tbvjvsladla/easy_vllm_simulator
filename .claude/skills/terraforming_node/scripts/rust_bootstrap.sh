#!/bin/bash
# rust_bootstrap.sh — 노드 로컬 Rust 도구체인 설치·검증·실행 (terraforming_node §2.5.2 · references/install/rust_runtime.md)
#
# 설치 라우팅은 bash/curl/coreutils로 시작하고, check는 cc/ar와 설치한 Rust를 쓴다. Cargo·Python 없이 설치할 수 있다.
# 버전의 단일 정본은 같은 디렉터리의 rust_toolchain.pin 이다(여기 숫자를 다시 적지 않는다).
#
# 명령(정본 = `--help`):
#   install [--runtime-home DIR] [--rustup-init-file FILE]   유일하게 네트워크를 쓰는 명령(공식 rustup HTTPS)
#   check   [--runtime-home DIR]                             버전 + Rust/C 임시 프로젝트 offline build·native 실행·fmt·clippy
#   status  [--runtime-home DIR]                             missing|partial|stale|foreign|ready 판정만(읽기 전용)
#   exec    [--runtime-home DIR] -- CMD [ARGS...]            pin 환경(CARGO_HOME·RUSTUP_HOME·RUSTUP_TOOLCHAIN·PATH)에서 CMD 실행
#
# 불변: HOME·셸 profile·전역 rustup/cargo 설정·인증을 건드리지 않는다(설치는 --no-modify-path). check/status/exec 는
#   설치·다운로드·전역 toolchain 폴백이 없다 — 미설치면 exit 3 으로 `install` 을 안내하고 멈춘다. sudo·패키지매니저 ✗.
set -u

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SDIR/../../../.." && pwd)"
PIN_FILE="$SDIR/rust_toolchain.pin"
TAG="[rust_bootstrap]"
printf -v SELF 'bash %q' "$SDIR/rust_bootstrap.sh"

# 종료코드(고정 계약): 0 성공 · 2 사용법/pin 결함 · 3 미설치·반쪽·stale·foreign(install 필요) ·
#   4 미지원 플랫폼 또는 선행조건(curl·cc·ar·sha256sum) 부재 · 5 다운로드/검증 실패 · 6 설치 실패 · 7 check probe 실패
die() { local rc="$1"; shift; printf '%s ERROR rc=%s: %s\n' "$TAG" "$rc" "$*" >&2; exit "$rc"; }
note() { printf '%s %s\n' "$TAG" "$*"; }

usage() {
  cat <<EOF
사용법: $SELF <install|check|status|exec> [옵션] [-- CMD ARGS...]

  install [--runtime-home DIR] [--rustup-init-file FILE]
      공식 rustup(HTTPS)으로 pin 버전·minimal 프로필·rustfmt·clippy 를 DIR 에 설치한다. 이미 ready 면 재다운로드하지 않는다.
      반쪽(partial)/stale 은 기존 rustup 으로 복구·갱신한다. 네트워크는 이 명령만 쓴다.
      --rustup-init-file FILE : 다운로드 대신 사전 준비한 rustup-init 실행파일을 쓴다(체크섬 검증 없음 · 폐쇄망/미러 준비용).
  check   [--runtime-home DIR]
      pin 버전·sysroot·fmt/clippy 및 실제 C 컴파일·정적 아카이브와 연결한 Rust의 offline build/native 실행을 확인한다.
  status  [--runtime-home DIR]
      상태(missing|partial|stale|foreign|ready)만 출력한다. 읽기 전용.
  exec    [--runtime-home DIR] -- CMD [ARGS...]
      CMD 를 pin 환경에서 새 프로세스로 실행한다(cwd·PATH·전역 설치와 무관). 미설치면 exit 3.

기본 runtime home = <repo>/.claude/runtime/rust (CARGO_HOME=<home>/cargo · RUSTUP_HOME=<home>/rustup). 경로에 공백이 있어도 된다.
pin: $PIN_FILE
종료코드: 0 성공 · 2 사용법 · 3 install 필요 · 4 미지원 플랫폼/선행조건 · 5 다운로드 · 6 설치 · 7 check probe
미러/프록시: RUSTUP_UPDATE_ROOT · RUSTUP_DIST_SERVER · HTTPS_PROXY 같은 기존 환경을 그대로 상속한다(값을 바꾸거나 출력하지 않는다).
EOF
}

# ── pin ───────────────────────────────────────────────────────────────────────
pin_get() { sed -n "s/^$1=//p" "$PIN_FILE" | head -n 1 | tr -d '[:space:]'; }

load_pin() {
  [ -f "$PIN_FILE" ] || die 2 "버전 정본이 없다: $PIN_FILE"
  PIN_VERSION="$(pin_get RUST_VERSION)"; PIN_PROFILE="$(pin_get RUST_PROFILE)"
  PIN_COMPONENTS="$(pin_get RUST_COMPONENTS)"; PIN_TRIPLES="$(pin_get RUST_SUPPORTED_TRIPLES)"
  [[ "$PIN_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die 2 "RUST_VERSION 은 정확한 stable 버전(x.y.z)이어야 한다: '${PIN_VERSION}' ($PIN_FILE)"
  [ -n "$PIN_PROFILE" ] && [ -n "$PIN_COMPONENTS" ] && [ -n "$PIN_TRIPLES" ] \
    || die 2 "pin 에 RUST_PROFILE/RUST_COMPONENTS/RUST_SUPPORTED_TRIPLES 가 비었다: $PIN_FILE"
}

# ── 플랫폼 ─────────────────────────────────────────────────────────────────────
detect_platform() {
  local os arch
  os="$(uname -s 2>/dev/null)"; arch="$(uname -m 2>/dev/null)"
  [ "$os" = "Linux" ] || die 4 "미지원 OS: $os/$arch — 지원은 Linux GNU(aarch64·x86_64)뿐이다. 다른 플랫폼은 사람이 공식 rustup 으로 pin ${PIN_VERSION} 을 설치해야 하며 이 helper 는 대신 설치하지 않는다(references/install/rust_runtime.md §지원 범위)."
  case "$arch" in
    aarch64|arm64) TRIPLE="aarch64-unknown-linux-gnu" ;;
    x86_64|amd64)  TRIPLE="x86_64-unknown-linux-gnu" ;;
    *) die 4 "미지원 CPU 아키텍처: $os/$arch — 지원은 aarch64·x86_64 GNU 뿐이다(references/install/rust_runtime.md §지원 범위)." ;;
  esac
  case ",$PIN_TRIPLES," in *",$TRIPLE,"*) ;; *) die 4 "pin 의 지원 목록($PIN_TRIPLES)에 $TRIPLE 이 없다." ;; esac
  if command -v ldd >/dev/null 2>&1 && ldd --version 2>&1 | grep -qi musl; then
    die 4 "musl libc 호스트다 — GNU(glibc) toolchain 만 지원한다. musl 은 별도 절차가 필요하며 이 helper 의 범위 밖이다."
  fi
}

require_native_tools() {
  command -v cc >/dev/null 2>&1 \
    || die 4 "C 컴파일러/링커(cc)가 PATH 에 없다 — rustc 가 링크에 쓴다. 이 helper 는 sudo·패키지매니저를 쓰지 않는다: 호스트 운영자가 build-essential(또는 gcc) 을 설치한 뒤 다시 실행하라."
  command -v ar >/dev/null 2>&1 \
    || die 4 "정적 아카이브 도구(ar)가 PATH 에 없다 — Rust 업무의 native 의존성 빌드에 필요하다. 호스트 운영자가 binutils(또는 build-essential) 를 준비한 뒤 다시 실행하라."
}

# ── 경로 · 환경 ────────────────────────────────────────────────────────────────
resolve_home() {
  RT_HOME="${OPT_HOME:-$REPO/.claude/runtime/rust}"
  case "$RT_HOME" in /*) ;; *) RT_HOME="$PWD/$RT_HOME" ;; esac
  command -v realpath >/dev/null 2>&1 || die 4 "realpath(coreutils)이 없다 — 설치 경로 해소에 필요하다."
  RT_HOME="$(realpath -m -- "$RT_HOME")" || die 4 "runtime home 경로를 해소할 수 없다."
  CARGO_HOME="$RT_HOME/cargo"; RUSTUP_HOME="$RT_HOME/rustup"
}

# 하위프로세스 환경을 **명시**한다. HOME 은 건드리지 않고, 전역 CARGO_HOME/RUSTUP_HOME/RUSTUP_TOOLCHAIN 은 덮어쓴다.
run_env() {
  env "CARGO_HOME=$CARGO_HOME" "RUSTUP_HOME=$RUSTUP_HOME" "RUSTUP_TOOLCHAIN=$PIN_VERSION-$TRIPLE" \
      "RUSTUP_AUTO_INSTALL=0" \
      "PATH=$CARGO_HOME/bin:$PATH" "$@"
}

proxies_ok() {
  local t
  for t in rustup cargo rustc cargo-fmt cargo-clippy; do
    [ -x "$CARGO_HOME/bin/$t" ] || { STATE_WHY="$CARGO_HOME/bin/$t 가 없다"; return 1; }
  done
}

structure_ok() {   # $1 = 버전. 설치기 프록시와 toolchain 구조를 실제로 확인한다.
  proxies_ok || return 1
  [ -d "$RUSTUP_HOME/toolchains/$1-$TRIPLE" ] || { STATE_WHY="toolchain 디렉터리가 없다: $RUSTUP_HOME/toolchains/$1-$TRIPLE"; return 1; }
  return 0
}

compute_state() {
  STATE=""; STATE_WHY=""
  local tool selected version
  if [ ! -x "$CARGO_HOME/bin/rustup" ]; then
    if [ ! -e "$CARGO_HOME/bin/rustup" ] && ! compgen -G "$RUSTUP_HOME/toolchains/*" >/dev/null 2>&1; then
      STATE=missing; STATE_WHY="노드 로컬 rustup과 toolchain 이 없다"
    else
      STATE=partial; STATE_WHY="실행 가능한 노드 로컬 rustup이 없다"
    fi
    return
  fi
  if [ ! -d "$RUSTUP_HOME/toolchains/$PIN_VERSION-$TRIPLE" ]; then
    STATE=stale; STATE_WHY="요청한 pin/host toolchain 이 없다: $PIN_VERSION-$TRIPLE"; return
  fi
  if ! structure_ok "$PIN_VERSION"; then STATE=partial; return; fi
  for tool in rustc cargo rustfmt clippy-driver; do
    selected="$(run_env "$CARGO_HOME/bin/rustup" which --toolchain "$PIN_VERSION-$TRIPLE" "$tool" 2>/dev/null)" || {
      STATE=partial; STATE_WHY="pin toolchain에서 $tool 을 찾지 못했다"; return;
    }
    case "$selected" in "$RUSTUP_HOME"/toolchains/*) ;; *)
      STATE=foreign; STATE_WHY="노드 로컬 toolchain 밖의 $tool 이 선택됐다"; return ;;
    esac
    [ -x "$selected" ] || { STATE=partial; STATE_WHY="$tool 실행파일이 없다"; return; }
  done
  version="$(run_env "$CARGO_HOME/bin/rustc" --version 2>/dev/null)" || {
    STATE=partial; STATE_WHY="설치된 rustc가 실행되지 않는다"; return;
  }
  [ "$(printf '%s' "$version" | awk '{print $2}')" = "$PIN_VERSION" ] || {
    STATE=stale; STATE_WHY="rustc 실행 버전이 pin과 다르다: $version"; return;
  }
  STATE=ready; STATE_WHY="설치된 실행파일에서 확인한 pin $PIN_VERSION · $TRIPLE"
}

need_ready() {   # check/exec 공통 — 암묵 설치 ✗
  compute_state
  case "$STATE" in
    ready) return 0 ;;
    *) local retry
       printf -v retry '%s install --runtime-home %q' "$SELF" "$RT_HOME"
       die 3 "Rust runtime 이 ready 가 아니다(state=$STATE · $STATE_WHY · home=$RT_HOME). 설치/복구는 명시 명령으로만 한다: $retry" ;;
  esac
}

# ── install ───────────────────────────────────────────────────────────────────
fetch() {   # $1 = URL · $2 = 저장 경로. 인증·프록시는 환경에서 상속하고 값은 출력하지 않는다.
  local rc
  curl --fail --silent --location --proto '=https' --tlsv1.2 --retry 2 --connect-timeout 20 --max-time 300 \
       --output "$2" "$1"
  rc=$?
  [ "$rc" -eq 0 ] || die 5 "다운로드 실패(curl rc=$rc) — 네트워크·DNS·프록시·HTTPS 미러를 확인하라. URL/자격증명 값은 출력하지 않는다."
}

obtain_rustup_init() {   # → RUSTUP_INIT (실행파일 경로) · DL_DIR (정리 대상, 있을 때)
  DL_DIR=""
  if [ -n "${OPT_INIT_FILE:-}" ]; then
    case "$OPT_INIT_FILE" in /*) ;; *) OPT_INIT_FILE="$PWD/$OPT_INIT_FILE" ;; esac
    [ -f "$OPT_INIT_FILE" ] && [ -x "$OPT_INIT_FILE" ] || die 2 "--rustup-init-file 은 실행 가능한 파일이어야 한다: $OPT_INIT_FILE"
    RUSTUP_INIT="$OPT_INIT_FILE"; note "사전 준비된 rustup-init 사용(다운로드·체크섬 검증 없음): $RUSTUP_INIT"; return
  fi
  command -v curl >/dev/null 2>&1 || die 4 "curl 이 없다 — 공식 rustup 다운로드에 필요하다. 호스트 운영자가 curl 을 설치하거나 --rustup-init-file 로 사전 준비한 rustup-init 을 주라."
  command -v sha256sum >/dev/null 2>&1 || die 4 "sha256sum 이 없다 — rustup-init 무결성 확인에 필요하다."
  local root="${RUSTUP_UPDATE_ROOT:-https://static.rust-lang.org/rustup}" url want got
  case "$root" in https://*) ;; *) die 2 "RUSTUP_UPDATE_ROOT 는 https:// 여야 한다." ;; esac
  url="$root/dist/$TRIPLE/rustup-init"
  DL_DIR="$(mktemp -d "$RT_HOME/.dl.XXXXXX")" || die 6 "임시 다운로드 디렉터리를 만들 수 없다: $RT_HOME"
  note "rustup-init 다운로드(HTTPS · target=$TRIPLE)"
  fetch "$url" "$DL_DIR/rustup-init"
  fetch "$url.sha256" "$DL_DIR/rustup-init.sha256"
  want="$(awk '{print $1; exit}' "$DL_DIR/rustup-init.sha256")"
  got="$(sha256sum "$DL_DIR/rustup-init" | awk '{print $1}')"
  [ -n "$want" ] && [ "$want" = "$got" ] || die 5 "rustup-init sha256 불일치(공식 .sha256 와 다르다) — 실행하지 않는다: expected=$want actual=$got"
  chmod +x "$DL_DIR/rustup-init" || die 6 "rustup-init 에 실행권한을 줄 수 없다."
  RUSTUP_INIT="$DL_DIR/rustup-init"
}

component_args() {   # 쉼표 목록 → -c a -c b (전역 COMP_ARGS 배열)
  COMP_ARGS=(); local c; local IFS=','
  for c in $PIN_COMPONENTS; do COMP_ARGS+=(-c "$c"); done
}

verify_install() {
  structure_ok "$PIN_VERSION" || die 6 "설치 뒤에도 구조가 불완전하다: $STATE_WHY"
  local v
  v="$(run_env "$CARGO_HOME/bin/rustc" --version 2>&1)" || die 6 "설치된 rustc 를 실행할 수 없다: $v"
  [ "$(printf '%s' "$v" | awk '{print $2}')" = "$PIN_VERSION" ] || die 6 "설치된 rustc 버전이 pin 과 다르다: '$v' (pin $PIN_VERSION)"
  run_env "$CARGO_HOME/bin/cargo" fmt --version >/dev/null 2>&1 || die 6 "rustfmt(cargo fmt)를 실행할 수 없다 — 컴포넌트 설치 실패"
  run_env "$CARGO_HOME/bin/cargo" clippy --version >/dev/null 2>&1 || die 6 "clippy(cargo clippy)를 실행할 수 없다 — 컴포넌트 설치 실패"
  compute_state
  [ "$STATE" = ready ] || die 6 "설치 뒤 실행파일 검증 실패: $STATE_WHY"
  note "install: ready — $v · home=$RT_HOME"
}

cmd_install() {
  detect_platform; require_native_tools
  compute_state
  case "$STATE" in
    ready) note "install: 이미 ready — 다운로드·설치를 하지 않는다 ($STATE_WHY · home=$RT_HOME)"; return 0 ;;
    foreign) die 3 "$STATE_WHY" ;;
  esac
  mkdir -p "$RT_HOME" || die 6 "runtime home 을 만들 수 없다: $RT_HOME"
  component_args
  if [ "$STATE" != missing ] && proxies_ok; then
    note "install: state=$STATE ($STATE_WHY) — 기존 rustup 으로 복구·갱신(rustup-init 재다운로드 없음)"
    run_env "$CARGO_HOME/bin/rustup" toolchain install "$PIN_VERSION-$TRIPLE" --profile "$PIN_PROFILE" --no-self-update "${COMP_ARGS[@]}" \
      || die 6 "rustup toolchain install 실패 — 네트워크/디스크/기존 rustup 상태를 확인하라(runtime home 은 지우지 않았다: $RT_HOME)"
  else
    [ "$STATE" = missing ] || note "install: state=$STATE ($STATE_WHY) — 설치기 프록시 복구가 필요해 rustup-init 을 재실행"
    obtain_rustup_init
    local rc
    # 공식 rustup-init. --no-modify-path: 셸 profile 불변 · RUSTUP_INIT_SKIP_PATH_CHECK: 전역 rustc 가 있어도 작업공간 pin 을 설치.
    run_env RUSTUP_INIT_SKIP_PATH_CHECK=yes "$RUSTUP_INIT" -y --no-modify-path --profile "$PIN_PROFILE" \
        --default-host "$TRIPLE" --default-toolchain "$PIN_VERSION" "${COMP_ARGS[@]}"
    rc=$?
    [ -z "$DL_DIR" ] || rm -rf -- "$DL_DIR"
    [ "$rc" -eq 0 ] || die 6 "rustup-init 실패(rc=$rc) — 위 출력의 원인(네트워크·디스크·권한·링커)을 확인하라. runtime home 은 지우지 않았다: $RT_HOME (재실행하면 반쪽 상태를 복구한다)"
  fi
  verify_install
}

# ── check ─────────────────────────────────────────────────────────────────────
PROBE=""
cleanup_probe() {   # 자기 mktemp 산출물만 정리한다(이름 접두 확인).
  case "$PROBE" in */rust-bootstrap-check.*) [ -d "$PROBE" ] && rm -rf -- "$PROBE" ;; esac
}

cmd_check() {
  detect_platform; need_ready; require_native_tools
  local v sysroot out
  note "check: home=$RT_HOME triple=$TRIPLE pin=$PIN_VERSION state=$STATE"
  v="$(run_env "$CARGO_HOME/bin/rustc" --version 2>&1)" || die 7 "rustc --version 실패: $v"
  [ "$(printf '%s' "$v" | awk '{print $2}')" = "$PIN_VERSION" ] || die 7 "rustc 버전이 pin 과 다르다: '$v' (pin $PIN_VERSION)"
  note "check: $v"
  out="$(run_env "$CARGO_HOME/bin/cargo" --version 2>&1)" || die 7 "cargo --version 실패: $out"
  note "check: $out"
  run_env "$CARGO_HOME/bin/cargo" fmt --version >/dev/null 2>&1 || die 7 "cargo fmt 사용 불가"
  run_env "$CARGO_HOME/bin/cargo" clippy --version >/dev/null 2>&1 || die 7 "cargo clippy 사용 불가"
  sysroot="$(run_env "$CARGO_HOME/bin/rustc" --print sysroot 2>&1)" || die 7 "rustc --print sysroot 실패: $sysroot"
  case "$sysroot" in "$RUSTUP_HOME"/toolchains/*) ;; *) die 7 "선택된 toolchain 이 작업공간 설치가 아니다(sysroot=$sysroot) — pin 환경이 적용되지 않았다." ;; esac
  note "check: sysroot=$sysroot"

  PROBE="$(mktemp -d "${TMPDIR:-/tmp}/rust-bootstrap-check.XXXXXX")" || die 7 "probe 임시 디렉터리를 만들 수 없다(TMPDIR=${TMPDIR:-/tmp})"
  trap cleanup_probe EXIT
  mkdir -p "$PROBE/src"
  printf '%s\n' '[package]' 'name = "rust-bootstrap-probe"' 'version = "0.0.0"' 'edition = "2021"' 'publish = false' '' \
                '[dependencies]' '' '[workspace]' > "$PROBE/Cargo.toml"
  printf '%s\n' 'int rust_bootstrap_c_probe(void) { return 7; }' > "$PROBE/probe.c"
  cc -c "$PROBE/probe.c" -o "$PROBE/probe.o" \
    || die 7 "C probe 컴파일 실패 — native 의존성을 빌드할 C 컴파일러/assembler 를 확인하라"
  ar crs "$PROBE/librust_bootstrap_c_probe.a" "$PROBE/probe.o" \
    || die 7 "C probe 정적 아카이브 실패 — ar/binutils 를 확인하라"
  printf '%s\n' 'fn main() {' \
    '    let root = std::env::var("CARGO_MANIFEST_DIR").expect("probe root");' \
    '    println!("cargo:rustc-link-search=native={root}");' \
    '    println!("cargo:rustc-link-lib=static=rust_bootstrap_c_probe");' '}' > "$PROBE/build.rs"
  printf '%s\n' 'extern "C" {' '    fn rust_bootstrap_c_probe() -> i32;' '}' '' 'fn main() {' \
                '    // SAFETY: the linked C probe takes no arguments and returns an int.' \
                '    assert_eq!(unsafe { rust_bootstrap_c_probe() }, 7);' \
                '    let acc: u64 = (1..=10u64).map(|i| i * i).sum();' \
                '    println!("rust-bootstrap-probe-ok {acc}");' '}' > "$PROBE/src/main.rs"
  # 새 cwd(probe) · 네트워크 금지(CARGO_NET_OFFLINE + --offline) · 산출물은 probe 안에만.
  ( cd "$PROBE" && run_env CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$PROBE/target" \
      "$CARGO_HOME/bin/cargo" build --offline --manifest-path "$PROBE/Cargo.toml" ) \
    || die 7 "Rust/C probe offline build 실패 — 링커(cc)/아카이브(ar)/디스크/TMPDIR(noexec 여부)를 확인하라"
  out="$("$PROBE/target/debug/rust-bootstrap-probe" 2>&1)" || die 7 "probe native 실행 실패: $out"
  [ "$out" = "rust-bootstrap-probe-ok 385" ] || die 7 "probe 출력이 기대와 다르다: '$out'"
  note "check: native 실행 OK — $out"
  ( cd "$PROBE" && run_env CARGO_NET_OFFLINE=true "$CARGO_HOME/bin/cargo" fmt --manifest-path "$PROBE/Cargo.toml" -- --check ) \
    || die 7 "cargo fmt --check 실패"
  ( cd "$PROBE" && run_env CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$PROBE/target" \
      "$CARGO_HOME/bin/cargo" clippy --offline --manifest-path "$PROBE/Cargo.toml" -- -D warnings ) \
    || die 7 "cargo clippy -D warnings 실패"
  note "check: PASS — rustc/cargo/rustfmt/clippy + C 컴파일·아카이브·Rust 연결 offline build/native 실행(실제 실행 결과 · fixture 아님)"
}

# ── status / exec ─────────────────────────────────────────────────────────────
cmd_status() {
  detect_platform; compute_state
  printf 'state=%s\nreason=%s\nruntime_home=%s\npin=%s\ntriple=%s\n' "$STATE" "$STATE_WHY" "$RT_HOME" "$PIN_VERSION" "$TRIPLE"
  [ "$STATE" = ready ] && return 0
  return 3
}

cmd_exec() {
  [ "$#" -gt 0 ] || die 2 "exec 는 '--' 뒤에 실행할 명령이 필요하다."
  detect_platform; need_ready
  export CARGO_HOME RUSTUP_HOME
  export RUSTUP_TOOLCHAIN="$PIN_VERSION-$TRIPLE"
  export RUSTUP_AUTO_INSTALL=0
  export PATH="$CARGO_HOME/bin:$PATH"
  exec "$@"
}

# ── 진입 ──────────────────────────────────────────────────────────────────────
[ "$#" -ge 1 ] || { usage >&2; exit 2; }
CMD="$1"; shift
OPT_HOME=""; OPT_INIT_FILE=""; REST=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --runtime-home) [ "$#" -ge 2 ] && [ -n "$2" ] || die 2 "--runtime-home 에 경로가 필요하다."; OPT_HOME="$2"; shift 2 ;;
    --runtime-home=*) OPT_HOME="${1#*=}"; [ -n "$OPT_HOME" ] || die 2 "--runtime-home 에 경로가 필요하다."; shift ;;
    --rustup-init-file) [ "$#" -ge 2 ] && [ -n "$2" ] || die 2 "--rustup-init-file 에 경로가 필요하다."; OPT_INIT_FILE="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    --) shift; REST=("$@"); break ;;
    *) die 2 "알 수 없는 인자: $1 (사용법: $SELF --help)" ;;
  esac
done

case "$CMD" in
  -h|--help|help) usage; exit 0 ;;
  install|check|status|exec) ;;
  *) die 2 "알 수 없는 명령: $CMD (install|check|status|exec — $SELF --help)" ;;
esac
[ -z "$OPT_INIT_FILE" ] || [ "$CMD" = install ] || die 2 "--rustup-init-file 은 install 에서만 쓴다."
[ "$CMD" = exec ] || [ "${#REST[@]}" -eq 0 ] || die 2 "'--' 뒤 명령은 exec 에서만 쓴다."

load_pin
resolve_home
case "$CMD" in
  install) cmd_install ;;
  check)   cmd_check ;;
  status)  cmd_status ;;
  exec)    cmd_exec "${REST[@]}" ;;
esac
