#!/usr/bin/env python3
"""render_dockerfile.py — 스켈레톤 템플릿 → 완성 산출물 결정론 렌더러 (G2 구현).

upstream-version-watch 의 render 단계. 템플릿의 {{ PLACEHOLDER }} 토큰을
manifest(환경 사실) + resolved(버전해소값, resolve_*.py 산출)로 치환해
빌드 가능한 산출물 문자열을 만든다. ${...}(빌드ARG·compose 런타임 변수)는 건드리지 않는다.

지원 템플릿:
  Dockerfile.template               (prebuilt wheel 트랙)
  Dockerfile.source-build.template  (source-build 트랙 — torch핀 가드)
  docker-compose.template.yaml      (compose, 트랙별 dockerfile 선택)

플레이스홀더 ↔ 소스:
  {{ VLLM_VERSION }}    ← resolved.vllm_version
  {{ CUDA_VERSION }}    ← resolved.wheel.cuda            (wheel 트랙 URL)
  {{ VLLM_MANYLINUX }}  ← resolved.wheel.manylinux       (wheel 트랙 URL)
  {{ NGC_TAG }}         ← resolved.ngc_base.tag
  {{ CPU_ARCH }}        ← manifest.cpu_arch              (build-time $(uname -m))
  {{ VLLM_REF }}        ← "v"+resolved.vllm_version      (source 트랙 git ref)
  {{ TORCH_CUDA_ARCH }} ← resolved.source_build.torch_cuda_arch (예 12.1a / GB10 sm_121a)
  {{ TORCH_PIN }}       ← resolved.torch.pin
  {{ NAS_MODEL_PATH }}  ← manifest.nas_model_path
  {{ IMAGE_NAME }}      ← "easy-vllm"
  {{ IMAGE_TAG }}       ← {vllm}-cu{cuda}-{arch}-{track}  (버전-키드, 모델-키잉 금지)
  {{ DOCKERFILE }}      ← 트랙별 ("Dockerfile" | "Dockerfile.source-build")
  {{ SOURCE_BUILD_PATCH_GUARD }} ← (NGC베이스×vLLM버전) 키 가드 (검증된 키=주석 / 미인식=빌드 명시 실패)

설계: 결정론 resolve 는 스크립트, 패치 SELECT 는 검토 루프(판단계층). 아래
VALIDATED_SOURCE_BUILD_KEYS 는 P6 카탈로그(source_build_patches.yaml) 승급 전의
최소 인라인 가드다(rev3). stdlib 만 사용. manifest.yaml 은 pyyaml 있으면 사용, 없으면 flat 미니파서.
"""

import sys
import os
import re
import json
import shutil
import argparse

IMAGE_NAME = "easy-vllm"

# 통로 self-containment(plan_2026062321_1 I1/I2): 컨테이너가 쓰는 러너 스크립트 정본은 repo-root configs/(tracked).
# render 가 이를 output/<topology>/configs/ 로 materialize(복사)해 통로를 완결시킨다(런타임 mount-overlay·통로 밖 마운트 금지).
RUNNER_SCRIPTS = ("serve_runner.sh", "debug-init.sh", "arm_patch.sh")

# ── NCCL/RDMA 통신 env (Plan 2: docker-compose 하드코딩 17개 → manifest-driven 렌더) ──────────
#   3-tier taxonomy: ①환경값 = manifest.interconnect(hca_devices/gid_index/socket_iface)
#                     ②프리셋 = NCCL_PRESETS[platform_preset]  ③불변 = NCCL_INVARIANTS.
#   배포 기준 = dgx-spark-gb10 단일 프리셋. 다른 플랫폼은 배포자 코드에이전트가 확장(선반영 금지·Karpathy B2).
#   미지 platform_preset → fail-loud(KeyError, 무증거 추측 금지). 회귀 oracle:
#     .claude/skills/upstream-version-watch/fixtures/nccl_env_dgx-spark-gb10.golden (검증 208.2 Gb/s).
NCCL_PRESETS = {
    "dgx-spark-gb10": {                 # devlog_250422 검증 튜닝(20→47 tok/s · 2.35x)
        "NCCL_IB_MERGE_NICS": "1",
        "NCCL_IB_QPS_PER_CONNECTION": "4",
        "NCCL_IB_SPLIT_DATA_ON_QPS": "0",
        "NCCL_NET_GDR_LEVEL": "SYS",
        "NCCL_NET_GDR_C2C": "1",
        "NCCL_NET_GDR_READ": "1",
        "NCCL_CROSS_NIC": "1",
    },
}
NCCL_INVARIANTS = {                     # ③ universal — 인터커넥트 무관 디버그/안전
    "NCCL_DEBUG": "INFO",
    "NCCL_DEBUG_SUBSYS": "INIT,NET,GRAPH,ENV",
    "NCCL_IB_DISABLE": "0",            # RoCE 상존 전제(dgx-spark). 비-RDMA 프리셋 생기면 ②로 이동(seam)
}
# socket_iface 한 값을 참조하는 ① env 키들(NCCL bootstrap·gloo·torch·UCX·OpenMPI).
_IFACE_ENV_KEYS = ("NCCL_SOCKET_IFNAME", "GLOO_SOCKET_IFNAME", "TP_SOCKET_IFNAME",
                   "UCX_NET_DEVICES", "OMPI_MCA_btl_tcp_if_include")

