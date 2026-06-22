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

# known-incompat 천장 — 시간드리프트(upstream 의 >= 범위가 최신으로 해소되며 깨지는 고정 회귀)를 영속 차단.
#   {패키지명(소문자): 추가 제약}. parse_requires 가 Requires-Dist 스펙에 merge 한다.
#   ⚠ REVIEW/EXPIRE: upstream 이 회귀를 고치면 여기서 제거할 것 — regen 마다 stdout·헤더에 표면화되어
#   S1 게이트에서 운영자 재평가를 강제한다(전역-영속 핀이 미래 수정을 조용히 막는 역-드리프트 방지).
KNOWN_INCOMPAT = {
    # fastapi 0.137.0 include_router 리팩터(_IncludedRouter, .path 부재)가 prometheus-fastapi-instrumentator
    # 와 충돌 → /health 500 (vLLM #45596, testlog 2026062220). regen 이 0.138+ 를 흡수하면 재발.
    "fastapi": "<0.137.0",
}


def parse_requires(reqs):
    """Requires-Dist 라인 목록 → requirements 라인(정렬, extra-조건부 제외, base 제외, known-incompat 천장 merge).

    반환: (lines, applied). applied = 적용된 KNOWN_INCOMPAT 천장 표면화용 리스트.
    """
    out = []
    applied = []
    for r in reqs:
        # '; extra == "x"' 같은 선택 extra 조건부는 건너뜀
        if ";" in r and "extra" in r.split(";", 1)[1]:
            continue
        base = r.split(";")[0].strip()
        name = re.split(r"[<>=!\[ ]", base, 1)[0].strip().lower()
        if not name or name in BASE_PROVIDED:
            continue
        if name in KNOWN_INCOMPAT:
            ceiling = KNOWN_INCOMPAT[name]
            sep = "," if any(c in base for c in "<>=!~") else ""  # 기존 제약 있으면 콤마로 AND
            base = base + sep + ceiling
            applied.append(name + ceiling)
        out.append(base)
    return sorted(set(out)), sorted(set(applied))


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

    lines, applied = parse_requires(reqs)
    hdr = ("# vLLM 런타임 의존성 — wheel 의 Requires-Dist(METADATA) 기준(권위 소스).\n"
           f"# source: {src}\n"
           "# base 제공분(torch/torchvision/torchaudio/setuptools)은 제외(--no-deps 보호).\n"
           "# extra(fastapi[standard] 등)는 그대로 — pip 가 빌드 시 transitive(uvicorn→uvloop) 해소.\n")
    if applied:
        hdr += ("# ⚠ KNOWN_INCOMPAT 천장 적용(시간드리프트 회귀 차단; upstream 수정 시 "
                "regen_requirements.py KNOWN_INCOMPAT 에서 제거): " + "; ".join(applied) + "\n")
    with open(a.out, "w", encoding="utf-8") as f:
        f.write(hdr + "\n".join(lines) + "\n")
    print(f"[regen] {a.out} : {len(lines)} packages (source: {src})")
    if applied:
        print("[regen] ⚠ KNOWN_INCOMPAT 천장 적용: " + ", ".join(applied)
              + " — S1 게이트서 재평가/만료 확인(KNOWN_INCOMPAT).")


if __name__ == "__main__":
    main()
