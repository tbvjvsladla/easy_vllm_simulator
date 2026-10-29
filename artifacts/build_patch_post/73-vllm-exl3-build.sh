#!/bin/bash
# Official vLLM v0.32.0 SM12x port candidate.
# Source pins: sibling build_patches_src/PROVENANCE.json; no previous GPU validation inherited.
# CPU source checks only so far; native ABI/import/GPU health/inference remain required.
# Historical development explanation is retained in pre-v032-buildkit, not current success evidence.
set -euo pipefail

TAG="[73-vllm-exl3-build]"

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
[ -n "$LIBDIR" ] || { echo "$TAG FAIL: exl3_port_lib.py / PROVENANCE.json 를 찾지 못했다(build_patches_src 와 함께 배달돼야 한다)" >&2; exit 1; }
export PYTHONPATH="$LIBDIR${PYTHONPATH:+:$PYTHONPATH}"

python3 - "$TAG" <<'PY'
import hashlib, json, os, pathlib, shutil, subprocess, sys
import exl3_port_lib as L
from exl3_port_lib import Edit

TAG = sys.argv[1]
P = L.prov()
inst, S = P["install"], P["sources"]
V = S["vllm_exl3"]
opt = pathlib.Path(inst["opt_dir"])
src = pathlib.Path(inst["vllm_exl3_src"])
ev3_marker = opt / ".exllamav3-built"
ev3_src = pathlib.Path(inst["exllamav3_patched_src"])
ext_inc = ev3_src / "exllamav3" / "exllamav3_ext"
if not ev3_marker.is_file() or not (ext_inc / "util.h").is_file():
    L.die(TAG, "70-exllamav3-build 가 먼저 성공해야 한다(패치된 exllamav3 소스 트리/마커 부재)")

M = "# [exl3-port:vllm-exl3]"

# ---------------------------------------------------------------- 편집(전부 앵커 · 정확한 발생 횟수 · fail-loud)
SETUP_OLD = '''                sources=[
                    str(ROOT / "csrc" / "bindings.cpp"),
                    str(ROOT / "csrc" / "exl3_gemv.cu"),
                    str(ROOT / "csrc" / "p2b_batched.cu"),
                    str(ROOT / "csrc" / "p2b_moe.cu"),
                    str(ROOT / "csrc" / "exl3_gemm.cu"),
                    str(ROOT / "csrc" / "exl3_fat_gemm.cu"),
                ],'''
SETUP_NEW = '''                sources=[  # [exl3-port:vllm-exl3] repo-relative: setuptools rejects absolute CUDAExtension sources
                    "csrc/bindings.cpp",
                    "csrc/exl3_gemv.cu",
                    "csrc/p2b_batched.cu",
                    "csrc/p2b_moe.cu",
                    "csrc/exl3_gemm.cu",
                    "csrc/exl3_fat_gemm.cu",
                ],'''
PYPROJECT_OLD = '\n[project.entry-points."vllm.general_plugins"]\nvllm_exl3 = "vllm_exl3:register"\n'
PYPROJECT_NEW = '\n# [exl3-port:vllm-exl3] entry point removed: registration only via the lazy hook in vllm quantization/__init__.py (patch 80)\n'

FROM_CFG_OLD = '''        inst.mtp_experts = str(config.get("mtp_experts", "exl3"))
        inst.mtp_experts_start_layer = config.get("mtp_experts_start_layer")
        return inst
'''
FROM_CFG_NEW = f'''        inst.mtp_experts = str(config.get("mtp_experts", "exl3"))
        inst.mtp_experts_start_layer = config.get("mtp_experts_start_layer")
        {M} vLLM 0.30.0's DeepSeek-V4.1 loader picks `weight_scale` vs `weight_scale_inv` (and pads shared
        # experts) from quant_config.weight_block_size; Exl3Config keeps the field only inside the raw config,
        # so copy it (same as sfxnz sitecustomize `exl3_weight_block_size`). Absent -> attribute stays unset.
        _nr = inst.non_routed_quantization if isinstance(inst.non_routed_quantization, dict) else {{}}
        _wbs = _nr.get("weight_block_size") or config.get("weight_block_size")
        if _wbs is not None:
            inst.weight_block_size = [int(_x) for _x in _wbs]
        return inst
'''

