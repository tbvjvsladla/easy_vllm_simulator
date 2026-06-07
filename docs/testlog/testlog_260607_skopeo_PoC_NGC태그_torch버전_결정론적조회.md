# testlog 260607 — NGC 태그 torch 버전 결정론적 조회 PoC

## 목적

Step 2 §3 핵심 난제 해소: NGC PyTorch 태그의 torch 빌드 버전을 **결정론적으로** 읽어
vLLM `pyproject.toml`의 torch 핀과 접두어 매칭할 수 있는지 검증.

## 환경

- skopeo: **미설치**
- docker: 29.2.1 (buildx 포함) · jq · curl: 설치됨

## 결과 — ✅ skopeo 없이 `docker buildx imagetools inspect`로 성공

### 1) 원격 inspect (익명, 레이어 pull 없음)

```bash
docker buildx imagetools inspect nvcr.io/nvidia/pytorch:26.03-py3
```
→ 인증 없이 manifest list 조회 성공 (multi-arch: linux/amd64, linux/arm64).

### 2) arm64(DGX Spark) config env에서 torch 버전 추출

```bash
docker buildx imagetools inspect nvcr.io/nvidia/pytorch:26.03-py3 --format '{{json .Image}}' \
  | jq -r '.["linux/arm64"].config.Env[]?' \
  | grep -iE "PYTORCH_BUILD_VERSION|PYTORCH_VERSION|CUDA_VERSION"
```
출력:
```
CUDA_VERSION=13.2.0.046
PYTORCH_BUILD_VERSION=2.11.0a0+a6c236b
PYTORCH_VERSION=2.11.0a0+a6c236b
NVIDIA_PYTORCH_VERSION=26.03
```

## 핵심 발견

- 이미지 config env의 실제 키는 **`PYTORCH_BUILD_VERSION` / `PYTORCH_VERSION`** (값 `2.11.0a0+...`).
  - 인터뷰에서 언급된 `NVIDIA_PYTORCH_BUILD_VERSION`은 레이어 history의 ARG 명칭이고,
    이미지 config env에는 위 두 키로 노출된다. **값(접두어 2.11.0)은 동일**.
  - `NVIDIA_PYTORCH_VERSION=26.03`은 컨테이너 태그(혼동 주의 — torch 버전 아님).
- 접두어 `2.11.0` = vLLM 0.21.0의 torch 핀 `2.11.0`과 일치 → **레이어 커플링 규칙 기계적 검증 완료**.

## 결정 (Step 2 §3 확정)

- **②NGC 조회 = `docker buildx imagetools inspect` 채택** (skopeo 불필요 → 설치정책 미발동).
- `resolve_ngc_tag` 스크립트: 후보 태그에 대해 위 명령으로 `PYTORCH_BUILD_VERSION` 접두어 추출 →
  torch 핀과 비교. 후보 태그 열거는 (a) 매핑/윈도우 프로빙 또는 (b) NGC 카탈로그 — 스크립트 작성 시 확정.
- arch는 `linux/arm64`(DGX Spark) 기준. amd64 병행 필요 시 `.["linux/amd64"]`도 동일 방식.

## 재현 명령 (영구 보관)

```bash
TAG=26.03-py3; ARCH=arm64
docker buildx imagetools inspect "nvcr.io/nvidia/pytorch:${TAG}" --format '{{json .Image}}' \
  | jq -r ".[\"linux/${ARCH}\"].config.Env[]? | select(startswith(\"PYTORCH_BUILD_VERSION=\"))"
```
