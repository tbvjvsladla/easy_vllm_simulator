# syntax=docker/dockerfile:1
# 스켈레톤 — render_dockerfile.py(G2)가 manifest+버전해소값으로 채움

# Phase 1 안정 앵커: vLLM 0.18.0 / NGC 26.01-py3 (torch 핀 prefix 매칭) — prebuilt 동작 검증 구간.
FROM nvcr.io/nvidia/pytorch:26.01-py3

# 아래 3개 ARG는 resolve_wheel.py 가 GitHub Release 자산에서 자동 도출한 값으로 채운다.
# 버전마다 살짝 바뀌는 휘발성 디테일(cuXXX·manylinux)을 ARG로 격리 — 사람은 vLLM 버전만 지정.
ARG VLLM_VERSION=0.18.0
ARG CUDA_VERSION=130
ARG VLLM_MANYLINUX=manylinux_2_35

RUN cp /etc/pip/constraint.txt /etc/pip/constraint.txt.bak \
    && : > /etc/pip/constraint.txt

# CPU_ARCH 는 render-time 리터럴 baking 금지 → build-time $(uname -m) (헌법: manifest 스캔값과 로컬빌드 일치)
RUN CPU_ARCH=$(uname -m) \
    && pip install --no-deps \
      "https://github.com/vllm-project/vllm/releases/download/v${VLLM_VERSION}/vllm-${VLLM_VERSION}+cu${CUDA_VERSION}-cp38-abi3-${VLLM_MANYLINUX}_${CPU_ARCH}.whl"

# 조건부 패치: NGC torch(2.11 alpha)의 register_opaque_type 가 hoist 인자를 안 받을 때만
# LayerName opaque-type 최적화를 끈다(Phase 2 구간 호환). torch 2.10대(이 앵커)에선 opaque_object 자체가
# 없거나 HAS_OPAQUE_TYPE=False 라 안전하게 skip 된다(서빙 기능엔 무영향).
#   ★ 2026-09-21: 결과를 빌드 원장(ledger.d/inline.has-opaque-type.status)에 남긴다(plan_26092119 §4.5).
#     치환 대상 문자열이 없으면 파일은 그대로다 — 그때 "disabled" 라고 말하지 않고 skipped 로 적는다.
RUN python3 - <<'PY'
import inspect, os, pathlib
needs = False
try:
    from torch._library.opaque_object import register_opaque_type
    needs = "hoist" not in inspect.signature(register_opaque_type).parameters
except Exception:
    needs = False  # torch 2.10 등 opaque_object 미존재 → 패치 불요
p = pathlib.Path("/usr/local/lib/python3.12/dist-packages/vllm/utils/torch_utils.py")
if needs and p.exists():
    old = p.read_text()
    s = old.replace(
        'HAS_OPAQUE_TYPE = is_torch_equal_or_newer("2.11.0.dev")',
        'HAS_OPAQUE_TYPE = False  # patched: NGC torch register_opaque_type lacks hoist arg',
    )
    p.write_text(s)
    if s != old:
        print("[patch] HAS_OPAQUE_TYPE disabled (NGC torch hoist mismatch)")
        status = "applied\tHAS_OPAQUE_TYPE disabled (NGC torch hoist mismatch)"
    else:
        print("[patch][WARN] HAS_OPAQUE_TYPE 치환 대상 문자열 미발견 — 상류 변경 의심")
        status = "skipped\thoist 미수용인데 치환 대상 문자열 미발견(WARN)"
else:
    print("[patch] skipped (torch supports hoist / no opaque_object / file missing)")
    status = "skipped\ttorch supports hoist / no opaque_object / file missing"
os.makedirs("/opt/easy-vllm/ledger.d", exist_ok=True)
with open("/opt/easy-vllm/ledger.d/inline.has-opaque-type.status", "w", encoding="utf-8") as fh:
    fh.write(status + "\n")
PY

COPY requirements.txt /tmp/requirements.txt
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install -r /tmp/requirements.txt

