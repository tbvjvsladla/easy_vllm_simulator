#!/usr/bin/env python3
"""agent_guard.py — 에이전트 예방 트리거 계약의 실행체 (plan_26080121).

노드블랙박스 3단 응답의 **2단**이다:
    1단 에이전트 예방(이 파일)  →  2단 데몬 워치독 SIGKILL  →  3단 사후 포착(efi_pstore/netconsole)

★ 무엇을 시험하는가: "코드에이전트가 상시감시하며 **미리** 예방 트리거를 수행하는가".
  데몬이 6초 앞에서 죽이기 전에, 이 가드가 300초 앞에서 개입해야 한다. 데몬이 죽였다면
  호스트는 살았어도 **에이전트는 실패한 것**이며, 그 사실을 PARTIAL 로 정직하게 남긴다.

★ 이 가드는 방어의 **추가 층**이지 대체가 아니다. 가드가 죽어도 데몬 워치독이 백스톱으로 남는다.
  그래서 가드는 워치독과 그 선언(serve_budget.env)을 **절대 건드리지 않는다** —
  감시자를 감시자가 끄면 그건 방어가 아니다.

시각 처리(§7): 벽시계 금지 계약과 실시간 루프를 둘 다 만족시킨다.
  --now 를 앵커로 받고, 주기 제어는 monotonic, 기록 timestamp = 앵커 + monotonic 경과.

종료코드: 0=정상 종료(준위 회복 또는 --once) · 3=LAST_RESORT 수행 · 4=데몬 트립 관측(PARTIAL)
          · 1=인자/전제 오류
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime, timedelta, timezone

ISO_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")

# 준위. 경계는 envelope.eta_params 에서 읽고, LAST_RESORT 만 여기서 파생한다.
NORMAL, NOTIFY, ACT, LAST_RESORT = "NORMAL", "NOTIFY", "ACT", "LAST_RESORT"

# LAST_RESORT 경계 = daemon_kill_s + kill latency 실측 상한 + 여유.
#   데몬보다 **먼저** 손쓸 수 있는 마지막 시점이다(plan_26080121 §2).
LAST_RESORT_MARGIN_S = 10.0


def _parse_now(value: str) -> datetime:
    if not isinstance(value, str) or not ISO_RE.match(value):
        raise SystemExit("--now 는 YYYY-MM-DDTHH:MM:SSZ 형식이어야 한다: %r" % (value,))
    return datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)


class Clock:
    """앵커 + monotonic 경과. 벽시계를 읽지 않는다(결정론 계약 §7)."""

    def __init__(self, anchor: datetime):
        self._anchor = anchor
        self._t0 = time.monotonic()

    def elapsed_s(self) -> float:
        return time.monotonic() - self._t0

    def stamp(self) -> str:
        return (self._anchor + timedelta(seconds=self.elapsed_s())).strftime("%Y-%m-%dT%H:%M:%SZ")


def read_mem_avail_mib() -> int | None:
    """/proc/meminfo MemAvailable(MiB). 워치독과 **같은 지표**를 본다 — 다른 걸 보면 판정이 갈린다."""
    try:
        with open("/proc/meminfo", "r", encoding="utf-8") as fh:
            for line in fh:
                if line.startswith("MemAvailable:"):
                    return int(line.split()[1]) // 1024
    except (OSError, ValueError, IndexError):
        return None
    return None


def load_envelope(node_dir: str) -> dict:
    """에이전트가 폴링마다 읽는 유일한 파일. 부재/불량이면 계약 기본값으로 fail-safe."""
    path = os.path.join(node_dir, "envelope.json")
    try:
        with open(path, "r", encoding="utf-8") as fh:
            env = json.load(fh) or {}
    except (OSError, ValueError):
        env = {}
    p = env.get("eta_params") or {}
    k = env.get("kill") or {}
    lat = (k.get("latency_upper_bound_s") or {}).get("max")
    return {
        "agent_notify_s": float(p.get("agent_notify_s") or 900),
        "agent_act_s": float(p.get("agent_act_s") or 300),
        "daemon_kill_s": float(p.get("daemon_kill_s") or 6),
        "kill_threshold_mib": int(k.get("threshold_mib") or 10240),
        "kill_latency_max_s": float(lat if lat is not None else 14),
        "_source": path if env else "(부재 — 계약 기본값)",
    }


def compute_eta_s(mem_avail_mib: int, rate_mib_s: float, kill_threshold_mib: int):
    """ETA = (잔량 − 데몬 임계) ÷ 하강률.

    ★ 바닥을 0 이 아니라 **데몬 임계**로 잡는다. 에이전트가 이겨야 하는 상대는 물리 고갈이 아니라
      데몬의 SIGKILL 이다. 0 기준이면 남은 시간을 과대평가해 **항상 늦는다**(plan_26080121 §1).
    반환: 초. 상승/정체(rate ≤ 0)면 None(=∞).
    """
    if rate_mib_s is None or rate_mib_s <= 0:
        return None
    headroom = mem_avail_mib - kill_threshold_mib
    if headroom <= 0:
        return 0.0
    return headroom / rate_mib_s


DECL_MARGIN_MIB = 8192          # 데몬 BB_DECL_MARGIN_MIB 와 같은 값 — 두 층이 다른 상한을 쓰면 안 된다
LEVEL_ORDER = (NORMAL, NOTIFY, ACT, LAST_RESORT)


def read_declared_ceiling_mib(node_dir: str, now_epoch: int | None = None) -> int:
    """`serve_budget.env` → arm 상한(MiB). 선언이 없거나 만료면 **무한대**(=현행 규칙 그대로).

    ★ 2026-08-01 실화로 이식됐다. 이 가드는 처음에 이 함수 없이 만들어졌고, 그 결과 ⑥의
      **정상 재기동을 4초 만에 죽였다**(12:55:02 NOTIFY → :05 ACT → :06 LAST_RESORT →
      graceful_stop). 데몬은 같은 결함을 오늘 아침에 고쳤는데 **그 교훈이 가드로 전파되지
      않았다** — 이 프로젝트가 반복해서 만나는 "단일 파일 교정은 결함 계열을 못 막는다".

    ★ 안전 계약은 데몬과 동일하다(mem_watchdog_eta.sh `_decl_field`):
      **`source` 금지**(비-root 파일을 실행하는 셈) · 문자셋 제한 파싱 · TTL 필수 ·
      모든 실패는 무한대(=보수적으로 현행 규칙)로 복귀.
    """
    path = os.path.join(node_dir, "serve_budget.env")
    INF = 1 << 30
    try:
        with open(path, "r", encoding="utf-8") as fh:
            text = fh.read()
    except OSError:
        return INF
    fields = {}
    for key in ("floor_mib", "expires_epoch"):
        m = re.search(r"^[ \t]*%s[ \t]*=[ \t]*([A-Za-z0-9._-]{1,64})[ \t]*$" % key,
                      text, re.MULTILINE)
        fields[key] = m.group(1) if m else ""
    if not fields["floor_mib"].isdigit() or not fields["expires_epoch"].isdigit():
        return INF
    if now_epoch is None:
        now_epoch = int(time.time())
    if now_epoch >= int(fields["expires_epoch"]):
        return INF                                   # 만료 → 선언 없음과 동일
    ceiling = int(fields["floor_mib"]) - DECL_MARGIN_MIB
    if ceiling < 16384:                              # 데몬과 동일한 하한 가드
        return INF
    return ceiling


def classify(eta_s, env: dict, mem_avail_mib: int | None = None,
             arm_ceiling_mib: int | None = None) -> str:
    """준위 판정.

    ★ 선언된 바닥 위에서는 **무장하지 않는다**(데몬 `bb_trip_now` 와 동형).
      모델 로드의 하강은 체크포인트 크기로 **유계**인데 ETA 선형외삽은 그걸 무계로 읽는다.
      임계 문제가 아니라 정보 문제이므로, 처방도 임계 조정이 아니라 정보 보충이다.
    ★ 단 데몬 임계 아래는 선언과 무관하게 **항상** 무장한다 — 선언이 있어도 거기까지 내려가면
      선언이 틀린 것이다.
    """
    last_resort_s = env["daemon_kill_s"] + env["kill_latency_max_s"] + LAST_RESORT_MARGIN_S
    if mem_avail_mib is not None and mem_avail_mib <= env["kill_threshold_mib"]:
        return LAST_RESORT                           # 절대 바닥 — 선언 무관
    if (arm_ceiling_mib is not None and mem_avail_mib is not None
            and mem_avail_mib > arm_ceiling_mib):
        return NORMAL                                # 선언된 바닥 위 = 예상된 유계 하강
    if eta_s is None:
        return NORMAL
    if eta_s <= last_resort_s:
        return LAST_RESORT
    if eta_s <= env["agent_act_s"]:
        return ACT
    if eta_s <= env["agent_notify_s"]:
        return NOTIFY
    return NORMAL


def promote_one_step(current: str, target: str) -> str:
    """준위는 **한 칸씩만** 오른다. 강등은 즉시 허용한다.

    ★ 근거(2026-08-01): 가중치 버퍼 선할당은 3초에 ~75 GiB 를 잡아 순간 하강률이 9,298 MiB/s
      까지 튄다(전 구간 평균 171 의 54배). 창 하나의 기울기로 NOTIFY→LAST_RESORT 를 한 번에
      건너뛰어 정상 로드를 죽였다. 회복은 빨라야 하고 승격은 신중해야 한다 — 비대칭이 옳다.
    """
    ci, ti = LEVEL_ORDER.index(current), LEVEL_ORDER.index(target)
    if ti <= ci:
        return target                                # 강등·유지는 즉시
    return LEVEL_ORDER[ci + 1]


def append_event(node_dir: str, rec: dict) -> None:
    """events/<YYYY-MM>.jsonl append. 사후 재구성의 유일한 근거이므로 실패해도 루프를 멈추지 않되
    stderr 로 반드시 보이게 한다(침묵 실패 금지)."""
    month = rec["ts"][:7]
    d = os.path.join(node_dir, "events")
    try:
        os.makedirs(d, exist_ok=True)
        with open(os.path.join(d, "%s.jsonl" % month), "a", encoding="utf-8") as fh:
            fh.write(json.dumps(rec, ensure_ascii=False, sort_keys=True) + "\n")
    except OSError as e:
        print("[agent-guard] WARN: 이벤트 기록 실패(%s) — 판정은 계속한다" % e, file=sys.stderr)


# ── 행동 사다리 ─────────────────────────────────────────────────────────────
# 수요를 **먼저** 줄이고 공급을 나중에 회수한다 — 수요가 계속 늘면 회수한 만큼 다시 먹힌다.

def act_halt_load(stop_file: str | None) -> dict:
    """① 수요 감소 — stop-file 을 만들어 부하 드라이버가 신규 요청 발행을 멈추게 한다.
    즉시 효과 · 부작용 0 · 되돌리기 쉬움이라 사다리 첫 칸이다."""
    if not stop_file:
        return {"action": "halt_load", "ok": False, "detail": "stop-file 미지정 — 건너뜀"}
    try:
        with open(stop_file, "w", encoding="utf-8") as fh:
            fh.write("halted by agent_guard\n")
        return {"action": "halt_load", "ok": True, "detail": stop_file}
    except OSError as e:
        return {"action": "halt_load", "ok": False, "detail": str(e)}


def act_drop_caches(helper: str) -> dict:
    """② 공급 회수 — sudoers NOPASSWD 가 **이미 이 사용자에게 프로비저닝돼 있다**.
    설계자가 의도한 에이전트 행동이라는 뜻이다(plan_26080121 §3)."""
    try:
        p = subprocess.run(["sudo", "-n", helper], capture_output=True, text=True, timeout=60)
        return {"action": "drop_caches", "ok": p.returncode == 0,
                "detail": (p.stdout or p.stderr).strip()[:200]}
    except (OSError, subprocess.TimeoutExpired) as e:
        return {"action": "drop_caches", "ok": False, "detail": str(e)}


def act_graceful_stop(container: str | None, grace_s: int) -> dict:
    """③ LAST_RESORT — 우아한 정지. **SIGKILL 을 이기는 게 아니라 증거를 남기는 게 목적**이다.
    SIGKILL 은 즉사시켜 로그가 중간에 끊긴다(sim_classify 가 '무예외 외부종료' 판별을 따로
    만들어야 했던 이유). 우아한 정지는 엔진이 왜 모자랐는지 스스로 기록할 시간을 준다."""
    if not container:
        return {"action": "graceful_stop", "ok": False, "detail": "컨테이너 미지정 — 건너뜀"}
    try:
        p = subprocess.run(["docker", "stop", "-t", str(grace_s), container],
                           capture_output=True, text=True, timeout=grace_s + 60)
        return {"action": "graceful_stop", "ok": p.returncode == 0,
                "detail": (p.stdout or p.stderr).strip()[:200]}
    except (OSError, subprocess.TimeoutExpired) as e:
        return {"action": "graceful_stop", "ok": False, "detail": str(e)}


def _main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="에이전트 예방 트리거 가드 (plan_26080121)")
    # ★ --self-test 는 하드웨어·맥락 불요여야 한다(다른 node_blackbox 도구와 동일 계약).
    #   required=True 로 두면 자체시험조차 노드 맥락을 요구해 CI/배포검증에서 못 돌린다.
    ap.add_argument("--node-dir", help="docs/logs/<node_id> (--self-test 외 필수)")
    ap.add_argument("--now", help="앵커 시각 YYYY-MM-DDTHH:MM:SSZ (벽시계 금지 · --self-test 외 필수)")
    ap.add_argument("--stop-file", help="부하 드라이버 정지 신호 파일(① halt_load 대상)")
    ap.add_argument("--container", help="서빙 컨테이너명(③ graceful_stop 대상)")
    ap.add_argument("--drop-caches-helper", default="/usr/local/sbin/vllm-drop-caches")
    ap.add_argument("--grace-s", type=int, default=30, help="graceful stop 유예")
    ap.add_argument("--poll-s", type=float, default=5.0, help="NORMAL 준위 폴링 주기")
    ap.add_argument("--poll-hot-s", type=float, default=1.0, help="NOTIFY 이상 폴링 주기")
    ap.add_argument("--rate-window", type=int, default=5, help="하강률 산정 표본 수")
    ap.add_argument("--debounce-n", type=int, default=3,
                    help="승격에 필요한 연속 폴 수(데몬 BB_DEBOUNCE_N 과 동형 · 순간 스파이크 차단)")
    ap.add_argument("--max-s", type=float, default=0, help="최대 구동 시간(0=무제한)")
    ap.add_argument("--once", action="store_true", help="1회 판정 후 종료(시험용)")
    ap.add_argument("--dry-run", action="store_true", help="판정만 하고 행동하지 않는다")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args(argv)

    if a.self_test:
        return _self_test()
    # ★ 감시 도구의 출력은 **실시간으로 보여야 한다**. 파일로 리다이렉트하면 Python stdout 이
    #   블록버퍼링돼 준위 전이가 수십 분 늦게 보인다 — 감시자가 안 보이면 감시가 아니다.
    try:
        sys.stdout.reconfigure(line_buffering=True)
    except AttributeError:
        pass
    missing = [f for f, v in (("--node-dir", a.node_dir), ("--now", a.now)) if not v]
    if missing:
        raise SystemExit("실행 모드에는 %s 가 필요하다(--self-test 는 예외)" % ", ".join(missing))

    clock = Clock(_parse_now(a.now))
    node_dir = a.node_dir
    env = load_envelope(node_dir)
    print("[agent-guard] 계약 로드: notify=%.0fs act=%.0fs daemon_kill=%.0fs kill_thresh=%dMiB "
          "kill_lat_max=%.0fs  src=%s"
          % (env["agent_notify_s"], env["agent_act_s"], env["daemon_kill_s"],
             env["kill_threshold_mib"], env["kill_latency_max_s"], env["_source"]))

    samples: list[tuple[float, int]] = []          # (elapsed_s, mem_mib)
    level_prev = NORMAL
    streak, streak_level = 0, None
    ladder_done: set = set()
    rc = 0

    while True:
        mem = read_mem_avail_mib()
        if mem is None:
            print("[agent-guard] WARN: MemAvailable 판독 실패 — 다음 폴로 넘어간다", file=sys.stderr)
            time.sleep(a.poll_s)
            continue
        t = clock.elapsed_s()
        samples.append((t, mem))
        if len(samples) > a.rate_window:
            samples.pop(0)

        # 하강률: 창 양끝 기울기. 상승이면 음수 → ETA None(∞).
        rate = None
        if len(samples) >= 2:
            dt = samples[-1][0] - samples[0][0]
            if dt > 0:
                rate = (samples[0][1] - samples[-1][1]) / dt      # 감소량/초 (양수=하강)

        eta = compute_eta_s(mem, rate, env["kill_threshold_mib"])
        # 선언은 매 폴 재평가한다 — TTL 만료·재선언이 즉시 반영돼야 한다.
        arm_ceiling = read_declared_ceiling_mib(node_dir)
        raw = classify(eta, env, mem_avail_mib=mem, arm_ceiling_mib=arm_ceiling)
        eta_txt = "inf" if eta is None else "%.1fs" % eta

        # ── 디바운스: 승격은 연속 N폴 지속 시에만. 강등은 즉시(회복은 빨라야 한다) ──
        if LEVEL_ORDER.index(raw) > LEVEL_ORDER.index(level_prev):
            streak = streak + 1 if raw == streak_level else 1
            streak_level = raw
            if streak < a.debounce_n:
                time.sleep(a.poll_hot_s if level_prev != NORMAL else a.poll_s)
                continue                     # 아직 지속 확인 전 — 승격하지 않는다
            level = promote_one_step(level_prev, raw)   # 한 칸만 오른다
            streak = 0
            streak_level = None
        else:
            streak, streak_level = 0, None
            level = raw

        if level != level_prev:
            append_event(node_dir, {
                "ts": clock.stamp(), "kind": "agent_level", "source": "agent_guard",
                "level": level, "prev": level_prev, "mem_avail_mib": mem,
                "rate_mib_s": round(rate, 2) if rate else 0.0,
                "eta_s": None if eta is None else round(eta, 1),
                "arm_ceiling_mib": None if arm_ceiling >= (1 << 30) else arm_ceiling,
            })
            print("[agent-guard] 준위 %s → %s  mem=%dMiB rate=%s eta=%s  %s"
                  % (level_prev, level, mem,
                     ("%.1f" % rate) if rate else "0.0", eta_txt, clock.stamp()))
            if level == NORMAL:                     # 회복 — 사다리 재무장
                ladder_done.clear()
            level_prev = level

        # ── 행동 ────────────────────────────────────────────────────────────
        if level == ACT and not a.dry_run:
            for name, fn in (("halt_load", lambda: act_halt_load(a.stop_file)),
                             ("drop_caches", lambda: act_drop_caches(a.drop_caches_helper))):
                if name in ladder_done:
                    continue
                res = fn()
                ladder_done.add(name)
                res.update({"ts": clock.stamp(), "kind": "agent_action",
                            "source": "agent_guard", "level": level, "mem_avail_mib": mem})
                append_event(node_dir, res)
                print("[agent-guard] 행동 %s ok=%s %s" % (res["action"], res["ok"], res["detail"]))
                break                                # 한 칸 실행 후 **재측정**한다(계약 §3)

        if level == LAST_RESORT and not a.dry_run:
            res = act_graceful_stop(a.container, a.grace_s)
            res.update({"ts": clock.stamp(), "kind": "agent_last_resort",
                        "source": "agent_guard", "mem_avail_mib": mem,
                        "eta_s": None if eta is None else round(eta, 1)})
            append_event(node_dir, res)
            print("[agent-guard] LAST_RESORT graceful_stop ok=%s %s" % (res["ok"], res["detail"]))
            return 3

        if a.once:
            print("[agent-guard] --once: level=%s mem=%dMiB eta=%s" % (level, mem, eta_txt))
            return 0
        if a.max_s and clock.elapsed_s() >= a.max_s:
            print("[agent-guard] --max-s 도달 — 종료(마지막 준위=%s)" % level)
            return rc
        time.sleep(a.poll_hot_s if level != NORMAL else a.poll_s)


def _self_test() -> int:
    """계약의 경계값·순서·금지를 시험한다. 하드웨어 불요."""
    ok = True

    def chk(cond, label):
        nonlocal ok
        print("  [%s] %s" % ("PASS" if cond else "FAIL", label))
        ok = ok and bool(cond)

    env = {"agent_notify_s": 900.0, "agent_act_s": 300.0, "daemon_kill_s": 6.0,
           "kill_latency_max_s": 14.0, "kill_threshold_mib": 10240}
    lr = env["daemon_kill_s"] + env["kill_latency_max_s"] + LAST_RESORT_MARGIN_S   # 30.0

    # ETA 정의 — 바닥은 데몬 임계지 0 이 아니다
    chk(compute_eta_s(20480, 10.0, 10240) == (20480 - 10240) / 10.0, "ETA 바닥 = 데몬 임계")
    chk(compute_eta_s(20480, 10.0, 0) != compute_eta_s(20480, 10.0, 10240),
        "바닥 0 과 데몬 임계는 다른 값(과대평가 방지)")
    chk(compute_eta_s(20480, 0, 10240) is None, "정체(rate=0) → ∞")
    chk(compute_eta_s(20480, -5.0, 10240) is None, "상승(rate<0) → ∞")
    chk(compute_eta_s(9000, 10.0, 10240) == 0.0, "이미 임계 아래 → 0")

    # 준위 경계
    chk(classify(None, env) == NORMAL, "∞ → NORMAL")
    chk(classify(901, env) == NORMAL, "901s → NORMAL")
    chk(classify(900, env) == NOTIFY, "900s(경계) → NOTIFY")
    chk(classify(301, env) == NOTIFY, "301s → NOTIFY")
    chk(classify(300, env) == ACT, "300s(경계) → ACT")
    chk(classify(lr + 0.1, env) == ACT, "LAST_RESORT 경계 직상 → ACT")
    chk(classify(lr, env) == LAST_RESORT, "30s(경계) → LAST_RESORT")
    chk(classify(0, env) == LAST_RESORT, "0s → LAST_RESORT")

    # LAST_RESORT 경계가 데몬보다 앞선다 — 이게 성립 안 하면 계약 자체가 무의미
    chk(lr > env["daemon_kill_s"], "LAST_RESORT 가 데몬 kill 보다 **앞선다**")
    chk(lr > env["kill_latency_max_s"], "LAST_RESORT 가 kill latency 실측 상한보다 앞선다")

    # 금지: 미지정 대상에 대한 행동은 조용히 성공하지 않는다
    chk(act_halt_load(None)["ok"] is False, "stop-file 미지정 → 거짓 성공 금지")
    chk(act_graceful_stop(None, 5)["ok"] is False, "컨테이너 미지정 → 거짓 성공 금지")

    # envelope 부재 시 fail-safe 기본값
    e2 = load_envelope("/nonexistent-node-dir-for-selftest")
    chk(e2["agent_act_s"] == 300 and e2["kill_threshold_mib"] == 10240,
        "envelope 부재 → 계약 기본값으로 fail-safe")

    # ── 2026-08-01 실패 재현 회귀 ────────────────────────────────────────────
    # ⑥ 재기동을 4초 만에 죽인 실측값을 그대로 넣는다. 고쳤다는 '말'이 아니라 증명이 필요하다.
    INF = 1 << 30
    chk(classify(46.1, env, mem_avail_mib=98495, arm_ceiling_mib=37160) == NORMAL,
        "회귀: mem=98,495 rate=1,916(eta 46s) — 선언 상한 위 → NORMAL (실패 당시 ACT)")
    chk(classify(6.9, env, mem_avail_mib=74257, arm_ceiling_mib=37160) == NORMAL,
        "회귀: mem=74,257 rate=9,298(eta 6.9s) — 선언 상한 위 → NORMAL (실패 당시 LAST_RESORT)")
    chk(classify(491.2, env, mem_avail_mib=111463, arm_ceiling_mib=37160) == NORMAL,
        "회귀: mem=111,463(eta 491s) — 선언 상한 위 → NORMAL (실패 당시 NOTIFY)")
    # 선언이 없으면 옛 동작 그대로(양성키 설계 — 빠뜨려도 안전한 쪽으로 실패하지 않는다)
    chk(classify(6.9, env, mem_avail_mib=74257, arm_ceiling_mib=INF) == LAST_RESORT,
        "선언 부재(INF) → 현행 규칙 그대로 발동")
    # 선언 아래로 뚫으면 재무장
    chk(classify(46.1, env, mem_avail_mib=30000, arm_ceiling_mib=37160) == ACT,
        "선언 상한 **아래**로 뚫으면 재무장")
    # 절대 바닥은 선언과 무관하게 항상 무장
    chk(classify(None, env, mem_avail_mib=10000, arm_ceiling_mib=37160) == LAST_RESORT,
        "데몬 임계 아래 → 선언 무관 LAST_RESORT")

    # 준위 건너뛰기 금지
    chk(promote_one_step(NORMAL, LAST_RESORT) == NOTIFY, "NORMAL→LAST_RESORT 요구 시 NOTIFY 까지만")
    chk(promote_one_step(NOTIFY, LAST_RESORT) == ACT, "NOTIFY→LAST_RESORT 요구 시 ACT 까지만")
    chk(promote_one_step(ACT, LAST_RESORT) == LAST_RESORT, "ACT→LAST_RESORT 는 한 칸")
    chk(promote_one_step(LAST_RESORT, NORMAL) == NORMAL, "강등은 즉시(회복은 빨라야 한다)")
    chk(promote_one_step(ACT, NORMAL) == NORMAL, "ACT→NORMAL 즉시 강등")

    # 선언 파서 안전 계약
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        with open(os.path.join(td, "serve_budget.env"), "w", encoding="utf-8") as fh:
            fh.write("floor_mib=45352\nexpires_epoch=9999999999\n")
        chk(read_declared_ceiling_mib(td, now_epoch=1) == 45352 - DECL_MARGIN_MIB,
            "선언 파싱 → floor - margin")
        chk(read_declared_ceiling_mib(td, now_epoch=9999999999) == INF, "TTL 만료 → INF")
        with open(os.path.join(td, "serve_budget.env"), "w", encoding="utf-8") as fh:
            fh.write("floor_mib=$(rm -rf /)\nexpires_epoch=9999999999\n")
        chk(read_declared_ceiling_mib(td, now_epoch=1) == INF, "주입 시도 → INF(거부)")
        with open(os.path.join(td, "serve_budget.env"), "w", encoding="utf-8") as fh:
            fh.write("floor_mib=20000\nexpires_epoch=9999999999\n")
        chk(read_declared_ceiling_mib(td, now_epoch=1) == INF, "상한<16384 → INF(하한 가드)")
    chk(read_declared_ceiling_mib("/nonexistent") == INF, "선언 부재 → INF")

    # 시각: 앵커 + monotonic (벽시계 미사용)
    c = Clock(_parse_now("2026-08-01T00:00:00Z"))
    chk(c.stamp().endswith("Z") and c.stamp().startswith("2026-08-01T00:00:0"),
        "stamp = 앵커 기반(벽시계 아님)")

    print("self-test: %s" % ("PASS" if ok else "FAIL"))
    return 0 if ok else 2


if __name__ == "__main__":
    raise SystemExit(_main())
