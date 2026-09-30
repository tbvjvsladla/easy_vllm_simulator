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
import fnmatch
import importlib.util
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import completion_gate
import evidence_publisher
import policy_registry
import verify_distribution

RUNTIME_DIR = Path(__file__).resolve().parent
PREDICATES_DIR = RUNTIME_DIR.parent / "predicates"
CLAUDE_DIR = RUNTIME_DIR.parents[1]


class RuntimeSelftestFailure(RuntimeError):
    """A required production regression property did not hold."""


def _require(condition: object, message: str) -> None:
    if not condition:
        raise RuntimeSelftestFailure(message)


def _walk_governing_files(root: Path, pattern: str):
    """Yield files governed by ``root``, excluding nested harness worktrees."""
    claude_dir = root / ".claude"
    for dirpath, dirnames, filenames in os.walk(claude_dir):
        if Path(dirpath) == claude_dir:
            # This is the harness isolation container, not a governed subtree.
            dirnames[:] = [name for name in dirnames if name != "worktrees"]
        for name in filenames:
            if fnmatch.fnmatch(name, pattern):
                yield Path(dirpath) / name


def _test_no_production_asserts(root: Path | None = None) -> None:
    root = REPO_ROOT if root is None else root
    offenders: list[str] = []
    for path in _walk_governing_files(root, "*.py"):
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
        offenders.extend(f"{path.relative_to(root)}:{node.lineno}"
                         for node in ast.walk(tree) if isinstance(node, ast.Assert))
    _require(not offenders, f"bare assert is optimization-unsafe: {offenders}")


def _test_nested_worktree_isolation() -> None:
    """A nested registered worktree's stale content and ref cannot govern its parent."""
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / "CLAUDE.md").write_text("fixture\n", encoding="utf-8")
        (root / ".claude/rules").mkdir(parents=True)
        (root / ".claude/rules/workflow.md").write_text("fixture\n", encoding="utf-8")
        (root / ".claude/policies").mkdir(parents=True)
        (root / ".claude/policies/registry.yaml").write_text("fixture\n", encoding="utf-8")
        (root / ".gitignore").write_text("fixture\n", encoding="utf-8")
        subprocess.run(["git", "init", "-q", "-b", "single-node", str(root)], check=True,
                       capture_output=True)
        subprocess.run(["git", "-C", str(root), "add", "."], check=True, capture_output=True)
        subprocess.run(["git", "-C", str(root), "-c", "user.name=fixture",
                        "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture"],
                       check=True, capture_output=True)
        nested = root / ".claude/worktrees/stale"
        special = root / ".claude/worktrees/space path"
        trailing = root / ".claude/worktrees/trailing-space "
        raw_bytes = os.fsencode(root / ".claude/worktrees") + b"/raw-\xff"
        raw = Path(os.fsdecode(raw_bytes))
        trailing_cr_bytes = os.fsencode(root / ".claude/worktrees") + b"/trailing-cr\r"
        trailing_cr = Path(os.fsdecode(trailing_cr_bytes))
        detached = root / ".claude/worktrees/detached"
        subprocess.run(["git", "-C", str(root), "worktree", "add", "-q", "-b", "stale-root",
                        str(nested)], check=True, capture_output=True)
        subprocess.run(["git", "-C", str(root), "worktree", "add", "-q", "-b", "space-root",
                        str(special)], check=True, capture_output=True)
        subprocess.run(["git", "-C", str(root), "worktree", "lock", str(special)], check=True,
                       capture_output=True)
        subprocess.run(["git", "-C", str(root), "worktree", "add", "-q", "-b", "trailing-root",
                        str(trailing)], check=True, capture_output=True)
        raw_created = False
        raw_result = subprocess.run([b"git", b"-C", os.fsencode(root), b"worktree", b"add", b"-q",
                                     b"-b", b"raw-root", raw_bytes], capture_output=True)
        if raw_result.returncode == 0:
            raw_created = True
        else:
            print("[runtime_selftest] SKIPPED undecodable worktree path fixture: "
                  + raw_result.stderr.decode(errors="replace").strip(), file=sys.stderr)
        cr_created = False
        cr_result = subprocess.run([b"git", b"-C", os.fsencode(root), b"worktree", b"add", b"-q",
                                    b"-b", b"trailing-cr-root", trailing_cr_bytes], capture_output=True)
        if cr_result.returncode == 0:
            cr_created = True
        else:
            print("[runtime_selftest] SKIPPED trailing-CR worktree path fixture: "
                  + cr_result.stderr.decode(errors="replace").strip(), file=sys.stderr)
        subprocess.run(["git", "-C", str(root), "worktree", "add", "-q", "--detach", str(detached)],
                       check=True, capture_output=True)
        bare = root / "bare.git"
        subprocess.run(["git", "init", "-q", "--bare", str(bare)], check=True, capture_output=True)
        try:
            expected_live = {"single-node", "stale-root", "space-root", "trailing-root"}
            if raw_created:
                expected_live.add("raw-root")
            if cr_created:
                expected_live.add("trailing-cr-root")
            _require(_live_registered_worktree_branches(root) == expected_live,
                     "live/locked/special/trailing/raw or detached worktree registration parsed incorrectly")
            _require(not _live_registered_worktree_branches(bare),
                     "bare repository yielded a worktree branch")
            (nested / "stale.py").write_text("assert False\n", encoding="utf-8")
            (nested / "stale.bak").write_text("stale\n", encoding="utf-8")
            (nested / "stale.md").write_text("tracked_index\n", encoding="utf-8")
            _test_no_production_asserts(root)
            _test_no_backup_artifacts(root)
            _test_no_retired_hash_mechanism_prose(root)
            _require(not verify_distribution._shipped_python_paths(root),
                     "distribution scan included nested worktree Python")

            current_python = root / ".claude/current.py"
            current_python.write_text("assert False\n", encoding="utf-8")
            _require(verify_distribution._shipped_python_paths(root) == [current_python],
                     "distribution scan omitted current-tree Python")
            try:
                _test_no_production_asserts(root)
            except RuntimeSelftestFailure:
                pass
            else:
                raise RuntimeSelftestFailure("Python scan did not catch current-tree violation")
            current_python.unlink()

            current_backup = root / ".claude/current.bak"
            current_backup.write_text("current violation\n", encoding="utf-8")
            try:
                _test_no_backup_artifacts(root)
            except RuntimeSelftestFailure:
                pass
            else:
                raise RuntimeSelftestFailure("backup scan did not catch current-tree violation")
            current_backup.unlink()

            current_prose = root / ".claude/current.md"
            current_prose.write_text("tracked_index\n", encoding="utf-8")
            try:
                _test_no_retired_hash_mechanism_prose(root)
            except RuntimeSelftestFailure:
                pass
            else:
                raise RuntimeSelftestFailure("prose scan did not catch current-tree violation")
            current_prose.unlink()

            # A live registered worktree branch is allowed; an abandoned registration is not.
            shutil.rmtree(nested)
            try:
                _test_no_backup_artifacts(root)
            except RuntimeSelftestFailure as exc:
                _require("stale-root" in str(exc),
                         f"prunable worktree ref was not reported as stray: {exc}")
            else:
                raise RuntimeSelftestFailure("prunable worktree ref was accepted")
            subprocess.run(["git", "-C", str(root), "worktree", "prune"], check=True,
                           capture_output=True)
            subprocess.run(["git", "-C", str(root), "branch", "-D", "stale-root"], check=True,
                           capture_output=True)

            subprocess.run(["git", "-C", str(root), "branch", "worktree-prefix-orphan"], check=True,
                           capture_output=True)
            try:
                _test_no_backup_artifacts(root)
            except RuntimeSelftestFailure as exc:
                _require("worktree-prefix-orphan" in str(exc),
                         f"prefix-only ref was not reported as stray: {exc}")
            else:
                raise RuntimeSelftestFailure("prefix-only worktree ref was accepted")
            subprocess.run(["git", "-C", str(root), "branch", "-D", "worktree-prefix-orphan"],
                           check=True, capture_output=True)
        finally:
            for worktree in (nested, special, trailing, raw, trailing_cr, detached):
                if worktree.exists():
                    subprocess.run(["git", "-C", str(root), "worktree", "unlock", str(worktree)],
                                   capture_output=True)
                    subprocess.run(["git", "-C", str(root), "worktree", "remove", "--force", str(worktree)],
                                   check=True, capture_output=True)


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

# lite 등급 인증서(2026-09-29 plan_26092923 · lite ⊂ full) -- hint_map_only 의 발행 자격 근거. 강한 6키는 위와 같다.
_PROMO_LITE_CERTIFICATE = """schema_version: 1
record_type: benchmark_certificate
verdict: not_applicable
model: selftest-model
gpu_model: GB10
vllm_version: 0.0.0.dev0
quantization: fp8
topology: single
tensor_parallel_size: 1
benchmark_mode: lite
lite_verdict: pass
entry_path: beta_lite_only_cell
lite_included: true
lite_gen_tps_warm: 40.0
measured_utc: "2026-01-01T00:00:00Z"
measured_node: main
"""

# manifest carrier(REFUTE 런) -- 인증서와 **동일한 계약**을 만족하는 최소 선언.
_PROMO_RUBRIC = {"rubric_authority": "explore", "floor_tps": 22.1,
                 "ratio_M_over_primary": 0.719, "primary_source": "E(external_reference)",
                 "rubric_source": "verdict_json"}


