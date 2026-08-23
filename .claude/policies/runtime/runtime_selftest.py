#!/usr/bin/env python3
"""Owner-local deterministic regression probe for constitution runtime modules.

This is production distribution evidence, not a development-only root test suite.  It exercises
security-critical negative paths under the active interpreter, including ``python -O`` and
``python -S`` when invoked by ``harness_verify.py`` / ``verify_distribution.py``.
"""
from __future__ import annotations

import ast
import contextlib
import io
import json
import os
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path

import agent_control
import completion_gate
import evidence_publisher
import policy_registry

RUNTIME_DIR = Path(__file__).resolve().parent
PREDICATES_DIR = RUNTIME_DIR.parent / "predicates"
CLAUDE_DIR = RUNTIME_DIR.parents[1]


class RuntimeSelftestFailure(RuntimeError):
    """A required production regression property did not hold."""


def _require(condition: object, message: str) -> None:
    if not condition:
        raise RuntimeSelftestFailure(message)


def _test_no_production_asserts() -> None:
    offenders: list[str] = []
    for path in CLAUDE_DIR.rglob("*.py"):
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
        offenders.extend(f"{path.relative_to(RUNTIME_DIR.parents[2])}:{node.lineno}"
                         for node in ast.walk(tree) if isinstance(node, ast.Assert))
    _require(not offenders, f"bare assert is optimization-unsafe: {offenders}")


def _test_completion_gate() -> None:
    work_enum = completion_gate.WORK_SCHEMA["definitions"]["executionApproval"]["properties"]["allowed_actions"]["items"]["enum"]
    side_enum = [value for value in completion_gate.SIDE_EFFECT_SCHEMA["properties"]["action"]["enum"]
                 if value is not None]
    expected_actions = set(completion_gate.ALLOWED_ACTIONS)
    _require(set(work_enum) == expected_actions and set(side_enum) == expected_actions,
             f"side-effect action enum drift: runtime={sorted(expected_actions)}, "
             f"work={sorted(work_enum)}, authorization={sorted(side_enum)}")
    try:
        completion_gate._resolve_ref("https://invalid.example/ref", {})
    except ValueError:
        pass
    else:
        raise RuntimeSelftestFailure("completion gate accepted a non-local $ref")

    try:
        completion_gate._emit_with_schema(
            {}, 0, {"title": "runtime-selftest", "type": "object", "required": ["must_exist"]}
        )
    except RuntimeError:
        pass
    else:
        raise RuntimeSelftestFailure("completion gate emitted schema-invalid own output")

    with tempfile.TemporaryDirectory(prefix="completion-runtime-selftest.") as td:
        root = Path(td)
        manifest_dir = root / "docs" / "_evidence"
        manifest_dir.mkdir(parents=True)
        status, parts = completion_gate._lexical_components(
            manifest_dir, root, "../../../outside.txt"
        )
        _require(status == "repo_escape" and parts is None,
                 f"lexical repo escape was not rejected: {status}, {parts}")

        target = root / "target.txt"
        target.write_text("evidence", encoding="utf-8")
        (root / "link.txt").symlink_to(target)
        root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
        try:
            status, fd = completion_gate._walk_nofollow(root_fd, ["link.txt"])
            if fd is not None:
                os.close(fd)
            _require(status == "symlink", f"symlink evidence was not rejected: {status}")

            unreadable, broken, invalid = completion_gate.check_required_local_links(
                root_fd, root, root, b"[required evidence](missing.md)"
            )
            _require(not unreadable and broken == ["missing.md"] and not invalid,
                     f"broken Markdown link was not detected: {unreadable}, {broken}, {invalid}")
        finally:
            os.close(root_fd)


# =============================================================================================
# 승격 게이트 -- explore 루브릭 권한의 **carrier 회귀** (plan_26082405)
# ---------------------------------------------------------------------------------------------
# 2026-08-24 실측 결함: explore 자동개방 경로가 rubric_authority 를 **인증서에서만** 읽었는데
# 인증서는 PASS 때만 발행된다 → REFUTE 런에서는 영원히 발화하지 못하는 **죽은 코드**였다.
# bench report 는 판정기 산출대로 "루브릭 권한 = explore" 를 적고 있었으므로 출처는 존재했고,
# 끊긴 것은 **통로**였다(침묵 누락). 아래 케이스들이 그 통로를 실제 subprocess 로 매번 다시 건다 --
# 단위 자체검사는 "분기가 도달 가능한가"를 못 잡기 때문에 끝단(verify)에서 확인한다.
# ★ 음성 대조: 수정 전 게이트에 C2 를 넣으면 `BENCHMARK_VERDICT_NOT_PASS` 로 막힌다(2026-08-24 확인).
#   그러므로 C2 의 PASS 는 "가드를 껐다"가 아니라 "끊긴 통로가 이어졌다"의 증거다.
# =============================================================================================

