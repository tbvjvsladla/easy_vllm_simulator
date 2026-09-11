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
로드-전 RAM 게이트(plan_26071019 §2.6): 모델 실재 확인 후 체크포인트(index weight_map 참조
샤드의 실제 파일 크기 합 — total_size 아님, 2026-08-01)÷TP+floor
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


def read_config_kv_mib(config_yaml):
    """트리플렛 yaml 의 `kv-cache-memory-bytes` → MiB. 없으면 None.

    ★ 예산 선언(blackbox_session declare-budget)의 `--kv-mib` 정본이다. **손으로 적지 않는다** —
      같은 개념이 두 곳에 손으로 적히면 4종 안티패턴의 `매직넘버·결함`이고, KV 클램프를 바꿨는데
      선언이 옛 값으로 남으면 예상 바닥이 틀려 워치독이 엉뚱한 지점에서 무장한다.
      KV 미선언(=None)이면 예산 선언 자체가 불가한 것이 설계 의도다
      (policy:KV_ABSOLUTE_CLAMP_PORTABILITY · blackbox_session.cmd_declare_budget 도크스트링)."""
    with open(config_yaml, encoding="utf-8") as f:
        for line in f:
            m = re.match(r"\s*kv[-_]cache[-_]memory[-_]bytes\s*:\s*(\d+)", line)
            if m:
                return int(m.group(1)) // (1024 * 1024)
    return None


def read_config_max_model_len(config_yaml):
    """트리플렛 yaml 의 `max-model-len`. 없으면 None."""
    with open(config_yaml, encoding="utf-8") as f:
        for line in f:
            m = re.match(r"\s*max[-_]model[-_]len\s*:\s*(\d+)", line)
            if m:
                return int(m.group(1))
    return None


