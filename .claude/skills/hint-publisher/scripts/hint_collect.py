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
import importlib.util
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
# `image_digest` — 태그는 가변 포인터라 같은 태그가 다른 내용을 가리킬 수 있다(2026-09-04 실측:
# 하루에 같은 태그가 다섯 내용을 가리켰다). digest 는 그 내용을 유일하게 지목한다. 옛 인증서에는
# 없으므로 `_fmt_kv` 가 부재 키를 건너뛰는 성질에 의존한다(부재 = 그 시점엔 필드가 없었다).
CERT_SOFT = ["driver_version", "cuda_version", "image_tag", "image_digest",
             "max_model_len", "max_num_seqs",
             "kv_cache_memory_bytes", "kv_cache_dtype", "gpu_memory_utilization",
             "moe_backend", "enforce_eager", "ngc_base_tag"]
CERT_PERF = ["benchmark_mode", "verdict", "decode_tps_conc1", "rubric_authority",
             "primary_source", "primary_tps", "floor_tps", "tolerance",
             "ratio_M_over_primary", "spec_on", "accept_len", "sweep_levels",
             "sweep_truncated", "lite_included", "lite_gen_tps_warm", "lite_gen_src",
             "lite_cold_ttft_ms", "lite_kv_gib", "measured_utc"]
# 측정 **구성**은 값이 아니라 조건이다 — 같은 수치라도 도구·버전·이미지·요청 포맷·오류 허용치가
# 다르면 나란히 놓을 수 없다. 2026-09-05(plan_26090516 3-12 · 축 B): hint 동봉 문서에 이것을
# **기재만** 한다(게이트 ✗ — 버전이 다르다고 hint 발행을 막지 않는다. 루브릭이 흔들리는 것은
# 감수하고, 대신 무엇으로 쟀는지가 문서에 남아 독자가 스스로 판단한다).
CERT_BENCH_TOOL = ["bench_tool", "bench_tool_version", "bench_tool_version_source",
                   "bench_tool_image_ref", "bench_tool_image_digest",
                   "bench_tool_image_digest_source", "bench_endpoint", "bench_max_error_rate"]


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

# 빌드 패치 디렉터리를 활성 Dockerfile 이 실제로 참조하는지 보는 패턴. `build_patches_src` 가
# `build_patches` 를 부분문자열로 포함하므로 post 는 `_src` 를 부정형으로 배제해야 한다.
_PRE_REF = re.compile(r"build_patches_src")
_POST_REF = re.compile(r"build_patches(?!_src)")


def _dry_copy_keys(slots: dict) -> list[str]:
    """복사 대상 슬롯 키 — `collect` 의 복사 루프와 **같은 술어**를 쓴다(둘이 갈리면 자체검사가 무의미).

    자체검사용 순수 함수이며 파일을 만들지 않는다.
    """
    return [key for key in ("build_patch_pre", "build_patch_post", "build_recipe", "compose")
            if slots[key]["present"] for _ in slots[key]["files"]]


def env_p_text(env_file: Path) -> str:
    return env_file.read_text(encoding="utf-8") if env_file.is_file() else ""