_PROMO_IDENTITY = {"model": "selftest-model", "gpu": "GB10", "vllm": "0.0.0.dev0",
                   "quant": "fp8", "topology": "single", "tp": 1}

# 인증서 carrier(PASS 런) -- 강한 일치 6키는 _PROMO_IDENTITY 와 글자 그대로 같아야 한다.
_PROMO_CERTIFICATE = """schema_version: 1
record_type: benchmark_certificate
verdict: PASS
model: selftest-model
gpu_model: GB10
vllm_version: 0.0.0.dev0
quantization: fp8
topology: single
tensor_parallel_size: 1
benchmark_mode: full
rubric_authority: {authority}
primary_source: E(external_reference)
primary_tps: 26.0
floor_tps: 22.1
tolerance: 0.15
ratio_M_over_primary: 0.719
"""

# manifest carrier(REFUTE 런) -- 인증서와 **동일한 계약**을 만족하는 최소 선언.
_PROMO_RUBRIC = {"rubric_authority": "explore", "floor_tps": 22.1,
                 "ratio_M_over_primary": 0.719, "primary_source": "E(external_reference)",
                 "rubric_source": "verdict_json"}


def _promotion_probe(root: Path, verdict: str, benchmark_extra: dict | None,
                     certificate: str | None = None, smoke_passed: bool = True) -> dict:
    """full_benchmark work-manifest 를 실제로 짓고 `completion_gate.py verify` 를 돌려 판정을 얻는다.

    in-process 호출이 아니라 subprocess 인 이유: verify 는 `_emit` 에서 프로세스를 끝내는 CLI 계약이며,
    운영에서 승격을 여는 것도 그 subprocess 다(authorize --mode promotion 이 자기 verify 를 spawn).
    같은 경로를 그대로 밟아야 "배선이 실제로 도는가"를 검사한 것이 된다.
    """
    evidence_dir = root / "docs" / "_evidence"
    evidence_dir.mkdir(parents=True, exist_ok=True)
    paths: dict[str, str] = {}
    for kind, subdir in (("plan", "plan"), ("devlog", "devlog"),
                         ("testlog", "testlog"), ("bench_report", "benchmark")):
        directory = root / "docs" / subdir
        directory.mkdir(parents=True, exist_ok=True)
        artifact = directory / f"{kind}_selftest.md"
        artifact.write_text(f"# {kind} selftest\n", encoding="utf-8")
        paths[kind] = os.path.relpath(artifact, evidence_dir)
    simlog = root / "docs" / "simlog" / "selftest_run"
    simlog.mkdir(parents=True, exist_ok=True)
    (simlog / "summary.json").write_text("{}\n", encoding="utf-8")
    paths["simlog"] = os.path.relpath(simlog, evidence_dir)
    if certificate is not None:
        artifact = root / "docs" / "benchmark" / "benchmark_selftest.yaml"
        artifact.write_text(certificate, encoding="utf-8")
        paths["certificate"] = os.path.relpath(artifact, evidence_dir)

    benchmark = {"mode": "full", "verdict": verdict}
    benchmark.update(benchmark_extra or {})
    manifest = {
        "schema_version": 1,
        "task_class": "full_benchmark",
        "identity": dict(_PROMO_IDENTITY),
        "runtime": {"health_ok": True, "functional_smoke_passed": smoke_passed,
                    "identity": dict(_PROMO_IDENTITY), "containers": []},
        "benchmark": benchmark,
        "conditions": {},
        "evidence": {key: {"path": value} for key, value in paths.items()},
        "pii_scan": {"passed": True, "scanned_paths": sorted(paths.values())},
    }
    manifest_path = evidence_dir / "selftest.work-manifest.json"
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    proc = subprocess.run(
        [sys.executable, str(RUNTIME_DIR / "completion_gate.py"), "verify",
         "--manifest", str(manifest_path), "--repo-root", str(root)],
        capture_output=True, text=True, timeout=120)
    try:
        out = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeSelftestFailure(
            f"completion_gate verify produced unparseable stdout: {exc}; "
            f"stderr={proc.stderr.strip()[:400]!r}") from exc
    out["_returncode"] = proc.returncode
    return out


