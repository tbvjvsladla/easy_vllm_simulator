#!/usr/bin/env python3
"""selftest_gmu_roles_batch.py — gmu 두 역할 분리 · max-num-seqs 산식 · lockset 기계 각인의 결정론 자체검사.

왜 있는가(plan_26091407 §4.2·§4.3 · §7 O3): `recipe.py` 의 `margin` 변수 하나가 클램프 산식·검증 게이트·배포
yaml gmu·되먹임 임계를 겸했고(F4), max-num-seqs 는 lockset 의 손값이었다(F2 — 엔진 보고 KV 토큰의 소비자 0).
교정은 역할을 `gate_margin`(=safety_margin)과 `deploy_gmu`(=target_gpu.target_gmu)로 가르고, batch 를
`min(concurrency_requirement, KV_fit@typical_request_tokens)` 로 2-위상 트라이얼이 산출·재검증하며, 수렴
lockset 에 `provenance`·`*_source` 를 **기계가** 적는 것이다. 이 파일은 그 셋이 실제로 그렇게 도는지를 친다.

세 층으로 친다 — 순수 함수만 보면 **호출이 실제 경로에 서 있는지**를 증명하지 못하고(호출자 없는 자체검사 = L1),
정적 mock 프로파일만 보면 **클램프에 따라 달라지는 엔진 토큰**을 증명하지 못한다(픽스처가 실물보다 좁다):
  U  함수 층(in-process · 배포되는 recipe.py/sim_classify.py 를 그대로 import):
     U1 target 흐름 역할 분리 · U2 host 흐름 manifest 자동 채움(통합·discrete · 선언 예산 충돌 exit 5)
     U3 fail-loud 음성대조 · U4 derive_batch 3경우 + 경계(요구 == KV-fit) + 요구 없음(batch 미정)
     U5 sim_classify adjust_target=batch(+경계·음성대조) · U6 클램프 산식(버퍼 일관 · required = max(len, L×batch))
     U7 게이트 승수 ≠ 클램프 승수 · U8 각인 어휘 · 선언 축 모양(tp 는 기재 · batch 는 fail-loud) · 셀 lockset 경로 판정
     U9 lockset 어휘 소유자 모양 결함(상수 없음·적재 예외·모양 불일치) → exit 5 안내(traceback ✗) + 음성대조
  E  CLI 층(subprocess · 임시 저장소에 **배포되는 바이트 사본**을 같은 상대경로로 놓고 `recipe.py simulate
     --mock-profile` 로 main() 의 Flag 게이트부터 3종 세트·lockset 쓰기까지 통째로 돈다):
     E1 요구 ≤ KV-fit(+입력 lockset 오염 ✗ · tp 불일치 기재) · E2 요구 > KV-fit · E3 요구 없음(batch 미정 · 1건 클램프)
     E4 위상 2 하향 · E5 host 흐름 · E6 손레버 보존 · E7 직전 explorer 산출 비승계 · E8 target_gmu 미선언 exit 5
     E9 입력 출처 어휘 밖 → 기재만 · E10 배선 층 역할 분리 · E11 tp 모양 결함은 기재 · E12 host 선언 예산 충돌 exit 5
     E13 캠페인 셀 lockset 은 --lockset-out 없이도 제자리 각인 · E14 캠페인 밖은 입력을 덮어쓰지 않는다(음성대조)
  F  가짜 엔진 층(subprocess · 사본 recipe.py 의 `cmd_simulate` 를 부르되 `run_trial` 만 **클램프에 비례하는 토큰을
     돌려주는 결정론 엔진**으로 바꾼다 — 블록 16 반올림 · 트라이얼별 OOM 주입 · 클램프 토큰 미관측 · per-token 불일치):
     F1 첫 트라이얼 OOM → 잠정 클램프 → 요구 16 을 배포 천장 KV-fit 으로 유지(음성대조: 잠정 클램프 원토큰은 4)
     F2 첫 트라이얼 OOM ∧ 요구 없음 → batch 미정 · 2트라이얼 수렴 · F3 요구 없음 → 1건 클램프(천장 클램프 ✗)
     F4 입력 사람 클램프 → 그 클램프의 엔진 토큰으로 산출 · 재검증 · kv_source=hand · F5 TP=2 타겟(÷TP 환산)
     F6 클램프 토큰 미관측 → verified=false · F7 per-token 불일치 → 위상 2 하향 · F8 같은 구성 cap 2 → HITL 사유
     F9 per-token 일치 → 하향 없음(실물 경계 통과) · 모든 수렴 F 는 되먹임 safety_margin_threshold == gate_margin

격리: docker·NAS·GPU 불요. mock 프로파일·가짜 엔진은 테스트 평면의 외부 의존 차단이며(정당한 모킹), 산출 lockset 은
`trial_provenance=mock`·`measured=false` 로 자기가 실측이 아님을 밝히는지까지 단언한다(실측인 척 ✗).
라이브 `output/**`·`docs/simlog/**`·`campaigns/**` 에 닿지 않는다(사본 저장소 안에서만 쓴다).

사용: python3 selftest_gmu_roles_batch.py   (exit 0 = 전부 통과)
"""
from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]
SKILL = REPO / ".claude" / "skills" / "vllm-recipe-explorer"
# 사본에 싣는 것: explorer 스킬 전체(로컬 산출물 제외) + recipe.py 가 읽는 두 외부 파일.
EXTRA_COPIES = (
    ".claude/skills/terraforming_node/scripts/campaign_template_validator.py",  # lockset 어휘 소유자
    ".claude/skills/wiki-desk/reference/references.md",                        # GPU 사양 역룩업
)
# 스킬 안의 비추적 로컬 산출물(.gitignore) — 사본이 운영자 입력을 들고 가지 않게 한다.
SKIP_NAMES = {"__pycache__", "feedback", "config.yaml"}

GIB = 1024 ** 3
BUFFER = 1.02   # estimate_vram.KV_BLOCK_ALIGN_BUFFER — 아래 _buffer() 가 정본에서 읽어 교차검증한다
MANIFEST = ("topology: single\ngpus_per_node: 1\ngpu_model: \"NVIDIA GB10\"\nmodel_source: managed\n"
            "terraforming:\n  complete: true\n  branch_verified: true\n")
MANIFEST_DISCRETE = MANIFEST.replace("NVIDIA GB10", "NVIDIA RTX PRO 6000")
MANIFEST_MULTI2 = ("topology: multi\ngpus_per_node: 1\ngpu_model: \"NVIDIA RTX PRO 6000\"\nmodel_source: managed\n"
                   "nodes:\n  - role: main\n  - role: sub\n"
                   "terraforming:\n  complete: true\n  branch_verified: true\n")
MODEL_CONFIG = {"architectures": ["LlamaForCausalLM"], "model_type": "llama", "hidden_size": 4096,
                "num_hidden_layers": 32, "num_attention_heads": 32, "num_key_value_heads": 8,
                "head_dim": 128, "max_position_embeddings": 131072, "torch_dtype": "bfloat16",
                "vocab_size": 128256}
MAX_LEN = 32768
L_TYP = 8192
# 픽스처 프로파일: 배포 천장(96 GiB × 0.85 − 10 − 2 = 69.6 GiB)에 가까운 측정 KV 70 GiB(host == target 에 가까운 형상).
#   PROFILE_NEAR 의 엔진 토큰 700,000 은 위상 1 환산 KV-fit 토큰(≈ 682k)보다 크다 → 위상 2 재검증이 통과한다.
#   PROFILE_SHORT 는 per-token 이 같고 측정 KV 만 40 GiB(측정 풀이 작았다) → 엔진 토큰 400,000 < 위상 1 KV-fit.
PROFILE_NEAR = {"vllm_profile": {"weights_gib": 10.0, "non_kv_overhead_gib": 2.0, "kv_cache_gib": 70.0,
                                 "kv_cache_tokens": 700000, "max_concurrency": 21.36},
                "functional": {"passed": True}}
# PROFILE_HEAVY: weights+overhead 84 GiB — 게이트 천장(96×0.90=86.4)은 통과하고 클램프 천장(96×0.85=81.6)은 넘는다.
PROFILE_HEAVY = {"vllm_profile": {"weights_gib": 70.0, "non_kv_overhead_gib": 14.0, "kv_cache_gib": 1.0,
                                  "kv_cache_tokens": 1000, "max_concurrency": 0.03},
                 "functional": {"passed": True}}
PROFILE_SHORT = {"vllm_profile": {"weights_gib": 10.0, "non_kv_overhead_gib": 2.0, "kv_cache_gib": 40.0,
                                  "kv_cache_tokens": 400000, "max_concurrency": 12.2},
                 "functional": {"passed": True}}
TARGET = {"gpu_model": "NVIDIA RTX PRO 6000", "per_card_vram_gib": 96, "target_gmu": 0.85, "cards_per_node": 1}
SAFETY = 0.90

