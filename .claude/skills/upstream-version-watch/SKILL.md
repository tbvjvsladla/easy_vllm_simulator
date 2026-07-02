---
name: upstream-version-watch
description: >-
  vLLM 업스트림 신버전을 추적해 변경을 독해하고, 커스텀 Docker 컨테이너 레이어(베이스 이미지·
  핵심 wheel·주변 의존성)의 bump를 제안한다. "vllm 업데이트", "vllm 버전 올려", "신규 vllm 버전
  반영", "컨테이너 버전 bump", "NGC 베이스 맞춰줘" 같은 지시에 발동. 제안만 하며, 실제 핀 변경·
  빌드·push는 .claude/rules/workflow.md 의 전파 워크플로(HITL 게이트)를 따른다.
---

# upstream-version-watch

업스트림 vLLM을 추적해 **최신 버전 감지(사람 지시) → 변경 독해 → 레이어 bump 제안**까지 수행한다.
이 스킬은 **제안한다.** 실제 적용은 핀 정책(`CLAUDE.md`)과 HITL 게이트(`.claude/rules/workflow.md`)를 따른다.

> **§0.0 진입 전제 — 테라포밍-완수 Flag 게이트 (헌법 §테라포밍-완수 Flag 게이트 따름정리 · plan_2026063018_1)**: 컨테이너 빌드·렌더·bump(작업) 전 **Flag 확인 필수**. 미발급 시 **info-only**: vLLM GitHub 릴리즈 조회·버전해소 *설명* OK / **렌더·빌드·bump ✗** → "빌드는 HW(arch·cuda)·모델경로(manifest)를 알아야 — `terraforming_node` 먼저"로 redirect. 빌드 시 manifest `model_source`(managed|ephemeral|custom)로 compose 분기(P4). **결정론 백스톱** = 빌드/렌더 작업 *前* 에이전트가 `python3 .claude/skills/terraforming_node/scripts/manifest_contract.py --topology <t> --require-flag` 실행(workflow S3 게이트). upstream 은 main-only 빌딩블럭·다단계 빌드라 단일 작업스크립트 진입점이 분산 → recipe(replicated, main() 베이크-인)와 달리 **S3 워크플로 게이트가 정본**.

> 설계 원칙(하네스 엔지니어링): **버전 문자열 해소는 결정론적 스크립트**(`scripts/`)가, 변경 요약·
> 리스크 판단은 모델이 한다. 버전·태그를 추측(확률론)으로 단정하지 않는다.

## 0. 입력 (config.yaml)

사람이 미리 작성한 `config.yaml`(영속)을 읽는다. 없으면 `config.example.yaml` 참고해 요청한다.

- `target_vllm_version`: 대상 vLLM 버전(채팅 지시로 덮어쓸 수 있음). cuda/manylinux는 미지정 — ③이 자동 도출.
- `smoke.single_node.config_name` / `smoke.multi_node.config_name`: 서빙 **트리플릿 키**.
  스모크 = `docker compose --env-file envs/.env.<config_name> --profile serve up`.
  모델 경로는 `configs/<config_name>.yaml`의 `model:`(/app/models/<벤더>/<모델>)에 있고, docker-compose가 NAS를 /app/models로 마운트.
- `ngc_probe_start`: NGC 태그 프로빙 시작 YY.MM.

**하드 제약**: `configs/<config_name>.yaml`의 모델이 NAS 경로에 없으면 **다운로드하지 말고 중단·보고**.

## 0.5. Phase 분기 (torch 세대에 따른 빌드 경로 — Step 3 교훈)

- **Phase 1 (prebuilt, 안정)**: 대상 vLLM의 torch 핀이 **2.10대**면 prebuilt wheel 경로가 동작한다(NGC 26.01 등).
  torch < 2.11 이라 `register_opaque_type(hoist)`·C++ ABI 이슈 미발생. **현 스킬의 검증된 routine.**
- **Phase 2 (source build, 검증됨)**: torch 핀이 **2.11+**면 NGC alpha torch와 prebuilt `_C`의 C++ ABI가 어긋나
  스모크에서 실패한다(`docs/testlog/testlog_260607_2`). → **소스 빌드 경로**(`§4.6`)로 전환: `_C`를 NGC torch에
  맞춰 직접 컴파일하면 ABI 벽이 해소된다. 0.22.1 검증 완료(`docs/testlog/testlog_260608_1`), 산출물 `Dockerfile.source-build`.
- ②resolve 후 torch 핀으로 분기를 판정하고, **스모크가 최종 중재자**다(빌드 성공 ≠ 서빙).
- **aarch64 트랙 가용성**: aarch64는 prebuilt wheel `cuNNN` 커버리지가 희소하고, cu129(CUDA12.9) wheel을 CUDA13.x 베이스에서 쓰는 것은
  forward-compat 의존(brittle)이다 → 많은 경우 **wheel 트랙이 부재**해 **source-build가 사실상 1차/유일** 경로가 된다.
  타겟 arch/cuda는 `manifest.yaml`(scan)에서 읽는다(헌법에 박지 않음).

