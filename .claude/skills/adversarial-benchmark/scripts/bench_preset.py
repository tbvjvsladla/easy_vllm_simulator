#!/usr/bin/env python3
# bench_preset.py — full-bench knob 프리셋 계약의 단일 소유자 (plan_26101011 · interview_20261010_015151)
#
# 무엇을 하나
#   · 바인딩 `config/bench_bindings.yaml`(추상 벤치 이름 → 도구 폴더)과 도구 폴더의 프리셋
#     `config/<폴더>/<이름>.yaml` 을 읽고 **닫힌 스키마**로 검증한다(값만 조정 · 키 추가/삭제는 계약 변경).
#   · 측정 형상 knob 을 해소한다: 사람 핀(CLI --pin K=V --pinned-by) > 사람 핀(campaign.yaml) > Agent 선택 > 프리셋 default.
#     스윕 플래그(--levels 등)는 Agent 선택이고 근거 코드(--knob-reason)를 남긴다. 사람 핀은 knob 단위로 귀속을 밝힌다.
#   · 해소 결과(값·출처·등급·근거 코드·상한 필터 제외분)를 기록 블록 `bench_knobs` 로 낸다 —
#     sweep_bench.sh 가 sweep_index.json 최상위에 싣는다.
#
# 등급 = 누가 그 선택지를 여나(정적 선언 · 자기신뢰도 라우팅 ✗)
#   auto     Agent 가 고른다
#   confirm  셀별 사전 승인(사람 발화 전사 --approved-by)이 있을 때만 Agent 가 고른다
#   escalate 사람 핀으로만 열린다
#   새 차단점(기동 중 팝업·셀 보류)은 만들지 않는다 — 조건이 안 서면 기동 거부(exit 2)다.
#
# 반복 수는 `repeat_axis.py` 가 해소를 소유한다(argv > 캠페인 > 프리셋 기본값). 여기서는 프리셋 기본값을
#   넘기고 돌아온 값에 등급·열거 검사를 건다. 하한(FULL_REPEATS_MIN)은 full 정의라 repeat_axis 에 남고,
#   프리셋 선택지가 그 하한을 지키는지 로더가 교차검증한다.
#
# 사용
#   bench_preset.py validate [--preset NAME|PATH]
#   bench_preset.py default --knob K [--preset NAME|PATH]   → 'VALUE<US>PRESET'(broad_search init 의 반복 수 기본값)
#   bench_preset.py resolve --tool vllm|guidellm [--preset NAME|PATH] [--serving-yaml PATH] [--dry-run]
#       [--levels CSV] [--input-len N] [--output-len N] [--num-prompts N] [--warmups N]
#       [--repeats N [--repeats-source TEXT]] [--backend X] [--bench-budget-mib N]
#       [--pin KNOB=VALUE... --pinned-by TEXT] [--knob-reason KNOB=CODE]... [--approved-by TEXT] [--campaign-id ID]
#       --record-out PATH
#     → stdout: `KEY<US>VALUE` 줄(LEVELS·ILEN·OLEN·NPROMPTS·WARMUPS·REPEATS·REPEATS_SOURCE·REPEAT_KIND·BACKEND·BENCH_BUDGET_MIB)
#   bench_preset.py --self-test
# 종료: 0=성공 · 2=계약 위반/입력 거부(fail-closed) · 1=자체검사 실패
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from pathlib import Path

import yaml

SDIR = Path(__file__).resolve().parent
SKILL_DIR = SDIR.parent
sys.path.insert(0, str(SDIR))
import repeat_axis as _ra  # noqa: E402

BINDINGS_REL = "config/bench_bindings.yaml"
FULL_BENCH = "full-bench"
DEFAULT_PRESET = "default_preset"
SCHEMA_VERSION = 1

GRADES = ("auto", "confirm", "escalate")
OWNERS = ("agent", "user")
CAP_LE_MAX_NUM_SEQS = "le_max_num_seqs"
CAP_INPUT_PLUS_OUTPUT = "input_plus_output_le_max_model_len"
CAP_NONE = "none"
HUMAN_PIN_REASON = "human_pin"
DEFAULT_REASON = "preset_default"
VERDICT_LEVEL = 1          # 판정점(동시성 1) — sweep_bench 가 verdict 를 재사용한다(verdict_point_level)

# 닫힌 스키마(tripwire — 바꾸려면 이 표와 자체검사의 스냅샷이 함께 바뀌어야 한다).
TOP_KEYS = ("schema_version", "bench", "tool", "reason_codes", "knobs")
KNOB_KEYS = ("owner", "kind", "default", "serving_cap", "options", "reasons")
OPTION_KEYS = ("value", "grade")
# knob → (kind, 값 타입, 허용 상한 필터, 기본값 없음 강제 여부)
KNOB_SPECS = {
    "levels":           ("list",   int, (CAP_LE_MAX_NUM_SEQS, CAP_NONE), False),
    "input_len":        ("scalar", int, (CAP_INPUT_PLUS_OUTPUT, CAP_NONE), False),
    "output_len":       ("scalar", int, (CAP_NONE,), False),
    "num_prompts":      ("scalar", int, (CAP_NONE,), False),
    "warmups":          ("scalar", int, (CAP_NONE,), False),
    "repeats":          ("scalar", int, (CAP_NONE,), False),
    "backend":          ("scalar", str, (CAP_NONE,), True),
    "bench_budget_mib": ("scalar", int, (CAP_NONE,), True),
    # 데이터셋 축(2026-10-16 · plan_26101614 · interview_20261016_045736) — 전부 owner: user.
    #   입력 텍스트의 출처만 바꾼다(길이는 input_len/output_len 이 그대로 정한다 · 판정 불변).
    #   자유값 knob(path·hex·map·text)은 options 가 **그 프리셋이 선언한 값**만 든다(닫힌 목록 계약 유지)
    #   — 값을 바꾸는 손은 프리셋 작성자(사람)이고, 모양 검사(DATASET_SHAPE)가 한 번 더 닫는다.
    "dataset_kind":        ("scalar", str, (CAP_NONE,), False),
    "dataset_source":      ("scalar", str, (CAP_NONE,), False),
    "dataset_id":          ("text",   str, (CAP_NONE,), False),
    "dataset_path":        ("path",   str, (CAP_NONE,), False),
    "dataset_revision":    ("hex40",  str, (CAP_NONE,), False),
    "source_sha256":       ("hex64",  str, (CAP_NONE,), False),
    "column_map":          ("map",    dict, (CAP_NONE,), False),
    "preprocess_strategy": ("scalar", str, (CAP_NONE,), False),
    "seed":                ("scalar", int, (CAP_NONE,), False),
}
# 데이터셋 knob — synthetic 이면 나머지는 전부 null, 아니면 전부 값이 있어야 한다(교차 규칙 · validate/resolve 공용).
DATASET_KNOBS = ("dataset_kind", "dataset_source", "dataset_id", "dataset_path", "dataset_revision",
                 "source_sha256", "column_map", "preprocess_strategy", "seed")