def build_track_is_wheel(env_text: str) -> bool:
    """활성 빌드 트랙. 선택자(`BUILD_DOCKERFILE`)가 있으면 그것이 정본이고, 없으면 태그로 판정한다.

    태그를 2순위로 내린 근거: 이미지 태그는 **가변 포인터**라 내용과 갈릴 수 있다(2026-09-04 실측 —
    `0.18.0-…-wheel` 태그가 0.27.1 소스빌드 내용을 가리켰다). 선택자는 빌드 입력 자체다.
    """
    selector = tag = None
    for line in env_text.splitlines():
        st = line.strip()
        if st.startswith("BUILD_DOCKERFILE="):
            selector = st.split("=", 1)[1].strip()
        elif st.startswith("IMAGE_TAG="):
            tag = st.split("=", 1)[1].strip()
    if selector:
        return selector == "Dockerfile"
    return "-source" not in (tag or "")


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
    # ── 활성 빌드 레시피를 **먼저** 정한다. 빌드 패치가 "이 이미지에 먹었는가" 는 디렉터리에
    #    파일이 있느냐가 아니라 **선택된 Dockerfile 이 그 디렉터리를 참조하느냐**로 갈린다.
    #    (2026-09-04 실측: 멀티 wheel 트랙은 build_patches* 를 렌더해 두지만 wheel Dockerfile 은
    #     컴파일 자체가 없어 하나도 실행하지 않는다. 디렉터리만 보면 '있음' 이 되어, 대사표가
    #     Agent 에게 `applicable:true` 를 강요하고 **먹지 않은 패치가 재현지침으로 배포**된다 —
    #     §3.1 이 막으려던 바로 그 해악이 반대편 문으로 들어온다.)
    env_text = env_p_text(triplet["env_file"])
    wheel_track = build_track_is_wheel(env_text)
    recipe_files = [out / "Dockerfile", out / "requirements.txt"]
    if not wheel_track:
        recipe_files.insert(1, out / "Dockerfile.source-build")
    found_recipe = [f for f in recipe_files if f.is_file()]
    recipe_texts = [f.read_text(encoding="utf-8", errors="replace")
                    for f in found_recipe if f.name.startswith("Dockerfile")]

    for key, d, phase, ref in (("build_patch_pre", pre_dir, "컴파일 전", _PRE_REF),
                               ("build_patch_post", post_dir, "컴파일 후", _POST_REF)):
        found, odd = listing(d)
        applied = any(ref.search(t) for t in recipe_texts)
        slots[key] = {
            "phase": phase,
            "owner": "upstream-version-watch",
            "files": [str(p.relative_to(repo)) for p in found],
            # ★ 존재 ∧ 활성 레시피가 참조 — 둘 다여야 "이 재현 키트의 일부" 다
            "present": bool(found) and applied,
            "exemptible": True,
            "slot_confidence": "3-signal(file+provenance+declaration)" if (found and applied)
                               else "1-signal(absence)",
        }
        if found and not applied:
            # 침묵 배제 금지 — 왜 빠졌는지 산출물이 스스로 밝힌다(헌법 §결정론 규율 출처 표시)
            slots[key]["excluded_by_recipe"] = {
                "reason": "활성 빌드 레시피가 이 디렉터리를 참조하지 않는다 — 이 이미지에 먹지 않았다",
                "recipe_files": [str(f.relative_to(repo)) for f in found_recipe
                                 if f.name.startswith("Dockerfile")],
                "rendered_but_unused": [str(p.relative_to(repo)) for p in found],
            }
            print(f"[hint_collect] 알림: {d} 에 {len(found)}건이 있으나 활성 레시피가 참조하지 않아 "
                  "슬롯을 '불해당' 으로 둔다(먹지 않은 패치는 재현지침이 아니다)", file=sys.stderr)
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
    # ── 바깥영역 확장 (2026-09-01 사용자 요청): 3+1+1 은 **서빙 시점**과 **패치** 평면만 덮는다.
    #    이미지를 *어떻게 지었나*(Dockerfile·requirements)와 *어떻게 띄우나*(compose·env 형상)가
    #    빠지면 재현 키트로 불완전하다. 슬롯이 아니라 **재현 자산**이므로 3+1+1 표를 늘리지 않고
    #    별도 두 칸으로 둔다 — 헌법의 슬롯 분류를 흐리지 않기 위해서다.
    # wheel_track·found_recipe 는 위(빌드 패치 대사 앞)에서 이미 산출했다 — 같은 값을 두 번 계산하면
    # 두 자리가 갈린다(헌법 §단일 권위).
    slots["build_recipe"] = {
        "phase": "build",
        "owner": "upstream-version-watch",
        "files": [str(f.relative_to(repo)) for f in found_recipe],
        "present": bool(found_recipe),
        "exemptible": False,   # 이미지를 지은 레시피 없이는 재현이 불가능하다
        "track": "wheel" if wheel_track else "source-build",
        "slot_confidence": "1-signal(file-presence)",
    }
    compose_files = [f for f in (out / "docker-compose.yaml", out / "docker-compose.yml")
                     if f.is_file()]
    slots["compose"] = {
        "phase": "serve(orchestration)",
        "owner": "upstream-version-watch",
        "files": [str(f.relative_to(repo)) for f in compose_files],
        "present": bool(compose_files),
        "exemptible": False,   # 기동 방법 없이는 재현이 불가능하다
        # 토폴로지 .env 는 **실물을 담지 않는다** — 운영자 절대경로(NAS·tiktoken)를 담기 때문이다.
        # 대신 변수 **형상**만 템플릿으로 옮긴다(어떤 변수가 필요한지는 재현에 필수 정보다).
        "topology_env_template_from": str((out / ".env").relative_to(repo))
                                      if (out / ".env").is_file() else None,
        "slot_confidence": "1-signal(file-presence)",
    }

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


# ── env 값 형상화 규칙 (2026-09-06 · plan_26090616 Q9 · 사용자 결정 "절대경로보다 신원류 형식") ──
#
# 종전 규칙은 **값이 `/` 로 시작하면 가린다** 하나뿐이었다. 그래서 운영자 호스트 IP·계정처럼
# 경로가 아닌 지문은 그대로 실렸고, 반대로 `MAX_JOBS=8` 같은 재현 필수 튜닝값은 남아야 하는데
# 남는 근거가 "우연히 `/` 로 시작하지 않아서" 였다. 판정 축을 **값의 모양이 아니라 키의 신원성**
# 으로 바꾼다 — 그래야 새 변수가 생겨도 이름만 보고 옳게 갈린다.
IDENTITY_KEY_AXIS = ("_HOST_IP", "_HOSTNAME", "_HOST", "_USER", "_IP", "_ADDR", "_ADDRESS",
                     "_SSH", "_KEY", "_TOKEN", "_SECRET", "_PASSWORD", "_ACCOUNT", "_MAIL")
# 튜닝류 — 값 자체가 **재현에 필요한 사실**이지 환경 지문이 아니다. 이 목록은 닫힌 tripwire 다:
# 새 튜닝 변수가 생기면 여기 등재하며 그때 사람이 "정말 지문이 아닌가" 를 한 번 본다.
TUNING_KEYS = frozenset({
    "RAY_PORT", "MAX_JOBS", "PYTORCH_CUDA_ALLOC_CONF", "RAY_OBJECT_STORE_MEMORY",
    "VLLM_VERSION", "IMAGE_TAG", "TENSOR_PARALLEL_SIZE", "GPU_MEMORY_UTILIZATION",
    "MAX_MODEL_LEN", "MAX_NUM_SEQS", "KV_CACHE_DTYPE", "MOE_BACKEND", "ATTENTION_BACKEND",
})


def shape_env_value(key: str, val: str) -> str:
    """`.env` 한 줄의 값 → 배포 가능한 형상. **키는 부르는 쪽이 항상 남긴다.**"""
    upper = key.upper()
    if upper in TUNING_KEYS:
        return val
    if val.startswith("/"):
        return f"<manifest.{key.lower()}>"
    if any(upper.endswith(suffix) or f"{suffix}_" in f"_{upper}_" for suffix in IDENTITY_KEY_AXIS):
        return f"<manifest.nodes[].{key.lower()}>"
    return val


