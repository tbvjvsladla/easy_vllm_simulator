# 01 · 산출물 — 무엇이 실제로 쓰였고, 어떻게 다시 띄우는가

## 1.1 적용 판정 표
> 기계가 적용 증거(빌드 원장 · 원장이 없으면 라벨된 재구성) × 파일 × 선언을 대사한 결과다. `artifacts/` 에는 **적용된 것만** 실렸다 — 빌드 때 스스로 skip 된 패치와 쓰이지 않은 평면의 파일은 표에만 남고 싣지 않는다.

<!-- FACT:slots -->
| 슬롯 | 실림 | 파일 | 신호 | 적용 증거 | 사유 |
|---|---|---|---|---|---|
| `build_patch_post` | 안 실림 | — | 1-signal | none:None | 미관측 |
| `build_patch_pre` | 안 실림 | — | 1-signal | none:None | 미관측 |
| `build_recipe` | 실림 | `artifacts/build_recipe/native-install.sh` · `artifacts/build_recipe/pip-freeze-main.txt` · `artifacts/build_recipe/pip-freeze-sub.txt` | 2-signal | file:native producer preserved install + pip freeze(main/sub) | (a) 무엇: run 2 에서 정문 `native_multinode_serve.py up` 이 **각 노드마다** 실행한 설치 순서를 그대로 적은 재현 스크립트다. 두 단계뿐이다 — ① `native_wheelhouse.py build`: 로컬 이미지에서 시작하지 않는 컨테이너를 만들어 설치 트리를 꺼내고 `*.dist-info/RECORD` 대로 휠을 다시 묶는다(재컴파일 ✗ · 승인 선언 대조) ② `native_wheelhouse.py verify`: `python3.12 -m venv` → `pip install --no-index --no-deps -r requirements-closure.txt` → 설치 집합 == closure 핀 → pip check ⊆ 선언된 이미지 고유 충돌 → vLLM… |
| `compose` | 안 실림 | — | 1-signal | none:None | 미관측 |
| `fork_pin` | 안 실림 | — | 1-signal | none:.claude/policies/arch_variant_ledger.json | 미관측 |
| `runtime_patch` | 안 실림 | — | 1-signal | none:output/multi/configs/nv4-bf-262k-mmp-native_patch.py | 미관측 |
| `triplet` | 실림 | `artifacts/triplet/.env.nv4-bf-262k-mmp-native` · `artifacts/triplet/nv4-bf-262k-mmp-native.sh` · `artifacts/triplet/nv4-bf-262k-mmp-native.yaml` | 3-signal | file:output/multi/configs/nv4-bf-262k-mmp-native.yaml | Docker 트리플렛 `nv4-bf-262k-mmp` 에서 `render_native_triplet.py` 가 결정론 파생한 렌더 산출물이다(손저작 ✗ · 원본 sha256 을 머리 주석에 싣는다). 노브는 원본과 바이트 동일하고, 경로만 자리표시로 바뀌었다 — `<manifest.quant_model_path>` · `<manifest.ple_mmap_host_path>` 는 **네 manifest 의 해당 필드 값으로 치환해 읽으라는 표지**다(발행기가 운영자 경로를 manifest 치환표로 바꿨다 — devlog_26092317 §2). 러너는 `NATIVE_VENV` · `CONFIG_FILE` · `SERVING_MODEL_NAME` 이 없으면 멈추고 `${NATIVE_VENV}/bin/vllm… |
<!-- /FACT:slots -->

<!-- FACT:applied_set -->
_적용 집합 미관측 — 빌드 원장·재구성 모두 없다(결손)._
<!-- /FACT:applied_set -->

## 1.2 슬롯별 적용 사유
> 이 절이 답하는 질문: 실린 산출물 하나하나는 무엇을 고치며, 왜 이 셀에 필요했고, 이 셀에서 실제로 발화했으며, 언제 불필요해지는가?

이 셀은 native 평면이라 이미지 안 빌드 원장(적용 집합)을 관측하지 않았고 빌드 패치 슬롯은 싣지 않았다(1.1 — `build_patch_pre` · `build_patch_post` 안 실림). 대신 **설치 재현 키트**(`native-install.sh` · 양 노드 pip freeze)와 **렌더된 트리플렛**이 실렸다. 빌드 패치가 사라진 것이 아니다 — 패치는 설치 원천인 이미지 안에 이미 구워져 있고 wheelhouse 가 그 바이트를 재포장해 가져온다(plan_26092311 N-D1). 그 흔적이 승인된 RECORD 불일치 하나다 — `humming-kernels` 의 `humming/utils/device.py` 는 빌드 패치 40-humming-nvml-gb10 이 설치 뒤 제자리 수정한 바이트다(W8 · 승인 선언).

### build_recipe — `native-install.sh`

(a) 무엇: run 2 에서 정문 `native_multinode_serve.py up` 이 **각 노드마다** 실행한 설치 순서를 그대로 적은 재현 스크립트다. 두 단계뿐이다 — ① `native_wheelhouse.py build`: 로컬 이미지에서 시작하지 않는 컨테이너를 만들어 설치 트리를 꺼내고 `*.dist-info/RECORD` 대로 휠을 다시 묶는다(재컴파일 ✗ · 승인 선언 대조) ② `native_wheelhouse.py verify`: `python3.12 -m venv` → `pip install --no-index --no-deps -r requirements-closure.txt` → 설치 집합 == closure 핀 → pip check ⊆ 선언된 이미지 고유 충돌 → vLLM `.so` 의 ldd → `import vllm, torch` + CUDA → 런타임 env 레시피 산출.

> [원문] native-install.sh §L11-15
> # ① 재포장(docker create → 설치 트리 cp → RECORD 대조 재포장 · 시작하지 않는 컨테이너)
> python3 "${WH_TOOL}" build --image "${IMAGE}" --out "${RUN_ROOT}/wh" --generated-utc 2026-09-23T06:53:50Z --accept-file "${WH_ACCEPT}"
> # ② 설치 게이트(이미지 동등성) — 도구가 python3.12 -m venv → pip install --no-index --no-deps -r wh/requirements-closure.txt
> #    → 설치 집합 == closure 핀 → pip check ⊆ 선언된 이미지 고유 충돌 → ldd(vllm *.so) → import vllm,torch + cuda → env 레시피
> python3 "${WH_TOOL}" verify --wheelhouse-dir "${RUN_ROOT}/wh" --venv "${RUN_ROOT}/venv" --generated-utc 2026-09-23T06:53:50Z --accept-file "${WH_ACCEPT}"

(b) 왜 필요했나: 캠페인이 패키지 다운로드를 금지했고(R3), 이미지와 같은 vLLM · torch · CUDA 라이브러리를 Docker 없이 쓰려면 이미지 설치본을 그대로 옮기는 수밖에 없었다(W8~W11). (c) 이 셀에서 실제로 돌았다 — run 2 의 build 는 양 노드 PASS(분포 320 · 휠 319 · 재포장 317 · editable 1 · 이미지 휠 1 · 제외 4 · 승인 외 불일치 0 · 메인 412.8 s · 서브 420.9 s), verify 는 closure PASS · pip install 약 75 s · 설치 집합 PASS · pip check 는 선언된 충돌 1줄만 · ldd · import/CUDA PASS 였다(`docs/simlog/26092315_native_N1/wheelhouse-{build,verify}-{main,sub}.json`). (d) 언제 불필요해지나: 목표 vLLM 이 공식 wheel 로 설치 가능하고 필요한 빌드 패치가 상류에 들어가면 이미지 재포장 대신 일반 오프라인 wheel 설치로 대체할 수 있다(추론). 스크립트는 저장소 루트에서 도구(`native_wheelhouse.py`)와 승인 선언(`native_wheelhouse_accept.json`)을 저장소 상대경로로 부른다 — **이 zip 만으로는 돌지 않는다**. 승인 선언은 두 이미지 digest 에만 결속돼 다른 이미지에서는 도구가 거부한다(목록 밖 불일치 → 실패).

### build_recipe — `pip-freeze-main.txt` · `pip-freeze-sub.txt`

run 2 의 양 노드 venv 에서 뜬 설치 목록이며 두 파일은 바이트 동일하다(320줄 · 저작자 diff). 핵심 핀:

> [원문] pip-freeze-main.txt §L306-306
> vllm==0.29.0rc7.dev0+g74c96922e.d20260922.cu133

`torch==2.13.0a0+9186a08b2c.nv26.7.59513937` · `triton==3.7.1+gitf797708c.nv26.7` · `ray==2.48.0` · `flashinfer-python==0.6.18` · `deep_gemm==2.5.0+a6b593d` · `humming-kernels==0.1.12` · `protobuf==6.33.6` · `grpcio-tools==1.83.0` 이 들어 있다. 버전 뒤의 `+g74c96922e` 는 빌드 입력 SHA 의 앞 9자이고 `rc7.dev0` 는 소스 빌드의 dev 문자열이다 — 재현 좌표가 아니다(00 §0.4). 이 목록을 인터넷 인덱스에 `pip install -r` 로 넣어도 같은 설치본이 되지 않는다 — `+nv26.7`·`+a6b593d`·`.cu133` 같은 로컬 버전은 NGC · 이미지 빌드 산출물이다.

### triplet — `nv4-bf-262k-mmp-native.yaml` · `nv4-bf-262k-mmp-native.sh` · `.env.nv4-bf-262k-mmp-native`

Docker 트리플렛 `nv4-bf-262k-mmp` 에서 `render_native_triplet.py` 가 결정론 파생한 렌더 산출물이다(손저작 ✗ · 원본 sha256 을 머리 주석에 싣는다). 노브는 원본과 바이트 동일하고, 경로만 자리표시로 바뀌었다 — `<manifest.quant_model_path>` · `<manifest.ple_mmap_host_path>` 는 **네 manifest 의 해당 필드 값으로 치환해 읽으라는 표지**다(발행기가 운영자 경로를 manifest 치환표로 바꿨다 — devlog_26092317 §2). 러너는 `NATIVE_VENV` · `CONFIG_FILE` · `SERVING_MODEL_NAME` 이 없으면 멈추고 `${NATIVE_VENV}/bin/vllm serve --config … --served-model-name …` 를 exec 한다 — 컨테이너 전용 arming(`arm_patch.sh` · tiktoken)은 렌더가 막았다. env 는 Docker 평면 키(`IMAGE_TAG` · `BUILD_DOCKERFILE` · `BUILD_JOBS` · `VLLM_REF` · 컨테이너 이름)를 뺐다 — 발행기는 이 부재로 평면을 native 로 판정한다. yaml 머리에 원본의 주석("A1 기준선 … 음성대조")이 그대로 남아 있는데 그것은 Docker 원본을 쓴 res-min 캠페인의 의도다(02 §2.5).

### 싣지 않은 슬롯

- `build_patch_pre` · `build_patch_post`: 안 실림 — native 는 빌드하지 않고 이미지 바이트를 재포장한다. 어느 패치가 이미지에 적용됐는지는 같은 이미지를 쓴 Docker 셀(D1)의 빌드 원장이 기록한다(이 셀의 관측이 아니다).
- `compose`: 안 실림 — Docker 평면이 아니다. 기동은 정문 up 이 Ray head/worker 와 `vllm serve` 를 호스트 프로세스로 띄운다(plan_26092311 §4.2).
- `fork_pin`: 없음(stock `v0.29.0rc6`) · `runtime_patch`: 없음.

## 1.3 값의 지위표
> 서빙 설정의 노브 **전부**와 기계가 모은 후보 지위다(lockset 출처 · yaml 주석 · 엔진 기본값). 최종 지위는 아래 `kind: value-status` 블록이 정한다 — 표의 노브마다 블록이 정확히 하나 있어야 린터를 통과한다.

<!-- FACT:value_status -->
서빙 노브 9개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · 값 칸의 `\|` 는 `|` 로 적는다).

| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |
|---|---|---|---|---|---|
| `tensor-parallel-size` | 2 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `distributed-executor-backend` | ray | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `gpu-memory-utilization` | 0.85 | declared-requirement | lockset 출처 gmu_source=target_gmu · lockset provenance=hand-authored | — | lockset.gmu_source=target_gmu |
| `max-model-len` | 262144 | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
| `kv-cache-dtype` | auto | engine-default | 값 `auto` = 엔진이 고른다 | — | serving-yaml 값 토큰 |
| `kv-cache-memory-bytes` | 21474836480 | inherited | lockset 출처 kv_source=hand · lockset provenance=hand-authored | 20480MiB/노드 | lockset.kv_source=hand |
| `enforce-eager` | true | inherited | 주석 단어 '승계': 승계 통제변인(캡처 스파이크 ~10GiB 회피) | 승계 통제변인(캡처 스파이크 ~10GiB 회피) | serving-yaml 줄 주석 |
| `async-scheduling` | false | inherited | 주석 단어 '필수': MTP+async 금지(0.29 자동비활성 안 탐 — 명시 필수) · yaml 주석만 있음 — 관측된 실패가 있으면 declared-requirement | MTP+async 금지(0.29 자동비활성 안 탐 — 명시 필수) | serving-yaml 줄 주석 |
| `speculative-config` | {"method":"mtp","num_speculative_tokens":3} | 후보 없음 | 근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다) | — | serving-yaml |
<!-- /FACT:value_status -->

> 이 절이 답하는 질문: 각 서빙 노브의 값은 이 셀에서 어떤 지위인가 — 조정됐나, 승계됐나, 음성대조로 일부러 둔 것인가, 엔진 기본값인가, 필요조건인가?

```hint-event
id: V1
kind: value-status
노브: tensor-parallel-size
값: 2
지위: inherited
근거: manifest 토폴로지(노드 2 × 노드당 GPU 1)에서 정해진 값 — 계보에서 바꿔 잰 기록 0
출처: [plan_26090918_qwen38_flashnext_multi_딥캠페인 §1.]
```

```hint-event
id: V2
kind: value-status
노브: distributed-executor-backend
값: ray
지위: inherited
근거: 멀티노드 TP 의 Ray 경로 — Docker 원본에서 렌더로 승계 · native 정문도 Ray head/worker 를 띄운다 · 빼고 잰 기록은 이 셀에 없다
출처: [plan_26092311_native_분산서빙_정문_구현_N1 §4.2 정문 (N-D4 · N-D6 · N-D7), nv4-bf-262k-mmp-native.yaml]
```

```hint-event
id: V3
kind: value-status
노브: gpu-memory-utilization
값: 0.85
지위: inherited
근거: P0-1 스모크 최종 설정에서 승계 · 이 셀에서 바꿔 잰 기록 0 · lockset gmu_source=target_gmu 는 선언이지 관측된 실패가 아니다
출처: [testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §패치 중재 세부]
```

```hint-event
id: V4
kind: value-status
노브: max-model-len
값: 262144
지위: inherited
근거: 모델 네이티브 컨텍스트(YaRN 없이) · 캠페인 셀 축 통제변인 — 조정 0
출처: [plan_26090918_qwen38_flashnext_multi_딥캠페인 §3. 셀 매트릭스 (기본 24셀 + 레버 서브스윕)]
```

```hint-event
id: V5
kind: value-status
노브: kv-cache-dtype
값: auto
지위: engine-default
근거: auto = 엔진이 모델 dtype(bf16)을 쓴다
출처: [plan_26091216_qwen38fn_res최소조건_후속캠페인 §2.4]
```

```hint-event
id: V6
kind: value-status
노브: kv-cache-memory-bytes
값: 21474836480
지위: inherited
근거: Docker 원본 트리플렛에서 조정 없이 승계한 손레버(lockset kv_source=hand) · 262k bf16 필요량 3,072MiB/노드를 크게 넘는다 · 음성대조 의도는 원본을 쓴 res-min 캠페인의 것 · 스윕 0
출처: [plan_26091216_qwen38fn_res최소조건_후속캠페인 §2.4, nv4-bf-262k-mmp-native.yaml]
```

```hint-event
id: V7
kind: value-status
노브: enforce-eager
값: true
지위: inherited
근거: W5(P0-1 · PLE resident 구성의 그래프 캡처 트립) 이후 승계 통제변인 — 이 셀에서 eager 를 끄고 잰 기록 0
출처: [testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정]
```

```hint-event
id: V8
kind: value-status
노브: async-scheduling
값: false
지위: inherited
근거: 외부 선례의 MTP+async 금지 조합 회피용 명시(R4 · Docker 계보) — 우리 계보에서 켜서 실패한 관측은 없다
출처: [testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정 §§1. ★ 중재 술어 재설계 — "토큰 정확 일치"는 이 스택에서 도달 불가]
```

```hint-event
id: V9
kind: value-status
노브: speculative-config
값: '{"method":"mtp","num_speculative_tokens":3}'
지위: inherited
근거: 사람 결정(MTP 켠 채 · 2026-09-12)으로 고정된 통제변인 · plan 이 레버로 적은 MTP k 스윕은 시도 0
출처: [plan_26091216_qwen38fn_res최소조건_후속캠페인 §4. 단계, plan_26090918_qwen38_flashnext_multi_딥캠페인 §3. 셀 매트릭스 (기본 24셀 + 레버 서브스윕)]
```

**요약.** 노브 9개는 전부 Docker 원본의 값을 렌더로 승계했고, 이 셀에서 조정한 값(`tuned`)도, 빼서 실패를 관측한 필요조건(`declared-requirement`)도 없다. 복사하면 가장 위험한 값은 V6(KV 20GiB) — 필요량을 크게 넘는 승계 손레버다. V3 · V7 · V9 는 다른 구성 · 사람 결정에서 승계됐다. `moe-backend` 는 표에 없다 — 명시하지 않는 것이 요건이다(W3 · W4). native 에서 노브보다 먼저 맞춰야 하는 것은 설치 입력(1.2)과 런타임 env(1.4)다.

## 1.4 재현 절차
> 셀 실행 기록에서 기계가 파생한 명령 순서와 단계별 소요다. 소요가 `미관측` 인 단계는 시각 기록이 없다는 뜻이다(0 이 아니다). 그 아래는 블랙박스 이벤트 원장에서 이 셀의 행을 기계가 뽑아 기동 시도로 묶은 것이고, 마지막은 측정한 실행의 엔진 로그가 echo 한 env 다(측정 당시 값의 정본).

<!-- FACT:reproduce -->
| 순서 | 단계 | 성공 판정 | 소요 | 관측 구간(UTC) | 소요 출처 | 명령 출처 |
|---|---|---|---|---|---|---|
| 1 | install | venv 에서 `python -c 'import vllm'` 성공 | 미관측 | 미관측 | native producer preserved build_recipe | shipped-file |
| 2 | serve | health 200 + 추론 1회 + cleanup attestation PASS | 미관측 | 미관측 | 미관측 | derived |
| 3 | bench | 리포트 발행(+PASS 면 인증서) | 미관측(관측 시각: 2026-09-23T08:10:39Z) | ? → 2026-09-23T08:10:39Z (none) | 미관측 | reconstructed(bench json) |

**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 아니다)

**1. install**

```sh
artifacts/build_recipe/native-install.sh + pip-freeze-main.txt + pip-freeze-sub.txt
```

**2. serve**

```sh
bash output/multi/configs/nv4-bf-262k-mmp-native.sh(native --plane native)
```

**3. bench**

```sh
# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).
# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · --trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh.
# output/multi/benchlog/sweep_nv4-bf-262k-mmp-native/lite_cold_nv4-bf-262k-mmp-native.json (date 20260923-164047)
vllm bench serve --backend openai-chat --model qwen3.8-flash-next --tokenizer <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4 --dataset-name random --random-input-len 566 --random-output-len 128 --num-prompts 1 --max-concurrency 1 --request-rate inf
# output/multi/benchlog/sweep_nv4-bf-262k-mmp-native/lite_warm_nv4-bf-262k-mmp-native.json (date 20260923-164125)
vllm bench serve --backend openai-chat --model qwen3.8-flash-next --tokenizer <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4 --dataset-name random --random-input-len 565 --random-output-len 128 --num-prompts 3 --max-concurrency 1 --request-rate inf
```

**기동 성공 판정(관측 · 발행 자격)**: health 200 = true · 추론 1회 = true · 근거 `output/multi/benchlog/serve_proof_nv4-bf-262k-mmp-native.json` · `output/multi/benchlog/serve_proof_nv4-bf-262k-mmp-native.json#cleanup_attestation_path → docs/simlog/26092315_native_N1/cleanup_attestation.json`
<!-- /FACT:reproduce -->

