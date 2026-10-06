#!/bin/bash
# ============================================================================================================
#  DRAFT(2026-10-04 · 커널 프로파일 후속 track K6): 정적 검증만 = v10probe 컨테이너 안에서 적용·재실행 no-op·py_compile·
#  CPU import(플래그 0/1)를 확인(track_k6/selftest_74.log). 비트 동일성·engaged·절감은 GPU 부팅 + 프로파일이 판정한다.
# ============================================================================================================
# 74-exl3-moe-prep-fused.sh - 빌드-바깥 의존 패치(build_patches 슬롯 · post-compile) · vllm_exl3 native MoE 입력 준비 1커널화
#
# what : sfxnz 저자 모듈 docker/patch/moe_prep_fused.py 를 핀 커밋(3de91468)에서 받아(트리 digest + sha256) **원문 그대로**
#        /opt/exl3-port/dsv41-patch/moe_prep_fused.py 에 설치하고, 73 이 설치한 vllm_exl3/exl3.py 끝에 무장 footer 1개를 붙인다.
#          footer = 저자 decode_levers._install_moe_prep_fused(decode_levers.py:245-250)의 재현:
#            DSV41_MOE_PREP_FUSED=1 일 때만 `import moe_prep_fused; print(f"dsv41: moe prep fused: {install()}")` ·
#            예외는 저자 decode_levers.install 과 같은 줄 `dsv41: decode lever moe-prep-fused FAILED, lever is OFF: ...` 로 삼킨다.
#          install() 은 저자 그대로 **런타임에** inspect.getsource(_apply_native_fused_moe / apply_exl3_experts) 를 5개 앵커로
#          재작성해 모듈 네임스페이스에 exec 한다 — (1) HEAD: selftest_once (2) glue 블록(map_topk_to_local·clamp·int32·valid·
#          fp16 캐스트·mul ≈14 커널) → Triton 커널 1개 (3) SIG: out_dtype 인자 (4) RET: fp16→model dtype 1회 캐스트
#          (5) CALL: apply_exl3_experts 가 out_dtype=x.dtype 전달. 앵커가 하나라도 어긋나면 저자 코드가 LOG_DISARMED 1줄 + stock 유지.
#        **우리 이탈(문서화)**:
#          (a) 무장 위치: 저자 sitecustomize→decode_levers(인터프리터 시작) 대신 exl3.py footer(플러그인 import 시점 · 80 지연 훅
#              경유). 같은 프로세스에서 같은 함수를 같은 순서로(row-policy 래핑 전) 재작성한다 — 우리 이미지엔 sitecustomize 없음.
#          (b) 모듈 경로: footer 가 sys.path 에 /opt/exl3-port/dsv41-patch 추가(저자 /opt/dsv41-patch · 92 와 같은 디렉터리).
#          (c) 빌드 시점 앵커 사전판정: 저자 patch_source/patch_experts_source 를 설치본 exl3.py 의 두 함수에 돌려(compile 까지)
#              런타임 DISARMED 를 빌드 실패로 앞당긴다. 저자 코드·자가시험(합성 m=1..8 + 첫 16 eager 호출 비트 대조)·폴백은 무수정.
# why  : 저자 flags.md:137 — routing 경로의 단일 블록 torch 커널 ~14개가 shared expert GEMM 뒤에 줄 서서 p2b 를 늦춘다.
#        serve 귀속(K_p vs A_p · c=1): router gate end → routed start −2.944 / −3.000 ms/step(rank0/rank1) · R35 번들 K 의 일부.
# 앵커 겹침 : 73 의 p2b-engaged 로그 편집은 `extra_args = ...` ↔ `result = fn(` 사이(glue 블록 뒤 · RET 앞)라 5개 앵커와 겹치지 않는다
#        (v10probe 설치본: 5개 OLD 앵커 각 1회 · NEW 0회 · ref 0909 설치본과 같은 위치). 로그가 참조하는 x2d·intermediate·k·mcg 는 재작성 뒤에도 정의됨.
# toggles(serve-time env · DSV41_ 는 .env 의 VLLM_RAY_EXTRA_ENV_VAR_PREFIXES_TO_COPY 로 Ray 워커에 간다):
#        DSV41_MOE_PREP_FUSED=0(저자 코드 기본 · 미설정 = 함수 원본 그대로 = 롤백)|1 · DSV41_MOE_PREP_VERIFY(기본 16 · 최소 1).
#        native p2b 경로(VLLM_EXL3_MOE_KERNEL=native) + expert_map None(TP 배치)에서만 융합 커널을 탄다 — 그 밖은 저자 stock ops.
# engaged 증거 : `dsv41: moe prep fused: vllm_exl3 _apply_native_fused_moe glue fused, p2b output cast once (verify=16)`(프로세스당 1회)
#        → `dsv41: moe prep fused self-check bit-exact (synthetic, m=1..8)`(워커 첫 eager 호출) + 프로파일에서 routing 경로 소형 커널 소멸.
#   반증 : `dsv41: moe prep fused DISABLED -> stock ops: ...` · `dsv41: moe prep fused: not installed` ·
#          `dsv41: decode lever moe-prep-fused FAILED, lever is OFF: ...`.
# provenance : PROVENANCE.json sources.sfxnz_moeprep(커밋·트리 digest 는 sources.sfxnz_p2b 와 동일해야 함 · arming_reference = decode_levers.py sha256).
#        MIT LICENSE → /opt/exl3-port/licenses/sfxnz-moeprep/. 받은 트리·tarball 은 실행 직후 삭제.
# prerequisite : 73(vllm-exl3 K1 판 = /opt/exl3-port/.vllm-exl3-built + p2b-engaged 편집). 없으면 die.
# idempotent : footer 마커 + 설치 모듈 sha256 이 맞으면 fetch 없이 no-op · 둘 중 하나만 있거나 NEW 앵커가 이미 있으면 die.
#        73 이 다시 빌드(pip 재설치)하면 footer 가 사라지므로 74 가 다시 붙는다(정렬 순서 73 < 74 < 79).
set -euo pipefail

