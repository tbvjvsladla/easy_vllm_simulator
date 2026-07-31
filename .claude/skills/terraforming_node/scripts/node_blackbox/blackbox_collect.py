#!/usr/bin/env python3
"""blackbox_collect.py -- 1초 원시 수집기 (plan_26073109 §2.4·§2.5).

레거시 `hunt_telemetry.sh` 의 **치명적 결함을 교정**한 후계다. 레거시는 5초치를 셸 변수에
누적한 뒤 한 번에 append 했다 -- 사망이 <=7초였으므로 **가장 중요한 마지막 1~5초가 로컬·미러
양쪽에서 증발**했다(블랙박스가 사고 순간만 못 찍는 구조).

교정 3가지:
  1) **배칭 제거** -- 매 샘플 즉시 기록.
  2) **임계지표는 /dev/kmsg 로 송출** -- netconsole 이 UDP 로 **동기** 전송하므로 페이지캐시
     손실이 구조적으로 없다. peer 없는 single 노드에서도 같은 경로가 ramoops 로 보존된다
     (= 토폴로지 중립 단일 메커니즘, plan §2.5).
  3) **로컬 CSV 는 fsync** -- 하드다운 시 페이지캐시에 머문 채 소실되는 것을 막는다.

nvidia-smi 는 매 초 스폰하면 비싸므로 `-l 1` 스트리밍 서브프로세스를 **한 번만** 띄우고
마지막 줄을 읽는다(프로세스 스폰 0회/초).

CSV 스키마(plan §2.4 canonical):
    ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n

종료코드: 0=정상 · 1=전제 실패 · 2=자체시험 실패.
"""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import time

# ★ GB10 통합메모리 실측(2026-07-31): nvidia-smi 의 memory.used/memory.total 은 **[N/A]** 다 --
#   GPU 전용 메모리 풀이 없고 호스트와 공유하기 때문. 즉 이 하드웨어에서 **메모리 신호는
#   /proc/meminfo 의 MemAvailable 이 유일**하며, gpu_mem 열은 상시 빈 칸인 것이 정상이다
#   (결측이 아니라 부재 -- 이 구분을 CSV 빈 칸으로 정직 표기한다).
CSV_HEADER = "ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n\n"
KMSG_PREFIX = "easy-vllm-bb"
GPU_QUERY = "temperature.gpu,power.draw,clocks.sm,utilization.gpu,memory.used"


def read_mem_avail_mib(proc_meminfo="/proc/meminfo"):
    """MemAvailable(MiB). 파싱 실패는 None -- 조용한 0 대체 금지(0 은 '위험'을 뜻하므로 치명적)."""
    try:
        with open(proc_meminfo, encoding="utf-8") as fh:
            for line in fh:
                if line.startswith("MemAvailable:"):
                    return int(line.split()[1]) // 1024
    except (OSError, ValueError, IndexError):
        return None
    return None


def read_load1(proc_loadavg="/proc/loadavg"):
    try:
        with open(proc_loadavg, encoding="utf-8") as fh:
            return float(fh.read().split()[0])
    except (OSError, ValueError, IndexError):
        return None


def parse_gpu_line(line):
    """nvidia-smi CSV 한 줄 -> dict. 다중 GPU 는 첫 줄(노드당 1 GPU 전제, GB10)."""
    if not line:
        return {}
    parts = [p.strip() for p in line.split(",")]
    if len(parts) < 5:
        return {}
    out, keys = {}, ("gpu_temp", "gpu_pwr", "gpu_sm", "gpu_util", "gpu_mem")
    for k, v in zip(keys, parts):
        try:
            out[k] = float(v) if "." in v else int(v)
        except ValueError:
            out[k] = None          # '[N/A]' 등 -- 부재를 빈 칸으로 정직 표기
    return out


def compute_rate(prev_mem, cur_mem, dt_s):
    """양수 = 하강(소비). prev 없음/비정상 dt 는 None."""
    if prev_mem is None or cur_mem is None or not dt_s or dt_s <= 0:
        return None
    return round((prev_mem - cur_mem) / dt_s, 2)