<!-- FACT:event_timeline -->
**블랙박스 이벤트 20행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌)

| # | UTC | 노드 | kind | label | 내용 | 출처 |
|---|---|---|---|---|---|---|
| 1 | 2026-09-23T04:05:15Z | main | `budget_declare` | `smoke-nv4-bf-262k-mmp-native` | 예산 선언(선언값 — 측정 아님) · floor_mib=40476 · weights_mib=38852 · kv_mib=20480 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 1/4: budget_renew 0회 · 닫힘 budget_clear 2026-09-23T04:05:22Z(선언 후 0m07s) · measured_utc 2026-09-23T08:10:39Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-09.jsonl:970 |
| 2 | 2026-09-23T04:05:15Z | main | `budget_honored` | `smoke-nv4-bf-262k-mmp-native` | budget_honored · floor_mib=40476 · arm_ceiling_mib=37404 · remaining_s=13500 · 창 시작 2026-09-23T04:05:15Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:501 |
| 3 | 2026-09-23T04:05:22Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-nv4-bf-262k-mmp-native 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-23T04:05:15Z 후 0m07s) | docs/logs/main/events/2026-09.jsonl:971 |
| 4 | 2026-09-23T04:05:23Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-nv4-bf-262k-mmp-native 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-23T04:05:15Z 후 0m08s) | docs/logs/main/events/watchdog.jsonl:502 |
| 5 | 2026-09-23T04:06:36Z | main | `budget_declare` | `smoke-nv4-bf-262k-mmp-native` | 예산 선언(선언값 — 측정 아님) · floor_mib=40476 · weights_mib=38852 · kv_mib=20480 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 2/4: budget_renew 0회 · 닫힘 budget_clear 2026-09-23T04:15:06Z(선언 후 8m30s) · measured_utc 2026-09-23T08:10:39Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-09.jsonl:973 |
| 6 | 2026-09-23T04:06:36Z | main | `budget_honored` | `smoke-nv4-bf-262k-mmp-native` | budget_honored · floor_mib=40476 · arm_ceiling_mib=37404 · remaining_s=13500 · 창 시작 2026-09-23T04:06:36Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:503 |
| 7 | 2026-09-23T04:15:06Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-nv4-bf-262k-mmp-native 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-23T04:06:36Z 후 8m30s) | docs/logs/main/events/2026-09.jsonl:974 |
| 8 | 2026-09-23T04:15:06Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-nv4-bf-262k-mmp-native 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-23T04:06:36Z 후 8m30s) | docs/logs/main/events/watchdog.jsonl:504 |
| 9 | 2026-09-23T04:18:16Z | main | `budget_declare` | `smoke-nv4-bf-262k-mmp-native` | 예산 선언(선언값 — 측정 아님) · floor_mib=40476 · weights_mib=38852 · kv_mib=20480 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 3/4: budget_renew 1회 · 닫힘 budget_clear 2026-09-23T05:40:12Z(선언 후 1h21m56s) · measured_utc 2026-09-23T08:10:39Z 는 이 창 밖(다른 기동 · 측정 전) | docs/logs/main/events/2026-09.jsonl:977 |
| 10 | 2026-09-23T04:18:17Z | main | `budget_honored` | `smoke-nv4-bf-262k-mmp-native` | budget_honored · floor_mib=40476 · arm_ceiling_mib=37404 · remaining_s=13499 · 창 시작 2026-09-23T04:18:17Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:505 |
| 11 | 2026-09-23T04:36:01Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=85989 · rate_mib_s=23017 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-nv4-bf-262k-mmp-native 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-23T04:18:17Z 후 17m44s) | docs/logs/main/events/watchdog.jsonl:506 |
| 12 | 2026-09-23T05:04:42Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp-native` | budget_renew · floor_mib=40476 · ttl_s=13500 · remaining_before_s=10714 · 창 시작 2026-09-23T04:18:16Z 후 46m26s | docs/logs/main/events/2026-09.jsonl:978 |
| 13 | 2026-09-23T05:40:12Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-nv4-bf-262k-mmp-native 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-23T04:18:16Z 후 1h21m56s) | docs/logs/main/events/2026-09.jsonl:979 |
| 14 | 2026-09-23T05:40:13Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-nv4-bf-262k-mmp-native 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-23T04:18:17Z 후 1h21m56s) | docs/logs/main/events/watchdog.jsonl:507 |
| 15 | 2026-09-23T06:53:53Z | main | `budget_declare` | `smoke-nv4-bf-262k-mmp-native` | 예산 선언(선언값 — 측정 아님) · floor_mib=40476 · weights_mib=38852 · kv_mib=20480 · overhead_mib=24800 · mem_total_mib=124608 · 기동 창 4/4: budget_renew 1회 · 닫힘 budget_clear 2026-09-23T08:12:10Z(선언 후 1h18m17s) · measured_utc 2026-09-23T08:10:39Z 가 이 창 안(이 측정의 기동) | docs/logs/main/events/2026-09.jsonl:982 |
| 16 | 2026-09-23T06:53:53Z | main | `budget_honored` | `smoke-nv4-bf-262k-mmp-native` | budget_honored · floor_mib=40476 · arm_ceiling_mib=37404 · remaining_s=13500 · 창 시작 2026-09-23T06:53:53Z 후 0m00s | docs/logs/main/events/watchdog.jsonl:508 |
| 17 | 2026-09-23T07:11:39Z | main | `watchdog_highrate_hold` | — | watchdog_highrate_hold · mem_avail_mib=81906 · rate_mib_s=26052 · max_rate_mib_s=20768 · legacy_rule_would_trip=true · 라벨 없음 → smoke-nv4-bf-262k-mmp-native 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-23T06:53:53Z 후 17m46s) | docs/logs/main/events/watchdog.jsonl:509 |
| 18 | 2026-09-23T07:40:30Z | main | `budget_renew` | `smoke-nv4-bf-262k-mmp-native` | budget_renew · floor_mib=40476 · ttl_s=13500 · remaining_before_s=10703 · 창 시작 2026-09-23T06:53:53Z 후 46m37s | docs/logs/main/events/2026-09.jsonl:983 |
| 19 | 2026-09-23T08:12:10Z | main | `budget_clear` | — | budget_clear · existed=true · 라벨 없음 → smoke-nv4-bf-262k-mmp-native 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-23T06:53:53Z 후 1h18m17s) | docs/logs/main/events/2026-09.jsonl:984 |
| 20 | 2026-09-23T08:12:10Z | main | `budget_none` | — | budget_none · arm_ceiling_mib=999999999 · 라벨 없음 → smoke-nv4-bf-262k-mmp-native 선언 창 귀속(노드당 선언 슬롯 1개 · 창 시작 2026-09-23T06:53:53Z 후 1h18m17s) | docs/logs/main/events/watchdog.jsonl:510 |

**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — 해석은 저작자 몫)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-nv4-bf-262k-mmp-native` | 2026-09-23T04:05:15Z | 없음 | 없음 | `budget_clear` 2026-09-23T04:05:22Z | 2 |
| 2 | main | `smoke-nv4-bf-262k-mmp-native` | 2026-09-23T04:06:36Z | 없음 | 없음 | `budget_clear` 2026-09-23T04:15:06Z | 2 |
| 3 | main | `smoke-nv4-bf-262k-mmp-native` | 2026-09-23T04:18:16Z | 1회 · 첫 2026-09-23T05:04:42Z | 없음 | `budget_clear` 2026-09-23T05:40:12Z | 3 |
| 4 | main | `smoke-nv4-bf-262k-mmp-native` | 2026-09-23T06:53:53Z | 1회 · 첫 2026-09-23T07:40:30Z | 없음 | `budget_clear` 2026-09-23T08:12:10Z | 3 |
<!-- /FACT:event_timeline -->

