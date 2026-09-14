#!/usr/bin/env bash
# judge_bench.sh — 루브릭 권한(authority)을 판정기까지 나르는 통로 (plan_26090415 §1.2 · CP2)
#
# 왜 있나: `verdict_rule.py` 는 `--authority weak|explicit|explore` 를 1년 가까이 갖고 있었지만
#   **그 플래그를 넘기는 실행 코드가 저장소 전체에 0건**이었다(인증서 45건 중 explore 0건이 방증).
#   판정 전 단계(roofline → verdict)를 사람이 매번 손으로 이어 붙였고, 그 손이 플래그를 잊으면
#   `default="weak"` 가 **조용히** 낙찰됐다. 설계가 아니라 배선이 빠져 있었다.
#   이 스크립트가 그 손을 대체한다 — 그리고 **권한을 발명하지 않는다**(아래).
#
# ★ `--authority` 는 필수다. 기본값이 없다.
#   근거: 권한은 **사용자 HITL 트리거**이지 스크립트의 판단이 아니다(SKILL.md §2). 기본값을 두면
#   트리거가 없어도 판정이 나오고, 그 판정은 "사용자가 약한 권한을 골랐다"와 "아무도 안 골랐다"를
#   구분하지 못한다. `config.example.yaml` 이 `authority` 를 **파일 키로 금지**한 것도 같은 이유다
#   (파일에 적히면 "사람이 당겼다"는 증거가 약해진다) — 그래서 통로는 파일이 아니라 **호출 인자**다.
#
# ★ 권한 조합 규칙(explicit↔--target-tps 필수 · explore↔--target-tps 금지 · 0/NaN 거부)은 여기서
#   **복제하지 않는다**. 그 규칙의 단일 소유자는 `verdict_rule.py` 이고, 여기서 한 벌 더 적으면
#   두 벌이 갈라진다. 이 스크립트는 인자를 그대로 넘기고 판정기의 exit 2 를 그대로 전달한다.
#
# 입력은 `sweep_index.json` 하나다 — model_path·tp·manifest 파생을 여기서 다시 하지 않는다
#   (sweep_bench.sh 가 이미 실측해 meta 에 넣었다. 파생을 복제하면 두 벌이 어긋난다).
#
# ★ accept_len 은 **승계 XOR 부재 명시**다(2026-09-14 · plan_26091407 §4.1 · F6).
#   종전에는 사람이 `--accept-len` 을 줄 때만 루프라인에 넘겼고, 안 주면 roofline 기본 1.0 이 **조용히**
#   낙찰됐다 — roofline.json 49건 중 47건이 1.0 이었고 그중 약 11건은 spec 이 켜진 채 실측값이 있었다.
#   그 1.0 은 R_token 만이 아니라 expected_achievable(= realistic_fraction × accept_len × R_fp)에도
#   곱해져 **합격선을 함께 내린다**. 한 층 아래 `parse_guidellm.py` 가 이미 가진 규율
#   (`--accept-len-src` XOR `--spec-axis-absent` + `spec_axis_source`)을 여기에 동형으로 적용한다:
#     사람 명시 `--accept-len L`             → --accept-len L --accept-len-source declared
#     판정 레벨 measured.json 의 유효 실측    → --accept-len v --accept-len-source measured (승계)
#     실측 없음 ∧ meta.spec_declared=off     → --accept-len-source declared-absent (1.0 이 정상)
#     실측 없음 ∧ on|unknown                 → --accept-len-source absent (결손 여부는 판정기가 정한다)
#   spec 선언 지문(`meta.spec_declared` — sweep_bench 가 서빙 yaml 에서 읽는다)은 판정기에
#   `--spec-declared on|off|unknown` 으로 **승계**한다(부재 = unknown · 여기서 yaml 을 다시 읽지 않는다).
#   결손의 사유코드(SPEC_ACCEPT_LEN_MISSING)와 accept_len 유효성 술어는 여기서 복제하지 않는다 —
#   각각 `verdict_rule.py`·`roofline.py` 가 소유하고 이 스크립트는 import 해서 쓴다.
#
# 사용: judge_bench.sh <config_name> --authority weak|explicit|explore
#         [--topology single|multi] [--sweep-dir DIR] [--level N]
#         [--target-tps X] [--reference-tps E] [--e-search hit|empty|no]
#         [--tolerance T] [--realistic-fraction F] [--accept-len L] [--spec-supported]
#         [--node-vram-gib "a,b"] [--out FILE] [--out-dir DIR] [--no-publish] [--dry-run]
#       judge_bench.sh --self-test      (격리 임시 저장소에서 승계·명시·spec off·결손·발행 억제를 실행 검증)
# 산출: <sweep_dir>/roofline.json · <sweep_dir>/verdict.json (또는 --out · --out-dir)
#   --out-dir DIR : roofline.json·verdict.json 을 sweep 디렉터리가 아니라 DIR 에 쓴다(오프라인 재판정 ·
#                   원 판정 산출물 불변). 자리를 옮긴 판정은 그 스윕의 정본 verdict 가 아니므로
#                   **인증서 자동 발행을 억제한다**(--no-publish 를 함의).
#   --no-publish  : 제자리 판정이어도 인증서 자동 발행을 건너뛴다(기재 로그는 남긴다).
# 종료: 0=판정 산출 · 2=인자/전제 오류 · 3=판정점 측정치 부재 · 그 외=하위 스크립트 rc 전달
set -euo pipefail

