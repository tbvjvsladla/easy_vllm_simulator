#!/usr/bin/env python3
"""A2 — no-download 스모크 모델 NAS 실재 체크 (결정론적).

스모크 전, config_name 트리플릿이 가리키는 모델이 NAS 에 실제로 있는지 확인한다.
없으면 비0 종료 + 보고 — 스킬은 절대 모델을 다운로드하지 않는다(하드 제약).

흐름(정규식 파싱, yaml 의존 없음):
1. configs/<config_name>.yaml 의 `model:` (컨테이너 경로, 예 /app/models/<벤더>/<모델>) 읽기.
2. docker-compose.yaml 에서 `:/app/models` 볼륨의 호스트 루트 읽기.
3. 컨테이너 경로의 /app/models 를 호스트 루트로 치환 → 실재(isdir) 확인.

종료코드: 0=존재 / 2=모델 부재(보고) / 3=설정 파싱 실패.
사용: python3 check_smoke_model.py <config_name> [--repo .]
"""
import sys, os, re, argparse


def read_model_path(config_yaml):
    with open(config_yaml, encoding="utf-8") as f:
        for line in f:
            m = re.match(r"\s*model\s*:\s*(\S+)", line)
            if m:
                return m.group(1).strip().strip("'\"")
    return None


def read_app_models_host_root(compose_yaml):
    # `- <host>:/app/models[:ro]` 형태에서 <host> 추출
    with open(compose_yaml, encoding="utf-8") as f:
        for line in f:
            m = re.search(r"(/\S+):/app/models(?::[a-z]+)?\s*(?:#.*)?$", line)
            if m:
                return m.group(1)
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("config_name")
    ap.add_argument("--repo", default=".")
    a = ap.parse_args()

    cfg = os.path.join(a.repo, "configs", f"{a.config_name}.yaml")
    compose = os.path.join(a.repo, "docker-compose.yaml")

    if not os.path.exists(cfg):
        print(f"[NAS-check] FAIL: 트리플릿 yaml 없음 — {cfg}", file=sys.stderr)
        sys.exit(3)
    model_ctr = read_model_path(cfg)
    if not model_ctr:
        print(f"[NAS-check] FAIL: {cfg} 에 model: 없음", file=sys.stderr)
        sys.exit(3)
    host_root = read_app_models_host_root(compose)
    if not host_root:
        print(f"[NAS-check] FAIL: docker-compose 에서 /app/models 호스트 마운트 못 찾음", file=sys.stderr)
        sys.exit(3)

    host_path = model_ctr.replace("/app/models", host_root, 1)
    if os.path.isdir(host_path):
        print(f"[NAS-check] OK: {a.config_name} → {host_path} 존재")
        sys.exit(0)
    print(f"[NAS-check] STOP: 모델 부재 — {host_path}\n"
          f"  (config_name={a.config_name}, container={model_ctr})\n"
          f"  → 규칙: 다운로드하지 않음. 사람이 모델을 NAS 에 사전 다운로드해야 함.", file=sys.stderr)
    sys.exit(2)


if __name__ == "__main__":
    main()