# bench_report 는 **PASS/REFUTE 무관하게 항상 발행되는** 문서라, 인증서가 구조적으로 없는 경로
# (perf_waiver · explore · hint_map_only — map_only 는 경량 리포트 또는 강등 셀의 리포트 · 2026-09-14)에서 lite 정량지표의
# 유일한 근거가 된다(옛 hint_tag `_require_serving_evidence` B 브랜치 → 2026-09-22 `hintlib.evidence` 결손 술어). 그래서 공용 픽스처의 리포트도 **실제로 lite 절과 실측 열을 갖는다** -- 빈 리포트를 쓰면
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
                              bench_report_text: str = _LITE_BENCH_REPORT,
                              task_class: str = "full_benchmark",
                              drop_evidence: tuple = ()) -> Path:
    """full_benchmark work-manifest + 그 증거 아티팩트를 `root` 안에 실제로 짓는다.

    승격 게이트 회귀(`_test_promotion_rubric_carrier` · `_test_certificate_run_resolution` ·
    `_test_hint_map_only_promotion`)가 **같은 픽스처**를 쓴다 -- 여러 평면이 같은 manifest 모양을
    각자 손으로 지으면 오늘 고치는 바로 그 "같은 가정이 여러 곳" 결함이 픽스처에서 다시 자란다.
    ⚠ 게이트 평면 전용이다(2026-09-22): hint 발행 E2E 는 이 손 manifest 를 쓰지 않는다 — 발행기의 work-manifest 는
    `hint.py publish` 가 evidence_publisher 를 실제로 구동해 만든다(AC5 손 JSON 0 · `_test_hint_cli_publication`).
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
    # ★ 2026-09-07: 실물 trial vault 는 **종결 기록**을 든다(`run_summary.json`). 픽스처가 그것을
    #   빠뜨리면 "vault 존재 = run 종결" 이라는 틀린 전제 위에서 시험이 초록으로 남는다 —
    #   실제로 그 전제 때문에 generate 실패로 종결 기록이 없는 vault 가 게이트를 통과했다
    #   (유예 결함 ⑥). 픽스처를 실물만큼 넓힌다.
    (simlog / "run_summary.json").write_text(
        json.dumps({"run_id": "selftest_run", "converged": True, "trial_count": 1,
                    "provenance": "mock"}) + "\n", encoding="utf-8")
    paths["simlog"] = os.path.relpath(simlog, evidence_dir)
    if certificate is not None:
        artifact = root / "docs" / "benchmark" / "benchmark_selftest.yaml"
        artifact.write_text(certificate, encoding="utf-8")
        paths["certificate"] = os.path.relpath(artifact, evidence_dir)

    benchmark = {"mode": "full", "verdict": verdict}
    benchmark.update(benchmark_extra or {})
    for _k in drop_evidence:
        paths.pop(_k, None)
    manifest = {
        "schema_version": 1,
        "task_class": task_class,
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
# 인증서가 발행되는 판정만 인증서를 요구한다 (2026-09-29 · plan_26092908 §4.8 "explore PASS 인증서 이음매" · V11④)
# ---------------------------------------------------------------------------------------------
# 실측 결함: DS4F `ds4f0731-1m-spec7-roce`(PASS · explore)가 `EVIDENCE_MISSING:certificate` 로 발행 게이트에서 막혔다 —
# 벤치 스킬은 explore PASS 에 인증서를 내지 않는데(judge_bench 자동 발행 = explicit ∧ PASS) 게이트는 full_benchmark ∧ PASS
# 전부에 인증서를 요구했다. 같은 explore 의 REFUTE(D1)는 인증서 요구가 없어 bench_report + manifest carrier 로 통과했다.
# ★ 음성대조: 수정 전 게이트에 W2 를 넣으면 `EVIDENCE_MISSING:certificate` 로 막힌다(2026-09-29 확인) — W2 의 PASS 는
#   "가드를 껐다"가 아니라 "요구가 발행 조건과 같은 술어가 됐다"의 증거다. W1·W5·W6·W7 이 면제가 옆문이 아님을 친다.
# =============================================================================================

def _bench_report_with_verdict_table(verdict: str, authority: str) -> str:
    """render_report.py 의 판정 절 모양 그대로(행 문구는 아래 writer 앵커 대조가 지킨다)."""
    return (_LITE_BENCH_REPORT + "\n## 판정 (표시만 — verdict_rule.py 결과)\n\n| 항목 | 값 |\n|---|---|\n"
            f"| verdict | **{verdict}** |\n| 측정 decode t/s (동시성1) | 20.0 t/s |\n"
            f"| 루브릭 권한 | {authority} |\n| floor (primary×(1−tol)) | 22.1 t/s |\n\n## 다음 절\n")


def _test_certificate_waiver_by_authority() -> None:
    gate = completion_gate

    # U1 요구 증거 행렬 — 인증서는 PASS ∧ (explicit ∨ 권한 모름)에서만.
    for verdict, authority, expect_cert in (
            ("PASS", "explicit", True), ("PASS", None, True), ("PASS", "explore", False), ("PASS", "weak", False),
            ("REFUTE", "explicit", False), ("REFUTE", "explore", False), ("FAIL", None, False)):
        got = "certificate" in gate.required_evidence_for("full_benchmark", {}, verdict, authority)
        _require(got is expect_cert,
                 f"required_evidence_for(full_benchmark, {verdict}, {authority}) certificate={got} (expected {expect_cert})")
    _require("certificate" not in gate.required_evidence_for("hint_map_only", {}, "PASS", None),
             "hint_map_only must never require a certificate")
    # U2 권한은 **출처 표시된** rubric 에서만 — 표시 없는 explore 는 면제를 열지 못한다.
    _require(gate.certificate_requirement_authority(dict(_PROMO_RUBRIC)) == "explore",
             "a verdict_json-sourced explore authority was not recognized")
    for bad in (dict(_PROMO_RUBRIC, rubric_source=None), dict(_PROMO_RUBRIC, rubric_source="hand"),
                dict(_PROMO_RUBRIC, rubric_authority="EXPLORE"), {}, None):
        _require(gate.certificate_requirement_authority(bad) is None,
                 f"an unsourced/invalid rubric opened the certificate waiver: {bad!r}")
    # U3 판정 표 파서 — 강조 벗김 · 표 밖 산문 무시 · 행 부재·중복은 unparseable.
    table, state = gate.bench_report_verdict_table(_bench_report_with_verdict_table("PASS", "explore"))
    _require(state == "parsed" and table == {"verdict": "PASS", "rubric_authority": "explore"},
             f"verdict table parse drift: {state} {table!r}")
    _require(gate.bench_report_verdict_table(_LITE_BENCH_REPORT)[0] is None, "absent verdict section parsed as a table")
    _require(gate.bench_report_verdict_table(
        "| verdict | **PASS** |\n| 루브릭 권한 | explore |\n")[0] is None,
        "rows outside the 판정 section were accepted")
    dup = _bench_report_with_verdict_table("PASS", "explore").replace("| 루브릭 권한 | explore |",
                                                                      "| 루브릭 권한 | explore |\n| verdict | REFUTE |")
    _require(gate.bench_report_verdict_table(dup)[1].startswith("unparseable"), "duplicate verdict row not rejected")
    # U4 writer 앵커 — 판정 절의 writer(render_report.py)가 파서가 읽는 머리·행 문구를 그대로 쓰는가.
    writer = (REPO_ROOT / ".claude/skills/adversarial-benchmark/scripts/render_report.py")
    if writer.is_file():
        src = writer.read_text(encoding="utf-8")
        for anchor in (gate.BENCH_REPORT_VERDICT_SECTION_PREFIX, '"| verdict | **%s** |"', '"| 루브릭 권한 | %s |"'):
            _require(anchor in src, f"render_report.py no longer writes the verdict-table anchor {anchor!r} "
                                    f"(completion_gate.bench_report_verdict_table 와 계약이 갈라졌다)")

    def verify(verdict, rubric, report_text, certificate=None) -> dict:
        with tempfile.TemporaryDirectory(prefix="certificate-waiver-selftest.") as td:
            root = Path(td)
            (root / ".git").mkdir()
            manifest_path = _write_promotion_manifest(root, verdict, rubric, certificate,
                                                      bench_report_text=report_text)
            proc = subprocess.run(
                [sys.executable, str(RUNTIME_DIR / "completion_gate.py"), "verify",
                 "--manifest", str(manifest_path), "--repo-root", str(root)],
                capture_output=True, text=True, timeout=120)
            try:
                out = json.loads(proc.stdout)
            except json.JSONDecodeError as exc:
                raise RuntimeSelftestFailure(f"completion_gate verify unparseable: {exc}; "
                                             f"stderr={proc.stderr.strip()[-600:]!r}") from exc
            out["_returncode"] = proc.returncode
            return out

    def promoted(out) -> bool:
        return out.get("state") == "promotion-ready" and out.get("eligible_for_promotion") is True \
            and out.get("_returncode") == 0

    def codes(out) -> list:
        return out.get("reason_codes") or []

    explicit = dict(_PROMO_RUBRIC, rubric_authority="explicit")
    # W1 ★음성: explicit PASS 인데 인증서 없음 → 거부 유지(인증서가 발행되는 판정).
    out = verify("PASS", explicit, _bench_report_with_verdict_table("PASS", "explicit"))
    _require(not promoted(out) and "EVIDENCE_MISSING:certificate" in codes(out),
             f"explicit PASS without a certificate was not rejected: {codes(out)}")
    # W2 ★양성: explore PASS + bench_report(판정 표 일치) · 인증서 없음 → 승격(DS4F 경로).
    out = verify("PASS", dict(_PROMO_RUBRIC), _bench_report_with_verdict_table("PASS", "explore"))
    _require(promoted(out) and "CERTIFICATE_WAIVED_NON_ISSUING_AUTHORITY" in codes(out)
             and "EVIDENCE_MISSING:certificate" not in codes(out)
             and (out.get("checked_evidence") or {}).get("certificate", {}).get("required") is not True
             and (out.get("rubric") or {}).get("authority_source") == "manifest_benchmark",
             f"explore PASS + bench_report did not satisfy the gate: {codes(out)}")
    # W3 ★양성: REFUTE(explore) + bench_report → 승격(D1 경로 · 인증서 요구 없음 불변).
    out = verify("REFUTE", dict(_PROMO_RUBRIC), _bench_report_with_verdict_table("REFUTE", "explore"))
    _require(promoted(out) and "BENCHMARK_EXPLORE_AUTHORITY_PROMOTION" in codes(out),
             f"explore REFUTE + bench_report regressed: {codes(out)}")
    # W4 weak PASS + bench_report(일치) → 승격(weak 도 비발행 권한 · manifest 계약 통과).
    out = verify("PASS", dict(_PROMO_RUBRIC, rubric_authority="weak"), _bench_report_with_verdict_table("PASS", "weak"))
    _require(promoted(out) and "CERTIFICATE_WAIVED_NON_ISSUING_AUTHORITY" in codes(out),
             f"weak PASS + bench_report did not satisfy the gate: {codes(out)}")
    # W5 ★음성: 판정 표가 없거나 다른 권한/판정을 말하면 면제 근거가 서지 않는다.
    for label, text in (("표 없음", _LITE_BENCH_REPORT),
                        ("권한 불일치", _bench_report_with_verdict_table("PASS", "explicit")),
                        ("판정 불일치", _bench_report_with_verdict_table("REFUTE", "explore"))):
        out = verify("PASS", dict(_PROMO_RUBRIC), text)
        _require(not promoted(out) and "CERTIFICATE_WAIVER_UNCORROBORATED" in codes(out),
                 f"waiver opened without bench_report corroboration ({label}): {codes(out)}")
    # W6 ★음성: 출처 표시 없는 explore(손저작 가능 평면) → 권한 모름 → 인증서 요구 유지.
    out = verify("PASS", dict(_PROMO_RUBRIC, rubric_source=None), _bench_report_with_verdict_table("PASS", "explore"))
    _require(not promoted(out) and "EVIDENCE_MISSING:certificate" in codes(out),
             f"an unsourced explore authority waived the certificate: {codes(out)}")
    # W7 ★음성: 면제된 PASS 도 공허 PASS 배제(floor>0)는 그대로 — 인증서 계약을 manifest carrier 가 대신 진다.
    out = verify("PASS", dict(_PROMO_RUBRIC, floor_tps=0), _bench_report_with_verdict_table("PASS", "explore"))
    _require(not promoted(out) and "CERTIFICATE_WAIVED_RUBRIC_CONTRACT_UNMET" in codes(out)
             and "MANIFEST_RUBRIC_FLOOR_INVALID" in codes(out),
             f"a waived PASS with floor<=0 was promoted (vacuous PASS): {codes(out)}")
    # W8 면제돼도 실린 인증서는 선택 증거로 그대로 검증·carrier 가 된다(DS4F 의 수동 인증서가 이미 바인딩된 경우).
    out = verify("PASS", dict(_PROMO_RUBRIC), _bench_report_with_verdict_table("PASS", "explore"),
                 certificate=_PROMO_CERTIFICATE.format(authority="explore"))
    _require(promoted(out) and (out.get("rubric") or {}).get("authority_source") == "certificate",
             f"an optional certificate on a waived PASS lost its carrier role: {codes(out)}")


# =============================================================================================
# hint 발행 — 신 CLI(`hint.py`) 전 경로 격리 E2E (2026-09-22 재작성 · plan_26092119 S4 · AC5·AC6·AC9)
# ---------------------------------------------------------------------------------------------
# (HIST · plan_26082405 §개정 R2) 옛 머리말 요지: 위 승격 게이트와 **같은 계열의 결함**이 hint_tag.py 에도 박혀
# 있었다 — "인증서가 없다 → perf_waiver 여야 한다"는 이분법. explore 런은 인증서도(PASS 아님) waiver 도(사람 서명
# 없음) 없으므로 `_require_serving_evidence` 에서 차단되고, 설령 통과해도 footer 바인딩이 죽었다. 승격 게이트만 고치면
# **다음 문에서 다시 막히는** 상태였다. 처방은 세 번째 case 추가 + 판정의 단일 소유(`no_cert_binding_source`)였다.
# 단위 자체검사는 "분기가 도달 가능한가"를 못 잡는다(2026-08-22 실증: 새로 넣은 분기가 선행 게이트와 상호배타여서
# 도달 불가였고, 끝단 음성대조가 커밋 직전에 잡았다) — 그래서 끝단을 실제 CLI 로 돌린다.
#
# ★ 2026-09-22 재작성(plan_26092119 §4.9 "격리 E2E 를 신 CLI 전 경로로 — 현행은 hint_branch·push 게이트 우회"):
#   옛 시험은 hint_tag·hint_collect 사본을 격리 레포에서 돌렸지만 **두 단계를 대역**으로 채웠다 — ① work-manifest 는
#   evidence_publisher 가 아니라 `_write_promotion_manifest` 가 손으로 지었고 ② 페이로드 커밋은 hint_branch publish 가
#   아니라 git 배관이 지었다(push 도 발행기의 게이트를 거치지 않았다). 그래서 페이로드 트리 대조·PII 스캔·push 게이트는
#   이 시험에서 한 번도 돌지 않았다. 새 시험은 **대역 0** 이다: 캠페인 셀 픽스처 → `hint.py publish`(evidence_publisher
#   실구동 · 손 JSON 0) → Agent 저작(프로그램 저작기 `hint._fx_author` · 템플릿 지시를 읽어 만든다) → `hint.py continue`
#   (승인 → 린트 → hintlib.branch 배관 커밋 → 봉인 → 이 태그 1개 로컬 검증 → push → 원격 SHA 대조 → 카탈로그 → 캠페인
#   publish 위상)를 **CLI 서브프로세스로** 밟는다. 시험 대상은 픽스처 안에 배포 배치로 놓은 CLI 사본(이 체크아웃의 바이트)
#   이다 — 스킬이 자기 저장소 밖 코드에 기대면 여기서 드러난다.
#   docker 는 PATH 앞의 가짜 실행기(읽기 전용 inspect·history · 시작하지 않는 컨테이너 create/cp/rm — 옛 이미지라 원장 · 패치 사본 부재 ·
#   2026-09-22 S2 round 3 이미지 탐침 기본)이고, 원격은 임시 디렉터리
#   **안의** bare 저장소다 — 실 저장소에는 태그·ref 가 하나도 생기지 않는다(옛 불변식 유지).
# =============================================================================================

_HINT_SCRIPTS_REL = ".claude/skills/hint-publisher/scripts"
_HINT_CLI_REL = f"{_HINT_SCRIPTS_REL}/hint.py"
# 배포 배치 사본에 **함께** 놓아야 하는 추적 파일(wave 1 요청): 어휘표는 이름 파생의 닫힌 목록이고, sweep_bench.sh 는
#   naming 자체검사가 교차 대조하는 producer 리터럴 앵커다(인증서 model 키 규칙). 빠지면 사본이 실물보다 좁아진다.
_HINT_VOCAB_REL = "hints/vocab.json"
_SWEEP_BENCH_REL = ".claude/skills/adversarial-benchmark/scripts/sweep_bench.sh"
_RENDER_REPORT_REL = ".claude/skills/adversarial-benchmark/scripts/render_report.py"
# 픽스처 셀과 기대 이름 — 이름은 **도구가 파생**한다(D8 · 발행자 입력 ✗). 아래 값은 파생 결과를 대조하는 앵커이지 입력이 아니다.
_HINT_FX_CAMPAIGN = "c1"
_HINT_FX_CELL = "c1-a"
_HINT_FX_LITE_CELL = "c1-l"
# 2026-09-29 v7(plan_26092908 §4.1): publish 는 결정론부(vllm·model·arch·q·len·kv)만 = **기본 이름**으로 스캐폴드하고, continue 가
#   저작 Agent 의 꼬리(`inputs/tail.json` · 토큰마다 뜻 + 이 셀 서빙 설정 file·key·value 근거)를 붙여 최종 이름을 확정한다.
#   H2 셀은 꼬리 1토큰(근거 = 서빙 yaml `enforce-eager: true`)으로 · lite 셀은 빈 꼬리(`[]` 명시)로 두 경로를 다 친다.
_HINT_FX_BASE_TAG = ("hint/0.9.0/fixture-model-nvfp4/gb10-1g2n-cluster-native/"
                     "qnvfp4-len4096-kvauto")
_HINT_FX_TAIL = [{"token": "eager", "meaning": "CUDA graph 대신 eager 실행 — 같은 q·len·kv 의 graph 셀과 가른다",
                  "evidence": {"file": "output/multi/configs/c1-a.yaml", "key": "enforce-eager", "value": "true"}}]
_HINT_FX_TAG = _HINT_FX_BASE_TAG + "-eager"
_HINT_FX_LITE_TAG = ("hint/0.9.0/fixture-model-nvfp4/gb10-1g2n-cluster-native/"
                     "qnvfp4-len2048-kvauto")
_HINT_FX_PUBLISH_UTC = "2026-01-02T06:00:00Z"
_HINT_FX_CONTINUE_UTC = "2026-01-02T07:00:00Z"
_HINT_FX_LITE_MEASURED_UTC = "2026-01-02T14:00:00Z"
_HINT_FX_LITE_REPORT = "docs/benchmark/bench_report_26010214_fixture-model-nvfp4_GB10_0.9.0.md"
_HINT_FX_LITE_CERT = "docs/benchmark/benchmark_26010214_fixture-model-nvfp4_GB10_0.9.0.yaml"   # 같은 측정 = 같은 stem

# 가짜 docker — 옛 in-process 대역(`hint._fx_docker`)과 같은 대답을 **실행 파일**로 낸다(CLI 서브프로세스가 PATH 로 찾는다).
#   읽기 전용 두 명령(image inspect·history)에 답하고, 원장 cat(`docker run`)은 원장 없는 옛 이미지로 rc 1 이다.
#   호출 argv 를 기록해 "docker 는 읽기와 시작하지 않는 컨테이너의 create · cp · rm 뿐" 을 단언한다.
#   2026-09-22 · plan_26092119 S2 round 3(이미지 탐침 = publish 기본): `create` 는 시작하지 않는 컨테이너 id 를 결정론으로 낸다
#   (`_hint_shim_cid(n)` — n = 이 호출까지의 create 순번 · 술어가 같은 식으로 "우리가 만든 컨테이너" 를 안다) · `cp` 는 옛 이미지라 파일
#   부재(docker 의 부재 문구 — artifacts 가 `absent` 로 읽는다 = 원장 없음 · 패치 사본 없음, 옛 원장 cat rc 1 과 같은 판정) · `rm` 은 성공.
_HINT_DOCKER_SHIM = r'''
import json, sys
from pathlib import Path
state = json.loads(Path(@STATE@).read_text(encoding="utf-8"))
argv = sys.argv[1:]
with open(@LOG@, "a", encoding="utf-8") as fh:
    fh.write(json.dumps(argv) + "\n")
if argv[:1] == ["create"]:
    if argv[-1] not in state["images"]:
        sys.stderr.write("Error response from daemon: No such image\n")
        sys.exit(1)
    with open(@LOG@, encoding="utf-8") as fh:
        n = sum(1 for ln in fh if ln.strip() and json.loads(ln)[:1] == ["create"])
    print("c0ffee" + format(n, "058x"))
    sys.exit(0)
if argv[:1] == ["cp"]:
    sys.stderr.write("Error response from daemon: Could not find the file in container\n")
    sys.exit(1)
if argv[:1] == ["rm"]:
    print(argv[-1])
    sys.exit(0)
if argv[:2] == ["image", "inspect"]:
    img = state["images"].get(argv[-1])
    if img is None:
        sys.stderr.write("Error: No such image\n")
        sys.exit(1)
    if "--format" in argv:
        val = {"{{.Id}}": img.get("Id"), "{{.Created}}": img.get("Created")}.get(argv[argv.index("--format") + 1])
        if not val:
            sys.exit(1)
        print(val)
        sys.exit(0)
    print(json.dumps([img]))
    sys.exit(0)
if argv[:1] == ["history"]:
    h = state["history"].get(argv[-1])
    if h is None:
        sys.exit(1)
    sys.stdout.write(h)
    sys.exit(0)
if argv[:1] == ["run"]:
    sys.stderr.write("cat: /opt/easy-vllm/build_ledger.json: No such file or directory\n")
    sys.exit(1)
sys.stderr.write("runtime_selftest docker shim: unexpected call\n")
sys.exit(1)
'''


def _hint_shim_cid(n: int) -> str:
    """가짜 docker 의 n 번째 create 가 내는 컨테이너 id(`_HINT_DOCKER_SHIM` 과 같은 식 · 갈리면 아래 술어가 cp · rm 을 거부해 붉어진다)."""
    return "c0ffee" + format(n, "058x")


def _hint_docker_calls_non_starting(calls: list) -> list:
    """발행기가 부를 수 있는 docker 호출(순서가 있는 기록 전체) — `image inspect` · `history` · **시작하지 않는** 컨테이너의 `create`
    (`--pull never` · `--network none` · `--entrypoint /bin/true`) · 그 컨테이너에서의 `cp` · 그 컨테이너의 `rm`. 그 밖(`run` · `start` ·
    `exec` · `build` · `image rm` …)은 전부 위반이고, 만들었는데 지우지 않은 컨테이너도 위반이다. 위반 호출 목록을 돌려준다(빈 목록 = 통과).

    2026-09-22 · plan_26092119 S2 round 3: 이미지 탐침이 publish 기본이 되어 옛 3종 계약(inspect · history · 원장 cat)을 넓혔다 — 원장도
    같은 create+cp 로 읽으므로 원장 cat(`docker run` = 컨테이너 **시작**)은 이제 위반이다(계약이 넓어진 쪽은 "시작하지 않는" 호출뿐).
    옛 술어의 교훈 유지(2026-09-22 적대 리뷰): 접두 한 칸(`image`)만 보면 `image rm` 이, 플래그 하나만 보면 임의 명령이 '읽기' 로 통과한다."""
    made: list[str] = []
    removed: set[str] = set()
    bad: list = []
    for argv in calls:
        a = [str(x) for x in argv]

        def pair(flag: str, value: str) -> bool:
            return any(a[i] == flag and a[i + 1] == value for i in range(len(a) - 1))

        if a[:2] == ["image", "inspect"] or a[:1] == ["history"]:
            continue
        if a[:1] == ["create"]:
            made.append(_hint_shim_cid(len(made) + 1))
            if not (pair("--pull", "never") and pair("--network", "none") and pair("--entrypoint", "/bin/true")):
                bad.append(a)
            continue
        if a[:1] == ["cp"]:
            srcs = [x for x in a[1:] if not x.startswith("-")]
            cid = srcs[0].split(":", 1)[0] if srcs and ":" in srcs[0] else ""
            if cid not in made or cid in removed:
                bad.append(a)
            continue
        if a[:1] == ["rm"]:
            ids = [x for x in a[1:] if not x.startswith("-")]
            if not ids or any(x not in made for x in ids):
                bad.append(a)
            removed.update(ids)
            continue
        bad.append(a)
    bad += [["<left-behind>", cid] for cid in made if cid not in removed]
    return bad


def _import_hint_cli():
    """hint.py 를 in-process 로 적재한다 — **픽스처 조립·프로그램 저작·단위 판정에만** 쓴다(시험 대상 경로는 CLI 서브프로세스).

    hint.py 는 자기 scripts/ 를 sys.path 에 넣고 hintlib(import 부수효과 0)을 적재한다. dataclass 가 모듈을 되찾을 수 있게
    sys.modules 에 먼저 등록한다(미등록이면 `from __future__ import annotations` 모듈의 dataclass 생성이 실패한다)."""
    name = "_runtime_selftest_hint_cli"
    if name in sys.modules:
        return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, CLAUDE_DIR.parent / _HINT_CLI_REL)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        sys.modules.pop(name, None)
        raise
    return module


@contextlib.contextmanager
def _hint_isolated_env(td: Path):
    """격리 git·자격증명 환경(픽스처 조립 + CLI 서브프로세스 공용). 실 사용자의 git 설정(서명·훅·자격 도우미)과 토큰,
    모델 경로 오버라이드가 픽스처로 스며들지 않는다 — 스며들면 시험 결과가 실행한 사람의 환경을 따라 바뀐다."""
    gcfg = td / "gitconfig"
    mail = "@".join(("selftest", "example.invalid"))      # 메일 모양 리터럴을 추적 배포 파일에 새로 두지 않는다(배포 4종 자기스캔)
    gcfg.write_text(f"[user]\n\tname = selftest\n\temail = {mail}\n[commit]\n\tgpgsign = false\n"
                    "[tag]\n\tgpgsign = false\n[core]\n\thooksPath = /dev/null\n", encoding="utf-8")
    keys = ("GIT_CONFIG_GLOBAL", "GIT_CONFIG_NOSYSTEM", "GIT_TERMINAL_PROMPT", "GITHUB_TOKEN", "QUANT_MODEL_PATH",
            "GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES")
    saved = {k: os.environ.get(k) for k in keys}
    for k in keys:
        os.environ.pop(k, None)
    os.environ.update(GIT_CONFIG_GLOBAL=str(gcfg), GIT_CONFIG_NOSYSTEM="1", GIT_TERMINAL_PROMPT="0")
    try:
        yield
    finally:
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v


def _hint_git(repo: Path, *args: str) -> str:
    proc = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, timeout=120)
    if proc.returncode != 0:
        raise RuntimeSelftestFailure(f"hint fixture git {args[:3]} rc={proc.returncode}: {proc.stderr.strip()[-400:]}")
    return proc.stdout.strip()


def _hint_refs(repo: Path) -> str:
    return _hint_git(repo, "for-each-ref", "--format=%(refname) %(objectname)")


def _hint_remote_ref(bare: Path, ref: str) -> str | None:
    """원격(bare) 의 ref 가 가리키는 오브젝트(annotated 태그면 **태그 오브젝트** · 피일 전)."""
    out = _hint_git(bare, "ls-remote", str(bare), ref)
    rows = [ln.split("\t") for ln in out.splitlines() if ln.strip()]
    hits = [sha for sha, name in rows if name == ref]
    return hits[0] if len(hits) == 1 else None


def _hint_cli_fixture(td: Path, hint, *, legacy: bool) -> dict:
    """발행 전 경로가 도는 격리 저장소(hint 자체검사와 **같은** 픽스처 조립기 `hint._fx_repo` 위에 짓는다 — 두 벌이면 갈라진다).

    그 위에 ① 배포 배치의 CLI 사본(hint.py · hintlib/ — 이 체크아웃의 바이트) ② 추적 어휘표·producer 앵커 ③ 가짜 docker
    실행기를 얹는다. `legacy=True` 면 원격에 과거·타 PC 태그(옛 5세그먼트 annotated · 로컬 오브젝트 없는 원격 전용 ·
    lightweight)를 먼저 심는다(AC6 — 신규 발행이 그것들에 막히지 않아야 한다 · D10)."""
    repo, fx = hint._fx_repo(td)
    real = CLAUDE_DIR.parent
    dst = repo / _HINT_SCRIPTS_REL
    shutil.copy2(real / _HINT_CLI_REL, dst / "hint.py")
    shutil.copytree(real / _HINT_SCRIPTS_REL / "hintlib", dst / "hintlib",
                    ignore=shutil.ignore_patterns("__pycache__"))
    for rel in (_HINT_VOCAB_REL, _SWEEP_BENCH_REL):
        (repo / rel).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(real / rel, repo / rel)
    old = hint._fx_legacy_remote(td, repo, fx["bare"]) if legacy else {}
    bindir = td / "bin"
    bindir.mkdir()
    state = td / "docker_state.json"
    state.write_text(json.dumps({"images": fx["docker"]["images"], "history": fx["docker"]["history"]}),
                     encoding="utf-8")
    log = td / "docker_calls.jsonl"
    shim = bindir / "docker"
    shim.write_text(f"#!{sys.executable}\n" + _HINT_DOCKER_SHIM.replace("@STATE@", repr(str(state)))
                    .replace("@LOG@", repr(str(log))), encoding="utf-8")
    shim.chmod(0o755)
    return {"repo": repo, "fx": fx, "bare": fx["bare"], "legacy": old, "bin": bindir, "docker_log": log}


def _hint_cli(f: dict, *args: str) -> subprocess.CompletedProcess:
    """픽스처 안의 CLI 사본을 서브프로세스로(전역 `--repo` 는 서브명령 앞 — 2026-09-12 인자 순서 사고)."""
    env = dict(os.environ)
    env["PATH"] = f"{f['bin']}{os.pathsep}{env.get('PATH', '')}"
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    repo = f["repo"]
    return subprocess.run([sys.executable, "-B", str(repo / _HINT_CLI_REL), "--repo", str(repo), *args],
                          cwd=str(repo), env=env, capture_output=True, text=True, timeout=300)


def _hint_cli_ok(f: dict, what: str, *args: str) -> subprocess.CompletedProcess:
    proc = _hint_cli(f, *args)
    _require(proc.returncode == 0,
             f"hint.py {what} failed: rc={proc.returncode} stdout={proc.stdout[-900:]!r} stderr={proc.stderr[-1500:]!r}")
    return proc


def _hint_single_draft(repo: Path) -> Path:
    drafts = sorted(p.parent for p in (repo / "hints/.drafts").glob("*/state.json"))
    _require(len(drafts) == 1, f"expected exactly one published draft, found {[str(d) for d in drafts]}")
    return drafts[0]


def _hint_edit_json(path: Path, fn) -> bytes:
    """JSON 파일을 고치고 **원래 바이트**를 돌려준다(음성대조 뒤 복원용)."""
    before = path.read_bytes()
    doc = json.loads(before)
    fn(doc)
    path.write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return before


# 프로그램 저작(Agent 대역) 구동기 — 픽스처의 CLI 사본 옆 `hint._fx_author` 를 부른다(저작기는 템플릿 지시를 읽어 hint-event 와
#   **치환 후 원문** 발췌를 만든다 · 지시를 여기 다시 적지 않는다).
_HINT_AUTHOR_DRIVER = (
    "import json, sys\n"
    "from pathlib import Path\n"
    "sys.path.insert(0, sys.argv[1])\n"
    "import hint\n"
    "draft = Path(sys.argv[3])\n"
    "hint._fx_author(Path(sys.argv[2]), draft, json.loads((draft / 'state.json').read_text(encoding='utf-8')))\n")


def _hint_author(f: dict, draft: Path) -> None:
    """저작 단계는 **자식 프로세스**로 돈다. 저작자는 발행 도구 밖의 행위자이고(사람·Agent), 저작기가 거치는 소유 모듈(서브 manifest
    로더 등)은 site-packages 의 YAML 을 쓴다 — 이 자체검사가 `python -S` 로 불려도(harness_verify·verify_distribution 이 자기
    플래그를 물려준다) 저작이 부모 인터프리터의 플래그에 묶이지 않게 한다(2026-09-22 첫 `-S` 실행에서 드러났다). CLI 단계와 같다."""
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    proc = subprocess.run([sys.executable, "-B", "-c", _HINT_AUTHOR_DRIVER, str(f["repo"] / _HINT_SCRIPTS_REL),
                           str(f["repo"]), str(draft)], env=env, capture_output=True, text=True, timeout=300)
    _require(proc.returncode == 0, f"programmatic authoring failed: rc={proc.returncode} {proc.stderr[-1200:]!r}")


def _hint_publish_and_author(f: dict, *extra: str) -> tuple[Path, dict]:
    """publish(스캐폴드 · 정지) → 프로그램 저작 → lint 0 까지. 반환 = (draft, state.json)."""
    _hint_cli_ok(f, "publish", "publish", "--campaign", _HINT_FX_CAMPAIGN, *extra,
                 "--generated-utc", _HINT_FX_PUBLISH_UTC)
    draft = _hint_single_draft(f["repo"])
    st = json.loads((draft / "state.json").read_text(encoding="utf-8"))
    _hint_author(f, draft)
    proc = _hint_cli(f, "lint", "--draft", str(draft), "--json")
    try:
        issues = json.loads(proc.stdout)
    except ValueError:
        issues = None
    _require(proc.returncode == 0 and issues == [],
             f"programmatic authoring left lint issues: rc={proc.returncode} {str(issues)[:900]} {proc.stderr[-400:]!r}")
    return draft, st


def _hint_tag_body(repo: Path, tag: str) -> str:
    return _hint_git(repo, "cat-file", "tag", f"refs/tags/{tag}")


def _test_hint_binding_source() -> None:
    """H0 단위 — 인증서 부재 바인딩 원천의 **단일 판정자**(`hintlib.evidence.no_cert_binding_source`·`binding_artifact_path`).

    우선순위: certificate > (perf_waiver ∨ hint_map_only ∨ explore) → bench_report > 차단(None). 2026-08-01·08-24·09-14 에
    두 결정점이 갈라져 봉인과 검증이 다른 산출물을 봤던 이력 때문에 판정자는 하나다(옛 hint_tag 에서 evidence 로 이관).
    E2E 단계(아래 CLI 시험)는 explore·인증서·lite 통로가 실제로 끝단까지 도달하는지를 친다."""
    ev = _import_hint_cli().evidence
    bench_report_rel = "../benchmark/bench_report_selftest.md"
    cert_rel = "../benchmark/benchmark_selftest.yaml"

    def manifest_shape(benchmark: dict, with_cert: bool = False, task_class: str = "full_benchmark") -> dict:
        evidence = {"bench_report": {"path": bench_report_rel}}
        if with_cert:
            evidence["certificate"] = {"path": cert_rel}
        return {"task_class": task_class, "benchmark": benchmark, "evidence": evidence}

    explore_manifest = manifest_shape(dict(_PROMO_RUBRIC))
    waiver_manifest = manifest_shape({"perf_waiver": {"authorized_by": "selftest-operator"}})
    bare_manifest = manifest_shape({"verdict": "REFUTE"})
    cert_manifest = manifest_shape(dict(_PROMO_RUBRIC), with_cert=True)
    map_only_manifest = manifest_shape({"mode": "lite", "verdict": None}, task_class="hint_map_only")
    map_only_cert_manifest = manifest_shape({"mode": "lite", "verdict": None}, with_cert=True, task_class="hint_map_only")
    _require(ev.no_cert_binding_source(explore_manifest) == "explore",
             "explore rubric authority did not open the no-certificate binding source")
    _require(ev.no_cert_binding_source(waiver_manifest) == "perf_waiver",
             "perf_waiver stopped opening the no-certificate binding source")
    _require(ev.no_cert_binding_source(map_only_manifest) == "hint_map_only",
             "hint_map_only (lite-only cell) did not open the no-certificate binding source")
    _require(ev.no_cert_binding_source(bare_manifest) is None,
             "a manifest with neither waiver nor explore nor map_only opened a binding source")
    for field, value in (("floor_tps", 0), ("rubric_source", None), ("primary_source", "N/A")):
        broken = manifest_shape(dict(_PROMO_RUBRIC, **{field: value}))
        _require(ev.no_cert_binding_source(broken) is None,
                 f"explore contract violation {field}={value!r} still opened the binding source")
    _require(ev.binding_artifact_path(cert_manifest) == cert_rel,
             "certificate lost priority over the manifest-declared explore fallback")
    _require(ev.binding_artifact_path(map_only_cert_manifest) == cert_rel,
             "a bound certificate lost priority over the hint_map_only fallback")
    for label, man in (("explore", explore_manifest), ("perf_waiver", waiver_manifest),
                       ("hint_map_only", map_only_manifest)):
        _require(ev.binding_artifact_path(man) == bench_report_rel,
                 f"{label} path did not bind the bench_report artifact")
    _require(ev.binding_artifact_path(bare_manifest) is None,
             "a manifest with no certificate/waiver/explore/map_only still produced a binding artifact")


def _test_hint_cli_publication() -> None:
    """H2(인증서 PASS) 전 경로 + AC6 + 음성대조 — `hint.py publish → 저작 → continue → 원격 SHA → 카탈로그` 를 CLI 로.

    v7(plan_26092908): publish = 기본 이름 · 꼬리 저작(tail.json) → continue 가 최종 이름 확정 · 페이로드 커밋 부모 = 원격 안내 커밋 ·
    로컬 hint 브랜치 불이동 · footer v2(bench_ref · bench_kind) · 카탈로그 판정 열 · 카탈로그 자동 커밋(두 경로만).
    ★ 음성대조(전부 부수효과 0 을 함께 단언): 승인 부재(HINT_APPROVAL_ABSENT · C6 의 정적 순서 검사를 행동으로 보완) ·
    미저작(HINT_LINT_FAILED · 커밋 전) · 소스 트리 앵커 태그(HINT_ANCHOR_PARENT_NOT_GUIDE · verify FAIL · push 거부 ·
    2026-09-07 native 3종) · PERF-WARNING 없는 waiver 본문(HINT_PERF_WARNING_MISSING) · 관측 없는 셀(HINT_QUALIFICATION_UNOBSERVED ·
    옛 H4 "근거 검사는 면제되지 않는다"의 후계) · 측정 뒤 바뀐 트리플렛(HINT_TRIPLET_DRIFT · 옛 레시피-lockset 대조의 후계)."""
    hint = _import_hint_cli()
    with tempfile.TemporaryDirectory(prefix="hint-cli-e2e.") as tds:
        td = Path(tds).resolve()
        with _hint_isolated_env(td):
            f = _hint_cli_fixture(td, hint, legacy=True)
            repo, bare = f["repo"], f["bare"]
            refs0 = _hint_refs(repo)

            # ── publish(스캐폴드 · 정지) ──
            _hint_cli_ok(f, "publish", "publish", "--campaign", _HINT_FX_CAMPAIGN, "--cell", _HINT_FX_CELL,
                         "--generated-utc", _HINT_FX_PUBLISH_UTC)
            draft = _hint_single_draft(repo)
            st = json.loads((draft / "state.json").read_text(encoding="utf-8"))
            _require(st.get("tag") == _HINT_FX_BASE_TAG and st.get("stage") == "scaffolded",
                     f"publish did not scaffold under the v7 base name (q·len·kv, no tail): {st.get('tag')!r} {st.get('stage')!r}")
            _require(_hint_refs(repo) == refs0, "publish moved a ref (publish makes no tag or branch)")
            calls = [json.loads(ln) for ln in f["docker_log"].read_text(encoding="utf-8").splitlines() if ln.strip()]
            ledger = hint.artifacts.LEDGER_IN_IMAGE
            c1 = _hint_shim_cid(1)
            probe_ok = [["create", "--pull", "never", "--network", "none", "--entrypoint", "/bin/true", "fixture"],
                        ["cp", "-L", f"{c1}:{ledger}", "/tmp/x"], ["rm", c1]]
            vacuous = [
                [["image", "rm", "fixture:0.9.0-source"]],
                [["run", "--rm", "--network", "none", "fixture", "sh"]],
                [["run", "--rm", "--network", "none", "--pull", "never", "--entrypoint", "cat", "fixture", ledger]],
                [["create", "--network", "none", "--entrypoint", "/bin/true", "fixture"], ["rm", c1]],
                [["create", "--pull", "never", "--network", "none", "--entrypoint", "/bin/true", "fixture"], ["start", c1], ["rm", c1]],
                [["cp", "-L", f"{_hint_shim_cid(9)}:{ledger}", "/tmp/x"]],
                probe_ok[:2],
                [*probe_ok, ["cp", "-L", f"{c1}:{ledger}", "/tmp/x"]]]
            _require(_hint_docker_calls_non_starting(probe_ok) == []
                     and all(_hint_docker_calls_non_starting(v) for v in vacuous),
                     "the non-starting docker predicate admits a starting/mutating call, an unpulled-never create, a cp from a "
                     "container it did not create, a container left behind, or rejects the plain probe sequence (vacuous check)")
            bad_calls = _hint_docker_calls_non_starting(calls)
            _require(calls and not bad_calls, f"publish made a docker call outside the non-starting probe contract: {bad_calls[:6]}")
            # 이미지 탐침은 publish 기본이다(2026-09-22 S2 round 3) — 플래그 없는 CLI publish 가 실제로 탐침했는가(옵트인으로 되돌아가면 붉어진다)
            _require(any(c[:1] == ["create"] for c in calls) and any(c[:1] == ["cp"] for c in calls),
                     f"flagless publish did not probe the measured image (create+cp absent): {calls[:8]}")

            # ── ★승인 부재 = 어느 ref 도 움직이지 않는다(O6 · 제안 전용) ──
            decl_p = repo / f"campaigns/{_HINT_FX_CAMPAIGN}/campaign.yaml"
            decl0 = _hint_edit_json(decl_p, lambda d: d.update(hint_targets=[]))
            proc = _hint_cli(f, "continue", "--draft", str(draft), "--generated-utc", _HINT_FX_CONTINUE_UTC)
            _require(proc.returncode != 0 and "HINT_APPROVAL_ABSENT" in proc.stderr and _hint_refs(repo) == refs0,
                     f"continue without a recorded approval was not refused before any ref moved: "
                     f"rc={proc.returncode} {proc.stderr[-600:]!r}")
            decl_p.write_bytes(decl0)

            # ── ★미저작 = 린트 차단(커밋 전 · hint 브랜치 없음) ──
            proc = _hint_cli(f, "continue", "--draft", str(draft), "--generated-utc", _HINT_FX_CONTINUE_UTC)
            _require(proc.returncode != 0 and "HINT_LINT_FAILED" in proc.stderr and _hint_refs(repo) == refs0,
                     f"an unauthored draft was not refused before the payload commit: rc={proc.returncode} "
                     f"{proc.stderr[-600:]!r}")

            # ── 저작(프로그램) → 이름 꼬리(tail.json · 근거 = 이 셀 서빙 yaml) → lint 0 → continue ──
            _hint_author(f, draft)
            tail_p = draft / "inputs/tail.json"
            tail_p.write_text(json.dumps([{**_HINT_FX_TAIL[0], "evidence": {**_HINT_FX_TAIL[0]["evidence"], "value": "false"}}],
                                         ensure_ascii=False), encoding="utf-8")
            proc = _hint_cli(f, "lint", "--draft", str(draft), "--json")
            _require(proc.returncode != 0 and "HINT_TAIL_UNGROUNDED" in proc.stdout,
                     f"a tail whose evidence disagrees with the serving config passed lint: {proc.stdout[-600:]!r}")
            tail_p.write_text(json.dumps(_HINT_FX_TAIL, ensure_ascii=False), encoding="utf-8")
            proc = _hint_cli(f, "lint", "--draft", str(draft), "--json")
            _require(proc.returncode == 0 and proc.stdout.strip() == "[]",
                     f"programmatic authoring left lint issues: {proc.stdout[-900:]!r}")

            # ── ★승격 게이트 거부 = 페이로드 커밋 **전에** 멈춘다(부수효과 전 게이트 · plan_26092119 SPEC §4.2 ③) ──
            #   승인·린트를 다 통과한 draft 라도 게이트가 거부하면 hint 브랜치 커밋·태그가 하나도 생기지 않아야 한다 — 봉인 앞의
            #   재확인(두 번째 게이트)만 남으면 거부가 **커밋 뒤**에 와서 태그 없는 페이로드 커밋이 hint 브랜치(되감지 않는다)에
            #   남는다. publish 의 사전 확인(아래 H5)과는 다른 자리다(2026-09-22 적대 리뷰: 이 자리를 걷어낸 변이가 두 자체검사를
            #   모두 통과했다). 게이트 JSON 이 stdout 에 그대로(action = hint_finalize · 소비자는 json.loads 한다).
            man_p = repo / st["manifest"]

            def gate_denied(proc: subprocess.CompletedProcess, action: str) -> bool:
                try:
                    g = json.loads(proc.stdout)
                except ValueError:
                    return False
                return (proc.returncode != 0 and isinstance(g, dict) and g.get("allowed") is False
                        and g.get("action") == action and proc.returncode == g.get("exit_code")
                        and "RUNTIME_FUNCTIONAL_SMOKE_NOT_PASSED" in (g.get("reason_codes") or []))

            def ineligible(d: dict) -> None:
                d["runtime"]["functional_smoke_passed"] = False

            man0 = _hint_edit_json(man_p, ineligible)
            proc = _hint_cli(f, "continue", "--draft", str(draft), "--generated-utc", _HINT_FX_CONTINUE_UTC,
                             "--remote", "origin")
            man_p.write_bytes(man0)
            _require(gate_denied(proc, "hint_finalize") and _hint_refs(repo) == refs0,
                     f"a gate denial at continue did not stop before the payload commit (or lost its gate JSON): "
                     f"rc={proc.returncode} stdout={proc.stdout[-500:]!r} stderr={proc.stderr[-400:]!r}")

            _hint_cli_ok(f, "continue", "continue", "--draft", str(draft), "--generated-utc", _HINT_FX_CONTINUE_UTC,
                         "--remote", "origin")

            tag_ref = f"refs/tags/{_HINT_FX_TAG}"
            local_obj = _hint_git(repo, "rev-parse", tag_ref)
            _require(_hint_remote_ref(bare, tag_ref) == local_obj,
                     "remote tag object differs from the locally sealed tag object (AC9)")
            _require(_hint_remote_ref(bare, "refs/heads/hint") == f["fx"]["guide"],
                     "publish moved the remote hint branch -- the publication push is the exact tag refspec only (O2 · "
                     "the branch = the guide commit, moved only by an approved branch-transition)")
            _require(json.loads((draft / "state.json").read_text(encoding="utf-8")).get("tag") == _HINT_FX_TAG,
                     "continue did not confirm the final v7 name (base + authored tail)")
            # 페이로드 커밋 = 태그가 가리키는 커밋 · 부모 = 원격 안내 커밋(plan_26092908 §4.7) · 로컬 hint 브랜치는 생기지도 움직이지도 않는다
            tip = _hint_git(repo, "rev-parse", f"{tag_ref}^{{commit}}")
            _require(_hint_git(repo, "rev-parse", f"{tip}^@").split() == [f["fx"]["guide"]]
                     and _hint_remote_ref(bare, "refs/heads/hint") == f["fx"]["guide"]
                     and "refs/heads/hint" not in _hint_refs(repo),
                     "the payload commit's parent is not the remote guide commit, or the publication moved a hint branch")
            body = _hint_tag_body(repo, _HINT_FX_TAG)
            _require(f"anchor: {tip}" in body and "bench_ref: ../benchmark/benchmark_" in body
                     and "bench_kind: certificate" in body and "certificate_ref:" not in body,
                     f"tag footer v2 does not bind the payload commit + certificate: {body[-600:]!r}")
            _require(_hint_git(repo, "log", "-1", "--format=%an <%ae>|%cn <%ce>", tip)
                     == f"{hint.branch.synthetic_identity()}|{hint.branch.synthetic_identity()}",
                     "payload commit is not authored by the synthetic identity (PII · branch.commit_payload)")
            tree = _hint_git(repo, "ls-tree", "--name-only", tip).splitlines()
            _require({"README.md", "00-hint.md", "01-artifacts.md", "02-narrative.md", "03-benchmark.md",
                      "PAYLOAD.json", "LINEAGE.json", "PROVENANCE.json"} <= set(tree) and ".claude" not in tree,
                     f"payload tree is not the v6 allowlist: {tree}")
            idx = json.loads((repo / "hints/index.json").read_text(encoding="utf-8"))
            rows = {h.get("tag"): h for h in idx.get("hints", []) if isinstance(h, dict)}
            _require((rows.get(_HINT_FX_TAG) or {}).get("grammar") == "v7"
                     and (rows.get(_HINT_FX_TAG) or {}).get("verdict") == "PASS",
                     f"catalog did not derive a v7 row with the PASS verdict column for the new tag: {rows.get(_HINT_FX_TAG)!r}")
            # 카탈로그 자동 커밋(plan_26092908 §4.8): 두 경로만 · 도구 메시지 · Co-Authored-By 없음
            _require(_hint_git(repo, "log", "-1", "--format=%s") == f"chore(hint): 카탈로그 파생 — {_HINT_FX_TAG} 발행 반영"
                     and sorted(_hint_git(repo, "show", "--name-only", "--format=", "HEAD").split())
                     == sorted(["HINTS.md", "hints/index.json"]),
                     "continue did not auto-commit exactly the two catalog paths with the tool message")
            legacy = f["legacy"]
            _require(all(legacy[k] in rows for k in ("old", "remote_only", "lightweight")),
                     f"AC6: legacy/foreign tags did not coexist as catalog rows: {sorted(rows)}")
            _require(rows[legacy["remote_only"]].get("object") == hint.catalog.OBJECT_ABSENT_LOCAL,
                     f"AC6: a remote-only tag must be recorded as absent-local (no synthesis): {rows[legacy['remote_only']]!r}")
            phase = json.loads((repo / f"campaigns/{_HINT_FX_CAMPAIGN}/phases/main/publish.status.json")
                               .read_text(encoding="utf-8"))
            _require(phase.get("state") == "done"
                     and (phase.get("proof") or {}).get("source") == f"{tag_ref}@{local_obj}",
                     f"campaign publish phase was not recorded with the remote SHA proof: {phase!r}")
            proc = _hint_cli_ok(f, "verify", "verify", "--tag", _HINT_FX_TAG)
            _require("PASS" in proc.stdout, f"verify did not report PASS: {proc.stdout[-400:]!r}")

            # ── ★단독 verify·push 도 게이트를 먼저 묻는다(HINT_ACTION_FOR_CMD 의 나머지 두 액션 · 거부 = 게이트 JSON · push 0) ──
            #   footer 의 manifest_ref 로 묻는다. 게이트가 거부하면 verify 는 판정하지 않고, push 는 dry-run 조차 하지 않는다.
            remote_obj0 = _hint_remote_ref(bare, tag_ref)
            man0 = _hint_edit_json(man_p, ineligible)
            v_proc = _hint_cli(f, "verify", "--tag", _HINT_FX_TAG)
            p_proc = _hint_cli(f, "push", "--tag", _HINT_FX_TAG, "--remote", "origin", "--apply")
            man_p.write_bytes(man0)
            _require(gate_denied(v_proc, "hint_verify"),
                     f"standalone verify ran without asking the gate (hint_verify): rc={v_proc.returncode} "
                     f"stdout={v_proc.stdout[-400:]!r}")
            _require(gate_denied(p_proc, "hint_push") and _hint_remote_ref(bare, tag_ref) == remote_obj0
                     and '"refspec"' not in p_proc.stdout,
                     f"standalone push ran without asking the gate (hint_push): rc={p_proc.returncode} "
                     f"stdout={p_proc.stdout[-400:]!r}")

            # ── ★소스 트리 커밋에 봉인한 태그(2026-09-07 native 3종) — verify FAIL · push 거부 · 원격 불변 ──
            forged = _HINT_FX_TAG + "-forged"
            head = _hint_git(repo, "rev-parse", "HEAD")
            bound = hint.tag.bench_binding(hint.tag.parse_annotation(hint.tag.read_tag(repo, _HINT_FX_TAG)["body"])["footer"])
            fields = {"version": hint.tag.FOOTER_VERSION, "tag": forged, "topology": st["topology_label"],
                      "anchor": head, "manifest_ref": st["manifest"], "bench_ref": bound["ref"], "bench_kind": bound["kind"]}
            message = hint.tag.annotation(hint.template.brief(draft / "payload"), fields)
            # 봉인과 같은 합성 tagger·주입 시각·정상 footer 로 짓는다 — 태그 오브젝트 자체는 흠이 없고 결함은 **앵커**(와 그 귀결:
            #   소스 커밋의 트리·신원)다. 판정은 앵커 사유코드 자체를 요구한다(다른 사유로 FAIL 해도 초록이 되지 않게 ·
            #   2026-09-22 변이 검사: 앵커 검사를 걷어낸 사본은 다른 사유로만 FAIL 하고 이 대조가 그것을 잡는다).
            tagger = {**os.environ, "GIT_COMMITTER_NAME": hint.core.SYNTHETIC_NAME,
                      "GIT_COMMITTER_EMAIL": hint.core.SYNTHETIC_EMAIL,
                      "GIT_COMMITTER_DATE": hint.core.git_date(_HINT_FX_CONTINUE_UTC)}
            proc = subprocess.run(["git", "-C", str(repo), "tag", "-a", "-F", "-", "--cleanup=verbatim", forged, head],
                                  input=message, capture_output=True, text=True, timeout=60, env=tagger)
            _require(proc.returncode == 0, f"fixture could not create the source-anchored tag: {proc.stderr[-300:]!r}")
            proc = _hint_cli(f, "verify", "--tag", forged)
            _require(proc.returncode != 0 and "HINT_ANCHOR_PARENT_NOT_GUIDE" in proc.stdout,
                     f"a tag sealed on a source-tree commit passed verify: rc={proc.returncode} {proc.stdout[-600:]!r}")
            proc = _hint_cli(f, "push", "--tag", forged, "--remote", "origin", "--apply")
            # 거부 사유도 본다: push 앞 로컬 검증(HINT_PUSH_UNVERIFIED)이 앵커 사유로 막았는가 — rc≠0 만 보면 무관한 실패(적재 오류 등)도
            #   "밀지 않았다" 로 초록이 된다(2026-09-22 적대 리뷰).
            _require(proc.returncode != 0 and _hint_remote_ref(bare, f"refs/tags/{forged}") is None
                     and "HINT_PUSH_UNVERIFIED" in proc.stderr and "HINT_ANCHOR_PARENT_NOT_GUIDE" in proc.stderr,
                     f"a source-anchored tag was pushed (or refused for an unrelated reason): rc={proc.returncode} "
                     f"{proc.stderr[-600:]!r}")

            # ── ★waiver 가 있는데 본문에 PERF-WARNING 이 없다 = 린트 차단(옛 hint_tag._require_perf_warning 의 이관) ──
            #   린트는 draft 의 work-manifest(state.manifest)를 읽는다 — 그 manifest 에 사람 waiver 가 **있다고** 두고 CLI lint 를
            #   다시 돈다(쌍: 원 manifest 로는 0건). 본문은 facts 에서 기계 렌더되므로 manifest 만 바뀐 상태는 경고 없는 waiver 다.
            def lint_codes_now() -> set:
                out = _hint_cli(f, "lint", "--draft", str(draft), "--json")
                try:
                    return {i["code"] for i in json.loads(out.stdout)}
                except (ValueError, TypeError, KeyError):
                    raise RuntimeSelftestFailure(f"hint.py lint --json produced no JSON: {out.stderr[-500:]!r}")

            _require(lint_codes_now() == set(), "sealed draft no longer lints clean against its own manifest")
            man0 = _hint_edit_json(man_p, lambda d: d.setdefault("benchmark", {}).update(perf_waiver={
                "authorized_by": "selftest-operator", "authorized_at_utc": "2026-09-07T00:00:00Z",
                "instruction": "loop-until-done 중단", "warning_flag": "PERF-WARNING: selftest fixture"}))
            codes = lint_codes_now()
            man_p.write_bytes(man0)
            _require(hint.template.PERF_WARNING_MARKER == "PERF-WARNING" and "HINT_PERF_WARNING_MISSING" in codes,
                     f"a perf_waiver without the PERF-WARNING body marker was accepted: {sorted(codes)}")

            # ── ★관측 없는 셀 = publish 가 첫 쓰기 전에 막는다(발행 자격 = 관측 · X8) ──
            ph = repo / f"output/multi/benchlog/sweep_{_HINT_FX_CELL}/level_01/post_health_{_HINT_FX_CELL}.json"
            ph_bytes = ph.read_bytes()
            ph.unlink()
            tree0, refs1 = hint._fx_tree(repo), _hint_refs(repo)
            proc = _hint_cli(f, "publish", "--campaign", _HINT_FX_CAMPAIGN, "--cell", _HINT_FX_CELL,
                             "--generated-utc", "2026-01-02T10:00:00Z", "--out", str(td / "unobserved"))
            _require(proc.returncode != 0 and "HINT_QUALIFICATION_UNOBSERVED" in proc.stderr
                     and hint._fx_tree(repo) == tree0 and _hint_refs(repo) == refs1
                     and not (td / "unobserved").exists(),
                     f"a cell without observed health+inference was not refused before any write: "
                     f"rc={proc.returncode} {proc.stderr[-500:]!r}")
            ph.write_bytes(ph_bytes)

            # ── ★측정 뒤 바뀐 트리플렛 = 이름을 짓지 않는다(이름은 측정한 것에서 짓는다 · 읽기 전용 `name`) ──
            yaml_p = repo / f"output/multi/configs/{_HINT_FX_CELL}.yaml"
            y0 = yaml_p.read_bytes()
            yaml_p.write_text(y0.decode("utf-8").replace("max-model-len: 4096", "max-model-len: 8192"), encoding="utf-8")
            proc = _hint_cli(f, "name", "--campaign", _HINT_FX_CAMPAIGN, "--cell", _HINT_FX_CELL)
            _require(proc.returncode != 0 and "HINT_TRIPLET_DRIFT" in proc.stderr,
                     f"a serving triplet changed after measurement still yielded a name: rc={proc.returncode} "
                     f"{proc.stdout[-200:]!r} {proc.stderr[-400:]!r}")
            yaml_p.write_bytes(y0)
            proc = _hint_cli_ok(f, "name", "name", "--campaign", _HINT_FX_CAMPAIGN, "--cell", _HINT_FX_CELL)
            _require(proc.stdout.split()[:1] == [_HINT_FX_BASE_TAG],
                     f"read-only `name` disagrees with the published base name: {proc.stdout[:200]!r}")


def _hint_fx_refute(repo: Path, authority: str) -> None:
    """REFUTE 런 모양 — 인증서는 PASS 때만 발행되므로 셀 포인터에 없다 · 판정기 산출(verdict.json)은 REFUTE + 루브릭 권한."""
    sweep = repo / f"output/multi/benchlog/sweep_{_HINT_FX_CELL}"
    (sweep / "verdict.json").write_text(json.dumps({"verdict": "REFUTE", "rubric": {
        "authority": authority, "floor": 8.5, "ratio_M_over_primary": 0.9,
        "source": "expected_achievable(roofline×MBU)"}}), encoding="utf-8")
    _hint_edit_json(repo / f"campaigns/{_HINT_FX_CAMPAIGN}/evidence_pointers.json",
                    lambda d: d.update(pointers=[p for p in d["pointers"] if p.get("kind") != "certificate"]))


def _test_hint_cli_refute_paths() -> None:
    """H1 · H5 — 인증서가 구조적으로 없는 REFUTE 런이 신 CLI 에서 어디로 가는가.

    H1 ★ explore(사람 HITL 탐색 · waiver 없음)는 publish→continue→push 까지 **완주**하고 footer 는 bench_report 에 묶인다
       (2026-08-24 죽은 코드의 hint 평면 쌍둥이 — 승격 게이트만 열고 봉인에서 다시 죽던 형태가 돌아오지 않았는가).
    H5 ★ weak 권한 REFUTE(waiver ✗ · explore ✗)는 publish 의 게이트 사전 확인에서 멈춘다 — 게이트 JSON 이 stdout 그대로,
       태그·브랜치·draft 상태 부수효과 0(가드를 끈 것이 아니라 통로를 이었다는 증거는 H1 과 이 H5 의 짝이다).
    `--node` 를 **주지 않는다**(2026-09-29 · plan_26092908 §4.8 V11③ · AC7): 인증서가 없는 런도 셀 측정 TP(2) > 노드당 GPU(1) 로
    evidence 가 노드 축을 `cluster` 로 파생한다. 종전(2026-09-22)에는 측정 노드를 실어 줄 문서가 없어 배정(main)으로 해소됐고
    cluster 로 등록된 리포트 포인터가 걸러져 스윕이 조인되지 않았다 — 그래서 사람이 `--node cluster` 를 명시했다(멀티 셀 3/3)."""
    hint = _import_hint_cli()
    with tempfile.TemporaryDirectory(prefix="hint-cli-refute-weak.") as tds:
        td = Path(tds).resolve()
        with _hint_isolated_env(td):
            f = _hint_cli_fixture(td, hint, legacy=False)
            repo = f["repo"]
            _hint_fx_refute(repo, "weak")
            refs0 = _hint_refs(repo)
            proc = _hint_cli(f, "publish", "--campaign", _HINT_FX_CAMPAIGN, "--cell", _HINT_FX_CELL,
                             "--generated-utc", _HINT_FX_PUBLISH_UTC)
            try:
                gate = json.loads(proc.stdout)
            except ValueError:
                gate = {}
            _require(proc.returncode != 0 and gate.get("allowed") is False
                     and "PROMOTION_GATE_NOT_ELIGIBLE" in (gate.get("reason_codes") or [])
                     and gate.get("action") == "hint_finalize",
                     f"a weak-authority REFUTE run without a waiver passed the publish gate pre-check: "
                     f"rc={proc.returncode} stdout={proc.stdout[-600:]!r} stderr={proc.stderr[-400:]!r}")
            _require(_hint_refs(repo) == refs0 and not list((repo / "hints/.drafts").glob("*/state.json")),
                     "a gate denial at publish left a tag/branch or a scaffolded draft behind")

    with tempfile.TemporaryDirectory(prefix="hint-cli-refute-explore.") as tds:
        td = Path(tds).resolve()
        with _hint_isolated_env(td):
            f = _hint_cli_fixture(td, hint, legacy=False)
            repo = f["repo"]
            _hint_fx_refute(repo, "explore")
            draft, st = _hint_publish_and_author(f, "--cell", _HINT_FX_CELL)
            man = json.loads((repo / st["manifest"]).read_text(encoding="utf-8"))
            _require((man.get("benchmark") or {}).get("verdict") == "REFUTE"
                     and (man.get("benchmark") or {}).get("rubric_authority") == "explore"
                     and not ((man.get("evidence") or {}).get("certificate") or {}).get("path"),
                     f"fixture did not produce an explore REFUTE manifest without a certificate: {man.get('benchmark')!r} "
                     f"{(man.get('evidence') or {}).get('certificate')!r}")
            _hint_cli_ok(f, "continue(explore)", "continue", "--draft", str(draft),
                         "--generated-utc", _HINT_FX_CONTINUE_UTC, "--remote", "origin")
            body = _hint_tag_body(repo, st["tag"])
            _require("bench_ref: ../benchmark/bench_report_" in body and "bench_kind: bench_report" in body,
                     f"explore footer v2 did not bind the bench_report (bench_kind=bench_report): {body[-500:]!r}")
            # 판정의 기계 표면(plan_26092908 §4.4 · V1): 인증서가 없어도 PAYLOAD.measurement.verdict = REFUTE
            payload = json.loads(_hint_git(repo, "show", f"refs/tags/{st['tag']}^{{commit}}:PAYLOAD.json"))
            _require((payload.get("measurement") or {}).get("verdict") == "REFUTE",
                     f"a REFUTE cell did not carry its verdict on the machine surface: {payload.get('measurement')!r}")
            _require(_hint_remote_ref(f["bare"], f"refs/tags/{st['tag']}")
                     == _hint_git(repo, "rev-parse", f"refs/tags/{st['tag']}"),
                     "explore-authority tag did not reach the remote with the sealed object")


def _render_lite_report(raw_json: Path) -> str:
    """**배포되는 렌더러**(`render_report.py --lite-only`)가 lite raw 로 낸 경량 리포트 본문.

    픽스처 문자열을 쓰지 않는 이유(2026-09-14): lite 통로의 합격 기준은 "경량 리포트**만으로** 발행이 닫힌다" 이고, 그 리포트는
    렌더러가 만든다 — 손으로 쓴 리포트로 초록이면 렌더러와 발행기 사이의 계약(측정 구성 표 · lite 지표 표 · `mode: lite` ·
    `생성일` 조인 키 · 측정 환경 표)이 갈라져도 모른다(픽스처가 실물보다 좁다). 값은 합성이며 GPU·docker·서빙에 닿지 않는다."""
    env = {k: v for k, v in os.environ.items() if k != "EASY_VLLM_VERSION"}
    proc = subprocess.run([sys.executable, "-B", str(CLAUDE_DIR.parent / _RENDER_REPORT_REL), "--lite-only",
                           "--lite-raw-json", str(raw_json), "--stdout"],
                          capture_output=True, text=True, timeout=120, env=env)
    _require(proc.returncode == 0 and proc.stdout.strip(),
             f"render_report --lite-only failed on a synthetic lite raw: rc={proc.returncode} "
             f"stderr={proc.stderr[-600:]!r}")
    return proc.stdout


def _hint_fx_lite_cell(repo: Path, fx: dict, *, with_report_pointer: bool) -> None:
    """lite 만 잰 셀(c1-l · TP=2 → cluster 축) — 트리플렛 · lite raw/warm/cold(렌더러 입력이자 발행 자격의 조인 원천) ·
    렌더러가 쓴 경량 리포트 · 배정·사전승인 · 셀 파일 · 포인터. 계보 문서는 c1-a 의 것을 공유한다(같은 캠페인의 서사)."""
    cell = _HINT_FX_LITE_CELL
    out = repo / "output/multi"
    (out / f"configs/{cell}.yaml").write_text(
        "# lite 셀\nmodel: /app/quant_models/Org/Fixture-Model-NVFP4\ntensor-parallel-size: 2\nmax-model-len: 2048\n"
        "kv-cache-dtype: auto\nenforce-eager: true\n", encoding="utf-8")
    (out / f"configs/{cell}.sh").write_text('#!/bin/bash\nvllm serve --config "/app/configs/${CONFIG_FILE}.yaml"\n',
                                           encoding="utf-8")
    # 소스빌드 좌표(VLLM_REPO·VLLM_REF)는 셀 env 에 둔다 — 스윕 meta(측정 digest)가 없는 lite 셀은 이미지 history 로 빌드 입력을
    #   관측할 자리가 없고, 릴리스 모양 ref 가 업스트림 태그인지는 VLLM_REPO 가 판정한다(D-h · 없으면 이름을 짓지 않는다).
    (out / f"envs/.env.{cell}").write_text(f"CONFIG_FILE={cell}\nIMAGE_TAG=fixture:0.9.0-source\n"
                                           "BUILD_DOCKERFILE=Dockerfile.source-build\n"
                                           "VLLM_REPO=https://github.com/vllm-project/vllm.git\nVLLM_REF=v0.9.0\n"
                                           "VLLM_PLE_MMAP=1\n", encoding="utf-8")
    bl = out / "benchlog"
    warm, cold, elog = bl / f"lite_warm_{cell}.json", bl / f"lite_cold_{cell}.json", bl / f"lite_engine_{cell}.log"
    warm.write_text(json.dumps({"completed": 3, "failed": 0, "median_tpot_ms": 81.3, "output_throughput": 12.3}),
                    encoding="utf-8")
    cold.write_text(json.dumps({"completed": 1, "failed": 0, "median_ttft_ms": 140.0}), encoding="utf-8")
    # 엔진 자기보고 한 줄 — 렌더러의 vllm_version 은 env > IMAGE_TAG(`easy-vllm:X.Y.Z` 만) > **엔진 로그 자기보고** 순이다.
    #   픽스처 이미지 태그는 `easy-vllm:` 모양이 아니므로 측정값(엔진 로그)이 이긴다 — 실물 lite 셀과 같은 출처다.
    elog.write_text("INFO Starting vLLM API server v0.9.0\nINFO GPU KV cache size: 54,192 tokens\n"
                    "INFO Available KV cache memory: 8.5 GiB\n", encoding="utf-8")
    raw = bl / f"lite_raw_{cell}.json"
    raw.write_text(json.dumps({
        "topology": "multi", "burst_n": 3, "config_name": cell, "measured_utc": _HINT_FX_LITE_MEASURED_UTC,
        "backend": "openai", "endpoint": "/v1/completions", "config_yaml": str(out / f"configs/{cell}.yaml"),
        "env_file": str(out / f"envs/.env.{cell}"), "manifest": str(out / "manifest.yaml"),
        "bench_warm_json": str(warm), "bench_cold_json": str(cold), "engine_log": str(elog),
        # lite 판정(2026-09-29 plan_26092923) — 이 셀은 lite 를 통과했다(판정 입력 수집은 lite_bench 소관 · 여기선 결과만)
        "lite_verdict": "pass", "lite_exit": 0, "lite_verdict_source": "selftest fixture(lite_bench 판정 결과 모양)",
        "nodes": [{"role": "main", "gpu_smi_used_mib": None, "gpu_smi_total_mib": None,
                   "ram_total_kib": 128000000, "ram_avail_kib": 64000000}]}), encoding="utf-8")
    text = _render_lite_report(raw)
    _require("mode: lite" in text and "| bench_mode | lite |" in text
             and f"생성일 {_HINT_FX_LITE_MEASURED_UTC}" in text,
             f"real lite report lost its mode header / measurement-config row / join key: {text[:700]!r}")
    (repo / _HINT_FX_LITE_REPORT).write_text(text, encoding="utf-8")
    # lite 등급 인증서 — **배포되는 발행기**(publish_benchmark_record.py --lite-raw-json)가 같은 raw 로 쓴다(손 인증서 ✗).
    #   hint 자격 = lite 통과(2026-09-29 plan_26092923 · 게이트 HINT_MAP_REQUIRES_LITE_PASS_CERTIFICATE).
    env = {k: v for k, v in os.environ.items() if k != "EASY_VLLM_VERSION"}
    proc = subprocess.run([sys.executable, "-B", str(CLAUDE_DIR.parent / _RENDER_REPORT_REL).replace(
                               "render_report.py", "publish_benchmark_record.py"),
                           "--lite-raw-json", str(raw), "--stdout"], capture_output=True, text=True, timeout=120, env=env)
    _require(proc.returncode == 0 and "benchmark_mode: lite" in proc.stdout,
             f"publish_benchmark_record --lite-raw-json failed on the lite fixture: rc={proc.returncode} "
             f"stderr={proc.stderr[-600:]!r}")
    (repo / _HINT_FX_LITE_CERT).write_text(proc.stdout, encoding="utf-8")

    def decl(d: dict) -> None:
        d["assignments"]["main"].append({"cell": cell, "mode": "AUTO"})
        d["hint_targets"][0]["cells"].append(cell)

    camp = repo / f"campaigns/{_HINT_FX_CAMPAIGN}"
    _hint_edit_json(camp / "campaign.yaml", decl)
    (camp / f"cells/{cell}").mkdir(parents=True, exist_ok=True)
    (camp / f"cells/{cell}/cell.status.json").write_text(json.dumps({"cell_id": cell, "cell_outcome": "measured"}),
                                                          encoding="utf-8")
    (camp / f"cells/{cell}/lockset.json").write_text(json.dumps({"id": cell, "provenance": "hand-authored"}),
                                                      encoding="utf-8")
    shutil.copy2(camp / f"cells/{_HINT_FX_CELL}/config.yaml", camp / f"cells/{cell}/config.yaml")
    ptrs = [{"kind": "devlog", "path": fx["devlog"], "cell_id": cell, "node_id": "main"},
            {"kind": "testlog", "path": fx["testlog"], "cell_id": cell, "node_id": "main"}]
    if with_report_pointer:
        ptrs.append({"kind": "bench_report", "path": _HINT_FX_LITE_REPORT, "cell_id": cell, "node_id": "cluster"})
        ptrs.append({"kind": "certificate", "path": _HINT_FX_LITE_CERT, "cell_id": cell, "node_id": "cluster"})
    _hint_edit_json(camp / "evidence_pointers.json", lambda d: d["pointers"].extend(ptrs))


def _test_hint_map_only_promotion() -> None:
    """`hint_map_only` — 계약 v5 §3 의 '결손 기재 후 발행' 이 **도달 가능한** 유일한 통로(게이트 평면).

    2026-09-07 신설(plan_26090715 §4.6). 종전에는 계약이 "§3·§4·§5 는 결손 기재 후 발행" 이라
    적었는데 코드에서 도달 불가였다 — 발행기가 요구하는 promotion-ready manifest 가
    full_benchmark ∧ mode=full ∧ verdict=PASS ∧ 증거 5종일 때만 나왔기 때문이다. 그래서 사람이
    vault 사본을 만들어 우회했다(D3 위반의 형태). 이 시험은 **문이 실제로 열리는지**와
    **열린 문으로 성능 주장이 새어 나가지 않는지**를 함께 본다.
    본문 마커(⑥ OBSERVATION-ONLY · PERF-WARNING)는 게이트가 아니라 발행기 린트(`hintlib.template` · 00-hint.md)의 소관이라
    2026-09-22 부터 아래 CLI 시험(`_test_hint_map_only_publication` · `_test_hint_cli_publication`)이 친다(평면 분리).
    """
    def _verify(root: Path, manifest: Path) -> dict:
        cp = subprocess.run([sys.executable, str(RUNTIME_DIR / "completion_gate.py"),
                             "verify", "--manifest", str(manifest), "--repo-root", str(root)],
                            cwd=str(root), capture_output=True, text=True, timeout=120)
        try:
            return json.loads(cp.stdout)
        except ValueError:
            return {"_rc": cp.returncode, "_stdout": cp.stdout[-400:], "_stderr": cp.stderr[-400:]}

    with tempfile.TemporaryDirectory(prefix="hint-map-only-selftest.") as td:
        root = Path(td).resolve()
        (root / ".git").mkdir()
        # ① bench_report·simlog 없이 **lite 등급 인증서 하나로** 지도 발행 통로가 열린다(2026-09-29 plan_26092923:
        #    hint 자격 = lite 통과 · 종전 "인증서 없이" 는 lite 불통과 셀도 hint 를 낼 수 있던 교집합이었다).
        m = _write_promotion_manifest(root, "PASS", dict(_PROMO_RUBRIC), certificate=_PROMO_LITE_CERTIFICATE,
                                      task_class="hint_map_only",
                                      drop_evidence=("simlog", "bench_report"))
        out = _verify(root, m)
        _require(out.get("state") == "promotion-ready" and out.get("eligible_for_promotion") is True
                 and "HINT_MAP_ONLY_PROMOTION" in (out.get("reason_codes") or [])
                 and (out.get("certificate") or {}).get("grade") == "lite",
                 f"hint_map_only did not reach promotion-ready with a lite-grade certificate: {out}")

        # ①-b ★음성대조 lite 등급 인증서가 없으면 자격이 닫힌다(lite 불통과·판정 부재 = 인증서 없음).
        m = _write_promotion_manifest(root, "PASS", dict(_PROMO_RUBRIC),
                                      task_class="hint_map_only",
                                      drop_evidence=("simlog", "bench_report"))
        out = _verify(root, m)
        _require(out.get("eligible_for_promotion") is not True
                 and "HINT_MAP_REQUIRES_LITE_PASS_CERTIFICATE" in (out.get("reason_codes") or []),
                 f"hint_map_only reached promotion without a lite-pass certificate: {out}")
        # ①-c ★음성대조 lite_verdict 가 pass 가 아닌 lite 인증서 · full 인증서는 지도 발행 근거가 아니다.
        for bad, code in ((_PROMO_LITE_CERTIFICATE.replace("lite_verdict: pass", "lite_verdict: server_failed"),
                           "CERTIFICATE_LITE_VERDICT_NOT_PASS"),
                          (_PROMO_CERTIFICATE.format(authority="explore"), "CERTIFICATE_GRADE_MISMATCH")):
            m = _write_promotion_manifest(root, "PASS", dict(_PROMO_RUBRIC), certificate=bad,
                                          task_class="hint_map_only", drop_evidence=("simlog", "bench_report"))
            out = _verify(root, m)
            _require(out.get("eligible_for_promotion") is not True and code in (out.get("reason_codes") or []),
                     f"hint_map_only accepted a non-lite-pass certificate (want {code}): {out}")
        # ①-d ★AC6 성능 완료(full_benchmark 승격)는 lite 등급 인증서로 열리지 않는다 · 등급 키 없는 옛 full 인증서는 full.
        m = _write_promotion_manifest(root, "PASS", dict(_PROMO_RUBRIC), certificate=_PROMO_LITE_CERTIFICATE)
        out = _verify(root, m)
        _require(out.get("eligible_for_promotion") is not True
                 and "CERTIFICATE_GRADE_NOT_FULL" in (out.get("reason_codes") or []),
                 f"full_benchmark promotion opened on a lite-grade certificate: {out}")
        m = _write_promotion_manifest(root, "PASS", dict(_PROMO_RUBRIC),
                                      certificate=_PROMO_CERTIFICATE.format(authority="explicit")
                                      .replace("benchmark_mode: full\n", ""))
        out = _verify(root, m)
        _require("CERTIFICATE_BENCHMARK_MODE_MISMATCH" not in (out.get("reason_codes") or [])
                 and (out.get("certificate") or {}).get("grade") == "full",
                 f"a legacy certificate without benchmark_mode was not read as full: {out}")

        # ② ★음성대조 verdict=FAIL 은 사람 positive key 없이는 못 연다(§3.2 perf_waiver).
        m = _write_promotion_manifest(root, "FAIL", dict(_PROMO_RUBRIC), certificate=_PROMO_LITE_CERTIFICATE,
                                      task_class="hint_map_only",
                                      drop_evidence=("simlog", "bench_report"))
        out = _verify(root, m)
        _require(out.get("eligible_for_promotion") is not True
                 and "HINT_MAP_FAIL_REQUIRES_WAIVER" in (out.get("reason_codes") or []),
                 f"a FAIL verdict reached promotion through hint_map_only without a waiver: {out}")

        # ③ waiver 4필드가 있으면 열린다 — 대가는 본문 PERF-WARNING 이고 그것은 hintlib.template 린트가 집행한다.
        m = _write_promotion_manifest(
            root, "FAIL", dict(_PROMO_RUBRIC, perf_waiver={
                "authorized_by": "selftest-operator", "authorized_at_utc": "2026-09-07T00:00:00Z",
                "instruction": "loop-until-done 중단", "warning_flag": "PERF-WARNING: selftest"}),
            certificate=_PROMO_LITE_CERTIFICATE,
            task_class="hint_map_only", drop_evidence=("simlog", "bench_report"))
        out = _verify(root, m)
        _require(out.get("eligible_for_promotion") is True,
                 f"hint_map_only with a complete perf_waiver was still blocked: {out}")

        # ④ ★음성대조 여정을 담는 셋(plan·devlog·testlog)은 면제되지 않는다.
        m = _write_promotion_manifest(root, "PASS", dict(_PROMO_RUBRIC),
                                      task_class="hint_map_only",
                                      drop_evidence=("simlog", "bench_report", "devlog"))
        out = _verify(root, m)
        _require(out.get("eligible_for_promotion") is not True,
                 f"hint_map_only reached promotion without devlog -- the journey evidence is not optional: {out}")

        # ⑤ ★음성대조 종전 경로 불변: full_benchmark 는 여전히 증거 5종을 요구한다.
        m = _write_promotion_manifest(root, "PASS", dict(_PROMO_RUBRIC),
                                      drop_evidence=("simlog",))
        out = _verify(root, m)
        _require(out.get("eligible_for_promotion") is not True,
                 f"full_benchmark stopped requiring simlog -- the new class relaxed the old one: {out}")

    # ⑦ ★음성대조 종결을 말하지 않는 simlog vault 는 차단된다(2026-09-07 · 유예 결함 ⑥).
    with tempfile.TemporaryDirectory(prefix="simlog-unterminated-selftest.") as td:
        root = Path(td).resolve()
        (root / ".git").mkdir()
        m = _write_promotion_manifest(root, "PASS", dict(_PROMO_RUBRIC))
        vault = root / "docs" / "simlog" / "selftest_run"
        (vault / "run_summary.json").unlink()          # 종결 기록만 뺀다 — 로그 파일은 그대로
        out = _verify(root, m)
        _require("EVIDENCE_SIMLOG_RUN_UNTERMINATED" in (out.get("reason_codes") or []),
                 f"a simlog vault with no terminal record still passed the evidence gate: {out}")
        # bench vault 장르는 run_summary 없이도 통과해야 한다(장르를 섞으면 정상 산출물이 막힌다)
        (vault / "level_01_measured.json").write_text(
            json.dumps({"measurement_ok": True, "decode_tps": 1.0}) + "\n", encoding="utf-8")
        out = _verify(root, m)
        _require("EVIDENCE_SIMLOG_RUN_UNTERMINATED" not in (out.get("reason_codes") or []),
                 f"a bench-genre vault was blocked by a trial-genre rule: {out}")


def _test_hint_map_only_publication() -> None:
    """O5 — lite 만 잰 셀의 hint 가 신 CLI 전 경로를 **완주**한다(plan_26091407 §4.5 · §7 O5 → plan_26092119 S4 재작성).

    2026-09-14 신설 사유(HIST): 종전에는 `hint_map_only` 가 승격 게이트는 통과하는데 seal 이 HINT_CERTIFICATE_EVIDENCE_MISSING
    으로 죽었다(audit_26091323 §1 · work-manifest 13건 전부 · 이 통로로 봉인된 태그 0). 2026-09-22 부터 **실물 순서 그대로**
    CLI 로 밟는다: 배포되는 render_report 가 경량 리포트를 쓰고 → `hint.py publish`(노드 축 = cluster 자동 파생) 가 발행기를 구동해
    hint_map_only + publish-lite-report 로 묶고(손 JSON 0) → 저작 → `hint.py continue` 가 봉인·push·카탈로그까지.
    ★ 음성대조: 경량 리포트가 셀에 없으면 publish 가 첫 쓰기 전에 막힌다(자격 조인 키·묶을 계측 산출물 없음 · 부수효과 0) ·
    00-hint.md 에서 OBSERVATION-ONLY 마커를 걷으면 린트가 HINT_MAP_ONLY_OBSERVATION_MARKER_MISSING 으로 막는다(옛 ⑥ 의 이관)."""
    hint = _import_hint_cli()
    with tempfile.TemporaryDirectory(prefix="hint-cli-lite.") as tds:
        td = Path(tds).resolve()
        with _hint_isolated_env(td):
            f = _hint_cli_fixture(td, hint, legacy=False)
            repo = f["repo"]
            # ★ 리포트 포인터 없는 lite 셀 — 묶을 계측 산출물이 없고, lite 자격의 조인 키(리포트 `생성일` = lite raw 의
            #   measured_utc)도 없다 → 발행 자격이 관측되지 않아 첫 쓰기 전에 거부(부수효과 0 · 옛 "경량 리포트 미바인딩 map_only
            #   봉인 거부" 의 후계 — 이제는 봉인까지 가지 않고 publish 입구에서 멈춘다)
            _hint_fx_lite_cell(repo, f["fx"], with_report_pointer=False)
            tree0, refs0 = hint._fx_tree(repo), _hint_refs(repo)
            proc = _hint_cli(f, "publish", "--campaign", _HINT_FX_CAMPAIGN, "--cell", _HINT_FX_LITE_CELL,
                             "--node", "cluster", "--generated-utc", _HINT_FX_PUBLISH_UTC)
            _require(proc.returncode != 0 and "HINT_QUALIFICATION_UNOBSERVED" in proc.stderr
                     and hint._fx_tree(repo) == tree0 and _hint_refs(repo) == refs0,
                     f"a lite cell with no bound lite report was not refused before any write: rc={proc.returncode} "
                     f"{proc.stderr[-600:]!r}")
            _hint_edit_json(repo / f"campaigns/{_HINT_FX_CAMPAIGN}/evidence_pointers.json",
                            lambda d: d["pointers"].extend([
                                {"kind": "bench_report", "path": _HINT_FX_LITE_REPORT,
                                 "cell_id": _HINT_FX_LITE_CELL, "node_id": "cluster"},
                                # lite 등급 인증서(β) — hint 자격의 근거(2026-09-29 plan_26092923)
                                {"kind": "certificate", "path": _HINT_FX_LITE_CERT,
                                 "cell_id": _HINT_FX_LITE_CELL, "node_id": "cluster"}]))

            _hint_cli_ok(f, "publish(lite)", "publish", "--campaign", _HINT_FX_CAMPAIGN, "--cell", _HINT_FX_LITE_CELL,
                         "--generated-utc", _HINT_FX_PUBLISH_UTC)       # --node 없음 = TP 2 > 노드당 GPU 1 → cluster 파생(AC7)
            draft = _hint_single_draft(repo)
            st = json.loads((draft / "state.json").read_text(encoding="utf-8"))
            _require(st.get("tag") == _HINT_FX_LITE_TAG, f"lite cell did not derive the expected v7 base name: {st.get('tag')!r}")
            man = json.loads((repo / st["manifest"]).read_text(encoding="utf-8"))
            _require(man.get("task_class") == "hint_map_only" and (man.get("benchmark") or {}).get("mode") == "lite"
                     and str(((man.get("evidence") or {}).get("bench_report") or {}).get("path", ""))
                     .endswith(Path(_HINT_FX_LITE_REPORT).name)
                     # 2026-09-29(plan_26092923): hint 자격 = lite 통과 → lite 등급 인증서가 함께 묶인다(종전: 인증서 없음)
                     and str(((man.get("evidence") or {}).get("certificate") or {}).get("path", ""))
                     .endswith(Path(_HINT_FX_LITE_CERT).name),
                     f"lite publication was not driven as hint_map_only + bound lite report + lite certificate: "
                     f"{man.get('task_class')!r} {man.get('benchmark')!r} {man.get('evidence')!r}")

            # ★ OBSERVATION-ONLY 는 기계 렌더되고, 걷어내면 린트가 막는다(옛 hint_tag._require_map_only_observation 의 이관)
            hint00 = draft / "payload/00-hint.md"
            text00 = hint00.read_text(encoding="utf-8")
            marker = hint.template.MAP_ONLY_MARKER

            def lint_codes() -> set:
                return {i["code"] for i in json.loads(_hint_cli(f, "lint", "--draft", str(draft), "--json").stdout)}

            _require(marker == "OBSERVATION-ONLY" and marker in text00
                     and "HINT_MAP_ONLY_OBSERVATION_MARKER_MISSING" not in lint_codes(),
                     "a hint_map_only scaffold does not carry the machine-rendered OBSERVATION-ONLY marker")
            hint00.write_text(text00.replace(marker, "OBSERVATION"), encoding="utf-8")
            _require("HINT_MAP_ONLY_OBSERVATION_MARKER_MISSING" in lint_codes(),
                     "a hint_map_only body without the OBSERVATION-ONLY marker passed lint")
            hint00.write_text(text00, encoding="utf-8")

            _hint_author(f, draft)
            proc = _hint_cli(f, "lint", "--draft", str(draft), "--json")
            _require(proc.returncode == 0 and proc.stdout.strip() == "[]",
                     f"programmatic authoring left lint issues on the lite draft: {proc.stdout[-900:]!r}")
            _hint_cli_ok(f, "continue(lite)", "continue", "--draft", str(draft), "--generated-utc", _HINT_FX_CONTINUE_UTC,
                         "--remote", "origin")
            tag_ref = f"refs/tags/{_HINT_FX_LITE_TAG}"
            _require(_hint_remote_ref(f["bare"], tag_ref) == _hint_git(repo, "rev-parse", tag_ref),
                     "lite tag did not reach the remote with the sealed object")
            body = _hint_tag_body(repo, _HINT_FX_LITE_TAG)
            # 바인딩 우선순위는 발행기 소유(인증서 > 리포트) — lite 셀도 이제 lite 등급 인증서를 묶는다(등급은 인증서 본문이 말한다)
            _require(f"bench_ref: ../benchmark/{Path(_HINT_FX_LITE_CERT).name}" in body and "bench_kind: certificate" in body,
                     f"map_only footer v2 did not bind the lite-grade certificate: {body[-500:]!r}")
            # 페이로드는 태그의 앵커 커밋에서 읽는다(v7: 페이로드 커밋은 hint 브랜치에 얹히지 않는다 · 태그만 가리킨다)
            payload = json.loads(_hint_git(repo, "show", f"{tag_ref}^{{commit}}:PAYLOAD.json"))
            _require((payload.get("measurement") or {}).get("verdict") == "OBSERVATION-ONLY",
                     f"a lite-only cell did not carry OBSERVATION-ONLY on the machine surface: {payload.get('measurement')!r}")
            mc = payload.get("measurement_config") or {}
            _require("BENCH_MODE_LITE" in (payload.get("missing") or [])
                     and "HINT_MISSING_LITE" not in (payload.get("missing") or [])
                     and mc.get("bench_mode") == "lite" and mc.get("bench_mode_kind") == "declared-lite"
                     and mc.get("repeats") == 1 and mc.get("bench_tool") == "vllm-bench-serve"
                     and mc.get("downgrade_reason") is None,
                     f"PAYLOAD must carry BENCH_MODE_LITE and the parsed measurement config: "
                     f"{payload.get('missing')!r} {mc!r}")
            idx = json.loads((repo / "hints/index.json").read_text(encoding="utf-8"))
            entries = [e for e in idx.get("hints", []) if e.get("tag") == _HINT_FX_LITE_TAG]
            _require(len(entries) == 1 and entries[0].get("bench_mode") == "lite(선언)"
                     and "BENCH_MODE_LITE" in (entries[0].get("missing") or []),
                     f"catalog did not derive the bench_mode column from the remote tag payload: {entries!r}")
            hints_md = (repo / "HINTS.md").read_text(encoding="utf-8")
            _require("| lite(선언) |" in hints_md, f"HINTS.md row lacks the derived bench_mode column: {hints_md[-600:]!r}")


def _test_policy_and_evidence_lifecycle() -> None:
    _require(policy_registry.scan_policy_citations("policy:SELFTEST_OK") == [(1, "SELFTEST_OK")],
             "policy citation broad scanner failed its canonical positive")
    _require(policy_registry.scan_policy_citations("xpolicy:NOT_A_CITATION") == [],
             "policy citation scanner accepted an embedded token")
    with contextlib.redirect_stdout(io.StringIO()):
        _require(policy_registry._self_test() == 0, "policy registry self-test did not return 0")
        evidence_publisher._self_test()


AGENT_CONTROL_REL = ".claude/skills/terraforming_node/scripts/agent_control.py"


def _test_agent_control_owner_selftest() -> None:
    """agent_control(A2A 전송 orchestrator)의 경계·소진·러너 판정 회귀는 **owner 의 `--self-test`** 가 소유한다.

    plan_26093022 Q3: agent_control 은 terraforming_node 스킬로 이동했다 — 기초층은 스킬의 사설 함수를
    import 하지 않고(닫힌 목록 · 공개 CLI 간선 1개), 그 self-test 를 같은 인터프리터 플래그로 실행한다.
    """
    script = REPO_ROOT / AGENT_CONTROL_REL
    _require(script.is_file(), f"agent_control owner script missing: {script}")
    proc = subprocess.run([*verify_distribution._child_python(), str(script), "--self-test"],
                          capture_output=True, text=True, check=False)
    _require(proc.returncode == 0,
             f"agent_control --self-test failed rc={proc.returncode}: {(proc.stderr or proc.stdout)[-800:]}")


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
    base_atoms = [f"approved_by: {approved_by}", f"approved_at_utc: {approved_at}",
                  "allowed_action: sync_to_sub"]
    scope = {"status": "approved", "topology": "single", "node_id": "sub-1",
             "ssh_host": "probe@node.example", "work_dir": "/srv/vllm",
             "planes": ["overlay"], "approved_utc": approved_at}
    scope_atoms = ["propagation_topology: single", "propagation_node_id: sub-1",
                   "propagation_ssh_host: probe@node.example", "propagation_work_dir: /srv/vllm",
                   "propagation_planes: overlay"]
    atoms = base_atoms + scope_atoms
    plan_body = ("# selftest plan\n\n## Execution approval\n" + "\n".join(base_atoms) + "\n")

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
            propagation_authorization = over.pop("propagation_authorization", None)
            execution_approval = over.pop("execution_approval", None)
            ea = {"approved": True, "approved_by": approved_by, "approved_at_utc": approved_at,
                  "plan_path": rel, "plan_sha256": sha, "approval_anchor": "## Execution approval",
                  "approval_atoms": list(base_atoms), "allowed_actions": ["sync_to_sub"]}
            if execution_approval is not None:
                ea = execution_approval
            else:
                ea.update(over)
            # evidence.plan.path 는 plan_path 를 따라간다 — 어긋나면 **앵커 검사 이전에** 경로
            #   불일치로 걸려, 이 시험이 겨냥한 가드가 아닌 다른 가드를 확인하게 된다.
            return {"schema_version": 1, "task_class": "harness_change",
                    "identity": {"model": "m", "gpu": "g", "vllm": "v", "quant": None,
                                 "topology": "single", "tp": 1},
                    "evidence": {"plan": {"path": ea["plan_path"]}},
                    "pii_scan": {"passed": True}, "execution_approval": ea,
                    **({"propagation_authorization": propagation_authorization}
                       if propagation_authorization is not None else {})}

        def _run(man, name):
            # Scope cases get their own approved plan bytes and digest; the base fixture stays
            # a non-propagation approval with only its legacy atoms.
            ea = man.get("execution_approval")
            if man.get("propagation_authorization") and isinstance(ea, dict):
                scoped_plan = "# selftest plan\n\n## Execution approval\n" + "\n".join(ea["approval_atoms"]) + "\n"
                plan_path.write_text(scoped_plan, encoding="utf-8")
                ea["plan_sha256"] = hashlib.sha256(plan_path.read_bytes()).hexdigest()
            else:
                plan_path.write_text(plan_body, encoding="utf-8")
                # 자기 plan 파일·digest 를 들고 오는 케이스는 다시 계산하지 않는다(wrong_action = 다른 액션의 일치 승인 · 2026-09-22).
                if isinstance(ea, dict) and name not in ("bad_sha", "no_anchor", "wrong_action"):
                    ea["plan_sha256"] = hashlib.sha256(plan_path.read_bytes()).hexdigest()
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

        # Established propagation authorization is a strict destination scope, not a broad
        # execution approval.  It admits sync_to_sub without a fresh plan approval only when
        # its topology agrees with the strong identity; transport then verifies node/host/path.
        propagation = _run(_manifest(execution_approval={
            "approved": True, "approved_by": approved_by, "approved_at_utc": approved_at,
            "plan_path": rel, "plan_sha256": sha, "approval_anchor": "## Execution approval",
            "approval_atoms": list(atoms), "allowed_actions": ["sync_to_sub"]
        }, propagation_authorization={**scope, "approval_atoms": list(atoms)}), "propagation")
        _require(propagation.get("allowed") is True,
                 f"approved exact propagation scope must admit sync_to_sub: {propagation}")

        no_execution_manifest = _manifest()
        no_execution_manifest.pop("execution_approval")
        no_execution_manifest["propagation_authorization"] = {
            "status": "approved", "topology": "single", "node_id": "sub-1",
            "ssh_host": "probe@node.example", "work_dir": "/srv/vllm",
            "planes": ["overlay"], "approved_utc": approved_at,
            "approval_atoms": list(atoms)}
        no_execution = _run(no_execution_manifest, "no_execution")
        _require(no_execution.get("allowed") is False
                 and "PROPAGATION_AUTHORIZATION_EXECUTION_APPROVAL_ABSENT" in no_execution.get("reason_codes", []),
                 f"standalone propagation scope must be rejected: {no_execution}")

        bad_propagation = _run(_manifest(execution_approval={
            "approved": True, "approved_by": approved_by, "approved_at_utc": approved_at,
            "plan_path": rel, "plan_sha256": sha, "approval_anchor": "## Execution approval",
            "approval_atoms": list(atoms), "allowed_actions": ["sync_to_sub"]
        }, propagation_authorization={
            "status": "approved", "topology": "multi", "node_id": "sub-1",
            "ssh_host": "probe@node.example", "work_dir": "/srv/vllm",
            "planes": ["band2", "overlay"], "approved_utc": approved_at,
            "approval_atoms": list(atoms)
        }), "bad_propagation")
        _require(bad_propagation.get("allowed") is False
                 and "PROPAGATION_AUTHORIZATION_TOPOLOGY_MISMATCH" in bad_propagation.get("reason_codes", []),
                 f"propagation scope topology drift must be rejected: {bad_propagation}")

        changed_destination = _run(_manifest(execution_approval={
            "approved": True, "approved_by": approved_by, "approved_at_utc": approved_at,
            "plan_path": rel, "plan_sha256": sha, "approval_anchor": "## Execution approval",
            "approval_atoms": list(atoms), "allowed_actions": ["sync_to_sub"]
        }, propagation_authorization={**scope, "work_dir": "/srv/other", "approval_atoms": list(atoms)}), "changed_destination")
        _require(changed_destination.get("allowed") is False and "EXECUTION_APPROVAL_ATOMS_INVALID" in changed_destination.get("reason_codes", []),
                 f"destination drift outside approved plan bytes must invalidate the approval atoms: {changed_destination}")

        shell_destination = _run(_manifest(execution_approval={
            "approved": True, "approved_by": approved_by, "approved_at_utc": approved_at,
            "plan_path": rel, "plan_sha256": sha, "approval_anchor": "## Execution approval",
            "approval_atoms": list(atoms), "allowed_actions": ["sync_to_sub"]
        }, propagation_authorization={**scope, "work_dir": "/srv/vllm;id", "approval_atoms": list(atoms)}), "shell_destination")
        _require(shell_destination.get("allowed") is False and "PROPAGATION_AUTHORIZATION_WORK_DIR_INVALID" in shell_destination.get("reason_codes", []),
                 f"shell metacharacter destination must be rejected: {shell_destination}")

        # Static integration tripwire: this guard is positioned before the B1 transaction and
        # checkout, so a same-scope invocation cannot delete opposite-branch tracked paths.
        sync_script = (REPO_ROOT / ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh").read_text(encoding="utf-8")
        guard = "STOP(PROPAGATION_SCOPE_BRANCH_TRANSITION)"
        _require(guard in sync_script and sync_script.index(guard) < sync_script.index("begin_remote_transaction \"$t\" 0"),
                 "established scope must reject branch transition before B1 transaction")

        bad_sha = _run(_manifest(plan_sha256="0" * 64), "bad_sha")
        _require(bad_sha.get("allowed") is False
                 and "EXECUTION_APPROVAL_PLAN_DIGEST_MISMATCH" in bad_sha.get("reason_codes", []),
                 f"a plan whose bytes changed after approval must be rejected: {bad_sha}")

        not_approved = _run(_manifest(approved=False), "not_approved")
        _require(not_approved.get("allowed") is False
                 and "EXECUTION_APPROVAL_NOT_APPROVED" in not_approved.get("reason_codes", []),
                 f"approved=false must be rejected: {not_approved}")

        # "다른 액션" 대조(2026-09-22 교정): 종전 픽스처는 manifest 원자만 `allowed_action: hint_create` 로 바꾸고 plan 은
        #   `sync_to_sub` 원자를 그대로 두었다 — 그래서 **액션 검사에 닿기도 전에** EXECUTION_APPROVAL_PLAN_ATOMS_MISSING 으로
        #   막혔고, "다른 액션이라 막혔다" 는 한 번도 증명되지 않았다(allowed=false 만 봤다 · 이 교정의 첫 실행이 드러냈다).
        #   이제 plan·manifest 가 **서로 일치하는** 다른 액션 승인을 짓고, 거부 사유가 액션 불일치인지까지 본다. 값은 살아 있는
        #   어휘여야 한다(X4: hint_create 는 어휘에서 빠졌다 — 죽은 값이면 스키마 거부로 막힌다).
        wrong_atoms = base_atoms[:2] + ["allowed_action: hint_push"]
        wrong_plan = repo / "docs/plan/w.md"
        wrong_plan.write_text("# selftest plan\n\n## Execution approval\n" + "\n".join(wrong_atoms) + "\n",
                              encoding="utf-8")
        wrong_action = _run(_manifest(plan_path="../plan/w.md",
                                      plan_sha256=hashlib.sha256(wrong_plan.read_bytes()).hexdigest(),
                                      allowed_actions=["hint_push"], approval_atoms=wrong_atoms),
                            "wrong_action")
        _require(wrong_action.get("allowed") is False
                 and wrong_action.get("reason_codes") == ["EXECUTION_APPROVAL_ACTION_NOT_ALLOWED"],
                 f"an approval for a different (live) action must be refused *because the action differs*: {wrong_action}")

        no_anchor_plan = repo / "docs/plan/q.md"
        no_anchor_plan.write_text("# no anchor here\n", encoding="utf-8")
        no_anchor = _run(_manifest(plan_path="../plan/q.md",
                                   plan_sha256=hashlib.sha256(no_anchor_plan.read_bytes()).hexdigest()),
                         "no_anchor")
        _require(no_anchor.get("allowed") is False
                 and "EXECUTION_APPROVAL_PLAN_ATOMS_MISSING" in no_anchor.get("reason_codes", []),
                 f"a plan without the anchor/atoms must be rejected: {no_anchor}")


# ─────────────────────────────────────────────────────────────────────────────
# tripwire 3종 (2026-09-03 신설 · plan_26090222 P2)
#
# 왜 여기인가: 해시 중복층을 걷어낸 뒤 **재발을 막는 것**은 목록(allowlist)이 아니라 파생 술어여야
# 한다. 목록은 새 파일이 생기면 조용히 늦어지지만, 파생 술어는 "git 이 이미 아는 것을 또 적었다"는
# 성질 자체를 본다. 세 단언은 병목(pre-commit)에 걸리므로 **1초 예산**을 지킨다.
#
# ⚠ 픽스처 격리: 이 파일의 단언은 저장소 상태를 읽는다. 그런데 hint CLI E2E(`_hint_cli_fixture()` · 2026-09-22 옛
# `_hint_repo()` 의 후계)는 격리 git 레포를 만들고 completion_gate 를 그 안에서 돌린다 — 거기서 브랜치 부분집합
# 단언이 발화하면 셀프테스트가 **자기 자신을 RED** 로 만든다. 그래서 모든 진입점이
# `_is_canonical_repo()` 로 "정본(헌법 소유) 저장소인가"를 먼저 판별한다.
# ─────────────────────────────────────────────────────────────────────────────

REPO_ROOT = RUNTIME_DIR.parents[2]

# 헌법을 소유한 저장소만 가지는 구조적 표지. 픽스처 레포(`_hint_cli_fixture` · 게이트 전용 임시 루트)는 completion_gate·
# 발행기·hint 스크립트만 두므로(심링크·사본) 이 셋을 동시에 갖지 못한다.
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
# `.native-e2e` (2026-09-23 · plan_26092311): native 서빙의 마커 소유 휘발 run root — 이미지에서 재포장한 **서드파티
#   설치본**(venv 의 jupyter `package.json.orig` 등)이 들어 있고 down 이 통째로 지운다. 백업 관행의 흔적이 아니다.
_BACKUP_SCAN_PRUNE_TOP = frozenset({".git", "seed", "output", ".native-e2e"})
# `.claude/worktrees/` is the harness isolation container, not a governed subtree.
_WORKTREE_ISOLATION_REL = Path(".claude/worktrees")

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
# ★ 2026-09-06 사정거리 정정(plan_26090616): 이 자리에는 세 접두어 + 세 파일의 **목록**이 있었다.
#   목록이라 새 배포면이 생기면 조용히 늦었고, 실제로 늦었다 — 루트에 쌓인 캠페인 셀 입력 21개가
#   운영자 NAS 절대경로를 담은 채 공개 원격까지 갔는데 이 가드의 사정거리 밖이었다. 배포되는 것은
#   **추적물**이므로(docs.md §보관·전파 matrix) 술어를 목록에서 `추적물 전부`로 되돌린다 —
#   allowlist 없는 파생 술어라 새 파일·새 디렉터리가 자동으로 사정거리에 든다.
#   비용 실측(2026-09-06 · 342 추적물): 0.06s — 1초 예산 안이다.

# tripwire ⑦ — 루트 표면 등록부. 저장소 루트는 어떤 스킬도 소유하지 않는 공유 표면이라, 산출물이
# 흘러도 관할 게이트가 0 이었다. 등록부가 그 소유를 만들고 이 술어가 대조한다.
_ROOT_REGISTRY_REL = ".claude/policies/root_registry.json"

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
    "tripwire⑤no-pii-in-deployed-artifacts(추적물 전부)",
    "tripwire⑥no-revived-antipatterns",
    "tripwire⑦root-surface-registry",
    "tripwire⑧branch-constitution-layering(4자일치·공통층 어휘)",
    "tripwire⑨base_to_skill_edges(닫힌 목록 · 차단 검사 ②)",
    "tripwire⑩ghost_section_anchors·skill_literal_binding(차단 검사 ①④)",
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
    `json.loads` 하는 소비자가 셋(`sync_branches.sh`·`sync_to_sub.sh`·`hint.py`(hintlib.evidence.authorize))이고,
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


def _live_registered_worktree_branches(root: Path) -> set[str]:
    """Return branches backed by a present, usable, non-prunable worktree stanza.

    `--porcelain -z` makes path attributes NUL-terminated rather than C-quoted, so whitespace
    and other special path characters remain byte-for-byte input to ``Path``.
    """
    proc = subprocess.run(["git", "-C", str(root), "worktree", "list", "--porcelain", "-z"],
                          capture_output=True, timeout=120)
    if proc.returncode != 0:
        raise RuntimeSelftestFailure(
            f"git worktree list --porcelain -z failed in {root}: "
            f"{proc.stderr.decode(errors='replace').strip()}"
        )
    live: set[str] = set()
    records = proc.stdout.split(b"\0")
    for start, attribute in enumerate(records):
        if not attribute.startswith(b"worktree "):
            continue
        stanza = []
        for item in records[start:]:
            if not item:
                break
            stanza.append(item)
        location = attribute[len(b"worktree "):]
        branch = next((item[len(b"branch refs/heads/"):] for item in stanza
                       if item.startswith(b"branch refs/heads/")), None)
        prunable = any(item.startswith(b"prunable") for item in stanza)
        if not branch or prunable:
            continue
        worktree = Path(os.fsdecode(location))
        if not worktree.is_dir():
            continue
        check = subprocess.run(["git", "-C", str(worktree), "rev-parse", "--show-toplevel"],
                               capture_output=True, timeout=60)
        # Git's protocol line terminator on this target is LF.  Remove exactly that byte: a
        # preceding CR can be a legitimate POSIX pathname byte, not a transport terminator.
        reported = check.stdout.removesuffix(b"\n")
        if check.returncode == 0 and Path(os.fsdecode(reported)).resolve() == worktree.resolve():
            live.add(os.fsdecode(branch))
    return live


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
        elif Path(rel_dir) == _WORKTREE_ISOLATION_REL:
            dirnames[:] = []
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
    # A branch is exempt only when its own porcelain stanza identifies a live worktree.  A name
    # prefix, a prunable registration, and a missing/broken directory remain forbidden backup refs.
    registered = _live_registered_worktree_branches(root)
    stray = sorted(b for b in branches if b not in _ALLOWED_BRANCHES and b not in registered)
    _require(not stray, f"refs/heads must be allowed durable branches or live registered worktrees: {stray}")

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

    targets = sorted(_walk_governing_files(root, "*.md"))
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
    """`scan_forbidden_strings.py` 를 in-process 로 적재한다(`_import_hint_cli` 와 같은 관용구).

    패턴·면제 규칙을 **복제하지 않는다** — 복제하면 두 자리가 갈라져 한쪽만 조용히 늦는다
    (`workflow.md` §결정론 규율 — 개념 중복). 커널은 그 파일의 `scan_lines` 하나다.
    """
    path = CLAUDE_DIR.parent / _FORBIDDEN_SCANNER_REL
    spec = importlib.util.spec_from_file_location("_runtime_selftest_pii_scan", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _deployed_pii_targets(root: Path) -> list[str]:
    """배포면에 해당하는 경로(repo-relative) = **추적물 전부**.

    추적물이 곧 배포물이다(`docs.md` §보관·전파 matrix) — 클론에 실리는 것은 인덱스에 있는 것이고,
    인덱스에 없는 것은 어떤 수신자에게도 도달하지 않는다. 그래서 이 술어에는 allowlist 가 없다:
    접두어 목록으로 좁히면 목록에 없는 새 배포면이 조용히 사정거리 밖에 남는다(2026-09-06 실측).
    """
    return list(_tracked_paths(root))


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
    hint-publisher 의 배포 평면 fail-closed 스캔이다 — 패턴 정본 `hintlib.pii`(deploy 프로필) · 집행 자리
    `hint.py continue`(배관 커밋 `hintlib.branch.commit_payload` + 봉인 `hintlib.tag.seal`)·`hint.py verify`
    (2026-09-22 옛 `hint_tag.finalize/verify` 에서 이관)이며, 트리 스캐너의 generic 축을
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
        # 사정거리가 다시 목록으로 좁아지는 회귀를 막는다 — 옛 세 접두어 밖의 추적물이 실제로
        # 들어 있어야 한다(그것이 2026-09-06 에 뚫린 자리다).
        _require(any(not r.startswith((".claude/", "docs/report/", "hints/"))
                     and r not in ("CLAUDE.md", "README.md", "HINTS.md") for r in targets),
                 "tripwire5 reach regressed to the old three-prefix list")


def _load_root_registry(root: Path) -> dict:
    """등록부를 읽는다. 부재·파손은 통과가 아니라 **FAIL** 이다 — 단일 권위가 사라지면 이 가드는
    통과한 것이 아니라 무력한 것이고, 그 둘을 구분하지 않으면 가드가 없는 것과 같다."""
    path = root / _ROOT_REGISTRY_REL
    _require(path.is_file(), f"root registry missing -- tripwire7 has no authority to compare against: {path}")
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise RuntimeSelftestFailure(f"root registry unreadable: {exc}") from exc
    _require(isinstance(doc, dict) and isinstance(doc.get("entries"), list),
             "root registry must carry an `entries` list")
    return doc


def _root_names_of(paths) -> set:
    """repo-relative 경로들의 **최상위 이름** 집합. `a/b/c` -> `a`, `x.md` -> `x.md`."""
    return {rel.split("/", 1)[0] for rel in paths if rel}


def _test_root_surface_registry(root: Path | None = None) -> None:
    """tripwire ⑦ — 저장소 루트에 사는 것은 등록부가 정한 것뿐이다.

    ★ 왜 tripwire 인가(2026-09-06 실측 · plan_26090616): 루트는 어떤 스킬도 소유하지 않는 공유
    표면이었다. 캠페인이 셀마다 `config.campaign-*.yaml` 을 루트에 쓰자 `.gitignore` 의 접두어-exact
    규칙(`/config.yaml`)이 그 변형을 놓쳤고, pre-commit PII tripwire 의 사정거리는 세 접두어였고,
    `verify_distribution` 의 스캔 루트는 빌드 평면이었다 — **어느 게이트에도 관할이 없어서**
    운영자 NAS 절대경로를 담은 21개 파일이 포크 5개를 가진 공개 원격까지 갔다.

    처방은 규칙 하나를 더 좁히는 것이 아니라 **소유를 만드는 것**이다. 등록부가 루트 표면의 단일
    권위이고, 이 술어는 세 방향을 대조한다:

      ① 인덱스의 루트 항목 ⊆ 등록부(tracked·either)  — 새 추적 산출물이 루트에 생기면 RED
      ② 등록부의 tracked 항목 ⊆ 인덱스               — 뼈대가 사라지면 RED(무력화 검출)
      ③ tombstone 이름은 인덱스에 없다               — 폐지된 거처가 되살아나면 RED

    ④ 워킹트리는 **경고**로만 본다(FAIL ✗): 비추적 로컬 도구가 루트에 디렉터리를 만드는 것은
    운영자 자유이며, 그것을 차단하면 가드가 정상 상태를 상시 RED 로 만든다. 배포에 실리는 것은
    인덱스이므로 집행은 ①~③ 이 한다.
    """
    root = REPO_ROOT if root is None else root
    if not _is_canonical_repo(root):
        return

    doc = _load_root_registry(root)
    declared = {e["name"]: e for e in doc["entries"]
                if isinstance(e, dict) and isinstance(e.get("name"), str)}
    trackable = {n for n, e in declared.items() if e.get("git") in ("tracked", "either")}
    must_exist = {n for n, e in declared.items() if e.get("git") == "tracked"}

    tracked = list(_tracked_paths(root))
    _require(tracked, f"tripwire7 saw an empty index under {root} -- the comparison would be vacuous")
    index_roots = _root_names_of(tracked)

    stray = sorted(index_roots - trackable)
    _require(not stray,
             f"tracked root entries absent from {_ROOT_REGISTRY_REL} ({len(stray)}): {stray[:20]} "
             f"-- an unlisted root artifact is a placement error, not a registry addition (D3)")

    vanished = sorted(must_exist - index_roots)
    _require(not vanished,
             f"registry declares these root entries tracked but the index has none: {vanished}")

    tombstoned = []
    for tomb in doc.get("tombstones", []) or []:
        if not isinstance(tomb, dict):
            continue
        name = tomb.get("name")
        if not isinstance(name, str):
            continue
        hits = sorted(n for n in index_roots if fnmatch.fnmatch(n, name))
        tombstoned += [f"{n} (retired {tomb.get('retired_utc')} -> {tomb.get('successor')})" for n in hits]
    _require(not tombstoned,
             f"retired root locations are tracked again ({len(tombstoned)}): {tombstoned[:20]}")


def _test_root_registry_predicate() -> None:
    """tripwire ⑦ 의 커널을 단위로 고정한다(라이브 트리 불요 · 양성 입력이 실제로 발화하는지).

    라이브 트리는 깨끗할 때 아무것도 증명하지 않는다 — 통과가 "검출력이 있다" 를 뜻하려면 양성이
    발화해야 한다(역-오라클 회피).
    """
    _require(_root_names_of(["a/b/c", "x.md", "campaigns/_template/t.json"]) == {"a", "x.md", "campaigns"},
             "root-name derivation must cut at the first path segment")
    _require(fnmatch.fnmatch("config.camp1-20b-0180.yaml", "config.camp1-*.yaml"),
             "tombstone globs must match the escaped campaign variants that caused the leak")
    _require(not fnmatch.fnmatch("configs", "config*.yaml"),
             "the tombstone glob must not swallow the tracked `configs/` directory")
    if _is_canonical_repo(REPO_ROOT):
        doc = _load_root_registry(REPO_ROOT)
        names = {e.get("name") for e in doc["entries"]}
        _require({"campaigns", "CLAUDE.md", ".claude"} <= names,
                 f"root registry looks wrong: {sorted(names)[:10]}")
        _require(any(t.get("name") == "tasks" for t in doc.get("tombstones", [])),
                 "the retired `tasks/` location must stay recorded as a tombstone")


# ── tripwire ⑧ — 브랜치 헌법 2계층 (2026-09-12 신설 · plan_26091210 · BRANCH_CONSTITUTION_LAYERING)

#: 공통층 산문에 들어서는 안 되는 **한쪽 토폴로지 전용 어휘**. 닫힌 목록이라 변경 시 리뷰가 강제된다
#: (`workflow.md` §4종 안티패턴 판정표의 하드코딩 **정당** 칸 = tripwire).
#:
#: 대소문자를 구분한다 — `Ray 워커`(멀티의 sub 정체)와 `ray 워커`(양 토폴로지 공통 교훈 문장)는
#: 다른 말이고, 무시하면 후자가 위양성으로 잡힌다(2026-09-12 실측).
#: 비교·대조 문장("왜 두 토폴로지가 다른가")은 애초에 공통층의 시민이므로 이 목록에 넣지 않는다.
_TOPOLOGY_ONLY_VOCABULARY = {
    "multi": ("Ray 워커", "ray-worker", "집단 연산", "빌드킷 배달", "동일 ABI",
              "슬레이브", "양노드", "쌍노드"),
    "single": ("a2a-agent", "A2A 원격 에이전트", "위임 셀", "push-attestation"),
}

#: 면제 — (파일, 어휘, 그 줄의 고유 부분문자열, 사유). **닫힌 목록이고 사유가 없으면 등재할 수 없다.**
#: 면제가 생기는 유일한 정당 사유는 *그 문장을 고칠 수 없다* 는 외부 제약이다.
_TOPOLOGY_VOCABULARY_EXEMPTIONS = (
    (".claude/rules/workflow.md", "양노드", "clean 빌드(양노드) → 스모크",
     "policy_registry.py 의 ARCH_WALL_VARIANT_LADDER 사다리 검사가 이 문구를 verbatim 으로 "
     "요구한다 — 고치면 정책 검증이 깨진다. 어휘는 남지만 그 문장은 사다리 순서 서술이라 "
     "양 토폴로지가 함께 읽는다."),
)


def _load_topology_parity(root: Path):
    """4자일치 술어를 소유자에게서 적재한다(규약 문자열을 여기서 두 번째로 적지 않는다)."""
    path = root / ".claude/policies/runtime/topology_parity.py"
    if not path.is_file():
        return None
    spec = importlib.util.spec_from_file_location("_runtime_selftest_topology_parity", path)
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _common_layer_prose(root: Path) -> list[Path]:
    """공통층 산문 = 헌법 본문 + rules 최상위 `.md` 에서 **특화층을 뺀 것**.

    특화층 식별은 접미사 규약 하나로 한다(파일별 손등록 금지 — 손등록 목록은 새 파일이 생길 때마다
    조용히 늦는다. `sync_branches.sh` allowlist 가 같은 형태로 네 번 침묵 누락을 냈다).
    """
    suffix = ".topology.md"
    out = [root / "CLAUDE.md"]
    rules = root / ".claude/rules"
    if rules.is_dir():
        out += sorted(p for p in rules.glob("*.md") if not p.name.endswith(suffix))
    return [p for p in out if p.is_file()]


def _vocabulary_offenders(root: Path) -> list[str]:
    """공통층 산문에서 토폴로지 전용 어휘를 찾는다. 면제는 줄 단위로만 적용된다."""
    offenders: list[str] = []
    for path in _common_layer_prose(root):
        rel = str(path.relative_to(root))
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (OSError, UnicodeDecodeError):
            continue
        for topo, terms in _TOPOLOGY_ONLY_VOCABULARY.items():
            for term in terms:
                for lineno, line in enumerate(lines, 1):
                    if term not in line:
                        continue
                    if any(e_rel == rel and e_term == term and e_frag in line
                           for e_rel, e_term, e_frag, _why in _TOPOLOGY_VOCABULARY_EXEMPTIONS):
                        continue
                    offenders.append(f"{rel}:{lineno} '{term}' ({topo} 전용)")
    return offenders


def _test_topology_layer_parity(root: Path | None = None) -> None:
    """tripwire ⑧ — 헌법 2계층이 서 있고, 브랜치가 나머지 셋과 어긋나지 않는다.

    ★ 왜 tripwire 인가(2026-09-11 실측): 헌법은 "브랜치로 토폴로지를 추론하지 않는다"고 선언해
    왔는데 **그 선언을 집행하는 실행자가 0** 이었고, 반대 방향으로 파생하는 코드는 13곳이었다.
    그래서 멀티 캠페인 14셀이 `single-node` 체크아웃에서 돌았고 울린 트립와이어는 사람뿐이었다.

    세 가지를 본다:
      ① **4자일치** — 판정은 `topology_parity.py` 가 단독 소유한다(여기서 규칙을 복제하지 않는다).
      ② **공통층 어휘** — 한쪽 토폴로지에서만 참인 문장이 양 브랜치가 읽는 자리에 있으면 FAIL.
         이것이 오분류의 **2차 방어**다: 사람이 분류표를 잘못 승인해도 반대 브랜치에서 여기서 걸린다.
      ③ **교차검증** — 경로 규약 문자열이 술어(python)와 동기화(bash) 두 자리에 같은 형태로 있는가.
         정적 파일끼리는 한쪽이 다른 쪽을 생성할 수 없으므로 교차검증이 차선이다(선례: BAND2_TOP ↔ .gitignore).
    """
    root = REPO_ROOT if root is None else root
    parity = _load_topology_parity(root)
    _require(parity is not None,
             "topology_parity.py 가 없다 — 4자일치 술어의 소유자가 사라지면 이 tripwire 는 "
             "판정할 근거가 없다")

    res = parity.evaluate(root)
    _require(res["verdict"] == "PASS",
             "topology parity RED (" + ", ".join(res["reason_codes"]) + "): "
             + " · ".join(res["reasons"]))

    offenders = _vocabulary_offenders(root)
    _require(not offenders,
             f"공통층 산문에 토폴로지 전용 어휘가 있다 ({len(offenders)}건) — 그 문장은 특화층으로 "
             f"가거나 양 토폴로지를 함께 말하도록 고쳐야 한다: {offenders[:12]}")

    sync = root / ".claude/skills/upstream-version-watch/scripts/sync_branches.sh"
    if sync.is_file():
        try:
            sync_src = sync.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            sync_src = ""
        _require(parity.LAYER_EXCLUDE_PATHSPEC in sync_src,
                 f"동기화가 특화층을 제외하지 않는다 — {parity.LAYER_EXCLUDE_PATHSPEC!r} 리터럴이 "
                 "sync_branches.sh 에 없다. 이 두 자리가 갈라지면 다음 sync 가 특화 파일을 "
                 "반대 브랜치로 실어 두 브랜치를 다시 같게 만든다")


def _test_topology_layer_parity_predicate() -> None:
    """tripwire ⑧ 의 커널을 격리 픽스처로 고정한다 — **양성이 실제로 발화하는지**.

    라이브 트리는 깨끗할 때 아무것도 증명하지 않는다(역-오라클 회피). 여기서 만드는 불일치는
    2026-09-11 사고의 형태 그대로다: 브랜치 `multi-node` · 특화 헤더 `single`.
    """
    parity = _load_topology_parity(REPO_ROOT)
    _require(parity is not None, "topology_parity.py 를 적재하지 못했다")

    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        subprocess.run(["git", "init", "-q", "-b", "multi-node", str(root)],
                       check=True, capture_output=True, timeout=60)
        rules = root / ".claude/rules"
        rules.mkdir(parents=True)
        spec_file = rules / "strategy.topology.md"

        # 양성 ① — 헤더가 브랜치와 어긋난다(사고의 형태)
        spec_file.write_text("# s\n\n**topology: single**\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(root), "add", "-A"], check=True,
                       capture_output=True, timeout=60)
        res = parity.evaluate(root)
        _require("LAYER_HEADER_MISMATCH" in res["reason_codes"],
                 f"브랜치 multi-node ↔ 헤더 single 이 발화하지 않았다: {res['reason_codes']}")

        # 음성 — 헤더를 브랜치에 맞추면 통과한다(비추적 다리는 absent 로 빠진다)
        spec_file.write_text("# s\n\n**topology: multi**\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(root), "add", "-A"], check=True,
                       capture_output=True, timeout=60)
        res = parity.evaluate(root)
        _require(res["verdict"] == "PASS", f"일치 상태가 PASS 가 아니다: {res['reasons']}")
        _require(res["legs"]["manifest"]["status"] == "absent"
                 and res["legs"]["campaign"]["status"] == "absent",
                 "비추적 다리의 부재는 absent 로 표시돼야 한다(위반이 아니다) — "
                 f"{res['legs']['manifest']['status']}/{res['legs']['campaign']['status']}")
        _require(res["legs_checked"] == 2,
                 f"3자·2자 판정을 4자처럼 보고하면 안 된다: legs_checked={res['legs_checked']}")

        # 양성 ② — 추적 특화 파일이 0개면 RED(추적 입력의 부재는 위반이다)
        subprocess.run(["git", "-C", str(root), "rm", "-q", "--cached",
                        ".claude/rules/strategy.topology.md"], check=True,
                       capture_output=True, timeout=60)
        res = parity.evaluate(root)
        _require("LAYER_ABSENT" in res["reason_codes"],
                 f"특화 파일 0개가 발화하지 않았다: {res['reason_codes']}")

    # 어휘 스캐너의 양성·음성
    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        (root / ".claude/rules").mkdir(parents=True)
        (root / "CLAUDE.md").write_text("# c\n서브는 Ray 워커다.\n", encoding="utf-8")
        _require(_vocabulary_offenders(root),
                 "공통층의 멀티 전용 어휘가 발화하지 않았다")
        (root / "CLAUDE.md").write_text("# c\n서브의 정체는 토폴로지가 정한다.\n", encoding="utf-8")
        _require(not _vocabulary_offenders(root), "음성대조 실패 — 어휘가 없는데 발화했다")
        # 특화층은 스캔 대상이 아니다(거기 있는 것이 정상이다)
        (root / ".claude/rules/strategy.topology.md").write_text(
            "**topology: multi**\n서브는 Ray 워커다.\n", encoding="utf-8")
        _require(not _vocabulary_offenders(root),
                 "특화층 파일이 공통층 스캔에 들어왔다 — 접미사 규약이 안 먹었다")
        # 면제는 줄 단위로만 듣는다
        _require(any(e[0] == ".claude/rules/workflow.md" for e in _TOPOLOGY_VOCABULARY_EXEMPTIONS),
                 "면제 목록이 비었다 — verbatim 잠금 문장에 대한 면제가 사라지면 라이브가 RED 가 된다")


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
    # 자체시험 블록은 ETA·열 워치독 둘에만 있다. 협역 워치독의 --self-test(2026-09-23 · N3 신설)는
    # pgid 표적 모드와 컨테이너 모드 회귀를 보며 이 술어 블록을 복제하지 않는다 — 그 빈자리는 술어
    # 본문 parity 가 덮는다(같은 글자면 같은 판정이다). 실행자는 verify_distribution 이다.
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


def _test_no_revived_antipatterns(root: Path) -> None:
    """⑥ ③ 단계에서 **제거한 과적합 형태**가 스테이징된 변경에 되돌아왔는지(2026-09-05 · 3-13).

    왜 전수 인벤토리가 아니라 닫힌 목록인가: 하네스 전체를 훑으면 3,900 히트가 나오고 그 대부분은
    정당이다(픽스처·진단·국소 상수). 총량을 게이트로 쓰면 그 게이트 자체가 과적합이며, 그것이
    이번 감사가 지목한 문제였다. 여기서 막는 것은 ③ 이 실제로 제거한 9가지 형태뿐이고, 검사 범위는
    **스테이징된 파일**이라 병목 예산 안에 든다. 면제는 같은 줄의 `antipattern-ok:` 표식으로만 된다.
    """
    scanner = root / ".claude" / "policies" / "runtime" / "antipattern_scan.py"
    if not scanner.is_file():
        raise RuntimeSelftestFailure(
            "antipattern_scan.py 가 없다 — 제거 tripwire 의 실행자가 사라졌다: %s" % scanner)
    proc = subprocess.run([sys.executable, str(scanner), "--root", str(root), "--tripwire"],
                          capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeSelftestFailure(
            "제거한 안티패턴이 되돌아왔다(rc=%s):\n%s" % (proc.returncode, proc.stderr.strip()[-1200:]))

# ─────────────────────────────────────────────────────────────────────────────
# tripwire ⑨ — 기초층 → 스킬 간선의 닫힌 목록 (plan_26093022 §4 · 차단 검사 ②)
#
# 기초층(`.claude/policies/**`)이 스킬 경로에 닿는 자리는 모두 원장 `base_skill_edges.json` 에 이유와 함께
# 등재돼 있어야 한다. 키는 **줄 번호가 아니라 (호출자 파일, 스킬 대상 경로)** 다 — 줄이 움직일 때마다 원장이
# 깨지면 tripwire 가 소음이 된다. 목록은 현 상태 박제가 아니라 **새 간선을 막는 문**이다(4종 안티패턴 표
# "하드코딩" 정당 칸 = tripwire): 새 간선 → RED, 사라진 간선(원장에만 남음) → RED(원장이 화석이 되지 않게).
# `branch_layer_ledger.json` 은 append-only 이력이라 옛 경로를 그대로 들고 있어야 하므로 판정 대상 밖이다.
# ─────────────────────────────────────────────────────────────────────────────
_BASE_SKILL_EDGES_REL = ".claude/policies/base_skill_edges.json"
_BASE_SKILL_EDGES_EXEMPT = frozenset({_BASE_SKILL_EDGES_REL, ".claude/policies/branch_layer_ledger.json"})
_BASE_SKILL_EDGES_SUFFIXES = (".py", ".json", ".yaml", ".yml", ".md")
_BASE_SKILL_EDGE_PHASES = frozenset({"runtime_operational", "verification_plane"})
_SKILL_SLASH_RE = re.compile(r"(?:(?<=\.claude/)|(?<![\w./]))skills/([\w*\-{}]+)((?:/[\w.*\-{}]+)*)")
_SKILL_JOIN_RE = re.compile(r"""["']skills["']\s*([,/])\s*["']([\w\-]+)["']((?:\s*[,/]\s*["'][\w.\-]+["'])*)""")


def _skill_edge_targets(line: str) -> set[str]:
    out = set()
    for m in _SKILL_SLASH_RE.finditer(line):
        out.add((m.group(1) + m.group(2)).rstrip("./"))
    for m in _SKILL_JOIN_RE.finditer(line):
        rest = re.findall(r"""["']([\w.\-]+)["']""", m.group(3))
        out.add("/".join([m.group(2), *rest]))
    return out


def scan_base_skill_edges(root: Path) -> set[tuple[str, str]]:
    """(caller, skill-target) 쌍 전수 — 추적된 기초층 파일만 읽는다."""
    listed = subprocess.run(["git", "-C", str(root), "ls-files", "-z", "--", ".claude/policies"],
                            capture_output=True, check=False).stdout.decode("utf-8", "surrogateescape")
    found: set[tuple[str, str]] = set()
    for rel in sorted(x for x in listed.split("\0") if x):
        if rel in _BASE_SKILL_EDGES_EXEMPT or not rel.endswith(_BASE_SKILL_EDGES_SUFFIXES):
            continue
        path = root / rel
        if not path.is_file():
            continue
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            if "skills" in line:
                for target in _skill_edge_targets(line):
                    found.add((rel, target))
    return found


def _base_skill_edge_violations(root: Path) -> list[str]:
    ledger_path = root / _BASE_SKILL_EDGES_REL
    if not ledger_path.is_file():
        return [f"{_BASE_SKILL_EDGES_REL} 부재 — 기초층→스킬 간선의 닫힌 목록이 없다"]
    data = json.loads(ledger_path.read_text(encoding="utf-8"))
    listed: set[tuple[str, str]] = set()
    problems = []
    for row in data.get("edges", []):
        key = (row.get("caller"), row.get("callee"))
        if row.get("phase") not in _BASE_SKILL_EDGE_PHASES or not str(row.get("reason") or "").strip():
            problems.append(f"원장 행에 phase/reason 이 없다: {key}")
        listed.add(key)
    found = scan_base_skill_edges(root)
    for caller, callee in sorted(found - listed)[:12]:
        problems.append(f"미등재 간선 {caller} → skill:{callee} (이유를 달아 {_BASE_SKILL_EDGES_REL} 에 등재하거나 간선을 없앤다)")
    for caller, callee in sorted(listed - found)[:12]:
        problems.append(f"사라진 간선이 원장에 남았다 {caller} → skill:{callee} (원장 행을 지운다)")
    return problems


def _test_base_skill_edges(root: Path | None = None) -> None:
    root = REPO_ROOT if root is None else root
    if not _is_canonical_repo(root):
        return
    problems = _base_skill_edge_violations(root)
    _require(not problems, "tripwire⑨ base_to_skill_edges: " + " | ".join(problems))


def _test_base_skill_edges_predicate() -> None:
    """⑨ 의 판정 자체를 음성대조한다 — 미등재 간선·원장 화석·phase 결손을 각각 잡는지.

    픽스처 문자열은 조립해서 만든다 — 리터럴로 적으면 이 검사 자신이 기초층→스킬 간선이 된다."""
    sk = "skill" + "s"
    _require(_skill_edge_targets(f'x = ".claude/{sk}/wiki-desk/scripts/doc_naming.py"') == {"wiki-desk/scripts/doc_naming.py"},
             "slash literal target")
    _require(_skill_edge_targets(f'os.path.join(REPO, ".claude", "{sk}", "tn", "scripts", "a.py")') == {"tn/scripts/a.py"},
             "join-style target")
    _require(not _skill_edge_targets("mentions my-skills/ and reskills/x"), "word-embedded 'skills/' must not match")
    with tempfile.TemporaryDirectory() as d:
        r = Path(d)
        subprocess.run(["git", "init", "-q", str(r)], check=True)
        (r / ".claude/policies/runtime").mkdir(parents=True)
        (r / ".claude/policies/runtime/a.py").write_text(f'P = ".claude/{sk}/s1/scripts/x.py"\n', encoding="utf-8")
        ledger = {"edges": [{"caller": ".claude/policies/runtime/a.py", "callee": "s1/scripts/x.py",
                             "phase": "verification_plane", "reason": "fixture"}]}
        (r / _BASE_SKILL_EDGES_REL).write_text(json.dumps(ledger), encoding="utf-8")
        subprocess.run(["git", "-C", str(r), "add", "-A"], check=True)
        _require(_base_skill_edge_violations(r) == [], "listed edge must pass")
        (r / ".claude/policies/runtime/a.py").write_text(
            f'P = ".claude/{sk}/s1/scripts/x.py"\nQ = ".claude/{sk}/s2/scripts/y.py"\n', encoding="utf-8")
        v = _base_skill_edge_violations(r)
        _require(len(v) == 1 and "미등재" in v[0] and "s2/scripts/y.py" in v[0], f"new edge must be RED: {v}")
        (r / ".claude/policies/runtime/a.py").write_text("P = 1\n", encoding="utf-8")
        v = _base_skill_edge_violations(r)
        _require(len(v) == 1 and "사라진" in v[0], f"stale ledger row must be RED: {v}")
        ledger["edges"][0]["reason"] = ""
        (r / _BASE_SKILL_EDGES_REL).write_text(json.dumps(ledger), encoding="utf-8")
        (r / ".claude/policies/runtime/a.py").write_text(f'P = ".claude/{sk}/s1/scripts/x.py"\n', encoding="utf-8")
        v = _base_skill_edge_violations(r)
        _require(any("phase/reason" in x for x in v), f"reasonless row must be RED: {v}")


# ─────────────────────────────────────────────────────────────────────────────
# tripwire ⑩ — terraforming_node 라우터 결속 (plan_26093022 · 차단 검사 ①·④)
#
# ① §앵커·references 경로 0-dangling: 추적물이 `terraforming_node … SKILL.md §x` 로 인용하는 번호는 라우터
#    SKILL.md 에 토큰으로 남아 있어야 하고(§번호 = 안정 식별자), `terraforming_node/references/…md` 와 스킬 안의
#    `references/…md` 인용은 실재해야 한다. 본문이 references 로 내려가도 인용이 썩지 않게 하는 문이다.
# ④ SKILL.md 리터럴 결속: 기초층 코드가 terraforming SKILL.md 를 읽어 `"…" in 변수` 로 요구하는 리터럴을
#    **AST 에서 파생**해(손목록 ✗) 라우터에 실재하는지 본다(`not in` 은 부재). 술어는 harness 에서만 돌지만
#    이 검사는 pre-commit 에서 돈다 — 라우터를 줄이다 결속 문장을 지우면 커밋 전에 막힌다.
# ─────────────────────────────────────────────────────────────────────────────
_TN_SKILL_REL = ".claude/skills/terraforming_node/SKILL.md"
_TN_DIR_REL = ".claude/skills/terraforming_node/"
_ANCHOR_SCAN_EXCLUDE_PREFIX = ("docs/", "seed/")
_ANCHOR_SCAN_EXCLUDE_FILES = frozenset({".claude/policies/branch_layer_ledger.json"})
_TN_ANCHOR_RE = re.compile(r"terraforming_node(?:/SKILL\.md)?`?\s*(?:SKILL\.md)?`?\s*\**\s*§\s*([0-9]+S?(?:\.[0-9]+)*[a-z]?)")
_TN_REF_ABS_RE = re.compile(r"terraforming_node/references/([\w./\-]+?\.md)")
_TN_REF_REL_RE = re.compile(r"(?<![\w/.])references/([\w./\-]+?\.md)")


def _anchor_token_present(anchor: str, text: str) -> bool:
    return re.search(r"(?<![\d.])" + re.escape(anchor) + r"(?!\.?\d)", text) is not None


def _router_binding_violations(root: Path) -> list[str]:
    skill_path = root / _TN_SKILL_REL
    if not skill_path.is_file():
        return [f"{_TN_SKILL_REL} 부재"]
    skill = skill_path.read_text(encoding="utf-8")
    # 후보는 git grep 으로 먼저 추린다(추적물 전수 읽기는 1초 예산을 넘는다 · 2.3s 실측).
    grep = subprocess.run(["git", "-C", str(root), "grep", "-lz", "-I", "-e", "terraforming_node", "--",
                           ".", ":!docs", ":!seed"], capture_output=True, check=False)
    listed = grep.stdout.decode("utf-8", "surrogateescape")
    skill_files = subprocess.run(["git", "-C", str(root), "ls-files", "-z", "--", _TN_DIR_REL],
                                 capture_output=True, check=False).stdout.decode("utf-8", "surrogateescape")
    problems: list[str] = []
    for rel in sorted({x for x in listed.split("\0") + skill_files.split("\0") if x}):
        if rel.startswith(_ANCHOR_SCAN_EXCLUDE_PREFIX) or rel in _ANCHOR_SCAN_EXCLUDE_FILES:
            continue
        path = root / rel
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        in_skill = rel.startswith(_TN_DIR_REL)
        if "terraforming_node" not in text and not (in_skill and "references/" in text):
            continue
        for line in text.splitlines():
            for m in _TN_ANCHOR_RE.finditer(line):
                a = m.group(1)
                if not _anchor_token_present(a, skill):
                    problems.append(f"① dangling §앵커 {rel}: SKILL.md §{a}")
            refs = [m.group(1) for m in _TN_REF_ABS_RE.finditer(line)]
            if in_skill and rel.endswith(".md"):
                refs += [m.group(1) for m in _TN_REF_REL_RE.finditer(line)]
            for r in refs:
                if not (root / _TN_DIR_REL / "references" / r).is_file():
                    problems.append(f"① dangling references 경로 {rel}: references/{r}")
    problems += _skill_literal_binding_violations(root, skill)
    return sorted(set(problems))


def _skill_literal_binding_violations(root: Path, skill: str) -> list[str]:
    listed = subprocess.run(["git", "-C", str(root), "ls-files", "-z", "--", ".claude/policies"],
                            capture_output=True, check=False).stdout.decode("utf-8", "surrogateescape")
    out: list[str] = []
    for rel in sorted(x for x in listed.split("\0") if x.endswith(".py")):
        src = (root / rel).read_text(encoding="utf-8")
        if "terraforming_node/SKILL.md" not in src:
            continue
        tree = ast.parse(src)
        lines = src.splitlines()

        def _text(node) -> str:
            return "\n".join(lines[node.lineno - 1:node.end_lineno])

        for fn in (n for n in ast.walk(tree) if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef))):
            if "terraforming_node/SKILL.md" not in _text(fn):
                continue
            names: set[str] = set()
            for node in ast.walk(fn):
                if isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name):
                    seg = _text(node.value)
                    if "terraforming_node/SKILL.md" in seg or any(
                            re.search(rf"\b{re.escape(v)}\.read_text\b", seg) for v in names):
                        names.add(node.targets[0].id)
            if not names:
                continue
            for node in ast.walk(fn):
                if (isinstance(node, ast.Compare) and len(node.ops) == 1
                        and isinstance(node.left, ast.Constant) and isinstance(node.left.value, str)
                        and isinstance(node.comparators[0], ast.Name) and node.comparators[0].id in names):
                    lit = node.left.value
                    if isinstance(node.ops[0], ast.In) and lit not in skill:
                        out.append(f"④ 결속 리터럴 부재 {rel}:{node.lineno}: {lit!r}")
                    if isinstance(node.ops[0], ast.NotIn) and lit in skill:
                        out.append(f"④ 금지 리터럴 존재 {rel}:{node.lineno}: {lit!r}")
    return out


