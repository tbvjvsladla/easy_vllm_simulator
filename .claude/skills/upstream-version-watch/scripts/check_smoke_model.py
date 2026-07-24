#!/usr/bin/env python3
"""A2 — no-download 스모크 모델 NAS 실재 체크 (결정론적).

스모크 전, config_name 트리플릿이 가리키는 모델이 NAS 에 실제로 있는지 확인한다.
없으면 비0 종료 + 보고 — 스킬은 절대 모델을 다운로드하지 않는다(하드 제약).

흐름(정규식 파싱, yaml 의존 없음):
1. configs/<config_name>.yaml 의 `model:` (컨테이너 경로, 예 /app/models/<벤더>/<모델>) 읽기.
2. docker-compose.yaml 에서 `:/app/models` 볼륨의 호스트 루트 읽기.
3. 컨테이너 경로의 /app/models 를 호스트 루트로 치환 → 실재(isdir) 확인.

종료코드: 0=존재(+RAM 게이트 통과) / 2=모델 부재(보고) / 3=설정 파싱 실패 / 7=로드-전 RAM 게이트 거부.
사용: python3 check_smoke_model.py <config_name> --topology {single|multi} [--repo .] [--no-ram-gate]
      python3 check_smoke_model.py <config_name> --base output/multi [--repo .]
산출물 통로(plan_26062312): configs/·docker-compose.yaml 는 output/<topology>/ 아래에 있다.
  → --topology 또는 --base 로 그 통로를 명시한다(루트 경로 폴백 금지 — fail-loud, single·multi 양쪽 정합).
로드-전 RAM 게이트(plan_26071019 §2.6): 모델 실재 확인 후 체크포인트(index total_size)÷TP+floor
  vs MemAvailable 을 결정론 판정(부족 시 drop-caches 1회 자동 → 재측정 → 거부 exit7). 게이트 소유는
  recipe-explorer(preload_ram_gate.py — 발견≠소유), 여기는 serve-평면 소비자. TP 미해소 시 경고 후
  게이트 생략(음성정직 — 거짓 TP 로 false-block 하지 않음).
"""
import sys, os, re, argparse


def read_config_tp(config_yaml):
    """트리플렛 yaml 의 명시 serve TP(`tensor-parallel-size`/`tensor_parallel_size`). 없으면 None.

    RAM 게이트의 노드당 로드 모델(ckpt÷serve-TP)은 **실제 서빙 TP** 가 정본 입력이다 — manifest 전체
    GPU 수가 아님. 헌법 manifest→서빙전략 배선 불변식('config 명시 override > manifest')과 정합:
    8-GPU 단일노드에서 tp=1 트리플렛으로 소형모델을 서빙하면 게이트는 ckpt÷1 로 판정해야 하는데
    manifest gpus_per_node=8 을 쓰면 ckpt÷8 로 과소산정(false-PASS) → 게이트 무력화."""
    with open(config_yaml, encoding="utf-8") as f:
        for line in f:
            m = re.match(r"\s*tensor[-_]parallel[-_]size\s*:\s*(\d+)", line)
            if m:
                return int(m.group(1))
    return None


def read_manifest_tp(base):
    """<base>/manifest.yaml 에서 TP = gpus_per_node × 노드 수(role: 라인 수, nodes 비면 1).
    **topology=single 이면 노드 배수 = 1 고정** — single 의 nodes[role=sub]는 sub-control 피어이지
    텐서 워커가 아님(δ1-2 `_target_tp` 오인 버그와 동일 클래스 차단 — devlog_26070813 PARKED).
    해소 실패 시 None(게이트 생략 신호 — manifest→서빙전략 배선 불변식의 serve-평면 소비)."""
    mpath = os.path.join(base, "manifest.yaml")
    if not os.path.isfile(mpath):
        return None
    try:
        text = open(mpath, encoding="utf-8").read()
    except OSError:
        return None
    g = re.search(r"^\s*gpus_per_node\s*:\s*(\d+)", text, re.M)
    if not g:
        return None
    topo = re.search(r"^\s*topology\s*:\s*(\w+)", text, re.M)
    if topo and topo.group(1).strip() == "single":
        return int(g.group(1))
    roles = re.findall(r"^\s*-?\s*role\s*:\s*(?:main|sub)\b", text, re.M)
    return int(g.group(1)) * max(1, len(roles))


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


def read_manifest_field(base, field="nas_model_path"):
    """<base>/manifest.yaml 의 <field> (정본 호스트 루트, terraforming scan). 없으면 None.
    field = 마운트 토큰 env-var 의 소문자형: NAS_MODEL_PATH→nas_model_path(/app/models) ·
    QUANT_MODEL_PATH→quant_model_path(/app/quant_models 2차 마운트) — 마운트마다 정본 필드가 다르다."""
    mpath = os.path.join(base, "manifest.yaml")
    if not os.path.isfile(mpath):
        return None
    with open(mpath, encoding="utf-8") as f:
        for line in f:
            m = re.match(r"\s*" + re.escape(field) + r"\s*:\s*(\S+)", line)
            if m:
                return m.group(1).strip().strip("'\"")
    return None


