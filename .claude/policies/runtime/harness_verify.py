#!/usr/bin/env python3
"""Constitution-owned minimal, tracked, fail-closed completion gate
(plan_26072506 Phase4 cycle2).

The gap this closes: no tracked runner or test contract previously invoked
`policy_registry.py verify --as-of` as part of a full correction-cycle --
only the CLI's own docstring mentioned it, never a caller. This script sequentially runs (1) the policy registry audit with an EXPLICIT --as-of (this caller observes the live UTC date once per run unless the operator passes one),
(2) the constitution-owned runtime regression probe, and (3) the production predicate suite.
It exits 0 only if ALL stages pass. Later stages never start after an earlier failure.
completion outright, fail-closed.

stdlib-only (argparse/datetime/subprocess/sys) -- no PyYAML, no jsonschema, no requests.

Reference date (user directive 2026-11-09: no fixed operating dates): the pure validator `policy_registry.py` keeps its explicit `--as-of`
and reads no clock. THIS caller supplies it: by default `observe_as_of()` observes the actual UTC date ONCE per run, and the run prints the
as_of with its source, the observed UTC instant and the exact input that reproduces the criteria. `--as-of YYYY-MM-DD` is the explicit
caller input for a deterministic reproduction (recorded as such); there is no environment override and no default date.

Usage:
    python3 .claude/policies/runtime/harness_verify.py [--as-of YYYY-MM-DD]
        [--registry PATH] [--schema PATH] [--claim-bindings PATH] [--repo-root PATH]

Exit codes:
    0  both stages passed.
    2  usage error (malformed --as-of), OR the policy audit itself exited 2
       (schema/load-tier violation -- verbatim propagation of policy_registry.py's own exit code).
    1  the policy audit exited 1 (lifecycle-tier violation -- verbatim propagation).
    3  stage 1 passed but stage 2 (the production predicate suite) failed.
"""
from __future__ import annotations

import argparse
import datetime
import os
import re
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


def observe_as_of(explicit: str | None = None, now_utc: datetime.datetime | None = None) -> dict:
    """The CALLER-side reference date of a policy audit, observed once and recorded (mechanical plumbing, no policy logic).

    explicit (strict YYYY-MM-DD) -> that date, source `explicit-caller-input` (deterministic reproduction).
    otherwise -> the UTC date of one clock observation, source `observed-utc-clock` (`now_utc` exists so tests can inject the instant; it must be
    timezone-aware UTC). The UTC date never depends on the host's local timezone. Malformed input raises ValueError (no fallback, no guess)."""
    if explicit is not None:
        if not isinstance(explicit, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}", explicit):
            raise ValueError(f"--as-of {explicit!r} is not a YYYY-MM-DD date")
        datetime.date.fromisoformat(explicit)
        return {"as_of": explicit, "as_of_source": "explicit-caller-input", "observed_utc": None, "repro_as_of": explicit}
    observed = datetime.datetime.now(datetime.timezone.utc) if now_utc is None else now_utc
    if observed.tzinfo is None or observed.utcoffset() != datetime.timedelta(0):
        raise ValueError(f"observed instant {observed!r} is not timezone-aware UTC")
    as_of = observed.date().isoformat()
    return {"as_of": as_of, "as_of_source": "observed-utc-clock", "observed_utc": observed.isoformat(), "repro_as_of": as_of}


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
    ap.add_argument("--as-of", default=None,
                     help="YYYY-MM-DD explicit caller input for a deterministic reproduction; default = the live UTC date observed once "
                          "per run and recorded -- passed straight through to policy_registry.py verify (the validator itself reads no clock)")

    ap.add_argument("--registry", default=None, help="passthrough to policy_registry.py verify --registry")
    ap.add_argument("--schema", default=None, help="passthrough to policy_registry.py verify --schema")
    ap.add_argument("--claim-bindings", default=None,
                     help="passthrough to policy_registry.py verify --claim-bindings")
    ap.add_argument("--repo-root", default=None, help="passthrough to policy_registry.py verify --repo-root")
    return ap.parse_args(argv)


def run_policy_audit(args: argparse.Namespace, as_of: str) -> int:
    cmd = [*_child_python(), str(POLICY_REGISTRY), "verify", "--as-of", as_of]
    if args.registry:
        cmd += ["--registry", args.registry]
    if args.schema:
        cmd += ["--schema", args.schema]
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
        reference = observe_as_of(args.as_of)
    except (ValueError, TypeError):
        print(f"[harness_verify] usage error: --as-of {args.as_of!r} is not a valid ISO-8601 date",
              file=sys.stderr)
        return 2

    print(f"[harness_verify] reference date: as_of={reference['as_of']} source={reference['as_of_source']} "
          f"observed_utc={reference['observed_utc']} repro: --as-of {reference['repro_as_of']}")
    print(f"[harness_verify] stage 1/3: policy_registry.py verify --as-of {reference['as_of']}")
    audit_exit = run_policy_audit(args, reference["as_of"])
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