def _test_router_bindings(root: Path | None = None) -> None:
    root = REPO_ROOT if root is None else root
    if not _is_canonical_repo(root):
        return
    problems = _router_binding_violations(root)
    _require(not problems, "tripwire⑩ ghost_section_anchors/skill_literal_binding: " + " | ".join(problems[:12]))


def _test_router_bindings_predicate() -> None:
    """⑩ 음성대조 — 사라진 §번호·없는 references 경로·지워진 결속 리터럴을 각각 잡는지."""
    _require(_anchor_token_present("2.7", "## 2.7 노드") and not _anchor_token_present("2.7", "### 2.7.0 x")
             and _anchor_token_present("2.7.7a", "→ 2.7.7a 턴제"), "anchor token boundaries")
    tn = "terraforming" + "_node"
    with tempfile.TemporaryDirectory() as d:
        r = Path(d)
        subprocess.run(["git", "init", "-q", str(r)], check=True)
        (r / _TN_DIR_REL / "references").mkdir(parents=True)
        (r / _TN_DIR_REL / "references" / "a.md").write_text("x\n", encoding="utf-8")
        (r / _TN_SKILL_REL).write_text("## 2.6 호스트\n### 2.7.1 평면\n→ `references/a.md`\n", encoding="utf-8")
        (r / "cite.md").write_text(f"`{tn}` SKILL.md §2.7.1 · {tn}/references/a.md\n", encoding="utf-8")
        (r / ".claude/policies/runtime").mkdir(parents=True)
        (r / ".claude/policies/runtime/p.py").write_text(
            f"def f():\n    s = _read('.claude/{'skill' + 's'}/{tn}/SKILL.md')\n    _require('## 2.6 호스트' in s)\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(r), "add", "-A"], check=True)
        _require(_router_binding_violations(r) == [], f"clean fixture must pass: {_router_binding_violations(r)}")
        (r / "cite.md").write_text(f"`{tn}` SKILL.md §2.7.9 · {tn}/references/zz.md\n", encoding="utf-8")
        v = _router_binding_violations(r)
        _require(any("§2.7.9" in x for x in v) and any("zz.md" in x for x in v), f"dangling cites must be RED: {v}")
        (r / "cite.md").write_text("ok\n", encoding="utf-8")
        (r / _TN_SKILL_REL).write_text("### 2.7.1 평면\n", encoding="utf-8")
        v = _router_binding_violations(r)
        _require(any("④" in x and "2.6" in x for x in v), f"removed bound literal must be RED: {v}")


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
        _test_no_revived_antipatterns(root)        # ⑥ 3-13 — ③ 이 제거한 형태의 부활 차단
        _test_root_surface_registry(root)          # ⑦ plan_26090616 — 루트 표면에 관할을 만든다
        _test_topology_layer_parity(root)          # ⑧ plan_26091210 — 브랜치 헌법 2계층·4자일치
        _test_base_skill_edges(root)               # ⑨ plan_26093022 — base_to_skill_edges(②) 닫힌 목록
        _test_router_bindings(root)                # ⑩ plan_26093022 — ghost_section_anchors(①) · skill_literal_binding(④)
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
             "retired-mechanism prose / duplicate certificates / deployed-artifact PII / "
             "revived antipatterns / root-surface registry / branch-constitution layering / "
             "base-to-skill edge closed list); "
             "1s budget, diagnostics on stderr")
    args = parser.parse_args(argv)  # argv=None -> argparse reads sys.argv[1:]

    if args.tripwires_only:
        return run_tripwires()

    _test_no_production_asserts()
    _test_nested_worktree_isolation()
    _test_completion_gate()
    _test_promotion_rubric_carrier()
    _test_certificate_waiver_by_authority()
    _test_certificate_run_resolution()
    _test_hint_binding_source()
    _test_hint_cli_publication()
    _test_hint_cli_refute_paths()
    _test_hint_map_only_promotion()
    _test_hint_map_only_publication()
    _test_policy_and_evidence_lifecycle()
    _test_agent_control_owner_selftest()
    _test_execution_approval_authorization()
    _test_duplicate_certificate_predicate()
    _test_deployed_pii_predicate()
    _test_root_registry_predicate()
    _test_topology_layer_parity_predicate()
    _test_watchdog_target_predicate_parity()
    _test_base_skill_edges_predicate()
    _test_router_bindings_predicate()
    # tripwire 6종은 축약 진입점과 **같은 함수**를 돈다 — 두 벌로 갈라지면 갈라진 쪽이 조용히
    # 늦는다(선례 3건). 전체 실행에서도 반드시 검사한다.
    # 비-정본 저장소에서 그 단언들이 no-op 이 되는 것은 `run_tripwires` 와 **같은 정상 경로**이며,
    # 여기서도 같은 SKIPPED 한 줄로 눈에 보이게 한다(침묵 no-op 금지).
    _announce_non_canonical(REPO_ROOT, "runtime_selftest")
    _test_no_backup_artifacts()
    _test_no_tracked_digest_rewrite()
    _test_no_retired_hash_mechanism_prose()
    _test_no_duplicate_certificates()
    _test_no_pii_in_deployed_artifacts()
    # ⑥⑦ 는 2026-09-06 에 이 목록에 편입했다 — 바로 위 주석이 "축약 진입점과 같은 함수를 돈다"
    # 라고 선언해 놓고 ⑥ 이 빠져 있었다(선언이 배선을 대체한 자리).
    _test_no_revived_antipatterns(REPO_ROOT)
    _test_root_surface_registry()
    _test_topology_layer_parity()
    _test_base_skill_edges()
    _test_router_bindings()
    for warning in _test_tripwire_executor_wiring():
        print(f"[runtime_selftest] WARN {warning}", file=sys.stderr)
    print("[runtime_selftest] PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
