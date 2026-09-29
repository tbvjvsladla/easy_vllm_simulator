"""hintlib.template — 챕터별 기재 지시(PROMPT) 템플릿 · 기계 사실 블록 · hint-event · 원문 발췌 · 봉인 린터
(plan_26092119 §4.2·§4.4 · SPEC §5.7 · 결정 D1/D2 · 2026-09-21).

무엇을 푸는가 (plan §2.3 원인 1·2·6)
    옛 발행기는 "무엇을 적어야 하는가" 를 구조로 갖지 않고 "무엇이 있는가" 만 검사했다. 태그2 zip 은 여정 44건 중
    가중 43.0% 만 실었고, 사람이 재저작한 사실 3건이 틀렸으며(F7), 값의 지위가 없어 음성대조용 과잉값이 필요조건처럼
    배포됐다(F8). 이 모듈은 산문을 **쓰지 않는다** — 산문은 발행 Agent 가 쓴다. 결정론의 자리는 넷이다(plan §4.2):
    ① 무엇을 읽는가(LINEAGE · lineage 모듈) ② 무엇을 적어야 하는가(템플릿의 PROMPT 지시) ③ 기계가 채우는 사실 블록
    ④ 적었는가 · 인용이 원문과 일치하는가의 검사(이 모듈의 `lint`). 질은 발행 시 사람 Y/N 이다("기계는 빈칸, 사람은 쓸모").

D2 경계 (plan_26090107 §6 — 옛 hint_collect docstring 에서 이관)
    "기계는 '무엇이 일어났는가'를 말하고, 에이전트는 '그것이 무엇을 뜻하는가'를 말한다. 기계 산출을 에이전트가 덮어쓸
    수 없고, 에이전트 산출을 기계가 지어낼 수 없다." — 앞 절반은 사실 블록 재생성 diff 0(`HINT_FACT_DRIFT`), 뒤 절반은
    미저작 표지 잔존 차단(`HINT_AGENT_PLACEHOLDER_RESIDUE`)이 집행한다. 헌법 불변식 B: 누락은 기계가 fail-closed 로
    잡고 거짓은 사람이 리뷰한다.

템플릿 지시 문법 (`templates/{00-hint,01-artifacts,02-narrative,03-benchmark}.prompt.md` — 이 문법의 정본은 템플릿 파일이고
이 모듈은 그것을 **그대로** 해석한다)
    `## <n.m> <제목>`                     필수 챕터(plan §4.2 목록 그대로 · `CHAPTERS` 트립와이어가 대조)
    `<!-- FACT:<id> -->` … `<!-- /FACT:<id> -->`
                                          기계 사실 블록. 템플릿에서는 비어 있고 `render_scaffold` 가 채운다. `wall_map`·
                                          `knob_status` 는 **파생 블록**이다 — 02/01 의 hint-event 에서 `refresh` 가 채운다.
    `<!-- BENCH_SECTION -->`              03 §3.2 부하 곡선 = **닫힘 표지 없는** 기계 영역(다음 챕터 헤딩 직전까지 · 사실 id
                                          `bench_section`). render_bench_section `--verify --section` 의 절 추출 계약
                                          (제목 ~ 다음 `## `)과 바이트가 맞도록 닫힘 표지를 두지 않는다.
    `<!-- PROMPT` … `-->`                 기재 지시. 필드 줄(`키: 값`, 들여쓴 줄은 앞 필드의 계속):
                                            질문 · 읽을 것 · 기재 · 필수 필드 · 금지  (사람·Agent 가 읽는 지시)
                                            블록: <kind>[>=N][, …]    이 절에 올 수 있는 hint-event kind 와 최소 개수
                                            필수 발췌: N              이 절의 `> [원문]` 발췌 최소 개수
                                            조건: <key>=<value>       성립하지 않으면 렌더가 "해당 없음" 줄로 바꾼다
                                            선택: 예                   저작 의무 없음(0.8 comment)
                                          `seal_prompts` 가 블록을 `> 이 절이 답하는 질문: <질문>` 한 줄로 바꾼다 — 지시문은
                                          배포되지 않되 의도는 남는다.
    `<!-- SELFCHECK` … `-->`              저작 자기점검(템플릿 최상단 · 2026-09-22 S2 round 2 F11). 독립 사실 검증의 7부류
                                          (`FACTCHECK_CLASSES`)를 저작자가 먼저 훑는다. 봉인이 **통째로** 지운다(질문 줄도 남기지
                                          않는다 · 배포 ✗) — 봉인 뒤 잔존 = `HINT_SELFCHECK_RESIDUE`.
    `<<AGENT: …>>`                        미저작 자리표시. 저작 뒤 남아 있으면 린트 오류(봉인도 거부한다).
    ` ```hint-event ` … ` ``` `           제한 YAML(아래 `parse_hint_events`). 주석(PROMPT) 안의 형식 예시는 사건이 아니다.
    `> [원문] <stem> §<절>` + `> ` 줄들     원문 발췌. 인용 본문(공백 정규화)이 출처(정규화 · 치환 후 · 공백 정규화)의 부분 문자열.
                                          `§<절>` = 마크다운 출처면 그 문서 제목 줄의 정규화 접두(D-c) · 비-마크다운 출처(로그 · .sh ·
                                          Dockerfile · JSON)면 줄 범위 `§L<a>-<b>`(정규화 뒤 번호 = `hint.py excerpt --lines` · 인용이 그
                                          줄들 안 · 범위 ≤ 인용 줄 + LINE_LABEL_SLACK). 정규화 = `normalize_source`(`\n` 줄 · `\r` 진행
                                          조각은 마지막 · ANSI 제거) — 발췌 도우미와 린터가 같은 함수다(2026-09-22 S2 round 2 F8).
                                          측정 도구 스냅샷(`<name>@<rev12>` = draft 상대 `inputs/sources/<name>@<rev12>` · LINEAGE
                                          evidence_candidates 에 등재된 것만)도 출처다 — 확장자가 `@<rev>` 뒤에 가려지므로 언제나 `§L<a>-<b>`
                                          (2026-09-22 S2 round 3: 2차 저작자가 측정 도구 인자를 원문 없이 "도구가 정했다" 로만 적었다).
    **지시(블록 · 필수 발췌 · 조건 · 선택)는 페이로드가 아니라 저장소의 템플릿에서 읽는다** — 저작자가 페이로드의
    `블록: wall>=1` 줄을 지워 규칙을 벗어나지 못하게(봉인 전 PROMPT 는 템플릿과 글자 그대로 같아야 한다 · `HINT_PROMPT_TAMPERED`).

render_scaffold 가 소비하는 facts 키 (evidence·artifacts·lineage 가 조립 · 모르는 키는 무시 · 없는 값은 `미관측`)
    tag                         str   파생된 태그 이름(필수)
    generated_utc               str   주입 UTC `…Z`(필수 · 벽시계 ✗)
    naming                      dict  PAYLOAD.naming — grammar · segments{축:{value,source}} · axes{축:{value,source}} ·
                                      vllm_build_input{kind,ref,sha,prev_release} · vllm_observed{<키>: 값 | {value,source,label},
                                      <키>_source · <키>_producer · <키>_label · source(옛 단일 출처)} — 00 §0.4 가 키마다 **그 값을 만든
                                      생산자**를 출처 칸에 싣는다(2026-09-22 S2 round 3: 인증서 vllm_version 은 엔진 자기보고가 아니라
                                      sweep_bench.sh 가 IMAGE_TAG 에서 자른 값이었다 · 엔진 자기보고 = 같은 digest 엔진 로그의 기동 배너)
    identity                    dict  model · gpu · vllm(엔진 자기보고) · quant(인증서 N/A 면 명명 축 q · source `naming-axis(q)`) ·
                                      topology(single|multi · 필수) · tp · hf_repo · hf_revision(체크포인트 git HEAD) · base_model ·
                                      base_slug · source{필드:출처}
    build                       dict  track · dockerfile · image_tag · image_digest · vllm_repo · vllm_ref · vllm_sha · torch ·
                                      cuda · ngc · cpu_arch · driver(인증서·스윕 meta driver_version) · (그 밖의 키는 정렬해 추가 행) ·
                                      source{필드:출처}
    plane                       str   docker|native
    applied_set                 dict  status(observed|unobservable) · source · patches[{phase,file,result,result_source,reason,
                                      declared_model_trigger,evidence{static[],loop,markers{status,declared,found,missing}},
                                      script_identity,image_script_sha256,ledger_result_source}] · probes[] · reconstruction{source,
                                      build_args} — result_source 어휘 = `RESULT_SOURCE_MEANINGS`(닫힌 목록 · 밖 = `HINT_RESULT_SOURCE_UNKNOWN`) ·
                                      패치별 판정 근거(evidence · 스크립트 바이트 정체 · 이미지 사본 sha256)는 01 §1.1 '판정 근거' 표
    slots                       dict  {슬롯:{files[],applicable,rationale,evidence{kind,ref,recipe_vs_image[],context_not_shipped[],
                                      selected_revision{path,commit,method,history_match{matched,total}},
                                      selected_revisions[{path,commit,method,history_match,shipped,verified}],
                                      post_measurement_regenerated[]},excluded[{file,why|reason,…}],confidence}} — 01 §1.1 이 슬롯마다
                                      실린 리비전(복수 · build_recipe 와 compose)과 싣지 않은 파일(사유 `post-measurement-regenerated` =
                                      측정 뒤 재생성된 env 형상 — 측정 당시 값은 measurement_env_observed)을 싣는다
    measurement_env_observed    list  evidence.measurement_env_observed — [{key,value,node(main|sub|null),source('<로그>:L<n>'),
                                      kind?,seen_in_logs?,ray_repeated?,value_note?,node_label?}]
                                      측정한 실행의 엔진 로그 env echo(2026-09-22 S2 round 3) → 01 §1.4 표 · 01 §1.5 노드별 접기 ·
                                      ray_repeated(Ray 가 접은 사본 수)가 있으면 한쪽만 관측을 단언하지 않는다
    tool_snapshots              list  evidence.tool_snapshots — [{name,repo_path,git_rev,snapshot_rel('inputs/sources/<name>@<rev12>')}]
                                      측정 시각 이전 마지막 커밋의 측정 도구 원문(draft 상대 · 발췌 출처 토큰 `<name>@<rev12>`) → 03 §3.4
    attestation_scope           dict|None  {config,written_utc,written_utc_source,bound_by,phase,path,scope} — evidence 는 attestation 이
                                      묶이면 늘 준다(scope = config 가 셀과 다르면 "image-build/smoke — not this cell's run" · 같으면
                                      "같은 실행인지 미검증") → attestation 을 인용하는 자리(01 §1.1 탐침 · 01 §1.5 ABI · 00 결손)마다
                                      범위 줄(scope 는 생산자 문구 그대로 — 이 모듈은 범위를 판정하지 않는다)
    qualification               dict  health_200 · inference_observed · sources[]
    measurement                 dict  측정값(파싱만 · 재계산 ✗) · source
    measurement_config          dict  배포 평면 열거·수치 키 + source(자유 서술 `*_source` 는 싣지 않는다 — evidence 몫)
    missing                     list[str] | dict[str,str]   결손 사유코드(dict 면 코드→뜻 · list 면 뜻을 evidence.MISSING_CODES 에서)
    lineage_reading_list        list  lineage.reading_list(LINEAGE) — 02 의 계보 표
    value_status_candidates     list  artifacts.value_status_candidates — {knob,value,candidate_status,source,comment}
    reproduce_steps             list  artifacts.reproduce_steps — {order,phase|step,command(여러 줄 가능),command_source,
                                      observed{start_utc,end_utc,seconds,bound,layer_span{oldest_utc,newest_utc,layers,source,note}},
                                      success(단계별 성공 판정),duration,source} · layer_span = build 단계의 이미지 층 CreatedAt 창(표 아래 줄) ·
                                      FACT 에 절 자리표시(`§3.x`)가 남으면 `HINT_FACT_PLACEHOLDER`
    event_timeline              list  evidence.event_timeline — [{utc,node,kind,label,detail,source}](블랙박스 원장의 이 셀 행 · 시각순)
    event_ledger_spans          list  evidence.event_ledger_spans — [{node,first_utc,last_utc,files,covers_measurement,measurement_end_utc}]
                                      (노드 원장의 관측 범위 · 범위 끝 뒤 = 관측 범위 밖 · FACT_FIX2 G8)
                                      → 01 §1.4 표 + 기동 시도 묶음 · 02 §2.2 기동 시도 요약
    bench_definition            dict  evidence.bench_definition — {current_full_definition(docs.md 원문),source,source_line,
                                      this_repeats,repeats_source,required_repeats,bench_tool,meets_current_full,reasons[]} → 03 §3.5 · 00 메타
    factcheck                   dict|None  hint.py continue 가 넣는 발행 전 독립 사실 검증 요약(status passed|waived|absent|invalid|open ·
                                      checker · author · open[] · waiver) → 00 메타(publish 직후 = None = "미실시")
    sub_recipe                  dict|None  artifacts.sub_recipe(멀티) — role_diff · launch_order · env_forward · image_identity_args ·
                                      per_node_values · preconditions · parity_attestation
    bench_section_md            str|None  render_bench_section 이 렌더한 절(바인딩된 리포트 또는 스윕 색인에서)
    task_class                  str   full_benchmark|hint_map_only (없으면 publication.task_class)
    perf_waiver                 dict|None  manifest.benchmark.perf_waiver — warning_flag 원문을 배포 본문에 싣는다
    campaign                    dict  id · cell · node · mode · (id_source — 재생 전용 대체의 출처 · 2026-09-29 F4)
    approval                    dict  approved_by · approved_utc · source
    prior_approval              dict  재생 전용: 원 발행의 승인(봉인 페이로드 기록) + replay_source — approval 이 없을 때만 메타에 표지와 함께
    publication                 dict  topic · manifest_ref · task_class

호출 순서 (hint.py)
    publish : render_scaffold(repo, facts, payload_dir)        → PROMPT 목록 · facts 스냅샷(`<payload_dir>/../inputs/template_facts.json`)
    (저작)  : Agent 가 `<<AGENT:` 줄을 산문·hint-event·발췌로 바꾼다 · 반복 확인은 refresh → lint
    continue: refresh(payload_dir) → lint(...) 오류 0 → seal_prompts(payload_dir) → lint(..., sealed=True) 오류 0 → brief(payload_dir)
    lint 는 부수효과 0 이다(파생 블록을 고치지 않는다 — 낡았으면 `HINT_DERIVED_BLOCK_STALE`).
    lint 의 `bench_report` = 03 부하 곡선을 렌더한 **그 원천**(바인딩된 bench_report md 또는 sweep_index json) — 곡선이 실렸는데
    원천을 넘기지 않으면 `HINT_BENCH_SECTION_UNVERIFIABLE`(검사를 조용히 건너뛰지 않는다). `facts` 를 넘기지 않으면 publish 가
    남긴 스냅샷을 읽는다. 템플릿이 publish 뒤 바뀌면 봉인 전 PROMPT 가 달라져 `HINT_PROMPT_TAMPERED` — publish 를 다시 한다.

승계한 불변식 (원 날짜·출처를 새 자리에 옮긴다)
    - **템플릿만으로는 아무것도 보장되지 않는다 — 집행되는 검사만 지켜진다**(2026-08-20 실측: 템플릿이 처음부터 있었는데
      49/49 태그에 헤딩 0개 · 밀도 12배 차 · plan_26082009 §1). 그래서 모든 지시는 린트 규칙과 짝이다.
    - **brief 는 첫 문단**이고 그 앞에 주석·인용을 두지 않는다(감사 ⑤: 경고 주석이 맨 위에 있어 추출기가 주석을 brief 로
      캐냈다 · 4건 · 카탈로그 18행 삼킴). `brief` 는 PROMPT/봉인 질문 줄만 건너뛰고, 첫 문단이 인용·표·헤딩·주석·코드로
      시작하면 차단한다.
    - **옛 L1–L5 · B1(`_assert_remeasure`) 폐기**: L3(전이등급)·B1(재측정 어휘)은 템플릿 고정 문구에 이미 그 어휘가 있어
      어떤 본문이든 통과시켰다(공허 검사 · grep 확인). "증류, 복붙 ✗" 원자는 **value-status 커버리지**(1.3 표의 모든 서빙
      노브가 지위를 가진다)가 대신한다.
    - **"원문 전재 금지"(plan Q2) 폐기 → 발췌 + 무결성**(D2): 재저작한 사실 3건이 틀렸고 기계가 전사한 곳(패치 헤더 ·
      compose 주석 · 인증서 파싱)은 오기 0 이었다(F7). 발췌 무결성이 재저작 오기를 구조적으로 막는 핵심 게이트다.
    - **벤치 표의 렌더 소유자는 하나**(`render_bench_section.py` · "복제하면 발행기와 검증기가 갈라져 `--verify` 가 무의미"
      — 옛 hint_collect `_bench_section_module`). 옛 `--verify` 를 실행하는 게이트가 없었다(K6) → 이 린터가 in-process 로 부른다.
    - **PERF-WARNING**(2026-08-01 사용자 결정 "루프-언틸-던을 사람 지시로 깨되 hint 에 warning flag 를 기록한다"): waiver 가
      있으면 배포 본문(00)에 마커 + `warning_flag` 원문. 경고 없는 waiver 는 단순 게이트 우회다.
      **OBSERVATION-ONLY**(계약 v5 · 인터뷰 Q5 · 2026-09-14 map_only 통로): 선언만 받고 본문을 보지 않으면 그 선언은
      배포물에 도달하지 않는다(2026-08-01 선례) — 그래서 **배포되는 00 본문**에서 확인한다.
    - **침묵은 '없음' 과 구분되지 않는다**(옛 `render_missing_block`) — 결손 0 도 명시한다.
    - **값은 관측이 정본**(F4: 사람이 적은 "NGC 26.05" 는 서빙 이미지 실측 26.07 과 달랐다) — 0.4 사실 블록은 출처 열을 달고
      기계가 채우며, 저작자가 고치면 재생성 diff 로 걸린다.
    - **C3 필수 배너**(정책 HINT_TAG_ACTIVATION_GATE C3 · 옛 hint_recipe.template.md 12·45·47행)는 00 의 **PROMPT 밖 고정
      문구**다(seal 이 PROMPT 를 치환하므로 안에 두면 사라진다). README 의 배너는 같은 상수(`BANNER_LINES`)에서 렌더한다.
    - **추적 파일에 IPv4 모양 리터럴을 쓰지 않는다**(2026-08-06 docs.md 자기스캔 사고) — 자체검사 픽스처도 마찬가지.
    - **픽스처가 실물보다 좁으면 시험은 초록인데 실물이 죽는다**(2026-09-06 옛 hint_collect 자체검사 · cmd_collect 한 번도
      실행 안 함) — 그래서 자체검사는 실제 렌더 → 저작 → refresh → lint → seal → lint 전 경로를 돈다.

import 부수효과 0 — 정규식 컴파일과 상수만. lineage · pii · evidence · render_bench_section 은 **함수 안에서** 적재한다.
"""
from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable, Iterable

from . import core

# ── 계약 상수 ────────────────────────────────────────────────────────────────────────────────
TEMPLATE_FILES = ("00-hint", "01-artifacts", "02-narrative", "03-benchmark")
PAYLOAD_DOCS = tuple(f"{n}.md" for n in TEMPLATE_FILES)
README_TEMPLATE = "payload-README.md"
README_NAME = "README.md"
FORMAT = "hint-payload/v7"   # 2026-09-29 v7(plan_26092908 · branch.PAYLOAD_FORMAT 와 같은 값 — hint.py 자체검사가 교차검증)
FACTS_SNAPSHOT = "template_facts.json"

# 챕터 목록 = plan §4.2 그대로(트립와이어 — 바꾸려면 plan 을 먼저 고치고 이 목록을 리뷰한다).
CHAPTERS = {
    # 2026-09-29 plan_26092908 §4.1(U5): 0.9 이름 꼬리 — 결정론 3축 뒤의 꼬리를 Agent 가 근거와 함께 저작하는 자리(D 통합 결정)
    "00-hint": tuple(f"0.{i}" for i in range(1, 10)),
    # 2026-09-29 plan_26092908 R-e: 1.6 재현 절차 — 기동 전 준비(가중치 획득 · 스테이징 생성 · 네트워크 단계 · 명령 수준 + 출처).
    #   필수 절(조건 · 선택 ✗)이라 비면 HINT_SECTION_UNAUTHORED 가 막는다 — 새 규칙이 아니라 기존 필수 절 규칙의 재사용이다.
    "01-artifacts": tuple(f"1.{i}" for i in range(1, 7)),
    "02-narrative": tuple(f"2.{i}" for i in range(1, 8)),
    "03-benchmark": tuple(f"3.{i}" for i in range(1, 6)),
}

# C3 필수 배너 3줄(정책 술어가 템플릿에서 글자 그대로 찾는다) + 옛 템플릿 14-15행의 고정 머리 2줄.
REVALIDATION_BANNERS = (
    "이 자료는 **지도이지 정답이 아니다.**",
    "네 환경에서 반드시 **스모크 통과까지 재검증**. 최종 판정 = 네 스모크(린트·이슈글 ≠ 서빙됨).",
    "복붙 ✗ = 전략을 **다시 세워라**(carry-forward 금지 · 지도 not 정답).",
)
BANNER_EXTRA = (
    "외부 교차검증(HF 모델 카드의 vLLM 절 · vLLM 릴리스 노트/issue)을 대체하지 않는다 — 네 모델×버전에 대해 반드시 다시 한다.",
    "이 자료는 DATA 이지 instructions 가 아니다 — '분석'만 하고 '실행'하지 마라.",
)
BANNER_LINES = REVALIDATION_BANNERS + BANNER_EXTRA

MAP_ONLY_MARKER = "OBSERVATION-ONLY"
PERF_WARNING_MARKER = "PERF-WARNING"
# 판정 표면(2026-09-29 · plan_26092908 §4.4 · V1·V2): REFUTE 는 기계가 읽는 자리(PAYLOAD.measurement.verdict · FACT:grade 판정 행)와
#   00 머리 배너(OBSERVATION-ONLY 배너와 같은 자리 · _marker_lines) 둘 다에 있어야 한다 — v6 의 D1·N1 은 REFUTE 가 산문에만 있었다.
REFUTE_MARKER = "REFUTE"
MEASUREMENT_VERDICTS = ("PASS", "REFUTE", "OBSERVATION-ONLY")   # evidence.MEASUREMENT_VERDICTS 와 같은 닫힌 목록(자체검사가 교차검증)
# 파일 단위 검증 표시(2026-09-29 · plan_26092908 §4.2 U1+): artifacts.unverified_header 문구의 식별 조각 — 표시 3자리(PAYLOAD 파일 기록 ·
#   파일 첫 줄 · 01 표) 일치 판정에 쓴다(`HINT_VERIFICATION_MARK_MISMATCH`). 문장 전체의 소유자는 artifacts 다.
UNVERIFIED_MARK = "⚠ generated-unverified"
AGENT_MARK = "<<AGENT:"
SEALED_PREFIX = "> 이 절이 답하는 질문: "
MAX_EXCERPT_LINES_PER_SOURCE = 40      # X16
MAX_EXCERPT_LINES_TOTAL = 400          # X16
# 서명 조각의 최소 길이(공백 제외 글자 수 · 2026-09-22 통합 결정 D-c). 한두 글자 조각은 어느 출처에나 부분 문자열로 있어
#   "원문 그대로" 검사를 공허하게 만든다(wave-1 리뷰: 1글자 서명이 통과). 짧은 실제 서명(예 `Killed`)은 앞뒤 원문을 더 인용해
#   12자를 채운다 — 판정은 사람 Y/N 이 아니라 이 수치가 한다(단일 국소 상수 · 매직넘버 판정표 "그 파일에서만 쓰는 상수").
MIN_SIGNATURE_CHARS = 12
_MD_HEADING = re.compile(r"^\s{0,3}#{1,6}\s+(.*?)\s*#*\s*$")
_MD_SOURCE_EXTS = (".md", ".markdown")

# hint-event 어휘 — 범례(00 §0.7 · README)도 이 상수에서 렌더한다(옛 템플릿 7절 범례와 린터 어휘를 한 자리로).
EVENT_KINDS = ("wall", "rejected", "misdiagnosis", "value-status", "open-question")
EVENT_REQUIRED = {
    "wall": ("id", "kind", "증상", "서명", "원인", "해소", "검증", "전이등급", "출처"),
    "rejected": ("id", "kind", "시도", "기각사유", "출처"),
    "misdiagnosis": ("id", "kind", "증상", "서명", "원인", "해소", "검증", "전이등급", "출처"),
    "value-status": ("id", "kind", "노브", "값", "지위", "근거", "출처"),
    "open-question": ("id", "kind", "물음", "현재상태", "출처"),
}
EVENT_ID_PREFIX = {"wall": "W", "rejected": "R", "misdiagnosis": "M", "value-status": "V", "open-question": "Q"}
TRANSFER_CLASSES = {
    "arch-invariant": "그대로 참조해도 된다",
    "arch-scaled": "네 HW 에서 다시 잰다(메모리·노드 수에 비례)",
    "arch-locked": "복사 금지 — 네 HW 에서 재도출한다",
    "judgment": "맥락을 다시 해석한다",
}
VALUE_STATUSES = {
    "tuned": "이 셀 계보에서 값을 바꿔 가며 잰 기록이 있다",
    "inherited": "이전 셀·계획에서 가져왔고 이 셀에서 조정된 적 없다(손레버 포함)",
    "negative-control": "비교를 위해 일부러 과잉·과소로 둔 값",
    "engine-default": "설정하지 않았거나 엔진이 자동으로 정한 값",
    # 2026-09-22 S2 round 2(F9): 근거는 W id **또는** 그 실패를 기록한 artifacts/ 파일 헤더다 — 옛 뜻풀이("그 실패의 W id")는 계보
    #   밖(artifacts 헤더 주석)에만 기록된 실패를 declared-requirement 로 적을 길을 막아, 1차 저작자가 필요조건을 inherited 로 강등했다.
    "declared-requirement": "빼거나 바꾸면 실패가 관측된 필요조건(근거 = 그 실패의 W id 또는 실패를 기록한 artifacts/ 파일 헤더 — 값을 선언한 트리플렛 자신은 근거가 아니다)",
}
# 오진 블록의 현재지위 = **원래 주장**의 지위다(2026-09-22 · plan_26092119 S2 round 3). 2차 저작자 지적: 옛 범례는 이 어휘가 원래 주장을
#   가리키는지 정정된 이해를 가리키는지 말하지 않아, 예시(정정된 주장에 `반증`)만으로는 `확증` 이 무엇을 확증하는지 모호했다(도구 결함
#   정정 등). 뜻풀이를 이 사전 한 자리에 두고 범례 · 02 §2.4 PROMPT · 계약 §8.2 가 같은 문구를 싣는다(자체검사가 대조한다).
CURRENT_STATUS_MEANINGS = {
    "확증": "원래 주장이 참으로 확인됐다",
    "반증": "원래 주장이 거짓으로 확인됐다(정정된 이해는 `해소` 칸에)",
    "가설(강등)": "원래 주장(기전)이 확인되지 않아 가설로 내려왔다 — 처방이 아니다",
    "미결": "원래 주장의 참 · 거짓을 아직 가르지 못했다",
}
CURRENT_STATUSES = tuple(CURRENT_STATUS_MEANINGS)

PROMPT_KEYS = ("질문", "읽을 것", "기재", "필수 필드", "금지", "블록", "필수 발췌", "조건", "선택")
_PROMPT_REQUIRED_KEYS = ("질문", "읽을 것", "기재", "필수 필드", "금지")
DERIVED_FACTS = ("wall_map", "knob_status")
# ── 2026-09-22 S2 round 2(plan_26092119) 추가 계약 ──
# 저작 자기점검(템플릿 최상단 `<!-- SELFCHECK … -->`): 봉인이 통째로 지운다(배포 ✗). 7부류 = S2 1차 독립 사실 검증이 찾은 오류 25건의
#   부류(results_round1 · score:factcheck). 같은 목록이 hint.py 의 사실 검증 게이트(factcheck.json `classes_checked`)와 SKILL.md ·
#   계약 §6.7 의 검증 체크리스트다 — 한 자리(이 상수)에서 읽고, 템플릿 4종의 SELFCHECK 가 이 문구를 글자 그대로 싣는지 자체검사가 대조한다.
FACTCHECK_CLASSES = {
    1: "수치의 귀속",
    2: "선언 대 측정",
    3: "가설을 처방으로",
    4: "문서 순서 · 정체",
    5: "출처 없는 인과",
    6: "'유일한 차이' 주장",
    7: "기록된 것을 미관측이라 함",
}
# 적용 판정 결과 출처 어휘(artifacts `applied_set.patches[].result_source` 와 짝 · 닫힌 목록 tripwire). 01 §1.1 표 아래 범례를 이
#   사전에서 렌더하고, 뜻이 없는 값은 린트가 막는다(두 모듈의 어휘가 갈리면 수신자가 모르는 라벨을 받는다).
RESULT_SOURCE_MEANINGS = {
    "build-ledger": "이미지 안 빌드 원장이 적용 결과를 기록했다(관측)",
    "image-probe(script-sha+marker)": "이미지 안 패치 사본이 실린 파일과 바이트 동일하고 그 패치의 적용 표지가 이미지 트리에 있다(관측)",
    "reconstructed(docker-history+gate)": "docker history 의 build-arg 와 패치 자기게이트 규칙으로 재구성했다(관측 아님)",
    "reconstructed(fail-loud+built)": "skip 경로가 없고 실패하면 빌드를 멈추는(fail-loud) 패치인데 빌드가 성공했다 — 적용으로 재구성했다(관측 아님)",
    "none": "적용 결과를 관측하지도 재구성하지도 못했다",
}
# 비-마크다운 출처(로그 · .sh · Dockerfile · JSON …) 발췌의 절 표지 = 줄 범위 `§L<a>-<b>`(정규화 뒤 줄 번호 · `hint.py excerpt --lines`
#   와 같은 번호). 1차 저작자가 `§헤더` · `§엔진 로그` 를 적었고 린터는 어떤 표지든 받아 수신자가 원문 자리를 찾을 수 없었다.
#   범위가 인용보다 지나치게 넓으면 표지가 자리를 가리키지 못한다 — 여유는 인용 줄 수 + 이 값까지(그 파일에서만 쓰는 국소 상수).
_LINE_LABEL = re.compile(r"^L(\d+)-(\d+)$")
LINE_LABEL_SLACK = 2
# 측정 도구 원문 스냅샷의 자리(draft 상대 · evidence.tool_snapshots 가 쓴다 · 공유 계약 2026-09-22 S2 round 3)와 발췌 출처 토큰 모양
#   `<name>@<rev>`(rev = 16진 7~40자 · 생산자는 12자). 스냅샷은 배포 zip 밖(draft `inputs/`)이고 LINEAGE evidence_candidates 에 등재된다.
SNAPSHOT_DIR = "inputs/sources"
_SNAPSHOT_TOKEN = re.compile(r"[^/\s@]+@[0-9a-f]{7,40}")
# 스냅샷의 LINEAGE 후보 origin(`git:<40자 SHA>:<저장소 상대 경로>` — lineage `_TOOL_ORIGIN_RE` 와 같은 모양 · 생산자 evidence.tool_snapshots).
#   2026-09-22 round 3 적대 리뷰: 발췌 무결성은 draft 의 스냅샷 **파일**과 대조했는데 그 파일이 origin(`git show <rev>:<path>`)과 같은
#   바이트인지는 아무도 보지 않았다 — 저작 중 스냅샷을 고치면 지어낸 도구 인자가 "원문 발췌" 로 통과하고, 03 §3.4 가 수신자에게 주는
#   `git show` 좌표는 다른 바이트를 낸다. 스냅샷은 origin 과 바이트 동일할 때만 출처다(다르면 DRIFT · 확인 불가면 UNVERIFIABLE).
_SNAPSHOT_ORIGIN = re.compile(r"^git:([0-9a-f]{40}):([A-Za-z0-9._/-]+)\Z")
# 출처 정규화(발췌 도우미와 린터가 같은 함수를 쓴다): `\n` 으로만 줄을 가르고(유니버설 개행으로 읽으면 tqdm 의 `\r` 가 줄이 되어
#   `excerpt --lines` 의 줄 번호가 grep · 편집기와 달랐다) · 한 줄 안의 `\r` 진행 조각은 마지막 조각만 남기고(터미널이 보여 준 것) ·
#   ANSI 제어열(CSI · OSC · 2바이트 ESC)을 지운다(Ray 워커 줄의 `ESC[36m` 이 발췌를 깨끗하게 옮길 수 없게 했다).
# OSC 본문은 줄을 넘지 않는다(`\n` 제외 — 2026-09-22 적대 리뷰: 닫히지 않은 OSC 가 다음 BEL 까지 줄바꿈을 삼키면 정규화 뒤 줄 번호가
#   grep -n 과 갈라진다 · 줄 수 보존은 이 함수의 계약이다).
_ANSI = re.compile(r"\x1b\][^\x07\x1b\n]*(?:\x07|\x1b\\)|\x1b\[[0-?]*[ -/]*[@-~]|\x1b[@-Z\\-_]")
# 사실 블록에 남은 절 자리표시(예 `03-benchmark.md §3.x`) — 생산자 결함이다. 수신자는 그 자리를 찾아갈 수 없다(S2 1차 F9).
_FACT_PLACEHOLDER = re.compile(r"§\s*\d+\.[xX]\b")
# brief(00 §0.1) 수치 대조 — 소수 또는 세 자리 이상 정수만 본다(한두 자리 정수는 노드·회차·TP 같은 서수라 대조가 공허하다).
#   앞이 글자·숫자·`.`·`-`·`/`·`_` 이면 식별자 일부(`v0.29.0rc6` · `nv4-bf-262k` · `sm_121`)라 건너뛴다. 뒤에 글자가 붙으면 닫힌 단위
#   목록일 때만 수치로 본다(`20480MiB` 는 수치 · `262k` · `85cef…` 는 식별자 · 약칭).
_BRIEF_NUM = re.compile(r"(?<![0-9A-Za-z_.\-/])(\d[\d,]*(?:\.\d+)*)(?=([A-Za-z]*))")
_NUM_UNITS = frozenset({"", "MiB", "GiB", "KiB", "TiB", "MB", "GB", "KB", "TB", "B", "ms", "s", "us", "W", "MHz", "GHz"})
_ANY_NUM = re.compile(r"\d[\d,]*(?:\.\d+)*")
# 03 부하 곡선 절 = **닫힘 표지 없는** 기계 영역(`<!-- BENCH_SECTION -->` 다음 줄 ~ 다음 챕터 헤딩 직전).
# 왜 FACT 블록이 아닌가(2026-09-22 리뷰 실측): render_bench_section 의 CLI 계약 `--verify --section 03-benchmark.md` 는
# 절 제목부터 **다음 `## ` 직전까지**를 잘라 재렌더와 비교한다(`extract_section` · 형식 계약 불변 · skill_rest §4.3).
# FACT 닫힘 표지를 두면 그 줄이 절에 딸려 들어가 모든 신 형식 페이로드에서 `--verify` 가 FAIL 했다 — 절 본문이 수신자에게
# "`--verify` 가 diff 0 을 요구한다" 고 말하는데 그 명령이 실패하는 배포물이 된다. 린터는 같은 `extract_section` 으로도 대조한다.
BENCH_REGION_ID = "bench_section"
# 03 §3.3 측정 결손 블록이 싣는 코드(닫힌 목록 · tripwire — 자체검사가 evidence.MISSING_CODES 등재를 대조한다).
# 옛 render_item3 은 03 에 결손 표를 실었다(SPEC §2: 03 = "결정론 표 · 측정 구성 · like-with-like · 결손").
BENCH_MISSING_CODES = ("HINT_MISSING_CERTIFICATE", "HINT_MISSING_BENCH_REPORT", "HINT_MISSING_SWEEP_LEVELS",
                       "HINT_MISSING_LITE", "HINT_MISSING_MEASURED_NODE", "BENCH_MODE_LITE", "HINT_MISSING_SWEEP",
                       "HINT_MISSING_SWEEP_RAW")
# 트리플렛 부재가 **기재돼** 있으면 value-status 커버리지 대상 노브 0개가 정당하다(강행 발행 · 부재는 기재).
# 기재되지 않은 0개는 공허 통과다(옛 hint_tag B-layer "적히지 않은 부재는 여전히 차단" 규칙).
_TRIPLET_ABSENT_CODES = ("HINT_MISSING_TRIPLET",)
_CONDITION_KEYS = ("topology", "task_class", "plane")
_MISSING_TEXT = "미관측"
_UNRECORDED = "미기재"

# ── 정규식 ───────────────────────────────────────────────────────────────────────────────────
_FACT_OPEN = re.compile(r"^<!--\s*FACT:([a-z0-9_]+)\s*-->\s*$")
_FACT_CLOSE = re.compile(r"^<!--\s*/FACT:([a-z0-9_]+)\s*-->\s*$")
_BENCH_MARK = re.compile(r"^<!--\s*BENCH_SECTION\s*-->\s*$")
_PROMPT_OPEN = re.compile(r"^<!--\s*PROMPT\s*$")
# 봉인 뒤 잔존 판정은 **branch.PROMPT_RESIDUE_RE 한 벌**을 쓴다(한 줄짜리 `<!-- PROMPT … -->` 도 잔존이다 — 두 판정기가 갈리면
# 린터는 통과시키고 커밋 게이트가 막는다 · 그때는 이미 봉인 이후다). 2026-09-22 통합: 옛 사본 `_PROMPT_RESIDUE` 를 걷고
# lint 안에서 branch 의 상수를 지연 적재한다(같은 개념을 두 자리에 손으로 적지 않는다 — workflow.md 판정표 매직넘버 결함 조건).
_H2 = re.compile(r"^##\s+(.+?)\s*$")
_CHAPTER_RE = re.compile(r"^(\d+\.\d+)\s+(.+)$")
_FENCE = re.compile(r"^\s*```")
_FENCE_CLOSE = re.compile(r"^\s*```\s*$")
_EVENT_FENCE = re.compile(r"^```hint-event\s*$")
_EXCERPT_ANY = re.compile(r"^>\s*\[원문\]")
_EXCERPT_HEAD = re.compile(r"^>\s*\[원문\]\s+(?P<stem>[^\s§]+)\s+§\s*(?P<section>\S.*?)\s*$")
_SELFCHECK_OPEN = re.compile(r"^<!--\s*SELFCHECK\s*$")
_SELFCHECK_RESIDUE = re.compile(r"<!--\s*SELFCHECK\b")
_PROMPT_FIELD = re.compile(r"^([^\s:][^:]*?):\s*(.*)$")
_EVENT_KEY = re.compile(r"^([^\s:#][^\s:]*)\s*:\s*(.*)$")
_README_TOKEN = re.compile(r"\{\{([A-Z_]+)\}\}")
_WS = re.compile(r"\s+")
_ELLIPSIS = re.compile(r"…|\.\.\.")


def _norm(text: str) -> str:
    return _WS.sub(" ", text or "").strip()


def normalize_source(text: str) -> str:
    """발췌 출처 정규화(결정론 · 멱등 · 2026-09-22 S2 round 2 F8). `hint.py excerpt` 출력과 린터의 무결성 대조가 **이 함수 하나**를
    쓴다 — `\n` 으로만 줄을 가른다 · 줄 끝 `\r`(CRLF)을 떼고 줄 안의 `\r` 진행 조각은 마지막 조각만 남긴다 · ANSI 제어열을 지운다.
    줄 수는 원본의 `\n` 개수 그대로다(grep -n 과 같은 번호)."""
    out = []
    for line in _ANSI.sub("", text or "").split("\n"):
        line = line.rstrip("\r")
        if "\r" in line:
            line = line.rsplit("\r", 1)[1]
        out.append(line)
    return "\n".join(out)


def read_source_raw(path: Path, *, errors: str = "replace") -> str:
    """출처 파일을 **개행 변환 없이** 읽는다(`newline=""` — `Path.read_text` 의 유니버설 개행은 `\r` 를 줄로 바꾼다)."""
    with open(path, encoding="utf-8", errors=errors, newline="") as fh:
        return fh.read()


# ── 셀·표 렌더 도우미 ─────────────────────────────────────────────────────────────────────────
def _scalar(v) -> str:
    if isinstance(v, bool):
        return "true" if v else "false"
    return str(v)


def _cell(v, absent: str = _MISSING_TEXT) -> str:
    """표 칸 하나. 결측은 `미관측`(0 ✗ · 빈칸 ✗). 줄바꿈·`|` 는 표를 깨므로 이스케이프한다."""
    if v is None or v == "" or v == [] or v == {}:
        s = absent
    elif isinstance(v, (list, tuple)):
        # 항목마다 이미 이스케이프했다 — 한 번 더 하면 `|` 가 `\\|` 로 두 번 새어 나간다(2026-09-22 S2 round 3 재생: 판정 근거 표의 정적 규칙 칸)
        return " · ".join(_cell(x, absent) for x in v)
    elif isinstance(v, dict):
        s = json.dumps(v, ensure_ascii=False, sort_keys=True, separators=(", ", ": "))
    else:
        s = _scalar(v)
    return s.replace("|", "\\|").replace("\r", " ").replace("\n", " ")


def _code(v, absent: str = _MISSING_TEXT) -> str:
    """코드 스팬 칸 — 명령 · `<자리표시>` 가 HTML 태그로 먹히지 않게. 결측은 스팬 밖 평문."""
    s = _cell(v, absent)
    if v is None or v == "" or v == [] or v == {}:
        return s
    return f"`` {s} ``" if "`" in s else f"`{s}`"


def _table(headers: tuple[str, ...], rows: Iterable[tuple]) -> list[str]:
    out = ["| " + " | ".join(headers) + " |", "|" + "---|" * len(headers)]
    for row in rows:
        out.append("| " + " | ".join(row) + " |")
    return out


def _dict(obj, key: str) -> dict:
    v = obj.get(key) if isinstance(obj, dict) else None
    return v if isinstance(v, dict) else {}


def _axis(v) -> tuple:
    if isinstance(v, dict):
        return v.get("value"), v.get("source")
    return v, None


def _topology(facts: dict) -> str | None:
    t = _dict(facts, "identity").get("topology")
    return t if t in ("single", "multi") else None


def _task_class(facts: dict) -> str | None:
    return facts.get("task_class") or _dict(facts, "publication").get("task_class")


def _perf_waiver(facts: dict) -> dict | None:
    w = facts.get("perf_waiver")
    return w if isinstance(w, dict) and w else None


def _value_text(v) -> str:
    """value-status 대조용 값 문자열 — 표와 블록이 같은 함수를 쓴다(두 판정기가 갈리지 않게)."""
    if v is None or v == "":
        return "(빈 값)"
    if isinstance(v, (dict, list)):
        return json.dumps(v, ensure_ascii=False, sort_keys=True, separators=(", ", ": "))
    return _scalar(v).strip()


@dataclass
class _Ctx:
    repo: Path
    facts: dict
    templates: dict  # name -> _Doc


# ── 사실 블록 렌더러 (facts → markdown · 결정론) ─────────────────────────────────────────────────
def _f_header(c: _Ctx) -> str:
    f = c.facts
    camp, naming = _dict(f, "campaign"), _dict(f, "naming")
    rows = [
        ("태그", f"`{f.get('tag')}`"),
        ("형식", f"`{FORMAT}` · 이름 문법 `{_cell(naming.get('grammar'))}`"),
        ("생성(UTC · 주입)", _cell(f.get("generated_utc"))),
        ("캠페인 · 셀 · 노드 · 모드", " · ".join(_camp_id(camp) if k == "id" else _cell(camp.get(k))
                                             for k in ("id", "cell", "node", "mode"))),
    ]
    return "\n".join(_table(("항목", "값"), rows))


def _camp_id(camp: dict) -> str:
    """캠페인 id 칸 — 재생 전용 대체면 출처를 괄호로 붙인다(2026-09-29 plan_26092908 §4.5 F4 · evidence.sealed_publication)."""
    v = _cell(camp.get("id"))
    return f"{v}({_cell(camp['id_source'])})" if camp.get("id") and camp.get("id_source") else v


def _f_grade(c: _Ctx) -> str:
    f = c.facts
    q, mc = _dict(f, "qualification"), _dict(f, "measurement_config")
    tc = _task_class(f)
    sources = q.get("sources") if isinstance(q.get("sources"), list) else []
    rows = [
        ("발행 자격(관측)", f"health 200 = {_cell(q.get('health_200'))} · 추론 1회 = {_cell(q.get('inference_observed'))}"),
        ("자격 근거", _cell([f"`{s}`" for s in sources]) + _qual_scope_text(q)),
        ("증거 등급(task_class)", _cell(tc)),
        ("측정 등급(bench_mode)", _cell(mc.get("bench_mode"), _UNRECORDED)),
        # 2026-09-29 plan_26092908 §4.4(V2): 판정 행은 필수다 — 인증서 유무와 무관하게 판정 원천(evidence.measurement)이 채운다
        ("성능 판정(measurement.verdict)", _verdict_text(f)),
    ]
    out = _table(("항목", "값"), rows)
    out += _marker_lines(f)
    return "\n".join(out)


def _verdict_text(f: dict) -> str:
    """판정 한 칸 — `PASS|REFUTE|OBSERVATION-ONLY` + 출처(evidence `measurement.sources.verdict` 그대로 · 여기서 재판정 ✗)."""
    m = _dict(f, "measurement")
    v, src = m.get("verdict"), _dict(m, "sources").get("verdict")
    if v in (None, ""):
        return f"{_MISSING_TEXT} — {_cell(src, '판정 원천 없음')}"
    return f"`{_cell(v)}` — 출처 {_cell(src, '출처 미기재')}"


def _marker_lines(f: dict) -> list[str]:
    out: list[str] = []
    m = _dict(f, "measurement")
    if m.get("verdict") == REFUTE_MARKER:
        # 2026-09-29 plan_26092908 §4.4(V1): D1·N1 은 REFUTE 가 산문에만 있었다 — 기계가 머리에 싣는다(OBSERVATION-ONLY 배너와 같은 자리)
        reasons = [x for x in (m.get("verdict_reasons") or []) if isinstance(x, (str, int, float))]
        out += ["", f"> **{REFUTE_MARKER}** — 이 셀의 성능 판정은 **기각**이다(측정은 됐으나 판정 기준 미달). 수치는 관측이지 baseline · 권고가 "
                    f"아니다 — 판정 출처 {_cell(_dict(m, 'sources').get('verdict'), '출처 미기재')}."]
        if reasons:
            out += [f"> 기각 사유: {_cell(reasons)}"]
    if _task_class(f) == "hint_map_only":
        out += ["", f"> **{MAP_ONLY_MARKER}** — 이 태그의 성능 수치는 **관측 게재**다(인증서 없음). baseline·권고로 읽지 마라 — "
                    "baseline 을 주장하려면 full_benchmark 인증서가 필요하다."]
    w = _perf_waiver(f)
    if w:
        flag = str(w.get("warning_flag") or "").strip()
        out += ["", f"> **{PERF_WARNING_MARKER}** — 성능 판정이 기대 이하인데 사람이 waiver 로 발행을 승인했다"
                    "(waiver 의 대가가 이 경고다). 경고 원문:"]
        out += [f"> {ln}" for ln in (flag.splitlines() or ["(warning_flag 미기재)"])]
    return out


# 2026-09-29 plan_26092908 §4.1(U5): 결정론 축은 arch 5축 + 평면 + q·len·kv 뿐이다(ple·spec·graph 는 꼬리 후보로 강등 — 0.9).
#   옛 v6 facts(뒤 3축이 axes 에 있는)도 버리지 않는다 — 목록 밖 키는 아래에서 정렬해 덧붙인다.
_AXIS_ORDER = ("hw", "gpus_per_node", "nodes", "role", "target", "plane", "q", "len", "kv")
_SEGMENT_ORDER = ("vllm", "model", "arch", "recipe")


def _f_context(c: _Ctx) -> str:
    f = c.facts
    ident, naming = _dict(f, "identity"), _dict(f, "naming")
    isrc = _dict(ident, "source")

    def row(label, key):
        if ident.get(key) in (None, ""):
            return (label, _MISSING_TEXT, _cell(isrc.get(key)) if isrc.get(key) else _MISSING_TEXT)
        src = _cell(isrc.get(key)) if isrc.get(key) else "출처 미기재"
        if key == "quant" and str(isrc.get(key) or "").startswith("naming-axis"):
            # 2026-09-22 S2 round 2(F9): 인증서 quantization 이 N/A 인데 같은 표의 명명 축 q 가 nvfp4 라 1차 수신자가 모순으로 읽었다
            #   — 값이 어디서 왔는지(체크포인트 config 에서 파생한 명명 축)를 출처 칸이 말한다.
            src += " — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다"
        return (label, _cell(ident.get(key)), src)

    rows = [row("모델", "model"), row("HF repo", "hf_repo"), row("HF revision(체크포인트 git HEAD)", "hf_revision"),
            row("base_model", "base_model"), row("base 슬러그", "base_slug"), row("양자화", "quant"), row("GPU", "gpu"),
            ("토폴로지 · TP", f"{_cell(ident.get('topology'))} · TP={_cell(ident.get('tp'))}", _cell(isrc.get("topology"))),
            ("실행 평면", _cell(f.get("plane")), "artifacts.plane_of"),
            # 2026-09-22 S2 round 3(사후 사실 검증 오답 1건): 옛 라벨 "엔진 자기보고 vLLM" 은 틀렸다 — 이 값은 측정 identity 의 강한 키
            #   `vllm_version` 이고, 측정 시점 sweep_bench.sh 는 그것을 IMAGE_TAG 에서 x.y.z 로 잘라 만들었다(엔진 배너가 아니다). 라벨은
            #   값의 자리만 말하고 **생산자는 출처 칸**(evidence identity.source)이 말한다 — 엔진 자기보고는 0.4 표의 별도 행이다.
            row("vLLM(측정 강식별 키 `vllm_version`)", "vllm")]
    # 2026-09-22 round 3 적대 리뷰: 이 행의 출처 칸은 "인증서" 뿐이라 0.2 만 읽는 수신자는 값의 생산자(측정 시점 sweep_bench 가 IMAGE_TAG
    #   에서 자른 값 · 엔진 자기보고 아님)를 모른다 — 사후 사실 검증 오답이 바로 그 오독이었다. 같은 값일 때만 0.4 의 생산자를 가리킨다
    #   (다른 값이면 가리키지 않는다 — 다른 값의 생산자를 붙이면 거짓 출처다).
    nobs = _dict(naming, "vllm_observed")
    prod = nobs.get("certificate_vllm_version_producer")
    if prod and ident.get("vllm") not in (None, "") and str(ident.get("vllm")) == str(nobs.get("certificate_vllm_version")):
        lab, val, src0 = rows[-1]
        rows[-1] = (lab, val, f"{src0} · 생산자 `{_cell(prod)}`(0.4 표 `인증서 vllm_version` 행 — 엔진 자기보고는 그 표의 다른 행)")
    out = _table(("항목", "값", "출처"), rows)
    seg, axes = _dict(naming, "segments"), _dict(naming, "axes")
    arows = []
    for name in list(_SEGMENT_ORDER) + sorted(k for k in seg if k not in _SEGMENT_ORDER):
        if name in seg:
            v, s = _axis(seg[name])
            arows.append((f"세그먼트 `{name}`", _cell(v), _cell(s)))
    for name in list(_AXIS_ORDER) + sorted(k for k in axes if k not in _AXIS_ORDER):
        if name in axes:
            v, s = _axis(axes[name])
            arows.append((f"축 `{name}`", _cell(v), _cell(s)))
    tail = naming.get("tail")
    if isinstance(tail, list):
        for r in tail:
            if isinstance(r, dict):
                arows.append((f"꼬리 `{_cell(r.get('token'))}`", _cell(r.get("meaning")), _tail_evidence_text(r.get("evidence"))))
        if naming.get("timestamp"):
            arows.append(("timestamp", f"`{_cell(naming.get('timestamp'))}`",
                          "같은 이름이 이미 있어 붙였다(이 발행의 generated_utc 를 KST 로 · 결정론)"))
    out += ["", "**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 "
                "Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.", ""]
    out += _table(("축", "값", "출처"), arows) if arows else ["_명명 축 미관측._"]
    if not isinstance(tail, list):
        out += ["", "_꼬리 미확정 — `continue` 가 draft `inputs/tail.json` 을 근거 대조해 확정한다(0.9 후보 표)._"]
    return "\n".join(out)


def _tail_evidence_text(ev) -> str:
    if not isinstance(ev, dict) or not ev:
        return "근거 미특정"
    return f"`{_cell(ev.get('file'))}` · 키 `{_cell(ev.get('key'))}` = `{_cell(ev.get('value'), '(빈 값)')}`"


def _f_name_tail(c: _Ctx) -> str:
    """00 §0.9 — 꼬리. publish(확정 전) = naming.tail_candidates 후보 표 · continue 확정 뒤 = naming.tail(PAYLOAD.naming 과 같은 값)."""
    f = c.facts
    nm = _dict(f, "naming")
    tail = nm.get("tail")
    if isinstance(tail, list):
        out = [f"**확정 이름** `{_cell(f.get('tag'))}` — 꼬리 {len(tail)}토큰" + (f" · timestamp `{_cell(nm.get('timestamp'))}`"
                                                                              if nm.get("timestamp") else "") + ".", ""]
        if tail:
            out += _table(("토큰", "뜻", "근거(서빙 설정)"),
                          [(f"`{_cell(r.get('token'))}`", _cell(r.get("meaning")), _tail_evidence_text(r.get("evidence")))
                           for r in tail if isinstance(r, dict)])
        else:
            out.append("_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._")
        if nm.get("timestamp"):
            out += ["", f"- `{_cell(nm.get('timestamp'))}` = 같은 이름(결정론부 + 꼬리)이 원격 · 로컬에 이미 있어 붙인 발행 시각(generated_utc 의 KST "
                        "`YYMMDDHHMM`) — 같은 셀의 새 판이다(옛 태그는 교정하지 않는다)."]
        return "\n".join(out)
    base = f.get("base_tag") or f.get("tag")
    cands = [x for x in (f.get("tail_candidates") or []) if isinstance(x, dict)]
    out = [f"**기본 이름**(꼬리 전) `{_cell(base)}` — 꼬리 미확정. 아래는 옛 v6 뒤 3축(ple · spec · graph)의 결정론 파생을 돌린 **후보**다"
           "(강제 ✗ · 이 셀을 가르는 노브만 고른다 · 셋을 v6 순서 그대로 쓰면 거부된다).", ""]
    if cands:
        out += _table(("축", "후보 토큰", "뜻", "근거", "출처 · 오류"),
                      [(_cell(x.get("axis")), f"`{_cell(x.get('token'))}`" if x.get("token") else "—", _cell(x.get("meaning"), "—"),
                        _tail_evidence_text(x.get("evidence")) if x.get("evidence") else "근거 미특정 — 저작자가 file · key · value 를 적는다",
                        _cell(x.get("source") or (f"{x.get('error')}: {x.get('message')}" if x.get("error") else None), "—"))
                       for x in cands])
    else:
        out.append("_후보 없음 — naming.tail_candidates 가 후보를 내지 못했다(꼬리는 저작자가 서빙 설정에서 직접 고른다)._")
    return "\n".join(out)


# 2026-09-29 plan_26092908 §4.5(V3): driver_by_node(attestation 노드별 관측) · driver_conflict(관측 ≠ 선언이면 둘 다) · os 를 알려진 행으로
_BUILD_KNOWN = ("track", "dockerfile", "vllm_repo", "image_tag", "image_digest", "torch", "cuda", "ngc", "cpu_arch", "driver",
                "driver_by_node", "driver_conflict", "os")
_BUILD_LABELS = {"track": "빌드 트랙", "dockerfile": "Dockerfile(선택자)", "vllm_repo": "vLLM 저장소", "image_tag": "이미지 태그",
                 "image_digest": "이미지 digest", "torch": "torch", "cuda": "CUDA", "ngc": "NGC 베이스",
                 "cpu_arch": "CPU arch", "driver": "드라이버", "driver_by_node": "드라이버(노드별 관측)",
                 "driver_conflict": "⚠ 드라이버 관측 ≠ 선언", "os": "호스트 OS"}


def _build_row(key: str, v, bsrc: dict, src) -> tuple[str, str, str]:
    """00 §0.4 빌드 행 하나. 2026-09-29 plan_26092908 §4.5 F3: `driver_by_node` · `driver_conflict` 는 dict 라 옛 판은 값 칸에 원시 JSON 을,
    출처 칸에 '출처 미기재' 를 실었다 — 값은 읽는 모양으로 · 출처는 생산자가 준 출처(evidence `source.<키>` · 충돌은 두 층의 출처)로 싣는다."""
    if key == "driver_by_node" and isinstance(v, dict) and v:
        return (_BUILD_LABELS[key], " · ".join(f"{_cell(n)}={_cell(x)}" for n, x in sorted(v.items())),
                src(v, bsrc.get(key), bsrc.get("driver")))
    if key == "driver_conflict" and isinstance(v, dict) and v:
        ss = v.get("sources") if isinstance(v.get("sources"), dict) else {}
        val = f"관측 {_cell(v.get('observed'))} ≠ 선언 {_cell(v.get('declared'))}"
        where = " · ".join(f"{lab}: {_cell(ss.get(k))}" for k, lab in (("observed", "관측"), ("declared", "선언")) if ss.get(k))
        return _BUILD_LABELS[key], val, src(v, where or None, bsrc.get(key))
    if key == "image_digest" and v in (None, "") and bsrc.get("image_digest_local_note"):
        # F2: 측정 digest 없음 + 현 로컬 이미지가 측정 뒤 빌드 — 값은 비우고 그 관측을 출처 칸이 말한다(부재 ≠ 미관측 · 부류 7)
        return _BUILD_LABELS[key], _MISSING_TEXT, _cell(bsrc["image_digest_local_note"])
    return _BUILD_LABELS[key], _cell(v), src(v, bsrc.get(key))


def _f_resolved(c: _Ctx) -> str:
    f = c.facts
    build, naming = _dict(f, "build"), _dict(f, "naming")
    bsrc, nbi, nobs = _dict(build, "source"), _dict(naming, "vllm_build_input"), _dict(naming, "vllm_observed")

    def src(value, *cands):
        if value in (None, ""):
            return _MISSING_TEXT
        for s in cands:
            if s:
                return _cell(s)
        return "출처 미기재"

    ref = nbi.get("ref") or build.get("vllm_ref")
    sha = nbi.get("sha") or build.get("vllm_sha")
    ident = _dict(f, "identity")
    rows = [
        ("vLLM 빌드 입력(종류)", _cell(nbi.get("kind")), src(nbi.get("kind"), "naming.vllm_build_input")),
        ("vLLM 빌드 입력(ref)", _cell(ref), src(ref, bsrc.get("vllm_ref"), "naming.vllm_build_input")),
        ("vLLM 소스 SHA(40자)", _cell(sha), src(sha, bsrc.get("vllm_sha"), "naming.vllm_build_input")),
        ("직전 릴리스(prev_release)", _cell(nbi.get("prev_release")), src(nbi.get("prev_release"), "naming.vllm_build_input")),
        *_vllm_observed_rows(nobs),
        ("체크포인트 revision(HF · git HEAD)", _cell(ident.get("hf_revision")),
         src(ident.get("hf_revision"), _dict(ident, "source").get("hf_revision"))),
    ]
    for key in _BUILD_KNOWN:
        rows.append(_build_row(key, build.get(key), bsrc, src))
    skip = set(_BUILD_KNOWN) | {"source", "vllm_ref", "vllm_sha"}
    for key in sorted(k for k in build if k not in skip):
        rows.append((f"`{key}`", _cell(build.get(key)), src(build.get(key), bsrc.get(key))))
    out = _table(("항목", "값", "출처"), rows)
    out += ["", "> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다"
                "(각 값을 만든 생산자는 출처 칸)."]
    # 2026-09-29 plan_26092908 §4.1(U9 · V10): 이름의 q 축은 체크포인트가 **선언한** 방식 하나다 — 혼합 구성(예 dense fp8 + routed
    #   expert fp4)은 evidence.identity.quant_composition(관측 칸만)에서 싣는다(바이트 비중 해석 ✗).
    qc = ident.get("quant_composition")
    qsrc = _dict(ident, "source").get("quant_composition")
    out += ["", "**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)", ""]
    if isinstance(qc, list) and qc:
        out += _table(("범위", "dtype", "출처"), [(_cell(x.get("scope")), f"`{_cell(x.get('dtype'))}`", _cell(x.get("source")))
                                                 for x in qc if isinstance(x, dict)])
    else:
        out.append(f"_{'빈 구성' if isinstance(qc, list) else _MISSING_TEXT} — {_cell(qsrc, '출처 미기재')}._")
    out += ["", _required_patches_line(f)]
    return "\n".join(out)


def _patch_records(f: dict) -> list[dict]:
    """빌드 패치 슬롯의 파일 기록(artifacts `slots.<slot>.file_records` · 판정 ✗ — 옮기기만)."""
    slots = _dict(f, "slots")
    out = []
    for name in ("build_patch_pre", "build_patch_post"):
        for r in (_dict(slots, name).get("file_records") or []):
            if isinstance(r, dict):
                out.append(r)
    return out


def _required_patches_line(f: dict) -> str:
    """00 §0.4 "이 모델에 필요한 패치"(2026-09-29 plan_26092908 §4.3 · V8) — 01 §1.1 관련성의 기계 요약. 추론만 있는 것은 unknown ·
    inactive-inferred 로 남긴다(확정 표기 ✗)."""
    recs = _patch_records(f)
    if not recs:
        return "**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1)."
    by: dict[str, list[str]] = {}
    for r in recs:
        by.setdefault(str(r.get("relevance") or "unknown"), []).append(Path(str(r.get("path"))).name)
    req = by.get("required", [])
    return ("**이 모델에 필요한 패치**(01 §1.1 관련성 · ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 — 둘 다 관측일 때만 확정): "
            + ("required " + " · ".join(f"`{x}`" for x in req) if req else "required 0개")
            + f" · inactive-inferred {len(by.get('inactive-inferred', []))}개 · unknown {len(by.get('unknown', []))}개"
            + "(unknown = 판정 신호 부족 — 필요 여부를 이 표로 단정하지 않는다).")


# vllm_observed 키 → 라벨(닫힌 tripwire · 모르는 키는 키 이름 그대로 싣는다 — 버리지 않는다). 2026-09-22 S2 round 3: 옛 표는 두 행을
#   하나의 `source` 로 묶어 인증서 값을 "엔진 자기보고" 로 적었다(사후 사실 검증 오답). 생산자 표기는 키마다 `<키>_source`(·`_producer`) 가
#   정본이고, 옛 단일 `source` 는 키별 출처가 없을 때만 쓴다(옛 스냅샷 재렌더 호환 — 새 생산자는 키별로 준다).
_VLLM_OBS_LABELS = {
    "engine_self_report": "vLLM 엔진 자기보고(엔진 로그 기동 배너)",
    "certificate_vllm_version": "인증서 `vllm_version`(강한 일치 키)",
    "wheel_meta": "wheel 메타(`vllm_build`)",
}
_VLLM_OBS_ALWAYS = ("engine_self_report", "wheel_meta")
# 키별 메타 칸(evidence.vllm_observed): `_producer` = 생산자 코드(예 image-tag-line) · `_source` = 출처 서술 · `_basis` = 관측의 지위
#   (예 this-measurement · same-digest-other-log) · `_label` = 표 라벨 덮어쓰기. 행이 아니라 그 키 행의 출처 칸에 합친다.
_VLLM_OBS_META_SUFFIX = ("_source", "_producer", "_label", "_basis")


def _vllm_observed_rows(nobs: dict) -> list[tuple[str, str, str]]:
    """naming.vllm_observed → 0.4 표 행(키마다 값 · 생산자). 값이 dict 면 {value, source, label}. 값이 없으면 `미관측` + 생산자가 남긴
    사유(예: 같은 digest 의 엔진 로그 배너 없음)."""
    keys = [k for k in nobs if k not in ("source", "sources") and not k.endswith(_VLLM_OBS_META_SUFFIX)]
    order = [k for k in _VLLM_OBS_LABELS if k in keys or k in _VLLM_OBS_ALWAYS] + sorted(k for k in keys if k not in _VLLM_OBS_LABELS)
    per = _dict(nobs, "sources")
    rows = []
    for k in order:
        raw = nobs.get(k)
        val, src, label = raw, None, None
        if isinstance(raw, dict):
            val, src, label = raw.get("value"), raw.get("source"), raw.get("label")
        if not src:
            parts = []
            if nobs.get(f"{k}_producer"):
                parts.append(f"생산자 `{_cell(nobs.get(f'{k}_producer'))}`")
            if nobs.get(f"{k}_source") or per.get(k):
                parts.append(_cell(nobs.get(f"{k}_source") or per.get(k)))
            if nobs.get(f"{k}_basis"):
                parts.append(f"지위 `{_cell(nobs.get(f'{k}_basis'))}`")
            src = " · ".join(parts) or None
        label = label or nobs.get(f"{k}_label") or _VLLM_OBS_LABELS.get(k) or f"`{k}`"
        if val in (None, ""):
            # 값이 없으면 키별 사유만 싣는다 — 옛 단일 `source` 는 **있는 값**의 출처라 빈 칸에 붙이면 거짓 출처가 된다
            rows.append((_cell(label), _MISSING_TEXT, _cell(src) if src else _MISSING_TEXT))
        else:
            src = src or nobs.get("source")
            rows.append((_cell(label), _cell(val), _cell(src) if src else "출처 미기재"))
    return rows


def _f_meta(c: _Ctx) -> str:
    f = c.facts
    camp, pub, appr = _dict(f, "campaign"), _dict(f, "publication"), _dict(f, "approval")
    rows = [
        ("태그", f"`{f.get('tag')}`"),
        ("형식", f"`{FORMAT}`"),
        ("앵커", "`PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다"),
        ("캠페인 · 셀 · 노드", " · ".join(_camp_id(camp) if k == "id" else _cell(camp.get(k)) for k in ("id", "cell", "node"))),
        ("발행 모드", _cell(camp.get("mode"))),
        ("발행 토픽", _cell(pub.get("topic"))),
        ("work-manifest", _cell(pub.get("manifest_ref"))),
        ("증거 등급(task_class)", _cell(_task_class(f))),
        *_approval_rows(appr, _dict(f, "prior_approval")),
        ("현행 full 정의 충족(측정 등급)", _bench_definition_verdict(_dict(f, "bench_definition"))),
        ("독립 사실 검증(발행 전)", _factcheck_line(f.get("factcheck"))),
    ]
    return "\n".join(_table(("항목", "값"), rows))


def _approval_rows(appr: dict, prior: dict) -> list[tuple[str, str]]:
    """00 메타 승인 세 칸. 이 판의 승인(continue 가 적는다)이 먼저 · 없으면 재생 전용 대체 — 원 발행의 승인(봉인 페이로드 기록)을
    **이 판의 승인이 아니라고** 표지해 싣는다(2026-09-29 plan_26092908 §4.5 F4). 둘 다 없으면 미관측."""
    if appr or not prior:
        return [("승인 출처", _cell(appr.get("source"))), ("승인 시각(UTC)", _cell(appr.get("approved_utc"))),
                ("승인 발화(전사)", _cell(appr.get("approved_by")))]
    tag = f"(재생 전용 대체 — 원 발행의 승인 · 이 판의 승인은 continue 가 적는다 · {_cell(prior.get('replay_source'))})"
    return [("승인 출처", f"{_cell(prior.get('source'))}{tag}"), ("승인 시각(UTC)", f"{_cell(prior.get('approved_utc'))}(원 발행)"),
            ("승인 발화(전사)", f"{_cell(prior.get('approved_by'))}(원 발행)")]


def _yes_no(v) -> str:
    return "예" if v is True else ("아니오" if v is False else _MISSING_TEXT)


def _bench_definition_source(bd: dict) -> str:
    src, ln = bd.get("source"), bd.get("source_line")
    return f"{_cell(src)}:{ln}" if src and isinstance(ln, int) else _cell(src)


def _bench_definition_verdict(bd: dict) -> str:
    """현행 full 정의 충족 한 칸(00 메타 · 03 §3.5 공용). 판정은 evidence.bench_definition 이 했다 — 여기서 재판정하지 않는다."""
    if not bd:
        return _MISSING_TEXT
    rep_n = bd.get("this_repeats")
    return (f"{_yes_no(bd.get('meets_current_full'))} · 이 측정의 반복 {_cell(rep_n)} · 정의 출처 "
            f"`{_bench_definition_source(bd)}`(03 §3.5)")


def _factcheck_line(fc) -> str:
    """00 메타의 사실 검증 한 칸 — continue 가 `inputs/factcheck.json`(또는 사람 면제)을 보고 facts.factcheck 로 넣는다(2026-09-22
    S2 round 2 F11). publish 직후(검증 전)에는 '미실시' 다 — 침묵은 '없음' 과 구분되지 않는다."""
    if not isinstance(fc, dict) or not fc:
        return "미실시 — continue 가 `inputs/factcheck.json`(저작자가 아닌 Agent 의 검증 보고)을 요구한다"
    st = fc.get("status")
    if st == "passed":
        return (f"통과 — 검증자 `{_cell(fc.get('checker'))}`(저작자 `{_cell(fc.get('author'))}` 아님) · 주장 {_cell(fc.get('claims_checked'))}건 · "
                f"지적 {_cell(fc.get('items'))}건 중 고침 {_cell(fc.get('fixed'))}건 · 열림 0 · {_cell(fc.get('checked_utc'))}")
    if st == "waived":
        # 면제한 것이 무엇인지 사유코드대로 말한다(2026-09-22 적대 리뷰: 1판은 열린 지적이 남은 보고를 면제해도 "독립 검증 없이 발행" 이라
        #   적었다 — 검증은 있었고 지적이 열린 채였다. 배포 본문의 오도 바이트다).
        what = {"HINT_FACTCHECK_ABSENT": "독립 검증 보고 없이 발행", "HINT_FACTCHECK_INVALID": "검증 보고 모양 결함인 채 발행",
                "HINT_FACTCHECK_OPEN": "검증 지적이 열린 채 발행"}.get(str(fc.get("reason_code")), "사실 검증 게이트 미통과인 채 발행")
        return (f"**사람 면제**({what}) — 사유코드 `{_cell(fc.get('reason_code'))}` · 열린 지적 "
                f"{_cell(fc.get('open'), '0')} · 면제 발화 {_cell(fc.get('waiver'))}")
    return f"미통과(`{_cell(fc.get('reason_code'))}`) — 열린 지적 {_cell(fc.get('open'), '0')}"


def _missing_meanings() -> dict:
    try:
        from . import evidence  # 지연 적재 — 결손 코드표의 소유자는 evidence 다
    except ImportError as e:
        core.fail("HINT_OWNER_MODULE_MISSING", f"hintlib.evidence 적재 실패(결손 코드표 · MISSING_CODES): {e}",
                  "facts['missing'] 를 코드→뜻 dict 로 넘기거나 evidence 모듈을 복구한다.")
    codes = getattr(evidence, "MISSING_CODES", None)
    if not isinstance(codes, dict):
        core.fail("HINT_OWNER_MODULE_MISSING", "hintlib.evidence.MISSING_CODES 가 dict 가 아니다.")
    return codes


def _missing_block(missing, meanings: dict | None = None, reasons: dict | None = None) -> str:
    """결손 표. 0건도 명시한다(침묵은 '없음' 과 구분되지 않는다 — 옛 render_missing_block). reasons = {코드: 이 셀의 사유}(2026-09-29 ·
    plan_26092908 §4.2 — 예: native 기동 기록을 producer 가 쓰지 못한 사유 `native_launch_error`) — 있으면 `이 셀의 사유` 열을 단다."""
    if isinstance(missing, dict):
        items = sorted((str(k), v) for k, v in missing.items())
    elif isinstance(missing, (list, tuple)):
        items = [(code, None) for code in sorted({str(x) for x in missing})]
    else:
        items = []
    if not items:
        return "**결손**: _없음 — 결손 코드표를 전수 대조한 결과 0건이다(침묵은 '없음' 과 구분되지 않는다 — 없음도 적는다)._"
    rows = []
    for code, meaning in items:
        if not meaning:
            if meanings is None:
                meanings = _missing_meanings()
            meaning = meanings.get(code)
        rows.append((f"`{code}`", _cell(meaning or "미등록 사유코드 — `evidence.MISSING_CODES` 에 뜻을 등재하라")))
    out = ["**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).", ""]
    rs = {str(k): v for k, v in (reasons or {}).items() if v}
    if rs:
        out += _table(("코드", "뜻", "이 셀의 사유"), [(a, b, _cell(rs.get(code.strip("`")), "—")) for (a, b), (code, _m)
                                                   in zip(rows, items)])
    else:
        out += _table(("코드", "뜻"), rows)
    return "\n".join(out)


def _missing_codes(missing) -> set[str]:
    if isinstance(missing, dict):
        return {str(k) for k in missing}
    if isinstance(missing, (list, tuple)):
        return {str(x) for x in missing}
    return set()


def _attestation_scope_line(f: dict) -> str | None:
    """attestation 을 인용하는 자리에 붙는 범위 줄(2026-09-22 · plan_26092119 S2 round 3). 2차 저작자 지적: FACT:sub_recipe 와 FACT:missing 이
    Phase 0 스모크 config(`q38fn-nvfp4-mmap` · 2026-09-09)의 attestation 을 이 셀의 ABI 정합 출처처럼 실었다(부류 4 귀속 위험을 기계
    블록이 만들었다). evidence.attestation_scope 는 attestation 이 묶이면 {config, written_utc, written_utc_source, bound_by, phase, path,
    scope} 를 준다 — scope 는 생산자 판정 그대로 싣는다(config 가 셀과 다르면 "image-build/smoke — not this cell's run" · 같으면
    "같은 실행인지 미검증"). 이 함수는 범위를 **새로 판정하지 않는다**. 없으면 줄도 없다."""
    sc = f.get("attestation_scope")
    if not isinstance(sc, dict) or not sc:
        return None
    parts = [f"config `{_cell(sc.get('config'))}`"]
    if sc.get("phase"):
        parts.append(f"phase {_cell(sc.get('phase'))}")
    parts.append(f"작성 {_cell(sc.get('written_utc'))}" + (f"({_cell(sc.get('written_utc_source'))})" if sc.get("written_utc_source") else ""))
    if sc.get("bound_by"):
        parts.append(f"묶은 근거 {_cell(sc.get('bound_by'))}")
    if sc.get("path"):
        parts.append(f"파일 `{_cell(sc.get('path'))}`")
    if sc.get("timing_note"):
        # 2026-09-29 plan_26092908 §4.5 F7: 작성 시각 대 측정 창(생산자 판정 그대로)
        parts.append(f"측정 창 대비 {_cell(sc.get('timing_note'))}")
    return f"> **attestation 범위** — {' · '.join(parts)}: **{_cell(sc.get('scope'))}**"


def _f_missing(c: _Ctx) -> str:
    out = _missing_block(c.facts.get("missing"), reasons=_dict(c.facts, "missing_reasons"))
    note = _attestation_scope_line(c.facts)
    if note and any("ATTESTATION" in code for code in _missing_codes(c.facts.get("missing"))):
        out += "\n\n" + note
    return out


def _f_bench_missing(c: _Ctx) -> str:
    """03 §3.3 측정 결손 — 00 결손 표 중 측정 코드만(옛 render_item3 의 03 결손 기재 · 선택 원천은 같은 facts.missing).
    인증서 부재면 옛 03 의 무인증서 배너를 그대로 옮긴다(selftest_hint_gate ① "판정되지 않았다" 단언의 새 자리)."""
    missing = c.facts.get("missing")
    codes = _missing_codes(missing)
    sel = [code for code in BENCH_MISSING_CODES if code in codes]
    out: list[str] = []
    m = _dict(c.facts, "measurement")
    if "HINT_MISSING_CERTIFICATE" in codes:
        # 2026-09-29 plan_26092908 §4.4: evidence 는 인증서가 **발행되는 판정**(explicit ∧ PASS · 또는 판정 미관측)에서만 이 결손을 싣는다
        #   (_certificate_not_due) — 배너도 그 뜻만 말한다.
        out += ["> **인증서가 없다** — 인증서가 발행되는 판정(루브릭 권한 explicit ∧ PASS, 또는 판정 · 권한 미관측)인데 인증서가 묶이지 않았다"
                "(발행은 `adversarial-benchmark` 의 책임).",
                "> 부재는 '성능이 나빴다' 가 아니라 인증서 형태로 '**그 형태로 판정되지 않았다**' 는 뜻이다 — 판정 "
                f"{_verdict_text(c.facts)} · 위 수치는 관측이다.", ""]
    elif m.get("verdict") in (REFUTE_MARKER, MAP_ONLY_MARKER) or (
            m.get("verdict") == "PASS" and not str(_dict(m, "sources").get("verdict") or "").startswith("certificate")):
        # 비발행 판정(REFUTE · OBSERVATION-ONLY · PASS ∧ 비발행 권한)은 결손이 아니다 — 판정 원천이 그 사실을 말한다(무인증서 배너 ✗)
        out += [f"> 인증서 비발행 판정(결손 아님) — {_verdict_text(c.facts)} · 인증서는 explicit ∧ PASS 판정에서만 나온다"
                f"(루브릭 권한 `{_cell(m.get('rubric_authority'))}`).", ""]
    if not sel:
        out.append("**측정 결손**: _없음 — 측정 결손 코드("
                   + " · ".join(f"`{x}`" for x in BENCH_MISSING_CODES)
                   + ")를 대조한 결과 0건이다(없음도 적는다)._")
        return "\n".join(out)
    sub = {k: v for k, v in missing.items() if k in sel} if isinstance(missing, dict) else sel
    block = _missing_block(sub)
    out.append(block.replace("**결손** —", "**측정 결손** —", 1))
    return "\n".join(out)


def _event_homes(templates: dict) -> dict[str, str]:
    """kind → 자리(파일 §절). 템플릿의 `블록:` 지시에서 **파생**한다(범례와 규칙이 두 자리에 손으로 적히지 않게)."""
    homes: dict[str, list[str]] = {}
    for name in TEMPLATE_FILES:
        doc = templates.get(name)
        if doc is None:
            continue
        for p in doc.prompts:
            for kind in _directives(p.fields)["blocks"]:
                homes.setdefault(kind, []).append(f"{name[:2]} §{p.chapter}")
    return {k: " · ".join(v) for k, v in homes.items()}


def _event_kinds_md(templates: dict) -> str:
    homes = _event_homes(templates)
    rows = [(f"`{k}`", f"`{EVENT_ID_PREFIX[k]}<n>`", " · ".join(EVENT_REQUIRED[k]), _cell(homes.get(k))) for k in EVENT_KINDS]
    return "\n".join(_table(("kind", "id", "필수 필드", "자리"), rows))


def _legend_md() -> str:
    out = ["**범례** — 린터가 hint-event 값 검증에 쓰는 어휘와 같은 상수에서 생성했다.", ""]
    out.append("- 전이등급: " + " · ".join(f"**{k}**({v})" for k, v in TRANSFER_CLASSES.items()))
    out.append("- 지위: " + " · ".join(f"**{k}**({v})" for k, v in VALUE_STATUSES.items()))
    out.append("- 현재지위(오진 · 권장 — **원래 주장**의 지위): " + " · ".join(f"**{k}**({v})" for k, v in CURRENT_STATUS_MEANINGS.items()))
    return "\n".join(out)


def _f_legend(c: _Ctx) -> str:
    return _event_kinds_md(c.templates) + "\n\n" + _legend_md()


def _shipped_names(f: dict) -> set[str]:
    names: set[str] = set()
    for row in _dict(f, "slots").values():
        if isinstance(row, dict):
            for p in row.get("files") or []:
                names.add(Path(str(p)).name)
    return names


def _slot_reason(s: dict) -> str:
    """slots 표 '사유' 칸 — 저작자 선언(`rationale` · continue 가 01 §1.2 에서 옮긴다)이 먼저, 없으면 artifacts 의 기계 파생 사유
    (`machine_reason` · 2026-09-29 plan_26092908 §4.5 F5 — 표지를 붙인다), 둘 다 없으면 미관측."""
    if s.get("rationale"):
        return _cell(s.get("rationale"))
    if s.get("machine_reason"):
        return f"{_cell(s.get('machine_reason'))}(기계 파생)"
    return _MISSING_TEXT


def _f_slots(c: _Ctx) -> str:
    slots = _dict(c.facts, "slots")
    if not slots:
        return "_슬롯 판정 미관측 — artifacts 수집 결과가 없다(결손)._"
    rows = []
    for name in sorted(slots):
        s = slots[name] if isinstance(slots[name], dict) else {}
        ev = _dict(s, "evidence")
        files = [f"`{Path(str(p)).as_posix()}`" for p in (s.get("files") or [])]
        rows.append((f"`{name}`", "실림" if s.get("files") else "안 실림", _cell(files, "—"),
                     _cell(s.get("confidence")), _cell(f"{ev.get('kind')}:{ev.get('ref')}" if ev else None),
                     _slot_reason(s)))
    out = _table(("슬롯", "실림", "파일", "신호", "적용 증거", "사유"), rows)
    # 2026-09-22 통합(artifacts 요청): 빌드 레시피 슬롯의 **이미지 대 레시피** 경고와 싣지 못한 COPY 원천을 배포 본문에 싣는다 —
    #   수신자가 "실린 Dockerfile 이 그 이미지를 지은 바이트인가" 를 모르면 재현이 조용히 다른 빌드가 된다(태그2 실측: 실린
    #   Dockerfile.source-build 가 이미지 빌드 뒤 재렌더된 작업트리 수정본이었다).
    notes: list[str] = []
    for name in sorted(slots):
        ev = _dict(slots[name] if isinstance(slots[name], dict) else {}, "evidence")
        for r in ev.get("recipe_vs_image") or []:
            if isinstance(r, dict) and r.get("warning"):
                notes.append(f"- ⚠ `{name}` `{_cell(r.get('path'))}` — {_cell(r.get('warning'))}"
                             f"(이미지 Created {_cell(r.get('image_created_utc'))} · 마지막 커밋 {_cell(r.get('last_commit_utc'))})")
        for r in ev.get("context_not_shipped") or []:
            if isinstance(r, dict):
                notes.append(f"- ⚠ `{name}` COPY 원천 `{_cell(r.get('source'))}` 미실림 — {_cell(r.get('why'))}")
        # 2026-09-22 S2 round 2(F9): 실린 Dockerfile 이 측정 이미지를 지은 리비전인가(artifacts 가 docker history 와 대조해 고른 커밋) ·
        #   측정 뒤 재생성된 env 형상(측정 당시 값이 아니다 — S2 1차: `.env.interconnect` 가 09-17 재생성본이라 NET/IB 대 Socket 이 갈렸다).
        # 2026-09-22 S2 round 3: 리비전은 **복수**(`selected_revisions` — build_recipe 의 Dockerfile · requirements 와 compose 슬롯의
        #   compose · 러너)다. 옛 표는 단수만 실어 compose 가 측정 뒤 수정본인지 수신자가 알 수 없었다(2차 기계 채점 AC2-sub-compose).
        revs = [r for r in (ev.get("selected_revisions") or []) if isinstance(r, dict)]
        rev_paths = {str(r.get("path")) for r in revs if r.get("path")}
        sel = ev.get("selected_revision")
        if isinstance(sel, dict) and sel and (not sel.get("path") or str(sel.get("path")) not in rev_paths):
            notes.append(_revision_line(name, sel))
        notes += [_revision_line(name, r) for r in revs]
        notes += [f"- ⚠ `{name}` 리비전 경고: {_cell(w)}" for w in (ev.get("revision_warnings") or []) if isinstance(w, str) and w]
        # 싣지 않은 파일 목록(`excluded` · 사유 post-measurement-regenerated)이 이미 말하는 파일은 여기서 다시 적지 않는다(두 자리 ✗)
        listed = [str(x.get("file")) for x in (slots[name].get("excluded") or []) if isinstance(x, dict)
                  and str(x.get("reason") or x.get("why") or "").startswith(_POST_MEASUREMENT)] if isinstance(slots[name], dict) else []
        regen = [x for x in (ev.get("post_measurement_regenerated") or []) if isinstance(x, str) and x
                 and not any(f and (x == f or x.endswith("/" + f)) for f in listed)]
        if regen:
            shipped = {Path(str(p)).name for p in (slots[name].get("files") or [])} if isinstance(slots[name], dict) else set()
            on = [x for x in regen if Path(x).name in shipped or f"{Path(x).name}.template" in shipped]
            off = [x for x in regen if x not in on]
            if on:
                notes.append(f"- ⚠ `{name}` 측정 뒤 재생성된 파일(mtime > 측정 시각)을 실었다: " + " · ".join(f"`{x}`" for x in on)
                             + " — 실린 형상은 측정 당시 값과 다를 수 있다(측정 당시 값 = 01 §1.4 측정 env 관측)")
            if off:
                notes.append(f"- `{name}` 측정 뒤 재생성된 파일(mtime > 측정 시각)은 싣지 않았다: " + " · ".join(f"`{x}`" for x in off)
                             + " — 측정 당시 값이 아니다(측정 당시 값 = 01 §1.4 측정 env 관측)")
    if notes:
        out += ["", "**레시피 경고 · 실린 리비전**(기계 관측 — 수신자 빌드가 여기서 갈라질 수 있다)", ""] + notes
    ex = _excluded_lines(slots)
    if ex:
        out += ["", "**싣지 않은 파일**(슬롯별 사유 — 빌드 때 skip 된 패치는 아래 적용 집합 표에 있다)", ""] + ex
    out += _file_marks_table(slots)
    out += _agent_requests_table(c.facts)
    return "\n".join(out)


def _file_records(slots: dict) -> list[tuple[str, dict]]:
    out = []
    for name in sorted(slots):
        row = slots[name] if isinstance(slots[name], dict) else {}
        for r in row.get("file_records") or []:
            if isinstance(r, dict) and r.get("path"):
                out.append((name, r))
    return out


def _verification_cell(r: dict) -> str:
    """01 표의 `검증` 칸 — 표시 3자리의 셋째(plan_26092908 §4.2 U1+). 형식 `<verification> · <generated_by>` 는 린터가 대조한다."""
    return f"`{_cell(r.get('verification'))}` · {_cell(r.get('generated_by'))}"


def _file_marks_table(slots: dict) -> list[str]:
    """파일별 검증 표시 · 관련성(2026-09-29 · plan_26092908 §4.2·§4.3) — artifacts `slots.<slot>.file_records` 를 옮긴다(판정 ✗)."""
    recs = _file_records(slots)
    if not recs:
        return []
    out = ["", "**파일별 검증 · 관련성**(`generated-unverified` = 실행 검증되지 않은 생성물 — 파일 첫 줄 경고 · PAYLOAD 파일 기록과 같은 표시 · "
               "관련성 = ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 서명 — 둘 다 관측일 때만 required/inactive-inferred)", ""]
    out += _table(("슬롯", "파일", "검증", "검증 근거", "관련성", "관련성 근거"),
                  [(f"`{name}`", f"`{_cell(r.get('path'))}`", _verification_cell(r), _cell(r.get("verification_basis")),
                    f"`{_cell(r.get('relevance'))}`", _cell(r.get("relevance_basis"), "—")) for name, r in recs])
    return out


def _agent_requests_table(f: dict) -> list[str]:
    """렌더러로 만들 수 없는 자리(artifacts `agent_requests` · plan §4.2 "렌더러 우선 · Agent 폴백") — publish 가 빈 파일 + 자리표시를
    두고, 저작 Agent 가 첫 줄 경고를 지킨 채 본문을 쓴다(린터: 경고 줄 · 자리표시 잔존 · 빈 본문)."""
    reqs = [r for r in (f.get("agent_requests") or []) if isinstance(r, dict) and r.get("path")]
    if not reqs:
        return []
    out = ["", f"**Agent 저작 요청 {len(reqs)}건**(렌더러 입력이 없어 기계가 만들지 못한 자리 — 저작 Agent 가 증거에서 쓴다 · 첫 줄 "
               f"`{UNVERIFIED_MARK} … 생성: agent` 유지 · verification 은 저작 뒤에도 generated-unverified)", ""]
    out += _table(("파일", "사유", "입력"), [(f"`{_cell(r.get('path'))}`", _cell(r.get("why")), _cell(r.get("inputs"), "—"))
                                            for r in reqs])
    return out


def _revision_line(slot: str, r: dict) -> str:
    """실린 파일 하나의 리비전 줄(artifacts `selected_revision(s)` 모양 그대로 — 판정 ✗)."""
    hm = r.get("history_match") if isinstance(r.get("history_match"), dict) else None
    commit = r.get("commit")
    head = f"커밋 `{str(commit)[:12]}`" if commit else "리비전 미특정"
    path = f" `{_cell(r.get('path'))}`" if r.get("path") else ""
    match = (f"docker history 대조 {_cell(hm.get('matched'))}/{_cell(hm.get('total'))}" if hm else "docker history 대조 없음")
    extra = ""
    if r.get("shipped") not in (None, ""):
        extra += f" · 실린 바이트 `{_cell(r.get('shipped'))}`"
    if r.get("basis"):
        # compose 슬롯 추적 파일(artifacts `_select_tracked`)의 판정 근거 — mtime ≤ 측정 · 측정 전 마지막 커밋 · 미관측
        extra += f" · 근거 `{_cell(r.get('basis'))}`"
    if r.get("executed") is False:
        # FACT_FIX2 G5: 이 셀이 실행하지 않은 경로(generated-unverified)의 파일 — 쓰인 바이트가 없다(mtime 판정은 추론이지 관측 ✗)
        extra += " · 쓰인 바이트임을 관측으로 확인 해당 없음(실행 안 됨)"
    elif isinstance(r.get("verified"), bool):
        extra += f" · 쓰인 바이트임을 관측으로 확인 {_yes_no(r.get('verified'))}"
    after = r.get("commits_after_measurement")
    if isinstance(after, list) and after:
        extra += f" · 측정 뒤 커밋 {len(after)}건"
    line = f"- `{slot}`{path} 실린 리비전: {head} · 선택 방법 `{_cell(r.get('method'))}` · {match}{extra}"
    return line + (f" — ⚠ {_cell(r.get('warning'))}" if r.get("warning") else "")


# 슬롯 제외 사유 중 **요약만** 싣는 것(닫힌 tripwire — artifacts `_applied` 의 why 접두): 패치가 아닌 파일은 개수 · 이름만.
_EXCLUDED_BRIEF = ("known-non-patch",)
# 적용 집합 표(1.1)에 이미 행이 있는 사유 — 두 자리에 싣지 않는다(바이트 채점: 중복 표는 수신자 쓸모 없이 B 를 늘렸다).
_EXCLUDED_IN_APPLIED = ("skipped",)
# artifacts 가 compose 슬롯에 적는 사유 코드(공유 계약 2026-09-22 S2 round 3 · artifacts.POST_MEASUREMENT_REGENERATED 와 짝) → 수신자용 뜻.
#   생산자의 why 가 괄호 설명을 이미 달았으면 뜻을 덧붙이지 않는다(같은 말 두 번 ✗).
_POST_MEASUREMENT = "post-measurement-regenerated"
_EXCLUDED_MEANINGS = {
    _POST_MEASUREMENT: "측정 뒤 재생성된 형상 — 렌더러가 측정 뒤에 바뀌면 측정 당시 없던 키 · 다른 값이 실린다 "
                       "(측정 당시 값 = 01 §1.4 측정 env 관측)",
}
# 제외 행에서 싣는 추가 칸(닫힌 목록 — 나머지 칸은 PAYLOAD 에 있다 · 바이트 채점: 같은 출처 문구 반복은 쓸모 없이 B 를 늘렸다)
_EXCLUDED_EXTRA_KEYS = ("mtime_utc", "measured_utc")


def _excluded_lines(slots: dict) -> list[str]:
    out: list[str] = []
    for name in sorted(slots):
        row = slots[name] if isinstance(slots[name], dict) else {}
        brief: dict[str, list[str]] = {}
        for x in row.get("excluded") or []:
            if not isinstance(x, dict):
                continue
            why = str(x.get("why") or x.get("reason") or "")
            code = why.split("(", 1)[0].strip()
            if code in _EXCLUDED_IN_APPLIED:
                continue
            if code in _EXCLUDED_BRIEF:
                brief.setdefault(code, []).append(str(x.get("file")))
                continue
            extras = [f"{k} {_cell(x[k])}" for k in _EXCLUDED_EXTRA_KEYS if x.get(k) not in (None, "")]
            meaning = _EXCLUDED_MEANINGS.get(code) if "(" not in why else None
            out.append(f"- `{name}` `{_cell(x.get('file'))}` — `{_cell(why)}`" + (f" · {meaning}" if meaning else "")
                       + (f" · {' · '.join(extras)}" if extras else ""))
        for code, files in sorted(brief.items()):
            out.append(f"- `{name}` {code} {len(files)}건: " + " · ".join(f"`{_cell(f_)}`" for f_ in files))
    return out


def _missing_sentence(f: dict, code: str, absent_text: str) -> str:
    """결손 문구를 결손 표에서 **파생**한다(2026-09-29 · plan_26092908 §4.2 V7 — 01 은 "결손" 이라 적는데 결손 표에는 코드가 없던 침묵).
    코드가 facts.missing 에 있으면 그 코드와 뜻(evidence.MISSING_CODES) · 이 셀의 사유(missing_reasons) · 없으면 absent_text."""
    missing = f.get("missing")
    if code not in _missing_codes(missing):
        return absent_text
    meaning = missing.get(code) if isinstance(missing, dict) else None
    if not meaning:
        try:
            meaning = _missing_meanings().get(code)
        except core.HintError:
            meaning = None
    reason = _dict(f, "missing_reasons").get(code)
    return (f"_결손 `{code}` — {_cell(meaning, '뜻 미등록')}" + (f" · 이 셀의 사유: {_cell(reason)}" if reason else "")
            + "(00 §0.7 결손 표)._")


def _f_applied_set(c: _Ctx) -> str:
    a = _dict(c.facts, "applied_set")
    if not a:
        return _missing_sentence(c.facts, "HINT_MISSING_APPLIED_SET",
                                 "_적용 집합 없음 — 빌드 패치 슬롯 입력이 없고 결손 코드도 없다(판정할 대상이 없다)._")
    shipped = _shipped_names(c.facts)
    out = [f"**적용 집합** — 상태 `{_cell(a.get('status'))}` · 출처 `{_cell(a.get('source'))}`"
           + (" · 원장 없음: 아래 결과는 **라벨된 재구성**이다(관측 아님)" if a.get("status") == "unobservable" else ""), ""]
    patches = [p for p in (a.get("patches") or []) if isinstance(p, dict)]
    if patches:
        rows = []
        for p in patches:
            res = p.get("result")
            shown = "unobserved(적용 미관측)" if res == "unobserved" else _cell(res)
            rows.append((_cell(p.get("phase")), f"`{_cell(p.get('file'))}`", shown, _cell(p.get("result_source")),
                         _cell(p.get("reason"), "—"), "실림" if str(p.get("file")) in shipped else "안 실림",
                         _cell(p.get("declared_model_trigger"), "—")))
        out += _table(("단계", "파일", "결과", "결과 출처", "사유", "실림", "model-trigger"), rows)
        used = sorted({str(p.get("result_source")) for p in patches if p.get("result_source")})
        if used:
            out += ["", "**결과 출처 범례**", ""]
            out += [f"- `{u}` — {RESULT_SOURCE_MEANINGS.get(u, '뜻 미등록(template.RESULT_SOURCE_MEANINGS)')}" for u in used]
        out += _patch_evidence_table(patches)
    else:
        out.append("_빌드 패치 없음 — 이 이미지에 빌드 패치 슬롯 입력이 없다._")
    probes = [x for x in (a.get("probes") or []) if isinstance(x, dict)]
    if probes:
        # 순위별 탐침 결과(artifacts `applied_set.probes`) — 조용한 강등 ✗: 원장이 왜 관측되지 않았는지(부재 · 이미지 없음 · 관측
        #   실패)를 본문이 말한다(2026-09-22 통합 · artifacts 요청).
        out += ["", "**원장 탐침**(순위: attestation → 메인 로컬 이미지 → 재구성)", ""]
        out += _table(("순위", "결과", "대상"), [(_cell(x.get("tier")), _cell(x.get("result")),
                                                 _cell(x.get("ref") or x.get("image"), "—")) for x in probes])
        note = _attestation_scope_line(c.facts)
        if note and any(x.get("tier") == "attestation" for x in probes):
            out += ["", note]
    rec = _dict(a, "reconstruction")
    args = _dict(rec, "build_args")
    if rec:
        out += ["", f"**재구성 입력** — `{_cell(rec.get('source'))}`(docker history build-arg × 패치 자기게이트 규칙)", ""]
        out += _table(("build-arg", "값"), [(f"`{k}`", _cell(args[k], "(빈 값)")) for k in sorted(args)]) if args \
            else ["_build-arg 미관측._"]
    return "\n".join(out)


def _markers_text(mk) -> str | None:
    if not isinstance(mk, dict) or not mk:
        return None
    s = f"{_cell(mk.get('status'))} {_cell(mk.get('found'), '0')}/{_cell(mk.get('declared'), '0')}"
    miss = mk.get("missing")
    return s + (f" · 부재 {_cell(miss)}" if miss else "")


def _patch_evidence_table(patches: list[dict]) -> list[str]:
    """패치별 판정 근거(2026-09-22 · plan_26092119 S2 round 3). artifacts 는 패치마다 판정에 쓴 관측(정적 규칙 · 빌드 루프 · 적용 표지 ·
    스크립트 바이트 정체 · 이미지 사본 sha256 · 원장 분류)을 PAYLOAD 에만 두고 본문에 싣지 않았다 — 2차 기계 채점이 "스크립트 바이트가
    빌드 뒤 다시 쓰였다" 는 사실을 본문에서 찾지 못했다(AC2-sub-patch-bytes). 판정 ✗ — 옮기기만 한다. 빌드 루프는 단계마다 같은 문장이라
    단계별 한 줄로 접는다."""
    keys = ("evidence", "script_identity", "image_script_sha256", "ledger_result_source")
    rows_p = [p for p in patches if any(p.get(k) for k in keys)]
    if not rows_p:
        return []
    rows, loops = [], {}
    for p in rows_p:
        ev = p.get("evidence") if isinstance(p.get("evidence"), dict) else {}
        static = ev.get("static")
        sha = p.get("image_script_sha256")
        # 2026-09-29 plan_26092908 §4.5 F1: 원장 경로는 이미지 사본이 아니라 원장이 기록한 실행 바이트 sha 다 — 출처를 붙여 옮긴다
        led = p.get("ledger_script_sha256")
        sha_txt = _code(sha) if sha else (f"{_code(led)}(빌드 원장 script_sha256 — 이미지 사본 아님)" if led else "—")
        rows.append((f"`{_cell(p.get('file'))}`", _cell(p.get("script_identity"), "—"), sha_txt,
                     _cell(_markers_text(ev.get("markers")), "—"), _cell(static, "—"), _cell(p.get("ledger_result_source"), "—")))
        if ev.get("loop"):
            loops.setdefault(str(p.get("phase")), [])
            if ev["loop"] not in loops[str(p.get("phase"))]:
                loops[str(p.get("phase"))].append(ev["loop"])
    out = ["", "**판정 근거**(패치별 — artifacts 가 판정에 쓴 관측 · 스크립트 바이트 정체 = 실린 바이트가 측정 이미지에 구워진 바이트인가)", ""]
    out += _table(("파일", "스크립트 바이트 정체", "이미지 사본 sha256", "적용 표지(상태 찾음/선언)", "정적 규칙", "원장 분류"), rows)
    out += [""] if loops else []
    for ph in sorted(loops):
        out.append(f"- `{ph}` 빌드 루프: " + " · ".join(_cell(x) for x in loops[ph]))
    return out


def _candidates(f: dict) -> list[dict]:
    raw = f.get("value_status_candidates")
    if isinstance(raw, dict):
        return [{"knob": k, **(v if isinstance(v, dict) else {"value": v})} for k, v in sorted(raw.items())]
    return [x for x in (raw or []) if isinstance(x, dict) and (x.get("knob") or x.get("name"))]


def _knob(c: dict) -> str:
    return str(c.get("knob") or c.get("name")).strip()


def _f_value_status(c: _Ctx) -> str:
    cands = _candidates(c.facts)
    if not cands:
        return "_서빙 노브 후보 0개 — 트리플렛 yaml 을 읽지 못했다(결손)._"
    # 후보 근거(reason · lockset 출처 등)와 yaml 줄 주석을 **둘 다** 싣는다 — 한 칸에 "주석 or 근거" 로 접으면 lockset 이
    # 준 후보 근거가 주석에 가려진다(값의 지위를 가르는 1차 원천이 둘이다: 태그2 yaml 주석 "A1 기준선 — 음성대조").
    rows = [(f"`{_knob(x)}`", _value_text(x.get("value")).replace("|", "\\|"),
             _cell(x.get("candidate_status") or x.get("status"), "후보 없음"), _cell(x.get("reason"), "—"),
             _cell(x.get("comment"), "—"), _cell(x.get("source"))) for x in cands]
    out = [f"서빙 노브 {len(cands)}개 — 노브마다 `kind: value-status` 블록이 **정확히 1개** 있어야 한다(노브·값은 이 표의 글자 그대로 · "
           "값 칸의 `\\|` 는 `|` 로 적는다).", ""]
    out += _table(("노브", "값", "후보 지위", "후보 근거", "yaml 주석", "출처"), rows)
    return "\n".join(out)


def _qual_scope_text(q: dict) -> str:
    """발행 자격 근거(serve_proof)의 시점 범위(FACT_FIX2 G2 · evidence.serve_proof_scope 그대로 — attestation 범위와 같은 분류)."""
    sc = q.get("scope") if isinstance(q.get("scope"), dict) else None
    if not sc or not sc.get("timing_note"):
        return ""
    return (f" — 시점 범위: 작성 {_cell(sc.get('written_utc'))}({_cell(sc.get('written_utc_source'))}) · 측정 창 대비 "
            f"{_cell(sc.get('timing_note'))}")


def _qualification_line(f: dict) -> str:
    q = _dict(f, "qualification")
    srcs = q.get("sources") if isinstance(q.get("sources"), list) else []
    return (f"**기동 성공 판정(관측 · 발행 자격)**: health 200 = {_cell(q.get('health_200'))} · 추론 1회 = "
            f"{_cell(q.get('inference_observed'))} · 근거 {_cell([f'`{s}`' for s in srcs])}{_qual_scope_text(q)}")


def _f_reproduce(c: _Ctx) -> str:
    """재현 절차 표 + 단계별 명령 원문. 표에 **단계별 성공 판정**(artifacts `success`)을 싣는다 — 부록 A S5("기동 성공
    판정 기준")가 옛 태그 둘 다 ○ 였다. 명령은 여러 줄일 수 있어(렌더 4줄 등) 표 칸이 아니라 단계별 코드 블록으로 싣는다 —
    표 칸에서 줄바꿈을 공백으로 접으면 명령 넷이 한 줄로 붙어 복사가 깨진다(2026-09-22 리뷰)."""
    steps = [s for s in (c.facts.get("reproduce_steps") or []) if isinstance(s, dict)]
    rows, cmds = [], []
    for i, s in enumerate(steps, 1):
        dur = s.get("duration")
        dur = _MISSING_TEXT if dur in (None, "", "unobserved") else dur
        name = _cell(s.get("phase") or s.get("step") or s.get("name"))
        order = _cell(s.get("order") or i)
        obs = _dict(s, "observed")
        span = (f"{_cell(obs.get('start_utc'), '?')} → {_cell(obs.get('end_utc'), '?')} ({_cell(obs.get('bound'))})"
                if obs and (obs.get("start_utc") or obs.get("end_utc")) else _MISSING_TEXT)
        rows.append((order, name, _cell(s.get("success") or s.get("success_criterion")), _cell(dur), span, _cell(s.get("source")),
                     _cell(s.get("command_source"), "—")))
        cmd = s.get("command") or s.get("input")
        cmds.append(f"**{order}. {name}**")
        if isinstance(cmd, str) and cmd.strip():
            fence = "````" if "```" in cmd else "```"
            cmds += ["", f"{fence}sh", *cmd.strip("\n").split("\n"), fence, ""]
        else:
            cmds += ["", f"_명령 {_MISSING_TEXT}._", ""]
    if rows:
        out = _table(("순서", "단계", "성공 판정", "소요", "관측 구간(UTC)", "소요 출처", "명령 출처"), rows)
        # 2026-09-22 S2 round 3: build 단계의 이미지 층 CreatedAt 창(artifacts `observed.layer_span`)은 PAYLOAD 에만 있었다 — 2차 저작자는
        #   "빌드 소요 미관측" 이라 적었고 사실 검증이 docker history 층 시각을 지적했다(부류 7). 소요가 아니라 **창**이다(캐시 층은 이전 빌드 시각).
        spans = []
        for s in steps:
            ls = _dict(_dict(s, "observed"), "layer_span")
            if ls:
                spans.append(f"- `{_cell(s.get('phase') or s.get('step') or s.get('name'))}` 이미지 층 CreatedAt(docker history): "
                             f"{_cell(ls.get('oldest_utc'))} → {_cell(ls.get('newest_utc'))} · 층 {_cell(ls.get('layers'))}개 · 출처 "
                             f"`{_cell(ls.get('source'))}`" + (f" — {_cell(ls.get('note'))}" if ls.get("note") else ""))
        if spans:
            out += ["", "**빌드 층 시각**(관측 · 소요가 아니라 층 창 — 캐시 재사용 층은 이전 빌드의 시각을 가진다)", ""] + spans
        # 2026-09-22 S2 round 3: 렌더 단계 명령은 **지금의** 렌더러로 env 를 다시 만든다 — 측정 뒤 렌더러가 바뀌었으면(태그2: 09-17 Socket ·
        #   IB 끔 기준선) 측정 당시와 다른 키 · 값이 나온다(2차 바이트 채점 X 항목: "재현 1단계가 측정한 NET/IB 대신 Socket 을 낸다").
        #   compose 슬롯이 측정 뒤 재생성 형상을 적었을 때만 경고한다(사실 블록에 근거가 있을 때만 말한다).
        if any(str(s.get("phase") or s.get("step")) == "render" for s in steps) and _regenerated_env(c.facts):
            out += ["", "> ⚠ `render` 단계는 **지금의** 렌더러로 env 를 다시 만든다 — 측정 뒤 재생성된 env 형상(01 §1.1 싣지 않은 파일 "
                        f"`{_POST_MEASUREMENT}`: " + " · ".join(f"`{x}`" for x in _regenerated_env(c.facts))
                        + ")이 있으니 렌더러가 측정 뒤 바뀌었을 수 있다 — 다시 렌더한 값을 아래 '측정 실행의 env 관측' 표(측정 당시 값)와 대조한다."]
        out += ["", "**단계별 명령**(기계 파생 — `명령 출처` 가 `derived(…)` 면 사용법에서 파생 · `reconstructed(…)` 면 괄호 안 관측"
                "(docker history build-arg · compose build · bench JSON 필드)에서 재구성한 명령이다. 어느 쪽도 실행 원문(로그에 남은 줄)은 "
                "아니다)", ""]
        out += cmds
        while out and not out[-1]:
            out.pop()
    else:
        out = ["_재현 단계 미관측 — 셀 실행 기록이 없다(결손)._"]
    out += _context_map_lines(c.facts)
    out += _derived_env_lines(c.facts)
    out += ["", _qualification_line(c.facts)]
    return "\n".join(out)


def _context_map_lines(f: dict) -> list[str]:
    """슬롯 → 빌드 컨텍스트 매핑(2026-09-29 · plan_26092908 §4.6 V14 — artifacts `build_context_map` 를 옮긴다). zip 슬롯 이름
    (`build_patch_pre/`·`triplet/`)과 Dockerfile · compose 가 기대하는 자리(`build_patches_src/`·`envs/`·`configs/`)가 달라 수신자가
    어디에 둘지 추측했다(블라인드 X1)."""
    rows = [r for r in (f.get("build_context_map") or []) if isinstance(r, dict) and r.get("slot_path")]
    if not rows:
        return []
    out = ["", "**슬롯 → 빌드 컨텍스트 매핑**(빌드 컨텍스트 = `output/<토폴로지>/` · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · "
               "volumes · command — `—` 는 빌드 컨텍스트 입력이 아니다)", ""]
    out += _table(("zip 경로", "빌드 컨텍스트 자리", "근거"),
                  [(f"`{_cell(r.get('slot_path'))}`", f"`{_cell(r.get('context_path'))}`" if r.get("context_path") else "—",
                    _cell(r.get("basis"))) for r in rows])
    return out


def _derived_rule_text(d: dict) -> str | None:
    """파생 키의 실효값 규칙 한 칸(2026-09-29 · plan_26092908 §4.5 F6). artifacts 는 `rule_text` 를 형상 헤더에만 쓰고 `derived_out` 에서는
    뺀다(같은 문장 두 벌 ✗) — 표는 **같은 렌더러 표**(`effective_values` 선택지 → 키 값 · `value_rule` 키 → `{필드}` 규칙)에서 같은 모양으로
    다시 쓴다(옛 판은 `effective_values` 를 읽지 않아 선택지형 필드가 '규칙 미관측' 이었고 값 전사형은 원시 JSON 이었다)."""
    ev_ = d.get("effective_values")
    if isinstance(ev_, dict) and ev_:
        return " | ".join(f"{opt} → " + " · ".join(f"{k}={v}" for k, v in sorted(kv.items()))
                             for opt, kv in sorted(ev_.items()) if isinstance(kv, dict))
    vr = d.get("value_rule")
    if isinstance(vr, dict) and vr:
        return " · ".join(f"{k}={v}" for k, v in sorted(vr.items()))
    return None


def _derived_env_lines(f: dict) -> list[str]:
    """env 형상의 파생 키 · 실효값(2026-09-29 · plan_26092908 §4.6 — artifacts `env_shapes[].derived`). 한 manifest 필드에서 여러 키가
    파생되면 키마다 `<derived:<필드>→<KEY>>` 자리표시를 두고, 선택지별 실효값은 렌더러 표에서 옮긴다(블라인드 X1: NCCL_IB_DISABLE 에
    필드 값 'rdma' 를 넣을 뻔했다)."""
    rows = []
    for e in f.get("env_shapes") or []:
        if not isinstance(e, dict):
            continue
        for d in e.get("derived") or []:
            if isinstance(d, dict):
                ph = d.get("placeholders") if isinstance(d.get("placeholders"), dict) else {}
                rule = d.get("rule_text") or _derived_rule_text(d)
                rows.append((f"`{_cell(e.get('template') or e.get('file'))}`", f"`{_cell(d.get('field'))}`",
                             _cell([f"`{k}`" for k in (d.get("keys") or [])]), _cell([f"`{ph[k]}`" for k in sorted(ph)], "—"),
                             _cell(rule, "규칙 미관측(렌더러에서 읽지 못함 — 지어내지 않는다)"), _cell(d.get("source"), "—")))
    if not rows:
        return []
    out = ["", "**env 형상의 파생 키 · 실효값**(한 manifest 필드 → 여러 키 · 키마다 자기 자리표시 — 값은 필드 값이 아니라 아래 규칙의 결과다)", ""]
    out += _table(("형상", "manifest 필드", "키", "자리표시", "실효값 규칙", "출처"), rows)
    return out


def _regenerated_env(f: dict) -> list[str]:
    """compose 슬롯이 적은 측정 뒤 재생성 파일(evidence.post_measurement_regenerated ∪ excluded 의 post-measurement-regenerated 행 · 순서 보존)."""
    row = _dict(_dict(f, "slots"), "compose")
    out = [x for x in (_dict(row, "evidence").get("post_measurement_regenerated") or []) if isinstance(x, str) and x]
    for x in row.get("excluded") or []:
        if isinstance(x, dict) and str(x.get("reason") or x.get("why") or "").startswith(_POST_MEASUREMENT):
            f_ = str(x.get("file"))
            if not any(y == f_ or y.endswith("/" + f_) for y in out):
                out.append(f_)
    return out


def _f_sub_recipe(c: _Ctx) -> str:
    f = c.facts
    if _topology(f) != "multi":
        return "해당 없음 — single 토폴로지(서브 노드 · Ray worker 없음)."
    sr = f.get("sub_recipe")
    if not isinstance(sr, dict) or not sr:
        return _missing_sentence(f, "HINT_MISSING_SUB_RECIPE",
                                 "_서브 레시피 없음 — sub_recipe 를 싣지 않았고 결손 코드도 없다(생산자 artifacts 가 말하지 않은 부재)._")
    out = ["원본: `artifacts/compose/sub_recipe.json`(기계 파생 — 스모크와 같은 함수 `slave_forward.derive` 의 출력).", ""]
    if f.get("plane") == "native" or sr.get("_verification"):
        # F12(2026-09-29 · plan_26092908 §4.5): native 셀의 sub_recipe 는 원천 Docker 셀 트리플렛으로 렌더한 compose 형 참고다 — 아래 표를
        #   이 셀의 기동 사실처럼 읽지 않게 머리에 범위를 단다(이 셀은 native 정문으로 기동했다 · 01 §1.4 serve 단계 · native 기동 기록).
        ns = _dict(f, "native_source")
        out = [f"> **Docker 형 참고**(원천 셀 `{_cell(ns.get('cell'))}` · **이 셀 run 아님** · `generated-unverified`) — 아래 master/slave 표 · "
               "서브 전달 env · 기동 순서는 compose 경로의 형상이다. 이 셀(native)은 compose 로 기동되지 않았다 — 실제 기동은 01 §1.4 "
               "serve 단계(native 정문 `up`)와 native 기동 기록(`artifacts/triplet/native-launch.md`)이 말한다.", ""] + out
    if "HINT_MISSING_SUB_RECIPE" in _missing_codes(f.get("missing")):
        # native 셀(plan §4.2): 실린 sub_recipe 는 Docker 형 렌더러 산출물이고 서브가 실제로 받은 env · 기동은 관측되지 않았다
        out = [_missing_sentence(f, "HINT_MISSING_SUB_RECIPE", ""), ""] + out
    roles = _dict(sr, "role_diff")
    out += _table(("역할", "하는 일"), [(f"`{k}`", _cell(roles[k])) for k in sorted(roles)]) if roles else ["_역할 차이 미관측._"]
    order = sr.get("launch_order") if isinstance(sr.get("launch_order"), list) else []
    out += ["", "**기동 순서**", ""] + ([f"{i}. {_cell(s)}" for i, s in enumerate(order, 1)] or ["_미관측._"])
    fw = sr.get("env_forward") if isinstance(sr.get("env_forward"), list) else []
    rows = []
    for r in fw:
        if isinstance(r, dict):
            rows.append((f"`{_cell(r.get('key'))}`", _cell(r.get("group")), _cell(r.get("from") or r.get("source")),
                         _cell(r.get("why"), "—")))
        else:
            rows.append((f"`{_cell(r)}`", _MISSING_TEXT, _MISSING_TEXT, "—"))
    out += ["", "**서브 전달 env**(값은 원본 JSON · 여기서는 무리와 이유만)", ""]
    out += _table(("키", "무리", "출처", "이유"), rows) if rows else ["_전달 env 미관측._"]
    ids = sr.get("image_identity_args") if isinstance(sr.get("image_identity_args"), list) else []
    out += ["", "**이미지 정체성 build-arg**: " + (" · ".join(f"`{_cell(x)}`" for x in ids) or _MISSING_TEXT)]
    pnv = _dict(sr, "per_node_values")
    out += ["", "**노드별 값(형상)**", ""]
    out += _table(("키", "자리표시"), [(f"`{k}`", _code(pnv[k])) for k in sorted(pnv)]) if pnv else ["_미관측._"]
    pre = sr.get("preconditions") if isinstance(sr.get("preconditions"), list) else []
    out += ["", "**선행조건**", ""] + ([f"- {_cell(x)}" for x in pre] or ["_미관측._"])
    par = _dict(sr, "parity_attestation")
    out += ["", "**ABI 동일성 관측**(같아야 하는 것은 digest 가 아니라 ABI — 노드마다 로컬 빌드)", ""]
    out += _table(("축", "관측"), [(f"`{k}`", _cell(par[k])) for k in sorted(par)]) if par else ["_미관측._"]
    note = _attestation_scope_line(f)
    if note:
        out += ["", note]
    return "\n".join(out)


def _env_rows(f: dict) -> list[dict]:
    return [r for r in (f.get("measurement_env_observed") or []) if isinstance(r, dict) and r.get("key")]


def _env_value(v) -> str:
    """env 값 칸 — 코드 스팬(`<nic:cluster>` 같은 자리표시가 HTML 태그로 먹히지 않게) · 빈 값은 `(빈 값)` · 없음은 `미관측`."""
    if v is None:
        return _MISSING_TEXT
    return "(빈 값)" if str(v) == "" else _code(v)


def _f_measurement_env(c: _Ctx) -> str:
    """01 §1.4 — 측정한 실행의 엔진 로그가 echo 한 env(evidence.measurement_env_observed · 2026-09-22 · plan_26092119 S2 round 3).
    왜: 2차 페이로드는 측정 뒤 재생성된 env 형상(09-17 렌더러 = Socket · IB 끔)을 싣고 값을 "측정 당시 미관측" 으로 가렸는데, 같은
    셀의 엔진 로그는 NCCL 값 8개를 echo 하고 있었다(부류 7) — 형상은 걷고(01 §1.1 싣지 않은 파일) 관측을 사실로 싣는다."""
    rows = _env_rows(c.facts)
    if not rows:
        return ("_측정 실행의 env 관측 없음 — 측정 엔진 로그에서 env echo 줄(`<KEY> set by environment to <VALUE>` 등)을 찾지 못했다"
                "(evidence.measurement_env_observed · 부재 ≠ 미설정)._")
    out = [f"**측정 실행의 env 관측 {len(rows)}행**(측정한 실행의 엔진 로그가 echo 한 값 · 기계 발췌 — 측정 당시 값의 정본이다. 표에 없는 "
           "키는 로그가 echo 하지 않은 것이지 설정되지 않았다는 뜻이 아니다 · 측정 뒤 재생성된 env 형상은 01 §1.1 을 본다)", ""]
    def seen(r: dict) -> str:
        parts = ([f"{r['occurrences']}줄"] if r.get("occurrences") else []) + \
                ([f"로그 {r['seen_in_logs']}개"] if r.get("seen_in_logs") else [])
        rep = _ray_folded(r)
        if rep:
            parts.append(f"Ray 접힘 +{rep}(접힌 사본의 노드 미관측)")
        return " · ".join(parts) or "—"

    out += _table(("#", "키", "값", "노드", "종류", "출처(첫 줄)", "관측"),
                  [(str(i), f"`{_cell(r.get('key'))}`", _env_value(r.get("value")),
                    _cell(r.get("node") or (f"미특정({r.get('node_label')})" if r.get("node_label") else None), "미특정"),
                    _cell(r.get("kind"), "—"), _cell(r.get("source")), seen(r))
                   for i, r in enumerate(rows, 1)])
    notes = sorted({str(r.get("value_note")) for r in rows if r.get("value_note")})
    if notes:
        out += [""] + [f"- 값 주: {_cell(n)}" for n in notes]
    if any(_ray_folded(r) for r in rows):
        out += ["", f"- Ray 접힘: 출처 줄 끝의 `{_RAY_FOLD_TEXT}` — Ray 가 다른 프로세스의 같은 줄 N개를 한 줄로 접었다. 노드 칸은 "
                    "**첫 줄**의 노드이고, 접힌 사본이 어느 노드의 것인지는 로그에 없다(관측 대상 밖)."]
    wins = _log_windows(rows)
    if wins:
        out += ["", "- 로그 창: " + " · ".join(f"`{_cell(w)}`(이 표의 출처 로그 {len(fs)}개)" for w, fs in sorted(wins.items()))
                + " — 이 캡처는 엔진 출력의 **마지막 N줄**이다(측정 도구의 `tail -N` 상한에 닿았다 · 생산자 evidence `log_window`). "
                  "창 밖에서 echo 한 줄은 이 표에 없다 — 표에 없는 키 · 한 노드에만 있는 키는 창 밖이었을 수 있다."]
    return "\n".join(out)


def _log_windows(rows: list[dict]) -> dict[str, set[str]]:
    """evidence `log_window`(출처 로그가 측정 도구의 `tail -N` 상한에 닿은 꼬리 캡처) → {창: {출처 로그}}. 2026-09-22 · plan_26092119
    S2 round 3 통합: 생산자는 이 칸을 냈는데 렌더가 읽지 않아, 태그2(측정 로그 전부 tail-800)의 01 §1.5 가 한쪽만 남은 키를
    '<노드> 만 관측' 으로 실었다 — 캡처 창 밖이었을 수 있는 줄의 부재를 단언한 셈이었다(evidence 소유자 지적)."""
    out: dict[str, set[str]] = {}
    for r in rows:
        w = r.get("log_window")
        if isinstance(w, str) and w.strip():
            out.setdefault(w.strip(), set()).add(re.sub(r":L?\d+$", "", str(r.get("source") or "")))
    return out


# evidence.measurement_env_observed 의 `ray_repeated`(출처 줄의 `[repeated Nx across cluster]` N). 2026-09-22 · plan_26092119 S2 round 3
#   적대 리뷰: 01 §1.5 노드 접기가 이 칸을 읽지 않아 태그2 라이브 재생에서 `NCCL_IB_HCA` 등 6키를 "sub 만 관측 = 다른 쪽 로그가 echo 하지
#   않았다" 로 실었다 — 실제로는 그 줄이 캡처 로그에 **한 번** 있고 Ray 가 다른 프로세스 사본 2개를 접었다(접힌 사본의 노드는 미관측).
#   배포 FACT 가 관측이 말하지 않는 부재를 단언한 오도 바이트였다 → 접힘은 표에 싣고, 한쪽만 관측의 판정을 "가를 수 없음" 으로 낮춘다.
_RAY_FOLD_TEXT = "[repeated Nx across cluster]"


def _ray_folded(r: dict) -> int:
    v = r.get("ray_repeated")
    return v if isinstance(v, int) and not isinstance(v, bool) and v > 0 else 0


def _f_measurement_env_nodes(c: _Ctx) -> str:
    """01 §1.5 — 01 §1.4 표를 키 × 노드로 접는다(판정 ✗ · 같은지 다른지만). 양 노드가 같은 값을 echo 한 키는 이름만 싣고(표 중복 ✗ ·
    바이트 채점), 다르거나 한쪽만 관측된 키만 행으로 싣는다 — 두 노드가 같아야 하는 값의 어긋남이 여기서 보인다."""
    f = c.facts
    if _topology(f) != "multi":
        return "해당 없음 — single 토폴로지(서브 노드 없음)."
    rows = _env_rows(f)
    if not rows:
        return "_노드별 env 관측 없음 — 01 §1.4 측정 env 관측 표가 비었다._"
    by: dict[str, dict[str, list[str]]] = {}
    folded: dict[str, int] = {}
    windowed: set[str] = set()
    for r in rows:
        key = str(r.get("key"))
        if _log_windows([r]):
            windowed.add(key)
        vals = by.setdefault(key, {}).setdefault(str(r.get("node") or "미특정"), [])
        v = _env_value(r.get("value"))
        if v not in vals:
            vals.append(v)
        folded[key] = max(folded.get(key, 0), _ray_folded(r))
    same, diff = [], []
    for key in sorted(by):
        nodes = by[key]
        # 같음 · 다름은 값 **집합**으로 가른다(로그 순서가 노드마다 달라도 같은 값 집합이면 같다 — 2026-09-22 round 3 적대 리뷰)
        eq = "main" in nodes and "sub" in nodes and sorted(nodes["main"]) == sorted(nodes["sub"])
        if eq and len(nodes) == 2:
            same.append(key)
            continue
        if "main" in nodes and "sub" in nodes:
            verdict = "같음" if eq else "다름"
        elif "main" in nodes or "sub" in nodes:
            one = "main" if "main" in nodes else "sub"
            other = "sub" if one == "main" else "main"
            # Ray 가 접은 줄이 있으면 다른 노드가 같은 줄을 냈는지 로그로 가를 수 없다 — "만 관측" 으로 부재를 단언하지 않는다
            if folded.get(key):
                verdict = f"{one} 만 줄에 남음 · Ray 접힘 +{folded[key]} — {other} 사본 여부 미관측"
            elif "미특정" in nodes:
                # 노드를 가리지 못한 줄(node None)이 같은 키에 있으면 그 줄이 다른 쪽의 것일 수 있다 — "만 관측" 으로 부재를 단언하지 않는다
                verdict = f"{one} 관측 · 노드 미특정 줄 있음 — {other} 여부 미관측"
            elif key in windowed:
                # 꼬리 캡처(01 §1.4 로그 창)의 한쪽 관측은 다른 쪽 줄이 창 밖이었을 수 있다 — 부재를 단언하지 않는다(2026-09-22 통합)
                verdict = f"{one} 만 캡처 창에 남음 — {other} 여부 미관측(꼬리 캡처)"
            else:
                verdict = f"{one} 만 관측"
        else:
            verdict = "노드 미특정"
        diff.append((f"`{key}`", " · ".join(nodes.get("main", [])) or "—", " · ".join(nodes.get("sub", [])) or "—",
                     " · ".join(nodes.get("미특정", [])) or "—", verdict))
    out = ["**노드별 측정 env**(01 §1.4 표를 키 × 노드로 접은 것 · `만 관측` = 다른 쪽 줄이 캡처 로그에 없다 · Ray 가 같은 줄을 접은 "
           f"키(`{_RAY_FOLD_TEXT}`)와 꼬리 캡처 로그(01 §1.4 로그 창)에서 한쪽만 남은 키는 다른 쪽 부재를 단언하지 않는다)", ""]
    out.append(f"- 양 노드 같은 값 {len(same)}개: " + (" · ".join(f"`{k}`" for k in same) if same else "없음"))
    if diff:
        out += [""] + _table(("키", "main", "sub", "노드 미특정", "판정"), diff)
    return "\n".join(out)


def _f_tool_snapshots(c: _Ctx) -> str:
    """03 §3.4 — 측정 도구 원문 스냅샷(evidence.tool_snapshots · 2026-09-22 S2 round 3). 2차 사실 검증: bench JSON 에 없는 인자
    (temperature · ignore-eos · range-ratio)를 저작자가 "도구가 정했다 · 원문 없음" 으로 남겼는데 측정 시점 도구 원문이 저장소에 있었다
    (부류 7). 스냅샷은 draft 의 `inputs/sources/` 에 있고(배포 zip 밖) 발췌 출처 토큰은 `<이름>@<rev12>` 다 — 수신자는 같은 저장소에서
    `git show` 로 같은 바이트를 얻는다."""
    rows = [x for x in (c.facts.get("tool_snapshots") or []) if isinstance(x, dict) and x.get("name")]
    if not rows:
        return ("_측정 도구 원문 스냅샷 없음 — 측정 시각 이전 커밋의 도구 원문을 고정하지 못했다(evidence.tool_snapshots) · 도구가 정한 "
                "인자는 원문 없이 단정하지 않는다._")
    out = [f"**측정 도구 원문 {len(rows)}건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · "
           "다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)", ""]

    def rev(sha, utc) -> str:
        # 2026-09-22 round 3 적대 리뷰: 생산자(evidence._tool_revision)는 "측정 뒤 변경 없음" 과 "HEAD 가 측정 커밋의 후손이 아니라 찾지
        #   않음" 을 둘 다 None 으로 준다 — `없음` 으로 적으면 뒤쪽을 '변경 없음' 으로 단언한다. 관측하지 못했다고만 적는다.
        return (_code(str(sha)[:12]) + (f" {_cell(utc)}" if utc else "")) if sha else "관측 없음"

    def basis(x: dict) -> str | None:
        b = x.get("next_rev_basis")
        return b.strip() if isinstance(b, str) and b.strip() else None

    def nxt(x: dict) -> str:
        # 2026-09-22 S2 round 3 통합: 생산자가 next_rev None 의 뜻(`next_rev_basis` — '경로 이력에 변경 없음' 대 '미관측(후손 아님)')을
        #   내면 그 말을 그대로 싣는다(생산자만 가를 수 있다). basis 가 없는 옛 facts 만 '관측 없음' 으로 낮춘다.
        if x.get("next_rev"):
            return rev(x.get("next_rev"), x.get("next_rev_utc"))
        return _cell(basis(x)) if basis(x) else "관측 없음"

    out += _table(("도구", "역할", "저장소 경로", "리비전(커밋 UTC)", "다음 변경", "발췌 출처 토큰", "다시 얻기"),
                  [(f"`{_cell(x.get('name'))}`", _cell(x.get("role"), "—"), f"`{_cell(x.get('repo_path'))}`",
                    rev(x.get("git_rev"), x.get("git_rev_utc")), nxt(x),
                    _code(Path(str(x.get("snapshot_rel"))).name if x.get("snapshot_rel") else None),
                    _code(f"git show {x.get('git_rev')}:{x.get('repo_path')}" if x.get("git_rev") and x.get("repo_path") else None))
                   for x in rows])
    whys = [f"- `{_cell(x.get('name'))}` — {_cell(x.get('why'))}" for x in rows if x.get("why")]
    # 2026-09-22 round 3 적대 리뷰: `다시 얻기` 는 그 커밋을 가진 클론에서만 성립한다 — 이 표는 원격 도달을 판정하지 않는다(말하지 않은 것을
    #   약속하지 않는다 · 커밋이 발행 원격에 없으면 수신자의 `git show` 는 실패한다).
    whys.append("- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).")
    if any(not x.get("next_rev") and not basis(x) for x in rows):
        whys.append("- 다음 변경 `관측 없음` = 측정 시점 커밋 뒤 그 파일을 바꾼 커밋을 찾지 못했다(지금 HEAD 가 그 커밋의 후손이 아니면 찾지 "
                    "않는다) — '도구가 바뀌지 않았다' 는 뜻이 아니다.")
    notes = []
    for x in rows:
        for k in ("driver_note", "worktree_note"):
            v = x.get(k)
            if isinstance(v, str) and v and v not in notes:
                notes.append(v)
    if whys or notes:
        out += [""] + whys + [f"- {_cell(n)}" for n in notes]
    return "\n".join(out)


# 기동 시도 묶음의 원장 kind 어휘(**정확 일치** · 닫힌 tripwire — evidence `_EVENT_OPEN_KINDS` · `_EVENT_CLOSE_KINDS` ·
#   `_EVENT_WINDOWED_KINDS` 와 짝). 2026-09-22 적대 리뷰: 1판은 부분 문자열(`"declare" in kind` · `"clear"`)로 갈라, 실 원장에 있는
#   `budget_declare_rejected`(거부된 선언 — 기동이 아니다)가 새 시도를 열고 `thermal_gpu_stale_clear` 가 종료로 읽힐 수 있었고,
#   `budget_expired` 는 종료로 읽히지 않았다. 사살 · 트립은 뒤따르는 clear 에 덮이지 않게 따로 센다(02 §2.2 요약만 읽는 저작자가
#   "hang" 과 "워치독 사살" 을 가를 수 있게 — 1차 오진 부류).
_ATTEMPT_OPEN = frozenset({"budget_declare"})
_ATTEMPT_RENEW = frozenset({"budget_renew"})
_ATTEMPT_CLOSE = frozenset({"budget_clear", "budget_expired"})
_ATTEMPT_ALERT = frozenset({"watchdog_trip", "watchdog_kill_ack", "thermal_trip", "thermal_kill_ack", "earlyoom_kill",
                            "budget_declare_rejected", "budget_rejected", "budget_blocked"})
# 기동 시도로 묶지 않는 행(2026-09-22 · plan_26092119 S2 round 3 통합): evidence.event_timeline 의 `measurement` 행은 원장 사건이 아니라
#   **측정 기록**의 사실이다(node=None · 노드 축은 detail). 옛 묶음은 선언 행이 없는 이 행으로 노드 '미관측' 의 가짜 시도를 열어
#   태그2 재생의 01 §1.4 · 02 §2.2 시도 표가 "3 시도" 를 말했다(원장 선언은 2건) — 이벤트 표에만 싣고 시도 묶음에서는 뺀다.
_ATTEMPT_NOT_EVENT = frozenset({"measurement"})


def _timeline_rows(f: dict) -> list[dict]:
    return [r for r in (f.get("event_timeline") or []) if isinstance(r, dict)]


def _attempts(rows: list[dict]) -> list[dict]:
    """이벤트 행 → 기동 시도(노드마다 `budget_declare` 행이 새 시도를 연다 · 그 뒤의 같은 노드 행은 — 라벨 없는 clear · 판독 불가
    행까지 — 그 시도에 붙는다 · 선언 행이 없는 노드(엔진 로그 표지의 `cluster` 등)의 행은 그 시각에 **가장 최근에 열린 시도**에
    붙는다). 판정 ✗ — 묶기만 한다(무엇이 '벤치 진입' 인지는 저작자가 원장 의미로 해석한다). 입력 순서(evidence.event_timeline 의
    시각순 · 시각 없는 행은 뒤)를 지킨다. 라벨로 묶지 않는 이유: 원장의 clear·사살 행은 라벨이 없고 선언 창에 귀속될 뿐이다(evidence
    `_ledger_timeline`). 2026-09-22 실측(태그2 재생): 엔진 로그 표지는 node=`cluster` · 원장은 node=`main` 이라 노드로만 묶으면
    엔진 표지가 선언 없는 셋째 '시도' 로 떨어졌다. kind 는 정확 일치로만 가른다(`_ATTEMPT_*`)."""
    out: list[dict] = []
    cur: dict[str, dict] = {}
    last: dict | None = None
    for r in rows:
        key = str(r.get("node"))
        kind = str(r.get("kind") or "")
        if kind in _ATTEMPT_NOT_EVENT:
            continue
        opens = kind in _ATTEMPT_OPEN
        a = None if opens else (cur.get(key) or last)
        if a is None:
            a = {"node": r.get("node"), "label": r.get("label"), "declare": r.get("utc") if opens else None,
                 "renew": [], "alerts": {}, "end": None, "end_utc": None, "other": 0}
            out.append(a)
            cur[key] = last = a
            if opens:
                continue
        if kind in _ATTEMPT_RENEW:
            a["renew"].append(r.get("utc"))
        elif kind in _ATTEMPT_CLOSE:
            a["end"], a["end_utc"] = kind, r.get("utc")
        elif kind in _ATTEMPT_ALERT:
            a["alerts"][kind] = a["alerts"].get(kind, 0) + 1
        else:
            a["other"] += 1
    return out


def _attempts_table(rows: list[dict]) -> list[str]:
    att = _attempts(rows)
    if not att:
        return ["_기동 시도 미관측 — 이 셀 label 의 원장 행이 없다._"]
    out = _table(("시도", "노드", "label", "선언(UTC)", "갱신(renew)", "사살 · 트립 · 거부", "닫힘", "그 밖 행"),
                 [(str(i), _cell(a["node"]), f"`{_cell(a['label'])}`" if a["label"] else "—", _cell(a["declare"], "—(선언 행 없음)"),
                   (f"{len(a['renew'])}회 · 첫 {_cell(a['renew'][0])}" if a["renew"] else "없음"),
                   (" · ".join(f"`{k}` {n}" for k, n in sorted(a["alerts"].items())) if a["alerts"] else "없음"),
                   (f"`{a['end']}` {_cell(a['end_utc'])}" if a["end"] else "—(닫힘 행 없음)"), str(a["other"]))
                  for i, a in enumerate(att, 1)])
    return out


def _ledger_span_lines(f: dict) -> list[str]:
    """원장 관측 범위 줄(FACT_FIX2 G8 · evidence.event_ledger_spans 그대로 — 판정 ✗). 범위 끝이 측정 끝보다 앞인 노드는 그 뒤의 선언이
    '없음' 이 아니라 **관측 범위 밖**이라고 적는다(D2: 서브 원장 미러가 09-11 에서 끝나 09-23 서브 선언이 표에 없었다)."""
    spans = [x for x in (f.get("event_ledger_spans") or []) if isinstance(x, dict)]
    if not spans:
        return []
    parts, outs = [], []
    for x in spans:
        files = " · ".join(f"`{_cell(p)}`" for p in (x.get("files") or []))
        if not x.get("last_utc"):
            parts.append(f"{_cell(x.get('node'))} = 원장 없음(이 노드의 사건은 관측 대상 밖)")
            outs.append(str(x.get("node")))
            continue
        parts.append(f"{_cell(x.get('node'))} = {_cell(x.get('first_utc'))} → {_cell(x.get('last_utc'))}({files})")
        if x.get("covers_measurement") is False:
            outs.append(f"{x.get('node')}(마지막 행 {x.get('last_utc')} < 측정 끝 {x.get('measurement_end_utc')})")
    line = "> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): " + " · ".join(parts)
    if outs:
        line += (f" — ⚠ {' · '.join(_cell(o) for o in outs)}: 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · "
                 "원장 미회수)")
    return ["", line]


def _f_event_timeline(c: _Ctx) -> str:
    """01 §1.4 — 블랙박스 이벤트 원장(`docs/logs/<node>/events/*.jsonl`)에서 이 셀의 행(evidence.event_timeline) + 기동 시도 묶음.
    2026-09-22 S2 round 2(F9): 1차 저작자가 "두 번 띄웠는지 미기록" 이라 적었는데 같은 원장에 예산 선언 2건이 있었다(fact-check #1)."""
    rows = _timeline_rows(c.facts)
    if not rows:
        # 2026-09-22 · plan_26092119 S2 round 2 통합 정정: 옛 문구는 "(원장 부재와 행 0 을 evidence 가 가른다)" 였다 — evidence.event_timeline
        #   은 둘 다 `[]` 를 돌려준다(가르지 않는다). 사실 블록이 생산자가 하지 않는 구분을 했다고 말하면 그것이 오도 바이트다.
        return "\n".join(["_블랙박스 이벤트 미관측 — 이 셀 label·config 에 맞는 원장 행이 없다(원장 파일이 없는 것과 파일은 있으나 이 셀 행이 0 인 것을 "
                          "이 표는 구분하지 않는다 — evidence.event_timeline 이 둘 다 빈 목록으로 준다)._"] + _ledger_span_lines(c.facts))
    out = [f"**블랙박스 이벤트 {len(rows)}행**(원장에서 이 셀 label·config 에 맞는 행만 · 시각순 · 기계 발췌)"] + _ledger_span_lines(c.facts) + [""]
    out += _table(("#", "UTC", "노드", "kind", "label", "내용", "출처"),
                  [(str(i), _cell(r.get("utc")), _cell(r.get("node")), f"`{_cell(r.get('kind'))}`",
                    f"`{_cell(r.get('label'))}`" if r.get("label") else "—", _cell(r.get("detail"), "—"), _cell(r.get("source")))
                   for i, r in enumerate(rows, 1)])
    out += ["", "**기동 시도**(예산 선언 `budget_declare` 마다 한 시도 · 같은 노드의 이어지는 행과 선언 없는 노드(엔진 로그)의 그 시각 행을 "
                "묶었다 · 사살 · 트립 · 거부는 닫힘과 따로 센다 · `measurement` 행은 측정 기록이라 시도로 세지 않는다(위 표에만) — "
                "해석은 저작자 몫)", ""]
    out += _attempts_table(rows)
    return "\n".join(out)


def _f_event_attempts(c: _Ctx) -> str:
    """02 §2.2 — 기동 시도 묶음만(01 §1.4 표의 요약 · 행 전문은 01). 서사에 '한 번 · 두 번 띄웠다' 를 적기 전에 대조한다."""
    rows = _timeline_rows(c.facts)
    if not rows:
        return "_기동 시도 미관측 — 블랙박스 원장에 이 셀 label 의 행이 없다(01 §1.4)._"
    return "\n".join(["**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)"] + _ledger_span_lines(c.facts) + [""]
                     + _attempts_table(rows))


def _f_bench_definition(c: _Ctx) -> str:
    """03 §3.5 — 현행 full 정의(docs.md 원문 그대로)와 이 측정의 반복 수 · 충족 여부(evidence.bench_definition · 판정은 그쪽).
    2026-09-22 S2 round 2(F9): 1차 저작자는 계보 문서만으로 "현행 정의" 를 답할 수 없어 '미기록' 으로 돌려 말했다(journey #43)."""
    bd = _dict(c.facts, "bench_definition")
    if not bd:
        return "_현행 full 정의 미관측 — evidence.bench_definition 결과가 없다._"
    sent = str(bd.get("current_full_definition") or "").strip()
    out = [f"**현행 full 정의**(`{_bench_definition_source(bd)}` 원문 그대로):", ""]
    out += [f"> {ln}" for ln in (sent.splitlines() or [_MISSING_TEXT])]
    rows = [("이 측정의 반복 수", _cell(bd.get("this_repeats")), _cell(bd.get("repeats_source")))]
    if "required_repeats" in bd:
        rows.append(("현행 정의의 반복 요건", _cell(bd.get("required_repeats")), "위 정의 문장"))
    if "bench_tool" in bd:
        rows.append(("이 측정의 도구", _cell(bd.get("bench_tool")), _cell(bd.get("bench_tool_source"))))
    reasons = [x for x in (bd.get("reasons") or []) if isinstance(x, str) and x]
    rows.append(("현행 full 정의 충족", _yes_no(bd.get("meets_current_full")),
                 " · ".join(reasons) if reasons else "evidence.bench_definition(반복 수 × 현행 정의)"))
    out += [""] + _table(("항목", "값", "출처 · 사유"), rows)
    return "\n".join(out)


def _f_lineage(c: _Ctx) -> str:
    items = [x for x in (c.facts.get("lineage_reading_list") or []) if isinstance(x, dict)]
    if not items:
        return "_계보 문서 0건 — 서사 원재료가 없다(린터가 `HINT_LINEAGE_EMPTY` 로 막는다)._"
    rows = []
    for i, x in enumerate(items, 1):
        doc = f"`{_cell(x.get('path'))}`" + (" (모호 해소)" if x.get("tier") == "ambiguous" else "")
        rows.append((str(i), doc, _cell(x.get("kind")), _cell(x.get("date_key")), _cell(x.get("depth")),
                     _cell(x.get("bytes")), _cell(x.get("axis_tokens"), "—"), _cell(x.get("superseded_by"), "—")))
    out = [f"계보 문서 {len(items)}건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. "
           "크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 "
           "`evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).", ""]
    out += _table(("#", "문서", "종류", "날짜", "깊이", "크기", "셀 축 토큰", "대체됨"), rows)
    return "\n".join(out)


_LEVEL_KEY_ORDER = ("level", "status")


def _f_measurement(c: _Ctx) -> str:
    """측정 수치(파싱만 · 재계산 ✗). 스칼라는 키-값 표 · 한 단계 dict(예: `lite`)는 `키.하위키` 행 · dict 목록(예: 레벨별
    `levels`)은 별도 표 — JSON 덩어리를 표 칸 하나에 밀어 넣으면 사람도 Agent 도 수치를 못 읽는다(2026-09-22 리뷰)."""
    m = _dict(c.facts, "measurement")
    if not m:
        out = ["_측정값 없음(미기재 — 0 이 아니다)._"]
    else:
        # 2026-09-29 plan_26092908 §4.4: 키별 출처(evidence `measurement.sources`)는 `sources.<키>` 행으로 펼치지 않고 그 키 행의 출처 칸에
        #   싣는다(값과 출처가 한 행 — 수신자가 판정 · 수치가 인증서인지 판정 원천인지 같은 자리에서 본다).
        per = _dict(m, "sources")
        keys = (["measured_utc"] if "measured_utc" in m else []) + sorted(k for k in m if k not in ("measured_utc", "source",
                                                                                                    "sources"))
        keys += ["source"] if "source" in m else []
        rows, subtables = [], []
        for k in keys:
            v = m[k]
            if isinstance(v, dict) and v and all(not isinstance(x, (dict, list)) for x in v.values()):
                rows += [(f"`{k}.{sk}`", _cell(v[sk], _UNRECORDED), _cell(per.get(k), "—")) for sk in sorted(v)]
            elif isinstance(v, list) and v and all(isinstance(x, dict) for x in v):
                cols = [x for x in _LEVEL_KEY_ORDER if any(x in r for r in v)]
                cols += sorted({x for r in v for x in r} - set(cols))
                subtables += ["", f"**`{k}`**({len(v)}행 · 출처 {_cell(per.get(k), '—')})", ""]
                subtables += _table(tuple(f"`{x}`" for x in cols), [tuple(_cell(r.get(x), _UNRECORDED) for x in cols) for r in v])
            else:
                rows.append((f"`{k}`", _cell(v, _UNRECORDED), _cell(per.get(k), "—")))
        out = _table(("키", "값", "출처"), rows) + subtables
    out += _marker_lines(c.facts)
    return "\n".join(out)


def _bench_module(repo: Path):
    return core.load_owner_module(repo, core.REL_RENDER_BENCH_SECTION, "hint_render_bench_section", add_dir_to_path=False)


def _f_measurement_config(c: _Ctx) -> str:
    mc = _dict(c.facts, "measurement_config")
    if not mc:
        return "_측정 구성 미기재 — 바인딩된 리포트에 측정 구성 표가 없다(부재는 미기재다 · lite 로 접지 않는다)._"
    order = tuple(getattr(_bench_module(c.repo), "MEASUREMENT_CONFIG_KEYS", ()))
    keys = [k for k in order if k in mc] + sorted(k for k in mc if k not in order)
    return "\n".join(_table(("키", "값"), [(f"`{k}`", _cell(mc[k], _UNRECORDED)) for k in keys]))


def _f_bench_section(c: _Ctx) -> str:
    md = c.facts.get("bench_section_md")
    if isinstance(md, str) and md.strip():
        return md.strip("\n")
    return ("_부하 곡선 없음 — 바인딩된 리포트 · 스윕 색인이 없다. 이 레시피의 부하 거동은 이 hint 로 알 수 없다"
            "(부재를 성능 판정으로 읽지 말 것)._")


FACT_RENDERERS: dict[str, Callable[[_Ctx], str]] = {
    "header": _f_header, "grade": _f_grade, "context": _f_context, "resolved": _f_resolved, "meta": _f_meta,
    "missing": _f_missing, "legend": _f_legend, "slots": _f_slots, "applied_set": _f_applied_set,
    "value_status": _f_value_status, "reproduce": _f_reproduce, "sub_recipe": _f_sub_recipe, "lineage": _f_lineage,
    "measurement": _f_measurement, "measurement_config": _f_measurement_config, BENCH_REGION_ID: _f_bench_section,
    "bench_missing": _f_bench_missing, "event_timeline": _f_event_timeline, "event_attempts": _f_event_attempts,
    "bench_definition": _f_bench_definition,
    # 2026-09-22 · plan_26092119 S2 round 3(공유 사실 계약): 측정 env 관측 · 노드별 접기 · 측정 도구 원문 스냅샷
    "measurement_env": _f_measurement_env, "measurement_env_nodes": _f_measurement_env_nodes, "tool_snapshots": _f_tool_snapshots,
    # 2026-09-29 · plan_26092908 §4.1: 0.9 이름 꼬리(publish = 후보 · continue 확정 뒤 = 확정 꼬리)
    "name_tail": _f_name_tail,
}


# ── 문서 모델 ─────────────────────────────────────────────────────────────────────────────────
@dataclass
class _Prompt:
    start: int
    end: int
    raw: list[str]
    fields: dict[str, str]
    chapter: str


@dataclass
class _Fact:
    """사실 블록. 보통은 start=여는 표지 줄 · end=닫는 표지 줄. open_ended(03 부하 곡선 · `<!-- BENCH_SECTION -->`)는
    start=표지 줄 · end=다음 챕터 헤딩 줄(영역 밖 · 없으면 문서 끝) — 어느 쪽이든 본문은 lines[start+1:end] 다."""
    fid: str
    start: int
    end: int
    body: str
    chapter: str
    open_ended: bool = False


@dataclass
class _Doc:
    """마크다운 한 벌의 줄 단위 모델. masked = 주석(PROMPT 포함)·사실 블록(기계 영역) — 저작 영역이 아니다."""
    name: str
    text: str
    lines: list[str] = field(default_factory=list)
    masked: list[bool] = field(default_factory=list)
    comment: list[bool] = field(default_factory=list)
    fence: list[bool] = field(default_factory=list)
    chapter_of: list[str] = field(default_factory=list)
    facts: list[_Fact] = field(default_factory=list)
    prompts: list[_Prompt] = field(default_factory=list)
    chapters: list[tuple[int, str, str]] = field(default_factory=list)
    other_h2: list[tuple[int, str]] = field(default_factory=list)
    structure: list[tuple[str, int, str]] = field(default_factory=list)
    selfchecks: list[tuple[int, int]] = field(default_factory=list)

    @classmethod
    def parse(cls, name: str, text: str) -> "_Doc":
        d = cls(name, text)
        d.lines = text.split("\n")
        n = len(d.lines)
        d.masked, d.comment, d.fence, d.chapter_of = [False] * n, [False] * n, [False] * n, ["preamble"] * n
        L, i, chapter, in_fence = d.lines, 0, "preamble", False
        while i < n:
            line = L[i]
            if not in_fence:
                m = _FACT_OPEN.match(line)
                if m:
                    j = i + 1
                    while j < n and not _FACT_CLOSE.match(L[j]) and not _FACT_OPEN.match(L[j]):
                        j += 1
                    close = _FACT_CLOSE.match(L[j]) if j < n else None
                    if close and close.group(1) == m.group(1):
                        for k in range(i, j + 1):
                            d.masked[k], d.chapter_of[k] = True, chapter
                        d.facts.append(_Fact(m.group(1), i, j, "\n".join(L[i + 1:j]), chapter))
                        i = j + 1
                        continue
                    d.structure.append(("HINT_FACT_BLOCK_UNCLOSED", i, f"FACT:{m.group(1)} 의 닫힘 표지가 없다"))
                    d.masked[i], d.chapter_of[i] = True, chapter
                    i += 1
                    continue
                if _FACT_CLOSE.match(line):
                    d.structure.append(("HINT_FACT_BLOCK_STRAY", i, "여는 표지 없는 FACT 닫힘 표지"))
                    d.masked[i], d.chapter_of[i] = True, chapter
                    i += 1
                    continue
                stripped = line.lstrip()
                if stripped.startswith("<!--"):
                    j = i
                    if "-->" not in stripped[4:]:
                        j = i + 1
                        while j < n and "-->" not in L[j]:
                            j += 1
                        if j >= n:
                            d.structure.append(("HINT_COMMENT_UNCLOSED", i, "HTML 주석이 닫히지 않았다"))
                            j = n - 1
                    for k in range(i, j + 1):
                        d.masked[k], d.comment[k], d.chapter_of[k] = True, True, chapter
                    if _PROMPT_OPEN.match(line) and j > i:
                        d.prompts.append(_Prompt(i, j, L[i:j + 1], _prompt_fields(L[i + 1:j]), chapter))
                    if _SELFCHECK_OPEN.match(line) and j > i:
                        d.selfchecks.append((i, j))
                    if j == i and _BENCH_MARK.match(line):
                        # 닫힘 표지 없는 기계 영역: 다음 **챕터** 헤딩(`## n.m …`) 직전까지. 영역 안의 `## ` 절 제목
                        # (render_bench_section 의 SECTION_TITLE)은 모르는 헤딩이 아니라 기계 본문이다.
                        k = i + 1
                        while k < n:
                            hm = _H2.match(L[k])
                            if hm and _CHAPTER_RE.match(hm.group(1)):
                                break
                            k += 1
                        for x in range(i + 1, k):
                            d.masked[x], d.chapter_of[x] = True, chapter
                        d.facts.append(_Fact(BENCH_REGION_ID, i, k, "\n".join(L[i + 1:k]), chapter, open_ended=True))
                        i = k
                        continue
                    i = j + 1
                    continue
                h = _H2.match(line)
                if h:
                    cm = _CHAPTER_RE.match(h.group(1))
                    if cm:
                        chapter = cm.group(1)
                        d.chapters.append((i, cm.group(1), cm.group(2).strip()))
                    else:
                        d.other_h2.append((i, h.group(1)))
            if _FENCE.match(line):
                d.fence[i] = True
                in_fence = not in_fence
            elif in_fence:
                d.fence[i] = True
            d.chapter_of[i] = chapter
            i += 1
        return d

    def visible(self) -> str:
        """주석(PROMPT 포함)을 뺀 본문 — 사실 블록 본문은 배포되는 글이므로 포함한다(마커 · 배너 검사의 대상)."""
        return "\n".join(line for i, line in enumerate(self.lines) if not self.comment[i])

    def fact(self, fid: str) -> list[_Fact]:
        return [f for f in self.facts if f.fid == fid]

    def chapter_lines(self, chapter: str) -> list[int]:
        return [i for i, c in enumerate(self.chapter_of) if c == chapter]


def _prompt_fields(body: list[str]) -> dict[str, str]:
    """PROMPT 본문 → {키: 값}. 들여쓴 줄 · 코드펜스 안 줄은 앞 필드의 계속이다(형식 예시가 필드로 오인되지 않게)."""
    fields: dict[str, list[str]] = {}
    cur, in_fence = None, False
    for raw in body:
        if _FENCE.match(raw):
            in_fence = not in_fence
            if cur:
                fields[cur].append(raw)
            continue
        m = None if in_fence else _PROMPT_FIELD.match(raw)
        if m and m.group(1) in PROMPT_KEYS:
            cur = m.group(1)
            if cur in fields:
                fields.setdefault("__dup__", []).append(cur)
            fields[cur] = [m.group(2)]
        elif cur is not None:
            fields[cur].append(raw)
        elif raw.strip():
            fields.setdefault("__stray__", []).append(raw)
    return {k: "\n".join(v).strip() for k, v in fields.items()}


def _question(fields: dict) -> str:
    return _norm(fields.get("질문", ""))


def _directives(fields: dict) -> dict:
    """기계 지시 해석. 템플릿이 틀리면 fail-loud(템플릿 결함은 모든 발행을 그르친다)."""
    blocks: dict[str, int] = {}
    for item in re.split(r"[,·]", fields.get("블록", "")):
        item = item.strip()
        if not item:
            continue
        m = re.fullmatch(r"([a-z-]+)\s*(?:>=\s*(\d+))?", item)
        if not m or m.group(1) not in EVENT_KINDS:
            core.fail("HINT_TEMPLATE_DIRECTIVE_INVALID", f"`블록:` 항목을 해석할 수 없다: {item!r}",
                      f"형식 `<kind>[>=N]` · kind ∈ {', '.join(EVENT_KINDS)}")
        blocks[m.group(1)] = int(m.group(2) or 0)
    raw_ex = fields.get("필수 발췌", "").strip()
    if raw_ex and not raw_ex.isdigit():
        core.fail("HINT_TEMPLATE_DIRECTIVE_INVALID", f"`필수 발췌:` 는 정수다: {raw_ex!r}")
    cond = None
    raw_cond = fields.get("조건", "").strip()
    if raw_cond:
        m = re.fullmatch(r"([a-z_]+)\s*=\s*(\S+)", raw_cond)
        if not m or m.group(1) not in _CONDITION_KEYS:
            core.fail("HINT_TEMPLATE_DIRECTIVE_INVALID", f"`조건:` 을 해석할 수 없다: {raw_cond!r}",
                      f"형식 `<key>=<value>` · key ∈ {', '.join(_CONDITION_KEYS)}")
        cond = (m.group(1), m.group(2))
    return {"blocks": blocks, "excerpts": int(raw_ex or 0), "condition": cond,
            "optional": fields.get("선택", "").strip() in ("예", "yes", "true")}


def _condition_holds(cond, facts: dict) -> tuple[bool, str]:
    if cond is None:
        return True, ""
    key, want = cond
    actual = {"topology": _topology(facts), "task_class": _task_class(facts), "plane": facts.get("plane")}[key]
    if actual in (None, ""):
        core.fail("HINT_TEMPLATE_CONDITION_UNDECIDABLE", f"조건 `{key}={want}` 를 판정할 사실이 없다(facts 의 {key} 부재).",
                  "evidence/artifacts 가 그 사실을 관측해 facts 에 넣어야 한다 — 추측으로 채우지 않는다.")
    return actual == want, str(actual)


def _na_line(cond, actual: str) -> str:
    return f"> 해당 없음 — 이 절의 조건 `{cond[0]}={cond[1]}` 이 이 셀에서 성립하지 않는다(`{cond[0]}={actual}`)."


# ── 템플릿 적재 ───────────────────────────────────────────────────────────────────────────────
def _templates_dir(repo: Path) -> Path:
    return Path(repo) / core.REL_SKILL / "templates"


def _load_templates(repo: Path) -> dict:
    """저장소의 템플릿 4종을 읽고 모양을 검사한다(트립와이어). 결함 = fail-loud — 조용히 좁아진 템플릿은 모든 발행을
    그르친다."""
    out = {}
    tdir = _templates_dir(repo)
    for name in TEMPLATE_FILES:
        p = tdir / f"{name}.prompt.md"
        if not p.is_file():
            core.fail("HINT_TEMPLATE_ABSENT", f"prompt 템플릿 부재: {core.REL_SKILL}/templates/{p.name}")
        doc = _Doc.parse(name, p.read_text(encoding="utf-8"))
        if doc.structure:
            code, i, msg = doc.structure[0]
            core.fail("HINT_TEMPLATE_SHAPE", f"{p.name}:{i + 1}: {code} {msg}")
        nums = tuple(num for _, num, _ in doc.chapters)
        if nums != CHAPTERS[name]:
            core.fail("HINT_TEMPLATE_SHAPE", f"{p.name} 챕터 {nums} ≠ plan §4.2 {CHAPTERS[name]}",
                      "챕터 목록은 plan §4.2 가 정본이다 — 템플릿과 CHAPTERS 를 함께 리뷰한다.")
        ids = [f.fid for f in doc.facts]
        for fid in ids:
            if fid not in FACT_RENDERERS and fid not in DERIVED_FACTS:
                core.fail("HINT_TEMPLATE_SHAPE", f"{p.name}: 렌더러 없는 FACT:{fid}")
            if ids.count(fid) > 1:
                core.fail("HINT_TEMPLATE_SHAPE", f"{p.name}: FACT:{fid} 가 두 번 있다")
            if [f for f in doc.facts if f.fid == fid][0].body.strip():
                core.fail("HINT_TEMPLATE_SHAPE", f"{p.name}: 템플릿의 FACT:{fid} 는 비어 있어야 한다")
        for pr in doc.prompts:
            if "__dup__" in pr.fields or "__stray__" in pr.fields:
                core.fail("HINT_TEMPLATE_SHAPE", f"{p.name}:{pr.start + 1}: PROMPT 필드 중복 또는 키 없는 줄")
            d = _directives(pr.fields)
            need = ("질문", "기재") if d["optional"] else _PROMPT_REQUIRED_KEYS
            miss = [k for k in need if not pr.fields.get(k)]
            if miss:
                core.fail("HINT_TEMPLATE_SHAPE", f"{p.name}:{pr.start + 1}: PROMPT 필수 키 부재 {miss}")
        out[name] = doc
    return out


# ── hint-event 제한 YAML ─────────────────────────────────────────────────────────────────────
def _strip_comment(s: str) -> str:
    """`#` 주석 제거 — 따옴표 밖이고 앞이 공백(또는 줄 처음)일 때만(`패치#60` 은 주석이 아니다)."""
    q = None
    for i, ch in enumerate(s):
        if q:
            if ch == q:
                q = None
        elif ch in "\"'" and (i == 0 or s[i - 1] in " \t[,"):
            q = ch
        elif ch == "#" and (i == 0 or s[i - 1] in " \t"):
            return s[:i]
    return s


def _unquote(item: str) -> tuple[str | None, str | None]:
    item = item.strip()
    if item and item[0] in "\"'":
        q = item[0]
        end = item.find(q, 1)
        if end == -1:
            return None, "따옴표가 닫히지 않았다"
        if item[end + 1:].strip():
            return None, "닫는 따옴표 뒤에 글자가 있다(값 전체를 다른 따옴표로 감싼다)"
        return item[1:end], None
    return item, None


def _parse_value(raw: str):
    """값 한 줄 → str | list[str]. (값, 오류) 반환."""
    s = _strip_comment(raw).strip()
    if not s:
        return "", None
    if s[0] in "\"'":
        return _unquote(s)
    if s[0] == "[":
        if not s.endswith("]"):
            return None, "인라인 리스트가 `]` 로 닫히지 않았다"
        items, cur, q = [], [], None
        for ch in s[1:-1]:
            if q:
                cur.append(ch)
                if ch == q:
                    q = None
                continue
            if ch in "\"'" and not "".join(cur).strip():
                q = ch
                cur.append(ch)
                continue
            if ch == ",":
                items.append("".join(cur))
                cur = []
                continue
            cur.append(ch)
        if q:
            return None, "리스트 항목의 따옴표가 닫히지 않았다"
        items.append("".join(cur))
        out = []
        for it in items:
            it = it.strip()
            if not it:
                continue
            if it[0] in "[{":
                return None, "중첩 리스트·매핑은 허용하지 않는다"
            v, err = _unquote(it)
            if err:
                return None, err
            out.append(v)
        return out, None
    if s[0] in "{|>&*!":
        return None, f"`{s[0]}` 로 시작하는 값은 제한 YAML 밖이다(매핑·블록 스칼라·앵커 ✗) — 값 전체를 따옴표로 감싼다"
    return s, None


def _parse_event_body(body: list[str]) -> tuple[dict, list[tuple[str, int, str]]]:
    """제한 YAML: 줄 단위 `키: 값` · 값은 한 줄 · 따옴표 선택 · 리스트는 `[a, b]` 인라인만 · `#` 이후 주석.
    반환 (사건, [(code, 본문 내 줄 오프셋, 메시지)])."""
    ev: dict = {}
    errs: list[tuple[str, int, str]] = []
    for off, raw in enumerate(body):
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        if raw[:1].isspace():
            errs.append(("HINT_EVENT_INVALID", off, "들여쓴 줄(중첩)은 제한 YAML 밖이다 — 값은 한 줄"))
            continue
        m = _EVENT_KEY.match(raw)
        if not m:
            errs.append(("HINT_EVENT_INVALID", off, f"`키: 값` 줄이 아니다: {raw.strip()[:60]!r}"))
            continue
        key = m.group(1)
        if key in ev:
            errs.append(("HINT_EVENT_INVALID", off, f"중복 키: {key}"))
            continue
        val, err = _parse_value(m.group(2))
        if err:
            errs.append(("HINT_EVENT_INVALID", off, f"{key}: {err}"))
            continue
        ev[key] = val
    return ev, errs


def _validate_event(ev: dict) -> list[tuple[str, str]]:
    errs: list[tuple[str, str]] = []
    kind = ev.get("kind")
    if kind not in EVENT_KINDS:
        return [("HINT_EVENT_KIND_UNKNOWN", f"kind 어휘 밖: {kind!r} (∈ {', '.join(EVENT_KINDS)})")]
    for key in EVENT_REQUIRED[kind]:
        v = ev.get(key)
        if v is None or (isinstance(v, str) and not v.strip()) or (isinstance(v, list) and not v):
            errs.append(("HINT_EVENT_FIELD_MISSING", f"{kind} 필수 필드 부재: {key}"))
    for key in ("id", "kind", "서명", "노브", "값", "지위", "전이등급", "현재지위"):
        if isinstance(ev.get(key), list):
            errs.append(("HINT_EVENT_INVALID", f"{key} 는 리스트가 아니라 한 값이다"))
    eid = ev.get("id")
    if isinstance(eid, str) and eid and not re.fullmatch(EVENT_ID_PREFIX[kind] + r"\d+", eid):
        errs.append(("HINT_EVENT_ID_SHAPE", f"{kind} 의 id 는 `{EVENT_ID_PREFIX[kind]}<n>` 이다: {eid!r}"))
    if kind in ("wall", "misdiagnosis") and ev.get("전이등급") and ev.get("전이등급") not in TRANSFER_CLASSES:
        errs.append(("HINT_EVENT_VOCAB", f"전이등급 어휘 밖: {ev.get('전이등급')!r}"))
    if kind == "value-status" and ev.get("지위") and ev.get("지위") not in VALUE_STATUSES:
        errs.append(("HINT_EVENT_VOCAB", f"지위 어휘 밖: {ev.get('지위')!r}"))
    if ev.get("현재지위") and ev.get("현재지위") not in CURRENT_STATUSES:
        errs.append(("HINT_EVENT_VOCAB", f"현재지위 어휘 밖: {ev.get('현재지위')!r}"))
    return errs


def _sources_of(ev: dict) -> list[str]:
    raw = ev.get("출처")
    items = raw if isinstance(raw, list) else ([raw] if isinstance(raw, str) and raw.strip() else [])
    return [str(x).strip() for x in items if str(x).strip()]


def _source_token(entry: str) -> str:
    head = entry.split("§", 1)[0].strip().strip("`")
    return head.split()[0] if head.split() else ""


# 트리플렛 슬롯의 페이로드 경로(artifacts `_copy(…, "triplet")` 와 짝 · 닫힌 tripwire) — 값을 선언한 파일 자신이다.
_TRIPLET_ARTIFACT_PREFIX = "artifacts/triplet/"


def _is_requirement_artifact(loc) -> bool:
    """declared-requirement 의 근거가 될 수 있는 출처인가 — 페이로드 artifacts 파일이되 트리플렛(선언 자신)이 아닌 것."""
    return bool(loc) and loc[0] == "artifact" and not str(loc[1]).startswith(_TRIPLET_ARTIFACT_PREFIX)


@dataclass
class _RawEvent:
    line: int
    chapter: str
    event: dict | None
    errors: list[tuple[str, int, str]]


@dataclass
class _Excerpt:
    line: int
    chapter: str
    stem: str
    section: str
    body: list[str]


def _scan(doc: _Doc) -> tuple[list[_RawEvent], list[_Excerpt], list[int]]:
    """저작 영역(주석·사실 블록 밖)의 hint-event 펜스와 원문 발췌를 모은다. 반환 (사건, 발췌, 모양이 틀린 발췌 머리 줄)."""
    L, n = doc.lines, len(doc.lines)
    events: list[_RawEvent] = []
    excerpts: list[_Excerpt] = []
    malformed: list[int] = []
    i = 0
    while i < n:
        if doc.masked[i]:
            i += 1
            continue
        line = L[i]
        if _FENCE.match(line):
            is_event = _EVENT_FENCE.match(line) is not None
            j = i + 1
            while j < n and not _FENCE_CLOSE.match(L[j]):
                j += 1
            if is_event:
                body = L[i + 1:j]
                ev, errs = _parse_event_body(body)
                errs = [(c, i + 2 + off, m) for c, off, m in errs]
                if j >= n:
                    errs.append(("HINT_EVENT_INVALID", i + 1, "hint-event 펜스가 닫히지 않았다"))
                if not errs:
                    errs = [(c, i + 1, m) for c, m in _validate_event(ev)]
                events.append(_RawEvent(i + 1, doc.chapter_of[i], ev, errs))
            i = j + 1
            continue
        if _EXCERPT_ANY.match(line):
            head = _EXCERPT_HEAD.match(line)
            j, body = i + 1, []
            while j < n and not doc.masked[j] and L[j].startswith(">") and not _EXCERPT_ANY.match(L[j]):
                rest = L[j][1:]
                body.append(rest[1:] if rest.startswith(" ") else rest)
                j += 1
            if head:
                excerpts.append(_Excerpt(i + 1, doc.chapter_of[i], head.group("stem"), head.group("section"), body))
            else:
                malformed.append(i + 1)
            i = j
            continue
        i += 1
    return events, excerpts, malformed


def parse_hint_events(md: str) -> list[dict]:
    """마크다운의 ` ```hint-event ` 블록 → 사건 목록(엄격). 주석(PROMPT 형식 예시)·사실 블록 안은 사건이 아니다.

    제한 YAML(SPEC §5.7): 줄 단위 `키: 값`(값 한 줄 · 따옴표 선택) · 리스트는 `[a, b]` 인라인만 · `#` 이후 주석.
    kind ∈ wall|rejected|misdiagnosis|value-status|open-question · kind 별 필수 필드 · 전이등급/지위/현재지위 어휘 ·
    id 모양(`W<n>` 등). 첫 오류에서 HintError(code) — 수신 Agent 의 파서와 발행 린터가 같은 규칙을 쓴다.
    반환 사건마다 `_line`(펜스 시작 줄 · 1-기반)과 `출처` 는 언제나 리스트다."""
    doc = _Doc.parse("<md>", md or "")
    events, _, _ = _scan(doc)
    out = []
    for raw in events:
        if raw.errors:
            code, line, msg = raw.errors[0]
            core.fail(code, f"hint-event(줄 {line}): {msg}")
        ev = dict(raw.event)
        ev["출처"] = _sources_of(ev)
        ev["_line"] = raw.line
        out.append(ev)
    return out


def excerpts(md: str) -> list[dict]:
    """원문 발췌 목록: `> [원문] <stem> §<절>` 머리와 이어지는 `> ` 줄들. {stem, section, body, line, line_count, chapter}."""
    doc = _Doc.parse("<md>", md or "")
    _, exs, _ = _scan(doc)
    return [{"stem": e.stem, "section": e.section, "body": "\n".join(e.body), "line": e.line,
             "line_count": len(e.body), "chapter": e.chapter} for e in exs]


def fact_blocks(md: str) -> dict[str, str]:
    """`<!-- FACT:<id> -->` … `<!-- /FACT:<id> -->` → {id: 본문}. 같은 id 가 둘이면 첫 것.
    03 의 닫힘 표지 없는 부하 곡선 영역(`<!-- BENCH_SECTION -->` ~ 다음 챕터 헤딩)은 id `bench_section` 이며 앞뒤 빈 줄을 뗀다."""
    doc = _Doc.parse("<md>", md or "")
    out: dict[str, str] = {}
    for f in doc.facts:
        out.setdefault(f.fid, f.body.strip("\n") if f.open_ended else f.body)
    return out


# ── 파생 블록 (00 §0.3 · §0.5) ────────────────────────────────────────────────────────────────
def _valid_events(md: str) -> list[dict]:
    doc = _Doc.parse("<md>", md or "")
    events, _, _ = _scan(doc)
    return [r.event for r in events if not r.errors]


def _derive_from_texts(narrative: str, artifacts_md: str) -> dict[str, str]:
    evs = _valid_events(narrative)
    walls = [e for e in evs if e.get("kind") == "wall"]
    miss = [e for e in evs if e.get("kind") == "misdiagnosis"]
    if walls:
        wall = _table(("순서", "id", "증상", "해소", "전이등급", "검증"),
                      [(str(i), _cell(e.get("id")), _cell(e.get("증상")), _cell(e.get("해소")), _cell(e.get("전이등급")),
                        _cell(e.get("검증"))) for i, e in enumerate(walls, 1)])
    else:
        wall = ["_02-narrative.md §2.2 에 `kind: wall` 블록이 아직 없다 — 저작 뒤 `refresh` 가 채운다(린터: wall ≥ 1)._"]
    if miss:
        wall += ["", "**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)", ""]
        wall += _table(("id", "증상", "현재지위", "검증"),
                       [(_cell(e.get("id")), _cell(e.get("증상")), _cell(e.get("현재지위"), "—"), _cell(e.get("검증")))
                        for e in miss])
    vs = [e for e in _valid_events(artifacts_md) if e.get("kind") == "value-status"]
    if vs:
        # 2026-09-22 S2 round 2(F9): 지위별 개수 — "tuned 가 아닌 것 N개" 한 줄은 필요조건(declared-requirement)까지 "조정 안 됨" 무리에
        #   섞어 읽혔다(0.5 배너 결함과 같은 뿌리).
        counts = {s: sum(1 for e in vs if e.get("지위") == s) for s in VALUE_STATUSES}
        knob = [f"노브 {len(vs)}개 — " + " · ".join(f"`{s}` {n}" for s, n in counts.items() if n) + ".", ""]
        knob += _table(("노브", "값", "지위", "근거", "id"),
                       [(f"`{_cell(e.get('노브'))}`", _cell(e.get("값")), _cell(e.get("지위")), _cell(e.get("근거")),
                         _cell(e.get("id"))) for e in vs])
    else:
        knob = ["_01-artifacts.md §1.3 에 `kind: value-status` 블록이 아직 없다 — 저작 뒤 `refresh` 가 채운다._"]
    return {"wall_map": "\n".join(wall), "knob_status": "\n".join(knob)}


def derive_blocks(payload_dir: Path) -> dict[str, str]:
    """파생 블록 본문(순수 함수 · 쓰기 없음): wall_map ← 02 의 wall·misdiagnosis 블록 · knob_status ← 01 의 value-status 블록."""
    p = Path(payload_dir)
    read = lambda name: (p / name).read_text(encoding="utf-8") if (p / name).is_file() else ""
    return _derive_from_texts(read("02-narrative.md"), read("01-artifacts.md"))


def _replace_fact_bodies(text: str, bodies: dict[str, str]) -> str:
    doc = _Doc.parse("<md>", text)
    lines = list(doc.lines)
    for f in sorted(doc.facts, key=lambda x: x.start, reverse=True):
        if f.fid in bodies:
            new = bodies[f.fid].split("\n") if bodies[f.fid] != "" else []
            lines[f.start + 1:f.end] = new
    return "\n".join(lines)


def refresh(payload_dir: Path) -> list[str]:
    """00 의 파생 블록을 02/01 의 현재 hint-event 로 다시 채운다(쓰기). 바뀐 블록 id 목록을 돌려준다.
    lint 전에 부른다 — lint 는 부수효과가 없으므로 낡은 파생 블록을 고치지 않고 `HINT_DERIVED_BLOCK_STALE` 로 막는다."""
    p = Path(payload_dir) / "00-hint.md"
    if not p.is_file():
        core.fail("HINT_DOCUMENT_ABSENT", f"00-hint.md 부재: {p}")
    text = p.read_text(encoding="utf-8")
    have = fact_blocks(text)
    want = derive_blocks(payload_dir)
    absent = [k for k in DERIVED_FACTS if k not in have]
    if absent:
        core.fail("HINT_FACT_BLOCK_ABSENT", f"00-hint.md 에 파생 블록 표지가 없다: {absent}",
                  "사실 블록 표지를 지우지 않는다 — publish 로 스캐폴드를 다시 만든다.")
    changed = [k for k in DERIVED_FACTS if have[k] != want[k]]
    if changed:
        p.write_text(_replace_fact_bodies(text, {k: want[k] for k in changed}), encoding="utf-8")
    return changed


# ── 렌더 ─────────────────────────────────────────────────────────────────────────────────────
def _normalize_facts(facts: dict) -> dict:
    """스냅샷 왕복과 같은 모양으로 정규화한다(튜플→리스트 · 키 문자열) — 렌더와 재생성이 같은 입력을 보게."""
    if not isinstance(facts, dict):
        core.fail("HINT_FACTS_SHAPE", f"facts 는 dict 여야 한다: {type(facts).__name__}")
    try:
        return json.loads(core.dumps(facts))
    except (TypeError, ValueError) as e:
        core.fail("HINT_FACTS_SHAPE", f"facts 를 JSON 으로 직렬화할 수 없다: {e}")


def _render_doc(ctx: _Ctx, name: str) -> str:
    doc = ctx.templates[name]
    lines = list(doc.lines)
    edits: list[tuple[int, int, list[str]]] = []
    for f in doc.facts:
        body = FACT_RENDERERS[f.fid](ctx) if f.fid in FACT_RENDERERS else ""
        new = body.split("\n") if body else []
        if f.open_ended:
            new = [""] + new + [""]      # 표지 · 다음 챕터 헤딩과 빈 줄로 떨어뜨린다(본문 대조는 앞뒤 빈 줄을 무시한다)
        edits.append((f.start + 1, f.end, new))
    for pr in doc.prompts:
        cond = _directives(pr.fields)["condition"]
        ok, actual = _condition_holds(cond, ctx.facts)
        if ok:
            continue
        end = pr.end
        for k in range(pr.end + 1, len(lines)):       # 뒤따르는 `<<AGENT:` 자리표시도 함께 걷는다
            if lines[k].startswith(AGENT_MARK):
                end = k
                break
            if lines[k].strip():
                break
        edits.append((pr.start, end + 1, [_na_line(cond, actual)]))
    for start, end, new in sorted(edits, key=lambda e: e[0], reverse=True):
        lines[start:end] = new
    return "\n".join(lines)


def _render_readme(ctx: _Ctx) -> str:
    p = _templates_dir(ctx.repo) / README_TEMPLATE
    if not p.is_file():
        core.fail("HINT_TEMPLATE_ABSENT", f"README 템플릿 부재: {core.REL_SKILL}/templates/{README_TEMPLATE}")
    f = ctx.facts
    tokens = {
        "TAG": str(f.get("tag")),
        "FORMAT": FORMAT,
        "GRAMMAR": _cell(_dict(f, "naming").get("grammar")),
        "GENERATED_UTC": str(f.get("generated_utc")),
        "BANNER": "\n".join(("> ⚠ " if i == 0 else "> ") + ln for i, ln in enumerate(BANNER_LINES)),
        "SUB_RECIPE_ROW": " · (멀티) 서브 레시피 해설" if _topology(f) == "multi" else "",
        "EVENT_KINDS": _event_kinds_md(ctx.templates),
        "LEGEND": _legend_md(),
        # 2026-09-29 plan_26092908 §4.4·§4.6: 판정 한 줄 · 이 태그 이름 읽는 법(PAYLOAD.naming 에서 생성) · 슬롯 → 빌드 컨텍스트 요약
        "VERDICT": f"> **판정** {_verdict_text(f)}" + (f" · **{REFUTE_MARKER}** = 기각된 성능 판정(수치는 관측)"
                                                       if _dict(f, "measurement").get("verdict") == REFUTE_MARKER else ""),
        "NAME_GUIDE": name_guide_md(f),
        "CONTEXT_MAP": _context_map_summary(f),
    }

    def repl(m):
        if m.group(1) not in tokens:
            core.fail("HINT_TEMPLATE_SHAPE", f"README 템플릿의 모르는 자리표시: {m.group(0)}")
        return tokens[m.group(1)]

    return _README_TOKEN.sub(repl, p.read_text(encoding="utf-8"))


# 이름 세대 표(수신자 안내 · 2026-09-29 plan_26092908 §4.6 U3 · V9). 안내 README(templates/hint-branch-README.md)의 세대 표와 같은 뜻 —
#   정적 파일과 생성 문구는 한쪽이 다른 쪽을 만들 수 없으므로 hint.py 자체검사가 세대 라벨 교차검증을 한다.
NAME_GENERATIONS = (
    ("v7", "`q·len·kv` 결정론 + 근거 붙은 꼬리 + 중복 시 `-t<YYMMDDHHMM>` · 페이로드 커밋의 부모 = 안내 커밋", "2026-09-29 ~"),
    ("v6", "레시피 여섯 축 고정 `q…-len…-kv…-ple…-spec<n|off>-<graph|eager>`(v7 파서는 3축 + 꼬리 셋으로 읽는다)", "2026-09-22 ~ 09-28"),
    ("옛 5세그먼트(노드 축)", "arch `<hw>-<main|sub|cluster>-<target>`", "2026-09-06 ~ 09-21"),
    ("옛 5세그먼트", "arch `<hw>-<target>` · 노드 축 없음", "2026-09-04 ~ 09-06"),
    ("옛 4세그먼트", "`hint/<vllm>/<model>/<arch>` · 레시피 축 없음", "2026-09-04 이전"),
)
_SEGMENT_MEANINGS = {
    "vllm": "빌드 입력 — 릴리스 태그면 그 릴리스, 커밋 핀이면 `<직전 릴리스>-g<커밋 12자>`(엔진 자기보고는 00 §0.4 의 다른 행)",
    "model": "체크포인트 이름(Hugging Face 등록명 소문자 · 양자화 접미사 그대로)",
    "q": "양자화 — 체크포인트가 **선언한** 방식(혼합 구성은 00 §0.4 양자화 구성 표)",
    "len": "최대 컨텍스트 길이(max-model-len)",
    "kv": "KV 캐시 dtype(서빙 설정 선언)",
}


def name_guide_md(f: dict) -> str:
    """페이로드 README "이 태그 이름 읽는 법"(PAYLOAD.naming 에서 결정론 생성 · 판정 ✗). 세그먼트 · 축마다 값 · 뜻 · 출처, 꼬리 토큰의 뜻 ·
    근거, timestamp 의 뜻, `native` · `-bare` 의 뜻, 세대 표. 꼬리 확정 전(publish 스캐폴드)이면 그렇다고 적는다(continue 가 다시 렌더)."""
    nm = _dict(f, "naming")
    seg, axes = _dict(nm, "segments"), _dict(nm, "axes")
    rows = []
    for k in ("vllm", "model"):
        v, src = _axis(seg.get(k))
        rows.append((f"`<{k}>`", f"`{_cell(v)}`", _SEGMENT_MEANINGS[k], _cell(src)))
    av, asrc = _axis(seg.get("arch"))
    parts = {k: _axis(axes.get(k))[0] for k in ("hw", "gpus_per_node", "nodes", "role", "target", "plane")}
    rows.append(("`<arch>`", f"`{_cell(av)}`",
                 f"`<hw>-<G>g<N>n-<role>-<target>[-bare]` — hw `{_cell(parts['hw'])}` · 노드당 GPU {_cell(parts['gpus_per_node'])} · "
                 f"노드 {_cell(parts['nodes'])} · 역할 `{_cell(parts['role'])}` · 타겟 `{_cell(parts['target'])}` · 평면 "
                 f"`{_cell(parts['plane'], 'docker(토큰 없음)')}`", _cell(asrc)))
    for k in ("q", "len", "kv"):
        v, src = _axis(axes.get(k))
        rows.append((f"`{k}`", f"`{_cell(v)}`", _SEGMENT_MEANINGS[k], _cell(src)))
    tail = nm.get("tail")
    if isinstance(tail, list):
        for r in tail:
            if isinstance(r, dict):
                rows.append((f"꼬리 `{_cell(r.get('token'))}`", f"`{_cell(r.get('token'))}`", _cell(r.get("meaning")),
                             "발행 Agent · 근거 " + _tail_evidence_text(r.get("evidence"))))
        if nm.get("timestamp"):
            rows.append(("timestamp", f"`{_cell(nm.get('timestamp'))}`",
                         "같은 이름이 이미 있어 붙인 발행 시각(KST `YYMMDDHHMM`) — 같은 셀의 새 판", "도구(generated_utc · 결정론)"))
    out = [f"`{_cell(f.get('tag'))}`", ""]
    out += _table(("자리", "값", "뜻", "출처"), rows)
    if not isinstance(tail, list):
        out += ["", "_꼬리 미확정 — 이 README 는 publish 스캐폴드다(continue 가 꼬리를 확정하며 다시 렌더한다)._"]
    target, plane = str(parts.get("target") or ""), parts.get("plane")
    out += ["",
            f"- `native` = **실제 하드웨어에서 잰 것**(`sim-<타겟>` = 다른 GPU 의 메모리 예산 흉내의 반대) — Docker 여부와 **무관**하다. 이 태그: "
            f"타겟 `{_cell(target)}`.",
            "- `-bare` = **Docker 없이**(호스트 venv) 서빙한 셀 · 토큰이 없으면 Docker 셀이다. 이 태그: 평면 "
            f"`{_cell(plane, 'docker')}`.",
            "- 꼬리는 규칙 목록이 아니라 근거가 붙은 자유 기재다 — 비슷한 hint 는 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)로 먼저 맞춘다.",
            "", "**이름 세대**(옛 태그는 교정 · 리콜하지 않는다)", ""]
    out += _table(("세대", "모양", "시기"), [(f"`{g}`" if g.startswith("v") else g, _cell(shape), when)
                                         for g, shape, when in NAME_GENERATIONS])
    return "\n".join(out)


def _context_map_summary(f: dict) -> str:
    """README 의 슬롯 → 빌드 컨텍스트 요약 — build_context_map 을 (zip 폴더 → 컨텍스트 폴더) 로 묶어 센다(파일별 표는 01 §1.4)."""
    rows = [r for r in (f.get("build_context_map") or []) if isinstance(r, dict) and r.get("slot_path")]
    if not rows:
        return "_매핑 없음 — 빌드 컨텍스트 매핑을 파생하지 못했다(01 §1.4)._"
    groups: dict[tuple[str, str], int] = {}
    outside = 0
    for r in rows:
        if not r.get("context_path"):
            outside += 1
            continue
        cdir = Path(str(r["context_path"])).parent.as_posix()
        key = (Path(str(r["slot_path"])).parent.as_posix() + "/", "(컨텍스트 루트)" if cdir == "." else cdir + "/")
        groups[key] = groups.get(key, 0) + 1
    out = _table(("zip 폴더", "빌드 컨텍스트 자리", "파일 수"), [(f"`{a}`", f"`{b}`", str(n)) for (a, b), n in sorted(groups.items())])
    if outside:
        out += ["", f"_빌드 컨텍스트 입력이 아닌 파일 {outside}개(기록 · pip freeze 등) — 01 §1.4 표의 `—` 행._"]
    return "\n".join(out)


def default_snapshot_path(payload_dir: Path) -> Path:
    """facts 스냅샷 자리 = 드래프트의 `inputs/`(X2 · 도구가 쓴 JSON — 사람 손 JSON 0 · gitignored `hints/.drafts/`)."""
    return Path(payload_dir).parent / "inputs" / FACTS_SNAPSHOT


def load_snapshot(path: Path) -> dict:
    return core.read_json(Path(path), code="HINT_FACT_SNAPSHOT_UNREADABLE")


def render_scaffold(repo: Path, facts: dict, payload_dir: Path, *, snapshot_path: Path | None = None) -> list[dict]:
    """템플릿 4종 + README 를 페이로드 디렉터리에 렌더한다 — 사실 블록을 채우고 PROMPT·`<<AGENT:` 자리를 남긴다.

    - 조건이 성립하지 않는 PROMPT(예: `조건: topology=multi` 인데 single)는 "해당 없음" 한 줄로 바꾼다.
    - facts 스냅샷을 `snapshot_path`(기본 `default_snapshot_path`)에 쓴다 — lint 의 사실 블록 재생성 diff 0 이 이것을 읽는다.
    - 기존 저작물을 덮지 않는다(HINT_SCAFFOLD_EXISTS).
    반환 = `prompts(payload_dir)`(채워야 할 PROMPT 목록)."""
    repo, payload_dir = Path(repo), Path(payload_dir)
    facts = _normalize_facts(facts)
    if not isinstance(facts.get("tag"), str) or not facts["tag"].startswith(core.HINT_TAG_PREFIX):
        core.fail("HINT_FACTS_SHAPE", f"facts.tag 는 `{core.HINT_TAG_PREFIX}` 로 시작하는 파생 이름이어야 한다: {facts.get('tag')!r}")
    core.require_utc(facts.get("generated_utc"), "facts.generated_utc")
    if _topology(facts) is None:
        core.fail("HINT_FACTS_SHAPE", "facts.identity.topology 가 single|multi 가 아니다 — 토폴로지는 manifest 에서 관측한다.")
    templates = _load_templates(repo)
    ctx = _Ctx(repo, facts, templates)
    targets = [payload_dir / n for n in PAYLOAD_DOCS] + [payload_dir / README_NAME]
    exist = [t.name for t in targets if t.is_file() and t.stat().st_size]
    if exist:
        core.fail("HINT_SCAFFOLD_EXISTS", f"기존 저작물을 덮지 않는다: {payload_dir} ({', '.join(exist)})",
                  "이어서 저작하려면 `hint.py continue` · 새로 시작하려면 드래프트를 지우고 publish 를 다시 한다.")
    rendered = {name: _render_doc(ctx, name) for name in TEMPLATE_FILES}
    readme = _render_readme(ctx)
    payload_dir.mkdir(parents=True, exist_ok=True)
    for name, text in rendered.items():
        (payload_dir / f"{name}.md").write_text(text, encoding="utf-8")
    (payload_dir / README_NAME).write_text(readme, encoding="utf-8")
    write_agent_requests(payload_dir, facts.get("agent_requests"))     # 2026-09-29 plan_26092908 §4.2 — 렌더러 없는 자리 = 빈 파일 + 자리표시
    core.write_json(Path(snapshot_path) if snapshot_path else default_snapshot_path(payload_dir), facts)
    refresh(payload_dir)
    return prompts(payload_dir)


def prompts(payload_dir: Path) -> list[dict]:
    """페이로드에 남은 PROMPT 목록 — {file, section, title, line, question, read, instruction, required, forbidden,
    blocks, excerpts_required, optional}. publish 가 "채워야 할 절" 로 출력한다."""
    out = []
    for name in PAYLOAD_DOCS:
        p = Path(payload_dir) / name
        if not p.is_file():
            continue
        doc = _Doc.parse(name, p.read_text(encoding="utf-8"))
        titles = {num: title for _, num, title in doc.chapters}
        for pr in doc.prompts:
            d = _directives(pr.fields)
            out.append({"file": name, "section": pr.chapter, "title": titles.get(pr.chapter, ""), "line": pr.start + 1,
                        "question": _question(pr.fields), "read": _norm(pr.fields.get("읽을 것", "")),
                        "instruction": pr.fields.get("기재", ""), "required": _norm(pr.fields.get("필수 필드", "")),
                        "forbidden": _norm(pr.fields.get("금지", "")),
                        "blocks": {k: v for k, v in sorted(d["blocks"].items())},
                        "excerpts_required": d["excerpts"], "optional": d["optional"]})
    return out


def seal_prompts(payload_dir: Path) -> None:
    """`<!-- PROMPT … -->` → `> 이 절이 답하는 질문: <질문>` (지시문은 배포되지 않되 의도는 남는다). 멱등.
    `<<AGENT:` 가 남아 있으면 봉인하지 않는다(미저작 절을 봉인하면 빈 절이 배포된다).
    **전부 검사한 뒤 전부 쓴다** — 문서 하나씩 검사·쓰기를 번갈아 하면 셋째 문서의 잔존에서 멈출 때 앞 둘만 봉인된 반쪽
    상태가 남는다(거부 = 부수효과 0 규율)."""
    sealed: dict[Path, str] = {}
    for name in PAYLOAD_DOCS:
        p = Path(payload_dir) / name
        if not p.is_file():
            core.fail("HINT_DOCUMENT_ABSENT", f"봉인할 문서 부재: {name}")
        text = p.read_text(encoding="utf-8")
        if AGENT_MARK in text:
            line = text[:text.index(AGENT_MARK)].count("\n") + 1
            core.fail("HINT_AGENT_PLACEHOLDER_RESIDUE", f"{name}:{line}: 미저작 자리표시 `{AGENT_MARK}` 가 남아 있다",
                      "그 절을 PROMPT 지시대로 저작한 뒤 lint 를 통과시키고 봉인한다.")
        doc = _Doc.parse(name, text)
        if not doc.prompts and not doc.selfchecks:
            continue
        lines = list(doc.lines)
        edits: list[tuple[int, int, list[str]]] = []
        for pr in doc.prompts:
            q = _question(pr.fields)
            if not q:
                core.fail("HINT_PROMPT_QUESTION_ABSENT", f"{name}:{pr.start + 1}: `질문:` 없는 PROMPT")
            nxt = lines[pr.end + 1] if pr.end + 1 < len(lines) else ""
            # 봉인 줄 바로 뒤에 산문이 붙으면 마크다운이 그 산문을 인용문의 게으른 계속으로 먹는다 — 빈 줄로 끊는다.
            edits.append((pr.start, pr.end + 1, [SEALED_PREFIX + q] + ([""] if nxt.strip() else [])))
        for a, b in doc.selfchecks:
            # 저작 자기점검(2026-09-22 S2 round 2 F11)은 통째로 지운다 — 수신자에게는 쓸모가 없는 저작 지시다. 뒤따르는 빈 줄 하나도
            #   함께 걷는다(빈 줄 두 개가 남지 않게).
            end = b + 1
            if end < len(lines) and not lines[end].strip() and (a == 0 or not lines[a - 1].strip()):
                end += 1
            edits.append((a, end, []))
        for a, b, new in sorted(edits, key=lambda e: e[0], reverse=True):
            lines[a:b] = new
        sealed[p] = "\n".join(lines)
    for p, text in sealed.items():
        p.write_text(text, encoding="utf-8")


_H3 = re.compile(r"^###\s+(.+?)\s*$")
_KEY_SPLIT = re.compile(r"\s|—|·|\(|:")


def subsections(payload_dir: Path, doc: str, chapter: str) -> list[dict]:
    """한 챕터의 `### <소제목>` 과 그 **첫 문단**(저작 영역만 — 주석·사실 블록·코드·인용·표·헤딩 줄은 문단이 아니다).
    반환 [{title, key, text}] — key = 소제목 첫 토큰(백틱 제거 · 슬롯 이름·파일명 대조용). 부작용 0(2026-09-22 통합: continue 가
    01 §1.2 슬롯별 적용 사유를 PAYLOAD.slots[*].rationale 로 옮길 때 쓴다 — 선언 신호 = 3신호의 셋째)."""
    p = Path(payload_dir) / f"{doc}.md"
    if not p.is_file():
        return []
    d = _Doc.parse(f"{doc}.md", p.read_text(encoding="utf-8"))
    out: list[dict] = []
    cur: dict | None = None
    for i in d.chapter_lines(chapter):
        line = d.lines[i]
        m = None if (d.masked[i] or d.fence[i]) else _H3.match(line)
        if m:
            title = m.group(1).strip()
            key = _KEY_SPLIT.split(title.strip("`").strip(), 1)[0].strip("`").strip()
            cur = {"title": title, "key": key, "para": [], "done": False}
            out.append(cur)
            continue
        if cur is None or cur["done"]:
            continue
        s = line.strip()
        if d.masked[i] or d.fence[i] or not s or s.startswith((">", "|", "#", "```", "<!--")):
            if cur["para"]:
                cur["done"] = True
            continue
        cur["para"].append(s)
    return [{"title": c["title"], "key": c["key"], "text": " ".join(c["para"]) or None} for c in out]


def brief(payload_dir: Path) -> str:
    """00-hint §0.1 의 첫 문단(최대 3줄) — annotation brief(D4). PROMPT·봉인 질문 줄만 건너뛴다.
    첫 문단이 인용·표·헤딩·주석·코드로 시작하면 차단(감사 ⑤ — 주석이 brief 로 캐내졌다)."""
    p = Path(payload_dir) / "00-hint.md"
    if not p.is_file():
        core.fail("HINT_BRIEF_ABSENT", "00-hint.md 부재")
    doc = _Doc.parse("00-hint.md", p.read_text(encoding="utf-8"))
    idx = doc.chapter_lines("0.1")
    if not idx:
        core.fail("HINT_BRIEF_ABSENT", "00-hint.md §0.1 부재")
    para: list[str] = []
    for i in idx[1:]:                       # idx[0] = 헤딩 줄
        line = doc.lines[i]
        if doc.masked[i] and not para and not doc.lines[i].lstrip().startswith("<!-- FACT"):
            continue                        # PROMPT(봉인 전) — 저작 영역이 아니다
        if not para and (not line.strip() or line.startswith(SEALED_PREFIX)):
            continue
        if not para:
            s = line.lstrip()
            if doc.masked[i] or s.startswith((">", "|", "#", "```", "<!--")):
                core.fail("HINT_BRIEF_SHAPE", f"00-hint.md:{i + 1}: §0.1 첫 문단이 인용·표·헤딩·주석·코드로 시작한다",
                          "brief 는 평문 한 문단이다(추출기가 첫 문단을 그대로 annotation 에 싣는다).")
        if not line.strip() or doc.masked[i]:
            break                           # 빈 줄 · 주석 줄(HTML 주석은 문단을 끊는다)에서 첫 문단이 끝난다
        para.append(line.rstrip())
    if not para:
        core.fail("HINT_BRIEF_ABSENT", "00-hint.md §0.1 에 문단이 없다")
    if any(AGENT_MARK in ln for ln in para):
        core.fail("HINT_BRIEF_ABSENT", "00-hint.md §0.1 이 아직 저작되지 않았다(`<<AGENT:` 잔존)")
    if any("<!--" in ln or "-->" in ln for ln in para):
        # tag._canonical_brief 가 같은 이유로 거부한다(⑤ 사고: `<!--` 가 카탈로그 렌더에서 뒤 행을 삼켰다). 그 거부는
        # hint 브랜치 커밋 **뒤** 봉인 단계에서 나므로(브랜치는 되감지 않는다) 여기 lint 에서 먼저 막는다.
        core.fail("HINT_BRIEF_SHAPE", "00-hint.md §0.1 첫 문단에 HTML 주석 경계(`<!--` · `-->`)가 있다",
                  "brief 문단에서 주석 표기를 뺀다(annotation·카탈로그가 그대로 싣는다).")
    if len(para) > 3:
        core.fail("HINT_BRIEF_TOO_LONG", f"00-hint.md §0.1 첫 문단이 {len(para)}줄이다(최대 3줄)")
    return "\n".join(para)


# ── 린터 ─────────────────────────────────────────────────────────────────────────────────────
_UNSET = object()


def _resolvers(repo: Path, subst_table, substitute, resolve_stem, source_text):
    """(substitute, resolve_stem, source_text) 기본값 채움. substitute 는 둘 다 없으면 None(= 무결성 검사 불가 · 호출부가
    fail-closed). lineage · pii 는 여기서 지연 적재한다(import 부수효과 0)."""
    if substitute is None and subst_table is not None:
        from . import pii as _pii
        table = list(subst_table)
        substitute = lambda t, _tb=table: _pii.substitute(t, _tb)
    if resolve_stem is None:
        from . import lineage as _lineage
        resolve_stem = _lineage.resolve_stem
    if source_text is None:
        def source_text(rel: str) -> str | None:
            try:
                return read_source_raw(Path(repo) / rel)      # 개행 변환 ✗ — 정규화는 _Sources.text 가 한 번 한다
            except OSError:
                return None
    return substitute, resolve_stem, source_text


def _git_origin_reader(repo: Path) -> Callable[[str, str], bytes | None]:
    """스냅샷 origin 의 기본 판독기(읽기 전용): `git show <rev>:<path>` 바이트 · 실패 · git 부재 = None(= 확인 불가 · fail-closed)."""
    def read(rev: str, path: str) -> bytes | None:
        try:
            return core.git_bytes(Path(repo), "show", f"{rev}:{path}", check=False)
        except OSError:
            return None
    return read


def substituted_source(repo: Path, payload_dir: Path, token: str, *, lineage: dict, subst_table=None,
                       substitute: Callable[[str], str] | None = None,
                       source_text: Callable[[str], str | None] | None = None,
                       resolve_stem: Callable[[dict, str], str | None] | None = None,
                       draft_dir: Path | None = None,
                       snapshot_origin: Callable[[str, str], bytes | None] | None = None) -> str:
    """발췌 저작 도우미(읽기 전용 · 부수효과 0): 출처 토큰(문서 stem · 저장소 상대 경로 · artifacts 파일명) → 린터가 발췌를
    대조하는 **치환 후 원문 전체**. 발췌 줄은 이 출력에서 그대로 옮긴다 — 운영자 경로·호스트·사설 IP 가 치환 자리표시
    (`<node:main>` · `<priv-ip>` 등)로 무엇이 되는지 저작자가 추측하지 않게(추측하면 무결성 불일치 또는 PII 검출).
    린터와 같은 해소기(`_resolvers` · `_Sources.locate`)를 쓴다 — 해소 불가·모호·읽기 불가·치환 함수 부재 = HintError."""
    repo, payload_dir = Path(repo), Path(payload_dir)
    substitute, resolve_stem, source_text = _resolvers(repo, subst_table, substitute, resolve_stem, source_text)
    if substitute is None:
        core.fail("HINT_EXCERPT_VERIFIER_ABSENT", "치환표(subst_table) 또는 치환 함수가 없다 — 치환 후 원문을 만들 수 없다.",
                  "pii.substitution_table(repo, manifest) 결과를 넘긴다.")
    src = _Sources(repo, payload_dir, lineage or {}, resolve_stem, source_text, substitute, draft_dir=draft_dir,
                   snapshot_origin=snapshot_origin)
    loc = src.locate(_source_token(token))
    if loc is None:
        core.fail("HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE", f"출처 `{token}` 가 LINEAGE · artifacts 밖이다(또는 모호).",
                  "LINEAGE.json 의 문서·evidence_candidates 경로 또는 artifacts 파일명을 쓴다 · 빠진 계보는 --lineage-add.")
    sp = src.snapshot_problem(loc)
    if sp is not None:
        core.fail(sp[0], sp[1], "스냅샷은 publish 가 `git show` 로 쓴 바이트 그대로여야 한다 — 고쳤다면 publish 를 새 draft 로 다시 한다.")
    text = src.text(loc)
    if text is None:
        core.fail("HINT_EXCERPT_SOURCE_UNREADABLE", f"출처를 읽을 수 없다: {loc[1]}")
    return substitute(text)


class _Sources:
    """출처 해소(발췌 · 서명 · 출처 필드 공용). 순서: 페이로드 artifacts → LINEAGE(문서 · evidence_candidates).
    `text` 는 **정규화된** 원문이다(normalize_source — `\n` 줄 · `\r` 진행 조각 · ANSI 제거) — 발췌 도우미와 린터가 같은 바이트를 본다."""

    def __init__(self, repo: Path, payload_dir: Path, lineage: dict, resolve_stem, source_text, substitute, *,
                 draft_dir: Path | None = None, snapshot_origin: Callable[[str, str], bytes | None] | None = None):
        self.repo, self.payload, self.lineage = repo, payload_dir, lineage
        self.resolve_stem, self.source_text, self.substitute = resolve_stem, source_text, substitute
        root = payload_dir / "artifacts"
        self.art = sorted(q.relative_to(payload_dir).as_posix() for q in root.rglob("*") if q.is_file()) \
            if root.is_dir() else []
        self.by_name: dict[str, list[str]] = {}
        for rel in self.art:
            self.by_name.setdefault(Path(rel).name, []).append(rel)
        # 측정 도구 스냅샷(2026-09-22 · plan_26092119 S2 round 3): draft 상대 `inputs/sources/<name>@<rev12>` — LINEAGE evidence_candidates
        #   에 **등재되고** 디스크에 있는 것만 출처다(등재 없는 파일 · 파일 없는 등재 = 밖). 저장소 상대 경로로 읽으면 없는 파일이라
        #   lineage 해소보다 먼저 가른다. draft 는 **명시**로 받는다 — hint.py 의 lint 는 페이로드 임시 사본(refresh 미리 반영)을, verify 는
        #   봉인 트리를 푼 임시 디렉터리를 검사하므로 payload_dir.parent 는 draft 가 아니다(2026-09-22 round 3 라이브 재생에서 드러남).
        self.draft = Path(draft_dir) if draft_dir is not None else payload_dir.parent
        cands = [x for x in (lineage or {}).get("evidence_candidates") or () if isinstance(x, dict)]
        listed = {str(x.get("path")) for x in cands}
        self.snap_origin = {str(x.get("path")): str(x.get("origin") or "") for x in cands
                            if str(x.get("path") or "").startswith(SNAPSHOT_DIR + "/")}
        self.origin_bytes = snapshot_origin if snapshot_origin is not None else _git_origin_reader(repo)
        self._snap_problem: dict[str, tuple[str, str] | None] = {}
        sdir = self.draft / SNAPSHOT_DIR
        self.snap: dict[str, str] = {}
        if sdir.is_dir():
            for q in sorted(sdir.iterdir()):
                rel = f"{SNAPSHOT_DIR}/{q.name}"
                if q.is_file() and not q.is_symlink() and rel in listed:
                    self.snap[q.name] = rel
        self._text: dict[tuple[str, str], str | None] = {}
        self._normsub: dict[tuple[str, str], str | None] = {}
        self._sublines: dict[tuple[str, str], list[str] | None] = {}

    def locate(self, token: str) -> tuple[str, str] | None:
        t = (token or "").strip().strip("`")
        if not t:
            return None
        if t.startswith(SNAPSHOT_DIR + "/") or _SNAPSHOT_TOKEN.fullmatch(t):
            # 스냅샷 모양의 토큰은 스냅샷으로만 해소한다(저장소 · artifacts 로 흘려 다른 파일에 붙지 않게)
            name = t[len(SNAPSHOT_DIR) + 1:] if t.startswith(SNAPSHOT_DIR + "/") else t
            return ("snapshot", self.snap[name]) if name in self.snap else None
        if t in self.art:
            return ("artifact", t)
        if f"artifacts/{t}" in self.art:
            return ("artifact", f"artifacts/{t}")
        names = self.by_name.get(t, [])
        if len(names) == 1:
            return ("artifact", names[0])
        path = self.resolve_stem(self.lineage, t)
        return ("lineage", path) if path else None

    def text(self, loc: tuple[str, str]) -> str | None:
        if loc not in self._text:
            kind, key = loc
            val = None
            if kind == "artifact":
                try:
                    val = read_source_raw(self.payload / key, errors="strict")
                except (OSError, UnicodeDecodeError):
                    val = None
            elif kind == "snapshot":
                # origin 과 바이트 동일한 스냅샷만 읽는다(다르면 None — 호출부가 snapshot_problem 으로 사유코드를 낸다)
                if self.snapshot_problem(loc) is None:
                    try:
                        val = read_source_raw(self.draft / key)      # git show 바이트 — 계보 문서와 같은 읽기(replace)
                    except OSError:
                        val = None
            else:
                val = self.source_text(key)
            self._text[loc] = None if val is None else normalize_source(val)
        return self._text[loc]

    def snapshot_problem(self, loc) -> tuple[str, str] | None:
        """스냅샷 출처의 origin 대조 결과 — None = draft 파일이 LINEAGE origin(`git show <rev>:<path>`)과 바이트 동일 ·
        (code, 사유) = HINT_EXCERPT_SNAPSHOT_DRIFT(바이트가 다르다) | HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE(origin 모양 결함 · git 에서 못 읽음).
        스냅샷이 아닌 출처는 None."""
        kind, key = loc
        if kind != "snapshot":
            return None
        if key not in self._snap_problem:
            prob: tuple[str, str] | None = None
            m = _SNAPSHOT_ORIGIN.match(self.snap_origin.get(key, ""))
            try:
                data = (self.draft / key).read_bytes()
            except OSError as e:
                data, prob = None, ("HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE", f"스냅샷 `{key}` 를 읽을 수 없다: {type(e).__name__}")
            if prob is None and m is None:
                prob = ("HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE",
                        f"스냅샷 `{key}` 의 LINEAGE origin 이 `git:<40자 SHA>:<저장소 경로>` 가 아니다: {self.snap_origin.get(key)!r}")
            if prob is None:
                want = self.origin_bytes(m.group(1), m.group(2))
                if want is None:
                    prob = ("HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE",
                            f"스냅샷 `{key}` 의 origin `git show {m.group(1)[:12]}:{m.group(2)}` 을 읽을 수 없다(커밋 부재 · git 아님)")
                elif want != data:
                    prob = ("HINT_EXCERPT_SNAPSHOT_DRIFT",
                            f"스냅샷 `{key}` 가 origin `git show {m.group(1)[:12]}:{m.group(2)}` 과 바이트가 다르다 — 스냅샷을 고치지 "
                            "않는다(publish 가 쓴 git 바이트가 발췌의 원천이다 · 다시 받으려면 publish 를 새로)")
            self._snap_problem[key] = prob
        return self._snap_problem[key]

    def sublines(self, loc) -> list[str] | None:
        """치환 후 원문의 줄 목록(`§L<a>-<b>` 대조 · `hint.py excerpt --lines` 와 같은 번호 — 둘 다 이 정규화 + 치환 뒤 `\n` 분할)."""
        if loc not in self._sublines:
            t = self.text(loc)
            self._sublines[loc] = None if t is None else self.substitute(t).split("\n")
        return self._sublines[loc]

    def normsub(self, loc) -> str | None:
        if loc not in self._normsub:
            t = self.text(loc)
            self._normsub[loc] = None if t is None else _norm(self.substitute(t))
        return self._normsub[loc]


def _heading_key(text: str) -> str:
    s = _WS.sub(" ", str(text or "")).strip().lstrip("§").strip().strip("`*").strip()
    return s.casefold()


def _section_in_headings(label: str, text: str, substitute) -> bool:
    """발췌 `§<절>` 이 마크다운 출처의 제목 줄과 **정규화 접두 일치**하는가(D-c). 코드 펜스 안 `#` 줄은 제목이 아니다.
    원문 제목과 치환 후 제목 둘 다 본다(저작자는 치환 후 원문을 보고 옮긴다 · 운영자 경로가 제목에 있을 수 있다)."""
    want = _heading_key(label)
    if not want:
        return False
    heads: list[str] = []
    in_fence = False
    for line in (text or "").split("\n"):
        if _FENCE.match(line):
            in_fence = not in_fence
            continue
        if not in_fence:
            m = _MD_HEADING.match(line)
            if m and m.group(1).strip():
                heads.append(m.group(1))
    keys = {_heading_key(h) for h in heads}
    if substitute is not None:
        keys |= {_heading_key(substitute(h)) for h in heads}
    return any(k.startswith(want) for k in keys)


def lint(repo: Path, payload_dir: Path, *, lineage: dict, subst_table=None, manifest: dict | None = None,
         bench_report: Path | str | None = None, facts: dict | None = None, sealed: bool = False,
         substitute: Callable[[str], str] | None = None, source_text: Callable[[str], str | None] | None = None,
         resolve_stem: Callable[[dict, str], str | None] | None = None, pii_terms=_UNSET,
         draft_dir: Path | None = None, snapshot_origin: Callable[[str, str], bytes | None] | None = None) -> list[dict]:
    """페이로드 md 4종 + README 를 결정론으로 검사한다(fail-closed · 부수효과 0). 반환 [{code, file, line, message}] — 빈 목록 = 통과.

    인자
        lineage      LINEAGE.json 객체(문서 0건 = HINT_LINEAGE_EMPTY)
        subst_table  pii.substitution_table 결과 — substitute 가 없으면 `pii.substitute(t, subst_table)` 로 치환한다
        substitute   치환 함수 주입(발췌·서명 대조의 양쪽에 적용 · 멱등). 둘 다 없으면 무결성 검사를 못 하므로 오류다
        resolve_stem (lineage, stem) → 저장소 상대 경로 | None — 기본 `lineage.resolve_stem`(유일 해소만 · 모호 = 밖)
        source_text  저장소 상대 경로 → 본문 | None — 기본 = 저장소 파일 읽기
        manifest     work-manifest(task_class · benchmark.perf_waiver) — 없으면 facts 의 값을 쓴다
        bench_report 03 부하 곡선을 렌더한 원천(bench_report md 또는 sweep_index json) — render_bench_section 로 재렌더 diff 0
        facts        render_scaffold 가 쓴 facts(없으면 기본 스냅샷 자리에서 읽는다 · 부재 = HINT_FACT_SNAPSHOT_ABSENT)
        sealed       True = 봉인 뒤 검사(PROMPT 잔존 = 오류)
        pii_terms    배포 PII 리터럴 주입(기본 = `.claude/pii_terms.txt` · 부재 = HINT_MISSING_PII_TERMS)
        draft_dir    측정 도구 스냅샷(`inputs/sources/`)을 찾는 draft — 기본 payload_dir.parent(임시 사본을 검사하면 반드시 넘긴다)
        snapshot_origin (rev, path) → bytes | None — 스냅샷 origin 판독기(기본 `git show <rev>:<path>` · 자체검사 주입). 스냅샷 파일이
                     origin 과 바이트가 다르면 HINT_EXCERPT_SNAPSHOT_DRIFT · 판독 불가면 HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE
    """
    repo, payload_dir = Path(repo), Path(payload_dir)
    issues: list[tuple[str, str, int, str]] = []

    def add(code: str, file: str, line: int, msg: str) -> None:
        issues.append((code, file, int(line or 1), msg))

    templates = _load_templates(repo)
    if facts is None:
        snap = default_snapshot_path(payload_dir)
        if snap.is_file():
            facts = load_snapshot(snap)
        else:
            add("HINT_FACT_SNAPSHOT_ABSENT", "-", 1, f"facts 스냅샷 부재({snap}) — 사실 블록 재생성 diff 를 검사할 수 없다")
    facts = _normalize_facts(facts) if facts is not None else None
    ctx = _Ctx(repo, facts, templates) if facts is not None else None

    if not (lineage or {}).get("documents"):
        add("HINT_LINEAGE_EMPTY", "LINEAGE.json", 1, "계보 문서 0건 — 서사 원재료가 없다")

    # 치환 · 출처 해소 함수(발췌 저작 도우미 `substituted_source` 와 같은 해소기 — 두 자리가 갈리면 저작자가 본 원문과 린터가
    # 대조하는 원문이 달라진다)
    substitute, resolve_stem, source_text = _resolvers(repo, subst_table, substitute, resolve_stem, source_text)
    sources = _Sources(repo, payload_dir, lineage or {}, resolve_stem, source_text, substitute or (lambda t: t),
                       draft_dir=draft_dir, snapshot_origin=snapshot_origin)

    docs: dict[str, _Doc] = {}
    for name in TEMPLATE_FILES:
        p = payload_dir / f"{name}.md"
        if not p.is_file():
            add("HINT_DOCUMENT_ABSENT", f"{name}.md", 1, "필수 문서 부재")
            continue
        docs[name] = _Doc.parse(f"{name}.md", p.read_text(encoding="utf-8"))

    all_events: list[tuple[str, _RawEvent]] = []
    all_excerpts: list[tuple[str, _Excerpt]] = []
    for name, doc in docs.items():
        fname, tdoc = doc.name, templates[name]
        for code, i, msg in doc.structure:
            add(code, fname, i + 1, msg)
        # ① 필수 챕터 · 순서 · 모르는 헤딩
        want = [(num, title) for _, num, title in tdoc.chapters]
        got = [(num, title) for _, num, title in doc.chapters]
        got_lines = {num: i for i, num, _ in doc.chapters}
        for num, title in want:
            if (num, title) not in got:
                add("HINT_SECTION_ABSENT", fname, 1, f"필수 챕터 부재: `## {num} {title}`")
            elif got.count((num, title)) > 1:
                add("HINT_SECTION_DUPLICATE", fname, got_lines[num] + 1, f"챕터 중복: {num}")
        order = [x for x in got if x in want]
        if order != [x for x in want if x in order]:
            add("HINT_SECTION_ORDER", fname, 1, "챕터 순서가 템플릿과 다르다")
        for i, num, title in doc.chapters:
            if (num, title) not in want:
                add("HINT_SECTION_UNKNOWN", fname, i + 1, f"템플릿에 없는 챕터: `## {num} {title}`")
        for i, text in doc.other_h2:
            add("HINT_SECTION_UNKNOWN", fname, i + 1, f"템플릿에 없는 `## ` 헤딩: {text!r} (소제목은 `###`)")
        # ② 사실 블록: 존재 · 자리 · 재생성 diff 0
        tfacts = {f.fid: f.chapter for f in tdoc.facts}
        for f in doc.facts:
            if f.fid not in tfacts:
                add("HINT_FACT_BLOCK_UNKNOWN", fname, f.start + 1, f"템플릿에 없는 사실 블록 FACT:{f.fid}")
        for fid, chapter in tfacts.items():
            found = doc.fact(fid)
            if not found:
                add("HINT_FACT_BLOCK_ABSENT", fname, 1, f"사실 블록 부재 FACT:{fid}")
                continue
            if len(found) > 1:
                add("HINT_FACT_BLOCK_DUPLICATE", fname, found[1].start + 1, f"사실 블록 중복 FACT:{fid}")
            if found[0].chapter != chapter:
                add("HINT_FACT_BLOCK_MISPLACED", fname, found[0].start + 1, f"FACT:{fid} 는 §{chapter} 에 있어야 한다")
            if fid in DERIVED_FACTS or ctx is None:
                continue
            try:
                expected = FACT_RENDERERS[fid](ctx)
            except core.HintError as e:
                add(e.code, fname, found[0].start + 1, f"FACT:{fid} 재생성 실패: {e.message}")
                continue
            if found[0].body.strip("\n") != expected.strip("\n"):
                add("HINT_FACT_DRIFT", fname, found[0].start + 1,
                    f"기계 사실 블록 FACT:{fid} 가 재생성 결과와 다르다(손으로 고치지 않는다 — 값이 틀리면 증거를 고친다)")
            ph = _FACT_PLACEHOLDER.search(expected)
            if ph:
                # 2026-09-22 S2 round 2(F9): 1차 FACT:reproduce 의 bench 명령이 `03-benchmark.md §3.x` 자리표시였다 — 생산자(artifacts ·
                #   evidence) 결함이다. 사실 블록은 손으로 고칠 수 없으므로 여기서 막고 생산자를 고친다.
                add("HINT_FACT_PLACEHOLDER", fname, found[0].start + 1,
                    f"기계 사실 블록 FACT:{fid} 에 절 자리표시 `{ph.group(0)}` 가 남아 있다 — 생산자(facts 조립)를 고친다")
        # ③ PROMPT: 템플릿과 글자 그대로(봉인 전) 또는 봉인 질문 줄(봉인 뒤) · 조건 불성립 = 해당 없음 줄
        tprompts = {p.chapter: p for p in tdoc.prompts}
        pprompts: dict[str, list[_Prompt]] = {}
        if sealed:
            from . import branch as _branch     # 잔존 정규식의 단일 소유자(지연 적재 · import 부수효과 0)
            for i, line in enumerate(doc.lines):
                if not doc.fence[i] and _branch.PROMPT_RESIDUE_RE.search(line):
                    add("HINT_PROMPT_RESIDUE", fname, i + 1, "봉인 뒤 PROMPT 잔존")
                if not doc.fence[i] and _SELFCHECK_RESIDUE.search(line):
                    add("HINT_SELFCHECK_RESIDUE", fname, i + 1, "봉인 뒤 저작 자기점검(SELFCHECK) 주석 잔존 — 배포하지 않는다")
        for pr in doc.prompts:
            pprompts.setdefault(pr.chapter, []).append(pr)
            if pr.chapter not in tprompts:
                add("HINT_PROMPT_TAMPERED", fname, pr.start + 1, f"템플릿에 없는 PROMPT(§{pr.chapter})")
        for chapter, tp in tprompts.items():
            d = _directives(tp.fields)
            ok, actual = True, ""
            if ctx is not None:
                try:
                    ok, actual = _condition_holds(d["condition"], ctx.facts)
                except core.HintError as e:
                    add(e.code, fname, tp.start + 1, e.message)
            lines_in = doc.chapter_lines(chapter)
            if not ok:
                na = _na_line(d["condition"], actual)
                if not any(doc.lines[i] == na for i in lines_in):
                    add("HINT_PROMPT_ABSENT", fname, 1, f"§{chapter}: 조건 불성립인데 '해당 없음' 줄이 없다")
                for pr in pprompts.get(chapter, []):
                    add("HINT_PROMPT_TAMPERED", fname, pr.start + 1, f"§{chapter}: 조건 불성립 절에 PROMPT 가 있다")
                continue
            sealed_line = SEALED_PREFIX + _question(tp.fields)
            has_sealed = any(doc.lines[i] == sealed_line for i in lines_in)
            mine = pprompts.get(chapter, [])
            same = [pr for pr in mine if [x.rstrip() for x in pr.raw] == [x.rstrip() for x in tp.raw]]
            for pr in mine:
                if pr not in same:
                    add("HINT_PROMPT_TAMPERED", fname, pr.start + 1, f"§{chapter}: PROMPT 가 템플릿과 다르다(지시를 고치지 않는다)")
            if sealed and not has_sealed:
                add("HINT_PROMPT_ABSENT", fname, 1, f"§{chapter}: 봉인 질문 줄이 없다")
            elif not sealed and not same and not has_sealed and not [pr for pr in mine if pr not in same]:
                add("HINT_PROMPT_ABSENT", fname, 1, f"§{chapter}: PROMPT(또는 봉인 질문 줄)가 없다")
            # ④ 저작 여부(선택 절 제외): 기계·고정 문구 밖에 저작된 줄이 있어야 한다
            if not d["optional"]:
                static = {doc_line.strip() for i, doc_line in enumerate(tdoc.lines)
                          if tdoc.chapter_of[i] == chapter and not tdoc.masked[i] and doc_line.strip()
                          and not doc_line.startswith(AGENT_MARK)}
                heading_idx = {i for i, num, _ in doc.chapters if num == chapter}
                authored = [i for i in lines_in if not doc.masked[i] and i not in heading_idx and doc.lines[i].strip()
                            and doc.lines[i] != sealed_line and doc.lines[i].strip() not in static
                            and not doc.lines[i].startswith(AGENT_MARK)]
                if not authored:
                    add("HINT_SECTION_UNAUTHORED", fname, (lines_in[0] + 1) if lines_in else 1,
                        f"§{chapter}: 저작된 내용이 없다(PROMPT 지시대로 기재한다)")
        # ⑤ 미저작 자리표시
        for i, line in enumerate(doc.lines):
            if AGENT_MARK in line:
                add("HINT_AGENT_PLACEHOLDER_RESIDUE", fname, i + 1, "미저작 자리표시 `<<AGENT:` 잔존")
        # ⑥ hint-event · 발췌 수집 + 절 지시(블록 kind · 최소 개수 · 필수 발췌)
        events, exs, malformed = _scan(doc)
        for ln in malformed:
            add("HINT_EXCERPT_HEADER_SHAPE", fname, ln, "발췌 머리는 `> [원문] <stem> §<절>` 이다")
        all_events += [(fname, e) for e in events]
        all_excerpts += [(fname, e) for e in exs]
        allowed = {p.chapter: _directives(p.fields) for p in tdoc.prompts}
        for raw in events:
            for code, line, msg in raw.errors:
                add(code, fname, line, msg)
            kind = (raw.event or {}).get("kind")
            if kind in EVENT_KINDS and kind not in allowed.get(raw.chapter, {"blocks": {}})["blocks"]:
                add("HINT_EVENT_KIND_MISPLACED", fname, raw.line,
                    f"§{raw.chapter} 에는 `kind: {kind}` 블록이 올 수 없다(템플릿 `블록:` 지시)")
        for chapter, d in allowed.items():
            if ctx is not None and d["condition"] is not None:
                try:
                    if not _condition_holds(d["condition"], ctx.facts)[0]:
                        continue
                except core.HintError:
                    continue                # 위 ③ 에서 이미 기재했다
            for kind, need in d["blocks"].items():
                have = sum(1 for r in events if r.chapter == chapter and not r.errors and r.event.get("kind") == kind)
                if have < need:
                    code = "HINT_JOURNEY_WALL_ABSENT" if kind == "wall" else "HINT_EVENT_BLOCK_COUNT"
                    add(code, fname, 1, f"§{chapter}: `kind: {kind}` 블록 {have}개 < {need}")
            have_ex = sum(1 for e in exs if e.chapter == chapter)
            if have_ex < d["excerpts"]:
                add("HINT_EXCERPT_REQUIRED", fname, 1, f"§{chapter}: 원문 발췌 {have_ex}개 < {d['excerpts']}")

    # ⑦ 여정 필수(계약 "필수는 여정 정보 하나") — 템플릿 지시와 무관하게 02 의 wall ≥ 1
    if "02-narrative" in docs and not any(f == "02-narrative.md" and not r.errors and r.event.get("kind") == "wall"
                                          for f, r in all_events):
        add("HINT_JOURNEY_WALL_ABSENT", "02-narrative.md", 1, "`kind: wall` 블록이 0개 — 여정이 없다")

    # ⑧ 사건 id 유일 · 출처 · 서명 대조
    seen: dict[str, tuple[str, int]] = {}
    verifier = substitute is not None
    for fname, raw in all_events:
        if raw.errors:
            continue
        ev = raw.event
        eid = ev.get("id")
        if eid in seen:
            add("HINT_EVENT_ID_DUPLICATE", fname, raw.line, f"id 중복: {eid}(처음 {seen[eid][0]}:{seen[eid][1]})")
        else:
            seen[eid] = (fname, raw.line)
        locs = []
        for entry in _sources_of(ev):
            loc = sources.locate(_source_token(entry))
            if loc is None:
                add("HINT_EVENT_SOURCE_OUTSIDE_LINEAGE", fname, raw.line,
                    f"{eid}: 출처 `{entry}` 가 LINEAGE 문서 · evidence_candidates · artifacts 밖이다(또는 모호)")
            elif sources.snapshot_problem(loc) is not None:
                add(sources.snapshot_problem(loc)[0], fname, raw.line, f"{eid}: {sources.snapshot_problem(loc)[1]}")
            else:
                locs.append(loc)
        sig = ev.get("서명")
        if isinstance(sig, str) and sig.strip() and ev.get("kind") in ("wall", "misdiagnosis"):
            if not verifier:
                add("HINT_EXCERPT_VERIFIER_ABSENT", fname, raw.line, f"{eid}: 치환 함수가 없어 서명을 대조할 수 없다")
                continue
            texts = [t for t in (sources.normsub(loc) for loc in locs) if t is not None]
            if not texts:
                add("HINT_SIGNATURE_UNVERIFIABLE", fname, raw.line, f"{eid}: 읽을 수 있는 출처가 없어 서명을 대조할 수 없다")
                continue
            for piece in _ELLIPSIS.split(sig):
                if not piece.strip():
                    continue
                if len(_WS.sub("", piece)) < MIN_SIGNATURE_CHARS:
                    # D-c: 짧은 조각은 원문 증거로 치지 않는다(어디에나 있는 부분 문자열) — 앞뒤 원문을 더 인용한다.
                    add("HINT_SIGNATURE_TOO_SHORT", fname, raw.line,
                        f"{eid}: 서명 조각이 {MIN_SIGNATURE_CHARS}자(공백 제외) 미만이라 원문 증거가 못 된다: {piece.strip()[:60]!r}")
                    continue
                frag = _norm(substitute(piece))
                if frag and not any(frag in t for t in texts):
                    add("HINT_SIGNATURE_MISMATCH", fname, raw.line,
                        f"{eid}: 서명 조각이 출처 원문에 없다 — 기억으로 재구성하지 않는다: {frag[:60]!r}")

    # ⑨ 원문 발췌 무결성 · 상한(X16)
    per_source: dict[str, int] = {}
    total = 0
    for fname, ex in all_excerpts:
        total += len(ex.body)
        loc = sources.locate(ex.stem)
        if loc is None:
            add("HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE", fname, ex.line, f"발췌 출처 `{ex.stem}` 가 LINEAGE · artifacts 밖이다(또는 모호)")
            continue
        per_source[loc[1]] = per_source.get(loc[1], 0) + len(ex.body)
        sp = sources.snapshot_problem(loc)
        if sp is not None:
            add(sp[0], fname, ex.line, sp[1])
            continue
        quoted = "\n".join(ex.body)
        if not _norm(quoted):
            add("HINT_EXCERPT_EMPTY", fname, ex.line, "발췌 본문이 비었다")
            continue
        if not verifier:
            add("HINT_EXCERPT_VERIFIER_ABSENT", fname, ex.line, "치환 함수가 없어 발췌 무결성을 검사할 수 없다")
            continue
        src = sources.normsub(loc)
        if src is None:
            add("HINT_EXCERPT_SOURCE_UNREADABLE", fname, ex.line, f"발췌 출처를 읽을 수 없다: {loc[1]}")
            continue
        want_q = _norm(substitute(quoted))
        if loc[0] != "snapshot" and loc[1].lower().endswith(_MD_SOURCE_EXTS):
            if not _section_in_headings(ex.section, sources.text(loc) or "", substitute):
                # D-c: 마크다운 출처의 `§<절>` 은 그 문서의 제목 줄이어야 한다(정규화 접두 일치). 본문만 맞고 절 표지가 틀리면 수신자가
                #   원문을 찾아가지 못한다(wave-1 리뷰: 절 표지는 검사되지 않았다).
                add("HINT_EXCERPT_SECTION_UNKNOWN", fname, ex.line,
                    f"발췌 머리 `§{ex.section}` 가 출처 `{loc[1]}` 의 제목 줄(`#`…)과 맞지 않는다 — 제목 줄의 앞부분 그대로 적는다")
        else:
            # 2026-09-22 S2 round 2(F8): 비-마크다운 출처의 절 표지 = 정규화 뒤 줄 범위 `§L<a>-<b>`(`hint.py excerpt --lines a-b` 와 같은
            #   번호). 인용은 그 줄들 **안에** 있어야 하고, 범위는 인용 줄 수 + LINE_LABEL_SLACK 을 넘지 않는다(넓은 범위는 자리를 못 가리킨다).
            lm = _LINE_LABEL.fullmatch(ex.section.strip())
            lines_s = sources.sublines(loc) or []
            if not lm:
                add("HINT_EXCERPT_SECTION_SHAPE", fname, ex.line,
                    f"비-마크다운 출처 `{loc[1]}` 의 발췌 머리는 `§L<a>-<b>`(줄 범위)다: `§{ex.section}` — `hint.py excerpt --lines a-b` 번호")
            else:
                a, b = int(lm.group(1)), int(lm.group(2))
                if a < 1 or b < a or b > len(lines_s):
                    add("HINT_EXCERPT_LINES_OUT_OF_RANGE", fname, ex.line,
                        f"`§L{a}-{b}` 가 출처 `{loc[1]}`(정규화 뒤 {len(lines_s)}줄)의 범위 밖이거나 거꾸로다")
                elif b - a + 1 > len(ex.body) + LINE_LABEL_SLACK:
                    add("HINT_EXCERPT_LINES_TOO_WIDE", fname, ex.line,
                        f"`§L{a}-{b}`({b - a + 1}줄)가 인용 {len(ex.body)}줄 + {LINE_LABEL_SLACK} 보다 넓다 — 인용한 줄만 가리킨다")
                elif want_q not in _norm("\n".join(lines_s[a - 1:b])) and want_q in src:
                    add("HINT_EXCERPT_OUTSIDE_LINES", fname, ex.line,
                        f"인용은 출처 `{loc[1]}` 에 있으나 `§L{a}-{b}` 줄 안이 아니다 — `hint.py excerpt --lines` 번호로 고친다")
        if want_q not in src:
            add("HINT_EXCERPT_MISMATCH", fname, ex.line,
                f"발췌가 출처 `{loc[1]}`(정규화 · 치환 후)의 부분 문자열이 아니다 — 원문 그대로 옮긴다(ANSI · `\\r` 조각은 정규화가 지운다)")
    for src_key, count in sorted(per_source.items()):
        if count > MAX_EXCERPT_LINES_PER_SOURCE:
            add("HINT_EXCERPT_SOURCE_LIMIT", "-", 1, f"`{src_key}` 발췌 {count}줄 > {MAX_EXCERPT_LINES_PER_SOURCE}")
    if total > MAX_EXCERPT_LINES_TOTAL:
        add("HINT_EXCERPT_TOTAL_LIMIT", "-", 1, f"발췌 합 {total}줄 > {MAX_EXCERPT_LINES_TOTAL}")

    # ⑩ value-status 커버리지(옛 B1 "증류, 복붙 ✗" 원자의 대체) — 1.3 표의 노브마다 블록 정확히 1개 · 값 글자 그대로
    if facts is not None:
        cands = {_knob(c): _value_text(c.get("value")) for c in _candidates(facts)}
        if not cands and not (_missing_codes(facts.get("missing")) & set(_TRIPLET_ABSENT_CODES)):
            # 후보 0개면 아래 커버리지는 공허하게 통과한다 — "증류, 복붙 ✗" 원자의 유일한 집행자가 조용히 꺼진다.
            add("HINT_VALUE_STATUS_CANDIDATES_ABSENT", "01-artifacts.md", 1,
                "서빙 노브 후보 0개인데 트리플렛 부재(`HINT_MISSING_TRIPLET`)가 기재되지 않았다 — value-status 커버리지를 "
                "판정할 수 없다(artifacts.value_status_candidates 가 서빙 yaml 을 읽었는지 확인한다)")
        blocks: dict[str, list[tuple[str, _RawEvent]]] = {}
        for fname, raw in all_events:
            if not raw.errors and raw.event.get("kind") == "value-status":
                blocks.setdefault(str(raw.event.get("노브")).strip(), []).append((fname, raw))
        for knob, want_value in cands.items():
            got_b = blocks.get(knob, [])
            if not got_b:
                add("HINT_VALUE_STATUS_GAP", "01-artifacts.md", 1, f"서빙 노브 `{knob}` 의 value-status 블록이 없다")
                continue
            if len(got_b) > 1:
                add("HINT_VALUE_STATUS_DUPLICATE", got_b[1][0], got_b[1][1].line, f"노브 `{knob}` 의 value-status 블록이 둘 이상")
            v = got_b[0][1].event.get("값")
            if _value_text(v) != want_value:
                add("HINT_VALUE_STATUS_VALUE_MISMATCH", got_b[0][0], got_b[0][1].line,
                    f"노브 `{knob}` 값 {v!r} ≠ 표의 값 {want_value!r}(글자 그대로 적는다)")
        for knob, got_b in sorted(blocks.items()):
            if knob not in cands:
                add("HINT_VALUE_STATUS_UNKNOWN_KNOB", got_b[0][0], got_b[0][1].line, f"1.3 표에 없는 노브: `{knob}`")

    # ⑩a declared-requirement 의 근거(2026-09-22 S2 round 2 F9 · VALUE_STATUSES 뜻풀이와 짝) — "빼면 깨진다" 는 관측된 실패가 있어야
    #     한다: 이 페이로드의 wall id(W<n>)를 근거·출처에 적거나, 그 실패를 기록한 artifacts/ 파일을 출처로 단다. yaml 주석의 '필수'
    #     한 단어는 근거가 아니다(1차: 주석 단어로 기계 후보가 declared-requirement 가 됐다).
    wall_ids = {str(r.event.get("id")) for _, r in all_events if not r.errors and r.event.get("kind") == "wall"}
    for fname, raw in all_events:
        if raw.errors or raw.event.get("kind") != "value-status" or raw.event.get("지위") != "declared-requirement":
            continue
        ev = raw.event
        text = " ".join([str(ev.get("근거") or "")] + _sources_of(ev))
        cited_w = set(re.findall(r"(?<![A-Za-z0-9])W\d+(?![0-9])", text)) & wall_ids
        # 트리플렛(`artifacts/triplet/` — 그 값을 **선언한** 서빙 yaml · env · 러너)은 근거가 아니다(2026-09-22 적대 리뷰): 선언이
        #   선언을 증명하지 못한다. 1판은 artifacts 파일이면 무엇이든 받아, 1차 함정(`async-scheduling` 을 그 yaml 의 '명시 필수'
        #   주석으로 필요조건화)을 yaml 을 출처로 달기만 하면 그대로 통과시켰다.
        art_src = [e for e in _sources_of(ev) if _is_requirement_artifact(sources.locate(_source_token(e)))]
        if not cited_w and not art_src:
            add("HINT_VALUE_STATUS_REQUIREMENT_UNGROUNDED", fname, raw.line,
                f"{ev.get('id')}: declared-requirement 인데 관측된 실패가 없다 — 이 페이로드의 wall id(W<n>)를 근거에 적거나 실패를 "
                "기록한 artifacts/ 파일(트리플렛 밖 — 값을 선언한 파일 자신은 근거가 아니다)을 출처로 단다(없으면 inherited)")

    # ⑩c 적용 판정 결과 출처 어휘(artifacts 와 짝 · 닫힌 목록) — 뜻 없는 라벨은 수신자에게 아무것도 말하지 않는다
    if facts is not None:
        for pt in (_dict(facts, "applied_set").get("patches") or []):
            rs = pt.get("result_source") if isinstance(pt, dict) else None
            if rs and rs not in RESULT_SOURCE_MEANINGS:
                add("HINT_RESULT_SOURCE_UNKNOWN", "01-artifacts.md", 1,
                    f"적용 판정 결과 출처 `{rs}` 가 template.RESULT_SOURCE_MEANINGS 에 없다({pt.get('file')}) — 어휘를 맞춘다")

    # ⑩b 결손 코드 어휘(SPEC §2.1: missing[] 는 evidence.MISSING_CODES 사전에서만) — 미등록 코드는 배포 본문에
    #     "미등록 사유코드" 로 실린다. 뜻 없는 코드는 수신자에게 아무것도 말하지 않는다.
    if facts is not None and _missing_codes(facts.get("missing")):
        try:
            registered = set(_missing_meanings())
        except core.HintError as e:
            add(e.code, "00-hint.md", 1, e.message)
        else:
            for code in sorted(_missing_codes(facts.get("missing")) - registered):
                add("HINT_MISSING_CODE_UNREGISTERED", "00-hint.md", 1,
                    f"결손 코드 `{code}` 가 evidence.MISSING_CODES 에 없다(코드표에 뜻을 등재하거나 evidence 가 쓰는 코드를 고친다)")

    # ⑪ 파생 블록 최신성(refresh 뒤 lint)
    if "00-hint" in docs:
        texts = {n: docs[n].text if n in docs else "" for n in ("01-artifacts", "02-narrative")}
        want_d = _derive_from_texts(texts["02-narrative"], texts["01-artifacts"])
        for fid in DERIVED_FACTS:
            got_f = docs["00-hint"].fact(fid)
            if got_f and got_f[0].body.strip("\n") != want_d[fid].strip("\n"):
                add("HINT_DERIVED_BLOCK_STALE", "00-hint.md", got_f[0].start + 1,
                    f"파생 블록 FACT:{fid} 가 현재 hint-event 와 다르다 — `refresh` 를 먼저 부른다")

    # ⑫ 03 벤치 표 재렌더 diff 0(render_bench_section 단일 소유 · K6)
    if "03-benchmark" in docs:
        got_b = docs["03-benchmark"].fact(BENCH_REGION_ID)
        body = got_b[0].body.strip("\n") if got_b else ""
        if bench_report is not None:
            try:
                expected_b = _render_bench_source(repo, bench_report).strip("\n")
                # CLI `render_bench_section --verify --section 03-benchmark.md` 와 **같은 커널**로 한 번 더 자른다 —
                # 린터가 통과시킨 페이로드를 그 CLI 가 FAIL 시키는 일이 다시 없게(2026-09-22 리뷰: FACT 닫힘 표지 사고).
                cli_cut = _bench_module(repo).extract_section(docs["03-benchmark"].text, expected_b.split("\n")[0].strip())
            except Exception as e:  # BenchSectionFailure · OSError · ValueError — 원인을 삼키지 않고 기재한다
                add("HINT_BENCH_SECTION_UNVERIFIABLE", "03-benchmark.md", 1, f"{type(e).__name__}: {e}")
            else:
                if body != expected_b:
                    add("HINT_BENCH_SECTION_DRIFT", "03-benchmark.md", got_b[0].start + 1 if got_b else 1,
                        f"부하 곡선이 원천(`{Path(str(bench_report)).name}`) 재렌더와 다르다")
                elif cli_cut is None or cli_cut.strip() != expected_b.strip():
                    add("HINT_BENCH_SECTION_DRIFT", "03-benchmark.md", got_b[0].start + 1 if got_b else 1,
                        "render_bench_section --verify 의 절 추출(첫 제목 줄 ~ 다음 `## `)이 재렌더와 다르다 — 같은 절 제목 "
                        "줄이 문서 앞쪽에 한 번 더 있다(CLI 는 첫 줄을 잡는다)")
        elif facts is not None and isinstance(facts.get("bench_section_md"), str) and facts["bench_section_md"].strip():
            add("HINT_BENCH_SECTION_UNVERIFIABLE", "03-benchmark.md", 1, "부하 곡선이 실렸는데 재렌더할 원천(bench_report)이 주어지지 않았다")

    # ⑬ 등급 마커(배포되는 00 본문) · 배너
    text00 = docs["00-hint"].visible() if "00-hint" in docs else ""
    tc_m = (manifest or {}).get("task_class") if isinstance(manifest, dict) else None
    tc_f = _task_class(facts) if facts is not None else None
    # 등급 마커는 facts 에서 기계 렌더된다(FACT:grade). manifest 와 facts 가 갈리면(facts 쪽 누락 포함) 마커가 기계 블록에
    # 실리지 않고 손으로만 채워진다 — 조립기(facts)의 누락을 여기서 드러낸다.
    if tc_m and facts is not None and tc_m != tc_f:
        add("HINT_TASK_CLASS_CONFLICT", "00-hint.md", 1, f"manifest task_class {tc_m!r} ≠ facts {tc_f!r}")
    if "hint_map_only" in (tc_m, tc_f) and MAP_ONLY_MARKER not in text00:
        add("HINT_MAP_ONLY_OBSERVATION_MARKER_MISSING", "00-hint.md", 1,
            f"task_class=hint_map_only 인데 00 본문에 `{MAP_ONLY_MARKER}` 가 없다(관측 게재 — baseline·권고 ✗)")
    bench_m = _dict(manifest, "benchmark") if isinstance(manifest, dict) else {}
    waiver_m = bench_m.get("perf_waiver") if isinstance(bench_m.get("perf_waiver"), dict) and bench_m.get("perf_waiver") \
        else None
    waiver_f = _perf_waiver(facts) if facts is not None else None
    if facts is not None and isinstance(manifest, dict) and (
            bool(waiver_m) != bool(waiver_f)
            or (waiver_m and waiver_f and str(waiver_m.get("warning_flag") or "").strip()
                != str(waiver_f.get("warning_flag") or "").strip())):
        add("HINT_PERF_WAIVER_CONFLICT", "00-hint.md", 1,
            "manifest.benchmark.perf_waiver 와 facts.perf_waiver 가 다르다 — PERF-WARNING 은 facts 에서 기계 렌더된다")
    waiver = waiver_m or waiver_f
    if waiver:
        flag = str(waiver.get("warning_flag") or "").strip()
        if PERF_WARNING_MARKER not in text00 or (flag and any(ln.strip() not in text00 for ln in flag.splitlines())):
            add("HINT_PERF_WARNING_MISSING", "00-hint.md", 1,
                f"perf_waiver 가 있는데 00 본문에 `{PERF_WARNING_MARKER}` + warning_flag 원문이 없다(경고 없는 waiver 는 우회다)")
    if "00-hint" in docs:
        d00 = docs["00-hint"]
        visible = "\n".join(line for i, line in enumerate(d00.lines) if not d00.masked[i])
        for b in BANNER_LINES:              # 고정 문구 = 주석·사실 블록 밖(seal 이 PROMPT 를 치환하므로 안에 두면 사라진다)
            if b not in visible:
                add("HINT_BANNER_ABSENT", "00-hint.md", 1, f"필수 배너 부재(PROMPT·주석 밖 고정 문구): {b}")
        try:
            btext = brief(payload_dir)
        except core.HintError as e:
            add(e.code, "00-hint.md", 1, e.message)
        else:
            # 2026-09-22 S2 round 2(F9): brief 는 annotation · 카탈로그로 나가는 가장 눈에 띄는 세 줄이다. 그 수치는 이 페이로드의 사실
            #   블록(03 표 포함)에 **글자 그대로** 있어야 한다 — 단위 환산(20480MiB ← 21474836480) · 반올림 · 다른 셀의 수치는 새 수치다.
            known = _fact_numbers(docs)
            for tok in _brief_numbers(btext):
                if tok not in known:
                    add("HINT_BRIEF_NUMBER_UNGROUNDED", "00-hint.md", 1,
                        f"brief 수치 `{tok}` 가 사실 블록(FACT · 03 표)에 없다 — 표의 값을 글자 그대로 쓰거나 수치를 빼고 V/W id 로 가리킨다")

    # ⑭ README 재생성 diff 0
    readme = payload_dir / README_NAME
    if not readme.is_file():
        add("HINT_DOCUMENT_ABSENT", README_NAME, 1, "README.md 부재")
    elif ctx is not None:
        try:
            want_readme = _render_readme(ctx)
        except core.HintError as e:
            add(e.code, README_NAME, 1, e.message)
        else:
            if readme.read_text(encoding="utf-8") != want_readme:
                add("HINT_README_DRIFT", README_NAME, 1, "README.md 가 형식 버전 렌더와 다르다(손으로 고치지 않는다)")

    # ⑮ PII(deploy 프로필 · 4종 + 리터럴) — 배포되는 텍스트 전부
    from . import pii as _pii
    terms = _pii.load_pii_terms(repo) if pii_terms is _UNSET else pii_terms
    if terms is None:
        add("HINT_MISSING_PII_TERMS", "-", 1, f"`{core.REL_PII_TERMS}` 부재 — 배포 PII 스캔은 리터럴 없이 통과시키지 않는다")
    for name in [*PAYLOAD_DOCS, README_NAME]:
        p = payload_dir / name
        if p.is_file():
            for hit in _pii.scan_text(p.read_text(encoding="utf-8"), terms or [], profile="deploy"):
                add("HINT_PII", name, hit.line, f"배포 PII 검출(pattern={hit.pattern}) — 발췌는 치환 후 원문을 옮긴다")

    # ⑯ 태그 이름은 사실 블록이 소유한다(2026-09-29 · plan_26092908 §4.1 · D 통합 결정 3): 꼬리는 continue 가 확정하므로 저작 산문 ·
    #     hint-event 에 적은 이름은 확정 전 이름으로 남는다(기본 이름이 최종 이름의 접두라 어느 쪽을 적어도 여기서 잡힌다).
    base = facts.get("base_tag") if facts is not None else None
    if isinstance(base, str) and base.startswith(core.HINT_TAG_PREFIX):
        for name, doc in docs.items():
            for i, line in enumerate(doc.lines):
                if not doc.masked[i] and base in line:
                    add("HINT_TAG_LITERAL_IN_PROSE", doc.name, i + 1,
                        "저작 영역에 태그 이름 리터럴이 있다 — 이름은 사실 블록(00 §0.1 머리 · §0.9)이 소유한다(꼬리 확정 전 이름이 남는다)")

    # ⑰ 파일 단위 검증 표시 3자리 일치(2026-09-29 · plan_26092908 §4.2 U1+ · R3 "미검증 파일을 검증된 것으로 오인"): ① PAYLOAD 슬롯 파일 기록
    #     (facts.slots[*].file_records — 01 표도 같은 facts 에서 렌더되므로 ③ 01 `검증` 열은 FACT 재생성 diff 가 함께 묶는다) ② 파일 첫 줄.
    if facts is not None:
        for _slot, rec in _file_records(_dict(facts, "slots")):
            for msg in _mark_problems(payload_dir, rec):
                add("HINT_VERIFICATION_MARK_MISMATCH", rec["path"], 1, msg)
        # ⑱ Agent 저작 요청(렌더러로 못 만든 자리): 첫 줄 경고 유지 · 자리표시 잔존 ✗ · 빈 본문 ✗
        for req in facts.get("agent_requests") or []:
            if isinstance(req, dict) and req.get("path"):
                for code, msg in agent_request_problems(payload_dir, req):
                    add(code, str(req["path"]), 1, msg)

    uniq = sorted(set(issues), key=lambda x: (x[1], x[2], x[0], x[3]))
    return [{"code": c, "file": f, "line": ln, "message": m} for c, f, ln, m in uniq]


def _mark_problems(payload_dir: Path, rec: dict) -> list[str]:
    """파일 기록 1건의 검증 표시 대조(순수 읽기). generated-unverified = 기록된 헤더(줄 · JSON 키)가 파일에 그대로 있고 문구가
    `⚠ generated-unverified … · 생성: <generated_by>` · verified = 파일 머리 3줄에 경고 표시가 없다."""
    path = Path(payload_dir) / str(rec.get("path"))
    if not path.is_file():
        return [f"파일 기록이 있는데 페이로드에 파일이 없다: {rec.get('path')}"]
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as e:
        return [f"파일을 읽을 수 없다({type(e).__name__}) — 검증 표시를 대조할 수 없다"]
    lines = text.split("\n")
    ver, gen, hdr = rec.get("verification"), rec.get("generated_by"), rec.get("header")
    if ver == "generated-unverified":
        if not isinstance(hdr, dict) or not isinstance(hdr.get("text"), str):
            return ["generated-unverified 인데 파일 기록에 header 가 없다(표시 3자리 중 파일 첫 줄 부재)"]
        want = hdr["text"]
        if UNVERIFIED_MARK not in want or f"· 생성: {gen}" not in want:
            return [f"header 문구가 기록(verification={ver} · generated_by={gen})과 다르다: {want[:80]!r}"]
        if hdr.get("style") == "json-key":
            try:
                got = json.loads(text).get(str(hdr.get("key")))
            except (ValueError, AttributeError):
                got = None
            return [] if got == want else [f"JSON 키 `{hdr.get('key')}` 의 경고가 기록과 다르다: {str(got)[:60]!r}"]
        ln = hdr.get("line")
        if not isinstance(ln, int) or not (1 <= ln <= len(lines)) or lines[ln - 1].strip() != want.strip():
            return [f"파일 {ln}행이 기록된 경고 줄과 다르다 — 경고를 지우거나 옮기지 않는다"]
        return []
    if any(UNVERIFIED_MARK in ln for ln in lines[:3]):
        return [f"verification={ver} 인데 파일 머리에 `{UNVERIFIED_MARK}` 경고가 있다(기록과 파일이 갈렸다)"]
    return []


def _comment_line(name: str, text: str) -> str:
    """파일 형식에 맞는 한 줄 주석(Agent 저작 요청 자리 · artifacts._insert_header 와 같은 관용 — md 는 HTML 주석)."""
    return f"<!-- {text} -->" if name.endswith((".md", ".markdown", ".html")) else f"# {text}"


def agent_request_header_line(req: dict) -> str:
    return _comment_line(str(req.get("name") or req.get("path")), str(req.get("header") or ""))


def agent_request_scaffold(req: dict) -> str:
    """Agent 저작 요청 파일의 스캐폴드(2026-09-29 · plan_26092908 §4.2 "렌더러 우선 · Agent 폴백"): 첫 줄 = artifacts 가 준 경고
    (`⚠ generated-unverified — … · 생성: agent`) · 둘째 줄 = 자리표시(`<<AGENT:` — 봉인 전 린터가 잔존을 막는다)."""
    inputs = " · ".join(str(x) for x in (req.get("inputs") or [])) or "PAYLOAD · artifacts"
    return (agent_request_header_line(req) + "\n" + f"{AGENT_MARK} {req.get('why')} — 입력: {inputs}. 이 줄을 파일 본문으로 바꿔라"
            "(증거에서 옮긴 것만 · 실행해 보지 않은 절차를 검증된 것처럼 쓰지 않는다 · 첫 줄 경고는 지우지 않는다)>>\n")


def write_agent_requests(payload_dir: Path, reqs) -> list[str]:
    """스캐폴드 쓰기(publish 전용 · 기존 파일은 덮지 않는다 — refacts 가 저작본을 보존한다). 반환 = 새로 쓴 페이로드 상대 경로."""
    out = []
    for req in reqs or []:
        if not isinstance(req, dict) or not req.get("path") or not req.get("header"):
            continue
        rel = str(req["path"])
        if rel.startswith("/") or ".." in rel.split("/") or not rel.startswith("artifacts/"):
            core.fail("HINT_AGENT_REQUEST_PATH", f"Agent 저작 요청 경로가 artifacts/ 상대가 아니다: {rel!r}")
        if rel.endswith(".json"):
            core.fail("HINT_AGENT_REQUEST_PATH", f"JSON 파일은 주석 경고를 달 수 없다 — Agent 저작 요청으로 받지 않는다: {rel}")
        dst = Path(payload_dir) / rel
        if dst.exists():
            continue
        dst.parent.mkdir(parents=True, exist_ok=True)
        dst.write_text(agent_request_scaffold(req), encoding="utf-8")
        out.append(rel)
    return out


def agent_request_problems(payload_dir: Path, req: dict) -> list[tuple[str, str]]:
    """Agent 저작 파일 1건 판정 → [(code, 메시지)]. 부재 = HINT_AGENT_ARTIFACT_ABSENT · 경고 줄(첫 줄 · shebang 이면 둘째 줄) 불일치 =
    HINT_VERIFICATION_MARK_MISMATCH · 자리표시 잔존 = HINT_AGENT_PLACEHOLDER_RESIDUE · 경고 밖 본문 없음 = HINT_AGENT_ARTIFACT_UNAUTHORED."""
    p = Path(payload_dir) / str(req.get("path"))
    if not p.is_file():
        return [("HINT_AGENT_ARTIFACT_ABSENT", "Agent 저작 요청 파일이 페이로드에 없다(publish 가 스캐폴드를 둔다 — 지우지 않는다)")]
    try:
        lines = p.read_text(encoding="utf-8").split("\n")
    except (OSError, UnicodeDecodeError) as e:
        return [("HINT_AGENT_ARTIFACT_ABSENT", f"읽을 수 없다({type(e).__name__})")]
    out: list[tuple[str, str]] = []
    hi = 1 if lines and lines[0].startswith("#!") else 0
    want = agent_request_header_line(req)
    if len(lines) <= hi or lines[hi].strip() != want.strip():
        out.append(("HINT_VERIFICATION_MARK_MISMATCH", f"{hi + 1}행이 경고 줄 `{want[:60]}…` 이 아니다 — 첫 줄 경고는 지우지 않는다"))
    if any(AGENT_MARK in ln for ln in lines):
        out.append(("HINT_AGENT_PLACEHOLDER_RESIDUE", f"미저작 자리표시 `{AGENT_MARK}` 잔존"))
    body = [ln for i, ln in enumerate(lines) if ln.strip() and i != hi and not (i == 0 and hi == 1) and AGENT_MARK not in ln]
    if not body:
        out.append(("HINT_AGENT_ARTIFACT_UNAUTHORED", "경고 줄 밖 본문이 없다 — 증거에서 파일 본문을 쓴다"))
    return out


def cited_sources(repo: Path, payload_dir: Path, *, lineage: dict, draft_dir: Path | None = None) -> dict:
    """페이로드가 인용한 **계보 출처**(읽기 전용 · 2026-09-29 plan_26092908 §4.6·§4.9) — `{"excerpt_counts": {경로: 발췌 수}, "paths": [경로…]}`.
    발췌 머리 · hint-event `출처` 가 LINEAGE(문서 · 후보)로 해소되는 것만 센다(artifacts 는 봉인 트리 안 · 도구 스냅샷은 git 이 바이트를 든다).
    소비자 = hint.py continue(봉인 출처 스냅샷 · LINEAGE 수신자 요약의 발췌 수). 해소기는 린터와 같다(`_Sources.locate`)."""
    from . import lineage as _lin
    src = _Sources(Path(repo), Path(payload_dir), lineage or {}, _lin.resolve_stem, lambda rel: None, lambda t: t,
                   draft_dir=draft_dir)
    counts: dict[str, int] = {}
    paths: set[str] = set()
    for name in PAYLOAD_DOCS:
        p = Path(payload_dir) / name
        if not p.is_file():
            continue
        events, exs, _ = _scan(_Doc.parse(name, p.read_text(encoding="utf-8")))
        for e in exs:
            loc = src.locate(e.stem)
            if loc and loc[0] == "lineage":
                counts[loc[1]] = counts.get(loc[1], 0) + 1
                paths.add(loc[1])
        for r in events:
            if r.errors:
                continue
            for entry in _sources_of(r.event):
                loc = src.locate(_source_token(entry))
                if loc and loc[0] == "lineage":
                    paths.add(loc[1])
    return {"excerpt_counts": counts, "paths": sorted(paths)}


def _num_key(tok: str) -> str:
    return tok.replace(",", "").rstrip(".")


def _brief_numbers(text: str) -> list[str]:
    """brief 에서 대조할 수치(소수 · 세 자리 이상 정수 · 단위만 붙은 것). 식별자 조각 · 약칭(`262k`)은 건너뛴다."""
    out: list[str] = []
    for m in _BRIEF_NUM.finditer(text or ""):
        tok, unit = m.group(1).rstrip(".,"), m.group(2)
        key = _num_key(tok)
        if tok.count(".") > 1:
            continue                                  # 버전 모양(0.29.0)은 수치가 아니다 — 빌드 좌표는 0.4 표가 싣는다
        # 소수는 뒤에 글자가 붙어도 수치다(`38.8t/s` · `2.5x` — 2026-09-22 적대 리뷰: 1판은 단위 목록 밖 글자가 붙은 소수를 건너뛰어
        #   반올림 수치가 `t/s` 를 붙이기만 하면 통과했다). 정수만 닫힌 단위 목록으로 식별자 · 약칭(`262k` · `85cef…`)을 가른다.
        if "." not in key and unit not in _NUM_UNITS:
            continue
        if "." in key or len(key) >= 3:
            out.append(key)
    return out


def _fact_numbers(docs: dict) -> set[str]:
    """**기계** 사실 블록(부하 곡선 영역 포함) 본문의 모든 수치 토큰(쉼표 제거). brief 대조의 허용 집합이다.
    파생 블록(00 §0.3 wall_map · §0.5 knob_status)은 뺀다 — 저작자가 hint-event 에 쓴 증상 · 해소 · 근거를 옮긴 표라, 넣으면 저작자가
    쓴 수치가 스스로를 근거 짓는다(2026-09-22 적대 리뷰: 1판은 wall 블록 `증상` 에 적은 수치로 brief 수치를 통과시켰다)."""
    out: set[str] = set()
    for d in docs.values():
        for f in d.facts:
            if f.fid in DERIVED_FACTS:
                continue
            # 토큰 전체만 넣는다 — `2.5932953826691967` 의 앞 조각(2.59) 같은 부분 일치를 허용하면 반올림이 통과한다.
            out.update(_num_key(m.group(0)) for m in _ANY_NUM.finditer(f.body))
    return out


def render_bench_source(repo: Path, source) -> str:
    """03 부하 곡선 절을 **그 원천**(bench_report md · sweep_index json)에서 렌더한다 — publish 가 facts.bench_section_md 를
    만들 때와 lint 가 재렌더할 때 **같은 함수**다(원천 경로는 hint.py 가 state.json 의 bench_source 로 남긴다)."""
    return _render_bench_source(Path(repo), source)


def update_facts(repo: Path, payload_dir: Path, changes: dict, *, snapshot_path: Path | None = None) -> list[str]:
    """facts 스냅샷에 `changes` 를 덮어쓰고, 그 사실로 렌더되는 **기계 사실 블록만** 다시 쓴다(2026-09-22 통합 · continue 가
    승인 기록·슬롯 적용 사유를 반영할 때). 저작 영역·PROMPT·봉인 질문 줄·파생 블록(wall_map·knob_status)은 건드리지 않는다 —
    사실 블록은 기계 소유라 다시 쓰는 것이 D2 경계("기계 산출은 기계가") 그대로다. 반환 = 다시 쓴 사실 블록 id(정렬).
    바뀐 것이 없으면 아무것도 쓰지 않는다(멱등 — 재실행 = 확인만)."""
    repo, payload_dir = Path(repo), Path(payload_dir)
    snap = Path(snapshot_path) if snapshot_path else default_snapshot_path(payload_dir)
    if not snap.is_file():
        core.fail("HINT_FACT_SNAPSHOT_ABSENT", f"facts 스냅샷 부재: {snap}", "hint.py publish 가 만든 드래프트에서 실행한다.")
    old = _normalize_facts(load_snapshot(snap))
    if not isinstance(changes, dict):
        core.fail("HINT_FACTS_SHAPE", "changes 는 dict 여야 한다")
    new = _normalize_facts({**old, **changes})
    if new == old:
        return []
    templates = _load_templates(repo)
    ctx = _Ctx(repo, new, templates)
    writes: dict[Path, str] = {}
    changed: list[str] = []
    for name in TEMPLATE_FILES:
        p = payload_dir / f"{name}.md"
        if not p.is_file():
            core.fail("HINT_DOCUMENT_ABSENT", f"사실 블록을 다시 쓸 문서 부재: {name}.md")
        doc = _Doc.parse(name, p.read_text(encoding="utf-8"))
        lines = list(doc.lines)
        edits = []
        for f in doc.facts:
            if f.fid in DERIVED_FACTS or f.fid not in FACT_RENDERERS:
                continue
            want = FACT_RENDERERS[f.fid](ctx)
            if f.body.strip("\n") == want.strip("\n"):
                continue
            body = want.split("\n") if want else []
            edits.append((f.start + 1, f.end, [""] + body + [""] if f.open_ended else body))
            changed.append(f.fid)
        if edits:
            for start, end, body in sorted(edits, key=lambda e: e[0], reverse=True):
                lines[start:end] = body
            writes[p] = "\n".join(lines)
    readme = payload_dir / README_NAME
    want_readme = _render_readme(ctx)
    if readme.is_file() and readme.read_text(encoding="utf-8") != want_readme:
        writes[readme] = want_readme
        changed.append("README")
    for p, text in writes.items():                   # 전부 계산한 뒤에 쓴다(반쪽 갱신 ✗)
        p.write_text(text, encoding="utf-8")
    core.write_json(snap, new)
    return sorted(set(changed))


def _render_bench_source(repo: Path, source) -> str:
    """render_bench_section 의 공개 함수로 원천을 렌더한다(CLI `build_from_args` 와 같은 우선순위 · 출처 이름 = 파일명)."""
    mod = _bench_module(repo)
    p = Path(source)
    p = p if p.is_absolute() else repo / p
    text = p.read_text(encoding="utf-8")
    if p.suffix == ".json":
        return mod.render(mod.parse_sweep_index(json.loads(text)), p.name)
    if mod.is_lite_report(text):
        return mod.render_lite(mod.parse_lite_report(text), p.name)
    return mod.render(mod.parse_report(text), p.name)


# ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
def selftest() -> list[str]:
    """실제 경로(render → 저작 → refresh → lint → seal → lint)를 격리 임시 저장소에서 돈다. 라이브 태그·hint 브랜치·
    캠페인 비의존. 템플릿과 render_bench_section 은 이 스킬의 파일을 임시 저장소로 복사해 쓴다(그 파일들이 계약이다)."""
    import shutil
    import tempfile

    bad: list[str] = []
    ran = [0]

    def ck(name: str, cond) -> None:
        ran[0] += 1
        if not cond:
            bad.append(f"template: {name}")

    def codes(issues) -> set[str]:
        return {i["code"] for i in issues}

    def raises(fn, code: str) -> bool:
        try:
            fn()
        except core.HintError as e:
            return e.code == code
        return False

    # 제한 YAML 파서 단위(★음성대조 포함)
    ok_md = ("```hint-event\nid: W1   # 주석\nkind: wall\n증상: 로드 실패\n서명: \"Err: a # b, c\"\n원인: 원인\n해소: 해소\n"
             "검증: doc §1\n전이등급: arch-scaled\n출처: [doc_a §1, 'x, y.sh']\n```\n")
    evs = parse_hint_events(ok_md)
    ck("YAML 파싱", len(evs) == 1 and evs[0]["id"] == "W1" and evs[0]["서명"] == "Err: a # b, c"
       and evs[0]["출처"] == ["doc_a §1", "x, y.sh"] and evs[0]["_line"] == 1)
    ck("★YAML 중첩 줄 차단", raises(lambda: parse_hint_events(ok_md.replace("원인: 원인", "원인:\n  - 중첩")), "HINT_EVENT_INVALID"))
    ck("★YAML 매핑 값 차단", raises(lambda: parse_hint_events(ok_md.replace("원인: 원인", "원인: {a: 1}")), "HINT_EVENT_INVALID"))
    ck("★YAML 중복 키 차단", raises(lambda: parse_hint_events(ok_md.replace("원인: 원인", "원인: 원인\n원인: 둘")), "HINT_EVENT_INVALID"))
    ck("★YAML 닫히지 않은 따옴표", raises(lambda: parse_hint_events(ok_md.replace('"Err: a # b, c"', '"Err')), "HINT_EVENT_INVALID"))
    ck("★필수 필드 부재(파서)", raises(lambda: parse_hint_events(ok_md.replace("원인: 원인\n", "")), "HINT_EVENT_FIELD_MISSING"))
    ck("★kind 어휘 밖", raises(lambda: parse_hint_events(ok_md.replace("kind: wall", "kind: wal")), "HINT_EVENT_KIND_UNKNOWN"))
    ck("★전이등급 어휘 밖", raises(lambda: parse_hint_events(ok_md.replace("arch-scaled", "scaled")), "HINT_EVENT_VOCAB"))
    ck("★id 모양", raises(lambda: parse_hint_events(ok_md.replace("id: W1", "id: V1")), "HINT_EVENT_ID_SHAPE"))
    ck("주석 안 예시는 사건 아님", parse_hint_events("<!-- PROMPT\n질문: q\n```hint-event\nid: <x>\n```\n-->\n") == [])
    ck("`패치#60` 은 주석 아님", _parse_value("패치#60 적용")[0] == "패치#60 적용")
    ck("결손 0 명시", "없음" in _missing_block([]) and "미등록" in _missing_block(["HINT_X"], {}))

    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        repo = root / "repo"
        (repo / core.REL_SKILL).mkdir(parents=True)
        shutil.copytree(core.TEMPLATES_DIR, repo / core.REL_SKILL / "templates")
        (repo / core.REL_SCRIPTS).mkdir(parents=True)
        shutil.copy2(core.SCRIPTS_DIR / "render_bench_section.py", repo / core.REL_RENDER_BENCH_SECTION)
        term = "zq-fixture-operator"
        (repo / ".claude").mkdir(exist_ok=True)
        (repo / core.REL_PII_TERMS).write_text("# fixture — 실물 pii_terms 를 복사하지 않는다\n" + term + "\n", encoding="utf-8")

        # 템플릿 트립와이어: C3 배너가 PROMPT 밖 고정 문구 · 챕터 = plan §4.2 · 지시 해석 가능
        tpls = _load_templates(repo)
        t00 = tpls["00-hint"]
        vis = "\n".join(l for i, l in enumerate(t00.lines) if not t00.masked[i])
        ck("템플릿 00 에 배너 5줄(PROMPT 밖)", all(b in vis for b in BANNER_LINES))
        ck("템플릿 02 §2.2 wall>=1", _directives({p.chapter: p for p in tpls["02-narrative"].prompts}["2.2"].fields)["blocks"] == {"wall": 1})
        ck("템플릿 01 §1.5 조건", _directives({p.chapter: p for p in tpls["01-artifacts"].prompts}["1.5"].fields)["condition"] == ("topology", "multi"))
        ck("템플릿 PROMPT 수(00 = 0.9 이름 꼬리 포함 · plan_26092908 §4.1 · 01 = 1.6 기동 전 준비 포함 R-e)",
           [len(tpls[n].prompts) for n in TEMPLATE_FILES] == [7, 5, 7, 2])
        # 2026-09-22 S2 round 2(F11): 저작 자기점검은 템플릿 4종 최상단(preamble)에 하나씩 · 7부류 문구를 글자 그대로(단일 상수와 교차검증)
        for n in TEMPLATE_FILES:
            sc = tpls[n].selfchecks
            body = "\n".join(tpls[n].lines[sc[0][0]:sc[0][1] + 1]) if len(sc) == 1 else ""
            miss = [k for k, lab in FACTCHECK_CLASSES.items() if f"{k}. {lab}" not in body]
            ck(f"템플릿 {n} SELFCHECK 1개 · preamble · 7부류 문구(빠짐 {miss})", len(sc) == 1
               and tpls[n].chapter_of[sc[0][0]] == "preamble" and not miss)
        ck("템플릿 02 §2.4 필수 발췌 1(PROMPT 와 린터가 같은 규칙)",
           _directives({p.chapter: p for p in tpls["02-narrative"].prompts}["2.4"].fields)["excerpts"] == 1)
        ck("★템플릿 0.5 배너가 declared-requirement 를 '복사 금지' 무리에 넣지 않는다", any(
            "`tuned` · `declared-requirement` 가 아닌 값은" in ln for ln in tpls["00-hint"].lines))
        ck("★템플릿 3.5 비교 규칙 = 인증서 소프트 지문 블록(옛 digest/NGC/torch/CUDA 목록 ✗)", any(
            "kv_cache_memory_bytes" in ln and "gpu_memory_utilization" in ln and "moe_backend" in ln
            for ln in tpls["03-benchmark"].text.split("\n")) and "00 §0.4 의 이미지 digest · NGC · torch · CUDA" not in tpls["03-benchmark"].text)
        ck("★템플릿 어디에도 절 자리표시(§3.x) 없음", not any(_FACT_PLACEHOLDER.search(tpls[n].text) for n in TEMPLATE_FILES))
        ck("템플릿 03 부하 곡선 = 닫힘 표지 없는 영역 하나(§3.2)",
           [(f.fid, f.chapter) for f in tpls["03-benchmark"].facts if f.open_ended] == [(BENCH_REGION_ID, "3.2")]
           and "FACT:bench_section" not in tpls["03-benchmark"].text)
        from . import evidence as _ev
        ck("03 측정 결손 코드 ⊆ evidence.MISSING_CODES(tripwire)", set(BENCH_MISSING_CODES) <= set(_ev.MISSING_CODES)
           and set(_TRIPLET_ABSENT_CODES) <= set(_ev.MISSING_CODES))

        # 계보 픽스처(격리 · IPv4 모양 리터럴 없음)
        plan_p = "docs/plan/plan_26090918_fixture_딥캠페인.md"
        tl_p = "docs/testlog/testlog_26090921_fixture_스모크_판정.md"
        tl2_p = "docs/testlog/testlog_26091009_fixture_24셀_판정.md"
        log_p = "output/multi/benchlog/serve_fail_fixture/master_failure.log"
        rep_p = "docs/benchmark/bench_report_26091304_fixture_GB10_0.29.0.md"
        files = {
            plan_p: "# plan_26090918 — fixture 딥캠페인\n\n## 1. 목표\nGB10 두 노드에서 fixture-model 을 262k 컨텍스트로 서빙한다.\n"
                    "노드당 메모리 총량은 124,610MiB 이고 인터커넥트는 RoCE 다.\n\n## 2. 예산\n"
                    "floor = MemTotal - weights - kv - overhead >= 11264\nKV 필요량 = 24KiB/token x 262144 = 6144MiB\n",
            tl_p: "# testlog_26090921\n\n## 2. 회차\n회차 3: ValueError: vLLM CUTLASS FP8 MoE backend is disabled for this configuration.\n"
                  "moe-backend 를 비우자 통과했다.\n\n## 3. 측정\nvllm bench serve --max-concurrency 1 --num-prompts 16\n\n## 4. 긴 표\n"
                  + "".join(f"줄 {k:02d} 관측값 기록\n" for k in range(1, 46)),
            tl2_p: "# testlog_26091009\n\n## 2. 오진\n당시 판정: hang 은 모델 결함이다.\n정정: 호스트 워치독이 사살했다.\n",
            # ANSI 색 · tqdm `\r` 진행 조각 · CRLF 줄 끝(2026-09-22 S2 round 2 F8 — 정규화 뒤 줄 번호 = `\n` 개수 = grep -n)
            log_p: ("INFO start\n\x1b[36m(RayWorkerProc pid=7)\x1b[0m worker ready\r\n"
                    "Loading 10%\rLoading 50%\rLoading 100%\nAttributeError: 'tuple' object has no attribute 'start'\n"),
            rep_p: "# bench report\n\n| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |\n"
                   "|---|---|---|---|---|---|---|\n| 1 ★판정점 | 38.81 | 38.5 | 200.1 | 353.2 | 25.1 | 16/0 |\n"
                   "| 2 | 60.25 | 59.9 | 310.4 | 410.0 | 30.2 | 16/0 |\n\n_절삭된 레벨 없음._\n",
        }
        for rel, body in files.items():
            (repo / rel).parent.mkdir(parents=True, exist_ok=True)
            (repo / rel).write_text(body, encoding="utf-8")
        lineage = {"schema_version": 1,
                   "documents": [{"path": plan_p, "kind": "plan", "date_key": "26090918", "depth": 1},
                                 {"path": tl_p, "kind": "testlog", "date_key": "26090921", "depth": 1},
                                 {"path": tl2_p, "kind": "testlog", "date_key": "26091009", "depth": 2}],
                   "evidence_candidates": [{"path": log_p, "kind": "engine_log", "sources": []}]}

        def fixture_resolve(lin, stem):
            s = stem.strip()
            paths = [d["path"] for d in lin.get("documents") or []] + [c["path"] for c in lin.get("evidence_candidates") or []]
            if s in paths:
                return s
            hit = [p for p in paths if Path(p).stem == s or Path(p).stem.startswith(s + "_")]
            return hit[0] if len(hit) == 1 else None

        bench = _bench_module(repo)
        bench_md = bench.render(bench.parse_report(files[rep_p]), Path(rep_p).name)
        base_facts = {
            "tag": "hint/0.29.0rc6/fixture-model-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvauto-plemmap-spec3-eager",
            "generated_utc": "2026-09-21T10:21:01Z",
            "naming": {"grammar": "v6",
                       "segments": {"vllm": {"value": "0.29.0rc6", "source": "cell env VLLM_REF"},
                                    "model": {"value": "fixture-model-nvfp4", "source": "checkpoint basename"}},
                       "axes": {"hw": {"value": "gb10", "source": "manifest gpu_model"}, "nodes": {"value": 2, "source": "manifest nodes"}},
                       "vllm_build_input": {"kind": "release", "ref": "v0.29.0rc6", "sha": "a" * 40, "prev_release": None},
                       "vllm_observed": {"engine_self_report": "0.29.0", "wheel_meta": None, "source": "sweep_index.meta"}},
            "identity": {"model": "fixture-model-nvfp4", "gpu": "NVIDIA GB10", "vllm": "0.29.0", "quant": "nvfp4",
                         "topology": "multi", "tp": 2, "hf_repo": "Org/Fixture-Model-NVFP4", "base_model": "Org/Fixture-Model",
                         "base_slug": "fixture-model", "hf_revision": "c" * 40,
                         "source": {"model": "certificate", "topology": "manifest", "quant": "naming-axis(q)",
                                    "hf_revision": "checkpoint .git HEAD → refs/heads/main"}},
            "event_timeline": [
                {"utc": "2026-09-12T07:43:46Z", "node": "main", "kind": "budget_declare", "label": "smoke-fixture-cell",
                 "detail": {"floor_mib": 40478}, "source": "docs/logs/main/events/2026-09.jsonl:727"},
                {"utc": "2026-09-12T08:14:38Z", "node": "main", "kind": "budget_clear", "label": "smoke-fixture-cell",
                 "detail": None, "source": "docs/logs/main/events/2026-09.jsonl:728"},
                {"utc": "2026-09-12T18:54:37Z", "node": "main", "kind": "budget_declare", "label": "smoke-fixture-cell",
                 "detail": {"floor_mib": 40478}, "source": "docs/logs/main/events/2026-09.jsonl:754"},
                {"utc": "2026-09-12T19:24:49Z", "node": "main", "kind": "budget_renew", "label": "smoke-fixture-cell",
                 "detail": None, "source": "docs/logs/main/events/2026-09.jsonl:755"},
                {"utc": "2026-09-12T19:34:53Z", "node": "main", "kind": "budget_clear", "label": "smoke-fixture-cell",
                 "detail": None, "source": "docs/logs/main/events/2026-09.jsonl:756"}],
            "bench_definition": {"current_full_definition": "full 계측(`lite ∪ GuideLLM × 반복 ≥3`)", "source": ".claude/rules/docs.md",
                                 "this_repeats": 1, "repeats_source": "bench_report(fixture) 측정 구성 repeats",
                                 "meets_current_full": False},
            "build": {"track": "source-build", "dockerfile": "Dockerfile.source-build", "image_tag": "easy-vllm:fixture",
                      # CUDA 버전은 네 마디라 IPv4 모양이다 — 추적 파일에 그 리터럴을 두지 않는다(2026-08-06 자기스캔 사고).
                      "image_digest": "sha256:" + "b" * 64, "torch": "2.13.0a0+9186a08", "cuda": "13.3" + ".1.008", "ngc": "26.07",
                      "cpu_arch": "aarch64", "source": {"ngc": "docker image inspect", "torch": "docker image inspect"}},
            "plane": "docker",
            "applied_set": {"status": "unobservable", "source": "no-build-ledger",
                            "patches": [{"phase": "pre", "file": "60-fixture-patch.sh", "result": "unobserved",
                                         "reason": "원장 부재", "result_source": "none"},
                                        {"phase": "pre", "file": "50-skip-patch.sh", "result": "skipped",
                                         "reason": "SM12X_PORT=0", "result_source": "reconstructed(docker-history+gate)"}],
                            "reconstruction": {"source": "docker-history+gate-rule", "build_args": {"SM12X_PORT": "0"}}},
            "slots": {"build_patch_pre": {"files": ["artifacts/build_patch_pre/60-fixture-patch.sh"], "applicable": True,
                                          "rationale": "재구성", "evidence": {"kind": "reconstruction", "ref": "no-build-ledger"},
                                          "confidence": "2-signal"}},
            "qualification": {"health_200": True, "inference_observed": True, "sources": ["output/multi/benchlog/serve_proof_fixture.json"]},
            "measurement": {"measured_utc": "2026-09-12T19:32:52Z", "decode_tps_conc1": 38.81, "source": "sweep_fixture",
                            "levels": [{"level": 1, "status": "ok", "decode_tps": 38.81, "completed": 16},
                                       {"level": 2, "status": "ok", "decode_tps": 31.1, "completed": 16}],
                            "lite": {"gen_tps": 33.03, "cold_ttft_ms": 2156.6}},
            "measurement_config": {"bench_mode": "full", "repeats": 1, "source": "bench_report(fixture)"},
            "missing": {"HINT_MISSING_SLAVE_ATTESTATION": "서브 attestation 부재"},
            "lineage_reading_list": [{"path": d["path"], "kind": d["kind"], "date_key": d["date_key"], "depth": d["depth"],
                                      "axis_tokens": ["fixture-model"], "superseded_by": [], "tier": "primary"}
                                     for d in lineage["documents"]],
            "value_status_candidates": [
                {"knob": "max-model-len", "value": "262144", "candidate_status": "inherited", "source": "serving-yaml", "comment": None},
                {"knob": "kv-cache-memory-bytes", "value": "21474836480", "candidate_status": "negative-control",
                 "source": "serving-yaml", "comment": "A1 기준선 — 음성대조"},
                {"knob": "speculative-config", "value": '{"method": "mtp", "num_speculative_tokens": 3}',
                 "candidate_status": "inherited", "source": "serving-yaml", "comment": None}],
            "reproduce_steps": [{"order": 1, "step": "render", "phase": "render",
                                 "command": "python3 render.py --materialize-configs\npython3 render.py --cluster-envfile",
                                 "command_source": "derived(렌더러 CLI)", "success": "러너 · env 생성", "duration": "미관측",
                                 "source": "미관측"},
                                {"order": 2, "phase": "build", "command": "<owner smoke> fixture --build", "duration": "unobserved",
                                 "success": "빌드 OK · 이미지 실재", "source": "cell build procedure"}],
            "sub_recipe": {"role_diff": {"master": "ray head + vllm serve", "slave": "ray worker"},
                           "launch_order": ["build", "master up", "slave up"],
                           "env_forward": [{"key": "VLLM_PLE_MMAP", "group": "serve_env", "from": "cell_env", "why": "worker 도 가중치를 올린다"}],
                           "image_identity_args": ["IMAGE_TAG"], "per_node_values": {"MASTER_HOST_IP": "<node:main>"},
                           "preconditions": ["가중치가 같은 경로에 실재"], "parity_attestation": {"observed": "unobservable"}},
            "bench_section_md": bench_md,
            "task_class": "full_benchmark", "perf_waiver": None,
            "campaign": {"id": "camp-fixture", "cell": "fixture-cell", "node": "cluster", "mode": "campaign"},
            "approval": {"approved_by": "사용자(발화 전사) — 픽스처", "approved_utc": "2026-09-21T10:21:01Z", "source": "campaign:hint_targets"},
            "publication": {"topic": "camp_fixture", "manifest_ref": "docs/_evidence/camp_fixture.work-manifest.json",
                            "task_class": "full_benchmark"},
        }

        def vs_block(i, c, status):
            val = c["value"]
            val = f"'{val}'" if any(ch in val for ch in ",:#\"") else val
            return (f"```hint-event\nid: V{i}\nkind: value-status\n노브: {c['knob']}\n값: {val}\n지위: {status}\n"
                    f"근거: 픽스처 근거\n출처: [plan_26090918 §2]\n```")

        def answers(fx: dict) -> dict:
            multi = _topology(fx) == "multi"
            vs = "\n\n".join(vs_block(i, c, c["candidate_status"]) for i, c in enumerate(_candidates(fx), 1))
            return {
                ("00-hint", "0.1"): "fixture-model 을 GB10 1g2n cluster(Ray TP=2)·v0.29.0rc6 로 서빙했다 — 동시성 1 decode 38.81 t/s.\n"
                                    "가장 비싼 벽은 CUTLASS FP8 MoE 가드 충돌(W1)이었고 moe-backend 를 비워 넘었다.\n"
                                    "KV 21474836480 바이트는 필요량 대비 과잉인 음성대조값(V2)이다 — 복사하지 마라.",
                ("00-hint", "0.2"): "통합메모리라 CPU offload 가 메모리를 거두지 못한다 — 근거 plan_26090918 §1.",
                ("00-hint", "0.4"): "source-build 를 골랐다 — 근거 testlog_26090921 §2. 재현 좌표는 v0.29.0rc6 이다.",
                ("00-hint", "0.5"): "① KV 는 재도출한다. ② moe-backend 는 명시하지 않는다(W1). ③ max-model-len 은 승계값이다.",
                ("00-hint", "0.6"): "1. 상위 버전에서 패치 필요 여부 → 트립와이어 로그 확인 → 그대로/폐기.\n빌드 문제 → upstream-version-watch.",
                ("00-hint", "0.9"): "빈 꼬리다 — 같은 q·len·kv 의 다른 셀이 없어 가르는 노브가 없다(inputs/tail.json = []).",
                ("01-artifacts", "1.2"): "### 60-fixture-patch.sh\n재구성 결과라 적용 미관측이다. 헤더 원문:\n\n"
                                         "> [원문] 60-fixture-patch.sh §L2-2\n> # fixture patch: PLE 게이트 완화",
                ("01-artifacts", "1.3"): vs + "\n\n승계값과 음성대조값을 복사하지 마라.",
                ("01-artifacts", "1.4"): "빌드 뒤 health 200 과 추론 1회로 판정한다. 기동 소요는 미관측이다.",
                ("01-artifacts", "1.5"): "slave 는 트리플렛을 받지 않는 Ray worker 다." if multi else None,
                ("01-artifacts", "1.6"): "### 가중치 획득\n관리 NAS 모드다 — 다운로드 없음(근거 plan_26090918 §1).\n\n"
                                         "### 스테이징\n해당 없음 — PLE 없는 체크포인트(근거 plan_26090918 §2).\n\n"
                                         "### 네트워크 단계\n베이스 이미지 pull · pip wheel — 오프라인이면 미리 받아 둔다(추론 — 실행 검증 ✗).",
                ("02-narrative", "2.1"): "첫 plan 의 목표:\n\n> [원문] plan_26090918 §1\n> GB10 두 노드에서 fixture-model 을 262k 컨텍스트로 서빙한다.",
                ("02-narrative", "2.2"): "### W1 CUTLASS 가드 충돌\n백엔드를 명시하자 가드와 충돌했다.\n\n```hint-event\nid: W1\nkind: wall\n"
                                         "증상: 기동 중 MoE 백엔드 선택에서 실패\n"
                                         "서명: \"ValueError: vLLM CUTLASS FP8 MoE backend is disabled…configuration.\"\n"
                                         "원인: 명시한 백엔드가 가드와 충돌\n해소: moe-backend 미설정\n검증: testlog_26090921 §2\n"
                                         "전이등급: arch-scaled\n출처: [testlog_26090921 §2, 60-fixture-patch.sh 헤더]\n```\n\n"
                                         "> [원문] testlog_26090921 §2\n> 회차 3: ValueError: vLLM CUTLASS FP8 MoE backend is disabled for this configuration.\n\n"
                                         "### W2 tuple 속성 오류\n```hint-event\nid: W2\nkind: wall\n증상: mmap 로드 중 예외\n"
                                         "서명: \"AttributeError: 'tuple' object has no attribute 'start'\"\n원인: range 대신 tuple\n"
                                         f"해소: rng[0]/[1]\n검증: 재기동 PASS\n전이등급: arch-invariant\n출처: [{log_p}]\n```",
                ("02-narrative", "2.3"): "```hint-event\nid: R1\nkind: rejected\n시도: moe-backend triton 명시\n기각사유: 미지원\n"
                                         "출처: [testlog_26090921 §2]\n```",
                ("02-narrative", "2.4"): "> [원문] testlog_26091009 §2\n> 당시 판정: hang 은 모델 결함이다.\n\n"
                                         "```hint-event\nid: M1\nkind: misdiagnosis\n증상: hang\n서명: \"당시 판정: hang 은 모델 결함이다\"\n"
                                         "원인: 워치독을 못 봤다\n해소: 호스트 워치독 사살\n검증: testlog_26091009 §2\n전이등급: judgment\n"
                                         "현재지위: 반증\n출처: [testlog_26091009 §2]\n```",
                ("02-narrative", "2.5"): "(V2) KV 는 필요량의 배수로 뒀다:\n\n> [원문] plan_26090918 §2\n> KV 필요량 = 24KiB/token x 262144 = 6144MiB",
                ("02-narrative", "2.6"): "- moe-backend 를 명시하지 마라 — W1 — 비워 둔다.",
                ("02-narrative", "2.7"): "```hint-event\nid: Q1\nkind: open-question\n물음: cold TTFT 이상치\n현재상태: 미설명\n"
                                         "출처: [testlog_26090921 §3]\n```",
                ("03-benchmark", "3.4"): "> [원문] testlog_26090921 §3\n> vllm bench serve --max-concurrency 1 --num-prompts 16\n\n- 반복 1회.",
                ("03-benchmark", "3.5"): "- 이전 측정 · 비교 불가 — 조건 차이. 판정 등급: full(반복 1회 · 현행 정의 미충족).",
            }

        def fill(payload: Path, fx: dict) -> None:
            ans = answers(fx)
            for name in TEMPLATE_FILES:
                p = payload / f"{name}.md"
                doc = _Doc.parse(name, p.read_text(encoding="utf-8"))
                lines = list(doc.lines)
                for i in range(len(lines) - 1, -1, -1):
                    if lines[i].startswith(AGENT_MARK):
                        body = ans.get((name, doc.chapter_of[i]))
                        if body is None:
                            raise RuntimeError(f"픽스처 답 없음: {name} §{doc.chapter_of[i]}")
                        lines[i:i + 1] = body.split("\n")
                p.write_text("\n".join(lines), encoding="utf-8")

        def scaffold(tag: str, fx: dict) -> Path:
            payload = root / tag / "payload"
            art = payload / "artifacts" / "build_patch_pre"
            art.mkdir(parents=True)
            (art / "60-fixture-patch.sh").write_text("#!/bin/sh\n# fixture patch: PLE 게이트 완화\nexit 0\n", encoding="utf-8")
            render_scaffold(repo, fx, payload)
            return payload

        facts_json = json.loads(core.dumps(base_facts))
        kw = dict(lineage=lineage, substitute=lambda t: t, resolve_stem=fixture_resolve,
                  manifest={"task_class": "full_benchmark"}, bench_report=rep_p)

        # ── 기본 경로 ──
        payload = scaffold("main", base_facts)
        ps = prompts(payload)
        ck("PROMPT 목록(조건 성립 · 선택 포함 · 0.9 이름 꼬리 · 1.6 기동 전 준비)", len(ps) == 21 and all(p["question"] for p in ps)
           and any(p["section"] == "0.9" and not p["optional"] for p in ps))
        ck("스냅샷 기록", default_snapshot_path(payload).is_file())
        ck("사실 블록 채움(0.4 관측 NGC)", "26.07" in fact_blocks((payload / "00-hint.md").read_text(encoding="utf-8"))["resolved"])
        ck("벤치 절 = 렌더 원문", fact_blocks((payload / "03-benchmark.md").read_text(encoding="utf-8"))["bench_section"] == bench_md.strip("\n"))
        t01 = (payload / "01-artifacts.md").read_text(encoding="utf-8")
        ck("재현 표: 단계별 성공 판정 칸 · 여러 줄 명령은 줄 그대로(한 칸에 접지 않는다)",
           "| 순서 | 단계 | 성공 판정 |" in t01 and "\npython3 render.py --materialize-configs\npython3 render.py --cluster-envfile\n" in t01
           and "| 러너 · env 생성 |" in t01)
        ck("값의 지위표: 후보 근거와 yaml 주석을 따로 싣는다", "| 노브 | 값 | 후보 지위 | 후보 근거 | yaml 주석 | 출처 |" in t01
           and "A1 기준선 — 음성대조" in t01)
        t03 = (payload / "03-benchmark.md").read_text(encoding="utf-8")
        ck("측정 표: 레벨 목록은 별도 표 · dict 는 키.하위키 행(JSON 덩어리 ✗)",
           "| `level` | `status` |" in t03 and "| `lite.gen_tps` | 33.03 |" in t03 and '{"' not in fact_blocks(t03)["measurement"])
        ck("03 측정 결손: 없음도 적는다", "**측정 결손**: _없음" in fact_blocks(t03)["bench_missing"])
        unfilled = lint(repo, payload, **kw)
        ck("★<<AGENT 잔존 차단", "HINT_AGENT_PLACEHOLDER_RESIDUE" in codes(unfilled) and "HINT_SECTION_UNAUTHORED" in codes(unfilled))
        ck("★미저작 봉인 거부", raises(lambda: seal_prompts(payload), "HINT_AGENT_PLACEHOLDER_RESIDUE"))
        ck("★brief 미저작", raises(lambda: brief(payload), "HINT_BRIEF_ABSENT"))
        fill(payload, base_facts)
        ck("refresh 가 파생 블록을 채움", set(refresh(payload)) == set(DERIVED_FACTS))
        ck("refresh 멱등", refresh(payload) == [])
        clean = lint(repo, payload, **kw)
        ck(f"저작 뒤 lint 통과: {clean[:4]}", clean == [])
        ck("wall 지도 파생", "W2" in fact_blocks((payload / "00-hint.md").read_text(encoding="utf-8"))["wall_map"])
        subs = subsections(payload, "01-artifacts", "1.2")
        ck(f"subsections: §1.2 소제목 키 · 첫 문단(인용 앞에서 끊는다)({subs})", subs == [
            {"title": "60-fixture-patch.sh", "key": "60-fixture-patch.sh", "text": "재구성 결과라 적용 미관측이다. 헤더 원문:"}])
        ck("★subsections: 소제목 없는 절 = 빈 목록(hint-event 펜스 안 줄은 문단이 아니다)",
           subsections(payload, "01-artifacts", "1.3") == [])
        dflt = lint(repo, payload, lineage=lineage, subst_table=[], manifest={"task_class": "full_benchmark"}, bench_report=rep_p)
        ck(f"기본 해소(lineage.resolve_stem · pii.substitute · pii_terms 파일): {dflt[:3]}", dflt == [])

        def cli_verify(p: Path, report: Path) -> int:
            """render_bench_section 의 **실제 CLI** `--verify --section` (린터가 통과시킨 페이로드를 그 CLI 가 FAIL 시키던
            2026-09-22 리뷰 사고의 음성대조 — 자체검사가 in-process 비교만 돌아 CLI 계약 위반을 보지 못했다)."""
            import contextlib
            import io
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                return bench.main(["--verify", "--section", str(p / "03-benchmark.md"), "--report", str(report)])

        ck("CLI render_bench_section --verify --section 03-benchmark.md = PASS", cli_verify(payload, repo / rep_p) == 0)

        def variant(tag: str, mutate) -> Path:
            dst = root / "v" / tag / "payload"
            shutil.copytree(payload, dst)
            mutate(dst)
            return dst

        def edit(name: str, old: str, new: str, count: int = 1):
            def f(p: Path):
                t = (p / name).read_text(encoding="utf-8")
                if old not in t:
                    raise RuntimeError(f"변형 앵커 없음: {name}: {old[:40]!r}")
                (p / name).write_text(t.replace(old, new, count), encoding="utf-8")
            return f

        def lint_v(p: Path, **over) -> set[str]:
            args = dict(kw, facts=facts_json)
            args.update(over)
            return codes(lint(repo, p, **args))

        ck("★PROMPT 잔존(봉인 검사)", "HINT_PROMPT_RESIDUE" in lint_v(payload, sealed=True))
        ck("★필수 필드 부재", "HINT_EVENT_FIELD_MISSING" in lint_v(variant("f", edit("02-narrative.md", "원인: 명시한 백엔드가 가드와 충돌\n", ""))))
        # R-e(2026-09-29 · plan_26092908): 1.6 기동 전 준비는 필수 절 — 비우면 기존 필수 절 규칙(HINT_SECTION_UNAUTHORED)이 §1.6 을 짚는다.
        prep_body = ("### 가중치 획득\n관리 NAS 모드다 — 다운로드 없음(근거 plan_26090918 §1).\n\n"
                     "### 스테이징\n해당 없음 — PLE 없는 체크포인트(근거 plan_26090918 §2).\n\n"
                     "### 네트워크 단계\n베이스 이미지 pull · pip wheel — 오프라인이면 미리 받아 둔다(추론 — 실행 검증 ✗).")
        empty_prep = [i for i in lint(repo, variant("prep", edit("01-artifacts.md", prep_body, "")), **dict(kw, facts=facts_json))
                      if i["code"] == "HINT_SECTION_UNAUTHORED"]
        ck("★R-e 1.6 기동 전 준비 절이 비면 차단(HINT_SECTION_UNAUTHORED · §1.6) · 채우면 그 코드 없음",
           any("§1.6" in i["message"] for i in empty_prep)
           and not any("§1.6" in i["message"] for i in lint(repo, payload, **dict(kw, facts=facts_json))
                       if i["code"] == "HINT_SECTION_UNAUTHORED"))
        p16 = {p.chapter: p for p in tpls["01-artifacts"].prompts}.get("1.6")
        ck("R-e 1.6 PROMPT 필수 필드 = 가중치 획득 경로 · 스테이징 생성 절차 · 네트워크 단계(명령 수준 · 출처) · 조건·선택 ✗",
           p16 is not None and all(w in p16.fields.get("필수 필드", "") for w in ("가중치 획득 경로", "스테이징 생성 절차",
                                                                                  "네트워크가 필요한 단계", "명령 수준", "출처"))
           and not _directives(p16.fields)["optional"] and _directives(p16.fields)["condition"] is None)
        ck("★발췌 1자 변조", "HINT_EXCERPT_MISMATCH" in lint_v(variant("x", edit("02-narrative.md", "> GB10 두 노드에서", "> GB11 두 노드에서"))))
        lin_wo_plan = {**lineage, "documents": [d for d in lineage["documents"] if d["path"] != plan_p]}
        ck("★LINEAGE 밖 발췌", "HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE" in lint_v(payload, lineage=lin_wo_plan))
        ck("★LINEAGE 밖 출처", "HINT_EVENT_SOURCE_OUTSIDE_LINEAGE" in lint_v(
            variant("s", edit("02-narrative.md", "출처: [testlog_26091009 §2]", "출처: [testlog_29999999 §2]"))))
        v1 = vs_block(1, base_facts["value_status_candidates"][0], "inherited")
        ck("★value-status 커버리지 결손", "HINT_VALUE_STATUS_GAP" in lint_v(variant("g", edit("01-artifacts.md", v1, ""))))
        ck("★value-status 값 불일치", "HINT_VALUE_STATUS_VALUE_MISMATCH" in lint_v(
            variant("gv", edit("01-artifacts.md", "값: 262144", "값: 262000"))))
        ck("★PERF-WARNING 부재(waiver)", "HINT_PERF_WARNING_MISSING" in lint_v(
            payload, manifest={"task_class": "full_benchmark", "benchmark": {"perf_waiver": {"warning_flag": "기대 이하 — 재측정"}}}))
        ck("★배너 삭제", "HINT_BANNER_ABSENT" in lint_v(variant("b", edit("00-hint.md", "> " + REVALIDATION_BANNERS[1] + "\n", ""))))
        ck("★배너를 주석 안으로", "HINT_BANNER_ABSENT" in lint_v(variant("bc", edit(
            "00-hint.md", "> " + REVALIDATION_BANNERS[1], "<!-- " + REVALIDATION_BANNERS[1] + " -->"))))

        def drop_walls(p: Path):
            t = (p / "02-narrative.md").read_text(encoding="utf-8")
            t = re.sub(r"```hint-event\nid: W\d+\nkind: wall\n.*?```", "", t, flags=re.DOTALL)
            (p / "02-narrative.md").write_text(t, encoding="utf-8")
            refresh(p)
        ck("★wall 블록 0", "HINT_JOURNEY_WALL_ABSENT" in lint_v(variant("w", drop_walls)))
        long_ex = "> [원문] testlog_26090921 §4\n" + "".join(f"> 줄 {k:02d} 관측값 기록\n" for k in range(1, 42))
        ck("★발췌 40줄 상한", "HINT_EXCERPT_SOURCE_LIMIT" in lint_v(variant("l", edit("02-narrative.md", "- moe-backend 를 명시하지 마라",
                                                                                      long_ex + "\n- moe-backend 를 명시하지 마라"))))
        ck("★사실 블록 재저작(F7 · NGC)", "HINT_FACT_DRIFT" in lint_v(variant("d", edit("00-hint.md", "| 26.07 |", "| 26.05 |"))))
        ck("★파생 블록 낡음", "HINT_DERIVED_BLOCK_STALE" in lint_v(variant("st", edit("02-narrative.md", "id: W2", "id: W3"))))
        # 앵커는 **부하 곡선 표의 판정점 행**이다 — 맨 앞 `| 38.81 |` 는 §3.1 측정 표에 있어, 옛 앵커는 곡선이 아니라 측정 표를
        # 변조하고도 초록이었다(2026-09-22 리뷰: 다른 블록의 drift 로 통과한 공허 시험).
        bench_row = "| 1 ★판정점 | 38.81 |"
        bn_codes = lint_v(variant("bn", edit("03-benchmark.md", bench_row, "| 1 ★판정점 | 38.18 |")))
        ck("★벤치 표 숫자 1개(부하 곡선 영역)", {"HINT_FACT_DRIFT", "HINT_BENCH_SECTION_DRIFT"} <= bn_codes)
        tampered_rep = root / "bench_report_26091304_fixture_GB10_0.29.0.md"
        tampered_rep.write_text(files[rep_p].replace("38.81", "39.81"), encoding="utf-8")
        ck("★벤치 원천 불일치", "HINT_BENCH_SECTION_DRIFT" in lint_v(payload, bench_report=tampered_rep))
        ck("★벤치 원천 부재", "HINT_BENCH_SECTION_UNVERIFIABLE" in lint_v(payload, bench_report=None))
        no_table = root / "bench_report_26091304_notable_GB10_0.29.0.md"
        no_table.write_text("# bench report\n\n표가 없다.\n", encoding="utf-8")
        ck("★벤치 원천에 표 없음(빈 표로 접지 않는다)", "HINT_BENCH_SECTION_UNVERIFIABLE" in lint_v(payload, bench_report=no_table))
        m_txt = fact_blocks((payload / "03-benchmark.md").read_text(encoding="utf-8"))["measurement"]
        ck("측정값 원문 그대로(반올림 ✗ · 재계산 ✗)", "| `decode_tps_conc1` | 38.81 |" in m_txt and "| `measured_utc` | 2026-09-12T19:32:52Z |" in m_txt)
        exs = excerpts((payload / "02-narrative.md").read_text(encoding="utf-8"))
        ck("excerpts() 공개 함수", [e["stem"] for e in exs][:2] == ["plan_26090918", "testlog_26090921"]
           and exs[0]["line_count"] == 1 and exs[0]["chapter"] == "2.1")
        ck("★서명 1자 변조", "HINT_SIGNATURE_MISMATCH" in lint_v(variant("sg", edit("02-narrative.md", "is disabled…", "is disabeld…"))))
        # D-c(2026-09-22 통합): 짧은 서명 조각은 원문 증거가 아니다 · 마크다운 출처의 §절은 제목 줄이어야 한다
        ck("★서명 조각 12자 미만(D-c)", "HINT_SIGNATURE_TOO_SHORT" in lint_v(variant("sgs", edit(
            "02-narrative.md", "서명: \"당시 판정: hang 은 모델 결함이다\"", "서명: \"hang\""))))
        ck("★서명 앞 조각만 짧아도 차단(…로 끼운 한두 글자 조각)", "HINT_SIGNATURE_TOO_SHORT" in lint_v(variant("sgs2", edit(
            "02-narrative.md", "is disabled…configuration.", "is disabled…on."))))
        ck("★발췌 §절이 출처 제목 줄과 다름(D-c)", "HINT_EXCERPT_SECTION_UNKNOWN" in lint_v(variant("secx", edit(
            "02-narrative.md", "> [원문] plan_26090918 §1", "> [원문] plan_26090918 §9"))))
        ck("발췌 §절 = 제목 줄 앞부분(번호만 · 번호+제목) 통과", not ({"HINT_EXCERPT_SECTION_UNKNOWN"} & lint_v(variant("secok", edit(
            "02-narrative.md", "> [원문] plan_26090918 §1", "> [원문] plan_26090918 §1. 목표")))))
        ck("비-마크다운 출처(.sh 헤더)는 §절 대조 대상 밖", _section_in_headings("헤더", "#!/bin/sh\n# x\n", None) is False
           and "HINT_EXCERPT_SECTION_UNKNOWN" not in lint_v(payload))
        ck("★코드 펜스 안 `#` 줄은 제목이 아니다", not _section_in_headings("x", "```\n# x\n```\n", None)
           and _section_in_headings("x", "# x\n", None))
        # ── 2026-09-22 S2 round 2(F8): 출처 정규화 · 비-마크다운 `§L<a>-<b>` ──
        ck("normalize_source: CRLF · \\r 진행 조각 · ANSI · 줄 수 보존",
           normalize_source("a\r\nb 1%\rb 100%\n\x1b[36mc\x1b[0m\n") == "a\nb 100%\nc\n"
           and normalize_source(normalize_source("x\ry\n")) == "y\n")
        osc = "a\x1b]0;title\nb\nc\x07d\n"
        ck("★닫히지 않은 OSC 가 줄바꿈을 삼키지 않는다(정규화 뒤 줄 수 = 원본 `\\n` 수)",
           normalize_source(osc).count("\n") == osc.count("\n") and normalize_source(osc).split("\n")[1] == "b")
        log_txt = substituted_source(repo, payload, "master_failure", lineage=lineage, substitute=lambda s: s,
                                     resolve_stem=fixture_resolve)
        log_rows = log_txt.split("\n")
        ck(f"발췌 도우미 = 정규화된 원문(줄 번호 = \\n 개수 · ANSI ✗ · \\r 마지막 조각)({log_rows[:4]})",
           log_rows[1] == "(RayWorkerProc pid=7) worker ready" and log_rows[2] == "Loading 100%" and len(log_rows) == 5
           and "\x1b" not in log_txt)
        log_q = "> Loading 100%\n> AttributeError: 'tuple' object has no attribute 'start'"
        base_14 = "빌드 뒤 health 200 과 추론 1회로 판정한다. 기동 소요는 미관측이다."

        def with_log_excerpt(tag: str, label: str, quote: str = log_q) -> Path:
            return variant(tag, edit("01-artifacts.md", base_14, f"{base_14}\n\n> [원문] master_failure {label}\n{quote}"))

        ok_log = with_log_excerpt("lg", "§L3-4")
        ck(f"비-마크다운 출처 발췌 §L3-4 통과: {sorted(lint_v(ok_log))}", lint_v(ok_log) == set())
        ck("★비-마크다운 출처에 제목식 표지(§엔진 로그) = HINT_EXCERPT_SECTION_SHAPE",
           "HINT_EXCERPT_SECTION_SHAPE" in lint_v(with_log_excerpt("lg-shape", "§엔진 로그")))
        ck("★줄 범위 밖의 인용(원문에는 있다) = HINT_EXCERPT_OUTSIDE_LINES",
           "HINT_EXCERPT_OUTSIDE_LINES" in lint_v(with_log_excerpt("lg-out", "§L1-2")))
        ck("★줄 범위가 원문 줄 수를 넘는다 = HINT_EXCERPT_LINES_OUT_OF_RANGE",
           "HINT_EXCERPT_LINES_OUT_OF_RANGE" in lint_v(with_log_excerpt("lg-oor", "§L3-9")))
        ck("★줄 범위가 인용보다 지나치게 넓다(인용 2줄 · 범위 5줄) = HINT_EXCERPT_LINES_TOO_WIDE",
           "HINT_EXCERPT_LINES_TOO_WIDE" in lint_v(with_log_excerpt("lg-wide", "§L1-5")))
        ck("★ANSI 제어열을 그대로 옮긴 인용 = HINT_EXCERPT_MISMATCH(정규화가 지운 바이트)", "HINT_EXCERPT_MISMATCH" in lint_v(
            with_log_excerpt("lg-ansi", "§L2-2", "> \x1b[36m(RayWorkerProc pid=7)\x1b[0m worker ready")))
        ck("★`\\r` 로 덮인 진행 조각(Loading 50%)을 인용 = HINT_EXCERPT_MISMATCH", "HINT_EXCERPT_MISMATCH" in lint_v(
            with_log_excerpt("lg-cr", "§L3-3", "> Loading 50%")))
        ck("artifacts(.sh) 발췌도 §L 표지: §L2-2 통과 · ★§헤더 = SECTION_SHAPE", "HINT_EXCERPT_SECTION_SHAPE" not in lint_v(payload)
           and "HINT_EXCERPT_SECTION_SHAPE" in lint_v(variant("sh-hdr", edit(
               "01-artifacts.md", "> [원문] 60-fixture-patch.sh §L2-2", "> [원문] 60-fixture-patch.sh §헤더"))))
        # ── 2026-09-22 S2 round 3: 측정 도구 스냅샷(draft 상대 `inputs/sources/<name>@<rev12>`)이 발췌 · 출처로 해소된다 ──
        snap_name = "run_bench.sh@0123456789ab"
        snap_rel = f"{SNAPSHOT_DIR}/{snap_name}"
        snap_body = "#!/bin/bash\nset -e\nvllm bench serve --random-range-ratio 0 --ignore-eos --temperature 0\n"
        snap_rev, snap_path = "0123456789ab" + "c" * 28, ".claude/skills/adversarial-benchmark/scripts/run_bench.sh"
        lin_snap = {**lineage, "evidence_candidates": [*lineage["evidence_candidates"],
                                                       {"path": snap_rel, "kind": "tool-source@rev",
                                                        "origin": f"git:{snap_rev}:{snap_path}"}]}
        snap_q = "> vllm bench serve --random-range-ratio 0 --ignore-eos --temperature 0"

        def snap_origin(rev: str, path: str) -> bytes | None:
            # 가짜 git 판독기(주입) — origin 이 가리키는 blob 만 안다(그 밖 = 읽을 수 없음)
            return snap_body.encode("utf-8") if (rev, path) == (snap_rev, snap_path) else None

        def lint_s(p: Path, **over) -> set[str]:
            return lint_v(p, **{"lineage": lin_snap, "snapshot_origin": snap_origin, **over})

        def with_snapshot(tag: str, label: str = "§L3-3", token: str = snap_name, write: bool = True, quote: str = snap_q) -> Path:
            def mut(p: Path) -> None:
                if write:
                    (p.parent / SNAPSHOT_DIR).mkdir(parents=True, exist_ok=True)
                    (p.parent / snap_rel).write_text(snap_body, encoding="utf-8")
                edit("01-artifacts.md", base_14, f"{base_14}\n\n> [원문] {token} {label}\n{quote}")(p)
            return variant(tag, mut)

        sn_ok = with_snapshot("sn-ok")
        ck(f"R3 스냅샷 발췌(`<name>@<rev12>` §L3-3) 통과: {sorted(lint_s(sn_ok))}", lint_s(sn_ok) == set())
        ck("R3 스냅샷 발췌: `inputs/sources/…` 경로 토큰도 같은 파일", lint_s(with_snapshot("sn-path", token=snap_rel)) == set())
        ck("R3 발췌 도우미: 스냅샷 = draft 상대 바이트(저장소 파일 아님)", "--ignore-eos" in substituted_source(
            repo, sn_ok, snap_name, lineage=lin_snap, substitute=lambda s: s, resolve_stem=fixture_resolve,
            snapshot_origin=snap_origin))
        ck("★R3 LINEAGE 에 등재되지 않은 스냅샷 파일 = HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE(디스크에 있어도)",
           "HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE" in lint_s(sn_ok, lineage=lineage))
        ck("★R3 등재됐으나 파일이 없는 스냅샷 = HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE(저장소 · artifacts 로 흘리지 않는다)",
           "HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE" in lint_s(with_snapshot("sn-nofile", write=False)))
        ck("★R3 스냅샷 발췌 머리가 줄 범위가 아니다 = HINT_EXCERPT_SECTION_SHAPE", "HINT_EXCERPT_SECTION_SHAPE" in lint_s(
            with_snapshot("sn-shape", label="§헤더")))
        ck("★R3 스냅샷 발췌 1자 변조 = HINT_EXCERPT_MISMATCH", "HINT_EXCERPT_MISMATCH" in lint_s(
            with_snapshot("sn-mm", quote=snap_q.replace("--ignore-eos", "--ignore-eoz"))))
        ck("★R3 다른 리비전 토큰(@rev 불일치) = 밖", "HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE" in lint_s(
            with_snapshot("sn-rev", token="run_bench.sh@0123456789ac")))
        # 2026-09-22 round 3 적대 리뷰: 스냅샷 **파일**을 고치고 발췌를 거기에 맞추면(1자) 발췌 대조는 통과한다 — origin 대조가 막는다
        sn_tamper = with_snapshot("sn-tamper", quote=snap_q.replace("--temperature 0", "--temperature 1"))
        (sn_tamper.parent / snap_rel).write_text(snap_body.replace("--temperature 0", "--temperature 1"), encoding="utf-8")
        ck("★R3 스냅샷 파일 변조(발췌는 변조본과 일치) = HINT_EXCERPT_SNAPSHOT_DRIFT · 발췌 대조로는 통과했을 것",
           "HINT_EXCERPT_SNAPSHOT_DRIFT" in lint_s(sn_tamper) and "HINT_EXCERPT_MISMATCH" not in lint_s(sn_tamper)
           and raises(lambda: substituted_source(repo, sn_tamper, snap_name, lineage=lin_snap, substitute=lambda s: s,
                                                 resolve_stem=fixture_resolve, snapshot_origin=snap_origin),
                      "HINT_EXCERPT_SNAPSHOT_DRIFT"))
        ck("★R3 스냅샷 origin 을 읽을 수 없다(커밋 부재 · git 아님) = HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE(통과 ✗)",
           "HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE" in lint_s(sn_ok, snapshot_origin=lambda rev, path: None))
        lin_snap_bad = {**lineage, "evidence_candidates": [*lineage["evidence_candidates"],
                                                           {"path": snap_rel, "kind": "tool-source@rev",
                                                            "origin": f"git:{snap_rev[:12]}:{snap_path}"}]}
        ck("★R3 스냅샷 origin 모양 결함(12자 rev) = HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE",
           "HINT_EXCERPT_SNAPSHOT_UNVERIFIABLE" in lint_s(sn_ok, lineage=lin_snap_bad))
        # 임시 사본 검사(hint.py lint = refresh 한 사본 · verify = 봉인 트리를 푼 임시 디렉터리): 스냅샷은 draft 에 있다 — draft_dir 명시
        sn_copy = root / "elsewhere" / "payload"
        shutil.copytree(sn_ok, sn_copy)
        ck("R3 스냅샷: 다른 자리의 페이로드 사본 + draft_dir = draft 면 통과", lint_s(sn_copy, draft_dir=sn_ok.parent) == set())
        ck("★R3 스냅샷: 사본만 검사하고 draft_dir 을 주지 않으면 밖(사본 옆에는 스냅샷이 없다 — 명시가 필요한 이유)",
           "HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE" in lint_s(sn_copy))
        # ── F9: brief 수치 · declared-requirement 근거 · 사실 블록 자리표시 · 결과 출처 어휘 ──
        ck("★brief 수치가 사실 블록에 없다(단위 환산 20480MiB ← 21474836480) = HINT_BRIEF_NUMBER_UNGROUNDED",
           "HINT_BRIEF_NUMBER_UNGROUNDED" in lint_v(variant("bn20480", edit("00-hint.md", "KV 21474836480 바이트는", "KV 20480MiB 는"))))
        ck("★brief 수치 반올림(38.81 → 38.8) = HINT_BRIEF_NUMBER_UNGROUNDED",
           "HINT_BRIEF_NUMBER_UNGROUNDED" in lint_v(variant("bnround", edit("00-hint.md", "decode 38.81 t/s", "decode 38.8 t/s"))))
        ck("brief 수치 대조: 약칭(262k) · 버전(v0.29.0rc6) · 한 자리 서수는 건너뛴다",
           _brief_numbers("262k · v0.29.0rc6 · TP=2 · W1 · 0.29.0 · 동시성 1") == [])
        # 2026-09-22 적대 리뷰: 글자가 붙은 소수 · 파생 블록(저작자 hint-event)만의 수치
        ck("★brief 반올림 소수에 글자 단위를 붙여도(38.8t/s) = HINT_BRIEF_NUMBER_UNGROUNDED",
           _brief_numbers("decode 38.8t/s · 2.5x") == ["38.8", "2.5"] and "HINT_BRIEF_NUMBER_UNGROUNDED" in lint_v(
               variant("bnunit", edit("00-hint.md", "decode 38.81 t/s", "decode 38.8t/s"))))

        def wall_only_number(p: Path) -> None:
            edit("02-narrative.md", "증상: 기동 중 MoE 백엔드 선택에서 실패\n", "증상: 기동 중 MoE 백엔드 선택에서 실패(777.7 초)\n")(p)
            edit("00-hint.md", "decode 38.81 t/s", "decode 38.81 t/s · 벽 777.7 초")(p)
            refresh(p)                                # 파생 wall_map 이 그 수치를 싣는다 — 싣고도 근거가 아니어야 한다
        bn_wall = variant("bnwall", wall_only_number)
        ck("★brief 수치가 저작자 hint-event(파생 wall_map)에만 있다 = HINT_BRIEF_NUMBER_UNGROUNDED(저작 수치가 스스로를 근거 짓지 않는다)",
           "777.7" in fact_blocks((bn_wall / "00-hint.md").read_text(encoding="utf-8"))["wall_map"]
           and "HINT_BRIEF_NUMBER_UNGROUNDED" in lint_v(bn_wall))
        dr_old = "지위: inherited\n근거: 픽스처 근거\n출처: [plan_26090918 §2]"
        ck("★declared-requirement 인데 W id · artifacts 출처가 없다 = HINT_VALUE_STATUS_REQUIREMENT_UNGROUNDED",
           "HINT_VALUE_STATUS_REQUIREMENT_UNGROUNDED" in lint_v(variant("dr0", edit(
               "01-artifacts.md", dr_old, "지위: declared-requirement\n근거: yaml 주석 필수\n출처: [plan_26090918 §2]"))))
        ck("★declared-requirement 의 W id 가 이 페이로드에 없는 벽(W9) = 근거 아님", "HINT_VALUE_STATUS_REQUIREMENT_UNGROUNDED" in lint_v(
            variant("dr9", edit("01-artifacts.md", dr_old, "지위: declared-requirement\n근거: W9 가 다시 난다\n출처: [plan_26090918 §2]"))))
        ck("declared-requirement 근거 = W id(있는 벽) 통과", "HINT_VALUE_STATUS_REQUIREMENT_UNGROUNDED" not in lint_v(variant("drw", edit(
            "01-artifacts.md", dr_old, "지위: declared-requirement\n근거: 빼면 W1 이 다시 난다\n출처: [plan_26090918 §2]"))))
        ck("declared-requirement 근거 = 실패를 기록한 artifacts 파일 출처 통과(계보 밖 기록)",
           "HINT_VALUE_STATUS_REQUIREMENT_UNGROUNDED" not in lint_v(variant("dra", edit(
               "01-artifacts.md", dr_old, "지위: declared-requirement\n근거: 패치 헤더가 실패를 적었다\n출처: [60-fixture-patch.sh §L2-2]"))))

        def with_triplet(p: Path) -> None:
            # 1차 함정의 모양: 값을 선언한 서빙 yaml 의 '명시 필수' 주석을 출처로 단다(2026-09-22 적대 리뷰 — 1판은 통과시켰다)
            (p / "artifacts" / "triplet").mkdir(parents=True)
            (p / "artifacts" / "triplet" / "fixture-cell.yaml").write_text(
                "async-scheduling: false        # 명시 필수\n", encoding="utf-8")
            edit("01-artifacts.md", dr_old, "지위: declared-requirement\n근거: yaml 주석이 명시 필수라 적었다\n"
                                            "출처: [fixture-cell.yaml §L1-1]")(p)
        ck("★declared-requirement 의 출처가 트리플렛(값을 선언한 yaml 자신)뿐 = HINT_VALUE_STATUS_REQUIREMENT_UNGROUNDED",
           "HINT_VALUE_STATUS_REQUIREMENT_UNGROUNDED" in lint_v(variant("drt", with_triplet)))
        ph_facts = json.loads(core.dumps(facts_json))
        ph_facts["reproduce_steps"][1]["command"] = "adversarial-benchmark 셀 스윕 — 명령 원문은 03-benchmark.md §3.x"
        ck("★사실 블록의 절 자리표시(§3.x) = HINT_FACT_PLACEHOLDER", "HINT_FACT_PLACEHOLDER" in lint_v(payload, facts=ph_facts))
        rs_facts = json.loads(core.dumps(facts_json))
        rs_facts["applied_set"]["patches"][0]["result_source"] = "guessed"
        ck("★결과 출처 어휘 밖 = HINT_RESULT_SOURCE_UNKNOWN", "HINT_RESULT_SOURCE_UNKNOWN" in lint_v(payload, facts=rs_facts))
        t00 = (payload / "00-hint.md").read_text(encoding="utf-8")
        t01x = (payload / "01-artifacts.md").read_text(encoding="utf-8")
        t02x = (payload / "02-narrative.md").read_text(encoding="utf-8")
        t03x = (payload / "03-benchmark.md").read_text(encoding="utf-8")
        ck("0.2 · 0.4: HF revision 행 · 양자화 폴백 라벨", ("c" * 40) in fact_blocks(t00)["context"]
           and ("c" * 40) in fact_blocks(t00)["resolved"] and "명명 축 q(체크포인트 config 파생)" in fact_blocks(t00)["context"])
        ck("0.7 메타: 현행 full 충족(아니오 · 반복 1) · 사실 검증 미실시", "아니오 · 이 측정의 반복 1" in fact_blocks(t00)["meta"]
           and "미실시" in fact_blocks(t00)["meta"])
        w_open = _factcheck_line({"status": "waived", "reason_code": "HINT_FACTCHECK_OPEN", "open": ["F1"], "waiver": "x"})
        w_abs = _factcheck_line({"status": "waived", "reason_code": "HINT_FACTCHECK_ABSENT", "open": [], "waiver": "x"})
        ck(f"★사람 면제 문구 = 면제한 사유대로(열린 지적을 면제하면 '검증 없이' 라 적지 않는다)({w_open[:40]})",
           "열린 채 발행" in w_open and "없이" not in w_open and "독립 검증 보고 없이 발행" in w_abs)
        ck("03 §3.5: 현행 full 정의 원문 그대로 + 충족 아니오", "> full 계측(`lite ∪ GuideLLM × 반복 ≥3`)" in fact_blocks(t03x)["bench_definition"]
           and "| 현행 full 정의 충족 | 아니오 |" in fact_blocks(t03x)["bench_definition"])
        ck("03 §3.5: evidence 의 확장 키(요건 · 도구 · 사유 · 정의 줄)도 싣는다", all(x in _f_bench_definition(_Ctx(repo, dict(
            facts_json, bench_definition={**facts_json["bench_definition"], "source_line": 12, "required_repeats": 3,
                                          "bench_tool": "vllm-bench-serve", "bench_tool_source": "측정 구성 표",
                                          "reasons": ["판정점 반복 1 < 현행 정의 3"]}), tpls)) for x in (
            ".claude/rules/docs.md:12", "| 현행 정의의 반복 요건 | 3 |", "| 이 측정의 도구 | vllm-bench-serve |", "판정점 반복 1 < 현행 정의 3")))
        ev01 = fact_blocks(t01x)["event_timeline"]
        ck("01 §1.4: 이벤트 5행 + 기동 시도 2(첫 시도 renew 없음 · 둘째 1회)", "블랙박스 이벤트 5행" in ev01
           and "| 1 | main | `smoke-fixture-cell` | 2026-09-12T07:43:46Z | 없음 | 없음 | `budget_clear` 2026-09-12T08:14:38Z |" in ev01
           and "| 2 | main | `smoke-fixture-cell` | 2026-09-12T18:54:37Z | 1회 · 첫 2026-09-12T19:24:49Z | 없음 |" in ev01)
        ck("02 §2.2: 기동 시도 요약(행 전문은 01)", "이 셀의 기동 시도" in fact_blocks(t02x)["event_attempts"]
           and "블랙박스 이벤트 5행" not in fact_blocks(t02x)["event_attempts"])
        ck("이벤트 없으면 '미관측' 명시(침묵 ✗)", "미관측" in _f_event_timeline(_Ctx(repo, dict(facts_json, event_timeline=[]), tpls))
           and "미관측" in _f_event_attempts(_Ctx(repo, dict(facts_json, event_timeline=None), tpls)))
        # ── FACT_FIX2 G8 · G2 · G5(2026-09-29): 원장 관측 범위 · 발행 자격 시점 범위 · 실행 안 된 경로의 쓰인 바이트 ──
        spans_ = [{"node": "main", "first_utc": "2026-09-01T00:00:00Z", "last_utc": "2026-09-30T00:00:00Z", "files": ["m.jsonl"],
                   "covers_measurement": True, "measurement_end_utc": "2026-09-23T01:57:09Z"},
                  {"node": "sub", "first_utc": "2026-09-03T00:00:00Z", "last_utc": "2026-09-11T11:10:38Z", "files": ["s.jsonl"],
                   "covers_measurement": False, "measurement_end_utc": "2026-09-23T01:57:09Z"}]
        et8 = _f_event_timeline(_Ctx(repo, dict(facts_json, event_ledger_spans=spans_), tpls))
        ck("G8 이벤트 표에 원장 관측 범위 · 범위 끝 < 측정 끝인 노드 = '관측 범위 밖'(없음 ✗) · 기동 시도 요약에도",
           "원장 관측 범위" in et8 and "sub = 2026-09-03T00:00:00Z → 2026-09-11T11:10:38Z" in et8 and "관측 범위 밖" in et8
           and "관측 범위 밖" in _f_event_attempts(_Ctx(repo, dict(facts_json, event_ledger_spans=spans_), tpls)))
        ck("★G8 음성대조: 모든 노드가 측정 끝을 덮으면 경고 없음 · 범위 입력이 없으면 줄 없음",
           "관측 범위 밖" not in _f_event_timeline(_Ctx(repo, dict(facts_json, event_ledger_spans=spans_[:1]), tpls))
           and "원장 관측 범위" not in _f_event_timeline(_Ctx(repo, dict(facts_json, event_ledger_spans=None), tpls)))
        q2 = {"health_200": True, "inference_observed": True, "sources": ["output/multi/benchlog/serve_proof_x.json"],
              "scope": {"written_utc": "2026-09-28T01:31:48Z", "written_utc_source": "파일 mtime", "timing": "after-measurement",
                        "timing_note": "작성 2026-09-28T01:31:48Z > 측정 끝 2026-09-28T00:35:13Z — 측정 뒤 재기동의 스모크 산출물"}}
        ck("G2 발행 자격 근거(FACT:grade · 재현 끝 줄)에 serve_proof 시점 범위",
           "측정 뒤 재기동의 스모크 산출물" in _qualification_line({"qualification": q2})
           and "측정 뒤 재기동의 스모크 산출물" in _qual_scope_text(q2))
        ck("★G2 음성대조: 범위가 없으면 근거 칸에 덧붙이지 않는다", _qual_scope_text({**q2, "scope": None}) == "")
        rl5 = _revision_line("compose", {"path": "x.sh", "commit": "a" * 40, "method": "worktree", "basis": "mtime≤measured",
                                         "verified": None, "executed": False})
        ck("G5 실행 안 된 경로 = '쓰인 바이트임을 관측으로 확인 해당 없음(실행 안 됨)'", "확인 해당 없음(실행 안 됨)" in rl5
           and "확인 예" not in rl5)
        ck("★G5 음성대조: 실행된 경로(verified 불리언)는 예/아니오 그대로",
           "확인 예" in _revision_line("compose", {"path": "x.sh", "method": "worktree", "verified": True}))
        # 2026-09-22 · plan_26092119 S2 round 2 통합: 빈 타임라인 문구는 생산자가 하지 않는 구분(원장 부재 대 행 0)을 했다고 말하지 않는다 —
        #   evidence.event_timeline 은 두 경우 모두 [] 다(음성대조: 옛 문구 "evidence 가 가른다" 가 돌아오면 RED).
        empty_ev = _f_event_timeline(_Ctx(repo, dict(facts_json, event_timeline=[]), tpls))
        ck("★빈 타임라인 = '구분하지 않는다' 로 정직하게(옛 'evidence 가 가른다' ✗)",
           "구분하지 않는다" in empty_ev and "evidence 가 가른다" not in empty_ev)
        ck("선언 행 없이 시작한 시도도 행으로(끊긴 원장)", "—(선언 행 없음)" in "\n".join(_attempts_table(
            [{"utc": "2026-01-01T00:00:00Z", "node": "main", "kind": "watchdog_trip", "label": None}])))
        # 2026-09-22 적대 리뷰: kind 는 정확 일치로 가른다(실 원장의 budget_declare_rejected · thermal_gpu_stale_clear) · 사살은
        #   뒤따르는 clear 에 덮이지 않는다 · budget_expired 는 닫힘이다
        exact = [{"utc": "2026-01-01T00:00:00Z", "node": "main", "kind": "budget_declare", "label": "smoke-x"},
                 {"utc": "2026-01-01T00:02:00Z", "node": "main", "kind": "budget_declare_rejected", "label": "smoke-x"},
                 {"utc": "2026-01-01T00:03:00Z", "node": "main", "kind": "thermal_gpu_stale_clear", "label": None},
                 {"utc": "2026-01-01T00:04:00Z", "node": "main", "kind": "watchdog_kill_ack", "label": None},
                 {"utc": "2026-01-01T00:05:00Z", "node": "main", "kind": "budget_clear", "label": None},
                 {"utc": "2026-01-01T01:00:00Z", "node": "main", "kind": "budget_declare", "label": "smoke-x"},
                 {"utc": "2026-01-01T01:30:00Z", "node": "main", "kind": "budget_expired", "label": None}]
        ae = _attempts(exact)
        tbl = "\n".join(_attempts_table(exact))
        ck(f"★거부된 선언(budget_declare_rejected)은 새 시도가 아니다 · stale_clear 는 닫힘이 아니다 · 사살은 clear 에 덮이지 않는다 · "
           f"expired = 닫힘({ae})", len(ae) == 2 and ae[0]["alerts"] == {"budget_declare_rejected": 1, "watchdog_kill_ack": 1}
           and ae[0]["end"] == "budget_clear" and ae[0]["other"] == 1 and ae[1]["end"] == "budget_expired"
           and "`budget_declare_rejected` 1 · `watchdog_kill_ack` 1" in tbl)
        mixed = [{"utc": "2026-01-01T00:00:00Z", "node": "main", "kind": "budget_declare", "label": "smoke-x"},
                 {"utc": "2026-01-01T00:05:00Z", "node": "main", "kind": "engine_init_done", "label": None},
                 {"utc": "2026-01-01T00:09:00Z", "node": "main", "kind": "budget_clear", "label": None},
                 {"utc": None, "node": "main", "kind": "ledger_unparsable_lines", "label": None}]
        am = _attempts(mixed)
        ck(f"★라벨 없는 clear · 엔진 표지 · 판독 불가 행은 같은 노드의 현재 시도에 붙는다(라벨로 쪼개지 않는다)({am})",
           len(am) == 1 and am[0]["end"] == "budget_clear" and am[0]["other"] == 2 and am[0]["label"] == "smoke-x")
        cross = [{"utc": "2026-01-01T00:00:00Z", "node": "main", "kind": "budget_declare", "label": "smoke-x"},
                 {"utc": "2026-01-01T00:01:00Z", "node": "cluster", "kind": "engine_log_window_start", "label": None},
                 {"utc": "2026-01-01T00:09:00Z", "node": "main", "kind": "budget_clear", "label": None},
                 {"utc": "2026-01-01T01:00:00Z", "node": "main", "kind": "budget_declare", "label": "smoke-x"},
                 {"utc": "2026-01-01T01:05:00Z", "node": "cluster", "kind": "engine_init_done", "label": None},
                 {"utc": "2026-01-01T01:06:00Z", "node": "main", "kind": "budget_renew", "label": "smoke-x"}]
        ac = _attempts(cross)
        ck(f"★선언 없는 노드(cluster 엔진 표지)의 행은 그 시각의 최근 시도에 붙는다 — 셋째 '시도' ✗(2026-09-22 태그2 실측)({ac})",
           len(ac) == 2 and [a["other"] for a in ac] == [1, 1] and ac[1]["renew"] == ["2026-01-01T01:06:00Z"])
        # 2026-09-22 S2 round 3 통합: 측정 기록 행(evidence `measurement` · node None)은 시도를 열지도 · 시도에 붙지도 않는다
        #   (태그2 재생: 09-10 측정 행이 노드 '미관측' 가짜 시도 1번을 열어 시도 3개로 보였다 · 원장 선언은 2건).
        meas = [{"utc": "2025-12-31T00:00:00Z", "node": None, "kind": "measurement", "label": "measurement (no budget event recorded)"},
                *cross[:3],
                {"utc": "2026-01-01T00:30:00Z", "node": None, "kind": "measurement", "label": "measurement (ledger unobserved)"},
                *cross[3:]]
        amx = _attempts(meas)
        ck(f"★측정 기록 행은 가짜 시도를 열지 않고 시도의 '그 밖 행' 에도 붙지 않는다({amx})",
           len(amx) == 2 and [a["other"] for a in amx] == [1, 1] and all(a["node"] == "main" for a in amx)
           and "measurement (" not in "\n".join(_attempts_table(meas)))
        ck("★측정 기록 행만 있으면 시도 표 = 미관측(가짜 시도 ✗)",
           "기동 시도 미관측" in "\n".join(_attempts_table(meas[:1])))
        ck("1.1 결과 출처 범례(쓰인 값만)", "**결과 출처 범례**" in t01x and "`reconstructed(docker-history+gate)` —" in t01x
           and "`image-probe(script-sha+marker)`" not in t01x)
        ck("0.5 파생 요약: 지위별 개수", "`inherited` 2 · `negative-control` 1" in fact_blocks(t00)["knob_status"])
        obs_facts = json.loads(core.dumps(facts_json))
        obs_facts["reproduce_steps"][1]["observed"] = {"start_utc": "2026-09-12T18:54:37Z", "end_utc": "2026-09-12T19:25:31Z",
                                                       "seconds": 1854, "bound": "upper"}
        ck("1.4 재현 표: 관측 구간 칸(start → end (bound))",
           "2026-09-12T18:54:37Z → 2026-09-12T19:25:31Z (upper)" in _f_reproduce(_Ctx(repo, obs_facts, tpls))
           and "| 관측 구간(UTC) |" in _f_reproduce(_Ctx(repo, facts_json, tpls)))
        ck("★PII 리터럴", "HINT_PII" in lint_v(variant("pii", edit("02-narrative.md", "- moe-backend 를", f"- {term} 가 moe-backend 를"))))
        ck("★brief 4줄", "HINT_BRIEF_TOO_LONG" in lint_v(variant("b4", edit("00-hint.md", "복사하지 마라.", "복사하지 마라.\n넷째 줄."))))
        ck("★brief 인용 시작", "HINT_BRIEF_SHAPE" in lint_v(variant("bq", edit("00-hint.md", "fixture-model 을 GB10", "> fixture-model 을 GB10"))))
        ck("★PROMPT 지시 변조", "HINT_PROMPT_TAMPERED" in lint_v(variant("pt", edit("02-narrative.md", "블록: wall>=1\n", ""))))
        ck("★kind 자리 위반", "HINT_EVENT_KIND_MISPLACED" in lint_v(variant("km", edit(
            "02-narrative.md", "- moe-backend 를 명시하지 마라", "```hint-event\nid: R9\nkind: rejected\n시도: a\n기각사유: b\n"
            "출처: [testlog_26090921 §2]\n```\n- moe-backend 를 명시하지 마라"))))
        ck("★id 중복", "HINT_EVENT_ID_DUPLICATE" in lint_v(variant("dup", edit("02-narrative.md", "id: W2", "id: W1"))))
        ck("★id 모양(kind 접두)", "HINT_EVENT_ID_SHAPE" in lint_v(variant("ids", edit("02-narrative.md", "id: R1\nkind: rejected\n시도: moe", "id: Q7\nkind: rejected\n시도: moe"))))
        ck("★필수 발췌 부재", "HINT_EXCERPT_REQUIRED" in lint_v(variant("er", edit(
            "02-narrative.md", "> [원문] plan_26090918 §1\n> GB10 두 노드에서 fixture-model 을 262k 컨텍스트로 서빙한다.", "첫 plan 요약."))))
        ck("★모르는 챕터", "HINT_SECTION_UNKNOWN" in lint_v(variant("sec", edit("02-narrative.md", "## 2.7 열린 물음", "## 2.8 추가\n\n## 2.7 열린 물음"))))
        ck("★README 손수정", "HINT_README_DRIFT" in lint_v(variant("rd", edit(README_NAME, "## 읽는 순서", "## 읽는 차례"))))
        ck("★스냅샷 부재", "HINT_FACT_SNAPSHOT_ABSENT" in codes(lint(repo, variant("ns", lambda p: None), **kw)))
        ck("★PII 용어 파일 부재", "HINT_MISSING_PII_TERMS" in lint_v(payload, pii_terms=None))
        ck("★치환 함수 부재", "HINT_EXCERPT_VERIFIER_ABSENT" in lint_v(payload, substitute=None, subst_table=None))
        # ── 2026-09-22 리뷰 추가 음성대조 ──
        bn_dir = variant("bn-cli", edit("03-benchmark.md", bench_row, "| 1 ★판정점 | 38.18 |"))
        ck("★CLI --verify 도 숫자 변조를 잡는다", cli_verify(bn_dir, repo / rep_p) == 1)
        ck("★절 제목이 앞쪽에 한 번 더(CLI 가 첫 줄을 잡는다)", "HINT_BENCH_SECTION_DRIFT" in lint_v(variant("bt2", edit(
            "03-benchmark.md", "## 3.1 측정 결과\n", "## 3.1 측정 결과\n" + bench_md.split("\n")[0] + "\n"))))
        no_cands = dict(facts_json, value_status_candidates=[])
        ck("★value-status 후보 0개(트리플렛 부재 미기재) = 공허 통과 ✗",
           "HINT_VALUE_STATUS_CANDIDATES_ABSENT" in lint_v(payload, facts=no_cands))
        ck("value-status 후보 0개 + 트리플렛 부재 기재 = 커버리지 면제", "HINT_VALUE_STATUS_CANDIDATES_ABSENT" not in lint_v(
            payload, facts=dict(no_cands, missing={"HINT_MISSING_TRIPLET": "트리플렛 부재"})))
        ck("★미등록 결손 코드", "HINT_MISSING_CODE_UNREGISTERED" in lint_v(payload, facts=dict(facts_json, missing=["HINT_NOT_A_CODE"])))
        ck("★task_class facts 누락(manifest 만 선언)", "HINT_TASK_CLASS_CONFLICT" in lint_v(
            payload, facts=dict(facts_json, task_class=None, publication={"topic": "camp_fixture"})))
        ck("★perf_waiver facts 누락(manifest 만 선언)", "HINT_PERF_WAIVER_CONFLICT" in lint_v(
            payload, manifest={"task_class": "full_benchmark", "benchmark": {"perf_waiver": {"warning_flag": "기대 이하"}}}))
        ck("★brief 문단 안의 주석 경계(봉인 뒤 tag 가 거부하기 전에 lint 가 막는다)", "HINT_BRIEF_SHAPE" in lint_v(variant(
            "bcm", edit("00-hint.md", "복사하지 마라.", "복사하지 마라. <!-- 메모 -->"))))
        bl = variant("bcl", edit("00-hint.md", "t/s.\n가장 비싼 벽은", "t/s.\n<!-- 메모 -->\n가장 비싼 벽은"))
        ck("brief 는 주석 줄에서 끝난다(주석을 brief 로 캐지 않는다)", brief(bl).endswith("t/s.") and "<!--" not in brief(bl))
        ck("03 인증서 부재 = '판정되지 않았다' 배너 + 측정 결손 표(옛 render_item3 · selftest_hint_gate ①)",
           "판정되지 않았다" in _f_bench_missing(_Ctx(repo, dict(facts_json, missing=["HINT_MISSING_CERTIFICATE"]), tpls))
           and "`HINT_MISSING_CERTIFICATE`" in _f_bench_missing(_Ctx(repo, dict(facts_json, missing=["HINT_MISSING_CERTIFICATE"]), tpls)))
        warn_facts = dict(facts_json, slots={"build_recipe": {"files": ["artifacts/build_recipe/Dockerfile.source-build"],
                                                             "evidence": {"kind": "file", "ref": "cell-env",
                                                                          "recipe_vs_image": [{"path": "output/multi/Dockerfile.source-build",
                                                                                               "warning": "작업트리 수정본(미커밋)"}],
                                                                          "context_not_shipped": [{"source": "x/*", "why": "글롭"}],
                                                                          "selected_revision": {"commit": "d" * 40, "method": "docker-history-match",
                                                                                                "history_match": {"matched": 12, "total": 12}}}},
                                         "compose": {"files": ["artifacts/compose/.env.interconnect.template"],
                                                     "evidence": {"kind": "file", "ref": "render",
                                                                  "post_measurement_regenerated": ["output/multi/envs/.env.interconnect"]}}},
                          applied_set=dict(facts_json["applied_set"], probes=[{"tier": "attestation", "result": "absent"},
                                                                               {"tier": "local-image", "image": "sha256:ab",
                                                                                "result": "no-ledger(cat rc=1)"}]))
        wctx = _Ctx(repo, warn_facts, tpls)
        ck("슬롯 표: 레시피 대 이미지 경고 · 싣지 못한 COPY 원천을 본문에 싣는다", "⚠ `build_recipe`" in _f_slots(wctx)
           and "작업트리 수정본" in _f_slots(wctx) and "COPY 원천 `x/*` 미실림" in _f_slots(wctx))
        ck("적용 집합: 원장 탐침 표(조용한 강등 ✗)", "**원장 탐침**" in _f_applied_set(wctx) and "no-ledger(cat rc=1)" in _f_applied_set(wctx))
        ck("슬롯 표: 선택 리비전(docker history 대조) · 측정 뒤 재생성 파일 경고(S2 round 2)",
           f"커밋 `{'d' * 12}` · 선택 방법 `docker-history-match` · docker history 대조 12/12" in _f_slots(wctx)
           and "측정 뒤 재생성된 파일(mtime > 측정 시각)을 실었다: `output/multi/envs/.env.interconnect`" in _f_slots(wctx))
        ck("경고·탐침 없으면 표만(기존 본문 불변)", "레시피 경고" not in _f_slots(_Ctx(repo, facts_json, tpls))
           and "원장 탐침" not in _f_applied_set(_Ctx(repo, facts_json, tpls)))
        # ── 2026-09-22 · plan_26092119 S2 round 3: 공유 사실 계약의 새 키 렌더(생산자 모양 그대로 · 판정 ✗) ──
        r3 = json.loads(core.dumps(facts_json))
        r3["slots"] = {
            "build_recipe": {"files": ["artifacts/build_recipe/Dockerfile.source-build", "artifacts/build_recipe/requirements.txt"],
                             "evidence": {"kind": "file", "ref": "cell-env",
                                          "selected_revision": {"path": "output/multi/Dockerfile.source-build", "commit": "d" * 40,
                                                                "method": "git log -1 --before", "history_match": {"matched": 16, "total": 16}},
                                          "selected_revisions": [
                                              {"path": "output/multi/Dockerfile.source-build", "commit": "d" * 40, "method": "git log -1 --before",
                                               "history_match": {"matched": 16, "total": 16}, "shipped": "git:" + "d" * 40, "verified": True},
                                              {"path": "output/multi/requirements.txt", "commit": None, "method": "worktree(추적 개정과 동일)",
                                               "history_match": None, "shipped": "worktree", "verified": False}]}},
            "compose": {"files": ["artifacts/compose/docker-compose.yaml", "artifacts/compose/serve_runner.sh"],
                        "evidence": {"kind": "file", "ref": "output/multi/docker-compose.yaml",
                                     "selected_revisions": [{"path": "output/multi/docker-compose.yaml", "commit": "e" * 40,
                                                             "method": "git log -1 --before=2026-09-12T19:32:52Z", "history_match": None,
                                                             "shipped": "git:" + "e" * 40, "verified": None,
                                                             "basis": "git-rev-before-measurement",
                                                             "commits_after_measurement": ["a" * 40, "b" * 40],
                                                             "warning": "작업트리가 측정 뒤 수정됐다(주석)"}],
                                     "revision_warnings": ["output/multi/docker-compose.yaml: 작업트리가 측정 뒤 수정됐다(주석)"],
                                     "post_measurement_regenerated": ["output/multi/envs/.env.interconnect"]},
                        "excluded": [{"file": ".env.interconnect", "reason": "post-measurement-regenerated",
                                      "mtime_utc": "2026-09-17T10:57:56Z", "measured_source": "certificate.measured_utc",
                                      "post_measurement_regenerated": True},
                                     {"file": "arm_patch.sh", "why": "not-armed(런타임 패치 없음)"}]},
            "build_patch_pre": {"files": ["artifacts/build_patch_pre/60-fixture-patch.sh"],
                                "evidence": {"kind": "reconstruction", "ref": "no-build-ledger"},
                                "excluded": [{"phase": "pre", "file": ".gitkeep", "why": "known-non-patch(패치가 아님 — 명시적 건너뜀)"},
                                             {"phase": "pre", "file": "files/", "why": "known-non-patch(패치가 아님 — 명시적 건너뜀)"},
                                             {"phase": "pre", "file": "50-skip-patch.sh", "why": "skipped(SM12X_PORT=0)"}]}}
        r3["applied_set"] = dict(r3["applied_set"], probes=[{"tier": "attestation", "ref": "output/multi/benchlog/attestation_build.json",
                                                             "result": "no-ledger"}])
        r3["applied_set"]["patches"][0].update(
            evidence={"static": ["skip 경로 없음"], "loop": "docker history 루프 `/tmp/p/*.sh … || exit 1`",
                      "markers": {"status": "all-found", "declared": 2, "found": 2}},
            script_identity="image-probe(sha256 일치 — 실린 바이트 = 이미지에 구워진 바이트)", image_script_sha256="f" * 64)
        r3["attestation_scope"] = {"config": "fixture-build", "written_utc": "2026-09-09T20:00:00Z", "written_utc_source": "파일 mtime",
                                   "bound_by": "이미지 compose 라벨 fixture-build", "phase": "smoke",
                                   "path": "output/multi/benchlog/attestation_fixture-build.json",
                                   "scope": "image-build/smoke — not this cell's run"}
        r3["reproduce_steps"][1]["observed"] = {"start_utc": "2026-09-09T13:40:00Z", "end_utc": "2026-09-09T14:24:35Z",
                                                "seconds": 2675, "bound": "layer-span",
                                                "layer_span": {"oldest_utc": "2026-09-09T13:40:00Z", "newest_utc": "2026-09-09T14:24:35Z",
                                                               "layers": 28, "source": "docker history", "note": "캐시 층 포함"}}
        r3["measurement_env_observed"] = [
            {"key": "NCCL_IB_DISABLE", "value": "0", "node": "main", "source": f"{log_p}:L1"},
            {"key": "NCCL_IB_DISABLE", "value": "0", "node": "sub", "source": f"{log_p}:L2"},
            {"key": "NCCL_NET_GDR_LEVEL", "value": "SYS", "node": "main", "source": f"{log_p}:L1"},
            {"key": "NCCL_NET_GDR_LEVEL", "value": "LOC", "node": "sub", "source": f"{log_p}:L2"},
            {"key": "NCCL_SOCKET_IFNAME", "value": "<nic:cluster>", "node": "main", "source": f"{log_p}:L1", "kind": "env-echo",
             "occurrences": 3, "seen_in_logs": 2, "value_note": "기계 치환(자리표시)"},
            {"key": "NCCL_DEBUG", "value": "", "node": None, "node_label": "cluster", "source": f"{log_p}:L3"}]
        r3["tool_snapshots"] = [{"name": "run_bench.sh", "repo_path": ".claude/skills/adversarial-benchmark/scripts/run_bench.sh",
                                 "git_rev": "0123456789ab" + "c" * 28, "snapshot_rel": "inputs/sources/run_bench.sh@0123456789ab",
                                 "role": "bench", "why": "sweep_bench.sh 가 레벨마다 부른다", "git_rev_utc": "2026-09-05T01:00:00Z",
                                 "next_rev": "9" * 40, "next_rev_utc": "2026-09-14T02:00:00Z",
                                 "worktree_note": "측정 시각의 워킹트리는 관측 대상 밖", "driver_note": None}]
        r3["naming"]["vllm_observed"] = {
            "engine_self_report": None, "engine_self_report_source": "같은 digest 의 엔진 로그 기동 배너 없음(benchlog 3개 탐색)",
            "engine_self_report_basis": "unobserved",
            "certificate_vllm_version": "0.29.0", "certificate_vllm_version_producer": "image-tag-line",
            "certificate_vllm_version_source": "sweep_bench.sh@8ca23d4 L337-340 — IMAGE_TAG 에서 x.y.z 로 자른 값(엔진 자기보고 아님)",
            "wheel_meta": None, "source": "인증서 vllm_version"}
        c3 = _Ctx(repo, r3, tpls)
        s3, a3, rp3 = _f_slots(c3), _f_applied_set(c3), _f_reproduce(c3)
        ck("R3 슬롯: 복수 리비전(build_recipe 2 · compose 1) 전부 · 단수는 복수에 있으면 한 번만",
           s3.count("`output/multi/Dockerfile.source-build` 실린 리비전") == 1 and "`output/multi/requirements.txt` 실린 리비전" in s3
           and f"`compose` `output/multi/docker-compose.yaml` 실린 리비전: 커밋 `{'e' * 12}`" in s3 and "docker history 대조 없음" in s3)
        ck("R3 슬롯: compose 추적 파일 근거(basis) · 측정 뒤 커밋 수 · 파일 경고 · 슬롯 리비전 경고",
           "근거 `git-rev-before-measurement`" in s3 and "측정 뒤 커밋 2건" in s3 and "— ⚠ 작업트리가 측정 뒤 수정됐다(주석)" in s3
           and "- ⚠ `compose` 리비전 경고: output/multi/docker-compose.yaml: 작업트리가 측정 뒤 수정됐다(주석)" in s3)
        ck("R3 슬롯: 싣지 않은 파일 — post-measurement-regenerated 뜻 · 추가 칸(mtime) · not-armed 사유",
           "`compose` `.env.interconnect` — `post-measurement-regenerated` · 측정 뒤 재생성된 형상" in s3
           and "mtime_utc 2026-09-17T10:57:56Z" in s3 and "`arm_patch.sh` — `not-armed(런타임 패치 없음)`" in s3)
        ck("★R3 슬롯: 제외 행의 추가 칸은 닫힌 목록(mtime · 측정 시각)만 — 생산자 보조 칸(measured_source · 불리언)은 PAYLOAD 몫",
           "measured_source" not in s3 and "post_measurement_regenerated True" not in s3)
        ck("★R3 슬롯: 싣지 않은 파일 목록이 말하는 재생성 파일은 경고 줄에 다시 적지 않는다(두 자리 ✗)",
           "측정 뒤 재생성된 파일" not in s3 and s3.count(".env.interconnect") == 1)
        r3b = json.loads(core.dumps(r3))
        r3b["slots"]["compose"]["excluded"] = []
        s3b = _f_slots(_Ctx(repo, r3b, tpls))
        ck("R3 슬롯: 싣지 않았고 제외 목록에도 없는 재생성 파일 = '싣지 않았다' 줄(옛 '실린 형상' 문구 ✗)",
           "측정 뒤 재생성된 파일(mtime > 측정 시각)은 싣지 않았다: `output/multi/envs/.env.interconnect`" in s3b
           and "실린 형상은 측정 당시 값과 다를 수 있다" not in s3b)
        ck("★_cell: 목록 항목의 `|` 를 두 번 이스케이프하지 않는다", _cell(["a || b", "c"]) == "a \\|\\| b · c")
        ck("★R3 슬롯: 패치 아님은 개수 · 이름만 · skip 행은 적용 집합 표에만(두 자리에 싣지 않는다)",
           "`build_patch_pre` known-non-patch 2건: `.gitkeep` · `files/`" in s3 and "50-skip-patch.sh" not in s3)
        ck("R3 적용 집합: 패치별 판정 근거(스크립트 바이트 정체 · 이미지 sha256 · 표지 · 정적 규칙) + 단계별 빌드 루프 한 줄",
           "**판정 근거**" in a3 and ("`" + "f" * 64 + "`") in a3 and "all-found 2/2" in a3 and "image-probe(sha256 일치" in a3
           and "- `pre` 빌드 루프: docker history 루프" in a3)
        ck("★R3 적용 집합: 근거 키가 없는 패치만 있으면 판정 근거 표 없음(빈 표 ✗)", "**판정 근거**" not in _f_applied_set(
            _Ctx(repo, facts_json, tpls)))
        ck("R3 attestation 범위 줄: 01 §1.1 탐침 · 01 §1.5 ABI · 00 결손(attestation 코드) 세 자리 · 생산자 scope 그대로",
           all("**attestation 범위**" in x and "config `fixture-build`" in x and "**image-build/smoke — not this cell's run**" in x
               and "묶은 근거 이미지 compose 라벨" in x and "phase smoke" in x for x in (a3, _f_sub_recipe(c3), _f_missing(c3))))
        ck("★R3 attestation 범위 없음 = 줄 없음 · 결손에 attestation 코드가 없으면 00 에 붙지 않는다",
           "attestation 범위" not in _f_sub_recipe(_Ctx(repo, facts_json, tpls))
           and "attestation 범위" not in _f_missing(_Ctx(repo, dict(r3, missing={"HINT_MISSING_TRIPLET": "x"}), tpls)))
        ck("R3 재현: 빌드 층 CreatedAt 창(layer_span) 줄", "`build` 이미지 층 CreatedAt(docker history): 2026-09-09T13:40:00Z → "
           "2026-09-09T14:24:35Z · 층 28개" in rp3 and "2026-09-09T13:40:00Z → 2026-09-09T14:24:35Z (layer-span)" in rp3)
        ck("R3 재현: 측정 뒤 재생성 형상이 있으면 render 단계 경고(지금의 렌더러 · 측정 env 관측과 대조)",
           "`render` 단계는 **지금의** 렌더러로" in rp3 and "`output/multi/envs/.env.interconnect`" in rp3)
        ck("★R3 재현: 재생성 형상이 없으면 render 경고 없음(근거 없는 경고 ✗)", "지금의** 렌더러" not in _f_reproduce(
            _Ctx(repo, facts_json, tpls)))
        me3, mn3, ts3 = _f_measurement_env(c3), _f_measurement_env_nodes(c3), _f_tool_snapshots(c3)
        ck("R3 측정 env 관측: 행 전부 · 값 코드 스팬(자리표시) · 빈 값 · 노드 미특정(표지) · 종류 · 횟수 · 값 주",
           "측정 실행의 env 관측 6행" in me3 and "| `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | main | env-echo |" in me3
           and "| `NCCL_DEBUG` | (빈 값) | 미특정(cluster) |" in me3 and f"{log_p}:L2" in me3 and "| 3줄 · 로그 2개 |" in me3
           and "- 값 주: 기계 치환" in me3)
        ck("R3 노드별 env: 양 노드 같음은 이름만 · 다름 · 한쪽만 · 미특정은 행으로",
           "양 노드 같은 값 1개: `NCCL_IB_DISABLE`" in mn3 and "| `NCCL_NET_GDR_LEVEL` | `SYS` | `LOC` | — | 다름 |" in mn3
           and "| `NCCL_SOCKET_IFNAME` | `<nic:cluster>` | — | — | main 만 관측 |" in mn3 and "노드 미특정 |" in mn3)
        ck("★R3 노드별 env: 값이 다른 키는 '같음' 목록에 들지 않는다 · single = 해당 없음",
           "`NCCL_NET_GDR_LEVEL`" not in mn3.split("\n")[2] and _f_measurement_env_nodes(
               _Ctx(repo, dict(r3, identity={**r3["identity"], "topology": "single"}), tpls)).startswith("해당 없음"))
        # 2026-09-22 round 3 적대 리뷰: Ray 가 접은 줄(`ray_repeated`)은 한쪽만 관측을 단언하지 않는다(태그2 라이브 재생 6키 오도) ·
        #   같음은 값 집합으로(로그 순서 무관)
        r3r = dict(r3, measurement_env_observed=[
            {"key": "NCCL_IB_HCA", "value": "=<nic:cluster>", "node": "sub", "source": f"{log_p}:L2", "ray_repeated": 2,
             "seen_in_logs": 6},
            {"key": "NCCL_CROSS_NIC", "value": "1", "node": "main", "source": f"{log_p}:L1"},
            {"key": "NCCL_ALGO", "value": "Ring", "node": "main", "source": f"{log_p}:L1"},
            {"key": "NCCL_ALGO", "value": "Tree", "node": "main", "source": f"{log_p}:L1"},
            {"key": "NCCL_ALGO", "value": "Tree", "node": "sub", "source": f"{log_p}:L2"},
            {"key": "NCCL_ALGO", "value": "Ring", "node": "sub", "source": f"{log_p}:L2"},
            {"key": "NCCL_PROTO", "value": "Simple", "node": "main", "source": f"{log_p}:L1"},
            {"key": "NCCL_PROTO", "value": "Simple", "node": None, "node_label": "cluster", "source": f"{log_p}:L3"}])
        me_r, mn_r = _f_measurement_env(_Ctx(repo, r3r, tpls)), _f_measurement_env_nodes(_Ctx(repo, r3r, tpls))
        hca = next((ln for ln in mn_r.split("\n") if ln.startswith("| `NCCL_IB_HCA`")), "")
        ck("R3 측정 env: Ray 접힘은 관측 칸 + 주(접힌 사본의 노드 미관측) · 접힘 없는 행에는 없다",
           "| 로그 6개 · Ray 접힘 +2(접힌 사본의 노드 미관측) |" in me_r and "- Ray 접힘: 출처 줄 끝의" in me_r
           and "Ray 접힘" not in me3)
        ck(f"★R3 노드별 env: Ray 가 접은 키는 '<노드> 만 관측'(다른 쪽 부재 단언)으로 적지 않는다({hca[-60:]!r})",
           "sub 만 줄에 남음 · Ray 접힘 +2 — main 사본 여부 미관측" in hca and "만 관측 |" not in hca
           and "| `NCCL_CROSS_NIC` | `1` | — | — | main 만 관측 |" in mn_r)
        ck("★R3 노드별 env: 같음 = 값 집합(노드마다 로그 순서가 달라도 같은 집합이면 '같은 값')",
           "양 노드 같은 값 1개: `NCCL_ALGO`" in mn_r and "다름" not in mn_r)
        ck("★R3 노드별 env: 한쪽 + 노드 미특정 줄이면 '만 관측' 으로 다른 쪽 부재를 단언하지 않는다",
           "| `NCCL_PROTO` | `Simple` | — | `Simple` | main 관측 · 노드 미특정 줄 있음 — sub 여부 미관측 |" in mn_r)
        # 2026-09-22 S2 round 3 통합: 꼬리 캡처(evidence log_window)의 한쪽 관측은 다른 쪽 부재를 단언하지 않는다(태그2 = 전 로그 tail-800)
        r3w = dict(r3r, measurement_env_observed=[
            {"key": "NCCL_NET_PLUGIN", "value": "spcx", "node": "sub", "source": f"{log_p}:L2",
             "log_window": "tail-800(lite_bench.sh@0df14da468f9 L125)"},
            {"key": "NCCL_CROSS_NIC", "value": "1", "node": "main", "source": f"{log_p}:L1"}])
        me_w, mn_w = _f_measurement_env(_Ctx(repo, r3w, tpls)), _f_measurement_env_nodes(_Ctx(repo, r3w, tpls))
        ck("R3 측정 env: 로그 창(log_window)을 싣는다 · 창이 없으면 주도 없다",
           "- 로그 창: `tail-800(lite_bench.sh@0df14da468f9 L125)`(이 표의 출처 로그 1개)" in me_w and "로그 창" not in me3)
        ck("★R3 노드별 env: 꼬리 캡처 로그의 한쪽 관측 = '캡처 창에 남음 — 다른 쪽 미관측'(부재 단언 ✗) · 창 없는 로그는 '만 관측' 유지",
           "| `NCCL_NET_PLUGIN` | — | `spcx` | — | sub 만 캡처 창에 남음 — main 여부 미관측(꼬리 캡처) |" in mn_w
           and "| `NCCL_CROSS_NIC` | `1` | — | — | main 만 관측 |" in mn_w)
        ck("★R3 측정 env · 도구 스냅샷 없음 = 미관측을 말한다(침묵 ✗)",
           "env 관측 없음" in _f_measurement_env(_Ctx(repo, facts_json, tpls))
           and "스냅샷 없음" in _f_tool_snapshots(_Ctx(repo, facts_json, tpls)))
        ck("R3 도구 스냅샷: 이름 · 역할 · 경로 · rev12(커밋 UTC) · 다음 변경 · 발췌 토큰 · git show 명령(전체 rev) · 이유 · 주",
           "`run_bench.sh@0123456789ab`" in ts3 and "`0123456789ab` 2026-09-05T01:00:00Z" in ts3 and "| bench |" in ts3
           and f"`{'9' * 12}` 2026-09-14T02:00:00Z" in ts3
           and f"`git show {'0123456789ab' + 'c' * 28}:.claude/skills/adversarial-benchmark/scripts/run_bench.sh`" in ts3
           and "- `run_bench.sh` — sweep_bench.sh 가 레벨마다 부른다" in ts3 and "- 측정 시각의 워킹트리는 관측 대상 밖" in ts3)
        ts_nx = _f_tool_snapshots(_Ctx(repo, dict(r3, tool_snapshots=[dict(r3["tool_snapshots"][0], next_rev=None,
                                                                            next_rev_utc=None)]), tpls))
        ck("★R3 도구 스냅샷: 다음 변경 None = '관측 없음' + 뜻(옛 '없음' = 변경 없음 단언 ✗) · 다음 변경이 있으면 그 주도 없다",
           "| 관측 없음 |" in ts_nx and "'도구가 바뀌지 않았다' 는 뜻이 아니다" in ts_nx and "| 없음 |" not in ts_nx
           and "관측 없음" not in ts3)
        ck("★R3 도구 스냅샷: `다시 얻기` 는 원격 도달을 약속하지 않는다(그 커밋을 가진 클론의 명령이라고 적는다)",
           "원격 도달은 이 표가 판정하지 않았다" in ts3)
        # 2026-09-22 S2 round 3 통합: next_rev None 의 뜻은 생산자의 next_rev_basis 가 말한다(두 뜻을 한 문구로 접지 않는다)
        def _ts_basis(b):
            return _f_tool_snapshots(_Ctx(repo, dict(r3, tool_snapshots=[dict(r3["tool_snapshots"][0], next_rev=None,
                                                                                next_rev_utc=None, next_rev_basis=b)]), tpls))
        ts_same = _ts_basis("0123456789ab..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트)")
        ts_nd = _ts_basis("미관측 — 지금 HEAD 가 측정 시점 커밋 0123456789ab 의 후손이 아니다(다른 브랜치에서 발행)")
        ck("★R3 도구 스냅샷: next_rev_basis 가 있으면 그 뜻을 싣는다(변경 없음 대 후손 아님 — 두 칸이 갈린다 · 일반 '관측 없음' 주 ✗)",
           "| 0123456789ab..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) |" in ts_same
           and "후손이 아니다(다른 브랜치에서 발행) |" in ts_nd and "| 관측 없음 |" not in ts_same + ts_nd
           and "'도구가 바뀌지 않았다' 는 뜻이 아니다" not in ts_same)
        r0 = dict(_vllm_observed_rows(r3["naming"]["vllm_observed"])[i][:2] for i in range(3))
        rsrc = {x[0]: x[2] for x in _vllm_observed_rows(r3["naming"]["vllm_observed"])}
        ck(f"R3 0.4 vLLM 관측: 키마다 생산자 · 인증서 값을 엔진 자기보고로 적지 않는다({r0})",
           r0.get("vLLM 엔진 자기보고(엔진 로그 기동 배너)") == _MISSING_TEXT and r0.get("인증서 `vllm_version`(강한 일치 키)") == "0.29.0"
           and "IMAGE_TAG" in rsrc["인증서 `vllm_version`(강한 일치 키)"]
           and rsrc["인증서 `vllm_version`(강한 일치 키)"].startswith("생산자 `image-tag-line` · sweep_bench.sh@8ca23d4")
           and rsrc["vLLM 엔진 자기보고(엔진 로그 기동 배너)"].startswith("같은 digest")
           and rsrc["vLLM 엔진 자기보고(엔진 로그 기동 배너)"].endswith("지위 `unobserved`") and len(rsrc) == 3)
        ck("★R3 0.4 vLLM 관측: 빈 값에 옛 단일 source(있는 값의 출처)를 붙이지 않는다",
           rsrc["wheel 메타(`vllm_build`)"] == _MISSING_TEXT)
        ck("★R3 0.2 라벨: 측정 강식별 키(옛 '엔진 자기보고 vLLM' ✗)", "엔진 자기보고 vLLM" not in _f_context(c3)
           and "vLLM(측정 강식별 키 `vllm_version`)" in _f_context(c3))
        r3v = json.loads(core.dumps(r3))
        r3v["naming"]["vllm_observed"]["certificate_vllm_version"] = "0.30.0"
        ck("★R3 0.2 vLLM 행: 같은 값이면 0.4 의 생산자(image-tag-line)를 가리키고 · 값이 다르면 가리키지 않는다(거짓 출처 ✗)",
           "생산자 `image-tag-line`(0.4 표" in _f_context(c3) and "생산자 `image-tag-line`" not in _f_context(_Ctx(repo, r3v, tpls)))
        ck("R3 범례 = CURRENT_STATUS_MEANINGS(원래 주장의 지위) · 02 §2.4 PROMPT 도 같은 뜻",
           all(f"**{k}**({v})" in _legend_md() for k, v in CURRENT_STATUS_MEANINGS.items())
           and "원래 주장" in _legend_md() and all(k in tpls["02-narrative"].text for k in CURRENT_STATUS_MEANINGS)
           and "현재지위 = **원래 주장**" in tpls["02-narrative"].text)
        p35 = {p.chapter: p for p in tpls["03-benchmark"].prompts}["3.5"].fields
        ck("★R3 3.5: acceptance length = 출력(비로 비교) · spec 켜짐/꺼짐 = 조건 · 이전 측정 레벨 전부 나란히(옛 '상태와 acceptance length' ✗)",
           "acceptance length 는 비교 조건이 아니라 측정 결과(출력)다" in p35["기재"] and "spec decode 켜짐/꺼짐과 방식" in p35["기재"]
           and "spec decode 상태와 acceptance length" not in p35["기재"] and "레벨 전부" in p35["기재"])
        mech = [({p.chapter: p for p in tpls[d].prompts}[ch].fields) for d, ch in (("00-hint", "0.2"), ("02-narrative", "2.5"))]
        ck("★R3 0.2 · 2.5: 버티는 기전 대 실패 기전 + 지위(확정 · 가설(강등) · 미결)", all(
            "확정 · 가설(강등) · 미결" in fl["기재"] and "기전" in fl["질문"] for fl in mech))
        ck("★R3 2.4: 교정 묶음은 이 셀에 영향을 준 항목을 id 로", "항목을 **id 로** 나열한다" in {
            p.chapter: p for p in tpls["02-narrative"].prompts}["2.4"].fields["기재"])
        sub_fn = lambda t: t.replace("RoCE", "<interconnect>")
        st_txt = substituted_source(repo, payload, "plan_26090918", lineage=lineage, substitute=sub_fn, resolve_stem=fixture_resolve)
        sub_line = next((ln for ln in st_txt.split("\n") if "<interconnect>" in ln), "")
        first_ex = "> GB10 두 노드에서 fixture-model 을 262k 컨텍스트로 서빙한다."
        rt = variant("rt", edit("02-narrative.md", first_ex, first_ex + "\n> " + sub_line))
        ck("발췌 도우미(치환 후 원문)에서 옮긴 줄은 무결성 통과", bool(sub_line) and not (
            {"HINT_EXCERPT_MISMATCH", "HINT_PII"} & lint_v(rt, substitute=sub_fn)))
        wrong = variant("rtw", edit("02-narrative.md", first_ex, first_ex + "\n> " + sub_line.replace("<interconnect>", "<host>")))
        ck("★자리표시를 추측하면(도우미 미사용) 무결성 불일치", "HINT_EXCERPT_MISMATCH" in lint_v(wrong, substitute=sub_fn))
        ck("★발췌 도우미: LINEAGE 밖 출처", raises(lambda: substituted_source(
            repo, payload, "testlog_29999999", lineage=lineage, substitute=sub_fn, resolve_stem=fixture_resolve),
            "HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE"))
        ck("★발췌 도우미: 치환 함수 부재", raises(lambda: substituted_source(
            repo, payload, "plan_26090918", lineage=lineage, resolve_stem=fixture_resolve), "HINT_EXCERPT_VERIFIER_ABSENT"))
        ck("발췌 도우미: artifacts 파일명", "PLE 게이트 완화" in substituted_source(
            repo, payload, "60-fixture-patch.sh", lineage=lineage, substitute=sub_fn, resolve_stem=fixture_resolve))
        half = variant("half", edit("03-benchmark.md", "## 3.5 like-with-like\n", "## 3.5 like-with-like\n<<AGENT: 미저작>>\n"))
        ck("★봉인 거부 = 부수효과 0(앞 문서도 봉인하지 않는다)", raises(lambda: seal_prompts(half), "HINT_AGENT_PLACEHOLDER_RESIDUE")
           and "<!-- PROMPT" in (half / "00-hint.md").read_text(encoding="utf-8"))

        # update_facts(continue 의 승인·슬롯 사유 반영) — 기계 사실 블록만 다시 쓰고 lint 는 그대로 통과한다
        uf = variant("uf", lambda p: None)
        shutil.copytree(default_snapshot_path(payload).parent, uf.parent / "inputs")
        before_00 = (uf / "00-hint.md").read_text(encoding="utf-8")
        new_appr = {"approved_by": "사용자(발화 전사) — 갱신", "approved_utc": "2026-09-21T11:11:11Z", "source": "cli:--approved-by"}
        ch = update_facts(repo, uf, {"approval": new_appr})
        after_00 = (uf / "00-hint.md").read_text(encoding="utf-8")
        ck(f"update_facts — 승인 변경은 meta 블록만 다시 쓴다({ch})", ch == ["meta"] and "사용자(발화 전사) — 갱신" in after_00
           and before_00.split("<!-- FACT:meta -->")[0] == after_00.split("<!-- FACT:meta -->")[0])
        ck("update_facts — 멱등(같은 변경 = 쓰기 0)", update_facts(repo, uf, {"approval": new_appr}) == [])
        fence_h3 = variant("h3", edit("01-artifacts.md", "### 60-fixture-patch.sh", "```\n### 60-fixture-patch.sh\n```"))
        ck("★subsections: 코드 펜스 안 `###` 는 소제목이 아니다", subsections(fence_h3, "01-artifacts", "1.2") == [])
        uf_issues = lint(repo, uf, **kw)
        ck(f"update_facts 뒤 lint 통과(스냅샷 = 새 사실): {uf_issues[:3]}", uf_issues == [])
        ck("★update_facts 없이 스냅샷만 바꾸면 사실 드리프트(기계 블록은 스냅샷에서 재생성된다)", "HINT_FACT_DRIFT" in lint_v(
            uf, facts=dict(facts_json, approval={**new_appr, "approved_by": "다른 발화"})))

        # 봉인
        sealed_dir = variant("sealed", lambda p: None)
        shutil.copytree(default_snapshot_path(payload).parent, sealed_dir.parent / "inputs")
        b0 = brief(sealed_dir)
        seal_prompts(sealed_dir)
        seal_prompts(sealed_dir)
        st = {n: (sealed_dir / n).read_text(encoding="utf-8") for n in PAYLOAD_DOCS}
        ck("봉인 = PROMPT 0 · 질문 줄", all("<!-- PROMPT" not in t for t in st.values())
           and (SEALED_PREFIX + "이 셀은 무엇을 서빙했고") in st["00-hint.md"])
        ck("봉인 멱등 · brief 불변", brief(sealed_dir) == b0 and len(b0.splitlines()) == 3)
        ck("봉인 줄 뒤 빈 줄(인용문 게으른 계속 방지)", (SEALED_PREFIX + "이 셀은 무엇을 서빙했고") in st["00-hint.md"] and all(
            not ln.strip() or not nxt.strip() or not ln.startswith(SEALED_PREFIX) for ln, nxt in
            zip(st["00-hint.md"].split("\n"), st["00-hint.md"].split("\n")[1:])) and (sealed_dir / "00-hint.md").read_text(
            encoding="utf-8").count(SEALED_PREFIX) == 7)          # 00 PROMPT 7개(0.9 이름 꼬리 포함 · 0.8 선택도 질문 줄은 남는다)
        sealed_issues = lint(repo, sealed_dir, sealed=True, **kw)
        ck(f"봉인 뒤 lint 통과: {sealed_issues[:3]}", sealed_issues == [])
        ck("★봉인 뒤 PROMPT 재삽입", "HINT_PROMPT_RESIDUE" in codes(lint(repo, variant("rs", lambda p: None), sealed=True, **kw)))
        ck("봉인 = SELFCHECK 0(저작 자기점검은 배포되지 않는다) · 봉인 전에는 있다", all("SELFCHECK" not in x for x in st.values())
           and all("<!-- SELFCHECK" in (payload / n).read_text(encoding="utf-8") for n in PAYLOAD_DOCS)
           and "\n\n\n" not in st["00-hint.md"].split("## 0.1")[0])
        # 2026-09-22 S2 round 3: 수신자 zip 술어 — 2차 바이트 채점이 봉인 전 draft 를 재고 "SELFCHECK · PROMPT 가 zip 에 남았다" 고 적었다.
        #   봉인이 실제로 하는 일을 한 술어로 고정한다: 문서마다 조건이 성립하는 템플릿 PROMPT 수 = 질문 줄 수(각 1줄) · 지시 필드 줄
        #   (`읽을 것:` · `기재:` · `금지:` …)은 한 줄도 없다 · SELFCHECK · PROMPT 주석 · `<<AGENT:` 0. 음성대조 = 봉인 전 payload 에 같은 술어.
        def receiver_zip(p: Path) -> tuple[bool, str]:
            for n in TEMPLATE_FILES:
                text = (p / f"{n}.md").read_text(encoding="utf-8")
                rows = text.split("\n")
                if "<!-- PROMPT" in text or "SELFCHECK" in text or AGENT_MARK in text:
                    return False, f"{n}: PROMPT · SELFCHECK · <<AGENT 잔존"
                for tp in tpls[n].prompts:
                    if not _condition_holds(_directives(tp.fields)["condition"], facts_json)[0]:
                        continue
                    if rows.count(SEALED_PREFIX + _question(tp.fields)) != 1:
                        return False, f"{n} §{tp.chapter}: 질문 줄이 정확히 1개가 아니다"
                    leak = [ln for ln in tp.raw if (m_ := _PROMPT_FIELD.match(ln)) and m_.group(1) in PROMPT_KEYS and ln in rows]
                    if leak:
                        return False, f"{n} §{tp.chapter}: 지시 필드 줄 잔존 {leak[0][:40]!r}"
            return True, ""

        rz = receiver_zip(sealed_dir)
        ck(f"★R3 수신자 zip = 봉인: 템플릿 PROMPT 마다 질문 줄 1개 · 지시 필드 0 · SELFCHECK 0({rz[1]})", rz[0])
        ck("★R3 수신자 zip 술어의 음성대조: 봉인 전 payload 는 같은 술어가 거부한다", not receiver_zip(payload)[0])
        sc_back = root / "v" / "sc-back" / "payload"
        shutil.copytree(sealed_dir, sc_back)
        (sc_back / "03-benchmark.md").write_text((sc_back / "03-benchmark.md").read_text(encoding="utf-8")
                                                 + "\n<!-- SELFCHECK\n1. 수치의 귀속\n-->\n", encoding="utf-8")
        ck("★봉인 뒤 SELFCHECK 재삽입 = HINT_SELFCHECK_RESIDUE",
           "HINT_SELFCHECK_RESIDUE" in codes(lint(repo, sc_back, sealed=True, facts=facts_json, **kw)))
        one = root / "v" / "one-line" / "payload"
        shutil.copytree(sealed_dir, one)
        (one / "02-narrative.md").write_text((one / "02-narrative.md").read_text(encoding="utf-8")
                                             + "\n<!-- PROMPT 질문: 한 줄 -->\n", encoding="utf-8")
        ck("★봉인 뒤 한 줄짜리 PROMPT 주석(branch.PROMPT_RESIDUE_RE 와 같은 판정)",
           "HINT_PROMPT_RESIDUE" in codes(lint(repo, one, sealed=True, facts=facts_json, **kw)))
        ck("봉인 뒤에도 CLI --verify PASS", cli_verify(sealed_dir, repo / rep_p) == 0)

        # map_only · single 토폴로지(1.5 조건 불성립) · waiver 양성
        mo = json.loads(core.dumps(base_facts))
        mo.update(task_class="hint_map_only", sub_recipe=None,
                  identity={**base_facts["identity"], "topology": "single", "tp": 1},
                  publication={**base_facts["publication"], "task_class": "hint_map_only"})
        pm = scaffold("maponly", mo)
        ck("single: 1.5 해당 없음 · PROMPT 없음", "해당 없음 — 이 절의 조건 `topology=multi`" in (pm / "01-artifacts.md").read_text(encoding="utf-8")
           and not any(p["section"] == "1.5" for p in prompts(pm)))
        fill(pm, mo)
        refresh(pm)
        mkw = dict(kw, manifest={"task_class": "hint_map_only"})
        ck(f"map_only lint 통과: {lint(repo, pm, **mkw)[:3]}", lint(repo, pm, **mkw) == [])
        dst = root / "v" / "mo-strip" / "payload"
        shutil.copytree(pm, dst)
        (dst / "00-hint.md").write_text((pm / "00-hint.md").read_text(encoding="utf-8").replace(MAP_ONLY_MARKER, "OBSERVATION"), encoding="utf-8")
        ck("★OBSERVATION-ONLY 부재(map_only)", "HINT_MAP_ONLY_OBSERVATION_MARKER_MISSING" in codes(lint(repo, dst, facts=json.loads(core.dumps(mo)), **mkw)))
        t = (dst / "00-hint.md").read_text(encoding="utf-8")
        (dst / "00-hint.md").write_text(t + f"\n<!-- {MAP_ONLY_MARKER} -->\n", encoding="utf-8")
        ck("★주석 속 OBSERVATION-ONLY 는 마커가 아니다", "HINT_MAP_ONLY_OBSERVATION_MARKER_MISSING" in codes(lint(repo, dst, facts=json.loads(core.dumps(mo)), **mkw)))
        wv = json.loads(core.dumps(base_facts))
        wv["perf_waiver"] = {"warning_flag": "기대 이하 — 재측정 필요", "reason": "픽스처"}
        pw = scaffold("waiver", wv)
        fill(pw, wv)
        refresh(pw)
        wkw = dict(kw, manifest={"task_class": "full_benchmark", "benchmark": {"perf_waiver": wv["perf_waiver"]}})
        ck(f"waiver: PERF-WARNING + warning_flag 원문 실림: {lint(repo, pw, **wkw)[:3]}", lint(repo, pw, **wkw) == [])
        ck("★기존 저작물 덮지 않음", raises(lambda: render_scaffold(repo, base_facts, payload), "HINT_SCAFFOLD_EXISTS"))
        bad_t = dict(base_facts, tag="not-a-hint")
        ck("★facts.tag 모양", raises(lambda: render_scaffold(repo, bad_t, root / "bt" / "payload"), "HINT_FACTS_SHAPE"))
        no_utc = dict(base_facts, generated_utc="2026-09-21 10:21")
        ck("★주입 시각 아님", raises(lambda: render_scaffold(repo, no_utc, root / "nu" / "payload"), "HINT_TIME_NOT_INJECTED"))

        # ── v7(2026-09-29 · plan_26092908 §4.1~§4.6): 판정 표면 · 이름 꼬리 · 검증 표시 3자리 · Agent 저작 요청 · 매핑 · 결손 파생 ──
        rf = json.loads(core.dumps(base_facts))
        rf["measurement"].update(verdict=REFUTE_MARKER, verdict_reasons=["decode 20.98 < floor 25.33"],
                                 sources={"verdict": "sweep verdict.json(fixture)", "decode_tps_conc1": "sweep verdict.json(fixture)"})
        rctx = _Ctx(repo, rf, tpls)
        g = _f_grade(rctx)
        ck("판정 행: FACT:grade 에 `REFUTE` + 출처 · 00 머리 REFUTE 배너(사유 포함)", "| 성능 판정(measurement.verdict) | `REFUTE` — 출처 "
           "sweep verdict.json(fixture) |" in g and f"> **{REFUTE_MARKER}** —" in g and "floor 25.33" in g)
        ck("판정 행: 판정 원천 없으면 '미관측 — <사유>'(침묵 ✗)", "성능 판정(measurement.verdict) | 미관측 —" in _f_grade(_Ctx(repo, facts_json, tpls)))
        bmr = _f_bench_missing(rctx)
        ck("★비발행 판정(REFUTE) = 무인증서 결손 배너 ✗ · 비발행 설명 줄", "인증서 비발행 판정(결손 아님)" in bmr and "인증서가 없다" not in bmr)
        mt = _f_measurement(rctx)
        ck("측정 표: 키별 출처는 출처 열(sources.<키> 행 ✗)", "| `verdict` | REFUTE | sweep verdict.json(fixture) |" in mt
           and "`sources." not in mt)
        ck("README 판정 줄", "> **판정** `REFUTE`" in _render_readme(rctx))
        # 이름 꼬리: publish(후보) · continue 확정(토큰 · 뜻 · 근거 · timestamp)
        nf = json.loads(core.dumps(base_facts))
        nf["base_tag"] = nf["tag"]
        nf["tail_candidates"] = [{"axis": "graph", "token": "eager", "meaning": "실행 모드 eager", "evidence": None, "source": "yaml"}]
        ck("0.9 후보 표(확정 전) · 기본 이름 · 근거 미특정 표기", "**기본 이름**(꼬리 전)" in _f_name_tail(_Ctx(repo, nf, tpls))
           and "`eager`" in _f_name_tail(_Ctx(repo, nf, tpls)) and "근거 미특정" in _f_name_tail(_Ctx(repo, nf, tpls)))
        cf = json.loads(core.dumps(nf))
        cf["naming"]["tail"] = [{"token": "eager", "meaning": "eager 실행", "evidence": {"file": "c.yaml", "key": "enforce-eager",
                                                                                     "value": "true"}}]
        cf["naming"]["timestamp"] = "t2609291230"
        cf["tag"] = nf["tag"] + "-eager-t2609291230"
        nt = _f_name_tail(_Ctx(repo, cf, tpls))
        ctx_b = _f_context(_Ctx(repo, cf, tpls))
        ck("0.9 확정 꼬리(토큰 · 뜻 · 근거 · timestamp) · 0.2 명명 축에 꼬리 · timestamp 행 · ple·spec·graph 결정론 행 ✗",
           "**확정 이름**" in nt and "`enforce-eager` = `true`" in nt and "t2609291230" in nt and "꼬리 `eager`" in ctx_b
           and "| timestamp |" in ctx_b and "결정론부(vllm · model · arch · q · len · kv" in ctx_b and "전량 파생했다" not in ctx_b)
        ng = name_guide_md(cf)
        ck("README 이름 읽는 법: 세그먼트 · 꼬리 뜻 · timestamp · native · -bare · 세대 표(v7 · v6 · 옛 5·4)",
           all(x in ng for x in ("`<vllm>`", "꼬리 `eager`", "t2609291230", "`native` = **실제 하드웨어", "`-bare` = **Docker 없이**",
                                 "| `v7` |", "| `v6` |", "옛 5세그먼트", "옛 4세그먼트")))
        # 결손에서 파생하는 01 문구 · 이 셀의 사유 열
        mf = json.loads(core.dumps(base_facts))
        mf.update(applied_set=None, sub_recipe=None, missing=["HINT_MISSING_APPLIED_SET", "HINT_MISSING_SUB_RECIPE",
                                                               "HINT_MISSING_NATIVE_LAUNCH"],
                  missing_reasons={"HINT_MISSING_NATIVE_LAUNCH": "producer native_launch_error: OSError: disk full"})
        mctx = _Ctx(repo, mf, tpls)
        ck("01 결손 문구 = 결손 표에서 파생(적용 집합 · 서브 레시피 — 코드 · 뜻) · 결손 표 '이 셀의 사유' 열",
           "`HINT_MISSING_APPLIED_SET`" in _f_applied_set(mctx) and "`HINT_MISSING_SUB_RECIPE`" in _f_sub_recipe(mctx)
           and "| 코드 | 뜻 | 이 셀의 사유 |" in _f_missing(mctx) and "disk full" in _f_missing(mctx))
        ck("★결손 코드가 없으면 01 이 '결손' 이라 말하지 않는다(침묵 ≠ 없음 — 부재 사실만)",
           "결손 `" not in _f_applied_set(_Ctx(repo, dict(mf, missing=[]), tpls)))
        # 0.4 양자화 구성 · 이 모델에 필요한 패치 · 드라이버 관측/선언
        qf = json.loads(core.dumps(base_facts))
        qf["identity"]["quant_composition"] = [{"scope": "quantization_config 선언 대상", "dtype": "fp8", "source": "config.json"},
                                              {"scope": "routed experts", "dtype": "fp4", "source": "config.json expert_dtype"}]
        qf["build"]["driver_conflict"] = {"observed": "580.178.04", "declared": "580.173.02", "sources": {}}
        qf["slots"]["build_patch_pre"]["file_records"] = [
            {"path": "artifacts/build_patch_pre/60-fixture-patch.sh", "sha256": "0" * 64, "verification": "verified",
             "generated_by": "cell-run", "verification_basis": "빌드 원장", "relevance": "required", "relevance_basis": "① ∋ ② 발화"}]
        rs_ = _f_resolved(_Ctx(repo, qf, tpls))
        ck("0.4 양자화 구성 표(선언 fp8 + routed expert fp4) · 필요한 패치 요약(required 목록) · 드라이버 관측 ≠ 선언 행",
           "| routed experts | `fp4` |" in rs_ and "required `60-fixture-patch.sh`" in rs_ and "⚠ 드라이버 관측 ≠ 선언" in rs_)
        # 재현: 슬롯 → 빌드 컨텍스트 매핑 · env 형상 파생 키 · README 요약
        xf = json.loads(core.dumps(base_facts))
        xf["build_context_map"] = [{"slot_path": "artifacts/build_patch_pre/60-fixture-patch.sh",
                                    "context_path": "build_patches_src/60-fixture-patch.sh", "basis": "Dockerfile COPY build_patches_src/"},
                                   {"slot_path": "artifacts/build_recipe/pip-freeze-main.txt", "context_path": None, "basis": "입력 아님"}]
        xf["env_shapes"] = [{"file": "envs/.env.interconnect", "template": "artifacts/compose/.env.interconnect.template", "derived": [
            {"field": "interconnect.nccl_transport", "keys": ["NCCL_IB_DISABLE", "NCCL_NET"],
             "placeholders": {"NCCL_IB_DISABLE": "<derived:interconnect.nccl_transport→NCCL_IB_DISABLE>",
                              "NCCL_NET": "<derived:interconnect.nccl_transport→NCCL_NET>"},
             "rule_text": "rdma → NCCL_IB_DISABLE=0 · NCCL_NET=IB", "source": "render_dockerfile.NCCL_TRANSPORTS"}]}]
        rp_ = _f_reproduce(_Ctx(repo, xf, tpls))
        ck("01 §1.4: 슬롯 → 빌드 컨텍스트 매핑 표(입력 아님 = —) · env 파생 키 자리표시 · 실효값 규칙",
           "| `artifacts/build_patch_pre/60-fixture-patch.sh` | `build_patches_src/60-fixture-patch.sh` |" in rp_
           and "| `artifacts/build_recipe/pip-freeze-main.txt` | — |" in rp_ and "<derived:interconnect.nccl_transport→NCCL_NET>" in rp_
           and "rdma → NCCL_IB_DISABLE=0" in rp_)
        # ── F3 · F6 · F5 · F2 · F1(2026-09-29 · plan_26092908 §4.5 FACT 생산자 교정) ──
        xf6 = json.loads(core.dumps(xf))
        d6 = xf6["env_shapes"][0]["derived"][0]
        d6.pop("rule_text")
        d6["effective_values"] = {"rdma": {"NCCL_IB_DISABLE": "0", "NCCL_NET": "IB"}, "socket": {"NCCL_IB_DISABLE": "1", "NCCL_NET": "Socket"}}
        xf6["env_shapes"][0]["derived"].append({"field": "interconnect.socket_iface", "keys": ["NCCL_SOCKET_IFNAME"],
                                                "placeholders": {}, "value_rule": {"NCCL_SOCKET_IFNAME": "{interconnect.socket_iface}"},
                                                "source": "탐침"})
        rp6 = _f_reproduce(_Ctx(repo, xf6, tpls))
        ck("F6 rule_text 없는 파생 키 = 렌더러 표(effective_values · value_rule)에서 규칙 · '규칙 미관측' ✗ · 원시 JSON ✗",
           "rdma → NCCL_IB_DISABLE=0 · NCCL_NET=IB \\| socket → NCCL_IB_DISABLE=1 · NCCL_NET=Socket" in rp6
           and "NCCL_SOCKET_IFNAME={interconnect.socket_iface}" in rp6 and "규칙 미관측" not in rp6 and '{"NCCL_SOCKET' not in rp6)
        d6.pop("effective_values")
        ck("★F6 음성대조: 렌더러 표도 없으면 '규칙 미관측'(지어내지 않는다)", "규칙 미관측" in _f_reproduce(_Ctx(repo, xf6, tpls)))
        qf3 = json.loads(core.dumps(qf))
        qf3["build"]["driver_by_node"] = {"main": "580.178.04", "sub": "580.178.04"}
        qf3["build"]["driver_conflict"] = {"observed": "580.178.04", "declared": "580.173.02",
                                           "sources": {"observed": "attestation parity.driver(fx)", "declared": "manifest(fx)"}}
        qf3["build"].setdefault("source", {})["driver_by_node"] = "attestation parity.driver(fx)"
        rs3 = _f_resolved(_Ctx(repo, qf3, tpls))
        ck("F3 드라이버 두 행: 값 = 읽는 모양 · 출처 = 생산자 출처(원시 JSON ✗ · 출처 미기재 ✗)",
           "| 드라이버(노드별 관측) | main=580.178.04 · sub=580.178.04 | attestation parity.driver(fx) |" in rs3
           and "| ⚠ 드라이버 관측 ≠ 선언 | 관측 580.178.04 ≠ 선언 580.173.02 | 관측: attestation parity.driver(fx) · 선언: manifest(fx) |"
           in rs3 and '{"' not in rs3.split("⚠ 드라이버")[1].split("\n")[0])
        qf2 = json.loads(core.dumps(qf))
        qf2["build"]["image_digest"] = None
        qf2["build"].setdefault("source", {})["image_digest_local_note"] = "docker image inspect fx .Id · 측정 뒤 빌드 — 측정 이미지 아님"
        ck("F2 측정 digest 없음 + 로컬 이미지 측정 뒤 빌드 → 값 미관측 · 출처 칸이 관측을 말한다",
           "| 이미지 digest | 미관측 | docker image inspect fx .Id · 측정 뒤 빌드 — 측정 이미지 아님 |" in _f_resolved(_Ctx(repo, qf2, tpls)))
        sf5 = json.loads(core.dumps(base_facts))
        sl5 = next(iter(sf5["slots"]))
        sf5["slots"][sl5].pop("rationale", None)
        sf5["slots"][sl5]["machine_reason"] = "셀 env 에 VARIANT 줄 없음 = stock"
        ck("F5 slots 사유 칸: rationale 없으면 기계 파생 사유 + 표지", "셀 env 에 VARIANT 줄 없음 = stock(기계 파생)"
           in _f_slots(_Ctx(repo, sf5, tpls)))
        sf5["slots"][sl5]["rationale"] = "저작자 선언"
        ck("★F5 음성대조: 저작자 rationale 이 있으면 그것이 먼저(기계 사유가 덮지 않는다)",
           "저작자 선언" in _f_slots(_Ctx(repo, sf5, tpls)) and "(기계 파생)" not in _f_slots(_Ctx(repo, sf5, tpls)).split(sl5)[1]
           .split("\n")[0])
        mf4 = json.loads(core.dumps(base_facts))
        mf4["campaign"] = {"id": "camp-fx", "cell": "c", "node": "cluster", "mode": "publication-replay",
                           "id_source": "봉인 페이로드 abcdef012345:PAYLOAD.json campaign.id(재생 전용 대체)"}
        mf4["approval"] = None
        mf4["prior_approval"] = {"approved_by": "사용자(fx)", "approved_utc": "2026-01-01T00:00:00Z", "source": "campaign:hint_targets",
                                 "replay_source": "봉인된 발행 페이로드 abcdef012345"}
        m4 = _f_meta(_Ctx(repo, mf4, tpls))
        ck("F4 재생: 캠페인 id = 봉인 기록(출처 괄호) · 승인 = 원 발행 승인(재생 전용 대체 표지)",
           "camp-fx(봉인 페이로드 abcdef012345:PAYLOAD.json campaign.id(재생 전용 대체))" in _f_header(_Ctx(repo, mf4, tpls))
           and "| 승인 출처 | campaign:hint_targets(재생 전용 대체 — 원 발행의 승인" in m4 and "2026-01-01T00:00:00Z(원 발행)" in m4)
        mf4["approval"] = {"approved_by": "사람(이 판)", "approved_utc": "2026-02-02T00:00:00Z", "source": "cli:--approved-by"}
        ck("★F4 음성대조: 이 판의 승인이 있으면 그것만(원 발행 승인과 섞지 않는다)",
           "| 승인 출처 | cli:--approved-by |" in _f_meta(_Ctx(repo, mf4, tpls)) and "원 발행" not in _f_meta(_Ctx(repo, mf4, tpls)))
        sr12 = json.loads(core.dumps(base_facts))
        if isinstance(sr12.get("sub_recipe"), dict) and sr12["sub_recipe"]:
            sr12["plane"] = "native"
            sr12["native_source"] = {"cell": "src-cell"}
            ck("F12 native sub_recipe = 'Docker 형 참고(원천 셀 · 이 셀 run 아님 · generated-unverified)' 머리",
               "**Docker 형 참고**(원천 셀 `src-cell` · **이 셀 run 아님** · `generated-unverified`)" in _f_sub_recipe(_Ctx(repo, sr12, tpls)))
            sr12["plane"] = "docker"
            ck("★F12 음성대조: Docker 셀 sub_recipe 에는 참고 표지 없음", "Docker 형 참고" not in _f_sub_recipe(_Ctx(repo, sr12, tpls)))
        else:
            ck("F12 픽스처: base_facts 에 sub_recipe 가 있어야 한다(검사가 조용히 빠지지 않게)", False)
        pe1 = _patch_evidence_table([{"file": "60-a.sh", "script_identity": "build-ledger(원장 script_sha256 = 작업트리 바이트)",
                                      "ledger_script_sha256": "a" * 64}])
        ck("F1 판정 근거 표: 원장 sha 를 출처 표지와 함께 옮긴다(이미지 사본과 구분)",
           f"`{'a' * 64}`(빌드 원장 script_sha256 — 이미지 사본 아님)" in "\n".join(pe1) and "build-ledger(원장" in "\n".join(pe1))
        cm = _context_map_summary(xf)
        ck("README 매핑 요약: zip 폴더 → 컨텍스트 폴더 · 입력 아닌 파일 수", "| `artifacts/build_patch_pre/` | `build_patches_src/` | 1 |" in cm
           and "입력이 아닌 파일 1개" in cm)
        # 검증 표시 3자리(PAYLOAD 파일 기록 · 파일 첫 줄 · 01 표) · Agent 저작 요청 · 태그 이름 리터럴
        vf = json.loads(core.dumps(base_facts))
        vf["base_tag"] = vf["tag"]
        hdr = "# " + UNVERIFIED_MARK + " — native 셀(Docker 미사용)이라 이 파일은 실행 검증되지 않았다 · 생성: renderer"
        vf["slots"]["build_patch_pre"]["file_records"] = qf["slots"]["build_patch_pre"]["file_records"]
        vf["slots"]["compose"] = {"files": ["artifacts/compose/serve_runner.sh"], "applicable": True, "confidence": "1-signal",
                                  "evidence": {"kind": "file", "ref": "renderer"},
                                  "file_records": [{"path": "artifacts/compose/serve_runner.sh", "sha256": "1" * 64,
                                                    "verification": "generated-unverified", "generated_by": "renderer",
                                                    "verification_basis": "렌더러 산출물", "relevance": "required",
                                                    "relevance_basis": "재현 경로 입력",
                                                    "header": {"style": "comment", "line": 2, "text": hdr}}]}
        areq = {"slot": "triplet", "name": "native-launch.md", "path": "artifacts/triplet/native-launch.md",
                "why": "native 기동 기록 부재", "inputs": ["artifacts/triplet/"], "verification": "generated-unverified",
                "generated_by": "agent", "header_required": True,
                "header": UNVERIFIED_MARK + " — Agent 가 증거에서 저작했다(실행 검증 ✗) · 생성: agent"}
        vf["agent_requests"] = [areq]
        pv = root / "v7marks" / "payload"
        (pv / "artifacts/build_patch_pre").mkdir(parents=True)
        (pv / "artifacts/build_patch_pre/60-fixture-patch.sh").write_text("#!/bin/sh\n# fixture patch: PLE 게이트 완화\nexit 0\n",
                                                                          encoding="utf-8")
        (pv / "artifacts/compose").mkdir(parents=True)
        (pv / "artifacts/compose/serve_runner.sh").write_text(f"#!/bin/bash\n{hdr}\necho serve\n", encoding="utf-8")
        render_scaffold(repo, vf, pv)
        ag = pv / areq["path"]
        ck("Agent 저작 요청: 스캐폴드 = 첫 줄 경고(md = HTML 주석) + 자리표시 · 01 요청 표 · 파일별 검증 · 관련성 표",
           ag.read_text(encoding="utf-8").split("\n")[0] == "<!-- " + areq["header"] + " -->" and AGENT_MARK in ag.read_text(encoding="utf-8")
           and "**Agent 저작 요청 1건**" in (pv / "01-artifacts.md").read_text(encoding="utf-8")
           and "`generated-unverified` · renderer" in (pv / "01-artifacts.md").read_text(encoding="utf-8"))
        fill(pv, vf)
        refresh(pv)
        vkw = dict(kw, facts=json.loads(core.dumps(vf)))
        ck("★Agent 요청 파일 미저작 = HINT_AGENT_PLACEHOLDER_RESIDUE · HINT_AGENT_ARTIFACT_UNAUTHORED",
           {"HINT_AGENT_PLACEHOLDER_RESIDUE", "HINT_AGENT_ARTIFACT_UNAUTHORED"} <= codes(lint(repo, pv, **vkw)))
        ag.write_text("<!-- " + areq["header"] + " -->\n# native 기동 절차\n1. ray head 기동\n", encoding="utf-8")

        def vvar(tag: str, mutate) -> Path:
            dst = root / "v7v" / tag / "payload"
            shutil.copytree(pv, dst)
            mutate(dst)
            return dst

        ck(f"검증 표시 3자리 일치 · Agent 저작 완료 = lint 0: {lint(repo, pv, **vkw)[:3]}", lint(repo, pv, **vkw) == [])
        ck("★Agent 저작 파일 첫 줄 경고를 지웠다 = HINT_VERIFICATION_MARK_MISMATCH", "HINT_VERIFICATION_MARK_MISMATCH" in codes(lint(repo, vvar(
            "agh", lambda q: (q / areq["path"]).write_text("# native 기동 절차\n1. ray head 기동\n", encoding="utf-8")), **vkw)))
        ck("★generated-unverified 파일의 헤더를 지웠다 = HINT_VERIFICATION_MARK_MISMATCH", "HINT_VERIFICATION_MARK_MISMATCH" in codes(lint(
            repo, vvar("vmh", lambda q: (q / "artifacts/compose/serve_runner.sh").write_text("#!/bin/bash\necho serve\n",
                                                                                                  encoding="utf-8")), **vkw)))
        ck("★verified 파일에 미검증 경고가 붙었다 = HINT_VERIFICATION_MARK_MISMATCH", "HINT_VERIFICATION_MARK_MISMATCH" in codes(lint(
            repo, vvar("vmv", lambda q: (q / "artifacts/build_patch_pre/60-fixture-patch.sh").write_text(
                f"#!/bin/sh\n{hdr}\nexit 0\n", encoding="utf-8")), **vkw)))
        ck("★01 `검증` 열을 손으로 verified 로 바꿨다 = HINT_FACT_DRIFT(셋째 자리)", "HINT_FACT_DRIFT" in codes(lint(repo, vvar(
            "vmt", edit("01-artifacts.md", "`generated-unverified` · renderer", "`verified` · renderer")), **vkw)))
        ck("★저작 산문에 태그 이름 리터럴 = HINT_TAG_LITERAL_IN_PROSE(이름은 사실 블록 소유)", "HINT_TAG_LITERAL_IN_PROSE" in codes(lint(
            repo, vvar("tl", edit("02-narrative.md", "- moe-backend 를 명시하지 마라", f"- {vf['tag']} 의 moe-backend 를 명시하지 마라")),
            **vkw)))
        cs = cited_sources(repo, payload, lineage=lineage)
        ck("cited_sources: 발췌 수(계보 문서) · 출처 경로(서명 · hint-event) — 봉인 스냅샷 · LINEAGE 요약 입력",
           cs["excerpt_counts"].get(plan_p, 0) >= 1 and tl_p in cs["paths"] and log_p in cs["paths"]
           and all(not x.startswith("artifacts/") for x in cs["paths"]))
    # 2026-09-22 S2 round 2: 하한을 새 음성대조까지 올린다(158 실측) — 새 검사 묶음이 조용히 빠지면 붉어진다.
    # 2026-09-22 S2 round 3: 공유 사실 계약 렌더 · 스냅샷 해소 · 수신자 zip 술어 묶음을 더해 197 실측 → 하한 190.
    # 2026-09-22 S2 round 3 적대 리뷰: 스냅샷 origin 대조 · Ray 접힘 · 값 집합 · 0.2 생산자 · 다음 변경 관측 없음 묶음(213 실측) → 하한 206.
    # 2026-09-29 plan_26092908(v7): 판정 표면 · 이름 꼬리 · 검증 표시 · Agent 요청 · 매핑 · 결손 파생 묶음 → 하한 225.
    ck(f"검사 전수 실행({ran[0]}) — 중도 반환으로 시험이 조용히 줄지 않게", ran[0] >= 225)
    return bad