if [ "${1:-}" = "--self-test" ]; then
  # 자체검사 본문은 별도 파일이다(파이썬 단언이 셸 heredoc 보다 읽기 쉽다). 검사 대상은 이 파일의
  # **바이트 사본**이므로 여기 적힌 코드가 그대로 시험된다.
  exec python3 "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/selftest_judge_bench.py"
fi

CONFIG="${1:?config_name 필요}"; shift || true
AUTHORITY=""; TOPO=""; SWEEPDIR=""; LEVEL=1; OUT=""; OUTDIR=""; NO_PUBLISH=0; DRYRUN=0; CHECK_ARGS=0
PASS_ARGS=(); SPEC=0
while [ $# -gt 0 ]; do case "$1" in
  --authority)          AUTHORITY="$2"; shift 2;;
  # 인자만 검사하고 나간다(파일 전제 불요). 가드가 "권한 미선언"과 "산출물 부재"를 **구분**할 수
  # 있어야 하기 때문이다 — 둘 다 exit 2 이면 기본값을 넣어도 rc 가 안 바뀌어, 가드가 틀린 이유로
  # 통과한다(2026-09-04 실제로 그렇게 만들었다가 변이 시험에서 잡혔다).
  --check-args)         CHECK_ARGS=1; shift;;
  --topology)           TOPO="$2"; shift 2;;
  --sweep-dir)          SWEEPDIR="$2"; shift 2;;
  --level)              LEVEL="$2"; shift 2;;
  --out)                OUT="$2"; shift 2;;
  --out-dir)            OUTDIR="$2"; shift 2;;
  --no-publish)         NO_PUBLISH=1; shift;;
  --dry-run)            DRYRUN=1; shift;;
  --spec-supported)     SPEC=1; shift;;
  # 판정기·루프라인이 소유한 노브는 **주어졌을 때만** 전달한다. 여기서 기본값을 복제하면
  # 같은 상수가 두 곳에 손으로 적힌다(workflow.md §4종 안티패턴 — 매직넘버 결함 칸).
  --target-tps|--reference-tps|--e-search|--tolerance|--node-vram-gib)
                        PASS_ARGS+=("$1" "$2"); shift 2;;
  --realistic-fraction) RF="$2"; shift 2;;
  --accept-len)         AL="$2"; shift 2;;
  *) echo "[judge_bench] 알 수 없는 인자: $1" >&2; exit 2;;
