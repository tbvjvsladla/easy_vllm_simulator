# testlog 260607-3 — Phase 1: vLLM 0.18.0 앵커 prebuilt 서빙 routine 확정

## 목적
스킬 안정화 Phase 1 — 구버전(0.18.0) + prebuilt wheel로 **서빙되는 routine을 확정**. torch 2.10대라
Step 3에서 막힌 torch 2.11 이슈(hoist/ABI)를 원천 회피.

## 앵커 구성
- vLLM **0.18.0** / NGC **26.01-py3** (torch 2.10.0a0+...nv26.01, CUDA 13.0) / wheel **cu130 / manylinux_2_35**
- resolve 체인이 결정론적으로 도출(0.18.0 → torch 2.10.0 → 26.01, cu130, manylinux_2_35).

## 결과 — ✅ end-to-end 성공
1. 빌드 exit 0. 조건부 hoist 패치는 torch 2.10이라 무해(파일에 해당 라인 없음).
2. `import vllm 0.18.0` OK + **`_C`/`_custom_ops` 로드 + platform=NVIDIA GB10** ✅ (Step3의 ABI 월 없음).
3. 의존성 정합: 1차엔 `uvloop` 누락으로 실패 → **원인: regen을 requirements/*.txt 기준으로 생성해
   wheel의 `fastapi[standard]`(→uvicorn→uvloop)를 놓침.** → **wheel Requires-Dist 기준 재생성**으로 해결.
4. 서빙: `docker compose --env-file envs/.env.kanana-safeguard-8b-normal --profile serve up` →
   "Application startup complete." + API 라우트 등록.
5. 추론 스모크: `안녕 너는 누구야?` → `<SAFE>`(세이프가드 모델 정상 출력, 비어있지 않은 완성) → **합격.**

## 핵심 교훈 (스킬 개선점)
- **regen_requirements 는 `requirements/*.txt`가 아니라 wheel의 Requires-Dist(설치 후 METADATA) 기준이어야 한다.**
  requirements/common.txt 엔 서버 deps(uvicorn/uvloop)가 없고 `fastapi[standard]` 같은 extra 도 누락.
  wheel METADATA 가 권위 런타임 의존성. → `regen_requirements.py`를 wheel-Requires-Dist 모드로 보강 필요(Phase 1 후속).
- no-download 체크: `configs/<config_name>.yaml`의 model 경로를 NAS에서 확인(kanana 존재 확인 후 진행).
- 스모크는 트리플릿(`envs/.env.<config_name>`)으로 서빙 — 모델명이 아니라 config_name 입력.

## Phase 1 확정
prebuilt wheel 방식의 **서빙 routine이 0.18.0 앵커에서 검증**됨. 스킬의 resolve→패치→빌드→정합→서빙→추론
파이프라인이 작동. 이 위에서 안정화/자동화를 쌓고, Phase 2(소스 빌드, torch 2.11+)로 확장한다.

## 다음
① regen_requirements wheel-METADATA 모드 보강 ② SKILL.md/workflow에 "의존성=wheel Requires-Dist" 명시
③ Phase 2(소스 빌드 폴백) 설계.
