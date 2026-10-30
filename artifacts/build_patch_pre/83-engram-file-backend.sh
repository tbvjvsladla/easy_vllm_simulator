#!/bin/bash
# Official vLLM v0.32.0 SM12x port candidate.
# Source pins: sibling build_patches_src/PROVENANCE.json; no previous GPU validation inherited.
# CPU source checks only so far; native ABI/import/GPU health/inference remain required.
# Historical development explanation is retained in pre-v032-buildkit, not current success evidence.
set -euo pipefail

TAG="[83-engram-file-backend]"

: "${SM12X_PORT:=0}"
if [ "${SM12X_PORT}" != "1" ]; then
    echo "$TAG skip - SM12X_PORT=${SM12X_PORT} (stock 빌드 경로 불변)"
    if [ -n "${EASY_VLLM_PATCH_STATUS:-}" ]; then printf 'skipped\tSM12X_PORT!=1\n' > "$EASY_VLLM_PATCH_STATUS"; fi
    exit 0
fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PYTHONPATH="$HERE${PYTHONPATH:+:$PYTHONPATH}"

python3 - "$TAG" <<'PY'
import pathlib, shutil, sys
import exl3_port_lib as L
from exl3_port_lib import Edit

TAG = sys.argv[1]
P = L.prov()
ROW_LIB = P["install"]["row_store_lib"]
M = "# [exl3-port:engram]"

nv = L.VLLM_SRC / "vllm/models/deepseek_v41/common"
if not (nv / "engram.py").is_file():
    L.die(TAG, f"{nv}/engram.py missing - 0.32.0 expects the CUDA Engram subclass here (NOT common/engram.py)")
src_nv = (nv / "engram.py").read_text()
if "class ParallelEngramEmbedding(nn.Module)" not in src_nv or "def engram_head_shard_rank" not in src_nv:
    L.die(TAG, "nvidia/engram.py no longer has the audited ParallelEngramEmbedding subclass / engram_head_shard_rank")

# ---- 1) engram_file_backend.py: pinned fetch -> anchored retarget edits (fresh from the pinned bytes every run)
text = L.mia_file(TAG, "overlay/engram_file_backend.py").read_text()

E_LIB = Edit("lib-path", '''_LIB_CANDIDATES = (
    Path("/opt/dsv41/librow_store.so"),
    Path(__file__).resolve().parent / "librow_store.so",
)
''', f'''{M} row_store lib: env override, then the POST-slot install location, then legacy path, then beside this file.
_LIB_CANDIDATES = tuple(
    Path(_p)
    for _p in (os.environ.get("EXL3_PORT_ROW_STORE_LIB", ""), "{ROW_LIB}", "/opt/dsv41/librow_store.so")
    if _p
) + (Path(__file__).resolve().parent / "librow_store.so",)
''')
E_LIBMSG = Edit("lib-msg", '''        "librow_store.so missing (rebuild dsv41-flash-exl3:local)"''',
                '''        "librow_store.so missing (built by build_patches/72-exl3-row-store.sh; or set EXL3_PORT_ROW_STORE_LIB)"''')
