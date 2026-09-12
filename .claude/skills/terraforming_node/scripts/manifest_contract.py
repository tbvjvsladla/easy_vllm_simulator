#!/usr/bin/env python3
"""manifest_contract.py — 테라포밍-완수 Flag 게이트의 결정론 리더 (plan_26063018).

헌법 §테라포밍-완수 Flag 게이트 따름정리의 **결정론 백스톱**. 3 런타임 스킬
(upstream-version-watch · vllm-recipe-explorer · adversarial-benchmark)의 *작업 스크립트*가
진입 시 이걸 호출해 Flag(terraforming.complete)를 확인한다 — 미발급이면 비0종료(작업 거부 → 에이전트 info-only).

설계:
- 단일계약: `output/<topology>/manifest.yaml` 를 읽는다(topology = 현재 git 브랜치가 선택하는 *통로*; --topology 로 override).
- **보수적**: `terraforming.complete == true` AND `terraforming.branch_verified == true` AND 필수 HW사실(topology·gpus_per_node) 존재해야 통과(fail-closed-on-flag).
- **TP**(manifest 파생): topology=single→gpus_per_node(노드 배수 1 고정 — sub 는 control 피어) ·
  그 외 nodes 있으면 len(nodes)×gpus_per_node · nodes 비면 None
  (roofline.py:196-209 동일 패턴 — **git 브랜치 폴백 없음**; config override·최종폴백 1 은 *호출자*가 적용).
- stdlib + yaml 만 · 외부 네트워크 호출 ✗(결정론 스크립트 평면 — 헌법 §금지).

CLI:
  manifest_contract.py [--topology single|multi] [--repo .] [--require-flag] [--json]
  exit 0 = Flag valid · 3 = manifest 부재 · 4 = Flag 미발급/미검증 · 5 = 필수필드 누락 · 2 = 사용오류

호출자 관용구(런타임 스킬 작업스크립트 진입):
  python3 .../manifest_contract.py --topology "$T" --repo "$REPO" --require-flag || {
      echo "Flag 미발급 — 테라포밍 먼저(info-only)"; exit 4; }
"""
import argparse
import json
import os
import subprocess
import sys

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_NO_MANIFEST = 3
EXIT_NO_FLAG = 4
EXIT_MISSING_FIELD = 5
#: 인자 topology 와 **실제 사실**(manifest 자신의 선언 · 체크아웃 브랜치)이 어긋난다.
#: 2026-09-12 신설(plan_26091210 A2 · policy BRANCH_CONSTITUTION_LAYERING C5). 종전에는 두 구멍이
#: 열려 있었다 — ⓐ `evaluate_contract` 가 manifest 안의 `topology` 를 인자와 **한 번도 대조하지
#: 않았고** ⓑ `multi-node` 체크아웃에서 `--topology single --require-flag` 가 exit 0 을 냈다(실측).
EXIT_TOPOLOGY_MISMATCH = 6

VALID_MODES = ("managed", "ephemeral", "custom")


def _git_branch(repo):
    try:
        out = subprocess.run(
            ["git", "-C", repo, "rev-parse", "--abbrev-ref", "HEAD"],
            capture_output=True, text=True,
        )
        if out.returncode == 0:
            return out.stdout.strip()
    except Exception:
        pass
    return None


def resolve_topology(repo, explicit):
    """topology = --topology 우선 · 없으면 git 브랜치가 *통로*를 선택(single-node→single / multi-node→multi)."""
    if explicit:
        return explicit
    branch = _git_branch(repo)
    if branch == "single-node":
        return "single"
    if branch == "multi-node":
        return "multi"
    return None  # 불명 — 호출자가 --topology 명시해야


