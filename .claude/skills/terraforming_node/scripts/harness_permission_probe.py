#!/usr/bin/env python3
"""Bounded work-permission observations, executed INSIDE the requested native tool.

No serving, image build, remote git, sudo, credential output or policy changes. Existing
protected/build files are only opened O_WRONLY without truncate or write, then closed.
Actual writes/commits use disposable scratch; git metadata probes create only a unique
owned marker, never index/refs/config. This does not prove Docker-daemon confinement,
publication, model quality, native resume, or every possible protected-path operation.
"""
from __future__ import annotations

import argparse
import errno
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

MARKER = "HARNESS_PERMISSION_PROBE="
SCHEMA_VERSION = 1
PROTECTED_FILES = ("AGENTS.md", "CLAUDE.md", "Agent_Card.json",
                   ".claude/settings.local.json", ".codex/config.toml",
                   ".agents/skills/wiki-desk/SKILL.md")
PROTECTED_FILES_BY_ROLE = {
    "sub": PROTECTED_FILES,
    # Main owns canonical harness development, including provider/discovery files.
    # Keep the historical protected:* receipt names; they name normally protected
    # paths, not a universal denied outcome. The role decides the expected result.
    "main": (".codex/config.toml", ".agents/skills/wiki-desk/SKILL.md"),
}
BUILD_FILES = ("docker-compose.yaml", "requirements.txt")
DOCKERFILES = ("Dockerfile", "Dockerfile.source-build", "Dockerfile.source-build-upstage")


def protected_files(role: str):
    return PROTECTED_FILES_BY_ROLE[role]


def protected_file_expectations(role: str) -> dict[str, str]:
    """Role ownership, not a result packet, defines these write-open expectations."""
    return {path: "allowed" if role == "main" else "denied"
            for path in protected_files(role)}


def _command(argv, cwd):
    try:
        env = None
        if argv[0] == "git":
            # cwd is not a git boundary when ambient location/config variables exist.
            # Disable user/system git config for these bounded local probes as well.
            env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
            env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
                       GIT_OPTIONAL_LOCKS="0", GIT_TERMINAL_PROMPT="0")
        p = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, text=True, timeout=20)
        # Do not expose command stdout/stderr: local config/remote URLs can contain secrets.
        return p.returncode, p.stdout
    except (OSError, subprocess.TimeoutExpired):
        return None, ""


