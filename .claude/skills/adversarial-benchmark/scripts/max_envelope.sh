#!/usr/bin/env bash
# max_envelope.sh — Max 모드: HW 안전-최대 컨텍스트 envelope 특성화 (plan_2026071510_1 §Max)
#
# ★ 별도 오퍼레이션 — 벤치마커의 *측정 인프라만* 공유(run_bench/verdict/render 재사용), 벤치마커 native
#   모드(§2·§4 적대검증)가 아니다. 벤치마커 본체 "기동 안 함" 불변식 보존 — **Max 가 기동/reload 를 소유**.
# 목적: "이 모델을 이 HW 에서 *안전하게* 최대 어느 컨텍스트까지 밀 수 있나"를 config 재서빙하며 **안전측 스텝업**
#   (낮은 max-model-len → 높은 값)으로 특성화. config-space 축 = **컨텍스트(max-model-len)**(옵션 A · plan §Max).
#   각 레벨: config reconfigure → serve+smoke(워치독/RAM게이트 내장) → 통과=안전·기록 → 다음. 실패/트립=직전이 안전상한·중단.
# 안전(헌법 §호스트 안전체계 따름정리 · 하드다운 계보 768k):
#   - **이중 게이트**: (1) 스크립트 `--confirm-risk` 명시 (2) 에이전트가 전작업완료 후 챗 경고톤 Y/N(선-기록 후-위험).
#   - serve+smoke = `multinode_serve_smoke.sh`(협역 워치독 자동 arming + 로드-전 RAM 게이트 내장).
#   - **선-기록 후-위험**: 각 레벨 결과를 *다음 레벨 시도 전* 디스크에 기록(하드다운이 진행분 소실 안 하게).
#   - **안전측 default 상한 = 524288(512k, 검증된 안전상한 testlog_2026071113_1)** — 초과 probe 는 --levels 명시로만
#     (768k 등 known-fatal 은 사용자가 하드다운 위험을 명시 수용해야 진입).
# 사용: max_envelope.sh <config> [--topology single|multi] [--levels 131072,262144,393216,524288]
#        [--confirm-risk] [--dry-run] [--out-dir DIR]
# 산출: output/<topo>/benchlog/max_<config>/{original_config.yaml, level_<L>/{config.yaml,result.json}, max_index.json}
#        → render_max_report.py 가 소비 → docs/benchmark/max_envelope_<model>_<gpu>_<vllm>.md.
# 종료: 0=성공(안전상한 확정) · 2=인자/전제 · 5=--confirm-risk 미명시(안전 게이트).
set -euo pipefail

CONFIG="${1:?config_name 필요}"; shift || true
TOPO=""; LEVELS="131072,262144,393216,524288"; CONFIRM=0; DRYRUN=0; OUTDIR_OVR=""
while [ $# -gt 0 ]; do case "$1" in
  --topology) TOPO="$2"; shift 2;;
  --levels) LEVELS="$2"; shift 2;;
  --confirm-risk) CONFIRM=1; shift;;
  --dry-run) DRYRUN=1; shift;;
  --out-dir) OUTDIR_OVR="$2"; shift 2;;
  *) echo "[max_envelope] 알 수 없는 인자: $1" >&2; exit 2;;
esac; done

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "$SDIR" rev-parse --show-toplevel 2>/dev/null || pwd)"
if [ -z "$TOPO" ]; then
  BR="$(git -C "$REPO" rev-parse --abbrev-ref HEAD 2>/dev/null || echo)"
  case "$BR" in multi-node) TOPO=multi;; single-node) TOPO=single;; *) TOPO=single;; esac
fi

# ── 안전 게이트 (1): --confirm-risk 명시 (경고톤) ──────────────────────────────
if [ "$CONFIRM" != 1 ] && [ "$DRYRUN" != 1 ]; then
  cat >&2 <<'WARN'
[max_envelope] ⚠⚠ 위험 오퍼레이션 — 실행 거부(--confirm-risk 미명시).
  Max 는 config 를 **재서빙(reload)** 하며 컨텍스트를 안전측으로 스텝업합니다.
  통합메모리(GB10) 호스트에서 고컨텍스트 prefill 은 **호스트 하드다운 위험**이 있습니다(하드다운 계보 768k · testlog_2026071111_1).
  선행 필수(선-기록 후-위험): 서빙 확정 · 문서(report/인증서) 발행 · wiki 등록 완료 + 호스트 안전체계 설치 권장.
  실행하려면: max_envelope.sh <config> --confirm-risk   (에이전트는 챗 경고톤 Y/N 승인 후에만 이 플래그를 붙일 것)
