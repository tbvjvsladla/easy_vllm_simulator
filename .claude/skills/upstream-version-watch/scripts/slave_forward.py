#!/usr/bin/env python3
"""slave_forward.py — 멀티 Ray worker(slave)에 넘길 build/serve 변수를 **실제 compose 참조에서 파생**한다.

왜 (2026-09-21 · plan_26092119 §4.6 · 코드맵 build_plane §3):
  slave 는 `--env-file .env.cluster`(Band2)만 받는다. 셀 env(.env.<config> · Band3)는 파일로 전파하지
  않으므로(policy:MODEL_TRIPLET_NO_SUB_PROPAGATION) 셀이 정한 값은 **ssh 명령 앞의 env prefix** 로만 slave 에
  닿는다. 그 prefix 목록이 compose 의 build-arg·보간 변수와 **따로 손으로** 자라는 동안 같은 형태의 침묵
  누락이 반복됐다 — BUILD_DOCKERFILE(07-24) → VLLM_PRETEND_VERSION(08-02) → SM12X_PORT·RAY_PORT·
  SLAVE_CONTAINER_NAME(08-14) → SRC_DEPS_AUTHORITY·BUILD_JOBS(08-15) → VLLM_VERSION(09-05) → PLE(09-09).
  빠질 때마다 "마스터만 변종 · slave 는 compose 기본값" 이 되어 같은 IMAGE_TAG 가 노드마다 다른 내용을 가졌다.
  처방은 목록을 또 늘리는 것이 아니라 **목록을 없애는 것**이다: 이 모듈이 스모크가 실제로 쓰는 compose 의
  `${VAR}` 참조에서 전달 집합을 파생한다. 새 build-arg 를 compose 에 더하면 코드 수정 없이 전달된다.
  (결과 대조 — 2026-09-05 "리스트를 늘리는 대신 결과를 대조한다" — 는 스모크의 빌드 후 대조·attestation 이
  계속 맡는다. 파생은 목록 실패를 줄이고, 대조는 남은 것을 잡는다.)

파생 규칙(단일 소유 — 스모크와 hint 가 **같은 함수**를 부른다):
  image_identity = x-gpu-common 의 `image:`·`build.dockerfile` 참조 + `build.args` 값의 참조 − NON_IDENTITY
  build_tuning   = NON_IDENTITY_BUILD_ARGS(BUILD_JOBS — 같은 산출물·다른 병렬도, 2026-08-15 서브 OOM)
  mount          = volumes 참조 · 값은 프로젝트 .env(materialize-env 산출) 우선, 없으면 셀 env
  cluster        = slave `container_name:` 참조 + slave environment 참조 중 Band2 렌더러(render_dockerfile.env_tier)가
                   소유한 키(.env.cluster 의 키) — 셀 env 가 덮어쓸 때만 넘긴다(마스터는 EF 가 EFC 를 이긴다)
  serve_env      = 그 밖의 slave/공통 참조(PLE mmap 등) · 셀 env 값
  NEVER_FORWARD  = CONFIG_FILE(slave Band2-only · plan_2026062811_2) — 어느 그룹에도 들지 않는다.
의미 정규화(X11 · 사람 결정 D-4): 첫 일치(옛 `val()` 과 같다) · **빈 값은 키째 생략**(빈 `SM12X_PORT=` 가
  compose `${SM12X_PORT:-0}` 을 빈 문자열로 덮는 사고 — 부재 = stock) · 인용 없이 `bash -lc '…'` 에 박히므로
  안전 문자 밖 값은 **fail-closed**(ForwardError). 옛 MOUNTVARS/PLEVARS 의 "모든 줄·빈 값 통과·공백 제거"
  의미는 이 정규화로 대체됐다.

CLI:
  slave_forward.py prefix --group G[,G] --cell-env EF --project-env PENV --compose C [--cluster-env EFC]
  slave_forward.py json   --cell-env EF --project-env PENV --compose C [--cluster-env EFC]
  slave_forward.py default --compose C --key K        # compose `${K:-기본값}` 의 기본값(없으면 rc 1)
  slave_forward.py --self-test
stdlib 전용 · import 부수효과 0(파일·git 읽기 없음) · 벽시계 없음.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import re
import subprocess
import sys
import tempfile
from dataclasses import asdict, dataclass
from pathlib import Path

# ── 닫힌 목록(tripwire · 정당 하드코딩 — 바꾸려면 리뷰가 강제된다) ─────────────────────────────
NON_IDENTITY_BUILD_ARGS = frozenset({"BUILD_JOBS"})   # 2026-08-15: 산출물 동일 · 병렬도만 다름(서브 OOM)
NEVER_FORWARD = frozenset({"CONFIG_FILE"})             # slave Band2-only(plan_2026062811_2) — 트리플렛 미수령
GROUPS = ("image_identity", "build_tuning", "cluster", "mount", "serve_env")
SLAVE_SERVICE = "vllm-slave-serve"
SAFE_VALUE = re.compile(r"[A-Za-z0-9._:/@+,=-]*")
_ENV_LINE = re.compile(r"([A-Za-z_][A-Za-z0-9_]*)=(.*)")
_REF = re.compile(r"(?<!\$)\$(?:\{([A-Za-z_][A-Za-z0-9_]*)|([A-Za-z_][A-Za-z0-9_]*))")
_NODE = re.compile(r"([A-Za-z0-9_.<-]+)\s*:(?:\s+(.*))?$")

# 사고 이력은 **데이터**다 — 스모크 주석에 흩어져 있던 날짜 박힌 "왜" 를 키에 붙여 hint(sub_recipe)까지 나른다.
WHY: dict[str, str] = {
    "IMAGE_TAG": "이미지 정체성의 이름 — slave 가 모르면 compose 기본 태그로 빌드·기동한다(plan_26062818 §S2.5 R10).",
    "BUILD_DOCKERFILE": "2026-07-24 Solar-Open2: slave build 가 --env-file EFC 만 받아 compose 기본 Dockerfile 로 "
                        "폴백 → 변종 트랙에서 slave 만 다른 Dockerfile 로 빌드됐다(기본값 콤보에서만 잠복).",
    "VLLM_REPO": "포크 핀 좌표 — 빠지면 slave 만 stock 저장소로 빌드된다(plan_26062818 §S2.5 R10).",
    "VLLM_REF": "포크/릴리스 ref — 빠지면 slave 가 compose 기본 ref 로 빌드된다(plan_26062818 §S2.5 R10).",
    "VLLM_PRETEND_VERSION": "2026-08-02: 비-semver 포크 태그의 setuptools_scm 우회값 — 빠지면 마스터만 빌드되고 "
                            "slave 는 같은 지점(ValueError)에서 죽는다.",
    "SM12X_PORT": "2026-08-14(plan_26081418 G-4): build_patches_src 소스 이식 변종 게이트 — 빠지면 마스터만 "
                  "이식본·slave 는 stock. 부재 = stock(빈 값을 흘리면 compose 기본값을 빈 문자열로 덮는다).",
    "SRC_DEPS_AUTHORITY": "2026-08-15(R3 포크 핀): 클론 소스트리 의존 권위 게이트 — 빠지면 마스터만 flashinfer "
                          "0.6.17·slave 0.6.16.post3(이 목록의 네 번째 전파 구멍).",
    "VLLM_VERSION": "2026-09-05: wheel 트랙 설치 버전 — 빠지면 slave 가 Dockerfile ARG 기본값(0.18.0)으로 빌드돼 "
                    "같은 IMAGE_TAG 가 노드마다 다른 엔진이 된다(다섯 번째 · compose build-arg 복원과 함께 생긴 구멍).",
    "BUILD_JOBS": "2026-08-15: 이미지 정체성은 아니지만 양 노드에 같아야 한다 — slave 만 기본 16 으로 컴파일하면 "
                  "거기서 OOM(하드다운 #2 가 서브였다).",
    "RAY_PORT": "2026-08-14(침묵 누락 3번째): 셀이 head 포트를 바꾸면 slave 는 기본 6379 로 join 을 시도해 "
                "nc -z 5분 대기 후 실패한다(클러스터 랑데부 주소의 절반). 과거 멀티 런은 전부 6379(=compose 기본값)라 "
                "우연히 일치했다(docs/simlog 26070213·26072500) — 콤보별 포트 분리(plan_26081310 §X2)의 첫 런에서 깨어나는 잠복 결함.",
    "SLAVE_CONTAINER_NAME": "2026-08-14: 폴백 이름이면 slave 협역 워치독 필터가 매칭 0 — 서브에서만 계층 2층이 "
                            "조용히 사라진다(devlog_26080212 ⑥ 부류 · 하드다운 #2 는 서브였다). 2026-08-02 에 이미 "
                            "'엉뚱한 이름으로 떠 있었고 아무도 몰랐다' 로 한 번 드러났다(그때는 마스터측만 교정).",
    "MASTER_HOST_IP": "Band2(.env.cluster) 키 — 셀이 덮어쓰면 마스터만 새 값을 본다(head 주소 불일치).",
    "SLAVE_HOST_IP": "Band2(.env.cluster) 키 — 셀이 덮어쓰면 slave 자기 IP 가 마스터 판단과 갈린다.",
    "NAS_MODEL_PATH": "결함#2b(plan_26070119): --env-file 을 쓰면 프로젝트 .env 자동 로드가 꺼져 compose 기본 "
                      "마운트로 폴백 → 모델 부재(serve 즉사). 셸 env 로 명시 주입한다.",
    "QUANT_MODEL_PATH": "결함#2b 와 같은 경로 — 양자화 모델 마운트원.",
    "TIKTOKEN_HOST_PATH": "결함#2b 와 같은 경로 — 오프라인 토크나이저 마운트원.",
    "PLE_MMAP_HOST_PATH": "2026-09-09(camp-26090918): PLE mmap 로컬 NVMe 스테이징 마운트원 — slave 도 같은 경로에 실재해야 한다.",
    "VLLM_PLE_MMAP": "2026-09-09(62-qwen4exp-ple-mmap): Ray 워커도 가중치를 올린다 — 마스터만 켜면 slave 는 상주 "
                     "로드로 OOM/불일치.",
    "VLLM_PLE_MMAP_DIR": "2026-09-09: 워커 로컬 mmap 스테이징 경로 — 마스터와 같은 값이어야 한다.",
}
_WHY_DERIVED = "compose 참조에서 자동 파생(기록된 사고 이력 없음 — 새 변수)"


class ForwardError(ValueError):
    """파생 불가·안전하지 않은 값·compose 모양 위반. code 로만 판정한다."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(f"{code}: {message}")
        self.code = code
        self.message = message


