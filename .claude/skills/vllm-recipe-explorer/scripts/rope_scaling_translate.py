#!/usr/bin/env python3
# rope_scaling_translate.py — 컨텍스트 확장 **선언**을 serve 인자로 번역한다
#   (2026-09-11 신설 · plan_26091108 R8)
#
# 왜 있는가. camp-26090918 의 셀 선언은 `declared_axes.yarn: factor 2` 를 들고 있었고 계획서는
#   축을 `512K(YaRN f2)·1M(YaRN f4)` 로 선포했는데, **그 선언을 serve 인자로 옮기는 실행자가
#   처음부터 없었다.** serve 트리플렛에는 rope 인자가 없었고 주석은 "YaRN off · 공식 네이티브 max"
#   라는 거짓을 적고 있었다(524288 은 네이티브가 아니다). 셀은 로드 전 ValidationError 로 즉사했고,
#   void_reason 은 그것을 "YaRN 확장 미적용(셀 축에 미포함)" 이라 적어 **원인을 반대로** 기록했다.
#
# ★ 병합이 핵심이다. 이 모델의 rope 설정은 `text_config.rope_parameters` 한 딕셔너리 안에
#   `mrope_interleaved`·`mrope_section`·`partial_rotary_factor`·`rope_theta` 와 **함께** 산다.
#   `--hf-overrides` 로 그 딕셔너리를 통째로 갈아끼우면 나머지 네 키가 조용히 사라지고, 모델은
#   뜨긴 뜨되 **다른 모델이 된다**(위치 인코딩이 바뀐다). 그래서 이 번역기는 기존 키를 전부 실어
#   보내는 **병합 결과**를 낸다. 이것이 손저작으로는 반복 재현하기 어려운 부분이다.
#
# ★ 정직한 공백. `mrope_interleaved=True` + `partial_rotary_factor=0.25` 위에서 YaRN 이 어떻게
#   합성되는지는 **이 하드웨어·이 vLLM 에서 검증된 바 없다**. 번역기는 그 사실을 산출물에
#   `unknowns[]` 로 싣고 가린 채 통과시키지 않는다 — 서빙 스모크가 유일한 중재자다.
#
# 사용: rope_scaling_translate.py --model-dir DIR --target-len N [--factor F] [--json]
#       rope_scaling_translate.py --self-test
# 종료: 0=번역 산출(또는 확장 불요) · 3=번역 불가(사유 명시) · 2=인자 오류
import argparse
import json
import math
import os
import sys

# rope 설정이 사는 자리. transformers 레이아웃이 두 세대 공존한다 — 둘 다 본다.
#   (신) text_config.rope_parameters · (구) text_config.rope_scaling / 최상위 rope_scaling
ROPE_CONTAINERS = (
    ("text_config", "rope_parameters"),
    ("text_config", "rope_scaling"),
    ("rope_parameters",),
    ("rope_scaling",),
)
# YaRN 이 확장할 수 있는 출발 상태. 이미 확장된 설정을 다시 확장하면 이중 적용이 된다.
EXTENDABLE_ROPE_TYPES = ("default", "linear", None)


def _get(cfg, path):
    cur = cfg
    for key in path:
        if not isinstance(cur, dict) or key not in cur:
            return None
        cur = cur[key]
    return cur if isinstance(cur, dict) else None


def _max_pos(cfg):
    for path in (("text_config", "max_position_embeddings"), ("max_position_embeddings",)):
        cur = cfg
        ok = True
        for key in path:
            if not isinstance(cur, dict) or key not in cur:
                ok = False
                break
            cur = cur[key]
        if ok and isinstance(cur, (int, float)):
            return int(cur), ".".join(path)
    return None, None


