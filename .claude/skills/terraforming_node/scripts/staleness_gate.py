#!/usr/bin/env python3
"""staleness_gate.py — 테라포밍 조건부 preflight 의 **결정론 트리거** (plan_26072506 Phase 5).

terraforming_node 는 매 작업마다 도는 단계가 아니라 **조건부 preflight** 다(plan §4.1):
하드웨어/네트워크/manifest 가 바뀌었거나 attestation 이 staleness 기준을 넘었을 때만 재진입한다.
"바뀌었나?"의 판정은 페르소나 감(勘)이 아니라 이 스크립트가 한다 — 같은 입력이면 같은 출력,
이유는 안정된 reason code 로 나온다(헌법 §"버전 문자열 해소는 결정론 스크립트로").

3 축(각 축 독립 · OR 결합):
  1. **manifest 축** — `output/<topology>/manifest.yaml` 부재 → MANIFEST_ABSENT
                      · terraforming.complete/branch_verified != true → FLAG_ABSENT
  2. **HW 축**      — `--observed <scan json>` HW 사실 ↔ manifest attestation 불일치 → HW_DRIFT
                      (비교 필드 고정 — 드리프트 필드명을 그대로 보고)
  3. **age 축**     — `--now` 기준 `terraforming.scanned_at` 경과일 > `--max-age-days` → ATTESTATION_STALE

**벽시계를 읽지 않는다.** 현재 시각은 항상 `--now YYYY-MM-DD`(또는 YYYYMMDDHH)로 주입받는다 —
주입이 없으면 age 축을 조용히 통과시키지 않고 `age_check: "skipped:no-now"` 로 표기한다(음성정직).
`--observed` 미제공도 `hw_check: "skipped:no-observed"` 로 표기한다.

CLI:
  staleness_gate.py [--topology single|multi] [--repo .] [--observed scan.json]
                    [--now YYYY-MM-DD] [--max-age-days N] [--json] [--self-test]
  exit 0 = preflight 불요(FRESH) · 4 = preflight 필요 · 2 = 사용오류
"""
import argparse
import datetime
import ipaddress
import json
import math
import os
import re
import sys

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_PREFLIGHT_REQUIRED = 4

REASON_MANIFEST_ABSENT = "MANIFEST_ABSENT"
REASON_FLAG_ABSENT = "FLAG_ABSENT"
REASON_HW_DRIFT = "HW_DRIFT"
REASON_ATTESTATION_STALE = "ATTESTATION_STALE"
REASON_MANIFEST_INVALID = "MANIFEST_INVALID"
REASON_OBSERVED_INVALID = "OBSERVED_INVALID"
REASON_FRESH = "FRESH"

REASON_CODES = (
    REASON_MANIFEST_ABSENT,
    REASON_FLAG_ABSENT,
    REASON_HW_DRIFT,
    REASON_ATTESTATION_STALE,
    REASON_MANIFEST_INVALID,
    REASON_OBSERVED_INVALID,
    REASON_FRESH,
)

# 비교 대상 고정(글롭 ✗ — 새 manifest 키가 조용히 판정에 끼어들지 않게).
HW_FIELDS = ("topology", "cpu_arch", "cuda_version", "gpus_per_node", "gpu_model")
INTERCONNECT_FIELDS = (
    "type", "hca_devices", "gid_index", "socket_iface",
    "bandwidth_gbps", "platform_preset",
)
NODE_FIELDS = ("role", "host")

TOKEN_RE = re.compile(r"^[A-Za-z0-9_.:-]+$")
CUDA_RE = re.compile(r"^[0-9]{2,4}$")
PRESET_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
HOST_LABEL_RE = re.compile(r"^[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?$")


def valid_host(value):
    if not isinstance(value, str) or not value or len(value) > 253:
        return False
    try:
        ipaddress.ip_address(value)
        return True
    except ValueError:
        labels = value[:-1].split(".") if value.endswith(".") else value.split(".")
        return bool(labels) and all(HOST_LABEL_RE.fullmatch(label) for label in labels)

