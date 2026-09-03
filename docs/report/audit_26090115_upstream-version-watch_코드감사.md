# upstream-version-watch — 코드레벨 감사

> 감사 시각: 2026-09-01 15:32 KST (KST 절대시각 `26090115`)
> 대상 브랜치: `single-node` · 대상 커밋: `ab2b459`
> 감사 범위: `.claude/skills/upstream-version-watch/` 전량 — 50파일 · 11,428행
> (`SKILL.md` 128행 · `scripts/` Python 12종 4,207행 · 셸 10종 3,479행 ·
> `references/` 4종 · `templates/` 5종 · `assets/` 12종 · `fixtures/` 4종)
> 판정 기준 정본: `CLAUDE.md` · `.claude/rules/workflow.md` · `.claude/rules/docs.md` · 대상 `SKILL.md`
> 감사 방식: PR Review Toolkit 전문 에이전트 **7종 병렬** 정적감사 + 감사자 직접 실행·재현
> 침습도: **read-only** — 대상 파일·저장소 상태를 변경하지 않았고, 빌드·서빙·동기화 스크립트를
> 한 번도 실행하지 않았다. 실행은 `--self-test`·`--check-fixture`·`--help`·순수함수 인메모리 호출에
> 한정했다. 이 감사 중 다른 세션이 같은 호스트에서 `gpt-oss-120b` 빌드→서빙→벤치마크를 수행 중이었다.

---

## 결론

**이 스킬의 문제는 "부재"가 아니라 "연결되지 않음"이다.**

직전 두 감사의 결론은 각각 *"설계는 정확하고 배선이 비어 있다"*(hint-publisher)와
*"만들지 않은 것이 아니라 전파되지 않은 것"*(terraforming_node)이었다. 이 스킬은 세 번째 변주다 —
**부품은 대부분 훌륭하고, 부품 사이를 잇는 것이 사람의 손이다.**

이 스킬은 이 저장소에서 가장 정교한 검증 자산 두 개를 갖고 있다. `_inherit_eligibility` 는 11개 술어
전건 통과를 요구하고 거부 사유를 이름으로 지목하며, 양성대조 1건과 **음성대조 12건**을 함께 둔 이유를
*"이 대조가 없으면 음성대조는 '가드가 항상 빨간불'인 것과 구분되지 않는다"* 로 적어 두었다.
`upstream_delta` attestation 은 미구현 축을 `NOT_IMPLEMENTED` + `excluded_from_global` 로 **데이터에
명시**하고, 판정의 한계(`limitations[]`)를 판정과 함께 실어 보낸다. 이 감사는 변이 실험 9종으로 그
검출력이 **실재함**을 측정했다(8/9 검출).

그런데 **그 시험들을 아무도 돌리지 않는다.** 배포 검증기 `verify_distribution.py:572-576` 은 이 스킬의
스크립트 8종을 `--help` 로만 호출하고, `judge_version_delta.py --self-test` 는 그 목록에 **없다**.
스킬별 게이트 등재 수를 세면 이 비대칭이 한눈에 보인다:

| 스킬 | self-test 보유 | 배포 게이트 등재 |
|---|---|---|
| **upstream-version-watch** | **4** | **0** |
| vllm-recipe-explorer | 3 | 3 |
| adversarial-benchmark | 1 | 1 |
| terraforming_node | 21 | 2 |

가장 선명한 세 장면:

- **정본이 자기 사고를 자백하고 있다.** `assets/current-production-resolution.json` 의
  `_meta.why_updated` 는 *"정본이 실물을 잃고 gitignored resolved.json 이 그림자로 실물을 보유 …
  그래서 8/11 렌더가 legacy `--template` 경로로 새어나갔다"* 고 적는다. 해소값 SSOT 는 2026-08-13 에
  고쳐졌다. **그런데 그 legacy 경로를 정본으로 가르치는 `references/resolve-and-render.md` 는 고쳐지지
  않았다** — 코드는 스스로 그 플래그에 `legacy`·`canonical rendering forbids it` 이라 이름을 붙여 놓았다.
- **불변식이 한국어 산문 문자열이다.** 헌법이 "prebuilt wheel 트랙 필수"라 부르는 torch 접두어 일치는
  `current-production-resolution.json` 의 `"prefix_invariant": "… 충족"` 이라는 **자기 주장 문장**이
  유일한 표현이다. `pytorch_build_version` 을 읽는 코드는 저장소 전체에 **0건**이다. 두 값이 같은 JSON
  안에 나란히 있는데 대조하는 술어가 없다.
- **가장 결과가 큰 분기가 부분문자열 검사다.** `track = "source" if "source" in decision else "wheel"`.
  대문자 한 글자(`Source-Build`)나 키 부재가 소스빌드를 wheel 트랙으로 뒤집는데, 그것을 막았어야 할
  유일한 무결성 가드는 **stale 잔재값 때문에 정확히 그 사고에서만 침묵한다.**

그리고 이 감사는 문서가 **실제보다 비관적인** 사례도 하나 잡았다 — `workflow.md` 의
*"`build_patches_src/` 는 미검증 슬롯 … 서빙에 성공한 사례가 아직 없다"* 표기는 **만료됐다**.
추적 원장에는 그 슬롯 위에서 **서빙 스모크 PASS · full 벤치 PASS** 한 변종이 3건 있다. 문서가 자기
만료조건("성공 사례가 나오면 이 표기를 지운다")을 충족했는데 표기가 남아, 사다리 3칸(자체 이식)을
도박으로 오평가하게 만들고 **포크 핀으로 조기 이동**을 유도한다.

---

## 감사 방법과 증거 등급

전문 에이전트 7종을 서로 다른 렌즈로 병렬 투입하고, 파급이 큰 주장은 감사자가 직접 재현해 확증했다.

| 등급 | 뜻 |
|---|---|
| **실측** | 감사자가 이 호스트/저장소에서 직접 실행·관측해 확인 (본 감사 **19건**) |
| **대조** | 감사자 또는 에이전트가 코드를 직접 읽어 확인 |
| **정적** | 에이전트의 코드 독해 결과 — 감사자가 재실행하지 않음 |

투입 에이전트: `code-reviewer` ×2(resolve 체인 / 렌더·판정) · `silent-failure-hunter` ×2(Python 12종 /
셸 10종) · `comment-analyzer`(주장 대 실재) · `pr-test-analyzer`(검증 커버리지·변이 실험) ·
`type-design-analyzer`(교환 계약·스키마).

### 방법론 주의 — 이 환경의 `grep` 은 ugrep 이다

`/usr/bin/grep` 은 **ugrep 7.8.4** 이며 `.gitignore` 를 기본 적용한다. 부정 주장("호출자가 없다")을
확정하는 데 그냥 쓰면 위양성이 난다. 실측한 은닉 규모:

| 질의 | 기본 grep | `--no-ignore-files` | 가려짐 |
|---|---|---|---|
| `resolved.json` | 17 | 58 | **41** |
| `upstream_delta` | 6 | 15 | 9 |
| `INHERITED_SOURCE_BUILD_KEYS` | 4 | 8 | 4 |

가려진 것은 전부 `seed/` 백업 사본과 gitignored `docs/`였고 **추적 파일 은닉은 0건**이라 이번 감사의
부정 주장은 유효하다. 다만 `seed/` 에 이 스킬의 **구버전 전체 사본**이 있어, `--no-ignore-files` 로 켜면
같은 결함이 두 벌로 잡힌다 — 본 보고서의 모든 줄 번호는 **추적본 기준**이다.

### 감사 기준선 (실측)

| 항목 | 값 |
|---|---|
| 구문 검사 | Python 12종 `py_compile` 전량 통과 · 셸 12종 `bash -n` 전량 통과 |
| 자체검사 4종 | **전량 PASS** (judge 27케이스 · render 5블록 · build_track 14단언 · fixture 2종 deep-diff) |
| 오프라인 재실행 | 4종 전부 **동일 통과** — 네트워크 무의존 확증(조용한 스킵 없음) |
| `output/single/manifest.yaml` | 실재(테라포밍 완료) · `output/multi/manifest.yaml` **부재** |
| 파일별 최종 변경 | 코드 2026-08-24까지 진행 · `references/` 다수 **2026-07-27 정지** |

### 반증된 가설 (음성 대조 — 재조사 방지)

감사에서 제기됐다가 **직접 확인으로 기각**된 것들이다. 기록하지 않으면 다음 감사가 같은 길을 다시 판다.

- **`--check-fixture` 는 순환 오라클이 아니다.** `strip_volatile` 이 벗기는 것은 `replay_of` 하나뿐이고
  `deep_diff` 는 전량 재귀 대조다. `--emit-fixture` 에 재생본↔라이브 대조 게이트(`code=5`)가 있어
  자기충족 루프를 한 번 끊는다. 변이 실험에서 골든은 M1·M2·M3·M4·M5·M8·M9 를 잡았다.
