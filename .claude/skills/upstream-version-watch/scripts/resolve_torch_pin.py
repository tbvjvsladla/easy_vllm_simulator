#!/usr/bin/env python3
"""① 대상 vLLM 버전 → torch 핀 (결정론적)

vLLM 의 pyproject.toml `[build-system].requires` 에서 torch 버전을 추출한다.
출력(JSON): {vllm_version, torch_spec, torch_pin, torch_prefix, source_url}
  + ==/=== 정확 핀 부재(범위 스펙, 예 torch>=2.12,<2.13) 시 fail-loud 필드 추가:
    {torch_specifier, range_bounds:{lower,upper}, warning} — 침묵 null 전파 금지.

확률론적 추론 금지 — 버전 문자열은 업스트림 원본에서 직접 읽는다(하네스 엔지니어링).
사용: python3 resolve_torch_pin.py 0.21.0
"""
import sys, json, argparse, urllib.request, tomllib
from packaging.requirements import Requirement
from packaging.version import Version, InvalidVersion

# 태그 우선, 릴리스 브랜치 폴백
REFS = ["v{v}", "releases/v{v}"]


def fetch(url: str) -> str:
    with urllib.request.urlopen(url, timeout=20) as r:
        return r.read().decode("utf-8")


def norm_prefix(v: str):
    """2.11.0a0+a6c236b → 2.11.0 (packaging release 튜플)."""
    try:
        return ".".join(map(str, Version(v).release))
    except InvalidVersion:
        return v


def pick_bound(versions, newest: bool):
    """범위 스펙 경계 선택 — 하한은 최대(newest), 상한은 최소. 파싱 불가 시 첫 값(결정론)."""
    if not versions:
        return None
    try:
        return sorted(versions, key=Version)[-1 if newest else 0]
    except InvalidVersion:
        return versions[0]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("vllm_version", help="예: 0.21.0")
    a = ap.parse_args()

    data = None
    last_err = None
    used_url = None
    for ref in REFS:
        url = f"https://raw.githubusercontent.com/vllm-project/vllm/{ref.format(v=a.vllm_version)}/pyproject.toml"
        try:
            data = tomllib.loads(fetch(url))
            used_url = url
            break
        except Exception as e:
            last_err = {"url": url, "error": str(e)}
    if data is None:
        print(json.dumps({"error": "pyproject.toml fetch/parse 실패", "detail": last_err},
                         ensure_ascii=False), file=sys.stderr)
        sys.exit(2)

    requires = data.get("build-system", {}).get("requires", [])
    torch_req = None
    for r in requires:
        try:
            req = Requirement(r)
        except Exception:
            continue
        if req.name.lower() == "torch":
            torch_req = req
            break
    if torch_req is None:
        print(json.dumps({"error": "build-system.requires 에 torch 없음", "requires": requires},
                         ensure_ascii=False), file=sys.stderr)
        sys.exit(3)

    pin = None
    for s in torch_req.specifier:
        if s.operator in ("==", "==="):
            pin = s.version
    out = {
        "vllm_version": a.vllm_version,
        "torch_spec": str(torch_req.specifier),
        "torch_pin": pin,
        "torch_prefix": norm_prefix(pin) if pin else None,
        "source_url": used_url,
    }
    if pin is None:
        # 범위 스펙 (예 torch>=2.12,<2.13) — 정확 핀 부재를 침묵 null 로 전파하지 않는다 (fail-loud)
        lowers = [s.version for s in torch_req.specifier if s.operator in (">=", ">", "~=")]
        uppers = [s.version for s in torch_req.specifier if s.operator in ("<=", "<")]
        out["torch_specifier"] = str(torch_req.specifier)
        out["range_bounds"] = {"lower": pick_bound(lowers, newest=True),
                               "upper": pick_bound(uppers, newest=False)}
        out["warning"] = ("범위 스펙 감지 — 정확 핀 부재. 하한/상한을 NGC 매칭 후보로 제시"
                          "(범위 교차), null 전파 아님")
    print(json.dumps(out, ensure_ascii=False, indent=2))


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
        print("[resolve_torch_pin] ERROR explicit upstream runtime root is empty", file=_native_sys.stderr)
        raise SystemExit(2)
    else:
        _native_runtime_root = _NativePath(_native_root_value)
    _native_binary = _native_runtime_root / "target/release/resolve_torch_pin"
    if not _native_binary.is_file() or not _native_os.access(_native_binary, _native_os.X_OK):
        print("[resolve_torch_pin] ERROR normal Rust ELF missing/non-executable: " + str(_native_binary), file=_native_sys.stderr)
        raise SystemExit(2)
    with _native_binary.open("rb") as _native_file:
        _native_magic = _native_file.read(4)
    if _native_magic != b"\x7fELF":
        print("[resolve_torch_pin] ERROR normal Rust ELF invalid: " + str(_native_binary), file=_native_sys.stderr)
        raise SystemExit(2)
    _native_os.execv(str(_native_binary), [str(_native_script), *_native_sys.argv[1:]])
