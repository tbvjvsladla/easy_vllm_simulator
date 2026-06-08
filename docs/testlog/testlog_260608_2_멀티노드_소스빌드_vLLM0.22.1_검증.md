# testlog 260608-2 — 멀티노드 소스빌드 vLLM 0.22.1 검증 (2노드 Ray)

## 목적
단일노드에서 검증된 Phase 2 소스빌드(0.22.1/NGC 26.03, docs/testlog/testlog_260608_1)를 **멀티노드(2노드 DGX Spark)로
확장**해 2노드 Ray 분산 서빙을 검증. master=메인(192.168.100.10, spark-a73e) / slave=서브(192.168.100.11, spark-bdc9).

## S1 resolve (결정론적)
- 0.22.1 → torch **2.11.0** (`resolve_torch_pin.py`, pyproject [build-system].requires) → NGC **26.03-py3**(prefix 매칭, 단일노드 검증값).
- CUDA 13.2, CPU_ARCH=aarch64(양노드 GB10), TORCH_CUDA_ARCH_LIST=12.1a(SM121).
- **ray 핀 ==2.48.0**: vLLM `requirements/test/cuda.txt` 기준. 멀티노드 분산(serve_runner `ray start`) 필수이나
  vLLM 코어 dep 아님(`pip install -e .` 로 안 들어옴) → Dockerfile.source-build 에서 명시 설치.
- 모델: gpt-oss-120b-MXFP4 (TP=2, distributed-executor-backend=ray). NAS 존재 확인(no-download).

## S2 patch (multi-node 브랜치)
- **`Dockerfile.source-build`(멀티노드 신규)** = 단일노드 레시피 + 멀티노드 추가분:
  - apt 머지: ccache + **netcat-openbsd**(slave 의 `nc -z` head 대기) + iproute2/iputils-ping/dnsutils(보존 규칙).
  - `RUN pip install ray==2.48.0`.
  - 본체 동일: constraint 비우기 → clone v0.22.1 → use_existing_torch → build-system.requires 설치 → **strip-hoist** → `pip install --no-build-isolation -e .`.
- **`docker-compose.yaml`**: `x-gpu-common.build.dockerfile: Dockerfile → Dockerfile.source-build`(1줄, 전 프로파일). prebuilt `Dockerfile`(0.18.0 앵커)은 보존.
- `sync_to_sub.sh`(빌딩블럭) 견고화: 체크섬에 `Dockerfile.source-build` 포함 + 로컬 부재 파일 skip.

## S2.5 sync → 서브
- `sync_to_sub.sh` dry-run 확인 → `--apply`. 체크섬 일치(Dockerfile.source-build 포함 4종). 서브 잔여 `gpt-oss-20b-normal.yaml` --delete 정합 제거.

## S3 smoke — 1차 실패 → 진단(Model-C) → 재실행 통과
**1차** `multinode_serve_smoke.sh gpt-oss-120b-MXFP4 --build`:
- 양노드 병렬 소스빌드 **OK**(ABI 벽 통과 — 멀티노드 확장 성공). `pip install -e .` 가 런타임 deps 해소(requirements.txt 불요 확인).
- 서빙 단계 master EXITED: `Engine core initialization failed`. 스모크가 tail+down 해 근본원인 가림 → `--keep-up` 재기동으로 전체 로그 확보.
- **근본 원인**(EngineCore 트레이스백, ray_executor_v2 _init_executor → worker init_device → request_memory):
  `ValueError: Free memory on device cuda:0 (73.5/121.69 GiB) on startup is less than desired GPU memory utilization (0.75, 91.27 GiB)`.
  → master 노드에 **단일노드 Phase 2 잔여 인터랙티브 컨테이너 `vllm-srcbuild`(Up 13h)** 가 살아있는 `VLLM::EngineCore` 로 **~36 GiB 점유**.
  (DGX Spark 128GB 통합메모리 공유.) 서브 노드는 클린. NCCL/RDMA·Ray 2노드 클러스터·TP=2 토폴로지는 **모두 정상 통과**.
  `triton_kernels.matmul_ogs` import 에러는 단일노드(gpt-oss-20b MXFP4)와 동일하게 MARLIN 폴백이라 **비치명적**.
- 분류: 소스빌드/멀티노드 설정 버그 아님 = **환경 오염**(직전 devlog "정리/보존 미결" 잔여물). 설정(0.75)은 0.18.0 검증값이라 우회(util 낮추기) 부적절.

**조치**: `docker rm -f vllm-srcbuild`(단일노드 검증 완료분 종료) → GPU 점유 0 확인.

**재실행** `multinode_serve_smoke.sh gpt-oss-120b-MXFP4`(재빌드 없이, 자동 down):
- NAS 체크 → master(Ray head)+slave(Ray worker) → 엔드포인트 :9000 health **READY ~320s** →
  **SMOKE PASS content='4' finish_reason=stop**(reasoning 정상) → 양노드 자동 down → exit 0. ✅

## 결과
- **멀티노드 0.22.1 소스빌드 = 통과.** 양노드 소스빌드 + 2노드 Ray 분산 서빙 + 추론 스모크('4') 모두 정상.
- 코드/설정은 처음부터 정상(재빌드 없이 잔여물 정리만으로 통과) — 멀티노드 소스빌드 확장 자체에 결함 없음.

## 교훈
- **검증/스모크 완료 후 기동 컨테이너 종료**(사용자 규칙). 안 하면 통합메모리 점유가 다음 검증을 OOM-위장 실패시킴.
- 멀티노드 OOM 진단은 master 로그를 `--keep-up`으로 보존해 EngineCore/worker 트레이스백 말미(실제 ValueError)까지 봐야 함(APIServer "Engine core initialization failed"는 다운스트림).
- 멀티노드 소스빌드 deps = `pip install -e .`(런타임 deps) + ray 명시. prebuilt 경로의 wheel-METADATA requirements.txt 불요.
- gpt-oss MXFP4 의 triton_kernels 미존재는 MARLIN 폴백(비치명적) — 단일/멀티노드 동일.
