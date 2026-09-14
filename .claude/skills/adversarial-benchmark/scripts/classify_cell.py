#!/usr/bin/env python3
# classify_cell.py — 광의의 탐색 셀 종결 3분류 (plan_26090415 §4.6 · CP6)
#
#   serve_failed       서빙 자체가 성립하지 않았다        verdict 없음 · 연속 실패 카운터 ↑
#   measurement_void   서빙은 됐는데 **측정이 파괴**됐다  verdict 없음 · 연속 실패 카운터 ↑
#   measured           측정이 성립했다                    verdict 있음(PASS/REFUTE/서술)
#
# ★ 왜 `measurement_void` 를 따로 두는가. 워치독이 죽인 런을 REFUTE 로 적으면 지도에
#   **"이 레시피는 느리다"** 라는 거짓 진술이 남는다. 진실은 "호스트가 죽였다"이며, 이 작업의
#   유일한 계약(진실된 결과만 보인다)을 정면으로 어긴다. 두 사건은 원인도 처방도 다르다.
#
# ★ `void_reason` 은 자기추론이 아니라 **노드 블랙박스 이벤트와의 시각 대조**로 채운다.
#   대조에 실패하면 `unknown` 이며 추측으로 메우지 않는다(부재와 결측을 가른다).
#   `mode` 를 반드시 본다 — dry-run 트립은 **아무것도 죽이지 않았다**. 그것을 사살로 읽으면
#   관측 전용 인스턴스가 남긴 기록이 거짓 사인(死因)이 된다.
#
# ★ 허용오차 기본값 0 의 근거(U7 해소): 워치독과 러너는 **같은 호스트의 같은 시계**를 쓰므로
#   스큐가 없고, 셀의 [시작,끝] 구간이 이미 로드·측정·정리를 전부 감싼다. 넓힐 이유가 생기면
#   `--tolerance-s` 로 명시한다(조용히 넓히지 않는다). ⚠ 이 근거는 **같은 노드**의 events 에만 선다(아래 ★).
#
# ★ 2026-09-14(⑧ 분석 발견 T1 · plan_26091407 §9): **대조 대상 노드**를 manifest·측정 정체성으로 정한다.
#   종전 발견 규칙은 `docs/logs/*/events/*.jsonl` — 노드 필터 없이 **이 저장소에 있는** 모든 노드의 events 를
#   허용오차 0 으로 대조했다. 두 토폴로지에서 반대 방향으로 틀린다:
#     · multi: 서브(Ray 워커)의 사살은 분산 서빙을 죽이는데 그 events 는 **서브에 남는다**(각 노드가 자기 기록 ·
#       회수는 fetch_sub_docs → sync_staging/sub_docs/). 메인 저장소에서 보이지 않으니 판정점 위 절삭 레벨의
#       서브 사살이 miss 로 읽혀 bench_mode=full(**거짓 full** · full 인증서 발행 가능)이 됐다.
#     · single: 노드마다 독립 서빙이라 다른 노드의 사살은 이 측정의 사인이 아니다. 메인에 손으로 둔
#       docs/logs/<다른 노드> 가 창에 걸리면 거짓 강등이 된다.
#   규칙(판정 소유는 이 파일 · 토폴로지 사실은 manifest · 참여 노드 판정은 node_role_contract):
#     · single = 측정 노드 하나(sweep_index meta.measured_node · 셀 종결 모드는 이 노드)
#     · multi  = 서빙 참여 노드 전부(manifest nodes[].role = node_id · node_role_contract 위반 0)
#     · **이 노드가 누구인가**는 판정하지 않고 소유자에게 묻는다 — node_identity.sh --resolve(self_role → 유일한
#       role: main · 블랙박스가 docs/logs/<node_id> 에 쓰는 바로 그 해소기). 2026-09-14 리뷰 정정: 초판은 manifest
#       self_role 만 읽어, self_role 없는 manifest(sweep_bench 가 `defaulted(self_role absent)` 로 다루는 살아 있는 상태)
#       에서 계획이 서지 않았고 그때 **이 노드 자신의** 창 안 사살까지 버려 거짓 full 이 됐다(종전 glob 은 lite 였다).
#     · 계획이 서지 않아도(계약 위반·옛 index·측정 정체성 모순) 이 노드의 기록은 같은 호스트·같은 시계라 대조한다 —
#       정하지 못한 나머지는 `(미정)` 노드 하나로 not_scanned 에 남긴다(보지 못했음 · 추측으로 모든 노드를 보지 않는다).
#     · 이 노드(self)의 events = docs/logs/<node>/events · 다른 노드 = 회수본(sync_staging/sub_docs/logs/<node>/events ·
#       사고 회수로 손으로 둔 docs/logs/<node>/events 도 같은 노드의 기록으로 읽는다)
#   노드별 상태 → 구조 필드 `downgrade_correlation` 은 닫힌 어휘 그대로 두고 **노드별 사실은 `events_scan`** 이 든다:
#     matched(그 노드 창 안 집행 사살) > not_scanned(대조 대상 노드 중 events 0 · 회수본이 창 끝을 덮지 못함) >
#     unavailable(원격 회수본은 창을 덮지만 노드 간 시계 허용오차 미선언) > miss(대조 대상 노드 전부 봤고 사살 없음).
#   ★ 원격 노드의 허용오차는 **선언으로만** 온다(`--cross-node-tolerance-s N --cross-node-tolerance-source TEXT` ·
#     매직넘버 ✗ · 같은 시계 가정은 노드를 넘으면 성립하지 않는다). 선언이 없으면 창을 넓히지 않고 **미스를 결론내지
#     않는다**(unavailable). 창 안 매치는 선언 없이도 matched 다 — 오판 방향이 강등·인증서 억제(보수)이기 때문이다.
#   ★ 대조 불가(not_scanned·unavailable)는 **강등 트리거가 아니다**(사용자 결정 Q3·Q7 — 기계 이벤트만 · 부재는
#     이벤트가 아니다). bench_mode 는 runs[] 로 정해지고 인증서 규칙도 그대로이며, 불확실성은 `downgrade_correlation`·
#     `events_scan`·출처 서술(리포트 bench_mode 행)에 **기재**된다(기재 항목을 게이트로 격상 ✗). multi 에서 결론을
#     원하면 fetch_sub_docs.sh 로 회수한 뒤 `sweep_bench.sh <config> --reassemble-only` 로 판정 기록을 다시 쓴다.
#
# ★ 2026-09-14(plan_26091407 §4.4 · 사용자 결정 Q3·Q7): **bench_mode 확정**도 여기서 한다.
#   full bench 의 정의는 `lite ∪ GuideLLM × 반복 ≥3` 이고(`repeat_axis.FULL_REPEATS_MIN`), 반복이 성립하지
#   않은 셀은 lite 로 **강등**된다. 강등 트리거는 **기계 이벤트만**이다 — run 실패(measurement_ok=false)와
#   노드 블랙박스 kill 이벤트. 분산(재현 밴드 폭)은 강등 사유가 아니다(기재일 뿐).
#   `sweep_bench.sh` 는 raw `runs[]` 와 스윕이 멈춘 자리(**즉시 신호** `repetition.stop`)만 쓰고, 종료부에서
#   이 파일을 **판정 소유자로 부른다**(bench-mode 모드 · 판정 기록 `bench_mode.json`). 판정은 post-hoc 이며
#   `void_reason` 과 같은 모양이다 — 사인은 자기추론이 아니라 멈춘 자리의 시각 창과 블랙박스 이벤트의
#   **시각 대조**가 답한다. 규칙(2026-09-14 리뷰 정정 — 반복 조건의 범위는 `repeat_axis.py` 헤더 ★):
#     · 멈춘 자리(repeat-break · clamp)의 창 안에 **집행된** 사살(KILL_EVENT_KINDS) → lite · `blackbox_kill`
#       (레벨·run 순번과 무관 — kill 이벤트는 기계 이벤트 트리거다)
#     · 그 밖에 판정점 반복이 섰다 → full (경계 레벨의 반복 중단·첫 run 실패는 적응 상한 클램프)
#     · 판정점 반복이 끊겼다 → lite · `run_failed`(트립 단독·시각 불일치·이벤트 미관측 포함 — 관측된 신호 그대로)
#   대조를 했는지·못 했는지는 사유 문자열이 아니라 구조 필드 `downgrade_correlation` 이 든다.
#   ⚠ E2E 정상 경로(판정점 반복 완주 · 사살 없음)는 이 강등 경로를 밟지 않는다. 포화 경계에서 스윕이 멈추는
#     것은 정상 경로다(클램프). 강등 경로는 실패주입 자체검사(`selftest_sweep_repeats.py`)가 지킨다.
#
# 사용: classify_cell.py --serve-rc N --measure-rc N --started-utc T --ended-utc T
#         [--events PATH]... [--events-from-repo REPO] [--tolerance-s N] [--json]            (셀 종결 모드)
#       classify_cell.py --sweep-index PATH [--events PATH]... [--events-from-repo REPO]
#         [--tolerance-s N] [--write-bench-mode]                                              (bench-mode 모드)
#   두 모드는 섞지 않는다 — 셀 인자와 --sweep-index 를 함께 주면 exit 2.
# 종료: 0=분류 산출 · 2=인자 오류
import argparse
import datetime as _dt
import glob
import json
import os
import subprocess
import sys
import tempfile

OUTCOME_SERVE_FAILED = "serve_failed"
OUTCOME_MEASUREMENT_VOID = "measurement_void"
OUTCOME_MEASURED = "measured"
# 2026-09-05(G-B13): "측정하지 않았다" 는 "측정에 성공했다" 와 **다른 사실**이다. 종전에는
#   broad_search 가 `MEASURE_RC="${MEASURE_RC:-0}"` 로 둘을 같은 0 에 접었고, serve 가 성립한
#   재조립 경로에서 아무것도 재지 않고도 셀이 `measured` 로 종결될 수 있었다.
OUTCOME_NOT_MEASURED = "not_measured"

# 사살을 **실제로 수행한** 이벤트만 사인 후보다. `*_trip` 은 판정이고 `*_kill_ack` 이 집행이며,
# `*_trip_dryrun` 은 관측 전용이라 여기 없다(닫힌 목록 = tripwire).
KILL_EVENT_KINDS = ("watchdog_kill_ack", "thermal_kill_ack", "earlyoom_kill")
# 집행 직전의 판정. kill_ack 이 없을 때에만 보조 근거로 쓴다(예: 이벤트가 잘려 나간 경우).
TRIP_EVENT_KINDS = ("watchdog_trip", "thermal_trip")

# ── bench_mode 어휘 (2026-09-14 · plan_26091407 §4.4) — 닫힌 목록 · 소유는 이 파일 ─────────────────
#   소비자(render_report · render_sweep_map · 단계 ⑤ lite hint 통로)는 import 하거나 이 토큰을 읽는다.
#   ⚠ 이름을 `OUTCOME_` 로 시작하지 않는다 — campaign_init 자체검사가 그 접두사를 셀 결과 어휘로 긁는다.
BENCH_MODE_FULL = "full"
BENCH_MODE_LITE = "lite"
BENCH_MODES = (BENCH_MODE_FULL, BENCH_MODE_LITE)
DOWNGRADE_RUN_FAILED = "run_failed"
DOWNGRADE_BLACKBOX_KILL = "blackbox_kill"
DOWNGRADE_REASONS = (DOWNGRADE_RUN_FAILED, DOWNGRADE_BLACKBOX_KILL)
# **선언된 lite-only**(강등 아님)와 **강등된 lite** 를 가르는 규칙(단계 ⑤ 가 결정론으로 읽는다):
#   bench_mode=lite ∧ downgrade_reason ∈ DOWNGRADE_REASONS            → 강등된 lite(full 을 시도했다)
#   bench_mode=lite ∧ downgrade_reason=null ∧ source 가 "declared(" 로 시작 → 선언된 lite-only
#   그 밖의 lite ∧ null                                              → 판정 불가(규칙 밖 기록 — 추측 ✗)
# 이 파일이 내는 lite 는 **언제나 강등**이다(사유 필수). 선언된 lite-only 기록은 lite 통로(단계 ⑤)가
# 같은 모양(`bench_mode_source="declared(...)"`)으로 적는다.
BENCH_MODE_SOURCE_DECLARED_PREFIX = "declared("
BENCH_MODE_RECORD_NAME = "bench_mode.json"   # 스윕 디렉터리 사이드카 — 판정 기록의 지속 자리
# 사살 대조를 했는가(구조 필드 · 단계 ⑤ 가 "보고도 없었다" 와 "보지 않았다" 를 문자열 파싱 없이 가른다):
#   matched=대조 대상 노드 창 안 집행 사살 있음 · miss=대조 대상 노드 전부의 이벤트를 봤고 창 안 집행 사살 없음
#   not_scanned=대조 대상 노드 중 이 창을 덮는 기록을 판독하지 못한 노드가 있다(events 0 · 원격 미회수 · 회수본이 창 끝 이전)
#   unavailable=창 시각을 읽지 못했다 · 원격 노드 시계 허용오차가 선언되지 않아 창을 그 노드 시계로 옮기지 못했다
#   not_applicable=스윕이 멈춘 자리가 없다(대조할 창이 없다)
CORRELATION_MATCHED = "matched"
CORRELATION_MISS = "miss"
CORRELATION_NOT_SCANNED = "not_scanned"
CORRELATION_UNAVAILABLE = "unavailable"
CORRELATION_NOT_APPLICABLE = "not_applicable"
DOWNGRADE_CORRELATIONS = (CORRELATION_MATCHED, CORRELATION_MISS, CORRELATION_NOT_SCANNED,
                          CORRELATION_UNAVAILABLE, CORRELATION_NOT_APPLICABLE)
