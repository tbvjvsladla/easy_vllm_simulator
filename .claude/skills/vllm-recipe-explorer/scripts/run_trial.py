#!/usr/bin/env python3
"""run_trial.py — 단일 트라이얼 실행기 (vllm-recipe-explorer 스킬 Phase 2).

CONTRACT(FROZEN) run_trial.py 절 준수. 한 candidate(lock-set)를 실제 서빙해
실측 프로파일 + 기능 스모크를 모은다.

동작(실 docker 경로):
  docker run -d 이미지(opts.image, 기본 "vllm-src-022:clean")로 서빙 →
  /health 200 폴링(opts.timeout 기본 900s) → functional_smoke →
  docker logs 를 simlog_dir/trialNN_vllm.log 로 캡처 → parse_vllm_log →
  컨테이너 teardown(docker rm -f, 통합메모리 잔류 OOM 방지).
  NAS 마운트 -v <nas_host_root>:/app/models:ro (호스트 경로는 config.nas_host_root →
  opts.nas_mount/--nas-mount 로 주입; 하드코딩 금지 — 포인터 원칙. 기본값만 NAS_MOUNT 상수),
  --runtime nvidia --ipc host --ulimit memlock=-1.
  candidate 에서 VLLM_ATTENTION_BACKEND env · --kv-cache-memory-bytes ·
  --max-num-seqs(batch) · --kv-cache-dtype · --tool-call-parser/
  --enable-auto-tool-choice · --reasoning-parser 를 구성.

--dry-run / opts.mock_profile(json path): docker 없이 주어진 vllm_profile +
  functional 결과를 반환(루프 배선 결정론 테스트용). 반드시 동작해야 함.

parse_vllm_log, functional_smoke, simlog_writer 를 import 해 사용한다.
stdlib(json,os,sys,argparse,subprocess,time,urllib) 만 사용. 외부 네트워크 없음.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

# scripts/ 디렉토리를 import 경로에 보장(직접 실행/타 스크립트에서 import 양쪽 대응).
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import parse_vllm_log  # noqa: E402
import functional_smoke  # noqa: E402
import simlog_writer  # noqa: E402

# ── 기본값 (CONTRACT) ────────────────────────────────────────────────────
DEFAULT_IMAGE = "vllm-src-022:clean"
DEFAULT_TIMEOUT = 900  # /health 200 폴링 타임아웃(초)
DEFAULT_PORT = 8903
# NAS_MOUNT 은 호스트 NAS 모델 루트의 **기본값일 뿐**이다. 실제 경로는
# config.nas_host_root → opts.nas_mount(또는 --nas-mount)로 주입된다(포인터 원칙, 하드코딩 아님).
NAS_MOUNT = "/mnt/models"  # default only — override via config.nas_host_root / --nas-mount
CONTAINER_MODELS = "/app/models"  # 컨테이너 내 모델 마운트 경로(:ro)
# 에어갭 인코딩 자산(tiktoken o200k 등) 컨테이너 마운트 경로(:ro). gpt-oss harmony/tiktoken
# 자산은 이미지에 미번들 → 호스트 tiktoken_host_path 를 여기로 마운트해 사전적재(C8).
CONTAINER_ENCODINGS = "/encodings"
HEALTH_POLL_INTERVAL = 3.0  # /health 폴링 간격(초)


class _Opts:
    """run_trial 동작 옵션 컨테이너(가벼운 namespace).

    image · timeout · port · simlog_dir 등은 run_trial 인자/opts 로 전달.
    dict 또는 attr 접근 양쪽을 받기 위해 _get 헬퍼로 정규화한다.
    """

    pass


def _opt(opts, key, default=None):
    """opts 가 dict 든 객체든 동일하게 key 를 읽는다(없으면 default)."""
    if opts is None:
        return default
    if isinstance(opts, dict):
        return opts.get(key, default)
    return getattr(opts, key, default)


def _kv_dtype_flag(candidate: dict) -> "str | None":
    """candidate.kv_cache_quant → --kv-cache-dtype 값. 없으면 None(미지정)."""
    q = candidate.get("kv_cache_quant")
    if not q:
        return None
    # vLLM --kv-cache-dtype 는 "fp8" 등 문자열을 그대로 받는다.
    return str(q)


def _build_serve_args(candidate: dict) -> list:
    """candidate(lock-set) → `vllm serve` 인자 리스트.

    모델 경로는 candidate.model_path_container(없으면 parse 출력 키) → 폴백 NAS 마운트.
    --kv-cache-memory-bytes / --max-num-seqs / --kv-cache-dtype /
    --quantization / --tool-call-parser(+--enable-auto-tool-choice) / --reasoning-parser
    를 구성한다. soft 변수는 능력 게이팅(model_capabilities)을 존중.
    """
    model_path = (
        candidate.get("model_path_container")
        or candidate.get("model_path")
        or CONTAINER_MODELS
    )
    args = ["vllm", "serve", str(model_path)]

    # ── served-model-name (스모크가 이 이름으로 /v1/chat/completions 호출 — 불일치 시 404) ─
    served = candidate.get("served_model_name")
    if served:
        args += ["--served-model-name", str(served)]

    # ── max-model-len (lock) ───────────────────────────────────────────
    if candidate.get("max_model_len") is not None:
        args += ["--max-model-len", str(int(candidate["max_model_len"]))]

    # ── max-num-seqs = batch (lock) ────────────────────────────────────
    if candidate.get("batch") is not None:
        args += ["--max-num-seqs", str(int(candidate["batch"]))]

    # ── gpu-memory-utilization (디바이스 풀 상한 = safety_margin) ───────
    # 통합메모리(GB10)는 시스템이 일부 점유 → vLLM 기본 0.92 가 free 초과 OOM.
    # SKILL §5: gmu 는 풀 상한(=margin)으로만 emit; 실제 KV 는 절대 클램프가 제어.
    gmu = candidate.get("gpu_memory_utilization")
    if gmu is not None:
        args += ["--gpu-memory-utilization", str(float(gmu))]

    # ── weight quantization (lock; none/null 은 미지정) ─────────────────
    quant = candidate.get("quantization")
    if quant and str(quant).lower() != "none":
        args += ["--quantization", str(quant)]

    # ── kv-cache-memory-bytes (free 변수 — 루프가 설정한 절대 클램프) ───
    kv_bytes = candidate.get("kv_cache_memory_bytes")
    if kv_bytes is not None:
        args += ["--kv-cache-memory-bytes", str(int(kv_bytes))]

    # ── kv-cache-dtype (kv quant lock) ─────────────────────────────────
    kv_dtype = _kv_dtype_flag(candidate)
    if kv_dtype is not None:
        args += ["--kv-cache-dtype", kv_dtype]

    # ── tool_call_parser (soft; 능력 있을 때만) ────────────────────────
    caps = candidate.get("model_capabilities") or {}
    tcp = candidate.get("tool_call_parser")
    if tcp and bool(caps.get("tool_call")):
        args += ["--enable-auto-tool-choice", "--tool-call-parser", str(tcp)]

    # ── reasoning_parser (soft; 능력 있을 때만) ────────────────────────
    rp = candidate.get("reasoning_parser")
    if rp and bool(caps.get("reasoning")):
        args += ["--reasoning-parser", str(rp)]

    # ── 서빙 포트 ──────────────────────────────────────────────────────
    args += ["--host", "0.0.0.0", "--port", str(DEFAULT_PORT)]
    return args


def _needs_tiktoken(candidate: dict) -> bool:
    """tiktoken 인코딩 자산이 필요한 모델인지(capability 기반).

    gpt-oss harmony/tiktoken o200k 자산은 이미지에 미번들 → tool_call/reasoning 류
    capability 가 있으면 에어갭 사전적재 자산을 마운트해야 한다(C8).
    """
    caps = candidate.get("model_capabilities") or {}
    return bool(caps.get("tool_call") or caps.get("reasoning"))


def _build_docker_cmd(candidate: dict, image: str, container_name: str, port: int,
                      nas_mount: str = NAS_MOUNT,
                      tiktoken_host_path: "str | None" = None) -> list:
    """docker run -d 명령 리스트를 구성한다.

    NAS read-only 마운트(nas_mount = config/manifest 의 nas_host_root, 기본 /mnt/models) ·
    --runtime nvidia · --ipc host · --ulimit memlock=-1 · 포트 매핑 ·
    VLLM_ATTENTION_BACKEND env(soft) · (에어갭) tiktoken 인코딩 마운트+env · 이미지 · serve 인자.

    tiktoken_host_path(config.tiktoken_host_path): capability(tool_call/reasoning)가 tiktoken 을
    요구하거나 경로가 명시되면 `-v <host>:/encodings:ro` + TIKTOKEN_ENCODINGS_BASE/
    TIKTOKEN_RS_CACHE_DIR/TIKTOKEN_ENABLED env 를 추가(C8, gpt-oss 차단 해소).
    """
    cmd = [
        "docker", "run", "-d",
        "--name", container_name,
        "--runtime", "nvidia",
        "--ipc", "host",
        "--ulimit", "memlock=-1",
        "-p", "%d:%d" % (int(port), DEFAULT_PORT),
        "-v", "%s:%s:ro" % (nas_mount, CONTAINER_MODELS),
    ]

    # ── 에어갭 tiktoken 인코딩 자산(C8) ────────────────────────────────
    # 마운트 opt-in = tiktoken_host_path 제공(env 주입에 자산 경로가 필요 → 경로 부재 시 마운트 불가).
    # capability상 tiktoken(harmony/o200k)이 필요하나 경로 미설정이면 폐쇄망 스모크 실패 위험 → 경고만(중단 안 함).
    if tiktoken_host_path:
        cmd += ["-v", "%s:%s:ro" % (tiktoken_host_path, CONTAINER_ENCODINGS)]
        cmd += ["-e", "TIKTOKEN_ENCODINGS_BASE=%s" % CONTAINER_ENCODINGS]
        cmd += ["-e", "TIKTOKEN_RS_CACHE_DIR=%s" % CONTAINER_ENCODINGS]
        cmd += ["-e", "TIKTOKEN_ENABLED=1"]
    elif _needs_tiktoken(candidate):
        sys.stderr.write(
            "[run_trial] WARN: candidate가 tiktoken(harmony/o200k)을 요구하나 "
            "tiktoken_host_path 미설정 — 폐쇄망 스모크 실패 가능"
            "(config.tiktoken_host_path 설정 권장).\n")

    # ── VLLM_ATTENTION_BACKEND env (soft) ──────────────────────────────
    backend = candidate.get("attention_backend")
    if backend:
        cmd += ["-e", "VLLM_ATTENTION_BACKEND=%s" % str(backend)]

    cmd += [image]
    cmd += _build_serve_args(candidate)
    return cmd


def _audit_emitted(candidate: dict, docker_cmd: list) -> None:
    """candidate 의 serve-관련 비-null 필드가 실제 emit 된 cmd 에 반영됐는지 전수 점검.

    gmu 미emit(C5) 같은 "candidate 신규 필드를 serve cmd 에 빠뜨리는 회귀류"를 클래스로
    차단(§9.3). 각 필드에 기대 플래그를 매핑하고, 비-null 인데 cmd 에 없으면 raise.
    능력 게이팅(tool_call/reasoning)으로 **의도적 제외**되는 soft 필드는 면제한다.
    """
    cmd_str = " ".join(docker_cmd)
    caps = candidate.get("model_capabilities") or {}

    # (candidate 키, 기대 플래그) — 값-무관, 플래그 존재만 확인.
    # gpu-memory-utilization 은 항상 필수 — startup free-memory 게이트(free ≥ gmu×total) + 총 cap.
    # vLLM 은 클램프 설정 시 gmu 를 *KV 사이징*에만 무시(config/cache.py)할 뿐, startup 검증엔 여전히 쓴다(E2E 실증).
    field_flags = [
        ("gpu_memory_utilization", "--gpu-memory-utilization"),
        ("max_model_len", "--max-model-len"),
        ("batch", "--max-num-seqs"),
        ("kv_cache_memory_bytes", "--kv-cache-memory-bytes"),
        ("kv_cache_quant", "--kv-cache-dtype"),
        ("attention_backend", "VLLM_ATTENTION_BACKEND="),
    ]
    # quantization: none/null 은 의도적 미emit.
    quant = candidate.get("quantization")
    if quant and str(quant).lower() != "none":
        field_flags.append(("quantization", "--quantization"))
    # tool_call_parser / reasoning_parser: 능력 있을 때만 emit(없으면 의도적 제외).
    if candidate.get("tool_call_parser") and bool(caps.get("tool_call")):
        field_flags.append(("tool_call_parser", "--tool-call-parser"))
    if candidate.get("reasoning_parser") and bool(caps.get("reasoning")):
        field_flags.append(("reasoning_parser", "--reasoning-parser"))

    missing = []
    for key, flag in field_flags:
        if candidate.get(key) is not None and flag not in cmd_str:
            missing.append("%s(기대 %s)" % (key, flag))

    if missing:
        raise RuntimeError(
            "emit-audit 실패 — candidate 의 비-null serve 필드가 cmd 에 누락됨: "
            + ", ".join(missing)
            + " | 이 가드는 gmu 미emit 같은 회귀류 클래스를 차단한다(§9.3)."
        )


def _health_url(port: int) -> str:
    return "http://127.0.0.1:%d/health" % int(port)


def _poll_health(port: int, timeout: float) -> bool:
    """/health 가 HTTP 200 을 줄 때까지 폴링. 성공 True, 타임아웃 False.

    준비판정 = :PORT/health http200 (로그 "startup complete" grep 금지 — 거짓양성).
    """
    url = _health_url(port)
    deadline = time.monotonic() + float(timeout)
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=5.0) as resp:
                if resp.status == 200:
                    return True
        except urllib.error.HTTPError as e:
            if e.code == 200:
                return True
        except Exception:  # noqa: BLE001 — 연결 거부/타임아웃은 아직 미준비
            pass
        time.sleep(HEALTH_POLL_INTERVAL)
    return False


def _docker_logs(container_name: str) -> str:
    """docker logs(stdout+stderr)를 텍스트로 캡처. 실패 시 ''.

    vLLM 은 INFO 로그를 stderr 로 내보내므로 둘 다 합쳐 캡처한다.
    """
    try:
        proc = subprocess.run(
            ["docker", "logs", container_name],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=60,
        )
        return proc.stdout.decode("utf-8", errors="replace")
    except Exception:  # noqa: BLE001
        return ""


def _docker_teardown(container_name: str) -> None:
    """컨테이너를 강제 제거(통합메모리 잔류 OOM 방지). 예외는 흡수."""
    try:
        subprocess.run(
            ["docker", "rm", "-f", container_name],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=120,
        )
    except Exception:  # noqa: BLE001
        pass


def _error_excerpt(log_text: str, limit: int = 2000) -> str:
    """로그 텍스트의 마지막 일부를 error_excerpt 로 잘라낸다(실패 진단용)."""
    if not log_text:
        return ""
    tail = log_text[-limit:]
    return tail


def _mock_result(
    candidate: dict,
    trial_number: int,
    vllm_profile: "dict | None",
    functional: "dict | None",
    log_path: str,
) -> dict:
    """--dry-run/mock_profile 모드 결과. docker 없이 주어진 값으로 반환.

    load_ok 는 vllm_profile 이 존재(=비-None)하면 True 로 본다.
    """
    load_ok = vllm_profile is not None
    return {
        "trial_number": int(trial_number),
        "candidate": candidate,
        "load_ok": bool(load_ok),
        "vllm_profile": vllm_profile,
        "functional": functional,
        "log_path": log_path,
        "error_excerpt": None,
    }


def _load_mock_profile(path: str) -> "dict | None":
    """mock_profile JSON 을 읽는다.

    형식 1: {"vllm_profile": {...}, "functional": {...}} 래퍼
    형식 2: parse_vllm_log 결과 dict 자체(이 경우 functional 은 None)
    반환: (vllm_profile, functional) 튜플은 호출자가 분리하지 않고
    여기선 dict 그대로 돌려준다(아래 run_trial 에서 분기).
    """
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def run_trial(candidate: dict, simlog_dir: str, trial_number: int, opts=None) -> dict:
    """한 candidate 를 서빙해 실측 프로파일 + 기능 스모크를 모은다.

    반환 keys:
        trial_number(int), candidate(dict), load_ok(bool),
        vllm_profile(dict|None=parse_vllm_log 결과), functional(dict|None),
        log_path(str), error_excerpt(str|None).

    opts(dict 또는 객체) 인식 키:
        image(기본 vllm-src-022:clean), timeout(기본 900), port(기본 8903),
        served_model_name(스모크용; 없으면 candidate.model_id 또는 모델 경로),
        dry_run(bool), mock_profile(json path), smoke_timeout(기본 60),
        container_name(기본 vllm_trialNN).

    dry_run 또는 mock_profile 지정 시: docker 없이 주어진 vllm_profile +
    functional 결과를 반환(루프 배선 결정론 테스트용).
    """
    image = _opt(opts, "image", DEFAULT_IMAGE)
    timeout = float(_opt(opts, "timeout", DEFAULT_TIMEOUT))
    port = int(_opt(opts, "port", DEFAULT_PORT))
    smoke_timeout = float(_opt(opts, "smoke_timeout", 60.0))
    dry_run = bool(_opt(opts, "dry_run", False))
    mock_profile_path = _opt(opts, "mock_profile", None)
    container_name = _opt(opts, "container_name", "vllm_trial%02d" % int(trial_number))

    log_path = os.path.join(simlog_dir, "trial%02d_vllm.log" % int(trial_number))

    # served_model_name 결정: opts → candidate.model_id → 모델 경로.
    served_model_name = (
        _opt(opts, "served_model_name", None)
        or candidate.get("model_id")
        or candidate.get("model_path_container")
        or candidate.get("model_path")
        or CONTAINER_MODELS
    )

    # ── DRY-RUN / MOCK 경로 (docker 없이 결정론 반환) ──────────────────
    if dry_run or mock_profile_path:
        vllm_profile = None
        functional = None
        if mock_profile_path:
            blob = _load_mock_profile(mock_profile_path)
            if isinstance(blob, dict) and (
                "vllm_profile" in blob or "functional" in blob
            ):
                vllm_profile = blob.get("vllm_profile")
                functional = blob.get("functional")
            else:
                # parse_vllm_log 결과 dict 자체로 해석.
                vllm_profile = blob
        return _mock_result(
            candidate, trial_number, vllm_profile, functional, log_path
        )

    # ── 실 DOCKER 경로 ─────────────────────────────────────────────────
    # 이전 잔류 컨테이너 제거(이름 충돌·통합메모리 잔류 방지).
    _docker_teardown(container_name)

    nas_mount = _opt(opts, "nas_mount", NAS_MOUNT)  # config/manifest nas_host_root 배선
    tiktoken_host_path = _opt(opts, "tiktoken_host_path", None)  # config.tiktoken_host_path(C8)
    docker_cmd = _build_docker_cmd(
        candidate, image, container_name, port, nas_mount,
        tiktoken_host_path=tiktoken_host_path,
    )
    # emit-audit: candidate 의 serve-관련 비-null 필드가 실제 cmd 에 반영됐는지 전수 점검
    # (gmu 미emit 회귀류 클래스 차단 — §9.3). 누락 시 즉시 raise.
    _audit_emitted(candidate, docker_cmd)

    load_ok = False
    vllm_profile = None
    functional = None
    error_excerpt = None

    try:
        proc = subprocess.run(
            docker_cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=120,
        )
        if proc.returncode != 0:
            # docker run 자체가 실패(이미지 부재·런타임 오류 등).
            err = proc.stderr.decode("utf-8", errors="replace")
            error_excerpt = _error_excerpt(err)
            # 로그/프로파일 없음 → load_ok False 로 반환.
            with open(log_path, "w", encoding="utf-8") as fh:
                fh.write(err)
            return {
                "trial_number": int(trial_number),
                "candidate": candidate,
                "load_ok": False,
                "vllm_profile": None,
                "functional": None,
                "log_path": log_path,
                "error_excerpt": error_excerpt,
            }

        # /health 200 폴링.
        healthy = _poll_health(port, timeout)
        load_ok = bool(healthy)

        if healthy:
            # 기능 스모크(완성·tool_call·reasoning) — 능력 게이팅은 smoke 가 처리.
            base_url = "http://127.0.0.1:%d" % port
            functional = functional_smoke.smoke(
                base_url, served_model_name, candidate, timeout=smoke_timeout
            )

        # docker logs 캡처 → simlog 파일로 저장 → parse_vllm_log.
        log_text = _docker_logs(container_name)
        with open(log_path, "w", encoding="utf-8") as fh:
            fh.write(log_text)
        vllm_profile = parse_vllm_log.parse_log(log_text)

        if not healthy:
            error_excerpt = _error_excerpt(log_text)

    finally:
        # 통합메모리 잔류 OOM 방지: 어떤 경로로 끝나든 컨테이너 강제 제거.
        _docker_teardown(container_name)

    return {
        "trial_number": int(trial_number),
        "candidate": candidate,
        "load_ok": bool(load_ok),
        "vllm_profile": vllm_profile,
        "functional": functional,
        "log_path": log_path,
        "error_excerpt": error_excerpt,
    }


def _main(argv: "list[str] | None" = None) -> int:
    p = argparse.ArgumentParser(
        description="단일 트라이얼 실행기 (vllm-recipe-explorer Phase 2)."
    )
    p.add_argument(
        "--candidate", required=True, help="candidate(lock-set) JSON 경로"
    )
    p.add_argument(
        "--simlog-dir", required=True, help="trialNN_vllm.log 등을 기록할 디렉토리"
    )
    p.add_argument("--trial-number", type=int, default=1)
    p.add_argument("--image", default=DEFAULT_IMAGE, help="서빙 docker 이미지")
    p.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT, help="/health 폴링 타임아웃(초)")
    p.add_argument("--port", type=int, default=DEFAULT_PORT, help="호스트 노출 포트")
    p.add_argument("--smoke-timeout", type=float, default=60.0, help="스모크 HTTP 타임아웃(초)")
    p.add_argument("--served-model-name", default=None, help="스모크용 served_model_name")
    p.add_argument("--container-name", default=None, help="컨테이너 이름(기본 vllm_trialNN)")
    p.add_argument(
        "--nas-mount",
        default=None,
        help="호스트 NAS 모델 루트(config.nas_host_root; 기본 %s). terraforming_subnode CLI 배선" % NAS_MOUNT,
    )
    p.add_argument(
        "--tiktoken-host-path",
        default=None,
        help="에어갭 tiktoken 인코딩 자산 호스트 경로(config.tiktoken_host_path) → /encodings:ro 마운트(C8)",
    )
    p.add_argument(
        "--dry-run",
        action="store_true",
        help="docker 없이 mock_profile/빈 결과를 반환(루프 테스트용)",
    )
    p.add_argument(
        "--mock-profile",
        default=None,
        help="docker 없이 사용할 vllm_profile(+functional) JSON 경로",
    )
    args = p.parse_args(argv)

    with open(args.candidate, "r", encoding="utf-8") as f:
        candidate = json.load(f)

    os.makedirs(args.simlog_dir, exist_ok=True)

    opts = {
        "image": args.image,
        "timeout": args.timeout,
        "port": args.port,
        "smoke_timeout": args.smoke_timeout,
        "served_model_name": args.served_model_name,
        "dry_run": args.dry_run,
        "mock_profile": args.mock_profile,
    }
    if args.container_name:
        opts["container_name"] = args.container_name
    if args.nas_mount:
        opts["nas_mount"] = args.nas_mount
    if args.tiktoken_host_path:
        opts["tiktoken_host_path"] = args.tiktoken_host_path

    result = run_trial(candidate, args.simlog_dir, args.trial_number, opts=opts)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())
