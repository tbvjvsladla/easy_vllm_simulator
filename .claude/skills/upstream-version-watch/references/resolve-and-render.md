# resolve & render — 결정론 해소 상세 (조건부 reference)

> spine step 2(resolve)·step 3(bump 제안)·step 4(render)의 **세부 절차·근거**. SKILL.md 는 순서와 명령만
> 들고 있고, "왜 이 값이 정본인가"는 여기서 읽는다. 정본은 언제나 **스크립트 출력**이지 이 산문이 아니다.

## 0. 입력 (`config.yaml`)

사람이 미리 작성한 `config.yaml`(영속)을 읽는다. 없으면 `config.example.yaml` 참고해 요청한다.

- `target_vllm_version`: 대상 vLLM 버전(채팅 지시로 덮어쓸 수 있음). cuda/manylinux는 미지정 — ③이 자동 도출.
- `smoke.single_node.config_name` / `smoke.multi_node.config_name`: 서빙 **트리플릿 키**.
  스모크 = `docker compose --env-file envs/.env.<config_name> --profile serve up`.
  모델 경로는 `configs/<config_name>.yaml`의 `model:`(/app/models/<벤더>/<모델>)에 있고, docker-compose가 NAS를 /app/models로 마운트.
- `ngc_probe_start`: NGC 태그 프로빙 시작 YY.MM.
- `reconciliation_cap`: 실패 재조정 상한(기본 3).

**하드 제약**: `configs/<config_name>.yaml`의 모델이 NAS 경로에 없으면 **다운로드하지 말고 중단·보고**.

## 0.5 Phase 분기 (torch 세대에 따른 빌드 경로 — Step 3 교훈)

- **Phase 1 (prebuilt, 안정)**: 대상 vLLM의 torch 핀이 **2.10대**면 prebuilt wheel 경로가 동작한다(NGC 26.01 등).
  torch < 2.11 이라 `register_opaque_type(hoist)`·C++ ABI 이슈 미발생. **검증된 routine.**
- **Phase 2 (source build, 검증됨)**: torch 핀이 **2.11+**면 NGC alpha torch와 prebuilt `_C`의 C++ ABI가 어긋나
  스모크에서 실패한다(`docs/testlog/testlog_260607_2`). → **소스 빌드 경로**(`source-build.md`)로 전환: `_C`를 NGC torch에
  맞춰 직접 컴파일하면 ABI 벽이 해소된다. 0.22.1 검증 완료(`docs/testlog/testlog_260608_1`), 산출물 `Dockerfile.source-build`.