WARN
  exit 5
fi

CFGYAML="$REPO/output/$TOPO/configs/${CONFIG}.yaml"
[ -f "$CFGYAML" ] || { echo "[max_envelope] config yaml 없음: $CFGYAML" >&2; exit 2; }
grep -qE '^\s*max-model-len:' "$CFGYAML" || { echo "[max_envelope] $CFGYAML 에 max-model-len 키 없음(컨텍스트 축 스텝업 불가)" >&2; exit 2; }
MANIFEST="$REPO/output/$TOPO/manifest.yaml"; EF="$REPO/output/$TOPO/envs/.env.${CONFIG}"
MAXDIR="${OUTDIR_OVR:-$REPO/output/$TOPO/benchlog/max_${CONFIG}}"

# 레벨 파싱: 정렬·중복제거(안전측 = 오름차순).
IFS=',' read -r -a _lv <<< "$LEVELS"
declare -A _seen; LV=()
for v in "${_lv[@]}"; do v="$(echo "$v" | tr -d '[:space:]')"; [ -z "$v" ] && continue
  case "$v" in (*[!0-9]*) echo "[max_envelope] 레벨은 정수만: '$v'" >&2; exit 2;; esac
  [ -n "${_seen[$v]:-}" ] && continue; _seen[$v]=1; LV+=("$v"); done
IFS=$'\n' LV=($(printf '%s\n' "${LV[@]}" | sort -n)); unset IFS

echo "[max_envelope] config=$CONFIG topo=$TOPO 컨텍스트 레벨(안전측 오름차순)=[${LV[*]}]"
echo "[max_envelope] maxdir=$MAXDIR (선-기록 후-위험: 레벨 결과를 다음 시도 전 기록)"

if [ "$DRYRUN" = "1" ]; then
  echo "[max_envelope] DRY-RUN — 레벨별 계획:"
  for L in "${LV[@]}"; do echo "  max-model-len=$L → reconfigure → serve+smoke($([ "$TOPO" = multi ] && echo multinode_serve_smoke.sh || echo compose-serve)) → 통과=안전·기록·상향 / 실패=직전 안전상한·중단"; done
  echo "[max_envelope] DRY-RUN 종료(재서빙·assemble 생략). 실행 = --confirm-risk."
  exit 0
fi

mkdir -p "$MAXDIR"
cp "$CFGYAML" "$MAXDIR/original_config.yaml"   # 원본 스냅샷
ORIG_LEN="$(awk -F: '/^\s*max-model-len:/{gsub(/[^0-9]/,"",$2);print $2;exit}' "$CFGYAML")"
# EXIT 트랩: 원본 config 복원(하드다운 시엔 트랩 미발동 — max_index 의 safe_ceiling 이 복구 앵커).
restore_cfg(){ cp "$MAXDIR/original_config.yaml" "$CFGYAML" 2>/dev/null && echo "[max_envelope] config 원복(max-model-len=$ORIG_LEN)"; }
trap restore_cfg EXIT

TRUNCLOG="$MAXDIR/truncation.log"; : > "$TRUNCLOG"
SAFE_CEIL=""; SAFE_LEVELS=()
for L in "${LV[@]}"; do
  LDIR="$MAXDIR/level_$L"; mkdir -p "$LDIR"
  # reconfigure: max-model-len 값만 치환(주석 보존)
  sed -i "s/^\(\s*max-model-len:[[:space:]]*\)[0-9]\+/\1$L/" "$CFGYAML"
  cp "$CFGYAML" "$LDIR/config.yaml"   # per-level config 스냅샷(simlog 규율)
  echo "[max_envelope] ── max-model-len=$L 재서빙+스모크 ──"
  SMOKE_EXIT=0
  if [ "$TOPO" = "multi" ]; then
    bash "$SDIR/../../upstream-version-watch/scripts/multinode_serve_smoke.sh" "$CONFIG" || SMOKE_EXIT=$?
  else
    # single-node serve+smoke = 후속(현재 multi 우선 — 하드다운 계보 env). graceful 보고.
    echo "[max_envelope] ⚠ single-node serve+smoke primitive 미구현 — multi 우선(후속). 이 레벨 skip·기록." | tee -a "$TRUNCLOG"
    SMOKE_EXIT=90
  fi
  # 선-기록: 결과를 다음 레벨 시도 전 기록
  STATUS=$([ "$SMOKE_EXIT" = "0" ] && echo safe || echo unsafe)
  L="$L" ML="$L" SE="$SMOKE_EXIT" ST="$STATUS" python3 - <<'PY' > "$LDIR/result.json"
