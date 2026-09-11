#!/bin/bash
# 55-src-deps-authority.sh — **소스트리 의존 권위 조정**(build_patches_src 슬롯 · pre-compile)
#   前身: `45-flashinfer-0.6.17.sh`(2026-08-14, R2 전용 flashinfer 승격). 2026-08-15 R3 착수 중
#   일반화하며 개명·번호 이동했다(50 뒤여야 한다 — 아래 §순서).
#
# what : 클론된 vLLM 소스트리가 **자기 의존의 권위**가 되게 한다. Dockerfile L66-68 이 우리
#        stock `requirements.txt` 를 `/etc/pip/constraint.txt` 로 굽는데, 포크/이식 트리는
#        자기 requirements 를 더 높은 버전으로 들고 온다 → 그 `==` 핀끼리 충돌해
#        `pip install -e .` 가 ResolutionImpossible 로 죽는다. 이 스크립트가 **트리가 스스로
#        핀한 패키지에 한해** constraint 를 양보시키고, install_requires 에서 빠지는
#        패키지(cubin)를 업스트림 인덱스에서 직접 깐다.
#
# why-general : 2026-08-15 실측 — PR#41834 head(9ad62027) 는 stock v0.27.0 대비 **4종**을 올린다.
#          flashinfer-python  0.6.16.post3 → 0.6.17
#          flashinfer-cubin   0.6.16.post3 → 0.6.17
#          nvidia-cutlass-dsl 4.6.0        → 4.6.2
#          quack-kernels      0.6.1        → 0.6.4
#          humming-kernels    0.1.10       → 0.1.12
#        (fastsafetensors 는 `>=` 하한 상향이라 무충돌 · apache-tvm-ffi 0.1.11 · tilelang 0.1.12 는 동일)
#        전신처럼 flashinfer 만 풀면 나머지 3종에서 같은 죽음이 반복된다. 그래서 **패키지 목록을
#        손으로 적지 않고 트리에서 읽는다** — "파생 가능한데 손으로 적은 것"(4종 안티패턴의 하드코딩)
#        을 만들지 않기 위해서다.
#
# ★ 순서(중요): 번호가 **50 보다 뒤**여야 한다. R2 자체이식 칸에서 `50-dsv4-sm12x-port.sh` 가
#   소스트리의 `requirements/cuda.txt` 를 먼저 고쳐 쓰므로, 그 결과를 읽어야 트리의 최종 핀이 잡힌다.
#   (전신이 45 였던 것은 버전을 하드코딩해 순서 의존이 없었기 때문이다 — 일반화하며 그 전제가 깨졌다.)
#
# ★ 천장은 보존한다: constraint 의 `<`·`<=`·`!=` 는 KNOWN_INCOMPAT **시간드리프트 회귀 핀**
#   (예 fastapi<0.137.0 · transformers<5.15.0)이라 양보 대상이 아니다. 트리 핀이 그 천장을 넘으면
#   **조용히 넘기지 않고 fail-loud** 로 세운다 — 회귀 핀을 우회하는 것은 이 스크립트의 권한 밖이다.
#
# ★ 인덱스 주의(2026-08-14 실측, 유효): `flashinfer-cubin` 은 **PyPI 에 0.6.13 까지만** 있다.
#   0.6.16/0.6.17 은 업스트림 인덱스(https://flashinfer.ai/whl/)에만 존재하고, vLLM `setup.py` 는
#   cubin 을 install_requires 에서 제외한다(그래서 wheel 이 해소불가 핀을 안 들고 다닌다).
#   → 아무도 안 깔아주므로 여기서 명시 설치한다. `--extra-index-url` 이 필요한 유일한 이유다.
#   vLLM `get_requirements()` 는 `--` 로 시작하는 줄을 건너뛰므로 트리 안의 `--extra-index-url`
#   선언은 pip 에 전달되지 않는다 — 그것도 여기서 보충한다.
#
# 근거 문서: plan_26081410 §4 R3 · plan_26081418 §4.1 G-3 · testlog_26081423(R2)
set -euo pipefail

TAG="[55-src-deps]"

