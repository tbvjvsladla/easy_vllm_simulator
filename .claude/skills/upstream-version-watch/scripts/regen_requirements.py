#!/usr/bin/env python3
"""④ requirements.txt 재생성 — wheel 의 Requires-Dist(METADATA) 기준 (권위 소스).

[왜 METADATA 인가]
vLLM prebuilt wheel 은 requirements/*.txt 에 없는 런타임 deps(서버 deps·extra)를
METADATA 의 Requires-Dist 로 선언한다. 예: `fastapi[standard]` → uvicorn → uvloop.
requirements/common.txt 기준으로 생성하면 이 transitive/extra 가 누락된다(이번 세션 uvloop 사고).
따라서 의존성 정본은 requirements/*.txt 가 아니라 **wheel METADATA** 다.

[두 모드]
  --from-wheel-url URL : 호스트에서 wheel 을 받아 METADATA 를 읽어 requirements.txt 생성(빌드 전 1차).
  --use-installed      : 컨테이너 내부에서 importlib.metadata.requires('vllm') 로 정합 재생성(가장 가벼움).

[규칙]
- base 제공분(torch/torchvision/torchaudio/setuptools 등)은 제외(--no-deps 로 보호, devlog §6.1).
- '; extra ==' 조건부 라인은 제외. 필요한 extra 는 Requires-Dist 직접 라인(예 `fastapi[standard]`)에
  들어있고, 그대로 두면 pip 가 빌드 시 transitive(uvicorn→uvloop)를 해소한다(Option A).
사용:
  python3 regen_requirements.py --use-installed -o requirements.txt          # 컨테이너 내부
  python3 regen_requirements.py --from-wheel-url <wheel URL> -o requirements.txt   # 호스트(빌드 전)
"""
import sys, re, argparse, tempfile, zipfile, os, urllib.request

BASE_PROVIDED = {"torch", "torchvision", "torchaudio", "torchao",
                 "setuptools", "numpy", "pip", "wheel"}


def parse_requires(reqs):
    """Requires-Dist 라인 목록 → requirements 라인(정렬, extra-조건부 제외, base 제외)."""
    out = []
    for r in reqs:
        # '; extra == "x"' 같은 선택 extra 조건부는 건너뜀
        if ";" in r and "extra" in r.split(";", 1)[1]:
            continue
        base = r.split(";")[0].strip()
        name = re.split(r"[<>=!\[ ]", base, 1)[0].strip().lower()
        if not name or name in BASE_PROVIDED:
            continue
        out.append(base)
    return sorted(set(out))


def from_installed():
    """컨테이너 내부: 설치된 vllm 의 METADATA Requires-Dist."""
    import importlib.metadata as m
    return list(m.requires("vllm") or [])


def from_wheel_url(url):
    """호스트: wheel 을 받아 .dist-info/METADATA 의 Requires-Dist 추출."""
    with tempfile.TemporaryDirectory() as td:
        whl = os.path.join(td, "pkg.whl")
        urllib.request.urlretrieve(url, whl)
        with zipfile.ZipFile(whl) as z:
            metas = [n for n in z.namelist() if n.endswith(".dist-info/METADATA")]
            if not metas:
                raise RuntimeError("wheel 에 METADATA 없음")
            txt = z.read(metas[0]).decode("utf-8")
    return [ln.split(":", 1)[1].strip()
            for ln in txt.splitlines() if ln.startswith("Requires-Dist:")]


def main():
    ap = argparse.ArgumentParser()
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--from-wheel-url", help="wheel URL (호스트, 빌드 전 1차 생성)")
    g.add_argument("--use-installed", action="store_true", help="컨테이너 내부 정합 재생성")
    ap.add_argument("-o", "--out", default="requirements.txt")
    a = ap.parse_args()

    if a.use_installed:
        reqs, src = from_installed(), "importlib.metadata.requires('vllm') (container)"
    else:
        reqs, src = from_wheel_url(a.from_wheel_url), a.from_wheel_url

    lines = parse_requires(reqs)
    hdr = ("# vLLM 런타임 의존성 — wheel 의 Requires-Dist(METADATA) 기준(권위 소스).\n"
           f"# source: {src}\n"
           "# base 제공분(torch/torchvision/torchaudio/setuptools)은 제외(--no-deps 보호).\n"
           "# extra(fastapi[standard] 등)는 그대로 — pip 가 빌드 시 transitive(uvicorn→uvloop) 해소.\n")
    with open(a.out, "w", encoding="utf-8") as f:
        f.write(hdr + "\n".join(lines) + "\n")
    print(f"[regen] {a.out} : {len(lines)} packages (source: {src})")


if __name__ == "__main__":
    main()