LMHEAD_OLD = '''                return Exl3LinearMethod(self, bits=self._bits_for_non_routed(prefix))
            return None
        if isinstance(layer, VocabParallelEmbedding):
'''
LMHEAD_NEW = f'''                return Exl3LinearMethod(self, bits=self._bits_for_non_routed(prefix))
            # {M[2:]}:lmhead-mxfp8 pack stores lm_head as MXFP8 (lm_head.weight e4m3 + lm_head.weight_scale)
            return _exl3port_lm_head_method(self, layer, prefix)
        if isinstance(layer, VocabParallelEmbedding):
'''

LOAD_OLD = '''        dest.copy_(sharded)
        return True if return_success else None

    def process_weights_after_loading(self, layer: torch.nn.Module) -> None:
        if not hasattr(layer, "w13_trellis"):
'''
LOAD_NEW = f'''        _exl3port_h2d_copy(dest, sharded)  {M} staged H2D (never DMA straight from the mmap'd shard)
        return True if return_success else None

    def process_weights_after_loading(self, layer: torch.nn.Module) -> None:
        if not hasattr(layer, "w13_trellis"):
'''

FOOT_MARK = f"{M} footer"
FOOTER = f'''
{FOOT_MARK}
def _exl3port_h2d_copy(dest, src):
    """dest.copy_(src) through vLLM's pinned staging helper (patch 82). Missing helper = fail loudly."""
    from vllm.model_executor.model_loader.weight_utils import exl3port_h2d_copy

    return exl3port_h2d_copy(dest, src)


_EXL3PORT_HEAD_CACHE: dict = {{}}


def _exl3port_lm_head_method(config, layer, prefix):
    """Quant method for ParallelLMHead when the checkpoint stores an MXFP8 head, else None (stock unquantized head).

    Detection = the local checkpoint index lists both `lm_head.weight` and `lm_head.weight_scale`. The method is vLLM's own
    ModelOptLinearMethod(MXFP8: e4m3 weight + per-32 e8m0 scale, dynamic MXFP8 activation) - ParallelLMHead is explicitly
    supported by its create_weights (0.30.0). CkptCtx() = scale block (1, 32), i.e. the stored [N, K/32] scale is used as is.
    """
    import json
    import os

    from vllm.config import get_current_vllm_config

    model_dir = str(get_current_vllm_config().model_config.model)
    hit = _EXL3PORT_HEAD_CACHE.get(model_dir)
    if hit is None:
        idx = os.path.join(model_dir, "model.safetensors.index.json")
        wm = {{}}
        if os.path.isfile(idx):
            with open(idx) as fh:
                wm = json.load(fh).get("weight_map", {{}})
        hit = ("lm_head.weight" in wm and "lm_head.weight_scale" in wm)
        _EXL3PORT_HEAD_CACHE[model_dir] = hit
        logger.info("exl3-port: lm_head %s (index %s)", "MXFP8 via ModelOptLinearMethod" if hit else "unquantized", idx)
    if not hit:
        return None
    from vllm.config.quantization import QuantSpec
    from vllm.model_executor.layers.quantization.modelopt import CkptCtx, ModelOptLinearMethod
    from vllm.model_executor.layers.quantization.utils.quant_utils import kMxfp8Dynamic, kMxfp8Static

    return ModelOptLinearMethod(QuantSpec(weight=kMxfp8Static, activation=kMxfp8Dynamic), CkptCtx())
'''

# ---- (4) p2b chain: the author's scripts (sources.sfxnz_p2b), run in the author's canonical order BEFORE our edits.
# Order = sfxnz docker/Dockerfile (canonical-e14). widen_p2b_mma.py is deliberately absent (Dockerfile.mma experiment, decode regression).
P2B_CHAIN = ["shapes", "mrow", "cfg1", "codebook", "fshift", "srcsort", "coop", "dataflow"]
X = S["sfxnz_p2b"]
P2B_SCRIPTS = [f"docker/patch/widen_p2b_{n}.py" for n in P2B_CHAIN]
if P2B_SCRIPTS != X["chain"] or set(P2B_SCRIPTS) != set(X["files_sha256"]):
    L.die(TAG, "PROVENANCE sources.sfxnz_p2b chain/files_sha256 disagree with the chain this script runs")
