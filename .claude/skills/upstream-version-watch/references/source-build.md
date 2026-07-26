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
5. **동결 + 재현**: 성공 레시피 → `Dockerfile.source-build`. **clean 재빌드 + 스모크 = DONE**(인터랙티브 성공만으론 부족).
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

## 4. 빌드-바깥 의존 패치 (`build_patches/`)

> §1~3 = vLLM **빌드 자체**의 ABI 수정(inline·torch/NGC-keyed). 여기 = **모델이 요구하는 native 의존**(lib/커널) 추가 —
> 예 **DeepGEMM**(DeepSeek-V4 DSA `SparseAttnIndexer` 가 요구, 미설치 시 하드 RuntimeError).
> per-model 3+1+1 의 "빌드-바깥 패치" 슬롯(plan_26062812). 빌드평면·동결·재현·HITL 거버넌스는 §1~3 과 공유.

- **발견 ≠ 소유 (intake)**: 발견은 `vllm-recipe-explorer` crosscheck(special-dep 경보) — **메인**이면 직접 핸드오프,
  **서브**면 docs insight 상향(D12, 서브는 빌드평면 미보유). upstream-version-watch 가 **이미지에 넣는 책임**(어떻게)을 진다.
  **patch.py ✗**(native lib 은 Python 몽키패치 불가).
- **모듈화 (Dockerfile bloat 차단)**: 패치 = **`output/<topology>/build_patches/<NN>-<name>.sh`** 모듈(추적 빌딩블럭 ·
  **빌드 컨텍스트=output/<t>/** · 통로 격리로 single/multi 혼재 차단 · 서브 전달=`sync_to_sub`). 각자 self-contained =
  헤더(what/why/model-trigger/plan-ref) + 설치·컴파일 + **검증(fail-loud)**. `Dockerfile.source-build` 는 **단일 thin 스탠자**:
  `COPY build_patches/ /tmp/build_patches/` + `RUN for p in $(ls /tmp/build_patches/*.sh|sort); do bash "$p"||exit 1; done`.
  → **패치 추가 = 파일 drop(Dockerfile 무수정)** · 폴더 listing = self-documenting 레지스트리(카탈로그 ✗).
- **절차 (probe → 모듈 → 동결)**: ① probe(인터랙티브 컨테이너서 설치·컴파일·작동확인 — sm arch 지원 포함) →
  ② `build_patches/<NN>-<name>.sh` 저작 → ③ clean 재빌드 + 서빙 스모크 = **DONE**. 중간삽입이 필요한 드문 케이스만
  `Dockerfile.source-build` inline-marker(`# build-patch:<name> START/END`) fallback.
- **이미지 네이밍 불변식 보존**: 빌드-바깥 lib 은 범용(flashinfer 처럼) — DSA 안 쓰는 모델은 무시. 모델-키잉 이미지 ✗.
  첫 사례 = `10-deepgemm.sh`(DeepSeek-V4-Flash).

## 5. 도커 패치 범위 래더 (최하단 → 상단)

`deps-패치(build_patches)` → `소스-게이트 패치(sed)` → **vLLM source-repo 오버라이드(fork SHA 핀 `VLLM_REPO`/`VLLM_REF`)** →
`체크포인트-교체`. build_patches 로도 stock vLLM 이 **구조적 불가**(arch-wall, 예 GB10 sm_121 DeepSeek-V4)면 위 사다리로
에스컬레이션한다(fork 핀 = 1급 HITL 오버라이드). 절차 정본 = `.claude/rules/workflow.md` S3 · 거버넌스 요약은
SKILL.md §escalation(기존모델 회귀 재스모크·단일 변종-트랙·무증거 오버라이드 금지).
