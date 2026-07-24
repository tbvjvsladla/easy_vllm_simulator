#!/usr/bin/env python3
"""preload_ram_gate.py — 로드-전 가용 RAM 결정론 게이트 (plan_26071019 §2.6).

사고 #5 직접 교훈(devlog_26070820: "Checkpoint size: 66.97 GiB. Available RAM: 45.80 GiB"
인데 그대로 로드 시작 → 호스트 하드다운): 체크포인트를 로드하기 *전에* 결정론으로 거부한다.

  required_mib = ceil(checkpoint_bytes / tp / MiB) + floor_mib
  MemAvailable < required → drop-caches 헬퍼(sudo -n, 설치돼 있으면) 1회 → 재측정 → 부족 지속 = 거부

- 체크포인트 크기 권위 = parse_model_config._native_weight_bytes(index total_size — **du 금지**,
  testlog_26070814 .git-부풀림 결함). 크기 미상(None)이면 **경고 후 게이트 생략**(음성정직 —
  거짓 크기로 false-block 하지 않는다).
- tp 분할: Ray TP=N 이면 노드당 로드는 ≈ 전체/N (DSpark 실측 77.7GiB/node @ TP=2 정합).
- floor 기본 10240MiB = mem_watchdog 상시 임계와 동일(게이트 통과 직후 워치독 존이 침식되지 않게).
- drop-caches 자동 실행은 §4.1 sudoers 단일 헬퍼(/usr/local/sbin/vllm-drop-caches)가 설치된
  경우에만(sudo -n — 무암호 아닐 땐 조용히 생략, 미설치 환경 fail-open + 안내).

stdlib-only. CLI 종료코드: 0=통과(또는 크기미상 생략) · 7=거부 · 1=인자 오류.
"""
import argparse
import os
import subprocess
import sys

MIB = 1024 * 1024
DROP_HELPER = "/usr/local/sbin/vllm-drop-caches"


def mem_available_mib():
    with open("/proc/meminfo", encoding="ascii") as f:
        for line in f:
            if line.startswith("MemAvailable:"):
                return int(line.split()[1]) // 1024
    return None


def checkpoint_bytes_for(host_path, warnings=None):
    """모델 디렉토리의 체크포인트 총 바이트 — parse_model_config._native_weight_bytes 재사용(중복 저작 금지)."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    from parse_model_config import _native_weight_bytes  # noqa: E402
    return _native_weight_bytes(host_path, warnings if warnings is not None else [])


def try_drop_caches():
    """설치돼 있으면 sudo -n 으로 1회 실행. 성공 True. 미설치/무권한은 False(예외 없음)."""
    if not os.path.exists(DROP_HELPER):
        return False
    try:
        r = subprocess.run(["sudo", "-n", DROP_HELPER], stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL, timeout=30)
        return r.returncode == 0
    except Exception:
        return False


def gate(checkpoint_bytes, tp=1, floor_mib=10240, auto_drop=True, log=None):
    """반환 dict: ok(bool)·required_mib·avail_before_mib·avail_after_mib·dropped(bool)·skipped(bool)."""
    _log = log or (lambda m: print("[preload-gate] %s" % m, file=sys.stderr))
    if not checkpoint_bytes or checkpoint_bytes <= 0:
        _log("⚠ 체크포인트 크기 미상(index total_size 부재) — 게이트 생략(음성정직·false-block 금지)")
        return {"ok": True, "skipped": True, "required_mib": None,
                "avail_before_mib": mem_available_mib(), "avail_after_mib": None, "dropped": False}
    tp = max(1, int(tp))
    required = -(-int(checkpoint_bytes) // tp // MIB) + int(floor_mib)  # ceil-div
    before = mem_available_mib()
    avail, dropped = before, False
    if avail is not None and avail < required and auto_drop:
        _log("부족(MemAvailable=%dMiB < required=%dMiB) → drop-caches 시도" % (avail, required))
        dropped = try_drop_caches()
        if dropped:
            avail = mem_available_mib()
            _log("drop-caches 후 MemAvailable=%dMiB" % avail)
        else:
            _log("drop-caches 헬퍼 미가용(미설치/무권한) — install_host_safety.sh 참조")
    ok = avail is not None and avail >= required
    if not ok:
        _log("거부: MemAvailable=%sMiB < required=%dMiB (ckpt=%.2fGiB ÷ tp=%d + floor=%dMiB) — "
             "잔존 컨테이너/페이지캐시 정리 후 재시도 (헌법 호스트 안전체계 따름정리)"
             % (avail, required, checkpoint_bytes / 2**30, tp, floor_mib))
    return {"ok": ok, "skipped": False, "required_mib": required,
            "avail_before_mib": before, "avail_after_mib": avail, "dropped": dropped}


def main():
    ap = argparse.ArgumentParser(description="로드-전 가용 RAM 게이트 (plan_26071019 §2.6)")
    src = ap.add_mutually_exclusive_group(required=True)
    src.add_argument("--model-host-path", help="모델 디렉토리(호스트 경로) — index total_size 산출")
    src.add_argument("--checkpoint-bytes", type=int, help="체크포인트 바이트 직접 지정")
    ap.add_argument("--tp", type=int, default=1, help="텐서 병렬 수(노드당 로드 ≈ 전체/tp)")
    ap.add_argument("--floor-mib", type=int, default=10240, help="가산 안전마진(기본 10240=워치독 임계)")
    ap.add_argument("--no-drop", action="store_true", help="drop-caches 자동 실행 생략")
    a = ap.parse_args()
    ckpt = a.checkpoint_bytes
    if a.model_host_path:
        warns = []
        ckpt = checkpoint_bytes_for(a.model_host_path, warns)
        for w in warns:
            print("[preload-gate] ⚠ %s" % w, file=sys.stderr)
    res = gate(ckpt, tp=a.tp, floor_mib=a.floor_mib, auto_drop=not a.no_drop)
    print("[preload-gate] %s required=%sMiB avail=%s→%sMiB dropped=%s"
          % ("PASS" if res["ok"] else "REFUSE", res["required_mib"],
             res["avail_before_mib"], res["avail_after_mib"], res["dropped"]), file=sys.stderr)
    sys.exit(0 if res["ok"] else 7)


if __name__ == "__main__":
    main()
