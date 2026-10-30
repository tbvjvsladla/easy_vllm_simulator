#!/bin/bash
# Official vLLM v0.32.0 SM12x port candidate.
# Source pins: sibling build_patches_src/PROVENANCE.json; no previous GPU validation inherited.
# CPU source checks only so far; native ABI/import/GPU health/inference remain required.
# Historical development explanation is retained in pre-v032-buildkit, not current success evidence.
set -euo pipefail

TAG="[88-engram-prestage]"

: "${SM12X_PORT:=0}"
if [ "${SM12X_PORT}" != "1" ]; then
    echo "$TAG skip - SM12X_PORT=${SM12X_PORT} (stock 빌드 경로 불변)"
    if [ -n "${EASY_VLLM_PATCH_STATUS:-}" ]; then printf 'skipped\tSM12X_PORT!=1\n' > "$EASY_VLLM_PATCH_STATUS"; fi
    exit 0
fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PYTHONPATH="${EXL3_PORT_LIBDIR:-$HERE}:$HERE${PYTHONPATH:+:$PYTHONPATH}"

python3 - "$TAG" <<'PY'
import sys
import exl3_port_lib as L
from exl3_port_lib import Edit

TAG = sys.argv[1]
M = "# [exl3-port:engram-prestage]"

# ---- 0) prerequisites (83 must have installed the backend with its instance flags)
BACKEND_REL = "vllm/models/deepseek_v41/common/engram_file_backend.py"
MS_REL = "vllm/models/deepseek_v41/nvidia/model_state.py"
backend = L.VLLM_SRC / BACKEND_REL
if not backend.is_file() or "_exl3port_file_backed" not in backend.read_text():
    L.die(TAG, "83-engram-file-backend must run first (engram_file_backend.py with _exl3port_file_backed missing)")
ms = L.VLLM_SRC / MS_REL
if not ms.is_file():
    L.die(TAG, f"{MS_REL} missing - prestage needs the V2 model-state hook (0.32.0 tree listing had it)")
ms_text = ms.read_text()
for needle in ("def prepare_inputs(", "lookback_token_ids", "def _gather_lookback_kernel("):
    if needle not in ms_text:
        L.die(TAG, f"{MS_REL}: needle {needle!r} absent - 0.32.0 model_state drifted from the audited shape")
MR_REL = "vllm/v1/worker/gpu/model_runner.py"
mr_text = (L.VLLM_SRC / MR_REL).read_text() if (L.VLLM_SRC / MR_REL).is_file() else ""
# early gather order contract: runner.prepare_attn runs BEFORE model_state.prepare_inputs in execute_model (audited 0.32.0 order)
_a, _b = mr_text.find("block_tables, slot_mappings = self.prepare_attn(input_batch)"), mr_text.find("**self.model_state.prepare_inputs(input_batch, self.req_states)")
if "def prepare_attn(" not in mr_text or _a < 0 or _b < 0 or not _a < _b:
    L.die(TAG, f"{MR_REL}: prepare_attn -> model_state.prepare_inputs order not found (early gather hook would be wrong)")

# ---- 1) backend: cpu_offload toggle (anchor = 83's E_FLAG output line, exact) — unchanged from v1
E_OFFLOAD = Edit(
    "cpu-offload-toggle",
    "        self.cpu_offload = True  # [exl3-port:engram] makes 0.32.0 Engram create the prefetch stream (lookup runs on it)\n",
    f'''        {M} prestage ON -> no 0.32.0 prefetch side stream (rows are staged before the graph, on the main stream).
        # prestage OFF -> EXL3PORT_ENGRAM_SIDE_STREAM=1 (default, = pre-88 image) or 0 (main stream, Mia's 0909 condition).
        self.cpu_offload = (
            os.environ.get("EXL3PORT_ENGRAM_SIDE_STREAM", "1") == "1"
            if os.environ.get("EXL3PORT_ENGRAM_PRESTAGE", "1") != "1"
            else False
        )
''',
)

