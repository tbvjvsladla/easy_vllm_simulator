#!/usr/bin/env python3
"""selftest_judge_bench.py — `judge_bench.sh --self-test` 의 본문. accept_len 승계 · spec 선언 승계 ·
결손 사유 · 발행 억제를 **배포되는 스크립트 자체**로 격리 실행해 단언한다.

왜 있는가(plan_26091407 §4.1 · §7 O2 · F6): `judge_bench.sh` 는 사람이 `--accept-len` 을 줄 때만
루프라인에 넘겼고, 안 주면 roofline 기본 1.0 이 조용히 낙찰됐다. 그 1.0 은 R_token 만이 아니라
expected_achievable(= realistic_fraction × accept_len × R_fp)에도 곱해져 합격선을 함께 내린다. 교정은
셸 진입 경로 안에 있으므로 판정기 단위 시험(`verdict_rule.py --self-test` T18~)만으로는 **그 승계가 실제
체인에 서 있는지**를 증명하지 못한다 — 그래서 여기서는 셸 스크립트를 실제로 돌린다.

격리 방식(`selftest_broad_search_precheck.py` 선례):
  · 임시 git 저장소에 `judge_bench.sh`·`roofline.py`·`verdict_rule.py` 의 **바이트 사본**을 같은 상대경로로
    둔다(재구현 ✗). judge_bench 는 저장소 루트를 git 에서 파생하므로 사본은 사본의 저장소만 본다.
  · 인증서 발행기(`publish_benchmark_record.py`)는 **사본에 두지 않고 스텁으로 대체**한다 — 실제 발행기는
    `docs/benchmark/` 와 `campaigns/<ACTIVE>/evidence_pointers.json` 에 쓰기 때문이다. 스텁은 호출 사실만
    임시 저장소 안 표지 파일에 남긴다(테스트 평면의 외부 의존 차단 — 정당한 모킹이며 산출물이 인증서처럼
    보이지 않는다).
  · 모델은 헤더만 있는 safetensors 1개(2.73e9 바이트 선언 · GB10 273 GB/s) → R_fp = 100 t/s 정확히.

사례(★ = 음성대조 · 막혀야/뒤집혀야 하는 것):
  J1  승계(measured)            spec on · 실측 2.0 → accept_len_source=measured · R_token 200 · expected 70 · PASS
  J2  사람 명시(declared)        실측 없음 · --accept-len 1.5 → source=declared · R_token 150 · PASS
  J2b 명시가 실측을 덮음          실측 2.0 · --accept-len 1.5 → declared 이되 근거에 shadowed measured=2.0 기재
  J3  spec off                  실측 없음 · 선언 off → source=declared-absent · R_fp 상한 · PASS(결손 아님)
  J4★ spec on ∧ 결손             → NEEDS_RUBRIC · reason_code=SPEC_ACCEPT_LEN_MISSING (1.0 자동 대체 ✗)
  J4b★ spec on ∧ 무효 실측(0.5)   → SPEC_ACCEPT_LEN_MISSING (무효를 1.0 으로 눕히지 않는다)
  J4c★ 무효 명시(--accept-len 0.5) → exit 2 · 판정 산출 없음(--out-dir)
  J4d★ 무효 명시 · **제자리**      → exit 2 · 기존 roofline/verdict 바이트 불변 · 임시 파일 잔재 없음(원자적 교체)
  J5  unknown(지문 없는 과거 sweep) 실측 2.0 → 승계 · 종전 규율 보존 PASS
  J5b★ unknown ∧ 실측 없음        → source=absent(declared-absent ✗) · 종전 동작(R_fp 상한 물리 초과 NEEDS_RUBRIC)
  J6  --out-dir                  explicit PASS → 판정은 out-dir 에만 · sweep 디렉터리 무변 · 발행 억제
  J6b --no-publish(제자리)        explicit PASS → sweep 디렉터리에 판정 · 발행 억제
  J7★ 억제 없음                   explicit PASS → 발행 분기 실제 진입(스텁 호출) — J6·J6b 의 억제가 공허하지 않다
  J8  --dry-run                  승계 인자가 argv 에 드러나고 아무것도 쓰지 않는다
  J9  격리 단언                   argv 의 manifest 가 임시 저장소 안이다
  J10★ roofline 조합 계약 직접    값∧absent/declared-absent · 출처 measured/declared ∧ 값 없음 · NaN → exit 2 ·
                                 값만 → declared · 둘 다 없음 → absent
  J11 선언 off ∧ 실측 spec_on     → 측정 우선: source=measured · R_token 상한 · spec_axis.mismatch=YES(...)
  J11b 선언 off ∧ 명시 1.5        → 상한 R_fp · mismatch 에 declared 기재("1.0 이 정상" 서술 ✗)

사용: python3 selftest_judge_bench.py  (또는 judge_bench.sh --self-test) · 종료 0=전부 통과 · 1=실패
"""
from __future__ import annotations