# Strings the chained source / built .so must carry (the author's own Dockerfile assert + the shapes/mrow message).
P2B_SO_NEEDLES = [b"positive multiple of 128", b"DSV41_P2B_SRC_SORT", b"DSV41_P2B_COOP", b"p2b coop dataflow kernel engaged"]

# ---- (5) p2b-engaged: anchored on the POST-chain text (codebook inserts the flags block above extra_args; mrow removes the row loop).
# Engagement proof (the load-time "fused_moe=exl3_moe" line is printed for every backend, so it proves nothing about native).
P2B_PY_LOG_OLD = '''    extra_args = (intermediate, clamp_limit) if extended_abi else ()
    result = fn(
        xh,
        native_out,
'''
P2B_PY_LOG_NEW = f'''    extra_args = (intermediate, clamp_limit) if extended_abi else ()
    getattr(logger, "info_once", logger.info)(  {M} p2b-engaged
        "exl3-port: native p2b_fused_moe engaged rows=%d hidden=%d inter=%d k=%d mcg=%s abi=%s coop=%s src_sort=%s",
        int(x2d.shape[0]), int(x2d.shape[1]), intermediate, k, mcg, getattr(module, "P2B_MOE_ABI_VERSION", 1),
        os.environ.get("DSV41_P2B_COOP", "unset"), os.environ.get("DSV41_P2B_SRC_SORT", "unset"))
    result = fn(
        xh,
        native_out,
'''

JOBS = [
    ("setup.py", [Edit("setup-relative-sources", SETUP_OLD, SETUP_NEW)], None, None),
    ("pyproject.toml", [Edit("drop-entry-point", PYPROJECT_OLD, PYPROJECT_NEW)], None, None),
    ("src/vllm_exl3/exl3.py",
     [Edit("weight_block_size", FROM_CFG_OLD, FROM_CFG_NEW), Edit("lm_head-mxfp8", LMHEAD_OLD, LMHEAD_NEW),
      Edit("h2d-staging", LOAD_OLD, LOAD_NEW), Edit("p2b-engaged-log", P2B_PY_LOG_OLD, P2B_PY_LOG_NEW)], FOOTER, FOOT_MARK),
]
edit_id = hashlib.sha256(json.dumps([[e.label, e.old, e.new] for _, es, _, _ in JOBS for e in es] + [FOOTER]).encode()).hexdigest()[:16]
p2b_id = hashlib.sha256(json.dumps([[s, X["files_sha256"][s]] for s in P2B_SCRIPTS]).encode()).hexdigest()[:16]
want_marker = (f"vllm_exl3={V['commit']} tree={V['tree_digest']['sha256']} edits={edit_id} "
               f"sfxnz={X['commit']} p2b={','.join(P2B_CHAIN)}:{p2b_id} "
               f"ev3={ev3_marker.read_text().strip()} arch={P['build']['torch_cuda_arch_list']}\n")
marker = opt / ".vllm-exl3-built"

def check_installed() -> tuple[bool, str]:
    """Import-level check run in a clean interpreter (cwd=/ so no source tree shadows the install)."""
    code = (
        "import importlib.metadata as m, importlib.util as u, pathlib, torch, vllm_exl3_c\n"
        "assert callable(getattr(vllm_exl3_c,'p2b_fused_moe',None)), 'p2b_fused_moe missing'\n"
        "import vllm_exl3\n"
        "d = m.distribution('vllm-exl3'); assert d.version == %r, d.version\n"
        "eps = [e for e in m.entry_points(group='vllm.general_plugins') if e.name == 'vllm_exl3']\n"
        "assert not eps, 'entry point still installed: %%r' %% eps\n"
        "f = pathlib.Path(u.find_spec('vllm_exl3').origin).with_name('exl3.py'); t = f.read_text()\n"
        "assert t.count(%r) >= 5, 'edit markers missing in installed exl3.py'\n"
        "assert 'p2b-engaged' in t, 'p2b-engaged log edit missing'\n"
        "assert '== hidden_meta == 4096' not in t and 'hidden_meta %% 128 == 0' in t, 'sfxnz shapes gate not in installed exl3.py'\n"
        "assert 'for row in range(int(x2d.shape[0])):' not in t, 'sfxnz mrow: per-row python loop still present'\n"
        "assert 'flags = getattr(layer, \"_exl3_codebook_flags\"' in t, 'sfxnz codebook: mcg/mul1 dispatch missing'\n"
        "so = open(vllm_exl3_c.__file__, 'rb').read()\n"
        "need = %r\n"
        "miss = [s for s in need if s not in so]; assert not miss, 'vllm_exl3_c lacks sfxnz p2b chain strings: %%r' %% miss\n"
        "assert b'hidden width 4096' not in so, 'vllm_exl3_c still carries the stock 4096 gate'\n"
        "print(f, d.version, 'p2b-chain-so-ok')\n" % (V["version"], M, P2B_SO_NEEDLES))
    r = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True, cwd="/")
    return r.returncode == 0, (r.stdout.strip() if r.returncode == 0 else r.stderr.strip()[-800:])

