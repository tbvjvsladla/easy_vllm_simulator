#!/usr/bin/env bash
# sweep_bench.sh — client-load 부하 스윕 (adversarial-benchmark full 모드 · report/인증서 입력 생성)
#
# 편지 패턴 B.4: 단일 running serve 에 **client-load 만** 변화(동시성 레벨 스윕) → 부하별 곡선.
#   **reload 0** — 벤치마커 "기동 안 함" 불변식 보존(config-space=batch×maxlen reload 는 Max 모드/explorer 소관).
#   판정점(동시성=1)을 스윕이 포함 → verdict 는 그 레벨을 재사용(재측정 0).
#   적응 상한 클램프 + **절삭 로그**(silent truncation 금지): 레벨 실패 시 그 레벨 절삭·기록하고 상위 중단.
# 각 레벨 = run_bench.sh 메커니즘 재사용(같은 serve·컨테이너·`--max-concurrency L`·request-rate inf).
#   run_bench 가 Flag/A2A 게이트·health precheck·envfile 해소를 수행 → 전이적 게이트 보존.
# 비용 규율(편지 B.5): 이 스윕은 재탐색 루프 내부가 아니라 **full 런 종결 시 1회**만 호출한다.
#
# 사용: sweep_bench.sh <config_name> [--topology single|multi] [--levels 1,2,4,8,16] [--backend openai-chat|openai]
#        [--input-len N] [--output-len N] [--num-prompts N] [--warmups N] [--vllm-version X] [--dry-run]
#        [--reassemble-only]
#
# --reassemble-only: **측정하지 않고** 기존 SWEEPDIR 의 raw(level_NN/measured.json · lite raw ·
#   truncation.log)에서 sweep_index.json 만 다시 조립한다. 라벨/파생키 계약이 바뀌었을 때
#   (예: 2026-08-15 model 강한키 파생 교정 · 2026-08-23 quantization/kv_cache_dtype 실측 승격)
#   **재측정 없이** 산출물을 정합화하는 유일한 정식 경로다 —
#   대안은 인증서 수기 편집(=증거 위조)이거나 재측정(=측정치가 아니라 라벨 문제인데 비용 지불)뿐이다.
#   측정시각(`generated_utc`)·실측 image_tag 는 **기존 index 에서 승계**한다(측정이 안 바뀌었으니
#   측정시각도 안 바뀐다). 기존 index 가 없으면 시각을 날조하는 대신 fail-closed 로 멈춘다.
# 산출: output/<topo>/benchlog/sweep_<config>/{level_NN/{bench_<config>.json,engine_<config>.log,measured.json},
#        truncation.log, sweep_index.json}  ← render_report.py·publish_benchmark_record.py 가 소비.
# 종료: 0=성공(레벨 ≥1 완료) · 2=인자/전제 오류 · 3=판정점(레벨1) 측정 불가(serve 미가동/게이트 등).
set -euo pipefail

CONFIG="${1:?config_name 필요}"; shift || true
TOPO=""; LEVELS="1,2,4,8,16"; ILEN=1024; OLEN=256; NPROMPTS=16; WARMUPS=2; VLLM_VER=""; DRYRUN=0; REASSEMBLE=0
# ★ 2026-09-01 신설 — run_bench.sh 의 --backend 를 레벨마다 그대로 전달한다.
#   전달하지 않으면 스윕 전 레벨이 openai-chat 로 돌아, harmony 계열(gpt-oss)에서 `--ignore-eos` 가
#   무력해져 **모든 레벨의 TPOT 이 동시에 왜곡**된다(run_bench.sh 의 BACKEND 주석 참조).
#   판정점(동시성=1)을 스윕이 포함하므로 그 왜곡은 곧 verdict 왜곡이다.
# ★ 2026-09-07: 여기서도 **기본값을 없앤다**(plan_26090715 §5 ⑤-①). 하류 run_bench 가 기본값을
#   버렸는데 이쪽이 들고 있으면 그 버림이 무효가 된다 — 기본값은 마지막 한 자리만 남아도 이긴다.
BACKEND=""
# ★ 2026-09-04(CP5 · plan_26090415 §3.1·§3.7) — 모드별 측정 도구 선택.
#   lite 레그는 이 노브와 무관하게 **언제나 `vllm bench serve`** 다(`full = lite ∪ GuideLLM`).
#   두 레그를 이질적으로 유지하는 것이 설계이며, 그 이질성이 실결함 2건을 잡았다(2026-09-01·09-03).
#   `--tool guidellm` 은 **레벨 측정만** 옮긴다.
TOOL="vllm"
BENCH_BUDGET_MIB=""
# 오류 허용치는 **선언에서만** 온다(기본 빈값 = 파서 기본 0 = 엄격).
MAX_ERROR_RATE=""
while [ $# -gt 0 ]; do case "$1" in
  --topology) TOPO="$2"; shift 2;;
  --tool) TOOL="$2"; shift 2;;
  --bench-budget-mib) BENCH_BUDGET_MIB="$2"; shift 2;;
  --max-error-rate) MAX_ERROR_RATE="$2"; shift 2;;
  --backend) BACKEND="$2"; shift 2;;
  --reassemble-only) REASSEMBLE=1; shift;;
  --levels) LEVELS="$2"; shift 2;;
  --input-len) ILEN="$2"; shift 2;;
  --output-len) OLEN="$2"; shift 2;;
  --num-prompts) NPROMPTS="$2"; shift 2;;
  --warmups) WARMUPS="$2"; shift 2;;
  --vllm-version) VLLM_VER="$2"; shift 2;;
  --dry-run) DRYRUN=1; shift;;
  *) echo "[sweep_bench] 알 수 없는 인자: $1" >&2; exit 2;;
esac; done

# 측정 엔드포인트 미선언을 **여기서** 친다 — 뒤로 미루면 lite 레그를 다 돌고 나서야 드러나고,
# 그때는 이미 잘못된 포맷으로 잰 판정점이 손에 있다(2026-09-07 · 유예 결함 ①).
case "$BACKEND" in
  openai|openai-chat) ;;
  "") echo "[sweep_bench] ERROR --backend 는 필수다(기본값 없음 · 2026-09-07)." >&2
      echo "  요청 포맷은 TPOT 을 바꾸는 1급 측정 축이고, 스윕은 그 포맷으로 **전 레벨**을 잰다." >&2
      echo "  harmony 계열(gpt-oss)은 --backend openai (완결 엔드포인트)." >&2
      exit 2 ;;
  *) echo "[sweep_bench] 알 수 없는 --backend: $BACKEND (openai|openai-chat)" >&2; exit 2 ;;
esac

case "$TOOL" in
  vllm) ;;
  guidellm)
    # 예산 미선언을 여기서 친다 — 뒤로 미루면 lite 레그와 레벨 1 을 다 돌고 나서야 드러난다.
    case "$BENCH_BUDGET_MIB" in
      ''|*[!0-9]*) echo "[sweep_bench] ERROR --tool guidellm 에는 --bench-budget-mib <양의 정수> 가 필수다(기본값 없음 · §4.8)" >&2; exit 2;;
    esac
    [ "$BENCH_BUDGET_MIB" -gt 0 ] || { echo "[sweep_bench] ERROR --bench-budget-mib 는 양수여야 한다" >&2; exit 2; }
    ;;
  *) echo "[sweep_bench] 알 수 없는 --tool: $TOOL (vllm|guidellm)" >&2; exit 2;;