<!-- FACT:measurement_env -->
_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다(evidence.measurement_env_observed · 부재 ≠ 미설정)._
<!-- /FACT:measurement_env -->

> 이 절이 답하는 질문: 수신자가 이 zip 만으로 이 셀을 다시 띄우려면 어떤 순서로 무엇을 하고, 각 단계의 성공을 무엇으로 판정하며, 얼마나 기다려야 하는가?

**재현 표 읽는 법.** install · serve 행의 소요는 `미관측` 이다 — native 평면에는 Docker 의 빌드 · 기동 기록 대신 정문 로그와 상태 파일이 있는데 재현 표 생산자가 그것을 읽지 않았다. 아래 (c) 의 시각이 그 빈칸을 채운다. bench 행의 재구성 명령은 lite 두 줄뿐이다 — 동시성 레벨 레그는 GuideLLM 이며 도구 원문은 03 §3.4 에 있다. 위 표의 lite 명령 두 줄의 `--tokenizer` 는 `<manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4` 자리표시다 — lite bench JSON 의 `tokenizer_id` 는 호스트 경로였고 발행기가 재구성 명령에 manifest 치환표를 적용한다(커밋 `1e78f24`) · 네 manifest 의 해당 필드 값으로 읽는다. 명령 옆 `date 20260923-164047` 은 호스트 KST 시각이다(UTC 07:40:47).