# ── Ray 클러스터-배포 env (S6 env-split: 모델 env 인라인 중복 → manifest-driven 렌더) ──────────
#   3-tier(NCCL 동형): ①환경값 = manifest.nodes[](main/sub host·ssh_user) → MASTER/SLAVE_HOST_IP·SSH_USER
#                       ②프리셋 = CLUSTER_PRESETS[platform_preset](통합메모리 풀 보호 RAY/alloc 튜닝)
#                       ③불변 = CLUSTER_INVARIANTS(RAY_PORT).
#   `.env.cluster` = Band2(토폴로지-keyed, 서브 전파). 모델 env(.env.<model>)는 Band3(모델/컨테이너명·포트만).
#   미지 platform_preset → fail-loud. 회귀 oracle: fixtures/cluster_env_dgx-spark-gb10.golden(RFC5737 합성 노드값=PII-free).
CLUSTER_PRESETS = {
    "dgx-spark-gb10": {                 # 통합메모리 풀 보호(전례 Qwen3-Next/122B serve 검증)
        "RAY_memory_usage_threshold": "0.99",
        "RAY_memory_monitor_refresh_ms": "0",
        "PYTORCH_CUDA_ALLOC_CONF": "expandable_segments:True",
        "RAY_OBJECT_STORE_MEMORY": "2000000000",   # serve_runner 가 --object-store-memory CLI 로 소비(Ray env 미인식)
        "MAX_JOBS": "4",                # serve-time MoE 커널 JIT(sm_121a, 多expert) nvcc 병렬 cap — 동시 컴파일 OOM 방지(122b attempt-3 실증).
                                        #   슬레이브도 model shard 로드·JIT 하므로 Band2(cluster)서 양노드 도달해야 함(plan_2026062811_2 — 슬레이브 Band2-only 완결).
                                        #   이미지 ENV 기본 16 override. 비-MoE 모델엔 no-op(안전 보수 상수). 모델별 override 필요시 .env.<model>(master) 에서.
    },
}
CLUSTER_INVARIANTS = {                  # ③ universal — 클러스터 포트
    "RAY_PORT": "6379",
}

# source-build 가드 키: (NGC 베이스 태그, vLLM 버전) 튜플.
#   변별자는 torch 핀이 아니라 **NGC 베이스(=실-링크 torch) × vLLM source 버전**이다.
#   use_existing_torch.py 가 pyproject torch 핀을 버리고 NGC torch 를 링크하므로,
#   pyproject 핀(예 0.23.0 → 2.11.0)은 변별력이 없다. testlog 증거:
#     (26.03-py3, 0.22.1)=성공(strip-hoist·setuptools-rust 수동·use_existing_torch),
#     (26.05-py3, 0.23.0)=성공(torch 2.12, strip-hoist 자동 skip) — E2E 검증,
#     (26.03-py3, 0.23.0)=FAIL(torch 2.11, Tensor::layout() 부재) ← 옛 '0.23.0 동일 torch2.11 캐리' 가정의 반증.
#   미인식 키는 빌드를 명시적으로 실패시킨다(false determinism 방지 — plan rev3 §5 / SKILL.md §4.6 HITL 발견 루프 유도).
#   P6: 이 인라인 셋을 source_build_patches.yaml + 패치-리졸버 페르소나로 승급.
VALIDATED_SOURCE_BUILD_KEYS = {("26.03-py3", "0.22.1"), ("26.05-py3", "0.23.0")}

_PLACEHOLDER_RE = re.compile(r"\{\{\s*[A-Z0-9_]+\s*\}\}")


# ── 입력 로더 ────────────────────────────────────────────────────────────────
def load_resolved(path: str) -> dict:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def load_manifest(path: str) -> dict:
    """manifest.yaml 로드. pyyaml 있으면 사용, 없으면 flat-YAML 미니파서(stdlib)."""
    try:
        import yaml  # type: ignore
        with open(path, encoding="utf-8") as f:
            return yaml.safe_load(f) or {}
    except Exception:
        d = _load_yaml_flat(path)
        ic = _parse_interconnect_block(path)   # 폴백: 중첩 interconnect 블록 보강(Plan 2 — flat 파서가 못 읽음)
        if ic:
            d["interconnect"] = ic
        nodes = _parse_nodes_block(path)       # 폴백: 중첩 nodes[] 블록 보강(S6 — .env.cluster 가 소비)
        if nodes:
            d["nodes"] = nodes
        return d


def _load_yaml_flat(path: str) -> dict:
    """flat scalar YAML 만 처리(우리 manifest 는 평탄): 'key: value  # 주석' → {key: value}.
    'key: []' → []. 블록/중첩은 무시(render 는 scalar 키만 사용)."""
    out = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            s = line.rstrip("\n")
            if not s.strip() or s.lstrip().startswith("#"):
                continue
            if s[0] in (" ", "\t", "-"):  # 들여쓰기/리스트 항목 = 중첩 → skip
                continue
            if ":" not in s:
                continue
            key, val = s.split(":", 1)
            key = key.strip()
            val = _strip_inline_comment(val).strip()
            if val in ("[]", ""):
                out[key] = [] if val == "[]" else ""
                continue
            if (val[0] == val[-1]) and val[0] in ("'", '"'):
                val = val[1:-1]
            out[key] = val
    return out