DATASET_SYNTHETIC = "synthetic"
# 값 어휘(닫힌 목록 — GuideLLM v0.7.3 `--data kind=` · `preprocess --strategy kind=` 의 부분집합).
DATASET_KINDS = (DATASET_SYNTHETIC, "parquet_file", "json_file", "csv_file")
DATASET_SOURCES = ("managed", "ephemeral", "custom")
PREPROCESS_STRATEGIES = ("concatenate", "ignore", "pad", "error")
# column_map 키 = GuideLLM generative_column_mapper 의 열 역할(이번 개정은 입력 텍스트 열 하나만 쓴다).
COLUMN_MAP_KEYS = ("text_column",)
# 도구가 실제로 받는 --tool 값(run_bench.sh · sweep_bench.sh 의 case 와 같은 닫힌 목록).
KNOWN_TOOLS = ("vllm", "guidellm")
# 스윕 CLI 플래그 ↔ knob
FLAG_OF = {"levels": "--levels", "input_len": "--input-len", "output_len": "--output-len",
           "num_prompts": "--num-prompts", "warmups": "--warmups", "repeats": "--repeats",
           "backend": "--backend", "bench_budget_mib": "--bench-budget-mib"}
# 데이터셋 knob 은 스윕 플래그가 없다(owner: user — 프리셋 작성 또는 사람 핀 --pin 으로만 바뀐다).
FLAG_OF.update({k: "--pin %s" % k for k in DATASET_KNOBS})
# stdout 키 ↔ knob (sweep_bench 의 셸 변수 이름)
SHELL_KEY_OF = {"levels": "LEVELS", "input_len": "ILEN", "output_len": "OLEN", "num_prompts": "NPROMPTS",
                "warmups": "WARMUPS", "repeats": "REPEATS", "backend": "BACKEND",
                "bench_budget_mib": "BENCH_BUDGET_MIB"}


class PresetError(ValueError):
    """프리셋 계약이나 해소 입력이 성립하지 않는다 — 기본값으로 메우지 않고 기동을 거부한다."""


# ── 로드 · 검증 ────────────────────────────────────────────────────────────────────────

def _load_yaml(path: Path):
    try:
        return yaml.safe_load(Path(path).read_text(encoding="utf-8"))
    except OSError as exc:
        raise PresetError("%s 를 읽지 못했다: %s" % (path, exc))
    except yaml.YAMLError as exc:
        raise PresetError("%s 가 yaml 이 아니다: %s" % (path, exc))


def load_bindings(skill_dir: Path = SKILL_DIR) -> dict:
    path = Path(skill_dir) / BINDINGS_REL
    doc = _load_yaml(path)
    if not isinstance(doc, dict) or set(doc) != {FULL_BENCH}:
        raise PresetError("%s 는 정확히 키 '%s' 하나여야 한다(지금: %r)" % (BINDINGS_REL, FULL_BENCH,
                                                                   sorted(doc) if isinstance(doc, dict) else doc))
    folder = doc[FULL_BENCH]
    if not isinstance(folder, str) or not re.fullmatch(r"[a-z0-9][a-z0-9._-]*", folder):
        raise PresetError("%s 의 %s 값은 도구 폴더 이름이어야 한다: %r" % (BINDINGS_REL, FULL_BENCH, folder))
    if not (Path(skill_dir) / "config" / folder).is_dir():
        raise PresetError("%s 가 가리키는 도구 폴더 config/%s/ 가 없다" % (BINDINGS_REL, folder))
    return doc


def preset_path(name, skill_dir: Path = SKILL_DIR, bindings: "dict | None" = None) -> Path:
    """프리셋 이름(→ config/<바인딩 폴더>/<이름>.yaml) 또는 경로를 고른다. 미지정 = default_preset."""
    bindings = bindings or load_bindings(skill_dir)
    folder = Path(skill_dir) / "config" / bindings[FULL_BENCH]
    if not name:
        name = DEFAULT_PRESET
    cand = Path(name)
    if "/" in str(name) or cand.suffix in (".yaml", ".yml"):
        p = cand if cand.is_absolute() else Path.cwd() / cand
        if not p.is_file():
            raise PresetError("프리셋 파일이 없다: %s" % name)
        return p.resolve()
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", str(name)):
        raise PresetError("프리셋 이름은 [A-Za-z0-9._-] 만 쓴다: %r" % name)
    p = folder / ("%s.yaml" % name)
    if not p.is_file():
        avail = sorted(x.stem for x in folder.glob("*.yaml"))
        raise PresetError("프리셋 '%s' 이 config/%s/ 에 없다(있는 것: %s)" % (name, bindings[FULL_BENCH], avail))
    return p


