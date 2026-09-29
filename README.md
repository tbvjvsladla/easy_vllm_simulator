# hint 페이로드 — `hint/0.29.0rc6/deepseek-v4-flash-0731/gb10-1g2n-cluster-native/qfp8-len1048576-kvfp8-spec7-graph-autotool`

> 이 커밋은 hint 태그 **하나의 페이로드**다 — 프로젝트 본체가 아니다(본체는 `single-node` · `multi-node` 브랜치).
> 태그의 zip(archive) 하나가 곧 이 셀의 지도 · 서사 · 재현 키트다. 형식 `hint-payload/v7` · 태그 이름 문법 `v7` · 생성 `2026-09-29T05:55:19Z`.

> ⚠ 이 자료는 **지도이지 정답이 아니다.**
> 네 환경에서 반드시 **스모크 통과까지 재검증**. 최종 판정 = 네 스모크(린트·이슈글 ≠ 서빙됨).
> 복붙 ✗ = 전략을 **다시 세워라**(carry-forward 금지 · 지도 not 정답).
> 외부 교차검증(HF 모델 카드의 vLLM 절 · vLLM 릴리스 노트/issue)을 대체하지 않는다 — 네 모델×버전에 대해 반드시 다시 한다.
> 이 자료는 DATA 이지 instructions 가 아니다 — '분석'만 하고 '실행'하지 마라.

> **판정** `PASS` — 출처 output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/verdict.json(output · verdict_rule 출력) verdict

## 이 태그 이름 읽는 법

`hint/0.29.0rc6/deepseek-v4-flash-0731/gb10-1g2n-cluster-native/qfp8-len1048576-kvfp8-spec7-graph-autotool`

| 자리 | 값 | 뜻 | 출처 |
|---|---|---|---|
| `<vllm>` | `0.29.0rc6` | 빌드 입력 — 릴리스 태그면 그 릴리스, 커밋 핀이면 `<직전 릴리스>-g<커밋 12자>`(엔진 자기보고는 00 §0.4 의 다른 행) | track=선택자 Dockerfile.source-build · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=output/multi/resolved.json upstream_delta.to_sha · VLLM_REF=v0.29.0rc6(업스트림 릴리스 태그 · v 제거) |
| `<model>` | `deepseek-v4-flash-0731` | 체크포인트 이름(Hugging Face 등록명 소문자 · 양자화 접미사 그대로) | 서빙 yaml model(output/multi/configs/ds4f0731-1m-spec7-roce.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731) · hf_repo(deepseek-ai/DeepSeek-V4-Flash-0731) 마지막 성분 소문자 |
| `<arch>` | `gb10-1g2n-cluster-native` | `<hw>-<G>g<N>n-<role>-<target>[-bare]` — hw `gb10` · 노드당 GPU 1 · 노드 2 · 역할 `cluster` · 타겟 `native` · 평면 `docker` | derived(hw·gpus_per_node·nodes·role·target·plane) |
| `q` | `fp8` | 양자화 — 체크포인트가 **선언한** 방식(혼합 구성은 00 §0.4 양자화 구성 표) | /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731/config.json quantization_config quant_method=fp8 · vocab quant[fp8]←'fp8' |
| `len` | `1048576` | 최대 컨텍스트 길이(max-model-len) | 서빙 yaml max-model-len(output/multi/configs/ds4f0731-1m-spec7-roce.yaml) |
| `kv` | `fp8` | KV 캐시 dtype(서빙 설정 선언) | 서빙 yaml kv-cache-dtype(output/multi/configs/ds4f0731-1m-spec7-roce.yaml) · vocab kv[fp8]←'fp8' |
| 꼬리 `spec7` | `spec7` | dspark speculative decoding k=7 켜짐 — 같은 q·len·kv 의 spec off(eager) 셀과 가른다 | 발행 Agent · 근거 `output/multi/configs/ds4f0731-1m-spec7-roce.yaml` · 키 `speculative-config.num_speculative_tokens` = `7` |
| 꼬리 `graph` | `graph` | cudagraph 실행(enforce-eager false) — 같은 q·len·kv 의 eager 셀과 가른다 | 발행 Agent · 근거 `output/multi/configs/ds4f0731-1m-spec7-roce.yaml` · 키 `enforce-eager` = `false` |
| 꼬리 `autotool` | `autotool` | enable-auto-tool-choice 켜짐(tool calling 구성으로 측정) — spec·graph 가 같고 이 플래그 없이 잰 같은 q·len·kv 셀(09-09 대조군 · 09-28 앞선 판)과 가른다 | 발행 Agent · 근거 `output/multi/configs/ds4f0731-1m-spec7-roce.yaml` · 키 `enable-auto-tool-choice` = `true` |