def triplet_declares_rope_override(config_yaml):
    """트리플렛이 rope 확장을 **실제로** 싣고 있는가(주석 ✗ · 값이 있어야 한다)."""
    with open(config_yaml, encoding="utf-8") as f:
        for line in f:
            if re.match(r"\s*#", line):
                continue                      # 주석은 인자가 아니다
            if re.search(r"(hf[-_]overrides|rope[-_]scaling)\s*:", line):
                return True
            if "VLLM_ALLOW_LONG_MAX_MODEL_LEN" in line:
                return True
    return False


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
    ap.add_argument("--no-spec-layout-check", action="store_true",
                    help="spec-decoder 레이아웃 선검사 생략(testlog_26081419 §7 후속 ① — 진단 시)")
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

        # ── spec-decoder 레이아웃 선검사(2026-08-14 신설 · testlog_26081419 §7 후속 ①) ──
        #   존재(PRESENT)와 정합(fits)은 다른 술어다. R1-b 는 이미지 실측 4건을 전부 통과하고도
        #   `KeyError 'model.layers.43.mtp_block.main_norm.weight'` 로 죽었고, 그 대조는
        #   **로드 13분**을 태우고서야 이루어졌다. index.json 키 스캔은 0.07초다.
        #   여기 배선하는 이유: 도구만 만들고 호출을 안 하면 교훈이 파일 단위로 갇힌다.
        #   ⚠ 조기 차단 전용이다 — PASS 는 서빙 성공을 뜻하지 않는다(R1-a 는 로더를 통과했다).
        # ── 컨텍스트 확장 선판정(2026-09-11 신설 · plan_26091108 R8) ──────────────────────
        #   vLLM 은 `max_model_len > max_position_embeddings` 를 **로드 진입에서** ValidationError
        #   로 친다. 그 판정을 여기로 당긴다 — camp-26090918 에서 이 죽음이 "YaRN 확장 미적용
        #   (셀 축에 미포함)" 으로 기록돼 **원인이 반대로** 남았고, 축 자체가 없었다는 거짓이 8셀의
        #   사인이 됐다. 같은 판정을 0.1초에 하고, 막는 대신 **넣을 줄을 그대로 찍어 준다.**
        _mml = read_config_max_model_len(cfg)
        if _mml is not None:
            _tr = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..",
                               "vllm-recipe-explorer", "scripts", "rope_scaling_translate.py")
            _tr = os.path.normpath(_tr)
            if os.path.isfile(_tr):
                import json as _json
                import subprocess
                _r = subprocess.run([sys.executable, _tr, "--model-dir", host_path,
                                     "--target-len", str(_mml), "--json"],
                                    capture_output=True, text=True)
                try:
                    _rope = _json.loads(_r.stdout)
                except ValueError:
                    _rope = None
                if _rope and _rope.get("needed") and not triplet_declares_rope_override(cfg):
                    print("[NAS-check] STOP: max-model-len=%d 이 모델 네이티브 %s(%s)를 넘는데 "
                          "트리플렛에 rope 확장 인자가 없다." % (
                              _mml, _rope.get("native_max_position_embeddings"),
                              _rope.get("native_source")), file=sys.stderr)
                    print("     ⇒ 이대로 로드하면 vLLM 이 ValidationError 로 즉사한다. "
                          "로드는 **0초도 시작하지 않았다**.", file=sys.stderr)
                    if _rope.get("hf_overrides"):
                        print("     트리플렛에 이 줄을 넣어라(기존 rope 키를 전부 실은 병합 결과다 — "
                              "갈아끼우면 mrope·partial rotary 가 사라진다):", file=sys.stderr)
                        print("       hf-overrides: '%s'"
                              % _json.dumps(_rope["hf_overrides"], separators=(",", ":")),
                              file=sys.stderr)
                    for _u in (_rope.get("unknowns") or []):
                        print("     ⚠ 미검증: %s" % _u, file=sys.stderr)
                    for _why in (_rope.get("reasons") or []):
                        print("     · %s" % _why, file=sys.stderr)
                    sys.exit(9)
            else:
                print("[NAS-check] ⚠ rope 번역기 부재 — 컨텍스트 확장 선판정 생략", file=sys.stderr)

        if not a.no_spec_layout_check:
            spec_script = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                       "check_spec_layout.py")
            if os.path.isfile(spec_script):
                import subprocess
                r = subprocess.run([sys.executable, spec_script,
                                    "--model-dir", host_path, "--config", cfg])
                if r.returncode == 8:
                    print("[NAS-check] STOP: spec-decoder 레이아웃 불일치 — 위 사유 참조 "
                          "(로드 전 차단)", file=sys.stderr)
                    sys.exit(8)
                elif r.returncode not in (0,):
                    # 선검사 내부 오류가 NAS-check 의 0/2/3 계약을 깨선 안 된다 →
                    # fail-open + 경고(모델 실재는 이미 확인됨). 차단은 rc=8 만.
                    print(f"[NAS-check] ⚠ spec 레이아웃 선검사 비정상 종료(rc={r.returncode}) — "
                          "차단하지 않음", file=sys.stderr)
            else:
                print("[NAS-check] ⚠ check_spec_layout.py 부재 — spec 레이아웃 선검사 생략",
                      file=sys.stderr)

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
                    # ★ 예산 선언(plan_26081415 C3-2)의 파생 입력. 게이트 통과 여부와 **독립**으로
                    #   emit 한다 — 게이트가 거부하면 스모크는 어차피 멈추지만, 여기서 조건을 겹치면
                    #   "게이트는 통과했는데 선언 입력이 안 나온다"는 침묵 결손이 생긴다.
                    #   스모크가 ckpt·tp·kv 를 **다시 파싱하지 않게** 하려는 것이다 —
                    #   같은 값을 두 곳에서 읽으면 반드시 갈린다(권위 평면 계약과 같은 부류).
                    #   weights 는 노드당 몫이므로 스모크가 ckpt÷tp 로 나눈다(R0 실측 79,578 과 일치).
                    if a.emit_gate_params and ckpt:
                        kvm = read_config_kv_mib(cfg)
                        # ★ 2026-09-11(plan_26091108 R2): `ple_mib` 를 함께 싣는다. weights 는
                        #   `ckpt ÷ tp` 인데 그 일부가 NVMe mmap 으로 빠지면 **상주하지 않는다** —
                        #   파일 크기 기준이라 res/mmp 가 같은 값이 되고, 게이트가 실제로 뜨는
                        #   셀(camp-26090918 `fp8+mmp`, 실서빙 성공)을 죽인다. 모드 판정은 여기서
                        #   하지 않는다: 그건 serve 평면의 사실(`VLLM_PLE_MMAP`)이고 스모크가 든다.
                        #   여기는 **모델 사실**(이 체크포인트에서 빠질 수 있는 바이트)만 낸다.
                        _n0 = len(warns)
                        _ple = preload_ram_gate.offloadable_bytes_for(host_path, warnings=warns)
                        for w in warns[_n0:]:
                            print(f"[NAS-check] ⚠ {w}", file=sys.stderr)
                        print("[NAS-check] BUDGET_PARAMS ckpt_mib=%d tp=%d kv_mib=%s ple_mib=%s"
                              % (int(ckpt / (1024 * 1024)), tp,
                                 kvm if kvm is not None else "none",
                                 int(_ple / (1024 * 1024)) if _ple is not None else "unknown"))
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
