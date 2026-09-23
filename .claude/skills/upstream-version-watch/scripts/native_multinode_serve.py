#!/usr/bin/env python3
"""native 2노드 Ray/vLLM 서빙 정문 — `up` / `down` 2단 생명주기 (plan_26092311 N-D4 · N-D6 · N-D7).

Docker 경로(multinode_serve_smoke.sh)와 같은 안전 순서를 Docker 없이 호스트 venv 에서 밟는다:
  up   : 양 노드 RAM 게이트 → 예산 선판정·선언·honored → run root(마커 소유) → 각 노드가 **자기 이미지**에서
         wheelhouse 재포장 → 오프라인 venv 설치·검증 → Ray head/worker(+pgid 워치독) → `vllm serve`(native 트리플렛)
         → health 폴링(ready_max_seconds) → 추론 1회 → serve proof(발행기 native 계약) → 예산 갱신 루프(pgid)
         → state.json → 서빙을 **띄운 채** exit 0. run root 가 생긴 뒤 어느 단계든 실패하면 자동 `down`(AC-N5).
  down : state.json **만** 읽는다 → 워치독·갱신 루프 회수 → 소유 프로세스 트리 스냅숏 → PID+starttime 로만
         TERM→KILL → 로그·state 를 docs/simlog 보존 디렉터리로 복사(삭제 **전**) → 신선한 잔재 조회(pgid 자손 ·
         GPU compute app · 포트 listen) → 깨끗한 노드만 run root 삭제(마커 소유 트리만) → 예산 선언 회수
         → cleanup attestation(발행기 계약 모양 · 잔재 0 일 때만 PASS).

안전 원시(뼈대 cf8e9cf 에서 유지): 마커 소유 run root · safe_remove_owned_tree(깊이 우선 · symlink/마운트 거부) ·
PID+starttime 정지(이름 기반 사살 없음) · 서브는 **고정 원격 러너**(프로그램·인자 모두 base64 — 원격 셸 조각 ✗).

실행 평면은 `--apply` 가 있을 때만 건드린다. `--apply` 없는 `up` 은 노드별 명령 계획을 찍고 exit 0(dry-run ·
부수효과 0). `--self-test` 는 가짜 러너·임시 트리만 쓴다(실서빙·Ray·컨테이너·실프로세스 사살 없음).

동시 인터페이스(한 함수씩 — 바뀌면 그 함수만 고친다):
  wheelhouse_build_argv · wheelhouse_verify_argv · parse_verify_verdict  (native_wheelhouse.py)
  watchdog_argv · renew_argv                                              (host_safety pgid 모드 · 커밋 65bdaec)
"""
from __future__ import annotations

import argparse
import base64
import contextlib
import dataclasses
import datetime as _dt
import hashlib
import importlib.util
import json
import os
import re
import shlex
import signal
import stat
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Callable, Optional, Sequence

EXIT_OK, EXIT_USAGE, EXIT_FAIL, EXIT_UNKNOWN = 0, 2, 3, 4
RUN_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$")
HOST = re.compile(r"^[A-Za-z0-9_.@:-]+$")
MARKER = ".native-multinode-owned.json"
CREATED_BY = "native_multinode_serve.py/v1"
HERE = Path(__file__).resolve().parent
REPO_DEFAULT = HERE.parents[3]
TOPOLOGY = "multi"
# 짧은 소유 루트 — Ray 세션 소켓·vLLM zmq ipc 는 AF_UNIX 경로 107바이트 한도에 걸린다. run root 는 캠페인 선언
#   (저장소 아래 · 60자대)이라 그 아래 `session_…/sockets/plasma_store` 가 한도를 넘는다 → Ray temp·TMPDIR 만 여기에 둔다.
#   run root 와 같은 마커 소유 규약으로 만들고 지운다(basename == run-id).
SHORT_BASE = "/tmp/native-mn"
AF_UNIX_BUDGET = 107
RAY_SESSION_SOCKET_TAIL = len("/session_2026-09-23_12-30-45_123456_1234567/sockets/plasma_store")
HEALTH_INTERVAL_S = 5
RAY_JOIN_MAX_S = 300               # serve_runner.sh 와 같은 창(60회 × 5s)
BUDGET_EVENT_WAIT_S = 40           # multinode_serve_smoke.sh wait_budget_event 와 같은 창
WATCHDOG_THRESH_MIB = 10240        # Docker 경로 협역 워치독 인자와 같다(`mem_watchdog.sh <filter> 10240 2`)
WATCHDOG_INTERVAL_S = 2
STOP_WAIT_S = 30
SMOKE_PROMPT = "2+2= ? 숫자만 답하세요."


class ServeError(RuntimeError): pass
class UnknownState(ServeError): pass


def j(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def starttime(pid: int) -> Optional[str]:
    """/proc/<pid>/stat 의 starttime(마지막 ')' 뒤 20번째). 부재·좀비 = None(좀비는 이미 끝난 프로세스다)."""
    try:
        tail = Path(f"/proc/{pid}/stat").read_text(encoding="ascii").rsplit(")", 1)[1].split()
        return None if tail[0] in ("Z", "X") else tail[19]
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


# ── 프로세스 관측·정지 원시(메인 in-process 와 서브 고정 러너가 **같은 코드**를 쓴다) ──────────────────────
# 이름 기반 판정은 없다: 트리는 ppid 사슬 + pgid 소속, 정지는 (pid, starttime) 대조 후 그룹/단일 신호.
PROC = r'''
import json,os,signal,subprocess,time
def st(pid):
 try:
  t=open('/proc/%d/stat'%pid).read().rsplit(')',1)[1].split()
  return None if t[0] in ('Z','X') else t[19]
 except Exception:return None
def procs():
 out={}
 for d in os.listdir('/proc'):
  if not d.isdigit():continue
  try:
   t=open('/proc/%s/stat'%d).read().rsplit(')',1)[1].split()
   if t[0] in ('Z','X'):continue
   try:a=open('/proc/%s/cmdline'%d,'rb').read().replace(b'\0',b' ').decode('utf-8','replace').strip()[:300]
   except Exception:a=''
   out[int(d)]={'pid':int(d),'ppid':int(t[1]),'pgid':int(t[2]),'starttime':t[19],'argv':a}
  except Exception:pass
 return out
def tree(pids,pgids):
 ps=procs();kids={};gs=set(int(g) for g in pgids)
 for p in ps.values():kids.setdefault(p['ppid'],[]).append(p['pid'])
 seen=set();todo=[int(x) for x in pids if int(x) in ps]
 while todo:
  x=todo.pop()
  if x in seen:continue
  seen.add(x);todo.extend(kids.get(x,[]))
 for p in ps.values():
  if p['pgid'] in gs:seen.add(p['pid'])
 return [ps[x] for x in sorted(seen) if x in ps]
def gpu():
 p=subprocess.run(['nvidia-smi','--query-compute-apps=pid,process_name,used_memory','--format=csv,noheader,nounits'],text=True,capture_output=True,timeout=20)
 if p.returncode:raise RuntimeError('gpu-query rc=%d'%p.returncode)
 return [x.strip() for x in p.stdout.splitlines() if x.strip()]
def ports():
 p=subprocess.run(['ss','-ltnH'],text=True,capture_output=True,timeout=20)
 if p.returncode:raise RuntimeError('port-query rc=%d'%p.returncode)
 out=set()
 for line in p.stdout.splitlines():
  f=line.split()
  if len(f)>=4 and ':' in f[3]:
   try:out.add(int(f[3].rsplit(':',1)[1]))
   except ValueError:pass
 return sorted(out)
def stop(pid,pgid,start,wait,pid_only):
 pid,pgid,start=int(pid),int(pgid),str(start)
 got=st(pid)
 if got is None:return 'already-gone'
 if got!=start:raise RuntimeError('pid-reuse-before-term')
 def sig(s):
  try:
   if pid_only:os.kill(pid,s)
   else:os.killpg(pgid,s)
  except ProcessLookupError:pass
 sig(signal.SIGTERM)
 end=time.monotonic()+wait
 while time.monotonic()<end and st(pid)==start:time.sleep(.1)
 got=st(pid)
 if got is None:return 'term-exited'
 if got!=start:raise RuntimeError('pid-reuse-before-kill')
 sig(signal.SIGKILL)
 end=time.monotonic()+min(wait,10)
 while time.monotonic()<end and st(pid)==start:time.sleep(.1)
 if st(pid)==start:raise RuntimeError('survived-kill')
 return 'kill-exited'
'''
_PROC: dict = {}
exec(compile(PROC, "native_proc", "exec"), _PROC)  # noqa: S102 — 이 파일의 고정 상수(원격과 같은 바이트)

# 고정 원격 러너: 프로그램도 요청도 base64 로만 넘는다(원격 셸이 쪼갤 텍스트 ✗ — 뼈대는 프로그램 원문을 ssh argv 로
#   넘겨 원격 셸이 다시 단어 분리했다). 요청의 경로는 전부 root 아래 상대경로로만 받는다.
REMOTE = PROC + r'''
import base64,stat,sys
r=json.loads(base64.b64decode(sys.argv[2]));root=r['root'];rid=r['run_id'];marker=r['marker'];created=r['created_by'];op=r['op']
def die(x):
 sys.stderr.write(str(x)+'\n');raise SystemExit(3)
def emit(x):print(json.dumps(x,sort_keys=True),flush=True)
def want():return json.dumps({'run_id':rid,'created_by':created},sort_keys=True,separators=(',',':'))+'\n'
def under_rel(rel):
 if not isinstance(rel,str) or not rel or rel.startswith('/'):die('bad-relative-path')
 p=os.path.realpath(os.path.join(root,rel));b=os.path.realpath(root)
 if os.path.commonpath([b,p])!=b:die('escape')
 return p
def owned_root():
 if os.path.basename(root)!=rid or os.path.islink(root) or not os.path.isdir(root) or os.path.ismount(root):die('bad-root')
 if open(os.path.join(root,marker)).read()!=want():die('bad-marker')
def remove(d):
 if os.path.islink(d) or os.path.ismount(d):die('link/mount')
 for e in list(os.scandir(d)):
  p=under_rel(os.path.relpath(e.path,root));m=e.stat(follow_symlinks=False).st_mode
  if stat.S_ISLNK(m):die('symlink')
  if stat.S_ISDIR(m):remove(p)
  elif stat.S_ISREG(m):os.unlink(p)
  else:die('special')
 os.rmdir(d)
try:
 if op=='init':
  if os.path.lexists(root):die('root-exists')
  parent=os.path.dirname(root)
  if r.get('ensure_parent'):os.makedirs(parent,0o700,exist_ok=True)
  if not os.path.isdir(parent) or os.path.islink(parent):die('bad-parent')
  os.mkdir(root,0o700)
  with open(os.path.join(root,marker),'w') as f:f.write(want())
  emit({'ok':True})
 elif op=='put':
  owned_root();p=under_rel(r['rel'])
  if os.path.lexists(p):die('put-exists')
  os.makedirs(os.path.dirname(p),0o700,exist_ok=True)
  fd=os.open(p,os.O_WRONLY|os.O_CREAT|os.O_EXCL|getattr(os,'O_NOFOLLOW',0),int(r['mode']))
  with os.fdopen(fd,'wb') as f:f.write(base64.b64decode(r['data']))
  emit({'ok':True})
 elif op=='run':
  p=subprocess.run(r['argv'],stdin=subprocess.DEVNULL,text=True,capture_output=True,timeout=r['timeout'])
  emit({'rc':p.returncode,'out':p.stdout,'err':p.stderr})
 elif op=='start':
  owned_root();log=under_rel(r['log_rel']);os.makedirs(os.path.dirname(log),0o700,exist_ok=True)
  f=open(log,'xb',buffering=0)
  p=subprocess.Popen(r['argv'],stdin=subprocess.DEVNULL,stdout=f,stderr=subprocess.STDOUT,start_new_session=True,close_fds=True);f.close();x=st(p.pid)
  if x is None:die('starttime(process exited at start)')
  emit({'node':r['node'],'role':r['role'],'pid':p.pid,'pgid':os.getpgid(p.pid),'starttime':x,'log_rel':r['log_rel']})
 elif op=='stop':
  i=r['identity']
  emit({'node':i['node'],'role':i['role'],'pid':int(i['pid']),'term_status':stop(i['pid'],i['pgid'],i['starttime'],int(r['wait']),bool(r.get('pid_only'))),'starttime_match':True})
 elif op=='text':
  with open(under_rel(r['rel']),encoding='utf-8',errors='replace') as f:emit({'text':f.read()})
 elif op=='tree':emit({'processes':tree(r['pids'],r['pgids'])})
 elif op=='gpu':emit({'processes':gpu()})
 elif op=='ports':emit({'ports':ports()})
 elif op=='exists':emit({'exists':os.path.lexists(root)})
 elif op=='remove_owned_tree':
  owned_root();remove(root)
  if os.path.lexists(root):die('remaining')
  emit({'ok':True})
 else:die('bad-op')
except RuntimeError as e:die(e)
except subprocess.TimeoutExpired:die('bounded-timeout')
'''
BOOT = "import base64,sys;exec(compile(base64.b64decode(sys.argv[1]),'native_remote','exec'))"
REMOTE_B64 = base64.b64encode(REMOTE.encode()).decode()


@dataclasses.dataclass(frozen=True)
class Identity:
    node: str; role: str; pid: int; pgid: int; starttime: str; log_rel: str
    def row(self) -> dict[str, Any]: return dataclasses.asdict(self)


@dataclasses.dataclass
class Result:
    argv: list[str]; rc: int; out: str = ""; err: str = ""


class Runner:
    """한 노드의 한 소유 루트에 묶인 실행 평면. 경로 인자는 전부 root 아래 상대경로다."""
    node: str
    root: str
    def init_root(self, ensure_parent: bool = False) -> None: raise NotImplementedError
    def put(self, rel: str, data: bytes, mode: int = 0o600) -> None: raise NotImplementedError
    def run(self, argv: Sequence[str], timeout: int, check: bool = True) -> Result: raise NotImplementedError
    def start(self, argv: Sequence[str], log_rel: str, role: str) -> Identity: raise NotImplementedError
    def stop(self, ident: Identity, wait: int, pid_only: bool = False) -> dict[str, Any]: raise NotImplementedError
    def text(self, rel: str) -> str: raise NotImplementedError
    def tree(self, pids: Sequence[int], pgids: Sequence[int]) -> list[dict[str, Any]]: raise NotImplementedError
    def gpu(self) -> list[str]: raise NotImplementedError
    def ports(self) -> list[int]: raise NotImplementedError
    def exists(self) -> bool: raise NotImplementedError
    def remove_owned_tree(self, run_id: str) -> None: raise NotImplementedError


def _check_argv(argv: Sequence[str]) -> list[str]:
    if not argv or any(not isinstance(x, str) or "\0" in x for x in argv): raise ServeError("invalid exact argv")
    return list(argv)


class Local(Runner):
    def __init__(self, root: str, node: str = "main") -> None: self.root, self.node = str(root), node
    def _p(self, rel: str) -> Path:
        if not rel or rel.startswith("/"): raise ServeError("bad-relative-path")
        return beneath(Path(self.root), Path(self.root) / rel)
    def init_root(self, ensure_parent: bool = False) -> None:
        root = Path(self.root)
        if os.path.lexists(root): raise ServeError(f"{self.node}: root already exists: {root}")
        if ensure_parent: root.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        if root.parent.is_symlink() or not root.parent.is_dir(): raise ServeError(f"{self.node}: bad parent {root.parent}")
        root.mkdir(mode=0o700)
        (root / MARKER).write_bytes(marker_bytes(root.name))
    def put(self, rel: str, data: bytes, mode: int = 0o600) -> None:
        p = self._p(rel); p.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        fd = os.open(p, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0), mode)
        with os.fdopen(fd, "wb") as fh: fh.write(data)
    def run(self, argv: Sequence[str], timeout: int, check: bool = True) -> Result:
        argv = _check_argv(argv)
        try: p = subprocess.run(argv, stdin=subprocess.DEVNULL, text=True, capture_output=True, timeout=timeout)
        except subprocess.TimeoutExpired as exc: raise UnknownState(f"{self.node}: bounded command timed out: {argv[:3]}") from exc
        except FileNotFoundError as exc: raise ServeError(f"{self.node}: executable not found: {argv[0]}") from exc
        r = Result(argv, p.returncode, p.stdout, p.stderr)
        if check and r.rc: raise ServeError(f"{self.node}: rc={r.rc}: {argv[:4]}: {r.err[-800:]}")
        return r
    def start(self, argv: Sequence[str], log_rel: str, role: str) -> Identity:
        argv = _check_argv(argv)
        log = self._p(log_rel); log.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        with log.open("xb", buffering=0) as fd:     # run 마다 새 파일(append ✗ · F11 — 옛 오류가 새 것처럼 보였다)
            p = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=fd, stderr=subprocess.STDOUT,
                                 start_new_session=True, close_fds=True)
        st = starttime(p.pid)
        if st is None: raise ServeError(f"{self.node}: {role} exited at start (see {log_rel})")
        return Identity(self.node, role, p.pid, os.getpgid(p.pid), st, log_rel)
    def stop(self, ident: Identity, wait: int, pid_only: bool = False) -> dict[str, Any]:
        try: status = _PROC["stop"](ident.pid, ident.pgid, ident.starttime, wait, pid_only)
        except RuntimeError as exc: raise UnknownState(f"{self.node}: {ident.role}: {exc}") from exc
        return {"node": self.node, "role": ident.role, "pid": ident.pid, "term_status": status, "starttime_match": True}
    def text(self, rel: str) -> str: return self._p(rel).read_text(encoding="utf-8", errors="replace")
    def tree(self, pids: Sequence[int], pgids: Sequence[int]) -> list[dict[str, Any]]: return _PROC["tree"](pids, pgids)
    def gpu(self) -> list[str]:
        try: return _PROC["gpu"]()
        except (RuntimeError, OSError, subprocess.TimeoutExpired) as exc: raise UnknownState(f"{self.node}: fresh GPU query failed: {exc}") from exc
    def ports(self) -> list[int]:
        try: return _PROC["ports"]()
        except (RuntimeError, OSError, subprocess.TimeoutExpired) as exc: raise UnknownState(f"{self.node}: fresh port query failed: {exc}") from exc
    def exists(self) -> bool: return os.path.lexists(self.root)
    def remove_owned_tree(self, run_id: str) -> None: safe_remove_owned_tree(Path(self.root), run_id)