def _strip_inline_comment(val: str) -> str:
    """따옴표 밖의 ' #...' 트레일링 주석 제거(경로엔 # 없음 전제)."""
    in_q = ""
    for i, ch in enumerate(val):
        if ch in ("'", '"'):
            in_q = "" if in_q == ch else (in_q or ch)
        elif ch == "#" and not in_q and i > 0 and val[i - 1] == " ":
            return val[:i]
    return val


def _parse_interconnect_block(path: str) -> dict:
    """stdlib 폴백 전용: manifest 의 중첩 `interconnect:` 블록만 파싱(pyyaml 부재 시).
    flat 파서(_load_yaml_flat)가 들여쓰기를 스킵하므로 interconnect 를 별도로 보강한다.
    list(`[a, b]`)·scalar·null 처리. 다른 최상위 키를 만나면 블록 종료."""
    ic: dict = {}
    in_block = False
    with open(path, encoding="utf-8") as f:
        for line in f:
            raw = line.rstrip("\n")
            if not raw.strip() or raw.lstrip().startswith("#"):
                continue
            indent = len(raw) - len(raw.lstrip(" \t"))
            stripped = raw.strip()
            if indent == 0:                       # 최상위 키 — interconnect 진입/이탈
                in_block = stripped.startswith("interconnect:")
                continue
            if not in_block or stripped.startswith("-") or ":" not in stripped:
                continue
            key, val = stripped.split(":", 1)
            key = key.strip()
            val = _strip_inline_comment(val).strip()
            if val.startswith("[") and val.endswith("]"):
                inner = val[1:-1].strip()
                ic[key] = [x.strip().strip("'\"") for x in inner.split(",") if x.strip()] if inner else []
            elif val in ("", "null", "~"):
                ic[key] = None
            else:
                if (val[0] == val[-1]) and val[0] in ("'", '"'):
                    val = val[1:-1]
                ic[key] = val
    return ic


def _parse_nodes_block(path: str) -> list:
    """stdlib 폴백 전용: manifest 의 중첩 `nodes:` 블록만 파싱(pyyaml 부재 시).
    각 `- role: X` 가 새 노드 시작, 하위 `key: val` 누적. 다른 최상위 키를 만나면 종료.
    (render_sub_env.parse_manifest 와 동일 원칙 — 이 스크립트 전용 미니파서.)"""
    nodes: list = []
    in_block = False
    cur: dict = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            raw = line.rstrip("\n")
            if not raw.strip() or raw.lstrip().startswith("#"):
                continue
            indent = len(raw) - len(raw.lstrip(" \t"))
            stripped = raw.strip()
            if indent == 0:                       # 최상위 키 — nodes 진입/이탈
                in_block = stripped.startswith("nodes:")
                continue
            if not in_block:
                continue
            if stripped.startswith("- "):         # 새 노드 항목 (- role: main)
                if cur:
                    nodes.append(cur)
                cur = {}
                stripped = stripped[2:].strip()    # '- ' 제거 후 첫 key:val 처리
            if ":" not in stripped:
                continue
            key, val = stripped.split(":", 1)
            val = _strip_inline_comment(val).strip()
            if (val and val[0] == val[-1]) and val[0] in ("'", '"'):
                val = val[1:-1]
            cur[key.strip()] = val
    if cur:
        nodes.append(cur)
    return nodes


# ── 컨텍스트 빌드 ─────────────────────────────────────────────────────────────
def _compact_cuda(cuda: str) -> str:
    """'13.2.0.046' → '132' · '13.0' → '130' · '129' → '129'."""
    cuda = str(cuda)
    if "." in cuda:
        parts = cuda.split(".")
        return f"{parts[0]}{parts[1]}"
    return cuda


def _patch_guard(ngc_tag: str, vllm_version: str) -> str:
    key = (ngc_tag, vllm_version)
    if key in VALIDATED_SOURCE_BUILD_KEYS:
        return (f'RUN echo "[guard] source-build key ({ngc_tag} x vLLM {vllm_version}): '
                f'validated patch set ((ngc_base x vllm_version) keyed) — proceeding."')
    return (
        'RUN echo "ERROR: source-build key (' + ngc_tag + ' x vLLM ' + vllm_version + ') has NO validated patch set." && \\\n'
        '    echo "  -> run SKILL.md §4.6 HITL discovery loop, then graduate the verified" && \\\n'
        '    echo "     patches into Dockerfile.source-build.template ((ngc_base x vllm_version) guarded, P6 catalog)." && \\\n'
        '    echo "  -> refusing to build on unvalidated determinism (plan rev3 §5)." && exit 1'
    )


