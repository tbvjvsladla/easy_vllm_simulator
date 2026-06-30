#!/usr/bin/env python3
"""manifest_contract.py — 테라포밍-완수 Flag 게이트의 결정론 리더 (plan_2026063018_1).

헌법 §테라포밍-완수 Flag 게이트 따름정리의 **결정론 백스톱**. 3 런타임 스킬
(upstream-version-watch · vllm-recipe-explorer · adversarial-benchmark)의 *작업 스크립트*가
진입 시 이걸 호출해 Flag(terraforming.complete)를 확인한다 — 미발급이면 비0종료(작업 거부 → 에이전트 info-only).

설계:
- 단일계약: `output/<topology>/manifest.yaml` 를 읽는다(topology = 현재 git 브랜치가 선택하는 *통로*; --topology 로 override).
- **보수적**: `terraforming.complete == true` AND `terraforming.branch_verified == true` AND 필수 HW사실(topology·gpus_per_node) 존재해야 통과(fail-closed-on-flag).
- **TP**(manifest 파생): nodes 있으면 len(nodes)×gpus_per_node · nodes 비면 topology=single→1 · 그 외 None
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


def manifest_tp(man, topology):
    """manifest 파생 TP (roofline.py:196-209 패턴). config override·최종폴백 1 은 호출자 책임.
    nodes 있으면 len(nodes)×gpus_per_node · nodes 비면 topology=single→1 · 그 외 None."""
    gpus = man.get("gpus_per_node") or 1
    try:
        gpus = max(1, int(gpus))
    except (TypeError, ValueError):
        gpus = 1
    nodes = man.get("nodes") or []
    if nodes:
        return max(1, len(nodes)) * gpus
    if (topology or "").startswith("single"):
        return 1
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
    res["manifest_tp"] = manifest_tp(man, topology)

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

    ms = man.get("model_source")
    if ms not in VALID_MODES:
        res["reason"] = "model_source 미설정/오류: %r (기대 %s)" % (ms, "|".join(VALID_MODES))
        res["exit_code"] = EXIT_MISSING_FIELD
        return res

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

    # TP 파생 회귀 (roofline 패턴)
    tp_cases = [
        ("tp-single-empty", {"gpus_per_node": 4, "nodes": []}, "single", 1),
        ("tp-multi-2x1", {"gpus_per_node": 1, "nodes": [1, 2]}, "multi", 2),
        ("tp-multi-2x2", {"gpus_per_node": 2, "nodes": [1, 2]}, "multi", 4),
        ("tp-empty-multi-none", {"gpus_per_node": 1, "nodes": []}, "multi", None),
    ]
    for name, man, topo, expect in tp_cases:
        got = manifest_tp(man, topo)
        cases.append((name, got == expect, "tp=%r (기대 %r)" % (got, expect), got))

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
