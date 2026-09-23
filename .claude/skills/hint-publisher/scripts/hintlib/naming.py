"""hintlib.naming — hint 태그 이름: 도구 전량 파생(D5~D8) · 정규 어휘표 · 신/구 문법 파서(구 문법 읽기 전용).

    hint/<vllm>/<model>/<arch>/<recipe>
      <vllm>   빌드 입력(X18)        릴리스 태그 → `0.29.0rc6` · 커밋 핀 → `<직전 릴리스>-g<sha12>`
      <model>  체크포인트 슬러그      HF repo 이름 또는 체크포인트 basename 소문자(= 인증서 model 키)
      <arch>   `<hw>-<G>g<N>n-<main|sub|cluster>-<target>[-<plane>]`   G=노드당 GPU · N=노드 수 · target=native|sim-<hw> ·
               plane = 실행 평면 토큰(vocab `plane` · docker = 토큰 없음 · native = `bare`)
      <recipe> `q<quant>-len<n>-kv<dtype>-ple<mode>-spec<k|off>-<graph|eager>`   순서 고정 · 전 축 필수

왜 이 모양인가 — 옛 hint_tag.py 에서 옮겨 온 날짜 박힌 불변식(삭제 ✗ · 코드맵 hint_tag_a §2.A·§8)
    - ★ 2026-09-04(CP7 · plan_26090415 §7.5 M1): **5세그먼트**. 레시피 축이 없어서 한 스윕의 여러 셀이 같은 이름을
      원했고 기존 이름 하드 차단으로 발행이 막혔다(레시피 변이를 가진 모델 6종). `serving_config` 는 축이 못 된다
      (45건 중 12건만 채워졌고 모델명으로 덮였다). 레시피를 **맨 뒤**에 붙인 이유: 모델 슬러그 인덱스
      (`split("/")[2]`)가 그대로 살아 변경 표면이 가장 작다 — 이 순서는 지금도 불변이다.
    - ★ 2026-09-06(plan_26090616 Q1/Q2 · 사용자 결정): arch 에 **노드 축**. hint 태그는 "이 조합이 된다"가 아니라
      **"어느 노드 형상이 수행한 기록"** 이다. 노드 축이 없던 `gb10-sim-h100` 시절 메인·서브가 같은 이름을 원했고,
      서브는 완주했는데 태그가 나가지 못했다(2026-09-05 캠페인: 기대 3종 중 2종 발행). `cluster` 는 쌍을 **하나의
      수행 정체성**으로 본다. 축은 arch **안에서** 늘린다(세그먼트 수를 늘리면 이름을 해체하는 모든 자리가 깨진다).
      옛 문법은 **다른 사유코드**로 가른다 — "형태 위반"과 "옛 문법"을 한 메시지로 뭉개면 고치는 사람이 무엇을
      고칠지 모른다. 이 원칙은 세대가 하나 늘어도 그대로다(`HINT_ARCH_LEGACY_GRAMMAR` 신설).
    - ★ 2026-09-23(plan_26092311 O-N1 = A · 사용자 승인): arch 끝에 **선택 평면 토큰** `-<plane>`. 이름 문법에 실행 평면
      축이 없어서 native(비-Docker) 셀이 축이 같은 Docker 셀과 한 이름을 원했다(N1 파생 이름 = D1 발행 태그 →
      HINT_NAME_COLLISION · F10). Docker 는 토큰이 없다(vocab 이 docker="" 를 강제) — 옛·신 Docker 태그 이름은 바이트 불변이고,
      native 만 `-bare` 를 붙인다. 평면 판정은 `artifacts.plane_of` 단독 소유(evidence 가 facts 로 싣는다 · 여기서 다시 판정 ✗).
      레시피 7번째 축(O-N1 B)을 택하지 않은 이유: 레시피는 "순서 고정 · 전 축 필수"라 옛 v6 태그 전부와 모양이 갈라진다.
    - ★ 2026-09-11(plan_26091108 R9): 레시피 4번째 축 `ple`. 3축은 camp-26090918 의 res·mmp 셀을 가르지 못해
      타임스탬프 접미로 유일화했고, 그 함수 주석이 이미 "반복되면 축을 늘려야 한다"고 적어 두었다.
    - ★ 2026-08-20(plan_26082008 R1 · 사용자 D2): 슬러그는 **발행자가 짓지 않는다**. 발행된 32 슬러그 중 27종이
      정본표 밖이었고 같은 모델이 철자로 2건 갈라졌다(`gemma-4-e2b-it`↔`-E2B-it` · `qwen3.5-…`↔`qwen35-…`).
    - ★ 2026-08-15(IDENTITY_MISMATCH:model · testlog_26081519 §9 B3): 인증서 `model` = **체크포인트 basename
      소문자**(adversarial-benchmark `sweep_bench.sh`) = 태그 `<model>` 세그먼트. 두 규칙이 갈리면 발행이 막힌다 —
      그래서 자체검사가 producer 리터럴을 열어 교차 대조한다(`_SLUG_PRODUCER`).
    - 이름은 **읽히는 토큰**이지 해시가 아니다(`policy:GIT_SINGLE_AUTHORITY` — 해시는 인증서가 이미 드는 값을 두
      번째 자리에 적는다). 태그는 불변이다 — 소급 개명 ✗(P1 리콜 금지). 옛 태그는 **읽기 전용으로 수용**만 하고
      판정하지 않는다(D10).

2026-09-21 개정(plan_26092119 §4.7 · D5~D8 · 코드맵 B9/F13)
    - D5 `<vllm>` = **빌드 입력**. 옛 이름은 엔진 자기보고(`0.29.0`)를 적어 rc6 소스빌드와 릴리스가 한 이름을 썼다.
      2026-09-22 통합 결정: D-g 릴리스 문법에 4마디(`M.m.p.q`)·`.postN` 포함 · D-h 소스빌드의 릴리스 모양 ref 는
      **업스트림 VLLM_REPO 일 때만** 릴리스 이름이다(포크 = `<직전 릴리스>-g<sha12>` 또는 차단 · naming facts 가 vllm_repo 를 싣는다).
    - D6 레시피 = 고정 6축 · 정규 어휘(`hints/vocab.json`) · **전 축 필수** · 값 없음은 명시 토큰(`plenone`·`specoff`).
      옛 `_recipe_token` 은 결측을 **생략**했고(`len262144-kvauto-plemmap` 처럼 q 가 사라짐), 비영숫자만 떼어
      `fp8_e4m3`→`kvfp8e4m3` 와 `fp8`→`kvfp8` 이 같은 dtype 을 두 이름으로 갈랐다(F13).
    - D7 arch = GPU 수·노드 수 분리. 옛 `x2` 는 `gb10x2`(2노드)와 `rtxpro6000x2`(2 GPU)를 구분하지 못했다(F13).
    - D8 이름 = **도구 전량 파생** — 발행자 입력 ✗. 파생 불가 축 = 차단(`HINT_AXIS_UNDERIVABLE`) · 어휘 밖 = 차단
      (`HINT_VOCAB_UNKNOWN`). 입력 계약은 SPEC §3.3 의 naming facts(evidence 가 조립)다.
    - 퇴역: `collide_suffix`(X13 — 충돌은 이제 차단이다 · 접미로 유일화하지 않는다) · `_existing_model_slugs` 철자
      재사용(D8 파생 전용 — 이전 철자를 끌어오면 인증서 model 키와 갈라진다) · `require_derived_slug`·family 연속성
      예외(families 폐기 · O3) · `LOCKSET_RECIPE_KEYS`·`_declared_ple_mode`(셀 사실 수집 → evidence) ·
      `DECLARED_PLE_MODES`(→ vocab.json `ple`).

순수 모듈: **import 시점 부수효과 0**(git·파일·경로 계산 없음). 옛 hint_tag.py 를 파일 경로로 적재하던
campaign_template_validator 는 import 한 번에 git 을 불렀고, 부재 시 arch 검사를 **침묵 생략**했다(코드맵 H1/H3).
git 이 필요한 검사(존재·원격 충돌·check-ref-format)는 tag.py 가 한다.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import unquote, urlparse

from . import core
from .core import HintError, fail

# ── 문법 세대 ────────────────────────────────────────────────────────────────────────────────
GRAMMAR_V6 = "v6"                              # <hw>-<G>g<N>n-<role>-<target> + 고정 6축 레시피
GRAMMAR_LEGACY_ARCH_NODE = "legacy-5seg-node"  # 2026-09-06 ~ 09-21: <hw>-<role>-<target> (예 gb10x2-cluster-native)
GRAMMAR_LEGACY_ARCH = "legacy-5seg"            # 2026-09-04 ~ 09-06: <hw>-<target>, 노드 축 없음 (예 gb10-sim-h100)
GRAMMAR_LEGACY_4SEG = "legacy-4seg"            # 2026-09-04 이전 hint/<vllm>/<model>/<arch> — grammar_of() 만 말한다
GRAMMAR_UNKNOWN = "unknown"                    # 5세그먼트지만 어느 세대에도 맞지 않음(타 발행처 · P2 인지만)

NODE_AXIS = ("main", "sub", "cluster")
VOCAB_AXES = ("hw", "quant", "kv", "ple", "graph")
ARCH_AXES = ("hw", "gpus_per_node", "nodes", "role", "target")
# 평면 축(2026-09-23 O-N1): ARCH_AXES 밖에 따로 둔다 — O-N1 이전 PAYLOAD.naming.axes 에는 이 칸이 없고(전부 Docker),
#   ARCH_AXES 를 순회하는 재조립이 옛 페이로드에서 KeyError 로 깨지면 안 된다. 닫힌 어휘는 판정 소유자
#   `artifacts._plane`(docker|native)과 같고, vocab `plane` 의 키 집합이 정확히 이것이어야 한다(validate_vocab).
PLANE_AXIS = "plane"
PLANE_NAMES = ("docker", "native")
_PLANE_NO_TOKEN = "docker"   # 문법 불변식: Docker 이름은 평면 토큰을 갖지 않는다(옛·신 태그 불변 · vocab 이 "" 로 강제)
RECIPE_AXES = ("q", "len", "kv", "ple", "spec", "graph")
_GRAPH_TOKENS = frozenset({"graph", "eager"})   # 문법이 고정한 두 토큰(vocab 가 바꾸지 못한다)

# ── 형태 ─────────────────────────────────────────────────────────────────────────────────────
_TOKEN_RE = re.compile(r"^[a-z0-9]+$")
# X18 릴리스 문법(2026-09-22 통합 결정 D-g 로 확장): `M.m.p` + 선택 4번째 마디(`M.m.p.q`) + 선택 단계(`rc6`·`a1` …) +
#   선택 `.postN`. 옛 판본(`M.m.p[<문자><수>]`)은 4마디(`M.m.p.q`)·`.postN` 릴리스 태그를 릴리스로 읽지 못해
#   그 빌드를 전부 HINT_AXIS_UNDERIVABLE 로 막았다(fail-closed 였지만 거짓 차단). 세 정규식이 **한 조각**을 공유한다 —
#   릴리스 판정 · describe 해체 · 세그먼트 문법이 따로 적히면 한쪽만 넓어져 파생한 이름을 문법이 거부한다.
#   nightly `.devN+g…`(타 발행처 옛 이름)는 여전히 릴리스가 아니다(읽기 전용 문법 판정 그대로).
_REL = r"\d+\.\d+\.\d+(?:\.\d+)?(?:[a-z]+\d+)?(?:\.post\d+)?"
_RELEASE_RE = re.compile(rf"^v?(?P<rel>{_REL})$")
_DESCRIBE_RE = re.compile(rf"^v?(?P<rel>{_REL})(?:-\d+-g(?P<g>[0-9a-f]{{4,40}}))?$")
_SHA40_RE = re.compile(r"^[0-9a-f]{40}$")
_VLLM_SEGMENT_RE = re.compile(rf"^{_REL}(?:-g[0-9a-f]{{12}})?$")
# 업스트림 vLLM 저장소(정규화 `host/owner/repo`) — tripwire 닫힌 목록(2026-09-22 통합 결정 D-h). 소스빌드의 릴리스 모양
#   VLLM_REF 는 **이 저장소의 태그일 때만** 릴리스 이름이 된다. 포크(비-업스트림 VLLM_REPO)의 `v0.29.0rc6` 은 업스트림
#   0.29.0rc6 과 다른 커밋일 수 있다 — 같은 이름을 쓰면 수신자는 업스트림 릴리스로 읽는다. 포크는 SHA 경로
#   (`<직전 릴리스>-g<sha12>`)로만 이름 짓고, SHA 가 없으면 차단한다. 목록을 늘리는 것은 사람 편집이다(미러 등).
UPSTREAM_VLLM_REPOS = ("github.com/vllm-project/vllm",)
_REPO_URL_RE = re.compile(r"^(?:[A-Za-z][A-Za-z0-9+.-]*://)?(?:[^@/\s]+@)?(?P<host>[^/:\s]+)[:/](?P<path>[^\s]+?)(?:\.git)?/*$")
_MODEL_SLUG_RE = re.compile(r"^[a-z0-9][a-z0-9._-]*$")
# v6 arch 인식은 느슨하게, 판정은 엄격하게 — `gb10-0g1n-main-native` 같은 잘못된 v6 가 옛 세대(노드 축 없음)로
# 오분류되면 고치는 사람이 엉뚱한 것을 고친다(사유코드 분리 원칙).
_ARCH_V6_LIKE_RE = re.compile(r"^[^-]+-\d+g\d+n(?:-|$)")
# 끝의 선택 평면 토큰(`-<plane>` · O-N1)은 **형태만** 본다 — 토큰 어휘는 vocab `plane` 이 소유하므로(코드에 사본 ✗) 파생·재조립이
#   어휘로 대조한다. sim 타겟은 `sim-[a-z0-9]+` 라 `-` 를 품지 않아 평면 토큰과 모호하지 않다.
_ARCH_V6_RE = re.compile(r"^(?P<hw>[a-z0-9]+)-(?P<g>[1-9]\d*)g(?P<n>[1-9]\d*)n-"
                         r"(?P<role>main|sub|cluster)-(?P<target>native|sim-[a-z0-9]+)(?:-(?P<plane>[a-z0-9]+))?$")
_ARCH_LEGACY_NODE_RE = re.compile(r"^(?P<hw>[a-z0-9]+)-(?P<role>main|sub|cluster)-(?P<target>[a-z0-9][a-z0-9-]*)$")
_ARCH_LEGACY_RE = re.compile(r"^(?P<hw>[a-z0-9]+)-(?!main-|sub-|cluster-)(?P<target>[a-z0-9][a-z0-9-]*)$")
_RECIPE_V6_RE = re.compile(r"^q(?P<q>[a-z0-9]+)-len(?P<len>[1-9]\d*)-kv(?P<kv>[a-z0-9]+)-ple(?P<ple>[a-z0-9]+)"
                           r"-spec(?P<spec>[1-9]\d*|off)-(?P<graph>graph|eager)$")
# 인증서·스윕 meta 의 "읽지 못함" 표기 — 값이 아니다(읽지 못함 ≠ 없음).
_UNOBSERVED = frozenset({"", "n/a", "na", "null", "unknown", "?"})
# 템플릿 빈칸(`<<FILL>>` — campaigns/_template · manifest `__REQUIRED__`)도 값이 아니다. 채워지지 않은 선언을 어휘 밖
# 값으로 읽으면 remedy 가 "그 빈칸 문자열을 어휘표에 추가하라"는 엉뚱한 처방을 낸다(2026-09-22 감사).
_TEMPLATE_BLANK_RE = re.compile(r"^(?:<<[^<>]*>>|__[A-Z_]+__)$")


def _is_unobserved(text) -> bool:
    """문자열이 '읽지 못함' 표기 또는 템플릿 빈칸이면 참(값이 아니다 → 파생 불가로 차단 · 어휘 등재 금지)."""
    if not isinstance(text, str):
        return False
    return _vnorm(text) in _UNOBSERVED or bool(_TEMPLATE_BLANK_RE.match(text.strip()))

# 축이 파생 불가일 때 "어느 증거가 있어야 하는가" (remedy 용 · plan §4.7 결정론 출처 열)
_AXIS_EVIDENCE = {
    "vllm": "셀 env VLLM_REF(소스빌드)/VLLM_VERSION(wheel) · native wheel URL · 소스빌드 릴리스 모양 ref 는 VLLM_REPO(업스트림 "
            "여부 · D-h) · 직전 릴리스 = 빌드 원장 vllm.describe 또는 이미지 안 `git describe --tags --match 'v*' --abbrev=0`",
    "model": "서빙 yaml `model:`(체크포인트 경로) 또는 HF 카드 repo id",
    "arch": "output/<topology>/manifest.yaml gpu_model·gpus_per_node·nodes · 측정 노드(sweep meta measured_node) · "
            "셀 config target_gpu(시뮬레이션 타겟 선언 여부)",
    "plane": "artifacts.plane_of(셀 env IMAGE_TAG·BUILD_DOCKERFILE = docker · native serve-proof plane=native)",
    "q": "체크포인트 config.json quantization_config / hf_quant_config.json(부재를 관측했으면 'none')",
    "len": "서빙 yaml max-model-len",
    "kv": "서빙 yaml kv-cache-dtype(부재를 관측했으면 엔진 기본값 'auto')",
    "ple": "셀 config.yaml declared_axes.ple_mode · 모델 config(PLE 부재 판정이면 'none')",
    "spec": "서빙 yaml·러너 speculative-config(부재를 관측했으면 raw=None → specoff)",
    "graph": "서빙 yaml enforce-eager(eager|graph 로 변환해 넘긴다)",
}

# ★ 교차검증 앵커 — 인증서 model 키의 정본은 producer(`sweep_bench.sh`)의 코드 토큰이다(주석 ✗ · 리팩터를 따라간다).
#   derive_slug 의 model_path 규칙은 그 **거울**이라, 정본이 바뀌면 여기 사본이 조용히 틀린 말을 한다.
#   자체검사가 producer 파일을 열어 리터럴 실재를 붙잡는다(campaign_template_validator `_DERIVED_NODE_PRODUCER` 선례 ·
#   workflow.md §결정론 규율 "단일 소유가 불가능하면 교차검증이 차선이다").
_SLUG_PRODUCER = (
    ".claude/skills/adversarial-benchmark/scripts/sweep_bench.sh",
    ('_mbase = os.path.basename(_mpath.rstrip("/")).strip()',
     'model_key, model_source = _mbase.lower(), "derived(model_path basename · lowercased)"'),
)


# ── 자료형 ───────────────────────────────────────────────────────────────────────────────────
@dataclass(frozen=True)
class TagName:
    vllm: str
    model: str
    arch: str
    recipe: str
    grammar: str

    @property
    def tag(self) -> str:
        return f"{core.HINT_TAG_PREFIX}{self.vllm}/{self.model}/{self.arch}/{self.recipe}"


@dataclass(frozen=True)
class Axis:
    value: str
    source: str
    token: str | None = None   # 값과 이름 토큰이 다른 축만(평면: value=docker|native · token=""|vocab 값)

    def as_dict(self) -> dict:
        d = {"value": self.value, "source": self.source}
        if self.token is not None:
            d["token"] = self.token
        return d


@dataclass(frozen=True)
class DerivedName:
    tag: str
    segments: dict   # {"vllm","model","arch","recipe"} → Axis
    axes: dict       # ARCH_AXES + (PLANE_AXIS) + RECIPE_AXES → Axis (값은 토큰 · 접두 없음 — recompose 가 다시 조립한다)
    vllm_build_input: dict   # {"kind": release|commit|wheel, "ref", "sha": <40>|None, "prev_release": …|None}

    def to_payload(self) -> dict:
        """PAYLOAD.json `naming` 의 도구 파생분(`vllm_observed` 는 evidence 가 덧붙인다)."""
        return {"grammar": GRAMMAR_V6,
                "segments": {k: v.as_dict() for k, v in self.segments.items()},
                "axes": {k: v.as_dict() for k, v in self.axes.items()},
                "vllm_build_input": dict(self.vllm_build_input)}


# ── 이름 해체 (단일 소유자) ─────────────────────────────────────────────────────────────────────
def _split5(name) -> list[str] | None:
    parts = name.split("/") if isinstance(name, str) else []
    if len(parts) != 5 or parts[0] != "hint" or not all(parts):
        return None
    return parts


def _grammar(vllm: str, model: str, arch: str, recipe: str) -> str:
    if (_VLLM_SEGMENT_RE.match(vllm) and _MODEL_SLUG_RE.match(model)
            and arch_violation(arch) is None and recipe_violation(recipe) is None):
        return GRAMMAR_V6
    if _ARCH_V6_LIKE_RE.match(arch):
        return GRAMMAR_UNKNOWN          # v6 처럼 생겼는데 어긋남 — 옛 세대로 오판하지 않는다
    if _ARCH_LEGACY_NODE_RE.match(arch):
        return GRAMMAR_LEGACY_ARCH_NODE
    if _ARCH_LEGACY_RE.match(arch):
        return GRAMMAR_LEGACY_ARCH
    return GRAMMAR_UNKNOWN


def parse_tag(name: str) -> TagName:
    """`hint/<vllm>/<model>/<arch>/<recipe>` → TagName(+문법 세대). **이름 해체의 단일 소유자.**

    종전 `_, vllm, model, arch = tag.split("/")` 가 5곳에 흩어져 있었다 — 세그먼트가 하나 늘면 그 다섯이 동시에
    깨지고, 하나라도 놓치면 그 경로만 조용히 옛 모양을 가정한다(코드맵 D4). 옛 세대는 **읽기 전용으로 수용**
    (카탈로그 나열용 · 판정 ✗ · P1). 5세그먼트가 아니면 HINT_NAME_SHAPE.
    """
    parts = _split5(name)
    if parts is None:
        fail("HINT_NAME_SHAPE", f"이름 형태 위반(hint/<vllm>/<model>/<arch>/<recipe> 5세그먼트): {name!r}",
             "이름은 손으로 짓지 않는다 — `hint.py name --campaign <id> --cell <cell>` 이 파생 이름을 보여 준다.")
    _, vllm, model, arch, recipe = parts
    return TagName(vllm, model, arch, recipe, _grammar(vllm, model, arch, recipe))


def grammar_of(name: str) -> str:
    """문법 세대 라벨(예외 없음 · 카탈로그 `문법` 컬럼용). 4세그먼트 옛 이름(`hint/<vllm>/<model>/<arch>`)도
    읽기 전용으로 라벨만 붙인다(plan §4.7 "옛 4·5세그먼트 수용")."""
    parts = _split5(name)
    if parts is not None:
        return _grammar(*parts[1:])
    p4 = name.split("/") if isinstance(name, str) else []
    if len(p4) == 4 and p4[0] == "hint" and all(p4):
        return GRAMMAR_LEGACY_4SEG
    return GRAMMAR_UNKNOWN


def plane_of_token(token: str, vocab: dict | None = None) -> str | None:
    """arch 평면 토큰 → 평면 이름(읽기 전용 · 예외 없음). 빈 토큰 = docker(문법 불변식 · vocab 없이도 참). 비어 있지 않은 토큰은
    vocab `plane` 으로만 역조회한다 — vocab 이 없거나 어휘 밖이면 None(추측 ✗)."""
    if not token:
        return _PLANE_NO_TOKEN
    table = (vocab or {}).get(PLANE_AXIS) if isinstance(vocab, dict) else None
    if not isinstance(table, dict):
        return None
    hits = [p for p, t in table.items() if not str(p).startswith("_") and t == token]
    return hits[0] if len(hits) == 1 else None


def parse_arch(arch: str, vocab: dict | None = None) -> dict | None:
    """arch → 축 사전(세대 무관 · 읽기 전용). v6 = {grammar, hw, gpus_per_node, nodes, role, target, plane_token, plane} ·
    옛 노드축 = {grammar, hw, role, target} · 옛 무노드 = {grammar, hw, target}. 해석 불가 = None.
    카탈로그 match 가 문자열 정확일치 대신 **축별** 비교를 하는 데 쓴다(코드맵 §1.9). v6 의 `plane` 은 토큰이 없으면 docker,
    있으면 vocab 역조회(vocab 미지정·어휘 밖 = None · O-N1)."""
    if not isinstance(arch, str):
        return None
    if arch_violation(arch) is None:
        m = _ARCH_V6_RE.match(arch)
        tok = m["plane"] or ""
        return {"grammar": GRAMMAR_V6, "hw": m["hw"], "gpus_per_node": int(m["g"]), "nodes": int(m["n"]),
                "role": m["role"], "target": m["target"], "plane_token": tok, "plane": plane_of_token(tok, vocab)}
    if _ARCH_V6_LIKE_RE.match(arch):
        return None
    if (m := _ARCH_LEGACY_NODE_RE.match(arch)):
        return {"grammar": GRAMMAR_LEGACY_ARCH_NODE, "hw": m["hw"], "role": m["role"], "target": m["target"]}
    if (m := _ARCH_LEGACY_RE.match(arch)):
        return {"grammar": GRAMMAR_LEGACY_ARCH, "hw": m["hw"], "target": m["target"]}
    return None


def parse_recipe(recipe: str) -> dict | None:
    """v6 레시피 → {q, len, kv, ple, spec, graph}(토큰 · spec 은 숫자 문자열 또는 'off'). v6 가 아니면 None."""
    m = _RECIPE_V6_RE.match(recipe) if isinstance(recipe, str) else None
    return None if m is None else {k: m[k] for k in RECIPE_AXES}


# ── 판정 (순수) ─────────────────────────────────────────────────────────────────────────────────
def arch_violation(arch: str) -> str | None:
    """arch 세그먼트 위반 사유코드(정상이면 None). **arch 문법의 단일 소유자.**

    HINT_ARCH_ABSENT · HINT_ARCH_SHAPE_VIOLATION · HINT_ARCH_ROLE_NODES_MISMATCH(v6 형태지만 cluster⇔N≥2 위반) ·
    HINT_ARCH_LEGACY_GRAMMAR(2026-09-06 세대: 노드 축은 있으나 GgNn 없음) ·
    HINT_ARCH_NODE_AXIS_ABSENT(2026-09-04 세대: 노드 축 없음).
    """
    if not arch or not isinstance(arch, str):
        return "HINT_ARCH_ABSENT"
    if _ARCH_V6_LIKE_RE.match(arch):
        m = _ARCH_V6_RE.match(arch)
        if not m:
            return "HINT_ARCH_SHAPE_VIOLATION"
        # cluster 는 쌍(N≥2)을 하나의 수행 정체성으로 본다. main/sub 는 그 노드 **하나**가 수행한 기록이다(N=1).
        # main 인데 N=2 같은 조합은 증거가 스스로 모순된 것이라 이름으로 굳히지 않는다.
        if (m["role"] == "cluster") != (int(m["n"]) >= 2):
            return "HINT_ARCH_ROLE_NODES_MISMATCH"
        return None
    if _ARCH_LEGACY_NODE_RE.match(arch):
        return "HINT_ARCH_LEGACY_GRAMMAR"
    if _ARCH_LEGACY_RE.match(arch):
        return "HINT_ARCH_NODE_AXIS_ABSENT"
    return "HINT_ARCH_SHAPE_VIOLATION"


def recipe_violation(recipe: str) -> str | None:
    """v6 레시피 위반 사유코드(정상이면 None). 형태만 본다(어휘 소속은 derive_name 이 보장하고, tag.py verify 가
    PAYLOAD.naming 재조립으로 대조한다). 옛 레시피(`qmxfp4-len131072-kvfp8` · `len262144-kvauto-plemmap`)는
    축이 빠졌거나 순서가 달라 여기서 걸린다 — 옛 태그는 판정하지 않고 신규 이름에만 쓴다."""
    if not recipe or not isinstance(recipe, str):
        return "HINT_RECIPE_ABSENT"
    return None if _RECIPE_V6_RE.match(recipe) else "HINT_RECIPE_SHAPE_VIOLATION"


_ARCH_MESSAGES = {
    "HINT_ARCH_ABSENT": "arch 세그먼트가 비었다.",
    "HINT_ARCH_SHAPE_VIOLATION": "arch 형태 위반(소문자 <hw>-<G>g<N>n-<main|sub|cluster>-<native|sim-<hw>>[-<plane>]).",
    "HINT_ARCH_ROLE_NODES_MISMATCH": "노드 축과 노드 수가 모순된다 — cluster 는 N≥2, main·sub 는 N=1 이다.",
    "HINT_ARCH_LEGACY_GRAMMAR": (
        "옛 arch 문법(<hw>-<main|sub|cluster>-<target> · 2026-09-06 세대)이다 — 신규 이름은 GPU 수·노드 수를 분리한 "
        "<hw>-<G>g<N>n-<role>-<target> 을 쓴다(D7). 옛 `x2` 는 gb10x2(2노드)와 rtxpro6000x2(2 GPU)를 구분하지 못했다(F13)."),
    "HINT_ARCH_NODE_AXIS_ABSENT": (
        "arch 에 **노드 축이 없다**(2026-09-04 세대 <hw>-<target>). hint 태그는 '이 조합이 된다'가 아니라 "
        "'어느 노드 형상이 수행한 기록'이다 — 축이 없으면 같은 하드웨어의 메인·서브가 같은 이름을 원하고, 서브의 "
        "기록은 완주했어도 발행되지 못한다(2026-09-05 실측)."),
}


def validate_new_name(tag: str) -> None:
    """신규 발행 이름이 v6 문법인가(순수). 위반은 세그먼트별 사유코드로 HintError.

    `git check-ref-format`·존재·원격 충돌은 git 이 필요하므로 tag.py 가 한다(이 모듈은 import 부수효과 0).
    옛 세대 이름은 parse_tag 로 **읽을 수는 있지만** 여기서는 세대별 사유코드로 거부된다.
    """
    t = parse_tag(tag)
    remedy = "이름은 손으로 짓지 않는다 — `hint.py name --campaign <id> --cell <cell>` 로 파생 이름과 축별 출처를 본다."
    if not _VLLM_SEGMENT_RE.match(t.vllm):
        fail("HINT_VLLM_SEGMENT_SHAPE",
             f"vLLM 세그먼트 형태 위반({t.vllm!r}) — 릴리스(`0.29.0rc6`, v 없음) 또는 `<직전 릴리스>-g<sha12>` 만 쓴다(X18).",
             remedy)
    if not _MODEL_SLUG_RE.match(t.model):
        fail("HINT_MODEL_SLUG_SHAPE", f"모델 슬러그 형태 위반({t.model!r}) — 소문자 [a-z0-9._-] 파생값만 쓴다.", remedy)
    why = arch_violation(t.arch)
    if why is not None:
        fail(why, f"{_ARCH_MESSAGES.get(why, why)} arch={t.arch!r}", remedy)
    why = recipe_violation(t.recipe)
    if why is not None:
        fail(why, f"레시피 세그먼트 형태 위반({t.recipe!r}) — q<quant>-len<n>-kv<dtype>-ple<mode>-spec<k|off>-<graph|eager> "
                  "순서 고정 · 전 축 필수(D6).", remedy)


# ── 조립 ─────────────────────────────────────────────────────────────────────────────────────
def _pos_int(value, what: str) -> int:
    if isinstance(value, bool) or not isinstance(value, (int, str)):
        fail("HINT_AXIS_VALUE_INVALID", f"{what} 는 양의 정수여야 한다: {value!r}")
    text = str(value).strip()
    if not re.fullmatch(r"[1-9]\d*", text):
        fail("HINT_AXIS_VALUE_INVALID", f"{what} 는 양의 정수여야 한다: {value!r}")
    return int(text)


def build_arch(hw: str, gpus_per_node: int, nodes: int, role: str, target: str, plane_token: str = "") -> str:
    """축 5개(+평면 토큰) → arch 세그먼트. 손으로 이어붙이는 자리를 없앤다(문법이 두 벌로 갈라지지 않게 · 2026-09-06).
    plane_token 은 vocab `plane` 의 값(docker = "" → 토큰 없음 · O-N1). 토큰 자체는 `plane_token()` 이 어휘에서 낸다."""
    if not isinstance(hw, str) or not _TOKEN_RE.match(hw):
        fail("HINT_ARCH_SHAPE_VIOLATION", f"hw 토큰은 [a-z0-9]+ 이어야 한다(vocab 정규화 결과): {hw!r}")
    if role not in NODE_AXIS:
        fail("HINT_ARCH_SHAPE_VIOLATION", f"노드 축은 {NODE_AXIS} 중 하나여야 한다: {role!r}")
    if not isinstance(plane_token, str) or (plane_token and not _TOKEN_RE.match(plane_token)):
        fail("HINT_ARCH_SHAPE_VIOLATION", f"평면 토큰은 빈 문자열 또는 [a-z0-9]+ 이어야 한다(vocab plane 값): {plane_token!r}")
    g = _pos_int(gpus_per_node, "gpus_per_node(G)")
    n = _pos_int(nodes, "nodes(N)")
    arch = f"{hw}-{g}g{n}n-{role}-{target}" + (f"-{plane_token}" if plane_token else "")
    why = arch_violation(arch)
    if why is not None:
        fail(why, f"조립한 arch 가 문법을 위반한다: {arch!r} — {_ARCH_MESSAGES.get(why, why)}")
    return arch


def build_recipe(q: str, length, kv: str, ple: str, spec, graph: str) -> str:
    """축 6개(토큰) → 레시피 세그먼트. spec 은 양의 정수 k(→ spec<k>) 또는 None/'off'(→ specoff)."""
    for what, tok in (("q", q), ("kv", kv), ("ple", ple)):
        if not isinstance(tok, str) or not _TOKEN_RE.match(tok):
            fail("HINT_RECIPE_SHAPE_VIOLATION", f"{what} 토큰은 [a-z0-9]+ 이어야 한다(vocab 정규화 결과): {tok!r}")
    if graph not in _GRAPH_TOKENS:
        fail("HINT_RECIPE_SHAPE_VIOLATION", f"graph 축은 graph|eager 둘 중 하나다: {graph!r}")
    n = _pos_int(length, "max-model-len(len)")
    k = "off" if spec is None or spec == "off" else str(_pos_int(spec, "speculative k(spec)"))
    recipe = f"q{q}-len{n}-kv{kv}-ple{ple}-spec{k}-{graph}"
    why = recipe_violation(recipe)
    if why is not None:
        fail(why, f"조립한 레시피가 문법을 위반한다: {recipe!r}")
    return recipe


def compose_from_payload(naming: dict) -> str:
    """PAYLOAD.naming(to_payload 형태) → 태그 이름 재조립. axes 로 arch·recipe 를 다시 만들어 segments 와 대조한다 —
    불일치 = HINT_NAMING_INCONSISTENT. tag.py verify 가 "이름 재파생 일치"를 볼 때 쓴다(봉인 시 재파생 대조 불변식 ·
    코드맵 §8.4)."""
    try:
        seg = {k: naming["segments"][k]["value"] for k in ("vllm", "model", "arch", "recipe")}
        ax = {k: naming["axes"][k]["value"] for k in ARCH_AXES + RECIPE_AXES}
    except (KeyError, TypeError) as e:
        fail("HINT_NAMING_INCONSISTENT", f"PAYLOAD.naming 에 segments/axes 칸이 없다: {e!r}")
    # 평면 축(O-N1 · 2026-09-23): 그 이전 페이로드에는 칸이 없다 — 그때는 native 를 발행할 수 없었으므로(HINT_PLANE_UNDERIVABLE)
    #   전부 Docker = 토큰 없음이다. 칸이 없는데 arch 에 평면 토큰이 있으면 아래 segments 대조가 잡는다(통과시키지 않는다).
    pl = naming["axes"].get(PLANE_AXIS)
    ptok = ""
    if pl is not None:
        if not isinstance(pl, dict) or pl.get("value") not in PLANE_NAMES or not isinstance(pl.get("token"), str):
            fail("HINT_NAMING_INCONSISTENT", f"PAYLOAD.naming.axes.plane 모양 위반(value∈{PLANE_NAMES} · token 문자열): {pl!r}")
        ptok = pl["token"]
        if (pl["value"] == _PLANE_NO_TOKEN) != (ptok == ""):
            fail("HINT_NAMING_INCONSISTENT", f"평면 {pl['value']!r} 와 토큰 {ptok!r} 가 모순된다(docker ⇔ 토큰 없음 · 문법 불변식)")
    arch = build_arch(ax["hw"], ax["gpus_per_node"], ax["nodes"], ax["role"], ax["target"], ptok)
    recipe = build_recipe(ax["q"], ax["len"], ax["kv"], ax["ple"], ax["spec"], ax["graph"])
    if arch != seg["arch"] or recipe != seg["recipe"]:
        fail("HINT_NAMING_INCONSISTENT",
             f"PAYLOAD.naming 의 축과 세그먼트가 어긋난다: arch {seg['arch']!r}≠{arch!r} 또는 recipe {seg['recipe']!r}≠{recipe!r}")
    tag = f"{core.HINT_TAG_PREFIX}{seg['vllm']}/{seg['model']}/{arch}/{recipe}"
    validate_new_name(tag)
    return tag


# ── 슬러그 ───────────────────────────────────────────────────────────────────────────────────
def derive_slug(hf_repo: str | None = None, model_path: str | None = None) -> str:
    """→ 모델 슬러그. **발행자가 이름을 짓지 못하게** 한다(2026-08-20 · 사용자 D2).

    - hf_repo(`<org>/<name>`) → 마지막 성분 소문자(옛 규칙 그대로).
    - model_path → **체크포인트 basename 소문자** = `sweep_bench.sh` 의 인증서 model 키 규칙(2026-08-15
      IDENTITY_MISMATCH:model). 두 규칙은 [A-Za-z0-9._-] basename 에서 옛 derive_slug 와 바이트 동일하다.
    - ★ 그 밖의 문자(공백·`+` 등)를 가진 basename 은 **차단**한다(HINT_MODEL_SLUG_UNSAFE). 옛 규칙은 그것을 `-` 로
      치환했지만, 인증서 model 키는 원문 소문자를 그대로 싣기 때문에 치환한 태그 슬러그가 identity.model 과
      **조용히 갈라진다** — 어느 쪽으로 맞춰도 한쪽이 틀리므로 파생하지 않는다.
    """
    if hf_repo:
        h = str(hf_repo).strip().rstrip("/")
        if "/" not in h:
            fail("HINT_MODEL_HF_REPO_SHAPE", f"hf_repo 는 '<org>/<name>' 형태여야 한다: {hf_repo!r}")
        slug = h.split("/")[-1].lower()
        src = f"hf_repo({h})"
    elif model_path:
        base = os.path.basename(str(model_path).rstrip("/")).strip()
        if not base:
            fail("HINT_AXIS_UNDERIVABLE", f"model_path 에서 이름을 뽑지 못했다: {model_path!r}",
                 f"필요한 증거: {_AXIS_EVIDENCE['model']}")
        slug = base.lower()
        src = f"model_path basename({base})"
    else:
        fail("HINT_AXIS_UNDERIVABLE", "hf_repo(정본) 또는 model_path(체크포인트 경로) 중 하나가 필요하다 — "
                                      "슬러그는 발행자가 짓지 않는다(plan_26082008 R1).",
             f"필요한 증거: {_AXIS_EVIDENCE['model']}")
    if not _MODEL_SLUG_RE.match(slug):
        fail("HINT_MODEL_SLUG_UNSAFE",
             f"{src} → {slug!r} 는 태그 슬러그 문자([a-z0-9._-], 첫 글자 영숫자) 밖이다. 치환하면 인증서 model 키"
             "(체크포인트 basename 소문자)와 갈라진다.",
             "체크포인트 디렉터리 이름을 [A-Za-z0-9._-] 로 바꿔 다시 서빙·측정한다(인증서 키와 태그 슬러그가 같은 원문에서 나와야 한다).")
    return slug


def base_slug(slug: str, vocab: dict) -> str:
    """끝의 양자화 접미(vocab `quant_suffixes`)를 떼어 기반 모델 슬러그를 낸다 — 계보 필터·match 용.
    예 `qwen3.8-flash-next-nvfp4` → `qwen3.8-flash-next` · `hy3-nvfp4-w4a16` → `hy3`(반복) ·
    접미뿐인 슬러그는 비우지 않는다.

    어휘표에 `quant_suffixes` 목록이 없으면 **차단**한다(HINT_VOCAB_ABSENT) — 조용히 아무것도 떼지 않으면 계보 필터·match
    의 기반 슬러그가 양자화 변종 이름으로 남아 같은 모델의 계보를 놓친다(침묵 폴백 · 2026-09-22 감사)."""
    sfx = vocab.get("quant_suffixes") if isinstance(vocab, dict) else None
    if not isinstance(sfx, list):
        fail("HINT_VOCAB_ABSENT", "어휘표에 quant_suffixes 목록이 없다 — 기반 슬러그를 파생할 수 없다",
             "load_vocab 으로 적재한 어휘표를 넘긴다.")
    suffixes = {str(s).lower() for s in sfx}
    s = str(slug)
    while True:
        cut = max(s.rfind("-"), s.rfind("_"))
        if cut <= 0 or s[cut + 1:].lower() not in suffixes:
            return s
        s = s[:cut]


def norm_slug(s: str) -> str:
    """슬러그 비교용 정규화(수신자 match·카탈로그의 단일 정규화기). 2026-08-20 실측: 대소문자·구두점만 다른 동일
    모델이 이미 2건 갈라져 있었고 match 가 정확일치 태그를 '다른 모델'로 판정했다 — 비교는 반드시 정규화 후.
    None 은 빈 키다 — `str(None)` 의 "none" 으로 접으면 값 없는 두 항목이 "같은 모델"이 된다(2026-09-22 감사)."""
    return "" if s is None else re.sub(r"[^a-z0-9]", "", str(s).lower())


def version_key(v: str) -> tuple:
    """vLLM 세그먼트 정렬 키(문법 인지). 릴리스 < 그 릴리스 뒤 커밋(`-g<sha12>`) < 다음 릴리스, dev<a<b<rc<정식<post.
    옛 `_vkey` 는 비숫자를 0 으로 만들어 `0.1.1.dev53+g…`·`<rel>-g<sha>` 를 엉뚱하게 줄 세웠다(코드맵 §1.9).
    해석 불가 문자열은 맨 앞에 원문 순으로 모은다."""
    rank = {"dev": 0, "a": 1, "b": 2, "rc": 3, None: 4, "post": 5}
    # D-g(2026-09-22): 4번째 마디(`M.m.p.q`)와 `.postN` 을 문법대로 줄 세운다(옛 판은 해석 불가로 맨 앞에 모았다).
    m = re.match(r"^v?(\d+)\.(\d+)\.(\d+)(?:\.(\d+))?(?:\.?(dev|a|b|rc|post)(\d+))?(?:\.post(\d+))?"
                 r"(?:-g([0-9a-f]{12})|\+g([0-9a-f]+))?$", str(v))
    if not m:
        return (-1, -1, -1, -1, -1, -1, -1, 0, str(v))
    stage = m.group(5)
    return (int(m.group(1)), int(m.group(2)), int(m.group(3)), int(m.group(4) or 0), rank.get(stage, 4),
            int(m.group(6)) if m.group(6) else 0, int(m.group(7)) if m.group(7) else -1,
            1 if (m.group(8) or m.group(9)) else 0, str(v))


# ── 어휘 ─────────────────────────────────────────────────────────────────────────────────────
def _vnorm(text: str) -> str:
    """어휘 매칭 키: 대소문자·공백만 정규화(정확 일치 · 부분 일치 ✗ — 에디션 구분이 목적)."""
    return " ".join(str(text).split()).casefold()


def validate_vocab(vocab) -> list[str]:
    """vocab 구조 문제 목록(빈 목록 = 정상). load_vocab 이 fail-closed 로 쓰고, 자체검사가 추적 파일에 쓴다."""
    bad: list[str] = []
    if not isinstance(vocab, dict):
        return ["최상위가 객체가 아니다"]
    if vocab.get("schema_version") != 1:
        bad.append(f"schema_version 이 1 이 아니다: {vocab.get('schema_version')!r}")
    known = set(VOCAB_AXES) | {"schema_version", "quant_suffixes", "hw_ambiguous", PLANE_AXIS}
    for k in vocab:
        if not str(k).startswith("_") and k not in known:
            bad.append(f"알 수 없는 키(오타?): {k!r}")
    for axis in VOCAB_AXES:
        table = vocab.get(axis)
        if not isinstance(table, dict) or not table:
            bad.append(f"{axis}: 토큰 사전이 없다")
            continue
        seen: dict[str, str] = {}
        for tok, aliases in table.items():
            if not isinstance(tok, str) or not _TOKEN_RE.match(tok):
                bad.append(f"{axis}: 토큰 {tok!r} 가 [a-z0-9]+ 가 아니다")
            if not isinstance(aliases, list) or not aliases:
                bad.append(f"{axis}.{tok}: 원문 목록이 비었다")
                continue
            for a in aliases:
                if not isinstance(a, str) or not a.strip():
                    bad.append(f"{axis}.{tok}: 빈 원문")
                    continue
                key = _vnorm(a)
                if _is_unobserved(a):
                    bad.append(f"{axis}.{tok}: {a!r} 는 '읽지 못함' 표기(또는 템플릿 빈칸)라 값으로 등재할 수 없다")
                if key in seen and seen[key] != tok:
                    bad.append(f"{axis}: 원문 {a!r} 가 두 토큰({seen[key]}·{tok})에 걸린다")
                seen[key] = tok
    if isinstance(vocab.get("graph"), dict) and set(vocab["graph"]) != _GRAPH_TOKENS:
        bad.append(f"graph: 토큰은 문법이 고정한 {sorted(_GRAPH_TOKENS)} 여야 한다")
    if isinstance(vocab.get("ple"), dict) and "none" not in vocab["ple"]:
        bad.append("ple: PLE 없는 모델의 명시 토큰 'none'(plenone)이 없다")
    bad += _plane_vocab_problems(vocab.get(PLANE_AXIS))
    sfx = vocab.get("quant_suffixes")
    if not isinstance(sfx, list) or not all(isinstance(s, str) and _TOKEN_RE.match(s) for s in sfx):
        bad.append("quant_suffixes: [a-z0-9]+ 문자열 목록이어야 한다")
    amb = vocab.get("hw_ambiguous", {})
    if not isinstance(amb, dict):
        bad.append("hw_ambiguous: 객체여야 한다")
    elif isinstance(vocab.get("hw"), dict):
        hw_keys = {_vnorm(a) for al in vocab["hw"].values() if isinstance(al, list) for a in al if isinstance(a, str)}
        for a in amb:
            if _vnorm(a) in hw_keys:
                bad.append(f"hw_ambiguous: {a!r} 가 hw 토큰에도 등재돼 있다(에디션 구분이 무너진다)")
    return bad


def _plane_vocab_problems(table) -> list[str]:
    """vocab `plane`(평면 이름 → arch 토큰 · O-N1) 구조 문제. 키 = PLANE_NAMES 정확히 · docker = "" · native = [a-z0-9]+ · 토큰 중복 ✗."""
    if not isinstance(table, dict):
        return [f"{PLANE_AXIS}: 평면→토큰 사전이 없다(O-N1 · docker=\"\" · native=<토큰>)"]
    bad: list[str] = []
    entries = {k: v for k, v in table.items() if not str(k).startswith("_")}
    if set(entries) != set(PLANE_NAMES):
        bad.append(f"{PLANE_AXIS}: 키는 판정 소유자(artifacts._plane)의 닫힌 어휘 {list(PLANE_NAMES)} 여야 한다: {sorted(entries)}")
    for k, v in entries.items():
        if not isinstance(v, str) or (v and not _TOKEN_RE.match(v)):
            bad.append(f"{PLANE_AXIS}.{k}: 토큰은 빈 문자열 또는 [a-z0-9]+ 이어야 한다: {v!r}")
    if entries.get(_PLANE_NO_TOKEN, "") != "":
        bad.append(f"{PLANE_AXIS}.{_PLANE_NO_TOKEN}: 빈 문자열이어야 한다(Docker 이름 불변 — 옛·신 태그가 바이트 그대로 파생돼야 한다)")
    toks = [v for k, v in entries.items() if k != _PLANE_NO_TOKEN]
    if any(not t for t in toks):
        bad.append(f"{PLANE_AXIS}: docker 밖 평면의 토큰이 비었다 — 축이 같은 Docker 셀과 이름이 충돌한다(2026-09-23 F10)")
    if len(set(toks)) != len(toks):
        bad.append(f"{PLANE_AXIS}: 두 평면이 같은 토큰을 쓴다")
    return bad


def plane_token(plane, vocab: dict) -> str:
    """평면 이름 → arch 평면 토큰(vocab `plane`). 어휘표에 plane 축 없음 = HINT_VOCAB_ABSENT · 어휘 밖 평면 = HINT_VOCAB_UNKNOWN."""
    table = (vocab or {}).get(PLANE_AXIS) if isinstance(vocab, dict) else None
    if not isinstance(table, dict):
        fail("HINT_VOCAB_ABSENT", f"어휘표에 {PLANE_AXIS} 축이 없다(O-N1 · 2026-09-23)",
             f"`{core.REL_VOCAB}` 에 \"{PLANE_AXIS}\": {{\"docker\": \"\", \"native\": \"<토큰>\"}} 를 둔다(사람 편집).")
    if not isinstance(plane, str) or plane.startswith("_") or plane not in table:
        fail("HINT_VOCAB_UNKNOWN", f"평면 {plane!r} 가 어휘표 {PLANE_AXIS} 밖이다(tripwire 닫힌 목록)",
             f"평면은 artifacts.plane_of 가 내는 {PLANE_NAMES} 중 하나다 — 새 평면이면 `{core.REL_VOCAB}` 의 \"{PLANE_AXIS}\" 에 "
             "토큰을 추가한다(사람 편집).")
    tok = table[plane]
    if not isinstance(tok, str) or (tok and not _TOKEN_RE.match(tok)) or ((plane == _PLANE_NO_TOKEN) != (tok == "")):
        fail("HINT_VOCAB_ABSENT", f"어휘표 {PLANE_AXIS}.{plane} 토큰이 깨졌다: {tok!r}", "load_vocab 으로 적재한 어휘표를 넘긴다.")
    return tok


def load_vocab(repo: Path) -> dict:
    """`hints/vocab.json`(추적 · 사람 편집). 부재·깨짐 = HINT_VOCAB_ABSENT(대체 어휘로 계속하지 않는다)."""
    path = Path(repo) / core.REL_VOCAB
    remedy = f"`{core.REL_VOCAB}` 을 복구한다(추적 파일 · `git checkout -- {core.REL_VOCAB}`)."
    if not path.is_file():
        fail("HINT_VOCAB_ABSENT", f"어휘표가 없다: {core.REL_VOCAB}", remedy)
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        fail("HINT_VOCAB_ABSENT", f"어휘표를 읽을 수 없다({core.REL_VOCAB}): {e}", remedy)
    problems = validate_vocab(doc)
    if problems:
        fail("HINT_VOCAB_ABSENT", f"어휘표가 깨졌다({core.REL_VOCAB}): " + " · ".join(problems[:6]),
             "tripwire 목록을 고친다(원문 하나는 토큰 하나에만 · 토큰은 [a-z0-9]+).")
    return doc


def normalize(axis: str, raw, vocab: dict) -> str:
    """원문 → 이름 토큰(hw/quant/kv/ple/graph). 어휘 밖 = HINT_VOCAB_UNKNOWN(remedy 에 추가할 키를 적는다)."""
    if axis not in VOCAB_AXES:
        fail("HINT_VOCAB_UNKNOWN", f"어휘 축이 아니다: {axis!r} (허용 {VOCAB_AXES})")
    where = f"`{core.REL_VOCAB}` 의 \"{axis}\""
    if not isinstance(raw, str) or not raw.strip():
        fail("HINT_VOCAB_UNKNOWN", f"{axis} 원문이 문자열이 아니다: {raw!r}",
             f"evidence 가 {axis} 축을 문자열로 넘긴다(예 graph 는 enforce-eager 관측을 eager|graph 로 변환).")
    if _is_unobserved(raw):
        # 어휘 밖이긴 하지만 "추가하라"가 처방이 아니다 — validate_vocab 이 이 표기의 등재를 거부한다(읽지 못함 ≠ 값).
        fail("HINT_VOCAB_UNKNOWN", f"{axis} 원문 {raw!r} 는 '읽지 못함' 표기(또는 템플릿 빈칸)다 — 값이 아니다",
             f"어휘표에 추가하지 않는다. {axis} 값을 실제로 관측한 증거를 넘긴다(derive_name 은 이 경우 HINT_AXIS_UNDERIVABLE).")
    table = (vocab or {}).get(axis)
    if not isinstance(table, dict):
        fail("HINT_VOCAB_ABSENT", f"어휘표에 {axis} 축이 없다", "load_vocab 으로 적재한 어휘표를 넘긴다.")
    key = _vnorm(raw)
    hits = {tok for tok, aliases in table.items() if isinstance(aliases, list)
            for a in aliases if isinstance(a, str) and _vnorm(a) == key}
    if len(hits) == 1:
        return next(iter(hits))
    if len(hits) > 1:
        fail("HINT_VOCAB_ABSENT", f"{axis} 원문 {raw!r} 가 여러 토큰 {sorted(hits)} 에 걸린다(어휘표 깨짐)",
             f"{where} 에서 원문 하나는 토큰 하나에만 둔다.")
    if axis == "hw":
        amb = {_vnorm(a): why for a, why in ((vocab or {}).get("hw_ambiguous") or {}).items()}
        if key in amb:
            fail("HINT_VOCAB_UNKNOWN", f"hw 원문 {raw!r} 는 에디션이 드러나지 않는다: {amb[key]}",
                 "에디션이 드러나는 원문을 증거로 넘긴다 — 이 원문을 어느 토큰에 합치지 않는다(합치면 에디션이 섞인다).")
    fail("HINT_VOCAB_UNKNOWN", f"{axis} 원문 {raw!r} 가 어휘표 밖이다(tripwire 닫힌 목록)",
         f"{where} 에서 이 값이 뜻하는 토큰의 목록에 \"{raw}\" 를 추가한다(사람 편집 · 새 토큰이면 [a-z0-9]+ 키를 만든다"
         + (" · hw 는 에디션이 구분되는 토큰에만" if axis == "hw" else "") + ").")


# ── 파생 ─────────────────────────────────────────────────────────────────────────────────────
def _underivable(axis: str, why: str, source: str | None = None):
    fail("HINT_AXIS_UNDERIVABLE", f"{axis} 축을 파생하지 못했다: {why} — 이름을 지어내지 않는다.",
         f"필요한 증거: {_AXIS_EVIDENCE.get(axis, axis)} (관측 기록: {source or '없음'}). "
         "`hint.py name --campaign <id> --cell <cell>` 로 축별 출처를 확인한다.")


def _facts_axis(facts: dict, axis: str) -> tuple[dict, str]:
    d = facts.get(axis) if isinstance(facts, dict) else None
    if not isinstance(d, dict):
        _underivable(axis, "naming facts 에 이 축이 없다")
    src = str(d.get("source") or "").strip()
    if not src:
        _underivable(axis, "출처(source)가 비었다 — 출처 없는 값은 이름이 되지 않는다")
    return d, src


def _raw(axis: str, d: dict, src: str, *, none_ok: bool = False):
    if "raw" not in d:
        _underivable(axis, "raw 키가 없다(읽지 못함 ≠ 없음)", src)
    raw = d["raw"]
    if raw is None:
        if none_ok:
            return None
        _underivable(axis, "raw 가 None 이다(관측되지 않은 값)", src)
    if _is_unobserved(raw):
        _underivable(axis, f"raw={raw!r} 는 '읽지 못함' 표기(또는 템플릿 빈칸)다(인증서·스윕 meta 의 N/A 는 '없음'이 "
                           "아니다 — 태그2 PAYLOAD.identity.quant='N/A' 였으나 체크포인트는 NVFP4 혼합이었다)", src)
    return raw


def _vocab_axis(axis_name: str, vocab_axis: str, facts: dict, vocab: dict) -> Axis:
    d, src = _facts_axis(facts, axis_name)
    raw = _raw(axis_name, d, src)
    tok = normalize(vocab_axis, raw, vocab)
    return Axis(tok, f"{src} · vocab {vocab_axis}[{tok}]←{raw!r}")


def _describe_release(text) -> str | None:
    m = _DESCRIBE_RE.match(str(text).strip()) if text else None
    return m["rel"] if m else None


def release_of(text) -> str | None:
    """릴리스 문법(X18 · D-g)이면 `v` 를 뗀 릴리스 문자열, 아니면 None. evidence 가 해소값 바인딩·계보 토큰에 쓴다 —
    릴리스 문법을 두 모듈에 따로 적지 않는다(한쪽만 넓어지면 이름과 바인딩이 갈라진다 · 2026-09-22 D-g)."""
    m = _RELEASE_RE.match(str(text or "").strip())
    return m["rel"] if m else None


def vllm_repo_kind(url) -> str | None:
    """VLLM_REPO → `upstream` · `fork` · None(미관측). 순수 함수(네트워크·git ✗ · D-h).

    정규화 = 스킴·사용자정보·scp형 `git@host:` · 끝 `.git`·`/` 제거 후 `host/owner/repo` 소문자. `UPSTREAM_VLLM_REPOS` 에
    없으면 전부 `fork` 다(로컬 경로·사내 미러 포함 — 업스트림 태그라는 증거가 없다). '읽지 못함' 표기 = None."""
    if url is None:
        return None
    s = str(url).strip()
    if not s or _is_unobserved(s):
        return None
    m = _REPO_URL_RE.match(s)
    if not m:
        return "fork"
    key = f"{m['host'].lower()}/{m['path'].strip('/').lower()}"
    return "upstream" if key in UPSTREAM_VLLM_REPOS else "fork"


def _resolve_vllm(d: dict, src: str) -> tuple[Axis, dict]:
    """X18: vLLM 세그먼트 = **빌드 입력**. 엔진 자기보고·wheel 메타 원문은 00 사실 블록에만 간다(여기 입력 ✗)."""
    track = d.get("track")
    if track not in ("source-build", "wheel", "native"):
        _underivable("vllm", f"빌드 트랙이 source-build|wheel|native 가 아니다: {track!r}", src)
    ref = str(d.get("vllm_ref") or "").strip() or None
    version = str(d.get("vllm_version") or "").strip() or None
    sha = str(d.get("vllm_sha") or "").strip().lower() or None
    if sha is not None and not _SHA40_RE.match(sha):
        _underivable("vllm", f"vllm_sha 가 40자 SHA 가 아니다: {sha!r}", src)
    url = str(d.get("wheel_url") or "").strip() or None
    url_sha = url_file = None
    if url:
        path = urlparse(url).path
        shas = [p for p in path.split("/") if _SHA40_RE.match(p.lower())]
        if len(shas) > 1 and len(set(s.lower() for s in shas)) > 1:
            _underivable("vllm", "wheel URL 에 40자 SHA 가 둘 이상이다(모호)", src)
        url_sha = shas[0].lower() if shas else None
        url_file = unquote(path.rsplit("/", 1)[-1]) or None

    def conflict(a: str, b: str, what: str):
        fail("HINT_VLLM_INPUT_CONFLICT", f"vLLM 빌드 입력이 서로 모순된다({what}: {a} ≠ {b})",
             "빌드 원장·셀 env 중 어느 쪽이 실제로 빌드된 입력인지 증거를 바로잡는다(이름은 한쪽을 고르지 않는다).")

    def commit(pin_sha: str, ref_text: str, why: str) -> tuple[Axis, dict]:
        prev = _describe_release(d.get("prev_release"))
        if prev is None:
            _underivable("vllm", f"커밋 핀({pin_sha[:12]})인데 직전 릴리스를 해소하지 못했다"
                                 f"(prev_release={d.get('prev_release')!r})", src)
        # describe 긴 형태(`v0.29.0rc5-37-g0123456`)는 **그것을 계산한 커밋**의 약식 SHA 를 들고 있다. 핀 SHA 와 접두가
        # 다르면 다른 커밋(빌드 컨텍스트의 HEAD 등)의 describe 다 — 그대로 쓰면 이름의 `<직전 릴리스>` 가 조용히
        # 틀린다(2026-09-22 감사 · 원장 `vllm.describe` = `git describe --tags --match 'v*'` · render_dockerfile).
        dm = _DESCRIBE_RE.match(str(d.get("prev_release")).strip())
        if dm and dm["g"] and not pin_sha.startswith(dm["g"]):
            conflict(dm["g"], pin_sha[:12], "prev_release describe 의 커밋 vs 핀 SHA")
        seg = f"{prev}-g{pin_sha[:12]}"
        return (Axis(seg, f"{src} · {why} → <직전 릴리스 {prev}>-g<sha12>"),
                {"kind": "commit", "ref": ref_text, "sha": pin_sha, "prev_release": prev})

    if track == "source-build":
        if ref is None:
            _underivable("vllm", "소스빌드인데 VLLM_REF 가 없다", src)
        m = _RELEASE_RE.match(ref)
        if m:
            # D-h(2026-09-22): 릴리스 모양 ref 가 릴리스 **이름**이 되려면 저장소가 업스트림이어야 한다. 저장소를 모르면
            #   (키 부재 · 미관측) 판정하지 않는다 — 포크 빌드가 업스트림 릴리스 이름을 쓰는 오도를 막는 자리다.
            if "vllm_repo" not in d:
                _underivable("vllm", f"naming facts 에 vllm_repo 키가 없다 — 릴리스 모양 VLLM_REF={ref} 가 업스트림 태그인지 "
                                     "판정할 수 없다(D-h · evidence.naming_facts 가 싣는다)", src)
            rk = vllm_repo_kind(d.get("vllm_repo"))
            if rk is None:
                _underivable("vllm", f"VLLM_REPO 미관측 — 릴리스 모양 VLLM_REF={ref} 가 업스트림 태그인지 판정할 수 없다"
                                     "(D-h · 이미지 history build-arg 또는 셀 env 의 VLLM_REPO)", src)
            if rk == "upstream":
                return (Axis(m["rel"], f"{src} · VLLM_REF={ref}(업스트림 릴리스 태그 · v 제거)"),
                        {"kind": "release", "ref": ref, "sha": sha, "prev_release": None})
            if sha is None:
                _underivable("vllm", f"포크 저장소(비-업스트림 VLLM_REPO)의 릴리스 모양 VLLM_REF={ref} — 이름은 SHA 경로"
                                     "(<직전 릴리스>-g<sha12>)만 허용하는데 빌드된 vllm_sha 가 없다(D-h · 빌드 원장 vllm.git_sha)", src)
            return commit(sha, ref, f"포크 VLLM_REPO 의 릴리스 모양 VLLM_REF={ref} → SHA 경로(D-h · 업스트림 릴리스 이름 ✗)")
        if _SHA40_RE.match(ref.lower()):
            if sha is not None and sha != ref.lower():
                conflict(ref.lower(), sha, "VLLM_REF 커밋 vs vllm_sha")
            return commit(ref.lower(), ref, f"VLLM_REF={ref[:12]}…(40자 커밋)")
        if sha is not None:      # 브랜치·짧은 SHA 는 움직이는 포인터 — 실제 빌드된 SHA 로 이름 짓는다
            if re.fullmatch(r"[0-9a-f]{7,39}", ref.lower()) and not sha.startswith(ref.lower()):
                conflict(ref, sha, "짧은 VLLM_REF vs vllm_sha")
            return commit(sha, ref, f"VLLM_REF={ref}(릴리스·40자 SHA 아님) · 빌드된 vllm_sha")
        _underivable("vllm", f"VLLM_REF={ref!r} 는 릴리스 태그도 40자 SHA 도 아니고 빌드된 vllm_sha 도 없다", src)

    if track == "wheel":
        if version and (m := _RELEASE_RE.match(version)):
            return (Axis(m["rel"], f"{src} · VLLM_VERSION={version}(wheel 릴리스)"),
                    {"kind": "wheel", "ref": version, "sha": sha, "prev_release": None})
        pin = url_sha or sha
        if url_sha and sha and url_sha != sha:
            conflict(url_sha, sha, "wheel URL SHA vs vllm_sha")
        if pin:
            return commit(pin, version or url_file or pin, f"wheel VLLM_VERSION={version!r}(릴리스 아님) · 커밋 핀")
        _underivable("vllm", f"wheel 트랙인데 VLLM_VERSION={version!r} 가 릴리스가 아니고 커밋 SHA 도 없다", src)

    # native — wheel URL 의 SHA 도 같은 규칙(X18)
    if url_sha and sha and url_sha != sha:
        conflict(url_sha, sha, "native wheel URL SHA vs vllm_sha")
    if url_sha:
        return commit(url_sha, url_file or url_sha, f"native wheel URL({url_file}) 의 커밋 SHA")
    file_ver = None
    if url_file and (fm := re.match(r"^vllm-([^-]+)-", url_file)):
        file_ver = fm.group(1)
    for cand, why in ((file_ver, f"native wheel 파일명({url_file})"), (version, "native vllm_version")):
        if cand and (m := _RELEASE_RE.match(cand)):
            return (Axis(m["rel"], f"{src} · {why} 릴리스 {m['rel']}"),
                    {"kind": "wheel", "ref": url_file or cand, "sha": sha, "prev_release": None})
    if sha:
        return commit(sha, url_file or version or sha, "native 빌드된 vllm_sha")
    _underivable("vllm", "native 인데 wheel URL SHA·릴리스 버전·vllm_sha 중 어느 것도 없다", src)


def vllm_segment(build_input: dict) -> Axis:
    """X18 — naming facts 의 `vllm` 사전 → vLLM 세그먼트 Axis. 파생 불가 = HINT_AXIS_UNDERIVABLE."""
    d, src = _facts_axis({"vllm": build_input}, "vllm")
    return _resolve_vllm(d, src)[0]


def derive_name(facts: dict, vocab: dict) -> DerivedName:
    """naming facts(SPEC §3.3 · evidence 가 조립) → 태그 이름 + 축별 {값, 출처}. **발행자 입력 ✗(D8).**"""
    if not isinstance(facts, dict):
        _underivable("facts", "naming facts 가 사전이 아니다")
    vd, vsrc = _facts_axis(facts, "vllm")
    vllm_axis, build_input = _resolve_vllm(vd, vsrc)

    md, msrc = _facts_axis(facts, "model")
    hf_repo = str(md.get("hf_repo") or "").strip() or None
    model_path = str(md.get("model_path") or "").strip() or None
    slug = derive_slug(hf_repo, model_path)
    if hf_repo and model_path:
        by_path = derive_slug(None, model_path)
        if by_path != slug:
            # 인증서 model 키는 체크포인트 basename 소문자다. HF repo 이름이 그와 다르면 태그 슬러그가 identity.model 과
            # 갈라진다 — base_model(양자화 전 원본)을 hf_repo 로 넘긴 실수도 여기서 드러난다.
            fail("HINT_MODEL_SLUG_CONFLICT",
                 f"hf_repo 슬러그 {slug!r} ≠ 체크포인트 basename 슬러그 {by_path!r}",
                 "hf_repo 는 **서빙한 체크포인트**의 repo id 여야 한다(base_model 이 아니다). 확인할 수 없으면 hf_repo 를 비운다.")
    model_axis = Axis(slug, f"{msrc} · " + (f"hf_repo({hf_repo}) 마지막 성분 소문자" if hf_repo else
                                          f"model_path basename({os.path.basename(model_path.rstrip('/'))}) 소문자"
                                          " = 인증서 model 키 규칙"))

    ad, asrc = _facts_axis(facts, "arch")
    gpu_model = ad.get("gpu_model")
    if not isinstance(gpu_model, str) or _is_unobserved(gpu_model):
        _underivable("arch", f"gpu_model 을 관측하지 못했다({gpu_model!r})", asrc)
    hw = normalize("hw", gpu_model, vocab)
    for k in ("gpus_per_node", "nodes", "role"):
        if ad.get(k) is None:
            _underivable("arch", f"{k} 가 없다", asrc)
    g = _pos_int(ad["gpus_per_node"], "gpus_per_node(G)")
    n = _pos_int(ad["nodes"], "nodes(N)")
    role = ad["role"]
    if "target_gpu" not in ad:
        _underivable("arch", "target_gpu 키가 없다(시뮬레이션 타겟 선언 여부를 관측하지 못했다 — 읽지 못함 ≠ 없음)", asrc)
    tgt_raw = ad["target_gpu"]
    if _is_unobserved(tgt_raw):
        # 'N/A'·빈칸은 "타겟 없음(native)"도 어휘 밖 hw 도 아니다 — 선언을 읽지 못한 것이다(None 만 '없음 관측').
        _underivable("arch", f"target_gpu={tgt_raw!r} 는 '읽지 못함' 표기(또는 템플릿 빈칸)다 — 타겟 없음은 None 으로만 "
                             "넘긴다", asrc)
    if tgt_raw is None:
        target, tsrc = "native", f"{asrc} · no-simulation-target-declared → native"
    else:
        ttok = normalize("hw", tgt_raw, vocab)
        if ttok == hw:
            target, tsrc = "native", f"{asrc} · target_gpu {tgt_raw!r} = 호스트 hw → native"
        else:
            target, tsrc = f"sim-{ttok}", f"{asrc} · vocab hw[{ttok}]←{tgt_raw!r} → sim-{ttok}"
    pd, psrc = _facts_axis(facts, PLANE_AXIS)
    plane = _raw(PLANE_AXIS, pd, psrc)
    ptok = plane_token(plane, vocab)
    arch = build_arch(hw, g, n, role, target, ptok)
    axes = {
        "hw": Axis(hw, f"{asrc} · vocab hw[{hw}]←{gpu_model!r}"),
        "gpus_per_node": Axis(str(g), asrc),
        "nodes": Axis(str(n), asrc),
        "role": Axis(role, asrc),
        "target": Axis(target, tsrc),
        PLANE_AXIS: Axis(plane, f"{psrc} · vocab {PLANE_AXIS}[{plane}]→{ptok!r}", token=ptok),
    }

    axes["q"] = _vocab_axis("q", "quant", facts, vocab)
    ld, lsrc = _facts_axis(facts, "len")
    axes["len"] = Axis(str(_pos_int(_raw("len", ld, lsrc), "max-model-len(len)")), lsrc)
    axes["kv"] = _vocab_axis("kv", "kv", facts, vocab)
    axes["ple"] = _vocab_axis("ple", "ple", facts, vocab)
    sd, ssrc = _facts_axis(facts, "spec")
    spec_raw = _raw("spec", sd, ssrc, none_ok=True)
    if spec_raw is None or (isinstance(spec_raw, str) and spec_raw.strip().lower() == "off"):
        axes["spec"] = Axis("off", f"{ssrc} · speculative-config 부재 관측 → specoff")
    else:
        axes["spec"] = Axis(str(_pos_int(spec_raw, "speculative k(spec)")), ssrc)
    axes["graph"] = _vocab_axis("graph", "graph", facts, vocab)
    recipe = build_recipe(axes["q"].value, axes["len"].value, axes["kv"].value, axes["ple"].value,
                          axes["spec"].value, axes["graph"].value)

    tag = f"{core.HINT_TAG_PREFIX}{vllm_axis.value}/{slug}/{arch}/{recipe}"
    validate_new_name(tag)   # 파생기가 스스로 문법을 검사한다(조립과 판정이 두 벌로 갈라지지 않게)
    segments = {"vllm": vllm_axis, "model": model_axis,
                "arch": Axis(arch, "derived(hw·gpus_per_node·nodes·role·target·plane)"),
                "recipe": Axis(recipe, "derived(q·len·kv·ple·spec·graph)")}
    return DerivedName(tag, segments, axes, build_input)


# ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
_IMPORT_PROBE = r"""
import json, sys
events = []
_WATCH = ("subprocess.Popen", "os.system", "os.exec", "os.posix_spawn", "os.spawn", "os.fork", "os.forkpty")
def _hook(ev, args):
    if ev in _WATCH:
        events.append([ev, repr(args)[:160]])
    elif ev == "open":
        p = args[0]
        if isinstance(p, bytes):
            p = p.decode("utf-8", "replace")
        if isinstance(p, str) and not p.endswith((".py", ".pyc")):
            events.append([ev, p])