E_TABLE = Edit("table-dir", '''def _table_dir() -> Path:
    env = os.environ.get("DSV41_SOURCE") or os.environ.get("ENGRAM_MOUNT") or ""
    if env:
        return Path(env)
    try:
        from vllm.config import get_current_vllm_config

        cfg = get_current_vllm_config().model_config.hf_config
        path = getattr(cfg, "engram_table_dir", None) or ""
        if path:
            return Path(path)
    except Exception:
        pass
    return Path("/engram-src")
''', f'''def _configured_table_dir() -> str:
    {M} explicit opt-in: env DSV41_SOURCE / ENGRAM_MOUNT, else hf_config(.text_config).engram_table_dir
    # (set via --hf-overrides '{{"engram_table_dir": ...}}'). Empty string = file-backed Engram not requested.
    env = os.environ.get("DSV41_SOURCE") or os.environ.get("ENGRAM_MOUNT") or ""
    if env:
        return env
    try:
        from vllm.config import get_current_vllm_config

        mc = get_current_vllm_config().model_config
        for cfg in (mc.hf_config, getattr(mc, "hf_text_config", None)):
            path = getattr(cfg, "engram_table_dir", None) or ""
            if path:
                return str(path)
    except Exception as exc:  # no current vllm config -> not requested (logged, not silent)
        logger.warning(
            "engram_table_dir lookup failed (%s: %s); file-backed Engram not requested",
            type(exc).__name__,
            exc,
        )
    return ""


def _table_dir() -> Path:
    path = _configured_table_dir()
    if not path:
        raise RuntimeError(
            "file-backed Engram: no table dir (pass --hf-overrides engram_table_dir or set DSV41_SOURCE)"
        )
    return Path(path)
''')
E_LAYER = Edit("layer-id-unique", '''    for layer_id, n in zip(ids, rows, strict=True):
        if n == int(num_embeddings):
            return layer_id
    raise RuntimeError(
        f"no Engram layer with {num_embeddings} rows in {table_dir}/config.json"
    )
''', f'''    {M} row count must identify exactly one layer (two layers with equal rows would silently share a table)
    matches = [
        layer_id
        for layer_id, n in zip(ids, rows, strict=True)
        if n == int(num_embeddings)
    ]
    if len(matches) == 1:
        return matches[0]
    raise RuntimeError(
        f"Engram layer lookup by row count is not unique: {{num_embeddings}} rows matched "
        f"layers {{matches}} in {{table_dir}}/config.json"
    )
''')
E_SIG = Edit("init-signature", '''        block_size: int = 32,
        cpu_offload: bool = False,
    ) -> None:
        del cpu_offload
        nn.Module.__init__(self)
''', f'''        block_size: int = 32,
        cpu_offload: bool = False,
        dp_shared_memory: bool = False,  {M} common Engram._create_embedding passes these kwargs
        use_thp: bool = False,
    ) -> None:
        del cpu_offload
        if use_thp:
            raise NotImplementedError("file-backed Engram does not support use_thp")
        if dp_shared_memory:
            raise NotImplementedError(
                "file-backed Engram does not support dp_shared_memory (untested; use TP-only, DP=1)"
            )
        if int(dim) != 256 or int(block_size) != 32:  # row_store.cpp hard-codes 256 B fp8 weight + 8 B ue8m0 scale per row
            raise NotImplementedError(
                f"file-backed Engram row store supports dim=256/block_size=32 only (got dim={{dim}} block_size={{block_size}})"
            )
        nn.Module.__init__(self)
''')
E_DP = Edit("dp-size-guard", '''        dp_size = get_engram_dp_size()
''', f'''        dp_size = get_engram_dp_size()
        if dp_size != 1:  {M} DP-sharded engram forward path is unaudited for the file backend
            raise NotImplementedError(
                f"file-backed Engram supports engram DP size 1 only (got {{dp_size}})"
            )
''')
E_FLAG = Edit("instance-flags", '''        self.cpu_offload = True
        self._views = None
        self._view_src = None
''', f'''        self.cpu_offload = True  {M} makes 0.32.0 Engram create the prefetch stream (lookup runs on it)
        self.dp_shared_memory = False
        self.use_thp = False
        self._packed = None
        self._shared_memory = None
        self._exl3port_file_backed = True
        self._views = None
        self._view_src = None
''')
E_INSTALL = Edit("install-dispatcher", '''def install(module=None) -> None:
    """Replace ParallelEngramEmbedding init/lookup. Safe to call more than once."""
    global _INSTALLED
    if module is None:
        from vllm.models.deepseek_v4_1.common import engram as module
    if getattr(module, "_dsv41_engram_file", False):
        _INSTALLED = True
        return
    module.ParallelEngramEmbedding.__init__ = _make_init(module)
    module.ParallelEngramEmbedding.lookup = _make_lookup(module)
    module.ParallelEngramEmbedding._storage = _make_storage()
    module._dsv41_engram_file = True
    _INSTALLED = True
    logger.info("installed file-backed Engram (row_store, no 47GiB pin)")
''', f'''def install(module=None) -> None:
    """Install the file-backed Engram on the 0.32.0 common ParallelEngramEmbedding.

    {M} retargeted from the 0909 common/engram.py class; opt-in per instance: the file backend is used
    only when an Engram table dir is configured, otherwise the original (pinned/UVA) implementation runs.
    Safe to call more than once.
    """
    global _INSTALLED
    if module is None:
        from vllm.models.deepseek_v41.common import engram as module
    if getattr(module, "_exl3port_engram_file", False):
        _INSTALLED = True
        return
    cls = module.ParallelEngramEmbedding
    for attr in ("lookup", "_storage", "_allocate_weights", "_get_shard_info"):
        if not hasattr(cls, attr):
            raise RuntimeError(
                f"ParallelEngramEmbedding has no {{attr!r}}: vLLM drifted from the audited v0.32.0 layout"
            )
    orig_init, orig_lookup, orig_storage = cls.__init__, cls.lookup, cls._storage
    file_init, file_lookup = _make_init(module), _make_lookup(module)

    def init(self, *args, **kwargs):
        if _configured_table_dir():
            return file_init(self, *args, **kwargs)
        return orig_init(self, *args, **kwargs)

    def lookup(self, indices, out, background=False):
        if getattr(self, "_exl3port_file_backed", False):
            return file_lookup(self, indices, out, background)
        return orig_lookup(self, indices, out, background)

    def _storage(self):
        if getattr(self, "_exl3port_file_backed", False):
            raise RuntimeError(
                "file-backed Engram has no UVA table; lookup() gathers rows from the row store"
            )
        return orig_storage(self)

    cls.__init__ = init
    cls.lookup = lookup
    cls._storage = _storage
    module._exl3port_engram_file = True
    _INSTALLED = True
    logger.info("installed file-backed Engram dispatcher (active only when an Engram table dir is configured)")
''')