def _generic_pii_hits(text: str) -> list:
    """배포 강도 4종 정규식 매치. 패턴의 단일 소유자는 `hint_tag.GENERIC_PII` 다 — 복제하지 않는다."""
    try:
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        import hint_tag as _ht
    except Exception as exc:                                  # pragma: no cover - 배선 실패는 소리내어
        die(f"백스톱 패턴을 적재하지 못했다(hint_tag): {exc} — 스캔 없이 통과시키지 않는다")
    hits = []
    for lineno, line in enumerate(text.splitlines(), 1):
        for name, pat in _ht.GENERIC_PII:
            for m in pat.finditer(line):
                hits.append(f"{lineno}:{name}:{m.group(0)}")
    return hits


def env_shape_template(text: str) -> str:
    """토폴로지 `.env` 의 **형상만** 옮긴다 — 값은 플레이스홀더로 바꾼다.

    그 파일은 운영자 절대경로(NAS 루트·tiktoken 경로)를 담고, 자기 주석이 스스로
    *"gitignored(output/* — PII)"* 라고 적는다. 그러나 **어떤 변수가 필요한지**는 재현에
    필수 정보다. 값을 지우고 형상을 남기는 것이 정답이며, 어디서 얻는지도 함께 적는다
    (docs.md `request` §자제: 플레이스홀더 + 획득 방법).
    """
    out = ["# 이 파일은 **형상 템플릿**이다 — 값은 발행자 환경의 것이 아니라 플레이스홀더다.",
           "# 각 값의 정본은 manifest 의 동명 필드다. 자기 환경 값으로 채워 쓰라.",
           "# (원본은 배포되지 않는다 — 운영자 절대경로·호스트·계정을 담기 때문이다.)",
           "# 키는 **항상 남는다** — 어떤 변수가 필요한지가 재현의 핵심이고, 키를 지우면 수신자는",
           "# 그 변수의 존재 자체를 모른다(멀티 클러스터 5변수가 그렇게 페이로드에서 빠졌다).", ""]
    for raw in text.splitlines():
        line = raw.rstrip()
        if not line or line.lstrip().startswith("#"):
            continue
        if "=" not in line:
            continue
        key, val = line.split("=", 1)
        key, val = key.strip(), val.strip()
        out.append(f"{key}={shape_env_value(key, val)}")
    shaped = "\n".join(out) + "\n"
    # ── 2차 백스톱(2026-09-06 · plan_26090616 Q9) ──
    # 위 형상화는 **키 축** 판정이라 새 이름의 지문 변수를 놓칠 수 있다. 그래서 형상화 뒤 결과를
    # 배포 강도 4종 정규식으로 한 번 더 훑고, 남아 있으면 **가리지 않고 차단**한다 — 조용히 덧칠하면
    # 형상화가 무엇을 놓쳤는지 영영 드러나지 않는다(침묵 폴백 금지).
    residue = _generic_pii_hits(shaped)
    if residue:
        die("★ env 형상화 뒤에도 배포 금지 패턴이 남았다 — 조용히 덧칠하지 않는다(fail-closed):\n  "
            + "\n  ".join(residue[:10])
            + "\n  → 해당 키를 `IDENTITY_KEY_AXIS` 에 편입하거나, 값의 성격을 확인해 형상화 규칙을 고쳐라.")
    return shaped


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
    # 인증서가 측정한 이미지와 기동이 가리키는 이미지가 같은가 — 관측 가능하고 의미가 크다.
    # 다르면 "재현지침이 다른 이미지를 띄운다"는 뜻이고, 그것이 벤치를 거짓말하게 만드는
    # 가장 값싼 경로다(.env 주석이 이미 경고한다).
    "build_recipe": "certificate.image_tag ↔ .env IMAGE_TAG",
    "compose": "certificate.image_tag ↔ .env IMAGE_TAG",
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
        # 활성 레시피가 참조하지 않으면 PROVENANCE 가 있어도 **이 이미지에는** 적용되지 않았다.
        # (이식 기록은 "이식했다" 는 증거이지 "이 빌드가 실행했다" 는 증거가 아니다.)
        if slots.get("build_patch_pre", {}).get("excluded_by_recipe"):
            return False
        prov = repo / "output" / topo / "build_patches_src" / "PROVENANCE.json"
        if not prov.is_file():
            return False
        try:
            doc = json.loads(prov.read_text(encoding="utf-8"))
        except Exception:
            return False
        return bool(doc.get("counts", {}).get("files"))
    if slot in ("build_recipe", "compose"):
        want = cert.get("image_tag")
        if not want:
            return None
        envrel = slots["triplet"]["files"].get("env_file")
        if not envrel:
            return None
        for line in (repo / envrel).read_text(encoding="utf-8").splitlines():
            if line.strip().startswith("IMAGE_TAG="):
                return line.split("=", 1)[1].strip() == want
        return False
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


