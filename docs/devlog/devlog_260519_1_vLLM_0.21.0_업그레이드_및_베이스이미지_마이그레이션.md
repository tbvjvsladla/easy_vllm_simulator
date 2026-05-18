# devlog 260519 — vLLM 0.21.0 업그레이드 및 베이스 이미지 마이그레이션

## 작업 요약

기존 vLLM 0.19.1 환경에서 최신 stable인 0.21.0으로 업그레이드를 진행했다.
단순 버전 핀 변경으로 끝날 줄 알았으나, **PyTorch exact pin → 베이스 이미지 강제 교체 → wheel URL 404 → CUDA 메이저 버전 불일치 → requirements.txt 전면 재검토**까지 연쇄적으로 문제가 발생했다.
최종적으로 `pytorch:26.01-py3` → `pytorch:26.03-py3` 베이스 교체, CUDA Forward Compatibility 활용, 9개 패키지 추가/수정으로 gpt-oss-120b 서빙 성공까지 완료.

---

## 1. 발단: 0.21.0의 새 기능 도입 필요성

vLLM 0.21.0의 주요 변경 사항을 확인하면서, DGX Spark 멀티노드 환경에 직접 도움이 되는 항목들을 발견.

| 항목 | 가치 |
|------|------|
| NVFP4 KV 캐시 지원 | 보유 중인 `Qwen3.5-122B-A10B-NVFP4`, `Qwen3.5-397B-A17B-NVFP4` 메모리 절감 |
| MXFP4 백엔드 개선 | `gpt-oss-120b-MXFP4` 성능 향상 |
| NIXL 커넥터 / Disaggregated Serving | 멀티노드 P-D 양방향 KV 전송 |
| `tokenspeed-mla` 신규 | Blackwell MLA 최적화 (DGX Spark GB10 호환) |
| FlashInfer 기본 활성화 + cubin 지연 로드 | Docker 이미지 ~2.5GB 축소 |

---

## 2. 첫 번째 함정: PyTorch exact pin

`Dockerfile`의 `VLLM_VERSION=0.21.0`만 바꾸려 했으나, vLLM 0.21.0 `pyproject.toml`을 확인하니:

```toml
torch == 2.11.0    # exact pin
```

`requirements/cuda.txt`에도 동일:
```
torch == 2.11.0
torchaudio == 2.11.0
torchvision == 0.26.0
```

기존 베이스 `nvcr.io/nvidia/pytorch:26.01-py3`는 **PyTorch 2.10.0**을 탑재 → exact pin 위반.
**베이스 이미지를 PyTorch 2.11.0 탑재 버전으로 올려야 한다는 결론.**

NGC 릴리스 노트 확인 결과 `pytorch:26.03-py3`가 조건 충족:

| 항목 | 26.01-py3 (기존) | 26.03-py3 (변경) |
|------|------------------|------------------|
| PyTorch | 2.10.0 | **2.11.0a0+a6c236b9fd1** ✅ |
| CUDA | 13.0 | **13.2.0.046** |
| Ubuntu | 24.04 | 24.04 |
| Python | 3.12 | 3.12 |

---

## 3. 두 번째 함정: vLLM wheel URL 404

베이스 이미지를 26.03으로 올린 후 빌드 실행:

```dockerfile
ARG VLLM_VERSION=0.21.0
ARG CUDA_VERSION=132    # CUDA 13.2 매칭 시도
```

```
ERROR: HTTP error 404 while getting
https://github.com/vllm-project/vllm/releases/download/v0.21.0/vllm-0.21.0+cu132-cp38-abi3-manylinux_2_35_aarch64.whl
```

GitHub Release API로 실제 자산 목록 조회:

```bash
curl -s "https://api.github.com/repos/vllm-project/vllm/releases/tags/v0.21.0" \
  | grep '"name":' | grep aarch64
```

결과:
```
vllm-0.21.0+cpu-cp38-abi3-manylinux_2_34_aarch64.whl
vllm-0.21.0+cu129-cp38-abi3-manylinux_2_34_aarch64.whl
vllm-0.21.0-cp38-abi3-manylinux_2_24_aarch64.whl
```