# ── 게이트 ────────────────────────────────────────────────────────────────────
#   SRC_DEPS_AUTHORITY=1 이면 동작. 기본 0 = off(stock 빌드 경로 완전 불변).
#   하위호환: off + SM12X_PORT=1(R2 자체이식) → 켠 것으로 간주하되 **경고를 찍는다**
#   (침묵 폴백 금지 — 결정론 규율 §4종 안티패턴 판정표의 fail-loud 폴백).
: "${SM12X_PORT:=0}"
: "${SRC_DEPS_AUTHORITY:=0}"
if [ "${SRC_DEPS_AUTHORITY}" != "1" ] && [ "${SM12X_PORT}" = "1" ]; then
    SRC_DEPS_AUTHORITY=1
    echo "$TAG ⚠ 폴백: SRC_DEPS_AUTHORITY off + SM12X_PORT=1 → on 으로 간주(R2 하위호환)."
    echo "$TAG   .env.<콤보> 에 SRC_DEPS_AUTHORITY=1 을 명시하라 — 이 폴백은 제거 예정이다."
fi
if [ "${SRC_DEPS_AUTHORITY}" != "1" ]; then
    echo "$TAG skip — SRC_DEPS_AUTHORITY=0 (constraint 그대로 · stock 빌드 경로 불변)"
    exit 0
fi

CONSTRAINT="/etc/pip/constraint.txt"
SRC="${SRC_TREE:-/workspace/vllm-src}"
FI_INDEX="https://flashinfer.ai/whl/"

[ -f "$CONSTRAINT" ] || { echo "$TAG FAIL: $CONSTRAINT 부재 — Dockerfile L66-68 스탠자가 바뀌었는지 확인하라" >&2; exit 1; }
[ -d "$SRC" ] || { echo "$TAG FAIL: 소스트리 $SRC 부재 — clone 스탠자가 바뀌었는지 확인하라" >&2; exit 1; }

echo "$TAG 소스트리=$SRC · constraint=$CONSTRAINT"

# ── 1) 트리의 `==` 핀을 읽어 constraint 를 양보시킨다(목록은 트리에서 파생 — 손으로 적지 않는다) ──
python3 - "$SRC" "$CONSTRAINT" <<'PY'
import re, sys, pathlib

src, constraint_path = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
TAG = "[55-src-deps]"

def norm(name):                      # PEP 503 정규화 — extras 는 벗긴다
    return re.sub(r"[-_.]+", "-", re.sub(r"\[.*?\]", "", name)).strip().lower()

# ⚠ 버전은 **문자열로 비교하면 안 된다**("0.6.17" < "0.6.9" 가 참이 되어 천장 침범을 놓친다).
try:
    from packaging.version import Version, InvalidVersion
    def vparse(s):
        try:
            return Version(s)
        except InvalidVersion:
            return None
except ImportError:                  # packaging 부재 시 숫자 튜플 폴백(fail-loud: 파싱 실패는 None)
    def vparse(s):
        m = re.match(r"^\d+(?:\.\d+)*", s or "")
        return tuple(int(x) for x in m.group(0).split(".")) if m else None

def read_reqs(path, seen=None):      # `-r` 재귀 (vLLM get_requirements 와 동형)
    seen = seen if seen is not None else set()
    if not path.exists() or path in seen:
        return []
    seen.add(path)
    out = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("-r "):
            out += read_reqs((path.parent / line.split(None, 1)[1].strip()).resolve(), seen)
        elif not line.startswith("-"):
            out.append(line)
    return out

# 소스트리가 `==` 로 못박은 것만 권위로 인정한다(하한 `>=` 은 constraint 와 다투지 않는다).
tree_pins = {}
for entry in read_reqs((src / "requirements" / "cuda.txt").resolve()):
    m = re.match(r"^([A-Za-z0-9._-]+(?:\[[^\]]*\])?)\s*==\s*([^\s;]+)", entry)
    if m:
        tree_pins[norm(m.group(1))] = m.group(2)