esac; done

if [ -z "$AUTHORITY" ]; then
  cat >&2 <<'MSG'
[judge_bench] ERROR --authority 는 필수다(기본값 없음).
  weak     = 사용자가 목표 tok/s 를 명시하지 않았다(E > c > expected)
  explicit = 사용자가 HITL 로 목표를 명시했다(c > E > expected · --target-tps 필수)
  explore  = 사용자가 HITL 로 광범위 탐색/목표 미설정을 지시했다(E > expected · --target-tps 금지)
  권한은 사용자 트리거이지 스크립트의 판단이 아니다 — 그래서 이 자리에 기본값을 두지 않는다.
MSG
  exit 2
fi

# 권한 값의 값역 검사(weak|explicit|explore)는 여기서 하지 않는다 — 그 닫힌 목록의 단일 소유자는
# `verdict_rule.py` 이고, 한 벌 더 적으면 두 벌이 갈라진다. 여기가 지키는 것은 **선언 여부**뿐이다.
[ "$CHECK_ARGS" = 1 ] && { echo "[judge_bench] check-args OK — authority=$AUTHORITY"; exit 0; }

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "$SDIR" rev-parse --show-toplevel 2>/dev/null || pwd)"
if [ -z "$TOPO" ]; then
  BR="$(git -C "$REPO" rev-parse --abbrev-ref HEAD 2>/dev/null || echo)"
  case "$BR" in multi-node) TOPO=multi;; single-node) TOPO=single;; *) TOPO=single;; esac
fi
[ -n "$SWEEPDIR" ] || SWEEPDIR="$REPO/output/$TOPO/benchlog/sweep_${CONFIG}"
INDEX="$SWEEPDIR/sweep_index.json"
MANIFEST="$REPO/output/$TOPO/manifest.yaml"
# 판정 산출 자리: 기본은 sweep 디렉터리(broad_search·render_report 가 거기서 읽는다).
#   --out-dir 은 재판정용 자리다 — 원 판정 산출물을 덮지 않는다.
if [ -n "$OUTDIR" ]; then
  ROOF="$OUTDIR/roofline.json"
  [ -n "$OUT" ] || OUT="$OUTDIR/verdict.json"
  NO_PUBLISH=1
else
  ROOF="$SWEEPDIR/roofline.json"
  [ -n "$OUT" ] || OUT="$SWEEPDIR/verdict.json"
fi
LEVEL_REL="level_$(printf '%02d' "$LEVEL")/measured.json"
MEASURED="$SWEEPDIR/$LEVEL_REL"

if [ ! -f "$INDEX" ]; then
  echo "[judge_bench] ERROR sweep_index.json 부재: $INDEX — 먼저 sweep_bench.sh 를 돌려라" >&2; exit 2
fi
if [ ! -f "$MEASURED" ]; then
  echo "[judge_bench] ERROR 판정점(level $LEVEL) 측정치 부재: $MEASURED" >&2; exit 3
fi

# meta 에서 루프라인 입력을 읽는다(파생 ✗ · 승계 ○).
#   tp 가 meta 에 없으면 **여기서 1 로 채우지 않는다** — 그 기본값의 단일 소유자는 roofline.py 이며
#   manifest 의 nodes×gpus 에서 파생한다. 여기서 1 을 적으면 멀티 스윕에서 조용히 틀린 tp 로
#   루프라인을 세우게 된다(같은 개념이 두 곳에 손으로 적힌 값 = 매직넘버 결함 칸).
read -r MODEL_PATH TP < <(python3 - "$INDEX" <<'PY'
import json, sys
meta = (json.load(open(sys.argv[1], encoding="utf-8")).get("meta") or {})
# sweep meta 의 키는 `tensor_parallel_size` 다. 종전에 `tp` 를 읽어 **항상 미승계**였고,
# single 에서는 roofline 이 manifest 로 1 을 파생해 우연히 맞았지만 multi 에서는 틀린 상한을
# 세운다(2026-09-04 실측 발견 — 값이 없어도 조용히 도는 형태라 로그로는 안 보인다).
tp = meta.get("tensor_parallel_size", meta.get("tp"))
try:
    tp = int(tp)
except (TypeError, ValueError):
    tp = None
print(meta.get("model_path") or "NA",
      tp if isinstance(tp, int) and not isinstance(tp, bool) and tp > 0 else "-")
PY
)
if [ "$MODEL_PATH" = "NA" ]; then
  echo "[judge_bench] ERROR sweep_index.meta.model_path 가 NA — 루프라인 입력이 성립하지 않는다" >&2; exit 2
