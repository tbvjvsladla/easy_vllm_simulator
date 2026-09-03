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

★ SoC 열 구간 수집 (plan_26082319 §5.3 · 2026-08-23 하드락업 후속)
  하드 락업은 userland 가 원리적으로 못 잡는다(감시자도 함께 얼어붙는다). 그러나 **직전 궤적은
  포착할 수 있고**, 외부 보고(NVIDIA 개발자 포럼 GB10 하드 파워오프)에 따르면 그 궤적의 **선행
  지표는 GPU 온도가 아니라 SoC 열(acpitz)** 이다 -- "acpitz 88->97.8도 in 5s" 로 급차단하며
  그때 nvidia-smi 의 GPU 온도는 87~88도에 머문다. 즉 GPU 온도만 보는 것은 **지연 지표**를 보는 것.
  2026-08-23 사건에서도 GPU 온도(max 85도)는 **평시일(2026-08-05~10 의 max 87도)보다 낮았다**.
  그래서 zone 별 온도를 스키마에 넣는다.

  열 구간이 **없는 플랫폼도 있다** -- 그때 `soc_temp`/`soc_zone` 은 상시 빈 칸이며 이는 결측이
  아니라 **부재**다(gpu_mem 열의 GB10 통합메모리 선례와 같은 규율).

CSV 스키마(plan §2.4 canonical + plan_26082319 §5.3 확장):
    ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n,soc_temp,soc_zone

  ⚠ 확장은 **뒤에 덧붙인다**. 하류 판독기(logs_lifecycle·regen_envelope·blackbox_eta.replay)는
    전부 `header` 이름으로 색인하고 `len(parts) < len(header)` 로 방어하므로, 뒤에 붙은 열은
    옛 판독기를 깨지 않는다. 열 순서를 바꾸거나 중간에 끼우면 그 보장이 사라진다.

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
CSV_HEADER = ("ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n,"
              "soc_temp,soc_zone\n")
KMSG_PREFIX = "easy-vllm-bb"
GPU_QUERY = "temperature.gpu,power.draw,clocks.sm,utilization.gpu,memory.used"

# SoC 열 구간(sysfs thermal). GB10 은 thermal_zone0~6 이 전부 type=acpitz 다.
THERMAL_ROOT = "/sys/class/thermal"
# ★ 타당성 밴드 -- **이 파일에서만 쓰는 국소 상수**(4종 안티패턴 판정표 §매직넘버 `정당` 칸).
#   센서가 미초기화/탈착 상태에서 내는 값(-274000, 2147483647 등)을 최대값에 섞으면 트립 소비자가
#   그 잡음으로 판정한다. 범위를 벗어난 zone 은 **읽지 않은 것으로 취급**한다(조용한 0 대체 아님).
SOC_TEMP_PLAUSIBLE_C = (-40, 150)


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


class ThermalZones:
    """sysfs thermal zone 판독기. **zone 목록은 1회만 열거**한다.

    매 초 glob 하면 1 Hz 핫패스에서 디렉터리 스캔이 반복되고, 더 나쁘게는 zone 이 늘거나 줄 때
    `soc_zone` 인덱스의 의미가 조용히 바뀐다. 열거를 고정하면 인덱스가 프로세스 수명 동안
    같은 물리 zone 을 가리킨다(재기동 시 재열거 -- 로그에 재기동 경계가 이미 남는다).

    zone 이 하나도 없는 플랫폼에서는 `enabled=False` 이고 판독은 (None, None) 이다 --
    **부재이지 결측이 아니다**(CSV 빈 칸으로 정직 표기).
    """

    def __init__(self, root=THERMAL_ROOT):
        self.root = root
        self.zones = []                 # [(index, temp_path, type)]
        try:
            names = sorted(n for n in os.listdir(root) if n.startswith("thermal_zone"))
        except OSError:
            names = []
        for n in names:
            try:
                idx = int(n[len("thermal_zone"):])
            except ValueError:
                continue                # thermal_zoneX 형태가 아니면 건너뛴다
            tpath = os.path.join(root, n, "temp")
            if not os.path.exists(tpath):
                continue
            try:
                with open(os.path.join(root, n, "type"), encoding="utf-8") as fh:
                    ztype = fh.read().strip()
            except OSError:
                ztype = ""
            self.zones.append((idx, tpath, ztype))
        self.enabled = bool(self.zones)

    def types(self):
        """진단용 -- 어떤 zone 을 보고 있는지 기동 로그에 남긴다(침묵 배선 금지)."""
        return [(i, t) for i, _, t in self.zones]

    def read_max(self):
        """(최고 온도 섭씨 int, 그 zone 인덱스) 또는 (None, None).

        커널은 millidegree 로 준다. 타당성 밴드를 벗어난 zone 은 **읽지 않은 것으로 취급**한다 --
        미초기화 센서의 -274000/2147483647 을 최대값에 섞으면 트립 소비자가 잡음으로 판정한다.
        """
        lo, hi = SOC_TEMP_PLAUSIBLE_C
        best_c, best_idx = None, None
        for idx, tpath, _ in self.zones:
            try:
                with open(tpath, encoding="utf-8") as fh:
                    milli = int(fh.read().strip())
            except (OSError, ValueError):
                continue
            c = milli // 1000
            if c < lo or c > hi:
                continue
            if best_c is None or c > best_c:
                best_c, best_idx = c, idx
        return best_c, best_idx


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
                     f("gpu_mem"), f("load1"), f("ctr_n"),
                     f("soc_temp"), f("soc_zone")]) + "\n"