if marker.is_file() and marker.read_text() == want_marker:
    ok, msg = check_installed()
    if ok:
        print(f"{TAG} already built for these pins/edits - no-op ({msg})")
        L.status("applied", "already built (no-op rerun)")
        raise SystemExit(0)
    print(f"{TAG} marker matches but install check failed ({msg}); rebuilding")

# 사전 정적 점검: 82 가 설치한 H2D 헬퍼가 vLLM 트리에 있어야 편집 (c) 가 런타임에 쓸 수 있다.
wu = (L.VLLM_SRC / "vllm/model_executor/model_loader/weight_utils.py")
if not wu.is_file() or "def exl3port_h2d_copy(" not in wu.read_text():
    L.die(TAG, "vllm weight_utils.py lacks exl3port_h2d_copy - run PRE patch 82 before building the plugin")

# ---------------------------------------------------------------- 받기 + 편집 + 빌드
marker.unlink(missing_ok=True)
L.fetch_pinned_tree(TAG, "vllm_exl3", src)

# (4) sfxnz p2b chain - fetched at the pin (tree digest + per-script sha256 + LICENSE sha256), run, then removed from the image.
sfx_root = pathlib.Path(inst["fetch_cache"]) / f"sfxnz_p2b-{X['commit']}"
L.fetch_pinned_tree(TAG, "sfxnz_p2b", sfx_root)
for f, want in X["license"]["license_sha256"].items():
    if L.sha256_file(sfx_root / f) != want:
        L.die(TAG, f"sfxnz {f} hash mismatch")
sfx_license = {f: (sfx_root / f).read_bytes() for f in X["license"]["license_sha256"]}
cu_path, py_path = src / "csrc/p2b_moe.cu", src / "src/vllm_exl3/exl3.py"
for rel in P2B_SCRIPTS:
    before = (L.sha256_file(cu_path), L.sha256_file(py_path))
    r = subprocess.run([sys.executable, str(sfx_root / rel), str(src)], capture_output=True, text=True)
    sys.stdout.write(r.stdout)
    if r.returncode != 0:
        L.die(TAG, f"sfxnz {rel} failed rc={r.returncode}: {(r.stderr or r.stdout).strip()[-600:]}")
    if (L.sha256_file(cu_path), L.sha256_file(py_path)) == before:
        L.die(TAG, f"sfxnz {rel} was a no-op on a fresh tree (its idempotency short-circuit fired = foreign/partial edit)")
    print(f"{TAG} p2b chain: {rel} applied")
shutil.rmtree(sfx_root, ignore_errors=True)
(pathlib.Path(inst["fetch_cache"]) / f"sfxnz_p2b-{X['commit']}.tar.gz").unlink(missing_ok=True)
_cu = cu_path.read_text()
_miss = [s.decode() for s in P2B_SO_NEEDLES if s.decode() not in _cu]
if _miss or "x.size(1) == 4096" in _cu or "hidden = 4096" in _cu:
    L.die(TAG, f"p2b chain: csrc/p2b_moe.cu missing {_miss} or still hardcodes hidden 4096")

# our edits go on top of the chained tree (p2b-engaged is anchored on post-chain text)
rep = L.apply_edit_jobs(TAG, JOBS, root=src)
for rel, out in rep:
    print(f"{TAG} {rel}: {', '.join(out)}")
