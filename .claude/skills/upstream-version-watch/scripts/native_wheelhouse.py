#!/usr/bin/env python3
"""native_wheelhouse.py — 로컬 이미지의 설치 트리를 오프라인 wheelhouse + runtime-lib 번들로 재포장한다(재컴파일 ✗).

소유: upstream-version-watch · 근거 plan_26092311 §3 N-D1/N-D2 · §4.1 · AC-N1 · O-N2(재컴파일 ✗ — 불일치면 멈춘다).

왜: 이미지에는 `/opt/wheelhouse` 가 없다(F3). torch·triton 은 NGC dist-packages 설치본이고 vLLM 은 editable
(`/workspace/vllm-src`)이다. 호스트 청정 venv 가 이미지와 **같은 바이트**의 vLLM 을 돌리려면 설치 트리를 그대로 옮겨야
한다 — 다시 빌드하면 같은 것이 아니다. 그래서 `*.dist-info/RECORD` 가 말하는 파일만, RECORD 해시를 대조한 뒤 싣는다.

컨테이너 접근 규칙(hint-publisher `_probe_only_runner` 와 같은 규칙): `image inspect` · 시작하지 않는 `create`
(`--pull never --network none`) · 자기가 만든 컨테이너의 `cp`/`export`/`rm` 만. `run`/`start`/`exec`/`build`/`pull` 은
부르지 않는다 — 그래서 이미지 안에서 pip 을 돌릴 수 없고, 복사한 파일시스템을 읽는다.

CLI
  build  --image <tag> --out <abs dir> --generated-utc <UTC>
         → <out>/wheelhouse/*.whl · <out>/runtime-lib/{cuda,lib} · <out>/closure.json · <out>/requirements-closure.txt
  verify --wheelhouse-dir <out> --venv <abs dir> --generated-utc <UTC> [--diagnostic]
         → 청정 venv · pip install --no-index · pip check · ldd 미해소 0 · import torch,vllm + cuda 가용 → JSON 판정
  --self-test  → 임시 디렉터리의 가짜 설치 트리로 재포장·불일치 음성대조·editable·출처 필드를 검사(부수효과 0)

종료코드: 0 PASS · 2 사용법/기반 오류 · 3 판정 FAIL(목록을 closure.json/verify JSON 에 남긴다).
시각은 주입만(`--generated-utc`) — 벽시계를 기록하지 않는다(소요 초는 monotonic 측정값이며 그렇게 표시한다).
"""
from __future__ import annotations

import argparse
import ast
import base64
import configparser
import concurrent.futures as cf
import csv
import email.parser
import fnmatch
import hashlib
import io
import json
import os
import re
import shutil
import stat
import struct
import subprocess
import sys
import tarfile
import tempfile
import time
import zipfile
from pathlib import Path
from typing import Callable, Iterable, Optional

EXIT_OK, EXIT_USAGE, EXIT_FAIL = 0, 2, 3
TOOL = "native_wheelhouse.py/v1"
SITE_DEFAULT = "/usr/local/lib/python3.12/dist-packages"
# Debian python3.12 의 sys.path 순서(site.py): /usr/local/lib/python3.12/dist-packages → /usr/lib/python3/dist-packages →
#   /usr/lib/python3.12/dist-packages. NGC 이미지의 PyYAML·pygments 는 두 번째 자리(dpkg 설치본 · RECORD 없음)에만 있다.
SITES_DEFAULT = [SITE_DEFAULT, "/usr/lib/python3/dist-packages", "/usr/lib/python3.12/dist-packages"]
CUDA_LINK_DEFAULT = "/usr/local/cuda"
UTC_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")

# 제외 분포(닫힌 tripwire — 변경은 사람 리뷰). 이유는 closure.json `excluded[]` 에 그대로 실린다(침묵 배제 ✗).
EXCLUDE_DISTS = {
    "pip": "설치기 자신 — venv 부트스트랩(ensurepip)이 소유한다. 설치 도중 실행 중인 설치기를 교체하지 않는다.",
}
# pip 가 설치 시 스스로 쓰는 표식 — 휠에 싣지 않는다(direct_url.json 은 이미지 경로 `file:///workspace/…` 를 거짓으로 주장한다).
INSTALL_MARKERS = frozenset({"INSTALLER", "REQUESTED", "direct_url.json"})
# 호스트가 제공해야 하는 공유 라이브러리(로더·커널 ABI·인터프리터). 이미지 사본을 LD 경로에 올리면 호스트 로더/드라이버와 갈라진다.
HOST_GLIBC = re.compile(r"^(?:ld-linux-aarch64\.so\.1|ld-linux-x86-64\.so\.2|lib(?:c|m|dl|pthread|rt|util|resolv|anl|mvec|"
                        r"BrokenLocale|c_malloc_debug|thread_db|nsl|nss_[a-z]+)\.so\.\d+)$")
HOST_DRIVER = re.compile(r"^lib(?:cuda|nvidia-[a-z0-9-]+|nvcuvid|nvoptix|nvidia-encode)\.so(?:\.\d+)*$")
HOST_PYTHON = re.compile(r"^libpython\d+\.\d+\.so(?:\.\d+)*$")
# 커널 드라이버와 ABI 로 맞물리는 사용자 라이브러리 — 호스트에 있으면 호스트 것을 쓴다(없으면 이미지 사본을 싣고 기재).
#   rdma-core 는 provider 플러그인을 자기 설치 디렉터리에서 dlopen 한다 — 이미지 libibverbs + 호스트 provider 는 버전이 갈려
#   장치가 안 보이고 NCCL 이 조용히 소켓으로 내려간다.
KERNEL_ABI_PREFER_HOST = re.compile(r"^lib(?:ibverbs|rdmacm|mlx[45]|efa|nl-3|nl-route-3|ibumad|ibmad)\.so\.\d+$")
# dlopen 패밀리(닫힌 tripwire): DT_NEEDED 로 보이지 않고 부모가 같은 디렉터리에서 dlopen 한다.
DLOPEN_FAMILIES = [(re.compile(r"^libcudnn\.so\.(\d+)$"), r"^libcudnn_[a-z_]+\.so\.{0}$")]
ZIP_DATE = (1980, 1, 1, 0, 0, 0)
DEFLATE_LEVEL = 1  # 속도 우선(휘발 run root 의 크기 상한은 80 GiB · R4). 결정론: 같은 입력 → 같은 바이트.
CHUNK = 1 << 20


class WheelhouseError(RuntimeError):
    pass


def _b64(digest: bytes) -> str:
    return base64.urlsafe_b64encode(digest).rstrip(b"=").decode()


def hash_file(path: Path, algo: str = "sha256") -> tuple[str, int]:
    h = hashlib.new(algo)
    n = 0
    with open(path, "rb") as f:
        while True:
            b = f.read(CHUNK)
            if not b:
                break
            h.update(b)
            n += len(b)
    return _b64(h.digest()), n


def canon(name: str) -> str:
    return re.sub(r"[-_.]+", "-", name).lower()


def whl_escape(name: str) -> str:
    return re.sub(r"[-_.]+", "_", name)


def _is_pyc(rel: str) -> bool:
    return rel.endswith((".pyc", ".pyo")) or "/__pycache__/" in f"/{rel}"


# ─────────────────────────────── 이미지 파일시스템 소스 ───────────────────────────────

class Source:
    """이미지 파일시스템을 이미지 좌표(절대경로)로 읽는다. index = {path: ('f'|'l'|'d'|'o', link)}."""

    root: Path
    index: dict
    meta: dict

    def __init__(self) -> None:
        self.index = {}
        self.mounts: list[tuple[str, Path]] = []
        self._dirs: Optional[dict] = None

    # 이미지 좌표 → 로컬 경로(마운트 우선, 가장 긴 접두어)
    def local(self, ipath: str) -> Path:
        for pre, loc in sorted(self.mounts, key=lambda x: -len(x[0])):
            if ipath == pre or ipath.startswith(pre + "/"):
                return loc / ipath[len(pre):].lstrip("/")
        return self.root / ipath.lstrip("/")

    def add_mount(self, ipath: str, loc: Path) -> None:
        self.mounts.append((ipath, loc))

    def resolve(self, path: str, hops: int = 40) -> Optional[str]:
        """심볼릭 링크를 성분별로 따라가 실재 경로를 낸다(없으면 None)."""
        comps = [c for c in path.split("/") if c not in ("", ".")]
        cur: list[str] = []
        while comps:
            c = comps.pop(0)
            if c == "..":
                if cur:
                    cur.pop()
                continue
            cand = "/" + "/".join(cur + [c])
            ent = self.index.get(cand)
            if ent is None:
                return None
            if ent[0] == "l":
                hops -= 1
                if hops < 0:
                    return None
                link = ent[1]
                if link.startswith("/"):
                    cur = []
                comps = [x for x in link.split("/") if x not in ("", ".")] + comps
                continue
            cur.append(c)
        return "/" + "/".join(cur)

    def listdir(self, ipath: str) -> list[str]:
        if self._dirs is None:
            d: dict = {}
            for p in self.index:
                head, _, tail = p.rpartition("/")
                d.setdefault(head or "/", []).append(tail)
            self._dirs = d
        return sorted(self._dirs.get(ipath.rstrip("/") or "/", []))

    def is_file(self, real: Optional[str]) -> bool:
        return bool(real) and self.index.get(real, ("",))[0] == "f"

    # 하위 클래스
    def stage_site(self, site: str) -> Path: raise NotImplementedError
    def build_index(self, prefetch: Callable[[str], bool]) -> None: raise NotImplementedError
    def fetch(self, real: str) -> Path: raise NotImplementedError
    def fetch_tree(self, real: str, dest: Path) -> None: raise NotImplementedError
    def close(self) -> None: pass


class DirSource(Source):
    """자체검사용: 로컬 디렉터리를 이미지 루트로 본다."""

    def __init__(self, root: Path, meta: dict) -> None:
        super().__init__()
        self.root = root
        self.meta = meta

    def stage_site(self, site: str) -> Path:
        return self.local(site)

    def build_index(self, prefetch: Callable[[str], bool]) -> None:
        idx = {}
        for dp, dns, fns in os.walk(self.root):
            for n in dns + fns:
                full = os.path.join(dp, n)
                ip = "/" + os.path.relpath(full, self.root)
                st = os.lstat(full)
                if stat.S_ISLNK(st.st_mode):
                    idx[ip] = ("l", os.readlink(full))
                elif stat.S_ISDIR(st.st_mode):
                    idx[ip] = ("d", "")
                elif stat.S_ISREG(st.st_mode):
                    idx[ip] = ("f", "")
                else:
                    idx[ip] = ("o", "")
        self.index = idx

    def fetch(self, real: str) -> Path:
        return self.local(real)

    def fetch_tree(self, real: str, dest: Path) -> None:
        shutil.copytree(self.local(real), dest, symlinks=True)


