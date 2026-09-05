#!/usr/bin/env python3
"""Owner-local deterministic regression probe for constitution runtime modules.

This is production distribution evidence, not a development-only root test suite.  It exercises
security-critical negative paths under the active interpreter, including ``python -O`` and
``python -S`` when invoked by ``harness_verify.py`` / ``verify_distribution.py``.
"""
from __future__ import annotations

import argparse
import ast
import contextlib
import hashlib
import importlib.util
import io
import json
import os
import re
import shlex
import shutil
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
measured_utc: "2026-01-01T00:00:00Z"
"""

# manifest carrier(REFUTE 런) -- 인증서와 **동일한 계약**을 만족하는 최소 선언.
_PROMO_RUBRIC = {"rubric_authority": "explore", "floor_tps": 22.1,
                 "ratio_M_over_primary": 0.719, "primary_source": "E(external_reference)",
                 "rubric_source": "verdict_json"}


# bench_report 는 **PASS/REFUTE 무관하게 항상 발행되는** 문서라, 인증서가 구조적으로 없는 경로
# (perf_waiver · explore)에서 lite 정량지표의 유일한 근거가 된다(hint_tag `_require_serving_evidence`
# B 브랜치). 그래서 공용 픽스처의 리포트도 **실제로 lite 절과 실측 열을 갖는다** -- 빈 리포트를 쓰면
# 그 브랜치가 늘 실패해 "통과했다"를 증명할 수 없다.
_LITE_BENCH_REPORT = """# bench_report selftest

## lite 지표 (서빙 성공 시 자동 수집 · 관측용)

| 항목 | 값 |
|---|---|
| gen tokens/sec (warm) | 26.0 |
| TTFT p50 (ms) | 120 |
"""


def _write_promotion_manifest(root: Path, verdict: str, benchmark_extra: dict | None,
                              certificate: str | None = None, smoke_passed: bool = True,
                              promotion_target: dict | None = None,
                              bench_report_text: str = _LITE_BENCH_REPORT) -> Path:
    """full_benchmark work-manifest + 그 증거 아티팩트를 `root` 안에 실제로 짓는다.

    승격 게이트 회귀(`_test_promotion_rubric_carrier`)와 hint 발행 회귀
    (`_test_hint_binding_source`)가 **같은 픽스처**를 쓴다 -- 두 평면이 같은 manifest 모양을
    각자 손으로 지으면 오늘 고치는 바로 그 "같은 가정이 여러 곳" 결함이 픽스처에서 다시 자란다.
    """
    evidence_dir = root / "docs" / "_evidence"
    evidence_dir.mkdir(parents=True, exist_ok=True)
    paths: dict[str, str] = {}
    for kind, subdir in (("plan", "plan"), ("devlog", "devlog"),
                         ("testlog", "testlog"), ("bench_report", "benchmark")):
        directory = root / "docs" / subdir
        directory.mkdir(parents=True, exist_ok=True)
        artifact = directory / f"{kind}_selftest.md"
        artifact.write_text(bench_report_text if kind == "bench_report" else f"# {kind} selftest\n",
                            encoding="utf-8")
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
    if promotion_target is not None:
        manifest["promotion_target"] = promotion_target
    manifest_path = evidence_dir / "selftest.work-manifest.json"
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    return manifest_path


def _promotion_probe(root: Path, verdict: str, benchmark_extra: dict | None,
                     certificate: str | None = None, smoke_passed: bool = True) -> dict:
    """full_benchmark work-manifest 를 실제로 짓고 `completion_gate.py verify` 를 돌려 판정을 얻는다.

    in-process 호출이 아니라 subprocess 인 이유: verify 는 `_emit` 에서 프로세스를 끝내는 CLI 계약이며,
    운영에서 승격을 여는 것도 그 subprocess 다(authorize --mode promotion 이 자기 verify 를 spawn).
    같은 경로를 그대로 밟아야 "배선이 실제로 도는가"를 검사한 것이 된다.
    """
    manifest_path = _write_promotion_manifest(root, verdict, benchmark_extra, certificate, smoke_passed)
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


def _test_certificate_run_resolution() -> None:
    """plan_26090410 P2 — 게이트가 인증서의 측정 키로 0/1/2+ 를 판정하는가(음성대조 포함).
    tripwire ④ 뒤의 두 번째 방어선: 같은 측정이 두 파일로 추적되면 승격이 열리지 않아야 한다."""
    cert = _PROMO_CERTIFICATE.format(authority="weak")

    def run(plant: dict | None = None, certificate: str = cert, extra: dict | None = None) -> dict:
        bench = dict(_PROMO_RUBRIC, rubric_authority="weak", **(extra or {}))
        with tempfile.TemporaryDirectory(prefix="cert-run-resolution.") as td:
            root = Path(td)
            (root / ".git").mkdir()
            _write_promotion_manifest(root, "PASS", bench, certificate)
            for name, text in (plant or {}).items():
                (root / "docs" / "benchmark" / name).write_text(text, encoding="utf-8")
            return _promotion_probe(root, "PASS", bench, certificate)

    out = run()
    _require(out.get("eligible_for_promotion") is True
             and (out.get("certificate") or {}).get("run_key", [None])[-1] == "2026-01-01T00:00:00Z"
             and (out.get("certificate") or {}).get("duplicates") == [],
             f"단독 인증서는 키가 식별되고 중복 0 이어야 한다: {out.get('reason_codes')} {out.get('certificate')}")

    out = run(plant={"benchmark_selftest_copy.yaml": cert})
    _require(out.get("eligible_for_promotion") is False
             and "CERTIFICATE_RUN_AMBIGUOUS" in (out.get("reason_codes") or [])
             and (out.get("certificate") or {}).get("duplicates") == ["docs/benchmark/benchmark_selftest_copy.yaml"]
             and "byte-identical" in json.dumps(out.get("messages") or {}, ensure_ascii=False),
             f"★같은 측정의 사본이 있으면 승격이 막혀야 한다: {out.get('reason_codes')}")

    out = run(plant={"benchmark_selftest_other.yaml": cert.replace("2026-01-01T00:00:00Z", "2026-01-01T00:00:01Z")})
    _require(out.get("eligible_for_promotion") is True
             and "CERTIFICATE_RUN_AMBIGUOUS" not in (out.get("reason_codes") or []),
             f"★음성대조 measured_utc 가 다른 형제는 다른 측정이다: {out.get('reason_codes')}")

    out = run(plant={"benchmark_selftest_diff.yaml": cert.replace("ratio_M_over_primary: 0.719", "ratio_M_over_primary: 0.9")})
    _require("CERTIFICATE_RUN_AMBIGUOUS" in (out.get("reason_codes") or [])
             and "DIFFERENT bytes" in json.dumps(out.get("messages") or {}, ensure_ascii=False),
             f"같은 측정 다른 내용은 더 나쁜 결함으로 가려야 한다: {out.get('reason_codes')}")

    out = run(certificate=cert.replace('measured_utc: "2026-01-01T00:00:00Z"\n', ""))
    _require(out.get("eligible_for_promotion") is False
             and "CERTIFICATE_RUN_KEY_UNRESOLVABLE" in (out.get("reason_codes") or []),
             f"★measured_utc 없는 인증서는 식별 불가로 차단: {out.get('reason_codes')}")

    # P3 — manifest 가 선언한 측정과 파일이 갈라지면 DRIFT, 같으면 통과(선언은 인증서에서 파생된 값)
    out = run(extra={"measured_utc": "2026-01-01T00:00:00Z"})
    _require(out.get("eligible_for_promotion") is True
             and "CERTIFICATE_RUN_DRIFT" not in (out.get("reason_codes") or []),
             f"선언과 파일이 같으면 통과: {out.get('reason_codes')}")
    out = run(extra={"measured_utc": "2026-01-01T00:00:59Z"})
    _require(out.get("eligible_for_promotion") is False
             and "CERTIFICATE_RUN_DRIFT" in (out.get("reason_codes") or []),
             f"★manifest 가 다른 측정을 선언하면 DRIFT 로 차단: {out.get('reason_codes')}")

    out = run(plant={"notes.yaml": "x: 1\n", "benchmark_garbled.yaml": "a:\n  nested: 1\n"})
    _require(out.get("eligible_for_promotion") is True,
             f"비-인증서·파싱불가 형제는 매치 대상이 아니다(승격 유지): {out.get('reason_codes')}")


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


# =============================================================================================
# hint 발행 게이트 -- 인증서-부재 바인딩의 **세 번째 case**(explore) 회귀 (plan_26082405 §개정 R2)
# ---------------------------------------------------------------------------------------------
# 위 승격 게이트와 **같은 계열의 결함**이 hint_tag.py 에도 박혀 있었다: "인증서가 없다 → perf_waiver
# 여야 한다"는 이분법. explore 런은 인증서도(PASS 아님) waiver 도(사람 서명 없음) 없으므로
# `_require_serving_evidence` 의 `elif not cert_rel` 로 떨어져 차단되고, 설령 통과해도
# `_binding_artifact_path` 가 None 을 돌려 footer 바인딩이 HINT_CERTIFICATE_EVIDENCE_MISSING 으로
# 죽었다. 즉 승격 게이트만 고치면 **다음 문에서 다시 막히는** 상태였다.
# 처방은 세 번째 case 추가 + 판정의 단일 소유(`_no_cert_binding_source`)다.
#
# 아래는 실제 CLI subprocess 로 create→seal→index→verify 전 구간을 격리 레포에서 돌린다 --
# 단위 자체검사는 "분기가 도달 가능한가"를 못 잡는다(2026-08-22 실증: 새로 넣은 분기가 선행 게이트와
# 상호배타여서 도달 불가였고, 끝단 음성대조가 커밋 직전에 잡았다).
# ★ 음성 대조(2026-08-24 확인): 수정 전 hint_tag 로 H2 를 돌리면 create 가
#   HINT_SERVING_EVIDENCE_INSUFFICIENT("evidence.certificate 부재")로 죽는다. H5 는 수정 후에도
#   그대로 차단된다 -- 가드를 끈 것이 아니라 통로를 이었다는 증거다.
# =============================================================================================

# ★ 2026-09-04(CP7 · plan_26090415 §7.5 M1): 태그가 5세그먼트가 됐다 —
#   `hint/<vllm>/<model>/<arch>/<recipe>`. 마지막 칸은 **인증서에서 파생**하며
#   `hint_tag.derive_recipe_segment` 가 아래 `_HINT_CERTIFICATE` 로부터 같은 값을 낸다
#   (이름과 측정이 어긋나면 seal 이 HINT_RECIPE_SEGMENT_MISMATCH 로 거부한다).
_HINT_TAG = "hint/0.0.0.dev0/selftest-model/gb10/qfp8-len32768-kvfp8"
_HINT_TOPOLOGY = "single 1노드 TP1"
_HINT_HF_REPO = "selftest-org/selftest-model"
_HINT_SCRIPT_REL = ".claude/skills/hint-publisher/scripts/hint_tag.py"
_HINT_TEMPLATE_REL = ".claude/skills/hint-publisher/templates/hint_recipe.template.md"
_HINT_CATALOG_REL = ".claude/skills/hint-publisher/scripts/hint_catalog.py"

# 인증서 carrier 케이스용 -- 실제 인증서는 lite 열을 갖는다(full ⊇ lite 불변식).
# 레시피 축 3종을 **전부** 담는다 — 하나만 담으면 픽스처가 실물보다 좁아져 파생기의
# 다축 결합을 시험하지 못한다(같은 계열 회귀를 하루에 네 번 겪었다).
_HINT_CERTIFICATE = (_PROMO_CERTIFICATE.format(authority="weak")
                     + "lite_included: true\nlite_gen_tps_warm: 26.0\n"
                     + "max_model_len: 32768\nkv_cache_dtype: fp8\n")

# 린터 L1~L5 를 실제로 통과하는 최소 본문(합성 픽스처 -- 실제 서빙 사실이 아니다).
_HINT_RECIPE_BODY = """selftest 합성 레시피 픽스처 — 실제 서빙 실적이 아니라 런타임 자체검사용이다.