def branch_topology_conflict(repo, explicit):
    """명시된 topology 가 **체크아웃 브랜치가 고르는 통로**와 어긋나면 사유를, 아니면 None.

    ★ 발화 조건은 **브랜치가 알려진 토폴로지로 해소될 때만**이다. 비-git 트리(배포 클론 스모크가
      쓰는 임시 디렉터리)·detached HEAD 에서는 종전 동작을 그대로 둔다 — 거기서 새로 죽으면
      배포 검증이 기대하는 종료코드가 바뀐다.

    브랜치는 권위가 아니라 필터다(policy BRANCH_CONSTITUTION_LAYERING). 그러므로 이 함수는
    "브랜치가 맞다" 고 말하지 않는다 — **둘이 어긋났다는 사실**만 말하고 어느 쪽을 고칠지는
    사람이 정한다(자동 교정 금지).
    """
    if not explicit:
        return None
    derived = resolve_topology(repo, None)
    if derived is None or derived == explicit:
        return None
    return ("요청 topology=%r 이 체크아웃 브랜치가 고르는 통로 %r 와 다르다 — 브랜치를 바꾸거나 "
            "요청을 바꿔라(자동 교정하지 않는다). 정본 술어: topology_parity.py"
            % (explicit, derived))


def effective_model_source(node, top_level_ms):
    """per-node model_source 해소 규칙(결정론 — plan_26070809_46_57 §4.4):
    node.model_source(있으면) > top-level model_source > None(오류, 호출자 판단)."""
    if isinstance(node, dict) and node.get("model_source"):
        return node["model_source"]
    return top_level_ms


def manifest_tp(man, topology):
    """manifest 파생 TP (roofline.py:196-209 패턴). config override·최종폴백 1 은 호출자 책임.

    **topology=single → 노드 배수 1 고정**(TP = gpus_per_node): single 의 nodes[role=sub] 는
    sub-control 피어이지 텐서 워커가 아니다. 이 배수를 안 걷으면 서브가 등록된 single manifest 가
    1-GPU 노드에 TP=2 를 요구한다(δ1-1 라이브 E2E 실버그 · check_smoke_model.read_manifest_tp 와
    recipe._target_cards 가 이미 쓰는 계약).
    그 외: nodes 있으면 len(nodes)×gpus_per_node · nodes 비면 None.

    **gpus_per_node 부재·비정수·1 미만 → ValueError**(2026-09-05 · audit_26090515 B4). 종전 `or 1` 은
    HW 사실을 기본값으로 대체하는 침묵 폴백이었다 — 계약의 소유자가 그 폴백을 갖고 있으면 하류 4사이트가
    같은 것을 복제한다. 호출자(evaluate_contract)는 예외를 잡아 EXIT_MISSING_FIELD 로 돌린다."""
    raw = man.get("gpus_per_node")
    try:
        gpus = int(raw)
    except (TypeError, ValueError):
        gpus = None
    if gpus is None or gpus < 1:
        raise ValueError("gpus_per_node 부재/비정수(%r) — TP 는 HW 사실이지 기본값이 아니다" % (raw,))
    if (topology or "").startswith("single"):
        return gpus
    nodes = man.get("nodes") or []
    if nodes:
        return max(1, len(nodes)) * gpus
    return None


