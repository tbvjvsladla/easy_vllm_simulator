#!/usr/bin/env python3
"""scripts/harness_verify.py -- minimal, tracked, fail-closed harness completion gate
(plan_26072506 Phase4 cycle2).

The gap this closes: no tracked runner or test contract previously invoked
`scripts/policy_registry.py verify --as-of` as part of a full harness E2E correction-cycle --
only the CLI's own docstring mentioned it, never a caller. This script sequentially runs (1) the
policy registry audit with an EXPLICIT --as-of (never wall-clock `today` -- the caller/test
supplies the date) and (2) the harness unittest suite, and only exits 0 if BOTH stages pass.
Stage 2 never even starts if stage 1 reports any violation -- a nonzero policy audit blocks
completion outright, fail-closed.

stdlib-only (argparse/datetime/subprocess/sys) -- no PyYAML, no jsonschema, no requests.

Usage:
    python3 scripts/harness_verify.py --as-of YYYY-MM-DD [--tests-dir tests/harness]
        [--registry PATH] [--schema PATH] [--evidence-manifest PATH] [--repo-root PATH]

Exit codes:
    0  both stages passed.
    2  usage error (missing/malformed --as-of), OR the policy audit itself exited 2
       (schema/load-tier violation -- verbatim propagation of policy_registry.py's own exit code).
    1  the policy audit exited 1 (lifecycle-tier violation -- verbatim propagation).
    3  stage 1 passed but stage 2 (the unittest suite) failed.
"""
from __future__ import annotations

import argparse
import datetime
import os
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
POLICY_REGISTRY = REPO_ROOT / "scripts" / "policy_registry.py"


def _child_python() -> list[str]:
    """Preserve security-relevant interpreter flags in child verification processes."""
    flags = []
    if sys.flags.isolated:
        flags.append("-I")
    if sys.flags.no_site:
        flags.append("-S")
    if sys.flags.optimize:
        flags.append("-" + "O" * sys.flags.optimize)
    if sys.flags.dont_write_bytecode:
        flags.append("-B")
    return [sys.executable, *flags]


def _child_env() -> dict[str, str]:
    """Under -S, expose only this interpreter environment's declared dependency directory."""
    env = os.environ.copy()
    if sys.flags.no_site:
        version = f"python{sys.version_info.major}.{sys.version_info.minor}"
        declared = Path(sys.executable).parent.parent / "lib" / version / "site-packages"
        if declared.is_dir():
            prior = env.get("PYTHONPATH", "")
            env["PYTHONPATH"] = str(declared) + (os.pathsep + prior if prior else "")
    return env


def _parse_args(argv: list) -> argparse.Namespace:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--as-of", required=True,
                     help="YYYY-MM-DD, required, no wall-clock default -- passed straight "
                          "through to policy_registry.py verify")
    ap.add_argument("--tests-dir", default="tests/harness")
    ap.add_argument("--registry", default=None, help="passthrough to policy_registry.py verify --registry")
    ap.add_argument("--schema", default=None, help="passthrough to policy_registry.py verify --schema")
    ap.add_argument("--evidence-manifest", default=None,
                     help="passthrough to policy_registry.py verify --evidence-manifest")
    ap.add_argument("--tracked-index", default=None,
                     help="passthrough to policy_registry.py verify --tracked-index")
    ap.add_argument("--claim-bindings", default=None,
                     help="passthrough to policy_registry.py verify --claim-bindings")
    ap.add_argument("--repo-root", default=None, help="passthrough to policy_registry.py verify --repo-root")
    return ap.parse_args(argv)


def run_policy_audit(args: argparse.Namespace) -> int:
    cmd = [*_child_python(), str(POLICY_REGISTRY), "verify", "--as-of", args.as_of]
    if args.registry:
        cmd += ["--registry", args.registry]
    if args.schema:
        cmd += ["--schema", args.schema]
    if args.evidence_manifest:
        cmd += ["--evidence-manifest", args.evidence_manifest]
    if args.tracked_index:
        cmd += ["--tracked-index", args.tracked_index]
    if args.claim_bindings:
        cmd += ["--claim-bindings", args.claim_bindings]
    if args.repo_root:
        cmd += ["--repo-root", args.repo_root]
    result = subprocess.run(cmd, cwd=REPO_ROOT, env=_child_env())
    return result.returncode


def run_test_suite(tests_dir: str) -> int:
    cmd = [*_child_python(), "-m", "unittest", "discover", "-s", tests_dir, "-p", "test_*.py"]
    result = subprocess.run(cmd, cwd=REPO_ROOT, env=_child_env())
    return result.returncode


def main(argv: list | None = None) -> int:
    args = _parse_args(sys.argv[1:] if argv is None else argv)

    try:
        datetime.date.fromisoformat(args.as_of)
    except (ValueError, TypeError):
        print(f"[harness_verify] usage error: --as-of {args.as_of!r} is not a valid ISO-8601 date",
              file=sys.stderr)
        return 2

    print(f"[harness_verify] stage 1/2: policy_registry.py verify --as-of {args.as_of}")
    audit_exit = run_policy_audit(args)
    if audit_exit != 0:
        print(f"[harness_verify] BLOCKED: policy audit exited {audit_exit} -- correction-cycle "
              "completion refused before the test suite even runs.", file=sys.stderr)
        return audit_exit

    print(f"[harness_verify] stage 2/2: unittest discover -s {args.tests_dir}")
    tests_exit = run_test_suite(args.tests_dir)
    if tests_exit != 0:
        print(f"[harness_verify] BLOCKED: harness test suite exited {tests_exit}.", file=sys.stderr)
        return 3

    print("[harness_verify] PASS: policy audit clean and harness suite green.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