def _typed(value, typ) -> bool:
    if typ is int:
        return isinstance(value, int) and not isinstance(value, bool)
    return isinstance(value, typ)


def _dataset_shape_problem(name: str, value) -> "str | None":
    """데이터셋 knob 값 하나의 모양 검사(형식만 · 실재 검사는 측정 시점 `check_dataset_runtime`). None = 통과."""
    if value is None:
        return None
    kind = KNOB_SPECS[name][0]
    if name == "dataset_kind" and value not in DATASET_KINDS:
        return "%r 는 %s 밖이다" % (value, list(DATASET_KINDS))
    if name == "dataset_source" and value not in DATASET_SOURCES:
        return "%r 는 %s 밖이다" % (value, list(DATASET_SOURCES))
    if name == "preprocess_strategy" and value not in PREPROCESS_STRATEGIES:
        return "%r 는 %s 밖이다" % (value, list(PREPROCESS_STRATEGIES))
    if kind == "path" and not (isinstance(value, str) and value.startswith("/") and ".." not in value.split("/")):
        return "%r 는 '..' 없는 절대경로여야 한다" % (value,)
    if kind == "hex40" and not (isinstance(value, str) and re.fullmatch(r"[0-9a-f]{40}", value)):
        return "%r 는 40자 소문자 hex(HF 커밋 SHA)여야 한다" % (value,)
    if kind == "hex64" and not (isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value)):
        return "%r 는 64자 소문자 hex(sha256)여야 한다" % (value,)
    if kind == "text" and not (isinstance(value, str) and value.strip() and "\n" not in value):
        return "%r 는 비어 있지 않은 한 줄 문자열이어야 한다" % (value,)
    if kind == "map":
        if not isinstance(value, dict) or set(value) != set(COLUMN_MAP_KEYS):
            return "%r 는 정확히 키 %s 를 가진 객체여야 한다" % (value, list(COLUMN_MAP_KEYS))
        if not all(isinstance(v, str) and re.fullmatch(r"[A-Za-z0-9_.-]+", v) for v in value.values()):
            return "%r 의 열 이름은 [A-Za-z0-9_.-] 문자열이다" % (value,)
    return None


def dataset_cross_problems(values: dict) -> list:
    """데이터셋 knob 교차 규칙(synthetic ↔ 나머지 null) — 프리셋 기본값과 해소값에 같은 규칙을 건다."""
    kind = values.get("dataset_kind")
    rest = [k for k in DATASET_KNOBS if k != "dataset_kind"]
    if kind == DATASET_SYNTHETIC:
        bad = [k for k in rest if values.get(k) is not None]
        return (["dataset_kind=synthetic 인데 %s 가 null 이 아니다 — 합성 입력에는 출처 값이 없다" % bad] if bad else [])
    missing = [k for k in rest if values.get(k) is None]
    return (["dataset_kind=%r 인데 %s 가 비었다 — 실제 데이터셋은 출처 전부를 선언한다" % (kind, missing)]
            if missing else [])


def check_dataset_runtime(values: dict) -> dict:
    """측정 시점 검사(dry-run 은 건너뛴다) — 원본 실재·읽기·sha256 일치. 위반은 PresetError(기동 거부).
    → 관측 기록(출처 필드에 붙인다). git 이 바이트를 들지 않는 입력이라 digest 대조가 정당한 자리다(맹점층)."""
    if values.get("dataset_kind") == DATASET_SYNTHETIC:
        return {"checked": "not_applicable(synthetic)"}
    if values.get("dataset_source") == "ephemeral":
        raise PresetError("dataset_source=ephemeral 은 이번 개정에서 경로가 없다 — 벤치 컨테이너는 HF_HUB_OFFLINE=1 이고 "
                          "임시 다운로드 경로를 아직 만들지 않았다(managed|custom 으로 원본을 두고 다시)")
    p = Path(values["dataset_path"])
    if not p.is_file():
        raise PresetError("dataset_path 가 없다(또는 파일이 아니다): %s" % p)
    if not os.access(p, os.R_OK):
        raise PresetError("dataset_path 를 읽을 수 없다: %s" % p)
    h = hashlib.sha256()
    with p.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    got = h.hexdigest()
    if got != values["source_sha256"]:
        raise PresetError("dataset_path 의 sha256 이 프리셋 선언과 다르다(선언 %s · 실측 %s) — 다른 바이트로 재지 않는다"
                          % (values["source_sha256"], got))
    return {"checked": "measured(sha256 일치 · %d bytes)" % p.stat().st_size}