@dataclass(frozen=True)
class Forward:
    key: str
    value: str
    group: str          # GROUPS 중 하나
    source: str         # "cell_env" | "project_env"
    ref: str = ""       # compose 안의 참조 자리(예 "x-gpu-common.build.args") — hint 해설용


# ── env 파일 ────────────────────────────────────────────────────────────────────────────────
def read_env_first(path) -> dict:
    """스모크 `val()`(= `grep -E "^KEY=" | head -1 | cut -d= -f2-`)과 같은 의미: **첫 일치** · 첫 '=' 뒤 원문.

    공백 제거·따옴표/주석 해석을 하지 않는다(원문 그대로 — 안전성은 SAFE_VALUE 가 판정한다). 줄머리 공백이 있는
    줄은 grep `^KEY=` 이 잡지 않으므로 여기서도 잡지 않는다. 부재 파일 = 빈 dict(호출부가 그룹별로 판단)."""
    if path is None:
        return {}
    p = Path(path)
    if not p.is_file():
        return {}
    out: dict = {}
    for raw in p.read_text(encoding="utf-8", errors="replace").splitlines():
        m = _ENV_LINE.fullmatch(raw)
        if m and m.group(1) not in out:
            out[m.group(1)] = m.group(2)
    return out


# ── compose 파서(무의존 · 이 저장소 compose 의 블록 매핑·시퀀스만) ─────────────────────────────
def _strip_comment(line: str) -> str:
    out, quote, prev = [], None, " "
    for ch in line:
        if quote:
            out.append(ch)
            if ch == quote:
                quote = None
        elif ch in "\"'":
            quote = ch
            out.append(ch)
        elif ch == "#" and prev in " \t":
            break
        else:
            out.append(ch)
        prev = ch
    return "".join(out).rstrip()


