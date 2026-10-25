#!/bin/bash
# arm_patch.sh — 모델구동 런타임 패치 arming (메인 저작 빌딩블럭 · 제네릭 · 결정론).
#
# 헌법 「모델구동 런타임 패치 따름정리」의 *결정론 메커니즘(틀)*. 패치 *내용*(<model>_patch.py)은
# 확률론·휘발(에이전트가 참조-그라운디드로 생성, 비추적)이지만, 이 arming 은 결정론·추적·메인 저작이다.
#
# 동작: /app/configs/${CONFIG_FILE}_patch.py 가 있으면 site-packages(purelib)에 .pth 한 줄을 써서
#   이후 시작되는 *모든* 파이썬 프로세스(vLLM engine + 로컬 TP worker + Ray worker)가 import 시점에
#   그 패치를 자동 로드하게 한다(.pth = python site-init 마다 자동 실행).
# 멱등: 같은 .pth 를 다시 써도 무해(serve_runner 양노드 + 단일 model.sh 양쪽에서 호출 가능).
# 전제: 패치 모듈은 **site-init safe(lazy meta-path finder)** — 최상위에서 transformers import 금지.
#   (어기면 그 이미지로 뜨는 모든 모델의 site-init 이 깨진다 — 헌법 패치 작성 규칙.)
# 통로: serve_runner.sh(멀티 양노드)가 source · gen_recipe_set 생성 model.sh(단일/모드2)가 source.

# 패치 키: RUNTIME_PATCH_STEM(셀 env 가 선언 · slave 로도 전달) > CONFIG_FILE — slave 는 CONFIG_FILE=default 라 model-keyed 패치를 못 찾던 침묵 누락 교정(2026-10-24 Solar-Open2)
_cfg="${RUNTIME_PATCH_STEM:-${CONFIG_FILE:-default}}"
# 둘 다 모델 값인데 갈리면 다른 셀의 패치를 무장하는 것일 수 있다(policy:RUNTIME_PATCH_NO_CARRY_FORWARD) — 막지 않고 크게 알린다(2026-10-26)
if [ -n "${RUNTIME_PATCH_STEM:-}" ] && [ -n "${CONFIG_FILE:-}" ] && [ "${CONFIG_FILE}" != "default" ] && [ "${RUNTIME_PATCH_STEM}" != "${CONFIG_FILE}" ]; then
    echo "[arm_patch] WARN: RUNTIME_PATCH_STEM=${RUNTIME_PATCH_STEM} ≠ CONFIG_FILE=${CONFIG_FILE} — 다른 셀의 패치를 무장하는지 확인하라(carry-forward 금지)" >&2
fi
_patch="/app/configs/${_cfg}_patch.py"
if [ -f "$_patch" ]; then
    _sp="$(python3 -c 'import sysconfig;print(sysconfig.get_paths()["purelib"])' 2>/dev/null || true)"
    if [ -n "$_sp" ] && [ -w "$_sp" ]; then
        printf "import importlib.util as u;s=u.spec_from_file_location('__cfgpatch','%s');m=u.module_from_spec(s);s.loader.exec_module(m)\n" \
            "$_patch" > "${_sp}/zzz_${_cfg}_patch.pth"
        echo "[arm_patch] runtime patch armed → ${_patch} (purelib ${_sp})"
    else
        echo "[arm_patch] WARN: purelib 쓰기 불가(${_sp:-unknown}) — 패치 미적용(헌법 전제: root·writable)" >&2
    fi
fi