def build_context(manifest: dict, resolved: dict) -> dict:
    """manifest + resolved → 플레이스홀더 치환 맵."""
    vllm = str(resolved["vllm_version"])
    arch = str(manifest["cpu_arch"])
    torch_pin = str(resolved.get("torch", {}).get("pin", ""))
    ngc = resolved.get("ngc_base", {})
    ngc_tag = str(ngc.get("tag", ""))
    decision = str(resolved.get("build_track", {}).get("decision", ""))
    track = "source" if "source" in decision else "wheel"
    wheel = resolved.get("wheel", {})

    if track == "source":
        cuda = _compact_cuda(ngc.get("cuda_version", "")) or "src"
        dockerfile = "Dockerfile.source-build"
    else:
        cuda = str(wheel.get("cuda", "") or "")
        dockerfile = "Dockerfile"

    image_tag = f"{vllm}-cu{cuda}-{arch}-{track}"

    return {
        "VLLM_VERSION": vllm,
        "CPU_ARCH": arch,
        "NGC_TAG": ngc_tag,
        "TORCH_PIN": torch_pin,
        "VLLM_REF": "v" + vllm,
        "TORCH_CUDA_ARCH": str(resolved.get("source_build", {}).get("torch_cuda_arch", "")),
        "NAS_MODEL_PATH": str(manifest.get("nas_model_path", "")),
        "IMAGE_NAME": IMAGE_NAME,
        "IMAGE_TAG": image_tag,
        "DOCKERFILE": dockerfile,
        "SOURCE_BUILD_PATCH_GUARD": _patch_guard(ngc_tag, vllm),
        "CUDA_VERSION": str(wheel.get("cuda", "") or ""),
        "VLLM_MANYLINUX": str(wheel.get("manylinux", "") or ""),
    }


# ── NCCL envfile 빌드 (Plan 2 — manifest.interconnect + preset → 17 KEY=VALUE) ───────────────
def build_nccl_env(manifest: dict) -> dict:
    """manifest.interconnect + NCCL_PRESETS + NCCL_INVARIANTS → NCCL env 17개 dict.
    결정론적 lookup·문자열포맷만(확률 추론 없음). 결손/미지 키는 fail-loud."""
    ic = manifest.get("interconnect")
    if not ic:
        raise ValueError("manifest.interconnect 부재 — NCCL 렌더 불가(fail-loud, 무증거 진행 금지)")

    hca = ic.get("hca_devices")
    gid = ic.get("gid_index")
    iface = ic.get("socket_iface")
    for name, v in (("hca_devices", hca), ("gid_index", gid), ("socket_iface", iface)):
        if v is None or v == "" or v == []:
            raise ValueError(f"manifest.interconnect.{name} 결손 — fail-loud")

    hca_list = [h.strip() for h in hca.split(",") if h.strip()] if isinstance(hca, str) else list(hca)

    env: dict = {}
    # ① 환경값 (manifest.interconnect)
    env["NCCL_IB_HCA"] = "=" + ",".join(hca_list)   # "=" prefix = NCCL 정확매칭(prefix-매칭 경로꼬임 방지)
    env["NCCL_IB_GID_INDEX"] = str(gid)
    for k in _IFACE_ENV_KEYS:
        env[k] = str(iface)
    # ② 프리셋 (platform_preset 역참조 — 미지키 fail-loud)
    preset_key = ic.get("platform_preset")
    if preset_key not in NCCL_PRESETS:
        raise KeyError(f"platform_preset '{preset_key}' 미정의 — 알려진 {sorted(NCCL_PRESETS)} "
                       f"(fail-loud; 확장은 배포자 코드에이전트가 NCCL_PRESETS 에 추가)")
    env.update(NCCL_PRESETS[preset_key])
    # ③ 불변
    env.update(NCCL_INVARIANTS)
    return env


def render_nccl_envfile(manifest: dict) -> str:
    """build_nccl_env → flat KEY=VALUE envfile 문자열(키 정렬=결정론). compose 가 env_file 로 참조."""
    env = build_nccl_env(manifest)
    preset_key = manifest["interconnect"].get("platform_preset")
    header = [
        "# .env.interconnect — manifest-driven NCCL/RDMA env (render_dockerfile.py 생성·비추적).",
        f"# 원천: manifest.interconnect(①환경값) + NCCL_PRESETS[{preset_key}](②프리셋) + NCCL_INVARIANTS(③불변).",
        "# 회귀 oracle: fixtures/nccl_env_dgx-spark-gb10.golden (집합 동치). 손수정 금지 — manifest/preset 을 고칠 것.",
        "",
    ]
    body = [f"{k}={env[k]}" for k in sorted(env)]
    return "\n".join(header + body) + "\n"


