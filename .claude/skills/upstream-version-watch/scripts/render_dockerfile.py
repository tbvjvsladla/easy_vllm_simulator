#!/usr/bin/env python3
"""render_dockerfile.py — 스켈레톤 템플릿 → 완성 산출물 결정론 렌더러 (G2 구현).

upstream-version-watch 의 render 단계. 템플릿의 {{ PLACEHOLDER }} 토큰을
manifest(환경 사실) + resolved(버전해소값, resolve_*.py 산출)로 치환해
빌드 가능한 산출물 문자열을 만든다. ${...}(빌드ARG·compose 런타임 변수)는 건드리지 않는다.

지원 템플릿:
  Dockerfile.template               (prebuilt wheel 트랙)
  Dockerfile.source-build.template  (source-build 트랙 — torch핀 가드)
  docker-compose.template.yaml      (compose, 트랙별 dockerfile 선택)

플레이스홀더 ↔ 소스:
  {{ VLLM_VERSION }}    ← resolved.vllm_version
  {{ CUDA_VERSION }}    ← resolved.wheel.cuda            (wheel 트랙 URL)
  {{ VLLM_MANYLINUX }}  ← resolved.wheel.manylinux       (wheel 트랙 URL)
  {{ NGC_TAG }}         ← resolved.ngc_base.tag
  {{ CPU_ARCH }}        ← manifest.cpu_arch              (build-time $(uname -m))
  {{ VLLM_REF }}        ← "v"+resolved.vllm_version      (source 트랙 git ref)
  {{ TORCH_CUDA_ARCH }} ← resolved.source_build.torch_cuda_arch (예 12.1a / GB10 sm_121a)
  {{ TORCH_PIN }}       ← resolved.torch.pin
  {{ NAS_MODEL_PATH }}  ← manifest.nas_model_path
  {{ IMAGE_NAME }}      ← "easy-vllm"
  {{ IMAGE_TAG }}       ← {vllm}-cu{cuda}-{arch}-{track}  (버전-키드, 모델-키잉 금지)
  {{ DOCKERFILE }}      ← 트랙별 ("Dockerfile" | "Dockerfile.source-build")
  {{ SOURCE_BUILD_PATCH_GUARD }} ← (NGC베이스×vLLM버전) 키 가드 (검증된 키=주석 / 미인식=빌드 명시 실패)

설계: 결정론 resolve 는 스크립트, 패치 SELECT 는 검토 루프(판단계층). 아래
VALIDATED_SOURCE_BUILD_KEYS 는 P6 카탈로그(source_build_patches.yaml) 승급 전의
최소 인라인 가드다(rev3). stdlib 만 사용. manifest.yaml 은 pyyaml 있으면 사용, 없으면 flat 미니파서.
"""

import sys
import os
import re
import json
import argparse

IMAGE_NAME = "easy-vllm"

# source-build 가드 키: (NGC 베이스 태그, vLLM 버전) 튜플.
#   변별자는 torch 핀이 아니라 **NGC 베이스(=실-링크 torch) × vLLM source 버전**이다.
#   use_existing_torch.py 가 pyproject torch 핀을 버리고 NGC torch 를 링크하므로,
#   pyproject 핀(예 0.23.0 → 2.11.0)은 변별력이 없다. testlog 증거:
#     (26.03-py3, 0.22.1)=성공(strip-hoist·setuptools-rust 수동·use_existing_torch),
#     (26.05-py3, 0.23.0)=성공(torch 2.12, strip-hoist 자동 skip) — E2E 검증,
#     (26.03-py3, 0.23.0)=FAIL(torch 2.11, Tensor::layout() 부재) ← 옛 '0.23.0 동일 torch2.11 캐리' 가정의 반증.
#   미인식 키는 빌드를 명시적으로 실패시킨다(false determinism 방지 — plan rev3 §5 / SKILL.md §4.6 HITL 발견 루프 유도).
#   P6: 이 인라인 셋을 source_build_patches.yaml + 패치-리졸버 페르소나로 승급.
VALIDATED_SOURCE_BUILD_KEYS = {("26.03-py3", "0.22.1"), ("26.05-py3", "0.23.0")}

_PLACEHOLDER_RE = re.compile(r"\{\{\s*[A-Z0-9_]+\s*\}\}")


# ── 입력 로더 ────────────────────────────────────────────────────────────────
def load_resolved(path: str) -> dict:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def load_manifest(path: str) -> dict:
    """manifest.yaml 로드. pyyaml 있으면 사용, 없으면 flat-YAML 미니파서(stdlib)."""
    try:
        import yaml  # type: ignore
        with open(path, encoding="utf-8") as f:
            return yaml.safe_load(f) or {}
    except Exception:
        return _load_yaml_flat(path)