def format_csv_row(sample):
    def f(key):
        v = sample.get(key)
        return "" if v is None else ("%g" % v if isinstance(v, float) else str(v))
    return ",".join([str(sample.get("ts", "")), f("mem_avail"), f("mem_rate"),
                     f("gpu_temp"), f("gpu_pwr"), f("gpu_sm"), f("gpu_util"),
                     f("gpu_mem"), f("load1"), f("ctr_n")]) + "\n"


def format_kmsg(sample):
    """오프박스로 나갈 **압축 1줄**. netconsole/ramoops 용량을 아끼려고 핵심 5개만.
    파싱 가능하도록 key=value 고정 순서."""
    def f(key):
        v = sample.get(key)
        return "na" if v is None else ("%g" % v if isinstance(v, float) else str(v))
    return "%s ts=%s mem=%s rate=%s t=%s w=%s" % (
        KMSG_PREFIX, sample.get("ts", ""), f("mem_avail"), f("mem_rate"),
        f("gpu_temp"), f("gpu_pwr"))


class GpuStream:
    """nvidia-smi -l 1 스트리밍. 스폰 1회 -- 매 초 스폰(50~200ms)을 피한다."""

    def __init__(self, interval=1, enabled=True):
        self.proc, self.last = None, ""
        self.enabled = enabled and bool(shutil.which("nvidia-smi"))
        if not self.enabled:
            return
        try:
            self.proc = subprocess.Popen(
                ["nvidia-smi", "--query-gpu=" + GPU_QUERY,
                 "--format=csv,noheader,nounits", "-l", str(interval)],
                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                text=True, bufsize=1)
            os.set_blocking(self.proc.stdout.fileno(), False)
        except OSError:
            self.proc, self.enabled = None, False

    def poll(self):
        if not self.proc:
            return {}
        try:
            while True:                     # 밀린 줄을 모두 소진해 **가장 최신**만 남긴다
                line = self.proc.stdout.readline()
                if not line:
                    break
                self.last = line.strip()
        except (BlockingIOError, ValueError, OSError):
            pass
        return parse_gpu_line(self.last)

    def close(self):
        if self.proc:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.proc.kill()


def container_count():
    try:
        out = subprocess.run(["docker", "ps", "-q"], capture_output=True, text=True,
                             timeout=5).stdout
        return len([x for x in out.split("\n") if x.strip()])
    except (OSError, subprocess.SubprocessError):
        return None


class SampleWriter:
    """일자 롤오버 + **매 샘플 fsync**. 하드다운에서 페이지캐시에 머문 채 소실되는 것을 막는다."""

    def __init__(self, samples_dir, fsync=True):
        self.dir, self.fsync, self.day, self.fh = samples_dir, fsync, None, None
        os.makedirs(samples_dir, exist_ok=True)

    def _rotate(self, day):
        if self.fh:
            self.fh.close()
        path = os.path.join(self.dir, "%s.csv" % day)
        new = not os.path.exists(path) or os.path.getsize(path) == 0
        self.fh = open(path, "a", encoding="utf-8")
        if new:
            self.fh.write(CSV_HEADER)
        self.day = day

    def write(self, day, row):
        if day != self.day:
            self._rotate(day)
        self.fh.write(row)
        self.fh.flush()
        if self.fsync:
            os.fsync(self.fh.fileno())

    def close(self):
        if self.fh:
            self.fh.close()


def write_kmsg(text, path="/dev/kmsg"):
    """실패는 치명이 아니다(비-root·kmsg 부재) -- 조용히 무시하되 호출자가 알 수 있게 bool 반환.

    append 모드를 쓴다: /dev/kmsg(캐릭터 디바이스)에는 'w' 와 동등하지만, 일반 파일로 리다이렉트해
    시험·검증할 때 'w' 는 매번 truncate 되어 마지막 1줄만 남는다(자체시험이 잡은 결함)."""
    try:
        with open(path, "a", encoding="utf-8") as fh:
            fh.write(text + "\n")
        return True
    except OSError:
        return False


