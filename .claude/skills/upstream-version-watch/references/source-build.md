# 소스빌드 경로 + 빌드-바깥 의존 패치 (조건부 reference · Phase 2 검증됨, vLLM 0.22.1)

> spine step 6 의 `source-build-class` 분기와 step 3 의 변종-트랙 결정이 여기로 온다.
> torch 2.11+ 구간(prebuilt ABI 벽)에서 **`_C` 를 NGC torch 에 맞춰 직접 컴파일** → ABI 벽 해소.
> **경험적·판단계층·임시 가교** 성격: 패치는 결정론 카탈로그에 codify 하지 않는다(소스빌드는 곧 prebuilt가 따라잡음).
> 런타임 패치(`vllm-recipe-explorer` · plan_26062711 Part1)와 **거버넌스 공유**(판단계층·참조-그라운디드·
> 사전-codify 금지·HITL) — 단 *빌드타임*이라 추적 `Dockerfile.source-build` 에 **동결**(런타임 패치는 휘발/비추적).
> bjk110/spark_vllm_docker = 진단 힌트(벤더링 X). 검증: `docs/testlog/testlog_260608_1`.

## 1. 절차 (인터랙티브 → 동결)

1. **resolve (동일)**: vLLM→torch핀→NGC 26.03(prefix 매칭). prefix-매칭은 **1차 후보**일 뿐 — 소스빌드 패치/베이스 유효성의 변별자는
   **(NGC 베이스 / 실-링크 torch) × vLLM source version** 이지 pyproject torch핀이 아니다(use_existing_torch 가 pyproject 핀을 버리고 NGC torch 를 링크하므로).
   같은 torch핀이라도 vLLM source 가 stable-ABI(`_C_stable_libtorch`: `torch::stable` layout()/6-arg from_blob)를 요구하면 prefix-매칭 alpha 베이스에
   심볼이 없을 수 있다 → **더 새 NGC 베이스 승격**. 오버라이드 전 **참조-그라운디드 해결**: 후보 NGC 베이스의 `torch::stable` 헤더(`tensor_struct.h`/`ops.h`)를
   grep 해 결여 심볼(`layout()`/6-arg `from_blob`)이 **그 후보 베이스엔 존재함**을 사전 증명한 뒤에만 승격(무증거 오버라이드 금지).
   절차 정본·HITL 레이어 = `.claude/rules/workflow.md` S3 Model-C NGC 오버라이드. NGC torch ↔ PyPI torch 의존성 충돌은
   `use_existing_torch.py` 로 pyproject 의 torch류 라인을 비활성화해 NGC torch 를 그대로 링크(설치 순서/충돌 해소). 선언 torch 충실(안정>성능).
2. **인터랙티브 컨테이너**: `docker run -d` NGC 26.03, env `TORCH_CUDA_ARCH_LIST=12.1a MAX_JOBS=N`, ccache(`PATH=/usr/lib/ccache:$PATH`), repo·NAS·ccache 마운트.
3. **빌드 루프(무제한·HITL)**: `/etc/pip/constraint.txt` 비우기 → `git clone --branch v<버전> vllm` → `python3 use_existing_torch.py`(NGC torch 사용) →
   build-system.requires **수동 설치**(`--no-build-isolation` 전제) → `pip install --no-build-isolation -e .` 컴파일 →
   실패 시 `classify_failure` → LLM 패치 제안(bjk110 힌트) → **Model-C HITL** → 소스 패치 → ccache 증분 재컴파일.
4. **서빙 스모크**: config.yaml 모델. `docker exec -d` 로 serve(긴 로드 → 타임아웃·로그 안정). gpt-oss 는 harmony 사전적재(런타임-fetch 미의존) 인코딩 필요.
5. **동결 + 재현**: 성공 레시피 → `Dockerfile.source-build`. **clean 재빌드 + 스모크 + 빌드 원장 실재 = DONE**
   (인터랙티브 성공만으론 부족 · 원장 = 이미지 안 `/opt/easy-vllm/build_ledger.json`, §4.1).
   **빌드검증 불변식**: 빌드스테이지 검증은 `import vllm._C` 금지(빌드스테이지엔 `libcuda.so.1` 드라이버 부재 → 거짓실패) →
   `importlib.util.find_spec('vllm')` 만 사용. 실 `_C` 로드/서빙은 **런타임 스모크가 최종 중재**.

**0.22.1 검증 레시피(`Dockerfile.source-build` 에 동결):** NGC 26.03 → ccache → constraint 비우기 → clone v0.22.1 →
`use_existing_torch` → build-system.requires 설치 → **strip-hoist 패치**(`register_opaque_type` hoist kwarg 제거) → `pip install -e .`.