esac

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "$SDIR" rev-parse --show-toplevel 2>/dev/null || pwd)"
if [ -z "$TOPO" ]; then
  BR="$(git -C "$REPO" rev-parse --abbrev-ref HEAD 2>/dev/null || echo)"
  case "$BR" in multi-node) TOPO=multi;; single-node) TOPO=single;; *) TOPO=single;; esac
fi

# 레벨 파싱: 정렬·중복제거 + 판정점(1) 강제 포함(verdict 재사용 보장).
IFS=',' read -r -a _lv <<< "$LEVELS"
declare -A _seen; SORTED=()
for v in 1 "${_lv[@]}"; do
  v="$(echo "$v" | tr -d '[:space:]')"; [ -z "$v" ] && continue
  case "$v" in (*[!0-9]*) echo "[sweep_bench] 레벨은 정수만: '$v'" >&2; exit 2;; esac
  [ -n "${_seen[$v]:-}" ] && continue; _seen[$v]=1; SORTED+=("$v")
done
# 오름차순 정렬(적응 클램프 = 낮은 레벨부터, 실패 시 상위 중단)
IFS=$'\n' SORTED=($(printf '%s\n' "${SORTED[@]}" | sort -n)); unset IFS

CFGYAML="$REPO/output/$TOPO/configs/${CONFIG}.yaml"
EF="$REPO/output/$TOPO/envs/.env.${CONFIG}"
MANIFEST="$REPO/output/$TOPO/manifest.yaml"
SWEEPDIR="$REPO/output/$TOPO/benchlog/sweep_${CONFIG}"

echo "[sweep_bench] config=$CONFIG topo=$TOPO levels=[${SORTED[*]}] in=$ILEN out=$OLEN n=$NPROMPTS warmup=$WARMUPS tool=$TOOL${BENCH_BUDGET_MIB:+ bench_budget=${BENCH_BUDGET_MIB}MiB}"
echo "[sweep_bench] sweepdir=$SWEEPDIR (판정점=동시성1 재사용)"
if [ "$DRYRUN" = "1" ]; then
  echo "[sweep_bench] DRY-RUN — 레벨별 실행 계획:"
  for L in "${SORTED[@]}"; do
    echo "  level $L → run_bench.sh $CONFIG --topology $TOPO --concurrency $L --tool $TOOL${BENCH_BUDGET_MIB:+ --bench-budget-mib $BENCH_BUDGET_MIB} --out-dir $SWEEPDIR/level_$(printf '%02d' "$L")"
  done
  echo "[sweep_bench] DRY-RUN 종료(실제 벤치·assemble 생략)"
  exit 0
fi

if [ "$REASSEMBLE" = "1" ]; then
  [ -d "$SWEEPDIR" ] || { echo "[sweep_bench] --reassemble-only: SWEEPDIR 부재 — $SWEEPDIR" >&2; exit 2; }
  [ -s "$SWEEPDIR/sweep_index.json" ] || {
    echo "[sweep_bench] --reassemble-only: 기존 sweep_index.json 부재 — 측정시각을 날조하지 않는다(중단)" >&2; exit 2; }
else
  mkdir -p "$SWEEPDIR"
fi
TRUNCLOG="$SWEEPDIR/truncation.log"
[ "$REASSEMBLE" = "1" ] || : > "$TRUNCLOG"   # 재조립은 절삭 기록을 승계한다(지우면 그 사실이 사라진다)

if [ "$REASSEMBLE" = "1" ]; then
  # 완료 레벨을 **디스크에서** 복원한다. 판정 기준은 최초 루프와 동일(measured.json 의
  # measurement_ok=true) — 기준을 새로 쓰면 두 벌이 되어 어긋난다.
  COMPLETED=()
  for _d in "$SWEEPDIR"/level_*; do
    [ -d "$_d" ] || continue
    _m="$_d/measured.json"; [ -s "$_m" ] || continue
    python3 -c "import json,sys; d=json.load(open(sys.argv[1])); sys.exit(0 if d.get('measurement_ok') else 1)" "$_m" \
      || continue
    COMPLETED+=("$(basename "$_d" | sed 's/^level_0*//')")
  done
  if [ "${#COMPLETED[@]}" -eq 0 ]; then
    echo "[sweep_bench] --reassemble-only: 복원 가능한 레벨 0개(measurement_ok) — 중단" >&2; exit 3
  fi
  IFS=$'\n' COMPLETED=($(printf '%s\n' "${COMPLETED[@]}" | sort -n)); unset IFS
  case " ${COMPLETED[*]} " in *" 1 "*) :;; *) echo "[sweep_bench] --reassemble-only: 판정점(레벨1) 부재 — 중단" >&2; exit 3;; esac
  LITE_RAW="$SWEEPDIR/lite_raw_${CONFIG}.json"
  [ -s "$LITE_RAW" ] || { echo "[sweep_bench] ⚠ --reassemble-only: lite raw 부재 — lite 블록 결손 승계" | tee -a "$TRUNCLOG"; LITE_RAW=""; }
  IMAGE_TAG_ACTUAL="NA"   # 재조립은 docker 를 건드리지 않는다 — 실측값은 기존 index 에서 승계(아래 PY)
  IMAGE_DIGEST_ACTUAL="NA"  # 동상(태그와 같은 승계 규율)
  echo "[sweep_bench] --reassemble-only: 측정 0회 · 복원 레벨 [${COMPLETED[*]}] · 측정시각 승계"
else

# ── full ⊇ lite 불변식 (2026-07-31 사용자 지시) ────────────────────────────────
# full 은 lite 의 **상위집합**이어야 한다. 종전엔 교집합이었다 — lite 만 cold TTFT·시스템 RAM·
# per-node nvidia-smi 를 재고, full 만 스윕·루프라인·verdict 를 냈다. 그러면 모드가 다른 두 모델의
# 지표 열(column) 집합이 달라져 **조건별 비교가 성립하지 않는다**(사용자 지적).
#
# 병렬 목록을 손으로 맞추는 방식은 쓰지 않는다 — 같은 날 파서 두 벌(D4↔D6)이 정확히 그 방식으로
# 어긋났다. 대신 **full 이 lite 를 실제로 실행해서 포함**한다. 포함관계가 구조로 보장되고,
# lite 에 지표가 추가되면 full 이 자동으로 따라온다(동기화 대상이 애초에 없다).
#
# 순서: lite 를 **먼저**. lite 의 cold TTFT 는 warmup 0 측정이라 스윕이 엔진을 데운 뒤에 재면
# 더 이상 cold 가 아니다.
echo "[sweep_bench] ── lite 포함 실행(full ⊇ lite 불변식) ──"
LITE_RAW="$SWEEPDIR/lite_raw_${CONFIG}.json"
# ★ 2026-09-03: `--backend "$BACKEND"` 가 **이 호출에만 빠져 있었다**(레벨 호출에는 있다).
#   2026-09-01 에 harmony 왜곡 대응으로 세 파일(run_bench·sweep_bench·lite_bench)에 노브를 신설했는데
#   sweep→lite 호출부 하나가 전달을 빠뜨렸다. 그래서 full 런의 lite 열은 **언제나 openai-chat** 으로
#   재졌고, gpt-oss 에서 gen_tps 11.83 vs 같은 런의 full decode 34.42 (2.9배)가 나왔다 —
#   2026-09-01 에 인증서까지 갔던 그 왜곡과 **같은 뿌리**다. 노브를 만든 것과 그것이 도는 것은 다르다.
if bash "$SDIR/lite_bench.sh" "$CONFIG" --topology "$TOPO" --backend "$BACKEND" --out-dir "$SWEEPDIR" >/dev/null 2>&1 \
   && [ -s "$LITE_RAW" ]; then
  echo "[sweep_bench] lite ✓ → $LITE_RAW"
