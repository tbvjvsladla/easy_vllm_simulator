#!/usr/bin/env python3
"""selftest_lite_gate.py — lite 판정(= full 의 진입 게이트)과 lite 등급 인증서의 격리 시험.

근거: plan_26092923 · 딥 인터뷰 interview_20260929_132122 · 시드 seed_2adb796d380e. 촉발은 D1(2026-09-29 · single lite 가
컨테이너 안 클라이언트에 호스트 포트를 줘 측정 0건인데 rc 0 · 표는 N/A) — 측정 불성립이 성공으로 접혔다.

배포되는 `lite_bench.sh`·`lite_metrics.py`·`render_report.py`·`publish_benchmark_record.py` 를 **바이트 사본 그대로**
격리 저장소에서 돌린다(샌드박스는 `selftest_lite_report.Sandbox` 를 그대로 쓴다 — docker·curl·nvidia-smi 는 shim).

  K1★ D1 연결거부(호스트 200 · 클라이언트 평면 미도달 · 결과 파일 없음) → raw lite_verdict=measurement_path_failed · exit 6
  K2★ α(플래그 없음 = 서빙 직후 자동 핸드오프) — 통과든 불통과든 리포트·인증서 0(기록·보고만 · 헌법 트리거 절)
  K3★ ② 서버 응답 실패(결과 파일이 실패 요청·빈 출력) → server_failed · exit 7 · --publish-report 여도 인증서 0
  K4  β lite 통과 + --publish-report → 리포트 1(lite 판정 절) + lite 등급 인증서 1
      (benchmark_mode: lite · verdict: not_applicable · lite_verdict: pass · 강한 6키 · 게이트 파서 ok · 리포트와 같은 stem)
  K5★ 인증서 발행기는 판정 부재·어휘 밖·불통과 raw 로 lite 인증서를 내지 않는다(exit 3 · 부재를 통과로 접지 않는다)
  K6  종료코드 = raw 판정에서 파생(LITE_EXIT) — 같은 raw 를 두 번 읽어도 갈라지지 않는다

종료: 0=PASS · 1=FAIL
"""
import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

SDIR = Path(__file__).resolve().parent
REPO = Path(__file__).resolve().parents[4]


def _load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    sys.path.insert(0, str(path.parent))
    spec.loader.exec_module(mod)
    return mod