## 2. 학습 (반드시 적용)

- **소스컴파일이 ABI 벽 해소의 핵심**: prebuilt `_C`(public torch 빌드)는 NGC alpha torch 와 ABI 불일치 → 소스로 NGC torch 에 맞춰 컴파일하면 undefined-symbol 없음.
- **`--no-build-isolation` → build-system.requires 수동 설치**: pip 가 자동 설치 안 함. 누락 시 `ModuleNotFoundError`(예 `setuptools_rust`)=requirements-fixable. setuptools 는 vLLM 핀(<81)로 조정됨.
- **strip-hoist**(torch 2.11a): `register_opaque_type(LayerName, typ="value", hoist=True)` → `hoist` 제거. NGC torch 2.11a 시그니처에 hoist 없음. 소스 패치=Model-C HITL.
- **gpt-oss harmony 사전적재**: 컨테이너가 런타임에 vocab 을 fetch 하지 않도록(read-only 마운트 불변식) → `/encodings`(o200k_base.tiktoken) 마운트 + `TIKTOKEN_ENCODINGS_BASE/RS_CACHE_DIR=/encodings`, `TIKTOKEN_ENABLED=true`.
- **긴 serve 는 `docker exec -d`**(detached): foreground 는 harness 타임아웃에 잘림. 폴링은 짧게 나눠.
- **DONE = 스모크 + 동결 + clean 재빌드 재현**.

## 3. 패치 검증 → 동결 라이프사이클 (발견 → 검증 → Dockerfile 동결)

*(이미지 clean-재빌드 재현을 위한 동결이지 카탈로그 '졸업'이 아님 — plan_26062711 Part 3)*

- **발견**: 신규 ABI 시그니처 충돌은 **HITL 판단계층 패치**(Model-C)다 — 사전-bake 금지(투기적 패치 금지).
- **검증**: 특정 키에서 실제 스모크 PASS 로 입증된 패치만 다음 단계로.
- **재현성 동결(조건부 임베드)**: 검증된 패치를 **(NGC베이스 / 실-링크 torch) × 에러시그니처 × vLLM버전** 으로 키잉한 조건부 패치로
  `*.source-build.template` 에 임베드 + **post-assert(fail-loud)**. 미인식 키 → **HITL-discovery 플레이스홀더 + 명시적 빌드 실패**
  (조용한 통과 금지 — 단 이 실패는 *미검증*이지 *불가 판정* 아님). **시도-빌드 우회(전방호환 시도-우선)**: 사람 승인 시
  `render_dockerfile.py --allow-unvalidated` 로 가드를 WARN 강등해 그대로 시도-빌드 → 스모크 중재 → **통과 시 그 (NGC×vLLM) 키를
  `VALIDATED_SOURCE_BUILD_KEYS` 에 codify**(+testlog 기록 의무 — 무기록 우회 금지). 키는 **pyproject torch핀이 아님**.
- **role화·파일추출 보류 불변식**: `VALIDATED_SOURCE_BUILD_KEYS` 2키 frozen-set + fail-loud 가드가 현재 충분 — 별도
  `source_build_patches.yaml`+resolver 는 오버엔지니어링(Karpathy B2/B3). **추출 트리거 = 인라인 셋 비대화(3번째+ 키)** 또는
  패치-바디 다양화. 근거 E2E = `testlog_26062217_25_17`(0.23.0 source 26.05) · `testlog_26062422`(듀얼모델 E2E 26.05 재검증).
- strip-hoist 가 torch 2.12 에서 자동 skip 된 것은 **조건부 패치의 재사용 가능 패턴**이다(부재감지 = 적용여부 자동결정).

## 3.1 가드 3출구 — 상속(inherit) · 시도-빌드(attempt) · 발견(discovery)

*(plan_26082112 §5.2 · P4. `render_dockerfile.py::_patch_guard` 의 세 분기가 이 표다. 가드 스탠자가 지시하는 주소가 여기다.)*

미인식 (NGC베이스 × vLLM버전) 키의 출구는 **하나가 아니다**. 하나뿐이면 델타가 진짜 ∅ 인 경우와
델타가 큰 경우가 같은 버튼(`--allow-unvalidated`)을 쓰게 되고, 그 버튼은 고무도장이 된다.

