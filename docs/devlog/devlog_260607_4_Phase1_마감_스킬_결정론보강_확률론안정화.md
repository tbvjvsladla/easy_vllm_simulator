# devlog 260607-4 — Phase 1 마감: 스킬 결정론 보강 + 확률론 안정화

## 작업 요약
싱글노드 `upstream-version-watch` 스킬(prebuilt wheel, Phase 1 서빙 검증 완료)을 시드
`seed_801f0bf97156`(QA 0.91) 명세대로 다듬어 **Phase 1을 마감**한다. 결정론/확률론 경계를 명확히 하고
(스크립트=사실, LLM=판단), 4단계 스코프(경계=소스빌드 '감지'까지) 안에서 안정화.
근거: 인터뷰 `interview_20260607_075652`, 계획서 `docs/plan/plan_260607_3_*`.

## Track A — 결정론 보강 (3 스크립트, 전부 검증)

| 산출물 | 내용 | 검증 |
|---|---|---|
| `scripts/regen_requirements.py` | **wheel METADATA(Requires-Dist) 기준** 재생성(`--from-wheel-url`/`--use-installed`). requirements/*.txt 아님 | 0.18.0: 59개, `fastapi[standard]`→uvloop 포함, torch 제외 ✓ |
| `scripts/check_smoke_model.py` | configs/<config_name>.yaml `model:` → `/app/models` 마운트 하 NAS 실재 체크. 부재=비0+중단(다운로드 금지) | 존재 exit0 / 부재 exit2 STOP ✓ |
| `failure_patterns.yaml` + `scripts/classify_failure.py` | 실패 시그니처→class 결정론 분류. 미매칭=unknown→Model-C | uvloop→fixable(0)/hoist·undefined→source-build(1)/novel→unknown(2) ✓ |

- 핵심 교훈 코드화: **의존성 정본 = wheel METADATA**(서버 deps·extra가 requirements/*.txt엔 없음 — uvloop 사고).
- 분류기 시딩 근거: testlog_260607_2(0.22.1 ABI 체인), testlog_260607_3(0.18.0 uvloop).

## Track B — 확률론 안정화 (SKILL/workflow/config 규약)

- **사실/판단 분리(B1·B2)**: change-summary·layer-bump-proposal의 사실 행은 ①~④ 스크립트 JSON을 그대로 테이블화
  (LLM 사실 작성 0). LLM은 **risk-memo(리스크 해석 + 실패 분류 근거)만**. (SKILL.md §3)
- **실패 분기(B3·B4)** (SKILL.md §3.5 / workflow.md S3):
  - requirements-fixable → **Loop-Until-Done**(조정→재빌드→스모크), `reconciliation_cap`(기본 3) 한정. 소진→Model-C.
  - source-build-class → **propose Y/N**(범위 밖=Phase 2). 폴백 루프 없음.
  - unknown → **Model-C**(LLM 제안+사람 승인 전 무행동, 신규 패턴은 diff 제안→승인 후 codify).
- config: `reconciliation_cap: 3` 추가.

## 완료 기준 점검 (시드 AC)
- [x] regen=wheel METADATA (uvloop 포함 검증)
- [x] NAS 체크 스크립트 (존재/부재 exit code 검증)
- [x] failure_patterns.yaml 분류기 + unknown→Model-C (4케이스 검증)
- [x] 사실=스크립트 출력 / risk-memo만 LLM (SKILL §3 규약)
- [x] Loop-Until-Done + cap(3, config) + 소진→Model-C (SKILL §3.5 / workflow S3 / config)
- [x] source-build-class 감지·propose Y/N (분류기 + workflow 분기) — 0.22.1/26.03 재현은 testlog_260607_2
- [x] 0.18.0 앵커 E2E 검증 (testlog_260607_3)
- [x] 문서화(이 devlog + SKILL/workflow/config 갱신)

## Phase 1 마감
prebuilt wheel 기반 싱글노드 컨테이너 제작·서빙 routine이 **검증·스크립트화·규약화** 됨.
결정론(사실 수집·분류) / 확률론(리스크·미지 판단) 경계가 명확하고, Agent OS 관리에 안정적.

## 다음 (Phase 2 — 범위 밖, 별도)
소스빌드 폴백(bjk110/spark_vllm_docker 청사진: 멀티스테이지 + 패치 + TORCH_CUDA_ARCH). 
source-build-class 감지 시 propose Y/N로 진입하는 별도 스킬로 확장.
