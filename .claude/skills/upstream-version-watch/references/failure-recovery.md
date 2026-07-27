# 실패 복구 playbook — 분류 → 행동 (조건부 reference)

> spine step 6(스모크 실패 분기)의 **행동 상세**. 진입은 언제나 결정론 분류기
> `scripts/classify_failure.py` + `failure_patterns.yaml` 이고, 이 문서는 그 class 별 처방이다.
> 절차-홈은 `.claude/rules/workflow.md` S3(HITL 게이트 포함) — 여기는 스킬-홈 관점의 세부.

## 1. requirements-fixable (exit 0)

**Loop-Until-Done** — (조정 → 재빌드 → 스모크)를 스모크 통과까지 반복한다.
단 `config.yaml` 의 `reconciliation_cap`(기본 3) 한정. **캡 소진 시 무한루프 금지 → Model-C(HITL)**.

전형: 누락 build 의존(`ModuleNotFoundError: setuptools_rust`), 상한 미적용 패키지(`KNOWN_INCOMPAT` 대상),
cu-마이너 forward-compat 신호. 재발 방지가 가능하면 `regen_requirements.py` 의 천장 테이블로 **예방 이전**한다.

## 2. source-build-class (exit 1)

prebuilt `_C` ↔ NGC torch 의 C++ ABI 벽. **propose Y/N** 으로 "소스빌드 필요 — 진행?" 을 사람에게 보고·확인하고,
승인 시 `source-build.md` 절차로 전환한다. 다른 NGC 태그로 **자동 폴백 루프를 돌지 않는다**(무증거 오버라이드 금지).

## 3. 멀티노드 서빙 실패

OOM / NCCL-RDMA / Ray join timeout 은 requirements·source-build 클래스가 **아니다** → **Model-C(HITL)**.
서브만 빌드 실패면 환경 불일치 신호 → 역시 Model-C. 진단 재료는 `multinode-build.md`.

## 4. stock-구조적-불가 (arch-wall)

대상 모델이 stock vLLM 에서 하드월로 토큰 1개 전에 죽는 경우. 처방 = **vLLM source-repo 오버라이드**
(fork **SHA 핀** `VLLM_REPO`/`VLLM_REF`) + 변종 트랙. 사다리·거버넌스는 `source-build.md` §래더,
절차 정본은 `.claude/rules/workflow.md` S3 arch-wall 분기. 무증거 오버라이드 금지.

## 5. unknown (exit 2) — Model-C 참조-그라운디드 해결

class 제안 전 **자기추론보다 권위 소스를 먼저 조회한다**(여기서의 토큰 증가는 정확도를 사므로 권장):

1. wheel METADATA(Requires-Dist)
2. NGC 이미지 라벨(`docker buildx imagetools inspect`)
3. 컨테이너 내부 torch 버전 + `torch::stable` 헤더(`tensor_struct.h`/`ops.h` 의 `layout()`/6-arg `from_blob` 존재)
4. 빌드/serve 로그 · `failure_patterns.yaml`
5. **외부 소스(메인 한정 — upstream 은 빌드평면·메인 불변)**: vLLM GitHub release/issue/PR + NGC 매트릭스
   (`.claude/skills/wiki-desk/reference/references.md` §1·§2 템플릿; egress-restricted 서브 = 증상 상향만)

그 위에 LLM 이 `{proposed_class, evidence, external_sources}` 를 제시한다 — **`external_sources` 빈 값이면
보고서에 "외부 미조회" 라벨 강제 표기**(자기추론-only 부정 결론 차단) → **사람 승인 전 무행동**.

사람이 승인하고 codify 를 원하면 에이전트가 `failure_patterns.yaml` 추가 **diff 를 제안**(직접 편집 금지) →
승인 시 반영(확률론 → 결정론 이전).
