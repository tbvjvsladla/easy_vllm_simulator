# plan_26062818 — Route B: jasl-fork SM12x 이미지 빌드 → 공식 MXFP4 DeepSeek-V4-Flash 2× GB10 Ray TP=2 서빙

> 브랜치: `multi-node` · 통로: `output/multi/` · 트리거: 사람 지시 "B안 수행"(수동 핀 정책)
> 근거: 조사 워크플로 5개 probe(A1–A5)+종합 + **포크 소스 직독(primary-source) R1 해소** · 선행 `plan_26062811_03_11`(DS4 서빙)·`plan_26062812`(3+1+1) · MEMORY(397B-int4 2노드 infeasible 선례)
> 상태: **HITL 게이트 ① 대기**(빌드 미착수). 양 노드 idle·watchdog 가동·전부 미커밋.

---

## 0. 한 줄 요지 (가장 먼저)

stock `0.23.0-source` 로 6회 연속 MARLIN-repack→통합메모리 OOM→호스트 하드다운한 공식 MXFP4 DeepSeek-V4-Flash 를,
**jasl/vllm SM12x 포크(`c766cbc6`)를 우리 NGC-torch 소스빌드 하네스로 빌드한 새 아치-트랙 이미지** `…-source-sm12x` 로 서빙한다.
**성패를 가르는 단 하나의 값**: 포크 소스 직독으로 확인했듯 **포크에서도 `--moe-backend auto` 는 sm_121 서 여전히 MARLIN(repack→OOM)을 고른다.** 비-repack 경로(HUMMING)는 **명시적 `--moe-backend humming` 으로만** 도달한다. ∴ 레시피의 핵심 = `--moe-backend humming`.

---

## 1. 핀 (결정론 · 재유도 금지 · resolved.json 기입 대상)

| 키 | 값 | 검증 |
|---|---|---|
| `VLLM_REPO` | `https://github.com/jasl/vllm.git` | A1·A3 |
| `VLLM_REF` (SHA) | **`c766cbc6fff2946dc81a2f7d4dbf6f4cd39c242b`** | `git ls-remote` 직접확인 = 태그 `sm120-pr-41834-stable-preview-20260626` |
| PR | #41834 "Add SM12x support for DeepSeek V4 Flash" — open, force-push됨 → **태그/브랜치명 금지, SHA로만 핀** | A1·A3 |
| 우리 stock 빌드 실제 SHA | `0fc695fc6` (0.23.1.dev0, v0.23.0 태그서 몇 커밋 앞) — 포크와 **diverged ahead 862 / behind 6** | A2 측정 |
| NGC 베이스 | `nvcr.io/nvidia/pytorch:26.05-py3` (불변) | — |
| `TORCH_CUDA_ARCH` | `12.1a` (sm_121) | — |
| 이미지 태그 | **`easy-vllm:0.23.0-cu132-aarch64-source-sm12x`** | §2.2 |
| 체크포인트 | 공식 `deepseek-ai/DeepSeek-V4-Flash` MXFP4 149GB(46 shard) — **NAS 존재 확인** `<nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash` (경로 정본 = manifest `nas_model_path`) | 본세션 ls |
| NCCL | NGC 26.05 = **2.30.4 (이미지 내장)** → LD_PRELOAD/패치 **불요** | A4 측정 |
| DeepGEMM | `deepseek-ai/DeepGEMM@891d57b4…` (기존 `10-deepgemm.sh` 핀 유지) | — |

---

## 2. 중앙 결정

### 2.1 fork-clone (NOT build-patch-port) — 만장일치 mandatory

- **stock 0.23.0 은 2중 하드월**(A2 이미지 직독): ① sparse-MLA 어텐션 `capability.major in [9,10]` → major 12 경로 부재 ② MXFP4 MoE 오라클 sm_121 auto = `[TRTLLM(SM100), DEEPGEMM(SM100), MARLIN, BATCHED_MARLIN]` → MARLIN 강제. **둘 다 토큰 1개 전에 죽인다.** 우리 `20/30-` 패치는 gpt-oss 게이트라 DS4 를 못 고친다.
- **patch-port 불가**(A1): PR 이 자기 base 에도 `mergeable:false` · 862 커밋 diverged · `sparse_mla_kernels.py` +3521줄 신규 = 재구현. `git apply` 가 hot-path 다수서 reject. 커뮤니티 검증도 포크-트리 기준.
- **포크가 어텐션 월을 해소**: `DeepseekV4FlashInferSM120Attention` 클래스 in-tree(`nvidia/model.py:65`) → sm120 sparse-MLA 경로 존재.