fi

# spec 축 승계(위 헤더 ★). 한 줄 = US(\x1f) 구분 4필드: spec_declared · accept_len(없으면 -) · 출처 · 근거.
#   구분자를 탭이 아니라 US 로 두는 이유: 탭은 IFS 공백류라 빈 필드가 접혀 뒤 필드가 밀린다.
#   유효성 술어는 roofline.accept_len_state 를 **import** 한다(복제 ✗). 사람 명시값의 유효성은 여기서
#   판정하지 않고 roofline 이 exit 2 로 거부한다(값역의 단일 소유자).
#   파싱이 실패하면(측정 JSON 손상 등) 빈 줄로 흘리지 않고 멈춘다 — 빈 값이 absent 로 접히면 결손이 위장된다.
_SPEC_ROW="$(
  SDIR="$SDIR" AL_HUMAN="${AL:-}" LEVEL_REL="$LEVEL_REL" python3 - "$INDEX" "$MEASURED" <<'PY'
import json, os, sys
sys.path.insert(0, os.environ["SDIR"])
from roofline import (ACCEPT_LEN_ABSENT, ACCEPT_LEN_DECLARED, ACCEPT_LEN_DECLARED_ABSENT,
                      ACCEPT_LEN_MEASURED, accept_len_state)
from verdict_rule import SPEC_DECLARED

meta = (json.load(open(sys.argv[1], encoding="utf-8")).get("meta") or {})
declared = meta.get("spec_declared")
# 값역 밖·부재는 unknown 이다 — 모르는 선언을 off 로 접으면 결손이 "정상 1.0" 으로 위장한다.
declared = declared if declared in SPEC_DECLARED else "unknown"
rel = os.environ["LEVEL_REL"]
human = os.environ.get("AL_HUMAN") or ""
md = json.load(open(sys.argv[2], encoding="utf-8"))
raw = md.get("accept_len") if isinstance(md, dict) else None
state, val = accept_len_state(raw)
# 파서 쪽 `spec_axis_source` 는 **다른 층의 어휘**다(parse_guidellm: 승계원 부재 선언 ≠ roofline
#   declared-absent — roofline.py 어휘 주석). 출처로 옮기지 않고 층 이름을 붙여 근거에만 싣는다.
axis_src = md.get("spec_axis_source") if isinstance(md, dict) else None
axis_note = " · measured.spec_axis_source=%s" % axis_src if axis_src else ""
if human:
    # 사람 명시가 이긴다(declared). 그러나 덮은 실측을 **지우지 않는다** — 명시값은 expected_achievable
    #   (합격선)에도 곱해지므로, 실측과 다른 명시는 흔적 없이 문턱을 옮긴다(측정 > 선언 · 기재만 · 게이트 ✗).
    ev = "argv(--accept-len)"
    if state == "valid":
        ev += " · shadowed measured=%r(%s)" % (val, rel)
    row = (declared, human, ACCEPT_LEN_DECLARED, ev)
elif state == "valid":
    row = (declared, repr(val), ACCEPT_LEN_MEASURED, rel + axis_note)
