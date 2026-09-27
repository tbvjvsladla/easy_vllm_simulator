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
  {{ IMAGE_TAG }}       ← {vllm}-cu{cuda}-{arch}-{track}  (버전-키드)
  {{ DOCKERFILE }}      ← 트랙별 ("Dockerfile" | "Dockerfile.source-build")
  {{ SOURCE_BUILD_PATCH_GUARD }} ← (NGC베이스×vLLM버전) 키 가드 (검증된 키=주석 / 미인식=빌드 명시 실패)

설계: 결정론 resolve 는 스크립트, 패치 SELECT 는 검토 루프(판단계층). 아래
VALIDATED_SOURCE_BUILD_KEYS 는 P6 카탈로그(source_build_patches.yaml) 승급 전의
최소 인라인 가드다(rev3). stdlib 만 사용. manifest.yaml 은 pyyaml 있으면 사용, 없으면 flat 미니파서.
"""

import sys
import os
import stat
import re
import json
import shutil
import subprocess
import argparse

IMAGE_NAME = "easy-vllm"

# Topology-neutral policy/production SSOT. Delivered root configs/ and output/<topology>/ are
# derivatives only; both branches consume these synchronized source assets.
SKILL_ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
SHARED_ASSET_DIR = os.path.join(SKILL_ROOT, "assets", "configs")
SHARED_TEMPLATE_DIR = os.path.join(SKILL_ROOT, "templates")
SHARED_RESOLUTION = os.path.join(SKILL_ROOT, "assets", "current-production-resolution.json")
RUNNER_SCRIPTS = ("serve_runner.sh", "debug-init.sh", "arm_patch.sh")
_SHARED_TEMPLATES = {
    ("single", "dockerfile"): "Dockerfile.template",
    ("multi", "dockerfile"): "Dockerfile.template",
    ("single", "source-build"): "Dockerfile.source-build.template",
    ("multi", "source-build"): "Dockerfile.source-build.template",
    ("single", "compose"): "docker-compose.single.template.yaml",
    ("multi", "compose"): "docker-compose.multi.template.yaml",
}

# ── NCCL/RDMA 통신 env (Plan 2: 당시 docker-compose 하드코딩 17개 → manifest-driven 렌더) ─────
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
        # GB10/CUDA13: forcing SYS selects an RDMA registration path that failed on both
        # large and small TP=2 models (`ibv_reg_mr_iova2: Cannot allocate memory`). LOC is the
        # approved OFAT diagnostic; performance remains a separate benchmark question.
        "NCCL_NET_GDR_LEVEL": "LOC",
        # LOC alone still selected DMABUF GDR on GB10 because the C2C override remained active.
        # Approved second OFAT: disable only that override before considering broader GDR disablement.
        "NCCL_NET_GDR_C2C": "0",
        # LOC+C2C=0+DMABUF=0 still failed general ibv_reg_mr_iova2 registration.
        # Approved final fine-grained OFAT: disable GPU-buffer GDR reads while retaining IB.
        "NCCL_NET_GDR_READ": "0",
        # LOC+C2C=0 still selected `GPU Direct RDMA (DMABUF)` and failed ibv_reg_mr_iova2.
        # Approved third OFAT disables only DMA-BUF registration while retaining IB/RoCE.
        "NCCL_DMABUF_ENABLE": "0",
        "NCCL_CROSS_NIC": "1",
    },
    # 튜닝 0 프리셋 — 비-DGX 플랫폼의 "시도→comms 스모크 중재" 경로(전방호환 시도-우선 따름정리 ·
    # plan_26070208 [C]#3). ①환경값(manifest.interconnect)+③불변만 방출 = NCCL 기본값으로 일단 돌려본다.
    # 성능 튜닝은 스모크 통과 후 플랫폼 프리셋으로 승격(무증거 튜닝 금지는 유지).
    "generic": {},
}
NCCL_INVARIANTS = {                     # ③ universal — 인터커넥트 무관 디버그/안전
    "NCCL_DEBUG": "INFO",
    "NCCL_DEBUG_SUBSYS": "INIT,NET,GRAPH,ENV",
}
# ① 전송 선택 = manifest.interconnect.nccl_transport (2026-09-28 · plan_26092808 · 사용자 승인).
#   09-17(782fd70)은 `NCCL_NET=Socket` 을 ③불변으로 박았다 — NCCL_IB_DISABLE 이 외부 IBext 플러그인을 막지 못해서다.
#   그 결과 RoCE 로 되돌릴 **정식 통로가 0** 이 됐다: sync_to_sub 는 렌더러로 다시 생성해 배달하므로 생성물 손수정은
#   서브에 닿지 않고, 서브 직접 수정은 노드 제어 ① 위반이다(D3 — 막힌 경로는 우회가 아니라 경로를 고친다).
#   · 키 부재 = "socket"(종전 기능 기준선 · golden 불변: GB10 TP=2 에서 세밀한 GDR 제어로도 ibv_reg_mr_iova2 가 실패해
#     Socket 으로 Ray/vLLM 기능을 IB 경로에서 분리했다 — IB_DISABLE 만으로는 외부 IBext 플러그인이 막히지 않아 NET 도 명시)
#   · "rdma" = NCCL 내장 verbs(IB_DISABLE=0 · NET=IB). 포럼 383023 의 실패 지점 `misc/ibvwrap.cc (wrap_ibv_reg_mr_iova2)` 가
#     이 내장 경로다. ★ 첫 교정(2e91a1e)은 rdma 를 "NET 미방출 + IB_DISABLE=1"(09-09 조건 모방)로 정의했는데, 현 이미지
#     (NCCL_NET_PLUGIN=spcx · 09-24 재빌드)에서는 플러그인이 장치를 거부해 **조용히 Socket** 으로 떨어졌다(엔진 로그
#     `Using network Socket` 실측 · 2026-09-28). 이름이 rdma 인데 Socket 을 재면 거짓 판정이다 → 전송을 명시한다.
#   · 그 밖의 값 = fail-loud.
NCCL_TRANSPORTS = {
    "socket": {"NCCL_IB_DISABLE": "1", "NCCL_NET": "Socket"},
    "rdma": {"NCCL_IB_DISABLE": "0", "NCCL_NET": "IB"},
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
                                        #   슬레이브도 model shard 로드·JIT 하므로 Band2(cluster)서 양노드 도달해야 함(plan_26062811_30_33 — 슬레이브 Band2-only 완결).
                                        #   이미지 ENV 기본 16 override. 비-MoE 모델엔 no-op(안전 보수 상수). 모델별 override 필요시 .env.<model>(master) 에서.
    },
    # 튜닝 0 — NCCL_PRESETS["generic"] 과 동형(③불변 RAY_PORT 만 방출). 비-DGX "시도→스모크 중재" 경로.
    "generic": {},
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
#     (26.05-py3, 0.24.0)=검증(torch 2.11.0 핀·NGC 26.05 동일, source-build 레시피 byte-동일 — 0.23.0 twin.
#         0.24.0=#43477 DeepSeek-V4 SM120 네이티브 stock. strip-hoist 자동 skip(torch 2.12). plan_26070119, 스모크 최종중재).
#   미인식 키는 빌드를 명시적으로 실패시킨다(false determinism 방지 — plan rev3 §5 / .claude/skills/upstream-version-watch/references/source-build.md §3 HITL 발견 루프 유도).
#   P6: 이 인라인 셋을 source_build_patches.yaml + 패치-리졸버 페르소나로 승급.
VALIDATED_SOURCE_BUILD_KEYS = {
    ("26.03-py3", "0.22.1"),
    ("26.05-py3", "0.23.0"),
    ("26.05-py3", "0.24.0"),
    ("26.05-py3", "0.25.1"),  # tracked output/multi freeze; exact pair must remain synchronized
    ("26.05-py3", "0.26.0"),  # 2026-07-30 승급: 양노드 빌드+TP=2 서빙+스모크+에이전트-레디 3종 PASS (testlog_26073015 §2, 사용자 졸업 승인)
    ("26.07-py3", "0.27.0"),  # 2026-08-11 승급(single-node): torch 2.13.0 강제 소스빌드+gemma-4-12b-it-dgxspark 스모크 PASS.
                              #   전제조건 2건 — ① transformers<5.15.0(KNOWN_INCOMPAT, PyPI 5.15.0=2026-08-10 업로드가
                              #   heterogeneity 가드 신설 → hybrid-attention 모델 즉사) ② source-build 트랙은 pip constraint
                              #   파일을 requirements.txt(extras 제거)로 채워야 함(이 템플릿에 신규 배선 — 이전엔 비워서 무의미했음).
                              #   testlog_26081111 참조.
}

# ── 출구① 상속(inherit) 원장 — plan_26082112 §5.2 · P4 ────────────────────────
#   **별도 dict 다**(계획 §10 U4 권장 채택). 위 VALIDATED_SOURCE_BUILD_KEYS 는 리터럴 set 원형을
#   그대로 보존한다 — 그 set 은 "이 조합으로 실제 빌드·스모크가 통과했다"는 tripwire 이고,
#   아래 dict 는 "직접 통과시킨 적은 없으나 델타 증거로 상속한다"는 **성질이 다른 주장**이다.
#   둘을 한 자료구조에 섞으면 그 구분이 데이터에서 사라진다(결정론 규율 §출처 표시).
#
#   **이 목록은 Judge 가 채우지 않는다.** 등재는 게이트 G1.6 에서 **사람이 손으로** 한 줄 적는 행위다
#   (계획 §9 R2 · 하드코딩 정당조건 = tripwire). judge_version_delta.py 의 산출물은 **evidence 이지
#   approval 이 아니다** — 아래 술어는 그 evidence 가 *존재하고 상속 조건을 만족하는지*만 본다.
#
#   항목 스키마(4필드 전부 필수):
#     (ngc_tag, vllm_new): {
#         "inherits":    (ngc_tag, vllm_old),          # 반드시 VALIDATED_… 에 실재 + 같은 NGC 베이스
#         "attestation": "<resolved.json 경로>#upstream_delta",
#         "approved_by": "<승인자>",                    # G1.6 사람 승인 기재
#         "approved_kst": "YYMMDDHH",
#     }
#   ⚠ 이 relay(P4)는 **메커니즘만** 배선하고 실제 상속 항목은 넣지 않는다 — 비어 있는 것이 정상이다.
INHERITED_SOURCE_BUILD_KEYS: dict = {
    # (등재 예시 — 주석으로만 둔다. 실제 등재는 G1.6 승인 뒤 사람이 이 주석 아래에 적는다)
    # ("26.07-py3", "0.27.1"): {
    #     "inherits": ("26.07-py3", "0.27.0"),
    #     "attestation": "output/multi/resolved.json#upstream_delta",
    #     "approved_by": "<승인자>",
    #     "approved_kst": "26082212",
    # },
}

INHERIT_REQUIRED_FIELDS = ("inherits", "attestation", "approved_by", "approved_kst")
INHERIT_ATTESTATION_KEY = "upstream_delta"
INHERIT_ATTESTATION_SUFFIX = "#" + INHERIT_ATTESTATION_KEY
# tripwire(하드코딩 정당조건 — workflow.md §4종 안티패턴 판정표): judge_version_delta.py 의
#   SCHEMA_VERSION 이 올라가면 이 닫힌 목록을 **사람이 검토해** 넓힌다. 파생하지 않는 이유는
#   render 가 judge 를 import 하지 않기 때문이다(judge 부재가 렌더를 깨면 안 된다 — 배포 단위가 다르다).
INHERIT_ACCEPTED_ATTESTATION_SCHEMAS = (1,)


def _norm_ref(ref) -> str:
    """'v0.27.1' · '0.27.1' → '0.27.1' (ref ↔ 버전문자열 정규화)."""
    r = str(ref or "").strip()
    return r[1:] if r.startswith("v") else r


def _echo_safe(text: str) -> str:
    """RUN echo "..." 안에 안전하게 넣을 1행 문자열."""
    return re.sub(r"\s+", " ", str(text)).replace('"', "'").replace("\\", "/").strip()


def _inherit_eligibility(ngc_tag: str, vllm_version: str, entry, resolved):
    """출구① 상속 성립 술어. → (ok, checks[(name, ok, detail)], att)

    **전부 통과해야 성립한다.** 하나라도 걸리면 렌더는 상속을 거부하고 fail-loud 스탠자를 낸다 —
    조용히 미검증 경로로 강등하지 않는다(침묵 폴백 금지 · workflow.md §4종 판정표 폴백-결함).
    """
    checks = []

    def ck(name, ok, detail=""):
        checks.append((name, bool(ok), detail))
        return bool(ok)

    if not isinstance(entry, dict):
        ck("entry_shape", False, f"항목이 dict 가 아님: {type(entry).__name__}")
        return False, checks, None
    missing = [f for f in INHERIT_REQUIRED_FIELDS if not entry.get(f)]
    ck("entry_shape", not missing, f"필수 필드 결손: {missing}")

    base = entry.get("inherits")
    base_t = tuple(base) if isinstance(base, (tuple, list)) else None
    ck("base_shape", base_t is not None and len(base_t) == 2, f"inherits 가 (ngc, vllm) 2-튜플이 아님: {base!r}")
    if base_t and len(base_t) == 2:
        ck("base_same_ngc", str(base_t[0]) == str(ngc_tag),
           f"상속원 NGC({base_t[0]}) != 대상 NGC({ngc_tag}) — 출구①은 **같은 NGC 베이스** 위의 vLLM bump 만 덮는다")
        ck("base_validated", base_t in VALIDATED_SOURCE_BUILD_KEYS,
           f"상속원 {base_t} 가 VALIDATED_SOURCE_BUILD_KEYS 에 없음 — 상속의 상속(체인) 금지")
    else:
        ck("base_same_ngc", False, "inherits 형식 불량으로 판정 불가")
        ck("base_validated", False, "inherits 형식 불량으로 판정 불가")

    ptr = str(entry.get("attestation") or "")
    ck("attestation_pointer",
       ptr.endswith(INHERIT_ATTESTATION_SUFFIX) and len(ptr) > len(INHERIT_ATTESTATION_SUFFIX),
       f"attestation 포인터가 '<경로>{INHERIT_ATTESTATION_SUFFIX}' 형식이 아님: {ptr!r}")

    att = (resolved or {}).get(INHERIT_ATTESTATION_KEY)
    if not isinstance(att, dict) or not att:
        ck("attestation_present", False,
           f"렌더 입력(resolved)에 '{INHERIT_ATTESTATION_KEY}' 블록이 없다 — "
           "judge_version_delta.py --write-resolved 로 발행하지 않았거나 다른 resolved 를 렌더 중이다")
        return False, checks, None
    ck("attestation_present", True)

    ck("attestation_schema", att.get("schema_version") in INHERIT_ACCEPTED_ATTESTATION_SCHEMAS,
       f"attestation schema_version={att.get('schema_version')!r} 이 승인목록 {list(INHERIT_ACCEPTED_ATTESTATION_SCHEMAS)} 밖")
    ck("attestation_provenance", att.get("provenance") == "measured",
       f"provenance={att.get('provenance')!r} (measured 만 상속 근거가 된다 — 모의·dry-run 금지)")

    to_ok = _norm_ref(att.get("to_ref")) == str(vllm_version)
    from_ok = bool(base_t) and len(base_t) == 2 and _norm_ref(att.get("from_ref")) == str(base_t[1])
    ck("attestation_refs", to_ok and from_ok,
       f"attestation {att.get('from_ref')!r}->{att.get('to_ref')!r} 이 상속 주장 "
       f"{(base_t[1] if base_t and len(base_t) == 2 else '?')}->{vllm_version} 와 불일치")

    ax_a = (att.get("axis_A_build_input") or {}).get("verdict")
    ax_b_block = att.get("axis_B_port_scope") or {}
    ax_b = ax_b_block.get("verdict")
    ck("axis_A_no_impact", ax_a == "NO_IMPACT", f"axis_A={ax_a!r} (NO_IMPACT 필요)")
    # B축 정지조건은 `verdict == NO_IMPACT` 가 아니라 **`silent_revert_risk == []`** 다
    # (plan_26082112 §5.2 U7 해소 · §5.3.1 정정). 이식 트랙의 번들은 상류에 없는 내용을 의도적으로
    # 가지므로 델타가 번들 스코프와 겹치기만 하면 axis_B 는 IMPACT 다 — verdict 를 정지조건으로 쓰면
    # 게이트가 정의상 도달 불가가 되고, 도달 불가능한 게이트는 게이트가 아니라 우회 유인이다.
    # 위험분(WOULD_REVERT·UNDETERMINED)만 담는 `silent_revert_risk` 는 정상 재파생으로 비울 수 있다.
    # 키 부재(None)는 통과가 아니다 — 리스트가 아니면 fail-closed(침묵 폴백 금지).
    srr = ax_b_block.get("silent_revert_risk")
    ck("axis_B_no_silent_revert", isinstance(srr, list) and not srr,
       f"axis_B.silent_revert_risk={srr!r} — 빈 리스트여야 한다"
       f"(axis_B.verdict={ax_b!r}; WOULD_REVERT/UNDETERMINED 프로브가 남아 있거나 필드가 없다)")

    gv = att.get("verdict")
    ck("global_verdict", gv in ("NO_IMPACT", "IMPACT"),
       f"전역 verdict={gv!r} — UNDETERMINED/미지 는 상속 근거가 아니다(미판정이 무영향으로 세탁되는 것을 막는다)")

    unk = att.get("unknown")
    ck("unknown_empty", isinstance(unk, list) and not unk, f"unknown={unk!r} (비어 있어야 한다)")

    return all(ok for _, ok, _ in checks), checks, att


_PLACEHOLDER_RE = re.compile(r"\{\{\s*[A-Z0-9_]+\s*\}\}")


# ── 입력 로더 ────────────────────────────────────────────────────────────────
def load_resolved(path: str) -> dict:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def load_manifest(path: str) -> dict:
    """Load a manifest; use the stdlib parser only when PyYAML itself is unavailable."""
    try:
        import yaml  # type: ignore
    except ImportError:
        d = _load_yaml_flat(path)
        ic = _parse_interconnect_block(path)
        if ic:
            d["interconnect"] = ic
        nodes = _parse_nodes_block(path)
        if nodes:
            d["nodes"] = nodes
        return d
    with open(path, encoding="utf-8") as f:
        return yaml.safe_load(f) or {}


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


# HITL 우회(전방호환 시도-우선 · plan_26070208 [C]#1): 사람이 명시 승인한 시도-빌드에서만
# 가드를 WARN 으로 강등(main --allow-unvalidated). 기본 = fail-loud(false determinism 방지 불변).
ALLOW_UNVALIDATED = False


def _patch_guard(ngc_tag: str, vllm_version: str, resolved: dict | None = None,
                 inherited: dict | None = None) -> str:
    """(NGC 베이스 × vLLM 버전) source-build 가드 스탠자 생성. 출구 3갈래(plan_26082112 §5.2).

    ① validated  — VALIDATED_SOURCE_BUILD_KEYS 등재 → 진행(변경 없음)
    ①' inherited — INHERITED_SOURCE_BUILD_KEYS 등재 **+ 델타 attestation 이 상속 조건 충족** → 진행
    ②  attempt   — --allow-unvalidated (WARN 강등, 변경 없음)
    ③  discovery — fail-closed exit 1 (변경 없음)

    `inherited` 는 테스트 주입점이다(기본=모듈 원장). 프로덕션 호출자는 기본값만 쓴다.
    """
    key = (ngc_tag, vllm_version)
    ledger = INHERITED_SOURCE_BUILD_KEYS if inherited is None else inherited
    if key in VALIDATED_SOURCE_BUILD_KEYS:
        return (f'RUN echo "[guard] source-build key ({ngc_tag} x vLLM {vllm_version}): '
                f'validated patch set ((ngc_base x vllm_version) keyed) — proceeding."')
    if key in ledger:
        # 출구① — 상속. **선언만으로는 성립하지 않는다**: 델타 attestation 이 술어를 통과해야 한다.
        ok, checks, att = _inherit_eligibility(ngc_tag, vllm_version, ledger[key], resolved)
        entry = ledger[key] if isinstance(ledger[key], dict) else {}
        base = entry.get("inherits")
        base_s = f"{tuple(base)}" if isinstance(base, (tuple, list)) else repr(base)
        if ok:
            ax_c = (att.get("axis_C_model_path") or {}).get("verdict")
            # 판정 근거는 att 에서 파생한다 — 하드코딩하면 술어가 바뀐 뒤 빌드 로그가 거짓을 찍는다.
            _axa = (att.get("axis_A_build_input") or {}).get("verdict")
            _axb_blk = att.get("axis_B_port_scope") or {}
            _basis = (f"axis_A={_axa} axis_B={_axb_blk.get('verdict')}"
                      f"(silent_revert_risk={len(_axb_blk.get('silent_revert_risk') or [])}) "
                      f"unknown={len(att.get('unknown') or [])}")
            return (
                'RUN echo "[guard] source-build key (' + ngc_tag + ' x vLLM ' + vllm_version + '): INHERITED from ' + _echo_safe(base_s) + '." && \\\n'
                '    echo "  -> basis: upstream_delta ' + _echo_safe(f"{att.get('from_ref')}->{att.get('to_ref')}") + ' verdict=' + _echo_safe(att.get("verdict")) + ' ' + _echo_safe(_basis) + '" && \\\n'
                '    echo "  -> attestation: ' + _echo_safe(entry.get("attestation")) + ' (judge_version_delta.py · provenance=' + _echo_safe(att.get("provenance")) + ' · delta_source=' + _echo_safe(att.get("delta_source")) + ')" && \\\n'
                '    echo "  -> approved_by: ' + _echo_safe(entry.get("approved_by")) + ' @ ' + _echo_safe(entry.get("approved_kst")) + ' (HITL 게이트 G1.6 — 이 항목은 사람이 손으로 등재했다. Judge 는 가드를 넓히지 않는다)" && \\\n'
                '    echo "  -> scope: 상속되는 것은 **빌드 키**(패치 셋 적용가능성)뿐이다. 기능 판정 아님 — axis_C=' + _echo_safe(ax_c) + '." && \\\n'
                '    echo "  -> arbiter = smoke (workflow S3 · HITL 게이트 ③). 상속은 스모크를 면제하지 않는다." && \\\n'
                '    echo "  -> 출구① 계약: .claude/skills/upstream-version-watch/references/source-build.md §3.1"'
            )
        reasons = "; ".join(f"{n}: {d}" for n, o, d in checks if not o) or "unknown"
        return (
            'RUN echo "ERROR: source-build key (' + ngc_tag + ' x vLLM ' + vllm_version + ') declares an INHERITED_SOURCE_BUILD_KEYS entry, but the inheritance is NOT eligible." && \\\n'
            '    echo "  -> refused: ' + _echo_safe(reasons) + '" && \\\n'
            '    echo "  -> 상속은 **증거로만** 성립한다 — 성립 조건 정본 = .claude/skills/upstream-version-watch/references/source-build.md §3.1. 선언은 증거가 아니다." && \\\n'
            '    echo "  -> --allow-unvalidated 로 우회되지 않는다 — 이것은 미검증 키가 아니라 **원장 항목의 결함**이다(D3: 우회 말고 경로를 고친다)." && \\\n'
            '    echo "  -> fix: judge_version_delta.py 를 재실행해 resolved.json#upstream_delta 를 갱신하거나, INHERITED 항목을 지우고 출구②(--allow-unvalidated)/③(source-build.md §3)로 간다." && exit 1'
        )
    if ALLOW_UNVALIDATED:
        return (
            'RUN echo "WARN: source-build key (' + ngc_tag + ' x vLLM ' + vllm_version + ') is UNVALIDATED — attempt-build under HITL override (--allow-unvalidated)." && \\\n'
            '    echo "  -> arbiter = smoke (workflow S3). REQUIRED: record this attempt in docs/testlog/." && \\\n'
            '    echo "  -> on smoke PASS: codify the key into VALIDATED_SOURCE_BUILD_KEYS (.claude/skills/upstream-version-watch/references/source-build.md §3)."'
        )
    return (
        'RUN echo "ERROR: source-build key (' + ngc_tag + ' x vLLM ' + vllm_version + ') has NO validated patch set." && \\\n'
        '    echo "  -> this is UNVALIDATED, not IMPOSSIBLE — 미검증이지 불가 판정 아님(시도-우선 따름정리)." && \\\n'
        '    echo "  -> run .claude/skills/upstream-version-watch/references/source-build.md §3 HITL discovery loop, then graduate the verified" && \\\n'
        '    echo "     patches into Dockerfile.source-build.template ((ngc_base x vllm_version) guarded, P6 catalog)." && \\\n'
        '    echo "  -> or: human-approved attempt-build via render --allow-unvalidated (WARN + testlog 의무)." && \\\n'
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
        # resolved 를 넘기는 이유: 출구① 상속 술어가 resolved.json#upstream_delta(델타 attestation)를
        #   읽어야 하기 때문이다. 없으면 상속은 성립하지 않는다(무증거 상속 금지 — plan §8.2 N4).
        "SOURCE_BUILD_PATCH_GUARD": _patch_guard(ngc_tag, vllm, resolved),
        "CUDA_VERSION": str(wheel.get("cuda", "") or ""),
        "VLLM_MANYLINUX": str(wheel.get("manylinux", "") or ""),
    }


# ── NCCL envfile 빌드 (Plan 2 — manifest.interconnect + preset → KEY=VALUE · 키 수는 golden 에서 파생) ──
def build_nccl_env(manifest: dict) -> dict:
    """manifest.interconnect + NCCL_PRESETS + NCCL_INVARIANTS → NCCL env dict(키 수 = ①+②+③ 합, 자체검사가 golden 과 대조).
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
                       f"(fail-loud·오타 방지; 미검증 플랫폼은 'generic'(튜닝 0)으로 일단 기동→comms 스모크 중재, "
                       f"검증 후 전용 프리셋으로 승격)")
    env.update(NCCL_PRESETS[preset_key])
    # ③ 불변
    env.update(NCCL_INVARIANTS)
    # ① 전송 선택(manifest.interconnect.nccl_transport · 부재 = socket)
    transport = ic.get("nccl_transport") or "socket"
    if transport not in NCCL_TRANSPORTS:
        raise ValueError(f"manifest.interconnect.nccl_transport '{transport}' 미정의 — 알려진 {sorted(NCCL_TRANSPORTS)} "
                         f"(fail-loud · 오타가 조용히 socket 으로 가면 RDMA 를 쟀다고 믿게 된다)")
    env.update(NCCL_TRANSPORTS[transport])
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