### 2.2 이미지-네이밍 불변식 화해 — 새 아치-트랙 `source-sm12x`

```
easy-vllm:0.23.0-cu132-aarch64-source-sm12x
```
- `{track}=source-sm12x` 는 **GPU-아치 소스 lineage**(wheel↔source 와 같은 종류 구분)이지 **모델-키잉이 아니다**(태그에 모델명 없음). 이미지는 **superset**(기존 모든 모델 + SM12x DeepSeek-V4, 제거 0) → "한 이미지가 모든 모델 서빙" 보존.
- `{vllm}=0.23.0` 은 config 앵커로 유지 · **정확 포크 SHA `c766cbc6` 는 Dockerfile ARG + resolved.json + testlog 에 기록**(확률론 추론 금지).
- **거버넌스 = workflow.md S3 "NGC 베이스 오버라이드" 와 동일 HITL 핀-오버라이드** 메커니즘을 vLLM repo 축으로 확장(참조-그라운디드·증거기반·testlog-기록·HITL승인 → resolved.json).
- **규율**: 단 하나의 `source-sm12x` 트랙이 GB10 서 stock-source 를 supersede. `-source-ds4` 등 모델-키잉 난립 금지. **승격 전 기존모델 회귀 재스모크 필수**(862 divergence — §6 승격게이트).

---

## 3. R1 해소 — 비-repack MoE 경로 (primary-source, 본 plan 의 최대 성과)

조사 종합은 R1(포크 DS4 MXFP4 가 비-repack 인가)을 **#1 미검증 치명값**으로 남겼다. 본세션이 포크 소스를 직독해 **확정**:

| 경로 | 트리거 | sm_121 가용 | repack | 결론 |
|---|---|---|---|---|
| DeepGEMM MegaMoE (`DeepseekV4MegaMoEExperts`) | `--kernel-config moe_backend=deep_gemm_mega_moe` | **✗** `NotImplementedError("DeepGEMM MegaMoE requires SM100 GPUs.")` (`nvidia/model.py:303`) | — | SM100 전용, **우리 ✗** |
| FusedMoE auto 오라클 | `--moe-backend auto`(기본) | ✓ but → **MARLIN** (`oracle/mxfp4.py:336` 우선순위 `[TRTLLM,DEEPGEMM,MARLIN,BATCHED_MARLIN]`) | **예(~37GiB 트랜지언트)** | **OOM = serve#1-6 재현** |
| **HUMMING** (fused grouped MoE) | **`--moe-backend humming`** | **✓** (`fused_humming_moe.py:is_supported_config` 에 **arch 하드월 없음** — activation_format+gemm_type 만 체크) | **아니오** | **✅ 우리 경로** |

**근거 체인**: `quant_config.py:152` → fp4·non-NVFP4 → `Mxfp4MoEMethod` → `select_deepseek_v4_mxfp4_moe_backend`(`oracle/mxfp4.py:564`) → `config.moe_backend != "auto"` 면 명시 backend 존중(:580) · `map_mxfp4_backend("humming")→[HUMMING]`(:283) · `_get_priority_backends()` auto 리스트엔 HUMMING **부재**(:336).

⟹ **레시피 필수: `--moe-backend humming`** (+ `VLLM_HUMMING_MOE_GEMM_TYPE`). 이는 기존 `deepseek-v4-flash.yaml` 의 "`--moe-backend` 미지정→auto · triton 강제 금지(122B 교훈)" 주석을 **명시적으로 뒤집는다**(122B=W4A4 NVFP4 교훈 ≠ DS4 MXFP4 fork-humming).

**남은 미검증(S1·스모크 중재)**: ① `VLLM_HUMMING_MOE_GEMM_TYPE` 기본값(indexed/grouped/auto — `humming_utils.py`) ② HUMMING `process_weights_after_loading` 이 자체 대형 repack 을 안 하는지(설계상 block-scaled 직소비 — 추정, 스모크 측정) ③ sm120 sparse-MLA 어텐션이 auto-활성인지 `VLLM_DEEPSEEK_V4_FLASHINFER_SM120_DECODE/PREFILL`(기본 False) 명시 필요인지.

---

## 4. 절차 (workflow S1–S4)

