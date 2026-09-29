"""hintlib.evidence — 셀 증거의 단일 읽기 면 · 발행 자격(관측) · 승격 게이트 클라이언트 · 발행기 구동.

한 셀(= 버전×모델 1조합 · 노드 형상 하나)의 증거를 `CellEvidence` 하나로 모은다. 이름 파생(naming)·계보(lineage)·
산출물(artifacts)·템플릿(template)이 **같은 객체**를 읽는다 — 네 모듈이 각자 캠페인·스윕·인증서를 다시 읽으면
같은 사실이 네 벌로 갈라진다(옛 hint_tag·hint_collect 가 인증서 파서를 세 벌 가졌던 결함 · 코드맵 F14).

불변식(날짜 박힌 "왜" — 코드맵 campaign_plane §1 · hint_tag_a §2 · hint_tag_b §1 에서 옮김)
    - **ACTIVE 를 읽지 않는다**(2026-09-07 · plan_26090715 Q6 · 옛 hint_tag.py:1650). 입력은 명시한 캠페인 id 뿐이다.
      라이브 인스턴스를 포인터로 따라가면 태그는 불변인데 입력이 흐르고, 캠페인 밖 발행이 활성 캠페인을 오독한다.
      `_bootstrap` 은 루트 폴백이 아니다(2026-09-06 — 루트 누출 21건). 발행 대상 id 로 받지 않는다.
    - **캠페인 상태를 바꾸지 않는다**. 옛 `hint_collect._backfill_campaign_cells` 는 ACTIVE 를 경유해 인스턴스 전체를
      발행 직전에 고쳤다(H-7 · I1 위반). 이 모듈의 캠페인 쓰기는 `phase_set_publish` 하나이고, 그것도 소유자
      `campaign_init.py` 의 CLI 를 부를 뿐이다(포맷 소유 1 · 호출부 N — 2026-09-07 plan_26090715 §4.1).
    - **발행 자격은 관측이다**(2026-09-21 · plan_26092119 X8 · F11). health 200 + 추론 1회는 serve_proof · post_health ·
      벤치 완료 파일로 읽는다. `manifest.runtime.health_ok` 같은 선언 복사는 증거가 아니다 — 계약 §2 의 유일한 위협이
      "서빙 실패를 성공으로 위장한 배포" 다.
    - **스윕 디렉터리는 `sweep_index.generated_utc == 인증서 measured_utc` 일 때만 묶는다**(2026-09-12 tag2 실측 ·
      코드맵 G6). 같은 셀 id 가 캠페인마다 재사용되고 스윕 디렉터리는 캠페인 스코프가 아니라서, 이름만으로 묶으면
      다른 측정의 원시를 이 태그의 증거로 싣는다(09-10 35.39 t/s 런이 09-12 38.81 t/s 런에 덮였다).
    - **인증서 파서는 completion_gate 한 벌**(`parse_flat_certificate` · 공개 별칭 `lexical_components`·
      `manifest_rubric_contract`). 사설 밑줄 심볼을 부르지 않는다(코드맵 policy_callers §2.7).
    - **이미지 digest 가 비교 권위다 — 태그는 이름이다**(2026-09-11 R5: wheel 태그를 단 0.27.1 소스빌드). 로컬 태그가
      측정 digest 와 다른 이미지를 가리키면 그 이미지의 history 를 빌드 정체로 쓰지 않는다.
    - **발행기 입력 JSON 은 도구가 쓴다**(AC5 · 손 JSON 0). identity/runtime/pii 는 `<draft>/inputs/` 에만 쓰고,
      work-manifest·발행 기록은 evidence_publisher 가 유일한 writer 다(X6 — hint 는 manifest 를 직접 고치지 않는다).
    - 측정 도구 스냅숏(2026-09-22 · S2 round 3)도 `<draft>/inputs/sources/` 에만 쓴다(`git show <측정 시점 rev>:<path>` 바이트 그대로 ·
      tool_snapshots). 저장소·git 상태는 읽기만 한다(reflog·log·show·ls-tree·cat-file).
    - runtime.identity 는 identity 의 **복사가 아니라 관측**이다(스윕 `sweep_index.meta` 또는 lite 리포트 측정 환경 표).
      종전 writer 는 전부 identity 를 복사해 `RUNTIME_IDENTITY_MISMATCH` 가 공허하게 참이었다(코드맵 §2.7).
    - PII: 비배포 증거는 nondeploy_prose 강도로 **실제 스캔**하고, simlog(기계생성 원시 평면)은 스캔한 척하지 않고
      `exempt_paths` 에 사유와 함께 적는다(X5 · docs.md §PII 스캔 적용 범위 · 2026-07-31 "좁은 패턴으로 스캔하고
      통과를 선언하면 그 선언 자체가 거짓").
    - 시각은 주입만 받는다. 벽시계를 읽지 않는다. docker 는 읽기(`image inspect`·`history`)만 부른다.
    - YAML(manifest·셀 config·서빙 yaml)은 이 모듈이 파싱하지 않는다 — 소유 로더(terraforming `manifest_contract.
      _load_manifest`)를 경로로 적재해 부른다. hintlib 은 stdlib 전용이고, PyYAML 의존은 그 소유자에게 남는다.
"""
from __future__ import annotations

import dataclasses
import datetime as _dt
import json
import os
import re
import shlex
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path, PurePath
from types import SimpleNamespace
from typing import Callable

from . import core

# ── 교차 스킬 소유 모듈(경로 · 부재 = 죽는다) ───────────────────────────────────────────────────────────────
REL_MANIFEST_CONTRACT = ".claude/skills/terraforming_node/scripts/manifest_contract.py"
REL_CHECK_SMOKE_MODEL = ".claude/skills/upstream-version-watch/scripts/check_smoke_model.py"
# 두-레이아웃(2026-07-31 · 옛 hint_tag._cgate): ① 정본 `.claude/policies/runtime/completion_gate.py`
#   ② hintlib 옆 동거 사본(`scripts/completion_gate.py`). 옛 로더는 ②를 `import completion_gate` 로 찾아서 sys.path 에
#   먼저 걸린 **아무 사본**이나 이겼다(코드맵 hint_tag_a §2.G 숨은 결합). 이제 둘 다 명시 경로다.
_COLOCATED_GATE = core.SCRIPTS_DIR / "completion_gate.py"
_GATE_REQUIRED_API = ("parse_flat_certificate", "certificate_run_key", "CERTIFICATE_FIELD_MAP", "STRONG_IDENTITY_FIELDS",
                      "lexical_components", "manifest_rubric_contract", "PII_MACHINE_RAW_EXEMPTION_REASON",
                      # 2026-09-29(plan_26092908 §4.8): 인증서 요구 = 발행 조건과 같은 술어(게이트 한 벌 · 비발행 권한 판정)
                      "certificate_requirement_authority", "certificate_waived", "CERTIFICATE_NON_ISSUING_AUTHORITIES")

NODE_AXIS = ("main", "sub", "cluster")
TOPOLOGIES = ("single", "multi")
RESERVED_CAMPAIGN_IDS = ("_template", "_bootstrap")
MODES = ("campaign", "publication-replay")
_CELL_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
_SHA40 = re.compile(r"^[0-9a-f]{40}$")
_UTC = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z")
_REPORT_BORN = re.compile(r"생성일\s+(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)")
_HISTORY_ARGS = re.compile(r"^(?:RUN\s+)?\|(\d+)\s+(.*)$")
_HF_URL = re.compile(r"^(?:https?://)?(?:[^/@\s]+@)?(?:huggingface\.co|hf\.co)/([A-Za-z0-9][A-Za-z0-9._-]*/[A-Za-z0-9][A-Za-z0-9._-]*?)(?:\.git)?/?$")
# 인증서·스윕 meta 가 "읽지 못함" 을 적는 표기. 값이 아니다(읽지 못함 ≠ 없음 · naming `_UNOBSERVED` 와 같은 결).
_UNOBSERVED = frozenset({"", "n/a", "na", "null", "none", "unknown", "?"})
_TRUE = frozenset({"1", "true", "yes", "on"})
_FALSE = frozenset({"0", "false", "no", "off"})
_PLE_KEYS = ("ple_layer_ids", "ple_embed_dim")          # 모델 config 의 PLE 실재 판정 키(태그2 체크포인트 실측)
_WHEEL_URL_KEYS = ("VLLM_WHEEL_URL", "WHEEL_URL")

# ── 결손 사유코드(PAYLOAD.missing[] 의 유일한 어휘 · SPEC §2.1) ───────────────────────────────────────────────
# 옛 hint_collect.MISSING_CODES 를 그대로 옮기고(문구 보존) 신설분을 더한다. 발행 계약 v4 절단선(2026-09-06 ·
# plan_26090616 Q7/Q8): 차단은 양성 검출이고 **부재는 기재**다 — 적힌 부재는 "모른 채 소비한다" 는 표지다.
MISSING_CODES: dict[str, str] = {
    # 2026-09-29(plan_26092908 §4.4 · V2): 옛 문구("full PASS 가 아니었거나 …")는 REFUTE 와 "인증서 없는 PASS" 를 같은 코드로 적었다.
    #   이제 **인증서가 발행되는 판정**(explicit ∧ PASS · 또는 판정·권한 미관측)에서만 싣는다 — 비발행 판정(REFUTE 전부 · weak/explore
    #   PASS · lite-only)은 결손이 아니라 정상이고 그 사실은 PAYLOAD.measurement.verdict(+ sources)가 말한다(_certificate_not_due).
    "HINT_MISSING_CERTIFICATE": ("인증서 부재 — 인증서가 발행되는 판정(루브릭 권한 explicit ∧ PASS, 또는 판정·권한을 관측하지 못한 측정)인데 "
                                 "인증서가 없다. 비발행 판정(REFUTE · weak/explore PASS · lite-only)은 이 결손이 아니다 — 그때 판정 근거는 "
                                 "bench_report 이고 판정은 measurement.verdict 가 말한다. 인증서 발행은 adversarial-benchmark 의 책임이다."),
    "HINT_MISSING_BENCH_REPORT": "벤치 리포트 부재 — 동시성별 곡선을 실을 수 없다.",
    "HINT_MISSING_SWEEP_LEVELS": ("부하 레벨이 1개 이하 — 부하 거동을 알 수 없다(경량 리포트는 동시성 곡선을 "
                                  "재지 않는다)."),
    "HINT_MISSING_LITE": "lite 관측 부재.",
    "HINT_MISSING_SLAVE_ATTESTATION": "슬레이브 ABI attestation 부재 — 멀티에서 두 노드가 같은 것을 "
                                      "돌렸다는 증거가 성공 경로에 보존되지 않았다.",
    # 옛 hint_collect 에서는 등재만 된 죽은 어휘였다(코드맵 hint_collect §11). 2026-09-21 부터 artifacts.collect 가 **발행**한다 —
    #   compose 가 `env_file:` 로 가리키는 층별 env(`.env.cluster`·`.env.interconnect` 등)가 출력 평면에 없을 때.
    "HINT_MISSING_ENV_SHAPE_RERENDER": ("측정 뒤 재생성된 토폴로지 env(`.env.interconnect`·`.env.cluster`)를 측정 당시 판본 렌더러로 다시 "
                                        "렌더하지 못했다(측정 전 렌더러 커밋 · manifest · 렌더러 거부 중 하나 — 사유는 compose 제외 행의 "
                                        "rerender_error). 형상 템플릿이 빠졌다 — 측정 당시 env 는 엔진 로그 관측 표가 대신한다."),
    "HINT_MISSING_ENV_SHAPE": ("토폴로지 env 형상 부재 — compose 가 참조하는 env 파일(`.env.cluster`·`.env.interconnect` 등)이 "
                               "출력 평면에 없어 형상 템플릿을 싣지 못했다. 수신자가 어떤 변수가 필요한지 모른다(artifacts 가 기재)."),
    # ⚠ 예약 어휘(2026-09-22 현재 발행자 0) — 서브에서 잰 셀은 메인이 그 원시를 볼 수 없어 발행 자격에서 먼저 막힌다
    #   (qualification). 서브 셀 발행을 문서기반 회수로 여는 설계가 서면 그 경로가 이 코드를 발행한다.
    "HINT_MISSING_SUB_TRIPLET": ("서브 트리플렛 부재 — 서브는 자기 것을 자율 저작하며 상향 회수는 "
                                "**문서기반 only** 다(코드·설정 직접 회수 금지). 해소는 서브가 자기 "
                                "트리플렛을 문서로 발행 → 메인이 재저작 → 그 문서를 발행 증거로 삼는 것이다."),
    "HINT_MISSING_TRIPLET": "트리플렛 부재(메인 워킹트리에 config/runner/env 3종이 없다).",
    "HINT_MISSING_PII_TERMS": "pii_terms.txt 부재 — 리터럴 스캔이 축소된 상태로 돌았다.",
    "HINT_MISSING_MEASURED_NODE": "인증서에 측정 노드 출처가 없다 — 어느 노드가 쟀는지 단정할 수 없다.",
    # 2026-09-14(plan_26091407 §4.5 · 사용자 결정 Q10): 등급 표지는 태그 이름에 새기지 않는다.
    "BENCH_MODE_LITE": ("full bench 정의(lite ∪ GuideLLM × 반복 ≥3)를 충족하지 않았다고 **기재된** 측정이다 — 선언된 "
                        "lite-only 셀이거나 반복 불성립(기계 이벤트)으로 강등된 셀이다. 수치는 lite 스냅샷(또는 강등 셀의 "
                        "대표 run 1회)이라 산포 추정치·인증서가 없다. 어느 쪽인지·강등 사유는 측정 구성 표"
                        "(bench_mode_kind · downgrade_reason)가 말한다. bench_mode 를 읽지 못한 측정(미확정·미기재)에는 "
                        "붙이지 않는다 — 모름을 lite 로 접으면 합성이다(그 사실은 카탈로그 bench_mode 칸이 말한다)."),
    # ── 2026-09-21 신설(plan_26092119) ──────────────────────────────────────────────────────────────────────
    # ★ 이 표는 **발행되는 코드의 사전**이다 — 아무도 쓰지 않는 코드를 적어 두면 코드표가 거짓 어휘를 말하고, 쓰는 코드가
    #   빠지면 template 린트가 HINT_MISSING_CODE_UNREGISTERED 로 막는다. 두 방향 모두 자체검사가 hintlib 소스의
    #   `missing.append("…")` 를 걷어 대조한다(tripwire · 2026-09-22 적대 검토: CLUSTER/INTERCONNECT_ENV_SHAPE 를 등재하고
    #   실제로 발행되는 APPLIED_PATCH_FILE·NATIVE_RECIPE 는 빠져 있었다).
    "HINT_MISSING_APPLIED_PATCH_FILE": ("원장(또는 재구성)이 **적용됐다**고 말한 빌드 패치 스크립트가 출력 평면에 없다 — 적용 사실은 "
                                        "있으나 그 바이트를 실을 수 없다(01 적용 판정표에 `file_absent` 로 기재 · artifacts 가 기재)."),
    "HINT_MISSING_BUILD_LEDGER": ("빌드 원장(`/opt/easy-vllm/build_ledger.json`) 부재 — 원장 도입 전 이미지다. 패치 적용 결과는 "
                                  "`applied_set.status: unobservable` 과 라벨된 재구성으로만 말한다(X9)."),
    # 2026-09-22 S2 round 3 통합: 이 뜻 문구는 페이로드(00 · PAYLOAD.missing)로 수신자에게 간다 — 수신자가 풀 수 없는 내부 문서
    #   좌표(옛 '코드맵 campaign_plane TL;DR 6')를 걷고 그 내용(옛 스모크 판본이 대조 행을 쓰지 않았다)만 남긴다(2차 저작자 지적).
    "HINT_MISSING_ATTESTATION_PARITY_ROWS": ("노드 정합 attestation 파일은 있으나 대조 행이 없다 — v1 은 `checks` 가 비었고(소스빌드 "
                                             "트랙의 옛 스모크 판본은 대조 행을 쓰지 않았다), v2 는 `parity` 의 "
                                             "모든 축이 `unobservable` 이다. 두 노드의 vLLM SHA·torch·driver 일치는 관측되지 않았다."),
    "HINT_MISSING_CAMPAIGN_INSTANCE": ("캠페인 인스턴스가 purge 됐다(발행 기록 재생) — 셀 lockset·config·cell.status·journey·"
                                       "grounding 이 없다. 값의 지위와 여정 한 줄은 문서(testlog·devlog) 서사에서만 복원된다."),
    "HINT_MISSING_CELL_STATUS": "캠페인 셀 상태(`cell.status.json`) 부재 — 선언↔서빙 불일치 기록을 읽지 못한다.",
    "HINT_MISSING_LOCKSET": "셀 lockset 부재 — 서빙 노브의 출처(explorer 파생 vs 손작성)를 읽지 못한다.",
    "HINT_MISSING_CELL_CONFIG": "셀 config 부재 — 선언 축(declared_axes·target_gpu)을 읽지 못한다.",
    "HINT_MISSING_SWEEP": ("이 측정의 스윕 디렉터리를 묶지 못했다 — `sweep_index.generated_utc` 가 인증서 `measured_utc` 와 같은 "
                           "사본이 없다(셀 id 재사용으로 덮였을 수 있다 · 이름만으로 묶지 않는다)."),
    "HINT_MISSING_SWEEP_RAW": ("출력 평면의 스윕 디렉터리는 다른 측정으로 덮였고, 발행 기록의 simlog 사본(sweep_index·판정점 measured)"
                               "만 묶었다 — 레벨별 원시·post_health·엔진 로그는 이 측정의 것이 아니다."),
    "HINT_MISSING_BUILD_IDENTITY": ("측정 이미지(digest)를 로컬에서 관측하지 못했다 — 태그가 다른 이미지를 가리키거나 이미지가 지워졌다. "
                                    "빌드 입력은 셀 env 선언과 인증서 지문으로만 말한다."),
    "HINT_MISSING_RESOLVE": ("`output/<t>/resolved.json` 을 이 빌드에 묶지 못했다(부재 또는 다른 vLLM 버전의 해소값) — "
                             "torch·NGC·CUDA 는 이미지 관측값만 싣는다."),
    # ── 2026-09-29 신설(plan_26092908 §4.2 · V7 "산문은 결손이라 적는데 결손 표에 없다") — 발행자 = artifacts.collect ──
    "HINT_MISSING_SUB_RECIPE": ("서브 레시피 미관측 — 서브 노드가 무엇으로 기동됐는지(`sub_recipe.json` · 서브 env 전달 파생)를 이 셀의 "
                                "증거에서 읽지 못했다. 서브 기동 절차는 메인 쪽 기록만으로 말한다(artifacts 가 기재)."),
    "HINT_MISSING_APPLIED_SET": ("빌드 패치 적용 집합 미관측 — 어느 빌드 패치가 이 이미지에 실제로 적용됐는지(빌드 원장 · 재구성) 읽지 "
                                 "못했다. 실린 패치 파일은 '적용됨' 이 아니라 '후보' 다(artifacts 가 기재)."),
    "HINT_MISSING_NATIVE_LAUNCH": ("native 기동 기록 부재 — native 셀의 기동 절차(Ray head/worker · vllm serve 인자)를 기록한 실물이 증거에 "
                                   "없다. 기동 명령은 재구성이지 관측이 아니다(artifacts 가 기재)."),
}

# 측정 구성 중 **배포 평면에 싣는 키**(옛 hint_collect.DISTRIBUTED_MEASUREMENT_KEYS 그대로).
# 리포트 표의 `*_source`(bench_mode_source · downgrade_reason_source · repeats_source · bench_tool_version_source)는
# **자유 서술 출처**다 — sweep_bench 가 `classify_cell --events-from-repo "$REPO"`(절대경로)로 부르면 강등 사유 출처에
# 블랙박스 events 파일의 **운영자 절대경로**가 그대로 박힌다. 그 문자열을 PAYLOAD 로 옮기면 배포면 4종 PII 스캔이
# fail-closed 로 죽는다. 출처 서술의 정본은 바인딩된 리포트(docs 평면 · 비배포)에 남고, 배포 평면은 그 문서를 가리키는
# `source`(`bench_report(<파일명>)`)로 출처를 표시한다(출처 표시 ≠ 서술 복제).
DISTRIBUTED_MEASUREMENT_KEYS = ("bench_mode", "bench_mode_kind", "downgrade_reason", "bench_tool", "bench_tool_version",
                                "repeats", "repeats_completed")

# 인증서에서 PAYLOAD.measurement 로 옮기는 측정 수치(옛 hint_collect.CERT_PERF — 선택이지 값의 복제가 아니다).
_CERT_MEASUREMENT_KEYS = ("benchmark_mode", "verdict", "decode_tps_conc1", "rubric_authority", "primary_source",
                          "primary_tps", "floor_tps", "tolerance", "ratio_M_over_primary", "spec_on", "accept_len",
                          "sweep_levels", "sweep_truncated", "lite_included", "lite_gen_tps_warm", "lite_gen_src",
                          "lite_cold_ttft_ms", "lite_kv_gib", "measured_utc", "bench_tool", "bench_tool_version")


# ── 자료형 ───────────────────────────────────────────────────────────────────────────────────────────────────
@dataclasses.dataclass
class CellEvidence:
    """셀 하나의 증거(SPEC §5.4 · campaign_plane §10). **모든 값의 출처는 `sources[<필드>]`** 에 있다 — 결정론 산출물은
    출처를 표시한다(헌법 §안전 경계). dict 형 필드는 자기 안에도 `source`/`_source` 를 든다.

    소비자 계약: lineage(`_ev`)·artifacts(`_get`)는 이 객체를 속성 이름으로 읽는다. SPEC 목록 밖으로 두는 것 — `serve_proof`
    (자격 ①) · `phases`(셀 필터된 진행표 사본 · 사람용 출처) · `sources`(필드별 출처) · `repo`. artifacts 는 묶인 attestation 경로를
    `lineage_seeds["attestation"]` 로 읽는다(없으면 `attestation_<셀>.json` 을 가정한다). native 평면 값은
    serve proof producer가 관측·보존한 값만 넣는다. 신호 부재를 native로 추론하지 않는다."""
    mode: str
    campaign_id: str | None
    cell: str
    node: str
    topology: str
    declaration: dict | None = None
    cell_status: dict | None = None
    lockset: dict | None = None
    cell_config: dict | None = None
    triplet: dict = dataclasses.field(default_factory=dict)
    sweep: dict | None = None
    certificate: dict | None = None
    certificate_path: str | None = None
    bench_report_path: str | None = None
    report_kind: str | None = None
    pointers: list = dataclasses.field(default_factory=list)
    attestation: dict | None = None
    build_identity: dict = dataclasses.field(default_factory=dict)
    resolve: dict | None = None
    manifest: dict | None = None
    publication: dict | None = None
    lineage_seeds: dict = dataclasses.field(default_factory=dict)
    missing: list = dataclasses.field(default_factory=list)
    # SPEC 목록 밖(소비자 요구 · interface 보고에 적었다)
    serve_proof: dict | None = None
    # native producer가 serve proof에 관측·보존한 값만 전달한다. None은 "native 아님"이 아니라 신호 부재다.
    plane: str | None = None
    native_install_path: str | None = None
    pip_freeze_path: str | None = None
    pip_freeze_paths: dict = dataclasses.field(default_factory=dict)
    cleanup_attestation_path: str | None = None
    cleanup_attestation: dict | None = None
    # native 기동 기록(2026-09-29 · plan_26092908 §4.2 · D 통합): native_multinode_serve.py 가 serve proof 에 적는 보존 경로(저장소 상대 ·
    #   이미 치환된 배포 바이트) · 쓰지 못했으면 그 사유(`native_launch_error`). 옛 N1 proof 에는 둘 다 없다(None = 신호 부재 → 결손 정상).
    native_launch_path: str | None = None
    native_launch_error: str | None = None
    phases: dict = dataclasses.field(default_factory=dict)       # artifacts.reproduce_steps 가 {phase: {started_utc, ended_utc}}
    sources: dict = dataclasses.field(default_factory=dict)
    repo: str | None = None


# ── 작은 도구 ─────────────────────────────────────────────────────────────────────────────────────────────────
def _repo(repo) -> Path:
    return Path(repo).resolve()


def _rel(repo: Path, path: Path | str) -> str:
    return core.rel(repo, path)


def _read_json_opt(path: Path, what: str):
    """부재 = None(호출부가 결손으로 기재) · **깨진 파일은 부재가 아니다** — 조용히 None 으로 접으면 "없었다" 와
    "읽지 못했다" 가 갈리지 않는다(docs.md §기계판독 데이터 평면 "부재와 결측의 구분")."""
    p = Path(path)
    if not p.exists():
        return None
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        core.fail("HINT_EVIDENCE_UNREADABLE", f"{what} 를 읽을 수 없다({p.name}): {type(e).__name__}: {e}",
                  "파일을 고치거나 지운다 — 깨진 증거를 부재로 읽지 않는다.")


def _unobserved(value) -> bool:
    return value is None or (isinstance(value, str) and value.strip().lower() in _UNOBSERVED)


def _norm_value(value):
    """인증서·스윕 meta 의 '읽지 못함' 표기를 None 으로. 그 밖은 원문(strip)."""
    if _unobserved(value):
        return None
    return value.strip() if isinstance(value, str) else value


def _as_int(value) -> int | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value
    if isinstance(value, str) and re.fullmatch(r"\d+", value.strip()):
        return int(value.strip())
    return None


def _boolish(value) -> bool | None:
    if isinstance(value, bool):
        return value
    if isinstance(value, str):
        low = value.strip().lower()
        if low in _TRUE:
            return True
        if low in _FALSE:
            return False
    return None


def _check_campaign_id(campaign_id) -> str:
    if not isinstance(campaign_id, str) or not campaign_id or "/" in campaign_id or campaign_id in (".", ".."):
        core.fail("HINT_CAMPAIGN_ID_REQUIRED", f"명시 캠페인 id 가 필요하다: {campaign_id!r}",
                  "`--campaign <camp-id>` 로 준다 — ACTIVE 포인터는 읽지 않는다(2026-09-07 Q6).")
    if campaign_id in RESERVED_CAMPAIGN_IDS:
        core.fail("HINT_CAMPAIGN_ID_RESERVED", f"{campaign_id!r} 는 캠페인 인스턴스가 아니다(예약 이름).",
                  "_bootstrap 은 캠페인 밖 대기실이고 _template 은 뼈대다 — 발행 대상 캠페인 id 를 준다.")
    return campaign_id


def _check_cell(cell) -> str:
    if not isinstance(cell, str) or not _CELL_RE.match(cell):
        core.fail("HINT_CELL_INVALID", f"셀 id 가 비었거나 경로 문자를 포함한다: {cell!r}")
    return cell


def _check_node(node) -> str:
    if node not in NODE_AXIS:
        core.fail("HINT_NODE_AXIS_INVALID", f"노드 축은 {list(NODE_AXIS)} 중 하나다: {node!r}")
    return node


# ── 소유 모듈 적재 ─────────────────────────────────────────────────────────────────────────────────────────────
def _cgate_path(repo: Path) -> Path:
    canonical = Path(repo) / core.REL_COMPLETION_GATE
    if canonical.is_file():
        return canonical
    if _COLOCATED_GATE.is_file():
        return _COLOCATED_GATE
    core.fail("HINT_OWNER_MODULE_MISSING", f"completion_gate 부재: {core.REL_COMPLETION_GATE} · 동거 사본 {_COLOCATED_GATE.name}",
              "대체 구현으로 계속하지 않는다(2026-07-31 ModuleNotFoundError 선례) — 정책 런타임이 있는 체크아웃에서 실행한다.")


def cgate(repo):
    """completion_gate 적재(두-레이아웃 · 부재 = die). 공개 API 가 모자라면 동거 사본이 낡은 것이다 — 죽는다."""
    path = _cgate_path(_repo(repo))
    mod = core.load_owner_module(path.parent, path.name, "_hint_completion_gate")
    absent = [a for a in _GATE_REQUIRED_API if not hasattr(mod, a)]
    if absent:
        core.fail("HINT_GATE_API_MISSING", f"completion_gate 에 공개 API {absent} 가 없다({path.name}).",
                  "동거 사본이 낡았을 수 있다 — 정본 `.claude/policies/runtime/completion_gate.py` 로 맞춘다.")
    return mod


def _validator(repo: Path):
    return core.load_owner_module(repo, core.REL_CAMPAIGN_VALIDATOR, "_hint_campaign_validator")


def _rbs(repo: Path):
    return core.load_owner_module(repo, core.REL_RENDER_BENCH_SECTION, "_hint_render_bench_section")


def _smoke_model(repo: Path):
    return core.load_owner_module(repo, REL_CHECK_SMOKE_MODEL, "_hint_check_smoke_model")


def _slave_forward(repo: Path):
    return core.load_owner_module(repo, core.REL_SLAVE_FORWARD, "_hint_slave_forward")


def _owner_yaml(repo: Path, path: Path, what: str):
    """YAML 은 소유 로더(`manifest_contract._load_manifest`)로 읽는다. 로더 부재·PyYAML 부재는 죽는다(대체 파서 ✗)."""
    mc = core.load_owner_module(repo, REL_MANIFEST_CONTRACT, "_hint_manifest_contract")
    loader = getattr(mc, "_load_manifest", None)
    if not callable(loader):
        core.fail("HINT_OWNER_MODULE_MISSING", f"{REL_MANIFEST_CONTRACT} 에 YAML 로더(_load_manifest)가 없다.")
    try:
        return loader(str(path))
    except ImportError as e:
        core.fail("HINT_OWNER_YAML_UNAVAILABLE", f"{what} 를 읽을 YAML 로더가 없다: {e}",
                  "manifest_contract 가 쓰는 PyYAML 을 설치한다(발행은 메인 단독 평면이다 — 수신자 match 는 YAML 불요).")
    except Exception as e:  # noqa: BLE001 — 소유 로더의 파싱 실패를 삼키지 않고 번역한다
        core.fail("HINT_EVIDENCE_UNREADABLE", f"{what} YAML 파손({Path(path).name}): {type(e).__name__}: {e}")


def _manifest(repo: Path, topology: str) -> tuple[dict | None, str]:
    p = repo / "output" / topology / "manifest.yaml"
    if not p.is_file():
        return None, f"output/{topology}/manifest.yaml 부재"
    doc = _owner_yaml(repo, p, "manifest")
    if not isinstance(doc, dict):
        core.fail("HINT_EVIDENCE_UNREADABLE", f"output/{topology}/manifest.yaml 최상위가 사전이 아니다.")
    return doc, f"output/{topology}/manifest.yaml(manifest_contract 로더)"


# ── 인증서 · 게이트 ──────────────────────────────────────────────────────────────────────────────────────────
def parse_certificate(repo, path) -> dict:
    """flat 인증서 → {키: 문자열}. **completion_gate.parse_flat_certificate 한 벌**(옛 3벌 삭제 · F14)."""
    repo = _repo(repo)
    rel = _rel(repo, path)
    p = repo / rel
    if not p.is_file():
        core.fail("HINT_CERTIFICATE_ABSENT", f"인증서 부재: {rel}")
    try:
        text = p.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as e:
        core.fail("HINT_CERTIFICATE_UNREADABLE", f"인증서를 읽을 수 없다({rel}): {type(e).__name__}")
    fields, ok = cgate(repo).parse_flat_certificate(text)
    if not ok:
        core.fail("HINT_CERTIFICATE_UNPARSEABLE", f"flat 인증서 형식이 아니다(들여쓰기·중복 키·잘못된 행): {rel}",
                  "결정론 발행기(publish_benchmark_record.py)가 쓴 원본을 바인딩한다 — 손으로 고친 인증서를 읽지 않는다.")
    if not fields:
        # 게이트 파서는 빈 입력(주석뿐)을 `({}, True)` 로 돌려준다 — 파서의 일은 형식 판정이고, "빈 인증서로 페이로드를 짓지
        #   않는다" 는 발행기의 판정이다(옛 hint_collect.parse_certificate 음성대조 · 2026-09-06). 옮기면서 잃지 않는다.
        core.fail("HINT_CERTIFICATE_UNPARSEABLE", f"인증서가 비었다(주석뿐): {rel}",
                  "빈 인증서로 페이로드를 짓지 않는다 — 벤치마커가 발행한 원본을 바인딩한다.")
    return dict(fields)


def _identity_from_certificate(repo: Path, cert: dict) -> dict:
    g = cgate(repo)
    out: dict = {}
    for f in g.STRONG_IDENTITY_FIELDS:
        out[f] = _norm_value(cert.get(g.CERTIFICATE_FIELD_MAP[f]))
    out["tp"] = _as_int(out.get("tp"))
    return out


def authorize(repo, manifest_path, action: str) -> dict:
    """`completion_gate.py authorize --mode promotion` 한 번(옛 hint_tag._require_promotion_authorization 계약 그대로).

    - 실행 불가·시간 초과 → HINT_GATE_SUBPROCESS_UNAVAILABLE(exit 2) · JSON 판독 불가 → HINT_GATE_OUTPUT_UNREADABLE(exit 2)
    - 거부 → HINT_GATE_DENIED · message = 게이트 stdout **원문** · exit_code = 게이트 종료코드(0 이면 1)
    - 허가 → 파싱한 게이트 JSON
    ★ 호출부는 이것을 **부수효과 전에** 부른다 — 거부는 언제나 부수효과 0 을 뜻한다(2026-07 Phase 3 · 코드맵 §5 불변식 1).
    """
    repo = _repo(repo)
    manifest = Path(manifest_path)
    if not manifest.is_absolute():
        manifest = repo / manifest
    gate_path = _cgate_path(repo)
    env = dict(os.environ)
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    try:
        proc = subprocess.run([sys.executable, str(gate_path), "authorize", "--manifest", str(manifest), "--mode",
                               "promotion", "--action", action, "--repo-root", str(repo)],
                              capture_output=True, text=True, timeout=120, cwd=str(repo), env=env)
    except (OSError, subprocess.TimeoutExpired) as e:
        core.fail("HINT_GATE_SUBPROCESS_UNAVAILABLE", f"completion_gate.py authorize 를 실행할 수 없다: {e}", exit_code=2)
    try:
        parsed = json.loads(proc.stdout)
    except ValueError:
        core.fail("HINT_GATE_OUTPUT_UNREADABLE",
                  f"completion_gate.py authorize (exit={proc.returncode}) 의 stdout 이 JSON 이 아니다; "
                  f"stderr={proc.stderr.strip()[:500]!r}", exit_code=2)
    if proc.returncode == 0 and isinstance(parsed, dict) and parsed.get("allowed") is True:
        return parsed
    core.fail("HINT_GATE_DENIED", proc.stdout if proc.stdout.endswith("\n") else proc.stdout + "\n",
              "게이트 JSON 의 reason_codes 를 해소한다 — 우회하지 않는다(D3).",
              exit_code=proc.returncode if proc.returncode != 0 else 1)


def no_cert_binding_source(manifest, repo=None) -> str | None:
    """인증서가 없을 때 **무엇이 bench_report 바인딩을 여는가**의 단일 판정자(옛 hint_tag._no_cert_binding_source 그대로).

      · `perf_waiver`   — 성능 REFUTE 를 **사람이 서명**해 통과시킨 경로(positive key).
      · `hint_map_only` — 지도 발행 통로(계약 v5 §3 C행 · 2026-09-14 · plan_26091407 §4.5). lite 만 잰 셀은 인증서가
                          **구조적으로** 없다. 종전에는 이 판정자가 map_only 를 몰라서 승격 게이트는 열렸는데 봉인이
                          HINT_CERTIFICATE_EVIDENCE_MISSING 으로 죽었다(audit_26091323 §1 A1 · work-manifest 13건 전부).
                          클래스 선언만으로 여는 이유: 그 클래스로 승격 게이트를 통과했다는 것이 여정 증거와 서빙 성립을
                          이미 뜻한다(completion_gate HINT_MAP_ONLY_PROMOTION) — 여기서 다시 묻지 않는다(평면 분리).
      · `explore`       — 루브릭 권한 explore. 성능 판정이 게이트가 아니라 **서술**이므로 REFUTE 여도 승격이 열린다.
                          explore 는 waiver 를 요구하지 **않는다** — 요구하면 completion_gate U1 자동개방이 여기서 다시
                          죽는다(2026-08-24 실측한 그 죽은 코드의 hint 평면 쌍둥이).
      · `weak`          — 2026-09-29(plan_26092908 §4.8 · V11④ · completion_gate `certificate_waived`): weak 도 explore 처럼
                          **비발행 권한**이다(judge_bench 는 explicit ∧ PASS 만 인증서를 낸다). 판정 원천에서 파생된 권한
                          (`certificate_requirement_authority` — rubric_source=verdict_json)이 weak 이고 rubric 계약을 통과하며
                          판정이 PASS·REFUTE 이면 bench_report 가 판정 근거다. 옛 판본은 explore 만 알아서 weak PASS 가 인증서를
                          요구받았다(게이트는 이제 면제하는데 봉인 쪽 결정자가 막는 이음매).
    순서가 판정이다: waiver 가 map_only 보다 먼저. 넷 다 아니면 None(=차단 — explicit REFUTE·권한 미상은 waiver 가 필요하다).
    ★ 단일 결정자(2026-08-01: finalize 는 리포트로 묶었는데 verify 는 인증서와 대조해 FAIL · 2026-08-24 · 2026-09-14).
    explore 계약은 completion_gate `manifest_rubric_contract` 를 그대로 재사용한다(포인터 원칙 — 손으로 다시 적지 않는다).
    repo=None 이면 이 hintlib 가 든 체크아웃의 게이트를 쓴다.
    """
    if not isinstance(manifest, dict):
        return None
    benchmark = manifest.get("benchmark")
    benchmark = benchmark if isinstance(benchmark, dict) else {}
    if benchmark.get("perf_waiver"):
        return "perf_waiver"
    if manifest.get("task_class") == "hint_map_only":
        return "hint_map_only"
    gate = cgate(repo if repo is not None else core.HINTLIB_DIR.parents[4])
    rubric = gate.manifest_rubric_contract(benchmark)
    if not (rubric.get("declared") and rubric.get("ok")):
        return None
    # 권한은 **판정 원천에서 파생됐다고 표시된** 것만(게이트 한 벌 · 원시 manifest 값으로 면제를 열지 않는다).
    authority = gate.certificate_requirement_authority(benchmark)
    if authority == "explore":
        return "explore"
    if authority in gate.CERTIFICATE_NON_ISSUING_AUTHORITIES and benchmark.get("verdict") in ("PASS", "REFUTE"):
        return authority
    return None


def binding_artifact_path(manifest, repo=None) -> str | None:
    """footer `certificate_ref` 가 가리킬 **계측 산출물 경로**의 단일 소유자(옛 hint_tag._binding_artifact_path).
    통상 인증서 · 인증서가 구조적으로 없을 세 경로(waiver·explore·map_only)면 bench_report · 그 밖 None.
    인증서가 있으면 map_only 여도 인증서가 이긴다."""
    ev = manifest.get("evidence") if isinstance(manifest, dict) else None
    ev = ev if isinstance(ev, dict) else {}
    cert = ev.get("certificate")
    path = cert.get("path") if isinstance(cert, dict) else None
    if path:
        return path
    if no_cert_binding_source(manifest, repo):
        rep = ev.get("bench_report")
        return rep.get("path") if isinstance(rep, dict) else None
    return None


def promotion_binding_problems(manifest, *, tag: str, topology: str, anchor: str) -> list[tuple[str, str]]:
    """work-manifest 의 `promotion_target` 이 **지금 봉인하는 그 태그**를 가리키는가(옛 hint_tag._require_hint_promotion_target
    의 이관 · 코드맵 hint_tag_a §2.G). 빈 목록 = 일치. 옛 함수의 결함 두 가지를 고쳤다: `kind == "hint"` 를 보지 않았고
    (verify 쌍둥이만 봤다), identity 가 없으면 AttributeError 로 죽었다.
    ★ 이름의 vLLM 세그먼트는 빌드 입력(X18)이고 identity.vllm 은 엔진 자기보고라 **비교하지 않는다**(코드맵 H3 — 옛 규칙은
    `0.29.0rc6` 태그를 `0.29.0` 인증서와 대조해 HINT_IDENTITY_VLLM_MISMATCH 로 막았다). 토폴로지 라벨은 자유 텍스트 정규식 대신
    identity 에서 파생한 X7 라벨과 정확히 같아야 한다."""
    if not isinstance(manifest, dict) or not isinstance(manifest.get("promotion_target"), dict):
        return [("HINT_PROMOTION_TARGET_MISSING", "work-manifest 에 promotion_target 이 없다 — `set_promotion_target` 뒤 finalize 가 방출한다")]
    pt = manifest["promotion_target"]
    out: list[tuple[str, str]] = []
    if pt.get("kind") != "hint":
        out.append(("HINT_PROMOTION_TARGET_KIND", f"promotion_target.kind={pt.get('kind')!r} ≠ 'hint'"))
    for key, want, code in (("tag", tag, "HINT_PROMOTION_TARGET_TAG_MISMATCH"),
                            ("topology", topology, "HINT_PROMOTION_TARGET_TOPOLOGY_MISMATCH"),
                            ("anchor", anchor, "HINT_PROMOTION_TARGET_ANCHOR_MISMATCH")):
        if pt.get(key) != want:
            out.append((code, f"promotion_target.{key}={pt.get(key)!r} ≠ 봉인 대상 {want!r}"))
    ident = manifest.get("identity") if isinstance(manifest.get("identity"), dict) else None
    if ident is None:
        out.append(("HINT_IDENTITY_ABSENT", "work-manifest 에 identity 가 없다"))
    else:
        try:
            label = topology_label(ident)
        except core.HintError as e:
            out.append((e.code, e.message))
        else:
            if label != topology:
                out.append(("HINT_IDENTITY_TOPOLOGY_LABEL_MISMATCH",
                            f"identity 파생 라벨 {label!r} ≠ promotion_target.topology {topology!r}(X7)"))
    return out


# ── 발행 기록 ────────────────────────────────────────────────────────────────────────────────────────────────
def publication_records(repo) -> list[dict]:
    """`docs/_evidence/*.json`(`*.work-manifest.json` 제외) 원문 dict + `_id`(파일 stem) — lineage 시드.
    ★ lineage.load_publication_records 와 **같은 규칙**이다(자체검사가 두 결과를 대조한다): 읽지 못한 기록은 버리지 않고
    `{"_id", "_unreadable": 사유}` 로 남긴다(부재와 실패는 다른 사실)."""
    d = _repo(repo) / core.REL_EVIDENCE_DIR
    out: list[dict] = []
    if not d.is_dir():
        return out
    for f in sorted(d.glob("*.json"), key=lambda x: x.name):
        if f.name.endswith(".work-manifest.json"):
            continue
        stem = f.name[: -len(".json")]
        try:
            data = json.loads(f.read_text(encoding="utf-8"))
        except (OSError, ValueError) as e:
            out.append({"_id": stem, "_unreadable": f"{type(e).__name__}: {e}"})
            continue
        if not isinstance(data, dict):
            out.append({"_id": stem, "_unreadable": f"최상위가 객체가 아니다({type(data).__name__})"})
            continue
        rec = dict(data)
        rec["_id"] = stem
        out.append(rec)
    return out


# ── 스윕 ─────────────────────────────────────────────────────────────────────────────────────────────────────
def _sweep_doc(repo: Path, directory: Path, binding: str) -> dict | None:
    idx = _read_json_opt(directory / "sweep_index.json", "sweep_index.json")
    if not isinstance(idx, dict):
        return None
    levels = idx.get("levels") if isinstance(idx.get("levels"), list) else []
    return {"dir": _rel(repo, directory), "index": idx, "generated_utc": idx.get("generated_utc"),
            "levels": levels, "lite": idx.get("lite") if isinstance(idx.get("lite"), dict) else None,
            "verdict": _read_json_opt(directory / "verdict.json", "verdict.json"),
            "roofline": _read_json_opt(directory / "roofline.json", "roofline.json"),
            "bench_mode": _read_json_opt(directory / "bench_mode.json", "bench_mode.json"),
            "binding": binding}


def _bind_sweep(repo: Path, topology: str, cell: str, measured_utc: str | None,
                simlog_dirs=()) -> tuple[dict | None, list[str], str]:
    """(sweep | None, 결손 코드, 출처 문장). 조인 키 없이 이름만으로 묶지 않는다(모듈 docstring · G6)."""
    out_dir = repo / "output" / topology / "benchlog" / f"sweep_{cell}"
    out_doc = _sweep_doc(repo, out_dir, "output") if out_dir.is_dir() else None
    if measured_utc is None:
        if out_doc is None:
            return None, [], "스윕 디렉터리 없음(조인 키도 없음 — lite 셀이면 정상)"
        return None, ["HINT_MISSING_SWEEP"], (f"{out_doc['dir']} 실재하나 조인 키(인증서 measured_utc · 리포트 생성일)가 없어 "
                                             "묶지 않았다")
    if out_doc is not None and out_doc["generated_utc"] == measured_utc:
        return out_doc, [], f"{out_doc['dir']} (generated_utc = measured_utc {measured_utc})"
    refused = (f"{out_doc['dir']} 의 generated_utc={out_doc['generated_utc']!r} ≠ measured_utc={measured_utc!r} — 다른 측정"
               if out_doc is not None else f"output/{topology}/benchlog/sweep_{cell} 부재")
    for sd in simlog_dirs:
        sd = repo / sd
        if sd.is_dir():
            cp = _sweep_doc(repo, sd, "simlog-copy")
            if cp is not None and cp["generated_utc"] == measured_utc:
                return cp, ["HINT_MISSING_SWEEP_RAW"], f"{cp['dir']} (simlog 사본 · {refused})"
    return None, ["HINT_MISSING_SWEEP"], f"묶지 않음 — {refused}"


# 수신자가 풀 수 없는 내부 문서 좌표(2026-09-22 · plan_26092119 S2 round 3 통합) — 결손 뜻 문구는 00 · PAYLOAD.missing 으로 배포된다.
#   옛 ATTESTATION_PARITY_ROWS 뜻이 내부 감사 산출물 좌표('코드맵 campaign_plane TL;DR 6')를 인용했다(2차 저작자 지적). 닫힌 목록(tripwire).
_INTERNAL_DOC_REF = re.compile(r"코드맵|TL;DR")


def _internal_doc_refs(text: str) -> list[str]:
    """배포 문구 속 내부 문서 좌표 목록(빈 목록 = 수신자가 풀 수 있다)."""
    return _INTERNAL_DOC_REF.findall(str(text or ""))


# ── docker (읽기 전용) ───────────────────────────────────────────────────────────────────────────────────────
_DOCKER_READ_ONLY = (("image", "inspect"), ("history",))


def _docker(runner: Callable | None, *args: str) -> str | None:
    """읽기 전용 docker 한 번. 실행 불가·비-0 = None(관측 불가 — 호출부가 결손으로 기재한다)."""
    if not any(tuple(args[:len(p)]) == p for p in _DOCKER_READ_ONLY):
        core.fail("HINT_DOCKER_WRITE_REFUSED", f"evidence 는 docker 읽기만 부른다: {args[:2]}")
    run = runner or subprocess.run
    try:
        proc = run(["docker", *args], capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if getattr(proc, "returncode", 1) != 0:
        return None
    return proc.stdout


def _history_build_args(text: str | None) -> tuple[dict, dict]:
    """`docker history --no-trunc --format '{{.CreatedBy}}'` → ({ARG: 값}, {ARG: [상충 값들]}).
    형식 `RUN |N K=V … /bin/sh -c …` 의 앞 N 토큰이 build-arg 다(2026-09-21 tag2 이미지 실측)."""
    seen: dict[str, list[str]] = {}
    for line in (text or "").splitlines():
        m = _HISTORY_ARGS.match(line.strip())
        if not m:
            continue
        for tok in m.group(2).split(" ")[: int(m.group(1))]:
            if "=" in tok:
                k, v = tok.split("=", 1)
                if re.fullmatch(r"[A-Z_][A-Z0-9_]*", k):
                    vals = seen.setdefault(k, [])
                    if v not in vals:
                        vals.append(v)
    args = {k: v[0] for k, v in sorted(seen.items())}
    conflicts = {k: v for k, v in sorted(seen.items()) if len(v) > 1}
    return args, conflicts


def _image_facts(ref: str, runner: Callable | None) -> dict | None:
    raw = _docker(runner, "image", "inspect", ref)
    if raw is None:
        return None
    try:
        doc = json.loads(raw)
    except ValueError:
        return None
    doc = doc[0] if isinstance(doc, list) and doc else doc
    if not isinstance(doc, dict):
        return None
    cfg = doc.get("Config") if isinstance(doc.get("Config"), dict) else {}
    env = {}
    for item in cfg.get("Env") or []:
        if isinstance(item, str) and "=" in item:
            k, v = item.split("=", 1)
            if k in ("NVIDIA_PYTORCH_VERSION", "PYTORCH_VERSION", "CUDA_VERSION"):
                env[k] = v
    labels = cfg.get("Labels") if isinstance(cfg.get("Labels"), dict) else {}
    history = _docker(runner, "history", "--no-trunc", "--format", "{{.CreatedBy}}", ref)
    args, conflicts = _history_build_args(history)
    return {"id": doc.get("Id"), "created": doc.get("Created"), "architecture": doc.get("Architecture"), "env": env,
            "compose_project": labels.get("com.docker.compose.project"), "history_build_args": args,
            "history_build_args_conflicts": conflicts, "history_observed": history is not None}


def _env_first(repo: Path, rel_path: str | None) -> dict:
    """셀 env 의 첫 KEY=VALUE — upstream `slave_forward.read_env_first`(스모크 `val()` 의미의 단일 소유자)."""
    if not rel_path:
        return {}
    return dict(_slave_forward(repo).read_env_first(repo / rel_path))


def _dockerfile_args(repo: Path, topology: str, dockerfile: str | None) -> tuple[frozenset | None, str]:
    """쓰인 Dockerfile 자신의 `^ARG` 이름 — upstream `render_dockerfile.ledger_build_args`(원장 build-arg 목록의 단일 소유자).
    docker history 에는 NGC 베이스 이미지의 빌드 인자(내부 URL 포함 수십 개)가 섞인다(2026-09-21 wheel 이미지 실측) —
    **우리 Dockerfile 의 ARG 만** 빌드 정체로 싣는다. Dockerfile 을 읽지 못하면 None(소속 판정 불가 → 싣지 않는다)."""
    if not dockerfile or "/" in dockerfile:
        return None, "쓰인 Dockerfile 선택자 없음 — history build-arg 소속 판정 불가(싣지 않는다)"
    p = repo / "output" / topology / dockerfile
    if not p.is_file():
        return None, f"output/{topology}/{dockerfile} 부재 — history build-arg 소속 판정 불가(싣지 않는다)"
    rd = core.load_owner_module(repo, core.REL_RENDER_DOCKERFILE, "_hint_render_dockerfile")
    fn = getattr(rd, "ledger_build_args", None)
    if not callable(fn):
        core.fail("HINT_OWNER_MODULE_MISSING", "render_dockerfile.ledger_build_args 부재(원장 build-arg 목록의 소유자).")
    return frozenset(fn(p.read_text(encoding="utf-8", errors="replace"))), \
        f"output/{topology}/{dockerfile} 의 ^ARG(render_dockerfile.ledger_build_args)"


def _build_identity(repo: Path, *, topology: str, env: dict, env_rel: str | None, measured: dict,
                    manifest: dict | None, runner: Callable | None) -> tuple[dict, list[str]]:
    """빌드 정체(PAYLOAD.build 모양 + 관측 원문). 관측 > 선언: history build-arg 가 셀 env 를 이긴다(상충은 기록)."""
    src: dict[str, str] = {}
    missing: list[str] = []
    env_src = f"셀 env({env_rel})" if env_rel else "셀 env 부재"
    image_tag = env.get("IMAGE_TAG") or measured.get("image_tag")
    src["image_tag"] = f"{env_src} IMAGE_TAG" if env.get("IMAGE_TAG") else measured.get("image_tag_source", "미관측")
    digest = measured.get("image_digest")
    if not isinstance(digest, str) or digest.strip() in ("", "NA", "N/A"):
        digest = None    # 스윕 meta 의 결측 표지 "NA" 는 digest 가 아니다(2026-09-23 N1: 참으로 평가돼 태그 관측을 건너뛰었다)
    src["image_digest"] = measured.get("image_digest_source", "미측정")
    facts = None
    if digest:
        facts = _image_facts(digest, runner)           # 내용 권위 = digest(태그는 이름 · 2026-09-11 R5)
        if facts is None and image_tag:
            tag_facts = _image_facts(image_tag, runner)
            if tag_facts is not None and tag_facts.get("id") != digest:
                src["image_observation"] = (f"로컬 태그 {image_tag} 는 측정 digest 와 다른 이미지({str(tag_facts.get('id'))[:19]}…)를 "
                                            "가리킨다 — 그 이미지의 history 를 빌드 정체로 쓰지 않는다")
        elif facts is not None:
            src["image_observation"] = f"docker image inspect/history {digest[:19]}…(측정 digest)"
    elif image_tag:
        facts = _image_facts(image_tag, runner)
        if facts is not None:
            src["image_observation"] = f"docker image inspect/history {image_tag}(측정 digest 미기재 — 태그로 관측)"
    if facts is None:
        missing.append("HINT_MISSING_BUILD_IDENTITY")
        src.setdefault("image_observation", "관측 불가(이미지 부재·docker 미가용·태그 이동)")
    docker_plane = bool(env.get("IMAGE_TAG") or env.get("BUILD_DOCKERFILE") or digest or facts is not None)
    dockerfile = env.get("BUILD_DOCKERFILE") or None
    if dockerfile:
        src["dockerfile"] = f"{env_src} BUILD_DOCKERFILE"
    elif docker_plane:
        # 셀 env 에 선택자가 없으면 빌드는 compose 의 `${BUILD_DOCKERFILE:-…}` 기본값을 썼다 — 그 기본값의 읽기는
        #   upstream slave_forward.compose_default 가 소유한다(스모크의 옛 하드 폴백 K10 을 대체한 그 함수).
        compose = repo / "output" / topology / "docker-compose.yaml"
        dockerfile = _slave_forward(repo).compose_default(compose, "BUILD_DOCKERFILE") if compose.is_file() else None
        src["dockerfile"] = (f"output/{topology}/docker-compose.yaml 기본값 ${{BUILD_DOCKERFILE:-{dockerfile}}}"
                             "(셀 env 선택자 없음 · 현재 compose 기준)" if dockerfile
                             else "선택자 없음(셀 env·compose 기본값 모두 부재)")
    else:
        src["dockerfile"] = "docker 평면 신호 없음(IMAGE_TAG·선택자·측정 이미지 부재)"
    raw_args = (facts or {}).get("history_build_args") or {}
    own, src["history_build_args"] = _dockerfile_args(repo, topology, dockerfile)
    hargs = {k: v for k, v in raw_args.items() if own is not None and k in own}
    hconf = {k: v for k, v in ((facts or {}).get("history_build_args_conflicts") or {}).items()
             if own is not None and k in own}
    if dockerfile == "Dockerfile":
        track = "wheel"
    elif dockerfile:
        track = "source-build"
    else:
        # 선택자를 관측하지 못했다 — 트랙을 추측하지 않는다(naming 이 파생 불가로 막는다). docker 신호가 **없다**는 것도
        #   native 의 관측이 아니다: 평면 판정의 단일 소유자는 artifacts.plane_of 이고, 그것은 "신호 없음 = native" 를
        #   추측하지 않는다(2026-09-22 적대 검토 — 옛 판본은 여기서 native 로 접어 두 모듈이 다른 말을 했다).
        track = None
    src["track"] = (f"선택자 {dockerfile}" if dockerfile else
                    "선택자 미관측 — 트랙 미정" if docker_plane else
                    "docker 평면 신호 없음 — 트랙 미정(native 여부는 artifacts.plane_of 가 관측 신호로 판정)")
    conflicts: dict[str, list] = {}
    for key in ("VLLM_REF", "VLLM_REPO", "VLLM_VERSION"):
        if key in hargs and env.get(key) not in (None, "") and env[key] != hargs[key]:
            conflicts[key] = [f"env:{env[key]}", f"history:{hargs[key]}"]
    # ★ 트랙 ↔ 내용(2026-09-04 실측: `-wheel` 태그에 0.27.1 소스빌드 내용). 선택자가 가리킨 Dockerfile 의 정체 인자(소스빌드
    #   `VLLM_REF` · wheel `VLLM_VERSION`)가 **관측된 history 에 없으면** 그 이미지는 다른 Dockerfile 로 지어졌다 — 그때 위
    #   필터는 인자를 전부 걸러 내고 이름은 셀 env 선언으로 조용히 폴백한다(2026-09-22 적대 검토). 두 트랙 이미지의 history 는
    #   각자 자기 인자를 든다(실측: 0.18.0-wheel 에 VLLM_VERSION ×5 · 0.29.0rc6-source 에 VLLM_REF ×13).
    track_key = {"wheel": "VLLM_VERSION", "source-build": "VLLM_REF"}.get(track or "")
    if track_key and raw_args and own is not None and track_key in own and track_key not in raw_args:
        conflicts["BUILD_DOCKERFILE"] = [f"env:{dockerfile}",
                                         f"history:{track_key} build-arg 없음(다른 Dockerfile 로 지은 이미지)"]
    fenv = (facts or {}).get("env") or {}

    def pick(key: str) -> tuple[str | None, str]:
        if key in hargs:
            return hargs[key] or None, f"docker history build-arg {key}"
        if env.get(key):
            return env[key], f"{env_src} {key}"
        return None, f"{key} 미관측"

    vllm_ref, src["vllm_ref"] = pick("VLLM_REF")
    vllm_repo, src["vllm_repo"] = pick("VLLM_REPO")
    vllm_version, src["vllm_version"] = pick("VLLM_VERSION")
    out = {
        "track": track, "dockerfile": dockerfile, "image_tag": image_tag, "image_digest": digest,
        "vllm_repo": vllm_repo, "vllm_ref": vllm_ref, "vllm_version": vllm_version, "vllm_sha": None,
        "torch": fenv.get("PYTORCH_VERSION"), "cuda": fenv.get("CUDA_VERSION"),
        "ngc": fenv.get("NVIDIA_PYTORCH_VERSION"),
        "cpu_arch": (facts or {}).get("architecture") or (manifest or {}).get("cpu_arch"),
        "created": (facts or {}).get("created"), "compose_project": (facts or {}).get("compose_project"),
        "history_build_args": hargs, "history_build_args_conflicts": hconf,
        "env_history_conflicts": conflicts,
        "wheel_url": next((env[k] for k in _WHEEL_URL_KEYS if env.get(k)), None),
    }
    for k, what in (("torch", "PYTORCH_VERSION"), ("cuda", "CUDA_VERSION"), ("ngc", "NVIDIA_PYTORCH_VERSION")):
        src[k] = f"docker image inspect Config.Env {what}" if out[k] else "이미지 관측 불가"
    src["cpu_arch"] = ("docker image inspect Architecture" if (facts or {}).get("architecture")
                       else "manifest cpu_arch" if out["cpu_arch"] else "미관측")
    # native 셀(2026-09-23 · plan_26092311): 스윕 meta 의 이미지는 **실행 평면이 아니라** native wheelhouse 를 재포장한 원천 이미지다
    #   (sweep_bench 가 serve proof 에서 옮긴 값). 빌드 정체(트랙·VLLM_REF·torch)는 그 원천 이미지의 것이 맞지만, 평면 판정자가
    #   이것을 docker 신호로 읽지 않도록 표지한다.
    out["native_wheelhouse_source"] = bool(not env.get("IMAGE_TAG") and "native serve proof" in str(src.get("image_tag") or ""))
    out["source"] = src
    return out, missing


# 측정 호스트의 OS·커널을 적는 키 후보(인증서·스윕 meta·manifest). 2026-09-22 현재 이 저장소의 어느 producer 도 쓰지 않는다 — 키가
#   생기면 그때 읽히고, 없으면 미관측이다(지어내지 않는다 · S2 round 2 F4: "OS 는 실제 원천이 있을 때만").
#   2026-09-22(S2 round 2 리뷰): `platform` 은 뺐다 — 이 저장소에서 그 낱말은 docker `--platform`(linux/arm64 · 이미지 아키텍처)
#   뜻으로도 쓰여 OS 사실로 읽으면 오기가 된다. 뜻이 하나인 키만 둔다.
_OS_KEYS = ("os", "os_release", "os_name", "os_version", "kernel", "kernel_release")


_DECLARED_COPY = "(manifest 선언 옮김)"


def _attested_host(att: dict | None, key: str, timing_note: str | None = None) -> tuple[str | None, str | None, dict]:
    """노드 정합 attestation 이 **노드별로 관측한** 호스트 사실(v2 `parity.<key>{main,sub,verdict}` > `nodes.<n>.<key>`).
    반환 (값 | None, 출처 | None, {노드: 값}). 노드끼리 다르면 값은 main 의 것이고 출처가 그 사실(노드별 상이)을 말한다 —
    한 값으로 접지 않는다(by_node 에 둘 다 남는다). attestation 이 없거나 칸이 없으면 (None, None, {})."""
    if not isinstance(att, dict):
        return None, None, {}
    path = att.get("_path") or "attestation"
    row = (att.get("parity") or {}).get(key) if isinstance(att.get("parity"), dict) else None
    by_node: dict[str, str] = {}
    if isinstance(row, dict):
        by_node = {n: str(row[n]).strip() for n in ("main", "sub") if not _unobserved(row.get(n))}
        where = f"attestation parity.{key}({path} · 노드별 관측 · verdict={row.get('verdict')})"
    if not by_node and isinstance(att.get("nodes"), dict):
        by_node = {n: str(d[key]).strip() for n, d in sorted(att["nodes"].items())
                   if isinstance(d, dict) and not _unobserved(d.get(key)) and isinstance(d.get(key), (str, int, float))}
        where = f"attestation nodes.<n>.{key}({path} · 노드별 관측)"
    if not by_node:
        return None, None, {}
    vals = sorted(set(by_node.values()))
    val = by_node.get("main") or vals[0]
    if len(vals) > 1:
        where += " — ⚠ 노드별 상이(" + " · ".join(f"{n}={v}" for n, v in sorted(by_node.items())) + " · 값 = main)"
    else:
        where += " — " + "=".join(sorted(by_node)) + " 일치"
    # 범위 표지(attestation_scope 와 같은 결): 파일은 같은 config 의 다음 실행이 덮는다 — 이 측정 실행의 것인지는 미검증이다.
    #   G10: 같은 기동으로 판별됐으면(timing_note 에 판별 표지) 미검증 문장을 달지 않는다.
    if not (timing_note and (SAME_BOOT_MARK in timing_note or WEAK_SAME_BOOT_MARK in timing_note)):
        where += " · 이 측정 실행과 같은 실행인지 미검증(같은 호스트의 관측)"
    if timing_note:
        where += f" · {timing_note}"          # F7(2026-09-29): 작성 시각 대 측정 창
    return val, where, by_node


def _host_facts(ev: CellEvidence) -> None:
    """build_identity 에 측정 호스트 사실 `driver`·`os` 를 **출처와 함께** 싣는다(2026-09-22 · plan_26092119 S2 round 2 · F4).
    1차 재생의 FACT 는 인증서·스윕 meta 가 `driver_version` 을 들고 있었는데도 드라이버를 "미관측" 이라 적었다.

    ★ 2026-09-29(plan_26092908 §4.5 · V3 — **관측 > 선언**): 옛 우선순위(인증서 > 스윕 meta > manifest)는 앞 둘이 manifest 선언을
      옮긴 값이라는 사실을 몰랐다 — sweep_bench.sh 는 `driver_version` 을 `grep_yaml(manifest)` 로 채우고(측정 ✗), 인증서는 그
      meta 를 옮긴다(publish_benchmark_record.py 소프트 지문). 그래서 DS4F·D1·N1 이 manifest 의 580.173.02 를 "측정 기록" 으로
      봉인했고, 같은 zip 의 attestation parity 는 실측 580.178.04 를 들고 있었다. 새 우선순위:
        ① attestation parity(노드별 관측 · cluster 축에서만 묶인다) > ② 스윕 meta > ③ 인증서 > ④ manifest.
      ②③ 은 `driver_version_source` 가 `measured…` 로 스스로 관측을 말하지 않으면 출처에 `(manifest 선언 옮김)` 을 단다.
      관측(①, 또는 measured 로 표지된 ②③)과 선언이 다르면 `driver_conflict = {observed, declared, sources}` 로 **둘 다** 싣는다.
    OS·커널도 같은 순서로 attestation 노드 칸이 있으면 먼저 읽는다(2026-09-29 현재 attestation 에 그 칸은 없다 — 생기면 읽힌다)."""
    b = ev.build_identity
    src = b.setdefault("source", {})
    cert = ev.certificate or {}
    meta = ((ev.sweep or {}).get("index") or {}).get("meta") or {}
    man = ev.manifest or {}
    man_where = ev.sources.get("manifest") or "manifest"

    def measured_src(doc: dict) -> bool:
        return str(doc.get("driver_version_source") or "").strip().startswith("measured")

    obs_v, obs_src, by_node = _attested_host(ev.attestation, "driver", _attestation_timing_note(ev))
    # 2026-09-29 plan_26092908 §4.5 F10: 관측 > 선언은 **같은 run 의 관측**에만 적용한다. 묶인 attestation 이 다른 run 의 것이면
    #   (native 셀 = 원천 Docker 셀의 attestation · 빌드 config 이름으로 묶인 셀 — attestation_scope 의 OTHER_RUN 과 같은 판정) 그 값을
    #   "관측" 층으로 올리지 않는다 — 값은 선언 층이 갖고, 다른 run 의 관측은 **범위를 붙여** 출처에 싣는다.
    att_cfg = (ev.attestation or {}).get("config")
    other_run = bool(obs_v) and (ev.plane == "native" or (att_cfg not in (None, "") and att_cfg != ev.cell))
    layers: list[tuple[str | None, str, bool]] = []          # (값, 출처, 관측인가)
    if obs_v and not other_run:
        layers.append((obs_v, obs_src, True))
    for doc, where in ((meta, f"sweep_index.meta.driver_version({(ev.sweep or {}).get('dir')} · 측정 시 조립"),
                       (cert, f"인증서 driver_version({Path(ev.certificate_path or '').name} · 스윕 meta 의 옮김")):
        v = _norm_value(doc.get("driver_version")) if isinstance(doc, dict) else None
        if v:
            obs = measured_src(doc)
            layers.append((str(v), where + (f" · {doc.get('driver_version_source')})" if obs else
                                            f" — sweep_bench.sh 가 manifest 에서 읽는다) {_DECLARED_COPY}"), obs))
    mv = _norm_value(man.get("driver_version")) if isinstance(man, dict) else None
    if mv:
        layers.append((str(mv), f"{man_where} driver_version(스캔 시점 선언 — 측정 시각 값 미보장)", False))
    b["driver"], src["driver"] = None, "미관측(attestation·인증서·스윕 meta·manifest 에 driver 없음)"
    if layers:
        b["driver"], src["driver"] = layers[0][0], layers[0][1]
    b.pop("driver_by_node", None)
    if by_node and not other_run:
        b["driver_by_node"] = by_node
        src["driver_by_node"] = obs_src
    if other_run:
        who = ("원천 셀" if ev.plane == "native" else "다른 run") + f"({(ev.attestation or {}).get('config')})"
        nodes = " · ".join(f"{n}={v}" for n, v in sorted(by_node.items()))
        note = (f"{who} attestation 관측 {obs_v}({nodes} · 이 run 아님 · 같은 호스트 — 관측 > 선언 규칙은 같은 run 의 관측에만 적용 · "
                f"{obs_src})")
        if b["driver"] is None:
            b["driver"], src["driver"] = obs_v, f"{note} — 이 run 의 관측·선언 모두 없어 그 값을 옮긴다(범위 = 다른 run)"
        else:
            src["driver"] += f" · {note}"
            if b["driver"] != obs_v:
                src["driver"] += f" · ⚠ 선언 {b['driver']} 과 다르다(이 run 에서 관측되지 않았다 — 판정 ✗)"
    observed = next((l for l in layers if l[2]), None)
    declared = next((l for l in layers if not l[2]), None)
    b.pop("driver_conflict", None)
    if observed and declared and observed[0] != declared[0]:
        b["driver_conflict"] = {"observed": observed[0], "declared": declared[0],
                                "sources": {"observed": observed[1], "declared": declared[1]}}
        src["driver"] += f" · ⚠ 선언 {declared[0]} 과 다르다(driver_conflict)"
    b["os"], src["os"] = None, f"미관측(attestation·인증서·스윕 meta·manifest 에 {list(_OS_KEYS)} 키 없음 — 지어내지 않는다)"
    for k in _OS_KEYS:
        v, where, _bn = _attested_host(ev.attestation, k)
        if v:
            b["os"], src["os"] = v, f"{where} {k}"
            return
    for doc, where in ((meta, "sweep_index.meta"), (cert, "인증서"), (man, man_where)):
        hit = next(((k, doc[k]) for k in _OS_KEYS if isinstance(doc, dict) and not _unobserved(doc.get(k))
                    and isinstance(doc.get(k), (str, int, float))), None)
        if hit:
            b["os"], src["os"] = str(hit[1]).strip(), f"{where} {hit[0]}"
            break


def _local_image_digest(repo: Path, ev: CellEvidence, runner: Callable | None) -> None:
    """측정 digest 가 기록되지 않은 셀(lite · native 원천 이미지)의 **현 로컬 이미지** digest 관측(2026-09-29 · plan_26092908 §4.5 F2).
    읽기 전용 `docker image inspect <IMAGE_TAG>`(Id · Created) — 태그는 가변 포인터라 이 값은 측정 digest 가 아니다. 판정 규칙:
    Created < 측정 시각(evidence `_measured_key` — 인증서 · 스윕 · lite 리포트 생성일) 이면 "측정 전 빌드 — 같은 빌드로 판정 가능(그 사이
    재태깅은 관측 밖)" · 아니면 "측정 뒤 빌드 — 측정 이미지 아님" 으로 표시한다. 결과는 `build_identity.image_digest_local` 에만 싣는다 —
    `image_digest`(측정 digest · 내용 권위)는 건드리지 않는다(artifacts 가 그 값을 '측정 digest' 로 읽는다). PAYLOAD 행 채움은 hint._build_doc."""
    b = ev.build_identity if isinstance(ev.build_identity, dict) else None
    if b is None:
        return
    b.pop("image_digest_local", None)
    tag = b.get("image_tag")
    if b.get("image_digest") or not isinstance(tag, str) or not tag.strip():
        return
    facts = _image_facts(tag, runner)
    if facts is None or not facts.get("id"):
        b["image_digest_local"] = {"id": None, "source": f"docker image inspect {tag} 관측 불가(이미지 부재 · docker 미가용)"}
        return
    created = _utc_z_soft(facts.get("created"))
    measured = _measured_key(repo, ev)
    iid = str(facts["id"])
    att_note = _node_image_id_note(ev.attestation, iid)
    who = "원천(native wheelhouse 재포장 원천) " if ev.plane == "native" else ""
    base = f"docker image inspect {tag} .Id · .Created {created or '미관측'}(현 로컬 {who}이미지 관측 · 태그는 가변 포인터){att_note}"
    if created and measured and created < measured:
        b["image_digest_local"] = {"id": iid, "created": created, "same_build": True,
                                   "source": f"{base} · Created < 측정 {measured} → 측정 전 빌드 — 같은 빌드로 판정 가능(그 사이 재태깅은 관측 밖)"}
    else:
        why = (f"Created {created} ≥ 측정 {measured} → 측정 뒤 빌드 — 측정 이미지 아님" if created and measured
               else "Created 또는 측정 시각 미관측 — 측정 전후 판정 불가")
        b["image_digest_local"] = {"id": iid, "created": created, "same_build": False, "source": f"{base} · {why}"}


def _node_image_id_note(att, iid: str) -> str:
    """현 로컬(메인) 이미지 Id 대 묶인 attestation 의 **노드별** image_id(2026-09-29 · FACT_FIX2 G6). 옛 판은 노드 image_id 집합에 들어 있으면
    "같다" 한 마디였다 — cluster 셀은 노드마다 로컬 빌드라 서브 image_id 가 다르고(특화헌법: 동일 ABI 이지 동일 digest 가 아니다), 한 마디는
    단일 이미지로 읽혔다. 노드마다 같다/다르다(다르면 그 id)를 적는다. attestation 에 노드 image_id 가 없으면 ""."""
    nodes = (att or {}).get("nodes") if isinstance(att, dict) else None
    by = {str(n): str(d.get("image_id")) for n, d in sorted(nodes.items())
          if isinstance(d, dict) and d.get("image_id")} if isinstance(nodes, dict) else {}
    if not by:
        return ""
    parts = [f"{n} {'같다' if v == iid else f'다르다({v[:19]}…)'}" for n, v in by.items()]
    note = f" · 묶인 attestation 노드별 image_id({att.get('_path')}): {' · '.join(parts)}"
    if len(set(by.values())) > 1:
        note += " — 노드별 로컬 빌드라 digest 가 노드마다 다른 것은 정상(요건은 동일 ABI 이지 동일 digest 가 아니다 · 이 행의 Id 는 메인 로컬 관측)"
    return note


def _utc_z_soft(v) -> str | None:
    """docker `.Created`(RFC3339 · 나노초 · 오프셋) → `YYYY-MM-DDTHH:MM:SSZ`. 못 읽으면 None."""
    if not isinstance(v, str) or not v.strip():
        return None
    m = re.match(r"^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}:\d{2})(?:\.\d+)?(Z|[+-]\d{2}:\d{2})$", v.strip())
    if not m:
        return None
    if m.group(3) == "Z":
        return f"{m.group(1)}T{m.group(2)}Z"
    try:
        dt = _dt.datetime.fromisoformat(f"{m.group(1)}T{m.group(2)}{m.group(3)}")
    except ValueError:
        return None
    return dt.astimezone(_dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _bind_resolve(repo: Path, topology: str, build: dict) -> tuple[dict | None, str]:
    """resolved.json 은 토폴로지 단위로 덮인다(다음 resolve 가 덮는다). **이 빌드의 버전을 해소한 것일 때만** 묶는다."""
    p = repo / "output" / topology / "resolved.json"
    doc = _read_json_opt(p, "resolved.json")
    if not isinstance(doc, dict):
        return None, f"output/{topology}/resolved.json 부재"
    from . import naming       # 릴리스 문법의 단일 소유자(D-g — 4마디·.postN 포함)
    want = None
    ref = str(build.get("vllm_ref") or "").strip().lower()
    rel = naming.release_of(ref)
    if rel:
        want = rel
    elif build.get("vllm_version"):
        want = str(build["vllm_version"]).lstrip("v")
    got = str(doc.get("vllm_version") or "").lstrip("v")
    delta = doc.get("upstream_delta") if isinstance(doc.get("upstream_delta"), dict) else {}
    to_sha = str(delta.get("to_sha") or "").lower()
    # 커밋 핀(40자 VLLM_REF)은 버전 문자열이 아니라 **해소된 커밋**으로 묶는다 — 해소값의 to_sha 가 그 핀일 때만.
    by_sha = bool(_SHA40.match(ref)) and to_sha == ref
    if not by_sha and (not want or got != want):
        return None, (f"output/{topology}/resolved.json vllm_version={got!r}·to_sha={to_sha[:12] or None!r} ≠ 빌드 입력 "
                      f"{want or ref[:12] or None!r} — 다른 해소값이라 묶지 않는다")
    torch = doc.get("torch") if isinstance(doc.get("torch"), dict) else {}
    ngc = doc.get("ngc_base") if isinstance(doc.get("ngc_base"), dict) else {}
    bt = doc.get("build_track") if isinstance(doc.get("build_track"), dict) else {}
    sb = doc.get("source_build") if isinstance(doc.get("source_build"), dict) else {}
    return ({"vllm_version": doc.get("vllm_version"), "torch_pin": torch.get("pin"), "ngc_tag": ngc.get("tag"),
             "ngc_image": ngc.get("image"), "ngc_cuda": ngc.get("cuda_version"), "ngc_torch_build": ngc.get("build_version"),
             "build_track": bt.get("decision"), "torch_cuda_arch": sb.get("torch_cuda_arch"),
             "to_ref": delta.get("to_ref"), "to_sha": to_sha if _SHA40.match(to_sha) else None,
             "generated_kst": delta.get("generated_kst"),
             "source": (f"output/{topology}/resolved.json(upstream_delta.to_sha = 빌드 입력 커밋 핀)" if by_sha
                        else f"output/{topology}/resolved.json(vllm_version={got} = 빌드 입력)")},
            f"output/{topology}/resolved.json")


def _bind_attestation(repo: Path, topology: str, node: str, cell: str, build: dict) -> tuple[dict | None, list[str], str]:
    """노드 정합 attestation. 이름은 **셀** 이름이 먼저, 없으면 이미지 compose 라벨의 빌드 config 이름(2026-09-21 ·
    코드맵 TL;DR 6: 태그2 는 빌드 config `q38fn-nvfp4-mmap` 이름으로 남았다). 다른 이미지의 attestation 은 묶지 않는다."""
    if topology != "multi" or node != "cluster":
        # 노드 정합은 **두 노드가 한 측정 정체성**일 때(cluster)의 사실이다. 멀티 체크아웃의 단일 노드 실행(main·sub)은
        #   collective 에 참여하지 않는다 — 대상이 아닌 것을 결손으로 적으면 결손 목록이 거짓을 말한다.
        return None, [], f"{topology}/{node} — 노드 정합 attestation 대상 아님(cluster 축 전용)"
    bench = repo / "output" / topology / "benchlog"
    names = [(f"attestation_{cell}.json", "셀 이름", "cell-name")]
    project = build.get("compose_project") or ""
    pm = re.fullmatch(r"vllm_(.+)_project", project)
    if pm and pm.group(1) != cell:
        names.append((f"attestation_{pm.group(1)}.json", f"이미지 compose 라벨 {project}", "compose-label"))
    refused: list[str] = []
    for name, why, bound_by in names:
        doc = _read_json_opt(bench / name, "attestation")
        if not isinstance(doc, dict):
            continue
        if doc.get("image_tag") and build.get("image_tag") and doc["image_tag"] != build["image_tag"]:
            refused.append(f"{name}: image_tag {doc['image_tag']!r} ≠ {build['image_tag']!r}")
            continue
        # ★ 태그는 가변 포인터다(2026-09-11 R5) — v2 는 실행 중 컨테이너의 image id 를 노드별로 든다. 측정 digest 와 다른
        #   이미지의 attestation 은 같은 태그라도 **다른 빌드의 정합**이다(셀 id·빌드 config 이름 재사용 · 2026-09-22 적대 검토).
        digest = build.get("image_digest")
        ids = sorted({str(n.get("image_id")) for n in (doc.get("nodes") or {}).values()
                      if isinstance(n, dict) and n.get("image_id")}) if isinstance(doc.get("nodes"), dict) else []
        if digest and ids and digest not in ids:
            refused.append(f"{name}: 노드 image_id {ids} 에 측정 digest {str(digest)[:19]}… 가 없다")
            continue
        att = dict(doc)
        att["_path"] = _rel(repo, bench / name)
        att["_source"] = f"{att['_path']} ({why})"
        att["_bound_by"] = bound_by          # attestation_scope 가 읽는다(2026-09-22 S2 round 3)
        return att, ([] if _attestation_has_rows(doc) else ["HINT_MISSING_ATTESTATION_PARITY_ROWS"]), att["_source"]
    return None, ["HINT_MISSING_SLAVE_ATTESTATION"], (f"attestation 부재({', '.join(n for n, _, _ in names)})"
                                                     + (f" · 다른 이미지라 묶지 않음: {refused}" if refused else ""))


def _attestation_has_rows(doc: dict) -> bool:
    """대조 행이 **관측됐는가**. v1(`checks[]` · 빌드 시점 행) 또는 v2(`parity{축: {verdict}}` · serve 시점 캡처 · X12).
    v2 의 `checks` 는 그 실행에 `--build` 가 있었을 때만 채워진다 — 비었다고 결손으로 적으면 정상 v2 가 전부 결손이 된다
    (2026-09-22 적대 검토 · AC8 "attestation 결손 0"). 모든 축이 `unobservable` 이면 관측이 없는 것이다(부재 ≠ 일치)."""
    checks = doc.get("checks")
    if isinstance(checks, list) and checks:
        return True
    parity = doc.get("parity")
    if isinstance(parity, dict):
        return any(isinstance(r, dict) and r.get("verdict") in ("equal", "differ") for r in parity.values())
    return False


def _triplet(repo: Path, topology: str, cell: str) -> dict:
    cand = {"yaml": f"output/{topology}/configs/{cell}.yaml", "sh": f"output/{topology}/configs/{cell}.sh",
            "env": f"output/{topology}/envs/.env.{cell}"}
    return {k: v for k, v in cand.items() if (repo / v).is_file()}


def _serve_proof(repo: Path, topology: str, cell: str) -> tuple[dict | None, str | None]:
    p = repo / "output" / topology / "benchlog" / f"serve_proof_{cell}.json"
    doc = _read_json_opt(p, "serve_proof")
    return (doc, _rel(repo, p)) if isinstance(doc, dict) else (None, None)


def _preserved_native_file(repo: Path, raw, field: str) -> tuple[str, Path]:
    """Native producer only provides preserved repo-relative regular files.

    Do not normalize a missing/malformed producer signal into an absent native file: the
    native consumer must fail closed, and payloads must never inherit host absolute paths.
    """
    if not isinstance(raw, str) or not raw.strip():
        core.fail("HINT_NATIVE_EVIDENCE_MISSING", f"native serve proof의 {field} 가 없다.",
                  "native producer가 run-root 삭제 전에 보존한 repo-relative evidence 경로를 기록해야 한다.")
    rel = raw.strip()
    if Path(rel).is_absolute():
        core.fail("HINT_NATIVE_EVIDENCE_PATH_ABSOLUTE", f"native serve proof의 {field} 는 repo-relative 여야 한다: {rel!r}")
    # Resolve first, so a symlink that escapes the repository is rejected by the same owner
    # helper; reject every symlink afterward so payload inputs are regular preserved files.
    path = repo / rel
    clean = core.rel(repo, path)
    lexical = repo / rel
    if not lexical.is_file() or lexical.is_symlink():
        core.fail("HINT_NATIVE_EVIDENCE_UNREADABLE", f"native serve proof의 {field} 가 regular file이 아니다: {clean}")
    return clean, lexical


def _native_cleanup_attestation(repo: Path, proof: dict) -> tuple[str, dict]:
    rel, path = _preserved_native_file(repo, proof.get("cleanup_attestation_path"), "cleanup_attestation_path")
    doc = _read_json_opt(path, "native cleanup attestation")
    if not isinstance(doc, dict):
        core.fail("HINT_NATIVE_CLEANUP_UNOBSERVED", f"native cleanup attestation이 JSON object가 아니다: {rel}")
    if doc.get("kind") != "native_multinode_cleanup_attestation" or doc.get("plane") != "native":
        core.fail("HINT_NATIVE_CLEANUP_INVALID", f"native cleanup attestation 정체가 맞지 않다: kind={doc.get('kind')!r}, plane={doc.get('plane')!r}")
    if doc.get("status") != "PASS" or doc.get("errors") != [] or doc.get("evidence_preserved") is not True:
        core.fail("HINT_NATIVE_CLEANUP_NONPASS", "native cleanup attestation이 PASS·errors=[]·evidence_preserved=true를 모두 만족하지 않는다.",
                  "cleanup 실패·unknown은 native hint 발행을 차단한다.")
    nodes = doc.get("nodes")
    if not isinstance(nodes, dict) or set(nodes) != {"main", "sub"}:
        core.fail("HINT_NATIVE_CLEANUP_INVALID", "native cleanup attestation의 nodes는 정확히 main·sub여야 한다.")
    for name in ("main", "sub"):
        node = nodes[name]
        if not isinstance(node, dict) or node.get("root_absent") is not True \
                or node.get("owned_processes") != [] or node.get("owned_gpu_processes") != [] \
                or node.get("unknown") != []:
            core.fail("HINT_NATIVE_CLEANUP_NONPASS", f"native cleanup attestation nodes.{name}가 완전한 cleanup PASS가 아니다.",
                      "root_absent=true, owned_processes=[], owned_gpu_processes=[], unknown=[]이 모두 필요하다.")
    if proof.get("run_id") != doc.get("run_id"):
        core.fail("HINT_NATIVE_CLEANUP_RUN_MISMATCH", "serve proof와 cleanup attestation의 run_id가 다르다.")
    return rel, doc


def _native_producer_evidence(repo: Path, topology: str, cell: str, proof: dict | None, proof_rel: str | None) -> dict:
    """Validate and extract the explicit native producer contract.

    This is intentionally gated only after the producer declared plane=native; no signal is
    never interpreted as native. Docker proof compatibility remains in the existing path.
    """
    if not isinstance(proof, dict) or proof.get("plane") != "native":
        return {}
    if topology != "multi" or proof.get("kind") != "native_multinode_serve_proof" or proof.get("status") != "PASS":
        core.fail("HINT_NATIVE_PROOF_INVALID", "native serve proof의 topology/kind/status가 producer 계약과 맞지 않는다.")
    if proof.get("cell", proof.get("config")) != cell:
        core.fail("HINT_NATIVE_PROOF_CELL_MISMATCH", f"native serve proof cell/config가 {cell!r}와 다르다.")
    health = proof.get("health") if isinstance(proof.get("health"), dict) else {}
    endpoints = proof.get("endpoints") if isinstance(proof.get("endpoints"), dict) else {}
    h_endpoint = endpoints.get("health") if isinstance(endpoints.get("health"), dict) else {}
    inf_endpoint = endpoints.get("inference") if isinstance(endpoints.get("inference"), dict) else {}
    inference = proof.get("inference") if isinstance(proof.get("inference"), dict) else {}
    if health.get("ok") is not True or str(h_endpoint.get("http_status")) != "200" \
            or inference.get("ok") is not True or str(inf_endpoint.get("http_status")) != "200" \
            or (_as_int(inference.get("completion_text_len")) or 0) < 1:
        core.fail("HINT_NATIVE_PROOF_UNOBSERVED", "native serve proof에 health 200 + inference 200 + completion_text_len>0 관측이 없다.")
    install_rel, _ = _preserved_native_file(repo, proof.get("native_install_path"), "native_install_path")
    freezes = proof.get("pip_freeze_paths")
    if not isinstance(freezes, dict) or set(freezes) != {"main", "sub"}:
        core.fail("HINT_NATIVE_EVIDENCE_MISSING", "native serve proof의 pip_freeze_paths는 정확히 main·sub 보존 경로여야 한다.")
    freeze_rel, _ = _preserved_native_file(repo, freezes.get("main"), "pip_freeze_paths.main")
    sub_freeze_rel, _ = _preserved_native_file(repo, freezes.get("sub"), "pip_freeze_paths.sub")
    cleanup_rel, cleanup = _native_cleanup_attestation(repo, proof)
    # 기동 기록은 **선택** 신호다(2026-09-29 · plan_26092908 §4.2): 있으면 보존 파일 계약(저장소 상대 regular)을 그대로 강제하고, 없으면
    #   결손(HINT_MISSING_NATIVE_LAUNCH · artifacts 가 기재)이다 — producer 가 쓰지 못한 사유(`native_launch_error`)는 결손 사유로 옮긴다.
    launch_raw = proof.get("native_launch_path")
    launch_rel = _preserved_native_file(repo, launch_raw, "native_launch_path")[0] if launch_raw not in (None, "") else None
    err = proof.get("native_launch_error")
    launch_err = str(err).strip()[-400:] if isinstance(err, str) and err.strip() and launch_rel is None else None
    return {"plane": "native", "native_install_path": install_rel, "pip_freeze_path": freeze_rel,
            "pip_freeze_paths": {"main": freeze_rel, "sub": sub_freeze_rel},
            "cleanup_attestation_path": cleanup_rel, "cleanup_attestation": cleanup,
            "native_launch_path": launch_rel, "native_launch_error": launch_err,
            "source": proof_rel or "serve_proof"}


def _measured_meta(ev_sweep: dict | None, cert: dict | None) -> dict:
    """이미지 태그·digest 의 **측정** 값과 출처. 스윕 meta 가 측정(docker inspect)이고 인증서는 그 옮김이다."""
    meta = ((ev_sweep or {}).get("index") or {}).get("meta") or {}
    out: dict = {}
    for key in ("image_tag", "image_digest"):
        if not _unobserved(meta.get(key)):
            out[key] = meta[key]
            out[f"{key}_source"] = f"sweep_index.meta.{key}({meta.get(key + '_source') or '출처 미표기'})"
        elif cert and not _unobserved(cert.get(key)):
            out[key] = cert[key]
            out[f"{key}_source"] = f"인증서 {key}"
    return out


def _report_born_utc(repo: Path, rel_path: str | None) -> str | None:
    """full 리포트 머리말 `생성일 <UTC>` = 스윕 generated_utc(render_report). 인증서가 없는 REFUTE 런의 조인 키."""
    if not rel_path or not (repo / rel_path).is_file():
        return None
    head = "\n".join((repo / rel_path).read_text(encoding="utf-8", errors="replace").splitlines()[:12])
    m = _REPORT_BORN.search(head)
    return m.group(1) if m else None


def _require_report_joins_certificate(repo: Path, report_rel: str | None, cert: dict | None) -> None:
    """바인딩할 리포트와 인증서가 **같은 측정**인가 — 리포트 `생성일` = 인증서 `measured_utc`(render_report 가 스윕
    generated_utc 를 생성일로 쓴다 · 같은 조인 키). 포인터가 다른 실행의 리포트를 가리키면 03 벤치 표와 인증서 수치가
    서로 다른 측정을 말한다 — 발행기가 고르지 않고 죽는다(2026-09-22 적대 검토: 옛 판본은 리포트 포인터가 하나면
    인증서 이름 쌍과 무관하게 그것을 묶었다). 어느 한쪽 키가 관측되지 않으면 막지 않는다(부재 ≠ 불일치)."""
    if not report_rel or not cert:
        return
    born, measured = _report_born_utc(repo, report_rel), _norm_value(cert.get("measured_utc"))
    if born and measured and born != measured:
        core.fail("HINT_BENCH_REPORT_CERT_MISMATCH", f"리포트 {report_rel} 생성일 {born} ≠ 인증서 measured_utc {measured} — 다른 측정이다.",
                  "같은 측정의 리포트(인증서 이름 쌍)를 가리키도록 캠페인 포인터를 정리한다 — 발행기가 둘을 섞지 않는다.")


def _report_twin(cert_rel: str | None) -> str | None:
    """인증서 이름 ↔ 리포트 이름은 명명 SSOT 의 쌍이다(docs.md §명명 — `benchmark_<X>.yaml` ↔ `bench_report_<X>.md`)."""
    if not cert_rel:
        return None
    p = Path(cert_rel)
    if not (p.name.startswith("benchmark_") and p.name.endswith(".yaml")):
        return None
    return (p.parent / ("bench_report_" + p.name[len("benchmark_"):-len(".yaml")] + ".md")).as_posix()


def _lite_snapshot(repo: Path, rel_path: str | None) -> dict:
    """리포트 `## 측정 환경 스냅샷` 2열 표 → {키: 값}(render_report 가 측정 환경에서 결정론으로 쓴 표)."""
    if not rel_path or not (repo / rel_path).is_file():
        return {}
    out: dict = {}
    on = False
    for ln in (repo / rel_path).read_text(encoding="utf-8", errors="replace").splitlines():
        s = ln.strip()
        if s.startswith("## "):
            on = s.startswith("## 측정 환경 스냅샷")
            continue
        if on and s.startswith("|"):
            cells = [c.strip() for c in s.strip("|").split("|")]
            if len(cells) == 2 and cells[0] not in ("키", "") and not set(cells[0]) <= {"-"}:
                out[cells[0]] = cells[1]
    return out


# ── 결손 · 측정 ──────────────────────────────────────────────────────────────────────────────────────────────
def _report_missing(repo: Path, cert: dict | None, report_rel: str | None) -> tuple[list[str], str | None]:
    """HINT_MISSING_BENCH_REPORT · SWEEP_LEVELS · LITE 를 옛 hint_collect(§4 5단)·hint_tag(B층) 술어 그대로 판정한다.
    `HINT_MISSING_LITE` 는 요구하는 자리와 적는 자리가 **같은 술어**다(render_bench_section.lite_metrics_present)."""
    missing: list[str] = []
    kind = None
    rbs = _rbs(repo)
    text = None
    if report_rel and (repo / report_rel).is_file():
        text = (repo / report_rel).read_text(encoding="utf-8", errors="replace")
    if text is None:
        missing.append("HINT_MISSING_BENCH_REPORT")
    elif rbs.is_lite_report(text):
        kind = "lite"
        missing.append("HINT_MISSING_SWEEP_LEVELS")   # 경량 리포트는 동시성 곡선을 재지 않는다 — 리포트 부재가 아니다
    else:
        kind = "full"
        try:
            if len(rbs.parse_report(text)["levels"]) <= 1:
                missing.append("HINT_MISSING_SWEEP_LEVELS")
        except rbs.BenchSectionFailure:
            missing.append("HINT_MISSING_BENCH_REPORT")
    if cert:
        # 옛 B층 인증서 술어(hint_tag._require_serving_evidence): lite_included 참 ∧ lite_gen_tps_warm 수치.
        if str(cert.get("lite_included", "")).strip().lower() != "true" or \
                not re.match(r"^[0-9]", str(cert.get("lite_gen_tps_warm", "")).strip()):
            missing.append("HINT_MISSING_LITE")
        if _unobserved(cert.get("measured_node")):
            missing.append("HINT_MISSING_MEASURED_NODE")
    else:
        missing.append("HINT_MISSING_CERTIFICATE")
        if text is not None and not rbs.lite_metrics_present(text):
            missing.append("HINT_MISSING_LITE")
    return missing, kind


def measurement_config(repo, ev: CellEvidence) -> dict:
    """측정 구성(bench_mode · 도구 · 반복 · 강등 사유) → PAYLOAD.measurement_config · 카탈로그 bench_mode 칸의 유일한 출처.
    옛 hint_collect.measurement_config_of 그대로(**파싱만** · 합성 ✗): 리포트 측정 구성 표 > 인증서(`benchmark_mode`·
    `bench_tool*`) > 경량 리포트 헤더 `mode: lite` > 발행 기록 `benchmark.mode=lite` > 부재. 헤더·기록은 **lite 쪽으로만**
    채운다(record 의 full 은 init 선언이라 full 정의 충족의 증거가 아니다). 배포 키 = DISTRIBUTED_MEASUREMENT_KEYS + source."""
    repo = _repo(repo)
    rbs = _rbs(repo)
    out: dict = {k: None for k in DISTRIBUTED_MEASUREMENT_KEYS}
    sources: list[str] = []
    rep = ev.bench_report_path
    name = Path(rep).name if rep else None
    text = (repo / rep).read_text(encoding="utf-8", errors="replace") if rep and (repo / rep).is_file() else None
    if text is not None:
        try:
            mc = rbs.parse_measurement_config(text)
        except rbs.BenchSectionFailure:
            mc = None
            # 사유(파서 예외)는 표 행 원문을 담을 수 있어 배포 평면에 옮기지 않는다.
            sources.append(f"unparseable(bench_report({name}) 측정 구성 표 — 파싱 실패)")
        if isinstance(mc, dict):
            for k in DISTRIBUTED_MEASUREMENT_KEYS:
                if mc.get(k) is not None:
                    out[k] = mc[k]
            sources.append(f"bench_report({name})")
    cert = ev.certificate or {}
    filled = False
    for k, ck in (("bench_mode", "benchmark_mode"), ("bench_tool", "bench_tool"), ("bench_tool_version", "bench_tool_version")):
        v = cert.get(ck)
        if out[k] is None and not _unobserved(v):
            out[k] = v
            filled = True
    if filled:
        sources.append(f"certificate({Path(ev.certificate_path or '').name})")
    if out["bench_mode"] is None and text is not None and rbs.is_lite_report(text):
        out["bench_mode"] = "lite"
        sources.append(f"bench_report({name}) 헤더 mode: lite")
    rec_mode = (((ev.publication or {}).get("record") or {}).get("benchmark") or {}).get("mode")
    if out["bench_mode"] is None and rec_mode == "lite":
        out["bench_mode"] = "lite"
        sources.append("publication record(benchmark.mode=lite)")
    out["source"] = " + ".join(sources) if sources else "absent(측정 구성 표·인증서 모두 없다 — 미기재)"
    return out


# 판정의 기계 표면(2026-09-29 · plan_26092908 §4.4 · V1). 판정 값의 어휘 — verdict_rule 의 둘 + lite-only 셀의 표지.
MEASUREMENT_VERDICTS = ("PASS", "REFUTE", "OBSERVATION-ONLY")
# 판정 원천 → PAYLOAD.measurement 키. verdict.json(verdict_rule 출력)의 경로(점 표기) · 리포트 `## 판정` 표의 항목 이름 ·
#   발행 기록 `benchmark` 의 키. 인증서 키 이름(_CERT_MEASUREMENT_KEYS)에 맞춘다 — 인증서가 있는 셀과 없는 셀이 같은 키를 말한다.
_JUDGED_KEYS = (
    # (측정 키,               verdict.json 경로,              리포트 판정 표 항목,           발행 기록 benchmark 키)
    ("verdict",               "verdict",                      "verdict",                   "verdict"),
    ("decode_tps_conc1",      "measured_decode_tps",          "측정 decode t/s (동시성1)",   None),
    ("rubric_authority",      "rubric.authority",             "루브릭 권한",                 "rubric_authority"),
    ("primary_tps",           "rubric.primary",               "루브릭 primary",              None),
    ("primary_source",        "rubric.source",                None,                        "primary_source"),
    ("floor_tps",             "rubric.floor",                 "floor (primary×(1−tol))",   "floor_tps"),
    ("tolerance",             "rubric.tolerance",             "tolerance",                 None),
    ("ratio_M_over_primary",  "rubric.ratio_M_over_primary",  "ratio (M/primary)",         "ratio_M_over_primary"),
    ("spec_on",               "measured_spec_on",             None,                        None),
    ("accept_len",            "measured_accept_len",          None,                        None),
)
# 인증서와 판정 원천이 **같은 측정**을 말하는지 대조하는 키(2026-09-29 · 계약 B-1 "인증서가 있으면 기존대로(값 일치 확인)").
_CROSSCHECK_KEYS = ("verdict", "decode_tps_conc1", "rubric_authority", "floor_tps", "ratio_M_over_primary")
_ABS_PATH_RE = re.compile(r"(?:^|[\s(=:'\"])/(?:home|mnt|root|Users|srv|data)\b")


def _surface(v) -> str | None:
    """판정 원천 값 → 인증서와 같은 **원문 문자열** 표면(인증서 칸은 문자열이다 · 합성 ✗ · 반올림 ✗)."""
    if v is None or (isinstance(v, str) and _unobserved(v)):
        return None
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, (int, float)):
        return repr(v) if isinstance(v, float) else str(v)
    return str(v).strip() or None


def _dig(doc, dotted: str):
    cur = doc
    for part in dotted.split("."):
        if not isinstance(cur, dict):
            return None
        cur = cur.get(part)
    return cur


def _report_verdict_table(repo: Path, rel_path: str | None) -> dict:
    """full 리포트 `## 판정` 2열 표 → {항목: 값}(render_report 가 verdict_rule 결과를 **표시만** 한 표 · 굵게·단위 표지는 벗긴다).
    `루브릭 primary` 칸은 `29.8 t/s (expected_achievable(roofline×MBU))` 모양 — 수치만 값으로, 괄호는 primary_source 로 쪼갠다."""
    if not rel_path or not (repo / rel_path).is_file():
        return {}
    out: dict = {}
    on = False
    for ln in (repo / rel_path).read_text(encoding="utf-8", errors="replace").splitlines():
        s = ln.strip()
        if s.startswith("## "):
            on = s.startswith("## 판정")
            continue
        if on and s.startswith("|"):
            cells = [c.strip() for c in s.strip("|").split("|")]
            if len(cells) == 2 and cells[0] not in ("항목", "") and not set(cells[0]) <= {"-"}:
                v = cells[1].replace("**", "").strip()
                if cells[0] == "루브릭 primary":
                    m = re.match(r"^([0-9.]+)\s*t/s\s*\((.+)\)\s*$", v)
                    if m:
                        out["루브릭 primary"], out["_primary_source"] = m.group(1), m.group(2)
                        continue
                out[cells[0]] = re.sub(r"\s*t/s$", "", v)
    return out


def _lite_report_numbers(repo: Path, rel_path: str | None) -> dict:
    """경량 리포트 `## lite 지표` 표의 master 열 수치 → {gen_tps, cold_ttft_ms}(수치 칸만 · 단위 벗김 · N/A = 결측).
    lite-only 셀은 스윕 색인이 없어 `measurement.lite` 가 비었다(2026-09-29 D2 실측 — brief 수치 원천이 없었다)."""
    if not rel_path or not (repo / rel_path).is_file():
        return {}
    rows = {"gen tokens/sec (warm) [master]": ("gen_tps", "t/s"), "cold-start TTFT [master]": ("cold_ttft_ms", "ms")}
    out: dict = {}
    on = False
    for ln in (repo / rel_path).read_text(encoding="utf-8", errors="replace").splitlines():
        s = ln.strip()
        if s.startswith("## "):
            on = s.startswith("## lite 지표")
            continue
        if on and s.startswith("|"):
            cells = [c.strip() for c in s.strip("|").split("|")]
            if len(cells) >= 2 and cells[0] in rows:
                key, unit = rows[cells[0]]
                m = re.fullmatch(r"([0-9]+(?:\.[0-9]+)?)\s*" + re.escape(unit), cells[1])
                if m:
                    out[key] = float(m.group(1))
    return out


def _judged(repo: Path, ev: CellEvidence) -> tuple[dict, dict, list[str]]:
    """인증서 없이도 읽히는 **판정 원천**(우선순위 = 스윕 verdict.json > 리포트 `## 판정` 표 > 발행 기록 benchmark).
    반환 ({키: 원문 문자열}, {키: 출처}, verdict_reasons). 스윕은 **이 측정에 묶였을 때만**(ev.sweep = generated_utc 조인) 읽는다 —
    같은 셀 id 의 옛 스윕 verdict 를 이 측정의 판정으로 싣지 않는다(G6 · lite 셀은 스윕을 묶지 않는다)."""
    vals: dict = {}
    srcs: dict = {}
    reasons: list[str] = []
    sw = ev.sweep or {}
    vj = sw.get("verdict") if isinstance(sw.get("verdict"), dict) else None
    if vj is not None:
        where = f"{sw.get('dir')}/verdict.json({sw.get('binding')} · verdict_rule 출력)"
        for key, path, _t, _r in _JUDGED_KEYS:
            v = _surface(_dig(vj, path))
            if v is not None and key not in vals:
                vals[key], srcs[key] = v, f"{where} {path}"
        reasons = [str(r) for r in vj.get("reasons") or [] if isinstance(r, str) and not _ABS_PATH_RE.search(r)]
    table = _report_verdict_table(repo, ev.bench_report_path)
    if table:
        where = f"bench_report({Path(ev.bench_report_path or '').name}) `## 판정` 표"
        for key, _p, item, _r in _JUDGED_KEYS:
            v = _surface(table.get(item)) if item else None
            if v is not None and key not in vals:
                vals[key], srcs[key] = v, f"{where} {item}"
        if "primary_source" not in vals and table.get("_primary_source"):
            vals["primary_source"], srcs["primary_source"] = table["_primary_source"], f"{where} 루브릭 primary(괄호)"
    bm = (((ev.publication or {}).get("record") or {}).get("benchmark") or {})
    if isinstance(bm, dict) and bm:
        where = f"발행 기록 benchmark({(ev.publication or {}).get('record_path')} · rubric_source={bm.get('rubric_source')})"
        for key, _p, _t, rk in _JUDGED_KEYS:
            v = _surface(bm.get(rk)) if rk else None
            if v is not None and key not in vals:
                vals[key], srcs[key] = v, f"{where} {rk}"
    if "verdict" in vals:
        vals["verdict"] = vals["verdict"].upper()
    return vals, srcs, reasons


def _certificate_not_due(repo: Path, ev: CellEvidence) -> str | None:
    """인증서가 **구조적으로 나오지 않는** 판정인가 → 사유(결손 아님) | None(발행되는 판정이거나 모름 — 결손으로 적는다).
    술어는 게이트 한 벌(completion_gate `CERTIFICATE_NON_ISSUING_AUTHORITIES` · 2026-09-29 plan_26092908 §4.8): lite-only ·
    REFUTE(어느 권한이든 인증서는 PASS 전용) · PASS ∧ 비발행 권한. 권한은 **판정 원천에서 파생된** 것만 믿는다 — verdict.json ·
    리포트 `## 판정` 표(verdict_rule 결과의 표시) · 발행 기록 benchmark(rubric_source=verdict_json 일 때만)."""
    if ev.report_kind == "lite":
        return "lite-only 셀 — 인증서는 full 측정에서만 나온다(판정 OBSERVATION-ONLY · 결손 아님)"
    jv, js, _r = _judged(repo, ev)
    verdict, authority = jv.get("verdict"), jv.get("rubric_authority")
    asrc = js.get("rubric_authority") or ""
    rec_bm = (((ev.publication or {}).get("record") or {}).get("benchmark") or {})
    if asrc.startswith("발행 기록") and rec_bm.get("rubric_source") != "verdict_json":
        authority = None                  # 출처 표시 없는 권한으로 면제를 열지 않는다(게이트 certificate_requirement_authority 와 같은 결)
    if verdict == "REFUTE":
        return f"REFUTE — 인증서는 PASS 전용이다(판정 {js.get('verdict')} · 결손 아님 · 판정 근거 = bench_report)"
    if verdict == "PASS" and authority in cgate(repo).CERTIFICATE_NON_ISSUING_AUTHORITIES:
        return (f"PASS ∧ 비발행 권한 {authority}({asrc}) — judge_bench 는 explicit ∧ PASS 만 인증서를 낸다(결손 아님 · "
                "판정 근거 = bench_report)")
    return None


def _num_equal(a: str, b: str) -> bool:
    """두 원문 문자열이 같은 측정값인가 — 수치는 **짧은 쪽 표기의 자릿수** 안에서 같으면 같다(인증서 `0.926` ↔ verdict `0.926` ·
    리포트 `20.98` ↔ verdict `20.98`). 수치가 아니면 글자(대소문자 무시)."""
    try:
        fa, fb = float(a), float(b)
    except (TypeError, ValueError):
        return str(a).strip().lower() == str(b).strip().lower()

    def decimals(s: str) -> int:
        s = str(s).strip()
        return len(s.split(".", 1)[1]) if "." in s and "e" not in s.lower() else 0
    tol = 0.5 * 10 ** -min(decimals(a), decimals(b)) + 1e-9
    return abs(fa - fb) <= tol


def measurement(ev: CellEvidence) -> dict:
    """PAYLOAD.measurement — 측정 기록(인증서 성능 칸의 **원문 문자열** + 묶인 스윕의 레벨 표 수치 · lite 수치). 파싱만 한다
    (합성 ✗ · 재계산 ✗). 인증서 칸은 선택 목록(_CERT_MEASUREMENT_KEYS)이라 판정·루브릭 표지(verdict·primary_source 등)도
    원문 그대로 실린다. 절대경로를 담는 스윕 칸(`raw_json`·`bench_json`)은 옮기지 않는다(배포 평면).

    ★ 2026-09-29(plan_26092908 §4.4 · V1): 옛 판본은 판정 칸을 **인증서에서만** 옮겼다 — 인증서는 explicit PASS 에만 나오므로 REFUTE
      (D1·N1)의 measurement 는 5키(판정·수치 없음)였다. 이제 인증서 유무와 무관하게 판정 원천(`_judged`: 스윕 verdict.json > 리포트
      `## 판정` 표 > 발행 기록 benchmark)에서 `verdict ∈ MEASUREMENT_VERDICTS` · `decode_tps_conc1` · `floor_tps` · `ratio_M_over_primary`
      · `rubric_authority` · `benchmark_mode` · `accept_len` 을 채우고, 키별 출처를 `sources{키: 출처}` 에 둔다. 판정이 한 번도 내려지지
      않은 lite-only 셀은 `OBSERVATION-ONLY`(판정 원천이 있으면 — 반복 불성립으로 강등된 full — 그 판정을 그대로 싣는다).
      인증서가 있으면 인증서 칸이 이기고, 같은 측정의 판정 원천과 _CROSSCHECK_KEYS 가 다르면 **죽는다**(HINT_MEASUREMENT_VERDICT_MISMATCH ·
      두 기록이 다른 측정을 말한다 — 고르지 않는다)."""
    repo = _repo(ev.repo or ".")
    out: dict = {}
    srcs: list[str] = []
    per: dict[str, str] = {}
    cert = ev.certificate or {}
    cname = f"certificate({Path(ev.certificate_path or '').name})"
    for k in _CERT_MEASUREMENT_KEYS:
        if k in cert:
            out[k] = cert[k]
            per[k] = f"{cname} {k}"
    if cert:
        srcs.append(cname)
    jv, js, reasons = _judged(repo, ev)
    if cert:
        bad = [f"{k}: 인증서={cert.get(k)!r} {js[k]}={jv[k]!r}" for k in _CROSSCHECK_KEYS
               if k in jv and not _unobserved(cert.get(k)) and not _num_equal(str(cert.get(k)), jv[k])]
        if bad:
            core.fail("HINT_MEASUREMENT_VERDICT_MISMATCH", f"인증서와 같은 측정의 판정 원천이 다른 값을 말한다: {bad}",
                      "인증서가 이 측정(스윕 generated_utc = measured_utc)의 판정에서 나왔는지 확인한다 — 발행기가 둘 중 하나를 고르지 않는다.")
    for k, v in jv.items():
        if k not in out or _unobserved(out.get(k)):
            out[k], per[k] = v, js[k]
    if jv and not cert:
        kinds = sorted({"verdict.json" if "verdict.json" in v else "bench_report 판정 표" if "`## 판정` 표" in v
                        else "발행 기록 benchmark" for v in js.values()})
        srcs.append(f"판정 원천({' · '.join(kinds)})")
    if reasons and out.get("verdict") == "REFUTE":
        out["verdict_reasons"], per["verdict_reasons"] = reasons, js.get("verdict", "verdict.json reasons")
    mc = measurement_config(repo, ev)
    if _unobserved(out.get("benchmark_mode")) and mc.get("bench_mode"):
        out["benchmark_mode"], per["benchmark_mode"] = mc["bench_mode"], f"measurement_config.bench_mode({mc.get('source')})"
    if _unobserved(out.get("verdict")):
        if out.get("benchmark_mode") == "lite" or ev.report_kind == "lite":
            out["verdict"] = "OBSERVATION-ONLY"
            per["verdict"] = ("lite-only 셀 — verdict_rule 에 투입되지 않았다(경량 리포트 `inform-only` · 판정 원천 없음 · "
                              f"benchmark_mode={out.get('benchmark_mode')} · report_kind={ev.report_kind})")
        else:
            out["verdict"] = None
            per["verdict"] = "미관측(인증서·스윕 verdict.json·리포트 `## 판정` 표·발행 기록 benchmark 모두 없다 — 지어내지 않는다)"
    elif out["verdict"] not in MEASUREMENT_VERDICTS:
        core.fail("HINT_MEASUREMENT_VERDICT_UNKNOWN", f"판정 원천의 verdict {out['verdict']!r} 가 {MEASUREMENT_VERDICTS} 밖이다({per.get('verdict')}).",
                  "verdict_rule 어휘가 바뀌었으면 evidence.MEASUREMENT_VERDICTS 를 개정한다(tripwire — 모르는 판정을 통과로 접지 않는다).")
    if "accept_len" not in out and out["verdict"] == "OBSERVATION-ONLY":
        # FACT_FIX2 G9: lite-only 셀도 수용 길이의 **측정 기록**은 있다 — lite 레그 JSON 의 spec_decode_acceptance_length(warm 우선 · 원문 값).
        #   판정 입력(verdict_rule 의 accept_len)이 아니라는 사실을 출처에 적는다.
        lw = lite_window(repo, ev)
        legs = (lw or {}).get("legs") or {}
        got = {leg: legs[leg]["doc"].get("spec_decode_acceptance_length") for leg in ("warm", "cold") if leg in legs}
        got = {k: v for k, v in got.items() if isinstance(v, (int, float)) and not isinstance(v, bool)}
        if got:
            leg = "warm" if "warm" in got else "cold"
            out["accept_len"] = str(got[leg])
            per["accept_len"] = (f"{legs[leg]['path']} spec_decode_acceptance_length({leg} 레그 · 원문)"
                                 + (f" · cold {got['cold']}" if leg == "warm" and "cold" in got else "")
                                 + " — lite 관측(판정 입력 아님 · lite-only 셀은 verdict_rule 에 투입되지 않았다)")
    for k in ("decode_tps_conc1", "floor_tps", "ratio_M_over_primary", "rubric_authority", "accept_len"):
        if k not in out:
            out[k] = None
            per[k] = ("lite-only 셀 — 동시성 1 스윕·루브릭이 없다(lite 수치는 `lite`)" if out["verdict"] == "OBSERVATION-ONLY"
                      else "미관측(판정 원천에 이 칸이 없다)")
    sw = ev.sweep or {}
    levels = []
    for lv in sw.get("levels") or []:
        if not isinstance(lv, dict):
            continue
        m = lv.get("measured") if isinstance(lv.get("measured"), dict) else {}
        levels.append({"level": lv.get("level"), "status": lv.get("status"),
                       **{k: m.get(k) for k in ("decode_tps", "decode_tps_mean", "output_throughput", "ttft_ms_median",
                                                "itl_ms_median", "tpot_ms_median", "accept_len", "completed", "failed",
                                                "max_concurrency", "measurement_ok") if k in m}})
    if levels:
        out["levels"] = levels
        out["verdict_point_level"] = (sw.get("index") or {}).get("verdict_point_level")
        per["levels"] = per["verdict_point_level"] = f"sweep_index({sw.get('dir')} · {sw.get('binding')})"
    lite = sw.get("lite") or {}
    lite_nums = {k: lite.get(k) for k in ("gen_tps", "gen_src", "cold_ttft_ms", "kv_gib") if k in lite}
    if isinstance(lite.get("engine"), dict):
        lite_nums.update({f"engine_{k}": v for k, v in lite["engine"].items() if isinstance(v, (int, float))})
    if lite_nums:
        out["lite"], per["lite"] = lite_nums, f"sweep_index.lite({sw.get('dir')})"
    elif ev.report_kind == "lite":
        ln = _lite_report_numbers(repo, ev.bench_report_path)
        if ln:
            out["lite"] = ln
            per["lite"] = f"bench_report({Path(ev.bench_report_path or '').name}) `## lite 지표` 표 master 열(단위 벗김 · 파싱만)"
            srcs.append(f"bench_report({Path(ev.bench_report_path or '').name} · lite)")
    if sw:
        out["generated_utc"] = sw.get("generated_utc")
        srcs.append(f"sweep_index({sw.get('dir')} · {sw.get('binding')})")
    # 측정 시각(2026-09-29 · §4.5 "미관측 오판"): 인증서가 없는 셀도 측정 시각은 관측돼 있다 — 스윕 generated_utc(= render_report 생성일)
    #   또는 경량 리포트 생성일(= lite_raw measured_utc). 옛 판본은 인증서의 measured_utc 만 옮겨 REFUTE·lite 셀에서 비었다.
    if _unobserved(out.get("measured_utc")):
        mu = sw.get("generated_utc") or _report_born_utc(repo, ev.bench_report_path)
        if mu:
            out["measured_utc"] = mu
            per["measured_utc"] = (f"sweep_index.generated_utc({sw.get('dir')})" if sw.get("generated_utc")
                                   else f"bench_report({Path(ev.bench_report_path or '').name}) 머리 `생성일`")
    out["source"] = " + ".join(srcs) if srcs else "absent(인증서·스윕 모두 없다)"
    out["sources"] = per
    return out


# ── 원장 타임라인 · 기동 표지 · full 정의 · 벤치 명령 재구성 ─────────────────────────────────────────────────────────
# 2026-09-22(plan_26092119 S2 round 2 · F6·F9): 1차 재생 저작자는 이 셀의 두 기동을 "미기록" 이라 적었다(채점 #28 오기) — 원장
#   `docs/logs/main/events/2026-09.jsonl` 에 같은 라벨 `smoke-nv4-bf-262k-mmp` 의 budget_declare 2건(07:43:46Z → renew 없이 08:14:38Z
#   clear · 18:54:37Z → 19:24:49Z renew)이 있었는데 사실 블록이 싣지 않았다. 기동 소요(~29.5분)도 셀 엔진 로그에 있었는데 "미관측"
#   으로 적혔다(#29). **사실은 기계가 싣고 해석은 저작자가 한다** — 이 절은 원장·엔진 로그의 행을 출처(파일:줄)와 함께 옮길 뿐
#   판정하지 않는다(예: renew 0회를 "벤치 미진입" 이라 단정하지 않는다 — renew 는 run_bench 진입과 budget_renew_loop 둘 다 쓴다).
REL_EVENTS_ROOT = "docs/logs"
REL_DOCS_RULES = ".claude/rules/docs.md"
# 원장 라벨 = 러너가 붙인 `<접두>-<셀>`(multinode_serve_smoke `smoke-${CONFIG}` · single_serve_up `serve-$CONFIG`). 닫힌 목록 · 정확 일치 —
#   접두 일치로 넓히면 `smoke-nv4-bf-262k-res-kv8g` 가 셀 `nv4-bf-262k-res` 의 기동으로 섞인다(2026-09-12 원장 실측).
_EVENT_LABEL_PREFIXES = ("smoke", "serve")
_EVENT_OPEN_KINDS = ("budget_declare", "budget_honored")          # 선언 슬롯을 여는 행(원장 스트림 · 워치독 스트림 각각)
_EVENT_CLOSE_KINDS = ("budget_clear", "budget_none", "budget_expired")
# 라벨 없는 행 중 열린 선언 창에 귀속하는 종류(닫힌 목록 · tripwire). 선언 슬롯은 노드당 하나다(blackbox_session `serve_budget.env`
#   단일 파일 · cmd_clear_budget 가 그 파일을 지운다) — 라벨 없는 clear·사살은 그때 열린 창의 것이다. `*_nomatch`(대상 없음 · 행동 0)는
#   싣지 않는다.
_EVENT_WINDOWED_KINDS = ("budget_clear", "budget_none", "budget_expired", "watchdog_trip", "watchdog_kill_ack",
                         "watchdog_highrate_hold", "thermal_trip", "thermal_kill_ack", "earlyoom_kill")
# detail 에 옮기는 필드(닫힌 목록 · 양성 목록). 원장 행에는 운영자 절대경로(`decl_path`)·boot_id 가 섞인다 — 배포 평면(PAYLOAD·00~03)에
#   가는 것은 이 목록뿐이다(부정 목록이면 producer 가 새 경로 칸을 더하는 순간 새어 나간다).
_EVENT_DETAIL_FIELDS = ("floor_mib", "weights_mib", "kv_mib", "overhead_mib", "mem_total_mib", "ttl_s", "remaining_before_s",
                        "existed", "arm_ceiling_mib", "remaining_s", "mem_avail_mib", "rate_mib_s", "max_rate_mib_s",
                        "legacy_rule_would_trip", "rule", "threshold_mib", "action", "target", "reason", "stage", "short_by_mib",
                        "model", "tensor_parallel_size", "topology", "duration_s", "verdict", "soc_temp_c", "gpu_pwr_w")
_SAFE_DETAIL_VALUE = re.compile(r"[A-Za-z0-9 ._:+=-]{1,80}")    # 값도 좁힌다(fullmatch) — 경로·주소 모양(`/`·`@`)은 싣지 않는다
# 2026-09-22(S2 round 2 리뷰): 위 문자 집합은 점 4마디 IPv4(주소:포트 모양)를 통과시킨다 — 배포 PII 게이트(private-ipv4)가 발행을
#   막을 뿐 새지는 않지만, 한 칸 값 때문에 발행 전체가 막히지 않게 여기서 뺀다(원장 detail 은 주소를 실을 이유가 없다). 이 주석과
#   픽스처는 추적 배포물이라 주소 리터럴을 적지 않는다(픽스처는 실행 시 조립 · 2026-08-06 자기스캔 사고).
_IPV4_LIKE = re.compile(r"(?<![0-9])\d{1,3}(?:\.\d{1,3}){3}(?![0-9])")
# 엔진 로그 표지(vLLM 로그 문구 · 닫힌 목록 · tripwire). 문구가 바뀌면 표지가 사라진다 — 사라진 것은 "일어나지 않았다" 가 아니라
#   "이 판본 문구로 읽지 못했다" 다(그래서 판정에 쓰지 않고 타임라인 행으로만 싣는다).
_ENGINE_TS = re.compile(r"\b(?:INFO|WARNING|ERROR|DEBUG|CRITICAL)\s+(\d{2})-(\d{2})\s+(\d{2}):(\d{2}):(\d{2})\b")
_ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
_ENGINE_MARKERS = (
    ("engine_prefetch_disabled", re.compile(r"Auto-prefetch is disabled because the filesystem \(([A-Za-z0-9_]+)\)"), "first"),
    ("engine_weights_loaded", re.compile(r"Loading weights took ([0-9.]+) seconds"), "last"),
    ("engine_model_loaded", re.compile(r"Model loading took ([0-9.]+) GiB memory and ([0-9.]+) seconds"), "last"),
    ("engine_shm_broadcast_wait", re.compile(r"No available shared memory broadcast block found in (\d+) seconds"), "first"),
    ("engine_kv_cache_sized", re.compile(r"GPU KV cache size: ([0-9,]+) tokens(?:, Maximum concurrency for ([0-9,]+) tokens "
                                         r"per request: ([0-9.]+)x)?"), "first"),
    ("engine_init_done", re.compile(r"init engine \(profile, create kv cache, warmup model\) took ([0-9.]+) s"), "first"),
    ("server_starting", re.compile(r"Starting vLLM server on "), "first"),
)
# full 정의 문장(docs.md compact document matrix `benchmark/` 행)의 모양 — tripwire. 문구가 바뀌면 정의를 **읽지 못함**(None)으로
#   떨어지고 meets_current_full 도 None 이다(모름을 충족/미충족으로 접지 않는다).
_FULL_DEF_RE = re.compile(r"full 계측\(`[^`]*`\)")
_FULL_REPEATS_RE = re.compile(r"반복\s*≥\s*(\d+)")


def _utc_dt(s) -> _dt.datetime | None:
    try:
        return _dt.datetime.strptime(str(s), "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=_dt.timezone.utc)
    except ValueError:
        return None


def _dur(a, b) -> str | None:
    """두 UTC 문자열 사이(b − a)를 `12m34s` · `1h02m03s` 로. 어느 쪽이든 읽지 못하면 None."""
    da, db = _utc_dt(a), _utc_dt(b)
    if da is None or db is None:
        return None
    s = int((db - da).total_seconds())
    sign, s = ("-" if s < 0 else ""), abs(s)
    h, r = divmod(s, 3600)
    m, sec = divmod(r, 60)
    return f"{sign}{h}h{m:02d}m{sec:02d}s" if h else f"{sign}{m}m{sec:02d}s"


def _label_is_cell(label, cell: str) -> bool:
    return isinstance(label, str) and (label == cell or any(label == f"{p}-{cell}" for p in _EVENT_LABEL_PREFIXES))


def _session_is_cell(sid, cell: str) -> bool:
    """serve_start/stop 의 session_id = `<접두>-<셀>` + 선택 `-YYYYMMDDTHHMMSSZ`(blackbox_session 세션 id 관행)."""
    pat = r"(?:%s)-%s(?:-\d{8}T\d{6}Z)?" % ("|".join(_EVENT_LABEL_PREFIXES), re.escape(cell))
    return isinstance(sid, str) and re.fullmatch(pat, sid) is not None


def _event_fields(d: dict) -> str:
    out = []
    for k in _EVENT_DETAIL_FIELDS:
        v = d.get(k)
        if isinstance(v, bool) or isinstance(v, (int, float)):
            out.append(f"{k}={str(v).lower() if isinstance(v, bool) else v}")
        elif isinstance(v, str) and _SAFE_DETAIL_VALUE.fullmatch(v) and not _IPV4_LIKE.search(v):
            out.append(f"{k}={v}")
    return " · ".join(out)


def _measured_key(repo: Path, ev: CellEvidence) -> str | None:
    return (_norm_value((ev.certificate or {}).get("measured_utc")) or (ev.sweep or {}).get("generated_utc")
            or _report_born_utc(repo, ev.bench_report_path))


def _event_node_dirs(repo: Path, ev: CellEvidence) -> list[str]:
    """이 셀 노드 축의 블랙박스 노드 디렉터리(`docs/logs/<node_id>/events`). cluster = 선언 노드 전부(선언 없으면 있는 것 전부) ·
    main/sub = 그 역할의 선언 노드(선언 없으면 축 이름 = node_id — 이 저장소의 node_identity 관행)."""
    base = repo / REL_EVENTS_ROOT
    if not base.is_dir():
        return []
    have = sorted(d.name for d in base.iterdir() if d.is_dir() and (d / "events").is_dir())
    nodes = [n for n in ((ev.declaration or {}).get("nodes") or []) if isinstance(n, dict) and n.get("node_id")]
    if ev.node == "cluster":
        want = [str(n["node_id"]) for n in nodes] or have
    else:
        want = [str(n["node_id"]) for n in nodes if n.get("role") == ev.node] or [ev.node]
    return [w for w in dict.fromkeys(want) if w in have]


def _event_rows(path: Path) -> tuple[list[tuple[str, int, dict]], int]:
    """jsonl → [(ts, 줄 번호(1-기반 · `\\n` 기준), 행)] (ts 순 · 같은 ts 는 줄 순) + JSON 파싱 불가 줄 수(부재가 아니라 판독 불가)."""
    try:
        # 바이트로 읽는다 — read_text 의 universal newline 이 `\r` 를 줄바꿈으로 바꿔 줄 번호가 grep·편집기와 갈라진다(1차 재생 결함).
        text = path.read_bytes().decode("utf-8", errors="replace")
    except OSError as e:
        core.fail("HINT_EVIDENCE_UNREADABLE", f"블랙박스 원장을 읽을 수 없다({path.name}): {type(e).__name__}: {e}")
    rows: list[tuple[str, int, dict]] = []
    bad = 0
    for i, line in enumerate(text.split("\n"), 1):
        s = line.strip()
        if not s:
            continue
        try:
            d = json.loads(s)
        except ValueError:
            bad += 1
            continue
        if isinstance(d, dict) and isinstance(d.get("ts"), str) and _UTC.fullmatch(d["ts"]):
            rows.append((d["ts"], i, d))
    rows.sort(key=lambda r: (r[0], r[1]))
    return rows, bad


def _event_windows(rows: list[tuple[str, int, dict]]) -> list[dict]:
    """스트림 하나의 선언 창 — 여는 행(선언·arm) ~ 닫는 행(clear·none·expired · 포함) 또는 다음 여는 행(교체 · 미포함).
    창 안의 행 = 같은 라벨 행 + 라벨 없는 _EVENT_WINDOWED_KINDS 행."""
    wins: list[dict] = []
    cur = None
    for idx, (ts, _ln, d) in enumerate(rows):
        k = d.get("kind")
        if k in _EVENT_OPEN_KINDS:
            if cur is not None:
                cur.update(end_ts=ts, end_kind=f"replaced({k})")
                wins.append(cur)
            cur = {"label": d.get("label"), "open_kind": k, "start_ts": ts, "rows": [idx], "end_ts": None, "end_kind": "open"}
            continue
        if cur is None:
            continue
        lab = d.get("label")
        if k in _EVENT_CLOSE_KINDS and (lab in (None, "") or lab == cur["label"]):
            cur["rows"].append(idx)
            cur.update(end_ts=ts, end_kind=k + ("(existed=false)" if d.get("existed") is False else ""))
            wins.append(cur)
            cur = None
        elif lab in (None, "") and k in _EVENT_WINDOWED_KINDS:
            cur["rows"].append(idx)
        elif lab == cur["label"]:
            cur["rows"].append(idx)
    if cur is not None:
        wins.append(cur)
    return wins


def _ledger_timeline(repo: Path, ev: CellEvidence, ctx: dict | None = None) -> tuple[list[dict], list[str], dict | None]:
    """(행들, 행을 낸 원장 파일들, 측정 창 {start_ts, end_ts, node}|None). 측정 창 = measured_utc 를 품은 이 셀의 budget_declare 창.
    ctx(선택 · 2026-09-22 S2 round 3)를 주면 **라벨 무관** 선언 창(`windows`)과 예산 행(`budget_rows`)을 채운다 — 과거 측정 창이
    "예산 선언 없이 잰 측정" 인지 가르는 입력이다(이 셀 라벨 창만 보면 다른 라벨 창 안의 측정도 '선언 없음' 으로 오기한다)."""
    cell = ev.cell
    measured = _measured_key(repo, ev)
    out: list[dict] = []
    used: set[str] = set()
    measured_win = None
    for node in _event_node_dirs(repo, ev):
        streams = []
        for f in sorted((repo / REL_EVENTS_ROOT / node / "events").glob("*.jsonl")):
            rows, bad = _event_rows(f)
            streams.append((_rel(repo, f), rows, bad))
        if ctx is not None:
            ctx.setdefault("nodes", []).append(node)
            for rel, rows, _bad in streams:
                if rows:
                    # 노드 원장의 관측 범위(스트림들의 첫 행 ~ 마지막 행 · 2026-09-22 S2 round 3 적대 검토): 원장이 끊긴 뒤의 시각은
                    #   "선언 없음" 이 아니라 **미관측** 이다(실측: sub 2026-09.jsonl 은 09-11T11:10:38Z 에서 끝난다).
                    sp = ctx.setdefault("spans", {}).setdefault(node, [rows[0][0], rows[-1][0]])
                    sp[0], sp[1] = min(sp[0], rows[0][0]), max(sp[1], rows[-1][0])
                for w in _event_windows(rows):
                    # stream_last: 열린 창(end_ts None)은 그 스트림의 마지막 행까지만 열려 있음이 관측된다 — 그 뒤는 닫힘 미관측이다
                    #   (실측: sub 스트림이 `smoke-nv4-f8-1m-res` 창을 연 채 끝난다 — 옛 판본은 그 뒤 모든 시각을 이 창이 덮는다고 읽었다).
                    ctx.setdefault("windows", []).append({"node": node, "rel": rel, "label": w["label"], "start_ts": w["start_ts"],
                                                          "end_ts": w["end_ts"], "line": rows[w["rows"][0]][1],
                                                          "stream_last": rows[-1][0]})
                for ts, ln, d in rows:
                    if d.get("kind") in _EVENT_OPEN_KINDS + _EVENT_CLOSE_KINDS + ("budget_renew",):
                        ctx.setdefault("budget_rows", []).append({"node": node, "utc": ts, "kind": d.get("kind"),
                                                                  "label": d.get("label") or None, "rel": rel, "line": ln})
        taken: dict[tuple[str, int], dict | None] = {}
        declare_wins: list[tuple[str, dict]] = []
        for rel, rows, _bad in streams:
            in_win: set[int] = set()
            for w in _event_windows(rows):
                if not _label_is_cell(w["label"], cell):
                    continue
                for idx in w["rows"]:
                    in_win.add(idx)
                    taken[(rel, idx)] = w
                if w["open_kind"] == "budget_declare":
                    declare_wins.append((rel, w))
            for idx, (_ts, _ln, d) in enumerate(rows):
                if idx in in_win:
                    continue
                if _label_is_cell(d.get("label"), cell) or (
                        d.get("kind") in ("serve_start", "serve_stop") and _session_is_cell(d.get("session_id"), cell)):
                    taken[(rel, idx)] = None
        # 창을 열지 않는 스트림(thermal 등)의 라벨 없는 행 — 같은 노드의 선언 창 **시간 구간**에 귀속한다.
        for rel, rows, _bad in streams:
            if any(d.get("kind") in _EVENT_OPEN_KINDS for _, _, d in rows):
                continue
            for idx, (ts, _ln, d) in enumerate(rows):
                if d.get("label") or d.get("kind") not in _EVENT_WINDOWED_KINDS:
                    continue
                w = next((w for _, w in declare_wins if w["start_ts"] <= ts and (w["end_ts"] is None or ts <= w["end_ts"])), None)
                if w is not None:
                    taken[(rel, idx)] = w
        declare_wins.sort(key=lambda x: (x[1]["start_ts"], x[0]))
        order = {id(w): i for i, (_, w) in enumerate(declare_wins, 1)}
        for _rel_w, w in declare_wins:
            if measured and measured_win is None and w["start_ts"] <= measured and (w["end_ts"] is None or measured <= w["end_ts"]):
                measured_win = {"start_ts": w["start_ts"], "end_ts": w["end_ts"], "node": node}
        node_start = len(out)
        for rel, rows, bad in streams:
            mine = sorted(idx for (r, idx) in taken if r == rel)
            if not mine:
                continue
            used.add(rel)
            for idx in mine:
                ts, ln, d = rows[idx]
                w = taken[(rel, idx)]
                k = str(d.get("kind"))
                fields = _event_fields(d)
                # `label` 은 **그 행의 라벨**이다(2026-09-22 · S2 round 2 리뷰). 라벨 없는 clear·사살·보류를 창의 라벨로 채우면 추론(창 귀속)이
                #   원장 데이터처럼 보인다 — 1차 사실확인이 센 오기 부류다. 귀속은 detail 이 "라벨 없음 → … 귀속" 으로 말한다.
                label = d.get("label") or None
                if k == "budget_declare" and w is not None and id(w) in order:
                    renews = sum(1 for j in w["rows"] if rows[j][2].get("kind") == "budget_renew")
                    if measured is None:
                        rel_m = "측정 시각 미관측"
                    elif w["start_ts"] <= measured and (w["end_ts"] is None or measured <= w["end_ts"]):
                        rel_m = f"measured_utc {measured} 가 이 창 안(이 측정의 기동)"
                    else:
                        # 전·후를 적는다(2026-09-22 round 2 리뷰) — 같은 셀 id 가 뒤 캠페인에서 다시 뜬 창(예: 09-10 측정 셀의 09-17 재기동
                        #   사살)이 이 측정의 여정처럼 읽히지 않게.
                        rel_m = (f"measured_utc {measured} 는 이 창 밖(다른 기동 · "
                                 + ("측정 뒤 — 이 측정과 무관할 수 있다)" if w["start_ts"] > measured else "측정 전)"))
                    closed = (f"닫힘 {w['end_kind']} {w['end_ts']}(선언 후 {_dur(w['start_ts'], w['end_ts'])})" if w["end_ts"]
                              else "닫힘 미관측(창이 열린 채 원장이 끝난다)")
                    detail = (f"예산 선언(선언값 — 측정 아님) · {fields} · 기동 창 {order[id(w)]}/{len(declare_wins)}: "
                              f"budget_renew {renews}회 · {closed} · {rel_m}")
                elif d.get("label") in (None, "") and w is not None:
                    detail = (f"{k} · {fields} · 라벨 없음 → {w['label']} 선언 창 귀속(노드당 선언 슬롯 1개 · "
                              f"창 시작 {w['start_ts']} 후 {_dur(w['start_ts'], ts)})")
                elif w is not None:
                    detail = f"{k} · {fields} · 창 시작 {w['start_ts']} 후 {_dur(w['start_ts'], ts)}"
                else:
                    detail = f"{k} · {fields}" + (f" · session={d.get('session_id')}" if d.get("session_id") else "")
                out.append({"utc": ts, "node": node, "kind": k, "label": label, "detail": detail, "source": f"{rel}:{ln}"})
            if bad:
                out.append({"utc": None, "node": node, "kind": "ledger_unparsable_lines", "label": None,
                            "detail": f"JSON 파싱 불가 {bad}줄 — 건너뜀(부재가 아니라 판독 불가 · 그 줄의 사건은 이 표에 없다)",
                            "source": rel})
        out[node_start:] = _merge_stream_copies(out[node_start:])
    return out, sorted(used), measured_win


def _merge_stream_copies(rows: list[dict]) -> list[dict]:
    """한 노드의 행에서 **다른 스트림 파일**에 실린 같은 사건(같은 ts · 같은 kind)을 하나로 합친다(2026-09-22 · S2 round 2 리뷰).
    왜: 월 원장(`YYYY-MM.jsonl`)은 journald 경유로 thermal·watchdog 스트림의 행을 다시 받는다 — 2026-09 실측 thermal 106/107행 ·
    watchdog 6행이 필드 구성만 다른 사본이다. 둘 다 실으면 사살·보류가 두 번 일어난 것처럼 읽힌다. 남기는 것은 정렬 순서(파일 사전순)의
    첫 행이고, 사본의 출처(`파일:줄`)는 그 행 detail 에 적는다(조용한 탈락 ✗). **같은 파일** 안의 같은 (ts, kind) 는 합치지 않는다 —
    같은 초의 서로 다른 사건일 수 있다(한 producer 가 두 번 쓴 것은 사본이 아니다)."""
    first: dict[tuple[str, str], dict] = {}
    kept: list[dict] = []
    for r in sorted(rows, key=_timeline_key):
        if r.get("utc") is None:
            kept.append(r)
            continue
        key = (str(r["utc"]), str(r.get("kind")))
        f0 = first.get(key)
        if f0 is not None and str(f0["source"]).rpartition(":")[0] != str(r["source"]).rpartition(":")[0]:
            f0["detail"] = f"{f0['detail']} · 같은 사건의 다른 스트림 사본 {r['source']}(한 번으로 싣는다)"
            continue
        first.setdefault(key, r)
        kept.append(r)
    return kept


def _startup_markers(repo: Path, ev: CellEvidence, win: dict | None) -> list[dict]:
    """이 측정의 엔진 로그(스윕 lite 엔진 로그 · 없으면 판정점 레벨 엔진 로그)에서 기동 표지 행. 엔진 로그 시각은 연도·시간대가 없는
    컨테이너 시계다 — **측정 창(원장 선언~닫힘) 안에 UTC 로 들어올 때만** 싣는다(시계 정합의 관측 · 밖이면 싣지 않고 그 사실을 한 행으로
    적는다). 스윕이 출력 평면에 묶이지 않았으면 그 로그는 이 측정의 것이 아니다 — 싣지 않는다."""
    sw = ev.sweep or {}
    if sw.get("binding") != "output" or win is None:
        return []
    base = repo / sw["dir"]
    vp = _as_int((sw.get("index") or {}).get("verdict_point_level")) or 1
    log = next((p for p in (base / f"lite_engine_{ev.cell}.log", base / f"level_{vp:02d}" / f"engine_{ev.cell}.log")
                if p.is_file()), None)
    if log is None:
        return []
    rel = _rel(repo, log)
    try:
        # 바이트로 읽는다 — tqdm `\r` 진행줄을 universal newline 이 줄로 쪼개면 `파일:줄` 이 grep·편집기 줄 번호와 달라진다(2026-09-22
        #   S2 1차 저작자 보고 · excerpt 줄 번호 결함과 같은 뿌리).
        text = log.read_bytes().decode("utf-8", errors="replace")
    except OSError as e:
        core.fail("HINT_EVIDENCE_UNREADABLE", f"엔진 로그를 읽을 수 없다({log.name}): {type(e).__name__}: {e}")
    start = _utc_dt(win["start_ts"])
    end = win.get("end_ts") or _measured_key(repo, ev)

    def to_utc(mo: str, d: str, h: str, mi: str, s: str) -> str | None:
        year = start.year + (1 if (int(mo), int(d)) < (start.month, start.day) else 0)
        try:
            return _dt.datetime(year, int(mo), int(d), int(h), int(mi), int(s)).strftime("%Y-%m-%dT%H:%M:%SZ")
        except ValueError:
            return None

    first = None
    hits: dict[str, list[tuple[int, str, re.Match]]] = {k: [] for k, _, _ in _ENGINE_MARKERS}
    for i, line in enumerate(text.split("\n"), 1):
        for seg in line.split("\r"):              # tqdm 진행줄(\r) — 줄 번호는 `\n` 기준(grep·편집기와 같다)
            clean = _ANSI.sub("", seg)
            tm = _ENGINE_TS.search(clean)
            utc = to_utc(*tm.groups()) if tm else None
            if utc is None:
                continue
            if first is None:
                first = (i, utc)
            for kind, rx, _pick in _ENGINE_MARKERS:
                m = rx.search(clean)
                if m:
                    hits[kind].append((i, utc, m))
    if first is None:
        return []
    stamps = [first[1]] + [u for v in hits.values() for _, u, _ in v]
    if min(stamps) < win["start_ts"] or (end and max(stamps) > end):
        return [{"utc": None, "node": ev.node, "kind": "engine_log_clock_unverified", "label": None,
                 "detail": (f"엔진 로그 시각(컨테이너 시계 · UTC 가정)이 측정 창 [{win['start_ts']}, {end}] 밖({min(stamps)}~{max(stamps)}) — "
                            "시계 정합을 관측하지 못해 기동 표지를 싣지 않는다"), "source": rel}]
    ws = win["start_ts"]
    rows = [{"utc": first[1], "node": ev.node, "kind": "engine_log_window_start", "label": None,
             "detail": (f"캡처된 엔진 로그의 첫 시각(꼬리 캡처일 수 있다 — 엔진 시작 시각이 아닐 수 있다) · 선언 {ws} 후 {_dur(ws, first[1])} · "
                        f"시계: 컨테이너 시각을 UTC 로 읽어 측정 창 [{ws}, {end}] 안에 드는 것을 확인"),
             "source": f"{rel}:{first[0]}"}]
    for kind, _rx, pick in _ENGINE_MARKERS:
        v = hits[kind]
        if not v:
            continue
        i, utc, m = v[0] if pick == "first" else v[-1]
        n = len(v)
        if kind == "engine_prefetch_disabled":
            detail = f"가중치 auto-prefetch 꺼짐 줄 ×{n} — filesystem {m.group(1)}(vLLM 로그 문구)"
        elif kind == "engine_weights_loaded":
            detail = f"가중치 적재 완료 줄 ×{n}(rank·단계별) · 최장 {max(float(x[2].group(1)) for x in v):.2f}s · 이 행 = 마지막 줄"
        elif kind == "engine_model_loaded":
            detail = f"모델 적재 완료 줄 ×{n}(rank 별) · 마지막 줄 {m.group(1)} GiB · {float(m.group(2)):.1f}s"
        elif kind == "engine_shm_broadcast_wait":
            detail = (f"shm_broadcast 대기 경고({m.group(1)}s 단위) ×{n} · {v[0][1]}–{v[-1][1]}({_dur(v[0][1], v[-1][1])}) — vLLM 문구상 "
                      "프로세스 hang 또는 긴 작업(컴파일·가중치/KV 양자화) 중 출현")
        elif kind == "engine_kv_cache_sized":
            detail = f"GPU KV cache {m.group(1)} tokens" + (f" · 요청당 {m.group(2)} tokens 기준 최대 동시성 {m.group(3)}x"
                                                           if m.group(2) else "")
        elif kind == "engine_init_done":
            detail = f"엔진 초기화(profile·KV 할당·warmup) {m.group(1)}s"
        else:
            detail = (f"API 서버 기동 — 이 측정 창 선언 {ws} 후 {_dur(ws, utc)} · 캡처 로그 첫 시각 {first[1]} 후 "
                      f"{_dur(first[1], utc)}")
        rows.append({"utc": utc, "node": ev.node, "kind": kind, "label": None, "detail": detail, "source": f"{rel}:{i}"})
    return rows


def _timeline_key(r: dict):
    src = str(r.get("source") or "")
    f, _, ln = src.rpartition(":")
    return (r.get("utc") is None, r.get("utc") or "", str(r.get("node") or ""), f or src, _as_int(ln) or 0)


def event_timeline(repo, ev: CellEvidence) -> list[dict]:
    """이 셀의 사건 타임라인 `[{utc, node, kind, label, detail, source}]`(utc 순 · 판독 불가 행은 뒤). 원천 = 블랙박스 원장
    `docs/logs/<node_id>/events/*.jsonl` 의 이 셀 라벨 행(예산 선언·갱신·arm·거부 · 서빙 세션) + 그 선언 창에 귀속하는 라벨 없는 행
    (clear·none·사살·보류) + 측정 창 안의 엔진 로그 기동 표지. `source` = `파일:줄`(발췌·대조 가능). `label` = 그 행 자신의 라벨
    (라벨 없는 행은 None — 창 귀속은 detail 이 말한다) · 다른 스트림 파일에 실린 같은 사건(ts·kind)은 한 행으로 싣고 사본 출처를 detail 에
    적는다(2026-09-22 · round 2 리뷰). **읽기만 한다.**
    2026-09-22(S2 round 3): 같은 셀의 **다른 측정**(과거 발행 기록 measured_utc · simlog 사본 generated_utc · 덮인 출력 스윕)도
    `kind: measurement` 행으로 싣는다 — 예산 선언이 하나도 없던 측정은 원장 행이 0 이라 종전 표에서 사라졌다(1차 저작자가 09-10 측정의
    '선언 없음' 을 손으로 캐야 했다). 라벨: `measurement (no budget event recorded)` · `(budget window recorded)` ·
    `(other label's budget window)` · `(budget window closure unobserved)`(스트림이 창을 연 채 끝났다) · `(ledger unobserved)`(그 시각을
    관측한 원장 없음). 판정 증인 = 그 시각을 기록 범위(첫 행~마지막 행) 안에 둔 노드 원장뿐(적대 검토 2026-09-22).
    2026-09-29: 캠페인 경계(`event_campaign_boundary`)가 관측되면 시각 있는 행마다 `campaign_scope` ∈ this-campaign · prior-campaign
    (경계 이전 = 같은 셀 이름 · 앞선 캠페인 — 행은 싣고 detail 에 적는다 · 기동 시도로 세지 않는다)."""
    repo = _repo(repo)
    rows, _files = _timeline_parts(repo, ev)
    return rows


def event_ledger_spans(repo, ev: CellEvidence) -> list[dict]:
    """이 셀 노드 축 원장의 **관측 범위**(2026-09-29 · FACT_FIX2 G8) — [{node, first_utc, last_utc, files[], covers_measurement}].
    D2: 메인의 서브 원장 미러(`docs/logs/sub/events/2026-09.jsonl`)가 09-11 에서 끝나 09-23 기동의 서브 선언이 표에 없었는데, FACT 가 범위를
    밝히지 않아 "서브는 선언하지 않았다" 로 읽혔다. 범위 끝 뒤의 시각은 "없음" 이 아니라 **관측 범위 밖**이다. covers_measurement =
    마지막 행 ≥ 측정 끝(측정 끝 미관측이면 None). 선언 노드인데 원장 디렉터리가 없으면 files [] · first/last None.
    2026-09-29(F10): 측정 끝을 덮지 못한 노드는 같은 기동의 스모크 로그에 echo 된 그 노드 원장 JSON 줄(선언 · honored)을 `smoke_echo`
    {log, rows[{ts, kind, label, source}], basis} 로 싣는다(보조 관측 · 그 밖의 사건은 여전히 관측 범위 밖 · 규칙 = _smoke_ledger_echo)."""
    repo = _repo(repo)
    try:
        end = _measure_window(repo, ev)[1]
    except core.HintError:
        end = None
    decl = [str(n["node_id"]) for n in ((ev.declaration or {}).get("nodes") or []) if isinstance(n, dict) and n.get("node_id")]
    have = _event_node_dirs(repo, ev)
    want = list(dict.fromkeys((decl if ev.node == "cluster" else []) + have))
    out = []
    for node in want:
        files, first, last = [], None, None
        d = repo / REL_EVENTS_ROOT / node / "events"
        for f in sorted(d.glob("*.jsonl")) if d.is_dir() else []:
            rows, _bad = _event_rows(f)
            if not rows:
                continue
            files.append(_rel(repo, f))
            first = rows[0][0] if first is None else min(first, rows[0][0])
            last = rows[-1][0] if last is None else max(last, rows[-1][0])
        span = {"node": node, "first_utc": first, "last_utc": last, "files": files,
                "covers_measurement": (last >= end) if (last and end) else None, "measurement_end_utc": end}
        if span["covers_measurement"] is not True:
            echo = _smoke_ledger_echo(repo, ev, node)
            if echo is not None:
                span["smoke_echo"] = echo
        out.append(span)
    return out


# ── 원장 미러가 측정 창을 덮지 못한 노드의 **보조 관측**(2026-09-29 · factcheck F10) ─────────────────────────────────────────
# D: 메인의 서브 원장 미러가 09-11 에서 끝나 "서브 선언 · 사건은 관측 범위 밖" 이라 적었는데, 같은 기동의 스모크 로그(multinode_serve_smoke
#   가 서브 원장의 budget_honored JSON 줄을 그대로 echo)에 서브의 원장 행이 원문으로 남아 있었다. 규칙:
#   ① 스모크 로그 = attestation_same_boot 의 **강한 판별**이 지목한 로그만(같은 기동의 스모크 — 새 판별을 만들지 않는다 · 약한 판별 · 판별 ✗
#      이면 보조 관측 없음)
#   ② 줄 = `<node>: budget_declare|budget_honored … {JSON}` 이고 JSON 이 파싱되며 kind 가 그 줄의 kind 와 같고 ts 가 UTC 이고 label 이 이 셀
#      스모크 라벨(있으면)과 같을 때만 — 그 밖의 문장 줄(예: '선언 발행: …env')은 원장 행이 아니다(싣지 않는다)
#   ③ 그 밖의 사건(사살 · 트립 · clear · 갱신)은 스모크 로그가 echo 하지 않는다 — 여전히 관측 범위 밖(template 이 한정을 적는다).
SMOKE_ECHO_KINDS = ("budget_declare", "budget_honored")
SMOKE_ECHO_NOTE = "원장 미러 밖 — 스모크 로그 echo"


def _smoke_ledger_echo(repo: Path, ev: CellEvidence, node: str) -> dict | None:
    """{log, rows[{ts, kind, label, source}], basis} | None — 위 규칙(①②). 같은 기동 판별이 없거나 echo 행이 0 이면 None."""
    att = ev.attestation if isinstance(ev.attestation, dict) else {}
    if not att.get("_path") or att.get("config") != ev.cell:
        return None
    written = _attestation_written(repo, att)[0]
    timing, _note, win = _attestation_timing(repo, ev, written)
    if timing != "before-measurement":
        return None
    sb = attestation_same_boot(repo, ev, written, win)
    if not sb or sb.get("strength") != "strong":
        return None
    rel = sb["files"][0]
    lg = repo / rel
    if not lg.is_file():
        return None
    label = f"smoke-{ev.cell}"
    pat = re.compile(r"^\s*(?:\[[^\]]*\]\s*)?" + re.escape(node) + r":\s+(" + "|".join(SMOKE_ECHO_KINDS) + r")\b[^{]*(\{.*\})\s*$")
    rows = []
    for i, line in enumerate(_log_lines(lg), 1):
        m = pat.match(_ANSI.sub("", line))
        if not m:
            continue
        try:
            d = json.loads(m.group(2))
        except ValueError:
            continue
        ts = d.get("ts") if isinstance(d, dict) else None
        if not (isinstance(ts, str) and _UTC.fullmatch(ts)) or d.get("kind") != m.group(1):
            continue
        if d.get("label") not in (None, label):
            continue
        rows.append({"ts": ts, "kind": d["kind"], "label": d.get("label"), "source": f"{rel}:{i}"})
    if not rows:
        return None
    return {"log": rel, "rows": rows,
            "basis": f"같은 기동 판별(attestation_same_boot 강한 판별)이 지목한 스모크 로그 {rel} 의 `{node}:` 원장 JSON echo 줄"}


def event_files(repo, ev: CellEvidence) -> list[str]:
    """타임라인이 **출처로 인용한** 파일(저장소 상대) — LINEAGE evidence_candidates 입력(발췌 출처가 계보 밖이 되지 않게).
    원장(행을 낸 것 + 과거 측정 행이 직전·직후 예산 행으로 인용한 것) · 과거 측정의 발행 기록 · simlog/출력 스윕 색인."""
    return [p for p, _k in _timeline_parts(_repo(repo), ev)[1]]


# ── 캠페인 경계(2026-09-29 · FACT 교정 — 라벨만으로 묶던 기동 시도) ─────────────────────────────────────────────────
# 원장 행은 **라벨(셀 이름)** 로 이 셀에 묶인다. 같은 셀 이름이 앞선 캠페인에서도 쓰이면(camp-26092808 → camp-26092913 의
#   `ds4f0731-1m-spec7-roce`) 앞선 캠페인의 기동 시도 1~3 과 그 측정까지 "이 셀의 기동 시도" 로 세였다(4회 — 이번 캠페인은 1회).
#   경계 = min(현 캠페인 선언 `declared_utc`, 이 발행이 묶인 측정의 선언 창 시작) — 둘 중 앞선 것. 둘을 함께 보는 이유: 선언 시각은
#   손으로 적힌 값이라 실제 첫 선언보다 늦을 수 있다(실측: declared_utc 04:55:00Z · 이 측정을 서빙한 budget_declare 04:54:44Z).
#   경계 이전 행 = `campaign_scope: prior-campaign`(삭제 ✗ · 행은 그대로 · detail 에 "같은 셀 이름 · 앞선 캠페인") · 이후 = `this-campaign`.
#   재생(캠페인 purge · 선언 없음)은 경계를 관측하지 못한다 — 행에 scope 를 달지 않고(현 규칙: 라벨 전부) 경계 미관측을 표지한다.
CAMPAIGN_SCOPE_THIS = "this-campaign"
CAMPAIGN_SCOPE_PRIOR = "prior-campaign"
PRIOR_CAMPAIGN_NOTE = "같은 셀 이름 · 앞선 캠페인"


def _campaign_boundary(ev: CellEvidence, win: dict | None) -> dict:
    """{utc | None, source, observed} — 위 주석의 규칙(판정은 여기 한 벌 · template 은 옮기기만)."""
    decl = (ev.declaration or {}).get("declared_utc") if ev.mode == "campaign" else None
    decl = decl if isinstance(decl, str) and _UTC.fullmatch(decl) else None
    ws = (win or {}).get("start_ts") if ev.mode == "campaign" else None
    if decl is None and ws is None:
        why = ("재생 모드(캠페인 purge) — 캠페인 선언을 관측하지 못한다" if ev.mode != "campaign"
               else "캠페인 선언 declared_utc · 측정을 서빙한 선언 창 모두 미관측")
        return {"utc": None, "observed": False, "campaign_id": ev.campaign_id,
                "source": f"{why} · 경계 미관측 — 라벨(셀 이름)로 묶은 행 전부를 이 셀의 행으로 센다(같은 셀 이름의 앞선 캠페인 행이 섞였을 수 있다)"}
    parts = []
    if decl:
        parts.append(f"campaigns/{ev.campaign_id}/campaign.yaml declared_utc {decl}")
    if ws:
        parts.append(f"이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 {ws}")
    b = min(x for x in (decl, ws) if x)
    return {"utc": b, "observed": True, "campaign_id": ev.campaign_id,
            "source": " · ".join(parts) + f" → 경계 = 앞선 값 {b}(이전 행 = {PRIOR_CAMPAIGN_NOTE})"}


def event_campaign_boundary(repo, ev: CellEvidence) -> dict:
    """이 셀 타임라인의 캠페인 경계(공유 계약 `facts['event_campaign_boundary']`) — {utc, observed, campaign_id, source}."""
    repo = _repo(repo)
    return _campaign_boundary(ev, _ledger_timeline(repo, ev)[2])


def _scope_rows(rows: list[dict], bnd: dict) -> list[dict]:
    b = bnd.get("utc")
    if not b:
        return rows
    out = []
    for r in rows:
        u = r.get("utc")
        if not isinstance(u, str):
            out.append(r)
            continue
        if u < b:
            out.append({**r, "campaign_scope": CAMPAIGN_SCOPE_PRIOR,
                        "detail": f"{r.get('detail') or ''} · {PRIOR_CAMPAIGN_NOTE}(캠페인 경계 {b} 이전 — 이 셀의 시도로 세지 않는다)"})
        else:
            out.append({**r, "campaign_scope": CAMPAIGN_SCOPE_THIS})
    return out


def _timeline_parts(repo: Path, ev: CellEvidence) -> tuple[list[dict], list[tuple[str, str]]]:
    """(시각순 행, [(인용 파일, 후보 kind)]). 원장 행 + 과거 측정 행 + 측정 창 안 엔진 기동 표지. 행마다 캠페인 경계 대비
    `campaign_scope`(경계 관측 시)."""
    ctx: dict = {}
    rows, files, win = _ledger_timeline(repo, ev, ctx)
    mrows, mfiles = _measurement_rows(repo, ev, ctx)
    out = _scope_rows(sorted(rows + mrows + _startup_markers(repo, ev, win), key=_timeline_key), _campaign_boundary(ev, win))
    cited = {f: "events_ledger" for f in files}
    for f, k in mfiles:
        cited.setdefault(f, k)
    return out, sorted(cited.items())


def _json_key_line(path: Path, key: str, value) -> int | None:
    """JSON 파일에서 `"key": <value>` 가 처음 나오는 줄(1-기반 · `\\n` 기준 — grep·편집기와 같은 번호). 못 찾으면 None."""
    try:
        text = path.read_bytes().decode("utf-8", errors="replace")
    except OSError:
        return None
    rx = re.compile(r'"%s"\s*:\s*%s' % (re.escape(key), re.escape(json.dumps(value, ensure_ascii=False))))
    return next((i for i, line in enumerate(text.split("\n"), 1) if rx.search(line)), None)


def _other_measurements(repo: Path, ev: CellEvidence) -> list[dict]:
    """같은 셀 · 같은 identity 의 **다른** 측정 [{utc, node, what, source, files[(path, kind)]}](utc 가 같은 원천은 한 항목).
    원천 = 발행 기록(`benchmark.measured_utc` · 없으면 simlog 사본 `sweep_index.generated_utc`) + 출력 평면 스윕 색인(이 측정에 묶이지
    않았을 때 — 다른 측정이 덮었다). 셀 대조·identity 대조는 lineage 의 규칙 한 벌(record_cell_match · identity_equal)."""
    from . import lineage
    measured = _measured_key(repo, ev)
    ident = (ev.lineage_seeds or {}).get("identity") or _measured_identity(repo, ev)[0]
    this_topic = (ev.publication or {}).get("topic")
    found: dict[str, dict] = {}

    def add(utc, node, what: str, source: str, files: list[tuple[str, str]]) -> None:
        if not isinstance(utc, str) or not _UTC.fullmatch(utc) or utc == measured:
            return
        e = found.setdefault(utc, {"utc": utc, "node": node if node in NODE_AXIS else ev.node, "what": [], "source": source,
                                   "files": []})
        e["what"].append(what)
        e["files"] += files

    for rec in publication_records(repo):
        rid = rec.get("_id")
        if not isinstance(rid, str) or "_unreadable" in rec or rid == this_topic:
            continue
        if not lineage.identity_equal(rec.get("identity"), ident):
            continue
        via, _mismatch = lineage.record_cell_match(repo, rid, rec, ev.cell)
        if via is None:
            continue
        rec_rel = f"{core.REL_EVIDENCE_DIR}/{rid}.json"
        bm = rec.get("benchmark") if isinstance(rec.get("benchmark"), dict) else {}
        mu = _norm_value(bm.get("measured_utc"))
        simlog = (rec.get("raw_log_paths") or {}).get("simlog") or (rec.get("scaffolded") or {}).get("simlog")
        idx, idx_bad = _read_json_soft(repo / simlog / "sweep_index.json") if isinstance(simlog, str) and simlog \
            and not os.path.isabs(simlog) and ".." not in Path(simlog).parts else (None, None)
        idx = idx if isinstance(idx, dict) else {}
        gu = idx.get("generated_utc") if isinstance(idx.get("generated_utc"), str) else None
        node = ((idx.get("meta") or {}).get("measured_node")) if isinstance(idx.get("meta"), dict) else None
        files: list[tuple[str, str]] = []
        idx_rel = f"{simlog}/sweep_index.json" if gu else None
        if mu:
            ln = _json_key_line(repo / rec_rel, "measured_utc", mu)
            source = f"{rec_rel}:{ln}" if ln else rec_rel
            files.append((rec_rel, "publication_record"))
        elif gu:
            mu = gu
            ln = _json_key_line(repo / idx_rel, "generated_utc", gu)
            source = f"{idx_rel}:{ln}" if ln else idx_rel
        else:
            continue
        if idx_rel:
            files.append((idx_rel, "raw_json"))
        verdict = _norm_value(bm.get("verdict"))
        extra = (f" · simlog 사본 generated_utc {gu}{'(같음)' if gu == mu else ' ≠ measured_utc — 기록끼리 어긋남'}" if gu
                 else f" · simlog 사본 {idx_bad}" if idx_bad else "")
        add(mu, node, f"발행 기록 {rid}(셀 대조 {via} · verdict {verdict or '미기재'}){extra}", source, files)
    sw = ev.sweep or {}
    out_idx = repo / "output" / ev.topology / "benchlog" / f"sweep_{ev.cell}" / "sweep_index.json"
    if sw.get("binding") != "output" and out_idx.is_file():
        doc, _bad = _read_json_soft(out_idx)
        meta = (doc or {}).get("meta") if isinstance((doc or {}).get("meta"), dict) else {}
        gu = (doc or {}).get("generated_utc")
        sid = {"model": meta.get("model"), "gpu": meta.get("gpu_model"), "topology": meta.get("topology"),
               "tp": meta.get("tensor_parallel_size")}
        if isinstance(gu, str) and meta.get("config_name") == ev.cell and lineage.identity_equal(sid, ident):
            rel = _rel(repo, out_idx)
            ln = _json_key_line(out_idx, "generated_utc", gu)
            add(gu, meta.get("measured_node"), "출력 평면 스윕(이 측정에 묶이지 않음 — 다른 측정이 덮었다)",
                f"{rel}:{ln}" if ln else rel, [(rel, "sweep_json")])
    return [found[k] for k in sorted(found)]


def _measurement_rows(repo: Path, ev: CellEvidence, ctx: dict) -> tuple[list[dict], list[tuple[str, str]]]:
    """과거(·뒤) 측정 → 타임라인 행 + 인용 파일. 원장 판정: 그 시각을 덮는 선언 창(라벨 무관)이 있는가 · 직전·직후 예산 행."""
    cell = ev.cell
    measured = _measured_key(repo, ev)
    nodes = ctx.get("nodes") or []
    spans = ctx.get("spans") or {}
    wins = ctx.get("windows") or []
    brows = sorted(ctx.get("budget_rows") or [], key=lambda r: (r["utc"], r["rel"], r["line"]))
    out: list[dict] = []
    files: list[tuple[str, str]] = []
    for m in _other_measurements(repo, ev):
        t = m["utc"]
        # 2026-09-22(S2 round 3 적대 검토): 판정은 **그 시각을 관측한 원장**으로만 한다. 옛 판본은 ① 원장이 끊긴 노드도 "선언 없음" 의
        #   증인으로 세웠고 ② 스트림 끝에서 열린 채 끝난 창(end_ts None)이 그 뒤 모든 시각을 덮는다고 읽었다 — 라이브 sub 원장은
        #   09-11T11:10:38Z 에서 `smoke-nv4-f8-1m-res` 창을 연 채 끝나므로, 그 뒤의 어떤 측정도 "다른 라벨 창 안" 으로 오기됐을 것이다.
        seen = [n for n in nodes if n in spans and spans[n][0] <= t <= spans[n][1]]
        unseen = [n for n in nodes if n not in seen]
        live = [w for w in wins if w["node"] in seen and w["start_ts"] <= t]
        cover = [w for w in live if (t <= w["end_ts"] if w["end_ts"] is not None else t <= w["stream_last"])]
        dangling = [w for w in live if w["end_ts"] is None and t > w["stream_last"]]
        mine = [w for w in cover if _label_is_cell(w["label"], cell)]

        def wtxt(w: dict) -> str:
            end = w["end_ts"] or f"닫힘 미관측(스트림 마지막 행 {w['stream_last']})"
            return f"{w['label'] or '라벨 없음'} {w['start_ts']}~{end} ({w['rel']}:{w['line']})"

        def span_txt(ns: list[str]) -> str:
            return " · ".join(f"{n}({'기록 ' + spans[n][0] + '~' + spans[n][1] if n in spans else '행 없음'})" for n in ns)
        gap = f" · 이 시각을 관측하지 않은 원장: {span_txt(unseen)}" if unseen and seen else ""
        if not nodes:
            label, led = "measurement (ledger unobserved)", "원장 미관측(이 셀 노드 축의 docs/logs/<node>/events 없음)"
        elif not seen:
            label, led = ("measurement (ledger unobserved)",
                          f"원장이 이 시각을 관측하지 않는다(기록 범위 밖): {span_txt(unseen)}")
        elif mine:
            label, led = "measurement (budget window recorded)", "이 셀 선언 창 안: " + " · ".join(wtxt(w) for w in mine) + gap
        elif cover:
            label, led = ("measurement (other label's budget window)",
                          "이 셀 라벨의 선언은 없고 다른 라벨의 창 안: " + " · ".join(wtxt(w) for w in cover) + gap)
        elif dangling:
            label, led = ("measurement (budget window closure unobserved)",
                          "덮는 창은 관측되지 않았으나 스트림이 창을 연 채 끝났다(그 시각의 창 상태 미관측): "
                          + " · ".join(wtxt(w) for w in dangling) + gap)
        else:
            label, led = ("measurement (no budget event recorded)",
                          f"이 시각을 관측한 원장({', '.join(seen)})의 어느 라벨의 예산 선언 창도 이 시각을 덮지 않는다 — 예산 선언 없이 잰 측정"
                          + gap)
        near = []
        before = [r for r in brows if r["utc"] <= t]
        after = [r for r in brows if r["utc"] > t]
        for tag_, r in (("직전", before[-1] if before else None), ("직후", after[0] if after else None)):
            if r is None:
                near.append(f"{tag_} 예산 행 없음")
                continue
            near.append(f"{tag_} 예산 행 {r['kind']}{'(' + r['label'] + ')' if r['label'] else ''} {r['utc']}({r['rel']}:{r['line']})")
            files.append((r["rel"], "events_ledger"))
        for w in cover + dangling:
            files.append((w["rel"], "events_ledger"))
        rel_m = ("측정 시각 미관측" if measured is None else
                 f"이 발행의 측정 {measured} {'전' if t < measured else '뒤 — 이 측정과 무관할 수 있다'}")
        detail = (f"같은 셀의 다른 측정(측정 노드 축 {m['node']}) · 시각 = 측정 조립(sweep generated_utc = 인증서 measured_utc · 시작 시각 "
                  f"미기록) · {rel_m} · {' / '.join(m['what'])} · 원장: {led} · {' · '.join(near)}")
        # node = None: 이 행은 원장 노드의 사건이 아니다(측정 기록의 사실 — 노드 축은 detail 이 말한다). 2026-09-22 실측: node 를 `cluster`
        #   로 두면 template._attempts 가 이 행으로 cluster 시도를 열고 뒤따르는 엔진 기동 표지(node=cluster)를 전부 09-10 측정에 붙였다.
        out.append({"utc": t, "node": None, "kind": "measurement", "label": label, "detail": detail, "source": m["source"]})
        files += m["files"]
    return out, files


def bench_definition(repo, ev: CellEvidence) -> dict:
    """현행 full 정의(`.claude/rules/docs.md` 원문) ↔ 이 측정의 반복·도구(2026-09-22 · plan_26092119 S2 round 2 · F9).
    1차 재생은 "현행 full 정의를 계보에서 읽을 수 없다" 며 결론을 돌려 말했다(채점 #43 partial) — 정의 원문은 헌법층 문서에 있고
    계보(docs/)가 아니라서 저작자 입력에 없었다. 여기서는 원문 문장을 그대로 싣고, 대조는 관측한 것만으로 한다(모름 = None).
    반환 {current_full_definition, source, source_line, required_repeats, required_legs, this_repeats(완주 관측 · 없으면 None),
    repeats_source, repeats_requested(선언 · 판정에 쓰지 않는다), bench_tool, bench_tool_source, verdict_level, meets_current_full,
    reasons[]}."""
    repo = _repo(repo)
    out: dict = {"current_full_definition": None, "source": REL_DOCS_RULES, "source_line": None, "required_repeats": None,
                 "required_legs": [], "this_repeats": None, "repeats_source": "미관측", "bench_tool": None,
                 "bench_tool_source": None, "verdict_level": None, "meets_current_full": None, "reasons": []}
    p = repo / REL_DOCS_RULES
    if p.is_file():
        for i, ln in enumerate(p.read_text(encoding="utf-8", errors="replace").split("\n"), 1):
            m = _FULL_DEF_RE.search(ln)
            if m:
                out["current_full_definition"], out["source_line"] = m.group(0), i
                break
    sent = out["current_full_definition"]
    if sent:
        rm = _FULL_REPEATS_RE.search(sent)
        out["required_repeats"] = int(rm.group(1)) if rm else None
        inner = re.search(r"`([^`]*)`", sent)
        legs = inner.group(1).split("×")[0] if inner else ""
        out["required_legs"] = [x.strip() for x in legs.split("∪") if x.strip()]
    else:
        out["reasons"].append(f"{REL_DOCS_RULES} 에서 full 정의 문장(`full 계측(…)`)을 읽지 못했다 — 대조하지 않는다")
    mc = measurement_config(repo, ev)
    out["bench_tool"], out["bench_tool_source"] = mc.get("bench_tool"), mc.get("source")
    sw = ev.sweep or {}
    # 판정점 레벨은 기록된 것만 쓴다 — 미기재를 level 1 로 접으면 "판정점 반복" 이 고른 레벨의 사실이 된다(2026-09-22 · round 2 리뷰).
    vp = _as_int((sw.get("index") or {}).get("verdict_point_level"))
    out["verdict_level"] = vp
    # 반복 수 = **완주 관측**만(2026-09-22 · S2 round 2 리뷰). 측정 구성 표의 `repeats` 는 **요청**(repeat_axis `requested` · 선언)이라
    #   완주를 말하지 않는다 — 종전 판은 그것을 this_repeats 로 써서 요청 3·완주 2 인 셀을 "반복 3 ≥ 3 충족" 으로 판정할 수 있었다(결정 경로의
    #   선언 폴백). 순서: 측정 구성 표 repeats_completed > 판정점 레벨 measured.repeats_completed > 그 runs[] measurement_ok 수 > 반복 축
    #   이전 배치(판정점 레벨에 run_KK/ 없이 bench 결과 1개 = 단일 run). run_KK/ 가 있는데 집계가 없으면 **판정 불가**(sweep_bench 가
    #   스스로 "runs[] 없는 레벨 = bench_mode 판정 불가" 라 적는 자리 — 파일 수는 실패 run 까지 센다).
    out["repeats_requested"] = _as_int(mc.get("repeats"))
    lv = (next((x for x in sw.get("levels") or [] if isinstance(x, dict) and _as_int(x.get("level")) == vp), None)
          if vp is not None else None)
    lm = (lv or {}).get("measured") if isinstance((lv or {}).get("measured"), dict) else {}
    if _as_int(mc.get("repeats_completed")) is not None:
        out["this_repeats"] = _as_int(mc["repeats_completed"])
        out["repeats_source"] = f"측정 구성 표 repeats_completed(완주 · {mc.get('source')})"
    elif vp is None:
        out["repeats_source"] = "미관측 — 스윕 판정점 레벨(verdict_point_level) 미기재 · 측정 구성 표에 완주 수 없음"
    elif _as_int(lm.get("repeats_completed")) is not None:
        out["this_repeats"] = _as_int(lm["repeats_completed"])
        out["repeats_source"] = f"sweep_index levels[{vp}].measured.repeats_completed(완주 · {sw.get('dir')})"
    elif isinstance(lm.get("runs"), list):
        out["this_repeats"] = sum(1 for r in lm["runs"] if isinstance(r, dict) and r.get("measurement_ok") is True)
        out["repeats_source"] = f"sweep_index levels[{vp}].measured.runs[] measurement_ok 수(완주 · {sw.get('dir')})"
    elif sw.get("binding") == "output":
        ldir = repo / sw["dir"] / f"level_{vp:02d}"
        run_dirs = sorted(d.name for d in ldir.glob("run_*") if d.is_dir()) if ldir.is_dir() else []
        if run_dirs:
            out["repeats_source"] = (f"판정 불가 — {_rel(repo, ldir)} 에 반복 축 {run_dirs} 가 있는데 집계(runs[]·repeats_completed)가 없다"
                                     "(파일 수는 실패 run 까지 센다)")
        elif (ldir / f"bench_{ev.cell}.json").is_file() and core.level_raw_is_measured_tool(ldir / f"bench_{ev.cell}.json", ev.cell):
            out["this_repeats"] = 1
            out["repeats_source"] = (f"observed({_rel(repo, ldir)}/bench_{ev.cell}.json 1개 · run_KK/ 없음 · runs[] 없음 — 반복 축 이전의 "
                                     "단일 run 배치)")
    if out["this_repeats"] is None and out["repeats_requested"] is not None:
        out["repeats_source"] += f" · 요청 반복 {out['repeats_requested']}(선언 — 완주 수가 아니라 판정에 쓰지 않는다)"
    bm = mc.get("bench_mode")
    need, legs = out["required_repeats"], [x.lower() for x in out["required_legs"]]
    tool = str(out["bench_tool"] or "").lower()
    verdict = None
    if sent:
        if bm == "lite":
            verdict = False
            out["reasons"].append("측정 구성 bench_mode=lite — full 측정이 아니다")
        if need is not None and out["this_repeats"] is not None and out["this_repeats"] < need:
            verdict = False
            out["reasons"].append(f"판정점 반복 {out['this_repeats']} < 현행 정의 {need}")
        if "guidellm" in legs and tool and "guidellm" not in tool:
            verdict = False
            out["reasons"].append(f"측정 도구 {out['bench_tool']} — 현행 정의의 full 레그(GuideLLM)가 아니다")
        if verdict is None and need is not None and out["this_repeats"] is not None and out["this_repeats"] >= need \
                and ("guidellm" not in legs or "guidellm" in tool):
            verdict = True
            out["reasons"].append(f"판정점 반복 {out['this_repeats']} ≥ {need} · 도구 {out['bench_tool']}")
        if verdict is None:
            out["reasons"].append("반복 수 또는 도구를 관측하지 못했다 — 충족 여부를 판정하지 않는다(모름 ≠ 미충족)")
        if verdict is False and bm == "full":
            # 기록된 라벨과 현행 대조가 갈리는 자리를 말해 둔다(2026-09-22 · round 2 리뷰) — 태그2 인증서는 benchmark_mode=full 인데
            #   현행 정의로는 미충족이다. 말하지 않으면 저작자가 둘 중 하나를 골라 적는다.
            out["reasons"].append(f"측정 기록의 bench_mode=full 은 기록된 라벨({mc.get('source')})이다 — 위 대조는 현행 정의 기준이라 "
                                  "두 판정이 갈린다")
    out["meets_current_full"] = verdict
    return out


# vllm bench serve 결과 JSON 키 → CLI 플래그(값 그대로). 결과에 **있는 키만** 옮긴다(닫힌 목록 · 추측 ✗).
_BENCH_JSON_FLAGS = (("backend", "--backend"), ("endpoint", "--endpoint"), ("model_id", "--model"), ("tokenizer_id", "--tokenizer"),
                     ("dataset_name", "--dataset-name"), ("random_input_len", "--random-input-len"),
                     ("random_output_len", "--random-output-len"), ("random_range_ratio", "--random-range-ratio"),
                     ("num_prompts", "--num-prompts"), ("max_concurrency", "--max-concurrency"),
                     ("request_rate", "--request-rate"), ("burstiness", "--burstiness"), ("num_warmups", "--num-warmups"),
                     ("temperature", "--temperature"), ("seed", "--seed"))
_BENCH_JSON_BOOL_FLAGS = (("ignore_eos", "--ignore-eos"), ("trust_remote_code", "--trust-remote-code"))
# 결과 JSON 이 들지 않아 재구성하지 못하는 인자(관측 불가 목록 · 저작자에게 "모른다" 를 알린다).
BENCH_COMMAND_SOURCE = "reconstructed(bench json)"
_BENCH_UNRECORDED = ("--base-url/--port", "--endpoint", "--dataset-name", "--ignore-eos", "--num-warmups", "--temperature",
                     "--random-range-ratio", "--seed", "--trust-remote-code")


def bench_command(repo, ev: CellEvidence) -> dict:
    """판정점 레벨의 `vllm bench serve` 결과 JSON 필드로 재구성한 명령(2026-09-22 · plan_26092119 S2 round 2 · F9).
    1차 재생은 "명령 줄 원문은 계보에 남아 있지 않다" 며 자매 캠페인 패턴을 옮겼다(채점 M1 partial). 원문은 없다 — 그러나 결과 JSON 은
    그 실행의 인자 일부를 **그대로 든다**. 있는 필드만 플래그로 옮기고(`fields[]` 에 키·값·출처), JSON 이 들지 않는 인자는
    `unrecorded[]` 에 적는다. 요청당 입출력 길이는 결과에 길이 키가 없을 때만 `total_*_tokens ÷ completed` 가 **나누어떨어질 때**
    파생하며(평균 · 균일 길이는 관측 아님) 그 사실을 플래그 출처에 적는다. 반환 {command|None, source, fields[], unrecorded[]}."""
    repo = _repo(repo)
    sw = ev.sweep or {}
    # 재구성 불가 반환도 같은 모양이다(command_source=None · 2026-09-22 round 2 리뷰 — 소비자가 키 부재와 재구성 불가를 가르지 않게).
    none = {"command": None, "command_source": None, "fields": [], "unrecorded": list(_BENCH_UNRECORDED)}
    if sw.get("binding") != "output":
        return {**none, "source": (f"미재구성 — 스윕이 출력 평면에 묶이지 않았다(binding={sw.get('binding')!r}) · 레벨 원시가 "
                                   "이 측정의 것이 아니다")}
    idx = sw.get("index") or {}
    vp_rec = _as_int(idx.get("verdict_point_level"))
    vp = vp_rec or 1
    # 판정점 미기재면 첫 레벨로 재구성하되 그 사실을 출처에 적는다(명령 재구성은 판정이 아니다 · 고른 레벨을 판정점이라 부르지 않는다).
    at = f"판정점 level {vp}" if vp_rec else "판정점 미기재 → 첫 레벨 level 1(판정점이라는 뜻 아님)"
    path = repo / sw["dir"] / f"level_{vp:02d}" / f"bench_{ev.cell}.json"
    doc = _read_json_opt(path, "레벨 벤치 JSON") if path.is_file() else None
    rel = _rel(repo, path)
    if isinstance(doc, dict) and not core.level_raw_is_measured_tool(path, ev.cell):
        return {**none, "source": (f"미재구성 — {rel} 는 이 레벨의 측정 도구(bench_tool_{ev.cell}.json)가 아닌 옛 도구 원시다"
                                   f"({at} · 도구를 바꿔 재스윕한 자리의 잔재 — 2026-09-23 D1)")}
    if not isinstance(doc, dict):
        return {**none, "source": f"미재구성 — {rel} 부재({at})"}
    if not all(k in doc for k in ("num_prompts", "total_input_tokens", "completed")):
        return {**none, "source": f"미재구성 — {rel} 가 vllm bench serve 결과 모양이 아니다(num_prompts·total_input_tokens 부재)"}
    fields: list[dict] = []
    args: list[str] = []

    def add(flag: str, value, key: str, source: str) -> None:
        fields.append({"flag": flag, "value": value, "field": key, "source": source})
        args.extend([flag, shlex.quote(str(value))] if value is not True else [flag])

    for key, flag in _BENCH_JSON_FLAGS:
        v = doc.get(key)
        if v is None or isinstance(v, (dict, list)) or (isinstance(v, str) and not v.strip()):
            continue
        add(flag, v, key, f"{rel} {key}")
    for key, flag in _BENCH_JSON_BOOL_FLAGS:
        if doc.get(key) is True:
            add(flag, True, key, f"{rel} {key}=true")
    done = _as_int(doc.get("completed"))
    # 파생하지 못한 길이 플래그도 unrecorded 에 **사유와 함께** 적는다(2026-09-22 · round 2 리뷰) — 조용히 빼면 명령이 도구 기본값으로
    #   읽힌다(빠진 것이 "기록 없음" 인지 "기록끼리 어긋남" 인지 수신자가 알 수 없다).
    not_derived: list[str] = []
    for key, tot, flag in (("random_input_len", "total_input_tokens", "--random-input-len"),
                           ("random_output_len", "total_output_tokens", "--random-output-len")):
        t = _as_int(doc.get(tot))
        if key in doc:
            continue
        if not done or t is None or t % done:
            not_derived.append(f"{flag}({tot} ÷ completed 가 나누어떨어지지 않거나 없음 — 파생 ✗)")
            continue
        per = t // done
        why = f"derived({tot} {t} ÷ completed {done} — 요청당 평균 · 균일 길이는 관측 아님)"
        il = _as_int(idx.get("input_len")) if key == "random_input_len" else None
        if il is not None:
            if il != per:
                # 두 기록이 어긋나면 고르지 않는다(플래그를 싣지 않는다 · 어긋남을 적는다)
                not_derived.append(f"{flag}(파생 {per} ≠ sweep_index.input_len {il} — 고르지 않는다)")
                continue
            why += f" · sweep_index.input_len={il} 일치"
        add(flag, per, tot, why)
    present = {f["flag"] for f in fields}
    unrec = [f for f in _BENCH_UNRECORDED if f not in present] + not_derived
    # command_source 는 공유 계약의 정확한 어휘(reproduce_steps 가 그대로 옮긴다) · source 는 그 원천 파일까지 적은 서술이다.
    return {"command": "vllm bench serve " + " ".join(args), "command_source": BENCH_COMMAND_SOURCE,
            "source": f"{BENCH_COMMAND_SOURCE} — {rel}({at} · 결과 JSON 필드만 · 원문 명령 아님)",
            "fields": fields, "unrecorded": unrec}


# ── 측정 환경 관측 · 측정 도구 스냅숏 · vLLM 관측 라벨 · attestation 범위 (2026-09-22 · plan_26092119 S2 round 3) ──────────
# 2차 재생의 오도 바이트 1,112 B 는 전부 "측정 **뒤** 재생성된 env 형상" 이었다(렌더러 782fd70 · fe1bc42 가 09-17 에 NCCL_NET ·
#   NCCL_DMABUF_ENABLE 를 더했다 — 09-12 측정 때는 없던 키). 그 형상은 싣지 않고(artifacts 가 slots.compose.excluded 로 옮긴다),
#   측정 당시 값은 **이 측정의 엔진 로그**가 되읊은 줄로 싣는다. 사후 사실확인의 오기 3건(run_bench.sh 의 --temperature 0 · lite_bench.sh
#   무변경 · 인증서 vllm_version 의 파생)은 측정 시점 도구 원천이 저장소 git 에 있었는데 저작자 입력에 없던 것이다 — 그 바이트를
#   draft 에 스냅숏한다. 인증서 vllm_version 을 "엔진 자기보고" 라 부른 오기(wrong 1건)는 그 값의 producer 를 측정 시점 sweep_bench.sh
#   에서 **검증**해 라벨로 싣는다.
REL_BENCH_TOOLS_DIR = ".claude/skills/adversarial-benchmark/scripts"
# 측정 도구(닫힌 목록 · tripwire): (파일, 역할). 드라이버(sweep_bench 를 부르는 스크립트)는 목록이 아니라 측정 시점 판본에서 **찾는다**.
_BENCH_TOOLS = (("sweep_bench.sh", "sweep"), ("run_bench.sh", "bench"), ("lite_bench.sh", "lite"))
_SDIR_CALL = '"$SDIR/{}"'                         # sweep_bench 의 하위 도구 호출 모양(`bash "$SDIR/run_bench.sh" …`) — 판본 본문에서 찾는다
# 인증서 bench_tool ↔ sweep_bench `--tool` 어휘(tripwire — 드라이버가 넘기는 --tool 상수와 측정 도구가 다르면 그 드라이버가 아니다).
_BENCH_TOOL_ARG = {"vllm-bench-serve": "vllm", "guidellm": "guidellm"}
_TOOL_ARG_RE = re.compile(r"--tool\s+\"?([A-Za-z][A-Za-z0-9_-]*)\"?")
# reflog 파일 한 줄 `<old> <new> <ident> <epoch> <tz>\t<msg>`(git 내부 형식 · 읽기만). `.*` 는 탐욕이라 ident 안의 숫자에 걸리지 않고
#   탭 바로 앞의 `<epoch> <tz>` 로 되돌아온다.
_REFLOG_LINE_RE = re.compile(r"^([0-9a-f]{40}) ([0-9a-f]{40}) .* (\d+) [+-]\d{4}(?:\t|$)")
# NCCL 이 읽은 env 의 되읊음(vLLM Ray 워커 로그 · 닫힌 목록 · tripwire). 문구가 바뀌면 줄을 못 읽을 뿐 값을 지어내지 않는다.
_ENV_ECHO_RES = (("env-echo", re.compile(r"\bNCCL INFO (NCCL_[A-Z0-9_]+) set by environment to (\S+)")),
                 ("param-echo", re.compile(r"\bNCCL INFO (NCCL_[A-Z0-9_]+) set to (\S+)")))
# NCCL 런타임 관측(env 가 아니다 — 키를 소문자 `nccl:` 로 적어 env 이름과 섞이지 않게 한다).
_ENV_RUNTIME_RES = (("nccl:network", re.compile(r"\bNCCL INFO Using network (\S+)")),
                    ("nccl:net_ib_devices", re.compile(r"\bNCCL INFO NET/IB : Using (.+?)\s*$")),
                    ("nccl:version", re.compile(r"\bNCCL INFO NCCL version (\S+)")))
_RAY_DEDUP_RE = re.compile(r"\s*\[repeated (\d+)x across cluster\].*$")
_NCCL_HOST_RE = re.compile(r"(?:^|[\s)])([A-Za-z0-9][A-Za-z0-9.-]*):\d+:\d+ \[\d+\] NCCL INFO")
_RAY_IP_RE = re.compile(r"\bip=(\d{1,3}(?:\.\d{1,3}){3})\)")
_NODE_PH_RE = re.compile(r"<node:([A-Za-z0-9_-]+)>")
# vLLM 기동 배너(엔진 자기보고 · 닫힌 목록 · tripwire): 아스키 배너 줄 · API 서버 줄 · 옛 V1 엔진 줄.
_BANNER_RES = (re.compile(r"█\s+version\s+(\d+\.\d+\.\d+[0-9A-Za-z.+_-]*)"),
               re.compile(r"vLLM API server version (\d+\.\d+\.\d+[0-9A-Za-z.+_-]*)"),
               re.compile(r"Initializing a V1 LLM engine \(v(\d+\.\d+\.\d+[0-9A-Za-z.+_-]*)\)"))
# 측정 시점 sweep_bench.sh 가 인증서 강한 키 vllm_version 을 만드는 규칙의 표지(tripwire — 판본이 바뀌어 표지를 못 찾으면 producer 는
#   '미검증' 으로 적는다 · 규칙을 여기서 다시 쓰지 않고 그 판본의 정규식 리터럴을 꺼내 **그대로** 적용한다).
_CERT_VLLM_MARKS = {"image_rule": re.compile(r"""re\.search\(r"(easy-vllm:[^"]+)",\s*_img\)"""),
                    "image_src": re.compile(r"""_img\s*=\s*grep_env\(envtext,\s*"IMAGE_TAG"\)"""),
                    "env_override": re.compile(r"""os\.environ\.get\("VLLM_VER"\)"""),
                    "banner": re.compile(r"""vllm_build\s*=\s*_m\.group\(1\)""")}
_WORKTREE_NOTE = "측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다"


def _utc_of_epoch(ep: int) -> str:
    return _dt.datetime.fromtimestamp(int(ep), tz=_dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _git_repo(repo: Path) -> bool:
    return (repo / ".git").exists()


def _head_at(repo: Path, utc: str | None) -> tuple[str | None, str]:
    """측정 시각의 체크아웃 커밋 → (sha | None, 방법). 1순위 reflog(`HEAD@{<epoch>}` · 그 시각 직전 이동의 새 값 — 브랜치 무관 · 워크트리
    무관) · 2순위 HEAD 이력의 committer date ≤ 측정 시각(reflog 가 그 시각을 덮지 못할 때 · 체크아웃 브랜치 미검증을 적는다)."""
    t = _utc_dt(utc)
    if t is None:
        return None, "측정 시각 미관측 — 판본을 고르지 않는다"
    if not _git_repo(repo):
        return None, "git 저장소가 아니다 — 측정 시점 판본 미관측"
    ep = int(t.timestamp())
    # HEAD 자신의 reflog 만 읽는다 — `logs/HEAD` 가 없으면 `git log -g HEAD` 는 **브랜치** reflog 로 조용히 넘어간다(2026-09-22 자체검사
    #   실측). 브랜치 reflog 는 체크아웃 이동을 적지 않으므로 "측정 때 체크아웃" 의 근거가 아니다.
    lp = core.git(repo, "rev-parse", "--git-path", "logs/HEAD", check=False).stdout.strip()
    has_head_log = bool(lp) and (repo / lp).is_file()
    # reflog 파일을 직접 읽는다(2026-09-22 적대 검토): `git log -g` 는 커밋 오브젝트가 없는 항목을 **조용히 건너뛰어** 그 시각의 실제
    #   체크아웃 대신 그 전 항목을 "직전 체크아웃" 으로 내놓는다(실측: 없는 커밋 항목을 덧붙여도 rc 0 · 그 줄만 빠진다). 파일은 추가 전용이라
    #   줄 순서가 기록 순서다 — 같은 초의 동률은 뒤 줄(더 나중 기록)을 고른다.
    entries: list[tuple[int, int, str]] = []
    if has_head_log:
        try:
            raw = (repo / lp).read_bytes().decode("utf-8", errors="replace")
        except OSError:
            raw = ""
        for i, ln in enumerate(raw.split("\n")):
            m = _REFLOG_LINE_RE.match(ln)
            if m:
                entries.append((int(m.group(3)), i, m.group(2)))
    before = [e for e in entries if e[0] <= ep]
    if before:
        ts, _i, sha = max(before)
        if core.git(repo, "cat-file", "-e", f"{sha}^{{commit}}", check=False).returncode == 0:
            return sha, f"reflog HEAD@{{{ts}}}({_utc_of_epoch(ts)}) = 측정 시각 {utc} 직전 체크아웃"
    # 대체 사유를 사실대로 적는다(2026-09-22 적대 검토: 옛 문구는 reflog 가 그 시각을 덮었지만 커밋 오브젝트가 없는 경우에도 "덮지 않는다" 라 했다).
    why = (f"reflog HEAD@{{{max(e[0] for e in before)}}} 의 커밋 오브젝트 부재(gc 등)" if before else
           f"reflog 가 측정 시각을 덮지 않는다(최초 {_utc_of_epoch(min(e[0] for e in entries))})" if entries else "reflog 없음")
    out = core.git(repo, "-c", "log.showSignature=false", "log", "-1", f"--before={utc}", "--format=%H", "HEAD", check=False)
    sha = out.stdout.strip() if out.returncode == 0 else ""
    if re.fullmatch(r"[0-9a-f]{40}", sha):
        return sha, f"HEAD 이력의 committer date ≤ {utc}({why} — 측정 때 체크아웃 브랜치는 미검증)"
    return None, f"측정 시각 이전 커밋 없음({why})"


def _git_text(repo: Path, rev: str, rel_path: str) -> bytes | None:
    return core.git_bytes(repo, "show", f"{rev}:{rel_path}", check=False)


def _tool_revision(repo: Path, at: str, rel_path: str) -> dict | None:
    """측정 시점 커밋 `at` 에서 그 경로의 판본: {git_rev(그 경로를 마지막으로 바꾼 커밋), git_rev_utc, next_rev, next_rev_utc}. 없으면 None."""
    if core.git(repo, "cat-file", "-e", f"{at}:{rel_path}", check=False).returncode != 0:
        return None
    out = core.git(repo, "-c", "log.showSignature=false", "log", "-1", "--format=%H %ct", at, "--", rel_path, check=False)
    parts = out.stdout.split() if out.returncode == 0 else []
    if len(parts) != 2 or not re.fullmatch(r"[0-9a-f]{40}", parts[0]):
        return None
    rev = parts[0]
    # origin(`git:<git_rev>:<path>`)이 스냅숏 바이트(`at:<path>`)를 그대로 말해야 한다(2026-09-22 · S2 round 3 적대 검토) — 경로 제한
    #   log 의 마지막 변경 커밋은 보통 같은 blob 이지만 그것을 가정하지 않고 blob id 로 확인한다. 다르면 측정 시점 커밋 자체를 쓴다.
    blob = [core.git(repo, "rev-parse", "--verify", "--quiet", f"{r}:{rel_path}", check=False).stdout.strip() for r in (rev, at)]
    rev_note = None
    if not blob[0] or blob[0] != blob[1]:
        rev_note = f"경로 제한 log 의 마지막 변경 {rev[:12]} 의 blob 이 측정 시점 {at[:12]} 와 달라 측정 시점 커밋을 쓴다"
        rev = at
    row = {"git_rev": rev, "git_rev_utc": _utc_of_epoch(int(parts[1])) if rev == parts[0] else None, "git_rev_note": rev_note,
           "next_rev": None, "next_rev_utc": None}
    if rev != parts[0]:
        ct = core.git(repo, "-c", "log.showSignature=false", "log", "-1", "--format=%ct", at, check=False).stdout.strip()
        row["git_rev_utc"] = _utc_of_epoch(int(ct)) if ct.isdigit() else None
    # 다음 변경은 **지금 HEAD 가 측정 시점 커밋의 후손일 때만** 말한다(다른 브랜치면 `at..HEAD` 가 무관한 커밋을 섞는다).
    #   next_rev None 이 "변경 없음" 인지 "미관측" 인지 가르는 칸이 next_rev_basis 다(2026-09-22 적대 검토 — 측정 체크아웃과 다른
    #   브랜치에서 발행하면 옛 판본은 두 경우를 같은 None 으로 냈고, 렌더는 그것을 "없음" 으로 읽었다).
    if core.git(repo, "merge-base", "--is-ancestor", at, "HEAD", check=False).returncode == 0:
        nx = core.git(repo, "-c", "log.showSignature=false", "log", "--reverse", "--format=%H %ct", f"{at}..HEAD", "--", rel_path,
                      check=False)
        first = (nx.stdout.splitlines() or [""])[0].split() if nx.returncode == 0 else []
        if len(first) == 2:
            row["next_rev"], row["next_rev_utc"] = first[0], _utc_of_epoch(int(first[1]))
            row["next_rev_basis"] = f"{at[:12]}..HEAD 경로 이력의 첫 변경"
        elif nx.returncode == 0:
            row["next_rev_basis"] = f"{at[:12]}..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트)"
        else:
            row["next_rev_basis"] = "미관측(git log 실패)"
    else:
        row["next_rev_basis"] = (f"미관측 — 지금 HEAD 가 측정 시점 커밋 {at[:12]} 의 후손이 아니다(다른 브랜치에서 발행 · 다음 변경을 "
                                 "말하지 않는다 — '변경 없음' 이 아니다)")
    return row


def _measured_bench_tool(ev: CellEvidence) -> str | None:
    meta = ((ev.sweep or {}).get("index") or {}).get("meta") or {}
    return _norm_value(meta.get("bench_tool")) or _norm_value((ev.certificate or {}).get("bench_tool"))


def _tool_selection(repo: Path, ev: CellEvidence) -> dict:
    """측정 도구 선택(판정은 관측만): {at, at_method, measured_utc, tools[{name, repo_path, role, why}], driver_note}.
    sweep_bench.sh = 스윕 색인의 조립자 · run_bench.sh·lite_bench.sh = 그 판본 본문이 부르고(`"$SDIR/<도구>"`) 색인이 그 레그를 기록할 때 ·
    드라이버 = 그 판본 디렉터리에서 sweep_bench.sh 를 부르는 스크립트 중 넘기는 `--tool` 상수가 이 측정 bench_tool 과 어긋나지 않는 것
    (어긋나면 배제하고 사유를 적는다 · 상수가 없으면 미검증 후보로 싣는다)."""
    measured = _measured_key(repo, ev)
    at, method = _head_at(repo, measured)
    out: dict = {"at": at, "at_method": method, "measured_utc": measured, "tools": [], "driver_note": None}
    if at is None:
        return out
    sw = ev.sweep or {}
    idx = sw.get("index") or {}
    sweep_rel = f"{REL_BENCH_TOOLS_DIR}/sweep_bench.sh"
    sweep_text = (_git_text(repo, at, sweep_rel) or b"").decode("utf-8", errors="replace") if sw else ""
    if sw and sweep_text:
        out["tools"].append({"name": "sweep_bench.sh", "repo_path": sweep_rel, "role": "sweep",
                             "why": f"스윕 색인 조립자({sw.get('dir')}/sweep_index.json · generated_utc = measured_utc 조인)"})
        if sw.get("levels") and _SDIR_CALL.format("run_bench.sh") in sweep_text:
            out["tools"].append({"name": "run_bench.sh", "repo_path": f"{REL_BENCH_TOOLS_DIR}/run_bench.sh", "role": "bench",
                                 "why": "그 판본 sweep_bench.sh 가 레벨마다 부른다(`\"$SDIR/run_bench.sh\"`) · 색인에 레벨 기록"})
        if isinstance(idx.get("lite"), dict) and _SDIR_CALL.format("lite_bench.sh") in sweep_text:
            out["tools"].append({"name": "lite_bench.sh", "repo_path": f"{REL_BENCH_TOOLS_DIR}/lite_bench.sh", "role": "lite",
                                 "why": "그 판본 sweep_bench.sh 가 lite 레그로 부른다(`\"$SDIR/lite_bench.sh\"`) · 색인에 lite 기록"})
    elif not sw and ev.report_kind == "lite":
        out["tools"].append({"name": "lite_bench.sh", "repo_path": f"{REL_BENCH_TOOLS_DIR}/lite_bench.sh", "role": "lite",
                             "why": "경량 리포트 셀(lite_bench.sh 가 리포트를 발행한다)"})
    if not sweep_text:
        return out
    ls = core.git(repo, "ls-tree", "--name-only", at, f"{REL_BENCH_TOOLS_DIR}/", check=False)
    names = sorted(PurePath(p).name for p in (ls.stdout.splitlines() if ls.returncode == 0 else ()))
    want = _BENCH_TOOL_ARG.get(_measured_bench_tool(ev) or "")
    notes = []
    n_cand = 0
    for nm in names:
        if not nm.endswith(".sh") or nm in {n for n, _ in _BENCH_TOOLS}:
            continue
        text = (_git_text(repo, at, f"{REL_BENCH_TOOLS_DIR}/{nm}") or b"").decode("utf-8", errors="replace")
        if _SDIR_CALL.format("sweep_bench.sh") not in text:
            continue
        consts = sorted({m.group(1) for ln in text.splitlines() if not ln.lstrip().startswith("#")
                         for m in _TOOL_ARG_RE.finditer(ln)})
        if want and len(consts) == 1 and consts[0] != want:
            notes.append(f"{nm} 배제(그 판본은 sweep_bench 에 --tool {consts[0]} 를 넘긴다 ≠ 이 측정 bench_tool "
                         f"{_measured_bench_tool(ev)} → --tool {want})")
            continue
        out["tools"].append({"name": nm, "repo_path": f"{REL_BENCH_TOOLS_DIR}/{nm}", "role": "driver-candidate",
                             "why": (f"그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 {consts or '없음'} 가 이 측정과 "
                                     "어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)")})
        notes.append(f"{nm} 후보(미검증)")
        n_cand += 1
    # 주장의 범위를 적는다(2026-09-22 · S2 round 3 적대 검토): 탐색은 **그 판본의 벤치 도구 디렉터리 하나**다. 옛 문구("직접 호출" ·
    #   "드라이버 없이 직접 불렀다")는 저장소 밖 호출자(세션 스크래치 스크립트 — 캠페인⑦ 사후감사의 실제 producer)와 미커밋 스크립트를
    #   배제한 것처럼 읽혔다. 판정은 문구 매치가 아니라 후보 수(n_cand)로 한다.
    scope = f"그 판본의 {REL_BENCH_TOOLS_DIR}/"
    outside = "저장소 밖 호출자(세션 스크래치 스크립트 등)·미커밋 스크립트는 관측 대상 밖"
    out["driver_note"] = (f"{scope} 안에서 sweep_bench.sh 를 부르는 드라이버: " + " · ".join(notes)) if notes else \
        f"{scope} 안에 sweep_bench.sh 를 부르는 스크립트가 없다 — 드라이버 미관측({outside})"
    if notes and n_cand == 0:
        out["driver_note"] += (" → 이 디렉터리의 드라이버는 이 측정을 부르지 않았다(배제로 추론 · 호출 기록 아님) — sweep_bench.sh 를 직접 "
                               f"또는 저장소 밖 호출자가 불렀다({outside})")
    return out


def tool_snapshots(repo, ev: CellEvidence, draft_dir) -> list[dict]:
    """측정 도구 원천을 측정 시점 판본으로 draft 에 스냅숏한다(공유 계약 `facts['tool_snapshots']`). 반환
    [{name, repo_path, git_rev, snapshot_rel, role, why, head_at_measurement, rev_method, git_rev_utc, git_rev_note, next_rev, next_rev_utc,
    next_rev_basis(next_rev None 이 '변경 없음' 인지 '미관측' 인지), worktree_note, driver_note}] · 부수효과 =
    `<draft>/inputs/sources/<이름>@<rev12>` 쓰기(`git show <rev>:<path>` 바이트 그대로) + `ev.lineage_seeds['evidence_candidates']` 에 `{path, kind: tool-source@rev, origin: git:<rev>:<path>}` 추가.
    ★ 호출 순서: lineage.derive **전에** 부른다(후보가 LINEAGE 에 실려야 발췌 출처가 계보 안이다). git 이 아니거나 측정 시각이 없으면
    [](지어내지 않는다). draft 밖에는 쓰지 않는다."""
    from . import lineage
    repo = _repo(repo)
    if draft_dir is None:
        core.fail("HINT_DRAFT_REQUIRED", "tool_snapshots 는 draft 디렉터리가 필요하다(스냅숏은 draft/inputs/sources 에만 쓴다).")
    draft = Path(draft_dir)
    sel = _tool_selection(repo, ev)
    rows: list[dict] = []
    for t in sel["tools"]:
        rv = _tool_revision(repo, sel["at"], t["repo_path"])
        if rv is None:
            continue
        data = _git_text(repo, sel["at"], t["repo_path"])
        if data is None:
            continue
        rel_path = lineage.tool_snapshot_rel(t["name"], rv["git_rev"])
        dest = draft / rel_path
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
        rows.append({"name": t["name"], "repo_path": t["repo_path"], "git_rev": rv["git_rev"], "snapshot_rel": rel_path,
                     "role": t["role"], "why": t["why"], "head_at_measurement": sel["at"], "rev_method": sel["at_method"],
                     "git_rev_utc": rv["git_rev_utc"], "next_rev": rv["next_rev"], "next_rev_utc": rv["next_rev_utc"],
                     "next_rev_basis": rv["next_rev_basis"], "git_rev_note": rv["git_rev_note"],
                     "worktree_note": _WORKTREE_NOTE,
                     "driver_note": sel["driver_note"] if t["role"] == "sweep" else None})
    cands = [c for c in (ev.lineage_seeds or {}).get("evidence_candidates") or [] if isinstance(c, dict)]
    have = {c.get("path") for c in cands}
    for r in rows:
        if r["snapshot_rel"] not in have:
            cands.append({"path": r["snapshot_rel"], "kind": lineage.TOOL_SOURCE_KIND,
                          "origin": f"git:{r['git_rev']}:{r['repo_path']}"})
    ev.lineage_seeds = {**(ev.lineage_seeds or {}), "evidence_candidates": cands}
    return rows


def _sweep_engine_logs(base: Path, idx: dict, levels: list, cell: str) -> list[Path]:
    """스윕 디렉터리의 엔진 로그 — 색인이 기록한 레그만(lite · 색인 레벨의 `level_NN/**/engine_<셀>.log`). 색인 밖 레벨 디렉터리는
    다른 측정의 잔재일 수 있다(다음 스윕이 레벨을 덜 돌면 옛 파일이 남는다)."""
    logs: list[Path] = []
    if isinstance(idx.get("lite"), dict) and (base / f"lite_engine_{cell}.log").is_file():
        logs.append(base / f"lite_engine_{cell}.log")
    lv = sorted({n for n in (_as_int(x.get("level")) for x in levels if isinstance(x, dict)) if n})
    for n in lv:
        logs += sorted(p for p in (base / f"level_{n:02d}").glob(f"**/engine_{cell}.log") if p.is_file())
    return logs


def _measured_engine_logs(repo: Path, ev: CellEvidence) -> tuple[list[Path], str]:
    """이 측정의 엔진 로그(파일 목록, 근거) — `_measured_engine_logs_ex` 의 앞 두 칸."""
    logs, why, _copies = _measured_engine_logs_ex(repo, ev)
    return logs, why


# ── 같은 실행의 전체 사본(2026-09-29 · FACT_FIX2 G1) ──────────────────────────────────────────────────────────────
# 측정 도구는 엔진 로그를 `tail -800` 으로 잡는다 — 기동 배너 · 노드별 NCCL env 되읊음은 그 창 앞이라 사라진다(DS4F: 엔진 자기보고 ·
#   main/sub env 가 "미관측"). 같은 측정 실행의 **전체 사본**(예 output/multi/benchlog/engine_roce_<셀>.log — 운영자가 따로 보존한 docker logs)
#   이 있으면 원천으로 쓴다. 이름만으로 묶지 않는다 — 같은 셀 id 의 다른 기동 로그일 수 있다. 판별 규칙(셋 모두 · 관측만):
#   ① EngineCore pid 집합이 꼬리 캡처와 같다(비어 있지 않음) ② 가중치 로드 줄(`<MM-DD HH:MM:SS> … Loading weights took <초> seconds`)이
#      시각·초까지 하나 이상 같다(같은 기동의 같은 사건 — pid 만으로는 컨테이너 재기동이 같은 pid 를 줄 수 있다) ③ 사본의 첫 엔진 시각 ≤
#      꼬리의 첫 시각(더 긴 머리를 가진 사본) 이고, 측정 창 시작(관측되면) 이전에 시작했다.
_ENGINE_CORE_PID = re.compile(r"\(EngineCore pid=(\d+)\)")
_LOAD_WEIGHTS = re.compile(r"(\d{2}-\d{2} \d{2}:\d{2}:\d{2})\s+\[[^\]]*\]\s+Loading weights took ([0-9.]+) seconds")
SAME_RUN_RULE = ("EngineCore pid 집합 일치 ∧ 가중치 로드 줄(시각·초) 공유 ∧ 사본 첫 시각 ≤ 꼬리 첫 시각(∧ ≤ 측정 시작)")


def _run_marks(path: Path) -> dict:
    """엔진 로그의 실행 표지 — {pids, loads{(시각, 초)}, first_ts `MM-DD HH:MM:SS`|None}. ANSI 는 벗긴다(사본은 색 코드가 없을 수 있다)."""
    pids: set[str] = set()
    loads: set[tuple[str, str]] = set()
    first = None
    for line in _log_lines(path):
        clean = _ANSI.sub("", line)
        pids.update(_ENGINE_CORE_PID.findall(clean))
        m = _LOAD_WEIGHTS.search(clean)
        if m:
            loads.add((m.group(1), m.group(2)))
        if first is None:
            tm = _ENGINE_TS.search(clean)
            if tm:
                first = f"{tm.group(1)}-{tm.group(2)} {tm.group(3)}:{tm.group(4)}:{tm.group(5)}"
    return {"pids": pids, "loads": loads, "first_ts": first}


def _same_run_candidates(repo: Path, ev: CellEvidence, have: set) -> list[Path]:
    """전체 사본 후보 — 출력 평면 벤치로그 루트의 `*_<셀>.log`(lite 꼬리 · 워치독 제외) + 계보 씨앗 engine_log 중 저장소 안 파일."""
    base = repo / "output" / ev.topology / "benchlog"
    out = [p for p in sorted(base.glob(f"*_{ev.cell}.log")) if p.is_file()] if base.is_dir() else []
    for c in (ev.lineage_seeds or {}).get("evidence_candidates") or []:
        rel = c.get("path") if isinstance(c, dict) and c.get("kind") == "engine_log" else None
        if isinstance(rel, str) and rel and not rel.startswith("/") and ".." not in PurePath(rel).parts:
            out.append(repo / rel)
    seen, res = set(), []
    for p in out:
        if p in seen or p in have or not p.is_file() or p.is_symlink() or not _nonempty(p):
            continue
        seen.add(p)
        if p.name.startswith("lite_engine_") or "watchdog" in p.name or p.parent.name.startswith("level_") \
                or p.parent.name.startswith("run_"):
            continue            # 꼬리 캡처 자리 · 워치독 로그(엔진 출력 아님)
        res.append(p)
    return res


def _mmdd(utc: str | None) -> str | None:
    m = re.fullmatch(r"\d{4}-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})Z", utc or "")
    return f"{m.group(1)}-{m.group(2)} {m.group(3)}:{m.group(4)}:{m.group(5)}" if m else None


def _same_run_copies(repo: Path, ev: CellEvidence, tails: list[Path]) -> dict[Path, str]:
    """꼬리 캡처 로그(tails)와 **같은 실행**으로 판별된 전체 사본 → 판별 근거 문장(SAME_RUN_RULE). 판별 불가는 싣지 않는다."""
    tmarks = [(t, _run_marks(t)) for t in tails if _nonempty(t)]
    tmarks = [(t, m) for t, m in tmarks if m["pids"]]
    if not tmarks:
        return {}
    start = None
    try:
        start = _mmdd(_measure_window(repo, ev)[0])
    except core.HintError:
        start = None
    out: dict[Path, str] = {}
    for cand in _same_run_candidates(repo, ev, set(tails)):
        cm = _run_marks(cand)
        if not cm["pids"] or not cm["first_ts"]:
            continue
        for t, tm in tmarks:
            shared = sorted(cm["loads"] & tm["loads"])
            if cm["pids"] != tm["pids"] or not shared or not tm["first_ts"] or cm["first_ts"] > tm["first_ts"]:
                continue
            if start and cm["first_ts"] > start:
                continue
            out[cand] = (f"같은 실행의 전체 사본 — 판별: EngineCore pid {sorted(cm['pids'])} 일치 · 가중치 로드 줄 "
                         f"`{shared[0][0]} … {shared[0][1]} seconds` 공유 · 사본 첫 시각 {cm['first_ts']} ≤ 꼬리 첫 시각 {tm['first_ts']}"
                         + (f" ≤ 측정 시작 {start}" if start else "") + f"(꼬리 캡처 {_rel(repo, t)} · 규칙 {SAME_RUN_RULE})")
            break
    return out


def _measured_engine_logs_ex(repo: Path, ev: CellEvidence) -> tuple[list[Path], str, dict[Path, str]]:
    """이 측정의 엔진 로그(파일 목록, 근거, {같은 실행 전체 사본: 판별 근거}). 스윕이 출력 평면에 묶였을 때(generated_utc = measured_utc)만
    그 디렉터리의 로그가 이 측정의 것이다 · lite 셀은 lite_raw 조인(measured_utc = 리포트 생성일)이 설 때 벤치로그 루트의 lite 엔진 로그.
    G1: 꼬리 캡처 로그와 같은 실행으로 판별된 전체 사본을 **앞에** 둔다(env 되읊음 · 배너의 첫 관측이 창 밖이 아니게)."""
    logs, why = _measured_engine_logs_base(repo, ev)
    if not logs:
        return logs, why, {}
    copies = _same_run_copies(repo, ev, [q for q in logs if q not in set(_native_preserved_engine_logs(repo, ev))])
    if copies:
        why += " + " + " · ".join(f"{_rel(repo, p)}({b})" for p, b in copies.items())
        logs = list(copies) + logs
    return logs, why, copies


def _measured_engine_logs_base(repo: Path, ev: CellEvidence) -> tuple[list[Path], str]:
    sw = ev.sweep or {}
    if sw.get("binding") == "output":
        base = repo / sw["dir"]
        logs = _sweep_engine_logs(base, sw.get("index") or {}, sw.get("levels") or [], ev.cell)
        why = f"{sw['dir']}(스윕 generated_utc = measured_utc · 색인이 기록한 레그만)"
        nat = _native_preserved_engine_logs(repo, ev)
        if nat:
            # F9(2026-09-29 · plan_26092908 §4.5): native 스윕의 엔진 로그 자리는 비어 있다(0 바이트 — 측정 도구가 docker logs 로 잡는
            #   자리) — 이 run 의 엔진 출력은 native 정문이 보존한 serve 로그(serve proof evidence_dir)에 있다.
            logs = [q for q in logs if _nonempty(q)] + nat
            why += f" + native serve proof evidence_dir 보존 엔진 로그({', '.join(_rel(repo, q) for q in nat)} · 이 run 의 vllm serve 전체 출력)"
        return logs, why
    if not sw and ev.report_kind == "lite":
        d = repo / "output" / ev.topology / "benchlog"
        raw = _read_json_opt(d / f"lite_raw_{ev.cell}.json", "lite_raw")
        born = _report_born_utc(repo, ev.bench_report_path)
        if isinstance(raw, dict) and born and raw.get("config_name") == ev.cell and raw.get("measured_utc") == born \
                and (d / f"lite_engine_{ev.cell}.log").is_file():
            return [d / f"lite_engine_{ev.cell}.log"], "lite_raw 조인(measured_utc = 리포트 생성일)"
        return [], "lite 셀 — lite_raw 조인 불성립 또는 엔진 로그 부재"
    return [], (f"스윕이 출력 평면에 묶이지 않았다(binding={sw.get('binding')!r}) — 그 로그는 이 측정의 것이 아니다" if sw
                else "측정 스윕 없음")


def _nonempty(p: Path) -> bool:
    try:
        return p.stat().st_size > 0
    except OSError:
        return False


def _native_preserved_engine_logs(repo: Path, ev: CellEvidence) -> list[Path]:
    """native 셀 serve proof `evidence_dir`(native 정문이 삭제 전에 보존한 저장소 상대 디렉터리)의 `logs/*vllm-serve*.log`(비어 있지 않은 것).
    native 가 아니거나 경로가 저장소 상대가 아니면 [] — 호스트 절대경로 · 저장소 밖은 읽지 않는다."""
    if ev.plane != "native" or not isinstance(ev.serve_proof, dict):
        return []
    raw = ev.serve_proof.get("evidence_dir")
    if not isinstance(raw, str) or not raw.strip() or raw.startswith("/") or ".." in PurePath(raw).parts:
        return []
    d = repo / raw.strip() / "logs"
    try:
        d.resolve().relative_to(repo.resolve())
    except ValueError:
        return []
    return sorted(q for q in d.glob("*vllm-serve*.log") if q.is_file() and _nonempty(q)) if d.is_dir() else []


def _log_lines(path: Path) -> list[str]:
    try:
        # 바이트로 읽는다 — `\r`(tqdm)을 줄바꿈으로 읽으면 `파일:줄` 이 grep·편집기 줄 번호와 갈라진다(round 2 와 같은 규율).
        return path.read_bytes().decode("utf-8", errors="replace").split("\n")
    except OSError as e:
        core.fail("HINT_EVIDENCE_UNREADABLE", f"엔진 로그를 읽을 수 없다({path.name}): {type(e).__name__}: {e}")


def _node_of_line(line: str, table) -> tuple[str | None, str | None]:
    """로그 줄의 호스트(`<host>:<pid>:<tid> [n] NCCL INFO`) · Ray 접두 `ip=` → manifest 치환표(pii.substitution_table)의 노드 표지.
    (main|sub|None, 표지 라벨). 치환표가 모르는 이름은 None — 노드를 추측하지 않는다."""
    from . import pii
    for rx in (_NCCL_HOST_RE, _RAY_IP_RE):
        m = rx.search(line)
        if not m:
            continue
        pm = _NODE_PH_RE.fullmatch(pii.substitute(m.group(1), table, residual=False))
        if pm:
            lab = pm.group(1)
            return (lab if lab in ("main", "sub") else None), lab
    return None, None


# 측정 도구가 엔진 로그를 **꼬리만** 잡는 줄(`docker logs … | tail -N > "$ELOG"` · 측정 시점 판본에서 N 을 읽는다 · tripwire).
_TAIL_CAP_RE = re.compile(r'\btail\s+-(?:n\s*)?(\d+)\s*>\s*"\$ELOG"')


def _log_tail_caps(repo: Path, ev: CellEvidence) -> dict[str, tuple[int, str]]:
    """{역할(lite|bench): (N, 출처 '<도구>@<rev12> L<줄>')} — 측정 시점 판본의 lite_bench.sh·run_bench.sh 가 엔진 로그를 `tail -N` 으로
    잘라 쓰는가(2026-09-22 · S2 round 3 적대 검토). 판본을 못 고르거나 그 줄이 없으면 그 역할은 빠진다(상한 미관측 — 지어내지 않는다)."""
    sel = _tool_selection(repo, ev)
    out: dict[str, tuple[int, str]] = {}
    for t in sel["tools"]:
        if t["role"] not in ("lite", "bench") or not sel.get("at"):
            continue
        raw = _git_text(repo, sel["at"], t["repo_path"])
        rv = _tool_revision(repo, sel["at"], t["repo_path"]) or {}
        for i, ln in enumerate((raw or b"").decode("utf-8", errors="replace").split("\n"), 1):
            m = _TAIL_CAP_RE.search(ln)
            if m and not ln.lstrip().startswith("#"):
                out[t["role"]] = (int(m.group(1)), f"{t['name']}@{str(rv.get('git_rev') or sel['at'])[:12]} L{i}")
                break
    return out


def measurement_env_observed(repo, ev: CellEvidence) -> list[dict]:
    """이 측정의 엔진 로그가 되읊은 env · NCCL 런타임 관측(공유 계약 `facts['measurement_env_observed']`).
    반환 [{key, value, node(main|sub|None), source('<로그>:L<줄>' — 공유 계약 표기), kind(env-echo|param-echo|runtime), seen_in_logs(그 값이 나온 캡처
    로그 수 — 캡처는 같은 엔진 출력의 겹치는 꼬리라 사건 수가 아니다), ray_repeated?(출처 줄의 `[repeated Nx across cluster]` N · 접힌
    사본의 노드는 미관측), value_note?, node_label?}] — (kind·key·value·node) 마다 첫 줄 하나(로그 순 · 줄 순). 값·장치·호스트 이름은 발췌와 같은 기계 치환
    (pii.substitution_table · manifest 파생 · 4종 잔여 가림)을 거친다(배포 평면의 최종 권위는 finalize 4종 스캔).
    ★ 이 줄들은 **측정 당시 프로세스가 본 값**이다(재생성 env 형상 대신 사실로 쓴다). NCCL 이 되읊지 않는 키는 여기 없다 — 없음 ≠ 미설정.
    log_window?(2026-09-22 적대 검토): 출처 로그가 측정 도구의 `tail -N` 상한에 닿은(줄 수 ≥ N) 꼬리 캡처면 `tail-<N>(<도구>@<rev12>
    L<줄>)` — 그 창 **앞**의 줄은 관측 대상 밖이다. 태그2 실측: 측정 로그 6개가 전부 800줄(run_bench.sh·lite_bench.sh 의
    `tail -800`)이라 한 노드만 보인 키(예: NCCL_NET_PLUGIN = sub 만)는 "다른 노드가 되읊지 않았다" 가 아니라 창 밖일 수 있다."""
    from . import pii
    repo = _repo(repo)
    logs, _why, copies = _measured_engine_logs_ex(repo, ev)
    if not logs:
        return []
    table = pii.substitution_table(repo, ev.manifest if isinstance(ev.manifest, dict) else None)
    caps = _log_tail_caps(repo, ev)
    windows: dict[str, str] = {}
    # F9: native 보존 로그 · G1: 같은 실행의 전체 사본은 꼬리 캡처가 아니다(전체 출력 — 줄 수가 상한 이상이어도 창 표지 ✗)
    full = set(_native_preserved_engine_logs(repo, ev)) | set(copies)
    for lp in logs:
        if lp in full:
            continue
        cap = caps.get("lite" if lp.name.startswith("lite_engine_") else "bench")
        try:
            n_lines = lp.read_bytes().count(b"\n")
        except OSError:
            n_lines = -1
        if cap and n_lines >= cap[0]:
            windows[_rel(repo, lp)] = f"tail-{cap[0]}({cap[1]})"
    rows: dict[tuple, dict] = {}
    seen: dict[tuple, set] = {}
    order: list[tuple] = []
    for lp in logs:
        rel = _rel(repo, lp)
        for i, line in enumerate(_log_lines(lp), 1):
            if "NCCL INFO" not in line:
                continue
            # 줄 번호는 `\n` 기준(_log_lines) — `\r` 진행줄이 섞여도 정규식은 줄 어디서든 찾으므로 쪼개지 않는다.
            clean = _ANSI.sub("", line).rstrip()
            dm = _RAY_DEDUP_RE.search(clean)
            dedup = int(dm.group(1)) if dm else 0
            body = clean[:dm.start()] if dm else clean
            hit = None
            for kind, rx in _ENV_ECHO_RES:
                m = rx.search(body)
                if m:
                    val = m.group(2)
                    if re.fullmatch(r"-?\d+\.", val):     # NCCL 정수 파라미터 문구는 끝에 마침표를 붙인다(`… to 0.`)
                        val = val[:-1]
                    hit = (kind, m.group(1), val)
                    break
            if hit is None:
                for key, rx in _ENV_RUNTIME_RES:
                    m = rx.search(body)
                    if m:
                        hit = ("runtime", key, m.group(1).strip())
                        break
            if hit is None:
                continue
            kind, key, raw_val = hit
            node, label = _node_of_line(body, table)
            # 값·장치·호스트·주소는 발췌와 **같은** 치환(표 + 4종 잔여 가림)을 거친다 — 배포 게이트(finalize 4종 스캔)가 최종 권위이고,
            #   이 치환은 그 게이트가 한 칸 값 때문에 발행 전체를 막지 않게 하는 자리다(round 2 detail 값 필터와 같은 결).
            val = pii.substitute(raw_val, table)
            k = (kind, key, val, node)
            if k in rows:
                seen[k].add(rel)
                rows[k]["seen_in_logs"] = len(seen[k])
                continue
            seen[k] = {rel}
            row = {"key": key, "value": val, "node": node, "source": f"{rel}:L{i}", "kind": kind, "seen_in_logs": 1}
            if rel in windows:
                row["log_window"] = windows[rel]
            if dedup:
                # Ray 가 같은 줄을 다른 프로세스에서 N 번 더 받아 한 줄로 접었다 — 그 사본들의 노드는 로그에 없다(관측 대상 밖).
                row["ray_repeated"] = dedup
            if val != raw_val:
                row["value_note"] = "기계 치환(pii.substitution_table · 장치·호스트·주소는 자리표시 — 원문은 출처 줄)"
            if node is None and label:
                row["node_label"] = label
            rows[k] = row
            order.append(k)
    return [rows[k] for k in order]


def _banner_of(path: Path) -> tuple[int, str, str | None] | None:
    """엔진 로그의 첫 vLLM 기동 배너 → (줄, 버전 원문, 컨테이너 시각 `MM-DD HH:MM:SS`|None). 없으면 None."""
    for i, line in enumerate(_log_lines(path), 1):
        for seg in line.split("\r"):
            clean = _ANSI.sub("", seg)
            for rx in _BANNER_RES:
                m = rx.search(clean)
                if m:
                    tm = _ENGINE_TS.search(clean)
                    return i, m.group(1), (f"{tm.group(1)}-{tm.group(2)} {tm.group(3)}:{tm.group(4)}:{tm.group(5)}" if tm else None)
    return None


def _read_json_soft(path: Path):
    """이웃 산출물(다른 셀 스윕 색인 · 과거 발행의 simlog 색인) 읽기 — 깨진 파일은 **이 셀의 발행을 막지 않고** 건너뛴다(그 파일은 이 측정의
    증거가 아니라 보조 대조 입력이다 · 이 측정 자신의 증거는 _read_json_opt 가 fail-closed 로 읽는다). 반환 (doc|None, 사유|None)."""
    try:
        return _read_json_opt(path, path.name), None
    except core.HintError as e:
        return None, f"{path.name} 판독 불가({e.code})"


def _digest_sweeps(repo: Path, ev: CellEvidence, digest: str | None, tag: str | None) -> list[dict]:
    """출력 평면 스윕 중 **측정한** digest(와 태그)가 이 측정과 같은 것 [{dir, utc, idx, levels}](utc 순). meta 의 `*_source` 가
    `measured` 로 시작하는 값만 믿는다(선언 태그 ✗)."""
    out = []
    if not digest:
        return out
    for p in sorted((repo / "output" / ev.topology / "benchlog").glob("sweep_*/sweep_index.json")):
        idx, _bad = _read_json_soft(p)
        if not isinstance(idx, dict):
            continue
        meta = idx.get("meta") if isinstance(idx.get("meta"), dict) else {}
        if meta.get("image_digest") != digest or not str(meta.get("image_digest_source", "")).startswith("measured"):
            continue
        tag_ok = tag is None or (meta.get("image_tag") == tag and str(meta.get("image_tag_source", "")).startswith("measured"))
        if isinstance(idx.get("generated_utc"), str) and _UTC.fullmatch(idx["generated_utc"]):
            out.append({"dir": p.parent, "utc": idx["generated_utc"], "idx": idx, "tag_ok": tag_ok,
                        "levels": idx.get("levels") if isinstance(idx.get("levels"), list) else []})
    return sorted(out, key=lambda s: (s["utc"], str(s["dir"])))


def _engine_banner(repo: Path, ev: CellEvidence) -> dict:
    """엔진 자기보고(기동 배너) — {value, source, basis, files}. basis: `this-measurement`(이 측정의 스윕 meta.vllm_build · 엔진 로그) ·
    `same-digest-recorded`(측정 digest 를 **기록한** 다른 스윕의 로그) · `same-digest-inferred`(digest 를 기록하지 않은 serve_fail 로그 —
    같은 IMAGE_TAG 이고 그 시각 앞뒤의 스윕이 같은 태그→digest 를 측정했을 때만 · 추론이라 적는다) · `conflict` · `unobserved`.
    값이 둘 이상이면 고르지 않는다(conflict)."""
    sw = ev.sweep or {}
    meta = (sw.get("index") or {}).get("meta") or {}
    mm = _measured_meta(ev.sweep, ev.certificate)
    digest = mm.get("image_digest") or (ev.build_identity or {}).get("image_digest")
    tag = mm.get("image_tag") if str(mm.get("image_tag_source", "")).startswith("sweep_index.meta.image_tag(measured") else None
    measured = _measured_key(repo, ev)
    # ① 이 측정 — sweep_bench 가 레벨 엔진 로그 배너에서 잡은 meta.vllm_build · 없으면 이 측정 로그를 직접 훑는다
    if sw.get("binding") == "output" and not _unobserved(meta.get("vllm_build")):
        ip = repo / sw["dir"] / "sweep_index.json"
        ln = _json_key_line(ip, "vllm_build", meta["vllm_build"])
        rel = _rel(repo, ip)
        return {"value": str(meta["vllm_build"]), "basis": "this-measurement", "files": [(rel, "sweep_json")],
                "source": (f"{rel}:{ln or '?'} meta.vllm_build(sweep_bench 가 이 측정의 레벨 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡은 값 — "
                           "배너 전용 규칙은 아니다)")}
    logs_, _w, copies = _measured_engine_logs_ex(repo, ev)
    for lp in logs_:
        b = _banner_of(lp)
        if b:
            rel = _rel(repo, lp)
            return {"value": b[1], "basis": "this-measurement", "files": [(rel, "engine_log")],
                    "source": f"{rel}:{b[0]}(이 측정의 엔진 로그 기동 배너" + (f" · {copies[lp]}" if lp in copies else "") + ")"}
    # tag 를 넘겨야 `tag_ok` 가 뜻을 갖는다(2026-09-22 · S2 round 3 적대 검토): 옛 판본은 None 을 넘겨 모든 스윕이 tag_ok=True 였고,
    #   ③ 괄호의 "앞뒤 스윕이 **같은 태그**→digest 를 측정" 조건이 죽은 코드였다 — 다른 태그로 같은 digest 를 잰 스윕도 괄호가 됐다.
    #   ② 는 digest 만 본다(tag_ok 로 거르지 않는다 — 같은 digest 를 기록한 로그는 태그와 무관하게 같은 이미지다).
    sweeps = _digest_sweeps(repo, ev, digest, tag)
    this_dir = (repo / sw["dir"]).resolve() if sw.get("dir") else None
    # ② 같은 digest 를 기록한 다른 스윕
    rec = []
    for s in sweeps:
        if this_dir is not None and s["dir"].resolve() == this_dir:
            continue
        smeta = s["idx"].get("meta") or {}
        if not _unobserved(smeta.get("vllm_build")):
            ip = s["dir"] / "sweep_index.json"
            rel = _rel(repo, ip)
            rec.append((s["utc"], str(smeta["vllm_build"]), f"{rel}:{_json_key_line(ip, 'vllm_build', smeta['vllm_build']) or '?'} "
                        f"meta.vllm_build", [(rel, "sweep_json")]))
            continue
        for lp in _sweep_engine_logs(s["dir"], s["idx"], s["levels"], str(smeta.get("config_name") or s["idx"].get("config") or "")):
            b = _banner_of(lp)
            if b:
                rel = _rel(repo, lp)
                rec.append((s["utc"], b[1], f"{rel}:{b[0]}", [(rel, "engine_log")]))
                break
    if rec:
        return _pick_banner(rec, measured, "same-digest-recorded",
                            f"측정 digest {str(digest)[:19]}… 를 기록한(sweep_index.meta.image_digest · measured) 다른 스윕의 엔진 로그")
    # ③ digest 미기록 serve_fail 로그 — 같은 태그 + 앞뒤 측정 스윕 괄호(추론)
    brackets = [s for s in sweeps if s["tag_ok"]] if tag else []
    inf = []
    if brackets:
        for d in sorted((repo / "output" / ev.topology / "benchlog").glob("serve_fail_*")):
            cfg = d.name[len("serve_fail_"):]
            env_rel = f"output/{ev.topology}/envs/.env.{cfg}"
            if not d.is_dir() or not _CELL_RE.match(cfg) or not (repo / env_rel).is_file() \
                    or _env_first(repo, env_rel).get("IMAGE_TAG") != tag:
                continue
            for lp in sorted(d.glob("*.log")):
                b = _banner_of(lp)
                if not b or not b[2]:
                    continue
                t = _bracketed(b[2], brackets, measured)
                if t is None:
                    continue
                t_utc, s1, s2 = t
                rel = _rel(repo, lp)
                inf.append((t_utc, b[1], (f"{rel}:{b[0]}(컨테이너 시계 {b[2]} → {t_utc} UTC 가정) — digest 미기록 로그: {env_rel} "
                                          f"IMAGE_TAG(현재 파일 값)={tag} · 앞뒤 스윕 {_rel(repo, s1['dir'])}({s1['utc']}) · "
                                          f"{_rel(repo, s2['dir'])}({s2['utc']}) 가 같은 태그→digest {str(digest)[:19]}… 를 측정했다 "
                                          "(그 사이 재태깅은 관측 대상 밖) → 같은 digest 로 **추론**(기록 아님)"),
                            [(rel, "engine_log")]))
    if inf:
        return _pick_banner(inf, measured, "same-digest-inferred", "digest 를 기록하지 않은 기동 실패 로그의 배너")
    why = ("측정 digest 미관측" if not digest else
           "이 측정 로그는 꼬리 캡처라 배너가 없고(sweep meta.vllm_build 도 NA) 같은 digest 를 기록·추론할 수 있는 다른 로그에도 배너가 없다")
    return {"value": None, "basis": "unobserved", "files": [], "source": f"미관측 — {why}"}


def _bracketed(mmdd_hms: str, brackets: list[dict], measured: str | None) -> tuple[str, dict, dict] | None:
    """컨테이너 시각 `MM-DD HH:MM:SS`(연도 없음 · UTC 가정) → 앞뒤 스윕 괄호 (utc, 앞, 뒤) | None. 연도는 측정 연도, 안 되면 그 전해."""
    base = _utc_dt(measured) or _utc_dt(brackets[-1]["utc"])
    m = re.fullmatch(r"(\d{2})-(\d{2}) (\d{2}):(\d{2}):(\d{2})", mmdd_hms)
    if base is None or not m:
        return None
    for year in (base.year, base.year - 1):
        try:
            t = _dt.datetime(year, *(int(x) for x in m.groups())).strftime("%Y-%m-%dT%H:%M:%SZ")
        except ValueError:
            continue
        before = [s for s in brackets if s["utc"] <= t]
        after = [s for s in brackets if s["utc"] >= t]
        if before and after:
            return t, before[-1], after[0]
    return None


def _pick_banner(cands: list[tuple], measured: str | None, basis: str, what: str) -> dict:
    """후보 [(utc, 값, 출처, files)] → 값이 하나면 측정 시각에 가장 가까운 출처(동률은 출처 사전순) · 둘 이상이면 conflict(고르지 않는다)."""
    vals = sorted({c[1] for c in cands})
    if len(vals) > 1:
        by = {v: min((c for c in cands if c[1] == v), key=lambda c: c[2])[2] for v in vals}
        return {"value": None, "basis": "conflict", "files": [f for c in cands for f in c[3]],
                "source": f"{what} 이 서로 다른 버전을 말한다 — 고르지 않는다: " + " · ".join(f"{v} ← {s}" for v, s in by.items())}
    md = _utc_dt(measured)

    def dist(c):
        cd = _utc_dt(c[0])
        return (abs((cd - md).total_seconds()) if cd and md else 0, c[2])
    best = min(cands, key=dist)
    more = len(cands) - 1
    return {"value": vals[0], "basis": basis, "files": list(best[3]),
            "source": f"{best[2]} — {what}" + (f" · 같은 값의 다른 로그 {more}건" if more else "")}


def _cert_vllm_producer(repo: Path, ev: CellEvidence, sel: dict | None = None) -> tuple[str, str]:
    """인증서 vllm_version 의 producer → (라벨, 출처 서술). 측정 시점 sweep_bench.sh 판본의 파생 규칙(_CERT_VLLM_MARKS 표지)을 찾아 그
    판본의 정규식 리터럴을 스윕 meta.image_tag_declared(= 그 규칙의 입력 `grep_env(envtext, "IMAGE_TAG")`)에 **그대로** 적용해 인증서
    값과 대조한다. 표지를 못 찾거나 값이 어긋나면 '미검증'(지어내지 않는다)."""
    cert_v = _norm_value((ev.certificate or {}).get("vllm_version"))
    meta = ((ev.sweep or {}).get("index") or {}).get("meta") or {}
    meta_v = _norm_value(meta.get("vllm_version"))
    if not cert_v:
        return "absent", "인증서 vllm_version 없음"
    sel = sel or _tool_selection(repo, ev)
    rel = f"{REL_BENCH_TOOLS_DIR}/sweep_bench.sh"
    raw = _git_text(repo, sel["at"], rel) if sel.get("at") else None
    if raw is None:
        return "unverified", f"미검증 — 측정 시점 sweep_bench.sh 판본 미관측({sel.get('at_method')})"
    lines = raw.decode("utf-8", errors="replace").split("\n")
    rev = (_tool_revision(repo, sel["at"], rel) or {}).get("git_rev") or sel["at"]
    at = {k: next((i for i, ln in enumerate(lines, 1) if rx.search(ln)), None) for k, rx in _CERT_VLLM_MARKS.items()}
    if not at["image_rule"] or not at["image_src"]:
        return "unverified", (f"미검증 — sweep_bench.sh@{rev[:12]} 에서 IMAGE_TAG 파생 규칙 표지를 찾지 못했다(판본이 다른 규칙을 쓴다 · "
                              "tripwire)")
    pat = _CERT_VLLM_MARKS["image_rule"].search(lines[at["image_rule"] - 1]).group(1)
    declared = _norm_value(meta.get("image_tag_declared"))
    try:
        mm = re.search(pat, declared or "")
    except re.error:
        mm = None
    cand = mm.group(1) if mm and mm.groups() else None
    env_txt = (f"더 높은 우선순위의 덮어쓰기(`--vllm-version` 인자 → VLLM_VER · EASY_VLLM_VERSION env · L{at['env_override']})는 측정 "
               "기록에 남지 않아 배제할 수 없다" if at["env_override"] else "env 덮어쓰기 표지 없음")
    if cand and cand == cert_v:
        same = "= sweep_index.meta.vllm_version(인증서는 그 옮김)" if meta_v == cert_v else f"(meta.vllm_version={meta_v!r} 와 다름)"
        return "image-tag-line", (f"sweep_bench.sh@{rev[:12]} L{at['image_src']}·L{at['image_rule']}: 선언 IMAGE_TAG "
                                  f"(sweep_index.meta.image_tag_declared {declared})에 그 판본의 정규식 `{pat}` → {cand} {same} — "
                                  f"**엔진 자기보고가 아니다**(빌드 입력 이미지 태그를 x.y.z 로 자른 값 · rc/dev 접미가 사라진다). {env_txt}")
    build = _norm_value(meta.get("vllm_build"))
    # 배너 접두 규칙은 **이미지 태그 규칙이 값을 내지 못했을 때만** 돈다(그 판본의 우선순위 — `if not vllm and vllm_build != "NA"`).
    #   2026-09-22 적대 검토: 옛 판본은 태그 규칙이 다른 값을 냈어도(= 덮어쓰기 신호) 배너 접두가 맞으면 이 라벨을 붙였고, 접두 비교도
    #   startswith 라 `0.9.0` 이 `0.9.01…` 에 맞았다 — 그 판본의 x.y.z 추출을 그대로 적용한다.
    bm = re.match(r"([0-9]+\.[0-9]+\.[0-9]+)", build or "")
    if cand is None and bm and bm.group(1) == cert_v and at["banner"]:
        return "engine-banner-prefix", (f"sweep_bench.sh@{rev[:12]} L{at['banner']}: 이미지 태그 규칙이 값을 내지 못해 엔진 로그 배너 "
                                        f"{build} 의 x.y.z 접두를 썼다 · {env_txt}")
    return "unverified", (f"미검증 — sweep_bench.sh@{rev[:12]} 이미지 태그 규칙 결과 {cand!r} ≠ 인증서 {cert_v!r} · 배너 접두도 아니다 "
                          f"({env_txt} — 덮어쓰기였을 수 있다)")


def vllm_observed(ev: CellEvidence, repo=None) -> dict:
    """PAYLOAD.naming.vllm_observed — vLLM 버전 관측 **라벨**(이름 입력 ✗ · 00 사실 블록 전용 · X18).
    2026-09-22(S2 round 3): 옛 판본은 인증서 vllm_version(0.29.0)을 `engine_self_report` 에 실었다 — 그 값은 측정 시점 sweep_bench.sh
    가 IMAGE_TAG 에서 x.y.z 로 자른 **빌드 입력 태그**라 사후 사실확인이 wrong 으로 판정했다. 이제 칸마다 producer 를 따로 적는다:
      engine_self_report         엔진 기동 배너(이 측정 → 같은 digest 를 기록한 다른 로그 → digest 미기록이지만 괄호로 추론한 로그 순) ·
                                 `_basis` 가 그 지위를 말한다(추론은 추론이라 적는다)
      certificate_vllm_version   인증서 강한 키 + `_producer`(측정 시점 sweep_bench.sh 판본에서 검증한 파생 규칙) · `_source`
      wheel_meta                 wheel 배포 메타 — 측정 경로에 관측자가 없다(sweep meta.vllm_build 는 엔진 로그 배너 정규식이지 wheel 메타가 아니다)
    `source` 는 옛 소비자용 요약(칸별 출처를 한 줄로)."""
    # 저장소를 cwd 로 대신하지 않는다(2026-09-22 적대 검토 — 옛 `ev.repo or "."` 는 결정 경로의 침묵 폴백이었다: 다른 체크아웃의 로그·git 을
    #   읽고도 출처는 이 저장소처럼 적었다). 해소기(_assemble)가 ev.repo 를 채우므로 둘 다 없으면 호출 결함이다.
    if repo is None and not ev.repo:
        core.fail("HINT_EVIDENCE_REPO_REQUIRED", "vllm_observed 에 저장소가 없다(repo 인자 · ev.repo 둘 다 없음).",
                  "evidence.from_campaign/from_publication 이 만든 CellEvidence 를 넘기거나 repo 를 준다.")
    rp = _repo(repo if repo is not None else ev.repo)
    banner = _engine_banner(rp, ev)
    label, psrc = _cert_vllm_producer(rp, ev)
    cert_v = _norm_value((ev.certificate or {}).get("vllm_version"))
    meta = ((ev.sweep or {}).get("index") or {}).get("meta") or {}
    wheel, wheel_src = _wheel_meta(rp, ev)
    if wheel is None:
        wheel_src += ("(sweep_index.meta.vllm_build="
                      f"{meta.get('vllm_build')!r} 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천)")
    return {"engine_self_report": banner["value"], "engine_self_report_source": banner["source"],
            "engine_self_report_basis": banner["basis"],
            "certificate_vllm_version": cert_v, "certificate_vllm_version_producer": label,
            "certificate_vllm_version_source": psrc,
            "wheel_meta": wheel, "wheel_meta_source": wheel_src,
            # 옛 소비자(칸 하나의 source)용 요약 — 칸별 서술을 다시 적지 않는다(같은 바이트 두 벌 ✗ · 정본은 `*_source`).
            "source": (f"칸별 출처는 *_source — engine_self_report={banner['basis']} · certificate_vllm_version={label}"
                       + ("(빌드 입력 태그 파생 · 엔진 자기보고 아님)" if label == "image-tag-line" else "")
                       + f" · wheel_meta={'관측' if wheel else '미관측'}")}


def _wheel_meta(repo: Path, ev: CellEvidence) -> tuple[str | None, str]:
    """설치된 vLLM 배포 메타 버전(`importlib.metadata.version('vllm')` = wheel METADATA) — 관측 원천 둘(2026-09-29 · plan_26092908 §4.5
    "미관측 오판"): ① 노드 정합 attestation `nodes.<n>.vllm_dist_version`(multinode_serve_smoke 가 실행 중 컨테이너에서 캡처)
    ② native serve proof 가 보존한 pip freeze 의 `vllm==` 줄(노드별). 옛 판본은 attestation 에 그 칸이 있는데도 "측정 경로에 관측자가
    없다" 고 적었다. 노드끼리 다르면 값을 고르지 않는다(None · 출처가 두 값을 말한다).
    native 셀은 ② 가 먼저다 — 실행 평면이 venv 이고, 묶인 attestation 은 원천 이미지(Docker) 실행의 것이다(다른 평면의 관측)."""
    v, where, by_node = _attested_host(ev.attestation, "vllm_dist_version", _attestation_timing_note(ev))
    if v is not None and ev.plane != "native":
        if len(set(by_node.values())) > 1:
            return None, f"노드별 상이 — 값을 고르지 않는다: {where}"
        return v, where
    freezes = ev.pip_freeze_paths or ({"main": ev.pip_freeze_path} if ev.pip_freeze_path else {})
    got: dict[str, str] = {}
    for n, rel in sorted(freezes.items()):
        p = repo / str(rel)
        if not p.is_file():
            continue
        for ln in p.read_text(encoding="utf-8", errors="replace").splitlines():
            m = re.match(r"^vllm==(\S+)\s*$", ln.strip())
            if m:
                got[n] = m.group(1)
                break
    if got:
        where = " · ".join(f"{n}={freezes[n]}" for n in sorted(got))
        if len(set(got.values())) > 1:
            return None, f"native pip freeze `vllm==` 노드별 상이({where}) — 값을 고르지 않는다"
        return next(iter(got.values())), f"native pip freeze `vllm==` 줄({where} · 노드별 관측 · 일치)"
    if v is not None and len(set(by_node.values())) == 1:
        return v, where + " · ⚠ native 셀인데 pip freeze 에 vllm 줄이 없어 원천 이미지 실행의 관측을 쓴다"
    return None, "미관측 — attestation vllm_dist_version · native pip freeze 모두 없다"


_ATTEST_TIME_KEYS = ("written_utc", "captured_utc", "generated_utc", "utc")
ATTESTATION_SCOPE_OTHER_RUN = "image-build/smoke — not this cell's run"


def attestation_scope(repo, ev: CellEvidence) -> dict | None:
    """묶인 노드 정합 attestation 의 **범위** 라벨(공유 계약 `facts['attestation_scope']`) — {config, written_utc, written_utc_source,
    bound_by, phase, scope}. 2차 저작자 보고: 태그2 는 셀 이름 파일이 없어 이미지 compose 라벨의 빌드 config(`q38fn-nvfp4-mmap` · 09-09
    작성)로 묶였는데 FACT 가 그것을 이 셀의 ABI 정합 출처처럼 인용했다(귀속 오기 위험). config 가 셀과 다르면 scope =
    ATTESTATION_SCOPE_OTHER_RUN · 같으면 "같은 config 의 {phase} 기록 — 이 측정 실행과 같은 실행인지는 미검증(파일은 다음 실행이 덮는다)".
    written_utc = 문서의 시각 칸(있으면) · 없으면 파일 mtime(파일시스템 관측 — 기록 필드 아님 · 출처에 그렇게 적는다). 묶인 것이 없으면 None."""
    repo = _repo(repo)
    att = ev.attestation
    if not isinstance(att, dict) or not att.get("_path"):
        return None
    cfg = att.get("config")
    written, wsrc = _attestation_written(repo, att)
    phase = att.get("phase") or ("build(v1 · --build 대조 행)" if att.get("checks") else "미기재")
    timing, tnote, win = _attestation_timing(repo, ev, written)
    if cfg != ev.cell:
        scope = ATTESTATION_SCOPE_OTHER_RUN
    elif timing == "after-measurement":
        scope = f"same config({cfg}) · {phase} — 측정 뒤 재기동의 관측이다(이 측정 실행의 관측이 아니다 · 파일은 다음 실행이 덮는다)"
    elif timing == "before-measurement" and (sb := attestation_same_boot(repo, ev, written, win)):
        mark = SAME_BOOT_MARK if sb.get("strength") == "strong" else WEAK_SAME_BOOT_MARK
        scope = f"same config({cfg}) · {phase} — {mark}(판별: {sb['basis']})"
    elif timing == "before-measurement":
        scope = (f"same config({cfg}) · {phase} — 측정 전(빌드·스모크)의 관측 · 측정을 서빙한 기동과 같은 실행인지는 미검증"
                 "(파일은 다음 실행이 덮는다)")
    elif timing == "during-measurement":
        scope = f"same config({cfg}) · {phase} — 측정 중의 관측(측정을 서빙한 실행 · 파일은 다음 실행이 덮는다)"
    else:
        scope = f"same config({cfg}) · {phase} — 이 측정 실행과 같은 실행인지는 미검증(파일은 다음 실행이 덮는다)"
    out = {"config": cfg, "written_utc": written, "written_utc_source": wsrc, "bound_by": att.get("_bound_by"),
           "phase": phase, "path": att["_path"], "scope": scope}
    if tnote:
        out.update(timing=timing, timing_note=tnote, measure_window=win)
    return out


def _attestation_written(repo: Path, att: dict) -> tuple[str | None, str]:
    """attestation 작성 시각 = 문서의 시각 칸(있으면) · 없으면 파일 mtime(파일시스템 관측 — 기록 필드 아님 · 출처에 그렇게 적는다)."""
    key = next((k for k in _ATTEST_TIME_KEYS if isinstance(att.get(k), str) and _UTC.fullmatch(att[k])), None)
    if key:
        return att[key], f"attestation {key}"
    try:
        return (_utc_of_epoch(int((repo / att["_path"]).stat().st_mtime)),
                f"{att['_path']} 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)")
    except (OSError, KeyError, TypeError):
        return None, "미관측(시각 칸 없음 · 파일 stat 불가)"


def _attestation_timing_note(ev: CellEvidence) -> str | None:
    """호스트 사실 출처(_attested_host)에 붙일 작성 시각 대 측정 창 문장(F7). 저장소 · attestation 이 없으면 None."""
    att = ev.attestation
    if not isinstance(att, dict) or not att.get("_path") or not ev.repo:
        return None
    repo = _repo(ev.repo)
    written = _attestation_written(repo, att)[0]
    timing, tnote, win = _attestation_timing(repo, ev, written)
    if timing == "before-measurement" and att.get("config") == ev.cell:
        sb = attestation_same_boot(repo, ev, written, win)
        if sb and sb.get("strength") == "strong":
            tnote = f"{tnote} · {SAME_BOOT_MARK}(판별 근거 = attestation 범위 줄 · 스모크 로그 {sb['files'][0]})"
        elif sb:
            tnote = f"{tnote} · {WEAK_SAME_BOOT_MARK}(판별 근거 = attestation 범위 줄 · 원장 {sb['files'][0]})"
    return tnote


# ── attestation 이 측정을 서빙한 **같은 기동**의 것인가(2026-09-29 · FACT_FIX2 G10) ────────────────────────────────────
# D1 은 스모크 로그가 'SMOKE PASS → attestation v2(serve) → --keep-up' 을 순서대로 적었고, 원장의 예산 창은 선언 하나(재선언 0)가 측정 끝까지
#   이어졌으며, 측정 엔진 로그의 APIServer pid 가 하나였는데 FACT 는 "같은 실행인지 미검증" 이라 적었다. 판별 규칙(모두 관측 · 하나라도
#   불성립이면 판별하지 않는다 — 지금처럼 '미검증'):
#   ① 원장: attestation 작성 시각 이전 마지막 `budget_declare`(이 셀 라벨) D 가 있고, 그 노드 원장이 측정 끝까지 기록 범위 안이며
#      (D, 측정 끝] 에 다른 `budget_declare` · 창을 닫는 `budget_clear(existed)` 가 없다(재기동 없음 · 예산 재선언 없음)
#   ② 스모크 로그(docs/simlog/**/*serve*.log): `SMOKE PASS` → 이 attestation 경로 줄 → `--keep-up` 이 이 순서로 있고, 그 로그의 main
#      `budget_honored` ts = D 다(같은 선언의 기동)
#   ③ 측정 엔진 로그의 APIServer pid 가 둘 이상이면 불성립(서빙 프로세스 교체) · 하나면 근거에 싣는다 · 없으면 미관측으로 적는다.
#   2026-09-29 교정: ②의 대조는 원장 `budget_declare` ts 와 스모크 로그 `budget_honored` ts 의 **문자 일치**였다 — 실 워치독은 선언 1초 뒤에
#   honored 를 쓴다(원장 declare 04:54:44Z · watchdog.jsonl honored 04:54:45Z) → 스모크 로그가 있어도 늘 불성립이었다. 이제 스모크 로그
#   honored ts 가 {D ts} ∪ {원장의 같은 라벨 `budget_honored` 중 D 이후 · attestation 작성 이전} 에 들면 같은 선언이다.
#   약한 판별(스모크 로그 미보존 · ② 불성립 · ①③ 성립): attestation · serve_proof(같은 config · 있으면) 작성 시각이 모두 [D, 측정 시작) 안이고,
#   측정 엔진 로그의 API 서버 기동 시각(`Starting vLLM server on` · 컨테이너 시계 = UTC 가정)이 관측되면 D ≤ 기동 ≤ attestation 작성
#   (스모크 PASS 는 서버가 떠 있어야 난다) — 모순이면 판별 ✗. 표지 = WEAK_SAME_BOOT_MARK(강한 판별과 다른 문구 · 근거 문장 필수).
SAME_BOOT_MARK = "측정을 서빙한 같은 기동의 스모크 직후 관측"
WEAK_SAME_BOOT_MARK = "원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)"
_SERVER_STARTING = re.compile(r"Starting vLLM server on ")
_APISERVER_PID = re.compile(r"\(APIServer pid=(\d+)\)")
_HONORED_TS = re.compile(r'budget_honored\b.*?"ts"\s*:\s*"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)"')


def _smoke_serve_logs(repo: Path) -> list[Path]:
    base = repo / "docs" / "simlog"
    if not base.is_dir():
        return []
    out = sorted(set(base.glob("*/*serve*.log")) | set(base.glob("*/*/*serve*.log")))
    return [p for p in out if p.is_file() and not p.is_symlink() and "watchdog" not in p.name]


def _api_server_start(repo: Path, ev: CellEvidence, year_hint: str) -> tuple[str | None, str | None]:
    """측정 엔진 로그의 첫 `Starting vLLM server on` 줄 시각(UTC · 컨테이너 시계 = UTC 가정 · 연도 = year_hint 의 연도) · 출처 `파일:줄`."""
    dt0 = _utc_dt(year_hint)
    for lp in _measured_engine_logs(repo, ev)[0]:
        for i, line in enumerate(_log_lines(lp), 1):
            clean = _ANSI.sub("", line)
            if not _SERVER_STARTING.search(clean):
                continue
            tm = _ENGINE_TS.search(clean)
            if not tm or dt0 is None:
                continue
            mo, d, h, mi, sec = (int(x) for x in tm.groups())
            try:
                u = _dt.datetime(dt0.year, mo, d, h, mi, sec).strftime("%Y-%m-%dT%H:%M:%SZ")
            except ValueError:
                continue
            return u, f"{_rel(repo, lp)}:{i}"
    return None, None


def attestation_same_boot(repo, ev: CellEvidence, written: str | None, win: dict | None) -> dict | None:
    """G10 판별 — {basis, files[], strength: strong|weak} | None(판별 불가). 규칙은 위 주석(①②③ · 약한 판별)."""
    repo = _repo(repo)
    att = ev.attestation if isinstance(ev.attestation, dict) else {}
    end = (win or {}).get("end_utc")
    start = (win or {}).get("start_utc")
    if not written or not end or not att.get("_path") or att.get("config") != ev.cell:
        return None
    label = f"smoke-{ev.cell}"
    decl = None
    for node in _event_node_dirs(repo, ev):
        rows = []
        for f in sorted((repo / REL_EVENTS_ROOT / node / "events").glob("*.jsonl")):
            rows += [(ts, ln, d, _rel(repo, f)) for ts, ln, d in _event_rows(f)[0]]
        rows.sort(key=lambda r: (r[0], r[1]))
        if not rows or rows[-1][0] < end:
            continue                    # 이 노드 원장은 측정 끝까지 기록 범위가 아니다 — 증인 ✗
        ds = [r for r in rows if r[2].get("kind") == "budget_declare" and r[2].get("label") == label and r[0] <= written]
        if not ds:
            continue
        d = ds[-1]
        brk = [r for r in rows if d[0] < r[0] <= end and (r[2].get("kind") == "budget_declare" or
                                                        (r[2].get("kind") == "budget_clear" and r[2].get("existed") is True))]
        if brk:
            return None                 # 측정 끝 전에 재선언 · 창 닫힘 — 같은 기동이 아닐 수 있다
        honored = {r[0] for r in rows if r[2].get("kind") == "budget_honored" and r[2].get("label") == label
                   and d[0] <= r[0] <= written}
        if decl is None or node == "main":
            decl = (node, d, honored)
    if decl is None:
        return None
    node, d, ledger_honored = decl
    pids: set[str] = set()
    for lp in _measured_engine_logs(repo, ev)[0]:
        pids.update(_APISERVER_PID.findall(_ANSI.sub("", lp.read_bytes().decode("utf-8", "replace"))))
    if len(pids) > 1:
        return None                     # 측정 로그에 서빙 프로세스가 둘 — 교체
    pid_txt = "측정 엔진 로그 APIServer pid " + (f"{sorted(pids)[0]} 하나" if pids else "미관측")
    same_decl = {d[0]} | ledger_honored
    hit = None
    seen: list[str] = []                # 이 attestation 경로를 적은 스모크 로그(있는데 ②가 불성립이면 약한 판별로 내려가지 않는다)
    ap = str(att["_path"])
    for lg in _smoke_serve_logs(repo):
        lines = _log_lines(lg)
        if any(ap in x for x in lines):
            seen.append(_rel(repo, lg))
        i_pass = next((i for i, x in enumerate(lines) if "SMOKE PASS" in x), None)
        i_att = next((i for i, x in enumerate(lines) if i_pass is not None and i > i_pass and ap in x), None)
        i_keep = next((i for i, x in enumerate(lines) if i_att is not None and i > i_att and "--keep-up" in x), None)
        honored = {m.group(1) for x in lines for m in [_HONORED_TS.search(x)] if m}
        if i_keep is not None and honored & same_decl:
            hit = (lg, i_pass + 1, i_att + 1, i_keep + 1, sorted(honored & same_decl)[0])
            break
    if hit is not None:
        lg, a, b, c, hts = hit
        basis = (f"스모크 로그 {_rel(repo, lg)} L{a} SMOKE PASS → L{b} attestation 작성 → L{c} --keep-up · 원장 {d[3]}:{d[1]} budget_declare"
                 f"({label} · {d[0]}) 가 측정 끝 {end} 까지 재선언 · 창 닫힘 없이 이어짐({node} 원장 기록 범위 안) · 스모크 로그 budget_honored "
                 f"ts {hts} = 그 선언" + ("" if hts == d[0] else "의 원장 budget_honored") + f" · {pid_txt}"
                 + (f" · 측정 시작 {start}" if start else ""))
        return {"basis": basis, "files": [_rel(repo, lg), d[3]], "strength": "strong"}
    # ── 약한 판별: 원장 순서만 — 스모크 로그가 **없을 때만**(있는데 순서 · 선언 대조가 불성립이면 모순이다 · 판별 ✗) ──
    if seen or not start or not (d[0] <= written < start):
        return None
    times = [f"attestation 작성 {written}"]
    sp = ev.serve_proof if isinstance(ev.serve_proof, dict) else None
    sp_rel = (ev.sources or {}).get("serve_proof")
    if sp is not None and sp.get("config") == ev.cell and isinstance(sp_rel, str) and (repo / sp_rel).is_file():
        spw = _attestation_written(repo, {**sp, "_path": sp_rel})[0]
        if not spw or not (d[0] <= spw < start):
            return None                 # serve_proof 작성 시각이 [선언, 측정 시작) 밖 — 다른 기동의 스모크일 수 있다
        times.append(f"serve_proof 작성 {spw}")
    else:
        times.append("serve_proof 미관측")
    srv, srv_src = _api_server_start(repo, ev, d[0])
    if srv and not (d[0] <= srv <= written):
        return None                     # API 서버 기동이 선언 전이거나 attestation 뒤 — 원장 순서와 모순
    srv_txt = (f"API 서버 기동 {srv}({srv_src} · 컨테이너 시계 = UTC 가정) ∈ [선언, attestation 작성]" if srv
               else "API 서버 기동 시각 미관측(모순 검사 불가)")
    basis = (f"원장 순서로 판별(스모크 로그 미보존 — docs/simlog/**/*serve*.log 에 이 attestation 을 적은 스모크 로그 없음): "
             f"원장 {d[3]}:{d[1]} budget_declare({label} · {d[0]}) ≤ {' · '.join(times)} < 측정 시작 {start} · 그 선언이 측정 끝 {end} 까지 "
             f"재선언 · 창 닫힘 없이 이어짐({node} 원장 기록 범위 안) · {srv_txt} · {pid_txt}")
    return {"basis": basis, "files": [d[3]] + ([srv_src.rsplit(":", 1)[0]] if srv_src else []), "strength": "weak"}


_BENCH_JSON_DATE = re.compile(r"^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})$")
LITE_START_NOTE = "lite_bench.sh 가 cold 레그 직전(부하 직전)에 찍는 측정 **시작** 시각"


def lite_window(repo, ev: CellEvidence) -> dict | None:
    """lite-only 셀(스윕 없음 · 경량 리포트)의 측정 창과 레그(2026-09-29 · FACT_FIX2 G7 · G9). 조인 = 벤치로그 루트 `lite_raw_<셀>.json` 의
    config_name · measured_utc = 리포트 머리 생성일(qualification · _measured_engine_logs 와 같은 키). ★ measured_utc 는 측정 **시작**이다
    (lite_bench.sh `MEASURED_UTC=$(date -u …)` 가 cold 레그 직전) — 옛 판은 '측정 끝' 으로 적었다. 끝 = 레그 결과 JSON `date` 중 가장 늦은 것
    (컨테이너 시계 = UTC 가정 · 시작보다 앞이면 가정 불성립으로 버린다) → 없으면 lite_raw 파일 mtime(파일시스템 관측).
    반환 {start_utc, start_source, end_utc, end_source, raw, legs{cold|warm: {path, doc}}} | None(조인 불성립 · lite-only 아님)."""
    repo = _repo(repo)
    if ev.sweep or ev.report_kind != "lite":
        return None
    d = repo / "output" / ev.topology / "benchlog"
    raw_p = d / f"lite_raw_{ev.cell}.json"
    raw, _bad = _read_json_soft(raw_p)
    born = _report_born_utc(repo, ev.bench_report_path)
    if not (isinstance(raw, dict) and born and raw.get("config_name") == ev.cell and raw.get("measured_utc") == born):
        return None
    legs: dict = {}
    for leg in ("cold", "warm"):
        p = d / f"lite_{leg}_{ev.cell}.json"
        named = Path(str(raw.get(f"bench_{leg}_json") or "")).name
        if named and named != p.name:
            continue
        doc, _b = _read_json_soft(p)
        if isinstance(doc, dict):
            legs[leg] = {"path": _rel(repo, p), "doc": doc}
    end = esrc = None
    dates = []
    for leg, x in legs.items():
        m = _BENCH_JSON_DATE.match(str(x["doc"].get("date") or ""))
        if m:
            u = "{}-{}-{}T{}:{}:{}Z".format(*m.groups())
            if u >= born:
                dates.append((u, x["path"]))
    if dates:
        end, p = max(dates)
        esrc = f"{p} date(마지막 레그 결과 저장 · 컨테이너 시계 = UTC 가정)"
    else:
        try:
            mt = _utc_of_epoch(int(raw_p.stat().st_mtime))
        except OSError:
            mt = None
        if mt and mt >= born:
            end, esrc = mt, f"{_rel(repo, raw_p)} 파일 mtime(파일시스템 관측 · 레그 뒤에 쓰인다)"
    return {"start_utc": born, "start_source": f"{_rel(repo, raw_p)} measured_utc(= bench_report 머리 생성일 · {LITE_START_NOTE})",
            "end_utc": end, "end_source": esrc, "raw": _rel(repo, raw_p), "raw_doc": raw, "legs": legs}


# 측정 도구(lite_bench.sh)의 `vllm bench serve` 호출에서 **리터럴** 인자만 읽는다(`$VAR` · 따옴표 인자는 호출마다 달라 도구 원문이 값을 정하지
#   않는다 · 닫힌 규칙 · 추측 ✗).
_TOOL_FLAG_RE = re.compile(r"(--[a-z][a-z0-9-]*)(?:[ =]([^\s\\'\"$-][^\s\\'\"$]*))?")


def lite_tool_args(repo, ev: CellEvidence) -> dict | None:
    """측정 시점 판본 lite_bench.sh 의 첫 `vllm bench serve` 호출 인자(2026-09-29 · FACT_FIX2 G3). 반환 {flags: [(플래그, 값|None)], source:
    '<도구>@<rev12> L<a>-L<b>', measured_line: 'L<n>'|None} | None(판본 미선택 · 호출 없음). 재구성 명령의 `--random-input-len` 은 bench JSON
    평균(chat 템플릿 포함 실측 토큰)이 아니라 이 값이다."""
    repo = _repo(repo)
    sel = _tool_selection(repo, ev)
    t = next((x for x in sel["tools"] if x["role"] == "lite"), None)
    if t is None or not sel.get("at"):
        return None
    raw = _git_text(repo, sel["at"], t["repo_path"])
    if not raw:
        return None
    rv = _tool_revision(repo, sel["at"], t["repo_path"]) or {}
    lines = raw.decode("utf-8", errors="replace").split("\n")
    tag = f"{t['name']}@{str(rv.get('git_rev') or sel['at'])[:12]}"
    mline = next((f"L{i}" for i, ln in enumerate(lines, 1) if re.match(r"\s*MEASURED_UTC=", ln)), None)
    for i, ln in enumerate(lines):
        if ln.lstrip().startswith("#") or "bench serve" not in ln:
            continue
        j = i
        body = [ln[ln.index("bench serve") + len("bench serve"):]]
        while lines[j].rstrip().endswith("\\") and j + 1 < len(lines):
            j += 1
            body.append(lines[j])
        flags: list[tuple[str, str | None]] = []
        for part in body:
            for m in _TOOL_FLAG_RE.finditer(part.split("#", 1)[0]):
                flag, val = m.group(1), m.group(2)
                after = part[m.end():m.end() + 2].lstrip()
                if val is None and after[:1] not in ("", "-", "\\"):
                    continue        # 값이 변수 · 따옴표 — 도구 원문이 값을 정하지 않는다
                if flag not in {f for f, _ in flags}:
                    flags.append((flag, val))
        return {"flags": flags, "source": f"{tag} L{i + 1}-L{j + 1}", "measured_line": mline, "tool": tag}
    return None


def _measure_window(repo: Path, ev: CellEvidence) -> tuple[str | None, str | None, str]:
    """이 측정의 창 (시작 | None, 끝 | None, 출처). 끝 = `_measured_key`(인증서 measured_utc · 스윕 generated_utc) ·
    시작 = artifacts 의 첫 측정 JSON date(컨테이너 시계 = UTC 가정 · 끝보다 뒤면 가정 불성립으로 버린다 — 재현 표와 같은 규칙).
    G7: lite-only 셀은 `lite_window`(시작 = lite_raw measured_utc · 끝 = 레그 JSON date) — 리포트 생성일은 시작이지 끝이 아니다."""
    from . import artifacts
    lw = lite_window(repo, ev)
    if lw:
        return lw["start_utc"], lw["end_utc"], f"시작 = {lw['start_source']} · 끝 = {lw['end_source'] or '미관측'}"
    end = _measured_key(repo, ev)
    start, ssrc = artifacts._first_measure_utc(repo, ev)
    if start and end and start > end:
        start, ssrc = None, f"{ssrc}(date 를 UTC 로 읽으면 측정 끝 뒤 — 시계 가정 불성립 · 버림)"
    return start, end, f"시작 {ssrc or '미관측'} · 끝 = 측정 키(인증서 measured_utc · 스윕 generated_utc · lite 리포트 생성일)"


def _attestation_timing(repo: Path, ev: CellEvidence, written: str | None) -> tuple[str | None, str | None, dict | None]:
    """attestation 작성 시각 대 측정 창(2026-09-29 · plan_26092908 §4.5 F7 — DS4F: 측정 00:35:13Z 끝 뒤 재기동 01:31:48Z 의 attestation 을
    '같은 실행인지 미검증' 으로만 적었다). (분류, 문장, 창) — 분류 ∈ before-measurement · during-measurement · after-measurement ·
    before-end(시작 미관측 · 끝 이전) · None(작성 또는 끝 미관측)."""
    if not written:
        return None, None, None
    start, end, src = _measure_window(repo, ev)
    if not end:
        return None, None, None
    win = {"start_utc": start, "end_utc": end, "source": src}
    if written > end:
        return "after-measurement", f"작성 {written} > 측정 끝 {end} — 측정 뒤 재기동의 관측", win
    if start and written < start:
        return "before-measurement", f"작성 {written} < 측정 시작 {start} — 측정 전(빌드·스모크)의 관측", win
    if start:
        return "during-measurement", f"측정 창 {start} → {end} 안의 작성 {written} — 측정 중의 관측", win
    return "before-end", f"작성 {written} ≤ 측정 끝 {end}(측정 시작 미관측 — 측정 전·중 미판별)", win


# ── 증거 조립(두 해소기의 공통 꼬리) ────────────────────────────────────────────────────────────────────────────
def _assemble(repo: Path, ev: CellEvidence, *, measured_key: str | None, simlog_dirs=(),
              runner: Callable | None = None) -> CellEvidence:
    missing: list[str] = list(ev.missing)
    ev.repo = str(repo)
    ev.triplet = _triplet(repo, ev.topology, ev.cell)
    ev.sources["triplet"] = f"output/{ev.topology}/(configs|envs) · 이름 = 셀 id(I14 · 2026-09-11 R10)"
    if len(ev.triplet) != 3:
        missing.append("HINT_MISSING_TRIPLET")
    rep_missing, ev.report_kind = _report_missing(repo, ev.certificate, ev.bench_report_path)
    missing += rep_missing
    ev.sources["report_kind"] = ("bench_report 머리 `mode: lite` 줄(render_bench_section.is_lite_report)" if ev.report_kind
                                 else "바인딩할 bench_report 없음")
    ev.sources["missing"] = "evidence.MISSING_CODES 어휘(부재는 기재 · 차단은 양성 검출 — 2026-09-06 Q7/Q8)"
    if ev.report_kind == "lite" and not ev.certificate:
        # 경량 리포트 셀은 스윕을 돌지 않는다 — 같은 이름의 옛 스윕 디렉터리를 이 셀의 증거로 묶지 않고, 부재를
        #   결손으로 적지도 않는다(측정 형태가 다를 뿐이다 · 그 사실은 BENCH_MODE_LITE 가 말한다).
        ev.sweep, ev.sources["sweep"] = None, "lite 셀(경량 리포트) — 스윕 조인 대상 아님"
    else:
        ev.sweep, sm, ev.sources["sweep"] = _bind_sweep(repo, ev.topology, ev.cell, measured_key, simlog_dirs)
        missing += sm
    if not ev.certificate and "HINT_MISSING_CERTIFICATE" in missing:
        not_due = _certificate_not_due(repo, ev)
        if not_due:
            missing = [c for c in missing if c != "HINT_MISSING_CERTIFICATE"]
        ev.sources["certificate_due"] = not_due or "인증서가 발행되는 판정(explicit ∧ PASS) 또는 판정·권한 미관측 — 부재 = 결손"
    ev.serve_proof, sp_rel = _serve_proof(repo, ev.topology, ev.cell)
    ev.sources["serve_proof"] = sp_rel or f"output/{ev.topology}/benchlog/serve_proof_{ev.cell}.json 부재"
    native = _native_producer_evidence(repo, ev.topology, ev.cell, ev.serve_proof, sp_rel)
    if native:
        ev.plane = native["plane"]
        ev.native_install_path = native["native_install_path"]
        ev.pip_freeze_path = native["pip_freeze_path"]
        ev.pip_freeze_paths = native["pip_freeze_paths"]
        ev.cleanup_attestation_path = native["cleanup_attestation_path"]
        ev.cleanup_attestation = native["cleanup_attestation"]
        ev.native_launch_path, ev.native_launch_error = native["native_launch_path"], native["native_launch_error"]
        ev.sources["native_launch_path"] = (f"{native['source']}#native_launch_path(preserved repo-relative)"
                                            if native["native_launch_path"] else
                                            f"{native['source']}#native_launch_path 부재" + (
                                                f" · producer 사유 native_launch_error: {native['native_launch_error']}"
                                                if native["native_launch_error"] else " · 옛 proof(필드 없음)"))
        ev.sources.update({
            "plane": f"{native['source']}#plane(observed native)",
            "native_install_path": f"{native['source']}#native_install_path(preserved repo-relative)",
            "pip_freeze_path": f"{native['source']}#pip_freeze_paths.main(preserved repo-relative)",
            "cleanup_attestation": f"{native['source']}#cleanup_attestation_path → {native['cleanup_attestation_path']}",
        })
        # Cleanup evidence is a producer fact and therefore belongs to the lineage candidates.
        ev.lineage_seeds = {**ev.lineage_seeds, "native_cleanup_attestation": native["cleanup_attestation_path"]}
    ev.manifest, ev.sources["manifest"] = _manifest(repo, ev.topology)
    env = _env_first(repo, ev.triplet.get("env"))
    ev.build_identity, bm = _build_identity(repo, topology=ev.topology, env=env, env_rel=ev.triplet.get("env"),
                                            measured=_measured_meta(ev.sweep, ev.certificate), manifest=ev.manifest,
                                            runner=runner)
    missing += bm
    ev.sources["build_identity"] = "build_identity.source(필드별)"
    ev.resolve, ev.sources["resolve"] = _bind_resolve(repo, ev.topology, ev.build_identity)
    ref = str(ev.build_identity.get("vllm_ref") or "").strip().lower()
    if _SHA40.match(ref):
        # 커밋 핀은 그 자체가 SHA 다 — resolved.json 바인딩 여부와 무관하다(옛 판본은 해소값이 묶일 때만 채워 커밋 핀
        #   빌드의 PAYLOAD.build.vllm_sha 가 비었다 · 2026-09-22 적대 검토).
        ev.build_identity["vllm_sha"] = ref
        ev.build_identity["source"]["vllm_sha"] = f"VLLM_REF(40자 커밋 · {ev.build_identity['source'].get('vllm_ref')})"
    from . import naming
    fork = naming.vllm_repo_kind(ev.build_identity.get("vllm_repo")) == "fork"
    if ev.resolve is None:
        missing.append("HINT_MISSING_RESOLVE")
    else:
        # D-h(2026-09-22): resolved.json 의 to_sha 는 **업스트림** 태그의 커밋이다 — 포크 빌드의 SHA 로 쓰면 이름·재현 좌표가 다른
        #   커밋을 가리킨다. 포크의 SHA 는 빌드 원장(`vllm.git_sha`)에서만 받는다(아래 attestation 바인딩 뒤).
        if not _SHA40.match(ref) and ev.resolve.get("to_sha") and not fork:
            ev.build_identity["vllm_sha"] = ev.resolve["to_sha"]
            ev.build_identity["source"]["vllm_sha"] = f"{ev.sources['resolve']} upstream_delta.to_sha"
        if ev.resolve.get("ngc_image") and ev.build_identity.get("ngc") and \
                str(ev.resolve.get("ngc_tag") or "").startswith(str(ev.build_identity["ngc"])):
            ev.build_identity["ngc"] = ev.resolve["ngc_image"]
            ev.build_identity["source"]["ngc"] += " = resolved.json ngc_base.image 접두 일치"
    ev.attestation, am, ev.sources["attestation"] = _bind_attestation(repo, ev.topology, ev.node, ev.cell,
                                                                       ev.build_identity)
    missing += am
    # 호스트 사실은 attestation 바인딩 **뒤에** 채운다 — 노드별 관측(parity)이 선언 옮김(스윕 meta·인증서)을 이긴다
    #   (plan_26092908 §4.5 · V3). 옛 판본은 attestation 을 묶기 전에 불러 관측 원천이 구조적으로 없었다.
    _host_facts(ev)
    _local_image_digest(repo, ev, runner)
    if fork and not _SHA40.match(ref):
        led_sha = str(_ledger_vllm(ev.attestation).get("git_sha") or "").strip().lower()
        ev.build_identity["vllm_sha"] = led_sha if _SHA40.match(led_sha) else None
        ev.build_identity["source"]["vllm_sha"] = (
            f"빌드 원장 vllm.git_sha({(ev.attestation or {}).get('_path')} · 포크 VLLM_REPO — resolved.json 은 업스트림 해소라 쓰지 않는다)"
            if ev.build_identity["vllm_sha"] else "미관측(포크 VLLM_REPO · 빌드 원장 vllm.git_sha 없음 — D-h: 이름은 차단된다)")
    if not (Path(repo) / core.REL_PII_TERMS).exists():
        missing.append("HINT_MISSING_PII_TERMS")
    ev.missing = sorted(set(missing))
    mc = measurement_config(repo, ev)
    if mc.get("bench_mode") == "lite":
        ev.missing = sorted(set(ev.missing) | {"BENCH_MODE_LITE"})
    unknown = [c for c in ev.missing if c not in MISSING_CODES]
    if unknown:
        core.fail("HINT_MISSING_CODE_UNREGISTERED", f"등재되지 않은 결손 코드: {unknown}",
                  "evidence.MISSING_CODES 에 뜻과 함께 등재한다(tripwire — 조용히 차단/통과로 떨어지지 않게).")
    # 계보 시드 — lineage.derive 가 읽는 예약 키(identity·cell_key·axis_tokens·evidence_candidates·this_topic) + 본문 인용 키
    seeds = dict(ev.lineage_seeds)
    seeds["cell_key"] = ev.cell
    seeds["identity"] = _measured_identity(repo, ev)[0]
    toks = [ev.cell]
    for v in (ev.build_identity.get("image_tag"), _release_of(ev.build_identity.get("vllm_ref"))):
        if v and v not in toks:
            toks.append(str(v))
    seeds["axis_tokens"] = toks
    cands = []
    bench = f"output/{ev.topology}/benchlog"
    if ev.attestation:
        cands.append({"path": ev.attestation["_path"], "kind": "attestation"})
        # artifacts(`_attestation_ref`)는 묶인 attestation 경로를 이 키로 읽는다 — 없으면 `attestation_<셀>.json` 을 가정해
        #   빌드 config 이름으로 묶인 파일(태그2: attestation_q38fn-nvfp4-mmap.json)을 sub_recipe 출처에 **없는 경로**로 적는다.
        seeds["attestation"] = ev.attestation["_path"]
    if sp_rel:
        cands.append({"path": sp_rel, "kind": "serve_proof"})
    if ev.cleanup_attestation_path:
        cands.append({"path": ev.cleanup_attestation_path, "kind": "native_cleanup_attestation"})
    names = [ev.cell]
    pm = re.fullmatch(r"vllm_(.+)_project", str(ev.build_identity.get("compose_project") or ""))
    if pm and pm.group(1) not in names:
        names.append(pm.group(1))
    for n in names:
        d = repo / bench / f"serve_fail_{n}"
        if d.is_dir():
            cands.append({"path": _rel(repo, d), "kind": "engine_failure_logs"})
    # 과거 측정 창의 identity 대조가 같은 identity 를 쓰도록 먼저 건다(아래 타임라인 인용 파일 계산이 읽는다).
    ev.lineage_seeds = {**ev.lineage_seeds, "identity": seeds["identity"]}
    # 타임라인(event_timeline)이 **출처로 인용한** 파일 — 저작자가 원장·발행 기록 줄을 발췌할 때 출처가 계보 밖이 되지 않게
    #   (2026-09-22 · S2 round 2 F6 · round 3: 과거 측정 행의 발행 기록·색인 · 직전/직후 예산 행의 원장).
    for f, kind in _timeline_parts(repo, ev)[1]:
        cands.append({"path": f, "kind": kind})
    # 2026-09-22(S2 round 3): 2차 저작자가 output/<t>/resolved.json 을 발췌하지 못했다(HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE — 빌드 트랙
    #   근거를 의역만 했다). 이 빌드에 **묶였을 때만** 싣는다(다른 버전의 해소값을 이 빌드의 출처로 인용하지 않게).
    if ev.resolve is not None:
        cands.append({"path": f"output/{ev.topology}/resolved.json", "kind": "resolve"})
    # 측정 환경 관측(measurement_env_observed)이 읽는 이 측정의 엔진 로그 · 엔진 자기보고 배너의 출처 로그(S2 round 3).
    for lp in _measured_engine_logs(repo, ev)[0]:
        cands.append({"path": _rel(repo, lp), "kind": "engine_log"})
    for f, kind in _engine_banner(repo, ev)["files"]:
        cands.append({"path": f, "kind": kind})
    seen: set[str] = set()
    seeds["evidence_candidates"] = [c for c in cands if not (c["path"] in seen or seen.add(c["path"]))]
    ev.lineage_seeds = seeds
    ev.sources["lineage_seeds"] = ("캠페인 journey·escalation·grounding(셀 필터) + 측정 identity + 증거 후보"
                                   if ev.mode == "campaign" else "발행 기록 토픽 + 측정 identity + 증거 후보(캠페인 purge)")
    return ev


def _release_of(ref) -> str | None:
    from . import naming       # 릴리스 문법의 단일 소유자(D-g)
    return naming.release_of(ref)


def _tp_signal(repo: Path, topology: str, cell: str, *, cert: dict | None, measured_key: str | None, report_rel: str | None,
               simlog_dirs=()) -> tuple[int | None, str]:
    """노드 축 자동 파생의 입력 — 이 셀의 측정 TP 와 출처. 측정 > 선언: 인증서 tensor_parallel_size > 조인된 스윕 meta(generated_utc =
    측정 키) > 리포트 측정 환경 표 > 서빙 yaml(선언 · 측정 기록이 TP 를 들지 않을 때만 · 출처에 그렇게 적는다)."""
    tp = _as_int(_norm_value((cert or {}).get("tensor_parallel_size")))
    if tp is not None:
        return tp, "인증서 tensor_parallel_size"
    if measured_key:
        sw, _m, _s = _bind_sweep(repo, topology, cell, measured_key, simlog_dirs)
        meta = ((sw or {}).get("index") or {}).get("meta") or {}
        tp = _as_int(_norm_value(meta.get("tensor_parallel_size")))
        if tp is not None:
            return tp, f"sweep_index.meta.tensor_parallel_size({(sw or {}).get('dir')} · generated_utc = 측정 키)"
    tp = _as_int(_norm_value(_lite_snapshot(repo, report_rel).get("tensor_parallel_size")))
    if tp is not None:
        return tp, f"bench_report({Path(report_rel or '').name}) 측정 환경 표 tensor_parallel_size"
    yrel = f"output/{topology}/configs/{cell}.yaml"
    if (repo / yrel).is_file():
        y = _owner_yaml(repo, repo / yrel, "서빙 yaml")
        tp = _as_int((y or {}).get("tensor-parallel-size")) if isinstance(y, dict) else None
        if tp is not None:
            return tp, f"서빙 yaml tensor-parallel-size({yrel} · 선언 — 측정 기록이 TP 를 들지 않는다)"
    return None, "측정 TP 미관측(인증서·조인 스윕 meta·리포트 표·서빙 yaml 모두 없음)"


def _auto_node(repo: Path, topology: str, cell: str, *, cert: dict | None, measured_key: str | None, report_rel: str | None,
               simlog_dirs=()) -> tuple[str | None, str]:
    """`--node` 미지정일 때 **cluster** 축 자동 파생(2026-09-29 · plan_26092908 §4.8 · V11 ③ — 멀티 셀 3/3 이 `--node cluster` 를 손으로
    줬다: 인증서 없는 REFUTE·lite 셀은 measured_node 가 없어 배정(main)으로 떨어지고 TP=2 측정이 HINT_ARCH_TP_CONFLICT 로 막혔다).
    신호: ① 측정 TP > manifest gpus_per_node(한 노드가 가진 GPU 보다 큰 TP 는 단일 노드 형상일 수 없다) ② 멀티 serve proof
    (`kind ∈ {multinode_serve_proof, native_multinode_serve_proof}` · config = 셀) ③ 셀 이름 노드 정합 attestation.
    판정: ① 참 → cluster · TP 관측 ∧ TP ≤ gpus_per_node(단일 노드 형상) ∧ ②③ 중 하나 → **모호 = 죽는다**(HINT_NODE_AXIS_UNDERIVABLE —
    기존 오류와 같은 코드) · TP 미관측 ∧ ②③ → cluster · 그 밖 → (None, 사유)(호출부가 기존 규칙 — 배정 · 오류 — 로 간다)."""
    if topology != "multi":
        return None, f"topology={topology} — cluster 자동 파생 대상 아님"
    tp, tp_src = _tp_signal(repo, topology, cell, cert=cert, measured_key=measured_key, report_rel=report_rel,
                            simlog_dirs=simlog_dirs)
    man, man_src = _manifest(repo, topology)
    gpn = _as_int((man or {}).get("gpus_per_node"))
    sp, sp_rel = _serve_proof(repo, topology, cell)
    multi_proof = isinstance(sp, dict) and sp.get("kind") in ("multinode_serve_proof", "native_multinode_serve_proof") \
        and sp.get("cell", sp.get("config")) == cell
    att_rel = f"output/{topology}/benchlog/attestation_{cell}.json"
    att = (repo / att_rel).is_file()
    multi = ([f"멀티 serve proof {sp_rel}(kind={sp.get('kind')})"] if multi_proof else []) + \
        ([f"노드 정합 attestation {att_rel}"] if att else [])
    if tp is not None and gpn:
        if tp > gpn:
            return "cluster", (f"자동 파생 — 측정 TP={tp}({tp_src}) > gpus_per_node={gpn}({man_src}) · 단일 노드 형상일 수 없다"
                               + (f" · 일치 신호: {' · '.join(multi)}" if multi else ""))
        if multi:
            core.fail("HINT_NODE_AXIS_UNDERIVABLE",
                      f"노드 축이 모호하다 — 측정 TP={tp}({tp_src}) ≤ gpus_per_node={gpn}(단일 노드 형상)인데 멀티 신호가 있다: {multi}.",
                      "측정한 노드 축을 `--node main|sub|cluster` 로 명시한다(한 태그 = 한 노드 형상의 실행 기록).")
        return None, f"측정 TP={tp} ≤ gpus_per_node={gpn} — 단일 노드 형상(자동 cluster ✗)"
    if multi:
        return "cluster", f"자동 파생 — TP·gpus_per_node 대조 불가({tp_src} · gpus_per_node={gpn}) · 멀티 신호: {' · '.join(multi)}"
    return None, f"cluster 신호 없음({tp_src} · gpus_per_node={gpn} · 멀티 serve proof·attestation 부재)"


def _resolve_node(ev_node, *, measured, topology: str, assigned: str | None, declared: list[str],
                  cert_node_source: str, auto: tuple[str | None, str] | None = None) -> tuple[str, str]:
    """발행 노드 축. **한 태그 = 한 노드 형상의 실행 기록**(계약 §1 · 옛 hint_tag._require_node_axis_match ·
    2026-09-07 plan_26090715 R4). 인증서 measured_node 와 다르면 죽는다 — 결손(부재)은 막지 않는다(부재 ≠ 불일치).
    `--node` 미지정 순서: 인증서 measured_node > 자동 cluster 파생(`_auto_node` · 2026-09-29 plan_26092908 §4.8) > 배정 > 오류."""
    measured = None if _unobserved(measured) else str(measured).strip()
    if ev_node is None:
        if measured in NODE_AXIS:
            node, src = measured, f"{cert_node_source} measured_node"
        elif auto is not None and auto[0] == "cluster":
            node, src = "cluster", auto[1]
        elif assigned:
            node, src = assigned, "캠페인 assignments(배정 SSOT)"
        else:
            core.fail("HINT_NODE_AXIS_UNDERIVABLE", "발행 노드 축을 파생하지 못했다(measured_node·자동 cluster·배정 모두 없음"
                      + (f" · 자동: {auto[1]}" if auto else "") + ").",
                      "`--node main|sub|cluster` 로 준다.")
    else:
        node, src = _check_node(ev_node), "발행 인자 --node"
    if node == "cluster" and topology != "multi":
        core.fail("HINT_ARCH_NODE_AXIS_CERT_MISMATCH", f"cluster 축은 multi 토폴로지에서만 성립한다(topology={topology}).")
    if measured is not None and measured != node:
        core.fail("HINT_ARCH_NODE_AXIS_CERT_MISMATCH",
                  f"{cert_node_source} measured_node={measured!r} ≠ 발행 노드 축 {node!r} — 한 태그 = 한 노드 형상의 실행 기록이다.",
                  "측정한 그 노드 축으로 발행한다(측정 노드 어휘는 campaign_template_validator.resolve_measurement_node).")
    if node in ("main", "sub") and assigned and node != assigned and node in declared:
        core.fail("HINT_NODE_ASSIGNMENT_MISMATCH", f"셀은 {assigned!r} 에 배정됐는데 발행 노드 축이 {node!r} 다.")
    return node, src


def _pointer_ok(p: dict, node: str, declared: list[str], val) -> bool:
    """포인터가 발행 노드 축에 속하는가. 측정축 어휘(`cluster` → 선언 노드)는 소유자
    `campaign_template_validator.resolve_measurement_node` 한 벌(옛 `_NODE_AXIS_TO_*` 삭제 · I9 · 2026-09-11 R0).
    노드 태그 없는 포인터는 **단일노드 캠페인에서만** 그 하나의 노드에 귀속한다 — 다노드에서 노드 없는 증거는 "어느 노드가
    쟀는지 모르는 측정" 이고(validator P2 · 2026-09-08 camp-26090721 null 6건), 발행 노드에 섞으면 다른 노드의 증거를 싣는다."""
    nid = p.get("node_id")
    if nid in (None, ""):
        return len(declared) <= 1
    if nid == node:
        return True
    if node in getattr(val, "DERIVED_MEASUREMENT_NODES", ()):
        cands, _kind = val.resolve_measurement_node(node, declared)
        return nid in cands
    return False


def _certificate_from_pointers(repo: Path, ptrs: list[dict]) -> tuple[str | None, dict | None, list[str]]:
    """셀 포인터의 인증서. 같은 측정 키(강한 6키 + measured_utc)면 하나로 본다 · 다른 측정이 둘 이상이면 모호 = 죽는다."""
    found: dict = {}
    absent: list[str] = []
    g = None
    for p in ptrs:
        rel = p.get("path")
        if not isinstance(rel, str) or not rel:
            continue
        if not (repo / rel).is_file():
            absent.append(rel)
            continue
        cert = parse_certificate(repo, rel)
        g = g or cgate(repo)
        found.setdefault(g.certificate_run_key(cert) or ("unidentifiable", rel), (rel, cert))
    if len(found) > 1:
        core.fail("HINT_CERTIFICATE_AMBIGUOUS", f"이 셀에 서로 다른 측정의 인증서가 {len(found)}건 걸렸다: "
                  f"{sorted(v[0] for v in found.values())}",
                  "stale 포인터를 캠페인 writer 로 정리한다 — 발행기가 둘 중 하나를 고르지 않는다.")
    if found:
        rel, cert = next(iter(found.values()))
        return rel, cert, absent
    return None, None, absent


def from_campaign(repo, campaign_id, cell, node=None, *, docker=None) -> CellEvidence:
    """캠페인 셀 → CellEvidence. **명시 id 만 읽는다 — `campaigns/ACTIVE` 를 열지 않는다**(모듈 docstring)."""
    repo = _repo(repo)
    _check_campaign_id(campaign_id)
    _check_cell(cell)
    base = repo / "campaigns" / campaign_id
    if not base.is_dir():
        core.fail("HINT_CAMPAIGN_ABSENT", f"캠페인 인스턴스 부재: campaigns/{campaign_id}",
                  "purge 된 캠페인이면 `--publication <topic> --replay` 로 발행 기록에서 재생한다.")
    decl = _read_json_opt(base / "campaign.yaml", "campaign.yaml")
    if not isinstance(decl, dict) or decl.get("id") != campaign_id:
        core.fail("HINT_CAMPAIGN_DECLARATION_INVALID", f"campaigns/{campaign_id}/campaign.yaml 의 id 가 {campaign_id!r} 가 아니다.")
    val = _validator(repo)
    declared = [n.get("node_id") for n in decl.get("nodes") or [] if isinstance(n, dict) and n.get("node_id")]
    assigned = next((n for n in declared if cell in val.assigned_cells(decl, n)), None)
    if assigned is None:
        core.fail("HINT_CELL_ASSIGNMENT_ABSENT", f"campaigns/{campaign_id} assignments 에 셀 {cell!r} 이 없다.",
                  "배정 SSOT 는 assignments 다 — 셀 이름이 곧 증거 연결이다(2026-09-11 R10).")
    node_decl = next((n for n in decl.get("nodes") or [] if isinstance(n, dict) and n.get("node_id") == assigned), {})
    topology = node_decl.get("topology") or (decl.get("control_variables") or {}).get("topology")
    if topology not in TOPOLOGIES:
        core.fail("HINT_TOPOLOGY_UNDERIVABLE", f"캠페인 선언에서 토폴로지를 해소하지 못했다: {topology!r}")
    cdir = base / "cells" / cell
    sources: dict = {"campaign_id": "발행 인자 --campaign(명시 id · ACTIVE 미독)", "cell": "발행 인자 --cell",
                     "declaration": f"campaigns/{campaign_id}/campaign.yaml(명시 id)",
                     "topology": f"campaigns/{campaign_id}/campaign.yaml nodes[{assigned}].topology"}
    missing: list[str] = []
    status = _read_json_opt(cdir / "cell.status.json", "cell.status.json")
    lock = _read_json_opt(cdir / "lockset.json", "lockset.json")
    config = _owner_yaml(repo, cdir / "config.yaml", "셀 config") if (cdir / "config.yaml").is_file() else None
    for code, obj, name in (("HINT_MISSING_CELL_STATUS", status, "cell.status.json"), ("HINT_MISSING_LOCKSET", lock, "lockset.json"),
                            ("HINT_MISSING_CELL_CONFIG", config, "config.yaml")):
        if not isinstance(obj, dict):
            missing.append(code)
        else:
            sources[{"cell.status.json": "cell_status", "lockset.json": "lockset", "config.yaml": "cell_config"}[name]] = \
                f"campaigns/{campaign_id}/cells/{cell}/{name}"
    ptr_doc = _read_json_opt(base / "evidence_pointers.json", "evidence_pointers.json")
    raw = ptr_doc.get("pointers") if isinstance(ptr_doc, dict) and isinstance(ptr_doc.get("pointers"), list) else []
    cell_ptrs = [dict(p) for p in raw if isinstance(p, dict) and p.get("cell_id") == cell]
    cert_rel, cert, absent = _certificate_from_pointers(repo, [p for p in cell_ptrs if p.get("kind") == "certificate"])
    auto = None
    if node is None and _unobserved((cert or {}).get("measured_node")):
        # 자동 cluster 파생(plan_26092908 §4.8)의 측정 키 — 노드 필터 **전** 의 셀 리포트 포인터(하나일 때만) 또는 인증서 이름 쌍.
        pre = sorted({p["path"] for p in cell_ptrs if p.get("kind") == "bench_report" and isinstance(p.get("path"), str)
                      and (repo / p["path"]).is_file()})
        pre_rel = pre[0] if len(pre) == 1 else _report_twin(cert_rel)
        auto = _auto_node(repo, topology, cell, cert=cert, report_rel=pre_rel,
                          measured_key=_norm_value((cert or {}).get("measured_utc")) or _report_born_utc(repo, pre_rel))
    node, sources["node"] = _resolve_node(node, measured=(cert or {}).get("measured_node"), topology=topology,
                                          assigned=assigned, declared=declared, cert_node_source="인증서", auto=auto)
    pointers = sorted((p for p in cell_ptrs if _pointer_ok(p, node, declared, val)),
                      key=lambda p: (str(p.get("kind")), str(p.get("path"))))
    if cert_rel and not any(p.get("path") == cert_rel for p in pointers):
        core.fail("HINT_ARCH_NODE_AXIS_CERT_MISMATCH", f"인증서 포인터 {cert_rel} 의 node_id 가 발행 노드 축 {node!r} 과 어긋난다.")
    sources["pointers"] = (f"campaigns/{campaign_id}/evidence_pointers.json · cell_id={cell} · node∈{node}"
                           + (f" · 파일 부재 포인터 {absent}" if absent else ""))
    reports = sorted({p["path"] for p in pointers if p.get("kind") == "bench_report" and isinstance(p.get("path"), str)
                      and (repo / p["path"]).is_file()})
    twin = _report_twin(cert_rel)
    if twin and twin in reports:
        report_rel, sources["bench_report_path"] = twin, "캠페인 포인터(인증서 이름 쌍)"
    elif len(reports) == 1:
        report_rel, sources["bench_report_path"] = reports[0], "캠페인 포인터 bench_report"
    elif len(reports) > 1:
        core.fail("HINT_BENCH_REPORT_AMBIGUOUS", f"셀 {cell} 에 bench_report 포인터가 {len(reports)}건이다: {reports}",
                  "측정 하나만 가리키도록 포인터를 정리한다 — 발행기가 고르지 않는다.")
    elif twin and (repo / twin).is_file():
        report_rel, sources["bench_report_path"] = twin, "인증서 이름 쌍(docs.md 명명 SSOT · 리포트 포인터 producer 부재 — G9)"
    else:
        report_rel = None
    sources["certificate"] = f"캠페인 포인터 certificate({cert_rel})" if cert_rel else "인증서 포인터 없음"
    _require_report_joins_certificate(repo, report_rel, cert)
    phases: dict = {}
    for cand in ([assigned] if node != "cluster" else [assigned] + [d for d in declared if d != assigned]):
        for f in sorted((base / "phases" / str(cand)).glob("*.status.json")):
            doc = _read_json_opt(f, "phase status")
            if isinstance(doc, dict) and doc.get("cell_id") == cell and doc.get("phase") and doc["phase"] not in phases:
                phases[doc["phase"]] = {k: doc.get(k) for k in ("phase", "state", "node_id", "started_utc", "ended_utc",
                                                                "first_started_utc", "proof")}
                phases[doc["phase"]]["_path"] = _rel(repo, f)
    sources["phases"] = f"campaigns/{campaign_id}/phases/<node>/*.status.json 중 cell_id={cell}(노드별 덮어쓰기 — 마지막 셀만 남는다)"
    seeds: dict = {}
    jpath = base / "journey.jsonl"
    if jpath.is_file():
        seeds["journey"] = [ln for ln in (_json_line(x) for x in jpath.read_text(encoding="utf-8").splitlines())
                            if isinstance(ln, dict) and ln.get("cell_id") == cell]
    epath = base / "escalation_candidates.jsonl"
    if epath.is_file():
        seeds["escalations"] = [ln for ln in (_json_line(x) for x in epath.read_text(encoding="utf-8").splitlines())
                                if isinstance(ln, dict) and ln.get("cell_id") == cell]
    gdir = base / "grounding"
    if gdir.is_dir():
        latest = sorted(gdir.glob("*.json"))
        if latest:
            g = _read_json_opt(latest[-1], "grounding")
            refs = ((g or {}).get("export") or {}).get("references") if isinstance(g, dict) else None
            seeds["grounding_refs"] = [r.get("path") or r.get("ref_id") for r in refs or [] if isinstance(r, dict)]
    if isinstance(decl.get("plan_ref"), str):
        seeds["plan_ref"] = decl["plan_ref"]
    ev = CellEvidence(mode="campaign", campaign_id=campaign_id, cell=cell, node=node, topology=topology,
                      declaration=decl, cell_status=status if isinstance(status, dict) else None,
                      lockset=lock if isinstance(lock, dict) else None,
                      cell_config=config if isinstance(config, dict) else None,
                      certificate=cert, certificate_path=cert_rel, bench_report_path=report_rel, pointers=pointers,
                      phases=phases, lineage_seeds=seeds, missing=missing, sources=sources)
    key = (cert or {}).get("measured_utc") or _report_born_utc(repo, report_rel)
    return _assemble(repo, ev, measured_key=_norm_value(key), runner=docker)


def _json_line(text: str):
    try:
        return json.loads(text)
    except ValueError:
        return None


def from_publication(repo, topic, node=None, *, docker=None) -> CellEvidence:
    """발행 기록 재생(purge 된 캠페인의 유일한 통로 · 코드맵 G5). `docs/_evidence/<topic>.json` + work-manifest + 스윕 조인.
    **읽기만 한다**(쓰기 0). 셀 이름은 발행 기록의 simlog 사본 `sweep_index.meta.config_name` 또는 출력 평면 스윕 중
    `generated_utc == measured_utc` 인 것에서 파생한다 — 토픽 문자열을 셀 이름으로 해석하지 않는다."""
    repo = _repo(repo)
    if not isinstance(topic, str) or not topic or "/" in topic:
        core.fail("HINT_PUBLICATION_TOPIC_INVALID", f"발행 토픽이 올바르지 않다: {topic!r}")
    rec_path = repo / core.REL_EVIDENCE_DIR / f"{topic}.json"
    record = _read_json_opt(rec_path, "발행 기록")
    if not isinstance(record, dict):
        core.fail("HINT_PUBLICATION_ABSENT", f"발행 기록 부재: {core.REL_EVIDENCE_DIR}/{topic}.json")
    if record.get("publication_id") not in (None, topic):
        core.fail("HINT_PUBLICATION_UNREADABLE", f"발행 기록 publication_id={record.get('publication_id')!r} ≠ {topic!r}")
    man_path = repo / core.REL_EVIDENCE_DIR / f"{topic}.work-manifest.json"
    work_manifest = _read_json_opt(man_path, "work-manifest")
    ident = record.get("identity") if isinstance(record.get("identity"), dict) else {}
    topology = ident.get("topology")
    if topology not in TOPOLOGIES:
        core.fail("HINT_TOPOLOGY_UNDERIVABLE", f"발행 기록 identity.topology 가 single|multi 가 아니다: {topology!r}")
    scaff = record.get("scaffolded") if isinstance(record.get("scaffolded"), dict) else {}
    cert_rel = scaff.get("certificate") if isinstance(scaff.get("certificate"), str) else None
    cert = parse_certificate(repo, cert_rel) if cert_rel and (repo / cert_rel).is_file() else None
    report_rel = scaff.get("bench_report") if isinstance(scaff.get("bench_report"), str) else None
    if report_rel and not (repo / report_rel).is_file():
        report_rel = None
    _require_report_joins_certificate(repo, report_rel, cert)
    measured_key = _norm_value((cert or {}).get("measured_utc")) or _norm_value((record.get("benchmark") or {}).get("measured_utc")) \
        or _report_born_utc(repo, report_rel)
    simlog = (record.get("raw_log_paths") or {}).get("simlog") or scaff.get("simlog")
    simlog_dirs = [simlog] if isinstance(simlog, str) and simlog else []
    cell, cell_src = _cell_of_publication(repo, topology, measured_key, simlog_dirs)
    auto = None
    if node is None and _unobserved((cert or {}).get("measured_node")):
        auto = _auto_node(repo, topology, cell, cert=cert, measured_key=measured_key, report_rel=report_rel,
                          simlog_dirs=simlog_dirs)
    node, node_src = _resolve_node(node, measured=(cert or {}).get("measured_node"), topology=topology, assigned=None,
                                   declared=[], cert_node_source="인증서", auto=auto)
    pointers = [{"kind": k, "path": v, "cell_id": cell, "node_id": node} for k, v in sorted(scaff.items())
                if isinstance(v, str) and v]
    sources = {"publication": f"{core.REL_EVIDENCE_DIR}/{topic}.json", "topology": "발행 기록 identity.topology",
               "cell": cell_src, "node": node_src, "pointers": "발행 기록 scaffolded(캠페인 인스턴스 purge — 캠페인 포인터 없음)",
               "certificate": f"발행 기록 scaffolded.certificate({cert_rel})" if cert else "발행 기록에 인증서 없음",
               "bench_report_path": "발행 기록 scaffolded.bench_report" if report_rel else "발행 기록에 리포트 없음"}
    publication = {"topic": topic, "record": record, "record_path": _rel(repo, rec_path),
                   "manifest_path": _rel(repo, man_path) if man_path.is_file() else None,
                   "manifest": work_manifest if isinstance(work_manifest, dict) else None,
                   "task_class": record.get("task_class")}
    ev = CellEvidence(mode="publication-replay", campaign_id=None, cell=cell, node=node, topology=topology,
                      certificate=cert, certificate_path=cert_rel if cert else None, bench_report_path=report_rel,
                      pointers=pointers, publication=publication, lineage_seeds={"this_topic": topic},
                      missing=["HINT_MISSING_CAMPAIGN_INSTANCE"], sources=sources)
    return _assemble(repo, ev, measured_key=measured_key, simlog_dirs=simlog_dirs, runner=docker)


def _cell_of_publication(repo: Path, topology: str, measured_key: str | None, simlog_dirs) -> tuple[str, str]:
    for sd in simlog_dirs:
        idx = _read_json_opt(repo / sd / "sweep_index.json", "simlog sweep_index.json")
        if isinstance(idx, dict):
            name = ((idx.get("meta") or {}).get("config_name")) or idx.get("config")
            if measured_key and idx.get("generated_utc") != measured_key:
                continue
            if isinstance(name, str) and _CELL_RE.match(name):
                return name, f"{sd}/sweep_index.json meta.config_name(발행 기록의 simlog 사본)"
    if measured_key:
        hits = []
        for idx_path in sorted((repo / "output" / topology / "benchlog").glob("sweep_*/sweep_index.json")):
            idx = _read_json_opt(idx_path, "sweep_index.json")
            if isinstance(idx, dict) and idx.get("generated_utc") == measured_key:
                name = (idx.get("meta") or {}).get("config_name") or idx.get("config")
                if isinstance(name, str) and idx_path.parent.name == f"sweep_{name}":
                    hits.append(name)
        if len(hits) == 1:
            return hits[0], f"output/{topology}/benchlog/sweep_{hits[0]}(generated_utc = measured_utc {measured_key})"
        if len(hits) > 1:
            core.fail("HINT_CELL_AMBIGUOUS", f"measured_utc={measured_key} 인 스윕이 {len(hits)}개다: {hits}")
        # lite-only 셀(2026-09-29 · plan_26092908 §4.8 재생 — D2 `nv4-f8-262k-mmp` 가 purge 뒤 재생 불가였다): 스윕이 없고 발행 기록에
        #   simlog 도 없다. 조인 키 = lite raw 의 `measured_utc`(= 경량 리포트 `생성일`) + `config_name` = 파일 이름의 셀(이름만으로 ✗ · G6).
        lite_hits = []
        for raw_path in sorted((repo / "output" / topology / "benchlog").glob("lite_raw_*.json")):
            raw = _read_json_opt(raw_path, "lite_raw")
            name = raw.get("config_name") if isinstance(raw, dict) else None
            if isinstance(name, str) and _CELL_RE.match(name) and raw_path.name == f"lite_raw_{name}.json" \
                    and raw.get("measured_utc") == measured_key:
                lite_hits.append(name)
        if len(lite_hits) == 1:
            return lite_hits[0], (f"output/{topology}/benchlog/lite_raw_{lite_hits[0]}.json(measured_utc = 리포트 생성일 {measured_key} · "
                                  "config_name = 파일 이름)")
        if len(lite_hits) > 1:
            core.fail("HINT_CELL_AMBIGUOUS", f"measured_utc={measured_key} 인 lite raw 가 {len(lite_hits)}개다: {lite_hits}")
    core.fail("HINT_CELL_UNDERIVABLE", "발행 기록에서 셀 이름을 파생하지 못했다(simlog 사본·조인되는 스윕 모두 없음).",
              "셀 이름은 토픽 문자열에서 추측하지 않는다 — 발행 기록의 simlog 에 sweep_index.json 이 있어야 한다.")


# ── 발행 자격(관측 · X8) ─────────────────────────────────────────────────────────────────────────────────────
# serve_proof 의 **통과 어휘**(닫힌 목록 · 양성 목록). 쓰는 쪽 = upstream `multinode_serve_smoke.sh` — `inference_verdict` ∈
#   {pass, fail, relaxed-reasoning-only, not-attempted}, `evidence` ∈ {chat.content, v1.completions, chat.reasoning(relaxed)}.
#   ★ 실패 어휘의 **부정 목록**으로 판정하면 쓰는 쪽이 새 판정어를 더하는 순간 그 판정어가 조용히 통과한다(fail-open ·
#   2026-09-22 적대 검토: 옛 부정 목록은 `relaxed-reasoning-only`·`not-attempted` 를 몰랐고, `v1.completions` 로 확증한 정상 PASS
#   는 chat content_len=0 이라 거부했다). 통과는 "생성 실재를 그 증거 칸의 길이로 확증한 pass" 둘뿐이다 — 완화 PASS
#   (reasoning-only)는 스모크 스스로 "생성 실재를 증명하지 않는다" 고 적는다(2026-08-22 W-10 · 빈 응답 통과 금지).
_SERVE_PROOF_PASS_EVIDENCE = {"chat.content": "content_len", "v1.completions": "completion_text_len"}


def _serve_proof_ok(sp: dict, cell: str, image_tag: str | None) -> tuple[bool, str]:
    # Native proof has a different producer schema. Its complete validation belongs to
    # _native_producer_evidence during assembly; this branch only preserves the shared
    # qualification contract without Docker image-tag coupling.
    if sp.get("plane") == "native":
        health = sp.get("health") if isinstance(sp.get("health"), dict) else {}
        endpoints = sp.get("endpoints") if isinstance(sp.get("endpoints"), dict) else {}
        h_endpoint = endpoints.get("health") if isinstance(endpoints.get("health"), dict) else {}
        inference = sp.get("inference") if isinstance(sp.get("inference"), dict) else {}
        i_endpoint = endpoints.get("inference") if isinstance(endpoints.get("inference"), dict) else {}
        ok = sp.get("kind") == "native_multinode_serve_proof" and sp.get("status") == "PASS" \
            and health.get("ok") is True and str(h_endpoint.get("http_status")) == "200" \
            and inference.get("ok") is True and str(i_endpoint.get("http_status")) == "200" \
            and (_as_int(inference.get("completion_text_len")) or 0) >= 1
        return ok, f"native kind={sp.get('kind')!r} status={sp.get('status')!r} health={health.get('ok')!r}/{h_endpoint.get('http_status')!r} inference={inference.get('ok')!r}/{i_endpoint.get('http_status')!r} completion_text_len={inference.get('completion_text_len')!r}"
    http = sp.get("health_http") if sp.get("health_http") is not None else sp.get("health_http_code")
    verdict = str(sp.get("inference_verdict") or "").strip()
    evidence_kind = str(sp.get("evidence") or "").strip()
    len_key = _SERVE_PROOF_PASS_EVIDENCE.get(evidence_kind)
    n = _as_int(sp.get(len_key)) if len_key else None
    why = (f"config={sp.get('config')!r} ready={sp.get('ready')!r} health={http!r} verdict={verdict!r} "
           f"evidence={evidence_kind!r} {len_key or 'len'}={n!r}")
    if sp.get("config") != cell or str(http) != "200" or sp.get("ready") is False:
        return False, why
    if verdict != "pass" or len_key is None or (n or 0) < 1:
        return False, why
    if not str(sp.get("provenance") or "").startswith("measured"):
        return False, why + " provenance 비관측"
    if sp.get("image_tag") and image_tag and sp["image_tag"] != image_tag:
        # 셀 id 재사용: 같은 이름의 다른 빌드 스모크(태그가 다르면 이 측정의 서빙이 아니다).
        return False, why + f" image_tag {sp['image_tag']!r} ≠ 측정 {image_tag!r}"
    return True, why


def serve_proof_scope(repo, ev: CellEvidence) -> dict | None:
    """발행 자격 serve_proof 의 **시점 범위**(2026-09-29 · FACT_FIX2 G2). serve_proof 파일은 같은 config 의 다음 스모크가 덮는다 — DS4F 는
    측정(00:35:13Z 끝) 뒤 재기동(01:31:48Z)의 산출물이 자격 근거로 범위 없이 실렸다. 작성 시각(문서 시각 칸 → 파일 mtime) 대 측정 창을
    attestation 과 **같은 함수**(`_attestation_timing`)로 분류한다. 반환 {written_utc, written_utc_source, timing, timing_note} | None."""
    repo = _repo(repo)
    rel = ev.sources.get("serve_proof")
    if not isinstance(ev.serve_proof, dict) or not isinstance(rel, str) or not (repo / rel).is_file():
        return None
    written, wsrc = _attestation_written(repo, {**ev.serve_proof, "_path": rel})
    timing, tnote, _win = _attestation_timing(repo, ev, written)
    if not tnote:
        return None
    return {"written_utc": written, "written_utc_source": wsrc, "timing": timing,
            "timing_note": tnote.replace("측정 뒤 재기동의 관측", "측정 뒤 재기동의 스모크 산출물 — 측정을 서빙한 기동의 자격 관측이 아니다")}


def qualification(ev: CellEvidence) -> dict:
    """발행 자격 = **관측**(X8). ① serve_proof ② 스윕(조인된 출력 평면)의 post_health(health 200 ∧ running ∧ ¬OOM) ∧
    같은 레벨 벤치 completed ≥ 1 ③ lite 셀의 lite_warm completed ≥ 1(lite_bench 의 health 200 선검사가 전제).
    어느 것도 없으면 **차단** HINT_QUALIFICATION_UNOBSERVED — 계약 §2 의 유일한 위협(서빙 실패를 성공으로 위장한 배포)."""
    repo = Path(ev.repo or ".")
    tried: list[str] = []
    sp = ev.serve_proof
    if isinstance(sp, dict):
        ok, why = _serve_proof_ok(sp, ev.cell, (ev.build_identity or {}).get("image_tag"))
        if ok:
            if sp.get("plane") == "native":
                # Never let a caller bypass native cleanup validation by constructing a
                # CellEvidence manually: qualification owns the final native safety gate.
                _native_producer_evidence(repo, ev.topology, ev.cell, sp, ev.sources.get("serve_proof"))
                return {"health_200": True, "inference_observed": True,
                        "sources": [ev.sources.get("serve_proof"), ev.sources.get("cleanup_attestation")],
                        "method": "native_serve_proof+cleanup_attestation"}
            out = {"health_200": True, "inference_observed": True, "sources": [ev.sources.get("serve_proof")],
                   "method": "serve_proof"}
            sc = serve_proof_scope(repo, ev)
            if sc:
                out["scope"] = sc
            return out
        tried.append(f"serve_proof 불성립({why})")
    sw = ev.sweep or {}
    if sw.get("binding") == "output":
        base = repo / sw["dir"]
        # `**` 는 0개 이상의 디렉터리 — level_NN/ 과 반복 축 level_NN/run_KK/ 를 함께 걷는다(run_bench.sh:352-372).
        for ph in sorted(set(base.glob(f"level_*/**/post_health_{ev.cell}.json"))):
            doc = _read_json_opt(ph, "post_health")
            if not isinstance(doc, dict):
                continue
            if str(doc.get("health_http_code")) != "200" or _boolish(doc.get("container_running")) is not True \
                    or _boolish(doc.get("container_oom_killed")) is True:
                tried.append(f"{_rel(repo, ph)} 불성립")
                continue
            bench = ph.parent / f"bench_{ev.cell}.json"
            bdoc = _read_json_opt(bench, "bench json") if core.level_raw_is_measured_tool(bench, ev.cell) else None
            if isinstance(bdoc, dict) and (_as_int(bdoc.get("completed")) or 0) >= 1:
                return {"health_200": True, "inference_observed": True,
                        "sources": [_rel(repo, ph), _rel(repo, bench)], "method": "post_health+bench"}
            tried.append(f"{_rel(repo, bench)} completed<1 또는 부재")
    elif sw:
        tried.append(f"스윕이 {sw.get('binding')} 로만 묶였다 — post_health 원시가 이 측정의 것이 아니다")
    if ev.report_kind == "lite":
        # 조인 키 = lite raw 의 `measured_utc`(render_report --lite-only 가 리포트 `생성일` 로 옮긴 그 값) + `config_name`.
        #   ★ 이름만으로 묶지 않는다 — 벤치로그 루트의 lite_* 는 같은 셀 id 의 다음 실행이 덮는다(셀 id 재사용 · G6 과 같은 결 ·
        #   2026-09-22 적대 검토: 옛 판본은 조인 없이 lite_warm 을 받아 다른 실행의 관측으로 자격을 열 수 있었다).
        born = _report_born_utc(repo, ev.bench_report_path)
        dirs = [repo / "output" / ev.topology / "benchlog"]
        if sw.get("binding") == "output":
            dirs.append(repo / sw["dir"])
        for d in dirs:
            raw = _read_json_opt(d / f"lite_raw_{ev.cell}.json", "lite_raw")
            if not isinstance(raw, dict):
                continue
            if born is None or raw.get("config_name") != ev.cell or raw.get("measured_utc") != born:
                tried.append(f"{_rel(repo, d / f'lite_raw_{ev.cell}.json')} 조인 불성립(measured_utc={raw.get('measured_utc')!r} "
                             f"≠ 리포트 생성일 {born!r} 또는 config_name={raw.get('config_name')!r})")
                continue
            warm = d / f"lite_warm_{ev.cell}.json"
            named = Path(str(raw.get("bench_warm_json") or "")).name
            if named and named != warm.name:
                tried.append(f"lite_raw 가 가리키는 warm 파일 {named!r} ≠ {warm.name}")
                continue
            doc = _read_json_opt(warm, "lite_warm")
            if isinstance(doc, dict) and (_as_int(doc.get("completed")) or 0) >= 1:
                return {"health_200": True, "inference_observed": True,
                        "sources": [_rel(repo, d / f"lite_raw_{ev.cell}.json"), _rel(repo, warm)],
                        "method": "lite_raw+lite_warm(measured_utc = 리포트 생성일 · lite_bench health 200 선검사 전제)"}
            tried.append(f"{_rel(repo, warm)} completed<1 또는 부재")
        if not any("lite_raw" in t for t in tried):
            tried.append(f"lite_raw_{ev.cell}.json 부재(조인 키 없음)")
    core.fail("HINT_QUALIFICATION_UNOBSERVED",
              f"셀 {ev.cell!r} 에 health 200 + 추론 1회 **관측**이 없다: {tried or ['관측 파일 0']}",
              f"스모크가 `output/{ev.topology}/benchlog/serve_proof_{ev.cell}.json`(pass · 생성 길이 ≥1)을 남기거나, 같은 "
              f"measured_utc 의 스윕(post_health + bench completed≥1) 또는 lite 셀이면 리포트 생성일과 같은 measured_utc 의 "
              f"`lite_raw_{ev.cell}.json` + lite_warm(completed≥1)이 이 노드의 출력 평면에 있어야 한다. 선언(manifest.runtime)은 "
              f"증거가 아니다. 서브에서 잰 셀은 메인이 그 원시를 볼 수 없다(문서기반 회수 only). 이번 해소의 측정 노드 = "
              f"{ev.node!r} — 인증서가 없는 셀(REFUTE·lite)은 노드 축이 배정(main)으로 해소돼 cluster 로 잰 증거 포인터가 "
              f"걸러질 수 있다: 실제 측정 노드가 다르면 `--node cluster|main|sub` 로 명시해 다시 실행한다(2026-09-22 통합 · "
              f"runtime_selftest 재현).")


# ── identity · 토폴로지 라벨 ─────────────────────────────────────────────────────────────────────────────────
def _measured_identity(repo: Path, ev: CellEvidence) -> tuple[dict, str, dict]:
    """강식별 6키의 **측정** 출처: 인증서 > 스윕 meta > 리포트 측정 환경 표. 읽지 못함 = None.
    반환 (6키, 출처, 키별 출처 덮어쓰기) — 리포트 표가 TP 를 들지 않아 서빙 yaml 선언을 쓴 경우 그 키의 출처를 따로 적는다."""
    if ev.certificate:
        return _identity_from_certificate(repo, ev.certificate), f"인증서({Path(ev.certificate_path or '').name})", {}
    meta = ((ev.sweep or {}).get("index") or {}).get("meta") or {}
    if meta:
        return _identity_from_meta(meta), f"sweep_index.meta({(ev.sweep or {}).get('dir')})", {}
    snap = _lite_snapshot(repo, ev.bench_report_path)
    if snap:
        six = _identity_from_snapshot(repo, ev, snap)
        tp_src = six.pop("_tp_source")
        return six, f"bench_report 측정 환경 표({Path(ev.bench_report_path).name})", ({"tp": tp_src} if tp_src else {})
    return {k: None for k in ("model", "gpu", "vllm", "quant", "topology", "tp")}, "관측 없음", {}


def _identity_from_meta(meta: dict) -> dict:
    return {"model": _norm_value(meta.get("model")), "gpu": _norm_value(meta.get("gpu_model")),
            "vllm": _norm_value(meta.get("vllm_version")), "quant": _norm_value(meta.get("quantization")),
            "topology": _norm_value(meta.get("topology")), "tp": _as_int(meta.get("tensor_parallel_size"))}


def _identity_from_snapshot(repo: Path, ev: CellEvidence, snap: dict) -> dict:
    """리포트 `## 측정 환경 스냅샷` → 6키. full 리포트 표는 `tensor_parallel_size` 를 든다(측정) — lite 표는 들지 않아서
    그때만 서빙 yaml 의 선언값을 쓰고, 그 사실을 `_tp_source` 에 남긴다(값 옆의 출처 · 표 출처로 뭉뚱그리지 않는다)."""
    tp = _as_int(_norm_value(snap.get("tensor_parallel_size")))
    tp_src = "bench_report 측정 환경 표 tensor_parallel_size" if tp is not None else None
    if tp is None and ev.triplet.get("yaml"):
        y = _owner_yaml(repo, repo / ev.triplet["yaml"], "서빙 yaml")
        tp = _as_int((y or {}).get("tensor-parallel-size")) if isinstance(y, dict) else None
        tp_src = f"서빙 yaml tensor-parallel-size({ev.triplet['yaml']} · 선언 — lite 표에 TP 칸이 없다)" if tp is not None else None
    return {"model": _norm_value(snap.get("model")), "gpu": _norm_value(snap.get("gpu_model")),
            "vllm": _norm_value(snap.get("vllm_version")), "quant": _norm_value(snap.get("quantization")),
            "topology": _norm_value(snap.get("topology")), "tp": tp, "_tp_source": tp_src}


def identity(ev: CellEvidence) -> dict:
    """PAYLOAD.identity — 강식별 6키(측정) + hf_repo/base_model/base_slug(체크포인트 관측) + source{필드: 출처}.
    `vllm` 은 엔진 자기보고(인증서 강키)다 — 이름의 vLLM 세그먼트(빌드 입력 · X18)와 다른 사실이다(코드맵 H3)."""
    from . import naming
    repo = _repo(ev.repo or ".")
    six, src6, per_key = _measured_identity(repo, ev)
    need = [k for k in ("model", "gpu", "vllm", "topology", "tp") if six.get(k) in (None, "")]
    if need:
        core.fail("HINT_IDENTITY_UNDERIVABLE", f"강식별 키 {need} 를 관측에서 파생하지 못했다({src6}).",
                  "인증서 또는 조인된 스윕 meta 가 있어야 한다 — 발행 기록의 손 identity 를 복사하지 않는다.")
    if six["topology"] != ev.topology:
        core.fail("HINT_IDENTITY_TOPOLOGY_MISMATCH", f"측정 topology={six['topology']!r} ≠ 셀 토폴로지 {ev.topology!r}")
    ck = _checkpoint(repo, ev)
    vocab = naming.load_vocab(repo)
    out = dict(six)
    out["hf_repo"] = ck.get("hf_repo")
    out["hf_revision"] = ck.get("hf_revision")
    out["base_model"] = ck.get("base_model")
    out["base_slug"] = naming.base_slug(six["model"], vocab)
    out["source"] = {**{k: per_key.get(k, src6) for k in six}, "hf_repo": ck.get("hf_repo_source"),
                     "hf_revision": ck.get("hf_revision_source") or ck.get("reason") or "체크포인트 미관측",
                     "base_model": ck.get("base_model_source"),
                     "base_slug": f"model({six['model']}) − vocab quant_suffixes(naming.base_slug)"}
    # 혼합 구성(2026-09-29 · plan_26092908 §4.1 U9): q 축은 선언 방식 하나 — 체크포인트가 실제로 든 dtype 구성은 여기 기계 필드로.
    out["quant_composition"], out["source"]["quant_composition"] = _quant_composition(ck)
    if out.get("quant") in (None, ""):
        # 2026-09-22(plan_26092119 S2 round 2 · F4): 인증서·스윕 meta 의 quantization 이 N/A 인데 이름의 q 축은 체크포인트 config 에서
        #   nvfp4 를 파생했다 — 같은 00 사실 블록이 "양자화 미관측" 과 "q=nvfp4" 를 함께 말했다(채점 AC3-b nit). 측정 identity(강식별
        #   키 · 발행기 identity.json · runtime 대조)는 **바꾸지 않고**(drive_publisher 는 측정값을 쓴다) PAYLOAD 표시만 이름 축으로
        #   채우며, 출처가 그 사실(측정 N/A · 이름 축 파생)을 말한다. 파생 불가면 None 그대로(지어내지 않는다).
        q = _quant_raw(ck)
        tok = None
        if isinstance(q.get("raw"), str) and q["raw"].strip():
            try:
                tok = naming.normalize("quant", q["raw"], vocab)
            except core.HintError:
                tok = None
        if tok:
            out["quant"] = tok
            out["source"]["quant"] = (f"naming-axis(q) — 측정 identity quantization 미관측({src6}) · q 축 = {q.get('source')} · "
                                      f"vocab quant[{tok}]←{q['raw']!r}")
    return out


def topology_label(identity) -> str:
    """X7 — 파생 라벨 `"<topology> TP=<tp>"` + multi 면 `"(Ray)"`(예 `multi TP=2(Ray)` · `single TP=1`).
    옛 자유 텍스트 `--topology` 라벨(`'multi 2노드 TP2'`)은 사람이 적어서 태그마다 모양이 달랐다."""
    top = (identity or {}).get("topology")
    tp = (identity or {}).get("tp")
    if top not in TOPOLOGIES or isinstance(tp, bool) or not isinstance(tp, int) or tp < 1:
        core.fail("HINT_TOPOLOGY_UNDERIVABLE", f"topology/tp 로 라벨을 만들 수 없다: {top!r}/{tp!r}")
    return f"{top} TP={tp}" + ("(Ray)" if top == "multi" else "")


# ── 체크포인트 관측(양자화·PLE·HF repo·base_model) ─────────────────────────────────────────────────────────────
def _serving_yaml(repo: Path, ev: CellEvidence) -> tuple[dict | None, str]:
    rel = ev.triplet.get("yaml")
    if not rel:
        return None, f"서빙 yaml 부재(output/{ev.topology}/configs/{ev.cell}.yaml)"
    doc = _owner_yaml(repo, repo / rel, "서빙 yaml")
    if not isinstance(doc, dict):
        core.fail("HINT_EVIDENCE_UNREADABLE", f"서빙 yaml 최상위가 사전이 아니다: {rel}")
    return doc, rel


def _checkpoint(repo: Path, ev: CellEvidence) -> dict:
    """서빙 yaml `model:`(컨테이너 경로) → 호스트 체크포인트(upstream check_smoke_model 규칙: env > manifest > compose 기본값).
    출처 문장에 **호스트 경로를 적지 않는다**(배포 평면 PII abs-op-path) — 컨테이너 경로와 manifest 필드 표지만."""
    out: dict = {"observed": False}
    y, yrel = _serving_yaml(repo, ev)
    model = (y or {}).get("model") if y else None
    if not isinstance(model, str) or not model.startswith("/"):
        out["reason"] = f"{yrel} 에 컨테이너 절대경로 model: 이 없다"
        out["hf_repo_source"] = out["base_model_source"] = out["reason"]
        return out
    out["model_path"] = model
    base = repo / "output" / ev.topology
    if ev.plane == "native":
        # native 평면(2026-09-23 · plan_26092311 N-D3): yaml `model:` 은 이미 **호스트** 경로다(컨테이너 마운트 없음 —
        #   render_native_triplet 이 manifest 경로 필드로 사상). 그 사상을 역으로 읽어 manifest 필드 표지로만 적는다
        #   (출처에 호스트 경로 ✗ · 위 docstring). 어느 manifest 경로 필드의 하위도 아니면 미해소로 남긴다(추측 ✗).
        mf = _owner_yaml(repo, base / "manifest.yaml", "manifest") if (base / "manifest.yaml").is_file() else None
        hits = sorted(((k, v.rstrip("/")) for k, v in (mf or {}).items()
                       if isinstance(k, str) and k.endswith("_path") and isinstance(v, str) and v.startswith("/")
                       and (model == v.rstrip("/") or model.startswith(v.rstrip("/") + "/"))),
                      key=lambda kv: -len(kv[1]))
        if not hits:
            out["reason"] = f"native model: 이 manifest 경로 필드의 하위가 아니다({yrel})"
            out["hf_repo_source"] = out["base_model_source"] = out["reason"]
            return out
        field, root = hits[0]
        out["host_label"] = f"native model: → <manifest.{field}>{model[len(root):]}"
        host = Path(model)
        if not host.is_dir():
            out["reason"] = f"체크포인트 호스트 경로 부재({out['host_label']})"
            out["hf_repo_source"] = out["base_model_source"] = out["reason"]
            return out
        out["model_path"] = f"<manifest.{field}>{model[len(root):]}"
        out["observed"] = True
        cfg = _read_json_opt(host / "config.json", "체크포인트 config.json")
        out["config"] = cfg if isinstance(cfg, dict) else None
        hq = _read_json_opt(host / "hf_quant_config.json", "hf_quant_config.json")
        out["hf_quant"] = hq if isinstance(hq, dict) else None
        out["hf_repo"], out["hf_repo_source"] = _hf_repo(host, out["host_label"])
        out["hf_revision"], out["hf_revision_source"] = _hf_revision(host, out["host_label"], _measured_key(repo, ev))
        out["base_model"], out["base_model_source"] = _base_model(host, out["host_label"])
        return out
    sm = _smoke_model(repo)
    root = "/".join(model.split("/")[:3])
    compose = base / "docker-compose.yaml"
    token = sm.read_app_models_host_token(str(compose), root) if compose.is_file() else None
    host_root = sm.resolve_app_models_host_root(token, str(base)) if token else None
    field = None
    fm = re.fullmatch(r"\$\{(\w+)(?::-[^}]*)?\}", token or "")
    if fm:
        field = fm.group(1).lower()
    label = f"<manifest.{field}>" if field else f"<compose:{root} 마운트>"
    out["host_label"] = f"{model} → {label}{model[len(root):]}"
    host = Path(model.replace(root, host_root, 1)) if host_root else None
    if host is None or not host.is_dir():
        out["reason"] = f"체크포인트 호스트 경로 미해소 또는 부재({out['host_label']})"
        out["hf_repo_source"] = out["base_model_source"] = out["reason"]
        return out
    out["observed"] = True
    cfg = _read_json_opt(host / "config.json", "체크포인트 config.json")
    out["config"] = cfg if isinstance(cfg, dict) else None
    hq = _read_json_opt(host / "hf_quant_config.json", "hf_quant_config.json")
    out["hf_quant"] = hq if isinstance(hq, dict) else None
    out["hf_repo"], out["hf_repo_source"] = _hf_repo(host, out["host_label"])
    out["hf_revision"], out["hf_revision_source"] = _hf_revision(host, out["host_label"], _measured_key(repo, ev))
    out["base_model"], out["base_model_source"] = _base_model(host, out["host_label"])
    return out


_GIT_REF_RE = re.compile(r"^refs/[A-Za-z0-9._/-]+$")


def _hf_revision(host: Path, label: str, measured: str | None = None) -> tuple[str | None, str]:
    """체크포인트를 받아 온 커밋 — 체크포인트 `.git` 의 HEAD(→ ref → loose ref 또는 packed-refs). 관측 불가 = None.
    2026-09-22(plan_26092119 S2 round 2 · F4): 1차 재생 채점에서 revision 이 "계보 어디에도 없다(미기록)" 로 적혔는데 체크포인트
    `.git/refs/heads/main` 에 40자 SHA 가 있었다 — hf_repo 를 같은 `.git` 에서 읽으면서 revision 은 읽지 않은 결함.
    ★ 이것은 **checkout 된 커밋**이다 — 작업트리 파일(LFS 가중치)이 그 커밋과 같은지는 검증하지 않는다(출처에 그렇게 적는다).
    git 은 부르지 않는다(stdlib 파일 읽기 · 체크포인트는 NAS 위라 git 이 느리거나 없을 수 있다)."""
    g = host / ".git"
    try:
        if g.is_file():                                # `gitdir: <경로>` 간접(worktree·submodule 모양)
            m = re.match(r"^gitdir:\s*(.+)$", g.read_text(encoding="utf-8", errors="replace").strip())
            g = (host / m.group(1).strip()).resolve() if m else g
        if not (g / "HEAD").is_file():
            return None, f"체크포인트 .git/HEAD 부재({label}) — revision 미관측"
        head = (g / "HEAD").read_text(encoding="utf-8", errors="replace").strip()
    except OSError as e:
        return None, f"체크포인트 .git/HEAD 를 읽지 못했다({label} · {type(e).__name__}) — revision 미관측"
    if _SHA40.match(head):
        return _revision_at(g, head, f"체크포인트 .git/HEAD(detached · {label}) — checkout 커밋(작업트리 일치는 미검증)", measured)
    m = re.match(r"^ref:\s*(\S+)$", head)
    ref = m.group(1) if m else ""
    if not _GIT_REF_RE.match(ref) or ".." in ref.split("/"):
        return None, f"체크포인트 .git/HEAD 가 SHA·ref 모양이 아니다({label}) — revision 미관측"
    sha, how = None, None
    loose = g / ref
    try:
        if loose.is_file():
            v = loose.read_text(encoding="utf-8", errors="replace").strip()
            if _SHA40.match(v):
                sha, how = v, "loose ref"
        packed: dict[str, str] = {}
        if (g / "packed-refs").is_file():
            for ln in (g / "packed-refs").read_text(encoding="utf-8", errors="replace").splitlines():
                parts = ln.strip().split(" ", 1)
                if len(parts) == 2 and _SHA40.match(parts[0]) and not ln.startswith(("#", "^")):
                    packed[parts[1].strip()] = parts[0]
    except OSError as e:
        return None, f"체크포인트 {ref} 를 읽지 못했다({label} · {type(e).__name__}) — revision 미관측"
    if sha is None and ref in packed:
        sha, how = packed[ref], "packed-refs"
    if sha is None:
        return None, f"체크포인트 HEAD → {ref} 를 loose ref·packed-refs 어디서도 해소하지 못했다({label}) — revision 미관측"
    branch = ref.rsplit("/", 1)[-1]
    remote = f"refs/remotes/origin/{branch}"
    rv = None
    try:
        rv = (g / remote).read_text(encoding="utf-8", errors="replace").strip() if (g / remote).is_file() else packed.get(remote)
    except OSError:
        rv = None
    tail = (f" · {remote} 와 일치(받아 온 원격 커밋)" if rv == sha else
            f" · {remote} 는 다른 커밋({str(rv)[:12]}) — 로컬 커밋일 수 있다" if rv else f" · {remote} 미관측")
    return _revision_at(g, sha, f"체크포인트 .git HEAD → {ref} → {how}({label}){tail} — checkout 커밋(작업트리 일치는 미검증)",
                        measured)


# reflog 한 줄 = `<old> <new> <이름> <<메일>> <epoch> <tz>\t<메시지>` — sha 둘과 epoch 만 읽는다(이름·메일은 캡처하지 않는다 · PII).
_REFLOG_RE = re.compile(r"^([0-9a-f]{40}) ([0-9a-f]{40}) .*? (\d{9,11}) [+-]\d{4}(?:\t|$)")


def _revision_at(g: Path, sha: str, desc: str, measured: str | None) -> tuple[str | None, str]:
    """지금의 HEAD 가 **측정 시점의 checkout** 과 같은가 — 체크포인트 reflog(`logs/HEAD`)로 대조한다(2026-09-22 · S2 round 2 리뷰).
    왜: HEAD 는 발행 시각에 읽는 값이다. 측정(예: 태그2 는 2026-09-12) 뒤 `git pull` 이 있었다면 지금의 HEAD 는 측정한 모델의
    revision 이 아니다 — 출처가 그것을 말하지 않으면 재현 키트가 다른 가중치를 가리킨다. 판정: reflog 에서 measured_utc 이전의 마지막
    이동이 가리킨 커밋 = 측정 시점 checkout. 그것이 지금 HEAD 와 같으면 그대로, 다르거나(측정 뒤 이동) 측정 뒤에 받아 왔으면 None
    (고르지 않는다 · 출처에 reflog 쪽 커밋 앞 12자를 적는다). reflog 가 없거나 측정 시각이 없으면 값은 두고 "미검증" 을 적는다."""
    if not measured:
        return sha, f"{desc} · 측정 시각 미관측 — 측정 시점 checkout 일치 미검증"
    p = g / "logs" / "HEAD"
    try:
        text = p.read_bytes().decode("utf-8", errors="replace") if p.is_file() else ""
    except OSError:
        text = ""
    moves: list[tuple[str, str]] = []
    for ln in text.split("\n"):
        m = _REFLOG_RE.match(ln)
        if m:
            moves.append((m.group(2), _dt.datetime.fromtimestamp(int(m.group(3)), tz=_dt.timezone.utc)
                          .strftime("%Y-%m-%dT%H:%M:%SZ")))
    if not moves:
        return sha, f"{desc} · reflog(logs/HEAD) 부재·판독 불가 — 측정 시점 checkout 일치 미검증"
    before = [s for s, u in moves if u <= measured]
    if not before:
        return None, (f"{desc} · 그러나 reflog 첫 항목 {moves[0][1]} 이 measured_utc {measured} 뒤다(측정 뒤 받아 온 checkout) — "
                      "측정한 revision 미관측(None · 지금 HEAD 를 고르지 않는다)")
    if before[-1] != sha:
        return None, (f"{desc} · 그러나 측정 시점 checkout(reflog · measured_utc {measured} 이전 마지막 이동) = {before[-1][:12]} ≠ 지금 "
                      "HEAD — 측정 뒤 이동: 측정한 revision 을 고르지 않는다(None)")
    later = sum(1 for _s, u in moves if u > measured)
    return sha, (f"{desc} · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc {measured} 이전 마지막 이동 "
                 f"{[u for _s, u in moves if u <= measured][-1]}" + (f" · 측정 뒤 이동 {later}회 있었으나 같은 커밋" if later else "") + ")")


def _hf_repo(host: Path, label: str) -> tuple[str | None, str]:
    """체크포인트를 받아 온 HF repo id — 체크포인트 `.git/config` 의 remote url(huggingface.co). 자격증명 부분은 버린다."""
    p = host / ".git" / "config"
    if not p.is_file():
        return None, f"체크포인트 .git/config 부재({label}) — HF repo 미관측"
    for line in p.read_text(encoding="utf-8", errors="replace").splitlines():
        s = line.strip()
        if s.startswith("url") and "=" in s:
            m = _HF_URL.match(s.split("=", 1)[1].strip())
            if m:
                return m.group(1), f"체크포인트 .git/config remote url(huggingface.co · {label})"
    return None, f"체크포인트 .git/config 에 huggingface.co remote 가 없다({label})"


def _base_model(host: Path, label: str) -> tuple[str | None, str]:
    """HF 카드(README 머리 YAML)의 `base_model` — 단일 값일 때만(다중이면 하나로 접지 않는다 · O3 families 대체 원천)."""
    p = host / "README.md"
    if not p.is_file():
        return None, f"체크포인트 README.md 부재({label})"
    lines = p.read_text(encoding="utf-8", errors="replace").splitlines()
    if not lines or lines[0].strip() != "---":
        return None, f"README 머리 YAML 없음({label})"
    vals: list[str] = []
    on = False
    for ln in lines[1:]:
        if ln.strip() == "---":
            break
        if re.match(r"^base_model\s*:", ln):
            rest = ln.split(":", 1)[1].strip()
            on = not rest
            if rest:
                vals.append(rest.strip("'\"[] "))
            continue
        if on:
            m = re.match(r"^\s*-\s*(.+)$", ln)
            if m:
                vals.append(m.group(1).strip().strip("'\""))
                continue
            on = False
    vals = [v for v in vals if re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*/[A-Za-z0-9][A-Za-z0-9._-]*", v)]
    if len(vals) == 1:
        return vals[0], f"체크포인트 README base_model({label})"
    if not vals:
        return None, f"README 머리 YAML 에 base_model 없음({label})"
    return None, f"README base_model 이 {len(vals)}개 — 단일 기반 모델로 접지 않는다({label})"


def _quant_raw(ck: dict) -> dict:
    """체크포인트 → q 축 raw. **규칙 = 체크포인트가 선언한 주 방식**(2026-09-29 · plan_26092908 §4.1 U9 · V10 명문화):
      ① `quantization_config.quant_method`(또는 hf_quant_config 의 modelopt) 가 선언한 방식을 그대로 쓴다 — 바이트 비중·층 dtype 을
         세어 다시 판정하지 않는다(DS4F: 선언 fp8 · routed expert 는 `expert_dtype: fp4` 로 바이트 대부분이지만 q = fp8).
      ② **보조 규칙** — modelopt `MIXED_PRECISION` 은 선언이 "혼합" 이라 한 방식을 말하지 않는다: 그때만 quantized_layers 의
         **우세 알고리즘**(층 개수 · 동률 = 파생 불가)을 쓴다(Qwen3.8 NVFP4 48/50).
      혼합 구성의 실제 모양(dtype 별 대상)은 이름에 넣지 않고 `identity.quant_composition`(_quant_composition)에 싣는다."""
    label = ck.get("host_label") or "체크포인트"
    if not ck.get("observed") or not isinstance(ck.get("config"), dict):
        return {"source": ""}      # raw 키 없음 = 파생 불가(읽지 못함 ≠ 없음)
    qc = ck["config"].get("quantization_config")
    hq = (ck.get("hf_quant") or {}).get("quantization") if isinstance(ck.get("hf_quant"), dict) else None
    if isinstance(qc, dict):
        method, algo, layers, where = str(qc.get("quant_method") or "").lower(), qc.get("quant_algo"), qc.get("quantized_layers"), "config.json quantization_config"
    elif isinstance(hq, dict):
        method, algo, layers, where = "modelopt", hq.get("quant_algo"), hq.get("quantized_layers"), "hf_quant_config.json quantization"
    else:
        return {"raw": "none", "source": f"{label}/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화"}
    if method == "modelopt":
        if str(algo or "").upper() == "MIXED_PRECISION":
            if not isinstance(layers, dict) and isinstance(hq, dict):
                layers = hq.get("quantized_layers")
                where += " + hf_quant_config.json quantized_layers"      # 층 표의 실제 출처를 적는다(출처 표시)
            algos = Counter((v.get("quant_algo") if isinstance(v, dict) else v) for v in (layers or {}).values()
                            if isinstance(v, (dict, str)))
            algos.pop(None, None)
            top = algos.most_common(2)
            if not top or (len(top) > 1 and top[0][1] == top[1][1]):
                return {"source": f"{label}/{where} MIXED_PRECISION quantized_layers 우세 알고리즘 판정 불가({dict(algos)})"}
            return {"raw": f"modelopt-dominant:{top[0][0]}",
                    "source": f"{label}/{where}(modelopt MIXED_PRECISION) quantized_layers 우세 {top[0][0]} "
                              f"{top[0][1]}/{sum(algos.values())}"}
        if algo:
            return {"raw": f"modelopt:{algo}", "source": f"{label}/{where} quant_method=modelopt quant_algo={algo}"}
        return {"source": f"{label}/{where} modelopt 인데 quant_algo 가 없다"}
    if method:
        return {"raw": method, "source": f"{label}/{where} quant_method={method}"}
    return {"source": f"{label}/{where} 에 quant_method 가 없다"}


def _quant_composition(ck: dict) -> tuple[list[dict] | None, str]:
    """체크포인트 config 에서 **관측되는** dtype 구성 → `[{scope, dtype, source[, count]}]`(2026-09-29 · plan_26092908 §4.1 U9 · V10).
    이름의 q 축(선언 방식 하나)이 말하지 못하는 혼합을 기계 필드로 싣는다 — 읽는 칸(닫힌 목록):
      · `quantization_config`(또는 hf_quant_config `quantization`) — quant_method/quant_algo(+ fmt · weight_block_size)
      · modelopt MIXED `quantized_layers` — 알고리즘별 층 개수(예시 모듈 1개) · `ignore`/`exclude_modules` — 비양자화 모듈 수
      · 최상위(또는 text_config) `expert_dtype` — routed expert dtype(DeepSeek 계열)
      · `torch_dtype`/`dtype` — 비양자화 가중치·계산 기본 dtype
    구성을 **해석하지 않는다**(바이트 비중 추정 ✗). 체크포인트 미관측 = (None, 사유). 출처에 호스트 경로를 적지 않는다(host_label)."""
    label = ck.get("host_label") or "체크포인트"
    cfg = ck.get("config") if ck.get("observed") else None
    if not isinstance(cfg, dict):
        return None, f"체크포인트 config 미관측({ck.get('reason') or label}) — 구성 미기재"
    out: list[dict] = []
    qc = cfg.get("quantization_config")
    hq = (ck.get("hf_quant") or {}).get("quantization") if isinstance(ck.get("hf_quant"), dict) else None
    where, q = ("config.json quantization_config", qc) if isinstance(qc, dict) else \
        ("hf_quant_config.json quantization", hq) if isinstance(hq, dict) else (None, None)
    if q is not None:
        method = str(q.get("quant_method") or ("modelopt" if where.startswith("hf_quant") else "")).lower()
        algo = q.get("quant_algo")
        layers = q.get("quantized_layers")
        if not isinstance(layers, dict) and isinstance(hq, dict):
            layers = hq.get("quantized_layers")
        if str(algo or "").upper() == "MIXED_PRECISION" and isinstance(layers, dict):
            by: dict[str, list[str]] = {}
            for name, v in layers.items():
                a = v.get("quant_algo") if isinstance(v, dict) else v if isinstance(v, str) else None
                if a:
                    by.setdefault(str(a), []).append(str(name))
            for a, names in sorted(by.items(), key=lambda kv: (-len(kv[1]), kv[0])):
                out.append({"scope": f"quantized_layers {len(names)}개(예 {names[0]})", "dtype": a, "count": len(names),
                            "source": f"{label}/{where} quantized_layers(modelopt MIXED_PRECISION)"})
        else:
            detail = " · ".join(f"{k}={q[k]}" for k in ("fmt", "weight_block_size", "group_size") if q.get(k) is not None)
            dtype = f"{method}:{algo}" if method == "modelopt" and algo else (method or str(algo or "")) or None
            if dtype:
                out.append({"scope": "quantization_config 선언 대상(양자화 층 · 제외 목록 밖)",
                            "dtype": dtype + (f" ({detail})" if detail else ""),
                            "source": f"{label}/{where} quant_method" + ("·quant_algo" if algo else "")})
        ign = q.get("ignore") if isinstance(q.get("ignore"), list) else q.get("exclude_modules") \
            if isinstance(q.get("exclude_modules"), list) else None
        if ign is None and isinstance(hq, dict) and isinstance(hq.get("exclude_modules"), list):
            ign = hq["exclude_modules"]
        if ign:
            out.append({"scope": f"양자화 제외 모듈 {len(ign)}개", "dtype": "unquantized(아래 기본 dtype)", "count": len(ign),
                        "source": f"{label}/{where} ignore·exclude_modules"})
    for d, dwhere in ((cfg, "config.json"), (cfg.get("text_config") if isinstance(cfg.get("text_config"), dict) else {},
                                                "config.json text_config")):
        if isinstance(d.get("expert_dtype"), str) and d["expert_dtype"].strip():
            out.append({"scope": "routed experts", "dtype": d["expert_dtype"].strip(), "source": f"{label}/{dwhere} expert_dtype"})
            break
    for k in ("torch_dtype", "dtype"):
        for d, dwhere in ((cfg, "config.json"), (cfg.get("text_config") if isinstance(cfg.get("text_config"), dict) else {},
                                                    "config.json text_config")):
            if isinstance(d.get(k), str) and d[k].strip():
                out.append({"scope": "기본 dtype(비양자화 가중치·계산)", "dtype": d[k].strip(), "source": f"{label}/{dwhere} {k}"})
                break
        else:
            continue
        break
    if not out:
        return [], f"{label}/config.json 에 양자화·dtype 칸이 없다(관측 · 빈 구성)"
    return out, f"{label}/config.json(+hf_quant_config.json) 관측 칸(evidence._quant_composition 닫힌 목록)"


def _ple_present(ck: dict) -> bool | None:
    cfg = ck.get("config") if ck.get("observed") else None
    if not isinstance(cfg, dict):
        return None
    for d in (cfg, cfg.get("text_config") if isinstance(cfg.get("text_config"), dict) else {}):
        if any(k in d for k in _PLE_KEYS):
            return True
    return False


# ── naming facts (SPEC §3.3) ─────────────────────────────────────────────────────────────────────────────────
def _yaml_get(y: dict, *keys):
    for k in keys:
        if k in y:
            return k, y[k]
    return None, None


def _same(a, b) -> bool:
    """측정 기록(문자열)과 yaml 값(yaml 타입)의 동치 — 수치는 값으로(`0.9` ≡ `0.90`), 참거짓은 참거짓으로, 그 밖은 글자로."""
    ba, bb = (a if isinstance(a, bool) else _boolish(str(a))), (b if isinstance(b, bool) else _boolish(str(b)))
    if ba is not None and bb is not None:
        return ba == bb
    try:
        return abs(float(str(a)) - float(str(b))) < 1e-9
    except ValueError:
        return str(a).strip().lower() == str(b).strip().lower()


def _measured_knobs(repo: Path, ev: CellEvidence) -> dict[str, tuple[object, str]]:
    """측정 때의 서빙 노브 기록 {키: (값, 출처)} — 스윕 meta(측정 시 조립) > 인증서(그 옮김) > 리포트 측정 환경 표.
    '읽지 못함' 표기는 싣지 않는다(부재 ≠ 불일치)."""
    out: dict[str, tuple[object, str]] = {}
    meta = ((ev.sweep or {}).get("index") or {}).get("meta") or {}
    layers = [(meta, "sweep meta"), (ev.certificate or {}, "인증서")]
    if not meta and not ev.certificate:
        layers.append((_lite_snapshot(repo, ev.bench_report_path), "리포트 측정 환경 표"))
    for doc, src in layers:
        for k, v in (doc or {}).items():
            if k not in out and not _unobserved(v):
                out[k] = (v, src)
    return out


def _drift_check(repo: Path, y: dict, yrel: str, env: dict, ev: CellEvidence) -> None:
    """디스크의 트리플렛이 **측정한 그것**인가 — 측정 기록(스윕 meta · 인증서 · 리포트 표)과 대조한다. 트리플렛은 비추적
    평면이라 측정 뒤에 바뀔 수 있고, 바뀐 값으로 이름을 지으면 거짓 정보가 된다(계약 §2 위협). 대조 불가(기록 부재·NA)는
    막지 않는다(부재 ≠ 불일치).
    ★ 2026-09-22 적대 검토: 옛 판본은 스윕 meta 가 있을 때 len·kv bytes·gmu·eager 만 봤다 — 이름 축인 kv dtype·spec·TP·
      모델 경로는 대조하지 않았고, 인증서만 있는 셀(스윕 미바인딩)은 아무것도 대조하지 않았다."""
    rec = _measured_knobs(repo, ev)
    bad = []
    pairs = (("max-model-len", "max_model_len", None), ("kv-cache-memory-bytes", "kv_cache_memory_bytes", None),
             ("gpu-memory-utilization", "gpu_memory_utilization", None), ("tensor-parallel-size", "tensor_parallel_size", None),
             ("max-num-seqs", "max_num_seqs", None), ("model", "model_path", None),
             # 기본값이 있는 축은 **실효값**으로 대조한다 — 줄이 지워져 기본값으로 돌아간 것도 드리프트다.
             ("kv-cache-dtype", "kv_cache_dtype", "auto"), ("enforce-eager", "enforce_eager", False))
    for yk, mk, default in pairs:
        if mk not in rec:
            continue
        if yk in y:
            got = y[yk]
        elif default is not None:
            got = default
        else:
            continue
        if not _same(got, rec[mk][0]):
            bad.append(f"{yk}: yaml={got!r} {rec[mk][1]}={rec[mk][0]!r}")
    spec_on = _boolish(str(rec["spec_on"][0])) if "spec_on" in rec else None
    if spec_on is not None:
        k, _v = _yaml_get(y, "speculative-config", "speculative_config")
        if (k is not None) != spec_on:
            bad.append(f"speculative-config: yaml {'있음' if k else '없음'} {rec['spec_on'][1]} spec_on={rec['spec_on'][0]!r}")
    decl_img = rec.get("image_tag_declared", (None, ""))[0]
    if env.get("IMAGE_TAG") and decl_img is not None and env["IMAGE_TAG"] != decl_img:
        bad.append(f"IMAGE_TAG: env={env['IMAGE_TAG']!r} meta.image_tag_declared={decl_img!r}")
    if bad:
        core.fail("HINT_TRIPLET_DRIFT", f"디스크 트리플렛({yrel})이 측정 때와 다르다: {bad}",
                  "측정한 그 트리플렛으로 되돌리거나, 바뀐 트리플렛으로 다시 서빙·측정한다(이름은 측정한 것에서 짓는다).")


def naming_facts(repo, ev: CellEvidence) -> dict:
    """SPEC §3.3 naming facts. 각 축은 `{raw, source}` — raw 키가 없거나 source 가 비면 naming 이 **파생 불가로 차단**한다
    (읽지 못함 ≠ 없음). 관측 > 선언이고, 두 관측이 어긋나면 이름을 짓지 않는다."""
    repo = _repo(repo)
    y, yrel = _serving_yaml(repo, ev)
    y = y or {}
    env = _env_first(repo, ev.triplet.get("env"))
    env_rel = ev.triplet.get("env") or f"output/{ev.topology}/envs/.env.{ev.cell}(부재)"
    if yrel and ev.triplet.get("yaml"):
        _drift_check(repo, y, yrel, env, ev)
    b = ev.build_identity or {}
    bsrc = b.get("source") or {}
    conflicts = b.get("env_history_conflicts") or {}
    if conflicts:
        core.fail("HINT_VLLM_INPUT_CONFLICT", f"셀 env 와 이미지 history build-arg 가 다르다: {conflicts}",
                  "측정 이미지가 다른 입력으로 빌드됐다 — 셀 env 를 실제 빌드 입력으로 바로잡거나 다시 빌드한다.")
    ck = _checkpoint(repo, ev)

    # vllm (X18 — 빌드 입력)
    from . import artifacts        # 평면 판정의 단일 소유자(artifacts.plane_of) — 여기서 다시 적지 않는다
    plane, plane_src = artifacts.plane_and_source(repo, ev)
    # native 인데 설치본이 소스빌드 이미지의 재포장이면(wheel URL 이 아니다) 빌드 입력은 원천 이미지의 트랙·ref 다 — 같은 입력의
    #   Docker 셀과 같은 vllm 세그먼트가 나온다(평면 구분은 arch `-bare` 가 한다 · O-N1).
    track = (b.get("track") if (plane == "native" and b.get("native_wheelhouse_source") and b.get("track"))
             else "native" if plane == "native" else b.get("track"))
    # D-h(2026-09-22): vllm_repo 는 **언제나 키로** 싣는다(값 None = 미관측) — 소스빌드의 릴리스 모양 ref 가 업스트림 태그인지는
    #   naming 이 저장소로 판정한다(포크 = SHA 경로 또는 차단). 키가 빠지면 naming 이 파생 불가로 막는다(읽지 못함 ≠ 업스트림).
    vllm = {"track": track, "vllm_ref": b.get("vllm_ref"), "vllm_version": b.get("vllm_version"),
            "vllm_repo": b.get("vllm_repo"),
            "vllm_sha": b.get("vllm_sha"), "prev_release": _prev_release(ev), "wheel_url": b.get("wheel_url"),
            "source": " · ".join(s for s in (f"track={bsrc.get('track')}", f"ref={bsrc.get('vllm_ref')}",
                                              f"version={bsrc.get('vllm_version')}", f"repo={bsrc.get('vllm_repo', '미관측')}",
                                              f"sha={bsrc.get('vllm_sha', '미관측')}")
                                 if s)}
    model = {"hf_repo": ck.get("hf_repo"), "model_path": ck.get("model_path"),
             "source": f"서빙 yaml model({yrel})" + (f" · hf_repo={ck.get('hf_repo_source')}" if ck.get("hf_repo") else "")}
    if not ck.get("model_path"):
        model["source"] = ""
    arch = _arch_facts(repo, ev)
    q = _quant_raw(ck)
    facts: dict = {"vllm": vllm, "model": model, "arch": arch, "q": q}
    k, v = _yaml_get(y, "max-model-len", "max_model_len")
    facts["len"] = {"raw": _as_int(v) if _as_int(v) is not None else v, "source": f"서빙 yaml {k}({yrel})"} if k \
        else {"source": f"서빙 yaml 에 max-model-len 없음({yrel}) — 엔진 기본값(모델 최대)은 이름으로 쓰지 않는다"}
    k, v = _yaml_get(y, "kv-cache-dtype", "kv_cache_dtype")
    facts["kv"] = {"raw": str(v), "source": f"서빙 yaml {k}({yrel})"} if k else \
        ({"raw": "auto", "source": f"서빙 yaml kv-cache-dtype 부재 관측({yrel}) → 엔진 기본값 auto"} if ev.triplet.get("yaml")
         else {"source": ""})
    facts["spec"] = _spec_fact(repo, ev, y, yrel)
    facts["graph"] = _graph_fact(repo, ev, y, yrel)
    facts["ple"] = _ple_fact(ev, env, env_rel, ck)
    # 평면(O-N1 · 2026-09-23 plan_26092311): arch 선택 토큰의 원문. 판정은 위 artifacts 한 벌(셀 env 선택자 · native serve-proof
    #   선언의 교차 대조 포함)이고, 여기서는 그 결과와 출처만 싣는다(두 번째 판정기 ✗).
    psrc = f"artifacts.plane_of → {plane_src}"
    if plane == "native":
        psrc += f" · env: IMAGE_TAG/BUILD_DOCKERFILE 없음({env_rel})"
        if ev.sources.get("plane"):
            psrc += f" · serve_proof {ev.sources['plane']}"
    facts["plane"] = {"raw": plane, "source": psrc}
    return facts


# ── 이름 꼬리 근거 대조 입력(2026-09-29 · plan_26092908 §4.1 U6) ─────────────────────────────────────────────────────────
_SH_FLAG_RE = re.compile(r"^--([A-Za-z0-9][A-Za-z0-9_-]*)(?:=(.*))?$")
_SH_ASSIGN_RE = re.compile(r"^(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$")


def _flat_str(v) -> str:
    """평탄화 값 → 문자열(대조는 naming.validate_tail 이 정규화한다 — 여기서는 표면만 고정: bool 소문자 · None 빈 문자열)."""
    if isinstance(v, bool):
        return "true" if v else "false"
    if v is None:
        return ""
    if isinstance(v, (dict, list)):
        return json.dumps(v, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return str(v)


def _flatten(prefix: str, v, out: dict) -> None:
    """중첩 yaml → `a.b` 평탄 키. 목록은 `a.0` · 문자열에 든 JSON 객체(`speculative-config: '{"…"}'`)도 한 단계 펼친다 — 원 키는
    원문 문자열로 남긴다(근거 대조가 원문과 펼친 칸 둘 다 받게)."""
    if isinstance(v, dict):
        for k, x in v.items():
            _flatten(f"{prefix}.{k}" if prefix else str(k), x, out)
        return
    if isinstance(v, list):
        out[prefix] = _flat_str(v)
        for i, x in enumerate(v):
            _flatten(f"{prefix}.{i}", x, out)
        return
    out[prefix] = _flat_str(v)
    if isinstance(v, str) and v.strip().startswith("{"):
        try:
            doc = json.loads(v)
        except ValueError:
            return
        if isinstance(doc, dict):
            for k, x in doc.items():
                _flatten(f"{prefix}.{k}", x, out)


def _flat_env(text: str) -> dict:
    out: dict = {}
    for ln in text.splitlines():
        s = ln.strip()
        if not s or s.startswith("#"):
            continue
        m = _SH_ASSIGN_RE.match(s)
        if m:
            val = m.group(2).strip()
            if len(val) >= 2 and val[0] == val[-1] and val[0] in "'\"":
                val = val[1:-1]
            out[m.group(1)] = val
    return out


def _flat_sh(text: str) -> dict:
    """러너 .sh → {`--flag`: 값}(CLI 인자 `--k v`·`--k=v` · 값 없는 플래그 = "true") + 변수 대입 `K=V`. 셸을 실행하지 않는다 —
    `$VAR` 확장은 원문 그대로 둔다(근거 대조는 원문과 대조한다 · 확장값을 지어내지 않는다)."""
    out: dict = {}
    for ln in text.splitlines():
        s = ln.strip()
        if not s or s.startswith("#"):
            continue
        m = _SH_ASSIGN_RE.match(s)
        if m and " " not in m.group(2).strip():
            out[m.group(1)] = m.group(2).strip().strip("'\"")
        try:
            toks = shlex.split(s, comments=True)
        except ValueError:
            continue
        for i, t in enumerate(toks):
            fm = _SH_FLAG_RE.match(t)
            if not fm:
                continue
            if fm.group(2) is not None:
                out[f"--{fm.group(1)}"] = fm.group(2)
            elif i + 1 < len(toks) and not toks[i + 1].startswith("--") and toks[i + 1] not in ("\\", "|", "&&", ";"):
                out[f"--{fm.group(1)}"] = toks[i + 1]
            else:
                out[f"--{fm.group(1)}"] = "true"
    return out


def tail_sources(repo, ev: CellEvidence) -> dict[str, dict]:
    """이 셀의 **서빙 설정 파일** → {저장소 상대경로: {평탄 key: 값(str)}}(2026-09-29 · plan_26092908 §4.1 U6 · 계약 B-5).
    naming.validate_tail(tail, sources) 의 근거 대조 입력이다 — 꼬리 토큰의 `evidence.file` 은 이 사전의 키 중 하나여야 하고 `key` 가
    그 파일에 실재하며 값이 일치해야 한다(거짓 꼬리만 막는다 · 규칙 목록을 늘리지 않는다).
      · 서빙 yaml(`output/<t>/configs/<셀>.yaml` · 중첩 `a.b` · 문자열 JSON 한 단계 펼침) · 셀 env(`output/<t>/envs/.env.<셀>` · K=V)
      · 러너 .sh(`--flag` → 값) · 캠페인 셀 config(`campaigns/<id>/cells/<셀>/config.yaml` · 캠페인 모드이고 인스턴스가 있을 때만)
    native 셀도 같은 트리플렛 경로다(render_native_triplet 이 같은 자리에 쓴다 · ev.triplet 한 벌). YAML 은 소유 로더로만 읽는다.
    읽지 못한 파일은 싣지 않는다(빈 사전으로 접지 않는다 — 없는 파일을 근거로 쓰면 대조가 파일 부재로 막힌다)."""
    repo = _repo(repo)
    out: dict[str, dict] = {}
    for kind, rel in sorted((ev.triplet or {}).items()):
        p = repo / rel
        if not p.is_file():
            continue
        if kind == "yaml":
            doc = _owner_yaml(repo, p, "서빙 yaml")
            if isinstance(doc, dict):
                flat: dict = {}
                _flatten("", doc, flat)
                out[rel] = flat
        elif kind == "env":
            out[rel] = _flat_env(p.read_text(encoding="utf-8", errors="replace"))
        elif kind == "sh":
            out[rel] = _flat_sh(p.read_text(encoding="utf-8", errors="replace"))
    if ev.mode == "campaign" and ev.campaign_id:
        rel = f"campaigns/{ev.campaign_id}/cells/{ev.cell}/config.yaml"
        if (repo / rel).is_file():
            doc = _owner_yaml(repo, repo / rel, "셀 config")
            if isinstance(doc, dict):
                flat = {}
                _flatten("", doc, flat)
                out[rel] = flat
    return out


def _ledger_vllm(att) -> dict:
    """attestation 이 든 **메인** 빌드 원장의 `vllm` 사전(v1 최상위 `build_ledger` 또는 v2 `nodes.main.build_ledger`) · 없으면 {}."""
    att = att or {}
    for led in (att.get("build_ledger"), ((att.get("nodes") or {}).get("main") or {}).get("build_ledger")
                if isinstance(att.get("nodes"), dict) else None):
        if isinstance(led, dict) and isinstance(led.get("vllm"), dict):
            return led["vllm"]
    return {}


def _prev_release(ev: CellEvidence) -> str | None:
    """커밋 핀의 직전 릴리스 — 빌드 원장 `vllm.describe` 만(이미지 안 git describe 는 docker run 이라 여기서 부르지 않는다)."""
    d = _ledger_vllm(ev.attestation).get("describe")
    return str(d) if d else None


def _arch_facts(repo: Path, ev: CellEvidence) -> dict:
    man = ev.manifest or {}
    msrc = ev.sources.get("manifest") or ""
    if not man:
        return {"gpu_model": None, "source": msrc}
    if man.get("topology") not in (None, ev.topology):
        core.fail("HINT_TOPOLOGY_MANIFEST_MISMATCH", f"manifest topology={man.get('topology')!r} ≠ 셀 토폴로지 {ev.topology!r}",
                  "토폴로지 사실의 권위는 manifest 다 — 이 체크아웃 통로를 확인한다(4자일치).")
    nodes = [n for n in man.get("nodes") or [] if isinstance(n, dict)]
    gpn = _as_int(man.get("gpus_per_node"))
    role = ev.node
    gpu = man.get("gpu_model")
    if role == "cluster":
        count = len(nodes)
        if count < 2:
            core.fail("HINT_AXIS_UNDERIVABLE", f"cluster 축인데 manifest nodes[] 가 {count}개다.")
    else:
        count = 1
        if role == "sub":
            entry = next((n for n in nodes if n.get("role") == "sub"), {})
            gpu = entry.get("gpu_model") or None
    measured = None
    if ev.certificate:
        measured, msrc2 = _norm_value(ev.certificate.get("gpu_model")), "인증서 gpu_model"
    else:
        measured = _norm_value((((ev.sweep or {}).get("index") or {}).get("meta") or {}).get("gpu_model"))
        msrc2 = "sweep meta gpu_model"
    if measured and gpu and str(measured).strip().lower() != str(gpu).strip().lower():
        core.fail("HINT_ARCH_GPU_CONFLICT", f"manifest gpu_model={gpu!r} ≠ {msrc2}={measured!r} — 다른 하드웨어의 측정이다.")
    # 측정 TP ↔ arch 형상 대조 — **모든 노드 축**에서. 옛 판본은 cluster 에서만, 그것도 인증서가 있을 때만 봤다. 그래서
    #   인증서 없는 셀(lite·REFUTE)은 배정 노드(sub)로 축이 정해지고 TP=2 측정이 `1g1n-sub` 로 이름 붙었다(2026-09-22 적대
    #   검토 · 라이브 camp-26091717 gpt-oss-20b-gb10·nv4-bf-262k-res — 두 셀 모두 topology multi · TP=2 측정). 한 노드가
    #   가진 GPU 보다 큰 TP 는 단일 노드 형상일 수 없다 — 이름이 거짓을 말하기 전에 막는다.
    six, six_src, per_key = _measured_identity(repo, ev)
    tp, tp_src = six.get("tp"), per_key.get("tp", six_src)
    if tp is None and ev.triplet.get("yaml"):
        y, yrel = _serving_yaml(repo, ev)
        tp, tp_src = _as_int((y or {}).get("tensor-parallel-size")), f"서빙 yaml tensor-parallel-size({yrel})"
    if tp is not None and gpn:
        if role == "cluster" and tp != count * gpn:
            core.fail("HINT_ARCH_TP_CONFLICT", f"cluster {count}노드×{gpn}GPU ≠ 측정 TP={tp}({tp_src})")
        if role in ("main", "sub") and tp > gpn:
            core.fail("HINT_ARCH_TP_CONFLICT",
                      f"발행 노드 축 {role!r}(1노드×{gpn}GPU)인데 측정 TP={tp}({tp_src}) — 단일 노드 형상이 아니다.",
                      "두 노드를 묶은 측정이면 `--node cluster` 로 발행한다(한 태그 = 한 노드 형상의 실행 기록).")
    arch = {"gpu_model": gpu, "gpus_per_node": gpn, "nodes": count, "role": role,
            "source": f"{msrc} gpu_model·gpus_per_node·nodes[] · 발행 노드 축({ev.sources.get('node')})"
                      + (f" · {msrc2} 일치" if measured else "")}
    tgt, tsrc = _target_gpu(ev, repo)
    if tsrc is not None:
        arch["target_gpu"] = tgt
        arch["source"] += f" · target: {tsrc}"
    return arch


def _target_gpu(ev: CellEvidence, repo=None) -> tuple[str | None, str | None]:
    """시뮬레이션 타겟 선언. 셀 config target_gpu.gpu_model > 캠페인 control_variables.target_gpu > 루브릭이 계산된 HW
    (roofline.json gpu_model). 어느 것도 관측하지 못하면 (None, None) — naming 이 파생 불가로 막는다(읽지 못함 ≠ 없음)."""
    cfg = ev.cell_config or {}
    tg = cfg.get("target_gpu") if isinstance(cfg.get("target_gpu"), dict) else None
    if tg is not None:
        v = tg.get("gpu_model")
        if isinstance(v, str) and v.strip() and "<<FILL>>" not in v:
            return v.strip(), "셀 config target_gpu.gpu_model(선언)"
        if v is None:
            return None, "셀 config target_gpu.gpu_model 이 null — no-simulation-target-declared"
    cv = (ev.declaration or {}).get("control_variables") or {}
    v = cv.get("target_gpu")
    if isinstance(v, str) and v.strip() and "<<FILL>>" not in v:
        return v.strip(), "캠페인 control_variables.target_gpu(선언)"
    if ev.mode == "campaign":
        # 캠페인 선언은 읽을 수 있는 평면이다 — 거기서 타겟을 읽지 못했으면 **읽지 못함**이지 native 가 아니다.
        return None, None
    # 재생(캠페인 purge) 전용: 타겟 선언이 사라졌다. roofline.json 의 gpu_model 은 **루브릭을 계산한 HW** 다(roofline.py 가
    #   manifest gpu_model 에서 읽는다 = 측정 호스트). 이것은 "선언" 이 아니라 "이 측정의 성능 기대가 호스트 HW 로 계산됐다" 는
    #   관측이고, 출처에 그렇게 적는다(2026-09-22 적대 검토: 옛 표지는 '루브릭 기준 HW' 만 적어 선언처럼 읽혔다).
    roof = (ev.sweep or {}).get("roofline") or {}
    if isinstance(roof, dict) and isinstance(roof.get("gpu_model"), str) and roof["gpu_model"].strip():
        return roof["gpu_model"].strip(), (f"{(ev.sweep or {}).get('dir')}/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 "
                                           "manifest) · 캠페인 purge 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님)")
    # 재생 전용 2순위(2026-09-29 · plan_26092908 S6): lite-only 셀은 스윕 roofline 이 없다(D2 재생이 arch 파생 불가로 막혔다).
    #   발행 기록이 이미 봉인한 페이로드 커밋(promotion_target.anchor)의 PAYLOAD.naming 은 **발행 당시 셀 config 선언을 관측한 값**이다
    #   — 그 target 이 native 면 "타겟 선언 없음(호스트 HW)" 으로 읽는다. sim-<x> 는 원문 hw 를 되살릴 수 없어 대체하지 않는다.
    pub = ev.publication if isinstance(ev.publication, dict) else {}
    pt = next((d.get("promotion_target") for d in (pub.get("record"), pub.get("manifest"), pub)
               if isinstance(d, dict) and isinstance(d.get("promotion_target"), dict)), None) or {}
    anchor = pt.get("anchor")
    if repo is not None and isinstance(anchor, str) and re.fullmatch(r"[0-9a-f]{40}", anchor):
        r = core.git(Path(repo), "show", f"{anchor}:PAYLOAD.json", check=False)
        try:
            ax = ((json.loads(r.stdout).get("naming") or {}).get("axes") or {}).get("target") if r.returncode == 0 else None
        except ValueError:
            ax = None
        if isinstance(ax, dict) and ax.get("value") == "native":
            return None, (f"봉인된 발행 페이로드 {anchor[:12]}:PAYLOAD.json naming.axes.target=native(발행 당시 셀 config 선언 관측) · "
                          "캠페인 purge 로 선언 원본 미관측(재생 전용 대체)")
    return None, None


def _sealed_payload(repo, ev: CellEvidence) -> tuple[dict | None, str | None]:
    """재생(캠페인 purge) 전용: 발행 기록의 `promotion_target.anchor`(봉인된 페이로드 커밋)의 PAYLOAD.json. (문서, 앵커) | (None, None)."""
    pub = ev.publication if isinstance(ev.publication, dict) else {}
    pt = next((d.get("promotion_target") for d in (pub.get("record"), pub.get("manifest"), pub)
               if isinstance(d, dict) and isinstance(d.get("promotion_target"), dict)), None) or {}
    anchor = pt.get("anchor")
    if repo is None or not isinstance(anchor, str) or not re.fullmatch(r"[0-9a-f]{40}", anchor):
        return None, None
    r = core.git(Path(repo), "show", f"{anchor}:PAYLOAD.json", check=False)
    if r.returncode != 0:
        return None, anchor
    try:
        doc = json.loads(r.stdout)
    except ValueError:
        return None, anchor
    return (doc if isinstance(doc, dict) else None), anchor


def sealed_publication(repo, ev: CellEvidence) -> dict | None:
    """재생 전용 대체(2026-09-29 · plan_26092908 §4.5 F4): 캠페인 인스턴스가 purge 돼 캠페인 id · 승인이 관측되지 않는 재생에서, 발행 기록의
    봉인 페이로드(promotion_target.anchor · 원 발행이 봉인한 커밋)가 기록한 `campaign`(id · cell · node) · `approval` 을 **출처를 붙여** 돌려준다
    (`_target_gpu` 의 재생 2순위와 같은 결). 셀 · 노드가 이 재생과 다르면 쓰지 않는다(다른 셀의 기록). 캠페인 모드 · 앵커 없음 = None.
    반환 {campaign: {id, cell, node} | None, approval: {...} | None, anchor, source}."""
    if ev.mode == "campaign":
        return None
    doc, anchor = _sealed_payload(repo, ev)
    if doc is None:
        return None
    camp = doc.get("campaign") if isinstance(doc.get("campaign"), dict) else {}
    if camp.get("cell") not in (None, ev.cell) or camp.get("node") not in (None, ev.node):
        return None
    src = (f"봉인된 발행 페이로드 {anchor[:12]}:PAYLOAD.json(발행 기록 promotion_target.anchor · 원 발행 당시 기록) · "
           "캠페인 purge 로 원본 미관측(재생 전용 대체)")
    ap = doc.get("approval") if isinstance(doc.get("approval"), dict) and doc.get("approval") else None
    cid = camp.get("id") if isinstance(camp.get("id"), str) and camp.get("id") else None
    return {"campaign": {"id": cid, "cell": camp.get("cell"), "node": camp.get("node")} if cid else None,
            "approval": {k: ap.get(k) for k in ("approved_by", "approved_utc", "source") if k in ap} if ap else None,
            "anchor": anchor, "source": src}


def _spec_fact(repo: Path, ev: CellEvidence, y: dict, yrel: str) -> dict:
    k, v = _yaml_get(y, "speculative-config", "speculative_config")
    sh_rel = ev.triplet.get("sh")
    sh_text = (repo / sh_rel).read_text(encoding="utf-8", errors="replace") if sh_rel else ""
    in_sh = re.search(r"--speculative[-_]config\b|--num[-_]speculative[-_]tokens\b", sh_text or "") is not None
    if k is None and in_sh:
        return {"source": f"러너({sh_rel})가 speculative 인자를 CLI 로 넘긴다 — 파싱하지 않는다(이름을 추측하지 않는다)"}
    if k is None:
        if not ev.triplet.get("yaml"):
            return {"source": ""}
        return {"raw": None, "source": f"서빙 yaml·러너에 speculative-config 부재 관측({yrel})"}
    if in_sh:
        core.fail("HINT_SPEC_CONFLICT", f"speculative 설정이 yaml({yrel})과 러너({sh_rel}) 두 곳에 있다.")
    cfg = v
    if isinstance(v, str):
        try:
            cfg = json.loads(v)
        except ValueError:
            return {"source": f"서빙 yaml {k} 가 JSON 이 아니다({yrel})"}
    n = cfg.get("num_speculative_tokens") if isinstance(cfg, dict) else None
    if _as_int(n) is None or _as_int(n) < 1:
        return {"source": f"서빙 yaml {k} 에 num_speculative_tokens(≥1)가 없다({yrel})"}
    # 구조화 근거(2026-09-29 · plan_26092908 §4.1): naming.tail_candidates 가 꼬리 후보의 evidence 로 싣는다 — tail_sources 와 같은 평탄 키.
    return {"raw": _as_int(n), "source": f"서빙 yaml {k}.num_speculative_tokens({yrel} · method={cfg.get('method')})",
            "evidence": {"file": yrel, "key": f"{k}.num_speculative_tokens", "value": _flat_str(n)}}


def _graph_fact(repo: Path, ev: CellEvidence, y: dict, yrel: str) -> dict:
    if not ev.triplet.get("yaml"):
        return {"source": ""}
    k, v = _yaml_get(y, "enforce-eager", "enforce_eager")
    sh_rel = ev.triplet.get("sh")
    sh_text = (repo / sh_rel).read_text(encoding="utf-8", errors="replace") if sh_rel else ""
    if k is not None:
        b = v if isinstance(v, bool) else _boolish(str(v))
        if b is None:
            return {"source": f"서빙 yaml {k}={v!r} 를 참/거짓으로 읽지 못했다({yrel})"}
        return {"raw": "eager" if b else "graph", "source": f"서빙 yaml {k}: {str(b).lower()}({yrel})",
                "evidence": {"file": yrel, "key": k, "value": _flat_str(v)}}
    m_eager = re.search(r"--enforce[-_]eager\b", sh_text or "")
    if m_eager:
        return {"raw": "eager", "source": f"러너 --enforce-eager({sh_rel})",
                "evidence": {"file": sh_rel, "key": m_eager.group(0), "value": _flat_sh(sh_text).get(m_eager.group(0), "true")}}
    k2, cc = _yaml_get(y, "compilation-config", "compilation_config")
    if k2 is not None:
        doc = cc
        if isinstance(cc, str):
            try:
                doc = json.loads(cc)
            except ValueError:
                return {"source": f"서빙 yaml {k2} 가 JSON 이 아니다({yrel})"}
        mode = str((doc or {}).get("cudagraph_mode") or "").upper() if isinstance(doc, dict) else ""
        if mode == "NONE":
            return {"raw": "eager", "source": f"서빙 yaml {k2}.cudagraph_mode=NONE({yrel})",
                    "evidence": {"file": yrel, "key": f"{k2}.cudagraph_mode", "value": str(doc.get("cudagraph_mode"))}}
    return {"raw": "graph", "source": f"서빙 yaml enforce-eager 부재 관측({yrel}) → 엔진 기본값 cudagraph"}


def _ple_fact(ev: CellEvidence, env: dict, env_rel: str, ck: dict) -> dict:
    """PLE 축. **serve 시점의 셀 env 가 권위**(sweep_bench R9 주석 · 2026-09-11) + 모델 config 의 PLE 실재.
    PLE 없는 모델은 env 와 무관하게 `none`(sweep_bench 의 `resident` 기본값은 PLE 없는 모델에서 거짓이다).
    env 를 읽지 못하면 셀 선언(declared_axes.ple_mode)으로, 스윕 meta 와 mmap 여부가 어긋나면 이름을 짓지 않는다."""
    present = _ple_present(ck)
    label = ck.get("host_label") or "체크포인트"
    meta = ((ev.sweep or {}).get("index") or {}).get("meta") or {}
    decl = ((ev.cell_config or {}).get("declared_axes") or {}).get("ple_mode") if isinstance(ev.cell_config, dict) else None
    if present is False and env.get("VLLM_PLE_MMAP") == "1":
        # 명시한 mmap 과 "PLE 없음" 관측이 모순이다 — 어느 쪽이 틀렸는지(낡은 env 인지 · 이 판정 키 목록이 모르는 PLE 형식인지)
        #   발행기가 고르지 않는다. 옛 판본은 조용히 `none` 으로 접었다(2026-09-22 적대 검토).
        core.fail("HINT_PLE_CONFLICT", f"셀 env VLLM_PLE_MMAP=1({env_rel}) 인데 모델 config 에 PLE 키{list(_PLE_KEYS)} 가 없다({label}).",
                  "PLE 없는 모델이면 셀 env 의 VLLM_PLE_MMAP 줄을 지우고 다시 측정한다 · PLE 모델이면 evidence._PLE_KEYS "
                  "(tripwire 목록)에 그 형식의 키를 등재한다.")
    if present is False:
        out = {"raw": "none", "source": f"모델 config PLE 키{list(_PLE_KEYS)} 부재 관측({label})"}
    elif ev.triplet.get("env"):
        mm = env.get("VLLM_PLE_MMAP")
        if mm == "1":
            out = {"raw": "mmap", "source": f"셀 env VLLM_PLE_MMAP=1({env_rel})",
                   "evidence": {"file": env_rel, "key": "VLLM_PLE_MMAP", "value": "1"}}
        elif present is True:
            out = {"raw": "resident", "source": f"셀 env VLLM_PLE_MMAP={mm!r}(부재/0 = stock resident · {env_rel}) · 모델 PLE 실재({label})"}
            if mm is not None:
                out["evidence"] = {"file": env_rel, "key": "VLLM_PLE_MMAP", "value": str(mm)}
        elif isinstance(decl, str) and decl.strip() and "<<FILL>>" not in decl:
            out = {"raw": decl.strip(), "source": "셀 config declared_axes.ple_mode(선언 · 모델 config 미관측)"}
        else:
            out = {"source": f"모델 config 미관측이라 PLE 실재를 판정하지 못했다({label}) — env 에 mmap 도 없다"}
    elif isinstance(decl, str) and decl.strip() and "<<FILL>>" not in decl:
        out = {"raw": decl.strip(), "source": "셀 config declared_axes.ple_mode(선언 · 셀 env 부재)"}
    elif not _unobserved(meta.get("ple_mode")):
        out = {"raw": meta["ple_mode"], "source": f"sweep meta ple_mode({meta.get('ple_mode_source')})"}
    else:
        out = {"source": ""}
    if str(out.get("source", "")).startswith("셀 config declared_axes.ple_mode") and ev.mode == "campaign" and ev.campaign_id:
        out["evidence"] = {"file": f"campaigns/{ev.campaign_id}/cells/{ev.cell}/config.yaml", "key": "declared_axes.ple_mode",
                           "value": str(decl).strip()}
    if "raw" in out and not _unobserved(meta.get("ple_mode")) and str(meta.get("ple_mode_source", "")).startswith("declared") \
            and (meta["ple_mode"] == "mmap") != (out["raw"] == "mmap"):
        core.fail("HINT_PLE_CONFLICT", f"PLE 축이 어긋난다: 파생={out['raw']!r}({out['source']}) · 스윕 meta={meta['ple_mode']!r}",
                  "측정 때의 셀 env 로 되돌리거나 다시 측정한다.")
    return out


# ── 캠페인 승인 · publish 위상 ───────────────────────────────────────────────────────────────────────────────
def approval_for(repo, campaign_id, cell, node) -> dict | None:
    """O6/X3 사전승인. 판정 원천은 terraforming `campaign_template_validator.hint_approval_for`(모양 판정 한 벌 — 그 파일
    docstring: "hint 발행기 continue 는 승인을 직접 파싱하지 않고 이 함수를 부른다"). 측정 노드 축(cluster)은
    `resolve_measurement_node` 로 선언 노드에 해소한다. 결손 기록(반쪽 승인·폐기 키 arch)은 **죽는다** — 모양이 무너진
    기록에서 어느 줄이 진짜 승인인지 가를 수 없다. 승인 없음 = None."""
    repo = _repo(repo)
    _check_campaign_id(campaign_id)
    decl = _read_json_opt(repo / "campaigns" / campaign_id / "campaign.yaml", "campaign.yaml")
    if not isinstance(decl, dict):
        core.fail("HINT_CAMPAIGN_ABSENT", f"campaigns/{campaign_id}/campaign.yaml 부재")
    val = _validator(repo)
    reasons = val.hint_target_reasons(decl)
    if reasons:
        core.fail("HINT_APPROVAL_INVALID", f"hint_targets 승인 기록이 무너졌다: {reasons}",
                  "`campaign_init.py --hint-approve --campaign-id <id> --node N --cells … --approved-by <발화 전사> --utc U` "
                  "로 다시 적는다(손편집 ✗).")
    declared = [n.get("node_id") for n in decl.get("nodes") or [] if isinstance(n, dict) and n.get("node_id")]
    cands, kind = val.resolve_measurement_node(_check_node(node), declared)
    if kind == "unknown":
        cands = [node]
    for cand in cands:
        hit = val.hint_approval_for(decl, cell, node=cand)
        if hit:
            a = hit["approval"]
            return {"approved_by": a["approved_by"], "approved_utc": a["approved_utc"], "source": "campaign:hint_targets",
                    "hitl_source": a["source"], "node_id": hit["node_id"], "index": hit["index"]}
    return None


def phase_set_publish(repo, campaign_id, cell, node, tag, remote_sha, utc, *, runner: Callable | None = None) -> None:
    """publish 위상 진행표 한 줄 — 소유자 `campaign_init.py --phase-set publish` CLI(유일한 writer · 포맷 소유 1).
    `--campaign-id` 를 **항상** 명시한다(생략하면 campaign_init 이 ACTIVE 로 해소한다 — 옛 backfill 결함 H-7).
    cluster 셀의 진행표 노드 = 그 셀의 배정 노드(phases/ 는 main|sub 디렉터리다)."""
    repo = _repo(repo)
    core.require_utc(utc, "publish 위상 --utc")
    _check_campaign_id(campaign_id)
    _check_cell(cell)
    if not isinstance(tag, str) or not tag.startswith(core.HINT_TAG_PREFIX):
        core.fail("HINT_TAG_NAMESPACE", f"hint/ 이름공간 밖 태그: {tag!r}")
    if not isinstance(remote_sha, str) or not _SHA40.match(remote_sha):
        core.fail("HINT_REMOTE_SHA_INVALID", f"원격 태그 오브젝트 SHA 가 40자 소문자 16진이 아니다: {remote_sha!r}")
    phase_node = _check_node(node)
    if phase_node == "cluster":
        decl = _read_json_opt(repo / "campaigns" / campaign_id / "campaign.yaml", "campaign.yaml") or {}
        val = _validator(repo)
        declared = [n.get("node_id") for n in decl.get("nodes") or [] if isinstance(n, dict) and n.get("node_id")]
        phase_node = next((n for n in declared if cell in val.assigned_cells(decl, n)), None)
        if phase_node is None:
            core.fail("HINT_CELL_ASSIGNMENT_ABSENT", f"cluster 셀 {cell!r} 의 배정 노드를 찾지 못했다(campaigns/{campaign_id}).")
    argv = ["--campaign-id", campaign_id, "--phase-set", "publish", "--node", phase_node, "--cell", cell,
            "--state", "done", "--proof-predicate", "원격 태그 오브젝트 SHA = 로컬 봉인 SHA(git ls-remote 대조)",
            "--proof-ok", "--proof-source", f"refs/tags/{tag}@{remote_sha}", "--ended-utc", utc,
            "--authored-by", "main"]
    # `--started-utc` 는 넘기지 않는다 — 주입 시각은 원격 SHA 대조가 끝난 시각(= done 기록 시각)뿐이다. 그 값을 시작 시각으로도
    #   적으면 진행표가 "시작하자마자 끝났다" 는 거짓을 말한다(evidence_publisher 재분류 시각 칸과 같은 규율 — 주입할 칸이
    #   없으면 적지 않는다 · 2026-09-22 적대 검토).
    run = runner or (lambda *a: core.run_python(repo, core.REL_CAMPAIGN_INIT, *a))
    proc = run(*argv)
    if proc.returncode != 0:
        core.fail("HINT_CAMPAIGN_WRITE_FAILED", f"publish 위상 기록 실패(rc={proc.returncode}): "
                  f"{(proc.stderr or '').strip()[-800:]}")


# ── 발행기 구동(campaign 모드 · 손 JSON 0) ────────────────────────────────────────────────────────────────────
def topic_for(ev: CellEvidence) -> str:
    """결정론 토픽 `<campaign>__<node>__<cell>`(doc_naming 토픽 문자 [A-Za-z0-9_.] 로 정규화). 재생 모드면 원 토픽.
    ★ 셀 키가 **끝**에 온다 — lineage `seeds_from_publications` 는 과거 발행 기록을 "정규화 발행 id 가 `_<셀 키>` 로 끝남" 으로
      찾는다(X17 · 옛 기록 `qwen38fn_full_nv4_bf_262k_mmp` 의 모양). 노드를 끝에 두면 같은 셀의 재발행이 이전 발행을 계보에서
      잃는다(2026-09-22 적대 검토)."""
    if ev.mode == "publication-replay":
        return str((ev.publication or {}).get("topic"))
    raw = f"{ev.campaign_id}__{ev.node}__{ev.cell}"
    return re.sub(r"[^A-Za-z0-9_.]", "_", raw)


def _publisher(repo: Path, sub: str, *args: str, accept=(0,)) -> tuple[int, dict]:
    proc = core.run_python(repo, core.REL_EVIDENCE_PUBLISHER, sub, "--repo-root", str(repo), *args)
    try:
        doc = json.loads(proc.stdout) if proc.stdout.strip() else {}
    except ValueError:
        core.fail("HINT_PUBLISHER_OUTPUT_UNREADABLE", f"evidence_publisher {sub} 출력이 JSON 이 아니다(rc={proc.returncode}): "
                  f"{proc.stdout[-600:]}{proc.stderr[-600:]}")
    if proc.returncode not in accept:
        core.fail("HINT_PUBLISHER_FAILED", f"evidence_publisher {sub} rc={proc.returncode}: "
                  f"{(doc or {}).get('reason_codes')} {json.dumps((doc or {}).get('messages'), ensure_ascii=False)[:800]}",
                  "사유코드를 해소한다 — 발행기의 판정을 우회하지 않는다.", exit_code=proc.returncode or 1)
    return proc.returncode, doc


def _narratives(ev: CellEvidence) -> dict:
    """plan = 캠페인 선언 plan_ref · devlog/testlog = 이 셀의 캠페인 포인터(하나씩). 없거나 모호하면 **쓰기 전에** 죽는다."""
    out: dict = {}
    plan = (ev.declaration or {}).get("plan_ref")
    for kind in ("plan", "devlog", "testlog"):
        cands = sorted({p["path"] for p in ev.pointers if p.get("kind") == kind and isinstance(p.get("path"), str)})
        if kind == "plan" and isinstance(plan, str) and plan and plan not in cands:
            cands = [plan] + cands
        cands = [c for c in cands if (Path(ev.repo) / c).is_file()]
        if not cands:
            core.fail("HINT_NARRATIVE_EVIDENCE_ABSENT", f"셀 {ev.cell!r} 의 {kind} 증거가 없다.",
                      f"셀 testlog·devlog 를 먼저 발행하고 `campaign_init.py --evidence-add --kind {kind} --path <docs/…> "
                      f"--cell {ev.cell} --node <n> --campaign-id {ev.campaign_id}` 로 등록한다(plan 은 campaign.yaml plan_ref).")
        if len(cands) > 1:
            core.fail("HINT_NARRATIVE_EVIDENCE_AMBIGUOUS", f"셀 {ev.cell!r} 의 {kind} 후보가 {len(cands)}건이다: {cands}",
                      "이 셀의 판정·서사 문서 하나만 가리키도록 포인터를 정리한다 — 발행기가 고르지 않는다.")
        out[kind] = cands[0]
    return out


def _runtime_identity(repo: Path, ev: CellEvidence) -> tuple[dict, str]:
    """runtime.identity 의 **관측**(모듈 docstring). 스윕 meta(측정 시 docker inspect·엔진 로그) → lite 리포트 측정 환경 표.
    identity 에서 복사하지 않는다 — 둘이 다르면 게이트(RUNTIME_IDENTITY_MISMATCH)가 판정한다."""
    meta = ((ev.sweep or {}).get("index") or {}).get("meta") or {}
    if meta:
        obs, src = _identity_from_meta(meta), f"sweep_index.meta({(ev.sweep or {}).get('dir')} · {(ev.sweep or {}).get('binding')})"
    else:
        snap = _lite_snapshot(repo, ev.bench_report_path)
        if not snap:
            core.fail("HINT_RUNTIME_IDENTITY_UNOBSERVED", "runtime identity 를 관측할 곳이 없다(스윕 meta·lite 측정 환경 표 모두 부재).",
                      "identity 를 복사하지 않는다 — 측정 산출물이 있어야 한다.")
        obs = _identity_from_snapshot(repo, ev, snap)
        tp_src = obs.pop("_tp_source")
        src = f"bench_report 측정 환경 표({Path(ev.bench_report_path).name})" + (f" · tp={tp_src}" if tp_src else "")
    need = [k for k in ("model", "gpu", "vllm", "topology", "tp") if obs.get(k) in (None, "")]
    if need:
        core.fail("HINT_RUNTIME_IDENTITY_UNOBSERVED", f"runtime identity 키 {need} 가 관측되지 않았다({src}).",
                  "identity 를 복사하지 않는다 — 측정 meta 에 그 키가 있어야 한다.")
    return obs, src


def _pii_scan(repo: Path, rels: dict, terms: list[str]) -> list[str]:
    from . import pii
    bad: list[str] = []
    for kind, rel in sorted(rels.items()):
        p = repo / rel
        if p.is_file():
            hits = pii.scan_text(p.read_text(encoding="utf-8", errors="replace"), terms, profile="nondeploy_prose")
            bad += [f"{rel}:{h.line} {h.pattern}" for h in hits]
        elif p.is_dir():
            bad += [f"{rel}/{r}:{h.line} {h.pattern}" for r, h in pii.scan_tree(p, terms, profile="nondeploy_prose")]
    return bad


def _append_simlog(repo: Path, topic: str, record: dict, src: str, dest: str, utc: str) -> None:
    """append-raw simlog 의 멱등 재개: 같은 이름·같은 바이트가 이미 있으면 건너뛴다 · 다른 바이트면 죽는다(append-only)."""
    sim = (record.get("scaffolded") or {}).get("simlog")
    if isinstance(sim, str) and (repo / sim / dest).is_file():
        if (repo / sim / dest).read_bytes() == (repo / src).read_bytes():
            return
        core.fail("HINT_SIMLOG_DEST_CONFLICT", f"{sim}/{dest} 가 이미 다른 내용으로 있다(append-only).")
    _publisher(repo, "append-raw", "--topic", topic, "--kind", "simlog", "--src", src, "--recorded-utc", utc,
               "--dest-name", dest)


def _record(repo: Path, topic: str) -> dict:
    rec = _read_json_opt(repo / core.REL_EVIDENCE_DIR / f"{topic}.json", "발행 기록")
    if not isinstance(rec, dict):
        core.fail("HINT_PUBLICATION_ABSENT", f"발행 기록 부재: {core.REL_EVIDENCE_DIR}/{topic}.json")
    return rec


def drive_publisher(repo, ev: CellEvidence, *, topic, generated_utc, draft_dir) -> Path:
    """evidence_publisher 를 구동해 work-manifest 를 만든다(campaign 모드 · SPEC §4.1 ③). **손 JSON 0**: identity·runtime·pii
    입력은 이 함수가 관측해 `<draft>/inputs/` 에만 쓴다. 판정(자격·서사 증거·PII·runtime 관측)은 **첫 쓰기 전에** 끝낸다.
    반환 = work-manifest 경로(게이트가 promotion-ready 인지는 호출부가 `authorize` 로 묻는다)."""
    repo = _repo(repo)
    if ev.mode != "campaign":
        core.fail("HINT_REPLAY_READ_ONLY", "발행 기록 재생 모드는 읽기만 한다 — 기존 발행 기록·manifest 를 쓴다.",
                  "`ev.publication['manifest_path']` 를 그대로 쓴다.")
    core.require_utc(generated_utc)
    draft = Path(draft_dir)
    inputs = draft / "inputs"
    # ── 판정(부수효과 0) ──
    qual = qualification(ev)
    narr = _narratives(ev)
    ident = identity(ev)
    # 발행기 identity.json 은 **측정** 6키다 — identity() 의 quant 는 측정이 N/A 일 때 이름 축으로 채운 표시값일 수 있고, 그것을 넘기면
    #   runtime.identity(스윕 meta 관측 · quantization=NA)와 갈라져 게이트가 RUNTIME_IDENTITY_MISMATCH 로 막는다(2026-09-22 · S2 round 2).
    measured6 = _measured_identity(repo, ev)[0]
    six = {k: (measured6.get(k) if k == "quant" else ident[k]) for k in ("model", "gpu", "vllm", "quant", "topology", "tp")}
    rt_ident, rt_src = _runtime_identity(repo, ev)
    mc = measurement_config(repo, ev)
    if not ev.bench_report_path:
        core.fail("HINT_BENCH_REPORT_ABSENT", f"셀 {ev.cell!r} 에 바인딩할 bench_report 가 없다.",
                  "벤치 스킬이 docs/benchmark/ 에 발행한 리포트를 캠페인 포인터로 등록한다(lite 는 `lite_bench.sh --publish-report`).")
    downgraded = mc.get("bench_mode_kind") == "downgraded-lite"
    if ev.report_kind == "lite" and not downgraded:
        task, mode, verdict = "hint_map_only", "lite", None
    else:
        v = ((ev.sweep or {}).get("verdict") or {}).get("verdict") if (ev.sweep or {}).get("binding") == "output" else None
        verdict = v or (ev.certificate or {}).get("verdict")
        if verdict not in ("PASS", "FAIL", "REFUTE"):
            core.fail("HINT_VERDICT_UNOBSERVED", f"셀 {ev.cell!r} 의 벤치 판정을 관측하지 못했다(verdict.json·인증서): {verdict!r}")
        task, mode = "full_benchmark", "full"
    sweep_dir = (ev.sweep or {}).get("dir")
    sim_srcs: list[tuple[str, str]] = []
    if task == "full_benchmark" and not downgraded:
        if not sweep_dir:
            core.fail("HINT_SIMLOG_SOURCE_ABSENT", "full 발행은 simlog(스윕 원시)가 필요한데 조인된 스윕이 없다.")
        vp = _as_int(((ev.sweep or {}).get("index") or {}).get("verdict_point_level")) or 1
        idx_src = f"{sweep_dir}/sweep_index.json"
        meas = f"{sweep_dir}/level_{vp:02d}/measured.json"
        if not (repo / meas).is_file():
            meas = f"{sweep_dir}/level_{vp:02d}_measured.json"
        for s, d in ((idx_src, "sweep_index.json"), (meas, f"level_{vp:02d}_measured.json")):
            if not (repo / s).is_file():
                core.fail("HINT_SIMLOG_SOURCE_ABSENT", f"simlog 원천 부재: {s}")
            sim_srcs.append((s, d))
    vj_ok = bool(sweep_dir and (ev.sweep or {}).get("binding") == "output" and (repo / sweep_dir / "verdict.json").is_file())
    if task == "full_benchmark" and not downgraded and not vj_ok:
        # 2026-09-29(plan_26092908 §4.8 · V11④): verdict.json 이 루브릭 권한의 **유일한** 운반자다(발행기 rubric_source=verdict_json).
        #   빠지면 manifest 권한이 None 이 되고 게이트(certificate_requirement_authority)는 "권한 모름" 으로 인증서를 다시 요구한다 —
        #   explore/weak PASS 가 조용히 인증서 결손으로 막히고, 사람은 인증서를 손으로 만든다(DS4F 수동 발행의 재발). 첫 쓰기 **전에**
        #   명시 실패로 둔다(발행 기록을 묶은 뒤 죽으면 거부된 publish 가 시각을 묶는다 — V11①).
        why = ("조인된 스윕이 없다" if not sweep_dir else
               f"스윕이 {(ev.sweep or {}).get('binding')} 로만 묶였다(출력 평면 원시가 이 측정의 것이 아니다)"
               if (ev.sweep or {}).get("binding") != "output" else f"{sweep_dir}/verdict.json 부재")
        core.fail("HINT_VERDICT_JSON_ABSENT",
                  f"full_benchmark 발행에 판정 산출물(verdict.json)이 필요한데 넘길 수 없다 — {why}.",
                  "verdict_rule 산출물이 있는 측정(스윕 generated_utc = 측정 키)으로 발행한다. 없으면 루브릭 권한이 게이트에 닿지 "
                  "않아 인증서 요구가 되살아난다(인증서를 손으로 만들지 않는다).")
    from . import pii
    terms = pii.require_terms(repo)          # 리터럴 없이 스캔하고 통과를 선언하면 그 선언이 거짓이다(docs.md §PII)
    pre = dict(narr)
    pre["bench_report"] = ev.bench_report_path
    if ev.certificate_path and verdict == "PASS":
        pre["certificate"] = ev.certificate_path
    hits = _pii_scan(repo, pre, terms)
    if hits:
        core.fail("HINT_PII_SCAN_FAILED", f"비배포 증거에 PII 매치(nondeploy_prose): {hits[:10]}",
                  "원문 문서를 고친다(서사 평면은 사람이 저작한다) — 발행기가 가리지 않는다.")
    # ── 쓰기 ──
    inputs.mkdir(parents=True, exist_ok=True)
    core.write_json(inputs / "identity.json", six)
    init = ["--topic", topic, "--task-class", task, "--generated-utc", generated_utc,
            "--identity-json", str(inputs / "identity.json"), "--benchmark-mode", mode]
    if verdict:
        init += ["--benchmark-verdict", verdict]
    rec = _read_json_opt(repo / core.REL_EVIDENCE_DIR / f"{topic}.json", "발행 기록")
    if not (downgraded and isinstance(rec, dict) and rec.get("task_class") == "hint_map_only"):
        _publisher(repo, "init", *init)
    for kind in ("plan", "devlog", "testlog"):
        _publisher(repo, "set-narrative", "--topic", topic, "--kind", kind, "--narrative-file", narr[kind],
                   "--author", "hint.py(evidence.drive_publisher)", "--generated-utc", generated_utc)
    if task == "full_benchmark":
        for s, d in sim_srcs:
            _append_simlog(repo, topic, _record(repo, topic), s, d, generated_utc)
        argv = ["--topic", topic, "--verdict", verdict, "--generated-utc", generated_utc,
                "--bench-report-src", ev.bench_report_path]
        if ev.certificate_path and verdict == "PASS":
            argv += ["--certificate-src", ev.certificate_path]
        if vj_ok:
            argv += ["--verdict-json-src", f"{sweep_dir}/verdict.json"]
        if not (downgraded and _record(repo, topic).get("task_class") == "hint_map_only"):
            _publisher(repo, "publish-benchmark", *argv)
        if downgraded:
            reason = mc.get("downgrade_reason")
            if not reason:
                core.fail("HINT_DOWNGRADE_REASON_ABSENT", "강등 셀인데 측정 구성 표에 downgrade_reason 이 없다.")
            _publisher(repo, "init", "--topic", topic, "--task-class", "hint_map_only", "--generated-utc", generated_utc,
                       "--identity-json", str(inputs / "identity.json"), "--benchmark-mode", "lite",
                       "--benchmark-verdict", verdict, "--downgrade-from", "full_benchmark", "--downgrade-reason", str(reason))
    else:
        _publisher(repo, "publish-lite-report", "--topic", topic, "--generated-utc", generated_utc,
                   "--bench-report-src", ev.bench_report_path)
    record = _record(repo, topic)
    _write_pii_and_runtime(repo, record, inputs, terms, qual, rt_ident, rt_src)
    return _finalize(repo, topic, inputs)


def _write_pii_and_runtime(repo: Path, record: dict, inputs: Path, terms: list[str], qual: dict, rt_ident: dict,
                           rt_src: str) -> None:
    """pii.json · runtime.json(+출처 사이드카) — 바인딩된 **실제 경로**를 finalize 가 쓸 문자열 그대로 적는다."""
    mdir = repo / core.REL_EVIDENCE_DIR
    scaff = record.get("scaffolded") or {}
    keys = list(record.get("required_evidence") or []) + list(record.get("or_group_scaffolded") or [])
    scan: dict = {}
    exempt: list[dict] = []
    reason = cgate(repo).PII_MACHINE_RAW_EXEMPTION_REASON
    for k in sorted(set(keys)):
        rel = scaff.get(k)
        if not isinstance(rel, str) or not rel:
            continue
        mrel = os.path.relpath(str(repo / rel), str(mdir))
        if k == "simlog":
            exempt.append({"path": mrel, "reason": reason})     # X5 — 판정 대상 밖임을 **기재**(스캔한 척 ✗)
        else:
            scan[mrel] = rel
    hits = _pii_scan(repo, {m: r for m, r in scan.items()}, terms)
    if hits:
        core.fail("HINT_PII_SCAN_FAILED", f"바인딩된 비배포 증거에 PII 매치(nondeploy_prose): {hits[:10]}")
    core.write_json(inputs / "pii.json", {"passed": True, "scanned_paths": sorted(scan), "exempt_paths": exempt})
    core.write_json(inputs / "runtime.json", {"health_ok": bool(qual["health_200"]),
                                             "functional_smoke_passed": bool(qual["inference_observed"]),
                                             "identity": rt_ident})
    # 스키마(runtime additionalProperties:false)가 받지 않는 출처는 옆 파일로 — 값 옆의 출처(헌법 §결정론 산출물).
    core.write_json(inputs / "runtime.provenance.json", {"qualification": qual, "identity_source": rt_src,
                                                        "containers": "미기재 — post_health 는 컨테이너 이름·restart_count 를 "
                                                                      "남기지 않는다(스키마 필수 칸 · 합성 ✗)"})


def _finalize(repo: Path, topic: str, inputs: Path) -> Path:
    for f in ("pii.json", "runtime.json"):
        if not (inputs / f).is_file():
            core.fail("HINT_PUBLISHER_INPUTS_ABSENT", f"도구가 쓴 발행기 입력 부재: {inputs / f}",
                      "`hint.py publish` 가 draft 의 inputs/ 를 만든다 — 손으로 쓰지 않는다.")
    _publisher(repo, "finalize", "--topic", topic, "--pii-scan-json", str(inputs / "pii.json"),
               "--runtime-json", str(inputs / "runtime.json"), accept=(0, 1))
    man = repo / core.REL_EVIDENCE_DIR / f"{topic}.work-manifest.json"
    if not man.is_file():
        core.fail("HINT_PUBLISHER_FAILED", f"finalize 뒤 work-manifest 부재: {_rel(repo, man)}")
    return man


def set_promotion_target(repo, *, topic, tag, topology, anchor, generated_utc, draft_dir=None) -> Path:
    """X6 — 승격 목표(태그 1건)를 **발행 기록**에 적고(`evidence_publisher set-promotion-target`) finalize 를 다시 돌려
    manifest 로 방출한다. hint 는 manifest 를 직접 고치지 않는다(writer 1). finalize 입력은 publish 때 도구가 쓴
    `<draft>/inputs/{pii,runtime}.json` 을 다시 쓴다(draft_dir 필수 — SPEC 시그니처에 더한 키워드)."""
    repo = _repo(repo)
    core.require_utc(generated_utc)
    if draft_dir is None:
        core.fail("HINT_PUBLISHER_INPUTS_ABSENT", "finalize 재실행에 draft 의 inputs/ 가 필요하다(draft_dir 미지정).",
                  "hint.py continue 가 state.json 의 draft 경로를 넘긴다.")
    _publisher(repo, "set-promotion-target", "--topic", topic, "--tag", tag, "--topology", topology,
               "--anchor", anchor, "--generated-utc", generated_utc)
    man = _finalize(repo, topic, Path(draft_dir) / "inputs")
    doc = _read_json_opt(man, "work-manifest") or {}
    want = {"kind": "hint", "tag": tag, "topology": topology, "anchor": anchor}
    if doc.get("promotion_target") != want:
        core.fail("HINT_PROMOTION_TARGET_NOT_EMITTED", f"finalize 가 promotion_target 을 방출하지 않았다: {doc.get('promotion_target')!r}")
    return man


def output_manifest(repo, topology: str) -> dict | None:
    """`output/<topology>/manifest.yaml`(소유 로더) — 부재 = None. 발췌 치환표(`pii.substitution_table`)의 입력이다
    (hint.py lint·continue·excerpt 가 publish 때와 **같은** 치환표를 다시 만든다 · 2026-09-22 통합)."""
    if topology not in TOPOLOGIES:
        core.fail("HINT_TOPOLOGY_UNDERIVABLE", f"토폴로지가 single|multi 가 아니다: {topology!r}")
    return _manifest(_repo(repo), topology)[0]


def publisher_inputs(repo, ev: CellEvidence, *, draft_dir) -> Path:
    """발행 기록 재생(replay) 드래프트를 **캠페인 밖 발행**으로 잇는 continue 가 쓰는 발행기 입력 — `<draft>/inputs/`
    `{pii,runtime,runtime.provenance}.json`(도구 작성 · 손 JSON 0 · X6). campaign 모드는 drive_publisher 가 같은 파일을 이미
    썼다(이 함수를 부르지 않는다). 관측은 drive_publisher 와 **같은 함수**(qualification · _runtime_identity · 발행 기록의
    required_evidence PII 스캔)다 — 재생 기록의 바인딩 증거를 다시 스캔하고 거짓 통과를 적지 않는다. 쓰기 = draft 안뿐."""
    repo = _repo(repo)
    if ev.mode != "publication-replay":
        core.fail("HINT_PUBLISHER_INPUTS_MODE", "publisher_inputs 는 재생 드래프트 전용이다 — campaign 모드는 drive_publisher 가 쓴다.")
    record = (ev.publication or {}).get("record")
    if not isinstance(record, dict):
        core.fail("HINT_PUBLICATION_ABSENT", "재생 발행 기록을 읽지 못했다 — publisher 입력을 관측할 기록이 없다.")
    from . import pii
    terms = pii.require_terms(repo)
    qual = qualification(ev)
    rt_ident, rt_src = _runtime_identity(repo, ev)
    inputs = Path(draft_dir) / "inputs"
    inputs.mkdir(parents=True, exist_ok=True)
    _write_pii_and_runtime(repo, record, inputs, terms, qual, rt_ident, rt_src)
    return inputs


# ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────────────────────
_OWNER_LINKS = (core.REL_COMPLETION_GATE, core.REL_EVIDENCE_PUBLISHER, core.REL_CAMPAIGN_VALIDATOR,
                core.REL_RENDER_BENCH_SECTION, REL_MANIFEST_CONTRACT, REL_CHECK_SMOKE_MODEL, core.REL_SLAVE_FORWARD,
                core.REL_RENDER_DOCKERFILE)
_M_UTC = "2026-01-02T03:04:05Z"
_SHA_A = "a" * 40
_SHA_H = "4" * 40            # 픽스처 체크포인트 checkout 커밋
_SHA0 = "0" * 40             # reflog 첫 항목의 old(무)
_DIGEST = "sha256:" + "d" * 64


def _code(fn, *a, **kw) -> str | None:
    try:
        fn(*a, **kw)
    except core.HintError as e:
        return e.code
    return None


def _fake_docker(state: dict):
    """읽기 전용 docker 가짜 — inspect/history 만 답한다. 호출 argv 를 state['calls'] 에 남긴다."""
    def run(argv, **kw):
        state.setdefault("calls", []).append(list(argv))
        if argv[1:3] == ["image", "inspect"]:
            ref = argv[3]
            img = state["images"].get(ref)
            if img is None:
                return SimpleNamespace(returncode=1, stdout="", stderr="no such image")
            return SimpleNamespace(returncode=0, stdout=json.dumps([img]), stderr="")
        if argv[1] == "history":
            ref = argv[-1]
            h = state["history"].get(ref)
            return SimpleNamespace(returncode=0 if h is not None else 1, stdout=h or "", stderr="")
        return SimpleNamespace(returncode=1, stdout="", stderr="unexpected")
    return run


def _fixture(td: Path) -> tuple[Path, dict]:
    """격리 저장소 — 소유 모듈은 이 체크아웃의 파일에 심링크(코드만) · 데이터는 전부 픽스처. campaign_init 은 링크하지
    않는다(finalize 의 서가 입고가 **실제 저장소**의 __llm-wiki 를 건드리지 않게 — 그 스크립트는 자기 __file__ 로 루트를 정한다)."""
    code_root = core.HINTLIB_DIR.parents[4]
    repo = td / "repo"
    repo.mkdir()
    for rel in _OWNER_LINKS:
        src = code_root / rel
        if not src.is_file():
            core.fail("HINT_OWNER_MODULE_MISSING", f"자체검사 소유 모듈 부재: {rel}")
        dst = repo / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        os.symlink(src, dst)
    # campaign_init 은 **사본**으로 둔다(심링크 ✗) — 그 스크립트는 `Path(__file__).resolve()` 로 루트를 정하므로 사본이면 루트가
    #   이 격리 저장소가 된다. 그래서 publish 위상 writer 를 **실물 CLI** 로 부를 수 있다(가짜 runner 로만 시험하면 argv 가 소유
    #   CLI 와 갈라져도 초록이다 — 2026-09-22 적대 검토 · 픽스처가 실물보다 좁은 결함의 재발 방지). 서가 입고 도구는 두지
    #   않는다 → 입고는 "건너뜀" 으로 끝나고 실제 저장소의 __llm-wiki 에 닿지 않는다.
    ci = repo / core.REL_CAMPAIGN_INIT
    ci.parent.mkdir(parents=True, exist_ok=True)
    ci.write_bytes((code_root / core.REL_CAMPAIGN_INIT).read_bytes())
    vocab = {"schema_version": 1,
             "hw": {"gb10": ["NVIDIA GB10", "GB10"], "h100": ["NVIDIA H100"]},
             "quant": {"nvfp4": ["nvfp4", "modelopt-dominant:NVFP4"], "fp8": ["fp8"], "bf16": ["bf16", "none"]},
             "kv": {"auto": ["auto"], "fp8": ["fp8"]},
             "ple": {"mmap": ["mmap"], "resident": ["resident"], "offload": ["offload"], "none": ["none"]},
             "graph": {"graph": ["graph"], "eager": ["eager"]},
             "plane": {"docker": "", "native": "bare"},
             "quant_suffixes": ["nvfp4", "fp8", "bf16"]}
    core.write_json(repo / core.REL_VOCAB, vocab)
    (repo / core.REL_PII_TERMS).write_text("# fixture\nfixturesecretterm\n", encoding="utf-8")
    out = repo / "output/multi"
    for d in ("configs", "envs", "benchlog/sweep_c1-a/level_01"):
        (out / d).mkdir(parents=True, exist_ok=True)
    models = td / "models"
    ckpt = models / "Org/Fixture-Model-NVFP4"
    (ckpt / ".git").mkdir(parents=True)
    core.write_json(ckpt / "config.json", {"quantization_config": {
        "quant_method": "modelopt", "quant_algo": "MIXED_PRECISION",
        "quantized_layers": {"l0": {"quant_algo": "NVFP4"}, "l1": {"quant_algo": "NVFP4"}, "l2": {"quant_algo": "FP8"}}},
        "text_config": {"ple_layer_ids": [1], "ple_embed_dim": 8}})
    (ckpt / "README.md").write_text("---\nbase_model:\n- Org/Fixture-Model\nlicense: other\n---\n# card\n", encoding="utf-8")
    (ckpt / ".git/config").write_text('[remote "origin"]\n\turl = https://huggingface.co/Org/Fixture-Model-NVFP4\n',
                                      encoding="utf-8")
    # checkout 커밋(F4 · S2 round 2): HEAD → refs/heads/main(loose) · packed-refs 의 origin/main 과 같다
    (ckpt / ".git/HEAD").write_text("ref: refs/heads/main\n", encoding="utf-8")
    (ckpt / ".git/refs/heads").mkdir(parents=True)
    (ckpt / ".git/refs/heads/main").write_text(_SHA_H + "\n", encoding="utf-8")
    (ckpt / ".git/packed-refs").write_text(f"# pack-refs with: peeled\n{_SHA_H} refs/remotes/origin/main\n", encoding="utf-8")
    # 블랙박스 원장(F6): 이 셀 라벨 창 2개(1차 renew 없음 · 2차 renew · 측정 포함) + ★접두만 같은 다른 셀 창 · ★nomatch · ★파손 줄 ·
    #   ★운영자 경로 칸(decl_path — 실행 시 조립: 추적 파일에 그 모양의 리터럴을 두지 않는다 · 2026-08-06 자기스캔 사고)
    ev_dir = repo / "docs/logs/main/events"
    ev_dir.mkdir(parents=True)
    op = "/" + "home" + "/op-fixture/serve_budget.env"
    ledger = [{"kind": "budget_clear", "existed": False, "ts": "2026-01-02T01:00:00Z"},
              {"kind": "budget_declare", "label": "smoke-c1-a", "floor_mib": 100, "weights_mib": 50, "ts": "2026-01-02T01:00:00Z",
               "decl_path": op},
              {"kind": "budget_clear", "existed": True, "ts": "2026-01-02T01:30:00Z"},
              {"kind": "budget_declare", "label": "smoke-c1-a-kv8g", "floor_mib": 90, "ts": "2026-01-02T01:40:00Z"},
              {"kind": "watchdog_trip", "rule": "absolute", "mem_avail_mib": 9000, "ts": "2026-01-02T01:45:00Z"},
              {"kind": "budget_clear", "existed": True, "ts": "2026-01-02T01:50:00Z"},
              {"kind": "serve_start", "session_id": "serve-c1-a-20260102T020000Z", "model": "fixture", "ts": "2026-01-02T02:00:00Z"},
              {"kind": "serve_start", "session_id": "serve-c1-ab-20260102T020000Z", "model": "other", "ts": "2026-01-02T02:00:00Z"},
              {"kind": "budget_declare", "label": "smoke-c1-a", "floor_mib": 100, "ts": "2026-01-02T02:30:00Z"},
              {"kind": "budget_renew", "label": "smoke-c1-a", "ttl_s": 900, "ts": "2026-01-02T02:59:00Z"},
              {"kind": "watchdog_trip_nomatch", "ts": "2026-01-02T03:00:00Z"},
              "{not json",
              {"kind": "budget_clear", "existed": True, "ts": "2026-01-02T03:10:00Z"},
              # 2026-09-22(S2 round 2 리뷰): ★월 원장의 journald 사본(thermal.jsonl 02:45 와 같은 사건 · 필드 구성만 다름) ·
              #   ★다른 셀 라벨의 clear(창 안 시각 — 이 셀 창을 닫으면 안 된다) · ★IPv4 모양 값(detail 로 새면 안 된다). 끝에 덧붙여
              #   위 줄 번호(:2 · :9)를 밀지 않는다(행은 ts 로 정렬된다).
              {"kind": "thermal_trip", "soc_temp_c": 98, "source": "unit:fixture-thermal", "ts": "2026-01-02T02:45:00Z"},
              {"kind": "budget_clear", "label": "smoke-zz-other", "existed": True, "ts": "2026-01-02T02:50:00Z"},
              {"kind": "watchdog_highrate_hold", "target": ".".join(("10", "1", "2", "3")) + ":8000", "mem_avail_mib": 7,
               "ts": "2026-01-02T02:52:00Z"}]
    (ev_dir / "2026-01.jsonl").write_text("".join((x if isinstance(x, str) else json.dumps(x)) + "\n" for x in ledger),
                                          encoding="utf-8")
    (ev_dir / "watchdog.jsonl").write_text("".join(json.dumps(x) + "\n" for x in (
        {"kind": "budget_honored", "label": "smoke-c1-a", "arm_ceiling_mib": 70, "ts": "2026-01-02T02:30:01Z"},
        {"kind": "watchdog_highrate_hold", "mem_avail_mib": 60000, "rate_mib_s": 25000, "ts": "2026-01-02T02:31:00Z"},
        {"kind": "budget_none", "arm_ceiling_mib": 999999999, "ts": "2026-01-02T03:10:01Z"},
        # ★같은 파일 · 같은 초 · 같은 kind 의 두 번째 행(사본이 아니다 — 합치면 안 된다 · 2026-09-22 round 2 리뷰)
        {"kind": "watchdog_highrate_hold", "mem_avail_mib": 59000, "rate_mib_s": 26000, "ts": "2026-01-02T02:31:00Z"})),
        encoding="utf-8")
    (ev_dir / "thermal.jsonl").write_text("".join(json.dumps(x) + "\n" for x in (
        {"kind": "thermal_trip", "soc_temp_c": 99, "ts": "2026-01-02T01:45:00Z"},
        {"kind": "thermal_trip", "soc_temp_c": 98, "ts": "2026-01-02T02:45:00Z"})), encoding="utf-8")
    (repo / "docs/logs/sub/events").mkdir(parents=True)
    (repo / "docs/logs/sub/events/2026-01.jsonl").write_text(
        json.dumps({"kind": "budget_declare", "label": "smoke-c1-b", "ts": "2026-01-02T02:30:00Z"}) + "\n", encoding="utf-8")
    (out / "manifest.yaml").write_text("topology: multi\nself_role: main\ngpus_per_node: 1\ngpu_model: NVIDIA GB10\n"
                                       "cpu_arch: aarch64\nnodes:\n  - role: main\n  - role: sub\n    gpu_model: NVIDIA GB10\n",
                                       encoding="utf-8")
    (out / "docker-compose.yaml").write_text(f"services:\n  vllm-master-serve:\n    volumes:\n      - {models}:/app/quant_models:ro\n",
                                             encoding="utf-8")
    (out / "configs/c1-a.yaml").write_text(
        "# 셀 c1-a\nmodel: /app/quant_models/Org/Fixture-Model-NVFP4\ntensor-parallel-size: 2\nmax-model-len: 4096\n"
        "kv-cache-dtype: auto\nkv-cache-memory-bytes: 1073741824\nenforce-eager: true   # 승계\n"
        "speculative-config: '{\"method\":\"mtp\",\"num_speculative_tokens\":2}'\n", encoding="utf-8")
    (out / "Dockerfile.source-build").write_text("FROM base\nARG VLLM_REPO=x\nARG VLLM_REF=y\nARG SM12X_PORT=0\n", encoding="utf-8")
    (out / "configs/c1-a.sh").write_text('#!/bin/bash\nvllm serve --config "/app/configs/${CONFIG_FILE}.yaml"\n', encoding="utf-8")
    (out / "envs/.env.c1-a").write_text("CONFIG_FILE=c1-a\nIMAGE_TAG=fixture:0.9.0-source\nBUILD_DOCKERFILE=Dockerfile.source-build\n"
                                        "VLLM_REF=v0.9.0\nVLLM_PLE_MMAP=1\n", encoding="utf-8")
    core.write_json(out / "resolved.json", {"vllm_version": "0.9.0", "torch": {"pin": "2.9.0"},
                                            "ngc_base": {"tag": "26.01-py3", "image": "nvcr.io/nvidia/pytorch:26.01-py3"},
                                            "build_track": {"decision": "source-build"},
                                            "upstream_delta": {"to_ref": "v0.9.0", "to_sha": _SHA_A}})
    sweep = out / "benchlog/sweep_c1-a"
    meta = {"model": "fixture-model-nvfp4", "measured_node": "cluster", "config_name": "c1-a", "gpu_model": "NVIDIA GB10",
            "vllm_version": "0.9.0", "quantization": "NA", "topology": "multi", "tensor_parallel_size": "2",
            "image_tag": "fixture:0.9.0-source", "image_tag_source": "measured(docker inspect)",
            "image_tag_declared": "fixture:0.9.0-source", "image_digest": _DIGEST,
            "image_digest_source": "measured(docker inspect .Image)", "max_model_len": "4096", "kv_cache_dtype": "auto",
            "enforce_eager": "true", "ple_mode": "mmap", "ple_mode_source": "declared(envfile VLLM_PLE_MMAP=1)",
            "gpu_memory_utilization": "NA", "kv_cache_memory_bytes": "1073741824",
            "model_path": "/app/quant_models/Org/Fixture-Model-NVFP4", "max_num_seqs": "NA"}
    core.write_json(sweep / "sweep_index.json", {"config": "c1-a", "topology": "multi", "generated_utc": _M_UTC,
                                                 "verdict_point_level": 1, "meta": meta,
                                                 "lite": {"gen_tps": 10.0, "raw_json": str(sweep / "x.json")},
                                                 "levels": [{"level": 1, "status": "ok", "measured": {
                                                     "decode_tps": 12.3, "completed": 4, "failed": 0,
                                                     "measurement_ok": True}, "bench_json": str(sweep / "b.json")}]})
    core.write_json(sweep / "level_01/measured.json", {"decode_tps": 12.3, "measurement_ok": True, "completed": 4})
    core.write_json(sweep / "level_01/bench_c1-a.json", {"completed": 4, "failed": 0})
    core.write_json(sweep / "level_01/post_health_c1-a.json", {"provenance": "measured", "config": "c1-a",
                                                                "health_http_code": "200", "container_running": "true",
                                                                "container_oom_killed": "false"})
    core.write_json(sweep / "verdict.json", {"verdict": "PASS", "rubric": {"authority": "weak", "floor": 8.5,
                                                                           "ratio_M_over_primary": 1.23,
                                                                           "source": "expected_achievable(roofline×MBU)"}})
    core.write_json(sweep / "roofline.json", {"gpu_model": "NVIDIA GB10"})
    # 엔진 기동 표지(F6 · 컨테이너 시계 = 측정 창 안) — 3번째 `\n` 줄 안의 `\r`(tqdm)이 줄 번호를 밀면 안 된다
    (sweep / "lite_engine_c1-a.log").write_bytes("\n".join((
        "(EngineCore pid=1) \x1b[36mINFO 01-02 02:31:00 [a.py:1] engine up",
        "(Worker_TP0 pid=2) INFO 01-02 02:32:00 [weight_utils.py:1] Auto-prefetch is disabled because the filesystem (CIFS) is "
        "not a recognized network FS",
        "progress 10%\rprogress 100%",
        "(EngineCore pid=1) INFO 01-02 02:40:00 [kv_cache_utils.py:1] GPU KV cache size: 1,000 tokens, Maximum concurrency for "
        "4,096 tokens per request: 2.00x",
        "(APIServer pid=3) INFO 01-02 02:58:00 [entry.py:1] Starting vLLM server on http://0.0.0.0:8080", "")).encode("utf-8"))
    core.write_json(out / "benchlog/attestation_c1-a.json", {"config": "c1-a", "image_tag": "fixture:0.9.0-source",
                                                             "checks": [{"axis": "vllm_sha", "node": "sub", "observed": _SHA_A}]})
    rbs = core.load_owner_module(repo, core.REL_RENDER_BENCH_SECTION, "_hint_render_bench_section")
    for d in ("plan", "devlog", "testlog", "benchmark", "simlog"):
        (repo / "docs" / d).mkdir(parents=True, exist_ok=True)
    stem = "26010212_fixture-model-nvfp4_GB10_0.9.0"
    cert_rel, rep_rel = f"docs/benchmark/benchmark_{stem}.yaml", f"docs/benchmark/bench_report_{stem}.md"
    (repo / cert_rel).write_text(
        "schema_version: 1\nrecord_type: benchmark_certificate\nverdict: PASS\nmodel: fixture-model-nvfp4\n"
        'gpu_model: "NVIDIA GB10"\nvllm_version: 0.9.0\nquantization: N/A\ntopology: multi\ntensor_parallel_size: 2\n'
        'image_tag: "fixture:0.9.0-source"\n' f'image_digest: "{_DIGEST}"\n'
        "max_model_len: 4096\nkv_cache_dtype: auto\nenforce_eager: true\n"
        "benchmark_mode: full\ndecode_tps_conc1: 12.3\nrubric_authority: weak\nprimary_source: expected_achievable(roofline×MBU)\n"
        "primary_tps: 10.0\nfloor_tps: 8.5\ntolerance: 0.15\nratio_M_over_primary: 1.23\nspec_on: true\nlite_included: true\n"
        f'lite_gen_tps_warm: 10.0\nmeasured_utc: "{_M_UTC}"\nmeasured_node: cluster\n', encoding="utf-8")
    (repo / rep_rel).write_text("\n".join([
        "# 성능 보고서", "", f"> 생성일 {_M_UTC}.", "", rbs.MEASUREMENT_CONFIG_TITLE, "", "| 키 | 값 |", "|---|---|",
        "| bench_mode | full |", "| bench_mode_kind | full |", "| repeats | 3 |", "",
        "## lite 지표 (full ⊇ lite)", "", "| 메트릭 | Main | Sub |", "|---|---|---|",
        "| gen tokens/sec (warm) [master] | 10.00 t/s | — |", "",
        "## 부하 스윕 곡선", "", rbs.REPORT_HEADER, "|---|---|---|---|---|---|---|",
        "| 1 ★판정점 | 12.3 | 12 | 60 | 100 | 20 | 4/0 |", "| 2 | 20.1 | 20 | 90 | 120 | 25 | 4/0 |", "",
        "## 측정 환경 스냅샷", "", "| 키 | 값 |", "|---|---|", "| model | fixture-model-nvfp4 |", ""]), encoding="utf-8")
    plan, devlog, testlog = ("docs/plan/plan_26010200_fixture.md", "docs/devlog/devlog_26010213_fixture.md",
                             "docs/testlog/testlog_26010213_fixture.md")
    for p, body in ((plan, "# plan\n픽스처 계획.\n"), (devlog, "# devlog\n픽스처 서사.\n"), (testlog, "# testlog\n픽스처 판정.\n")):
        (repo / p).write_text(body, encoding="utf-8")
    camp = repo / "campaigns/c1"
    (camp / "cells/c1-a").mkdir(parents=True)
    decl = {"id": "c1", "plan_ref": plan,
            "nodes": [{"node_id": "main", "role": "main", "topology": "multi"}, {"node_id": "sub", "role": "sub", "topology": "multi"}],
            "assignments": {"main": [{"cell": "c1-a", "mode": "AUTO"}], "sub": [{"cell": "c1-b", "mode": "AUTO"}]},
            "control_variables": {"topology": "multi", "target_gpu": "NVIDIA GB10"},
            "hint_targets": [{"node_id": "main", "cells": ["c1-a"],
                              "approval": {"approved_by": "사용자 발화 전사", "approved_utc": "2026-01-02T00:00:00Z",
                                           "source": "declaration-popup"}}]}
    core.write_json(camp / "campaign.yaml", decl)
    core.write_json(camp / "cells/c1-a/cell.status.json", {"cell_id": "c1-a", "cell_outcome": "measured"})
    core.write_json(camp / "cells/c1-a/lockset.json", {"id": "c1-a", "provenance": "hand-authored"})
    (camp / "cells/c1-a/config.yaml").write_text("declared_axes:\n  ple_mode: mmap\ntarget_gpu:\n  gpu_model: NVIDIA GB10\n",
                                                 encoding="utf-8")
    core.write_json(camp / "evidence_pointers.json", {"campaign_id": "c1", "pointers": [
        {"kind": "certificate", "path": cert_rel, "cell_id": "c1-a", "node_id": "cluster"},
        {"kind": "bench_report", "path": rep_rel, "cell_id": "c1-a", "node_id": "cluster"},
        {"kind": "devlog", "path": devlog, "cell_id": "c1-a", "node_id": "main"},
        {"kind": "testlog", "path": testlog, "cell_id": "c1-a", "node_id": "main"},
        {"kind": "testlog", "path": "docs/testlog/testlog_26010214_other.md", "cell_id": "c1-b", "node_id": "sub"}]})
    core.write_json(camp / "phases/main/serve.status.json", {"phase": "serve", "state": "done", "node_id": "main",
                                                             "cell_id": "c1-a", "started_utc": "2026-01-02T01:00:00Z",
                                                             "ended_utc": "2026-01-02T01:30:00Z"})
    (camp / "journey.jsonl").write_text(json.dumps({"cell_id": "c1-a", "next_intent": "testlog_26010213_fixture 참조"},
                                                   ensure_ascii=False) + "\n", encoding="utf-8")
    # ★ ACTIVE 는 미끼 — 읽으면 PermissionError 로 죽고, 따라가면 다른 캠페인의 증거가 섞인다.
    (repo / "campaigns/decoy").mkdir()
    core.write_json(repo / "campaigns/decoy/campaign.yaml", {"id": "decoy"})
    active = repo / "campaigns/ACTIVE"
    active.write_text("decoy\n", encoding="utf-8")
    os.chmod(active, 0)
    dstate = {"images": {_DIGEST: {"Id": _DIGEST, "Created": "2026-01-01T00:00:00Z", "Architecture": "arm64",
                                   "Config": {"Env": ["PYTORCH_VERSION=2.9.0a0", "CUDA_VERSION=13.0", "NVIDIA_PYTORCH_VERSION=26.01",
                                                      "PATH=/usr/bin"],
                                              "Labels": {"com.docker.compose.project": "vllm_c1-build_project"}}}},
              "history": {_DIGEST: "RUN |3 VLLM_REPO=https://github.com/vllm-project/vllm.git VLLM_REF=v0.9.0 SM12X_PORT=0 "
                                   "/bin/sh -c build\nCOPY x y\n"
                                   "RUN |2 BASE_INTERNAL_URL=http://base-internal/x CUDA_VERSION=130 /bin/sh -c base\n"}}
    return repo, {"cert": cert_rel, "report": rep_rel, "plan": plan, "devlog": devlog, "testlog": testlog,
                  "docker": dstate, "sweep": sweep, "decl": decl}


def _tree(root: Path) -> dict:
    out = {}
    for p in sorted(root.rglob("*")):
        if p.is_file() and not p.is_symlink() and p.name != "ACTIVE":
            out[p.relative_to(root).as_posix()] = p.read_bytes()
    return out


def _selftest_round2(ck, repo: Path, td: Path, ev: CellEvidence, ident: dict) -> None:
    """2026-09-22(plan_26092119 S2 round 2) 신설 규칙의 자체검사 — F4 identity/빌드 호스트 사실 · F6 원장 타임라인·기동 표지 ·
    F9 full 정의 대조·벤치 명령 재구성. ★ = 음성대조(규칙을 끄면 그 검사가 빨개져야 한다)."""
    # ── F4 checkout 커밋 · q 축 대체 · 드라이버/OS ──
    ck("hf_revision = 체크포인트 HEAD → loose ref · origin 일치", ident.get("hf_revision") == _SHA_H
       and "loose ref" in ident["source"]["hf_revision"] and "일치" in ident["source"]["hf_revision"]
       and str(td) not in ident["source"]["hf_revision"])
    gd = td / "gitprobe"
    for name, head, files, want in (
            ("packed", "ref: refs/heads/dev\n", {"packed-refs": f"{_SHA_H} refs/heads/dev\n"}, _SHA_H),
            ("detached", _SHA_A + "\n", {}, _SHA_A),
            ("dangling", "ref: refs/heads/none\n", {}, None),
            ("traversal", "ref: refs/../../escape\n", {"../escape": _SHA_A + "\n"}, None)):
        g = gd / name / ".git"
        (g / "refs").mkdir(parents=True)          # `refs/../..` 가 OS 에서 실제로 풀리게(가드가 없으면 escape 를 읽는다)
        (g / "HEAD").write_text(head, encoding="utf-8")
        for rel_f, body in files.items():
            (g / rel_f).parent.mkdir(parents=True, exist_ok=True)
            (g / rel_f).write_text(body, encoding="utf-8")
        got, why = _hf_revision(gd / name, "L")
        ck(f"{'★' if want is None else ''}hf_revision 해소 — {name}({why[:60]})", got == want)
    ck("★.git 부재 = revision 미관측(None · 지어내지 않는다)", _hf_revision(td / "no-such-ckpt", "L")[0] is None)
    # 측정 시점 checkout(reflog `logs/HEAD` · 2026-09-22 round 2 리뷰): 이름·메일 칸은 읽지 않는다 · epoch 은 UTC 로 읽는다
    #   (1767139200 = 2025-12-31T00:00:00Z · 1767398400 = 2026-01-03T00:00:00Z · 측정 _M_UTC = 2026-01-02T03:04:05Z)
    fx_mail = "op" + "@" + "fixture.invalid"
    for name, lines, want, why_has in (
            ("rl-same", [(_SHA0, _SHA_H, 1767139200)], _SHA_H, "측정 시점 checkout = 지금 HEAD"),
            ("rl-moved-same", [(_SHA0, _SHA_H, 1767139200), (_SHA_H, _SHA_H, 1767398400)], _SHA_H, "측정 뒤 이동 1회"),
            ("rl-moved", [(_SHA0, _SHA_A, 1767139200), (_SHA_A, _SHA_H, 1767398400)], None, _SHA_A[:12]),
            ("rl-cloned-after", [(_SHA0, _SHA_H, 1767398400)], None, "측정 뒤 받아 온"),
            ("rl-none", None, _SHA_H, "reflog(logs/HEAD) 부재")):
        g = gd / name / ".git"
        (g / "refs/heads").mkdir(parents=True)
        (g / "HEAD").write_text("ref: refs/heads/main\n", encoding="utf-8")
        (g / "refs/heads/main").write_text(_SHA_H + "\n", encoding="utf-8")
        if lines is not None:
            (g / "logs").mkdir()
            (g / "logs/HEAD").write_text("".join(f"{a} {b} Op Name <{fx_mail}> {t} +0900\tclone: from x\n" for a, b, t in lines),
                                         encoding="utf-8")
        got, why = _hf_revision(gd / name, "L", _M_UTC)
        ck(f"{'★' if want is None else ''}hf_revision 측정 시점 대조 — {name}", got == want and why_has in why
           and fx_mail not in why and "Op Name" not in why)
    rg = gd / "remote-diff" / ".git"
    (rg / "refs/heads").mkdir(parents=True)
    (rg / "refs/remotes/origin").mkdir(parents=True)
    (rg / "HEAD").write_text("ref: refs/heads/main\n", encoding="utf-8")
    (rg / "refs/heads/main").write_text(_SHA_H + "\n", encoding="utf-8")
    (rg / "refs/remotes/origin/main").write_text(_SHA_A + "\n", encoding="utf-8")
    got_rd, why_rd = _hf_revision(gd / "remote-diff", "L")
    ck("★origin 이 다른 커밋이면 '일치' 라 적지 않는다(로컬 커밋일 수 있다)", got_rd == _SHA_H and "다른 커밋" in why_rd
       and "와 일치" not in why_rd)
    ck("★측정 시각이 없으면 값은 두되 '미검증' 을 적는다", _hf_revision(gd / "rl-moved", "L")[0] == _SHA_H
       and "미검증" in _hf_revision(gd / "rl-moved", "L")[1])
    ck("identity.quant — 측정 N/A 면 이름 q 축(출처 naming-axis(q))", ident.get("quant") == "nvfp4"
       and str(ident["source"]["quant"]).startswith("naming-axis(q)"))
    iq = identity(dataclasses.replace(ev, certificate={**(ev.certificate or {}), "quantization": "fp8"}))
    ck("★인증서가 quantization 을 들면 그 측정값(이름 축 대체 ✗)", iq["quant"] == "fp8"
       and not str(iq["source"]["quant"]).startswith("naming-axis"))
    ck("★체크포인트 미관측이면 q 대체 없음(None)", identity(dataclasses.replace(ev, triplet={}))["quant"] is None)
    b = ev.build_identity
    ck("★드라이버 원천이 없으면 None + 미관측(지어내지 않는다)", b.get("driver") is None and "미관측" in b["source"]["driver"]
       and b.get("os") is None and "미관측" in b["source"]["os"])
    meta = dict(((ev.sweep or {}).get("index") or {}).get("meta") or {})

    def host(cert_extra: dict, meta_extra: dict) -> dict:
        sw = dict(ev.sweep or {})
        sw["index"] = {**(sw.get("index") or {}), "meta": {**meta, **meta_extra}}
        e2 = dataclasses.replace(ev, certificate={**(ev.certificate or {}), **cert_extra}, sweep=sw,
                                 build_identity={**ev.build_identity, "source": dict(ev.build_identity["source"])})
        _host_facts(e2)
        return e2.build_identity
    h1 = host({"driver_version": "N/A"}, {"driver_version": "580.1", "kernel_release": "6.8.0-fx"})
    ck("★인증서 N/A 는 건너뛰고 스윕 meta driver · OS 는 실제 키가 있을 때만", h1["driver"] == "580.1"
       and "sweep_index.meta" in h1["source"]["driver"] and h1["os"] == "6.8.0-fx" and "kernel_release" in h1["source"]["os"])
    h2 = host({"driver_version": "999.9"}, {"driver_version": "580.1"})
    # 2026-09-29(plan_26092908 §4.5): 우선순위 개정 — attestation 관측 > 스윕 meta > 인증서(스윕 meta 의 옮김) > manifest.
    ck("드라이버 우선순위 — 스윕 meta > 인증서(옮김) · 출처에 (manifest 선언 옮김)", h2["driver"] == "580.1"
       and h2["source"]["driver"].startswith("sweep_index.meta") and _DECLARED_COPY in h2["source"]["driver"]
       and "driver_conflict" not in h2)
    # ── F6 원장 타임라인 · 기동 표지 ──
    tl = event_timeline(repo, ev)
    dump = json.dumps(tl, ensure_ascii=False)
    decl = [r for r in tl if r["kind"] == "budget_declare"]
    ck("타임라인 — 이 셀 라벨의 선언 창 2개(1차 renew 0 · 2차 renew 1 · 측정 포함)", len(decl) == 2
       and "기동 창 1/2" in decl[0]["detail"] and "budget_renew 0회" in decl[0]["detail"] and "이 창 밖" in decl[0]["detail"]
       and "기동 창 2/2" in decl[1]["detail"] and "budget_renew 1회" in decl[1]["detail"] and "이 창 안" in decl[1]["detail"])
    ck("타임라인 — 출처 = 파일:줄(`\\n` 기준)", decl[0]["source"] == "docs/logs/main/events/2026-01.jsonl:2"
       and decl[1]["source"] == "docs/logs/main/events/2026-01.jsonl:9")
    tl_early = event_timeline(repo, dataclasses.replace(ev, certificate={**(ev.certificate or {}), "measured_utc": "2026-01-02T00:30:00Z"}))
    ck("★측정 창 밖 기동은 측정 전·뒤를 적는다(뒤 캠페인의 재기동이 이 측정의 여정으로 읽히지 않게)",
       "측정 전)" in decl[0]["detail"] and all("측정 뒤 — 이 측정과 무관할 수 있다" in r["detail"] for r in tl_early
                                               if r["kind"] == "budget_declare"))
    ck("라벨 없는 clear·arm 해제·보류·thermal 은 열린 창에 귀속", {("budget_clear", "2026-01-02T01:30:00Z"),
       ("budget_clear", "2026-01-02T03:10:00Z"), ("budget_none", "2026-01-02T03:10:01Z"),
       ("watchdog_highrate_hold", "2026-01-02T02:31:00Z"), ("thermal_trip", "2026-01-02T02:45:00Z")}
       <= {(r["kind"], r["utc"]) for r in tl})
    ck("★접두만 같은 다른 셀 라벨(smoke-c1-a-kv8g)의 창과 그 창의 사살·clear·thermal 은 싣지 않는다",
       "kv8g" not in dump and not any(r["utc"] in ("2026-01-02T01:45:00Z", "2026-01-02T01:50:00Z") for r in tl))
    ck("★nomatch 판정·다른 셀 세션·창 밖 pre-clear 는 싣지 않는다", "nomatch" not in dump and "c1-ab" not in dump
       and not any(r["kind"] == "budget_clear" and r["utc"] == "2026-01-02T01:00:00Z" for r in tl)
       and any(r["kind"] == "serve_start" for r in tl))
    ck("★운영자 경로 칸(decl_path)은 detail 로 새지 않는다(양성 목록)", "op-fixture" not in dump and "decl_path" not in dump)
    ck("★IPv4 모양 값은 detail 로 새지 않는다(값 필터)", ".".join(("10", "1", "2", "3")) not in dump
       and any(r["utc"] == "2026-01-02T02:52:00Z" and "mem_avail_mib=7" in r["detail"] and "target=" not in r["detail"] for r in tl))
    ck("★label = 그 행의 라벨(라벨 없는 clear·보류·thermal 은 None · 창 귀속은 detail 이 말한다)",
       all(r["label"] is None and "라벨 없음 →" in r["detail"] for r in tl
           if r["kind"] in ("budget_clear", "budget_none", "watchdog_highrate_hold", "thermal_trip"))
       and all(r["label"] == "smoke-c1-a" for r in decl))
    th = [r for r in tl if r["kind"] == "thermal_trip" and r["utc"] == "2026-01-02T02:45:00Z"]
    ck("★다른 스트림 파일의 같은 사건(ts·kind)은 한 행으로 싣고 사본 출처를 적는다(월 원장 journald 사본)", len(th) == 1
       and th[0]["source"] == "docs/logs/main/events/2026-01.jsonl:14" and "thermal.jsonl:2" in th[0]["detail"])
    ck("★같은 파일 안의 같은 (ts·kind) 두 행은 합치지 않는다(같은 초의 서로 다른 사건)",
       len([r for r in tl if r["kind"] == "watchdog_highrate_hold" and r["utc"] == "2026-01-02T02:31:00Z"]) == 2)
    ck("★다른 셀 라벨의 clear 는 이 셀 창을 닫지 않고 싣지도 않는다", "smoke-zz-other" not in dump
       and "닫힘 budget_clear 2026-01-02T03:10:00Z" in decl[1]["detail"])
    ck("파손 줄은 판독 불가로 기재(부재 ≠ 실패)", any(r["kind"] == "ledger_unparsable_lines" and "1줄" in r["detail"] for r in tl))
    ck("타임라인 utc 순", [r["utc"] for r in tl if r["utc"]] == sorted(r["utc"] for r in tl if r["utc"]))
    # ── 캠페인 경계(2026-09-29 FACT 교정): 같은 셀 이름의 앞선 캠페인 행은 이 셀의 시도로 세지 않는다 ──
    d0u, d1u = decl[0]["utc"], decl[1]["utc"]
    late_decl = {**(ev.declaration or {}), "declared_utc": "2026-01-02T23:00:00Z"}     # 손으로 적힌 선언 시각이 실제 선언보다 늦다
    evb = dataclasses.replace(ev, mode="campaign", declaration=late_decl)
    bnd = event_campaign_boundary(repo, evb)
    tlb = event_timeline(repo, evb)
    db = [r for r in tlb if r["kind"] == "budget_declare"]
    ck("캠페인 경계 = min(declared_utc, 측정 선언 창 시작) · 이전 선언 창 = 같은 셀 이름 · 앞선 캠페인(행은 남는다)",
       bnd["utc"] == d1u and bnd["observed"] and "declared_utc 2026-01-02T23:00:00Z" in bnd["source"]
       and len(db) == 2 and db[0]["campaign_scope"] == CAMPAIGN_SCOPE_PRIOR and PRIOR_CAMPAIGN_NOTE in db[0]["detail"]
       and db[1]["campaign_scope"] == CAMPAIGN_SCOPE_THIS and PRIOR_CAMPAIGN_NOTE not in db[1]["detail"]
       and len(tlb) == len(tl))
    early = dataclasses.replace(evb, declaration={**late_decl, "declared_utc": "2025-12-31T00:00:00Z"})
    ck("★음성대조: 선언 시각이 모든 행보다 앞이면 앞선 캠페인 행 0(경계가 과하게 자르지 않는다)",
       event_campaign_boundary(repo, early)["utc"] == "2025-12-31T00:00:00Z"
       and not any(r.get("campaign_scope") == CAMPAIGN_SCOPE_PRIOR for r in event_timeline(repo, early)))
    rp = dataclasses.replace(evb, mode="publication-replay", campaign_id=None)
    bnr = event_campaign_boundary(repo, rp)
    ck("★재생 모드(캠페인 purge) = 경계 미관측 표지 · 행에 scope 없음(현 규칙: 라벨 전부) · 행 수 같음",
       bnr["utc"] is None and bnr["observed"] is False and "경계 미관측" in bnr["source"]
       and not any("campaign_scope" in r for r in event_timeline(repo, rp)) and d0u < d1u)
    srv = next((r for r in tl if r["kind"] == "server_starting"), {})
    ck("기동 표지 — API 서버 기동 = 측정 창 선언 후 28m00s · 출처 줄 = `\\n` 기준(★`\\r` 진행줄이 줄을 밀지 않는다)",
       "후 28m00s" in srv.get("detail", "") and srv.get("source", "").endswith("lite_engine_c1-a.log:5"))
    ck("기동 표지 — ANSI 제거 후 첫 시각(1번 줄) · prefetch(CIFS) · KV", {"engine_prefetch_disabled", "engine_kv_cache_sized"}
       <= {r["kind"] for r in tl} and next((r["source"] for r in tl if r["kind"] == "engine_log_window_start"), "")
       .endswith("lite_engine_c1-a.log:1"))
    logp = repo / ev.sweep["dir"] / "lite_engine_c1-a.log"
    log_bytes = logp.read_bytes()
    logp.write_bytes(log_bytes.replace(b"01-02 02:", b"01-02 11:"))
    tl_bad = event_timeline(repo, ev)
    ck("★엔진 시계가 측정 창 밖이면 기동 표지를 싣지 않고 그 사실만 적는다",
       [r["kind"] for r in tl_bad if r["source"].startswith(ev.sweep["dir"])] == ["engine_log_clock_unverified"])
    logp.write_bytes(log_bytes)
    ck("★스윕이 출력 평면에 묶이지 않으면 엔진 표지 없음", not any(r["kind"].startswith(("engine_", "server_")) for r in
       event_timeline(repo, dataclasses.replace(ev, sweep={**ev.sweep, "binding": "simlog-copy"}))))
    ck("★원장 노드 = 셀 노드 축(main 셀은 sub 원장을 읽지 않는다 · cluster 는 선언 노드 전부)",
       _event_node_dirs(repo, dataclasses.replace(ev, node="main", declaration=None)) == ["main"]
       and _event_node_dirs(repo, dataclasses.replace(ev, node="sub", declaration=None)) == ["sub"]
       and _event_node_dirs(repo, dataclasses.replace(ev, node="cluster", declaration=None)) == ["main", "sub"])
    ef = event_files(repo, ev)
    ck("원장 파일 = 행을 낸 것만(★다른 노드·다른 셀만 든 파일 제외) · 계보 후보에 실린다",
       ef == ["docs/logs/main/events/2026-01.jsonl", "docs/logs/main/events/thermal.jsonl", "docs/logs/main/events/watchdog.jsonl"]
       and {c["path"] for c in ev.lineage_seeds["evidence_candidates"] if c["kind"] == "events_ledger"} == set(ef))
    # ── F9 full 정의 · 벤치 명령 ──
    bd0 = bench_definition(repo, ev)
    ck("★docs.md 부재 = 정의 미관측 → 충족 판정 None(모름 ≠ 미충족)", bd0["current_full_definition"] is None
       and bd0["meets_current_full"] is None)
    rules = repo / REL_DOCS_RULES
    rules.parent.mkdir(parents=True, exist_ok=True)
    rules.write_text("# docs\n\n| `benchmark/` | full 계측(`lite ∪ GuideLLM × 반복 ≥3`) · lite-only 셀의 경량 report | x |\n",
                     encoding="utf-8")
    bd1 = bench_definition(repo, ev)
    ck("현행 full 정의 원문·줄 · 반복 하한 · 레그", bd1["current_full_definition"] == "full 계측(`lite ∪ GuideLLM × 반복 ≥3`)"
       and bd1["source"] == REL_DOCS_RULES and bd1["source_line"] == 3 and bd1["required_repeats"] == 3
       and bd1["required_legs"] == ["lite", "GuideLLM"])
    # 2026-09-22(round 2 리뷰): 측정 구성 표의 `repeats 3` 은 **요청**이다 — 완주 관측(단일 run 배치 1개)이 this_repeats 다.
    ck("★요청 반복(repeats · 선언)은 this_repeats 가 아니다 — 완주 관측(단일 run 배치) 1 · 요청은 repeats_requested",
       bd1["this_repeats"] == 1 and bd1["repeats_requested"] == 3 and bd1["repeats_source"].startswith("observed(")
       and bd1["meets_current_full"] is False)
    cert = ev.certificate or {}
    rep_p = repo / ev.bench_report_path
    rep_text = rep_p.read_text(encoding="utf-8")
    rep_p.write_text(rep_text.replace("| repeats | 3 |", "| repeats | 3 |\n| repeats_completed | 3 |"), encoding="utf-8")
    bd_c = bench_definition(repo, ev)
    ck("완주 수(repeats_completed) 가 있으면 그것 · ★도구 미관측이면 반복이 충분해도 판정하지 않는다", bd_c["this_repeats"] == 3
       and "repeats_completed" in bd_c["repeats_source"] and bd_c["meets_current_full"] is None)
    bd_v = bench_definition(repo, dataclasses.replace(ev, certificate={**cert, "bench_tool": "vllm-bench-serve"}))
    ck("★full 레그 도구가 GuideLLM 이 아니면 미충족(사유 기재) · 기록 라벨 full 과 갈림을 적는다", bd_v["meets_current_full"] is False
       and any("GuideLLM" in r for r in bd_v["reasons"]) and any("기록된 라벨" in r for r in bd_v["reasons"]))
    bd_g = bench_definition(repo, dataclasses.replace(ev, certificate={**cert, "bench_tool": "guidellm"}))
    ck("GuideLLM × 완주 3 = 충족", bd_g["meets_current_full"] is True)
    rep_p.write_text(rep_text.replace("| repeats | 3 |", "| repeats | 3 |\n| repeats_completed | 3 |")
                     .replace("| bench_mode | full |", "| bench_mode | lite |"), encoding="utf-8")
    ck("★측정 구성 bench_mode=lite 면 완주·도구가 맞아도 미충족", bench_definition(repo, dataclasses.replace(
        ev, certificate={**cert, "bench_tool": "guidellm", "benchmark_mode": "lite"}))["meets_current_full"] is False)
    rep_p.write_text(rep_text, encoding="utf-8")
    sw0 = ev.sweep or {}
    ev_nv = dataclasses.replace(ev, certificate={**cert, "bench_tool": "guidellm"},
                                sweep={**sw0, "index": {k: v for k, v in (sw0.get("index") or {}).items() if k != "verdict_point_level"}})
    bd_r = bench_definition(repo, ev_nv)
    ck("★완주 관측이 없으면(판정점 미기재) 요청 반복 3 으로 충족을 말하지 않는다(None · 요청은 사유로만)", bd_r["this_repeats"] is None
       and bd_r["verdict_level"] is None and bd_r["meets_current_full"] is None and "요청 반복 3(선언" in bd_r["repeats_source"])

    def with_level(measured_extra: dict) -> CellEvidence:
        lv = [dict(x, measured={**(x.get("measured") or {}), **measured_extra}) if _as_int(x.get("level")) == 1 else x
              for x in sw0.get("levels") or []]
        return dataclasses.replace(ev, bench_report_path=None, certificate={**cert, "bench_tool": "guidellm"}, sweep={**sw0, "levels": lv})
    runs3 = [{"run": 1, "measurement_ok": True}, {"run": 2, "measurement_ok": True}, {"run": 3, "measurement_ok": False}]
    bd_u = bench_definition(repo, with_level({"runs": runs3}))
    ck("★레벨 runs[] 는 measurement_ok 인 run 만 센다(실패 run 포함 3 → 완주 2 < 3 미충족)", bd_u["this_repeats"] == 2
       and "runs[]" in bd_u["repeats_source"] and bd_u["meets_current_full"] is False)
    bd_uc = bench_definition(repo, with_level({"runs": runs3, "repeats_completed": 3}))
    ck("레벨 measured.repeats_completed 가 runs[] 보다 먼저", bd_uc["this_repeats"] == 3 and bd_uc["meets_current_full"] is True)
    bd_n = bench_definition(repo, dataclasses.replace(ev, bench_report_path=None, certificate={**cert, "bench_tool": "guidellm"}))
    ck("★반복 기록이 없으면 단일 run 배치 관측 — 1 < 3 미충족", bd_n["this_repeats"] == 1
       and bd_n["repeats_source"].startswith("observed(") and bd_n["meets_current_full"] is False)
    rd = repo / ev.sweep["dir"] / "level_01/run_02"
    rd.mkdir()
    bd_k = bench_definition(repo, dataclasses.replace(ev, bench_report_path=None, certificate={**cert, "bench_tool": "guidellm"}))
    ck("★run_KK/ 가 있는데 집계가 없으면 판정 불가(파일 수로 세지 않는다)", bd_k["this_repeats"] is None
       and "판정 불가" in bd_k["repeats_source"] and bd_k["meets_current_full"] is None)
    rd.rmdir()
    bc0 = bench_command(repo, ev)
    ck("★vllm bench serve 결과 모양이 아니면 재구성하지 않는다", bc0["command"] is None and "모양이 아니다" in bc0["source"])
    bj = repo / ev.sweep["dir"] / "level_01/bench_c1-a.json"
    bj_text = bj.read_text(encoding="utf-8")
    full = {"backend": "openai", "endpoint_type": "openai", "label": None, "model_id": "fixture-served", "tokenizer_id": "/app/x y",
            "num_prompts": 4, "completed": 4, "failed": 0, "total_input_tokens": 4096, "total_output_tokens": 1000,
            "max_concurrency": 1, "request_rate": "inf", "burstiness": 1.0}
    core.write_json(bj, full)
    bc = bench_command(repo, ev)
    cmd = bc["command"] or ""
    ck("벤치 명령 재구성 — 결과 JSON 필드만 · 길이는 나누어떨어질 때 파생(출처 표시)",
       cmd.startswith("vllm bench serve --backend openai --model fixture-served --tokenizer '/app/x y' --num-prompts 4")
       and "--random-input-len 1024" in cmd and "--random-output-len 250" in cmd
       and bc["source"].startswith("reconstructed(bench json)") and bc.get("command_source") == "reconstructed(bench json)"
       and "§" not in cmd + bc["source"]
       and any(f["flag"] == "--random-input-len" and f["source"].startswith("derived(") for f in bc["fields"])
       and "--ignore-eos" in bc["unrecorded"] and "--label" not in cmd)
    core.write_json(bj, {**full, "total_output_tokens": 1001})
    bc_nd = bench_command(repo, ev)
    ck("★나누어떨어지지 않으면 출력 길이 플래그를 싣지 않고 unrecorded 에 사유를 적는다",
       "--random-output-len" not in (bc_nd["command"] or "")
       and any(u.startswith("--random-output-len(") for u in bc_nd["unrecorded"]))
    sw_il = {**ev.sweep, "index": {**ev.sweep["index"], "input_len": 2048}}
    bc_il = bench_command(repo, dataclasses.replace(ev, sweep=sw_il))
    ck("★sweep_index.input_len 과 어긋나면 입력 길이 플래그를 싣지 않는다(고르지 않는다 · 어긋남을 적는다)",
       "--random-input-len" not in (bc_il["command"] or "") and any("≠ sweep_index.input_len 2048" in u for u in bc_il["unrecorded"]))
    bc_nb = bench_command(repo, dataclasses.replace(ev, sweep={**ev.sweep, "binding": "simlog-copy"}))
    ck("★스윕이 출력 평면에 묶이지 않으면 재구성하지 않는다(같은 모양 · command_source None)",
       bc_nb["command"] is None and "command_source" in bc_nb and bc_nb["command_source"] is None)
    core.write_json(bj, {**full, "ignore_eos": True, "trust_remote_code": False})
    bc_b = bench_command(repo, ev)
    ck("★불리언 플래그는 참일 때만(ignore_eos=true → --ignore-eos · false 는 싣지 않는다)", " --ignore-eos" in (bc_b["command"] or "")
       and "--trust-remote-code" not in (bc_b["command"] or "") and "--ignore-eos" not in bc_b["unrecorded"])
    bj.write_text(bj_text, encoding="utf-8")


def _git_fixture(td: Path) -> tuple[Path, dict]:
    """측정 도구 판본 선택용 격리 git 저장소(S2 round 3). 시각은 전부 주입(GIT_*_DATE · reflog 도 그 시각을 쓴다).
    main: c1(01-01 · 도구 v1) → c2(01-02T00:00 · run_bench v2) · side: c1 에서 01-02T01:00 분기 → c4(01-02T02:00 · lite_bench side) ·
    01-02T05:00 main 복귀 → c3(01-03 · run_bench v3). 측정 _M_UTC(01-02T03:04:05Z) 의 체크아웃 = side(c4)."""
    g = td / "gitrepo"
    d = g / REL_BENCH_TOOLS_DIR
    d.mkdir(parents=True)
    genv = {"GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_AUTHOR_NAME": core.SYNTHETIC_NAME, "GIT_AUTHOR_EMAIL": core.SYNTHETIC_EMAIL,
            "GIT_COMMITTER_NAME": core.SYNTHETIC_NAME, "GIT_COMMITTER_EMAIL": core.SYNTHETIC_EMAIL}

    def at(utc: str) -> dict:
        return {**genv, "GIT_AUTHOR_DATE": core.git_date(utc), "GIT_COMMITTER_DATE": core.git_date(utc)}

    def commit(utc: str, msg: str) -> str:
        core.git(g, "add", "-A", env_extra=at(utc))
        core.git(g, "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m", msg, env_extra=at(utc))
        return core.git(g, "rev-parse", "HEAD").stdout.strip()
    sweep_v1 = "\n".join((
        "#!/bin/bash", "# fixture sweep_bench", 'bash "$SDIR/lite_bench.sh" "$CONFIG"', 'bash "$SDIR/run_bench.sh" "${RB_ARGS[@]}"',
        "python3 - <<'PY'", '_img = grep_env(envtext, "IMAGE_TAG") or ""', "_m = re.search(r\"\\bv([0-9.]+)\", _elog)",
        'vllm_build = _m.group(1) if _m else "NA"', 'vllm = os.environ.get("VLLM_VER") or os.environ.get("EASY_VLLM_VERSION") or None',
        "if not vllm:", '    _mi = re.search(r"easy-vllm:([0-9]+\\.[0-9]+\\.[0-9]+)", _img)', "PY", ""))
    # 2026-09-22(적대 검토): run_bench v1 = 엔진 로그 꼬리 상한 없음(주석의 `tail -1 > "$ELOG"` 은 세지 않는다 · ELOG 로 가지 않는 tail 도) ·
    #   lite_bench side = `tail -3 > "$ELOG"`(log_window 대조).
    rb_v1 = "# run_bench v1\n# tail -1 > \"$ELOG\" 는 주석이라 세지 않는다\ndocker logs \"$CTR\" 2>&1 | tail -1 | grep -q ready\n"
    lb_side = "# lite_bench side\ndocker logs \"$CTR\" 2>&1 | tail -3 > \"$ELOG\" || true\n"
    files = {"sweep_bench.sh": sweep_v1, "run_bench.sh": rb_v1, "lite_bench.sh": "# lite_bench v1\n",
             "broad_search.sh": ('SB_ARGS=("$CONFIG" --topology "$TOPO" --tool guidellm)\n# 사용: --tool vllm 은 주석이라 세지 않는다\n'
                                 'bash "$SDIR/sweep_bench.sh" "${SB_ARGS[@]}"\n'),
             "other.sh": "# sweep_bench 를 부르지 않는다\n"}
    for n, body in files.items():
        (d / n).write_text(body, encoding="utf-8")
    core.git(g, "init", "-q", env_extra=at("2026-01-01T00:00:00Z"))
    core.git(g, "symbolic-ref", "HEAD", "refs/heads/main", env_extra=at("2026-01-01T00:00:00Z"))
    c1 = commit("2026-01-01T00:00:00Z", "c1")
    (d / "run_bench.sh").write_text("# run_bench v2\n", encoding="utf-8")
    c2 = commit("2026-01-02T00:00:00Z", "c2")
    core.git(g, "checkout", "-q", "-b", "side", c1, env_extra=at("2026-01-02T01:00:00Z"))
    (d / "lite_bench.sh").write_text(lb_side, encoding="utf-8")
    c4 = commit("2026-01-02T02:00:00Z", "c4")
    core.git(g, "checkout", "-q", "main", env_extra=at("2026-01-02T05:00:00Z"))
    (d / "run_bench.sh").write_text("# run_bench v3\n", encoding="utf-8")
    c3 = commit("2026-01-03T00:00:00Z", "c3")
    return g, {"c1": c1, "c2": c2, "c3": c3, "c4": c4, "rb_v1": rb_v1, "lb_side": lb_side}


def _selftest_round3(ck, repo: Path, td: Path, ev: CellEvidence, fx: dict) -> None:
    """2026-09-22(plan_26092119 S2 round 3) — 측정 환경 관측 · 도구 스냅숏(측정 시점 판본) · vLLM 관측 라벨(producer 검증 · 배너 3계층) ·
    attestation 범위 · 같은 셀의 다른 측정 창 · 계보 후보(resolved.json · 엔진 로그). ★ = 음성대조."""
    from . import lineage
    sweep = fx["sweep"]
    # ── 측정 환경 관측(엔진 로그 NCCL 되읊음) ──
    ip_sub = ".".join(("10", "9", "8", "7"))       # 추적 파일에 주소 리터럴을 두지 않는다(실행 시 조립)
    man = {"nodes": [{"role": "main", "hostname": "fixhost-m1"}, {"role": "sub", "hostname": "fixhost-s2", "host": ip_sub}],
           "interconnect": {"hca_devices": ["mlx5_7", "mlx5_8"]}}
    evm = dataclasses.replace(ev, manifest=man)
    lite = sweep / "lite_engine_c1-a.log"
    lite0 = lite.read_bytes()
    sub_p = f"(EngineCore pid=1) (RayWorkerProc pid=402, ip={ip_sub}) fixhost-s2:402:534 [0] NCCL INFO "
    main_p = "(EngineCore pid=1) \x1b[36m(RayWorkerProc pid=7)\x1b[0m fixhost-m1:7:9 [0] NCCL INFO "
    lite.write_bytes(lite0 + "\n".join((
        sub_p + "NCCL_IB_GID_INDEX set by environment to 3.",
        main_p + "NCCL_IB_GID_INDEX set by environment to 3.",
        "progress 1%\rprogress 2%",
        sub_p + "NCCL_IB_HCA set to =mlx5_7,mlx5_8 [repeated 2x across cluster] (Ray deduplicates logs by default.)",
        sub_p + "Using network IB [repeated 3x across cluster]",
        sub_p + f"NET/IB : Using [0]mlx5_7:1/RoCE [1]mlx5_8:1/RoCE [RO]; OOB mlx5_7:{ip_sub}<0> [repeated 2x across cluster] (Ray x)",
        "(EngineCore pid=1) otherhost-9:1:1 [0] NCCL INFO NCCL_NET_PLUGIN set by environment to spcx",
        main_p + "P2P Chunksize set to 262144",
        main_p + "NCCL_CROSS_NIC set by environment to 1.\x1b[0m",
        main_p + "NCCL_DEBUG_SUBSYS set by environment to fixturesecretterm", "")).encode("utf-8"))
    lv1 = sweep / "level_01/engine_c1-a.log"
    lv1.write_bytes((sub_p + "NCCL_IB_GID_INDEX set by environment to 3.\n").encode("utf-8"))
    (sweep / "level_02").mkdir()
    (sweep / "level_02/engine_c1-a.log").write_bytes((sub_p + "NCCL_IB_TIMEOUT set by environment to 22.\n").encode("utf-8"))
    base_ln = lite0.decode("utf-8").count("\n") + (0 if lite0.endswith(b"\n") else 1)
    env = measurement_env_observed(repo, evm)
    by = {(r["kind"], r["key"], r["node"]): r for r in env}
    gid_sub = by.get(("env-echo", "NCCL_IB_GID_INDEX", "sub"), {})
    ck("env 되읊음 — 키·값(정수 끝 마침표 제거)·노드(Ray ip → manifest 치환표 <node:sub>)·출처 줄(`\\n` 기준)",
       gid_sub.get("value") == "3" and gid_sub.get("source") == f"{_rel(repo, lite)}:L{base_ln + 1}"
       and gid_sub.get("seen_in_logs") == 2)
    ck("같은 키·값이라도 노드가 다르면 다른 행(호스트 → <node:main> · ANSI 제거)",
       by.get(("env-echo", "NCCL_IB_GID_INDEX", "main"), {}).get("source") == f"{_rel(repo, lite)}:L{base_ln + 2}")
    hca = by.get(("param-echo", "NCCL_IB_HCA", "sub"), {})
    ck("★장치 이름은 기계 치환(<nic:cluster>) · Ray 중복 제거 수 · `\\r` 진행줄이 줄 번호를 밀지 않는다",
       hca.get("value") == "=<nic:cluster>,<nic:cluster>" and hca.get("ray_repeated") == 2 and "value_note" in hca
       and hca.get("source", "").endswith(f":L{base_ln + 4}") and "mlx5_7" not in json.dumps(env))
    ck("NCCL 런타임 관측(nccl:network — env 이름과 섞이지 않는 소문자 키)",
       by.get(("runtime", "nccl:network", "sub"), {}).get("value") == "IB")
    ck("★NET/IB 장치 목록 — Ray 중복 제거 꼬리는 값이 아니다 · 장치·주소 치환", by.get(("runtime", "nccl:net_ib_devices", "sub"), {})
       .get("value") == "[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:sub><0>")
    ck("★치환표가 모르는 호스트 = 노드 None(추측 ✗)", by.get(("env-echo", "NCCL_NET_PLUGIN", None), {}).get("value") == "spcx")
    ck("★NCCL_ 이 아닌 'set to'(P2P Chunksize)는 env 가 아니다", not any("Chunksize" in r["key"] or r["value"] == "262144" for r in env))
    ck("★PII 용어는 값으로 새지 않는다(치환 <redacted>)", "fixturesecretterm" not in json.dumps(env, ensure_ascii=False)
       and by.get(("env-echo", "NCCL_DEBUG_SUBSYS", "main"), {}).get("value") == "<redacted>")
    ck("★색인에 없는 레벨 디렉터리(level_02)의 로그는 이 측정이 아니다", not any(r["key"] == "NCCL_IB_TIMEOUT" for r in env))
    ck("★주소 리터럴 비유출", ip_sub not in json.dumps(env))
    ck("★줄 끝 ANSI 리셋은 값이 아니다(제거 후 정수 마침표도 뗀다)",
       by.get(("env-echo", "NCCL_CROSS_NIC", "main"), {}).get("value") == "1")
    man2 = {"nodes": [{"role": "main", "hostname": "fixhost-m1"}, {"role": "sub", "hostname": "fixhost-s1"},
                      {"role": "sub", "hostname": "fixhost-s2", "host": ip_sub}], "interconnect": man["interconnect"]}
    env2 = measurement_env_observed(repo, dataclasses.replace(ev, manifest=man2))
    g2 = next((r for r in env2 if r["key"] == "NCCL_IB_GID_INDEX" and r.get("node_label") == "sub2"), {})
    ck("★계약 밖 노드 라벨(sub2 — 같은 role 둘)은 node None + node_label 로(main|sub 로 접지 않는다)",
       bool(g2) and g2.get("node") is None and not any(r["node"] not in ("main", "sub", None) for r in env2))
    ck("★스윕이 출력 평면에 묶이지 않으면 [] (그 로그는 이 측정의 것이 아니다)",
       measurement_env_observed(repo, dataclasses.replace(evm, sweep={**evm.sweep, "binding": "simlog-copy"})) == [])
    # ── 계보 후보: 이 측정의 엔진 로그 · resolved.json(묶였을 때만) ──
    ev_c = from_campaign(repo, "c1", "c1-a", docker=_fake_docker(fx["docker"]))
    cands = {c["path"]: c["kind"] for c in ev_c.lineage_seeds["evidence_candidates"]}
    ck("계보 후보 — 이 측정 엔진 로그(lite · 색인 레벨) · 묶인 resolved.json",
       cands.get(_rel(repo, lite)) == "engine_log" and cands.get(_rel(repo, lv1)) == "engine_log"
       and cands.get("output/multi/resolved.json") == "resolve"
       and not any("level_02" in p for p in cands))
    rsv = repo / "output/multi/resolved.json"
    rsv_text = rsv.read_text(encoding="utf-8")
    rsv.write_text(rsv_text.replace('"vllm_version": "0.9.0"', '"vllm_version": "0.8.0"').replace(_SHA_A, "b" * 40), encoding="utf-8")
    ev_nr = from_campaign(repo, "c1", "c1-a", docker=_fake_docker(fx["docker"]))
    ck("★다른 버전의 해소값(묶이지 않음)은 계보 후보가 아니다", ev_nr.resolve is None and "output/multi/resolved.json"
       not in {c["path"] for c in ev_nr.lineage_seeds["evidence_candidates"]})
    rsv.write_text(rsv_text, encoding="utf-8")
    lite.write_bytes(lite0)
    lv1.unlink()
    (sweep / "level_02/engine_c1-a.log").unlink()
    (sweep / "level_02").rmdir()
    # ── attestation 범위 ──
    att = repo / "output/multi/benchlog/attestation_c1-a.json"
    os.utime(att, (1767225600, 1767225600))        # 2026-01-01T00:00:00Z — 결정론(벽시계 ✗)
    sc = attestation_scope(repo, ev_c)
    ck("attestation 범위 — 셀 이름 바인딩 · 같은 config · 실행 동일성 미검증 · written_utc = mtime(출처가 그렇게 말한다)",
       sc and sc["config"] == "c1-a" and sc["bound_by"] == "cell-name" and sc["scope"].startswith("same config(c1-a)")
       and "미검증" in sc["scope"] and sc["written_utc"] == "2026-01-01T00:00:00Z" and "mtime" in sc["written_utc_source"])
    ck("attestation 범위 — 문서 시각 칸이 있으면 그것", attestation_scope(repo, dataclasses.replace(
        ev_c, attestation={**ev_c.attestation, "captured_utc": "2026-01-02T00:00:00Z"}))["written_utc_source"] == "attestation captured_utc")
    # F7(plan_26092908 §4.5): 작성 시각 대 측정 창 — 측정 끝(_M_UTC) 뒤 작성 = 측정 뒤 재기동 · 앞 = 측정 전/끝 이전
    mk_ = _measured_key(repo, ev_c)
    sc7 = attestation_scope(repo, dataclasses.replace(ev_c, attestation={**ev_c.attestation, "captured_utc": "2099-01-01T00:00:00Z"}))
    ck("★F7 작성 > 측정 끝 = '측정 뒤 재기동의 관측'(scope · timing_note · 창)", mk_ and sc7.get("timing") == "after-measurement"
       and "측정 뒤 재기동" in sc7["scope"] and "측정 뒤 재기동" in sc7["timing_note"] and sc7["measure_window"]["end_utc"] == mk_)
    ck("F7 음성대조: 작성 ≤ 측정 끝 = 측정 뒤 ✗(측정 전 또는 끝 이전)", sc.get("timing") in ("before-measurement", "before-end")
       and "측정 뒤" not in sc["scope"])
    ck("F7 호스트 사실 출처에도 같은 문장(드라이버 · wheel 메타 출처가 창을 말한다)",
       (_attestation_timing_note(dataclasses.replace(ev_c, attestation={**ev_c.attestation, "captured_utc": "2099-01-01T00:00:00Z"}))
        or "").endswith("측정 뒤 재기동의 관측"))
    # F2(plan_26092908 §4.5): 측정 digest 없음 → 현 로컬 태그 inspect(Id · Created) — 측정 전 빌드면 same_build · 뒤면 표시만
    def _dk2(created):
        return _fake_docker({"images": {"fx:tag": {"Id": "sha256:" + "e" * 64, "Created": created, "Config": {}}}, "history": {}})
    e2d = dataclasses.replace(ev_c, build_identity={**ev_c.build_identity, "image_digest": None, "image_tag": "fx:tag",
                                                    "source": dict(ev_c.build_identity.get("source") or {})})
    _local_image_digest(repo, e2d, _dk2("2025-12-31T00:00:00.123456789+09:00"))
    loc = e2d.build_identity.get("image_digest_local") or {}
    ck("F2 로컬 이미지 Created < 측정 → same_build · Id · 출처(태그는 가변 포인터 · 측정 전 빌드)",
       loc.get("same_build") is True and loc.get("id") == "sha256:" + "e" * 64 and "측정 전 빌드" in loc.get("source", "")
       and "태그는 가변 포인터" in loc["source"] and e2d.build_identity.get("image_digest") is None)
    _local_image_digest(repo, e2d, _dk2("2099-01-01T00:00:00Z"))
    loc2 = e2d.build_identity.get("image_digest_local") or {}
    ck("★F2 음성대조: Created ≥ 측정 → same_build ✗(측정 이미지 아님) · 측정 digest 가 있으면 관측하지 않는다",
       loc2.get("same_build") is False and "측정 뒤 빌드" in loc2.get("source", "")
       and (_local_image_digest(repo, ev_c, _dk2("2025-12-31T00:00:00Z")) or "image_digest_local" not in ev_c.build_identity))
    # F9(plan_26092908 §4.5): native 셀 — 스윕 엔진 로그 자리 대신 serve proof evidence_dir 의 보존 엔진 로그(전체 출력 · 꼬리 캡처 ✗)
    nd = repo / "docs/simlog/fx_native/logs"
    nd.mkdir(parents=True, exist_ok=True)
    (nd / "main-vllm-serve.log").write_text("INFO boot\nfxhost:1:1 [0] NCCL INFO NCCL_IB_DISABLE set by environment to 1.\n",
                                            encoding="utf-8")
    ev9 = dataclasses.replace(ev_c, plane="native", serve_proof={**(ev_c.serve_proof or {}), "evidence_dir": "docs/simlog/fx_native"})
    lg9 = _measured_engine_logs(repo, ev9)
    rows9 = measurement_env_observed(repo, ev9)
    ck("F9 native — 보존 엔진 로그가 측정 로그 · env 관측 원천(창 표지 없음)",
       nd / "main-vllm-serve.log" in lg9[0] and "evidence_dir" in lg9[1]
       and any(r["key"] == "NCCL_IB_DISABLE" and r["source"].startswith("docs/simlog/fx_native/logs/main-vllm-serve.log")
               and "log_window" not in r for r in rows9))
    ck("★F9 음성대조: docker 평면 · 절대경로 · 저장소 밖 evidence_dir 은 읽지 않는다",
       _native_preserved_engine_logs(repo, dataclasses.replace(ev9, plane="docker")) == []
       and _native_preserved_engine_logs(repo, dataclasses.replace(ev9, serve_proof={"evidence_dir": str(nd.parent)})) == []
       and _native_preserved_engine_logs(repo, dataclasses.replace(ev9, serve_proof={"evidence_dir": "docs/../../x"})) == []
       and nd / "main-vllm-serve.log" not in _measured_engine_logs(repo, ev_c)[0])
    import shutil as _sh
    _sh.rmtree(repo / "docs/simlog/fx_native")
    att_text = att.read_text(encoding="utf-8")
    att.unlink()
    core.write_json(repo / "output/multi/benchlog/attestation_c1-build.json", {
        "config": "c1-build", "image_tag": "fixture:0.9.0-source", "checks": [{"axis": "torch", "node": "sub", "observed": "x"}]})
    ev_lb = from_campaign(repo, "c1", "c1-a", docker=_fake_docker(fx["docker"]))
    sc2 = attestation_scope(repo, ev_lb)
    ck("★compose 라벨 대체로 묶인 다른 config = 이 셀 실행이 아니다(계약 문자열 그대로)",
       sc2 and sc2["scope"] == ATTESTATION_SCOPE_OTHER_RUN and sc2["bound_by"] == "compose-label" and sc2["config"] == "c1-build")
    (repo / "output/multi/benchlog/attestation_c1-build.json").unlink()
    ck("★묶인 attestation 이 없으면 None", attestation_scope(repo, from_campaign(repo, "c1", "c1-a", docker=_fake_docker(fx["docker"])))
       is None)
    att.write_text(att_text, encoding="utf-8")
    # ── 같은 셀의 다른 측정 창(과거 발행 기록 · 예산 선언 유무) ──
    ident = ev_c.lineage_seeds["identity"]
    made: list[Path] = []

    def rec(rid: str, mu: str, cell: str = "c1-a", identity=None, node: str = "cluster") -> None:
        sl = f"docs/simlog/26010100_{rid}"
        core.write_json(repo / sl / "sweep_index.json", {"generated_utc": mu, "meta": {"config_name": cell, "measured_node": node}})
        core.write_json(repo / core.REL_EVIDENCE_DIR / f"{rid}.json", {
            "identity": identity or ident, "benchmark": {"measured_utc": mu, "verdict": "PASS"}, "raw_log_paths": {"simlog": sl}})
        made.extend([repo / sl / "sweep_index.json", repo / sl, repo / core.REL_EVIDENCE_DIR / f"{rid}.json"])
    rec("old_c1_a", "2026-01-01T05:00:00Z")
    rec("gap_c1_a", "2026-01-02T01:35:00Z")
    rec("kv_c1_a", "2026-01-02T01:45:00Z")
    rec("win_c1_a", "2026-01-02T02:40:00Z")
    rec("late_c1_a", "2026-01-02T03:30:00Z")
    rec("same_c1_a", _M_UTC)
    rec("othercell_c1_a", "2026-01-01T06:00:00Z", cell="c1-b")
    rec("othermodel_c1_a", "2026-01-01T07:00:00Z", identity={**ident, "model": "different-model"})
    tl = event_timeline(repo, ev_c)
    ms = {r["utc"]: r for r in tl if r["kind"] == "measurement"}
    gap = ms.get("2026-01-02T01:35:00Z", {})
    ck("과거 측정 — 원장 창 밖 = 'measurement (no budget event recorded)' · 출처 = 발행 기록 measured_utc 줄 · node None(축은 detail)",
       gap.get("label") == "measurement (no budget event recorded)" and gap.get("node") is None and "측정 노드 축 cluster" in gap["detail"]
       and gap.get("source", "").startswith("docs/_evidence/gap_c1_a.json:") and "측정 2026-01-02T03:04:05Z 전" in gap["detail"]
       and "직후 예산 행 budget_declare(smoke-c1-a-kv8g)" in gap["detail"])
    # 2026-09-22(S2 round 3 적대 검토): 판정은 그 시각을 **관측한** 원장만 — 끊긴 원장은 증인이 아니고, 열린 채 끝난 창은 그 뒤를 덮지 않는다.
    ck("★그 시각을 관측하지 않은 원장(sub · 기록 02:30 한 점)은 '선언 없음' 의 증인이 아니다 — 관측 밖이라 적는다",
       "이 시각을 관측한 원장(main)" in gap.get("detail", "") and "이 시각을 관측하지 않은 원장: sub(기록 2026-01-02T02:30:00Z~" in gap["detail"])
    old = ms.get("2026-01-01T05:00:00Z", {})
    ck("★원장 기록 범위 밖(모든 노드 첫 행 이전)의 측정 = 'ledger unobserved'(선언 없음이라 말하지 않는다)",
       old.get("label") == "measurement (ledger unobserved)" and "기록 범위 밖" in old.get("detail", ""))
    ck("★원장이 끝난 뒤의 측정 = 'ledger unobserved'(sub 의 열린 채 끝난 창이 덮는다고 읽지 않는다)",
       ms.get("2026-01-02T03:30:00Z", {}).get("label") == "measurement (ledger unobserved)")
    src_f, _, src_ln = gap.get("source", "::").rpartition(":")
    ck("과거 측정 출처 줄이 실제로 measured_utc 를 든다", _as_int(src_ln) is not None and
       "2026-01-02T01:35:00Z" in (repo / src_f).read_text(encoding="utf-8").split("\n")[_as_int(src_ln) - 1])
    ck("★다른 라벨 선언 창 안의 측정 = 'other label's budget window'(이 셀 선언 없음)",
       ms.get("2026-01-02T01:45:00Z", {}).get("label") == "measurement (other label's budget window)"
       and "smoke-c1-a-kv8g" in ms["2026-01-02T01:45:00Z"]["detail"])
    ck("이 셀 선언 창 안의 다른 측정 = 'budget window recorded'",
       ms.get("2026-01-02T02:40:00Z", {}).get("label") == "measurement (budget window recorded)")
    ck("★이 측정 자신(measured_utc 같음) · 다른 셀 · 다른 모델의 기록은 행이 아니다",
       set(ms) == {"2026-01-01T05:00:00Z", "2026-01-02T01:35:00Z", "2026-01-02T01:45:00Z", "2026-01-02T02:40:00Z",
                   "2026-01-02T03:30:00Z"})
    ev_dir = repo / "docs/logs/main/events"
    extra = ev_dir / "zz-extra.jsonl"            # main 원장을 04:00 까지 늘린다(비예산 행 — 창·예산 행 불변)
    extra.write_text(json.dumps({"kind": "thermal_trip", "soc_temp_c": 90, "ts": "2026-01-02T04:00:00Z"}) + "\n", encoding="utf-8")
    late = {r["utc"]: r for r in event_timeline(repo, ev_c) if r["kind"] == "measurement"}.get("2026-01-02T03:30:00Z", {})
    ck("★다른 노드 스트림이 열린 채 끝난 창(sub smoke-c1-b · 마지막 행 02:30)은 그 뒤 시각을 덮지 않는다(main 관측 · 창 없음 = 선언 없음)",
       late.get("label") == "measurement (no budget event recorded)" and "smoke-c1-b" not in late.get("detail", "")
       and "이 시각을 관측하지 않은 원장: sub" in late["detail"])
    dang = ev_dir / "2025-12.jsonl"             # 같은 노드의 다른 스트림이 창을 연 채 끝난다(01:31) — 01:35 의 창 상태는 미관측
    dang.write_text(json.dumps({"kind": "budget_declare", "label": "smoke-zz-dangling", "ts": "2026-01-02T01:31:00Z"}) + "\n",
                    encoding="utf-8")
    g2 = {r["utc"]: r for r in event_timeline(repo, ev_c) if r["kind"] == "measurement"}.get("2026-01-02T01:35:00Z", {})
    ck("★스트림 끝에서 열린 채 끝난 창 뒤의 시각 = 'budget window closure unobserved'(덮었다고도 · 선언 없었다고도 말하지 않는다)",
       g2.get("label") == "measurement (budget window closure unobserved)" and "smoke-zz-dangling" in g2.get("detail", "")
       and "닫힘 미관측(스트림 마지막 행 2026-01-02T01:31:00Z)" in g2["detail"])
    dang.unlink()
    extra.unlink()
    ef = set(event_files(repo, ev_c))
    ck("과거 측정의 발행 기록·simlog 색인이 계보 후보 입력", {"docs/_evidence/old_c1_a.json",
                                                    "docs/simlog/26010100_old_c1_a/sweep_index.json"} <= ef)
    ev_nl = dataclasses.replace(ev_c, node="main", declaration={"nodes": [{"node_id": "nope", "role": "main"}]})
    ck("★원장 노드가 없으면 'ledger unobserved'(선언 없음이라 말하지 않는다)",
       {r["label"] for r in event_timeline(repo, ev_nl) if r["kind"] == "measurement"} == {"measurement (ledger unobserved)"})
    for p in made:
        if p.is_file():
            p.unlink()
    for p in made:
        if p.is_dir():
            p.rmdir()
    # ── 엔진 자기보고(배너) 3계층 ──
    evb = from_campaign(repo, "c1", "c1-a", docker=_fake_docker(fx["docker"]))
    ck("★배너 원천이 없으면 unobserved(인증서 값을 자기보고로 옮기지 않는다)", vllm_observed(evb)["engine_self_report"] is None
       and vllm_observed(evb)["engine_self_report_basis"] == "unobserved" and vllm_observed(evb)["certificate_vllm_version"] == "0.9.0")
    bl = repo / "output/multi/benchlog"
    made_b: list[Path] = []

    def sweep_dir(name: str, gu: str, digest: str = _DIGEST, tag: str = "fixture:0.9.0-source", build: str = "NA") -> Path:
        d = bl / f"sweep_{name}"
        (d / "level_01").mkdir(parents=True)
        core.write_json(d / "sweep_index.json", {"config": name, "generated_utc": gu, "levels": [{"level": 1}], "meta": {
            "config_name": name, "image_digest": digest, "image_digest_source": "measured(docker inspect .Image)",
            "image_tag": tag, "image_tag_source": "measured(docker inspect)", "vllm_build": build}})
        made_b.append(d)
        return d

    def fail_log(cfg: str, stamp: str, ver: str, tag: str = "fixture:0.9.0-source", pre: str = "") -> None:
        d = bl / f"serve_fail_{cfg}"
        d.mkdir()
        (d / "master_exited.log").write_text(f"== banner ==\n{pre}(APIServer pid=1) INFO {stamp} [api_utils.py:1]  ▄▄ █  version {ver}\n",
                                             encoding="utf-8")
        (repo / f"output/multi/envs/.env.{cfg}").write_text(f"IMAGE_TAG={tag}\n", encoding="utf-8")
        made_b.extend([d, repo / f"output/multi/envs/.env.{cfg}"])
    sweep_dir("c1-z", "2026-01-01T12:00:00Z")
    # ★같은 줄 앞 `\r` 진행 조각의 다른 시각(12-30)을 배너 시각으로 읽으면 괄호 밖이 된다 — 배너 조각의 시각만 쓴다.
    fail_log("c1-y", "01-02 01:00:00", "0.9.0rc2.dev5+gfix", pre="(Worker pid=2) INFO 12-30 00:00:00 [p.py:1] load 5%\r")
    fail_log("c1-late", "01-03 01:00:00", "0.9.9-late")
    fail_log("c1-othertag", "01-02 01:30:00", "0.9.8-other", tag="fixture:0.8.0-source")
    vo = vllm_observed(evb)
    ck("digest 미기록 로그 = 같은 태그 + 앞뒤 측정 스윕 괄호로 **추론**(basis · 출처가 추론이라 말한다)",
       vo["engine_self_report"] == "0.9.0rc2.dev5+gfix" and vo["engine_self_report_basis"] == "same-digest-inferred"
       and "추론" in vo["engine_self_report_source"] and "serve_fail_c1-y/master_exited.log:2" in vo["engine_self_report_source"]
       and "컨테이너 시계 01-02 01:00:00" in vo["engine_self_report_source"])
    ck("★괄호 밖(뒤 스윕 없음) · 다른 IMAGE_TAG 의 로그는 쓰지 않는다", "0.9.9-late" not in json.dumps(vo) and "0.9.8-other" not in json.dumps(vo))
    ck("추론 배너 로그는 계보 후보", "output/multi/benchlog/serve_fail_c1-y/master_exited.log" in {
        c["path"] for c in from_campaign(repo, "c1", "c1-a", docker=_fake_docker(fx["docker"])).lineage_seeds["evidence_candidates"]})
    ck("★연도 없는 컨테이너 시각 — 측정 연도로 괄호 밖이면 전해로 읽는다(연말 로그 · 연초 측정)",
       (_bracketed("12-31 18:00:00", [{"utc": "2025-12-31T12:00:00Z"}, {"utc": _M_UTC}], _M_UTC) or ("",))[0] == "2025-12-31T18:00:00Z"
       and _bracketed("12-31 18:00:00", [{"utc": "2026-01-01T12:00:00Z"}, {"utc": _M_UTC}], _M_UTC) is None)
    # 2026-09-22(S2 round 3 적대 검토): 괄호 스윕의 **태그** 조건이 죽어 있었다(_digest_sweeps 에 tag=None) — 같은 digest 를 다른 태그로
    #   잰 스윕은 "그 시각에 이 태그가 그 digest 였다" 의 근거가 아니다.
    z_idx = bl / "sweep_c1-z/sweep_index.json"
    z_text = z_idx.read_text(encoding="utf-8")
    z_doc = json.loads(z_text)
    z_doc["meta"]["image_tag"] = "fixture:0.8.0-source"
    core.write_json(z_idx, z_doc)
    vz = vllm_observed(evb)
    ck("★괄호 스윕은 같은 **태그**로 그 digest 를 잰 것만(같은 digest · 다른 태그 = 괄호 ✗ → 추론 ✗)",
       vz["engine_self_report"] is None and vz["engine_self_report_basis"] == "unobserved")
    z_idx.write_text(z_text, encoding="utf-8")
    fail_log("c1-y2", "01-02 02:00:00", "0.9.0rc2.dev6+gother")
    ck("★같은 계층에서 값이 둘이면 고르지 않는다(conflict)", vllm_observed(evb)["engine_self_report"] is None
       and vllm_observed(evb)["engine_self_report_basis"] == "conflict")
    z2 = sweep_dir("c1-w", "2026-01-01T13:00:00Z")
    (z2 / "level_01/engine_c1-w.log").write_text("x\n(APIServer pid=1) INFO 01-01 12:59:00 [a.py:1] vLLM API server version "
                                                 "0.9.0rc2.dev7+grec\n", encoding="utf-8")
    vr = vllm_observed(evb)
    ck("digest 를 **기록한** 다른 스윕의 로그가 추론보다 먼저(same-digest-recorded)", vr["engine_self_report"] == "0.9.0rc2.dev7+grec"
       and vr["engine_self_report_basis"] == "same-digest-recorded" and "sweep_c1-w/level_01/engine_c1-w.log:2" in vr["engine_self_report_source"])
    sweep_dir("c1-v", "2026-01-01T14:00:00Z", digest="sha256:" + "9" * 64, build="7.7.7-otherimage")
    (bl / "sweep_c1-broken").mkdir()
    (bl / "sweep_c1-broken/sweep_index.json").write_text("{", encoding="utf-8")
    made_b.append(bl / "sweep_c1-broken")
    ck("★이웃 셀의 깨진 스윕 색인은 이 셀 발행을 막지 않는다(건너뛴다 · 이 측정 자신의 증거는 fail-closed 유지)",
       _code(vllm_observed, evb) is None and vllm_observed(evb)["engine_self_report_basis"] == "same-digest-recorded")
    ck("★다른 digest 스윕의 vllm_build 는 쓰지 않는다", "7.7.7" not in json.dumps(vllm_observed(evb)))
    idx_p = sweep / "sweep_index.json"
    idx_text = idx_p.read_text(encoding="utf-8")
    idx_doc = json.loads(idx_text)
    idx_doc["meta"]["vllm_build"] = "0.9.0rc2.dev8+gthis"
    core.write_json(idx_p, idx_doc)
    vt = vllm_observed(from_campaign(repo, "c1", "c1-a", docker=_fake_docker(fx["docker"])))
    ck("이 측정의 meta.vllm_build 가 있으면 그것(this-measurement · 색인 줄)", vt["engine_self_report"] == "0.9.0rc2.dev8+gthis"
       and vt["engine_self_report_basis"] == "this-measurement" and "sweep_c1-a/sweep_index.json:" in vt["engine_self_report_source"])
    idx_p.write_text(idx_text, encoding="utf-8")
    for p in sorted(made_b, key=lambda x: -len(x.parts)):
        if p.is_dir():
            for f in sorted(p.rglob("*"), key=lambda x: -len(x.parts)):
                f.unlink() if f.is_file() else f.rmdir()
            p.rmdir()
        elif p.is_file():
            p.unlink()
    ck("wheel_meta 는 관측자가 없다(엔진 로그 값을 wheel 메타로 부르지 않는다)", vt["wheel_meta"] is None
       and "엔진 로그에서" in vt["wheel_meta_source"])
    ck("★git 이 아닌 저장소 = 인증서 producer 미검증(지어내지 않는다)", vt["certificate_vllm_version_producer"] == "unverified")
    # ── 측정 도구 스냅숏(측정 시점 판본 · reflog) · 인증서 vllm_version producer 검증 ──
    g, revs = _git_fixture(td)
    sw_g = {"dir": "output/multi/benchlog/sweep_c1-a", "binding": "output", "generated_utc": _M_UTC, "levels": [{"level": 1}],
            "index": {"lite": {"gen_tps": 1.0}, "meta": {"bench_tool": "vllm-bench-serve", "vllm_version": "0.9.0", "vllm_build": "NA",
                                                         "image_tag_declared": "easy-vllm:0.9.0rc2-cu130-source"}}}
    evg = dataclasses.replace(ev, repo=str(g), sweep=sw_g, certificate={"measured_utc": _M_UTC, "vllm_version": "0.9.0"},
                              lineage_seeds={"evidence_candidates": [{"path": "x.json", "kind": "raw_json"}]})
    draft = td / "draft-g"
    snaps = tool_snapshots(g, evg, draft)
    sn = {r["name"]: r for r in snaps}
    ck("도구 스냅숏 — 측정 시각의 체크아웃(reflog → side 브랜치 c4) · 이름·경로·rev·draft 상대 경로(계약 키)",
       set(sn) == {"sweep_bench.sh", "run_bench.sh", "lite_bench.sh"} and all(r["head_at_measurement"] == revs["c4"] for r in snaps)
       and sn["lite_bench.sh"]["git_rev"] == revs["c4"] and sn["run_bench.sh"]["git_rev"] == revs["c1"]
       and sn["run_bench.sh"]["snapshot_rel"] == f"inputs/sources/run_bench.sh@{revs['c1'][:12]}"
       and all({"name", "repo_path", "git_rev", "snapshot_rel"} <= set(r) for r in snaps) and "reflog" in snaps[0]["rev_method"])
    ck("★committer date 로 고르면 main 의 c2(run_bench v2)였다 — reflog 가 실제 체크아웃(side)을 고른다",
       (draft / sn["run_bench.sh"]["snapshot_rel"]).read_text(encoding="utf-8") == revs["rb_v1"]
       and (draft / sn["lite_bench.sh"]["snapshot_rel"]).read_text(encoding="utf-8") == revs["lb_side"])
    ck("스냅숏 바이트 = git show <rev>:<path>", all((draft / r["snapshot_rel"]).read_bytes()
                                                   == core.git_bytes(g, "show", f"{r['git_rev']}:{r['repo_path']}") for r in snaps))
    ck("★다른 브랜치의 측정 시점 판본이면 다음 변경을 말하지 않는다(at..HEAD 는 무관 커밋) · '변경 없음' 이 아니라 미관측이라 적는다",
       sn["run_bench.sh"]["next_rev"] is None and sn["run_bench.sh"]["next_rev_basis"].startswith("미관측")
       and "후손이 아니다" in sn["run_bench.sh"]["next_rev_basis"])
    ck("★드라이버 배제 — 그 판본 broad_search 는 --tool guidellm(주석의 --tool 은 세지 않는다) ≠ 이 측정 vllm-bench-serve",
       "broad_search.sh" not in sn and "broad_search.sh 배제" in (sn["sweep_bench.sh"]["driver_note"] or "")
       and "배제로 추론" in sn["sweep_bench.sh"]["driver_note"] and "저장소 밖 호출자" in sn["sweep_bench.sh"]["driver_note"])
    cands = {c["path"]: c for c in evg.lineage_seeds["evidence_candidates"]}
    ck("계보 후보 = tool-source@rev · origin git:<rev>:<path> · 기존 후보 보존 · lineage 검증 통과",
       all(cands.get(r["snapshot_rel"], {}).get("origin") == f"git:{r['git_rev']}:{r['repo_path']}" for r in snaps)
       and "x.json" in cands and all(lineage._tool_candidate_problem(g, cands[r["snapshot_rel"]]) is None for r in snaps))
    n_before = len(evg.lineage_seeds["evidence_candidates"])
    tool_snapshots(g, evg, draft)
    ck("재호출 멱등(후보 중복 ✗)", len(evg.lineage_seeds["evidence_candidates"]) == n_before)
    ck("★draft 밖에 쓰지 않는다(inputs/sources 만)", sorted(p.relative_to(draft).parts[:2] for p in draft.rglob("*") if p.is_file())
       == [("inputs", "sources")] * len(snaps))
    # ── 엔진 로그 꼬리 창(log_window · 2026-09-22 적대 검토): 측정 도구 판본의 `tail -N > "$ELOG"` 상한에 닿은 로그만 표시 ──
    sdir = g / "output/multi/benchlog/sweep_c1-a"
    (sdir / "level_01").mkdir(parents=True)
    nccl = "(EngineCore pid=1) fixhost-m1:7:9 [0] NCCL INFO NCCL_IB_GID_INDEX set by environment to 3.\n"
    (sdir / "lite_engine_c1-a.log").write_text("x\ny\n" + nccl, encoding="utf-8")               # 3줄 = lite 상한 3 도달
    (sdir / "level_01/engine_c1-a.log").write_text(nccl.replace("GID_INDEX", "TC"), encoding="utf-8")  # run_bench v1 = 상한 없음
    wg = {r["key"]: r for r in measurement_env_observed(g, evg)}
    ck("log_window — lite 로그가 측정 시점 lite_bench.sh 의 `tail -3 > \"$ELOG\"` 상한에 닿았다(도구·rev·줄 출처)",
       wg.get("NCCL_IB_GID_INDEX", {}).get("log_window") == f"tail-3(lite_bench.sh@{revs['c4'][:12]} L2)")
    ck("★상한 줄이 없는 도구(주석의 tail · ELOG 로 가지 않는 tail 은 세지 않는다)의 로그는 log_window 없음",
       "NCCL_IB_TC" in wg and "log_window" not in wg["NCCL_IB_TC"])
    (sdir / "lite_engine_c1-a.log").write_text("x\n" + nccl, encoding="utf-8")                     # 2줄 < 상한 3
    ck("★상한에 닿지 않은 로그(줄 수 < N)는 log_window 없음(전체 출력일 수 있다)",
       "log_window" not in {r["key"]: r for r in measurement_env_observed(g, evg)}.get("NCCL_IB_GID_INDEX", {"log_window": 1}))
    ck("★git 판본을 못 고르면(git 아님) log_window 없음 — 상한을 지어내지 않는다", not any("log_window" in r for r in env))
    for f in sorted((g / "output").rglob("*"), key=lambda x: -len(x.parts)):
        f.unlink() if f.is_file() else f.rmdir()
    (g / "output").rmdir()
    ev_t2 = dataclasses.replace(evg, certificate={"measured_utc": "2026-01-02T00:30:00Z", "vllm_version": "0.9.0"},
                                sweep={**sw_g, "generated_utc": "2026-01-02T00:30:00Z"}, lineage_seeds={})
    sn2 = {r["name"]: r for r in tool_snapshots(g, ev_t2, td / "draft-g2")}
    ck("main 체크아웃 시각의 측정 — run_bench v2(c2) · 다음 변경 c3 · 측정 뒤 커밋 v3 는 싣지 않는다 · 변경 없는 도구는 '변경 없음'",
       sn2["run_bench.sh"]["git_rev"] == revs["c2"] and sn2["run_bench.sh"]["next_rev"] == revs["c3"]
       and "첫 변경" in sn2["run_bench.sh"]["next_rev_basis"] and sn2["sweep_bench.sh"]["next_rev"] is None
       and "변경 없음" in sn2["sweep_bench.sh"]["next_rev_basis"] and sn2["run_bench.sh"]["git_rev_note"] is None
       and (td / "draft-g2" / sn2["run_bench.sh"]["snapshot_rel"]).read_text(encoding="utf-8") == "# run_bench v2\n")
    ev_gl = dataclasses.replace(ev_t2, sweep={**ev_t2.sweep, "index": {**sw_g["index"], "meta": {**sw_g["index"]["meta"],
                                                                                            "bench_tool": "guidellm"}}})
    gl_rows = tool_snapshots(g, ev_gl, td / "draft-g3")
    ck("★bench_tool 이 드라이버 --tool 과 맞으면 드라이버 후보로 싣는다(미검증이라 적는다 · '직접 불렀다' 추론 ✗)",
       any(r["name"] == "broad_search.sh" and r["role"] == "driver-candidate" and "미검증" in r["why"] for r in gl_rows)
       and not any("배제로 추론" in (r.get("driver_note") or "") for r in gl_rows))
    ck("★색인에 lite 기록이 없으면 lite_bench.sh 를 싣지 않는다", "lite_bench.sh" not in {r["name"] for r in tool_snapshots(
        g, dataclasses.replace(ev_t2, sweep={**ev_t2.sweep, "index": {"meta": sw_g["index"]["meta"]}}), td / "draft-g4")})
    ck("★측정 시각 이전 커밋이 없으면 []", tool_snapshots(g, dataclasses.replace(
        ev_t2, certificate={"measured_utc": "2025-01-01T00:00:00Z"}, sweep={**sw_g, "generated_utc": None}), td / "draft-g5") == [])
    ck("★git 이 아니면 []", tool_snapshots(repo, dataclasses.replace(ev, lineage_seeds={}), td / "draft-g6") == [])
    ck("★draft 없이 부르면 차단", _code(tool_snapshots, g, evg, None) == "HINT_DRAFT_REQUIRED")
    lh = g / ".git/logs/HEAD"
    lh_text = lh.read_bytes()
    lh.unlink()
    at_fb, why_fb = _head_at(g, _M_UTC)
    ck("★reflog 가 없으면 committer date 대체 — 방법이 그렇게 말한다(브랜치 미검증)", at_fb == revs["c2"] and "committer date" in why_fb
       and "미검증" in why_fb)
    lh.write_bytes(lh_text)
    # ★reflog 가 그 시각을 덮지만 커밋 오브젝트가 없는 항목(`git log -g` 는 이 줄을 조용히 건너뛰고 그 전 항목 c4 를 냈다) — 파일을 직접 읽어
    #   그 항목을 보고, 오브젝트 부재를 사유로 대체한다(2026-09-22 적대 검토).
    gone = "e" * 40
    lh.write_bytes(lh_text + (f"{revs['c4']} {gone} {core.SYNTHETIC_NAME} <{core.SYNTHETIC_EMAIL}> "
                              f"{int(_utc_dt('2026-01-02T03:00:00Z').timestamp())} +0000\tcheckout: moving from side to gone\n")
                  .encode("utf-8"))
    at_gone, why_gone = _head_at(g, _M_UTC)
    ck("★reflog 항목의 커밋이 없으면 그 전 항목으로 조용히 넘어가지 않는다 — committer date 대체 · 사유 = 오브젝트 부재",
       at_gone == revs["c2"] and "오브젝트 부재" in why_gone and "committer date" in why_gone)
    lh.write_bytes(lh_text)
    lab, psrc = _cert_vllm_producer(g, evg)
    ck("인증서 vllm_version producer = 측정 시점 sweep_bench 판본의 이미지 태그 규칙(검증 · 줄 번호 · 엔진 자기보고 아님)",
       lab == "image-tag-line" and "L6·L11" in psrc and "엔진 자기보고가 아니다" in psrc and "L9" in psrc)
    ck("★규칙 결과가 인증서와 다르면 미검증(덮어쓰기였을 수 있다)",
       _cert_vllm_producer(g, dataclasses.replace(evg, certificate={**evg.certificate, "vllm_version": "0.9.1"}))[0] == "unverified")
    # 2026-09-22(적대 검토): 배너 접두 규칙은 태그 규칙이 값을 못 냈을 때만 — 태그 규칙이 다른 값을 냈으면 덮어쓰기 신호다(배너 라벨 ✗).
    def with_meta(**kw) -> CellEvidence:
        return dataclasses.replace(evg, sweep={**sw_g, "index": {**sw_g["index"], "meta": {**sw_g["index"]["meta"], **kw}}})
    ck("태그 규칙이 값을 못 내면 배너 x.y.z 접두(그 판본 우선순위)",
       _cert_vllm_producer(g, with_meta(image_tag_declared="other:tag", vllm_build="0.9.0rc2.dev1+gx"))[0] == "engine-banner-prefix")
    ck("★태그 규칙이 다른 값(0.9.1)을 냈으면 배너 접두가 맞아도 미검증(덮어쓰기 신호)",
       _cert_vllm_producer(g, with_meta(image_tag_declared="easy-vllm:0.9.1rc2-cu130-source", vllm_build="0.9.0rc2.dev1+gx"))[0]
       == "unverified")
    ck("★배너 접두는 x.y.z 추출 일치만(0.9.01… 은 0.9.0 이 아니다)",
       _cert_vllm_producer(g, with_meta(image_tag_declared="other:tag", vllm_build="0.9.01rc1"))[0] == "unverified")
    ck("★저장소 없이(repo·ev.repo 둘 다 없음) vllm_observed = 차단(cwd 폴백 ✗)",
       _code(vllm_observed, dataclasses.replace(evg, repo=None)) == "HINT_EVIDENCE_REPO_REQUIRED")
    ev_old = dataclasses.replace(evg, certificate={"measured_utc": "2025-06-01T00:00:00Z", "vllm_version": "0.9.0"})
    ck("★측정 시점 판본이 없으면 미검증", _cert_vllm_producer(g, ev_old)[0] == "unverified")
    (g / REL_BENCH_TOOLS_DIR / "sweep_bench.sh").write_text("# 규칙 표지 없는 판본\n", encoding="utf-8")
    core.git(g, "add", "-A")
    core.git(g, "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "c5", env_extra={
        "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1", "GIT_AUTHOR_NAME": core.SYNTHETIC_NAME,
        "GIT_AUTHOR_EMAIL": core.SYNTHETIC_EMAIL, "GIT_COMMITTER_NAME": core.SYNTHETIC_NAME,
        "GIT_COMMITTER_EMAIL": core.SYNTHETIC_EMAIL, "GIT_AUTHOR_DATE": core.git_date("2026-01-04T00:00:00Z"),
        "GIT_COMMITTER_DATE": core.git_date("2026-01-04T00:00:00Z")})
    ev_new = dataclasses.replace(evg, certificate={"measured_utc": "2026-01-05T00:00:00Z", "vllm_version": "0.9.0"})
    ck("★판본에 규칙 표지가 없으면 미검증(tripwire)", _cert_vllm_producer(g, ev_new)[0] == "unverified"
       and "표지" in _cert_vllm_producer(g, ev_new)[1])


def _selftest_v7(ck, repo: Path, td: Path, ev: CellEvidence, fx: dict, dk) -> None:
    """2026-09-29(plan_26092908 · 계약 B) 신설 규칙 — 판정의 기계 표면(V1) · 관측 > 선언 호스트 사실(V3) · quant_composition(U9) ·
    노드 축 자동 cluster(V11③) · tail_sources(U6) · "미관측" 오판(§4.5) · 비발행 권한 바인딩(V11④). ★ = 음성대조."""
    from . import naming
    sw = dict(ev.sweep or {})
    vj0 = dict(sw.get("verdict") or {})
    # ── V1 판정의 기계 표면 ──
    m = measurement(ev)
    ck("measurement — 인증서 셀: 판정·출처 키별(sources) · 측정 시각", m.get("verdict") == "PASS"
       and m["sources"]["verdict"].startswith("certificate(") and m.get("measured_utc") == _M_UTC
       and m.get("rubric_authority") == "weak" and m.get("floor_tps") == "8.5")
    bad_sw = {**sw, "verdict": {**vj0, "measured_decode_tps": 99.0}}
    ck("★인증서와 같은 측정의 verdict.json 이 다른 값 = 죽는다(고르지 않는다)",
       _code(measurement, dataclasses.replace(ev, sweep=bad_sw)) == "HINT_MEASUREMENT_VERDICT_MISMATCH")
    ok_sw = {**sw, "verdict": {**vj0, "measured_decode_tps": 12.3}}
    ck("음성대조: 같은 값(12.3)이면 대조 통과", _code(measurement, dataclasses.replace(ev, sweep=ok_sw)) is None)
    refute = {"verdict": "REFUTE", "measured_decode_tps": 5.5, "measured_accept_len": 1.5, "measured_spec_on": True,
              "rubric": {"authority": "explore", "floor": 8.5, "ratio_M_over_primary": 0.647, "primary": 10.0, "tolerance": 0.15,
                         "source": "expected_achievable(roofline×MBU)"},
              "reasons": ["M(5.5) < 10.0×(1−0.15)=8.5 → 기대 이하", "see " + "/" + "home" + "/op-fixture/x.json"]}
    e_ref = dataclasses.replace(ev, certificate=None, certificate_path=None, sweep={**sw, "verdict": refute})
    mr = measurement(e_ref)
    ck("★REFUTE 셀(인증서 없음) — 판정·수치가 verdict.json 에서 채워진다(옛 5키 결함 · V1)",
       mr.get("verdict") == "REFUTE" and mr.get("decode_tps_conc1") == "5.5" and mr.get("floor_tps") == "8.5"
       and mr.get("ratio_M_over_primary") == "0.647" and mr.get("rubric_authority") == "explore"
       and mr.get("accept_len") == "1.5" and mr.get("spec_on") == "true" and mr.get("benchmark_mode") == "full"
       and "verdict.json" in mr["sources"]["verdict"] and mr.get("measured_utc") == _M_UTC
       and "sweep_index.generated_utc" in mr["sources"]["measured_utc"])
    ck("★REFUTE 사유는 싣되 운영자 절대경로 줄은 싣지 않는다(배포 평면)", mr.get("verdict_reasons") == [refute["reasons"][0]])
    ck("REFUTE = 인증서 비발행 판정(결손 아님)", (_certificate_not_due(repo, e_ref) or "").startswith("REFUTE"))
    pass_rub = lambda a: {"verdict": "PASS", "measured_decode_tps": 12.3, "rubric": {"authority": a, "floor": 8.5,
                                                                                       "ratio_M_over_primary": 1.23}}
    e_ex = dataclasses.replace(e_ref, sweep={**sw, "verdict": pass_rub("explicit")})
    ck("★explicit PASS 인데 인증서 없음 = 결손(발행되는 판정)", _certificate_not_due(repo, e_ex) is None)
    ck("weak PASS = 비발행 권한(결손 아님)",
       "weak" in (_certificate_not_due(repo, dataclasses.replace(e_ref, sweep={**sw, "verdict": pass_rub("weak")})) or ""))
    e_unk = dataclasses.replace(e_ref, sweep={**sw, "verdict": {"verdict": "PASS"}})
    ck("★권한 미관측 PASS = 결손(모름을 면제로 접지 않는다)", _certificate_not_due(repo, e_unk) is None)
    ck("★판정 어휘 밖 = 죽는다(tripwire)", _code(measurement, dataclasses.replace(
        e_ref, sweep={**sw, "verdict": {"verdict": "MAYBE"}})) == "HINT_MEASUREMENT_VERDICT_UNKNOWN")
    # 리포트 `## 판정` 표 대체(스윕 미바인딩 · 재생의 simlog 사본 등)
    rep2 = "docs/benchmark/bench_report_26010215_fixture-model-nvfp4_GB10_0.9.0.md"
    (repo / rep2).write_text("\n".join([
        "# 성능 보고서", "", "> 생성일 2026-01-02T15:00:00Z.", "", "## 판정 (표시만 — verdict_rule.py 결과)", "", "| 항목 | 값 |",
        "|---|---|", "| verdict | **REFUTE** |", "| 측정 decode t/s (동시성1) | 20.98 t/s |", "| 루브릭 권한 | explore |",
        "| 루브릭 primary | 29.8 t/s (expected_achievable(roofline×MBU)) |", "| floor (primary×(1−tol)) | 25.33 t/s |",
        "| ratio (M/primary) | 0.704 |", "| tolerance | 0.15 |", "", "## 다음", ""]), encoding="utf-8")
    mt = measurement(dataclasses.replace(ev, certificate=None, certificate_path=None, sweep=None, bench_report_path=rep2))
    ck("리포트 `## 판정` 표 — 굵게·단위 벗김 · primary 괄호 = primary_source · 측정 시각 = 생성일",
       mt.get("verdict") == "REFUTE" and mt.get("decode_tps_conc1") == "20.98" and mt.get("floor_tps") == "25.33"
       and mt.get("primary_tps") == "29.8" and mt.get("primary_source") == "expected_achievable(roofline×MBU)"
       and "`## 판정` 표" in mt["sources"]["verdict"] and mt.get("measured_utc") == "2026-01-02T15:00:00Z")
    (repo / rep2).unlink()
    mn = measurement(dataclasses.replace(ev, certificate=None, certificate_path=None, sweep=None, bench_report_path=None,
                                         report_kind=None))
    ck("★판정 원천이 하나도 없으면 verdict None + 미관측(지어내지 않는다)", mn.get("verdict") is None
       and "미관측" in mn["sources"]["verdict"] and mn.get("decode_tps_conc1") is None)
    # ── V3 관측 > 선언(드라이버) ──
    meta = dict((sw.get("index") or {}).get("meta") or {})

    def host(att, meta_extra: dict, man_extra: dict | None = None) -> dict:
        s2 = {**sw, "index": {**(sw.get("index") or {}), "meta": {**meta, **meta_extra}}}
        e2 = dataclasses.replace(ev, attestation=att, sweep=s2, manifest={**(ev.manifest or {}), **(man_extra or {})},
                                 build_identity={**ev.build_identity, "source": dict(ev.build_identity["source"])})
        _host_facts(e2)
        return e2.build_identity
    att = {"_path": "output/multi/benchlog/attestation_c1-a.json",
           "parity": {"driver": {"main": "580.178.04", "sub": "580.178.04", "verdict": "equal"}}}
    hb = host(att, {"driver_version": "580.173.02"})
    cf = hb.get("driver_conflict") or {}
    ck("★attestation 노드별 관측이 선언 옮김(스윕 meta)을 이긴다 + driver_conflict 에 둘 다(V3)",
       hb["driver"] == "580.178.04" and hb["source"]["driver"].startswith("attestation parity.driver")
       and cf.get("observed") == "580.178.04" and cf.get("declared") == "580.173.02"
       and _DECLARED_COPY in cf["sources"]["declared"] and hb.get("driver_by_node") == {"main": "580.178.04", "sub": "580.178.04"})
    ck("음성대조: 관측 = 선언이면 conflict 없음", "driver_conflict" not in host(att, {"driver_version": "580.178.04"}))
    hm = host(None, {"driver_version": "580.2", "driver_version_source": "measured(nvidia-smi)"}, {"driver_version": "580.1"})
    ck("스윕 meta 가 스스로 measured 를 말하면 관측 — 옮김 표지 없음 · manifest 선언과 다르면 conflict",
       hm["driver"] == "580.2" and _DECLARED_COPY not in hm["source"]["driver"]
       and (hm.get("driver_conflict") or {}).get("declared") == "580.1")
    hd = host({"_path": "a.json", "parity": {"driver": {"main": "1.0", "sub": "2.0", "verdict": "differ"}}}, {})
    ck("★노드별 상이는 한 값으로 접지 않는다(main 값 + 출처가 상이를 말한다 · by_node 둘 다)",
       hd["driver"] == "1.0" and "노드별 상이" in hd["source"]["driver"] and hd.get("driver_by_node") == {"main": "1.0", "sub": "2.0"})
    # ── F3 · F10(plan_26092908 §4.5): 노드별 관측 행의 출처 · 관측 > 선언은 같은 run 의 관측에만 ──
    ck("F3 드라이버(노드별 관측) 행 출처 = attestation 출처(출처 미기재 ✗)",
       str(hb["source"].get("driver_by_node") or "").startswith("attestation parity.driver"))
    hn = host({**att, "config": ev.cell}, {"driver_version": "580.173.02"})
    ck("F10 음성대조: 같은 config(같은 run 후보)의 attestation 은 관측 층 — 선언을 이긴다", hn["driver"] == "580.178.04"
       and "driver_conflict" in hn)
    e_nat = dataclasses.replace(ev, plane="native", attestation={**att, "config": "src-cell"}, sweep={
        **sw, "index": {**(sw.get("index") or {}), "meta": {**meta, "driver_version": "580.173.02"}}},
        build_identity={**ev.build_identity, "source": dict(ev.build_identity["source"])})
    _host_facts(e_nat)
    hn2 = e_nat.build_identity
    ck("★F10 native(원천 셀 attestation = 다른 run) — 값 = 선언 · 원천 관측은 범위를 붙여 출처에 · conflict · 노드별 관측 행 없음",
       hn2["driver"] == "580.173.02" and "원천 셀(src-cell) attestation 관측 580.178.04" in hn2["source"]["driver"]
       and "이 run 아님" in hn2["source"]["driver"] and "driver_conflict" not in hn2 and "driver_by_node" not in hn2)
    ho = host({**att, "config": "other-cfg"}, {})
    ck("★F10 다른 config attestation · 선언 없음 — 값은 옮기되 출처가 '다른 run' 범위를 말한다",
       ho["driver"] == "580.178.04" and "다른 run(other-cfg)" in ho["source"]["driver"] and "driver_by_node" not in ho)
    # ── U9 quant_composition ──
    ident = identity(ev)
    qc = ident.get("quant_composition") or []
    ck("quant_composition — MIXED 층 알고리즘별 개수(해석 ✗) · 출처", [(r["dtype"], r.get("count")) for r in qc] ==
       [("NVFP4", 2), ("FP8", 1)] and all(r.get("source") for r in qc) and str(td) not in json.dumps(qc, ensure_ascii=False))
    ds4 = {"observed": True, "host_label": "L", "config": {"quantization_config": {"quant_method": "fp8", "fmt": "e4m3",
                                                                                    "weight_block_size": [128, 128]},
                                                          "expert_dtype": "fp4", "torch_dtype": "bfloat16"}}
    comp, _src = _quant_composition(ds4)
    ck("quant_composition — DS4F 형(선언 fp8 + routed expert fp4 + 기본 bf16) · q 축은 선언 방식 그대로(fp8)",
       [r["dtype"].split(" ")[0] for r in comp] == ["fp8", "fp4", "bfloat16"] and comp[1]["scope"] == "routed experts"
       and _quant_raw(ds4).get("raw") == "fp8")
    ck("★체크포인트 미관측 = 구성 None(빈 구성으로 접지 않는다)", _quant_composition({"observed": False, "reason": "r"})[0] is None)
    # ── V11③ 노드 축 자동 cluster ──
    bl = repo / "output/multi/benchlog"
    ck("자동 cluster — 측정 TP(2) > gpus_per_node(1)", _auto_node(repo, "multi", "c1-z", cert={"tensor_parallel_size": "2"},
                                                             measured_key=None, report_rel=None)[0] == "cluster")
    ck("음성대조: TP ≤ gpus_per_node · 멀티 신호 없음 = 파생하지 않는다(배정으로 간다)",
       _auto_node(repo, "multi", "c1-z", cert={"tensor_parallel_size": "1"}, measured_key=None, report_rel=None)[0] is None)
    core.write_json(bl / "serve_proof_c1-z.json", {"kind": "multinode_serve_proof", "config": "c1-z"})
    try:
        ck("★모호(TP=1 단일 노드 형상인데 멀티 serve proof) = 기존 오류로 죽는다",
           _code(_auto_node, repo, "multi", "c1-z", cert={"tensor_parallel_size": "1"}, measured_key=None, report_rel=None)
           == "HINT_NODE_AXIS_UNDERIVABLE")
        ck("TP 미관측 + 멀티 serve proof = cluster", _auto_node(repo, "multi", "c1-z", cert=None, measured_key=None,
                                                               report_rel=None)[0] == "cluster")
    finally:
        (bl / "serve_proof_c1-z.json").unlink()
    ck("single 토폴로지는 자동 cluster 대상 아님", _auto_node(repo, "single", "c1-a", cert={"tensor_parallel_size": "2"},
                                                        measured_key=None, report_rel=None)[0] is None)
    ck("자동 cluster 가 배정(main)을 이긴다 · 인증서 measured_node 는 자동을 이긴다",
       _resolve_node(None, measured=None, topology="multi", assigned="main", declared=["main", "sub"], cert_node_source="x",
                     auto=("cluster", "why"))[0] == "cluster"
       and _code(_resolve_node, None, measured="main", topology="multi", assigned="main", declared=["main", "sub"],
                 cert_node_source="x", auto=("cluster", "why")) is None)
    # ── U6 tail_sources ──
    ts = tail_sources(repo, ev)
    y_rel, e_rel, c_rel = "output/multi/configs/c1-a.yaml", "output/multi/envs/.env.c1-a", "campaigns/c1/cells/c1-a/config.yaml"
    ck("tail_sources — 서빙 yaml(중첩 `a.b` · 문자열 JSON 펼침) · 셀 env · 러너 · 셀 config",
       ts.get(y_rel, {}).get("speculative-config.num_speculative_tokens") == "2" and ts[y_rel].get("enforce-eager") == "true"
       and ts.get(e_rel, {}).get("VLLM_PLE_MMAP") == "1" and ts.get(c_rel, {}).get("declared_axes.ple_mode") == "mmap"
       and ts.get(c_rel, {}).get("target_gpu.gpu_model") == "NVIDIA GB10"
       and ts.get("output/multi/configs/c1-a.sh", {}).get("--config") == "/app/configs/${CONFIG_FILE}.yaml")
    ck("재생(캠페인 밖)은 셀 config 를 싣지 않는다", c_rel not in tail_sources(repo, dataclasses.replace(ev, mode="publication-replay")))
    nf = naming_facts(repo, ev)
    tail = [{"token": t, "meaning": "뜻", "evidence": nf[ax]["evidence"]} for t, ax in (("mmap", "ple"), ("s2", "spec"), ("eager", "graph"))]
    ck("naming facts 구조화 근거(ple·spec·graph)가 tail_sources 와 정확히 맞는다(validate_tail 0건)",
       all(nf[ax].get("evidence") for ax in ("ple", "spec", "graph")) and naming.validate_tail(tail, ts) == [])
    liar = [{"token": "spec9", "meaning": "뜻", "evidence": {"file": y_rel, "key": "speculative-config.num_speculative_tokens",
                                                           "value": "9"}}]
    ck("★거짓 꼬리(값 불일치) = HINT_TAIL_UNGROUNDED", any(c == "HINT_TAIL_UNGROUNDED" for c, _ in naming.validate_tail(liar, ts)))
    # ── §4.5 "미관측" 오판 ──
    wm, wsrc = _wheel_meta(repo, dataclasses.replace(ev, attestation={"_path": "a.json", "nodes": {
        "main": {"vllm_dist_version": "0.9.0+gabc"}, "sub": {"vllm_dist_version": "0.9.0+gabc"}}}))
    ck("★wheel 메타 — attestation vllm_dist_version 이 있으면 관측(옛 '관측자 없음' 오판)", wm == "0.9.0+gabc" and "일치" in wsrc)
    ck("음성대조: 원천이 없으면 None + 미관측", _wheel_meta(repo, dataclasses.replace(ev, attestation=None))[0] is None)
    ck("★노드별 상이 wheel 은 고르지 않는다", _wheel_meta(repo, dataclasses.replace(ev, attestation={"_path": "a", "nodes": {
        "main": {"vllm_dist_version": "1"}, "sub": {"vllm_dist_version": "2"}}}))[0] is None)
    fz = repo / "docs/simlog/fx_native"
    fz.mkdir(parents=True, exist_ok=True)
    (fz / "pip-freeze-main.txt").write_text("torch==2.9\nvllm==0.9.0+native\n", encoding="utf-8")
    (fz / "pip-freeze-sub.txt").write_text("vllm==0.9.0+native\n", encoding="utf-8")
    wn, wnsrc = _wheel_meta(repo, dataclasses.replace(ev, plane="native", attestation={"_path": "a", "nodes": {
        "main": {"vllm_dist_version": "other"}}}, pip_freeze_paths={"main": "docs/simlog/fx_native/pip-freeze-main.txt",
                                                                     "sub": "docs/simlog/fx_native/pip-freeze-sub.txt"}))
    ck("native 셀 wheel 메타 = pip freeze `vllm==`(실행 평면 관측이 원천 이미지 attestation 을 이긴다)",
       wn == "0.9.0+native" and "pip freeze" in wnsrc)
    # ── V11④ 비발행 권한 바인딩 · verdict.json 필수 ──
    base = {"rubric_source": "verdict_json", "floor_tps": 1.0, "ratio_M_over_primary": 0.9, "primary_source": "x"}
    ck("weak PASS·REFUTE 는 bench_report 로 묶인다(비발행 권한 · 게이트 술어 한 벌)",
       no_cert_binding_source({"task_class": "full_benchmark", "benchmark": {**base, "rubric_authority": "weak",
                                                                            "verdict": "PASS"}}, repo) == "weak"
       and no_cert_binding_source({"task_class": "full_benchmark", "benchmark": {**base, "rubric_authority": "weak",
                                                                                "verdict": "REFUTE"}}, repo) == "weak")
    ck("★explicit PASS 는 인증서가 필요하다(bench_report 로 열지 않는다)",
       no_cert_binding_source({"task_class": "full_benchmark", "benchmark": {**base, "rubric_authority": "explicit",
                                                                            "verdict": "PASS"}}, repo) is None)
    ck("★출처 표시 없는 weak 는 열지 않는다(rubric_source 부재)",
       no_cert_binding_source({"task_class": "full_benchmark", "benchmark": {**{k: v for k, v in base.items() if k != "rubric_source"},
                                                                            "rubric_authority": "weak", "verdict": "PASS"}}, repo) is None)
    vjp = Path(repo / str(sw.get("dir")) / "verdict.json")
    held = vjp.read_bytes()
    vjp.unlink()
    try:
        ck("★full_benchmark 발행에 verdict.json 이 없으면 첫 쓰기 전에 명시 실패(권한 None → 인증서 요구 부활 방지)",
           _code(drive_publisher, repo, ev, topic="c1__vj_absent", generated_utc="2026-01-03T05:00:00Z",
                 draft_dir=repo / "hints/.drafts/vj") == "HINT_VERDICT_JSON_ABSENT"
           and not (repo / core.REL_EVIDENCE_DIR / "c1__vj_absent.json").exists() and not (repo / "hints/.drafts/vj").exists())
    finally:
        vjp.write_bytes(held)
    # 코드표 — 이번에 등재한 결손 코드(발행자 = artifacts · 계약 C-4)
    ck("결손 코드 등재 — SUB_RECIPE · APPLIED_SET · NATIVE_LAUNCH",
       {"HINT_MISSING_SUB_RECIPE", "HINT_MISSING_APPLIED_SET", "HINT_MISSING_NATIVE_LAUNCH"} <= set(MISSING_CODES))


def _selftest_fix2(ck, td: Path, ev: CellEvidence) -> None:
    """2026-09-29 FACT_FIX2(독립 사실 검증 target: fact 4건) — G1 같은 실행 전체 사본 · G2 serve_proof 시점 범위 · G3 lite 도구 원문 인자 ·
    G6 노드별 image_id · G7 lite 측정 창(시작 ≠ 끝) · G8 원장 관측 범위 · G9 lite accept_len · G10 같은 기동 판별. ★ = 음성대조.
    격리 저장소(td/fix2) · 라이브 저장소 비의존."""
    r = td / "fix2"
    bl = r / "output/multi/benchlog"
    bl.mkdir(parents=True, exist_ok=True)
    code_root = core.HINTLIB_DIR.parents[4]
    for rel in _OWNER_LINKS:                        # 소유 모듈(코드만) 심링크 — _fixture 와 같은 규칙
        (r / rel).parent.mkdir(parents=True, exist_ok=True)
        if not (r / rel).exists():
            os.symlink(code_root / rel, r / rel)
    # ── G1: 같은 실행의 전체 사본 ──
    sw_dir = bl / "sweep_cx"
    sw_dir.mkdir(exist_ok=True)
    tail = sw_dir / "lite_engine_cx.log"
    t_lines = ["(EngineCore pid=71) \x1b[36m(RayWorkerProc pid=9)\x1b[0m INFO 01-02 02:10:00 [x.py:1] tail start",
               "(EngineCore pid=71) (Worker_TP0 pid=9) INFO 01-02 02:20:00 [default_loader.py:430] Loading weights took 353.23 seconds"]
    tail.write_text("\n".join(t_lines) + "\n", encoding="utf-8")
    full_lines = ["INFO 01-02 02:00:00 [api_server.py:1] █ version 9.9.9rc1.dev0",
                  "(EngineCore pid=71) fixm:9:9 [0] NCCL INFO NCCL_NET_PLUGIN set by environment to spcx",
                  "(EngineCore pid=71) INFO 01-02 02:05:00 [x.py:1] boot",
                  "(EngineCore pid=71) (Worker_TP0 pid=9) INFO 01-02 02:20:00 [default_loader.py:430] Loading weights took 353.23 seconds"]
    full = bl / "engine_roce_cx.log"
    full.write_text("\n".join(full_lines) + "\n", encoding="utf-8")
    ev1 = dataclasses.replace(ev, repo=str(r), cell="cx", topology="multi", plane="docker", serve_proof=None, lineage_seeds={},
                              certificate={"measured_utc": "2026-01-02T03:00:00Z"}, report_kind=None, bench_report_path=None,
                              sweep={"dir": "output/multi/benchlog/sweep_cx", "binding": "output", "generated_utc": "2026-01-02T03:00:00Z",
                                     "index": {"lite": {"gen_tps": 1.0}}, "levels": []})
    logs1, why1, cp1 = _measured_engine_logs_ex(r, ev1)
    env1 = measurement_env_observed(r, ev1)
    ban1 = _engine_banner(r, ev1)
    ck("G1 같은 실행 전체 사본(pid · 로드 줄 · 첫 시각) = 원천(앞에 둔다) · 판별 규칙이 근거에 남는다",
       logs1[:1] == [full] and full in cp1 and "EngineCore pid ['71'] 일치" in cp1[full] and SAME_RUN_RULE in cp1[full]
       and "engine_roce_cx.log" in why1)
    ck("G1 env 관측 · 엔진 자기보고가 사본에서 나온다(창 표지 ✗ · 배너 출처에 판별)",
       any(x["key"] == "NCCL_NET_PLUGIN" and x["source"].startswith("output/multi/benchlog/engine_roce_cx.log") and "log_window" not in x
           for x in env1) and ban1.get("value") == "9.9.9rc1.dev0" and "같은 실행의 전체 사본" in ban1.get("source", ""))
    full.write_text("\n".join(full_lines[:-1] + [full_lines[-1].replace("353.23", "300.00")]) + "\n", encoding="utf-8")
    ck("★G1 음성대조: 로드 줄(시각·초)이 다르면 같은 실행이 아니다(pid 만으로 묶지 않는다)", _measured_engine_logs_ex(r, ev1)[2] == {})
    full.write_text("\n".join(x.replace("pid=71", "pid=72") for x in full_lines) + "\n", encoding="utf-8")
    ck("★G1 음성대조: EngineCore pid 가 다르면 사본 ✗", _measured_engine_logs_ex(r, ev1)[2] == {})
    full.write_text("\n".join([full_lines[0].replace("02:00:00", "02:15:00"), full_lines[1],
                               full_lines[2].replace("02:05:00", "02:15:00"), full_lines[3]]) + "\n", encoding="utf-8")
    ck("★G1 음성대조: 사본이 꼬리보다 늦게 시작하면(더 긴 머리가 아니다) 사본 ✗", _measured_engine_logs_ex(r, ev1)[2] == {})
    (bl / "watchdog_cx.log").write_text("\n".join(full_lines) + "\n", encoding="utf-8")
    ck("★G1 음성대조: 워치독 로그는 후보가 아니다", _measured_engine_logs_ex(r, ev1)[2] == {})
    full.unlink()
    (bl / "watchdog_cx.log").unlink()
    # ── G6: 노드별 image_id ──
    att2 = {"_path": "a.json", "nodes": {"main": {"image_id": "sha256:" + "a" * 64}, "sub": {"image_id": "sha256:" + "b" * 64}}}
    n6 = _node_image_id_note(att2, "sha256:" + "a" * 64)
    ck("G6 노드별: main 같다 · sub 다르다(id) · 노드별 로컬 빌드 정상(동일 ABI ≠ 동일 digest)",
       "main 같다" in n6 and "sub 다르다(sha256:bbbbbbbbbbbb" in n6 and "동일 ABI" in n6)
    n6b = _node_image_id_note({"_path": "a.json", "nodes": {"main": {"image_id": "x"}, "sub": {"image_id": "x"}}}, "x")
    ck("★G6 음성대조: 노드 id 가 같으면 '다르다' · 정상 주석 없음 · image_id 없으면 빈 문자열",
       "다르다" not in n6b and "동일 ABI" not in n6b and "main 같다 · sub 같다" in n6b and _node_image_id_note({}, "x") == "")
    # ── G7 · G9: lite-only 셀 측정 창 · accept_len ──
    t0, t1 = "2026-01-02T03:00:00Z", "2026-01-02T03:01:05Z"
    (r / "docs/benchmark").mkdir(parents=True, exist_ok=True)
    (r / "docs/benchmark/bench_report_fx.md").write_text(f"# lite\n생성일 {t0}\nmode: lite\n", encoding="utf-8")
    core.write_json(bl / "lite_raw_cl.json", {"config_name": "cl", "measured_utc": t0, "topology": "multi", "backend": "openai-chat",
                                              "bench_warm_json": "/x/lite_warm_cl.json", "bench_cold_json": "/x/lite_cold_cl.json"})
    leg = {"backend": "openai-chat", "num_prompts": 3, "completed": 3, "total_input_tokens": 1695, "total_output_tokens": 384,
           "max_concurrency": 1, "request_rate": "inf"}
    core.write_json(bl / "lite_cold_cl.json", {**leg, "date": "20260102-030030", "spec_decode_acceptance_length": 2.0})
    core.write_json(bl / "lite_warm_cl.json", {**leg, "date": "20260102-030105", "spec_decode_acceptance_length": 1.81})
    ev7 = dataclasses.replace(ev1, cell="cl", sweep=None, certificate=None, certificate_path=None, report_kind="lite",
                              bench_report_path="docs/benchmark/bench_report_fx.md")
    lw = lite_window(r, ev7)
    ck("G7 lite 측정 창: 시작 = lite_raw measured_utc(부하 직전 · '시작') · 끝 = 마지막 레그 JSON date",
       lw and lw["start_utc"] == t0 and lw["end_utc"] == t1 and "시작" in lw["start_source"] and "lite_warm_cl.json" in lw["end_source"]
       and _measure_window(r, ev7)[:2] == (t0, t1))
    ck("★G7 음성대조: 조인 불성립(생성일 ≠ measured_utc) · 스윕 셀 = lite_window 없음",
       lite_window(r, dataclasses.replace(ev7, bench_report_path=None)) is None
       and lite_window(r, dataclasses.replace(ev7, sweep=ev1.sweep)) is None)
    m7 = measurement(dataclasses.replace(ev7, repo=str(r)))
    ck("G9 lite-only accept_len = warm 레그 JSON 원문(판정 입력 아님 · cold 병기)",
       m7.get("accept_len") == "1.81" and "lite_warm_cl.json" in m7["sources"]["accept_len"] and "판정 입력 아님" in m7["sources"]["accept_len"]
       and "cold 2.0" in m7["sources"]["accept_len"])
    core.write_json(bl / "lite_warm_cl.json", {**leg, "date": "20260102-030105"})
    core.write_json(bl / "lite_cold_cl.json", {**leg, "date": "20260102-030030"})
    ck("★G9 음성대조: 레그 JSON 에 수용 길이가 없으면 미기재(지어내지 않는다)",
       measurement(dataclasses.replace(ev7, repo=str(r))).get("accept_len") is None)
    # ── G2: serve_proof 시점 범위(attestation 과 같은 분류) ──
    sp = bl / "serve_proof_cl.json"
    core.write_json(sp, {"config": "cl", "health_http": 200})
    ev2 = dataclasses.replace(ev7, serve_proof={"config": "cl"}, sources={**ev7.sources, "serve_proof": "output/multi/benchlog/serve_proof_cl.json"})
    os.utime(sp, (1767326400, 1767326400))           # 2026-01-02T04:00:00Z > 측정 끝
    sc2 = serve_proof_scope(r, ev2)
    ck("G2 serve_proof 작성 > 측정 끝 = 측정 뒤 재기동의 스모크 산출물(자격 관측이 아니다)",
       sc2 and sc2["timing"] == "after-measurement" and "측정 뒤 재기동의 스모크 산출물" in sc2["timing_note"])
    os.utime(sp, (1767322800, 1767322800))           # 2026-01-02T03:00:00Z … 시작과 같은 초 → 창 안
    os.utime(sp, (1767322700, 1767322700))           # 2026-01-02T02:58:20Z < 시작
    sc2b = serve_proof_scope(r, ev2)
    ck("★G2 음성대조: 측정 전 작성 = 측정 뒤 ✗ · serve_proof 없으면 None",
       sc2b and sc2b["timing"] == "before-measurement" and "측정 뒤" not in sc2b["timing_note"]
       and serve_proof_scope(r, dataclasses.replace(ev2, serve_proof=None)) is None)
    # ── G8: 원장 관측 범위 ──
    for node, rows in (("main", [("2026-01-01T00:00:00Z", "budget_declare", "smoke-cl"), ("2026-01-02T02:50:00Z", "budget_declare", "smoke-cl"),
                                 ("2026-01-02T03:30:00Z", "budget_clear", None)]),
                       ("sub", [("2025-12-01T00:00:00Z", "budget_declare", "smoke-old")])):
        d = r / REL_EVENTS_ROOT / node / "events"
        d.mkdir(parents=True, exist_ok=True)
        (d / "2026-01.jsonl").write_text("".join(json.dumps({"ts": ts, "kind": k, **({"label": lb} if lb else {}),
                                                             **({"existed": True} if k == "budget_clear" else {})}) + "\n"
                                                 for ts, k, lb in rows), encoding="utf-8")
    ev8 = dataclasses.replace(ev2, node="cluster", declaration={"nodes": [{"node_id": "main", "role": "main"},
                                                                          {"node_id": "sub", "role": "sub"}]})
    sp8 = {x["node"]: x for x in event_ledger_spans(r, ev8)}
    ck("G8 원장 관측 범위: 노드별 첫·마지막 행 · 측정 끝보다 앞에서 끝난 노드 = covers_measurement False",
       sp8["main"]["last_utc"] == "2026-01-02T03:30:00Z" and sp8["main"]["covers_measurement"] is True
       and sp8["sub"]["last_utc"] == "2025-12-01T00:00:00Z" and sp8["sub"]["covers_measurement"] is False)
    ev8b = dataclasses.replace(ev8, declaration={"nodes": [{"node_id": "main"}, {"node_id": "ghost"}]})
    ck("★G8 음성대조: 선언 노드인데 원장 없음 = 범위 None(없음으로 단언 ✗)",
       {x["node"]: x for x in event_ledger_spans(r, ev8b)}["ghost"]["last_utc"] is None)
    # ── G10: 같은 기동 판별 ──
    att_rel = "output/multi/benchlog/attestation_cl.json"
    core.write_json(r / att_rel, {"config": "cl", "phase": "serve"})
    ev10 = dataclasses.replace(ev8, attestation={"_path": att_rel, "config": "cl", "phase": "serve"})
    (bl / "lite_engine_cl.log").write_text("(APIServer pid=5) INFO 01-02 03:00:10 [x.py:1] POST\n", encoding="utf-8")
    sl = r / "docs/simlog/fx_run"
    sl.mkdir(parents=True, exist_ok=True)
    good = ["[mn] main: budget_honored ✓ {\"ts\":\"2026-01-02T02:50:00Z\",\"kind\":\"budget_honored\"}", "[mn] SMOKE PASS — ok",
            f"[mn] 노드 정합 attestation v2(serve) → {att_rel}", "[mn] --keep-up: 워치독 유지"]
    (sl / "fx_serve.log").write_text("\n".join(good) + "\n", encoding="utf-8")
    win = {"start_utc": t0, "end_utc": t1}
    sb = attestation_same_boot(r, ev10, "2026-01-02T02:55:00Z", win)
    ck("G10 스모크 로그 순서 · 원장 단일 선언(재선언 ✗) · APIServer pid 하나 = 같은 기동의 스모크 직후 관측(근거 파일 · 줄)",
       sb and "L2 SMOKE PASS → L3 attestation 작성 → L4 --keep-up" in sb["basis"] and "APIServer pid 5 하나" in sb["basis"]
       and sb["files"][0] == "docs/simlog/fx_run/fx_serve.log")
    os.utime(r / att_rel, (1767322500, 1767322500))   # 02:55:00Z < 측정 시작 → before-measurement
    sc10 = attestation_scope(r, ev10)
    ck("G10 attestation 범위 scope 가 판별을 말한다('미검증' ✗) · 호스트 사실 출처에도 표지",
       sc10 and SAME_BOOT_MARK in sc10["scope"] and "미검증" not in sc10["scope"]
       and SAME_BOOT_MARK in (_attestation_timing_note(dataclasses.replace(ev10, repo=str(r))) or ""))
    (sl / "fx_serve.log").write_text("\n".join([good[0], good[1], good[3], good[2]]) + "\n", encoding="utf-8")
    ck("★G10 음성대조: 순서가 어긋나면(keep-up 이 attestation 앞) 판별 ✗ → scope '미검증' 유지",
       attestation_same_boot(r, ev10, "2026-01-02T02:55:00Z", win) is None and "미검증" in attestation_scope(r, ev10)["scope"])
    (sl / "fx_serve.log").write_text("\n".join(good) + "\n", encoding="utf-8")
    (bl / "lite_engine_cl.log").write_text("(APIServer pid=5) a\n(APIServer pid=6) b\n", encoding="utf-8")
    ck("★G10 음성대조: 측정 로그에 APIServer pid 둘(서빙 프로세스 교체) = 판별 ✗",
       attestation_same_boot(r, ev10, "2026-01-02T02:55:00Z", win) is None)
    (bl / "lite_engine_cl.log").write_text("(APIServer pid=5) a\n", encoding="utf-8")
    # ── G10 교정(2026-09-29): 워치독 honored 는 선언 1초 뒤다 — 원장의 같은 라벨 budget_honored 로 대조한다 ──
    led = r / REL_EVENTS_ROOT / "main/events/2026-01.jsonl"
    led_bytes = led.read_bytes()
    off = [good[0].replace("02:50:00Z", "02:50:01Z")] + good[1:]
    (sl / "fx_serve.log").write_text("\n".join(off) + "\n", encoding="utf-8")
    ck("★G10 음성대조: 스모크 로그 honored ts 가 선언 · 원장 honored 어디에도 없으면 판별 ✗",
       attestation_same_boot(r, ev10, "2026-01-02T02:55:00Z", win) is None)
    with open(led, "a", encoding="utf-8") as fh:
        fh.write(json.dumps({"ts": "2026-01-02T02:50:01Z", "kind": "budget_honored", "label": "smoke-cl"}) + "\n")
    sb1 = attestation_same_boot(r, ev10, "2026-01-02T02:55:00Z", win)
    ck("G10 스모크 로그 honored ts = 원장의 같은 선언 budget_honored(1초 뒤)면 강한 판별",
       sb1 and sb1["strength"] == "strong" and "02:50:01Z = 그 선언의 원장 budget_honored" in sb1["basis"])
    # ── F10: 미러 밖 노드(sub · 원장 2025-12-01 끝)의 스모크 로그 echo = 보조 관측 ──
    sub_echo = ["[mn]   sub: 선언 발행: /x/docs/logs/sub/serve_budget.env",
                "[mn]   sub: budget_honored ✓ {\"ts\":\"2026-01-02T02:50:04Z\",\"kind\":\"budget_honored\",\"label\":\"smoke-cl\"}",
                "[mn]   sub: budget_honored ✓ {\"ts\":\"2026-01-02T02:50:05Z\",\"kind\":\"budget_honored\",\"label\":\"smoke-other\"}",
                "[mn]   sub: budget_honored ✓ {\"ts\":\"2026-01-02T02:50:06Z\",\"kind\":\"budget_clear\"}",
                "[mn]   sub: mem_kill {\"ts\":\"2026-01-02T02:50:07Z\",\"kind\":\"mem_kill\"}"]
    (sl / "fx_serve.log").write_text("\n".join([off[0]] + sub_echo + off[1:]) + "\n", encoding="utf-8")
    sp10 = {x["node"]: x for x in event_ledger_spans(r, ev10)}
    se = sp10["sub"].get("smoke_echo") or {}
    ck("F10 미러 밖 노드: 같은 기동 스모크 로그의 그 노드 원장 JSON echo(선언 · honored · 이 셀 라벨)만 보조 관측 · 출처 L 번호",
       sp10["sub"]["covers_measurement"] is False and se.get("log") == "docs/simlog/fx_run/fx_serve.log"
       and [(x["ts"], x["kind"], x["source"]) for x in se.get("rows") or []]
       == [("2026-01-02T02:50:04Z", "budget_honored", "docs/simlog/fx_run/fx_serve.log:3")])
    ck("★F10 음성대조: 측정 끝을 덮는 노드(main)는 보조 관측 ✗ · 문장 줄 · 다른 라벨 · kind 불일치 · 목록 밖 kind 는 싣지 않는다",
       "smoke_echo" not in sp10["main"] and len(se.get("rows") or []) == 1)
    (sl / "fx_serve.log").write_text("\n".join([off[0]] + sub_echo + [off[1], off[3], off[2]]) + "\n", encoding="utf-8")
    ck("★F10 음성대조: 같은 기동 판별 불성립(순서 어긋남)이면 스모크 로그 echo 를 싣지 않는다",
       "smoke_echo" not in {x["node"]: x for x in event_ledger_spans(r, ev10)}["sub"])
    (sl / "fx_serve.log").write_text("\n".join(off) + "\n", encoding="utf-8")
    # ── 약한 판별: 스모크 로그 미보존 · 원장 순서만 ──
    (sl / "fx_serve.log").unlink()
    (bl / "lite_engine_cl.log").write_text("(APIServer pid=5) INFO 01-02 02:52:00 [entry.py:1] Starting vLLM server on http://x\n",
                                           encoding="utf-8")
    sbw = attestation_same_boot(r, ev10, "2026-01-02T02:55:00Z", win)
    scw = attestation_scope(r, ev10)
    ck("G10 약한 판별: 스모크 로그 없음 · 선언 ≤ serve_proof · attestation 작성 < 측정 시작 · API 서버 기동 ∈ [선언, 작성] = 원장 순서 판별",
       sbw and sbw["strength"] == "weak" and "원장 순서로 판별(스모크 로그 미보존" in sbw["basis"]
       and "serve_proof 작성 2026-01-02T02:58:20Z" in sbw["basis"] and "API 서버 기동 2026-01-02T02:52:00Z" in sbw["basis"]
       and WEAK_SAME_BOOT_MARK in scw["scope"] and SAME_BOOT_MARK not in scw["scope"] and "미검증" not in scw["scope"]
       and WEAK_SAME_BOOT_MARK in (_attestation_timing_note(dataclasses.replace(ev10, repo=str(r))) or ""))
    (bl / "lite_engine_cl.log").write_text("(APIServer pid=5) INFO 01-02 02:57:00 [entry.py:1] Starting vLLM server on http://x\n",
                                           encoding="utf-8")
    ck("★G10 약한 판별 음성대조: API 서버 기동이 attestation 작성 뒤(모순) = 판별 ✗ · scope '미검증'",
       attestation_same_boot(r, ev10, "2026-01-02T02:55:00Z", win) is None and "미검증" in attestation_scope(r, ev10)["scope"])
    (bl / "lite_engine_cl.log").write_text("(APIServer pid=5) a\n", encoding="utf-8")
    os.utime(sp, (1767322860, 1767322860))           # serve_proof 03:01:00Z ≥ 측정 시작
    ck("★G10 약한 판별 음성대조: serve_proof 작성이 측정 시작 이후 = 판별 ✗",
       attestation_same_boot(r, ev10, "2026-01-02T02:55:00Z", win) is None)
    os.utime(sp, (1767322700, 1767322700))
    ck("G10 약한 판별: API 서버 기동 미관측은 모순이 아니다(근거에 미관측을 적는다)",
       "API 서버 기동 시각 미관측" in (attestation_same_boot(r, ev10, "2026-01-02T02:55:00Z", win) or {}).get("basis", ""))
    (sl / "fx_serve.log").write_text("\n".join([good[0], good[1], good[3], good[2]]) + "\n", encoding="utf-8")
    ck("★G10 음성대조: 스모크 로그가 있는데 순서가 어긋나면 약한 판별로 내려가지 않는다(모순)",
       attestation_same_boot(r, ev10, "2026-01-02T02:55:00Z", win) is None)
    (sl / "fx_serve.log").write_text("\n".join(good) + "\n", encoding="utf-8")
    led.write_bytes(led_bytes)
    with open(r / REL_EVENTS_ROOT / "main/events/2026-01.jsonl", "a", encoding="utf-8") as fh:
        fh.write(json.dumps({"ts": "2026-01-02T03:00:30Z", "kind": "budget_declare", "label": "smoke-other"}) + "\n")
    ck("★G10 음성대조: 측정 끝 전 재선언(다른 기동) = 판별 ✗", attestation_same_boot(r, ev10, "2026-01-02T02:55:00Z", win) is None)
    # ── G3: 측정 시점 lite_bench.sh 의 리터럴 인자 ──
    g = td / "fix2g"
    tdir = g / REL_BENCH_TOOLS_DIR
    tdir.mkdir(parents=True, exist_ok=True)
    (tdir / "lite_bench.sh").write_text("\n".join((
        "#!/bin/bash", "_bench() {", '  docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \\',
        "      --backend $BACKEND --base-url $BASE_URL --model '$MODEL_NAME' --trust-remote-code \\",
        "      --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \\",
        "      --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \\",
        "      --save-result --result-dir /tmp\"", "}", 'MEASURED_UTC="$(date -u +%FT%TZ)"', "")), encoding="utf-8")
    genv = {"GIT_AUTHOR_NAME": core.SYNTHETIC_NAME, "GIT_AUTHOR_EMAIL": core.SYNTHETIC_EMAIL,
            "GIT_COMMITTER_NAME": core.SYNTHETIC_NAME, "GIT_COMMITTER_EMAIL": core.SYNTHETIC_EMAIL,
            "GIT_AUTHOR_DATE": core.git_date("2026-01-01T00:00:00Z"), "GIT_COMMITTER_DATE": core.git_date("2026-01-01T00:00:00Z")}
    core.git(g, "init", "-q", env_extra=genv)
    core.git(g, "add", "-A", env_extra=genv)
    core.git(g, "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "c", env_extra=genv)
    (g / "docs/benchmark").mkdir(parents=True, exist_ok=True)
    (g / "docs/benchmark/bench_report_fx.md").write_text(f"# lite\n생성일 {t0}\n", encoding="utf-8")
    ta = lite_tool_args(g, dataclasses.replace(ev7, repo=str(g)))
    fl = dict((ta or {}).get("flags") or [])
    ck("G3 lite 도구 원문 인자(리터럴만): --random-input-len 512 · --random-range-ratio 0 · --ignore-eos · 출처 도구@rev 줄 · MEASURED_UTC 줄",
       ta and fl.get("--random-input-len") == "512" and fl.get("--random-range-ratio") == "0" and "--ignore-eos" in fl
       and fl.get("--max-concurrency") == "1" and ta["source"].startswith("lite_bench.sh@") and ta["measured_line"] == "L9")
    ck("★G3 음성대조: `$변수` · 따옴표 인자(--backend · --model · --num-prompts · --num-warmups)는 싣지 않는다 · git 아니면 None",
       not ({"--backend", "--model", "--num-prompts", "--num-warmups"} & set(fl)) and lite_tool_args(r, ev7) is None)


def selftest() -> list[str]:
    """실패 메시지 목록(빈 목록 = 통과). 격리 저장소 · 라이브 태그/브랜치/캠페인 비의존 · ★ = 음성대조."""
    from . import naming
    bad: list[str] = []

    def ck(name: str, cond) -> None:
        if not cond:
            bad.append(f"evidence: {name}")

    from . import lineage
    prev = os.environ.pop("QUANT_MODEL_PATH", None)     # check_smoke_model 은 env 를 먼저 본다 — 픽스처 격리
    # ★ ACTIVE 미독의 증명은 권한 0 만으로는 공허하다(root 는 모드 0 파일도 읽는다) — 이 프로세스의 모든 open 을 덫에 걸어
    #   `campaigns/ACTIVE` 가 한 번이라도 열리면 기록한다(pathlib 은 io.open 을 부른다). 서브프로세스(campaign_init·발행기)는
    #   권한 0 이 잡는다.
    import builtins
    import io
    active_hits: list[str] = []
    real_open = builtins.open

    def trap_open(file, *a, **kw):
        try:
            name = os.fspath(file)
        except TypeError:
            name = ""
        mode = str(a[0] if a else kw.get("mode", "r"))
        if isinstance(name, (str, bytes)) and str(name).replace("\\", "/").endswith("campaigns/ACTIVE") \
                and not any(c in mode for c in "wax"):     # 픽스처가 미끼를 **쓰는** 것은 읽기가 아니다
            active_hits.append(str(name))
        return real_open(file, *a, **kw)

    builtins.open = io.open = trap_open
    try:
        with tempfile.TemporaryDirectory() as tds:
            td = Path(tds)
            repo, fx = _fixture(td)
            dk = _fake_docker(fx["docker"])
            # ── from_campaign · ★ACTIVE 미독 ──
            ev = from_campaign(repo, "c1", "c1-a", docker=dk)
            ck("명시 id 만 읽는다(★ACTIVE=decoy·권한 0 — 읽었다면 죽었다)", ev.campaign_id == "c1" and ev.cell == "c1-a")
            ck("노드 축 = 인증서 measured_node(cluster)", ev.node == "cluster" and ev.topology == "multi")
            ck("셀 필터 — 다른 셀 포인터 제외", all(p.get("cell_id") == "c1-a" for p in ev.pointers) and len(ev.pointers) == 4)
            ck("인증서 파싱(게이트 파서)", (ev.certificate or {}).get("measured_utc") == _M_UTC)
            ck("스윕 조인(generated_utc = measured_utc)", (ev.sweep or {}).get("binding") == "output")
            ck("빌드 정체 = history build-arg · digest 관측", ev.build_identity.get("vllm_ref") == "v0.9.0"
               and ev.build_identity["source"].get("vllm_ref") == "docker history build-arg VLLM_REF"
               and ev.build_identity.get("torch") == "2.9.0a0" and ev.build_identity.get("track") == "source-build")
            ck("★베이스 이미지 build-arg 는 싣지 않는다(쓰인 Dockerfile 의 ARG 만)",
               set(ev.build_identity.get("history_build_args") or {}) == {"VLLM_REPO", "VLLM_REF", "SM12X_PORT"})
            ck("resolve 바인딩 → vllm_sha", ev.build_identity.get("vllm_sha") == _SHA_A)
            ck("attestation 셀 이름 바인딩", (ev.attestation or {}).get("config") == "c1-a" and
               "HINT_MISSING_SLAVE_ATTESTATION" not in ev.missing)
            ck("phases 셀 필터(artifacts 소비 모양)", ev.phases.get("serve", {}).get("ended_utc") == "2026-01-02T01:30:00Z")
            ck("docker 는 읽기만(inspect·history)", all(c[1] in ("image", "history") for c in fx["docker"]["calls"]))
            ck("결손 코드는 전부 등재", all(c in MISSING_CODES for c in ev.missing))
            ck("lineage 시드 예약 키", ev.lineage_seeds.get("cell_key") == "c1-a" and
               ev.lineage_seeds.get("identity", {}).get("tp") == 2 and ev.lineage_seeds.get("journey"))
            ck("모든 dict 필드에 출처", all(k in ev.sources for k in ("sweep", "manifest", "attestation", "resolve", "node")))
            q = qualification(ev)
            ck("자격 = post_health + bench(관측)", q.get("method") == "post_health+bench" and len(q["sources"]) == 2)
            facts = naming_facts(repo, ev)
            dn = naming.derive_name(facts, naming.load_vocab(repo))
            # 2026-09-29(plan_26092908 §4.1): v7 결정론부 = q·len·kv 만(ple·spec·graph 는 Agent 꼬리 — naming.derive_name).
            ck(f"이름 전량 파생 ({dn.tag})", dn.tag == "hint/0.9.0/fixture-model-nvfp4/gb10-1g2n-cluster-native/"
                                                    "qnvfp4-len4096-kvauto")
            ck("출처에 호스트 경로 없음(PII)", str(td) not in json.dumps(facts, ensure_ascii=False))
            ck("D-h naming facts 가 vllm_repo 를 키로 싣는다(업스트림 판정 입력)", "vllm_repo" in facts["vllm"]
               and naming.vllm_repo_kind(facts["vllm"]["vllm_repo"]) == "upstream")
            hist0 = fx["docker"]["history"][_DIGEST]
            fx["docker"]["history"][_DIGEST] = hist0.replace("VLLM_REPO=https://github.com/vllm-project/vllm.git",
                                                             "VLLM_REPO=https://example.invalid/fork/vllm.git")
            ev_fork = from_campaign(repo, "c1", "c1-a", docker=dk)
            ck("★D-h 포크 VLLM_REPO 면 resolved.json(업스트림 해소)의 to_sha 를 vllm_sha 로 쓰지 않는다",
               ev_fork.build_identity.get("vllm_sha") is None and "포크" in ev_fork.build_identity["source"]["vllm_sha"])
            ck("★D-h 포크 + 릴리스 모양 ref + 원장 SHA 없음 = 이름 차단(업스트림 릴리스 이름 ✗)",
               _code(naming.derive_name, naming_facts(repo, ev_fork), naming.load_vocab(repo)) == "HINT_AXIS_UNDERIVABLE")
            fx["docker"]["history"][_DIGEST] = hist0
            envp = repo / "output/multi/envs/.env.c1-a"
            env_text = envp.read_text(encoding="utf-8")
            envp.write_text(env_text.replace("VLLM_REF=v0.9.0", "VLLM_REF=v0.9.1"), encoding="utf-8")
            ck("★셀 env 와 이미지 history 가 다르면 이름을 짓지 않는다",
               _code(naming_facts, repo, from_campaign(repo, "c1", "c1-a", docker=dk)) == "HINT_VLLM_INPUT_CONFLICT")
            envp.write_text(env_text, encoding="utf-8")
            y = repo / "output/multi/configs/c1-a.yaml"
            y_text = y.read_text(encoding="utf-8")
            y.write_text(y_text.replace("max-model-len: 4096", "max-model-len: 8192"), encoding="utf-8")
            ck("★측정 뒤 바뀐 트리플렛으로 이름을 짓지 않는다", _code(naming_facts, repo, ev) == "HINT_TRIPLET_DRIFT")
            ck("★스윕 meta 가 없어도 인증서 기록과 대조한다", _code(naming_facts, repo, dataclasses.replace(ev, sweep=None))
               == "HINT_TRIPLET_DRIFT")
            # 이름 축(kv dtype · spec · TP · 모델 경로)도 측정 기록과 대조한다(2026-09-22 적대 검토 — 옛 판본은 안 봤다)
            for new_text, what in ((y_text.replace("kv-cache-dtype: auto", "kv-cache-dtype: fp8"), "kv dtype"),
                                   (y_text.replace("speculative-config:", "# speculative-config:"), "spec 줄 삭제 · 인증서 spec_on"),
                                   (y_text.replace("tensor-parallel-size: 2", "tensor-parallel-size: 1"), "TP"),
                                   (y_text.replace("Fixture-Model-NVFP4", "Fixture-Model-FP8"), "모델 경로")):
                y.write_text(new_text, encoding="utf-8")
                ck(f"★측정 기록과 다른 트리플렛({what})으로 이름을 짓지 않는다", _code(naming_facts, repo, ev) == "HINT_TRIPLET_DRIFT")
            y.write_text(y_text.replace("kv-cache-dtype: auto\n", ""), encoding="utf-8")
            ck("kv dtype 줄이 없어도 실효값(auto)이 기록과 같으면 드리프트가 아니다", _code(naming_facts, repo, ev) is None)
            y.write_text(y_text, encoding="utf-8")
            cfgp = td / "models/Org/Fixture-Model-NVFP4/config.json"
            cfg_text = cfgp.read_text(encoding="utf-8")
            cfg_doc = json.loads(cfg_text)
            cfg_doc.pop("text_config")
            cfgp.write_text(json.dumps(cfg_doc), encoding="utf-8")
            ck("★PLE 키 없는 모델에 VLLM_PLE_MMAP=1 = 모순(조용히 plenone 으로 접지 않는다)",
               _code(naming_facts, repo, ev) == "HINT_PLE_CONFLICT")
            cfgp.write_text(cfg_text, encoding="utf-8")
            ev_nt = dataclasses.replace(ev, cell_config={}, declaration={**fx["decl"], "control_variables": {
                "topology": "multi", "target_gpu": "<<FILL>>"}})
            ck("★캠페인 모드에서 타겟 선언을 읽지 못하면 roofline 으로 대체하지 않는다(target_gpu 키 부재 = 파생 불가)",
               "target_gpu" not in _arch_facts(repo, ev_nt))
            ptrp = repo / "campaigns/c1/evidence_pointers.json"
            ptr_text = ptrp.read_text(encoding="utf-8")
            ptr_doc = json.loads(ptr_text)
            ptr_doc["pointers"].append({"kind": "testlog", "path": fx["testlog"], "cell_id": "c1-a", "node_id": None})
            core.write_json(ptrp, ptr_doc)
            ck("★다노드 캠페인의 노드 태그 없는 포인터는 싣지 않는다(validator P2 · 귀속 불명)",
               len(from_campaign(repo, "c1", "c1-a", docker=dk).pointers) == 4)
            other_rep = "docs/benchmark/bench_report_26010111_fixture-model-nvfp4_GB10_0.9.0.md"
            (repo / other_rep).write_text((repo / fx["report"]).read_text(encoding="utf-8").replace(_M_UTC, "2026-01-01T02:00:00Z"),
                                          encoding="utf-8")
            ptr_doc = json.loads(ptr_text)
            ptr_doc["pointers"] = [p for p in ptr_doc["pointers"] if p["kind"] != "bench_report"] + [
                {"kind": "bench_report", "path": other_rep, "cell_id": "c1-a", "node_id": "cluster"}]
            core.write_json(ptrp, ptr_doc)
            ck("★인증서와 다른 측정의 리포트 포인터 = 섞지 않고 죽는다(생성일 ≠ measured_utc)",
               _code(from_campaign, repo, "c1", "c1-a", docker=dk) == "HINT_BENCH_REPORT_CERT_MISMATCH")
            (repo / other_rep).unlink()
            ptrp.write_text(ptr_text, encoding="utf-8")
            # attestation v2(serve 시점 캡처 · X12) — checks 는 비고 parity 가 대조 행이다
            attp = repo / "output/multi/benchlog/attestation_c1-a.json"
            att_text = attp.read_text(encoding="utf-8")
            v2 = {"schema_version": 2, "config": "c1-a", "image_tag": "fixture:0.9.0-source", "phase": "serve", "checks": [],
                  "nodes": {"main": {"image_id": _DIGEST, "build_ledger": {"vllm": {"describe": "v0.9.0"}}},
                            "sub": {"image_id": "sha256:" + "e" * 64}},
                  "parity": {"vllm_sha": {"verdict": "equal", "main": _SHA_A, "sub": _SHA_A}, "torch": {"verdict": "unobservable"}}}
            core.write_json(attp, v2)
            ev_v2 = from_campaign(repo, "c1", "c1-a", docker=dk)
            ck("v2 attestation(checks 비어 있음 · parity 관측) = 대조 행 결손 아님",
               ev_v2.attestation is not None and "HINT_MISSING_ATTESTATION_PARITY_ROWS" not in ev_v2.missing)
            ck("artifacts 이음매 — 묶인 attestation 경로 = lineage_seeds.attestation",
               ev_v2.lineage_seeds.get("attestation") == "output/multi/benchlog/attestation_c1-a.json")
            v2["parity"] = {"vllm_sha": {"verdict": "unobservable"}, "torch": {"verdict": "unobservable"}}
            core.write_json(attp, v2)
            ck("★v2 의 모든 축 unobservable = 대조 행 결손", "HINT_MISSING_ATTESTATION_PARITY_ROWS"
               in from_campaign(repo, "c1", "c1-a", docker=dk).missing)
            v2["nodes"]["main"]["image_id"] = "sha256:" + "f" * 64
            core.write_json(attp, v2)
            ev_x = from_campaign(repo, "c1", "c1-a", docker=dk)
            ck("★노드 image_id 에 측정 digest 가 없는 attestation(같은 태그 · 다른 빌드)은 묶지 않는다",
               ev_x.attestation is None and "HINT_MISSING_SLAVE_ATTESTATION" in ev_x.missing)
            attp.unlink()
            core.write_json(repo / "output/multi/benchlog/attestation_c1-build.json", {
                "config": "c1-build", "image_tag": "fixture:0.9.0-source", "checks": [{"axis": "torch", "node": "sub", "observed": "x"}]})
            ev_lbl = from_campaign(repo, "c1", "c1-a", docker=dk)
            ck("attestation — 셀 이름이 없으면 이미지 compose 라벨의 빌드 config 이름으로",
               ev_lbl.lineage_seeds.get("attestation") == "output/multi/benchlog/attestation_c1-build.json"
               and "compose 라벨" in (ev_lbl.attestation or {}).get("_source", ""))
            (repo / "output/multi/benchlog/attestation_c1-build.json").unlink()
            attp.write_text(att_text, encoding="utf-8")
            # 커밋 핀 — vllm_sha 는 핀 자체 · resolved.json 은 to_sha 가 그 핀일 때만 묶인다
            sha_c = "c" * 40
            hist0 = fx["docker"]["history"][_DIGEST]
            fx["docker"]["history"][_DIGEST] = hist0.replace("VLLM_REF=v0.9.0", f"VLLM_REF={sha_c}")
            envp.write_text(env_text.replace("VLLM_REF=v0.9.0", f"VLLM_REF={sha_c}"), encoding="utf-8")
            ev_pin = from_campaign(repo, "c1", "c1-a", docker=dk)
            ck("커밋 핀 = vllm_sha 는 핀 그 자체(resolved.json 미바인딩이어도 · 결손은 기재)",
               ev_pin.build_identity.get("vllm_sha") == sha_c and "HINT_MISSING_RESOLVE" in ev_pin.missing)
            ck("resolved.json — to_sha = 핀이면 묶고 ★다른 핀이면 묶지 않는다",
               _bind_resolve(repo, "multi", {"vllm_ref": _SHA_A})[0] is not None
               and _bind_resolve(repo, "multi", {"vllm_ref": sha_c})[0] is None)
            # ★ 트랙 ↔ 내용 — 소스빌드 선택자인데 측정 이미지 history 에 VLLM_REF 가 없다(wheel Dockerfile 로 지은 이미지)
            fx["docker"]["history"][_DIGEST] = "RUN |2 VLLM_VERSION=0.8.0 CUDA_VERSION=130 /bin/sh -c pip install\n"
            ck("★선택자 트랙과 다른 Dockerfile 로 지은 이미지 = 이름을 짓지 않는다(env 선언으로 폴백 ✗)",
               _code(naming_facts, repo, from_campaign(repo, "c1", "c1-a", docker=dk)) == "HINT_VLLM_INPUT_CONFLICT")
            fx["docker"]["history"][_DIGEST] = hist0
            envp.write_text(env_text, encoding="utf-8")
            envp.write_text(env_text.replace("BUILD_DOCKERFILE=Dockerfile.source-build\n", ""), encoding="utf-8")
            ev_nosel = from_campaign(repo, "c1", "c1-a", docker=dk)
            ck("★선택자 미관측이면 트랙을 추측하지 않는다(이름 파생 불가)", ev_nosel.build_identity.get("track") is None and
               _code(lambda: naming.derive_name(naming_facts(repo, ev_nosel), naming.load_vocab(repo))) == "HINT_AXIS_UNDERIVABLE")
            comp = repo / "output/multi/docker-compose.yaml"
            comp_text = comp.read_text(encoding="utf-8")
            comp.write_text(comp_text + "    build:\n      dockerfile: ${BUILD_DOCKERFILE:-Dockerfile.source-build}\n", encoding="utf-8")
            ev_def = from_campaign(repo, "c1", "c1-a", docker=dk)
            ck("셀 env 선택자 부재 → compose 기본값(slave_forward.compose_default)",
               ev_def.build_identity.get("track") == "source-build" and "기본값" in ev_def.build_identity["source"]["dockerfile"])
            comp.write_text(comp_text, encoding="utf-8")
            envp.write_text(env_text, encoding="utf-8")
            ident = identity(ev)
            ck("identity 6키 + base_model/hf_repo", ident["hf_repo"] == "Org/Fixture-Model-NVFP4"
               and ident["base_model"] == "Org/Fixture-Model" and ident["base_slug"] == "fixture-model")
            ck("X7 토폴로지 라벨", topology_label(ident) == "multi TP=2(Ray)" and
               topology_label({"topology": "single", "tp": 1}) == "single TP=1")
            ck("★라벨 파생 불가", _code(topology_label, {"topology": "multi", "tp": "2"}) == "HINT_TOPOLOGY_UNDERIVABLE")
            _selftest_round2(ck, repo, td, ev, ident)
            _selftest_round3(ck, repo, td, ev, fx)
            _selftest_v7(ck, repo, td, ev, fx, dk)
            _selftest_fix2(ck, td, ev)
            m = measurement(ev)
            ck("measurement 수치만(절대경로 칸 제외)", m.get("decode_tps_conc1") == "12.3" and "raw_json" not in json.dumps(m))
            mc = measurement_config(repo, ev)
            ck("measurement_config = 리포트 표 + 배포 키만", mc.get("bench_mode") == "full" and mc.get("repeats") == 3
               and set(mc) == set(DISTRIBUTED_MEASUREMENT_KEYS) | {"source"})
            # ── 승인(X3) ──
            ap = approval_for(repo, "c1", "c1-a", "cluster")
            ck("승인 — cluster 축을 배정 노드로 해소", (ap or {}).get("node_id") == "main" and ap["source"] == "campaign:hint_targets")
            ck("★승인 없는 셀 = None", approval_for(repo, "c1", "c1-b", "sub") is None)
            partial = json.loads(json.dumps(fx["decl"]))
            partial["hint_targets"][0]["approval"].pop("approved_utc")
            core.write_json(repo / "campaigns/c1/campaign.yaml", partial)
            ck("★반쪽 승인 = 죽는다", _code(approval_for, repo, "c1", "c1-a", "cluster") == "HINT_APPROVAL_INVALID")
            partial = json.loads(json.dumps(fx["decl"]))
            partial["hint_targets"][0]["arch"] = "gb10-main-native"
            core.write_json(repo / "campaigns/c1/campaign.yaml", partial)
            ck("★폐기 키 arch = 죽는다", _code(approval_for, repo, "c1", "c1-a", "cluster") == "HINT_APPROVAL_INVALID")
            core.write_json(repo / "campaigns/c1/campaign.yaml", fx["decl"])
            ck("★_bootstrap 발행 거부", _code(from_campaign, repo, "_bootstrap", "c1-a") == "HINT_CAMPAIGN_ID_RESERVED")
            ck("★경로 셀 거부", _code(from_campaign, repo, "c1", "../x") == "HINT_CELL_INVALID")
            ck("★측정 노드와 다른 축 거부", _code(from_campaign, repo, "c1", "c1-a", "main", docker=dk)
               == "HINT_ARCH_NODE_AXIS_CERT_MISMATCH")
            # ── 인증서 파서 ★ ──
            bad_cert = repo / "docs/benchmark/benchmark_26010213_bad_GB10_0.9.0.yaml"
            bad_cert.write_text('# 주석\nschema_version: 1\nmodel: demo\ngpu_model: "NVIDIA GB10"\ndecode_tps_conc1: 34.55\n',
                                encoding="utf-8")
            pc = parse_certificate(repo, _rel(repo, bad_cert))
            ck("인증서 파싱(게이트 파서 한 벌) — 따옴표 제거 · 전행 주석 무시 · 값은 원문 문자열",
               pc.get("gpu_model") == "NVIDIA GB10" and pc.get("decode_tps_conc1") == "34.55" and "주석" not in json.dumps(pc, ensure_ascii=False))
            for body, what in (("model: x\n  nested: y\n", "들여쓰기(중첩)"), ("a: 1\nb:\n  c: 2\n", "중첩 사전"),
                               ("# 주석뿐\n", "빈 인증서"), ("model: a\nmodel: b\n", "중복 키")):
                bad_cert.write_text(body, encoding="utf-8")
                ck(f"★{what} 인증서 = 파싱 거부", _code(parse_certificate, repo, _rel(repo, bad_cert)) == "HINT_CERTIFICATE_UNPARSEABLE")
            bad_cert.unlink()
            # ── no_cert_binding_source (단일 결정자) ──
            ck("waiver 가 map_only 보다 먼저", no_cert_binding_source({"task_class": "hint_map_only",
                                                                   "benchmark": {"perf_waiver": {"a": 1}}}, repo) == "perf_waiver")
            ck("map_only 는 클래스로 연다", no_cert_binding_source({"task_class": "hint_map_only", "benchmark": {}}, repo) == "hint_map_only")
            ck("explore 는 계약 통과로 연다", no_cert_binding_source({"task_class": "full_benchmark", "benchmark": {
                "rubric_authority": "explore", "rubric_source": "verdict_json", "floor_tps": 1.0, "ratio_M_over_primary": 0.9,
                "primary_source": "x"}}, repo) == "explore")
            ck("★공허 floor 의 explore 는 닫힌다", no_cert_binding_source({"task_class": "full_benchmark", "benchmark": {
                "rubric_authority": "explore", "rubric_source": "verdict_json", "floor_tps": 0, "ratio_M_over_primary": 0.9,
                "primary_source": "x"}}, repo) is None)
            ck("인증서가 map_only 여도 이긴다", binding_artifact_path({"task_class": "hint_map_only", "benchmark": {},
                "evidence": {"certificate": {"path": "c"}, "bench_report": {"path": "r"}}}, repo) == "c")
            # ── publication_records = lineage 규칙(tripwire) ──
            (repo / core.REL_EVIDENCE_DIR).mkdir(parents=True, exist_ok=True)
            (repo / core.REL_EVIDENCE_DIR / "broken.json").write_text("{", encoding="utf-8")
            from . import lineage
            ck("publication_records ≡ lineage.load_publication_records",
               publication_records(repo) == lineage.load_publication_records(repo))
            (repo / core.REL_EVIDENCE_DIR / "broken.json").unlink()
            # ── drive_publisher · ★손 JSON 0 · runtime 관측 ──
            draft = repo / "hints/.drafts/d1"
            before = _tree(repo)
            topic = topic_for(ev)
            ck(f"토픽은 셀 키로 끝난다({topic})", topic == "c1__cluster__c1_a")
            sp_ = lineage.seeds_from_publications(repo, [{"_id": "c0__cluster__c1_a", "identity": ev.lineage_seeds["identity"],
                                                          "generated_utc": "2026-01-01T00:00:00Z", "scaffolded": {}}],
                                                  this_topic=topic, identity=ev.lineage_seeds["identity"], cell_key=ev.cell)
            ck("이음매 — 다른 캠페인의 같은 셀 발행이 lineage 과거 발행 시드로 잡힌다(토픽 끝 = 셀 키)",
               "c0__cluster__c1_a" in sp_["prior"])
            man = drive_publisher(repo, ev, topic=topic, generated_utc="2026-01-02T05:00:00Z", draft_dir=draft)
            after = _tree(repo)
            new = sorted(set(after) - set(before))
            changed = sorted(k for k in set(before) & set(after) if before[k] != after[k])
            allowed = lambda p: (p.startswith("hints/.drafts/d1/inputs/") or p.startswith("docs/_evidence/")  # noqa: E731
                                 or p.startswith("docs/simlog/"))
            ck(f"★발행기 구동은 inputs/·발행기 소유 평면 밖에 쓰지 않는다(new={[p for p in new if not allowed(p)]})",
               all(allowed(p) for p in new) and not changed)
            ck("inputs/ 는 도구가 쓴 JSON 뿐", sorted(p.name for p in (draft / "inputs").iterdir()) ==
               ["identity.json", "pii.json", "runtime.json", "runtime.provenance.json"])
            ck("★발행기 identity.json 의 quant 는 측정값(N/A → None) — PAYLOAD 표시용 이름 축 대체값이 아니다",
               json.loads((draft / "inputs/identity.json").read_text(encoding="utf-8")).get("quant") is None)
            mdoc = json.loads(man.read_text(encoding="utf-8"))
            ck("manifest runtime.identity = 스윕 meta 관측", mdoc.get("runtime", {}).get("identity", {}).get("tp") == 2)
            ck("X5 simlog 면제 기재", any(e["reason"] == cgate(repo).PII_MACHINE_RAW_EXEMPTION_REASON
                                         for e in mdoc["pii_scan"].get("exempt_paths", [])))
            auth = authorize(repo, man, "hint_finalize")
            ck("게이트 허가(격리 저장소 · promotion-ready)", auth.get("allowed") is True)
            # set_promotion_target → finalize 방출
            ck("★draft 없이 목표 기록 거부", _code(set_promotion_target, repo, topic=topic, tag="hint/x", topology="multi TP=2(Ray)",
                                             anchor=_SHA_A, generated_utc="2026-01-02T06:00:00Z") == "HINT_PUBLISHER_INPUTS_ABSENT")
            tag = dn.tag
            man2 = set_promotion_target(repo, topic=topic, tag=tag, topology="multi TP=2(Ray)", anchor=_SHA_A,
                                        generated_utc="2026-01-02T06:00:00Z", draft_dir=draft)
            mdoc2 = json.loads(man2.read_text(encoding="utf-8"))
            ck("promotion_target 방출(writer = finalize)", mdoc2.get("promotion_target", {}).get("anchor") == _SHA_A)
            ck("봉인 대상 = manifest 목표(X7 라벨 · H3 vLLM 비대조)",
               promotion_binding_problems(mdoc2, tag=tag, topology="multi TP=2(Ray)", anchor=_SHA_A) == [])
            ck("★다른 앵커·라벨은 불일치", {c for c, _ in promotion_binding_problems(mdoc2, tag=tag, topology="multi TP=2",
                                                                          anchor="b" * 40)} ==
               {"HINT_PROMOTION_TARGET_TOPOLOGY_MISMATCH", "HINT_PROMOTION_TARGET_ANCHOR_MISMATCH",
                "HINT_IDENTITY_TOPOLOGY_LABEL_MISMATCH"})
            ck("★목표 없는 manifest", promotion_binding_problems({}, tag=tag, topology="x", anchor="y")[0][0]
               == "HINT_PROMOTION_TARGET_MISSING")
            ck("★짧은 앵커 = 발행기 거부", _code(set_promotion_target, repo, topic=topic, tag=tag, topology="multi TP=2(Ray)",
                                          anchor="abc", generated_utc="2026-01-02T06:00:00Z", draft_dir=draft)
               == "HINT_PUBLISHER_FAILED")
            man_re = drive_publisher(repo, ev, topic=topic, generated_utc="2026-01-02T05:00:00Z", draft_dir=draft)
            ck("★발행기 재구동(full)은 멱등이고 기록된 승격 목표를 말없이 떨어뜨리지 않는다",
               man_re == man and (json.loads(man_re.read_text(encoding="utf-8")).get("promotion_target") or {}).get("anchor") == _SHA_A)
            # ★ runtime identity 는 관측이다 — meta 가 다르면 복사하지 않고 그대로 적는다(게이트가 거부)
            idx = json.loads((fx["sweep"] / "sweep_index.json").read_text(encoding="utf-8"))
            idx["meta"]["vllm_version"] = "0.9.0-observed"
            core.write_json(fx["sweep"] / "sweep_index.json", idx)
            ev2 = from_campaign(repo, "c1", "c1-a", docker=dk)
            man3 = drive_publisher(repo, ev2, topic=topic + "_rt", generated_utc="2026-01-02T07:00:00Z",
                                   draft_dir=repo / "hints/.drafts/d2")
            rt = json.loads((repo / "hints/.drafts/d2/inputs/runtime.json").read_text(encoding="utf-8"))
            ck("★runtime.identity 는 identity 복사가 아니다(스윕 meta 관측)", rt["identity"]["vllm"] == "0.9.0-observed")
            denied = None
            try:
                authorize(repo, man3, "hint_finalize")
            except core.HintError as e:
                denied = e
            ck("★관측이 identity 와 다르면 게이트 거부(원문 JSON · 종료코드)", denied is not None and denied.code == "HINT_GATE_DENIED"
               and "RUNTIME_IDENTITY_MISMATCH" in denied.message and denied.exit_code != 0)
            idx["meta"]["vllm_version"] = "0.9.0"
            # ★ 스윕 generated_utc 불일치 = 묶지 않는다 → 자격 관측 불가
            idx["generated_utc"] = "2026-01-09T00:00:00Z"
            core.write_json(fx["sweep"] / "sweep_index.json", idx)
            ev3 = from_campaign(repo, "c1", "c1-a", docker=dk)
            ck("★generated_utc ≠ measured_utc = 스윕 미바인딩", ev3.sweep is None and "HINT_MISSING_SWEEP" in ev3.missing)
            ck("★스윕 미바인딩 → 자격 관측 불가 차단", _code(qualification, ev3) == "HINT_QUALIFICATION_UNOBSERVED")
            spp = repo / "output/multi/benchlog/serve_proof_c1-a.json"
            sp_base = {"schema_version": 1, "kind": "multinode_serve_proof", "config": "c1-a", "topology": "multi", "ready": True,
                       "health_http": 200, "inference_verdict": "pass", "evidence": "chat.content", "content_len": 12,
                       "completion_text_len": None, "image_tag": "fixture:0.9.0-source",
                       "provenance": "measured(multinode_serve_smoke)"}
            core.write_json(spp, sp_base)
            ck("serve_proof 가 1순위 자격", qualification(from_campaign(repo, "c1", "c1-a", docker=dk)).get("method") == "serve_proof")
            core.write_json(spp, {**sp_base, "evidence": "v1.completions", "content_len": 0, "completion_text_len": 5})
            ck("serve_proof — /v1/completions 로 확증한 pass 도 자격(chat content_len=0)",
               qualification(from_campaign(repo, "c1", "c1-a", docker=dk)).get("method") == "serve_proof")
            # native producer contract: explicit plane plus preserved evidence; no Docker
            # signal is present, and the qualification path must require cleanup PASS.
            nroot = repo / "native-evidence"; nroot.mkdir()
            install = nroot / "native-install.sh"; install.write_text("python -m pip install vllm\n", encoding="utf-8")
            freeze_main = nroot / "pip-freeze-main.txt"; freeze_main.write_text("vllm==0.9.0\n", encoding="utf-8")
            freeze_sub = nroot / "pip-freeze-sub.txt"; freeze_sub.write_text("vllm==0.9.0\n", encoding="utf-8")
            cleanup = nroot / "cleanup.json"
            clean_base = {"schema_version": 1, "kind": "native_multinode_cleanup_attestation", "plane": "native",
                          "run_id": "native-fixture", "status": "PASS", "errors": [], "evidence_preserved": True,
                          "nodes": {n: {"root_absent": True, "owned_processes": [], "owned_gpu_processes": [], "unknown": []}
                                    for n in ("main", "sub")}}
            core.write_json(cleanup, clean_base)
            native_base = {"schema_version": 1, "kind": "native_multinode_serve_proof", "plane": "native", "status": "PASS",
                           "cell": "c1-a", "topology": "multi", "run_id": "native-fixture",
                           "health": {"ok": True}, "inference": {"ok": True, "completion_text_len": 3},
                           "endpoints": {"health": {"http_status": 200}, "inference": {"http_status": 200}},
                           "native_install_path": "native-evidence/native-install.sh",
                           "pip_freeze_paths": {"main": "native-evidence/pip-freeze-main.txt", "sub": "native-evidence/pip-freeze-sub.txt"},
                           "cleanup_attestation_path": "native-evidence/cleanup.json"}
            core.write_json(spp, native_base)
            native_ev = from_campaign(repo, "c1", "c1-a", docker=dk)
            ck("native proof는 explicit plane을 CellEvidence로 전달", native_ev.plane == "native" and
               native_ev.native_install_path == "native-evidence/native-install.sh" and native_ev.cleanup_attestation_path == "native-evidence/cleanup.json")
            ck("native proof + cleanup PASS가 자격", qualification(native_ev).get("method") == "native_serve_proof+cleanup_attestation")
            # 기동 기록(2026-09-29 · plan_26092908 §4.2 · D 통합): 옛 proof = 필드 없음(결손 정상) · 있으면 보존 파일 계약 · 사유는 옮긴다
            ck("native 기동 기록: 옛 proof(필드 없음) = None · 사유 없음", native_ev.native_launch_path is None
               and native_ev.native_launch_error is None and "옛 proof" in native_ev.sources.get("native_launch_path", ""))
            launch = nroot / "native-launch.md"; launch.write_text("# native 기동 기록\n", encoding="utf-8")
            core.write_json(spp, {**native_base, "native_launch_path": "native-evidence/native-launch.md"})
            nl_ev = from_campaign(repo, "c1", "c1-a", docker=dk)
            ck("native 기동 기록: proof native_launch_path → CellEvidence(저장소 상대) · artifacts 가 serve_proof 로도 읽는다",
               nl_ev.native_launch_path == "native-evidence/native-launch.md" and nl_ev.native_launch_error is None
               and (nl_ev.serve_proof or {}).get("native_launch_path") == "native-evidence/native-launch.md")
            core.write_json(spp, {**native_base, "native_launch_error": "OSError: disk full"})
            ne_ev = from_campaign(repo, "c1", "c1-a", docker=dk)
            ck("native 기동 기록 부재 + producer 사유 = native_launch_error 로 옮긴다(결손 사유)",
               ne_ev.native_launch_path is None and ne_ev.native_launch_error == "OSError: disk full"
               and "disk full" in ne_ev.sources.get("native_launch_path", ""))
            core.write_json(spp, {**native_base, "native_launch_path": "/abs/native-launch.md"})
            ck("★native 기동 기록 절대경로 = HINT_NATIVE_EVIDENCE_PATH_ABSOLUTE(보존 파일 계약)",
               _code(from_campaign, repo, "c1", "c1-a", docker=dk) == "HINT_NATIVE_EVIDENCE_PATH_ABSOLUTE")
            core.write_json(spp, native_base)
            core.write_json(cleanup, {**clean_base, "status": "FAIL"})
            ck("★native cleanup nonpass 차단", _code(from_campaign, repo, "c1", "c1-a", docker=dk) == "HINT_NATIVE_CLEANUP_NONPASS")
            core.write_json(cleanup, clean_base)
            core.write_json(spp, {k: v for k, v in native_base.items() if k != "cleanup_attestation_path"})
            ck("★native cleanup signal 없음 차단", _code(from_campaign, repo, "c1", "c1-a", docker=dk) == "HINT_NATIVE_EVIDENCE_MISSING")
            core.write_json(spp, sp_base)
            for patch, what in (({"inference_verdict": "fail", "evidence": None, "content_len": 0}, "빈 응답 fail"),
                                ({"inference_verdict": "relaxed-reasoning-only", "evidence": "chat.reasoning(relaxed)",
                                  "content_len": 0, "reasoning_len": 40}, "완화 PASS(reasoning-only)"),
                                ({"inference_verdict": "not-attempted", "ready": False, "health_http": None, "evidence": None,
                                  "content_len": None}, "READY 미도달"),
                                ({"image_tag": "fixture:0.8.0-source"}, "다른 이미지 태그의 스모크"),
                                ({"inference_verdict": "passed-somehow"}, "모르는 판정어")):
                core.write_json(spp, {**sp_base, **patch})
                ck(f"★serve_proof 불성립 — {what}", _code(qualification, from_campaign(repo, "c1", "c1-a", docker=dk))
                   == "HINT_QUALIFICATION_UNOBSERVED")
            ck("★재생 모드 쓰기 거부", _code(drive_publisher, repo, SimpleNamespace(mode="publication-replay"), topic="t",
                                         generated_utc="2026-01-02T05:00:00Z", draft_dir=draft) == "HINT_REPLAY_READ_ONLY")
            # ── from_publication(재생 · simlog 사본으로 셀 파생 · 출력 스윕은 다른 측정) ──
            evp = from_publication(repo, topic, docker=dk)
            ck("재생 — 셀은 simlog 사본 meta 에서", evp.cell == "c1-a" and evp.mode == "publication-replay")
            ck("재생 — 출력 스윕이 덮였으면 simlog 사본만", (evp.sweep or {}).get("binding") == "simlog-copy"
               and "HINT_MISSING_SWEEP_RAW" in evp.missing and "HINT_MISSING_CAMPAIGN_INSTANCE" in evp.missing)
            ck("재생 — 계보 시드 this_topic", evp.lineage_seeds.get("this_topic") == topic)
            ck("★재생 — simlog 사본만 묶이면 roofline 도 없다 → 타겟 미관측(파생 불가)", "target_gpu" not in _arch_facts(repo, evp))
            # 재생 2순위(2026-09-29 · plan_26092908 S6 — lite-only D2 재생이 arch 파생 불가로 막혔다): 봉인된 페이로드의 target=native
            with tempfile.TemporaryDirectory(prefix="hint-ev-tgt-") as gd:
                g = Path(gd)
                ident = ["-c", "user.name=fx", "-c", "user.email=" + "fx" + "\x40" + "fixture.invalid"]
                core.git(g, "init", "-q")
                sha_of = {}
                for tv in ("native", "sim-h100"):
                    core.write_json(g / "PAYLOAD.json", {"naming": {"axes": {"target": {"value": tv, "source": "fx"}}}})
                    core.git(g, "add", "PAYLOAD.json")
                    core.git(g, *ident, "commit", "-q", "-m", tv)
                    sha_of[tv] = core.git(g, "rev-parse", "HEAD").stdout.strip()
                def _ev(anchor):
                    return SimpleNamespace(cell_config=None, declaration=None, mode="publication-replay", sweep=None,
                                           publication={"record": {"promotion_target": {"anchor": anchor}}})
                t0, s0 = _target_gpu(_ev(sha_of["native"]), g)
                ck("재생 — 봉인된 페이로드 target=native → 선언 없음(출처가 봉인 페이로드를 말한다)",
                   t0 is None and s0 is not None and "봉인된 발행 페이로드" in s0)
                ck("★재생 — 봉인된 페이로드 target=sim-* 는 원문 hw 를 되살리지 않는다(대체 ✗)",
                   _target_gpu(_ev(sha_of["sim-h100"]), g) == (None, None))
                ck("★재생 — repo 없이는 대체 ✗", _target_gpu(_ev(sha_of["native"])) == (None, None))
                # F4(plan_26092908 §4.5): 재생 캠페인 · 승인 = 봉인 페이로드의 campaign · approval(재생 전용 대체 · 출처)
                core.write_json(g / "PAYLOAD.json", {"campaign": {"id": "camp-fx", "cell": "c1-a", "node": "cluster", "mode": "campaign"},
                                                     "approval": {"approved_by": "사용자(fx)", "approved_utc": "2026-01-01T00:00:00Z",
                                                                  "source": "campaign:hint_targets"}})
                core.git(g, "add", "PAYLOAD.json")
                core.git(g, *ident, "commit", "-q", "-m", "camp")
                a4 = core.git(g, "rev-parse", "HEAD").stdout.strip()

                def _ev4(anchor, cell="c1-a", mode="publication-replay"):
                    return SimpleNamespace(mode=mode, cell=cell, node="cluster",
                                           publication={"record": {"promotion_target": {"anchor": anchor}}})
                sp4 = sealed_publication(g, _ev4(a4))
                ck("F4 재생 — 봉인 페이로드의 campaign.id · approval 을 출처와 함께(재생 전용 대체)",
                   sp4 and sp4["campaign"]["id"] == "camp-fx" and sp4["approval"]["approved_utc"] == "2026-01-01T00:00:00Z"
                   and "재생 전용 대체" in sp4["source"] and a4[:12] in sp4["source"])
                ck("★F4 음성대조: 다른 셀의 봉인 기록 · 캠페인 모드 · 캠페인 없는 봉인 페이로드는 대체 ✗",
                   sealed_publication(g, _ev4(a4, cell="other")) is None
                   and sealed_publication(g, _ev4(a4, mode="campaign")) is None
                   and (sealed_publication(g, _ev4(sha_of["native"])) or {}).get("campaign") is None)
            idx["generated_utc"] = _M_UTC
            core.write_json(fx["sweep"] / "sweep_index.json", idx)
            arp = _arch_facts(repo, from_publication(repo, topic, docker=dk))
            ck("재생 — 출력 스윕이 묶이면 타겟은 roofline 대체이고 출처가 그렇게 말한다(선언 아님)",
               arp.get("target_gpu") == "NVIDIA GB10" and "재생 전용" in arp.get("source", ""))
            idx["generated_utc"] = "2026-01-09T00:00:00Z"
            core.write_json(fx["sweep"] / "sweep_index.json", idx)
            # ── phase_set_publish(명시 id · cluster → 배정 노드) ──
            calls = []
            phase_set_publish(repo, "c1", "c1-a", "cluster", tag, "b" * 40, "2026-01-02T08:00:00Z",
                              runner=lambda *a: calls.append(a) or SimpleNamespace(returncode=0, stderr=""))
            a = calls[0] if calls else ()
            ck("publish 위상 — 명시 --campaign-id · 배정 노드 · 원격 SHA 출처 · ★시작 시각을 지어내지 않는다",
               a[:2] == ("--campaign-id", "c1") and a[a.index("--node") + 1] == "main" and f"refs/tags/{tag}@{'b' * 40}" in a
               and "--started-utc" not in a)
            phase_set_publish(repo, "c1", "c1-a", "cluster", tag, "b" * 40, "2026-01-02T08:00:00Z")
            pdoc = _read_json_opt(repo / "campaigns/c1/phases/main/publish.status.json", "publish status") or {}
            ck("publish 위상 — 실물 campaign_init CLI(격리 사본)가 진행표를 쓴다",
               pdoc.get("state") == "done" and (pdoc.get("proof") or {}).get("source") == f"refs/tags/{tag}@{'b' * 40}"
               and pdoc.get("cell_id") == "c1-a" and pdoc.get("ended_utc") == "2026-01-02T08:00:00Z"
               and pdoc.get("started_utc") is None and pdoc.get("authored_by") == "main")
            ck("★40자 아닌 원격 SHA 거부", _code(phase_set_publish, repo, "c1", "c1-a", "cluster", tag, "abc",
                                             "2026-01-02T08:00:00Z", runner=lambda *x: None) == "HINT_REMOTE_SHA_INVALID")
            # ── lite 셀(측정 구성 lite · 자격 lite_warm) ──
            (repo / "output/multi/configs/c1-b.yaml").write_text("model: /app/quant_models/Org/Fixture-Model-NVFP4\n"
                                                                 "tensor-parallel-size: 2\nmax-model-len: 2048\n", encoding="utf-8")
            (repo / "output/multi/configs/c1-b.sh").write_text("vllm serve\n", encoding="utf-8")
            (repo / "output/multi/envs/.env.c1-b").write_text("IMAGE_TAG=fixture:0.9.0-source\n", encoding="utf-8")
            lite_rel = "docs/benchmark/bench_report_26010214_fixture-model-nvfp4_GB10_0.9.0.md"
            rbs = _rbs(repo)
            (repo / lite_rel).write_text("\n".join([
                "# 경량", "", rbs.REPORT_LITE_MODE_LINE, "", "> 생성일 2026-01-02T14:00:00Z.", "", rbs.MEASUREMENT_CONFIG_TITLE,
                "", "| 키 | 값 |", "|---|---|", "| bench_mode | lite |", "| bench_mode_kind | declared-lite |", "| repeats | 1 |", "",
                "## lite 지표 (서빙 성공 직후)", "", "| 메트릭 | Main | Sub |", "|---|---|---|",
                "| gen tokens/sec (warm) [master] | 9.00 t/s | — |", "", "## 측정 환경 스냅샷", "", "| 키 | 값 |", "|---|---|",
                "| model | fixture-model-nvfp4 |", "| gpu_model | NVIDIA GB10 |", "| vllm_version | 0.9.0 |", "| topology | multi |", ""]),
                encoding="utf-8")
            ptrs = json.loads((repo / "campaigns/c1/evidence_pointers.json").read_text(encoding="utf-8"))
            ptrs["pointers"].append({"kind": "bench_report", "path": lite_rel, "cell_id": "c1-b", "node_id": "sub"})
            core.write_json(repo / "campaigns/c1/evidence_pointers.json", ptrs)
            stale = repo / "output/multi/benchlog/sweep_c1-b"
            stale.mkdir()
            core.write_json(stale / "sweep_index.json", {"generated_utc": "2025-01-01T00:00:00Z", "meta": {}})
            # 2026-09-29(plan_26092908 §4.8 · V11③): `--node` 미지정이면 TP=2 > gpus_per_node=1 → 배정(sub)이 아니라 cluster 로 파생된다.
            #   옛 판본은 배정 sub 로 떨어져 이름 파생에서야 HINT_ARCH_TP_CONFLICT 로 막혔다(멀티 셀 3/3 손 `--node cluster`).
            eva = from_campaign(repo, "c1", "c1-b", docker=dk)
            ck("★lite 셀 — `--node` 미지정 + 측정 TP 2 > 노드당 GPU 1 → 자동 cluster(배정 sub 를 이긴다)",
               eva.node == "cluster" and "자동 파생" in eva.sources["node"] and "선언" in eva.sources["node"])
            evl = from_campaign(repo, "c1", "c1-b", "sub", docker=dk)
            ck("★lite 셀은 옛 스윕을 묶지 않고 스윕 결손도 적지 않는다", evl.sweep is None and "HINT_MISSING_SWEEP" not in evl.missing)
            ck("★멀티 체크아웃의 단일 노드(sub) 실행은 노드 정합 대상 아님",
               "HINT_MISSING_SLAVE_ATTESTATION" not in evl.missing and evl.attestation is None)
            ck("lite 셀 — 노드 축 = 명시(sub) · report_kind lite", evl.node == "sub" and evl.report_kind == "lite")
            # 2026-09-29(plan_26092908 §4.4 · V2): lite-only 는 인증서가 구조적으로 없다 — 결손이 아니라 OBSERVATION-ONLY 판정이다.
            ck("lite 셀 — BENCH_MODE_LITE 기재 · ★인증서 결손 아님(비발행 판정)", "BENCH_MODE_LITE" in evl.missing
               and "HINT_MISSING_CERTIFICATE" not in evl.missing and "HINT_MISSING_LITE" not in evl.missing
               and "lite-only" in evl.sources.get("certificate_due", ""))
            ml = measurement(evl)
            ck("lite 셀 measurement — verdict OBSERVATION-ONLY · lite 수치(리포트 표) · 측정 시각 = 리포트 생성일",
               ml.get("verdict") == "OBSERVATION-ONLY" and ml.get("benchmark_mode") == "lite"
               and (ml.get("lite") or {}).get("gen_tps") == 9.0 and ml.get("measured_utc") == "2026-01-02T14:00:00Z"
               and ml.get("decode_tps_conc1") is None and "lite-only" in ml["sources"]["verdict"])
            ck("★lite_warm 없으면 자격 차단", _code(qualification, evl) == "HINT_QUALIFICATION_UNOBSERVED")
            warm = repo / "output/multi/benchlog/lite_warm_c1-b.json"
            core.write_json(warm, {"completed": 3, "failed": 0})
            ck("★조인 키(lite_raw) 없는 lite_warm 은 자격이 아니다", _code(qualification, evl) == "HINT_QUALIFICATION_UNOBSERVED")
            rawp = repo / "output/multi/benchlog/lite_raw_c1-b.json"
            core.write_json(rawp, {"config_name": "c1-b", "measured_utc": "2025-12-31T00:00:00Z", "bench_warm_json": str(warm)})
            ck("★다른 실행의 lite_raw(measured_utc ≠ 리포트 생성일)는 자격이 아니다",
               _code(qualification, evl) == "HINT_QUALIFICATION_UNOBSERVED")
            core.write_json(rawp, {"config_name": "c1-b", "measured_utc": "2026-01-02T14:00:00Z", "bench_warm_json": str(warm)})
            ck("lite 자격 = lite_raw 조인(measured_utc = 생성일) + lite_warm",
               qualification(evl).get("method", "").startswith("lite_raw+lite_warm"))
            ck("★TP=2 측정을 단일 노드(sub) 형상으로 이름 짓지 않는다", _code(naming_facts, repo, evl) == "HINT_ARCH_TP_CONFLICT")
            ck("같은 셀을 cluster 축으로 발행하면 2노드 형상", _arch_facts(repo, from_campaign(repo, "c1", "c1-b", "cluster",
                                                                                        docker=dk)).get("nodes") == 2)
            before_l = _tree(repo)
            ck("★셀 서사 증거가 없으면 쓰기 전에 차단", _code(drive_publisher, repo, evl, topic=topic_for(evl),
                                                        generated_utc="2026-01-02T15:00:00Z",
                                                        draft_dir=repo / "hints/.drafts/d3") == "HINT_NARRATIVE_EVIDENCE_ABSENT"
               and _tree(repo) == before_l)
            for kind, rel_doc in (("devlog", "docs/devlog/devlog_26010215_lite.md"), ("testlog", "docs/testlog/testlog_26010215_lite.md")):
                (repo / rel_doc).write_text(f"# {kind}\nlite 셀 서사.\n", encoding="utf-8")
                ptrs["pointers"].append({"kind": kind, "path": rel_doc, "cell_id": "c1-b", "node_id": "sub"})
            core.write_json(repo / "campaigns/c1/evidence_pointers.json", ptrs)
            evl = from_campaign(repo, "c1", "c1-b", "sub", docker=dk)
            manl = drive_publisher(repo, evl, topic=topic_for(evl), generated_utc="2026-01-02T15:00:00Z",
                                   draft_dir=repo / "hints/.drafts/d3")
            mdl = json.loads(manl.read_text(encoding="utf-8"))
            ck("lite 발행 = hint_map_only + 경량 리포트 바인딩", mdl.get("task_class") == "hint_map_only"
               and mdl["benchmark"].get("mode") == "lite" and (mdl["evidence"].get("bench_report") or {}).get("path"))
            ck("lite 발행 게이트 허가", authorize(repo, manl, "hint_finalize").get("allowed") is True)
            # ── 강등 셀(full 로 쟀으나 반복 불성립 → downgraded-lite · plan_26091407 §4.5) — init full → publish-benchmark
            #    (인증서 억제) → init --downgrade-from 경로. 종전에는 이 분기를 한 번도 돌리지 않았다(2026-09-22 적대 검토).
            decl_d = json.loads(json.dumps(fx["decl"]))
            decl_d["assignments"]["main"].append({"cell": "c1-d", "mode": "AUTO"})
            core.write_json(repo / "campaigns/c1/campaign.yaml", decl_d)
            for suffix in ("configs/{}.yaml", "configs/{}.sh", "envs/.env.{}"):
                src_t = (repo / "output/multi" / suffix.format("c1-a")).read_text(encoding="utf-8")
                (repo / "output/multi" / suffix.format("c1-d")).write_text(src_t.replace("c1-a", "c1-d"), encoding="utf-8")
            d_utc = "2026-01-03T01:00:00Z"
            sw_d = repo / "output/multi/benchlog/sweep_c1-d"
            (sw_d / "level_01").mkdir(parents=True)
            idx_d = json.loads(json.dumps(idx))
            idx_d.update({"config": "c1-d", "generated_utc": d_utc})
            idx_d["meta"].update({"config_name": "c1-d", "vllm_version": "0.9.0"})
            core.write_json(sw_d / "sweep_index.json", idx_d)
            core.write_json(sw_d / "level_01/measured.json", {"decode_tps": 12.0, "measurement_ok": True})
            core.write_json(sw_d / "level_01/bench_c1-d.json", {"completed": 4, "failed": 0})
            core.write_json(sw_d / "level_01/post_health_c1-d.json", {"health_http_code": "200", "container_running": "true",
                                                                      "container_oom_killed": "false"})
            core.write_json(sw_d / "verdict.json", {"verdict": "PASS"})
            rep_d = "docs/benchmark/bench_report_26010310_fixture-model-nvfp4_GB10_0.9.0.md"
            # ★ PII 회귀(옛 hint_collect #80-81 · 2026-09-14 리뷰 "픽스처가 실물보다 좁았다"): 강등 사유 출처 칸에는 sweep_bench 가
            #   `classify_cell --events-from-repo "$REPO"` 로 부른 **운영자 절대경로**가 박힌다. 배포 평면(measurement_config)에 0건이어야
            #   한다. 추적 파일에 그 모양의 리터럴을 쓰지 않도록 실행 시 조립한다(2026-08-06 자기스캔 사고와 같은 규율).
            op_path = "/" + "home" + "/op-fixture/ws/docs/logs/main/events/2026-01.jsonl"
            (repo / rep_d).write_text("\n".join([
                "# 성능 보고서", "", f"> 생성일 {d_utc}.", "", rbs.MEASUREMENT_CONFIG_TITLE, "", "| 키 | 값 |", "|---|---|",
                "| bench_mode | lite |", "| bench_mode_kind | downgraded-lite |",
                f"| bench_mode_source | derived(classify_cell · events {op_path}) |",
                "| downgrade_reason | run_failed |", f"| downgrade_reason_source | events({op_path}) |", "| repeats | 3 |", "",
                "## lite 지표 (full ⊇ lite)", "", "| 메트릭 | Main | Sub |", "|---|---|---|",
                "| gen tokens/sec (warm) [master] | 10.00 t/s | — |", "",
                "## 부하 스윕 곡선", "", rbs.REPORT_HEADER, "|---|---|---|---|---|---|---|",
                "| 1 ★판정점 | 12.0 | 12 | 60 | 100 | 20 | 4/0 |", "| 2 | 20.0 | 20 | 90 | 120 | 25 | 4/0 |", ""]),
                encoding="utf-8")
            for kind, rel_doc in (("devlog", "docs/devlog/devlog_26010311_down.md"), ("testlog", "docs/testlog/testlog_26010311_down.md")):
                (repo / rel_doc).write_text(f"# {kind}\n강등 셀 서사.\n", encoding="utf-8")
                ptrs["pointers"].append({"kind": kind, "path": rel_doc, "cell_id": "c1-d", "node_id": "main"})
            ptrs["pointers"].append({"kind": "bench_report", "path": rep_d, "cell_id": "c1-d", "node_id": "main"})
            core.write_json(repo / "campaigns/c1/evidence_pointers.json", ptrs)
            evd = from_campaign(repo, "c1", "c1-d", "cluster", docker=dk)
            mcd = measurement_config(repo, evd)
            ck("강등 셀 — 측정 구성 표가 downgraded-lite 를 말한다 · BENCH_MODE_LITE 기재",
               mcd.get("bench_mode_kind") == "downgraded-lite" and mcd.get("downgrade_reason") == "run_failed"
               and "BENCH_MODE_LITE" in evd.missing)
            ck("★PII 회귀 — 자유 서술 *_source 의 운영자 절대경로가 배포 평면(measurement_config·measurement)에 0건",
               "op-fixture" not in json.dumps([mcd, measurement(evd)], ensure_ascii=False)
               and not any(k.endswith("_source") for k in mcd))
            mand = drive_publisher(repo, evd, topic=topic_for(evd), generated_utc="2026-01-03T02:00:00Z",
                                   draft_dir=repo / "hints/.drafts/d4")
            mdd = json.loads(mand.read_text(encoding="utf-8"))
            recd = _read_json_opt(repo / core.REL_EVIDENCE_DIR / f"{topic_for(evd)}.json", "강등 기록") or {}
            ck("강등 셀 발행 = hint_map_only 재분류(사유 run_failed · 판정 보존 · 리포트 바인딩)",
               mdd.get("task_class") == "hint_map_only" and mdd["benchmark"].get("mode") == "lite"
               and (recd.get("reclassification") or {}).get("reason") == "run_failed"
               and (mdd["evidence"].get("bench_report") or {}).get("path", "").endswith(Path(rep_d).name))
            ck("강등 셀 발행 게이트 허가", authorize(repo, mand, "hint_finalize").get("allowed") is True)
            ck("강등 셀 재실행은 멱등(같은 manifest · 재분류 재시도 ✗)",
               drive_publisher(repo, evd, topic=topic_for(evd), generated_utc="2026-01-03T02:00:00Z",
                               draft_dir=repo / "hints/.drafts/d4") == mand)
            core.write_json(repo / "campaigns/c1/campaign.yaml", fx["decl"])
            # 결손 코드 tripwire — hintlib 모듈이 **실제로 발행하는** 코드(`missing.append("…")`·`["HINT_MISSING_…"]`)는 전부 이 표에
            #   있어야 한다. 옛 검사는 아무도 발행하지 않는 코드의 등재를 확인해 공허하게 초록이었다(2026-09-22 적대 검토).
            emitted: set[str] = set()
            for f in sorted(core.HINTLIB_DIR.glob("*.py")):
                t = f.read_text(encoding="utf-8")
                emitted |= set(re.findall(r'missing\.append\("([A-Z_]+)"\)', t))
                emitted |= set(re.findall(r'\["(HINT_MISSING_[A-Z_]+)"\]', t))
            ck(f"결손 코드 tripwire — 발행되는 코드 전부 등재(미등재 {sorted(emitted - set(MISSING_CODES))})",
               {"HINT_MISSING_APPLIED_PATCH_FILE", "HINT_MISSING_ENV_SHAPE"} <= emitted
               and emitted <= set(MISSING_CODES))
            # 2026-09-22 S2 round 3 통합: 뜻 문구는 페이로드로 수신자에게 간다 — 수신자가 풀 수 없는 내부 좌표가 있으면 붉다.
            leaked = {c: _internal_doc_refs(m) for c, m in MISSING_CODES.items() if _internal_doc_refs(m)}
            ck(f"결손 코드 뜻 문구에 수신자가 풀 수 없는 내부 좌표 없음({sorted(leaked)})", not leaked)
            ck("★음성대조: 옛 ATTESTATION_PARITY_ROWS 문구(코드맵 campaign_plane TL;DR 6)는 내부 좌표로 잡힌다",
               bool(_internal_doc_refs("대조 행이 없던 스모크 판본 · 코드맵 campaign_plane TL;DR 6")))
    except core.HintError as e:
        bad.append(f"evidence: 자체검사 중 예외 {e.code}: {e.message[:600]}")
    finally:
        builtins.open = io.open = real_open
        if prev is not None:
            os.environ["QUANT_MODEL_PATH"] = prev
    ck(f"★campaigns/ACTIVE 는 이 모듈에서 한 번도 열리지 않았다({active_hits[:2]})", not active_hits)
    return bad