- **브랜치로 토폴로지를 추론하는 코드는 없다.** 헌법 금지사항은 지켜진다(`abbrev-ref` 용례는
  sync 스크립트의 자기 목적뿐).
- **`--allow-unvalidated` 로 부적격 상속을 우회할 수 없다.** 미인식 키 기본 `exit 1` 도 확인.
- **에이전트가 `INHERITED_SOURCE_BUILD_KEYS` 에 쓰는 코드 경로는 없다.** SKILL.md §금지가 지켜진다.
- **`render` 자체검사의 `rendered != golden` 은 dict 비교라 값까지 대조한다.** 배너 문구
  "17키 == golden 집합 동치"가 실제 검사보다 **약하게 말하고 있을 뿐**이다.
- **`multinode_comms_smoke.sh` 162행은 전건 통과 — 발견 0건.**
- **`check_smoke_model.py` 의 no-download 게이트 자체는 견고하다.** 예외 없이 exit 2 이고 다운로드
  기계장치가 전무하며 술어 2종이 이를 감시한다.
- **`output/single/Dockerfile` 이 `?? `(비무시)인 것은 결함이 아니다** — `.gitignore:125` 가 인덱스
  권위 빌드킷 스냅샷을 위해 **의도적으로 재포함**한 것이다.

---

## 진행 중 작업에 대한 즉시 경고 (실측)

이 감사 시점에 다른 세션이 같은 호스트에서 서빙 작업 중이며, **아래 두 명령은 지금 실행하면 그 작업을
훼손한다.** 감사 중 실행하지 않았고, 실행 전 확인을 권고한다.

1. **`sync_branches.sh --apply`** — 현재 미커밋 4파일(`.claude/policies/runtime/runtime_selftest.py` ·
   `.claude/skills/hint-publisher/scripts/{hint_tag.py,bootstrap_families.py}` · `.gitignore`)이
   **전부 `ALLOWLIST` 아래**이며, 이 스크립트에는 dirty·stash·backup·trap 검사가 **한 줄도 없다**
   (아래 S1-⑥).
2. **`single_serve_down.sh` / `multinode_serve_smoke.sh --down`** — 회수 대상 선정이 **노드 전역**이다.
   지금 `mem_watchdog.sh vllm_trial03 10240 2`(PID 876456, 3시간 24분 경과)가 돌고 있고,
   `single_serve_down.sh:110` 은 스크립트 basename 만으로 매칭해 **config 범위를 보지 않는다**.
   컨테이너는 안 죽지만 다른 세션의 호스트 방어 2층이 벗겨진다(아래 S1-⑩).

---

## 심각도 1 — 10건

### ① 소스빌드 신호가 requirements 루프로 오라우팅된다 — 전용 패턴이 도달 불가 사문(死文)이다

**위치**: `failure_patterns.yaml:14,22` vs `:30,36,38` · `scripts/classify_failure.py:74-81`
**등급**: 실측(2케이스 재현)

`classify_failure.py` 는 파일 순서대로 첫 매치에서 종료하는데, **가장 넓은 두 패턴이 맨 앞에** 있다.

```
$ classify_failure.py --log <torch._opaque_base 트레이스백>
{"class": "requirements-fixable",
 "matched_signature": "ModuleNotFoundError: No module named '([^']+)'",
 "evidence": "ModuleNotFoundError: No module named 'torch._opaque_base'"}   EXIT=0
```

`:30` 에 이 실패 **전용** `source-build-class` 패턴이 있는데 `:14` 광역 패턴이 항상 먼저 잡는다.
두 번째 재현은 더 나쁘다 — 무해한 한 줄이 진짜 ABI 벽을 가린다:

```
로그: "/bin/sh: 1: nvcc: command not found"
      "ImportError: .../vllm/_C.abi3.so: undefined symbol: _ZN3c105Error..."
→ {"class": "requirements-fixable",
   "matched_signature": "([A-Za-z0-9_-]+): command not found",
   "evidence": "/bin/sh: 1: nvcc: command not found"}                        EXIT=0
```

**같은 ABI 벽이 무해한 한 줄의 유무로 exit 1↔0 을 오간다.** evidence 필드까지 오도하므로 사람 게이트도
막지 못한다. 결과: torch 2.11+ ABI 벽이 `reconciliation_cap` 3회분 컨테이너 재빌드를 태운 뒤에야
Model-C 로 떨어지고, `references/source-build.md` 출구에는 영원히 진입하지 않는다.

**근거**: `failure_patterns.yaml:8` 이 스스로 적은 규칙을 자기가 어긴다 — *"순서 = 우선순위 …
**스코프 좁은 시그니처를 넓은 폴백보다 앞에**"*. `workflow.md` §실패 라우팅 표의 `source-build-class`
행이 이 두 입력에 대해 구조적으로 도달 불가가 된다.

**처방**: `:30`·`:36`·`:38` 블록을 `:14`·`:22` 앞으로 이동. **3줄 순서 교환으로 닫힌다.**

---

### ② 빌드트랙 판정이 부분문자열 검사이고, 유일한 안전망은 stale 잔재 때문에 침묵한다

**위치**: `scripts/render_dockerfile.py:463-464`, `:711-712` · `assets/current-production-resolution.json:30-33`
**등급**: 실측(인메모리 순수함수 프로브 4케이스)

```python
decision = str(resolved.get("build_track", {}).get("decision", ""))
track = "source" if "source" in decision else "wheel"
```

실제 canonical 해소값으로 프로브한 결과:

| `decision` 값 | DOCKERFILE | IMAGE_TAG | CUDA_VERSION |
|---|---|---|---|
| `source-build` | `Dockerfile.source-build` | `0.27.0-cu133-aarch64-source` | `129` |
| `Source-Build` | **`Dockerfile`** | `0.27.0-cu129-aarch64-**wheel**` | `129` |
| `src-build` | **`Dockerfile`** | `0.27.0-cu129-aarch64-**wheel**` | `129` |
| 키 부재 | **`Dockerfile`** | `0.27.0-cu129-aarch64-**wheel**` | `129` |

대문자 한 글자가 소스빌드를 프리빌트 wheel 트랙으로 뒤집는다. 그러면 torch 2.13 해소값 위에 wheel
Dockerfile 이 렌더되어 **헌법의 레이어 커플링 불변식이 정면으로 깨지는데, 깨졌다는 사실이 데이터
어디에도 남지 않는다.**

**그리고 유일한 무결성 가드가 정확히 이 사고에서만 침묵한다.** `:711-712` 는
`if "vllm-${VLLM_VERSION}+cu${CUDA_VERSION}" in text and not ctx.get("CUDA_VERSION"): raise` 로
**비어 있을 때만** 운다. 위 표의 `CUDA_VERSION='129'` 는 정본 최상위에 남은 **0.26.0 시절 stale 잔재**다
(source-build 트랙 실제 CUDA 는 `13.3.1.008`, wheel 트랙은 `130`/`manylinux_2_35` — 어느 트랙과도 맞지
않는다). 값이 **없었다면** fail-loud 로 죽었을 것을, stale 값이 있어서 조용히 통과시킨다.

> 여기서는 **결측이 stale 보다 안전하다.** 그런데 스키마에 `null` 도 `applicable: false` 도
> `ABSENT` 센티넬도 없다 — 이 저장소는 이미 그 처방을 다른 스킬에서 쓰고 있다.

**처방**: (a) `_TRACKS = {"wheel","source-build"}` 닫힌 열거 + 밖이면 `raise`(현 데이터가 이미 정확한
리터럴이라 파괴적 변경 없음) (b) 최상위 stale `wheel` 블록 제거. **(a)만 하고 (b)를 남기면 다른 경로로
샌다.**

---

### ③ 로드-전 RAM 게이트가 exit 0 으로 "통과"와 "미실행"을 융합한다 — 양 노드 방어가 동시에 꺼진다

**위치**: `scripts/check_smoke_model.py:260-268` · 소비자 `scripts/multinode_serve_smoke.sh:181-192`
**등급**: 실측(코드 대조 + 대조군 확인)

도크스트링 계약은 `:12` 에 **`0=존재(+RAM 게이트 통과)`** 로 못박혀 있다. 그런데 두 예외 경로가 모두
`sys.exit(0)` 으로 떨어진다 — `except ImportError`(모듈 부재로 오인되는 경로 오류 포함)와
`except Exception`(NAS 마운트의 ESTALE/EIO 등). 유일한 기계 소비자는 종료코드만 본다:

```bash
case "$NAS_RC" in
  0) : ;;                    # ← "게이트 통과"와 "게이트 미실행"이 동일
...
REQ_MIB=$(... grep -oE 'required_mib=[0-9]+' ...)
if [ -n "$REQ_MIB" ]; then   # ← 비면 슬레이브 게이트 블록 전체가 무출력 skip
```