def translate(cfg, target_len, factor=None):
    """모델 config + 목표 컨텍스트 → serve 에 실을 override. 순수 함수."""
    native, native_src = _max_pos(cfg)
    out = {
        "target_len": int(target_len),
        "native_max_position_embeddings": native,
        "native_source": native_src,
        "needed": None, "hf_overrides": None, "container_path": None,
        "factor": None, "unknowns": [], "reasons": [],
    }
    if native is None:
        out["reasons"].append(
            "모델 config 에서 max_position_embeddings 를 찾지 못했다 — 확장 필요 여부를 "
            "판정할 수 없다(추측하지 않는다).")
        return out
    if int(target_len) <= native:
        out["needed"] = False
        out["reasons"].append(
            "목표 %d ≤ 네이티브 %d — 확장이 필요 없다. rope 인자를 넣지 않는 것이 옳다."
            % (int(target_len), native))
        return out
    out["needed"] = True

    container, path = None, None
    for cand in ROPE_CONTAINERS:
        got = _get(cfg, cand)
        if got is not None:
            container, path = got, cand
            break
    if container is None:
        out["reasons"].append(
            "rope 설정 딕셔너리를 찾지 못했다(%s 어디에도 없다) — 어느 키에 얹어야 하는지 모른 채 "
            "override 를 만들면 조용히 무시된다." % " · ".join(".".join(c) for c in ROPE_CONTAINERS))
        return out
    out["container_path"] = ".".join(path)

    rope_type = container.get("rope_type") or container.get("type")
    if rope_type not in EXTENDABLE_ROPE_TYPES:
        out["reasons"].append(
            "이미 rope_type=%r 로 선언돼 있다 — 그 위에 YaRN 을 얹으면 **이중 적용**이다. "
            "모델이 이미 확장된 것인지 먼저 확인하라." % rope_type)
        return out

    f = float(factor) if factor else math.ceil(int(target_len) / native)
    if native * f < int(target_len):
        out["reasons"].append(
            "factor %g 로는 %d 에 닿지 않는다(%d × %g = %d). 선언한 factor 가 목표와 "
            "어긋난다 — 둘 중 하나가 틀렸다." % (f, int(target_len), native, f, int(native * f)))
        return out
    out["factor"] = f

    # ★ 병합 — 기존 키를 전부 싣는다. 갈아끼우면 mrope·partial rotary 가 사라진다.
    merged = dict(container)
    merged["rope_type"] = "yarn"
    merged["factor"] = f
    merged["original_max_position_embeddings"] = native
    node = merged
    for key in reversed(path):
        node = {key: node}
    out["hf_overrides"] = node
    out["reasons"].append(
        "네이티브 %d → 목표 %d 를 YaRN factor %g 로 연다. 기존 rope 키(%s)를 **전부 실어** "
        "보낸다 — 딕셔너리를 갈아끼우면 그 키들이 조용히 사라지고 위치 인코딩이 바뀐다."
        % (native, int(target_len), f, ", ".join(sorted(container)) or "없음"))

    if container.get("mrope_interleaved") or container.get("mrope_section"):
        out["unknowns"].append(
            "mrope(interleaved=%r · section=%r) 위에서 YaRN 이 어떻게 합성되는지 이 하드웨어·이 "
            "vLLM 에서 검증된 바 없다. 스모크가 유일한 중재자다."
            % (container.get("mrope_interleaved"), container.get("mrope_section")))
    prf = container.get("partial_rotary_factor")
    if prf is not None and float(prf) != 1.0:
        out["unknowns"].append(
            "partial_rotary_factor=%s — 회전 차원이 head_dim 의 일부다. YaRN 의 주파수 보간이 "
            "부분 회전 구간에만 적용되는지 전 차원에 걸리는지는 구현 의존이며 미검증이다." % prf)
    return out