def evaluate_contract(man, topology):
    """순수 게이트 함수(파일 IO 없음 — --self-test 가 이걸 친다).
    returns dict: {flag, reason, topology, gpus_per_node, nodes, manifest_tp, model_source,
                   nas_model_path, custom_model_paths, quant_model_path, tiktoken_host_path, exit_code}.
    """
    res = {
        "flag": False, "reason": "", "exit_code": EXIT_NO_FLAG,
        "topology": man.get("topology"),
        "gpus_per_node": man.get("gpus_per_node"),
        "nodes": man.get("nodes") or [],
        "model_source": man.get("model_source"),
        "nas_model_path": man.get("nas_model_path"),
        "custom_model_paths": man.get("custom_model_paths") or {},
        "quant_model_path": man.get("quant_model_path") or "",
        "tiktoken_host_path": man.get("tiktoken_host_path") or "",
    }
    try:
        res["manifest_tp"] = manifest_tp(man, topology)
        tp_error = None
    except ValueError as e:  # 부재/비정수 — 아래 필수 HW필드 검사가 EXIT_MISSING_FIELD 로 돌린다(침묵 1 ✗)
        res["manifest_tp"] = None
        tp_error = str(e)

    terra = man.get("terraforming") or {}
    if terra.get("complete") is not True:
        res["reason"] = "terraforming.complete != true (Flag 미발급 — info-only)"
        res["exit_code"] = EXIT_NO_FLAG
        return res
    if terra.get("branch_verified") is not True:
        res["reason"] = "terraforming.branch_verified != true (§1.5 3자일치 미통과 — info-only)"
        res["exit_code"] = EXIT_NO_FLAG
        return res

    # 필수 HW사실 (보수적: Flag 켜졌어도 핵심 필드 없으면 거부)
    missing = [k for k in ("topology", "gpus_per_node") if not man.get(k)]
    if missing:
        res["reason"] = "Flag true 이나 필수 HW필드 누락: %s" % ", ".join(missing)
        res["exit_code"] = EXIT_MISSING_FIELD
        return res
    if man.get("topology") != topology:
        # manifest 가 토폴로지 사실의 권위다. 인자가 그와 다르면 호출부가 **다른 통로의 사실로**
        # 판정하려는 것이고, 그 조합이 2026-09-11 사고의 형태다(멀티 캠페인을 싱글 통로에서 실행).
        res["reason"] = ("manifest 선언 topology=%r 과 요청 topology=%r 이 다르다 — manifest 가 권위다"
                         % (man.get("topology"), topology))
        res["exit_code"] = EXIT_TOPOLOGY_MISMATCH
        return res
    if tp_error:  # 존재하되 비정수/0 — `not man.get()` 은 못 잡는다
        res["reason"] = "Flag true 이나 %s" % tp_error
        res["exit_code"] = EXIT_MISSING_FIELD
        return res

    ms = man.get("model_source")
    if ms not in VALID_MODES:
        res["reason"] = "model_source 미설정/오류: %r (기대 %s)" % (ms, "|".join(VALID_MODES))
        res["exit_code"] = EXIT_MISSING_FIELD
        return res

    # per-node model_source override 검증 (single-node sub-control 한정 — plan_26070809_46_57 §4.4).
    # 해소규칙: node.model_source(있으면) > top-level ms. 멀티는 override 있어도 무시 대상(단일 정책)이라 WARN.
    node_warnings = []
    bad_node_sources = []
    for idx, node in enumerate(res["nodes"]):
        if not isinstance(node, dict):
            continue
        node_ms = node.get("model_source")
        if node_ms is None:
            continue
        if node_ms not in VALID_MODES:
            bad_node_sources.append(
                "nodes[%d](%s).model_source=%r" % (idx, node.get("role", "?"), node_ms))
        elif topology == "multi":
            node_warnings.append(
                "nodes[%d](%s).model_source override 는 single-node sub-control 한정 — "
                "multi 는 무시(단일 정책)" % (idx, node.get("role", "?")))
    if bad_node_sources:
        res["reason"] = "per-node model_source 오류: %s (기대 %s)" % (
            "; ".join(bad_node_sources), "|".join(VALID_MODES))
        res["exit_code"] = EXIT_MISSING_FIELD
        return res
    # ── 사전적재 자산이 노드-로컬이면 다중노드에서 구조적으로 도달 불가 (2026-09-03 신설) ──
    #
    # 실증: 서브가 gpt-oss-20b serve 를 완주하지 못했다. KV 도 메모리도 아니고
    # `openai_harmony.HarmonyError: invalid tiktoken vocab file` 이었다. gpt-oss 계열은
    # `reasoning_parser=openai_gptoss` 가 harmony 인코딩(o200k tiktoken vocab)을 **무조건** 로드하는데,
    # 그 자산은 자동 생성되는 캐시가 아니라 **사전적재가 필요한 입력**이고, 서브는 curl/wget/hf 가
    # 전부 deny 라 스스로 얻을 수 없다. 메인에만 노드-로컬로 두면 서브는 **영원히** 얻지 못한다.
    #
    # 규정의 분류 오류였다: `sync_to_sub` 의 BAND2_EXCLUDED_TOP 이 `cache`(JIT 캐시 — 자동 생성이라
    # 전파가 무의미)와 `tiktoken_cache`(사전적재 입력 — 획득 경로가 필요)를 **한 이름으로 묶어**
    # 둘 다 "노드-로컬 캐시" 로 불렀다. 앞의 것은 전파 제외가 옳고, 뒤의 것은 전파도 다운로드도
    # 막히면 막다른 길이 된다. 처방은 전파 경로 신설이 아니라 **공유 스토리지 + manifest 포인터**다
    # (2026-09-03 사용자 결정 — NAS 배치). 그러면 전파 자체가 불필요해진다.
    #
    # 이 술어는 그 결정이 다음 노드에서 조용히 되돌아가는 것을 막는다. **차단이 아니라 경고**다 —
    # 서브가 없는 순수 단일노드에서는 노드-로컬 경로가 정상이기 때문이다.
    tik = res.get("tiktoken_host_path") or ""
    if tik and len(res["nodes"]) > 1:
        _local_of = [n.get("work_dir") or "" for n in res["nodes"] if isinstance(n, dict)]
        _owner = next((w for w in _local_of if w and (tik == w or tik.startswith(w.rstrip("/") + "/"))), None)
        if _owner:
            node_warnings.append(
                "tiktoken_host_path=%r 가 노드 작업경로(%s) 안이다 — 사전적재 자산은 노드-로컬에 두면 "
                "다른 노드가 얻을 수 없다(서브는 curl/wget/hf deny). 공유 스토리지(nas_model_path 와 같은 "
                "마운트)로 옮기고 이 포인터를 그리로 돌려라." % (tik, _owner))

    res["node_warnings"] = node_warnings
    res["effective_model_source"] = {
        (node.get("role") or "node%d" % i): effective_model_source(node, ms)
        for i, node in enumerate(res["nodes"]) if isinstance(node, dict)
    }

    res["flag"] = True
    res["reason"] = "Flag valid (complete + branch_verified + HW사실 + model_source)"
    res["exit_code"] = EXIT_OK
    return res