**두 가지 불일치 발견:**
1. manylinux 태그: `2_35` (Dockerfile) → **실제는 `2_34`**
2. CUDA 태그: `cu132`/`cu130` 미존재 → **`cu129`만 aarch64 빌드 제공**

---

## 4. 해결: CUDA Forward Compatibility 활용

CUDA 13.2 베이스 ↔ vLLM cu129 wheel은 메이저 버전 차이(12.x → 13.x)이지만, NVIDIA의 **CUDA Forward Compatibility** 기능으로 호환 가능.

```dockerfile
ARG VLLM_VERSION=0.21.0
ARG CUDA_VERSION=129

RUN CPU_ARCH=$(uname -m) \
    && pip install --no-deps \
      "https://github.com/vllm-project/vllm/releases/download/v${VLLM_VERSION}/vllm-${VLLM_VERSION}+cu${CUDA_VERSION}-cp38-abi3-manylinux_2_34_${CPU_ARCH}.whl"
```

컨테이너 부팅 시 NVIDIA 런타임 메시지로 활성화 확인:
```
NOTE: CUDA Forward Compatibility mode ENABLED.
  Using CUDA 13.2 driver version 595.45.04 with kernel driver version 580.142.
```

→ wheel은 CUDA 12.9 빌드이지만 13.2 드라이버 위에서 정상 동작.

---

## 5. 세 번째 함정: requirements.txt 의존성 재검토

vLLM 0.21.0의 `common.txt` + `cuda.txt`는 0.19.1 대비 다수의 의존성이 추가/변경되었다.
수동 비교는 비효율적이라 **컨테이너 내부에서 의존성 상태를 자동 분석하는 스크립트**를 작성했다.

### `check_reqs.py` (요약)
- vLLM 0.21.0 `common.txt` + `cuda.txt`를 GitHub raw에서 fetch
- `importlib.metadata`로 현재 설치 버전 조회
- `packaging.specifiers`로 spec 만족 여부 판정
- PyPI에서 spec 만족 최대 버전 조회

```bash
python3 /tmp/check_reqs.py
```

### 분석 결과 (69개 의존성 중)
- **OK**: 56개 (이미 베이스 이미지 또는 requirements.txt로 만족)
- **MISSING**: 6개
- **MISMATCH**: 7개

---

## 6. 카테고리별 조치 결정

### 6.1 베이스 이미지 영역 — 무시 (건드리면 안 됨)

| 패키지 | 설치됨 | 요구 | 이유 |
|--------|--------|------|------|
| `torch` | `2.11.0a0+...nv26.3.46836102` | `==2.11.0` | NGC 커스텀 빌드. alpha 태그 때문에 mismatch로 보이나 실제로는 2.11.0 기반 |
| `torchvision` | `0.25.0a0+...nv26.3` | `==0.26.0` | NGC와 함께 출하. 강제 교체 시 ABI 충돌 |
| `torchaudio` | 미설치 | `==2.11.0` | vLLM이 audio 직접 사용 안 함. NGC에 없는 게 정상 |
| `setuptools` | `81.0.0` | `>=77.0.3,<81.0.0` | 베이스에 0.0.1 초과. 다운그레이드 시 pip 깨질 위험 |

→ `requirements.txt`에 명시하지 않음.

### 6.2 실제 추가 필요 (MISSING 중 5개)

```text
flashinfer-python==0.6.8.post1
flashinfer-cubin==0.6.8.post1
tilelang==0.1.9
tokenspeed-mla==0.1.2
fastsafetensors>=0.2.2
```

### 6.3 버전 조정 필요 (MISMATCH 중 4개)

```diff
- outlines_core==0.2.11
+ outlines_core==0.2.14
+ numba==0.65.0                    # 현재 0.64.0
+ apache-tvm-ffi==0.1.9            # 현재 0.1.11 (다운)
+ nvidia-cutlass-dsl==4.4.2        # 현재 4.5.1 (다운)
```

### 6.4 제거

```diff
- outlines    # 0.21.0에서 outlines_core만 사용
```

---

## 7. 최종 Dockerfile 변경