# 노드 블랙박스 events 의 자리 — 발견 규칙을 호출부(broad_search · sweep_bench)마다 다시 적지 않게 여기 한 곳에 둔다.
#   이 노드: docs.md §기계판독 데이터 평면 `docs/logs/<node_id>/events/<YYYY-MM>.jsonl`
#   다른 노드: fetch_sub_docs.sh 의 회수 미러(DEST 기본값 `sync_staging/sub_docs` = 서브 docs/ 사본) 아래 같은 모양.
#   ⚠ 미러 자리 리터럴은 relay.read_brief · campaign_template_validator.discover_briefs 와 공유한다 — 셋 다
#     fetch_sub_docs.sh DEST 기본값의 소비자다(교차검증: 이 파일 --self-test K-mirror).
EVENTS_LOCAL_PARTS = ("docs", "logs")
EVENTS_RECOVERED_PARTS = ("sync_staging", "sub_docs", "logs")
# sweep_bench meta.measured_node 어휘(main|sub|cluster · hint 노드 축과 같은 철자) 중 분산 쌍의 정체성.
MEASURED_NODE_CLUSTER = "cluster"
# 노드별 대조 상태(`events_scan.nodes[].status` · 닫힌 목록). 집계 규칙은 `correlate_nodes`.
NODE_SCAN_MATCHED = "matched"
NODE_SCAN_MISS = "miss"
NODE_SCAN_NOT_SCANNED = "not_scanned"
NODE_SCAN_NOT_COVERED = "not_covered"
NODE_SCAN_SKEW_UNDECLARED = "skew_undeclared"
NODE_SCAN_STATUSES = (NODE_SCAN_MATCHED, NODE_SCAN_MISS, NODE_SCAN_NOT_SCANNED, NODE_SCAN_NOT_COVERED,
                      NODE_SCAN_SKEW_UNDECLARED)
UNIDENTIFIED_NODE = "(미정)"   # 노드 계획이 서지 않아 정하지 못한 대조 대상의 자리표(경로 성분이 아니다 — 판독하지 않는다)


def _path_segment(value):
    """경로 성분 하나로 쓸 수 있는 이름인가(node_id · 통로 이름). 형식 판정은 소유자(node_identity.sh)의 몫이고
    여기서는 경로 탈출만 막는다(`..`·구분자 ✗)."""
    return isinstance(value, str) and bool(value) and os.path.basename(value) == value and not value.startswith(".")