TAG="[74-exl3-moe-prep-fused]"

: "${SM12X_PORT:=0}"
if [ "${SM12X_PORT}" != "1" ]; then
    echo "$TAG skip - SM12X_PORT=${SM12X_PORT} (stock 빌드 경로 불변)"
    if [ -n "${EASY_VLLM_PATCH_STATUS:-}" ]; then printf 'skipped\tSM12X_PORT!=1\n' > "$EASY_VLLM_PATCH_STATUS"; fi
    exit 0
fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIBDIR=""
for d in "${EXL3_PORT_LIBDIR:-}" "$HERE/../build_patches_src" /tmp/build_patches_src; do
    if [ -n "$d" ] && [ -f "$d/exl3_port_lib.py" ] && [ -f "$d/PROVENANCE.json" ]; then LIBDIR="$(cd "$d" && pwd)"; break; fi
done
[ -n "$LIBDIR" ] || { echo "$TAG FAIL: exl3_port_lib.py / PROVENANCE.json 를 찾지 못했다(build_patches_src 와 함께 배달돼야 한다)" >&2; exit 1; }

# cwd=/ : 소스 트리가 설치본을 가리지 않게(73 check_installed 와 같은 이유)
( cd / && PYTHONPATH="$LIBDIR${PYTHONPATH:+:$PYTHONPATH}" python3 - "$TAG" <<'PY'
import ast
import importlib.util
import pathlib
import py_compile as _pyc
import shutil
import subprocess
import sys

import exl3_port_lib as L

TAG = sys.argv[1]
M = "# [exl3-port:moe-prep-fused]"
SCRIPT = "docker/patch/moe_prep_fused.py"
FUNCS = ("_apply_native_fused_moe", "apply_exl3_experts")

P = L.prov()
S = P["sources"]
if "sfxnz_moeprep" not in S:
    L.die(TAG, "PROVENANCE.json lacks sources.sfxnz_moeprep")
W, X = S["sfxnz_moeprep"], S["sfxnz_p2b"]
if W.get("chain") != [SCRIPT] or set(W.get("files_sha256", {})) != {SCRIPT}:
    L.die(TAG, "sources.sfxnz_moeprep chain/files_sha256 disagree with the module this patch installs")
if W["commit"] != X["commit"] or W["tree_digest"] != X["tree_digest"]:
    L.die(TAG, "sources.sfxnz_moeprep must pin the same sfxnz commit/tree as sources.sfxnz_p2b")
RT = pathlib.Path(W["runtime_install"]["dir"])
RT_FILES = W["runtime_install"]["files"]       # {repo rel: 설치 파일명}
if RT_FILES != {SCRIPT: "moe_prep_fused.py"} or str(RT) != "/opt/exl3-port/dsv41-patch":
    L.die(TAG, "sources.sfxnz_moeprep.runtime_install disagrees with the sys.path entry the footer appends")
AREF = W["arming_reference"]

# ---- 선행: 73(K1 판) 설치본 ----
opt = pathlib.Path(P["install"]["opt_dir"])
if not (opt / ".vllm-exl3-built").is_file():
    L.die(TAG, "73-vllm-exl3-build marker absent - build the plugin first")
spec = importlib.util.find_spec("vllm_exl3")          # 패키지를 실행하지 않고 위치만 찾는다
if spec is None or not spec.origin:
    L.die(TAG, "vllm_exl3 is not installed")
target = pathlib.Path(spec.origin).with_name("exl3.py")
if not target.is_file():
    L.die(TAG, f"target missing: {target}")
text = target.read_text()
if "# [exl3-port:vllm-exl3] p2b-engaged" not in text:
    L.die(TAG, f"{target}: 73 p2b-engaged edit absent (not the K1 build this patch was audited against)")

FOOT_MARK = f"{M} arming footer"
FOOTER = f'''

{FOOT_MARK} - sfxnz decode_levers._install_moe_prep_fused (3de91468, decode_levers.py:245-250) at plugin import.
# The author module (installed verbatim in {RT}) rewrites _apply_native_fused_moe / apply_exl3_experts in this
# module's namespace via inspect.getsource + exec. DSV41_MOE_PREP_FUSED unset/0 = nothing happens (stock functions).
def _exl3port_arm_moe_prep_fused():
    import os
    import sys

    if (os.environ.get("DSV41_MOE_PREP_FUSED", "0") or "0") != "1":
        return
    try:
        if "{RT}" not in sys.path:
            sys.path.append("{RT}")
        import moe_prep_fused

        print(f"dsv41: moe prep fused: {{moe_prep_fused.install()}}", flush=True)
    except Exception as exc:  # noqa: BLE001 - one lever never blocks the others (author decode_levers.install)
        print(f"dsv41: decode lever moe-prep-fused FAILED, lever is OFF: {{exc!r}}", flush=True)


_exl3port_arm_moe_prep_fused()
'''


def runtime_ok() -> bool:
    return all((RT / dst).is_file() and L.sha256_file(RT / dst) == W["files_sha256"][src] for src, dst in RT_FILES.items())


def segments(src: str) -> dict:
    """설치본 exl3.py 의 두 대상 함수 원문(inspect.getsource 가 돌려줄 def..끝 줄 범위 · 끝 개행 포함)."""
    tree = ast.parse(src)
    lines = src.splitlines(keepends=True)
    out = {}
    for name in FUNCS:
        fns = [n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == name]
        if len(fns) != 1 or fns[0].decorator_list:
            L.die(TAG, f"{target.name}: top-level def {name} count={len(fns)} (or decorated) - drifted from the audited shape")
        out[name] = "".join(lines[fns[0].lineno - 1:fns[0].end_lineno])
    return out


def dry_run(mod, src: str) -> None:
    """저자 patch_source/patch_experts_source 를 설치본 함수에 적용(런타임과 같은 입력) + compile. 실패 = 빌드 실패."""
    seg = segments(src)
    for name, old in (("OLD_BLOCK", mod.OLD_BLOCK), ("SIG_OLD", mod.SIG_OLD), ("HEAD_OLD", mod.HEAD_OLD),
                      ("RET_OLD", mod.RET_OLD)):
        if seg["_apply_native_fused_moe"].count(old) != 1 or src.count(old) != 1:
            L.die(TAG, f"author anchor {name}: count != 1 in _apply_native_fused_moe/file - runtime would print LOG_DISARMED")
    if seg["apply_exl3_experts"].count(mod.CALL_OLD) != 1 or src.count(mod.CALL_OLD) != 1:
        L.die(TAG, "author anchor CALL_OLD: count != 1 in apply_exl3_experts/file - runtime would print LOG_DISARMED")
    for new in (mod.NEW_BLOCK, mod.SIG_NEW, mod.HEAD_NEW, mod.RET_NEW, mod.CALL_NEW):
        if new in src:
            L.die(TAG, f"{target.name}: author NEW text already present on disk (foreign text rewrite) - rebuild from 73")
    try:
        a = mod.patch_source(seg["_apply_native_fused_moe"])
        b = mod.patch_experts_source(seg["apply_exl3_experts"])
        compile(a, "<moe_prep_fused dry-run>", "exec")
        compile(b, "<moe_prep_fused dry-run>", "exec")
    except (ValueError, SyntaxError) as exc:
        L.die(TAG, f"author rewrite dry-run failed on the installed exl3.py: {exc}")
    # 재작성본이 73 p2b-engaged 로그를 그대로 품는가(겹침 없음의 증거)
    if a.count("# [exl3-port:vllm-exl3] p2b-engaged") != 1 or "map_topk_to_local(" in a:
        L.die(TAG, "dry-run: rewritten _apply_native_fused_moe lost the 73 p2b-engaged log or kept the stock glue")


def load_author(path: pathlib.Path):
    spec = importlib.util.spec_from_file_location("_sfxnz_moe_prep_fused", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)          # 최상위 import 는 stdlib 전용(저자 docstring)
    return mod


# ---- 멱등 판정: footer + 설치 모듈 둘 다면 no-op · 하나만이면 die ----
done = {"footer": FOOT_MARK in text, "runtime": runtime_ok()}
if all(done.values()):
    dry_run(load_author(RT / "moe_prep_fused.py"), text.split(FOOT_MARK, 1)[0])
    print(f"{TAG} already applied - no-op")
    L.status("applied", "already present (no-op rerun)")
    raise SystemExit(0)
if any(done.values()) or M in text:
    L.die(TAG, f"partial/foreign moe-prep-fused state ({done}) - rebuild from 73 (v10probe)")

cache = pathlib.Path(P["install"]["fetch_cache"])
root = cache / f"sfxnz_moeprep-{W['commit']}"
L.fetch_pinned_tree(TAG, "sfxnz_p2b", root)
try:
    for rel, want in W["files_sha256"].items():
        if L.sha256_file(root / rel) != want:
            L.die(TAG, f"sfxnz {rel} sha256 mismatch")
    if L.sha256_file(root / AREF["file"]) != AREF["sha256"]:
        L.die(TAG, f"sfxnz {AREF['file']} (arming reference) sha256 mismatch")
    for f, want in W["license"]["license_sha256"].items():
        if L.sha256_file(root / f) != want:
            L.die(TAG, f"sfxnz {f} sha256 mismatch")
    author = load_author(root / SCRIPT)
    if author.LOG_ENGAGED != "dsv41: moe prep fused self-check bit-exact" or author.DEFAULT_VERIFY != 16:
        L.die(TAG, "author moe_prep_fused.py LOG_ENGAGED/DEFAULT_VERIFY disagree with this patch's documented strings")
    dry_run(author, text)
    # 런타임 모듈 원문 설치(92 가 만든 디렉터리 공유 · 다른 파일은 건드리지 않는다) + sha256 재대조.
    RT.mkdir(parents=True, exist_ok=True)
    for src, dst in RT_FILES.items():
        (RT / dst).write_bytes((root / src).read_bytes())
    if not runtime_ok():
        L.die(TAG, f"installed runtime module under {RT} fails sha256")
    lic = pathlib.Path(P["install"]["licenses_dir"]) / "sfxnz-moeprep"
    shutil.rmtree(lic, ignore_errors=True)
    lic.mkdir(parents=True)
    for f in W["license"]["license_sha256"]:
        (lic / f).write_bytes((root / f).read_bytes())
    (lic / "NOTICE").write_text(f"sfxnz/{W['repo'].split('/')[-1]} @ {W['commit']}: MIT (LICENSE) - moe_prep_fused.py kept "
                                f"verbatim in {RT}; armed by a footer appended to the installed vllm_exl3/exl3.py.\n")
finally:
    shutil.rmtree(root, ignore_errors=True)
    (cache / f"sfxnz_p2b-{W['commit']}.tar.gz").unlink(missing_ok=True)

print(f"{TAG} exl3.py: {', '.join(L.apply_edits(TAG, target.name, [], FOOTER, FOOT_MARK, root=target.parent))}")
L.py_compile(TAG, [target, RT / "moe_prep_fused.py"])
_pyc.compile(str(target), doraise=True)      # 설치본 __pycache__ 를 새 원문에 맞춘다

# ---- post-asserts ----
t = target.read_text()
if t.count(FOOT_MARK) != 1 or t.count("\n_exl3port_arm_moe_prep_fused()\n") != 1 or not t.startswith(text):
    L.die(TAG, "post-assert: footer count != 1 or the edit touched text above the footer")
# 깨끗한 인터프리터에서 플러그인 import(GPU 불요): 플래그 0 = 원본 함수 그대로 · 플래그 1 = 저자 install() 성공 문자열 + 재작성 확인.
CHK = r'''
import inspect, os, sys
import vllm_exl3.exl3 as m
on = os.environ.get("DSV41_MOE_PREP_FUSED") == "1"
fn, ex = m._apply_native_fused_moe, m.apply_exl3_experts
if on:
    assert getattr(fn, "_dsv41_moe_prep", False), "install() did not mark _apply_native_fused_moe"
    assert fn.__code__.co_filename.endswith("[dsv41 moe_prep_fused]"), fn.__code__.co_filename
    assert ex.__code__.co_filename.endswith("[dsv41 moe_prep_fused]"), ex.__code__.co_filename
    assert "out_dtype" in inspect.signature(fn).parameters
    assert type(m._dsv41_moe_prep).__name__ == "MoePrep"
    import moe_prep_fused as a
    assert a._STATE == {"armed": True, "verify_left": 16, "engaged": False}, a._STATE
else:
    assert not hasattr(fn, "_dsv41_moe_prep") and "_dsv41_moe_prep" not in vars(m), "lever armed with the flag off"
    assert fn.__code__.co_filename == m.__file__ and ex.__code__.co_filename == m.__file__
    assert "out_dtype" not in inspect.signature(fn).parameters
    assert "moe_prep_fused" not in sys.modules
print("moe-prep-check flag=%s ok" % ("1" if on else "0"))
'''
for flag in ("0", "1"):
    env = {k: v for k, v in __import__("os").environ.items() if not k.startswith("DSV41_MOE_PREP")}
    env["DSV41_MOE_PREP_FUSED"] = flag
    r = subprocess.run([sys.executable, "-c", CHK], capture_output=True, text=True, cwd="/", env=env)
    out = (r.stdout + r.stderr)
    want = "dsv41: moe prep fused: vllm_exl3 _apply_native_fused_moe glue fused, p2b output cast once (verify=16)"
    if r.returncode != 0 or f"moe-prep-check flag={flag} ok" not in out or (flag == "1") != (want in out) \
            or "lever is OFF" in out or "DISABLED" in out:
        L.die(TAG, f"post-assert: plugin import with DSV41_MOE_PREP_FUSED={flag} failed: {out.strip()[-900:]}")
    print(f"{TAG} import check DSV41_MOE_PREP_FUSED={flag}: ok" + (f" ({want})" if flag == "1" else " (stock functions)"))
L.status("applied", f"sfxnz moe_prep_fused @ {W['commit'][:12]} (runtime {RT}) + exl3.py arming footer (DSV41_MOE_PREP_FUSED gate)")
print(f"{TAG} OK")
PY
)