게이트가 생략되면 `GATE_PARAMS required_mib=` 도 방출되지 않으므로 `REQ_MIB` 이 비고, **슬레이브 대칭
게이트가 경고 한 줄 없이 통째로 건너뛰어진다.** 그 대칭은 주석(`:188`)이 밝히듯
**"하드다운 #2 = DS4 serve#1 '서브'였음"** 때문에 만들어진 것이다.

**대조군이 이 진단을 확정한다**: 같은 스모크 스크립트가 `BUDGET_PARAMS` 미검출에 대해서는 `:492-502`
에서 *"로드는 **0초도 시작하지 않았다**"* 를 명시하며 fail-closed 한다. **한 스크립트 안에서 두
안전신호의 처방이 갈렸다** — 처방은 이미 저장소 안에 있고 적용되지 않았을 뿐이다.

**처방**: 게이트 미실행 전용 종료코드(예 9) 또는 `RAM_GATE=skipped(reason=...)` 를 stdout 계약에 추가.

---

### ④ single 통로에서는 NAS 실재 체크·RAM 게이트·spec 선검사가 한 번도 실행되지 않는다

**위치**: `scripts/check_smoke_model.py`(호출자) · `scripts/multinode_serve_smoke.sh:179`
**등급**: 실측(전수 grep)

`check_smoke_model.py` 의 **실행 호출자는 저장소 전체에서 하나**이고, 거기에 `--topology multi` 가
하드코딩돼 있다. 추적본의 다른 언급은 `verify_distribution.py`(`--help` 호출)와
`claim_predicates.py`(소스 문자열 읽기)뿐이다. 그리고 `check_spec_layout.py` 의 유일한 호출자는
`check_smoke_model.py:200` 이다.

⇒ **현재 체크아웃(`single-node`)에서는 세 게이트가 전부 사정거리 0 이다.** `--topology single` 인자가
구현돼 있는데 그것을 넘기는 코드가 없다. 헌법이 "두 브랜치 **둘 다 배포 대상**"이라 못박은 상황에서,
호스트 안전 게이트가 한쪽 배포 대상에만 배선돼 있다.

**연쇄(심각도를 올리는 이유)**: `check_smoke_model.py:189` 의 PASS 술어는 `os.path.isdir(host_path)`
**뿐**이다(샤드·`config.json`·인덱스 미검사). 빈 디렉터리 → `checkpoint_bytes<=0` → `skipped:True` →
`required_mib` 부재 → 위 ③의 슬레이브 스킵. NAS 마운트 끊김·중단된 다운로드가 **"모델 있음 + 양노드
무방비"** 로 서빙에 진입한다.

---

### ⑤ 레이어 커플링(torch 접두어 일치) 불변식의 집행 코드가 0건이다

**위치**: `scripts/resolve_torch_pin.py:87`(생산) · `assets/current-production-resolution.json:20,46,48`
**등급**: 실측(전수 grep — `--no-ignore-files`)

헌법이 "prebuilt wheel 트랙 필수 불변식(wheel `_C` 의 ABI 요건)"이라 부르는 규칙의 **유일한 표현**은
정본 JSON 의 서술 문자열이다:

```json
"prefix_invariant": "torch 2.10.0 == NGC 2.10.0a0 접두어 일치 — wheel 트랙 필수 불변식(...) 충족"
```

- 이 문자열을 파싱하는 코드: **0건**
- `pytorch_build_version`(NGC 베이스의 실제 torch 빌드버전)을 **읽는** 코드: **0건**
- `torch_prefix` 출현 6줄은 **전부 생산자·문서** — 소비자·검증자 0건

`torch.pin`(`2.13.0`)과 `ngc_base.pytorch_build_version`(`2.13.0a0+9186a08`)이 **같은 JSON 안에 나란히**
있는데 둘을 대조하는 술어가 없다. `tracks.wheel.torch.pin` 을 손으로 바꿔도 옆줄의 `"… 충족"` 은 그대로
충족이라 주장한다.

**처방**: `load_shared_resolution()` 에서 wheel 트랙일 때 접두어 대조를 fail-closed 로 추가. 같은 함수가
이미 `ngc_base.tag` 부재를 fail-closed 처리하므로 **같은 자리에 술어 하나**이고, 두 값이 이미 파일에
있으므로 비용이 사실상 0 이다.

---

### ⑥ `sync_branches.sh` 가 미커밋 작업물을 무경고 파괴한다 — 지금 이 워킹트리에서 재현된다

**위치**: `scripts/sync_branches.sh:414`
**등급**: 실측(전수 grep + ALLOWLIST 대조)

```bash
git checkout "$SRC_BRANCH" -- "${PATHS[@]}"
```

`git checkout <branch> -- <paths>` 는 워킹트리와 **인덱스를 동시에** 덮는다. 467행 전체에
`porcelain`·`stash`·`dirty`·`trap`·백업 검사가 **하나도 없다**(전수 grep 결과 0건).

DRY-RUN 도 이것을 못 잡는다 — `:393` 의 `git diff --stat "$SRC_BRANCH" -- ...` 는 **정본↔워킹트리** 차이라
"내 미커밋 편집"과 "정본이 정상적으로 바뀐 것"이 같은 화면에 뭉쳐 나오고, 사람이 자기 작업물을 식별할
방법이 없다. 게다가 `|| true` 가 붙어 diff 실패 시 **빈 출력**이 나오는데, 바로 위 문구가
*"정본과 현재 working-dir 의 차이(없으면 이미 동일):"* 라 사람은 오류를 "이미 동일"로 읽는다.

**비대칭이 결함을 확정한다**: 같은 스킬의 `sync_to_sub.sh` 는 **원격** 서브 트리에 대해
`policy:SUB_SYNC_DIRTY_AUTOSAVE` 로 dirty 를 `[improve]` 커밋으로 보존하고 실패 시 exit 8 한다.
**로컬 트리에는 그 보호가 0이다.**

---

### ⑦ 서브 dirty **탐지** 실패가 "clean" 으로 읽혀, 소실방지 게이트 전체가 우회된다

**위치**: `scripts/sync_to_sub.sh:591,1346-1356`
**등급**: 대조

```bash
sub_dirty() { sub_run "git status --porcelain 2>/dev/null"; }
DIRT="$(sub_dirty || true)"
if [ -n "$DIRT" ]; then      # ← 보존(autosave) 경로 전체가 이 안에 있다
```

`2>/dev/null` 이 stderr 를, `|| true` 가 비-0 종료를 지운다. 남는 빈 stdout 은 "clean" 과 구분되지 않는다.
서브에 `.git/index.lock` 이 남아 있는 상황 — **이 파일 자신이 `:823-829` 에서 실제로 발생했다고 기록한
상황** — 에서 `git status` 는 stderr 로 죽고 `DIRT=""` 가 되어 보존 블록이 통째로 건너뛰어진 뒤
`rsync -az --delete` 가 서브의 미커밋 저작물을 덮는다.

**바로 아래 두 줄이 대비다**: `:1350`·`:1352` 는 `git add -A`·커밋 **실패**를 각각 exit 8 로 닫는다.
저자는 **보존 실패**를 닫았지만 **탐지 실패**는 열어 뒀다 — 탐지가 열려 있으면 보존 게이트는 애초에
실행되지 않으므로, 닫아 둔 쪽이 무의미해진다.

---

### ⑧ 정본 문서가 코드의 `legacy` 경로를 정본으로 가르친다 — 포크 핀·빌드패치가 통째로 사라진다

**위치**: `references/resolve-and-render.md:130,134,135` vs `scripts/render_dockerfile.py:1051,1055`
**등급**: 실측(템플릿 대조)

코드가 스스로 붙인 이름:

```python
ap.add_argument("--template", help="legacy caller-provided template path")
ap.add_argument("--resolved", help="legacy caller-provided resolution; canonical rendering forbids it")
```

정본은 `--canonical-kind {dockerfile|source-build|compose}` + `--topology` 인데, **`--canonical-kind` 는
SKILL.md·references 어디에도 0건**이다. 그리고 문서가 지목한 루트 템플릿은 정본과 갈라져 있다:

| | 루트 `Dockerfile.source-build.template`(문서가 지목) | `templates/…`(코드 정본) |
|---|---|---|
| `VLLM_REPO` 출현 | **0** | **5** |
| `build_patches` 출현 | **0** | **12** |

⇒ 문서대로 렌더하면 포크 핀(`VLLM_REPO`/`VLLM_REF`)·`build_patches/`(post)·`build_patches_src/`(pre)가
**전부 빠진 Dockerfile 이 나오고, 빌드는 성공한다** — stock vLLM 이 조용히 빌드될 뿐이다. arch-wall
변종 트랙 전체가 무효화되고 실패는 수 시간 뒤 서빙에서야 드러난다. compose 도 같다: 문서가 지목한
루트 `docker-compose.template.yaml` 은 single 판과 바이트 동일이라, **multi 통로에 Ray 워커 서비스가
없는 compose** 가 들어간다.