### S1 resolve — 포크 핀 + R1 잔여 플래그 확정 (HITL 게이트 ①)
1. `resolved.json` 에 §1 값 기입(`VLLM_REPO`/`VLLM_REF` 신규 키 + 트랙 `source-sm12x`).
2. **빌드 전 reference-grounded 확정**(정확도>토큰): 포크 `c766cbc6` 에서 ⓐ `humming_utils.get_humming_moe_gemm_type` 기본·env ⓑ `nvidia/model.py` 의 sm120 어텐션 선택(auto vs `VLLM_DEEPSEEK_V4_FLASHINFER_SM120_*` 게이트) ⓒ `use_existing_torch.py` 존재 ⓓ 가장 권위있는 커뮤니티 launch 스크립트(al-engr/lmxxf/hazyumps)의 **정확 flag+env 세트** 대조 → humming 경로 확정 + 보조 env 목록 고정.
3. **verify**: §1 표 + ⓐ–ⓓ 결과 출력 → 사람이 포크핀·트랙명·humming 레시피 확인.

### S2 patch — Dockerfile + build_patches + recipe + env (HITL 게이트 ②)

**(A) `output/multi/Dockerfile.source-build`**:
- L21 위 신규 `ARG VLLM_REPO=https://github.com/vllm-project/vllm.git`.
- L38 교체(raw SHA 는 ref-name 아님 → shallow `--branch` 불가; force-push 면역 위해 검증):
  ```dockerfile
  RUN git clone --filter=blob:none ${VLLM_REPO} /workspace/vllm-src \
   && git -C /workspace/vllm-src checkout ${VLLM_REF} \
   && test "$(git -C /workspace/vllm-src rev-parse HEAD)" = "${VLLM_REF}"
  ```
- `render_dockerfile.py` 가 resolved.json 에서 ARG 기본+빌드태그(`-source-sm12x`) emit. L19 torch가드 메시지·strip-hoist(L47-72)는 포크도 동일 NGC torch 라 유지(self-adapting).

**(B) `output/multi/build_patches/`**:
- **보조패치 신규 추가 금지** — victor.euler GPU-cache fix·hazyumps indexer 폴백(`sm12x_deep_gemm_fallbacks.py`)은 **`c766cbc6` 에 이미 in-tree**(A3 + 본세션 트리확인 `nvidia/ops/sm12x_deep_gemm_fallbacks.py`). 재추가 시 non-idempotent → 빌드 깨짐.
- `10-deepgemm.sh` 유지(DSA indexer 런타임 DeepGEMM). `20/30-` 유지(sm12x 이미지의 **gpt-oss-120b 회귀용** — DS4 MoE 는 §3 humming 이 지배, 20/30 이 DS4 를 고친다 가정 ✗). 각 패치 fail-loud probe 가 포크 트리서 문자열 이동 시 의도적 FAIL(맹목 carry 차단).

**(C) `output/multi/configs/deepseek-v4-flash.yaml` — Phase-A first-light**:
```yaml
model: /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash
tensor-parallel-size: 2
distributed-executor-backend: ray
moe-backend: humming                # ★ R1 — 비-repack. auto=MARLIN-repack-OOM 금지(§3)
enable-expert-parallel: true        # 256 routed experts 분산 (al-engr·hazyumps)
gpu-memory-utilization: 0.80        # 보수 first-light(통합메모리 cap≤0.90, weights floor~0.59 위)
kv-cache-dtype: fp8                  # 사용자 지시 유지
block-size: 256                     # 커뮤니티 공통(MLA/DeepGEMM 정렬)
enforce-eager: true                 # Phase-A(Ray compile-hang 선례). Phase-B서 drop+cudagraph
max-model-len: 32768                # 보수. 로드 성공시 Phase-B ramp(→131072→393216)
max-num-seqs: 1                     # 완성증명 우선. Phase-B ramp
max-num-batched-tokens: 4096        # first-light
enable-prefix-caching: true
enable-chunked-prefill: true
disable-custom-all-reduce: true     # cross-node RoCE(NVLink 부재)
# kv-cache-memory-bytes: <S3 측정>  ← startup-log "GPU KV cache" 측정 후 절대클램프(이식성)
```
`.sh`: Phase-A 는 `exec vllm serve --config …` 유지(파서 add-on 없음). **MTP/cudagraph/reasoning·tool 파서 = Phase-B**(완성증명 후). `.sh` echo "max-len=65536" 오기 정정.

**(D) env-file**:
- `.env.deepseek-v4-flash`: `IMAGE_TAG=easy-vllm:0.23.0-cu132-aarch64-source-sm12x`. `BUILD_DOCKERFILE=Dockerfile.source-build`(동일파일 ARG 분기). **`VLLM_HUMMING_MOE_GEMM_TYPE`(S1 확정값) 추가**. 기타(`MAX_JOBS`·`PYTORCH_CUDA_ALLOC_CONF`·`RAY_OBJECT_STORE_MEMORY`) 유지.
- `.env.cluster`(Band2·manifest-rendered·손수정 금지): NCCL 2.30.4 내장 → LD_PRELOAD 불요. 런타임 JIT용 `TORCH_CUDA_ARCH_LIST=12.1a` 프리셋 추가 검토(§5 R10).
- `.env.interconnect`(Band2): as-is(2.30.4).