def main() -> int:
    failures = []

    def ck(name, cond, detail=""):
        print("  [%s] %s%s" % ("PASS" if cond else "FAIL", name, "" if cond else " — " + str(detail)[-900:]))
        if not cond:
            failures.append(name)

    if shutil.which("git") is None:
        print("[selftest_lite_gate] FAIL git 이 없다 — 격리 저장소를 만들 수 없다", file=sys.stderr)
        return 1
    slr = _load("_gate_slr", SDIR / "selftest_lite_report.py")
    lm = _load("_gate_lm", SDIR / "lite_metrics.py")
    gate = _load("_gate_cg", REPO / ".claude/policies/runtime/completion_gate.py")

    with tempfile.TemporaryDirectory(prefix="lite-gate-selftest.") as td:
        sb = slr.Sandbox(Path(td))

        def raw():
            try:
                return json.loads(sb.raw_path().read_text(encoding="utf-8"))
            except (OSError, ValueError):
                return None

        def clean_docs():
            shutil.rmtree(sb.root / "docs" / "benchmark", ignore_errors=True)

        # ── K1 D1 연결거부 ─────────────────────────────────────────────────────────────────
        sb.env.update(SHIM_BENCH_MODE="refused", SHIM_CLIENT_HEALTH="unreachable")
        cp = sb.lite()
        r = raw() or {}
        ck("K1★ D1 연결거부 → exit 6 · raw lite_verdict=measurement_path_failed · 사유에 클라이언트 평면 미도달",
           cp.returncode == 6 and r.get("lite_verdict") == "measurement_path_failed" and r.get("lite_exit") == 6
           and any("클라이언트가 도는 평면" in x for x in r.get("lite_verdict_reasons") or []),
           (cp.returncode, r.get("lite_verdict"), r.get("lite_verdict_reasons"), cp.stderr[-400:]))
        ck("K1 판정 입력이 raw 에 실린다(요청 rc · 두 평면 health · bench 출력 끝)",
           r.get("bench_cold_rc") == 0 and r.get("host_health_after") == "200"      # 실물: 연결 거부에도 rc 0(D1 의 모양)
           and r.get("client_plane_health") == "unreachable" and "Cannot connect" in (r.get("bench_stderr_tail") or ""), r)
        ck("K2★ α 불통과(플래그 없음) → 리포트·인증서 0(기록·보고만)", sb.reports() == [] and sb.certs() == [],
           (sb.reports(), sb.certs()))
        cp = sb.lite("--publish-report")
        ck("K1★ D1 + --publish-report → exit 6 · 리포트 1(판정 절에 measurement_path_failed) · 인증서 0",
           cp.returncode == 6 and len(sb.reports()) == 1 and sb.certs() == []
           and "| lite_verdict | measurement_path_failed |" in (sb.root / "docs/benchmark" / sb.reports()[0]).read_text(encoding="utf-8"),
           (cp.returncode, sb.reports(), sb.certs(), cp.stderr[-400:]))
        clean_docs()

        # ── K1b 클라이언트 평면이 닿아도 실패가 연결 오류면 ① (결과 파일의 failed 를 서버 실패로 접지 않는다) ─────
        sb.env.update(SHIM_BENCH_MODE="refused", SHIM_CLIENT_HEALTH="200")
        cp = sb.lite()
        r = raw() or {}
        ck("K1b★ 연결 오류 실패 + 클라이언트 평면 200 → 여전히 ① · exit 6(② 로 접지 않는다)",
           cp.returncode == 6 and r.get("lite_verdict") == "measurement_path_failed", (cp.returncode, r.get("lite_verdict_reasons")))

        # ── K3 ② 서버 응답 실패 ────────────────────────────────────────────────────────────
        sb.env.update(SHIM_BENCH_MODE="http500", SHIM_CLIENT_HEALTH="200")
        cp = sb.lite("--publish-report")
        r = raw() or {}
        ck("K3★ 서버가 실패로 응답(failed>0 · 출력 0) → exit 7 · server_failed · 인증서 0",
           cp.returncode == 7 and r.get("lite_verdict") == "server_failed" and sb.certs() == [],
           (cp.returncode, r.get("lite_verdict"), r.get("lite_verdict_reasons"), sb.certs()))
        clean_docs()

        # ── K2·K4 통과 ────────────────────────────────────────────────────────────────────
        sb.env.update(SHIM_BENCH_MODE="ok", SHIM_CLIENT_HEALTH="200")
        cp = sb.lite()
        r = raw() or {}
        ck("K2★ α 통과(플래그 없음) → exit 0 · raw pass · 리포트·인증서 0 · 판정을 출력으로 보고",
           cp.returncode == 0 and r.get("lite_verdict") == "pass" and sb.reports() == [] and sb.certs() == []
           and "lite_verdict: pass" in cp.stdout, (cp.returncode, r.get("lite_verdict_reasons"), cp.stderr[-400:]))
        ck("K6 종료코드 = raw 판정에서 파생(LITE_EXIT) · 두 번 읽어도 같다",
           lm.LITE_EXIT[lm.read_lite_verdict(r)] == cp.returncode == r.get("lite_exit"), r)
        cp = sb.lite("--publish-report")
        certs, reps = sb.certs(), sb.reports()
        ctext = (sb.root / "docs/benchmark" / certs[0]).read_text(encoding="utf-8") if len(certs) == 1 else ""
        fields, ok = gate.parse_flat_certificate(ctext) if ctext else ({}, False)
        ck("K4 β 통과 + --publish-report → exit 0 · 리포트 1 · lite 등급 인증서 1",
           cp.returncode == 0 and len(reps) == 1 and len(certs) == 1, (cp.returncode, reps, certs, cp.stderr[-500:]))
        ck("K4 인증서 = benchmark_mode lite · verdict not_applicable · lite_verdict pass · entry β · 게이트 파서 ok",
           ok and fields.get("benchmark_mode") == "lite" and fields.get("verdict") == "not_applicable"
           and fields.get("lite_verdict") == "pass" and fields.get("entry_path") == "beta_lite_only_cell"
           and fields.get("record_type") == "benchmark_certificate", (ok, fields))
        ck("K4 강한 6키가 채워진다(model·gpu_model·vllm_version·quantization·topology·tp) · 측정 노드 · 측정시각=raw",
           all(fields.get(k) not in (None, "", "N/A") for k in ("model", "gpu_model", "vllm_version", "topology",
                                                               "tensor_parallel_size"))
           and "quantization" in fields and fields.get("measured_node") == "main"
           and fields.get("measured_utc") == (raw() or {}).get("measured_utc"), fields)
        ck("K4 인증서와 리포트가 같은 stem(발행기 바인딩 전제)",
           certs and reps and certs[0][len("benchmark_"):-len(".yaml")] == reps[0][len("bench_report_"):-len(".md")],
           (certs, reps))
        ck("K4 lite 인증서의 run key 가 풀린다(중복 판정·바인딩이 같은 키로 찾는다)",
           ok and gate.certificate_run_key(fields) is not None, fields)

        # ── K5 발행기 음성대조 ────────────────────────────────────────────────────────────
        pbr = str(sb.root / slr.AB / "publish_benchmark_record.py")
        good = raw() or {}
        for label, mut in (("판정 부재", lambda d: d.pop("lite_verdict", None)),
                           ("어휘 밖", lambda d: d.update(lite_verdict="ok")),
                           ("불통과", lambda d: d.update(lite_verdict="server_failed"))):
            bad = dict(good)
            mut(bad)
            p = sb.root / "bad_raw.json"
            p.write_text(json.dumps(bad), encoding="utf-8")
            clean_docs()
            cp = sb.run([sys.executable, pbr, "--lite-raw-json", str(p)])
            ck("K5★ %s raw → 인증서 발행기 exit 3 · 인증서 0" % label, cp.returncode == 3 and sb.certs() == [],
               (cp.returncode, sb.certs(), cp.stderr[-300:]))
        cp = sb.run([sys.executable, pbr, "--lite-raw-json", str(sb.raw_path()), "--sweep-index", "x.json"])
        ck("K5 --lite-raw-json 과 full 인자 혼합 → exit 2", cp.returncode == 2, cp.stderr[-200:])

    if failures:
        print("[selftest_lite_gate] FAIL %d 건: %s" % (len(failures), failures), file=sys.stderr)
        return 1
    print("[selftest_lite_gate] PASS — K1~K6(D1 연결거부 · ② · α 관측 · β lite 인증서 · 발행기 음성대조 · 종료코드 파생)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