def _load_yaml_flat(path: str) -> dict:
    """flat scalar YAML 만 처리(우리 manifest 는 평탄): 'key: value  # 주석' → {key: value}.
    'key: []' → []. 블록/중첩은 무시(render 는 scalar 키만 사용)."""
    out = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            s = line.rstrip("\n")
            if not s.strip() or s.lstrip().startswith("#"):
                continue
            if s[0] in (" ", "\t", "-"):  # 들여쓰기/리스트 항목 = 중첩 → skip
                continue
            if ":" not in s:
                continue
            key, val = s.split(":", 1)
            key = key.strip()
            val = _strip_inline_comment(val).strip()
            if val in ("[]", ""):
                out[key] = [] if val == "[]" else ""
                continue
            if (val[0] == val[-1]) and val[0] in ("'", '"'):
                val = val[1:-1]
            out[key] = val
    return out


def _strip_inline_comment(val: str) -> str:
    """따옴표 밖의 ' #...' 트레일링 주석 제거(경로엔 # 없음 전제)."""
    in_q = ""
    for i, ch in enumerate(val):
        if ch in ("'", '"'):
            in_q = "" if in_q == ch else (in_q or ch)
        elif ch == "#" and not in_q and i > 0 and val[i - 1] == " ":
            return val[:i]
    return val


# ── 컨텍스트 빌드 ─────────────────────────────────────────────────────────────
def _compact_cuda(cuda: str) -> str:
    """'13.2.0.046' → '132' · '13.0' → '130' · '129' → '129'."""
    cuda = str(cuda)
    if "." in cuda:
        parts = cuda.split(".")
        return f"{parts[0]}{parts[1]}"
    return cuda


def _patch_guard(ngc_tag: str, vllm_version: str) -> str:
    key = (ngc_tag, vllm_version)
    if key in VALIDATED_SOURCE_BUILD_KEYS:
        return (f'RUN echo "[guard] source-build key ({ngc_tag} x vLLM {vllm_version}): '
                f'validated patch set ((ngc_base x vllm_version) keyed) — proceeding."')
    return (
        'RUN echo "ERROR: source-build key (' + ngc_tag + ' x vLLM ' + vllm_version + ') has NO validated patch set." && \\\n'
        '    echo "  -> run SKILL.md §4.6 HITL discovery loop, then graduate the verified" && \\\n'
        '    echo "     patches into Dockerfile.source-build.template ((ngc_base x vllm_version) guarded, P6 catalog)." && \\\n'
        '    echo "  -> refusing to build on unvalidated determinism (plan rev3 §5)." && exit 1'
    )


def build_context(manifest: dict, resolved: dict) -> dict:
    """manifest + resolved → 플레이스홀더 치환 맵."""
    vllm = str(resolved["vllm_version"])
    arch = str(manifest["cpu_arch"])
    torch_pin = str(resolved.get("torch", {}).get("pin", ""))
    ngc = resolved.get("ngc_base", {})
    ngc_tag = str(ngc.get("tag", ""))
    decision = str(resolved.get("build_track", {}).get("decision", ""))
    track = "source" if "source" in decision else "wheel"
    wheel = resolved.get("wheel", {})

    if track == "source":
        cuda = _compact_cuda(ngc.get("cuda_version", "")) or "src"
        dockerfile = "Dockerfile.source-build"
    else:
        cuda = str(wheel.get("cuda", "") or "")
        dockerfile = "Dockerfile"

    image_tag = f"{vllm}-cu{cuda}-{arch}-{track}"

    return {
        "VLLM_VERSION": vllm,
        "CPU_ARCH": arch,
        "NGC_TAG": ngc_tag,
        "TORCH_PIN": torch_pin,
        "VLLM_REF": "v" + vllm,
        "TORCH_CUDA_ARCH": str(resolved.get("source_build", {}).get("torch_cuda_arch", "")),
        "NAS_MODEL_PATH": str(manifest.get("nas_model_path", "")),
        "IMAGE_NAME": IMAGE_NAME,
        "IMAGE_TAG": image_tag,
        "DOCKERFILE": dockerfile,
        "SOURCE_BUILD_PATCH_GUARD": _patch_guard(ngc_tag, vllm),
        "CUDA_VERSION": str(wheel.get("cuda", "") or ""),
        "VLLM_MANYLINUX": str(wheel.get("manylinux", "") or ""),
    }


# ── 렌더 ─────────────────────────────────────────────────────────────────────
def _substitute(text: str, context: dict) -> str:
    for key, val in context.items():
        text = re.sub(r"\{\{\s*" + re.escape(key) + r"\s*\}\}", lambda _m, v=val: v, text)
    leftover = _PLACEHOLDER_RE.findall(text)
    if leftover:
        raise ValueError(f"미치환 플레이스홀더 잔존(컨텍스트 누락): {sorted(set(leftover))}")
    return text