**verify**: 변경 라인이 S1 해소값 직결(Karpathy B3). 양노드 diff 사람검토.

### S2.5 sync — 메인→서브 하향 (멀티 전용)
- `scripts/sync_to_sub.sh`(dry-run→`--apply`): `build_patches/`·`Dockerfile.source-build`·`serve_runner.sh`·`.env.cluster`·`.env.interconnect` 배달. **모델 트리플렛(`deepseek-v4-flash.{yaml,sh}`·`.env.deepseek-v4-flash`)=Band3 직접전파 금지**. **슬레이브=Band2-only Ray worker**(`.env.cluster`+`.env.interconnect` 만, `CONFIG_FILE` 미요구). 서브 dirty=fail-closed.
- **주의**: `--moe-backend humming` 은 마스터 yaml(Band3)에 — 슬레이브 워커도 MoE expert 실행하나 Ray 가 executor 설정을 워커에 전파(미검증 → §5 R10).

### S3 smoke — NAS체크 + 2노드 빌드 + Ray 서빙 (HITL 게이트 ③)
1. `check_smoke_model.py deepseek-v4-flash --topology multi`(149GB 존재 전제, 부재→중단·다운로드 금지). 폐쇄망 DeepGEMM 미러 사전확인.
2. **watchdog 임계 재조정**(serve 前 양노드 `mem_watchdog.sh master <T> &`·`… slave <T> &`): gmu 0.80 healthy free ~13GiB → **`T≈4096`**(4GiB, 하드다운 절벽 0.5–1.4GiB 위·false-trip 없음). **스크립트 주석 "20–30GiB floor" 가정은 거짓 → 정정.** (watchdog=느린압력 backstop; fast MARLIN repack 은 못잡음 → primary 방어는 §3 humming + 게이트4.)
3. `bash scripts/multinode_serve_smoke.sh deepseek-v4-flash --build`: NAS체크→양노드 병렬빌드(`VLLM_REPO/REF`=포크)→master `--profile master`+ssh slave `--profile slave` Ray클러스터→`:8940/health` http200 게이트(master "startup complete" grep 금지=거짓양성)→master 추론. Ray 가 join → `--headless`/`--nnodes`/`--node-rank`/`--master-addr` **전부 없음**(R9).
4. **MoE 백엔드 로그 게이트(serve#1-6 호스트다운 회피)**: master 로그서 선택 백엔드 grep → **MARLIN 으로 떨어졌으면 weight-load 진입 전 abort**(humming 미적용 신호 = 미완결). HUMMING 확인 후에만 진행.
5. **KV 절대클램프 측정**: clean load 후 startup-log `GPU KV cache` GiB → `--kv-cache-memory-bytes`(절대) + `gpu-memory-utilization` **둘 다 emit**(이식성=clamp·startup게이트=gmu≤0.90). 공식 batch 금지(near-max=측정).
6. 실패 → `classify_failure.py`: OOM/NCCL-RDMA/Ray-timeout→Model-C(HITL). humming-적용 OOM 이면 **397B-int4 선례처럼 2노드 infeasible 가능성** 정직 평가 → Route C(NVFP4, 다운로드 완료시)/Model-C 에스컬레이트.

### S4 commit — 스모크 통과분만 last-good (HITL 게이트 ④)
- multi-node 로컬 커밋(브랜치핀 독립) + `git tag last-good-multi-node`. 산출물 통로 `output/multi/` 만(껍데기 추적·생성물 비추적). resolved.json 오버라이드·새 트랙을 devlog/testlog 기록.
- 서브 전파=S2.5. single↔multi 공유블럭 동기화=`.claude/skills/upstream-version-watch/scripts/sync_branches.sh`(작업종료 후 사람질의; owner-path relocation 반영). **keep-assets 정본화**: `mem_watchdog.sh`(검증된 안전망)·`10-deepgemm.sh`.

---

## 5. 리스크 / 충돌 / 미검증 레지스트리

| # | 리스크 | 해소/조치 | 심각도 |
|---|---|---|---|
| **R1** | ~~포크 DS4 MXFP4 가 비-repack 인가~~ → **§3서 RESOLVED**: 비-repack=HUMMING(`--moe-backend humming`); auto/MegaMoE 는 sm_121 부적합 | 레시피 `moe-backend: humming` 확정. 잔여(gemm_type 기본·HUMMING repack 여부·sm120 어텐션 게이트)=S1+스모크 | ~~치명~~→중(경로확정, 실측 잔존) |
| **R1b** | **HUMMING 도 TP=2 서 통합메모리 적합한가** — 정직: HUMMING 가 자체 repack/대형버퍼 시 또 OOM 가능 | 게이트4 로그-abort(weight-load 전) + watchdog. al-engr 가 TP=2 fork-MXFP4 서빙성공=존재증명. 불가시 Route C/Model-C | 높 |
| R2 | build_patches 20/30 운명 | gpt-oss 회귀용 유지, DS4 는 §3 지배. 포크서 fail-loud probe 가 게이트변경 알림 | 중 |
| R3 | `10-deepgemm.sh` sm_120 실제실행 vs import-게이트 | 유지(belt-and-suspenders, DSA indexer 런타임 요구). 스모크 중재 | 중 |
| R4 | 보조패치 추가? | **추가 금지** — `c766cbc6` in-tree(non-idempotent, 추가시 빌드깨짐) | 중 |
| R5 | recipe first-light 발산(A4 vs A5) | §4(C)로 화해(gmu0.80·eager·MTP-off·tokens4096). Phase-B ladder | 낮 |
| R6 | watchdog 10240 false-trip | →`T≈4096`@0.80(§4-S3.2). 주석 정정 | 중 |
| R10 | humming 백엔드가 Ray 슬레이브 워커에 전파되나(executor env propagation 미검증) | 마스터 yaml 설정 → Ray executor 전파 가정. 미전파시 `.env.cluster` 프리셋(Band2)로 보강(거버넌스 HITL — manifest-render 손수정금지) | 중 |
| R11 | 포크 버전문자열/트랙명 | `{vllm}=0.23.0` 앵커·`{track}=source-sm12x`·SHA=ARG/resolved/testlog. 새 트랙=HITL 비준 | 낮 |

**negative-honesty 승계(조사)**: A3 는 포크파일 **독해만**(빌드·실행 ✗) → "in-tree ⇒ 실서빙" 미증명. 본세션 §3 도 **정적 트레이스**(실행 ✗) → 스모크가 최종 중재자. al-engr/hazyumps `start_*.sh` 본문 일부 404(lmxxf 의존). Ray env-propagation·HUMMING repack 여부·DSA indexer 버퍼크기 = 미측정.

---

## 6. 검증 / 합격 기준

- **1차 합격(본 plan)**: `multinode_serve_smoke.sh deepseek-v4-flash --build` → `:8940/health` http200 + master 엔드포인트 **프롬프트 1회 → 비어있지 않은 완성 1회**. master 로그 **HUMMING 선택 확인**(MARLIN 아님). 양노드 watchdog 무트립·호스트 무하드다운.
- **증거 emit**: gmu + 측정 `kv-cache-memory-bytes`(절대). startup-log MoE 백엔드 라인. simlog/testlog.
- **승격 게이트(별도 HITL, 본 plan 외)**: sm12x 이미지가 GB10 default 로 stock-source supersede 하려면 **gpt-oss-120b·Qwen3-Next-80B·gemma 회귀 재스모크 통과**(862 divergence).
- **smoke-before-commit**: 로컬 빌드+스모크 통과분만 last-good.

---

## 7. HITL 승인 필요 (게이트별)

- **게이트 ①(S1)**: 포크핀 오버라이드(`VLLM_REPO=jasl/vllm`·`VLLM_REF=c766cbc6…`) resolved.json 기입 · 새 트랙 `source-sm12x` 비준(R11) · **humming 레시피 + 잔여 env(S1 확정) 승인**.
- **게이트 ②(S2)**: Dockerfile diff(blobless+SHA verify) · build_patches 결정(20/30 유지·보조 무추가·신규 0) · recipe Phase-A(`moe-backend: humming`) · R10 Band2 거버넌스.
- **게이트 ③(S3)**: 스모크+classify+risk-memo · MoE 로그-abort 판단 · 회귀 승격게이트.
- **게이트 ④(S4)**: 최종 커밋+서브전파 · `sync_branches.sh` 질의.
- **모델/안전**: 149GB MXFP4 NAS 존재 확인(부재시 무인 다운로드 금지). 폐쇄망 DeepGEMM 사전미러.
- **판단계층(사전-codify 금지)**: build_patches 포크 재유도(R2)는 S3 Model-C 판단계층 — 무증거 행동 금지.

**핵심 한 줄(재): 포크-클론은 mandatory, 그러나 `--moe-backend humming` 없이는 포크에서도 MARLIN-repack-OOM 으로 죽는다. humming 이 본 plan 의 성패다.**