**(a) 사전 준비.**
- **이미지가 먼저다.** 각 노드에 `easy-vllm:0.29.0rc6-cu133-aarch64-source` 가 로컬로 있어야 한다(Docker 로 빌드 — 빌드 패치 60 · 62 가 그 안에 들어간다 · 이 셀의 빌드 절차는 같은 이미지를 쓴 Docker 셀 D1 의 페이로드). 이미지 전송은 금지다 — 노드마다 자기 이미지에서 재포장한다(W10). 승인 선언의 `images[]` 는 이 두 노드의 digest(메인 `a2c4ca49…` · 서브 `87b6a67d…`)만 받는다 — 네 이미지 digest 는 다르므로 **네 재포장 게이트 #1 을 다시 돌려 불일치 목록을 새로 승인**해야 한다(Q6).
- 호스트: Python 3.12(venv 생성) · Docker 엔진(재포장 때 시작하지 않는 컨테이너 create/cp 에만) · NVIDIA 드라이버(호스트 CUDA 툴킷은 쓰지 않는다 — CUDA 사용자 라이브러리는 이미지 번들 `runtime-lib` 을 쓴다 · plan_26092311 N-D2).
- 디스크: run 2 의 메인 기준 venv 약 12.49 GB · wheelhouse 디렉터리 약 10.89 GB(`wheelhouse-verify-main.json` sizes — 바이트 값 환산).
- 가중치: 양 노드의 같은 경로 `<manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4`. native 엔진은 이 호스트 마운트를 `AUTOFS` 로 보고했다(Docker 셀에서는 컨테이너 안에서 CIFS — 보존 로그 · 계보 목록 밖).
- PLE 스테이징: 양 노드 로컬 NVMe 의 같은 자리 `<manifest.ple_mmap_host_path>/q38fn-nvfp4/` · 트리플렛 env 의 `VLLM_PLE_MMAP_DIR` 가 그 자리를 가리킨다.
- 필수 env 의 자리: 트리플렛 `.env.nv4-bf-262k-mmp-native`(`VLLM_PLE_MMAP=1` · `VLLM_PLE_MMAP_DIR` · `CONFIG_FILE` · `SERVING_MODEL_NAME`) + 정문이 주입하는 런타임 env — verify 가 산출한 `LD_LIBRARY_PATH`(venv 의 torch/lib · torch_tensorrt/lib · 이미지 번들 `runtime-lib/cuda/compat/lib.real` · `runtime-lib/lib` · `runtime-lib/cuda/targets/sbsa-linux/lib` 순 — "compat-first(이미지 env 순서)") · `CUDA_HOME` · `TRITON_*` 경로 · `NATIVE_VENV`. NCCL/interconnect env 는 Docker 와 같은 `.env.interconnect` 값을 정문이 Ray 기동에 넘기도록 설계됐다(plan_26092311 §4.2). 위 표(스윕 산출물)에는 env echo 가 없지만(native 스윕의 엔진 로그 자리가 비었다 · Q4), 정문이 보존한 run 2 엔진 로그는 그 값들이 실제로 쓰였음을 기록한다(1.5(5)).

