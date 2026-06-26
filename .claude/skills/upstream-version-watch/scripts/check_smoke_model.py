#!/usr/bin/env python3
"""A2 — no-download 스모크 모델 NAS 실재 체크 (결정론적).

스모크 전, config_name 트리플릿이 가리키는 모델이 NAS 에 실제로 있는지 확인한다.
없으면 비0 종료 + 보고 — 스킬은 절대 모델을 다운로드하지 않는다(하드 제약).

흐름(정규식 파싱, yaml 의존 없음):
1. configs/<config_name>.yaml 의 `model:` (컨테이너 경로, 예 /app/models/<벤더>/<모델>) 읽기.
2. docker-compose.yaml 에서 `:/app/models` 볼륨의 호스트 루트 읽기.
3. 컨테이너 경로의 /app/models 를 호스트 루트로 치환 → 실재(isdir) 확인.

종료코드: 0=존재 / 2=모델 부재(보고) / 3=설정 파싱 실패.
사용: python3 check_smoke_model.py <config_name> --topology {single|multi} [--repo .]
      python3 check_smoke_model.py <config_name> --base output/multi [--repo .]
산출물 통로(plan_2026062312_1): configs/·docker-compose.yaml 는 output/<topology>/ 아래에 있다.
  → --topology 또는 --base 로 그 통로를 명시한다(루트 경로 폴백 금지 — fail-loud, single·multi 양쪽 정합).
"""
import sys, os, re, argparse


def read_model_path(config_yaml):
    with open(config_yaml, encoding="utf-8") as f:
        for line in f:
            m = re.match(r"\s*model\s*:\s*(\S+)", line)
            if m:
                return m.group(1).strip().strip("'\"")
    return None


def read_app_models_host_token(compose_yaml, container_root="/app/models"):
    # `- <token>:<container_root>[:ro]` 의 <token> 추출. token 은 리터럴 경로 또는 env-var 구문
    # `${NAS_MODEL_PATH:-/mnt/models}`(테라포밍이 manifest.nas_model_path→env 로 주입) 일 수 있다.
    # container_root 는 모델 컨테이너 경로의 마운트 prefix(/app/models | /app/quant_models 등 2차 마운트).
    with open(compose_yaml, encoding="utf-8") as f:
        for line in f:
            m = re.search(r"-\s*(\S+):" + re.escape(container_root) + r"(?::[a-z]+)?\s*(?:#.*)?$", line)
            if m:
                return m.group(1)
    return None


def read_manifest_nas_root(base):
    """<base>/manifest.yaml 의 nas_model_path (정본 호스트 NAS 루트, terraforming scan). 없으면 None."""
    mpath = os.path.join(base, "manifest.yaml")
    if not os.path.isfile(mpath):
        return None
    with open(mpath, encoding="utf-8") as f:
        for line in f:
            m = re.match(r"\s*nas_model_path\s*:\s*(\S+)", line)
            if m:
                return m.group(1).strip().strip("'\"")
    return None


def resolve_app_models_host_root(token, base):
    """compose 토큰 → 실제 호스트 NAS 루트. 서브-소비자(serve-time)와 동일 우선순위로 해소:
    env(NAS_MODEL_PATH 런타임 주입) > manifest.nas_model_path(정본) > compose default.
    token 이 ${VAR:-default} 면 그 구문을 해소(리터럴 default 의 stray '}' 버그 회피).
    리터럴 경로 토큰이면 그대로 반환."""
    m = re.fullmatch(r"\$\{(\w+)(?::-([^}]*))?\}", token or "")
    if not m:                       # 리터럴 경로(env-var 아님)
        return token
    var, default = m.group(1), m.group(2)
    env_val = os.environ.get(var)
    if env_val:
        return env_val             # 런타임 주입값(serve-time 과 동일)
    man = read_manifest_nas_root(base)
    if man:
        return man                 # 정본 = manifest(env 미주입 시 serve 가 받아야 할 값)
    return default or None         # compose default(/mnt/models) — 최후


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("config_name")
    ap.add_argument("--repo", default=".")
    ap.add_argument("--topology", choices=["single", "multi"],
                    help="산출물 통로 output/<topology>/ 선택")
    ap.add_argument("--base",
                    help="configs/·docker-compose.yaml 을 담은 디렉토리 직접 지정(--topology 보다 우선)")
    a = ap.parse_args()

    # 산출물 통로 해소: --base 우선, 없으면 output/<topology>/. 둘 다 없으면 fail-loud(루트 폴백 금지).
    if a.base:
        base = a.base if os.path.isabs(a.base) else os.path.join(a.repo, a.base)
    elif a.topology:
        base = os.path.join(a.repo, "output", a.topology)
    else:
        print("[NAS-check] FAIL: --topology {single|multi} 또는 --base 필요 "
              "(configs·compose 는 output/<topology>/ 통로에 있음 — plan_2026062312_1)", file=sys.stderr)
        sys.exit(3)

    cfg = os.path.join(base, "configs", f"{a.config_name}.yaml")
    compose = os.path.join(base, "docker-compose.yaml")

    if not os.path.exists(cfg):
        print(f"[NAS-check] FAIL: 트리플릿 yaml 없음 — {cfg}", file=sys.stderr)
        sys.exit(3)
    model_ctr = read_model_path(cfg)
    if not model_ctr:
        print(f"[NAS-check] FAIL: {cfg} 에 model: 없음", file=sys.stderr)
        sys.exit(3)
    if not os.path.exists(compose):
        print(f"[NAS-check] FAIL: docker-compose.yaml 없음 — {compose} "
              "(render 산출물 통로 확인 — output/<topology>/)", file=sys.stderr)
        sys.exit(3)
    # 컨테이너 마운트 prefix 도출(/app/models | /app/quant_models 등 2차 마운트 지원).
    parts = model_ctr.split("/")
    container_root = "/".join(parts[:3]) if len(parts) >= 3 else "/app/models"
    token = read_app_models_host_token(compose, container_root)
    if not token:
        print(f"[NAS-check] FAIL: docker-compose 에서 {container_root} 호스트 마운트 못 찾음", file=sys.stderr)
        sys.exit(3)
    host_root = resolve_app_models_host_root(token, base)
    if not host_root:
        print(f"[NAS-check] FAIL: {container_root} 호스트 루트 해소 실패(token={token}) — "
              "manifest.nas_model_path 또는 NAS_MODEL_PATH env 확인", file=sys.stderr)
        sys.exit(3)

    host_path = model_ctr.replace(container_root, host_root, 1)
    if os.path.isdir(host_path):
        print(f"[NAS-check] OK: {a.config_name} → {host_path} 존재")
        sys.exit(0)
    print(f"[NAS-check] STOP: 모델 부재 — {host_path}\n"
          f"  (config_name={a.config_name}, container={model_ctr})\n"
          f"  → 규칙: 다운로드하지 않음. 사람이 모델을 NAS 에 사전 다운로드해야 함.", file=sys.stderr)
    sys.exit(2)


if __name__ == "__main__":
    main()
