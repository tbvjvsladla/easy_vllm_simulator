#!/usr/bin/env python3
"""node_role_contract.py — **토폴로지 축 노드 계약**의 단일 소유자 (헌법 §불변식 A 의 결정론 배선).

정본 선언: `.claude/skills/terraforming_node/SKILL.md` §2.7.0 · §2.7.6.

헌법 불변식 A 는 *왜*를 말한다 — "노드 정체성·자율성의 제1축은 토폴로지다. 멀티 sub 는 Ray 워커
(위치=rank·집단 연산 ABI 동기)이고, 싱글 sub 는 A2A 원격 에이전트(정체성=AgentCard·자율 처리)다."
이 파일은 그 문장을 **판정 가능한 술어**로 바꾼다. 없으면 각 소비자가 `role: sub` 존재만 보고
제각기 추론하고, 그 결과가 2026-08-22 진단의 5증상이었다(`plan_26082214` §0):

    output/single/manifest.yaml 의 `- role: sub`  →  sync_to_sub 가 배달 평면을 켬(오판)
                                                  →  멀티의 "빌드→전파·버전싱크" 개념이 싱글로 유입

**핵심 판정 3종** (모두 `(value, source)` 쌍으로 답한다 — 헌법 §결정론 규율 "출처 표시"):

  1. `resolve_sub_mode(topology, declared)`  — 이 sub 는 어느 정체성 계약인가
  2. `resolve_rank(topology, nodes, role)`   — 집단 안의 위치(멀티만 정의됨)
  3. `delivery_plane(topology, sub_mode)`    — 빌드킷 배달(rsync) 평면이 켜지는가

**sub_mode 는 파생값이다 — 선언은 tripwire다.** 불변식 A 의 사상은 1:1(single↔a2a-agent ·
multi↔ray-worker)이므로 topology 만 알면 계산된다. 그래서 manifest 의 `nodes[].sub_mode` 는
**필수 손저작 값이 아니라**(그건 헌법 §4종 안티패턴의 "파생 가능한데 손으로 적은 것" = 하드코딩
결함이다) **선택적 선언**이고, 선언되면 파생값과 일치해야 한다 — 어긋나면 fail-closed 다.
즉 선언의 값어치는 "값을 알려주는 것"이 아니라 **혼동 지점에서 의미를 읽히게 하고 변경 시 리뷰를
강제하는 것**(판정표의 정당 칸 = tripwire)이다.

CLI:
    node_role_contract.py evaluate --manifest <path> [--topology single|multi]
                                   [--role sub] [--field <k>] [--format json|value]
    node_role_contract.py --self-test

    --field ∈ {sub_mode, rank, identity_authority, delivery_plane, topology}
    --format value 는 셸 소비자용(한 줄). 위반이 있으면 값을 찍지 않고 종료코드로 말한다.

종료코드:
    0 = 계약 정합 · 2 = 사용오류/파싱 실패 · 5 = 계약 위반(fail-closed)

벽시계를 읽지 않는다. 네트워크·서브 디스크를 만지지 않는다(순수 판정).
"""
import argparse
import json
import os
import sys

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_CONTRACT_VIOLATION = 5

TOPOLOGIES = ("single", "multi")

# ── 닫힌 목록(tripwire) ────────────────────────────────────────────────────────
# 불변식 A 의 1:1 사상. 값을 늘리려면 SKILL.md §2.7.6 표와 이 dict 가 **함께** 바뀐다 —
# 손으로 적었지만 파생 불가한 정의역이므로 헌법 §판정표의 "tripwire" 칸(정당)이다.
SUB_MODE_BY_TOPOLOGY = {
    "single": "a2a-agent",   # A2A 원격 에이전트: 불투명 피어·자율 처리·client↔server (grounding [3])
    "multi": "ray-worker",   # Ray 워커: 제어평면은 head 종속·데이터평면은 집단 연산 (grounding [1][2])
}
SUB_MODES = tuple(sorted(set(SUB_MODE_BY_TOPOLOGY.values())))

