"""exl3_port_lib.py - shared helpers for the EXL3 + disk-Engram self-port scripts.

Not a patch itself (the Dockerfile loop only runs *.sh). Owns: provenance loading,
pinned fetch with sha256 verification, the status-protocol writer and the
anchored-edit primitive. Every failure raises SystemExit with a non-zero code
(fail-loud). Pins live ONLY in PROVENANCE.json next to this file.
"""
from __future__ import annotations

import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tarfile
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
PROV_PATH = HERE / "PROVENANCE.json"
VLLM_SRC = pathlib.Path(os.environ.get("EXL3_PORT_VLLM_SRC", "/workspace/vllm-src"))


def die(tag: str, msg: str, rc: int = 1):
    sys.stderr.write(f"{tag} FAIL: {msg}\n")
    raise SystemExit(rc)


def prov() -> dict:
    if not PROV_PATH.is_file():
        die("[exl3-port-lib]", f"provenance json missing: {PROV_PATH}")
    return json.loads(PROV_PATH.read_text())


def status(kind: str, note: str) -> None:
    """Status protocol: one line `applied<TAB>note` | `skipped<TAB>reason`."""
    assert kind in ("applied", "skipped")
    path = os.environ.get("EASY_VLLM_PATCH_STATUS")
    if path:
        with open(path, "w") as fh:
            fh.write(f"{kind}\t{note}\n")