# ── 클러스터 envfile 빌드 (S6 — manifest.nodes + preset → KEY=VALUE · 키 수는 golden 에서 파생) ────
def build_cluster_env(manifest: dict) -> dict:
    """manifest.nodes[] + CLUSTER_PRESETS + CLUSTER_INVARIANTS → Ray 클러스터-배포 env dict(키 수는 golden 과 대조).
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
                       f"(fail-loud·오타 방지; 미검증 플랫폼은 'generic'(튜닝 0)으로 일단 기동→스모크 중재, "
                       f"검증 후 전용 프리셋으로 승격)")
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


# ── env 키의 층(tier) 판정 — 공개 API (2026-09-21 · plan_26092119 §4.5·§4.6 · hint compose 슬롯) ──────
#   `.env.cluster`·`.env.interconnect` 는 3층(① manifest 환경값 · ② 플랫폼 프리셋 · ③ 불변)으로 렌더된다.
#   hint 는 이 두 파일을 **형상 템플릿**으로 싣는데(① 값만 `<manifest.…>` 로 치환 · ②③ 은 재현 사실이라
#   리터럴 유지), 옛 hint_collect 는 그 층을 **키 이름 접미사 휴리스틱**(TUNING_KEYS·IDENTITY_KEY_AXIS)으로
#   손으로 다시 적었다 — CLUSTER_PRESETS 4키를 중복하고 RAY_memory_*·NCCL 전부를 빠뜨렸다(코드맵 §4.4).
#   층의 정본은 이 파일의 렌더 함수이므로 여기서 **파생**한다:
#     ③ invariant = NCCL_INVARIANTS ∪ CLUSTER_INVARIANTS (렌더 순서상 마지막 writer — 우선)
#     ② preset    = 어느 프리셋 표든 그 키를 가진 것(detail = 프리셋 이름)
#     ① env       = 탐침 manifest(자리표시 값)로 build_*_env 를 실제로 돌려 **값에 탐침이 실린 키**
#                   (detail = 그 탐침이 온 manifest 필드) — ① 키 목록을 손으로 다시 적지 않는다.
#   규약 밖 키 = ("unknown", None) — 순수 판정기의 None 반환이며, 호출부(hintlib.artifacts)가
#   HINT_ENV_TIER_UNKNOWN 으로 fail-closed 한다(workflow.md §4종 판정표 폴백-정당).
_ENV_TIER_PROBES = {
    "__probe_hca__": "interconnect.hca_devices",
    "__probe_gid__": "interconnect.gid_index",
    "__probe_iface__": "interconnect.socket_iface",
    "__probe_main_host__": "nodes[main].host",
    "__probe_sub_host__": "nodes[sub].host",
    "__probe_main_ssh__": "nodes[main].ssh_user",
    "__probe_sub_ssh__": "nodes[sub].ssh_user",
}
_ENV_TIER_CACHE: dict = {}


def _env_tier_probe_env() -> dict:
    """탐침 manifest 로 두 렌더러를 돌린 ① 환경값 키 → 값. 프리셋은 두 표에 공통인 첫 이름을 쓴다
    (② 키는 아래 판정에서 먼저 걸러지므로 어느 프리셋이든 ① 판정은 같다)."""
    if "probe" in _ENV_TIER_CACHE:
        return _ENV_TIER_CACHE["probe"]
    common = sorted(set(NCCL_PRESETS) & set(CLUSTER_PRESETS))
    if not common:
        raise KeyError("NCCL_PRESETS 와 CLUSTER_PRESETS 에 공통 프리셋이 없다 — env_tier 탐침 불가(fail-loud)")
    probe = {
        "interconnect": {"hca_devices": ["__probe_hca__"], "gid_index": "__probe_gid__",
                         "socket_iface": "__probe_iface__", "platform_preset": common[0]},
        "nodes": [{"role": "main", "host": "__probe_main_host__", "ssh_user": "__probe_main_ssh__"},
                  {"role": "sub", "host": "__probe_sub_host__", "ssh_user": "__probe_sub_ssh__"}],
    }
    env = dict(build_nccl_env(probe))
    env.update(build_cluster_env(probe))
    _ENV_TIER_CACHE["probe"] = env
    return env


def env_tier(key: str) -> tuple:
    """`.env.cluster`/`.env.interconnect` 키의 층 → (tier, detail).

    tier ∈ {"env", "preset", "invariant"} · detail = env 면 manifest 필드 경로(예 "nodes[main].host"),
    preset 이면 그 키를 가진 프리셋 이름(쉼표 결합), invariant 면 None. 규약 밖 키 = ("unknown", None).
    """
    key = str(key)
    if key in NCCL_INVARIANTS or key in CLUSTER_INVARIANTS:
        return ("invariant", None)
    if any(key in keys for keys in NCCL_TRANSPORTS.values()):
        return ("env", "interconnect.nccl_transport")
    presets = sorted({name for table in (NCCL_PRESETS, CLUSTER_PRESETS)
                      for name, keys in table.items() if key in keys})
    if presets:
        return ("preset", ",".join(presets))
    value = _env_tier_probe_env().get(key)
    if value is not None:
        for sentinel, field in _ENV_TIER_PROBES.items():
            if sentinel in str(value):
                return ("env", field)
    return ("unknown", None)


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


# ── 통로 materialize (plan_26062321 — 통로 self-containment) ───────────────
def _repo_root() -> str:
    """이 스크립트(.claude/skills/upstream-version-watch/scripts/) 기준 repo 루트(4단계 상위)."""
    return os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", ".."))


def materialize_configs(repo: str, topology: str) -> list:
    """Shared canonical runner assets → output/<topology>/configs/.

    Idempotent, mode-preserving, and fail-closed when the synchronized source is absent.
    """
    if topology not in ("single", "multi"):
        raise ValueError(f"unknown topology: {topology!r}")
    src_dir = SHARED_ASSET_DIR
    dst_dir = os.path.join(repo, "output", topology, "configs")
    os.makedirs(dst_dir, exist_ok=True)
    copied = []
    for name in RUNNER_SCRIPTS:
        src = os.path.join(src_dir, name)
        if not os.path.isfile(src):
            raise FileNotFoundError(f"shared runner source missing: {src}")
        dst = os.path.join(dst_dir, name)
        shutil.copyfile(src, dst)
        os.chmod(dst, 0o755)
        copied.append(dst)
    return copied


def materialize_env(repo: str, topology: str, manifest: dict) -> str:
    """manifest.nas_model_path → output/<topology>/.env (프로젝트-레벨 compose 변수치환용).

    docker compose 가 docker-compose.yaml 의 ${NAS_MODEL_PATH}·${TIKTOKEN_HOST_PATH} 치환에 쓰는
    프로젝트 .env 를 manifest 에서 생성한다. 이게 없으면 serve 가 compose 기본값(/mnt/models)을 마운트해
    모델을 못 찾는다(check_smoke_model.py 도 동일 정본=manifest 직독). PII(NAS 경로) 포함 → output/* gitignored.
    근거: testlog_26062422 결함#2(serve-time NAS 미전파). nas_model_path 부재 시 fail-loud(무증거 진행 금지)."""
    nas = str(manifest.get("nas_model_path", "")).strip()
    if not nas:
        raise ValueError(
            "manifest.nas_model_path 부재 — output/%s/.env materialize 불가. "
            "serve 가 compose 기본값 /mnt/models 를 마운트해 모델을 못 찾는다. manifest 를 채울 것." % topology)
    # tiktoken·quant 도 manifest 정본 우선(env > manifest > 리터럴 default — 헌법 §serve-time env 통로 불변식).
    # plan_26063018: P1 이 manifest 에 tiktoken_host_path·quant_model_path 필드 추가 → 여기서 .env 로 materialize.
    tiktoken = str(manifest.get("tiktoken_host_path", "") or "").strip() or os.path.join(repo, "tiktoken_cache")
    # ⚠ quant 폴백은 **fail-loud** 여야 한다(2026-08-14 교정). 옛 코드는 필드 부재 시 조용히 NAS 루트로
    #   대체했고, 그 결과 메인 manifest 에는 quant_model_path 가 있고 **서브에는 없어** 같은 양자화 모델이
    #   마스터에선 해소되고 슬레이브에선 엉뚱한 경로가 됐다. TP=2 는 슬레이브도 가중치 절반을 로드하므로
    #   분산 서빙이 거기서 깨지는데, 원인이 Ray 워커 안쪽으로 숨어 진단이 어렵다.
    #   `plan_26081314` D2 판정표의 **"결정 경로에서 원인을 삼키는 침묵 폴백"** 에 해당한다 —
    #   폴백 자체는 유지하되(하위호환) 발화시키고, 산출물에 출처를 각인한다(결정론 규율: 값 옆에 출처).
    _quant_declared = str(manifest.get("quant_model_path", "") or "").strip()
    quant = _quant_declared or nas
    quant_source = "manifest.quant_model_path" if _quant_declared else "FALLBACK:nas_model_path"
    if not _quant_declared:
        sys.stderr.write(
            "[render] WARN: manifest.quant_model_path 부재 — QUANT_MODEL_PATH 를 nas_model_path(%s) 로 폴백한다.\n"
            "[render]   양자화 모델(/app/quant_models/...)을 쓰면 이 노드에서 경로가 해소되지 않는다. manifest 를 채울 것.\n"
            % nas)
    dst_dir = os.path.join(repo, "output", topology)
    os.makedirs(dst_dir, exist_ok=True)
    dst = os.path.join(dst_dir, ".env")
    # PLE mmap 스테이징(2026-09-09 · camp-26090918): manifest.ple_mmap_host_path 가 선언되면
    #   compose 볼륨 ${PLE_MMAP_HOST_PATH}:/app/ple_mmap:ro 치환값으로 방출한다. 미선언이면
    #   줄 자체를 쓰지 않는다(compose 의 `:-/mnt/models` 플레이스홀더로 떨어지고, mmap 자체는
    #   셀 env 의 VLLM_PLE_MMAP=1 이 있을 때만 팔리므로 stock 경로는 무영향).
    _ple_mmap = str(manifest.get("ple_mmap_host_path", "") or "").strip()
    body = (
        "# 프로젝트-레벨 env (compose 변수치환) — render_dockerfile.py --materialize-env 가 manifest 에서 생성.\n"
        "# docker compose 가 docker-compose.yaml 의 ${NAS_MODEL_PATH}·${QUANT_MODEL_PATH}·${TIKTOKEN_HOST_PATH} 치환에 사용.\n"
        "# gitignored(output/* — PII). 손수정 금지 — manifest(nas_model_path·quant_model_path·tiktoken_host_path·ple_mmap_host_path)를 고칠 것.\n"
        "NAS_MODEL_PATH=%s\n"
        "# QUANT_MODEL_PATH 출처(결정론 규율 — 값 옆에 출처): %s\n"
        "QUANT_MODEL_PATH=%s\n"
        "TIKTOKEN_HOST_PATH=%s\n"
    ) % (nas, quant_source, quant, tiktoken)
    if _ple_mmap:
        body += (
            "# PLE_MMAP_HOST_PATH 출처(결정론 규율 — 값 옆에 출처): manifest.ple_mmap_host_path\n"
            "PLE_MMAP_HOST_PATH=%s\n"
        ) % _ple_mmap
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


# ── 빌드 원장(build ledger) 스탠자 — 단일 소유 (2026-09-21 · plan_26092119 §4.5 · 코드맵 build_plane §2) ──
#   왜: 태그2(camp-26091216)의 hint 는 **skip 된 패치(50·55)를 "적용됨"으로 실었다**. 패치는 skip 과 적용을
#   둘 다 `exit 0` 으로 끝내고 skip 은 자유 문장으로만 말했으며(K4), 옛 "적용 증거"는 skip 된 50번 자신의
#   PROVENANCE.json 이었다(K3). 이미지가 자기 빌드를 서술하지 않으면 그 사실은 빌드 로그와 함께 사라진다.
#   처방: 두 Dockerfile 템플릿이 `/opt/easy-vllm/build_ledger.json` 을 이미지 안에 남긴다.
#     · 패치 루프는 **원시 사실만** `ledger.d/` 에 남긴다(종료코드·스크립트 sha256·로그 꼬리·상태 줄).
#     · 분류·조립은 이 스탠자 — **마지막 RUN** — 가 한다. 분류 로직을 고쳐도 이 레이어만 다시 빌드되고
#       `_C` 컴파일 캐시는 보존된다(캐시 불변식 · 자체검사가 "마지막 RUN" 을 확인한다).
#     · 분류 우선순위: 패치가 쓴 상태 줄(`EASY_VLLM_PATCH_STATUS` 파일, `applied|skipped<TAB>사유`) >
#       로그 토큰(옛 패치의 `] skip —`/`— skip` 자유 문장 · 라벨 `log-token`) > 종료코드(`exit-code` — 자기보고 없음).
#     · build-arg 목록은 **템플릿 자신의 `^ARG` 줄에서 파생**한다 — 새 ARG 는 원장에 자동 기록된다(손목록 ✗).
#     · `import vllm._C` 금지(빌드 시 libcuda 없음 — source-build.md §1.5). 버전은 importlib.metadata 로만 읽는다.
#     · 베이스 사실은 **파일**에서 읽는다 — wheel 트랙 `ARG CUDA_VERSION=130` 이 NGC ENV `CUDA_VERSION` 을 가린다(K8).
#       같은 이름의 ARG 가 가리는 ENV 는 값 대신 `shadowed-by-ARG` 로 적는다(추측 ✗).
#     · 셸 무관(K8): wheel 베이스는 `/bin/sh -c`, 26.07 은 `/bin/bash -c` — 논리는 python3 에 둔다.
#   policy:GIT_SINGLE_AUTHORITY: 원장은 git 이 드는 바이트의 digest 재기재가 아니라 **빌드 시점 적용 결과**
#   (Q1 아니오 · Q2 아니오 — 상류 클론·네트워크 의존·게이트 평가에 달렸다)이므로 맹점층이다 — 유지 정당.
#   ⚠ 두 템플릿에 이 코드를 손으로 두 벌 적지 않는다 — `{{ BUILD_LEDGER_STANZA }}` 한 자리로 주입한다.
_LEDGER_ARG_RE = re.compile(r"^ARG\s+([A-Za-z_][A-Za-z0-9_]*)", re.M)
_LEDGER_STANZA_HEAD = (
    "# ═════════════════════════════════════════════════════════════════════════════\n"
    "# [빌드 원장] /opt/easy-vllm/build_ledger.json — 이미지 자기서술(render_dockerfile.py 단일 소유 주입)\n"
    "#   build-arg 목록 = 이 템플릿의 `^ARG` 줄에서 렌더 시 파생 · 분류 = 상태 줄 > 로그 토큰 > 종료코드.\n"
    "#   **마지막 RUN** 이어야 한다 — 이 레이어만 바뀌면 `_C` 컴파일 캐시가 보존된다.\n"
    "#   `import vllm._C` 금지(빌드 시 libcuda 없음) · 베이스 사실은 파일에서 읽는다(ARG 가 ENV 를 가린다).\n"
    "# ═════════════════════════════════════════════════════════════════════════════\n"
)
# heredoc 본문(따옴표 친 구분자 → Docker 변수 확장 없음). 줄머리 `#` 주석을 두지 않는다(해석 차이 회피).
_LEDGER_PY = r'''import hashlib, json, os, platform, re, subprocess
from importlib import metadata
ROOT = os.environ.get("EASY_VLLM_LEDGER_ROOT") or "/opt/easy-vllm"
D = os.path.join(ROOT, "ledger.d")
ARGS = __EASY_VLLM_LEDGER_ARGS__
DOCKERFILE = __EASY_VLLM_LEDGER_DOCKERFILE__
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
'''


def ledger_build_args(template_text: str) -> list:
    """템플릿 자신의 `^ARG <NAME>` 줄 → 원장이 기록할 build-arg 이름(선언 순서 · 중복 제거).

    `_ensure_copy_context_dirs` 와 같은 관용(렌더 본문에서 파생)이다 — 목록을 손으로 적으면 새 ARG 가
    원장에서 조용히 빠진다(SLAVE_IMGVARS 가 build-arg 와 따로 자라며 다섯 번 겪은 그 형태)."""
    return list(dict.fromkeys(_LEDGER_ARG_RE.findall(template_text)))


def ledger_dockerfile_name(template_path: str) -> str:
    """원장의 `dockerfile` 값 = **실제 렌더한 템플릿**의 이름(`.template` 제거).

    build_context 의 DOCKERFILE 은 resolved 의 트랙에서 오므로 템플릿과 갈릴 수 있다(예: 다른 트랙의
    resolved 로 렌더) — 이미지가 자기를 서술하는 값은 입력 선언이 아니라 쓰인 파일에서 파생한다."""
    name = os.path.basename(str(template_path))
    return name[:-len(".template")] if name.endswith(".template") else name


def build_ledger_stanza(build_args: list, dockerfile: str) -> str:
    """원장 조립 RUN 스탠자(헤더 주석 + python3 heredoc). 끝 개행 없음(템플릿 줄이 개행을 가진다)."""
    body = (_LEDGER_PY.replace("__EASY_VLLM_LEDGER_ARGS__", json.dumps(list(build_args)))
            .replace("__EASY_VLLM_LEDGER_DOCKERFILE__", json.dumps(str(dockerfile))))
    return _LEDGER_STANZA_HEAD + "RUN python3 - <<'PY'\n" + body + "PY"


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
    # 원장 스탠자는 build_context(manifest, resolved) 가 아니라 **템플릿 자신**에서 파생한다 — ARG 목록과
    # 파일 이름은 입력 선언이 아니라 렌더되는 본문의 사실이다(build_context 시그니처 불변: 모델/패치 입력 ✗).
    ctx["BUILD_LEDGER_STANZA"] = build_ledger_stanza(ledger_build_args(text),
                                                     ledger_dockerfile_name(template_path))
    out = _substitute(text, ctx)
    # wheel 트랙 무결성: wheel URL 의 cuXXX 가 비면(데이터 누락) fail-loud
    if "vllm-${VLLM_VERSION}+cu${CUDA_VERSION}" in text and not ctx.get("CUDA_VERSION"):
        raise ValueError("wheel 트랙인데 resolved.wheel.cuda 부재 → wheel URL 불완전")
    return out


# kind → build track. 갈림의 축은 브랜치/토폴로지가 아니라 **빌드 트랙**이다(2026-08-13 사용자 지시):
#   torch 2.11+ 는 prebuilt _C 가 NGC alpha torch 와 ABI 불일치 → source-build 강제.
#   2.10 이하만 wheel 성립(대가 = 구동 가능 모델 범위 축소).
# compose 는 트랙 중립이라 default(top-level)를 읽는다.
_KIND_TO_TRACK = {"dockerfile": "wheel", "source-build": "source-build", "compose": None}


def load_shared_resolution(track: str | None = None) -> dict:
    """Canonical production resolution, optionally for a specific build track.

    top-level IS the `default_track` resolution -- it is never duplicated under `tracks`
    (a second copy would be a second list, and this project has been bitten by that repeatedly).
    A non-default track must exist under `tracks.<name>` or this fails closed.
    """
    try:
        with open(SHARED_RESOLUTION, encoding="utf-8") as fh:
            value = json.load(fh)
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise ValueError(f"shared production resolution unreadable: {exc}") from exc
    if not isinstance(value, dict):
        raise ValueError("shared production resolution is not an object")
    if track is not None and track != value.get("default_track"):
        tracks = value.get("tracks")
        if not isinstance(tracks, dict) or not isinstance(tracks.get(track), dict):
            raise ValueError(
                f"shared production resolution lacks tracks.{track} "
                f"(default_track={value.get('default_track')!r})")
        value = tracks[track]
    if not value.get("ngc_base", {}).get("tag"):
        raise ValueError(
            f"shared production resolution lacks ngc_base.tag"
            f"{'' if track is None else f' for track {track!r}'}")
    return value


def render_shared(kind: str, topology: str, manifest: dict,
                  version_resolution: dict | None = None) -> str:
    """Render a synchronized canonical source for an explicit synthetic topology.

    Checked-out branch identity is never consulted. Unknown topology/kind, a missing source, or
    unresolved placeholders fail closed.
    """
    key = (topology, kind)
    if key not in _SHARED_TEMPLATES:
        raise ValueError(f"unknown topology/artifact kind: {topology!r}/{kind!r}")
    path = os.path.join(SHARED_TEMPLATE_DIR, _SHARED_TEMPLATES[key])
    if not os.path.isfile(path):
        raise FileNotFoundError(f"shared template missing: {path}")
    resolution = (version_resolution if version_resolution is not None
                  else load_shared_resolution(_KIND_TO_TRACK[kind]))
    return render(path, manifest, resolution)


# ── self-test (A7 게이트가 호출) ──────────────────────────────────────────────
def _require(condition, message):
    if not condition:
        raise AssertionError(message)


# ── 출구① 상속 회귀 — plan_26082112 §8.2 N4·N5 음성대조 ──────────────────────
#   합성 attestation 은 **테스트 평면 격리**용이다(4종 판정표 모킹-정당). 프로덕션 산출물과 같은
#   모양으로 새어나가지 않도록 이 함수 안에서만 만들고 밖으로 쓰지 않는다.
_INHERIT_KEY = ("26.05-py3", "9.99.1")           # 실재 NGC 베이스 × 합성 vLLM 버전(실 키와 미충돌)
_INHERIT_BASE = ("26.05-py3", "0.26.0")          # VALIDATED_… 실재 항목 · 같은 NGC 베이스(체인 아님)


def _synthetic_attestation(**over) -> dict:
    att = {
        "schema_version": 1,
        "from_ref": "v" + _INHERIT_BASE[1], "to_ref": "v" + _INHERIT_KEY[1],
        "provenance": "measured", "delta_source": "git-local",
        "axis_A_build_input": {"verdict": "NO_IMPACT"},
        # judge_version_delta.axis_b() 는 항상 `probes`(전체)와 `silent_revert_risk`(위험분만)를
        # 함께 싣는다 — 상속 술어가 후자를 읽으므로 픽스처도 실물과 같은 모양이어야 한다.
        "axis_B_port_scope": {"verdict": "NO_IMPACT", "probes": [], "silent_revert_risk": []},
        "axis_C_model_path": {"verdict": "NOT_IMPLEMENTED"},
        "verdict": "NO_IMPACT", "unknown": [],
    }
    att.update(over)
    return att


def _synthetic_entry(**over) -> dict:
    e = {"inherits": _INHERIT_BASE,
         "attestation": "output/multi/resolved.json#upstream_delta",
         "approved_by": "self-test", "approved_kst": "26082212"}
    e.update(over)
    return e


def _inherit_self_test(tpl: str, man: dict) -> None:
    ngc, vllm = _INHERIT_KEY

    def render_with(ledger, att):
        """모듈 원장을 **일시 치환**해 build_context → _patch_guard 정문 배선까지 실제로 태운다.
        (술어만 직접 호출하면 '만든 것'은 증명되나 '도는 것'은 증명되지 않는다.)"""
        global INHERITED_SOURCE_BUILD_KEYS
        saved = INHERITED_SOURCE_BUILD_KEYS
        try:
            INHERITED_SOURCE_BUILD_KEYS = ledger
            res = {"vllm_version": vllm, "torch": {"pin": "2.13.0"},
                   "ngc_base": {"tag": ngc, "cuda_version": "13.2.0.046"},
                   "build_track": {"decision": "source-build"},
                   "source_build": {"torch_cuda_arch": "12.1a"}}
            if att is not None:
                res["upstream_delta"] = att
            return _substitute(tpl, build_context(man, res))
        finally:
            INHERITED_SOURCE_BUILD_KEYS = saved

    led = {_INHERIT_KEY: _synthetic_entry()}

    # 양성 대조 — 술어를 전부 만족하면 상속 스탠자가 나오고 exit 1 이 **없어야** 한다.
    #   (이 대조가 없으면 아래 음성대조는 "가드가 항상 빨간불"인 것과 구분되지 않는다.)
    ok_out = render_with(led, _synthetic_attestation())
    _require("INHERITED from" in ok_out and "exit 1" not in ok_out,
             f"양성 대조 실패 — 적격 상속인데 진행하지 않음:\n{ok_out}")
    _require("arbiter = smoke" in ok_out, "상속 스탠자에 스모크 불변 문구 누락")
    _require("axis_C=NOT_IMPLEMENTED" in ok_out, "상속 스탠자가 axis_C 미구현 사실을 숨김")
    _require("approved_by: self-test" in ok_out, "상속 스탠자에 G1.6 승인자 미기재")
    _require("silent_revert_risk=0" in ok_out,
             f"상속 스탠자의 basis 가 att 에서 파생되지 않음(하드코딩 회귀):\n{ok_out}")

    # 양성 대조 ② — U7 해소의 본체. axis_B.verdict 가 IMPACT 여도 **위험분이 0건이면** 상속이 성립한다.
    #   (이 대조가 없으면 F-4e 교정이 배선됐는지 '도는 것'으로 증명되지 않는다 — 옛 술어라면 여기서 exit 1.)
    ok_b = render_with(led, _synthetic_attestation(
        axis_B_port_scope={"verdict": "IMPACT",
                           "probes": [{"path": "vllm/x.py", "verdict": "NO_REVERT"}],
                           "silent_revert_risk": []},
        verdict="IMPACT"))
    _require("INHERITED from" in ok_b and "exit 1" not in ok_b,
             f"양성 대조② 실패 — axis_B=IMPACT ∧ silent_revert_risk=[] 인데 상속 거부:\n{ok_b}")
    _require("axis_B=IMPACT(silent_revert_risk=0)" in ok_b,
             f"양성 대조② 실패 — basis 가 실제 axis_B 상태를 숨김:\n{ok_b}")

    # N4 — attestation **없이** 상속 시도 → exit 1
    n4 = render_with(led, None)
    _require("exit 1" in n4 and "NOT eligible" in n4, f"N4 실패 — 무증거 상속이 통과:\n{n4}")
    _require("attestation_present" in n4, f"N4 실패 — 거부 사유가 지목되지 않음:\n{n4}")

    # N5 — verdict == UNDETERMINED attestation 으로 상속 시도 → exit 1
    n5 = render_with(led, _synthetic_attestation(verdict="UNDETERMINED",
                                                 unknown=["UNKNOWN_PLANE: newplane/x.toml"]))
    _require("exit 1" in n5 and "NOT eligible" in n5, f"N5 실패 — UNDETERMINED 상속이 통과:\n{n5}")
    _require("global_verdict" in n5 and "unknown_empty" in n5,
             f"N5 실패 — UNDETERMINED/unknown 사유가 지목되지 않음:\n{n5}")

    # N5' — 축별 미판정도 같은 결론(전역만 보고 새지 않는다)
    for axis in ("axis_A_build_input", "axis_B_port_scope"):
        o = render_with(led, _synthetic_attestation(**{axis: {"verdict": "UNDETERMINED"}}))
        _require("exit 1" in o, f"N5' 실패 — {axis}=UNDETERMINED 인데 통과")

    # 추가 음성 — 상속 술어의 나머지 성립조건이 각각 실제로 가드한다.
    neg = {
        "axis_B.silent_revert_risk 비지 않음(재파생 필요)": (led, _synthetic_attestation(
            axis_B_port_scope={"verdict": "IMPACT",
                               "probes": [{"path": "vllm/x.py", "verdict": "WOULD_REVERT"}],
                               "silent_revert_risk": [{"path": "vllm/x.py",
                                                       "verdict": "WOULD_REVERT"}]},
            verdict="IMPACT")),
        "axis_B 에 silent_revert_risk 필드 자체가 없음": (led, _synthetic_attestation(
            axis_B_port_scope={"verdict": "IMPACT"}, verdict="IMPACT")),
        "attestation 이 다른 bump 의 것": (led, _synthetic_attestation(from_ref="v0.1.0")),
        "provenance=mock(실측 아님)": (led, _synthetic_attestation(provenance="mock")),
        "미지 schema_version": (led, _synthetic_attestation(schema_version=99)),
        "상속원이 VALIDATED 에 없음(체인 금지)": (
            {_INHERIT_KEY: _synthetic_entry(inherits=("26.05-py3", "0.99.0"))},
            _synthetic_attestation(from_ref="v0.99.0")),
        "상속원 NGC 베이스 불일치": (   # 상속원 자체는 VALIDATED 지만 NGC 가 다르다
            {_INHERIT_KEY: _synthetic_entry(inherits=("26.03-py3", "0.22.1"))},
            _synthetic_attestation(from_ref="v0.22.1")),
        "승인자 미기재(G1.6 미이행)": (
            {_INHERIT_KEY: _synthetic_entry(approved_by="")}, _synthetic_attestation()),
        "attestation 포인터 형식 불량": (
            {_INHERIT_KEY: _synthetic_entry(attestation="output/multi/resolved.json")},
            _synthetic_attestation()),
    }
    for name, (ledger, att) in neg.items():
        o = render_with(ledger, att)
        _require("exit 1" in o, f"음성대조 실패 — [{name}] 인데 상속이 통과:\n{o}")

    # 원장 자기정합 — 사람이 손으로 등재한 항목이 스키마를 지키는지(등재 시점에 빨간불).
    for k, e in INHERITED_SOURCE_BUILD_KEYS.items():
        _require(isinstance(k, tuple) and len(k) == 2, f"INHERITED 키 형식 불량: {k!r}")
        _require(k not in VALIDATED_SOURCE_BUILD_KEYS,
                 f"INHERITED 항목 {k!r} 이 VALIDATED 에도 있음 — 두 주장이 겹치면 출처가 흐려진다")
        miss = [f for f in INHERIT_REQUIRED_FIELDS if not (isinstance(e, dict) and e.get(f))]
        _require(not miss, f"INHERITED 항목 {k!r} 필수 필드 결손: {miss}")
        _require(tuple(e["inherits"]) in VALIDATED_SOURCE_BUILD_KEYS,
                 f"INHERITED 항목 {k!r} 의 상속원 {e['inherits']!r} 이 VALIDATED 에 없음(체인 금지)")

    print(f"[render] inherit self-test OK — 출구① 상속 양성2(NO_IMPACT·IMPACT∧위험0) + 음성{2 + 2 + len(neg)}건"
          f"(N4 무증거·N5 UNDETERMINED 포함) 전부 fail-closed · 원장 항목 "
          f"{len(INHERITED_SOURCE_BUILD_KEYS)}건 자기정합")


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
    _require("{{" not in out_a, "leftover placeholder")
    _require("26.03-py3" in out_a, "ngc tag (a)")
    _require("0.22.1-cu132-aarch64-source" in out_a, f"image tag (a): {out_a!r}")
    _require("validated patch set" in out_a, "guard(validated) for (26.03, 0.22.1)")
    # success 픽스처 ②: E2E 검증 (NGC 26.05, vLLM 0.23.0) — pyproject torch핀은 2.11.0이나 실-링크 torch는 2.12
    res_b = {"vllm_version": "0.23.0", "torch": {"pin": "2.11.0"},
             "ngc_base": {"tag": "26.05-py3", "cuda_version": "13.2.0.046"},
             "build_track": {"decision": "source-build"},
             "source_build": {"torch_cuda_arch": "12.1a"}}
    out_b = _substitute(tpl, build_context(man, res_b))
    _require("26.05-py3" in out_b, "ngc tag (b)")
    _require("0.23.0-cu132-aarch64-source" in out_b, f"image tag (b): {out_b!r}")
    _require("validated patch set" in out_b, "guard(validated) for (26.05, 0.23.0)")
    # fail-loud 픽스처: (NGC 26.03, vLLM 0.23.0) = testlog 가 FAIL 로 증명 → 가드가 빌드 실패시켜야 함
    res_bad = {"vllm_version": "0.23.0", "torch": {"pin": "2.11.0"},
               "ngc_base": {"tag": "26.03-py3", "cuda_version": "13.2.0.046"},
               "build_track": {"decision": "source-build"},
               "source_build": {"torch_cuda_arch": "12.1a"}}
    out_bad = _substitute(tpl, build_context(man, res_bad))
    _require("exit 1" in out_bad, "guard(fail-loud) for (26.03, 0.23.0)")
    print("[render] self-test OK — source-build 렌더 + (NGC베이스×vLLM버전) 키 가드(검증x2/fail-loud) 정상")

    _inherit_self_test(tpl, man)

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
    _require(len(rendered) == len(golden),
             f"NCCL 키 수는 golden에서 파생되어야 한다({len(golden)} 기대, got {len(rendered)})")
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
    # 전송 선택: rdma = 내장 verbs 명시(IB_DISABLE=0·NET=IB — 조용한 Socket 강등 ✗) · socket 명시 = 부재와 같다 · 미지값 fail-loud
    _rd = build_nccl_env({"interconnect": {**man_ic["interconnect"], "nccl_transport": "rdma"}})
    _require(_rd.get("NCCL_NET") == "IB" and _rd.get("NCCL_IB_DISABLE") == "0" and len(_rd) == len(golden),
             f"nccl_transport=rdma 렌더 이상: NCCL_NET={_rd.get('NCCL_NET')!r} IB_DISABLE={_rd.get('NCCL_IB_DISABLE')!r} keys={len(_rd)}")
    _require(build_nccl_env({"interconnect": {**man_ic["interconnect"], "nccl_transport": "socket"}})
             == build_nccl_env(man_ic), "nccl_transport=socket 명시가 부재(기본)와 갈라졌다")
    try:
        build_nccl_env({"interconnect": {**man_ic["interconnect"], "nccl_transport": "roce"}})
        raise AssertionError("미지 nccl_transport 인데 통과(fail-loud 위반)")
    except ValueError:
        pass
    # 키 수는 golden 에서 파생한다 — 옛 문구의 "17키" 는 golden(19)과 갈라진 낡은 리터럴이었다(코드맵 K12).
    print(f"[render] NCCL self-test OK — .env.interconnect {len(golden)}키 == golden 집합 동치 · 미지preset/결손 fail-loud 정상")

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
    # 키 수는 golden 에서 파생한다(8→9: MAX_JOBS Band2 재귀속 plan_26062811_30_33 — 리터럴을 두면 다시 갈린다).
    _require(len(crendered) == len(cgolden), f"cluster {len(cgolden)}키 기대(golden), got {len(crendered)}")
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
    print(f"[render] cluster self-test OK — .env.cluster {len(cgolden)}키 == golden 집합 동치 · 미지preset/노드결손 fail-loud 정상")

    _env_tier_self_test(golden, cgolden)
    _ledger_self_test()

    # ── materialize self-test: shared SSOT → 통로 복사 멱등·실행권한·fail-loud ──
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        os.makedirs(os.path.join(td, "output", "multi"))
        copied = materialize_configs(td, "multi")
        _require(len(copied) == len(RUNNER_SCRIPTS), "materialize 복사 수")
        for name in RUNNER_SCRIPTS:
            dst = os.path.join(td, "output", "multi", "configs", name)
            _require(os.path.isfile(dst), f"materialize 대상 부재: {name}")
            _require(stat.S_IMODE(os.stat(dst).st_mode) == 0o755, f"exact 0755 미보존: {name}")
        materialize_configs(td, "multi")
        global SHARED_ASSET_DIR
        original_assets = SHARED_ASSET_DIR
        try:
            SHARED_ASSET_DIR = os.path.join(td, "missing-shared-assets")
            try:
                materialize_configs(td, "multi")
                raise AssertionError("shared source 부재인데 통과(fail-loud 위반)")
            except FileNotFoundError:
                pass
        finally:
            SHARED_ASSET_DIR = original_assets
    print("[render] materialize self-test OK — 러너 스크립트 통로 복사(멱등·권한·fail-loud) 정상")


def _env_tier_self_test(nccl_golden: dict, cluster_golden: dict) -> None:
    """env_tier(key) 공개 API — golden 두 벌의 모든 키가 규약 층을 받고, ① 키는 탐침에서 파생된다."""
    allowed = {"env", "preset", "invariant"}
    for key in sorted(set(nccl_golden) | set(cluster_golden)):
        tier, detail = env_tier(key)
        _require(tier in allowed, f"env_tier({key!r}) = {tier!r} — golden 키가 규약 층 밖")
        if tier == "env":
            _require(detail and not detail.startswith("__"), f"env_tier({key!r}) detail 이 manifest 필드가 아님: {detail!r}")
        if tier == "invariant":
            _require(detail is None, f"invariant 의 detail 은 None 이어야 한다: {key!r}={detail!r}")
    # 대표 사실(렌더 함수와 대조) — ① 은 manifest 필드, ② 는 프리셋 이름, ③ 은 불변.
    _require(env_tier("MASTER_HOST_IP") == ("env", "nodes[main].host"), env_tier("MASTER_HOST_IP"))
    _require(env_tier("SLAVE_HOST_IP") == ("env", "nodes[sub].host"), env_tier("SLAVE_HOST_IP"))
    _require(env_tier("NCCL_SOCKET_IFNAME") == ("env", "interconnect.socket_iface"), env_tier("NCCL_SOCKET_IFNAME"))
    _require(env_tier("NCCL_IB_HCA") == ("env", "interconnect.hca_devices"), env_tier("NCCL_IB_HCA"))
    _require(env_tier("RAY_PORT") == ("invariant", None), env_tier("RAY_PORT"))
    _require(env_tier("NCCL_IB_DISABLE") == ("env", "interconnect.nccl_transport"), env_tier("NCCL_IB_DISABLE"))
    _require(env_tier("NCCL_NET") == ("env", "interconnect.nccl_transport"), env_tier("NCCL_NET"))
    _require(env_tier("NCCL_NET_GDR_LEVEL") == ("preset", "dgx-spark-gb10"), env_tier("NCCL_NET_GDR_LEVEL"))
    _require(env_tier("MAX_JOBS") == ("preset", "dgx-spark-gb10"), env_tier("MAX_JOBS"))
    # ① 키 집합은 손목록이 아니라 탐침 파생 — golden 에서 env 층으로 판정된 키와 정확히 같아야 한다.
    derived_env = {k for k in set(nccl_golden) | set(cluster_golden) if env_tier(k)[0] == "env"}
    _require(derived_env == set(_IFACE_ENV_KEYS) | {"NCCL_IB_HCA", "NCCL_IB_GID_INDEX", "MASTER_HOST_IP",
                                                    "SLAVE_HOST_IP", "SSH_USER", "NCCL_NET",
                                                    "NCCL_IB_DISABLE"},
             f"① 환경값 키 집합이 렌더 함수와 갈라졌다: {sorted(derived_env)}")
    # ★ 음성대조: 규약 밖 키는 unknown 이다(조용히 preset/env 로 흡수 ✗ — 호출부가 fail-closed 한다).
    _require(env_tier("NOT_A_RENDERED_KEY") == ("unknown", None), env_tier("NOT_A_RENDERED_KEY"))
    _require(env_tier("CONFIG_FILE") == ("unknown", None), "CONFIG_FILE 은 Band2 렌더 키가 아니다")
    print(f"[render] env_tier self-test OK — golden {len(set(nccl_golden) | set(cluster_golden))}키 전수 층 판정 · "
          f"① {len(derived_env)}키 탐침 파생 · 규약 밖 unknown(음성대조)")


def _ledger_stanza_of(rendered: str) -> tuple:
    """렌더 본문에서 원장 heredoc 본문과 그 RUN 줄 위치를 찾는다(없으면 fail-loud)."""
    marker = "RUN python3 - <<'PY'\nimport hashlib, json, os, platform, re, subprocess\n"
    idx = rendered.find(marker)
    _require(idx >= 0, "렌더 본문에 빌드 원장 스탠자가 없다")
    _require(rendered.find(marker, idx + 1) < 0, "빌드 원장 스탠자가 두 번 이상 나온다")
    body_start = idx + len("RUN python3 - <<'PY'\n")
    body_end = rendered.index("\nPY\n", body_start)
    return idx, rendered[body_start:body_end + 1], body_end


def _ledger_self_test() -> None:
    """빌드 원장 스탠자 — 파생(ARG)·배치(마지막 RUN)·금지(`import vllm`)·분류(실행) 를 실제로 태운다."""
    import tempfile
    man = {"cpu_arch": "aarch64", "nas_model_path": "/nas"}
    res_src = {"vllm_version": "0.26.0", "torch": {"pin": "2.12.0"},
               "ngc_base": {"tag": "26.05-py3", "cuda_version": "13.2.0.046"},
               "build_track": {"decision": "source-build"}, "source_build": {"torch_cuda_arch": "12.1a"},
               "wheel": {}}
    res_whl = {"vllm_version": "0.18.0", "torch": {"pin": "2.10.0"},
               "ngc_base": {"tag": "26.01-py3", "cuda_version": "13.1"},
               "build_track": {"decision": "wheel"}, "source_build": {},
               "wheel": {"cuda": "130", "manylinux": "manylinux_2_35"}}
    cases = (("Dockerfile.source-build.template", res_src, "Dockerfile.source-build"),
             ("Dockerfile.template", res_whl, "Dockerfile"))
    ledger_body = None
    for tname, res, dname in cases:
        tpath = os.path.join(SHARED_TEMPLATE_DIR, tname)
        with open(tpath, encoding="utf-8") as fh:
            ttext = fh.read()
        out = render(tpath, man, res)
        idx, body, body_end = _ledger_stanza_of(out)
        args = ledger_build_args(ttext)
        _require(args, f"{tname}: 템플릿에 ARG 가 없다(파생 원천 부재)")
        _require(f"ARGS = {json.dumps(args)}\n" in body, f"{tname}: 원장 ARGS 가 템플릿 ^ARG 줄과 다르다")
        rendered_args = set(_LEDGER_ARG_RE.findall(out))
        _require(rendered_args == set(args), f"{tname}: 렌더 본문 ARG {sorted(rendered_args)} ≠ 원장 {args}")
        _require(f"DOCKERFILE = {json.dumps(dname)}\n" in body, f"{tname}: 원장 dockerfile 이름이 템플릿 파생이 아니다")
        # 캐시 불변식: 원장 조립은 **마지막 RUN** 이다(뒤에 RUN 이 오면 분류 수정이 그 레이어들까지 무효화).
        _require(not re.search(r"^RUN\b", out[body_end:], re.M), f"{tname}: 원장 스탠자 뒤에 RUN 이 있다(마지막이어야 한다)")
        # 금지: 빌드 시 vllm import(= _C 로드 시도) — libcuda 가 없어 죽는다(source-build.md §1.5).
        _require(not re.search(r"^\s*(import\s+vllm|from\s+vllm)", body, re.M), f"{tname}: 원장이 vllm 을 import 한다")
        compile(body, f"<ledger:{tname}>", "exec")   # 문법 오류는 SyntaxError 로 fail-loud
        ledger_body = ledger_body or body
    # ★ 음성대조(e 의 원장판): 템플릿에 ARG 한 줄을 더하면 **코드 수정 없이** 원장이 그것을 기록한다.
    with tempfile.TemporaryDirectory() as td:
        with open(os.path.join(SHARED_TEMPLATE_DIR, "Dockerfile.source-build.template"), encoding="utf-8") as fh:
            ttext = fh.read()
        probe = os.path.join(td, "Dockerfile.source-build.template")
        with open(probe, "w", encoding="utf-8") as fh:
            fh.write(ttext.replace("ARG BUILD_JOBS=16\n", "ARG BUILD_JOBS=16\nARG SELFTEST_NEW_ARG=0\n", 1))
        _, body2, _ = _ledger_stanza_of(render(probe, man, res_src))
        _require('"SELFTEST_NEW_ARG"' in body2, "새 ARG 가 원장 목록에 자동으로 들어가지 않았다(손목록 회귀)")
        # 분류기 실행 — 상태 줄 > 로그 토큰 > 종료코드 우선순위와 형식 밖 상태 줄의 폴백을 실제로 태운다.
        root = os.path.join(td, "ledger-root")
        d = os.path.join(root, "ledger.d")
        os.makedirs(d)
        files = {
            "pre.inventory": ".gitkeep\n50-gated.sh\n60-marked.sh\n70-badstatus.sh\n",
            "pre.50-gated.sh.rc": "0\n", "pre.50-gated.sh.sha256": "aa\n",
            "pre.50-gated.sh.log": "[50-gated] skip — SM12X_PORT=0 (stock 빌드 경로 불변)\n",
            "pre.60-marked.sh.rc": "0\n", "pre.60-marked.sh.sha256": "bb\n",
            "pre.60-marked.sh.log": "[60] did work — skip nothing\n",
            "pre.60-marked.sh.status": "applied\tmarker ok\n",
            "pre.70-badstatus.sh.rc": "0\n", "pre.70-badstatus.sh.sha256": "cc\n",
            "pre.70-badstatus.sh.log": "[70] 이미 적용됨 — skip\n",
            "pre.70-badstatus.sh.status": "done\n",
            "post.inventory": "10-plain.sh\n",
            "post.10-plain.sh.rc": "0\n", "post.10-plain.sh.sha256": "dd\n",
            "post.10-plain.sh.log": "building...\n",
            "inline.strip-hoist.status": "skipped\taccepts hoist\n",
        }
        for name, text in files.items():
            with open(os.path.join(d, name), "w", encoding="utf-8") as fh:
                fh.write(text)
        env = dict(os.environ, EASY_VLLM_LEDGER_ROOT=root, VLLM_REF="v9.9.9", BUILD_JOBS="4",
                   NVIDIA_PYTORCH_VERSION="26.05")
        env.pop("SM12X_PORT", None)
        proc = subprocess.run([sys.executable, "-c", ledger_body], env=env, capture_output=True, text=True,
                              timeout=180)
        _require(proc.returncode == 0, f"원장 조립 실행 실패: {proc.stderr[-800:]}")
        with open(os.path.join(root, "build_ledger.json"), encoding="utf-8") as fh:
            led = json.load(fh)
        rows = {(r["phase"], r["file"]): r for r in led["patches"]}
        _require(rows[("pre", "50-gated.sh")]["result"] == "skipped"
                 and rows[("pre", "50-gated.sh")]["result_source"] == "log-token",
                 f"★ 자유 문장 skip 이 log-token 으로 분류되지 않았다: {rows.get(('pre', '50-gated.sh'))}")
        _require(rows[("pre", "60-marked.sh")]["result"] == "applied"
                 and rows[("pre", "60-marked.sh")]["result_source"] == "status-file",
                 "상태 줄이 로그 토큰보다 우선하지 않았다")
        bad = rows[("pre", "70-badstatus.sh")]
        _require(bad["result_source"] == "log-token" and "status_file_error" in bad,
                 f"★ 형식 밖 상태 줄이 조용히 채택되거나 오류 표시 없이 버려졌다: {bad}")
        _require(rows[("post", "10-plain.sh")]["result"] == "applied"
                 and rows[("post", "10-plain.sh")]["result_source"] == "exit-code",
                 "자기보고 없는 적용이 exit-code 출처로 기록되지 않았다")
        _require(rows[("inline", "strip-hoist")]["result"] == "skipped", "inline 상태가 원장에 없다")
        _require(led["slots_supported"] == ["build_patch_post", "build_patch_pre"], led["slots_supported"])
        _require(led["build_args"]["VLLM_REF"] == "v9.9.9" and led["build_args"]["BUILD_JOBS"] == "4",
                 "build_args 가 환경(ARG)에서 읽히지 않았다")
        _require(led["build_args"].get("SM12X_PORT") is None, "정의되지 않은 ARG 를 값으로 지어냈다")
        _require(led["base"]["ngc_pytorch_version"] == "26.05", "베이스 ENV 가 읽히지 않았다")
        _require(led["track"] == "source-build" and led["dockerfile"] == "Dockerfile.source-build", led["track"])
        _require(led["provenance"].startswith("measured"), "원장에 출처 표시가 없다")
    _ledger_loop_self_test(man, res_src, ledger_body)
    print("[render] build-ledger self-test OK — ARG 파생(★새 ARG 자동)·마지막 RUN·vllm import 금지·"
          "분류(상태줄>로그토큰>종료코드 · ★형식 밖 상태줄) 실행 대조 정상")


def _run_instructions(rendered: str) -> list:
    """렌더 본문의 RUN 명령(백슬래시 연속줄 결합 · heredoc 본문 제외)."""
    out, cur = [], None
    for line in rendered.splitlines():
        if cur is None:
            if line.startswith("RUN ") and "<<" not in line:
                cur = line[4:]
                if not cur.rstrip().endswith("\\"):
                    out.append(cur); cur = None
                else:
                    cur = cur.rstrip()[:-1]
            continue
        stripped = line.rstrip()
        if stripped.endswith("\\"):
            cur += " " + stripped[:-1].strip()
        else:
            out.append(cur + " " + stripped.strip()); cur = None
    return out


def _ledger_loop_self_test(man: dict, res_src: dict, ledger_body: str) -> None:
    """source 템플릿의 pre/post 루프 RUN 을 **그 텍스트 그대로** POSIX `sh` 로 실행한다(경로만 임시로 바꿈).

    원시 캡처(rc·sha256·log·status·inventory)가 실제로 남는지, 0 이 아닌 종료가 여전히 빌드를 멈추는지
    (fail-loud 불변식), 그리고 그 캡처를 원장 조립기가 읽어 분류하는지까지 한 번에 태운다."""
    import shutil as _sh
    import tempfile
    sh = _sh.which("sh")
    _require(sh, "POSIX sh 가 없다 — 루프 실행 대조 불가(fail-loud)")
    out = render(os.path.join(SHARED_TEMPLATE_DIR, "Dockerfile.source-build.template"), man, res_src)
    runs = _run_instructions(out)
    pre = [r for r in runs if "/tmp/build_patches_src/*.sh" in r]
    post = [r for r in runs if "/tmp/build_patches/*.sh" in r]
    _require(len(pre) == 1 and len(post) == 1, f"pre/post 루프 RUN 을 하나씩 찾지 못했다: pre={len(pre)} post={len(post)}")
    with tempfile.TemporaryDirectory() as td:
        src, pst, root = (os.path.join(td, n) for n in ("src", "post", "root"))
        os.makedirs(src); os.makedirs(pst)

        def loop(cmd: str) -> subprocess.CompletedProcess:
            cmd = (cmd.replace("/tmp/build_patches_src", src).replace("/tmp/build_patches", pst)
                   .replace("/opt/easy-vllm", root))
            return subprocess.run([sh, "-c", cmd], capture_output=True, text=True, timeout=60)

        with open(os.path.join(src, ".gitkeep"), "w", encoding="utf-8"):
            pass
        with open(os.path.join(src, "50-gated.sh"), "w", encoding="utf-8") as fh:
            fh.write('echo "[50-gated] skip — SM12X_PORT=${SM12X_PORT:-0} (stock)"\nexit 0\n')
        with open(os.path.join(src, "60-marked.sh"), "w", encoding="utf-8") as fh:
            fh.write('# model-trigger : Fixture4ForCausalLM\n'
                     'printf "applied\\tfixture marker\\n" > "$EASY_VLLM_PATCH_STATUS"\necho patched\n')
        with open(os.path.join(pst, "10-plain.sh"), "w", encoding="utf-8") as fh:
            fh.write("echo building\n")
        p1, p2 = loop(pre[0]), loop(post[0])
        _require(p1.returncode == 0 and p2.returncode == 0, f"루프 실행 실패: {p1.stderr[-400:]} {p2.stderr[-400:]}")
        _require("[build-patch-src] applying" in p1.stdout and "patched" in p1.stdout,
                 "패치 출력이 빌드 로그로 흐르지 않는다(tee 누락)")
        d = os.path.join(root, "ledger.d")
        for name in ("pre.inventory", "post.inventory", "pre.50-gated.sh.rc", "pre.50-gated.sh.sha256",
                     "pre.50-gated.sh.log", "pre.60-marked.sh.status", "post.10-plain.sh.rc"):
            _require(os.path.isfile(os.path.join(d, name)), f"루프가 원시 사실 {name} 을 남기지 않았다")
        env = dict(os.environ, EASY_VLLM_LEDGER_ROOT=root)
        body = ledger_body.replace('"pre": "/tmp/build_patches_src"', f'"pre": {json.dumps(src)}') \
                          .replace('"post": "/tmp/build_patches"', f'"post": {json.dumps(pst)}')
        proc = subprocess.run([sys.executable, "-c", body], env=env, capture_output=True, text=True, timeout=180)
        _require(proc.returncode == 0, f"원장 조립 실패: {proc.stderr[-600:]}")
        with open(os.path.join(root, "build_ledger.json"), encoding="utf-8") as fh:
            rows = {(r["phase"], r["file"]): r for r in json.load(fh)["patches"]}
        _require(rows[("pre", "50-gated.sh")]["result"] == "skipped", rows.get(("pre", "50-gated.sh")))
        _require(rows[("pre", "60-marked.sh")]["result_source"] == "status-file"
                 and rows[("pre", "60-marked.sh")]["declared_model_trigger"] == "Fixture4ForCausalLM",
                 rows.get(("pre", "60-marked.sh")))
        _require(rows[("post", "10-plain.sh")]["result_source"] == "exit-code", rows.get(("post", "10-plain.sh")))
        _require(rows[("pre", "50-gated.sh")]["script_sha256"] and len(rows[("pre", "50-gated.sh")]["script_sha256"]) == 64,
                 "스크립트 sha256 이 루프에서 캡처되지 않았다")
        # ★ 음성대조: 0 이 아닌 종료는 여전히 빌드를 멈춘다(원장 캡처가 fail-loud 를 삼키면 안 된다).
        with open(os.path.join(src, "70-broken.sh"), "w", encoding="utf-8") as fh:
            fh.write("echo boom\nexit 3\n")
        p3 = loop(pre[0])
        _require(p3.returncode != 0 and "FAIL 70-broken.sh rc=3" in p3.stdout,
                 f"★ 실패한 패치가 빌드를 멈추지 않았다(rc={p3.returncode}): {p3.stdout[-300:]}")
    print("[render] build-ledger loop self-test OK — pre/post RUN 텍스트를 POSIX sh 로 실행 · 원시 캡처 → 원장 분류 · "
          "★실패 패치 fail-loud 유지")


def _require_terraform_flag(manifest_path: str) -> None:
    """Require readiness through the manifest-contract owner."""
    if not os.path.isfile(manifest_path):
        print("[render] FAIL: manifest 부재 — 테라포밍 미완(fail-closed).", file=sys.stderr)
        sys.exit(4)
    try:
        man = load_manifest(manifest_path)
        scripts = os.path.join(_repo_root(), ".claude", "skills", "terraforming_node", "scripts")
        if scripts not in sys.path:
            sys.path.insert(0, scripts)
        import manifest_contract as contract
        result = contract.evaluate_contract(man, man.get("topology"))
    except (ImportError, OSError, ValueError) as exc:
        print("[render] FAIL: manifest readiness 판정 불가(fail-closed): %s" % exc, file=sys.stderr)
        sys.exit(4)
    if not result.get("flag"):
        print("[render] FAIL: manifest/Flag readiness 거부 — %s" % result.get("reason"), file=sys.stderr)
        sys.exit(int(result.get("exit_code") or 4))


# `COPY <dir>/ ...` 는 그 디렉터리가 **빌드 컨텍스트에 없으면 docker build 가 죽는다**.
# 슬롯 산출물(build_patches·build_patches_src)은 2026-09-10 추적 예외 철회로 비추적이 됐고,
# git 은 빈 디렉터리를 들지 않으므로 **fresh clone 에는 그 디렉터리가 아예 없다**.
# ∴ 존재를 보장하는 주체는 git 이 아니라 이 렌더러다(헌법: 배포 단위 = 스켈레톤 + 생성엔진).
#   실측 2026-09-10: `0693af1` 이 인덱스에서 10파일을 뺀 뒤 multi 배포 클론의
#   `build_patches{,_src}/` 가 사라졌고 그 상태의 source-build 는 COPY 에서 죽는다.
#   로컬 워킹트리에는 파일이 남아 있어 **여기서는 보이지 않았다**(배포 클론에서만 드러나는 결함).
# 목록을 손으로 적지 않는다 — **방금 렌더한 본문에서 파생**한다(닫힌 목록을 두면 갈라진다).
_COPY_CONTEXT_DIR_RE = re.compile(r"^\s*COPY\s+(?:--\S+\s+)*([A-Za-z0-9._-]+)/\s+\S", re.M)


def _ensure_copy_context_dirs(rendered: str, out_path: str) -> None:
    """렌더 본문이 COPY 하는 상대 디렉터리를 산출물 옆에 실재시킨다(멱등)."""
    base = os.path.dirname(os.path.abspath(out_path))
    for d in sorted(set(_COPY_CONTEXT_DIR_RE.findall(rendered))):
        target = os.path.join(base, d)
        if not os.path.isdir(target):
            os.makedirs(target, exist_ok=True)
            print(f"[render] 빌드 컨텍스트 디렉터리 생성 → {d}/ "
                  f"(COPY 대상 · 비추적 슬롯이라 clone 에 부재)", file=sys.stderr)
        # 뼈대 선언(2026-09-10 사용자 결정 · `.gitignore` 3단 규칙의 짝). 디렉터리만 만들면 git 이
        # 그것을 들지 못해 **다음 클론에서 또 사라진다** — 자기 빌드킷을 자율 저작하는 노드
        # (싱글 서브 = A2A 에이전트 · 헌법 §불변식 A)에서는 이 파일이 유일한 선언 수단이다.
        # 산출물은 계속 비추적이다 — 추적되는 것은 이 빈 마커 하나뿐이다.
        keep = os.path.join(target, ".gitkeep")
        if not os.path.exists(keep):
            with open(keep, "w", encoding="utf-8"):
                pass
            print(f"[render] 뼈대 선언 → {d}/.gitkeep", file=sys.stderr)


def main() -> None:
    ap = argparse.ArgumentParser(description="render_dockerfile.py — G2 결정론 렌더러")
    ap.add_argument("--self-test", action="store_true", help="내장 self-test(A7 게이트 + NCCL 회귀)")
    ap.add_argument("--nccl-envfile", action="store_true",
                    help="NCCL .env.interconnect 렌더(manifest.interconnect 소비, Plan 2)")
    ap.add_argument("--cluster-envfile", action="store_true",
                    help="Ray .env.cluster 렌더(manifest.nodes[]+CLUSTER_PRESETS 소비, S6 env-split)")
    ap.add_argument("--materialize-configs", action="store_true",
                    help="러너 스크립트(serve_runner/debug-init)를 output/<topology>/configs/ 로 복사(통로 self-containment, plan_26062321)")
    ap.add_argument("--materialize-env", action="store_true",
                    help="output/<topology>/.env 를 manifest(nas_model_path)+tiktoken_cache 에서 생성(serve-time NAS 마운트 정합, 결함#2)")
    ap.add_argument("--topology", choices=["single", "multi"], help="--materialize-configs/--materialize-env 대상 통로")
    ap.add_argument("--repo", help="repo 루트(미지정 시 스크립트 위치 기준 자동)")
    ap.add_argument("--template", help="legacy caller-provided template path")
    ap.add_argument("--canonical-kind", choices=["dockerfile", "source-build", "compose"],
                    help="render synchronized canonical source (requires explicit --topology)")
    ap.add_argument("--manifest", default="manifest.yaml")
    ap.add_argument("--resolved", help="legacy caller-provided resolution; canonical rendering forbids it")
    ap.add_argument("-o", "--out", help="출력 파일(미지정 시 stdout)")
    ap.add_argument("--allow-unvalidated", action="store_true",
                    help="미검증 (NGC×vLLM) 키의 source-build 가드를 WARN 으로 강등(사람 승인 전제 시도-빌드 — "
                         "스모크가 중재·testlog 기록 의무·통과 시 키 codify. 기본=fail-loud 유지)")
    a = ap.parse_args()

    if a.allow_unvalidated:
        global ALLOW_UNVALIDATED
        ALLOW_UNVALIDATED = True
        print("[render] --allow-unvalidated: source-build 가드 WARN 강등(HITL 시도-빌드 — testlog 기록 의무)",
              file=sys.stderr)

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

    # 헌법 §테라포밍-완수 Flag 게이트 — render deliverable(canonical-kind/template)은 Flag 전제.
    # env-file/materialize 는 serve-time 런타임 설정이라 게이트 대상 아님(info-only).
    if a.canonical_kind or a.template:
        _require_terraform_flag(a.manifest)

    if a.canonical_kind:
        if not a.topology or a.template or a.resolved:
            print("[render] FAIL: --canonical-kind requires --topology and forbids --template/--resolved",
                  file=sys.stderr)
            sys.exit(2)
        manifest = load_manifest(a.manifest)
        out = render_shared(a.canonical_kind, a.topology, manifest)
    elif a.template:
        manifest = load_manifest(a.manifest)
        resolved = load_resolved(a.resolved or "resolved.json")
        out = render(a.template, manifest, resolved)
    else:
        _self_test()
        return
    if a.out:
        with open(a.out, "w", encoding="utf-8") as f:
            f.write(out)
        print(f"[render] deterministic render → {a.out} ({len(out)} bytes)", file=sys.stderr)
        _ensure_copy_context_dirs(out, a.out)
    else:
        sys.stdout.write(out)


if __name__ == "__main__":
    main()