## 1. 벽 지도
이 태그는 자체검사 픽스처이므로 실제로 부딪힌 아치-월이 없다. 수신자는 이 칸을 근거로 삼지 말고
자기 하드웨어에서 벽을 다시 확인해야 한다. 전이등급 arch-invariant — 벽 지도 자체는 하드웨어를
가리지 않는 서술이지만, 여기 적힌 값은 어느 것도 실측이 아니다.

## 2. 결정론 해소값
해소값은 결정론 스크립트가 채우는 자리이며 픽스처에서는 합성값이다. torch/NGC/CUDA 핀은 목표
버전 선언에서 파생되고, 빌드트랙 판정은 소유 스킬이 내린다. 이 절의 값은 재현 대상이 아니라
정보구조 린터의 밀도 요건을 만족시키기 위한 자리표시자다.

## 3. 모델 서빙 노브
서빙 노브는 모델·하드웨어 조합마다 다시 확정한다. 이전 모델의 노브를 그대로 옮기는 carry-forward
는 금지이며, 픽스처의 노브는 어떤 실측에도 대응하지 않는다. 수신자는 자기 환경에서 재측정한 뒤
자기 값을 쓴다.

## 4. 빌드평면 노브
빌드 평면의 노브(빌드 패치·포크 핀·변종 이미지 태그)는 serve 평면과 성립 시점이 다르다. 픽스처는
어떤 변종도 요구하지 않는 stock 경로를 가정하며, 실제 발행물이라면 여기에 변종 좌표의 등재부를
가리키는 포인터가 들어간다.

## 5. 성능 baseline
성능 기준선은 적대적 벤치마크가 판정하며 픽스처의 수치는 합성이다. 이 태그는 explore 루브릭 권한
경로의 배선을 검사하기 위한 것이므로, 여기 적힌 어떤 처리량도 성능 주장을 구성하지 않는다.
수신자는 자기 환경에서 재측정한다.

## 6. 재검증
재검증 절차는 기능 스모크와 성능 계측을 분리해 수행한다. 픽스처에서는 실제 서빙을 기동하지 않고
게이트 배선만 검사하므로, 이 절은 재현 지시가 아니라 린터가 요구하는 구조를 채우는 서술이다.
실제 발행물에서는 여기에 재측정 명령이 그대로 들어간다.