def sha256_bytes(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def sha256_file(p) -> str:
    return sha256_bytes(pathlib.Path(p).read_bytes())


def _cache_dir(p: dict) -> pathlib.Path:
    d = pathlib.Path(p["install"]["fetch_cache"])
    d.mkdir(parents=True, exist_ok=True)
    return d


def _download(url: str, dest: pathlib.Path, tag: str) -> None:
    tmp = dest.with_suffix(dest.suffix + ".part")
    cmd = ["curl", "-fsSL", "--retry", "5", "--retry-delay", "3", "--retry-all-errors", "--connect-timeout", "30",
           "--speed-limit", "2048", "--speed-time", "30", "-o", str(tmp), url]  # abort+retry stalled GitHub archive streams
    r = subprocess.run(cmd)
    if r.returncode != 0:
        die(tag, f"download failed rc={r.returncode}: {url}")
    tmp.replace(dest)


def _extract(tgz: pathlib.Path, dest: pathlib.Path, tag: str) -> None:
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    with tarfile.open(tgz) as tf:
        members = tf.getmembers()
        root = members[0].name.split("/")[0]
        for m in members:
            parts = m.name.split("/")
            if parts[0] != root:
                die(tag, f"unexpected tar layout member {m.name!r}")
            rel = "/".join(parts[1:])
            if not rel:
                continue
            if rel.startswith("/") or ".." in rel.split("/"):
                die(tag, f"unsafe tar path {m.name!r}")
            m.name = rel
            tf.extract(m, dest, filter="tar") if sys.version_info >= (3, 12) else tf.extract(m, dest)


def fetch_mia_tree(tag: str) -> pathlib.Path:
    """Fetch the pinned Mia repo tarball (cached), return extracted root."""
    p = prov()
    s = p["sources"]["mia_overlay"]
    cache = _cache_dir(p)
    root = cache / f"mia-{s['commit']}"
    if not root.is_dir():
        tgz = cache / f"mia-{s['commit']}.tar.gz"
        url = s["tarball_url"].format(commit=s["commit"])
        print(f"{tag} fetching {url}")
        _download(url, tgz, tag)
        _extract(tgz, root, tag)
    return root


def mia_file(tag: str, rel: str) -> pathlib.Path:
    """Return path of a pinned Mia file after verifying its sha256 (mismatch = build failure)."""
    p = prov()
    table = p["sources"]["mia_overlay"]["files_sha256"]
    if rel not in table:
        die(tag, f"{rel} is not in the pinned file table")
    root = fetch_mia_tree(tag)
    f = root / rel
    if not f.is_file():
        die(tag, f"pinned commit lacks {rel}")
    got = sha256_file(f)
    if got != table[rel]:
        die(tag, f"sha256 mismatch for {rel}: got {got} want {table[rel]} (supply-chain guard)")
    return f


def tree_digest(root: pathlib.Path) -> tuple[str, int]:
    lines = []
    for q in sorted(root.rglob("*")):
        if q.is_file() and not q.is_symlink():
            lines.append(f"{q.relative_to(root).as_posix()} {sha256_file(q)}")
    return sha256_bytes("\n".join(lines).encode()), len(lines)


def fetch_pinned_tree(tag: str, key: str, dest: pathlib.Path) -> None:
    """Fetch the pinned GitHub tarball sources[key] into dest (fresh) and verify it.

    Verified (any mismatch = build failure, dest removed): whole-tree digest + file count, and - when the pin
    carries `files_sha256` - every listed file individually (so a tree-digest pin that was regenerated
    carelessly still cannot hide a changed file we install or edit).
    """
    p = prov()
    s = p["sources"][key]
    cache = _cache_dir(p)
    tgz = cache / f"{key}-{s['commit']}.tar.gz"
    if not tgz.is_file():
        url = s["tarball_url"].format(commit=s["commit"])
        print(f"{tag} fetching {url}")
        _download(url, tgz, tag)
    _extract(tgz, dest, tag)
    got, n = tree_digest(dest)
    want = s["tree_digest"]
    if got != want["sha256"] or n != want["file_count"]:
        shutil.rmtree(dest, ignore_errors=True)
        die(tag, f"{key} tree digest mismatch: got {got} ({n} files) want {want['sha256']} ({want['file_count']})")
    for rel, sha in (s.get("files_sha256") or {}).items():
        f = dest / rel
        if not f.is_file() or sha256_file(f) != sha:
            shutil.rmtree(dest, ignore_errors=True)
            die(tag, f"{key}: per-file sha256 mismatch/missing for {rel}")
    print(f"{tag} {key} {s.get('version', '')} @ {s['commit'][:12]} tree digest OK ({n} files)")


def fetch_exllamav3_tree(tag: str, dest: pathlib.Path) -> None:
    """Fetch pinned exllamav3 into dest (fresh), verify the whole-tree digest."""
    fetch_pinned_tree(tag, "exllamav3", dest)


def refuse_round1_residue(tag: str) -> None:
    """v2 must be applied to a STOCK v0.30.0 tree. A tree that already carries round-1 (Mia in-tree lineage) edits would
    silently get a second, conflicting hook (the anchors of round 1 still exist), so refuse loudly instead."""
    found = []
    if (VLLM_SRC / "vllm/model_executor/layers/quantization/exl3.py").exists():
        found.append("vllm/model_executor/layers/quantization/exl3.py (round-1 in-tree Mia module)")
    probes = [
        ("vllm/model_executor/layers/quantization/__init__.py", "from . import exl3 as _exl3_port_module"),
        ("vllm/models/deepseek_v41/compressor.py", "[exl3-port:wiring]"),
        ("vllm/models/deepseek_v41/attention.py", "[exl3-port:wiring]"),
        ("vllm/models/deepseek_v41/nvidia/dspark.py", "[exl3-port:wiring]"),
        ("vllm/models/deepseek_v4/nvidia/ops/o_proj.py", "[exl3-port:wiring]"),
        ("vllm/v1/worker/gpu_worker.py", "from vllm.model_executor.layers.quantization import exl3 as _exl3"),
    ]
    for rel, needle in probes:
        f = VLLM_SRC / rel
        if f.is_file() and needle in f.read_text():
            found.append(f"{rel} contains {needle!r}")
    if found:
        die(tag, "round-1 residue in the vLLM tree - apply the v2 set to a stock v0.30.0 source tree (rebuild from the base image), "
                 "not on top of the round-1 patched tree: " + "; ".join(found))


def cleanup_cache() -> None:
    p = prov()
    shutil.rmtree(p["install"]["fetch_cache"], ignore_errors=True)


# ---------------------------------------------------------------- anchored edits
class Edit:
    """One anchored text edit; `new` is also the idempotency marker.

    `count` = exact number of occurrences of `old` that must exist (replace-all).
    Outcome: 'already' (new present count times) | 'applied'; anything else is fatal.
    """

    def __init__(self, label: str, old: str, new: str, count: int = 1):
        self.label, self.old, self.new, self.count = label, old, new, count

    def apply(self, text: str, tag: str, fname: str) -> tuple[str, str]:
        cn, co = text.count(self.new), text.count(self.old)
        if cn == self.count:
            return text, "already"
        if cn == 0 and co == self.count:
            return text.replace(self.old, self.new), "applied"
        die(tag, f"{fname}: anchor for '{self.label}' unusable: old x{co}, new x{cn}, expected {self.count} "
                 f"- vLLM source drifted from v0.30.0 assumptions (or a partial/foreign edit is present)")


def apply_edit_jobs(tag: str, jobs, root: pathlib.Path | None = None) -> list[tuple[str, list[str]]]:
    """Two-phase multi-file edit. jobs = [(rel, [Edit...], footer|None, footer_marker|None)].

    Phase 1 computes every new text in memory (any anchor mismatch -> die, NOTHING written).
    Phase 2 writes the changed files. Returns [(rel, [per-edit outcome...])].
    `root` defaults to the vLLM source tree; the plugin build passes its own extracted tree.
    """
    staged, report = [], []
    for rel, edits, footer, fmark in jobs:
        path = (root or VLLM_SRC) / rel
        if not path.is_file():
            die(tag, f"target missing: {path}")
        text = path.read_text()
        out = []
        for e in edits:
            text, st = e.apply(text, tag, rel)
            out.append(f"{e.label}:{st}")
        if footer is not None:
            assert fmark
            if fmark in text:
                out.append("footer:already")
            else:
                if not text.endswith("\n"):
                    text += "\n"
                text += footer
                out.append("footer:applied")
        staged.append((path, text, any(o.endswith(":applied") for o in out)))
        report.append((rel, out))
    for path, text, changed in staged:
        if changed:
            path.write_text(text)
    return report


def apply_edits(tag: str, rel: str, edits: list[Edit], footer: str | None = None, footer_marker: str | None = None,
                root: pathlib.Path | None = None) -> list[str]:
    """Single-file convenience wrapper over apply_edit_jobs."""
    return apply_edit_jobs(tag, [(rel, edits, footer, footer_marker)], root=root)[0][1]


def py_compile(tag: str, paths) -> None:
    """Syntax check without writing .pyc (compile() on the source text)."""
    for q in paths:
        try:
            compile(pathlib.Path(q).read_text(), str(q), "exec")
        except SyntaxError as exc:
            die(tag, f"syntax error after patch: {q}: {exc}")
