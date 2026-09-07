#!/usr/bin/env python3
"""run_trial.py — 단일 트라이얼 실행기 (vllm-recipe-explorer 스킬 Phase 2).

CONTRACT(FROZEN) run_trial.py 절 준수. 한 candidate(lock-set)를 실제 서빙해
실측 프로파일 + 기능 스모크를 모은다.

동작(실 docker 경로):
  docker run -d 이미지(opts.image, 기본 "vllm-src-022:clean")로 서빙 →
  /health 200 폴링 + 컨테이너 생존검사(opts.timeout 기본 900s; 컨테이너가 죽으면 타임아웃을
  기다리지 않고 즉시 이탈 — plan_26082223 결함 C) → functional_smoke →
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
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

# scripts/ 디렉토리를 import 경로에 보장(직접 실행/타 스크립트에서 import 양쪽 대응).
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import parse_vllm_log  # noqa: E402
import functional_smoke  # noqa: E402
from gen_recipe_set import serve_env_pairs  # noqa: E402  (serve_env 모양의 단일 소유자 — 두 자리에 적으면 갈린다)
import simlog_writer  # noqa: E402

# ── trial 산출물의 출처(provenance) — 단일 소유 (2026-08-13 · plan_26081314 D1) ──────────────
#   결정론 규율: **실측한 값과 만들어낸 값은 데이터에서 구분되어야 한다.** 구분이 없으면 하류의
#   판정·인증서가 무엇을 근거로 삼았는지 알 수 없고, 헌법의 `합성 금지`·`측정 > 공식` 이 집행 불가가 된다.
#   (근거 규율 = CoC 가 성립하는 이유: 인터프리터 실행분과 LM 에뮬레이션분을 program state 에서 색으로
#    가른다. 기법이 아니라 이 구분 규율만 이식한다 — seed/Chain of Code…pdf Fig.1)
PROVENANCE_MEASURED = "measured"   # 실제 docker 기동 + 로그 파싱으로 얻은 값
PROVENANCE_MOCK = "mock"           # --mock-profile 로 주입한 값(docker 미기동)
PROVENANCE_DRY_RUN = "dry-run"     # --dry-run: 값 자체가 없음(배선 점검용)
PROVENANCE_VALUES = (PROVENANCE_MEASURED, PROVENANCE_MOCK, PROVENANCE_DRY_RUN)

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

# ── 호스트 안전바닥 (policy:HOST_SAFETY_LAYERED_DEFENSE) ─────────────────────────────
#   트라이얼 협역 워치독(`mem_watchdog.sh`)이 `MemAvailable < 이 값` 에서 컨테이너를 죽인다.
#   **단일 소유**: 이전엔 같은 10240 이 `_start_memwatch` 기본인자와 그 호출부 두 곳에 손으로
#   적혀 있었고(§4종 안티패턴 — 같은 개념이 두 곳 이상), 이제 gmu 캡까지 세 번째 소비자가 됐다.
#   ⚠ 이 값을 **낮추는 것은 처방이 아니다** — 낮추면 호스트 하드다운 방어가 얇아진다.
#   결함 B 의 처방은 바닥을 내리는 것이 아니라 **요구(gmu)를 깎는 것**이다.
HOST_FLOOR_MIB = 10240

#   캡은 근사식이다(vLLM 실제 할당 ≈ gmu × total, 풀 밖 상주분·단편화 미반영). 바닥에 딱 붙여
#   착지시키면 근사오차가 그대로 트립이 된다 → 오차 흡수분을 둔다. 이 파일에서만 쓰는 국소 상수.
HOST_FLOOR_HEADROOM_MIB = 2048

#   캡을 걸더라도 측정이 성립해야 한다 — KV 가 이보다 작아질 상한이면 캡을 **걸지 않는다**
#   (사살은 회복 가능하지만, weights 도 못 올리는 상한은 회복 불가한 하드 실패가 된다).
MIN_MEASURE_KV_MIB = 4096

# ── 서빙 예산 선언 (2026-08-16 신설 · plan_26081415 C3-1 "단일노드 경로도 대칭 적용") ─────────
#   왜 여기 있나: ETA 워치독의 트립 조건은 `... && mem <= BB_ARM_CEILING_MIB && ...` 이고,
#   **선언이 없으면 그 상한이 999999999** 라 AND 가 무력화된다 = ETA 규칙이 무제한으로 작동한다.
#   그 규칙은 모델 로드의 유계 하강을 무계로 읽어 **58 GiB 급 정상 로드를 3회 중 3회 사살**했다
#   (2026-08-01 · testlog_26073123). 처방(선언된 바닥)은 2026-08-14 에 multi 스모크에 배선됐고
#   계획은 단일노드도 대칭 적용하라 했으나 **미구현으로 남아 있었다** — 2026-08-16 조사에서
#   `declare-budget` 호출자가 전 코드베이스에 `multinode_serve_smoke.sh` 하나뿐임이 확인됐다.
#   상시 워치독은 `@vllm` **광역 필터**로 돌므로 여기 trial 컨테이너도 사살 대상이다.
#
#   ⚠ 이 파일의 다른 방어층은 이 클래스를 못 막는다: `preload_ram_gate` 는 로드 **전**만 보고,
#     `engine_liveness_watchdog` 는 "메모리 정상 + 엔진 교착"이라는 **다른 서브클래스**이며,
#     `oom_score_adj` 는 진성 OOM 용이다. 위양성 사살의 유일한 제한자가 이 선언이다.
#
#   실패 정책 = **경고 후 진행 + `budget-skip` 이벤트**(multi 의 진입 차단과 다르다). 근거:
#     ① trial-loop 는 KV 절대클램프를 **수렴시키는 탐색**이라 초기 후보에 kv 가 없을 수 있는데,
#        거기서 차단하면 루프 자체가 죽는다(게이트가 겨눌 주체는 로드이지 탐색이 아니다).
#     ② 같은 파일 계열의 선례가 이미 그 방향이다 — `preload_ram_gate.gate()` 는 크기 미상일 때
#        "게이트 생략(음성정직·false-block 금지)" 한다.
#     ③ 침묵은 금지된다 — 생략은 반드시 `budget-skip` 이벤트로 남는다(plan_26081415 C3 기준3).
def _repo_root() -> str:
    """이 스크립트 위치에서 레포 루트를 파생한다(.claude/skills/<skill>/scripts/ → 4단계 위).

    manifest 경로에서 거슬러 올라가지 않는다 — manifest 는 `output/<topology>/` 아래라 깊이가
    다르고, 그 가정이 틀리면 로그를 **레포 밖**에 쓰게 된다.
    """
    return os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                         "..", "..", "..", ".."))


# 노드 도구(node_blackbox·host_safety)가 사는 두 평면. 메인은 **소유자 스킬** 아래에 있고,
# 서브는 렌더 트리라 `.claude/runtime/<plane>/` 로 배달받는다(`render_sub_env.py`).
_NODE_TOOL_PLANES = {
    "node_blackbox": (".claude", "skills", "terraforming_node", "scripts", "node_blackbox"),
    "host_safety":   (".claude", "skills", "terraforming_node", "scripts", "host_safety"),
}


def _node_tool_path(plane: str, name: str) -> "str | None":
    """노드 도구의 위치 — **소유자 정본 → 서브 런타임 배달분** 순. 없으면 None.

    ★ 이 두-후보 패턴의 **단일 소유자**다(2026-09-07). 종전에는 같은 패턴이 손으로 두 번 적혀
      있었고(`_budget_session_path`·`_memwatch_script_path`), 세 번째 호출부인 node_id 해소기가
      그 선례를 따르지 않아 **정본 경로 하나만** 들었다. 메인에는 그 경로가 실재하므로 결함이
      보이지 않는다 — 서브에서만 부재가 되고, 하필 그 해소기는 **서브 무보호 로드를 닫으려고**
      넣은 것이었다(결함 ⑧). 같은 개념이 여러 곳에 손으로 적히면 갈라진다는 4종 안티패턴의
      매직넘버 결함과 같은 형태라, 처방은 주석이 아니라 **소유 단일화**다.
    """
    root = _repo_root()
    owner = _NODE_TOOL_PLANES[plane]
    candidates = (
        os.path.join(root, *owner, name),
        os.path.join(root, ".claude", "runtime", plane, name),
    )
    return next((c for c in candidates if os.path.isfile(c)), None)


def _canonical_node_tool_path(plane: str, name: str) -> str:
    """소유자 정본 경로(부재해도 반환). 호출부가 부재를 **자기 사유로** 알리고 싶을 때 쓴다."""
    return os.path.join(_repo_root(), *_NODE_TOOL_PLANES[plane], name)


def _budget_session_path():
    """예산 세션 해소기의 위치 — **소유자 정본 → 서브 런타임 배달분** 순(2026-09-05).

    같은 파일의 `_memwatch_script_path()` 가 이미 이 두-후보 패턴을 쓴다. 이 상수만 정본
    경로 하나로 굳어 있었고, **메인에는 그 경로가 실재하므로 결함이 보이지 않았다** —
    서브는 `.claude/runtime/node_blackbox/` 로 배달받으므로 거기엔 없다.

    실증(서브가 발견 · 캠페인 1 · 2026-09-05): 서브에서 `kv_cache_memory_bytes` 를 명시하면
    `_budget_declare()` 가 이 경로를 subprocess 로 불러 rc=2 FileNotFoundError 로 **컨테이너
    기동 전에** 죽었다. 그것을 피하려고 클램프를 비우면 vLLM 이 KV 88.23 GiB 를 자동산정해
    6초 만에 MemAvailable 94.9→9.47 GB 로 떨어졌고 전역 워치독이 정당하게 docker_kill 했다.
    즉 **안전하게 서빙하는 유일한 경로가 코드로 막혀 있었다**(막힘 3분류의 침묵 누락).

    부재는 여기서 죽이지 않는다 — 호출부가 `budget-skip` 이벤트로 남기는 기존 계약을 지킨다.
    """
    return (_node_tool_path("node_blackbox", "blackbox_session.py")
            or _canonical_node_tool_path("node_blackbox", "blackbox_session.py"))


BUDGET_SESSION_PY = _budget_session_path()
# blackbox_session --overhead-mib 기본값과 동일. **이 값은 "안전측"이 아니다** —
#   overhead 를 낮게 잡으면 선언 바닥(mem_total - weights - kv - overhead)이 **높게** 나오고,
#   워치독 arm 상한도 같이 높아져 **정상 서빙이 무장 밴드 안에 들어간다**. 2026-09-04 실측:
#   gpt-oss-120b/GB10 의 실제 overhead 는 17,971 MiB 로 이 기본값보다 5,683 MiB 크다.
#   그래서 이건 기본값일 뿐이고, 워크로드가 아는 값이 있으면 `--overhead-mib` 로 넘긴다.
# 2026-09-05(G-B1): 기본값 삭제. overhead 는 **워크로드 사실**이라 상수로 두면 매번 틀리고,
# 틀린 방향이 하필 무장 밴드를 넓히는 쪽이다. 선언 경로는 후보 config 의 `overhead_mib`,
# 또는 환경 `TRIAL_BUDGET_OVERHEAD_MIB`. 둘 다 없으면 **선언을 요구하며 죽는다**.
BUDGET_OVERHEAD_ENV = "TRIAL_BUDGET_OVERHEAD_MIB"
# 자체검사 픽스처가 쓰는 값 — 옛 기본값과 같은 수이지만 **여기서만 산다**(프로덕션 경로는 선언을
# 요구한다). 픽스처 상수는 4종 안티패턴 판정표의 `매직넘버·정당` 칸이다(그 파일에서만 쓰는 국소 상수).
_FIXTURE_OVERHEAD_MIB = 12288  # antipattern-ok: G-B1-overhead-default — 자체검사 전용 상수(프로덕션 경로는 선언을 요구한다)


def require_overhead_mib(opts=None, env=None):
    """예산 overhead(MiB)를 **선언에서만** 읽는다. 부재는 fail-loud(조용한 기본값 ✗)."""
    env = os.environ if env is None else env
    for value, where in ((_opt(opts, "overhead_mib", None) if opts is not None else None,
                          "config candidate `overhead_mib`"),
                         (env.get(BUDGET_OVERHEAD_ENV), "env %s" % BUDGET_OVERHEAD_ENV)):
        if value in (None, ""):
            continue
        try:
            mib = int(value)
        except (TypeError, ValueError):
            raise SystemExit("[trial] FAIL: overhead 선언이 정수가 아니다(%s = %r)" % (where, value))
        if mib <= 0:
            raise SystemExit("[trial] FAIL: overhead 선언이 양수가 아니다(%s = %r)" % (where, value))
        return mib
    raise SystemExit(
        "[trial] FAIL: 예산 overhead 가 선언되지 않았다 — 기본값을 쓰지 않는다(2026-09-05 · G-B1).\n"
        "  → 왜: overhead 를 낮게 잡으면 선언 바닥이 높아져 워치독 arm 상한이 정상 서빙 위로 올라간다.\n"
        "     옛 기본값 12288 MiB 는 gpt-oss-120b/GB10 실측(17,971)보다 5,683 작았고 정상 로드를 죽였다.\n"
        "  → 어떻게: 후보 config 에 `overhead_mib: <n>` 을 적거나 %s=<n> 으로 넘겨라.\n"
        "     모르면 재라: 로드 완료 후 (MemTotal − MemAvailable) − weights − kv." % BUDGET_OVERHEAD_ENV)
def budget_ttl_floor_s():
    """TTL 하한을 **단일 소유자에게 물어본다**(2026-09-05 · G-B2 · 손기재 5사이트 정리)."""
    out = subprocess.run([sys.executable, os.path.normpath(BUDGET_SESSION_PY),
                          "--node-dir", ".", "budget-defaults", "--field", "ttl_s"],
                         capture_output=True, text=True)
    if out.returncode != 0 or not out.stdout.strip().isdigit():
        raise SystemExit("[trial] FAIL: 예산 TTL 기본값을 blackbox_session 에서 읽지 못했다 — "
                         "여기에 숫자를 다시 적지 않는다(rc=%s %s)"
                         % (out.returncode, (out.stderr or "").strip()[:200]))
    return int(out.stdout.strip())


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
    --kv-cache-memory-bytes / --max-num-seqs / --max-num-batched-tokens / --kv-cache-dtype /
    --gdn-prefill-backend / --enforce-eager / --language-model-only / --quantization /
    --tool-call-parser(+--enable-auto-tool-choice) / --reasoning-parser
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

    # ── max-num-batched-tokens (lock; KV 프로파일링 더미배치 상한) ────────
    # 미설정 시 vLLM 기본값이 max_model_len 에 연동돼(긴 ctx 모델일수록) 시동 시 KV 캐시
    # 프로파일링 더미 forward pass 가 커진다. GDN/hybrid-attention 계열(Qwen3.5/3.6)은 특히
    # mamba 캐시 정렬 제약으로 이 값을 명시해야 한다는 보고 다수 + DGX Spark 공식 레시피가
    # 명시값(8192) 사용 — 참조-그라운디드 확인 후 candidate 에 설정.
    if candidate.get("max_num_batched_tokens") is not None:
        args += ["--max-num-batched-tokens", str(int(candidate["max_num_batched_tokens"]))]

    # ── gpu-memory-utilization (디바이스 풀 상한 = safety_margin) ───────
    # 통합메모리(GB10)는 시스템이 일부 점유 → vLLM 기본 0.92 가 free 초과 OOM.
    # SKILL §5: gmu 는 풀 상한(=margin)으로만 emit; 실제 KV 는 절대 클램프가 제어.
    gmu = candidate.get("gpu_memory_utilization")
    if gmu is not None:
        args += ["--gpu-memory-utilization", str(float(gmu))]

    # ── gdn-prefill-backend (lock; GDN 커널 JIT 컴파일 백엔드 선택) ────────
    # FlashInfer GDN JIT 컴파일이 기본으로 전 CPU 코어를 동시 사용(코어당 ~3GB) → GB10 같은
    # 코어수 많은 호스트에서 RAM 폭주(vllm-project/vllm 커뮤니티 보고 — H100 에서도 재현,
    # RFC #39287 로 프로젝트가 인지 중). 검증된 우회책 = triton 백엔드(FlashInfer JIT 회피,
    # CUDA 그래프 정상 유지). 참조-그라운디드 확인 후 candidate 에 명시 설정.
    gdn_backend = candidate.get("gdn_prefill_backend")
    if gdn_backend:
        args += ["--gdn-prefill-backend", str(gdn_backend)]

    # ── enforce-eager (lock; CUDA 그래프 캡처 생략) ──────────────────────
    # GDN/hybrid-attention MoE 계열(Qwen3.5/3.6 등)의 CUDA 그래프 캡처 단계 메모리 폭증은
    # vLLM 상류 미해결 이슈(vllm-project/vllm#38486 — "cuda graph takes too much memory for
    # qwen 3.5", 유일 검증된 우회책 = enforce-eager). 참조-그라운디드 확인 후에만 candidate 에
    # 명시 설정할 것(carry-forward 금지 — 모델×아키텍처별 재확인, 헌법 §모델별 서빙전략 독립).
    if candidate.get("enforce_eager"):
        args += ["--enforce-eager"]

    # ── language-model-only (lock; 멀티모달 비전 인코더 완전 비활성화) ────
    # `--limit-mm-per-prompt` 를 전 모달리티 0 으로 설정하는 것과 동일(vLLM
    # MultiModalConfig.language_model_only). 비전 인코더 캐시/더미 프로파일링 자체를
    # 건너뛰어 그 단계의 메모리 사용을 제거 — 텍스트 전용 스모크에 한해 안전(모델이
    # VL 계열이라도 텍스트만 검증하면 비전 경로 자체가 불필요).
    if candidate.get("language_model_only"):
        args += ["--language-model-only"]

    # ── weight quantization (lock; none/null 은 미지정) ─────────────────
    quant = candidate.get("quantization")
    if quant and str(quant).lower() != "none":
        args += ["--quantization", str(quant)]

    # ── moe_backend (lock; auto/null 은 미지정 = 엔진 oracle 에 맡김) ───
    # ★ 이 노브는 **후보 스키마에 있는데 emit 되지 않고 있었다**(2026-08-01 발견).
    #   그 결과 `moe_backend: MARLIN` 을 줘도 조용히 무시되고 엔진이 auto 로 돌았다.
    #   이 프로젝트에서 moe-backend 는 결정적 레버였다 — DeepSeek-V4-Flash 는
    #   `humming` 이 6× OOM 을 풀었고 Qwen3-Next 는 `triton` 이 CUTLASS-JIT-OOM 을 풀었다.
    #   즉 "설정했는데 안 먹는" 것이 가장 비싼 종류의 침묵 실패다.
    #   플래그명 version-exact 확인: vllm/engine/arg_utils.py:1523 `--moe-backend`.
    moe = candidate.get("moe_backend")
    if moe and str(moe).lower() != "auto":
        args += ["--moe-backend", str(moe)]

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
                      tiktoken_host_path: "str | None" = None,
                      jit_cache_root: "str | None" = None,
                      max_jobs: "int | None" = None,
                      nas_container_root: "str | None" = None) -> list:
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
        # OOM킬러 우선희생 지정(plan_26071019 §2.4) — 사고 #5 에서 커널이 wireplumber 만
        # 죽이고 70GiB 진범을 못 잡은 오발 교정. 커널이 트라이얼 컨테이너를 먼저 잡게 한다.
        "--oom-score-adj", "800",
        "-p", "%d:%d" % (int(port), DEFAULT_PORT),
        # ★ 컨테이너 마운트 경로는 config.nas_container_root 를 따른다(기본 /app/models).
        #   예전엔 CONTAINER_MODELS 상수로 **하드코딩**돼 있어, quant_model 2차 NAS 를 쓰는
        #   모델(config 가 /app/quant_models 를 선언)에서 트라이얼만 /app/models 에 마운트했다.
        #   그러면 candidate.model_path_container 가 컨테이너 안에 실재하지 않아 HF 가 그 경로를
        #   repo id 로 오해한다 — `HFValidationError: Repo id must be in the form ...`
        #   (2026-08-01 Qwen3.5-122B-NVFP4 실측). 더 중요한 건 **트라이얼과 서빙의 컨테이너 경로가
        #   갈린다**는 점이다 — 검증한 것과 배포되는 것이 달라진다.
        "-v", "%s:%s:ro" % (nas_mount, nas_container_root or CONTAINER_MODELS),
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

    # ── JIT/컴파일 캐시 영속 + 컴파일 팬아웃 캡 ────────────────────────
    # ★ serve 평면(docker-compose.yaml)은 ./cache/{vllm,flashinfer} 를 마운트하고
    #   "cold JIT 은 호스트 하드다운 리스크를 매번 새로 진다(uncapped nvcc 팬아웃)"고 명시해 뒀다.
    #   그런데 trial 평면엔 그 조치가 없었다 — **미검증 설정을 돌리는 더 위험한 쪽**이 무방비였다.
    #   실측(2026-08-01 GLM-4.7-Flash S6): 로드 후 안정(41 GiB)했다가 torch.compile 뒤
    #   2.5분에 걸쳐 12,880 MiB 까지 지속 하강 후 사망. 같은 설정의 serve 런은 38.5 GiB 평탄.
    #   유일한 차이가 이 마운트였다.
    # MAX_JOBS: 팬아웃은 기본 nproc(이 호스트 20)까지 벌어진다. gmu 도 KV 클램프도 이 구간에
    #   닿지 않는다(Laguna FP4 lazy JIT 선례에서 확인 — MAX_JOBS 가 유일한 노브).
    if jit_cache_root:
        for sub in ("vllm", "flashinfer"):
            host_dir = os.path.join(jit_cache_root, sub)
            os.makedirs(host_dir, exist_ok=True)
            cmd += ["-v", "%s:/root/.cache/%s" % (host_dir, sub)]
    if max_jobs:
        cmd += ["-e", "MAX_JOBS=%d" % int(max_jobs)]

    # ── VLLM_ATTENTION_BACKEND env (soft) ──────────────────────────────
    backend = candidate.get("attention_backend")
    if backend:
        cmd += ["-e", "VLLM_ATTENTION_BACKEND=%s" % str(backend)]

    # ── extra_env (진단 전용 — 임의 env var 통과) ──────────────────────
    # 정식 candidate 스키마 필드가 아님(감사 대상 아님) — 실패지점 함수단위 추적
    # 등 ad-hoc 디버깅(VLLM_LOGGING_LEVEL=DEBUG·VLLM_TRACE_FUNCTION=1 등)에 한정 사용.
    for k, v in (candidate.get("extra_env") or {}).items():
        cmd += ["-e", "%s=%s" % (str(k), str(v))]

    # ── serve_env (선언된 서빙 env — extra_env 와 다르다) ────────────────
    # extra_env 는 진단 전용이라 배포 3종 세트로 승격되지 않는다. 그런데 커널 스위치
    # (VLLM_USE_FLASHINFER_MOE_MXFP4_BF16 등)는 **레시피의 일부**다 — 그것이 있고 없고가
    # 서빙 성능을 가른다. 승격 경로가 없어서 직전 캠페인은 생성된 .sh 를 손으로 고쳐
    # 스위치를 넣었고(output/multi/configs/*-b6.sh:16), 그 손질은 선언에서 재현되지 않으며
    # 재생성이 덮어쓴다 — 그림자 배달 경로다. 여기와 gen_recipe_set 양쪽에 자리를 만든다.
    # 트라이얼에도 거는 이유: 트라이얼이 KV 클램프를 수렴시키는데, 커널이 바뀌면 메모리
    # 발자국도 바뀐다. 배포될 것과 다른 조건에서 수렴시키면 그 수렴값이 거짓이다.
    for k, v in serve_env_pairs(candidate.get("serve_env")):
        cmd += ["-e", "%s=%s" % (k, v)]

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
        ("max_num_batched_tokens", "--max-num-batched-tokens"),
        ("kv_cache_memory_bytes", "--kv-cache-memory-bytes"),
        ("kv_cache_quant", "--kv-cache-dtype"),
        ("attention_backend", "VLLM_ATTENTION_BACKEND="),
        ("moe_backend", "--moe-backend"),
    ]
    # serve_env: 선언했으면 **이름이 cmd 에 실제로 있어야** 한다. 축을 선언만 하고 안 거는
    # 사고가 이 클래스의 본체다(2026-09-05: 다섯 셀이 선언과 다른 커널로 돌았다).
    for _k, _v in serve_env_pairs(candidate.get("serve_env")):
        field_flags.append(("serve_env", "%s=" % _k))
    # enforce_eager: false/미설정은 의도적 미emit(CUDA 그래프 기본 활성 유지).
    if candidate.get("enforce_eager"):
        field_flags.append(("enforce_eager", "--enforce-eager"))
    # language_model_only: false/미설정은 의도적 미emit(멀티모달 기본 활성 유지).
    if candidate.get("language_model_only"):
        field_flags.append(("language_model_only", "--language-model-only"))
    # gdn_prefill_backend: 미설정은 의도적 미emit(vLLM 기본 선택 유지).
    if candidate.get("gdn_prefill_backend"):
        field_flags.append(("gdn_prefill_backend", "--gdn-prefill-backend"))
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


def _container_state(container_name: str) -> dict:
    """컨테이너 생존 판정 — `docker inspect` 1회로 (state, exit_code, oom_killed) 를 얻는다.

    반환 state: `running` | `dead` | `absent` | `unknown`.
      running : 아직 살아 있다(=미준비일 뿐).
      dead    : `State.Running=false` — 죽었다(워치독 SIGKILL·엔진 크래시·cgroup OOM).
      absent  : 컨테이너 오브젝트 자체가 없다(누가 rm 했다) — 죽음과 동치로 다룬다.
      unknown : docker CLI/데몬 조회 실패. **죽음으로 취급하지 않는다**(아래 이유).

    ★ `unknown` 을 죽음으로 읽으면 안 된다 — 일시적 조회 실패로 정상 로드를 중단시키는 것은
      이 프로젝트가 이미 값을 치른 위양성 클래스다(ETA 워치독이 정상 로드 3/3 을 사살한
      2026-08-01 · `docs/logs` envelope 처방 참조). **확정 신호에만 fail-closed** 하고
      조회 실패는 로그로 남긴 뒤 폴링을 계속한다(침묵 금지).
    """
    try:
        proc = subprocess.run(
            ["docker", "inspect", "-f",
             "{{.State.Running}} {{.State.ExitCode}} {{.State.OOMKilled}}", container_name],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=20,
        )
    except Exception as exc:  # noqa: BLE001 — CLI 부재/타임아웃
        return {"state": "unknown", "detail": "%s: %s" % (type(exc).__name__, exc)}
    return parse_inspect_state(
        proc.returncode,
        proc.stdout.decode("utf-8", errors="replace"),
        proc.stderr.decode("utf-8", errors="replace"),
    )


def parse_inspect_state(returncode: int, stdout: str, stderr: str) -> dict:
    """`docker inspect` 결과 → 생존 판정. **순수 함수**(IO 없음 → `--self-test` 가 이걸 친다)."""
    out = (stdout or "").strip()
    err = (stderr or "").strip()
    if returncode != 0:
        low = err.lower()
        if "no such object" in low or "no such container" in low:
            return {"state": "absent", "detail": err[:200]}
        return {"state": "unknown", "detail": err[:200] or "rc=%d" % returncode}
    parts = out.split()
    if not parts:
        return {"state": "unknown", "detail": "빈 inspect 출력"}
    running = parts[0].lower()
    info = {"detail": out}
    if len(parts) >= 2:
        info["exit_code"] = parts[1]
    if len(parts) >= 3:
        info["oom_killed"] = parts[2].lower() == "true"
    if running == "true":
        info["state"] = "running"
    elif running == "false":
        info["state"] = "dead"
    else:
        info["state"] = "unknown"
    return info


def _poll_health(port: int, timeout: float, container_name: "str | None" = None) -> dict:
    """/health 가 HTTP 200 을 줄 때까지 폴링하되, **컨테이너가 죽으면 즉시 이탈**한다.

    준비판정 = :PORT/health http200 (로그 "startup complete" grep 금지 — 거짓양성).

    ★ 반환은 bool 이 아니라 **dict** 다. `if _poll_health(...)` 로 쓰면 항상 참이 된다 —
      호출부는 `["healthy"]` 를 읽어라(자체검사 C5 가 이 계약을 지킨다).
      keys: healthy(bool) · outcome(str) · waited_s(float) · polls(int) ·
            container_state(str|None) · container_detail(str|None) · unknown_polls(int).
      outcome ∈ `healthy` | `container_died` | `timeout`.

    왜 생존검사가 필요한가(plan_26082223 결함 C · 2026-08-22 실측):
      호스트 mem-watchdog 이 컨테이너를 `docker kill` 해도 이 루프는 그 사실을 몰라
      `--timeout 1800`(30분)을 **전량 소진**했다. 런당 최대 ~26분, 캠페인 누적 ~3.5시간.
      `/health` 연결거부는 "아직 미준비"와 "죽었다"를 구분하지 못한다 — 구분자는
      `docker inspect` 뿐이다. `engine_liveness_watchdog.sh` 는 휴면(미배선)이라 이 갭을
      못 막는다. **사망은 미준비가 아니라 종단 상태**이므로 즉시 이탈한다.

    `container_name` 미주입이면 생존검사를 건너뛴다(하위 호환 — 기존 동작 그대로).
    """
    url = _health_url(port)
    started = time.monotonic()
    deadline = started + float(timeout)
    polls = 0
    unknown_polls = 0
    last_state = None
    last_detail = None

    def _done(healthy, outcome):
        return {
            "healthy": bool(healthy),
            "outcome": outcome,
            "waited_s": round(time.monotonic() - started, 3),
            "polls": polls,
            "container_state": last_state,
            "container_detail": last_detail,
            "unknown_polls": unknown_polls,
        }

    while time.monotonic() < deadline:
        polls += 1
        try:
            with urllib.request.urlopen(url, timeout=5.0) as resp:
                if resp.status == 200:
                    return _done(True, "healthy")
        except urllib.error.HTTPError as e:
            if e.code == 200:
                return _done(True, "healthy")
        except Exception:  # noqa: BLE001 — 연결 거부/타임아웃은 아직 미준비
            pass

        # 미준비다. 그러면 **아직 살아 있기는 한가**를 본다.
        if container_name:
            info = _container_state(container_name)
            last_state = info.get("state")
            last_detail = info.get("detail")
            if last_state in ("dead", "absent"):
                print("[run_trial] 컨테이너 사망 감지(%s) — /health 폴링 즉시 중단"
                      " (경과 %.1fs / 타임아웃 %.0fs). detail=%s"
                      % (last_state, time.monotonic() - started, float(timeout), last_detail),
                      file=sys.stderr)
                return _done(False, "container_died")
            if last_state == "unknown":
                unknown_polls += 1
                # 침묵 금지 — 단 매 폴마다 찍으면 30분 폴링이 수백 줄 잡음이 된다.
                # **첫 회는 반드시** 찍고 이후는 감속한다(횟수는 반환 dict 에 전량 남는다).
                if unknown_polls == 1 or unknown_polls % 20 == 0:
                    print("[run_trial] ⚠ 컨테이너 생존조회 실패(%d회째) — 폴링 계속(위양성 방지). %s"
                          % (unknown_polls, last_detail), file=sys.stderr)

        time.sleep(HEALTH_POLL_INTERVAL)
    return _done(False, "timeout")


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


def _read_meminfo_mib(key: str) -> "int | None":
    """/proc/meminfo 의 한 항목을 MiB 로 읽는다. 실패 시 None(호출부가 fail-closed 처리)."""
    try:
        with open("/proc/meminfo", encoding="utf-8") as fh:
            for line in fh:
                if line.startswith(key + ":"):
                    return int(line.split()[1]) // 1024
    except (OSError, IndexError, ValueError):
        return None
    return None


def host_floor_gmu_cap(requested_gmu, mem_total_mib, mem_available_mib, floor_mib,
                       weights_mib=None, overhead_mib=None,
                       headroom_mib=HOST_FLOOR_HEADROOM_MIB,
                       min_kv_mib=MIN_MEASURE_KV_MIB) -> dict:
    """측정 트라이얼의 gmu 를 **호스트 안전바닥 준수 상한**으로 캡한다. 순수 함수(IO 없음).

    풀려는 문제(plan_26082223 결함 B · 2026-08-22 실측): 통합메모리 호스트에서 클램프 미설정
    측정 트라이얼은 `gmu 0.90 × 121.69 GiB = 109.5 GiB` 를 전부 잡고, 그러면 MemAvailable 은
    필연적으로 8 GiB 대로 수렴해 **고정 10 GiB 바닥 아래**로 떨어진다 → 협역 워치독이 **항상**
    사살한다(모델 크기와 무관한 구조적 상호배타).

    처방의 방향이 중요하다 — **바닥을 낮추지 않고 요구를 깎는다**
    (`policy:HOST_SAFETY_LAYERED_DEFENSE`). 바닥을 낮추면 호스트 하드다운 방어가 얇아진다.

    ⚠ plan §4.3 의 리터럴 식 `(MemTotal − floor − weights − overhead)/MemTotal` 은 **KV 몫의
      비율**이지 gmu 상한이 아니다. vLLM 의 `--gpu-memory-utilization` 은 weights·activation·KV 를
      **전부 포함한** 총 상한 비율이므로, 그 식을 그대로 쓰면 이 호스트에서 0.39 가 나와
      가중치(51 GiB)조차 못 올린다 — 회복 가능한 사살을 **회복 불가한 하드 실패**로 바꾼다.
      그래서 여기서는 같은 의도를 총 상한 축으로 옮겨 적는다:
          `cap = (MemAvailable − floor − headroom) / MemTotal`
      (MemAvailable 을 쓰는 이유: 워치독이 비교하는 값이 바로 그것이고, 호스트 상주분을 추정이
       아니라 실측으로 반영한다.)

    반환 dict: gmu(적용값) · applied(bool) · reason(str) · cap(float|None) ·
               requested(float) · 입력 스냅샷. **출처 표시**(§결정론 규율)로 하류가 이 값이
               요청값인지 캡값인지 구분할 수 있게 한다.
    """
    info = {
        "gmu": requested_gmu,
        "applied": False,
        "reason": "",
        "cap": None,
        "requested": requested_gmu,
        "mem_total_mib": mem_total_mib,
        "mem_available_mib": mem_available_mib,
        "floor_mib": floor_mib,
        "headroom_mib": headroom_mib,
        "weights_mib": weights_mib,
        "overhead_mib": overhead_mib,
    }
    if requested_gmu is None:
        info["reason"] = "gmu_unset"
        return info
    if not mem_total_mib or not mem_available_mib or mem_total_mib <= 0:
        info["reason"] = "meminfo_unavailable"
        return info

    cap = (int(mem_available_mib) - int(floor_mib) - int(headroom_mib)) / float(mem_total_mib)
    # 4자리 내림 — 올림하면 캡이 제 목적(바닥 준수)을 어긴다.
    cap = int(cap * 10000) / 10000.0
    info["cap"] = cap

    if cap >= float(requested_gmu):
        info["reason"] = "not_needed"
        return info
    if cap <= 0:
        info["reason"] = "cap_nonpositive"
        return info
    if weights_mib is None:
        # 가중치를 모르면 캡이 가중치를 굶기는지 **증명할 수 없다** → 캡을 걸지 않는다.
        # (조용한 생략이 아니다 — reason 이 산출물에 남는다.)
        info["reason"] = "weights_unknown"
        return info
    allowed_mib = cap * float(mem_total_mib)
    need_mib = int(weights_mib) + int(overhead_mib or 0) + int(min_kv_mib)
    if allowed_mib < need_mib:
        # 캡을 걸면 weights+overhead+최소KV 도 못 들어간다 → 사살(회복 가능)보다 나쁘다.
        info["reason"] = "cap_infeasible(allowed=%dMiB < need=%dMiB)" % (allowed_mib, need_mib)
        return info

    info["gmu"] = cap
    info["applied"] = True
    info["reason"] = "capped_to_host_floor"
    return info


def _memwatch_script_path():
    """Resolve the owner-local canonical source, then the rendered sub runtime asset."""
    return _node_tool_path("host_safety", "mem_watchdog.sh")


def _start_memwatch(container_name: str, simlog_dir: str, trial_number: int,
                    thresh_mib: int = HOST_FLOOR_MIB):
    """트라이얼 협역 워치독 사이드 기동(plan_26071019 §2.3 — 계층 방어 2층).

    systemd 상시(광역) 인스턴스와 병행(임계 동급·필터 협역 — 로그가 simlog 에 남아
    trial 증거로 편입). 스크립트 부재/기동 실패 시 경고 후 (None, None) — fail-open,
    상시층이 최후 커버(δ2-1 "미기동이 유일한 실패 원인" 교훈의 역: 기동을 코드가 보장).
    반환 (Popen|None, 로그파일핸들|None).
    """
    script = _memwatch_script_path()
    if not script:
        print("[run_trial] ⚠ host-safety mem_watchdog asset 부재 — 협역 워치독 생략(상시 systemd 층만)",
              file=sys.stderr)
        return None, None
    log_p = os.path.join(simlog_dir, "trial%02d_memwatch.log" % int(trial_number))
    fh = open(log_p, "w", encoding="utf-8")
    try:
        proc = subprocess.Popen(
            ["bash", script, container_name, str(int(thresh_mib)), "2"],
            stdout=fh, stderr=subprocess.STDOUT,
        )
        return proc, fh
    except Exception as e:  # noqa: BLE001
        print("[run_trial] ⚠ 협역 워치독 기동 실패(%s) — 상시 systemd 층만" % e, file=sys.stderr)
        try:
            fh.close()
        except Exception:  # noqa: BLE001
            pass
        return None, None


def _stop_memwatch(proc, fh) -> None:
    """PID(핸들) 기반 정지 — **pkill -f 금지**(자기참조 매칭 부모셸 사망 exit144 선례,
    devlog_26062718). 예외 흡수."""
    try:
        if proc is not None:
            proc.terminate()
            proc.wait(timeout=5)
    except Exception:  # noqa: BLE001
        try:
            proc.kill()
        except Exception:  # noqa: BLE001
            pass
    try:
        if fh is not None:
            fh.close()
    except Exception:  # noqa: BLE001
        pass


def _now_iso() -> str:
    """벽시계 금지 규약의 예외가 아니다 — blackbox_session 이 `--now` 를 **요구**하므로 여기서
    한 번만 만들어 넘긴다(스크립트 안에서 시각을 *판정*에 쓰지는 않는다)."""
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def _budget_session(node_dir: str, *args: str) -> "tuple[bool, str]":
    """blackbox_session 서브커맨드 1회 실행 → (성공, 출력)."""
    cmd = [sys.executable, os.path.normpath(BUDGET_SESSION_PY), "--node-dir", node_dir, *args]
    try:
        proc = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=60)
    except (OSError, subprocess.TimeoutExpired) as exc:
        return False, "%s: %s" % (type(exc).__name__, exc)
    return proc.returncode == 0, proc.stdout.decode("utf-8", errors="replace").strip()


def _budget_node_dir(opts) -> "str | None":
    """선언을 기록할 노드 디렉터리. 명시 > **해소기 호출** > None(생략 · fail-loud 로그).

    **각자 파싱하지 않는다**(`terraforming_node` SKILL.md §2.7.6 · 헌법 노드제어). node_id 의 단일
    해소기는 `node_blackbox/node_identity.sh` 이고, 이 함수는 그 CLI(`--resolve`)를 부른다.

    ★ 2026-09-07 교정(plan_26090715 §4.9 · 결함 ⑧). 종전 구현은 manifest 를 여기서 직접 열어
      로스터 항목의 role 이 main 이 아니면 건너뛰고 main 을 반환했다. 서브 manifest 에도 **로스터**
      `nodes: [- role: main, - role: sub]` 가 있으므로 서브에서도 `main` 이 나왔고, 서브의 예산
      선언이 `docs/logs/main/` 으로 갔다 — 서브 `mem_watchdog_eta` 가 자기 선언을 못 읽어
      **무보호 로드**가 됐다(2026-09-05 부터, 회수 미러의 `logs/main`·`logs/sub` 동시 기록이 증거).
      해소기는 **`self_role` 을 로스터보다 먼저** 본다. 그것이 "이 파일이 놓인 노드가 누구인가" 의 답이다.
    """
    explicit = _opt(opts, "budget_node_dir", None)
    if explicit:
        return str(explicit)
    repo = _repo_root()
    resolver = _node_tool_path("node_blackbox", "node_identity.sh")
    if not resolver:
        print("[budget] node_id 해소기 부재 — %s(정본) · .claude/runtime/node_blackbox(서브 배달분) "
              "어디에도 없다. 선언을 생략한다(추측하지 않는다)."
              % _canonical_node_tool_path("node_blackbox", "node_identity.sh"), file=sys.stderr)
        return None
    try:
        cp = subprocess.run(["bash", resolver, "--resolve", "--repo", repo],
                            capture_output=True, text=True, timeout=60, check=False)
    except (OSError, subprocess.SubprocessError) as exc:
        print("[budget] node_id 해소기 실행 실패(%s) — 선언을 생략한다." % exc, file=sys.stderr)
        return None
    if cp.returncode != 0:
        # 해소기의 fail-loud 사유를 그대로 올린다(삼키면 침묵 폴백이 된다).
        sys.stderr.write(cp.stderr or "")
        print("[budget] node_id 미해소(rc=%d) — 선언을 생략한다." % cp.returncode, file=sys.stderr)
        return None
    node_id = (cp.stdout or "").strip()
    # 해소기가 이미 스킴을 강제하지만, 경로 성분이 되므로 소비 지점에서도 다시 본다(경계 검증).
    if not re.match(r"^[a-z][a-z0-9-]{0,31}$", node_id):
        print("[budget] 해소기가 낸 node_id 가 스킴 위반이다(%r) — 선언을 생략한다." % node_id,
              file=sys.stderr)
        return None
    return os.path.join(repo, "docs", "logs", node_id)


def _budget_inputs(candidate: dict, opts) -> "tuple[dict | None, str]":
    """선언 입력을 **파생**한다(손으로 적지 않는다 · plan_26081415 C3-2).

    반환 (inputs, skip_reason). inputs 가 None 이면 skip_reason 이 사유다.

    ⚠ **`model_host_path`·`tp` 는 candidate 가 아니라 opts 에서만 읽는다.** candidate 에서 읽으면
      `gen_recipe_set.assert_serve_knob_parity` 의 계약 — *"run_trial 이 candidate 에서 읽는 필드는
      3종 세트까지 도달해야 한다"* — 에 걸린다. 예산 선언 입력은 **serve 노브가 아니므로** 3종 세트에
      도달할 이유가 없고, 그렇다고 면제 목록을 늘리면 그 tripwire 가 둔해진다. 애초에 평면을 섞지
      않는 것이 옳다(2026-08-16 실측으로 발각 — 초판은 candidate 폴백을 뒀다가 파리티를 깼다).
      `kv_cache_memory_bytes` 만은 candidate 가 권위다 — 그건 실제 serve 노브이고 이미 3종 세트에
      도달한다(파리티 통과 필드).
    """
    kv_bytes = candidate.get("kv_cache_memory_bytes")
    if not kv_bytes:
        # 설계 의도: KV 절대클램프 미선언이면 선언 자체가 불가하다(blackbox_session --kv-mib).
        return None, "kv_clamp_absent"
    try:
        mem_total_mib = int(
            [l for l in open("/proc/meminfo", encoding="utf-8") if l.startswith("MemTotal:")][0]
            .split()[1]
        ) // 1024
    except (OSError, IndexError, ValueError):
        return None, "mem_total_unavailable"

    # 체크포인트 바이트: 호출부가 **이미 가진 값**을 우선 받는다. recipe.py 는 같은 값을
    # `preload_ram_gate(parsed["native_weight_bytes"], tp=tp)` 로 로드-전 게이트에 쓰고 있으므로,
    # 예산 선언이 같은 값을 쓰면 두 게이트가 **같은 축**을 보게 된다. 파일시스템을 다시 뒤지면
    # 같은 사실의 두 번째 출처가 생기고, 둘이 갈리면 어느 쪽이 맞는지 알 수 없다.
    ckpt_bytes = _opt(opts, "checkpoint_bytes", None)
    if not ckpt_bytes:
        # CLI 단독 사용 경로 — 호스트 경로에서 직접 산출한다(같은 산출기를 쓴다).
        host_path = _opt(opts, "model_host_path", None)
        if not host_path or not os.path.isdir(str(host_path)):
            return None, "model_host_path_absent"
        try:
            import preload_ram_gate  # noqa: PLC0415 — 같은 scripts/ 디렉터리(경로는 위에서 보장)
            ckpt_bytes = preload_ram_gate.checkpoint_bytes_for(str(host_path))
        except Exception:
            return None, "checkpoint_size_unavailable"
    if not ckpt_bytes or int(ckpt_bytes) <= 0:
        # preload_ram_gate 와 동일 판단: 크기 미상은 거짓 차단이 아니라 생략 사유다.
        return None, "checkpoint_size_unavailable"

    # tp 는 manifest 권위다(recipe.py resolve_tp: config override > manifest > 1). 여기서 candidate 를
    # 뒤지지 않고 호출부가 정한 값을 받는다 — 추측하면 항상 1 로 떨어져 weights 를 과대평가한다.
    try:
        tp = int(_opt(opts, "tp", 1) or 1) or 1
    except (TypeError, ValueError):
        tp = 1
    # overhead 는 선언에서만 온다(부재 = fail-loud). 조용한 기본값이 무장 밴드를 넓힌다.
    overhead_mib = require_overhead_mib(opts)
    return {
        "mem_total_mib": mem_total_mib,
        "weights_mib": -(-int(ckpt_bytes) // tp // (1024 * 1024)),   # ceil-div (게이트와 같은 축)
        "kv_mib": -(-int(kv_bytes) // (1024 * 1024)),
        "overhead_mib": overhead_mib,
    }, ""


def _budget_declare(node_dir: str, inputs: dict, label: str, expected_load_s: float) -> bool:
    """로드 개시 **전** 선언. 순서가 판정 기준이다(사후 발행은 무의미 · C3 성공기준1)."""
    ttl_s = max(budget_ttl_floor_s(), int(expected_load_s) * 3)
    ok, out = _budget_session(
        node_dir, "declare-budget",
        "--mem-total-mib", str(inputs["mem_total_mib"]),
        "--weights-mib", str(inputs["weights_mib"]),
        "--kv-mib", str(inputs["kv_mib"]),
        "--overhead-mib", str(inputs["overhead_mib"]),
        "--ttl-s", str(ttl_s),
        "--expected-load-s", str(int(expected_load_s)),
        "--label", label,
        "--now", _now_iso(),
    )
    print("[trial] 예산 선언: %s" % (out or ("ok" if ok else "실패")), file=sys.stderr)
    return ok


def _budget_skip(node_dir: "str | None", reason: str, label: str) -> None:
    """무보호 진입을 **기록**한다. 침묵 금지 — 생략과 '원래 없었음'은 구분돼야 한다."""
    print("[trial] ⚠ 예산 선언 생략(무보호 진입): %s" % reason, file=sys.stderr)
    if node_dir:
        _budget_session(node_dir, "budget-skip", "--reason", reason,
                        "--label", label, "--now", _now_iso())


def _budget_clear(node_dir: str) -> None:
    """서빙을 내렸으면 선언도 내린다 — 남기면 다음 로드가 **남의 바닥**으로 무장한다."""
    ok, out = _budget_session(node_dir, "clear-budget", "--now", _now_iso())
    print("[trial] 예산 회수: %s" % (out or ("ok" if ok else "실패")), file=sys.stderr)


def _drop_caches_best_effort() -> None:
    """teardown 후 페이지캐시 드랍(plan_26071019 §4.1 자동 지점 ②).

    GB10 통합메모리는 페이지캐시가 CUDA 와 물리풀 경쟁 — 다음 로드의 MemAvailable 을
    미리 회복. sudoers 단일 헬퍼(install_host_safety.sh)가 설치된 경우에만, 실패 무해.
    """
    helper = "/usr/local/sbin/vllm-drop-caches"
    if not os.path.exists(helper):
        return
    try:
        subprocess.run(["sudo", "-n", helper], stdout=subprocess.DEVNULL,
                       stderr=subprocess.DEVNULL, timeout=30)
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
    provenance: str,
) -> dict:
    """--dry-run/mock_profile 모드 결과. docker 없이 주어진 값으로 반환.

    load_ok 는 vllm_profile 이 존재(=비-None)하면 True 로 본다.

    ★ provenance 필수(2026-08-13 · plan_26081314 D1): 이 함수의 산출물은 **실측이 아니다**.
      예전엔 실측 경로와 dict 모양이 완전히 같아 하류(simlog·판정·인증서)에서 구분이 불가능했고,
      그것은 헌법의 `합성 금지`·`측정 > 공식` 경계를 지우는 통로였다. 근거 규율은 CoC 가 성립하는
      이유와 같다 — 인터프리터가 실행한 값과 에뮬레이트한 값은 상태에서 구분되어야 한다.
    """
    if provenance == PROVENANCE_MEASURED:
        raise ValueError("mock result must never claim measured provenance")
    load_ok = vllm_profile is not None
    return {
        "trial_number": int(trial_number),
        "candidate": candidate,
        "load_ok": bool(load_ok),
        "vllm_profile": vllm_profile,
        "functional": functional,
        "log_path": log_path,
        "error_excerpt": None,
        "provenance": provenance,
        # 스키마 균일성: mock/dry-run 은 준비 대기를 하지 않는다(None ≠ timeout).
        "health_wait": None,
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
        served_model_name(스모크용; 없으면 candidate.served_model_name → model_id → 모델 경로),
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

    # served_model_name 결정: opts → candidate.served_model_name → candidate.model_id → 모델 경로.
    #   ⚠ `candidate.served_model_name` 이 이 체인에 **없었다**(2026-08-16 실측 발각).
    #   `_build_serve_args` 는 그 값을 `--served-model-name` 으로 넣어 vLLM 을 그 이름으로 띄우는데,
    #   스모크는 여기서 `model_path_container` 로 떨어져 **다른 이름을 조회**했다 → 항상 http_404
    #   (`The model '<path>' does not exist`). 서빙은 성공(load_ok=True)인데 스모크만 실패하므로
    #   "모델이 안 떴다"로 오독하기 쉽다. serve 가 쓰는 값을 스모크도 쓰게 해 두 평면을 일치시킨다.
    served_model_name = (
        _opt(opts, "served_model_name", None)
        or candidate.get("served_model_name")
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
            candidate, trial_number, vllm_profile, functional, log_path,
            PROVENANCE_MOCK if mock_profile_path else PROVENANCE_DRY_RUN,
        )

    # ── 실 DOCKER 경로 ─────────────────────────────────────────────────
    # 이전 잔류 컨테이너 제거(이름 충돌·통합메모리 잔류 방지).
    _docker_teardown(container_name)

    nas_mount = _opt(opts, "nas_mount", NAS_MOUNT)  # config/manifest nas_host_root 배선
    tiktoken_host_path = _opt(opts, "tiktoken_host_path", None)  # config.tiktoken_host_path(C8)
    # JIT 캐시 통로·컴파일 팬아웃 캡(serve 평면과 parity — 위 _build_docker_cmd 주석 참조).
    # 기본 4: 이 호스트 nproc 20 을 그대로 쓰면 nvcc 팬아웃이 수십 GiB 를 먹는다.
    jit_cache_root = _opt(opts, "jit_cache_root", None)
    max_jobs = _opt(opts, "max_jobs", 4)

    # ── 측정 트라이얼 gmu 캡 (plan_26082223 결함 B) ─────────────────────────────────
    #   왜 여기만: 절대 KV 클램프가 **설정된** 트라이얼은 vLLM 사용량이 클램프로 이미 유계라
    #   gmu 는 풀 상한일 뿐이고, 그걸 깎으면 검증하려던 클램프가 거짓 기각된다. 사살이
    #   구조적으로 항상 일어나는 쪽은 **클램프 미설정(측정) 트라이얼**이다.
    #   왜 unified 게이트: 캡의 근거는 "엔진 할당이 MemAvailable 을 직접 끌어내린다"이며 이는
    #   통합메모리에서만 참이다. discrete GPU 에서 호스트 RAM 기준으로 gmu 를 깎으면 근거 없는
    #   축소다. 통합 여부는 호출부(recipe)가 device_total↔MemTotal 파생으로 판정해 넘긴다.
    memwatch_thresh_mib = int(_opt(opts, "memwatch_thresh_mib", HOST_FLOOR_MIB))
    gmu_cap_info = None
    effective_gmu = candidate.get("gpu_memory_utilization")
    if candidate.get("kv_cache_memory_bytes") is None and _opt(opts, "unified_memory", None) is True:
        _ck = _opt(opts, "checkpoint_bytes", None)
        try:
            _tp = int(_opt(opts, "tp", 1) or 1) or 1
        except (TypeError, ValueError):
            _tp = 1
        _w_mib = -(-int(_ck) // _tp // (1024 * 1024)) if _ck else None
        gmu_cap_info = host_floor_gmu_cap(
            effective_gmu,
            _read_meminfo_mib("MemTotal"),
            _read_meminfo_mib("MemAvailable"),
            memwatch_thresh_mib,
            weights_mib=_w_mib,
            overhead_mib=require_overhead_mib(opts),
        )
        if gmu_cap_info["applied"]:
            print("[trial] gmu 캡 적용: %.4f → %.4f (호스트 바닥 %dMiB + 여유 %dMiB 준수, "
                  "MemAvailable=%sMiB/MemTotal=%sMiB). 측정 트라이얼의 구조적 사살 회피 — "
                  "바닥을 낮춘 것이 아니라 요구를 깎았다."
                  % (gmu_cap_info["requested"], gmu_cap_info["gmu"], memwatch_thresh_mib,
                     HOST_FLOOR_HEADROOM_MIB, gmu_cap_info["mem_available_mib"],
                     gmu_cap_info["mem_total_mib"]), file=sys.stderr)
            # 원본 candidate 는 건드리지 않는다(호출부가 최종 레시피로 쓰는 객체다).
            candidate = dict(candidate, gpu_memory_utilization=gmu_cap_info["gmu"])
            effective_gmu = gmu_cap_info["gmu"]
        else:
            print("[trial] gmu 캡 미적용(%s) — 요청값 %s 그대로 진입. 침묵 금지: 이 사유가 "
                  "trial 산출물 gmu_cap.reason 에 남는다."
                  % (gmu_cap_info["reason"], gmu_cap_info["requested"]), file=sys.stderr)

    docker_cmd = _build_docker_cmd(
        candidate, image, container_name, port, nas_mount,
        tiktoken_host_path=tiktoken_host_path,
        jit_cache_root=jit_cache_root, max_jobs=max_jobs,
        nas_container_root=_opt(opts, "nas_container_root", None),
    )
    # emit-audit: candidate 의 serve-관련 비-null 필드가 실제 cmd 에 반영됐는지 전수 점검
    # (gmu 미emit 회귀류 클래스 차단 — §9.3). 누락 시 즉시 raise.
    _audit_emitted(candidate, docker_cmd)

    # 협역 워치독(계층 2층·§2.3) — docker run *전* 기동해 가중치 로드 구간부터 커버.
    # 사고 #5 는 로드 시작 직후 폭주였음(14:56 로드 → 압박). 로그 = trialNN_memwatch.log.
    wd_proc, wd_fh = _start_memwatch(
        container_name, simlog_dir, trial_number,
        thresh_mib=memwatch_thresh_mib,   # gmu 캡과 **같은 바닥**을 본다(단일 소유).
    )

    # 서빙 예산 선언 — 워치독 기동 **직후 · 로드 개시 전**(multi 스모크와 같은 순서:
    # declare → honored → up). 상세 근거·실패정책은 파일 상단 BUDGET_* 주석.
    budget_label = "trial%02d-%s" % (int(trial_number), str(served_model_name).split("/")[-1])
    budget_node_dir = None if bool(_opt(opts, "no_budget", False)) else _budget_node_dir(opts)
    budget_declared = False
    if bool(_opt(opts, "no_budget", False)):
        _budget_skip(None, "no_budget_flag", budget_label)
    elif budget_node_dir is None:
        # 노드 디렉터리를 못 정하면 이벤트를 남길 곳도 없다 — stderr 로만 정직하게 알린다.
        _budget_skip(None, "node_dir_unresolved", budget_label)
    else:
        inputs, skip_reason = _budget_inputs(candidate, opts)
        if inputs is None:
            _budget_skip(budget_node_dir, skip_reason, budget_label)
        else:
            budget_declared = _budget_declare(budget_node_dir, inputs, budget_label, timeout)
            if not budget_declared:
                _budget_skip(budget_node_dir, "declare_failed", budget_label)

    load_ok = False
    vllm_profile = None
    functional = None
    error_excerpt = None
    health_wait = None

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
                "provenance": PROVENANCE_MEASURED,
                # docker run 자체가 실패 → 준비 대기에 진입조차 못 했다(None ≠ timeout).
                "health_wait": None,
                "effective_gmu": effective_gmu,
                "gmu_cap": gmu_cap_info,
            }

        # /health 200 폴링 + 컨테이너 생존검사(사망 시 즉시 이탈 — plan_26082223 결함 C).
        health_wait = _poll_health(port, timeout, container_name=container_name)
        healthy = bool(health_wait["healthy"])
        load_ok = healthy

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
        # 워치독은 teardown 완료까지 커버 후 PID 기반 정지(§2.3) → 페이지캐시 드랍(§4.1 ②).
        _stop_memwatch(wd_proc, wd_fh)
        _drop_caches_best_effort()
        # 예산 회수 — teardown **뒤**다. 선언은 로드 구간을 덮어야 하므로 컨테이너보다 오래 산다.
        #   trial-loop 는 한 실행에 여러 번 띄웠다 내리므로, 회수하지 않으면 다음 trial 의 declare 가
        #   stale 선언과 같은 폴에 몰려 워치독이 상태 전이를 관측하지 못한다(2026-08-15 multi 실증).
        #   docker run 실패로 조기 return 하는 경로도 이 finally 를 지난다.
        if budget_declared and budget_node_dir:
            _budget_clear(budget_node_dir)

    return {
        "trial_number": int(trial_number),
        "candidate": candidate,
        "load_ok": bool(load_ok),
        "vllm_profile": vllm_profile,
        "functional": functional,
        "log_path": log_path,
        "error_excerpt": error_excerpt,
        "provenance": PROVENANCE_MEASURED,
        # 준비 대기의 **종단 사유**. `timeout`(30분 소진)과 `container_died`(사망)는
        # 같은 load_ok=False 지만 원인이 다르다 — 구분이 없으면 sim_classify 의 note 도
        # 사후분석도 둘을 못 가른다(plan_26082223 결함 C).
        "health_wait": health_wait,
        # 실제로 emit 된 gmu 와 그 출처(§결정론 규율). 호출부는 overhead 유도에 **이 값**을
        # 써야 한다 — 요청값을 쓰면 캡이 걸린 트라이얼에서 overhead 가 틀어진다.
        "effective_gmu": effective_gmu,
        "gmu_cap": gmu_cap_info,
    }


# ===========================================================================
# 자체검사 (--self-test) — 결함 C(생존검사) · 결함 B(gmu 캡). plan_26082223 §4.
#   docker·모델·하드웨어 불요. 생존 조회는 순수 파서/주입으로 대체하고, 실 docker 음성대조는
#   별도로 수행한다(단위 자체검사는 "도는지"를 못 본다 — 2026-08-22 교훈).
# ===========================================================================

def _self_test() -> int:
    global HEALTH_POLL_INTERVAL, _container_state
    failures: list[str] = []

    def check(name, got, want):
        if got != want:
            failures.append("%s: got=%r want=%r" % (name, got, want))

    import socket

    def _closed_port():
        s = socket.socket()
        s.bind(("127.0.0.1", 0))
        port = s.getsockname()[1]
        s.close()
        return port

    orig_interval = HEALTH_POLL_INTERVAL
    orig_state_fn = _container_state
    HEALTH_POLL_INTERVAL = 0.01   # 자체검사 전용 가속(원복은 finally).
    try:
        port = _closed_port()

        # ── 결함 C ──────────────────────────────────────────────────────────
        # C1 — container_name 미주입 = 하위 호환: 타임아웃 전량 소진(기존 동작).
        r = _poll_health(port, 0.15)
        check("C1.healthy", r["healthy"], False)
        check("C1.outcome", r["outcome"], "timeout")

        # C2 — 사망 감지 시 **타임아웃 전에 즉시 이탈**. 이것이 결함 C 의 처방 자체다.
        _container_state = lambda name: {"state": "dead", "detail": "false 137 true"}
        r = _poll_health(port, 30.0, container_name="probe")
        check("C2.outcome", r["outcome"], "container_died")
        check("C2.healthy", r["healthy"], False)
        check("C2.container_state", r["container_state"], "dead")
        if r["waited_s"] >= 5.0:
            failures.append("C2.early_exit: %.2fs 소요 — 즉시 이탈이 아니다" % r["waited_s"])

        # C3 — 컨테이너 오브젝트 부재(누가 rm 했다)도 죽음과 동치다.
        _container_state = lambda name: {"state": "absent", "detail": "No such object"}
        r = _poll_health(port, 30.0, container_name="probe")
        check("C3.outcome", r["outcome"], "container_died")

        # C4 — ★위양성 방지: 조회 실패(unknown)는 **죽음이 아니다**. 폴링을 계속해야 한다.
        #      이 가드가 없으면 일시적 docker 조회 실패가 정상 로드를 중단시킨다.
        _container_state = lambda name: {"state": "unknown", "detail": "daemon busy"}
        r = _poll_health(port, 0.15, container_name="probe")
        check("C4.outcome", r["outcome"], "timeout")
        if not r["unknown_polls"] >= 1:
            failures.append("C4.unknown_polls: 조회 실패가 기록되지 않았다(침묵 금지 위반)")

        # C5 — 반환 계약: bool 이 아니라 dict 다(`if _poll_health(...)` 오용 차단).
        if not isinstance(r, dict):
            failures.append("C5.type: 반환이 dict 가 아니다 — bool 회귀 시 호출부가 조용히 참이 된다")
        for k in ("healthy", "outcome", "waited_s", "polls", "container_state", "unknown_polls"):
            if k not in r:
                failures.append("C5.key: 반환 계약 키 누락 %r" % k)

        # C6 — inspect 출력 파서(순수).
        check("C6.running", parse_inspect_state(0, "true 0 false", "")["state"], "running")
        check("C6.dead", parse_inspect_state(0, "false 137 false", "")["state"], "dead")
        check("C6.oom", parse_inspect_state(0, "false 137 true", "")["oom_killed"], True)
        check("C6.absent", parse_inspect_state(1, "", "Error: No such object: x")["state"], "absent")
        check("C6.unknown_rc", parse_inspect_state(1, "", "daemon down")["state"], "unknown")
        check("C6.unknown_empty", parse_inspect_state(0, "", "")["state"], "unknown")
    finally:
        HEALTH_POLL_INTERVAL = orig_interval
        _container_state = orig_state_fn

    # ── 결함 B — gmu 캡 ────────────────────────────────────────────────────
    # 실측 스냅샷(2026-08-22 이 호스트): MemTotal 124610 MiB · weights(ckpt÷tp) 52989 MiB.
    TOTAL, WEIGHTS, FLOOR = 124610, 52989, HOST_FLOOR_MIB

    # B1 — 캡이 걸리고, **캡 이후 착지 예상 MemAvailable 이 바닥 이상**이어야 한다.
    avail = 119215
    b = host_floor_gmu_cap(0.90, TOTAL, avail, FLOOR, weights_mib=WEIGHTS,
                           overhead_mib=_FIXTURE_OVERHEAD_MIB)
    check("B1.applied", b["applied"], True)
    if not b["gmu"] < 0.90:
        failures.append("B1.cap: 캡이 요청값을 낮추지 않았다(%r)" % b["gmu"])
    landed = avail - b["gmu"] * TOTAL
    if not landed >= FLOOR:
        failures.append("B1.floor: 캡 후 착지 %.0fMiB < 바닥 %dMiB — 캡이 제 목적을 못 한다"
                        % (landed, FLOOR))

    # B2 — ★바닥은 절대 낮추지 않는다(policy:HOST_SAFETY_LAYERED_DEFENSE).
    #      캡 함수는 floor 를 **입력으로만** 쓰고 결코 되돌려 깎지 않는다.
    check("B2.floor_untouched", b["floor_mib"], FLOOR)

    # B3 — 여유가 충분하면 캡을 걸지 않는다(불필요한 축소 금지).
    b3 = host_floor_gmu_cap(0.50, TOTAL, avail, FLOOR, weights_mib=WEIGHTS,
                            overhead_mib=_FIXTURE_OVERHEAD_MIB)
    check("B3.applied", b3["applied"], False)
    check("B3.reason", b3["reason"], "not_needed")
    check("B3.gmu", b3["gmu"], 0.50)

    # B4 — 캡이 weights 를 굶기면 **걸지 않는다**(회복 가능한 사살 < 회복 불가한 하드 실패).
    b4 = host_floor_gmu_cap(0.90, TOTAL, 70000, FLOOR, weights_mib=WEIGHTS,
                            overhead_mib=_FIXTURE_OVERHEAD_MIB)
    check("B4.applied", b4["applied"], False)
    if not b4["reason"].startswith("cap_infeasible"):
        failures.append("B4.reason: got=%r want=cap_infeasible*" % b4["reason"])

    # B5 — 증명 불가 입력이면 캡을 걸지 않되 **사유를 남긴다**(침묵 폴백 금지).
    #      ★ reason 만 보면 부족하다 — 사유를 남기면서 값은 캡해 버리는 훼손이 초록불로
    #        통과했다(2026-08-23 음성대조에서 실제 검출). **불변식으로 못박는다**:
    #        `applied=False` ⇔ `gmu == requested`. 사유 문자열과 실제 값이 갈리면 안 된다.
    for _name, _b in (
        ("weights_unknown", host_floor_gmu_cap(0.90, TOTAL, avail, FLOOR)),
        ("meminfo_unavailable", host_floor_gmu_cap(0.90, None, None, FLOOR)),
        ("gmu_unset", host_floor_gmu_cap(None, TOTAL, avail, FLOOR)),
        ("cap_nonpositive", host_floor_gmu_cap(0.90, TOTAL, FLOOR, FLOOR, weights_mib=1)),
    ):
        check("B5.%s.reason" % _name, _b["reason"], _name)
        check("B5.%s.applied" % _name, _b["applied"], False)
        if _b["gmu"] != _b["requested"]:
            failures.append("B5.%s: applied=False 인데 gmu(%r) != requested(%r) — "
                            "사유만 남기고 값은 바꾸는 것은 침묵 폴백이다"
                            % (_name, _b["gmu"], _b["requested"]))
    # 같은 불변식을 적용/미적용 전 사례에 일괄 적용한다(위 B3·B4 포함).
    for _name, _b in (("B3", b3), ("B4", b4)):
        if not _b["applied"] and _b["gmu"] != _b["requested"]:
            failures.append("%s: applied=False 인데 gmu 가 바뀌었다(%r != %r)"
                            % (_name, _b["gmu"], _b["requested"]))

    # B6 — plan §4.3 리터럴 식은 **KV 몫의 비율**이라 gmu 상한으로 쓸 수 없다.
    #      이 호스트에서 그 식은 weights 조차 못 올리는 값을 낸다 — 그래서 총상한 축으로
    #      옮겨 적었다(host_floor_gmu_cap docstring). 그 사실을 회귀로 못박는다.
    plan_literal = (TOTAL - FLOOR - WEIGHTS - _FIXTURE_OVERHEAD_MIB) / float(TOTAL)
    if not plan_literal * TOTAL < WEIGHTS:
        failures.append("B6: plan 리터럴 식이 weights 를 담는다(%.0fMiB ≥ %dMiB) — 교정 근거 재확인 필요"
                        % (plan_literal * TOTAL, WEIGHTS))
    if not b["gmu"] > plan_literal:
        failures.append("B6: 교정된 캡(%.4f)이 리터럴 식(%.4f)보다 크지 않다" % (b["gmu"], plan_literal))

    if failures:
        sys.stderr.write("[run_trial --self-test] FAIL %d 건:\n" % len(failures))
        for f in failures:
            sys.stderr.write("  - %s\n" % f)
        return 1
    sys.stdout.write("[run_trial --self-test] OK — C1~C6(생존검사) · B1~B6(gmu 캡) 통과"
                     " (plan_26082223 결함 C·B)\n")
    return 0


def _main(argv: "list[str] | None" = None) -> int:
    p = argparse.ArgumentParser(
        description="단일 트라이얼 실행기 (vllm-recipe-explorer Phase 2)."
    )
    p.add_argument(
        "--candidate", help="candidate(lock-set) JSON 경로"
    )
    p.add_argument("--self-test", action="store_true",
                   help="생존검사·gmu 캡 회귀(docker/모델/하드웨어 불요)")
    p.add_argument(
        "--simlog-dir", help="trialNN_vllm.log 등을 기록할 디렉토리"
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
        help="호스트 NAS 모델 루트(config.nas_host_root; 기본 %s). terraforming_node CLI 배선" % NAS_MOUNT,
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
    # ── 서빙 예산 선언 배선 (plan_26081415 C3-1) ──────────────────────────────────────────
    p.add_argument(
        "--manifest",
        default=None,
        help="manifest.yaml 경로. nodes[role=main].node_id 로 선언 기록 위치를 파생한다"
             "(노드 사실의 단일 권위 — 브랜치·파일명으로 추론하지 않는다)",
    )
    p.add_argument(
        "--budget-node-dir",
        default=None,
        help="선언을 기록할 노드 디렉터리(docs/logs/<node_id>). --manifest 파생을 덮어쓴다",
    )
    p.add_argument(
        "--model-host-path",
        default=None,
        help="모델 디렉터리 **호스트** 경로 — 체크포인트 크기(weights_mib) 파생용. "
             "미지정 시 candidate.model_host_path/model_path 를 시도한다",
    )
    p.add_argument(
        "--tp",
        type=int,
        default=None,
        help="tensor parallel 수 — weights_mib 파생용(ckpt÷tp). 미지정 시 1. "
             "candidate 에서 추측하지 않는다(manifest 가 tp 의 권위 · recipe.resolve_tp)",
    )
    p.add_argument(
        "--overhead-mib",
        type=int,
        default=None,
        help="예산 선언의 overhead(MiB). **선언 필수**(기본값 없음 · 2026-09-05 G-B1) — "
             "후보 config `overhead_mib` 또는 env %s 로도 선언할 수 있다. 낮게 잡으면 선언 "
             "바닥이 높아져 워치독이 정상 서빙을 무장 밴드에 넣는다" % BUDGET_OVERHEAD_ENV,
    )
    p.add_argument(
        "--no-budget",
        action="store_true",
        help="예산 선언 생략(무보호 진입). 생략 사실은 budget_skipped 이벤트로 남는다(침묵 금지)",
    )
    args = p.parse_args(argv)

    if args.self_test:
        return _self_test()
    _missing = [n for n, v in (("--candidate", args.candidate),
                               ("--simlog-dir", args.simlog_dir)) if not v]
    if _missing:
        p.error("%s 는 필수다(--self-test 는 예외)" % ", ".join(_missing))

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
    if args.manifest:
        opts["manifest"] = args.manifest
    if args.budget_node_dir:
        opts["budget_node_dir"] = args.budget_node_dir
    if args.model_host_path:
        opts["model_host_path"] = args.model_host_path
    if args.tp:
        opts["tp"] = args.tp
    if args.overhead_mib:
        opts["overhead_mib"] = args.overhead_mib
    if args.no_budget:
        opts["no_budget"] = True

    result = run_trial(candidate, args.simlog_dir, args.trial_number, opts=opts)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())