**(b) 설치.** 노드마다 `native-install.sh <run-root>` 순서(1.2). run 2 에서 재포장은 메인 412.8 s · 서브 420.9 s, venv 설치는 약 75 s 였다(`wheelhouse-{build,verify}-*.json`). 성공 판정 = verify 의 게이트 전부 PASS(closure · 설치 집합 == 핀 · pip check ⊆ 선언 · ldd · import/CUDA).

**(c) 기동 순서와 성공 판정.** 정문 `native_multinode_serve.py up --cell nv4-bf-262k-mmp-native --run-id <id>` 가 순서를 소유한다 — 양 노드 RAM 게이트 → 예산 선언/honored → run root(소유 마커) → 재포장 · 설치 → Ray head(메인)/worker(서브) → GPU 2 합류 확인 → `vllm serve`(렌더된 러너) → health 폴링 → 추론 1회 → serve proof → 워치독 · 예산 갱신 루프를 pgid+starttime 모드로 무장. run 2 의 시각(원장 UTC · 엔진 로그는 호스트 KST 라 −9시간 환산):
- 06:53:53Z 메인 예산 선언 · honored(이벤트 표 15·16행) · 서브는 up 로그의 `sub: budget_honored ✓` · 상태 파일 `created_utc` 06:53:50Z.
- 07:11:39Z `watchdog_highrate_hold`(`legacy_rule_would_trip=true` · 사살 없음 · 17행) — 적재 초기에 같은 hold 가 run 1(11행)에도 있었다. 벽이 아니다.
- 가중치 적재 · `Model loading took 39.05 GiB`(서브 07:24:34Z · 메인 07:25:02Z) → 07:26:04Z 부터 `shm_broadcast` 대기 경고 ×13 → 07:37:59Z KV 1,343,310 토큰(요청당 262,144 기준 5.12배) — 보존 로그 `docs/simlog/26092315_native_N1/logs/main-vllm-serve.log`(계보 목록 밖).
- serve proof: READY 1770 s(`ready_after_s`) · health 200 · chat 추론 1회(content_len 141 · finish_reason stop) PASS. PLE mmap 발화 서명(`PLE mmap: layer 1, 128 shards … tp_world 2`)은 보존 로그에 양 rank 로 찍혔다.
- 측정 07:40:31Z → 08:10:39Z · 판정 · 리포트 뒤 `native_multinode_serve.py down --cell … --run-id … --apply` 가 워치독 · 루프 회수 → 로그 · pip freeze · 설치 스크립트 보존 → PID+starttime TERM → 잔재 신선 조회 → run root 완전삭제 → 예산 해제(08:12:10Z `budget_clear`) → attestation PASS(08:12:14Z · 양 노드 run root 부재 · owned 프로세스 0 · GPU 0 · `unknown: []`).
- 기동 시도는 표대로 4이다 — 1·2 는 install 단계 실패(W9 · W10/W11), 3 은 run 1(측정 성립 · down FAIL_CLOSED · W13), 4 가 run 2 다(02 §2.2).