import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]
SCRIPTS_REL = ".claude/skills/adversarial-benchmark/scripts"
COPIES = ("judge_bench.sh", "roofline.py", "verdict_rule.py")
PUBLISHER = "publish_benchmark_record.py"   # 사본에 두지 않는다 — 스텁으로 대체
STUB_SENTINEL = "SELFTEST_JUDGE_BENCH_PUBLISH_STUB"
MISSING = object()

STUB = '''#!/usr/bin/env python3
# %s — 실제 발행기가 아니다(테스트 평면 격리). docs/·campaigns/ 에 닿지 않고 호출 사실만 남긴다.
import os, sys
root = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", ".."))
with open(os.path.join(root, "PUBLISH_CALLED"), "a", encoding="utf-8") as f:
    f.write(" ".join(sys.argv[1:]) + "\\n")
''' % STUB_SENTINEL


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


class Sandbox:
    def __init__(self, root: Path):
        self.root = root
        self.sdir = root / SCRIPTS_REL
        self.sdir.mkdir(parents=True)
        for name in COPIES:
            shutil.copy2(REPO / SCRIPTS_REL / name, self.sdir / name)
        _write(self.sdir / PUBLISHER, STUB)
        subprocess.run(["git", "init", "-q", str(root)], check=True, timeout=60)
        _write(root / "output/single/manifest.yaml",
               'topology: single\ngpus_per_node: 1\ngpu_model: "NVIDIA GB10"\n')
        self.model = root / "models/fx-dense"
        _write(self.model / "config.json", json.dumps({"hidden_size": 64, "num_hidden_layers": 2}))
        # 헤더만 있는 safetensors: 2.73e9 바이트 선언 → t_weight = 2.73e9 / 273e9 = 10 ms → R_fp = 100.
        header = json.dumps({"w": {"dtype": "F16", "shape": [1365000000],
                                   "data_offsets": [0, 2730000000]}}).encode("utf-8")
        (self.model / "model.safetensors").write_bytes(struct.pack("<Q", len(header)) + header)
        self.marker = root / "PUBLISH_CALLED"

    def sweep_dir(self, cfg: str) -> Path:
        return self.root / "output/single/benchlog" / ("sweep_" + cfg)

    def make_sweep(self, cfg, *, spec_declared=MISSING, k=None, accept_len=None, decode_tps=105.0):
        d = self.sweep_dir(cfg)
        meta = {"model_path": str(self.model), "tensor_parallel_size": "1", "config_name": cfg,
                "model": "fx-dense", "gpu_model": "NVIDIA GB10"}
        if spec_declared is not MISSING:
            meta.update({"spec_declared": spec_declared, "spec_declared_k": k,
                         "spec_declared_source": "fixture"})
        _write(d / "sweep_index.json", json.dumps({"config": cfg, "meta": meta,
                                                   "generated_utc": "2026-01-01T00:00:00Z",
                                                   "levels": [{"level": 1, "status": "ok"}]}))
        spec_on = bool(isinstance(accept_len, (int, float)) and not isinstance(accept_len, bool)
                       and accept_len > 1.05)
        _write(d / "level_01/measured.json", json.dumps({
            "decode_tps": decode_tps, "spec_on": spec_on, "accept_len": accept_len,
            "completed": 16, "failed": 0, "measurement_ok": True}))
        return d

    def judge(self, cfg, *args):
        cp = subprocess.run(["bash", str(self.sdir / "judge_bench.sh"), cfg, "--topology", "single", *args],
                            cwd=self.root, capture_output=True, text=True, timeout=180)
        return cp

    @staticmethod
    def load(path: Path):
        try:
            return json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            return None