def _tree(text: str) -> dict:
    root = {"key": None, "value": "", "children": [], "indent": -1, "item": False}
    stack = [root]
    for raw in text.splitlines():
        line = _strip_comment(raw)
        if not line.strip():
            continue
        indent = len(line) - len(line.lstrip(" "))
        body = line.strip()
        item = body == "-" or body.startswith("- ")
        if item:
            body = body[1:].strip()
        m = _NODE.fullmatch(body) if body else None
        node = {"key": m.group(1) if m else None, "value": ((m.group(2) or "") if m else body).strip(),
                "children": [], "indent": indent, "item": item}
        while stack[-1]["indent"] >= indent:
            stack.pop()
        stack[-1]["children"].append(node)
        stack.append(node)
    return root


def _child(node: dict | None, key: str) -> dict | None:
    if node is None:
        return None
    return next((c for c in node["children"] if c["key"] == key and not c["item"]), None)


def _texts(node: dict):
    yield node["value"]
    for c in node["children"]:
        yield from _texts(c)


def _refs(*texts: str) -> list:
    out: list = []
    for text in texts:
        for m in _REF.finditer(text or ""):
            name = m.group(1) or m.group(2)
            if name not in out:
                out.append(name)
    return out


def compose_refs(compose_path) -> dict:
    """compose 의 공통 앵커(x-gpu-common)와 slave 서비스가 참조하는 `${VAR}` 를 자리별로 돌려준다.

    모양 위반(앵커·slave 서비스 부재, slave 가 앵커를 병합하지 않음)은 ForwardError — 파생의 전제가 깨진 채
    빈 목록을 돌려주면 "전달할 것이 없다" 로 읽혀 침묵 누락이 된다."""
    path = Path(compose_path)
    if not path.is_file():
        raise ForwardError("SF_COMPOSE_ABSENT", f"compose 파일이 없다: {path}")
    tree = _tree(path.read_text(encoding="utf-8"))
    common = next((c for c in tree["children"] if c["key"] and c["key"].startswith("x-")
                   and c["value"].startswith("&")), None)
    if common is None:
        raise ForwardError("SF_COMPOSE_SHAPE", "공통 앵커 블록(x-…: &name)을 찾지 못했다")
    anchor = common["value"][1:].strip()
    slave = _child(_child(tree, "services"), SLAVE_SERVICE)
    if slave is None:
        raise ForwardError("SF_COMPOSE_SHAPE", f"services.{SLAVE_SERVICE} 가 없다")
    merge = _child(slave, "<<")
    if merge is None or merge["value"] != f"*{anchor}":
        raise ForwardError("SF_COMPOSE_SHAPE", f"{SLAVE_SERVICE} 가 공통 앵커 *{anchor} 를 병합하지 않는다")

    build = _child(common, "build")
    selectors = _refs(_child(common, "image")["value"] if _child(common, "image") else "",
                      _child(build, "dockerfile")["value"] if _child(build, "dockerfile") else "")
    args_node = _child(build, "args")
    build_args = _refs(*[c["value"] for c in (args_node["children"] if args_node else [])])
    volumes = _refs(*[t for n in (_child(common, "volumes"), _child(slave, "volumes")) if n for t in _texts(n)])
    container = _refs(_child(slave, "container_name")["value"] if _child(slave, "container_name") else "")
    env_node = _child(slave, "environment")
    environment = _refs(*[t for t in (_texts(env_node) if env_node else [])])
    env_files = []
    ef_node = _child(slave, "env_file")
    for c in (ef_node["children"] if ef_node else []):
        env_files.append(c["value"] if c["key"] is None else (c["value"] if c["key"] == "path" else ""))
    known = set(selectors) | set(build_args) | set(volumes) | set(container) | set(environment)
    other = [r for r in _refs(*_texts(common), *_texts(slave)) if r not in known]
    return {"anchor": anchor, "selectors": selectors, "build_args": build_args, "volumes": volumes,
            "slave_container": container, "slave_environment": environment, "slave_env_files": env_files,
            "other": other}


def compose_default(compose_path, key: str) -> str | None:
    """compose 안 첫 `${KEY:-기본값}`(또는 `${KEY-기본값}`)의 기본값 — 중첩 `{{ }}` 도 균형 괄호로 읽는다.

    스모크의 옛 하드 폴백 `_bld_ver="0.27.1"`(K10 — compose 기본값은 이미 v0.29.0rc6 였다)을 대체한다.
    **주석은 참조가 아니다**(compose_refs 와 같은 규칙) — 주석에 옛 기본값을 인용한 줄이 있으면 그것을 기본값으로
    읽는 순간 K10 과 같은 거짓 대조가 된다. 한 줄 안에서만 괄호 균형을 본다(보간식은 줄을 넘지 않는다)."""
    path = Path(compose_path)
    if not path.is_file():
        raise ForwardError("SF_COMPOSE_ABSENT", f"compose 파일이 없다: {path}")
    head = re.compile(r"(?<!\$)\$\{" + re.escape(key) + r":?-")
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = _strip_comment(raw)
        m = head.search(line)
        if not m:
            continue
        depth, i = 1, m.end()
        for j in range(i, len(line)):
            if line[j] == "{":
                depth += 1
            elif line[j] == "}":
                depth -= 1
                if depth == 0:
                    return line[i:j]
        raise ForwardError("SF_COMPOSE_SHAPE", f"${{{key}:-…}} 의 괄호가 한 줄 안에서 닫히지 않는다: {raw.strip()!r}")
    return None


# ── Band2 소유 키(렌더러에서 파생) ─────────────────────────────────────────────────────────────
_RENDER_CACHE: dict = {}