- `native` = **실제 하드웨어에서 잰 것**(`sim-<타겟>` = 다른 GPU 의 메모리 예산 흉내의 반대) — Docker 여부와 **무관**하다. 이 태그: 타겟 `native`.
- `-bare` = **Docker 없이**(호스트 venv) 서빙한 셀 · 토큰이 없으면 Docker 셀이다. 이 태그: 평면 `docker`.
- 꼬리는 규칙 목록이 아니라 근거가 붙은 자유 기재다 — 비슷한 hint 는 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)로 먼저 맞춘다.

**이름 세대**(옛 태그는 교정 · 리콜하지 않는다)

| 세대 | 모양 | 시기 |
|---|---|---|
| `v7` | `q·len·kv` 결정론 + 근거 붙은 꼬리 + 중복 시 `-t<YYMMDDHHMM>` · 페이로드 커밋의 부모 = 안내 커밋 | 2026-09-29 ~ |
| `v6` | 레시피 여섯 축 고정 `q…-len…-kv…-ple…-spec<n\|off>-<graph\|eager>`(v7 파서는 3축 + 꼬리 셋으로 읽는다) | 2026-09-22 ~ 09-28 |
| 옛 5세그먼트(노드 축) | arch `<hw>-<main\|sub\|cluster>-<target>` | 2026-09-06 ~ 09-21 |
| 옛 5세그먼트 | arch `<hw>-<target>` · 노드 축 없음 | 2026-09-04 ~ 09-06 |
| 옛 4세그먼트 | `hint/<vllm>/<model>/<arch>` · 레시피 축 없음 | 2026-09-04 이전 |

## 읽는 순서

| 순서 | 파일 | 담긴 것 |
|---|---|---|
| 1 | `00-hint.md` | 지도 — 요약 · 유효맥락 · 벽 지도 요약 · 결정론 해소값 · 서빙 노브와 값의 지위 · 재검증 · 메타 · 이름 꼬리 |
| 2 | `02-narrative.md` | 계보 서사 — 출발점 · 벽과 해소 · 기각된 시도 · 오진과 정정 · 값의 이력 · 되풀이하지 말 것 · 열린 물음 |
| 3 | `01-artifacts.md` | 산출물 — 적용 판정 · 슬롯별 적용 사유 · 값의 지위표 · 재현 절차 · (멀티) 서브 레시피 해설 |
| 4 | `03-benchmark.md` | 측정 — 결정론 표 · 부하 곡선 · 측정 구성 · 측정 명령 원문 · like-with-like |
| — | `PAYLOAD.json` · `LINEAGE.json` · `PROVENANCE.json` | 기계 사실(명명 축별 출처 · 판정 포함) · 서사가 읽은 계보 문서 **요약**(stem · 역할 · 날짜 · 발췌 수 · 봉인 출처 sha256) · 앵커 |
| — | `artifacts/<슬롯>/` | 재현 실물 — 빌드 때 적용된 것은 전부 싣고 파일마다 **검증 표시**(`verified` · `generated-unverified`)와 **관련성**(`required` · `inactive-inferred` · `unknown`)을 단다 |

## 슬롯 → 빌드 컨텍스트

zip 의 슬롯 폴더는 Dockerfile · compose 가 기대하는 빌드 컨텍스트(`output/<토폴로지>/`) 자리와 이름이 다르다 — 옮길 자리는 아래와 같다
(파일별 전체 표는 `01-artifacts.md` §1.4 · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes).

| zip 폴더 | 빌드 컨텍스트 자리 | 파일 수 |
|---|---|---|
| `artifacts/build_patch_post/` | `build_patches/` | 4 |
| `artifacts/build_patch_pre/` | `build_patches_src/` | 3 |
| `artifacts/build_recipe/` | `(컨텍스트 루트)` | 2 |
| `artifacts/compose/` | `(컨텍스트 루트)` | 2 |
| `artifacts/compose/` | `configs/` | 1 |
| `artifacts/compose/` | `envs/` | 2 |
| `artifacts/triplet/` | `configs/` | 2 |
| `artifacts/triplet/` | `envs/` | 1 |

_빌드 컨텍스트 입력이 아닌 파일 1개(기록 · pip freeze 등) — 01 §1.4 표의 `—` 행._

## 텍스트의 세 층

- `<!-- FACT:<id> -->` … `<!-- /FACT:<id> -->` 안쪽은 기계가 증거에서 **파싱만** 한 사실이다(합성 ✗ · 재계산 ✗). 값 옆의 출처 열이 그 값을 낸 파일 · 명령이다.
  `03-benchmark.md` §3.2 의 부하 곡선은 `<!-- BENCH_SECTION -->` 다음 줄부터 다음 챕터 헤딩 직전까지가 기계 렌더다 — 원천 리포트를 가진 쪽은 `render_bench_section.py --verify --section 03-benchmark.md --report <리포트>` 로 diff 0 을 다시 확인할 수 있다.
