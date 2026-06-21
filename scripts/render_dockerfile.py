#!/usr/bin/env python3
"""render_dockerfile.py — 스켈레톤 템플릿 → 완성 Dockerfile 결정론 렌더러 (G1 스텁).

테라포밍 파이프라인의 렌더 단계. Dockerfile.template / docker-compose.template.yaml 의
{{ PLACEHOLDER }} 토큰을 manifest(환경 사실) + 버전해소값(torch 핀·NGC 태그·CUDA·wheel)으로
채워 빌드 가능한 산출물 문자열을 만든다.

플레이스홀더 ↔ 소스 매핑(G2 구현 시 채움):
  {{ VLLM_VERSION }}   ← version_resolution["vllm_version"]
  {{ CUDA_VERSION }}   ← version_resolution["cuda_version"]
  {{ VLLM_MANYLINUX }} ← version_resolution["manylinux"]
  {{ NGC_TAG }}        ← version_resolution["ngc_tag"]   (torch 핀 prefix 매칭 산출)
  {{ CPU_ARCH }}       ← manifest["cpu_arch"]            (auto: uname -m)
  {{ NAS_MOUNT }}      ← manifest["nas_model_path"]      (템플릿에 토큰이 있을 때만)

이 파일은 G1 인터페이스 계약(시그니처)만 동결한다. 실제 치환 로직은 G2.
패턴은 기존 .claude/skills/vllm-recipe-explorer/scripts/gen_recipe_set.py 를 따른다.
stdlib 만 사용(sys).
"""

import sys


def render(template_path: str, manifest: dict, version_resolution: dict) -> str:
    """스켈레톤 + manifest/해소값 → 완성 Dockerfile 문자열. (G2 구현)

    Args:
        template_path: Dockerfile.template 등 스켈레톤 경로.
        manifest: 테라포밍된 환경 매니페스트 dict(cpu_arch·nas_model_path 등).
        version_resolution: 결정론 버전해소 결과 dict(vllm_version·cuda_version·ngc_tag 등).

    Returns:
        {{ PLACEHOLDER }} 가 모두 치환된 빌드 가능한 Dockerfile 문자열.
    """
    raise NotImplementedError("render = G2; G1은 인터페이스 계약만")


if __name__ == "__main__":
    print("render_dockerfile.py: G1 스텁 — 렌더 구현은 G2(테라포밍)에서 채운다.",
          file=sys.stderr)
    sys.exit(1)
