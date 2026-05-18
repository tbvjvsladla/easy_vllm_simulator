#!/usr/bin/env python3
"""vLLM 0.21.0 requirements analyzer
- 현재 설치 버전 vs 요구 사양 vs PyPI 최신 호환 버전
"""
import re, json, subprocess, urllib.request
from importlib.metadata import version, PackageNotFoundError
from packaging.requirements import Requirement
from packaging.version import Version, InvalidVersion
from packaging.specifiers import SpecifierSet

VLLM_VERSION = "0.21.0"
URLS = [
    f"https://raw.githubusercontent.com/vllm-project/vllm/releases/v{VLLM_VERSION}/requirements/common.txt",
    f"https://raw.githubusercontent.com/vllm-project/vllm/releases/v{VLLM_VERSION}/requirements/cuda.txt",
]

def fetch(url):
    with urllib.request.urlopen(url, timeout=20) as r:
        return r.read().decode("utf-8")

def parse(line):
    line = line.split("#")[0].strip()
    if not line or line.startswith("-"):
        return None
    if ";" in line:
        line = line.split(";")[0].strip()
    m = re.match(r"^([A-Za-z0-9_\-\.]+)(\[[^\]]+\])?\s*(.*)$", line)
    if not m:
        return None
    return m.group(1).lower(), (m.group(3) or "").strip()

def installed(pkg):
    for alt in (pkg, pkg.replace("-", "_"), pkg.replace("_", "-")):
        try:
            return version(alt)
        except PackageNotFoundError:
            continue
    return None

def pypi_versions(pkg):
    try:
        with urllib.request.urlopen(f"https://pypi.org/pypi/{pkg}/json", timeout=15) as r:
            data = json.load(r)
        vs = []
        for v in data.get("releases", {}).keys():
            try:
                vs.append(Version(v))
            except InvalidVersion:
                pass
        return sorted(vs)
    except Exception:
        return []

def max_satisfying(pkg, spec):
    if not spec:
        vs = pypi_versions(pkg)
        return str(vs[-1]) if vs else "-"
    try:
        ss = SpecifierSet(spec)
    except Exception:
        return "-"
    vs = [v for v in pypi_versions(pkg) if v in ss and not v.is_prerelease]
    return str(vs[-1]) if vs else "-"

reqs = {}
for u in URLS:
    for line in fetch(u).splitlines():
        p = parse(line)
        if p:
            reqs[p[0]] = p[1]

rows = []
for name in sorted(reqs):
    spec = reqs[name]
    cur = installed(name)
    if cur is None:
        status = "MISSING"
    else:
        try:
            ok = (not spec) or (Version(cur) in SpecifierSet(spec))
            status = "OK" if ok else "MISMATCH"
        except Exception:
            status = "?"
    max_v = max_satisfying(name, spec)
    rows.append((name, cur or "-", spec or "(any)", max_v, status))

print(f"{'PACKAGE':<35} {'INSTALLED':<22} {'REQUIRED SPEC':<35} {'MAX OK':<15} {'STATUS'}")
print("-" * 120)
for r in rows:
    print(f"{r[0]:<35} {r[1]:<22} {r[2]:<35} {r[3]:<15} {r[4]}")

print()
print("=== SUMMARY ===")
for label in ("MISSING", "MISMATCH"):
    items = [r[0] for r in rows if r[4] == label]
    if items:
        print(f"  {label} ({len(items)}): {', '.join(items)}")
ok_cnt = sum(1 for r in rows if r[4] == "OK")
print(f"  OK: {ok_cnt} / {len(rows)}")