def main() -> int:
    failures: list[str] = []

    def check(name, cond, detail=""):
        print("  [%s] %s%s" % ("PASS" if cond else "FAIL", name, "" if cond else " " + str(detail)))
        if not cond:
            failures.append(name)

    with tempfile.TemporaryDirectory(prefix="selftest-judge-bench.") as td:
        sb = Sandbox(Path(td) / "repo")
        explore_e = ("--authority", "explore", "--reference-tps", "110", "--e-search", "hit")

        # ── J1 승계(measured) ────────────────────────────────────────────────────────────────
        d = sb.make_sweep("j1", spec_declared="on", k=3, accept_len=2.0)
        cp = sb.judge("j1", *explore_e)
        r, v = sb.load(d / "roofline.json"), sb.load(d / "verdict.json")
        check("J1 승계 rc=0", cp.returncode == 0, cp.stderr[-400:])
        check("J1 roofline accept_len_source=measured · 근거=판정 레벨 measured.json",
              r and r.get("accept_len_source") == "measured" and r.get("accept_len") == 2.0
              and (r.get("accept_len_evidence") or "").startswith("level_01/measured.json"), r)
        check("J1 R_fp 100 · R_token 200 · expected_achievable 70 (accept_len 이 합격선에도 곱해진다)",
              r and r.get("R_fp") == 100.0 and r.get("R_token") == 200.0
              and r.get("expected_achievable") == 70.0, r and {k: r.get(k) for k in
                                                               ("R_fp", "R_token", "expected_achievable")})
        check("J1 verdict PASS · R_token 상한 · spec_declared=on 승계",
              v and v.get("verdict") == "PASS" and v.get("reason_code") is None
              and v["rubric"]["physical"]["limit_tps"] == 200.0
              and v["rubric"]["spec_axis"]["declared"] == "on"
              and v["rubric"]["floor"] > 0 and v["rubric"]["ratio_M_over_primary"] is not None,
              v and {"verdict": v.get("verdict"), "physical": (v.get("rubric") or {}).get("physical")})
        check("J1 explore PASS 는 발행 분기에 들지 않는다", not sb.marker.exists())
        # J9 격리 단언 — 사본이 사본의 저장소만 본다.
        check("J9 argv 의 manifest 는 임시 저장소 안이다",
              str(sb.root / "output/single/manifest.yaml") in cp.stdout and str(REPO) + "/output" not in cp.stdout,
              cp.stdout[:300])

        # ── J2 사람 명시(declared) ───────────────────────────────────────────────────────────
        d = sb.make_sweep("j2", spec_declared="on", k=3, accept_len=None)
        cp = sb.judge("j2", *explore_e, "--accept-len", "1.5")
        r, v = sb.load(d / "roofline.json"), sb.load(d / "verdict.json")
        check("J2 declared — 출처가 사람 명시로 드러나고 R_token 150 으로 PASS",
              cp.returncode == 0 and r and r.get("accept_len_source") == "declared"
              and r.get("accept_len_evidence") == "argv(--accept-len)" and r.get("R_token") == 150.0
              and v and v.get("verdict") == "PASS"
              and v["rubric"]["spec_axis"]["accept_len_source"] == "declared",
              (cp.returncode, r and r.get("accept_len_source"), v and v.get("verdict")))

        # ── J2b 명시가 실측을 덮음 — 덮인 실측이 근거에 남는다 ─────────────────────────────────
        d = sb.make_sweep("j2b", spec_declared="on", k=3, accept_len=2.0)
        cp = sb.judge("j2b", *explore_e, "--accept-len", "1.5")
        r = sb.load(d / "roofline.json")
        check("J2b 명시 1.5 가 실측 2.0 을 덮으면 source=declared · R_token 150 · 근거에 shadowed measured=2.0",
              cp.returncode == 0 and r and r.get("accept_len_source") == "declared" and r.get("R_token") == 150.0
              and "shadowed measured=2.0(level_01/measured.json)" in (r.get("accept_len_evidence") or ""),
              (cp.returncode, r and r.get("accept_len_evidence"), cp.stderr[-300:]))

        # ── J3 spec off ─────────────────────────────────────────────────────────────────────
        d = sb.make_sweep("j3", spec_declared="off", accept_len=None, decode_tps=60.0)
        cp = sb.judge("j3", "--authority", "explore", "--e-search", "empty")
        r, v = sb.load(d / "roofline.json"), sb.load(d / "verdict.json")
        check("J3 spec off — declared-absent · R_token==R_fp · R_fp 상한 PASS(결손 아님)",
              cp.returncode == 0 and r and r.get("accept_len_source") == "declared-absent"
              and r.get("R_token") == r.get("R_fp") == 100.0 and r.get("expected_achievable") == 35.0
              and v and v.get("verdict") == "PASS" and v.get("reason_code") is None
              and v["rubric"]["physical"]["limit_source"].startswith("R_fp")
              and v["rubric"]["spec_axis"]["missing"] is False,
              (cp.returncode, r and r.get("accept_len_source"), v and v.get("verdict"), cp.stderr[-300:]))

        # ── J4★ spec on ∧ 결손 ───────────────────────────────────────────────────────────────
        d = sb.make_sweep("j4", spec_declared="on", k=3, accept_len=None)
        cp = sb.judge("j4", *explore_e)
        r, v = sb.load(d / "roofline.json"), sb.load(d / "verdict.json")
        check("J4★ spec on ∧ 결손 → NEEDS_RUBRIC · SPEC_ACCEPT_LEN_MISSING (1.0 자동 대체 ✗)",
              cp.returncode == 0 and r and r.get("accept_len_source") == "absent"
              and v and v.get("verdict") == "NEEDS_RUBRIC" and v.get("failure_axis") == "establish"
              and v.get("reason_code") == "SPEC_ACCEPT_LEN_MISSING" and v["rubric"]["floor"] is None
              and "reason_code=SPEC_ACCEPT_LEN_MISSING" in cp.stdout,
              (cp.returncode, r and r.get("accept_len_source"), v and v.get("verdict"),
               v and v.get("reason_code")))

        # ── J4b★ spec on ∧ 무효 실측 ─────────────────────────────────────────────────────────
        d = sb.make_sweep("j4b", spec_declared="on", k=3, accept_len=0.5)
        cp = sb.judge("j4b", *explore_e)
        r, v = sb.load(d / "roofline.json"), sb.load(d / "verdict.json")
        check("J4b★ 무효 실측 0.5 → absent(근거에 invalid) · SPEC_ACCEPT_LEN_MISSING",
              cp.returncode == 0 and r and r.get("accept_len_source") == "absent"
              and "invalid" in (r.get("accept_len_evidence") or "")
              and v and v.get("reason_code") == "SPEC_ACCEPT_LEN_MISSING",
              (cp.returncode, r and r.get("accept_len_evidence"), v and v.get("reason_code")))

        # ── J4c★ 무효 명시값 → exit 2 ────────────────────────────────────────────────────────
        out4c = sb.root / "rejudge/j4c"
        cp = sb.judge("j4", *explore_e, "--accept-len", "0.5", "--out-dir", str(out4c))
        check("J4c★ 무효 명시 --accept-len 0.5 → exit 2 · 판정 산출 없음",
              cp.returncode == 2 and not (out4c / "verdict.json").exists(),
              (cp.returncode, cp.stderr[-300:]))

        # ── J4d★ 같은 무효 명시를 **제자리**에서 — 정본 산출물 짝이 찢어지지 않는다 ────────────────
        d = sb.sweep_dir("j4")
        before = {n: (d / n).read_bytes() for n in ("roofline.json", "verdict.json")}
        cp = sb.judge("j4", *explore_e, "--accept-len", "0.5")
        after = {n: ((d / n).read_bytes() if (d / n).exists() else None) for n in before}
        leftovers = sorted(x.name for x in d.iterdir() if ".partial." in x.name)
        check("J4d★ 제자리 무효 명시 → exit 2 · 기존 roofline/verdict 바이트 불변 · 임시 파일 잔재 없음",
              cp.returncode == 2 and all(len(b) > 0 for b in before.values()) and after == before
              and not leftovers,
              (cp.returncode, {n: (len(before[n]), len(after[n] or b"")) for n in before}, leftovers))

        # ── J5 unknown(지문 없는 과거 sweep) ─────────────────────────────────────────────────
        d = sb.make_sweep("j5", accept_len=2.0)
        cp = sb.judge("j5", *explore_e)
        r, v = sb.load(d / "roofline.json"), sb.load(d / "verdict.json")
        check("J5 unknown — 실측은 여전히 승계 · 종전 규율(measured.spec_on) PASS",
              cp.returncode == 0 and r and r.get("accept_len_source") == "measured"
              and v and v.get("verdict") == "PASS" and v["rubric"]["spec_axis"]["declared"] == "unknown",
              (cp.returncode, r and r.get("accept_len_source"), v and v.get("verdict")))

        # ── J5b★ unknown ∧ 실측 없음 → 종전 동작 보존(결손을 가르지 못한다) ─────────────────────
        d = sb.make_sweep("j5b", accept_len=None)
        cp = sb.judge("j5b", *explore_e)
        r, v = sb.load(d / "roofline.json"), sb.load(d / "verdict.json")
        check("J5b★ unknown ∧ 실측 없음 → 사유코드 없이 물리 초과 NEEDS_RUBRIC(지문 없으면 결손 식별 불가)",
              cp.returncode == 0 and v and v.get("verdict") == "NEEDS_RUBRIC" and v.get("reason_code") is None
              and v["rubric"]["physical"]["exceeds"] is True,
              (cp.returncode, v and v.get("verdict"), v and v.get("reason_code")))
        # 모르는 선언의 1.0 은 자리표시자다 — declared-absent 로 적히면 E2E 술어 ④ 가 결손을 통과시킨다.
        check("J5b★ unknown ∧ 실측 없음의 roofline 출처는 absent 다(declared-absent ✗)",
              r and r.get("accept_len_source") == "absent", r and r.get("accept_len_source"))

        explicit_pass = ("--authority", "explicit", "--target-tps", "90")

        # ── J6 --out-dir: 원 판정 자리 무변 · 발행 억제 ───────────────────────────────────────
        d = sb.make_sweep("j6", spec_declared="on", k=3, accept_len=2.0)
        out6 = sb.root / "rejudge/j6"
        cp = sb.judge("j6", *explicit_pass, "--out-dir", str(out6))
        v6 = sb.load(out6 / "verdict.json")
        check("J6 --out-dir — 판정은 out-dir 에만 · sweep 디렉터리에 roofline/verdict 없음 · 발행 억제",
              cp.returncode == 0 and v6 and v6.get("verdict") == "PASS"
              and (out6 / "roofline.json").is_file()
              and not (d / "roofline.json").exists() and not (d / "verdict.json").exists()
              and not sb.marker.exists() and "인증서 발행 억제" in cp.stdout,
              (cp.returncode, v6 and v6.get("verdict"), sb.marker.exists(), cp.stdout[-300:]))

        # ── J6b --no-publish(제자리) ─────────────────────────────────────────────────────────
        d = sb.make_sweep("j6b", spec_declared="on", k=3, accept_len=2.0)
        cp = sb.judge("j6b", *explicit_pass, "--no-publish")
        check("J6b --no-publish — 제자리 판정 · 발행 억제",
              cp.returncode == 0 and (sb.load(d / "verdict.json") or {}).get("verdict") == "PASS"
              and not sb.marker.exists() and "인증서 발행 억제" in cp.stdout,
              (cp.returncode, sb.marker.exists()))

        # ── J7★ 억제 없음 → 발행 분기 실제 진입(스텁) ─────────────────────────────────────────
        d = sb.make_sweep("j7", spec_declared="on", k=3, accept_len=2.0)
        cp = sb.judge("j7", *explicit_pass)
        called = sb.marker.read_text(encoding="utf-8") if sb.marker.exists() else ""
        check("J7★ 억제 없는 explicit PASS 는 발행기를 부른다(J6·J6b 억제가 공허하지 않다)",
              cp.returncode == 0 and str(d / "verdict.json") in called
              and STUB_SENTINEL in (sb.sdir / PUBLISHER).read_text(encoding="utf-8"),
              (cp.returncode, called[:200]))

        # ── J8 --dry-run ────────────────────────────────────────────────────────────────────
        d = sb.make_sweep("j8", spec_declared="on", k=3, accept_len=2.0)
        out8 = sb.root / "rejudge/j8"
        cp = sb.judge("j8", *explore_e, "--dry-run", "--out-dir", str(out8))
        check("J8 --dry-run — 승계 인자가 argv 에 드러나고 판정 산출물도 out-dir 자리도 만들지 않는다",
              cp.returncode == 0 and "--accept-len-source measured" in cp.stdout
              and "--spec-declared on" in cp.stdout and not (d / "roofline.json").exists()
              and not out8.exists(),
              (cp.returncode, out8.exists(), cp.stdout[-300:]))

        # ── J10★ roofline 조합 계약 — 사본 roofline.py 를 직접 부른다(judge_bench 는 모순 조합을 만들지 않는다) ──
        roof_base = ["python3", str(sb.sdir / "roofline.py"), "--model-path", str(sb.model),
                     "--manifest", str(sb.root / "output/single/manifest.yaml"), "--tp", "1", "--json"]

        def roof(*extra):
            return subprocess.run([*roof_base, *extra], cwd=sb.root, capture_output=True, text=True, timeout=120)

        for label, extra in (("값 2 ∧ 출처 absent", ("--accept-len", "2", "--accept-len-source", "absent")),
                             ("값 2 ∧ 출처 declared-absent",
                              ("--accept-len", "2", "--accept-len-source", "declared-absent")),
                             ("출처 measured ∧ 값 없음", ("--accept-len-source", "measured")),
                             ("출처 declared ∧ 값 없음", ("--accept-len-source", "declared")),
                             ("값 nan ∧ 출처 measured", ("--accept-len", "nan", "--accept-len-source", "measured"))):
            cp = roof(*extra)
            check("J10★ roofline %s → exit 2" % label, cp.returncode == 2 and "[roofline] ERROR" in cp.stderr,
                  (cp.returncode, cp.stdout[:120], cp.stderr[-200:]))
        cp = roof("--accept-len", "2")
        j = json.loads(cp.stdout) if cp.returncode == 0 else {}
        check("J10 roofline 값만(--accept-len 2) → declared · R_token 200 (종전 호출 호환)",
              cp.returncode == 0 and j.get("accept_len_source") == "declared" and j.get("R_token") == 200.0,
              (cp.returncode, j.get("accept_len_source"), cp.stderr[-200:]))
        cp = roof()
        j = json.loads(cp.stdout) if cp.returncode == 0 else {}
        check("J10 roofline 값·출처 모두 없음 → absent · accept_len 1.0(자리표시자)",
              cp.returncode == 0 and j.get("accept_len_source") == "absent" and j.get("accept_len") == 1.0,
              (cp.returncode, j.get("accept_len_source"), cp.stderr[-200:]))

        # ── J11 선언 off ∧ 실측 spec_on → 측정 우선(체인) ──────────────────────────────────────
        d = sb.make_sweep("j11", spec_declared="off", accept_len=2.0)
        cp = sb.judge("j11", *explore_e)
        r, v = sb.load(d / "roofline.json"), sb.load(d / "verdict.json")
        ax = ((v or {}).get("rubric") or {}).get("spec_axis") or {}
        check("J11 선언 off ∧ 실측 2.0 → source=measured · R_token 200 · 상한 R_token · mismatch=YES",
              cp.returncode == 0 and r and r.get("accept_len_source") == "measured" and r.get("R_token") == 200.0
              and ax.get("limit") == "R_token" and (ax.get("mismatch") or "").startswith("YES")
              and v["rubric"]["physical"]["limit_tps"] == 200.0,
              (cp.returncode, r and r.get("accept_len_source"), ax, cp.stderr[-300:]))

        # ── J11b 선언 off ∧ 사람 명시 1.5 → 상한 R_fp · 명시가 곱해진 사실 기재 ─────────────────────
        d = sb.make_sweep("j11b", spec_declared="off", accept_len=None, decode_tps=60.0)
        cp = sb.judge("j11b", "--authority", "explore", "--e-search", "empty", "--accept-len", "1.5")
        v = sb.load(d / "verdict.json")
        ax = ((v or {}).get("rubric") or {}).get("spec_axis") or {}
        check("J11b 선언 off ∧ 명시 1.5 → 상한 R_fp · mismatch 에 accept_len_source=declared · '정상' 서술 ✗",
              cp.returncode == 0 and ax.get("limit") == "R_fp" and "정상" not in (ax.get("basis") or "")
              and "accept_len_source=declared" in (ax.get("mismatch") or ""),
              (cp.returncode, ax, cp.stderr[-300:]))

    if failures:
        sys.stderr.write("[judge_bench --self-test] FAIL %d 건: %s\n" % (len(failures), failures))
        return 1
    print("[judge_bench --self-test] OK — J1~J11 전부 통과(승계·명시·spec off·결손·원자적 교체·조합 계약·"
          "발행 억제 음성대조 포함)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