# ── 클러스터 envfile 빌드 (S6 — manifest.nodes + preset → 8 KEY=VALUE) ────────────────────────
def build_cluster_env(manifest: dict) -> dict:
    """manifest.nodes[] + CLUSTER_PRESETS + CLUSTER_INVARIANTS → Ray 클러스터-배포 env 8개 dict.
    결정론적 lookup·문자열포맷만. 결손/미지 키 fail-loud(무증거 진행 금지)."""
    nodes = manifest.get("nodes")
    if not nodes:
        raise ValueError("manifest.nodes[] 부재 — .env.cluster 렌더 불가(fail-loud, 무증거 진행 금지)")
    by_role: dict = {}
    for n in nodes:
        r = str(n.get("role", "")).strip()
        if r:
            by_role[r] = n
    main, sub = by_role.get("main"), by_role.get("sub")
    if not main or not sub:
        raise ValueError(f"manifest.nodes[] 에 role=main·sub 둘 다 필요(멀티 분산) — got roles={sorted(by_role)}")
    master_ip = str(main.get("host", "")).strip()
    slave_ip = str(sub.get("host", "")).strip()
    ssh_user = str(main.get("ssh_user", "") or sub.get("ssh_user", "")).strip()
    if not master_ip or not slave_ip or not ssh_user:
        raise ValueError(f"nodes[] host/ssh_user 결손 — master={master_ip!r} slave={slave_ip!r} ssh_user={ssh_user!r}")
    env: dict = {}
    # ① 환경값 (manifest.nodes)
    env["MASTER_HOST_IP"] = master_ip
    env["SLAVE_HOST_IP"] = slave_ip
    env["SSH_USER"] = ssh_user
    # ② 프리셋 (platform_preset 역참조 — NCCL 과 동일 키, 미지 fail-loud)
    preset_key = (manifest.get("interconnect") or {}).get("platform_preset")
    if preset_key not in CLUSTER_PRESETS:
        raise KeyError(f"platform_preset '{preset_key}' 미정의 — 알려진 {sorted(CLUSTER_PRESETS)} "
                       f"(fail-loud; 확장은 배포자 코드에이전트가 CLUSTER_PRESETS 에 추가)")
    env.update(CLUSTER_PRESETS[preset_key])
    # ③ 불변
    env.update(CLUSTER_INVARIANTS)
    return env


def render_cluster_envfile(manifest: dict) -> str:
    """build_cluster_env → flat KEY=VALUE envfile 문자열(키 정렬=결정론). compose master/slave 가 env_file 로 참조."""
    env = build_cluster_env(manifest)
    preset_key = (manifest.get("interconnect") or {}).get("platform_preset")
    header = [
        "# .env.cluster — manifest-driven Ray 클러스터-배포 env (render_dockerfile.py --cluster-envfile 생성·비추적).",
        f"# 원천: manifest.nodes[](①환경값) + CLUSTER_PRESETS[{preset_key}](②프리셋) + CLUSTER_INVARIANTS(③불변).",
        "# Band2(토폴로지-keyed, 서브 전파). 손수정 금지 — manifest.nodes/preset 을 고칠 것.",
        "",
    ]
    body = [f"{k}={env[k]}" for k in sorted(env)]
    return "\n".join(header + body) + "\n"


def _parse_env_pairs(text: str) -> dict:
    """KEY=VALUE envfile → dict(주석·공백·빈줄 무시, 첫 '='로 split). 집합 동치 비교용."""
    out: dict = {}
    for line in text.splitlines():
        s = line.strip()
        if not s or s.startswith("#"):
            continue
        k, _, v = s.partition("=")
        out[k.strip()] = v.strip()
    return out


# ── 통로 materialize (plan_2026062321_1 — 통로 self-containment) ───────────────
def _repo_root() -> str:
    """이 스크립트(.claude/skills/upstream-version-watch/scripts/) 기준 repo 루트(4단계 상위)."""
    return os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", ".."))


def materialize_configs(repo: str, topology: str) -> list:
    """repo-root configs/ 의 러너 스크립트(정본) → output/<topology>/configs/ 복사(통로 self-containment, I1/I2).
    멱등(덮어쓰기) · 실행권한 보존 · 정본 부재 시 fail-loud. 반환: 복사한 대상 경로 리스트.
    compose 가 통로(`./configs`)만 단일 마운트하면 되도록 통로를 완결시킨다(런타임 mount-overlay 폐기)."""
    src_dir = os.path.join(repo, "configs")
    dst_dir = os.path.join(repo, "output", topology, "configs")
    os.makedirs(dst_dir, exist_ok=True)
    copied = []
    for name in RUNNER_SCRIPTS:
        src = os.path.join(src_dir, name)
        if not os.path.isfile(src):
            raise FileNotFoundError(f"러너 스크립트 정본 부재: {src} (repo-root configs/ — tracked 빌딩블럭)")
        dst = os.path.join(dst_dir, name)
        shutil.copyfile(src, dst)
        shutil.copymode(src, dst)   # 실행권한 보존
        copied.append(dst)
    return copied


