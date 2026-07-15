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
# 사용: sweep_bench.sh <config_name> [--topology single|multi] [--levels 1,2,4,8,16]
#        [--input-len N] [--output-len N] [--num-prompts N] [--warmups N] [--vllm-version X] [--dry-run]
# 산출: output/<topo>/benchlog/sweep_<config>/{level_NN/{bench_<config>.json,engine_<config>.log,measured.json},
#        truncation.log, sweep_index.json}  ← render_report.py·publish_benchmark_record.py 가 소비.
# 종료: 0=성공(레벨 ≥1 완료) · 2=인자/전제 오류 · 3=판정점(레벨1) 측정 불가(serve 미가동/게이트 등).
set -euo pipefail

CONFIG="${1:?config_name 필요}"; shift || true
TOPO=""; LEVELS="1,2,4,8,16"; ILEN=1024; OLEN=256; NPROMPTS=16; WARMUPS=2; VLLM_VER=""; DRYRUN=0
while [ $# -gt 0 ]; do case "$1" in
  --topology) TOPO="$2"; shift 2;;
  --levels) LEVELS="$2"; shift 2;;
  --input-len) ILEN="$2"; shift 2;;
  --output-len) OLEN="$2"; shift 2;;
  --num-prompts) NPROMPTS="$2"; shift 2;;
  --warmups) WARMUPS="$2"; shift 2;;
  --vllm-version) VLLM_VER="$2"; shift 2;;
  --dry-run) DRYRUN=1; shift;;
  *) echo "[sweep_bench] 알 수 없는 인자: $1" >&2; exit 2;;
esac; done

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

echo "[sweep_bench] config=$CONFIG topo=$TOPO levels=[${SORTED[*]}] in=$ILEN out=$OLEN n=$NPROMPTS warmup=$WARMUPS"
echo "[sweep_bench] sweepdir=$SWEEPDIR (판정점=동시성1 재사용)"
if [ "$DRYRUN" = "1" ]; then
  echo "[sweep_bench] DRY-RUN — 레벨별 실행 계획:"
  for L in "${SORTED[@]}"; do
    echo "  level $L → run_bench.sh $CONFIG --topology $TOPO --concurrency $L --out-dir $SWEEPDIR/level_$(printf '%02d' "$L")"
  done
  echo "[sweep_bench] DRY-RUN 종료(실제 벤치·assemble 생략)"
  exit 0
fi

mkdir -p "$SWEEPDIR"
TRUNCLOG="$SWEEPDIR/truncation.log"; : > "$TRUNCLOG"
COMPLETED=()
for L in "${SORTED[@]}"; do
  LDIR="$SWEEPDIR/level_$(printf '%02d' "$L")"; mkdir -p "$LDIR"
  echo "[sweep_bench] ── level 동시성=$L ──"
  if bash "$SDIR/run_bench.sh" "$CONFIG" --topology "$TOPO" --concurrency "$L" \
        --input-len "$ILEN" --output-len "$OLEN" --num-prompts "$NPROMPTS" \
        --warmups "$WARMUPS" --out-dir "$LDIR"; then
    BJSON="$LDIR/bench_${CONFIG}.json"; ELOG="$LDIR/engine_${CONFIG}.log"
    if python3 "$SDIR/parse_bench.py" --bench-json "$BJSON" --engine-log "$ELOG" > "$LDIR/measured.json" 2>/dev/null \
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

# ── sweep_index.json 조립 + meta 추출(결정론 · stdlib · fail-soft N/A) ──────────
CONFIG="$CONFIG" TOPO="$TOPO" CFGYAML="$CFGYAML" EF="$EF" MANIFEST="$MANIFEST" \
SWEEPDIR="$SWEEPDIR" VLLM_VER="$VLLM_VER" COMPLETED="${COMPLETED[*]:-}" ILEN="$ILEN" python3 - <<'PY'
import json, os, re, glob, datetime

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

# vLLM 버전: --vllm-version > env > config yaml 첫 주석 'vLLM X.Y.Z' > NA
vllm = os.environ.get("VLLM_VER") or os.environ.get("EASY_VLLM_VERSION") or None
if not vllm:
    m = re.search(r"vLLM[\s]*([0-9]+\.[0-9]+\.[0-9]+)", cfgtext)
    vllm = m.group(1) if m else "NA"

gpu_model = grep_yaml(mftext, "gpu_model") or "NA"
gpu_key = re.sub(r"[^A-Za-z0-9]", "", gpu_model.replace("NVIDIA", "")) or "NA"  # "NVIDIA GB10" → "GB10"
# tp: config tensor-parallel-size > manifest nodes*gpus_per_node > 1
tp = grep_yaml(cfgtext, "tensor-parallel-size")
if tp is None:
    n_nodes = len(re.findall(r"(?m)^\s*-\s*role:\s*", mftext))
    gpn = grep_yaml(mftext, "gpus_per_node")
    try: tp = str(max(1, n_nodes) * int(gpn)) if (n_nodes and gpn) else "1"
    except (TypeError, ValueError): tp = "1"

meta = {
    # 강한 일치 키
    "model": cfg, "gpu_model": gpu_model, "gpu_key": gpu_key, "vllm_version": vllm,
    "quantization": grep_yaml(cfgtext, "quantization") or "NA",
    "topology": topo, "tensor_parallel_size": tp,
    # 소프트 지문
    "driver_version": grep_yaml(mftext, "driver_version") or "NA",
    "cuda_version": grep_yaml(mftext, "cuda_version") or "NA",
    "image_tag": grep_env(envtext, "IMAGE_TAG") or "NA",
    "max_model_len": grep_yaml(cfgtext, "max-model-len") or "NA",
    "max_num_seqs": grep_yaml(cfgtext, "max-num-seqs") or "NA",
    "kv_cache_memory_bytes": grep_yaml(cfgtext, "kv-cache-memory-bytes") or "NA",
    "kv_cache_dtype": grep_yaml(cfgtext, "kv-cache-dtype") or "NA",
    "gpu_memory_utilization": grep_yaml(cfgtext, "gpu-memory-utilization") or "NA",
    "moe_backend": grep_yaml(cfgtext, "moe-backend") or "NA",
    "enforce_eager": grep_yaml(cfgtext, "enforce-eager") or "NA",
    "serving_model_name": grep_env(envtext, "SERVING_MODEL_NAME") or "NA",
    "model_path": grep_yaml(cfgtext, "model") or "NA",
}

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

index = {
    "config": cfg, "topology": topo,
    "generated_utc": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "verdict_point_level": 1,
    "meta": meta,
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
