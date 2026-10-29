#!/bin/bash
# Official vLLM v0.32.0 SM12x port candidate.
# Source pins: sibling build_patches_src/PROVENANCE.json; no previous GPU validation inherited.
# CPU source checks only so far; native ABI/import/GPU health/inference remain required.
# Historical development explanation is retained in pre-v032-buildkit, not current success evidence.
set -euo pipefail

TAG="[80-exl3-quant-method]"

: "${SM12X_PORT:=0}"
if [ "${SM12X_PORT}" != "1" ]; then
    echo "$TAG skip - SM12X_PORT=${SM12X_PORT} (stock 빌드 경로 불변)"
    if [ -n "${EASY_VLLM_PATCH_STATUS:-}" ]; then printf 'skipped\tSM12X_PORT!=1\n' > "$EASY_VLLM_PATCH_STATUS"; fi
    exit 0
fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PYTHONPATH="$HERE${PYTHONPATH:+:$PYTHONPATH}"
DST="${EXL3_PORT_VLLM_SRC:-/workspace/vllm-src}"
[ -d "$DST/vllm/model_executor/layers/quantization" ] || { echo "$TAG FAIL: vLLM 소스트리 부재: $DST" >&2; exit 1; }

python3 - "$TAG" <<'PY'
import os, pathlib, shutil, sys
import exl3_port_lib as L

TAG = sys.argv[1]
P = L.prov()
root = L.VLLM_SRC
# 대상 vLLM 버전 가드: 이 이식은 v0.30.0 레이아웃 전용이다(HEAD sha 가 핀과 같거나, 태그가 같아야 한다 - 얕은/분리 클론 모두 수용).
import subprocess
def _git(*a):
    try:
        return subprocess.run(["git", "-C", str(root), *a], capture_output=True, text=True).stdout.strip()
    except Exception:
        return ""
head, desc = _git("rev-parse", "HEAD"), _git("describe", "--tags", "--exact-match")
want_tag, want_sha = P["target_vllm"]["tag"], P["target_vllm"]["sha"]
if head != want_sha and desc != want_tag:
    L.die(TAG, f"소스 ref 불일치 - 이 이식은 {want_tag}({want_sha}) 전용이다(현재 HEAD={head or 'unknown'} describe={desc or 'none'}).")

L.refuse_round1_residue(TAG)  # v2 는 stock 트리 전용(round-1 편집이 남은 트리 위에 얹으면 훅이 이중으로 들어간다)

# (1) chat template (Mia 고정 커밋 · 해시 검증)
inst = P["install"]
s = L.mia_file(TAG, "files/chat_template.jinja")
d = pathlib.Path(inst["chat_template_path"]); d.parent.mkdir(parents=True, exist_ok=True)
shutil.copyfile(s, d)
if L.sha256_file(d) != P["sources"]["mia_overlay"]["files_sha256"]["files/chat_template.jinja"]:
    L.die(TAG, "files/chat_template.jinja 설치 후 해시 불일치")
print(f"{TAG} installed files/chat_template.jinja -> {d}")

# (2) 지연 등록(앵커 2곳)
from exl3_port_lib import Edit
M = "# [exl3-port:register]"
EDITS = [
    Edit("methods-list", "QUANTIZATION_METHODS: list[str] = list(get_args(QuantizationMethods))\n",
         "QUANTIZATION_METHODS: list[str] = list(get_args(QuantizationMethods))\n"
         f"{M} 'exl3' is a CUSTOM method: listed here but deliberately NOT in the QuantizationMethods Literal (ModelConfig's\n"
         "# override-order guard only rejects Literal members missing from its overrides list). Class is imported lazily below.\n"
         'QUANTIZATION_METHODS.append("exl3")\n'),
    Edit("lazy-import", "    # Update the `method_to_config` with customized quantization methods.\n"
         "    method_to_config.update(_CUSTOMIZED_METHOD_TO_QUANT_CONFIG)\n",
         f"    {M} lazy import: vllm.config / fused_moe are fully imported by now (an eager import at package-import time hits a\n"
         "    # circular import when vllm.config is imported first). vllm_exl3.register() imports vllm_exl3.exl3, whose decorator registers\n"
         "    # Exl3Config, and installs the per-bit native row policy (idempotent). Failures propagate.\n"
         '    if "exl3" not in _CUSTOMIZED_METHOD_TO_QUANT_CONFIG:\n'
         "        import vllm_exl3 as _exl3_port_pkg\n\n"
         "        _exl3_port_pkg.register()\n"
         '        if "exl3" not in _CUSTOMIZED_METHOD_TO_QUANT_CONFIG:  # the plugin degrades to no-op stubs if a vllm import failed mid-way\n'
         "            raise RuntimeError(\n"
         '                "vllm_exl3 was imported but quantization method \'exl3\' is not registered "\n'
         '                "(vllm_exl3.exl3 fell back to its no-vLLM stubs: an import inside it failed)"\n'
         "            )\n\n"
         "    # Update the `method_to_config` with customized quantization methods.\n"
         "    method_to_config.update(_CUSTOMIZED_METHOD_TO_QUANT_CONFIG)\n"),
]
out = L.apply_edits(TAG, "vllm/model_executor/layers/quantization/__init__.py", EDITS)
print(f"{TAG} lazy registration: {out}")

L.py_compile(TAG, [root / "vllm/model_executor/layers/quantization/__init__.py"])
L.status("applied", "lazy registration hook + chat template installed")
PY

# post-assert 주의: PRE 위상에서는 vLLM 이 아직 컴파일/설치 전이라(_C·_version 부재) `import vllm` 과 플러그인(POST 73)이 신뢰할 수 없다.
#   import 시험(레지스트리 해소 · 클래스 import)은 컴파일 후 슬롯 build_patches/79-exl3-port-selfcheck.sh 가 소유한다.
#   여기서는 구문(py_compile) + 앵커/마커만 정적으로 보증한다(위 python 블록).
echo "$TAG OK"