- resolve 후 torch 핀으로 분기를 판정하고, **스모크가 최종 중재자**다(빌드 성공 ≠ 서빙).
- **aarch64 트랙 가용성**: aarch64는 prebuilt wheel `cuNNN` 커버리지가 희소하고, cu129(CUDA12.9) wheel을 CUDA13.x 베이스에서 쓰는 것은
  forward-compat 의존(brittle)이다 → 많은 경우 **wheel 트랙이 부재**해 **source-build가 사실상 1차/유일** 경로가 된다.
  **단 cu-마이너 불일치 wheel 은 사전 기각 금지(전방호환 시도-우선 따름정리)** — CUDA minor forward-compat 로
  동작하는 경우가 많으니 wheel 자산이 실재하면 설치를 *시도*하고 스모크/`classify_failure` 가 중재한다
  (`failure_patterns.yaml` 의 forward-compat "신호"(requirements-fixable) 처리·`resolve_wheel.py` 실자산-독해가 준거 패턴 —
  "brittle" = 리스크 라벨이지 불가 판정 아님). 타겟 arch/cuda는 `manifest.yaml`(scan)에서 읽는다(헌법에 박지 않음).

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
# ⑤ no-download NAS 체크 (스모크 전): 모델 부재면 비0+중단·보고. --topology 필수(산출물 통로 output/<topology>/)
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
  - **known-incompat 천장(불변식)**: regen 은 `KNOWN_INCOMPAT` 천장 테이블 적용 **후**가 정본 — 시간드리프트(upstream `>=` 가 최신으로 해소되며 깨진 고정 회귀)를 영속 차단한다(예 `fastapi<0.137.0` / vLLM #45596: 0.137 include_router 리팩터 × prometheus-instrumentator → /health 500). regen 마다 적용 override 를 **stdout·헤더에 표면화**해 게이트서 재평가/만료를 강제(전역-영속 핀의 역-드리프트 방지). `failure_patterns.yaml` 분류기가 사후 탐지하면 regen 이 예방으로 닫는다.
- ⑤NAS: `configs/<config_name>.yaml`의 `model:` 경로를 `/app/models` 마운트 하에서 확인. 부재 시 다운로드 금지·중단.
- ⑥분류: `failure_patterns.yaml`(시그니처→class)로 결정론 1차 분류. 미매칭=unknown→Model-C(`failure-recovery.md`).
- ⑦트랙 제안자(proposer): torch핀 휴리스틱(2.10→wheel / 2.11+→source) + SM arch(manifest scan)**만** 결정론, **최종은 스모크 중재**.
  `build_track.decision`·`source_build.torch_cuda_arch`를 `resolved.json`에 채운다.

## 1.5 델타 판정 (Judge — spine 2.5 · HITL 게이트 ①.5)

`from_ref → to_ref` 의 변경이 **우리** 빌드/이식/기능에 닿는지를 3축으로 결정론 판정한다.
"동일빌드 vs 다른빌드"는 단일 질문이 아니다 — 같은 델타가 축에 따라 답이 갈린다.

```bash
# 로컬 clone 이 1차 권위(없으면 compare API 폴백 · 둘 다 실패면 비-0. 추정 금지)
python3 scripts/judge_version_delta.py \
  --from-ref v<old> --to-ref v<new> \
  --generated-kst <YYMMDDHH> \                       # 벽시계 금지 — 주입만
  --repo-cache <로컬 vLLM clone> \                    # 선택(권장)
  --resolved output/<t>/resolved.json --write-resolved \
  --provenance output/<t>/build_patches_src/PROVENANCE.json   # 이식 트랙일 때만
# 회귀: --self-test · --check-fixture <fixtures/version_delta_*.json>
```

| 축 | 질문 | 답이 바꾸는 것 |
|---|---|---|
| **A. 빌드입력 동일성** | 델타가 우리 빌드 입력(ABI·deps·csrc·빌드시스템)을 건드리는가 | 가드 키 **상속** 가부(§source-build.md §3.1 출구①) |
| **B. 이식 스코프 교차** | 델타가 **우리 번들이 덮어쓰는 파일**과 겹치는가 | 이식 **재파생** 필요 여부(침묵 되돌림 위험) |
| **C. 모델 코드경로 도달성** | 델타가 대상 모델이 실제로 도는 경로의 심볼에 닿는가 | 기능 재검증의 **성격**(거동검증 vs 재현확인). **미구현** |

- **verdict 는 worst-wins**: 어느 축이든 `UNDETERMINED` → 전역 `UNDETERMINED`; 아니면 `IMPACT` 하나라도 있으면 `IMPACT`.
  `NO_IMPACT` 는 **구현된 모든 축이 완전 커버리지로 계산됐을 때만** 나온다.
- **`UNDETERMINED` 는 무영향이 아니다** — 게이팅상 `IMPACT` 와 같게 취급하되 **기록은 분리**한다.
  미판정이 무영향으로 세탁되는 것을 막는 것이 이 설계의 첫 번째 목적이다.
- **닫힌 열거 밖 경로 = `UNKNOWN_PLANE` = fail-closed.** vLLM 레이아웃이 바뀌면 **막히지, 새지 않는다**.
- **C축은 스모크를 면제하지 않는다** — 현재 `NOT_IMPLEMENTED`(전역 verdict 에서 제외)이며,
  구현되더라도 *증거의 성격*만 바꾼다. `arbiter="smoke"` 는 어느 축에서도 불변이다.
- **axis_B 가 `IMPACT` 면 `regen_build_patches_src.py derive` 재파생이 선행**한다. 정지조건은
  `silent_revert_risk == []`(위험분만 담기며, 전체 프로브는 `probes` 에 남는다 — `NO_REVERT` 도 증거다).
  ⚠ 파생 변종 번들은 **정의상 상류와 다르므로** `probes` 가 비지 않는다. 그것이 정상이며,
  `PROVENANCE.source` 의 파생-베이스 선언이 증거로 붙되 **verdict 는 fail-closed 로 유지**한다(선언 ≠ 증명).

**HITL 게이트 ①.5**(렌더 전): ⓐ `axis_A` 가 정말 ∅ 인지 ⓑ `axis_B.silent_revert_risk` 처리 계획
ⓒ `unknown[]` 이 비었는지. **`UNDETERMINED` 면 렌더 진입 금지.**

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

**사실/판단 분리**: change-summary·bump 제안표의 **사실 행**(torch핀·NGC태그·wheel URL·requirements delta·
Dockerfile ARG·**버전 델타 파일/라인수**)은 ①~④ 및 §1.5 Judge 의 스크립트 JSON 출력(`resolved.json#upstream_delta`)을 **그대로 테이블화**한다.
델타 행을 손으로 옮겨 적지 않는다 — 실제로 `+5/-1` 을 `+6/-2` 로 옮겨 적은 전사 오류가 있었고, 가능했던 이유는 그 사실을 만들어 주는 스크립트가 없어서였다. LLM은 사실을 작성하지 않는다.
LLM 몫은 그 위의 **risk-memo(리스크 해석 + 실패 분류 근거)** 뿐이며, 스크립트 JSON에 없는 버전·URL·패키지명을
risk-memo에 새로 도입하지 않는다. 불명·미매칭은 **"확인 필요"**(Model-C).

## 2.5 render — 통로 산출 (호출 순서 · 결함#2 codify)

**render 는 단일 호출이 아니라 시퀀스다** — 빠뜨리면 serve 단계서 잠복 실패(NAS 미전파). `render_dockerfile.py` 를 아래 순서로 부른다(전부 `--topology <single|multi>` · `--manifest output/<t>/manifest.yaml`):

```bash
# ① 컨테이너 정본(트랙별 Dockerfile + compose + requirements)
python3 scripts/render_dockerfile.py --template <Dockerfile[.source-build].template> --manifest … --resolved resolved.json -o output/<t>/Dockerfile[.source-build]
python3 scripts/render_dockerfile.py --template docker-compose.template.yaml          --manifest … --resolved resolved.json -o output/<t>/docker-compose.yaml
python3 scripts/regen_requirements.py --from-wheel-url <…> -o output/<t>/requirements.txt
# ② (multi 전용) 러너 스크립트 통로 materialize
python3 scripts/render_dockerfile.py --materialize-configs --topology <t>
# ③ serve-time env 통로 materialize (필수 — 누락 시 compose 가 /mnt/models 기본 마운트 → 모델 못 찾음. testlog_26062422 결함#2)
python3 scripts/render_dockerfile.py --materialize-env --topology <t>
```

- **③ `--materialize-env` 가 정본**: manifest.nas_model_path + tiktoken_cache → `output/<t>/.env`(compose 변수치환). serve 가 의존하는 `NAS_MODEL_PATH`·`TIKTOKEN_HOST_PATH` 를 manifest 에서 박는다. 헌법 `serve-time env 통로 불변식` 참조. nas_model_path 부재면 fail-loud(무증거 진행 금지).
- 단계 독립: ③의 fail-loud 가 ①(template render)를 막지 않는다(각 호출 분리). dormant single(`nodes:[]`)도 manifest 의 nas_model_path 만 있으면 ③ 성립.
