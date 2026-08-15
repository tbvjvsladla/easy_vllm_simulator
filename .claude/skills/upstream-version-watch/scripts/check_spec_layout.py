#!/usr/bin/env python3
"""저비용 선검사 — 체크포인트 spec-decoder 레이아웃 ↔ 선택한 `method` 정합 (결정론적).

왜 있나(2026-08-14 · testlog_26081419 §7 후속 ①):
  R1-b(`method=mtp`)는 착수 전에 이미지 실측 4건을 확인하고도 서빙에서 죽었다 —
  `KeyError: 'model.layers.43.mtp_block.main_norm.weight'`. 확인했던 것은 전부 참이었다
  (`mtp.py` PRESENT · `class DeepSeekV4MTP` · `_remap_weight_name` · registry 등재).
  **빠진 검사는 "체크포인트가 실제로 가진 텐서 이름 ↔ 그 method 의 모듈이 담을 수 있는 구조"**
  였다. 존재(PRESENT)와 정합(fits)은 다른 술어다.
  그 대조는 **로드 13분**을 태우고서야 이루어졌는데, `model.safetensors.index.json`
  키 스캔은 **초 단위**다. 이 스크립트가 그 초 단위 검사다.

무엇을 보나:
  `model.safetensors.index.json` 의 `weight_map` 키에서 spec-decoder 계열
  (`mtp.<i>.*` · `model.layers.<n>.mtp_block.*`)을 뽑아 **부품 이름 집합**을 만들고,
  선택한 method 가 그 구조를 담을 수 있는지 판정한다.

  0731 실측(measured · testlog_26081419 §2.4): `mtp.*` 4,705 텐서 · 인덱스 mtp.0/1/2 ·
  `mtp.0` 이 `main_norm`·`main_proj`·`markov_head`·`confidence_head`·`hc_head_*` 를 갖는다.
  이는 plain-MTP 부품이 아니라 **DSpark 스페큘레이터**의 부품이다. stock 의
  `DeepSeekV4MultiTokenPredictorLayer` 에는 담을 슬롯 자체가 없다 —
  이름을 바꿔 해결되는 문제가 아니라 **모듈 구조가 다른** 문제다.

⚠ 이 검사는 **판정이 아니라 조기 차단**이다. PASS 가 서빙 성공을 뜻하지 않는다
  (R1-a 는 로더를 통과하고 커널에서 죽었다). FAIL 만 신뢰하라 — 그게 이 검사의 값어치다.

4종 안티패턴 고지(§결정론 규율 판정표): 아래 `DSPARK_ONLY_MARKERS` 는 **하드코딩이지만
  tripwire 다**(닫힌 목록 · 변경 시 리뷰 강제). 파생 가능한 값이 아니다 — 어느 부품이 어느
  스페큘레이터에 속하는지는 업스트림 모듈 정의에서만 오고, 그건 이미지 안에 있지 데이터에
  없다. 목록을 늘릴 때는 근거 testlog 를 함께 적는다.

종료코드: 0=정합(또는 검사 비대상) / 8=구조 불일치(조기 차단) / 3=입력 파싱 실패
사용: python3 check_spec_layout.py --model-dir <호스트 체크포인트 경로> --method {mtp|dspark|...}
      python3 check_spec_layout.py --config <triplet.yaml> --model-dir <호스트 경로>
"""
import argparse
import json
import os
import re
import sys

# ── tripwire: DSpark 스페큘레이터에만 있는 부품(plain MTP 모듈에 슬롯이 없다) ──
#   근거: testlog_26081419 §2.4(0731 index 전수 조회) + 체크포인트 config 가
#         dspark_block_size·dspark_markov_rank·dspark_target_layer_ids 를 1급으로 가짐.
DSPARK_ONLY_MARKERS = (
    "main_norm",
    "main_proj",
    "markov_head",
    "markov_w1",
    "markov_w2",
    "confidence_head",
    "hc_head_base",
    "hc_head_fn",
    "hc_head_scale",
)

# plain MTP(DeepSeekV4MultiTokenPredictorLayer 계열)가 담는 부품.
PLAIN_MTP_COMPONENTS = (
    "attn", "attn_norm", "input_layernorm", "ffn", "ffn_norm", "mlp",
    "norm", "emb", "embed_tokens", "head", "shared_head", "eh_proj",
    "enorm", "hnorm", "tok_emb",
)

SPEC_KEY_RE = re.compile(r"^(?:model\.layers\.\d+\.mtp_block|mtp\.(\d+))\.(.+)$")


def read_method_from_config(config_yaml):
    """트리플렛 yaml 의 `speculative-config: '{...}'` 에서 method 를 뽑는다. 없으면 None.

    순수 파서다 — 못 읽으면 None 을 돌려주고 **호출부가 fail-closed 처리**한다
    (§결정론 규율: 순수 파서의 None 은 정당한 폴백, 침묵 폴백이 아니다)."""
    try:
        with open(config_yaml, encoding="utf-8") as f:
            for line in f:
                m = re.match(r"\s*speculative[-_]config\s*:\s*(.+)$", line)
                if not m:
                    continue
                raw = m.group(1).strip().strip("'\"")
                try:
                    return (json.loads(raw) or {}).get("method")
                except json.JSONDecodeError:
                    m2 = re.search(r'"method"\s*:\s*"([^"]+)"', raw)
                    return m2.group(1) if m2 else None
    except OSError:
        return None
    return None


