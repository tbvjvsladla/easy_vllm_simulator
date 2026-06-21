#!/usr/bin/env python3
"""② torch 핀 → 매칭 NGC PyTorch 베이스 태그 (결정론적, docker buildx)

NGC 컨테이너 config env 의 PYTORCH_BUILD_VERSION 접두어가 torch 핀 접두어와
일치하는 '최신' 태그를 찾는다. PoC 근거: docs/testlog/testlog_260607_*.
skopeo 불필요 — `docker buildx imagetools inspect` (익명, 레이어 pull 없음).

출력(JSON): {torch_prefix, matched_tag, build_version, cuda_version, image, probed}
사용:
  python3 resolve_ngc_tag.py 2.11.0 --start 26.05 --months 18 --arch arm64
  python3 resolve_ngc_tag.py 2.11.0 --candidates 26.03-py3,26.01-py3
"""
import sys, json, argparse, subprocess
from packaging.version import Version, InvalidVersion

IMAGE = "nvcr.io/nvidia/pytorch"


def norm(v: str):
    try:
        return ".".join(map(str, Version(v).release))
    except InvalidVersion:
        return v


def inspect_env(tag: str, arch: str):
    """태그의 config env 를 dict 로. 실패(미존재/네트워크)면 None."""
    ref = f"{IMAGE}:{tag}"
    try:
        out = subprocess.run(
            ["docker", "buildx", "imagetools", "inspect", ref, "--format", "{{json .Image}}"],
            capture_output=True, text=True, timeout=90)
    except Exception:
        return None
    if out.returncode != 0:
        return None
    try:
        img = json.loads(out.stdout)
    except Exception:
        return None
    cfg = img.get(f"linux/{arch}") or img  # multi-arch dict 또는 단일
    env = ((cfg or {}).get("config", {}) or {}).get("Env", []) or []
    d = {}
    for e in env:
        if "=" in e:
            k, v = e.split("=", 1)
            d[k] = v
    return d


def gen_candidates(start: str, months: int):
    """'YY.MM' → newest→oldest 월별 태그 목록."""
    yy, mm = (int(x) for x in start.split("."))
    out = []
    for _ in range(months):
        out.append(f"{yy:02d}.{mm:02d}-py3")
        mm -= 1
        if mm == 0:
            mm = 12
            yy -= 1
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("torch_pin", help="예: 2.11.0")
    ap.add_argument("--arch", default="arm64", help="arm64(DGX Spark) | amd64")
    ap.add_argument("--start", default=None, help="YY.MM (가장 최신 후보). newest→oldest 프로빙")
    ap.add_argument("--months", type=int, default=18, help="--start 부터 거슬러 프로빙할 개월수")
    ap.add_argument("--candidates", default=None, help="명시 태그 목록(쉼표, newest-first)")
    a = ap.parse_args()

    target = norm(a.torch_pin)
    if a.candidates:
        cands = [c.strip() for c in a.candidates.split(",") if c.strip()]
    elif a.start:
        cands = gen_candidates(a.start, a.months)
    else:
        print(json.dumps({"error": "--candidates 또는 --start 필요"}, ensure_ascii=False),
              file=sys.stderr)
        sys.exit(2)

    probed = []
    for tag in cands:
        env = inspect_env(tag, a.arch)
        if env is None:
            probed.append({"tag": tag, "status": "unavailable"})
            continue
        bv = env.get("PYTORCH_BUILD_VERSION") or env.get("PYTORCH_VERSION")
        bp = norm(bv) if bv else None
        probed.append({"tag": tag, "build_version": bv, "prefix": bp})
        if bp == target:  # newest→oldest 이므로 첫 매칭 = 최신 매칭
            print(json.dumps({
                "torch_pin": a.torch_pin, "torch_prefix": target,
                "matched_tag": tag, "build_version": bv,
                "cuda_version": env.get("CUDA_VERSION"),
                "image": f"{IMAGE}:{tag}", "arch": a.arch, "probed": probed,
            }, ensure_ascii=False, indent=2))
            return

    print(json.dumps({"error": "매칭 NGC 태그 없음", "torch_prefix": target, "probed": probed},
                     ensure_ascii=False, indent=2), file=sys.stderr)
    sys.exit(4)


if __name__ == "__main__":
    main()