def resolve_app_models_host_root(token, base):
    """compose 토큰 → 실제 호스트 루트. 서브-소비자(serve-time)와 동일 우선순위로 해소:
    env(런타임 주입) > manifest.<var 소문자>(정본) > compose default.
    token 이 ${VAR:-default} 면 그 구문을 해소(리터럴 default 의 stray '}' 버그 회피).
    ⚠ 2차 마운트(/app/quant_models = ${QUANT_MODEL_PATH:-…})는 nas_model_path 가 아니라 quant_model_path 를
      정본으로 읽어야 한다 → manifest 필드를 var 이름에서 도출(var.lower()). 리터럴 경로 토큰이면 그대로 반환."""
    m = re.fullmatch(r"\$\{(\w+)(?::-([^}]*))?\}", token or "")
    if not m:                       # 리터럴 경로(env-var 아님)
        return token
    var, default = m.group(1), m.group(2)
    env_val = os.environ.get(var)
    if env_val:
        return env_val             # 런타임 주입값(serve-time 과 동일)
    man = read_manifest_field(base, var.lower())   # var-aware: QUANT_MODEL_PATH→quant_model_path 정본
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
    ap.add_argument("--no-ram-gate", action="store_true",
                    help="로드-전 RAM 게이트 생략(plan_26071019 §2.6 — 진단/강제 시)")
    ap.add_argument("--emit-gate-params", action="store_true",
                    help="게이트 통과 시 stdout 에 'GATE_PARAMS required_mib=<n>' 출력"
                         "(멀티노드 스모크가 슬레이브 노드 동일-문턱 검사에 재사용 — §2.6 예방 대칭)")
    a = ap.parse_args()

    # 산출물 통로 해소: --base 우선, 없으면 output/<topology>/. 둘 다 없으면 fail-loud(루트 폴백 금지).
    if a.base:
        base = a.base if os.path.isabs(a.base) else os.path.join(a.repo, a.base)
    elif a.topology:
        base = os.path.join(a.repo, "output", a.topology)
    else:
        print("[NAS-check] FAIL: --topology {single|multi} 또는 --base 필요 "
              "(configs·compose 는 output/<topology>/ 통로에 있음 — plan_26062312)", file=sys.stderr)
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
        # ── 로드-전 RAM 게이트(plan_26071019 §2.6 — serve 평면) ──
        if not a.no_ram_gate:
            # serve TP 정본 = 트리플렛 명시값 > manifest GPU 수(config override > manifest 불변식).
            tp = read_config_tp(cfg)
            if tp is None:
                tp = read_manifest_tp(base)
            if tp is None:
                print("[NAS-check] ⚠ serve TP 해소 실패(트리플렛·manifest 모두) — RAM 게이트 생략"
                      "(음성정직·false-block 금지)", file=sys.stderr)
            else:
                gate_dir = os.path.join(a.repo, ".claude", "skills", "vllm-recipe-explorer", "scripts")
                try:
                    sys.path.insert(0, gate_dir)
                    import preload_ram_gate  # noqa: E402
                    warns = []
                    ckpt = preload_ram_gate.checkpoint_bytes_for(host_path, warns)
                    for w in warns:
                        print(f"[NAS-check] ⚠ {w}", file=sys.stderr)
                    res = preload_ram_gate.gate(ckpt, tp=tp)
                    if res["ok"] and not res.get("skipped"):
                        print("[NAS-check] RAM-gate PASS: MemAvailable=%sMiB ≥ required=%sMiB(ckpt÷tp=%d+floor)"
                              % (res["avail_after_mib"], res["required_mib"], tp))
                        # 멀티노드 스모크가 슬레이브 노드에 같은 문턱을 적용하도록 required 를 emit.
                        if a.emit_gate_params and res.get("required_mib"):
                            print("[NAS-check] GATE_PARAMS required_mib=%d" % res["required_mib"])
                    if not res["ok"]:
                        print("[NAS-check] STOP: 로드-전 RAM 게이트 거부 — MemAvailable=%sMiB < "
                              "required=%sMiB(ckpt÷tp=%d+floor). 잔존 컨테이너/페이지캐시 정리 후 재시도 "
                              "(헌법 호스트 안전체계 따름정리)"
                              % (res["avail_after_mib"], res["required_mib"], tp), file=sys.stderr)
                        sys.exit(7)
                except ImportError:
                    print("[NAS-check] ⚠ preload_ram_gate 모듈 부재 — RAM 게이트 생략", file=sys.stderr)
                except SystemExit:
                    raise  # exit 7(거부)은 그대로 전파
                except Exception as e:  # noqa: BLE001
                    # 게이트 내부 오류(예 파일 IO)가 NAS-check 의 0/2/3 계약을 깨선 안 됨 →
                    # fail-open + 경고(모델 실재는 이미 확인됨. 안전망은 워치독 상시층이 최후 커버).
                    print("[NAS-check] ⚠ RAM 게이트 내부 오류 — 생략(모델 실재는 OK): %r" % e, file=sys.stderr)
        sys.exit(0)
    print(f"[NAS-check] STOP: 모델 부재 — {host_path}\n"
          f"  (config_name={a.config_name}, container={model_ctr})\n"
          f"  → 규칙: 다운로드하지 않음. 사람이 모델을 NAS 에 사전 다운로드해야 함.", file=sys.stderr)
    sys.exit(2)


if __name__ == "__main__":
    main()