def validate_preset(doc, bindings: dict) -> list:
    """닫힌 스키마 검사 → 위반 문자열 목록(빈 목록 = 통과)."""
    probs = []
    if not isinstance(doc, dict):
        return ["프리셋 최상위가 객체가 아니다"]
    keys = set(doc)
    if keys != set(TOP_KEYS):
        miss, extra = sorted(set(TOP_KEYS) - keys), sorted(keys - set(TOP_KEYS))
        probs.append("최상위 키 계약 위반(누락 %s · 미지 %s) — 값만 조정한다. 꺼 둔 블록·다른 도구 블록을 실제 키로 "
                     "남기지 않는다" % (miss, extra))
    if doc.get("schema_version") != SCHEMA_VERSION:
        probs.append("schema_version 은 %d 이어야 한다(지금 %r)" % (SCHEMA_VERSION, doc.get("schema_version")))
    if doc.get("bench") != FULL_BENCH:
        probs.append("bench 는 바인딩 키 '%s' 이어야 한다(지금 %r)" % (FULL_BENCH, doc.get("bench")))
    if doc.get("tool") not in KNOWN_TOOLS:
        probs.append("tool 은 %s 중 하나다(지금 %r)" % (list(KNOWN_TOOLS), doc.get("tool")))
    rc = doc.get("reason_codes")
    if not isinstance(rc, dict) or not rc:
        probs.append("reason_codes 는 비어 있지 않은 객체여야 한다")
        rc = {}
    for code, desc in rc.items():
        if not isinstance(code, str) or not re.fullmatch(r"[a-z][a-z0-9_]*", code):
            probs.append("근거 코드 이름 %r 는 [a-z][a-z0-9_]* 여야 한다" % (code,))
        if not isinstance(desc, str) or not desc.strip():
            probs.append("근거 코드 %r 의 설명이 비었다" % (code,))
    for need in (HUMAN_PIN_REASON, DEFAULT_REASON):
        if need not in rc:
            probs.append("reason_codes 에 필수 코드 %r 가 없다(해소기가 사람 핀·기본값에 붙인다)" % need)
    knobs = doc.get("knobs")
    if not isinstance(knobs, dict):
        probs.append("knobs 는 객체여야 한다")
        return probs
    if set(knobs) != set(KNOB_SPECS):
        probs.append("knobs 키 계약 위반(누락 %s · 미지 %s)" % (sorted(set(KNOB_SPECS) - set(knobs)),
                                                        sorted(set(knobs) - set(KNOB_SPECS))))
    for name, k in knobs.items():
        if name not in KNOB_SPECS:
            continue
        kind, typ, caps, no_default = KNOB_SPECS[name]
        where = "knobs.%s" % name
        if not isinstance(k, dict):
            probs.append("%s 는 객체여야 한다" % where)
            continue
        if set(k) != set(KNOB_KEYS):
            probs.append("%s 키 계약 위반(누락 %s · 미지 %s)" % (where, sorted(set(KNOB_KEYS) - set(k)),
                                                          sorted(set(k) - set(KNOB_KEYS))))
            continue
        if k["owner"] not in OWNERS:
            probs.append("%s.owner 는 %s 중 하나다" % (where, list(OWNERS)))
        if k["kind"] != kind:
            probs.append("%s.kind 는 %s 여야 한다(지금 %r)" % (where, kind, k["kind"]))
        if k["serving_cap"] not in caps:
            probs.append("%s.serving_cap 은 %s 중 하나다(지금 %r)" % (where, list(caps), k["serving_cap"]))
        opts = k["options"]
        grade_of = {}
        if not isinstance(opts, list) or not opts:
            probs.append("%s.options 는 비어 있지 않은 목록이어야 한다" % where)
            opts = []
        for i, o in enumerate(opts):
            if not isinstance(o, dict) or set(o) != set(OPTION_KEYS):
                probs.append("%s.options[%d] 는 정확히 {value, grade} 여야 한다" % (where, i))
                continue
            nullable = name in DATASET_KNOBS and o["value"] is None   # synthetic 짝의 '값 없음'
            if not nullable and not _typed(o["value"], typ):
                probs.append("%s.options[%d].value=%r 의 타입이 %s 가 아니다" % (where, i, o["value"], typ.__name__))
                continue
            if name in DATASET_KNOBS:
                bad = _dataset_shape_problem(name, o["value"])
                if bad:
                    probs.append("%s.options[%d].value %s" % (where, i, bad))
                    continue
            if isinstance(o["value"], dict):   # map 값은 해시 불가 — 비교 키는 정규화한 JSON 문자열
                o = dict(o, value=json.dumps(o["value"], sort_keys=True))
            if o["value"] in grade_of:
                probs.append("%s.options 에 값 %r 가 중복된다" % (where, o["value"]))
            if o["grade"] not in GRADES:
                probs.append("%s.options[%d].grade 는 %s 중 하나다" % (where, i, list(GRADES)))
            grade_of[o["value"]] = o["grade"]
        reasons = k["reasons"]
        if not isinstance(reasons, list) or not reasons:
            probs.append("%s.reasons 는 비어 있지 않은 목록이어야 한다" % where)
            reasons = []
        for r in reasons:
            if r not in rc:
                probs.append("%s.reasons 의 %r 가 reason_codes 에 정의돼 있지 않다(정의 자리는 하나)" % (where, r))
        if HUMAN_PIN_REASON not in reasons:
            probs.append("%s.reasons 에 %r 가 없다 — 사람 핀은 어느 knob 에나 열려 있다" % (where, HUMAN_PIN_REASON))
        d = k["default"]
        if no_default:
            if d is not None:
                probs.append("%s.default 는 null 이어야 한다(기본값 없음 결정 유지 — 값이 없으면 기동 거부)" % where)
        else:
            dvals = d if kind == "list" else [d]
            if kind == "list" and (not isinstance(d, list) or not d):
                probs.append("%s.default 는 비어 있지 않은 목록이어야 한다" % where)
                dvals = []
            if DEFAULT_REASON not in reasons:
                probs.append("%s.reasons 에 %r 가 없다(기본값이 있는 knob)" % (where, DEFAULT_REASON))
            dvals = [json.dumps(v, sort_keys=True) if isinstance(v, dict) else v for v in dvals]
            for v in dvals:
                if v not in grade_of:
                    probs.append("%s.default 의 %r 가 options 밖이다" % (where, v))
                elif grade_of[v] != "auto":
                    probs.append("%s.default 의 %r 는 auto 여야 한다(기본값은 누구나 열 수 있어야 한다)" % (where, v))
            if name == "levels" and VERDICT_LEVEL not in dvals:
                probs.append("%s.default 는 판정점 %d 을 포함해야 한다" % (where, VERDICT_LEVEL))
            if k["owner"] == "user":
                for v, g in grade_of.items():
                    if v not in dvals and g != "escalate":
                        probs.append("%s 는 owner: user 인데 비기본 값 %r 가 %s 다 — 사용자 knob 의 비기본 값은 "
                                     "escalate(사람 핀으로만)여야 한다" % (where, v, g))
        if name == "levels" and VERDICT_LEVEL not in grade_of:
            probs.append("%s.options 에 판정점 %d 이 없다" % (where, VERDICT_LEVEL))
        if name == "repeats":
            for v in grade_of:
                if _typed(v, int) and v < _ra.FULL_REPEATS_MIN:
                    probs.append("%s.options 의 %r 는 full 정의 하한 %d 미만이다(repeat_axis.FULL_REPEATS_MIN 교차검증)"
                                 % (where, v, _ra.FULL_REPEATS_MIN))
        if name == "backend" and k["owner"] != "agent":
            probs.append("%s.owner 는 agent 다(모델 계열에서 파생)" % where)
        if name in DATASET_KNOBS and k["owner"] != "user":
            probs.append("%s.owner 는 user 다(데이터셋은 측정 축 — 프리셋 작성자가 정한다)" % where)
    if isinstance(knobs, dict) and all(isinstance(knobs.get(n), dict) for n in DATASET_KNOBS):
        probs.extend("knobs 기본값 " + p for p in dataset_cross_problems({n: knobs[n].get("default") for n in DATASET_KNOBS}))
    return probs


