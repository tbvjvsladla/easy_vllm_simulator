#!/usr/bin/env python3
"""render_native_triplet.py — Docker 트리플렛에서 native(Docker 없음) 트리플렛을 **결정론 파생**한다.

왜 (plan_26092311 N-D3 · F2 · F7):
  native 셀은 Docker 셀과 **같은 서빙 노브**를 호스트 venv 에서 돌리는 셀이다. 손으로 트리플렛을 새로 쓰면
  두 셀의 노브가 조용히 갈라진다(비교의 전제가 무너진다). 그래서 native 트리플렛은 손저작하지 않고 Docker
  트리플렛에서 렌더한다 — 바뀌는 것은 **실행 평면이 강제하는 것**뿐이다:
    ① 컨테이너 경로(`/app/<mount>/…`) → 호스트 경로. 사상은 compose 볼륨(`${VAR:-…}:/app/<mount>`)의 VAR 를
       manifest 의 `var.lower()` 필드로 해소한다(check_smoke_model 과 같은 규칙 · **env·compose 기본값 폴백 ✗** —
       렌더는 결정론이어야 하고, 폴백 기본값은 엉뚱한 트리를 가리킨 선례가 있다).
    ② Docker 평면 키 제거 — 이미지 정체(`IMAGE_TAG`·`BUILD_DOCKERFILE`·build-arg)·빌드 튜닝·컨테이너 이름·
       `COMPOSE_*`. 목록은 손으로 적지 않고 slave_forward.candidates()(compose `${VAR}` 참조 파생)에서 얻는다.
       이 키가 남으면 발행기 평면 판정(artifacts._plane)이 docker 로 읽어 HINT_PLANE_MISMATCH 가 된다(F7).
    ③ `CONFIG_FILE` = native 셀 이름(러너가 자기 yaml 을 읽게) · `.sh` = 호스트 venv 의 `vllm serve`.
  나머지(서빙 노브 전부)는 **바이트 그대로** 옮긴다.

  ⚠ PLE mmap 디렉터리는 run root 아래가 아니라 manifest `ple_mmap_host_path` 스테이징으로 사상한다 — 그것은
    캐시가 아니라 **읽기 전용으로 미리 스테이징된 모델 데이터**(compose 에서 `:ro` 마운트 · 50 GiB 대)이고,
    run root 로 복사하면 노드당 상한(campaign native_run_root_max_gib_per_node)을 넘는다.

CLI:
  render_native_triplet.py --source-cell <docker 셀> --cell <native 셀> [--topology multi] [--repo R]
      (기본) 렌더 결과의 경로·sha256 만 보인다(쓰기 ✗)
      --apply  파일을 쓴다(같은 내용이면 no-op)
      --check  디스크의 트리플렛이 렌더와 바이트 동일한지(0=동일 · 1=표류/부재)
  render_native_triplet.py --self-test
종료코드: 0 성공 · 1 표류(--check) · 2 사용 오류 · 3 렌더 거부(fail-closed)
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import re
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO_DEFAULT = HERE.parents[3]
TOOL = "render_native_triplet.py"


class RenderError(RuntimeError):
    pass


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RenderError(f"모듈 적재 실패: {path}")
    if name in sys.modules:
        return sys.modules[name]
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod          # dataclass 가 자기 모듈을 sys.modules 에서 찾는다
    spec.loader.exec_module(mod)
    return mod


def _modules():
    # env 파서·compose 참조 분류의 단일 소유자(slave_forward) · 마운트 토큰/ manifest 필드 읽기(check_smoke_model).
    return _load("_nt_slave_forward", HERE / "slave_forward.py"), _load("_nt_check_smoke_model", HERE / "check_smoke_model.py")


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def triplet_rel(topology: str, cell: str) -> dict[str, str]:
    return {"yaml": f"output/{topology}/configs/{cell}.yaml", "sh": f"output/{topology}/configs/{cell}.sh",
            "env": f"output/{topology}/envs/.env.{cell}"}


def _yaml_scalar(text: str, key: str) -> str | None:
    m = re.search(rf"(?m)^{re.escape(key)}\s*:\s*(.+?)\s*(?:#.*)?$", text)
    return m.group(1).strip().strip("'\"") if m else None


def _docker_only_keys(sf, compose: Path, cluster_env: Path, env_keys: list[str]) -> tuple[set[str], list[str]]:
    """Docker 평면 키(파생) + 그 규칙 문장. 이미지 정체·빌드 튜닝 = compose 의 image/build 참조(slave_forward.candidates) ·
    컨테이너 이름 = `CONTAINER_NAME`/`*_CONTAINER_NAME` · compose 예약 = `COMPOSE_*`."""
    groups = sf.candidates(compose, cluster_env if cluster_env.is_file() else None)
    drop = set(groups.get("image_identity", [])) | set(groups.get("build_tuning", []))
    for k in [*env_keys, *groups.get("cluster", [])]:
        if k == "CONTAINER_NAME" or k.endswith("_CONTAINER_NAME") or k.startswith("COMPOSE_"):
            drop.add(k)
    rules = ["slave_forward.candidates(compose): image_identity ∪ build_tuning",
             "이름 규칙: CONTAINER_NAME · *_CONTAINER_NAME · COMPOSE_*"]
    return drop, rules


def _map_container_path(cs, compose: Path, base: Path, value: str) -> tuple[str, str] | None:
    """컨테이너 경로 → (호스트 경로, 출처). 마운트 대상이 아니면 None. 마운트인데 해소 불가면 RenderError."""
    parts = value.split("/")
    if len(parts) < 3 or not value.startswith("/"):
        return None
    root = "/".join(parts[:3])
    token = cs.read_app_models_host_token(str(compose), root)
    if not token:
        return None
    m = re.fullmatch(r"\$\{(\w+)(?::-[^}]*)?\}", token)
    if not m:
        raise RenderError(f"{root} 의 compose 마운트 원천이 VAR 가 아니다({token!r}) — manifest 로 사상할 수 없다")
    field = m.group(1).lower()
    host_root = cs.read_manifest_field(str(base), field)
    if not host_root:
        # compose 기본값·env 로 폴백하지 않는다(렌더 결정론 · 폴백 기본값은 엉뚱한 트리를 가리킨 선례).
        raise RenderError(f"manifest.{field} 가 없다 — {root} 를 호스트로 사상할 수 없다(폴백 ✗)")
    return host_root.rstrip("/") + value[len(root):], f"{root} → manifest.{field}"


def render(repo: Path, topology: str, source_cell: str, cell: str) -> dict[str, str]:
    """{저장소 상대 경로: 내용} — 부수효과 없음."""
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,95}", cell) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,95}", source_cell):
        raise RenderError("셀 이름이 안전 문자 밖이다")
    if cell == source_cell:
        raise RenderError("native 셀 이름은 원본 Docker 셀과 달라야 한다")
    sf, cs = _modules()
    base = repo / "output" / topology
    src = triplet_rel(topology, source_cell)
    for k, rel in src.items():
        if not (repo / rel).is_file():
            raise RenderError(f"원본 Docker 트리플렛 {k} 부재: {rel}")
    compose = base / "docker-compose.yaml"
    if not compose.is_file():
        raise RenderError(f"{compose.relative_to(repo)} 부재 — 마운트 사상·평면 키 파생의 원천이 없다")
    patch = base / "configs" / f"{source_cell}_patch.py"
    if patch.exists():
        raise RenderError(f"원본 셀에 런타임 패치({patch.name})가 있다 — native 평면 arming 은 미지원(fail-closed · "
                          "policy:RUNTIME_PATCH_NO_CARRY_FORWARD)")
    y_src = (repo / src["yaml"]).read_text(encoding="utf-8")
    e_src = (repo / src["env"]).read_text(encoding="utf-8")
    env = sf.read_env_first(repo / src["env"])
    if str(env.get("TIKTOKEN_ENABLED", "false")).strip().lower() == "true":
        raise RenderError("TIKTOKEN_ENABLED=true — native 평면 tiktoken 인코딩 배선은 미지원(fail-closed)")
    if not re.search(r"(?m)^\s*distributed-executor-backend\s*:\s*ray\b", y_src):
        raise RenderError("원본 yaml 에 distributed-executor-backend: ray 가 없다(멀티 TP 필수 노브 · serve_runner 게이트와 같다)")
    y_port, e_port = _yaml_scalar(y_src, "port"), str(env.get("SERVING_PORT", "")).strip()
    if not y_port or y_port != e_port:
        raise RenderError(f"yaml port={y_port!r} ≠ env SERVING_PORT={e_port!r} — native 는 포트 사상이 없으므로 같아야 한다")
    for need in ("SERVING_MODEL_NAME", "SERVING_PORT", "CONFIG_FILE"):
        if not str(env.get(need, "")).strip():
            raise RenderError(f"원본 env 에 {need} 가 없다")

    # ① yaml — model: 한 줄만 호스트 경로로(나머지 바이트 그대로)
    mm = re.search(r"(?m)^(model\s*:\s*)(\S+)(.*)$", y_src)
    if not mm:
        raise RenderError("원본 yaml 에 model: 이 없다")
    mapped = _map_container_path(cs, compose, base, mm.group(2).strip("'\""))
    if mapped is None:
        raise RenderError(f"model: {mm.group(2)} 가 compose 마운트 대상이 아니다 — 호스트 경로로 사상할 수 없다")
    model_host, model_src = mapped
    y_body = y_src[:mm.start()] + mm.group(1) + model_host + mm.group(3) + y_src[mm.end():]
    prov = [f"# ⚙ 렌더 산출물 — {TOOL} 가 Docker 트리플렛 {source_cell} 에서 결정론 파생(손저작 ✗ · plan_26092311 N-D3).",
            f"#   원본 {src['yaml']} sha256={sha256_text(y_src)[:16]} · 경로 사상 model: {model_src}",
            "#   서빙 노브는 원본과 바이트 동일하다 — 고치려면 원본 Docker 트리플렛을 고치고 다시 렌더한다."]
    yaml_out = f"# 셀 {cell} — native(Docker 없음) 트리플렛 yaml\n" + "\n".join(prov) + "\n" + y_body

    # ② env — Docker 평면 키 제거 · 컨테이너 경로 값 사상 · CONFIG_FILE = native 셀
    drop, rules = _docker_only_keys(sf, compose, base / "envs" / ".env.cluster", list(env))
    out_lines, dropped, remapped = [], [], []
    for raw in e_src.splitlines():
        m = re.fullmatch(r"([A-Za-z_][A-Za-z0-9_]*)=(.*)", raw)
        if not m:
            out_lines.append(raw)
            continue
        key, val = m.group(1), m.group(2)
        if key in drop:
            dropped.append(key)
            continue
        if key == "CONFIG_FILE":
            val = cell
        elif val.startswith("/"):
            mp = _map_container_path(cs, compose, base, val)
            if mp is None and val.startswith("/app/"):
                raise RenderError(f"{key}={val} 는 컨테이너 경로인데 compose 마운트가 없다 — 호스트로 사상할 수 없다")
            if mp is not None:
                val = mp[0]
                remapped.append(f"{key}({mp[1]})")
        out_lines.append(f"{key}={val}")
    env_head = [f"# vLLM native 서빙 환경 — {cell}",
                f"# ⚙ 렌더 산출물 — {TOOL} 가 Docker 셀 env {src['env']} (sha256={sha256_text(e_src)[:16]}) 에서 파생.",
                "#   제거한 Docker 평면 키(값 없음 · 규칙: " + " / ".join(rules) + "): " + (" · ".join(sorted(dropped)) or "없음"),
                "#   컨테이너 경로 → 호스트 사상: " + (" · ".join(remapped) or "없음") + " · CONFIG_FILE → " + cell,
                "#   실행 평면 = native(이미지 선택자 부재) — 발행기 artifacts._plane 이 이 파일로 평면을 판정한다."]
    env_out = "\n".join(env_head + out_lines).rstrip("\n") + "\n"

    # ③ sh — 호스트 venv 의 vllm serve(설정은 같은 디렉터리의 native yaml)
    sh_out = "\n".join([
        "#!/bin/bash",
        f"# {cell} native 서빙 러너 — {TOOL} 가 Docker 트리플렛 {source_cell} 에서 렌더(손저작 ✗).",
        f"#   원본 {src['sh']} sha256={sha256_text((repo / src['sh']).read_text(encoding='utf-8'))[:16]} 의 `vllm serve --config … --served-model-name …` 를",
        "#   호스트 venv 로 옮긴 것이다. 컨테이너 전용 arming(arm_patch.sh·tiktoken /encodings)은 렌더가 fail-closed 로 막는다.",
        "#   기동 주체는 native_multinode_serve.py up 이다 — 셀 env·캐시 격리 env·LD_LIBRARY_PATH·NATIVE_VENV 를 주입해 이 파일을 실행한다.",
        "set -euo pipefail",
        ': "${NATIVE_VENV:?NATIVE_VENV(run root 의 venv) 미주입 — native_multinode_serve.py up 이 주입한다}"',
        ': "${CONFIG_FILE:?CONFIG_FILE 미주입(셀 env)}"',
        ': "${SERVING_MODEL_NAME:?SERVING_MODEL_NAME 미주입(셀 env)}"',
        'HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"',
        'exec "${NATIVE_VENV}/bin/vllm" serve --config "${HERE}/${CONFIG_FILE}.yaml" \\',
        '    --served-model-name "${SERVING_MODEL_NAME}"',
        ""])
    dst = triplet_rel(topology, cell)
    return {dst["yaml"]: yaml_out, dst["sh"]: sh_out, dst["env"]: env_out}


def check(repo: Path, rendered: dict[str, str]) -> list[str]:
    """표류 목록(빈 = 디스크 == 렌더)."""
    bad = []
    for rel, text in sorted(rendered.items()):
        p = repo / rel
        if p.is_symlink() or not p.is_file():
            bad.append(f"{rel}: 부재")
        elif p.read_text(encoding="utf-8") != text:
            bad.append(f"{rel}: 렌더와 다르다(손편집 또는 원본 변경)")
    return bad


def apply(repo: Path, rendered: dict[str, str]) -> list[str]:
    wrote = []
    for rel, text in sorted(rendered.items()):
        p = repo / rel
        if p.is_symlink():
            raise RenderError(f"{rel} 가 symlink 다 — 쓰지 않는다")
        if p.is_file() and p.read_text(encoding="utf-8") == text:
            continue
        p.parent.mkdir(parents=True, exist_ok=True)
        tmp = p.with_name(p.name + ".render-tmp")
        tmp.write_text(text, encoding="utf-8")
        tmp.chmod(0o755 if rel.endswith(".sh") else 0o644)
        tmp.replace(p)
        wrote.append(rel)
    return wrote


# ── 자체검사 ────────────────────────────────────────────────────────────────────────────────────────
_FX_COMPOSE = """x-gpu-common: &gpu-common
  image: ${IMAGE_TAG:-easy-vllm:x}
  build:
    context: .
    dockerfile: ${BUILD_DOCKERFILE:-Dockerfile.source-build}
    args:
      VLLM_REF: ${VLLM_REF:-v0.9.0}
      BUILD_JOBS: ${BUILD_JOBS:-16}
  volumes:
    - ${QUANT_MODEL_PATH:-/srv/models}:/app/quant_models:ro
    - ${PLE_MMAP_HOST_PATH:-/srv/models}:/app/ple_mmap:ro