DEFAULT_MAX_AGE_DAYS = 90  # policy lifecycle 재검토 주기와 동일(plan §4.6)


def validate_document(doc, kind, expected_topology=None):
    """Return sorted invalid field paths for a manifest/observed object."""
    if not isinstance(doc, dict):
        return ["$"]

    invalid = []
    topology_key = "topology" if kind == "manifest" else "topology_declared"
    topology = doc.get(topology_key, doc.get("topology"))
    if topology not in ("single", "multi"):
        invalid.append(topology_key)
    if kind == "manifest" and expected_topology is not None and topology != expected_topology:
        invalid.append("topology")

    for field in ("cpu_arch", "cuda_version", "gpu_model"):
        if not isinstance(doc.get(field), str) or not doc[field].strip():
            invalid.append(field)
    if isinstance(doc.get("cpu_arch"), str) and doc["cpu_arch"] not in ("aarch64", "x86_64", "amd64"):
        invalid.append("cpu_arch")
    if isinstance(doc.get("cuda_version"), str) and not CUDA_RE.fullmatch(doc["cuda_version"]):
        invalid.append("cuda_version")
    gpus = doc.get("gpus_per_node")
    if isinstance(gpus, bool) or not isinstance(gpus, int) or gpus < 1:
        invalid.append("gpus_per_node")

    if kind == "manifest":
        terraforming = doc.get("terraforming")
        if not isinstance(terraforming, dict):
            invalid.append("terraforming")
        elif parse_stamp(terraforming.get("scanned_at")) is None:
            invalid.append("terraforming.scanned_at")

    interconnect = doc.get("interconnect")
    if not isinstance(interconnect, dict):
        invalid.append("interconnect")
    else:
        if interconnect.get("type") not in ("generic-ethernet", "RoCE v2"):
            invalid.append("interconnect.type")
        hcas = interconnect.get("hca_devices")
        if (not isinstance(hcas, list)
                or any(not isinstance(item, str) or not TOKEN_RE.fullmatch(item) for item in hcas)):
            invalid.append("interconnect.hca_devices")
        gid_index = interconnect.get("gid_index")
        gid_scalar_valid = (
            gid_index is None
            or (isinstance(gid_index, int) and not isinstance(gid_index, bool) and gid_index >= 0)
        )
        gid_list_valid = (
            isinstance(gid_index, list) and len(gid_index) >= 2
            and all(isinstance(item, int) and not isinstance(item, bool) and item >= 0
                    for item in gid_index)
            and gid_index == sorted(set(gid_index))
        )
        if "gid_index" not in interconnect or not (gid_scalar_valid or gid_list_valid):
            invalid.append("interconnect.gid_index")
        socket_iface = interconnect.get("socket_iface")
        if "socket_iface" not in interconnect or (
                socket_iface is not None
                and (not isinstance(socket_iface, str) or not TOKEN_RE.fullmatch(socket_iface))):
            invalid.append("interconnect.socket_iface")
        bandwidth = interconnect.get("bandwidth_gbps")
        if ("bandwidth_gbps" not in interconnect
                or (bandwidth is not None and (
                    isinstance(bandwidth, bool) or not isinstance(bandwidth, (int, float))
                    or not math.isfinite(bandwidth) or bandwidth < 0))):
            invalid.append("interconnect.bandwidth_gbps")
        preset = interconnect.get("platform_preset")
        if "platform_preset" not in interconnect or (
                preset is not None
                and (not isinstance(preset, str) or not PRESET_RE.fullmatch(preset))):
            invalid.append("interconnect.platform_preset")
        if interconnect.get("type") == "RoCE v2":
            if not isinstance(hcas, list) or not hcas:
                invalid.append("interconnect.hca_devices")
            if gid_index is None:
                invalid.append("interconnect.gid_index")
            if socket_iface is None:
                invalid.append("interconnect.socket_iface")

    if "nodes" in doc:
        nodes = doc["nodes"]
        if not isinstance(nodes, list):
            invalid.append("nodes")
        else:
            for index, node in enumerate(nodes):
                if not isinstance(node, dict):
                    invalid.append("nodes[%d]" % index)
                    continue
                for field in NODE_FIELDS:
                    if not isinstance(node.get(field), str) or not node[field].strip():
                        invalid.append("nodes[%d].%s" % (index, field))
                if isinstance(node.get("role"), str) and node["role"] not in ("main", "sub"):
                    invalid.append("nodes[%d].role" % index)
                if isinstance(node.get("host"), str) and not valid_host(node["host"]):
                    invalid.append("nodes[%d].host" % index)
    if kind == "manifest" and doc.get("topology") == "single":
        # 헌법 §불변식: "single-node 은 manifest 에 서브가 등록되면 서브제어 확장기능을 얻는다."
        # 종전 술어는 `nodes != []` 를 무조건 MANIFEST_INVALID 로 봐서 dormant 만 허용했다 —
        # 헌법이 허용한 상태를 게이트가 금지하는 **술어 노후화**이며, 그 결과 sub 가 등록된 single
        # manifest 는 HW·age 축에 도달조차 못 했다(2026-08-21 실증: 두 축 모두 blocked:invalid-input).
        # 교정: dormant(nodes 부재 또는 [])는 그대로 유효 · **활성**(비어있지 않음)이면 sub-control
        # 로스터 형태(main 정확히 1 + sub ≥ 1)를 요구한다 — 등록 자체는 허용하되 형태는 검사한다.
        # ⚠ 실행평면 경계는 여기서 완화되지 않는다: single 의 sub 는 A2A 제어 피어이지 텐서 워커가
        #   아니며, 그 계약은 `manifest_contract.manifest_tp`(single → 노드 배수 1 고정)가 소유한다.
        nodes = doc.get("nodes")
        if isinstance(nodes, list) and nodes:
            roles = [node.get("role") for node in nodes if isinstance(node, dict)]
            if roles.count("main") != 1 or roles.count("sub") < 1:
                invalid.append("nodes.roster")
    if kind == "manifest" and doc.get("topology") == "multi":
        nodes = doc.get("nodes")
        roles = [node.get("role") for node in nodes if isinstance(node, dict)] \
            if isinstance(nodes, list) else []
        if roles.count("main") != 1 or roles.count("sub") < 1:
            invalid.append("nodes.roster")
    return sorted(set(invalid))