## 1. 해소 (결정론적 — `scripts/`)

대상 vLLM 버전에 대해 아래를 순서대로 실행해 사실을 수집한다(`docker`·`python3`+`packaging` 필요).

```bash
V=<target_vllm_version>; ARCH=$(uname -m)          # DGX Spark → aarch64
# ① vLLM → torch 핀
python3 scripts/resolve_torch_pin.py "$V"
# ② torch 핀 → 매칭 NGC 태그 (PYTORCH_BUILD_VERSION 접두어 매칭, 최신 우선)
python3 scripts/resolve_ngc_tag.py <torch_prefix> --start <ngc_probe_start> --arch arm64
# ③ wheel URL (GitHub Release 자산 실재 검증, 404 방지)
python3 scripts/resolve_wheel.py "$V" --arch "$ARCH"          # cuda 미지정 시 자동
# ④ requirements 재생성 — wheel METADATA(Requires-Dist) 기준 (권위 소스, requirements/*.txt 아님)
python3 scripts/regen_requirements.py --from-wheel-url <wheel URL> -o requirements.txt   # 빌드 전(호스트)
# (컨테이너 내부 정합: python3 scripts/regen_requirements.py --use-installed -o requirements.txt)
# ⑤ no-download NAS 체크 (스모크 전): 모델 부재면 비0+중단·보고. --topology 필수(산출물 통로 output/<topology>/, plan_2026062312_1)
python3 scripts/check_smoke_model.py <config_name> --topology <single|multi> --repo .
# ⑥ 실패 분류 (빌드/스모크 실패 시): requirements-fixable(0)/source-build-class(1)/unknown(2)
docker logs <c> 2>&1 | python3 scripts/classify_failure.py
# ⑦ 빌드 트랙 제안 (proposer): torch핀 휴리스틱(2.10→wheel/2.11+→source) + SM arch(manifest scan)
python3 scripts/resolve_build_track.py "$V"   # build_track.decision·source_build.torch_cuda_arch → resolved.json
```

- ①torch 핀: vLLM `pyproject.toml` `[build-system].requires` 에서 추출.
- ②NGC 태그: `docker buildx imagetools inspect` 로 후보 태그의 `PYTORCH_BUILD_VERSION` 접두어를
  읽어 torch 핀과 매칭(skopeo 불필요). **접두어 매칭은 필요조건이지 충분조건 아님** — 스모크가 최종 중재자.