def _test_promotion_rubric_carrier() -> None:
    def probe(**kwargs) -> dict:
        with tempfile.TemporaryDirectory(prefix="promotion-rubric-selftest.") as td:
            root = Path(td)
            (root / ".git").mkdir()
            return _promotion_probe(root, **kwargs)

    def _promoted(out: dict) -> bool:
        return (out.get("state") == "promotion-ready"
                and out.get("eligible_for_promotion") is True
                and out.get("_returncode") == 0)

    # C1 PASS + explore(인증서 carrier) -- 종전 거동 불변.
    out = probe(verdict="PASS", benchmark_extra=dict(_PROMO_RUBRIC),
                certificate=_PROMO_CERTIFICATE.format(authority="explore"))
    _require(_promoted(out) and (out.get("rubric") or {}).get("authority_source") == "certificate",
             f"PASS+explore lost its certificate carrier: {out.get('reason_codes')}")

    # C2 ★ REFUTE + explore(manifest carrier) -- perf_waiver 없이 열려야 한다(이 결함의 본체).
    out = probe(verdict="REFUTE", benchmark_extra=dict(_PROMO_RUBRIC))
    _require(_promoted(out)
             and "BENCHMARK_EXPLORE_AUTHORITY_PROMOTION" in (out.get("reason_codes") or [])
             and (out.get("rubric") or {}).get("authority_source") == "manifest_benchmark",
             f"explore-REFUTE auto-open is dead again (manifest carrier not consulted): {out.get('reason_codes')}")

    # C3 REFUTE + weak -- 통상 REFUTE 는 여전히 사람 서명 없이는 못 연다(조건 완화 ✗).
    out = probe(verdict="REFUTE", benchmark_extra=dict(_PROMO_RUBRIC, rubric_authority="weak"))
    _require(out.get("eligible_for_promotion") is False
             and "BENCHMARK_VERDICT_NOT_PASS" in (out.get("reason_codes") or []),
             f"weak-authority REFUTE was promoted without a human waiver: {out.get('reason_codes')}")

    # C3b rubric 미선언(legacy manifest) -- 차단은 유지하되 **잡음 reason 을 만들지 않는다**
    #     (부재와 결측의 구분: 채널 미사용은 결함이 아니다).
    out = probe(verdict="REFUTE", benchmark_extra=None)
    _require(out.get("eligible_for_promotion") is False
             and out.get("rubric") is None
             and not [c for c in (out.get("reason_codes") or []) if c.startswith("MANIFEST_RUBRIC")],
             f"legacy manifest without a rubric channel was mis-handled: {out.get('reason_codes')}")

    # C4 REFUTE + perf_waiver -- 사람 positive-key 경로는 그대로 살아 있다.
    out = probe(verdict="REFUTE", benchmark_extra={"perf_waiver": {
        "authorized_by": "selftest-operator", "authorized_at_utc": "2026-08-24T00:00:00Z",
        "instruction": "loop-until-done 중단", "warning_flag": "PERF-WARNING: floor 미달"}})
    _require(_promoted(out) and "BENCHMARK_VERDICT_WAIVED" in (out.get("reason_codes") or []),
             f"human perf_waiver path regressed: {out.get('reason_codes')}")

    # C5~C8 계약 위반 4종 -- explore 를 선언해도 **계약을 못 지키면 열리지 않는다**.
    #   floor<=0 은 특히 공허 PASS 배제 불변이며 어떤 authority 로도 뚫리지 않아야 한다.
    for field, value, expected_code in (
            ("floor_tps", 0, "MANIFEST_RUBRIC_FLOOR_INVALID"),
            ("ratio_M_over_primary", None, "MANIFEST_RUBRIC_RATIO_MISSING"),
            ("primary_source", "N/A", "MANIFEST_RUBRIC_SOURCE_MISSING"),
            ("rubric_source", None, "MANIFEST_RUBRIC_PROVENANCE_MISSING")):
        out = probe(verdict="REFUTE", benchmark_extra=dict(_PROMO_RUBRIC, **{field: value}))
        _require(out.get("eligible_for_promotion") is False
                 and expected_code in (out.get("reason_codes") or []),
                 f"manifest rubric contract did not fail closed on {field}={value!r}: "
                 f"{out.get('reason_codes')}")

    # C9 두 carrier 가 갈라지면 인증서(디스크 아티팩트)가 정본이고, 갈라진 사실은 표면화된다.
    out = probe(verdict="PASS", benchmark_extra=dict(_PROMO_RUBRIC),
                certificate=_PROMO_CERTIFICATE.format(authority="weak"))
    _require("RUBRIC_AUTHORITY_CARRIER_DISAGREEMENT" in (out.get("reason_codes") or [])
             and (out.get("rubric") or {}).get("authority") == "weak",
             f"carrier disagreement was silently resolved in the manifest's favor: {out.get('reason_codes')}")

    # C10 explore 라도 **서빙 성립**은 면제되지 않는다 -- U1 의 자격은 기능 ∧ 유효 측정이다.
    out = probe(verdict="REFUTE", benchmark_extra=dict(_PROMO_RUBRIC), smoke_passed=False)
    _require(out.get("eligible_for_promotion") is False
             and "RUNTIME_FUNCTIONAL_SMOKE_NOT_PASSED" in (out.get("reason_codes") or []),
             f"explore authority bypassed the functional-smoke requirement: {out.get('reason_codes')}")