def run(node_dir, interval=1.0, use_kmsg=True, fsync=True, max_samples=None,
        docker_every=5, kmsg_path="/dev/kmsg"):
    writer = SampleWriter(os.path.join(node_dir, "samples"), fsync=fsync)
    gpu = GpuStream(interval=int(max(1, interval)))
    prev_mem, prev_t, n, ctr_n = None, None, 0, None
    kmsg_ok = None
    try:
        while max_samples is None or n < max_samples:
            t0 = time.time()
            mem = read_mem_avail_mib()
            if n % docker_every == 0:
                ctr_n = container_count()
            sample = {"ts": int(t0), "mem_avail": mem,
                      "mem_rate": compute_rate(prev_mem, mem, (t0 - prev_t) if prev_t else None),
                      "load1": read_load1(), "ctr_n": ctr_n}
            sample.update(gpu.poll())
            writer.write(time.strftime("%Y-%m-%d", time.gmtime(t0)), format_csv_row(sample))
            if use_kmsg:
                ok = write_kmsg(format_kmsg(sample), kmsg_path)
                if kmsg_ok is None:
                    kmsg_ok = ok
                    if not ok:
                        print("[collect] 경고: %s 기록 불가(비-root?) — 오프박스 경로 없음"
                              % kmsg_path, file=sys.stderr)
            prev_mem, prev_t, n = mem, t0, n + 1
            time.sleep(max(0.0, interval - (time.time() - t0)))
    except KeyboardInterrupt:
        pass
    finally:
        gpu.close()
        writer.close()
    return n