def render(template_path: str, manifest: dict, version_resolution: dict) -> str:
    """스켈레톤 + manifest/해소값 → 완성 산출물 문자열. (G2 구현)

    Args:
        template_path: Dockerfile.template / Dockerfile.source-build.template /
            docker-compose.template.yaml 등 스켈레톤 경로.
        manifest: 테라포밍된 환경 매니페스트 dict(cpu_arch·nas_model_path 등).
        version_resolution: resolve_*.py 산출 dict(resolved.json 구조).

    Returns:
        {{ PLACEHOLDER }} 가 모두 치환된 산출물 문자열.

    Raises:
        ValueError: 미치환 플레이스홀더가 남으면(컨텍스트 누락) fail-loud.
    """
    with open(template_path, encoding="utf-8") as f:
        text = f.read()
    ctx = build_context(manifest, version_resolution)
    out = _substitute(text, ctx)
    # wheel 트랙 무결성: wheel URL 의 cuXXX 가 비면(데이터 누락) fail-loud
    if "vllm-${VLLM_VERSION}+cu${CUDA_VERSION}" in text and not ctx.get("CUDA_VERSION"):
        raise ValueError("wheel 트랙인데 resolved.wheel.cuda 부재 → wheel URL 불완전")
    return out


# ── self-test (A7 게이트가 호출) ──────────────────────────────────────────────
def _self_test() -> None:
    tpl = ("FROM nvcr.io/nvidia/pytorch:{{ NGC_TAG }}\n"
           "{{ SOURCE_BUILD_PATCH_GUARD }}\n"
           "ARG VLLM_REF={{ VLLM_REF }}\n"
           "# arch={{ CPU_ARCH }} tag={{ IMAGE_TAG }} torch={{ TORCH_PIN }} cuarch={{ TORCH_CUDA_ARCH }}\n")
    man = {"cpu_arch": "aarch64", "nas_model_path": "/nas"}
    # success 픽스처 ①: 검증된 (NGC 26.03, vLLM 0.22.1)
    res_a = {"vllm_version": "0.22.1", "torch": {"pin": "2.11.0"},
             "ngc_base": {"tag": "26.03-py3", "cuda_version": "13.2.0.046"},
             "build_track": {"decision": "source-build"},
             "source_build": {"torch_cuda_arch": "12.1a"}}
    out_a = _substitute(tpl, build_context(man, res_a))
    assert "{{" not in out_a, "leftover placeholder"
    assert "26.03-py3" in out_a, "ngc tag (a)"
    assert "0.22.1-cu132-aarch64-source" in out_a, f"image tag (a): {out_a!r}"
    assert "validated patch set" in out_a, "guard(validated) for (26.03, 0.22.1)"
    # success 픽스처 ②: E2E 검증 (NGC 26.05, vLLM 0.23.0) — pyproject torch핀은 2.11.0이나 실-링크 torch는 2.12
    res_b = {"vllm_version": "0.23.0", "torch": {"pin": "2.11.0"},
             "ngc_base": {"tag": "26.05-py3", "cuda_version": "13.2.0.046"},
             "build_track": {"decision": "source-build"},
             "source_build": {"torch_cuda_arch": "12.1a"}}
    out_b = _substitute(tpl, build_context(man, res_b))
    assert "26.05-py3" in out_b, "ngc tag (b)"
    assert "0.23.0-cu132-aarch64-source" in out_b, f"image tag (b): {out_b!r}"
    assert "validated patch set" in out_b, "guard(validated) for (26.05, 0.23.0)"
    # fail-loud 픽스처: (NGC 26.03, vLLM 0.23.0) = testlog 가 FAIL 로 증명 → 가드가 빌드 실패시켜야 함
    res_bad = {"vllm_version": "0.23.0", "torch": {"pin": "2.11.0"},
               "ngc_base": {"tag": "26.03-py3", "cuda_version": "13.2.0.046"},
               "build_track": {"decision": "source-build"},
               "source_build": {"torch_cuda_arch": "12.1a"}}
    out_bad = _substitute(tpl, build_context(man, res_bad))
    assert "exit 1" in out_bad, "guard(fail-loud) for (26.03, 0.23.0)"
    print("[render] self-test OK — source-build 렌더 + (NGC베이스×vLLM버전) 키 가드(검증x2/fail-loud) 정상")


def main() -> None:
    ap = argparse.ArgumentParser(description="render_dockerfile.py — G2 결정론 렌더러")
    ap.add_argument("--self-test", action="store_true", help="내장 self-test(A7 게이트)")
    ap.add_argument("--template", help="템플릿 경로")
    ap.add_argument("--manifest", default="manifest.yaml")
    ap.add_argument("--resolved", default="resolved.json")
    ap.add_argument("-o", "--out", help="출력 파일(미지정 시 stdout)")
    a = ap.parse_args()

    if a.self_test or not a.template:
        _self_test()
        return

    manifest = load_manifest(a.manifest)
    resolved = load_resolved(a.resolved)
    out = render(a.template, manifest, resolved)
    if a.out:
        with open(a.out, "w", encoding="utf-8") as f:
            f.write(out)
        print(f"[render] {a.template} → {a.out} ({len(out)} bytes)", file=sys.stderr)
    else:
        sys.stdout.write(out)


if __name__ == "__main__":
    main()