def _test_policy_and_evidence_lifecycle() -> None:
    _require(policy_registry.scan_policy_citations("policy:SELFTEST_OK") == [(1, "SELFTEST_OK")],
             "policy citation broad scanner failed its canonical positive")
    _require(policy_registry.scan_policy_citations("xpolicy:NOT_A_CITATION") == [],
             "policy citation scanner accepted an embedded token")
    with contextlib.redirect_stdout(io.StringIO()):
        _require(policy_registry._self_test() == 0, "policy registry self-test did not return 0")
        evidence_publisher._self_test()


def _request(transport: str = "local") -> dict:
    target = {"role": "main", "transport": transport, "work_dir": "/tmp/runtime probe"}
    if transport == "ssh":
        target.update({"role": "sub", "host": "192.0.2.10", "ssh_user": "probe"})
    return {
        "schema_version": 1,
        "provider": "claude_code",
        "intent": "probe",
        "task": "read-only runtime probe",
        "model": "sonnet",
        "max_turns": 1,
        "capabilities": ["read"],
        "target": target,
    }


def _test_agent_provider_boundary() -> None:
    request_schema = agent_control._load_schema(agent_control.REQUEST_SCHEMA_PATH)
    result_schema = agent_control._load_schema(agent_control.RESULT_SCHEMA_PATH)
    _require(agent_control._schema_violations({}, request_schema),
             "empty agent request unexpectedly passed schema validation")
    invalid = agent_control._invalid_request_result({})
    _require(not agent_control._schema_violations(invalid, result_schema),
             "invalid-request fail-closed envelope violates result schema")

    provider = agent_control._load_provider("claude_code")
    agent_source = Path(agent_control.__file__).read_text(encoding="utf-8")
    provider_file = getattr(provider, "__file__", None)
    _require(isinstance(provider_file, str), "loaded provider has no source path")
    provider_source = Path(str(provider_file)).read_text(encoding="utf-8")
    for token in ("claude -p", "--model sonnet", "--output-format json"):
        _require(token not in agent_source, f"provider-specific token leaked into orchestrator: {token}")
        _require(token in provider_source, f"provider adapter lost required CLI token: {token}")

    local_argv = provider.build_argv(_request("local"))
    _require(local_argv[0] == "claude" and "--model" in local_argv,
             f"local provider argv malformed: {local_argv}")
    ssh_argv = provider.build_argv(_request("ssh"))
    _require(ssh_argv[:2] == ["ssh", "--"] and ssh_argv[2] == "probe@192.0.2.10",
             f"SSH provider argv malformed: {ssh_argv}")
    remote_shell = shlex.split(ssh_argv[3])
    _require(remote_shell[:2] == ["bash", "-lc"] and
             remote_shell[2].startswith("cd '/tmp/runtime probe' && claude "),
             f"SSH work_dir is not safely shell-quoted: {ssh_argv[3]}")

    blocked = _request("local")
    blocked["model"] = "opus"
    result = provider.invoke(blocked)
    _require(result["status"] == "model_safety_blocked" and result["exit_code"] == 3,
             f"non-Sonnet request was not blocked before execution: {result}")


def main() -> int:
    _test_no_production_asserts()
    _test_completion_gate()
    _test_promotion_rubric_carrier()
    _test_policy_and_evidence_lifecycle()
    _test_agent_provider_boundary()
    print("[runtime_selftest] PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