def materialize_env(repo: str, topology: str, manifest: dict) -> str:
    """manifest.nas_model_path → output/<topology>/.env (프로젝트-레벨 compose 변수치환용).

    docker compose 가 docker-compose.yaml 의 ${NAS_MODEL_PATH}·${TIKTOKEN_HOST_PATH} 치환에 쓰는
    프로젝트 .env 를 manifest 에서 생성한다. 이게 없으면 serve 가 compose 기본값(/mnt/models)을 마운트해
    모델을 못 찾는다(check_smoke_model.py 도 동일 정본=manifest 직독). PII(NAS 경로) 포함 → output/* gitignored.
    근거: testlog_2026062422_1 결함#2(serve-time NAS 미전파). nas_model_path 부재 시 fail-loud(무증거 진행 금지)."""
    nas = str(manifest.get("nas_model_path", "")).strip()
    if not nas:
        raise ValueError(
            "manifest.nas_model_path 부재 — output/%s/.env materialize 불가. "
            "serve 가 compose 기본값 /mnt/models 를 마운트해 모델을 못 찾는다. manifest 를 채울 것." % topology)
    # tiktoken·quant 도 manifest 정본 우선(env > manifest > 리터럴 default — 헌법 §serve-time env 통로 불변식).
    # plan_2026063018_1: P1 이 manifest 에 tiktoken_host_path·quant_model_path 필드 추가 → 여기서 .env 로 materialize.
    tiktoken = str(manifest.get("tiktoken_host_path", "") or "").strip() or os.path.join(repo, "tiktoken_cache")
    quant = str(manifest.get("quant_model_path", "") or "").strip() or nas  # 미설정 시 NAS 루트 폴백
    dst_dir = os.path.join(repo, "output", topology)
    os.makedirs(dst_dir, exist_ok=True)
    dst = os.path.join(dst_dir, ".env")
    body = (
        "# 프로젝트-레벨 env (compose 변수치환) — render_dockerfile.py --materialize-env 가 manifest 에서 생성.\n"
        "# docker compose 가 docker-compose.yaml 의 ${NAS_MODEL_PATH}·${QUANT_MODEL_PATH}·${TIKTOKEN_HOST_PATH} 치환에 사용.\n"
        "# gitignored(output/* — PII). 손수정 금지 — manifest(nas_model_path·quant_model_path·tiktoken_host_path)를 고칠 것.\n"
        "NAS_MODEL_PATH=%s\n"
        "QUANT_MODEL_PATH=%s\n"
        "TIKTOKEN_HOST_PATH=%s\n"
    ) % (nas, quant, tiktoken)
    with open(dst, "w", encoding="utf-8") as f:
        f.write(body)
    return dst


# ── 렌더 ─────────────────────────────────────────────────────────────────────
def _substitute(text: str, context: dict) -> str:
    for key, val in context.items():
        text = re.sub(r"\{\{\s*" + re.escape(key) + r"\s*\}\}", lambda _m, v=val: v, text)
    leftover = _PLACEHOLDER_RE.findall(text)
    if leftover:
        raise ValueError(f"미치환 플레이스홀더 잔존(컨텍스트 누락): {sorted(set(leftover))}")
    return text


def render(template_path: str, manifest: dict, version_resolution: dict) -> str:
    """스켈레톤 + manifest/해소값 → 완성 산출물 문자열. (G2 구현)

    Args:
        template_path: Dockerfile.template / Dockerfile.source-build.template /
            docker-compose.template.yaml 등 스켈레톤 경로.
        manifest: 테라포밍된 환경 매니페스트 dict(cpu_arch·nas_model_path 등).
        version_resolution: resolve_*.py 산출 dict(resolved.json 구조).

    Returns:
        {{ PLACEHOLDER }} 가 모두 치환된 산출물 문자열.

    Raises:
        ValueError: 미치환 플레이스홀더가 남으면(컨텍스트 누락) fail-loud.
    """
    with open(template_path, encoding="utf-8") as f:
        text = f.read()
    ctx = build_context(manifest, version_resolution)
    out = _substitute(text, ctx)
    # wheel 트랙 무결성: wheel URL 의 cuXXX 가 비면(데이터 누락) fail-loud
    if "vllm-${VLLM_VERSION}+cu${CUDA_VERSION}" in text and not ctx.get("CUDA_VERSION"):
        raise ValueError("wheel 트랙인데 resolved.wheel.cuda 부재 → wheel URL 불완전")
    return out