class SSH(Runner):
    def __init__(self, host: str, root: str, node: str = "sub", transport: Optional[list[str]] = None) -> None:
        if not HOST.fullmatch(host): raise ServeError("unsafe sub host")
        self.host, self.root, self.node = host, root, node
        # transport=[] 은 자체검사 전용(같은 고정 프로그램을 로컬 python 으로 돌린다).
        self.transport = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8", host] if transport is None else transport
    def _call(self, op: str, bound_s: int, **request: Any) -> dict[str, Any]:
        # bound_s = 이 호출의 상한(뼈대는 인자 이름이 요청 키 `timeout` 과 겹쳐 run 이 TypeError 로 죽었다)
        allowed = {"init", "put", "run", "start", "stop", "text", "tree", "gpu", "ports", "exists", "remove_owned_tree"}
        if op not in allowed: raise ServeError(f"sub fixed runner operation outside allowlist: {op}")
        request.update(op=op, root=self.root, run_id=Path(self.root).name, marker=MARKER, created_by=CREATED_BY)
        payload = base64.b64encode(j(request).encode()).decode()
        argv = ["python3", "-c", BOOT, REMOTE_B64, payload]
        if self.transport: argv = [*self.transport, shlex.join(argv)]   # 원격 셸이 받는 것은 인용된 고정 부트스트랩 + base64 뿐
        try: p = subprocess.run(argv, stdin=subprocess.DEVNULL, text=True, capture_output=True, timeout=bound_s + 13)
        except subprocess.TimeoutExpired as exc: raise UnknownState(f"{self.node}: fixed runner bounded timeout ({op})") from exc
        if p.returncode: raise UnknownState(f"{self.node}: fixed runner rejected {op}: {p.stderr.strip()[-500:]}")
        try: return json.loads(p.stdout)
        except json.JSONDecodeError as exc: raise UnknownState(f"{self.node}: fixed runner non-JSON result ({op})") from exc
    def init_root(self, ensure_parent: bool = False) -> None:
        if self._call("init", 30, ensure_parent=ensure_parent).get("ok") is not True: raise UnknownState(f"{self.node}: init non-pass")
    def put(self, rel: str, data: bytes, mode: int = 0o600) -> None:
        if self._call("put", 60, rel=rel, data=base64.b64encode(data).decode(), mode=mode).get("ok") is not True:
            raise UnknownState(f"{self.node}: put non-pass")
    def run(self, argv: Sequence[str], timeout: int, check: bool = True) -> Result:
        argv = _check_argv(argv)
        response = self._call("run", timeout, argv=argv, timeout=timeout)
        result = Result(argv, int(response["rc"]), str(response.get("out", "")), str(response.get("err", "")))
        if check and result.rc: raise ServeError(f"{self.node}: rc={result.rc}: {argv[:4]}: {result.err[-800:]}")
        return result
    def start(self, argv: Sequence[str], log_rel: str, role: str) -> Identity:
        return Identity(**self._call("start", 30, argv=_check_argv(argv), log_rel=log_rel, role=role, node=self.node))
    def stop(self, ident: Identity, wait: int, pid_only: bool = False) -> dict[str, Any]:
        return self._call("stop", wait + 15, identity=ident.row(), wait=wait, pid_only=pid_only)
    def text(self, rel: str) -> str: return str(self._call("text", 30, rel=rel)["text"])
    def tree(self, pids: Sequence[int], pgids: Sequence[int]) -> list[dict[str, Any]]:
        return list(self._call("tree", 30, pids=list(pids), pgids=list(pgids))["processes"])
    def gpu(self) -> list[str]: return list(self._call("gpu", 25)["processes"])
    def ports(self) -> list[int]: return list(self._call("ports", 25)["ports"])
    def exists(self) -> bool: return bool(self._call("exists", 20)["exists"])
    def remove_owned_tree(self, run_id: str) -> None:
        if Path(self.root).name != run_id: raise ServeError("owned-tree basename/run-id mismatch")
        if self._call("remove_owned_tree", 120).get("ok") is not True: raise UnknownState(f"{self.node}: owned-tree removal non-pass")


# ── 선언·사실 읽기 ────────────────────────────────────────────────────────────────────────────────────
def _load_module(name: str, path: Path):
    if name in sys.modules: return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None: raise ServeError(f"module load failed: {path}")
    mod = importlib.util.module_from_spec(spec); sys.modules[name] = mod; spec.loader.exec_module(mod)
    return mod


def _read_env(path: Path) -> dict[str, str]:
    # 저장소의 env 파서 한 벌(slave_forward.read_env_first · 첫 일치 · 원문) — 사본을 만들지 않는다.
    sf = _load_module("_nm_slave_forward", HERE / "slave_forward.py")
    if not path.is_file(): raise ServeError(f"env file absent: {path}")
    return {k: str(v) for k, v in sf.read_env_first(path).items()}


@dataclasses.dataclass
class NodeSpec:
    name: str; node_id: str; host_ip: str; work_dir: str; root: str; short: str
    session_py: str; renew_sh: str; watchdog_sh: str; node_dir: str; wheel_tool: str
    def row(self) -> dict[str, Any]: return dataclasses.asdict(self)


@dataclasses.dataclass
class Ctx:
    repo: Path; cell: str; source_cell: str; run_id: str; campaign_id: str
    base: str; max_bytes: int; sub_host: str; ready_max_s: int; overhead_mib: int
    serve_port: int; ray_port: int; model_name: str; object_store: str; image: str
    cell_env: dict; cluster_env: dict; ic_env: dict; serve_env_keys: list
    triplet: dict; nodes: dict; proof_rel: str; simlog_topic: str; wheel_tool_rel: str


