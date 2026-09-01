#!/usr/bin/env python3
"""hint_collect.py — 3소스 수집 + 3항목 저작 스캐폴드 (plan_26090107 Phase 3 · §4·§5·§6).

경계가 이 스크립트의 전부다 (plan §6 D2):
    **기계는 "무엇이 일어났는가"를 말하고, 에이전트는 "그것이 무엇을 뜻하는가"를 말한다.**
    기계 산출을 에이전트가 덮어쓸 수 없고, **에이전트 산출을 기계가 지어낼 수 없다.**

따라서 이 스크립트는 **산문을 한 줄도 합성하지 않는다.** 판단이 필요한 자리에는 값을 넣는
대신 `<<AGENT: …>>` 마커를 남기고, 마커가 남아 있으면 `hint_branch publish` 로 못 간다
(§check 서브커맨드가 fail-closed 로 잡는다). 이것이 헌법 불변식 B 의 분담이다 —
*"누락은 기계가 fail-closed 로 잡고 거짓은 사람이 리뷰한다."*

수집 소스 (plan §4) — 세 통로의 **제약이 서로 다르다**:
    single-node / multi-node 브랜치 → 워킹트리 직접 읽기 (서빙 직후에만 트리플렛이 존재한다)
    서브노드                        → `fetch_sub_docs.sh` **문서기반** 회수만.
                                      코드·설정 직접 회수 금지 · 재스캔 금지(헌법 · docs.md).
    ⚠ 토폴로지 축(헌법 불변식 A): 멀티의 sub(Ray 워커)와 싱글의 sub(A2A 원격 에이전트)는
      다른 것이다. 어느 sub 인지는 manifest 에서 읽는다 — **브랜치로 추론하지 않는다.**

슬롯 판정 (헌법 3+1+1 · plan §7)
    경로 **규약에서 파생**한다(하드코딩이 아니다). Phase 3 은 신호 ①파일 존재만 결정론으로
    확정하고, ②적용 증거·③Agent 선언은 Phase 4 가 3신호 대사로 닫는다. 그래서 지금 나오는
    판정에는 `slot_confidence` 가 붙는다 — 판정 강도를 표시하지 않고 섞으면 하류가 무엇을
    근거로 삼았는지 알 수 없다(헌법 §결정론 규율).
    ★ 트리플렛 3 은 **선언으로 면제 불가**: 서빙에 원리적으로 필수이므로 부재는 항상 누락이다.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sys
from pathlib import Path

AGENT_MARK = "<<AGENT:"
PAYLOAD_JSON = "PAYLOAD.json"
FILES_LIST = "files.txt"

# 인증서에서 그대로 옮길 필드. **파생 가능한 값을 손으로 적지 않는다** — 여기 있는 것은
# "인증서의 어느 키를 페이로드로 옮기는가"라는 선택이지 값의 복제가 아니다(tripwire 칸).
CERT_STRONG = ["model", "gpu_model", "vllm_version", "quantization", "topology",
               "tensor_parallel_size"]
CERT_SOFT = ["driver_version", "cuda_version", "image_tag", "max_model_len", "max_num_seqs",
             "kv_cache_memory_bytes", "kv_cache_dtype", "gpu_memory_utilization",
             "moe_backend", "enforce_eager", "ngc_base_tag"]
CERT_PERF = ["benchmark_mode", "verdict", "decode_tps_conc1", "rubric_authority",
             "primary_source", "primary_tps", "floor_tps", "tolerance",
             "ratio_M_over_primary", "spec_on", "accept_len", "sweep_levels",
             "sweep_truncated", "lite_included", "lite_gen_tps_warm", "lite_gen_src",
             "lite_cold_ttft_ms", "lite_kv_gib", "measured_utc"]


def die(msg: str, code: int = 2) -> None:
    print(f"[hint_collect] {msg}", file=sys.stderr)
    raise SystemExit(code)


def sha256_of(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


# ---------------------------------------------------------------- 인증서 파싱

_SCALAR = re.compile(r'^([A-Za-z_][A-Za-z0-9_]*):\s*(.*?)\s*$')


def parse_certificate(path: Path) -> dict:
    """평면 YAML 인증서를 **파싱만** 한다. 값을 만들지 않는다(합성 금지 · 측정 > 공식).

    yaml 모듈에 의존하지 않는 이유: 인증서는 결정론 발행기가 쓴 평면 스칼라뿐이고,
    배포 클론에 PyYAML 이 없을 수 있다. 중첩이 나오면 그건 계약 위반이므로 **터진다**.
    """
    doc: dict[str, str] = {}
    for lineno, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.split("#", 1)[0].rstrip() if not raw.lstrip().startswith("#") else ""
        if not line.strip():
            continue
        if line.startswith((" ", "\t", "-")):
            die(f"{path.name}:{lineno} 인증서에 중첩/리스트가 있다 — 평면 스칼라 계약 위반: {raw!r}")
        m = _SCALAR.match(line)
        if not m:
            die(f"{path.name}:{lineno} 파싱 불가: {raw!r}")
        val = m.group(2)
        if len(val) >= 2 and val[0] == val[-1] and val[0] in "\"'":
            val = val[1:-1]
        doc[m.group(1)] = val
    if not doc:
        die(f"{path} 가 비었다 — 빈 인증서로 페이로드를 짓지 않는다")
    return doc


# ---------------------------------------------------------------- 슬롯 발견

def discover_slots(repo: Path, topo: str, cfg: str) -> dict:
    """3+1+1 슬롯을 **경로 규약에서 파생**한다.

    각 슬롯의 판정은 신호 ①파일 존재만 결정론이다. ②적용증거·③선언은 Phase 4.
    """
    out = repo / "output" / topo
    triplet = {
        "config_yaml": out / "configs" / f"{cfg}.yaml",
        "runner_sh": out / "configs" / f"{cfg}.sh",
        "env_file": out / "envs" / f".env.{cfg}",
    }
    runtime_patch = out / "configs" / f"{cfg}_patch.py"
    pre_dir = out / "build_patches_src"
    post_dir = out / "build_patches"

    def listing(d: Path) -> tuple[list[Path], list[str]]:
        """빌드 패치를 **명명 규약에서 파생**해 고른다: `<NN>-*.sh` (workflow.md 3+1+1 슬롯 표).

        2026-09-01 실측 결함: 이전 구현은 `glob("*")` 로 전부 담아 디렉터리 스켈레톤 마커
        `.gitkeep` 을 **빌드 패치로 셌다**. 슬롯이 실제로는 '없음'인데 '있음 · 3-signal' 로
        답했고, 그 마커가 패치인 양 배포될 뻔했다. 픽스처에는 `.gitkeep` 이 없어 자체검사가
        못 봤고 **실물에서만 드러났다**.

        규약에 안 맞는 항목은 **조용히 버리지 않는다** — 이름을 틀린 진짜 패치가 침묵 누락되면
        "패치가 없었던 것"과 구분되지 않는다(docs.md 부재≠결측).
        """
        if not d.is_dir():
            return [], []
        found, odd = [], []
        for p in sorted(d.glob("*")):
            if not p.is_file():
                continue
            if p.name == ".gitkeep":
                continue  # 디렉터리 스켈레톤 마커 — 패치가 아니다(알려진 비-패치)
            if re.fullmatch(r"\d+-.*\.sh", p.name):
                found.append(p)
            else:
                odd.append(p.name)
        return found, odd

    slots: dict[str, dict] = {}
    missing_triplet = [k for k, p in triplet.items() if not p.is_file()]
    slots["triplet"] = {
        "phase": "serve",
        "owner": "vllm-recipe-explorer",
        "files": {k: str(p.relative_to(repo)) for k, p in triplet.items() if p.is_file()},
        "present": not missing_triplet,
        "missing": missing_triplet,
        "exemptible": False,  # ★ 선언으로 면제 불가 — 서빙에 원리적으로 필수
        "slot_confidence": "1-signal(file-presence)",
    }
    slots["runtime_patch"] = {
        "phase": "serve(arming)",
        "owner": "vllm-recipe-explorer",
        "files": {"patch_py": str(runtime_patch.relative_to(repo))} if runtime_patch.is_file() else {},
        "present": runtime_patch.is_file(),
        "exemptible": True,
        # plan Q1 귀결: arming 로그가 미배선이라 당분간 2신호(파일 + Agent 선언)로만 판정한다
        "slot_confidence": "2-signal(file+declaration)",
        "note_for_agent": "arming 이 실제로 '먹었는지'는 로그로 실증해야 한다(workflow.md 위상 오배정 실증)",
    }
    for key, d, phase in (("build_patch_pre", pre_dir, "컴파일 전"),
                          ("build_patch_post", post_dir, "컴파일 후")):
        found, odd = listing(d)
        slots[key] = {
            "phase": phase,
            "owner": "upstream-version-watch",
            "files": [str(p.relative_to(repo)) for p in found],
            "present": bool(found),
            "exemptible": True,
            "slot_confidence": "3-signal(file+provenance+declaration)" if found
                               else "1-signal(absence)",
        }
        if odd:
            # 규약 밖 항목은 소리내어 남긴다 — 하류가 이것을 보고 사람에게 묻게 한다
            slots[key]["nonconforming"] = odd
            print(f"[hint_collect] 경고: {d} 에 명명규약(<NN>-*.sh) 밖 항목 {odd} — "
                  "패치라면 이름을 고치고, 아니라면 그 디렉터리에 두지 마라", file=sys.stderr)
    # 바깥 포크핀 — .env 의 VARIANT 한 줄이 정본(workflow.md §변종 좌표의 거처).
    variant_id = None
    env_p = triplet["env_file"]
    if env_p.is_file():
        for line in env_p.read_text(encoding="utf-8").splitlines():
            if line.strip().startswith("VARIANT="):
                variant_id = line.split("=", 1)[1].strip()
    slots["fork_pin"] = {
        "phase": "build",
        "owner": "upstream-version-watch",
        "variant_id": variant_id,
        # 줄이 **없으면 stock** 이다 — 부재가 기본값이다(workflow.md)
        "present": variant_id is not None,
        "exemptible": True,
        "ledger": ".claude/policies/arch_variant_ledger.json" if variant_id else None,
        "slot_confidence": "3-signal(ledger+file+declaration)" if variant_id else "1-signal(absence)",
    }
    return slots


# ---------------------------------------------------------------- 신호② 적용 증거

DECLARATION_NAME = "slots.declaration.json"

# 슬롯별 **관측 가능한** 적용 증거의 공급원. plan Q1: "관측되는 신호만 사용".
# 값이 None 인 슬롯은 공급원이 **아직 없다** — 있는 척하지 않고 2신호로 강등한다.
EVIDENCE_SOURCE = {
    "triplet": "certificate.serving_config",
    "runtime_patch": None,      # arming 로그 미배선 (plan Q1 귀결)
    "build_patch_pre": "build_patches_src/PROVENANCE.json",
    "build_patch_post": None,   # 컴파일 후 적용을 사후 관측할 공급원이 없다
    "fork_pin": "arch_variant_ledger.source_build_variants",
}


def evidence_signal(repo: Path, topo: str, slot: str, slots: dict, cert: dict) -> bool | None:
    """신호② — **관측**한다. 관측원이 없으면 `None`(모름)이지 `False`(없음)가 아니다.

    '없다'와 '모른다'를 같은 값으로 뭉개면 거짓 음성이 난다 — 감사 ⑦ 이 실증한 기전이다
    (`-` 하나가 "SD 없음"과 "색인이 낡음"을 구분하지 못했다).
    """
    src = EVIDENCE_SOURCE.get(slot)
    if src is None:
        return None
    if slot == "triplet":
        cfgname = cert.get("serving_config")
        if not cfgname:
            return None
        # 인증서의 운영 조합명이 트리플렛 파일명에 담겨 있으면 그 트리플렛이 서빙에 쓰였다는
        # 관측이다. 완전 일치가 아니라 포함 관계인 이유: serving_config 는 모델 축이고
        # 트리플렛 basename 은 `<model>-<hw>` 조합이다(실측: 'gpt-oss-120b' ⊂ 'gpt-oss-120b-gb10').
        any_file = next(iter(slots["triplet"]["files"].values()), "")
        return bool(cfgname) and cfgname in Path(any_file).name
    if slot == "build_patch_pre":
        prov = repo / "output" / topo / "build_patches_src" / "PROVENANCE.json"
        if not prov.is_file():
            return False
        try:
            doc = json.loads(prov.read_text(encoding="utf-8"))
        except Exception:
            return False
        return bool(doc.get("counts", {}).get("files"))
    if slot == "fork_pin":
        vid = slots["fork_pin"].get("variant_id")
        if not vid:
            return False
        led = repo / ".claude" / "policies" / "arch_variant_ledger.json"
        if not led.is_file():
            return None
        try:
            doc = json.loads(led.read_text(encoding="utf-8"))
        except Exception:
            return None
        variants = doc.get("source_build_variants") or {}
        return vid in {k for k in variants if not k.startswith("_")}
    return None


def dialogue(present: bool, evidence: bool | None, declared: bool | None,
             exemptible: bool) -> tuple[str, str]:
    """plan §7 대사표. `(verdict, 사유)` — verdict ∈ {ok, blocked, undeclared}.

    | 파일 | 적용증거 | 선언 | 판정 |
    |---|---|---|---|
    | 부재 | 부재 | 불해당 | not-applicable 통과 |
    | 부재 | **있음** | — | **차단**(진짜 누락) |
    | 있음 | 부재 | — | **차단**(먹지 않은 패치를 재현지침으로 배포) |

    ★ `exemptible=False`(트리플렛)는 **선언으로 면제 불가**다. 서빙에 원리적으로 필수이므로
      부재는 언제나 누락이고, Agent 가 "불해당"이라 해도 통과시키지 않는다. 이 구분이 없으면
      선언 하나가 안전 게이트를 무력화한다.
    """
    if not present and not exemptible:
        return "blocked", "면제 불가 슬롯이 부재하다 — 선언과 무관하게 누락이다"
    if declared is None:
        return "undeclared", "Agent 선언이 비어 있다(부재는 미판정이지 통과가 아니다)"
    if not present and evidence is True:
        return "blocked", "파일은 없는데 **적용 증거가 있다** — 진짜 누락이다"
    if present and evidence is False:
        return "blocked", "파일은 있는데 **적용 증거가 없다** — 먹지 않은 패치를 재현지침으로 배포하게 된다"
    if present and not declared:
        return "blocked", "파일이 있는데 Agent 가 '불해당'이라 선언했다 — 모순이다"
    if not present and declared:
        return "blocked", "파일이 없는데 Agent 가 '해당'이라 선언했다 — 모순이다"
    return "ok", ("해당(파일·증거·선언 일치)" if present else "불해당(세 신호 모두 부재/불해당)")


def confidence_of(evidence: bool | None) -> str:
    """판정 강도를 **표시**한다. 강도가 다른 것을 섞으면 하류가 근거를 알 수 없다(헌법 §결정론 규율)."""
    return "2-signal(file+declaration)" if evidence is None else "3-signal(file+evidence+declaration)"


# ---------------------------------------------------------------- 저작 스캐폴드

def _fmt_kv(doc: dict, keys: list[str]) -> str:
    rows = [f"| `{k}` | {doc[k]} |" for k in keys if k in doc]
    return "\n".join(rows) if rows else "| — | 인증서에 해당 필드 없음 |"


def render_item1(slots: dict) -> str:
    lines = ["# 1. 산출물 — 무엇이 실제로 쓰였나", "",
             "> 슬롯 분류는 **경로 규약에서 파생**한 결정론 산출이다(헌법 3+1+1).",
             "> **판정 강도가 슬롯마다 다르다** — `slot_confidence` 를 함께 읽어라.",
             "> 판정 기준은 *\"무엇을 고치나\"가 아니라 \"언제 성립해야 하나\"* 다.", "",
             "| 슬롯 | 성립 시점 | owner | 존재 | 판정 강도 |", "|---|---|---|---|---|"]
    for name, s in slots.items():
        lines.append(f"| `{name}` | {s['phase']} | {s['owner']} | "
                     f"{'있음' if s['present'] else '없음'} | {s['slot_confidence']} |")
    lines += ["", "## 파일", ""]
    for name, s in slots.items():
        files = s.get("files")
        if isinstance(files, dict) and files:
            lines += [f"**{name}**"] + [f"- `{v}`" for v in files.values()] + [""]
        elif isinstance(files, list) and files:
            lines += [f"**{name}**"] + [f"- `{v}`" for v in files] + [""]
    if slots["fork_pin"]["present"]:
        lines += [f"**fork_pin** — `VARIANT={slots['fork_pin']['variant_id']}` "
                  f"(좌표 정본 = `{slots['fork_pin']['ledger']}`)", ""]
    else:
        lines += ["**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.", ""]
    lines += ["## 적용 사유 (Agent)", "",
              f"{AGENT_MARK} 슬롯별로 **왜 그것이 필요했는지 / 왜 불해당인지**를 쓴다. "
              "파일 목록은 위에 이미 있다 — 여기 반복하지 말고 **이유**만 쓴다. "
              "불해당 슬롯도 이유를 남긴다(부재는 미판정이지 통과가 아니다). >>", ""]
    return "\n".join(lines)


def render_item2(devlog_path: str | None, testlog_path: str | None) -> str:
    return "\n".join([
        "# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나", "",
        "> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).",
        "> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어",
        "> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**", "",
        "## 읽을 원재료 (복사 대상 아님 · 포인터)", "",
        f"- devlog: `{devlog_path or '(work-manifest 에 없음)'}`",
        f"- testlog: `{testlog_path or '(work-manifest 에 없음)'}`", "",
        "## 서사", "",
        f"{AGENT_MARK} 증상 → 원인 → 해소 순으로 쓴다. **인용 게이트 필수**(불변식 B) — "
        "각 주장에 그 근거가 어느 문서 어느 절인지 붙인다. 인용 없는 결정은 거짓이 아니라 "
        "**누락**이며, 누락은 기계가 fail-closed 로 잡는다. >>", "",
        "## 되풀이하지 말 것", "",
        f"{AGENT_MARK} 다음 사람이 같은 벽에 부딪히지 않도록, 시도했다가 **버린 경로**와 "
        "그 이유를 쓴다. 성공 경로만 적으면 독자는 실패 경로를 다시 걷는다. >>", ""])


def render_item3(cert: dict, cert_name: str) -> str:
    return "\n".join([
        "# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나", "",
        "> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.",
        f"> 출처: `{cert_name}`", "",
        "## 성능", "", "| 항목 | 값 |", "|---|---|", _fmt_kv(cert, CERT_PERF), "",
        "## 강한 일치 키 — 하나라도 다르면 이 수치는 **무효**다", "",
        "| 키 | 값 |", "|---|---|", _fmt_kv(cert, CERT_STRONG), "",
        "## 소프트 지문 — 다르면 stale, 재측정 권고", "",
        "| 키 | 값 |", "|---|---|", _fmt_kv(cert, CERT_SOFT), "",
        "## like-with-like 한정자 (Agent)", "",
        f"{AGENT_MARK} 이 수치를 **무엇과 비교할 수 있고 무엇과는 비교할 수 없는지** 쓴다. "
        "입력 길이·동시성·데이터셋이 다르면 같은 모델·같은 하드웨어라도 몇 배씩 갈린다. "
        "특히 **다른 vLLM 버전의 hint 와 나란히 놓을 때** 측정 조건이 같은지 먼저 확인하고, "
        "다르면 '비교 불가'를 명시한다. >>", ""])


# ---------------------------------------------------------------- collect

def cmd_collect(a) -> int:
    repo = Path(a.repo).resolve()
    man_path = Path(a.manifest).resolve()
    man = json.loads(man_path.read_text(encoding="utf-8"))
    ident = man.get("identity") or {}
    topo = a.topology or ident.get("topology")
    if not topo:
        die("topology 를 정할 수 없다 — manifest.identity.topology 또는 --topology 가 필요하다. "
            "브랜치로 추론하지 않는다(헌법).")

    # 증거 포인터는 manifest 기준 상대경로다
    ev = man.get("evidence") or {}

    def ev_path(key: str) -> Path | None:
        node = ev.get(key)
        rel = node.get("path") if isinstance(node, dict) else None
        return (man_path.parent / rel).resolve() if rel else None

    cert_p = ev_path("certificate")
    if not cert_p or not cert_p.is_file():
        die(f"인증서를 찾을 수 없다: {cert_p} — 벤치 항목을 합성하지 않는다(fail-closed)")
    cert = parse_certificate(cert_p)

    slots = discover_slots(repo, topo, a.config_name)
    if not slots["triplet"]["present"]:
        die("★ 트리플렛이 불완전하다: " + ", ".join(slots["triplet"]["missing"]) +
            "\n  트리플렛은 **선언으로 면제 불가**다(plan §7) — 서빙에 원리적으로 필수이므로"
            "\n  부재는 언제나 누락이다. 서빙 직후 워킹트리에서 수집하라(plan §2.2).")

    # 신호② 를 **관측**하고, 그 결과로 판정 강도를 재산정한다.
    # discover_slots 가 붙여 둔 강도는 신호① 만 본 잠정값이다 — 관측원이 없는 슬롯은
    # 여기서 2-signal 로 강등된다(있는 척하지 않는다).
    for name, sl in slots.items():
        sig2 = evidence_signal(repo, topo, name, slots, cert)   # `ev` 는 manifest.evidence 다 — 가리지 않는다
        sl["evidence"] = sig2
        sl["evidence_source"] = EVIDENCE_SOURCE.get(name)
        sl["slot_confidence"] = confidence_of(sig2)

    out = Path(a.out).resolve()
    if out.exists() and any(out.iterdir()):
        die(f"출력 디렉터리가 비어 있지 않다: {out} — 이전 발행 잔재가 섞이면 오염이다(plan C1). "
            "빈 디렉터리를 주거나 지우고 다시 실행하라.")
    (out / "artifacts").mkdir(parents=True, exist_ok=True)

    copied: list[str] = []

    def take(src_rel: str, slot: str) -> None:
        src = repo / src_rel
        dst = out / "artifacts" / slot / Path(src_rel).name
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)
        copied.append(str(dst.relative_to(out)))

    for k, v in slots["triplet"]["files"].items():
        take(v, "triplet")
    if slots["runtime_patch"]["present"]:
        take(slots["runtime_patch"]["files"]["patch_py"], "runtime_patch")
    for key in ("build_patch_pre", "build_patch_post"):
        for rel in slots[key]["files"]:
            take(rel, key)

    # 3항목 저작 스캐폴드 — 기계는 사실만 채우고 판단 자리는 마커로 남긴다
    def ev_rel(key: str) -> str | None:
        node = ev.get(key)
        return node.get("path") if isinstance(node, dict) else None

    (out / "01-artifacts.md").write_text(render_item1(slots), encoding="utf-8")
    (out / "02-narrative.md").write_text(render_item2(ev_rel("devlog"), ev_rel("testlog")),
                                         encoding="utf-8")
    (out / "03-benchmark.md").write_text(render_item3(cert, cert_p.name), encoding="utf-8")

    payload_doc = {
        "schema_version": 1,
        "kind": "hint_payload_facts",
        "generated_kst": a.generated_kst,
        "provenance": "derived",  # 이 문서 자체는 파생물이다(헌법 §결정론 규율 출처 표시)
        "identity": {k: ident.get(k) for k in ("model", "gpu", "vllm", "quant", "topology", "tp")},
        "runtime": {k: (man.get("runtime") or {}).get(k)
                    for k in ("health_ok", "functional_smoke_passed")},
        "benchmark": {k: cert.get(k) for k in CERT_PERF if k in cert},
        "benchmark_source": {"certificate": cert_p.name, "sha256": sha256_of(cert_p),
                             "parsed_not_synthesized": True},
        "slots": slots,
        "source_channel": {
            "topology": topo,
            "read_from": "working-tree(main)",
            "sub_note": "서브 산출물은 이 통로로 오지 않는다 — 문서기반 회수(fetch_sub_docs.sh) 뒤 "
                        "메인이 재저작한다(docs.md 상향 회수 규약).",
        },
        "artifacts": copied,
    }
    decl = {
        "_note": ("신호③ — **Agent 선언**. 각 슬롯의 `applicable` 을 true/false 로, `rationale` 을 "
                  "한 줄로 채운다. null 이 하나라도 남으면 `hint_collect check` 가 막는다. "
                  "선언은 파일 존재·적용 증거와 **대사**되며, 셋이 어긋나면 차단이다(plan §7). "
                  "★ triplet 은 선언으로 면제할 수 없다 — 서빙에 원리적으로 필수다."),
        "schema_version": 1,
        "slots": {name: {"applicable": None, "rationale": None} for name in slots},
    }
    (out / DECLARATION_NAME).write_text(
        json.dumps(decl, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    (out / PAYLOAD_JSON).write_text(
        json.dumps(payload_doc, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8")

    rels = sorted(["01-artifacts.md", "02-narrative.md", "03-benchmark.md",
                   PAYLOAD_JSON, DECLARATION_NAME] + copied)
    (out / FILES_LIST).write_text(
        "# hint 페이로드 allowlist — hint_branch 가 이 목록으로만 트리를 짓는다.\n"
        "# 여기 없는 것은 배포되지 않는다(넣지 않은 것은 들어갈 수 없다).\n"
        + "\n".join(rels) + "\n", encoding="utf-8")

    print(f"[hint_collect] 수집 완료 → {out}")
    print(f"  아티팩트 {len(copied)}건 · 저작 스캐폴드 3건 · allowlist {len(rels)}행")
    for name, sl in slots.items():
        ev = {True: "증거O", False: "증거X", None: "관측불가"}[sl["evidence"]]
        print(f"  slot {name:18s} {'있음' if sl['present'] else '없음':4s} {ev:8s} {sl['slot_confidence']}")
    print(f"[hint_collect] 다음: 02-narrative.md 등의 {AGENT_MARK}…>> 를 Agent 가 채운 뒤 "
          f"`hint_collect check --payload {out}`")
    return 0


def cmd_check(a) -> int:
    """저작 완료 게이트 — Agent 마커가 남아 있으면 발행으로 못 간다."""
    out = Path(a.payload).resolve()
    files_list = out / FILES_LIST
    if not files_list.is_file():
        die(f"{FILES_LIST} 가 없다 — collect 를 먼저 돌려라")
    rels = [l.strip() for l in files_list.read_text(encoding="utf-8").splitlines()
            if l.strip() and not l.startswith("#")]
    problems: list[str] = []
    for rel in rels:
        p = out / rel
        if not p.is_file():
            problems.append(f"{rel}: allowlist 에 있으나 실물 부재")
            continue
        try:
            text = p.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for lineno, line in enumerate(text.splitlines(), 1):
            if AGENT_MARK in line:
                problems.append(f"{rel}:{lineno}: 미저작 Agent 슬롯이 남아 있다")
    # ── 슬롯 3신호 대사 (plan §7) — 파일존재 × 적용증거 × Agent선언
    pj, dj = out / PAYLOAD_JSON, out / DECLARATION_NAME
    verdicts: list[str] = []
    if not pj.is_file() or not dj.is_file():
        problems.append(f"{PAYLOAD_JSON} 또는 {DECLARATION_NAME} 부재 — 3신호 대사 불가(fail-closed)")
    else:
        facts = json.loads(pj.read_text(encoding="utf-8")).get("slots", {})
        decl = json.loads(dj.read_text(encoding="utf-8")).get("slots", {})
        for name, sl in facts.items():
            d = decl.get(name) or {}
            applicable = d.get("applicable")
            verdict, why = dialogue(bool(sl.get("present")), sl.get("evidence"),
                                    applicable, bool(sl.get("exemptible")))
            verdicts.append(f"  slot {name:18s} {verdict:10s} [{sl.get('slot_confidence')}] {why}")
            if verdict != "ok":
                problems.append(f"{DECLARATION_NAME}: 슬롯 `{name}` {verdict} — {why}")
            elif applicable is not None and not (d.get("rationale") or "").strip():
                problems.append(f"{DECLARATION_NAME}: 슬롯 `{name}` 의 rationale 이 비었다 — "
                                "판정에는 이유가 따라야 한다(불해당도 이유를 남긴다)")
        missing_decl = sorted(set(facts) - set(decl))
        if missing_decl:
            problems.append(f"{DECLARATION_NAME}: 선언이 없는 슬롯 {missing_decl}")

    for line in verdicts:
        print(line)
    if problems:
        print(f"[hint_collect] CHECK FAIL — {len(problems)}건", file=sys.stderr)
        for p in problems[:20]:
            print(f"  {p}", file=sys.stderr)
        return 1
    print(f"[hint_collect] CHECK PASS — {len(rels)} 파일 · 미저작 슬롯 0 · 3신호 대사 {len(verdicts)}슬롯 ok")
    return 0


# ---------------------------------------------------------------- self-test

def _run_self_test() -> int:
    import tempfile
    checks: list[tuple[str, bool]] = []

    def ck(n: str, c: bool) -> None:
        checks.append((n, bool(c)))

    def expect_die(fn):
        try:
            fn()
        except SystemExit:
            return "raised"
        return None

    with tempfile.TemporaryDirectory() as tmp:
        t = Path(tmp)
        repo = t / "repo"
        cfg = "demo-gb10"
        (repo / "output" / "single" / "configs").mkdir(parents=True)
        (repo / "output" / "single" / "envs").mkdir(parents=True)
        (repo / "output" / "single" / "configs" / f"{cfg}.yaml").write_text("m: d\n", encoding="utf-8")
        (repo / "output" / "single" / "configs" / f"{cfg}.sh").write_text("#!/bin/sh\n", encoding="utf-8")
        env = repo / "output" / "single" / "envs" / f".env.{cfg}"
        env.write_text("IMAGE_TAG=x\n", encoding="utf-8")

        slots = discover_slots(repo, "single", cfg)
        ck("트리플렛 3 발견", slots["triplet"]["present"] and len(slots["triplet"]["files"]) == 3)
        ck("트리플렛은 면제 불가로 표시", slots["triplet"]["exemptible"] is False)
        ck("VARIANT 줄 없으면 stock", slots["fork_pin"]["present"] is False)
        ck("빌드패치 부재는 1-signal(absence)",
           slots["build_patch_pre"]["slot_confidence"] == "1-signal(absence)")
        ck("런타임패치 부재", slots["runtime_patch"]["present"] is False)

        # ── 2026-09-01 실물 회귀: .gitkeep 을 빌드패치로 세던 결함
        bp = repo / "output" / "single" / "build_patches"
        bp.mkdir(parents=True, exist_ok=True)
        (bp / ".gitkeep").write_text("스켈레톤 마커\n", encoding="utf-8")
        sg = discover_slots(repo, "single", cfg)
        ck("★회귀 .gitkeep 은 빌드패치가 아니다",
           sg["build_patch_post"]["present"] is False
           and sg["build_patch_post"]["slot_confidence"] == "1-signal(absence)")
        (bp / "30-real.sh").write_text("#!/bin/sh\n", encoding="utf-8")
        (bp / "notes.txt").write_text("규약 밖\n", encoding="utf-8")
        sg2 = discover_slots(repo, "single", cfg)
        ck("규약에 맞는 패치만 담김",
           sg2["build_patch_post"]["files"] == ["output/single/build_patches/30-real.sh"])
        ck("★규약 밖 항목은 침묵하지 않고 고지",
           sg2["build_patch_post"].get("nonconforming") == ["notes.txt"])
        shutil.rmtree(bp)

        env.write_text("IMAGE_TAG=x\nVARIANT=source-sm12x-vllm-0.23.0\n", encoding="utf-8")
        s2 = discover_slots(repo, "single", cfg)
        ck("★VARIANT 줄이 있으면 fork_pin 검출",
           s2["fork_pin"]["present"] and s2["fork_pin"]["variant_id"] == "source-sm12x-vllm-0.23.0")
        env.write_text("IMAGE_TAG=x\n", encoding="utf-8")

        # 트리플렛 결손 → 면제 불가가 실제로 막는가
        (repo / "output" / "single" / "configs" / f"{cfg}.sh").unlink()
        s3 = discover_slots(repo, "single", cfg)
        ck("★음성대조 트리플렛 결손 검출",
           (not s3["triplet"]["present"]) and s3["triplet"]["missing"] == ["runner_sh"])
        (repo / "output" / "single" / "configs" / f"{cfg}.sh").write_text("#!/bin/sh\n", encoding="utf-8")

        # 인증서 파싱
        cert = t / "cert.yaml"
        cert.write_text("# 주석\nschema_version: 1\nmodel: demo\n"
                        'gpu_model: "NVIDIA GB10"\ndecode_tps_conc1: 34.55\nverdict: PASS\n',
                        encoding="utf-8")
        doc = parse_certificate(cert)
        ck("인증서 파싱", doc["model"] == "demo" and doc["decode_tps_conc1"] == "34.55")
        ck("따옴표 제거", doc["gpu_model"] == "NVIDIA GB10")
        ck("주석 무시", "주석" not in json.dumps(doc, ensure_ascii=False))

        nested = t / "nested.yaml"
        nested.write_text("a: 1\nb:\n  c: 2\n", encoding="utf-8")
        ck("★음성대조 중첩 인증서 거부", expect_die(lambda: parse_certificate(nested)) == "raised")
        empty = t / "empty.yaml"
        empty.write_text("# 주석뿐\n", encoding="utf-8")
        ck("★음성대조 빈 인증서 거부", expect_die(lambda: parse_certificate(empty)) == "raised")

        # 스캐폴드가 산문을 합성하지 않고 마커를 남기는가
        i1, i2, i3 = render_item1(slots), render_item2("d.md", "t.md"), render_item3(doc, "c.yaml")
        ck("항목1 에 Agent 마커", AGENT_MARK in i1)
        ck("항목2 는 전부 Agent — 마커 2개 이상", i2.count(AGENT_MARK) >= 2)
        ck("항목3 에 like-with-like 마커", AGENT_MARK in i3)
        ck("항목3 이 인증서 값을 그대로 옮김", "34.55" in i3 and "NVIDIA GB10" in i3)
        ck("★항목2 가 devlog 원문을 담지 않음(포인터만)", "복사 대상 아님" in i2)

        # ── 3신호 대사표 (plan §7) — 표의 각 행을 직접 친다
        ck("대사 부재+부재+불해당 → ok", dialogue(False, False, False, True)[0] == "ok")
        ck("대사 존재+증거O+해당 → ok", dialogue(True, True, True, True)[0] == "ok")
        ck("★대사 부재인데 **증거 있음** → 차단(진짜 누락)",
           dialogue(False, True, False, True)[0] == "blocked")
        ck("★대사 존재인데 **증거 없음** → 차단(먹지 않은 패치 배포)",
           dialogue(True, False, True, True)[0] == "blocked")
        ck("★대사 트리플렛 부재는 선언과 무관하게 차단(면제 불가)",
           dialogue(False, None, False, False)[0] == "blocked")
        ck("★대사 선언 미기입 → undeclared", dialogue(True, None, None, True)[0] == "undeclared")
        ck("★대사 파일있음+불해당선언 모순 검출", dialogue(True, None, False, True)[0] == "blocked")
        ck("★대사 파일없음+해당선언 모순 검출", dialogue(False, None, True, True)[0] == "blocked")
        ck("관측불가는 2신호로 표시", confidence_of(None).startswith("2-signal")
           and confidence_of(True).startswith("3-signal"))

        # check 게이트 (3신호 대사 포함)
        pay = t / "pay"
        pay.mkdir()
        facts = {"slots": {
            "triplet": {"present": True, "evidence": True, "exemptible": False,
                        "slot_confidence": "3-signal"},
            "runtime_patch": {"present": False, "evidence": None, "exemptible": True,
                              "slot_confidence": "2-signal"}}}
        decl_ok = {"slots": {"triplet": {"applicable": True, "rationale": "서빙에 쓰였다"},
                             "runtime_patch": {"applicable": False, "rationale": "stock 으로 충분"}}}
        (pay / PAYLOAD_JSON).write_text(json.dumps(facts), encoding="utf-8")
        (pay / DECLARATION_NAME).write_text(json.dumps(decl_ok), encoding="utf-8")
        (pay / "a.md").write_text(f"{AGENT_MARK} 아직 안 씀 >>\n", encoding="utf-8")
        (pay / FILES_LIST).write_text(f"a.md\n{PAYLOAD_JSON}\n{DECLARATION_NAME}\n", encoding="utf-8")
        ck("★음성대조 미저작 마커 차단", cmd_check(argparse.Namespace(payload=str(pay))) == 1)
        (pay / "a.md").write_text("다 썼다\n", encoding="utf-8")
        ck("저작 완료 + 대사 일치 시 통과", cmd_check(argparse.Namespace(payload=str(pay))) == 0)

        decl_null = {"slots": {"triplet": {"applicable": None, "rationale": None},
                               "runtime_patch": {"applicable": False, "rationale": "x"}}}
        (pay / DECLARATION_NAME).write_text(json.dumps(decl_null), encoding="utf-8")
        ck("★음성대조 선언 미기입 차단", cmd_check(argparse.Namespace(payload=str(pay))) == 1)

        decl_no_why = {"slots": {"triplet": {"applicable": True, "rationale": "  "},
                                 "runtime_patch": {"applicable": False, "rationale": "x"}}}
        (pay / DECLARATION_NAME).write_text(json.dumps(decl_no_why), encoding="utf-8")
        ck("★음성대조 이유 없는 판정 차단", cmd_check(argparse.Namespace(payload=str(pay))) == 1)

        # 트리플렛을 선언으로 면제하려는 시도
        facts_no_trip = {"slots": {"triplet": {"present": False, "evidence": None,
                                               "exemptible": False, "slot_confidence": "1-signal"}}}
        (pay / PAYLOAD_JSON).write_text(json.dumps(facts_no_trip), encoding="utf-8")
        (pay / DECLARATION_NAME).write_text(
            json.dumps({"slots": {"triplet": {"applicable": False, "rationale": "필요 없다"}}}),
            encoding="utf-8")
        ck("★음성대조 트리플렛 면제 시도 거부",
           cmd_check(argparse.Namespace(payload=str(pay))) == 1)

        (pay / PAYLOAD_JSON).write_text(json.dumps(facts), encoding="utf-8")
        (pay / DECLARATION_NAME).write_text(json.dumps(decl_ok), encoding="utf-8")
        (pay / FILES_LIST).write_text(
            f"a.md\nghost.md\n{PAYLOAD_JSON}\n{DECLARATION_NAME}\n", encoding="utf-8")
        ck("★음성대조 allowlist 실물부재 차단",
           cmd_check(argparse.Namespace(payload=str(pay))) == 1)

    failed = [n for n, ok in checks if not ok]
    for n, ok in checks:
        print(f"  {'ok  ' if ok else 'FAIL'} {n}")
    print(f"[hint_collect] self-test {len(checks) - len(failed)}/{len(checks)} "
          + ("PASS" if not failed else "FAIL"))
    return 1 if failed else 0


def main() -> int:
    ap = argparse.ArgumentParser(description="hint 페이로드 3소스 수집 + 3항목 저작 스캐폴드")
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--repo", default=".")
    sub = ap.add_subparsers(dest="cmd")

    c = sub.add_parser("collect", help="산출물 수집 + 3항목 스캐폴드 생성")
    c.add_argument("--manifest", required=True, help="work-manifest.json (증거 포인터의 정본)")
    c.add_argument("--config-name", required=True, help="트리플렛 basename (예: gpt-oss-120b-gb10)")
    c.add_argument("--out", required=True, help="페이로드 출력 디렉터리 (비어 있어야 한다)")
    c.add_argument("--topology", default=None, help="미지정 시 manifest 에서 읽는다 (브랜치 추론 ✗)")
    c.add_argument("--generated-kst", required=True, help="시각은 주입만 받는다(벽시계 금지)")
    c.set_defaults(fn=cmd_collect)

    k = sub.add_parser("check", help="저작 완료 게이트 — Agent 마커 잔존 시 fail-closed")
    k.add_argument("--payload", required=True)
    k.set_defaults(fn=cmd_check)

    a = ap.parse_args()
    if a.self_test:
        return _run_self_test()
    if not getattr(a, "fn", None):
        ap.print_help()
        return 2
    return a.fn(a)


if __name__ == "__main__":
    raise SystemExit(main())