else
  # fail-soft: lite 실패가 full 전체를 죽이지는 않되 **침묵하지 않는다**(절삭 로그와 동일 규율).
  echo "[sweep_bench] ⚠ lite 수집 실패 — full 이 lite 를 포함하지 못했다(지표 열 결손)" | tee -a "$TRUNCLOG"
  echo "lite truncated: lite_bench.sh 실패 또는 raw 부재" >> "$TRUNCLOG"
  LITE_RAW=""
fi

COMPLETED=()
for L in "${SORTED[@]}"; do
  LDIR="$SWEEPDIR/level_$(printf '%02d' "$L")"; mkdir -p "$LDIR"
  echo "[sweep_bench] ── level 동시성=$L ──"
  RB_ARGS=("$CONFIG" --topology "$TOPO" --concurrency "$L"
           --input-len "$ILEN" --output-len "$OLEN" --num-prompts "$NPROMPTS"
           --warmups "$WARMUPS" --backend "$BACKEND" --out-dir "$LDIR" --tool "$TOOL")
  [ -n "$BENCH_BUDGET_MIB" ] && RB_ARGS+=(--bench-budget-mib "$BENCH_BUDGET_MIB")
  if bash "$SDIR/run_bench.sh" "${RB_ARGS[@]}"; then
    ELOG="$LDIR/engine_${CONFIG}.log"
    # 파서는 도구가 정한다 — 스키마가 다르므로 파일명도 다르고, 잘못된 파서가 조용히 빈 값을
    # 내는 일이 없게 한다.
    if [ "$TOOL" = "guidellm" ]; then
      BJSON="$LDIR/guidellm_${CONFIG}.json"
      # spec 축은 GuideLLM 이 보고하지 않는다. **같은 스윕의 lite 레그**에서 승계한다 —
      # 이것이 `full = lite ∪ GuideLLM` 이 값으로 갚아 주는 자리다. lite 가 절삭됐으면
      # 부재를 명시 선언한다(그 사실은 이미 truncation.log 에 남아 있다).
      if [ -n "$LITE_RAW" ] && [ -s "$LITE_RAW" ]; then
        PARSE_CMD=(python3 "$SDIR/parse_guidellm.py" --benchmarks-json "$BJSON"
                   --engine-log "$ELOG" --accept-len-src "$LITE_RAW")
      else
        PARSE_CMD=(python3 "$SDIR/parse_guidellm.py" --benchmarks-json "$BJSON"
                   --engine-log "$ELOG" --spec-axis-absent)
      fi
      [ -n "$MAX_ERROR_RATE" ] && PARSE_CMD+=(--max-error-rate "$MAX_ERROR_RATE")
      # 벤치 종료 시 서버 생존 관측(2026-09-07 · 유예 결함 ②). run_bench 가 벤치 직후 · teardown
      # 전에만 남길 수 있는 사실이며, 이것이 없으면 엔진 사망 중 잘린 SSE 가 도구 경계로 면제된다.
      _PH="$LDIR/post_health_${CONFIG}.json"
      [ -s "$_PH" ] && PARSE_CMD+=(--post-health-json "$_PH")
    else
      BJSON="$LDIR/bench_${CONFIG}.json"
      PARSE_CMD=(python3 "$SDIR/parse_bench.py" --bench-json "$BJSON" --engine-log "$ELOG")
    fi
    if "${PARSE_CMD[@]}" > "$LDIR/measured.json" 2>/dev/null \
       && python3 -c "import json,sys; d=json.load(open('$LDIR/measured.json')); sys.exit(0 if d.get('measurement_ok') else 1)"; then
      COMPLETED+=("$L"); echo "[sweep_bench] level $L ✓"
    else
      echo "[sweep_bench] level $L 측정 파싱 실패 → 절삭" | tee -a "$TRUNCLOG"
      echo "level $L truncated: parse/measurement_ok=false" >> "$TRUNCLOG"
      [ "$L" = "1" ] && { echo "[sweep_bench] 판정점(레벨1) 측정 불가 — 중단" >&2; exit 3; }
      break
    fi
  else
    RC=$?
    echo "level $L truncated: run_bench exit $RC (serve health-drop/게이트/bench 실패 — 상위 레벨 중단)" | tee -a "$TRUNCLOG"
    if [ "$L" = "1" ]; then
      echo "[sweep_bench] 판정점(레벨1) 측정 불가(run_bench exit $RC) — serve 미가동/게이트 확인" >&2; exit 3
    fi
    break   # 적응 클램프: 낮은 레벨 실패면 상위도 실패 → 중단
  fi
done

# ── 실제 측정 대상 이미지 캡처(2026-08-13 신설) ────────────────────────────────
# image_tag 는 여태 envfile 선언값만 적었다. 그런데 IMAGE_TAG 는 `docker compose` 호출 시
# 환경변수로 override 되는 것이 정상 경로이므로(변종·재빌드 트랙), **선언값과 실측이 갈린다**.
#   2026-08-13 X1 실측: envfile 은 `...-source`, 실제 컨테이너는 `...-source-canonical`.
#   그대로 두면 인증서가 **측정하지 않은 이미지**의 이름을 달고 발행되어 재현이 불가능해진다.
# 이 파일은 vLLM 버전에 대해 이미 같은 규율을 갖고 있다(선언값 대신 **실측을 강한키로 싣고**
# 선언값은 `*_declared` 로 나란히 남긴다). image_tag 에도 동형으로 채운다.
_CTR_RE='^(MASTER_)?CONTAINER_NAME='
[ "$TOPO" = "multi" ] && _CTR_RE='^MASTER_CONTAINER_NAME='
# tr 의 인자는 8진 이스케이프로 준다(\042=" \047=') — 셸 따옴표 중첩 회피.
_CTR_NAME="$( { grep -E "$_CTR_RE" "$EF" 2>/dev/null || true; } | tail -1 | cut -d= -f2- | tr -d '\042\047' )"
IMAGE_TAG_ACTUAL="NA"
# ★ 태그는 **가변 포인터**다 — 같은 문자열이 다른 이미지를 가리킬 수 있다(2026-09-04 실측:
#   메인 `easy-vllm:0.18.0-cu130-aarch64-wheel` = ca36fd12…, 서브 같은 태그 = c642de38…,
#   그 뒤 멀티 빌드가 메인 것을 또 덮었다). 인증서가 태그만 적으면 "어느 이미지로 쟀는가" 에
#   답하지 못한다. 컨테이너 이미지는 git 이 바이트를 들지 않고 추적 입력만으로 결정론 재생성도
#   보장되지 않으므로 **맹점층**이고(헌법 2문항 판정), 그 자리의 digest 는 중복이 아니라 유일한 증거다.
IMAGE_DIGEST_ACTUAL="NA"
if [ -n "$_CTR_NAME" ]; then
  IMAGE_TAG_ACTUAL="$(docker inspect "$_CTR_NAME" --format '{{.Config.Image}}' 2>/dev/null || echo NA)"
  [ -n "$IMAGE_TAG_ACTUAL" ] || IMAGE_TAG_ACTUAL="NA"
  # `.Image` = 컨테이너가 실제로 기동한 이미지의 content id(태그 재사용과 무관).
  IMAGE_DIGEST_ACTUAL="$(docker inspect "$_CTR_NAME" --format '{{.Image}}' 2>/dev/null || echo NA)"
  [ -n "$IMAGE_DIGEST_ACTUAL" ] || IMAGE_DIGEST_ACTUAL="NA"
fi