def load_preset(name=None, skill_dir: Path = SKILL_DIR) -> "tuple[Path, dict, dict]":
    bindings = load_bindings(skill_dir)
    path = preset_path(name, skill_dir, bindings)
    doc = _load_yaml(path)
    probs = validate_preset(doc, bindings)
    if probs:
        raise PresetError("프리셋 %s 계약 위반 %d건:\n  - %s" % (path, len(probs), "\n  - ".join(probs)))
    return path, doc, bindings


# ── 서빙 상한 ──────────────────────────────────────────────────────────────────────────

def _grep_yaml_int(text: str, key: str):
    # sweep_bench 의 meta 판독(grep_yaml)과 같은 규칙: 'key: value' 줄 · '# 주석' 제거 · dash/underscore 양변형
    for k in (key, key.replace("-", "_")):
        m = re.search(r"(?m)^\s*%s\s*:\s*([^\n#]+)" % re.escape(k), text)
        if m:
            raw = m.group(1).strip().strip('"').strip("'")
            return int(raw) if re.fullmatch(r"[0-9]+", raw) else raw
    return None


def serving_caps(serving_yaml, dry_run: bool) -> dict:
    """explorer 서빙 yaml 에서 max-num-seqs·max-model-len 을 읽는다(상한 필터 입력 · 값을 만들지 않는다)."""
    if not serving_yaml or not Path(serving_yaml).is_file():
        if dry_run:
            return {"max_num_seqs": None, "max_model_len": None,
                    "source": "unavailable(dry-run · 서빙 yaml 부재: %s)" % serving_yaml}
        raise PresetError("서빙 yaml 이 없다(%s) — 상한 필터 없이 측정하지 않는다(explorer 산출 configs/<config>.yaml)"
                          % serving_yaml)
    text = Path(serving_yaml).read_text(encoding="utf-8", errors="replace")
    out = {"source": "declared(%s)" % serving_yaml}
    for key, k in (("max_num_seqs", "max-num-seqs"), ("max_model_len", "max-model-len")):
        v = _grep_yaml_int(text, k)
        if v is not None and not _typed(v, int):
            raise PresetError("서빙 yaml %s 의 %s=%r 를 정수로 읽지 못했다" % (serving_yaml, k, v))
        out[key] = v
    return out


# ── 해소 ──────────────────────────────────────────────────────────────────────────────

def _parse_cli_value(name: str, raw):
    kind, typ, _caps, _nd = KNOB_SPECS[name]
    if raw is None or str(raw).strip() == "":
        return None
    if kind == "list":
        vals = []
        for tok in str(raw).split(","):
            tok = tok.strip()
            if not tok:
                continue
            if not re.fullmatch(r"[0-9]+", tok):
                raise PresetError("%s 원소는 정수만: %r" % (FLAG_OF[name], tok))
            vals.append(int(tok))
        return vals
    if typ is int:
        if not re.fullmatch(r"[0-9]+", str(raw).strip()):
            raise PresetError("%s 는 정수다: %r" % (FLAG_OF[name], raw))
        return int(str(raw).strip())
    if typ is dict:
        try:
            v = json.loads(str(raw))
        except ValueError:
            raise PresetError("%s 는 JSON 객체다: %r" % (FLAG_OF[name], raw))
        if not isinstance(v, dict):
            raise PresetError("%s 는 JSON 객체다: %r" % (FLAG_OF[name], raw))
        return v
    return str(raw).strip()


def _norm(v):
    """선택지 비교 키 — map 값은 해시 불가라 정규화한 JSON 문자열로 비교한다(validate 와 같은 규칙)."""
    return json.dumps(v, sort_keys=True) if isinstance(v, dict) else v


def _parse_knob_reasons(items) -> dict:
    out = {}
    for it in items or []:
        if "=" not in it:
            raise PresetError("--knob-reason 은 KNOB=CODE 모양이다: %r" % it)
        k, c = (s.strip() for s in it.split("=", 1))
        if k not in KNOB_SPECS:
            raise PresetError("--knob-reason 의 knob %r 를 모른다(%s)" % (k, sorted(KNOB_SPECS)))
        if k in out and out[k] != c:
            raise PresetError("--knob-reason %s 가 두 번 다른 값으로 왔다" % k)
        out[k] = c
    return out


