#!/bin/bash
# Official vLLM v0.32.0 SM12x port candidate.
# Source pins: sibling build_patches_src/PROVENANCE.json; no previous GPU validation inherited.
# CPU source checks only so far; native ABI/import/GPU health/inference remain required.
# Historical development explanation is retained in pre-v032-buildkit, not current success evidence.
set -euo pipefail

TAG="[79-exl3-port-selfcheck]"

: "${SM12X_PORT:=0}"
if [ "${SM12X_PORT}" != "1" ]; then
    echo "$TAG skip - SM12X_PORT=${SM12X_PORT} (stock 빌드 경로 불변)"
    if [ -n "${EASY_VLLM_PATCH_STATUS:-}" ]; then printf 'skipped\tSM12X_PORT!=1\n' > "$EASY_VLLM_PATCH_STATUS"; fi
    exit 0
fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIBDIR=""
for d in "${EXL3_PORT_LIBDIR:-}" "$HERE/../build_patches_src" /tmp/build_patches_src; do
    if [ -n "$d" ] && [ -f "$d/exl3_port_lib.py" ] && [ -f "$d/PROVENANCE.json" ]; then LIBDIR="$(cd "$d" && pwd)"; break; fi
done
[ -n "$LIBDIR" ] || { echo "$TAG FAIL: exl3_port_lib.py / PROVENANCE.json 를 찾지 못했다" >&2; exit 1; }