## 7. 메타
발행 맥락·근거 문서·후속 계획을 적는 칸이다. 이 픽스처는 runtime_selftest 가 임시 레포에서만
만들고 즉시 버리므로 어떤 원격에도 도달하지 않으며, 색인·배포 권위 선언도 임시 디렉터리 안에서만
유효하다.
"""


def _import_hint_tag():
    """hint_tag.py 를 in-process 로 적재한다(claim_predicates.py 와 같은 관용구).

    E2E 만으로는 `_no_cert_binding_source` 의 **계약 위반 4종** 같은 조합을 다 돌리기에 비싸고,
    단위만으로는 분기 도달성을 못 잡는다. 그래서 아래 H0(단위 조합 표) + H1~H5(E2E) 를 함께 둔다.
    """
    path = CLAUDE_DIR.parent / _HINT_SCRIPT_REL
    spec = importlib.util.spec_from_file_location("_runtime_selftest_hint_tag", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _hint_git(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-c", "commit.gpgsign=false", "-c", "user.name=selftest",
         "-c", "user.email=selftest@example.invalid", "-C", str(root), *args],
        capture_output=True, text=True, timeout=120)


def _hint_repo(root: Path) -> str:
    """hint_tag.py 가 실제로 도는 최소 격리 레포를 짓고 앵커 커밋 SHA 를 돌려준다.

    hint_tag 는 ROOT 를 **cwd 기준 `git rev-parse --show-toplevel`** 으로 잡고
    completion_gate/스키마/템플릿을 ROOT 상대로 찾는다. 그래서 진짜 git 레포 + 그 배치가 필요하다.
    """
    for rel in (".claude/policies/runtime", ".claude/schemas",
                ".claude/skills/hint-publisher/scripts",
                ".claude/skills/hint-publisher/templates", "hints"):
        (root / rel).mkdir(parents=True, exist_ok=True)
    shutil.copy2(RUNTIME_DIR / "completion_gate.py", root / ".claude/policies/runtime/completion_gate.py")
    for name in ("work-manifest.schema.json", "completion-manifest.schema.json",
                 "side-effect-authorization.schema.json"):
        shutil.copy2(CLAUDE_DIR / "schemas" / name, root / ".claude/schemas" / name)
    shutil.copy2(CLAUDE_DIR.parent / _HINT_SCRIPT_REL, root / _HINT_SCRIPT_REL)
    shutil.copy2(CLAUDE_DIR.parent / _HINT_TEMPLATE_REL, root / _HINT_TEMPLATE_REL)
    shutil.copy2(CLAUDE_DIR.parent / _HINT_CATALOG_REL, root / _HINT_CATALOG_REL)
    # 카탈로그 관리 구역은 **마커 쌍**이다(2026-09-01 D1.1). 여는 마커가 없던 옛 형식은
    # 구역의 시작이 모호해 마커 유실 시 전 행이 조용히 사라질 수 있었다(감사 ⑬).
    (root / "HINTS.md").write_text(
        "# hints\n\n<!-- hint-index:rows -->\n<!-- hint-index:rows -->\n", encoding="utf-8")
    # 실 pii_terms.txt 는 운영자 리터럴이라 격리 레포로 복사하지 않는다(픽스처 전용 토큰만).
    (root / ".claude/pii_terms.txt").write_text(
        "# selftest fixture terms\nselftest-forbidden-token\n", encoding="utf-8")
    (root / "hints/index.json").write_text('{"hints": []}\n', encoding="utf-8")
    (root / "hints/.central_authority").write_text(
        "이 체크아웃이 자기 원격의 hint 색인·배포 권위다(selftest 임시 레포).\n", encoding="utf-8")
    _hint_git(root, "init", "-q", "-b", "selftest")
    _hint_git(root, "commit", "--allow-empty", "-q", "-m", "selftest anchor")
    return _hint_git(root, "rev-parse", "HEAD").stdout.strip()


def _hint_cli(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(root / _HINT_SCRIPT_REL), *args],
                          cwd=str(root), capture_output=True, text=True, timeout=180)


def _hint_publish_probe(benchmark_extra: dict | None, certificate: str | None = None,
                        bench_report_text: str = _LITE_BENCH_REPORT,
                        recipe_body: str = _HINT_RECIPE_BODY) -> dict:
    """격리 레포에서 create→seal→**catalog derive**→verify 를 실제로 돌린다.

    2026-09-01: `index`(손저작 색인)가 D1.1 로 폐쇄되어 카탈로그 단계를 원격 파생으로 옮겼다.
    프로브가 임시 bare 원격을 만들고 태그를 push 한 뒤 파생한다 — 발행 사실을 실제로 만든다.
    """
    with tempfile.TemporaryDirectory(prefix="hint-binding-selftest.") as td:
        root = Path(td).resolve()
        anchor = _hint_repo(root)
        manifest = _write_promotion_manifest(
            root, "PASS" if certificate is not None else "REFUTE", benchmark_extra,
            certificate=certificate, bench_report_text=bench_report_text,
            promotion_target={"kind": "hint", "tag": _HINT_TAG,
                              "topology": _HINT_TOPOLOGY, "anchor": anchor})
        # `--allow-new-slug` 는 2026-09-01 제거됐다 — 슬러그 정본표(CANONICAL_SLUGS)가 사라지고
        # 철자 충돌 대조가 **발행된 태그에서 파생**되도록 바뀌면서 '표에 없음'이라는 상태 자체가
        # 없어졌기 때문이다(plan_26090107 §6). 신규 슬러그는 이제 플래그 없이 통과한다.
        common = ("--tag", _HINT_TAG, "--topology", _HINT_TOPOLOGY, "--commit", anchor,
                  "--hf-repo", _HINT_HF_REPO, "--manifest", str(manifest))
        out = {"anchor": anchor, "create": _hint_cli(root, "create", *common)}
        if out["create"].returncode != 0:
            return out
        recipe = root / "hints" / ".drafts" / "selftest_recipe.md"
        recipe.write_text(recipe_body, encoding="utf-8")
        out["seal"] = _hint_cli(root, "seal", *common, "--recipe", str(recipe),
                                "--tagger-name", "selftest",
                                "--tagger-email", "selftest@example.invalid")
        if out["seal"].returncode != 0:
            return out
        out["tag_object"] = _hint_git(root, "cat-file", "tag", _HINT_TAG).stdout
        # 카탈로그는 **원격 발행 태그에서 파생**한다(D1.1) — 손저작 `index` 경로는 폐쇄됐다.
        # 그래서 프로브도 진짜로 원격을 만들고 거기에 push 한 뒤 파생한다. 원격을 만들지 않으면
        # "발행됐다"의 증거가 없어 카탈로그가 비는 것이 **정상 동작**이므로, 그 경로를 시험하려면
        # 발행 사실 자체를 만들어야 한다.
        bare = root.parent / (root.name + ".remote.git")
        _hint_git(root, "init", "--bare", "-q", str(bare))
        _hint_git(root, "push", "-q", str(bare),
                  f"refs/tags/{_HINT_TAG}:refs/tags/{_HINT_TAG}")
        out["catalog"] = subprocess.run(
            [sys.executable, "-B", str(root / _HINT_CATALOG_REL), "--repo", str(root),
             "derive", "--remote", str(bare), "--generated-kst", "2026-01-01T00:00:00"],
            cwd=str(root), capture_output=True, text=True)
        out["verify"] = _hint_cli(root, "verify", "--manifest", str(manifest))
        return out


def _test_hint_binding_source() -> None:
    hint_tag = _import_hint_tag()
    bench_report_rel = "../benchmark/bench_report_selftest.md"
    cert_rel = "../benchmark/benchmark_selftest.yaml"

    def manifest_shape(benchmark: dict, with_cert: bool = False) -> dict:
        evidence = {"bench_report": {"path": bench_report_rel}}
        if with_cert:
            evidence["certificate"] = {"path": cert_rel}
        return {"benchmark": benchmark, "evidence": evidence}

    # ---- H0 단위: 판정자 하나가 네 모양을 모두 가른다(우선순위 certificate > waiver∨explore > 차단)
    explore_manifest = manifest_shape(dict(_PROMO_RUBRIC))
    waiver_manifest = manifest_shape({"perf_waiver": {"authorized_by": "selftest-operator"}})
    bare_manifest = manifest_shape({"verdict": "REFUTE"})
    cert_manifest = manifest_shape(dict(_PROMO_RUBRIC), with_cert=True)
    _require(hint_tag._no_cert_binding_source(explore_manifest) == "explore",
             "explore rubric authority did not open the no-certificate binding source")
    _require(hint_tag._no_cert_binding_source(waiver_manifest) == "perf_waiver",
             "perf_waiver stopped opening the no-certificate binding source")
    _require(hint_tag._no_cert_binding_source(bare_manifest) is None,
             "a manifest with neither waiver nor explore opened a binding source")
    for field, value in (("floor_tps", 0), ("rubric_source", None), ("primary_source", "N/A")):
        broken = manifest_shape(dict(_PROMO_RUBRIC, **{field: value}))
        _require(hint_tag._no_cert_binding_source(broken) is None,
                 f"explore contract violation {field}={value!r} still opened the binding source")
    _require(hint_tag._binding_artifact_path(cert_manifest) == cert_rel,
             "certificate lost priority over the manifest-declared explore fallback")
    _require(hint_tag._binding_artifact_path(explore_manifest) == bench_report_rel,
             "explore path did not bind the bench_report artifact")
    _require(hint_tag._binding_artifact_path(waiver_manifest) == bench_report_rel,
             "perf_waiver path stopped binding the bench_report artifact")
    _require(hint_tag._binding_artifact_path(bare_manifest) is None,
             "a manifest with no certificate/waiver/explore still produced a binding artifact")

    # ---- H1 E2E ★ explore(REFUTE·waiver 없음) 가 create→seal→index→verify 전 구간을 통과한다.
    out = _hint_publish_probe(dict(_PROMO_RUBRIC))
    for step in ("create", "seal", "catalog", "verify"):
        proc = out.get(step)
        _require(proc is not None and proc.returncode == 0,
                 f"explore-authority hint publication died at `{step}`: "
                 f"rc={getattr(proc, 'returncode', None)} "
                 f"stdout={getattr(proc, 'stdout', '')[-700:]!r} "
                 f"stderr={getattr(proc, 'stderr', '')[-700:]!r}")
    _require("certificate_ref: ../benchmark/bench_report_selftest.md" in out["tag_object"],
             f"explore footer did not bind the bench_report: {out['tag_object'][-500:]!r}")

    # ---- H2 E2E 종전 경로 불변: 인증서(PASS) 는 여전히 인증서에 묶인다.
    out = _hint_publish_probe(None, certificate=_HINT_CERTIFICATE)
    for step in ("create", "seal", "catalog", "verify"):
        proc = out.get(step)
        _require(proc is not None and proc.returncode == 0,
                 f"certificate hint publication regressed at `{step}`: "
                 f"rc={getattr(proc, 'returncode', None)} "
                 f"stderr={getattr(proc, 'stderr', '')[-700:]!r}")
    _require("certificate_ref: ../benchmark/benchmark_selftest.yaml" in out["tag_object"],
             "certificate path stopped being the binding artifact")

    # ---- H3 E2E 종전 경로 불변: perf_waiver(REFUTE 사람승인) 도 그대로 bench_report 에 묶인다.
    #      waiver 는 경고 플래그가 본문에 실려야 하므로 마커를 담은 본문으로 seal 한다.
    waiver_body = (_HINT_RECIPE_BODY + "\n## 8. comment\n"
                   "PERF-WARNING: selftest fixture — 성능 기준 미달을 사람이 승인한 합성 픽스처다.\n")
    out = _hint_publish_probe({"perf_waiver": {
        "authorized_by": "selftest-operator", "authorized_at_utc": "2026-08-24T00:00:00Z",
        "instruction": "loop-until-done 중단",
        "warning_flag": "PERF-WARNING: selftest fixture"}}, recipe_body=waiver_body)
    for step in ("create", "seal", "catalog", "verify"):
        proc = out.get(step)
        _require(proc is not None and proc.returncode == 0,
                 f"perf_waiver hint publication regressed at `{step}`: "
                 f"rc={getattr(proc, 'returncode', None)} "
                 f"stderr={getattr(proc, 'stderr', '')[-700:]!r}")
    _require("certificate_ref: ../benchmark/bench_report_selftest.md" in out["tag_object"],
             "perf_waiver footer stopped binding the bench_report")

    # ---- H4 ★ explore 라도 **근거 검사는 면제되지 않는다**: lite 열 없는 리포트는 차단된다.
    out = _hint_publish_probe(dict(_PROMO_RUBRIC), bench_report_text="# bench_report selftest\n")
    _require(out["create"].returncode != 0
             and "HINT_SERVING_EVIDENCE_INSUFFICIENT" in out["create"].stdout,
             f"explore bypassed the lite-evidence requirement: rc={out['create'].returncode} "
             f"stdout={out['create'].stdout[-700:]!r}")

    # ---- H5 ★ 셋 다 부재(REFUTE · waiver ✗ · explore ✗) 는 여전히 차단된다.
    out = _hint_publish_probe(None)
    _require(out["create"].returncode != 0
             and "PROMOTION_GATE_NOT_ELIGIBLE" in out["create"].stdout,
             f"a REFUTE run with no waiver and no explore authority was allowed to publish: "
             f"rc={out['create'].returncode} stdout={out['create'].stdout[-700:]!r}")


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
        # 2026-09-03(plan_26090317 P1): 이 픽스처는 스키마 required 인 `timeout_seconds` 를 빠뜨리고
        #   있었다 — 실물 request 는 반드시 갖는 필드다. 픽스처가 실물보다 좁으면 그 위의 단언은
        #   실물에서 성립하는 성질을 시험하지 못한다(원격 timeout 래핑이 그 예였다).
        "timeout_seconds": 60,
        "max_turns": 1,
        "capabilities": ["read"],
        "target": target,
    }


def _test_execution_approval_authorization() -> None:
    """`authorize --action sync_to_sub --mode experimental` 의 **양방향**을 실제로 실행한다.

    2026-09-03(plan_26090317 P3): 이 경로는 `plan_bytes` 미정의로 **NameError 를 내며 한 번도
    성공한 적이 없었다** — 그런데 어떤 검사도 그것을 실행하지 않아 3주 넘게 아무도 몰랐다.
    "안내 문구가 틀렸다"(B0)의 아래층에 "고쳐도 그 다음 줄에서 죽는다" 가 있었던 셈이다.
    앞으로는 정상 통과와 4종 거부를 **매번 실행해서** 확인한다(단언이 아니라 실행).
    """
    import hashlib
    import shutil
    import subprocess as _sp
    gate = REPO_ROOT / ".claude/policies/runtime/completion_gate.py"
    approved_by, approved_at = "selftest", "2026-01-01T00:00:00Z"
    atoms = [f"approved_by: {approved_by}", f"approved_at_utc: {approved_at}",
             "allowed_action: sync_to_sub"]
    plan_body = ("# selftest plan\n\n## Execution approval\n\n" + "\n".join(atoms) + "\n")

    with tempfile.TemporaryDirectory() as td:
        repo = Path(td) / "repo"
        (repo / "docs" / "plan").mkdir(parents=True)
        (repo / "docs" / "_evidence").mkdir(parents=True)
        # 게이트는 repo_root 안의 실재 파일만 받아들이므로 최소 저장소를 만든다.
        shutil.copy2(gate, repo / "gate.py")
        for extra in ("policy_registry.py",):
            src = REPO_ROOT / ".claude/policies/runtime" / extra
            if src.exists():
                shutil.copy2(src, repo / extra)
        plan_path = repo / "docs/plan/p.md"
        plan_path.write_text(plan_body, encoding="utf-8")
        sha = hashlib.sha256(plan_path.read_bytes()).hexdigest()
        rel = "../plan/p.md"

        def _manifest(**over):
            ea = {"approved": True, "approved_by": approved_by, "approved_at_utc": approved_at,
                  "plan_path": rel, "plan_sha256": sha, "approval_anchor": "## Execution approval",
                  "approval_atoms": list(atoms), "allowed_actions": ["sync_to_sub"]}
            ea.update(over)
            # evidence.plan.path 는 plan_path 를 따라간다 — 어긋나면 **앵커 검사 이전에** 경로
            #   불일치로 걸려, 이 시험이 겨냥한 가드가 아닌 다른 가드를 확인하게 된다.
            return {"schema_version": 1, "task_class": "harness_change",
                    "identity": {"model": "m", "gpu": "g", "vllm": "v", "quant": None,
                                 "topology": "single", "tp": 1},
                    "evidence": {"plan": {"path": ea["plan_path"]}},
                    "pii_scan": {"passed": True}, "execution_approval": ea}

        def _run(man, name):
            mp = repo / "docs/_evidence" / f"{name}.json"
            mp.write_text(json.dumps(man, ensure_ascii=False), encoding="utf-8")
            out = _sp.run([sys.executable, str(gate), "authorize", "--action", "sync_to_sub",
                           "--mode", "experimental", "--manifest", str(mp),
                           "--repo-root", str(repo)], capture_output=True, text=True)
            _require(out.stdout.strip(), f"authorize produced no JSON for {name}: {out.stderr[-400:]}")
            return json.loads(out.stdout)

        ok = _run(_manifest(), "ok")
        _require(ok.get("allowed") is True and ok.get("authorization_state") == "execution-approved",
                 f"a genuine execution approval must be admitted, got {ok.get('reason_codes')}")

        bad_sha = _run(_manifest(plan_sha256="0" * 64), "bad_sha")
        _require(bad_sha.get("allowed") is False
                 and "EXECUTION_APPROVAL_PLAN_DIGEST_MISMATCH" in bad_sha.get("reason_codes", []),
                 f"a plan whose bytes changed after approval must be rejected: {bad_sha}")

        not_approved = _run(_manifest(approved=False), "not_approved")
        _require(not_approved.get("allowed") is False
                 and "EXECUTION_APPROVAL_NOT_APPROVED" in not_approved.get("reason_codes", []),
                 f"approved=false must be rejected: {not_approved}")

        wrong_action = _run(_manifest(allowed_actions=["hint_create"],
                                      approval_atoms=atoms[:2] + ["allowed_action: hint_create"]),
                            "wrong_action")
        _require(wrong_action.get("allowed") is False,
                 f"an approval for a different action must not open sync_to_sub: {wrong_action}")

        no_anchor_plan = repo / "docs/plan/q.md"
        no_anchor_plan.write_text("# no anchor here\n", encoding="utf-8")
        no_anchor = _run(_manifest(plan_path="../plan/q.md",
                                   plan_sha256=hashlib.sha256(no_anchor_plan.read_bytes()).hexdigest()),
                         "no_anchor")
        _require(no_anchor.get("allowed") is False
                 and "EXECUTION_APPROVAL_PLAN_ATOMS_MISSING" in no_anchor.get("reason_codes", []),
                 f"a plan without the anchor/atoms must be rejected: {no_anchor}")


def _test_provider_turn_exhaustion_reachable() -> None:
    """`claude -p` 가 **exit 1 + 정상 result JSON** 으로 소진을 알리는 실제 형태를 재현한다.

    2026-09-04(plan_26090317 P4 라이브): 소진 분류 분기가 `returncode != 0` 조기 반환 뒤에 있어
    **한 번도 실행되지 않았다**. 단위 자체검사는 성공 경로만 봤고, 첫 라이브 위임이 알려줬다 —
    원장에 `budget=None`·`turns=None` 만 남아 다음 attempt 예산을 정할 근거가 사라진다.
    그러므로 여기서는 **실측 payload 모양 그대로** 넣고 세 값이 살아 나오는지 본다.
    """
    provider = agent_control._load_provider("claude_code")
    payload = {"type": "result", "subtype": "error_max_turns", "is_error": True,
               "num_turns": 26, "session_id": "sess-abc",
               "duration_ms": 812_345, "duration_api_ms": 640_000,
               "errors": ["Reached maximum number of turns (25)"],
               "result": "", "modelUsage": {}}

    class _Completed:
        returncode, stdout, stderr = 1, json.dumps(payload), ""

    real_run = provider.subprocess.run
    provider.subprocess.run = lambda *a, **k: _Completed()
    try:
        req = _request("local")
        req["max_turns"] = 25
        res = provider.invoke(req)
    finally:
        provider.subprocess.run = real_run

    _require(res["budget_outcome"] == "exhausted",
             f"turn exhaustion must be classified as exhausted, got {res.get('budget_outcome')!r} "
             f"-- an unreachable branch leaves the ledger with no basis to size the next attempt")
    _require(res["num_turns"] == 26 and res["session_id"] == "sess-abc",
             f"num_turns/session_id must survive a non-zero exit: {res}")
    _require(res["status"] == "execution_failed",
             "exhaustion is still a failure of that attempt -- it must not read as completed")
    # 2026-09-05(축 F): 결과가 **실제 결과 스키마**를 통과해야 한다. 필드 몇 개만 보던 종전 검사는
    #   새 required 필드가 빠져도 초록이었다(픽스처가 실물보다 좁다).
    _res_schema = agent_control._load_schema(agent_control.RESULT_SCHEMA_PATH)
    _require(not agent_control._schema_violations(res, _res_schema),
             f"provider result violates result schema: {agent_control._schema_violations(res, _res_schema)}")
    _require(res["duration_ms"] == 812_345 and res["duration_api_ms"] == 640_000,
             f"provider-reported durations must survive a non-zero exit: {res}")

    # 음성대조: payload 가 아예 없는 비-0 종료(전송 실패)는 여전히 NONZERO_EXIT 이고 원장 3필드는 null.
    class _Broken:
        returncode, stdout, stderr = 255, "", "ssh: connect failed"

    provider.subprocess.run = lambda *a, **k: _Broken()
    try:
        res2 = provider.invoke(_request("ssh"))
    finally:
        provider.subprocess.run = real_run
    _require(res2["reason_codes"] == ["NONZERO_EXIT"] and res2["budget_outcome"] is None
             and res2["num_turns"] is None,
             f"a transport failure has no budget story -- it must stay null, got {res2}")
    _require(res2["duration_ms"] is None and res2["duration_api_ms"] is None,
             f"unmeasured durations stay null (0 would read as 'finished instantly'): {res2}")

    # 외생 중단(원격 timeout 124 / SIGTERM 143)은 **예산 사건이 아니다** — 원장이 둘을 갈라야
    # 다음 attempt 의 처방이 뒤집히지 않는다(2026-09-05 · F).
    for _rc in (124, 143):
        class _Killed:
            returncode, stdout, stderr = _rc, "", "Terminated"
        provider.subprocess.run = lambda *a, **k: _Killed()
        try:
            res3 = provider.invoke(_request("ssh"))
        finally:
            provider.subprocess.run = real_run
        _require(res3["budget_outcome"] == "external_interruption",
                 f"rc={_rc} is an external interruption, not a budget outcome: {res3}")


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
    # 2026-09-05(G-A1): `--model sonnet` 토큰 강제는 **모델 핀**이었다 — 어댑터 경계는 "claude 문법이
    #   여기에만 산다" 를 지키면 되고, 어느 모델을 부르는지는 요청의 선언이다.
    for token in ("claude -p", "--model", "--output-format json"):
        _require(token not in agent_source, f"provider-specific token leaked into orchestrator: {token}")
        _require(token in provider_source, f"provider adapter lost required CLI token: {token}")

    local_argv = provider.build_argv(_request("local"))
    _require(local_argv[0] == "claude" and "--model" in local_argv,
             f"local provider argv malformed: {local_argv}")
    ssh_argv = provider.build_argv(_request("ssh"))
    # 2026-09-03(F5 · plan_26090317 P1): 위임 전송만 맨 ssh 였다 — 미등록 host key·패스프레이즈에서
    #   ssh 가 /dev/tty 를 읽으며 timeout_seconds(≤3600s)까지 멈추고, 그 뒤에도 **원격 claude 는 살아**
    #   서브 워크스페이스를 계속 편집했다(메인은 이미 실패로 기록한 뒤). 하드닝을 계약으로 고정한다.
    _require(ssh_argv[0] == "ssh", f"SSH provider argv malformed: {ssh_argv}")
    _require("-o" in ssh_argv and "BatchMode=yes" in ssh_argv and "ConnectTimeout=8" in ssh_argv,
             f"SSH delegation must never be able to prompt on a tty: {ssh_argv}")
    _require(ssh_argv[-2] == "probe@192.0.2.10" and ssh_argv[ssh_argv.index("--") + 1] == "probe@192.0.2.10",
             f"SSH destination misplaced: {ssh_argv}")
    remote_shell = shlex.split(ssh_argv[-1])
    _require(remote_shell[:2] == ["bash", "-lc"] and
             remote_shell[2].startswith("cd '/tmp/runtime probe' && "),
             f"SSH work_dir is not safely shell-quoted: {ssh_argv[-1]}")
    # 원격 동반사망: 클라이언트 timeout 만으로는 서브에 고아 에이전트가 남는다.
    _require("timeout " in remote_shell[2] and " claude " in remote_shell[2],
             f"remote command must be wrapped in `timeout` so the sub agent dies with the client: {remote_shell[2]}")
    # 2026-09-05(3-4): 서브 `-p` 릴레이는 재시도 워치독을 켠다. **순서가 계약이다** — env 대입은
    #   `timeout` 앞에 와야 한다(뒤에 두면 timeout 이 `VAR=1` 을 실행 파일로 알고 즉사한다).
    _remote_cmd = remote_shell[2].split("&&", 1)[1].strip()
    _require(_remote_cmd.startswith("CLAUDE_CODE_RETRY_WATCHDOG=1 timeout "),
             f"sub relay must set the retry watchdog before `timeout`: {_remote_cmd[:120]}")
    _require("CLAUDE_CODE_RETRY_WATCHDOG" not in " ".join(local_argv),
             "local transport is the main node's own plane -- the sub relay env must not leak into it")

    # 2026-09-05(G-A1): 모델은 **선언**이다 — 어댑터가 막지 않고, 실제로 돈 모델을 기록한다.
    #   종전 이 자리는 `--model opus` 를 exit 3 으로 차단했고, 그 한 줄 때문에 모델을 바꾸려면
    #   하네스를 고쳐야 했다(모델 과적합의 정면 사례 · audit_26090515 A1).
    _opus_payload = {"type": "result", "subtype": "success", "is_error": False,
                     "num_turns": 2, "session_id": "sess-opus", "result": "done",
                     "duration_ms": 4200, "duration_api_ms": 3900,
                     "modelUsage": {"claude-opus-5": {"canonicalModel": "claude-opus-5"}}}

    class _OpusRun:
        returncode, stdout, stderr = 0, json.dumps(_opus_payload), ""

    _real_run = provider.subprocess.run
    provider.subprocess.run = lambda *a, **k: _OpusRun()
    try:
        declared = _request("local")
        declared["model"] = "opus"
        result = provider.invoke(declared)
    finally:
        provider.subprocess.run = _real_run
    _require(result["status"] == "completed" and result["exit_code"] == 0,
             f"the model is a declaration, not a gate -- opus must run: {result}")
    _require(result["model_used"] == ["claude-opus-5"] and result["model_requested"] == "opus",
             f"the model that actually ran must be recorded: {result}")
    _res_schema2 = agent_control._load_schema(agent_control.RESULT_SCHEMA_PATH)
    _require(not agent_control._schema_violations(result, _res_schema2),
             f"non-Sonnet completed result must satisfy the result schema: {result}")

    # metadata 부재는 **기록의 부재**이지 차단 사유가 아니다(model_used=[] 로 남고 결과는 유효하다).
    _bare = dict(_opus_payload); _bare.pop("modelUsage")

    class _BareRun:
        returncode, stdout, stderr = 0, json.dumps(_bare), ""

    provider.subprocess.run = lambda *a, **k: _BareRun()
    try:
        bare_result = provider.invoke(_request("local"))
    finally:
        provider.subprocess.run = _real_run
    _require(bare_result["status"] == "completed" and bare_result["model_used"] == [],
             f"absent model metadata must be recorded as empty, not blocked: {bare_result}")
    _require(not agent_control._schema_violations(bare_result, _res_schema2),
             f"metadata-less completed result must satisfy the result schema: {bare_result}")

    # 2026-09-05 회귀: 권한 거부 경로는 거부된 도구 목록을 output 에 실어 돌려주는데, 결과 스키마의
    #   execution_failed 가지가 `output: const null` 이라 그 결과가 **스키마 위반**이 됐고
    #   orchestrator 가 PROVIDER_RESULT_INVALID 봉투로 갈아끼워 진단이 호출자에게 도달하지 못했다.
    _denied = {"type": "result", "subtype": "success", "is_error": False, "num_turns": 1,
               "session_id": "sess-deny", "result": "…", "duration_ms": 10, "duration_api_ms": 5,
               "permission_denials": [{"tool_name": "Write", "tool_input": {"path": "x"}}],
               "modelUsage": {"claude-sonnet-4-5": {"canonicalModel": "claude-sonnet-4-5"}}}

    class _DeniedRun:
        returncode, stdout, stderr = 0, json.dumps(_denied), ""

    provider.subprocess.run = lambda *a, **k: _DeniedRun()
    try:
        denied_result = provider.invoke(_request("local"))
    finally:
        provider.subprocess.run = _real_run
    _require(denied_result["reason_codes"] == ["PERMISSION_DENIED"]
             and "permission_denials" in (denied_result["output"] or ""),
             f"permission denial must carry its diagnosis: {denied_result}")
    _require(not agent_control._schema_violations(denied_result, _res_schema2),
             f"permission-denied result must satisfy the result schema: "
             f"{agent_control._schema_violations(denied_result, _res_schema2)}")


# ─────────────────────────────────────────────────────────────────────────────
# tripwire 3종 (2026-09-03 신설 · plan_26090222 P2)
#
# 왜 여기인가: 해시 중복층을 걷어낸 뒤 **재발을 막는 것**은 목록(allowlist)이 아니라 파생 술어여야
# 한다. 목록은 새 파일이 생기면 조용히 늦어지지만, 파생 술어는 "git 이 이미 아는 것을 또 적었다"는
# 성질 자체를 본다. 세 단언은 병목(pre-commit)에 걸리므로 **1초 예산**을 지킨다.
#
# ⚠ 픽스처 격리: 이 파일의 단언은 저장소 상태를 읽는다. 그런데 `_hint_repo()` 는 `git init -b
# selftest` 로 격리 레포를 만들고 completion_gate 를 그 안에서 돌린다 — 거기서 브랜치 부분집합
# 단언이 발화하면 셀프테스트가 **자기 자신을 RED** 로 만든다. 그래서 모든 진입점이
# `_is_canonical_repo()` 로 "정본(헌법 소유) 저장소인가"를 먼저 판별한다.
# ─────────────────────────────────────────────────────────────────────────────

REPO_ROOT = RUNTIME_DIR.parents[2]

# 헌법을 소유한 저장소만 가지는 구조적 표지. 픽스처 레포(`_hint_repo`)는 completion_gate·schemas·
# hint 스크립트만 복사하므로 이 셋을 동시에 갖지 못한다.
_CANONICAL_MARKERS = ("CLAUDE.md", ".claude/rules/workflow.md", ".claude/policies/registry.yaml")

_ALLOWED_BRANCHES = frozenset({"single-node", "multi-node", "hint"})
_ALLOWED_TAG_PREFIX = "hint/"
_BACKUP_SUFFIXES = (".bak", ".orig")
# 경로 부분문자열 술어. `백업` 은 2026-09-03 추가 — 이 저장소의 실제 백업 명명이 한글이라
# `backup` 만으로는 검출력이 0 이었다(P0 §5-①-a MINOR: `seed/이전 plan 백업` 156파일 미매칭).
_BACKUP_PATH_TOKENS = ("backup", "백업")
# 워킹트리 스캔에서 최상위만 잘라내는 디렉터리. `seed/` 는 사용자 보관소(비배포·비추적)이고
# `.git/` 은 git 내부다 — 둘 다 "습관 제도화" 의 대상이 아니다.
# `output/` 은 2026-09-03 추가(P0-C-④): 빌드/캐시 산출물이라 "백업 습관" 평면이 아니고,
# 컨테이너가 만든 하위 디렉터리에 읽기권한이 없어(`output/*/cache/vllm/modelinfos/…: Permission
# denied`) 스캔 자체가 불가능하다. 범위 밖으로 명시해야 아래 `onerror` 가 위양성 없이 산다.
_BACKUP_SCAN_PRUNE_TOP = frozenset({".git", "seed", "output"})

# tripwire ③: 걷어낸 해시 중복층 메커니즘의 이름. 산문이 이 이름을 다시 쓰면 사라진 기계를
# 가리키는 지시가 되살아난다(문서가 코드보다 오래 산다).
_RETIRED_HASH_MECHANISMS = (
    "tracked_index", "evidence_manifest", "governed_prose_snapshot",
    "plan_blob_sha1", "image_version_match", "patch_sha256",
)
# 범위 정의이지 allowlist 가 아니다: `docs/report/*` 는 "발행 시점이 고정된" 장르라(docs.md
# §명명 SSOT) 과거 발행분의 본문을 고쳐 쓰면 그 장르 규약 자체가 깨진다.
_PROSE_SCAN_EXTRA = ("CLAUDE.md", "README.md")

# tripwire ⑤ — 배포 산출물 PII. 범위 정의는 `.claude/rules/docs.md` §PII 스캔 적용 범위의
# **"배포 산출물"** 행 그대로다(추적 템플릿 `.claude/**`·`CLAUDE.md` · `docs/report/*` ·
# 카탈로그). 비배포 산문(`docs/{plan,devlog,testlog,...}`)은 그 표가 다른 강도를 규정하므로
# 범위 밖이고, **추적물만** 본다 — 배포되는 것은 추적물이고, 워킹트리의 비추적 로컬 설정
# (예: 스킬 `config.yaml` 의 운영자 경로)까지 잡으면 가드가 정상 상태를 상시 RED 로 만든다.
_FORBIDDEN_SCANNER_REL = ".claude/skills/wiki-desk/scripts/scan_forbidden_strings.py"
_DEPLOYED_PII_PREFIXES = (".claude/", "docs/report/", "hints/")
_DEPLOYED_PII_FILES = ("CLAUDE.md", "README.md", "HINTS.md")

# `ls-files -s` 의 gitlink(서브모듈) 모드. 이 술어의 범위는 blob 이므로 입력에서 제외한다.
_GITLINK_MODES = frozenset({"160000"})

# 대소문자 무관하게 잡고 **비교는 lower 정규화**한다(2026-09-03 P0-C-②): 룩어라운드는 이미
# 대문자를 배제했는데 본체가 `[0-9a-f]` 뿐이라, 추적 JSON/YAML 이 digest 를 대문자로 적으면
# 조용히 통과했다 — "allowlist 없는 파생 술어" 라는 성질이 대소문자에서 깨진다.
_HEX_CONST_RE = re.compile(r"(?<![0-9a-fA-F])(?:[0-9a-fA-F]{40}|[0-9a-fA-F]{64})(?![0-9a-fA-F])")


# 정본 판별이 건너뛰는 단언들의 이름. SKIPPED 를 출력할 때 **무엇이 안 돌았는지**를 같이
# 적기 위한 목록이다 — "건너뛰었다"만 말하고 무엇을 건너뛰었는지 안 말하면 여전히 반쪽 침묵이다.
_REPO_STATE_ASSERTIONS = (
    "tripwire①no-backup-artifacts(+refs/heads·tags·.gitignore)",
    "tripwire②no-tracked-digest-rewrite",
    "tripwire③no-retired-hash-mechanism-prose",
    "tripwire④no-duplicate-certificates",
    "tripwire⑤no-pii-in-deployed-artifacts",
    "executor-wiring(core.hooksPath·hook tracked)",
)


def _canonical_repo_reason(root: Path) -> str | None:
    """`root` 가 정본 저장소가 **아니라면 그 사유**를, 정본이면 None 을 돌려준다.

    ★ 2026-09-03 (적대검증 MAJOR ①): 예전에는 bool 만 돌려줬고, 호출부는 거짓일 때 조용히
    `return` 했다 — 그래서 **무력화된 가드와 통과한 가드가 출력에서 구분되지 않았다**(둘 다
    `[tripwire] PASS` rc=0). 표지 파일이 미래에 사라지면(이 캠페인이 방금 원장 3종을 지웠듯)
    가드 전체가 소리 없이 죽는데 아무도 모른다. 처방은 금지가 아니라 **표시**다
    (`workflow.md` §결정론 규율 — 침묵 폴백은 결함, 출처 표시가 처방).
    """
    missing = [m for m in _CANONICAL_MARKERS if not (root / m).is_file()]
    if missing:
        return f"missing canonical marker(s) {missing} under {root}"
    proc = subprocess.run(["git", "-C", str(root), "rev-parse", "--show-toplevel"],
                          capture_output=True, text=True, timeout=60)
    if proc.returncode != 0:
        return (f"`git rev-parse --show-toplevel` failed in {root} "
                f"(rc={proc.returncode}): {proc.stderr.strip()[:200]}")
    toplevel = Path(proc.stdout.strip())
    if toplevel.resolve() != root.resolve():
        return f"{root} is not a git work-tree root (toplevel={toplevel})"
    return None


def _is_canonical_repo(root: Path) -> bool:
    """`root` 가 헌법을 소유한 정본 저장소인가(= 저장소상태 단언을 돌려도 되는가)."""
    return _canonical_repo_reason(root) is None


def _announce_non_canonical(root: Path, prefix: str) -> str | None:
    """정본이 아니면 **구분되는 SKIPPED 한 줄**을 stderr 로 내고 사유를 돌려준다(정본이면 None).

    ⚠ stdout 계약: 진단은 전부 stderr 다. `completion_gate authorize` 의 stdout JSON 을
    `json.loads` 하는 소비자가 셋(`sync_branches.sh`·`sync_to_sub.sh`·`hint_tag.py`)이고,
    이 스크립트는 그 authorize 가 자식으로 띄운다.
    """
    reason = _canonical_repo_reason(root)
    if reason is None:
        return None
    print(f"[{prefix}] SKIPPED (non-canonical repo: {reason}) -- "
          f"repo-state assertions NOT run: {', '.join(_REPO_STATE_ASSERTIONS)}", file=sys.stderr)
    return reason


def _git_out(root: Path, *args: str) -> str:
    proc = subprocess.run(["git", "-C", str(root), *args],
                          capture_output=True, text=True, timeout=120)
    if proc.returncode != 0:
        raise RuntimeSelftestFailure(f"git {' '.join(args)} failed in {root}: {proc.stderr.strip()}")
    return proc.stdout


def _tracked_paths(root: Path) -> list[str]:
    """`-z` 로 읽는다 — `git ls-files` 의 기본 quotePath 가 한글 경로를 이스케이프해 거짓
    드리프트를 만든 선례가 있다(verify_distribution 회수 건)."""
    return [p for p in _git_out(root, "ls-files", "-z").split("\0") if p]


def _test_no_backup_artifacts(root: Path | None = None) -> None:
    """tripwire ① — 백업 관행 자체를 금지한다(숨기지 않는다).

    `.gitignore` 의 `*.bak`/`*.orig` 는 2026-09-03 에 삭제됐다: 무시는 관행을 제도화하고
    잔재를 `git status` 밖으로 숨긴다. 이 단언이 그 자리를 대신한다.
    """
    root = REPO_ROOT if root is None else root
    if not _is_canonical_repo(root):
        return

    offenders: list[str] = []
    # `os.walk` 기본 `onerror=None` 은 권한 오류를 **삼킨다** — 못 읽은 디렉터리 아래에 백업
    # 아티팩트가 있어도 가드가 초록을 낸다(스캔 실패와 "없음" 이 구분되지 않는다). 범위 안에서
    # 못 읽은 것은 판정 불가이므로 실패로 다룬다(2026-09-03 P0-C-④).
    scan_errors: list[str] = []

    def _on_scan_error(err: OSError) -> None:
        name = getattr(err, "filename", None)
        where = os.path.relpath(name, root) if name else "<unknown>"
        scan_errors.append(f"{where}: {err.strerror or err}")

    for dirpath, dirnames, filenames in os.walk(root, onerror=_on_scan_error):
        rel_dir = os.path.relpath(dirpath, root)
        if rel_dir == ".":
            dirnames[:] = [d for d in dirnames if d not in _BACKUP_SCAN_PRUNE_TOP]
        for name in filenames:
            rel = name if rel_dir == "." else os.path.join(rel_dir, name)
            low = rel.lower()
            if rel.endswith(_BACKUP_SUFFIXES) or any(tok in low for tok in _BACKUP_PATH_TOKENS):
                offenders.append(rel)
    _require(not offenders, f"backup artifact in working tree (백업 금지 · 숨김 금지): {offenders[:20]}")
    _require(not scan_errors,
             "backup scan could not read part of its own scope (스캔 실패 ≠ 없음): "
             f"{scan_errors[:20]}")

    branches = {b for b in _git_out(root, "for-each-ref", "--format=%(refname:short)",
                                   "refs/heads").split("\n") if b}
    stray = sorted(branches - _ALLOWED_BRANCHES)
    _require(not stray, f"refs/heads must be a subset of {sorted(_ALLOWED_BRANCHES)}: {stray}")

    tags = {t for t in _git_out(root, "tag", "-l").split("\n") if t}
    bad_tags = sorted(t for t in tags if not t.startswith(_ALLOWED_TAG_PREFIX))
    _require(not bad_tags, f"tags must all be under {_ALLOWED_TAG_PREFIX!r}: {bad_tags}")

    ignore_lines = {line.strip() for line in
                    (root / ".gitignore").read_text(encoding="utf-8").splitlines()}
    resurrected = sorted({"*.bak", "*.orig"} & ignore_lines)
    _require(not resurrected,
             f".gitignore must not hide backup artifacts (deleted 2026-09-03): {resurrected}")


def _tracked_blob_digests(root: Path) -> set[str]:
    """추적 blob 의 sha1(= git object id) ∪ sha256 을 한 번의 `cat-file --batch` 로 모은다.

    2026-09-03 (P0-C-③) 두 가지를 고쳤다.

    ① **gitlink 제외** — `ls-files -s` 는 서브모듈을 mode 160000 + **커밋** id 로 낸다. 커밋은
       blob 이 아니라 `--batch` 가 `<sha> missing`(2필드)을 내며, 이 술어의 범위 자체가 blob 이다
       (docstring 하단 참조: tree/commit 으로 넓히면 즉시 위양성). 그러니 입력에서 먼저 뺀다.
    ② **버린 배치 항목에 fail-loud** — 옛 파서는 2필드 헤더를 만나면 `break` 로 루프를 끊어
       **그 뒤 정렬 순서의 digest 를 전부 잃었고**, 그러면 tripwire ② 가 조용히 공허통과한다.
       이제는 건너뛰되 못 푼 항목을 세고, 하나라도 있으면 실패한다 — 결정·게이트 경로에서
       원인을 삼키는 침묵 폴백은 금지다(`workflow.md` §4종 안티패턴 판정표).
    """
    sha1s: set[str] = set()
    for entry in _git_out(root, "ls-files", "-s", "-z").split("\0"):
        if not entry:
            continue
        meta = entry.split("\t", 1)[0].split()
        if len(meta) < 2 or meta[0] in _GITLINK_MODES:
            continue
        sha1s.add(meta[1])
    digests: set[str] = set(sha1s)
    if not sha1s:
        return digests
    proc = subprocess.run(["git", "-C", str(root), "cat-file", "--batch"],
                          input=("\n".join(sorted(sha1s)) + "\n").encode(),
                          capture_output=True, timeout=180)
    buf = proc.stdout
    pos = 0
    resolved = 0
    unresolved: list[str] = []
    while pos < len(buf):
        nl = buf.find(b"\n", pos)
        if nl < 0:
            unresolved.append(f"<truncated batch output at byte {pos}>")
            break
        header = buf[pos:nl].decode("utf-8", "replace").split()
        if len(header) < 3 or header[1] != "blob" or not header[2].isdigit():
            unresolved.append(" ".join(header) or "<empty header>")
            pos = nl + 1
            continue
        size = int(header[2])
        body = buf[nl + 1:nl + 1 + size]
        digests.add(hashlib.sha256(body).hexdigest())
        resolved += 1
        pos = nl + 1 + size + 1
    _require(not unresolved and resolved == len(sha1s),
             "git cat-file --batch dropped tracked blobs — digest 집합이 불완전하면 tripwire ② 가 "
             "조용히 공허통과한다(침묵 폴백 금지): "
             f"resolved={resolved}/{len(sha1s)} unresolved={unresolved[:10]}")
    return digests


def _test_no_tracked_digest_rewrite(root: Path | None = None) -> None:
    """tripwire ② — **파생 술어**(allowlist 없음): git 이 이미 든 바이트의 해시를 추적 데이터가
    다시 적으면 FAIL.

    적중 조건이 "추적 blob 의 sha1 또는 sha256 과 같다" 이므로, 상수를 옮기거나 파일을 새로
    만들어도 술어가 따라간다 — 면제 목록을 유지할 필요가 없다.

    자연 통과(설계상 적중하지 않는 것들 — 면제가 아니라 **대상 밖**):
      · `output/*/build_patches_src/PROVENANCE.json` — 추적 파일 안의 **비추적** 상류 vLLM 소스
        digest(108건). git 이 그 바이트를 들고 있지 않으므로 추적 blob 집합에 없다.
      · Judge 골든 픽스처(`version_delta_*.json`) — 상류 vLLM 커밋 sha.
      · `hints/index.json` — 커밋/태그 **object id**(blob 이 아니다). 술어를 tree/commit 으로
        넓히면 즉시 위양성이 되므로 blob 으로 좁혀 둔다.
    """
    root = REPO_ROOT if root is None else root
    if not _is_canonical_repo(root):
        return

    digests = _tracked_blob_digests(root)
    offenders: list[str] = []
    for rel in _tracked_paths(root):
        if not rel.endswith((".json", ".yaml", ".yml")):
            continue
        path = root / rel
        if not path.is_file():
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        hits = sorted({m for m in _HEX_CONST_RE.findall(text) if m.lower() in digests})
        if hits:
            offenders.append(f"{rel}:{len(hits)} (e.g. {hits[0]})")
    _require(not offenders,
             "tracked data re-states a digest git already owns (single authority = git): "
             f"{offenders}")


def duplicate_certificate_groups(certs: dict[str, str]) -> tuple[dict[tuple, list[str]], list[str]]:
    """순수 술어(git 불요) — {상대경로: 본문} → (같은 측정 키를 가진 경로 그룹(크기>1), 식별불가 경로).

    키는 `completion_gate.certificate_run_key`(강한 6키 + measured_utc) 하나다 — 감지기와
    해소기가 같은 술어를 쓴다. 파싱 실패·키 결측은 "매치 없음" 이 아니라 **식별 불가**로 따로 낸다
    (부재≠결측 · 침묵 폴백 금지).
    """
    groups: dict[tuple, list[str]] = {}
    unkeyed: list[str] = []
    for rel, text in certs.items():
        fields, ok = completion_gate.parse_flat_certificate(text)
        key = completion_gate.certificate_run_key(fields) if ok else None
        if key is None:
            unkeyed.append(rel)
            continue
        groups.setdefault(key, []).append(rel)
    dups = {k: sorted(v) for k, v in groups.items() if len(v) > 1}
    return dups, sorted(unkeyed)


def _test_no_duplicate_certificates(root: Path | None = None) -> None:
    """tripwire ④ — **파생 술어**(allowlist 없음): 추적 인증서 두 장이 같은 측정이면 FAIL.

    왜(plan_26090410 §1): 발행기가 공급받은 인증서(이미 추적 원본)를 **발행 시각** 이름으로 복사해
    같은 측정이 두 파일로 추적됐다(2026-09-04 실측 8쌍 · 바이트 동일). `0afa3ee` 가 사본 1개를
    손으로 지웠지만 생성기가 남아 하루 만에 재발했다 — 이 술어가 그 재발을 pre-commit 에서 막는다.

    술어가 이름·해시가 아니라 **내용의 측정 키**로 판정하므로, 파일을 개명하거나 다른 시간대에
    다시 발행해도 따라간다. 사본이 원본과 바이트가 다르더라도(같은 측정 다른 내용) 걸린다 —
    그것은 더 나쁜 결함이라 메시지로 가른다.
    """
    root = REPO_ROOT if root is None else root
    if not _is_canonical_repo(root):
        return
    certs: dict[str, str] = {}
    for rel in _tracked_paths(root):
        if not (rel.startswith("docs/benchmark/benchmark_") and rel.endswith(".yaml")):
            continue
        path = root / rel
        if not path.is_file():
            continue
        try:
            certs[rel] = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError) as exc:
            _require(False, f"tracked certificate unreadable — fail-closed: {rel}: {exc}")
    dups, unkeyed = duplicate_certificate_groups(certs)
    _require(not unkeyed,
             "tracked certificate(s) cannot be keyed (flat parse failed or measured_utc/strong key "
             f"missing) — fail-closed, not 'no match': {unkeyed}")
    lines = []
    for key, rels in sorted(dups.items(), key=lambda kv: kv[1][0]):
        digests = {hashlib.sha256((root / r).read_bytes()).hexdigest() for r in rels}
        kind = "byte-identical (duplicate layer — delete all but one)" if len(digests) == 1 \
            else "DIFFERENT bytes for the same measurement (publication-chain defect)"
        lines.append(f"{key[0]}@{key[-1]} [{kind}]: {rels}")
    _require(not dups,
             "same measurement tracked under more than one certificate file (single authority = one "
             "file per (identity, measured_utc)):\n  " + "\n  ".join(lines))


def _test_duplicate_certificate_predicate() -> None:
    """tripwire ④ 술어의 hermetic 자체검사(음성대조 포함)."""
    base = ("schema_version: 1\nrecord_type: benchmark_certificate\nverdict: PASS\nmodel: m\n"
            'gpu_model: "NVIDIA GB10"\nvllm_version: 0.18.0\nquantization: mxfp4\ntopology: single\n'
            "tensor_parallel_size: 1\n")
    a = base + 'measured_utc: "2026-09-03T12:00:00Z"\n'
    b = base + 'measured_utc: "2026-09-03T12:00:01Z"\n'
    dups, unkeyed = duplicate_certificate_groups({"x.yaml": a, "y.yaml": a})
    _require(len(dups) == 1 and next(iter(dups.values())) == ["x.yaml", "y.yaml"] and not unkeyed,
             "같은 키 두 장은 한 그룹으로 잡혀야 한다")
    dups, unkeyed = duplicate_certificate_groups({"x.yaml": a, "y.yaml": b})
    _require(not dups and not unkeyed, "★음성대조 measured_utc 가 다르면 다른 측정이다")
    dups, unkeyed = duplicate_certificate_groups({"x.yaml": a, "z.yaml": base})
    _require(not dups and unkeyed == ["z.yaml"], "★키 결측은 '매치 없음' 이 아니라 식별불가로 나와야 한다")
    dups, unkeyed = duplicate_certificate_groups({"n.yaml": "a:\n  nested: 1\n"})
    _require(unkeyed == ["n.yaml"], "★파싱 실패도 식별불가")
    _require(completion_gate.CERTIFICATE_RUN_KEY_FIELDS[-1] == "measured_utc"
             and "image_digest" not in completion_gate.CERTIFICATE_RUN_KEY_FIELDS,
             "키는 강한 6키 + measured_utc 뿐 — 소프트 지문은 identity 가 아니다")


def _test_no_retired_hash_mechanism_prose(root: Path | None = None) -> None:
    """tripwire ③ — 걷어낸 메커니즘 이름이 규약 산문에 되살아나면 FAIL(음성 regex).

    범위는 `.claude/**/*.md` · `CLAUDE.md` · `README.md` 다. `docs/report/*` 는 발행 시점이
    고정된 장르라 **범위 밖**이며(범위 정의이지 allowlist 가 아니다), 과거 감사 보고서가 사라진
    기계를 서술하는 것은 정상이다.
    """
    root = REPO_ROOT if root is None else root
    if not _is_canonical_repo(root):
        return

    targets = sorted((root / ".claude").rglob("*.md"))
    targets += [root / name for name in _PROSE_SCAN_EXTRA]
    offenders: list[str] = []
    for path in targets:
        if not path.is_file():
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        for lineno, line in enumerate(text.splitlines(), 1):
            for name in _RETIRED_HASH_MECHANISMS:
                if name in line:
                    offenders.append(f"{path.relative_to(root)}:{lineno}:{name}")
    _require(not offenders, f"retired hash mechanism named in governing prose: {offenders[:20]}")


def _import_forbidden_scanner():
    """`scan_forbidden_strings.py` 를 in-process 로 적재한다(`_import_hint_tag` 와 같은 관용구).

    패턴·면제 규칙을 **복제하지 않는다** — 복제하면 두 자리가 갈라져 한쪽만 조용히 늦는다
    (`workflow.md` §결정론 규율 — 개념 중복). 커널은 그 파일의 `scan_lines` 하나다.
    """
    path = CLAUDE_DIR.parent / _FORBIDDEN_SCANNER_REL
    spec = importlib.util.spec_from_file_location("_runtime_selftest_pii_scan", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _deployed_pii_targets(root: Path) -> list[str]:
    """배포면에 해당하는 **추적** 경로(repo-relative)."""
    return [rel for rel in _tracked_paths(root)
            if rel.startswith(_DEPLOYED_PII_PREFIXES) or rel in _DEPLOYED_PII_FILES]


def _test_no_pii_in_deployed_artifacts(root: Path | None = None) -> None:
    """tripwire ⑤ — 추적 배포 산출물에 운영자 PII 가 실리면 FAIL.

    ★ 왜 tripwire 인가(2026-09-04 실측): 이 스캐너에는 **자동 실행자가 하나도 없었다**.
    두 스킬 문서가 "손으로 돌려라" 라고 적어 두었을 뿐이라, P5 에서 들어간 운영자 호스트명
    2건(자체검사 픽스처 1 · 주석 1)이 추적 파일로 커밋되고 브랜치싱크로 전파됐다. 헌법
    §노드 제어 5불변식 3 이 정확히 이 형태를 금지한다 — *"가드를 놓을 때는 그 처방을 누가
    실행하는가를 먼저 적는다. 실행자가 아예 없으면 안전장치가 아니라 교착이다."*

    ⚠ 강도 경계: 여기서 도는 것은 스캐너의 현행 패턴(리터럴 term + `GENERIC_PATTERNS`)이지
    hint 평면의 **4종 전부**가 아니다. 4종을 이 트리에 그대로 걸면 픽스처·플레이스홀더가
    대량으로 걸려(2026-09-04 실측 63건 중 진성 2건) 기준이 정의상 달성 불가가 된다 —
    `docs.md` 가 simlog/logs 에 대해 이미 편 논리와 같다. 4종 강도의 정본 집행자는
    `hint_tag.finalize/verify`(배포 평면 fail-closed)이며, 트리 스캐너의 generic 축을
    넓히려면 shell-default 판별자와 픽스처 면제를 함께 설계해야 한다(후속).
    """
    root = REPO_ROOT if root is None else root
    if not _is_canonical_repo(root):
        return

    scanner = _import_forbidden_scanner()
    terms = scanner.load_terms(scanner._default_terms_file())
    targets = _deployed_pii_targets(root)
    # 사정거리 0 방어 — 목록이 비면 이 가드는 "통과" 가 아니라 **무력**이다(선례: envelope 수확기).
    _require(targets, f"tripwire5 scanned nothing under {root} -- deployed surface enumeration is empty")

    offenders: list[str] = []
    for rel in targets:
        path = root / rel
        try:
            if path.stat().st_size > scanner.MAX_BYTES:
                print(f"[tripwire] WARN 5 skipped (too large) {rel}", file=sys.stderr)
                continue
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue  # 바이너리 — 확장자가 아니라 내용으로 판정(스캐너와 같은 규율)
        except OSError as exc:
            print(f"[tripwire] WARN 5 unreadable {rel}: {exc}", file=sys.stderr)
            continue
        offenders += [f"{rel}:{lineno}:{what}"
                      for lineno, what, exempt in scanner.scan_lines(text, terms) if not exempt]
    _require(not offenders,
             f"operator PII in tracked deployed artifacts ({len(offenders)}): {offenders[:20]}")


def _test_deployed_pii_predicate() -> None:
    """tripwire ⑤ 의 커널·면제·사정거리를 단위로 고정한다(라이브 트리 불요).

    라이브 트리는 **깨끗할 때 아무것도 증명하지 않는다** — 통과가 "검출력이 있다" 를 뜻하려면
    양성 입력이 실제로 발화해야 한다(역-오라클 회피).
    """
    scanner = _import_forbidden_scanner()
    hits = scanner.scan_lines("host 192.168.0.7\nclean line\n", ["secret-node"])  # pii-scan-fixture
    _require([h for h in hits if h[1].startswith("generic:")], "scan_lines must fire on private IPv4")
    _require(scanner.scan_lines("node secret-node", ["secret-node"])[0][1] == "term:secret-node",
             "scan_lines must fire on a literal term")
    exempt = scanner.scan_lines(f"192.168.0.7  # {scanner.EXEMPT_MARK}", [])  # pii-scan-fixture
    _require(exempt and exempt[0][2] is True, "EXEMPT_MARK must mark the row exempt, not drop it")
    _require(not scanner.scan_lines("GB10 2-node TP=2 · 53.92 t/s", ["secret-node"]),
             "clean text must not produce hits")
    if _is_canonical_repo(REPO_ROOT):
        targets = _deployed_pii_targets(REPO_ROOT)
        _require(len(targets) > 50 and "CLAUDE.md" in targets,
                 f"deployed surface enumeration looks wrong: {len(targets)} paths")


def _test_tripwire_executor_wiring(root: Path | None = None) -> list[str]:
    """실행자 자기검사 — `core.hooksPath` 설정과 훅 파일의 **추적 여부**.

    미설치 클론(막 클론한 배포본)에서는 hooksPath 가 아직 안 잡혀 있는 것이 정상이므로 FAIL 이
    아니라 **WARN** 이다(plan §10 risk). 반면 훅 파일이 추적되지 않는 것은 저작 결함이라
    같은 WARN 으로 보고하되, 두 경고 모두 stderr 로 나간다 — stdout JSON 계약을 오염시키지 않는다.

    비-정본 저장소에서 빈 리스트를 돌려주는 것은 여전히 정상 경로다. 다만 그 침묵이 tripwire
    3종의 침묵과 겹쳐 **이중 침묵**이 되던 것은 2026-09-03 에 닫혔다 — 호출부(`run_tripwires`·
    `main`)가 먼저 `_announce_non_canonical()` 로 SKIPPED 한 줄을 내고, 그 줄이 이 검사도
    건너뛰었음을 이름으로 밝힌다(`_REPO_STATE_ASSERTIONS`).
    """
    root = REPO_ROOT if root is None else root
    warnings: list[str] = []
    if not _is_canonical_repo(root):
        return warnings

    hook = root / ".claude/hooks/pre-commit"
    proc = subprocess.run(["git", "-C", str(root), "config", "--get", "core.hooksPath"],
                          capture_output=True, text=True, timeout=60)
    configured = proc.stdout.strip()
    if configured != ".claude/hooks":
        warnings.append(
            f"core.hooksPath is {configured!r}, expected '.claude/hooks' — "
            "run: git config core.hooksPath .claude/hooks")
    if not hook.is_file():
        warnings.append(".claude/hooks/pre-commit is missing")
    else:
        tracked = subprocess.run(
            ["git", "-C", str(root), "ls-files", "--error-unmatch", ".claude/hooks/pre-commit"],
            capture_output=True, text=True, timeout=60)
        if tracked.returncode != 0:
            warnings.append(".claude/hooks/pre-commit exists but is NOT tracked "
                            "(untracked hooks do not reach a clone)")
    return warnings


_WATCHDOG_PREDICATE_FILES = (
    ".claude/skills/terraforming_node/scripts/node_blackbox/mem_watchdog_eta.sh",
    ".claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh",
    ".claude/skills/terraforming_node/scripts/node_blackbox/thermal_watchdog.sh",
)
_WATCHDOG_PREDICATE_BLOCKS = (
    ("BB_TARGET_PREDICATE_V1", _WATCHDOG_PREDICATE_FILES),
    # 자체시험 블록은 self-test 를 가진 두 워치독에만 있다. 협역 워치독은 --self-test 진입점이
    # 없어 대상에서 빠지며, 그 빈자리는 술어 본문 parity 가 덮는다(같은 글자면 같은 판정이다).
    ("BB_TARGET_PREDICATE_SELFTEST_V1",
     (".claude/skills/terraforming_node/scripts/node_blackbox/mem_watchdog_eta.sh",
      ".claude/skills/terraforming_node/scripts/node_blackbox/thermal_watchdog.sh")),
)


def _extract_marked_block(text: str, marker: str) -> str | None:
    """`# ── <marker> …` 부터 `# ── /<marker> …` 까지를 그대로 돌려준다(없으면 None)."""
    start = end = None
    for lineno, line in enumerate(text.splitlines()):
        stripped = line.strip()
        if stripped.startswith(f"# ── /{marker}"):
            end = lineno
            break
        if start is None and stripped.startswith(f"# ── {marker}"):
            start = lineno
    if start is None or end is None or end <= start:
        return None
    return "\n".join(text.splitlines()[start:end + 1])


def _test_watchdog_target_predicate_parity(root: Path | None = None) -> None:
    """세 워치독의 킬 대상 술어가 **글자 그대로 같은지** 확인한다 (CP0 · plan_26090415 §3.3).

    왜 parity 인가: 이 술어는 세 파일에 복제돼 있고 **단일 소유가 불가능**하다 — 설치기가
    각 스크립트를 확장자 없는 단독 바이너리로 복사하므로 공유 파일을 source 하면 현장에서
    sibling 경로가 사라진다(2026-09-01 블랙박스 sibling import 파손 선례). 정적 파일끼리는
    한쪽이 다른 쪽을 생성할 수 없으므로 차선은 교차검증이다(`workflow.md` §결정론 규율 ·
    `assert_band2_top_gitignore_parity` 선례).

    갈라졌을 때의 대가가 비대칭이라 침묵을 허용하지 않는다: 한 워치독만 넓은 채로 남으면
    그 워치독이 트립할 때 **측정 컨테이너를 서빙과 함께 죽인다**. 그런데 그 사건은 OOM 압박
    구간에서만 재현되므로 평시 시험으로는 영영 드러나지 않는다.

    블록이 통째로 사라진 것도 FAIL 이다 — 부재를 "같다" 로 접으면 가드가 스스로 꺼진다.
    """
    root = REPO_ROOT if root is None else root
    if not _is_canonical_repo(root):
        return

    for marker, files in _WATCHDOG_PREDICATE_BLOCKS:
        blocks: dict[str, str] = {}
        for rel in files:
            path = root / rel
            _require(path.is_file(), f"watchdog predicate carrier missing: {rel}")
            block = _extract_marked_block(path.read_text(encoding="utf-8"), marker)
            _require(block is not None, f"{rel} lost its {marker} block (guard would silently disarm)")
            blocks[rel] = block
        distinct = sorted(set(blocks.values()))
        _require(
            len(distinct) == 1,
            f"{marker} diverged across {len(files)} carriers: "
            + ", ".join(f"{rel}={hashlib.sha256(b.encode()).hexdigest()[:12]}"
                        for rel, b in blocks.items()),
        )

    # 술어가 **살아 있는지**는 parity 가 답하지 못한다(셋 다 똑같이 망가질 수 있다).
    # self-test 를 가진 워치독을 실제로 돌려 음성대조까지 통과하는지 본다.
    for rel in (".claude/skills/terraforming_node/scripts/node_blackbox/mem_watchdog_eta.sh",
                ".claude/skills/terraforming_node/scripts/node_blackbox/thermal_watchdog.sh"):
        proc = subprocess.run(["bash", str(root / rel), "--self-test"],
                              capture_output=True, text=True, timeout=120)
        _require(proc.returncode == 0, f"{rel} --self-test failed: {proc.stdout[-800:]}")
        _require("★음성대조" in proc.stdout,
                 f"{rel} --self-test ran without the kill-target negative control")


def run_tripwires(root: Path | None = None) -> int:
    """병목(pre-commit·authorize)에서 도는 축약 진입점. 1초 예산.

    ★ 2026-09-03: 비-정본 저장소에서는 `PASS` 가 아니라 **`SKIPPED`** 를 낸다. rc 는 여전히 0
    이다(격리 픽스처에서 도는 것이 정상 경로이므로 차단하면 셀프테스트가 자기 자신을 RED 로
    만든다) — 바뀐 것은 **rc 가 아니라 가시성**이다. 무력화된 가드가 통과한 가드처럼 보이지
    않는 것, 그것 하나가 이 변경의 전부다.
    """
    root = REPO_ROOT if root is None else root
    if _announce_non_canonical(root, "tripwire") is not None:
        return 0
    try:
        _test_no_backup_artifacts(root)
        _test_no_tracked_digest_rewrite(root)
        _test_no_retired_hash_mechanism_prose(root)
        _test_no_duplicate_certificates(root)      # ④ plan_26090410 P4 — 사본 정리 뒤 배선
        _test_no_pii_in_deployed_artifacts(root)   # ⑤ P6 — 스캐너에 실행자가 없던 것을 배선
    except RuntimeSelftestFailure as exc:
        print(f"[tripwire] FAIL {exc}", file=sys.stderr)
        return 1
    for warning in _test_tripwire_executor_wiring(root):
        print(f"[tripwire] WARN {warning}", file=sys.stderr)
    print("[tripwire] PASS", file=sys.stderr)
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--tripwires-only", action="store_true",
        help="run only the pre-commit tripwires (backup artifacts / tracked digest rewrite / "
             "retired-mechanism prose / duplicate certificates / deployed-artifact PII); "
             "1s budget, diagnostics on stderr")
    args = parser.parse_args(argv)  # argv=None -> argparse reads sys.argv[1:]

    if args.tripwires_only:
        return run_tripwires()

    _test_no_production_asserts()
    _test_completion_gate()
    _test_promotion_rubric_carrier()
    _test_certificate_run_resolution()
    _test_hint_binding_source()
    _test_policy_and_evidence_lifecycle()
    _test_provider_turn_exhaustion_reachable()
    _test_execution_approval_authorization()
    _test_agent_provider_boundary()
    _test_duplicate_certificate_predicate()
    _test_deployed_pii_predicate()
    _test_watchdog_target_predicate_parity()
    # tripwire 5종은 축약 진입점과 **같은 함수**를 돈다 — 두 벌로 갈라지면 갈라진 쪽이 조용히
    # 늦는다(선례 3건). 전체 실행에서도 반드시 검사한다.
    # 비-정본 저장소에서 그 단언들이 no-op 이 되는 것은 `run_tripwires` 와 **같은 정상 경로**이며,
    # 여기서도 같은 SKIPPED 한 줄로 눈에 보이게 한다(침묵 no-op 금지).
    _announce_non_canonical(REPO_ROOT, "runtime_selftest")
    _test_no_backup_artifacts()
    _test_no_tracked_digest_rewrite()
    _test_no_retired_hash_mechanism_prose()
    _test_no_duplicate_certificates()
    _test_no_pii_in_deployed_artifacts()
    for warning in _test_tripwire_executor_wiring():
        print(f"[runtime_selftest] WARN {warning}", file=sys.stderr)
    print("[runtime_selftest] PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
