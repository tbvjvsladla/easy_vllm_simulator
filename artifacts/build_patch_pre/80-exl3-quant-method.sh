#!/bin/bash
# 80-exl3-quant-method.sh - **소스 이식 패치**(build_patches_src 슬롯 · pre-compile) · EXL3 양자화 메서드 **등록 훅**(round 2: vllm-exl3 계보)
#
# what : stock vLLM v0.30.0 의 quantization/__init__.py 에 **지연 등록** 2곳을 넣는다. EXL3 양자화 메서드 본체(Exl3Config/Exl3MoEMethod)는 더 이상 이 트리에 복사하지 않는다 -
#        POST 73 이 pip 패키지 `vllm_exl3`(vcruz305/vllm-exl3 @ 핀)로 설치하고, 이 훅이 첫 `get_quantization_config()` 호출 시 `vllm_exl3.register()` 를 부른다.
#          (1) `QUANTIZATION_METHODS.append("exl3")`(커스텀 메서드로 · Literal 에는 넣지 않는다)
#          (2) get_quantization_config() 안의 지연 `import vllm_exl3; vllm_exl3.register()`(그 시점엔 vllm.config/fused_moe 가 완전히 import 돼 순환이 없다) +
#              **등록 사후 단언**: 플러그인 exl3.py 는 vllm import 가 중간에 실패하면 `except ImportError` 로 no-op 스텁(register_quantization_config 가 아무것도 안 함)으로 조용히 퇴화한다 -
#              그러면 'exl3' 가 미등록인 채로 조용히 지나가므로, 훅이 등록 여부를 단언해 **시끄럽게** 실패시킨다.
#        + files/chat_template.jinja -> /opt/dsv41/chat_template.jinja (서빙 레시피 선택 사용 · round 1 과 동일)
#        round 1(Mia 계보)과의 차이: overlay/exl3.py 복사 · exl3_k_map.json 설치를 **제거**(sfxnz 팩은 K=2 균일 · quantization_config.json/k_map 없음 - bits 는 config 에서 온다).
# why  : 0.30.0 은 register_quantization_config() 와 ModelConfig 의 커스텀 메서드 우선순위를 이미 갖고 있어 레지스트리 Literal 편집이 필요 없다.
#        out-of-tree 플러그인(entry-point)이 아니라 **소스 훅**을 택한 이유: 플러그인은 엔진코어가 --quantization 을 해소한 *뒤* 로드될 수 있다는 순서 불확실성이 있고(sfxnz sitecustomize 주석),
#        소스 훅은 get_quantization_config 가 불리는 모든 프로세스(API/엔진코어/Ray·mp 워커)에서 실행되어 순서 의존이 없다. .pth/sitecustomize 는 Ray/mp 워커에 안 닿는다(슬롯 판정).
#        ⚠ 실측으로 기각된 첫 설계(round 1): 패키지 __init__ 끝의 즉시 `from . import exl3` 훅 - `import vllm.config` 가 첫 import 일 때 순환 ImportError 가 try/except 에 삼켜져 exl3 가 조용히 미등록이 됐다.
#        트레이드오프: 양자화 체크포인트의 자동 탐지 루프가 매번 vllm_exl3 를 한 번 import 한다(가볍다 · 네이티브는 지연 로드). 플러그인이 미설치면 **시끄럽게 실패**한다(빌드 79 자체검사가 사전 포착).
#
# model-trigger : sfxnz 하이브리드 EXL3 팩(및 EXL3 라우티드 전문가 체크포인트 일반). SM12X_PORT=1 변종 이미지에서만 적용.
# provenance : PROVENANCE.json(핀의 단일 거처). chat_template 은 **빌드 시점에** Mia 고정 커밋에서 받아 sha256 을 검증한다(벤더링 ✗ · 불일치 = 빌드 실패).
# plan : delta report(overlay_delta_report.md) §3 · sfxnz_lineage_report.md §5 · 최종중재 = GPU 서빙 스모크(이 스크립트는 정적 이식만 보증)
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