**(d) 호스트 메모리 안전 예산.** 선언(측정 아님): weights 38,852(`(ckpt − ple) ÷ tp` · PLE 48,828 을 mmap 으로 뺌) · kv 20,480 · overhead 24,800 · MemTotal 124,608 → 바닥 40,476 · 워치독 arm 상한 37,404MiB(이벤트 표 15·16행). 실측 나란히: 엔진 `Model loading took 39.05 GiB`(rank 별) · lite 표의 메인 시스템 RAM 79.8 GiB(66%)(03 §3.2 원천 리포트). native 워치독은 컨테이너 이름이 아니라 정문 상태 파일의 pgid+starttime 을 표적으로 한다(plan_26092311 N-D5 · cleanup attestation 의 watchdog 역할 행) — 정문 밖에서 `vllm serve` 를 손으로 띄우면 이 보호가 붙지 않는다.

**(e) 측정 명령.** 레벨 레그는 `sweep_bench.sh --serve-plane native --tool guidellm`(GuideLLM 컨테이너가 호스트 엔드포인트에 붙는다) · 측정 클라이언트는 정문이 만든 래퍼 `bin/vllm-client`(W12). 도구 원문은 03 §3.4.

**(f) 엔진이 스스로 고른 런타임 경로**(보존 로그 · 계보 목록 밖 — Docker 셀 D1 과 같은 줄들이다): FlashInfer all-reduce 는 world_size=2 미지원으로 꺼짐 · custom all-reduce 는 MNNVL 부재로 꺼짐 · NvFp4 MoE `FLASHINFER_CUTLASS` · Fp8 MoE `TRITON` · attention block 1600 토큰 · eager 라 CUDA 그래프 없음. 스윕 meta 의 `moe_backend` 는 native 엔진 로그를 읽지 못해 `NA` 로 남았다(Q4).