# ── 결손 사유코드 (2026-09-06 · plan_26090616 Q7/Q8 · 사용자 결정 "강행 발행") ──
#
# 정책이 바뀌었다: hint 태그는 **토큰노믹스 정책**이며 필수는 "여정 정보" 하나뿐이다. 그 밖의
# 부재는 **차단 사유가 아니라 기재 대상**이다 — 부재로 발행을 막으면 발행돼야 할 hint 가 안 나가고
# (2026-09-05 실측: 기대 3종 중 2종), 그러면 수신자는 "이 조합은 시도된 적 없다" 로 오독한다.
# 차단은 **양성 검출**(서빙 성공 허위 · 3신호 모순 · PII 매치)일 때만이다.
MISSING_CODES = {
    "HINT_MISSING_CERTIFICATE": "인증서 부재 — full PASS 가 아니었거나 벤치마커가 발행하지 않았다. "
                                "인증서 발행은 adversarial-benchmark 의 책임이지 발행기의 책임이 아니다.",
    "HINT_MISSING_BENCH_REPORT": "벤치 리포트 부재 — 동시성별 곡선을 실을 수 없다.",
    "HINT_MISSING_SWEEP_LEVELS": "부하 레벨이 1개뿐 — 부하 거동을 알 수 없다.",
    "HINT_MISSING_LITE": "lite 관측 부재.",
    "HINT_MISSING_SLAVE_ATTESTATION": "슬레이브 ABI attestation 부재 — 멀티에서 두 노드가 같은 것을 "
                                      "돌렸다는 증거가 성공 경로에 보존되지 않았다.",
    "HINT_MISSING_ENV_SHAPE": "토폴로지 env 형상 부재 — 수신자가 어떤 변수가 필요한지 모른다.",
    "HINT_MISSING_SUB_TRIPLET": "서브 트리플렛 부재(서브는 자기 것을 자율 저작하며 메인으로 전파하지 않는다).",
    "HINT_MISSING_PII_TERMS": "pii_terms.txt 부재 — 리터럴 스캔이 축소된 상태로 돌았다.",
    "HINT_MISSING_MEASURED_NODE": "인증서에 측정 노드 출처가 없다 — 어느 노드가 쟀는지 단정할 수 없다.",
}