def format_kmsg(sample):
    """오프박스로 나갈 **압축 1줄**. netconsole/ramoops 용량을 아끼려고 핵심만.
    파싱 가능하도록 key=value 고정 순서.

    ★ `s=` (SoC 열)가 여기 있는 이유(plan_26082319 §5.3): 하드다운 직전 마지막 수 초는 로컬
      CSV 가 fsync 로 버텨 주지만, **커널이 얼어붙으면 그 이후는 오프박스 경로만 남는다**.
      SoC 열이 하드 파워오프의 선행 지표이므로 그 궤적이 오프박스에 실려야 사후분석이 성립한다.
    """
    def f(key):
        v = sample.get(key)
        return "na" if v is None else ("%g" % v if isinstance(v, float) else str(v))
    return "%s ts=%s mem=%s rate=%s t=%s w=%s s=%s" % (
        KMSG_PREFIX, sample.get("ts", ""), f("mem_avail"), f("mem_rate"),
        f("gpu_temp"), f("gpu_pwr"), f("soc_temp"))


class GpuStream:
    """nvidia-smi -l 1 스트리밍. 스폰 1회 -- 매 초 스폰(50~200ms)을 피한다."""

    # 무출력 판정 임계. 스트리밍 간격의 몇 배로 잡아 정상 지터를 트립으로 읽지 않는다.
    STALE_MULTIPLIER = 15

    def __init__(self, interval=1, enabled=True, stale_after_s=None):
        self.proc, self.last = None, ""
        self.interval = int(max(1, interval))
        self.stale_after_s = stale_after_s or (self.interval * self.STALE_MULTIPLIER)
        self.last_ts = None
        self.respawns = 0
        self.enabled = enabled and bool(shutil.which("nvidia-smi"))
        if not self.enabled:
            return
        self._spawn()
        self.last_ts = time.time() if self.proc else None

    def _spawn(self):
        try:
            self.proc = subprocess.Popen(
                ["nvidia-smi", "--query-gpu=" + GPU_QUERY,
                 "--format=csv,noheader,nounits", "-l", str(self.interval)],
                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                text=True, bufsize=1)
            os.set_blocking(self.proc.stdout.fileno(), False)
            self.respawns += 1
        except OSError:
            self.proc, self.enabled = None, False

    def poll(self):
        """최신 GPU 줄. 2026-09-03(plan_26090317 P3 실측): `nvidia-smi -l 1` 자식이 **살아 있는데
        출력을 멈추는** 상태가 실제로 발생했다(서브 09:51:32 이후 무출력 · 같은 시각 열 워치독이
        `thermal_gpu_stale` 발행). 비차단 읽기라 예외도 안 나고, 마지막 줄이 그대로 남아 **낡은 값이
        신선한 값처럼** 계속 실린다 — 관측층이 조용히 거짓말을 하는 형태다. 신선도를 스스로 보고,
        임계를 넘으면 자식을 되살리며 그 사실을 stderr(journal)로 남긴다."""
        if not self.proc:
            return {}
        got = False
        try:
            while True:                     # 밀린 줄을 모두 소진해 **가장 최신**만 남긴다
                line = self.proc.stdout.readline()
                if not line:
                    break
                self.last = line.strip()
                got = True
        except (BlockingIOError, ValueError, OSError):
            pass
        now = time.time()
        if got:
            self.last_ts = now
        elif self.last_ts and (now - self.last_ts) > self.stale_after_s:
            age = now - self.last_ts
            print("[collect] 경고: GPU 스트림 %.0fs 무출력 — 자식을 재기동한다"
                  "(낡은 값을 신선한 것처럼 싣지 않는다)" % age, file=sys.stderr)
            self.last = ""                  # 낡은 값을 즉시 버린다(부재가 거짓보다 낫다)
            self._respawn()
        return parse_gpu_line(self.last)

    def _respawn(self):
        try:
            self.close()
        except Exception:
            pass
        self.proc = None
        self._spawn()
        self.last_ts = time.time() if self.proc else None

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

    @staticmethod
    def _has_current_header(path):
        """이 파일이 **현재 스키마 헤더를 이미 담고 있는가**. 읽을 수 없으면 True(= 보류).

        ⚠ 첫 줄만 보면 안 된다(자체시험이 잡은 결함): 옛 헤더로 시작하는 파일은 드리프트 헤더를
          이미 적은 뒤에도 첫 줄이 여전히 옛 헤더라, **재기동마다 헤더가 한 줄씩 쌓인다**.
          판정 기준은 "이 파일이 현재 스키마를 이미 선언했는가" 이므로 전수 확인한다.
          비용은 롤오버·기동 때 1회이고 O(1) 메모리로 흐른다.
        판독 실패를 True 로 두는 방향: 헤더를 **덜 쓰는** 쪽이며, 잘못 쓰면 되돌릴 수 없다.
        """
        try:
            with open(path, encoding="utf-8", errors="replace") as fh:
                for line in fh:
                    if line == CSV_HEADER:
                        return True
            return False
        except OSError:
            return True

    def _rotate(self, day):
        if self.fh:
            self.fh.close()
        path = os.path.join(self.dir, "%s.csv" % day)
        new = not os.path.exists(path) or os.path.getsize(path) == 0
        # ★ 스키마 드리프트를 **조용히 넘기지 않는다**(plan_26082319 §5.3 · 침묵 금지).
        #   수집기가 열을 늘린 날, 그 날짜 파일은 이미 옛 헤더로 열려 있다. 헤더를 안 쓰면 열이
        #   늘어난 행이 **이름 없이** 쌓이고(soc_temp 가 헤더에 없다), 하류는 그 열이 원래
        #   없었던 것과 구별하지 못한다. 그래서 새 헤더 줄을 한 번 더 적어 경계를 데이터에 남긴다.
        #   판독기는 숫자 파싱 실패로 그 줄을 건너뛴다(replay/rollup 모두 try/except 로 방어).
        drift = (not new) and not self._has_current_header(path)
        self.fh = open(path, "a", encoding="utf-8")
        if new or drift:
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
        docker_every=5, kmsg_path="/dev/kmsg", thermal_root=THERMAL_ROOT):
    writer = SampleWriter(os.path.join(node_dir, "samples"), fsync=fsync)
    gpu = GpuStream(interval=int(max(1, interval)))
    soc = ThermalZones(thermal_root)
    # 배선을 기동 로그에 남긴다 -- "만들었지만 돌지 않는" 것을 사람이 알아채는 유일한 지점이다.
    if soc.enabled:
        print("[collect] SoC 열 zone %d개: %s"
              % (len(soc.zones), ", ".join("%d:%s" % z for z in soc.types())), file=sys.stderr)
    else:
        print("[collect] SoC 열 zone 없음(%s) — soc_temp/soc_zone 은 상시 빈 칸(부재)"
              % thermal_root, file=sys.stderr)
    prev_mem, prev_t, n, ctr_n = None, None, 0, None
    kmsg_ok = None
    try:
        while max_samples is None or n < max_samples:
            t0 = time.time()
            mem = read_mem_avail_mib()
            if n % docker_every == 0:
                ctr_n = container_count()
            soc_c, soc_idx = soc.read_max()
            sample = {"ts": int(t0), "mem_avail": mem,
                      "mem_rate": compute_rate(prev_mem, mem, (t0 - prev_t) if prev_t else None),
                      "load1": read_load1(), "ctr_n": ctr_n,
                      "soc_temp": soc_c, "soc_zone": soc_idx}
            sample.update(gpu.poll())
            writer.write(time.strftime("%Y-%m-%d", time.gmtime(t0)), format_csv_row(sample))
            if use_kmsg:
                ok = write_kmsg(format_kmsg(sample), kmsg_path)
                if kmsg_ok is None:
                    kmsg_ok = ok
                    if not ok:
                        print("[collect] 경고: %s 기록 불가(비-root?) — 오프박스 경로 없음"
                              % kmsg_path, file=sys.stderr)
            # 2026-09-03(P3): 한 바퀴가 비정상적으로 길면 그 사실이 journal 에 남아야 한다.
            #   "서비스는 active 인데 샘플이 안 자란다" 는 상태가 실제로 발생했고, 그때 프로세스는
            #   조용했다 — 관측층의 정지는 관측층 자신이 말해야 한다.
            _elapsed = time.time() - t0
            if interval > 0 and _elapsed > interval * 5:
                print("[collect] 경고: 한 샘플 주기가 %.1fs 걸렸다(간격 %.1fs) — 외부 호출 지연 의심"
                      % (_elapsed, interval), file=sys.stderr)
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

        # ── SoC 열 구간(plan_26082319 §5.3) ────────────────────────────────
        tz = os.path.join(td, "thermal")
        def _mkzone(idx, milli, ztype="acpitz"):
            d = os.path.join(tz, "thermal_zone%d" % idx)
            os.makedirs(d, exist_ok=True)
            open(os.path.join(d, "temp"), "w").write("%d\n" % milli)
            open(os.path.join(d, "type"), "w").write(ztype + "\n")
            return d
        _mkzone(0, 46800); _mkzone(1, 45000); _mkzone(5, 91200); _mkzone(6, 45800)
        z = ThermalZones(tz)
        checks.append(("SoC zone 열거(4개)", len(z.zones) == 4 and z.enabled))
        checks.append(("SoC zone type 보존", dict(z.types())[5] == "acpitz"))
        checks.append(("SoC 최고 zone·온도(91C @zone5)", z.read_max() == (91, 5)))
        # 부재는 결측이 아니다 -- zone 이 없는 플랫폼
        z0 = ThermalZones(os.path.join(td, "no-thermal"))
        checks.append(("SoC zone 부재 -> (None,None)·enabled False",
                       z0.read_max() == (None, None) and not z0.enabled))
        # 타당성 밴드: 미초기화 센서값이 최대값을 오염시키지 않는다
        _mkzone(7, 2147483647)          # 미초기화 센서
        _mkzone(8, -274000)             # 절대영도 미만
        z2 = ThermalZones(tz)
        checks.append(("SoC 타당성 밴드 밖 zone 무시(잡음이 최대값을 먹지 않음)",
                       z2.read_max() == (91, 5)))
        # 읽기 불가 zone 은 조용히 건너뛴다(다른 zone 은 계속 판독)
        os.chmod(os.path.join(tz, "thermal_zone5", "temp"), 0o000)
        z3 = ThermalZones(tz)
        _unreadable_ok = (z3.read_max() == (46, 0)) if os.geteuid() != 0 else True
        checks.append(("SoC 판독 불가 zone 은 건너뛰고 나머지로 판정", _unreadable_ok))
        os.chmod(os.path.join(tz, "thermal_zone5", "temp"), 0o644)
        # 비-thermal_zone 디렉터리(cooling_device 등)를 zone 으로 세지 않는다
        os.makedirs(os.path.join(tz, "cooling_device0"), exist_ok=True)
        checks.append(("cooling_device 는 zone 이 아니다", len(ThermalZones(tz).zones) == 6))

        # 하강률 부호: 양수 = 하강
        checks.append(("하강 -> 양수", compute_rate(40104, 11419, 15.0) == round(28685 / 15.0, 2)))
        checks.append(("상승 -> 음수", compute_rate(10186, 117283, 1.0) < 0))
        checks.append(("prev 없음 -> None", compute_rate(None, 100, 1.0) is None))
        checks.append(("dt 0 -> None", compute_rate(200, 100, 0) is None))

        # CSV 행 -- 부재 필드는 빈 칸
        row = format_csv_row({"ts": 1785, "mem_avail": 117380, "mem_rate": -12.0,
                              "gpu_temp": 54, "gpu_pwr": 31.2, "load1": 1.31})
        _cols = CSV_HEADER.strip().split(",")
        checks.append(("CSV 헤더와 열 수 일치",
                       len(row.strip().split(",")) == len(_cols)))
        checks.append(("CSV 부재 필드 빈칸", row.strip().split(",")[6] == ""))
        # SoC 열이 스키마에 있고, 부재는 빈 칸이다(결측 아님)
        checks.append(("CSV 스키마에 soc_temp·soc_zone 존재",
                       _cols[-2:] == ["soc_temp", "soc_zone"]))
        checks.append(("SoC 부재 -> 빈 칸 2개", row.strip().split(",")[-2:] == ["", ""]))
        row2 = format_csv_row({"ts": 1785, "soc_temp": 91, "soc_zone": 5})
        checks.append(("SoC 값 기록", row2.strip().split(",")[-2:] == ["91", "5"]))
        # zone 0 은 거짓값이 아니다 -- `if not idx` 류의 실수가 들어오면 여기서 잡힌다
        row3 = format_csv_row({"ts": 1, "soc_temp": 46, "soc_zone": 0})
        checks.append(("soc_zone=0 이 빈 칸으로 삼켜지지 않음",
                       row3.strip().split(",")[-1] == "0"))

        # kmsg 압축 라인
        km = format_kmsg({"ts": 1785, "mem_avail": 117380, "mem_rate": -12.0,
                          "gpu_temp": 54, "gpu_pwr": 31.2})
        checks.append(("kmsg 접두어", km.startswith(KMSG_PREFIX + " ")))
        checks.append(("kmsg 압축 길이 <120B", len(km) < 120))
        checks.append(("kmsg 부재는 na", "na" in format_kmsg({"ts": 1, "mem_avail": None})))
        # SoC 열이 오프박스 경로에 실린다(하드다운 직전 궤적 보존 -- plan_26082319 §5.3)
        km2 = format_kmsg({"ts": 1785, "mem_avail": 19251, "mem_rate": 0,
                           "gpu_temp": 63, "gpu_pwr": 90.33, "soc_temp": 91})
        checks.append(("kmsg 에 SoC 열 포함(s=)", " s=91" in km2))
        checks.append(("kmsg SoC 포함해도 <120B", len(km2) < 120))
        checks.append(("kmsg SoC 부재는 s=na", " s=na" in format_kmsg({"ts": 1})))

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
        # ★ 스키마 드리프트: 옛 헤더 파일에 이어 쓰면 새 헤더를 **한 번 더** 적어 경계를 남긴다.
        #   (침묵 금지 — 이름 없는 열이 쌓이면 "원래 없었던 것"과 구별되지 않는다)
        legacy = os.path.join(sd, "2026-06-01.csv")
        old_hdr = "ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n\n"
        open(legacy, "w").write(old_hdr + "1,2,3,4,5,6,7,,8,9\n")
        w3 = SampleWriter(sd); w3.write("2026-06-01", "x\n"); w3.close()
        _txt = open(legacy).read()
        checks.append(("옛 헤더 파일 -> 새 헤더 1줄 삽입(경계 표시)",
                       _txt.count(CSV_HEADER) == 1 and _txt.startswith(old_hdr)
                       and _txt.endswith("x\n")))
        # 같은 헤더면 다시 적지 않는다(드리프트 판정이 상시 참이면 헤더가 매 재기동마다 쌓인다)
        w4 = SampleWriter(sd); w4.write("2026-06-01", "y\n"); w4.close()
        checks.append(("드리프트 없는 재기동은 헤더를 더 쓰지 않음",
                       open(legacy).read().count(CSV_HEADER) == 1))

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
                # thermal_root 를 fixture 로 고정한다 — 설치 호스트에 zone 이 있든 없든 같은
                # 판정을 내야 한다(파라미터 종속 위양성 방지, mem_watchdog_eta 선례).
                run(nd, interval=0.0, use_kmsg=True, fsync=True, max_samples=1,
                    kmsg_path=km_path, thermal_root=tz)
                p = os.path.join(nd, "samples",
                                 time.strftime("%Y-%m-%d", time.gmtime()) + ".csv")
                sizes.append(os.path.getsize(p))
        finally:
            time.sleep = real_sleep
        checks.append(("샘플마다 즉시 기록(배칭 없음)", sizes[0] < sizes[1] < sizes[2]))
        checks.append(("kmsg 매 샘플 송출", len(open(km_path).read().strip().split("\n")) == 3))
        # ★ 배선 실증(“만든 것과 도는 것은 다르다”): run() 이 실제로 SoC 를 채워 CSV·kmsg 로
        #   내보내는가. 함수 단위 시험만으로는 호출부가 비어 있어도 전부 PASS 한다.
        _written = open(os.path.join(nd, "samples",
                        time.strftime("%Y-%m-%d", time.gmtime()) + ".csv")).read()
        _last = _written.strip().split("\n")[-1].split(",")
        checks.append(("run() 이 CSV 에 SoC 를 실제로 기록(배선 실증)",
                       _last[-2] == "91" and _last[-1] == "5"))
        checks.append(("run() 이 kmsg 에 SoC 를 실제로 송출(배선 실증)",
                       " s=91" in open(km_path).read().strip().split("\n")[-1]))
        # 음성대조 — zone 이 없으면 빈 칸이어야 한다(있는 척하지 않는다)
        nd2 = os.path.join(td, "node3")
        time.sleep = lambda s: None
        try:
            run(nd2, interval=0.0, use_kmsg=False, fsync=False, max_samples=1,
                thermal_root=os.path.join(td, "no-thermal"))
        finally:
            time.sleep = real_sleep
        _l2 = open(os.path.join(nd2, "samples", time.strftime("%Y-%m-%d", time.gmtime())
                                + ".csv")).read().strip().split("\n")[-1].split(",")
        # ── GPU 스트림 신선도 자가회복 (2026-09-03 신설 · plan_26090317 P3 실측 근거) ──
    #   서브에서 `nvidia-smi -l 1` 자식이 **살아 있는데 출력만 멈추는** 상태가 발생했다. 비차단
    #   읽기라 예외가 없고 `self.last` 가 남아, 낡은 값이 신선한 값처럼 계속 실린다. 부재보다
    #   나쁜 것이 **거짓 신선도**이므로, 임계를 넘으면 값을 버리고 자식을 되살린다.
    class _FakeProc:
        def __init__(self):
            import io as _io
            self.stdout = _io.StringIO("")
            self.terminated = False
        def terminate(self): self.terminated = True
        def wait(self, timeout=None): return 0
        def kill(self): self.terminated = True

    gs = GpuStream.__new__(GpuStream)
    gs.proc, gs.last = _FakeProc(), "50, 90.0, 1000, 30, 1024"
    gs.interval, gs.stale_after_s, gs.respawns = 1, 5, 1
    gs.enabled = True
    gs.last_ts = time.time()
    gs._spawn = lambda: setattr(gs, "proc", _FakeProc())
    fresh = gs.poll()
    checks.append(("무출력이지만 임계 이내면 마지막 값을 유지(정상 지터 오탐 ✗)",
                   fresh.get("gpu_temp") == 50))
    gs.last_ts = time.time() - 99          # 임계 초과로 늙힌다
    stale = gs.poll()
    checks.append(("무출력이 임계를 넘으면 낡은 값을 버린다(거짓 신선도 ✗)", stale == {}))
    checks.append(("무출력이 임계를 넘으면 자식을 되살린다", gs.respawns >= 1 and gs.proc is not None))
    # 음성대조: 값이 계속 오면 재기동하지 않는다.
    gs2 = GpuStream.__new__(GpuStream)
    import io as _io2
    gs2.proc = _FakeProc(); gs2.proc.stdout = _io2.StringIO("51, 91.0, 1001, 31, 1025\n")
    gs2.last, gs2.interval, gs2.stale_after_s, gs2.respawns, gs2.enabled = "", 1, 5, 0, True
    gs2.last_ts = time.time() - 99
    got = gs2.poll()
    checks.append(("출력이 있으면 임계를 넘겨 늙었어도 재기동하지 않는다(대조군)",
                   got.get("gpu_temp") == 51 and gs2.respawns == 0))

    checks.append(("zone 부재 노드는 SoC 빈 칸(음성대조)", _l2[-2] == "" and _l2[-1] == ""))

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