failures: list[str] = []


def ck(name: str, cond: bool, detail: str = "") -> None:
    print(("  [PASS] " if cond else "  [FAIL] ") + name + ("" if cond else "  → " + detail))
    if not cond:
        failures.append(name)


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _dies(fn, *args, **kwargs):
    """fail-loud 경로가 실제로 멈추는가. (멈췄나, 종료코드, stderr)."""
    err = io.StringIO()
    try:
        with contextlib.redirect_stderr(err):
            fn(*args, **kwargs)
    except SystemExit as exc:
        return True, exc.code, err.getvalue()
    return False, None, err.getvalue()


def _fit_tokens(max_safe: int, per_token: float) -> int:
    """recipe._kv_fit_tokens 의 언클램프 환산과 **같은 식**(기대값 계산용 — 결과 대조 대상은 산출물이다)."""
    tokens = int(max_safe // (per_token * BUFFER))
    while tokens > 0 and int(per_token * tokens * BUFFER) > max_safe:
        tokens -= 1
    return tokens


# ───────────────────────────── U — 함수 층 ─────────────────────────────

def unit_layer(tmp: Path) -> None:
    sys.path.insert(0, str(SKILL / "scripts"))
    recipe = _load("_selftest_recipe", SKILL / "recipe.py")
    sim = _load("_selftest_sim_classify", SKILL / "scripts" / "sim_classify.py")
    ev = _load("_selftest_estimate_vram", SKILL / "scripts" / "estimate_vram.py")
    ck("U0 픽스처 버퍼 상수가 정본(estimate_vram.KV_BLOCK_ALIGN_BUFFER)과 같다(기대값 계산이 거울로 늦지 않게)",
       ev.KV_BLOCK_ALIGN_BUFFER == BUFFER, "정본=%r" % ev.KV_BLOCK_ALIGN_BUFFER)
    froot = tmp / "unit_root"
    droot = tmp / "unit_root_discrete"
    for root, man in ((froot, MANIFEST), (droot, MANIFEST_DISCRETE)):
        for topo in ("single", "multi"):   # 사본 루트는 git 이 아니라 single 로 읽히지만, 통로 선택에 기대지 않는다
            (root / "output" / topo).mkdir(parents=True, exist_ok=True)
            (root / "output" / topo / "manifest.yaml").write_text(man, encoding="utf-8")

    # U1 — target 흐름: deploy_gmu == target_gmu · gate_margin == safety_margin 으로 갈라진다.
    with contextlib.redirect_stderr(io.StringIO()):
        r = recipe._gpu_roles({"target_gpu": dict(TARGET), "safety_margin": SAFETY}, 1, str(froot))
    ck("U1 target 흐름: deploy_gmu == target_gpu.target_gmu(0.85)", r["deploy_gmu"] == 0.85, json.dumps(r))
    ck("U1 target 흐름: gate_margin == config.safety_margin(0.90) — 둘이 한 변수로 접히지 않는다",
       r["gate_margin"] == SAFETY and r["gate_margin"] != r["deploy_gmu"], json.dumps(r))
    ck("U1 target 흐름: lockset gmu_source=target_gmu · 예산 = per_card_vram",
       r["gmu_source"] == "target_gmu" and r["budget_gib"] == 96.0 and r["flow"] == "target", json.dumps(r))

    # U2 — host 흐름: manifest 에서 target_gpu 를 채워 같은 해소 경로를 탄다.
    with contextlib.redirect_stderr(io.StringIO()):
        h = recipe._gpu_roles({"safety_margin": 0.88, "test_device_total_gib": 100}, 1, str(froot))
    ck("U2 host 흐름: gpu_model=manifest · per_card=통합메모리 선언 total · deploy_gmu=safety_margin 승계",
       h["flow"] == "host" and h["gpu_model"] == "NVIDIA GB10" and h["budget_gib"] == 100.0
       and h["deploy_gmu"] == 0.88 and h["gate_margin"] == 0.88, json.dumps(h))
    ck("U2 host 흐름: 승계는 정본 칸이 아니므로 gmu_source=hand + 출처 문장이 승계를 말한다(침묵 폴백 ✗)",
       h["gmu_source"] == "hand" and "safety_margin 승계" in h["deploy_gmu_source"], json.dumps(h))
    with contextlib.redirect_stderr(io.StringIO()):
        hc = recipe._gpu_roles({"safety_margin": 0.95, "test_device_total_gib": 100}, 1, str(froot))
    ck("U2 host 흐름 통합메모리 0.90 하드클램프 유지(deploy 만 · 게이트 승수는 선언 그대로)",
       hc["deploy_gmu"] == 0.90 and hc["gate_margin"] == 0.95 and "하드클램프" in hc["deploy_gmu_source"],
       json.dumps(hc))
    with contextlib.redirect_stderr(io.StringIO()):
        hd = recipe._gpu_roles({"safety_margin": 0.9, "test_device_total_gib": 100}, 1, str(droot))
        hl = recipe._gpu_roles({"safety_margin": 0.9}, 1, str(droot))
    ck("U2 host 흐름 discrete: 선언 test_device_total_gib(100) > references.md 역룩업(96) · 선언 없으면 역룩업",
       hd["budget_gib"] == 100.0 and "declared" in hd["budget_source"]
       and hl["budget_gib"] == 96.0 and "역룩업" in hl["budget_source"], json.dumps([hd, hl]))
    died, code, err = _dies(recipe._gpu_roles, {"safety_margin": 0.9, "test_device_total_gib": 100,
                                                "vram_budget_gb": 24}, 1, str(froot))
    ck("U2 host 흐름: 선언 예산 vram_budget_gb=24 ↔ 노드 per_card 100 충돌 → exit 5(실측이 선언을 조용히 덮지 않는다)",
       died and code == 5 and "target_gpu 블록" in err, "died=%s code=%s" % (died, code))
    with contextlib.redirect_stderr(io.StringIO()):
        hm = recipe._gpu_roles({"safety_margin": 0.9, "test_device_total_gib": 100, "vram_budget_gb": 98}, 1,
                               str(froot))
    ck("U2 음성대조: 허용오차 안(98 vs 100)이면 진행하고 정합 사실을 예산 출처에 적는다",
       hm["budget_gib"] == 100.0 and "vram_budget_gb=98" in hm["budget_source"], json.dumps(hm))

    # U3 — fail-loud 음성대조: 선언이 없으면 기본값으로 메우지 않는다.
    t = dict(TARGET)
    t.pop("target_gmu")
    died, code, _ = _dies(recipe._gpu_roles, {"target_gpu": t, "safety_margin": SAFETY}, 1, str(froot))
    ck("U3 target_gpu.target_gmu 미선언 → exit 5(종전 기본 0.90 으로 메우지 않는다)", died and code == 5,
       "died=%s code=%s" % (died, code))
    died, code, _ = _dies(recipe._gpu_roles, {"test_device_total_gib": 100}, 1, str(froot))
    ck("U3 host 흐름 safety_margin 도 미선언 → exit 5(배포 gmu 를 기본값으로 쓰지 않는다)", died and code == 5,
       "died=%s code=%s" % (died, code))

    # U4 — derive_batch: min(요구, KV-fit) 3경우 + 경계 + 요구 없음.
    def plan(req, typ, req_state="declared"):
        return {"mode": "derive", "requirement": req, "requirement_state": req_state if req else "null",
                "requirement_source": "fx", "typical": typ, "typical_source": "fx", "prior_batch": None}
    d = recipe.derive_batch(plan(16, 1000), MAX_LEN, 20000, "fx")
    ck("U4 요구 16 ≤ KV-fit 20 → batch 16 · declared-requirement",
       d["batch"] == 16 and d["kv_fit"] == 20 and d["batch_source"] == "declared-requirement", json.dumps(d))
    d = recipe.derive_batch(plan(20, 1000), MAX_LEN, 20000, "fx")
    ck("U4 경계: 요구 20 == KV-fit 20 → 요구 채택(declared-requirement · 낮추지 않는다)",
       d["batch"] == 20 and d["batch_source"] == "declared-requirement", json.dumps(d))
    d = recipe.derive_batch(plan(21, 1000), MAX_LEN, 20000, "fx")
    ck("U4 경계 +1: 요구 21 > KV-fit 20 → batch 20 · kv-fit-measured",
       d["batch"] == 20 and d["batch_source"] == "kv-fit-measured", json.dumps(d))
    d = recipe.derive_batch(plan(64, 1000), MAX_LEN, 20000, "fx")
    ck("U4 요구 64 > KV-fit 20 → batch 20 · kv-fit-measured · 낮춘 사유 기재",
       d["batch"] == 20 and d["batch_source"] == "kv-fit-measured" and "낮춤" in d["reason"], json.dumps(d))
    d = recipe.derive_batch(plan(None, None), MAX_LEN, 700000, "fx")
    ck("U4 요구 없음(null · L 부재) → batch 미정 · batch_source 없음 · KV-fit 은 참고값(L=max_model_len 대체 기재)",
       d["batch"] is None and d["batch_source"] is None and d["kv_fit"] == 700000 // MAX_LEN
       and d["kv_fit_length"] == MAX_LEN and "대체" in d["kv_fit_length_source"] and "참고값" in d["reason"],
       json.dumps(d))
    d = recipe.derive_batch(plan(8, 1000), MAX_LEN, None, "엔진 토큰 부재")
    ck("U4 KV-fit 산출 불가 ∧ 요구 선언 → batch=요구 · verified=False(검증 없이 채택을 적는다)",
       d["batch"] == 8 and d["batch_source"] == "declared-requirement" and d["verified"] is False, json.dumps(d))
    d = recipe.derive_batch(plan(None, None), MAX_LEN, None, "엔진 토큰 부재")
    ck("U4 KV-fit 산출 불가 ∧ 요구 없음 → batch 미정(max-num-seqs 를 지어내지 않는다)",
       d["batch"] is None and d["batch_source"] is None, json.dumps(d))

    # U5 — sim_classify: 모두-OK 분기의 KV-fit 재검증 → adjust_target=batch.
    base = {"load_ok": True, "functional": {"passed": True},
            "vllm_profile": {"weights_gib": 10.0, "non_kv_overhead_gib": 2.0, "kv_cache_gib": 20.0,
                             "kv_cache_tokens": 10000}}
    over = dict(base, candidate={"batch": 16, "kv_cache_memory_bytes": 20 * GIB})
    v = sim.classify(over, 96, SAFETY, typical_request_tokens=1000)
    ck("U5 클램프 트라이얼 batch 16 > 엔진 KV-fit 10 → failure_class=none · adjust_target=batch · kv_fit=10",
       v["failure_class"] == "none" and v["adjust_target"] == "batch" and v.get("kv_fit") == 10, json.dumps(v))
    v = sim.classify(dict(base, candidate={"batch": 10, "kv_cache_memory_bytes": 20 * GIB}), 96, SAFETY,
                     typical_request_tokens=1000)
    ck("U5 경계: batch 10 == 엔진 KV-fit 10 → 넘지 않는다(adjust 없음 · checked)",
       v["adjust_target"] is None and (v.get("kv_fit_check") or {}).get("checked") is True
       and (v.get("kv_fit_check") or {}).get("exceeds") is False, json.dumps(v))
    v = sim.classify(dict(base, candidate={"batch": 11, "kv_cache_memory_bytes": 20 * GIB}), 96, SAFETY,
                     typical_request_tokens=1000)
    ck("U5 경계 +1: batch 11 > KV-fit 10 → adjust_target=batch", v["adjust_target"] == "batch", json.dumps(v))
    v = sim.classify(over, 96, SAFETY)
    ck("U5 비회귀: typical 미전달(기존 호출부) → 종전 결과(adjust 없음 · kv_fit_check 없음)",
       v["adjust_target"] is None and "kv_fit_check" not in v, json.dumps(v))
    v = sim.classify(dict(base, candidate={"batch": 16, "kv_cache_memory_bytes": None}), 96, SAFETY,
                     typical_request_tokens=1000)
    ck("U5 언클램프 측정 트라이얼은 재검증 대상 아님(측정 풀 토큰은 배포 KV 가 아니다)",
       v["adjust_target"] is None and (v.get("kv_fit_check") or {}).get("checked") is False, json.dumps(v))

    # U6 — 클램프 산식 경계: 위상 1 KV-fit 토큰으로 정한 batch 는 같은 산식의 클램프가 받아들인다.
    prof = {"weights_gib": 10.0, "non_kv_overhead_gib": 2.0, "kv_cache_gib": 70.0, "kv_cache_tokens": 700000}
    tokens, _src = recipe._kv_fit_tokens(prof, 96, 0.85, 1, None)
    res = recipe._resolve_clamp_kv({}, {"max_model_len": 1, "batch": tokens}, prof, 96, 0.85, 2,
                                   tp_divisor=1, request_tokens=1)
    ck("U6 경계: batch = KV-fit 토큰(L=1)이 정확히 천장에 닿아도 클램프 산식이 받아들인다(버퍼 일관)",
       res["fail"] is None and res["kv"] is not None, json.dumps(res))
    per_token = 70.0 * GIB / 700000
    max_safe = int(96 * 0.85 * GIB) - int(10 * GIB) - int(2 * GIB)
    naive = int(max_safe // per_token)
    res = recipe._resolve_clamp_kv({}, {"max_model_len": 1, "batch": naive}, prof, 96, 0.85, 2,
                                   tp_divisor=1, request_tokens=1)
    ck("U6 음성대조: 버퍼 없이 센 KV-fit(%d)을 batch 로 쓰면 같은 산식이 vram_infeasible 로 거부한다" % naive,
       naive > tokens and res["kv"] is None
       and (res["fail"] or {}).get("failure_class") == "vram_infeasible", json.dumps(res))
    res = recipe._resolve_clamp_kv({}, {"max_model_len": 4096, "batch": 4}, prof, 96, 0.85, 2,
                                   tp_divisor=1, request_tokens=1000)
    ck("U6 required 토큰 = max(max_model_len, L×batch) — 최악 길이 1건이 대표 길이 batch 보다 크면 그쪽",
       "max(max_model_len=4096, L=1000×batch=4)=4096" in res["note"], res["note"])
    res = recipe._resolve_clamp_kv({}, {"max_model_len": MAX_LEN, "batch": 16}, prof, 96, 0.85, 2,
                                   tp_divisor=1, request_tokens=L_TYP)
    want = int(per_token * max(MAX_LEN, L_TYP * 16) * BUFFER)
    ck("U6 산출 batch 의 클램프 = required(L×batch 토큰 × per_token × 버퍼) · 천장(max_safe)이 아니다(천장은 실행 가능 경계)",
       res["kv"] == want and res["kv"] < max_safe
       and "max(max_model_len=%d, L=%d×batch=16)=%d tokens" % (MAX_LEN, L_TYP, L_TYP * 16) in res["note"],
       "kv=%s want=%s max_safe=%s note=%s" % (res["kv"], want, max_safe, res["note"]))
    res = recipe._resolve_clamp_kv({}, {"max_model_len": MAX_LEN, "batch": 16}, prof, 96, 0.85, 2, tp_divisor=1)
    ck("U6 비회귀: request_tokens 미전달(손레버·구 lockset) → 종전 식 max_model_len × batch",
       res["kv"] == int(per_token * MAX_LEN * 16 * BUFFER) and "max_model_len=%d×batch=16" % MAX_LEN in res["note"],
       res["note"])

    # U7 — 두 승수는 다른 물음에 답한다: 게이트는 통과하고 클램프 천장은 넘는 트라이얼.
    heavy = {"load_ok": True, "functional": {"passed": True}, "candidate": {},
             "vllm_profile": {"weights_gib": 70.0, "non_kv_overhead_gib": 14.0, "kv_cache_gib": 1.0,
                              "kv_cache_tokens": 1000}}
    v = sim.classify(heavy, 96, SAFETY)
    res = recipe._resolve_clamp_kv({}, {"max_model_len": 1000, "batch": 1}, heavy["vllm_profile"], 96, 0.85, 2)
    ck("U7 weights+overhead 84 GiB: 게이트(96×gate_margin 0.90=86.4) 통과 ∧ 클램프 천장(96×deploy_gmu 0.85=81.6) 초과",
       v["failure_class"] == "none" and (res["fail"] or {}).get("failure_class") == "vram_infeasible",
       "classify=%s clamp=%s" % (json.dumps(v), json.dumps(res)))

    # U8 — 각인 어휘: explorer 가 적는 값은 어휘 밖이면 멈추고, 입력이 들고 온 값은 기재만 한다.
    died, code, _ = _dies(recipe._stamp, "batch_source", "bogus")
    ck("U8 explorer 각인 값 어휘 밖 → exit 5(내부 결함)", died and code == 5, "died=%s code=%s" % (died, code))
    val, why = recipe._input_source("batch_source", "bogus")
    ck("U8 음성대조: 입력 lockset 의 어휘 밖 출처는 멈추지 않고 사유로 돌려준다(P6 와 같은 등급 — 기재)",
       val is None and why and "어휘 밖" in why, "val=%r why=%r" % (val, why))
    val, why = recipe._input_source("kv_source", "measured-clamp")
    ck("U8 어휘 안 입력 출처는 그대로 읽힌다", val == "measured-clamp" and why is None)
    # 선언 축의 모양: tp 는 기재 전용(멈추지 않는다) · 산식 입력은 fail-loud · lockset batch 모양 결함은 fail-loud.
    ok_states = [recipe._declared_axis({"declared_axes": {"tp": v}}, "tp", strict=False)[1]
                 for v in ("2", 2.0, "해당 없음", 0, True)]
    ck("U8 declared_axes.tp 모양 결함(문자열·실수·0·bool)은 멈추지 않고 malformed 로 돌려준다(기재 항목)",
       ok_states == ["malformed"] * 5, repr(ok_states))
    died, code, _ = _dies(recipe._declared_axis, {"declared_axes": {"concurrency_requirement": "16"}},
                          "concurrency_requirement")
    ck("U8 음성대조: 산식 입력(concurrency_requirement='16')의 모양 결함은 exit 5(거짓 선언을 부재로 접지 않는다)",
       died and code == 5, "died=%s code=%s" % (died, code))
    died, code, _ = _dies(recipe._batch_plan, {"batch": "<<FILL>>", "max_model_len": 1024}, {})
    ck("U8 lockset batch='<<FILL>>' → traceback 이 아니라 exit 5 안내", died and code == 5,
       "died=%s code=%s" % (died, code))
    root = os.path.realpath(recipe.REPO_ROOT)
    shapes = [recipe._is_campaign_cell_lockset(os.path.join(root, *p)) for p in (
        ("campaigns", "camp-x", "cells", "c1", "lockset.json"),
        ("campaigns", "_template", "cells", "_cell", "lockset.json"),
        ("campaigns", "camp-x", "cells", "c1", "config.yaml"),
        ("lockset.json",),
        ("campaigns", "camp-x", "lockset.json"))]
    ck("U8 캠페인 셀 lockset 경로 판정: 인스턴스 셀만 참(뼈대 _template·다른 파일·루트·깊이 다름은 거짓)",
       shapes == [True, False, False, False, False], repr(shapes))

    # U9 — lockset 어휘 소유자가 **있는데 모양이 다르다**(옛 판 검증기 · 적재 실패): traceback 이 아니라 exit 5 안내
    #   (2026-09-14 · ⑧ 분석 발견 T7). 모듈 사본을 따로 적재해 REPO_ROOT 만 임시 트리로 돌린다 — 위 사례들의 모듈 상태를 건드리지 않는다.
    rv = _load("_selftest_recipe_vocab", SKILL / "recipe.py")
    vroot = tmp / "vocab_root"
    vfile = vroot / rv._LOCKSET_VOCAB_REL
    vfile.parent.mkdir(parents=True, exist_ok=True)
    rv.REPO_ROOT = str(vroot)
    for label, body, want in (
            ("어휘 상수 없음(옛 판)", "LOCKSET_KNOB_SOURCES = {}\n", "LOCKSET_PROVENANCE"),
            ("적재 예외(구문 오류)", "def broken(:\n", "SyntaxError"),
            ("모양 불일치(노브 표가 dict 아님)", "LOCKSET_PROVENANCE = ('explorer-phase2',)\nLOCKSET_KNOB_SOURCES = 3\n", "모양")):
        vfile.write_text(body, encoding="utf-8")
        rv._LOCKSET_VOCAB = None
        died, code, err = _dies(rv._lockset_vocab)
        ck("U9 lockset 어휘 소유자 %s → traceback 이 아니라 exit 5 · 원인(%s)을 말한다" % (label, want),
           died and code == 5 and want in err, "died=%s code=%s err=%r" % (died, code, err[-300:]))
    vfile.write_text("LOCKSET_PROVENANCE = ('explorer-phase2', 'hand-authored')\n"
                     "LOCKSET_KNOB_SOURCES = {'batch_source': ('declared-requirement',)}\n", encoding="utf-8")
    rv._LOCKSET_VOCAB = None
    died, code, _ = _dies(rv._lockset_vocab)
    ck("U9 음성대조: 계약 모양의 소유자는 그대로 적재된다(가드가 전부를 막으면 가드가 아니다)",
       not died and rv._LOCKSET_VOCAB == (("explorer-phase2", "hand-authored"), {"batch_source": ("declared-requirement",)}),
       repr(rv._LOCKSET_VOCAB))
    sys.path.remove(str(SKILL / "scripts"))


# ───────────────────────────── E — CLI 층 ─────────────────────────────

def _copy_tree(root: Path) -> None:
    for src_dir, dirs, files in os.walk(SKILL):
        dirs[:] = [d for d in dirs if d not in SKIP_NAMES]
        for f in files:
            if f in SKIP_NAMES or f.endswith(".pyc") or (f.startswith("lockset") and f.endswith(".json")):
                continue
            src = Path(src_dir) / f
            dst = root / src.relative_to(REPO)
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, dst)
    for rel in EXTRA_COPIES:
        (root / rel).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO / rel, root / rel)