fi   # ── /REASSEMBLE 분기 끝(위 측정·캡처 전량은 재조립 모드에서 건너뛴다) ──

# ── sweep_index.json 조립 + meta 추출(결정론 · stdlib · fail-soft N/A) ──────────
CONFIG="$CONFIG" TOPO="$TOPO" CFGYAML="$CFGYAML" EF="$EF" MANIFEST="$MANIFEST" \
SWEEPDIR="$SWEEPDIR" VLLM_VER="$VLLM_VER" COMPLETED="${COMPLETED[*]:-}" ILEN="$ILEN" \
 IMAGE_TAG_ACTUAL="$IMAGE_TAG_ACTUAL" IMAGE_DIGEST_ACTUAL="$IMAGE_DIGEST_ACTUAL" REASSEMBLE="$REASSEMBLE" \
 LITE_RAW="$LITE_RAW" SDIR="$SDIR" python3 - <<'PY'
import json, os, re, glob, sys, datetime
sys.path.insert(0, os.environ["SDIR"])   # 벤치 명명 SSOT(같은 스킬 디렉터리 · 서브에도 배달됨)
from doc_naming import gpu_key as _gpu_key

cfg = os.environ["CONFIG"]; topo = os.environ["TOPO"]
sweepdir = os.environ["SWEEPDIR"]
completed = [int(x) for x in os.environ.get("COMPLETED", "").split() if x.strip()]