def invalid_result(reason, invalid_fields, manifest_path=None):
    """Stable structured fail-closed result for malformed inputs."""
    return {
        "preflight_required": True,
        "reasons": [reason],
        "manifest_path": manifest_path or "",
        "hw_check": "blocked:invalid-input",
        "drift_fields": [],
        "age_check": "blocked:invalid-input",
        "age_days": None,
        "max_age_days": DEFAULT_MAX_AGE_DAYS,
        "invalid_fields": sorted(invalid_fields),
        "notes": [],
    }


# --------------------------------------------------------------------------------------
# 정규화 / 비교
# --------------------------------------------------------------------------------------
def _scalar(value):
    """None/빈값은 ""로 접어 비교한다(YAML null ↔ JSON null ↔ 빈 문자열 오탐 방지)."""
    if value is None:
        return ""
    if isinstance(value, bool):
        return "true" if value else "false"
    return str(value).strip()


def hw_facts(doc):
    """manifest 또는 scan 결과(dict) → 비교용 canonical HW 사실 dict."""
    doc = doc or {}
    out = {}
    for field in HW_FIELDS:
        value = doc.get(field)
        if field == "topology" and value in (None, ""):
            value = doc.get("topology_declared")
        out[field] = _scalar(value)
    ic = doc.get("interconnect") or {}
    canonical_ic = {}
    for field in INTERCONNECT_FIELDS:
        value = ic.get(field)
        if field == "hca_devices":
            canonical_ic[field] = sorted(_scalar(item) for item in (value or []))
        elif field == "gid_index" and isinstance(value, list):
            canonical_ic[field] = tuple(value)
        else:
            canonical_ic[field] = _scalar(value)
    out["interconnect"] = canonical_ic
    nodes = doc.get("nodes")
    out["_nodes_present"] = "nodes" in doc
    out["nodes"] = [
        {field: _scalar((node or {}).get(field)) for field in NODE_FIELDS}
        for node in (nodes if isinstance(nodes, list) else [])
    ]
    return out


