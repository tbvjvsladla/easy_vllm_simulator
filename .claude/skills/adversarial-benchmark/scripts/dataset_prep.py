#!/usr/bin/env python3
# dataset_prep.py — full-bench 데이터셋 축의 전처리·런별 행 선택 (plan_26101614 · interview_20261016_045736)
#
# 무엇을 하나(run_bench.sh 의 guidellm 가지가 부른다 · 입력 = bench_preset resolve 가 낸 데이터셋 해소값)
#   prepare  같은 고정 GuideLLM 이미지로 **네트워크를 끈** 컨테이너를 따로 띄워 `guidellm preprocess dataset` 을 돌린다.
#            원본 :ro · 토크나이저 = run_bench 가 스테이징한 그 모델의 사본(/tok) · 결과는 스윕 캐시 폴더.
#            캐시 키(원본 sha256 · 토크나이저 파일 digest · 길이 · 전략 · seed · 열 · 이미지)가 같으면 다시 돌리지 않는다.
#   slice    런 번호(스윕 안에서 레벨·반복을 가로지르는 0-기반 순번)마다 **겹치지 않는 연속 행**을 잘라 쓴다.
#            행이 모자라면 멈추지 않고 seed 고정 랜덤 중복 샘플링을 하고 `duplicate_sampling: true` 를 남긴다(사용자 결정 U11).
#            사이드카 dataset_<config>.json 에 그 런이 **실제로 쓴** 출처를 적는다 — sweep_bench 가 인증서 meta 로 승계한다.
#   synthetic-sidecar  합성 입력도 출처를 남긴다(dataset_kind: synthetic · 신규 발행분은 항상 기록 · U10).
#
# 판정은 건드리지 않는다. 길이는 전처리가 input_len 에 맞추고 출력은 ignore_eos 로 output_len 에 고정한다.
# 전처리 결과 파일은 원본+토크나이저+seed 로 재생성되므로 digest 를 두지 않는다(policy:GIT_SINGLE_AUTHORITY 2문항 Q2).
# 종료: 0 성공 · 2 입력 거부 · 3 전처리 실패(fail-closed — 합성으로 대체하지 않는다)
from __future__ import annotations

import argparse
import hashlib
import json
import os
import random
import subprocess
import sys
from pathlib import Path

PREP_NAME = "prepared.jsonl"
PREP_META = "prepared_meta.json"
# 전처리 결과의 열 — guidellm preprocess 가 붙인다(v0.7.3 실측 2026-10-16 spike: article·highlights·id·prompt_tokens_count·output_tokens_count).
PROMPT_COUNT_COL = "prompt_tokens_count"
OUTPUT_COUNT_COL = "output_tokens_count"


def _die(rc: int, msg: str):
    sys.stderr.write("[dataset_prep] %s\n" % msg)
    sys.exit(rc)


def _load_spec(path: str) -> dict:
    try:
        spec = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        _die(2, "데이터셋 해소값(%s)을 읽지 못했다: %s" % (path, exc))
    if not isinstance(spec, dict) or "dataset_kind" not in spec:
        _die(2, "데이터셋 해소값에 dataset_kind 가 없다: %s" % path)
    return spec


def _dir_digest(d: Path) -> str:
    h = hashlib.sha256()
    for p in sorted(d.iterdir()):
        if p.is_file():
            h.update(p.name.encode())
            h.update(hashlib.sha256(p.read_bytes()).digest())
    return h.hexdigest()


def column_mapper_arg(spec: dict, with_counts: bool) -> str:
    """GuideLLM generative_column_mapper 인자. ★ column_mappings 를 주면 **기본 매핑 전체가 교체된다**
    (v0.7.3 `datasets_mappings`) — 실행 시점에 출력 길이 열을 빠뜨리면 max_tokens 가 사라지고 ignore_eos 가
    컨텍스트 끝까지 생성한다. 그래서 측정용(with_counts)은 세 열을 전부 명시한다."""
    m = {"text_column": spec["column_map"]["text_column"]}
    if with_counts:
        m["prompt_tokens_count_column"] = PROMPT_COUNT_COL
        m["output_tokens_count_column"] = OUTPUT_COUNT_COL
    # ★ JSON 객체 한 덩이로 준다 — `kind=..,column_mappings={..}` 꼴은 CLI 가 쉼표에서 쪼개 다중 키 dict 가 깨진다
    #   (2026-10-16 실측 · v0.7.3 click_pydantic 은 key=value 쌍 **또는** JSON/YAML 객체를 받는다).
    return json.dumps({"kind": "generative_column_mapper", "column_mappings": m}, separators=(",", ":"))