**이 사고는 이미 한 번 났다** — 정본 JSON 의 `_meta.why_updated` 가 *"8/11 렌더가 legacy `--template`
경로로 새어나갔다"* 고 자백한다. 원인이던 해소값 SSOT 는 2026-08-13 에 고쳐졌고 **문서는 안 고쳐졌다.**

---

### ⑨ 델타 판정의 게이팅이 코드에 없다 — `judge_version_delta.py` 는 프로그램 호출자가 0건이다

**위치**: `SKILL.md:43,60` · `.claude/rules/workflow.md` spine · `scripts/render_dockerfile.py` 전역
**등급**: 실측

- `SKILL.md:43` — *"**`UNDETERMINED` 면 렌더 진입 금지.**"*
- 실재 — `render_dockerfile.py` 에서 `UNDETERMINED` 는 `_inherit_eligibility` 안 **한 곳**뿐이고, 그
  함수는 **원장에 키가 등재됐을 때만** 호출된다. 키가 `VALIDATED_SOURCE_BUILD_KEYS` 에 있으면
  `_patch_guard` 가 `resolved` 를 **보지도 않고** 즉시 통과시킨다.
- `judge_version_delta.py` 는 저장소 전체에서 **프로그램 호출자 0건**이다. 스스로도 `:126` 에
  *"게이팅은 **호출부가 한다**"* 라고 적어 두었는데, 그 호출부가 없다.

**비대칭 지적**: 같은 SKILL.md §0.0 은 Flag 게이트에 대해 *"결정론 백스톱"* 을 요구하고 render 에 실제로
bake-in 했다(`:1112`). 델타 게이트에만 그 백스톱이 없다.