def interconnect_compared(attested, observed):
    """HW 축이 interconnect 6필드를 비교할지의 **단일 술어**(제외 규칙의 유일한 소유자).

    **single 은 비교하지 않는다**(2026-08-21 · approved_by AhnSangHun). 근거는 실행평면 사실이다 —
    single 은 분산서빙을 하지 않으므로 interconnect 를 **쓰지 않는다**. 그래서 single manifest 의
    interconnect 블록은 "미사용" 선언(generic-ethernet/[]/null)이고, 재스캔은 머신에 실재하는
    RoCE v2 를 그대로 보고한다 — 이 발산은 **의도된 것**이지 드리프트가 아니다.
    `scan_node.evaluate_gate` 는 이미 같은 의미론을 갖는다: single 에서 RoCE 존재는 blocking 이
    아니라 warning 이고 gate note 가 "single-node: interconnect 스캔 skip(실패 아님)" 이다.
    두 판정기가 single 의미론에서 갈리면 한쪽이 오탐을 낸다(2026-08-21 실증: S0-b 가
    interconnect 4필드로 HW_DRIFT exit 4 — manifest 선언도 스캔 실측도 각각 정상인데도).

    **multi 는 완화하지 않는다** — 거기서 interconnect 는 텐서패브릭 그 자체다.
    제외는 **양쪽이 모두 single 일 때만** 성립한다: attested 와 observed 의 topology 가 갈리면
    그 자체가 `topology` 필드 드리프트이므로 interconnect 까지 눈감을 이유가 없다(fail-closed).
    """
    return not (attested.get("topology") == "single" and observed.get("topology") == "single")


def diff_fields(attested, observed):
    """불일치 필드명을 dotted 경로로 정렬 반환(빈 리스트 = 일치)."""
    out = []
    for field in HW_FIELDS:
        if attested.get(field) != observed.get(field):
            out.append(field)
    if interconnect_compared(attested, observed):
        a_ic, o_ic = attested.get("interconnect", {}), observed.get("interconnect", {})
        for field in INTERCONNECT_FIELDS:
            if a_ic.get(field) != o_ic.get(field):
                out.append("interconnect.%s" % field)
    if attested.get("_nodes_present") and observed.get("_nodes_present"):
        a_nodes, o_nodes = attested.get("nodes", []), observed.get("nodes", [])
        if len(a_nodes) != len(o_nodes):
            out.append("nodes.count")
        else:
            for idx, (a_node, o_node) in enumerate(zip(a_nodes, o_nodes)):
                for field in NODE_FIELDS:
                    if a_node.get(field) != o_node.get(field):
                        out.append("nodes[%d].%s" % (idx, field))
    return sorted(out)


# --------------------------------------------------------------------------------------
# 시각 파싱 (주입값만 — 벽시계 조회 없음)
# --------------------------------------------------------------------------------------
def parse_stamp(raw):
    """Exact `YYYYMMDDHH` or `YYYY-MM-DD` → date. 불가면 None(값 날조 ✗)."""
    if not isinstance(raw, str):
        return None
    text = raw
    match = re.fullmatch(r"(\d{4})-(\d{2})-(\d{2})", text)
    if match:
        year, month, day = map(int, match.groups())
    else:
        match = re.fullmatch(r"(\d{4})(\d{2})(\d{2})(\d{2})", text)
        if not match:
            return None
        year, month, day, hour = map(int, match.groups())
        if hour > 23:
            return None
    try:
        return datetime.date(year, month, day)
    except ValueError:
        return None


