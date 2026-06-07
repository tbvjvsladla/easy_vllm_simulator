# testlog 260607-8 — 스킬-driven 멀티노드 재테스트 (ray 누락 catch → 수정 → 통과)

## 목적
개선된 스킬(`multinode_serve_smoke.sh`)로 멀티노드를 재검증. 재현 빌드(`--build`)가 숨은 누락 의존성을 노출.

## 1차 실행 — 실패 (스킬이 버그 catch)
`multinode_serve_smoke.sh gpt-oss-120b-MXFP4 --build`
- 양 노드 빌드 OK → master 기동 → **즉시 EXIT**: `serve_runner.sh: line 40: ray: command not found`.
- 진단: NGC 26.01 베이스·requirements.txt·vLLM 0.18.0 wheel METADATA **어디에도 ray 없음**.
  이전 통과는 docker 레이어 캐시에 우연히 남은 ray 덕(**비재현**). `--build` 재현빌드가 노출.
- 근본 원인: regen을 wheel-METADATA 기준으로 바꾼 뒤(단일노드 정답) **멀티노드 필수 ray가 누락**.
  (vLLM 0.18.0은 ray를 핵심 의존성으로 선언 안 함 — requirements/test.txt에 `ray==2.48.0` 핀만.)

## 분류 / 수정 (Model-C → Loop-Until-Done)
- `classify_failure`: "ray: command not found" 미매칭 → unknown → **Model-C**.
- LLM 제안 + 사람 승인: `requirements-fixable` — 멀티노드 requirements.txt에 `ray==2.48.0` 추가.
- **codify**: `failure_patterns.yaml`에 `command not found → requirements-fixable` 패턴 추가(확률론→결정론).
  검증: 분류기가 이제 "ray: command not found" → requirements-fixable(결정론).
- 수정 전파: requirements.txt에 ray 추가 → `sync_to_sub.sh --apply`(체크섬 일치).

## 2차 실행 — 통과 (Loop-Until-Done iter 1)
`multinode_serve_smoke.sh gpt-oss-120b-MXFP4 --build` (ray 포함 재빌드)
- 양 노드 빌드 OK → master+slave Ray 클러스터 → 엔드포인트 health **READY ~285s** → **SMOKE PASS content='4' fr=stop** → exit 0. ✅

## 결론
스킬-driven 재테스트가 캐시에 가려졌던 **실제 누락 의존성(ray)을 정확히 노출**하고, 스킬의 실패-처리 루프
(분류→Model-C→수정→codify→재빌드→재스모크)로 **재현 가능**하게 해결. ray가 requirements에 명시되어
캐시 무관하게 멀티노드 빌드 재현. 멀티노드 path가 스킬-driven으로 견고히 동작함을 확인.

## 산출 변경
- `requirements.txt`(multi-node): `ray==2.48.0` 추가(멀티노드 전용, 주석 명시).
- `failure_patterns.yaml`: `command not found → requirements-fixable` 패턴 추가.