def load_ctx(repo: Path, cell: str, run_id: str, *, source_cell: Optional[str] = None,
             ready_max_s: Optional[int] = None, simlog_topic: str = "native_N1") -> Ctx:
    import yaml
    repo = repo.resolve()
    if not RUN_ID.fullmatch(run_id): raise ServeError("unsafe run-id")
    active = (repo / "campaigns" / "ACTIVE").read_text(encoding="utf-8").strip() if (repo / "campaigns" / "ACTIVE").is_file() else ""
    if not active or active == "_bootstrap": raise ServeError("활성 캠페인이 없다(campaigns/ACTIVE) — native 선언을 읽을 곳이 없다")
    camp = yaml.safe_load((repo / "campaigns" / active / "campaign.yaml").read_text(encoding="utf-8"))
    sec = ((camp.get("topology_sections") or {}).get(TOPOLOGY) or {})
    if cell not in (sec.get("native_cells") or []): raise ServeError(f"{cell} 는 캠페인 {active} 의 native_cells 가 아니다")
    base, max_gib, sub_host = sec.get("native_run_root_base"), sec.get("native_run_root_max_gib_per_node"), sec.get("native_sub_host")
    if not (isinstance(base, str) and base.startswith("/")) or not isinstance(max_gib, (int, float)) or max_gib <= 0 or not isinstance(sub_host, str):
        raise ServeError("캠페인 topology_sections.multi 의 native_run_root_base/_max_gib_per_node/native_sub_host 선언이 불완전하다")
    budgets = camp.get("budgets") or {}
    rmax = int(ready_max_s if ready_max_s is not None else budgets.get("ready_max_seconds") or 0)
    overhead = int(budgets.get("smoke_budget_overhead_mib") or 0)
    if rmax <= 0 or overhead <= 0: raise ServeError("budgets.ready_max_seconds·smoke_budget_overhead_mib 선언이 없다(기본값 ✗)")
    source = source_cell or (cell[: -len("-native")] if cell.endswith("-native") else "")
    if not source: raise ServeError("원본 Docker 셀을 파생할 수 없다(--source-cell 또는 `<셀>-native` 이름)")
    rn = _load_module("_nm_render_native", HERE / "render_native_triplet.py")
    try:
        drift = rn.check(repo, rn.render(repo, TOPOLOGY, source, cell))
    except rn.RenderError as exc:
        raise ServeError(f"native 트리플렛 렌더 거부: {exc}") from exc
    if drift: raise ServeError("native 트리플렛이 렌더와 다르다(손편집·원본 변경) — render_native_triplet.py --apply: " + "; ".join(drift))
    out = repo / "output" / TOPOLOGY
    triplet = rn.triplet_rel(TOPOLOGY, cell)
    cell_env = _read_env(repo / triplet["env"])
    src_env = _read_env(out / "envs" / f".env.{source}")
    cluster_env = _read_env(out / "envs" / ".env.cluster")
    ic_env = _read_env(out / "envs" / ".env.interconnect")
    sf = _load_module("_nm_slave_forward", HERE / "slave_forward.py")
    serve_env_keys = list(sf.candidates(out / "docker-compose.yaml", out / "envs" / ".env.cluster").get("serve_env") or [])
    image = src_env.get("IMAGE_TAG", "").strip()
    if not image: raise ServeError(f"원본 셀 env 에 IMAGE_TAG 가 없다 — wheelhouse 의 원천 이미지를 알 수 없다")
    man = yaml.safe_load((out / "manifest.yaml").read_text(encoding="utf-8")) or {}
    nodes_m = {n.get("role"): n for n in (man.get("nodes") or []) if isinstance(n, dict)}
    if set(nodes_m) < {"main", "sub"}: raise ServeError("manifest nodes[] 에 main·sub 가 없다")
    msub = nodes_m["sub"]
    if f"{msub.get('ssh_user')}@{msub.get('host')}" != sub_host:
        raise ServeError(f"캠페인 native_sub_host 가 manifest nodes[sub] 와 다르다")
    if not HOST.fullmatch(sub_host): raise ServeError("unsafe sub host")
    main_ip, sub_ip = str(nodes_m["main"].get("host") or ""), str(msub.get("host") or "")
    if cluster_env.get("MASTER_HOST_IP") not in (None, main_ip) or cluster_env.get("SLAVE_HOST_IP") not in (None, sub_ip):
        raise ServeError(".env.cluster 의 MASTER/SLAVE_HOST_IP 가 manifest nodes[] 와 다르다")
    sub_wd = str(msub.get("work_dir") or "")
    if not sub_wd.startswith("/"): raise ServeError("manifest nodes[sub].work_dir 가 절대경로가 아니다")
    root = validate_remote_root(f"{base.rstrip('/')}/{run_id}", run_id)
    short = validate_remote_root(f"{SHORT_BASE}/{run_id}", run_id)
    if len(short) + len("/ray") + RAY_SESSION_SOCKET_TAIL > AF_UNIX_BUDGET:
        raise ServeError(f"run-id 가 길어 Ray 소켓 경로가 AF_UNIX {AF_UNIX_BUDGET}B 를 넘는다 — 더 짧은 --run-id")
    nb_main = ".claude/skills/terraforming_node/scripts"
    nodes = {
        "main": NodeSpec("main", "main", main_ip, str(repo), root, short,
                         f"{repo}/{nb_main}/node_blackbox/blackbox_session.py", f"{repo}/{nb_main}/node_blackbox/budget_renew_loop.sh",
                         f"{repo}/{nb_main}/host_safety/mem_watchdog.sh", f"{repo}/docs/logs/main",
                         f"{repo}/.claude/skills/upstream-version-watch/scripts/native_wheelhouse.py"),
        "sub": NodeSpec("sub", "sub", sub_ip, sub_wd, root, short,
                        f"{sub_wd}/.claude/runtime/node_blackbox/blackbox_session.py", f"{sub_wd}/.claude/runtime/node_blackbox/budget_renew_loop.sh",
                        f"{sub_wd}/.claude/runtime/host_safety/mem_watchdog.sh", f"{sub_wd}/docs/logs/sub",
                        f"{root}/tools/native_wheelhouse.py"),   # 서브는 메인 바이트를 run root 로 put(배달 경로 무관 · sha 기록)
    }
    y = yaml.safe_load((repo / triplet["yaml"]).read_text(encoding="utf-8")) or {}
    port = int(cell_env.get("SERVING_PORT") or 0)
    if port <= 0 or int(y.get("port") or 0) != port: raise ServeError("셀 SERVING_PORT 와 yaml port 가 다르다")
    ray_port = int(cluster_env.get("RAY_PORT") or 6379)
    return Ctx(repo=repo, cell=cell, source_cell=source, run_id=run_id, campaign_id=active, base=base, max_bytes=int(max_gib * (1 << 30)),
               sub_host=sub_host, ready_max_s=rmax, overhead_mib=overhead, serve_port=port, ray_port=ray_port,
               model_name=str(cell_env.get("SERVING_MODEL_NAME") or ""), object_store=str(cluster_env.get("RAY_OBJECT_STORE_MEMORY") or "2000000000"),
               image=image, cell_env=cell_env, cluster_env=cluster_env, ic_env=ic_env, serve_env_keys=serve_env_keys,
               triplet=triplet, nodes=nodes, proof_rel=f"output/{TOPOLOGY}/benchlog/serve_proof_{cell}.json", simlog_topic=simlog_topic,
               wheel_tool_rel=".claude/skills/upstream-version-watch/scripts/native_wheelhouse.py")


# ── 명령 빌더(순수 함수 · dry-run 과 실행이 **같은** 함수를 쓴다) ─────────────────────────────────────────
def wh_dir(root: str) -> str: return f"{root}/wh"
def venv_dir(root: str) -> str: return f"{root}/venv"


def wheelhouse_build_argv(tool: str, image: str, root: str, generated_utc: str) -> list[str]:
    """native_wheelhouse.py build — 각 노드가 **자기 로컬 이미지**에서 재포장(N-D1)."""
    return ["python3", tool, "build", "--image", image, "--out", wh_dir(root), "--generated-utc", generated_utc]


def wheelhouse_verify_argv(tool: str, root: str, generated_utc: str) -> list[str]:
    """native_wheelhouse.py verify — venv 생성(python3.12)·`pip install --no-index`·pip check·ldd·import+CUDA 를 **도구가** 소유한다
    (비어 있지 않은 venv 는 거부하므로 여기서 venv 를 먼저 만들지 않는다)."""
    return ["python3", tool, "verify", "--wheelhouse-dir", wh_dir(root), "--venv", venv_dir(root), "--generated-utc", generated_utc]


def parse_verify_verdict(text: str) -> tuple[bool, dict, str]:
    """verify 의 JSON 판정 → (통과, 런타임 env 레시피, 사유). `verdict`=PASS 이고 선택된 후보의 env(LD_LIBRARY_PATH 필수)가
    있을 때만 통과다 — 레시피가 없으면 추측하지 않고 불통과로 본다."""
    doc = None
    for cand in (text.strip(), (text.strip().splitlines() or [""])[-1]):   # 전체 JSON 또는 마지막 줄 JSON
        with contextlib.suppress(json.JSONDecodeError):
            doc = json.loads(cand); break
    if not isinstance(doc, dict): return False, {}, "verify 출력이 JSON object 가 아니다"
    verdict = str(doc.get("verdict") or "").upper()
    env = dict(doc.get("env") or {}) if isinstance(doc.get("env"), dict) else {}
    ld = env.get("LD_LIBRARY_PATH") or doc.get("ld_library_path")
    if verdict != "PASS": return False, {}, f"verify verdict={verdict or '없음'} (selected={doc.get('selected')!r})"
    if not isinstance(ld, str) or not ld.strip(): return False, {}, "verify 가 LD_LIBRARY_PATH 레시피를 내지 않았다"
    env["LD_LIBRARY_PATH"] = ld.strip()
    return True, {str(k): str(v) for k, v in env.items()}, f"PASS({doc.get('selected')})"


def pip_freeze_argv(root: str) -> list[str]: return [f"{venv_dir(root)}/bin/python", "-m", "pip", "freeze", "--all"]


def watchdog_argv(spec: NodeSpec, target: Identity) -> list[str]:
    """host_safety/mem_watchdog.sh pgid 모드(커밋 65bdaec) — 그 그룹에만 TERM→KILL(이름 ✗ · C6)."""
    return ["bash", spec.watchdog_sh, "--pgid", str(target.pgid), "--starttime", target.starttime,
            "--thresh-mib", str(WATCHDOG_THRESH_MIB), "--interval-s", str(WATCHDOG_INTERVAL_S)]


def renew_argv(spec: NodeSpec, target: Identity, ttl_s: int) -> list[str]:
    """node_blackbox/budget_renew_loop.sh pgid 모드 — 리더가 사라지면 스스로 끝난다."""
    return ["bash", spec.renew_sh, "--node-dir", spec.node_dir, "--pgid", str(target.pgid), "--starttime", target.starttime,
            "--ttl-s", str(ttl_s)]


CACHE_DIRS = {"XDG_CACHE_HOME": "cache/xdg", "TRITON_CACHE_DIR": "cache/triton", "TORCHINDUCTOR_CACHE_DIR": "cache/inductor",
              "FLASHINFER_WORKSPACE_BASE": "cache/flashinfer", "VLLM_CACHE_ROOT": "cache/vllm", "HF_HOME": "cache/hf",
              "PIP_CACHE_DIR": "cache/pip", "CUDA_CACHE_PATH": "cache/nv", "TORCH_EXTENSIONS_DIR": "cache/torch_extensions",
              "HOME": "home"}


def runtime_env(ctx: Ctx, spec: NodeSpec, verify_env: dict[str, str]) -> dict[str, str]:
    """`env -i` 로 넘길 전체 환경 — 호출자 셸·HOME 에서 아무것도 새지 않는다(N-D7).
    우선순위는 compose 와 같다: .env.cluster < .env.interconnect < 셀 env(메인) / serve_env 키(서브 · 트리플렛 미전파 ·
    policy:MODEL_TRIPLET_NO_SUB_PROPAGATION) — 그 위에 verify 가 고른 런타임 레시피(LD_LIBRARY_PATH·번들 CUDA 도구 경로 등),
    그 위에 격리 키가 **이긴다**(셀 env 가 캐시를 run root 밖으로 돌리지 못한다). PATH 는 run root 안 항목만 verify 에서 받는다."""
    env: dict[str, str] = {}
    env.update(ctx.cluster_env); env.update(ctx.ic_env)
    if spec.name == "main": env.update(ctx.cell_env)
    else: env.update({k: ctx.cell_env[k] for k in ctx.serve_env_keys if ctx.cell_env.get(k, "") != ""})
    env.update({k: v for k, v in verify_env.items() if k != "PATH"})
    owned_path = [p for p in verify_env.get("PATH", "").split(":") if p.startswith(spec.root + "/") and p != f"{venv_dir(spec.root)}/bin"]
    env.update({k: f"{spec.root}/{v}" for k, v in CACHE_DIRS.items()})
    env.update({"TMPDIR": f"{spec.short}/tmp", "PATH": ":".join([*owned_path, f"{venv_dir(spec.root)}/bin", "/usr/local/bin:/usr/bin:/bin"]),
                "LANG": "C.UTF-8", "VLLM_HOST_IP": spec.host_ip, "HF_HUB_OFFLINE": "1",
                "TRANSFORMERS_OFFLINE": "1", "VLLM_NO_USAGE_STATS": "1", "DO_NOT_TRACK": "1", "RAY_USAGE_STATS_ENABLED": "0",
                "NATIVE_VENV": venv_dir(spec.root)})
    return env


def env_argv(env: dict[str, str], argv: list[str]) -> list[str]:
    return ["env", "-i", *[f"{k}={v}" for k, v in sorted(env.items())], *argv]


def mkdirs_argv(spec: NodeSpec) -> list[str]:
    return ["mkdir", "-p", *[f"{spec.root}/{v}" for v in sorted(set(CACHE_DIRS.values()))], f"{spec.root}/logs", f"{spec.short}/tmp", f"{spec.short}/ray"]


def ray_head_argv(ctx: Ctx, spec: NodeSpec) -> list[str]:
    return [f"{venv_dir(spec.root)}/bin/ray", "start", "--head", "--node-ip-address", spec.host_ip, "--port", str(ctx.ray_port),
            "--object-store-memory", ctx.object_store, "--temp-dir", f"{spec.short}/ray", "--disable-usage-stats", "--block"]


def ray_worker_argv(ctx: Ctx, spec: NodeSpec) -> list[str]:
    # 워커의 --temp-dir 는 Ray 가 무시한다(헤드의 경로를 쓴다) — 같은 문자열 경로를 서브 짧은 루트에 미리 만든다.
    return [f"{venv_dir(spec.root)}/bin/ray", "start", "--address", f"{ctx.nodes['main'].host_ip}:{ctx.ray_port}",
            "--node-ip-address", spec.host_ip, "--object-store-memory", ctx.object_store, "--disable-usage-stats", "--block"]


def ray_status_argv(ctx: Ctx, spec: NodeSpec) -> list[str]:
    return [f"{venv_dir(spec.root)}/bin/ray", "status", "--address", f"{spec.host_ip}:{ctx.ray_port}"]


def serve_argv(ctx: Ctx) -> list[str]:
    return ["bash", str(ctx.repo / ctx.triplet["sh"])]


def native_install_script(ctx: Ctx, tool_sha: str, generated_utc: str) -> str:
    """재현 설치 명령 — 위 빌더를 `$RUN_ROOT`/`$IMAGE` 자리표시로 다시 부른 결과(실행된 argv 와 같은 함수 · 운영자 경로 ✗)."""
    def q(tok: str) -> str:
        return f'"{tok}"' if "${" in tok else shlex.quote(tok)
    root, image, tool = "${RUN_ROOT}", "${IMAGE}", "${WH_TOOL}"
    lines = [
        "#!/bin/bash",
        f"# native-install.sh — {ctx.cell} native 설치 재현 명령(native_multinode_serve.py up 이 run {ctx.run_id} 에서 **각 노드마다** 실행한 순서).",
        "#   각 노드가 자기 로컬 이미지에서 wheelhouse 를 재포장하고(이미지 전송 ✗) 오프라인으로만 설치한다(--no-index · 다운로드 ✗).",
        f"#   provenance: 원천 이미지 태그 {ctx.image} · wheelhouse 도구 sha256={tool_sha} · generated_utc={generated_utc}",
        "#   사용: bash native-install.sh <run-root> [image] — 저장소 루트에서 실행(도구는 저장소 상대경로).",
        "set -euo pipefail",
        'RUN_ROOT="${1:?사용: native-install.sh <run-root> [image]}"',
        f'IMAGE="${{2:-{ctx.image}}}"',
        f"WH_TOOL={shlex.quote(ctx.wheel_tool_rel)}",
    ]
    lines.append("# ① 재포장(docker create → 설치 트리 cp → RECORD 대조 재포장 · 시작하지 않는 컨테이너)")
    lines.append(" ".join(q(t) for t in wheelhouse_build_argv(tool, image, root, generated_utc)))
    lines.append("# ② 설치 게이트 — 도구가 python3.12 -m venv → pip install --no-index --find-links wh/wheelhouse -r wh/requirements-closure.txt")
    lines.append("#    → pip check → ldd(vllm *.so) → import vllm,torch + cuda 를 실행하고 런타임 env 레시피(LD_LIBRARY_PATH …)를 고른다")
    lines.append(" ".join(q(t) for t in wheelhouse_verify_argv(tool, root, generated_utc)))
    return "\n".join(lines) + "\n"