# --------------------------------------------------------------------------------------
# 판정
# --------------------------------------------------------------------------------------
def evaluate(manifest, observed=None, now=None, max_age_days=DEFAULT_MAX_AGE_DAYS,
             manifest_path=None, expected_topology=None):
    """결정론 판정. 같은 입력 → 같은 dict."""
    if manifest is not None:
        invalid = validate_document(manifest, "manifest", expected_topology)
        if invalid:
            result = invalid_result(REASON_MANIFEST_INVALID, invalid, manifest_path)
            result["max_age_days"] = max_age_days
            return result
    if observed is not None:
        invalid = validate_document(observed, "observed")
        if invalid:
            result = invalid_result(REASON_OBSERVED_INVALID, invalid, manifest_path)
            result["max_age_days"] = max_age_days
            return result

    if manifest is not None and now is not None:
        scanned = parse_stamp((manifest.get("terraforming") or {}).get("scanned_at"))
        if scanned is not None and scanned > now:
            result = invalid_result(
                REASON_MANIFEST_INVALID, ["terraforming.scanned_at.future"], manifest_path)
            result["max_age_days"] = max_age_days
            return result

    reasons = []
    notes = []
    drift = []
    age_days = None
    age_check = "skipped:no-manifest"
    hw_check = "skipped:no-manifest"

    if manifest is None:
        reasons.append(REASON_MANIFEST_ABSENT)
    else:
        terra = manifest.get("terraforming") or {}
        if terra.get("complete") is not True or terra.get("branch_verified") is not True:
            reasons.append(REASON_FLAG_ABSENT)

        if observed is None:
            hw_check = "skipped:no-observed"
        else:
            attested_facts, observed_facts = hw_facts(manifest), hw_facts(observed)
            drift = diff_fields(attested_facts, observed_facts)
            hw_check = "evaluated"
            if not interconnect_compared(attested_facts, observed_facts):
                # 제외를 **침묵시키지 않는다** — 무엇을 안 봤는지가 산출물에 남아야 하류가 안다.
                notes.append("single-node: interconnect 6필드는 HW 비교 대상 제외"
                             "(분산서빙 미사용 · scan_node.evaluate_gate 와 동일 의미론)")
            if drift:
                reasons.append(REASON_HW_DRIFT)

        scanned = parse_stamp(terra.get("scanned_at"))
        if now is None:
            age_check = "skipped:no-now"
        elif scanned is None:
            age_check = "skipped:unparsable-scanned-at"
            notes.append("terraforming.scanned_at 해석 불가 — age 축 미평가(값 날조 ✗)")
        else:
            age_days = (now - scanned).days
            age_check = "evaluated"
            if age_days < 0:
                notes.append("scanned_at 이 --now 보다 미래 — age 축 미적용")
            elif age_days > max_age_days:
                reasons.append(REASON_ATTESTATION_STALE)

    if not reasons:
        reasons.append(REASON_FRESH)

    return {
        "preflight_required": reasons != [REASON_FRESH],
        "reasons": reasons,
        "manifest_path": manifest_path or "",
        "hw_check": hw_check,
        "drift_fields": drift,
        "age_check": age_check,
        "age_days": age_days,
        "max_age_days": max_age_days,
        "notes": notes,
    }


# --------------------------------------------------------------------------------------
# I/O
# --------------------------------------------------------------------------------------
def manifest_path_for(repo, topology):
    return os.path.join(repo, "output", topology, "manifest.yaml")


def _load_yaml(path):
    try:
        import yaml
    except ModuleNotFoundError:
        # `-S` removes the active venv's site-packages. Restore only that declared
        # dependency boundary; do not add ambient user/global package directories.
        version = "python%d.%d" % (sys.version_info.major, sys.version_info.minor)
        venv_site = os.path.join(os.path.dirname(os.path.dirname(sys.executable)),
                                 "lib", version, "site-packages")
        if os.path.isdir(venv_site) and venv_site not in sys.path:
            sys.path.insert(0, venv_site)
        import yaml
    with open(path, "r", encoding="utf-8") as handle:
        return yaml.safe_load(handle)