# 정체성 **권위**가 어디에 있는가(누가 "이 노드는 누구인가"를 답하는가).
IDENTITY_AUTHORITY_BY_TOPOLOGY = {
    "single": "agent-card",              # Agent_Card.json (정체성+능력+엔드포인트)
    "multi": "manifest-rank-and-role",   # manifest nodes[] 인덱스(rank) + role 슬러그
}

# 빌드킷 배달(rsync) 평면이 존재하는 이유는 **집단 연산 ABI 정합**이다 — 같은 바이너리·드라이버로
# 같은 collective 에 참여해야 하므로. 자율 A2A 에이전트는 자기 빌드킷을 자율 저작하므로 대상이 아니다.
DELIVERY_PLANE_SUB_MODES = ("ray-worker",)

MAIN_ROLE = "main"
SUB_ROLE = "sub"


class ContractViolation(Exception):
    """계약 위반(fail-closed). code 는 안정 문자열이며 하류 라우팅 키다."""

    def __init__(self, code, message, field=None):
        super().__init__(message)
        self.code = code
        self.message = message
        self.field = field

    def as_dict(self):
        return {"code": self.code, "message": self.message, "field": self.field}


# --------------------------------------------------------------------------------------
# I/O — YAML 로더의 **단일 소유자**
#   `-S` 로 실행되면 활성 venv 의 site-packages 가 sys.path 에서 빠진다. 선언된 의존 경계만
#   복원하고 사용자/전역 패키지 디렉터리는 더하지 않는다.
#   ⚠ 같은 shim 이 `staleness_gate._load_yaml` 에도 있다 — **의도된 비결합**이다(조건부 preflight
#   게이트가 이 파일의 부재로 죽으면 안 된다). 합칠 신호는 두 파서의 판정이 갈리는 순간이다.
# --------------------------------------------------------------------------------------
def load_yaml(path):
    try:
        import yaml
    except ModuleNotFoundError:
        version = "python%d.%d" % (sys.version_info.major, sys.version_info.minor)
        venv_site = os.path.join(os.path.dirname(os.path.dirname(sys.executable)),
                                 "lib", version, "site-packages")
        if os.path.isdir(venv_site) and venv_site not in sys.path:
            sys.path.insert(0, venv_site)
        import yaml
    with open(path, "r", encoding="utf-8") as handle:
        return yaml.safe_load(handle)


def manifest_path_for(repo, topology):
    return os.path.join(repo, "output", topology, "manifest.yaml")


# --------------------------------------------------------------------------------------
# 판정 1 — sub_mode (이 sub 는 어느 정체성 계약인가)
# --------------------------------------------------------------------------------------
def assert_topology(topology):
    if topology not in TOPOLOGIES:
        raise ContractViolation(
            "TOPOLOGY_UNKNOWN",
            "topology=%r 는 선언 어휘 %r 밖이다 — 토폴로지는 인터뷰로만 확정된다"
            "(브랜치 추론 ✗ · SKILL.md §0.5.3)." % (topology, list(TOPOLOGIES)),
            field="topology")
    return topology


def resolve_sub_mode(topology, declared=None):
    """→ {"value": <sub_mode>, "source": "derived-from-topology"|"declared-and-agrees"}"""
    assert_topology(topology)
    derived = SUB_MODE_BY_TOPOLOGY[topology]
    if declared is None or (isinstance(declared, str) and not declared.strip()):
        return {"value": derived, "source": "derived-from-topology"}
    if not isinstance(declared, str):
        raise ContractViolation(
            "SUB_MODE_UNKNOWN",
            "nodes[sub].sub_mode 는 문자열이어야 한다(got %r)." % (declared,),
            field="nodes[sub].sub_mode")
    declared = declared.strip()
    if declared not in SUB_MODES:
        raise ContractViolation(
            "SUB_MODE_UNKNOWN",
            "nodes[sub].sub_mode=%r 는 어휘 %r 밖이다." % (declared, list(SUB_MODES)),
            field="nodes[sub].sub_mode")
    if declared != derived:
        raise ContractViolation(
            "SUB_MODE_TOPOLOGY_CONFLICT",
            "topology=%s 는 sub_mode=%s 를 함의하는데 선언은 %r 이다 — 불변식 A 위반"
            "(한 스킴의 role:sub 로 두 정체성을 덮지 않는다)." % (topology, derived, declared),
            field="nodes[sub].sub_mode")
    return {"value": declared, "source": "declared-and-agrees"}