services:
  vllm-master-serve:
    <<: *gpu-common
    container_name: ${MASTER_CONTAINER_NAME:-m}
  vllm-slave-serve:
    <<: *gpu-common
    container_name: ${SLAVE_CONTAINER_NAME:-s}
    environment:
      RAY_PORT: ${RAY_PORT:-6379}
      CONFIG_FILE: ${CONFIG_FILE:-default}
      VLLM_PLE_MMAP: ${VLLM_PLE_MMAP:-0}
      VLLM_PLE_MMAP_DIR: ${VLLM_PLE_MMAP_DIR:-/app/ple_mmap/x}
    env_file:
      - envs/.env.interconnect
      - envs/.env.cluster
"""
_FX_YAML = """# 셀 src
model: /app/quant_models/Org/Model-X
host: 0.0.0.0
port: 8080
tensor-parallel-size: 2
distributed-executor-backend: ray
kv-cache-memory-bytes: 21474836480   # 20480MiB/노드
enforce-eager: true
"""
_FX_ENV = """# fixture env
COMPOSE_PROJECT_NAME=p
MASTER_CONTAINER_NAME=src-master-container
SLAVE_CONTAINER_NAME=src-slave-container
SERVING_PORT=8080
SERVING_MODEL_NAME=m
CONFIG_FILE=src
TIKTOKEN_ENABLED=false
IMAGE_TAG=easy-vllm:x
BUILD_DOCKERFILE=Dockerfile.source-build
VLLM_REF=v0.9.0
BUILD_JOBS=8
VLLM_PLE_MMAP=1
VLLM_PLE_MMAP_DIR=/app/ple_mmap/q
"""


def _fixture(root: Path, manifest_extra: str = "") -> Path:
    base = root / "output" / "multi"
    (base / "configs").mkdir(parents=True)
    (base / "envs").mkdir(parents=True)
    (base / "docker-compose.yaml").write_text(_FX_COMPOSE, encoding="utf-8")
    (base / "manifest.yaml").write_text('quant_model_path: "/fx/quant"\nple_mmap_host_path: "/fx/ple"\n' + manifest_extra, encoding="utf-8")
    (base / "configs" / "src.yaml").write_text(_FX_YAML, encoding="utf-8")
    (base / "configs" / "src.sh").write_text("#!/bin/bash\nvllm serve --config /app/configs/${CONFIG_FILE}.yaml\n", encoding="utf-8")
    (base / "envs" / ".env.src").write_text(_FX_ENV, encoding="utf-8")
    (base / "envs" / ".env.cluster").write_text("RAY_PORT=6379\nMASTER_HOST_IP=198.51.100.1\n", encoding="utf-8")
    return root


def self_test() -> int:
    bad: list[str] = []

    def ck(name: str, cond: bool) -> None:
        print(("[PASS] " if cond else "[FAIL] ") + name)
        if not cond:
            bad.append(name)

    sf, _ = _modules()
    with tempfile.TemporaryDirectory(prefix="render-native-") as d:
        repo = _fixture(Path(d) / "repo")
        out = render(repo, "multi", "src", "src-native")
        ck("결정론(두 번 같은 바이트)", out == render(repo, "multi", "src", "src-native"))
        rel = triplet_rel("multi", "src-native")
        y, e, s = out[rel["yaml"]], out[rel["env"]], out[rel["sh"]]
        ck("yaml model: 가 manifest.quant_model_path 로 사상", "\nmodel: /fx/quant/Org/Model-X\n" in y)
        body_src = [ln for ln in _FX_YAML.splitlines() if not ln.startswith("model:")]
        body_out = [ln for ln in y.splitlines() if not ln.startswith("#") and not ln.startswith("model:")]
        ck("yaml 서빙 노브는 원본과 바이트 동일", [ln for ln in body_src if not ln.startswith("#")] == body_out)
        ck("yaml 머리말에 렌더 출처(원본 셀)", "Docker 트리플렛 src" in y and TOOL in y)
        apply(repo, out)
        envp = repo / rel["env"]
        keys = sf.read_env_first(envp)
        ck("★음성대조 — 렌더 env 에 IMAGE_TAG·BUILD_DOCKERFILE 키 없음", "IMAGE_TAG" not in keys and "BUILD_DOCKERFILE" not in keys)
        ck("★음성대조 — 원본 env 에는 있었다(대조가 눈멀지 않았다)",
           {"IMAGE_TAG", "BUILD_DOCKERFILE"} <= set(sf.read_env_first(repo / "output/multi/envs/.env.src")))
        ck("빌드 인자·컨테이너 이름·compose 키 제거", not ({"VLLM_REF", "BUILD_JOBS", "MASTER_CONTAINER_NAME",
                                                  "SLAVE_CONTAINER_NAME", "COMPOSE_PROJECT_NAME"} & set(keys)))
        ck("서빙 키 유지 + CONFIG_FILE=native 셀", keys.get("SERVING_PORT") == "8080" and keys.get("SERVING_MODEL_NAME") == "m"
           and keys.get("CONFIG_FILE") == "src-native" and keys.get("VLLM_PLE_MMAP") == "1")
        ck("PLE mmap 디렉터리 → manifest.ple_mmap_host_path", keys.get("VLLM_PLE_MMAP_DIR") == "/fx/ple/q")
        ck("env 머리말에 렌더 출처", "Docker 셀 env" in e and TOOL in e)
        ck("sh = 호스트 venv vllm serve · 설정은 native yaml", '"${NATIVE_VENV}/bin/vllm" serve --config "${HERE}/${CONFIG_FILE}.yaml"' in s
           and "/app/" not in s.split("set -euo pipefail", 1)[1])
        ck("--check: 쓴 직후 표류 0", check(repo, out) == [])
        envp.write_text(envp.read_text(encoding="utf-8") + "IMAGE_TAG=hand\n", encoding="utf-8")
        ck("--check: 손편집(IMAGE_TAG 재삽입)을 표류로 잡는다", any("env" in b for b in check(repo, out)))
        ck("apply 멱등(재적용 = 표류분만 쓴다)", apply(repo, out) == [rel["env"]] and apply(repo, out) == [])
        # 발행기 자신의 평면 판정 — native 선언 + 렌더 env = native · 같은 선언 + Docker env = HINT_PLANE_MISMATCH
        try:
            sys.path.insert(0, str(REPO_DEFAULT / ".claude/skills/hint-publisher/scripts"))
            from hintlib import artifacts, core  # noqa: E402
            fwd = _load("_nt_sf_pub", HERE / "slave_forward.py")
            ck("발행기 artifacts.plane_of(렌더 env) = native",
               artifacts.plane_of(repo, {"topology": "multi", "cell": "src-native", "plane": "native"}, forward_module=fwd) == "native")
            code = None
            try:
                artifacts.plane_of(repo, {"topology": "multi", "cell": "src", "plane": "native"}, forward_module=fwd)
            except core.HintError as exc:
                code = exc.code
            ck("★음성대조 — Docker env 에 native 선언 = HINT_PLANE_MISMATCH", code == "HINT_PLANE_MISMATCH")
        except ImportError as exc:
            ck(f"발행기 평면 판정 import({exc})", False)
        # fail-closed 경로
        for what, mutate in (("manifest 필드 부재 → 폴백 ✗", lambda r: (r / "output/multi/manifest.yaml").write_text("x: 1\n", encoding="utf-8")),
                             ("런타임 패치 존재 → 거부", lambda r: (r / "output/multi/configs/src_patch.py").write_text("", encoding="utf-8")),
                             ("port ≠ SERVING_PORT → 거부", lambda r: (r / "output/multi/configs/src.yaml").write_text(_FX_YAML.replace("port: 8080", "port: 9"), encoding="utf-8")),
                             ("tiktoken → 거부", lambda r: (r / "output/multi/envs/.env.src").write_text(_FX_ENV.replace("TIKTOKEN_ENABLED=false", "TIKTOKEN_ENABLED=true"), encoding="utf-8"))):
            r2 = _fixture(Path(d) / f"neg{len(bad)}{abs(hash(what)) % 10000}")
            mutate(r2)
            refused = False
            try:
                render(r2, "multi", "src", "src-native")
            except RenderError:
                refused = True
            ck(f"★fail-closed — {what}", refused)
    print(f"[render-native-triplet] {'PASS' if not bad else 'FAIL'}")
    return 0 if not bad else 1


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--source-cell")
    ap.add_argument("--cell")
    ap.add_argument("--topology", default="multi", choices=["multi"])
    ap.add_argument("--repo", default=str(REPO_DEFAULT))
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--apply", action="store_true")
    g.add_argument("--check", action="store_true")
    a = ap.parse_args(argv)
    if a.self_test:
        return self_test()
    if not a.source_cell or not a.cell:
        print("사용: --source-cell <docker 셀> --cell <native 셀> [--apply|--check]", file=sys.stderr)
        return 2
    repo = Path(a.repo).resolve()
    try:
        out = render(repo, a.topology, a.source_cell, a.cell)
        if a.check:
            drift = check(repo, out)
            for line in drift:
                print(f"[render-native] DRIFT {line}")
            return 1 if drift else 0
        if a.apply:
            wrote = apply(repo, out)
            print(f"[render-native] {'쓴 파일: ' + ', '.join(wrote) if wrote else '변경 없음(이미 렌더와 동일)'}")
            return 0
        for rel, text in sorted(out.items()):
            print(f"[render-native] (dry) {rel} sha256={sha256_text(text)[:16]} {len(text)}B")
        return 0
    except RenderError as exc:
        print(f"[render-native] FAIL: {exc}", file=sys.stderr)
        return 3


if __name__ == "__main__":
    raise SystemExit(main())