# ═════════════════════════════════════════════════════════════════════════════
# [분산서빙 런타임 의존] — source-build 트랙과 **같은 규약**이다(그 템플릿 §분산서빙 런타임 의존).
#   2026-09-05: wheel 트랙에는 이 스탠자가 **없었다**. 그래서 output/multi/Dockerfile 에 손으로
#   얹혀 있었고, 재렌더 한 번이면 조용히 사라져 TP=2 가 `ray: command not found` 로 죽는다
#   (SM12X_PORT 와 같은 형태의 침묵 누락 — 실제로 2026-09-05 재렌더에서 사라진 것을 확인했다).
#   토폴로지 조건부로 만들지 않는 이유도 동일하다: IMAGE_TAG 에 토폴로지 축이 없으므로
#   조건부화는 이미지 정체성을 갈라 policy:VARIANT_IMAGE_BUILD_VS_SERVE_PLANE 위반이 된다.
#   A) netcat: serve_runner.sh 의 slave 가 `nc` 로 Ray head 포트를 대기한다 → 부재 시 무한대기.
#   B) ray   : vLLM 코어 dep 이 아니라 requirements 에 안 나온다 → 명시 설치가 유일한 통로.
# ═════════════════════════════════════════════════════════════════════════════
RUN apt-get update && apt-get install -y --no-install-recommends \
        iproute2 netcat-openbsd iputils-ping dnsutils \
    && rm -rf /var/lib/apt/lists/*
ARG RAY_VERSION=2.48.0
RUN pip install ray==${RAY_VERSION}

# ═════════════════════════════════════════════════════════════════════════════
# [빌드 원장] /opt/easy-vllm/build_ledger.json — 이미지 자기서술(render_dockerfile.py 단일 소유 주입)
#   build-arg 목록 = 이 템플릿의 `^ARG` 줄에서 렌더 시 파생 · 분류 = 상태 줄 > 로그 토큰 > 종료코드.
#   **마지막 RUN** 이어야 한다 — 이 레이어만 바뀌면 `_C` 컴파일 캐시가 보존된다.
#   `import vllm._C` 금지(빌드 시 libcuda 없음) · 베이스 사실은 파일에서 읽는다(ARG 가 ENV 를 가린다).
# ═════════════════════════════════════════════════════════════════════════════
RUN python3 - <<'PY'
import hashlib, json, os, platform, re, subprocess
from importlib import metadata
ROOT = os.environ.get("EASY_VLLM_LEDGER_ROOT") or "/opt/easy-vllm"
D = os.path.join(ROOT, "ledger.d")
ARGS = ["VLLM_VERSION", "CUDA_VERSION", "VLLM_MANYLINUX", "RAY_VERSION"]
DOCKERFILE = "Dockerfile"
PATCH_DIRS = {"pre": "/tmp/build_patches_src", "post": "/tmp/build_patches"}
SLOTS = (("pre", "build_patch_pre"), ("post", "build_patch_post"))
VLLM_SRC = "/workspace/vllm-src"
SKIP_TOKEN = re.compile(r"(\] skip —|— skip\b|— skip\()")
TRIGGER = re.compile(r"^#\s*model-trigger\s*:\s*(.+?)\s*$", re.M)
STATUS = re.compile(r"^(applied|skipped)\t(.*)$")
errors = {}
def _read(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            return fh.read()
    except OSError:
        return None
def _sha256(path):
    try:
        with open(path, "rb") as fh:
            return hashlib.sha256(fh.read()).hexdigest()
    except OSError:
        return None
def _run(key, *cmd):
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
    except (OSError, subprocess.TimeoutExpired) as exc:
        errors[key] = type(exc).__name__
        return None
    if proc.returncode != 0:
        errors[key] = "rc=%d %s" % (proc.returncode, (proc.stderr or "").strip()[-200:])
        return None
    return proc.stdout
def _dist(name):
    try:
        return metadata.version(name)
    except metadata.PackageNotFoundError:
        errors["dist:" + name] = "not-installed"
        return None
def _status(path):
    text = _read(path)
    if text is None:
        return None, None
    lines = text.splitlines()
    line = lines[0] if lines else ""
    m = STATUS.match(line)
    if m:
        return (m.group(1), m.group(2).strip() or None), None
    return None, "형식 밖 상태 줄: %r" % line[:120]
def _listing():
    return sorted(os.listdir(D)) if os.path.isdir(D) else []
def _patch_rows(phase):
    rows = []
    head = phase + "."
    for entry in _listing():
        if not (entry.startswith(head) and entry.endswith(".rc")):
            continue
        name = entry[len(head):-len(".rc")]
        base = os.path.join(D, head + name)
        rc = (_read(base + ".rc") or "").strip()
        row = {"phase": phase, "file": name,
               "exit_code": int(rc) if rc.lstrip("-").isdigit() else None,
               "script_sha256": (_read(base + ".sha256") or "").strip() or None,
               "log_tail_sha256": _sha256(base + ".log")}
        status, err = _status(base + ".status")
        if err:
            row["status_file_error"] = err
        if status:
            row.update(result=status[0], reason=status[1], result_source="status-file")
        else:
            log = _read(base + ".log") or ""
            hit = next((ln.strip() for ln in log.splitlines() if SKIP_TOKEN.search(ln)), None)
            if hit:
                row.update(result="skipped", reason=hit, result_source="log-token")
            else:
                row.update(result="applied", reason=None, result_source="exit-code")
        m = TRIGGER.search(_read(os.path.join(PATCH_DIRS[phase], name)) or "")
        row["declared_model_trigger"] = m.group(1) if m else None
        rows.append(row)
    return rows
def _inline_rows():
    rows = []
    for entry in _listing():
        if not (entry.startswith("inline.") and entry.endswith(".status")):
            continue
        status, err = _status(os.path.join(D, entry))
        row = {"phase": "inline", "file": entry[len("inline."):-len(".status")], "exit_code": None,
               "script_sha256": None, "log_tail_sha256": None, "declared_model_trigger": None}
        if status:
            row.update(result=status[0], reason=status[1], result_source="status-file")
        else:
            row.update(result="unobserved", reason=err, result_source="none")
        rows.append(row)
    return rows
base = {"source": {}}
for field, env_name in (("ngc_pytorch_version", "NVIDIA_PYTORCH_VERSION"),
                        ("pytorch_build_version", "PYTORCH_BUILD_VERSION")):
    if env_name in ARGS:
        base[field] = None
        base["source"][field] = "shadowed-by-ARG:" + env_name
    else:
        base[field] = os.environ.get(env_name) or None
        base["source"][field] = "base-image ENV " + env_name
cuda_text = _read("/usr/local/cuda/version.json")
base["cuda_version"] = None
if cuda_text:
    try:
        base["cuda_version"] = (json.loads(cuda_text).get("cuda") or {}).get("version")
    except ValueError:
        errors["cuda_version_json"] = "unparsable"
else:
    errors["cuda_version_json"] = "absent"
base["source"]["cuda_version"] = "/usr/local/cuda/version.json"
vllm = {"git_sha": None, "describe": None, "dist_version": _dist("vllm"), "direct_url": None}
if os.path.isdir(os.path.join(VLLM_SRC, ".git")):
    out = _run("vllm_git_sha", "git", "-C", VLLM_SRC, "rev-parse", "HEAD")
    vllm["git_sha"] = out.strip() if out else None
    out = _run("vllm_describe", "git", "-C", VLLM_SRC, "describe", "--tags", "--match", "v*")
    vllm["describe"] = out.strip() if out else None
try:
    direct = metadata.distribution("vllm").read_text("direct_url.json")
    vllm["direct_url"] = json.loads(direct) if direct else None
except (metadata.PackageNotFoundError, ValueError):
    pass
inventory = {}
for phase, slot in SLOTS:
    text = _read(os.path.join(D, phase + ".inventory"))
    if text is not None:
        inventory[slot] = sorted(x for x in text.splitlines() if x)
os.makedirs(ROOT, exist_ok=True)
freeze = _run("pip_freeze", "python3", "-m", "pip", "freeze", "--all")
if freeze is not None:
    with open(os.path.join(ROOT, "pip-freeze.txt"), "w", encoding="utf-8") as fh:
        fh.write(freeze)
patches = _patch_rows("pre") + _patch_rows("post") + _inline_rows()
ledger = {
    "schema_version": 1, "kind": "easy_vllm_build_ledger",
    "provenance": "measured(build-time, in-image)",
    "dockerfile": DOCKERFILE,
    "track": "source-build" if DOCKERFILE.startswith("Dockerfile.source-build") else "wheel",
    "build_args": {k: os.environ.get(k) for k in ARGS},
    "base": base, "vllm": vllm, "torch": {"dist_version": _dist("torch")},
    "cpu_arch": platform.machine(),
    "slots_supported": sorted(inventory), "context_inventory": inventory,
    "patches": patches,
    "requirements_sha256": _sha256("/tmp/requirements.txt"),
    "pip_freeze_sha256": hashlib.sha256(freeze.encode("utf-8")).hexdigest() if freeze is not None else None,
    "errors": errors,
}
path = os.path.join(ROOT, "build_ledger.json")
with open(path, "w", encoding="utf-8") as fh:
    json.dump(ledger, fh, ensure_ascii=False, indent=1, sort_keys=True)
    fh.write("\n")
print("[build-ledger] %s — patches=%d slots=%s errors=%s" % (path, len(patches), sorted(inventory), sorted(errors)))
PY

WORKDIR /workspace

EXPOSE 8000