# --------------------------------------------------------------------------------------
# 판정 2 — rank (집단 안의 위치). **멀티에서만 정의된다.**
# --------------------------------------------------------------------------------------
def resolve_rank(topology, nodes, role):
    """→ {"value": <int|None>, "source": ...}

    멀티: rank = `nodes[]` 배열 인덱스(NCCL "0..n-1 의 고유 rank" 차용 — grounding [1]).
    싱글: rank 는 **정의되지 않는다**. None 을 내되 출처 필드로 그 사실을 남긴다(음성정직) —
          조용한 0 은 "싱글 sub 가 집단의 0번"이라는 거짓말이 된다.
    """
    assert_topology(topology)
    if topology == "single":
        return {"value": None, "source": "not-applicable:single-a2a-agent"}

    if not isinstance(nodes, list) or not nodes:
        raise ContractViolation(
            "RANK_UNRESOLVED",
            "topology=multi 인데 nodes[] 가 비었다 — rank 를 해소할 수 없다.",
            field="nodes")
    hits = [i for i, n in enumerate(nodes)
            if isinstance(n, dict) and n.get("role") == role]
    if not hits:
        raise ContractViolation(
            "RANK_UNRESOLVED",
            "nodes[] 에 role=%r 항목이 없다 — rank 를 해소할 수 없다." % (role,),
            field="nodes")
    if len(hits) > 1:
        # ⚠ 알려진 경계: 현행 role 어휘는 {main, sub} 닫힌 목록이라 **서브가 2대 이상이면**
        #   슬러그가 겹친다(node_id 도 겹친다 — 로그 트리 혼합). 조용히 첫 항목을 고르지 않는다.
        #   N>2 를 지원하려면 role 어휘를 유일 슬러그로 넓혀야 하고, 그 어휘는 세 곳이 공동
        #   소유한다: 이 파일 · scan_node.py 의 manifest 게이트 · node_identity.sh 의 NI_MAIN_ROLE.
        raise ContractViolation(
            "RANK_AMBIGUOUS_ROLE",
            "nodes[] 에 role=%r 이 %d개다 — rank·node_id 가 겹친다. N>2 는 유일 슬러그가 "
            "필요하며 어휘 확장은 node_role_contract·scan_node·node_identity 세 곳이 함께 "
            "바뀐다." % (role, len(hits)),
            field="nodes")
    return {"value": hits[0], "source": "manifest-nodes-index"}


# --------------------------------------------------------------------------------------
# 판정 3 — 정체성 권위 · 배달 평면
# --------------------------------------------------------------------------------------
def identity_authority(topology):
    assert_topology(topology)
    return {"value": IDENTITY_AUTHORITY_BY_TOPOLOGY[topology],
            "source": "constitution-invariant-A"}


def delivery_plane(topology, sub_mode):
    """빌드킷 배달(rsync) 평면 활성 여부. → {"value": "active"|"dormant", "source": ...}

    **종전 술어와의 차이가 이 파일의 존재 이유다.** `sync_to_sub.sh:_single_extension_active` 는
    `role: sub` 의 *존재*만 보고 켰다. 존재는 정체성을 말하지 않는다 — 두 종류의 sub 가 같은
    단어를 쓰기 때문이다. 이제 판정 입력은 **정체성 계약(sub_mode)** 이다.
    """
    assert_topology(topology)
    if sub_mode not in SUB_MODES:
        raise ContractViolation(
            "SUB_MODE_UNKNOWN",
            "sub_mode=%r 는 어휘 %r 밖이다." % (sub_mode, list(SUB_MODES)),
            field="nodes[sub].sub_mode")
    active = sub_mode in DELIVERY_PLANE_SUB_MODES
    return {"value": "active" if active else "dormant",
            "source": "sub-mode:%s" % sub_mode}


# --------------------------------------------------------------------------------------
# manifest 전체 평가 (예외를 삼키지 않고 **모아서** 보고 — 게이트 방향은 항상 fail-closed)
# --------------------------------------------------------------------------------------
def _sub_node(nodes):
    for node in (nodes if isinstance(nodes, list) else []):
        if isinstance(node, dict) and node.get("role") == SUB_ROLE:
            return node
    return None


