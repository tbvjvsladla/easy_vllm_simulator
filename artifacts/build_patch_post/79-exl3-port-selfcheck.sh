#!/bin/bash
# 79-exl3-port-selfcheck.sh - 빌드-바깥 의존 패치(build_patches 슬롯 · post-compile) · EXL3 이식 정적 자체검사(GPU 불요)
#
# what : 컴파일된 vLLM 이 설치된 뒤에만 가능한 import 수준 검증을 한 곳에 모은다(PRE 위상에선 vllm 이 아직 import 불가라 80-87 은 구문/앵커/해시만 본다).  [round 2]
#          1) 레지스트리 훅: get_quantization_config("exl3") 가 **플러그인(vllm_exl3.exl3)의 진짜 Exl3Config** 로 해소(스텁 퇴화 아님 · vLLM QuantizationConfig 의 하위클래스)
#          2) Exl3 메서드 클래스가 추상 메서드 없이 구체인가(0.30.0 FusedMoEMethodBase/LinearMethodBase 대비)
#          3) 플러그인 편집: weight_block_size 복사 · MXFP8 헤드 훅 · 스테이징 H2D 심볼 · entry-point 부재 · 설치 버전
#          4) Engram 훅: nvidia/engram.py import 시 디스패처가 설치되고(_exl3port_engram_file) 옵트인 판정이 env 로 동작 + 87 의 embed 스킵 판정(비활성=통과 · 활성=스킵)
#          5) 네이티브: exllamav3_ext(exl3_moe 등) · vllm_exl3_c(p2b_fused_moe) · librow_store.so · 라이선스 파일 설치
# why  : "만든 것과 도는 것은 다르다" - 앵커가 맞아 편집이 들어갔어도 import 가 깨지면 서빙은 첫 부팅에서 죽는다. 이 검사는 그 부류를 빌드에서 잡는다.
#        GPU 가 필요한 검증(커널 실행, Engram lookup, KV 블록 shim, CUDA graph, MXFP8 헤드 GEMM)은 **여기서 하지 않는다 = UNTESTED**.
# model-trigger : SM12X_PORT=1 변종 이미지 한정.
# fail-loud : 하나라도 실패하면 빌드 실패(비-0).
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
    import vllm.models.deepseek_v41.nvidia.engram as e
    from vllm.models.deepseek_v41.nvidia import engram_file_backend as fb
    assert getattr(e, "_exl3port_engram_file", False), "dispatcher not installed on nvidia/engram.py"
    assert e.ParallelEngramEmbedding.__init__.__name__ == "init"
    for k in ("DSV41_SOURCE", "ENGRAM_MOUNT"):
        os.environ.pop(k, None)
    assert fb._configured_table_dir() == "", "file backend must be opt-in (no table dir configured)"
    os.environ["DSV41_SOURCE"] = "/x"
    assert fb._configured_table_dir() == "/x"
    os.environ.pop("DSV41_SOURCE")
    # the audited attributes the 0.30.0 Engram reads on the embedding exist on the stock class (so stock path is intact)
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