if any(o.endswith(":already") for _, out in rep for o in out):
    L.die(TAG, "fresh tree unexpectedly already edited")
L.py_compile(TAG, [src / "setup.py", src / "src/vllm_exl3/exl3.py"])

env = dict(os.environ)
env["TORCH_CUDA_ARCH_LIST"] = P["build"]["torch_cuda_arch_list"]
env["EXL3_EXT_INCLUDE"] = str(ext_inc)      # setup.py: headers of the patched exllamav3 tree (70), not a guessed path
env.setdefault("MAX_JOBS", "8")
if not pathlib.Path("/usr/local/cuda/include/cuda_runtime.h").is_file():
    import site
    cu13 = pathlib.Path(site.getsitepackages()[0]) / "nvidia" / "cu13" / "include"
    if cu13.is_dir():
        for k in ("CPATH", "CPLUS_INCLUDE_PATH", "C_INCLUDE_PATH"):
            env[k] = f"{cu13}" + (":" + env[k] if env.get(k) else "")
print(f"{TAG} building vllm-exl3 (arch={env['TORCH_CUDA_ARCH_LIST']} headers={ext_inc}) ...", flush=True)
subprocess.run([sys.executable, "-m", "pip", "uninstall", "-y", "vllm-exl3"], capture_output=True)
r = subprocess.run([sys.executable, "-m", "pip", "install", "--no-deps", "--no-build-isolation", "--no-cache-dir", "--force-reinstall", "."],
                   cwd=src, env=env)
if r.returncode != 0:
    L.die(TAG, f"pip install vllm-exl3 failed rc={r.returncode}")

# ---------------------------------------------------------------- 라이선스 파일 설치(해시 검증 후)
lic = pathlib.Path(inst["licenses_dir"])
if lic.exists():
    shutil.rmtree(lic)
(lic / "vllm-exl3").mkdir(parents=True)
for f in ("LICENSE", "LICENSE.APACHE-2.0", "NOTICE", "THIRD_PARTY_NOTICES.md"):
    if L.sha256_file(src / f) != V["files_sha256"][f]:
        L.die(TAG, f"license file {f} hash mismatch before install")
    shutil.copyfile(src / f, lic / "vllm-exl3" / f)
(lic / "exllamav3").mkdir()
shutil.copyfile(ev3_src / "LICENSE", lic / "exllamav3" / "LICENSE")
mia = L.fetch_mia_tree(TAG)
(lic / "mia-overlay").mkdir()
for f, want in S["mia_overlay"]["license"]["license_sha256"].items():
    if L.sha256_file(mia / f) != want:
        L.die(TAG, f"Mia {f} hash mismatch")
    shutil.copyfile(mia / f, lic / "mia-overlay" / f)
(lic / "sfxnz-p2b").mkdir()
for f, b in sfx_license.items():
    (lic / "sfxnz-p2b" / f).write_bytes(b)
(lic / "README").write_text(
    "License texts of third-party code baked into this image by the SM12X_PORT=1 patches (see PROVENANCE.json for pins).\n"
    "vllm-exl3: AGPL-3.0-only (modified at build time by build_patches/73) - NOTICE + THIRD_PARTY_NOTICES.md MUST be retained with any redistribution.\n"
    "exllamav3: MIT (Turboderp). mia-overlay: AGPL-3.0 (LICENSE) + MIT (LICENSE.MIT).\n"
    "sfxnz-p2b: MIT (LICENSE) - build-time p2b kernel scripts (widen_p2b_*) whose generated CUDA is compiled into vllm_exl3_c.\n")
print(f"{TAG} licenses installed -> {lic}")

# 빌드 작업 트리 정리(설치본만 남긴다)
shutil.rmtree(src, ignore_errors=True)

ok, msg = check_installed()
if not ok:
    L.die(TAG, f"post-assert failed: {msg}")
print(f"{TAG} installed: {msg}")
marker.write_text(want_marker)
L.status("applied", f"vllm-exl3 {V['version']} @ {V['commit'][:12]} + sfxnz p2b {'/'.join(P2B_CHAIN)} @ {X['commit'][:12]} built for {P['build']['torch_cuda_arch_list']}")
print(f"{TAG} OK")
PY