( cd / && EXL3_PORT_LIBDIR="$LIBDIR" PYTHONPATH="$LIBDIR${PYTHONPATH:+:$PYTHONPATH}" python3 - "$TAG" <<'PY'
import ctypes, json, os, pathlib, sys
import exl3_port_lib as L

TAG = sys.argv[1]
P = L.prov()
fails = []
def check(name, fn):
    try:
        msg = fn()
        print(f"{TAG} PASS {name}" + (f": {msg}" if msg else ""))
    except Exception as exc:  # noqa: BLE001
        print(f"{TAG} FAIL {name}: {type(exc).__name__}: {exc}")
        fails.append(name)

def registry():
    from vllm.model_executor.layers.quantization import QUANTIZATION_METHODS, get_quantization_config
    from vllm.model_executor.layers.quantization.base_config import QuantizationConfig
    assert "exl3" in QUANTIZATION_METHODS, "exl3 not in QUANTIZATION_METHODS"
    cls = get_quantization_config("exl3")
    assert cls.__name__ == "Exl3Config" and cls.__module__ == "vllm_exl3.exl3", (cls, cls.__module__)
    assert issubclass(cls, QuantizationConfig), "Exl3Config is a no-vLLM stub (plugin import degraded)"
    return str(cls)

def abstracts():
    from vllm_exl3 import exl3 as m
    bad = {}
    for n in dir(m):
        c = getattr(m, n)
        if isinstance(c, type) and n.startswith("Exl3") and getattr(c, "__abstractmethods__", None):
            bad[n] = sorted(c.__abstractmethods__)
    assert not bad, f"abstract methods unimplemented: {bad}"
    return "Exl3* classes concrete"

def plugin_edits():
    import importlib.metadata as md
    from vllm_exl3 import exl3 as m
    d = md.distribution("vllm-exl3")
    assert d.version == P["sources"]["vllm_exl3"]["version"], d.version
    assert not [e for e in md.entry_points(group="vllm.general_plugins") if e.name == "vllm_exl3"], "entry point installed"
    cfg = m.Exl3Config.from_config({"quant_method": "exl3", "bits": 2, "codebook": "mcg", "mtp_experts": "source", "mtp_experts_start_layer": 40,
                                    "non_routed_quantization": {"quant_method": "deepseek_v4_fp8", "fmt": "e4m3", "activation_scheme": "dynamic",
                                                                "scale_fmt": "ue8m0", "weight_block_size": [32, 32], "expert_dtype": "fp4"},
                                    "weight_block_size": [32, 32]})
    assert cfg.weight_block_size == [32, 32] and cfg.bits == 2 and cfg.codebook == "mcg"
    assert callable(m._exl3port_lm_head_method) and callable(m._exl3port_h2d_copy)
    from vllm.model_executor.model_loader import weight_utils as wu
    assert callable(wu.exl3port_h2d_copy)
    return f"vllm-exl3 {d.version}; weight_block_size copied; lm_head hook + staged H2D present; no entry point"

def natives():
    import torch  # noqa: F401
    import exllamav3_ext as e
    assert hasattr(e, "exl3_moe") and hasattr(e, "exl3_fat_gemm")
    import vllm_exl3_c as c
    assert callable(getattr(c, "p2b_fused_moe", None))
    lib = ctypes.CDLL(P["install"]["row_store_lib"])
    for s in ("row_store_open", "row_store_range", "row_store_stats", "row_store_attach_packed", "row_store_lookup"):
        getattr(lib, s)
    from vllm_exl3 import exl3 as m
    m.load_linear_exl3_cls()  # exllamav3 namespace stub path: LinearEXL3 importable without FlashAttention
    lic = pathlib.Path(P["install"]["licenses_dir"])
    for rel in ("vllm-exl3/LICENSE", "vllm-exl3/NOTICE", "vllm-exl3/THIRD_PARTY_NOTICES.md", "vllm-exl3/LICENSE.APACHE-2.0",
                "exllamav3/LICENSE", "mia-overlay/LICENSE", "mia-overlay/LICENSE.MIT"):
        assert (lic / rel).is_file(), f"license file missing: {lic / rel}"
    return "exllamav3_ext + vllm_exl3_c + librow_store.so + LinearEXL3 + license files"

def engram_hook():
    import vllm.models.deepseek_v41.common.engram as e
    from vllm.models.deepseek_v41.common import engram_file_backend as fb
    assert getattr(e, "_exl3port_engram_file", False), "dispatcher not installed on common/engram.py"
    assert e.ParallelEngramEmbedding.__init__.__name__ == "init"
    for k in ("DSV41_SOURCE", "ENGRAM_MOUNT"):
        os.environ.pop(k, None)
    assert fb._configured_table_dir() == "", "file backend must be opt-in (no table dir configured)"
    os.environ["DSV41_SOURCE"] = "/x"
    assert fb._configured_table_dir() == "/x"
    os.environ.pop("DSV41_SOURCE")
    # the audited attributes the 0.32.0 Engram reads on the embedding exist on the stock class (so stock path is intact)
    for a in ("lookup", "_storage", "_allocate_weights", "_get_shard_info", "forward"):
        assert hasattr(e.ParallelEngramEmbedding, a), a
    from vllm.model_executor.model_loader.ep_weight_filter import should_skip_weight
    for k in ("DSV41_SOURCE", "ENGRAM_MOUNT"):
        os.environ.pop(k, None)
    assert should_skip_weight("layers.1.engram.embed.weight", None) is False, "stock behaviour broken (no table dir)"
    os.environ["DSV41_SOURCE"] = "/x"
    try:
        for n in ("layers.1.engram.embed.weight", "layers.14.engram.embed.scale"):
            assert should_skip_weight(n, None) is True, n
        for n in ("layers.1.engram.wkv.weight", "layers.1.engram.wkv.scale", "layers.1.engram.q_weight", "embed.weight",
                  "layers.3.ffn.experts.7.w1.trellis"):
            assert should_skip_weight(n, None) is False, n
    finally:
        os.environ.pop("DSV41_SOURCE", None)
    return "dispatcher installed; opt-in gate OK; embed skip active only when a table dir is configured"

for name, fn in (("registry", registry), ("exl3-abstracts", abstracts), ("plugin-edits", plugin_edits),
                 ("natives", natives), ("engram-hook", engram_hook)):
    check(name, fn)
if fails:
    L.die(TAG, f"self-check failures: {fails}")
L.status("applied", "self-check PASS (registry, classes, plugin edits, natives, engram hook + embed skip)")
print(f"{TAG} OK")
PY
)
