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
                "config dtype. num_params 는 헤더 numel 합(정확) 우선, 불가 시 보수적 bpw 폴백"
                % (disk_dtype, disk_bpw, native_dtype, serve_bpw)
            )

    # num_params 산출.
    if native_weight_bytes is None:
        num_params = None
        weight_basis = None
        warnings.append(
            "native_weight_bytes 부재 → num_params=None (estimate 단계 에러표시)"
        )
    elif prequantized:
        # prequantized: native weight 를 양자화 bpw 로 나눠 best-effort 파라미터 수.
        # 미지 quant_method(테이블 미등록)면 ValueError 로 죽지 않고(=비결정 traceback 금지)
        # disk_bpw 실측으로 폴백한다(명시 경고). estimate 의 prequant 분기는
        # native_weight_bytes 만 쓰므로 num_params 부정확은 게이트 안전성에 영향 없음.
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
        "warnings": warnings,
    }


# ===========================================================================
# 자체검사 (--self-test) — 하이브리드 KV 층수 인지(plan_26082223 결함 A).
#   `resolve_kv_layers` 는 순수 함수라 파일 IO·모델·하드웨어가 필요 없다
#   (다른 스킬 도구의 `--self-test` 계약과 동일).
# ===========================================================================

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

    if failures:
        sys.stderr.write("[parse_model_config --self-test] FAIL %d 건:\n" % len(failures))
        for f in failures:
            sys.stderr.write("  - %s\n" % f)
        return 1
    sys.stdout.write("[parse_model_config --self-test] OK — A1~A8 통과"
                     " (하이브리드 KV 층수 인지 · plan_26082223 결함 A)\n")
    return 0


def _main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description="config.json 결정론 파서 (vllm-recipe-explorer)"
    )
    ap.add_argument("path", nargs="?", help="모델 경로 (/app/models/<Org>/<Name> 또는 호스트 경로)")
    ap.add_argument("--self-test", action="store_true",
                    help="하이브리드 KV 층수 인지 회귀(모델/하드웨어 불요)")
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