```diff
- FROM nvcr.io/nvidia/pytorch:26.01-py3
+ FROM nvcr.io/nvidia/pytorch:26.03-py3

- ARG VLLM_VERSION=0.19.1
- ARG CUDA_VERSION=130
+ ARG VLLM_VERSION=0.21.0
+ ARG CUDA_VERSION=129

  RUN CPU_ARCH=$(uname -m) \
      && pip install --no-deps \
-       "https://github.com/vllm-project/vllm/releases/download/v${VLLM_VERSION}/vllm-${VLLM_VERSION}+cu${CUDA_VERSION}-cp38-abi3-manylinux_2_35_${CPU_ARCH}.whl"
+       "https://github.com/vllm-project/vllm/releases/download/v${VLLM_VERSION}/vllm-${VLLM_VERSION}+cu${CUDA_VERSION}-cp38-abi3-manylinux_2_34_${CPU_ARCH}.whl"
```

핵심 변경점 3가지:
1. 베이스 이미지 `26.01-py3` → `26.03-py3` (PyTorch 2.11 + CUDA 13.2)
2. wheel URL의 `cu132` → `cu129` (실제 배포 버전)
3. wheel URL의 `manylinux_2_35` → `manylinux_2_34` (실제 배포 태그)

---

## 8. 서빙 성공 확인

```bash
docker compose --env-file envs/.env.gpt-oss-120b-normal --profile master up
```

컨테이너 부팅 로그에서 CUDA Forward Compatibility 활성화 확인 후, 외부에서 API 통신 테스트:

```bash
PS C:\Users\tbvjv> curl.exe http://192.168.0.19:8900/v1/models
{
  "object": "list",
  "data": [{
    "id": "gpt-oss-120b",
    "object": "model",
    "created": 1779124369,
    "owned_by": "vllm",
    "root": "/app/models/OpenAI/gpt-oss-120b",
    "max_model_len": 32768,
    ...
  }]
}
```

→ vLLM 0.21.0 기반 gpt-oss-120b 서빙 정상 동작 확인.

---

## 9. 향후 참고사항

### CUDA Forward Compatibility 의존성
- 현재는 cu129 wheel을 CUDA 13.2 드라이버 위에서 forward compat로 실행 중
- vLLM이 차후 cu130/cu132 aarch64 wheel을 배포하면 즉시 전환 권장 (drift 위험 제거)
- 모니터링 포인트: [vLLM Release Assets](https://github.com/vllm-project/vllm/releases)

### NGC 베이스 이미지 업그레이드 시 체크리스트
1. NGC release notes에서 PyTorch / CUDA / Python 버전 확인
2. vLLM `pyproject.toml`의 `torch ==` 핀과 일치하는지 확인
3. vLLM GitHub Release 자산에서 aarch64 wheel의 `cuXXX` / `manylinux_X_YY` 태그 확인
4. `check_reqs.py`로 의존성 차이 분석 후 requirements.txt 갱신

### check_reqs.py 활용
- 향후 모든 vLLM 업그레이드 시 1차 검증 도구로 사용
- 스크립트 자체는 `/tmp` 임시 위치였으므로, 재사용성을 위해 `configs/check_reqs.py`로 영구화 검토 권장

### 미해결 / 후속 확인 필요
- [ ] Qwen3.5-122B-A10B-NVFP4 멀티노드 서빙에서 0.21.0의 NVFP4 KV 캐시 효과 측정
- [ ] NIXL 커넥터 활성화 시 멀티노드 토큰 처리량 변화 측정
- [ ] `tokenspeed-mla` 백엔드 실제 활성화 여부 및 성능 영향 확인
- [ ] 베이스 이미지 81.0.0 setuptools 경고가 빌드 중 실제로 어떤 영향을 주는지 추적

---

## 10. 최종 파일 현황

```
vllm_serving_server/
├── Dockerfile                            # 베이스 26.03-py3, vLLM 0.21.0, cu129
├── requirements.txt                      # +9개 추가/수정, -1개 제거
├── docker-compose.yaml
├── configs/
│   └── check_reqs.py                     # ← 의존성 검증 스크립트 (영구화 후보)
└── docs/devlog/
    └── devlog_260519_1_...               ← 이 문서
```
