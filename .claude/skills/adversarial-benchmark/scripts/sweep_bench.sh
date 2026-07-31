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
if bash "$SDIR/lite_bench.sh" "$CONFIG" --topology "$TOPO" --out-dir "$SWEEPDIR" >/dev/null 2>&1 \
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
CONFIG="$CONFIG" TOPO="$TOPO" CFGYAML="$CFGYAML" EF="$EF" MANIFEST="$MANIFEST" AGENT_CARD="$REPO/Agent_Card.json" \
SWEEPDIR="$SWEEPDIR" VLLM_VER="$VLLM_VER" COMPLETED="${COMPLETED[*]:-}" ILEN="$ILEN" \
 LITE_RAW="$LITE_RAW" SDIR="$SDIR" python3 - <<'PY'
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
# 엔진 자기보고는 버리지 않고 **vllm_build(소프트) + 교차검증**으로 쓴다 — 2026-07-31 에
#   IMAGE_TAG 소실로 compose 가 0.24.0 으로 조용히 폴백했는데 health 200 이라 아무 데서도 안 걸렸다.
#   두 출처를 다 기록하고 라인이 어긋나면 vllm_mismatch 로 **시끄럽게** 남긴다(침묵 치환 금지).
_img = grep_env(envtext, "IMAGE_TAG") or ""
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
# 교차검증: 이미지 라인 ↔ 엔진 자기보고. dev 빌드는 마이너 +1 이 정상이므로(0.26.0 이미지가
# 0.26.1.dev0 을 보고) major.minor 만 비교하고, 그마저 어긋나면 치환으로 본다.
vllm_mismatch = "no"
if vllm != "NA" and vllm_build != "NA":
    _a = ".".join(vllm.split(".")[:2])
    _b = ".".join(vllm_build.split(".")[:2])
    if _a != _b:
        _bm = re.match(r"([0-9]+)\.([0-9]+)", vllm_build)
        _am = re.match(r"([0-9]+)\.([0-9]+)", vllm)
        ok_devbump = bool(_am and _bm and _am.group(1) == _bm.group(1)
                          and int(_bm.group(2)) == int(_am.group(2)) + 1 and ".dev" in vllm_build)
        vllm_mismatch = "no" if ok_devbump else "YES(image=%s engine=%s)" % (vllm, vllm_build)

# gpu_model: manifest > Agent_Card.json(node_identity) > NA.
#   ⚠ 서브 노드에는 manifest.yaml 이 **설계상 부재**(D10 — sync_to_sub 가 manifest 를 배달하지 않는다; 서브 정체성은
#   메인이 render_sub_env.py 로 렌더한 Agent_Card.json 에 산다). 폴백이 없으면 서브에서 돈 full 벤치의
#   인증서 강한키 gpu 가 "NA" 로 발행돼 carry-forward 재검증이 무력화된다(plan_26072217 실측).
#   Agent_Card 의 gpu_model 은 메인의 HW 동질성 스캔 산물이므로 날조가 아니라 **A2A attestation** 이다.
gpu_model = grep_yaml(mftext, "gpu_model")
if not gpu_model:
    try:
        with open(os.environ.get("AGENT_CARD", ""), encoding="utf-8") as _f:
            gpu_model = (json.load(_f).get("node_identity") or {}).get("gpu_model") or None
    except Exception:
        gpu_model = None
gpu_model = gpu_model or "NA"
gpu_key = re.sub(r"[^A-Za-z0-9]", "", gpu_model.replace("NVIDIA", "")) or "NA"  # "NVIDIA GB10" → "GB10"
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

meta = {
    # 강한 일치 키
    # model 강한키 = **서빙 모델명**(SERVING_MODEL_NAME) 우선. config 파일명은 운영 산물이라
    # 같은 모델을 다른 config 로 재실험하면 강한키가 갈라져 carry-forward 가 끊긴다.
    # (기존 인증서들은 config 이름이 우연히 모델명과 같아 이 구분이 드러나지 않았다.)
    "model": grep_env(envtext, "SERVING_MODEL_NAME") or cfg,
    "config_name": cfg,
    "gpu_model": gpu_model, "gpu_key": gpu_key, "vllm_version": vllm,
    "vllm_build": vllm_build, "vllm_mismatch": vllm_mismatch,
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
    "generated_utc": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
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
