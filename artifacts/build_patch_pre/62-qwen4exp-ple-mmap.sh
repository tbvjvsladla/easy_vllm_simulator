#!/bin/bash
# 62-qwen4exp-ple-mmap.sh — **소스 이식 패치**(build_patches_src 슬롯 · pre-compile)
#
# what : qwen4_exp 의 47.7GiB PLE n-gram 테이블을 **NVMe mmap**으로 서빙하는 경로를
#        stock vLLM(74c96922e)에 이식한다. `VLLM_PLE_MMAP=1` 일 때만 활성.
#        GB10 은 호스트/GPU 가 하나의 121GiB 풀이라 VLLM_PLE_CPU_OFFLOAD(미머지·pinned
#        host RAM)가 무의미하다 — 테이블을 페이지캐시/디스크로 빼는 mmap 만이 실제
#        메모리를 거둔다(camp-26090918 의 PLE=mmap 셀 전제 · 사용자 결정: +1/+1 자체이식).
#
# reference : github.com/blazux/qwen3.8-Flash-DGX `src/vllm_ple_mmap.py` (597줄)
#        + fast gather hot path(@Saren-Arterius). 싱글노드 실증: 상주 76GiB · 디코드 ~37 t/s.
#
# 우리 베이스 어댑테이션 (참조와 다른 6가지 — 전부 이 파일이 소유):
#   ① 클래스명  Qwen3_8FlashNextNGramEmbedding → Qwen4ExpNGramEmbedding
#   ② swap 대상  VocabParallelEmbedding → PLEVocabParallelEmbedding (우리 ctor 심볼)
#   ③ __init__ 시그니처  우리 트리는 layer_name 추가 파라미터
#   ④ 스케일 거처  참조는 `_offload_weight_scale`(NGramEmbedding) — 우리 트리의
#      `_get_embedding_weight_scale` 은 `ngram_embedding.weight_scale` 를 읽으므로
#      placeholder 에 `weight_scale` 로 등록(PLELayer 디콴프 경로 무수정)
#   ⑤ ★ TP=2 rank-aware masking + all-reduce ★ 참조는 TP=1 이라 전수 gather 후
#      그대로 반환필 — TP=2 에서 그렇게 하면 all-reduce 가 2배합산해 출력이 틀어진다.
#      placeholder 가 vocab_range_from_global_vocab_size 로 rank 구간을 계산해
#      구간 밖 id 는 0 으로 두고(디스크 읽기도 rank 몫으로 준다) 구간 내 전역행만
#      gather 한 뒤 tensor_model_parallel_all_reduce — stock VPE 의 의미론 재현.
#      **blazux 도 미검증한 영역이라 우리가 설계·스모크로 중재한다.**
#   ⑥ forward 래핑  참조 트리는 forward_impl 이었으나 우리 트리는 forward — 이미
#      custom op 패턴(qwen4_exp_compute_ple_ngram_ids)이 있어 같은 패턴으로
#      `vllm::qwen4_exp_ple_mmap_lookup` 을 등록(compile opaque · PIECEWISE splitting 대상).
#
# gate : 없음(ungated) — VLLM_PLE_MMAP 미설정 시 apply() 가 no-op 이라 바이트만 있고
#        행동은 stock 과 동일(60 과 같은 불활성 백포트 판정).
#
# probe: 최종 중재는 serve 스모크(TP=2)다. 스모크 구성은 enforce-eager(변수 분리) —
#        PIECEWISE+splitting_ops 는 셀 레버로 후속.
#
# plan : plan_26090918 §Phase 0 P0-2.
set -euo pipefail

TAG="[62-qwen4exp-ple-mmap]"
DST="/workspace/vllm-src"
MOD="$DST/vllm/models/qwen4_exp/nvidia/ple_mmap.py"
PLE="$DST/vllm/models/qwen4_exp/nvidia/ple_layer.py"

