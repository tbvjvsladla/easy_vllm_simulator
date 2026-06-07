# testlog 260607-2 — vLLM 0.22.1 dry-run → 빌드 → 스모크 → torch 2.11 ABI 한계 규명

## 목적
부트스트랩 Step 3(단일노드 검증). `/upstream-version-watch` 스킬로 vLLM 0.21.0 → **0.22.1** 전파를
dry-run하고, 실제 빌드 + 단일노드 스모크(gpt-oss-20b)로 검증. 결과적으로 **prebuilt wheel + NGC torch
2.11 alpha 조합의 근본 한계**를 규명함.

## 1. Dry-run (결정론적 해소, 스킬 검증) — ✅ 성공
- ① torch 핀: 0.22.1 → `torch==2.11.0`
- ② NGC 태그: 프로빙이 최신 26.04/26.05(torch **2.12.0**)를 거부하고 torch 2.11.0 매칭 **26.03-py3** 선택
- ③ wheel: `cu129 / manylinux_2_28` (0.21.0의 manylinux_2_34와 다름 → **휘발성**, 자산 실재 검증으로 404 방지)
- ④ deps delta: flashinfer-python/cubin 0.6.8→0.6.11.post2, llguidance <1.4→>=1.7,<1.8, nvidia-cutlass-dsl 4.4.2→4.5.2, +humming-kernels==0.1.2, +safetensors>=0.6.2

## 2. 패턴 A — wheel 휘발성 ARG 격리 (Dockerfile 개선)
`manylinux` 하드코딩 → `ARG VLLM_MANYLINUX`로 격리. 트리거=vLLM 버전만, cuda/manylinux는 resolve_wheel.py 자동 도출.
검증: 패턴이 만든 URL == resolve_wheel.py 검증 URL (0.21.0/0.22.1 모두 일치).

## 3. 빌드 → 의존성 정합 (컨테이너 `--use-installed`) — ✅
- 1차 빌드 exit 0이나 pip resolver 충돌 경고. 컨테이너 내부 판정으로 **진짜 2건** 식별:
  - `diskcache` 미설치 → vLLM 하드 의존(common.txt) → 추가 `diskcache==5.6.3`
  - `compressed-tensors` 0.17.0 vs vLLM `==0.15.0.1` → 핀 교정
- 나머지(torch/torchvision/torchaudio/setuptools)는 nv 접미어 = **base 제공, 무시**(devlog §6.1).
- 근본원인 분석: 호스트 모드 dry-run이 못 잡은 이유 = ① base 제공 vs 진짜 누락 구분 못 함 ② 존재만 보고 스펙 비교 안 함. (분석 가능 결함, 암묵지 아님)

## 4. 스모크(서빙) — ❌ 이슈 체인 발견 (스모크가 최종 중재자)

| # | 이슈 | 성격 | 결과 |
|---|---|---|---|
| 1 | manylinux 휘발성 | 패턴 | ✅ ARG 격리 |
| 2 | python deps(diskcache/compressed-tensors) | requirements | ✅ 보정 |
| 3 | torch **파이썬** API: `register_opaque_type(hoist=True)` | 소스 패치 | ✅ `HAS_OPAQUE_TYPE=False` 조건부 패치 |
| 4 | `libcudart.so.12` 부재 (cu129 wheel ↔ CUDA13 base) | requirements+ENV | ✅ nvidia-cuda-runtime-cu12 + LD_LIBRARY_PATH |
| 5 | torch **C++ ABI** 심볼 `c10::cuda::CUDAStream::query()` | **컴파일 바이너리** | ❌ **하드 월** |

- #5 증거: `_C.abi3.so`가 `_ZNK3c104cuda10CUDAStream5queryEv`를 `U`(외부 요구). NGC torch `libc10_cuda.so`엔
  `CUDAStream::stream()`만 있고 `query()` **0개**. vLLM은 `_C`를 무조건 import(cuda.py:21), env 스위치 없음.
  `_C_stable_libtorch`(stable ABI)만으론 커스텀 op 미등록(torch.ops._C 비어있음) → 서빙 불가.

## 5. Upstream 확인 (GitHub 이슈) — 알려진·미수정 조건
- **#38431** "torch 2.11 is not supported" (Closed)
- **#43435** vllm 0.21 + cu129 + libcudart 불일치, nvidia-cuda-runtime-cu12 깔아도 실패 → **Closed as "not planned"**
- **#36302 / #13608** `_C.abi3.so: undefined symbol _ZN3c104cuda...` (torch↔vLLM 빌드 불일치류)
- **#31424** 공식 `nvcr.io/nvidia/vllm` 컨테이너 구버전 lag
- vLLM 공식 문서: "wheel은 PUBLIC torch에 빌드되어 같은 버전 다른 빌드구성과도 binary incompat. 기존 PyTorch엔 소스 빌드."

## 6. 결론
- **vLLM 0.21~0.22(torch 2.11 핀) prebuilt wheel을 현재 NGC pytorch(torch 2.11 *alpha*)에 얹는 건 지금 깨진다.**
  진짜 벽은 CUDA가 아니라 **torch 2.11 alpha의 C++ ABI 불안정성** ↔ prebuilt `_C`. 알려진 upstream 조건(미수정).
- prebuilt 방법론은 **torch ABI 매칭 시(2.10 era)에만 유효**. 불일치 시 길은: **소스 빌드**(공식 권장) / 공식 vLLM 컨테이너(lag) / torch 2.11 stable 대기.
- **기존 0.21/0.22 조합도 "지금 재빌드하면" #3에서 동일 실패**(이미 뜬 옛 이미지는 동작).

## 7. 스킬 관점 수확
- resolve 체인·의존성 정합·manylinux 패턴·패치 메커니즘 전부 작동. **스모크 게이트가 빌드·deps·파이썬패치를 통과한 ABI 월까지 잡아냄** → "스모크=최종 중재자" 입증.
- 후속: 스킬 워크플로에 "prebuilt 스모크 실패 → 소스빌드 폴백 분기" 반영. upstream 이슈 선확인 습관화.

## 8. 다음
참조 레포 https://github.com/bjk110/spark_vllm_docker 가 이 문제(DGX Spark + 최신 vLLM)를 어떻게 푸는지 조사 → 소스빌드/패치 전략 도출.