def _campaign_pins(campaign_id, repo_root) -> "tuple[str | None, dict, str]":
    camp, budgets, why = _ra.load_campaign_budgets(campaign_id, repo_root=repo_root)
    if budgets is None:
        return None, {}, why
    pins = budgets.get("bench_pins", {})
    if not isinstance(pins, dict):
        raise PresetError("campaigns/%s/campaign.yaml budgets.bench_pins 가 객체가 아니다" % camp)
    for k, v in pins.items():
        if k == "repeats":
            raise PresetError("budgets.bench_pins.repeats 는 두 번째 자리다 — 반복 수 핀은 budgets.repeats 하나다")
        if k not in KNOB_SPECS:
            raise PresetError("budgets.bench_pins 의 %r 를 모른다(%s)" % (k, sorted(KNOB_SPECS)))
        kind, typ, _c, _n = KNOB_SPECS[k]
        ok = (isinstance(v, list) and v and all(_typed(x, int) for x in v)) if kind == "list" else _typed(v, typ)
        if not ok:
            raise PresetError("budgets.bench_pins.%s=%r 의 모양이 knob 과 다르다" % (k, v))
    return camp, pins, ""


def _parse_pins(items) -> dict:
    out = {}
    for it in items or []:
        if "=" not in it:
            raise PresetError("--pin 은 KNOB=VALUE 모양이다: %r" % it)
        k, raw = (s.strip() for s in it.split("=", 1))
        if k not in KNOB_SPECS:
            raise PresetError("--pin 의 knob %r 를 모른다(%s)" % (k, sorted(KNOB_SPECS)))
        if k in out:
            raise PresetError("--pin %s 가 두 번 왔다" % k)
        out[k] = raw
    return out