| 출구 | 조건 | 처방 | 스탠자 |
|---|---|---|---|
| **① 상속(inherit)** | `axis_A == NO_IMPACT` ∧ `axis_B.silent_revert_risk == []` ∧ 전역 `verdict != UNDETERMINED` ∧ `unknown == []` ∧ **사람이 원장에 등재** | `INHERITED_SOURCE_BUILD_KEYS` 항목 + `resolved.json#upstream_delta` attestation | `[guard] … INHERITED from (ngc, vllm_old)` → 진행 |
| **② 시도-빌드(attempt)** | 전역 `verdict == IMPACT` 이고 사람이 명시 승인 | `render --allow-unvalidated` (WARN 강등 + testlog 의무) — §3 | `WARN: … UNVALIDATED` → 진행 |
| **③ 발견(discovery)** | 전역 `verdict == UNDETERMINED` 또는 `UNKNOWN_PLANE` 존재 또는 판정 미실시 | 위 §3 HITL 발견 루프 | `ERROR: … NO validated patch set` → `exit 1` |

**출구①의 성립 조건 — 선언은 증거가 아니다.** 원장에 한 줄 적는 것만으로는 상속이 성립하지 않는다.
렌더는 매번 아래를 **전부** 확인하고, 하나라도 걸리면 `exit 1` 한다(사유를 스탠자에 열거한다):

- 항목 4필드(`inherits`·`attestation`·`approved_by`·`approved_kst`) 전부 존재
- 상속원이 **같은 NGC 베이스**이고 `VALIDATED_SOURCE_BUILD_KEYS` 에 실재 (→ **상속의 상속=체인 금지**)
- `attestation` 포인터가 `<경로>#upstream_delta` 형식
- 렌더 입력 `resolved` 에 `upstream_delta` 블록이 실재하고 `provenance == "measured"`, `schema_version` 이 승인 목록 안
- attestation 의 `from_ref → to_ref` 가 상속 주장 `vllm_old → vllm_new` 와 **일치** (다른 bump 의 증거 재활용 차단)
- `axis_A` 가 `NO_IMPACT`, **`axis_B.silent_revert_risk == []`**, 전역 `verdict` 가 `UNDETERMINED` 아님, `unknown == []`
  - B축 정지조건은 `axis_B.verdict` 가 **아니다**(2026-09-03 · `plan_26082112` U7 해소). 이식 트랙의
    번들은 상류에 없는 내용을 의도적으로 가지므로 델타가 번들 스코프와 겹치기만 하면 `axis_B` 는
    `IMPACT` 다 — verdict 를 정지조건으로 쓰면 출구①이 **정의상 도달 불가**가 되어 ②가 상시화된다.
    위험분(`WOULD_REVERT`·`UNDETERMINED`)만 담는 `silent_revert_risk` 는 정상 재파생으로 비울 수
    있으므로 도달 가능한 게이트가 된다(§5.3.1 정지조건과 같은 축). 필드가 **없으면 통과가 아니라
    거부**다 — 리스트가 아니면 fail-closed.

> **부적격 상속은 `--allow-unvalidated` 로 우회되지 않는다.** 그것은 *미검증 키*가 아니라 **원장 항목의 결함**이므로,
> 처방은 증거를 고치거나(judge 재실행) 항목을 지우는 것이다(D3: 우회 말고 경로를 고친다). 항목이 **없으면 기본은
> ③ 발견**이다 — 부재가 기본값이라 "상속을 쓰지 않는 상태"가 값 수정이 아니라 **줄 삭제**로 표현된다.

**상속되는 것과 되지 않는 것.** 상속되는 것은 **빌드 키**(그 패치 셋이 이 조합에 적용 가능한가)뿐이다.
기능 판정이 아니다 — `axis_C`(모델 코드경로 도달성)는 전역 verdict 에서 제외되어 있고, **상속은 스모크를 면제하지
않는다**(workflow S3 · HITL 게이트 ③). 상속 스탠자가 이 두 사실을 매 빌드 로그에 찍는 이유다.

**등재는 사람의 손이다**(게이트 ①.6). Judge 는 attestation 을 낼 뿐 원장을 넓히지 않는다 —
`judge_version_delta.py` 의 산출물은 **evidence 이지 approval 이 아니다**.

## 4. 빌드-바깥 의존 패치 (`build_patches/`)

> §1~3 = vLLM **빌드 자체**의 ABI 수정(inline·torch/NGC-keyed). 여기 = **모델이 요구하는 native 의존**(lib/커널) 추가 —
> 예 **DeepGEMM**(DeepSeek-V4 DSA `SparseAttnIndexer` 가 요구, 미설치 시 하드 RuntimeError).
> per-model 3+1+1 의 "빌드-바깥 패치" 슬롯(plan_26062812). 빌드평면·동결·재현·HITL 거버넌스는 §1~3 과 공유.

