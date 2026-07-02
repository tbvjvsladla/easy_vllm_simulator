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
    # +cuXXX 변종:   vllm-<ver>+cu<NNN>-cp38-abi3-manylinux_X_YY_<arch>.whl
    # 무접미어 기본: vllm-<ver>-cp38-abi3-manylinux_X_YY_<arch>.whl  (릴리스 기본 CUDA; 파일명에 cuXXX 없음.
    #               예: 0.23.0 기본 = CUDA 13.0 — devlog 260622. 파일명만으론 CUDA 숫자 미확정 → cuda=null.)
    pat_cu = re.compile(
        rf"^vllm-{re.escape(a.vllm_version)}\+cu(\d+)-cp\d+-abi3-(manylinux_\d+_\d+)_{re.escape(a.arch)}\.whl$")
    pat_default = re.compile(
        rf"^vllm-{re.escape(a.vllm_version)}-cp\d+-abi3-(manylinux_\d+_\d+)_{re.escape(a.arch)}\.whl$")
    cands = []          # +cuXXX (CUDA 명시)
    default_cands = []  # 무접미어 (릴리스 기본 CUDA)
    for name in assets:
        m = pat_cu.match(name)
        if m:
            cands.append({"name": name, "cuda": m.group(1), "manylinux": m.group(2)})
            continue
        d = pat_default.match(name)
        if d:
            default_cands.append({"name": name, "cuda": None, "manylinux": d.group(1),
                                  "variant": "default(release-CUDA, 파일명에 cuXXX 없음)"})
    if not cands and not default_cands:
        print(json.dumps({"error": ("arch 매칭 wheel 자산 없음(+cu/무접미어 모두) — "
                                    "자산 명명 스킴 변경 가능성 — vLLM release 페이지를 사람이 재확인"),
                          "arch": a.arch, "assets": assets},
                         ensure_ascii=False, indent=2), file=sys.stderr)
        sys.exit(4)

    if a.cuda:
        chosen = next((c for c in cands if c["cuda"] == str(a.cuda)), None)
        if chosen is None:
            # 정확일치 부재 ≠ 설치 불가 — 같은 major 의 인접 minor 후보를 전방호환 힌트로 제시
            req = str(a.cuda)
            nearest = []
            if req.isdigit() and len(req) >= 2:
                req_major, req_minor = int(req[:-1]), int(req[-1])
                nearest = sorted(
                    (c for c in cands
                     if c["cuda"].isdigit() and len(c["cuda"]) >= 2
                     and int(c["cuda"][:-1]) == req_major),
                    key=lambda c: abs(int(c["cuda"][-1]) - req_minor))
            print(json.dumps({"error": f"cu{a.cuda} 정확일치 자산 없음", "available": cands,
                              "default_variants": default_cands,
                              "forward_compat_hint": {
                                  "nearest_cu_candidates": nearest,
                                  "note": ("CUDA minor 전방호환으로 동작하는 경우 많음 — "
                                           "사전 기각 금지, 설치 시도 후 스모크/classify_failure 가 중재"),
                              }},
                             ensure_ascii=False, indent=2), file=sys.stderr)
            sys.exit(5)
    elif cands:
        chosen = sorted(cands, key=lambda c: int(c["cuda"]))[-1]  # 가장 높은 cuXXX
    else:
        chosen = default_cands[0]  # +cu 변종 없음 → 릴리스 기본(무접미어) wheel

    url = (f"https://github.com/vllm-project/vllm/releases/download/"
           f"v{a.vllm_version}/{chosen['name']}")
    print(json.dumps({
        "vllm_version": a.vllm_version, "arch": a.arch,
        "cuda_version": chosen["cuda"], "manylinux": chosen["manylinux"],
        "asset_name": chosen["name"], "wheel_url": url,
        "all_arch_assets": cands, "default_variants": default_cands,
    }, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