# ── self-test (A7 게이트가 호출) ──────────────────────────────────────────────
def _self_test() -> None:
    tpl = ("FROM nvcr.io/nvidia/pytorch:{{ NGC_TAG }}\n"
           "{{ SOURCE_BUILD_PATCH_GUARD }}\n"
           "ARG VLLM_REF={{ VLLM_REF }}\n"
           "# arch={{ CPU_ARCH }} tag={{ IMAGE_TAG }} torch={{ TORCH_PIN }} cuarch={{ TORCH_CUDA_ARCH }}\n")
    man = {"cpu_arch": "aarch64", "nas_model_path": "/nas"}
    # success 픽스처 ①: 검증된 (NGC 26.03, vLLM 0.22.1)
    res_a = {"vllm_version": "0.22.1", "torch": {"pin": "2.11.0"},
             "ngc_base": {"tag": "26.03-py3", "cuda_version": "13.2.0.046"},
             "build_track": {"decision": "source-build"},
             "source_build": {"torch_cuda_arch": "12.1a"}}
    out_a = _substitute(tpl, build_context(man, res_a))
    assert "{{" not in out_a, "leftover placeholder"
    assert "26.03-py3" in out_a, "ngc tag (a)"
    assert "0.22.1-cu132-aarch64-source" in out_a, f"image tag (a): {out_a!r}"
    assert "validated patch set" in out_a, "guard(validated) for (26.03, 0.22.1)"
    # success 픽스처 ②: E2E 검증 (NGC 26.05, vLLM 0.23.0) — pyproject torch핀은 2.11.0이나 실-링크 torch는 2.12
    res_b = {"vllm_version": "0.23.0", "torch": {"pin": "2.11.0"},
             "ngc_base": {"tag": "26.05-py3", "cuda_version": "13.2.0.046"},
             "build_track": {"decision": "source-build"},
             "source_build": {"torch_cuda_arch": "12.1a"}}
    out_b = _substitute(tpl, build_context(man, res_b))
    assert "26.05-py3" in out_b, "ngc tag (b)"
    assert "0.23.0-cu132-aarch64-source" in out_b, f"image tag (b): {out_b!r}"
    assert "validated patch set" in out_b, "guard(validated) for (26.05, 0.23.0)"
    # fail-loud 픽스처: (NGC 26.03, vLLM 0.23.0) = testlog 가 FAIL 로 증명 → 가드가 빌드 실패시켜야 함
    res_bad = {"vllm_version": "0.23.0", "torch": {"pin": "2.11.0"},
               "ngc_base": {"tag": "26.03-py3", "cuda_version": "13.2.0.046"},
               "build_track": {"decision": "source-build"},
               "source_build": {"torch_cuda_arch": "12.1a"}}
    out_bad = _substitute(tpl, build_context(man, res_bad))
    assert "exit 1" in out_bad, "guard(fail-loud) for (26.03, 0.23.0)"
    print("[render] self-test OK — source-build 렌더 + (NGC베이스×vLLM버전) 키 가드(검증x2/fail-loud) 정상")

    # ── NCCL envfile 회귀(Plan 2 S2): golden 대비 KEY=VALUE 집합 동치 + fail-loud ──
    golden_path = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                               "..", "fixtures", "nccl_env_dgx-spark-gb10.golden")
    with open(golden_path, encoding="utf-8") as f:
        golden = _parse_env_pairs(f.read())
    man_ic = {"interconnect": {"type": "RoCE v2",
                               "hca_devices": ["rocep1s0f1", "roceP2p1s0f1"],
                               "gid_index": 3, "socket_iface": "enp1s0f1np1",
                               "bandwidth_gbps": 208.2, "platform_preset": "dgx-spark-gb10"}}
    rendered = _parse_env_pairs(render_nccl_envfile(man_ic))
    if rendered != golden:
        only_r = {k: rendered[k] for k in rendered if golden.get(k) != rendered[k]}
        only_g = {k: golden[k] for k in golden if rendered.get(k) != golden[k]}
        raise AssertionError(f"NCCL 렌더 != golden(집합 동치 위반)\n  rendered-side={only_r}\n  golden-side={only_g}")
    assert len(rendered) == 17, f"NCCL 17키 기대, got {len(rendered)}"
    # fail-loud ①: 미지 platform_preset → KeyError
    try:
        build_nccl_env({"interconnect": {**man_ic["interconnect"], "platform_preset": "no-such-preset"}})
        raise AssertionError("미지 platform_preset 인데 통과(fail-loud 위반)")
    except KeyError:
        pass
    # fail-loud ②: interconnect 필드 결손 → ValueError
    try:
        build_nccl_env({"interconnect": {"platform_preset": "dgx-spark-gb10"}})
        raise AssertionError("interconnect 필드 결손인데 통과(fail-loud 위반)")
    except ValueError:
        pass
    print("[render] NCCL self-test OK — .env.interconnect 17키 == golden 집합 동치 · 미지preset/결손 fail-loud 정상")

    # ── 클러스터 envfile 회귀(S6): golden 대비 집합 동치 + fail-loud (노드값=RFC5737 합성=PII-free) ──
    cgolden_path = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "..", "fixtures", "cluster_env_dgx-spark-gb10.golden")
    with open(cgolden_path, encoding="utf-8") as f:
        cgolden = _parse_env_pairs(f.read())
    man_nodes = {"interconnect": {"platform_preset": "dgx-spark-gb10"},
                 "nodes": [{"role": "main", "host": "198.51.100.10", "ssh_user": "deployer"},
                           {"role": "sub", "host": "198.51.100.11", "ssh_user": "deployer"}]}
    crendered = _parse_env_pairs(render_cluster_envfile(man_nodes))
    if crendered != cgolden:
        only_r = {k: crendered[k] for k in crendered if cgolden.get(k) != crendered[k]}
        only_g = {k: cgolden[k] for k in cgolden if crendered.get(k) != cgolden[k]}
        raise AssertionError(f"cluster 렌더 != golden(집합 동치 위반)\n  rendered-side={only_r}\n  golden-side={only_g}")
    assert len(crendered) == 9, f"cluster 9키 기대, got {len(crendered)}"   # 8→9: MAX_JOBS Band2 재귀속(plan_2026062811_2)
    try:                                   # fail-loud ①: 미지 platform_preset → KeyError
        build_cluster_env({**man_nodes, "interconnect": {"platform_preset": "no-such"}})
        raise AssertionError("미지 platform_preset 인데 통과(fail-loud 위반)")
    except KeyError:
        pass
    try:                                   # fail-loud ②: sub 노드 결손 → ValueError
        build_cluster_env({"interconnect": {"platform_preset": "dgx-spark-gb10"},
                           "nodes": [{"role": "main", "host": "198.51.100.10", "ssh_user": "deployer"}]})
        raise AssertionError("sub 노드 결손인데 통과(fail-loud 위반)")
    except ValueError:
        pass
    print("[render] cluster self-test OK — .env.cluster 9키 == golden 집합 동치 · 미지preset/노드결손 fail-loud 정상")

    # ── materialize self-test (plan_2026062321_1): 정본 → 통로 복사 멱등·실행권한·fail-loud ──
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        os.makedirs(os.path.join(td, "configs"))
        os.makedirs(os.path.join(td, "output", "multi"))
        for name in RUNNER_SCRIPTS:
            p = os.path.join(td, "configs", name)
            with open(p, "w", encoding="utf-8") as f:
                f.write(f"#!/bin/bash\n# {name}\n")
            os.chmod(p, 0o755)
        copied = materialize_configs(td, "multi")
        assert len(copied) == len(RUNNER_SCRIPTS), "materialize 복사 수"
        for name in RUNNER_SCRIPTS:
            dst = os.path.join(td, "output", "multi", "configs", name)
            assert os.path.isfile(dst), f"materialize 대상 부재: {name}"
            assert os.access(dst, os.X_OK), f"실행권한 미보존: {name}"
        materialize_configs(td, "multi")  # 멱등(재호출 OK)
        os.remove(os.path.join(td, "configs", RUNNER_SCRIPTS[0]))
        try:
            materialize_configs(td, "multi")
            raise AssertionError("정본 부재인데 통과(fail-loud 위반)")
        except FileNotFoundError:
            pass
    print("[render] materialize self-test OK — 러너 스크립트 통로 복사(멱등·권한·fail-loud) 정상")


