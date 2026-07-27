#!/usr/bin/env python3
"""Constitution-owned minimal, tracked, fail-closed completion gate
(plan_26072506 Phase4 cycle2).

The gap this closes: no tracked runner or test contract previously invoked
`policy_registry.py verify --as-of` as part of a full correction-cycle --
only the CLI's own docstring mentioned it, never a caller. This script sequentially runs (1) the policy registry audit with an EXPLICIT --as-of,
(2) the constitution-owned runtime regression probe, and (3) the production predicate suite.
It exits 0 only if ALL stages pass. Later stages never start after an earlier failure.
completion outright, fail-closed.

stdlib-only (argparse/datetime/subprocess/sys) -- no PyYAML, no jsonschema, no requests.

Usage:
    python3 .claude/policies/runtime/harness_verify.py --as-of YYYY-MM-DD
        [--registry PATH] [--schema PATH] [--evidence-manifest PATH] [--repo-root PATH]

Exit codes:
    0  both stages passed.
    2  usage error (missing/malformed --as-of), OR the policy audit itself exited 2
       (schema/load-tier violation -- verbatim propagation of policy_registry.py's own exit code).
    1  the policy audit exited 1 (lifecycle-tier violation -- verbatim propagation).
    3  stage 1 passed but stage 2 (the production predicate suite) failed.
"""
from __future__ import annotations

import argparse
import datetime
import os
import site
import subprocess
import sys
import sysconfig
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
POLICY_REGISTRY = Path(__file__).resolve().parent / "policy_registry.py"
RUNTIME_SELFTEST = Path(__file__).resolve().parent / "runtime_selftest.py"
PREDICATES_DIR = REPO_ROOT / ".claude" / "policies" / "predicates"
CLAIM_PREDICATES = PREDICATES_DIR / "claim_predicates.py"


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
    """Under -S, expose only this interpreter's declared global dependency directories."""
    env = os.environ.copy()
    if sys.flags.no_site:
        declared = [*site.getsitepackages(), sysconfig.get_path("purelib"), sysconfig.get_path("platlib")]
        executable_env = Path(sys.executable).parent.parent
        if (executable_env / "pyvenv.cfg").is_file():
            version = f"python{sys.version_info.major}.{sys.version_info.minor}"
            declared.insert(0, str(executable_env / "lib" / version / "site-packages"))
        roots = list(dict.fromkeys(str(Path(p)) for p in declared if p and Path(p).is_dir()))
        prior = env.get("PYTHONPATH", "")
        env["PYTHONPATH"] = os.pathsep.join([*roots, *([prior] if prior else [])])
    return env


def _parse_args(argv: list) -> argparse.Namespace:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--as-of", required=True,
                     help="YYYY-MM-DD, required, no wall-clock default -- passed straight "
                          "through to policy_registry.py verify")

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


def run_predicate_suite() -> int:
    claim = subprocess.run([*_child_python(), str(CLAIM_PREDICATES)],
                           cwd=REPO_ROOT, env=_child_env())
    if claim.returncode != 0:
        return claim.returncode
    companion = subprocess.run(
        [*_child_python(), "-m", "unittest", "discover", "-s", str(PREDICATES_DIR),
         "-p", "*_predicate.py"], cwd=REPO_ROOT, env=_child_env())
    return companion.returncode


def run_runtime_selftest() -> int:
    result = subprocess.run([*_child_python(), str(RUNTIME_SELFTEST)],
                            cwd=REPO_ROOT, env=_child_env())
    return result.returncode


def main(argv: list | None = None) -> int:
    args = _parse_args(sys.argv[1:] if argv is None else argv)

    try:
        datetime.date.fromisoformat(args.as_of)
    except (ValueError, TypeError):
        print(f"[harness_verify] usage error: --as-of {args.as_of!r} is not a valid ISO-8601 date",
              file=sys.stderr)
        return 2

    print(f"[harness_verify] stage 1/3: policy_registry.py verify --as-of {args.as_of}")
    audit_exit = run_policy_audit(args)
    if audit_exit != 0:
        print(f"[harness_verify] BLOCKED: policy audit exited {audit_exit} -- correction-cycle "
              "completion refused before runtime/predicate probes run.", file=sys.stderr)
        return audit_exit

    print("[harness_verify] stage 2/3: constitution-owned runtime regression probe")
    runtime_exit = run_runtime_selftest()
    if runtime_exit != 0:
        print(f"[harness_verify] BLOCKED: runtime regression probe exited {runtime_exit}.", file=sys.stderr)
        return 4

    print("[harness_verify] stage 3/3: constitution-owned production predicates")
    predicates_exit = run_predicate_suite()
    if predicates_exit != 0:
        print(f"[harness_verify] BLOCKED: production predicate suite exited {predicates_exit}.", file=sys.stderr)
        return 3

    print("[harness_verify] PASS: policy audit, runtime regression, and production predicates green.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
