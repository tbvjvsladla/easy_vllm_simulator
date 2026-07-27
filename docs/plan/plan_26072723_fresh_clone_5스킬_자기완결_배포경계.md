# Fresh-clone 5-Skill 자기완결 배포경계 검증·복구 Plan

- 상태: **ACTIVE — 사용자 실행 지시 수신**
- 기준 시각: 2026-07-27 23:26 KST
- 목적: 3일간의 completion/policy/topology 개선을 보존하면서 fresh clone의 public surface를 헌법 + 5개 skill 중심으로 회수한다.
- Source baseline: origin/multi-node `05dc65d28ffe184a70ecb6f2afb5d0b9d79396d0`
- Candidate baseline: local multi-node `ed08c3f719dc985ca5ba230f8f32aefa7d900c6e`

## 1. 사용자 여정 정본

```text
fresh clone
→ terraforming_node 정착
→ upstream-version-watch
→ vllm-recipe-explorer
→ adversarial-benchmark
→ wiki-desk + 헌법 loop
```

판정은 기존 920-test 자기참조 결과가 아니라 이 fresh-clone 사용자 여정이 수행 가능한지로 한다.

## 2. 핵심 질문

1. origin 기준선에서 root `tests/`와 root `scripts/`는 어떤 역할을 가졌는가?
2. 현재 accepted tree에서 root `tests/`와 root `scripts/`를 제거하면 위 사용자 여정의 어느 경계가 실제로 실패하는가?
3. 실패 원인이 runtime executable 부재인지, policy trust가 test path에 과결합된 것인지 구분한다.
4. completion/policy/topology 개선을 버리지 않고 owner skill/헌법 경로로 이전할 수 있는가?
5. primary/single/sub가 topology별 최소 tree로 materialize돼도 fresh startup과 안전 gate가 닫히는가?

## 3. 비목표·안전경계

- origin push/tag/force-push 금지
- 현재 Solar stop/recreate/reload 금지
- sudo/systemd/Docker lifecycle mutation 금지
- NAS/model bytes 수정 금지
- source workspace hard reset/clean/stash/drop 금지
- 기존 scanner patch와 cleanup patch byte-preserving 보존
- 원격 clone은 `/tmp` 격리 경로에서만 수행
- 실제 모델 적재가 필요한 단계에서는 멈추고 별도 HITL을 요청

## 4. Phase 1 — 원격 fresh-clone 기준선

1. origin/multi-node를 immutable commit `05dc65d…`로 `/tmp` clone한다.
2. README, CLAUDE, 5개 SKILL의 최초 사용자 entrypoint를 추출한다.
3. terraforming discovery/HW scan/model acquisition selection 전까지 dry-run한다.
4. runtime 3 skill의 help/dry-run/import/required-path를 검증한다.
5. wiki/헌법 loop의 query/validation entrypoint를 검증한다.
6. root tests/scripts 의존성을 command→consumer→owner graph로 기록한다.

## 5. Phase 2 — 현재 candidate mutation

Disposable clean-index export에서 별도 실행한다.

- control A: full candidate
- mutation B: root `tests/`만 제거
- mutation C: root `scripts/`만 제거
- mutation D: 둘 다 제거

각 arm에서 같은 사용자 여정 contract를 실행한다. 기존 `harness_verify.py` 실패만으로 판정하지 않고 각 public skill entrypoint와 fresh-start prerequisites를 직접 검증한다.

## 6. Phase 3 — 제3안: 5-Skill self-contained distribution

문제가 과결합으로 확인되면 다음 목표를 적용한다.

1. root `tests/`를 production evidence authority에서 제거한다.
2. clause predicate는 헌법 소유 production predicate 경로로 이전한다.
3. root `scripts/`의 각 executable을 owner skill 또는 헌법 control-plane 경로로 이전한다.
4. public user journey는 헌법 + 5개 skill만 따라가도 모든 deterministic executable을 찾는다.
5. 개발 회귀 test는 source에서 유지할 수 있으나 production startup/trust/promotion은 test path에 의존하지 않는다.
6. 배포 tree는 root `tests/`/`scripts/`가 없어도 fresh-clone contract를 통과한다.
7. 서브는 topology-specific 최소 runtime materialization을 유지하고 primary와 무조건 동일 tree로 만들지 않는다.

예상 owner mapping:

- terraforming_node: host safety, memwatch, cleanup, HW/preload/manifest
- upstream-version-watch: sync, clone smoke, rendering/build-track
- vllm-recipe-explorer: runtime readiness, functional smoke, recipe/trial
- adversarial-benchmark: benchmark/verdict/report/certificate
- wiki-desk: docs naming/evidence publication/discovery
- 헌법 control plane: policy registry, completion authorization, claim predicates, provider-neutral agent control

## 7. RED acceptance

구현 전에 다음 RED를 고정한다.

- `tests/` 없는 clean candidate에서 production policy verify가 test evidence missing으로 실패
- `scripts/` 없는 clean candidate에서 root-script pointer가 unresolved
- 5개 skill 문서가 root scripts를 가리키는 stale pointer가 존재
- sub topology에서 primary-only control plane을 요구하면 실패

## 8. GREEN acceptance

- root `tests/` 없음
- root `scripts/` 없음
- 5개 public skill 정확히 유지
- 모든 public entrypoint가 owner-local executable로 resolve
- production policy/evidence/promotion path가 test 파일을 읽지 않음
- origin baseline 사용자 여정 대비 기능 손실 0
- completion lifecycle, REFUTE, expected-negative, topology trust 보존
- primary/single/sub 각각 clean export fresh-clone contract PASS
- service continuity: main/sub running, restart 0, OOM false, health 200
- dual independent reviewer PASS/0 on same candidate tree

## 9. 동기화·커밋 정책

- 먼저 primary candidate exact tree를 검증한다.
- source gate 전 remote/sub mutation 금지
- single은 allowlist materialization 후 destination gate를 통과해야 commit 가능
- sub는 topology-specific materialized bytes/mode를 검증한 뒤에만 local sync 가능
- origin push는 최종 diff와 사용자 별도 승인 전 금지

## 10. 중단 조건

- 원격 fresh clone이 root tests/scripts 없이 이미 사용자 여정을 만족하지 못함
- owner mapping이 5개 skill public contract를 변경해야 함
- 실제 모델 reload/서비스 중단 필요
- source 또는 destination dirty work를 보존할 수 없음
- dual reviewer blocker 발생

중단 시 전체 rollback을 자동 수행하지 않는다. exact failing boundary와 최소 보존/회수 선택지를 보고한다.