def resolve(preset_doc: dict, preset_name: str, *, tool: str, cli: dict, cli_pins=None, pinned_by=None,
            knob_reasons=None, approved_by=None, campaign_id=None, serving: dict,
            repo_root: Path = _ra.REPO_ROOT, repeats_source=None, dry_run: bool = False) -> "tuple[dict, dict]":
    """→ (해소값, 기록 블록). 위반은 PresetError(기동 거부).

    cli      스윕 플래그 값(--levels 등) = **Agent 선택**(근거 코드 --knob-reason)
    cli_pins --pin KNOB=VALUE = **사람 핀**(귀속 --pinned-by 필수) — CLI 는 Agent 와 사람이 함께 쓰는 입구라
             귀속을 knob 단위로 밝힌다. 같은 knob 에 Agent 값이 함께 오면 같을 때만 통과한다.
    """
    if tool not in KNOWN_TOOLS:
        raise PresetError("--tool 은 %s 중 하나다: %r" % (list(KNOWN_TOOLS), tool))
    knob_reasons = dict(knob_reasons or {})
    cli_pins = dict(cli_pins or {})
    if cli_pins and not (pinned_by and str(pinned_by).strip()):
        raise PresetError("--pin 은 --pinned-by <사람 발화 전사> 와 함께만 쓴다(귀속 없는 핀은 Agent 선택이다)")
    if pinned_by and not cli_pins:
        raise PresetError("--pinned-by 는 --pin KNOB=VALUE 와 함께 쓴다(무엇을 핀했는지 없다)")
    knobs = preset_doc["knobs"]
    camp, pins, camp_why = _campaign_pins(campaign_id, repo_root)
    values, record = {}, {}
    for name, spec in knobs.items():
        kind = spec["kind"]
        grade_of = {_norm(o["value"]): o["grade"] for o in spec["options"]}
        cli_v = _parse_cli_value(name, cli.get(name))
        cpin_v = _parse_cli_value(name, cli_pins.get(name)) if name in cli_pins else None
        if cpin_v is not None and cli_v is not None and cli_v != cpin_v:
            raise PresetError("%s=%r(Agent) 와 --pin %s=%r(사람)가 다르다 — 사람 핀을 조용히 덮지 않는다"
                              % (FLAG_OF[name], cli_v, name, cpin_v))
        rec = {"owner": spec["owner"]}
        if name == "bench_budget_mib" and tool != "guidellm" and cli_v is None and cpin_v is None and name not in pins:
            rec.update({"value": None, "source": "not_applicable", "grade": None, "reason": None,
                        "note": "--tool %s 는 벤치 컨테이너를 띄우지 않는다(예산은 guidellm 전용)" % tool})
            record[name] = rec
            values[name] = None
            continue
        if name == "repeats":
            attributed = bool(repeats_source or cpin_v is not None)
            r_val = cpin_v if cpin_v is not None else cli_v
            r_src_text = repeats_source or (("pinned-by: %s" % pinned_by) if cpin_v is not None else None)
            try:
                n, rsrc = _ra.resolve_repeats(r_val, argv_source=r_src_text, campaign_id=campaign_id,
                                              repo_root=repo_root, preset_default=spec["default"],
                                              preset_source=preset_name)
            except _ra.RepeatAxisError as exc:
                raise PresetError("반복 수: %s" % exc)
            if rsrc.startswith("declared(argv"):
                source = "cli_pin" if attributed else "agent"
            elif rsrc.startswith("declared(campaigns"):
                source = "campaign"
            else:
                source = "preset"
            val = n
            rec["repeats_source"] = rsrc
        else:
            pin_v = pins.get(name)
            if cpin_v is not None:
                source, val = "cli_pin", cpin_v
                if pin_v is not None and pin_v != cpin_v:
                    rec["overrode_campaign_pin"] = pin_v
            elif pin_v is not None:
                if cli_v is not None and cli_v != pin_v:
                    raise PresetError("%s: Agent 선택 %r 가 사람 핀(campaigns/%s budgets.bench_pins.%s=%r)과 다르다 — "
                                      "조용히 덮어쓰지 않는다(사람 핀이면 --pinned-by 로 귀속을 밝혀라)"
                                      % (FLAG_OF[name], cli_v, camp, name, pin_v))
                source, val = "campaign", pin_v
            elif cli_v is not None:
                source, val = "agent", cli_v
            elif spec["default"] is not None or name in DATASET_KNOBS:
                source, val = "preset", spec["default"]   # 데이터셋 knob 의 null = synthetic 짝(값 없음이 정상값)
            else:
                raise PresetError("%s 는 필수다 — 프리셋 %s 에 기본값이 없다(기본값 없음 결정). 선택지: %s"
                                  % (FLAG_OF[name], preset_name, [o["value"] for o in spec["options"]]))
        vals = list(val) if kind == "list" else [val]
        if name == "levels" and VERDICT_LEVEL not in vals:
            vals = [VERDICT_LEVEL] + vals
            rec["verdict_point_added"] = True
        if kind == "list":
            vals = sorted(dict.fromkeys(vals))
        outside = [v for v in vals if _norm(v) not in grade_of]
        if outside:
            raise PresetError("%s=%s 가 프리셋 %s 의 선택지 밖이다(선택지 %s) — 선택지를 늘리려면 프리셋을 고친다(리뷰 대상)"
                              % (FLAG_OF[name], outside, preset_name, [o["value"] for o in spec["options"]]))
        grades = {_norm(v): grade_of[_norm(v)] for v in vals}
        # 근거 코드
        if source in ("cli_pin", "campaign"):
            reason = HUMAN_PIN_REASON
        elif source == "preset":
            reason = DEFAULT_REASON
        else:
            given = knob_reasons.get(name)
            is_default = (spec["default"] is not None and
                          (vals == sorted(dict.fromkeys([VERDICT_LEVEL] + list(spec["default"])))
                           if kind == "list" else _norm(vals[0]) == _norm(spec["default"])))
            if given is None:
                if not is_default:
                    raise PresetError("%s=%s 는 Agent 선택인데 근거 코드가 없다 — --knob-reason %s=<%s>"
                                      % (FLAG_OF[name], vals if kind == "list" else vals[0], name,
                                         "|".join(r for r in spec["reasons"] if r != HUMAN_PIN_REASON)))
                given = DEFAULT_REASON
            if given not in spec["reasons"] or given == HUMAN_PIN_REASON:
                raise PresetError("--knob-reason %s=%s 는 이 knob 의 Agent 근거 코드가 아니다(허용 %s)"
                                  % (name, given, [r for r in spec["reasons"] if r != HUMAN_PIN_REASON]))
            reason = given
            # 등급: Agent 는 auto 를 연다 · confirm 은 사전 승인 · escalate 는 못 연다
            esc = [v for v, g in grades.items() if g == "escalate"]
            if esc:
                raise PresetError("%s=%s 는 escalate 등급이다 — 사람 핀(--pinned-by · campaign.yaml budgets.bench_pins)"
                                  "으로만 열린다%s" % (FLAG_OF[name], esc,
                                                   " (owner: user knob)" if spec["owner"] == "user" else ""))
            conf = [v for v, g in grades.items() if g == "confirm"]
            if conf and not approved_by:
                raise PresetError("%s=%s 는 confirm 등급이다 — 셀별 사전 승인(사람 발화 전사 --approved-by)이 있을 때만 "
                                  "Agent 가 고른다" % (FLAG_OF[name], conf))
            if conf:
                rec["approved_by"] = approved_by
        # 서빙 상한 필터
        filtered = []
        cap = spec["serving_cap"]
        if cap == CAP_LE_MAX_NUM_SEQS:
            lim = serving.get("max_num_seqs")
            if lim is None:
                rec["serving_cap_applied"] = "not_applied(서빙 yaml 에 max-num-seqs 선언 없음 · %s)" % serving.get("source")
            else:
                keep = [v for v in vals if v <= lim or v == VERDICT_LEVEL]
                filtered = [{"value": v, "reason": "> max-num-seqs %d (%s)" % (lim, serving.get("source"))}
                            for v in vals if v not in keep]
                vals = keep
                rec["serving_cap_applied"] = "le_max_num_seqs(%d)" % lim
                if not vals:
                    raise PresetError("%s: 상한 필터 뒤 남은 레벨이 없다" % FLAG_OF[name])
        rec.update({"value": vals if kind == "list" else vals[0], "source": source,
                    "grade": ({str(v): grades[v] for v in vals} if kind == "list" else grades[_norm(vals[0])]),
                    "reason": reason})
        if filtered:
            rec["filtered_out"] = filtered
        record[name] = rec
        values[name] = vals if kind == "list" else vals[0]
    # input_len 상한(두 knob 이 정해진 뒤)
    il = knobs["input_len"]
    if il["serving_cap"] == CAP_INPUT_PLUS_OUTPUT:
        mml = serving.get("max_model_len")
        if mml is None:
            record["input_len"]["serving_cap_applied"] = ("not_applied(서빙 yaml 에 max-model-len 선언 없음 · %s)"
                                                          % serving.get("source"))
        else:
            need = values["input_len"] + values["output_len"]
            if need > mml:
                raise PresetError("--input-len %d + --output-len %d = %d > max-model-len %d (%s) — 서빙 context window "
                                  "밖은 측정하지 않는다" % (values["input_len"], values["output_len"], need, mml,
                                                    serving.get("source")))
            record["input_len"]["serving_cap_applied"] = "input_plus_output_le_max_model_len(%d)" % mml
    # 데이터셋 축(plan_26101614) — 교차 규칙 · 도구 정합 · 측정 시점 원본 검사
    dset = {k: values[k] for k in DATASET_KNOBS}
    cross = dataset_cross_problems(dset)
    if cross:
        raise PresetError("데이터셋 knob 해소값 " + "; ".join(cross))
    if tool != "guidellm" and dset["dataset_kind"] != DATASET_SYNTHETIC:
        raise PresetError("--tool %s 는 데이터셋 knob 을 받지 않는다(dataset_kind=%r) — 실제 데이터셋 입력은 full-bench "
                          "(--tool guidellm) 전용이다. 조용히 synthetic 으로 재지 않는다" % (tool, dset["dataset_kind"]))
    dset_check = ({"checked": "skipped(dry-run · 측정 시점에 원본 실재·sha256 을 본다)"} if dry_run
                  else check_dataset_runtime(dset))
    stray = sorted(set(knob_reasons) - {k for k, r in record.items() if r.get("source") == "agent"})
    if stray:
        raise PresetError("--knob-reason %s 의 knob 은 Agent 선택이 아니다(사람 핀·기본값 · 근거 코드가 붙지 않는다)" % stray)
    block = {
        "bench": FULL_BENCH, "measured_tool": tool, "preset_tool": preset_doc["tool"],
        "preset": preset_name, "campaign": camp, "campaign_pins_note": camp_why or None,
        "pinned_by": pinned_by, "approved_by": approved_by,
        "serving_cap": serving, "knobs": record,
        "dataset": dict(dset, runtime_check=dset_check),
        "resolution_order": "cli_pin > campaign > agent > preset",
    }
    return values, block