- **발견 ≠ 소유 (intake)**: 발견은 `vllm-recipe-explorer` crosscheck(special-dep 경보) — **메인**이면 직접 핸드오프,
  **서브**면 docs insight 상향(D12, 서브는 빌드평면 미보유). upstream-version-watch 가 **이미지에 넣는 책임**(어떻게)을 진다.
  **patch.py ✗**(native lib 은 Python 몽키패치 불가).
- **모듈화 (Dockerfile bloat 차단)**: 패치 = **`output/<topology>/build_patches/<NN>-<name>.sh`** 모듈(**비추적 산출물**(2026-09-10 철회 — 3+1+1 슬롯 산출물이라 추적금지가 기본. 배달은 hint 페이로드·sync_to_sub) ·
  **빌드 컨텍스트=output/<t>/** · 통로 격리로 single/multi 혼재 차단 · 서브 전달=`sync_to_sub`). 각자 self-contained =
  헤더(what/why/model-trigger/plan-ref) + 설치·컴파일 + **검증(fail-loud)**. `Dockerfile.source-build` 는 **thin 스탠자 +
  원장 원시 캡처**: `COPY build_patches/ /tmp/build_patches/` + 정렬 루프(패치마다 `bash "$p"` · 0 이 아니면 `exit 1`)가
  패치별 종료코드·sha256·로그 꼬리·상태 줄을 `ledger.d/` 에 남긴다(2026-09-21 · §4.1).
  → **패치 추가 = 파일 drop(Dockerfile 무수정)** 은 그대로다 · 폴더 listing 이 아니라 **원장이 "무엇이 실제로 돌았나"** 를 말한다.
- **절차 (probe → 모듈 → 동결)**: ① probe(인터랙티브 컨테이너서 설치·컴파일·작동확인 — sm arch 지원 포함) →
  ② `build_patches/<NN>-<name>.sh` 저작 → ③ clean 재빌드 + 서빙 스모크 = **DONE**. 중간삽입이 필요한 드문 케이스만
  `Dockerfile.source-build` inline-marker(`# build-patch:<name> START/END`) fallback.
- **이미지 네이밍 불변식 보존**: 빌드-바깥 lib 은 범용(flashinfer 처럼) — DSA 안 쓰는 모델은 무시. 모델-키잉 이미지 ✗.
  첫 사례 = `10-deepgemm.sh`(DeepSeek-V4-Flash).

### 4.1 빌드 원장(build ledger) — `/opt/easy-vllm/build_ledger.json` (2026-09-21 · plan_26092119 §4.5)

**왜**: 패치는 skip 과 적용을 **둘 다 `exit 0`** 으로 끝낸다 — skip 은 `[50-…] skip — SM12X_PORT=0 …` 같은 자유 문장뿐이었다.
태그2(camp-26091216) hint 는 그래서 **skip 된 50·55 를 "적용된 빌드 패치"로 실었고**, 옛 "적용 증거"는 skip 된 50번 자신의
`PROVENANCE.json` 이었다. 폴더에 **있음** 은 **적용됨** 이 아니다. 이미지가 자기 빌드를 서술하지 않으면 그 사실은 빌드 로그와 함께 사라진다.

- **스탠자 계약**(두 템플릿 공통 · `render_dockerfile.py` 가 `{{ BUILD_LEDGER_STANZA }}` 한 자리로 **단일 소유** 주입):
  - pre/post 루프는 **원시 사실만** 남긴다: `ledger.d/<pre|post>.<파일>.{rc,sha256,log,status}` + `<pre|post>.inventory`(`ls -A`).
    로그는 `tee` 로 빌드 출력에 그대로 흐르고 원장엔 꼬리 64KiB 만 남는다. POSIX sh 만 쓴다(wheel 베이스 `/bin/sh -c` ↔ 26.07 `/bin/bash -c`).
  - 인라인 패치(source `strip-hoist` · wheel `HAS_OPAQUE_TYPE`)는 `ledger.d/inline.<이름>.status` 한 줄을 쓴다.
  - 분류·조립은 **마지막 RUN**(검증 RUN 뒤 · `WORKDIR` 앞) 한 곳 — 분류 로직을 고쳐도 그 레이어만 다시 빌드되고 `_C` 컴파일 캐시는
    보존된다. 원장 도입 자체는 pre 루프 텍스트가 바뀌므로 **1회** 컴파일 캐시를 깬다(비용 기록).
  - build-arg 목록은 **템플릿 자신의 `^ARG` 줄에서 렌더 시 파생**한다(손목록 ✗ — 새 ARG 는 자동 기록). `dockerfile` 값은 렌더한
    템플릿 이름에서 파생한다(resolved 의 트랙 선언 ✗).
  - `import vllm._C` 금지(빌드 시 libcuda 없음 · §1 step 5 불변식) — 버전은 `importlib.metadata` 로만 읽는다.
  - 베이스 사실은 **파일**에서 읽는다(`/usr/local/cuda/version.json`). wheel 트랙 `ARG CUDA_VERSION=130` 이 NGC ENV `CUDA_VERSION` 을
    가리기 때문이다 — 같은 이름의 ARG 가 가리는 ENV 는 값 대신 `shadowed-by-ARG` 로 적는다(추측 ✗).
- **상태 프로토콜**(패치 작성자용 · 선택): 루프는 패치마다 `EASY_VLLM_PATCH_STATUS=<파일>` 을 넘긴다. 패치는 그 파일에
  `applied<TAB>메모` 또는 `skipped<TAB>사유` **한 줄**을 쓰고 `exit 0` 한다. 분류 우선순위 = 상태 줄(`status-file`) >
  옛 패치의 로그 토큰(`] skip —`·`— skip`·`— skip(` → `log-token`, 라벨된 폴백) > 종료코드(`exit-code` = "적용(자기보고 없음)").
  형식 밖 상태 줄은 채택하지 않고 `status_file_error` 로 표시한 뒤 폴백한다. 실패(0 아닌 종료)는 여전히 빌드를 멈춘다.
- **스키마**(`schema_version: 1` · `provenance: measured(build-time, in-image)`): `dockerfile`·`track`·`build_args`(ARG 전부) ·
  `base{ngc_pytorch_version, pytorch_build_version, cuda_version, source}` · `vllm{git_sha, describe(git describe --tags --match 'v*'),
  dist_version, direct_url}` · `torch.dist_version` · `cpu_arch` · `slots_supported` · `context_inventory` ·
  `patches[{phase: pre|post|inline, file, script_sha256, exit_code, result: applied|skipped|unobserved, reason, result_source,
  log_tail_sha256, declared_model_trigger(# model-trigger 헤더)}]` · `requirements_sha256` · `pip_freeze_sha256`(+ `/opt/easy-vllm/pip-freeze.txt`) ·
  `errors`. 벽시계 시각은 없다(이미지 `Created` 가 이미 든다).
- **적용 ≠ 모델 요구**: post 패치(10–40 · 공유 이미지 arch-enablement)는 적용됐으면 적용으로 기록된다. 그것이 "이 모델의 요구" 인지는
  `declared_model_trigger` 가 말하고, 해석(공유 이미지의 일부)은 hint 가 싣는다(X10).
- **읽는 자**: 멀티는 `multinode_serve_smoke.sh` 가 serve 시점에 두 노드 **실행 중 컨테이너**에서 캡처해 attestation v2 에 싣는다
  (`multinode-build.md`) — hint 는 그 파일만 읽는다(서브 무단 스캔 ✗ · 노드 제어 ①).
- **옛 이미지**(원장 도입 전): `applied_set: unobservable` 로 기재한다(침묵 폴백 ✗ · 차단 ✗). `docker history` 의 build-arg 와 패치
  자기게이트 규칙으로 만든 재구성은 **라벨된 재구성**(`reconstructed(docker-history+gate)`)이지 관측이 아니다.
- **policy:GIT_SINGLE_AUTHORITY**: 원장은 추적물 digest 의 재기재가 아니라 **빌드 시점 적용 결과**다(Q1 아니오 · Q2 아니오 — 상류 클론·
  네트워크 의존·게이트 평가에 달렸다) → 맹점층이라 유지가 정당하다. 스크립트 sha256 은 비추적 슬롯 산출물(2026-09-10~)의 것이다.
- **철회 조건의 대가**: 원장을 이미지 `LABEL` 로 축소하면 build-arg(정적 치환)만 남고 **패치별 결과(RUN 출력)는 잃는다** —
  태그2 의 skip 오기 같은 사고를 다시 못 잡는다.

## 5. 도커 패치 범위 래더 (최하단 → 상단)

`deps-패치(build_patches)` → `소스-게이트 패치(sed)` → **vLLM source-repo 오버라이드(fork SHA 핀 `VLLM_REPO`/`VLLM_REF`)** →
`체크포인트-교체`. build_patches 로도 stock vLLM 이 **구조적 불가**(arch-wall, 예 GB10 sm_121 DeepSeek-V4)면 위 사다리로
에스컬레이션한다(fork 핀 = 1급 HITL 오버라이드). 절차 정본 = `.claude/rules/workflow.md` S3 · 거버넌스 요약은
SKILL.md §escalation(기존모델 회귀 재스모크·단일 변종-트랙·무증거 오버라이드 금지).
