#!/usr/bin/env python3
"""selftest_bench_preset.py — full-bench knob 프리셋 계약의 결정론 거부 묶음 (plan_26101011 · Step 4)

왜 있는가: "값만 조정, 키 추가·삭제는 계약 변경" 과 "등급 = 누가 선택지를 여나" 는 산문으로 적으면 아무도
집행하지 않는다. 이 묶음이 리뷰 시점의 집행자이고(verify_distribution 이 부른다), 실행 시점의 집행자는
sweep_bench.sh 가 부하 전에 부르는 `bench_preset.py resolve` 다. 여기서는 그 둘이 **같은 함수**를 쓴다.

무엇을 치나
  K  스키마 고정 — 닫힌 키 표가 스냅샷과 같고, 배포되는 default_preset 이 그 표를 정확히 채운다
  V  로더 거부 — 미지/누락 키 · 꺼 둔 다른 도구 블록 · 열거 밖 기본값 · 미정의 근거 코드 · user knob 비기본 ≠ escalate ·
     repeats < full 하한 · 기본값 없음 knob 에 값 · 바인딩 모양/폴더 부재 · bench 불일치
  R  해소 거부·기록 — 상한 필터 · 근거 코드 필수 · confirm 사전 승인 · escalate 는 사람 핀만 · 열거 밖 핀 거부 ·
     캠페인 핀 우선/충돌 거부 · CLI 핀 > 캠페인 핀 · bench_pins.repeats 두 번째 자리 거부 · 필수값 부재 · context 초과 ·
     반복 수 귀속 · dry-run 상한 부재 · 커스텀 프리셋 이름/경로 선택과 검증
GPU·docker·NAS 불요.
"""
from __future__ import annotations

import copy
import json
import shutil
import sys
import tempfile
from pathlib import Path

import yaml

SDIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SDIR))
import bench_preset as bp  # noqa: E402
import repeat_axis as ra  # noqa: E402

# 스키마 스냅샷(tripwire). 바뀌면 이 줄과 프리셋·문서가 함께 리뷰돼야 한다.
SNAP_TOP = ("schema_version", "bench", "tool", "reason_codes", "knobs")
SNAP_KNOB = ("owner", "kind", "default", "serving_cap", "options", "reasons")
SNAP_KNOBS = ("levels", "input_len", "output_len", "num_prompts", "warmups", "repeats", "backend", "bench_budget_mib",
              # 데이터셋 축(2026-10-16 · plan_26101614)
              "dataset_kind", "dataset_source", "dataset_id", "dataset_path", "dataset_revision", "source_sha256",
              "column_map", "preprocess_strategy", "seed")