def read(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as f: return f.read()
    except OSError: return ""

def grep_yaml(text, key):
    # dash/underscore 양변형 허용 · '# 주석' 제거
    m = re.search(r"(?m)^\s*%s\s*:\s*([^\n#]+)" % re.escape(key), text)
    return m.group(1).strip().strip('"').strip("'") if m else None

def grep_env(text, key):
    m = re.search(r"(?m)^\s*%s\s*=\s*([^\n#]*)" % re.escape(key), text)
    return (m.group(1).strip().strip('"').strip("'") or None) if m else None

cfgtext = read(os.environ["CFGYAML"]); envtext = read(os.environ["EF"]); mftext = read(os.environ["MANIFEST"])

# ── 재조립 모드: 측정에서 나온 값(측정시각·실측 image_tag)은 기존 index 에서 **승계**한다 ──────
# 라벨 계약이 바뀌어 다시 조립하는 것이므로 파생키는 새로 계산하지만, **측정이 만든 사실**은
# 새로 만들지 않는다(그러면 재조립이 조용히 재측정 행세를 한다).
reassemble = os.environ.get("REASSEMBLE") == "1"
prior = {}
if reassemble:
    try:
        with open(os.path.join(sweepdir, "sweep_index.json"), encoding="utf-8") as f:
            prior = json.load(f)
    except (OSError, ValueError):
        prior = {}
    if not isinstance(prior, dict) or not prior.get("generated_utc"):
        raise SystemExit("[sweep_bench] --reassemble-only: 기존 index 의 generated_utc 부재 — "
                         "측정시각 승계 불가(날조 금지) → 중단")

# vLLM 버전: --vllm-version > env > **engine log(런타임 실측·권위)** > config yaml 주석 > NA
#
# engine log 를 config 주석보다 **위**에 둔다: 주석은 렌더 시점의 의도이고 engine log 는 실제로
# 돈 바이너리의 자기보고다. 2026-07-31 에 `simulate --force` 가 env 를 재생성하며 IMAGE_TAG 를
# 날려 compose 가 기본 이미지(0.24.0)로 조용히 폴백했는데, health 200 이라 아무 데서도 안 걸렸고
# vllm_version 이 "NA" 로 기록돼 **어느 버전을 쟀는지 알 수 없는 report** 가 나왔다.
# 인증서 강한키가 NA 면 carry-forward staleness 판정이 통째로 무력화된다.
# 강한키 vllm_version = **이미지 라인**(선례 정합: image easy-vllm:0.25.1-… ↔ 인증서 0.25.1 ↔
#   hint/0.25.1/…). 엔진은 소스빌드라 항상 dev 문자열(0.26.1.dev0+g…)을 자기보고하므로 그걸 강한키로
#   쓰면 인증서·hint 태그 세그먼트와 영영 불일치한다.
# 엔진 자기보고는 버리지 않고 **vllm_build(소프트 지문)** 으로 나란히 기록한다 — 2026-07-31 에
#   IMAGE_TAG 소실로 compose 가 0.24.0 으로 조용히 폴백했는데 health 200 이라 아무 데서도 안 걸렸다.
#   두 출처를 다 남기면 사람이 report 에서 갈림을 읽는다(침묵 치환 금지). 자동 판정은 두지 않는다 —
#   dev 빌드는 마이너 +1 이 정상이라 규칙이 예외로만 이루어져 위양성·위음성을 함께 낳았다.
_img = grep_env(envtext, "IMAGE_TAG") or ""
_img_actual = (os.environ.get("IMAGE_TAG_ACTUAL") or "NA").strip() or "NA"
_img_digest = (os.environ.get("IMAGE_DIGEST_ACTUAL") or "NA").strip() or "NA"
if reassemble:
    # docker 를 다시 묻지 않는다 — 그 사이 컨테이너가 바뀌었으면 **측정하지 않은 이미지** 이름이
    # 실린다. 이전 조립이 실측으로 잡아둔 값만 승계하고, 실측이 아니었으면 NA 로 둔다.
    _pm = prior.get("meta") or {}
    _img_actual = (_pm.get("image_tag") or "NA") if str(_pm.get("image_tag_source", "")).startswith("measured") else "NA"
    # digest 도 같은 규율로 승계한다 — 재조립은 측정하지 않으므로 docker 에 다시 묻지 않는다.
    _img_digest = ((_pm.get("image_digest") or "NA")
                   if str(_pm.get("image_digest_source", "")).startswith("measured") else "NA")
_elog = ""
for _lvl in sorted(completed):
    _p = os.path.join(sweepdir, "level_%02d" % _lvl, "engine_%s.log" % cfg)
    if os.path.isfile(_p):
        _elog = read(_p); break
# "Initializing a V1 LLM engine (v0.26.1.dev0+g568afb3a1.d20260730)"
_m = re.search(r"\bv([0-9]+\.[0-9]+\.[0-9]+(?:\.dev[0-9]*)?(?:\+g[0-9a-f]+)?[^\s)]*)", _elog)
vllm_build = _m.group(1) if _m else "NA"
vllm = os.environ.get("VLLM_VER") or os.environ.get("EASY_VLLM_VERSION") or None
if not vllm:
    _mi = re.search(r"easy-vllm:([0-9]+\.[0-9]+\.[0-9]+)", _img)
    vllm = _mi.group(1) if _mi else None
if not vllm and vllm_build != "NA":
    _mb = re.match(r"([0-9]+\.[0-9]+\.[0-9]+)", vllm_build)
    vllm = _mb.group(1) if _mb else None
if not vllm:
    _mc = re.search(r"vLLM[\s]*([0-9]+\.[0-9]+\.[0-9]+)", cfgtext)
    vllm = _mc.group(1) if _mc else "NA"
# gpu_model: manifest 단일 권위(2026-09-05 · plan_26090516 §7.3 · audit_26090515 B9).
#   종전에는 "서브 노드는 manifest 를 갖지 않는다"(D10) 는 전제로 Agent_Card.json(node_identity.gpu_model) 폴백을 두었고,
#   그 폴백은 렌더러의 `<n>x-<arch>`/`unknown-gpu` 기본값과 이어져 위조 정체성이 인증서 **강한 키** `gpu` 로
#   흘러갈 수 있었다. 이제 서브도 자기 manifest 를 갖는다(메인 terraforming 이 실측·발급 · self_role: sub).
#   Agent_Card v2 는 A2A 평면 계약(능력·엔드포인트·서명)이라 HW 필드가 없다. gpu_model 이 없으면 인증서를
#   "NA" 강한 키로 발행하지 않고 여기서 멈춘다(fail-loud).
gpu_model = grep_yaml(mftext, "gpu_model")
if not gpu_model:
    sys.stderr.write("[sweep_bench] FAIL: manifest(%s) 에 gpu_model 이 없다 — 인증서 강한 키 gpu 를 NA 로 발행하지 않는다. "
                     "terraforming_node 스캔(서브면 scan_node.py --emit-sub-manifest)으로 채워라.\n" % os.environ.get("MANIFEST", "?"))
    sys.exit(2)
gpu_key = _gpu_key(gpu_model)  # 정규화 규칙은 doc_naming 한 곳(인라인 사본 제거 · 감사 D-1)
# tp: config tensor-parallel-size > manifest 파생 > 1
# **topology=single 이면 노드 배수 1 고정** — single manifest 의 nodes[role=sub] 는 sub-control
# 피어이지 텐서 워커가 아니다. 이 검사가 없어서 single 스윕이 tp=2 를 인증서 **강한키**에 박았고,
# 그러면 carry-forward 재검증이 영영 불일치한다(2026-07-31 발견 · δ1-1 동일 계열 7번째 사이트).
tp = grep_yaml(cfgtext, "tensor-parallel-size")
if tp is None:
    gpn = grep_yaml(mftext, "gpus_per_node")
    mtopo = grep_yaml(mftext, "topology") or topo
    try:
        if str(mtopo).strip().strip('"').startswith("single"):
            tp = str(max(1, int(gpn))) if gpn else "1"
        else:
            n_nodes = len(re.findall(r"(?m)^\s*-\s*role:\s*", mftext))
            tp = str(max(1, n_nodes) * int(gpn)) if (n_nodes and gpn) else "1"
    except (TypeError, ValueError): tp = "1"

# ── model 강한키 = **체크포인트 경로에서 파생**(2026-08-15 교정 · IDENTITY_MISMATCH:model) ────
# 종전 규칙은 `SERVING_MODEL_NAME or config_name` 이었다. 그 둘은 **운영 조합명**이다 —
# 한 모델을 여러 사다리 칸으로 가르려면 한 엔드포인트 평면에서 이름이 갈려야 하므로
# `ds4f0731-x2-sm12x` 처럼 (모델×토폴로지×이미지변종) 조합을 이름에 담게 된다. 그러면 그 조합명이
# 인증서의 **모델 축**에 실리고, 두 요구가 동시에 만족될 수 없게 된다:
#   hint_tag.py      : identity.model == 태그의 <model> 세그먼트(= 모델 슬러그)
#   completion_gate  : 인증서.model == identity.model
# 2026-08-15 이 충돌로 HINT 발행이 실제로 막혔다(`testlog_26081519` §9 B3). 0.26.1 때는
# SERVING_MODEL_NAME 이 마침 모델 슬러그였기에 잠복했을 뿐이다.
#   ⇒ 모델 축은 **체크포인트 디렉터리 basename** 에서 파생한다(손으로 적지 않는다 — 파생 가능한 값을
#     손으로 적는 것이 하드코딩 안티패턴이다). **소문자화**하는 이유는 대소문자만 다른 키가
#     `정확일치` 게이트를 조용히 깨뜨리는 사고를 구조적으로 없애기 위해서다.
#   ⇒ 조합명은 버리지 않고 **serving_config** 라는 자기 축으로 분리한다(축이 둘이면 둘 다 적는다).
#   ⚠ carry-forward 영향: 2026-08-15 이전 인증서 중 모델명이 대문자를 포함한 것들
#     (`gemma-4-E2B-it`·`LFM2.5-2.6B`·`Qwen3-4B`)은 이 규칙 뒤 소문자 키로 발행되므로 옛 인증서와
#     **정확일치하지 않는다** — 실패 방향은 "일치 안 함 = 재측정"(fail-closed)이라 안전한 쪽이다.
_mpath = grep_yaml(cfgtext, "model") or ""
_mbase = os.path.basename(_mpath.rstrip("/")).strip()
_serving_cfg = grep_env(envtext, "SERVING_MODEL_NAME") or cfg
if _mbase:
    model_key, model_source = _mbase.lower(), "derived(model_path basename · lowercased)"
elif grep_env(envtext, "SERVING_MODEL_NAME"):
    model_key, model_source = grep_env(envtext, "SERVING_MODEL_NAME"), "fallback(serving_model_name)"
else:
    model_key, model_source = cfg, "fallback(config_name)"
if not _mbase:      # fail-loud 폴백 — 대체했다는 사실을 침묵시키지 않는다
    print("[sweep_bench] ⚠ config yaml 에 model 경로가 없어 강한키를 조합명으로 폴백했다 "
          "(model_source=%s) — 인증서 모델 축이 조합명이 된다" % model_source, file=sys.stderr)

# ── serve-plane 실측: quantization · kv_cache_dtype (2026-08-23 신설 · plan_26082322) ──────────
# 종전엔 두 값을 **config yaml 에서만** 읽었다. 그런데 이 둘은 serve-plane CLI(runner 의 EXTRA_ARGS →
# `--quantization` / `--kv-cache-dtype`)로도 들어온다 — 캠페인 축 F/H 가 정확히 그 경로다. 그러면
# yaml 에 줄이 없어 "NA" 가 찍히고, 사람은 "NA" 를 **"양자화 없음 · bf16 KV"** 로 읽는다.
#   2026-08-23 R7(1M·fp8 KV·fp8 가중치)이 실제로 그렇게 발행됐다 — 엔진은 그 순간 `quantization=fp8,
#   kv_cache_dtype=fp8` 을 자기보고하고 있었다. 이번엔 verdict=REFUTE 라 인증서가 안 나가 유출은
#   없었지만, PASS 였다면 "양자화 없이 이 성능"이라는 **거짓 계약**이 배포됐을 것이다.
#   ⇒ 같은 파일의 image_tag 가 이미 갖춘 규율(측정 > 선언 · 선언 병기 · 갈리면 mismatch)을 동형 복제한다.
#
# 권위 = **엔진 config 자기보고 라인 하나**로 고정한다(`Initializing a V1 LLM engine … with config: …`).
#   로그 전체를 긁으면 안 된다 — 같은 로그에 `[runner] KV_CACHE_DTYPE=…`(러너 에코)와
#   `FlashInfer resolved … kv_cache_dtype=torch.float8_e4m3fn`(백엔드 해석값)이 함께 산다.
#   2026-08-23 캠페인 러너의 6g 행이 정확히 전역 검색을 써서 `kv_cache_dtype=torch` 를 집었다(실증).
#
# 재조립(--reassemble-only)에서도 이 파싱은 **정당하다** — 그때 그 측정이 남긴 raw 로그를 다시 읽는
# 것이지 새로 재는 것이 아니다(바로 위 vllm_build 가 이미 같은 자격으로 _elog 를 재파싱한다).
# image_tag 를 승계로 처리한 이유와 대비된다: 저쪽은 docker 에 **다시 묻는** 행위였다.
_ecfg_m = re.search(r"Initializing a V1 LLM engine[^\n]*?with config:([^\n]*)", _elog)
_ecfg = _ecfg_m.group(1) if _ecfg_m else ""


def engine_cfg_val(key):
    """엔진 config 라인에서 `key=<값>` 을 하나 뽑는다. 라인/키가 없으면 None — 순수 파서의 None
    이며 대체값을 만들지 않는다(호출부가 어느 소스로 떨어질지 정한다).
    키 앞 경계를 요구하는 이유: 같은 라인에 `quantization_config=None` 이 나란히 있어서, 경계 없이
    찾으면 접두어가 겹치는 다른 키를 집을 수 있다."""
    if not _ecfg:
        return None
    m = re.search(r"(?:^|[\s,({\[])%s=([^,\s)\]}]+)" % re.escape(key), _ecfg)
    return m.group(1).strip().strip("'\"") if m else None


def _norm_measured(v):
    """엔진의 파이썬 repr 을 계약 토큰으로 정규화한다.
    **`None` 은 '측정된 기본값'이지 미상이 아니다** — 그래서 "NA" 가 아니라 `none` 으로 적는다
    (kv 쪽 엔진 기본 토큰 `auto` 와 동렬). 이 구분이 이 수정의 핵심이다: "측정 못 함"과 "기본값"을
    한 칸에 뭉개면 하류 독자가 후자를 전자로, 또는 그 반대로 읽는다.
    소문자화는 대소문자만 다른 키가 정확일치 게이트를 조용히 깨뜨리는 사고를 없애기 위해서다
    (model 강한키가 2026-08-15 에 같은 이유로 소문자화됐다)."""
    if v is None:
        return None
    return "none" if v.strip() == "None" else v.strip().lower()


def _measured_first(meas, decl):
    """(값, source, declared, mismatch) 4-튜플. image_tag 스탠자와 **동형**이다.
    mismatch 는 둘 다 알 때만 판정한다 — 모름을 일치로 위장하지 않는다(image_tag 와 같은 규율).
    ⚠ 선언 평면은 config yaml 하나만 본다. 축 F/H 는 실제로는 envfile(`QUANTIZATION`·
      `KV_CACHE_DTYPE`)로 선언되지만 그 변수명은 **모델 러너(Band3 트리플렛) 로컬 규약**이고
      `.claude/` 어디에도 계약이 없다 — 공유 하네스가 비계약 이름에 묶이면 다음 러너가 이름을 바꾸는
      순간 조용히 어긋난다. 그래서 교차검증은 포기하고("unknown") **값은 측정으로** 채운다."""
    m = _norm_measured(meas)
    d = decl.strip() if isinstance(decl, str) and decl.strip() else None
    if m is not None:
        value, source = m, "measured(engine log)"
    elif d is not None:
        value, source = d, "declared(config yaml)"
    else:
        value, source = "NA", "absent(both)"   # "NA" 는 오직 여기 — 양 소스 모두 부재
    mismatch = "unknown" if (m is None or d is None) else ("no" if m == d.lower() else "YES(measured=%s declared=%s)" % (m, d))
    return value, source, (d or "NA"), mismatch


_quant, _quant_src, _quant_decl, _quant_mm = _measured_first(
    engine_cfg_val("quantization"), grep_yaml(cfgtext, "quantization"))
_kvdt, _kvdt_src, _kvdt_decl, _kvdt_mm = _measured_first(
    engine_cfg_val("kv_cache_dtype"), grep_yaml(cfgtext, "kv-cache-dtype"))

# ── 커널 백엔드 축: 요청과 **실효**가 갈릴 수 있다 (2026-09-04 · plan_26090419) ────────────────
#   실측 계기: 트리플렛 러너가 `VLLM_ATTENTION_BACKEND=FLASHINFER` 를 export 하는데 엔진 로그는
#   `Using TRITON_ATTN attention backend out of potential backends: ['TRITON_ATTN']` 이었다.
#   후보 목록이 **한 개**라 요청이 조용히 무시된 것이다. 이 사실을 meta 가 싣지 않으면, 커널 축을
#   도는 캠페인이 "FLASHINFER 로 34.5 t/s" 라는 **거짓 좌표**를 지도에 남긴다 — 이 오퍼레이션이
#   막으려는 바로 그 오도(誤導)다.
#   `VLLM_ATTENTION_BACKEND` 는 Band3 로컬 규약이 아니라 **vLLM 업스트림이 소유한 이름**이므로
#   교차검증 대상으로 삼는다(envfile 의 QUANTIZATION 계열을 포기한 이유가 여기엔 해당하지 않는다).
_shtext = read(os.path.join(os.path.dirname(os.environ["CFGYAML"]), "%s.sh" % cfg))
_m = re.search(r"Using\s+(\S+)\s+attention backend", _elog)
_attn_meas = _m.group(1) if _m else None
_m = re.search(r"attention backend out of potential backends:\s*\[([^\]]*)\]", _elog)
_attn_cands = ([c.strip().strip("'\"") for c in _m.group(1).split(",") if c.strip()] if _m else None)
_m = re.search(r"^\s*export\s+VLLM_ATTENTION_BACKEND=(\S+)", _shtext, re.M)
_attn_decl_raw = _m.group(1) if _m else None
_attn, _attn_src, _attn_decl, _attn_mm = _measured_first(_attn_meas, _attn_decl_raw)

# MoE 백엔드도 측정 우선으로 올린다 — 종전에는 config yaml 선언만 봐서 실측 인증서가 "N/A" 였다.
# 실제 라인(0.19.1): `[mxfp4.py:352] Using 'MARLIN' Mxfp4 MoE backend.`
#   0.18.0 은 `[mxfp4.py:166] Using Marlin backend` 였다 — 따옴표 유무와 어순이 버전마다 다르므로
#   "Using <이름> … backend" 형태를 느슨하게 잡되 **이름만** 캡처한다. 못 잡으면 None 이고
#   `_measured_first` 가 선언 평면으로 떨어진다(합성 ✗).
_m = re.search(r"Using\s+'?([A-Za-z0-9_]+)'?\s+(?:\w+\s+)*?MoE backend", _elog) \
     or re.search(r"mxfp4\.py[^\]]*\]\s*Using\s+'?([A-Za-z0-9_]+)'?\s+backend", _elog)
_moe_meas = _m.group(1) if _m else None
_moe, _moe_src, _moe_decl, _moe_mm = _measured_first(_moe_meas, grep_yaml(cfgtext, "moe-backend"))

if any(x.startswith("YES") for x in (_quant_mm, _kvdt_mm, _attn_mm, _moe_mm)):
    print("[sweep_bench] ⚠ 선언↔실측 불일치 — quantization:%s · kv_cache_dtype:%s · "
          "attention_backend:%s · moe_backend:%s"
          % (_quant_mm, _kvdt_mm, _attn_mm, _moe_mm), file=sys.stderr)
# 후보가 한 개면 그 축은 **이 조합에서 선택지가 없다**. 실패가 아니라 축이 비어 있는 것이며,
# 그 사실이 지도에 실려야 다음 캠페인이 같은 셀을 다시 돌지 않는다.
if _attn_cands is not None and len(_attn_cands) <= 1:
    print("[sweep_bench] ⓘ 어텐션 축 무선택 — 후보 %s (요청 %s 는 실효 없음)"
          % (_attn_cands, _attn_decl_raw or "미선언"), file=sys.stderr)

# ── 측정 노드 출처 (2026-09-06 신설 · plan_26090616 ⑤) ──
# 인증서에 **어느 노드가 쟀는지**가 없었다. 그래서 메인과 서브가 같은 모델·같은 버전을 재면
# 파일명까지 동명이 되어 서로를 덮었고, hint 발행기는 그 인증서를 메인 것으로만 읽었다 —
# 서브가 완주해도 그 증거가 태그에 닿는 경로가 없었다(2026-09-05 실측).
# 축은 hint 태그의 노드 축과 **같은 어휘**를 쓴다(main|sub|cluster) — 두 자리가 다른 말을 쓰면
# 발행기가 대조하지 못한다. 멀티의 쌍은 하나의 측정 정체성이므로 `cluster` 다(불변식 A).
# ★ 2026-09-07 정직화(plan_26090715 §4.7 1차 · audit_26090708 §1.2). 종전은 `or "main"` 으로 부재를
#   메인으로 접고도 출처를 `derived(manifest.self_role=main)` 이라고 적었다 — **부재의 기본값을 파생이라고
#   거짓 표시**한 것이다(헌법 §결정론 규율: 모의 자체는 금지가 아니고 실측인 척하는 것이 금지다).
#   부재는 이제 `defaulted(self_role absent)` 로 스스로를 밝힌다. 노드축 2차 대조(차단)는 이 라벨이
#   선 뒤에 켠다 — 라벨 없이 차단부터 켜면 부재가 전부 위양성 차단이 된다.
_self_role = grep_yaml(mftext, "self_role")
if topo == "multi":
    measured_node, measured_node_source = "cluster", "derived(topology=multi — 쌍이 하나의 측정 정체성)"
elif _self_role == "sub":
    measured_node, measured_node_source = "sub", "derived(manifest.self_role=sub)"
elif _self_role:
    measured_node, measured_node_source = "main", "derived(manifest.self_role=%s)" % _self_role
else:
    measured_node, measured_node_source = "main", "defaulted(self_role absent)"

meta = {
    # 강한 일치 키
    "model": model_key,
    # 측정 노드 출처(강한 키 아님 — 출처 표시다. 헌법 §결정론 규율)
    "measured_node": measured_node,
    "measured_node_source": measured_node_source,
    "model_source": model_source,
    # 운영 조합명(모델 축이 아니다) — 같은 모델의 사다리 칸/이미지 변종을 가르는 축.
    "serving_config": _serving_cfg,
    "config_name": cfg,
    "gpu_model": gpu_model, "gpu_key": gpu_key, "vllm_version": vllm,
    "vllm_build": vllm_build,
    # ⚠ 강한 일치 키. 2026-08-23 이전엔 yaml 부재 = "NA" 였고, 그래서 **양자화 안 한 모델도
    # 양자화한 모델도 똑같이 "N/A"** 로 발행됐다. 이제 엔진 실측이 `none`(기본) / `fp8`(적용)을
    # 가른다. carry-forward 영향: 그 이전 인증서(quantization: N/A)와 정확일치하지 않는다 —
    # 실패 방향은 "일치 안 함 = 재측정"(fail-closed)이라 model 강한키 소문자화(2026-08-15) 때와
    # 같은 안전한 쪽이다. 작업 매니페스트 identity 의 `quant` 도 같은 토큰(`none`)을 써야 한다.
    "quantization": _quant,
    "quantization_source": _quant_src,
    "quantization_declared": _quant_decl,
    "quantization_mismatch": _quant_mm,
    "topology": topo, "tensor_parallel_size": tp,
    # 소프트 지문
    "driver_version": grep_yaml(mftext, "driver_version") or "NA",
    "cuda_version": grep_yaml(mftext, "cuda_version") or "NA",
    # 실측 우선(측정 > 선언). 선언값은 버리지 않고 `image_tag_declared` 로 나란히 남긴다 —
    # vllm_version/vllm_build 가 쓰는 규율과 동형이다(2026-08-13 신설, 위 캡처 스탠자 참조).
    "image_tag": _img_actual if _img_actual != "NA" else (grep_env(envtext, "IMAGE_TAG") or "NA"),
    "image_tag_source": "measured(docker inspect)" if _img_actual != "NA" else "declared(envfile)",
    "image_tag_declared": grep_env(envtext, "IMAGE_TAG") or "NA",
    # 태그가 가리키는 **내용**의 신원. 재조립 모드에서는 기존 index 에서 승계된다.
    "image_digest": _img_digest,
    "image_digest_source": "measured(docker inspect .Image)" if _img_digest != "NA" else "unavailable",
    "max_model_len": grep_yaml(cfgtext, "max-model-len") or "NA",
    "max_num_seqs": grep_yaml(cfgtext, "max-num-seqs") or "NA",
    "kv_cache_memory_bytes": grep_yaml(cfgtext, "kv-cache-memory-bytes") or "NA",
    # 소프트 지문이지만 같은 규율을 적용한다 — "NA" 를 bf16 으로 오독하는 것이 강한키 오독보다
    # 덜 위험하지도 않다(KV dtype 은 KV 용량·정확도 양쪽을 동시에 바꾼다).
    "kv_cache_dtype": _kvdt,
    "kv_cache_dtype_source": _kvdt_src,
    "kv_cache_dtype_declared": _kvdt_decl,
    "kv_cache_dtype_mismatch": _kvdt_mm,
    "gpu_memory_utilization": grep_yaml(cfgtext, "gpu-memory-utilization") or "NA",
    # ── PLE 상주/mmap 축 (2026-09-11 신설 · plan_26091108 R9) ─────────────────────────────
    #   ★ 이 축이 **증거 어디에도 없었다.** camp-26090918 은 PLE=resident 와 PLE=mmap 을 별개
    #     셀로 돌았는데, 두 인증서의 강한키·소프트키가 **완전히 동일**했다 — 이름이 충돌해
    #     hint 태그가 타임스탬프 접미사로 회피했고(`collide_suffix` 가 "축이 부족하다"고 경고한
    #     바로 그 신호), carry-forward 대조는 둘을 같은 레시피로 읽는다. 47.7GiB 가 상주하느냐
    #     NVMe 에서 오느냐는 예산·지연 양쪽을 바꾸는 축이고, 축이 증거에 없으면 그 셀은 재현 불가다.
    #   권위는 **serve 시점의 셀 env** 다(트리플렛 Band3). 부재는 stock 경로 = resident 이며,
    #   그 사실을 `defaulted` 로 스스로 밝힌다(부재를 파생이라 적지 않는다 — 2026-09-07 교훈).
    "ple_mode": ("mmap" if grep_env(envtext, "VLLM_PLE_MMAP") == "1" else "resident"),
    "ple_mode_source": ("declared(envfile VLLM_PLE_MMAP=1)"
                        if grep_env(envtext, "VLLM_PLE_MMAP") == "1"
                        else "defaulted(envfile VLLM_PLE_MMAP absent/0 = stock resident)"),
    "moe_backend": _moe,
    "moe_backend_source": _moe_src,
    "moe_backend_declared": _moe_decl,
    "moe_backend_mismatch": _moe_mm,
    # 어텐션 축 — 값·출처·선언·불일치 + **후보 목록**. 후보가 1개면 그 축은 선택지가 없다.
    "attention_backend": _attn,
    "attention_backend_source": _attn_src,
    "attention_backend_declared": _attn_decl,
    "attention_backend_mismatch": _attn_mm,
    "attention_backend_candidates": _attn_cands,
    "enforce_eager": grep_yaml(cfgtext, "enforce-eager") or "NA",
    "serving_model_name": grep_env(envtext, "SERVING_MODEL_NAME") or "NA",
    "model_path": grep_yaml(cfgtext, "model") or "NA",
}

# ── 측정 도구 소프트 지문 (plan_26090415 §3.6 · 2026-09-04) ──────────────────────
#   **강한키 6종은 불변**이다. 도구를 7번째 강한키로 올리면 부재값 None 대 "N/A" 비교 때문에
#   레거시 인증서 45장이 같은 vLLM-bench 런에 대해서도 즉시 전부 무효화된다. 같은 범주(측정 환경
#   지문)의 처방은 이미 소프트로 굳어 있다 — runtime_selftest 가 `image_digest not in
#   CERTIFICATE_RUN_KEY_FIELDS` 를 하드 assert 한다.
#   값은 **파서가 산출물에서 실측한 것을 승계**한다. 여기서 TOOL 환경변수를 그대로 적으면
#   "무엇을 시켰나"가 되고, 우리가 남겨야 하는 것은 "무엇이 실제로 쟀나"다.
_bt, _btv, _btv_src = "NA", "NA", "unavailable"
_bt_err = "NA"
for _lvl in sorted(completed):
    _mp = os.path.join(sweepdir, "level_%02d" % _lvl, "measured.json")
    try:
        with open(_mp, encoding="utf-8") as _f:
            _md = json.load(_f)
    except (OSError, ValueError):
        continue
    _bt = _md.get("bench_tool") or "vllm-bench-serve"
    _btv = _md.get("bench_tool_version") or "NA"
    _btv_src = _md.get("bench_tool_version_source") or "declared(도구가 버전을 자기보고하지 않는다)"
    if _md.get("max_error_rate_declared") is not None:
        _bt_err = _md["max_error_rate_declared"]
    break
meta["bench_tool"] = _bt
meta["bench_tool_version"] = _btv
meta["bench_tool_version_source"] = _btv_src

# ── 벤치 종료 시 서버 생존(2026-09-07 · 유예 결함 ②) ─────────────────────────────────────────
#   `RemoteProtocolError` 는 두 원인이 같은 모양으로 나온다(도구가 끊음 · 엔진이 죽어 잘림).
#   절단선은 "벤치가 끝난 시점에 서버가 살아 있었는가" 이고, 파서가 그 관측으로 도구경계 면제를
#   켜거나 끈다. 인증서는 그 사실을 **싣기만** 한다 — 판정은 파서가 이미 했고, 여기서 다시 하면
#   같은 질문에 답이 둘이 된다. 관측이 없으면 None 이며 그것도 사실이다(구세대 산출물).
_alive, _bx = None, None
for _lvl in sorted(completed):
    _mp = os.path.join(sweepdir, "level_%02d" % _lvl, "measured.json")
    try:
        with open(_mp, encoding="utf-8") as _f:
            _md = json.load(_f)
    except (OSError, ValueError):
        continue
    _alive = _md.get("server_alive_at_bench_end")
    _bx = _md.get("boundary_exemption")
    break
meta["server_alive_at_bench_end"] = _alive
meta["boundary_exemption"] = _bx
meta["bench_max_error_rate"] = _bt_err

# ── 측정 도구 **런별 구성**(2026-09-05 · plan_26090516 3-8/3-12 · 축 A·B) ─────────
#   digest 게이트를 걷어낸 대신 **무엇으로 쟀는지**를 인증서가 싣는다. 값은 run_bench 가 그 런에
#   실제로 쓴 이미지에서 실측해 남긴 사이드카에서 **승계**한다 — 여기서 기록 파일을 다시 읽으면
#   "무엇을 시켰나"가 되고, 남겨야 하는 것은 "무엇이 실제로 돌았나" 다.
_bt_ref, _bt_dig, _bt_ep = "NA", "NA", "NA"
for _lvl in sorted(completed):
    _tp = os.path.join(sweepdir, "level_%02d" % _lvl, "bench_tool_%s.json" % cfg)
    try:
        with open(_tp, encoding="utf-8") as _f:
            _td = json.load(_f)
    except (OSError, ValueError):
        continue
    _bt_ref = _td.get("image_ref") or "NA"
    _bt_dig = _td.get("observed_digest") or "NA"
    _bt_ep = _td.get("endpoint") or "NA"
    break
meta["bench_tool_image_ref"] = _bt_ref
meta["bench_tool_image_digest"] = _bt_dig
meta["bench_tool_image_digest_source"] = ("measured(docker image inspect .RepoDigests)"
                                          if _bt_dig != "NA" else "unavailable")
meta["bench_endpoint"] = _bt_ep

levels = []
for L in sorted(completed):
    mp = os.path.join(sweepdir, "level_%02d" % L, "measured.json")
    try:
        with open(mp, encoding="utf-8") as f: measured = json.load(f)
    except (OSError, ValueError): measured = None
    levels.append({"level": L, "status": "ok", "measured": measured,
                   "bench_json": os.path.join(sweepdir, "level_%02d" % L, "bench_%s.json" % cfg)})

trunc = []
for line in read(os.path.join(sweepdir, "truncation.log")).splitlines():
    line = line.strip()
    if line.startswith("level ") and "truncated" in line:
        trunc.append(line)

# ── lite 블록 (full ⊇ lite 불변식의 실제 담지체) ──────────────────────────────
# lite_metrics.py 를 **그대로** 호출한다. 산정식을 여기 복제하면 두 벌이 되어 어긋난다
# (D4↔D6 파서 두 벌 사건과 동일 계열). 산정 권위는 lite_metrics 하나뿐이다.
lite_block = None
_lraw = os.environ.get("LITE_RAW") or ""
if _lraw and os.path.isfile(_lraw):
    try:
        import importlib.util as _ilu
        _lm_path = os.path.join(os.environ["SDIR"], "lite_metrics.py")
        _spec = _ilu.spec_from_file_location("lite_metrics", _lm_path)
        _lm = _ilu.module_from_spec(_spec); _spec.loader.exec_module(_lm)
        with open(_lraw, encoding="utf-8") as f:
            _raw = json.load(f)
        _built = _lm.build(_raw)
        lite_block = {
            "raw_json": _lraw,
            "gen_tps": _built.get("gen_tps"),
            "gen_src": _built.get("gen_src"),
            "cold_ttft_ms": _built.get("cold_ttft_ms"),
            "kv_gib": _built.get("kv_gib"),
            "capacity": _built.get("capacity"),
            "table": _built.get("table"),
            "engine": _built.get("engine"),
        }
    except Exception as e:                      # fail-soft, 단 침묵하지 않는다
        lite_block = {"raw_json": _lraw, "error": "lite_metrics 산정 실패: %s" % e}

index = {
    "config": cfg, "topology": topo,
    # 재조립은 측정시각을 승계한다 — 측정이 안 바뀌었는데 시각이 바뀌면 인증서 `measured_utc` 와
    # 파일명 시간토큰이 "새로 쟀다"고 거짓말한다.
    "generated_utc": (prior["generated_utc"] if reassemble
                      else datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")),
    "reassembled_from_raw": True if reassemble else None,
    "verdict_point_level": 1,
    "meta": meta,
    # full ⊇ lite: 이 키가 None 이면 full 은 lite 를 포함하지 못한 것이며, 그 사실이
    # truncated 에도 남는다. 소비자(report/인증서)는 이 블록을 그대로 렌더한다.
    "lite": lite_block,
    "levels": levels,
    "truncated": trunc,
    "input_len": int(os.environ.get("ILEN", "0") or 0),
}
outp = os.path.join(sweepdir, "sweep_index.json")
with open(outp, "w", encoding="utf-8") as f:
    json.dump(index, f, ensure_ascii=False, indent=2)
print("[sweep_bench] sweep_index.json → %s (완료 레벨 %s · 절삭 %d건)"
      % (outp, [l["level"] for l in levels], len(trunc)))
PY

echo "[sweep_bench] DONE — SWEEP_INDEX=$SWEEPDIR/sweep_index.json"