# ---- 2) model_state footer v2 (runtime wrapper; plain string, no f-string escaping)
V1_MARK = f"{M} footer"                       # v1 marker line (must NOT be a substring of the v2 marker)
FOOT_MARK = "# [exl3-port:engram-prestage-v2] footer"
assert V1_MARK not in FOOT_MARK
FOOTER = '''
@@FOOT_MARK@@
def _exl3port_install_engram_prestage() -> None:
    import ctypes
    import inspect
    import os
    import sys
    import types
    from time import perf_counter as _now

    import torch

    from vllm.logger import init_logger

    log = init_logger(__name__)
    if os.environ.get("EXL3PORT_ENGRAM_PRESTAGE", "1") != "1":
        log.info("exl3-port: engram prestage OFF (EXL3PORT_ENGRAM_PRESTAGE!=1) - Engram lookups stay inside the forward")
        return
    mod = sys.modules[__name__]
    owners = [
        c for c in vars(mod).values()
        if isinstance(c, type) and c.__module__ == mod.__name__ and "prepare_inputs" in vars(c)
    ]
    if len(owners) != 1:
        raise RuntimeError(
            f"exl3-port prestage: expected exactly one class defining prepare_inputs in {mod.__name__}, found "
            f"{[c.__name__ for c in owners]}"
        )
    cls = owners[0]
    if getattr(cls, "_exl3port_prestage_installed", False):
        return
    orig_init, orig_prepare = cls.__init__, cls.prepare_inputs
    init_sig = inspect.signature(orig_init)
    prep_params = list(inspect.signature(orig_prepare).parameters)
    if "input_batch" not in prep_params:
        raise RuntimeError(f"exl3-port prestage: {cls.__name__}.prepare_inputs has no 'input_batch' parameter: {prep_params}")
    batch_pos = prep_params.index("input_batch")  # includes 'self' at 0

    def _env_int(name, default, lo=0):
        raw = os.environ.get(name, "")
        try:
            val = int(raw) if raw.strip() else default
        except ValueError:
            raise RuntimeError(f"exl3-port prestage: {name}={raw!r} is not an integer") from None
        return max(lo, val)

    CENSUS_EVERY = _env_int("EXL3PORT_ENGRAM_PRESTAGE_CENSUS", 0)
    EARLY = os.environ.get("EXL3PORT_ENGRAM_EARLY", "0") == "1"
    EARLY_VERIFY = _env_int("EXL3PORT_ENGRAM_EARLY_VERIFY", 4, lo=1)
    EARLY_MAX_TOKENS = _env_int("EXL3PORT_ENGRAM_EARLY_MAX_TOKENS", 64, lo=1)

    def _image_sentinel_mask():
        import importlib

        for name in (
            "vllm.models.deepseek_v41.common.mm_preprocess",
            "vllm.models.deepseek_v41.mm_preprocess",
            "vllm.models.deepseek_v41.nvidia.mm_preprocess",
        ):
            try:
                return getattr(importlib.import_module(name), "image_sentinel_mask")
            except (ImportError, AttributeError):
                continue
        raise RuntimeError("exl3-port prestage: image_sentinel_mask not found in deepseek_v41 (0.32.0 drift)")

    class _Census:
        """Host perf_counter + deferred CUDA-event timings. Never synchronizes; events still in flight at report time
        are carried into the next window."""

        def __init__(self, every, n_tables, fb, stores):
            self.every = every
            self.fb, self.stores = fb, stores
            self.keys = (["step", "prep", "hash", "wait"] + [f"lookup{i}" for i in range(n_tables)]
                         + ["upload", "exposed", "total", "w_sync"])
            self.calls, self.rows, self.early_hits = 0, 0, 0
            self.last_entry = None
            self.gpu = []
            self.prev_stats = [self._hm(s) for s in stores]
            self._reset()

        def _reset(self):
            self.sum = dict.fromkeys(self.keys, 0.0)
            self.max = dict.fromkeys(self.keys, 0.0)
            self.cnt = dict.fromkeys(self.keys, 0)
            self.gsum, self.gmax, self.gcnt = {}, {}, {}

        def _hm(self, store):
            st = self.fb._stats(store)
            return int(st["hits"]), int(st["misses"])

        def add(self, key, sec):
            self.sum[key] += sec
            self.cnt[key] += 1
            if sec > self.max[key]:
                self.max[key] = sec

        def mark_step(self, t):
            if self.last_entry is not None:
                self.add("step", t - self.last_entry)
            self.last_entry = t

        def ev(self):
            e = torch.cuda.Event(enable_timing=True)
            e.record()
            return e

        def gpu_pair(self, name, a, b):
            self.gpu.append((name, a, b))

        def step_done(self, rows, early):
            self.calls += 1
            self.rows += rows
            self.early_hits += int(bool(early))
            if self.calls % self.every == 0:
                self.report()

        def report(self):
            pending = []
            for name, a, b in self.gpu:
                if not b.query():
                    pending.append((name, a, b))
                    continue
                ms_ = a.elapsed_time(b)
                self.gsum[name] = self.gsum.get(name, 0.0) + ms_
                self.gcnt[name] = self.gcnt.get(name, 0) + 1
                self.gmax[name] = max(self.gmax.get(name, 0.0), ms_)
            self.gpu = pending
            parts = []
            for k in self.keys:
                if self.cnt[k]:
                    parts.append("%s=%.3f/%.3f" % (k, 1e3 * self.sum[k] / self.cnt[k], 1e3 * self.max[k]))
            for k in sorted(self.gsum):
                parts.append("gpu_%s=%.3f/%.3f" % (k, self.gsum[k] / self.gcnt[k], self.gmax[k]))
            for i, s in enumerate(self.stores):
                h, m = self._hm(s)
                ph, pm = self.prev_stats[i]
                self.prev_stats[i] = (h, m)
                dh, dm = h - ph, m - pm
                parts.append("hit%d=%.1f%%(miss=%d)" % (i, 100.0 * dh / max(1, dh + dm), dm))
            log.info(
                "exl3-port engram census: steps=%d rows/layer=%.1f early=%d %s (mean/max ms)",
                self.every, self.rows / max(1, self.every), self.early_hits, " ".join(parts),
            )
            self.rows = self.early_hits = 0
            self._reset()

    class _Stager:
        """Stage file-backed Engram rows in prepare_inputs, outside any captured forward (sfx prestage design,
        gather = Mia row_store called synchronously on the host; optional early off-thread gather)."""

        def __init__(self, hash_state, engrams):
            from vllm.models.deepseek_v41.common import engram_file_backend as fb

            self.fb = fb
            self.lib, _ = fb._ensure_native()
            self.hash_state = hash_state
            if getattr(hash_state, "use_slot_cache", True):
                raise RuntimeError("exl3-port prestage requires the V2 stateless lookback hash path")
            self.mask = _image_sentinel_mask()
            fwd = list(inspect.signature(hash_state.forward).parameters)
            if len(fwd) != 8:
                raise RuntimeError(f"exl3-port prestage: NgramHashState.forward has {len(fwd)} params {fwd} (0909 shape = 8)")
            self.engrams = sorted(engrams, key=lambda e: int(e.layer_hash_index))
            emb0 = self.engrams[0].embed_tokens
            self.local_heads = int(emb0.part_n_hash_cols)
            self.head_start = int(emb0.head_start)
            self.head_end = min(self.head_start + self.local_heads, int(emb0.n_hash_cols))
            self.dim = int(emb0.dim)
            for e in self.engrams:
                emb = e.embed_tokens
                if (int(emb.part_n_hash_cols), int(emb.head_start), int(emb.dim)) != (self.local_heads, self.head_start, self.dim):
                    raise RuntimeError("exl3-port prestage: Engram tables disagree on head sharding/dim")
            self.max_tokens = int(self.engrams[0].staged_rows.shape[0])
            num_layers = int(hash_state.multipliers.shape[0])
            self.hash_host = torch.empty(
                (self.max_tokens, num_layers, self.head_end - self.head_start),
                dtype=torch.int32, device="cpu", pin_memory=True,
            )
            self.hashes_ready = torch.cuda.Event()
            self.verify_left = max(0, int(os.environ.get("EXL3PORT_ENGRAM_PRESTAGE_VERIFY", "4")))
            self.census = (
                _Census(CENSUS_EVERY, len(self.engrams), fb, [e.embed_tokens._store for e in self.engrams])
                if CENSUS_EVERY > 0 else None
            )
            # early gather state (EXL3PORT_ENGRAM_EARLY=1)
            self.early_on = False
            self.early_verify_left = EARLY_VERIFY
            self._early_rec = None   # (batch, n, gpu hashes) for the batch prepare_attn launched
            self._early_fut = None   # the single in-flight worker job; pinned buffers are reused only after it is done
            self._pool = None
            for e in self.engrams:
                e.staged_rows.zero_()  # padded graph rows (> n) must never read uninitialised bf16
                for extra in getattr(e, "_extra_staged_rows", ()) or ():
                    extra.zero_()
                e.prepare_embeddings = types.MethodType(lambda self, *a, **k: None, e)  # rows are already staged
                emb = e.embed_tokens
                emb._exl3port_unguarded_lookup = emb.lookup

                def _guard(indices, out, background=False, _orig=emb.lookup):
                    if torch.cuda.is_current_stream_capturing():
                        raise RuntimeError(
                            "exl3-port: file-backed Engram lookup reached a CUDA-graph capture while prestage is armed "
                            "(a caller other than prepare_embeddings - e.g. a drafter Engram - is still looking up in-forward)"
                        )
                    return _orig(indices, out, background)

                emb.lookup = _guard
                e._exl3port_prestage = True

        # ---------------------------------------------------------------- shared pieces (v1 semantics, split for reuse)
        def _dequant(self, emb, rows, out2d):
            fb = self.fb
            if fb._DEQUANT is None:
                fb._DEQUANT = fb._dequant_kernel()
            from vllm.triton_utils import triton

            grid = min(triton.cdiv(rows, 16), max(1, int(emb._num_sms)))
            fb._DEQUANT[(grid,)](
                emb._dev_w.view(torch.float8_e4m3fn), emb._dev_s, out2d, rows,
                DIM=emb.dim, QUANT_BLOCK=emb.block_size, BLOCK_R=16, GRID=grid,
            )

        def _launch_hash(self, ids, positions, query_start_loc, lookback, n):
            hashes = self.hash_state(
                ids, positions, query_start_loc, self.mask(ids),
                lookback, self.mask(lookback), None, None,
            )
            self.hash_host[:n].copy_(hashes[:, :, self.head_start:self.head_end], non_blocking=True)
            self.hashes_ready.record()
            return hashes

        def _lookup(self, e, n):
            """Host only (no CUDA calls): ids from hash_host -> pinned _ids, then the synchronous row_store C call
            (ctypes CDLL releases the GIL). Safe on a worker thread."""
            emb = e.embed_tokens
            rows = n * self.local_heads
            if rows > int(emb._ids.numel()):
                raise RuntimeError(f"exl3-port prestage: rows={rows} exceed Engram staging {int(emb._ids.numel())}")
            width = self.head_end - self.head_start
            view = emb._ids[:rows].view(n, self.local_heads)
            view[:, :width].copy_(self.hash_host[:n, int(e.layer_hash_index), :])
            if width < self.local_heads:
                view[:, width:].fill_(-1)
            work = emb._works.get(rows)
            if work is None:
                work = self.fb.Work(emb._store, emb._ids.data_ptr(), emb._host_w.data_ptr(), emb._host_s.data_ptr(), rows)
                emb._works[rows] = work
            self.lib.row_store_lookup(ctypes.addressof(work))  # same C entry the host node ran, now synchronous

        def _upload(self, e, n):
            emb = e.embed_tokens
            rows = n * self.local_heads
            emb._dev_w[:rows].copy_(emb._host_w[:rows], non_blocking=True)
            emb._dev_s[:rows].copy_(emb._host_s[:rows], non_blocking=True)
            dest = (e._staged_rows_for_ubatch() if hasattr(e, "_staged_rows_for_ubatch") else e.staged_rows)[:n]  # 0.32.0: ubatch 메서드 없음(dp_size=1 · _ready_rows=staged_rows[:n])
            self._dequant(emb, rows, dest.view(rows, self.dim))
            return dest

        def _verify_rows(self, e, dest, hashes, n):
            emb = e.embed_tokens
            ref = torch.empty_like(dest)
            emb._exl3port_unguarded_lookup(hashes[:, int(e.layer_hash_index), :].contiguous(), ref, False)
            if torch.isnan(dest).any().item() or not torch.equal(dest.view(torch.int16), ref.view(torch.int16)):
                raise RuntimeError(
                    f"exl3-port prestage: staged rows != in-forward lookup (layer_hash_index={e.layer_hash_index}, n={n})"
                )

        # ---------------------------------------------------------------- early gather (EXL3PORT_ENGRAM_EARLY=1)
        def arm_early(self, device):
            from concurrent.futures import ThreadPoolExecutor

            def _init_worker(dev=device):
                torch.cuda.set_device(dev)

            self._pool = ThreadPoolExecutor(max_workers=1, thread_name_prefix="exl3-engram-early", initializer=_init_worker)
            self.early_on = True

        def disarm_early(self, exc):
            if self.early_on:
                self.early_on = False
                log.warning("exl3-port: engram early gather DISABLED -> stock stage order: %r", exc)

        def drain_early(self):
            """Wait for the in-flight worker job (if any) so hash_host/_ids/_host_w can be rewritten. Its exception, if any,
            is logged and disarms the early path; the caller then runs the stock stage, which recomputes everything."""
            fut, self._early_fut = self._early_fut, None
            if fut is None:
                return None
            try:
                return fut.result()
            except Exception as exc:  # noqa: BLE001 - fail-loud fallback: logged, disarmed, stock path refills the step
                self.disarm_early(exc)
                return None

        def _worker(self, n):
            with torch.inference_mode():
                t0 = _now()
                self.hashes_ready.synchronize()  # releases the GIL; main keeps building attention metadata
                t1 = _now()
                lk = []
                for e in self.engrams:
                    ta = _now()
                    self._lookup(e, n)
                    lk.append(_now() - ta)
                return t1 - t0, lk

        def early_launch(self, model_state, batch, req_states):
            """GPUModelRunner.prepare_attn entry: launch lookback + hash + D2H now and hand the gather to the worker."""
            if not self.early_on or batch.input_ids is None:
                return
            if getattr(model_state, "rope_state", None) is not None:
                return  # positions are rewritten by model_state.prepare_inputs (M-RoPE) - early inputs would not be final
            n = min(int(batch.num_tokens), self.max_tokens)
            if n <= 0 or n > EARLY_MAX_TOKENS or not self.hash_state.ensure_cache():
                return
            window = getattr(model_state, "lookback_token_ids", None)
            if window is None:
                return
            self.drain_early()
            if not self.early_on:
                return
            c = self.census
            th = _now() if c is not None else 0.0
            ga = c.ev() if c is not None else None
            # DeepseekV41ModelState.prepare_inputs' lookback gather, verbatim (it runs again there; same inputs -> same window).
            all_token_ids = req_states.all_token_ids.gpu
            depth = window.shape[1]
            _gather_lookback_kernel[(window.shape[0],)](
                window, batch.idx_mapping, req_states.num_computed_tokens.gpu, all_token_ids,
                all_token_ids.stride(0), batch.idx_mapping.shape[0],
                DEPTH=depth, BLOCK_DEPTH=triton.next_power_of_2(depth),
            )
            hashes = self._launch_hash(
                batch.input_ids[:n], batch.positions[:n], batch.query_start_loc[: batch.num_reqs + 1], window, n,
            )
            if c is not None:
                c.gpu_pair("hash", ga, c.ev())
                c.add("hash", _now() - th)
            self._early_fut = self._pool.submit(self._worker, n)
            self._early_rec = (batch, n, hashes)  # strong ref for one step only; cleared in prepare_inputs' finally

        # ---------------------------------------------------------------- stage (called from prepare_inputs)
        @torch.inference_mode()
        def stage(self, input_ids, positions, query_start_loc, lookback, num_tokens, batch=None):
            c = self.census
            t0 = _now() if c is not None else 0.0
            n = min(int(num_tokens), self.max_tokens)
            if n <= 0 or not self.hash_state.ensure_cache():
                return 0
            rec, self._early_rec = self._early_rec, None
            early = (
                self.early_on and rec is not None and batch is not None and rec[0] is batch
                and rec[1] == n and input_ids is batch.input_ids and self._early_fut is not None
            )
            hashes = None
            if early:
                hashes = rec[2]
                tw = _now() if c is not None else 0.0
                got = self.drain_early()
                if got is None:  # worker failed (logged + disarmed): stock path below refills this step
                    early = False
                else:
                    t_sync = _now()
                    if c is not None:
                        c.add("wait", t_sync - tw)
                        c.add("w_sync", got[0])
                        for i, s in enumerate(got[1]):
                            c.add(f"lookup{i}", s)
                    if self.early_verify_left:
                        early_host = self.hash_host[:n].clone()
                        ids = input_ids[:n]
                        hashes = self._launch_hash(ids, positions[:n], query_start_loc, lookback, n)
                        self.hashes_ready.synchronize()
                        if not torch.equal(early_host, self.hash_host[:n]):
                            self.disarm_early(RuntimeError(f"early hash != stock hash (n={n})"))
                            for e in self.engrams:  # hash_host now holds the stock hashes: gather them again
                                self._lookup(e, n)
                        else:
                            self.early_verify_left -= 1
                            if not self.early_verify_left:
                                log.info("exl3-port: engram early gather matches the stock hash (verified steps done)")
                    for e in self.engrams:
                        ta = _now() if c is not None else 0.0
                        ga = c.ev() if c is not None else None
                        dest = self._upload(e, n)
                        if c is not None:
                            c.gpu_pair("upload", ga, c.ev())
                            c.add("upload", _now() - ta)
                        if self.verify_left:
                            self._verify_rows(e, dest, hashes, n)
            if not early:
                self.drain_early()  # never rewrite pinned buffers under a running worker
                ids = input_ids[:n]
                th = _now() if c is not None else 0.0
                ga = c.ev() if c is not None else None
                hashes = self._launch_hash(ids, positions[:n], query_start_loc, lookback, n)
                if c is not None:
                    c.gpu_pair("hash", ga, c.ev())
                    t1 = _now()
                    c.add("hash", t1 - th)
                self.hashes_ready.synchronize()  # legal: eager, before the graph replay (the sfx design pays this sync)
                if c is not None:
                    t_sync = _now()
                    c.add("wait", t_sync - t1)
                for i, e in enumerate(self.engrams):
                    if c is not None:
                        ta = _now()
                    self._lookup(e, n)
                    if c is not None:
                        tb = _now()
                        c.add(f"lookup{i}", tb - ta)
                        ga = c.ev()
                    dest = self._upload(e, n)
                    if c is not None:
                        c.gpu_pair("upload", ga, c.ev())
                        c.add("upload", _now() - tb)
                    if self.verify_left:
                        self._verify_rows(e, dest, hashes, n)
            if self.verify_left:
                self.verify_left -= 1
                if not self.verify_left:
                    log.info("exl3-port: engram prestage self-check bit-exact (%d tables)", len(self.engrams))
            if c is not None:
                t_end = _now()
                c.add("exposed", t_end - t_sync)
                c.add("total", t_end - t0)
                c.step_done(n * self.local_heads, early)
            return n

    def _build(model):
        engrams = [
            m for m in model.modules()
            if callable(getattr(m, "prepare_embeddings", None))
            and getattr(getattr(m, "embed_tokens", None), "_exl3port_file_backed", False)
        ]
        if not engrams:
            return None
        # v0.32.0 introduces EngramBatch, excluded by official TP2/DP1 create().
        # Reject unsupported batching instead of staging into its replaced/empty buffers.
        if any(type(m).__name__ == "EngramBatch" for m in model.modules()):
            raise RuntimeError("exl3-port prestage: EngramBatch is unsupported; use TP2/DP1")
        if any(e.embed_tokens.dp_size != 1 or e.embed_tokens.tp_size != 2 for e in engrams):
            raise RuntimeError("exl3-port prestage candidate requires TP2/DP1")
        hash_states = [m for m in model.modules() if type(m).__name__ == "NgramHashState"]
        if len(hash_states) != 1:
            raise RuntimeError(f"exl3-port prestage: expected one NgramHashState, found {len(hash_states)}")
        stager = _Stager(hash_states[0], engrams)
        if EARLY:
            _install_early_hook()
            stager.arm_early(engrams[0].staged_rows.device)
        log.info(
            "exl3-port: engram prestage armed (tables=%d, max_tokens=%d, heads=[%d,%d), verify=%d, census=%d, early=%s)",
            len(stager.engrams), stager.max_tokens, stager.head_start, stager.head_end, stager.verify_left,
            CENSUS_EVERY, "on(verify=%d,max_tokens=%d)" % (EARLY_VERIFY, EARLY_MAX_TOKENS) if EARLY else "off",
        )
        return stager

    def _install_early_hook():
        if globals().get("_gather_lookback_kernel") is None or globals().get("triton") is None:
            raise RuntimeError("exl3-port early gather: _gather_lookback_kernel missing from model_state (0.32.0 drift)")
        mr = sys.modules.get("vllm.v1.worker.gpu.model_runner")
        runner_cls = getattr(mr, "GPUModelRunner", None) if mr is not None else None
        if runner_cls is None or "prepare_attn" not in vars(runner_cls):
            raise RuntimeError("exl3-port early gather: GPUModelRunner.prepare_attn not found (V2 runner not loaded / drifted)")
        if getattr(runner_cls, "_exl3port_early_hooked", False):
            return
        orig_attn = runner_cls.prepare_attn

        def prepare_attn(self_r, input_batch, *a, **k):
            ms_ = getattr(self_r, "model_state", None)
            st = getattr(ms_, "_exl3port_engram_stager", None)
            if st is not None and st.early_on:
                try:
                    st.early_launch(ms_, input_batch, self_r.req_states)
                except Exception as exc:  # noqa: BLE001 - logged + disarmed; stage() then runs the stock order
                    st._early_rec = None
                    st.disarm_early(exc)
            return orig_attn(self_r, input_batch, *a, **k)

        runner_cls.prepare_attn = prepare_attn
        runner_cls._exl3port_early_hooked = True

    def __init__(self, *args, **kwargs):
        orig_init(self, *args, **kwargs)
        model = getattr(self, "model", None)
        if model is None:
            model = init_sig.bind(self, *args, **kwargs).arguments.get("model")
        if model is None:
            raise RuntimeError(f"exl3-port prestage: cannot find the model on {cls.__name__} (no self.model / 'model' arg)")
        self._exl3port_engram_stager = _build(model)

    def prepare_inputs(self, *args, **kwargs):
        stager = getattr(self, "_exl3port_engram_stager", None)
        if stager is None:
            return orig_prepare(self, *args, **kwargs)
        c = stager.census
        if c is not None:
            tp = _now()
            c.mark_step(tp)
        try:
            model_inputs = orig_prepare(self, *args, **kwargs)
            if c is not None:
                c.add("prep", _now() - tp)
            batch = kwargs["input_batch"] if "input_batch" in kwargs else args[batch_pos - 1]
            if batch.input_ids is not None:
                positions = model_inputs.get("positions") if isinstance(model_inputs, dict) else None
                stager.stage(
                    batch.input_ids,
                    positions if positions is not None else batch.positions,
                    batch.query_start_loc[: batch.num_reqs + 1],
                    model_inputs["lookback_token_ids"],
                    batch.num_tokens,
                    batch=batch,
                )
        finally:
            stager._early_rec = None  # an early launch is consumed by this step or never
        return model_inputs

    cls.__init__ = __init__
    cls.prepare_inputs = prepare_inputs
    cls._exl3port_prestage_installed = True


_exl3port_install_engram_prestage()
'''.replace("@@FOOT_MARK@@", FOOT_MARK)
FOOT_TAIL = "\n_exl3port_install_engram_prestage()\n"