def _band2_owned(key: str) -> bool:
    """render_dockerfile.env_tier 가 층을 아는 키 = `.env.cluster`/`.env.interconnect` 렌더러가 소유한 키."""
    mod = _RENDER_CACHE.get("mod")
    if mod is None:
        path = Path(__file__).resolve().parent / "render_dockerfile.py"
        spec = importlib.util.spec_from_file_location("_slave_forward_render_dockerfile", path)
        if spec is None or spec.loader is None or not path.is_file():
            raise ForwardError("SF_RENDERER_ABSENT", f"Band2 렌더러가 없다: {path}")
        mod = importlib.util.module_from_spec(spec)
        try:
            spec.loader.exec_module(mod)
        except (ImportError, SyntaxError, OSError) as exc:
            raise ForwardError("SF_RENDERER_ABSENT", f"Band2 렌더러 적재 실패: {path}: {type(exc).__name__}: {exc}") from exc
        if not callable(getattr(mod, "env_tier", None)):
            raise ForwardError("SF_RENDERER_ABSENT", "render_dockerfile.env_tier(key) 공개 API 가 없다")
        _RENDER_CACHE["mod"] = mod
    return _tier(mod, key)[0] != "unknown"


def _tier(mod, key: str) -> tuple:
    """env_tier 호출 — 렌더러의 fail-loud(KeyError·ValueError: 프리셋 표 모양 위반)를 사유 코드로 번역한다.
    호출부(스모크 `|| exit 3` · hint artifacts)는 code 로만 판정한다 — 날것의 예외가 새면 hint 가 추적으로 죽는다."""
    try:
        got = mod.env_tier(key)
    except (KeyError, ValueError) as exc:
        raise ForwardError("SF_ENV_TIER_FAILED", f"render_dockerfile.env_tier({key!r}) 실패: {exc}") from exc
    if not (isinstance(got, tuple) and len(got) == 2):
        raise ForwardError("SF_ENV_TIER_FAILED", f"render_dockerfile.env_tier({key!r}) 반환이 (tier, detail) 가 아니다: {got!r}")
    return got


def candidates(compose_path, cluster_env=None) -> dict:
    """그룹 → 후보 키(값과 무관한 compose 구조 파생). derive·자체검사 tripwire 가 같은 분류를 쓴다."""
    refs = compose_refs(compose_path)
    band2_file = set(read_env_first(cluster_env)) if cluster_env else set()
    out = {g: [] for g in GROUPS}
    seen: set = set()

    def put(group: str, key: str) -> None:
        if key in NEVER_FORWARD or key in seen:
            return
        seen.add(key)
        out[group].append(key)

    for key in [*refs["selectors"], *refs["build_args"]]:
        put("build_tuning" if key in NON_IDENTITY_BUILD_ARGS else "image_identity", key)
    for key in refs["volumes"]:
        put("mount", key)
    for key in refs["slave_container"]:
        put("cluster", key)
    for key in refs["slave_environment"]:
        put("cluster" if (key in band2_file or _band2_owned(key)) else "serve_env", key)
    for key in refs["other"]:
        put("serve_env", key)
    return out


def _ref_place(group: str) -> str:
    return {"image_identity": "x-gpu-common.image|build.dockerfile|build.args",
            "build_tuning": "x-gpu-common.build.args", "mount": "volumes",
            "cluster": f"services.{SLAVE_SERVICE}.container_name|environment(Band2 키)",
            "serve_env": f"services.{SLAVE_SERVICE}.environment"}[group]


def derive(cell_env, project_env, compose_path, cluster_env=None) -> list:
    """slave 로 넘길 Forward 목록(그룹 순서 = compose 참조 순서). 안전하지 않은 값 = ForwardError."""
    groups = candidates(compose_path, cluster_env)
    cell = read_env_first(cell_env)
    project = read_env_first(project_env)
    rows: list = []
    for group in ("image_identity", "build_tuning", "mount", "cluster", "serve_env"):
        for key in groups[group]:
            if group == "mount":
                # 마스터 실효값 = 셸 env(프로젝트 .env) > EF > EFC — slave 에도 같은 값을 준다.
                value, source = (project.get(key), "project_env") if project.get(key) else (cell.get(key), "cell_env")
            else:
                # EF 가 정한 값만 넘긴다: EFC 키는 slave 도 --env-file 로 이미 받고, 부재는 compose 기본값(=stock)이다.
                value, source = cell.get(key), "cell_env"
            if not value:
                continue            # 빈 값·부재 = 키째 생략(빈 대입은 compose 기본값을 빈 문자열로 덮는다)
            if not SAFE_VALUE.fullmatch(value):
                raise ForwardError("SF_UNSAFE_VALUE",
                                   f"{key}={value!r} 는 인용 없는 원격 셸 prefix 에 안전하지 않다"
                                   f"(허용 문자 {SAFE_VALUE.pattern}) — {source} 를 고쳐라")
            rows.append(Forward(key, value, group, source, _ref_place(group)))
    return rows


def _check_groups(groups) -> tuple:
    names = tuple(g for g in groups if g)
    unknown = [g for g in names if g not in GROUPS]
    if unknown or not names:
        raise ForwardError("SF_GROUP_UNKNOWN", f"그룹 {unknown or '(없음)'} — 허용 {list(GROUPS)}")
    return names


def prefix(forwards, *groups: str) -> str:
    """`KEY=VALUE KEY2=VALUE2`(공백 구분 · 인용 없음 — 값은 derive 가 SAFE_VALUE 로 이미 검증)."""
    wanted = set(_check_groups(groups))
    return " ".join(f"{f.key}={f.value}" for f in forwards if f.group in wanted)


def _value_shape(f: Forward) -> str:
    """hint 에 싣는 값 형상 — 운영자 절대경로·호스트 값은 manifest 자리표시로 바꾼다."""
    if f.group == "mount":
        # materialize_env 가 manifest.<key.lower()> 를 이 키로 방출한다(render_dockerfile.materialize_env).
        return f"<manifest.{f.key.lower()}>"
    if f.group == "cluster" and _band2_owned(f.key):
        tier, detail = _tier(_RENDER_CACHE["mod"], f.key)
        if tier == "env":
            return f"<manifest.{detail}>"
    return f.value