kept, yielded, violations = [], [], []
for raw in constraint_path.read_text(encoding="utf-8").splitlines():
    line = raw.strip()
    m = re.match(r"^([A-Za-z0-9._-]+)\s*(==|<=|<|!=|>=|>|~=)\s*([^\s;]+)", line)
    if not m:
        kept.append(raw); continue
    pkg, op, ver = norm(m.group(1)), m.group(2), m.group(3)
    want = tree_pins.get(pkg)
    if want is None:
        kept.append(raw); continue
    if op == "==":
        if want == ver:
            kept.append(raw)                      # 이미 일치 — 양보 불요
        else:
            yielded.append((pkg, ver, want))      # 트리가 권위 → constraint 줄 제거
        continue
    # 천장(`<`·`<=`·`!=`)은 보존한다. 트리 핀이 천장을 침범하면 조용히 넘기지 않는다.
    vw, vc = vparse(want), vparse(ver)
    if vw is None or vc is None:
        # 버전을 못 읽으면 **모른다고 말한다** — 침묵 통과가 아니라 명시 경고(판정은 사람/빌드가).
        print(f"{TAG} ⚠ 버전 파싱 실패로 천장 판정 생략: {pkg} tree={want} constraint={op}{ver}",
              file=sys.stderr)
    elif (op == "<" and vw >= vc) or (op == "<=" and vw > vc) or (op == "!=" and vw == vc):
        violations.append((pkg, f"{op}{ver}", want))
    kept.append(raw)

if violations:
    for pkg, ceiling, want in violations:
        print(f"{TAG} FAIL: 소스트리 {pkg}=={want} 가 KNOWN_INCOMPAT 천장 {ceiling} 을 침범한다", file=sys.stderr)
    print(f"{TAG}       천장은 시간드리프트 회귀 핀이다 — 이 스크립트가 우회할 권한이 없다.", file=sys.stderr)
    print(f"{TAG}       regen_requirements.py 의 KNOWN_INCOMPAT 를 재판정하라(사람 게이트).", file=sys.stderr)
    sys.exit(1)

constraint_path.write_text("\n".join(kept) + "\n", encoding="utf-8")
if yielded:
    for pkg, was, now in yielded:
        print(f"{TAG} 양보: {pkg} constraint=={was} → 소스트리=={now} (constraint 줄 제거)")
else:
    print(f"{TAG} 양보 대상 없음 — 소스트리 핀이 constraint 와 이미 일치한다")

# 다음 단계가 쓸 수 있게 트리의 flashinfer 핀을 남긴다(install_requires 제외분 설치용).
(pathlib.Path("/tmp/55_tree_pins.env")).write_text(
    "".join(f"{k.replace('-', '_').upper()}={v}\n" for k, v in sorted(tree_pins.items())
            if k.startswith("flashinfer")), encoding="utf-8")
PY

# ── 2) install_requires 에서 제외되는 트리 핀 설치(cubin — PyPI 부재라 업스트림 인덱스 필요) ──
FI_CUBIN=""
[ -f /tmp/55_tree_pins.env ] && FI_CUBIN=$(grep -E '^FLASHINFER_CUBIN=' /tmp/55_tree_pins.env | cut -d= -f2- || true)
if [ -n "$FI_CUBIN" ]; then
    echo "$TAG flashinfer-cubin==${FI_CUBIN} 설치(업스트림 인덱스 — setup.py 가 install_requires 에서 제외하는 항목)"
    pip install --no-cache-dir --extra-index-url "$FI_INDEX" "flashinfer-cubin==${FI_CUBIN}"
else
    echo "$TAG 소스트리에 flashinfer-cubin 핀 없음 — 설치 생략"
fi

# ── 3) fail-loud 검증 — 조용한 버전 폴백을 잡는다(이 슬롯의 존재 이유가 그것이다) ──
#   ⚠ flashinfer-python 은 **여기서 검증하지 않는다**: 뒤이은 `pip install -e .` 가 트리의
#     install_requires 로 그것을 설치/조정하며, 이 시점의 값은 최종값이 아니다. cubin 만
#     여기서 최종이다(아무도 안 건드리므로). 짝 일치(python==cubin)는 컴파일 후 검증 스탠자가 본다.
if [ -n "$FI_CUBIN" ]; then
    python3 - "$FI_CUBIN" <<'PY'
import sys
from importlib.metadata import PackageNotFoundError, version
want = sys.argv[1]
try:
    got = version("flashinfer-cubin")
except PackageNotFoundError:
    sys.exit(f"[55-src-deps] FAIL: flashinfer-cubin 부재 (기대 {want})")
if got != want:
    sys.exit(f"[55-src-deps] FAIL: flashinfer-cubin={got} (기대 {want})")
print(f"[55-src-deps] OK — flashinfer-cubin {want} 설치 확인")
PY
fi

echo "$TAG done (컴파일 전). 실제 커널 디스패치 성립은 serve 스모크가 최종 중재."