def main() -> None:
    ap = argparse.ArgumentParser(description="render_dockerfile.py — G2 결정론 렌더러")
    ap.add_argument("--self-test", action="store_true", help="내장 self-test(A7 게이트 + NCCL 회귀)")
    ap.add_argument("--nccl-envfile", action="store_true",
                    help="NCCL .env.interconnect 렌더(manifest.interconnect 소비, Plan 2)")
    ap.add_argument("--cluster-envfile", action="store_true",
                    help="Ray .env.cluster 렌더(manifest.nodes[]+CLUSTER_PRESETS 소비, S6 env-split)")
    ap.add_argument("--materialize-configs", action="store_true",
                    help="러너 스크립트(serve_runner/debug-init)를 output/<topology>/configs/ 로 복사(통로 self-containment, plan_2026062321_1)")
    ap.add_argument("--materialize-env", action="store_true",
                    help="output/<topology>/.env 를 manifest(nas_model_path)+tiktoken_cache 에서 생성(serve-time NAS 마운트 정합, 결함#2)")
    ap.add_argument("--topology", choices=["single", "multi"], help="--materialize-configs/--materialize-env 대상 통로")
    ap.add_argument("--repo", help="repo 루트(미지정 시 스크립트 위치 기준 자동)")
    ap.add_argument("--template", help="템플릿 경로")
    ap.add_argument("--manifest", default="manifest.yaml")
    ap.add_argument("--resolved", default="resolved.json")
    ap.add_argument("-o", "--out", help="출력 파일(미지정 시 stdout)")
    a = ap.parse_args()

    if a.self_test:
        _self_test()
        return

    if a.nccl_envfile:
        out = render_nccl_envfile(load_manifest(a.manifest))
        if a.out:
            with open(a.out, "w", encoding="utf-8") as f:
                f.write(out)
            print(f"[render] NCCL envfile → {a.out} ({len(_parse_env_pairs(out))} keys)", file=sys.stderr)
        else:
            sys.stdout.write(out)
        return

    if a.cluster_envfile:
        out = render_cluster_envfile(load_manifest(a.manifest))
        if a.out:
            with open(a.out, "w", encoding="utf-8") as f:
                f.write(out)
            print(f"[render] cluster envfile → {a.out} ({len(_parse_env_pairs(out))} keys)", file=sys.stderr)
        else:
            sys.stdout.write(out)
        return

    if a.materialize_configs:
        if not a.topology:
            print("[render] FAIL: --materialize-configs 에는 --topology {single|multi} 필요", file=sys.stderr)
            sys.exit(2)
        copied = materialize_configs(a.repo or _repo_root(), a.topology)
        for c in copied:
            print(f"[render] materialize → {c}", file=sys.stderr)
        return

    if a.materialize_env:
        if not a.topology:
            print("[render] FAIL: --materialize-env 에는 --topology {single|multi} 필요", file=sys.stderr)
            sys.exit(2)
        env_path = materialize_env(a.repo or _repo_root(), a.topology, load_manifest(a.manifest))
        print(f"[render] materialize env → {env_path}", file=sys.stderr)
        return

    if not a.template:
        _self_test()
        return

    manifest = load_manifest(a.manifest)
    resolved = load_resolved(a.resolved)
    out = render(a.template, manifest, resolved)
    if a.out:
        with open(a.out, "w", encoding="utf-8") as f:
            f.write(out)
        print(f"[render] {a.template} → {a.out} ({len(out)} bytes)", file=sys.stderr)
    else:
        sys.stdout.write(out)


if __name__ == "__main__":
    main()