## 1.5 서브 레시피 해설
<!-- FACT:sub_recipe -->
_서브 레시피 미관측 — 결손(0.7 결손 표)._
<!-- /FACT:sub_recipe -->

<!-- FACT:measurement_env_nodes -->
_노드별 env 관측 없음 — 01 §1.4 측정 env 관측 표가 비었다._
<!-- /FACT:measurement_env_nodes -->

> 이 절이 답하는 질문: 서브(Ray worker) 노드는 메인과 무엇이 다르고, 무엇을 반드시 같게 맞춰야 하는가?

**(1) 역할 차이.** 메인은 Ray head 와 `vllm serve`(렌더된 러너 · 트리플렛 수령)를, 서브는 Ray worker 만 호스트 프로세스로 띄운다 — 컨테이너도 compose 도 없다. 두 노드 모두 **자기 run root** 에 자기 이미지에서 재포장한 wheelhouse 와 venv 를 갖는다. 기동 · 정리의 주체는 메인의 정문 하나다 — 서브 프로세스도 정문이 원격으로 띄우고 정문의 상태 파일(`identities[]` · node · role · pid · pgid · starttime)로 멈춘다(cleanup attestation 의 `ray-worker`(sub) · `watchdog-ray-worker`(sub) · `budget-renew-sub` 행). 런타임 패치 무장은 양쪽 다 없다.

