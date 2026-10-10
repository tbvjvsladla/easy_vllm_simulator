#!/bin/bash
# 83-engram-file-backend.sh - **소스 이식 패치**(build_patches_src 슬롯 · pre-compile) · 디스크 기반 Engram(v0.30.0 재타깃)
#
# what : Mia 오버레이의 file-backed Engram(engram_file_backend.py + engram_layout.py)을 vLLM 트리 모듈로 설치하고,
#        **v0.30.0 클래스 배치에 맞춰 재타깃**한 훅을 CUDA 서브클래스 모듈 끝에 덧붙인다.
#          - vllm/models/deepseek_v41/nvidia/engram_file_backend.py  (Mia 파일 + 앵커 편집 9곳 · 아래 §재타깃)
#          - vllm/models/deepseek_v41/nvidia/engram_layout.py        (바이트 그대로 · 런타임 미사용 - 오프라인 pack_engram 도구용)
#          - vllm/models/deepseek_v41/nvidia/engram.py footer        (install(sys.modules[__name__]) 호출)
# why  : EXL3 팩에는 engram 임베딩 테이블이 없다(원본 DeepSeek-V4.1-Flash 샤드 47+48 에 있다). stock 은 테이블을 pinned UVA 로 올리는데
#        (레이어당 ~47GiB x 2) GB10 은 호스트 RAM = GPU 메모리라 EXL3 가중치(~99.5GiB/rank)와 함께 OOM 이다. 이 백엔드는 safetensors
#        샤드를 row_store(pread 32~96 스레드 + 선택적 packed 샤드)로 열고 cudaLaunchHostFunc 로 행을 모은다.
# §재타깃(0909 -> 0.30.0, delta report §0/§4 가 지목한 핵심 재작업):
#   * 0909: ParallelEngramEmbedding 하나가 common/engram.py 에 있었다. 0.30.0: 베이스(common/engram.py:645) + **CUDA 서브클래스
#     nvidia/engram.py:217**(자체 __init__/_allocate_weights/_storage/forward 소유 · engram_head_shard_rank 는 이 모듈 속성).
#     Mia 훅을 common/ 에 붙이면 서브클래스가 덮어쓴 메서드를 못 바꾸고 engram_head_shard_rank 도 없다 -> **nvidia/engram.py 에 붙인다.**
#   * 서브클래스 ctor 에 `dp_shared_memory` kwarg 가 생겼다(Engram._create_embedding 이 넘긴다) -> Mia init 시그니처에 추가,
#     dp_shared_memory 또는 engram DP size != 1 이면 NotImplementedError(우리는 TP=2/DP=1 · 그 밖은 GPU 검증 전 미지원 = fail-loud).
#   * 서브클래스가 읽는 속성(Engram._init_staging: embed_tokens.dp_size/cpu_offload · _ready_rows: dp_size · prepare_embeddings: lookup(background=True))을
#     Mia init 이 모두 세팅한다(dp_size=1, cpu_offload=True -> prefetch 스트림 경로 사용). dp_shared_memory=False 도 명시.
#   * **옵트인 디스패처**: 훅은 stock 동작을 보존한다. Engram 테이블 경로가 설정된 경우에만(env DSV41_SOURCE/ENGRAM_MOUNT 또는
#     `--hf-overrides '{"engram_table_dir": ...}'` -> hf_config(.text_config).engram_table_dir) file 백엔드가 쓰이고, 아니면 원래 구현.
#     Mia 원본의 `/engram-src` 묵시 기본값은 제거했다(침묵 폴백 -> 명시 요구).
#   * 행수 중복 시 레이어 오매칭을 막는 모호성 검사 추가(_layer_id_from_embeddings). row_store.cpp 는 행 = 256B fp8 + 8B ue8m0 스케일을 하드코딩하므로
#     dim!=256 또는 block_size!=32 이면 NotImplementedError(다른 형상 모델에 조용히 틀린 행을 서빙하지 않는다).
#   * librow_store.so 경로: 환경변수 EXL3_PORT_ROW_STORE_LIB -> provenance install.row_store_lib(POST 73 이 빌드) -> /opt/dsv41 -> 파일 옆.
# model-trigger : DeepseekV41 + EXL3 팩 + Engram 원본 샤드 디렉터리. 비-EXL3 모델/경로 미설정은 stock 경로(디스패처).
# provenance : 파일 2개는 Mia 커밋에서 빌드 시점에 받아 sha256 검증(PROVENANCE.json). 라이선스 AGPL-3.0(provenance 사실 참조).
# ⚠ UNTESTED(GPU 필요): file lookup 이 0.30.0 prefetch 스트림/eager-break/CUDA graph 와 맞물려 도는지, 속성 감사의 런타임 정합,
#   cudaLaunchHostFunc 가 piecewise 그래프 사이에서 안전한지. 최종중재 = GPU 서빙 스모크.
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

