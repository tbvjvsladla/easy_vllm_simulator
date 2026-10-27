#!/usr/bin/env python3
"""parse_model_config.py — config.json 결정론 파서 (vllm-recipe-explorer).

폐쇄망 전제. 고정된 한 모델의 config.json + 로컬 safetensors 헤더를 결정론으로 읽어
estimate/rank 단계가 소비하는 정규화 dict 를 만든다. 모델 다운로드는 절대 하지 않는다
(NAS 부재 시 비0 종료 + 명확한 중단·보고).

CONTRACT(FROZEN) parse_model_config.py 절을 near-pseudocode 그대로 구현.
quant_table 에서 dtype_bpw / vllm_quant_bpw 를 import (벤더링 테이블 소유는 quant_table.py).
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import struct
import sys

# scripts/ 를 sys.path 에 넣어 형제 모듈(quant_table) import 가능하게 한다.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from quant_table import dtype_bpw, vllm_quant_bpw_or_none  # noqa: E402

# /app/models 컨테이너 경로가 매핑되는 호스트 NAS 루트 기본값 (CONTRACT 경로 매핑).
DEFAULT_NAS_HOST_ROOT = "/mnt/models"

# safetensors/src/tensor.rs MAX_HEADER_SIZE. 가중치 payload를 헤더로 읽지 않는다.
_SAFETENSORS_MAX_HEADER = 100_000_000

# safetensors 헤더 dtype 문자열 → 온디스크 bytes-per-weight (disk_bpw).
# F32→4, F16/BF16→2, F8*→1. (config torch_dtype 가 거짓일 수 있으므로 실측이 진실의 원천.)
_SAFETENSORS_DTYPE_BPW: dict[str, float] = {
    "F64": 8.0,
    "I64": 8.0,
    "F32": 4.0,
    "I32": 4.0,
    "F16": 2.0,
    "BF16": 2.0,
    "I16": 2.0,
    "F8_E4M3": 1.0,
    "F8_E5M2": 1.0,
    "I8": 1.0,
    "U8": 1.0,
}

# ── layer_types 어휘 (하이브리드 KV 층수 인지 · plan_26082223 결함 A) ────────────────
#   `_KV_FREE_LAYER_TYPES` = **표준 KV 캐시를 보유하지 않는** 층 타입의 닫힌 목록(tripwire —
#   새 타입을 넣으려면 리뷰가 강제된다). 여기 없는 타입은 전부 KV 보유로 센다(보수적).
#   linear_attention = GDN/mamba 계열 conv+recurrent state. 토큰 수에 비례하지 않는다.
_KV_FREE_LAYER_TYPES = ("linear_attention",)
_KV_FULL_LAYER_TYPE = "full_attention"

# safetensors 헤더 dtype 문자열 → 원소당 바이트 (가장 큰 텐서 판정용).
_SAFETENSORS_DTYPE_ELEM_BYTES: dict[str, int] = {
    "F64": 8,
    "I64": 8,
    "F32": 4,
    "I32": 4,
    "F16": 2,
    "BF16": 2,
    "I16": 2,
    "F8_E4M3": 1,
    "F8_E5M2": 1,
    "I8": 1,
    "U8": 1,
    "BOOL": 1,
}


def resolve_kv_layers(layer_types, num_hidden_layers, full_attention_interval=None):
    """`layer_types` → (num_full_attention_layers, is_hybrid, counts, warnings). **순수 함수**.

    왜 있나: KV 공식은 `num_hidden_layers` 전층이 표준 KV 를 보유한다고 가정한다. 그러나
    GDN/linear-attention 하이브리드(Qwen3.6/3.8 계열)는 전 층 중 일부만 `full_attention` 이고
    나머지는 `linear_attention`(conv+recurrent state — 토큰 비례 KV 가 아니다)이다. 그 결과
    Phase-1 하드게이트가 KV 를 **3.95× 과대추정**해 실제로 서빙되는 티어를 FAIL 시켰다
    (2026-08-22 Qwen3.8-27B 실측: 공식 262,144 B/token vs 측정 66,290 B/token ·
     `docs/simlog/26082221_qwen38_27b_R0기준선/raw/P0_P2_findings.md` · plan_26082223 결함 A).
    Phase-2 는 측정 per-token 을 써서 정확했고(`recipe._resolve_clamp_kv` 주석의 Qwen3.6 선례),
    측정이 없는 Phase-1 만 공식으로 돌아 이 갭이 생겼다.

    판정은 `layer_types` **단독**으로 한다 — `full_attention_interval` 은 publisher 마다 있을
    수도 없을 수도 있어 이식성이 없다(있으면 tripwire 교차검증에만 쓴다).

    ★ 미지 타입은 **KV 보유로 센다**(보수적 = 과대추정 = OOM 안전). 예: `sliding_attention` 은
      창 크기만큼이지만 KV 를 보유하므로 0 으로 세면 과소추정이 된다. 아는 KV-무보유 타입만
      `_KV_FREE_LAYER_TYPES` 닫힌 목록으로 뺀다.

    부정합(길이 불일치 · KV 보유층 0)은 **조용히 쓰지 않고** 전층 폴백 + 경고다 — 과소추정은
    하드게이트를 무력화하지만 과대추정은 보수적일 뿐이다.
    """
    warnings: list[str] = []
    if not isinstance(layer_types, list) or not layer_types:
        return None, False, None, warnings

    counts: dict[str, int] = {}
    for _t in layer_types:
        _k = str(_t)
        counts[_k] = counts.get(_k, 0) + 1

    kv_bearing = sum(n for t, n in counts.items() if t not in _KV_FREE_LAYER_TYPES)
    unknown_types = sorted(
        t for t in counts if t not in _KV_FREE_LAYER_TYPES and t != _KV_FULL_LAYER_TYPE
    )
    if unknown_types:
        warnings.append(
            "layer_types 에 미지 타입 %s → KV 보유로 계산(보수적·과대추정). KV-무보유가 맞다면 "
            "parse_model_config._KV_FREE_LAYER_TYPES 에 근거와 함께 등재하라"
            % (", ".join(unknown_types),)
        )

    if num_hidden_layers is not None and len(layer_types) != int(num_hidden_layers):
        warnings.append(
            "layer_types 길이(%d) != num_hidden_layers(%s) → 층수 인지 포기, 전층 폴백(보수적)"
            % (len(layer_types), num_hidden_layers)
        )
        return None, False, counts, warnings

    if kv_bearing <= 0:
        # KV 보유층 0 = KV 0 = 하드게이트 무조건 통과. 조용한 과소추정 금지 → 전층 폴백.
        warnings.append(
            "layer_types 에 KV 보유층이 0 개 → 층수 인지 포기, 전층 폴백(과소추정 방지)"
        )
        return None, False, counts, warnings

    is_hybrid = num_hidden_layers is not None and kv_bearing < int(num_hidden_layers)

    # tripwire: interval 이 있으면 교차검증한다(둘이 갈리면 사람이 봐야 한다).
    if full_attention_interval and num_hidden_layers is not None:
        try:
            expected = int(num_hidden_layers) // int(full_attention_interval)
        except (TypeError, ValueError, ZeroDivisionError):
            expected = None
        if expected is not None and expected != kv_bearing:
            warnings.append(
                "full_attention_interval(%s)로 기대한 full 층수(%d) != layer_types 집계(%d) — "
                "layer_types 를 권위로 쓴다(interval 은 참고값)"
                % (full_attention_interval, expected, kv_bearing)
            )

    return int(kv_bearing), bool(is_hybrid), counts, warnings


def _resolve_host_path(model_path: str, nas_host_root: str,
                       nas_container_root: str = "/app/models") -> str:
    """컨테이너 경로(nas_container_root/...)면 NAS 루트로 치환. 그 외는 그대로.

    nas_container_root: 컨테이너 마운트 경로 prefix(기본 /app/models). quant_model 등
    2차 마운트(/app/quant_models)를 쓰면 config 에서 override(산출물 통로/마운트 불변식)."""
    prefix = nas_container_root.rstrip("/")
    if model_path == prefix or model_path.startswith(prefix + "/"):
        rel = model_path[len(prefix):].lstrip("/")
        return os.path.join(nas_host_root, rel)
    return model_path


def _model_id_from_path(model_path: str) -> str:
    """경로 끝 두 컴포넌트(<Org>/<Name>)를 model_id 로 채택. 부족하면 basename."""
    norm = model_path.rstrip("/")
    parts = [p for p in norm.split("/") if p]
    if len(parts) >= 2:
        return parts[-2] + "/" + parts[-1]
    if parts:
        return parts[-1]
    return model_path


def _read_safetensors_header(st_path: str) -> dict | None:
    """safetensors 파일 헤더 JSON dict 반환. 첫 8바이트 LE uint64 = header_len, 이어 JSON."""
    try:
        with open(st_path, "rb") as f:
            length_bytes = f.read(8)
            if len(length_bytes) != 8:
                return None
            header_len = struct.unpack("<Q", length_bytes)[0]
            if header_len > _SAFETENSORS_MAX_HEADER or header_len > os.fstat(f.fileno()).st_size - 8:
                return None
            header_bytes = f.read(header_len)
            if len(header_bytes) != header_len:
                return None
            return json.loads(header_bytes.decode("utf-8"))
    except (OSError, ValueError, json.JSONDecodeError):
        return None


def _measure_disk_dtype(st_path: str) -> tuple[str | None, float | None]:
    """safetensors 헤더에서 '가장 큰 텐서(원소수×바이트)'의 dtype 과 disk_bpw 채택.

    반환 (disk_dtype, disk_bpw). 헤더 못 읽으면 (None, None).
    """
    header = _read_safetensors_header(st_path)
    if not header:
        return None, None

    best_dtype: str | None = None
    best_size = -1
    for name, meta in header.items():
        if name == "__metadata__":
            continue
        if not isinstance(meta, dict):
            continue
        dtype = meta.get("dtype")
        shape = meta.get("shape")
        if not dtype or not isinstance(shape, list):
            continue
        numel = 1
        for d in shape:
            try:
                numel *= int(d)
            except (TypeError, ValueError):
                numel = 0
                break
        elem_bytes = _SAFETENSORS_DTYPE_ELEM_BYTES.get(dtype, 2)
        size = numel * elem_bytes
        if size > best_size:
            best_size = size
            best_dtype = dtype

    if best_dtype is None:
        return None, None
    disk_bpw = _SAFETENSORS_DTYPE_BPW.get(best_dtype)
    return best_dtype, disk_bpw


def _native_weight_bytes(host_path: str, warnings: list[str]) -> int | None:
    """폐쇄망 로컬 weight 바이트 원천(결정론).

    우선순위: **index weight_map 참조 샤드의 실제 파일 크기 합**(정본) →
    *.safetensors 크기 합 → *.bin 크기 합 → None+경고.

    ★ 권위는 "로더가 실제로 읽을 파일들의 os.path.getsize 합"이다. 세 가지 함정을 동시에 피한다:
      1) `du`/디렉터리 walk — `.git/lfs/objects/` 가 모든 샤드의 **복제본**을 갖고 있어 2배가 된다
         (2026-08-01 Olmo-3.1-32B 실측: du 121 GiB ↔ 실로드 60.04 GiB). 이 함정은 이 프로젝트에서
         네 번 발현했다.
      2) `*.safetensors` 글롭 — 동거 포맷 세트를 함께 센다(LFM2 실측: F32 of-00007 31.1 GiB 가
         bf16 of-00004 15.5 GiB 와 동거). index 가 가리키는 쪽만 로드된다.
      3) **`metadata.total_size` 자체** — 이건 디스크 바이트가 아니라 **발행자가 계산해 적은 값**이며
         틀릴 수 있다. Olmo-3.1-32B 는 32B 파라미터를 fp32(4 B) 기준으로 적어 실제 bf16 파일 합의
         **정확히 2배**(128.9 GB vs 64.5 GB)를 신고한다. 이 값을 권위로 쓰면 로드-전 RAM 게이트가
         멀쩡한 모델을 거부한다(2026-08-01 실제 발생).
    total_size 는 **교차검증용**으로만 쓰고 10% 넘게 어긋나면 경고한다 — 조용히 버리면
    "왜 다른가"를 다음 사람이 다시 조사하게 된다.
    """
    index_path = os.path.join(host_path, "model.safetensors.index.json")
    if os.path.isfile(index_path):
        try:
            with open(index_path, "r", encoding="utf-8") as f:
                idx = json.load(f)
            weight_map = idx.get("weight_map")
            if isinstance(weight_map, dict) and weight_map:
                shards = sorted(set(weight_map.values()))
                paths = [os.path.join(host_path, s) for s in shards]
                missing = [s for s, p in zip(shards, paths) if not os.path.isfile(p)]
                if missing:
                    warnings.append(
                        "index weight_map 참조 샤드 %d개 부재(예: %s) → 글롭 폴백"
                        % (len(missing), missing[0])
                    )
                else:
                    real = sum(os.path.getsize(p) for p in paths)
                    total = idx.get("metadata", {}).get("total_size")
                    if isinstance(total, (int, float)) and total > 0 and real > 0:
                        ratio = float(total) / real
                        if not (0.9 <= ratio <= 1.1):
                            warnings.append(
                                "index metadata.total_size=%.2f GiB 가 실제 샤드 합 %.2f GiB 의 "
                                "%.2f배 — 발행자가 다른 dtype 기준으로 적은 값이다. "
                                "실제 파일 크기를 채택한다."
                                % (total / (1024 ** 3), real / (1024 ** 3), ratio)
                            )
                    return int(real)
        except (OSError, ValueError, json.JSONDecodeError):
            warnings.append(
                "model.safetensors.index.json 읽기 실패 → 파일 크기 합으로 폴백"
            )

    safetensors = sorted(glob.glob(os.path.join(host_path, "*.safetensors")))
    if safetensors:
        try:
            return sum(os.path.getsize(p) for p in safetensors)
        except OSError as e:
            warnings.append(f"*.safetensors 크기 합 실패: {e}")

    bins = sorted(glob.glob(os.path.join(host_path, "*.bin")))
    if bins:
        try:
            return sum(os.path.getsize(p) for p in bins)
        except OSError as e:
            warnings.append(f"*.bin 크기 합 실패: {e}")

    warnings.append(
        "weight 파일(*.safetensors/*.bin/index.json)을 찾지 못함 → native_weight_bytes=None"
    )
    return None


# ─────────────────────────────────────────────────────────────────────────────
# NVMe mmap 으로 **상주하지 않는** weight (2026-09-11 신설 · plan_26091108 R2)
#
# 왜: 예산 게이트의 weights 는 `ckpt_mib ÷ tp` 였다 — 체크포인트 **파일 크기** 기준이라
#   그 일부가 NVMe mmap 으로 빠져도 같은 값이 나온다. camp-26090918 실측이 그 맹점을 정면으로
#   보여줬다: `fp8+res`(weights 88,464 → 예상 바닥 −9,134)는 막아야 옳고 `fp8+mmp`(weights
#   64,050 → 바닥 15,280)는 **실제로 서빙에 성공했는데**, 게이트는 둘을 같은 88,464 로 계산해
#   둘 다 막는다. 게이트를 그대로 켜면 뜨는 셀을 죽인다.
#
# 닫힌 목록인 이유(§4종 안티패턴 판정표 '하드코딩' 의 **정당** 칸 = tripwire): 어떤 텐서가
#   mmap 으로 빠지는지는 **런타임 패치가 정하는 사실**이고 체크포인트에서 파생되지 않는다.
#   새 mmap 기구가 생기면 이 표에 줄을 더하게 하고, 그 편집이 리뷰를 강제한다.
#   ★ 매칭이 0건이면 보정은 0 이다 — 그것이 "이 모델엔 해당 기구가 없다" 는 정직한 답이며,
#     추측으로 보정하면 게이트가 없는 여유를 있다고 말한다.
MMAP_OFFLOADABLE_PATTERNS = {
    # qwen4_exp PLE n-gram 테이블 — build_patches_src/62-qwen4exp-ple-mmap.sh 가 `VLLM_PLE_MMAP=1`
    # 일 때 이 텐서들을 NVMe mmap 으로 서빙한다(상주 ✗). 패턴 정본은 그 패치의 `_find_shards`
    # 이고 여기는 **예산 평면의 사본**이다 — 교차검증 앵커는 preload_ram_gate 자체검사에 있다.
    "ple_mmap": r"\.ple\.ple_embedding\.ngram_embedding\.shard_\d+\.weight$",
    # DeepSeek-V4.1 Engram 조회 테이블 — build_patches_src/83-engram-file-backend.sh(+87-engram-embed-skip.sh)가
    # hf-overrides `engram_table_dir` 가 선언된 서빙에서 이 테이블을 **파일 백엔드(디스크 pread)** 로 서빙한다(상주 ✗).
    # 가중치 로더는 이 텐서를 읽지 않는다(87 의 should_skip_weight 가드). 선언이 없으면 stock 경로가 이 테이블을
    # 핀드 메모리에 올리므로 **보정하면 안 된다** — 호출부(check_smoke_model)가 선언 여부로만 이 기구를 고른다.
    # A 팩(sfxnz 2.0bpw) 실측: layers.{1,14}.engram.embed.{weight,scale} = 188.8 GiB(샤드 47·48).
    "engram_file": r"\.engram\.embed\.(weight|scale)$",
}


def offloadable_weight_bytes(host_path: str, mechanism: str,
                             warnings: list[str]) -> int | None:
    """`mechanism` 이 상주에서 덜어내는 weight 바이트. 결정론 — index + safetensors 헤더에서만 읽는다.

    반환 None = **산출 실패**(보정하지 않는다). 0 = 산출했고 해당 텐서가 없다.
    둘을 같은 값으로 접으면 "못 쟀다" 와 "없다" 가 구분되지 않는다.
    """
    import re
    pattern = MMAP_OFFLOADABLE_PATTERNS.get(mechanism)
    if pattern is None:
        warnings.append(f"mmap 기구 '{mechanism}' 이 닫힌 목록에 없다 → 보정 0(추측 ✗)")
        return None
    index_path = os.path.join(host_path, "model.safetensors.index.json")
    if not os.path.isfile(index_path):
        warnings.append("index.json 부재 → mmap 오프로드 바이트 산출 불가(보정 ✗)")
        return None
    try:
        with open(index_path, "r", encoding="utf-8") as f:
            weight_map = json.load(f).get("weight_map")
    except (OSError, ValueError) as exc:
        warnings.append(f"index.json 읽기 실패 → mmap 오프로드 산출 불가: {exc}")
        return None
    if not isinstance(weight_map, dict) or not weight_map:
        warnings.append("weight_map 이 비었다 → mmap 오프로드 산출 불가(보정 ✗)")
        return None

    rx = re.compile(pattern)
    by_shard: dict[str, list[str]] = {}
    for name, shard in weight_map.items():
        if rx.search(name):
            by_shard.setdefault(shard, []).append(name)
    if not by_shard:
        return 0                       # 해당 기구의 텐서가 이 모델에 없다 — 정직한 0

    total = 0
    for shard, names in by_shard.items():
        path = os.path.join(host_path, shard)
        try:
            with open(path, "rb") as f:
                hdr_len = struct.unpack("<Q", f.read(8))[0]
                header = json.loads(f.read(hdr_len))
        except (OSError, ValueError, struct.error) as exc:
            warnings.append(f"샤드 헤더 읽기 실패({shard}) → mmap 오프로드 산출 불가: {exc}")
            return None
        for name in names:
            entry = header.get(name) or {}
            offs = entry.get("data_offsets")
            if not (isinstance(offs, list) and len(offs) == 2):
                warnings.append(f"{name}: data_offsets 부재 → mmap 오프로드 산출 불가")
                return None
            total += int(offs[1]) - int(offs[0])
    return total


def _largest_safetensors(host_path: str) -> str | None:
    """가장 큰 *.safetensors 파일 경로 (disk_bpw 실측 대상). 없으면 None."""
    files = glob.glob(os.path.join(host_path, "*.safetensors"))
    if not files:
        return None
    try:
        return max(files, key=os.path.getsize)
    except OSError:
        return None


def _count_params_from_headers(host_path: str) -> int | None:
    """모든 *.safetensors 헤더에서 텐서별 numel(shape 곱)을 합산한 '정확한' 파라미터 수.

    온디스크 바이트(total_size)를 단일 bytes-per-weight 로 나누는 방식은 혼합 정밀도
    체크포인트(예: 임베딩만 F32, 나머지 BF16)에서 2x 오산된다. 반면 헤더의 shape 는
    저장 dtype 과 무관한 '논리 원소 수'라 그 합이 곧 참 파라미터 수다(F32 로 저장된
    bf16 모델 Motif-2.6B 도, 혼합 dtype LFM2-8B 도 정확히 나온다).

    전제: *.safetensors 헤더 기반(비양자화 가중치). 양자화 패킹(mxfp4=U8) 텐서는 numel 이
    논리 파라미터 수와 어긋나므로 호출측에서 prequantized 가 아닐 때만 사용한다.
    모든 샤드 헤더가 읽혀야 정확하다 — 하나라도 못 읽으면 None(부분합 과소 방지).

    한 디렉토리에 **여러 벌의 체크포인트가 공존**할 수 있다(예: LFM2-8B-A1B 는 -of-00004
    BF16 세트와 -of-00007 F32 세트가 같이 있어 glob 으로 다 세면 2x 이중집계). 따라서
    index.json 의 weight_map 이 가리키는 '정본 샤드 집합' 만 센다(있을 때). 없으면 glob 폴백.
    """
    # index 가 정본 샤드 집합을 정의 → 대체 정밀도/샤딩 사본 이중집계 방지.
    referenced: list[str] | None = None
    index_path = os.path.join(host_path, "model.safetensors.index.json")
    if os.path.isfile(index_path):
        try:
            with open(index_path, "r", encoding="utf-8") as f:
                idx = json.load(f)
            weight_map = idx.get("weight_map")
            if isinstance(weight_map, dict) and weight_map:
                referenced = sorted(
                    {os.path.join(host_path, v) for v in weight_map.values()}
                )
        except (OSError, ValueError, json.JSONDecodeError):
            referenced = None

    files = referenced if referenced else sorted(
        glob.glob(os.path.join(host_path, "*.safetensors"))
    )
    if not files:
        return None
    total = 0
    for path in files:
        header = _read_safetensors_header(path)
        if not header:
            return None  # 샤드 누락 → 과소합 위험, 폴백시킨다.
        for name, meta in header.items():
            if name == "__metadata__" or not isinstance(meta, dict):
                continue
            shape = meta.get("shape")
            if not isinstance(shape, list):
                continue
            numel = 1
            for d in shape:
                try:
                    numel *= int(d)
                except (TypeError, ValueError):
                    numel = 0
                    break
            total += numel
    return total if total > 0 else None


def count_mxfp4_params(host_path: str) -> dict:
    """명시된 MXFP4의 헤더만 검증·집계한다(미지원 구조는 근사하지 않는다).

    OpenAI gpt_oss/torch/weights.py 및 Transformers integrations/mxfp4.py:
    U8 blocks 마지막 축 16 = FP4 32개, U8 scales.shape = blocks.shape[:-1].
    scales는 exponent 메타데이터이며 일반 부동소수 텐서와 별도로 센다.
    호출자는 quant_method=mxfp4를 확인해야 한다. 모델 이름/총계는 사용하지 않는다.
    """
    def fail(reason):
        raise ValueError("MXFP4_HEADER_LAYOUT_UNSUPPORTED: " + reason)

    def unique_keys(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                fail(f"JSON 중복 키: {key}")
            result[key] = value
        return result

    index_path = os.path.join(host_path, "model.safetensors.index.json")
    weight_map = None
    if os.path.exists(index_path):
        try:
            with open(index_path, encoding="utf-8") as f:
                weight_map = json.load(f, object_pairs_hook=unique_keys).get("weight_map")
        except (OSError, ValueError, AttributeError) as exc:
            fail(f"index 읽기 실패: {exc}")
        if not isinstance(weight_map, dict) or not weight_map:
            fail("index weight_map 부재/부정합")
        if any(not isinstance(k, str) or not k or not isinstance(v, str)
               or os.path.basename(v) != v or not v.endswith(".safetensors")
               for k, v in weight_map.items()):
            fail("index 텐서/샤드 이름 부정합")
        files = sorted(set(weight_map.values()))
    else:
        files = sorted(os.path.basename(p) for p in
                       glob.glob(os.path.join(host_path, "*.safetensors")))
    if not files:
        fail("정본 safetensors 샤드 부재")

    tensors = {}
    payload_bytes = file_bytes = storage_numel = 0
    float_types = {"BF16", "F16", "F32", "F64"}
    for shard in files:
        path = os.path.join(host_path, shard)
        try:
            size = os.path.getsize(path)
            with open(path, "rb") as f:
                n = struct.unpack("<Q", f.read(8))[0]
                if n > _SAFETENSORS_MAX_HEADER or n > size - 8:
                    fail(f"{shard}: 헤더가 크기 상한/파일 경계를 초과")
                header = json.loads(f.read(n), object_pairs_hook=unique_keys)
        except (OSError, ValueError, struct.error) as exc:
            fail(f"{shard}: 헤더 읽기 실패: {exc}")
        if not isinstance(header, dict):
            fail(f"{shard}: 헤더 객체 부재")
        spans = []
        for name, meta in header.items():
            if name == "__metadata__":
                continue
            if name in tensors or not isinstance(meta, dict):
                fail(f"{name}: 중복 텐서/헤더 부정합")
            if weight_map is not None and weight_map.get(name) != shard:
                fail(f"{name}: index와 실제 샤드 불일치")
            dt, shape, offsets = meta.get("dtype"), meta.get("shape"), meta.get("data_offsets")
            if not isinstance(dt, str) or dt not in float_types | {"U8"}:
                fail(f"{name}: 미지원 dtype {dt!r}")
            if not isinstance(shape, list) or any(type(d) is not int or d <= 0 for d in shape):
                fail(f"{name}: shape 부정합")
            numel = 1
            for d in shape:
                numel *= d
            if not (isinstance(offsets, list) and len(offsets) == 2
                    and all(type(o) is int and o >= 0 for o in offsets)
                    and offsets[1] - offsets[0] == numel * _SAFETENSORS_DTYPE_ELEM_BYTES[dt]
                    and offsets[1] <= size - 8 - n):
                fail(f"{name}: dtype/shape/offset/파일 경계 불일치")
            spans.append(tuple(offsets))
            tensors[name] = (dt, shape, numel)
            storage_numel += numel
        end = 0
        for start, stop in sorted(spans):
            if start != end:
                fail(f"{shard}: payload 중복/빈 구간")
            end = stop
        if end != size - 8 - n:
            fail(f"{shard}: payload와 헤더 크기 불일치")
        payload_bytes += end
        file_bytes += size
    if weight_map is not None and set(tensors) != set(weight_map):
        fail("index가 선언한 텐서 누락")

    unpacked = packed = scales = pairs = expert_pairs = 0
    consumed = set()
    for name, (dt, shape, numel) in tensors.items():
        suffix = next((s for s in ("_blocks", ".blocks") if name.endswith(s)), None)
        if suffix is None:
            continue
        scale_name = name[:-len(suffix)] + suffix.replace("blocks", "scales")
        scale = tensors.get(scale_name)
        if (dt != "U8" or len(shape) < 2 or shape[-1] != 16 or scale is None
                or scale[0] != "U8" or shape[:-1] != scale[1]):
            fail(f"{name}: U8 blocks[...,16]/scales[...] 쌍 부정합")
        consumed.update((name, scale_name))
        packed += numel
        scales += scale[2]
        pairs += 1
        expert_pairs += int("expert" in name.lower() and "shared" not in name.lower())
    if not pairs:
        fail("지원하는 blocks/scales 쌍 부재")
    for name, (dt, _shape, numel) in tensors.items():
        if name in consumed:
            continue
        if dt not in float_types or name.endswith(("_scales", ".scales")):
            fail(f"{name}: 미확인 packed 텐서/고아 scale")
        unpacked += numel
    return {
        "format": "mxfp4-blocks16-u8-scales-u8",
        "num_shards": len(files), "tensor_count": len(tensors),
        "logical_params": unpacked + 2 * packed,
        "unpacked_params": unpacked, "packed_storage_numel": packed,
        "scale_metadata_numel": scales, "storage_numel": storage_numel,
        "payload_bytes": payload_bytes, "file_bytes": file_bytes,
        "packed_tensor_count": pairs, "packed_expert_tensor_count": expert_pairs,
    }


def parse(model_path: str, nas_host_root: str = DEFAULT_NAS_HOST_ROOT,
          nas_container_root: str = "/app/models") -> dict:
    """config.json 을 결정론으로 파싱해 정규화 dict 를 반환.

    model_path: 컨테이너 경로(<nas_container_root>/<Org>/<Name>) 또는 호스트 경로.
    nas_host_root: nas_container_root 가 매핑되는 호스트 NAS 루트.
    nas_container_root: 컨테이너 마운트 prefix(기본 /app/models · quant_model 마운트면 /app/quant_models).
    """
    warnings: list[str] = []

    # 1) host_path 해소 + 디렉토리/config.json 존재 검사. 부재 → 명시적 에러(다운로드 금지).
    host_path = _resolve_host_path(model_path, nas_host_root, nas_container_root)
    if not os.path.isdir(host_path):
        raise FileNotFoundError(
            f"모델 디렉토리 없음: {host_path} "
            f"(컨테이너 경로 {model_path}). 폐쇄망 — 모델 자체 다운로드 금지. "
            f"NAS에 사전 배치 후 재시도."
        )
    config_path = os.path.join(host_path, "config.json")
    if not os.path.isfile(config_path):
        raise FileNotFoundError(
            f"config.json 없음: {config_path}. 폐쇄망 — 다운로드 금지. 중단·보고."
        )

    # 2) config.json 로드 + text_config 중첩 처리.
    with open(config_path, "r", encoding="utf-8") as f:
        config = json.load(f)

    text_config = config.get("text_config")
    if isinstance(text_config, dict):
        # 아키텍처 필드는 text_config 우선, top-level fallback. vision_config 는 무시.
        def field(key, default=None):
            if key in text_config:
                return text_config.get(key, default)
            return config.get(key, default)
    else:
        text_config = {}

        def field(key, default=None):
            return config.get(key, default)

    # 3) 아키텍처 필드 추출 (누락 시 fallback/경고).
    hidden_size = field("hidden_size")
    num_hidden_layers = field("num_hidden_layers")
    num_attention_heads = field("num_attention_heads")

    if hidden_size is None:
        warnings.append("hidden_size 누락 (필수)")
    if num_hidden_layers is None:
        warnings.append("num_hidden_layers 누락 (필수)")
    if num_attention_heads is None:
        warnings.append("num_attention_heads 누락 (필수)")

    num_key_value_heads = field("num_key_value_heads")
    if num_key_value_heads is None:
        # MHA: num_key_value_heads = num_attention_heads.
        num_key_value_heads = num_attention_heads
        if num_attention_heads is not None:
            warnings.append(
                "num_key_value_heads 누락 → num_attention_heads 사용 (MHA 가정)"
            )

    # MLA(Multi-head Latent Attention) 신호 인지: DeepSeek/GLM-4.x-MoE 계열은 config 에
    # head_dim 키 없이 qk_nope_head_dim/qk_rope_head_dim/kv_lora_rank 로 attention 차원을
    # 표현한다. 이때 hidden//heads 산출은 의미 없는 값(예: GLM-4.7-Flash 2048//20=102).
    qk_nope_head_dim = field("qk_nope_head_dim")
    qk_rope_head_dim = field("qk_rope_head_dim")
    kv_lora_rank = field("kv_lora_rank")
    is_mla = (
        qk_nope_head_dim is not None
        or qk_rope_head_dim is not None
        or kv_lora_rank is not None
    )

    head_dim = field("head_dim")
    if head_dim is None:
        if is_mla and qk_nope_head_dim is not None and qk_rope_head_dim is not None:
            # 실제 attention head dim = qk_nope_head_dim + qk_rope_head_dim.
            # (KV_cache 공식은 MLA 압축 KV 를 반영하지 못해 과대추정이 되지만, 방향은
            #  보수적[과대=OOM 안전]이다. 압축 미반영을 아래 경고로 명시한다.)
            head_dim = int(qk_nope_head_dim) + int(qk_rope_head_dim)
            warnings.append(
                "MLA 감지(qk_nope=%s, qk_rope=%s, kv_lora_rank=%s) → head_dim=%s "
                "사용. KV_cache 공식은 MLA 압축 KV 미반영 → 과대추정(보수적·OOM 안전)"
                % (qk_nope_head_dim, qk_rope_head_dim, kv_lora_rank, head_dim)
            )
        elif hidden_size is not None and num_attention_heads:
            head_dim = hidden_size // num_attention_heads
            if is_mla:
                # MLA 신호는 있으나 qk_nope/qk_rope 가 불완전 → hidden//heads 폴백은
                # 근거 없는 값일 수 있음. 조용한 오류 금지(명시 경고).
                warnings.append(
                    "head_dim 누락 + MLA 신호(kv_lora_rank=%s) 감지지만 "
                    "qk_nope/qk_rope_head_dim 불완전 → hidden_size // num_attention_heads "
                    "=%s 폴백(근거 약함, KV_cache 과대/오추정 가능)"
                    % (kv_lora_rank, hidden_size // num_attention_heads)
                )
            else:
                warnings.append(
                    "head_dim 누락 → hidden_size // num_attention_heads 사용"
                )
        else:
            warnings.append(
                "head_dim 누락 + hidden_size/num_attention_heads 부족 → 산출 불가"
            )

    # ── 3.5) 하이브리드 attention 층수 인지 (2026-08-23 · plan_26082223 결함 A) ─────────
    #   순수 판정은 `resolve_kv_layers()` 가 소유한다(파일 IO 없음 → `--self-test` 가 이걸 친다).
    layer_types = field("layer_types")
    full_attention_interval = field("full_attention_interval")
    num_full_attention_layers, is_hybrid, layer_type_counts, _lt_warnings = resolve_kv_layers(
        layer_types, num_hidden_layers, full_attention_interval
    )
    warnings.extend(_lt_warnings)

    vocab_size = field("vocab_size")
    if vocab_size is None:
        warnings.append("vocab_size 누락")
    max_position_embeddings = field("max_position_embeddings")
    if max_position_embeddings is None:
        warnings.append("max_position_embeddings 누락")

    model_type = field("model_type") or config.get("model_type")
    architectures = field("architectures") or config.get("architectures")

    # 4) dtype 해소: torch_dtype | dtype | text_config.dtype → 기본 'bfloat16'(+warning).
    native_dtype = (
        config.get("torch_dtype")
        or config.get("dtype")
        or text_config.get("dtype")
        or text_config.get("torch_dtype")
    )
    if native_dtype is None:
        native_dtype = "bfloat16"
        warnings.append("torch_dtype/dtype 누락 → 'bfloat16' 기본 가정")
    serve_bpw, dtype_warn = dtype_bpw(native_dtype)
    if dtype_warn:
        warnings.append(dtype_warn)

    # 5) 양자화: top-level 또는 text_config 의 quantization_config 있으면 prequantized.
    quant_config = config.get("quantization_config")
    if not isinstance(quant_config, dict):
        quant_config = text_config.get("quantization_config")
    prequantized = isinstance(quant_config, dict)
    quant_method_native = None
    if prequantized:
        quant_method_native = quant_config.get("quant_method")

    # 6) is_moe: 전문가 수 키 존재 또는 model_type 에 'moe' 포함.
    #
    # ★ 키 이름은 publisher 마다 다르다. HF 표준은 `num_experts`/`num_local_experts` 지만
    #   **DeepSeek 계열은 `n_routed_experts`** 를 쓴다(+ `n_shared_experts` 는 공유전문가로 별개).
    #   그 결과 DeepSeek-V4-Flash 계열이 전부 `is_moe=False` 로 오판됐다 — 2026-08-01
    #   DeepSeek-V4-Flash-0731(n_routed_experts=256, num_experts_per_tok=6) 실측으로 발견.
    #   오판의 실해악: MoE 백엔드 선택(humming/triton/marlin)·KV 공식·루프라인 산정이 전부
    #   dense 가정으로 흐른다. 이 프로젝트에서 그 셋은 각각 다른 실패로 이어진 전례가 있다.
    #   ※ 지금까지 DeepSeek 서빙이 성공한 건 사람/에이전트가 알고 우회했기 때문이지 도구가
    #     맞았기 때문이 아니다 — 지식이 도구로 전파되지 않은 전형이다.
    #   routed 를 우선한다: MoE 라우팅 폭은 routed 전문가 수이고, shared 는 상시활성이라 폭이 아니다.
    _EXPERT_COUNT_KEYS = ("num_experts", "num_local_experts", "n_routed_experts")
    num_experts = None
    for _k in _EXPERT_COUNT_KEYS:
        if num_experts is None:
            num_experts = field(_k)
    num_experts_per_tok = field("num_experts_per_tok")
    is_moe = (
        num_experts is not None
        or (isinstance(model_type, str) and "moe" in model_type.lower())
    )

    # 7) weight 바이트 원천 + disk_bpw 실측.
    # 명시 MXFP4는 미지원/깨진 헤더를 legacy dtype 추정에 넘기기 전에 검증한다.
    parameter_count = count_mxfp4_params(host_path) if quant_method_native == "mxfp4" else None
    native_weight_bytes = _native_weight_bytes(host_path, warnings)

    # disk_bpw 실측: 가장 큰 *.safetensors 헤더 읽어 온디스크 dtype 판정.
    disk_dtype: str | None = None
    disk_bpw: float | None = None
    largest = _largest_safetensors(host_path)
    if largest is not None:
        disk_dtype, disk_bpw = _measure_disk_dtype(largest)
    if disk_bpw is None:
        # 헤더 못 읽음(또는 safetensors 부재) → serve_bpw 폴백 + 경고.
        disk_bpw = serve_bpw
        warnings.append(
            "safetensors 헤더에서 disk dtype 실측 실패 → disk_bpw=serve_bpw 폴백"
        )

    # disk_dtype 와 native_dtype(config) 불일치는 혼합 정밀도 신호(조용한 오류 금지).
    # disk_bpw 는 '가장 큰 단일 샤드의 가장 큰 텐서' 1개 dtype 으로 정해지는데, 혼합
    # 정밀도 모델(예: LFM2-8B-A1B 의 최대 샤드는 F32 conv 텐서로 채워짐)에서는 전체
    # 모델 dtype 과 다를 수 있어 num_params 가 2x 오산될 수 있다.
    _disk_native_mismatch = False
    if disk_dtype is not None:
        # serve dtype(config) bpw 와 disk_bpw 가 다르면 불일치로 본다.
        if abs(float(disk_bpw) - float(serve_bpw)) > 1e-9:
            _disk_native_mismatch = True
            warnings.append(
                "disk_dtype(%s, bpw=%s) != native_dtype(%s, bpw=%s) — 저장 정밀도 ≠ "
                "config dtype. "
                % (disk_dtype, disk_bpw, native_dtype, serve_bpw)
                + ("num_params 는 MXFP4 blocks/scales 논리 원소 검증값(미지원 구조는 중단)"
                   if parameter_count is not None else
                   "num_params 는 헤더 numel 합(정확) 우선, 불가 시 보수적 bpw 폴백")
            )

    # num_params 산출. MXFP4는 혼합 저장 바이트/bpw가 아니라 검증된 논리 원소를 센다.
    if parameter_count is not None:
        num_params = parameter_count["logical_params"]
        weight_basis = "safetensors_mxfp4_logical_numel(exact)"
    elif native_weight_bytes is None:
        num_params = None
        weight_basis = None
        warnings.append(
            "native_weight_bytes 부재 → num_params=None (estimate 단계 에러표시)"
        )
    elif prequantized:
        # prequantized: native weight 를 양자화 bpw 로 나눠 best-effort 파라미터 수.
        # 미지 quant_method(테이블 미등록)면 ValueError 로 죽지 않고(=비결정 traceback 금지)
        # disk_bpw 실측으로 폴백한다(명시 경고). MXFP4는 위의 검증된 별도 경로다.
        bpw, qwarn = vllm_quant_bpw_or_none(quant_method_native, serve_bpw)
        if bpw is None:
            warnings.append(
                "%s → disk_bpw(%s) 실측으로 num_params 폴백(best-effort)"
                % (qwarn, disk_bpw)
            )
            num_params = round(native_weight_bytes / disk_bpw)
            weight_basis = "disk_total/disk_bpw(prequant_unknown_method)"
        else:
            num_params = round(native_weight_bytes / bpw)
            weight_basis = "disk_total_prequant"
    else:
        # 비prequantized: 헤더 텐서별 numel 합 = 저장 dtype 무관 '정확한' 파라미터 수.
        # (total_size/단일bpw 는 혼합 정밀도에서 2x 오산 — Motif-2.6B 가 F32 저장이라
        #  disk_bpw=4 면 정확하지만, LFM2-8B 처럼 최대 텐서만 F32 면 과소산. numel 합은 둘 다 정확.)
        exact = _count_params_from_headers(host_path)
        if exact is not None:
            num_params = exact
            weight_basis = "safetensors_numel_sum(exact)"
        elif _disk_native_mismatch:
            # 헤더 numel 합 불가 + 혼합 정밀도 → 두 경로 중 큰 쪽(보수적·OOM 안전).
            np_disk = round(native_weight_bytes / disk_bpw)
            np_native = round(native_weight_bytes / serve_bpw)
            num_params = max(np_disk, np_native)
            weight_basis = (
                "disk_total/serve_bpw(mixed_dtype_conservative)"
                if num_params == np_native
                else "disk_total/disk_bpw(mixed_dtype_conservative)"
            )
        else:
            num_params = round(native_weight_bytes / disk_bpw)
            weight_basis = "disk_total/disk_bpw"

    # 8) JSON 출력 (키 고정).
    return {
        "model_path_container": model_path,
        "model_path_host": host_path,
        "model_id": _model_id_from_path(model_path),
        "model_type": model_type,
        "architectures": architectures,
        "is_moe": is_moe,
        "native_dtype": native_dtype,
        "serve_bpw": serve_bpw,
        "disk_dtype": disk_dtype,
        "disk_bpw": disk_bpw,
        "prequantized": prequantized,
        "quant_method_native": quant_method_native,
        "num_hidden_layers": num_hidden_layers,
        # 하이브리드 KV 층수(plan_26082223 결함 A). None = layer_types 부재/부정합 →
        # 소비자(estimate_vram.kv_bearing_layers)가 전층 폴백한다.
        "num_full_attention_layers": num_full_attention_layers,
        "is_hybrid": is_hybrid,
        "layer_type_counts": layer_type_counts,
        "full_attention_interval": full_attention_interval,
        "num_attention_heads": num_attention_heads,
        "num_key_value_heads": num_key_value_heads,
        "head_dim": head_dim,
        "hidden_size": hidden_size,
        "vocab_size": vocab_size,
        "max_position_embeddings": max_position_embeddings,
        "num_experts": num_experts,
        "num_experts_per_tok": num_experts_per_tok,
        "native_weight_bytes": native_weight_bytes,
        "num_params": num_params,
        "weight_basis": weight_basis,
        **({"parameter_count": parameter_count} if parameter_count is not None else {}),
        "warnings": warnings,
    }


# ===========================================================================
# 자체검사 (--self-test) — 하이브리드 KV + 소형 MXFP4 파일 fixture.
#   실제 모델/하드웨어/네트워크 없이 정본 parser→crosscheck 경로를 확인한다.
# ===========================================================================

def _mxfp4_self_tests(check):
    import contextlib
    import copy
    import io
    import tempfile
    from pathlib import Path
    from unittest.mock import patch
    import crosscheck_model_card as C
    import parse_model_config as P

    # 서로 다른 샤드에 pair를 두며 일반 BF16 10개 + packed FP4 64개 = 논리 74개.
    blocks = "model.experts.proj_blocks"
    scales = "model.experts.proj_scales"
    base = {
        "a.safetensors": {"norm.weight": {"dtype": "BF16", "shape": [10], "data_offsets": [0, 20]},
                          blocks: {"dtype": "U8", "shape": [2, 16], "data_offsets": [20, 52]}},
        "b.safetensors": {scales: {"dtype": "U8", "shape": [2], "data_offsets": [0, 2]}},
    }
    cfg = {"model_type": "fixture", "hidden_size": 16, "num_hidden_layers": 2,
           "num_attention_heads": 2, "torch_dtype": "bfloat16",
           "quantization_config": {"quant_method": "mxfp4"}}
    with tempfile.TemporaryDirectory(prefix="mxfp4-headers-") as tmp:
        root = Path(tmp)

        def write(headers=None, config=None, mapping=None):
            for path in root.iterdir():
                path.unlink()
            headers = copy.deepcopy(base if headers is None else headers)
            for shard, header in headers.items():
                encoded = json.dumps(header).encode()
                length = max(meta["data_offsets"][1] for meta in header.values())
                (root / shard).write_bytes(struct.pack("<Q", len(encoded)) + encoded + bytes(length))
            wm = {name: shard for shard, hdr in headers.items() for name in hdr}
            (root / "model.safetensors.index.json").write_text(json.dumps({"weight_map": wm if mapping is None else mapping}))
            (root / "config.json").write_text(json.dumps(cfg if config is None else config))
            return str(root)

        def reject(name, headers=None, alter=None):
            write(headers)
            if alter:
                alter()
            try:
                parse(str(root))
            except ValueError as exc:
                check(name, "MXFP4_HEADER_LAYOUT_UNSUPPORTED" in str(exc), True)
            else:
                check(name, "accepted", "rejected")

        write()
        observed = parse(str(root))
        details = observed["parameter_count"]
        check("M1.logical", observed["num_params"], 74)
        check("M1.storage", details["storage_numel"], 44)
        check("M1.scale_metadata", details["scale_metadata_numel"], 2)
        check("M1.payload", details["payload_bytes"], 54)
        check("M1.physical", observed["native_weight_bytes"], details["file_bytes"])
        check("M1.headers_not_params", observed["num_params"] != round(details["file_bytes"] / .5), True)
        (root / "unreferenced.safetensors").write_bytes(b"not the canonical checkpoint")
        check("M2.index_canonical", parse(str(root))["parameter_count"], details)

        dotted = {shard: {name.replace("_blocks", ".blocks").replace("_scales", ".scales"): meta
                          for name, meta in hdr.items()} for shard, hdr in base.items()}
        write(dotted)
        check("M3.dot_loader_naming", parse(str(root))["num_params"], 74)
        (root / "model.safetensors.index.json").unlink()
        check("M3.no_index_all_shards", parse(str(root))["num_params"], 74)

        bad = copy.deepcopy(base)
        bad["b.safetensors"] = {"opaque_u8": bad["b.safetensors"][scales]}
        reject("M4.unknown_u8_missing_pair", bad)
        bad = copy.deepcopy(base)
        bad["b.safetensors"]["orphan_scales"] = {"dtype": "U8", "shape": [1], "data_offsets": [2, 3]}
        reject("M5.orphan_scale", bad)
        bad = copy.deepcopy(base)
        bad["b.safetensors"][scales]["shape"] = [1, 2]
        reject("M6.pair_shape", bad)
        bad = copy.deepcopy(base)
        bad["b.safetensors"][scales] = {"dtype": "F16", "shape": [2], "data_offsets": [0, 4]}
        reject("M7.scale_dtype", bad)
        bad = copy.deepcopy(base)
        bad["a.safetensors"][blocks]["shape"] = [4, 8]
        bad["b.safetensors"][scales] = {"dtype": "U8", "shape": [4], "data_offsets": [0, 4]}
        reject("M8.block_width", bad)
        bad = copy.deepcopy(base)
        bad["a.safetensors"][blocks]["data_offsets"] = [20, 51]
        reject("M9.payload_length", bad)
        bad = copy.deepcopy(base)
        bad["a.safetensors"][blocks]["data_offsets"] = [19, 51]
        reject("M10.overlap", bad)
        reject("M11.truncated", alter=lambda: (root / "a.safetensors").write_bytes((root / "a.safetensors").read_bytes()[:-1]))
        reject("M12.missing_shard", alter=lambda: (root / "b.safetensors").unlink())
        reject("M13.all_shards_absent", alter=lambda: [(root / p).unlink() for p in base])
        reject("M14.malformed_index", alter=lambda: (root / "model.safetensors.index.json").write_text("{}"))
        mapping = {name: shard for shard, hdr in base.items() for name in hdr}
        mapping[scales] = "a.safetensors"
        reject("M15.wrong_index_map", alter=lambda: (root / "model.safetensors.index.json").write_text(json.dumps({"weight_map": mapping})))
        mapping = {name: shard for shard, hdr in base.items() for name in hdr}
        mapping["missing.weight"] = "a.safetensors"
        reject("M16.missing_index_tensor", alter=lambda: (root / "model.safetensors.index.json").write_text(json.dumps({"weight_map": mapping})))

        write()
        with patch.object(C, "fetch_hf_safetensors_total", return_value={"ok": True, "total": 74, "parameters": {}}):
            result = C.crosscheck(str(root), "fixture/model", local_num_params=74)
            check("M17.valid_crosscheck", result["verdict"], "OK")
            check("M17.mixed_evidence", result["dtype_scan"]["is_mixed_precision"], True)
            check("M17.expert_evidence", result["dtype_scan"]["fourbit_experts"], True)
            check("M18.real_external_mismatch", C.crosscheck(str(root), "fixture/model", local_num_params=100)["verdict"], "MISMATCH")
            check("M18.tolerance_unchanged", C.crosscheck_external_vram(77, None, "fixture/model")["verdict"], "OK")
            check("M19.ephemeral_unchanged", C.crosscheck_external_vram(None, None, "fixture/model")["mode"], "ephemeral-estimate")
        with patch.object(C, "fetch_hf_safetensors_total", return_value={"ok": False, "reason": "unavailable"}):
            check("M20.optional_unavailable", C.crosscheck_external_vram(74, None, "fixture/model")["verdict"], "UNAVAILABLE")
        with patch.object(P, "parse", side_effect=ValueError("local observation failed")), \
             patch.object(C, "fetch_hf_safetensors_total", side_effect=AssertionError("must not estimate")), \
             patch.object(sys, "argv", ["crosscheck", str(root), "--hf-repo-id", "fixture/model", "--json"]), \
             contextlib.redirect_stdout(io.StringIO()) as out, contextlib.redirect_stderr(io.StringIO()):
            try:
                C.main()
            except SystemExit as exc:
                check("M21.managed_parse_failure_exit", exc.code, 2)
            check("M21.managed_failure_preserved", json.loads(out.getvalue())["checks"][-1]["reason"], "local_parse_failed")
        (root / "b.safetensors").unlink()
        check("M22.layout_gate_without_external", C.crosscheck(str(root))["verdict"], "MISMATCH")
        with contextlib.redirect_stderr(io.StringIO()):
            check("M22.parse_cli_failure", _main([str(root), "--json"]), 2)

        # 다른 형식/선언 없음에 U8만 보고 MXFP4를 부여하지 않는다.
        undeclared = dict(cfg)
        undeclared.pop("quantization_config")
        write(config=undeclared)
        check("M23.undeclared_u8", C.crosscheck(str(root))["dtype_scan"]["fourbit_experts"], False)
        high_mixed = {"a.safetensors": {
            "experts.packed": {"dtype": "U8", "shape": [16], "data_offsets": [0, 16]},
            "experts.scale": {"dtype": "F8_E8M0", "shape": [1], "data_offsets": [16, 17]},
            "norm": {"dtype": "BF16", "shape": [1], "data_offsets": [17, 19]}}}
        coarse = {**cfg, "quantization_config": {"quant_method": "fp8"}}
        write(high_mixed, config=coarse)
        check("M24.other_coarse_gate", C.crosscheck(str(root))["verdict"], "MISMATCH")
        dense = {"a.safetensors": {"weight": {"dtype": "BF16", "shape": [10], "data_offsets": [0, 20]}}}
        write(dense, config=undeclared)
        check("M25.dense_count_unchanged", parse(str(root))["num_params"], 10)
        check("M25.dense_fields_unchanged", "parameter_count" in parse(str(root)), False)

        def oversize_header():
            with (root / "a.safetensors").open("r+b") as f:
                f.write(struct.pack("<Q", _SAFETENSORS_MAX_HEADER + 1))
                f.truncate(_SAFETENSORS_MAX_HEADER + 9)  # sparse: payload를 쓰거나 읽지 않음
        reject("M26.header_read_bound", alter=oversize_header)
        check("M26.legacy_reader_bound", _read_safetensors_header(str(root / "a.safetensors")), None)

        def duplicate_header():
            path = root / "a.safetensors"
            data = path.read_bytes()
            n = struct.unpack("<Q", data[:8])[0]
            header = data[8:8+n].decode().replace('"dtype": "BF16"', '"dtype": "U8", "dtype": "BF16"')
            path.write_bytes(struct.pack("<Q", len(header)) + header.encode() + data[8+n:])
        reject("M27.duplicate_header_key", alter=duplicate_header)
        def duplicate_index():
            path = root / "model.safetensors.index.json"
            path.write_text(path.read_text().replace('"norm.weight": "a.safetensors"',
                           '"norm.weight": "b.safetensors", "norm.weight": "a.safetensors"'))
        reject("M28.duplicate_index_key", alter=duplicate_index)
        bad = copy.deepcopy(base)
        bad["a.safetensors"][blocks] = {"dtype": "U8", "shape": [16], "data_offsets": [20, 36]}
        bad["b.safetensors"][scales] = {"dtype": "U8", "shape": [], "data_offsets": [0, 1]}
        reject("M29.loader_requires_group_axis", bad)


def _self_test() -> int:
    failures: list[str] = []

    def check(name, got, want):
        if got != want:
            failures.append("%s: got=%r want=%r" % (name, got, want))

    # A1 — Qwen3.8-27B 실물 형태: 64층 중 full 16 · linear 48.
    lt = ["full_attention" if (i + 1) % 4 == 0 else "linear_attention" for i in range(64)]
    n, hyb, counts, warns = resolve_kv_layers(lt, 64, 4)
    check("A1.num_full", n, 16)
    check("A1.is_hybrid", hyb, True)
    check("A1.counts", counts, {"linear_attention": 48, "full_attention": 16})
    check("A1.no_warning", warns, [])

    # A2 — 전층 full_attention(비하이브리드): 값은 나오되 is_hybrid=False.
    n, hyb, _c, warns = resolve_kv_layers(["full_attention"] * 32, 32, None)
    check("A2.num_full", n, 32)
    check("A2.is_hybrid", hyb, False)
    check("A2.no_warning", warns, [])

    # A3 — layer_types 부재(구형 config): None → 소비자가 전층 폴백(회귀 0).
    n, hyb, counts, warns = resolve_kv_layers(None, 64, None)
    check("A3.num_full", n, None)
    check("A3.is_hybrid", hyb, False)
    check("A3.counts", counts, None)

    # A4 — 길이 불일치는 **조용히 쓰지 않는다**: 전층 폴백 + 경고.
    n, hyb, _c, warns = resolve_kv_layers(["full_attention"] * 10, 64, None)
    check("A4.num_full", n, None)
    if not any("길이" in w for w in warns):
        failures.append("A4: 길이 불일치 경고 누락 — 침묵 폴백은 금지다")

    # A5 — KV 보유층 0 은 KV=0(게이트 무조건 통과) 이므로 전층 폴백 + 경고.
    n, _h, _c, warns = resolve_kv_layers(["linear_attention"] * 8, 8, None)
    check("A5.num_full", n, None)
    if not any("KV 보유층이 0" in w for w in warns):
        failures.append("A5: KV 보유층 0 경고 누락 — 과소추정이 조용히 통과한다")

    # A6 — 미지 타입은 **KV 보유로 센다**(보수적) + 경고. sliding_attention 이 실사례.
    lt = ["sliding_attention"] * 4 + ["full_attention"] * 4 + ["linear_attention"] * 8
    n, hyb, _c, warns = resolve_kv_layers(lt, 16, None)
    check("A6.num_full", n, 8)
    check("A6.is_hybrid", hyb, True)
    if not any("미지 타입" in w for w in warns):
        failures.append("A6: 미지 타입 경고 누락 — 조용한 보수화도 침묵이다")

    # A7 — interval tripwire: layer_types 가 권위, 불일치는 경고로 드러낸다.
    lt = ["full_attention" if (i + 1) % 4 == 0 else "linear_attention" for i in range(64)]
    n, _h, _c, warns = resolve_kv_layers(lt, 64, 8)   # interval 8 → 기대 8 ≠ 집계 16
    check("A7.num_full", n, 16)
    if not any("full_attention_interval" in w for w in warns):
        failures.append("A7: interval 교차검증 경고 누락 — tripwire 가 죽었다")

    # A8 — 소비자 계약: estimate_vram 이 이 값을 실제로 집어 KV 를 1/4 로 만든다.
    #      (도구를 만든 것과 도는 것은 다르다 — 끝단까지 실측한다.)
    import estimate_vram as _E
    parsed_hybrid = {"num_hidden_layers": 64, "num_full_attention_layers": 16,
                     "num_key_value_heads": 4, "head_dim": 256}
    parsed_dense = {"num_hidden_layers": 64, "num_key_value_heads": 4, "head_dim": 256}
    check("A8.hybrid_per_token", _E.per_token_kv_bytes(parsed_hybrid, 2), 65536)
    check("A8.dense_per_token", _E.per_token_kv_bytes(parsed_dense, 2), 262144)

    _mxfp4_self_tests(check)

    if failures:
        sys.stderr.write("[parse_model_config --self-test] FAIL %d 건:\n" % len(failures))
        for f in failures:
            sys.stderr.write("  - %s\n" % f)
        return 1
    sys.stdout.write("[parse_model_config --self-test] OK — A1~A8 + M1~M29 통과"
                     " (하이브리드 KV · MXFP4 헤더/관리형 교차검증)\n")
    return 0


def _main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description="config.json 결정론 파서 (vllm-recipe-explorer)"
    )
    ap.add_argument("path", nargs="?", help="모델 경로 (/app/models/<Org>/<Name> 또는 호스트 경로)")
    ap.add_argument("--self-test", action="store_true",
                    help="하이브리드 KV·MXFP4 헤더/교차검증 회귀(모델/하드웨어/네트워크 불요)")
    ap.add_argument(
        "--nas-root",
        default=DEFAULT_NAS_HOST_ROOT,
        help=f"/app/models 매핑 호스트 NAS 루트 (기본 {DEFAULT_NAS_HOST_ROOT})",
    )
    ap.add_argument("--json", action="store_true", help="JSON 으로 출력")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()
    if not args.path:
        ap.error("path 는 필수다(--self-test 는 예외)")

    try:
        result = parse(args.path, nas_host_root=args.nas_root)
    except FileNotFoundError as e:
        print(f"[parse_model_config] 중단: {e}", file=sys.stderr)
        return 2
    except ValueError as e:
        # 미지 quant_method 등 결정론 해소 실패 — traceback 금지, 명확한 중단·보고로 통일.
        print(f"[parse_model_config] 중단(해소 실패): {e}", file=sys.stderr)
        return 2

    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        for k, v in result.items():
            if k == "warnings":
                continue
            print(f"{k:28} {v}")
        if result["warnings"]:
            print("--- warnings ---")
            for w in result["warnings"]:
                print(f"  ! {w}")
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())