elif declared == "off":
    # 선언의 출처도 싣는다 — 재조립·소급으로 **현재 파일에서** 뽑은 off 는 retro 표지를 달고 온다.
    row = (declared, "-", ACCEPT_LEN_DECLARED_ABSENT,
           "sweep_index.meta.spec_declared=off(%s) · %s accept_len=%r"
           % (meta.get("spec_declared_source"), rel, raw))
else:
    row = (declared, "-", ACCEPT_LEN_ABSENT, "%s accept_len=%r(%s)%s" % (rel, raw, state, axis_note))
print("\x1f".join(str(x).replace("\x1f", " ") for x in row))
PY
)" || { echo "[judge_bench] ERROR spec 축 승계 입력 해석 실패($INDEX · $MEASURED) — 판정하지 않는다" >&2; exit 2; }
IFS=$'\x1f' read -r SPEC_DECLARED AL_VALUE AL_SOURCE AL_EVIDENCE <<< "$_SPEC_ROW"
NAS_ROOT="$(sed -n 's/^[[:space:]]*nas_model_path:[[:space:]]*"\{0,1\}\([^"#]*\)"\{0,1\}.*/\1/p' \
             "$MANIFEST" 2>/dev/null | head -1 | sed 's/[[:space:]]*$//')"
# quant 루트도 같은 문법으로(2026-09-10 · camp-26090918 — /app/quant_models 매핑의 호스트 루트)
QUANT_ROOT="$(sed -n 's/^[[:space:]]*quant_model_path:[[:space:]]*"\{0,1\}\([^"#]*\)"\{0,1\}.*/\1/p' \
             "$MANIFEST" 2>/dev/null | head -1 | sed 's/[[:space:]]*$//')"

ROOF_CMD=(python3 "$SDIR/roofline.py" --model-path "$MODEL_PATH" --json)
[ "$TP" != "-" ] && ROOF_CMD+=(--tp "$TP")
[ -f "$MANIFEST" ] && ROOF_CMD+=(--manifest "$MANIFEST")
[ -n "$NAS_ROOT" ] && ROOF_CMD+=(--nas-root "$NAS_ROOT")
[ -n "$QUANT_ROOT" ] && ROOF_CMD+=(--quant-root "$QUANT_ROOT")
[ -n "${RF:-}" ] && ROOF_CMD+=(--realistic-fraction "$RF")
[ "$AL_VALUE" != "-" ] && ROOF_CMD+=(--accept-len "$AL_VALUE")
ROOF_CMD+=(--accept-len-source "$AL_SOURCE" --accept-len-evidence "$AL_EVIDENCE")

# 산출은 **짝으로 원자적 교체**한다(2026-09-14 리뷰 정정). 종전 `> "$ROOF"` 는 하위 스크립트가 돌기 전에
#   파일을 비웠다 — 제자리 재판정이 exit 2(무효 `--accept-len` 등)로 끝나면 정본 sweep 의 roofline.json 은
#   0 바이트, verdict.json 은 옛 판정으로 남아 짝이 찢어졌다(broad_search·render_report 가 둘을 함께 읽는다).
#   그래서 둘 다 임시 자리에 쓰고, 둘 다 성공했을 때만 교체한다. 판정기는 임시 roofline 을 읽는다.
ROOF_TMP="$ROOF.partial.$$"
OUT_TMP="$OUT.partial.$$"

VERDICT_CMD=(python3 "$SDIR/verdict_rule.py" --measured "$MEASURED" --roofline "$ROOF_TMP"
             --authority "$AUTHORITY" --spec-declared "$SPEC_DECLARED"
             "${PASS_ARGS[@]+"${PASS_ARGS[@]}"}")
[ "$SPEC" = 1 ] && VERDICT_CMD+=(--spec-supported)

# 실제로 넘긴 argv 를 남긴다 — 어느 권한으로 잰 판정인지는 산출물의 rubric.authority 가 밝히지만,
# **무엇을 넘기지 않았는지**는 argv 만이 말한다(플래그 누락이 이 결함의 원인이었다).
echo "[judge_bench] authority=$AUTHORITY topo=$TOPO level=$LEVEL spec_declared=$SPEC_DECLARED accept_len=$AL_VALUE source=$AL_SOURCE"
echo "[judge_bench] roofline: ${ROOF_CMD[*]}"
echo "[judge_bench] verdict : ${VERDICT_CMD[*]}"
if [ "$DRYRUN" = 1 ]; then echo "[judge_bench] DRY-RUN — 실행하지 않는다"; exit 0; fi
[ -z "$OUTDIR" ] || mkdir -p "$OUTDIR"   # dry-run 은 자리조차 만들지 않는다

trap 'rm -f "$ROOF_TMP" "$OUT_TMP"' EXIT
"${ROOF_CMD[@]}" > "$ROOF_TMP"
"${VERDICT_CMD[@]}" > "$OUT_TMP"
mv -f "$ROOF_TMP" "$ROOF"
mv -f "$OUT_TMP" "$OUT"
trap - EXIT
echo "[judge_bench] DONE — ROOFLINE=$ROOF VERDICT=$OUT"
python3 - "$OUT" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1], encoding="utf-8"))
rub = doc.get("rubric") or {}
print("[judge_bench] verdict=%s authority=%s source=%s floor=%s reason_code=%s"
      % (doc.get("verdict"), rub.get("authority"), rub.get("source"), rub.get("floor"),
         doc.get("reason_code")))
PY

# ── 인증서 자동 발행 (2026-09-08 · plan_26090813 §4.4 · 사용자 결정 D18) ─────────────────
# 왜 여기인가: `publish_benchmark_record.py` 는 저장소 안 **호출자가 0** 이었다. 발행기가 있는데
# 부르는 손이 없으면 인증서는 사람이 기억해야 나오고, 기억은 캠페인을 못 넘긴다. 판정을 낸 바로
# 이 자리가 발행 조건을 아는 유일한 자리다(verdict 와 authority 가 둘 다 여기 있다).
#
# **명시 권한(explicit)의 PASS 만** 자동이다. 탐색(explore)은 사용자가 목표를 안 준 상태의 측정이라
# "검증된 한계" 를 주장할 근거가 없다 — 리포트와 sweep map 으로 남긴다. 권한 모델의 정본은
# adversarial-benchmark SKILL.md §2 이며 여기서 두 번째 답을 만들지 않는다.
_V="$(python3 -c "import json,sys;print(json.load(open(sys.argv[1],encoding='utf-8')).get('verdict') or '')" "$OUT")"
_A="$(python3 -c "import json,sys;print(((json.load(open(sys.argv[1],encoding='utf-8')).get('rubric') or {}).get('authority')) or '')" "$OUT")"
if [ "$NO_PUBLISH" = 1 ]; then
  # 억제도 결과다 — 로그에 남겨야 "왜 인증서가 없지?" 가 조용한 누락과 구분된다.
  echo "[judge_bench] 인증서 발행 억제 — verdict=$_V authority=$_A (--no-publish${OUTDIR:+ · --out-dir=$OUTDIR 는 정본 판정 자리가 아니다})"
elif [ "$_V" = "PASS" ] && [ "$_A" = "explicit" ]; then
  echo "[judge_bench] 인증서 자동 발행 — verdict=PASS · authority=explicit"
  python3 "$SDIR/publish_benchmark_record.py" --sweep-index "$INDEX" --verdict-json "$OUT" \
    || echo "[judge_bench] ⚠ 인증서 발행 실패 — 판정은 남았고 인증서만 없다(위 사유 참조)" >&2
else
  # 음성정직: "발행 안 함" 은 결손이 아니라 판정 결과다. 그 사실이 로그에 남아야 나중에
  # "왜 인증서가 없지?" 가 조용한 누락과 구분된다.
  echo "[judge_bench] 인증서 미발행 — verdict=$_V authority=$_A (자동 발행은 explicit ∧ PASS 뿐)"
fi