def run() -> int:
    fails = []

    def ck(label, cond, detail=""):
        print("  [%s] %s%s" % ("PASS" if cond else "FAIL", label, "" if cond else " — %s" % (detail,)))
        if not cond:
            fails.append(label)

    def rejects(fn, *a, **kw):
        try:
            fn(*a, **kw)
        except bp.PresetError as exc:
            return str(exc)
        return None

    path, base, bindings = bp.load_preset(None)
    # K — 스키마 고정
    ck("K1 닫힌 키 표 = 스냅샷", bp.TOP_KEYS == SNAP_TOP and bp.KNOB_KEYS == SNAP_KNOB
       and tuple(bp.KNOB_SPECS) == SNAP_KNOBS)
    ck("K2 배포 default_preset 통과 · 키 집합이 표와 정확히 같다",
       set(base) == set(SNAP_TOP) and set(base["knobs"]) == set(SNAP_KNOBS)
       and all(set(k) == set(SNAP_KNOB) for k in base["knobs"].values()))
    ck("K3 바인딩 full-bench → 존재하는 도구 폴더", bindings == {"full-bench": "guide-llm"}, bindings)
    # 캠페인 스키마의 bench_pins 키 목록은 정적 파일끼리라 한쪽이 다른 쪽을 생성할 수 없다 — 교차검증이 차선(workflow.md §결정론 규율).
    schema = json.loads((ra.REPO_ROOT / ra.SCHEMA_REL).read_text(encoding="utf-8"))
    pn = (schema["definitions"]["budgets"]["properties"].get("bench_pins") or {}).get("propertyNames", {}).get("enum")
    ck("K4 campaign.schema budgets.bench_pins 키 = knob 표 − repeats(반복 수 핀은 budgets.repeats 하나)",
       pn is not None and set(pn) == set(bp.KNOB_SPECS) - {"repeats"}, pn)

    def viol(mut):
        d = copy.deepcopy(base)
        mut(d)
        return bp.validate_preset(d, bindings)

    # V — 로더 거부
    ck("V1 미지 최상위 키(꺼 둔 vllm-bench 블록) 거부", viol(lambda d: d.__setitem__("vllm_bench", {"burst_n": 3})))
    ck("V2 knob 누락 거부", viol(lambda d: d["knobs"].pop("warmups")))
    ck("V3 knob 미지 키 거부", viol(lambda d: d["knobs"]["levels"].__setitem__("note", "x")))
    ck("V4 기본값이 열거 밖 거부", viol(lambda d: d["knobs"]["input_len"].__setitem__("default", 1000)))
    ck("V5 미정의 근거 코드 참조 거부", viol(lambda d: d["knobs"]["input_len"]["reasons"].append("vibes")))
    ck("V6 user knob 비기본 값이 auto 면 거부",
       viol(lambda d: d["knobs"]["output_len"]["options"].__setitem__(0, {"value": 128, "grade": "auto"})))
    ck("V7 repeats 선택지 < full 하한 거부",
       viol(lambda d: d["knobs"]["repeats"]["options"].append({"value": ra.FULL_REPEATS_MIN - 1, "grade": "escalate"})))
    ck("V8 기본값 없음 knob(backend)에 값 거부", viol(lambda d: d["knobs"]["backend"].__setitem__("default", "openai")))
    ck("V9 bench 불일치 거부", viol(lambda d: d.__setitem__("bench", "lite-bench")))
    ck("V10 기본값이 confirm 이면 거부",
       viol(lambda d: d["knobs"]["input_len"].__setitem__("default", 16384)))
    ck("V11 잘못된 등급 이름 거부",
       viol(lambda d: d["knobs"]["levels"]["options"].__setitem__(0, {"value": 1, "grade": "maybe"})))
    ck("V12 levels 기본값에 판정점 1 없음 거부", viol(lambda d: d["knobs"]["levels"].__setitem__("default", [2, 4])))
    ck("V13 허용되지 않은 상한 필터 거부",
       viol(lambda d: d["knobs"]["output_len"].__setitem__("serving_cap", bp.CAP_LE_MAX_NUM_SEQS)))

    with tempfile.TemporaryDirectory(prefix="bench-preset.") as td:
        td = Path(td)
        # 격리 스킬 디렉터리(바인딩·프리셋 사본) — 배포 파일을 건드리지 않는다.
        sk = td / "skill"
        shutil.copytree(bp.SKILL_DIR / "config", sk / "config")
        (sk / bp.BINDINGS_REL).write_text("full-bench: guide-llm\nlite-bench: vllm-bench\n", encoding="utf-8")
        ck("V14 바인딩 키 추가 거부", rejects(bp.load_bindings, sk))
        (sk / bp.BINDINGS_REL).write_text("full-bench: no-such-tool\n", encoding="utf-8")
        ck("V15 바인딩이 없는 폴더를 가리키면 거부", rejects(bp.load_bindings, sk))
        (sk / bp.BINDINGS_REL).write_text("full-bench: guide-llm\n", encoding="utf-8")
        custom = copy.deepcopy(base)
        custom["knobs"]["input_len"]["default"] = 8192
        (sk / "config/guide-llm/long_ctx.yaml").write_text(yaml.safe_dump(custom, allow_unicode=True), encoding="utf-8")
        p, d, _ = bp.load_preset("long_ctx", sk)
        ck("R14a 커스텀 프리셋 이름으로 선택 · 같은 검증 통과", p.name == "long_ctx.yaml" and d["knobs"]["input_len"]["default"] == 8192)
        p2, _d, _ = bp.load_preset(str(sk / "config/guide-llm/long_ctx.yaml"), sk)
        ck("R14b 커스텀 프리셋 경로로 선택", p2 == (sk / "config/guide-llm/long_ctx.yaml").resolve())
        bad = copy.deepcopy(custom)
        bad["knobs"]["input_len"]["extra"] = 1
        (sk / "config/guide-llm/bad.yaml").write_text(yaml.safe_dump(bad, allow_unicode=True), encoding="utf-8")
        ck("R14c 계약 위반 커스텀 프리셋은 선택되지 않는다", rejects(bp.load_preset, "bad", sk))
        ck("R14d 없는 프리셋 이름 거부", rejects(bp.load_preset, "nope", sk))

        # 격리 저장소(캠페인 해소자 사본) — 캠페인 핀을 바꿔 가며 묻는다.
        root = td / "repo"
        ci = root / ra.CAMPAIGN_INIT_REL
        ci.parent.mkdir(parents=True)
        ci.write_bytes((ra.REPO_ROOT / ra.CAMPAIGN_INIT_REL).read_bytes())
        camps = root / "campaigns"
        camps.mkdir()

        def set_campaign(budgets):
            if budgets is None:
                (camps / "ACTIVE").unlink(missing_ok=True)
                return
            (camps / "camp-x").mkdir(exist_ok=True)
            (camps / "camp-x" / "campaign.yaml").write_text(json.dumps({"id": "camp-x", "budgets": budgets}),
                                                            encoding="utf-8")
            (camps / "ACTIVE").write_text("camp-x\n", encoding="utf-8")

        SERV = {"max_num_seqs": 2, "max_model_len": 4096, "source": "declared(fixture)"}
        NEED = {"backend": "openai"}
        RS = {"backend": "harmony_completion"}

        def res(cli=None, reasons=None, serving=SERV, tool="vllm", **kw):
            c = dict(NEED)
            c.update(cli or {})
            r = dict(RS)
            r.update(reasons or {})
            return bp.resolve(base, "default_preset", tool=tool, cli=c, knob_reasons=r, serving=serving,
                              repo_root=root, **kw)

        set_campaign(None)
        v, rec = res()
        k = rec["knobs"]
        ck("R1 기본값 + 상한 필터: max-num-seqs 2 → levels [1,2] · 4·8·16 제외 기록",
           v["levels"] == [1, 2] and [f["value"] for f in k["levels"]["filtered_out"]] == [4, 8, 16]
           and k["levels"]["source"] == "preset" and k["levels"]["reason"] == "preset_default", k["levels"])
        ck("R1b 반복 수 = 프리셋 기본 3 · repeat_axis 출처", v["repeats"] == 3 and k["repeats"]["source"] == "preset"
           and k["repeats"]["repeats_source"].startswith("defaulted(preset default_preset"), k["repeats"])
        ck("R2 Agent 비기본 선택에 근거 코드 없음 → 거부", rejects(res, {"input_len": "2048"}))
        v, rec = res({"input_len": "2048"}, {"input_len": "serving_context"})
        ck("R2b 근거 코드 동반 Agent 선택 → source agent · reason 기록",
           v["input_len"] == 2048 and rec["knobs"]["input_len"]["source"] == "agent"
           and rec["knobs"]["input_len"]["reason"] == "serving_context")
        big = dict(SERV, max_model_len=65536)
        ck("R3 confirm(16384)을 승인 없이 Agent 가 고름 → 거부",
           rejects(res, {"input_len": "16384"}, {"input_len": "serving_context"}, serving=big))
        v, rec = res({"input_len": "16384"}, {"input_len": "serving_context"}, serving=big,
                     approved_by="사용자(plan 승인 팝업 전사) — \"승인\"")
        ck("R3b confirm + 사전 승인 전사 → 통과 · approved_by 기록",
           v["input_len"] == 16384 and "승인" in rec["knobs"]["input_len"]["approved_by"])
        ck("R4 escalate(32768)를 Agent 가 고름 → 거부(승인 전사가 있어도)",
           rejects(res, {"input_len": "32768"}, {"input_len": "serving_context"}, serving=dict(SERV, max_model_len=65536),
                   approved_by="x"))
        v, rec = res(serving=dict(SERV, max_model_len=65536), cli_pins={"input_len": "32768"},
                     pinned_by="사용자 — \"32k 로\"")
        ck("R4b escalate 를 사람 핀(--pinned-by)이 연다 · reason human_pin",
           v["input_len"] == 32768 and rec["knobs"]["input_len"]["source"] == "cli_pin"
           and rec["knobs"]["input_len"]["reason"] == "human_pin")
        ck("R5 열거 밖 값은 사람 핀이어도 거부", rejects(res, cli_pins={"input_len": "3000"}, pinned_by="사용자"))
        ck("R5b 귀속 없는 --pin 거부", rejects(res, cli_pins={"input_len": "2048"}))
        ck("R5c Agent 값과 사람 핀이 다르면 거부",
           rejects(res, {"input_len": "1024"}, {"input_len": "prior_cell_shape"}, cli_pins={"input_len": "2048"},
                   pinned_by="사용자"))
        ck("R15 user knob(output_len 512)을 Agent 가 고름 → 거부(escalate)",
           rejects(res, {"output_len": "512"}, {"output_len": "preset_default"}))
        set_campaign({"bench_pins": {"input_len": 2048}})
        v, rec = res()
        ck("R6 캠페인 핀 → source campaign · reason human_pin",
           v["input_len"] == 2048 and rec["knobs"]["input_len"]["source"] == "campaign"
           and rec["campaign"] == "camp-x")
        ck("R6b Agent 선택이 캠페인 핀과 다르면 거부(조용히 덮어쓰지 않는다)",
           rejects(res, {"input_len": "1024"}, {"input_len": "prior_cell_shape"}))
        v, rec = res(serving=dict(SERV, max_model_len=8192), cli_pins={"input_len": "4096"}, pinned_by="사용자 — \"4k\"")
        ck("R7 CLI 사람 핀 > 캠페인 핀 · 덮은 값 기록",
           v["input_len"] == 4096 and rec["knobs"]["input_len"].get("overrode_campaign_pin") == 2048)
        set_campaign({"bench_pins": {"repeats": 4}})
        ck("R8 bench_pins.repeats(두 번째 자리) 거부", rejects(res))
        set_campaign({"bench_pins": {"no_such": 1}})
        ck("R8b bench_pins 미지 knob 거부", rejects(res))
        set_campaign({"repeats": 5})
        v, rec = res()
        ck("R11 캠페인 budgets.repeats 5 → source campaign(escalate 를 사람 선언이 연다)",
           v["repeats"] == 5 and rec["knobs"]["repeats"]["source"] == "campaign")
        set_campaign(None)
        ck("R11b 반복 수 4 를 귀속 없이 → Agent 선택 · escalate 거부", rejects(res, {"repeats": "4"}))
        v, rec = res({"repeats": "4"}, repeats_source="sweep state declared_budget.repeats")
        ck("R11c --repeats-source 귀속 → cli_pin", v["repeats"] == 4 and rec["knobs"]["repeats"]["source"] == "cli_pin")
        ck("R11d 반복 수 2 → full 정의 위반 거부", rejects(res, {"repeats": "2"}, repeats_source="x"))
        ck("R9 backend 부재 → 거부(기본값 없음)",
           rejects(bp.resolve, base, "default_preset", tool="vllm", cli={}, serving=SERV, repo_root=root))
        v, rec = res(cli_pins={"repeats": "4"}, pinned_by="사용자 — \"4회\"")
        ck("R11e 반복 수 사람 핀(--pin repeats=4) → cli_pin", v["repeats"] == 4 and rec["knobs"]["repeats"]["source"] == "cli_pin")
        ck("R9b guidellm 인데 예산 부재 → 거부", rejects(res, tool="guidellm"))
        v, rec = res(tool="vllm")
        ck("R9c vllm 도구는 예산 not_applicable 로 기록",
           v["bench_budget_mib"] is None and rec["knobs"]["bench_budget_mib"]["source"] == "not_applicable")
        ck("R9d 예산 16384(confirm)를 승인 없이 → 거부",
           rejects(res, {"bench_budget_mib": "16384"}, {"bench_budget_mib": "host_budget"}, tool="guidellm"))
        ck("R10 input+output > max-model-len → 거부",
           rejects(res, {"input_len": "4096"}, {"input_len": "serving_context"}))
        ck("R12 Agent 선택이 아닌 knob 에 --knob-reason → 거부", rejects(res, None, {"warmups": "preset_default"}))
        ck("R12b 다른 knob 의 근거 코드를 쓰면 거부", rejects(res, {"input_len": "2048"}, {"input_len": "host_budget"}))
        ck("R13 실측에서 서빙 yaml 부재 → 거부", rejects(bp.serving_caps, td / "nope.yaml", False))
        sc = bp.serving_caps(td / "nope.yaml", True)
        ck("R13b dry-run 은 unavailable 로 기록", sc["max_num_seqs"] is None and sc["source"].startswith("unavailable("))
        nolim = {"max_num_seqs": None, "max_model_len": None, "source": "declared(fixture · 선언 없음)"}
        v, rec = res(serving=nolim)
        ck("R16 서빙 yaml 에 한계 선언이 없으면 필터는 not_applied 로 기록(값을 만들지 않는다)",
           v["levels"] == [1, 2, 4, 8, 16] and rec["knobs"]["levels"]["serving_cap_applied"].startswith("not_applied"))
        v, rec = res({"levels": "2,4"}, {"levels": "serving_concurrency"}, serving=nolim)
        ck("R17 판정점 1 은 Agent 가 빼도 포함된다", v["levels"] == [1, 2, 4] and rec["knobs"]["levels"]["verdict_point_added"])
        ck("R18 levels 32(confirm)를 승인 없이 → 거부",
           rejects(res, {"levels": "1,32"}, {"levels": "serving_concurrency"}, serving=nolim))

        # D — 데이터셋 축(2026-10-16 · plan_26101614): 기본은 synthetic · 실제 데이터셋은 사용자 프리셋 · 판정 불변
        v, rec = res(serving=nolim)
        ck("D1 기본 프리셋 = synthetic · 나머지 null · 기록 블록에 dataset 이 실린다",
           v["dataset_kind"] == "synthetic" and all(v[n] is None for n in bp.DATASET_KNOBS if n != "dataset_kind")
           and rec["dataset"]["dataset_kind"] == "synthetic")
        raw = td / "raw.parquet"
        raw.write_bytes(b"cnn-fixture")
        import hashlib as _h
        good_sha = _h.sha256(b"cnn-fixture").hexdigest()

        def dset_preset(**over):
            d = copy.deepcopy(base)
            vals = {"dataset_kind": "parquet_file", "dataset_source": "managed", "dataset_id": "abisee/cnn_dailymail 3.0.0 test",
                    "dataset_path": str(raw), "dataset_revision": "a" * 40, "source_sha256": good_sha,
                    "column_map": {"text_column": "article"}, "preprocess_strategy": "concatenate", "seed": 42}
            vals.update(over)
            for n, val in vals.items():
                d["knobs"][n]["default"] = val
                d["knobs"][n]["options"] = [{"value": val, "grade": "auto"}]
            return d

        cnn = dset_preset()
        ck("D2 사용자 프리셋(CNN 모양)이 같은 검증을 통과한다", not bp.validate_preset(cnn, bindings),
           bp.validate_preset(cnn, bindings))
        ck("D3 상대경로 dataset_path 거부", bp.validate_preset(dset_preset(dataset_path="data/x.parquet"), bindings))
        ck("D4 40자 hex 가 아닌 revision 거부", bp.validate_preset(dset_preset(dataset_revision="main"), bindings))
        ck("D5 닫힌 목록 밖 dataset_kind 거부", bp.validate_preset(dset_preset(dataset_kind="huggingface"), bindings))
        ck("D6 교차 규칙: 실제 데이터셋인데 sha256 이 null → 거부", bp.validate_preset(dset_preset(source_sha256=None), bindings))
        ck("D6b 교차 규칙: synthetic 인데 경로가 있다 → 거부",
           bp.validate_preset(dset_preset(dataset_kind="synthetic", dataset_source=None, dataset_id=None, dataset_revision=None,
                                          source_sha256=None, column_map=None, preprocess_strategy=None, seed=None), bindings))
        ck("D6c column_map 은 text_column 하나만", bp.validate_preset(dset_preset(column_map={"text_column": "a", "x": "b"}), bindings))

        def res_d(doc, tool="guidellm", dry_run=False):
            c = dict(NEED, bench_budget_mib="4096")
            return bp.resolve(doc, "cnn_preset", tool=tool, cli=c, knob_reasons=dict(RS, bench_budget_mib="host_budget"),
                              serving=nolim, repo_root=root, dry_run=dry_run)

        v, rec = res_d(cnn)
        ck("D7 측정 시점: 원본 실재 · sha256 일치 → 통과하고 관측을 기록",
           v["dataset_kind"] == "parquet_file" and rec["dataset"]["runtime_check"]["checked"].startswith("measured("))
        ck("D8 --tool vllm + 실제 데이터셋 → 거부(조용히 synthetic 으로 재지 않는다)", rejects(res_d, cnn, tool="vllm"))
        ck("D9 sha256 불일치 → 거부", rejects(res_d, dset_preset(source_sha256="b" * 64)))
        ck("D9b 원본 부재 → 거부", rejects(res_d, dset_preset(dataset_path=str(td / "missing.parquet"))))
        v, rec = res_d(dset_preset(dataset_path=str(td / "missing.parquet")), dry_run=True)
        ck("D9c dry-run 은 원본 검사를 건너뛰고 그 사실을 기록", rec["dataset"]["runtime_check"]["checked"].startswith("skipped("))
        ck("D10 ephemeral 은 이번 개정에서 경로가 없다 → 거부", rejects(res_d, dset_preset(dataset_source="ephemeral")))
        ck("D11 기본 프리셋의 비기본 데이터셋 값은 escalate(Agent 가 못 연다) · 데이터셋 knob 은 스윕 플래그가 없다",
           all(o["grade"] == "escalate" for n in ("dataset_kind", "dataset_source", "preprocess_strategy")
               for o in base["knobs"][n]["options"] if o["value"] != base["knobs"][n]["default"])
           and all(bp.FLAG_OF[n].startswith("--pin ") for n in bp.DATASET_KNOBS))

    if fails:
        sys.stderr.write("[selftest_bench_preset] FAIL %d건: %s\n" % (len(fails), fails))
        return 1
    print("[selftest_bench_preset] OK — K1~K4 · V1~V15 · R1~R18(+R5b·R5c·R11e) · D1~D11 전부 통과")
    return 0


if __name__ == "__main__":
    sys.exit(run())
