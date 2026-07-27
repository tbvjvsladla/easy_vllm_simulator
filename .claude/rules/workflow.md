# workflow.md — 런타임 전파 계약

> 목표: 사람의 업데이트/서빙 지시를 검증 가능한 상태 전이로 바꾸고, 실패를 정확한 owner로 돌린다.
> 항상 참인 경계는 `CLAUDE.md`; 세부 실행은 아래 owner 스킬·스크립트가 소유한다.

## 상태·owner 계약

| 상태 | owner | 입력 | 출력 | 통과 게이트 | 실패 라우팅 |
|---|---|---|---|---|---|
| init | `terraforming_node` | 인터뷰 답·노드 사실 | manifest·완수 Flag | plan HITL→스캔→attestation | 스킬 §0.5·`manifest_contract.py` |
| S1 resolve | `upstream-version-watch` | 목표 vLLM·manifest | torch/NGC/CUDA/build-track/deps 해소값 | HITL ① | 스킬 `references/resolve-and-render.md` |
| S2 patch | `upstream-version-watch` | S1 해소값·두 topology | 렌더된 이미지/serve 입력 | 양 브랜치 diff HITL ② | 스킬 §2·renderer |
| S2.5 sync | `upstream-version-watch` | multi manifest·렌더 산출물 | 검증된 메인→서브 배달 | dry-run→apply·checksum | `sync_to_sub.sh` |
| S3 smoke | upstream + recipe | 렌더 산출물·모델·HW | 기능 스모크·분류·risk memo | 아래 HITL ③ | `classify_failure.py`·owner reference |
| S4 commit | 사람 + upstream | S3 PASS 증거 | 로컬 last-good·문서·선택적 전파 | 최종 HITL ④ | `policy:LAST_GOOD_ROLLBACK_ANCHOR` |
| benchmark | `adversarial-benchmark` | 성공 recipe·manifest | lite 관측 또는 full report/certificate | full 수동 게이트 | 해당 skill references/scripts |
| docs | `wiki-desk` + publisher | plan/raw/narrative | 규약 문서·evidence record | `.claude/rules/docs.md` | publisher stable rejection |

스킬 의존순서는 `terraforming_node` → `upstream-version-watch` → `vllm-recipe-explorer` → `adversarial-benchmark`; `wiki-desk`는 discovery/failure/publication sidecar이며 상태를 전이하지 않는다.

## 공통 진입 게이트

- S1–S4·escalation·B0–B3 전 tracked terraform validator를 실행한다. 서브 진입은 `policy:A2A_DELEGATION_KEY_FAIL_CLOSED`도 검증한다. topology는 인터뷰/manifest에서만 읽고 브랜치로 추론하지 않는다.
- Flag가 없으면 작업을 멈추고 다음 문구로 onboarding을 제안한다: *"HW스캔이 덜 되어(Flag 미발행) HW 스펙(GPU·OS)을 알기 어려워 모델 `<HF URL>` 의 정확한 서빙전략을 세우기 어렵습니다. `terraforming_node` 로 ① HW스캔 + ② 모델 다운로드 전략(관리 NAS 경로? 컨테이너 임시 다운로드(컨테이너 down 시 삭제)? 특정 경로 저장·마운트?)을 먼저 정합시다."*
- fresh clone 감지는 제안만 한다. 인터뷰→승인 뒤에만 스캔한다. 정본은 `policy:TERRAFORM_FLAG_GATE`와 terraforming 스킬 §0.5다.

## 필수 전이 spine

```text
S1 resolve  → 결정론 스크립트로 vLLM/torch/NGC/CUDA/wheel-or-source/deps 해소
   verify   → 해소값 출력
   HITL 게이트 ① → 사람 확인
S2 patch    → single·multi를 같은 해소값에서 렌더; serve-time env는 manifest보다 우선
   verify   → 변경이 S1에 직결되고 topology 산출물 통로가 분리됨
   HITL 게이트 ② → 각 브랜치 diff 확인
S2.5 sync   → multi만 dry-run 후 선택적 tracked runtime·위임키·산출물 배달
S3 smoke    → 모델/NAS→RAM/host preflight→build/serve→health/inference
   verify   → 요구 topology 기능 스모크 PASS
   실패     → classify_failure.py reason에 따라 owner route
   HITL 게이트 ③ : 스모크 결과(+분류·risk-memo)를 사람이 확인 (smoke-before-commit)
S4 commit   → 스모크 통과분만 로컬 last-good 커밋
   verify   → docs/devlog·testlog에 버전·변경·스모크·anchor 기록
   HITL 게이트 ④ → 최종 커밋/전파 승인
```