def _self_test():
    failures = []

    def ck(name, cond, detail=""):
        print(("  [PASS] " if cond else "  [FAIL] ") + name + ("" if cond else " " + detail))
        if not cond:
            failures.append(name)

    cfg = {"text_config": {"max_position_embeddings": 262144,
                           "rope_parameters": {"rope_type": "default", "rope_theta": 10000000,
                                               "mrope_interleaved": True,
                                               "mrope_section": [11, 11, 10],
                                               "partial_rotary_factor": 0.25}}}
    r = translate(cfg, 262144)
    ck("Y1 목표가 네이티브 이하면 확장하지 않는다(불필요한 인자를 넣지 않는다)",
       r["needed"] is False and r["hf_overrides"] is None)

    r = translate(cfg, 524288, factor=2)
    rp = r["hf_overrides"]["text_config"]["rope_parameters"]
    ck("Y2 factor 2 로 512k 를 연다", r["needed"] and rp["rope_type"] == "yarn"
       and rp["factor"] == 2 and rp["original_max_position_embeddings"] == 262144)
    ck("★Y3 병합: 기존 rope 키가 **전부 살아남는다**(갈아끼우면 다른 모델이 된다)",
       rp["mrope_interleaved"] is True and rp["mrope_section"] == [11, 11, 10]
       and rp["partial_rotary_factor"] == 0.25 and rp["rope_theta"] == 10000000)
    ck("★Y4 미검증 상호작용을 산출물이 스스로 밝힌다(가린 채 통과 ✗)",
       len(r["unknowns"]) == 2 and any("mrope" in u for u in r["unknowns"])
       and any("partial_rotary_factor" in u for u in r["unknowns"]))
    ck("Y5 factor 미지정이면 목표에서 올림으로 파생한다",
       translate(cfg, 1048576)["factor"] == 4)
    r = translate(cfg, 1048576, factor=2)
    ck("★Y6 factor 가 목표에 못 미치면 거부한다(조용히 모자란 확장 ✗)",
       r["hf_overrides"] is None and any("닿지 않는다" in x for x in r["reasons"]))

    already = {"text_config": {"max_position_embeddings": 262144,
                               "rope_parameters": {"rope_type": "yarn", "factor": 2}}}
    r = translate(already, 524288)
    ck("★Y7 이미 확장된 모델에 다시 얹지 않는다(이중 적용 방지)",
       r["hf_overrides"] is None and any("이중 적용" in x for x in r["reasons"]))

    r = translate({"text_config": {"max_position_embeddings": 262144}}, 524288)
    ck("★Y8 rope 딕셔너리를 못 찾으면 override 를 지어내지 않는다(조용히 무시될 인자 금지)",
       r["hf_overrides"] is None and any("찾지 못했다" in x for x in r["reasons"]))

    r = translate({}, 524288)
    ck("Y9 네이티브를 모르면 판정하지 않는다", r["needed"] is None)

    legacy = {"rope_scaling": {"rope_type": "default"}, "max_position_embeddings": 8192}
    r = translate(legacy, 16384, factor=2)
    ck("Y10 구 레이아웃(최상위 rope_scaling)도 같은 규칙으로 번역한다",
       r["hf_overrides"]["rope_scaling"]["rope_type"] == "yarn"
       and r["container_path"] == "rope_scaling")

    # 음성대조 — 병합을 끄면(갈아끼우면) 기존 키가 사라진다는 것을 시험이 붙잡는가.
    merged = translate(cfg, 524288, factor=2)["hf_overrides"]["text_config"]["rope_parameters"]
    naive = {"rope_type": "yarn", "factor": 2, "original_max_position_embeddings": 262144}
    ck("★Y11 음성대조: 순진한 override 는 mrope·partial rotary 를 잃는다(병합이 그것을 막는다)",
       set(naive) < set(merged) and "mrope_section" not in naive)

    print("[rope_scaling_translate] %s" % ("PASS" if not failures else "FAIL (%d)" % len(failures)))
    return 0 if not failures else 1


def main():
    ap = argparse.ArgumentParser(description="컨텍스트 확장 선언 → serve 인자 번역 (R8)")
    ap.add_argument("--model-dir", help="모델 디렉터리(config.json 을 읽는다)")
    ap.add_argument("--target-len", type=int, help="목표 max-model-len")
    ap.add_argument("--factor", type=float, help="선언된 YaRN factor(생략 시 목표에서 파생)")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        sys.exit(_self_test())
    if not a.model_dir or not a.target_len:
        print("[rope-translate] FAIL: --model-dir 과 --target-len 이 필요하다", file=sys.stderr)
        sys.exit(2)
    path = os.path.join(a.model_dir, "config.json")
    try:
        with open(path, encoding="utf-8") as f:
            cfg = json.load(f)
    except (OSError, ValueError) as exc:
        print("[rope-translate] FAIL: config.json 을 읽지 못했다 — %s" % exc, file=sys.stderr)
        sys.exit(2)
    out = translate(cfg, a.target_len, a.factor)
    if a.json:
        print(json.dumps(out, ensure_ascii=False, indent=2))
    else:
        for r in out["reasons"]:
            print("[rope-translate] %s" % r)
        for u in out["unknowns"]:
            print("[rope-translate] ⚠ 미검증: %s" % u)
        if out["hf_overrides"]:
            print("[rope-translate] serve 트리플렛에 넣을 줄:")
            print("hf-overrides: '%s'" % json.dumps(out["hf_overrides"], separators=(",", ":")))
    if out["needed"] and not out["hf_overrides"]:
        sys.exit(3)
    sys.exit(0)


if __name__ == "__main__":
    main()
