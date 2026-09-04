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
# 사용: judge_bench.sh <config_name> --authority weak|explicit|explore
#         [--topology single|multi] [--sweep-dir DIR] [--level N]
#         [--target-tps X] [--reference-tps E] [--e-search hit|empty|no]
#         [--tolerance T] [--realistic-fraction F] [--accept-len L] [--spec-supported]
#         [--node-vram-gib "a,b"] [--out FILE] [--dry-run]
# 산출: <sweep_dir>/roofline.json · <sweep_dir>/verdict.json (또는 --out)
# 종료: 0=판정 산출 · 2=인자/전제 오류 · 3=판정점 측정치 부재 · 그 외=하위 스크립트 rc 전달
set -euo pipefail

CONFIG="${1:?config_name 필요}"; shift || true
AUTHORITY=""; TOPO=""; SWEEPDIR=""; LEVEL=1; OUT=""; DRYRUN=0; CHECK_ARGS=0
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
[ -n "$OUT" ] || OUT="$SWEEPDIR/verdict.json"
ROOF="$SWEEPDIR/roofline.json"
MEASURED="$SWEEPDIR/level_$(printf '%02d' "$LEVEL")/measured.json"

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
tp = meta.get("tp")
print(meta.get("model_path") or "NA",
      tp if isinstance(tp, int) and not isinstance(tp, bool) and tp > 0 else "-")
PY
)
if [ "$MODEL_PATH" = "NA" ]; then
  echo "[judge_bench] ERROR sweep_index.meta.model_path 가 NA — 루프라인 입력이 성립하지 않는다" >&2; exit 2
fi
NAS_ROOT="$(sed -n 's/^[[:space:]]*nas_model_path:[[:space:]]*"\{0,1\}\([^"#]*\)"\{0,1\}.*/\1/p' \
             "$MANIFEST" 2>/dev/null | head -1 | sed 's/[[:space:]]*$//')"

ROOF_CMD=(python3 "$SDIR/roofline.py" --model-path "$MODEL_PATH" --json)
[ "$TP" != "-" ] && ROOF_CMD+=(--tp "$TP")
[ -f "$MANIFEST" ] && ROOF_CMD+=(--manifest "$MANIFEST")
[ -n "$NAS_ROOT" ] && ROOF_CMD+=(--nas-root "$NAS_ROOT")
[ -n "${RF:-}" ] && ROOF_CMD+=(--realistic-fraction "$RF")
[ -n "${AL:-}" ] && ROOF_CMD+=(--accept-len "$AL")

VERDICT_CMD=(python3 "$SDIR/verdict_rule.py" --measured "$MEASURED" --roofline "$ROOF"
             --authority "$AUTHORITY" "${PASS_ARGS[@]+"${PASS_ARGS[@]}"}")
[ "$SPEC" = 1 ] && VERDICT_CMD+=(--spec-supported)

# 실제로 넘긴 argv 를 남긴다 — 어느 권한으로 잰 판정인지는 산출물의 rubric.authority 가 밝히지만,
# **무엇을 넘기지 않았는지**는 argv 만이 말한다(플래그 누락이 이 결함의 원인이었다).
echo "[judge_bench] authority=$AUTHORITY topo=$TOPO level=$LEVEL"
echo "[judge_bench] roofline: ${ROOF_CMD[*]}"
echo "[judge_bench] verdict : ${VERDICT_CMD[*]}"
if [ "$DRYRUN" = 1 ]; then echo "[judge_bench] DRY-RUN — 실행하지 않는다"; exit 0; fi

"${ROOF_CMD[@]}" > "$ROOF"
"${VERDICT_CMD[@]}" > "$OUT"
echo "[judge_bench] DONE — ROOFLINE=$ROOF VERDICT=$OUT"
python3 - "$OUT" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1], encoding="utf-8"))
rub = doc.get("rubric") or {}
print("[judge_bench] verdict=%s authority=%s source=%s floor=%s"
      % (doc.get("verdict"), rub.get("authority"), rub.get("source"), rub.get("floor")))
PY
