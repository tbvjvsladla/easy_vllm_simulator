#!/usr/bin/env python3
"""crosscheck_model_card.py — HF 원본 모델카드 교차검증 (vllm-recipe-explorer).

폐쇄망 전제. 모델 디렉토리의 **번들 README.md(=HF 원본 모델카드)** + `inference/requirements.txt`
+ `config.json` + safetensors 헤더(dtype 실측)를 교차대조해, config.json **단독** 파싱이 놓치는
사실을 serving 착수 *전*에 노출한다:
  · coarse quant 라벨 함정(config `quant_method:fp8` 인데 실제 experts=FP4 혼합)
  · special 커널/엔진 의존(DeepGEMM·tilelang·flash_attn …) — novel-arch 구동 사전경보
  · 정밀도·파라미터·컨텍스트·아키 카드값 ↔ config/실측 dtype 합치

동기(실증, plan_2026062811_2 item③): config.json `quant_method:fp8`(coarse) + `du` 아티팩트만 봐
DeepSeek-V4-Flash 를 순수 FP8/298GB/인피저블로 **오판** → 모델카드 README 가 `FP4+FP8 Mixed(experts FP4)`
명시했고 `inference/requirements.txt` 가 `tilelang` 명시. 카드 우선 참조 시 오판·DeepGEMM-class 함정 조기 포착.

결정론(stdlib only). **네트워크 호출 없음**(번들 README = HF 원본 카드 — 모델과 함께 받음). 헌법 §금지 "참조-그라운디드".
**MISMATCH 1건 이상이면 비0 종료(게이트)** — parse 직후 필수 루틴(SKILL.md §2 ①).
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import re
import struct
import sys
from collections import Counter

# novel-arch serving 에서 엔진(vLLM)에 별도 설치/op 가 필요한 special 커널·라이브러리.
# 발견 시 "vLLM dry-init / op 가용성 확인" 권고(예: DeepSeek-V4 DSA 가 DeepGEMM 요구 → SparseAttnIndexer RuntimeError).
KNOWN_SPECIAL_DEPS = {
    "deepgemm": "FP8 블록 GEMM / DeepSeek 희소어텐션(DSA) 커널 — 미설치 시 SparseAttnIndexer RuntimeError",
    "tilelang": "타일 기반 커널 DSL(DeepSeek inference 커널)",
    "fast_hadamard_transform": "Hadamard 변환 커널(양자화/rotation)",
    "flash_attn": "FlashAttention 커널",
    "flashinfer": "FlashInfer 어텐션/MoE 커널",
    "sgl_kernel": "SGLang 커널",
    "sgl-kernel": "SGLang 커널",
    "causal_conv1d": "Mamba/SSM 커널",
    "mamba_ssm": "Mamba SSM 커널",
    "triton": "Triton JIT 커널",
    "xformers": "xFormers 어텐션",
    "apex": "NVIDIA Apex(fused 커널/norm)",
}

# safetensors dtype → 정밀도 클래스(교차검증용). 4-bit 는 보통 int8 패킹 + 마이크로스케일(E8M0/UE8M0).
_FP8 = {"F8_E4M3", "F8_E5M2", "FLOAT8_E4M3FN", "FLOAT8_E5M2"}
_SCALE = {"F8_E8M0", "F8_UE8M0", "UE8M0"}
_HIGH = {"BF16", "F16", "F32", "FLOAT16", "BFLOAT16", "FLOAT32"}
_INTPACK = {"I8", "U8", "UINT8", "INT8", "I4", "U4"}


def _read_st_header(path: str) -> dict:
    with open(path, "rb") as fh:
        n = struct.unpack("<Q", fh.read(8))[0]
        return json.loads(fh.read(n))


def scan_dtypes(model_dir: str) -> dict:
    """전 safetensors 헤더만 읽어 dtype별 텐서수·byte + expert/비-expert 분리 + 정밀도 시그니처."""
    shards = sorted(glob.glob(os.path.join(model_dir, "*.safetensors")))
    if not shards:
        return {"present": False}
    counts, nbytes = Counter(), Counter()
    expert_dt, other_dt = Counter(), Counter()
    for f in shards:
        try:
            hdr = _read_st_header(f)
        except Exception:
            continue
        for k, v in hdr.items():
            if k == "__metadata__" or not isinstance(v, dict):
                continue
            dt = v.get("dtype")
            off = v.get("data_offsets", [0, 0])
            nb = (off[1] - off[0]) if len(off) == 2 else 0
            counts[dt] += 1
            nbytes[dt] += nb
            is_expert = ("expert" in k.lower()) and ("shared" not in k.lower())
            (expert_dt if is_expert else other_dt)[dt] += 1
    total = sum(nbytes.values()) or 1
    has_intpack = any(d in _INTPACK for d in counts)
    has_scale = any(d in _SCALE for d in counts)
    has_fp8 = any(d in _FP8 for d in counts)
    has_high = any(d in _HIGH for d in counts)
    # 4-bit(MXFP4-류) = int8 패킹 + 마이크로스케일 동반 (expert 가중치에 집중)
    fourbit = has_intpack and has_scale
    classes = []
    if fourbit:
        classes.append("4bit(MXFP4-pack: int8+microscale)")
    if has_fp8:
        classes.append("fp8")
    if has_high:
        classes.append("bf16/fp16")
    return {
        "present": True,
        "num_shards": len(shards),
        "tensor_counts": dict(counts),
        "byte_gib": {k: round(b / 2**30, 2) for k, b in nbytes.items()},
        "total_gib": round(total / 2**30, 2),
        "expert_dtypes": dict(expert_dt),
        "nonexpert_dtypes": dict(other_dt),
        "precision_classes": classes,
        "is_mixed_precision": len(classes) >= 2,
        "fourbit_experts": fourbit and bool(expert_dt) and any(d in _INTPACK for d in expert_dt),
    }


def config_precision(cfg: dict) -> str:
    qc = cfg.get("quantization_config")
    if isinstance(qc, dict):
        qm = qc.get("quant_method") or qc.get("quant_algo") or "?"
        fmt = qc.get("fmt") or qc.get("weight_dtype") or ""
        return f"{qm}{('/' + fmt) if fmt else ''}"
    return str(cfg.get("torch_dtype", "?"))


_PREC_KEYWORDS = re.compile(r"(FP4\s*\+\s*FP8|NVFP4|MXFP4|FP8\s*Mixed|FP8|FP4|BF16|INT4|INT8|AWQ|GPTQ|W4A16|W8A8)", re.I)


def card_precision(card: str, model_name: str) -> dict:
    """README 마크다운 테이블에서 model_name 행의 정밀도 셀 추출, 실패 시 키워드 스캔."""
    out = {"table_row": None, "keywords": []}
    base = model_name.split("/")[-1]
    nbase = re.sub(r"\W", "", base).lower()
    candidates = []   # (exactness, prec) — name-cell 이 base 와 정확 일치(예 'Flash' vs 'Flash-Base')하면 우선
    for line in card.splitlines():
        if "|" not in line or base.lower() not in line.lower():
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        precs = [c for c in cells if _PREC_KEYWORDS.search(c)]
        if not precs:
            continue
        exact = any(re.sub(r"\W", "", c).lower() == nbase for c in cells)
        candidates.append((0 if exact else 1, precs[0]))
    if candidates:
        candidates.sort(key=lambda x: x[0])
        out["table_row"] = candidates[0][1]
    out["keywords"] = sorted({m.group(0) for m in _PREC_KEYWORDS.finditer(card)})
    # FP4+FP8 mixed 주석(experts=FP4) 같은 정의문 캡처
    mdef = re.search(r"FP4\s*\+\s*FP8[^\n]*?(expert[^\n]*?FP4[^\n]*)", card, re.I)
    if mdef:
        out["mixed_note"] = mdef.group(0)[:200]
    return out


def read_inference_deps(model_dir: str) -> list:
    deps = []
    for rel in ("inference/requirements.txt", "requirements.txt"):
        p = os.path.join(model_dir, rel)
        if os.path.isfile(p):
            with open(p, encoding="utf-8", errors="replace") as f:
                for ln in f:
                    ln = ln.strip()
                    if ln and not ln.startswith("#"):
                        deps.append({"src": rel, "spec": ln,
                                     "pkg": re.split(r"[<>=!~\[ ]", ln, 1)[0].lower()})
    return deps


def crosscheck(model_dir: str) -> dict:
    if not os.path.isdir(model_dir):
        raise FileNotFoundError(f"모델 디렉토리 부재(다운로드 금지): {model_dir}")
    cfg_path = os.path.join(model_dir, "config.json")
    if not os.path.isfile(cfg_path):
        raise FileNotFoundError(f"config.json 부재(다운로드 금지): {cfg_path}")
    cfg = json.load(open(cfg_path, encoding="utf-8"))
    card_path = next((os.path.join(model_dir, n) for n in ("README.md", "readme.md", "MODEL_CARD.md")
                      if os.path.isfile(os.path.join(model_dir, n))), None)
    card = open(card_path, encoding="utf-8", errors="replace").read() if card_path else ""
    dt = scan_dtypes(model_dir)
    deps = read_inference_deps(model_dir)
    name = os.path.basename(model_dir.rstrip("/"))

    checks, recs = [], []

    # ── 1. 정밀도 교차검증 (coarse-label 함정) ──
    cprec = config_precision(cfg)
    cardp = card_precision(card, name) if card else {"table_row": None, "keywords": []}
    sig = dt.get("precision_classes", []) if dt.get("present") else []
    verdict, note = "info", ""
    card_says_mixed = bool(cardp.get("mixed_note")) or (cardp.get("table_row") and "+" in (cardp.get("table_row") or ""))
    if dt.get("present"):
        coarse_single = ("/" not in cprec and cprec.lower() in ("fp8", "fp16", "bf16")) or \
                        (isinstance(cfg.get("quantization_config"), dict) and "+" not in cprec)
        if dt.get("is_mixed_precision") and coarse_single:
            verdict = "MISMATCH"
            note = (f"config quant='{cprec}'(coarse) 가 실제 혼합정밀도를 과소표기. "
                    f"실측 dtype classes={sig}"
                    + (f"; experts=4bit" if dt.get("fourbit_experts") else "")
                    + (f"; 카드='{cardp.get('table_row') or cardp.get('mixed_note')}'" if (cardp.get('table_row') or cardp.get('mixed_note')) else ""))
            recs.append("정밀도 = 실측 dtype·카드 기준으로 확정(config coarse 라벨 신뢰 금지). 메모리/적합성 재산정.")
        elif card_says_mixed and not dt.get("is_mixed_precision"):
            verdict = "MISMATCH"
            note = f"카드는 혼합정밀도('{cardp.get('table_row')}') 인데 실측 dtype 단일({sig}) — 다운로드본 변형 의심."
        else:
            note = f"config='{cprec}' · 실측={sig} · 카드='{cardp.get('table_row') or cardp.get('keywords')}'"
    checks.append({"name": "precision", "config": cprec, "dtype_evidence": sig,
                   "card": cardp.get("table_row") or cardp.get("keywords"), "verdict": verdict, "note": note})

    # ── 2. special 커널/엔진 의존 (novel-arch serving 사전경보) ──
    flagged = []
    for d in deps:
        if d["pkg"] in KNOWN_SPECIAL_DEPS:
            flagged.append({"pkg": d["pkg"], "spec": d["spec"], "src": d["src"], "why": KNOWN_SPECIAL_DEPS[d["pkg"]]})
    # README 본문 special-dep 언급도 스캔
    for kw, why in KNOWN_SPECIAL_DEPS.items():
        if card and re.search(r"\b" + re.escape(kw) + r"\b", card, re.I) and not any(f["pkg"] == kw for f in flagged):
            flagged.append({"pkg": kw, "spec": "(README 언급)", "src": "README.md", "why": why})
    if flagged:
        recs.append("novel-arch special-dep 발견 → serving 전 **vLLM dry-init(모델 클래스 import) + 해당 op 가용성** 확인"
                    "(예: DSA SparseAttnIndexer→DeepGEMM). 분류(3+1+1 따름정리): **native lib/커널 = 빌드-바깥** → "
                    "upstream-version-watch `build_patches/`(patch.py ✗ — 몽키패치로 native 설치 불가) · "
                    "**Python 코드불일치 = `<model>_patch.py`**. 빌드-바깥은 recipe-explorer 발견·핸드오프만.")
    checks.append({"name": "special_deps", "verdict": "WARN" if flagged else "info", "flagged": flagged})

    # ── 3. 컨텍스트 / 4. 아키 / 5. reasoning (정보 합치) ──
    mpe = cfg.get("max_position_embeddings")
    card_ctx = "1M" if re.search(r"\b1\s*M(illion)?\b|1,?048,?576|one million", card, re.I) else None
    checks.append({"name": "context", "config_max_position_embeddings": mpe, "card": card_ctx, "verdict": "info"})
    checks.append({"name": "arch", "config": cfg.get("architectures"), "model_type": cfg.get("model_type"),
                   "card_keywords": sorted({k for k in ("MoE", "Mixture-of-Experts", "Sparse Attention",
                                                        "Multi-head Latent", "MTP", "Multi-Token") if card and k.lower() in card.lower()}),
                   "verdict": "info"})
    is_reasoning = bool(card and re.search(r"reasoning effort|thinking budget|<think>|reasoning mode", card, re.I))
    if is_reasoning:
        recs.append("reasoning 모델 — 스모크 max_tokens 충분히(finish_reason=stop) · reasoning-parser 검토.")
    checks.append({"name": "reasoning", "verdict": "info", "is_reasoning": is_reasoning})

    mismatches = [c for c in checks if c.get("verdict") == "MISMATCH"]
    return {
        "model": name,
        "model_dir": model_dir,
        "sources_read": {"config.json": True, "README.md": bool(card_path),
                         "inference_requirements": any(d["src"].startswith("inference") for d in deps),
                         "safetensors_headers": dt.get("present", False)},
        "dtype_scan": dt,
        "checks": checks,
        "recommendations": recs,
        "verdict": "MISMATCH" if mismatches else ("WARN" if any(c.get("verdict") == "WARN" for c in checks) else "OK"),
        "n_mismatch": len(mismatches),
    }


def _human(r: dict) -> str:
    L = [f"# 모델카드 교차검증: {r['model']}  → {r['verdict']}",
         f"  sources: {', '.join(k for k, v in r['sources_read'].items() if v)}"]
    for c in r["checks"]:
        mark = {"OK": "✅", "MISMATCH": "🔴", "WARN": "⚠️", "info": "·"}.get(c.get("verdict"), "·")
        if c["name"] == "precision":
            L.append(f"  {mark} precision: config={c['config']} | 실측={c['dtype_evidence']} | 카드={c['card']}")
            if c.get("note"):
                L.append(f"       {c['note']}")
        elif c["name"] == "special_deps":
            if c["flagged"]:
                for f in c["flagged"]:
                    L.append(f"  ⚠️ special-dep: {f['pkg']} ({f['spec']}, {f['src']}) — {f['why']}")
            else:
                L.append("  · special-deps: 없음")
        elif c["name"] == "context":
            L.append(f"  · context: config={c['config_max_position_embeddings']} | 카드={c['card']}")
        elif c["name"] == "arch":
            L.append(f"  · arch: {c['config']} / {c['model_type']} | 카드={c['card_keywords']}")
        elif c["name"] == "reasoning":
            L.append(f"  · reasoning: {c['is_reasoning']}")
    if r["recommendations"]:
        L.append("  권고:")
        L += [f"   → {x}" for x in r["recommendations"]]
    return "\n".join(L)


def main():
    ap = argparse.ArgumentParser(description="HF 원본 모델카드(번들 README) 교차검증 — config 단독파싱 함정 차단(폐쇄망·결정론).")
    ap.add_argument("path", help="모델 디렉토리(호스트 경로 또는 /app/models/<Org>/<Name>)")
    ap.add_argument("--json", action="store_true", help="JSON 출력")
    a = ap.parse_args()
    try:
        r = crosscheck(a.path)
    except FileNotFoundError as e:
        print(f"[crosscheck] STOP: {e}", file=sys.stderr)
        sys.exit(3)
    print(json.dumps(r, ensure_ascii=False, indent=2) if a.json else _human(r))
    sys.exit(2 if r["verdict"] == "MISMATCH" else 0)


if __name__ == "__main__":
    main()