[ -d "$DST/vllm/models/qwen4_exp" ] || { echo "$TAG FAIL: qwen4_exp 트리 부재 — 베이스가 아니다" >&2; exit 1; }
echo "$TAG base HEAD: $(git -C "$DST" rev-parse HEAD 2>/dev/null || echo unknown)"

# 머지/중복 트립와이어
if [ -f "$MOD" ] || grep -q 'qwen4_exp_ple_mmap_lookup' "$PLE"; then
    echo "$TAG FAIL: ple_mmap 경로가 이미 존재한다 — 상류 머지 또는 이전 적용. 이 패치를 제거하고 stock 경로를 확인하라." >&2
    exit 1
fi

# ── ① 모듈 설치(heredoc — rsync 배달이 *.sh 만 나륯으므로 모듈을 스크립트에 내장) ──
cat > "$MOD" <<'PYEOF'
"""ple_mmap — serve the qwen4_exp PLE n-gram table from NVMe via mmap.

Port of blazux/qwen3.8-Flash-DGX src/vllm_ple_mmap.py to the qwen4_exp tree
(0.29.0rc6=74c96922e), with TP-aware masking added: the reference is TP=1 only.

With VLLM_PLE_MMAP=1 this module patches ``Qwen4ExpNGramEmbedding``:
  * ``__init__`` swaps the PLEVocabParallelEmbedding for a placeholder whose
    ``forward`` gathers rows from ``np.memmap`` views of the checkpoint's
    ngram_embedding.shard_N tensors (zero-copy, page-cache backed);
  * ``load_weights`` drops the shard tensors, keeps the global FP8
    ``weight_scale`` ON THE PLACEHOLDER (this tree's
    ``Qwen4ExpPLELayer._get_embedding_weight_scale`` reads
    ``ngram_embedding.weight_scale``);
  * TP>1: the placeholder computes this rank's vocab range, zeroes out-of-range
    ids (stock VocabParallelEmbedding semantics) and all-reduces, so each rank
    reads only its own rows from disk;
  * the gather is wrapped in custom op ``vllm::qwen4_exp_ple_mmap_lookup`` so
    torch.compile treats it as opaque and it can run outside piecewise CUDA
    graphs (CPU gather + pageable H2D cannot be captured).

Knobs (env): VLLM_PLE_MMAP=1 · VLLM_PLE_MMAP_WORKERS=32 · VLLM_PLE_MMAP_CHUNK=2048
  VLLM_PLE_MMAP_MADVISE=random|normal · VLLM_PLE_MMAP_PREWARM=0|1
  VLLM_PLE_MMAP_DIR=<local NVMe staging dir> · VLLM_PLE_MMAP_FAST_ROWS=512
  VLLM_PLE_MMAP_STATS_SEC=30
"""

from __future__ import annotations

import glob
import json
import logging
import math
import os
import re
import struct
import sys
from concurrent.futures import ThreadPoolExecutor
from typing import Iterable

import numpy as np
import torch
import torch.nn as nn

logger = logging.getLogger("vllm.ple_mmap")

ENV_ENABLE = "VLLM_PLE_MMAP"
_FP8_DTYPES = {
    "F8_E4M3": torch.float8_e4m3fn,
    "F8_E5M2": torch.float8_e5m2,
}
_TABLE_DTYPES = {
    **_FP8_DTYPES,
    "BF16": torch.bfloat16,
    "F16": torch.float16,
}


def enabled() -> bool:
    return os.environ.get(ENV_ENABLE, "0").lower() in ("1", "true", "yes")


def _madvise(mm: np.memmap, kind: str) -> None:
    try:
        import mmap as _mmap

        flag = {"random": _mmap.MADV_RANDOM, "normal": _mmap.MADV_NORMAL}[kind]
        raw = getattr(mm, "_mmap", None)
        if raw is not None:
            raw.madvise(flag)
    except Exception as exc:  # pragma: no cover - platform dependent
        logger.warning("PLE mmap: madvise(%s) failed: %s", kind, exc)


