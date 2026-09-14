#!/usr/bin/env python3
"""selftest_broad_search_precheck.py — `broad_search.sh cell` 셀 출처 precheck 의 격리 음성대조.

왜 있는가(plan_26091407 §4.0 · §7 O1): 캠페인 셀 11/11 이 lockset 없이 손작성 yaml 로 측정됐고
그 사실을 가리는 자리가 0 이었다(F1). 교정은 측정 진입에 precheck 하나를 두는 것이다 — 표시
부재·무효는 exit 2, `hand-authored` 는 통과, 선언↔서빙 실물 불일치는 cell.status 에 **기재**.
precheck 는 셸 스크립트 안에 있으므로 단위 함수 시험(`campaign_init --selftest`)만으로는 **그 호출이
실제 진입 경로에 서 있는지**를 증명하지 못한다. 그래서 여기서는 배포되는 스크립트 자체를 돌린다.

격리 방식:
  · 임시 git 저장소에 **배포되는 스크립트의 바이트 사본**을 같은 상대경로로 놓는다(재구현 ✗).
    `broad_search.sh` 는 저장소 루트를 git 에서 파생하므로, 사본은 사본의 저장소만 본다 —
    라이브 `campaigns/ACTIVE`·`output/**` 에 닿지 않는다.
  · 부하 경로는 **구조적으로** 열리지 않는다: `sweep_bench.sh`·`judge_bench.sh` 를 사본에 두지 않고,
    health 프로브의 `curl` 은 PATH 앞단 shim 이 `000` 을 돌려준다(테스트 평면의 외부 의존 차단 —
    정당한 모킹이며 산출물이 실측처럼 보이지 않는다: 셀은 serve_failed 로 기록된다).

사례(양성 = 막혀야 하는 것 · 음성대조 = 통과해야 하는 것):
  A 부하 경로 · lockset 부재           → exit 2, health 프로브 전(shim 미호출)
  B 재조립 경로 · lockset 부재         → exit 2
  C provenance 목록 밖 값             → exit 2
  D 뼈대 복사본(provenance=<<FILL>>)   → exit 2
  E hand-authored                     → exit 0 · 셀 기록·cell.status 에 출처 · gmu 불일치 기재
  F 부하 경로 · hand-authored           → precheck 통과 후 health 프로브까지 진행(shim 호출)
  G explorer-phase2                   → exit 0
  H 캠페인 밖(ACTIVE 부재=_bootstrap)  → exit 0 · not_applicable 을 stderr·셀 기록에 **이름으로** 남김
  I --serve-failed 기록               → exit 0 · not_evaluated(트리플렛 없는 실패 기록은 면제)
  J 판정자(campaign_init.py) 부재       → exit 2, health 프로브 전 · 통과로 접지 않는다
  K 검증기 부재(판정자 예외 rc 1)        → exit 2 '판정 불가' · "hand-authored 로 표시" 안내 ✗(오분류 방지)
  L 포인터 무효(오타) ∧ 상태 파일이 캠페인 안 → 상태 파일의 캠페인으로 판정(부재 → exit 2 · 표시 → 통과)
  M 판정 사유 코드의 마지막 홉  verdict.json 의 reason_code(SPEC_ACCEPT_LEN_MISSING)가 셀 기록 verdict_narrative 에
                                 실린다 · 음성대조: reason_code=null 이면 서술 모양은 종전 그대로(plan_26091407 §4.1)

사용: python3 selftest_broad_search_precheck.py   (exit 0 = 전부 통과)
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
COPIES = (
    ".claude/skills/adversarial-benchmark/scripts/broad_search.sh",
    ".claude/skills/adversarial-benchmark/scripts/sweep_stop.py",
    ".claude/skills/adversarial-benchmark/scripts/classify_cell.py",
    ".claude/skills/terraforming_node/scripts/campaign_init.py",
    ".claude/skills/terraforming_node/scripts/campaign_template_validator.py",
)
# 부하를 거는 스크립트는 사본에 **두지 않는다** — 경로가 잘못 열려도 실행할 대상이 없다.
NEVER_COPY = (
    ".claude/skills/adversarial-benchmark/scripts/sweep_bench.sh",
    ".claude/skills/adversarial-benchmark/scripts/judge_bench.sh",
)
CAMP = "camp-fx"
CELL = "cell-a"
CONFIG = "cfg-a"
NOW = "2026-01-01T00:00:00Z"


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _build_fixture(root: Path, bindir: Path) -> None:
    for rel in COPIES:
        dst = root / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO / rel, dst)
    subprocess.run(["git", "init", "-q", str(root)], check=True, timeout=60)
    camp = root / "campaigns" / CAMP
    _write(root / "campaigns" / "ACTIVE", CAMP + "\n")
    _write(camp / "campaign.yaml", json.dumps({
        "schema_version": 1, "id": CAMP, "plan_ref": "docs/plan/fixture.md",
        "nodes": [{"node_id": "main", "role": "main", "topology": "single"}],
        "assignments": {"main": [{"cell": CELL}]},
        "control_variables": {"model": "m", "vllm_version": "0.0.0", "topology": "single",
                              "target_gpu": "fixture"}}, ensure_ascii=False))
    _write(camp / "cells" / CELL / "config.yaml",
           "target_gpu:\n  gpu_model: fixture\n  target_gmu: 0.85\n")
    _write(root / "output" / "single" / "envs" / f".env.{CONFIG}", "SERVING_PORT=18080\n")
    sweep = root / "output" / "single" / "benchlog" / f"sweep_{CONFIG}"
    _write(sweep / "sweep_index.json", json.dumps(
        {"meta": {"gpu_memory_utilization": "0.80", "image_digest": "sha256:fixture"}, "levels": []}))
    _write(sweep / "level_01" / "measured.json", json.dumps({"measurement_ok": True}))
    # curl shim — 호출되면 표지 파일을 남기고 `000` 을 낸다(health≠200 → serve 미가동 경로).
    _write(bindir / "curl", "#!/bin/sh\ntouch \"$(dirname \"$0\")/curl.called\"\nprintf 000\n")
    (bindir / "curl").chmod(0o755)


def _lockset(root: Path, doc: dict | None) -> None:
    path = root / "campaigns" / CAMP / "cells" / CELL / "lockset.json"
    if doc is None:
        path.unlink(missing_ok=True)
    else:
        _write(path, json.dumps(doc, ensure_ascii=False))


def _bs(root: Path, env: dict, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["bash", str(root / COPIES[0]), *args], cwd=str(root), env=env,
        capture_output=True, text=True, timeout=120)


def _init_state(root: Path, env: dict, camp: str, name: str) -> Path:
    state = root / "campaigns" / camp / "sweeps" / f"{name}.json"
    r = _bs(root, env, "init", "--sweep-id", name, "--state", str(state), "--cells", CELL,
            "--control-variable", "fixture", "--max-cells", "10", "--wall-clock-budget-s", "100000",
            "--consecutive-failure-limit", "5", "--declared-by", "selftest", "--basis", "fixture",
            "--authority", "explore", "--now-utc", NOW, "--topology", "single")
    if r.returncode != 0:
        raise SystemExit(f"[selftest_broad_search_precheck] 픽스처 init 실패: {r.stderr}")
    return state


def _cell(root: Path, env: dict, state: Path, *extra: str) -> subprocess.CompletedProcess:
    return _bs(root, env, "cell", "--state", str(state), "--cell-key", CELL, "--config", CONFIG,
               "--axis-citation", "fixture", "--next-intent", "fixture", "--bench-budget-mib", "1",
               "--now-utc", NOW, "--topology", "single", *extra)


def _record(state: Path) -> dict:
    doc = json.loads(state.read_text(encoding="utf-8"))
    rows = [c for c in doc.get("cells") or [] if c.get("cell_key") == CELL]
    return rows[-1] if rows else {}


def main() -> int:
    failures: list[str] = []

    def ck(label: str, cond: bool, detail: str = "") -> None:
        print(("  PASS " if cond else "  FAIL ") + label)
        if not cond:
            failures.append(label + (f" — {detail}" if detail else ""))

    if shutil.which("git") is None:
        print("[selftest_broad_search_precheck] FAIL git 이 없다 — 격리 저장소를 만들 수 없다"
              "(부재를 통과로 접지 않는다)", file=sys.stderr)
        return 1
    with tempfile.TemporaryDirectory(prefix="bs-precheck.") as tmp:
        root, bindir = Path(tmp) / "repo", Path(tmp) / "bin"
        _build_fixture(root, bindir)
        env = dict(os.environ, PATH=f"{bindir}{os.pathsep}{os.environ.get('PATH', '')}")
        marker = bindir / "curl.called"
        ck("사본에는 부하 스크립트가 없다(경로가 잘못 열려도 실행할 대상이 없다)",
           not any((root / rel).exists() for rel in NEVER_COPY))

        # A — 부하 경로. precheck 는 health 프로브보다 **앞**에 서 있어야 한다.
        _lockset(root, None)
        st = _init_state(root, env, CAMP, "a")
        r = _cell(root, env, st, "--confirm-risk")
        ck("A 부하 경로 · lockset 부재 → exit 2", r.returncode == 2, f"rc={r.returncode} {r.stderr[-400:]}")
        ck("A 거부 메시지가 '셀 materialize 는 explorer 소관' 을 안내한다", "explorer 소관" in r.stderr)
        ck("A 표시 부재는 **거부**로 분류된다('판정 불가' 로 새지 않는다 — 두 분류는 처방이 다르다)",
           "precheck 거부" in r.stderr and "판정 불가(rc=" not in r.stderr, r.stderr[-400:])
        ck("A precheck 가 health 프로브 전에 멈췄다(curl 미호출)", not marker.exists())
        ck("A 거부된 셀은 스윕 기록에 들어가지 않는다", _record(st) == {})

        # B·C·D — 재조립 경로(부하 없음)에서도 같은 precheck 가 선다.
        st = _init_state(root, env, CAMP, "b")
        r = _cell(root, env, st, "--reassemble-only")
        ck("B 재조립 경로 · lockset 부재 → exit 2", r.returncode == 2, f"rc={r.returncode} {r.stderr[-400:]}")
        _lockset(root, {"id": CELL, "provenance": "explorer"})
        r = _cell(root, env, st, "--reassemble-only")
        ck("C provenance 목록 밖 값 → exit 2 · 거부로 분류",
           r.returncode == 2 and "precheck 거부" in r.stderr and "판정 불가(rc=" not in r.stderr,
           f"rc={r.returncode} {r.stderr[-400:]}")
        tpl = json.loads((REPO / "campaigns/_template/cells/_cell/lockset.json").read_text(encoding="utf-8"))
        _lockset(root, dict(tpl, id=CELL))
        r = _cell(root, env, st, "--reassemble-only")
        ck("D 뼈대 복사본(provenance=<<FILL>>) → exit 2", r.returncode == 2,
           f"rc={r.returncode} {r.stderr[-400:]}")

        # E — hand-authored 는 통과하고, 출처와 불일치가 기재된다(차단 ✗).
        _lockset(root, {"id": CELL, "provenance": "hand-authored", "gmu_source": "hand"})
        st = _init_state(root, env, CAMP, "e")
        r = _cell(root, env, st, "--reassemble-only")
        ck("E hand-authored → exit 0(손작성은 금지가 아니라 표시 대상)", r.returncode == 0,
           f"rc={r.returncode} {r.stderr[-600:]}")
        rec = _record(st)
        cp = rec.get("cell_provenance") or {}
        ck("E 스윕 셀 기록에 precheck 판정이 남는다",
           cp.get("status") == "passed" and cp.get("provenance") == "hand-authored", json.dumps(cp))
        cs_path = root / "campaigns" / CAMP / "cells" / CELL / "cell.status.json"
        cs = json.loads(cs_path.read_text(encoding="utf-8")) if cs_path.is_file() else {}
        ck("E cell.status 에 셀 출처가 옮겨진다(writer 가 lockset 을 직접 읽는다)",
           (cs.get("provenance") or {}).get("lockset") == "hand-authored", json.dumps(cs.get("provenance")))
        has_yaml = subprocess.run([shutil.which("python3") or sys.executable, "-c", "import yaml"],
                                  capture_output=True, env=env, timeout=60).returncode == 0
        mm = cs.get("provenance_mismatch")
        if has_yaml:
            ck("E 서빙 gmu 0.80 ≠ 선언 target_gmu 0.85 가 provenance_mismatch[] 에 기재된다",
               isinstance(mm, list) and len(mm) == 1 and mm[0].get("knob") == "gpu_memory_utilization"
               and mm[0].get("declared") == 0.85 and mm[0].get("observed") == 0.8, json.dumps(mm))
        else:
            ck("E (PyYAML 부재 노드) 불일치 대신 '대조 불가' 가 기재된다 — 일치로 접지 않는다",
               mm == [] and any("PyYAML" in g for g in
                                (cs.get("provenance") or {}).get("compare_gaps") or []),
               json.dumps(cs.get("provenance")))
        ck("E 불일치는 셀 결과를 바꾸지 않는다(기재일 뿐)", cs.get("cell_outcome") == rec.get("cell_outcome"))

        # F — 부하 경로에서도 hand-authored 는 precheck 를 통과해 health 프로브까지 간다.
        st = _init_state(root, env, CAMP, "f")
        r = _cell(root, env, st, "--confirm-risk")
        ck("F 부하 경로 · hand-authored → precheck 통과(health 프로브까지 진행 · shim 이 000)",
           r.returncode == 0 and marker.exists() and _record(st).get("cell_outcome") == "serve_failed",
           f"rc={r.returncode} marker={marker.exists()} {r.stderr[-400:]}")

        # G — explorer-phase2 도 통과한다(두 번째 허용 값).
        _lockset(root, {"id": CELL, "provenance": "explorer-phase2"})
        st = _init_state(root, env, CAMP, "g")
        r = _cell(root, env, st, "--reassemble-only")
        ck("G explorer-phase2 → exit 0", r.returncode == 0, f"rc={r.returncode} {r.stderr[-400:]}")

        # H — 캠페인 밖 호출. 막지 않되 **이름으로** 남긴다(조용한 우회 ✗).
        _lockset(root, None)
        (root / "campaigns" / "ACTIVE").unlink()
        st = _init_state(root, env, "_bootstrap", "h")
        r = _cell(root, env, st, "--reassemble-only")
        cp = _record(st).get("cell_provenance") or {}
        ck("H ACTIVE 부재(_bootstrap) → exit 0 · stderr 에 '대상 아님'",
           r.returncode == 0 and "대상 아님" in r.stderr, f"rc={r.returncode} {r.stderr[-400:]}")
        ck("H 셀 기록에 not_applicable 이 남는다", cp.get("status") == "not_applicable", json.dumps(cp))
        _write(root / "campaigns" / "ACTIVE", CAMP + "\n")

        # I — serve_failed 기록은 트리플렛이 없으므로 precheck 를 타지 않는다(과잉차단 ✗).
        st = _init_state(root, env, CAMP, "i")
        r = _cell(root, env, st, "--serve-failed", "fixture: materialize 실패",
                  "--serve-started-utc", NOW)
        cp = _record(st).get("cell_provenance") or {}
        ck("I --serve-failed · lockset 부재 → exit 0(실패 기록은 지도가 실어야 할 정보)",
           r.returncode == 0, f"rc={r.returncode} {r.stderr[-400:]}")
        ck("I 셀 기록에 not_evaluated 가 남는다", cp.get("status") == "not_evaluated", json.dumps(cp))

        # J — 판정자 부재. 부하 경로에서 hand-authored 로 두어, precheck 가 사라지면 curl 까지 가게 한다
        #     (음성대조가 실제로 "판정자 부재 → 통과" 변이를 잡으려면 통과 조건이 갖춰져 있어야 한다).
        _lockset(root, {"id": CELL, "provenance": "hand-authored"})
        ci_copy = root / COPIES[3]
        ci_bytes = ci_copy.read_bytes()
        marker.unlink(missing_ok=True)
        ci_copy.unlink()
        try:
            st = _init_state(root, env, CAMP, "j")
            r = _cell(root, env, st, "--confirm-risk")
            ck("J 판정자(campaign_init.py) 부재 → exit 2 · health 프로브 전(부재를 통과로 접지 않는다)",
               r.returncode == 2 and "판정자 부재" in r.stderr and not marker.exists()
               # 뒤의 '판정 불가' 분기가 대신 막은 것이 아니라 **부재 분기 자체**가 멈췄는가
               and "판정 불가(rc=" not in r.stderr,
               f"rc={r.returncode} marker={marker.exists()} {r.stderr[-400:]}")
        finally:
            ci_copy.write_bytes(ci_bytes)

        # K — 검증기 부재. 판정자는 돌지만 판정하지 못한다(rc 1 · stdout 비어 있음). 출처 표시 문제로
        #     안내하면 운영자는 lockset 에 거짓 라벨을 덮어쓴다 — 분류가 달라야 한다.
        val_copy = root / COPIES[4]
        val_bytes = val_copy.read_bytes()
        marker.unlink(missing_ok=True)
        val_copy.unlink()
        try:
            st = _init_state(root, env, CAMP, "k")
            r = _cell(root, env, st, "--confirm-risk")
            ck("K 검증기 부재 → exit 2 '판정 불가' · health 프로브 전",
               r.returncode == 2 and "판정 불가" in r.stderr and not marker.exists(),
               f"rc={r.returncode} marker={marker.exists()} {r.stderr[-400:]}")
            ck("K 판정 불가를 '출처 표시 없음' 으로 오분류하지 않는다(hand-authored 표시 안내 ✗)",
               "hand-authored 로 표시하라" not in r.stderr and "precheck 거부" not in r.stderr,
               r.stderr[-400:])
        finally:
            val_copy.write_bytes(val_bytes)

        # L — 포인터 무효(오타·purge 잔존) ∧ 상태 파일이 캠페인 안. 포인터를 잃었다는 이유로 캠페인 셀의
        #     게이트가 열리면 조용한 우회다 — 상태 파일의 캠페인으로 판정한다.
        _lockset(root, None)
        st = _init_state(root, env, CAMP, "l")
        _write(root / "campaigns" / "ACTIVE", "camp-typo\n")
        r = _cell(root, env, st, "--reassemble-only")
        ck("L 포인터 무효 ∧ 상태 파일 campaigns/<캠페인>/sweeps · lockset 부재 → exit 2(not_applicable 로 새지 않는다)",
           r.returncode == 2 and "precheck 거부" in r.stderr and "활성 캠페인은" in r.stderr,
           f"rc={r.returncode} {r.stderr[-600:]}")
        _lockset(root, {"id": CELL, "provenance": "hand-authored"})
        r = _cell(root, env, st, "--reassemble-only")
        cp = _record(st).get("cell_provenance") or {}
        ck("L 음성대조: 같은 조건에서 표시가 있으면 통과하고 판정 캠페인은 상태 파일의 캠페인이다",
           r.returncode == 0 and cp.get("status") == "passed" and cp.get("campaign_id") == CAMP,
           f"rc={r.returncode} {json.dumps(cp)} {r.stderr[-400:]}")
        _write(root / "campaigns" / "ACTIVE", CAMP + "\n")

        # M — 판정 사유 코드가 지도 층(셀 기록)까지 올라오는가. 재조립 경로는 sweep 의 verdict.json 을 읽는다.
        verdict_path = root / "output" / "single" / "benchlog" / f"sweep_{CONFIG}" / "verdict.json"
        _lockset(root, {"id": CELL, "provenance": "hand-authored"})
        _write(verdict_path, json.dumps({"verdict": "NEEDS_RUBRIC", "failure_axis": "establish",
                                         "reason_code": "SPEC_ACCEPT_LEN_MISSING",
                                         "rubric": {"authority": "explore", "source": None, "floor": None}}))
        st = _init_state(root, env, CAMP, "m")
        r = _cell(root, env, st, "--reassemble-only")
        narr = _record(st).get("verdict_narrative") or ""
        ck("M verdict.reason_code 가 셀 기록 verdict_narrative 에 실린다(마지막 홉에서 삼키지 않는다)",
           r.returncode == 0 and narr.endswith(" · reason_code=SPEC_ACCEPT_LEN_MISSING")
           and narr.startswith("NEEDS_RUBRIC · authority=explore"), f"rc={r.returncode} {narr!r} {r.stderr[-300:]}")
        _write(verdict_path, json.dumps({"verdict": "PASS", "reason_code": None,
                                         "rubric": {"authority": "explore", "source": "E", "floor": 1.0}}))
        st = _init_state(root, env, CAMP, "m2")
        r = _cell(root, env, st, "--reassemble-only")
        narr = _record(st).get("verdict_narrative") or ""
        ck("M 음성대조: reason_code=null 이면 서술은 종전 모양 그대로(사유 꼬리 없음)",
           r.returncode == 0 and narr == "PASS · authority=explore · source=E · floor=1.0",
           f"rc={r.returncode} {narr!r}")
        verdict_path.unlink()

    if failures:
        print(f"[selftest_broad_search_precheck] FAIL {len(failures)}건:", file=sys.stderr)
        for f in failures:
            print(f"  - {f}", file=sys.stderr)
        return 1
    print("[selftest_broad_search_precheck] PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