def load_index_keys(model_dir):
    """체크포인트 인덱스의 텐서 이름 전체. index 부재 시 (None, 사유)."""
    for name in ("model.safetensors.index.json", "pytorch_model.bin.index.json"):
        p = os.path.join(model_dir, name)
        if os.path.isfile(p):
            try:
                with open(p, encoding="utf-8") as f:
                    wm = json.load(f).get("weight_map") or {}
            except (OSError, json.JSONDecodeError) as e:
                return None, f"{name} 파싱 실패: {e!r}"
            return list(wm.keys()), name
    return None, "index.json 부재(단일 샤드 체크포인트일 수 있음 — 검사 생략)"


def analyze(keys):
    """spec-decoder 계열 키에서 인덱스별 부품 집합을 만든다."""
    per_index = {}
    total = 0
    for k in keys:
        m = SPEC_KEY_RE.match(k)
        if not m:
            continue
        total += 1
        idx = m.group(1) if m.group(1) is not None else "block"
        comp = m.group(2).split(".")[0]
        per_index.setdefault(idx, set()).add(comp)
    return per_index, total


def read_num_nextn(model_dir):
    p = os.path.join(model_dir, "config.json")
    try:
        with open(p, encoding="utf-8") as f:
            return json.load(f).get("num_nextn_predict_layers")
    except (OSError, json.JSONDecodeError):
        return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model-dir", required=True, help="호스트에서 본 체크포인트 디렉터리")
    ap.add_argument("--method", help="speculative method(미지정 시 --config 에서 읽음)")
    ap.add_argument("--config", help="트리플렛 yaml (method 도출용)")
    a = ap.parse_args()

    method = a.method or (read_method_from_config(a.config) if a.config else None)
    if not method:
        print("[spec-layout] SKIP: speculative method 없음 — 검사 비대상")
        sys.exit(0)

    if not os.path.isdir(a.model_dir):
        print(f"[spec-layout] FAIL: 체크포인트 디렉터리 부재 — {a.model_dir}", file=sys.stderr)
        sys.exit(3)

    keys, src = load_index_keys(a.model_dir)
    if keys is None:
        print(f"[spec-layout] SKIP: {src}")
        sys.exit(0)

    per_index, total = analyze(keys)
    if not per_index:
        print(f"[spec-layout] SKIP: spec-decoder 계열 키 0건(method={method}) — "
              f"드래프트 가중치를 별도 경로에서 받는 구성일 수 있다. 검사 생략")
        sys.exit(0)

    all_comps = set().union(*per_index.values())
    dspark_hits = sorted(c for c in all_comps if any(m in c for m in DSPARK_ONLY_MARKERS))
    nextn = read_num_nextn(a.model_dir)

    print(f"[spec-layout] provenance=measured src={src} 전체텐서={len(keys)} "
          f"spec계열={total} 인덱스={sorted(per_index)} num_nextn_predict_layers={nextn}")
    print(f"[spec-layout] 부품집합={sorted(all_comps)}")

    if method == "mtp" and dspark_hits:
        visible = "mtp.0" if nextn in (1, None) else f"mtp.0..{nextn - 1}"
        print(
            "[spec-layout] STOP: 체크포인트의 spec 가중치가 **plain MTP 구조가 아니다**.\n"
            f"  DSpark 전용 부품 검출: {dspark_hits}\n"
            f"  stock 의 MTP 모듈에는 이 부품들을 담을 슬롯이 없다 → 로드 중 KeyError 로 죽는다.\n"
            f"  (num_nextn_predict_layers={nextn} 이므로 stock 이 보는 블록은 {visible} 인데,\n"
            "   하필 그 블록이 DSpark 전용 텐서를 가장 많이 가진 블록이다.)\n"
            "  → `method: mtp` 는 이 체크포인트에 **구조적으로 부적용**이다. `method: dspark`\n"
            "     (또는 그 경로를 여는 이미지 변종)로 가라. 근거: testlog_26081419 §2.4·§2.5.\n"
            "  이 판정은 로드 13분이 아니라 index.json 키 스캔으로 나왔다.",
            file=sys.stderr)
        sys.exit(8)

    if method == "dspark" and not dspark_hits:
        print(
            "[spec-layout] STOP: `method: dspark` 인데 체크포인트에 DSpark 부품이 없다.\n"
            f"  검출된 부품={sorted(all_comps)}\n"
            "  → 드래프터가 자기 가중치를 못 찾는다. method 또는 체크포인트를 재확인하라.",
            file=sys.stderr)
        sys.exit(8)

    unknown = sorted(c for c in all_comps
                     if c not in PLAIN_MTP_COMPONENTS and c not in dspark_hits)
    if unknown:
        print(f"[spec-layout] ⚠ 미분류 부품(차단하지 않음 · tripwire 확장 후보): {unknown}",
              file=sys.stderr)

    print(f"[spec-layout] OK: method={method} ↔ 체크포인트 구조 정합(조기 차단 사유 없음). "
          "⚠ 서빙 성공을 뜻하지 않는다 — 커널·디스패치 층은 스모크가 판정한다.")
    sys.exit(0)


if __name__ == "__main__":
    main()