nv = L.VLLM_SRC / "vllm/models/deepseek_v41/nvidia"
if not (nv / "engram.py").is_file():
    L.die(TAG, f"{nv}/engram.py missing - 0.30.0 expects the CUDA Engram subclass here (NOT common/engram.py)")
src_nv = (nv / "engram.py").read_text()
if "class ParallelEngramEmbedding(BaseParallelEngramEmbedding)" not in src_nv or "def engram_head_shard_rank" not in src_nv:
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
        dp_shared_memory: bool = False,  {M} 0.30.0 Engram._create_embedding passes this kwarg
    ) -> None:
        del cpu_offload
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
''', f'''        self.cpu_offload = True  {M} makes 0.30.0 Engram create the prefetch stream (lookup runs on it)
        self.dp_shared_memory = False
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
    """Install the file-backed Engram on the 0.30.0 CUDA ParallelEngramEmbedding subclass.

    {M} retargeted from the 0909 common/engram.py class; opt-in per instance: the file backend is used
    only when an Engram table dir is configured, otherwise the original (pinned/UVA) implementation runs.
    Safe to call more than once.
    """
    global _INSTALLED
    if module is None:
        from vllm.models.deepseek_v41.nvidia import engram as module
    if getattr(module, "_exl3port_engram_file", False):
        _INSTALLED = True
        return
    cls = module.ParallelEngramEmbedding
    for attr in ("lookup", "_storage", "_allocate_weights", "_get_shard_info"):
        if not hasattr(cls, attr):
            raise RuntimeError(
                f"ParallelEngramEmbedding has no {{attr!r}}: vLLM drifted from the audited v0.30.0 layout"
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

    from vllm.models.deepseek_v41.nvidia import engram_file_backend

    engram_file_backend.install(sys.modules[__name__])


_exl3port_install_file_engram()
'''
out = L.apply_edits(TAG, "vllm/models/deepseek_v41/nvidia/engram.py", [], footer=FOOTER, footer_marker=FOOT_MARK)
print(f"{TAG} engram.py footer: {out}")

# ---- post-assert (static; import test needs the compiled vLLM -> build_patches/79 self-check)
L.py_compile(TAG, [dst, nv / "engram_layout.py", nv / "engram.py"])
final = dst.read_text()
for needle in ("_exl3port_file_backed", "_configured_table_dir", "dp_shared_memory: bool = False", "vllm.models.deepseek_v41.nvidia import engram as module"):
    if needle not in final:
        L.die(TAG, f"post-assert: {needle!r} missing from installed backend")
if "deepseek_v4_1" in final:
    L.die(TAG, "post-assert: stale deepseek_v4_1 path still in backend")
tail = (nv / "engram.py").read_text()
if tail.count(FOOT_MARK) != 1:
    L.die(TAG, "post-assert: engram.py footer count != 1")
n_new = 1 if any(o.endswith(":applied") for o in out) else 0
L.status("applied", "engram backend retargeted to nvidia/engram.py" + ("" if n_new else " (footer already present)"))
print(f"{TAG} OK")
PY