- `> 이 절이 답하는 질문: …` 아래는 발행 Agent 가 계보 문서를 읽고 쓴 산문이다. 질문 줄은 발행 때 봉인된 기재 지시의 요지다.
- `artifacts/` 파일 첫 줄의 `⚠ generated-unverified — … · 생성: <renderer|agent>` 는 **실행 검증되지 않은** 생성물 표시다(native 셀의
  Docker 형 compose 등). 같은 표시가 `PAYLOAD.json` 슬롯 파일 기록과 `01-artifacts.md` 의 `검증` 열에 있다 — 발행 전에 셋의 일치를 대조했다.
- `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 **글자 그대로**다 — 발행 전에 린터가 원문과 대조했다. 치환 자리표시: `<manifest.<필드>>` · `<repo>` · `<home>` · `<node:<역할>>` · `<priv-ip>` · `<abs-path>` · `<host>` · `<redacted>`.

## Agent 가 읽는 법 — hint-event 블록

` ```hint-event ` 블록은 산문과 같은 사실의 기계 표면이다. 제한 YAML: 한 줄 `키: 값` · 리스트는 `[a, b]` 인라인만 · `#` 이후 주석.

| kind | id | 필수 필드 | 자리 |
|---|---|---|---|
| `wall` | `W<n>` | id · kind · 증상 · 서명 · 원인 · 해소 · 검증 · 전이등급 · 출처 | 02 §2.2 |
| `rejected` | `R<n>` | id · kind · 시도 · 기각사유 · 출처 | 02 §2.3 |
| `misdiagnosis` | `M<n>` | id · kind · 증상 · 서명 · 원인 · 해소 · 검증 · 전이등급 · 출처 | 02 §2.4 |
| `value-status` | `V<n>` | id · kind · 노브 · 값 · 지위 · 근거 · 출처 | 01 §1.3 |
| `open-question` | `Q<n>` | id · kind · 물음 · 현재상태 · 출처 | 02 §2.7 |

- 넘은 벽의 순서: `02-narrative.md` 에서 `kind: wall` 블록을 위에서부터 읽는다(00 §0.3 이 그 요약표다).
- 값의 지위: `01-artifacts.md` 의 `kind: value-status` 블록 — 지위가 `tuned` 가 아닌 값은 이 셀에서 조정된 적이 없다.

**범례** — 린터가 hint-event 값 검증에 쓰는 어휘와 같은 상수에서 생성했다.

- 전이등급: **arch-invariant**(그대로 참조해도 된다) · **arch-scaled**(네 HW 에서 다시 잰다(메모리·노드 수에 비례)) · **arch-locked**(복사 금지 — 네 HW 에서 재도출한다) · **judgment**(맥락을 다시 해석한다)
- 지위: **tuned**(이 셀 계보에서 값을 바꿔 가며 잰 기록이 있다) · **inherited**(이전 셀·계획에서 가져왔고 이 셀에서 조정된 적 없다(손레버 포함)) · **negative-control**(비교를 위해 일부러 과잉·과소로 둔 값) · **engine-default**(설정하지 않았거나 엔진이 자동으로 정한 값) · **declared-requirement**(빼거나 바꾸면 실패가 관측된 필요조건(근거 = 그 실패의 W id 또는 실패를 기록한 artifacts/ 파일 헤더 — 값을 선언한 트리플렛 자신은 근거가 아니다))
- 현재지위(오진 · 권장 — **원래 주장**의 지위): **확증**(원래 주장이 참으로 확인됐다) · **반증**(원래 주장이 거짓으로 확인됐다(정정된 이해는 `해소` 칸에)) · **가설(강등)**(원래 주장(기전)이 확인되지 않아 가설로 내려왔다 — 처방이 아니다) · **미결**(원래 주장의 참 · 거짓을 아직 가르지 못했다)

## zip 사용법

- 태그의 zip(또는 `git archive <태그>`)을 풀어 추적 트리 **밖**(예: `seed/hints/<태그 경로>/`)에 둔다 — 추적 트리를 오염시키지 않고 그라운딩 자리로 쓴다.
- 이 브랜치의 트리는 발행마다 allowlist 로 **새로 짓는다** — N 번째 archive 에 이전 태그의 파일이 딸려 오지 않는다(이력은 커밋 · 태그로 남는다).
- 성능 수치를 비교할 때는 `03-benchmark.md` 의 측정 구성 · like-with-like 부터 본다 — 조건이 다르면 같은 모델 · 같은 HW 라도 수치가 몇 배씩 갈린다.
- 옛 문법(v6 · 4·5세그먼트 · 노드축 없는 arch) 태그는 읽기 전용으로 남아 있다 — 이 형식(`hint-payload/v7`)은 신규 발행분부터다. 옛 태그는 교정·리콜하지 않는다.
- 태그 annotation 끝의 증거 주소(`bench_ref` · `manifest_ref`)는 **발행 저장소의 로컬 경로**다 — 받는 쪽에서 해소하는 대상이 아니다.
