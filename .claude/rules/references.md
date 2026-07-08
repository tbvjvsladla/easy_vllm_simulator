# references.md — 외부 레퍼런스 레지스트리 (ID→URL 정규화 + 검색 진입점)

> **역할**: 외부 교차검증(참조-그라운디드 해결·escalation·adversarial E)의 **1차 조회 대상**.
> 과거 작업(docs 코퍼스 98건 채굴 · plan_2026070208_1 Phase 3)이 실제로 인용한 외부 소스에서 유도했다.
> **URL 저장소가 아니다** — 실인용의 지배 형태는 축약 ID(PR#41834, forum 370309)이므로 이 파일의 1차
> 기능은 **ID→URL 정규화 템플릿 + 검색 레시피(진입점)**다.
> **평면**: 메인 전용(빌딩블럭 전파 축 — 파일 배치는 메인 고정) · **egress-restricted 서브**는 열람 불가 → 증상 상향(D12) · **egress-online+A2A 위임 서브**는 조건부 열람(이중게이트 — `plan_2026070809_2`). 추적 빌딩블럭(배포 대상).
> **자기증식 루프**: 외부검색 전 이 레지스트리 warm-start → 미스 시 신규 검색 → load-bearing 히트는
> 여기(stable/HW-스코프) 또는 docs(one-off 주장)로 입고.

## 0. 설계 원칙 (등재 규율)

- **stable 만 live-URL 등재**(release-notes 인덱스·카테고리·repo). **one-off**(특정 버그의 특정 issue·
  커뮤니티 launch-script)는 URL 등재 ✗ — rot 실증(hazyumps 404, plan_2026062818_1) — 대신 **"인용시점
  핵심 주장 1줄 + 인용 doc 경로"**로 docs 에 남긴다(wiki-desk 가 색인 — 그게 실효 보존 메커니즘).
- **스크립트-소유 템플릿은 중복 저작 금지** — 정본=코드, 여기엔 포인터만(§2).
- **음성정직**: 조회 실패(404·계정벽)면 실패로 기록 — 대체 URL 날조 ✗.
- **호환성 노트 필드**: 버전 조합-한정 호환성 사실(예 cu-minor 전방호환 성공 사례)은 엔트리에
  `호환성:` 노트로 축적(전방호환 시도-우선 따름정리의 사례 장부 — 헌법 §전방호환 시도-우선).

## 1. ID→URL 정규화 템플릿 (축약 인용 ↔ 역참조)

| 축약 ID 형태 | URL 템플릿 | 용도 |
|---|---|---|
| vLLM `#{n}` (PR/issue) | `github.com/vllm-project/vllm/{pull\|issues}/{n}` | arch 지원 시점·버그·포크 PR 확인 |
| vLLM `v{ver}` 릴리즈 | `github.com/vllm-project/vllm/releases/tag/v{ver}` | bump 1차 근거(사람용 — 예: #43477 SM120 enablement 인용) |
| 포크 `behind_by` 판정 | `api.github.com/repos/vllm-project/vllm/compare/v{ver}...{fork_sha}` | **포크 졸업/유지 결정론 증거**(behind_by/ahead_by/merge_base — testlog_2026070207_1 기법) |
| HF 모델카드 | `huggingface.co/{org}/{model}` (+`/discussions`, `/blob/main/config.json`) | **로컬 번들 우선**: 1차 = 모델 디렉토리 번들 README.md(=카드 원본, plan_2026062811_2 루틴) · 온라인은 2차 |
| HF discussions `#{n}` | `huggingface.co/{org}/{model}/discussions/{n}` | "stock OOB 미동작" 커뮤니티 보고 클래스(예: DS4 #28 → build_patch 유지 근거) |
| NVIDIA forum `{id}` | `forums.developer.nvidia.com/t/{id}` | 동일-HW 성능/배포 스레드 역참조 |
| GitHub raw 파일 | `raw.githubusercontent.com/{org}/{repo}/{ref}/{path}` | 소스 직독 범용(pyproject·requirements·백엔드 oracle — §2 ③의 일반형) |

## 2. 스크립트-소유 템플릿 (정본=코드 — 포인터만)

- ① vLLM release 자산 API: `resolve_wheel.py` (api.github.com/…/releases/tags/v{v} — 실자산 독해·404 함정 방지).
- ② wheel 다운로드 URL 구성: `resolve_wheel.py` (manylinux/cu 변종은 추측 않고 자산명에서 읽음 — **시도-차단 회피의 모범 패턴**).
- ③ raw pyproject 직독: `resolve_torch_pin.py` (torch 핀 추출 — requirements/*.txt·oracle 직독에도 재사용 가능).
- ④ NGC 베이스 해소: `resolve_ngc_tag.py` = **nvcr.io/nvidia/pytorch:{YY.MM}-py3 레지스트리 probe**(`docker buildx imagetools inspect` env 직독).
  docs.nvidia.com release-notes 페이지는 실증 사용 0회 — 사람용 보조 후보로만(투기적 codify ✗).

## 3. Curated stable 진입점

- **NVIDIA DGX-Spark 포럼 카테고리**: `forums.developer.nvidia.com` (DGX Spark) — GB10 성능/배포 권위.
  실증: 369076·361967(397B 2노드 용량불가 확정) · 370309·373808(MTP 31~44 t/s baseline — 15 t/s "천장"
  자기오판 반증 = adversarial-benchmark 탄생 계기) · GUIDE 374742. **성능 baseline 인용 규약**: 동일 HW·
  동일 모델·MTP on/off(R_fp/R_token like-with-like) 명시와 함께 기록.
- **deepseek-ai/DeepGEMM**: `github.com/deepseek-ai/DeepGEMM` (+`nv_dev` 브랜치·issues) — DS4/DSA 계열
  타겟인 동안 semi-stable(빌드-바깥 패치 3+1+1 의 +1 의존 원천 — output/<topology>/build_patches/ 의 deepgemm 모듈).
- **인코딩 자산 고정 URL**(에어갭 사전적재 따름정리 — 명칭=런타임 마운트 순수성, 환경 offline 아님): o200k_base →
  `openaipublic.blob.core.windows.net/encodings/o200k_base.tiktoken` (harmony 계열 동일 host).

## 4. HW-스코프 섹션 (manifest `gpu_model` 키 — 자기 HW 항목만 조회)

> **GPU-스펙 웹취득 규율 (자기증식 — plan_2026070809_1)**: 미지 GPU 카드(manifest `gpu_model` 이 아래에
> 미등재)를 만나면 → **웹검색(1급 리서치·에이전트 의무)** 으로 per-card VRAM·arch(sm_xx)·통합/discrete
> 여부를 조회 → **HITL 확인** → 아래에 해당 `gpu_model` 섹션 신설·입고(§0 "자기증식 루프"의 GPU-스펙 축).
> 조회는 획득모드와 무관하게 항상 허용·의무(모델획득 격리와 별개 평면 — CLAUDE.md §모델 획득 모드 따름정리).
> **소비 배선(γ, `plan_2026070809_3`)**: 각 GPU 섹션에 `per-card VRAM (GiB): <num>` 라인이 있으면
> `recipe.py resolve_target_gpu_budget()`(`_lookup_gpu_spec`)이 역룩업으로 읽어 타겟 예산에 대입한다
> (값 채우기·웹취득 규율 자체는 위 α 소관 — γ 는 소비만). 미등재 시 `config.target_gpu.per_card_vram_gib`
> 명시로 우회(quick-win). 헤더에 "통합메모리" 포함 시 `target_gmu≤0.90` 하드클램프 자동 적용.

### NVIDIA GB10 (sm_121 · aarch64 · 통합메모리)
- 추적 issue: vLLM `#41063` (GB10 tracking — GB10 타겟인 동안 semi-stable; 10-layer 디버그 patch-matrix).
- 검색 레시피: **"동일-HW 커뮤니티 배포 repo/launch-script 스캔"** — 개별 repo URL 은 churn 하므로 미등재,
  검색 패턴만: GitHub 검색 `"DGX Spark" vllm` / `GB10 vllm docker`. 인용 시 핵심 주장+doc 경로로 보존.
- 호환성: sm_121 arch-wall 계보 = PR#41834(SM12x 포크) → #43477(0.24.0 stock SM120 enablement) —
  버전별 stock 가능 여부는 릴리즈노트 문장 단위로 재확인(carry-forward ✗).

### NVIDIA RTX PRO 6000 (Blackwell · discrete · sm_120)
- per-card VRAM (GiB): 96
- 근거: 공개 스펙(96GB GDDR7, Blackwell 아키텍처, discrete — 통합메모리 아님) · 사용자 HITL 확인
  (δ 1-1 타겟-GPU 인터뷰, 2026-07-08 — `docs/plan/plan_2026070809_3` 인수 시나리오 첫 실사용).
- 등재 사유: γ(타겟-GPU 이식형 KV 클램프)의 첫 실 타겟 지정 요청 — `references.md` §4 웹취득 규율의
  자기증식 루프 첫 발동. (single-node 브랜치에서 최초 등재, 공유 빌딩블럭 정합을 위해 포팅.)

### NVIDIA GeForce RTX 4080 (Ada Lovelace · discrete · sm_89)
- per-card VRAM (GiB): 16
- 근거: 공개 스펙(16GB GDDR6X, 256-bit, Ada Lovelace, discrete — 통합메모리 아님) · 사용자 HITL 확인
  (δ 1-2 타겟-GPU 인터뷰, 2026-07-08 — 결합-HITL 단일턴 분기 테스트의 메인측 결정).
- 등재 사유: γ 두 번째 실 타겟 지정 요청(단일 턴에 메인·서브 결정이 묶여 온 첫 사례 — `plan_2026070809_4` 1-2).

### NVIDIA GeForce RTX 4070 (Ada Lovelace · discrete · sm_89)
- per-card VRAM (GiB): 12
- 근거: 공개 스펙(12GB GDDR6X, 192-bit, Ada Lovelace, discrete — 통합메모리 아님) · 사용자 HITL 확인
  (δ 1-2 타겟-GPU 인터뷰, 2026-07-08 — 결합-HITL 단일턴 분기 테스트의 서브측 결정).
- 등재 사유: 위와 동일 이벤트의 서브측 반쪽 — 메인/서브 두 GPU 스펙 확인이 사용자의 **단일 결합 메시지**로
  동시 도착해, 메인이 이를 "메인용/서브용"으로 분기해 각각 처리했다(δ 1-2 릴레이 분기 실증의 근거자료).

*(새 GPU 로 테라포밍하면 그 gpu_model 섹션을 신설 — 첫 escalation/벤치/**recipe 타겟-GPU 예산 산정**에서 채운다.)*

## 5. 부정판정 최소범위 레시피 (음성정직 (iii)·"현 vLLM 불가" 선언의 최소 탐색)

> 소비자: `upstream-version-watch` §3.6 (iii) · `vllm-recipe-explorer` §5.5/§6 · Phase-1 인피저블 보고.
> **이 레시피 수행·기록 없이 "공식·포크 모두 미지원" 음성정직 보고 금지**(가장 강한 부정 결론 =
> 가장 강한 증거 요구). 수행 기록 = testlog **"탐색 증거" 섹션**(검색어·URL·일자) 의무.

1. vLLM releases 최신 N개(기본 3) 노트에서 모델/arch 키워드 grep (§1 릴리즈 템플릿).
2. vLLM GitHub issue/PR 검색 — **모델 클래스명**(config.json `architectures`, 예 `Qwen3_5MoeForConditionalGeneration`)
   + 모델명 양쪽 (§1 PR/issue 템플릿; arch 지원 시점 확인의 실증 패턴 — testlog_2026062718_1).
3. HF 모델카드 vLLM 절 + discussions 탭 ("OOB 미동작" 보고 클래스) — 로컬 번들 README 1차.
4. 알려진 포크/enablement PR 검색 (아치-enablement 변종 트랙 후보 — escalation 3출구 (ii) 증거 클래스).
5. (성능 축이면) §4 HW-스코프 + §3 포럼 카테고리에서 동일-HW baseline.

## 6. 소비 배선 (스킬별)

- `upstream-version-watch`: §1 릴리즈/compare 템플릿(S1 resolve·포크 거버넌스) · §2 포인터 · §5 레시피((iii) 출구).
- `vllm-recipe-explorer`: §1 HF 템플릿(모델카드 교차검증 — 획득모드 무관 항상 의무) · §5 레시피(§5.5 발견 술어·§6 복구 ⑤단계·Phase-1 인피저블).
- `adversarial-benchmark`: §3 포럼·§4 HW-스코프 = E 검색 1차 진입점(E-arm = 이중게이트(A2A 위임 키 ∧ egress-online) 조건부 서브 자율) → 히트 baseline 재입고.