- S2 runtime patch는 `validate_runtime_patch.py`→materialize/arm→S3 순서이며 `policy:RUNTIME_PATCH_NO_CARRY_FORWARD`; 모델 triplet은 `policy:MODEL_TRIPLET_NO_SUB_PROPAGATION`; variant image/serve 입력은 `policy:VARIANT_IMAGE_BUILD_VS_SERVE_PLANE`을 따른다.
- S3 모델 부재는 `policy:MODEL_ACQUISITION_TERNARY_GATE`; 모든 load/build/cleanup은 `policy:HOST_SAFETY_LAYERED_DEFENSE`; KV 산출은 `policy:KV_ABSOLUTE_CLAMP_PORTABILITY`. 실제 명령·health 판정·cleanup은 각 skill script가 소유한다.
- S4 뒤 공유 빌딩블럭 동기화는 사람 질의로만 `.claude/skills/upstream-version-watch/scripts/sync_branches.sh`; hint 후보는 `policy:HINT_TAG_ACTIVATION_GATE`와 `.claude/skills/upstream-version-watch/scripts/hint_tag.py`의 match→create→judgment→finalize→verify 순서만 허용한다.

## 실패 라우팅

| 신호 | owner/경로 | gate·출구 |
|---|---|---|
| requirements-fixable | upstream failure recovery | 조정→재빌드→S3, `reconciliation_cap` 소진 시 Model-C |
| source-build-class | upstream `references/source-build.md` | ABI 증거→HITL→동결→clean build→S3 |
| NGC base mismatch | `resolve_ngc_tag.py` + source-build owner | 후보 헤더/로그 증거→HITL→해소값 override→S3 |
| stock-구조적-불가 | upstream arch variant owner | 아래 사다리·증거·승인 후 S3 |
| multi OOM/NCCL/Ray | upstream multi reference | 증거 보존→Model-C |
| unknown | 사람 | `{proposed_class,evidence}`만 제시; 승인 전 무행동 |
| recipe 중 구조적 불가 발견 | recipe §5.5→upstream §3.6 | 공식 bump / 포크 SHA pin / 음성정직; cap 뒤 Model-C |

arch-wall은 단계를 건너뛰지 않는다: deps-패치 → 소스-게이트 패치 → **소스-repo 오버라이드(포크 핀)** → 체크포인트-교체. `stock-구조적-불가` 뒤 **참조-그라운디드 확증** → **testlog 기록 + 사람 승인** → canonical ledger `source_build_variants` → **Dockerfile build-arg 파라미터화** → **clean 빌드(양노드) → 스모크** 순서다. 정본은 `policy:ARCH_WALL_VARIANT_LADDER`, upstream skill §4.6과 `references/source-build.md`다.

## 메인↔서브 B0–B3

| 상태 | 입력→출력 | gate | 실패/owner |
|---|---|---|---|
| B0 bootstrap | 최초 배달→서브 local git | dry-run→HITL apply | `policy:SUB_GIT_LOCAL_ONLY` |
| B1 하향 | canonical render/output→서브 | dirty preflight·checksum | `policy:SUB_SYNC_DIRTY_FAIL_CLOSED`; `sync_to_sub.sh` |
| B2 상향 | 서브 docs path→`sync_staging/sub_docs`→HITL 재저작 | **상향 회수(서브→메인) = 문서기반 only** | `fetch_sub_docs.sh`; patch/code 직접 회수 금지 |
| B3 경계 | attestation+미러 docs→메인 관측 | A2A key | 메인은 서브 디스크 재스캔/직접교정 금지 |

서브 개선은 `[improve]` history에 남고 메인 저작권은 template→render→delivery로만 행사한다. egress-restricted 서브의 외부조사 실패도 docs로만 상향한다.

## 완료 조건

컨테이너 변경은 S3 PASS 전 done이 아니다. 실패는 partial apply 없이 last-good로 복구하고(`policy:LAST_GOOD_ROLLBACK_ANCHOR`), build/검증 증거는 `docs/testlog/`, 전파 서사는 `docs/devlog/`에 남긴다. bump·full benchmark·모델 다운로드는 명시된 사람 승인 없이는 실행하지 않는다.
