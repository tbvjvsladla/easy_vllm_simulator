#!/usr/bin/env python3
"""③ vLLM 버전(+CUDA/arch) → 검증된 pre-built wheel URL (결정론적)

GitHub Release 자산 목록을 실제 조회해 wheel URL을 '실재 확인 후' 구성한다.
404 함정 방지(devlog 260519 교훈): manylinux_2_34 vs 2_35, cu129 vs cu132 등은
추측하지 않고 실제 자산명에서 읽는다.

출력(JSON): {vllm_version, arch, cuda_version, manylinux, asset_name, wheel_url, all_arch_assets}
사용:
  python3 resolve_wheel.py 0.21.0 --arch aarch64           # CUDA 자동(자산 최고 cuXXX)
  python3 resolve_wheel.py 0.21.0 --arch aarch64 --cuda 129
"""
import sys, json, argparse, urllib.request, re

API = "https://api.github.com/repos/vllm-project/vllm/releases/tags/v{v}"


def fetch_json(url: str):
    req = urllib.request.Request(
        url, headers={"Accept": "application/vnd.github+json", "User-Agent": "upstream-version-watch"})
    with urllib.request.urlopen(req, timeout=20) as r:
        return json.load(r)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("vllm_version", help="예: 0.21.0")
    ap.add_argument("--arch", default="aarch64", help="uname -m 값 (aarch64 | x86_64)")
    ap.add_argument("--cuda", default=None, help="예 129. 미지정 시 자산에서 최고 cuXXX 자동")
    a = ap.parse_args()

    rel = fetch_json(API.format(v=a.vllm_version))
    assets = [x["name"] for x in rel.get("assets", [])]
    # vllm-<ver>+cu<NNN>-cp38-abi3-manylinux_X_YY_<arch>.whl
    pat = re.compile(
        rf"^vllm-{re.escape(a.vllm_version)}\+cu(\d+)-cp\d+-abi3-(manylinux_\d+_\d+)_{re.escape(a.arch)}\.whl$")
    cands = []
    for name in assets:
        m = pat.match(name)
        if m:
            cands.append({"name": name, "cuda": m.group(1), "manylinux": m.group(2)})
    if not cands:
        print(json.dumps({"error": "arch 매칭 cuXXX wheel 자산 없음", "arch": a.arch, "assets": assets},
                         ensure_ascii=False, indent=2), file=sys.stderr)
        sys.exit(4)

    if a.cuda:
        chosen = next((c for c in cands if c["cuda"] == str(a.cuda)), None)
        if chosen is None:
            print(json.dumps({"error": f"cu{a.cuda} 자산 없음", "available": cands},
                             ensure_ascii=False, indent=2), file=sys.stderr)
            sys.exit(5)
    else:
        chosen = sorted(cands, key=lambda c: int(c["cuda"]))[-1]  # 가장 높은 cuXXX

    url = (f"https://github.com/vllm-project/vllm/releases/download/"
           f"v{a.vllm_version}/{chosen['name']}")
    print(json.dumps({
        "vllm_version": a.vllm_version, "arch": a.arch,
        "cuda_version": chosen["cuda"], "manylinux": chosen["manylinux"],
        "asset_name": chosen["name"], "wheel_url": url,
        "all_arch_assets": cands,
    }, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