**그리고 게이트가 살아 있어도 도달 불가다**: canonical 렌더는 `--resolved` 를 금지하고
`current-production-resolution.json` 만 읽는데, **그 파일에 `upstream_delta` 키가 없다**(실측 —
top-level keys 에 부재). 사람이 게이트 ①.6 을 정상 이행해 원장에 정당한 항목을 등재해도 렌더가
`attestation_present` 실패로 `exit 1` 스탠자를 낸다. 가드가 제시하는 처방("judge 를 재실행해
resolved.json 을 갱신하라")은 **canonical 경로에서 적용 불가**다 — 남는 선택지가 legacy 유출뿐이라
D3 법칙("경로를 고친다")의 압력이 정확히 ⑧ 방향으로 향한다.

> 방향 자체는 fail-closed(안전)라 파괴는 없다. 문제는 **출구① 메커니즘 전체가 정본 경로에서 도달
> 불가**이며, 사람을 우회로 민다는 것이다.

---

### ⑩ teardown 이 살아 있는 서빙의 예산 선언·워치독을 노드 전역으로 회수한다

**위치**: `scripts/multinode_serve_smoke.sh:646,373-374,396-397` · `scripts/single_serve_down.sh:98-112`
**등급**: 실측(현재 도는 프로세스 확인)

**(a) docker 불통이 "master EXITED" 로 읽힌다.**

```bash
docker ps --filter name="$MC" --filter status=running -q | grep -q . \
  || { echo "[mn] master EXITED"; ...; break; }
```

출력이 없다는 사실 하나로 "죽었다"고 단정한다 — `timeout` 도 `-m` 도 없다. 대형 가중치 로드 중(=워치독이
가장 필요한 구간, 기본 15분·실측 권장값이면 50분) `docker ps` 가 한 번 실패하면
`break → RESULT=2 → teardown_serve` 로 이어지고, 그 끝의 `:396-397` 이 **양 노드의 `clear-budget` 을
무조건 실행**한다. **컨테이너는 여전히 로딩 중인데 협역 워치독은 내려갔고 예산 선언은 지워진다** —
*"선언 없으면 arm_ceiling 무한대 = 무방비"* 상태이며, 상시 ETA 워치독은 선언된 바닥 없이 계속 돈다.

**(b) 회수 실패가 침묵한다.** `:373-374` 는 `>/dev/null 2>&1` + 반환값 미검사(`set -e` 없음),
`:396-397` 은 `| sed` 로 파이프라인 상태가 버려진다. 서브가 순간 도달 불가라 `down` 이 실패해도
`[mn] --down 완료` + 종료코드 0 이 나오고, 슬레이브 컨테이너는 GPU 를 쥔 채 그 노드의 선언만 사라진다.

**(c) 회수 사정거리가 config 를 보지 않는다.** `single_serve_down.sh:110` 은 argv 위치 판정으로
`mem_watchdog.sh` **스크립트 이름**만 매칭한다 — trial/config 범위가 없다. 실측: 지금
`mem_watchdog.sh vllm_trial03 10240 2`(PID 876456)가 3시간 24분째 돌고 있으며, 다른 세션 소유다.
게다가 `kill "$p" 2>/dev/null && { ... n=$((n+1)); }` 라 **EPERM 이 카운트도 메시지도 없이 삼켜지고**
`:112` 가 무조건 `DONE` 을 찍는다. "회수 대상 없음"과 "회수 실패"가 똑같이 `0건 · DONE` 이다.

**이 스크립트의 존재 이유가 2단계에서 성립하지 않는다** — 파일 헤더가 *"각 단계의 DONE/SKIPPED(사유)/
FAIL 을 반드시 출력한다"* 를 계약으로 선언했고, SKILL.md 도 같은 문장을 싣는다.

---

## 심각도 2 — 주요 발견

**해소 체인**

- **`regen_requirements.py:56-58` 이 `extra` 이외의 환경마커를 전부 무단 제거한다**(실측):
  `platform_machine == "x86_64"` · `sys_platform == "win32"` · `python_version < "3.10"` 게이트가
  사라져 **조건부 의존이 무조건 의존이 된다**. 타깃은 aarch64 인데 x86 전용 패키지가 생 라인으로
  들어가면 빌드가 죽고, 재조정 루프는 **원본 마커를 복원할 방법이 없어** cap 을 소진한다.
  도크스트링은 `extra` 제외만 정당화하고 이 동작은 언급조차 없다.
- **`regen_requirements.py:74` — 빈 `Requires-Dist` 를 "0개"로 확정하고 기존 파일을 덮어쓴다**:
  `m.requires("vllm") or []` → 길이 가드 없이 `open(out,"w")` → `0 packages` 를 **성공 메시지로** 출력.
  `--use-installed` 는 컨테이너 안 정합 재생성 모드라 여기서 빈 파일을 낳으면 다음 빌드가 죽는다.
- **`resolve_ngc_tag.py:41-63` — 프로브 실패 3종이 "그 태그 없음"과 융합되고 stderr 는 전량 폐기**:
  docker 미설치·데몬 정지·프록시 차단·타임아웃이 전부 `status:"unavailable"` 이 되고, 전 후보 실패 시
  exit 4 + `"매칭 NGC 태그 없음(완전일치)"` 라는 **한 번도 조회하지 못한 사실에 대한 확정 판정**이
  나간다. 그 메시지가 지목하는 라우팅의 처방은 **"해소값 override"** 다 — 헌법 §안전 경계
  "무증거 NGC/repo 오버라이드 금지"를 침식한다.
- **`resolve_build_track.py:161` — 소비하는 manifest 키의 생산자가 0건**: `compute_capability`·`sm_arch`·
  `gpu_compute_capability`·`cuda_arch` 네 이름을 manifest 에 쓰는 코드가 없다. `--manifest` 기본값도
  CWD 상대라 실제 경로(`output/<t>/`)와 어긋난다. 결과: `torch_cuda_arch` 는 항상 `null`.
- **미인식 SM arch 가 경고 없이 통과한다**(실측): `resolve_build_track.py --sm-arch "NVIDIA GB10"` →
  `"torch_cuda_arch": "NVIDIA GB10"`, EXIT=0, note 없음. **누락은 표시되고 오염은 표시되지 않는다** —
  반대로 되어 있어야 한다. 그리고 `null` 은 `render_dockerfile.py:482` 의 `.get(…, "")` 를 통과해
  문자열 `"None"` 이 되어 `TORCH_CUDA_ARCH_LIST=None` 이 렌더된다. `resolve_build_track.py:242` 의
  note 는 *"source-build 트랙이면 … 렌더 fail-loud"* 라 **단언하는데 그 가드는 실재하지 않는다.**
- **검증된 `asset_name` 이 버려지고 템플릿이 wheel 파일명을 재조립한다**: `resolve_wheel.py` 의 존재
  이유가 도크스트링에 *"404 함정 방지 … 추측하지 않고 실제 자산명에서 읽는다"* 로 적혀 있는데,
  `asset_name`·`wheel_url` 소비자는 **0건**이고 템플릿은 부품 3개 + 하드코딩 `cp38-abi3` 로 URL 을
  재조립한다. 4종 판정표의 하드코딩 **결함** 칸("파생 가능한데 손으로 적은 것")을 넘어 —
  **이미 파생·검증돼 파일에 들어 있는 값**을 버린다.

**렌더·게이트**

- **Flag 게이트가 렌더 시퀀스 3단계 중 1단계에만 걸린다**(실측): `_require_terraform_flag` 는 `:1112`
  인데 `--nccl-envfile`(1072)·`--cluster-envfile`(1082)·`--materialize-configs`(1092)·
  `--materialize-env`(1101)가 **전부 그 앞에서 return** 한다. SKILL.md spine 4 가 이 셋을 하나의 시퀀스로
  묶는데 게이트는 1/3 만 덮는다.
- **렌더 시퀀스 누락이 렌더·serve 양쪽에서 침묵한다**: `--materialize-env` 를 빠뜨려도 렌더러는
  성공을 보고하고, 스모크는 `[ -f "$PENV_FILE" ]` 실패 시 `MOUNTVARS=""` 로 조용히 진행한다
  (`set -e` 없음). compose 가 기본값을 마운트해 컨테이너가 모델을 못 찾는다 — **그 결함을 막으려고
  만든 배선이 부재 시 아무 소리도 내지 않는다.** 바로 위 주석이 이 결과를 정확히 서술해 두었다.
- **`--canonical-kind` 가 `--topology` 를 manifest 와 대조하지 않는다**: single manifest 로 multi
  compose 를 single 통로에 렌더하는 조합이 통과한다(헌법 §산출물 통로).
- **pyyaml 부재 시 Flag 게이트가 거짓 사유로 차단한다**: `except Exception` 이 ImportError 와 YAML
  문법 오류를 함께 삼키고, flat 폴백 파서가 `terraforming:` 중첩 블록을 통째로 버려
  `complete: true` 가 멀쩡한 manifest 에도 `exit 4 "Flag 미발급"` 이 나간다. 처방대로 재스캔해도
  통과할 수 없다. (**이 호스트에는 pyyaml 6.0.1 이 있어 현재는 잠복이다** — 그러나 이 파일은
  docstring 에서 "stdlib 만"을 이식성 계약으로 선언했고 배포 단위는 "누구든 클론 후 테라포밍"이다.)
- **`build_patches_src` 스탠자에 외부 게이트가 없다**: 주석은 *"각 스크립트는 스스로도 자기
  게이트한다(**이중 방어**)"* 라 하나 루프에는 조건이 없어 판정 권위 전부가 각 `.sh` 안의 단일점이다.
  자기게이트를 빠뜨린 스크립트를 통로에 놓으면 stock 빌드에도 적용되고 이미지 태그는 stock 그대로다.
  (**긍정**: 위상 자체는 정상 — `RUN` 이 `pip install -e .` 앞이라 "컴파일 전"이 순서로 보장되고
  `|| exit 1` 로 fail-loud 하다.)

**서브 배달·동기화**

- **`SUB_WORK_DIR` 미해소가 메인 저장소 절대경로로 조용히 폴백한다**: `SUB_HOST` 미해소는 exit 4 로
  닫는데 `SUB_WORK_DIR` 만 `${SRC%/}` 로 대체된다. 그 값이 `rsync --delete` 목적지이자 롤백의
  `rm -rf` 인자다. 이 프로젝트의 **독립 근거 규칙**(루트경로 폴백 금지 = fail-loud 게이트, manifest
  포인터 원칙)이 정확히 이것을 금지한다 — 4종 안티패턴 완화의 명시적 **적용 예외** 항목이다.
- **브랜치 가드가 하드코딩 2파일에만 걸려 부분 드리프트에 파괴 안내를 그대로 낸다**: 판정 근거가
  `BAND2_TOP` 배열에서 파생되지 않고 `Dockerfile`·`docker-compose.yaml` 두 이름으로 손저작돼 있다.
  `requirements.txt`·`build_patches*` 만 결손된 부분 드리프트에서는 STOP 가드가 발화하지 않고
  `ALLOW_DELETE=<n> 로 재실행` 안내가 나간다 — *"안내를 그대로 따르면 파괴가 완성되는 형태"*로
  이 저장소가 명문화한 D5 안티패턴 그대로다.
- **`policy:SUB_GIT_LOCAL_ONLY` 검사가 실패를 "확증"으로 번역한다**: `REMOTES="$(sub_run 'git remote'
  || true)"` 뒤 빈 문자열이 곧바로 `✅ origin 0 (로컬 전용 확증)` 이 된다. 아무것도 확인하지 못한 상태가
  헌법 불변식의 통과 선언이 되는 **거짓 어테스테이션**이다.
- **오버레이 미배달이 성공으로 보고되고 체크섬 검증이 0건 검사로 통과한다**: `deliver_overlay` 는
  staging 부재를 `(info)` + `return 0` 으로 처리하고, `verify_checksums` 는 소스에 없는 파일을 전부
  `continue` 하므로 **비교를 한 건도 하지 않고 PASS** 할 수 있다. 검증 건수를 세는 단언이 없어
  "3건 검증·15건 정당 부재"와 "0건 검증·staging 소실"이 구분되지 않는다.
- **롤백 인벤토리가 조용히 축소된다**: `find … -print0` 프로세스 치환의 종료 상태를 어디서도 검사하지
  않는다(`pipefail` 은 파이프라인이 아니므로 무력). 읽을 수 없는 하위 디렉터리 하나면 백업 목록이 짧아지고,
  이후 `rsync --delete` 가 **백업되지 않은 파일을 지운 뒤 롤백이 "성공"을 보고**한다.

**검증기 자체**

- **`validate_runtime_patch.py` 는 문법 검증조차 없다**: `compile`/`ast.parse`/import 시도가 0줄이라
  파이썬이 아닌 텍스트도 stamp/verify PASS. 이전 모델 패치를 새 이름으로 복사해도 통과하므로
  **`policy:RUNTIME_PATCH_NO_CARRY_FORWARD` 가 금지하는 바로 그 행위가 검증기의 축복을 받는다.**
  실제 집행 명제는 "no carry-forward"가 아니라 "stamp 이후 파일이 안 바뀜"이다.
  (헤더의 *"Cryptographically bind"* 도 과장 — 키·서명 없이 SHA-256 재계산 일치이며 누구나 재실행 가능,
  위변조 방지가 아니라 드리프트 검출이다.)
- **`check_spec_layout.py` 의 도크스트링 계약을 같은 파일의 호출부가 어긴다**: *"순수 파서다 — 못 읽으면
  None 을 돌려주고 **호출부가 fail-closed 처리**한다"* 인데, 호출부가 곧 이 파일의 `main()` 이고 SKIP +
  exit 0 이다. 파일 부재·권한 오류·블록 스칼라 표기가 전부 "검사 비대상"으로 간다. 상위 소비자도
  rc≠0/8 을 "차단하지 않음"으로 넘긴다. 로드 13분을 아끼려 만든 초 단위 조기차단이 초록불로 사라진다.
- **`regen_build_patches_src.py` 의 `derive` 가 실패 시 payload 를 파괴한다**: `materialize` 가
  완결성 검사 **전에** `shutil.rmtree(files_root)` 를 한다. 같은 파일의 `unbundle` 은 정확히 이 이유로
  스테이징(`.files.incoming`)을 쓰고 *"기존 payload 를 건드리지 않고 중단한다"* 를 명시한다 —
  **같은 도구 안의 비대칭**이며, payload 는 비추적이라 git 복구가 불가능하다.
- **`smoke_clone.sh` 의 PII 게이트가 축소 모드에서 완전 통과를 선언한다**: `.claude/pii_terms.txt` 는
  비추적이라 **배포본에서 부재가 정상**인데, 그 경우 RFC1918 세 대역 중 하나만 검사하고(email·
  `abs-op-path`·`spark-host` 전무) 결과에 대해 `PASS A4 PII 0: … 0 매치` 를 출력한다. **축소 모드였다는
  사실이 PASS 줄에 없다.** `docs.md` 가 이 형태를 이름 붙여 금지한다 — *"좁은 패턴으로 스캔하고 통과를
  선언하면 그 선언 자체가 거짓이다(2026-07-31 실제 발생)"*.
  (**완화**: 발행 경로의 최종 권위는 `hint_tag.py` 의 독립 4종이며 그쪽은 그대로 강제된다.)

---

## 심각도 3~4 — 계열별 요약

- **종료코드 계약 드리프트**: `check_smoke_model.py` 헤더는 `0/2/3/7` 인데 실제로 `exit 8`(spec-layout
  차단)을 낸다. 소비자 `case` 도 8 을 `*)` 로 흘려 *"NAS/설정 확인 실패"* 로 오분류 출력한다(차단은
  되므로 안전 구멍은 아니나 처방이 로그 본문에만 남는다). `classify_failure.py:81` 의
  `exit(0 if class=="requirements-fixable" else 1)` 은 제3의 class 추가 시 조용히 source-build 로 간다.
- **안내문이 옛 임계값을 말한다**: 예산 게이트 실패 진단이 `arm 상한 floor-8192 … 최소 16384MiB` 인데
  정본은 2026-08-18 에 `3072`/`8192` 로 바뀌었다. **거울 갱신 때 안내문만 누락됐다** — 코드 동작은
  정상이고 사람 판단만 오염되며, 이 메시지는 선언이 거부된 바로 그 순간 표시된다.
- **문서 사다리에서 필수 칸이 빠졌다**: `references/source-build.md` 의 사다리는 4칸
  (`deps → 소스게이트 → 포크핀 → 체크포인트교체`)인데 헌법·workflow 는 5칸으로 **자체 이식**을 포크 핀
  **앞에** 둔다. 헌법이 *"포크 의존은 상시화하지 않는다"* 라 명시하는데 스킬이 그 단을 삭제했다 —
  따르면 포크 의존이 기본이 된다.
- **헌법 앵커 4종이 실재하지 않는다**(실측): `테라포밍-완수 Flag 게이트 따름정리`·`hint 배포 레이어
  따름정리`·`escalation 역루프 따름정리` 모두 `CLAUDE.md` 매치 **0건**. 현행 헌법은 `policy:` ID 로
  개편했고 **이 스킬이 인용하는 `policy:` ID 는 0건**이다. 안전 게이트 근거를 찾으러 간 에이전트가
  아무것도 못 찾는다.
- **`config.yaml` 위치가 갈라져 있고 실제로 쓰이는 쪽이 무시되지 않는다**(실측): `.gitignore:57` 은
  `.claude/skills/**/config.yaml` 을 덮고 `smoke_clone.sh` A5 도 그 두 경로만 검사하는데, **둘 다
  실재하지 않는다.** 실제로 쓰이는 것은 루트 `config.yaml` 이며 지금 `?? `(비무시·커밋 가능) 상태이고
  운영자 NAS 루트 절대경로를 담고 있다. 같은 스크립트의 주석은 루트 `config.yaml` 을 *"gitignore
  대상이라 부재가 정상"* 이라 **단언**한다 — 거짓이다. 그리고 선언된 입력 스키마
  (`target_vllm_version`·`smoke.*.config_name`·`ngc_probe_start`·`reconciliation_cap`)는 실물 파일에
  **한 필드도 없다**. 덧붙여 `ngc_probe_start`·`reconciliation_cap` 은 **소비 코드가 0건**이라
  "cap 소진 시 무한루프 금지"는 에이전트 자율 계수에만 의존한다.
- **`config.example.yaml` 이 신규 배포자를 막는다**: `ngc_probe_start: "26.05"` 인데 프로덕션 베이스는
  `26.07-py3` 이고 후보 생성은 start 에서 **과거로만** 훑는다 — 예시대로 복사하면 현재 프로덕션 베이스에
  **구조적으로 도달할 수 없다**. 스모크 `config_name` 두 개도 저장소에 트리플렛이 없다(전수 0건).
- **`Dockerfile.source-build-dsv4.template` 은 참조자 0건인 드리프트 사본**이다. 2026-08-14/15 에
  신설된 `SM12X_PORT`·`SRC_DEPS_AUTHORITY` 게이트가 이 사본에는 없고, 정책 검사도 정본 템플릿만 읽어
  **게이트 사정거리 밖**이다.
- **미지 인자를 조용히 무시한다**: `--keepup` 오타 → `KEEP=0` → 상주시키려던 서빙이 teardown 된다.
  같은 파일이 `--down` 조합 모순은 정성껏 fail-closed 하고 `single_serve_down.sh` 는 미지 인자를
  exit 3 한다 — 규율이 갈렸다.
- **`serve_runner.sh` 의 런타임 패치 arming 이 로그 한 줄 없이 건너뛰어진다**: `[ -f arm_patch.sh ]`
  실패 시 무출력. `workflow.md` 3+1+1 절이 이 슬롯에 대해 *"런타임 슬롯은 '먹었는지'를 반드시 로그로
  실증하라"* 고 명시적 규율을 세운 자리다(발화 0회 불발 사례가 근거). 그리고 `arm_patch.sh` 의
  `runtime patch armed` 는 `.pth` 를 **쓴 직후** 출력되므로 *의도*의 선언이지 *효과*의 확인이 아니다.
- **클러스터 준비 판정에 `"2"` 가 하드코딩돼 있다**(`serve_runner.sh`): manifest 가 TP·노드 수의 단일
  권위인데 파생하지 않고, 한 줄에 같은 개념이 두 번 손으로 적혀 있다.
- **`resolutions.json` 이 "무엇에 대해 파생됐는지"를 담을 칸이 없다**(실측): 0.27.0 판과 0.27.1 판이
  `variant_id` 한 줄(diff 4줄)을 빼고 **바이트 동일**하다. 108 파일 항목·counts·`derived_with_git` 전부
  같다. 재파생된 것인지 복사 후 id 만 고친 것인지 **문서 자체로는 영원히 구별할 수 없다.**
  같은 파일 안에서 `counts` 는 언더스코어(`fork_verbatim`), `files[].strategy` 는 하이픈
  (`fork-verbatim`)이라 **같은 열거가 두 표기법**으로 적혀 있고 일치를 검사하는 코드가 없다(실측).
- **`hint_tag.py` 가 gitignored `resolved.json` 을 읽고 없으면 조용히 진행한다**: fresh clone 에서
  CUDA·TORCH_PIN·NGC_TAG·BUILD_TRACK·CPU_ARCH 가 전부 `<채워넣기>` 인 레시피가 경고 없이 발행될 수
  있다. **hint 태그는 배포 산출물**이다. 관련해 `docs/benchmark/*.yaml` **44건 전부가
  `ngc_base_tag: N/A`** 인데(실측), 이는 "측정 못 했다"가 아니라 **"이 키를 쓰는 코드가 존재한 적
  없다"** 이며 둘은 완전히 다른 사실이다(헌법 §부재와 결측의 구분).
- **`VARIANT=` 규약은 읽는 코드가 0건이다**(실측): `workflow.md` 가 신규 변종의 정본 규약으로
  선언했으나 소비자가 없다. 규약대로 적으면 **stock 이미지가 조용히 빌드된다** — 그 규약이 고치려던
  "그림자 배달 경로"를 다른 형태로 되살린다. workflow.md 자신이 §철회 조건을 두었으므로, 배선하거나
  철회하거나 둘 중 하나여야 한다. **미배선 규약을 정본으로 두는 것이 가장 나쁘다.**

---

## 관통하는 패턴 4종

### 1. 표시는 있는데 집행이 없다

이 스킬은 출처 필드를 성실히 단다 — `delta_source`·`applicable`·`replay_of`·`bundle_sha_source`·
`quant_source`·`sm_arch_source`·`provenance`. 그런데 **게이트가 그 필드를 읽지 않는다.**
`_inherit_eligibility` 의 술어 중 `delta_source`·`applicable`·`replay_of` 를 보는 것은 **하나도 없다**.

가장 날카로운 사례: `--provenance` 를 안 주면 axis_B 가 `{"verdict":"NO_IMPACT","applicable":False}` 를
내는데, 상속 술어는 `verdict` 만 본다. **플래그 하나 누락으로 침묵-되돌림 프로브가 한 번도 계산되지
않은 채 "교차 없음"으로 세탁되고 상속이 승인된다.** 구분 필드는 정직하게 달려 있다 — 읽는 자가 없을 뿐.

> §결정론 규율은 **표시**를 요구한다. 표시된 값을 소비하는 술어가 없으면 표시는 장식이다.
> 처방은 새 설계가 아니라 **이미 있는 필드를 술어에 연결하는 것**이다.

### 2. 주석이 존재하지 않는 가드를 선언한다

- `resolve_build_track.py` — *"source-build 트랙이면 렌더 fail-loud"* → 그 검사가 없다
- `check_spec_layout.py` — *"호출부가 fail-closed 처리한다"* → 같은 파일 호출부가 exit 0
- `BlobReader` 도크스트링 — *"`absent` 와 `error` 를 구분한다"* → git 분기가 안 한다
- `regen_build_patches_src.py` 헤더 — *"적용 실패 시 그 사실을 지목해 실패한다"* → log 경고뿐
- `Dockerfile.source-build.template` — *"각 스크립트는 스스로도 자기 게이트한다(이중 방어)"* → 루프에 조건 없음
- `sync_to_sub.sh` — *"reject a stale 0644/0664 source **before transport**"* → 소스측 단언 3개가
  전부 항진명제다(직전 호출이 무조건 `chmod 0755` 한다). **주석과 무력화 코드가 같은 커밋에서 태어났다** —
  시차가 아니라 설계 시점의 자기모순이다.

직전 감사의 결론이 그대로 재현된다: **단언이 검증을 대체하면 깨진 순간을 아무도 모른다.** 여기서는
한 겹 더 나쁘다 — 리뷰어가 주석을 믿고 코드를 안 본다.

### 3. 기계 평면에서 "통과"와 "미실행"이 같다

로그는 사람 평면에 있고, 결정은 기계 평면(종료코드·`REQ_MIB`·빈 문자열)에서 난다. ③·⑩·⑦·⑨가 전부
이 형태다. 처방은 제거가 아니라 **구분** — 미실행 전용 종료코드, `SKIPPED(사유)` 의 stdout 계약,
검증 **건수**를 세는 단언.

### 4. 문서가 코드보다 낡으면 정식 경로가 우회 경로를 가리킨다

코드는 2026-08-24 까지 진화했고 `references/` 는 **2026-07-27 에 멈췄다.** 이 시차가 심각도 1 열 건 중
셋(⑧·⑨의 절반·사다리 누락)을 만들었다. 그리고 반대 방향도 있다 — `workflow.md` 의 "미검증 슬롯" 표기는
**실제보다 비관적**이라 자체 이식 칸을 도박으로 오평가하게 만든다(아래).

---

## 역방향 발견 — 문서가 실제보다 비관적인 곳

`workflow.md:87,102` 는 `build_patches_src/`(pre 빌드패치 슬롯)를 **"⚠ 미검증 슬롯 — 그 위에서 서빙에
성공한 사례가 아직 없다(이식 3회 실패)"** 로 표기하고, 만료조건을 *"성공 사례가 나오면 이 표기를
지운다"* 로 스스로 적었다.

추적 원장 `.claude/policies/arch_variant_ledger.json` 실측:

| variant | status 발췌 |
|---|---|
| `…-sm12x-port-0.27.0` | **빌드 완료 · 서빙 스모크 PASS · full 벤치 PASS**(decode 36.87 t/s · 인증서 발행) |
| `…-sm12x-fork41834-0.27.0` | **서빙 스모크 PASS · full 벤치 PASS**(decode 37.05 t/s · 인증서 발행) |
| `…-sm12x-port-0.27.1` | 재파생·정적검증 완료 · **양노드 빌드 · 서빙 스모크 PASS · full 벤치 PASS**(decode 37.36 t/s) |

**만료조건이 3중으로 충족됐고 표기가 남아 있다.** 미승격 사유는 슬롯이 아니라 `regression_evidence`
부재다. 그리고 자체이식(37.36)이 포크핀(37.05)에 뒤지지 않는다 — 헌법의 *"포크 의존을 상시화하지
않는다"* 를 실측이 뒷받침한다. 표기를 남겨 두면 **사다리 3칸을 건너뛰고 포크 핀으로 조기 이동**하도록
유도하므로, 이 낡음은 안전 방향이 아니다.

부수로, `workflow.md` 가 pre 슬롯 owner 문서로 지목한 `references/source-build.md` §4 는 제목부터
`빌드-바깥 의존 패치 (build_patches/)` 로 **post 슬롯 전용**이며 `build_patches_src` 문자열이 그 문서에
**0건**이다 — pre 슬롯은 소유 문서 섹션이 없다.

---

## 검증 커버리지 — 변이 실험으로 측정한 검출력

자체검사 4종을 실행하고(전량 PASS), 픽스처에 **변이 9종**을 주입해 검출력을 실측했다.

| 변이 | 골든 픽스처 | 음성 픽스처 | self-test |
|---|---|---|---|
| M1 `docker/**` 평면 규칙 삭제 | 검출 | 검출 | 검출 |
| M2 `vllm/**` → BUILD_INPUT | 검출 | 검출 | 검출 |
| M3 `csrc/**` 삭제 | 검출 | 검출 | 검출 |
| M4 `pyproject.toml` 삭제 | 검출 | 검출 | 놓침 |
| M5 VERDICT_RANK 전도 | 검출 | **놓침** | 검출 |
| M7 unknown → NO_IMPACT | **놓침** | 검출 | 검출 |
| M8 axis_b overlap 무력화 | 검출 | 검출 | 검출 |
| M9 C축 전역 포함 | 검출 | 검출 | 놓침 |
| **M6 `VERDICT_EXIT` UNDETERMINED 4→0** | **놓침** | **놓침** | **놓침** |

**8/9 검출.** 골든과 음성 픽스처는 **상보적**이다(M5 는 골든만, M7 은 음성만) — 둘 다 필요하다.
유일한 사각 M6 는 verdict→종료코드 매핑이며, 그 매핑이 곧 §9의 "UNDETERMINED 면 렌더 진입 금지"를
집행할 유일한 채널이다.

**그리고 이 검출력은 지금 아무 데서도 발휘되지 않는다.** 배포 검증기는 이 스킬을 `--help` 로만 돌리고,
`judge_version_delta.py` 는 그 목록에도 없다. 더 날카롭게는 — 배포 검증기가 두 검사를
*"fresh-clone 실행 검사는 `smoke_clone.sh` A8 소유"* 로 명시 위임하는데, **`smoke_clone.sh` 를 실행하는
코드는 저장소에 0건**이다(경로 문자열 등장뿐). 책임이 빈 곳으로 넘겨져 있다.

| 스크립트 | 행수 | self-test | 배포 게이트 | 안 덮는 위험 경로 |
|---|---|---|---|---|
| `sync_to_sub.sh` | 1394 | 없음 | 없음 | 파괴연산 10곳 전부 |
| `render_dockerfile.py` | 1137 | 있음 | `--help` | 파일 여는 함수 13개/382행(**34%**) · wheel 트랙 전체 · 템플릿 5종 0종 렌더 |
| `judge_version_delta.py` | 973 | 있음+픽스처2 | **미등재** | 라이브 수집층 전체(`collect_delta_api`·`_http_get`) |
| `multinode_serve_smoke.sh` | 763 | 없음 | 없음 | 판정 로직 전부 |
| `regen_build_patches_src.py` | 539 | 없음 | **미등재** | 전량(실행 하네스 0) |
| `sync_branches.sh` | 467 | 없음 | 텍스트 단언 5 | dirty·롤백·trap·삭제캡 전부 |
| `validate_runtime_patch.py` | 107 | 없음 | **미등재** | 문법·발화 검증 0 |

**도달 불가 앵커 1건**(정직성은 확인): `render_dockerfile.py:876-883` 의 상속 원장 검증 4건은
`INHERITED_SOURCE_BUILD_KEYS` 가 `{}` 라 **0회 반복**한다 — 등재하는 그 순간이 첫 실행이다.
다만 배너가 "원장 항목 0건"을 스스로 밝히므로 **조용한 도달 불가는 아니다.**

**그리고 이 저장소는 지금 이미 RED 다**(실측): `CLAUDE.md:58` 이 불변식 A 의 그라운딩 정본으로 지목한
`docs/report/node-identity-topology-grounding.md` 가 **실재하지 않는다**. 커밋 `d9fbdfb`(report 13건
rename)에서 `harness_26082218_08_32_노드_정체성_토폴로지_그라운딩.md` 로 개명됐는데 헌법 포인터가
갱신되지 않았다. `smoke_clone.sh` A3 는 이것을 dangling 으로 FAIL 하도록 만들어져 있고, **아무도 돌리지
않아서 통과하고 있다.**

---

## 타입 설계 등급 (기준선 = `node_role_contract.py` = 5)

| 구조 | 캡슐화 | 불변식 표현력 | 유용성 | 집행력 | 근거 |
|---|---|---|---|---|---|
| `resolved.json` 본체 | 1 | 1 | 3 | 1 | 스키마·생산자·검증자 전부 부재. 소비자 6곳이 각자 다른 키 집합을 가정 |
| `upstream_delta` attestation | 3 | **5** | **5** | 3 | 축별 verdict·`provenance`·`unknown[]`·`limitations` 명시가 모범. **attest 대상과 결합되지 않음** |
| `_inherit_eligibility` + 원장 | 4 | **5** | 3 | **5** | 이 스킬 최고. 전건 통과 요구·거부 사유 지목·음성대조 12. **정본 경로에서 도달 불가**라 유용성 감점 |
| `arch_variant_ledger.json` | 4 | 4 | 4 | 4 | 실제 validator 가 필드 allowlist·문법·상태 열거를 fail-closed 집행 |
| 실패 분류(yaml+py) | 2 | 2 | 3 | 1 | 정규식으로 YAML 파싱, 열거 미폐쇄, `else 1` 이 오타를 source-build 로 라우팅 |
| `build_track.decision` | 1 | 1 | 4 | 1 | **부분문자열 검사 + wheel 기본값.** 레이어 커플링 불변식의 유일한 스위치인데 |
| `resolutions.json` | 2 | 1 | 2 | 2 | 파생 근거를 담을 칸이 없어 두 변종이 바이트 동일 |

**근본원인 한 줄**: `.claude/schemas/` 에 이미 6종의 JSON Schema 가 있는데 **주 교환 계약에만 스키마가
없다.** 이 저장소는 스키마를 쓸 줄 알고, 여기에 안 썼다. 리졸버 4종의 출력 키와 `resolved.json` 이
기대하는 경로가 **전부 다르고**(`matched_tag`→`ngc_base.tag`, `rationale`→`reason`, `torch_prefix`·
`sm_arch_source` 는 아예 소실), 그 사이를 사람이 손으로 옮긴다. 특히 `resolve_ngc_tag` 와
`resolve_wheel` 이 **같은 이름 `cuda_version` 으로 전혀 다른 것**(베이스 CUDA `13.3.1.008` vs wheel
자산 `130`)을 뱉는다.

> `judge_version_delta.py` 는 이미 *"사실 행은 손저작 금지 — 전사 오류가 구조적으로 불가능해진다"* 를
> 확립했다. **같은 규율을 해소값 자체에 적용하지 않은 것이 공백이다.**

---

## 잘 설계된 것 (이 저장소 기준 모범)

- **`_inherit_eligibility`** — 전건 통과 요구, 거부 사유를 이름으로 Dockerfile 에 각인,
  `provenance == "measured"` 강제(헌법 "합성 금지·측정 > 공식"이 집행 가능한 형태로 구현된 드문 사례),
  `VALIDATED`(set)와 `INHERITED`(dict)를 **다른 자료구조**에 담아 *"둘을 섞으면 그 구분이 데이터에서
  사라진다"* 를 주석으로 밝힘, 상속의 상속 구조적 차단, 음성대조 12 + 양성대조 1 과 그 이유 명시.
- **`upstream_delta` 스키마** — `NOT_IMPLEMENTED` + `excluded_from_global` 로 미구현을 데이터에 명시,
  `limitations[]` 가 판정과 함께 이동, `PLANE_RULES` 닫힌 열거 밖은 `UNKNOWN_PLANE`→UNDETERMINED
  fail-closed, 오분류 비용의 비대칭을 근거로 보수적 넓힘을 선택했다고 주석에 기록,
  `VERDICT_EXIT` 를 라이브·재생 **양 분기에 동일 적용**하고 그 이유까지 적음.
- **`multinode_serve_smoke.sh` 의 예산 게이트** — 미검출 시 원인 후보 서술 + 영구 이벤트 기록 +
  워치독 disarm + exit 4, 그리고 **"로드는 0초도 시작하지 않았다"** 명시. 이 감사에서 발견한 가장 잘 된
  fail-closed 다.
- **`regen_build_patches_src.py` 의 `run()`** — rc 검사 + **stderr 를 실패 메시지에 실어** 실패시킨다.
  이 저장소에서 subprocess 를 다루는 모범이며, `resolve_ngc_tag`·judge git 분기가 따라야 할 형태다.
- **`sync_to_sub.sh` 의 `verify_destination_retirement_consumers`** — `if hits="$(…)"; then :; else
  "scanner/transport failed"; return 98; fi` 로 **전송 실패와 "매치 없음"을 명시적으로 가른다.**
  이 파일에서 유일하게 그 구분을 제대로 한 곳이자, ⑦의 처방 원형이다.
- **`check_spec_layout.py`** — *"PASS 가 서빙 성공을 뜻하지 않는다. FAIL 만 신뢰하라 — 그게 이 검사의
  값어치다"* 를 docstring 에 못박고 tripwire 를 자기고지한다. 술어의 **방향성 있는 신뢰도**를 타입
  문서에 담은 드문 예(호출부 결함은 별건).

---

## 이 감사 자체의 결함 (기록)

- **인메모리 프로브 2건을 내가 틀린 인자 순서·자료형으로 호출했다.** `build_context(manifest, resolved)`
  를 반대로 넘겨 전 케이스가 `KeyError` 로 죽었고, `resolutions.json` 의 `files` 를 리스트로 가정했다
  (실제는 dict). 둘 다 재실행으로 교정했으며, **에이전트 보고를 액면 그대로 받았다면 내 실패를
  코드 결함으로 오인했을 것**이다.
- **에이전트 주장 1건을 과장으로 정정했다.** "render 의 A2A 게이트가 recipe 보다 약하다"는 절반만
  참이다 — `EASY_VLLM_A2A_DELEGATED` env override 는 recipe 에도 **먼저** 있다. 유효한 절반(게이트가
  렌더 시퀀스 1/3 만 덮음)만 본문에 실었다.
- **`output/single/Dockerfile` 이 비무시인 것을 결함 후보로 올렸다가 반증했다** — `.gitignore:125` 의
  의도적 재포함이다. 반증 대조를 안 했으면 위양성을 발행할 뻔했다.
- **동적 경로는 거의 못 봤다.** 진행 중 작업 보호를 위해 빌드·서빙·동기화·teardown 을 하나도 실행하지
  않았다. `sync_to_sub.sh` 1394행의 파괴연산, 스모크 판정 로직, 라이브 델타 수집층은 **정적 독해와
  대조군 비교만** 했다. ⑩의 (a)(b)는 코드 대조이고 (c)만 실측이다.
- **`seed/` 백업 사본이 부정 주장을 두 번 세게 만든다.** 모든 줄 번호는 추적본 기준이며, 구버전 사본에
는 같은 결함이 다른 줄에 있을 수 있다.

---

## 권고 (비용 대비 차단력 순)

1. **`failure_patterns.yaml` 3줄 순서 교환** — `source-build-class` 3블록을 광역 2블록 앞으로.
   지금 이 순간 잘못된 판정을 내고 있고, 재현이 즉시 가능하며, 파괴적 변경이 없다.
2. **배포 검증기의 `--help` 루프에 자체검사 4종 추가** — `judge --self-test`, `--check-fixture` ×2,
   `render --self-test`, `resolve_build_track --self-test`. 변이표가 보여주듯 **이미 검출력이 있고
   지금은 아무도 돌리지 않을 뿐이다.** 이 감사에서 가장 저렴한 큰 이득이다.
3. **`build_track.decision` 닫힌 열거 + 최상위 stale `wheel` 블록 제거** — 둘은 한 쌍이다. 하나만
   하면 다른 경로로 샌다.
4. **접두어 대조 술어 1개 추가** — `torch.pin` ↔ `ngc_base.pytorch_build_version`. 두 값이 이미 같은
   파일에 나란히 있어 비용이 거의 0이고, 헌법 불변식이 처음으로 **집행 가능**해진다.
5. **`sync_branches.sh` 에 dirty 게이트** — `sync_to_sub.sh` 가 원격에 대해 이미 하는 것을 로컬에
   대칭 적용. 지금 워킹트리에 재현 조건이 있다.
6. **`CLAUDE.md:58` dangling 포인터 교정** — 한 줄이면 `smoke_clone.sh` A3 가 GREEN 이 된다.
7. **RAM 게이트 미실행 전용 종료코드 + single 통로 배선** — ③④는 같은 처방(게이트를 `--topology
   single` 로도 부르고, 미실행을 통과와 구분)으로 닫힌다.
8. **`resolve-and-render.md` §2.5 를 `--canonical-kind` 로 재작성** — 심각도 1 중 ⑧ 전부와 ⑨ 절반이
   여기서 닫힌다. 그리고 정본 자산 스키마에 `upstream_delta` 칸을 열어 출구①의 도달 불가를 해소한다.
9. **`workflow.md` 의 "미검증 슬롯" 표기 철회** — 만료조건이 3중 충족됐다. 낡은 경고가 포크 핀 조기
   이동을 유도한다.
10. **`VARIANT=` 규약 — 배선하거나 철회하거나.** workflow.md 가 이미 §철회 조건을 두었다.

---

## 최종 판정

**부품 품질은 이 저장소 상위권이고, 부품 사이의 배선은 하위권이다.**

`_inherit_eligibility` 와 `upstream_delta` 는 다른 스킬이 참고해야 할 설계이며, 변이 실험이 그 검출력이
**장식이 아님**을 실측했다(8/9). 예산 게이트의 fail-closed 는 이 감사가 본 가장 좋은 형태다. 결함은 그
정교함의 부재가 아니라, **그 정교함이 손으로 옮겨 적는 JSON 과 문서 위에 얹혀 있다**는 층위에 있다.

세 감사를 관통하는 형태가 이제 분명하다. hint-publisher 는 *설계가 있고 배선이 없었다.*
terraforming_node 는 *배선이 있고 전파가 없었다.* upstream-version-watch 는 **검증이 있고 그것을
돌리는 자가 없다.** 세 번 다 처방이 이미 저장소 안에 있었다 — 옆 파일, 옆 함수, 심지어 같은 파일의
바로 아래 줄에.

이 스킬에서 가장 무거운 한 문장은 정본 JSON 이 스스로 적은 것이다:

> *"정본이 실물을 잃고 gitignored resolved.json 이 그림자로 실물을 보유 … 그래서 렌더가 legacy 경로로
> 새어나갔다."*

**사고는 기록됐고, 원인이던 데이터는 고쳐졌고, 그 경로로 사람을 보내는 문서는 그대로다.**