def _load_manifest(path):
    import yaml
    with open(path, "r", encoding="utf-8") as f:
        return yaml.safe_load(f) or {}


# --- 호출자용 정본 redirect 템플릿 (Flag 미발급 시 페르소나가 출력) ---
REDIRECT_TEMPLATE = (
    "HW스캔이 덜 되어(Flag 미발행) HW 스펙(GPU·OS)을 알기 어려워, 모델의 정확한 서빙전략을 "
    "세우기 어렵습니다. terraforming_node 로 ① HW스캔 + ② 모델 다운로드 전략(관리 NAS 경로? "
    "컨테이너 임시 다운로드(컨테이너 down 시 삭제)? 특정 경로 저장·마운트?)을 먼저 정합시다."
)


def _self_test():
    cases = []

    def chk(name, man, topology, expect_flag, expect_exit):
        r = evaluate_contract(man, topology)
        ok = (r["flag"] == expect_flag) and (r["exit_code"] == expect_exit)
        cases.append((name, ok, r["reason"], r.get("manifest_tp")))
        return ok

    base_ok = {
        "topology": "single", "gpus_per_node": 1, "model_source": "managed",
        "nas_model_path": "/mnt/models", "nodes": [],
        "terraforming": {"complete": True, "branch_verified": True},
    }
    chk("valid-single", dict(base_ok), "single", True, EXIT_OK)

    multi_ok = {
        "topology": "multi", "gpus_per_node": 1, "model_source": "ephemeral",
        "nodes": [{"role": "main"}, {"role": "sub"}],
        "terraforming": {"complete": True, "branch_verified": True},
    }
    chk("valid-multi", multi_ok, "multi", True, EXIT_OK)

    chk("no-flag-complete-false",
        {**base_ok, "terraforming": {"complete": False, "branch_verified": True}},
        "single", False, EXIT_NO_FLAG)
    chk("no-flag-missing-block",
        {k: v for k, v in base_ok.items() if k != "terraforming"},
        "single", False, EXIT_NO_FLAG)
    chk("no-flag-branch-unverified",
        {**base_ok, "terraforming": {"complete": True, "branch_verified": False}},
        "single", False, EXIT_NO_FLAG)
    chk("flag-but-missing-gpus",
        {**base_ok, "gpus_per_node": None},
        "single", False, EXIT_MISSING_FIELD)
    chk("flag-but-bad-model-source",
        {**base_ok, "model_source": "nas"},
        "single", False, EXIT_MISSING_FIELD)
    chk("flag-but-gpus-not-int(B4)",
        {**base_ok, "gpus_per_node": "abc"},
        "single", False, EXIT_MISSING_FIELD)
    chk("flag-but-gpus-zero(B4)",
        {**base_ok, "gpus_per_node": 0},
        "single", False, EXIT_MISSING_FIELD)

    # per-node model_source override 회귀(plan_26070809_46_57 §4.4 — valid-override · bad-override).
    single_sub_control = {
        "topology": "single", "gpus_per_node": 1, "model_source": "managed",
        "nas_model_path": "/mnt/models",
        "nodes": [{"role": "main"}, {"role": "sub", "model_source": "ephemeral"}],
        "terraforming": {"complete": True, "branch_verified": True},
    }
    r = evaluate_contract(single_sub_control, "single")
    ok = (r["flag"] is True and r.get("effective_model_source", {}).get("sub") == "ephemeral"
          and r.get("effective_model_source", {}).get("main") == "managed")
    cases.append(("valid-override(single sub-control)", ok, r["reason"], r.get("manifest_tp")))

    bad_override = {
        "topology": "single", "gpus_per_node": 1, "model_source": "managed",
        "nas_model_path": "/mnt/models",
        "nodes": [{"role": "main"}, {"role": "sub", "model_source": "nas-bogus"}],
        "terraforming": {"complete": True, "branch_verified": True},
    }
    r = evaluate_contract(bad_override, "single")
    ok = (r["flag"] is False and r["exit_code"] == EXIT_MISSING_FIELD)
    cases.append(("bad-override(invalid per-node model_source)", ok, r["reason"], r.get("manifest_tp")))

    # 사전적재 자산 경로 술어(2026-09-03): 노드-로컬 → WARN · 공유 → 무경고 · 단일노드 → 무경고
    _base_nodes = [{"role": "main", "work_dir": "/home/u/proj"}, {"role": "sub", "work_dir": "/home/u/proj"}]
    _tik_local = {"topology": "single", "model_source": "managed", "nas_model_path": "/mnt/models",
                  "gpus_per_node": 1, "tiktoken_host_path": "/home/u/proj/tiktoken_cache",
                  "nodes": _base_nodes,
                  "terraforming": {"complete": True, "branch_verified": True}}
    r = evaluate_contract(_tik_local, "single")
    cases.append(("사전적재 자산 노드-로컬 → WARN",
                  any("tiktoken_host_path" in w for w in r.get("node_warnings", [])),
                  r["reason"], None))
    _tik_shared = dict(_tik_local, tiktoken_host_path="/mnt/models/OpenAI/harmony_tokenizer")
    r = evaluate_contract(_tik_shared, "single")
    cases.append(("공유 스토리지 → 무경고(대조군)",
                  not any("tiktoken_host_path" in w for w in r.get("node_warnings", [])),
                  r["reason"], None))
    _tik_solo = dict(_tik_local, nodes=[{"role": "main", "work_dir": "/home/u/proj"}])
    r = evaluate_contract(_tik_solo, "single")
    cases.append(("서브 없는 단일노드 → 무경고(로컬이 정상)",
                  not any("tiktoken_host_path" in w for w in r.get("node_warnings", [])),
                  r["reason"], None))

    multi_override_warn = {
        "topology": "multi", "gpus_per_node": 1, "model_source": "ephemeral",
        "nodes": [{"role": "main"}, {"role": "sub", "model_source": "managed"}],
        "terraforming": {"complete": True, "branch_verified": True},
    }
    r = evaluate_contract(multi_override_warn, "multi")
    ok = (r["flag"] is True and len(r.get("node_warnings", [])) >= 1)
    cases.append(("multi-override(WARN, 무시 대상)", ok, r["reason"], r.get("manifest_tp")))

    # TP 파생 회귀 (roofline 패턴)
    # δ1-1 회귀: single 은 nodes 가 **차 있어도** 노드 배수를 걷지 않는다. sub-control 확장으로
    # 서브가 등록된 single manifest 가 1-GPU 노드에 TP=2 를 요구하던 실버그 — 아래 두 single-with-sub
    # 케이스가 정확히 그 구멍이며, 종전 케이스는 nodes:[] 만 봐서 이를 통과시켰다.
    tp_cases = [
        ("tp-single-empty", {"gpus_per_node": 4, "nodes": []}, "single", 4),
        ("tp-single-with-sub(δ1-1)", {"gpus_per_node": 1,
                                      "nodes": [{"role": "main"}, {"role": "sub"}]}, "single", 1),
        ("tp-single-with-sub-4gpu", {"gpus_per_node": 4,
                                     "nodes": [{"role": "main"}, {"role": "sub"}]}, "single", 4),
        ("tp-multi-2x1", {"gpus_per_node": 1, "nodes": [1, 2]}, "multi", 2),
        ("tp-multi-2x2", {"gpus_per_node": 2, "nodes": [1, 2]}, "multi", 4),
        ("tp-empty-multi-none", {"gpus_per_node": 1, "nodes": []}, "multi", None),
    ]
    for name, man, topo, expect in tp_cases:
        got = manifest_tp(man, topo)
        cases.append((name, got == expect, "tp=%r (기대 %r)" % (got, expect), got))
    # B4 음성대조: gpus_per_node 부재/비정수는 1 로 떨어지지 않고 ValueError 로 멈춘다
    for name, man in (("tp-missing-gpus-raises(B4)", {"nodes": []}),
                      ("tp-str-gpus-raises(B4)", {"gpus_per_node": "two", "nodes": []})):
        try:
            got = manifest_tp(man, "single")
            cases.append((name, False, "예외 없이 tp=%r 반환(침묵 폴백 생존)" % (got,), got))
        except ValueError as e:
            cases.append((name, True, "ValueError: %s" % e, None))

    passed = sum(1 for _, ok, _, _ in cases if ok)
    for name, ok, reason, tp in cases:
        print("%s %s — %s" % ("PASS" if ok else "FAIL", name, reason))
    print("\n%d/%d self-test 통과" % (passed, len(cases)))
    return 0 if passed == len(cases) else 1