def data_arg(kind: str, path: str) -> str:
    """`--data` 인자 — 같은 이유로 JSON 객체. 단일 파일을 Dataset 으로 읽게 split=train 을 준다(DatasetDict 회피)."""
    return json.dumps({"kind": kind, "path": path, "load_kwargs": {"split": "train"}}, separators=(",", ":"))


def cmd_prepare(a) -> int:
    spec = _load_spec(a.spec)
    if spec["dataset_kind"] == "synthetic":
        _die(2, "synthetic 은 전처리가 없다(호출부 결함)")
    raw = Path(spec["dataset_path"])
    tok = Path(a.tokstage)
    cache = Path(a.cache)
    if not raw.is_file():
        _die(2, "원본이 없다: %s" % raw)
    if not tok.is_dir():
        _die(2, "토크나이저 스테이징 폴더가 없다: %s" % tok)
    key = {
        "source_sha256": spec["source_sha256"], "dataset_kind": spec["dataset_kind"],
        "column_map": spec["column_map"], "preprocess_strategy": spec["preprocess_strategy"], "seed": spec["seed"],
        "input_len": int(a.input_len), "output_len": int(a.output_len),
        "tokenizer_digest": _dir_digest(tok), "image": a.image,
    }
    meta_p, out_p = cache / PREP_META, cache / PREP_NAME
    if meta_p.is_file() and out_p.is_file():
        try:
            if json.loads(meta_p.read_text(encoding="utf-8")).get("key") == key:
                print("[dataset_prep] 캐시 재사용 — %s" % out_p)
                return 0
        except ValueError:
            pass
    cache.mkdir(parents=True, exist_ok=True)
    for p in (meta_p, out_p):
        p.unlink(missing_ok=True)
    name = "guidellm-preprocess-%d" % os.getpid()
    cmd = ["docker", "run", "--rm", "--name", name, "--network", "none",
           "--user", "%d:%d" % (os.getuid(), os.getgid()),
           "--memory", "%sm" % a.budget_mib, "--memory-swap", "%sm" % a.budget_mib,
           "-e", "HOME=/tmp", "-e", "XDG_CACHE_HOME=/tmp/.cache", "-e", "HF_HUB_OFFLINE=1", "-e", "TRANSFORMERS_OFFLINE=1",
           "-v", "%s:/data:ro" % raw.parent, "-v", "%s:/tok:ro" % tok, "-v", "%s:/out" % cache,
           "--entrypoint", "guidellm", a.image, "preprocess", "dataset",
           data_arg(spec["dataset_kind"], "/data/%s" % raw.name),
           "/out/%s" % PREP_NAME,
           "--tokenizer", "kind=huggingface_auto,model=/tok",
           "--strategy", "kind=%s,prompt_tokens=%d,output_tokens=%d" % (spec["preprocess_strategy"], int(a.input_len),
                                                                      int(a.output_len)),
           "--data-column-mapper", column_mapper_arg(spec, with_counts=False),
           "--seed", "kind=static,value=%d" % int(spec["seed"])]
    print("[dataset_prep] 전처리(오프라인 · %s · budget %sMiB) → %s" % (a.image, a.budget_mib, out_p))
    try:
        rc = subprocess.run(cmd, timeout=int(a.timeout_s)).returncode
    except subprocess.TimeoutExpired:
        subprocess.run(["docker", "rm", "-f", name], capture_output=True)
        _die(3, "전처리가 %ss 안에 끝나지 않았다 — 측정을 시작하지 않는다" % a.timeout_s)
    finally:
        subprocess.run(["docker", "rm", "-f", name], capture_output=True)
    if rc != 0 or not out_p.is_file():
        _die(3, "guidellm preprocess 실패(rc=%s) — 합성 입력으로 대체하지 않는다" % rc)
    rows = 0
    exact = 0
    with out_p.open(encoding="utf-8") as f:
        for line in f:
            if not line.strip():
                continue
            rows += 1
            if json.loads(line).get(PROMPT_COUNT_COL) == int(a.input_len):
                exact += 1
    if rows == 0:
        _die(3, "전처리 결과가 비었다: %s" % out_p)
    meta = {"key": key, "rows": rows, "rows_exact_input_len": exact,
            "provenance": "measured(guidellm preprocess dataset · 오프라인 컨테이너)"}
    meta_p.write_text(json.dumps(meta, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print("[dataset_prep] 전처리 완료 — 행 %d (input_len 정확 %d)" % (rows, exact))
    return 0


def cmd_slice(a) -> int:
    spec = _load_spec(a.spec)
    cache = Path(a.cache)
    meta = json.loads((cache / PREP_META).read_text(encoding="utf-8"))
    # 전략 목표 길이의 행만 쓴다 — concatenate 의 마지막 남은 조각(짧은 행)이 길이 고정을 깨지 않게.
    #   ★ 2026-10-16(P7 라이브 발견): 목표를 `== input_len` 으로 비교하면 특수 토큰(BOS)을 세는 토크나이저
    #   (minicpm: 본문 1024 + BOS = 1025)에서 6,265 행 중 11 행만 남아 같은 11 프롬프트가 반복됐다(prefix cache 오염).
    #   목표는 **분포의 최빈값**이다(전략이 거의 모든 행을 그 길이로 만든다) — 토크나이저마다 다른 상수를 적지 않는다.
    rows = []
    with (cache / PREP_NAME).open(encoding="utf-8") as f:
        for line in f:
            if line.strip():
                rows.append((json.loads(line).get(PROMPT_COUNT_COL), line if line.endswith("\n") else line + "\n"))
    target, pool = target_rows(rows, int(a.input_len))
    n, idx = int(a.count), int(a.run_index)
    if n <= 0 or idx < 0:
        _die(2, "--count 는 양수 · --run-index 는 0 이상이다")
    if not pool:
        _die(3, "input_len=%s 목표 길이 행이 없다" % a.input_len)
    start = idx * n
    # 스윕 오프셋(2026-10-17 · plan_26101622 F2 후속): 행 선택이 스윕마다 같으면(첫 런 = 늘 행 0..n-1) 같은 config 를 prefix cache 가
    #   비기 전에 다시 잴 때 같은 프롬프트가 적중한다(실측: granite 재측정 level 1 run 1 적중률 0.9856 · TTFT 95 vs 263 ms). 시작점을
    #   스윕 salt(sweep_bench 의 스윕 시작 UTC)에서 결정론으로 정한다 — 같은 salt 면 같은 행(재현) · 스윕 안 런끼리는 여전히 겹치지 않는다.
    #   salt 부재 = 오프셋 0(종전 거동 그대로).
    base = (int(hashlib.sha256(a.sweep_salt.encode("utf-8")).hexdigest()[:16], 16) % len(pool)) if a.sweep_salt else 0
    if start + n <= len(pool):
        ids = [(base + start + j) % len(pool) for j in range(n)]
        chosen, dup = [pool[i] for i in ids], False
        how = ("contiguous(rows %d..%d · 다른 런과 겹치지 않음)" % (start, start + n - 1) if not base else
               "contiguous(offset %d + rows %d..%d mod %d · 스윕 salt %s · 다른 런과 겹치지 않음)"
               % (base, start, start + n - 1, len(pool), a.sweep_salt))
    else:
        rng = random.Random("%s:%d" % (spec["seed"], idx))
        chosen = [pool[rng.randrange(len(pool))] for _ in range(n)]
        dup, how = True, "random_with_replacement(seed=%s:%d · 행 %d < 필요 %d)" % (spec["seed"], idx, len(pool), start + n)
    Path(a.out).write_text("".join(chosen), encoding="utf-8")
    side = {k: spec.get(k) for k in ("dataset_kind", "dataset_source", "dataset_id", "dataset_path", "dataset_revision",
                                     "source_sha256", "column_map", "preprocess_strategy", "seed")}
    side.update({"input_len": int(a.input_len), "output_len": int(a.output_len), "run_index": idx, "rows_used": n,
                 "rows_available": len(pool), "rows_prepared": meta.get("rows"), "selection": how,
                 "prompt_tokens_count_target": target, "sweep_salt": a.sweep_salt or None, "sweep_offset": base,
                 "duplicate_sampling": dup, "tokenizer": a.tokenizer_label,
                 "tokenizer_digest": meta["key"]["tokenizer_digest"],
                 "provenance": "measured(run_bench 가 이 런에 넘긴 행 · dataset_prep slice)"})
    Path(a.sidecar).write_text(json.dumps(side, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print("[dataset_prep] 런 %d: %d 행 · %s" % (idx, n, how))
    return 0


def target_rows(rows: list, input_len: int) -> "tuple[int | None, list]":
    """(prompt_tokens_count, line) 목록 → (목표 길이, 그 길이의 행). 목표 = 최빈값. 최빈값이 input_len 에서
    특수 토큰 몇 개 이상 벗어나면(±8) 전처리가 목표를 못 맞춘 것이라 빈 목록을 돌려 호출부가 fail-closed 한다."""
    counts = {}
    for c, _ in rows:
        if isinstance(c, int):
            counts[c] = counts.get(c, 0) + 1
    if not counts:
        return None, []
    target = max(sorted(counts), key=lambda c: counts[c])
    if abs(target - input_len) > 8:
        return target, []
    return target, [ln for c, ln in rows if c == target]


def _self_test() -> int:
    fails = []

    def ck(label, cond):
        print("  [%s] %s" % ("PASS" if cond else "FAIL", label))
        if not cond:
            fails.append(label)

    t, p = target_rows([(1025, "a")] * 5 + [(1024, "b")] * 2 + [(300, "c")], 1024)
    ck("T1 BOS 를 세는 토크나이저(1025 최빈) → 목표 1025 · 5 행", t == 1025 and len(p) == 5)
    t, p = target_rows([(1024, "a")] * 4 + [(1022, "z")], 1024)
    ck("T2 최빈 1024 → 꼬리 조각(1022) 제외", t == 1024 and p == ["a"] * 4)
    t, p = target_rows([(512, "a")] * 4, 1024)
    ck("T3 목표가 input_len 에서 크게 벗어나면 빈 목록(호출부 fail-closed)", p == [])
    ck("T4 데이터·매퍼 인자는 JSON 객체(쉼표 분할 회피)",
       json.loads(column_mapper_arg({"column_map": {"text_column": "article"}}, True))["column_mappings"]
       == {"text_column": "article", "prompt_tokens_count_column": PROMPT_COUNT_COL,
           "output_tokens_count_column": OUTPUT_COUNT_COL}
       and json.loads(data_arg("json_file", "/out/x.jsonl"))["load_kwargs"] == {"split": "train"})
    # T5~T7 스윕 오프셋(2026-10-17 · plan_26101622 F2 후속)
    import tempfile, argparse as _ap, contextlib, io
    with tempfile.TemporaryDirectory() as td:
        tdp = Path(td)
        (tdp / "spec.json").write_text(json.dumps({"dataset_kind": "parquet_file", "seed": 42}), encoding="utf-8")
        (tdp / PREP_META).write_text(json.dumps({"rows": 100, "key": {"tokenizer_digest": "x"}}), encoding="utf-8")
        (tdp / PREP_NAME).write_text("".join(json.dumps({PROMPT_COUNT_COL: 1024, "id": i}) + "\n" for i in range(100)),
                                     encoding="utf-8")

        def sl(idx, salt):
            ns = _ap.Namespace(spec=str(tdp / "spec.json"), cache=td, run_index=idx, count=10, input_len=1024,
                               output_len=256, out=str(tdp / "o.jsonl"), sidecar=str(tdp / "s.json"),
                               tokenizer_label="t", sweep_salt=salt)
            with contextlib.redirect_stdout(io.StringIO()):
                cmd_slice(ns)
            return [json.loads(x)["id"] for x in (tdp / "o.jsonl").read_text(encoding="utf-8").splitlines()]
        ck("T5 salt 부재 = 종전 거동(런 0 = 행 0..9)", sl(0, "") == list(range(10)))
        a0, a1 = sl(0, "2026-10-17T00:00:00Z"), sl(1, "2026-10-17T00:00:00Z")
        ck("T6 같은 salt 의 런끼리 겹치지 않고 재현된다", not set(a0) & set(a1) and a0 == sl(0, "2026-10-17T00:00:00Z"))
        ck("T7 다른 salt(다른 스윕)는 다른 행에서 시작한다(prefix cache 재적중 회피)", a0 != sl(0, "2026-10-17T01:00:00Z"))
    if fails:
        sys.stderr.write("[dataset_prep] self-test FAIL %s\n" % fails)
        return 1
    print("[dataset_prep] self-test PASS — T1~T7")
    return 0


def cmd_synthetic_sidecar(a) -> int:
    side = {"dataset_kind": "synthetic", "input_len": int(a.input_len), "output_len": int(a.output_len),
            "duplicate_sampling": None,
            "provenance": "declared(GuideLLM synthetic_text · 원본 없음)"}
    Path(a.sidecar).write_text(json.dumps(side, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return 0


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="full-bench 데이터셋 전처리·런별 행 선택")
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("prepare")
    for k in ("--spec", "--tokstage", "--cache", "--image", "--budget-mib", "--input-len", "--output-len"):
        p.add_argument(k, required=True)
    p.add_argument("--timeout-s", default="1800")
    s = sub.add_parser("slice")
    for k in ("--spec", "--cache", "--run-index", "--count", "--input-len", "--output-len", "--out", "--sidecar",
              "--tokenizer-label"):
        s.add_argument(k, required=True)
    s.add_argument("--sweep-salt", default="", help="스윕 시작 UTC(sweep_bench) — 행 시작 오프셋의 결정론 입력 · 부재 = 오프셋 0")
    y = sub.add_parser("synthetic-sidecar")
    for k in ("--input-len", "--output-len", "--sidecar"):
        y.add_argument(k, required=True)
    if argv is None and sys.argv[1:] == ["--self-test"]:
        return _self_test()
    a = ap.parse_args(argv)
    return {"prepare": cmd_prepare, "slice": cmd_slice, "synthetic-sidecar": cmd_synthetic_sidecar}[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main())
