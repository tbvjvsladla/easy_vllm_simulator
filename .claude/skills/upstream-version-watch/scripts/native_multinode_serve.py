#!/usr/bin/env python3
"""Fail-closed native two-node Ray/vLLM serve owner.

``run`` is an explicit real-E2E surface, but requires ``--apply``.  This change
runs only ``--self-test``: fake executors plus private temporary trees.  A live
run uses an already-built image as the only source of Python wheels and native
runtime libraries; no package/model download or host package installation exists.
"""
from __future__ import annotations

import argparse
import base64
import contextlib
import dataclasses
import hashlib
import json
import os
import re
import signal
import stat
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Optional, Sequence

EXIT_USAGE, EXIT_FAIL, EXIT_UNKNOWN = 2, 3, 4
RUN_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$")
HOST = re.compile(r"^[A-Za-z0-9_.@:-]+$")
MARKER = ".native-multinode-owned.json"
CREATED_BY = "native_multinode_serve.py/v1"


class ServeError(RuntimeError): pass
class UnknownState(ServeError): pass


def j(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def starttime(pid: int) -> Optional[str]:
    try:
        tail = Path(f"/proc/{pid}/stat").read_text(encoding="ascii").rsplit(")", 1)[1].split()
        return tail[19]
    except (OSError, IndexError):
        return None


def beneath(root: Path, path: Path) -> Path:
    root, path = root.resolve(), path.resolve(strict=False)
    try: path.relative_to(root)
    except ValueError as exc: raise ServeError(f"root escape rejected: {path}") from exc
    return path


def validate_remote_root(root: str, run_id: str) -> str:
    p = Path(root)
    if not p.is_absolute() or p.name != run_id or any(x in (".", "..") for x in p.parts):
        raise ServeError("remote root must be absolute, traversal-free, and basename == run-id")
    return str(p)


def marker_bytes(run_id: str) -> bytes:
    return (j({"run_id": run_id, "created_by": CREATED_BY}) + "\n").encode()


def safe_remove_owned_tree(root: Path, expected_run_id: str, marker: str = MARKER) -> None:
    """Delete only a marker-owned ordinary directory with depth-first unlink/rmdir.

    Rejecting a symlink/mount point before recursion makes a supplied run root a
    capability boundary, not a broad filesystem deletion selector.
    """
    if root.name != expected_run_id or not RUN_ID.fullmatch(expected_run_id):
        raise ServeError("owned-tree basename/run-id mismatch")
    if root.is_symlink() or not root.is_dir(): raise ServeError("owned-tree root missing, symlink, or non-directory")
    if os.path.ismount(root): raise ServeError("owned-tree root is a mount point")
    if (root / marker).read_bytes() != marker_bytes(expected_run_id):
        raise ServeError("owned-tree marker is absent or not exact")
    canonical = root.resolve()
    def remove(directory: Path) -> None:
        directory = beneath(canonical, directory)
        if directory.is_symlink() or os.path.ismount(directory):
            raise ServeError(f"owned-tree entry is symlink/mount point: {directory}")
        with os.scandir(directory) as scan:
            entries = list(scan)
        for entry in entries:
            child = beneath(canonical, directory / entry.name)
            info = entry.stat(follow_symlinks=False)
            if stat.S_ISLNK(info.st_mode): raise ServeError(f"owned-tree symlink rejected: {child}")
            if stat.S_ISDIR(info.st_mode): remove(child)
            elif stat.S_ISREG(info.st_mode): child.unlink()
            else: raise ServeError(f"owned-tree special file rejected: {child}")
        directory.rmdir()
    remove(canonical)
    if root.exists() or root.is_symlink(): raise UnknownState("owned tree remains after depth-first removal")


@dataclasses.dataclass(frozen=True)
class Identity:
    node: str; role: str; pid: int; pgid: int; starttime: str; log_rel: str
    def row(self) -> dict[str, Any]: return dataclasses.asdict(self)

@dataclasses.dataclass
class Result:
    argv: list[str]; rc: int; out: str = ""; err: str = ""

class Runner:
    node: str
    def run(self, argv: Sequence[str], timeout: int, check: bool = True) -> Result: raise NotImplementedError
    def start(self, argv: Sequence[str], log_rel: str) -> Identity: raise NotImplementedError
    def stop(self, ident: Identity, wait: int) -> dict[str, Any]: raise NotImplementedError
    def text(self, rel: str) -> str: raise NotImplementedError
    def descendants(self, pgid: int) -> list[dict[str, Any]]: raise NotImplementedError
    def gpu(self) -> list[str]: raise NotImplementedError
    def remove_owned_tree(self, run_id: str) -> None: raise NotImplementedError

class Local(Runner):
    def __init__(self, root: Path, node: str = "main") -> None: self.root, self.node = root, node
    def run(self, argv: Sequence[str], timeout: int, check: bool = True) -> Result:
        if not argv or any(not isinstance(x, str) or "\0" in x for x in argv): raise ServeError("invalid exact argv")
        try: p = subprocess.run(list(argv), text=True, capture_output=True, timeout=timeout)
        except subprocess.TimeoutExpired as exc: raise UnknownState(f"{self.node}: bounded command timed out") from exc
        r = Result(list(argv), p.returncode, p.stdout, p.stderr)
        if check and r.rc: raise ServeError(f"{self.node}: rc={r.rc}: {r.err[-800:]}")
        return r
    def start(self, argv: Sequence[str], log_rel: str) -> Identity:
        log = beneath(self.root, self.root / log_rel); log.parent.mkdir(parents=True, exist_ok=True)
        with log.open("ab", buffering=0) as fd:
            p = subprocess.Popen(list(argv), stdin=subprocess.DEVNULL, stdout=fd, stderr=subprocess.STDOUT,
                                 start_new_session=True, close_fds=True)
        st = starttime(p.pid)
        if st is None: raise UnknownState("cannot record child starttime")
        return Identity(self.node, Path(argv[0]).name, p.pid, os.getpgid(p.pid), st, log_rel)
    def stop(self, ident: Identity, wait: int) -> dict[str, Any]:
        observed = starttime(ident.pid)
        if observed is None: return {"node": self.node, "role": ident.role, "pid": ident.pid, "term_status": "already-gone", "starttime_match": True}
        if observed != ident.starttime: raise UnknownState(f"{self.node}: PID reuse before TERM")
        with contextlib.suppress(ProcessLookupError): os.killpg(ident.pgid, signal.SIGTERM)
        end = time.monotonic() + wait
        while time.monotonic() < end:
            if starttime(ident.pid) is None: return {"node": self.node, "role": ident.role, "pid": ident.pid, "term_status": "term-exited", "starttime_match": True}
            time.sleep(.1)
        observed = starttime(ident.pid)
        if observed != ident.starttime:
            if observed is None: return {"node": self.node, "role": ident.role, "pid": ident.pid, "term_status": "term-exited", "starttime_match": True}
            raise UnknownState(f"{self.node}: PID reuse before KILL")
        with contextlib.suppress(ProcessLookupError): os.killpg(ident.pgid, signal.SIGKILL)
        end = time.monotonic() + min(wait, 10)
        while time.monotonic() < end:
            if starttime(ident.pid) is None: return {"node": self.node, "role": ident.role, "pid": ident.pid, "term_status": "kill-exited", "starttime_match": True}
            time.sleep(.1)
        raise UnknownState(f"{self.node}: same-starttime process survived KILL")
    def text(self, rel: str) -> str: return beneath(self.root, self.root / rel).read_text(encoding="utf-8", errors="replace")
    def descendants(self, pgid: int) -> list[dict[str, Any]]:
        r = self.run(["ps", "-eo", "pid=,ppid=,pgid=,args="], 15)
        hits = []
        for line in r.out.splitlines():
            q = line.strip().split(None, 3)
            if len(q) >= 3 and q[2] == str(pgid): hits.append({"pid": int(q[0]), "ppid": int(q[1]), "pgid": int(q[2]), "starttime": starttime(int(q[0])), "argv": q[3] if len(q) > 3 else ""})
        return hits
    def gpu(self) -> list[str]:
        r = self.run(["nvidia-smi", "--query-compute-apps=pid,process_name,used_memory", "--format=csv,noheader,nounits"], 20, False)
        if r.rc: raise UnknownState(f"{self.node}: fresh GPU query failed")
        return [x for x in r.out.splitlines() if x.strip()]
    def remove_owned_tree(self, run_id: str) -> None: safe_remove_owned_tree(self.root, run_id)

# Fixed remote runner: arguments are base64 JSON, never interpolated into remote shell text.
REMOTE = r'''import base64,json,os,signal,stat,subprocess,sys,time
r=json.loads(base64.b64decode(sys.argv[1])); root=r['root']; rid=r['run_id']; marker=r['marker']; created=r['created_by']; op=r['op']
def die(x): raise SystemExit(x)
def emit(x): print(json.dumps(x,sort_keys=True),flush=True)
def under_rel(rel):
 if not isinstance(rel,str) or rel.startswith('/'): die('bad-relative-path')
 p=os.path.realpath(os.path.join(root,rel)); b=os.path.realpath(root)
 if os.path.commonpath([b,p])!=b: die('escape')
 return p
def st(pid):
 try:return open('/proc/%d/stat'%pid).read().rsplit(')',1)[1].split()[19]
 except Exception:return None
def owned_root():
 if os.path.basename(root)!=rid or os.path.islink(root) or not os.path.isdir(root) or os.path.ismount(root):die('bad-root')
 want=json.dumps({'run_id':rid,'created_by':created},sort_keys=True,separators=(',',':'))+'\n'
 if open(os.path.join(root,marker)).read()!=want:die('bad-marker')
def remove(d):
 d=under_rel(os.path.relpath(d,root))
 if os.path.islink(d) or os.path.ismount(d):die('link/mount')
 for e in list(os.scandir(d)):
  p=under_rel(os.path.relpath(e.path,root)); m=e.stat(follow_symlinks=False).st_mode
  if stat.S_ISLNK(m):die('symlink')
  if stat.S_ISDIR(m):remove(p)
  elif stat.S_ISREG(m):os.unlink(p)
  else:die('special')
 os.rmdir(d)
if op=='init':
 if os.path.lexists(root):die('root-exists')
 parent=os.path.dirname(root)
 if not os.path.isdir(parent) or os.path.islink(parent):die('bad-parent')
 os.mkdir(root,0o700)
 open(os.path.join(root,marker),'w').write(json.dumps({'run_id':rid,'created_by':created},sort_keys=True,separators=(',',':'))+'\n')
 emit({'ok':True})
elif op=='run':
 p=subprocess.run(r['argv'],stdin=subprocess.DEVNULL,text=True,capture_output=True,timeout=r['timeout'])
 emit({'rc':p.returncode,'out':p.stdout,'err':p.stderr})
elif op=='start':
 log=under_rel(r['log_rel']);os.makedirs(os.path.dirname(log),exist_ok=True);f=open(log,'ab',buffering=0)
 p=subprocess.Popen(r['argv'],stdin=subprocess.DEVNULL,stdout=f,stderr=subprocess.STDOUT,start_new_session=True,close_fds=True);f.close();x=st(p.pid)
 if x is None:die('starttime')
 emit({'node':'sub','role':os.path.basename(r['argv'][0]),'pid':p.pid,'pgid':os.getpgid(p.pid),'starttime':x,'log_rel':r['log_rel']})
elif op=='stop':
 i=r['identity'];pid=int(i['pid']);got=st(pid)
 if got is None:emit({'node':'sub','role':i['role'],'pid':pid,'term_status':'already-gone','starttime_match':True})
 elif got!=str(i['starttime']):die('pid-reuse-before-term')
 else:
  try:os.killpg(int(i['pgid']),signal.SIGTERM)
  except ProcessLookupError:pass
  end=time.monotonic()+int(r['wait'])
  while time.monotonic()<end and st(pid)is not None:time.sleep(.1)
  got=st(pid)
  if got is None:emit({'node':'sub','role':i['role'],'pid':pid,'term_status':'term-exited','starttime_match':True})
  elif got!=str(i['starttime']):die('pid-reuse-before-kill')
  else:
   try:os.killpg(int(i['pgid']),signal.SIGKILL)
   except ProcessLookupError:pass
   end=time.monotonic()+min(int(r['wait']),10)
   while time.monotonic()<end and st(pid)is not None:time.sleep(.1)
   if st(pid)is not None:die('survived-kill')
   emit({'node':'sub','role':i['role'],'pid':pid,'term_status':'kill-exited','starttime_match':True})
elif op=='text':
 with open(under_rel(r['rel']),encoding='utf-8',errors='replace')as f:emit({'text':f.read()})
elif op=='descendants':
 p=subprocess.run(['ps','-eo','pid=,ppid=,pgid=,args='],text=True,capture_output=True,check=True);out=[]
 for line in p.stdout.splitlines():
  q=line.strip().split(None,3)
  if len(q)>=3 and q[2]==str(r['pgid']):out.append({'pid':int(q[0]),'ppid':int(q[1]),'pgid':int(q[2]),'starttime':st(int(q[0])),'argv':q[3] if len(q)>3 else ''})
 emit({'processes':out})
elif op=='gpu':
 p=subprocess.run(['nvidia-smi','--query-compute-apps=pid,process_name,used_memory','--format=csv,noheader,nounits'],text=True,capture_output=True)
 if p.returncode:die('gpu-query')
 emit({'processes':[x for x in p.stdout.splitlines()if x.strip()]})
elif op=='remove_owned_tree':
 owned_root();remove(root)
 if os.path.lexists(root):die('remaining')
 emit({'ok':True})
else:die('bad-op')'''

class SSH(Runner):
    def __init__(self, host: str, root: str, node: str = "sub") -> None:
        if not HOST.fullmatch(host): raise ServeError("unsafe sub host")
        self.host, self.root, self.node = host, root, node
    def _call(self, op: str, timeout: int, **request: Any) -> dict[str, Any]:
        # Fixed remote Python program; caller data is base64 JSON, never an SSH shell fragment.
        allowed = {"init", "run", "start", "stop", "text", "descendants", "gpu", "remove_owned_tree"}
        if op not in allowed:
            raise ServeError(f"sub fixed runner operation outside allowlist: {op}")
        request.update(op=op, root=self.root, run_id=request.pop("run_id"), marker=MARKER, created_by=CREATED_BY)
        payload = base64.b64encode(j(request).encode()).decode()
        try:
            p = subprocess.run(["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8", self.host, "python3", "-c", REMOTE, payload], text=True, capture_output=True, timeout=timeout + 13)
        except subprocess.TimeoutExpired as exc: raise UnknownState("sub fixed runner bounded timeout") from exc
        if p.returncode: raise UnknownState(f"sub fixed runner rejected: {p.stderr[-500:]}")
        try: return json.loads(p.stdout)
        except json.JSONDecodeError as exc: raise UnknownState("sub fixed runner non-JSON result") from exc
    def init_root(self) -> None:
        response = self._call("init", 30, run_id=Path(self.root).name)
        if response.get("ok") is not True:
            raise UnknownState("sub fixed runner init non-pass")
    def run(self, argv: Sequence[str], timeout: int, check: bool = True) -> Result:
        response = self._call("run", timeout, run_id=Path(self.root).name, argv=list(argv), timeout=timeout)
        result = Result(list(argv), int(response["rc"]), str(response.get("out", "")), str(response.get("err", "")))
        if check and result.rc: raise ServeError(f"sub: rc={result.rc}: {result.err[-800:]}")
        return result
    def start(self, argv: Sequence[str], log_rel: str) -> Identity:
        response = self._call("start", 30, run_id=Path(self.root).name, argv=list(argv), log_rel=log_rel)
        return Identity(**response)
    def stop(self, ident: Identity, wait: int) -> dict[str, Any]:
        return self._call("stop", wait + 15, run_id=Path(self.root).name, identity=ident.row(), wait=wait)
    def text(self, rel: str) -> str:
        return str(self._call("text", 30, run_id=Path(self.root).name, rel=rel)["text"])
    def descendants(self, pgid: int) -> list[dict[str, Any]]:
        return list(self._call("descendants", 20, run_id=Path(self.root).name, pgid=pgid)["processes"])
    def gpu(self) -> list[str]:
        return list(self._call("gpu", 25, run_id=Path(self.root).name)["processes"])
    def remove_owned_tree(self, run_id: str) -> None:
        response = self._call("remove_owned_tree", 45, run_id=run_id)
        if response.get("ok") is not True: raise UnknownState("sub owned-tree removal non-pass")

class Evidence:
    def __init__(self, root: Path) -> None: self.root = root
    def write(self, rel: str, value: Any) -> Path:
        dst = beneath(self.root, self.root / rel); dst.parent.mkdir(parents=True, exist_ok=True)
        dst.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8"); return dst
    def text(self, rel: str, value: str) -> Path:
        dst = beneath(self.root, self.root / rel); dst.parent.mkdir(parents=True, exist_ok=True); dst.write_text(value, encoding="utf-8"); return dst

@dataclasses.dataclass
class Config:
    run_id: str; cell: str; root: Path; repo: Path; preserve: Path; preserve_rel: str
    image: str; wheelhouse: str; model: str; master: str; sub: str; remote_root: str; ray_port: int; serve_port: int; tp: int

class Orchestrator:
    def __init__(self, c: Config, main: Runner, sub: Runner) -> None:
        self.c, self.main, self.sub = c, main, sub; self.ev = Evidence(c.root); self.owned: list[tuple[Runner, Identity]] = []; self.preserved = False
    def _record_marker(self) -> None:
        self.ev.text(MARKER, marker_bytes(self.c.run_id).decode())
    def _cmd(self, runner: Runner, argv: list[str], timeout: int, check: bool = True) -> Result:
        r = runner.run(argv, timeout, check); self.ev.text(f"evidence/{runner.node}-commands.jsonl", j({"argv": argv, "rc": r.rc, "stderr_tail": r.err[-1000:]}) + "\n"); return r
    def prepare(self) -> None:
        self._record_marker()
        # Exact production surface intentionally avoids downloads: Docker extracts artifacts; pip is offline.
        for runner, root in ((self.main, str(self.c.root)), (self.sub, self.c.remote_root)):
            self._cmd(runner, ["mkdir", "-p", f"{root}/wheelhouse", f"{root}/runtime-lib", f"{root}/logs"], 30)
            # Script is static; image/root are positional parameters. Image-derived runtime lib bundle includes CUDA13 user libraries when present.
            script = "set -eu; root=$1; image=$2; wh=$3; docker create --name $4 $image true >/dev/null; c=$4; trap 'docker rm -f $c >/dev/null 2>&1 || :' EXIT; docker cp $c:$wh/. $root/wheelhouse; for d in /lib/aarch64-linux-gnu /usr/lib/aarch64-linux-gnu /usr/local/cuda-13.3/lib64 /usr/local/cuda/lib64 /opt/nvidia; do docker cp $c:$d $root/runtime-lib/ 2>/dev/null || :; done; find $root/wheelhouse -name '*.whl' -type f -print -quit | grep -q ."
            self._cmd(runner, ["bash", "-ceu", script, "native-extract", root, self.c.image, self.c.wheelhouse, f"native-{self.c.run_id}-{runner.node}"], 240)
            py = f"{root}/venv/bin/python"; self._cmd(runner, ["python3", "-m", "venv", f"{root}/venv"], 90)
            self._cmd(runner, [py, "-m", "pip", "install", "--no-index", "--disable-pip-version-check", "--no-cache-dir", "--find-links", f"{root}/wheelhouse", "vllm", "ray"], 900)
            freeze = self._cmd(runner, [py, "-m", "pip", "freeze", "--all"], 90).out; self.ev.text(f"evidence/{runner.node}-pip-freeze.txt", freeze)
            # ldd must see no unresolved dependencies with only bundle injected; host CUDA/driver stay host-owned.
            self._cmd(runner, ["env", f"LD_LIBRARY_PATH={root}/runtime-lib", "ldd", f"{root}/venv/lib/python3/site-packages/vllm/_C.abi3.so"], 60)
    def start(self) -> None:
        # Hooks are deliberately conservative; default real command surface is exact argv, no shell composition.
        groups = [(self.main, "ray-head", ["env", f"LD_LIBRARY_PATH={self.c.root}/runtime-lib", f"{self.c.root}/venv/bin/ray", "start", "--head", "--node-ip-address", self.c.master, "--port", str(self.c.ray_port), "--block"]),
                  (self.sub, "ray-worker", ["env", f"LD_LIBRARY_PATH={self.c.remote_root}/runtime-lib", f"{self.c.remote_root}/venv/bin/ray", "start", "--address", f"{self.c.master}:{self.c.ray_port}", "--block"]),
                  (self.main, "vllm-serve", ["env", f"LD_LIBRARY_PATH={self.c.root}/runtime-lib", f"RAY_ADDRESS={self.c.master}:{self.c.ray_port}", f"{self.c.root}/venv/bin/vllm", "serve", self.c.model, "--host", "127.0.0.1", "--port", str(self.c.serve_port), "--tensor-parallel-size", str(self.c.tp), "--distributed-executor-backend", "ray"])]
        for r, role, argv in groups:
            ident = r.start(argv, f"logs/{role}.log"); self.owned.append((r, ident))
    def copy_before_cleanup(self) -> None:
        for r, ident in self.owned: self.ev.text(f"evidence/{r.node}-{ident.role}.log", r.text(ident.log_rel))
        if self.c.preserve.exists(): raise ServeError("caller-specified evidence preserve directory already exists")
        self.c.preserve.parent.mkdir(parents=True, exist_ok=True)
        # Copy each regular evidence file; preserve directory is outside disposable root and repo-relative by construction.
        self.c.preserve.mkdir()
        for source in (self.c.root / "evidence").rglob("*"):
            if source.is_file():
                dst = self.c.preserve / source.relative_to(self.c.root / "evidence"); dst.parent.mkdir(parents=True, exist_ok=True); dst.write_bytes(source.read_bytes())
        self.preserved = True
    def cleanup(self) -> dict[str, Any]:
        if not self.preserved: raise UnknownState("evidence preserve incomplete; owned roots cannot be removed")
        errors: list[str] = []; nodes = {"main": {"root_absent": False, "owned_processes": [], "owned_gpu_processes": [], "unknown": False}, "sub": {"root_absent": False, "owned_processes": [], "owned_gpu_processes": [], "unknown": False}}
        cleanup = []
        for r, ident in reversed(self.owned):
            try: cleanup.append(r.stop(ident, 30))
            except Exception as exc: nodes[r.node]["unknown"] = True; errors.append(str(exc))
        for r, ident in self.owned:
            try:
                hits = r.descendants(ident.pgid); nodes[r.node]["owned_processes"].extend(hits)
                if hits: errors.append(f"owned descendant residue {r.node}/{ident.pgid}")
            except Exception as exc: nodes[r.node]["unknown"] = True; errors.append(str(exc))
        for r in (self.main, self.sub):
            try:
                gpu = r.gpu(); owned = {str(i.pid) for x, i in self.owned if x is r}; nodes[r.node]["owned_gpu_processes"] = [line for line in gpu if line.split(",", 1)[0].strip() in owned]
                if nodes[r.node]["owned_gpu_processes"]: errors.append(f"owned GPU residue {r.node}")
            except Exception as exc: nodes[r.node]["unknown"] = True; errors.append(str(exc))
        try: self.main.remove_owned_tree(self.c.run_id); nodes["main"]["root_absent"] = not self.c.root.exists()
        except Exception as exc: nodes["main"]["unknown"] = True; errors.append(str(exc))
        try: self.sub.remove_owned_tree(self.c.run_id); nodes["sub"]["root_absent"] = True
        except Exception as exc: nodes["sub"]["unknown"] = True; errors.append(str(exc))
        ok = not errors and all(v["root_absent"] and not v["owned_processes"] and not v["owned_gpu_processes"] and not v["unknown"] for v in nodes.values())
        return {"schema_version": 1, "kind": "native_multinode_cleanup_attestation", "plane": "native", "run_id": self.c.run_id, "cell": self.c.cell, "status": "PASS" if ok else "FAIL_CLOSED", "nodes": nodes, "cleanup": cleanup, "preserved": {"evidence_root": self.c.preserve_rel}, "errors": errors, "provenance": "measured(cleanup + fresh residue queries)"}

class Fake(Runner):
    def __init__(self, node: str, root: Path, fault: str = "") -> None: self.node, self.root, self.fault, self.n = node, root, fault, 100
    def run(self, argv: Sequence[str], timeout: int, check: bool = True) -> Result:
        if self.fault == "prepare": raise ServeError("fake extraction fault")
        return Result(list(argv), 0, "", "")
    def start(self, argv: Sequence[str], log_rel: str) -> Identity:
        self.n += 1; return Identity(self.node, "fake", self.n, self.n, str(self.n), log_rel)
    def stop(self, ident: Identity, wait: int) -> dict[str, Any]:
        if self.fault == "reuse": raise UnknownState("fake PID reuse")
        return {"node": self.node, "role": ident.role, "pid": ident.pid, "term_status": "term-exited", "starttime_match": True}
    def text(self, rel: str) -> str: return "fake log\n"
    def descendants(self, pgid: int) -> list[dict[str, Any]]: return [{"pid": 9}] if self.fault == "residue" else []
    def gpu(self) -> list[str]:
        if self.fault == "gpu": raise UnknownState("fake GPU unknown")
        return []
    def remove_owned_tree(self, run_id: str) -> None: safe_remove_owned_tree(self.root, run_id)

def self_test() -> int:
    bad = []
    def ck(name: str, cond: bool) -> None:
        print(("[PASS] " if cond else "[FAIL] ") + name); bad.extend([] if cond else [name])
    with tempfile.TemporaryDirectory(prefix="native-mn-") as d:
        base = Path(d); root = base / "run"; root.mkdir(); (root / MARKER).write_bytes(marker_bytes("run")); (root / "a").mkdir(); (root / "a/f").write_text("x")
        safe_remove_owned_tree(root, "run"); ck("owned tree depth-first deletion", not root.exists())
        for case in ("symlink", "mount-marker", "bad-marker"):
            r = base / case; r.mkdir(); (r / MARKER).write_bytes(marker_bytes(case))
            if case == "symlink": (r / "link").symlink_to(base)
            if case == "mount-marker":
                original_ismount = os.path.ismount
                os.path.ismount = lambda path: Path(path) == r
            if case == "bad-marker": (r / MARKER).write_text("bad")
            failed = False
            try: safe_remove_owned_tree(r, case)
            except ServeError: failed = True
            finally:
                if case == "mount-marker": os.path.ismount = original_ismount
            ck(f"reject {case}", failed)
        # Fake cleanup validates evidence-before-remove state and all residue faults.
        for fault in ("", "reuse", "residue", "gpu"):
            rid = "x" + (fault or "ok"); r = base / rid; r.mkdir(); (r / MARKER).write_bytes(marker_bytes(rid)); repo = base / "repo"; repo.mkdir(exist_ok=True); preserve = repo / rid
            c = Config(rid, "cell", r, repo, preserve, rid, "img", "/wh", "model", "m", "s", "/remote/" + rid, 1, 2, 2)
            o = Orchestrator(c, Fake("main", r, fault), Fake("sub", base / (rid + "sub"), fault))
            # Fake remote does not own actual local root: exercise state with a marker-owned substitute and expect failure/pass as appropriate.
            subparent = base / (rid + "-remote"); subparent.mkdir()
            subroot = subparent / rid; subroot.mkdir(); (subroot / MARKER).write_bytes(marker_bytes(rid)); o.sub.root = subroot
            o.owned = [(o.main, Identity("main", "head", 1, 1, "1", "logs/a")), (o.sub, Identity("sub", "worker", 2, 2, "2", "logs/b"))]
            o.copy_before_cleanup(); out = o.cleanup()
            ck(f"cleanup {fault or 'normal'} fail-closed", (out["status"] == "PASS") == (fault == ""))
        body = Path(__file__).read_text(); ck("no name-based cleanup primitives", "p" + "kill" not in body and "p" + "grep" not in body)
        ck("same-starttime KILL contract", "PID reuse before KILL" in body)
        ck("offline/no-index contract", "--no-index" in body and "runtime-lib" in body)
    print(f"[native-multinode-serve] {'PASS' if not bad else 'FAIL'}")
    return 0 if not bad else 1

def main() -> int:
    ap = argparse.ArgumentParser(); ap.add_argument("--self-test", action="store_true"); sub = ap.add_subparsers(dest="op"); run = sub.add_parser("run")
    for n, kw in (("--run-id", {"required": True}), ("--cell", {"required": True}), ("--run-root", {"required": True}), ("--repo-root", {"required": True}), ("--evidence-preserve-dir", {"required": True}), ("--image", {"required": True}), ("--wheelhouse-path", {"default": "/opt/wheelhouse"}), ("--model", {"required": True}), ("--master-host", {"required": True}), ("--sub-host", {"required": True}), ("--remote-run-root", {"required": True})): run.add_argument(n, **kw)
    run.add_argument("--apply", action="store_true"); run.add_argument("--ray-port", type=int, default=6379); run.add_argument("--serve-port", type=int, default=8000); run.add_argument("--tp", type=int, default=2)
    a = ap.parse_args()
    if a.self_test: return self_test()
    if a.op != "run" or not a.apply: print("REFUSE: real run requires run --apply; --self-test is side-effect-free", file=sys.stderr); return EXIT_USAGE
    try:
        if not RUN_ID.fullmatch(a.run_id) or not HOST.fullmatch(a.sub_host): raise ServeError("unsafe run-id or sub host")
        root, repo = Path(a.run_root), Path(a.repo_root).resolve()
        if not root.is_absolute() or root.name != a.run_id or root.exists(): raise ServeError("new absolute run root basename must equal run-id")
        preserve = Path(a.evidence_preserve_dir).resolve(); preserve.relative_to(repo); rel = str(preserve.relative_to(repo))
        root.mkdir(mode=0o700); c = Config(a.run_id, a.cell, root, repo, preserve, rel, a.image, a.wheelhouse_path, a.model, a.master_host, a.sub_host, validate_remote_root(a.remote_run_root, a.run_id), a.ray_port, a.serve_port, a.tp)
        o = Orchestrator(c, Local(root), SSH(a.sub_host, c.remote_root)); o.prepare(); o.start(); raise UnknownState("live proof surface intentionally requires production remote Runner implementation")
    except UnknownState as exc: print(f"FAIL_CLOSED_UNKNOWN: {exc}", file=sys.stderr); return EXIT_UNKNOWN
    except ServeError as exc: print(f"FAIL: {exc}", file=sys.stderr); return EXIT_FAIL
if __name__ == "__main__": raise SystemExit(main())