class DockerSource(Source):
    """시작하지 않는 컨테이너 1개에서 cp/export 로만 읽는다(규칙 = hint-publisher `_probe_only_runner`)."""

    def __init__(self, image: str, staging: Path) -> None:
        super().__init__()
        self.image = image
        self.root = staging / "rootfs"
        self.cid = ""
        r = self._docker(["image", "inspect", image])
        info = json.loads(r.stdout)[0]
        self.meta = {
            "tag": image,
            "id": info["Id"],
            "repo_digests": info.get("RepoDigests") or [],
            "architecture": info.get("Architecture"),
            "os": info.get("Os"),
            "env": (info.get("Config") or {}).get("Env") or [],
        }

    def _docker(self, argv: list[str], stdout=subprocess.PIPE, timeout: Optional[int] = 600) -> subprocess.CompletedProcess:
        verb = argv[0]
        if verb == "image":
            ok = argv[1:2] == ["inspect"]
        elif verb == "create":
            ok = ("--pull", "never") in zip(argv, argv[1:]) and ("--network", "none") in zip(argv, argv[1:])
        elif verb in ("cp", "export", "rm"):
            ok = bool(self.cid) and any(a == self.cid or a.startswith(self.cid + ":") for a in argv[1:])
        else:
            ok = False
        if not ok:
            raise WheelhouseError(f"docker 호출 거부(탐침 전용 규칙): {' '.join(argv)}")
        r = subprocess.run(["docker", *argv], stdout=stdout, stderr=subprocess.PIPE, text=True, timeout=timeout)
        if r.returncode != 0:
            raise WheelhouseError(f"docker {' '.join(argv)} rc={r.returncode}: {r.stderr.strip()[-400:]}")
        return r

    def start(self) -> None:
        r = self._docker(["create", "--pull", "never", "--network", "none", self.image, "true"])
        cid = r.stdout.strip().splitlines()[-1].strip()
        if not re.fullmatch(r"[0-9a-f]{64}", cid):
            raise WheelhouseError(f"docker create 가 컨테이너 id 를 내지 않았다: {r.stdout!r}")
        self.cid = cid

    def stage_site(self, site: str) -> Path:
        dest = self.local(site)
        dest.parent.mkdir(parents=True, exist_ok=True)
        self._docker(["cp", f"{self.cid}:{site}", str(dest)], timeout=None)
        return dest

    def build_index(self, prefetch: Callable[[str], bool]) -> None:
        """`docker export` 1회 스트림 — 전 경로 색인 + prefetch 술어가 고른 정규 파일 추출."""
        proc = subprocess.Popen(["docker", "export", self.cid], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        idx: dict = {}
        try:
            with tarfile.open(fileobj=proc.stdout, mode="r|") as tf:
                for m in tf:
                    name = "/" + m.name.lstrip("./").rstrip("/")
                    if name == "/":
                        continue
                    if m.issym():
                        idx[name] = ("l", m.linkname)
                    elif m.isdir():
                        idx[name] = ("d", "")
                    elif m.isreg() or m.islnk():
                        idx[name] = ("f", "")
                        if m.isreg() and prefetch(name):
                            dest = self.local(name)
                            dest.parent.mkdir(parents=True, exist_ok=True)
                            src = tf.extractfile(m)
                            with open(dest, "wb") as out:
                                shutil.copyfileobj(src, out, CHUNK)
                            os.chmod(dest, m.mode & 0o777)
                    else:
                        idx[name] = ("o", "")
        finally:
            if proc.stdout:
                proc.stdout.close()
            rc = proc.wait()
        if rc != 0:
            raise WheelhouseError(f"docker export rc={rc}: {proc.stderr.read().decode(errors='replace')[-400:] if proc.stderr else ''}")
        self.index = idx

    def fetch(self, real: str) -> Path:
        dest = self.local(real)
        if dest.exists():
            return dest
        dest.parent.mkdir(parents=True, exist_ok=True)
        self._docker(["cp", f"{self.cid}:{real}", str(dest)], timeout=None)
        return dest

    def fetch_tree(self, real: str, dest: Path) -> None:
        dest.parent.mkdir(parents=True, exist_ok=True)
        self._docker(["cp", f"{self.cid}:{real}", str(dest)], timeout=None)

    def close(self) -> None:
        if self.cid:
            try:
                self._docker(["rm", "-f", "-v", self.cid])
            finally:
                self.cid = ""


# ─────────────────────────────── ELF(동적 섹션) ───────────────────────────────

def elf_dynamic(path: Path) -> Optional[dict]:
    """ELF64 LE 의 DT_NEEDED/RPATH/RUNPATH/SONAME. ELF 가 아니면 None."""
    try:
        with open(path, "rb") as f:
            hdr = f.read(64)
            if len(hdr) < 64 or hdr[:4] != b"\x7fELF":
                return None
            if hdr[4] != 2 or hdr[5] != 1:
                return {"needed": [], "rpath": [], "runpath": [], "soname": None, "error": "not-elf64-le"}
            shoff = struct.unpack_from("<Q", hdr, 0x28)[0]
            shentsize, shnum = struct.unpack_from("<HH", hdr, 0x3A)
            if not shoff or not shnum:
                return {"needed": [], "rpath": [], "runpath": [], "soname": None, "error": "no-section-headers"}
            f.seek(shoff)
            raw = f.read(shentsize * shnum)
            secs = [struct.unpack_from("<IIQQQQIIQQ", raw, i * shentsize) for i in range(shnum)]
            dyn = [s for s in secs if s[1] == 6]
            if not dyn:
                return {"needed": [], "rpath": [], "runpath": [], "soname": None}
            d = dyn[0]
            strsec = secs[d[6]]
            f.seek(strsec[4])
            strtab = f.read(strsec[5])
            f.seek(d[4])
            draw = f.read(d[5])
    except OSError as e:
        return {"needed": [], "rpath": [], "runpath": [], "soname": None, "error": f"read:{e}"}

    def s(off: int) -> str:
        end = strtab.find(b"\0", off)
        return strtab[off:end if end >= 0 else None].decode(errors="replace")
    out = {"needed": [], "rpath": [], "runpath": [], "soname": None}
    for i in range(0, len(draw) - 15, 16):
        tag, val = struct.unpack_from("<qQ", draw, i)
        if tag == 0:
            break
        if tag == 1:
            out["needed"].append(s(val))
        elif tag == 14:
            out["soname"] = s(val)
        elif tag == 15:
            out["rpath"] += [x for x in s(val).split(":") if x]
        elif tag == 29:
            out["runpath"] += [x for x in s(val).split(":") if x]
    return out


def looks_shared_object(name: str, mode: int) -> bool:
    return bool(re.search(r"\.so(\.\d+)*$", name)) or bool(mode & 0o111)


def host_ldcache() -> set:
    try:
        r = subprocess.run(["ldconfig", "-p"], capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return set()
    return {ln.strip().split(" ", 1)[0] for ln in r.stdout.splitlines()[1:] if "=>" in ln}


# ─────────────────────────────── dist-info 파싱 ───────────────────────────────

def read_dist(site_local: Path, dname: str) -> dict:
    di = site_local / dname
    meta_p = di / "METADATA"
    if not meta_p.exists():
        meta_p = di / "PKG-INFO"
    hdr = email.parser.HeaderParser().parsestr(meta_p.read_text(encoding="utf-8", errors="replace")) if meta_p.exists() else {}
    name = (hdr.get("Name") if hdr else None) or dname.rsplit("-", 1)[0]
    version = (hdr.get("Version") if hdr else None) or dname[: -len(".dist-info")].rsplit("-", 1)[-1]
    rows = []
    rec = di / "RECORD"
    if rec.exists():
        with open(rec, encoding="utf-8", newline="") as f:
            for row in csv.reader(f):
                if row and row[0]:
                    rows.append((row[0], row[1] if len(row) > 1 else ""))
    tags = []
    whl = di / "WHEEL"
    if whl.exists():
        tags = [ln.split(":", 1)[1].strip() for ln in whl.read_text(errors="replace").splitlines() if ln.startswith("Tag:")]
    eps = set()
    epf = di / "entry_points.txt"
    if epf.exists():
        cp = configparser.ConfigParser(delimiters=("=",), interpolation=None, strict=False)
        cp.optionxform = str  # type: ignore[assignment]
        try:
            cp.read_string(epf.read_text(errors="replace"))
            for sec in ("console_scripts", "gui_scripts"):
                if cp.has_section(sec):
                    eps.update(k.strip() for k in cp.options(sec))
        except configparser.Error:
            pass
    direct = None
    du = di / "direct_url.json"
    if du.exists():
        try:
            direct = json.loads(du.read_text())
        except ValueError:
            direct = None
    return {"dist_info": dname, "name": name, "version": version, "rows": rows, "has_record": rec.exists(),
            "tags": tags, "entry_scripts": eps, "direct_url": direct,
            "editable": bool(direct and (direct.get("dir_info") or {}).get("editable"))}


def editable_mapping(site_local: Path, dist: dict) -> dict:
    """setuptools editable finder 의 MAPPING(top-level 패키지 → 소스 디렉터리)을 ast 로 읽는다(실행 ✗)."""
    for rel, _ in dist["rows"]:
        if re.match(r"^__editable___.*_finder\.py$", rel):
            tree = ast.parse((site_local / rel).read_text())
            for node in tree.body:
                tgt = node.target if isinstance(node, ast.AnnAssign) else (node.targets[0] if isinstance(node, ast.Assign) else None)
                if isinstance(tgt, ast.Name) and tgt.id == "MAPPING" and node.value is not None:
                    return ast.literal_eval(node.value)
    url = ((dist.get("direct_url") or {}).get("url") or "")
    if url.startswith("file://"):
        # finder 가 없는 editable(.pth 가 소스 루트를 가리키는 형식) — top_level.txt 로 패키지를 고른다.
        srcroot = url[len("file://"):]
        tl = site_local / dist["dist_info"] / "top_level.txt"
        tops = [t.strip() for t in tl.read_text().splitlines() if t.strip()] if tl.exists() else []
        return {t: f"{srcroot}/{t}" for t in tops}
    return {}


def tag_string(tags: list[str]) -> str:
    pys, abis, plats = set(), set(), set()
    for t in tags:
        p, a, pl = t.split("-")
        pys.update(p.split("."))
        abis.update(a.split("."))
        plats.update(pl.split("."))
    return "-".join(".".join(sorted(x)) for x in (pys, abis, plats))


# ─────────────────────────────── 휠 쓰기 ───────────────────────────────

def write_wheel(dest: Path, entries: list[tuple[str, object, int]], distinfo: str) -> tuple[int, int]:
    """entries = (arcname, Path|bytes, mode). RECORD 는 실은 바이트로 새로 쓴다. 반환 (파일 수, 바이트)."""
    tmp = dest.with_suffix(".whl.part")
    rec_rows = []
    total = 0
    with zipfile.ZipFile(tmp, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=DEFLATE_LEVEL, allowZip64=True) as zf:
        for arc, src, mode in sorted(entries, key=lambda e: e[0]):
            zi = zipfile.ZipInfo(arc, ZIP_DATE)
            zi.compress_type = zipfile.ZIP_DEFLATED
            zi.external_attr = ((stat.S_IFREG | (mode & 0o777)) & 0xFFFF) << 16
            h = hashlib.sha256()
            n = 0
            with zf.open(zi, "w", force_zip64=True) as w:
                if isinstance(src, (bytes, bytearray)):
                    h.update(src)
                    w.write(src)
                    n = len(src)
                else:
                    with open(src, "rb") as f:  # type: ignore[arg-type]
                        while True:
                            b = f.read(CHUNK)
                            if not b:
                                break
                            h.update(b)
                            w.write(b)
                            n += len(b)
            rec_rows.append([arc, f"sha256={_b64(h.digest())}", str(n)])
            total += n
        rec_rows.append([f"{distinfo}/RECORD", "", ""])
        buf = io.StringIO()
        csv.writer(buf, lineterminator="\n").writerows(rec_rows)
        zi = zipfile.ZipInfo(f"{distinfo}/RECORD", ZIP_DATE)
        zi.compress_type = zipfile.ZIP_DEFLATED
        zi.external_attr = ((stat.S_IFREG | 0o644) & 0xFFFF) << 16
        zf.writestr(zi, buf.getvalue())
    os.replace(tmp, dest)
    return len(entries), total


# ─────────────────────────────── build ───────────────────────────────

def _ld_conf_dirs(src: Source) -> list[str]:
    out: list[str] = []

    def parse(path: str, depth: int) -> None:
        real = src.resolve(path)
        if depth > 8 or not src.is_file(real):
            return
        for ln in src.fetch(real).read_text(errors="replace").splitlines():
            ln = ln.split("#", 1)[0].strip()
            if not ln:
                continue
            if ln.startswith("include"):
                pat = ln.split(None, 1)[1].strip()
                if not pat.startswith("/"):
                    pat = os.path.join(os.path.dirname(path), pat)
                for p in sorted(p for p in src.index if fnmatch.fnmatch(p, pat)):
                    parse(p, depth + 1)
            else:
                out.append(ln.rstrip("/"))
    parse("/etc/ld.so.conf", 0)
    return out


def _dpkg_rows(src: Source, dsite: str, dist_info: str) -> tuple[Optional[str], list]:
    """RECORD 가 없는 Debian 설치본(dh_python): dpkg `.list` 로 소유 파일을, `.md5sums` 로 해시를 얻는다(RECORD 대체 무결성 원천)."""
    want = f"{dsite}/{dist_info}"
    for lp in sorted(p for p in src.index if p.startswith("/var/lib/dpkg/info/") and p.endswith(".list")):
        real = src.resolve(lp)
        if not src.is_file(real):
            continue
        lines = src.fetch(real).read_text(errors="replace").splitlines()
        if want not in lines:
            continue
        pkg = os.path.basename(lp)[: -len(".list")]
        md5 = {}
        mp = src.resolve(f"/var/lib/dpkg/info/{pkg}.md5sums")
        if src.is_file(mp):
            for ln in src.fetch(mp).read_text(errors="replace").splitlines():
                if "  " in ln:
                    hx, path = ln.split("  ", 1)
                    md5["/" + path.strip().lstrip("/")] = "md5=" + _b64(bytes.fromhex(hx.strip()))
        rows = []
        for ln in lines:
            if ln.startswith(dsite + "/") and src.is_file(src.resolve(ln)):
                rows.append((os.path.relpath(ln, dsite), md5.get(ln, "")))
        return pkg, rows
    return None, []


def build(src: Source, out: Path, sites: list, cuda_link: str, generated_utc: str, accept: dict, jobs: int,
          keep_staging: bool = False, staging: Optional[Path] = None, accept_meta: Optional[dict] = None) -> dict:
    """sites = 이미지 파이썬의 sys.path 순서(앞이 우선). 뒤 디렉터리의 같은 이름 분포는 가려진 것(shadowed)이다."""
    t0 = time.monotonic()
    if accept_meta and accept_meta.get("image_id") != src.meta.get("id"):
        raise WheelhouseError(f"승인 파일의 이미지 {accept_meta.get('image_id')} ≠ 재포장 대상 {src.meta.get('id')} — 다른 이미지의 승인은 쓰지 않는다")
    accept_used: set = set()
    wh = out / "wheelhouse"
    rl = out / "runtime-lib"
    wh.mkdir(parents=True)
    rl.mkdir(parents=True)
    image_ref = f"image@{src.meta['id']}"
    failures: list[dict] = []
    warnings: list[dict] = []
    site = sites[0]
    m = re.search(r"/python(\d)\.(\d+)/", site + "/")
    if not m:
        raise WheelhouseError(f"site-packages 경로에서 파이썬 버전을 파생하지 못했다: {site}")
    py_tag = f"cp{m.group(1)}{m.group(2)}"
    arch = {"arm64": "aarch64", "amd64": "x86_64"}.get(src.meta.get("architecture") or "", src.meta.get("architecture") or "")
    plat_tag = f"linux_{arch}"
    triplet = f"{arch}-linux-gnu"

    staged: list[tuple[str, Path]] = []
    sites_note = []
    for i, sd in enumerate(sites):
        try:
            loc = src.stage_site(sd)
        except WheelhouseError as e:
            if i == 0:
                raise
            sites_note.append({"site": sd, "present": False, "detail": str(e)[-200:]})
            continue
        if not loc.is_dir():
            if i == 0:
                raise WheelhouseError(f"주 site-packages 부재: {sd}")
            sites_note.append({"site": sd, "present": False})
            continue
        sites_note.append({"site": sd, "present": True})
        staged.append((sd, loc))

    dists = []
    excluded: list[dict] = []
    seen_canon: dict = {}
    for sd, loc in staged:
        pre = sd.rsplit("/lib/", 1)[0]
        for dn in sorted(os.listdir(loc)):
            if dn.endswith(".dist-info"):
                d = read_dist(loc, dn)
            elif dn.endswith(".egg-info"):
                pk = loc / dn / "PKG-INFO"
                hdr = email.parser.HeaderParser().parsestr(pk.read_text(errors="replace")) if pk.exists() else {}
                d = {"dist_info": dn, "name": (hdr.get("Name") if hdr else None) or dn.split("-")[0],
                     "version": (hdr.get("Version") if hdr else None) or "", "egg_info": True}
            else:
                continue
            d.update(site=sd, site_local=loc, prefix=pre, key=f"{sd}/{dn}")
            c = canon(d["name"])
            if c in seen_canon:
                excluded.append({"name": d["name"], "version": d["version"], "dist_info": d["key"],
                                 "reason": f"shadowed — 앞선 sys.path 의 {seen_canon[c]} 가 이긴다(이미지에서도 import 되지 않는다)",
                                 "provenance": "derived:site-order"})
                continue
            seen_canon[c] = d["key"]
            if d.get("egg_info"):
                failures.append({"dist": d["name"], "kind": "egg-info-unsupported", "path": d["key"]})
                continue
            dists.append(d)

    # 1) 색인 + 트리 밖 RECORD 파일(bin/share/etc…) · ld.so.conf · dpkg 목록을 export 1회로 가져온다.
    outside: set = set()
    for d in dists:
        for rel, _ in d["rows"]:
            ip = os.path.normpath(os.path.join(d["site"], rel))
            if not ip.startswith(d["site"] + "/"):
                outside.add(ip)

    def prefetch(p: str) -> bool:
        return (p in outside or p == "/etc/ld.so.conf" or p.startswith("/etc/ld.so.conf.d/")
                or (p.startswith("/var/lib/dpkg/info/") and p.endswith((".list", ".md5sums"))))
    src.build_index(prefetch)

    # RECORD 없는 분포 → dpkg 매니페스트(있으면)로 목록·해시를 채운다
    for d in dists:
        if d["has_record"]:
            d["record_source"] = "RECORD"
            continue
        pkg, rows = _dpkg_rows(src, d["site"], d["dist_info"])
        if pkg and rows:
            d["rows"] = rows
            d["has_record"] = True
            d["record_source"] = f"dpkg:{pkg}.list+md5sums"

    # 2) RECORD 대조 해시(현재 바이트)
    plan: dict = {}  # (key, rel) → (ipath, real)
    need_hash: dict = {}  # real → algo set
    for d in dists:
        for rel, h in d["rows"]:
            if _is_pyc(rel) or rel == f"{d['dist_info']}/RECORD":
                continue
            ip = os.path.normpath(os.path.join(d["site"], rel))
            real = src.resolve(ip)
            plan[(d["key"], rel)] = (ip, real if src.is_file(real) else None)
            if src.is_file(real) and h:
                need_hash.setdefault(real, set()).add(h.split("=", 1)[0])

    def _hash(real: str) -> tuple[str, dict]:
        p = src.fetch(real)
        return real, {a: hash_file(p, a)[0] for a in need_hash[real]}
    cur_hash: dict = {}
    with cf.ThreadPoolExecutor(max_workers=jobs) as ex:
        for real, hs in ex.map(_hash, sorted(need_hash)):
            cur_hash[real] = hs

    owners: dict = {}
    for d in dists:
        for rel, h in d["rows"]:
            owners.setdefault(os.path.normpath(os.path.join(d["site"], rel)), []).append((d["key"], h))

    def matches(real: Optional[str], h: str) -> bool:
        if not real or not h:
            return False
        algo, val = h.split("=", 1)
        return cur_hash.get(real, {}).get(algo) == val

    # 3) 분포별 결정
    records: list[dict] = []
    jobs_list: list[tuple] = []
    elf_roots: list[tuple[str, Path]] = []  # (이미지 좌표, 로컬 경로) — runtime 폐포 뿌리
    image_wheels = {p for p in src.index if p.endswith(".whl") and src.index[p][0] == "f"}

    for d in dists:
        c = canon(d["name"])
        if c in EXCLUDE_DISTS:
            excluded.append({"name": d["name"], "version": d["version"], "dist_info": d["dist_info"],
                             "reason": EXCLUDE_DISTS[c], "provenance": "tripwire:EXCLUDE_DISTS"})
            continue
        rec: dict = {"name": d["name"], "version": d["version"], "dist_info": d["dist_info"], "wheel": None,
                     "source": None, "file_count": 0, "payload_bytes": 0,
                     "record_verify": {"result": None, "verified": 0, "unhashed": [], "mismatches": [], "missing": [],
                                       "shared_path_omitted": [], "accepted": []},
                     "scripts": {"regenerated_entry_points": [], "packed": [], "shebang_rewritten": 0},
                     "data_files": 0, "dropped": [], "site": d["site"], "record_source": d.get("record_source")}
        rv = rec["record_verify"]
        if not d["has_record"]:
            failures.append({"dist": d["name"], "kind": "record-absent", "detail": f"{d['dist_info']}/RECORD 없음"})
            rv["result"] = "FAIL"
            records.append(rec)
            continue
        datadir = f"{whl_escape(d['name'])}-{d['version']}.data"
        entries: list[tuple[str, object, int]] = []
        for rel, h in d["rows"]:
            base = os.path.basename(rel)
            if _is_pyc(rel):
                continue
            if rel == f"{d['dist_info']}/RECORD":
                continue
            if rel.startswith(d["dist_info"] + "/") and base in INSTALL_MARKERS:
                rec["dropped"].append({"path": rel, "reason": "install-marker(pip 가 설치 시 다시 쓴다)"})
                continue
            if d["editable"] and (base.startswith("__editable__") or re.match(r"^__editable___.*_finder\.py$", rel)):
                rec["dropped"].append({"path": rel, "reason": "editable-shim(실제 패키지 트리로 대체)"})
                continue
            ip, real = plan[(d["key"], rel)]
            if real is None:
                rv["missing"].append(rel)
                failures.append({"dist": d["name"], "kind": "record-file-missing", "path": rel})
                continue
            if h:
                if matches(real, h):
                    rv["verified"] += 1
                else:
                    owner = next((o for o, oh in owners.get(ip, []) if o != d["key"] and matches(real, oh)), None)
                    key = f"{c}:{rel}"
                    if owner:
                        rv["shared_path_omitted"].append({"path": rel, "owner": owner})
                        continue
                    if key in accept:
                        accept_used.add(key)
                        rv["accepted"].append({"path": rel, "reason": accept[key]["reason"], "provenance": accept[key]["provenance"]})
                    else:
                        rv["mismatches"].append(rel)
                        failures.append({"dist": d["name"], "kind": "record-hash-mismatch", "path": rel})
            else:
                rv["unhashed"].append(rel)
            local = src.fetch(real)
            mode = os.stat(local).st_mode
            if ip.startswith(d["site"] + "/"):
                arc = os.path.relpath(ip, d["site"])
                entries.append((arc, local, mode))
                if looks_shared_object(arc, mode):
                    elf_roots.append((ip, local))
            elif ip.startswith(d["prefix"] + "/bin/"):
                if base in d["entry_scripts"]:
                    rec["scripts"]["regenerated_entry_points"].append(base)
                    continue
                data = local.read_bytes()
                if data.startswith(b"#!") and b"python" in data.split(b"\n", 1)[0]:
                    data = b"#!python\n" + (data.split(b"\n", 1)[1] if b"\n" in data else b"")
                    rec["scripts"]["shebang_rewritten"] += 1
                entries.append((f"{datadir}/scripts/{base}", data, mode | 0o755))
                rec["scripts"]["packed"].append(base)
            elif ip.startswith(d["prefix"] + "/"):
                entries.append((f"{datadir}/data/{os.path.relpath(ip, d['prefix'])}", local, mode))
                rec["data_files"] += 1
            else:
                failures.append({"dist": d["name"], "kind": "unplaceable-path", "path": rel})
        tags = d["tags"]
        if d["editable"]:
            mapping = editable_mapping(d["site_local"], d)
            if not mapping:
                failures.append({"dist": d["name"], "kind": "editable-mapping-absent", "detail": "finder MAPPING/direct_url 없음"})
            rec["editable"] = {"direct_url": (d["direct_url"] or {}).get("url"), "mapping": mapping, "tree_files": 0,
                               "tree_verify": "n/a(소스 트리는 RECORD 가 없다 — 설치 트리 그대로 싣고 파일 목록을 기재)"}
            has_elf = False
            for top, srcdir in sorted(mapping.items()):
                real = src.resolve(srcdir)
                if not real:
                    failures.append({"dist": d["name"], "kind": "editable-source-missing", "path": srcdir})
                    continue
                tdest = (staging or out / ".staging") / "editable" / top
                src.fetch_tree(real, tdest)
                for dp, dns, fns in os.walk(tdest):
                    dns[:] = sorted(x for x in dns if x != "__pycache__")
                    for fn in sorted(fns):
                        if fn.endswith((".pyc", ".pyo")):
                            continue
                        lp = Path(dp) / fn
                        if lp.is_symlink():
                            tgt = src.resolve(real + "/" + os.path.relpath(lp, tdest))
                            if not src.is_file(tgt):
                                failures.append({"dist": d["name"], "kind": "editable-dangling-symlink", "path": str(lp)})
                                continue
                            lp = src.fetch(tgt)
                        relp = os.path.relpath(Path(dp) / fn, tdest)
                        mode = os.stat(lp).st_mode
                        entries.append((f"{top}/{relp}", lp, mode))
                        rec["editable"]["tree_files"] += 1
                        if looks_shared_object(fn, mode):
                            elf_roots.append((f"{real}/{relp}", lp))
                            has_elf = has_elf or fn.endswith(".so")
            tags = [f"{py_tag}-{py_tag}-{plat_tag}"] if has_elf else ["py3-none-any"]
            wheel_txt = ("Wheel-Version: 1.0\nGenerator: " + TOOL + "\nRoot-Is-Purelib: " + ("false" if has_elf else "true")
                         + "\n" + "".join(f"Tag: {t}\n" for t in tags))
            entries = [e for e in entries if e[0] != f"{d['dist_info']}/WHEEL"] + [(f"{d['dist_info']}/WHEEL", wheel_txt.encode(), 0o644)]
            rec["source"] = f"editable-repacked({image_ref})"
        else:
            if not tags:
                has_so = any(e[0].endswith(".so") or ".so." in e[0] for e in entries)
                tags = [f"{py_tag}-{py_tag}-{plat_tag}"] if has_so else ["py3-none-any"]
                wheel_txt = ("Wheel-Version: 1.0\nGenerator: " + TOOL + "\nRoot-Is-Purelib: " + ("false" if has_so else "true")
                             + "\n" + "".join(f"Tag: {t}\n" for t in tags))
                entries.append((f"{d['dist_info']}/WHEEL", wheel_txt.encode(), 0o644))
                warnings.append({"dist": d["name"], "kind": "wheel-file-absent", "detail": f"WHEEL 생성(Tag {tags[0]})"})
            rec["source"] = f"repacked({image_ref})"
        rv["result"] = "FAIL" if (rv["mismatches"] or rv["missing"]) else ("ACCEPTED" if rv["accepted"] else "PASS")

        # 이미지 안 원본 휠(설치 기록이 그 휠 해시를 말하고, 설치본이 RECORD 와 일치할 때만)
        arch_info = ((d["direct_url"] or {}).get("archive_info") or {})
        url = (d["direct_url"] or {}).get("url") or ""
        whl_ip = url[len("file://"):].replace("%2B", "+") if url.startswith("file://") and url.endswith(".whl") else ""
        want = (arch_info.get("hashes") or {}).get("sha256") or (arch_info.get("hash", "").split("=", 1)[-1] if arch_info.get("hash", "").startswith("sha256=") else "")
        if whl_ip in image_wheels and want and rv["result"] == "PASS" and not rv["shared_path_omitted"]:
            lp = src.fetch(whl_ip)
            got = hashlib.sha256(lp.read_bytes()).hexdigest()
            if got == want:
                dst = wh / os.path.basename(whl_ip)
                shutil.copyfile(lp, dst)
                rec.update(wheel=dst.name, source="wheel(image)", file_count=len(entries),
                           image_wheel={"path": whl_ip, "sha256": got, "matched": "direct_url.archive_info"})
                records.append(rec)
                continue
            warnings.append({"dist": d["name"], "kind": "image-wheel-hash-differs", "detail": f"{whl_ip} {got} ≠ {want} → 재포장"})
        fname = f"{whl_escape(d['name'])}-{d['version'].replace('-', '_')}-{tag_string(tags)}.whl"
        rec["wheel"] = fname
        rec["file_count"] = len(entries)
        jobs_list.append((rec, wh / fname, entries, d["dist_info"]))
        records.append(rec)

    def _write(job: tuple) -> None:
        rec, dest, entries, distinfo = job
        n, b = write_wheel(dest, entries, distinfo)
        rec["file_count"], rec["payload_bytes"] = n, b
    with cf.ThreadPoolExecutor(max_workers=jobs) as ex:
        list(ex.map(_write, sorted(jobs_list, key=lambda j: -sum(os.path.getsize(e[1]) if isinstance(e[1], Path) else len(e[1]) for e in j[2]))))

    # 4) runtime-lib: CUDA 트리(compat 포함) + DT_NEEDED 폐포 + dlopen 패밀리
    cuda_real = src.resolve(cuda_link)
    if not cuda_real or src.index.get(cuda_real, ("",))[0] != "d":
        failures.append({"kind": "cuda-tree-absent", "path": cuda_link})
        cuda_real = None
    else:
        src.fetch_tree(cuda_real, rl / "cuda")
        src.add_mount(cuda_real, rl / "cuda")
    env = dict(e.split("=", 1) for e in src.meta.get("env", []) if "=" in e)
    img_ld = [p.rstrip("/") for p in env.get("LD_LIBRARY_PATH", "").split(":") if p]
    conf_dirs = _ld_conf_dirs(src)
    defaults = [f"/lib/{triplet}", f"/usr/lib/{triplet}", "/lib", "/usr/lib"]
    hostcache = host_ldcache()
    packaged_ips = {ip for (ip, _) in elf_roots}

    bundled: dict = {}   # real → {sonames, reason, needed_by}
    classes: dict = {}   # soname → class(host-*)
    unresolved: dict = {}
    covered = {"wheel": 0, "cuda-tree": 0}
    seen: set = set()
    queue: list[tuple[str, Path]] = list(elf_roots)
    rcache: dict = {}

    def search(name: str, origin: str, dyn: dict) -> Optional[str]:
        def sub(p: str) -> str:
            return p.replace("${ORIGIN}", origin).replace("$ORIGIN", origin)
        dirs = ([sub(x) for x in dyn["rpath"]] if not dyn["runpath"] else []) + img_ld + [sub(x) for x in dyn["runpath"]] + conf_dirs + defaults
        key = (name, tuple(dirs))
        if key in rcache:
            return rcache[key]
        hit = None
        for dd in dirs:
            r = src.resolve(os.path.normpath(dd) + "/" + name)
            if src.is_file(r):
                hit = r
                break
        rcache[key] = hit
        return hit

    def classify_host(name: str) -> Optional[str]:
        if HOST_GLIBC.match(name):
            return "host-glibc"
        if HOST_DRIVER.match(name):
            return "host-driver"
        if HOST_PYTHON.match(name):
            return "host-python"
        if KERNEL_ABI_PREFER_HOST.match(name) and name in hostcache:
            return "host-kernel-abi"
        return None

    def add_bundle(real: str, soname: str, reason: str, by: str) -> None:
        ent = bundled.setdefault(real, {"sonames": [], "reason": reason, "needed_by": []})
        if soname not in ent["sonames"]:
            ent["sonames"].append(soname)
        if len(ent["needed_by"]) < 3 and by not in ent["needed_by"]:
            ent["needed_by"].append(by)

    def walk_queue() -> None:
        while queue:
            ip, lp = queue.pop()
            if ip in seen:
                continue
            seen.add(ip)
            dyn = elf_dynamic(lp)
            if dyn is None:
                continue
            if dyn.get("error"):
                warnings.append({"kind": "elf-unparsed", "path": ip, "detail": dyn["error"]})
            origin = os.path.dirname(ip)
            for n in dyn["needed"]:
                hc = classify_host(n)
                if hc:
                    classes[n] = hc
                    continue
                r = search(n, origin, dyn)
                if r is None:
                    if n in hostcache:
                        classes[n] = "host-ldcache"
                    else:
                        lst = unresolved.setdefault(n, [])
                        if len(lst) < 3:
                            lst.append(ip)
                    continue
                if any(r.startswith(sd + "/") for sd, _ in staged):
                    covered["wheel"] += 1
                    if r not in packaged_ips and r not in seen:
                        warnings.append({"kind": "site-lib-not-packaged", "path": r, "needed_by": ip})
                    queue.append((r, src.fetch(r)))
                    continue
                if cuda_real and r.startswith(cuda_real + "/"):
                    covered["cuda-tree"] += 1
                    queue.append((r, src.fetch(r)))
                    continue
                add_bundle(r, n, "DT_NEEDED", ip)
                queue.append((r, src.fetch(r)))
    walk_queue()
    # dlopen 패밀리: 번들된 부모와 같은 디렉터리의 형제
    for real in list(bundled):
        for son in list(bundled[real]["sonames"]):
            for pat, fam in DLOPEN_FAMILIES:
                mm = pat.match(son)
                if not mm:
                    continue
                famre = re.compile(fam.format(*mm.groups()))
                ddir = os.path.dirname(real)
                for sib in src.listdir(ddir):
                    if famre.match(sib):
                        r = src.resolve(f"{ddir}/{sib}")
                        if src.is_file(r):
                            add_bundle(r, sib, f"dlopen-family:{son}", real)
                            queue.append((r, src.fetch(r)))
    plug = env.get("NCCL_NET_PLUGIN", "")
    if plug and plug.lower() not in ("none", ""):
        name = f"libnccl-net-{plug}.so"
        r = next((src.resolve(os.path.normpath(dd) + "/" + name) for dd in img_ld + conf_dirs + defaults
                  if src.is_file(src.resolve(os.path.normpath(dd) + "/" + name))), None)
        if r:
            add_bundle(r, name, "dlopen:image-env NCCL_NET_PLUGIN", "env")
            queue.append((r, src.fetch(r)))
        else:
            warnings.append({"kind": "nccl-plugin-not-found", "detail": name})
    walk_queue()

    libdir = rl / "lib"
    libdir.mkdir()
    bundled_out = []
    for real in sorted(bundled):
        ent = bundled[real]
        first = ent["sonames"][0]
        lp = src.fetch(real)
        shutil.copy2(lp, libdir / first)
        for other in ent["sonames"][1:]:
            if not (libdir / other).exists():
                os.symlink(first, libdir / other)
        dg, sz = hash_file(libdir / first)
        bundled_out.append({"sonames": ent["sonames"], "image_path": real, "sha256": dg, "bytes": sz,
                            "reason": ent["reason"], "needed_by": ent["needed_by"], "provenance": f"copied({image_ref})"})

    # LD_LIBRARY_PATH 틀(이미지 env 순서 → venv/번들 좌표로 사상). compat 은 별도 표식(verify 가 유무 두 후보를 판정).
    compat_dir = None
    template: list[dict] = []
    for p in img_ld:
        if p.startswith(site + "/"):
            template.append({"path": "{VENV_SITE}/" + os.path.relpath(p, site), "from": p})
        elif p.rstrip("/").endswith("/compat/lib") and cuda_real:
            # shinit_v2: lib → lib.real(/dev/nvgpu 가 있으면 lib.real-nvgpu). 이미지에는 lib 심볼릭이 없고 런타임 검사가 만든다.
            cands = [x for x in ("lib.real-nvgpu" if os.path.exists("/dev/nvgpu") else "lib.real",)
                     if src.resolve(f"{cuda_real}/compat/{x}")]
            if cands:
                compat_dir = "{OUT}/runtime-lib/cuda/compat/" + cands[0]
                template.append({"path": compat_dir, "from": p, "compat": True})
        else:
            r = src.resolve(p)
            if r and cuda_real and (r == cuda_real or r.startswith(cuda_real + "/")):
                template.append({"path": "{OUT}/runtime-lib/cuda/" + os.path.relpath(r, cuda_real), "from": p})
            else:
                template.append({"path": None, "from": p, "dropped": "이미지 밖 주입 경로(컨테이너 툴킷) 또는 부재 — 호스트 로더가 대신한다"})
    template.append({"path": "{OUT}/runtime-lib/lib", "from": "bundled DT_NEEDED closure"})
    for p in conf_dirs:
        r = src.resolve(p)
        if r and cuda_real and (r == cuda_real or r.startswith(cuda_real + "/")) and not r.startswith(cuda_real + "/compat"):
            q = "{OUT}/runtime-lib/cuda/" + os.path.relpath(r, cuda_real)
            if all(t.get("path") != q for t in template):
                template.append({"path": q, "from": f"ld.so.conf:{p}"})
    env_map = {}
    for k, v in sorted(env.items()):
        if cuda_real and any(seg.startswith((cuda_link, cuda_real)) for seg in v.split(":")) and k not in ("PATH", "LD_LIBRARY_PATH", "LIBRARY_PATH"):
            nv = v
            for pre in sorted({cuda_real, cuda_link}, key=len, reverse=True):
                nv = nv.replace(pre, "{OUT}/runtime-lib/cuda")
            env_map[k] = nv

    unowned = []
    listed = {os.path.normpath(os.path.join(d["site"], rel)) for d in dists for rel, _ in d["rows"]}
    for sd, loc in staged:
        for dp, dns, fns in os.walk(loc):
            dns[:] = [x for x in dns if x != "__pycache__" and not x.endswith((".dist-info", ".egg-info"))]
            for fn in fns:
                ip = sd + "/" + os.path.relpath(os.path.join(dp, fn), loc)
                if ip not in listed and not fn.endswith((".pyc", ".pyo")):
                    unowned.append(ip)

    def du(p: Path) -> int:
        tot = 0
        for dp, _, fns in os.walk(p):
            for fn in fns:
                try:
                    tot += os.lstat(os.path.join(dp, fn)).st_blocks * 512
                except OSError:
                    pass
        return tot

    stale = sorted(set(accept) - accept_used)
    for k in stale:
        # 승인했는데 일어나지 않은 불일치 — 이미지가 바뀌었거나 선언이 낡았다. 침묵하지 않는다(판정은 바꾸지 않고 소리낸다).
        warnings.append({"kind": "stale-accept", "entry": k, "detail": "승인 목록에 있으나 이번 재포장에서 불일치가 관측되지 않았다"})
        print(f"native_wheelhouse: WARNING stale-accept {k} — 승인 항목이 관측되지 않았다(선언 갱신 필요)", file=sys.stderr)
    status = "FAIL" if failures else "PASS"
    closure = {
        "schema_version": 1, "kind": "native_wheelhouse_closure", "tool": TOOL, "generated_utc": generated_utc,
        "status": status, "failures": failures, "warnings": warnings,
        "image": {k: src.meta.get(k) for k in ("tag", "id", "repo_digests", "architecture", "os")},
        "image_env": src.meta.get("env", []),
        "site_packages": sites_note, "python_tag": py_tag, "platform_tag": plat_tag,
        "provenance": {"method": "repack-from-installed-tree(RECORD)", "recompiled": False, "network": False,
                       "container_access": "create(--pull never --network none)/cp/export/rm only"},
        "counts": {"distributions_seen": len(dists), "wheels": sum(1 for r in records if r["wheel"]),
                   "repacked": sum(1 for r in records if (r["source"] or "").startswith("repacked")),
                   "editable_repacked": sum(1 for r in records if (r["source"] or "").startswith("editable-repacked")),
                   "image_wheels": sum(1 for r in records if r["source"] == "wheel(image)"),
                   "excluded": len(excluded),
                   "record_mismatches": sum(len(r["record_verify"]["mismatches"]) for r in records),
                   "shared_path_omitted": sum(len(r["record_verify"]["shared_path_omitted"]) for r in records)},
        "accept": ({**accept_meta, "applied": sorted(accept_used), "stale": stale} if accept_meta
                   else ({"source": "cli --accept-mismatch", "applied": sorted(accept_used), "stale": stale} if accept else None)),
        "distributions": records, "excluded": excluded,
        "unowned_site_files": {"count": len(unowned), "sample": sorted(unowned)[:40],
                               "note": "어느 RECORD 에도 없는 site-packages 파일 — 휠에 실리지 않는다(venv 에 없다)"},
        "runtime_lib": {
            "cuda_tree": {"image_link": cuda_link, "image_real": cuda_real, "local": "runtime-lib/cuda",
                          "compat_dir": compat_dir},
            "bundled": bundled_out, "host_provided": dict(sorted(classes.items())),
            "unresolved": {k: v for k, v in sorted(unresolved.items())},
            "covered_needed_edges": covered,
            "ld_library_path_template": template, "env_template": env_map,
            "path_prepend_template": ["{OUT}/runtime-lib/cuda/bin"] if cuda_real else [],
        },
        "sizes": {"wheelhouse_bytes": du(wh), "runtime_lib_bytes": du(rl)},
        "elapsed_s_monotonic": round(time.monotonic() - t0, 1),
    }
    req = ["# native_wheelhouse.py 생성 — 정확 핀(이미지 설치본 그대로) · pip install --no-index 전용",
           f"# image {src.meta.get('tag')} {src.meta['id']} · generated_utc {generated_utc}"]
    req += sorted(f"{r['name']}=={r['version']}" for r in records if r["wheel"])
    (out / "requirements-closure.txt").write_text("\n".join(req) + "\n")
    (out / "closure.json").write_text(json.dumps(closure, ensure_ascii=False, indent=1, sort_keys=False) + "\n")
    return closure


# ─────────────────────────────── verify ───────────────────────────────

IMPORT_PROBE = r"""
import json, sys
import torch, vllm
ok = bool(torch.cuda.is_available())
print(json.dumps({"torch": torch.__version__, "torch_cuda": torch.version.cuda, "vllm": getattr(vllm, "__version__", None),
                  "torch_file": torch.__file__, "vllm_file": vllm.__file__, "cuda_available": ok,
                  "device_count": torch.cuda.device_count() if ok else 0}))
sys.exit(0 if ok else 7)
"""


def _clean_env(extra: dict) -> dict:
    keep = {k: os.environ[k] for k in ("HOME", "USER", "LANG", "LC_ALL", "TERM") if k in os.environ}
    keep["PATH"] = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
    keep.update({"PIP_NO_INDEX": "1", "PIP_CONFIG_FILE": os.devnull, "PIP_DISABLE_PIP_VERSION_CHECK": "1",
                 "PIP_NO_INPUT": "1", "PYTHONNOUSERSITE": "1"})
    keep.update(extra)
    return keep


def _tail(s: str, n: int = 3000) -> str:
    return s[-n:] if s else ""


GATE_RESOLVER = "resolver(pip install --no-index 리졸버 · pip check 0)"
GATE_PARITY = "image-parity(accept-file: --no-deps 정확 핀 · 설치 집합 == closure 핀 · pip check ⊆ image_inherent_conflicts)"


def verify(whdir: Path, venv: Path, generated_utc: str, diagnostic: bool, accept_path: Optional[Path] = None) -> dict:
    closure = json.loads((whdir / "closure.json").read_text())
    declared: list[str] = []
    ameta = None
    if accept_path is not None:
        _, ameta, adoc = load_accept_file(accept_path)
        if ameta["image_id"] != (closure.get("image") or {}).get("id"):
            raise WheelhouseError(f"승인 파일 이미지 {ameta['image_id']} ≠ closure 이미지 {(closure.get('image') or {}).get('id')}")
        declared = [str(e["pip_check_line"]).strip() for e in adoc["image_inherent_conflicts"]]
    parity = ameta is not None
    res: dict = {"schema_version": 1, "kind": "native_wheelhouse_verify", "tool": TOOL, "generated_utc": generated_utc,
                 "wheelhouse_dir": str(whdir), "venv": str(venv), "closure_status": closure.get("status"),
                 "gate_definition": GATE_PARITY if parity else GATE_RESOLVER,
                 "accept": ({k: ameta[k] for k in ("sha256", "approved_by", "approved_utc", "image_id")} if parity else None),
                 "diagnostic_only": bool(diagnostic), "gates": {}, "verdict": "FAIL"}
    if closure.get("status") != "PASS" and not diagnostic:
        res["gates"]["closure"] = {"result": "FAIL", "detail": "closure.json status≠PASS — --diagnostic 없이는 설치하지 않는다"}
        return res
    res["gates"]["closure"] = {"result": "PASS" if closure.get("status") == "PASS" else "FAIL(diagnostic 진행)"}
    if venv.exists() and any(venv.iterdir()):
        raise WheelhouseError(f"venv 경로가 비어 있지 않다(덮어쓰지 않는다): {venv}")
    env = _clean_env({})
    t = time.monotonic()
    r = subprocess.run(["python3.12", "-m", "venv", str(venv)], capture_output=True, text=True, env=env)
    res["gates"]["venv_create"] = {"result": "PASS" if r.returncode == 0 else "FAIL", "rc": r.returncode, "stderr": _tail(r.stderr)}
    if r.returncode != 0:
        return res
    py = venv / "bin" / "python"
    rb = subprocess.run([str(py), "-m", "pip", "list", "--format", "json"], capture_output=True, text=True, env=env)
    try:
        baseline = sorted(canon(x["name"]) for x in json.loads(rb.stdout or "[]"))
    except ValueError:
        baseline = []
    t = time.monotonic()
    cmd = [str(py), "-m", "pip", "install", "--no-index"] + (["--no-deps"] if parity else []) + [
        "--find-links", str(whdir / "wheelhouse"), "-r", str(whdir / "requirements-closure.txt")]
    r = subprocess.run(cmd, capture_output=True, text=True, env=env)
    res["gates"]["pip_install"] = {"result": "PASS" if r.returncode == 0 else "FAIL", "rc": r.returncode, "argv": cmd[1:],
                                   "seconds_monotonic": round(time.monotonic() - t, 1),
                                   "stdout_tail": _tail(r.stdout, 1500), "stderr_tail": _tail(r.stderr)}
    if r.returncode != 0 and diagnostic and not parity:
        # 진단 전용: 리졸버가 이미지 자체의 선언 불일치로 막히면(이미지는 --no-deps/constraint 로 조립됐다) 정확 핀 집합을
        #   --no-deps 로 깔아 뒤 게이트를 관측한다. 이 경로는 판정을 PASS 로 만들지 못한다(pip_install 게이트는 FAIL 로 남는다).
        t = time.monotonic()
        cmd2 = cmd[:4] + ["--no-deps"] + cmd[4:]
        r2 = subprocess.run(cmd2, capture_output=True, text=True, env=env)
        res["gates"]["pip_install_no_deps_diagnostic"] = {
            "result": "PASS" if r2.returncode == 0 else "FAIL", "rc": r2.returncode, "argv": cmd2[1:],
            "seconds_monotonic": round(time.monotonic() - t, 1), "stderr_tail": _tail(r2.stderr),
            "note": "진단 전용 — 판정에 산입하지 않는다"}
    r = subprocess.run([str(py), "-m", "pip", "check"], capture_output=True, text=True, env=env)
    res["gates"]["pip_check"] = {"result": "PASS" if r.returncode == 0 else "FAIL", "rc": r.returncode,
                                 "output": _tail(r.stdout + r.stderr, 6000)}
    # 설치 집합 = closure 핀 집합인가(같으면 pip check 의 불일치는 이미지 메타데이터에서 물려받은 것이다)
    r = subprocess.run([str(py), "-m", "pip", "list", "--format", "json"], capture_output=True, text=True, env=env)
    try:
        inst = {canon(x["name"]): x["version"] for x in json.loads(r.stdout or "[]")}
    except ValueError:
        inst = {}
    pins = {}
    for ln in (whdir / "requirements-closure.txt").read_text().splitlines():
        if ln and not ln.startswith("#") and "==" in ln:
            n, vv = ln.split("==", 1)
            pins[canon(n)] = vv
    diff_missing = sorted(k for k in pins if inst.get(k) != pins[k])
    diff_extra = sorted(k for k in inst if k not in pins)
    res["gates"]["installed_set"] = {"result": "PASS" if not diff_missing else "FAIL", "pins": len(pins), "installed": len(inst),
                                     "missing_or_other_version": diff_missing[:50], "extra_in_venv": diff_extra,
                                     "note": "extra = venv 부트스트랩(pip 등)"}
    res["gates"]["pip_check"]["inherited_from_image"] = bool(pins) and not diff_missing
    if parity:
        # 동등성: 여분은 설치 **전** venv 에 이미 있던 부트스트랩(ensurepip 관측)만 허용
        boot = set(baseline)
        bad_extra = [k for k in diff_extra if k not in boot]
        ig = res["gates"]["installed_set"]
        ig["result"] = "PASS" if (pins and not diff_missing and not bad_extra) else "FAIL"
        ig["unexpected_extra"] = bad_extra
        ig["allowed_bootstrap_extra"] = sorted(boot)
        pc = res["gates"]["pip_check"]
        lines = [ln.strip() for ln in pc["output"].splitlines() if ln.strip() and ln.strip() != "No broken requirements found."]
        new = [ln for ln in lines if ln not in declared]
        pc.update(result="PASS" if not new else "FAIL", observed_conflicts=lines, new_conflicts=new,
                  declared_not_observed=[d for d in declared if d not in lines],
                  rule="관측 충돌 ⊆ image_inherent_conflicts(선언 밖 충돌 = FAIL)")
    r = subprocess.run([str(py), "-c", "import sysconfig;print(sysconfig.get_paths()['purelib'])"], capture_output=True, text=True, env=env)
    vsite = r.stdout.strip()
    rt = closure["runtime_lib"]

    def render(with_compat: bool) -> list[str]:
        out = []
        for ent in rt["ld_library_path_template"]:
            p = ent.get("path")
            if not p or (ent.get("compat") and not with_compat):
                continue
            p = p.replace("{VENV_SITE}", vsite).replace("{OUT}", str(whdir))
            if p not in out:
                out.append(p)
        return out
    sofiles = sorted(str(p) for p in Path(vsite, "vllm").rglob("*.so")) if vsite else []
    cands = []
    for label, wc in (("compat-first(이미지 env 순서)", True), ("host-driver(compat 제외)", False)):
        if wc and not rt["cuda_tree"].get("compat_dir"):
            continue
        ld = ":".join(render(wc))
        extra = {"LD_LIBRARY_PATH": ld}
        for k, v in rt.get("env_template", {}).items():
            extra[k] = v.replace("{OUT}", str(whdir))
        extra["PATH"] = ":".join([p.replace("{OUT}", str(whdir)) for p in rt.get("path_prepend_template", [])] + [str(venv / "bin"), env["PATH"]])
        e2 = _clean_env(extra)
        ldd = []
        for so in sofiles:
            lr = subprocess.run(["ldd", so], capture_output=True, text=True, env=e2)
            nf = sorted({ln.split("=>")[0].strip() for ln in lr.stdout.splitlines() if "not found" in ln})
            ldd.append({"file": os.path.relpath(so, vsite), "rc": lr.returncode, "not_found": nf})
        ldd_ok = bool(sofiles) and all(x["rc"] == 0 and not x["not_found"] for x in ldd)
        with tempfile.TemporaryDirectory() as cwd:
            ir = subprocess.run([str(py), "-c", IMPORT_PROBE], capture_output=True, text=True, env=e2, cwd=cwd, timeout=900)
        probe = None
        try:
            probe = json.loads(ir.stdout.strip().splitlines()[-1]) if ir.stdout.strip() else None
        except ValueError:
            probe = None
        in_venv = bool(probe) and str(probe.get("vllm_file", "")).startswith(str(venv)) and str(probe.get("torch_file", "")).startswith(str(venv))
        imp_ok = ir.returncode == 0 and bool(probe and probe.get("cuda_available")) and in_venv
        cands.append({"label": label, "with_compat": wc, "ld_library_path": ld, "env": extra,
                      "ldd": {"result": "PASS" if ldd_ok else "FAIL", "files": len(ldd),
                              "not_found": {x["file"]: x["not_found"] for x in ldd if x["not_found"] or x["rc"]}},
                      "import_cuda": {"result": "PASS" if imp_ok else "FAIL", "rc": ir.returncode, "probe": probe,
                                      "imported_from_venv": in_venv, "stderr_tail": _tail(ir.stderr, 2500)}})
    sel = next((c for c in cands if c["ldd"]["result"] == "PASS" and c["import_cuda"]["result"] == "PASS"), None)
    res["gates"]["runtime_candidates"] = cands
    res["gates"]["ldd"] = {"result": (sel or (cands[0] if cands else {"ldd": {"result": "FAIL"}}))["ldd"]["result"],
                           "for": (sel or (cands[0] if cands else {})).get("label")}
    res["gates"]["import_cuda"] = {"result": "PASS" if sel else "FAIL", "for": sel["label"] if sel else None}
    res["selected"] = sel["label"] if sel else None
    res["ld_library_path"] = sel["ld_library_path"] if sel else None
    res["env"] = sel["env"] if sel else None
    base_gates = ("venv_create", "pip_install", "installed_set", "pip_check") if parity else ("venv_create", "pip_install", "pip_check")
    base_ok = all(res["gates"][g]["result"] == "PASS" for g in base_gates)
    res["verdict_gates"] = list(base_gates) + ["ldd", "import_cuda"]
    res["verdict"] = "PASS" if (base_ok and sel and closure.get("status") == "PASS") else "FAIL"

    def du(p: Path) -> int:
        tot = 0
        for dp, _, fns in os.walk(p):
            for fn in fns:
                try:
                    tot += os.lstat(os.path.join(dp, fn)).st_blocks * 512
                except OSError:
                    pass
        return tot
    res["sizes"] = {"venv_bytes": du(venv), "wheelhouse_dir_bytes_total": du(whdir)}
    return res


# ─────────────────────────────── self-test ───────────────────────────────

def _rec_line(rel: str, data: Optional[bytes]) -> list[str]:
    if data is None:
        return [rel, "", ""]
    return [rel, "sha256=" + _b64(hashlib.sha256(data).digest()), str(len(data))]


def _fake_dist(site: Path, name: str, ver: str, files: dict, *, extra_rows: Optional[list] = None, ep: str = "",
               direct: Optional[dict] = None, tamper: Optional[dict] = None, wheel_tag: Optional[str] = "py3-none-any",
               requires: str = "") -> None:
    di = f"{name}-{ver}.dist-info"
    (site / di).mkdir(parents=True)
    meta = (f"Metadata-Version: 2.1\nName: {name}\nVersion: {ver}\n" + (f"Requires-Dist: {requires}\n" if requires else "")).encode()
    files = dict(files)
    files[f"{di}/METADATA"] = meta
    files[f"{di}/INSTALLER"] = b"pip\n"
    if wheel_tag:
        files[f"{di}/WHEEL"] = f"Wheel-Version: 1.0\nGenerator: t\nRoot-Is-Purelib: true\nTag: {wheel_tag}\n".encode()
    if ep:
        files[f"{di}/entry_points.txt"] = ep.encode()
    if direct is not None:
        files[f"{di}/direct_url.json"] = json.dumps(direct).encode()
    rows = []
    for rel, data in files.items():
        p = Path(os.path.normpath(site / rel))
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes((tamper or {}).get(rel, data))
        if rel.startswith("../../../bin/"):
            p.chmod(0o755)
        rows.append(_rec_line(rel, data))
    rows += extra_rows or []
    rows.append([f"{di}/RECORD", "", ""])
    buf = io.StringIO()
    csv.writer(buf, lineterminator="\n").writerows(rows)
    (site / di / "RECORD").write_text(buf.getvalue())


def _make_fake_root(root: Path, with_bad: bool) -> dict:
    site = root / SITE_DEFAULT.lstrip("/")
    site.mkdir(parents=True)
    _fake_dist(site, "alpha", "1.0", {
        "alpha/__init__.py": b"VALUE = 42\n",
        "../../../bin/alpha": b"#!/usr/bin/python3\nimport alpha\n",
        "../../../bin/alpha-tool": b"#!/usr/bin/python3\nprint('tool')\n",
        "../../../share/alpha/data.txt": b"data\n",
    }, extra_rows=[["alpha/__pycache__/__init__.cpython-312.pyc", "", ""]],
        ep="[console_scripts]\nalpha = alpha:main\n")
    if with_bad:
        # beta: RECORD 불일치 + 이미지 고유 의존성 충돌(선언은 zeta>=9 · 설치본 zeta 0.5)
        _fake_dist(site, "beta", "2.0", {"beta/__init__.py": b"ORIG = 1\n"}, tamper={"beta/__init__.py": b"PATCHED = 1\n"},
                   requires="zeta>=9")
    # 공유 경로: delta 와 epsilon 이 ns/__init__.py 를 둘 다 적고, 현재 바이트는 epsilon 것
    _fake_dist(site, "delta", "1.0", {"ns/__init__.py": b"", "ns/d.py": b"D = 1\n"},
               tamper={"ns/__init__.py": b"# epsilon\n"})
    _fake_dist(site, "epsilon", "1.0", {"ns/__init__.py": b"# epsilon\n", "ns/e.py": b"E = 1\n"})
    # editable gamma
    src_pkg = root / "workspace/gamma-src/gamma"
    (src_pkg / "__pycache__").mkdir(parents=True)
    (src_pkg / "__init__.py").write_bytes(b"from gamma.sub import X\n")
    (src_pkg / "sub.py").write_bytes(b"X = 'editable-ok'\n")
    (src_pkg / "_C.abi3.so").write_bytes(b"not-really-elf")
    (src_pkg / "__pycache__/sub.cpython-312.pyc").write_bytes(b"junk")
    finder = "MAPPING: dict[str, str] = {'gamma': '/workspace/gamma-src/gamma'}\nNAMESPACES = {}\n"
    _fake_dist(site, "gamma", "3.0.dev0+g1", {
        "__editable__.gamma-3.0.dev0+g1.pth": b"import __editable___gamma_3_0_dev0_g1_finder\n",
        "__editable___gamma_3_0_dev0_g1_finder.py": finder.encode(),
        "../../../bin/gamma": b"#!/usr/bin/python3\nimport gamma\n",
    }, ep="[console_scripts]\ngamma = gamma:main\n",
        direct={"dir_info": {"editable": True}, "url": "file:///workspace/gamma-src"}, wheel_tag="cp312-cp312-linux_aarch64")
    # Debian 자리: RECORD 없는 zeta(dpkg 매니페스트) + 앞 자리에 가려진 alpha
    deb = root / "usr/lib/python3/dist-packages"
    (deb / "zeta-0.5.dist-info").mkdir(parents=True)
    (deb / "zeta").mkdir()
    zf = {"zeta/__init__.py": b"Z = 5\n", "zeta-0.5.dist-info/METADATA": b"Metadata-Version: 2.1\nName: zeta\nVersion: 0.5\n",
          "zeta-0.5.dist-info/WHEEL": b"Wheel-Version: 1.0\nRoot-Is-Purelib: true\nTag: py3-none-any\n"}
    for rel, data in zf.items():
        (deb / rel).write_bytes(data)
    (deb / "alpha-0.1.dist-info").mkdir()
    (deb / "alpha-0.1.dist-info/METADATA").write_bytes(b"Metadata-Version: 2.1\nName: alpha\nVersion: 0.1\n")
    info = root / "var/lib/dpkg/info"
    info.mkdir(parents=True)
    dsite = "/usr/lib/python3/dist-packages"
    (info / "python3-zeta.list").write_text("\n".join([dsite, f"{dsite}/zeta-0.5.dist-info"] + [f"{dsite}/{r}" for r in zf]) + "\n")
    (info / "python3-zeta.md5sums").write_text("".join(f"{hashlib.md5(v).hexdigest()}  {dsite.lstrip('/')}/{r}\n" for r, v in zf.items()))
    cuda = root / "usr/local/cuda-13.3"
    (cuda / "compat/lib.real").mkdir(parents=True)
    (cuda / "compat/lib.real/libcuda.so.1").write_bytes(b"fake")
    (cuda / "bin").mkdir()
    os.symlink("/usr/local/cuda-13.3", root / "usr/local/cuda")
    (root / "etc").mkdir()
    (root / "etc/ld.so.conf").write_text("/usr/local/cuda/lib64\n")
    return {"tag": "fake:selftest", "id": "sha256:" + "0" * 64, "repo_digests": [], "architecture": "arm64", "os": "linux",
            "env": ["LD_LIBRARY_PATH=/usr/local/lib/python3.12/dist-packages/torch/lib:/usr/local/cuda/compat/lib:/usr/local/nvidia/lib",
                    "TRITON_PTXAS_PATH=/usr/local/cuda/bin/ptxas", "CUDA_HOME=/usr/local/cuda"]}


def self_test() -> int:
    checks: list[tuple[str, bool, str]] = []

    def ck(name: str, cond: bool, detail: str = "") -> None:
        checks.append((name, bool(cond), detail))
    with tempfile.TemporaryDirectory(prefix="nwh-selftest-") as td:
        tdp = Path(td)
        # (1) 음성대조: RECORD 불일치 → FAIL
        r1 = tdp / "root_bad"
        m1 = _make_fake_root(r1, with_bad=True)
        o1 = tdp / "out_bad"
        c1 = build(DirSource(r1, m1), o1, SITES_DEFAULT, CUDA_LINK_DEFAULT, "2026-01-01T00:00:00Z", {}, 4, staging=tdp / "st1")
        ck("mismatch→status FAIL", c1["status"] == "FAIL", c1["status"])
        ck("mismatch 목록에 beta/__init__.py", any(f.get("dist") == "beta" and f.get("path") == "beta/__init__.py"
                                                  and f["kind"] == "record-hash-mismatch" for f in c1["failures"]))
        ck("다른 분포는 불일치 아님", all(f.get("dist") == "beta" for f in c1["failures"]), json.dumps(c1["failures"], ensure_ascii=False))
        # (1b) 사람 수용 → ACCEPTED 로 표시되고 PASS
        o1b = tdp / "out_acc"
        c1b = build(DirSource(r1, m1), o1b, SITES_DEFAULT, CUDA_LINK_DEFAULT, "2026-01-01T00:00:00Z",
                    _parse_accept(["beta:beta/__init__.py=selftest"]), 4, staging=tdp / "st1b")
        bb = next(d for d in c1b["distributions"] if d["name"] == "beta")
        ck("수용된 불일치는 ACCEPTED+provenance", c1b["status"] == "PASS" and bb["record_verify"]["result"] == "ACCEPTED"
           and bb["record_verify"]["accepted"][0]["provenance"].startswith("human"))
        # (1c) 추적 승인 파일: 적용·출처·stale 경고·digest 불일치 거부 → 이미지 동등성 verify
        def accept_doc(conflicts: list, image_id: str) -> Path:
            p = tdp / f"accept-{len(list(tdp.glob('accept-*')))}.json"
            p.write_text(json.dumps({"schema_version": 1, "approved_by": "selftest-human", "approved_utc": "2026-01-01T00:00:00Z",
                                     "image": {"tag": "fake:selftest", "id": image_id},
                                     "accepted_mismatches": [{"dist": "beta", "path": "beta/__init__.py", "reason": "t"},
                                                             {"dist": "beta", "path": "beta/never.py", "reason": "stale"}],
                                     "image_inherent_conflicts": [{"pip_check_line": x} for x in conflicts]}))
            return p
        conflict = "beta 2.0 has requirement zeta>=9, but you have zeta 0.5."
        af = accept_doc([conflict], m1["id"])
        acc, ameta, _ = load_accept_file(af)
        o1c = tdp / "out_accfile"
        c1c = build(DirSource(r1, m1), o1c, SITES_DEFAULT, CUDA_LINK_DEFAULT, "2026-01-01T00:00:00Z", acc, 4,
                    staging=tdp / "st1c", accept_meta=ameta)
        bc = next(d for d in c1c["distributions"] if d["name"] == "beta")
        ck("승인 파일 적용 → PASS · provenance accepted-mismatch(human: …)", c1c["status"] == "PASS"
           and bc["record_verify"]["accepted"][0]["provenance"] == "accepted-mismatch(human: selftest-human)")
        ck("일어나지 않은 승인 = stale-accept 경고", c1c["accept"]["stale"] == ["beta:beta/never.py"]
           and any(w["kind"] == "stale-accept" for w in c1c["warnings"]))
        try:
            build(DirSource(r1, m1), tdp / "out_wrongimg", SITES_DEFAULT, CUDA_LINK_DEFAULT, "2026-01-01T00:00:00Z", acc, 4,
                  staging=tdp / "st1d", accept_meta={**ameta, "image_id": "sha256:" + "1" * 64})
            ck("다른 이미지 digest 의 승인 파일 거부", False)
        except WheelhouseError:
            ck("다른 이미지 digest 의 승인 파일 거부", True)
        vp = verify(o1c, tdp / "venv_parity", "2026-01-01T00:00:00Z", False, af)
        g = vp["gates"]
        ck("parity: gate_definition 표기", vp["gate_definition"] == GATE_PARITY)
        ck("parity: --no-deps 설치 PASS", g["pip_install"]["result"] == "PASS" and "--no-deps" in g["pip_install"]["argv"],
           _tail(g["pip_install"].get("stderr_tail", ""), 400))
        ck("parity: 설치 집합 == 핀", g["installed_set"]["result"] == "PASS", json.dumps(g["installed_set"], ensure_ascii=False))
        ck("parity: 선언된 충돌은 PASS", g["pip_check"]["result"] == "PASS" and g["pip_check"]["observed_conflicts"] == [conflict],
           json.dumps(g["pip_check"], ensure_ascii=False))
        vn = verify(o1c, tdp / "venv_parity_neg", "2026-01-01T00:00:00Z", False, accept_doc([], m1["id"]))
        ck("parity: 선언 밖 충돌 → FAIL", vn["gates"]["pip_check"]["result"] == "FAIL"
           and vn["gates"]["pip_check"]["new_conflicts"] == [conflict] and vn["verdict"] == "FAIL")
        # (2) 정상 트리
        r2 = tdp / "root_ok"
        m2 = _make_fake_root(r2, with_bad=False)
        o2 = tdp / "out_ok"
        c2 = build(DirSource(r2, m2), o2, SITES_DEFAULT, CUDA_LINK_DEFAULT, "2026-01-01T00:00:00Z", {}, 4, staging=tdp / "st2")
        ck("정상 트리 PASS", c2["status"] == "PASS", json.dumps(c2["failures"], ensure_ascii=False))
        byn = {d["name"]: d for d in c2["distributions"]}
        ck("출처 필드 전 항목", all(d["source"] and d["wheel"] and d["record_verify"]["result"] for d in c2["distributions"]))
        ck("alpha = repacked(image@…)", byn["alpha"]["source"] == f"repacked(image@{m2['id']})")
        ck("gamma = editable-repacked(image@…)", byn["gamma"]["source"] == f"editable-repacked(image@{m2['id']})")
        ck("closure 최상위 필드", all(k in c2 for k in ("image", "generated_utc", "provenance", "runtime_lib", "excluded")))
        ck("compat 표식", c2["runtime_lib"]["cuda_tree"]["compat_dir"] == "{OUT}/runtime-lib/cuda/compat/lib.real")
        ck("env 템플릿 재사상", c2["runtime_lib"]["env_template"].get("TRITON_PTXAS_PATH") == "{OUT}/runtime-lib/cuda/bin/ptxas",
           json.dumps(c2["runtime_lib"]["env_template"]))
        za = zipfile.ZipFile(o2 / "wheelhouse" / byn["alpha"]["wheel"])
        names = set(za.namelist())
        ck("alpha: 엔트리포인트 스크립트는 싣지 않음(pip 재생성)", not any(n.endswith("scripts/alpha") for n in names)
           and byn["alpha"]["scripts"]["regenerated_entry_points"] == ["alpha"])
        ck("alpha: 비-엔트리 스크립트 #!python", za.read("alpha-1.0.data/scripts/alpha-tool").startswith(b"#!python\n"))
        ck("alpha: data 파일", "alpha-1.0.data/data/share/alpha/data.txt" in names)
        ck("alpha: pyc·INSTALLER 없음", not any(n.endswith(".pyc") or n.endswith("/INSTALLER") for n in names))
        zg = zipfile.ZipFile(o2 / "wheelhouse" / byn["gamma"]["wheel"])
        gn = set(zg.namelist())
        ck("gamma: 소스 트리 + .so", {"gamma/__init__.py", "gamma/sub.py", "gamma/_C.abi3.so"} <= gn, str(sorted(gn)))
        ck("gamma: editable shim·direct_url 없음", not any("__editable__" in n or n.endswith("direct_url.json") for n in gn))
        ck("gamma: 플랫폼 태그", byn["gamma"]["wheel"].endswith("-cp312-cp312-linux_aarch64.whl"), byn["gamma"]["wheel"])
        zd = zipfile.ZipFile(o2 / "wheelhouse" / byn["delta"]["wheel"])
        ck("공유 경로: delta 는 ns/__init__.py 생략(소유=epsilon)", "ns/__init__.py" not in zd.namelist()
           and byn["delta"]["record_verify"]["shared_path_omitted"][0]["owner"].endswith("/epsilon-1.0.dist-info"))
        ck("dpkg 매니페스트로 RECORD 대체(zeta)", byn.get("zeta", {}).get("record_source") == "dpkg:python3-zeta.list+md5sums"
           and byn["zeta"]["record_verify"]["result"] == "PASS" and byn["zeta"]["record_verify"]["verified"] == 3,
           json.dumps(byn.get("zeta", {}).get("record_verify"), ensure_ascii=False))
        ck("뒤 자리의 같은 이름은 shadowed 로 제외", any(e["name"] == "alpha" and e["reason"].startswith("shadowed") for e in c2["excluded"]))
        req = (o2 / "requirements-closure.txt").read_text()
        ck("requirements 정확 핀", "gamma==3.0.dev0+g1" in req and "alpha==1.0" in req)
        # (3) 실제 pip 로 오프라인 설치(임시 venv)
        venv = tdp / "venv"
        env = _clean_env({})
        rv = subprocess.run(["python3.12", "-m", "venv", str(venv)], capture_output=True, text=True, env=env)
        if rv.returncode == 0:
            ri = subprocess.run([str(venv / "bin/python"), "-m", "pip", "install", "--no-index", "--find-links", str(o2 / "wheelhouse"),
                                 "-r", str(o2 / "requirements-closure.txt")], capture_output=True, text=True, env=env)
            ck("pip install --no-index rc 0", ri.returncode == 0, _tail(ri.stdout + ri.stderr, 800))
            ck("venv/bin/gamma 재생성", (venv / "bin/gamma").exists())
            rp = subprocess.run([str(venv / "bin/python"), "-c", "import gamma,alpha,ns.d,ns.e,zeta;print(gamma.X)"],
                                capture_output=True, text=True, env=env, cwd=td)
            ck("설치본 import", rp.stdout.strip() == "editable-ok", _tail(rp.stderr, 400))
        else:
            ck("venv 생성", False, rv.stderr[-400:])
    ok = all(c[1] for c in checks)
    print(json.dumps({"self_test": "PASS" if ok else "FAIL", "checks": [{"name": n, "ok": o, **({"detail": d} if d and not o else {})}
                                                                        for n, o, d in checks]}, ensure_ascii=False, indent=1))
    return EXIT_OK if ok else EXIT_FAIL


# ─────────────────────────────── CLI ───────────────────────────────

def _parse_accept(items: Iterable[str]) -> dict:
    out = {}
    for it in items:
        m = re.match(r"^([^:]+):([^=]+)=(.+)$", it)
        if not m:
            raise WheelhouseError(f"--accept-mismatch 형식은 DIST:PATH=REASON: {it!r}")
        out[f"{canon(m.group(1))}:{m.group(2)}"] = {"reason": m.group(3), "provenance": "human:--accept-mismatch"}
    return out


def load_accept_file(path: Path) -> tuple[dict, dict, dict]:
    """추적 승인 파일 → (불일치 승인 dict, 메타, 원문). 형식 위반은 거부(fail-closed)."""
    raw = path.read_bytes()
    try:
        doc = json.loads(raw)
    except ValueError as e:
        raise WheelhouseError(f"승인 파일 JSON 오류: {path}: {e}")
    need = ("schema_version", "approved_by", "approved_utc", "image", "accepted_mismatches", "image_inherent_conflicts")
    miss = [k for k in need if k not in doc]
    img = doc.get("image") or {}
    if miss or not str(img.get("id", "")).startswith("sha256:") or not UTC_RE.match(str(doc.get("approved_utc", ""))) \
            or not str(doc.get("approved_by", "")).strip():
        raise WheelhouseError(f"승인 파일 형식 위반(누락 {miss} · image.id sha256 · approved_utc · approved_by): {path}")
    who = str(doc["approved_by"]).strip()
    acc = {}
    for e in doc["accepted_mismatches"]:
        if not (e.get("dist") and e.get("path") and e.get("reason")):
            raise WheelhouseError(f"accepted_mismatches 항목에 dist/path/reason 필수: {e}")
        acc[f"{canon(e['dist'])}:{e['path']}"] = {"reason": e["reason"], "provenance": f"accepted-mismatch(human: {who})"}
    for e in doc["image_inherent_conflicts"]:
        if not str(e.get("pip_check_line", "")).strip():
            raise WheelhouseError(f"image_inherent_conflicts 항목에 pip_check_line 필수: {e}")
    meta = {"source": "accept-file", "sha256": hashlib.sha256(raw).hexdigest(), "approved_by": who,
            "approved_utc": doc["approved_utc"], "image_id": img["id"], "image_tag": img.get("tag")}
    return acc, meta, doc


def main(argv: Optional[list[str]] = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    b = sub.add_parser("build")
    b.add_argument("--image", required=True)
    b.add_argument("--out", required=True)
    b.add_argument("--generated-utc", required=True)
    b.add_argument("--site-packages", action="append", default=None,
                   help=f"이미지 sys.path 순서의 site 디렉터리(반복 · 기본 {SITES_DEFAULT})")
    b.add_argument("--cuda-link", default=CUDA_LINK_DEFAULT)
    b.add_argument("--accept-mismatch", action="append", default=[],
                   help="DIST:PATH=REASON — 사람이 검토해 수용한 RECORD 불일치(closure.json 에 human 출처로 기재)")
    b.add_argument("--accept-file", default=None, help="추적 승인 선언(JSON) — 불일치 승인·이미지 digest 대조")
    b.add_argument("--jobs", type=int, default=min(16, os.cpu_count() or 4))
    b.add_argument("--keep-staging", action="store_true")
    v = sub.add_parser("verify")
    v.add_argument("--wheelhouse-dir", required=True)
    v.add_argument("--venv", required=True)
    v.add_argument("--generated-utc", required=True)
    v.add_argument("--accept-file", default=None, help="이미지 동등성 게이트(--no-deps 정확 핀 · pip check ⊆ 선언된 이미지 고유 충돌)")
    v.add_argument("--diagnostic", action="store_true", help="closure FAIL 이어도 게이트를 돌려 본다(판정은 FAIL 로 남는다)")
    v.add_argument("--out-json", default=None)
    a = ap.parse_args(argv)
    if a.self_test:
        return self_test()
    try:
        if a.cmd == "build":
            out = Path(a.out)
            if not out.is_absolute() or not UTC_RE.match(a.generated_utc):
                print("--out 은 절대경로, --generated-utc 는 YYYY-MM-DDTHH:MM:SSZ", file=sys.stderr)
                return EXIT_USAGE
            if out.exists() and any(out.iterdir()):
                print(f"--out 이 비어 있지 않다(덮어쓰지 않는다): {out}", file=sys.stderr)
                return EXIT_USAGE
            out.mkdir(parents=True, exist_ok=True)
            staging = out / ".staging"
            src = DockerSource(a.image, staging)
            try:
                src.start()
                acc = _parse_accept(a.accept_mismatch)
                ameta = None
                if a.accept_file:
                    fa, ameta, _ = load_accept_file(Path(a.accept_file))
                    acc = {**fa, **acc}
                c = build(src, out, a.site_packages or SITES_DEFAULT, a.cuda_link, a.generated_utc, acc,
                          a.jobs, staging=staging, accept_meta=ameta)
            finally:
                src.close()
                if not a.keep_staging:
                    shutil.rmtree(staging, ignore_errors=True)
            summary = {k: c[k] for k in ("status", "counts", "sizes", "elapsed_s_monotonic")}
            summary["failures"] = c["failures"]
            summary["unresolved"] = c["runtime_lib"]["unresolved"]
            print(json.dumps(summary, ensure_ascii=False, indent=1))
            return EXIT_OK if c["status"] == "PASS" else EXIT_FAIL
        if a.cmd == "verify":
            wd, ve = Path(a.wheelhouse_dir), Path(a.venv)
            if not wd.is_absolute() or not ve.is_absolute() or not UTC_RE.match(a.generated_utc):
                print("경로는 절대경로, --generated-utc 는 YYYY-MM-DDTHH:MM:SSZ", file=sys.stderr)
                return EXIT_USAGE
            res = verify(wd, ve, a.generated_utc, a.diagnostic, Path(a.accept_file) if a.accept_file else None)
            txt = json.dumps(res, ensure_ascii=False, indent=1)
            Path(a.out_json or (wd / "verify.json")).write_text(txt + "\n")
            print(txt)
            return EXIT_OK if res["verdict"] == "PASS" else EXIT_FAIL
    except WheelhouseError as e:
        print(f"native_wheelhouse: {e}", file=sys.stderr)
        return EXIT_USAGE
    ap.print_help()
    return EXIT_USAGE


if __name__ == "__main__":
    sys.exit(main())
