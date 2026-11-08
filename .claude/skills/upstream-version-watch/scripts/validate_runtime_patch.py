#!/usr/bin/env python3
"""Cryptographically bind an ephemeral runtime patch to current model/topology inputs."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
from pathlib import Path

SCHEMA_VERSION = 1


def _sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _inside(path: Path, parent: Path) -> Path:
    resolved, root = path.resolve(), parent.resolve()
    try:
        resolved.relative_to(root)
    except ValueError as exc:
        raise ValueError(f"path escapes authoritative directory: {path}") from exc
    return resolved


def _context(args) -> tuple[Path, Path, dict]:
    patch = _inside(Path(args.patch), Path(args.config_dir))
    if not patch.is_file() or not patch.name.endswith("_patch.py"):
        raise ValueError("patch must be an existing configs/<model>_patch.py file")
    stem = patch.name.removesuffix("_patch.py")
    if not stem or any(ch not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-" for ch in stem):
        raise ValueError("invalid model stem")
    config_dir, env_dir = Path(args.config_dir), Path(args.env_dir)
    inputs = {}
    for candidate in (config_dir / f"{stem}.sh", config_dir / f"{stem}.yaml"):
        if candidate.is_file():
            inputs[candidate.name] = _sha(candidate)
    if not inputs:
        raise ValueError("runtime patch requires a current model .sh or .yaml input")
    env = env_dir / f".env.{stem}"
    if env.is_file():
        inputs[f"envs/{env.name}"] = _sha(env)
    resolution = Path(args.resolution)
    if not resolution.is_file():
        raise ValueError("canonical production resolution missing")
    # Parse as an object as well as hashing it: malformed/empty canonical state is never stampable.
    parsed = json.loads(resolution.read_text(encoding="utf-8"))
    if not isinstance(parsed, dict) or not parsed:
        raise ValueError("canonical production resolution must be a non-empty object")
    sidecar = config_dir / f"{stem}_patch.provenance.json"
    record = {
        "schema_version": SCHEMA_VERSION,
        "model_stem": stem,
        "topology": args.topology,
        "patch_sha256": _sha(patch),
        "canonical_resolution_sha256": _sha(resolution),
        "model_inputs_sha256": dict(sorted(inputs.items())),
    }
    return patch, sidecar, record


def stamp(args) -> int:
    _, sidecar, record = _context(args)
    sidecar.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix=sidecar.name + ".", dir=sidecar.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            json.dump(record, fh, ensure_ascii=False, sort_keys=True, indent=2)
            fh.write("\n")
        os.replace(tmp, sidecar)
    finally:
        if os.path.exists(tmp):
            os.unlink(tmp)
    print(sidecar)
    return 0


def verify(args) -> int:
    _, sidecar, expected = _context(args)
    if not sidecar.is_file():
        raise ValueError(f"runtime patch provenance missing: {sidecar}")
    actual = json.loads(sidecar.read_text(encoding="utf-8"))
    if actual != expected:
        raise ValueError("runtime patch provenance is stale or does not bind current topology/model inputs")
    print(f"PASS {sidecar}")
    return 0


def main(argv=None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=("stamp", "verify"))
    parser.add_argument("--patch", required=True)
    parser.add_argument("--topology", required=True, choices=("multi", "single"))
    parser.add_argument("--config-dir", required=True)
    parser.add_argument("--env-dir", required=True)
    parser.add_argument("--resolution", required=True)
    args = parser.parse_args(argv)
    try:
        return stamp(args) if args.action == "stamp" else verify(args)
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as exc:
        parser.exit(2, f"runtime-patch-provenance FAIL: {exc}\n")


if __name__ == "__main__":
    # Explicit native startup only; imported functions and dependency profile stay unchanged.
    import os as _native_os
    import sys as _native_sys
    from pathlib import Path as _NativePath
    _native_script = _NativePath(__file__).resolve()
    _native_root_value = _native_os.environ.get("EASY_VLLM_UPSTREAM_RUNTIME_ROOT")
    if _native_root_value is None:
        _native_runtime_root = _native_script.parent.parent / "rust/runtime"
    elif not _native_root_value:
        print("[validate_runtime_patch] ERROR explicit upstream runtime root is empty", file=_native_sys.stderr)
        raise SystemExit(2)
    else:
        _native_runtime_root = _NativePath(_native_root_value)
    _native_binary = _native_runtime_root / "target/release/validate_runtime_patch"
    if not _native_binary.is_file() or not _native_os.access(_native_binary, _native_os.X_OK):
        print("[validate_runtime_patch] ERROR normal Rust ELF missing/non-executable: " + str(_native_binary), file=_native_sys.stderr)
        raise SystemExit(2)
    with _native_binary.open("rb") as _native_file:
        _native_magic = _native_file.read(4)
    if _native_magic != b"\x7fELF":
        print("[validate_runtime_patch] ERROR normal Rust ELF invalid: " + str(_native_binary), file=_native_sys.stderr)
        raise SystemExit(2)
    _native_os.execv(str(_native_binary), [str(_native_script), *_native_sys.argv[1:]])