# ── CLI ──────────────────────────────────────────────────────────────────────────────

def _cmd_resolve(a) -> int:
    path, doc, bindings = load_preset(a.preset)
    name = a.preset or DEFAULT_PRESET
    cli = {k: getattr(a, k, None) for k in KNOB_SPECS}
    serving = serving_caps(a.serving_yaml, a.dry_run)
    values, block = resolve(doc, name, tool=a.tool, cli=cli, cli_pins=_parse_pins(a.pin), pinned_by=a.pinned_by,
                            knob_reasons=_parse_knob_reasons(a.knob_reason), approved_by=a.approved_by,
                            campaign_id=a.campaign_id, serving=serving, repeats_source=a.repeats_source,
                            dry_run=a.dry_run)
    try:
        rel = str(path.relative_to(_ra.REPO_ROOT))
    except ValueError:
        rel = str(path)
    block["preset_path"] = rel
    block["binding"] = "%s → config/%s/ (%s)" % (FULL_BENCH, bindings[FULL_BENCH], BINDINGS_REL)
    Path(a.record_out).parent.mkdir(parents=True, exist_ok=True)
    Path(a.record_out).write_text(json.dumps(block, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    for k, sk in SHELL_KEY_OF.items():
        v = values[k]
        v = ",".join(str(x) for x in v) if isinstance(v, list) else ("" if v is None else str(v))
        print("%s\x1f%s" % (sk, v))
    print("REPEATS_SOURCE\x1f%s" % block["knobs"]["repeats"]["repeats_source"].replace("\x1f", " "))
    print("REPEAT_KIND\x1f%s" % _ra.REPEAT_KIND_WARM)
    print("PRESET_PATH\x1f%s" % rel)
    print("DATASET_KIND\x1f%s" % values["dataset_kind"])
    return 0


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="full-bench knob 프리셋 로더·해소기")
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    v = sub.add_parser("validate", help="프리셋 스키마 검증")
    v.add_argument("--preset")
    dflt = sub.add_parser("default", help="knob 하나의 프리셋 기본값 → 'VALUE<US>PRESET' (검증 통과 뒤)")
    dflt.add_argument("--knob", required=True); dflt.add_argument("--preset")
    r = sub.add_parser("resolve", help="knob 해소 → KEY<US>VALUE 줄 + 기록 JSON")
    r.add_argument("--tool", required=True)
    r.add_argument("--preset")
    r.add_argument("--serving-yaml")
    r.add_argument("--dry-run", action="store_true")
    r.add_argument("--record-out", required=True)
    r.add_argument("--levels"); r.add_argument("--input-len", dest="input_len")
    r.add_argument("--output-len", dest="output_len"); r.add_argument("--num-prompts", dest="num_prompts")
    r.add_argument("--warmups"); r.add_argument("--repeats"); r.add_argument("--repeats-source")
    r.add_argument("--backend"); r.add_argument("--bench-budget-mib", dest="bench_budget_mib")
    r.add_argument("--pin", action="append", default=[], help="사람 핀 KNOB=VALUE(--pinned-by 필수)")
    r.add_argument("--pinned-by"); r.add_argument("--knob-reason", action="append", default=[])
    r.add_argument("--approved-by"); r.add_argument("--campaign-id")
    a = ap.parse_args(argv)
    if a.self_test:
        import selftest_bench_preset
        return selftest_bench_preset.run()
    try:
        if a.cmd == "validate":
            path, _doc, _b = load_preset(a.preset)
            print("[bench_preset] OK — %s" % path)
            return 0
        if a.cmd == "default":
            _p, doc, _b = load_preset(a.preset)
            if a.knob not in doc["knobs"]:
                raise PresetError("knob %r 를 모른다(%s)" % (a.knob, sorted(doc["knobs"])))
            d = doc["knobs"][a.knob]["default"]
            if d is None:
                raise PresetError("knob %r 는 기본값이 없다(기본값 없음 결정)" % a.knob)
            print("%s\x1f%s" % (",".join(map(str, d)) if isinstance(d, list) else d, a.preset or DEFAULT_PRESET))
            return 0
        if a.cmd == "resolve":
            if a.pinned_by is not None and not a.pinned_by.strip():
                raise PresetError("--pinned-by 는 사람 발화의 전사다 — 빈 값은 귀속이 아니다")
            if a.approved_by is not None and not a.approved_by.strip():
                raise PresetError("--approved-by 는 사람 발화의 전사다 — 빈 값은 승인이 아니다")
            return _cmd_resolve(a)
    except PresetError as exc:
        sys.stderr.write("[bench_preset] ERROR %s\n" % exc)
        return 2
    ap.print_help()
    return 2


if __name__ == "__main__":
    sys.exit(main())