def footer_v2(text: str) -> tuple[str, str]:
    """Footer state machine: v2 present -> already; v1 at the tail -> replace (in-place upgrade); none -> append."""
    n1, n2 = text.count(V1_MARK), text.count(FOOT_MARK)
    if n2 == 1 and n1 == 0:
        if not text.endswith(FOOT_TAIL):
            L.die(TAG, f"{MS_REL}: v2 footer present but not at the file tail (foreign edit after it)")
        return text, "footer-v2:already"
    if n2:
        L.die(TAG, f"{MS_REL}: footer markers v1 x{n1} v2 x{n2} - partial/foreign edit, rebuild from the base image")
    if n1 == 1:
        i = text.index("\n" + V1_MARK + "\n")
        tail = text[i:]
        if (not tail.endswith(FOOT_TAIL) or tail.count("def _exl3port_install_engram_prestage(") != 1
                or tail.count(FOOT_TAIL) != 1):
            L.die(TAG, f"{MS_REL}: v1 footer is not the exact file tail - refusing in-place upgrade")
        return text[:i] + FOOTER, "footer-v1->v2:applied"
    if n1 == 0:
        if "_exl3port_install_engram_prestage" in text:
            L.die(TAG, f"{MS_REL}: prestage symbol present without a footer marker - foreign edit")
        return (text if text.endswith("\n") else text + "\n") + FOOTER, "footer-v2:applied"
    L.die(TAG, f"{MS_REL}: v1 footer marker x{n1} (expected 0 or 1)")


