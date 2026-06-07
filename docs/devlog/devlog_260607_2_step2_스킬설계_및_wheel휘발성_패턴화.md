# devlog 260607-2 — Step 2: 커스텀 스킬 설계 + wheel 휘발성 패턴화

## 작업 요약

부트스트랩 **Step 2(스킬 설계)** 를 완료했다. `/upstream-version-watch` 커스텀 스킬 + 결정론적
해소 스크립트 4종 + `config.yaml` 스키마를 작성했고, 0.22.1 dry-run 중 **wheel 파일명 휘발성**
(`manylinux` 태그가 버전마다 바뀜)이라는 반복 에러원을 발견해 **패턴 A(ARG 격리)** 로 구조 개선했다.
근거: 계획서 `docs/plan/plan_260607_2_*`, PoC `docs/testlog/testlog_260607_*`.

---

## 1. 산출물 (`.claude/skills/upstream-version-watch/`)

| 파일 | 역할 |
|------|------|
| `SKILL.md` | 스킬 본체(frontmatter + 해소→매핑→제안, 제안만). 적용은 rules/workflow.md |
| `scripts/resolve_torch_pin.py` | ① vLLM `pyproject.toml` → torch 핀 |
| `scripts/resolve_ngc_tag.py` | ② torch 핀 → NGC 태그 (`docker buildx imagetools inspect`, skopeo 불필요) |
| `scripts/resolve_wheel.py` | ③ wheel URL (GitHub Release 자산 실재 검증, 404 방지) |
| `scripts/regen_requirements.py` | ④ requirements 재생성 분석 (`configs/check_reqs.py` 발전형) |
| `config.example.yaml` / `config.yaml` | 입력 스키마(트리거 = vLLM 버전만) |

- 하네스 엔지니어링: **버전 문자열 해소 = 결정론적 스크립트**, 변경요약·리스크 = 모델 판단.
- 모두 `.claude/` 하위 = 빌딩블럭(origin push 안 함).

## 2. 단위 검증 (0.21.0 기준, 현 Dockerfile 재현)

| 스크립트 | 입력 | 출력 | 일치 |
|---|---|---|---|
| ① | 0.21.0 | torch 2.11.0 | ✓ |
| ② | 2.11.0 | 26.03-py3 (build 2.11.0a0+a6c236b) | ✓ |
| ③ | 0.21.0 aarch64 | cu129 / manylinux_2_34 | ✓ (Dockerfile 실제값) |
| ④ | 0.21.0 | 69 deps 분석 | ✓ |

## 3. 0.22.1 dry-run 에서 드러난 핵심 — wheel 휘발성

- ② NGC 프로빙이 최신 `26.05`/`26.04`(torch **2.12.0**)를 거부하고 torch 2.11.0 매칭 `26.03`을 선택.
  → "최신이 아닌 torch 매칭" 규칙이 결정론적으로 동작(베이스 오선정 방지).
- ③ wheel: 0.22.1은 `manylinux_2_28` (0.21.0은 `manylinux_2_34`). **버전마다 미세 변경**.
  - 기존 Dockerfile은 `manylinux_2_34`를 **하드코딩** → 0.22.1에서 404 발생하는 반복 에러원.
- ④ 의존성 delta (0.21.0 → 0.22.1):
  - 변경: `flashinfer-python/cubin` 0.6.8.post1→0.6.11.post2, `llguidance` <1.4.0→>=1.7.0,<1.8.0, `nvidia-cutlass-dsl` 4.4.2→4.5.2
  - 신규: `humming-kernels==0.1.2`, `safetensors>=0.6.2`(베이스 제공 여부 컨테이너 `--use-installed` 확인)

## 4. 패턴 A — wheel 휘발성 ARG 격리 (Dockerfile 구조 개선)

휘발성 디테일(`cuXXX`·`manylinux`)을 Dockerfile 본문에 하드코딩하지 않고 **ARG로 격리**한다.
empirical 확인: 한 릴리스 안에서 `cuda`·`manylinux`는 arch(aarch64/x86_64) 무관 동일, 파일명의 arch 접미어만 다름.

```diff
+ ARG VLLM_MANYLINUX=manylinux_2_34
  ...
-       "...vllm-${VLLM_VERSION}+cu${CUDA_VERSION}-cp38-abi3-manylinux_2_34_${CPU_ARCH}.whl"
+       "...vllm-${VLLM_VERSION}+cu${CUDA_VERSION}-cp38-abi3-${VLLM_MANYLINUX}_${CPU_ARCH}.whl"
```

- **트리거 = vLLM 버전 하나.** `CUDA_VERSION`·`VLLM_MANYLINUX`는 `resolve_wheel.py`가 자동 도출해 채운다.
- `$(uname -m)` 멀티arch 유지. bump = ARG 값 3개 교체.
- 검증: 패턴이 만드는 URL == `resolve_wheel.py` 검증 URL (0.21.0/0.22.1 모두 일치).

## 5. 완료 기준 점검

- [x] SKILL.md + 스크립트 4종 + config 스키마 작성
- [x] ①~④ 단위 검증(0.21.0 재현, 0.22.1 dry-run)
- [x] `docker buildx` 로 NGC torch 버전 결정론적 조회(skopeo 불필요)
- [x] wheel 휘발성 패턴 A 격리 + URL 일치 검증
- [ ] `/upstream-version-watch` 슬래시 발동 E2E (다음 세션 로드 후)

## 6. 다음 단계 — Step 3 (단일노드 검증)

`config.yaml target_vllm_version=0.22.1` 기준으로 Dockerfile/requirements.txt bump 적용 →
`docker compose --profile debug build` → 단일노드 스모크(gpt-oss-120b, NAS) → `docs/testlog` 기록.
