# 하네스 Rust 완전이식 · 양 토폴로지 E2E 결산 (2026-11-17)

## 요약

- 결정론 하네스(게이트·검증기·런처 뒤 실행체·셀프테스트)를 **전부 Rust 로 이식하고 추적 하네스 `.py` 160개를 삭제**했다.
  남은 추적 `.py` 는 허용목록 1건(`configs/check_reqs.py` — 컨테이너 내부 실행물)뿐이다.
- 경계: **하네스는 Rust, 개발산출물은 그 산출물의 메인 언어**다. vLLM 런타임/빌드 패치·컨테이너 내부 코드·GuideLLM 연동은
  vLLM 이 Python 이므로 Python 으로 남는다.
- 재유입 차단: `py_boundary_guard`(추적 `*.py` ⊆ 허용목록 · pre-commit 과 배포 검사에서 집행) · `inline_python_scan`
  (셸 안의 `python3 -c` 등을 컨테이너 평면 사유 없이는 거부) · 검증 중에는 `py_exec_trap`(호스트 python 실행을 가로채 기록).
- 삭제 전 라이브 증거로 아래 두 E2E 를 **호스트 python 실행 0건**으로 완주했다.

## single 토폴로지 — 4모델 동시 상주(노드당 2모델) + Full bench

| 노드 | 모델 | c1 decode | 판정 |
|---|---|---|---|
| main | Qwen3.6-27B-NVFP4 | 12.03 t/s | weak PASS |
| main | Qwen3.6-35B-A3B-NVFP4 | 76.12 t/s | weak PASS |
| sub | Muse-Glimmer-30B-NVFP4 | 12.02 t/s | weak PASS |
| sub | ZwZ2-9B (bf16) | 13.11 t/s | weak PASS |

- 엔드포인트 4개 동시 상주 · lite → GuideLLM full · 레벨 [1,2,4,8] × 반복 3 · 모델당 통합메모리 ≤55GB 몫(gmu 0.42).
- 서브는 A2A 위임으로 자기 셀 2개를 자율 수렴·측정·보고했다.

## multi 토폴로지 — TP=2 분산(양 노드 multi-node 브랜치) 2모델 순차 + Full bench

| 모델 | c1 decode | 기타 | 판정 |
|---|---|---|---|
| NVIDIA-Nemotron-3.5-Super-VL-120B-A12B-NVFP4 | 18.67 t/s | c8 총 366 tok/s · 실패 0 | weak PASS ⚠ |
| Solar-Open2-250B-Nota-NVFP4 | 16.83 t/s | 반복 밴드 0.18% · 직전 측정과 일치 | weak PASS |

- ⚠ Nemotron: 측정값이 roofline 상한을 2.5배 넘었다 — roofline 이 중첩 VL config 에서 MoE 를 감지하지 못한 결함이며,
  그 결과 이 셀의 성능 게이트는 사실상 걸리지 않았다(측정 자체는 실재 · 후속 교정 대상).
- Nemotron 첫 기동은 flashinfer JIT(컨테이너 내부 캐시) 때문에 READY 가 약 29분 걸려 선언한 상한을 넘었고, 상한을 실측에 맞춘 뒤
  정식 스모크를 다시 돌려 PASS 를 얻었다.

## 이번에 드러난 결함(후속)

- 같은 노드 2모델 상주: 레시피 생성기가 서빙 yaml 에 호스트 포트를 넣는다(컨테이너 포트여야 함) · explorer 기본 이미지가 낡음 ·
  KV 클램프 바닥값이 max_len 에 비해 작음 · 하이브리드(Mamba/linear-attention) 모델의 batch 가 블록 수를 넘음.
- multi: 생성기가 tp/ray 인자와 multi env 키를 내지 않음 · 추정기·roofline 이 VL omni 의 중첩 config/혼합 양자화를 파싱하지 못함.
- 벤치: 컨테이너 내부 bench 클라이언트가 호스트 포트를 겨누던 결함은 이번에 교정했다.

## 종료 상태

- 모든 서빙 down · 양 노드 single-node 복귀 · 양 브랜치 push(일반 push · force/rebase 없음).