def _utc(t: float) -> str: return _dt.datetime.fromtimestamp(t, _dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _parse_utc(s: str) -> Optional[float]:
    try: return _dt.datetime.strptime(s, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=_dt.timezone.utc).timestamp()
    except (TypeError, ValueError): return None


def _http(method: str, url: str, body: Optional[dict], timeout: int) -> tuple[int, str]:
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method, headers={"Content-Type": "application/json"} if data else {})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return int(resp.status), resp.read().decode("utf-8", errors="replace")
    except urllib.error.HTTPError as exc: return int(exc.code), ""
    except (urllib.error.URLError, OSError, TimeoutError): return 0, ""


class Clock:
    def now(self) -> float: return time.time()
    def mono(self) -> float: return time.monotonic()
    def sleep(self, s: float) -> None: time.sleep(s)


# ── 정문 ──────────────────────────────────────────────────────────────────────────────────────────────
class NativeServe:
    def __init__(self, ctx: Ctx, runners: dict[str, Runner], short_runners: dict[str, Runner], *,
                 http: Callable[..., tuple[int, str]] = _http, clock: Clock = Clock(), log: Callable[[str], None] = print) -> None:
        self.c, self.r, self.rs, self.http, self.clock, self.log = ctx, runners, short_runners, http, clock, log
        self.state: dict[str, Any] = {}
        self.budget_declared = {"main": False, "sub": False}

    # ---- 공용 ----
    def _state_write(self) -> None:
        if not self.state: return
        path = Path(self.c.nodes["main"].root) / "state.json"
        if not path.parent.is_dir(): return
        tmp = path.with_name("state.json.tmp")
        tmp.write_text(json.dumps(self.state, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        tmp.replace(path)

    def _add_ident(self, ident: Identity, kind: str, target: Optional[str] = None) -> None:
        row = {**ident.row(), "kind": kind}
        if target: row["target_role"] = target
        self.state.setdefault("identities", []).append(row); self._state_write()

    def _preserve(self, rel: str, data: bytes, *, overwrite: bool = False) -> str:
        pdir = self.c.repo / self.state["preserve_rel"]
        dst = beneath(pdir, pdir / rel); dst.parent.mkdir(parents=True, exist_ok=True)
        if dst.is_symlink(): raise ServeError(f"preserve target is symlink: {rel}")
        if overwrite:
            tmp = dst.with_name(dst.name + ".tmp"); tmp.write_bytes(data); tmp.replace(dst)
        else:
            with dst.open("xb") as fh: fh.write(data)          # 증거는 덮어쓰지 않는다(append ✗ · 새 파일만)
        return str(dst.relative_to(self.c.repo))

    def _write_repo_json(self, rel: str, doc: dict) -> None:
        p = self.c.repo / rel; p.parent.mkdir(parents=True, exist_ok=True)
        tmp = p.with_name(p.name + ".tmp")
        tmp.write_text(json.dumps(doc, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8"); tmp.replace(p)

    # ---- 예산 ----
    def _wait_event(self, name: str, kind: str, t0: float) -> Optional[str]:
        ev = f"{self.c.nodes[name].node_dir}/events/watchdog.jsonl"
        end = self.clock.mono() + BUDGET_EVENT_WAIT_S
        while True:
            r = self.r[name].run(["tail", "-n", "30", ev], 20, check=False)
            for line in reversed(r.out.splitlines()):
                if f'"{kind}"' in line:
                    m = re.search(r'"ts"\s*:\s*"([^"]+)"', line)
                    ts = _parse_utc(m.group(1)) if m else None
                    if ts is not None and ts >= t0: return line
                    break
            if self.clock.mono() >= end: return None
            self.clock.sleep(1)

    def _session(self, name: str, *args: str) -> list[str]:
        spec = self.c.nodes[name]
        return ["python3", spec.session_py, "--node-dir", spec.node_dir, *args, "--now", _utc(self.clock.now())]

    def _declare(self, name: str, p: dict) -> None:
        spec, r = self.c.nodes[name], self.r[name]
        mem = self._meminfo(name)
        had = r.run(["test", "-f", f"{spec.node_dir}/serve_budget.env"], 15, check=False).rc == 0
        t_clear = int(self.clock.now())
        r.run(self._session(name, "clear-budget"), 30)
        if had and self._wait_event(name, "budget_none", t_clear) is None:
            raise ServeError(f"{name}: clear-budget 상태 전이 미관측")
        t0 = int(self.clock.now())
        r.run(self._session(name, "declare-budget", "--mem-total-mib", str(mem["MemTotal"]), "--weights-mib", str(p["weights_mib"]),
                            "--kv-mib", str(p["kv_mib"]), "--overhead-mib", str(self.c.overhead_mib), "--ttl-s", str(p["ttl_s"]),
                            "--expected-load-s", str(self.c.ready_max_s), "--label", f"smoke-{self.c.cell}"), 30)
        self.budget_declared[name] = True
        hon = self._wait_event(name, "budget_honored", t0)
        if hon is None:
            raise ServeError(f"{name}: budget_honored 미검출({BUDGET_EVENT_WAIT_S}s) — 워치독이 선언을 수락하지 않았다(진입 차단)")
        self.log(f"[native] {name}: budget_honored ✓")

    def _clear_budget(self, name: str) -> Optional[str]:
        try:
            self.r[name].run(self._session(name, "clear-budget"), 30)
            return None
        except ServeError as exc:
            return f"{name}: clear-budget 실패: {exc}"

    def _meminfo(self, name: str) -> dict[str, int]:
        out = self.r[name].run(["cat", "/proc/meminfo"], 15).out
        vals = {m.group(1): int(m.group(2)) // 1024 for m in re.finditer(r"(?m)^(\w+):\s+(\d+)\s+kB", out)}
        if "MemTotal" not in vals or "MemAvailable" not in vals: raise ServeError(f"{name}: /proc/meminfo 해석 실패")
        return vals

    # ---- up 단계 ----
    def gates_and_budget(self) -> dict:
        c = self.c
        r = self.r["main"].run(["python3", str(HERE / "check_smoke_model.py"), c.source_cell, "--repo", str(c.repo),
                                "--topology", TOPOLOGY, "--emit-gate-params"], 600, check=False)
        text = r.out + r.err
        if r.rc == 7: raise ServeError("로드-전 RAM 게이트 거부(메인) — 모델 실재·가용 RAM 부족(다운로드 불요)")
        if r.rc == 2: raise ServeError("스모크 모델 부재 — 다운로드 금지, 중단")
        if r.rc != 0: raise ServeError(f"NAS/설정 확인 실패(rc={r.rc}): {text[-400:]}")
        bp = re.search(r"BUDGET_PARAMS ckpt_mib=(\d+) tp=(\d+) kv_mib=(\S+) ple_mib=(\S+)", text)
        req = re.search(r"GATE_PARAMS required_mib=(\d+)", text)
        if not bp or not req: raise ServeError("BUDGET_PARAMS/GATE_PARAMS 미검출 — 예산 선언 입력을 파생할 수 없다(진입 차단)")
        ckpt, tp, kv, ple = int(bp.group(1)), int(bp.group(2)), bp.group(3), bp.group(4)
        if not kv.isdigit() or tp <= 0: raise ServeError("kv_mib/tp 파생 실패 — 트리플렛 kv-cache-memory-bytes 확인")
        required = int(req.group(1))
        avail = self._meminfo("sub")["MemAvailable"]
        if avail < required:
            dc = "/usr/local/sbin/vllm-drop-caches"
            if self.r["sub"].run(["test", "-x", dc], 15, check=False).rc == 0:
                self.r["sub"].run(["sudo", "-n", dc], 60, check=False)
            avail = self._meminfo("sub")["MemAvailable"]
        if avail < required:
            raise ServeError(f"슬레이브 로드-전 RAM 게이트 거부 — MemAvailable={avail}MiB < required={required}MiB")
        self.log(f"[native] RAM 게이트 PASS: main(check_smoke_model) · sub MemAvailable={avail}MiB ≥ {required}MiB")
        resident, note = ckpt, "ple=resident"
        if str(c.cell_env.get("VLLM_PLE_MMAP", "")).strip() == "1":
            if ple.isdigit() and int(ple) > 0: resident, note = ckpt - int(ple), f"ple=mmap(−{ple}MiB)"
            else: note = "ple=mmap선언·크기미상(보정 0)"
        weights = resident // tp
        floor = self.r["main"].run(["python3", c.nodes["main"].session_py, "--node-dir", ".", "budget-defaults", "--field", "ttl_s"], 30).out.strip()
        if not floor.isdigit(): raise ServeError("예산 TTL 기본값을 blackbox_session 에서 읽지 못했다")
        ttl = min(max(c.ready_max_s * 3, int(floor)), 86400)
        pf = self.r["main"].run(["python3", str(HERE / "budget_preflight.py"), "--json", "--declared-gmu-yaml", str(c.repo / c.triplet["yaml"]),
                                 "--mem-total-mib", str(self._meminfo("main")["MemTotal"]), "--weights-mib", str(weights),
                                 "--kv-mib", kv, "--overhead-mib", str(c.overhead_mib)], 60, check=False)
        if pf.rc == 4: raise ServeError(f"예산 선판정: 워치독이 이 선언을 거부한다(arm 상한 < 최소) — {pf.out[-300:]}")
        if pf.rc != 0: raise ServeError(f"예산 선판정 실패(rc={pf.rc})")
        params = {"weights_mib": weights, "kv_mib": int(kv), "ttl_s": ttl, "note": note}
        self.log(f"[native] 예산 파생: weights={weights}MiB({note}) kv={kv}MiB overhead={c.overhead_mib}MiB ttl={ttl}s")
        for name in ("main", "sub"):
            self._declare(name, params)
        return params

    def preflight_tools(self) -> None:
        """로드 전에 **배선 존재**를 본다 — 서브에 pgid 모드가 아직 배달되지 않았으면(S2.5 전) 여기서 멈춘다."""
        for name in ("main", "sub"):
            spec = self.c.nodes[name]
            for path in (spec.session_py, spec.renew_sh, spec.watchdog_sh):
                if self.r[name].run(["test", "-f", path], 15, check=False).rc != 0:
                    raise ServeError(f"{name}: 필수 도구 부재: {path} (서브는 sync_to_sub 배달 필요)")
            for path in (spec.renew_sh, spec.watchdog_sh):
                if self.r[name].run(["grep", "-q", "--", "--pgid", path], 15, check=False).rc != 0:
                    raise ServeError(f"{name}: {path} 에 pgid 표적 모드가 없다(커밋 65bdaec 미배달)")
        if not (self.c.repo / self.c.wheel_tool_rel).is_file():
            raise ServeError(f"wheelhouse 도구 부재: {self.c.wheel_tool_rel}")

    def make_roots(self, generated_utc: str) -> None:
        c = self.c
        naming = _load_module("_nm_doc_naming", REPO_DEFAULT / ".claude/skills/wiki-desk/scripts/doc_naming.py")
        simlog = c.repo / "docs" / "simlog"
        existing = [p.name for p in simlog.iterdir()] if simlog.is_dir() else []
        preserve_rel = f"docs/simlog/{naming.simlog_dirname(generated_utc, c.simlog_topic, existing)}"
        self.state = {"schema_version": 1, "kind": "native_multinode_state", "run_id": c.run_id, "cell": c.cell,
                      "campaign_id": c.campaign_id, "created_utc": generated_utc, "sub_host": c.sub_host,
                      "nodes": {k: v.row() for k, v in c.nodes.items()}, "ports": {"serve": c.serve_port, "ray": c.ray_port},
                      "preserve_rel": preserve_rel, "proof_rel": c.proof_rel,
                      "attestation_rel": f"{preserve_rel}/cleanup_attestation.json",
                      "budget_declared": dict(self.budget_declared), "roots_created": {}, "identities": []}
        for name in ("main", "sub"):
            for kind, runner in (("run", self.r[name]), ("short", self.rs[name])):
                runner.init_root(ensure_parent=True)
                self.state["roots_created"].setdefault(name, []).append(kind)
                if name == "main" and kind == "run": self._state_write()
        self._state_write()
        (c.repo / preserve_rel).mkdir(parents=True, exist_ok=False)
        for name in ("main", "sub"):
            self.r[name].run(mkdirs_argv(c.nodes[name]), 30)

    def _du_guard(self, name: str, stage: str) -> None:
        out = self.r[name].run(["du", "-sxb", self.c.nodes[name].root], 600).out.split()
        used = int(out[0]) if out and out[0].isdigit() else -1
        if used < 0: raise ServeError(f"{name}: run root 크기 측정 실패({stage})")
        if used > self.c.max_bytes:
            raise ServeError(f"{name}: run root {used / (1 << 30):.1f}GiB > 상한 {self.c.max_bytes / (1 << 30):.0f}GiB ({stage})")
        self.state.setdefault("root_bytes", {}).setdefault(name, {})[stage] = used

    def install(self, generated_utc: str) -> dict[str, dict]:
        c = self.c
        tool_bytes = (c.repo / c.wheel_tool_rel).read_bytes()
        tool_sha = hashlib.sha256(tool_bytes).hexdigest()
        self.r["sub"].put("tools/native_wheelhouse.py", tool_bytes, 0o600)
        self.state["wheel_tool_sha256"] = tool_sha
        lds: dict[str, str] = {}
        for name in ("main", "sub"):
            spec, r = c.nodes[name], self.r[name]
            self.log(f"[native] {name}: wheelhouse 재포장(자기 이미지 {c.image})…")
            b = r.run(wheelhouse_build_argv(spec.wheel_tool, c.image, spec.root, generated_utc), 7200, check=False)
            self._preserve(f"wheelhouse-build-{name}.json", (b.out or b.err).encode())
            if b.rc != 0: raise ServeError(f"{name}: wheelhouse 재포장 불통과(rc={b.rc}) — 다운로드 폴백 ✗(O-N2): {b.err[-400:]}")
            self._du_guard(name, "wheelhouse")
            v = r.run(wheelhouse_verify_argv(spec.wheel_tool, spec.root, generated_utc), 3600, check=False)
            self._preserve(f"wheelhouse-verify-{name}.json", (v.out or v.err).encode())
            self._du_guard(name, "venv")
            ok, venv_env, why = parse_verify_verdict(v.out)
            if v.rc != 0 or not ok: raise ServeError(f"{name}: 설치 게이트 불통과(rc={v.rc}): {why}")
            lds[name] = venv_env
            freeze = r.run(pip_freeze_argv(spec.root), 120).out
            self.state.setdefault("pip_freeze_rel", {})[name] = self._preserve(f"pip-freeze-{name}.txt", freeze.encode())
        self.state["native_install_rel"] = self._preserve("native-install.sh", native_install_script(c, tool_sha, generated_utc).encode())
        self.state["runtime_env_recipe"] = lds; self._state_write()
        return lds

    def _arm_watchdog(self, name: str, target: Identity) -> None:
        spec = self.c.nodes[name]
        wd = self.r[name].start(watchdog_argv(spec, target), f"logs/watchdog-{target.role}.log", f"watchdog-{target.role}")
        self._add_ident(wd, "watchdog", target.role)

    def start_cluster(self, lds: dict[str, dict]) -> Identity:
        c = self.c
        m, s = c.nodes["main"], c.nodes["sub"]
        head = self.r["main"].start(env_argv(runtime_env(c, m, lds["main"]), ray_head_argv(c, m)), "logs/ray-head.log", "ray-head")
        self._add_ident(head, "serve"); self._arm_watchdog("main", head)
        worker = self.r["sub"].start(env_argv(runtime_env(c, s, lds["sub"]), ray_worker_argv(c, s)), "logs/ray-worker.log", "ray-worker")
        self._add_ident(worker, "serve"); self._arm_watchdog("sub", worker)
        end = self.clock.mono() + RAY_JOIN_MAX_S
        while True:
            st = self.r["main"].run(env_argv(runtime_env(c, m, lds["main"]), ray_status_argv(c, m)), 60, check=False)
            if re.search(r"/2\.0 GPU", st.out): break
            if self.clock.mono() >= end: raise ServeError(f"Ray worker 가 {RAY_JOIN_MAX_S}s 안에 합류하지 않았다(GPU 2 미관측)")
            self.clock.sleep(HEALTH_INTERVAL_S)
        self.log("[native] Ray 클러스터 GPU 2 관측 — vllm serve 기동")
        env = runtime_env(c, m, lds["main"]); env["RAY_ADDRESS"] = f"{m.host_ip}:{c.ray_port}"
        serve = self.r["main"].start(env_argv(env, serve_argv(c)), "logs/vllm-serve.log", "vllm-serve")
        self._add_ident(serve, "serve"); self._arm_watchdog("main", serve)
        return serve

    def health_and_smoke(self, serve: Identity) -> dict:
        c = self.c
        base = f"http://127.0.0.1:{c.serve_port}"
        t0 = self.clock.mono(); last = 0; ready = False
        while self.clock.mono() - t0 <= c.ready_max_s:
            last, _ = self.http("GET", f"{base}/health", None, 5)
            if last == 200: ready = True; break
            if starttime_of(self.r["main"], serve) is False:
                raise ServeError("vllm serve 가 준비 전에 종료됐다(logs/vllm-serve.log)")
            self.clock.sleep(HEALTH_INTERVAL_S)
        waited = int(self.clock.mono() - t0)
        health = {"ok": ready, "http_status": last, "ready_after_s": waited if ready else None, "window_s": c.ready_max_s}
        if not ready:
            return {"health": health, "inference": {"ok": False, "verdict": "not-attempted"}, "endpoints": {"health": {"path": "/health", "http_status": last}}}
        body = {"model": c.model_name, "messages": [{"role": "user", "content": SMOKE_PROMPT}], "max_tokens": 256}
        code, text = self.http("POST", f"{base}/v1/chat/completions", body, 120)
        content, reasoning, fr = "", "", None
        with contextlib.suppress(Exception):
            ch = json.loads(text)["choices"][0]; msg = ch.get("message") or {}
            content = (msg.get("content") or "").strip(); reasoning = (msg.get("reasoning") or msg.get("reasoning_content") or "").strip()
            fr = ch.get("finish_reason")
        inf = {"chat_content_len": len(content), "reasoning_len": len(reasoning), "finish_reason": fr}
        ep_inf = {"path": "/v1/chat/completions", "http_status": code}
        if code == 200 and content:
            inf.update(ok=True, evidence="chat.content", completion_text_len=len(content))
        elif code == 200 and reasoning:
            # 파서 우회 확증(Docker 스모크 W-10 과 같다) — reasoning 만으로는 생성 실재를 인정하지 않는다
            c2, t2 = self.http("POST", f"{base}/v1/completions", {"model": c.model_name, "prompt": "2+2=", "max_tokens": 32}, 120)
            n = 0
            with contextlib.suppress(Exception): n = len((json.loads(t2)["choices"][0].get("text") or "").strip())
            ep_inf = {"path": "/v1/completions", "http_status": c2}
            inf.update(ok=(c2 == 200 and n > 0), evidence="v1.completions", completion_text_len=n)
        else:
            inf.update(ok=False, evidence=None, completion_text_len=0)
        return {"health": health, "inference": inf,
                "endpoints": {"health": {"path": "/health", "http_status": last}, "inference": ep_inf}}

    def write_proof(self, status: str, obs: dict, stage: str = "", reason: str = "") -> None:
        st = self.state
        doc = {"schema_version": 1, "kind": "native_multinode_serve_proof", "plane": "native", "topology": TOPOLOGY,
               "status": status, "cell": self.c.cell, "config": self.c.cell, "run_id": self.c.run_id,
               "source_cell": self.c.source_cell, "campaign_id": self.c.campaign_id,
               "health": obs.get("health") or {"ok": False}, "inference": obs.get("inference") or {"ok": False},
               "endpoints": obs.get("endpoints") or {},
               "native_install_path": st.get("native_install_rel"), "pip_freeze_paths": st.get("pip_freeze_rel"),
               "cleanup_attestation_path": st.get("attestation_rel"), "evidence_dir": st.get("preserve_rel"),
               "wheelhouse": {"source_image_tag": self.c.image, "tool_sha256": st.get("wheel_tool_sha256"),
                              "note": "각 노드가 자기 로컬 이미지에서 재포장(이미지 전송 ✗)"},
               "provenance": "measured(native_multinode_serve.py up · health GET + 추론 POST 관측)"}
        if status != "PASS": doc.update(failed_stage=stage, reason=reason[-600:])
        self._write_repo_json(self.c.proof_rel, doc)

    def arm_renew(self, serve_main: Identity, ttl: int) -> None:
        idents = {row["role"]: Identity(**{k: row[k] for k in ("node", "role", "pid", "pgid", "starttime", "log_rel")})
                  for row in self.state.get("identities", []) if row.get("kind") == "serve"}
        for name, target in (("main", serve_main), ("sub", idents["ray-worker"])):
            rn = self.r[name].start(renew_argv(self.c.nodes[name], target, ttl), f"logs/budget-renew-{name}.log", f"budget-renew-{name}")
            self._add_ident(rn, "renew", target.role)

    def up(self, generated_utc: str) -> int:
        c = self.c
        self.log(f"[native] up cell={c.cell} run={c.run_id} source={c.source_cell} image={c.image}")
        stage = "preflight"
        try:
            self.preflight_tools()
            stage = "gates+budget"; params = self.gates_and_budget()
            stage = "run-roots"; self.make_roots(generated_utc)
            self.state["budget_declared"] = dict(self.budget_declared); self.state["budget_ttl_s"] = params["ttl_s"]; self._state_write()
            stage = "install"; lds = self.install(generated_utc)
            stage = "start"; serve = self.start_cluster(lds)
            stage = "health"; obs = self.health_and_smoke(serve)
            if not obs["health"]["ok"]: raise ServeError(f"health 200 미도달({c.ready_max_s}s 창 · 마지막 http={obs['health']['http_status']})")
            if not obs["inference"].get("ok"): raise ServeError(f"추론 1회 관측 실패: {obs['inference']}")
            self.write_proof("PASS", obs)
            stage = "renew"; self.arm_renew(serve, params["ttl_s"])
            self.state["up_completed_utc"] = _utc(self.clock.now()); self._state_write()
            self.log(f"[native] serve proof → {c.proof_rel} (PASS) · 서빙 유지 — 회수: native_multinode_serve.py down --cell {c.cell} --run-id {c.run_id} --apply")
            return EXIT_OK
        except (Exception, KeyboardInterrupt) as exc:  # noqa: BLE001 — 어떤 실패든(버그·중단 포함) run root 가 있으면 자동 down
            self.log(f"[native] FAIL stage={stage}: {type(exc).__name__}: {exc}")
            obs = locals().get("obs") or {}
            with contextlib.suppress(Exception):
                self.write_proof("FAIL", obs, stage, str(exc))   # 최신 관측이 옛 PASS 를 덮는다
            if self.state.get("roots_created"):
                self.log("[native] run root 가 있다 — 자동 down(AC-N5)")
                att = down_from_state(self.state, self.c.repo, self.r, self.rs, clock=self.clock, log=self.log)
                return EXIT_FAIL if att.get("status") == "PASS" else EXIT_UNKNOWN
            errs = [e for e in (self._clear_budget(n) for n, d in self.budget_declared.items() if d) if e]
            for e in errs: self.log(f"[native] ⚠ {e}")
            return EXIT_FAIL if not errs else EXIT_UNKNOWN

    # ---- dry-run ----
    def plan(self) -> list[str]:
        c = self.c
        ph = lambda role: Identity("?", role, f"<pid:{role}>", f"<pgid:{role}>", f"<starttime:{role}>", "")  # noqa: E731 — 자리표시
        fake_ld = {"LD_LIBRARY_PATH": "<verify.env.LD_LIBRARY_PATH>"}
        lines = [f"# native up 계획(dry-run · 부수효과 0) cell={c.cell} run={c.run_id} source={c.source_cell} campaign={c.campaign_id}",
                 f"#   run root={c.nodes['main'].root} (양 노드 · 상한 {c.max_bytes >> 30}GiB/노드) · 짧은 루트={c.nodes['main'].short}",
                 f"#   ready_max_s={c.ready_max_s} overhead_mib={c.overhead_mib} serve_port={c.serve_port} ray_port={c.ray_port} image={c.image}"]
        def add(node: str, argv: list[str], note: str = "") -> None:
            lines.append(f"[{node}] {shlex.join(argv)}" + (f"   # {note}" if note else ""))
        m, s = c.nodes["main"], c.nodes["sub"]
        for n, spec in (("main", m), ("sub", s)):
            for p in (spec.session_py, spec.renew_sh, spec.watchdog_sh): add(n, ["test", "-f", p], "도구 실재")
            for p in (spec.renew_sh, spec.watchdog_sh): add(n, ["grep", "-q", "--", "--pgid", p], "pgid 모드 배달")
        add("main", ["python3", str(HERE / "check_smoke_model.py"), c.source_cell, "--repo", str(c.repo), "--topology", TOPOLOGY, "--emit-gate-params"], "RAM 게이트(원본 Docker 셀 = 같은 모델·TP·KV)")
        add("sub", ["cat", "/proc/meminfo"], "동일 문턱 RAM 게이트(부족 시 vllm-drop-caches 1회)")
        add("main", ["python3", str(HERE / "budget_preflight.py"), "--json", "--declared-gmu-yaml", str(c.repo / c.triplet["yaml"]), "--mem-total-mib", "<MemTotal>", "--weights-mib", "<(ckpt−ple)÷tp>", "--kv-mib", "<kv_mib>", "--overhead-mib", str(c.overhead_mib)], "선판정")
        for n in ("main", "sub"):
            add(n, ["python3", c.nodes[n].session_py, "--node-dir", c.nodes[n].node_dir, "declare-budget", "--mem-total-mib", "<MemTotal>", "--weights-mib", "<w>", "--kv-mib", "<kv>", "--overhead-mib", str(c.overhead_mib), "--ttl-s", "<max(3×ready,floor)>", "--expected-load-s", str(c.ready_max_s), "--label", f"smoke-{c.cell}", "--now", "<UTC>"], "clear→declare→budget_honored 관측")
        for n, spec in (("main", m), ("sub", s)):
            lines.append(f"[{n}] <init marker-owned root> {spec.root} · {spec.short}")
            add(n, mkdirs_argv(spec))
        lines.append(f"[sub] <put> {s.wheel_tool} ← {c.wheel_tool_rel} (sha256 기록)")
        for n, spec in (("main", m), ("sub", s)):
            add(n, wheelhouse_build_argv(spec.wheel_tool, c.image, spec.root, "<generated-utc>"), "자기 이미지에서 재포장")
            add(n, ["du", "-sxb", spec.root], "상한 검사")
            add(n, wheelhouse_verify_argv(spec.wheel_tool, spec.root, "<generated-utc>"), "venv·pip --no-index·pip check·ldd·import+CUDA → env 레시피")
            add(n, ["du", "-sxb", spec.root], "상한 검사")
            add(n, pip_freeze_argv(spec.root), f"→ docs/simlog/<YYMMDDHH>_{c.simlog_topic}/pip-freeze-{n}.txt")
        add("main", env_argv(runtime_env(c, m, fake_ld), ray_head_argv(c, m)), "start(새 세션) → identity")
        add("main", watchdog_argv(m, ph("ray-head")), "start")
        add("sub", env_argv(runtime_env(c, s, fake_ld), ray_worker_argv(c, s)), "start(새 세션) → identity")
        add("sub", watchdog_argv(s, ph("ray-worker")), "start")
        add("main", ray_status_argv(c, m), f"GPU 2 합류 대기 ≤{RAY_JOIN_MAX_S}s")
        env = runtime_env(c, m, fake_ld); env["RAY_ADDRESS"] = f"{m.host_ip}:{c.ray_port}"
        add("main", env_argv(env, serve_argv(c)), "start → identity")
        add("main", watchdog_argv(m, ph("vllm-serve")), "start")
        lines.append(f"[main] GET http://127.0.0.1:{c.serve_port}/health 매 {HEALTH_INTERVAL_S}s ≤ {c.ready_max_s}s → POST /v1/chat/completions 1회")
        lines.append(f"[main] → {c.proof_rel} (native_multinode_serve_proof)")
        add("main", renew_argv(m, ph("vllm-serve"), 0), "start · ttl=<ttl>")
        add("sub", renew_argv(s, ph("ray-worker"), 0), "start · ttl=<ttl>")
        lines.append(f"[main] → {m.root}/state.json · exit 0(서빙 유지) · 실패 시 자동 down")
        return lines


def starttime_of(runner: Runner, ident: Identity) -> bool:
    """리더 생존(같은 starttime). 원격은 tree 조회로 본다."""
    try:
        rows = runner.tree([ident.pid], [])
    except ServeError:
        return True     # 조회 불가는 '죽었다' 가 아니다 — health 창이 판정한다
    return any(r["pid"] == ident.pid and str(r["starttime"]) == ident.starttime for r in rows)


# ── down ─────────────────────────────────────────────────────────────────────────────────────────────
def _ident(row: dict) -> Identity:
    return Identity(**{k: row[k] for k in ("node", "role", "pid", "pgid", "starttime", "log_rel")})


def down_from_state(state: dict, repo: Path, runners: dict[str, Runner], short_runners: dict[str, Runner], *,
                    clock: Clock = Clock(), log: Callable[[str], None] = print) -> dict:
    run_id, cell = state["run_id"], state["cell"]
    nodes_spec = state["nodes"]
    errors: list[str] = []
    nodes = {n: {"root_absent": False, "owned_processes": [], "owned_gpu_processes": [], "unknown": []} for n in ("main", "sub")}
    idents = [(row.get("kind"), _ident(row)) for row in state.get("identities", [])]
    cleanup: list[dict] = []

    # ① 무장 해제 — 워치독·갱신 루프를 먼저(선언을 곧 지우므로 갱신자가 남으면 안 된다) · PID+starttime 만
    for kind, ident in idents:
        if kind in ("watchdog", "renew"):
            try: cleanup.append(runners[ident.node].stop(ident, 10))
            except ServeError as exc: nodes[ident.node]["unknown"].append(f"{ident.role}: {exc}"); errors.append(str(exc))
    serve = [i for k, i in idents if k == "serve"]
    # ② 소유 트리 스냅숏(정지 전 · ppid 사슬 + pgid) — 신호를 받을 수 있는 것은 이 스냅숏 안의 (pid, starttime) 뿐이다
    snap: dict[str, dict[int, str]] = {"main": {}, "sub": {}}
    for n in ("main", "sub"):
        mine = [i for i in serve if i.node == n]
        if not mine: continue
        try:
            for row in runners[n].tree([i.pid for i in mine], [i.pgid for i in mine]):
                snap[n][int(row["pid"])] = str(row["starttime"])
        except ServeError as exc:
            nodes[n]["unknown"].append(f"스냅숏 조회 실패: {exc}"); errors.append(str(exc))
    # ③ 리더 정지 — 역순(vllm-serve → ray-worker → ray-head) · 그룹 TERM→KILL · starttime 재확인
    for ident in reversed(serve):
        try: cleanup.append(runners[ident.node].stop(ident, STOP_WAIT_S))
        except ServeError as exc: nodes[ident.node]["unknown"].append(f"{ident.role}: {exc}"); errors.append(str(exc))
    # ④ 그룹 밖으로 나간 스냅숏 자손 — 단일 PID 신호(같은 starttime 일 때만)
    for n in ("main", "sub"):
        leaders = {i.pid for i in serve if i.node == n}
        for pid, st in sorted(snap[n].items()):
            if pid in leaders: continue
            try:
                alive = runners[n].tree([pid], [])
                if any(int(r["pid"]) == pid and str(r["starttime"]) == st for r in alive):
                    cleanup.append(runners[n].stop(Identity(n, f"descendant-{pid}", pid, pid, st, ""), 10, pid_only=True))
            except ServeError as exc:
                nodes[n]["unknown"].append(f"자손 {pid}: {exc}"); errors.append(str(exc))
    # ⑤ 증거 보존 — 삭제 **전** · run 마다 새 파일(재시도는 attempt 디렉터리)
    preserved = True
    pdir = repo / state["preserve_rel"]
    try:
        pdir.mkdir(parents=True, exist_ok=True)
        sub = "logs"; k = 2
        while (pdir / sub).exists(): sub = f"logs.attempt{k}"; k += 1
        (pdir / sub).mkdir()
        (pdir / sub / "state.json").write_text(json.dumps(state, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        for _, ident in idents:
            try: text = runners[ident.node].text(ident.log_rel)
            except (ServeError, OSError) as exc:
                preserved = False; errors.append(f"로그 보존 실패 {ident.node}/{ident.role}: {exc}"); continue
            with (pdir / sub / f"{ident.node}-{ident.role}.log").open("x", encoding="utf-8") as fh: fh.write(text)
        for rel in [state.get("native_install_rel"), *(state.get("pip_freeze_rel") or {}).values()]:
            if rel and not (repo / rel).is_file(): preserved = False; errors.append(f"보존 증거 부재: {rel}")
    except OSError as exc:
        preserved = False; errors.append(f"보존 디렉터리 실패: {exc}")
    # ⑥ 신선한 잔재 조회
    ports = state.get("ports") or {}
    for n in ("main", "sub"):
        mine = [i for i in serve if i.node == n]
        try:
            rows = runners[n].tree([i.pid for i in mine], [i.pgid for i in mine]) if mine else []
            owned = [r for r in rows if str(r["starttime"]) == snap[n].get(int(r["pid"])) or int(r["pgid"]) in {i.pgid for i in mine}]
            nodes[n]["owned_processes"] = owned
            if owned: errors.append(f"{n}: owned process residue {[r['pid'] for r in owned]}")
        except ServeError as exc:
            nodes[n]["unknown"].append(f"잔재 조회 실패: {exc}"); errors.append(str(exc))
        try:
            gpu = runners[n].gpu()
            pids = set(snap[n]) | {i.pid for i in mine}
            for line in gpu:
                head = line.split(",", 1)[0].strip()
                if head.isdigit() and int(head) in pids: nodes[n]["owned_gpu_processes"].append(line)
                else: nodes[n]["unknown"].append(f"귀속 불명 GPU compute app: {line}")
            if nodes[n]["owned_gpu_processes"]: errors.append(f"{n}: owned GPU residue")
        except ServeError as exc:
            nodes[n]["unknown"].append(f"GPU 조회 실패: {exc}"); errors.append(str(exc))
        if n == "main":
            try:
                listening = set(runners[n].ports())
                for label in ("serve", "ray"):
                    if ports.get(label) in listening: nodes[n]["unknown"].append(f"포트 {ports[label]}({label}) 가 아직 listen")
            except ServeError as exc:
                nodes[n]["unknown"].append(f"포트 조회 실패: {exc}"); errors.append(str(exc))
    # ⑦ 깨끗한 노드만 소유 루트 삭제(살아 있는 프로세스의 파일을 지우지 않는다 — 재시도 down 이 state 를 다시 읽는다)
    for n in ("main", "sub"):
        clean = not nodes[n]["owned_processes"] and not nodes[n]["owned_gpu_processes"] and not nodes[n]["unknown"]
        if not clean or not preserved:
            errors.append(f"{n}: 잔재·미확정·증거 미보존 — 소유 루트를 남긴다(재시도: down)")
            continue
        created = (state.get("roots_created") or {}).get(n, [])
        try:
            for kind, runner in (("short", short_runners[n]), ("run", runners[n])):
                if kind in created and runner.exists(): runner.remove_owned_tree(run_id)
            nodes[n]["root_absent"] = not short_runners[n].exists() and not runners[n].exists()
            if not nodes[n]["root_absent"]: errors.append(f"{n}: 삭제 뒤에도 루트가 남았다")
        except ServeError as exc:
            nodes[n]["unknown"].append(f"루트 삭제 실패: {exc}"); errors.append(str(exc))
    # ⑧ 예산 선언 회수(갱신 루프는 ①에서 정지) · 페이지캐시 드랍(best-effort)
    for n in ("main", "sub"):
        if (state.get("budget_declared") or {}).get(n):
            spec = nodes_spec[n]
            try: runners[n].run(["python3", spec["session_py"], "--node-dir", spec["node_dir"], "clear-budget", "--now", _utc(clock.now())], 30)
            except ServeError as exc: errors.append(f"{n}: clear-budget 실패: {exc}")
        with contextlib.suppress(ServeError):
            dc = "/usr/local/sbin/vllm-drop-caches"
            if runners[n].run(["test", "-x", dc], 15, check=False).rc == 0: runners[n].run(["sudo", "-n", dc], 60, check=False)
    ok = not errors and preserved and all(v["root_absent"] and not v["owned_processes"] and not v["owned_gpu_processes"] and not v["unknown"]
                                          for v in nodes.values())
    att = {"schema_version": 1, "kind": "native_multinode_cleanup_attestation", "plane": "native", "run_id": run_id, "cell": cell,
           "status": "PASS" if ok else "FAIL_CLOSED", "nodes": nodes, "cleanup": cleanup, "errors": errors,
           "evidence_preserved": preserved, "preserved": {"evidence_root": state["preserve_rel"]},
           "attested_utc": _utc(clock.now()), "provenance": "measured(PID+starttime 정지 · 신선한 tree/GPU/port 조회 · 루트 부재 조회)"}
    p = repo / state["attestation_rel"]; p.parent.mkdir(parents=True, exist_ok=True)
    tmp = p.with_name(p.name + ".tmp")
    tmp.write_text(json.dumps(att, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8"); tmp.replace(p)
    log(f"[native] cleanup attestation → {state['attestation_rel']} ({att['status']}" + (f" · errors={len(errors)}" if errors else "") + ")")
    return att


def runners_for(state_nodes: dict, sub_host: str) -> tuple[dict[str, Runner], dict[str, Runner]]:
    m, s = state_nodes["main"], state_nodes["sub"]
    return ({"main": Local(m["root"], "main"), "sub": SSH(sub_host, s["root"], "sub")},
            {"main": Local(m["short"], "main"), "sub": SSH(sub_host, s["short"], "sub")})


def down_cli(repo: Path, cell: str, run_id: str, apply: bool, base: Optional[str]) -> int:
    """state.json 만 읽는다. 없으면 — 직전 down 이 이미 끝낸 run 인지(proof 의 run_id + attestation PASS)만 보고 멱등 no-op."""
    import yaml
    if not RUN_ID.fullmatch(run_id): print("FAIL: unsafe run-id", file=sys.stderr); return EXIT_USAGE
    if base is None:
        active = (repo / "campaigns" / "ACTIVE").read_text(encoding="utf-8").strip()
        camp = yaml.safe_load((repo / "campaigns" / active / "campaign.yaml").read_text(encoding="utf-8"))
        base = ((camp.get("topology_sections") or {}).get(TOPOLOGY) or {}).get("native_run_root_base")
    state_path = Path(validate_remote_root(f"{str(base).rstrip('/')}/{run_id}", run_id)) / "state.json"
    if not state_path.is_file():
        proof = repo / f"output/{TOPOLOGY}/benchlog/serve_proof_{cell}.json"
        with contextlib.suppress(Exception):
            doc = json.loads(proof.read_text(encoding="utf-8"))
            att = json.loads((repo / doc["cleanup_attestation_path"]).read_text(encoding="utf-8"))
            if doc.get("run_id") == run_id and att.get("run_id") == run_id and att.get("status") == "PASS":
                print(f"[native] down: run {run_id} 은 이미 정리됐다(attestation PASS) — no-op"); return EXIT_OK
        print(f"FAIL_CLOSED_UNKNOWN: {state_path} 없음 — 소유 정체를 모르면 아무것도 정지·삭제하지 않는다", file=sys.stderr)
        return EXIT_UNKNOWN
    state = json.loads(state_path.read_text(encoding="utf-8"))
    if state.get("run_id") != run_id or state.get("cell") != cell:
        print("FAIL: state.json 의 run_id/cell 이 인자와 다르다", file=sys.stderr); return EXIT_FAIL
    if not apply:
        print(f"[native] down 계획(dry-run) run={run_id} cell={cell}")
        for row in state.get("identities", []):
            print(f"[{row['node']}] stop pid={row['pid']} pgid={row['pgid']} starttime={row['starttime']} role={row['role']} kind={row.get('kind')}")
        print(f"[*] 보존 → {state['preserve_rel']}/logs · 잔재 조회 → 루트 삭제 {state['nodes']['main']['root']} · {state['nodes']['main']['short']} (양 노드) · 예산 회수 · attestation → {state['attestation_rel']}")
        return EXIT_OK
    r, rs = runners_for(state["nodes"], state["sub_host"])
    att = down_from_state(state, repo, r, rs)
    return EXIT_OK if att["status"] == "PASS" else EXIT_UNKNOWN


# ── 자체검사(가짜 러너 · 임시 트리 — 실서빙·Ray·컨테이너·실프로세스 사살 없음) ─────────────────────────────
class FakeClock(Clock):
    def __init__(self) -> None: self.t = 1_790_000_000.0
    def now(self) -> float: return self.t
    def mono(self) -> float: return self.t
    def sleep(self, s: float) -> None: self.t += s


class Fake(Runner):
    """명령 argv 모양으로 응답하는 가짜 노드. 루트는 진짜 임시 디렉터리(마커·삭제는 실코드)."""
    def __init__(self, node: str, root: str, clock: FakeClock, world: dict, fault: set) -> None:
        self.node, self.root, self.clock, self.w, self.fault = node, root, clock, world, fault
        self.calls: list[list[str]] = []
    def init_root(self, ensure_parent: bool = False) -> None: Local(self.root, self.node).init_root(ensure_parent)
    def put(self, rel: str, data: bytes, mode: int = 0o600) -> None: Local(self.root, self.node).put(rel, data, mode)
    def run(self, argv: Sequence[str], timeout: int, check: bool = True) -> Result:
        a = list(argv); self.calls.append(a); s = " ".join(a); rc, out = 0, ""
        if "check_smoke_model.py" in s:
            out = "[NAS-check] BUDGET_PARAMS ckpt_mib=60000 tp=2 kv_mib=20480 ple_mib=40000\n[NAS-check] GATE_PARAMS required_mib=40000\n"
        elif a[:2] == ["cat", "/proc/meminfo"] and "meminfo" in self.fault: raise ServeError(f"{self.node}: unreachable")
        elif a[:2] == ["cat", "/proc/meminfo"]: out = "MemTotal:       125000000 kB\nMemAvailable:   110000000 kB\n"
        elif "budget-defaults" in a: out = "7200\n"
        elif "budget_preflight.py" in s: out = '{"floor_mib": 50000, "arm_ceiling_mib": 41808}'
        elif "declare-budget" in a or "clear-budget" in a: self.w.setdefault("budget", []).append((self.node, a[a.index("--node-dir") + 2]))
        elif a[0] == "tail":
            out = json.dumps({"ts": _utc(self.clock.now()), "event": "budget_honored"}) + "\n"
        elif a[0] == "test" and a[1] == "-f" and a[2].endswith("serve_budget.env"): rc = 1
        elif a[0] == "test" and a[1] == "-x": rc = 1
        elif a[0] == "du": out = f"1073741824\t{a[-1]}\n"
        elif "native_wheelhouse.py" in s and "verify" in a:
            rl = f"{self.root}/wh/runtime-lib"
            out = json.dumps({"kind": "native_wheelhouse_verify", "verdict": "PASS", "selected": "compat-first", "ld_library_path": f"{rl}/cuda/lib:{rl}/lib",
                              "env": {"LD_LIBRARY_PATH": f"{rl}/cuda/lib:{rl}/lib", "TRITON_PTXAS_PATH": f"{rl}/cuda/bin/ptxas",
                                      "PATH": f"{rl}/cuda/bin:{self.root}/venv/bin:/srv/x/bin:/usr/bin"}}, indent=1) + "\n"
        elif a[-2:] == ["freeze", "--all"]: out = "ray==2.49.0\ntorch==2.13.0a0\nvllm==0.29.0rc6\n"
        elif "status" in a and any(x.endswith("/bin/ray") for x in a): out = "Resources\n 0.0/2.0 GPU\n"
        elif a[0] == "mkdir": pass
        if check and rc: raise ServeError(f"{self.node}: rc={rc}: {a[:3]}")
        return Result(a, rc, out, "")
    def start(self, argv: Sequence[str], log_rel: str, role: str) -> Identity:
        if f"start:{role}" in self.fault: raise ServeError(f"fake {role} exited at start")
        self.w["pid"] = self.w.get("pid", 1000) + 1; pid = self.w["pid"]
        Local(self.root, self.node).put(log_rel, f"{role} log line\n".encode())
        ident = Identity(self.node, role, pid, pid, str(pid * 7), log_rel)
        self.w.setdefault("alive", {})[(self.node, pid)] = {"pid": pid, "ppid": 1, "pgid": pid, "starttime": str(pid * 7), "argv": role}
        if role == "ray-worker" or role == "vllm-serve":   # 그룹 안 워커 1개(스냅숏·잔재 조회 대상)
            self.w["pid"] += 1; kid = self.w["pid"]
            self.w["alive"][(self.node, kid)] = {"pid": kid, "ppid": pid, "pgid": pid, "starttime": str(kid * 7), "argv": role + "-worker"}
        self.w.setdefault("argv", {})[role] = list(argv)
        return ident
    def stop(self, ident: Identity, wait: int, pid_only: bool = False) -> dict[str, Any]:
        row = self.w.get("alive", {}).get((self.node, ident.pid))
        if row and row["starttime"] != ident.starttime or "reuse" in self.fault and ident.role in ("ray-head", "ray-worker"):
            raise UnknownState(f"{self.node}: pid-reuse-before-term")
        for key in [k for k, v in self.w.get("alive", {}).items() if k[0] == self.node and (v["pid"] == ident.pid or (not pid_only and v["pgid"] == ident.pgid))]:
            if "survive" in self.fault and self.w["alive"][key]["argv"].endswith("-worker"): continue
            del self.w["alive"][key]
        return {"node": self.node, "role": ident.role, "pid": ident.pid, "term_status": "term-exited", "starttime_match": True}
    def text(self, rel: str) -> str: return Local(self.root, self.node).text(rel)
    def tree(self, pids: Sequence[int], pgids: Sequence[int]) -> list[dict[str, Any]]:
        rows = [v for k, v in self.w.get("alive", {}).items() if k[0] == self.node]
        keep, todo = set(), [p for p in pids if any(r["pid"] == p for r in rows)]
        while todo:
            x = todo.pop()
            if x in keep: continue
            keep.add(x); todo.extend(r["pid"] for r in rows if r["ppid"] == x)
        keep |= {r["pid"] for r in rows if r["pgid"] in set(pgids)}
        return [dict(r) for r in rows if r["pid"] in keep]
    def gpu(self) -> list[str]:
        if "gpu" in self.fault: raise UnknownState(f"{self.node}: fake GPU unknown")
        return [f"{r['pid']}, python, 1000" for k, r in self.w.get("alive", {}).items() if k[0] == self.node and r["argv"].endswith("-worker")]
    def ports(self) -> list[int]: return [8080] if self.node == "main" and any(k[0] == "main" for k in self.w.get("alive", {})) else []
    def exists(self) -> bool: return os.path.lexists(self.root)
    def remove_owned_tree(self, run_id: str) -> None: safe_remove_owned_tree(Path(self.root), run_id)


def _fixture_repo(base: Path) -> tuple[Path, str]:
    rn = _load_module("_nm_render_native", HERE / "render_native_triplet.py")
    repo = rn._fixture(base / "repo", manifest_extra=(
        "nodes:\n  - role: main\n    host: \"198.51.100.1\"\n    ssh_user: \"u\"\n    work_dir: \"/fx/repo\"\n"
        "  - role: sub\n    host: \"198.51.100.2\"\n    ssh_user: \"u\"\n    work_dir: \"/fx/sub\"\n"))
    rn.apply(repo, rn.render(repo, "multi", "src", "src-native"))
    (repo / "output/multi/envs/.env.interconnect").write_text("NCCL_SOCKET_IFNAME=eth9\nNCCL_IB_HCA==dev0\n", encoding="utf-8")
    (repo / "output/multi/envs/.env.cluster").write_text("MASTER_HOST_IP=198.51.100.1\nSLAVE_HOST_IP=198.51.100.2\nRAY_PORT=6379\n"
                                                           "RAY_OBJECT_STORE_MEMORY=2000000000\n", encoding="utf-8")
    (repo / ".claude/skills/upstream-version-watch/scripts").mkdir(parents=True)
    (repo / ".claude/skills/upstream-version-watch/scripts/native_wheelhouse.py").write_text("# fixture wheelhouse tool\n", encoding="utf-8")
    camp = {"id": "camp-fx", "budgets": {"ready_max_seconds": 60, "smoke_budget_overhead_mib": 24800},
            "topology_sections": {"multi": {"native_cells": ["src-native"], "native_run_root_base": str(base / "runs"),
                                            "native_run_root_max_gib_per_node": 80, "native_sub_host": "u@198.51.100.2"}}}
    (repo / "campaigns/camp-fx").mkdir(parents=True)
    (repo / "campaigns/camp-fx/campaign.yaml").write_text(json.dumps(camp), encoding="utf-8")
    (repo / "campaigns/ACTIVE").write_text("camp-fx\n", encoding="utf-8")
    return repo, "src-native"


def _scenario(base: Path, tag: str, fault: set, *, http_ok: bool = True):
    repo, cell = _fixture_repo(base / tag)
    ctx = load_ctx(repo, cell, f"r-{tag}")
    # 짧은 루트는 /tmp 를 건드리지 않게 자체검사 트리 아래로 옮긴다(동일 규약: basename == run-id)
    for n, spec in ctx.nodes.items():
        spec.root = str(base / tag / f"{n}-run" / ctx.run_id); spec.short = str(base / tag / f"{n}-short" / ctx.run_id)
    clock, world = FakeClock(), {}
    r = {n: Fake(n, ctx.nodes[n].root, clock, world, fault) for n in ("main", "sub")}
    rs = {n: Fake(n, ctx.nodes[n].short, clock, world, fault) for n in ("main", "sub")}
    polls = {"n": 0}
    def http(method: str, url: str, body, timeout: int) -> tuple[int, str]:
        if url.endswith("/health"):
            polls["n"] += 1
            return (200, "") if http_ok and polls["n"] >= 3 else (503, "")
        return 200, json.dumps({"choices": [{"message": {"content": "4"}, "finish_reason": "stop"}]})
    ns = NativeServe(ctx, r, rs, http=http, clock=clock, log=lambda s: None)
    return repo, ctx, ns, world, r, rs


def self_test() -> int:
    bad: list[str] = []
    def ck(name: str, cond: bool) -> None:
        print(("[PASS] " if cond else "[FAIL] ") + name); bad.extend([] if cond else [name])
    sys.path.insert(0, str(REPO_DEFAULT / ".claude/skills/hint-publisher/scripts"))
    from hintlib import core as hcore, evidence as hev  # 발행기 자신의 판정기 — 이것이 진짜 합격 시험이다
    def publisher(repo: Path, cell: str) -> tuple[bool, str]:
        proof = json.loads((repo / f"output/multi/benchlog/serve_proof_{cell}.json").read_text(encoding="utf-8"))
        ok, why = hev._serve_proof_ok(proof, cell, None)
        if not ok: return False, why
        try: hev._native_producer_evidence(repo, "multi", cell, proof, f"output/multi/benchlog/serve_proof_{cell}.json")
        except hcore.HintError as exc: return False, exc.code
        return True, "accepted"
    with tempfile.TemporaryDirectory(prefix="native-mn-") as d:
        base = Path(d)
        # ── 뼈대 안전 원시 ──
        root = base / "run"; root.mkdir(); (root / MARKER).write_bytes(marker_bytes("run")); (root / "a").mkdir(); (root / "a/f").write_text("x")
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
        # ── 고정 원격 러너를 로컬 python 으로(ssh 없이 같은 바이트) ──
        rr = SSH("u@h", str(base / "rparent" / "rid1"), "sub", transport=[])
        rr.init_root(ensure_parent=True)
        ck("remote init(ensure_parent) + 정확한 마커", (base / "rparent/rid1" / MARKER).read_bytes() == marker_bytes("rid1"))
        refused = False
        try: rr.init_root()
        except UnknownState: refused = True
        ck("remote init 은 기존 루트를 거부", refused)
        rr.put("tools/t.py", b"print(1)\n"); ck("remote put + text", rr.text("tools/t.py") == "print(1)\n")
        refused = False
        try: rr.put("tools/t.py", b"x")
        except UnknownState: refused = True
        ck("remote put 은 덮어쓰기 거부", refused)
        refused = False
        try: rr.text("../escape")
        except UnknownState: refused = True
        ck("remote 경로 탈출 거부", refused)
        ck("remote run(정확한 argv · 셸 없음)", rr.run(["printf", "%s", "a b;c"], 10).out == "a b;c")
        me = os.getpid()
        ck("remote tree(ppid 사슬)가 자기 부모를 본다", any(int(x["pid"]) == me for x in rr.tree([me], [])))
        # PID 재사용 거부 — 실제 살아 있는 PID(이 프로세스)에 **틀린** starttime 을 주면 신호 없이 거부한다
        fake_id = Identity("sub", "reuse-probe", me, os.getpgid(me), "0", "")
        refused = False
        try: rr.stop(fake_id, 1)
        except UnknownState as exc: refused = "pid-reuse-before-term" in str(exc)
        ck("★remote stop: starttime 불일치 = 신호 없이 거부(PID 재사용)", refused)
        refused = False
        try: Local(str(base), "main").stop(Identity("main", "reuse-probe", me, os.getpgid(me), "0", ""), 1)
        except UnknownState as exc: refused = "pid-reuse-before-term" in str(exc)
        ck("★local stop: starttime 불일치 = 신호 없이 거부(PID 재사용)", refused)
        (base / "rparent/rid1/evil").symlink_to(base)
        refused = False
        try: rr.remove_owned_tree("rid1")
        except UnknownState: refused = True
        ck("remote remove 는 symlink 를 거부", refused and (base / "rparent/rid1").exists())
        (base / "rparent/rid1/evil").unlink(); rr.remove_owned_tree("rid1")
        ck("remote remove 깊이 우선 삭제", not rr.exists())

        # ── 정상 경로: up → proof → down → attestation · 발행기 판정기가 받아들이는가 ──
        repo, ctx, ns, world, r, rs = _scenario(base, "ok", set())
        rc = ns.up("2026-09-23T03:30:00Z")
        proof = json.loads((repo / ctx.proof_rel).read_text(encoding="utf-8"))
        ck("up 정상 = exit 0 · proof PASS · 서빙 유지(리더 생존)", rc == 0 and proof["status"] == "PASS" and any(k[0] == "main" for k in world["alive"]))
        st = json.loads((Path(ctx.nodes["main"].root) / "state.json").read_text(encoding="utf-8"))
        kinds = sorted(x["kind"] + ":" + x["role"] for x in st["identities"])
        ck("state.json = 리더 3 + 워치독 3(그룹마다) + 갱신 루프 2", kinds.count("serve:ray-head") == 1 and sum(k.startswith("watchdog") for k in kinds) == 3
           and sum(k.startswith("renew") for k in kinds) == 2)
        wd = world["argv"]["watchdog-vllm-serve"]
        ck("워치독 = host_safety/mem_watchdog.sh --pgid --starttime(이름 ✗)", wd[1].endswith("host_safety/mem_watchdog.sh") and "--pgid" in wd and "--starttime" in wd)
        ck("갱신 루프 = --node-dir --pgid --starttime --ttl-s", all(x in world["argv"]["budget-renew-main"] for x in ("--node-dir", "--pgid", "--starttime", "--ttl-s")))
        env_head = world["argv"]["ray-head"]
        ck("env -i · 캐시·TMPDIR·HOME 가 소유 루트 아래(HOME 독립)", env_head[:2] == ["env", "-i"] and all(
            any(t.startswith(f"{k}={ctx.nodes['main'].root}/") for t in env_head) for k in ("XDG_CACHE_HOME", "TRITON_CACHE_DIR", "TORCHINDUCTOR_CACHE_DIR",
                                                                                          "FLASHINFER_WORKSPACE_BASE", "VLLM_CACHE_ROOT", "HF_HOME", "PIP_CACHE_DIR", "HOME"))
           and f"TMPDIR={ctx.nodes['main'].short}/tmp" in env_head)
        ck("NCCL interconnect env + verify 레시피(LD_LIBRARY_PATH·번들 도구) · PATH 는 소유 루트 항목만", "NCCL_IB_HCA==dev0" in env_head
           and any(t.startswith("LD_LIBRARY_PATH=") and "runtime-lib" in t for t in env_head) and any(t.startswith("TRITON_PTXAS_PATH=") for t in env_head)
           and not any(t.startswith("PATH=") and "/srv/x" in t for t in env_head))
        ck("서브 워커엔 셀 트리플렛(CONFIG_FILE) 미전파 · serve_env(PLE)는 전달", not any(t.startswith("CONFIG_FILE=") for t in world["argv"]["ray-worker"])
           and "VLLM_PLE_MMAP=1" in world["argv"]["ray-worker"])
        ck("vllm serve = 렌더된 native .sh", world["argv"]["vllm-serve"][-2:] == ["bash", str(repo / "output/multi/configs/src-native.sh")])
        budget_nodes = [b[0] for b in world["budget"]]
        ck("예산 clear→declare 양 노드(로드 전)", budget_nodes.count("main") >= 2 and budget_nodes.count("sub") >= 2)
        inst = (repo / proof["native_install_path"]).read_text(encoding="utf-8")
        ck("native-install.sh = 실행 빌더와 같은 명령(build→verify) · 운영자 경로 ✗", "--no-index" in inst and '"${RUN_ROOT}/wh"' in inst
           and " verify " in inst and " build " in inst
           and str(base) not in inst and "/home/" not in inst)
        cell = "src-native"
        ok, why = publisher(repo, cell)
        ck(f"★발행기(up 직후): attestation 아직 없음 → 거부({why})", not ok)
        att = down_from_state(st, repo, r, rs, clock=FakeClock(), log=lambda s: None)
        ck("down 정상 = attestation PASS · 루트 부재 · 잔재 0", att["status"] == "PASS" and not os.path.lexists(ctx.nodes["main"].root)
           and not os.path.lexists(ctx.nodes["sub"].short) and not [k for k in world["alive"]])
        ck("down 이 로그·state 를 삭제 전에 보존", (repo / st["preserve_rel"] / "logs/main-vllm-serve.log").is_file()
           and (repo / st["preserve_rel"] / "logs/state.json").is_file())
        ck("attestation 모양(unknown 리스트 · evidence_preserved · errors=[])", att["nodes"]["main"]["unknown"] == [] and att["evidence_preserved"] is True and att["errors"] == [])
        ok, why = publisher(repo, cell)
        ck(f"★발행기 _serve_proof_ok + _native_producer_evidence 가 proof+attestation 을 받아들인다({why})", ok)

        # ── 실패 주입: vllm serve 기동 실패 → 자동 down → 잔재 0 ──
        repo2, ctx2, ns2, world2, r2, rs2 = _scenario(base, "startfail", {"start:vllm-serve"})
        rc2 = ns2.up("2026-09-23T04:30:00Z")
        att2 = json.loads((repo2 / json.loads((repo2 / ctx2.proof_rel).read_text(encoding="utf-8"))["cleanup_attestation_path"]).read_text(encoding="utf-8"))
        proof2 = json.loads((repo2 / ctx2.proof_rel).read_text(encoding="utf-8"))
        ck("★기동 실패 → exit≠0 · proof FAIL(stage=start)", rc2 == EXIT_FAIL and proof2["status"] == "FAIL" and proof2["failed_stage"] == "start")
        ck("★기동 실패 → 자동 down 잔재 0(루트 부재 · 프로세스 0 · attestation PASS)", att2["status"] == "PASS" and not world2["alive"]
           and not os.path.lexists(ctx2.nodes["main"].root) and not os.path.lexists(ctx2.nodes["sub"].root))
        ck("★기동 실패 → 예산 선언 회수(clear-budget 양 노드)", sum(1 for n, _ in world2["budget"] if n == "sub") >= 3)
        ck("★기동 실패 proof 는 발행 자격 없음", not publisher(repo2, "src-native")[0])

        # ── 실패 주입: health 창 소진 → 자동 down ──
        repo3, ctx3, ns3, world3, _, _ = _scenario(base, "nohealth", set(), http_ok=False)
        rc3 = ns3.up("2026-09-23T05:30:00Z")
        ck("★health 미도달 → 자동 down 잔재 0", rc3 == EXIT_FAIL and not world3["alive"] and not os.path.lexists(ctx3.nodes["main"].root))

        # ── 실패 주입: 예산 게이트 전 실패(run root 없음) → 루트를 만들지 않는다 ──
        repo4, ctx4, ns4, world4, r4, _ = _scenario(base, "gate", set())
        r4["sub"].fault.add("meminfo")
        rc4 = ns4.up("2026-09-23T06:30:00Z")
        ck("★RAM 게이트 실패 → 루트 미생성 · 기동 0", rc4 == EXIT_FAIL and not os.path.lexists(ctx4.nodes["main"].root) and not world4.get("alive"))

        # ── PID 재사용 at down → FAIL_CLOSED · 루트 보존 · 발행기 거부 ──
        repo5, ctx5, ns5, world5, r5, rs5 = _scenario(base, "reuse", set())
        ns5.up("2026-09-23T07:30:00Z")
        st5 = json.loads((Path(ctx5.nodes["main"].root) / "state.json").read_text(encoding="utf-8"))
        for fk in (r5, rs5):
            for f in fk.values(): f.fault.add("reuse")
        att5 = down_from_state(st5, repo5, r5, rs5, clock=FakeClock(), log=lambda s: None)
        ck("★PID 재사용 → attestation FAIL_CLOSED · unknown 비어있지 않음 · 루트 보존",
           att5["status"] == "FAIL_CLOSED" and att5["nodes"]["main"]["unknown"] and os.path.lexists(ctx5.nodes["main"].root))
        ok5, why5 = publisher(repo5, "src-native")
        ck(f"★발행기가 비-PASS attestation 을 거부({why5})", not ok5 and why5 == "HINT_NATIVE_CLEANUP_NONPASS")

        # ── GPU 잔재(그룹 밖으로 빠진 워커가 살아남음) → FAIL ──
        repo6, ctx6, ns6, world6, r6, rs6 = _scenario(base, "gpu", set())
        ns6.up("2026-09-23T08:30:00Z")
        st6 = json.loads((Path(ctx6.nodes["main"].root) / "state.json").read_text(encoding="utf-8"))
        for fk in (r6, rs6):
            for f in fk.values(): f.fault.add("survive")
        att6 = down_from_state(st6, repo6, r6, rs6, clock=FakeClock(), log=lambda s: None)
        ck("★살아남은 소유 워커·GPU 잔재 → FAIL_CLOSED(루트 보존)", att6["status"] == "FAIL_CLOSED" and att6["nodes"]["main"]["owned_gpu_processes"]
           and os.path.lexists(ctx6.nodes["main"].root))

        # ── dry-run: 계획만 · 부수효과 0 ──
        repo7, ctx7, ns7, world7, r7, rs7 = _scenario(base, "dry", set())
        before = sorted(str(p.relative_to(base)) for p in base.rglob("*"))
        lines = ns7.plan()
        after = sorted(str(p.relative_to(base)) for p in base.rglob("*"))
        calls = sum(len(f.calls) for f in (*r7.values(), *rs7.values()))
        ck("★dry-run: 파일 변화 0 · 러너 호출 0", before == after and calls == 0 and not world7)
        ck("dry-run: 노드별 명령(메인·서브 · wheelhouse·ray·serve·watchdog·renew)",
           any(x.startswith("[sub] ") and "native_wheelhouse.py build" in x for x in lines) and any("ray start --head" in x for x in lines)
           and any(x.startswith("[sub] bash") and "mem_watchdog.sh --pgid" in x for x in lines) and any("budget_renew_loop.sh" in x for x in lines))

        body = Path(__file__).read_text(encoding="utf-8")
        ck("no name-based cleanup primitives", ("p" + "kill") not in body and ("p" + "grep") not in body and ("kill" + "all") not in body)
        ck("same-starttime KILL contract", "pid-reuse-before-kill" in body)
        ck("offline/no-index contract · 이미지 내 고정 wheelhouse 경로 기본값 없음", "--no-index" in body and ("/opt/" + "wheelhouse") not in body)
    print(f"[native-multinode-serve] {'PASS' if not bad else 'FAIL'}")
    return 0 if not bad else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="native 2노드 Ray/vLLM 서빙 정문(up/down)")
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="op")
    for name in ("up", "down"):
        p = sub.add_parser(name)
        p.add_argument("--cell", required=True); p.add_argument("--run-id", required=True)
        p.add_argument("--apply", action="store_true"); p.add_argument("--repo", default=str(REPO_DEFAULT))
        if name == "up":
            p.add_argument("--source-cell"); p.add_argument("--ready-max-seconds", type=int)
            p.add_argument("--simlog-topic", default="native_N1")
        else:
            p.add_argument("--run-root-base", help="state.json 을 찾을 base(기본 = 활성 캠페인 native_run_root_base)")
    a = ap.parse_args()
    if a.self_test: return self_test()
    if a.op not in ("up", "down"): ap.print_help(sys.stderr); return EXIT_USAGE
    repo = Path(a.repo).resolve()
    try:
        if a.op == "down":
            return down_cli(repo, a.cell, a.run_id, a.apply, a.run_root_base)
        ctx = load_ctx(repo, a.cell, a.run_id, source_cell=a.source_cell, ready_max_s=a.ready_max_seconds, simlog_topic=a.simlog_topic)
        clock = Clock(); utc = _utc(clock.now())
        r = {"main": Local(ctx.nodes["main"].root, "main"), "sub": SSH(ctx.sub_host, ctx.nodes["sub"].root, "sub")}
        rs = {"main": Local(ctx.nodes["main"].short, "main"), "sub": SSH(ctx.sub_host, ctx.nodes["sub"].short, "sub")}
        ns = NativeServe(ctx, r, rs, clock=clock)
        if not a.apply:
            print("\n".join(ns.plan())); return EXIT_OK
        return ns.up(utc)
    except UnknownState as exc: print(f"FAIL_CLOSED_UNKNOWN: {exc}", file=sys.stderr); return EXIT_UNKNOWN
    except ServeError as exc: print(f"FAIL: {exc}", file=sys.stderr); return EXIT_FAIL


if __name__ == "__main__": raise SystemExit(main())