def _write_marker(directory: Path, nonce: str) -> str:
    """Exclusive owned marker; failed cleanup is a failure, never a successful write."""
    marker = directory / (".harness-permission-" + nonce)
    fd = None
    created = False
    try:
        fd = os.open(marker, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        created = True
        payload = nonce.encode()
        if os.write(fd, payload) != len(payload):
            return "failed"
        os.fsync(fd)
        os.close(fd)
        fd = None
        return "allowed" if marker.read_bytes() == payload else "failed"
    except OSError as e:
        return "denied" if e.errno in (errno.EACCES, errno.EPERM, errno.EROFS) else "unobserved"
    finally:
        if fd is not None:
            os.close(fd)
        if created:
            # Only our O_EXCL-created file. Never delete a preexisting path.
            marker.unlink()


def _write_open(path: Path) -> dict:
    """Observe write-open only; preserve original bytes. Missing is not denial evidence."""
    try:
        if path.is_symlink() or not path.is_file():
            return {"outcome": "unobserved", "preserved": None}
        before = hashlib.sha256(path.read_bytes()).hexdigest()
        try:
            fd = os.open(path, os.O_WRONLY | os.O_NOFOLLOW)
        except OSError as e:
            outcome = "denied" if e.errno in (errno.EACCES, errno.EPERM, errno.EROFS) else "unobserved"
        else:
            os.close(fd)
            outcome = "allowed"
        return {"outcome": outcome, "preserved": hashlib.sha256(path.read_bytes()).hexdigest() == before}
    except OSError:
        return {"outcome": "unobserved", "preserved": None}


def _git_dirs(root: Path):
    found = []
    for flag in ("--absolute-git-dir", "--git-common-dir"):
        rc, out = _command(["git", "rev-parse", flag], root)
        if rc != 0 or not out.strip():
            return None
        p = Path(out.strip())
        p = (root / p).resolve() if not p.is_absolute() else p.resolve()
        if not p.is_dir():
            return None
        if p not in found:
            found.append(p)
    return found


def _hint_root(workspace: Path):
    # Node-local declaration only. Never accept a main-guessed external path argument.
    try:
        for line in (workspace / "envs/.env").read_text().splitlines():
            if line.strip().startswith("HINT_CLONE_DIR="):
                value = line.strip().split("=", 1)[1].strip().strip("\"'")
                p = Path(value)
                if not value or not p.is_absolute() or not p.is_dir():
                    return None
                p = p.resolve()
                if p == workspace or workspace in p.parents or p in workspace.parents:
                    return None
                rc, out = _command(["git", "rev-parse", "--show-toplevel"], p)
                return p if rc == 0 and Path(out.strip()).resolve() == p else None
    except (OSError, UnicodeError):
        pass
    return None


# Integrity authority of the delivered reviewed original (workflow.md git 단일 권위, 2문항 판정표) - decided per NODE by where this helper's bytes live:
#   * a git on THIS node holds the helper as a stage-0 blob (Q1 = yes, e.g. the tracked Main checkout): git is the ONLY authority. The bytes read here must equal
#     the blob git holds; NO digest is consulted, so no second integrity authority is created and a change that git carries (staged/committed) is git's call.
#   * no git holds it (a git-less overlay node: wiki-only ray-worker, a2a-agent sub - Q1 = no, and not regenerable from tracked inputs there - Q2 = no): the
#     blind-spot layer, where a digest is the only evidence. BOOTSTRAP_READER_SHA256 is then a CLOSED ORIGINAL-SEMANTIC-PROFILE tripwire (it pins WHICH reviewed
#     semantic profile, e84fa13c…, the permission canary reads manifests with), not an integrity digest that restates a tracked file.
# Consequence (stated, not hidden): on a git-tracked node the semantic-profile pin is NOT enforced - a deliberately staged/committed change of the helper passes.
# Enforcing the pin there as well would restate a tracked object's digest in a second place, which the single-authority rule forbids; that is a policy question
# for the user, not something this probe decides.
BOOTSTRAP_READER_SHA256 = "e84fa13c1f37ce4e2635113627219a9ab44b1b6b1eeed246f4e7f80e8b19f651"


def _blob_id(data: bytes, hex_len: int) -> str:
    """Git object id of `data` as a blob (no filters), SHA-1 or SHA-256 object format by the id length git printed."""
    algo = hashlib.sha256 if hex_len == 64 else hashlib.sha1
    return algo(b"blob %d\0" % len(data) + data).hexdigest()


def _git_held_blob(path: Path):
    """The stage-0 blob id a git on this node holds for `path`, or None when no git holds it (no git binary, not in a work tree, untracked, ignored, conflicted)."""
    rc, out = _command(["git", "ls-files", "-s", "-z", "--", path.name], path.parent)
    if rc != 0:
        return None
    for entry in out.split("\0"):
        meta, _, name = entry.partition("\t")
        fields = meta.split()
        if name == path.name and len(fields) == 3 and fields[2] == "0" and fields[0] in ("100644", "100755") and re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", fields[1]):
            return fields[1]
    return None


def _reader_authority(path: Path, data: bytes) -> str:
    """Verify the ONE read of the helper against the node's integrity authority; returns "git" or "semantic-profile-pin", else raises (never `unobserved`)."""
    held = _git_held_blob(path)
    if held is not None:
        if _blob_id(data, len(held)) != held:
            raise RuntimeError("[harness_permission_probe] bootstrap manifest reader differs from the blob this node's git holds (git is the integrity authority here): %s" % path)
        return "git"
    if hashlib.sha256(data).hexdigest() != BOOTSTRAP_READER_SHA256:
        raise RuntimeError("[harness_permission_probe] bootstrap manifest reader is not the reviewed original and no git on this node holds it (semantic-profile pin differs): %s" % path)
    return "semantic-profile-pin"


def _contract_reader():
    """Bootstrap permission-metadata reader: the delivered EXACT reviewed original (`manifest_contract_compat_owner.py`), selected by this probe's role and purpose.
    This probe runs on every sub plane, including a wiki-only ray-worker that builds no B and receives no main ELF, so its reader is Python BY DESIGN: it is a
    permission/metadata reader, not a business Rust claim, and there is no try-Rust-then-Python fallback (the Rust-backed adapter is never imported here).
    The helper is read ONCE; the very bytes that pass `_reader_authority` are the bytes compiled and executed (no second read window). A missing, unreadable or
    unauthorised helper is a clear failure of the probe itself; `observe` calls this first, before any observation side effect, so it can never be masked as `unobserved`."""
    import types
    path = Path(__file__).resolve().with_name("manifest_contract_compat_owner.py")
    try:
        data = path.read_bytes()
    except OSError as error:
        raise RuntimeError("[harness_permission_probe] bootstrap manifest reader is missing or unreadable: %s (%s)" % (path, error)) from error
    _reader_authority(path, data)
    module = types.ModuleType("manifest_contract_compat_owner")
    module.__file__ = str(path)
    exec(compile(data, str(path), "exec"), module.__dict__)
    return module


def _build_files(workspace: Path, topology: str, reader=None):
    """Inspect the declared compose input; unresolved interpolation is not proof. `reader` is the module `observe` loaded once (direct callers get a fresh one)."""
    mc = reader if reader is not None else _contract_reader()
    try:
        compose_path = workspace / "output" / topology / "docker-compose.yaml"
        compose = mc._load_manifest(compose_path)
        # Multi build inputs select Dockerfiles through Compose interpolation.
        # Observe Compose's actual selection; unresolved strings are never proof.
        if any(isinstance(service, dict) and isinstance(service.get("build"), dict)
               and "$" in str(service["build"].get("dockerfile", ""))
               for service in compose.get("services", {}).values()):
            argv = ["docker", "compose", "--profile", "*"]
            env_path = compose_path.parent / ".env"
            if env_path.is_file():
                argv += ["--env-file", str(env_path)]
            rc, out = _command(argv + ["-f", str(compose_path), "config", "--format", "json"], workspace)
            if rc != 0:
                return None
            compose = json.loads(out)
            if not isinstance(compose, dict):
                return None
        selected = set()
        for service in compose.get("services", {}).values():
            build = service.get("build") if isinstance(service, dict) else None
            if isinstance(build, dict):
                name = build.get("dockerfile", "Dockerfile")
                if name not in DOCKERFILES:
                    return None
                selected.add(name)
        return [*BUILD_FILES, *sorted(selected)] if selected else None
    except (OSError, ValueError, TypeError, AttributeError, ImportError):
        return None


def _model_asset_read(workspace: Path, topology: str, role: str, reader=None):
    """Canonical loader, path access only. Never read or print weight/config bytes."""
    mc = reader if reader is not None else _contract_reader()
    try:
        man = mc._load_manifest(workspace / "output" / topology / "manifest.yaml")
        node = next((n for n in man.get("nodes", []) if n.get("role") == role), {})
        mode = mc.effective_model_source(node, man.get("model_source")) if topology == "single" else man.get("model_source")
        if mode == "ephemeral":
            return {"outcome": "not_applicable", "model_source": mode,
                    "operation": "manifest-declared ephemeral; no model acquisition attempted"}
        if mode == "managed":
            paths = [man.get("nas_model_path")]
            if man.get("quant_model_path"):
                paths.append(man["quant_model_path"])
        elif mode == "custom" and isinstance(man.get("custom_model_paths"), dict):
            paths = list(man["custom_model_paths"].values())
        else:
            paths = []
        if not paths or any(not isinstance(p, str) or not Path(p).is_absolute() for p in paths):
            return {"outcome": "unobserved", "model_source": mode}
        for p in paths:
            fd = os.open(p, os.O_RDONLY | os.O_DIRECTORY)
            os.close(fd)
        return {"outcome": "allowed", "model_source": mode,
                "operation": "read-open declared model root directories; no weight bytes", "root_count": len(paths)}
    except (OSError, ValueError, TypeError, AttributeError, ImportError):
        return {"outcome": "unobserved", "operation": "model declaration/read unavailable"}


def observe(workspace: Path, topology: str, role: str, nonce: str) -> dict:
    workspace = workspace.resolve()
    # The bootstrap reader is loaded and authorised ONCE, first: a missing/tampered helper stops the probe before the scratch dir, the protected-file write-opens
    # or any Git metadata marker exists - those side effects must never precede (or be mistaken for) a reader failure.
    reader = _contract_reader()
    checks = []

    def add(name, outcome, *, axis="functional", **extra):
        checks.append({"name": name, "axis": axis, "outcome": outcome, **extra})

    try:
        # No preexisting files are overwritten. TemporaryDirectory removes only its own tree.
        logs = workspace / "docs/logs"
        logs.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="harness-permission-", dir=logs) as td:
            scratch = Path(td)
            add("scratch_write_cleanup", _write_marker(scratch, nonce))
            initialized = _command(["git", "init", "--quiet", "--template="], scratch)[0] == 0
            git_dirs = _git_dirs(scratch) if initialized else None
            rc, top = _command(["git", "rev-parse", "--show-toplevel"], scratch) if initialized else (None, "")
            bound = (rc == 0 and Path(top.strip()).resolve() == scratch.resolve() and git_dirs
                     and all(scratch.resolve() in p.parents for p in git_dirs))
            commands = (
                        ["git", "-c", "user.name=Harness Canary", "-c", "user.email=harness-canary@invalid",
                         "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit",
                         "--quiet", "--allow-empty", "-m", "Disposable permission canary"],
                        ["git", "branch", "canary-local-only"])
            add("scratch_local_git", "allowed" if bound and all(_command(a, scratch)[0] == 0 for a in commands) else "failed")
        # Only report cleanup after the context actually completed.
        add("scratch_removed", "allowed" if not scratch.exists() else "failed")
    except OSError:
        add("scratch_write_cleanup", "unobserved")
    dirs = _git_dirs(workspace)
    if dirs is None:
        add("workspace_git_metadata", "unobserved")
    else:
        for n, directory in enumerate(dirs):
            try:
                outcome = _write_marker(directory, nonce)
            except OSError:
                outcome = "failed"
            add("workspace_git_metadata" + ("_common" if n else ""), outcome)
    # OS write-open is an additional Codex boundary. Claude's native tool/managed
    # restrictions must be observed on that tool surface, not inferred from Python.
    for rel in protected_files(role):
        add("protected:" + rel, axis="os_enforcement", **_write_open(workspace / rel))
    build_files = _build_files(workspace, topology, reader)
    add("buildkit_declaration", "allowed" if build_files else "unobserved")
    for name in build_files or BUILD_FILES:
        add("buildkit:" + name, axis="os_enforcement",
            **_write_open(workspace / "output" / topology / name))
    model_read = _model_asset_read(workspace, topology, role, reader)
    add("model_asset_read", **model_read)
    rc, out = _command(["docker", "version", "--format", "{{json .Server.Version}}"], workspace)
    try:
        server = json.loads(out)
    except (ValueError, TypeError):
        server = None
    add("docker_daemon_read", "allowed" if rc == 0 and isinstance(server, str) and server else "unobserved")
    rc, out = _command(["docker", "compose", "version", "--short"], workspace)
    add("docker_compose_read", "allowed" if rc == 0 and out.strip() else "unobserved")
    rc, out = _command(["node", ".claude/skills/wiki-desk/src/cli.ts", "query", "--terms",
                        "harness-permission-canary-" + nonce, "--limit", "1"], workspace)
    try:
        query = json.loads(out)
    except (ValueError, TypeError):
        query = None
    add("library_query", "allowed" if rc == 0 and isinstance(query, dict)
        and isinstance(query.get("query_tokens"), list) and query["query_tokens"] else "unobserved")
    if topology == "single" and role == "sub":
        clone = _hint_root(workspace)
        if clone is None:
            add("hint_clone_write", "unobserved")
        else:
            dirs = _git_dirs(clone)
            roots = list(dict.fromkeys([clone, *(dirs or [])]))
            add("hint_clone_git_discovery", "allowed" if dirs else "unobserved")
            for n, directory in enumerate(roots):
                try:
                    outcome = _write_marker(directory, nonce)
                except OSError:
                    outcome = "failed"
                # Do not export node-local paths to main. Exact scopes are equipped separately.
                add("hint_clone_write" + ("_git_" + str(n) if n else ""), outcome)
    return {"schema_version": SCHEMA_VERSION, "nonce": nonce, "topology": topology, "role": role,
            "workspace": str(workspace), "provenance": "observed_local_process",
            "build_files": build_files, "checks": checks,
            "hint_work_plane": ("node-local external clone" if topology == "single" and role == "sub"
                                else "workspace git" if role == "main" else "not applicable"),
            "limits": ["write-open is not file modification/rename/create confinement proof",
                       "main canonical write access is a positive role check, not proof of outside-workspace denial",
                       "Docker read access does not prove build/run or daemon confinement",
                       "library query can populate its existing derived cache; no model or remote git",
                       "hint markers prove local clone/git writes, not publication or network push",
                       "provider tool restrictions and OS enforcement are separate surfaces"]}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--workspace", required=True)
    ap.add_argument("--topology", choices=("single", "multi"), required=True)
    ap.add_argument("--role", choices=("main", "sub"), required=True)
    ap.add_argument("--nonce", required=True)
    a = ap.parse_args()
    if not Path(a.workspace).is_absolute() or not re.fullmatch(r"[a-f0-9]{32}", a.nonce):
        ap.error("absolute workspace and a fresh 32-hex nonce are required")
    print(MARKER + json.dumps(observe(Path(a.workspace), a.topology, a.role, a.nonce),
                              ensure_ascii=False, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