def _fixture_repo(tmp: Path) -> Path:
    root = tmp / "repo"
    _copy_tree(root)
    try:   # 사본의 저장소 루트를 git 에서 파생하게 한다(바깥 저장소로 새지 않게). git 이 없으면 경로 폴백이 같은 루트다.
        subprocess.run(["git", "init", "-q", str(root)], check=True, timeout=60,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except (OSError, subprocess.SubprocessError):
        pass
    (root / "output" / "single").mkdir(parents=True, exist_ok=True)
    (root / "output" / "single" / "manifest.yaml").write_text(MANIFEST, encoding="utf-8")
    model = tmp / "nas" / "fx-org" / "fx-model"
    model.mkdir(parents=True, exist_ok=True)
    (model / "config.json").write_text(json.dumps(MODEL_CONFIG), encoding="utf-8")
    for name, blob in (("near", PROFILE_NEAR), ("short", PROFILE_SHORT), ("heavy", PROFILE_HEAVY)):
        (tmp / f"profile_{name}.json").write_text(json.dumps(blob), encoding="utf-8")
    return root


def _base_lock(name: str) -> dict:
    return {"id": "fx-" + name, "quantization": "native", "max_model_len": MAX_LEN, "batch": None,
            "kv_cache_quant": None, "kv_cache_memory_bytes": None, "attention_backend": None,
            "attention_backend_candidates": [], "tool_call_parser": None, "tool_call_parser_candidates": [],
            "reasoning_parser": None, "reasoning_parser_candidates": [], "model_capabilities": {}}


def _write_cfg(case: Path, tmp: Path, name: str, cfg_extra: dict, target: bool) -> Path:
    import yaml  # recipe.py 와 같은 의존(호스트 PyYAML)
    cfg = {"target_model": {"path": "/app/models/fx-org/fx-model"}, "nas_host_root": str(tmp / "nas"),
           "vram_budget_gb": 96, "safety_margin": SAFETY, "kv_cache_dtype_bytes": 2,
           "serving": {"config_name": "fx-" + name, "port": 18080, "served_model_name": "fx"}}
    if target:
        cfg["target_gpu"] = dict(TARGET)
    cfg.update(cfg_extra)
    cfg = {k: v for k, v in cfg.items() if v is not None}
    path = case / "config.yaml"
    path.write_text(yaml.safe_dump(cfg, allow_unicode=True), encoding="utf-8")
    return path


def _run_case(root: Path, tmp: Path, name: str, *, cfg_extra: dict, lockset: dict, profile: str = "near",
              target: bool = True, lock_path: Path | None = None, lockset_out: bool = True):
    case = tmp / "cases" / name
    case.mkdir(parents=True, exist_ok=True)
    cfg_path = _write_cfg(case, tmp, name, cfg_extra, target)
    base_lock = _base_lock(name)
    base_lock.update(lockset)
    lk = lock_path or (case / "lockset.json")
    lk.parent.mkdir(parents=True, exist_ok=True)
    lk.write_text(json.dumps(base_lock), encoding="utf-8")
    argv = [sys.executable, str(root / ".claude/skills/vllm-recipe-explorer/recipe.py"), "simulate",
            "--config", str(cfg_path), "--candidate", str(lk),
            "--mock-profile", str(tmp / f"profile_{profile}.json"), "--run-id", "fx_" + name]
    if lockset_out:
        argv += ["--lockset-out", str(lk)]
    proc = subprocess.run(argv, cwd=str(root), capture_output=True, text=True, timeout=120,
                          env=dict(os.environ, PYTHONDONTWRITEBYTECODE="1"))
    out = {"rc": proc.returncode, "stderr": proc.stderr, "stdout": proc.stdout,
           "lockset": json.loads(lk.read_text(encoding="utf-8")), "input_lock": base_lock}
    summ = root / "docs" / "simlog" / ("fx_" + name) / "run_summary.json"
    out["summary"] = json.loads(summ.read_text(encoding="utf-8")) if summ.is_file() else None
    yml = root / "output" / "single" / "configs" / ("fx-%s.yaml" % name)
    out["yaml"] = yml.read_text(encoding="utf-8") if yml.is_file() else None
    return out


def _yaml_value(text: str | None, key: str):
    for line in (text or "").splitlines():
        if line.startswith(key + ":"):
            return line.split(":", 1)[1].strip()
    return None


def cli_layer(tmp: Path, root: Path) -> None:
    validator = _load("_selftest_validator", REPO / EXTRA_COPIES[0])
    decl = lambda req, typ: {"declared_axes": {"concurrency_requirement": req, "typical_request_tokens": typ}}

    def stamped(o, label):
        lk = o["lockset"]
        tmpf = tmp / ("stamp_%s.json" % label)
        tmpf.write_text(json.dumps(lk), encoding="utf-8")
        knobs_ok = all(lk.get(k) is None or lk.get(k) in allowed
                       for k, allowed in validator.LOCKSET_KNOB_SOURCES.items())
        return (validator.lockset_provenance_reason(tmpf) is None and lk.get("provenance") == "explorer-phase2"
                and knobs_ok)

    # E1 — 요구 ≤ KV-fit. 선언 tp=4 는 해소 TP 1 과 달라도 기재만 한다.
    o = _run_case(root, tmp, "e1", cfg_extra={"declared_axes": {"concurrency_requirement": 16,
                                                                "typical_request_tokens": L_TYP, "tp": 4}},
                  lockset={})
    lk, roles = o["lockset"], o["lockset"].get("gmu_roles") or {}
    ck("E1 수렴(rc 0)", o["rc"] == 0, o["stderr"][-800:])
    ck("E1 lockset 기계 각인: provenance=explorer-phase2 · *_source 가 소유자 어휘 안(검증기 판정 함수로 확인)",
       stamped(o, "e1"), json.dumps({k: lk.get(k) for k in ("provenance", "batch_source", "gmu_source", "kv_source")}))
    ck("E1 batch=16 · batch_source=declared-requirement · 위상 2 재검증 통과(verified)",
       lk.get("batch") == 16 and lk.get("batch_source") == "declared-requirement"
       and (lk.get("batch_derivation") or {}).get("verified") is True, json.dumps(lk.get("batch_derivation")))
    ck("E1 gmu_source=target_gmu · kv_source=measured-clamp",
       lk.get("gmu_source") == "target_gmu" and lk.get("kv_source") == "measured-clamp", json.dumps(lk))
    ck("E1 역할 분리 기록: deploy_gmu == target_gmu(0.85) · gate_margin == safety_margin(0.90)",
       roles.get("deploy_gmu") == TARGET["target_gmu"] and roles.get("gate_margin") == SAFETY, json.dumps(roles))
    ck("E1 서빙 yaml: gpu-memory-utilization == target_gmu(≠ safety_margin) · max-num-seqs == 산출 batch · 클램프 emit",
       _yaml_value(o["yaml"], "gpu-memory-utilization") == "0.85"
       and _yaml_value(o["yaml"], "max-num-seqs") == "16"
       and _yaml_value(o["yaml"], "kv-cache-memory-bytes") == str(lk.get("kv_cache_memory_bytes")),
       o["yaml"] or "yaml 없음")
    ck("E1 yaml 에 출처 주석(batch_source·gmu_source)이 실린다 — serve 키가 아닌 주석 줄",
       "batch_source=declared-requirement" in (o["yaml"] or "") and "gmu_source=target_gmu" in (o["yaml"] or ""),
       o["yaml"] or "")
    ck("E1 mock 은 실측인 척하지 않는다: lockset trial_provenance=mock · measured=false · run_summary provenance=mock",
       lk.get("trial_provenance") == "mock" and lk.get("measured") is False
       and (o["summary"] or {}).get("provenance") == "mock", json.dumps(o["summary"] or {})[:400])
    ck("E1 엔진 max_concurrency 는 보수 하한으로 기재된다(산식 입력 ✗ · 소비자 0 해소)",
       any(t.get("conservative_floor") == 21 for t in
           ((lk.get("batch_derivation") or {}).get("engine_max_concurrency") or {}).get("trials") or []),
       json.dumps((lk.get("batch_derivation") or {}).get("engine_max_concurrency")))
    leaked = [k for k in ("gpu_memory_utilization", "model_path_container", "served_model_name", "model_id")
              if k in lk and k not in o["input_lock"]]
    ck("E1 트라이얼 주입값(gmu·모델 경로·서빙명)은 lockset 칸으로 되쓰이지 않는다(다음 실행에서 옛 gmu 가 deploy_gmu 를 이기지 않게)",
       leaked == [], "새어 든 칸=%s" % leaked)
    tpchk = (lk.get("declared_axes_check") or {}).get("tp") or {}
    ck("E1 declared_axes.tp=4 ≠ 해소 TP 1 → 기재(match=false · stderr ⚠) · rc 0(차단 ✗)",
       tpchk.get("declared") == 4 and tpchk.get("resolved") == 1 and tpchk.get("match") is False
       and "declared_axes.tp=4" in o["stderr"], json.dumps(tpchk))

    # E2 — 요구 > KV-fit: 위상 1 에서 KV-fit 으로 낮춘다.
    o = _run_case(root, tmp, "e2", cfg_extra=decl(128, L_TYP), lockset={})
    lk, der = o["lockset"], o["lockset"].get("batch_derivation") or {}
    ck("E2 수렴(rc 0) · batch = KV-fit < 요구 128 · kv-fit-measured · 낮춘 사유",
       o["rc"] == 0 and der.get("kv_fit") is not None and lk.get("batch") == der.get("kv_fit") < 128
       and lk.get("batch_source") == "kv-fit-measured" and "낮춤" in (der.get("reason") or ""),
       json.dumps(der)[:600] + o["stderr"][-400:])
    ck("E2 경계 산출 batch 로 산정한 클램프가 거부되지 않았다(vram_infeasible ✗) · 재검증 통과",
       der.get("verified") is True and _yaml_value(o["yaml"], "max-num-seqs") == str(lk.get("batch")),
       json.dumps(der)[:400])

    # E3 — 요구 없음(declared_axes 블록 없음 — 캠페인 밖 기본 경로): batch 미정 · 1건 클램프(종전 동작).
    o = _run_case(root, tmp, "e3", cfg_extra={}, lockset={})
    lk, der = o["lockset"], o["lockset"].get("batch_derivation") or {}
    hist = (o["summary"] or {}).get("correction_history") or []
    ck("E3 수렴 · 요구 absent → batch 미정 · batch_source 없음 · max-num-seqs 미emit · KV-fit 은 참고값으로만",
       o["rc"] == 0 and der.get("concurrency_requirement_state") == "absent" and lk.get("batch") is None
       and lk.get("batch_source") is None and _yaml_value(o["yaml"], "max-num-seqs") is None
       and isinstance(der.get("kv_fit"), int) and "참고값" in (der.get("reason") or ""),
       json.dumps(der)[:600] + o["stderr"][-400:])
    ck("E3 클램프는 max_model_len 1건 기준(천장 클램프 ✗ — 선언 없는 config 가 배포 천장 전체를 받지 않는다)",
       any("max_model_len=%d×batch=1" % MAX_LEN in (h.get("after_note") or "") for h in hist)
       and lk.get("kv_cache_memory_bytes") == int(70.0 * GIB / 700000 * MAX_LEN * BUFFER),
       json.dumps(hist)[:500])

    # E4 — 위상 2 재검증이 batch 를 낮춘다(엔진 토큰 400,000 ÷ 8192 = 48 < 위상 1 산출 64).
    o = _run_case(root, tmp, "e4", cfg_extra=decl(64, L_TYP), lockset={}, profile="short")
    lk, der = o["lockset"], o["lockset"].get("batch_derivation") or {}
    hist = (o["summary"] or {}).get("correction_history") or []
    ck("E4 수렴 · 위상 2 에서 adjust_target=batch 로 64→48 · kv-fit-measured · 재검증 통과",
       o["rc"] == 0 and lk.get("batch") == 400000 // L_TYP and lk.get("batch_source") == "kv-fit-measured"
       and any(h.get("adjust_target") == "batch" and (h.get("before") or {}).get("batch") == 64 for h in hist)
       and der.get("verified") is True and (der.get("phase2_adjustments") or [{}])[0].get("batch_before") == 64,
       json.dumps(hist)[:600] + o["stderr"][-400:])

    # E5 — host 흐름: target_gpu 미정의 → manifest 자동 채움 · deploy_gmu = safety_margin 승계(표시).
    o = _run_case(root, tmp, "e5", cfg_extra=dict(decl(4, L_TYP), safety_margin=0.88, test_device_total_gib=100),
                  lockset={}, target=False)
    lk, roles = o["lockset"], o["lockset"].get("gmu_roles") or {}
    ck("E5 host 흐름 수렴 · gmu_source=hand · deploy_gmu=gate_margin=0.88 · 승계 사실이 출처 문장과 stderr 에",
       o["rc"] == 0 and lk.get("gmu_source") == "hand" and roles.get("flow") == "host"
       and roles.get("deploy_gmu") == 0.88 and roles.get("gate_margin") == 0.88
       and "승계" in (roles.get("deploy_gmu_source") or "") and "정본 칸" in o["stderr"]
       and _yaml_value(o["yaml"], "gpu-memory-utilization") == "0.88",
       json.dumps(roles) + o["stderr"][-600:])

    # E6 — 손레버 보존(음성대조): 사람이 적은 batch 는 덮어쓰지 않고 산식 값은 대조로만 기재한다.
    o = _run_case(root, tmp, "e6", cfg_extra=decl(64, L_TYP), lockset={"batch": 8})
    lk, der = o["lockset"], o["lockset"].get("batch_derivation") or {}
    ck("E6 손레버 batch=8 유지 · batch_source=hand-lever · 산식 대조값(64)은 derived_batch_would_be 로만",
       o["rc"] == 0 and lk.get("batch") == 8 and lk.get("batch_source") == "hand-lever"
       and der.get("applied") is False and der.get("derived_batch_would_be") == 64
       and _yaml_value(o["yaml"], "max-num-seqs") == "8", json.dumps(der)[:600] + o["stderr"][-400:])

    # E7 — 직전 explorer 산출은 승계하지 않는다(carry-forward ✗): batch·클램프를 비우고 2-위상을 다시 돈다.
    o = _run_case(root, tmp, "e7", cfg_extra=decl(16, L_TYP),
                  lockset={"batch": 99, "batch_source": "kv-fit-measured", "kv_cache_memory_bytes": 12345,
                           "kv_source": "measured-clamp", "provenance": "explorer-phase2"})
    lk, der = o["lockset"], o["lockset"].get("batch_derivation") or {}
    hist = (o["summary"] or {}).get("correction_history") or []
    ck("E7 직전 batch 99·클램프 12345 비승계 → trial1 언클램프 실측부터 · batch 16 재산출 · 새 클램프",
       o["rc"] == 0 and lk.get("batch") == 16 and der.get("prior_batch") == 99 and der.get("prior_kv") == 12345
       and lk.get("kv_cache_memory_bytes") not in (None, 12345)
       and bool(hist) and (hist[0].get("before") or {}).get("kv_cache_memory_bytes") is None
       and (hist[0].get("before") or {}).get("batch") is None,
       json.dumps(hist)[:500] + o["stderr"][-400:])

    # E8 — 음성대조: target_gmu 미선언 → exit 5 · 트라이얼·3종 세트·lockset 각인 0.
    t = dict(TARGET)
    t.pop("target_gmu")
    o = _run_case(root, tmp, "e8", cfg_extra=dict(decl(16, L_TYP), target_gpu=t), lockset={})
    ck("E8 target_gmu 미선언 → exit 5 · yaml 없음 · lockset 미각인(입력 그대로)",
       o["rc"] == 5 and o["yaml"] is None and "provenance" not in o["lockset"] and "target_gmu" in o["stderr"],
       "rc=%s %s" % (o["rc"], o["stderr"][-400:]))

    # E9 — 입력 출처 어휘 밖: 멈추지 않고 기재하며 사람 값으로 본다(덮어쓰기 ✗).
    o = _run_case(root, tmp, "e9", cfg_extra=decl(16, L_TYP), lockset={"batch": 12, "batch_source": "bogus"})
    lk, der = o["lockset"], o["lockset"].get("batch_derivation") or {}
    ck("E9 입력 batch_source 어휘 밖 → rc 0 · 손레버로 유지(12) · 이탈 사유가 기록과 stderr 에",
       o["rc"] == 0 and lk.get("batch") == 12 and lk.get("batch_source") == "hand-lever"
       and any("어휘 밖" in a for a in der.get("input_source_anomalies") or []) and "어휘 밖" in o["stderr"],
       json.dumps(der)[:400] + o["stderr"][-400:])

    # E10 — 배선 층의 역할 분리: 트라이얼 분류(게이트)는 gate_margin 으로, 클램프 천장은 deploy_gmu 로 판정한다.
    #   두 승수를 다시 한 변수로 접으면 같은 트라이얼이 게이트에서 먼저 죽는다(분류 note 가 바뀐다).
    o = _run_case(root, tmp, "e10", cfg_extra=decl(16, L_TYP), lockset={}, profile="heavy")
    s = o["summary"] or {}
    ck("E10 분류는 gate_margin 천장(86.4)으로 통과 → 클램프 천장(deploy_gmu 81.6)에서 vram_infeasible(HITL rc 3)",
       o["rc"] == 3 and "classify → none" in o["stderr"] and s.get("failure_class") == "vram_infeasible"
       and "budget*margin" not in (s.get("note") or "") and "weights+overhead(실측)" in (s.get("note") or ""),
       "rc=%s summary=%s" % (o["rc"], json.dumps({k: s.get(k) for k in ("failure_class", "note")})))
    ck("E10 미수렴 요약에도 역할 기록이 실린다(gate_margin 0.90 · deploy_gmu 0.85) · lockset 은 각인하지 않는다",
       (s.get("gmu_roles") or {}).get("gate_margin") == SAFETY
       and (s.get("gmu_roles") or {}).get("deploy_gmu") == TARGET["target_gmu"]
       and "provenance" not in o["lockset"], json.dumps(s.get("gmu_roles")))

    # E11 — 기재 전용 선언 축의 모양 결함은 simulate 를 멈추지 않는다.
    o = _run_case(root, tmp, "e11", cfg_extra={"declared_axes": {"concurrency_requirement": 16,
                                                                 "typical_request_tokens": L_TYP, "tp": "2"}},
                  lockset={})
    tpchk = (o["lockset"].get("declared_axes_check") or {}).get("tp") or {}
    ck("E11 declared_axes.tp='2'(문자열) → rc 0 · declared_state=malformed 기재 · stderr ⚠(기재가 게이트로 격상되지 않는다)",
       o["rc"] == 0 and tpchk.get("declared_state") == "malformed" and tpchk.get("match") is None
       and "대조하지 못했다" in o["stderr"], "rc=%s %s %s" % (o["rc"], json.dumps(tpchk), o["stderr"][-300:]))

    # E12 — host 흐름: 선언 예산(carve-out 24)이 노드 per_card(100)와 충돌 → exit 5 · 산출물 0.
    o = _run_case(root, tmp, "e12", cfg_extra=dict(decl(4, L_TYP), vram_budget_gb=24, test_device_total_gib=100),
                  lockset={}, target=False)
    ck("E12 host 흐름 vram_budget_gb=24 ↔ per_card 100 → exit 5 · yaml·각인 0(실측이 선언 carve-out 을 덮어 rc 0 으로 내지 않는다)",
       o["rc"] == 5 and o["yaml"] is None and "provenance" not in o["lockset"] and "target_gpu 블록" in o["stderr"],
       "rc=%s %s" % (o["rc"], o["stderr"][-400:]))

    # E13 — 캠페인 셀 lockset 은 --lockset-out 없이도 제자리에 기계 각인된다.
    cell_lock = root / "campaigns" / "camp-fx" / "cells" / "e13" / "lockset.json"
    o = _run_case(root, tmp, "e13", cfg_extra=decl(16, L_TYP), lockset={}, lock_path=cell_lock, lockset_out=False)
    ck("E13 --candidate 가 campaigns/<id>/cells/<cell>/lockset.json 이면 --lockset-out 없이 그 파일에 각인",
       o["rc"] == 0 and o["lockset"].get("provenance") == "explorer-phase2"
       and o["lockset"].get("trial_provenance") == "mock" and "--lockset-out 기본값" in o["stderr"],
       "rc=%s %s" % (o["rc"], o["stderr"][-400:]))
    # E14 — 음성대조: 캠페인 밖 lockset 은 --lockset-out 없으면 입력을 덮어쓰지 않는다.
    o = _run_case(root, tmp, "e14", cfg_extra=decl(16, L_TYP), lockset={}, lockset_out=False)
    ck("E14 음성대조: 캠페인 밖 lockset 은 입력 그대로 · 각인은 run_summary 에만 · 안내 줄",
       o["rc"] == 0 and o["lockset"] == o["input_lock"]
       and ((o["summary"] or {}).get("lockset") or {}).get("provenance") == "explorer-phase2"
       and "캠페인 셀 lockset 이 아니다" in o["stderr"], "rc=%s %s" % (o["rc"], o["stderr"][-300:]))


# ───────────────────────────── F — 가짜 엔진 층 ─────────────────────────────

# 사본 recipe.py 를 import 해 `run_trial` 만 결정론 엔진으로 바꾸고 `cmd_simulate` 를 부른다. mock 플래그를 쓰지 않으므로
# 되먹임 경로(feedback_append)까지 돈다 — 그 기록은 파일이 아니라 캡처로 받는다(사본 스킬 feedback/ 에도 쓰지 않는다).
DRIVER = r'''
import contextlib, importlib.util, io, json, os, sys
root, spec_path = sys.argv[1], sys.argv[2]
spec = json.load(open(spec_path, encoding="utf-8"))
with open(os.path.join(root, "output", "single", "manifest.yaml"), "w", encoding="utf-8") as fh:
    fh.write(spec["manifest"])
skill = os.path.join(root, ".claude", "skills", "vllm-recipe-explorer")
sys.path.insert(0, os.path.join(skill, "scripts"))
ms = importlib.util.spec_from_file_location("recipe_fx", os.path.join(skill, "recipe.py"))
recipe = importlib.util.module_from_spec(ms)
ms.loader.exec_module(recipe)
GIB = 1024 ** 3
eng = spec["engine"]
calls, feedback = [], []

def fake_run_trial(candidate, run_dir, trial_number, opts):
    clamp = candidate.get("kv_cache_memory_bytes")
    tp = eng["tp"]
    if clamp is None:   # 언클램프: 엔진이 트라이얼 gmu 천장까지 KV 를 잡는다(per-GPU)
        kv_bytes = (int(eng["per_card_gib"] * float(candidate["gpu_memory_utilization"]) * GIB)
                    - int(eng["weights_gib"] / tp * GIB) - int(eng["overhead_gib"] / tp * GIB))
        per_tok = eng["per_token_bytes"]
    else:               # 클램프: 엔진 토큰은 클램프에 비례한다(블록 반올림)
        kv_bytes = int(clamp)
        per_tok = int(eng["per_token_bytes"] * eng.get("clamped_per_token_scale", 1.0))
    tokens = (kv_bytes // (per_tok * eng["block"])) * eng["block"]
    load_ok = trial_number not in eng.get("oom_trials", [])
    unobserved = clamp is not None and eng.get("clamped_tokens_unobserved")
    prof = {"weights_gib": eng["weights_gib"], "non_kv_overhead_gib": eng["overhead_gib"],
            "kv_cache_gib": kv_bytes / GIB, "kv_cache_tokens": (None if unobserved else tokens),
            "max_concurrency": (None if unobserved else tokens / float(candidate["max_model_len"]))}
    calls.append({"t": trial_number, "clamp": clamp, "batch": candidate.get("batch"), "tokens": tokens,
                  "load_ok": load_ok})
    # OOM 주입 = 호스트 워치독 SIGKILL 형상(KV 를 잡은 뒤 예외 없이 종료 → sim_classify vram_oom)
    return {"trial_number": trial_number, "candidate": candidate, "load_ok": load_ok, "vllm_profile": prof,
            "functional": ({"passed": True} if load_ok else None), "log_path": None, "error_excerpt": "",
            "provenance": "mock", "health_wait": None, "effective_gmu": candidate.get("gpu_memory_utilization")}

recipe.run_trial = fake_run_trial
recipe.preload_ram_gate = lambda *a, **k: {"ok": True}
recipe.feedback_append = lambda rec, path: feedback.append(rec)
args = recipe.build_parser().parse_args(["simulate", "--config", spec["config"], "--candidate", spec["lockset"],
                                         "--lockset-out", spec["lockset"], "--run-id", spec["run_id"],
                                         "--cap", str(spec["cap"])])
err, out = io.StringIO(), io.StringIO()
rc = 0
try:
    with contextlib.redirect_stderr(err), contextlib.redirect_stdout(out):
        args.func(args)
except SystemExit as exc:
    rc = exc.code
summ = os.path.join(root, "docs", "simlog", spec["run_id"], "run_summary.json")
json.dump({"rc": rc, "calls": calls, "feedback": feedback, "stderr": err.getvalue()[-3000:],
           "lockset": json.load(open(spec["lockset"], encoding="utf-8")),
           "summary": (json.load(open(summ, encoding="utf-8")) if os.path.isfile(summ) else None)},
          open(spec["out"], "w", encoding="utf-8"), ensure_ascii=False)
'''

ENGINE = {"per_card_gib": 96, "weights_gib": 10.0, "overhead_gib": 2.0, "per_token_bytes": 131072, "block": 16,
          "tp": 1}


def _engine_case(root: Path, tmp: Path, name: str, *, cfg_extra: dict, lockset: dict | None = None,
                 engine: dict | None = None, manifest: str = MANIFEST, cap: int = 3, target: dict | None = None):
    case = tmp / "fcases" / name
    case.mkdir(parents=True, exist_ok=True)
    extra = dict(cfg_extra)
    extra.setdefault("test_device_total_gib", 96)
    cfg_path = _write_cfg(case, tmp, "f" + name, dict(extra, target_gpu=dict(target or TARGET)), True)
    lock = _base_lock("f" + name)
    lock.update(lockset or {})
    lk = case / "lockset.json"
    lk.write_text(json.dumps(lock), encoding="utf-8")
    spec = {"manifest": manifest, "engine": dict(ENGINE, **(engine or {})), "config": str(cfg_path),
            "lockset": str(lk), "run_id": "ffx_" + name, "cap": cap, "out": str(case / "result.json")}
    (case / "spec.json").write_text(json.dumps(spec), encoding="utf-8")
    drv = tmp / "fake_engine_driver.py"
    if not drv.is_file():
        drv.write_text(DRIVER, encoding="utf-8")
    proc = subprocess.run([sys.executable, str(drv), str(root), str(case / "spec.json")], cwd=str(root),
                          capture_output=True, text=True, timeout=120,
                          env=dict(os.environ, PYTHONDONTWRITEBYTECODE="1"))
    res_path = case / "result.json"
    if proc.returncode != 0 or not res_path.is_file():
        return {"rc": "driver-crash", "calls": [], "feedback": [], "lockset": {}, "summary": None,
                "stderr": proc.stderr[-2000:]}
    return json.loads(res_path.read_text(encoding="utf-8"))


def _unclamped(per_card=96, gmu=0.85, w=10.0, o=2.0, tp=1, per_tok=131072, block=16):
    kv = int(per_card * gmu * GIB) - int(w / tp * GIB) - int(o / tp * GIB)
    tokens = (kv // (per_tok * block)) * block
    return kv, tokens


def engine_layer(tmp: Path, root: Path) -> None:
    decl = lambda req, typ=L_TYP: {"declared_axes": {"concurrency_requirement": req, "typical_request_tokens": typ}}
    max_safe = int(96 * 0.85 * GIB) - int(10 * GIB) - int(2 * GIB)

    def fb_ok(r):
        fb = r["feedback"]
        return (len(fb) == 1 and fb[0].get("safety_margin_threshold") == SAFETY
                and fb[0].get("gpu_memory_utilization") == TARGET["target_gmu"])

    # F1 — 첫 트라이얼 OOM(워치독 SIGKILL 형상) → 잠정 클램프(max_model_len 1건) → 통과 → 배포 천장 환산 KV-fit 으로
    #      요구 16 유지 → 산출 batch 로 클램프 재산정 → 재검증.
    r = _engine_case(root, tmp, "oomfirst", cfg_extra=decl(16), engine={"oom_trials": [1]})
    lk, der, calls = r["lockset"], r["lockset"].get("batch_derivation") or {}, r["calls"]
    raw_fit = (calls[1]["tokens"] // L_TYP) if len(calls) > 1 else None
    ck("F1 음성대조 전제: 잠정 클램프 트라이얼의 원토큰으로 세면 KV-fit 은 요구보다 작다(%s < 16)" % raw_fit,
       raw_fit is not None and raw_fit < 16 and calls[1]["clamp"] is not None and calls[0]["load_ok"] is False,
       json.dumps(calls))
    ck("F1 OOM-first: batch 16 · declared-requirement(잠정 클램프 원토큰으로 깎지 않는다) · KV-fit 은 배포 천장 환산",
       r["rc"] == 0 and lk.get("batch") == 16 and lk.get("batch_source") == "declared-requirement"
       and "언클램프 측정 환산" in (der.get("kv_fit_tokens_source") or "") and (der.get("kv_fit") or 0) >= 16,
       json.dumps(der)[:700] + r["stderr"][-600:])
    clamp3 = calls[2]["clamp"] if len(calls) > 2 else None
    per_tok_t2 = (calls[1]["clamp"] / calls[1]["tokens"]) if len(calls) > 1 and calls[1]["tokens"] else None
    ck("F1 잠정 클램프를 산출 batch 로 재산정(required = per_token × max(len, L×16) × 버퍼) → 3번째 트라이얼이 재검증 · verified",
       len(calls) == 3 and clamp3 == int(per_tok_t2 * max(MAX_LEN, L_TYP * 16) * BUFFER)
       and calls[2]["batch"] == 16 and lk.get("kv_cache_memory_bytes") == clamp3
       and lk.get("kv_source") == "measured-clamp" and der.get("verified") is True,
       "calls=%s clamp3=%s" % (json.dumps(calls), clamp3))
    ck("F1 되먹임: safety_margin_threshold == gate_margin(0.90) · gpu_memory_utilization == deploy_gmu(0.85)",
       fb_ok(r), json.dumps(r["feedback"])[:400])

    # F2 — 첫 트라이얼 OOM ∧ 요구 없음 → batch 미정 · 잠정 클램프가 곧 최종(1건 기준) · 빈 트라이얼 없이 2칸 수렴.
    r = _engine_case(root, tmp, "oomnoreq", cfg_extra=decl(None, None), engine={"oom_trials": [1]})
    lk, calls = r["lockset"], r["calls"]
    ck("F2 OOM-first ∧ 요구 없음 → batch 미정 · 2트라이얼 수렴 · 클램프 = 1건(max_model_len) 기준",
       r["rc"] == 0 and len(calls) == 2 and lk.get("batch") is None and lk.get("batch_source") is None
       and lk.get("kv_cache_memory_bytes") == calls[1]["clamp"]
       and calls[1]["clamp"] < 0.1 * max_safe, "calls=%s %s" % (json.dumps(calls), r["stderr"][-400:]))

    # F3 — 요구 없음(정상): 천장 클램프 ✗ · max_model_len 1건 클램프.
    r = _engine_case(root, tmp, "noreq", cfg_extra=decl(None, None))
    lk, der, calls = r["lockset"], r["lockset"].get("batch_derivation") or {}, r["calls"]
    kv1, tok1 = _unclamped()
    ck("F3 요구 없음 → batch 미정 · 클램프 = per_token × max_model_len × 버퍼(1건) · 배포 천장의 10% 미만 · KV-fit 참고값",
       r["rc"] == 0 and lk.get("batch") is None and len(calls) == 2
       and lk.get("kv_cache_memory_bytes") == int(kv1 / tok1 * MAX_LEN * BUFFER)
       and lk.get("kv_cache_memory_bytes") < 0.1 * max_safe and isinstance(der.get("kv_fit"), int),
       "kv=%s calls=%s %s" % (lk.get("kv_cache_memory_bytes"), json.dumps(calls), r["stderr"][-400:]))

    # F4 — 입력 사람 클램프(40 GiB) ∧ batch 미정: 그 클램프가 배포값이다 → 엔진 원토큰으로 산출 · 재검증 · kv_source=hand.
    r = _engine_case(root, tmp, "handclamp", cfg_extra=decl(16), lockset={"kv_cache_memory_bytes": 40 * GIB})
    lk, der, calls = r["lockset"], r["lockset"].get("batch_derivation") or {}, r["calls"]
    hand_tokens = (40 * GIB // (131072 * 16)) * 16
    ck("F4 사람 클램프 → KV-fit = 그 클램프의 엔진 원토큰 ÷ L(%d) · batch 16 · 재검증 트라이얼 · kv_source=hand"
       % (hand_tokens // L_TYP),
       r["rc"] == 0 and der.get("kv_fit") == hand_tokens // L_TYP and "배포 클램프 트라이얼" in (der.get("kv_fit_tokens_source") or "")
       and lk.get("batch") == 16 and len(calls) == 2 and calls[1]["batch"] == 16
       and lk.get("kv_cache_memory_bytes") == 40 * GIB and lk.get("kv_source") == "hand"
       and der.get("verified") is True, json.dumps(der)[:500] + json.dumps(calls))

    # F5 — TP=2 타겟 흐름: 측정 weights/overhead 를 TP 로 나눠 KV-fit 을 환산한다(클램프 산식과 같은 ÷TP).
    r = _engine_case(root, tmp, "tp2", cfg_extra=decl(16), manifest=MANIFEST_MULTI2,
                     engine={"weights_gib": 40.0, "overhead_gib": 4.0, "tp": 2})
    lk, der, calls = r["lockset"], r["lockset"].get("batch_derivation") or {}, r["calls"]
    kv_t, tok_t = _unclamped(w=40.0, o=4.0, tp=2)
    safe_div = int(96 * 0.85 * GIB) - int(20 * GIB) - int(2 * GIB)
    safe_nodiv = int(96 * 0.85 * GIB) - int(40 * GIB) - int(4 * GIB)
    want_div, want_nodiv = _fit_tokens(safe_div, kv_t / tok_t), _fit_tokens(safe_nodiv, kv_t / tok_t)
    ck("F5 TP=2: tp_divisor=2 · KV-fit 토큰 = (천장 − weights/2 − overhead/2) 환산(%d) — ÷TP 를 빼면 %d 가 된다"
       % (want_div, want_nodiv),
       r["rc"] == 0 and (lk.get("gmu_roles") or {}).get("tp_divisor") == 2
       and der.get("kv_fit_tokens") == want_div and want_div != want_nodiv and lk.get("batch") == 16,
       json.dumps(der)[:500] + r["stderr"][-600:])

    # F6 — 클램프 트라이얼의 엔진 토큰 미관측 → 위상 2 재검증 불가 → verified=false(일치로 접지 않는다).
    r = _engine_case(root, tmp, "unobserved", cfg_extra=decl(16), engine={"clamped_tokens_unobserved": True})
    lk, der = r["lockset"], r["lockset"].get("batch_derivation") or {}
    ck("F6 클램프 토큰 미관측 → 수렴하되 verified=false · verified_gap 에 사유(미관측)",
       r["rc"] == 0 and lk.get("batch") == 16 and der.get("verified") is False
       and "미관측" in (der.get("verified_gap") or ""), json.dumps(der)[:500])

    # F7 — per-token 불일치(클램프 트라이얼 엔진 per-token ×1.34): 위상 2 가 실제로 낮춘다.
    mismatch = {"clamped_per_token_scale": 1.34}
    r = _engine_case(root, tmp, "mismatch", cfg_extra=decl(64), engine=mismatch)
    lk, der, calls = r["lockset"], r["lockset"].get("batch_derivation") or {}, r["calls"]
    low = (calls[1]["tokens"] // L_TYP) if len(calls) > 1 else None
    ck("F7 per-token 불일치 → 위상 2 가 64→%s 로 낮추고 같은 클램프로 재검증 통과(클램프는 재산정하지 않는다)" % low,
       r["rc"] == 0 and low is not None and low < 64 and lk.get("batch") == low
       and (der.get("phase2_adjustments") or [{}])[0].get("batch_before") == 64 and der.get("verified") is True
       and len(calls) == 3 and calls[1]["clamp"] == calls[2]["clamp"], json.dumps(calls) + json.dumps(der)[:300])

    # F8 — 같은 구성 --cap 2: 마지막 칸에서 낮췄다 → HITL(rc 3) 요약이 사유를 말한다 · batch_derivation 이 실린다.
    r = _engine_case(root, tmp, "mismatch_cap2", cfg_extra=decl(64), engine=mismatch, cap=2)
    s = r["summary"] or {}
    ck("F8 cap 2 소진 → rc 3 · note 에 'cap(2) 소진 — 위상 2' · 요약 batch_derivation.phase2_adjustments · lockset 미각인",
       r["rc"] == 3 and "cap(2) 소진" in (s.get("note") or "") and "위상 2" in (s.get("note") or "")
       and ((s.get("batch_derivation") or {}).get("phase2_adjustments") or [{}])[0].get("batch_before") == 64
       and "provenance" not in r["lockset"], json.dumps({k: s.get(k) for k in ("failure_class", "note")}))

    # F9 — per-token 일치(실물 경계): 요구 64 ≤ 환산 KV-fit → 위상 2 하향 없음 · 2칸 수렴.
    r = _engine_case(root, tmp, "realistic64", cfg_extra=decl(64))
    lk, der, calls = r["lockset"], r["lockset"].get("batch_derivation") or {}, r["calls"]
    ck("F9 per-token 일치 → 클램프 트라이얼 엔진 토큰이 batch×L 을 담는다 · 하향 없음 · verified · 2칸",
       r["rc"] == 0 and lk.get("batch") == 64 and not der.get("phase2_adjustments") and der.get("verified") is True
       and len(calls) == 2 and calls[1]["tokens"] // L_TYP >= 64 and fb_ok(r),
       json.dumps(calls) + json.dumps(der)[:300])


def main() -> int:
    print("[selftest_gmu_roles_batch] plan_26091407 §4.2·§4.3 · §7 O3")
    with tempfile.TemporaryDirectory(prefix="selftest_gmu_roles_batch_") as td:
        tmp = Path(td)
        unit_layer(tmp)
        root = _fixture_repo(tmp)
        cli_layer(tmp, root)
        engine_layer(tmp, root)
    print("[selftest_gmu_roles_batch] %s" % ("PASS" if not failures else "FAIL (%d)" % len(failures)))
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