- ③wheel: 실제 Release 자산명에서 `cuXXX`·`manylinux_X_YY`·arch를 읽어 URL 구성(추측 금지).
- ④deps: **wheel METADATA Requires-Dist가 정본**(requirements/common.txt엔 서버 deps·extra가 없어 누락 — 예 `fastapi[standard]`→uvloop). extra는 그대로 두어 pip가 transitive 해소(Option A).
  - **known-incompat 천장(불변식)**: regen 은 `KNOWN_INCOMPAT` 천장 테이블 적용 **후**가 정본 — 시간드리프트(upstream `>=` 가 최신으로 해소되며 깨진 고정 회귀)를 영속 차단한다(예 `fastapi<0.137.0` / vLLM #45596: 0.137 include_router 리팩터 × prometheus-instrumentator → /health 500). regen 마다 적용 override 를 **stdout·헤더에 표면화**해 S1 게이트서 재평가/만료를 강제(전역-영속 핀의 역-드리프트 방지). `failure_patterns.yaml` 분류기가 사후 탐지하면 regen 이 예방으로 닫는다.
- ⑤NAS: `configs/<config_name>.yaml`의 `model:` 경로를 `/app/models` 마운트 하에서 확인. 부재 시 다운로드 금지·중단.
- ⑥분류: `failure_patterns.yaml`(시그니처→class)로 결정론 1차 분류. 미매칭=unknown→Model-C.
- ⑦트랙 제안자(proposer): torch핀 휴리스틱(2.10→wheel / 2.11+→source) + SM arch(manifest scan)**만** 결정론, **최종은 스모크 중재**.
  `build_track.decision`·`source_build.torch_cuda_arch`를 `resolved.json`에 채운다.

## 2. 레이어 bump 매핑 (제안 표)

①~④ 결과를 모아 표로 제안한다. **Dockerfile 의 휘발성 부분은 3개 ARG로 격리**되어 있으므로
(패턴 A — `VLLM_VERSION`·`CUDA_VERSION`·`VLLM_MANYLINUX`), 사람은 vLLM 버전만 지정하고 나머지는 스크립트가 채운다:

| 레이어 / Dockerfile 키 | 현재 | 제안 | 근거 |
|---|---|---|---|
| `FROM` 베이스 이미지 | `pytorch:<현재>` | ②`matched_tag` | torch 핀 접두어 매칭 |
| `ARG VLLM_VERSION` | `<현재>` | 대상 버전 | config.yaml |
| `ARG CUDA_VERSION` | `<현재>` | ③`cuda_version` | wheel 자산 cuXXX (자동) |
| `ARG VLLM_MANYLINUX` | `<현재>` | ③`manylinux` | wheel 자산 manylinux_X_YY (자동, 휘발성 격리) |
| `requirements.txt` | 현재 핀 | ④ ADDED/CHANGED | vLLM requirements/* delta |

> 휘발성 wheel 파일명(cuXXX·manylinux)은 버전마다 미세하게 바뀐다(예 0.21.0=2_34 → 0.22.1=2_28).
> 그래서 Dockerfile에 하드코딩하지 않고 `ARG VLLM_MANYLINUX`·`ARG CUDA_VERSION`으로 격리,
> ③`resolve_wheel.py`가 GitHub 자산에서 실재 검증한 값으로 채운다. arch 는 빌드 시점 `$(uname -m)`.

## 2.5. render — 통로 산출 (호출 순서 · 결함#2 codify)

**render 는 단일 호출이 아니라 시퀀스다** — 빠뜨리면 serve 단계서 잠복 실패(NAS 미전파). `render_dockerfile.py` 를 아래 순서로 부른다(전부 `--topology <single|multi>` · `--manifest output/<t>/manifest.yaml`):

```bash
# ① 컨테이너 정본(트랙별 Dockerfile + compose + requirements)
python3 scripts/render_dockerfile.py --template <Dockerfile[.source-build].template> --manifest … --resolved resolved.json -o output/<t>/Dockerfile[.source-build]
python3 scripts/render_dockerfile.py --template docker-compose.template.yaml          --manifest … --resolved resolved.json -o output/<t>/docker-compose.yaml
python3 scripts/regen_requirements.py --from-wheel-url <…> -o output/<t>/requirements.txt
# ② (multi 전용) 러너 스크립트 통로 materialize
python3 scripts/render_dockerfile.py --materialize-configs --topology <t>
# ③ serve-time env 통로 materialize (필수 — 누락 시 compose 가 /mnt/models 기본 마운트 → 모델 못 찾음. testlog_2026062422_1 결함#2)
python3 scripts/render_dockerfile.py --materialize-env --topology <t>
```

- **③ materialize-env 가 정본**: manifest.nas_model_path + tiktoken_cache → `output/<t>/.env`(compose 변수치환). serve 가 의존하는 `NAS_MODEL_PATH`·`TIKTOKEN_HOST_PATH` 를 manifest 에서 박는다. 헌법 `serve-time env 통로 불변식` 참조. nas_model_path 부재면 fail-loud(무증거 진행 금지).
- 단계 독립: ③의 fail-loud 가 ①(template render)를 막지 않는다(각 호출 분리). dormant single(`nodes:[]`)도 manifest 의 nas_model_path 만 있으면 ③ 성립.

## 3. 산출 — 사실/판단 분리 (하네스 엔지니어링)

- **사실(결정론)**: change-summary·layer-bump-proposal의 사실 행(torch핀·NGC태그·wheel URL·requirements delta·
  Dockerfile ARG)은 ①~④ 스크립트 JSON 출력을 **그대로 테이블화**한다. LLM은 사실을 작성하지 않는다(누락·환각 불가).
- **판단(확률론) = risk-memo만**: LLM은 위 사실 위에 **리스크 해석 + 실패 분류(근거 포함)**만 더한다.
  risk-memo에 스크립트 JSON에 없는 버전·URL·패키지명을 새로 도입하지 않는다.
- 이 산출물은 HITL 게이트(S1/S2/S3)에서 사람이 소비한다. 불명·미매칭은 **"확인 필요"**(Model-C로).

## 3.5. 실패 처리 분기 (⑥분류 → 행동)

빌드/스모크 실패 시 `classify_failure.py` 결과로 분기한다:
- **requirements-fixable**: **Loop-Until-Done** — (조정→재빌드→스모크)를 스모크 통과까지 반복.
  단 `config.yaml`의 `reconciliation_cap`(기본 3) 한정. 캡 소진 시 무한루프 금지 → Model-C로.
- **source-build-class**: 범위 밖(Phase 2). **propose Y/N**으로 "소스빌드 필요 — 진행?"을 사람에게 보고·확인.
  스킬은 소스빌드를 수행하지 않고, 다른 NGC 태그로 폴백 루프도 돌지 않는다.
- **unknown (Model-C)**: **참조-그라운디드 해결** — class 제안 전 자기추론보다 **권위 소스**를 먼저 조회한다(여기서의 토큰 증가는 정확도를 사므로 권장): wheel METADATA(Requires-Dist) · NGC 이미지 라벨(`docker buildx imagetools inspect`) · 컨테이너 내부 torch 버전 + `torch::stable` 헤더(`tensor_struct.h`/`ops.h`의 `layout()`/6-arg `from_blob` 존재) · 빌드/serve 로그 · `failure_patterns.yaml` · **외부 소스(메인 한정)** — vLLM GitHub release/issue/PR + NGC 매트릭스(`.claude/rules/references.md` §1·§2 템플릿; 서브 에어갭 = 증상 상향만). 그 위에 LLM이 `{proposed_class, evidence, external_sources}`를 제시(**`external_sources` 빈 값이면 보고서에 "외부 미조회" 라벨 강제 표기** — 자기추론-only 부정 결론 차단) → **사람 승인 전 무행동**.
  사람이 승인하고 codify를 원하면, 에이전트가 `failure_patterns.yaml` 추가 **diff를 제안**(직접 편집 금지) → 승인 시 반영(확률론→결정론 이전). (참조-그라운디드 해결 = 헌법 "버전 문자열 해소 확률론 금지"의 error-recovery 연장.)

## 3.6. escalation 수신 — recipe 핸드오프 → 버전핀 소유·3출구 (발견≠소유의 버전-bump 축)

> `vllm-recipe-explorer` §5.5가 "현 vLLM 불가"를 외부 교차검증으로 **발견**하고 사용자 승인을 거쳐 넘긴 핸드오프의 **수신점**. recipe는 발견·핸드오프까지, **버전핀 소유·처방·rebuild는 본 스킬**(발견≠소유 — §4.7 intake 의 *버전-bump 축 형제*; §4.7=lib-축 *진입*이나 그 사다리 상단은 §3.6(ii)와 동일 fork-pin으로 수렴 — 처방 머신리 공유). 헌법 §escalation 역루프 따름정리 · `plan_2026063009_2` · 절차-홈 `workflow.md` §escalation 역루프.

- **진입(승인 완료 전제)**: recipe가 첨부한 증거(오프라인 증상 + 외부 확증: HF 모델카드·vLLM GitHub issue/release/PR) 수신 → **버전해소 리서치**(release 노트·머지 PR·포크 — §1 GitHub 추적 근육 재사용) → **3출구 판정**. 무승인/무증거 수신 ✗(트리거 정책·무증거 오버라이드 금지).
- **3출구 → 기존 경로 매핑**:
  - **(i) 공식 bump** — 모델이 더 새 *공식* vLLM release에서 지원 → **표준 bump 경로**(`workflow.md` S1–S3, HITL 게이트). 가장 단순한 출구.
  - **(ii) 커스텀/포크핀** — 모델카드가 포크·미머지 PR 지목(예 jasl/vllm PR) → **§4.6 source-repo 오버라이드**(fork **SHA 핀** `VLLM_REPO`/`VLLM_REF` build-arg) + `…-source-<변종>` superset 변종 트랙(`resolved.json` `source_build_variants`). 거버넌스 = 아치-enablement 변종 트랙 따름정리(클러스터-와이드 이미지·**기존모델 회귀 재스모크**·단일 변종-트랙·무증거 오버라이드 금지). 절차 정본 = `workflow.md` S3 arch-wall 분기.
  - **(iii) 음성정직** — vLLM이 아직 미지원(공식·포크 모두 부재), transformers-only → *"현재 vLLM으로 서빙 불가"* 보고(없는 길 날조 ✗). 사용자가 transformers 폴백/대기를 결정. **단 "공식·포크 모두 부재" 선언은 `.claude/rules/references.md` §5 최소범위 레시피 수행 + testlog "탐색 증거"(검색어·URL·일자) 기록 후에만 허용** — 가장 강한 부정 결론엔 가장 강한 증거("찾을 수 있는 길을 덜 찾고 포기" 방어 · plan_2026070208_1; 3출구 중 (i)/(ii)는 증거 요건이 이미 강한데 (iii)만 없던 비대칭 해소).
- **최종 중재 = 스모크**(린트·이슈글 ≠ 서빙됨): (i)/(ii) 출구는 render+build+S3 스모크 통과가 done. **순환 차단** — rebuild 후도 미구동이면 `config.yaml`의 `reconciliation_cap` 한정 재진입 → 소진 시 Model-C(무한 bump ✗).
- **완료 후 recipe 재개 신호**: rebuild된 이미지로 `vllm-recipe-explorer`가 전략수립(§2–§6) 재진입. 핀 변경·push는 §4·`workflow.md` HITL 게이트.

## 4. 적용

- 핀 변경·빌드·스모크·push는 핀 정책(`CLAUDE.md`)과 `.claude/rules/workflow.md`의 전파 4단계
  (S1 resolve → S2 patch → S3 smoke → S4 push) 및 단계별 HITL 게이트를 따른다.

## 4.5. 멀티노드 경로 (검증됨 — docs/testlog/testlog_260607_7)

> `multi-node` 브랜치 = 2노드 분산 서빙(master 노드 = Ray head+serve / sub 노드 = Ray worker,
> ConnectX 등 고속 인터커넥트 — 실제 주소는 `manifest.yaml` `nodes[]` 참조). `config.yaml`의 `smoke.multi_node.config_name` 사용.
> 핵심: **메인에서 코드 완성 → 서브로 직접 전달·동기화 → 양 노드 빌드 → 2노드 Ray 서빙**.

**절차(검증된 순서):**
1. **resolve**: 단일노드와 동일(①~④ 스크립트). 버전·torch핀·NGC태그·wheel·deps.
2. **patch (multi-node 브랜치)**: Dockerfile **버전 라인만** 갱신(FROM 태그·VLLM_VERSION·CUDA·manylinux).
   **보존**: 네트워크 디버그 apt(iproute2/netcat)·`configs/serve_runner.sh`(Ray master/slave)·NCCL/RDMA env·/dev/infiniband.
   requirements.txt 갱신. `.gitignore`에 빌딩블럭(CLAUDE.md/seed/) 제외 정렬(아래 학습).
3. **sync to sub**: `scripts/sync_to_sub.sh`(기본 dry-run → `--apply`). 검증된 rsync, 체크섬 검증,
   `.git/.claude/seed/docs/__pycache__/CLAUDE.md` 제외(빌딩블럭·서브 페르소나 보호).
4. **multi-smoke**: `scripts/multinode_serve_smoke.sh <config_name> [--build] [--keep-up]`.
   NAS체크 → (양 노드 병렬 빌드) → master+slave 기동(Ray 클러스터) → **엔드포인트 health 폴링** → master 엔드포인트 추론 → 정리. **변종이미지 시 build-plane ≠ serve-plane**: 이미지 정체(`IMAGE_TAG`·`VLLM_REPO`/`VLLM_REF` build-arg)는 클러스터-wide **Band2** 라 슬레이브 compose 보간에도 forward(슬레이브가 변종 이미지를 직접 빌드+기동) · 모델 serve config(`CONFIG_FILE`·트리플렛)는 **Band3** 라 슬레이브 미forward → `multinode_serve_smoke.sh` 는 슬레이브에 `IMAGE_TAG`/`VLLM_REPO`/`VLLM_REF` 만 forward(`CONFIG_FILE` ✗). ∴ 슬레이브 Band2-only = **serve-plane 불변식**(build-plane 아님). 헌법 "변종이미지 build-plane ≠ serve-plane 따름정리" · `workflow.md` S2.5.

**학습 (반드시 적용):**
- **빌딩블럭은 gitignored → 브랜치 전환에도 persist**(워킹디렉토리 단일 사본). cross-branch 동기화·cherry-pick **불필요**. 단 multi-node `.gitignore`도 `.claude/`+`CLAUDE.md`+`seed/` 제외하도록 정렬(실수 추적 방지).
- **준비 판정 = 엔드포인트 `:PORT/health` http 200**. master 로그의 "Application startup complete"는 조기 컴포넌트에서도 떠 **거짓양성**(로그 grep 금지).
- **reasoning 모델(gpt-oss 등)**: 스모크 `max_tokens` 충분히(content는 `finish_reason=stop` 도달 후). content 또는 reasoning 비어있지 않으면 통과.
- **서브 빌드는 직접 SSH 백그라운드**(긴 빌드는 `claude -p` Bash 타임아웃 위험). 서브 CC 호출 시 `ssh sub 'bash -lc "claude -p ..."'`(login shell PATH).
- master만 API 노출 → **프로브는 master 엔드포인트만**. slave는 worker(API 없음).
- 멀티노드 실패(OOM/NCCL-RDMA/Ray join timeout)는 requirements/source-build 클래스가 아님 → **Model-C(HITL)**.
- **빌드 교차검증**: 서브 독립 빌드가 메인과 동일 동작(서브만 실패면 환경 불일치 신호 → Model-C).
- **멀티노드 deps = wheel METADATA + ray**: vLLM은 ray를 핵심 Requires-Dist로 선언하지 않음(단일노드 불필요).
  멀티노드 분산(serve_runner의 `ray start`)엔 **ray 필수** → multi-node requirements.txt에 명시 추가(0.18.0 핀 `ray==2.48.0`,
  vLLM `requirements/test.txt` 기준). 캐시가 가리면 비재현 → `--build` 재현빌드로 확인(testlog_260607_8).

## 4.6. 소스빌드 경로 (Phase 2 — 검증됨, vLLM 0.22.1)

> torch 2.11+ 구간(prebuilt ABI 벽). **`_C`를 NGC torch에 맞춰 직접 컴파일** → ABI 벽 해소.
> **경험적·판단계층·임시 가교** 성격: 패치는 결정론 카탈로그에 codify하지 않는다(소스빌드는 곧 prebuilt가 따라잡음).
> 런타임 패치(스킬 `vllm-recipe-explorer` §5 · plan_2026062711_1 Part1)와 **거버넌스 공유**(판단계층·참조-그라운디드·사전-codify 금지·HITL) — 단 *빌드타임*이라 추적 `Dockerfile.source-build`에 **동결**(런타임 패치는 휘발/비추적). 평면별 지속성 비대칭 = 레이어드 적응 메타원칙 산물(plan_2026062711_1 Part 3).
> bjk110/spark_vllm_docker = 진단 힌트(벤더링 X). 검증: `docs/testlog/testlog_260608_1`. 산출물 `Dockerfile.source-build`.

**절차 (인터랙티브 → 동결):**
1. **resolve (동일)**: vLLM→torch핀→NGC 26.03(prefix 매칭). prefix-매칭은 **1차 후보**일 뿐 — 소스빌드 패치/베이스 유효성의 변별자는
   **(NGC 베이스 / 실-링크 torch) × vLLM source version**이지 pyproject torch핀이 아니다(use_existing_torch가 pyproject 핀을 버리고 NGC torch를 링크하므로).
   같은 torch핀이라도 vLLM source가 stable-ABI(`_C_stable_libtorch`: `torch::stable` layout()/6-arg from_blob)를 요구하면 prefix-매칭 alpha 베이스에
   심볼이 없을 수 있다 → **더 새 NGC 베이스 승격**. 오버라이드 전 **참조-그라운디드 해결**: 후보 NGC 베이스의 `torch::stable` 헤더(`tensor_struct.h`/`ops.h`)를
   grep해 결여 심볼(`layout()`/6-arg `from_blob`)이 **그 후보 베이스엔 존재함**을 사전 증명한 뒤에만 승격(무증거 오버라이드 금지). 절차 정본·HITL 레이어 = **`workflow.md` S3 Model-C NGC 오버라이드**(repo-축 형제 = **vLLM source-repo 오버라이드** = fork SHA 핀 `VLLM_REPO`/`VLLM_REF` build-arg — arch-wall 로 stock vLLM 이 모델에 **구조적 불가**일 때 동일 거버넌스의 1급 핀-오버라이드 → `resolved.json` `source_build_variants` · `…-source-sm12x` superset 변종 트랙(전 모델 유지·모델-키잉 ✗). 헌법 "아치-enablement 변종 트랙 따름정리"). NGC torch ↔ PyPI torch 의존성 충돌은
   `use_existing_torch.py`로 pyproject의 torch류 라인을 비활성화해 NGC torch를 그대로 링크(설치 순서/충돌 해소). 선언 torch 충실(안정>성능).
2. **인터랙티브 컨테이너**: `docker run -d` NGC 26.03, env `TORCH_CUDA_ARCH_LIST=12.1a MAX_JOBS=N`, ccache(`PATH=/usr/lib/ccache:$PATH`), repo·NAS·ccache 마운트.
3. **빌드 루프(무제한·HITL)**: `/etc/pip/constraint.txt` 비우기 → `git clone --branch v<버전> vllm` → `python3 use_existing_torch.py`(NGC torch 사용) →
   build-system.requires **수동 설치**(`--no-build-isolation` 전제) → `pip install --no-build-isolation -e .` 컴파일 →
   실패 시 `classify_failure` → LLM 패치 제안(bjk110 힌트) → **Model-C HITL** → 소스 패치 → ccache 증분 재컴파일.
4. **서빙 스모크**: config.yaml 모델. `docker exec -d`로 serve(긴 로드 → 타임아웃·로그 안정). gpt-oss는 harmony 오프라인 인코딩 필요(아래).
5. **동결 + 재현**: 성공 레시피 → `Dockerfile.source-build`. **clean 재빌드 + 스모크 = DONE**(인터랙티브 성공만으론 부족).
   **빌드검증 불변식**: 빌드스테이지 검증은 `import vllm._C` 금지(빌드스테이지엔 `libcuda.so.1` 드라이버 부재 → 거짓실패) → `importlib.util.find_spec('vllm')`만 사용.
   실 `_C` 로드/서빙은 **런타임 스모크가 최종 중재**. (이미 레포 루트 `Dockerfile.source-build.template:78`에 반영됨 — 집=루트 템플릿; `.claude/skills/`엔 없음.)

**0.22.1 검증 레시피(`Dockerfile.source-build`에 동결):** NGC 26.03 → ccache → constraint 비우기 → clone v0.22.1 →
`use_existing_torch` → build-system.requires 설치 → **strip-hoist 패치**(`register_opaque_type` hoist kwarg 제거) → `pip install -e .`.

**학습 (반드시 적용):**
- **소스컴파일이 ABI 벽 해소의 핵심**: prebuilt `_C`(public torch 빌드)는 NGC alpha torch와 ABI 불일치 → 소스로 NGC torch에 맞춰 컴파일하면 undefined-symbol 없음.
- **`--no-build-isolation` → build-system.requires 수동 설치**: pip가 자동 설치 안 함. 누락 시 `ModuleNotFoundError`(예 `setuptools_rust`)=requirements-fixable. setuptools는 vLLM 핀(<81)로 조정됨.
- **strip-hoist**(torch 2.11a): `register_opaque_type(LayerName, typ="value", hoist=True)` → `hoist` 제거. NGC torch 2.11a 시그니처에 hoist 없음. 소스 패치=Model-C HITL.
- **gpt-oss harmony 오프라인**: 폐쇄망에서 vocab 다운로드 실패 → `/encodings`(o200k_base.tiktoken) 마운트 + `TIKTOKEN_ENCODINGS_BASE/RS_CACHE_DIR=/encodings`, `TIKTOKEN_ENABLED=true`(서빙 단계 docker-compose + `configs/<>.sh`가 처리).
- **긴 serve는 `docker exec -d`**(detached): foreground는 harness 2분 타임아웃에 잘림. 폴링은 짧게 나눠.
- **DONE = 스모크 + 동결 + clean 재빌드 재현**.

**패치 검증 → 동결(재현성) 라이프사이클 (발견 → 검증 → Dockerfile 동결):** *(이미지 clean-재빌드 재현 위한 동결이지 카탈로그 '졸업'이 아님 — plan_2026062711_1 Part 3)*
- **발견**: 신규 ABI 시그니처 충돌은 **HITL 판단계층 패치**(Model-C)다 — 사전-bake 금지(투기적 패치 금지).
- **검증**: 특정 키에서 실제 스모크 PASS로 입증된 패치만 다음 단계로.
- **재현성 동결(조건부 임베드)**: 검증된 패치를 **(NGC베이스 / 실-링크 torch) × 에러시그니처 × vLLM버전**으로 키잉한 조건부 패치로 `*.source-build.template`에 임베드 + **post-assert(fail-loud)** — *별도 카탈로그가 아니라 이미지 clean-재빌드 재현을 위한 동결*(판단계층 patch-body 는 사전-codify 금지 유지). 미인식 키 → **HITL-discovery 플레이스홀더 + 명시적 빌드 실패**(조용한 통과 금지). 키는 **pyproject torch핀이 아님**(step1 C2 동일 근거 — use_existing_torch가 핀을 버림).
- **role화 보류**: `source_build_patches.yaml` + patch-resolver 페르소나로의 역할 분리는 **E2E testlog 존재 후**에 한다(투기적 설계 금지).
  - **파일추출 보류 불변식**: `VALIDATED_SOURCE_BUILD_KEYS` 2키 frozen-set + fail-loud 가드가 현재 충분 — 별도 `source_build_patches.yaml`+resolver 는 오버엔지니어링(Karpathy B2/B3). **추출 트리거 = 인라인 셋 비대화(3번째+ 키)** 또는 패치-바디 다양화. patch-body 는 판단계층 유지(사전-codify 금지 — formula 위험과 동류). 근거 E2E(날짜 박힌 게이트 판정 서사) = devlog/testlog 인용: `testlog_2026062217_1`(0.23.0 source 26.05) · `testlog_2026062422_1`(듀얼모델 E2E 26.05 재검증).
- strip-hoist가 torch 2.12에서 자동 skip된 것은 **조건부 패치의 재사용 가능 패턴**이다(부재감지 = 적용여부 자동결정).

## 4.7. 빌드-바깥 의존 패치 (모델구동 빌드타임 — `build_patches/`)

> §4.6과 **다른 범주**: §4.6 = vLLM **빌드 자체**의 ABI 수정(inline·torch/NGC-keyed). §4.7 = **모델이 요구하는 native 의존**(lib/커널) 추가 — 예 **DeepGEMM**(DeepSeek-V4 DSA `SparseAttnIndexer` 가 요구, 미설치 시 하드 RuntimeError). per-model 3+1+1 의 "빌드-바깥 패치" 슬롯(plan_2026062812_1). 빌드평면·동결·재현·HITL 거버넌스는 §4.6과 공유. **도커 패치 범위 LADDER 의 최하단**(deps-patch=build_patches → source-gate-patch(sed) → **vLLM source-repo 오버라이드**(fork SHA 핀 `VLLM_REPO`/`VLLM_REF`, §4.6 repo-축 형제) → checkpoint-swap): build_patches 로도 stock vLLM 이 **구조적 불가**(arch-wall, 예 GB10 sm_121 DeepSeek-V4)면 위 사다리로 에스컬레이션(fork 핀 = 1급 HITL 오버라이드, 절차 정본 = `workflow.md` S3). 헌법 "아치-enablement 변종 트랙 따름정리".

- **발견 ≠ 소유 (intake)**: 발견은 `vllm-recipe-explorer` crosscheck(special-dep 경보) — **메인**이면 직접 핸드오프, **서브**면 docs insight 상향(D12, 서브는 빌드평면 미보유). upstream-version-watch 가 **이미지에 넣는 책임**(어떻게)을 진다. **patch.py ✗**(native lib 은 Python 몽키패치 불가).
- **모듈화 (Dockerfile bloat 차단)**: 패치 = **`output/<topology>/build_patches/<NN>-<name>.sh`** 모듈(추적 빌딩블럭 — .gitignore output 예외 · **빌드 컨텍스트=output/<t>/**(compose build.context `.` = compose 파일 위치 기준) · **통로 격리로 single/multi 혼재 차단**(산출물 통로 불변식) · 서브 전달=`sync_to_sub`(output/<t>/ native)). 각자 self-contained = 헤더(what/why/model-trigger/plan-ref) + 설치·컴파일 + **검증(fail-loud)**. `Dockerfile.source-build` 는 **단일 thin 스탠자**: `COPY build_patches/ /tmp/build_patches/`(컨텍스트=output/<t>/ 상대) + `RUN for p in $(ls /tmp/build_patches/*.sh|sort); do bash "$p"||exit 1; done`. → **패치 추가 = 파일 drop(Dockerfile 무수정)** · 폴더 listing = self-documenting 레지스트리(카탈로그 ✗).
- **절차 (probe → 모듈 → 동결)**: ① probe(인터랙티브 컨테이너서 설치·컴파일·작동확인 — sm arch 지원 포함) → ② `build_patches/<NN>-<name>.sh` 저작 → ③ clean 재빌드 + 서빙 스모크 = **DONE**(인터랙티브만으론 부족, §4.6 동일). 중간삽입 필요한 드문 케이스만 `Dockerfile.source-build` inline-marker(`# build-patch:<name> START/END`) fallback.
- **이미지 네이밍 불변식 보존**: 빌드-바깥 lib 은 범용(flashinfer 처럼) — DSA 안 쓰는 모델은 무시. 모델-키잉 이미지 ✗. 첫 사례 = `10-deepgemm.sh`(DeepSeek-V4-Flash).

## 5. 금지

- 사람 승인(HITL 게이트) 없는 핀 변경·자동 push.
- 스모크 모델 자동 다운로드(NAS 부재 시 중단·보고).
- 버전·태그·매핑을 추측으로 단정(불명은 "확인 필요").

## 6. 보조 파일

- `scripts/resolve_torch_pin.py` `resolve_ngc_tag.py` `resolve_wheel.py` — ①②③ 결정론 해소.
- `scripts/regen_requirements.py` — ④ requirements 재생성(wheel METADATA 기준, `--from-wheel-url`/`--use-installed`).
- `scripts/check_smoke_model.py` — ⑤ no-download NAS 모델 실재 체크.
- `scripts/classify_failure.py` + `failure_patterns.yaml` — ⑥ 실패 결정론 분류(미지→Model-C).
- `scripts/resolve_build_track.py` — ⑦ 트랙 제안자(torch핀 휴리스틱 + SM arch만 결정론, 최종은 스모크 중재). `build_track.decision`·`source_build.torch_cuda_arch`를 `resolved.json`에 채움.
- `scripts/sync_to_sub.sh` — 멀티노드 [전달]: 메인→서브 rsync(dry-run 기본/`--apply`, 체크섬, 빌딩블럭 제외).
- `scripts/multinode_serve_smoke.sh` — 멀티노드 2노드 Ray 서빙+multi-smoke 오케스트레이션(`<config> [--build] [--keep-up]`).
- (서브노드 빌드워커 CC 페르소나·Agent_Card·통신프로토콜은 **`terraforming_node` 스킬이 소유·렌더** — plan_2026062408_1 에서 `sub_node/` 이전. 이 스킬은 `sync_to_sub.sh`(전달)·`multinode_serve_smoke.sh`(서빙 스모크) 제어평면만 보유.)
- `<repo>/Dockerfile.source-build` — Phase 2 소스빌드 동결 산출물(§4.6, 0.22.1 검증). prebuilt `Dockerfile`과 별도.
- `config.example.yaml` — 입력 스키마(트리거·스모크 config_name·`reconciliation_cap`).
- 외부 레퍼런스 = **`.claude/rules/references.md`(추적 레지스트리 — plan_2026070208_1 로 승격·시딩됨)**: ID→URL 정규화 템플릿(release·PR/issue·compare API)·스크립트-소유 포인터·HW-스코프·부정판정 최소범위 레시피. 외부검색 전 1차 조회(warm-start) → 미스 시 신규 검색 → load-bearing 히트 재입고(자기증식). (구 `reference.md` 온디맨드-폴백 예약은 이 레지스트리로 대체 — torch↔NGC 매핑 정본은 여전히 결정론 스크립트 ①②③.)