def _bench_section_module():
    """`render_bench_section.py` 를 in-process 로 적재한다. 표 렌더의 **단일 소유자**이며
    여기서 파싱을 복제하지 않는다 — 복제하면 발행기와 검증기가 갈라져 `--verify` 가 무의미해진다."""
    path = Path(__file__).resolve().parent / "render_bench_section.py"
    spec = importlib.util.spec_from_file_location("_hint_bench_section", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def render_missing_block(missing: list) -> str:
    """결손을 **본문에 적는다**. 비어 있으면 그것도 적는다 — 침묵은 '없음' 과 구분되지 않는다."""
    if not missing:
        return "_결손 없음 — 이 절이 요구하는 증거가 모두 도착했다._"
    lines = ["> ⚠ 아래 정보가 **부재한 채로 발행**됐다. 부재는 실패가 아니라 **기록**이며, 이 태그를",
             "> 소비할 때 그만큼을 모른 채 소비한다는 뜻이다(강행 발행 정책 · plan_26090616 Q7/Q8).", "",
             "| 사유코드 | 뜻 |", "|---|---|"]
    for code in missing:
        lines.append(f"| `{code}` | {MISSING_CODES.get(code, '미등록 사유코드 — 코드표를 갱신하라')} |")
    return "\n".join(lines)


def render_item3(cert: dict, cert_name: str, bench_section: str = "",
                 missing: list | None = None) -> str:
    missing = missing or []
    if not cert:
        return "\n".join([
            "# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나", "",
            "> **인증서가 없다.** 이 절은 인증서 없이 발행된다 — 그것이 정책이다(강행 발행).",
            "> 인증서는 full 모드 verdict==PASS 일 때만 나오며, 그 발행은 `adversarial-benchmark` 의",
            "> 책임이다. 부재는 '성능이 나빴다' 가 아니라 '**그 형태로 판정되지 않았다**' 는 뜻이다.", "",
            render_missing_block(missing), "",
            bench_section or "_동시성별 곡선도 없다(벤치 리포트 부재)._", "",
            "## like-with-like 한정자 (Agent)", "",
            f"{AGENT_MARK} 인증서 없이 무엇을 말할 수 있고 무엇은 말할 수 없는지 쓴다. "
            "수치가 없다면 여정(무엇을 시도했고 어디서 멈췄나)이 이 태그의 값이다. >>", ""])
    return "\n".join([
        "# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나", "",
        "> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.",
        f"> 출처: `{cert_name}`", "",
        "## 성능", "", "| 항목 | 값 |", "|---|---|", _fmt_kv(cert, CERT_PERF), "",
        "## 측정 구성 — 무엇으로 쟀나(기재 · 게이트 아님)", "",
        "> 도구·버전이 다르면 수치를 나란히 놓기 전에 조건부터 본다. 부재 키는 그 시점에 그 필드가",
        "> 없었다는 뜻이다(합성하지 않는다).", "",
        "| 항목 | 값 |", "|---|---|", _fmt_kv(cert, CERT_BENCH_TOOL), "",
        "## 강한 일치 키 — 하나라도 다르면 이 수치는 **무효**다", "",
        "| 키 | 값 |", "|---|---|", _fmt_kv(cert, CERT_STRONG), "",
        "## 소프트 지문 — 다르면 stale, 재측정 권고", "",
        "| 키 | 값 |", "|---|---|", _fmt_kv(cert, CERT_SOFT), "",
        bench_section, "",
        "## 결손 기재", "", render_missing_block(missing), "",
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

    # ── 인증서 부재는 **차단이 아니라 기재**다 (2026-09-06 · plan_26090616 Q7/Q8) ──
    #   종전에는 여기서 죽었다. 그런데 인증서 발행은 `adversarial-benchmark` 의 책임이고,
    #   "캠페인을 종료했다" 와 "hint 를 발행해야 한다" 는 **독립 사건**이다(사용자 결정).
    #   발행기가 남의 책임 부재로 자기 발행을 막으면, 발행돼야 할 hint 가 안 나가고 수신자는
    #   "이 조합은 시도된 적 없다" 로 오독한다. 합성은 여전히 금지다 — 없는 수치를 지어내지 않고,
    #   **없다는 사실을 사유코드로 적는다**.
    missing: list = []
    cert_p = ev_path("certificate")
    if not cert_p or not cert_p.is_file():
        cert, cert_name = {}, "(부재)"
        missing.append("HINT_MISSING_CERTIFICATE")
        print(f"[hint_collect] ⚠ 인증서 부재({cert_p}) — 결손으로 기재하고 발행을 강행한다.",
              file=sys.stderr)
    else:
        cert = parse_certificate(cert_p)
        cert_name = cert_p.name
        if not str(cert.get("measured_node") or "").strip():
            missing.append("HINT_MISSING_MEASURED_NODE")

    # 동시성별 곡선은 **인증서가 아니라 벤치 리포트**에서 온다(인증서는 판정점 하나만 싣는다).
    bench_section = ""
    report_p = ev_path("bench_report") or ev_path("report")
    if report_p and report_p.is_file():
        _rbs = _bench_section_module()
        try:
            parsed = _rbs.parse_report(report_p.read_text(encoding="utf-8"))
            bench_section = _rbs.render(parsed, report_p.name)
            if len(parsed["levels"]) <= 1:
                missing.append("HINT_MISSING_SWEEP_LEVELS")
        except _rbs.BenchSectionFailure as exc:
            print(f"[hint_collect] ⚠ 벤치 리포트 파싱 실패({report_p.name}): {exc}", file=sys.stderr)
            missing.append("HINT_MISSING_BENCH_REPORT")
    else:
        missing.append("HINT_MISSING_BENCH_REPORT")
        print("[hint_collect] ⚠ 벤치 리포트 포인터가 없다 — 동시성별 곡선 없이 발행한다.",
              file=sys.stderr)
    if not (repo / ".claude" / "pii_terms.txt").is_file():
        missing.append("HINT_MISSING_PII_TERMS")

    # 멀티는 **두 노드가 같은 것을 돌렸다**는 증거가 있어야 태그가 클러스터를 대표한다.
    # 그 대조는 multinode_serve_smoke 가 하고 2026-09-06 부터 성공 경로에서도 파일로 남긴다.
    attestation_rel = None
    if topo == "multi":
        attest = repo / "output" / "multi" / "benchlog" / f"attestation_{a.config_name}.json"
        if attest.is_file():
            attestation_rel = str(attest.relative_to(repo))
        else:
            missing.append("HINT_MISSING_SLAVE_ATTESTATION")
            print(f"[hint_collect] ⚠ 노드 정합 attestation 부재({attest.name}) — 결손으로 기재한다.",
                  file=sys.stderr)

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
    for key in ("build_patch_pre", "build_patch_post", "build_recipe", "compose"):
        # ★ 배제된 슬롯은 **실물도 싣지 않는다**. 슬롯을 '불해당' 으로 두고 파일만 archive 에
        #   넣으면 수신자는 그 디렉터리를 재현 지침으로 읽는다 — 배제의 의미가 사라진다.
        if not slots[key]["present"]:
            continue
        for rel in slots[key]["files"]:
            take(rel, key)
    # 토폴로지 env 는 실물이 아니라 **형상 템플릿**으로 옮긴다(PII).
    envsrc = slots["compose"].get("topology_env_template_from")
    if envsrc:
        tpl = out / "artifacts" / "compose" / "topology.env.template"
        tpl.parent.mkdir(parents=True, exist_ok=True)
        tpl.write_text(env_shape_template((repo / envsrc).read_text(encoding="utf-8")),
                       encoding="utf-8")
        copied.append(str(tpl.relative_to(out)))

    # 3항목 저작 스캐폴드 — 기계는 사실만 채우고 판단 자리는 마커로 남긴다
    def ev_rel(key: str) -> str | None:
        node = ev.get(key)
        return node.get("path") if isinstance(node, dict) else None

    (out / "01-artifacts.md").write_text(render_item1(slots), encoding="utf-8")
    (out / "02-narrative.md").write_text(render_item2(ev_rel("devlog"), ev_rel("testlog")),
                                         encoding="utf-8")
    (out / "03-benchmark.md").write_text(
        render_item3(cert, cert_name, bench_section=bench_section, missing=missing),
        encoding="utf-8")

    payload_doc = {
        "schema_version": 1,
        "kind": "hint_payload_facts",
        "generated_kst": a.generated_kst,
        "provenance": "derived",  # 이 문서 자체는 파생물이다(헌법 §결정론 규율 출처 표시)
        "identity": {k: ident.get(k) for k in ("model", "gpu", "vllm", "quant", "topology", "tp")},
        "runtime": {k: (man.get("runtime") or {}).get(k)
                    for k in ("health_ok", "functional_smoke_passed")},
        "benchmark": {k: cert.get(k) for k in CERT_PERF if k in cert},
        "bench_tool": {k: cert.get(k) for k in CERT_BENCH_TOOL if k in cert},
        "benchmark_source": ({"certificate": cert_name, "sha256": sha256_of(cert_p),
                              "parsed_not_synthesized": True}
                             if cert else {"certificate": None, "parsed_not_synthesized": True,
                                           "absent_reason": "HINT_MISSING_CERTIFICATE"}),
        # 결손은 페이로드에도 남는다 — 본문만 적으면 기계가 읽을 수 없고, 기계가 못 읽으면
        # 카탈로그·검증기가 "무엇을 모른 채 발행됐는가" 를 집계하지 못한다.
        "missing": sorted(set(missing)),
        # 멀티 노드 정합 attestation 포인터(사본 ✗). 부재는 위 missing 이 말한다.
        "node_parity_attestation": attestation_rel,
        "missing_policy": ("forced_publication: 부재는 기재하고 발행한다. 차단은 양성 검출"
                           "(서빙 성공 허위 · 3신호 모순 · PII 매치)일 때만이다 — plan_26090616 Q7/Q8."),
        "slots": slots,
        "source_channel": {
            "topology": topo,
            "read_from": "working-tree(main)",
            "sub_note": ("서브 산출물은 이 통로로 오지 않는다 — 문서기반 회수(fetch_sub_docs.sh) 뒤 "
                         "메인이 재저작한다(docs.md 상향 회수 규약). 2026-09-06 이후 그 회수분은 "
                         "`sync_staging/sub_docs/benchmark/` 에서 열리며, 어느 노드가 쟀는지는 "
                         "인증서 `measured_node` 가 말한다 — 서브 리포트를 --report 로 지정하면 "
                         "이 발행기가 서브의 동시성 곡선을 그대로 렌더한다(발행 주체는 메인 단독)."),
            "sub_publish_phase": ("서브의 publish 위상이 만든 `hint_inputs` 사이드카(task-report "
                                  "스키마)가 이 통로의 입력이다. 서브는 태그를 발행하지 않는다."),
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

        # ── 바깥영역 확장: 빌드 레시피 · compose (2026-09-01 신설)
        (repo / "output" / "single" / "Dockerfile").write_text("FROM x\n", encoding="utf-8")
        (repo / "output" / "single" / "requirements.txt").write_text("vllm==1\n", encoding="utf-8")
        (repo / "output" / "single" / "docker-compose.yaml").write_text("services: {}\n", encoding="utf-8")
        (repo / "output" / "single" / ".env").write_text(
            "# 주석\nNAS_MODEL_PATH=/mnt/fixture-nas/Model/x\nIMAGE_TAG=easy-vllm:1-wheel\n", encoding="utf-8")
        env.write_text("IMAGE_TAG=easy-vllm:1-wheel\n", encoding="utf-8")
        sx = discover_slots(repo, "single", cfg)
        ck("build_recipe 발견(Dockerfile+requirements)",
           sx["build_recipe"]["present"] and len(sx["build_recipe"]["files"]) == 2)
        ck("wheel 트랙이면 source-build Dockerfile 미포함",
           sx["build_recipe"]["track"] == "wheel"
           and not any("source-build" in f for f in sx["build_recipe"]["files"]))
        ck("compose 발견 + env 형상 출처 기록",
           sx["compose"]["present"] and sx["compose"]["topology_env_template_from"].endswith(".env"))
        ck("build_recipe·compose 는 면제 불가",
           sx["build_recipe"]["exemptible"] is False and sx["compose"]["exemptible"] is False)

        (repo / "output" / "single" / "Dockerfile.source-build").write_text("FROM y\n", encoding="utf-8")
        env.write_text("IMAGE_TAG=easy-vllm:1-source-sm12x\n", encoding="utf-8")
        sy = discover_slots(repo, "single", cfg)
        ck("★source 트랙이면 source-build Dockerfile 포함",
           sy["build_recipe"]["track"] == "source-build"
           and any("source-build" in f for f in sy["build_recipe"]["files"]))
        env.write_text("IMAGE_TAG=easy-vllm:1-wheel\n", encoding="utf-8")

        # ── 2026-09-04 실물 회귀: 렌더돼 있으나 **활성 레시피가 안 쓰는** 빌드 패치
        #    (멀티 wheel 트랙. 디렉터리만 보면 '있음' 이라 대사표가 `applicable:true` 를 강요했다.)
        bps = repo / "output" / "single" / "build_patches_src"
        bps.mkdir(parents=True, exist_ok=True)
        (bps / "50-port.sh").write_text("#!/bin/sh\n", encoding="utf-8")
        (bps / "PROVENANCE.json").write_text('{"counts": {"files": 3}}', encoding="utf-8")
        bpp = repo / "output" / "single" / "build_patches"
        bpp.mkdir(parents=True, exist_ok=True)
        (bpp / "10-native.sh").write_text("#!/bin/sh\n", encoding="utf-8")
        sz = discover_slots(repo, "single", cfg)
        ck("★wheel 레시피가 참조 안 하면 빌드패치는 불해당",
           sz["build_patch_pre"]["present"] is False and sz["build_patch_post"]["present"] is False)
        ck("★배제는 침묵하지 않는다(사유·레시피·미사용 목록 기록)",
           set(sz["build_patch_pre"]["excluded_by_recipe"]) == {"reason", "recipe_files",
                                                               "rendered_but_unused"})
        ck("★PROVENANCE 가 있어도 레시피 미참조면 적용증거 아님",
           evidence_signal(repo, "single", "build_patch_pre", sz, {"image_tag": "easy-vllm:1-wheel",
                                                                  "serving_config": "demo"}) is False)
        ck("★그 조합의 대사는 '불해당' 으로 통과한다(applicable=false)",
           dialogue(False, False, False, True)[0] == "ok")

        # 음성대조 — 레시피가 참조하면 같은 파일이 '있음' 이 된다(교정이 무조건 배제가 아님을 증명)
        (repo / "output" / "single" / "Dockerfile").write_text(
            "FROM x\nCOPY build_patches_src /tmp/bps\nCOPY build_patches /tmp/bp\n", encoding="utf-8")
        sz2 = discover_slots(repo, "single", cfg)
        ck("★음성대조 레시피가 참조하면 빌드패치 present",
           sz2["build_patch_pre"]["present"] is True and sz2["build_patch_post"]["present"] is True
           and "excluded_by_recipe" not in sz2["build_patch_pre"])
        ck("★배제 슬롯은 아티팩트로 복사되지 않는다",
           not any("build_patch" in c for c in _dry_copy_keys(sz)))
        ck("★음성대조 참조되면 복사 대상이 된다",
           any("build_patch_pre" in c for c in _dry_copy_keys(sz2)))
        ck("★post 참조 판정이 build_patches_src 에 오염되지 않는다",
           _POST_REF.search("COPY build_patches_src /x") is None
           and _POST_REF.search("COPY build_patches /x") is not None)
        (repo / "output" / "single" / "Dockerfile").write_text("FROM x\n", encoding="utf-8")
        shutil.rmtree(bps); shutil.rmtree(bpp)

        # 트랙 선택자 우선순위 — 태그가 아니라 BUILD_DOCKERFILE 이 정본이다
        ck("★선택자가 태그를 이긴다(태그 wheel · 선택자 source-build)",
           build_track_is_wheel("IMAGE_TAG=easy-vllm:1-wheel\nBUILD_DOCKERFILE=Dockerfile.source-build\n")
           is False)
        ck("선택자 부재 시 태그로 판정",
           build_track_is_wheel("IMAGE_TAG=easy-vllm:1-source-x\n") is False
           and build_track_is_wheel("IMAGE_TAG=easy-vllm:1-wheel\n") is True)

        # env 형상 템플릿 — 절대경로만 가리고 나머지는 남긴다
        tpl = env_shape_template("# 주석\nNAS_MODEL_PATH=/mnt/fixture-nas/Model/x\n"
                                 "IMAGE_TAG=easy-vllm:1-wheel\nTIKTOKEN_ENABLED=true\n")
        ck("★env 템플릿이 운영자 절대경로를 가린다",
           "/mnt/fixture-nas" not in tpl and "<manifest.nas_model_path>" in tpl)
        ck("★env 템플릿이 비-경로 값은 남긴다",
           "IMAGE_TAG=easy-vllm:1-wheel" in tpl and "TIKTOKEN_ENABLED=true" in tpl)
        ck("env 템플릿에 원본 주석이 실리지 않는다", "주석" not in tpl)

        # 신호② — 인증서 image_tag ↔ .env IMAGE_TAG
        certx = {"image_tag": "easy-vllm:1-wheel", "serving_config": "demo"}
        ck("★신호② 이미지 태그 일치 → 증거O",
           evidence_signal(repo, "single", "compose", sx, certx) is True)
        certy = {"image_tag": "easy-vllm:9-other", "serving_config": "demo"}
        ck("★신호② 이미지 태그 불일치 → 증거X (재현지침이 다른 이미지를 띄운다)",
           evidence_signal(repo, "single", "build_recipe", sx, certy) is False)

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

    # ── cmd_collect 실경로 (2026-09-06 신설) ────────────────────────────────────────────
    # ★ 왜 이제서야: 이 자체검사는 `discover_slots`·`cmd_check` 만 돌았고 **`cmd_collect` 는 한 번도
    #   실행하지 않았다**. 그래서 그 함수 안의 배선 결함(할당 전 사용)이 55/55 초록 아래에서
    #   그대로 살아 있었다 — 픽스처가 실물보다 좁으면 시험은 초록인데 실물이 죽는다.
    def _mk_tree(root: Path, topo: str, cfg: str) -> Path:
        (root / "output" / topo / "configs").mkdir(parents=True, exist_ok=True)
        (root / "output" / topo / "envs").mkdir(parents=True, exist_ok=True)
        (root / "output" / topo / "configs" / f"{cfg}.yaml").write_text("m: d\n", encoding="utf-8")
        (root / "output" / topo / "configs" / f"{cfg}.sh").write_text("#!/bin/sh\n", encoding="utf-8")
        (root / "output" / topo / "envs" / f".env.{cfg}").write_text("IMAGE_TAG=x\n", encoding="utf-8")
        (root / "output" / topo / "envs" / f".env.{topo}").write_text(
            "MASTER_HOST_IP=192.168.0.11\nRAY_PORT=6379\nSSH_USER=someone\n", encoding="utf-8")  # pii-scan-fixture
        return root

    def _mk_manifest(evdir: Path, cert_rel, report_rel) -> Path:
        ev = {}
        if cert_rel:
            ev["certificate"] = {"path": cert_rel}
        if report_rel:
            ev["bench_report"] = {"path": report_rel}
        doc = {"identity": {"model": "demo", "gpu": "GB10", "vllm": "0.19.0",
                            "quant": "mxfp4", "topology": "single", "tp": 1},
               "runtime": {"health_ok": True, "functional_smoke_passed": True},
               "evidence": ev}
        mp = evdir / "work-manifest.json"
        mp.write_text(json.dumps(doc, ensure_ascii=False), encoding="utf-8")
        return mp

    REPORT_ROWS = ("| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |\n"
                   "|---|---|---|---|---|---|---|\n"
                   "| 1 ★판정점 | 45.28 | 45.30 | 226.70 | 160.76 | 21.54 | 16/0 |\n"
                   "| 2 | 42.84 | 86.04 | 430.51 | 39.54 | 23.20 | 16/0 |\n")

    with tempfile.TemporaryDirectory() as tmp:
        t2 = Path(tmp)
        cfg = "demo-gb10"
        repo2 = _mk_tree(t2 / "repo", "single", cfg)
        evdir = t2 / "ev"
        evdir.mkdir()
        (evdir / "cert.yaml").write_text(
            "verdict: PASS\nmodel: demo\ndecode_tps_conc1: 45.28\nlite_included: true\n"
            "measured_node: main\n", encoding="utf-8")
        (evdir / "report.md").write_text("# 리포트\n\n" + REPORT_ROWS, encoding="utf-8")

        # (1) 인증서 + 리포트 모두 있는 정상 경로
        mp = _mk_manifest(evdir, "cert.yaml", "report.md")
        out1 = t2 / "out1"
        rc = cmd_collect(argparse.Namespace(repo=str(repo2), manifest=str(mp), topology="single",
                                            config_name=cfg, out=str(out1), generated_kst="2026-09-06 17:00"))
        ck("★cmd_collect 실경로가 돈다(rc=0)", rc == 0)
        pay1 = json.loads((out1 / PAYLOAD_JSON).read_text(encoding="utf-8"))
        # 픽스처 레포에는 `.claude/pii_terms.txt` 가 없다 — 그 부재도 **결손으로 적히는 것이 옳다**.
        # 여기서 기대를 넓히지 않고 실제 사유를 지목한다(합격 기준을 실물에 맞춘다).
        ck("결손이 사유코드로만 적힌다(정상 경로)",
           pay1.get("missing") == ["HINT_MISSING_PII_TERMS"])
        ck("★인증서·리포트가 있으면 그 둘은 결손이 아니다",
           "HINT_MISSING_CERTIFICATE" not in pay1["missing"]
           and "HINT_MISSING_BENCH_REPORT" not in pay1["missing"])
        b3 = (out1 / "03-benchmark.md").read_text(encoding="utf-8")
        ck("동시성별 표가 벤치 절에 실린다", "부하 스윕 곡선 — 동시성별" in b3 and "| 2 |" in b3)
        ck("판정점 표시가 표에 남는다", "★판정점" in b3)

        # (2) 인증서 부재 — **죽지 않고** 결손으로 기재한다(강행 발행)
        mp2 = _mk_manifest(evdir, None, "report.md")
        out2 = t2 / "out2"
        rc2 = cmd_collect(argparse.Namespace(repo=str(repo2), manifest=str(mp2), topology="single",
                                             config_name=cfg, out=str(out2), generated_kst="2026-09-06 17:00"))
        ck("★인증서 부재로 죽지 않는다", rc2 == 0)
        pay2 = json.loads((out2 / PAYLOAD_JSON).read_text(encoding="utf-8"))
        ck("★인증서 부재가 missing 에 적힌다", "HINT_MISSING_CERTIFICATE" in pay2.get("missing", []))
        ck("결손 정책 문장이 페이로드에 남는다", "forced_publication" in (pay2.get("missing_policy") or ""))
        ck("인증서 부재 절도 렌더된다",
           "인증서가 없다" in (out2 / "03-benchmark.md").read_text(encoding="utf-8"))

        # (3) 리포트 부재 — 곡선 없이 발행하되 사유를 적는다
        mp3 = _mk_manifest(evdir, "cert.yaml", None)
        out3 = t2 / "out3"
        rc3 = cmd_collect(argparse.Namespace(repo=str(repo2), manifest=str(mp3), topology="single",
                                             config_name=cfg, out=str(out3), generated_kst="2026-09-06 17:00"))
        ck("★리포트 부재로도 죽지 않는다", rc3 == 0)
        ck("★리포트 부재가 missing 에 적힌다",
           "HINT_MISSING_BENCH_REPORT" in json.loads((out3 / PAYLOAD_JSON).read_text(encoding="utf-8")).get("missing", []))

        # (4) 멀티 — 노드 정합 attestation 부재가 기재된다(이 분기가 NameError 로 죽던 자리)
        repo4 = _mk_tree(t2 / "repo4", "multi", cfg)
        mp4 = _mk_manifest(evdir, "cert.yaml", "report.md")
        out4 = t2 / "out4"
        rc4 = cmd_collect(argparse.Namespace(repo=str(repo4), manifest=str(mp4), topology="multi",
                                             config_name=cfg, out=str(out4), generated_kst="2026-09-06 17:00"))
        ck("★멀티 실경로가 돈다(attestation 분기 도달)", rc4 == 0)
        pay4 = json.loads((out4 / PAYLOAD_JSON).read_text(encoding="utf-8"))
        ck("★attestation 부재가 missing 에 적힌다",
           "HINT_MISSING_SLAVE_ATTESTATION" in pay4.get("missing", []))
        ck("attestation 포인터 칸이 있다", "node_parity_attestation" in pay4)
        # env 형상화는 compose 슬롯이 있을 때만 산출된다. 슬롯 유무와 무관하게 **커널은** 직접 시험한다 —
        # 조건부로 건너뛰면 그 조건이 거짓인 날 이 시험이 조용히 사라진다(역-오라클).
        tpl = env_shape_template("MASTER_HOST_IP=192.168.0.11\nRAY_PORT=6379\nSSH_USER=someone\n")  # pii-scan-fixture
        ck("★env 형상: 신원 키는 가려지고 튜닝 키는 남는다",
           "MASTER_HOST_IP=<manifest" in tpl and "RAY_PORT=6379" in tpl
           and "SSH_USER=<manifest" in tpl and "192.168." not in tpl)  # pii-scan-fixture

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
