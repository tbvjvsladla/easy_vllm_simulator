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


def _resolve_host_path(model_path: str, nas_host_root: str) -> str:
    """컨테이너 경로(/app/models/...)면 NAS 루트로 치환. 그 외는 그대로."""
    prefix = "/app/models"
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

    우선순위: model.safetensors.index.json metadata.total_size →
    *.safetensors 크기 합 → *.bin 크기 합 → None+경고.
    """
    index_path = os.path.join(host_path, "model.safetensors.index.json")
    if os.path.isfile(index_path):
        try:
            with open(index_path, "r", encoding="utf-8") as f:
                idx = json.load(f)
            total = idx.get("metadata", {}).get("total_size")
            if isinstance(total, (int, float)) and total > 0:
                return int(total)
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


def parse(model_path: str, nas_host_root: str = DEFAULT_NAS_HOST_ROOT) -> dict:
    """config.json 을 결정론으로 파싱해 정규화 dict 를 반환.

    model_path: 컨테이너 경로(/app/models/<Org>/<Name>) 또는 호스트 경로.
    nas_host_root: /app/models 가 매핑되는 호스트 NAS 루트.
    """
    warnings: list[str] = []

    # 1) host_path 해소 + 디렉토리/config.json 존재 검사. 부재 → 명시적 에러(다운로드 금지).
    host_path = _resolve_host_path(model_path, nas_host_root)
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

    # 6) is_moe: num_experts/num_local_experts 존재 또는 model_type 에 'moe' 포함.
    num_experts = field("num_experts")
    if num_experts is None:
        num_experts = field("num_local_experts")
    num_experts_per_tok = field("num_experts_per_tok")
    is_moe = (
        num_experts is not None
        or field("num_local_experts") is not None
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


def _main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description="config.json 결정론 파서 (vllm-recipe-explorer)"
    )
    ap.add_argument("path", help="모델 경로 (/app/models/<Org>/<Name> 또는 호스트 경로)")
    ap.add_argument(
        "--nas-root",
        default=DEFAULT_NAS_HOST_ROOT,
        help=f"/app/models 매핑 호스트 NAS 루트 (기본 {DEFAULT_NAS_HOST_ROOT})",
    )
    ap.add_argument("--json", action="store_true", help="JSON 으로 출력")
    args = ap.parse_args(argv)

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