def evaluate_manifest(doc, topology=None, role=SUB_ROLE):
    """manifest dict → 계약 평가. 절대 raise 하지 않는다(위반은 violations[] 로).

    반환 키: topology · sub_present · sub_mode · rank · identity_authority ·
             delivery_plane · notes[] · violations[]
    """
    doc = doc if isinstance(doc, dict) else {}
    topo = topology or doc.get("topology")
    out = {
        "topology": topo,
        "sub_present": False,
        "sub_mode": None,
        "rank": None,
        "identity_authority": None,
        # fail-closed 기본값: 무엇 하나라도 못 정하면 배달은 열리지 않는다.
        "delivery_plane": {"value": "dormant", "source": "fail-closed:unevaluated"},
        "notes": [],
        "violations": [],
    }

    def record(exc):
        out["violations"].append(exc.as_dict())

    try:
        assert_topology(topo)
    except ContractViolation as exc:
        record(exc)
        return out

    out["identity_authority"] = identity_authority(topo)

    nodes = doc.get("nodes")
    if not isinstance(nodes, list):
        nodes = []
    sub = _sub_node(nodes)
    out["sub_present"] = sub is not None

    if nodes:
        main_hits = [i for i, n in enumerate(nodes)
                     if isinstance(n, dict) and n.get("role") == MAIN_ROLE]
        if len(main_hits) == 1 and main_hits[0] != 0 and topo == "multi":
            # 위반은 아니다 — Ray/NCCL 은 head 가 배열 첫 항목일 것을 요구하지 않는다.
            # 다만 관례(head=rank 0)와 어긋나면 사람이 알아야 하므로 침묵하지 않는다.
            out["notes"].append(
                "nodes[] 의 role:main 이 인덱스 %d 다(관례상 head=rank 0). rank 는 "
                "인덱스 그대로 부여된다." % main_hits[0])

    if sub is None:
        # 서브 미등록 = dormant. 이것은 정상 상태이며 위반이 아니다.
        out["delivery_plane"] = {"value": "dormant", "source": "no-sub-registered"}
        out["rank"] = {"value": None, "source": "not-applicable:no-sub-registered"}
        return out

    try:
        out["sub_mode"] = resolve_sub_mode(topo, sub.get("sub_mode"))
    except ContractViolation as exc:
        record(exc)
        return out          # sub_mode 미확정 → 배달 평면은 fail-closed dormant 유지

    try:
        out["rank"] = resolve_rank(topo, nodes, role)
    except ContractViolation as exc:
        record(exc)

    try:
        out["delivery_plane"] = delivery_plane(topo, out["sub_mode"]["value"])
    except ContractViolation as exc:
        record(exc)
    return out


# --------------------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------------------
FIELDS = ("sub_mode", "rank", "identity_authority", "delivery_plane", "topology")


def _field_value(result, field):
    if field == "topology":
        return result.get("topology")
    node = result.get(field)
    if node is None:
        return ""
    if isinstance(node, dict):
        value = node.get("value")
        return "" if value is None else value
    return node


def cmd_evaluate(args):
    path = args.manifest
    if not path:
        if not args.topology:
            print("[node-role] FAIL: --manifest 또는 --topology 중 하나는 있어야 한다.",
                  file=sys.stderr)
            return EXIT_USAGE
        path = manifest_path_for(args.repo, args.topology)
    if not os.path.isfile(path):
        print("[node-role] FAIL: manifest 부재: %s" % path, file=sys.stderr)
        return EXIT_USAGE
    try:
        doc = load_yaml(path)
    except Exception as exc:                                   # noqa: BLE001 -- 파싱 실패는 사용오류
        print("[node-role] FAIL: manifest 파싱 실패(%s): %s" % (path, exc), file=sys.stderr)
        return EXIT_USAGE

    result = evaluate_manifest(doc, topology=args.topology, role=args.role)
    result["manifest_path"] = path

    if args.format == "json" and not args.field:
        print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
    elif args.field:
        if result["violations"]:
            for v in result["violations"]:
                print("[node-role] VIOLATION %s: %s" % (v["code"], v["message"]), file=sys.stderr)
        elif args.format == "value":
            print(_field_value(result, args.field))
        else:
            print(json.dumps({args.field: result.get(args.field)},
                             ensure_ascii=False, sort_keys=True))
    for note in result["notes"]:
        print("[node-role] note: %s" % note, file=sys.stderr)
    if result["violations"]:
        if args.format == "json" and not args.field:
            pass                                              # 이미 JSON 에 담겼다
        else:
            for v in result["violations"]:
                if not args.field:
                    print("[node-role] VIOLATION %s: %s" % (v["code"], v["message"]),
                          file=sys.stderr)
        return EXIT_CONTRACT_VIOLATION
    return EXIT_OK