def as_sub_recipe(forwards) -> list:
    """hint `artifacts/compose/sub_recipe.json` 의 env_forward 행 — 원값 대신 형상을 싣는다."""
    return [{"key": f.key, "group": f.group, "from": f.source, "compose_ref": f.ref,
             "why": WHY.get(f.key, _WHY_DERIVED), "value_shape": _value_shape(f)} for f in forwards]


# ── 자체검사 ────────────────────────────────────────────────────────────────────────────────
# 옛 스모크(HEAD a9b6cdd 의 multinode_serve_smoke.sh:82·118-156·253-254)의 파생 셸 — **시험 평면의 대조군**이다
# (4종 판정표 모킹-정당: 프로덕션 산출물과 같은 모양으로 나가지 않는다). 새 파생이 오늘의 전달 집합을
# 그대로 재현하는지 이것과 실행 대조한다.
_LEGACY_SHELL = r'''
val(){ grep -E "^$1=" "$EF" | head -1 | cut -d= -f2-; }
IMG=$(val IMAGE_TAG); VREPO=$(val VLLM_REPO); VREF=$(val VLLM_REF); BDF=$(val BUILD_DOCKERFILE)
VPV=$(val VLLM_PRETEND_VERSION); SMPORT=$(val SM12X_PORT); SDA=$(val SRC_DEPS_AUTHORITY)
VVER=$(val VLLM_VERSION)
SLAVE_IMGVARS="${IMG:+IMAGE_TAG=$IMG }${BDF:+BUILD_DOCKERFILE=$BDF }${VVER:+VLLM_VERSION=$VVER }${VREPO:+VLLM_REPO=$VREPO }${VPV:+VLLM_PRETEND_VERSION=$VPV }${SMPORT:+SM12X_PORT=$SMPORT }${SDA:+SRC_DEPS_AUTHORITY=$SDA }${VREF:+VLLM_REF=$VREF}"
RAYP=$(val RAY_PORT); SLVC=$(val SLAVE_CONTAINER_NAME)
SLAVE_CLUSTERVARS="${RAYP:+RAY_PORT=$RAYP }${SLVC:+SLAVE_CONTAINER_NAME=$SLVC}"
SLAVE_IMGVARS="$SLAVE_IMGVARS $SLAVE_CLUSTERVARS"
MOUNTVARS=""
[ -f "$PENV_FILE" ] && MOUNTVARS="$(grep -E '^(NAS_MODEL_PATH|QUANT_MODEL_PATH|TIKTOKEN_HOST_PATH|PLE_MMAP_HOST_PATH)=' "$PENV_FILE" | tr '\n' ' ')"
PLEVARS="$(grep -E '^VLLM_PLE_MMAP(_DIR)?=' "$EF" 2>/dev/null | tr -d ' ' | tr '\n' ' ')"
BJOBS=$(val BUILD_JOBS)
SLAVE_BUILDVARS="${BJOBS:+BUILD_JOBS=$BJOBS}"
printf 'IMG=%s\nBUILD=%s\nMOUNT=%s\nPLE=%s\n' "$SLAVE_IMGVARS" "$SLAVE_BUILDVARS" "$MOUNTVARS" "$PLEVARS"
'''

# 오늘(2026-09-21)의 전달 집합 — tripwire. compose 에 참조가 늘거나 줄면 여기서 울린다: 새 키가 어느 그룹에
# 들어가는지 사람이 확인하고(WHY 한 줄 포함) 이 표를 고친다. 파생 자체는 코드 수정 없이 이미 새 키를 넘긴다.
_TRIPWIRE_GROUPS = {
    "image_identity": ["IMAGE_TAG", "BUILD_DOCKERFILE", "VLLM_VERSION", "VLLM_REPO", "VLLM_REF",
                       "VLLM_PRETEND_VERSION", "SM12X_PORT", "SRC_DEPS_AUTHORITY"],
    "build_tuning": ["BUILD_JOBS"],
    "cluster": ["SLAVE_CONTAINER_NAME", "SLAVE_HOST_IP", "MASTER_HOST_IP", "RAY_PORT"],
    "mount": ["NAS_MODEL_PATH", "QUANT_MODEL_PATH", "TIKTOKEN_HOST_PATH", "PLE_MMAP_HOST_PATH"],
    "serve_env": ["VLLM_PLE_MMAP", "VLLM_PLE_MMAP_DIR"],
}
# 태그2 셀(nv4-bf-262k-mmp · camp-26091216)의 셀 env 형상(운영자 값 없음 — 이름·태그·ref 뿐).
_TAG2_CELL_ENV = """# fixture
COMPOSE_PROJECT_NAME=vllm_nv4-bf-262k-mmp_project
CONTAINER_NAME=nv4-bf-262k-mmp-serving-container
MASTER_CONTAINER_NAME=nv4-bf-262k-mmp-master-container
SLAVE_CONTAINER_NAME=nv4-bf-262k-mmp-slave-container
SERVING_PORT=8080
SERVING_MODEL_NAME=qwen3.8-flash-next
CONFIG_FILE=nv4-bf-262k-mmp
IMAGE_TAG=easy-vllm:0.29.0rc6-cu133-aarch64-source
BUILD_DOCKERFILE=Dockerfile.source-build
VLLM_REF=v0.29.0rc6
BUILD_JOBS=8
VLLM_PLE_MMAP=1
VLLM_PLE_MMAP_DIR=/app/ple_mmap/q38fn-nvfp4
"""
_PROJECT_ENV = ("NAS_MODEL_PATH=/data/models\nQUANT_MODEL_PATH=/data/quant\n"
                "TIKTOKEN_HOST_PATH=/data/tiktoken\nPLE_MMAP_HOST_PATH=/data/ple\n")
