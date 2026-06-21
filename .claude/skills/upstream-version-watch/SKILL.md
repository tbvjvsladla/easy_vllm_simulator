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
# ⑤ no-download NAS 체크 (스모크 전): 모델 부재면 비0+중단·보고
python3 scripts/check_smoke_model.py <config_name> --repo .
# ⑥ 실패 분류 (빌드/스모크 실패 시): requirements-fixable(0)/source-build-class(1)/unknown(2)
docker logs <c> 2>&1 | python3 scripts/classify_failure.py
```

- ①torch 핀: vLLM `pyproject.toml` `[build-system].requires` 에서 추출.
- ②NGC 태그: `docker buildx imagetools inspect` 로 후보 태그의 `PYTORCH_BUILD_VERSION` 접두어를
  읽어 torch 핀과 매칭(skopeo 불필요). **접두어 매칭은 필요조건이지 충분조건 아님** — 스모크가 최종 중재자.
- ③wheel: 실제 Release 자산명에서 `cuXXX`·`manylinux_X_YY`·arch를 읽어 URL 구성(추측 금지).
- ④deps: **wheel METADATA Requires-Dist가 정본**(requirements/common.txt엔 서버 deps·extra가 없어 누락 — 예 `fastapi[standard]`→uvloop). extra는 그대로 두어 pip가 transitive 해소(Option A).
- ⑤NAS: `configs/<config_name>.yaml`의 `model:` 경로를 `/app/models` 마운트 하에서 확인. 부재 시 다운로드 금지·중단.
- ⑥분류: `failure_patterns.yaml`(시그니처→class)로 결정론 1차 분류. 미매칭=unknown→Model-C.

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
- **unknown (Model-C)**: LLM이 `{proposed_class, evidence}`를 제시 → **사람 승인 전 무행동**.
  사람이 승인하고 codify를 원하면, 에이전트가 `failure_patterns.yaml` 추가 **diff를 제안**(직접 편집 금지) → 승인 시 반영(확률론→결정론 이전).

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
   NAS체크 → (양 노드 병렬 빌드) → master+slave 기동(Ray 클러스터) → **엔드포인트 health 폴링** → master 엔드포인트 추론 → 정리.

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
> bjk110/spark_vllm_docker = 진단 힌트(벤더링 X). 검증: `docs/testlog/testlog_260608_1`. 산출물 `Dockerfile.source-build`.

**절차 (인터랙티브 → 동결):**
1. **resolve (동일)**: vLLM→torch핀→NGC 26.03(prefix 매칭). **커플링 규칙 그대로**(빌드방식만 변경). 선언 torch 충실(안정>성능).
2. **인터랙티브 컨테이너**: `docker run -d` NGC 26.03, env `TORCH_CUDA_ARCH_LIST=12.1a MAX_JOBS=N`, ccache(`PATH=/usr/lib/ccache:$PATH`), repo·NAS·ccache 마운트.
3. **빌드 루프(무제한·HITL)**: `/etc/pip/constraint.txt` 비우기 → `git clone --branch v<버전> vllm` → `python3 use_existing_torch.py`(NGC torch 사용) →
   build-system.requires **수동 설치**(`--no-build-isolation` 전제) → `pip install --no-build-isolation -e .` 컴파일 →
   실패 시 `classify_failure` → LLM 패치 제안(bjk110 힌트) → **Model-C HITL** → 소스 패치 → ccache 증분 재컴파일.
4. **서빙 스모크**: config.yaml 모델. `docker exec -d`로 serve(긴 로드 → 타임아웃·로그 안정). gpt-oss는 harmony 오프라인 인코딩 필요(아래).
5. **동결 + 재현**: 성공 레시피 → `Dockerfile.source-build`. **clean 재빌드 + 스모크 = DONE**(인터랙티브 성공만으론 부족).

**0.22.1 검증 레시피(`Dockerfile.source-build`에 동결):** NGC 26.03 → ccache → constraint 비우기 → clone v0.22.1 →
`use_existing_torch` → build-system.requires 설치 → **strip-hoist 패치**(`register_opaque_type` hoist kwarg 제거) → `pip install -e .`.

**학습 (반드시 적용):**
- **소스컴파일이 ABI 벽 해소의 핵심**: prebuilt `_C`(public torch 빌드)는 NGC alpha torch와 ABI 불일치 → 소스로 NGC torch에 맞춰 컴파일하면 undefined-symbol 없음.
- **`--no-build-isolation` → build-system.requires 수동 설치**: pip가 자동 설치 안 함. 누락 시 `ModuleNotFoundError`(예 `setuptools_rust`)=requirements-fixable. setuptools는 vLLM 핀(<81)로 조정됨.
- **strip-hoist**(torch 2.11a): `register_opaque_type(LayerName, typ="value", hoist=True)` → `hoist` 제거. NGC torch 2.11a 시그니처에 hoist 없음. 소스 패치=Model-C HITL.
- **gpt-oss harmony 오프라인**: 폐쇄망에서 vocab 다운로드 실패 → `/encodings`(o200k_base.tiktoken) 마운트 + `TIKTOKEN_ENCODINGS_BASE/RS_CACHE_DIR=/encodings`, `TIKTOKEN_ENABLED=true`(서빙 단계 docker-compose + `configs/<>.sh`가 처리).
- **긴 serve는 `docker exec -d`**(detached): foreground는 harness 2분 타임아웃에 잘림. 폴링은 짧게 나눠.
- **DONE = 스모크 + 동결 + clean 재빌드 재현**.

## 5. 금지

- 사람 승인(HITL 게이트) 없는 핀 변경·자동 push.
- 스모크 모델 자동 다운로드(NAS 부재 시 중단·보고).
- 버전·태그·매핑을 추측으로 단정(불명은 "확인 필요").

## 6. 보조 파일

- `scripts/resolve_torch_pin.py` `resolve_ngc_tag.py` `resolve_wheel.py` — ①②③ 결정론 해소.
- `scripts/regen_requirements.py` — ④ requirements 재생성(wheel METADATA 기준, `--from-wheel-url`/`--use-installed`).
- `scripts/check_smoke_model.py` — ⑤ no-download NAS 모델 실재 체크.
- `scripts/classify_failure.py` + `failure_patterns.yaml` — ⑥ 실패 결정론 분류(미지→Model-C).
- `scripts/sync_to_sub.sh` — 멀티노드 [전달]: 메인→서브 rsync(dry-run 기본/`--apply`, 체크섬, 빌딩블럭 제외).
- `scripts/multinode_serve_smoke.sh` — 멀티노드 2노드 Ray 서빙+multi-smoke 오케스트레이션(`<config> [--build] [--keep-up]`).
- `sub_node/CLAUDE.md` — 서브노드 빌드워커 CC 페르소나 정본(서브로 배포, sync 제외 보호).
- `<repo>/Dockerfile.source-build` — Phase 2 소스빌드 동결 산출물(§4.6, 0.22.1 검증). prebuilt `Dockerfile`과 별도.
- `config.example.yaml` — 입력 스키마(트리거·스모크 config_name·`reconciliation_cap`).
- `reference.md` — (필요 시 생성) torch↔NGC 매핑 테이블 폴백 + 레이어 매핑 상세.