def _node_role_contract():
    """multi 참여 노드 판정의 소유자(terraforming_node/scripts/node_role_contract.py)를 **코드 옆 경로**에서 적재한다.
    multi 벤치는 메인에서만 돈다(ray-worker tool_plane = 0종) — 없으면 판정하지 않는다(None)."""
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "terraforming_node", "scripts",
                        "node_role_contract.py")
    if not os.path.isfile(path):
        return None
    import importlib.util
    try:
        spec = importlib.util.spec_from_file_location("_classify_cell_node_role_contract", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
    except Exception:  # noqa: BLE001 — 적재 실패는 판정 불가(아래 undeterminable)이지 크래시가 아니다
        return None
    return module


def _node_identity_resolver():
    """node_id 단일 해소기(`node_identity.sh`)의 자리 — **소유자 정본 → 서브 런타임 배달분** 순(run_trial `_node_tool_path`
    와 같은 두 후보 · 서브는 `.claude/runtime/node_blackbox/` 로 배달받는다). 코드 옆 경로로 찾는다(`_node_role_contract` 와 같은 규율)."""
    here = os.path.dirname(os.path.abspath(__file__))
    candidates = (os.path.join(here, "..", "..", "terraforming_node", "scripts", "node_blackbox", "node_identity.sh"),
                  os.path.join(here, "..", "..", "..", "runtime", "node_blackbox", "node_identity.sh"))
    return next((os.path.normpath(c) for c in candidates if os.path.isfile(c)), None)


def resolve_self_node(repo):
    """이 저장소가 놓인 노드의 node_id → (node | None, 출처 또는 사유). **각자 파싱하지 않는다**(terraforming_node SKILL.md
    §2.7.6) — 해소기 CLI(`node_identity.sh --resolve --repo`)를 부른다: manifest self_role → 유일한 `role: main`.
    블랙박스가 events 를 쓰는 `docs/logs/<node_id>` 가 바로 이 해소기의 답이므로, 대조가 읽는 자리와 기록이 쓰인 자리가 갈라지지 않는다."""
    if not repo:
        return None, "repo 미지정"
    resolver = _node_identity_resolver()
    if resolver is None:
        return None, "node_identity.sh 부재(terraforming_node/scripts/node_blackbox · .claude/runtime/node_blackbox)"
    try:
        proc = subprocess.run(["bash", resolver, "--resolve", "--repo", repo], capture_output=True, text=True,
                              timeout=60, check=False)
    except (OSError, subprocess.SubprocessError) as exc:
        return None, "node_identity.sh 실행 실패: %s" % exc
    node = (proc.stdout or "").strip()
    if proc.returncode != 0 or not _path_segment(node):
        # 해소기의 fail-loud 사유를 그대로 싣는다(삼키면 침묵 폴백이 된다).
        why = " / ".join(line.strip() for line in (proc.stderr or "").splitlines()[:2] if line.strip())
        return None, "node_identity.sh --resolve rc=%s%s" % (proc.returncode, (" — " + why) if why else "")
    return node, "node_identity.sh --resolve(self_role → 유일한 role: main)"


def events_node_plan(topology, measured_node=None, measured_node_source=None, manifest_path=None, repo=None):
    """대조 대상 노드 계획. 순수 판정 + manifest 판독 + 노드 정체성 해소기 호출. 반환 dict — `undeterminable` 이 None 이
    아니면 계획이 서지 않았다(그래도 `self_node` 가 있으면 그 노드의 기록은 대조한다 · `scan_node_events`).

    single: 측정 노드 하나 — `measured_node`(sweep meta) 우선, 없으면 이 노드(`resolve_self_node`).
    multi : manifest `nodes[].role` 전부(node_role_contract 위반 0 · 역할 중복 ✗) · self = `resolve_self_node`.
    """
    plan = {"topology": topology, "nodes": [], "self_node": None, "self_node_source": None, "source": None,
            "undeterminable": None, "excluded_rule": None, "manifest": manifest_path}
    identity = {}

    def self_identity():
        if "node" not in identity:
            identity["node"], identity["source"] = resolve_self_node(repo)
        return identity["node"], identity["source"]

    def bad(reason):
        plan["undeterminable"] = reason
        # 계획이 서지 않아도 **이 노드**의 기록은 같은 호스트·같은 시계다 — 버리면 로컬 사살이 not_scanned 로 새어
        #   거짓 full 이 된다(2026-09-14 리뷰 정정). 정하지 못한 나머지는 scan 단계가 `(미정)` 으로 남긴다.
        node, src = self_identity()
        plan["self_node"] = node if _path_segment(node) else None
        plan["self_node_source"] = src
        return plan

    if topology == "single":
        if measured_node is not None:
            node = measured_node
            src = "sweep_index.meta.measured_node=%s(%s)" % (measured_node, measured_node_source or "출처 미기재")
        else:
            node, why = self_identity()
            src = "이 노드 — %s" % why
        if node == MEASURED_NODE_CLUSTER or not _path_segment(node):
            return bad("single 인데 측정 노드를 정하지 못했다(%r ← %s)" % (node, src))
        plan.update(nodes=[node], self_node=node, self_node_source=src, source="single: 측정 노드만 — " + src,
                    excluded_rule=("single 의 노드는 각자 독립 서빙이다 — 다른 노드(docs/logs/<다른 노드>)의 사살은 이 측정의 "
                                   "사인이 아니므로 대조하지 않는다"))
        return plan
    if topology == "multi":
        if measured_node is not None and measured_node != MEASURED_NODE_CLUSTER:
            return bad("multi 인데 measured_node=%r — 분산 서빙의 측정 정체성은 쌍(%s)이다"
                       % (measured_node, MEASURED_NODE_CLUSTER))
        contract = _node_role_contract()
        if contract is None:
            return bad("node_role_contract 부재 — multi 참여 노드를 판정할 소유자가 없다")
        if not manifest_path or not os.path.isfile(manifest_path):
            return bad("manifest 부재(%s) — 참여 노드를 읽을 사실이 없다" % manifest_path)
        try:
            doc = contract.load_yaml(manifest_path)
        except Exception as exc:  # noqa: BLE001
            return bad("manifest 판독 실패(%s): %s" % (manifest_path, exc))
        if not isinstance(doc, dict):
            return bad("manifest 가 객체가 아니다(%s)" % manifest_path)
        result = contract.evaluate_manifest(doc, topology="multi")
        if result.get("violations"):
            return bad("node_role_contract 위반: %s" % "; ".join(v.get("code", "?") for v in result["violations"]))
        roles = [n.get("role") for n in (doc.get("nodes") or []) if isinstance(n, dict)]
        if not roles or len(set(roles)) != len(roles) or not all(_path_segment(r) for r in roles):
            return bad("manifest nodes[].role 로 참여 노드를 정할 수 없다(%r — node_id = role 슬러그 · 비었거나 중복)" % roles)
        self_node, self_src = self_identity()
        if self_node not in roles:
            return bad("이 노드(%r ← %s)가 manifest nodes[].role(%s)에 없다 — 어느 노드의 events 가 이 저장소의 것인지 모른다"
                       % (self_node, self_src, ",".join(roles)))
        plan.update(nodes=roles, self_node=self_node, self_node_source=self_src,
                    source="multi: 서빙 참여 노드 전부 — manifest nodes[].role(%s) · node_role_contract 위반 0"
                           % ",".join(roles))
        return plan
    return bad("topology=%r — 대조 대상 노드 규칙이 없다(single|multi)" % (topology,))


def scan_node_events(repo, plan):
    """계획의 노드마다 events 파일을 찾아 읽는다. 반환 = 노드별 scan dict 목록.

    계획이 서지 않았으면 이 노드(`plan.self_node` · 해소기의 답)의 로컬 기록만 읽고, 정하지 못한 나머지를
    `(미정)` 항목 하나로 붙인다 — 이 노드에서 창 안 사살이 보이면 matched, 아니면 not_scanned 다(miss 로 접지 않는다)."""
    scans = []
    if not isinstance(plan, dict):
        return scans

    def read(node, remote):
        locations = ((EVENTS_RECOVERED_PARTS, EVENTS_LOCAL_PARTS) if remote else (EVENTS_LOCAL_PARTS,))
        paths = []
        for parts in locations:
            paths += sorted(p for p in glob.glob(os.path.join(repo, *parts, node, "events", "*.jsonl"))
                            if os.path.isfile(p))
        lines, got = _read_events(paths)
        return {"node": node, "remote": remote, "files": got, "lines": lines, "repo": repo}

    if plan.get("undeterminable"):
        if _path_segment(plan.get("self_node")):
            scans.append(dict(read(plan["self_node"], False), fallback=True))
        scans.append({"node": UNIDENTIFIED_NODE, "remote": True, "unidentified": True, "files": [], "lines": [],
                      "repo": repo})
        return scans
    for node in plan.get("nodes") or []:
        scans.append(read(node, node != plan.get("self_node")))
    return scans


def _newest_event_ts(lines):
    newest = None
    for line in lines:
        try:
            event = json.loads(line)
            ts = _utc(event.get("ts"), "event.ts") if isinstance(event, dict) else None
        except (ValueError, ClassifyError):
            continue
        if ts is not None and (newest is None or ts > newest):
            newest = ts
    return newest


def _iso(ts):
    return ts.strftime("%Y-%m-%dT%H:%M:%SZ") if ts is not None else None


def correlate_nodes(node_scans, lo, hi, tolerance_s=0, cross_node_tolerance_s=None, cross_node_tolerance_source=None):
    """노드별 창 대조 → (hits, downgrade_correlation, per_node). 순수 함수(파일 I/O 없음 — scan 은 이미 읽힌 줄을 든다)."""
    hits, per_node = [], []
    for scan in node_scans:
        remote = bool(scan.get("remote"))
        declared = cross_node_tolerance_s is not None
        tol = (cross_node_tolerance_s if declared else 0) if remote else tolerance_s
        repo = scan.get("repo")
        if scan.get("unidentified"):
            tol_source = "n/a(대조 대상 미정 — 판독하지 않았다)"
        elif remote:
            tol_source = (("declared(%s)" % cross_node_tolerance_source) if declared
                          else "undeclared(노드 간 시계 허용오차 미선언 — 창을 넓히지 않고 미스를 결론내지 않는다)")
        else:
            tol_source = "same-node-clock(--tolerance-s)"
        entry = {"node": scan.get("node"), "remote": remote,
                 "files": [os.path.relpath(p, repo) if repo else p for p in scan.get("files") or []],
                 "tolerance_s": tol, "tolerance_source": tol_source}
        if scan.get("fallback"):
            entry["fallback"] = True
        if scan.get("unidentified"):
            entry["status"] = NODE_SCAN_NOT_SCANNED
            entry["detail"] = "노드 계획 불성립 — 이 노드 밖 대조 대상을 정하지 못했다(보지 못했음)"
        elif not scan.get("files"):
            entry["status"] = NODE_SCAN_NOT_SCANNED
            entry["detail"] = ("회수본 없음 — fetch_sub_docs.sh 로 회수한 뒤 판정 기록을 다시 쓴다" if remote
                               else "블랙박스 events 파일 0")
        else:
            node_hits = kill_events_in_window(scan.get("lines") or [], lo, hi, tol)
            for hit in node_hits:
                hit["node"] = scan.get("node")
            hits.extend(node_hits)
            if any(h["kind"] in KILL_EVENT_KINDS for h in node_hits):
                entry["status"] = NODE_SCAN_MATCHED
                if remote:
                    # 원격 매치의 창이 선언된 허용오차로 넓어졌는지를 출처 서술에 드러낸다 — 미선언이면 창을 넓히지 않은
                    #   매치(창 안 사살)만 인정한 것이다(비대칭 · 오판 방향이 강등·인증서 억제라 보수 · plan_26091407 §9 결정 대기).
                    entry["detail"] = (("허용오차 %ss declared" % tol) if declared
                                       else "허용오차 미선언 — 0s · 창 안 매치만 인정")
            elif remote:
                newest = _newest_event_ts(scan.get("lines") or [])
                edge = hi + _dt.timedelta(seconds=tol)
                entry["newest_event_utc"] = _iso(newest)
                if newest is None or newest < edge:
                    entry["status"] = NODE_SCAN_NOT_COVERED
                    entry["detail"] = ("회수본의 최신 이벤트 %s < 창 끝+허용오차 %s — 이 창을 덮는 기록이 아니다(회수가 이르다)"
                                       % (_iso(newest), _iso(edge)))
                elif not declared:
                    entry["status"] = NODE_SCAN_SKEW_UNDECLARED
                    entry["detail"] = "회수본은 창을 덮지만 노드 간 시계 허용오차가 선언되지 않았다"
                else:
                    entry["status"] = NODE_SCAN_MISS
            else:
                entry["status"] = NODE_SCAN_MISS
        per_node.append(entry)
    hits.sort(key=lambda h: h["ts"])
    statuses = [e["status"] for e in per_node]
    if NODE_SCAN_MATCHED in statuses:
        correlation = CORRELATION_MATCHED
    elif not per_node or NODE_SCAN_NOT_SCANNED in statuses or NODE_SCAN_NOT_COVERED in statuses:
        correlation = CORRELATION_NOT_SCANNED
    elif NODE_SCAN_SKEW_UNDECLARED in statuses:
        correlation = CORRELATION_UNAVAILABLE
    else:
        correlation = CORRELATION_MISS
    return hits, correlation, per_node


def events_scan_summary(scan_doc):
    """판정 기록의 `events_scan` → 한 줄 서술(리포트·스윕 로그 공용 · 어휘 소유자가 만든다). 없으면 None."""
    if not isinstance(scan_doc, dict):
        return None
    plan = scan_doc.get("plan") if isinstance(scan_doc.get("plan"), dict) else {}
    head = ("대조 노드 미정(%s)" % plan["undeterminable"]) if plan.get("undeterminable") else None
    nodes = scan_doc.get("nodes")
    if not isinstance(nodes, list):
        if head:
            return head
        return "대조 노드 계획[%s] · 대조할 창 없음" % ",".join(plan.get("nodes") or []) if plan.get("nodes") else None
    parts = []
    for entry in nodes:
        if not isinstance(entry, dict):
            continue
        text = "%s%s=%s" % (entry.get("node"), "(원격)" if entry.get("remote") else "", entry.get("status"))
        if entry.get("status") != NODE_SCAN_MISS and entry.get("detail"):
            text += "(%s)" % entry["detail"]
        parts.append(text)
    body = "대조 노드[%s]" % " · ".join(parts) if parts else "대조 노드 0"
    return "%s · %s" % (head, body) if head else body


def bench_mode_record_path(sweep_index_path):
    """판정 기록의 관례 자리 — sweep_index 옆 사이드카."""
    return os.path.join(os.path.dirname(os.path.abspath(sweep_index_path)), BENCH_MODE_RECORD_NAME)


def read_bench_mode_record(sweep_index_path, index_doc, record_path=None):
    """판정 기록 판독(소비자 공용 — render_report · publish_benchmark_record · broad_search).

    → (record | None, status). status: `ok` · `absent(…)` · `unreadable(…)` · `stale(…)`.
    stale = 기록의 `sweep_index_generated_utc` 가 지금 index 의 측정시각과 다르다(같은 스윕 디렉터리에 나중 측정이
    덮였다) — 낡은 분류를 싣지 않는다. 부재·판독 실패·낡음을 full 로도 lite 로도 접지 않는다."""
    path = record_path or bench_mode_record_path(sweep_index_path)
    name = os.path.basename(path)
    if not os.path.isfile(path):
        return None, "absent(%s 부재 — 이 스윕을 분류한 기록이 없다)" % name
    try:
        with open(path, encoding="utf-8") as handle:
            record = json.load(handle)
    except (OSError, ValueError) as exc:
        return None, "unreadable(%s 판독 실패: %s)" % (name, exc)
    if not isinstance(record, dict):
        return None, "unreadable(%s 가 객체가 아니다)" % name
    index_utc = index_doc.get("generated_utc") if isinstance(index_doc, dict) else None
    if record.get("sweep_index_generated_utc") != index_utc:
        return None, ("stale(%s 는 다른 측정(generated_utc=%s)의 분류다 · 이 스윕 %s)"
                      % (name, record.get("sweep_index_generated_utc"), index_utc))
    return record, "ok"


def write_bench_mode_record(sweep_index_path, record):
    """판정 기록을 **원자적으로** 쓴다(임시 파일 → os.replace). 반쯤 쓰인 기록을 소비자가 읽지 않게 한다."""
    out = bench_mode_record_path(sweep_index_path)
    fd, tmp = tempfile.mkstemp(prefix=".%s." % BENCH_MODE_RECORD_NAME, dir=os.path.dirname(out))
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            json.dump(record, handle, ensure_ascii=False, indent=2)
            handle.write("\n")
        os.replace(tmp, out)
    except BaseException:
        if os.path.exists(tmp):
            os.unlink(tmp)
        raise
    return out


def bench_mode_kind(record):
    """기록 → 'full' | 'declared-lite' | 'downgraded-lite' | 'undetermined'. 순수 함수(단계 ⑤ 판독 규칙)."""
    if not isinstance(record, dict):
        return "undetermined"
    mode, reason = record.get("bench_mode"), record.get("downgrade_reason")
    source = record.get("bench_mode_source") or ""
    if mode == BENCH_MODE_FULL and reason is None:
        return "full"
    if mode == BENCH_MODE_LITE and reason in DOWNGRADE_REASONS:
        return "downgraded-lite"
    if (mode == BENCH_MODE_LITE and reason is None
            and isinstance(source, str) and source.startswith(BENCH_MODE_SOURCE_DECLARED_PREFIX)):
        return "declared-lite"
    return "undetermined"


class ClassifyError(ValueError):
    """분류가 성립하지 않는 입력."""


def _utc(value, field):
    if not isinstance(value, str) or not value.strip():
        raise ClassifyError("%s 는 ISO-8601 UTC 문자열이어야 한다(받은 값: %r)" % (field, value))
    text = value.strip()
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    try:
        parsed = _dt.datetime.fromisoformat(text)
    except ValueError as exc:
        raise ClassifyError("%s 를 시각으로 읽을 수 없다(%r): %s" % (field, value, exc)) from exc
    if parsed.tzinfo is None:
        raise ClassifyError("%s 에 타임존이 없다(%r)" % (field, value))
    return parsed.astimezone(_dt.timezone.utc)


def kill_events_in_window(event_lines, started, ended, tolerance_s=0):
    """구간 안의 **집행된** 사살 이벤트. dry-run 은 제외한다(아무것도 죽이지 않았다)."""
    lo = started - _dt.timedelta(seconds=tolerance_s)
    hi = ended + _dt.timedelta(seconds=tolerance_s)
    hits = []
    for line in event_lines:
        line = line.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except ValueError:
            continue                      # 손상 줄은 건너뛴다 — 없는 것으로 읽지 않고 아래 참조
        if not isinstance(event, dict):
            continue
        kind = event.get("kind")
        if kind not in KILL_EVENT_KINDS and kind not in TRIP_EVENT_KINDS:
            continue
        if event.get("mode") == "dry-run":
            continue                      # 관측 전용 — 사인이 될 수 없다
        try:
            ts = _utc(event.get("ts"), "event.ts")
        except ClassifyError:
            continue
        if lo <= ts <= hi:
            hits.append({"ts": event.get("ts"), "kind": kind,
                         "source": event.get("source"), "mode": event.get("mode"),
                         "targets": event.get("targets")})
    hits.sort(key=lambda h: h["ts"])
    return hits


def _miss_source(correlation, events_scanned):
    """사살을 찾지 못한 대조의 출처 서술. 대조 **했는데** 없었다(miss)와 **보지 못했다**(not_scanned·unavailable)를
    접두사로 가른다 — broad_search 는 `events(` 만 사인으로 쓰므로 이 구분은 사인 칸을 바꾸지 않고 출처만 정직하게 만든다."""
    prefix = {CORRELATION_NOT_SCANNED: "not-scanned", CORRELATION_UNAVAILABLE: "correlation-unavailable"}.get(
        correlation, "correlation-miss")
    return "%s(%s)" % (prefix, events_scanned)


def classify(serve_rc, measure_rc, kill_hits, events_scanned, correlation=None):
    """rc 두 개 + 이벤트 대조 → 종결 분류. 순수 함수.

    `measure_rc is None` = **측정 단계에 들어가지 않았다**(부재). 성공(0)과 구분한다.
    `correlation`(선택) = `correlate_nodes` 의 집계 — 주면 사살 부재의 출처가 miss/not-scanned/unavailable 로 갈린다.
    """
    executed = [h for h in kill_hits if h["kind"] in KILL_EVENT_KINDS]
    unseen = correlation in (CORRELATION_NOT_SCANNED, CORRELATION_UNAVAILABLE)
    if serve_rc != 0:
        # ★ 2026-09-11(plan_26091108 R4): 이 분기가 **사살을 무시했다**. 종전에는 구간 안에
        #   집행된 사살이 있어도 `void_reason: None` 을 내고 사유 칸을 비웠고, 그 빈자리를
        #   사람의 산문("fused_moe FP8 config 부재 추정 hang")이 메웠다 — camp-26090918 의
        #   void 3건이 그렇게 기록됐고, 그 서술이 근거 없는 체크포인트 재다운로드로 이어졌다.
        #   사인은 추정이 아니라 **시각 대조**가 답한다. 분류(serve_failed)는 바꾸지 않는다 —
        #   서빙은 실제로 성립하지 않았다. 바뀌는 것은 **왜** 다.
        if executed:
            first = executed[0]
            return {
                "cell_outcome": OUTCOME_SERVE_FAILED,
                "void_reason": first["kind"],
                "void_reason_source": "events(%s)" % events_scanned,
                "kill_events": kill_hits,
                "note": "서빙이 성립하지 않았다(rc=%s). 구간 안에서 %s 가 %s 에 **집행**됐다 — "
                        "이 죽음은 모델·빌드 평면의 구동불가가 아니라 호스트 평면의 사살이다."
                        % (serve_rc, first["kind"], first["ts"]),
            }
        return {
            "cell_outcome": OUTCOME_SERVE_FAILED,
            "void_reason": None,
            "void_reason_source": _miss_source(correlation, events_scanned),
            "kill_events": kill_hits,
            "note": "서빙이 성립하지 않아 측정 단계에 도달하지 않았다(rc=%s). %s — 사인을 추측하지 않는다."
                    % (serve_rc, "대조 대상 노드 중 구간을 보지 못한 노드가 있다" if unseen
                       else "구간 안에 집행된 사살 이벤트는 없다"),
        }

    if measure_rc is None:
        return {
            "cell_outcome": OUTCOME_NOT_MEASURED,
            "void_reason": None,
            "void_reason_source": None,
            "kill_events": kill_hits,
            "note": "서빙은 성립했으나 측정 단계에 들어가지 않았다(rc 부재) — 성공으로 집계하지 않는다.",
        }
    if measure_rc != 0:
        if executed:
            first = executed[0]
            return {
                "cell_outcome": OUTCOME_MEASUREMENT_VOID,
                "void_reason": first["kind"],
                "void_reason_source": "events(%s)" % events_scanned,
                "kill_events": kill_hits,
                "note": "서빙은 성립했으나 %s 가 %s 에 집행되어 측정이 파괴됐다."
                        % (first["kind"], first["ts"]),
            }
        return {
            "cell_outcome": OUTCOME_MEASUREMENT_VOID,
            "void_reason": "unknown",
            # 대조를 **했는데** 못 찾은 것과 대조를 안 한 것은 다르다. 후자를 unknown 으로
            # 적으면 나중에 "이벤트를 봤나"를 알 수 없다.
            "void_reason_source": _miss_source(correlation, events_scanned),
            "kill_events": kill_hits,
            "note": "서빙은 성립했으나 측정이 실패했고(rc=%s) %s — 사인을 추측하지 않는다."
                    % (measure_rc, "대조 대상 노드 중 구간을 보지 못한 노드가 있다" if unseen
                       else "구간 안에 집행된 사살 이벤트가 없다"),
        }

    result = {
        "cell_outcome": OUTCOME_MEASURED,
        "void_reason": None,
        "void_reason_source": None,
        "kill_events": kill_hits,
        "note": "측정 성립.",
    }
    if executed:
        # 측정은 성공했는데 같은 구간에 사살이 있었다 — 다른 컨테이너였을 수 있으므로 분류를
        # 바꾸지 않는다. 그러나 **보이게** 남긴다(침묵 금지). 사람이 판단할 사실이다.
        result["note"] = ("측정 성립. 다만 같은 구간에 집행된 사살 이벤트가 %d건 있다 — "
                          "대상이 이 셀이 아니었을 수 있으나 기록으로 남긴다." % len(executed))
    return result


def _bench_mode_record(mode, source, reason=None, reason_source=None, repetition=None, kill_hits=None,
                       correlation=CORRELATION_NOT_APPLICABLE, events_scan=None):
    return {"bench_mode": mode, "bench_mode_source": source,
            "downgrade_reason": reason, "downgrade_reason_source": reason_source,
            "downgrade_correlation": correlation, "events_scan": events_scan,
            "repetition": repetition, "repetition_kill_events": kill_hits or []}


def declared_lite_record(detail):
    """**선언된 lite-only** 판정 기록 — lite 통로(단계 ⑤ · `render_report.py --lite-only`)가 쓰는 모양.

    이 파일이 내는 lite 는 언제나 강등(사유 필수)이고, 선언된 lite-only 는 full 을 시도하지 않은 셀이라 강등 사유가
    없다. 그 기록의 **모양**(키 집합 · `declared(` 출처 접두사 · 대조 not_applicable)은 어휘 소유자인 여기서 만든다 —
    소비자가 손으로 dict 를 지으면 `bench_mode_kind` 가 읽는 규칙과 조용히 갈라진다(2026-09-14 · plan_26091407 §4.5)."""
    text = str(detail or "").strip()
    if not text:
        raise ValueError("declared_lite_record: 선언 출처 설명이 비었다 — 누가 lite 만 재기로 했는지 적는다")
    return _bench_mode_record(BENCH_MODE_LITE, "%s%s)" % (BENCH_MODE_SOURCE_DECLARED_PREFIX, text))


def classify_bench_mode(index, event_lines, events_scanned, tolerance_s=0, node_scans=None, plan=None,
                        cross_node_tolerance_s=None, cross_node_tolerance_source=None):
    """sweep_index(대표 run 의 runs[] · repetition.clamp_run 포함) + 블랙박스 이벤트 → bench_mode 확정.
    순수 함수(파일 I/O 없음). `events_scanned` = 판독한 events 파일 목록(또는 쉼표 문자열 · "none"/빈 값 = 0개).
    `node_scans`(선택) = `scan_node_events` 의 노드별 판독분 — 주면 노드별로 대조한다(`plan` 은 기록에 싣는다).
    없으면 `event_lines` 를 호출자가 선언한 한 묶음(같은 시계)으로 대조한다(`--events` 명시 경로 · 종전 규약).

    판정 순서(post-hoc):
      ① 반복 요약은 `repeat_axis.summarize` 가 raw `levels[].measured.runs` 에서 **다시 계산**한다(index 의 요약을
         믿지 않는다 · 클램프 경계 사실만 index `repetition.clamp_run` 에서 받는다 — 그 레벨은 levels 에 없다)
      ② 반복 조건(`repeat_axis.repetition_established` · 판정점 범위)이 판정 불가면 판정하지 않는다(null)
      ③ 스윕이 멈춘 자리가 있으면 그 창과 이벤트를 대조한다 — 집행된 사살이 있으면 lite · blackbox_kill
      ④ 그 밖: 반복 조건 성립 → full · 불성립 → lite · run_failed(대조 결과는 downgrade_correlation 에)
    """
    try:
        sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
        import repeat_axis as _ra
    except ImportError as exc:   # 배선 결함 — 부재를 full/lite 로 접지 않는다
        return _bench_mode_record(None, "undeterminable(repeat_axis 적재 실패: %s)" % exc)
    if not isinstance(index, dict) or not isinstance(index.get("levels"), list) or not index["levels"]:
        return _bench_mode_record(None, "not_evaluated(sweep_index levels 부재 — 측정 레벨이 없다)")
    declared = index.get("repetition") if isinstance(index.get("repetition"), dict) else {}
    summ = _ra.summarize(index["levels"], requested=declared.get("requested"),
                         requested_source=declared.get("requested_source"), kind=declared.get("kind"),
                         clamp_run=declared.get("clamp_run"))
    established, why = _ra.repetition_established(summ, index.get("verdict_point_level"))
    if established is None:
        return _bench_mode_record(None, why, repetition=summ)

    if isinstance(events_scanned, (list, tuple)):
        scanned_txt = ",".join(events_scanned) if events_scanned else "none"
    else:
        scanned_txt = events_scanned or "none"
    if node_scans is None:   # 호출자가 선언한 한 묶음(같은 시계) — 종전 규약
        node_scans = [{"node": "explicit", "remote": False, "lines": list(event_lines or []),
                       "files": [] if scanned_txt == "none" else scanned_txt.split(",")}]
    scan_doc = {"plan": plan if isinstance(plan, dict) else {"source": "explicit(호출자가 대상 events 를 선언했다 — 노드 계획 없음)"},
                "nodes": None, "cross_node_tolerance_s": cross_node_tolerance_s,
                "cross_node_tolerance_source": cross_node_tolerance_source}
    stop = summ.get("stop")
    hits, correlation, window, signal = [], CORRELATION_NOT_APPLICABLE, None, None
    if isinstance(stop, dict):
        signal = "stop[%s · level=%s · run=%s](%s)" % (stop.get("kind"), stop.get("level"), stop.get("run"),
                                                      stop.get("detail"))
        window = "[%s, %s] tol=%ss" % (stop.get("window_start_utc"), stop.get("window_end_utc"), tolerance_s)
        try:
            lo = _utc(stop.get("window_start_utc"), "stop.window_start_utc")
            hi = _utc(stop.get("window_end_utc"), "stop.window_end_utc")
        except ClassifyError as exc:
            correlation, window = CORRELATION_UNAVAILABLE, "창 시각 판독 불가: %s" % exc
        else:
            hits, correlation, scan_doc["nodes"] = correlate_nodes(node_scans, lo, hi, tolerance_s,
                                                                    cross_node_tolerance_s, cross_node_tolerance_source)
    scope = events_scan_summary(scan_doc) if isinstance(plan, dict) else None
    executed = [h for h in hits if h["kind"] in KILL_EVENT_KINDS]
    if executed:
        first = executed[0]
        return _bench_mode_record(
            BENCH_MODE_LITE,
            "%s · 멈춘 자리 창 안에 집행된 사살 — kill 이벤트는 레벨·run 순번과 무관한 강등 트리거" % why,
            DOWNGRADE_BLACKBOX_KILL,
            "events(%s) · %s @ %s%s ∈ %s · %s%s" % (scanned_txt, first["kind"], first["ts"],
                                                  (" · node=%s" % first["node"]) if isinstance(plan, dict) else "",
                                                  window, signal, (" · " + scope) if scope else ""),
            repetition=summ, kill_hits=hits, correlation=correlation, events_scan=scan_doc)
    note = ""
    scope_tail = (" · " + scope) if scope else ""
    if correlation == CORRELATION_MISS:
        trips = len(hits)
        note = "correlation-miss(%s · 창 %s%s%s)" % (scanned_txt, window,
                                                    (" · 트립 %d건은 집행이 아니라 사인 불충분" % trips) if trips else "",
                                                    scope_tail)
    elif correlation == CORRELATION_NOT_SCANNED:
        if isinstance(plan, dict):
            note = ("not-scanned(대조 대상 노드 중 이 창을 덮는 기록을 판독하지 못한 노드가 있다 — 보지 못했음이지 사살 없음이 "
                    "아니다 · 창 %s%s)" % (window, scope_tail))
        else:
            note = "not-scanned(판독한 블랙박스 events 파일 0 — 사살 여부를 보지 못했다 · 창 %s)" % window
    elif correlation == CORRELATION_UNAVAILABLE:
        if isinstance(stop, dict) and scan_doc.get("nodes") is not None:
            note = ("correlation-unavailable(원격 노드 시계 허용오차 미선언 — 미스를 결론내지 않는다 · 창 %s%s)"
                    % (window, scope_tail))
        else:
            note = "correlation-unavailable(%s)" % window
    if established:
        return _bench_mode_record(BENCH_MODE_FULL, "%s%s" % (why, (" · " + note) if note else ""),
                                  repetition=summ, kill_hits=hits, correlation=correlation, events_scan=scan_doc)
    return _bench_mode_record(BENCH_MODE_LITE, why, DOWNGRADE_RUN_FAILED, "%s · %s" % (signal, note),
                              repetition=summ, kill_hits=hits, correlation=correlation, events_scan=scan_doc)


def _self_test():
    failures = []

    def check(name, condition, detail=""):
        if condition:
            print("  [PASS] %s" % name)
        else:
            print("  [FAIL] %s %s" % (name, detail)); failures.append(name)

    started = _utc("2026-09-04T10:00:00Z", "s")
    ended = _utc("2026-09-04T10:30:00Z", "e")
    ev = lambda kind, ts, mode="armed": json.dumps(
        {"ts": ts, "kind": kind, "source": "mem_watchdog_eta", "mode": mode, "targets": "abc"})

    lines = [ev("watchdog_trip", "2026-09-04T10:10:00Z"),
             ev("watchdog_kill_ack", "2026-09-04T10:10:01Z"),
             ev("watchdog_kill_ack", "2026-09-03T10:10:01Z"),          # 구간 밖
             ev("watchdog_trip_dryrun", "2026-09-04T10:12:00Z", "dry-run"),
             ev("watchdog_kill_ack", "2026-09-04T10:13:00Z", "dry-run"),  # 관측 전용
             "not json at all"]
    hits = kill_events_in_window(lines, started, ended)
    check("K1 구간 밖 이벤트는 제외", all(h["ts"].startswith("2026-09-04") for h in hits))
    check("K2 dry-run 은 사인 후보가 아니다", all(h["mode"] != "dry-run" for h in hits))
    check("K3 손상 줄이 분류를 죽이지 않는다", len(hits) == 2)

    out = classify(3, 0, [], "none")
    check("C1 서빙 실패 → serve_failed",
          out["cell_outcome"] == OUTCOME_SERVE_FAILED and out["void_reason"] is None)

    # ── R4(2026-09-11): serve 실패에서도 **사인은 시각 대조가 답한다** ──────────────────
    out = classify(3, None, hits, "docs/logs/main/events/2026-09.jsonl")
    check("★C7 서빙 실패 + 집행된 사살 → 분류는 serve_failed, 사인은 이벤트에서 온다",
          out["cell_outcome"] == OUTCOME_SERVE_FAILED
          and out["void_reason"] == "watchdog_kill_ack"
          and out["void_reason_source"].startswith("events(")
          and "호스트 평면의 사살" in out["note"])
    out = classify(3, None, [], "docs/logs/main/events/2026-09.jsonl")
    check("★C8 서빙 실패 + 대조 실패 → 사인 없음이되 **대조했음**이 남는다(추측 ✗)",
          out["cell_outcome"] == OUTCOME_SERVE_FAILED
          and out["void_reason"] is None
          and out["void_reason_source"].startswith("correlation-miss("))
    out = classify(3, None, [h for h in hits if h["kind"] in TRIP_EVENT_KINDS],
                   "docs/logs/main/events/2026-09.jsonl")
    check("★C9 음성대조: 트립만으로 서빙 실패를 사살이라 단정하지 않는다(집행만 사인이다)",
          out["void_reason"] is None)

    out = classify(0, 4, hits, "docs/logs/main/events/2026-09.jsonl")
    check("C2 측정 실패 + 집행된 사살 → measurement_void(사인 명시)",
          out["cell_outcome"] == OUTCOME_MEASUREMENT_VOID
          and out["void_reason"] == "watchdog_kill_ack"
          and out["void_reason_source"].startswith("events("))

    out = classify(0, 4, [], "docs/logs/main/events/2026-09.jsonl")
    check("C3 측정 실패 + 대조 실패 → unknown(추측 ✗ · 대조했음이 남는다)",
          out["cell_outcome"] == OUTCOME_MEASUREMENT_VOID
          and out["void_reason"] == "unknown"
          and out["void_reason_source"].startswith("correlation-miss("))

    out = classify(0, 0, [], "none")
    check("C4 정상 → measured", out["cell_outcome"] == OUTCOME_MEASURED)

    out = classify(0, 0, hits, "x")
    check("C5 측정 성공인데 사살이 있었다 → 분류 유지·기록 노출",
          out["cell_outcome"] == OUTCOME_MEASURED and "사살 이벤트가" in out["note"])

    trips_only = kill_events_in_window([ev("watchdog_trip", "2026-09-04T10:10:00Z")], started, ended)
    out = classify(0, 4, trips_only, "x")
    check("C6 트립만 있고 집행이 없으면 사인으로 단정하지 않는다",
          out["void_reason"] == "unknown" and len(out["kill_events"]) == 1)

    # ── D: bench_mode 확정(2026-09-14 · plan_26091407 §4.4 · §7 O4 · 리뷰 정정: 판정점 범위·클램프 사살) ──────
    #   run k 는 base 분 + k-1 분에 [:00, :50]. 판정점(level 1) 실패주입은 run 2(=창 [10:01:00, 10:02:50]).
    def run(k, ok=True, tps=30.0, base=1):
        return {"run": k, "started_utc": "2026-09-04T10:%02d:00Z" % (k - 1 + base),
                "ended_utc": "2026-09-04T10:%02d:50Z" % (k - 1 + base),
                "run_bench_rc": 0, "parse_rc": 0, "measurement_ok": ok, "decode_tps": tps}

    def level(lv, runs):
        return {"level": lv, "status": "ok", "measured": {"measurement_ok": True, "runs": runs}}

    def index(*levels, requested=3, clamp_run=None, verdict_point=1):
        rep = {"requested": requested, "requested_source": "declared(fixture)", "kind": "warm-rerun"}
        if clamp_run is not None:
            rep["clamp_run"] = clamp_run
        doc = {"levels": list(levels), "repetition": rep}
        if verdict_point is not None:
            doc["verdict_point_level"] = verdict_point
        return doc
    evf = ["docs/logs/main/events/2026-09.jsonl"]
    ok3 = [run(1), run(2, tps=31.0), run(3, tps=29.0)]
    broken = [run(1), run(2, ok=False)]

    out = classify_bench_mode(index(level(1, ok3)), [], "none")
    check("D1 레벨 전부 반복 3 완주 → full · 사유 null · 대조 대상 없음(not_applicable)",
          out["bench_mode"] == BENCH_MODE_FULL and out["downgrade_reason"] is None
          and out["downgrade_correlation"] == CORRELATION_NOT_APPLICABLE and bench_mode_kind(out) == "full", out)

    kill_in = [ev("watchdog_kill_ack", "2026-09-04T10:02:10Z")]
    out = classify_bench_mode(index(level(1, broken)), kill_in, evf)
    check("★D2 실패주입: 판정점 run 2 무너짐 ∧ 끊긴 창 안 집행 사살 → lite · blackbox_kill · matched",
          out["bench_mode"] == BENCH_MODE_LITE and out["downgrade_reason"] == DOWNGRADE_BLACKBOX_KILL
          and out["downgrade_reason_source"].startswith("events(") and bench_mode_kind(out) == "downgraded-lite"
          and out["downgrade_correlation"] == CORRELATION_MATCHED, out)

    out = classify_bench_mode(index(level(1, broken)), [ev("thermal_trip", "2026-09-04T10:02:10Z")], evf)
    check("★D3 음성대조: 같은 창에 트립만 → blackbox_kill 로 단정 ✗ · run_failed · miss · 트립 사인 불충분 기재",
          out["downgrade_reason"] == DOWNGRADE_RUN_FAILED and "사인 불충분" in out["downgrade_reason_source"]
          and out["downgrade_correlation"] == CORRELATION_MISS, out)

    out = classify_bench_mode(index(level(1, broken)), [ev("watchdog_kill_ack", "2026-09-04T10:05:00Z")], evf)
    check("★D4 음성대조: 사살 시각이 창 밖(불일치 · tolerance 0) → run_failed · miss",
          out["bench_mode"] == BENCH_MODE_LITE and out["downgrade_reason"] == DOWNGRADE_RUN_FAILED
          and not out["repetition_kill_events"] and out["downgrade_correlation"] == CORRELATION_MISS, out)
    out = classify_bench_mode(index(level(1, broken)), [ev("watchdog_kill_ack", "2026-09-04T10:05:00Z")], evf,
                              tolerance_s=200)
    check("D4b 허용오차는 **명시**로만 넓어진다(--tolerance-s 200 이면 같은 사살이 창에 든다)",
          out["downgrade_reason"] == DOWNGRADE_BLACKBOX_KILL, out)

    out = classify_bench_mode(index(level(1, broken)), [ev("watchdog_kill_ack", "2026-09-04T10:02:10Z", "dry-run")], evf)
    check("★D5 dry-run 사살 기록은 사인이 아니다 → run_failed", out["downgrade_reason"] == DOWNGRADE_RUN_FAILED, out)

    wide = [run(1, tps=10.0), run(2, tps=30.0), run(3, tps=50.0)]
    out = classify_bench_mode(index(level(1, wide)), [], "none")
    check("★D6 음성대조: 분산만 크다(10/30/50) → full · 강등 사유 ✗(밴드는 기재일 뿐)",
          out["bench_mode"] == BENCH_MODE_FULL and out["downgrade_reason"] is None, out)

    out = classify_bench_mode(index(level(1, ok3)), kill_in, evf)
    check("D7 스윕이 멈추지 않았는데 같은 시간대 사살 → full(멈춘 자리가 없으면 반복을 끊지 않았다 · C5 와 동형)",
          out["bench_mode"] == BENCH_MODE_FULL and out["downgrade_correlation"] == CORRELATION_NOT_APPLICABLE, out)

    out = classify_bench_mode({"levels": [{"level": 1, "measured": {"measurement_ok": True}}],
                               "verdict_point_level": 1}, [], "none")
    check("D8 runs[] 없는 산출물(반복 축 이전) → 판정하지 않는다(null · not_evaluated)",
          out["bench_mode"] is None and out["bench_mode_source"].startswith("not_evaluated("), out)

    five = [run(1), run(2), run(3), run(4), run(5, ok=False)]
    out = classify_bench_mode(index(level(1, five), requested=5), [], evf)
    check("D9 요청 5 · 판정점 run 5 에서 끊김 → 완주 4 ≥ 3 이라 full(중단은 출처에 기재)",
          out["bench_mode"] == BENCH_MODE_FULL and "판정점 level 1 run 5" in out["bench_mode_source"], out)

    l2_broken = [run(1, base=5), run(2, ok=False, base=5)]
    out = classify_bench_mode(index(level(1, ok3), level(2, l2_broken)), [], evf)
    check("★D10 비대칭 교정: 경계 레벨 2 의 run 2 실패(판정점 3) → full(적응 상한 클램프 · 경계 첫 run 실패 D13 과 같은 판정)",
          out["bench_mode"] == BENCH_MODE_FULL and "적응 상한 클램프" in out["bench_mode_source"]
          and out["downgrade_correlation"] == CORRELATION_MISS, out)
    out = classify_bench_mode(index(level(1, ok3), level(2, l2_broken)),
                              [ev("thermal_kill_ack", "2026-09-04T10:06:20Z")], evf)
    check("★D10b 같은 경계 창(레벨 2 run 1 시작~run 2 끝) 안 집행 사살 → lite · blackbox_kill(판정점 반복이 섰어도)",
          out["bench_mode"] == BENCH_MODE_LITE and out["downgrade_reason"] == DOWNGRADE_BLACKBOX_KILL, out)

    out = classify_bench_mode(index(level(1, [run(1), run(2)]), requested=2), [], "none")
    check("D11 요청 자체가 정의 미만(끊김 없음) → 판정 불가(강등 사유 발명 ✗)",
          out["bench_mode"] is None and out["bench_mode_source"].startswith("unclassifiable("), out)

    check("D12 판독 규칙: 선언된 lite-only 와 강등된 lite 는 사유·출처로 갈린다",
          bench_mode_kind({"bench_mode": "lite", "downgrade_reason": None,
                           "bench_mode_source": "declared(lite_bench · lite-only 셀)"}) == "declared-lite"
          and bench_mode_kind({"bench_mode": "lite", "downgrade_reason": None,
                               "bench_mode_source": "runs[]"}) == "undetermined"
          and bench_mode_kind({"bench_mode": "lite", "downgrade_reason": "variance"}) == "undetermined"
          and bench_mode_kind(None) == "undetermined")
    _decl = declared_lite_record("lite_bench · lite-only 셀")
    check("D12b 선언 lite 기록 빌더(단계 ⑤ lite 통로가 쓴다) → declared-lite · 사유 null · 대조 not_applicable",
          bench_mode_kind(_decl) == "declared-lite" and _decl["downgrade_reason"] is None
          and _decl["downgrade_correlation"] == CORRELATION_NOT_APPLICABLE
          and set(_decl) == set(_bench_mode_record(None, "x")), _decl)
    try:
        declared_lite_record("  ")
        _empty_ok = False
    except ValueError:
        _empty_ok = True
    check("★D12c 음성대조: 출처 설명 없는 선언 lite 기록은 만들지 않는다", _empty_ok)

    clamp = {"level": 2, "run": 1, "started_utc": "2026-09-04T10:05:00Z", "ended_utc": "2026-09-04T10:05:30Z",
             "run_bench_rc": 3, "parse_rc": 0}
    out = classify_bench_mode(index(level(1, ok3), clamp_run=clamp), [], evf)
    check("D13 경계 레벨 2 첫 run 실패(클램프) · 사살 없음 → full · 대조는 miss 로 남는다",
          out["bench_mode"] == BENCH_MODE_FULL and out["downgrade_correlation"] == CORRELATION_MISS
          and out["repetition"]["stop"]["kind"] == "clamp", out)
    out = classify_bench_mode(index(level(1, ok3), clamp_run=clamp),
                              [ev("watchdog_kill_ack", "2026-09-04T10:05:10Z")], evf)
    check("★D14 클램프 레벨 첫 run 창 안 집행 사살 → lite · blackbox_kill(가장 흔한 실사살 형태 · 절삭으로 삼키지 않는다)",
          out["bench_mode"] == BENCH_MODE_LITE and out["downgrade_reason"] == DOWNGRADE_BLACKBOX_KILL
          and bench_mode_kind(out) == "downgraded-lite", out)
    out = classify_bench_mode(index(level(1, ok3), clamp_run=clamp),
                              [ev("thermal_trip", "2026-09-04T10:05:10Z")], evf)
    check("★D15 음성대조: 같은 클램프 창에 트립 단독 → full 유지(사인 불충분)",
          out["bench_mode"] == BENCH_MODE_FULL and "사인 불충분" in out["bench_mode_source"], out)
    out = classify_bench_mode(index(level(1, ok3), clamp_run=clamp), [], "none")
    check("D16 이벤트 파일 0(블랙박스 미설치) → 판정은 runs 로 full · 대조는 not_scanned(보지 못했음이 구조로 남는다)",
          out["bench_mode"] == BENCH_MODE_FULL and out["downgrade_correlation"] == CORRELATION_NOT_SCANNED, out)
    out = classify_bench_mode(index(level(1, broken)), [], [])
    check("D16b 판정점 끊김 ∧ 이벤트 파일 0 → run_failed · not_scanned",
          out["downgrade_reason"] == DOWNGRADE_RUN_FAILED
          and out["downgrade_correlation"] == CORRELATION_NOT_SCANNED, out)
    out = classify_bench_mode(index(level(1, ok3), clamp_run={"level": 2, "run": 1, "unreadable": "부재"}),
                              kill_in, evf)
    check("D17 클램프 경계 사실 판독 불가 → 창 대조 unavailable(이름으로 남긴다 · 사살을 발명하지 않는다)",
          out["bench_mode"] == BENCH_MODE_FULL and out["downgrade_correlation"] == CORRELATION_UNAVAILABLE, out)
    out = classify_bench_mode(index(level(1, ok3), level(2, [run(1, base=5), run(2, base=5)]),
                                    level(4, [run(1, base=9), run(2, base=9), run(3, base=9)])), [], evf)
    check("★D18 경계가 아닌 레벨이 정의 미만(스윕은 멈추지 않았다) → 판정 불가",
          out["bench_mode"] is None and out["bench_mode_source"].startswith("unclassifiable("), out)
    out = classify_bench_mode(index(level(1, ok3), verdict_point=None), [], evf)
    check("★D19 판정점 레벨 부재(verdict_point_level 없음) → 판정 불가(1 을 가정하지 않는다)",
          out["bench_mode"] is None and "verdict_point_level" in out["bench_mode_source"], out)

    # ── E: 판정 기록 자리 · CLI (원자적 쓰기 · 신선도 · 모드 분리 · --tolerance-s 전달) ────────────────────
    import contextlib
    import io
    import tempfile as _tf
    with _tf.TemporaryDirectory(prefix="classify-cell.") as td:
        idx_path = os.path.join(td, "sweep_index.json")
        doc = dict(index(level(1, broken)), generated_utc="2026-09-04T11:00:00Z")
        with open(idx_path, "w", encoding="utf-8") as handle:
            json.dump(doc, handle)
        rec, status = read_bench_mode_record(idx_path, doc)
        check("E1 기록 부재 → (None, absent)", rec is None and status.startswith("absent("), status)
        events_path = os.path.join(td, "events.jsonl")
        with open(events_path, "w", encoding="utf-8") as handle:
            handle.write(ev("watchdog_kill_ack", "2026-09-04T10:05:00Z") + "\n")
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = main(["--sweep-index", idx_path, "--events", events_path, "--write-bench-mode"])
        rec, status = read_bench_mode_record(idx_path, doc)
        check("E2 CLI bench-mode 모드 → 기록이 index 측정시각에 묶여 쓰인다 · tolerance 0 이면 창 밖 사살은 run_failed",
              rc == 0 and status == "ok" and rec["downgrade_reason"] == DOWNGRADE_RUN_FAILED
              and not [n for n in os.listdir(td) if n.startswith("." + BENCH_MODE_RECORD_NAME)], (rc, status, rec))
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = main(["--sweep-index", idx_path, "--events", events_path, "--tolerance-s", "200",
                       "--write-bench-mode"])
        rec, status = read_bench_mode_record(idx_path, doc)
        check("★E3 CLI 가 --tolerance-s 를 판정에 **전달**한다(같은 사살이 창에 든다 → blackbox_kill)",
              rc == 0 and rec["downgrade_reason"] == DOWNGRADE_BLACKBOX_KILL and rec["tolerance_s"] == 200, rec)
        rec, status = read_bench_mode_record(idx_path, dict(doc, generated_utc="2026-09-04T12:00:00Z"))
        check("★E4 다른 측정의 기록 → (None, stale)", rec is None and status.startswith("stale("), status)
        with open(bench_mode_record_path(idx_path), "w", encoding="utf-8") as handle:
            handle.write('{"bench_mode": "fu')
        rec, status = read_bench_mode_record(idx_path, doc)
        check("★E5 반쯤 쓰인 기록 → (None, unreadable) · full/lite 로 접지 않는다",
              rec is None and status.startswith("unreadable("), status)
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            rc_mix = main(["--sweep-index", idx_path, "--serve-rc", "0", "--measure-rc", "0",
                           "--started-utc", "2026-09-04T10:00:00Z", "--ended-utc", "2026-09-04T11:00:00Z"])
            rc_nowrite = main(["--write-bench-mode", "--serve-rc", "0", "--measure-rc", "0",
                               "--started-utc", "2026-09-04T10:00:00Z", "--ended-utc", "2026-09-04T11:00:00Z"])
        check("★E6 두 모드를 섞으면 exit 2 · --write-bench-mode 는 --sweep-index 없이 exit 2", rc_mix == 2 and rc_nowrite == 2,
              (rc_mix, rc_nowrite))

    # ── N·F·G: 대조 대상 노드(2026-09-14 · ⑧ 분석 발견 T1) — 노드 계획 · CLI 실패주입 · 음성대조 ─────────────────────
    #   픽스처는 실물 자리 모양 그대로다: 이 노드 = docs/logs/<node>/events · 다른 노드 = fetch_sub_docs 미러
    #   sync_staging/sub_docs/logs/<node>/events · manifest = output/<topology>/manifest.yaml(nodes[].role · self_role).
    #   창은 D13 과 같은 클램프 창 [10:03:00, 10:05:30](repeat_axis 가 정한 멈춘 자리 창 · F3 출력으로 확인).
    _here = os.path.dirname(os.path.abspath(__file__))
    _fetch = os.path.normpath(os.path.join(_here, "..", "..", "upstream-version-watch", "scripts", "fetch_sub_docs.sh"))
    if os.path.isfile(_fetch):
        with open(_fetch, encoding="utf-8") as handle:
            _fetch_src = handle.read()
        # 주석에도 같은 리터럴이 있으므로 부분문자열이 아니라 **대입 줄의 값**을 뽑아 같다고 본다(2026-09-14 리뷰 정정 —
        #   종전 검사는 기본값을 바꿔도 주석 덕에 초록이었다). 미러는 서브 `docs/` 의 사본이므로 그 다음 성분이 `logs` 다.
        import re as _re
        _dest = _re.findall(r'^DEST="\$\{DEST:-\$\{SRC%/\}/([^}"]+)\}"\s*$', _fetch_src, _re.MULTILINE)
        _copies_docs = bool(_re.search(r'rsync -az [^\n]*"\$SUB_HOST:\$SUB_WORK_DIR/docs/" "\$DEST/"', _fetch_src))
        check("K-mirror 회수 미러 자리 = fetch_sub_docs.sh DEST 기본값 · 미러 = 서브 docs/ 사본(교차검증 · 한쪽만 바뀌면 원격 "
              "events 를 조용히 못 본다)",
              _dest == ["/".join(EVENTS_RECOVERED_PARTS[:2])] and _copies_docs
              and EVENTS_RECOVERED_PARTS[2:] == EVENTS_LOCAL_PARTS[1:], (_dest, _copies_docs))
    else:
        print("  [SKIP] K-mirror fetch_sub_docs.sh 부재(서브 배달 트리 — 메인 전용 스크립트다)")

    _multi_manifest = ("self_role: main\ntopology: multi\ngpus_per_node: 1\nnodes:\n  - role: main\n    host: 192.0.2.10\n"
                       "  - role: sub\n    host: 192.0.2.11\n")
    with _tf.TemporaryDirectory(prefix="classify-cell-nodes.") as td:
        def put(rel, text):
            path = os.path.join(td, rel)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(text)
            return path
        mm = put("repo/output/multi/manifest.yaml", _multi_manifest)
        sm = put("repo/output/single/manifest.yaml", "self_role: main\ntopology: single\ngpus_per_node: 1\n")
        dup = put("dup/output/multi/manifest.yaml", _multi_manifest + "  - role: sub\n    host: 192.0.2.12\n")

        pl = events_node_plan("single", measured_node="main", measured_node_source="derived(manifest.self_role=main)")
        check("N1 single 계획 = 측정 노드 하나(meta.measured_node) · 다른 노드 제외 규칙이 기록된다",
              pl["undeterminable"] is None and pl["nodes"] == ["main"] and pl["self_node"] == "main"
              and pl["excluded_rule"], pl)
        repo = os.path.join(td, "repo")
        pl = events_node_plan("single", manifest_path=sm, repo=repo)
        check("N2 single 셀 종결 계획 = 이 노드(소유자 node_identity.sh --resolve)", pl["nodes"] == ["main"]
              and "node_identity.sh --resolve" in pl["source"], pl)
        check("★N3 음성대조: single 에 measured_node=cluster · 경로 탈출 이름 → 계획 불성립(추측 ✗)",
              events_node_plan("single", measured_node="cluster")["undeterminable"]
              and events_node_plan("single", measured_node="../x")["undeterminable"])
        pl = events_node_plan("multi", measured_node="cluster", manifest_path=mm, repo=repo)
        _contract_ok = _node_role_contract() is not None
        _identity_ok = _node_identity_resolver() is not None
        check("F00 노드 정체성 해소기(node_identity.sh)가 코드 옆에 있다(없으면 이 노드를 정하지 못해 전 사례가 not_scanned 로 접힌다)",
              _identity_ok)
        check("N4 multi 계획 = manifest nodes[].role 전부 · self=이 노드(해소기)", (not _contract_ok) or (
              pl["undeterminable"] is None and pl["nodes"] == ["main", "sub"] and pl["self_node"] == "main"), pl)
        pl = events_node_plan("multi", manifest_path=dup, repo=os.path.join(td, "dup"))
        check("★N5 음성대조: multi 에 역할 슬러그 중복(sub 2대) → 계획 불성립(node_role_contract 위반) · 이 노드(main)는 남긴다",
              bool(pl["undeterminable"]) and pl["nodes"] == [] and pl["self_node"] == "main", pl)
        check("★N6 음성대조: multi 인데 measured_node=main → 계획 불성립(쌍이 측정 정체성)",
              bool(events_node_plan("multi", measured_node="main", manifest_path=mm, repo=repo)["undeterminable"]))

        def sweep(topo, measured_node, name, root="repo"):
            path = put("%s/output/%s/benchlog/sweep_%s/sweep_index.json" % (root, topo, name), json.dumps(dict(
                index(level(1, ok3), clamp_run=clamp), generated_utc="2026-09-04T11:00:00Z",
                meta={"topology": topo, "measured_node": measured_node,
                      "measured_node_source": "derived(fixture)"})))
            return path

        def cli(*argv):
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(io.StringIO()):
                rc = main(list(argv))
            try:
                return rc, json.loads(buf.getvalue() or "{}")
            except ValueError:
                return rc, {}

        def node_status(rec, node):
            return next((e.get("status") for e in ((rec.get("events_scan") or {}).get("nodes") or [])
                         if e.get("node") == node), None)

        main_ev = "repo/docs/logs/main/events/2026-09.jsonl"
        mirror_ev = "repo/sync_staging/sub_docs/logs/sub/events/2026-09.jsonl"
        put(main_ev, ev("budget_renew", "2026-09-04T10:40:00Z") + "\n")
        if _contract_ok:
            idx = sweep("multi", "cluster", "m")
            put(mirror_ev, ev("watchdog_kill_ack", "2026-09-04T10:05:10Z") + "\n" + ev("serve_stop", "2026-09-04T10:40:00Z") + "\n")
            rc, rec = cli("--sweep-index", idx, "--events-from-repo", repo)
            check("★F1 실패주입 multi: 서브 사살이 클램프 창 안(회수 미러에 있다) → lite · blackbox_kill · matched · node=sub",
                  rc == 0 and rec.get("bench_mode") == BENCH_MODE_LITE and rec.get("downgrade_reason") == DOWNGRADE_BLACKBOX_KILL
                  and rec.get("downgrade_correlation") == CORRELATION_MATCHED and node_status(rec, "sub") == NODE_SCAN_MATCHED
                  and "node=sub" in (rec.get("downgrade_reason_source") or ""), rec)
            os.remove(os.path.join(td, mirror_ev))
            rc, rec = cli("--sweep-index", idx, "--events-from-repo", repo)
            check("★F2 실패주입 multi: 서브 events 미회수 → full 이되 대조는 not_scanned(조용한 miss ✗) · sub=not_scanned 기재",
                  rc == 0 and rec.get("bench_mode") == BENCH_MODE_FULL
                  and rec.get("downgrade_correlation") == CORRELATION_NOT_SCANNED
                  and node_status(rec, "main") == NODE_SCAN_MISS and node_status(rec, "sub") == NODE_SCAN_NOT_SCANNED
                  and "not-scanned(" in (rec.get("bench_mode_source") or "")
                  and "sub(원격)=not_scanned" in (rec.get("bench_mode_source") or ""), rec)
            put(mirror_ev, ev("serve_stop", "2026-09-04T09:00:00Z") + "\n")
            rc, rec = cli("--sweep-index", idx, "--events-from-repo", repo)
            check("★F3 회수가 이르다(미러 최신 이벤트 < 창 끝) → not_scanned · sub=not_covered(낡은 미러의 부재를 miss 로 읽지 않는다)",
                  rc == 0 and rec.get("downgrade_correlation") == CORRELATION_NOT_SCANNED
                  and node_status(rec, "sub") == NODE_SCAN_NOT_COVERED, rec)
            put(mirror_ev, ev("serve_stop", "2026-09-04T10:40:00Z") + "\n")
            rc, rec = cli("--sweep-index", idx, "--events-from-repo", repo)
            check("★F4 미러가 창을 덮지만 노드 간 허용오차 미선언 → unavailable · sub=skew_undeclared(미스를 결론내지 않는다)",
                  rc == 0 and rec.get("bench_mode") == BENCH_MODE_FULL
                  and rec.get("downgrade_correlation") == CORRELATION_UNAVAILABLE
                  and node_status(rec, "sub") == NODE_SCAN_SKEW_UNDECLARED, rec)
            rc, rec = cli("--sweep-index", idx, "--events-from-repo", repo, "--cross-node-tolerance-s", "2",
                          "--cross-node-tolerance-source", "fixture 선언")
            _sub = next(e for e in rec["events_scan"]["nodes"] if e["node"] == "sub")
            check("F5 허용오차를 출처와 함께 선언 → miss(대조 대상 노드 전부 봤다) · 출처 declared(…) 기록",
                  rc == 0 and rec.get("downgrade_correlation") == CORRELATION_MISS and _sub["tolerance_s"] == 2
                  and _sub["tolerance_source"] == "declared(fixture 선언)", rec)
            rc_nosrc, _ = cli("--sweep-index", idx, "--events-from-repo", repo, "--cross-node-tolerance-s", "2")
            check("★F6 음성대조: 출처 없는 노드 간 허용오차 → exit 2(매직넘버 ✗)", rc_nosrc == 2)
            put(mirror_ev, ev("watchdog_kill_ack", "2026-09-04T10:05:40Z") + "\n" + ev("serve_stop", "2026-09-04T10:40:00Z") + "\n")
            rc, rec = cli("--sweep-index", idx, "--events-from-repo", repo, "--cross-node-tolerance-s", "15",
                          "--cross-node-tolerance-source", "fixture 선언")
            check("F7 선언된 노드 간 허용오차가 원격 창만 넓힌다(창 끝+10s 사살 → tol 15 이면 matched)",
                  rc == 0 and rec.get("downgrade_reason") == DOWNGRADE_BLACKBOX_KILL, rec)
            os.remove(os.path.join(td, mirror_ev))
            put("repo/docs/logs/sub/events/2026-09.jsonl", ev("watchdog_kill_ack", "2026-09-04T10:05:10Z") + "\n")
            rc, rec = cli("--sweep-index", idx, "--events-from-repo", repo)
            check("F8 multi: 사고 회수로 메인 docs/logs/sub 에 둔 서브 기록도 같은 노드의 기록으로 대조 → blackbox_kill",
                  rc == 0 and rec.get("downgrade_reason") == DOWNGRADE_BLACKBOX_KILL and node_status(rec, "sub") == NODE_SCAN_MATCHED,
                  rec)
            os.remove(os.path.join(td, "repo/docs/logs/sub/events/2026-09.jsonl"))

            # ── F9·F10: 노드 계획이 manifest self_role 에 기대지 않는다 · 계획이 서지 않아도 이 노드 기록은 본다(2026-09-14 리뷰 정정) ──
            #   실물 모양: self_role 없는 manifest 는 저장소가 살아 있는 상태로 다룬다(sweep_bench `defaulted(self_role absent)` ·
            #   selftest_sweep_repeats Sandbox). 초판은 여기서 계획을 버리고 이 노드의 창 안 사살까지 not_scanned → full 로 냈다.
            put("legacy/output/multi/manifest.yaml", _multi_manifest.replace("self_role: main\n", ""))
            put("legacy/output/single/manifest.yaml", "topology: single\ngpus_per_node: 1\n")
            legacy = os.path.join(td, "legacy")
            idx_l = sweep("multi", "cluster", "legacy", root="legacy")
            put("legacy/docs/logs/main/events/2026-09.jsonl", ev("watchdog_kill_ack", "2026-09-04T10:05:10Z") + "\n")
            rc, rec = cli("--sweep-index", idx_l, "--events-from-repo", legacy)
            check("★F9 실패주입 multi · manifest self_role 없음 ∧ 이 노드(main) 창 안 사살 → 계획은 소유자 규칙(유일한 role: main)으로 "
                  "서고 lite · blackbox_kill · matched",
                  rc == 0 and rec.get("downgrade_reason") == DOWNGRADE_BLACKBOX_KILL
                  and rec.get("downgrade_correlation") == CORRELATION_MATCHED
                  and (rec["events_scan"]["plan"] or {}).get("undeterminable") is None
                  and node_status(rec, "main") == NODE_SCAN_MATCHED and node_status(rec, "sub") == NODE_SCAN_NOT_SCANNED, rec)
            idx_d = sweep("multi", "cluster", "dup", root="dup")
            dup_repo = os.path.join(td, "dup")
            rc, rec = cli("--sweep-index", idx_d, "--events-from-repo", dup_repo)
            check("★F10a 계약 위반 manifest(역할 중복) · 이 노드 events 없음 → 계획 불성립 기록 · not_scanned · (미정) 노드 기재(miss ✗)",
                  rc == 0 and rec.get("bench_mode") == BENCH_MODE_FULL
                  and rec.get("downgrade_correlation") == CORRELATION_NOT_SCANNED
                  and (rec["events_scan"]["plan"] or {}).get("undeterminable")
                  and node_status(rec, UNIDENTIFIED_NODE) == NODE_SCAN_NOT_SCANNED
                  and "대조 노드 미정" in (rec.get("bench_mode_source") or ""), rec)
            put("dup/docs/logs/main/events/2026-09.jsonl", ev("serve_stop", "2026-09-04T10:40:00Z") + "\n")
            rc, rec = cli("--sweep-index", idx_d, "--events-from-repo", dup_repo)
            check("★F10b 계약 위반 · 이 노드 기록은 있고 사살 없음 → main=miss 여도 (미정) 때문에 not_scanned(미스를 결론내지 않는다)",
                  rc == 0 and rec.get("downgrade_correlation") == CORRELATION_NOT_SCANNED
                  and node_status(rec, "main") == NODE_SCAN_MISS and node_status(rec, UNIDENTIFIED_NODE) == NODE_SCAN_NOT_SCANNED,
                  rec)
            put("dup/docs/logs/main/events/2026-09.jsonl", ev("watchdog_kill_ack", "2026-09-04T10:05:10Z") + "\n")
            rc, rec = cli("--sweep-index", idx_d, "--events-from-repo", dup_repo)
            check("★F10c 실패주입 계약 위반 manifest ∧ 이 노드 창 안 사살 → lite · blackbox_kill(같은 호스트·같은 시계 기록은 버리지 않는다)",
                  rc == 0 and rec.get("downgrade_reason") == DOWNGRADE_BLACKBOX_KILL
                  and rec.get("downgrade_correlation") == CORRELATION_MATCHED, rec)

            # ── F11: 집계 순서 — 다른 노드가 not_scanned 여도 한 노드의 matched 가 이긴다(matched > not_scanned) ──
            os.remove(os.path.join(td, main_ev))
            put(mirror_ev, ev("watchdog_kill_ack", "2026-09-04T10:05:10Z") + "\n" + ev("serve_stop", "2026-09-04T10:40:00Z") + "\n")
            rc, rec = cli("--sweep-index", idx, "--events-from-repo", repo)
            check("★F11 집계 순서: main=not_scanned(events 0) ∧ sub(원격)=matched → matched · blackbox_kill · 원격 매치의 허용오차 미선언이 "
                  "출처에 드러난다",
                  rc == 0 and rec.get("downgrade_correlation") == CORRELATION_MATCHED
                  and node_status(rec, "main") == NODE_SCAN_NOT_SCANNED and node_status(rec, "sub") == NODE_SCAN_MATCHED
                  and "허용오차 미선언" in (rec.get("downgrade_reason_source") or ""), rec)
            os.remove(os.path.join(td, mirror_ev))
            put(main_ev, ev("budget_renew", "2026-09-04T10:40:00Z") + "\n")
        else:
            check("F0 multi 노드 계획의 소유자(node_role_contract)가 코드 옆에 있어야 multi 실패주입을 친다", False,
                  "node_role_contract 적재 실패")

        idx = sweep("single", "main", "s")
        put("repo/docs/logs/sub/events/2026-09.jsonl", ev("watchdog_kill_ack", "2026-09-04T10:05:10Z") + "\n")
        rc, rec = cli("--sweep-index", idx, "--events-from-repo", repo)
        check("★G1 음성대조 single: 다른 노드(docs/logs/sub)의 창 안 사살은 대조 제외 → full · miss · 대조 노드는 main 하나",
              rc == 0 and rec.get("bench_mode") == BENCH_MODE_FULL and rec.get("downgrade_correlation") == CORRELATION_MISS
              and [e["node"] for e in rec["events_scan"]["nodes"]] == ["main"]
              and rec["events_scan"]["plan"]["excluded_rule"], rec)
        put(main_ev, ev("watchdog_kill_ack", "2026-09-04T10:05:10Z") + "\n")
        rc, rec = cli("--sweep-index", idx, "--events-from-repo", repo)
        check("G2 single: 측정 노드 자신의 창 안 사살 → lite · blackbox_kill", rc == 0
              and rec.get("downgrade_reason") == DOWNGRADE_BLACKBOX_KILL, rec)
        put(main_ev, ev("budget_renew", "2026-09-04T10:40:00Z") + "\n")   # main 의 사살을 걷어낸다 — G3 가 main 으로 통과하지 않게
        idx_sub = sweep("single", "sub", "on-sub")
        rc, rec = cli("--sweep-index", idx_sub, "--events-from-repo", repo)
        check("G3 single 서브에서 잰 스윕(measured_node=sub) → 자기 docs/logs/sub 가 로컬 대조 대상 → blackbox_kill "
              "(대조 노드는 sub 하나 · manifest self_role=main 이 아니라 측정 기록이 정한다)",
              rc == 0 and rec.get("downgrade_reason") == DOWNGRADE_BLACKBOX_KILL
              and [e["node"] for e in rec["events_scan"]["nodes"]] == ["sub"]
              and rec["events_scan"]["nodes"][0]["remote"] is False, rec)

        cell = ("--serve-rc", "3", "--measure-rc", "absent", "--started-utc", "2026-09-04T10:00:00Z",
                "--ended-utc", "2026-09-04T10:30:00Z", "--events-from-repo", repo)
        rc, out = cli(*cell, "--topology", "single")
        check("★G4 셀 종결 single: 다른 노드 사살은 사인이 아니다 → void_reason null · correlation-miss(대조 노드 main)",
              rc == 0 and out.get("void_reason") is None and str(out.get("void_reason_source")).startswith("correlation-miss(")
              and "대조 노드[main=miss]" in out.get("void_reason_source", ""), out)
        rc_notopo, _ = cli(*cell)
        check("★G5 음성대조: 셀 종결 모드의 --events-from-repo 에 --topology 없음 → exit 2(모든 노드 대조 ✗)", rc_notopo == 2)
        if _contract_ok:
            rc, out = cli(*cell, "--topology", "multi")
            check("G6 셀 종결 multi: 서브 사살(메인 docs/logs/sub 사고 회수본) → serve_failed 사인 = 이벤트",
                  rc == 0 and out.get("void_reason") == "watchdog_kill_ack"
                  and str(out.get("void_reason_source")).startswith("events("), out)
            os.remove(os.path.join(td, "repo/docs/logs/sub/events/2026-09.jsonl"))
            rc, out = cli(*cell, "--topology", "multi")
            check("★G7 셀 종결 multi: 서브 미회수 → 사인 null · 출처 not-scanned(보지 못했음이 남는다)",
                  rc == 0 and out.get("void_reason") is None and str(out.get("void_reason_source")).startswith("not-scanned(")
                  and out.get("downgrade_correlation") == CORRELATION_NOT_SCANNED, out)
        rc, rec = cli("--sweep-index", idx, "--events-from-repo", repo, "--topology", "single")
        check("★G8 음성대조: bench-mode 에 --topology → exit 2(측정 기록 meta 가 토폴로지를 말한다)", rc == 2)
        idx_old = put("repo/output/single/benchlog/sweep_old/sweep_index.json", json.dumps(dict(
            index(level(1, ok3), clamp_run=clamp), generated_utc="2026-09-04T11:00:00Z")))
        rc, rec = cli("--sweep-index", idx_old, "--events-from-repo", os.path.join(td, "nomanifest"))
        check("G9 meta·manifest 없는 옛 index → 노드 계획 불성립을 기록 · not_scanned(추측으로 모든 노드를 보지 않는다)",
              rc == 0 and rec.get("downgrade_correlation") == CORRELATION_NOT_SCANNED
              and (rec["events_scan"]["plan"] or {}).get("undeterminable")
              and "대조 노드 미정" in (rec.get("bench_mode_source") or ""), rec)
        put(main_ev, ev("watchdog_kill_ack", "2026-09-04T10:05:10Z") + "\n")
        rc, rec = cli("--sweep-index", idx_old, "--events-from-repo", repo)
        check("★G9b meta 없는 옛 index · manifest 있는 저장소 ∧ 이 노드 창 안 사살 → 계획 불성립이어도 이 노드 기록은 대조 → blackbox_kill "
              "(다른 노드 docs/logs/sub 는 보지 않는다)",
              rc == 0 and rec.get("downgrade_reason") == DOWNGRADE_BLACKBOX_KILL
              and [e["node"] for e in rec["events_scan"]["nodes"]] == ["main", UNIDENTIFIED_NODE], rec)
        put(main_ev, ev("budget_renew", "2026-09-04T10:40:00Z") + "\n")

        # ── G10: 셀 종결 모드 · self_role 없는 manifest(실물 Sandbox 모양) — 이 노드는 소유자 규칙으로 선다 ──
        put("legacy/docs/logs/main/events/2026-09.jsonl", ev("watchdog_kill_ack", "2026-09-04T10:10:00Z") + "\n")
        rc, out = cli("--serve-rc", "3", "--measure-rc", "absent", "--started-utc", "2026-09-04T10:00:00Z",
                      "--ended-utc", "2026-09-04T10:30:00Z", "--events-from-repo", os.path.join(td, "legacy"),
                      "--topology", "single")
        check("★G10 실패주입 셀 종결 single · manifest self_role 없음 ∧ 이 노드 구간 안 사살 → 사인 = 이벤트(계획 불성립 ✗)",
              rc == 0 and out.get("void_reason") == "watchdog_kill_ack" and str(out.get("void_reason_source")).startswith("events(")
              and (out["events_scan"]["plan"] or {}).get("undeterminable") is None, out)

    if failures:
        sys.stderr.write("[classify_cell --self-test] FAIL %d 건: %s\n" % (len(failures), failures))
        return 1
    print("[classify_cell --self-test] OK — K1~K3 · C1~C9 · D1~D19(bench_mode 확정 · 실패주입 · 음성대조) · E1~E6 · "
          "K-mirror · N1~N6 · F00 · F1~F11 · G1~G10(대조 대상 노드 · 이 노드 해소 · 계획 불성립 시 이 노드 기록) 전부 통과")
    return 0


def _read_events(paths):
    lines, scanned = [], []
    for path in paths:
        try:
            with open(path, encoding="utf-8", errors="replace") as handle:
                lines.extend(handle.readlines())
            scanned.append(path)
        except OSError as exc:
            # 이벤트 파일을 못 읽은 것을 "사건이 없었다"로 접지 않는다 — 큰 소리로 남긴다.
            sys.stderr.write("[classify_cell] WARN events 판독 실패(%s): %s\n" % (path, exc))
    return lines, scanned


def _scan_desc(scans, plan):
    """출처 서술용 판독 목록 — 파일 목록(없으면 none)에 노드 계획 요약을 붙인다."""
    files = [p for s in scans for p in s.get("files") or []]
    return files, ",".join(files) if files else "none"


def main(argv=None):
    ap = argparse.ArgumentParser(description="광의의 탐색 셀 종결 3분류 · full bench bench_mode 확정(결정론)")
    ap.add_argument("--serve-rc", type=int)
    ap.add_argument("--measure-rc", default=None,
                    help="측정 종료코드. **측정하지 않았으면 `absent`** 를 넘겨라(0 과 다른 사실이다).")
    ap.add_argument("--started-utc")
    ap.add_argument("--ended-utc")
    ap.add_argument("--events", action="append", default=[],
                    help="노드 블랙박스 events JSONL(반복 가능) — 호출자가 선언한 같은 시계 묶음(노드 계획 밖)")
    ap.add_argument("--events-from-repo",
                    help="저장소에서 **대조 대상 노드**의 events 를 찾는다(이 노드 docs/logs/<node>/events · 다른 노드 "
                         "sync_staging/sub_docs/logs/<node>/events). 노드 계획: bench-mode = sweep_index meta(topology · "
                         "measured_node) · 셀 종결 모드 = --topology(필수) + manifest · 이 노드 = node_identity.sh --resolve")
    ap.add_argument("--topology", choices=("single", "multi"),
                    help="셀 종결 모드에서 --events-from-repo 의 노드 계획 토폴로지(bench-mode 는 sweep_index meta 가 말한다)")
    ap.add_argument("--manifest",
                    help="노드 계획이 읽을 manifest(생략 시 <repo>/output/<topology>/manifest.yaml)")
    ap.add_argument("--tolerance-s", type=int, default=0,
                    help="같은 노드 시각 대조 허용오차(기본 0 — 같은 호스트 같은 시계라 스큐가 없다)")
    ap.add_argument("--cross-node-tolerance-s", type=int, default=None,
                    help="다른 노드 events 의 시각 대조 허용오차(초 · 선언으로만 · --cross-node-tolerance-source 필수). "
                         "미선언이면 원격 창을 넓히지 않고 미스를 결론내지 않는다(unavailable)")
    ap.add_argument("--cross-node-tolerance-source",
                    help="--cross-node-tolerance-s 의 출처(예: 두 노드 시계 동기 오차 실측 명령·시각)")
    ap.add_argument("--sweep-index",
                    help="bench-mode 모드: sweep_bench 산출 sweep_index.json 의 bench_mode(full|lite)·downgrade_reason 을 "
                         "확정한다(셀 종결 인자와 함께 쓰지 않는다)")
    ap.add_argument("--write-bench-mode", action="store_true",
                    help="bench_mode 판정 기록을 sweep_index 옆 `%s` 에 원자적으로 쓴다(자리 이름의 소유는 이 파일 · "
                         "--sweep-index 필요)" % BENCH_MODE_RECORD_NAME)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()
    cell_args = [n for n, v in (("--serve-rc", args.serve_rc), ("--measure-rc", args.measure_rc),
                                ("--started-utc", args.started_utc), ("--ended-utc", args.ended_utc))
                 if v is not None]
    if args.write_bench_mode and not args.sweep_index:
        sys.stderr.write("[classify_cell] ERROR --write-bench-mode 는 --sweep-index 가 필요하다\n")
        return 2
    if (args.cross_node_tolerance_s is None) != (not (args.cross_node_tolerance_source or "").strip()):
        sys.stderr.write("[classify_cell] ERROR --cross-node-tolerance-s 와 --cross-node-tolerance-source 는 함께 준다 — "
                         "노드를 넘는 허용오차는 출처 없는 숫자로 두지 않는다\n")
        return 2
    if args.cross_node_tolerance_s is not None and args.cross_node_tolerance_s < 0:
        sys.stderr.write("[classify_cell] ERROR --cross-node-tolerance-s 는 0 이상이다\n")
        return 2
    cross = (args.cross_node_tolerance_s, (args.cross_node_tolerance_source or "").strip() or None)

    def gather(plan):
        scans = scan_node_events(args.events_from_repo, plan) if plan is not None else []
        if args.events:
            lines, read = _read_events(list(args.events))
            scans.append({"node": "explicit", "remote": False, "files": read, "lines": lines, "repo": None})
        return scans

    # ── bench-mode 모드(post-hoc · sweep_bench 종료부가 부른다) ─────────────────────────────────────
    if args.sweep_index:
        if cell_args:
            sys.stderr.write("[classify_cell] ERROR --sweep-index(bench-mode 모드)와 셀 종결 인자(%s)를 섞지 않는다 — "
                             "셀 기록은 판정 기록(%s)을 읽는다\n" % (", ".join(cell_args), BENCH_MODE_RECORD_NAME))
            return 2
        if args.topology:
            sys.stderr.write("[classify_cell] ERROR bench-mode 에서는 --topology 를 받지 않는다 — 측정의 토폴로지는 "
                             "sweep_index meta.topology 가 말한다(측정 기록과 인자가 갈리면 어느 쪽도 고르지 않는다)\n")
            return 2
        try:
            with open(args.sweep_index, encoding="utf-8") as handle:
                index_doc = json.load(handle)
        except (OSError, ValueError) as exc:
            # 판독하지 못한 index 는 판정 대상이 아니다 — 기록을 쓰지 않는다(낡은 기록은 소비자가 stale 로 거른다).
            sys.stderr.write("[classify_cell] ERROR sweep-index 판독 실패(%s): %s\n" % (args.sweep_index, exc))
            return 2
        if not isinstance(index_doc, dict):
            sys.stderr.write("[classify_cell] ERROR sweep-index 가 객체가 아니다(%s)\n" % args.sweep_index)
            return 2
        plan = None
        if args.events_from_repo:
            meta = index_doc.get("meta") if isinstance(index_doc.get("meta"), dict) else {}
            topo = meta.get("topology")
            manifest = args.manifest or (os.path.join(args.events_from_repo, "output", topo, "manifest.yaml")
                                         if _path_segment(topo) else None)
            plan = events_node_plan(topo, measured_node=meta.get("measured_node"),
                                    measured_node_source=meta.get("measured_node_source"), manifest_path=manifest,
                                    repo=args.events_from_repo)
            if plan.get("undeterminable"):
                sys.stderr.write("[classify_cell] WARN 대조 대상 노드를 정하지 못했다 — %s (이 노드 %s 의 기록만 대조하고 "
                                 "나머지는 not_scanned 로 기록된다 · 대조 불가는 강등 트리거가 아니다)\n"
                                 % (plan["undeterminable"], plan.get("self_node") or "미해소(%s)" % plan.get("self_node_source")))
        scans = gather(plan)
        scanned, _ = _scan_desc(scans, plan)
        lines = [ln for s in scans for ln in s.get("lines") or []]
        bm = classify_bench_mode(index_doc, lines, scanned, args.tolerance_s, node_scans=scans, plan=plan,
                                 cross_node_tolerance_s=cross[0], cross_node_tolerance_source=cross[1])
        record = dict(bm, schema_version=1, kind="bench_mode_record", generated_by="classify_cell.py",
                      sweep_index=args.sweep_index, sweep_index_generated_utc=index_doc.get("generated_utc"),
                      events_scanned=scanned, tolerance_s=args.tolerance_s)
        if args.write_bench_mode:
            try:
                record["record_path"] = write_bench_mode_record(args.sweep_index, record)
            except OSError as exc:
                sys.stderr.write("[classify_cell] ERROR bench_mode 기록 실패(%s): %s\n"
                                 % (bench_mode_record_path(args.sweep_index), exc))
                print(json.dumps(record, ensure_ascii=False, indent=2))
                return 2
        print(json.dumps(record, ensure_ascii=False, indent=2))
        return 0

    # ── 셀 종결 모드 ─────────────────────────────────────────────────────────────────────────────
    if args.measure_rc in ("absent", ""):
        args.measure_rc = None
    elif args.measure_rc is not None:
        try:
            args.measure_rc = int(args.measure_rc)
        except ValueError:
            sys.stderr.write("[classify_cell] ERROR --measure-rc 는 정수 또는 `absent`\n")
            return 2
    else:
        sys.stderr.write("[classify_cell] ERROR --measure-rc 미지정 — 부재는 `absent` 로 **명시**하라"
                         "(빠뜨림과 부재를 구분한다)\n")
        return 2
    missing = [n for n, v in (("--serve-rc", args.serve_rc),
                              ("--started-utc", args.started_utc), ("--ended-utc", args.ended_utc))
               if v is None]
    if missing:
        sys.stderr.write("[classify_cell] ERROR 필수 인자 부재: %s\n" % ", ".join(missing))
        return 2
    plan = None
    if args.events_from_repo:
        if not args.topology:
            sys.stderr.write("[classify_cell] ERROR 셀 종결 모드의 --events-from-repo 는 --topology 가 필요하다 — 대조 대상 "
                             "노드를 정하지 않고 저장소의 모든 노드 events 를 대조하지 않는다\n")
            return 2
        plan = events_node_plan(args.topology, manifest_path=(
            args.manifest or os.path.join(args.events_from_repo, "output", args.topology, "manifest.yaml")),
            repo=args.events_from_repo)
        if plan.get("undeterminable"):
            sys.stderr.write("[classify_cell] WARN 대조 대상 노드를 정하지 못했다 — %s (이 노드 %s 의 기록만 대조하고 "
                             "나머지는 not-scanned 로 기록된다)\n"
                             % (plan["undeterminable"], plan.get("self_node") or "미해소(%s)" % plan.get("self_node_source")))

    scans = gather(plan)
    scanned, scanned_txt = _scan_desc(scans, plan)
    try:
        started = _utc(args.started_utc, "--started-utc")
        ended = _utc(args.ended_utc, "--ended-utc")
    except ClassifyError as exc:
        sys.stderr.write("[classify_cell] ERROR %s\n" % exc)
        return 2
    hits, correlation, per_node = correlate_nodes(scans, started, ended, args.tolerance_s, cross[0], cross[1])
    scan_doc = {"plan": plan if plan is not None else {"source": "explicit(호출자가 대상 events 를 선언했다 — 노드 계획 없음)"},
                "nodes": per_node, "cross_node_tolerance_s": cross[0], "cross_node_tolerance_source": cross[1]}
    if plan is not None:
        scanned_txt = "%s · %s" % (scanned_txt, events_scan_summary(scan_doc))
    out = classify(args.serve_rc, args.measure_rc, hits, scanned_txt, correlation=correlation)
    out["events_scanned"] = scanned
    out["tolerance_s"] = args.tolerance_s
    out["downgrade_correlation"] = correlation
    out["events_scan"] = scan_doc
    print(json.dumps(out, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