def _self_test():
    import json  # noqa: F401  (fixture 가독성용)
    import tempfile
    checks = []

    with tempfile.TemporaryDirectory() as td:
        # meminfo 파싱
        mi = os.path.join(td, "meminfo")
        open(mi, "w").write("MemTotal:  131072000 kB\nMemAvailable:  12058624 kB\nCached: 1 kB\n")
        checks.append(("MemAvailable MiB 파싱", read_mem_avail_mib(mi) == 12058624 // 1024))
        open(mi, "w").write("MemTotal: 1 kB\n")
        checks.append(("MemAvailable 부재 -> None(0 대체 금지)", read_mem_avail_mib(mi) is None))
        checks.append(("파일 부재 -> None", read_mem_avail_mib(os.path.join(td, "nope")) is None))

        # loadavg
        la = os.path.join(td, "loadavg")
        open(la, "w").write("1.31 0.90 0.72 2/1234 5678\n")
        checks.append(("load1 파싱", read_load1(la) == 1.31))

        # GPU 라인
        g = parse_gpu_line("54, 31.20, 1275, 0, 2048")
        checks.append(("GPU 파싱", g == {"gpu_temp": 54, "gpu_pwr": 31.2, "gpu_sm": 1275,
                                         "gpu_util": 0, "gpu_mem": 2048}))
        g2 = parse_gpu_line("54, [N/A], 1275, 0, 2048")
        checks.append(("GPU N/A -> None(정직 부재)", g2.get("gpu_pwr") is None))
        checks.append(("GPU 빈 입력 -> {}", parse_gpu_line("") == {}))
        checks.append(("GPU 필드부족 -> {}", parse_gpu_line("54, 31.2") == {}))

        # 하강률 부호: 양수 = 하강
        checks.append(("하강 -> 양수", compute_rate(40104, 11419, 15.0) == round(28685 / 15.0, 2)))
        checks.append(("상승 -> 음수", compute_rate(10186, 117283, 1.0) < 0))
        checks.append(("prev 없음 -> None", compute_rate(None, 100, 1.0) is None))
        checks.append(("dt 0 -> None", compute_rate(200, 100, 0) is None))

        # CSV 행 -- 부재 필드는 빈 칸
        row = format_csv_row({"ts": 1785, "mem_avail": 117380, "mem_rate": -12.0,
                              "gpu_temp": 54, "gpu_pwr": 31.2, "load1": 1.31})
        checks.append(("CSV 헤더와 열 수 일치",
                       len(row.strip().split(",")) == len(CSV_HEADER.strip().split(","))))
        checks.append(("CSV 부재 필드 빈칸", row.strip().split(",")[6] == ""))

        # kmsg 압축 라인
        km = format_kmsg({"ts": 1785, "mem_avail": 117380, "mem_rate": -12.0,
                          "gpu_temp": 54, "gpu_pwr": 31.2})
        checks.append(("kmsg 접두어", km.startswith(KMSG_PREFIX + " ")))
        checks.append(("kmsg 압축 길이 <120B", len(km) < 120))
        checks.append(("kmsg 부재는 na", "na" in format_kmsg({"ts": 1, "mem_avail": None})))

        # 쓰기 경로: 롤오버 + 헤더 1회 + fsync
        sd = os.path.join(td, "node", "samples")
        w = SampleWriter(sd)
        w.write("2026-07-31", "a\n"); w.write("2026-07-31", "b\n"); w.write("2026-08-01", "c\n")
        w.close()
        d1 = open(os.path.join(sd, "2026-07-31.csv")).read()
        d2 = open(os.path.join(sd, "2026-08-01.csv")).read()
        checks.append(("일자 롤오버", os.path.exists(os.path.join(sd, "2026-08-01.csv"))))
        checks.append(("헤더 파일당 1회", d1.count(CSV_HEADER) == 1 and d2.count(CSV_HEADER) == 1))
        checks.append(("행 append", d1.endswith("a\nb\n")))
        # 재개 시 헤더 중복 없음
        w2 = SampleWriter(sd); w2.write("2026-07-31", "d\n"); w2.close()
        checks.append(("재기동 시 헤더 중복 없음",
                       open(os.path.join(sd, "2026-07-31.csv")).read().count(CSV_HEADER) == 1))

        # kmsg 실패가 치명적이지 않은가
        checks.append(("kmsg 쓰기 불가 -> False(예외 아님)",
                       write_kmsg("x", os.path.join(td, "no", "such", "kmsg")) is False))

        # 배칭이 없는가 = 샘플마다 파일이 커지는가 (레거시 5초 배칭 회귀 차단)
        nd = os.path.join(td, "node2")
        km_path = os.path.join(td, "fake_kmsg")
        open(km_path, "w").close()
        sizes = []
        real_sleep = time.sleep
        time.sleep = lambda s: None                    # 자체시험 가속
        try:
            for i in range(3):
                run(nd, interval=0.0, use_kmsg=True, fsync=True, max_samples=1,
                    kmsg_path=km_path)
                p = os.path.join(nd, "samples",
                                 time.strftime("%Y-%m-%d", time.gmtime()) + ".csv")
                sizes.append(os.path.getsize(p))
        finally:
            time.sleep = real_sleep
        checks.append(("샘플마다 즉시 기록(배칭 없음)", sizes[0] < sizes[1] < sizes[2]))
        checks.append(("kmsg 매 샘플 송출", len(open(km_path).read().strip().split("\n")) == 3))

    ok = True
    for name, passed in checks:
        print("  [%s] %s" % ("PASS" if passed else "FAIL", name))
        ok = ok and passed
    print("self-test: %s (%d 케이스)" % ("PASS" if ok else "FAIL", len(checks)))
    return 0 if ok else 2


def main(argv=None):
    ap = argparse.ArgumentParser(description="노드블랙박스 1초 수집기")
    ap.add_argument("--node-dir", help="docs/logs/<node_id>")
    ap.add_argument("--interval", type=float, default=1.0)
    ap.add_argument("--no-kmsg", action="store_true", help="kmsg 송출 끄기(오프박스 경로 포기)")
    ap.add_argument("--no-fsync", action="store_true", help="fsync 끄기(성능 시험용 — 운영 금지)")
    ap.add_argument("--max-samples", type=int, default=None)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()
    if not args.node_dir:
        ap.error("--node-dir 가 필요합니다")
    n = run(args.node_dir, interval=args.interval, use_kmsg=not args.no_kmsg,
            fsync=not args.no_fsync, max_samples=args.max_samples)
    print("[collect] %d 샘플 기록 -> %s/samples" % (n, args.node_dir))
    return 0


if __name__ == "__main__":
    sys.exit(main())