def main(argv=None):
    ap = argparse.ArgumentParser(description="테라포밍-완수 Flag 게이트 결정론 리더")
    ap.add_argument("--topology", choices=["single", "multi"], help="미지정 시 git 브랜치가 통로 선택")
    ap.add_argument("--repo", default=".", help="레포 루트(기본 .)")
    ap.add_argument("--require-flag", action="store_true",
                    help="Flag 미발급 시 비0종료(작업 스크립트 게이트용)")
    ap.add_argument("--json", action="store_true", help="계약 JSON 을 stdout 으로")
    ap.add_argument("--self-test", action="store_true", help="게이트/ TP 회귀(하드웨어 불요)")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()

    repo = os.path.abspath(args.repo)
    conflict = branch_topology_conflict(repo, args.topology)
    if conflict:
        if args.json:
            print(json.dumps({"flag": False, "reason": conflict, "topology": args.topology,
                              "redirect": REDIRECT_TEMPLATE}, ensure_ascii=False))
        else:
            print(conflict, file=sys.stderr)
        return EXIT_TOPOLOGY_MISMATCH
    topology = resolve_topology(repo, args.topology)
    if not topology:
        print("topology 불명 — --topology 명시 필요(git 브랜치가 single-node/multi-node 아님)", file=sys.stderr)
        return EXIT_USAGE

    mpath = os.path.join(repo, "output", topology, "manifest.yaml")
    if not os.path.isfile(mpath):
        msg = "manifest 부재: %s (테라포밍 미완 — info-only)" % mpath
        if args.json:
            print(json.dumps({"flag": False, "reason": msg, "topology": topology,
                              "redirect": REDIRECT_TEMPLATE}, ensure_ascii=False))
        else:
            print(msg, file=sys.stderr)
        return EXIT_NO_MANIFEST  # manifest 부재 = flag invalid (require-flag 무관 항상 비0; 조회는 --json 출력으로)

    try:
        man = _load_manifest(mpath)
    except Exception as e:  # noqa
        print("manifest 파싱 실패: %s" % e, file=sys.stderr)
        return EXIT_USAGE

    res = evaluate_contract(man, topology)
    res["manifest_path"] = mpath
    if not res["flag"]:
        res["redirect"] = REDIRECT_TEMPLATE

    if args.json:
        print(json.dumps(res, ensure_ascii=False, indent=2))
    else:
        if res["flag"]:
            print("Flag valid · topology=%s · gpus_per_node=%s · manifest_tp=%s · model_source=%s"
                  % (res["topology"], res["gpus_per_node"], res["manifest_tp"], res["model_source"]))
        else:
            print("Flag 미발급: %s" % res["reason"], file=sys.stderr)

    return res["exit_code"] if args.require_flag else (EXIT_OK if res["flag"] else res["exit_code"])


if __name__ == "__main__":
    sys.exit(main())