for e in (E_LIB, E_LIBMSG, E_TABLE, E_LAYER, E_SIG, E_DP, E_FLAG, E_INSTALL):
    text, st = e.apply(text, TAG, "engram_file_backend.py")
    if st != "applied":
        L.die(TAG, f"retarget edit {e.label} unexpectedly {st} on pristine bytes")
dst = nv / "engram_file_backend.py"
dst.write_text(text)
print(f"{TAG} installed retargeted {dst} (8 anchored edits)")

# ---- 2) engram_layout.py verbatim
lay = L.mia_file(TAG, "overlay/engram_layout.py")
shutil.copyfile(lay, nv / "engram_layout.py")
if L.sha256_file(nv / "engram_layout.py") != P["sources"]["mia_overlay"]["files_sha256"]["overlay/engram_layout.py"]:
    L.die(TAG, "engram_layout.py install hash mismatch")

# ---- 3) footer on nvidia/engram.py (idempotent)
FOOT_MARK = "# [exl3-port:engram-file-install]"
FOOTER = f'''
{FOOT_MARK}
def _exl3port_install_file_engram() -> None:
    import sys

    from vllm.models.deepseek_v41.common import engram_file_backend

    engram_file_backend.install(sys.modules[__name__])


_exl3port_install_file_engram()
'''
out = L.apply_edits(TAG, "vllm/models/deepseek_v41/common/engram.py", [], footer=FOOTER, footer_marker=FOOT_MARK)
print(f"{TAG} engram.py footer: {out}")

# ---- post-assert (static; import test needs the compiled vLLM -> build_patches/79 self-check)
L.py_compile(TAG, [dst, nv / "engram_layout.py", nv / "engram.py"])
final = dst.read_text()
for needle in ("_exl3port_file_backed", "_configured_table_dir", "dp_shared_memory: bool = False", "vllm.models.deepseek_v41.common import engram as module"):
    if needle not in final:
        L.die(TAG, f"post-assert: {needle!r} missing from installed backend")
if "deepseek_v4_1" in final:
    L.die(TAG, "post-assert: stale deepseek_v4_1 path still in backend")
tail = (nv / "engram.py").read_text()
if tail.count(FOOT_MARK) != 1:
    L.die(TAG, "post-assert: engram.py footer count != 1")
n_new = 1 if any(o.endswith(":applied") for o in out) else 0
L.status("applied", "engram backend retargeted to common/engram.py" + ("" if n_new else " (footer already present)"))
print(f"{TAG} OK")
PY