def _env_int(name: str, default: int) -> int:
    try:
        return int(os.environ.get(name, default))
    except ValueError:
        return default


# --------------------------------------------------------------------------- #
# safetensors header parsing (raw offsets — the safetensors API hides them)
# --------------------------------------------------------------------------- #
def parse_safetensors_header(path: str) -> tuple[dict, int]:
    with open(path, "rb") as f:
        (header_len,) = struct.unpack("<Q", f.read(8))
        header = json.loads(f.read(header_len))
    header.pop("__metadata__", None)
    return header, 8 + header_len


def _itemsize(dtype_str: str) -> int:
    return {
        "F8_E4M3": 1,
        "F8_E5M2": 1,
        "U8": 1,
        "I8": 1,
        "BF16": 2,
        "F16": 2,
        "F32": 4,
    }[dtype_str]


class MmapPleTable:
    """Row gather over a table split into ``split_ngram_parts`` shard files."""

    def __init__(
        self,
        shards: dict[int, tuple[str, int, int]],
        shard_size: int,
        row_bytes: int,
        torch_dtype: torch.dtype,
        workers: int = 32,
        chunk: int = 2048,
    ) -> None:
        if not shards:
            raise ValueError("no PLE shards")
        self.shard_size = int(shard_size)
        self.row_bytes = int(row_bytes)
        self.torch_dtype = torch_dtype
        self.chunk = max(1, int(chunk))
        self.paths: list[str | None] = [None] * (max(shards) + 1)
        self.mm: list[np.memmap | None] = [None] * (max(shards) + 1)
        self.rows_total = 0
        advise = os.environ.get("VLLM_PLE_MMAP_MADVISE", "random").strip().lower()
        for idx, (path, offset, rows) in shards.items():
            self.paths[idx] = path
            self.mm[idx] = np.memmap(
                path, dtype=np.uint8, mode="r", offset=offset, shape=(rows, row_bytes)
            )
            if advise in ("random", "1"):
                _madvise(self.mm[idx], "random")
            self.rows_total += rows
        self.pool = ThreadPoolExecutor(max_workers=max(1, int(workers)))
        self.fast_rows = _env_int("VLLM_PLE_MMAP_FAST_ROWS", 512)

    def gather(self, ids: np.ndarray) -> np.ndarray:
        import time as _time

        t0 = _time.perf_counter()
        try:
            return self._gather(ids)
        finally:
            _STATS["gather_ms"] += (_time.perf_counter() - t0) * 1e3
            _STATS["rows"] += int(np.asarray(ids).size)
            _STATS["bytes"] += int(np.asarray(ids).size) * self.row_bytes

    def _gather(self, ids: np.ndarray) -> np.ndarray:
        ids = np.ascontiguousarray(ids, dtype=np.int64).reshape(-1)
        if ids.size == 0:
            return np.empty((0, self.row_bytes), dtype=np.uint8)
        if ids.size <= self.fast_rows:
            if ids.min() < 0 or ids.max() >= self.rows_total:
                raise IndexError(
                    f"PLE row id out of range: [{ids.min()}, {ids.max()}] "
                    f"for {self.rows_total} rows"
                )
            shard = ids // self.shard_size
            local = ids - shard * self.shard_size
            out = np.empty((ids.size, self.row_bytes), dtype=np.uint8)
            for si in np.unique(shard):
                mask = shard == si
                out[mask] = self.mm[si][local[mask]]
            return out
        uniq, inverse = np.unique(ids, return_inverse=True)
        if uniq[0] < 0 or uniq[-1] >= self.rows_total:
            raise IndexError(
                f"PLE row id out of range: [{uniq[0]}, {uniq[-1]}] "
                f"for {self.rows_total} rows"
            )
        shard = uniq // self.shard_size
        local = uniq - shard * self.shard_size
        out = np.empty((uniq.size, self.row_bytes), dtype=np.uint8)

        bounds = np.flatnonzero(np.diff(shard)) + 1
        starts = np.concatenate(([0], bounds))
        ends = np.concatenate((bounds, [uniq.size]))
        tasks: list[tuple[int, int, int]] = []
        for s, e in zip(starts.tolist(), ends.tolist()):
            si = int(shard[s])
            for c in range(s, e, self.chunk):
                tasks.append((si, c, min(c + self.chunk, e)))

        def run(task: tuple[int, int, int]) -> None:
            si, a, b = task
            mm = self.mm[si]
            if mm is None:
                raise IndexError(f"PLE shard {si} missing")
            out[a:b] = mm[local[a:b]]

        if len(tasks) == 1:
            run(tasks[0])
        else:
            for _ in self.pool.map(run, tasks):
                pass
        return out[inverse]

    def prewarm(self) -> None:
        block = 64 << 20
        for path, mm in zip(self.paths, self.mm):
            if path is None or mm is None:
                continue
            start = mm.offset
            end = start + mm.shape[0] * mm.shape[1]
            with open(path, "rb", buffering=0) as f:
                pos = start
                while pos < end:
                    n = f.readinto(bytearray(min(block, end - pos)))  # noqa: F841
                    if not n:
                        break
                    pos += n