def _load_json(path):
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def _self_test():
    cases = []

    def chk(name, ok, detail=""):
        cases.append((name, bool(ok), detail))

    base = {
        "topology": "single", "cpu_arch": "aarch64", "cuda_version": "132",
        "gpus_per_node": 1, "gpu_model": "NVIDIA GB10",
        # 실 `output/single/manifest.yaml` 의 interconnect 블록과 동형(= "미사용" 선언).
        # 아래 roce_ic 와 짝지으면 S0-b 가 실제로 본 4필드 발산이 그대로 재현된다.
        "interconnect": {
            "type": "generic-ethernet", "hca_devices": [], "gid_index": None,
            "socket_iface": None, "bandwidth_gbps": None, "platform_preset": None,
        },
        "nodes": [],
        "terraforming": {"complete": True, "branch_verified": True, "scanned_at": "2026072000"},
    }
    observed = {k: base[k] for k in ("topology", "cpu_arch", "cuda_version", "gpus_per_node",
                                     "gpu_model", "interconnect", "nodes")}
    ref = datetime.date(2026, 7, 25)

    fresh = evaluate(base, observed, ref)
    chk("fresh → preflight 불요",
        fresh["reasons"] == [REASON_FRESH] and not fresh["preflight_required"], str(fresh["reasons"]))

    chk("manifest 부재 → MANIFEST_ABSENT",
        evaluate(None, observed, ref)["reasons"] == [REASON_MANIFEST_ABSENT])

    no_flag = dict(base, terraforming={"complete": False, "branch_verified": True,
                                       "scanned_at": "2026072000"})
    chk("Flag 미발급 → FLAG_ABSENT", REASON_FLAG_ABSENT in evaluate(no_flag, observed, ref)["reasons"])

    drifted = evaluate(base, dict(observed, gpus_per_node=2), ref)
    chk("HW 드리프트 → HW_DRIFT + 필드명 보고",
        REASON_HW_DRIFT in drifted["reasons"] and drifted["drift_fields"] == ["gpus_per_node"],
        str(drifted["drift_fields"]))

    # ── interconnect 의미론: single 제외 / multi 유지 (2026-08-21 · approved_by AhnSangHun) ──
    # 종전 이 자리의 케이스는 **single fixture** 로 "인터커넥트 드리프트 → 필드명 보고"를 요구했다.
    # 그 술어 자체가 S0-b 오탐의 원천이었으므로(의도된 미사용 선언 ↔ 실측 RoCE) 아래로 대체한다.
    # 실측 재현: manifest = generic-ethernet/[]/null/null · 스캔 = RoCE v2 2 HCA/gid 3/enp1s0f1np1.
    roce_ic = {
        "type": "RoCE v2", "hca_devices": ["roceP2p1s0f1", "rocep1s0f1"], "gid_index": 3,
        "socket_iface": "enp1s0f1np1", "bandwidth_gbps": None, "platform_preset": None,
    }
    single_ic = evaluate(base, dict(observed, interconnect=roce_ic), ref)
    chk("single + interconnect 발산(미사용 선언 ↔ 실측 RoCE) → HW_DRIFT 미발생",
        single_ic["reasons"] == [REASON_FRESH] and single_ic["drift_fields"] == []
        and single_ic["hw_check"] == "evaluated"
        and any("interconnect" in note for note in single_ic["notes"]),
        "%s drift=%s notes=%s" % (single_ic["reasons"], single_ic["drift_fields"],
                                  single_ic["notes"]))

    single_ic_plus = evaluate(base, dict(observed, interconnect=roce_ic, gpus_per_node=2), ref)
    chk("single interconnect 제외가 다른 HW 축을 가리지 않음",
        REASON_HW_DRIFT in single_ic_plus["reasons"]
        and single_ic_plus["drift_fields"] == ["gpus_per_node"],
        str(single_ic_plus["drift_fields"]))

    single_ic_match = evaluate(base, observed, ref)
    chk("single + interconnect 일치 → 제외 note 는 그대로 표기(침묵 아님)",
        any("interconnect" in note for note in single_ic_match["notes"]),
        str(single_ic_match["notes"]))

    stale = evaluate(base, observed, datetime.date(2027, 7, 25))
    chk("attestation 만료 → ATTESTATION_STALE", REASON_ATTESTATION_STALE in stale["reasons"])

    skipped = evaluate(base, None, None)
    chk("축 생략은 음성정직 표기",
        skipped["hw_check"] == "skipped:no-observed" and skipped["age_check"] == "skipped:no-now")

    unknown_stamp = evaluate(dict(base, terraforming={
        "complete": True, "branch_verified": True, "scanned_at": "언젠가"}), observed, ref)
    chk("불명 scanned_at 은 fail-closed",
        unknown_stamp["reasons"] == [REASON_MANIFEST_INVALID]
        and unknown_stamp["preflight_required"]
        and "terraforming.scanned_at" in unknown_stamp["invalid_fields"])

    # ── single sub-control 확장기능 회귀 (헌법 §불변식 "서브가 등록되면 서브제어 확장기능을 얻는다") ──
    # 종전 술어(`nodes != []` → 무조건 invalid)는 아래 활성 케이스를 MANIFEST_INVALID 로 막아
    # HW·age 축 도달 자체를 봉쇄했다(2026-08-21 실증: 두 축 모두 blocked:invalid-input).
    # 실 scan 엔벨로프(build_local_scan_result)는 single 에서 `nodes` 키를 내지 않으므로 fixture 도 동형.
    sub_roster = [{"role": "main", "host": "192.0.2.10"}, {"role": "sub", "host": "192.0.2.11"}]
    observed_no_nodes = {k: v for k, v in observed.items() if k != "nodes"}

    active = evaluate(dict(base, nodes=sub_roster), observed_no_nodes, ref)
    chk("single + sub-control 로스터 → 유효(HW·age 축 도달)",
        active["reasons"] == [REASON_FRESH] and active["age_check"] == "evaluated"
        and active["hw_check"] == "evaluated",
        "%s hw=%s age=%s" % (active["reasons"], active["hw_check"], active["age_check"]))

    chk("single dormant(nodes:[]) → 여전히 유효",
        evaluate(base, observed, ref)["reasons"] == [REASON_FRESH])

    no_main = evaluate(dict(base, nodes=[{"role": "sub", "host": "192.0.2.11"}]),
                       observed_no_nodes, ref)
    chk("single 로스터에 main 부재 → nodes.roster fail-closed",
        no_main["reasons"] == [REASON_MANIFEST_INVALID]
        and "nodes.roster" in no_main.get("invalid_fields", []),
        str(no_main.get("invalid_fields")))

    two_main = evaluate(dict(base, nodes=[{"role": "main", "host": "192.0.2.10"},
                                          {"role": "main", "host": "192.0.2.11"}]),
                        observed_no_nodes, ref)
    chk("single 로스터 main 중복 → nodes.roster fail-closed",
        "nodes.roster" in two_main.get("invalid_fields", []),
        str(two_main.get("invalid_fields")))

    multi_ok = evaluate(dict(base, topology="multi", nodes=sub_roster), None, ref)
    chk("multi 정상 로스터 → 유효(규칙 불변)",
        multi_ok["reasons"] == [REASON_FRESH], str(multi_ok["reasons"]))

    multi_no_sub = evaluate(dict(base, topology="multi",
                                 nodes=[{"role": "main", "host": "192.0.2.10"}]), None, ref)
    chk("multi 로스터에 sub 부재 → fail-closed(완화되지 않음)",
        "nodes.roster" in multi_no_sub.get("invalid_fields", []),
        str(multi_no_sub.get("invalid_fields")))

    # ── multi 는 interconnect 비교를 **유지**한다(제외는 single 전용 — 완화 없음 증명) ──
    multi_attested = dict(base, topology="multi", nodes=sub_roster)
    multi_observed = dict(observed_no_nodes, topology="multi")
    multi_ic = evaluate(multi_attested, dict(multi_observed, interconnect=roce_ic), ref)
    chk("multi + interconnect 발산 → 여전히 HW_DRIFT(4필드 보고)",
        REASON_HW_DRIFT in multi_ic["reasons"]
        and set(multi_ic["drift_fields"]) == {
            "interconnect.gid_index", "interconnect.hca_devices",
            "interconnect.socket_iface", "interconnect.type"}
        and not any("interconnect" in note for note in multi_ic["notes"]),
        "%s notes=%s" % (multi_ic["drift_fields"], multi_ic["notes"]))

    chk("multi + interconnect 일치 → FRESH",
        evaluate(multi_attested, multi_observed, ref)["reasons"] == [REASON_FRESH])

    mixed = evaluate(base, dict(observed, topology="multi", interconnect=roce_ic), ref)
    chk("topology 갈림(single 선언 ↔ multi 관측) → 제외 미적용·fail-closed",
        REASON_HW_DRIFT in mixed["reasons"] and "topology" in mixed["drift_fields"]
        and "interconnect.type" in mixed["drift_fields"],
        str(mixed["drift_fields"]))

    twice = [json.dumps(evaluate(base, observed, ref), sort_keys=True, ensure_ascii=False)
             for _ in range(2)]
    chk("결정론(동일 입력 → 동일 출력)", twice[0] == twice[1])

    future = evaluate(base, observed, datetime.date(2026, 1, 1))
    chk("미래 scanned_at 은 fail-closed",
        future["reasons"] == [REASON_MANIFEST_INVALID]
        and "terraforming.scanned_at.future" in future["invalid_fields"])

    failed = [c for c in cases if not c[1]]
    for name, ok, detail in cases:
        print("%s %s%s" % ("PASS" if ok else "FAIL", name, (" — " + detail) if detail else ""))
    print("--- %d/%d PASS" % (len(cases) - len(failed), len(cases)))
    return EXIT_OK if not failed else 1


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="테라포밍 조건부 preflight 의 결정론 staleness 트리거")
    parser.add_argument("--topology", choices=("single", "multi"))
    parser.add_argument("--repo", default=".")
    parser.add_argument("--observed", help="scan_node.py stdout JSON(또는 manifest 형태) HW 사실 파일")
    parser.add_argument("--now", help="기준 시각 YYYY-MM-DD | YYYYMMDDHH (벽시계 미조회 — 주입 전용)")
    parser.add_argument("--max-age-days", type=int, default=DEFAULT_MAX_AGE_DAYS)
    parser.add_argument("--json", action="store_true", help="JSON 출력(기본과 동일)")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        return _self_test()

    if not args.topology:
        print("usage: --topology single|multi 필요(브랜치 추론 ✗ — 토폴로지는 manifest 통로 선택자)",
              file=sys.stderr)
        return EXIT_USAGE

    ref = None
    if args.now:
        ref = parse_stamp(args.now)
        if ref is None:
            print("usage: --now 는 YYYY-MM-DD 또는 YYYYMMDDHH", file=sys.stderr)
            return EXIT_USAGE

    path = manifest_path_for(args.repo, args.topology)
    manifest = None
    manifest_exists = os.path.isfile(path)
    if manifest_exists:
        try:
            manifest = _load_yaml(path)
        except Exception as exc:  # 손상 manifest = 재진입 사유(추정 복구 ✗)
            print("manifest 판독 실패: %s" % exc, file=sys.stderr)
            manifest = []
        if manifest is None:
            manifest = []

    observed = None
    if args.observed:
        if not os.path.isfile(args.observed):
            result = invalid_result(REASON_OBSERVED_INVALID, ["$"], path)
            result["notes"] = ["observed 경로가 regular file이 아님: %s" % args.observed]
            result["max_age_days"] = args.max_age_days
            print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
            return EXIT_USAGE
        try:
            observed = _load_json(args.observed)
        except Exception as exc:
            result = invalid_result(REASON_OBSERVED_INVALID, ["$"], path)
            result["notes"] = ["observed JSON 판독 실패: %s" % exc]
            result["max_age_days"] = args.max_age_days
            print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
            return EXIT_USAGE
        if observed is None:
            observed = []

    result = evaluate(manifest, observed, ref, args.max_age_days, manifest_path=path,
                      expected_topology=args.topology)
    print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
    if REASON_OBSERVED_INVALID in result["reasons"]:
        return EXIT_USAGE
    return EXIT_PREFLIGHT_REQUIRED if result["preflight_required"] else EXIT_OK


if __name__ == "__main__":
    sys.exit(main())
