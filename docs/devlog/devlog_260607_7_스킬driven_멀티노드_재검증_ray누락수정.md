# devlog 260607-7 — 스킬-driven 멀티노드 재검증 + ray 누락 수정

## 작업
개선된 스킬(`multinode_serve_smoke.sh` + SKILL.md §4.5)로 멀티노드를 다시 검증. 재현 빌드가 숨은
누락 의존성(ray)을 노출 → 스킬의 실패-처리 루프로 수정·codify·재통과. 상세 검증: docs/testlog/testlog_260607_8.

## 흐름 (스킬 가치 실증)
1. **스킬-driven 재테스트**: `multinode_serve_smoke.sh gpt-oss-120b-MXFP4 --build` → master 즉시 EXIT
   `ray: command not found`. (이전 수동/캐시 통과가 가렸던 **비재현 누락**을 재현빌드가 노출.)
2. **근본 원인**: regen을 wheel-METADATA 기준으로 전환(단일노드 정답) → 단일노드 불필요·**멀티노드 필수 ray**가
   누락. vLLM 0.18.0은 ray를 핵심 의존성 미선언(test.txt에 `ray==2.48.0` 핀만).
3. **분류·수정** (Model-C → Loop-Until-Done):
   - classify_failure: unknown → Model-C → 사람 승인 → requirements-fixable.
   - 수정: multi-node requirements.txt에 `ray==2.48.0` 추가 → sync_to_sub.sh로 서브 전파.
   - codify: failure_patterns.yaml에 `command not found → requirements-fixable`(확률론→결정론 이전).
4. **재통과**: 재빌드(ray 포함) → Ray 클러스터 → READY ~285s → SMOKE PASS content='4' → exit 0. ✅

## 교훈 (스킬에 반영)
- **wheel-METADATA regen은 단일노드 정답이나 멀티노드는 ray를 별도 추가**해야 함(멀티노드 requirements = wheel METADATA + ray).
  → 향후 멀티노드 patch 시 ray 포함 확인. (SKILL.md §4.5 학습에 합류 가능.)
- **재현 빌드(`--build`)의 가치**: 캐시가 가린 누락을 노출. 스킬-driven 재테스트가 수동 테스트보다 견고.
- 실패-처리 루프(분류→Model-C→수정→codify→Loop-Until-Done)가 실전에서 작동 확인.

## 상태
멀티노드 path가 스킬-driven으로 재현 가능하게 통과. 미커밋: main 0.18.0 + multi-node 0.18.0(+ray) 패치
(스모크 통과, smoke-before-push — 커밋/푸시는 사람 승인 시).
