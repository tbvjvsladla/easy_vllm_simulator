#!/usr/bin/env python3
"""crosscheck_model_card.py — HF 원본 모델카드 교차검증 (vllm-recipe-explorer).

폐쇄망 전제. 모델 디렉토리의 **번들 README.md(=HF 원본 모델카드)** + `inference/requirements.txt`
+ `config.json` + safetensors 헤더(dtype 실측)를 교차대조해, config.json **단독** 파싱이 놓치는
사실을 serving 착수 *전*에 노출한다:
  · coarse quant 라벨 함정(config `quant_method:fp8` 인데 실제 experts=FP4 혼합)
  · special 커널/엔진 의존(DeepGEMM·tilelang·flash_attn …) — novel-arch 구동 사전경보
  · 정밀도·파라미터·컨텍스트·아키 카드값 ↔ config/실측 dtype 합치

동기(실증, plan_26062811_30_33 item③): config.json `quant_method:fp8`(coarse) + `du` 아티팩트만 봐
DeepSeek-V4-Flash 를 순수 FP8/298GB/인피저블로 **오판** → 모델카드 README 가 `FP4+FP8 Mixed(experts FP4)`
명시했고 `inference/requirements.txt` 가 `tilelang` 명시. 카드 우선 참조 시 오판·DeepGEMM-class 함정 조기 포착.

결정론(stdlib only). 로컬 카드/config/safetensors 교차검증은 **네트워크 호출 없음**(번들 README = HF 원본
카드 — 모델과 함께 받음). 헌법 §금지 "참조-그라운디드".
**MISMATCH 1건 이상이면 비0 종료(게이트)** — parse 직후 필수 루틴(SKILL.md §2 ①).

**외부 VRAM 교차검증 (plan_26070814)**: `fetch_hf_safetensors_total`/`crosscheck_external_vram` 은
예외적으로 네트워크 호출(HF 공개 API `GET /api/models/<repo_id>` — 모델 자체는 받지 않고 `safetensors.total`
파라미터 총계만 조회)을 한다. 동기: `du -sh` 류 디렉토리 전체크기가 `.git`(HF LFS 캐시) 오염으로 실제
가중치보다 2배 가까이 부풀 수 있음이 실증됨(2026-07-08, gemma-4-E2B-it 20GB→실 9.54GiB) — 로컬 실측
(managed)이든 다운로드 전 사전추정(ephemeral)이든 외부 권위 소스로 **이중검증**한다(헌법 "서빙전략 수립의
외부 교차검증은 획득모드와 무관하게 항상 허용·의무"). 조회 실패는 음성정직(대체값 날조 금지) — verdict
"UNAVAILABLE"로 기록하고 로컬 실측만으로 진행(managed) 또는 사용자에게 명시 보고(ephemeral).
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import re
import struct
import sys
import urllib.error
import urllib.request
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from quant_table import dtype_bpw  # noqa: E402

HF_API_BASE = "https://huggingface.co/api/models"

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


def _canonical_shards(model_dir: str) -> list:
    """로더가 실제로 읽을 샤드 집합 = index weight_map 참조분. 없으면 glob 폴백.

    ★ 글롭만 쓰면 **한 디렉토리에 공존하는 다른 정밀도 세트**를 함께 센다
      (LFM2-8B-A1B: bf16 -of-00004 와 F32 -of-00007 동거). 그러면 순수 bf16 모델이
      '혼합 정밀도'로 오판되어 양자화 판단이 뒤틀린다.
      `_count_params_from_headers`(parse_model_config)가 같은 이유로 이미 weight_map 을 쓴다 —
      여기만 빠져 있었다(2026-08-01 계열 전수조사에서 발견).
    """
    index_path = os.path.join(model_dir, "model.safetensors.index.json")
    if os.path.isfile(index_path):
        try:
            with open(index_path, "r", encoding="utf-8") as f:
                wm = json.load(f).get("weight_map")
            if isinstance(wm, dict) and wm:
                ref = sorted({os.path.join(model_dir, v) for v in wm.values()})
                if all(os.path.isfile(p) for p in ref):
                    return ref
        except (OSError, ValueError, json.JSONDecodeError):
            pass
    return sorted(glob.glob(os.path.join(model_dir, "*.safetensors")))


def scan_dtypes(model_dir: str) -> dict:
    """**정본 샤드**(index weight_map) 헤더만 읽어 dtype별 텐서수·byte + expert/비-expert 분리 + 정밀도 시그니처."""
    shards = _canonical_shards(model_dir)
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


def fetch_hf_safetensors_total(repo_id: str, timeout: float = 8.0) -> dict:
    """HF 공개 API 로 안전텐서 파라미터 총계를 조회(모델 자체는 받지 않음 — plan_26070814).

    반환: {"ok": True, "total": int, "parameters": {dtype: count}} 또는
          {"ok": False, "reason": str}(조회 실패 — 음성정직, 대체값 날조 금지).
    """
    if not repo_id:
        return {"ok": False, "reason": "repo_id 미지정"}
    url = f"{HF_API_BASE}/{repo_id}"
    try:
        req = urllib.request.Request(url, headers={"Accept": "application/json"})
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            data = json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        return {"ok": False, "reason": f"HTTP {e.code}({url})"}
    except (urllib.error.URLError, TimeoutError, OSError) as e:
        return {"ok": False, "reason": f"네트워크 오류: {e}({url})"}
    except (json.JSONDecodeError, ValueError) as e:
        return {"ok": False, "reason": f"응답 파싱 실패: {e}"}
    st = data.get("safetensors")
    if not isinstance(st, dict) or "total" not in st:
        return {"ok": False, "reason": "응답에 safetensors.total 필드 없음(비공개/게이트/구형 포맷 가능)"}
    return {"ok": True, "total": int(st["total"]), "parameters": st.get("parameters") or {}}


def crosscheck_external_vram(local_num_params: "int | None", local_native_weight_bytes: "int | None",
                              repo_id: "str | None", tolerance: float = 0.05) -> dict:
    """로컬 실측(managed) 또는 사전추정(ephemeral) vs 외부(HF API) 파라미터 총계 대조.

    tolerance(기본 5%) 초과 시 verdict=MISMATCH. 외부 조회 자체가 실패하면 verdict=UNAVAILABLE
    (음성정직 — 로컬 실측만으로 진행 가능이지 MISMATCH 아님).
    """
    ext = fetch_hf_safetensors_total(repo_id) if repo_id else {"ok": False, "reason": "repo_id 미지정"}
    if not ext.get("ok"):
        return {"verdict": "UNAVAILABLE", "reason": ext.get("reason"), "repo_id": repo_id,
                "local_num_params": local_num_params, "external_total": None}
    ext_total = ext["total"]
    if local_num_params is None:
        # ephemeral(로컬 실측 없음): 외부 총계를 사전추정치로 그대로 채택 — 대조 대상 없음(OK로 보고).
        est_bytes = None
        if ext.get("parameters"):
            est_bytes = sum(int(cnt) * dtype_bpw(dt)[0] for dt, cnt in ext["parameters"].items())
        return {"verdict": "OK", "mode": "ephemeral-estimate", "repo_id": repo_id,
                "external_total": ext_total, "external_native_weight_bytes_est": est_bytes,
                "external_native_weight_gib_est": round(est_bytes / 2**30, 2) if est_bytes else None}
    diff = abs(local_num_params - ext_total)
    rel = diff / ext_total if ext_total else 1.0
    verdict = "MISMATCH" if rel > tolerance else "OK"
    return {"verdict": verdict, "mode": "managed-doublecheck", "repo_id": repo_id,
            "local_num_params": local_num_params, "external_total": ext_total,
            "relative_diff": round(rel, 4),
            "note": (f"로컬 실측({local_num_params:,}) vs 외부({ext_total:,}) 오차 {rel:.1%} "
                     f"{'허용범위 초과 — 로컬 파일 재확인 필요' if verdict == 'MISMATCH' else '(허용범위 내)'}")}


def crosscheck(model_dir: str, hf_repo_id: "str | None" = None,
               local_num_params: "int | None" = None) -> dict:
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

    # ── 6. 외부(HF API) VRAM 이중검증 (managed — plan_26070814) ──
    # du -sh 류 디렉토리 전체크기의 .git 오염 실증(2026-07-08) 대응. hf_repo_id 미지정 시 스킵(info).
    if hf_repo_id:
        extv = crosscheck_external_vram(local_num_params, None, hf_repo_id)
        ev_verdict = extv["verdict"] if extv["verdict"] in ("MISMATCH",) else ("info" if extv["verdict"] == "UNAVAILABLE" else "OK")
        checks.append({"name": "external_vram", "verdict": ev_verdict, **extv})
        if extv["verdict"] == "MISMATCH":
            recs.append(f"외부(HF API) 파라미터 총계와 로컬 실측 불일치({extv.get('note')}) — "
                        f"로컬 safetensors 파일이 손상/변형됐을 가능성. 재검증 필요.")
    else:
        checks.append({"name": "external_vram", "verdict": "info", "reason": "hf_repo_id 미지정 — 외부 이중검증 스킵"})

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
        elif c["name"] == "external_vram":
            if c["verdict"] == "info" and c.get("reason"):
                L.append(f"  · external_vram: {c['reason']}")
            elif c["verdict"] == "info":
                L.append(f"  · external_vram: 조회불가({c.get('reason')}) — 로컬 실측만으로 진행")
            else:
                L.append(f"  {mark} external_vram: {c.get('note', c)}")
    if r["recommendations"]:
        L.append("  권고:")
        L += [f"   → {x}" for x in r["recommendations"]]
    return "\n".join(L)


def main():
    ap = argparse.ArgumentParser(description="HF 원본 모델카드(번들 README) 교차검증 — config 단독파싱 함정 차단(결정론).")
    ap.add_argument("path", nargs="?", default=None,
                    help="모델 디렉토리(호스트 경로 또는 /app/models/<Org>/<Name>) — managed 모드.")
    ap.add_argument("--hf-repo-id", default=None,
                    help="HF repo id(예 google/gemma-3-1b-it) — managed: 로컬 실측 이중검증. "
                         "--ephemeral-estimate 와 함께 쓰면 그게 곧 repo_id.")
    ap.add_argument("--ephemeral-estimate", action="store_true",
                    help="로컬 디렉토리 없이(다운로드 전) --hf-repo-id 만으로 외부 파라미터 총계 사전추정.")
    ap.add_argument("--json", action="store_true", help="JSON 출력")
    a = ap.parse_args()

    if a.ephemeral_estimate:
        if not a.hf_repo_id:
            print("[crosscheck] STOP: --ephemeral-estimate 는 --hf-repo-id 필수", file=sys.stderr)
            sys.exit(3)
        r = crosscheck_external_vram(None, None, a.hf_repo_id)
        if a.json:
            print(json.dumps(r, ensure_ascii=False, indent=2))
        else:
            print(f"# ephemeral 사전추정: {a.hf_repo_id} → {r['verdict']}")
            if r["verdict"] == "OK":
                print(f"  파라미터 총계: {r['external_total']:,} · 예상 가중치: "
                      f"{r.get('external_native_weight_gib_est')} GiB(dtype 혼합 시 근사)")
            else:
                print(f"  조회 실패: {r.get('reason')}")
        sys.exit(0 if r["verdict"] != "UNAVAILABLE" else 4)

    if not a.path:
        print("[crosscheck] STOP: managed 모드는 모델 디렉토리 경로 필수(또는 --ephemeral-estimate)", file=sys.stderr)
        sys.exit(3)

    local_num_params = None
    if a.hf_repo_id:
        try:
            sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
            from parse_model_config import parse as _parse_model_config
            local_num_params = _parse_model_config(a.path).get("num_params")
        except Exception as e:  # noqa: BLE001 — 이중검증은 best-effort, parse 실패해도 나머지 체크는 진행
            print(f"[crosscheck] WARN: num_params 재측정 실패({e}) — external_vram 체크 스킵", file=sys.stderr)

    try:
        r = crosscheck(a.path, hf_repo_id=a.hf_repo_id, local_num_params=local_num_params)
    except FileNotFoundError as e:
        print(f"[crosscheck] STOP: {e}", file=sys.stderr)
        sys.exit(3)
    print(json.dumps(r, ensure_ascii=False, indent=2) if a.json else _human(r))
    sys.exit(2 if r["verdict"] == "MISMATCH" else 0)


if __name__ == "__main__":
    main()