sys.addaudithook(_hook)
sys.path.insert(0, sys.argv[1])
import hintlib.naming
sys.stdout.write(json.dumps(events))
"""


def _code(fn, *a, **kw) -> str | None:
    try:
        fn(*a, **kw)
    except HintError as e:
        return e.code
    return None


def _fixture_vocab() -> dict:
    return {"schema_version": 1,
            "hw": {"gb10": ["NVIDIA GB10", "GB10"], "h100": ["NVIDIA H100 80GB HBM3", "NVIDIA H100", "H100"],
                   "rtxpro6000maxq": ["NVIDIA RTX PRO 6000 Blackwell Max-Q Workstation Edition"],
                   "rtxpro6000srv": ["NVIDIA RTX PRO 6000 Blackwell Server Edition"]},
            "hw_ambiguous": {"NVIDIA RTX PRO 6000": "에디션 미표기"},
            "quant": {"nvfp4": ["nvfp4", "modelopt:NVFP4", "modelopt-dominant:NVFP4"], "fp8": ["fp8"],
                      "mxfp4": ["mxfp4"], "bf16": ["bf16", "bfloat16", "none"]},
            "kv": {"auto": ["auto"], "fp8": ["fp8", "fp8_e4m3", "fp8e4m3"], "fp8e5m2": ["fp8_e5m2"]},
            "ple": {"mmap": ["mmap"], "resident": ["resident"], "offload": ["offload"], "none": ["none"]},
            "graph": {"graph": ["graph", "cudagraph"], "eager": ["eager"]},
            "plane": {"docker": "", "native": "bare"},
            "quant_suffixes": ["nvfp4", "fp8", "mxfp4", "int4", "awq", "gptq", "w4a16", "bf16"]}


_TAG2_SHA = "74c96922ecb9017f413318c76d1af83aa2ab45a5"   # 태그2 셀 nv4-bf-262k-mmp 의 빌드 SHA(코드맵 campaign_plane §8)
_TAG2_EXPECTED = ("hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/"
                  "qnvfp4-len262144-kvauto-plemmap-spec3-eager")   # AC4 · plan §4.7


def _tag2_facts() -> dict:
    return {"vllm": {"track": "source-build", "vllm_ref": "v0.29.0rc6", "vllm_version": None, "vllm_sha": _TAG2_SHA,
                     "vllm_repo": "https://" + UPSTREAM_VLLM_REPOS[0] + ".git",
                     "prev_release": None, "wheel_url": None, "source": "셀 env VLLM_REF(output/multi/envs/.env.<cell>)"},
            "model": {"hf_repo": None, "model_path": "/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4",
                      "source": "서빙 yaml model"},
            "arch": {"gpu_model": "NVIDIA GB10", "gpus_per_node": 1, "nodes": 2, "role": "cluster",
                     "target_gpu": None, "source": "output/multi/manifest.yaml · sweep meta measured_node=cluster"},
            "plane": {"raw": "docker", "source": "artifacts.plane_of → cell-env(IMAGE_TAG|BUILD_DOCKERFILE)"},
            "q": {"raw": "modelopt-dominant:NVFP4", "source": "hf_quant_config.json quantized_layers 우세 알고리즘"},
            "len": {"raw": 262144, "source": "서빙 yaml max-model-len"},
            "kv": {"raw": "auto", "source": "서빙 yaml kv-cache-dtype"},
            "ple": {"raw": "mmap", "source": "셀 env VLLM_PLE_MMAP=1"},
            "spec": {"raw": 3, "source": "서빙 yaml speculative-config num_speculative_tokens"},
            "graph": {"raw": "eager", "source": "서빙 yaml enforce-eager: true"}}


def selftest() -> list[str]:
    """실패 메시지 목록(빈 목록 = 통과). 라이브 태그·브랜치·캠페인에 의존하지 않는다(픽스처 · 임시 디렉터리).
    추적 파일 두 개(`hints/vocab.json` · producer `sweep_bench.sh`)는 이 체크아웃의 배포물로서 읽는다."""
    bad: list[str] = []

    def ck(name: str, cond: bool) -> None:
        if not cond:
            bad.append(f"naming: {name}")

    V = _fixture_vocab()
    ck("픽스처 어휘표가 구조 검사를 통과한다", validate_vocab(V) == [])

    # ── 태그2 셀 기대값(AC4) ──
    dn = derive_name(_tag2_facts(), V)
    ck(f"태그2 facts → 정확히 기대 이름(실제 {dn.tag})", dn.tag == _TAG2_EXPECTED)
    ck("축별 {값, 출처} 가 전부 채워진다",
       set(dn.axes) == set(ARCH_AXES + (PLANE_AXIS,) + RECIPE_AXES) and all(a.value and a.source for a in dn.axes.values())
       and set(dn.segments) == {"vllm", "model", "arch", "recipe"})
    ck("q 출처에 어휘 정규화 흔적(modelopt-dominant:NVFP4 → nvfp4)",
       dn.axes["q"].value == "nvfp4" and "modelopt-dominant:NVFP4" in dn.axes["q"].source)
    ck("target None → native · 출처에 no-simulation-target-declared",
       dn.axes["target"].value == "native" and "no-simulation-target-declared" in dn.axes["target"].source)
    ck("릴리스 빌드 입력 기록(kind=release · sha 보존)",
       dn.vllm_build_input == {"kind": "release", "ref": "v0.29.0rc6", "sha": _TAG2_SHA, "prev_release": None})
    ck("★출처에 체크포인트 절대경로를 싣지 않는다(basename 만)",
       "/app/quant_models" not in json.dumps(dn.to_payload(), ensure_ascii=False))
    ck("파생 이름은 v6 문법으로 해체된다", parse_tag(dn.tag).grammar == GRAMMAR_V6)
    pay = dn.to_payload()
    ck("PAYLOAD.naming 재조립 = 태그", compose_from_payload(pay) == dn.tag)
    tam = json.loads(json.dumps(pay))
    tam["axes"]["len"]["value"] = "131072"
    ck("★음성대조 PAYLOAD 축 위조는 재조립 대조가 잡는다", _code(compose_from_payload, tam) == "HINT_NAMING_INCONSISTENT")

    # ── 평면 토큰(O-N1 · 2026-09-23 plan_26092311) — Docker 불변 · native 만 `-<vocab plane.native>` ──
    ntok = V["plane"]["native"]
    ck("docker 평면 = 토큰 없음(태그2 이름 바이트 불변) · 축 기록",
       dn.axes[PLANE_AXIS].value == "docker" and dn.axes[PLANE_AXIS].token == ""
       and "/gb10-1g2n-cluster-native/" in dn.tag and pay["axes"][PLANE_AXIS]["token"] == "")
    fn = _tag2_facts()
    fn["plane"] = {"raw": "native", "source": "artifacts.plane_of → evidence.plane(declared) · serve_proof plane=native"}
    dnn = derive_name(fn, V)
    ck(f"native 평면 → arch 끝 -{ntok}({dnn.tag})",
       dnn.tag == _TAG2_EXPECTED.replace("/gb10-1g2n-cluster-native/", f"/gb10-1g2n-cluster-native-{ntok}/"))
    ck("native 는 축이 같은 Docker 이름과 충돌하지 않는다(F10)", dnn.tag != dn.tag)
    ck("평면 출처가 PAYLOAD.naming 에 실린다(serve_proof · vocab 대응)",
       "serve_proof plane=native" in dnn.axes[PLANE_AXIS].source and f"→{ntok!r}" in dnn.axes[PLANE_AXIS].source)
    ck("native PAYLOAD.naming 재조립 = 태그", compose_from_payload(dnn.to_payload()) == dnn.tag)
    ck("파서 왕복: -<plane> 있음 → v6 · plane=native", parse_tag(dnn.tag).grammar == GRAMMAR_V6
       and parse_arch(parse_tag(dnn.tag).arch, V) == {"grammar": GRAMMAR_V6, "hw": "gb10", "gpus_per_node": 1, "nodes": 2,
                                                       "role": "cluster", "target": "native", "plane_token": ntok,
                                                       "plane": "native"})
    ck("파서 왕복: 토큰 없음 → plane=docker(vocab 없이도)", parse_arch("gb10-1g2n-cluster-native")["plane"] == "docker"
       and parse_arch("gb10-1g2n-cluster-native")["plane_token"] == "")
    ck("sim 타겟 뒤 평면 토큰도 해체된다", (parse_arch(f"gb10-1g1n-sub-sim-h100-{ntok}", V) or {}).get("target") == "sim-h100"
       and (parse_arch(f"gb10-1g1n-sub-sim-h100-{ntok}", V) or {}).get("plane") == "native")
    ck("vocab 없이 비어 있지 않은 토큰은 추측하지 않는다(plane=None)", parse_arch(f"gb10-1g2n-cluster-native-{ntok}")["plane"] is None)
    old_pay = json.loads(json.dumps(pay))
    del old_pay["axes"][PLANE_AXIS]
    ck("O-N1 이전 페이로드(평면 칸 없음 · Docker)도 재조립된다", compose_from_payload(old_pay) == dn.tag)
    old_nat = json.loads(json.dumps(dnn.to_payload()))
    del old_nat["axes"][PLANE_AXIS]
    ck("★음성대조 평면 칸 없이 -<plane> 이름 = 재조립 불일치", _code(compose_from_payload, old_nat) == "HINT_NAMING_INCONSISTENT")
    lie = json.loads(json.dumps(dnn.to_payload()))
    lie["axes"][PLANE_AXIS]["value"] = "docker"
    ck("★음성대조 평면 값·토큰 모순 = 재조립 거부", _code(compose_from_payload, lie) == "HINT_NAMING_INCONSISTENT")
    Vn = {k: v for k, v in V.items() if k != PLANE_AXIS}
    ck("★vocab 에 plane 축 없음 = HINT_VOCAB_ABSENT(fail-loud)", _code(derive_name, fn, Vn) == "HINT_VOCAB_ABSENT"
       and any(PLANE_AXIS in b for b in validate_vocab(Vn)))
    fx = _tag2_facts()
    fx["plane"] = {"raw": "podman", "source": "fixture"}
    ck("★어휘 밖 평면 = HINT_VOCAB_UNKNOWN", _code(derive_name, fx, V) == "HINT_VOCAB_UNKNOWN")
    fx = _tag2_facts()
    del fx["plane"]
    ck("★평면 사실 부재 = HINT_AXIS_UNDERIVABLE(docker 로 추측 ✗)", _code(derive_name, fx, V) == "HINT_AXIS_UNDERIVABLE")
    for bad_plane, why in (({"docker": "dk", "native": ntok}, "docker 토큰 금지(Docker 이름 불변)"),
                           ({"docker": "", "native": ""}, "native 빈 토큰(충돌)"),
                           ({"docker": ""}, "키 집합 ≠ 판정 소유자 어휘")):
        ck(f"★vocab plane 구조 위반 거부 — {why}", bool(validate_vocab({**V, PLANE_AXIS: bad_plane})))
    ck("★평면 토큰 형태 위반 조립 거부", _code(build_arch, "gb10", 1, 2, "cluster", "native", "Bare") == "HINT_ARCH_SHAPE_VIOLATION")

    # ── vLLM 세그먼트(X18) ──
    sha = "0123456789abcdef0123456789abcdef01234567"
    f = _tag2_facts()
    f["vllm"] = {"track": "source-build", "vllm_ref": sha, "vllm_sha": None, "prev_release": "v0.29.0rc5",
                 "source": "셀 env VLLM_REF"}
    dc = derive_name(f, V)
    ck("커밋 핀 → <prev>-g<sha12>", dc.tag.startswith("hint/0.29.0rc5-g0123456789ab/")
       and dc.vllm_build_input["kind"] == "commit" and dc.vllm_build_input["sha"] == sha)
    ck("직전 릴리스는 describe 긴 형태에서도 해소된다",
       vllm_segment({"track": "source-build", "vllm_ref": sha, "prev_release": "v0.29.0rc5-37-g0123456",
                     "source": "빌드 원장 vllm.describe"}).value == "0.29.0rc5-g0123456789ab")
    ck("describe 약식 SHA 가 핀의 접두면 통과(긴 약식도)",
       vllm_segment({"track": "source-build", "vllm_ref": sha, "prev_release": "v0.29.0rc5-37-g0123456789ab",
                     "source": "빌드 원장 vllm.describe"}).value == "0.29.0rc5-g0123456789ab")
    ck("★음성대조 다른 커밋의 describe(약식 SHA ≠ 핀 접두) = 모순 차단",
       _code(vllm_segment, {"track": "source-build", "vllm_ref": sha, "prev_release": "v0.29.0rc5-37-g7777777",
                            "source": "빌드 원장 vllm.describe"}) == "HINT_VLLM_INPUT_CONFLICT")
    ck("★음성대조 커밋 핀인데 직전 릴리스 없음 = 파생 불가",
       _code(vllm_segment, {"track": "source-build", "vllm_ref": sha, "source": "셀 env"}) == "HINT_AXIS_UNDERIVABLE")
    ck("★음성대조 VLLM_REF 커밋과 vllm_sha 가 다르면 모순 차단",
       _code(vllm_segment, {"track": "source-build", "vllm_ref": sha, "vllm_sha": _TAG2_SHA,
                            "prev_release": "0.29.0rc5", "source": "셀 env"}) == "HINT_VLLM_INPUT_CONFLICT")
    ck("브랜치 ref 는 빌드된 SHA 로 이름 짓는다",
       vllm_segment({"track": "source-build", "vllm_ref": "feature-x", "vllm_sha": sha, "prev_release": "0.29.0",
                     "source": "빌드 원장"}).value == "0.29.0-g0123456789ab")
    ck("wheel 트랙 VLLM_VERSION 릴리스", vllm_segment({"track": "wheel", "vllm_version": "0.18.0",
                                                    "source": "셀 env VLLM_VERSION"}).value == "0.18.0")
    ck("native wheel URL 의 SHA 도 같은 규칙",
       vllm_segment({"track": "native", "prev_release": "v0.1.0",
                     "wheel_url": f"https://wheels.vllm.ai/{sha}/vllm-0.1.1.dev53%2Bg0123456-cp38-abi3-linux_x86_64.whl",
                     "source": "native 설치 명령"}).value == "0.1.0-g0123456789ab")
    ck("★음성대조 nightly 버전 문자열만으로는 파생하지 않는다(짧은 SHA · 직전 릴리스 없음)",
       _code(vllm_segment, {"track": "native", "vllm_version": "0.1.1.dev53+g30118ba27",
                            "source": "엔진 자기보고"}) == "HINT_AXIS_UNDERIVABLE")
    ck("★음성대조 출처 빈 vllm 입력 = 파생 불가",
       _code(vllm_segment, {"track": "wheel", "vllm_version": "0.18.0", "source": ""}) == "HINT_AXIS_UNDERIVABLE")

    # ── 명시 토큰(D6) ──
    f = _tag2_facts()
    f["spec"] = {"raw": None, "source": "서빙 yaml·러너에 speculative-config 없음(관측)"}
    f["ple"] = {"raw": "none", "source": "모델 config 에 PLE 키 없음"}
    f["graph"] = {"raw": "cudagraph", "source": "서빙 yaml enforce-eager 부재"}
    f["kv"] = {"raw": "fp8_e4m3", "source": "서빙 yaml kv-cache-dtype"}
    ds = derive_name(f, V)
    ck(f"specoff·plenone·graph·kvfp8(fp8_e4m3 정규화) 명시 토큰({ds.tag})",
       ds.tag.endswith("/qnvfp4-len262144-kvfp8-plenone-specoff-graph"))

    # ── ★음성대조: 어휘 밖 · 파생 불가 ──
    f = _tag2_facts()
    f["arch"] = dict(f["arch"], gpu_model="NVIDIA B300")
    ck("★미지 hw = HINT_VOCAB_UNKNOWN", _code(derive_name, f, V) == "HINT_VOCAB_UNKNOWN")
    try:
        normalize("hw", "NVIDIA RTX PRO 6000", V)
        ck("★에디션 모호 hw 는 어느 토큰에도 합치지 않는다", False)
    except HintError as e:
        ck("★에디션 모호 hw = HINT_VOCAB_UNKNOWN(에디션 remedy)", e.code == "HINT_VOCAB_UNKNOWN" and "에디션" in e.message)
    try:
        normalize("kv", "fp4_weird", V)
        ck("★어휘 밖 kv 는 통과하지 않는다", False)
    except HintError as e:
        ck("★어휘 밖 remedy 가 추가할 키를 가리킨다", e.code == "HINT_VOCAB_UNKNOWN" and '"kv"' in (e.remedy or "")
           and core.REL_VOCAB in (e.remedy or ""))
    ck("hw 는 대소문자·공백만 정규화한 정확 일치", normalize("hw", "  nvidia   gb10 ", V) == "gb10")
    ck("★부분 일치 ✗(에디션이 다른 원문의 접두사)", _code(normalize, "hw", "NVIDIA GB10 Superchip", V) == "HINT_VOCAB_UNKNOWN")
    f = _tag2_facts()
    del f["spec"]["raw"]
    ck("★raw 키 부재(읽지 못함) = HINT_AXIS_UNDERIVABLE — specoff 로 떨어지지 않는다",
       _code(derive_name, f, V) == "HINT_AXIS_UNDERIVABLE")
    f = _tag2_facts()
    f["kv"] = {"raw": "auto", "source": ""}
    ck("★출처 빈 축 = HINT_AXIS_UNDERIVABLE", _code(derive_name, f, V) == "HINT_AXIS_UNDERIVABLE")
    f = _tag2_facts()
    f["q"] = {"raw": "N/A", "source": "인증서 quantization"}
    ck("★인증서 'N/A' 는 양자화 없음(bf16)이 아니라 파생 불가", _code(derive_name, f, V) == "HINT_AXIS_UNDERIVABLE")
    f = _tag2_facts()
    del f["arch"]["target_gpu"]
    ck("★target_gpu 키 부재 = 파생 불가(None 은 관측한 경우에만)", _code(derive_name, f, V) == "HINT_AXIS_UNDERIVABLE")
    # 2026-09-22 감사: '읽지 못함'·템플릿 빈칸은 어휘 밖 값(HINT_VOCAB_UNKNOWN → "어휘표에 추가하라")이 아니라 파생 불가다.
    for what, patch in (("target_gpu 'N/A'", {"target_gpu": "N/A"}), ("target_gpu 빈칸", {"target_gpu": "<<FILL>>"}),
                        ("gpu_model 빈칸", {"gpu_model": "<<FILL>>"}), ("gpu_model __REQUIRED__", {"gpu_model": "__REQUIRED__"})):
        f = _tag2_facts()
        f["arch"] = dict(f["arch"], **patch)
        ck(f"★{what} = HINT_AXIS_UNDERIVABLE(어휘 추가 처방 ✗)", _code(derive_name, f, V) == "HINT_AXIS_UNDERIVABLE")
    f = _tag2_facts()
    f["q"] = {"raw": "<<FILL>>", "source": "셀 선언"}
    ck("★레시피 축 템플릿 빈칸 = HINT_AXIS_UNDERIVABLE", _code(derive_name, f, V) == "HINT_AXIS_UNDERIVABLE")
    try:
        normalize("hw", "N/A", V)
        ck("★'읽지 못함' 표기는 정규화되지 않는다", False)
    except HintError as e:
        ck("★'읽지 못함' 표기의 remedy 는 어휘 추가를 권하지 않는다",
           e.code == "HINT_VOCAB_UNKNOWN" and "추가하지 않는다" in (e.remedy or ""))
    f = _tag2_facts()
    del f["len"]
    ck("★축 통째 부재 = HINT_AXIS_UNDERIVABLE", _code(derive_name, f, V) == "HINT_AXIS_UNDERIVABLE")
    f = _tag2_facts()
    f["arch"] = dict(f["arch"], role="main")
    ck("★main 인데 N=2 = HINT_ARCH_ROLE_NODES_MISMATCH", _code(derive_name, f, V) == "HINT_ARCH_ROLE_NODES_MISMATCH")
    f = _tag2_facts()
    f["arch"] = dict(f["arch"], nodes=1, role="sub", target_gpu="NVIDIA H100")
    ck("시뮬레이션 타겟 → sim-<hw> · 서브 단독 1g1n", "/gb10-1g1n-sub-sim-h100/" in derive_name(f, V).tag)
    f["arch"] = dict(f["arch"], target_gpu="GB10")
    ck("타겟 = 호스트 hw 면 native", "/gb10-1g1n-sub-native/" in derive_name(f, V).tag)
    f = _tag2_facts()
    f["model"] = {"hf_repo": "Qwen/Qwen3.8-Flash-Next", "model_path": "/app/q/Qwen/Qwen3.8-Flash-Next-NVFP4",
                  "source": "HF 카드 base_model"}
    ck("★hf_repo(base_model 오기) ≠ 체크포인트 basename = HINT_MODEL_SLUG_CONFLICT",
       _code(derive_name, f, V) == "HINT_MODEL_SLUG_CONFLICT")

    # ── 옛 세대 이름: 읽기 전용 수용 · 신규 거부(세대별 사유코드) ──
    legacy = {"hint/0.18.0/gpt-oss-20b/gb10-sim-h100/qmxfp4-len131072-kvfp8":
                  (GRAMMAR_LEGACY_ARCH, "HINT_ARCH_NODE_AXIS_ABSENT"),
              "hint/0.29.0/qwen3.8-flash-next-nvfp4/gb10x2-cluster-native/len262144-kvauto-plemmap":
                  (GRAMMAR_LEGACY_ARCH_NODE, "HINT_ARCH_LEGACY_GRAMMAR"),
              "hint/0.18.0/gpt-oss-20b/gb10-main-sim-h100/qmxfp4-len131072-kvfp8":
                  (GRAMMAR_LEGACY_ARCH_NODE, "HINT_ARCH_LEGACY_GRAMMAR")}
    for name, (gram, code) in legacy.items():
        arch = name.split("/")[3]
        ck(f"옛 이름 읽기 전용 해체({arch} → {gram})", parse_tag(name).grammar == gram)
        ck(f"★옛 arch {arch} 는 신규 이름에서 {code} 로 거부", _code(validate_new_name, name) == code)
    ck("★옛 세대 사유코드는 형태 위반과 다르고 서로도 다르다",
       len({c for _, c in legacy.values()} | {"HINT_ARCH_SHAPE_VIOLATION"}) == 3)
    ck("타 발행처 옛 이름(nightly vllm 세그먼트)도 읽기는 한다",
       parse_tag("hint/0.1.1.dev53+g30118ba27/qwen3.8-flash-next-fp8/rtxpro6000x2-main-native/"
                 "qfp8-len262144-kvauto-pleresident").grammar == GRAMMAR_LEGACY_ARCH_NODE)
    ck("★음성대조 4세그먼트는 parse_tag 가 HINT_NAME_SHAPE",
       _code(parse_tag, "hint/0.23.0/deepseek-v4-flash/sm121") == "HINT_NAME_SHAPE")
    ck("grammar_of 는 4세그먼트에 라벨만 붙인다", grammar_of("hint/0.23.0/deepseek-v4-flash/sm121") == GRAMMAR_LEGACY_4SEG)
    ck("★음성대조 6세그먼트·빈 세그먼트 거부",
       _code(parse_tag, "hint/a/b/c/d/e") == "HINT_NAME_SHAPE" and _code(parse_tag, "hint/a//c/d") == "HINT_NAME_SHAPE")
    ck("★v6 처럼 생긴 위반은 옛 세대로 오분류하지 않는다",
       arch_violation("gb10-0g1n-main-native") == "HINT_ARCH_SHAPE_VIOLATION"
       and grammar_of("hint/0.29.0/m/gb10-0g1n-main-native/x") == GRAMMAR_UNKNOWN)
    # 옛 hint_tag·selftest_hint_gate 의 arch 음성대조 이관(2026-09-06)
    ck("arch 부재 차단", arch_violation("") == "HINT_ARCH_ABSENT")
    ck("★대문자는 형태 위반", arch_violation("GB10-main-x") == "HINT_ARCH_SHAPE_VIOLATION"
       and arch_violation("GB10-1g1n-main-native") == "HINT_ARCH_SHAPE_VIOLATION")
    ck("★미지 노드 축(옛 세대 모양)은 노드 축 부재", arch_violation("gb10-node-x") == "HINT_ARCH_NODE_AXIS_ABSENT")
    ck("v6 3형상 통과", all(arch_violation(a) is None for a in
                          ("gb10-1g1n-main-sim-h100", "gb10-1g1n-sub-native", "gb10-1g2n-cluster-native",
                           "rtxpro6000maxq-2g1n-main-native")))
    ck("arch 해체가 축을 준다", parse_arch("gb10-1g2n-cluster-native") ==
       {"grammar": GRAMMAR_V6, "hw": "gb10", "gpus_per_node": 1, "nodes": 2, "role": "cluster", "target": "native",
        "plane_token": "", "plane": "docker"})
    ck("옛 arch 도 축 해체(읽기 전용)", parse_arch("gb10x2-cluster-sim-h100") ==
       {"grammar": GRAMMAR_LEGACY_ARCH_NODE, "hw": "gb10x2", "role": "cluster", "target": "sim-h100"})
    ck("★음성대조 미지 노드 축 조립 거부", _code(build_arch, "gb10", 1, 1, "worker", "native") == "HINT_ARCH_SHAPE_VIOLATION")
    ck("★음성대조 G=0 조립 거부", _code(build_arch, "gb10", 0, 1, "main", "native") == "HINT_AXIS_VALUE_INVALID")
    ck("★음성대조 bool 은 수가 아니다", _code(build_arch, "gb10", True, 1, "main", "native") == "HINT_AXIS_VALUE_INVALID")
    # 레시피(옛 RECIPE_SHAPE 음성대조의 후계)
    ck("v6 레시피 통과", recipe_violation("qnvfp4-len262144-kvauto-plemmap-spec3-eager") is None)
    ck("★옛 3축 레시피는 v6 위반", recipe_violation("qmxfp4-len131072-kvfp8") == "HINT_RECIPE_SHAPE_VIOLATION")
    ck("★축 순서가 다르면 위반", recipe_violation("len262144-qnvfp4-kvauto-plemmap-spec3-eager") == "HINT_RECIPE_SHAPE_VIOLATION")
    ck("★결측 축 생략(옛 동작)은 위반", recipe_violation("qnvfp4-len262144-kvauto-spec3-eager") == "HINT_RECIPE_SHAPE_VIOLATION")
    ck("★손저작 형태(대문자·spec0·접미)는 위반",
       all(recipe_violation(r) == "HINT_RECIPE_SHAPE_VIOLATION" for r in
           ("QNVFP4-len1-kvauto-plemmap-spec3-eager", "qnvfp4-len1-kvauto-plemmap-spec0-eager",
            "qnvfp4-len1-kvauto-plemmap-spec3-eager_260904T0730Z")))
    ck("레시피 비었음", recipe_violation("") == "HINT_RECIPE_ABSENT")
    ck("★vLLM 세그먼트에 v 접두 ✗",
       _code(validate_new_name, _TAG2_EXPECTED.replace("/0.29.0rc6/", "/v0.29.0rc6/")) == "HINT_VLLM_SEGMENT_SHAPE")
    ck("★모델 슬러그 대문자 ✗",
       _code(validate_new_name, _TAG2_EXPECTED.replace("qwen3.8", "Qwen3.8")) == "HINT_MODEL_SLUG_SHAPE")
    ck("모델 슬러그 인덱스는 그대로 [2](레시피를 맨 뒤에 붙인 이유 · 2026-09-04)",
       _TAG2_EXPECTED.split("/")[2] == "qwen3.8-flash-next-nvfp4")

    # ── 슬러그 · sweep_bench 규칙 교차 대조(2026-08-15) ──
    repo = core.HINTLIB_DIR.parents[4]
    prod = repo / _SLUG_PRODUCER[0]
    try:
        ptext = prod.read_text(encoding="utf-8")
    except OSError:
        ptext = ""
    ck("★producer(sweep_bench.sh) 에 인증서 model 키 규칙 리터럴이 실재한다(거울 갈라짐 방지)",
       all(lit in ptext for lit in _SLUG_PRODUCER[1]))

    def sweep_rule(p: str) -> str:   # 위 리터럴 두 줄의 파이썬 거울: basename(rstrip('/')).strip().lower()
        return os.path.basename(p.rstrip("/")).strip().lower()

    def old_rule(p: str) -> str:     # 옛 hint_tag.derive_slug(model_path) 규칙의 거울(2026-08-20 R1 · 파일은 퇴역)
        return re.sub(r"[^a-z0-9._-]+", "-", os.path.basename(p.rstrip("/")).lower()).strip("-")

    for p in ("/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4", "/app/models/OpenAI/gpt-oss-20b/",
              "/app/models/google/gemma-4-E2B-it", "/app/models/LiquidAI/LFM2.5-2.6B", "rel/Qwen3-4B"):
        ck(f"derive_slug(model_path) = sweep_bench 인증서 model 키({p})", derive_slug(model_path=p) == sweep_rule(p))
        ck(f"derive_slug(model_path) = 옛 hint_tag 규칙(안전 문자 basename · {p})", derive_slug(model_path=p) == old_rule(p))
    ck("★두 옛 규칙이 갈라지는 basename(공백·+)은 어느 쪽으로도 맞추지 않고 차단한다",
       sweep_rule("/m/My Model+v2") != old_rule("/m/My Model+v2")
       and _code(derive_slug, None, "/m/My Model+v2") == "HINT_MODEL_SLUG_UNSAFE")
    ck("hf_repo 는 마지막 성분 소문자", derive_slug("Qwen/Qwen3.8-Flash-Next-NVFP4") == "qwen3.8-flash-next-nvfp4")
    ck("hf_repo 가 model_path 보다 먼저(옛 우선순위)", derive_slug("org/Abc", "/x/Def") == "abc")
    ck("★음성대조 '/' 없는 hf_repo", _code(derive_slug, "Qwen3-4B") == "HINT_MODEL_HF_REPO_SHAPE")
    ck("★음성대조 입력 없음 = 파생 불가", _code(derive_slug) == "HINT_AXIS_UNDERIVABLE")
    ck("★음성대조 치환이 필요한 basename 은 차단(인증서 키와 갈라짐)",
       _code(derive_slug, None, "/m/My Model+v2") == "HINT_MODEL_SLUG_UNSAFE")
    ck("base_slug 가 양자화 접미를 뗀다",
       base_slug("qwen3.8-flash-next-nvfp4", V) == "qwen3.8-flash-next" and base_slug("hy3-nvfp4-w4a16", V) == "hy3"
       and base_slug("gpt-oss-20b", V) == "gpt-oss-20b" and base_slug("qwen3-4b", V) == "qwen3-4b")
    ck("★접미뿐인 슬러그는 비우지 않는다", base_slug("fp8", V) == "fp8")
    ck("★quant_suffixes 없는 어휘표 = 차단(조용히 아무것도 떼지 않는 폴백 ✗)",
       _code(base_slug, "qwen3-4b-fp8", {}) == "HINT_VOCAB_ABSENT"
       and _code(base_slug, "qwen3-4b-fp8", None) == "HINT_VOCAB_ABSENT")
    ck("정규화 슬러그 동치(2026-08-20 철자 분열)", norm_slug("gemma-4-E2B-it") == norm_slug("gemma-4-e2b-it"))
    ck("★None 은 빈 키('none' 슬러그로 접지 않는다)", norm_slug(None) == "" and norm_slug("None") == "none")
    order = ["0.29.0rc5", "0.29.0rc5-g0123456789ab", "0.29.0rc6", "0.29.0", "0.29.1"]
    ck("version_key 가 릴리스·커밋 핀·rc 를 문법대로 줄 세운다", sorted(reversed(order), key=version_key) == order)
    order4 = ["0.9.2", "0.9.2.post1", "0.9.2.1", "0.9.2.1.post1", "0.9.3rc1", "0.9.3"]
    ck("version_key — 4마디 · .postN 도 문법대로(D-g)", sorted(reversed(order4), key=version_key) == order4)

    # ── D-g: 4마디 · .postN 릴리스 ──
    f = _tag2_facts()
    for ref, seg in (("v0.9.2.1", "0.9.2.1"), ("v0.9.2.post1", "0.9.2.post1"), ("0.9.2.1rc2", "0.9.2.1rc2")):
        f["vllm"] = dict(_tag2_facts()["vllm"], vllm_ref=ref)
        got = derive_name(f, V)
        ck(f"D-g 릴리스 {ref} → 세그먼트 {seg} · v6 문법 통과", got.tag.split("/")[1] == seg
           and parse_tag(got.tag).grammar == GRAMMAR_V6 and got.vllm_build_input["kind"] == "release")
    ck("D-g wheel 4마디 릴리스", vllm_segment({"track": "wheel", "vllm_version": "0.9.2.1",
                                            "source": "셀 env VLLM_VERSION"}).value == "0.9.2.1")
    ck("D-g describe 4마디 · 커밋 핀", vllm_segment({"track": "source-build", "vllm_ref": sha,
                                                   "prev_release": "v0.9.2.1-3-g0123456", "source": "빌드 원장"}).value
       == "0.9.2.1-g0123456789ab")
    ck("★D-g 다섯 마디·nightly 는 여전히 릴리스 아님", _code(vllm_segment, {"track": "wheel", "vllm_version": "0.9.2.1.1",
                                                               "source": "셀 env"}) == "HINT_AXIS_UNDERIVABLE"
       and _code(validate_new_name, _TAG2_EXPECTED.replace("/0.29.0rc6/", "/0.1.1.dev53/")) == "HINT_VLLM_SEGMENT_SHAPE")

    # ── D-h: 포크 저장소의 릴리스 모양 ref = SHA 경로 또는 차단 ──
    up = "https://" + UPSTREAM_VLLM_REPOS[0] + ".git"
    ck("vllm_repo_kind: https · scp형 · 끝 슬래시 = upstream",
       vllm_repo_kind(up) == "upstream" and vllm_repo_kind("git@" + UPSTREAM_VLLM_REPOS[0].replace("/", ":", 1) + ".git")
       == "upstream" and vllm_repo_kind(up[:-4] + "/") == "upstream")
    ck("vllm_repo_kind: 포크 · 로컬 경로 = fork · 미관측 = None",
       vllm_repo_kind("https://example.invalid/me/vllm.git") == "fork" and vllm_repo_kind("./vllm-src") == "fork"
       and vllm_repo_kind(None) is None and vllm_repo_kind("N/A") is None)
    fork = "https://example.invalid/someone/vllm.git"
    ck("★D-h 포크 + 릴리스 모양 ref + SHA 없음 = 파생 불가(업스트림 이름 ✗)",
       _code(vllm_segment, {"track": "source-build", "vllm_ref": "v0.29.0rc6", "vllm_repo": fork, "source": "셀 env"})
       == "HINT_AXIS_UNDERIVABLE")
    fk = derive_name({**_tag2_facts(), "vllm": dict(_tag2_facts()["vllm"], vllm_repo=fork,
                                                    prev_release="v0.29.0rc6")}, V)
    ck(f"D-h 포크 + SHA + 직전 릴리스 → <prev>-g<sha12>({fk.tag.split('/')[1]})",
       fk.tag.split("/")[1] == "0.29.0rc6-g" + _TAG2_SHA[:12] and fk.vllm_build_input["kind"] == "commit"
       and fork not in json.dumps(fk.to_payload(), ensure_ascii=False))
    ck("★D-h vllm_repo 키 부재 = 파생 불가", _code(vllm_segment, {"track": "source-build", "vllm_ref": "v0.29.0rc6",
                                                            "vllm_sha": _TAG2_SHA, "source": "셀 env"}) == "HINT_AXIS_UNDERIVABLE")
    ck("★D-h VLLM_REPO 미관측(None) = 파생 불가", _code(vllm_segment, {"track": "source-build", "vllm_ref": "v0.29.0rc6",
                                                                 "vllm_repo": None, "vllm_sha": _TAG2_SHA,
                                                                 "source": "셀 env"}) == "HINT_AXIS_UNDERIVABLE")
    ck("D-h 40자 커밋 핀은 저장소와 무관(SHA 가 곧 좌표)", vllm_segment({"track": "source-build", "vllm_ref": sha,
                                                                  "vllm_repo": fork, "prev_release": "v0.29.0rc5",
                                                                  "source": "셀 env"}).value == "0.29.0rc5-g0123456789ab")

    # ── 어휘표 적재 ──
    with tempfile.TemporaryDirectory() as td:
        r = Path(td)
        ck("★어휘표 부재 = HINT_VOCAB_ABSENT", _code(load_vocab, r) == "HINT_VOCAB_ABSENT")
        (r / "hints").mkdir()
        (r / core.REL_VOCAB).write_text("{not json", encoding="utf-8")
        ck("★어휘표 깨짐 = HINT_VOCAB_ABSENT", _code(load_vocab, r) == "HINT_VOCAB_ABSENT")
        broken = _fixture_vocab()
        broken["kv"]["fp8e5m2"].append("FP8_E4M3")
        (r / core.REL_VOCAB).write_text(json.dumps(broken), encoding="utf-8")
        ck("★원문 하나가 두 토큰에 걸리면 HINT_VOCAB_ABSENT", _code(load_vocab, r) == "HINT_VOCAB_ABSENT")
        broken = _fixture_vocab()
        broken["quant"]["bf16"].append("N/A")
        ck("★'N/A' 를 값으로 등재하면 구조 검사가 잡는다", any("읽지 못함" in p for p in validate_vocab(broken)))
        broken = _fixture_vocab()
        broken["hw"]["gb10"].append("<<FILL>>")
        ck("★템플릿 빈칸을 값으로 등재하면 구조 검사가 잡는다", any("빈칸" in p for p in validate_vocab(broken)))
        broken = _fixture_vocab()
        broken["hw"]["rtxpro6000srv"].append("NVIDIA RTX PRO 6000")
        ck("★에디션 모호 원문을 토큰에 합치면 구조 검사가 잡는다", any("에디션" in p for p in validate_vocab(broken)))
        broken = _fixture_vocab()
        broken["graph"]["cudagraph"] = ["cg"]
        broken["kv_"] = {}
        ck("★graph 토큰 추가·오타 키는 구조 검사가 잡는다", len(validate_vocab(broken)) >= 2)
        (r / core.REL_VOCAB).write_text(json.dumps(_fixture_vocab()), encoding="utf-8")
        ck("정상 어휘표 적재", load_vocab(r)["kv"]["fp8"][0] == "fp8")

    # 추적 어휘표(배포물) — 저장소에 실재하는 gpu 원문이 에디션 토큰으로 떨어지는가(tripwire: 삭제·오타를 잡는다)
    try:
        real = load_vocab(repo)
    except HintError as e:
        real = None
        ck(f"추적 어휘표 적재({e.code}: {e.message})", False)
    if real is not None:
        observed = {"NVIDIA GB10": "gb10", "GB10": "gb10", "NVIDIA H100": "h100", "H100": "h100",
                    "RTX 4090 (simulated on GB10)": "rtx4090",
                    "NVIDIA RTX PRO 6000 Blackwell Max-Q Workstation Edition x2": "rtxpro6000maxq",
                    "NVIDIA RTX PRO 6000 Blackwell Server Edition": "rtxpro6000srv"}
        for raw, tok in observed.items():
            ck(f"추적 어휘표: {raw!r} → {tok}", _code(normalize, "hw", raw, real) is None
               and normalize("hw", raw, real) == tok)
        ck("★추적 어휘표: 에디션 모호 원문은 차단",
           _code(normalize, "hw", "NVIDIA RTX PRO 6000", real) == "HINT_VOCAB_UNKNOWN")
        ck("추적 어휘표: kv fp8·fp8_e4m3 → fp8(F13)",
           normalize("kv", "fp8", real) == "fp8" and normalize("kv", "fp8_e4m3", real) == "fp8")
        ck("★추적 어휘표: quant 'N/A' 는 어휘 밖", _code(normalize, "quant", "N/A", real) == "HINT_VOCAB_UNKNOWN")
        ck("추적 어휘표로도 태그2 기대 이름", derive_name(_tag2_facts(), real).tag == _TAG2_EXPECTED)
        # AC-N6(plan_26092311): 추적 어휘표의 native 토큰 = O-N1 승인값 `bare` · N1 이름이 D1 과 갈라진다
        fr = {**_tag2_facts(), PLANE_AXIS: {"raw": "native", "source": "fixture · serve_proof plane=native"}}
        ck("AC-N6 추적 어휘표: native 셀 → …-cluster-native-bare",
           derive_name(fr, real).tag == _TAG2_EXPECTED.replace("-cluster-native/", "-cluster-native-bare/"))

    # ── import 부수효과 0: PATH 를 끊고 새 프로세스에서 적재 · 감사 훅으로 프로세스 실행·비 .py 파일 열기 관측 ──
    try:
        out = subprocess.run([sys.executable, "-B", "-c", _IMPORT_PROBE, str(core.SCRIPTS_DIR)],
                             capture_output=True, text=True, env={"PATH": "/nonexistent"}, timeout=60)
        ok = out.returncode == 0
        events = json.loads(out.stdout) if ok else None
    except (OSError, ValueError, subprocess.SubprocessError) as e:
        ok, events, out = False, None, None
        bad.append(f"naming: import 탐침 실행 실패: {e!r}")
    ck(f"import 는 PATH 없이도 성공한다(git 호출 없음){'' if ok else ' · ' + (out.stderr[-300:] if out else '')}", ok)
    ck(f"★import 가 프로세스를 띄우거나 파일을 열지 않는다({events})", events == [])
    return bad