# --------------------------------------------------------------------------------------
# 자기검사 — 회귀 고정 (하드웨어·네트워크 불요)
# --------------------------------------------------------------------------------------
def _self_test():
    failures = []

    def chk(name, cond, detail=""):
        print("%s %s%s" % ("PASS" if cond else "FAIL", name,
                           "" if cond else " — %s" % detail))
        if not cond:
            failures.append(name)

    def violation_code(fn, *a, **kw):
        try:
            fn(*a, **kw)
        except ContractViolation as exc:
            return exc.code
        return None

    # ── sub_mode 파생·선언 ────────────────────────────────────────────────────
    r = resolve_sub_mode("single")
    chk("single 미선언 → a2a-agent 파생",
        r == {"value": "a2a-agent", "source": "derived-from-topology"}, r)
    r = resolve_sub_mode("multi")
    chk("multi 미선언 → ray-worker 파생",
        r == {"value": "ray-worker", "source": "derived-from-topology"}, r)
    r = resolve_sub_mode("single", "a2a-agent")
    chk("일치 선언 → declared-and-agrees(출처 구분)",
        r == {"value": "a2a-agent", "source": "declared-and-agrees"}, r)
    chk("single + ray-worker 선언 → 불변식 A 충돌 fail-closed",
        violation_code(resolve_sub_mode, "single", "ray-worker") == "SUB_MODE_TOPOLOGY_CONFLICT")
    chk("multi + a2a-agent 선언 → 충돌 fail-closed",
        violation_code(resolve_sub_mode, "multi", "a2a-agent") == "SUB_MODE_TOPOLOGY_CONFLICT")
    chk("어휘 밖 선언 → SUB_MODE_UNKNOWN",
        violation_code(resolve_sub_mode, "multi", "ray_worker") == "SUB_MODE_UNKNOWN")
    chk("빈 선언은 미선언과 같다(공백 허용)",
        resolve_sub_mode("multi", "   ")["source"] == "derived-from-topology")
    chk("토폴로지 미선언/추정 → TOPOLOGY_UNKNOWN(브랜치 추론 ✗)",
        violation_code(resolve_sub_mode, "auto") == "TOPOLOGY_UNKNOWN")

    # ── rank ─────────────────────────────────────────────────────────────────
    nodes = [{"role": "main", "host": "192.0.2.10"}, {"role": "sub", "host": "192.0.2.11"}]
    chk("multi rank(main)=0", resolve_rank("multi", nodes, "main")
        == {"value": 0, "source": "manifest-nodes-index"})
    chk("multi rank(sub)=1", resolve_rank("multi", nodes, "sub")
        == {"value": 1, "source": "manifest-nodes-index"})
    r = resolve_rank("single", nodes, "sub")
    chk("single rank 은 None + 음성정직 출처",
        r == {"value": None, "source": "not-applicable:single-a2a-agent"}, r)
    chk("multi 에 해당 role 부재 → RANK_UNRESOLVED",
        violation_code(resolve_rank, "multi", [{"role": "main", "host": "h"}], "sub")
        == "RANK_UNRESOLVED")
    chk("role 슬러그 중복 → RANK_AMBIGUOUS_ROLE(조용한 첫 항목 선택 ✗)",
        violation_code(resolve_rank, "multi",
                       [{"role": "main"}, {"role": "sub"}, {"role": "sub"}], "sub")
        == "RANK_AMBIGUOUS_ROLE")

    # ── 정체성 권위 · 배달 평면 ───────────────────────────────────────────────
    chk("single 정체성 권위 = agent-card",
        identity_authority("single")["value"] == "agent-card")
    chk("multi 정체성 권위 = manifest-rank-and-role",
        identity_authority("multi")["value"] == "manifest-rank-and-role")
    chk("★ single(a2a-agent) 배달 평면 = dormant (plan_26082214 §4.4 회귀핀)",
        delivery_plane("single", "a2a-agent")["value"] == "dormant")
    chk("multi(ray-worker) 배달 평면 = active",
        delivery_plane("multi", "ray-worker")["value"] == "active")

    # ── manifest 전체 평가 ───────────────────────────────────────────────────
    single_with_sub = {"topology": "single", "nodes": nodes}
    r = evaluate_manifest(single_with_sub)
    chk("★ single + nodes[main,sub] 미선언 → 위반 0 · 배달 dormant (증상1·5 회귀핀)",
        r["violations"] == [] and r["delivery_plane"]["value"] == "dormant"
        and r["sub_mode"]["value"] == "a2a-agent" and r["rank"]["value"] is None, r)

    multi_with_sub = {"topology": "multi", "nodes": nodes}
    r = evaluate_manifest(multi_with_sub)
    chk("multi + nodes[main,sub] → 배달 active · rank(sub)=1",
        r["violations"] == [] and r["delivery_plane"]["value"] == "active"
        and r["rank"]["value"] == 1, r)

    bad = {"topology": "single",
           "nodes": [{"role": "main"}, {"role": "sub", "sub_mode": "ray-worker"}]}
    r = evaluate_manifest(bad)
    chk("single 에 ray-worker 선언 → 위반 보고 + 배달 fail-closed dormant",
        [v["code"] for v in r["violations"]] == ["SUB_MODE_TOPOLOGY_CONFLICT"]
        and r["delivery_plane"]["value"] == "dormant", r)

    r = evaluate_manifest({"topology": "single", "nodes": []})
    chk("서브 미등록 → dormant(no-sub-registered) · 위반 0",
        r["violations"] == [] and r["delivery_plane"]["source"] == "no-sub-registered", r)

    r = evaluate_manifest({"nodes": nodes})
    chk("topology 부재 → TOPOLOGY_UNKNOWN + 배달 fail-closed",
        [v["code"] for v in r["violations"]] == ["TOPOLOGY_UNKNOWN"]
        and r["delivery_plane"]["value"] == "dormant", r)

    r = evaluate_manifest({"topology": "multi",
                           "nodes": [{"role": "sub"}, {"role": "main"}]})
    chk("multi 에서 main 이 인덱스 0 이 아니면 note(위반 ✗)",
        r["violations"] == [] and len(r["notes"]) == 1, r)

    chk("결정론(동일 입력 → 동일 출력)",
        json.dumps(evaluate_manifest(single_with_sub), sort_keys=True)
        == json.dumps(evaluate_manifest(single_with_sub), sort_keys=True))

    total = 24
    passed = total - len(failures)
    print("--- %d/%d PASS" % (passed, total))
    return 0 if not failures else 1


def main():
    ap = argparse.ArgumentParser(description="토폴로지 축 노드 계약(불변식 A)의 결정론 판정기")
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    ev = sub.add_parser("evaluate", help="manifest 의 노드 계약을 평가")
    ev.add_argument("--manifest", default=None)
    ev.add_argument("--repo", default=".")
    ev.add_argument("--topology", choices=list(TOPOLOGIES), default=None,
                    help="manifest 의 topology 를 덮어쓴다(통로 선택 겸용).")
    ev.add_argument("--role", default=SUB_ROLE, help="rank 를 물어볼 role 슬러그(기본 sub)")
    ev.add_argument("--field", choices=list(FIELDS), default=None)
    ev.add_argument("--format", choices=["json", "value"], default="json")
    args = ap.parse_args()

    if args.self_test:
        return _self_test()
    if args.cmd == "evaluate":
        return cmd_evaluate(args)
    ap.print_help()
    return EXIT_USAGE


if __name__ == "__main__":
    sys.exit(main())