# --------------------------------------------------------------------------- #
# Placeholder that stands in for PLEVocabParallelEmbedding
# --------------------------------------------------------------------------- #
class _MmapNgramEmbedding(nn.Module):
    """Duck-types the bits of VocabParallelEmbedding the PLE code reads.

    TP-aware (the reference port is TP=1 only): each rank zeroes ids outside its
    vocab range and all-reduces, so the downstream sees exactly the stock
    VocabParallelEmbedding result while each rank reads only its own rows.

    ``weight_scale`` lives HERE (not on the NGramEmbedding): this tree's
    ``Qwen4ExpPLELayer._get_embedding_weight_scale`` reads
    ``ngram_embedding.weight_scale``.
    """

    def __init__(self, num_embeddings: int, embedding_dim: int,
                 params_dtype: torch.dtype | None = None) -> None:
        super().__init__()
        self.num_embeddings = int(num_embeddings)
        self.org_vocab_size = int(num_embeddings)
        self.embedding_dim = int(embedding_dim)
        self.table: MmapPleTable | None = None
        self.weight_scale: torch.Tensor | None = None
        self._zeros_dtype = params_dtype or torch.bfloat16
        # TP shard range (stock VocabParallelEmbedding semantics)
        from vllm.distributed import (
            get_tensor_model_parallel_rank,
            get_tensor_model_parallel_world_size,
        )

        self.tp_world = int(get_tensor_model_parallel_world_size())
        if self.tp_world > 1:
            from vllm.model_executor.layers.vocab_parallel_embedding import (
                vocab_range_from_global_vocab_size,
            )

            rng = vocab_range_from_global_vocab_size(
                self.num_embeddings,
                get_tensor_model_parallel_rank(),
                self.tp_world,
            )
            self.tp_start, self.tp_end = int(rng[0]), int(rng[1])
        else:
            self.tp_start, self.tp_end = 0, self.num_embeddings

    def _pinned_buf(self, rows: int, row_bytes: int) -> torch.Tensor | None:
        buf = getattr(self, "_pinned", None)
        if buf is None or buf.shape[0] < rows or buf.shape[1] != row_bytes:
            try:
                cap = max(rows + rows // 2, 4096)
                buf = torch.empty((cap, row_bytes), dtype=torch.uint8, pin_memory=True)
            except RuntimeError:
                buf = None
            self._pinned = buf
        return buf

    def forward(self, ids: torch.Tensor) -> torch.Tensor:
        table = self.table
        if table is None:
            # Weights never loaded (e.g. --load-format dummy): keep plumbing alive.
            return torch.zeros(
                (*ids.shape, self.embedding_dim),
                dtype=self._zeros_dtype,
                device=ids.device,
            )
        flat = ids.reshape(-1)
        if self.tp_world > 1:
            in_range = (flat >= self.tp_start) & (flat < self.tp_end)
            work = flat[in_range]
        else:
            in_range = None
            work = flat
        ids_np = work.detach().to("cpu", non_blocking=False).numpy().reshape(-1)
        uniq, inverse = np.unique(ids_np, return_inverse=True)
        rows = table.gather(uniq) if uniq.size else np.empty(
            (0, table.row_bytes), dtype=np.uint8
        )
        if in_range is not None:
            # Expand back to the full token count with zeros out of range —
            # the all-reduce below then sums each rank's own rows only.
            full = np.zeros((flat.shape[0], table.row_bytes), dtype=np.uint8)
            mask_np = in_range.detach().cpu().numpy()
            full[mask_np] = rows[inverse]
            rows, u = full, flat.shape[0]
        else:
            rows = rows[inverse]
            u = rows.shape[0]
        buf = self._pinned_buf(u, table.row_bytes) if ids.device.type == "cuda" else None
        if buf is not None:
            buf[:u].numpy()[:] = rows
            dev = buf[:u].to(ids.device, non_blocking=True)
        else:
            dev = torch.from_numpy(rows).to(ids.device)
        if self.tp_world > 1:
            # all-reduce 는 **uint8 뷰**에서 한다 — vocab 구간이 rank 를 분할하므로
            # 위치마다 정확히 한 rank 만 nonzero 바이트를 기여해 합산이 곧 fp8 바이트다.
            # fp8 dtype 을 직접 all-reduce 하면 NCCL 이 dtype 을 거부할 수 있다.
            from vllm.distributed import tensor_model_parallel_all_reduce

            dev = tensor_model_parallel_all_reduce(dev)
        out = dev.view(table.torch_dtype)
        return out.reshape(*ids.shape, self.embedding_dim)


# --------------------------------------------------------------------------- #
# Patch
# --------------------------------------------------------------------------- #
def _find_shards(
    model_path: str, layer_idx: int
) -> tuple[dict[int, tuple[str, int, int]], str | None, tuple[str, int, int, str] | None, int | None]:
    shard_re = re.compile(
        rf"layers\.{layer_idx}\.ple\.ple_embedding\.ngram_embedding\.shard_(\d+)\.weight$"
    )
    scale_re = re.compile(
        rf"layers\.{layer_idx}\.ple\.ple_embedding\.ngram_embedding\.weight_scale$"
    )
    index_path = os.path.join(model_path, "model.safetensors.index.json")
    if os.path.exists(index_path):
        with open(index_path) as f:
            weight_map = json.load(f)["weight_map"]
        files = sorted(
            {
                os.path.join(model_path, fn)
                for name, fn in weight_map.items()
                if shard_re.search(name) or scale_re.search(name)
            }
        )
    else:
        files = sorted(glob.glob(os.path.join(model_path, "*.safetensors")))

    shards: dict[int, tuple[str, int, int]] = {}
    dtype_str: str | None = None
    scale_entry: tuple[str, int, int, str] | None = None
    cols: int | None = None
    for path in files:
        header, data_start = parse_safetensors_header(path)
        for name, meta in header.items():
            m = shard_re.search(name)
            if m:
                start, end = meta["data_offsets"]
                rows, cols = meta["shape"]
                if dtype_str is None:
                    dtype_str = meta["dtype"]
                elif meta["dtype"] != dtype_str:
                    raise ValueError("PLE shards have mixed dtypes")
                if end - start != rows * cols * _itemsize(dtype_str):
                    raise ValueError(f"PLE shard {name}: size/shape mismatch")
                shards[int(m.group(1))] = (path, data_start + start, rows)
            elif scale_re.search(name):
                start, end = meta["data_offsets"]
                scale_entry = (path, data_start + start, end - start, meta["dtype"])
    return shards, dtype_str, scale_entry, cols


def _read_scale(entry: tuple) -> torch.Tensor:
    path, offset, nbytes, dtype_str = entry
    with open(path, "rb") as f:
        f.seek(offset)
        raw = f.read(nbytes)
    if dtype_str == "F32":
        return torch.tensor(struct.unpack("<f", raw[:4])[0], dtype=torch.float32)
    if dtype_str == "BF16":
        u16 = struct.unpack("<H", raw[:2])[0]
        return torch.tensor(u16 << 16, dtype=torch.int32).view(torch.float32).squeeze()
    if dtype_str == "F16":
        return torch.frombuffer(bytearray(raw[:2]), dtype=torch.float16).clone().squeeze()
    raise ValueError(f"unsupported weight_scale dtype {dtype_str}")


_REGISTRY: dict[str, nn.Module] = {}
_OP_NAME = "qwen4_exp_ple_mmap_lookup"
_STATS = {"calls": 0, "op_ms": 0.0, "gather_ms": 0.0, "rows": 0, "bytes": 0}
_STATS_LAST = [0.0]
_STATS_SEC = _env_int("VLLM_PLE_MMAP_STATS_SEC", 30)


def _stats_log() -> None:
    import time as _time

    now = _time.monotonic()
    if _STATS_SEC <= 0 or now - _STATS_LAST[0] < _STATS_SEC:
        return
    elapsed = now - _STATS_LAST[0] if _STATS_LAST[0] else float(_STATS_SEC)
    _STATS_LAST[0] = now
    s = _STATS
    if not s["calls"]:
        return
    logger.info(
        "PLE mmap stats (last %.0fs): %d ops, op %.0f ms total (%.2f ms/op), "
        "gather %.0f ms total (%.2f ms/op), %d rows, %.1f MiB read",
        elapsed, s["calls"], s["op_ms"], s["op_ms"] / s["calls"],
        s["gather_ms"], s["gather_ms"] / s["calls"],
        s["rows"], s["bytes"] / 2**20,
    )
    s.update(calls=0, op_ms=0.0, gather_ms=0.0, rows=0, bytes=0)


def _lookup_impl(
    ngram_ids: torch.Tensor,
    output: torch.Tensor,
    layer_name: str,
) -> None:
    import time as _time

    t0 = _time.perf_counter()
    layer = _REGISTRY[layer_name]
    result = layer.ngram_embedding(ngram_ids)
    output.copy_(result.to(output.dtype).view(output.shape))
    _STATS["calls"] += 1
    _STATS["op_ms"] += (_time.perf_counter() - t0) * 1e3
    _stats_log()


def _lookup_fake(
    ngram_ids: torch.Tensor,
    output: torch.Tensor,
    layer_name: str,
) -> None:
    return


def _register_op() -> None:
    if hasattr(torch.ops.vllm, _OP_NAME):
        return
    from vllm.utils.torch_utils import direct_register_custom_op

    direct_register_custom_op(
        op_name=_OP_NAME,
        op_func=_lookup_impl,
        mutates_args=["output"],
        fake_impl=_lookup_fake,
    )


def apply(cls: type) -> None:
    """Patch ``Qwen4ExpNGramEmbedding`` (pass the class) when enabled."""
    if not enabled():
        return
    if getattr(cls, "_ple_mmap_patched", False):
        return
    mod = sys.modules[cls.__module__]
    orig_init = cls.__init__
    orig_load_weights = cls.load_weights

    def __init__(self, config, embedding_dim, ple_dense_layer_id, max_total_tokens,
                 max_num_reqs, prefix, layer_name, quant_config=None, params_dtype=None):
        # Swap the embedding class for our placeholder during the stock ctor so
        # nothing large is allocated. quant_config=None keeps the stock code
        # from selecting an FP8 quant method that would create a weight param.
        real_embedding_cls = mod.PLEVocabParallelEmbedding
        mod.PLEVocabParallelEmbedding = lambda n, d, **_kw: _MmapNgramEmbedding(
            n, d, params_dtype=_kw.get("params_dtype")
        )
        try:
            orig_init(self, config, embedding_dim, ple_dense_layer_id,
                      max_total_tokens, max_num_reqs, prefix, layer_name,
                      quant_config=None, params_dtype=params_dtype)
        finally:
            mod.PLEVocabParallelEmbedding = real_embedding_cls
        self._ple_mmap_prefix = prefix
        _REGISTRY[prefix] = self
        self._ple_mmap_model_path = None
        try:
            from vllm.config import get_current_vllm_config
            self._ple_mmap_model_path = get_current_vllm_config().model_config.model
        except Exception as exc:  # pragma: no cover - defensive
            logger.warning("PLE mmap: cannot read model path from vllm config: %s", exc)
        logger.info(
            "PLE mmap: %s -> placeholder embedding (%d rows x %d), table will be mmapped",
            prefix, self.ngram_embedding.org_vocab_size, self.head_dim,
        )

    def load_weights(self, weights: Iterable[tuple[str, torch.Tensor]]) -> set[str]:
        loaded: set[str] = set()
        rest: list[tuple[str, torch.Tensor]] = []
        for name, w in weights:
            if name.startswith("ngram_embedding.shard_") and name.endswith(".weight"):
                loaded.add(name)  # served from disk, never materialised
                continue
            if name == "ngram_embedding.weight_scale":
                self.ngram_embedding.weight_scale = w.detach().to(
                    torch.accelerator.current_accelerator()
                )
                loaded.add(name)
                continue
            rest.append((name, w))
        loaded.update(orig_load_weights(self, rest))
        _setup_table(self)
        return loaded

    def _setup_table(self) -> None:
        if self.ngram_embedding.table is not None:
            return
        model_path = os.environ.get("VLLM_PLE_MMAP_DIR") or self._ple_mmap_model_path
        if not model_path or not os.path.isdir(model_path):
            raise RuntimeError(
                f"PLE mmap: table path {model_path!r} is not a local directory; "
                "point --model at the downloaded snapshot or set VLLM_PLE_MMAP_DIR"
            )
        m = re.search(r"layers\.(\d+)\.", self._ple_mmap_prefix)
        if not m:
            raise RuntimeError(f"PLE mmap: cannot find layer index in {self._ple_mmap_prefix!r}")
        layer_idx = int(m.group(1))
        shards, dtype_str, scale_entry, cols = _find_shards(model_path, layer_idx)
        if not shards:
            raise RuntimeError(f"PLE mmap: no shard tensors for layer {layer_idx} under {model_path}")
        if cols != self.head_dim:
            raise RuntimeError(f"PLE mmap: shard width {cols} != head_dim {self.head_dim}")
        if dtype_str not in _TABLE_DTYPES:
            raise RuntimeError(f"PLE mmap: unsupported shard dtype {dtype_str}")
        if dtype_str in _FP8_DTYPES and self.ngram_embedding.weight_scale is None:
            if scale_entry is None:
                raise RuntimeError("PLE mmap: FP8 shards without ngram_embedding.weight_scale")
            self.ngram_embedding.weight_scale = _read_scale(scale_entry).to(
                torch.accelerator.current_accelerator()
            )
        parts = int(self.split_ngram_parts)
        vocab = int(self.ngram_embedding.org_vocab_size)
        shard_size = math.ceil(vocab / parts)
        for idx, (_p, _o, rows) in shards.items():
            expected = max(0, min(shard_size, vocab - idx * shard_size))
            if rows != expected:
                raise RuntimeError(
                    f"PLE mmap: shard {idx} has {rows} rows, expected {expected}"
                )
        table = MmapPleTable(
            shards, shard_size, cols * _itemsize(dtype_str), _TABLE_DTYPES[dtype_str],
            workers=_env_int("VLLM_PLE_MMAP_WORKERS", 32),
            chunk=_env_int("VLLM_PLE_MMAP_CHUNK", 2048),
        )
        if _env_int("VLLM_PLE_MMAP_PREWARM", 0):
            logger.info("PLE mmap: prewarming page cache (%.1f GiB)...", table.rows_total * table.row_bytes / 2**30)
            table.prewarm()
        self.ngram_embedding.table = table
        logger.info(
            "PLE mmap: layer %d, %d shards, %d rows x %d B (%.1f GiB on disk), dtype %s, %d workers, tp_world %d",
            layer_idx, len(shards), table.rows_total, table.row_bytes,
            table.rows_total * table.row_bytes / 2**30, dtype_str,
            table.pool._max_workers, self.ngram_embedding.tp_world,
        )

    def forward(self, input_ids, query_start_loc, ngram_context):
        ngram_ids = input_ids.new_empty(
            (input_ids.shape[0], self.ngram_heads), dtype=torch.long
        )
        torch.ops.vllm.qwen4_exp_compute_ple_ngram_ids(
            input_ids, query_start_loc, ngram_context, ngram_ids, self.layer_name
        )
        table = self.ngram_embedding.table
        out_dtype = table.torch_dtype if table is not None else self.ngram_embedding._zeros_dtype
        output = torch.empty(
            (ngram_ids.shape[0], self.ngram_heads, self.head_dim),
            dtype=out_dtype,
            device=input_ids.device,
        )
        getattr(torch.ops.vllm, _OP_NAME)(ngram_ids, output, self._ple_mmap_prefix)
        return output.flatten(-2)

    _register_op()
    cls.forward = forward
    cls.__init__ = __init__
    cls.load_weights = load_weights
    cls._setup_table = _setup_table
    cls._ple_mmap_patched = True
    logger.info("PLE mmap patch applied to %s.%s", cls.__module__, cls.__name__)
PYEOF
echo "$TAG wrote $MOD ($(wc -l < "$MOD") lines)"

# ── ② ple_layer.py 끝에 apply 호출 추가 ──
python3 - "$PLE" <<'PY'
import sys
path = sys.argv[1]
MARKER = "[port:62-ple-mmap]"
s = open(path).read()
if MARKER in s:
    print(f"[62-qwen4exp-ple-mmap] FAILED — marker already in {path}", file=sys.stderr)
    raise SystemExit(1)
if "class Qwen4ExpNGramEmbedding" not in s:
    print(f"[62-qwen4exp-ple-mmap] FAILED — Qwen4ExpNGramEmbedding 부재(베이스 아님)", file=sys.stderr)
    raise SystemExit(1)
s += f'''
# {MARKER} VLLM_PLE_MMAP=1 일 때 PLE n-gram 테이블을 NVMe mmap 으로 서빙한다
# (blazux 이식 + TP=2 rank-aware masking 자체 추가 — 자세한 것은 ple_mmap.py docstring).
from vllm.models.qwen4_exp.nvidia import ple_mmap as _ple_mmap

_ple_mmap.apply(Qwen4ExpNGramEmbedding)
'''
open(path, "w").write(s)
print(f"[62-qwen4exp-ple-mmap] apply-call appended to {path}")
PY

# ── 빌드타임 검증(fail-loud) ──
python3 -m compileall -q "$MOD" "$PLE" || { echo "$TAG FAIL: compileall" >&2; exit 1; }
grep -q 'port:62-ple-mmap' "$PLE" || { echo "$TAG FAIL: apply 마커 부재" >&2; exit 1; }
echo "$TAG OK — ple_mmap 모듈 설치·apply 배선(최종 중재 = TP=2 serve 스모크)"