import json, os
print(json.dumps({"max_model_len": int(os.environ["ML"]), "serve_smoke_exit": int(os.environ["SE"]),
                  "status": os.environ["ST"]}, ensure_ascii=False))
PY
  if [ "$SMOKE_EXIT" = "0" ]; then
    SAFE_CEIL="$L"; SAFE_LEVELS+=("$L"); echo "[max_envelope] max-model-len=$L ✓ 안전(누적 상한=$SAFE_CEIL)"
  else
    echo "max-model-len $L unsafe: serve+smoke exit $SMOKE_EXIT → 직전 안전상한=${SAFE_CEIL:-없음(최저레벨도 실패)}, 상위 중단" | tee -a "$TRUNCLOG"
    break   # 안전측: 첫 실패 레벨에서 중단(직전이 안전상한)
  fi
done

# ── max_index.json 조립 + meta(sweep_bench 와 동형 grep) ───────────────────────
CONFIG="$CONFIG" TOPO="$TOPO" CFGYAML="$MAXDIR/original_config.yaml" EF="$EF" MANIFEST="$MANIFEST" \
MAXDIR="$MAXDIR" SAFE_CEIL="${SAFE_CEIL:-}" SAFE_LEVELS="${SAFE_LEVELS[*]:-}" ORIG_LEN="${ORIG_LEN:-}" python3 - <<'PY'
import json, os, re, datetime
def read(p):
    try:
        with open(p, encoding="utf-8", errors="replace") as f: return f.read()
    except OSError: return ""
def gy(t,k):
    m=re.search(r"(?m)^\s*%s\s*:\s*([^\n#]+)"%re.escape(k),t); return m.group(1).strip().strip('"').strip("'") if m else None
cfg=os.environ["CONFIG"]; topo=os.environ["TOPO"]; maxdir=os.environ["MAXDIR"]
mft=read(os.environ["MANIFEST"]); cfgt=read(os.environ["CFGYAML"])
gpu=gy(mft,"gpu_model") or "NA"; gkey=re.sub(r"[^A-Za-z0-9]","",gpu.replace("NVIDIA","")) or "NA"
m=re.search(r"vLLM[\s]*([0-9]+\.[0-9]+\.[0-9]+)",cfgt); vllm=os.environ.get("EASY_VLLM_VERSION") or (m.group(1) if m else "NA")
tp=gy(cfgt,"tensor-parallel-size") or "1"
levels=[]
for L in sorted(int(x) for x in os.environ.get("SAFE_LEVELS","").split() if x.strip()):
    try:
        with open(os.path.join(maxdir,"level_%d"%L,"result.json"),encoding="utf-8") as f: r=json.load(f)
    except (OSError,ValueError): r={"max_model_len":L,"status":"safe"}
    levels.append(r)
# unsafe(중단) 레벨도 truncation.log 에서 수집(있으면 마지막 시도가 unsafe)
trunc=[l.strip() for l in read(os.path.join(maxdir,"truncation.log")).splitlines() if "unsafe" in l or "skip" in l or "미구현" in l]
idx={"config":cfg,"topology":topo,"axis":"max-model-len",
     "generated_utc":datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
     "safe_ceiling_max_model_len": int(os.environ["SAFE_CEIL"]) if os.environ.get("SAFE_CEIL") else None,
     "original_max_model_len": int(os.environ["ORIG_LEN"]) if os.environ.get("ORIG_LEN") else None,
     "meta":{"model":cfg,"gpu_model":gpu,"gpu_key":gkey,"vllm_version":vllm,"topology":topo,
             "tensor_parallel_size":tp,"driver_version":gy(mft,"driver_version") or "NA",
             "quantization":gy(cfgt,"quantization") or "NA","moe_backend":gy(cfgt,"moe-backend") or "NA",
             "image_tag":"NA"},
     "safe_levels":levels,"truncated":trunc}
outp=os.path.join(maxdir,"max_index.json")
with open(outp,"w",encoding="utf-8") as f: json.dump(idx,f,ensure_ascii=False,indent=2)
print("[max_envelope] max_index.json → %s (안전상한 max-model-len=%s · 안전레벨 %d · 절삭 %d)"
      %(outp, idx["safe_ceiling_max_model_len"], len(levels), len(trunc)))
PY

echo "[max_envelope] DONE — 안전상한 max-model-len=${SAFE_CEIL:-없음}. MAX_INDEX=$MAXDIR/max_index.json"
echo "[max_envelope] 다음: render_max_report.py --max-index $MAXDIR/max_index.json  (→ docs/benchmark/max_envelope_*.md)"