**(2) 서브에 전달돼야 하는 것(무리별).**
- 도구 · 요청: 정문이 서브에 도구와 요청을 **stdin** 으로 보낸다(W9 — argv 로 실으면 인자 상한을 넘는다).
- 설치 입력: 서브는 메인의 wheelhouse 를 받지 않는다 — 자기 이미지에서 재포장하며, 승인 선언 `images[]` 에 서브 digest 가 있어야 한다(W10).
- 서빙 env: PLE mmap 두 키(`VLLM_PLE_MMAP` · `VLLM_PLE_MMAP_DIR`)와 런타임 env(`LD_LIBRARY_PATH` · `CUDA_HOME` 등 — 서브 venv 경로 기준) · interconnect(NCCL) env — Ray 워커도 자기 rank 의 가중치 · PLE 를 올린다.
- 안전층: 서브 예산 선언 · 워치독 · 예산 갱신 루프(pgid 모드)는 정문이 원격으로 무장한다.

**(3) 선행조건.** 서브에 같은 태그의 로컬 이미지(자기 빌드) · Python 3.12 · 가중치 마운트와 PLE 스테이징이 메인과 **같은 경로** · 같은 드라이버 계열 · 메인 저장소의 도구 배달(N5 · `sync_to_sub.sh --mode experimental --apply` · 오버레이 53/53 — testlog_26092317 §2).

**(4) ABI 동일성.** 이 셀의 사실 블록에는 서브 레시피 · ABI 관측이 없다(1.5 결손). 관측된 것은 다음이다 — 양 노드 pip freeze 가 바이트 동일(320줄 · vLLM `0.29.0rc7.dev0+g74c96922e…` · torch `2.13.0a0+9186a08b2c.nv26.7.59513937` 포함) · 양 노드 wheelhouse build/verify 가 같은 개수(분포 320 · 휠 319 · 재포장 317 · 승인 외 불일치 0)로 PASS(소요 · 바이트 크기는 노드마다 조금 다르다) · 드라이버는 계보 문서가 양 노드 580.178.04 로 적었다(testlog_26092317 유효맥락). 이미지 digest 는 노드마다 다르다(메인 `a2c4ca49…` · 서브 `87b6a67d…` — 승인 선언) — 로컬 빌드라 정상이며, 같아야 하는 것은 digest 가 아니라 ABI 다. Docker 셀처럼 빌드 원장 identity 를 두 노드에서 대조한 attestation 은 이 셀에 없다.

**(5) 측정 당시 인터커넥트 실효값.** 스윕 산출물에는 없다 — native 스윕의 엔진 로그 자리가 비어 측정 env echo 가 수집되지 않았다(01 §1.4 표 · Q4). 대신 정문이 보존한 run 2 엔진 로그(`docs/simlog/26092315_native_N1/logs/main-vllm-serve.log` · 계보 목록 밖 · 전체 로그라 꼬리 캡처가 아니다)가 양 노드 줄로 `NCCL_SOCKET_IFNAME=<nic:cluster>` · `NCCL_IB_DISABLE=1` · `NCCL_DMABUF_ENABLE=0` · `NCCL_CROSS_NIC=1` · 네트워크 `Socket` · NCCL `2.30.7+cuda13.3` 을 기록했다(sub L68-L94 · main L538-L563). Docker 셀과 다른 점 하나: Docker 측정 로그에 있던 `NCCL_NET_PLUGIN=spcx` echo 가 없고 `NET/Plugin: Could not find: libnccl-net.so`(L73 · L543)가 찍혔다 — 두 평면 모두 네트워크는 `Socket` 이며 이 차이가 decode 에 주는 영향은 잰 적이 없다.