_FIXTURE_COMPOSE = """x-gpu-common: &gpu-common
  image: ${IMAGE_TAG:-easy-vllm:{{ IMAGE_TAG }}}
  build:
    context: .
    dockerfile: ${BUILD_DOCKERFILE:-Dockerfile.source-build}   # 주석 ${NOT_A_REF}
    args:
      VLLM_VERSION: ${VLLM_VERSION:-0.18.0}
      VLLM_REPO: ${VLLM_REPO:-https://example.invalid/vllm.git}
      VLLM_REF: ${VLLM_REF:-v0.29.0rc6}
      VLLM_PRETEND_VERSION: ${VLLM_PRETEND_VERSION:-}
      SM12X_PORT: ${SM12X_PORT:-0}
      SRC_DEPS_AUTHORITY: ${SRC_DEPS_AUTHORITY:-0}
      LITERAL_ARG: fixed
      BUILD_JOBS: ${BUILD_JOBS:-16}
  volumes:
    - ${NAS_MODEL_PATH:-/data/x}:/app/models:ro
    - ${QUANT_MODEL_PATH:-/data/x}:/app/quant_models:ro
    - ${TIKTOKEN_HOST_PATH:-/data/x}:/encodings:ro
    - ${PLE_MMAP_HOST_PATH:-/data/x}:/app/ple_mmap:ro
    - ./configs:/app/configs:ro
services:
  vllm-master-serve:
    <<: *gpu-common
    container_name: ${MASTER_CONTAINER_NAME:-m}
    environment:
      CONFIG_FILE: ${CONFIG_FILE:-default}
      MASTER_ONLY: ${MASTER_ONLY:-x}
  vllm-slave-serve:
    <<: *gpu-common
    profiles: [slave]
    container_name: ${SLAVE_CONTAINER_NAME:-vllm-slave-serve-container}
    environment:
      NODE_ROLE: slave
      VLLM_HOST_IP: ${SLAVE_HOST_IP}
      HEAD_NODE_IP: ${MASTER_HOST_IP}
      RAY_PORT: ${RAY_PORT:-6379}
      CONFIG_FILE: ${CONFIG_FILE:-default}
      ESCAPED: $${NOT_A_REF_EITHER}
      VLLM_PLE_MMAP: ${VLLM_PLE_MMAP:-0}
      VLLM_PLE_MMAP_DIR: ${VLLM_PLE_MMAP_DIR:-/app/ple_mmap/q38fn-nvfp4}
    env_file:
      - envs/.env.interconnect
      - envs/.env.cluster
"""


def _legacy(cell_text: str, project_text: str | None) -> dict:
    """옛 셸 파생을 bash 로 실행해 그룹별 토큰 집합을 돌려준다(대조군)."""
    with tempfile.TemporaryDirectory() as td:
        ef = Path(td) / "cell.env"
        ef.write_text(cell_text, encoding="utf-8")
        penv = Path(td) / "project.env"
        if project_text is not None:
            penv.write_text(project_text, encoding="utf-8")
        script = f'EF="{ef}"; PENV_FILE="{penv}"\n{_LEGACY_SHELL}'
        proc = subprocess.run(["bash", "-c", script], capture_output=True, text=True, timeout=30)
        if proc.returncode != 0:
            raise ForwardError("SF_LEGACY_ORACLE", proc.stderr[-400:])
        out = {}
        for line in proc.stdout.splitlines():
            k, _, v = line.partition("=")
            out[k] = set(v.split())
        return out