# two-phase: compute everything in memory first (any die -> nothing written)
ms_new, ms_out = footer_v2(ms_text)
compile(ms_new, str(ms), "exec")  # syntax gate before any write
report = L.apply_edit_jobs(TAG, [(BACKEND_REL, [E_OFFLOAD], None, None)])
if ms_new != ms_text:
    ms.write_text(ms_new)
report.append((MS_REL, [ms_out]))
for rel, out in report:
    print(f"{TAG} {rel}: {', '.join(out)}")

# ---- post-assert (static; runtime arming is proven only by the boot log line 'engram prestage armed')
L.py_compile(TAG, [backend, ms])
bt, mt = backend.read_text(), ms.read_text()
if "EXL3PORT_ENGRAM_PRESTAGE" not in bt or "self.cpu_offload = True  # [exl3-port:engram]" in bt:
    L.die(TAG, "post-assert: backend cpu_offload toggle missing or stock-83 line still present")
if mt.count(FOOT_MARK) != 1 or mt.count(V1_MARK) != 0 or not mt.endswith(FOOT_TAIL):
    L.die(TAG, "post-assert: model_state footer v2 count != 1, v1 residue, or footer not at tail")
for needle in ("EXL3PORT_ENGRAM_PRESTAGE_CENSUS", "EXL3PORT_ENGRAM_EARLY", "def early_launch(", "def _lookup(", "hashes_ready.synchronize()"):
    if needle not in mt:
        L.die(TAG, f"post-assert: model_state footer v2 lacks {needle!r}")
n_new = sum(1 for _, out in report for o in out if o.endswith(":applied"))
L.status("applied", f"{n_new} new edits (prestage v2: census+early)" if n_new else "already present (no-op rerun)")
print(f"{TAG} OK ({n_new} new)")
PY
