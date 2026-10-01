#!/usr/bin/env python3
"""selftest_broad_search_cell_args.py — `broad_search.sh cell` 의 인자 선검사(F3)·권한 승계(F4) 격리 음성대조.

왜 있는가(plan_26100113 H6 · 2026-10-01 실측 두 건):
  F3  `--backend` 누락이 측정기(sweep_bench)의 **측정 전** 거부(rc 2)로 돌아왔는데, cell 은 그 rc 를 측정 결과로
      받아 셀 기록(void)을 남기고 셀 예산을 먹었다.
  F4  판정 호출이 `${AUTHORITY:-explore}` 였다 — init 이 상태 파일에 적은 선언 권한(weak)을 읽지 않고 explore 로
      판정했다. 판정 경로의 침묵 폴백이다.
두 교정은 셸 스크립트의 진입 경로에 서 있으므로, 배포되는 스크립트 바이트 사본을 격리 저장소에서 직접 돌린다.

격리 방식(selftest_broad_search_precheck.py 와 같은 규칙):
  · 임시 git 저장소에 배포 스크립트의 바이트 사본을 같은 상대경로로 둔다(재구현 ✗).
  · 부하 스크립트(`sweep_bench.sh`·`judge_bench.sh`)는 사본이 아니라 **argv 를 기록만 하는 stub** 이다 —
    실제 부하는 구조적으로 열리지 않는다. stub 산출물은 측정값을 흉내 내지 않는다(모킹이 실측인 척하지 않게).
  · `curl` 은 PATH 앞단 shim 이 `200` 을 돌려준다(health 프로브를 지나 측정기 호출 자리까지 가게).

사례(★ = 교정 전 거동이면 FAIL 하는 것):
  N1★ 부하 경로 · --backend 없음        → exit 2 · 측정기·health 미호출 · 셀 기록 0
  N2★ 측정기 stub rc 2(인자/전제 거부)  → exit 2 · 셀 기록 0 · cells_remaining 불변
  N3★ 상태 파일 권한 weak + 판정 노브    → 판정기 stub argv 에 `--authority weak` 와 노브가 그대로 도달
  N4★ cell 에 --authority 를 준다        → exit 2 · 측정기 미호출(init 선언이 유일한 원천)
  N5★ 상태 파일에 rubric_authority 없음  → exit 2 · 측정기 미호출(기본값을 넣지 않는다)
  N6  음성대조: 재조립(--reassemble-only)은 --backend 없이도 막히지 않는다(부하 경로가 아니다)

사용: python3 selftest_broad_search_cell_args.py   (exit 0 = 전부 통과)
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]
AB = ".claude/skills/adversarial-benchmark/scripts"
COPIES = (
    f"{AB}/broad_search.sh",
    f"{AB}/sweep_stop.py",
    f"{AB}/classify_cell.py",
    f"{AB}/repeat_axis.py",
    f"{AB}/verdict_rule.py",
    f"{AB}/roofline.py",
    ".claude/skills/terraforming_node/scripts/campaign_init.py",
    ".claude/skills/terraforming_node/scripts/campaign_template_validator.py",
)
CAMP = "camp-fx"
CELL = "cell-a"
CONFIG = "cfg-a"
NOW = "2026-01-01T00:00:00Z"
# argv 를 한 줄씩 기록하고 환경변수로 받은 rc 로 끝나는 stub. 측정 산출물을 만들지 않는다.
STUB = '#!/bin/sh\nprintf "%s\\n" "$*" >> "$(dirname "$0")/{name}.argv"\nexit "${{{var}:-0}}"\n'


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _build(root: Path, bindir: Path) -> None:
    for rel in COPIES:
        dst = root / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO / rel, dst)
    for name, var in (("sweep_bench", "STUB_SWEEP_RC"), ("judge_bench", "STUB_JUDGE_RC")):
        p = root / AB / f"{name}.sh"
        _write(p, STUB.format(name=name, var=var))
        p.chmod(0o755)
    subprocess.run(["git", "init", "-q", str(root)], check=True, timeout=60)
    camp = root / "campaigns" / CAMP
    _write(root / "campaigns" / "ACTIVE", CAMP + "\n")
    _write(camp / "campaign.yaml", json.dumps({
        "schema_version": 1, "id": CAMP, "plan_ref": "docs/plan/fixture.md",
        "nodes": [{"node_id": "main", "role": "main", "topology": "single"}],
        "assignments": {"main": [{"cell": CELL}]},
        "control_variables": {"model": "m", "vllm_version": "0.0.0", "topology": "single",
                              "target_gpu": "fixture"}}, ensure_ascii=False))
    _write(camp / "cells" / CELL / "lockset.json",
           json.dumps({"id": CELL, "provenance": "hand-authored"}))
    _write(root / "output" / "single" / "envs" / f".env.{CONFIG}", "SERVING_PORT=18080\n")
    sweep = root / "output" / "single" / "benchlog" / f"sweep_{CONFIG}"
    _write(sweep / "sweep_index.json", json.dumps({"meta": {}, "levels": []}))
    _write(sweep / "level_01" / "measured.json", json.dumps({"measurement_ok": True}))
    _write(bindir / "curl", "#!/bin/sh\ntouch \"$(dirname \"$0\")/curl.called\"\nprintf 200\n")
    (bindir / "curl").chmod(0o755)


def _bs(root: Path, env: dict, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["bash", str(root / COPIES[0]), *args], cwd=str(root), env=env,
                          capture_output=True, text=True, timeout=120)


def _init(root: Path, env: dict, name: str, authority: str) -> Path:
    state = root / "campaigns" / CAMP / "sweeps" / f"{name}.json"
    r = _bs(root, env, "init", "--sweep-id", name, "--state", str(state), "--cells", CELL,
            "--control-variable", "fixture", "--max-cells", "10", "--wall-clock-budget-s", "100000",
            "--consecutive-failure-limit", "5", "--declared-by", "selftest", "--basis", "fixture",
            "--authority", authority, "--now-utc", NOW, "--topology", "single")
    if r.returncode != 0:
        raise SystemExit(f"[selftest_broad_search_cell_args] 픽스처 init 실패: {r.stderr}")
    return state


def _cell(root: Path, env: dict, state: Path, *extra: str) -> subprocess.CompletedProcess:
    return _bs(root, env, "cell", "--state", str(state), "--cell-key", CELL, "--config", CONFIG,
               "--axis-citation", "fixture", "--next-intent", "fixture", "--bench-budget-mib", "1",
               "--now-utc", NOW, "--topology", "single", "--ack-uncalibrated-thermal", *extra)


def _state(state: Path) -> dict:
    return json.loads(state.read_text(encoding="utf-8"))


def main() -> int:
    failures: list[str] = []

    def ck(label: str, cond: bool, detail: str = "") -> None:
        print(("  PASS " if cond else "  FAIL ") + label)
        if not cond:
            failures.append(label + (f" — {detail}" if detail else ""))

    if shutil.which("git") is None:
        print("[selftest_broad_search_cell_args] FAIL git 이 없다 — 격리 저장소를 만들 수 없다", file=sys.stderr)
        return 1
    with tempfile.TemporaryDirectory(prefix="bs-cellargs.") as tmp:
        root, bindir = Path(tmp) / "repo", Path(tmp) / "bin"
        _build(root, bindir)
        env = dict(os.environ, PATH=f"{bindir}{os.pathsep}{os.environ.get('PATH', '')}")
        sweep_argv = root / AB / "sweep_bench.argv"
        judge_argv = root / AB / "judge_bench.argv"
        curl_mark = bindir / "curl.called"

        def reset() -> None:
            for p in (sweep_argv, judge_argv, curl_mark):
                p.unlink(missing_ok=True)

        # N1 — 부하 경로 · --backend 없음
        reset()
        st = _init(root, env, "n1", "weak")
        before = _state(st)
        r = _cell(root, env, st, "--confirm-risk")
        ck("N1★ --backend 없음 → exit 2", r.returncode == 2, f"rc={r.returncode} {r.stderr[-400:]}")
        ck("N1★ 측정기·health 프로브 미호출(측정 진입 0)",
           not sweep_argv.exists() and not curl_mark.exists())
        ck("N1★ 상태 파일 불변(셀 기록 0 · 예산 불변)", _state(st) == before)

        # N2 — 측정기 rc 2
        reset()
        st = _init(root, env, "n2", "weak")
        before = _state(st)
        r = _cell(root, dict(env, STUB_SWEEP_RC="2"), st, "--confirm-risk", "--backend", "openai")
        ck("N2★ 측정기 rc 2 → exit 2", r.returncode == 2, f"rc={r.returncode} {r.stderr[-400:]}")
        ck("N2 측정기는 실제로 불렸다(rc 2 를 받은 자리를 시험한다)", sweep_argv.exists())
        after = _state(st)
        ck("N2★ 셀 기록 0 · cells_remaining 불변",
           after.get("cells") == before.get("cells")
           and after.get("cells_remaining") == before.get("cells_remaining"),
           json.dumps({"cells": after.get("cells"), "rem": after.get("cells_remaining")}, ensure_ascii=False)[:300])
        ck("N2 판정기 미호출(측정 결과가 아니다)", not judge_argv.exists())

        # N3 — 권한 승계 + 판정 노브 전달
        reset()
        st = _init(root, env, "n3", "weak")
        r = _cell(root, env, st, "--confirm-risk", "--backend", "openai",
                  "--reference-tps", "53.7", "--e-search", "hit")
        jv = judge_argv.read_text(encoding="utf-8") if judge_argv.exists() else ""
        ck("N3★ 판정기가 상태 파일 권한 weak 를 받는다", "--authority weak" in jv, f"argv={jv!r} rc={r.returncode}")
        ck("N3★ 판정 노브가 그대로 도달한다", "--reference-tps 53.7" in jv and "--e-search hit" in jv, f"argv={jv!r}")
        ck("N3★ explore 폴백이 없다(판정기가 실제로 불린 위에서)", bool(jv) and "--authority explore" not in jv, f"argv={jv!r}")
        ck("N3 stdout 이 권한 출처를 밝힌다", "루브릭 권한 = weak" in r.stdout, r.stdout[-300:])

        # N4 — cell 에 --authority
        reset()
        st = _init(root, env, "n4", "weak")
        r = _cell(root, env, st, "--confirm-risk", "--backend", "openai", "--authority", "explore")
        ck("N4★ cell --authority → exit 2 · 측정기 미호출",
           r.returncode == 2 and not sweep_argv.exists() and "유일한 원천" in r.stderr,
           f"rc={r.returncode} {r.stderr[-300:]}")

        # N5 — 상태 파일 권한 부재
        reset()
        st = _init(root, env, "n5", "weak")
        doc = _state(st)
        doc.pop("rubric_authority", None)
        st.write_text(json.dumps(doc, ensure_ascii=False, indent=2), encoding="utf-8")
        r = _cell(root, env, st, "--confirm-risk", "--backend", "openai")
        ck("N5★ rubric_authority 부재 → exit 2 · 측정기 미호출",
           r.returncode == 2 and not sweep_argv.exists() and "rubric_authority" in r.stderr,
           f"rc={r.returncode} {r.stderr[-300:]}")

        # N6 — 음성대조: 재조립은 --backend 가 없어도 선검사에 막히지 않는다
        reset()
        st = _init(root, env, "n6", "weak")
        r = _cell(root, env, st, "--reassemble-only")
        ck("N6 재조립 · --backend 없음 → 선검사 통과(exit 0)",
           r.returncode == 0 and "--backend 가 필수" not in r.stderr, f"rc={r.returncode} {r.stderr[-400:]}")

    if failures:
        print(f"[selftest_broad_search_cell_args] FAIL {len(failures)}건:")
        for f in failures:
            print("  - " + f)
        return 1
    print("[selftest_broad_search_cell_args] PASS — N1~N6(★ 교정 전 거동이면 FAIL)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