def selftest() -> list:
    failures: list = []

    def ck(name: str, cond: bool) -> None:
        if not cond:
            failures.append(f"slave_forward: {name}")

    def expect_error(name: str, code: str, fn) -> None:
        try:
            fn()
        except ForwardError as exc:
            ck(f"{name}(code={exc.code})", exc.code == code)
            return
        ck(f"{name} — 차단되지 않았다", False)

    with tempfile.TemporaryDirectory(prefix="slave-forward-") as td:
        root = Path(td)
        compose = root / "docker-compose.yaml"
        compose.write_text(_FIXTURE_COMPOSE, encoding="utf-8")
        cell = root / ".env.cell"
        project = root / ".env"
        project.write_text(_PROJECT_ENV, encoding="utf-8")

        # 1) 구조 파생 — 픽스처 compose 의 분류가 오늘의 tripwire 표와 같다(LITERAL_ARG·주석·$$·마스터 전용 참조 제외).
        groups = candidates(compose)
        ck("구조 파생 = tripwire 표", groups == _TRIPWIRE_GROUPS)
        ck("★CONFIG_FILE 은 어느 그룹에도 없다", all("CONFIG_FILE" not in v for v in groups.values()))
        ck("주석·$$ 이스케이프·마스터 전용 참조는 참조가 아니다",
           not {"NOT_A_REF", "NOT_A_REF_EITHER", "MASTER_ONLY", "MASTER_CONTAINER_NAME"}
           & {k for v in groups.values() for k in v})

        # 2) 태그2 셀 — 오늘의 전달 집합을 옛 셸과 실행 대조.
        cell.write_text(_TAG2_CELL_ENV, encoding="utf-8")
        fw = derive(cell, project, compose)
        legacy = _legacy(_TAG2_CELL_ENV, _PROJECT_ENV)
        ck("태그2 image_identity+cluster = 옛 SLAVE_IMGVARS",
           set(prefix(fw, "image_identity", "cluster").split()) == legacy["IMG"])
        ck("태그2 build_tuning = 옛 SLAVE_BUILDVARS", set(prefix(fw, "build_tuning").split()) == legacy["BUILD"])
        ck("태그2 mount = 옛 MOUNTVARS", set(prefix(fw, "mount").split()) == legacy["MOUNT"])
        ck("태그2 serve_env = 옛 PLEVARS", set(prefix(fw, "serve_env").split()) == legacy["PLE"])
        ck("태그2 image_identity 정확값", prefix(fw, "image_identity") ==
           "IMAGE_TAG=easy-vllm:0.29.0rc6-cu133-aarch64-source BUILD_DOCKERFILE=Dockerfile.source-build "
           "VLLM_REF=v0.29.0rc6")

        # 3) 옛 술어 벡터 (a)~(d) + 빈 값 · 첫 일치 — 옛 셸과 같은 결과.
        base = ("IMAGE_TAG=easy-vllm:0.25.1-cu132-aarch64-source-sm12x\n"
                "VLLM_REPO=https://github.com/jasl/vllm.git\nVLLM_REF=b5c0d43b967c\n"
                "BUILD_DOCKERFILE=Dockerfile.source-build\nVLLM_PRETEND_VERSION=0.26.1\n")
        vectors = {"(a) 변종 SM12X_PORT=1": base + "SM12X_PORT=1\n",
                   "(b) stock 부재": base,
                   "(b') 빈 SM12X_PORT=": base + "SM12X_PORT=\n",
                   "(c) wheel VLLM_VERSION": "IMAGE_TAG=easy-vllm:0.19.0-cu130-aarch64-wheel\n"
                                             "BUILD_DOCKERFILE=Dockerfile\nVLLM_VERSION=0.19.0\n",
                   "(d) SRC_DEPS_AUTHORITY=1": base + "SRC_DEPS_AUTHORITY=1\n",
                   "첫 일치(중복 키)": base + "SM12X_PORT=1\nSM12X_PORT=0\n",
                   "RAY_PORT 셀 덮어쓰기": base + "RAY_PORT=6383\n"}
        for name, text in vectors.items():
            cell.write_text(text, encoding="utf-8")
            got = set(prefix(derive(cell, project, compose), "image_identity", "cluster").split())
            ck(f"{name} = 옛 셸", got == _legacy(text, _PROJECT_ENV)["IMG"])
        cell.write_text(base, encoding="utf-8")
        ck("★(b) 부재는 키째 생략", "SM12X_PORT" not in prefix(derive(cell, project, compose), "image_identity"))
        cell.write_text(base + "SM12X_PORT=\n", encoding="utf-8")
        ck("★(b') 빈 값은 빈 대입으로 새지 않는다",
           "SM12X_PORT" not in prefix(derive(cell, project, compose), "image_identity"))

        # 4) ★(e) 새 build-arg 가 코드 수정 없이 전달된다 · 다른 이름의 변수를 참조하면 그 변수를 넘긴다.
        compose.write_text(_FIXTURE_COMPOSE.replace(
            "      LITERAL_ARG: fixed\n",
            "      LITERAL_ARG: fixed\n      NEW_IDENTITY_ARG: ${NEW_IDENTITY_ARG:-0}\n"
            "      RENAMED_ARG: ${OTHER_NAME_VAR:-x}\n"), encoding="utf-8")
        cell.write_text(base + "NEW_IDENTITY_ARG=1\nOTHER_NAME_VAR=y\nCONFIG_FILE=cell\n", encoding="utf-8")
        got = {f.key: f for f in derive(cell, project, compose)}
        ck("★(e) 새 build-arg 자동 전달", got.get("NEW_IDENTITY_ARG") and got["NEW_IDENTITY_ARG"].group == "image_identity")
        ck("build-arg 가 참조하는 변수 이름을 넘긴다", "OTHER_NAME_VAR" in got and "RENAMED_ARG" not in got)
        ck("★CONFIG_FILE 은 셀이 정해도 넘기지 않는다", "CONFIG_FILE" not in got)
        compose.write_text(_FIXTURE_COMPOSE, encoding="utf-8")

        # 5) 마운트 — 프로젝트 .env 우선 · 없으면 셀 env · 형상은 manifest 자리표시.
        cell.write_text(base + "NAS_MODEL_PATH=/data/cell-models\n", encoding="utf-8")
        fwm = {f.key: f for f in derive(cell, project, compose)}
        ck("마운트는 프로젝트 .env 가 이긴다", fwm["NAS_MODEL_PATH"].value == "/data/models"
           and fwm["NAS_MODEL_PATH"].source == "project_env")
        fwn = {f.key: f for f in derive(cell, root / "absent.env", compose)}
        ck("프로젝트 .env 가 없으면 셀 env 값", fwn["NAS_MODEL_PATH"].source == "cell_env")
        recipe = {r["key"]: r for r in as_sub_recipe(derive(cell, project, compose))}
        ck("sub_recipe 는 마운트 원값을 싣지 않는다", recipe["NAS_MODEL_PATH"]["value_shape"] == "<manifest.nas_model_path>")
        ck("sub_recipe why 는 사고 이력", recipe["BUILD_DOCKERFILE"]["why"].startswith("2026-07-24"))

        # 6) ★ 안전하지 않은 값 · 모양 위반 · 모르는 그룹 = 차단.
        cell.write_text(base + "VLLM_PLE_MMAP_DIR=/data/with space\n", encoding="utf-8")
        expect_error("★공백 값 차단", "SF_UNSAFE_VALUE", lambda: derive(cell, project, compose))
        cell.write_text(base.replace("VLLM_REF=b5c0d43b967c", "VLLM_REF=v1$(id)"), encoding="utf-8")
        expect_error("★셸 메타문자 차단", "SF_UNSAFE_VALUE", lambda: derive(cell, project, compose))
        cell.write_text(base, encoding="utf-8")
        bad = root / "bad.yaml"
        bad.write_text(_FIXTURE_COMPOSE.replace("  vllm-slave-serve:\n    <<: *gpu-common\n",
                                                "  vllm-slave-serve:\n"), encoding="utf-8")
        expect_error("★slave 가 앵커를 병합하지 않음", "SF_COMPOSE_SHAPE", lambda: derive(cell, project, bad))
        bad.write_text(_FIXTURE_COMPOSE.replace("vllm-slave-serve:", "vllm-other:"), encoding="utf-8")
        expect_error("★slave 서비스 부재", "SF_COMPOSE_SHAPE", lambda: derive(cell, project, bad))
        expect_error("★compose 부재", "SF_COMPOSE_ABSENT", lambda: derive(cell, project, root / "none.yaml"))
        expect_error("★모르는 그룹", "SF_GROUP_UNKNOWN", lambda: prefix([], "imgvars"))

        class _BrokenRenderer:          # 렌더러 fail-loud(프리셋 표 모양 위반) → 사유 코드로 번역되어야 한다
            @staticmethod
            def env_tier(key):
                raise KeyError("no common preset")

        class _ShapelessRenderer:
            @staticmethod
            def env_tier(key):
                return "env"
        expect_error("★env_tier 예외는 사유 코드로", "SF_ENV_TIER_FAILED", lambda: _tier(_BrokenRenderer, "RAY_PORT"))
        expect_error("★env_tier 반환 모양 위반", "SF_ENV_TIER_FAILED", lambda: _tier(_ShapelessRenderer, "RAY_PORT"))

        # 7) compose_default — 중첩 `{{ }}` 균형 · 부재 · ★주석 인용은 기본값이 아니다(K10 재발 방지).
        ck("compose_default 단순", compose_default(compose, "VLLM_REF") == "v0.29.0rc6")
        ck("compose_default 중첩 괄호", compose_default(compose, "IMAGE_TAG") == "easy-vllm:{{ IMAGE_TAG }}")
        ck("compose_default 빈 기본값", compose_default(compose, "VLLM_PRETEND_VERSION") == "")
        ck("compose_default 부재", compose_default(compose, "SLAVE_HOST_IP") is None)
        commented = root / "commented.yaml"
        commented.write_text("# 옛 기본값 ${VLLM_REF:-v0.27.1} 은 주석이다\n" + _FIXTURE_COMPOSE, encoding="utf-8")
        ck("★compose_default 는 주석 속 옛 기본값을 읽지 않는다", compose_default(commented, "VLLM_REF") == "v0.29.0rc6")
        expect_error("★compose_default compose 부재", "SF_COMPOSE_ABSENT",
                     lambda: compose_default(root / "none.yaml", "VLLM_REF"))

        # 8) CLI — 스모크가 실제로 부르는 경로(`prefix --group a,b` · `default`)가 API 와 같은 값을 낸다.
        cell.write_text(_TAG2_CELL_ENV, encoding="utf-8")
        cli = [sys.executable, str(Path(__file__).resolve())]
        common = ["--cell-env", str(cell), "--project-env", str(project), "--compose", str(compose)]
        proc = subprocess.run(cli + ["prefix", "--group", "image_identity,cluster", *common],
                              capture_output=True, text=True, timeout=60)
        ck(f"CLI prefix rc=0({proc.stderr.strip()[-200:]})", proc.returncode == 0)
        ck("CLI prefix = API prefix", proc.stdout.strip() ==
           prefix(derive(cell, project, compose), "image_identity", "cluster"))
        proc = subprocess.run(cli + ["default", "--compose", str(compose), "--key", "VLLM_REF"],
                              capture_output=True, text=True, timeout=60)
        ck("CLI default", proc.returncode == 0 and proc.stdout.strip() == "v0.29.0rc6")
        proc = subprocess.run(cli + ["default", "--compose", str(compose), "--key", "NO_SUCH_KEY"],
                              capture_output=True, text=True, timeout=60)
        ck("★CLI default 부재 = rc 1(빈 값으로 통과시키지 않는다)", proc.returncode == 1 and not proc.stdout.strip())
        cell.write_text(_TAG2_CELL_ENV + "VLLM_REPO=https://x.invalid/a b\n", encoding="utf-8")
        proc = subprocess.run(cli + ["prefix", "--group", "image_identity", *common],
                              capture_output=True, text=True, timeout=60)
        ck("★CLI 안전하지 않은 값 = rc 2 · stdout 비어 있음(스모크 `|| exit 3` 가 잡는다)",
           proc.returncode == 2 and not proc.stdout.strip() and "SF_UNSAFE_VALUE" in proc.stderr)

    # 9) tripwire — 추적 템플릿(양 브랜치 공통)과 추적 렌더(multi 브랜치에만)의 구조가 오늘의 표와 같다.
    skill = Path(__file__).resolve().parents[1]
    template = skill / "templates" / "docker-compose.multi.template.yaml"
    ck("멀티 compose 템플릿 실재", template.is_file())
    if template.is_file():
        ck("★템플릿 구조 = tripwire 표(새 참조면 WHY·표를 사람이 갱신)", candidates(template) == _TRIPWIRE_GROUPS)
    tracked = skill.parents[2] / "output" / "multi" / "docker-compose.yaml"
    if tracked.is_file():
        ck("추적 렌더 구조 = tripwire 표(템플릿↔렌더 드리프트 K6)", candidates(tracked) == _TRIPWIRE_GROUPS)
    return failures


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="slave 전달 변수 파생(compose 참조 단일 소유)")
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    for name in ("prefix", "json"):
        q = sub.add_parser(name)
        q.add_argument("--cell-env", required=True)
        q.add_argument("--project-env", required=True)
        q.add_argument("--compose", required=True)
        q.add_argument("--cluster-env")
        if name == "prefix":
            q.add_argument("--group", required=True, help="쉼표 결합: " + ",".join(GROUPS))
    d = sub.add_parser("default")
    d.add_argument("--compose", required=True)
    d.add_argument("--key", required=True)
    a = ap.parse_args(argv)
    if a.self_test:
        bad = selftest()
        for line in bad:
            print("FAIL " + line, file=sys.stderr)
        print(f"[slave-forward] self-test {'PASS' if not bad else 'FAIL'} ({len(bad)} failure(s))")
        return 1 if bad else 0
    if not a.cmd:
        ap.error("하위 명령이 필요하다(prefix|json|default) 또는 --self-test")
    try:
        if a.cmd == "default":
            value = compose_default(a.compose, a.key)
            if value is None:
                print(f"[slave-forward] {a.key} 의 compose 기본값이 없다", file=sys.stderr)
                return 1
            print(value)
            return 0
        rows = derive(a.cell_env, a.project_env, a.compose, a.cluster_env)
        if a.cmd == "prefix":
            print(prefix(rows, *a.group.split(",")))
        else:
            print(json.dumps({"schema_version": 1, "forwards": [asdict(f) for f in rows],
                              "sub_recipe": as_sub_recipe(rows)}, ensure_ascii=False, indent=2, sort_keys=True))
        return 0
    except ForwardError as exc:
        print(f"[slave-forward] FAIL({exc.code}): {exc.message}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
