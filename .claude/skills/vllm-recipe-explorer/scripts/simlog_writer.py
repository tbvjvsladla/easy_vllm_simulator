#!/usr/bin/env python3
"""simlog_writer.py — vllm-recipe-explorer Phase 2 시뮬레이션 로그 라이터.

CONTRACT(FROZEN) 준수:
- `new_run_dir(repo_root, run_id)` : docs/simlog/<run_id>/ 생성 후 경로 반환.
- `write_trial(run_dir, trial_number, vllm_log_text, profile_dict, candidate_dict, smoke_dict)`
  : trialNN_vllm.log / _profile.json / _candidate.yaml / _smoke.json 기록.
- `append_correction(run_dir, record_dict)` : correction_history.jsonl 한 줄 append.
- `write_summary(run_dir, summary_dict)` : run_summary.json 기록.
- run_id 는 호출자가 만들어 전달(=<YYYYMMDDHH>_<seq>_<주제>). 이 모듈은 Date/시간 호출 안 함.
- stdlib 단독(json,os). candidate 직렬화에 yaml 사용 가능(미설치 시 json fallback).
"""

import json
import os

try:
    import yaml as _yaml
except ImportError:  # 폐쇄망 fallback: yaml 미설치 시 candidate 도 json 으로 직렬화.
    _yaml = None


def new_run_dir(repo_root, run_id):
    """docs/simlog/<run_id>/ 를 생성하고 그 경로(str)를 반환한다."""
    run_dir = os.path.join(repo_root, "docs", "simlog", run_id)
    os.makedirs(run_dir, exist_ok=True)
    return run_dir


def _trial_prefix(trial_number):
    """trialNN 접두사(2자리 0패딩)."""
    return "trial%02d" % int(trial_number)


def write_trial(run_dir, trial_number, vllm_log_text, profile_dict, candidate_dict, smoke_dict):
    """한 트라이얼의 4종 아티팩트를 run_dir 에 기록한다.

    trialNN_vllm.log     : 원시 vLLM 로그 텍스트
    trialNN_profile.json : parse_vllm_log 결과(profile_dict)
    trialNN_candidate.yaml: candidate lock-set(yaml 가능 시 yaml, 아니면 json)
    trialNN_smoke.json   : functional_smoke 결과(smoke_dict)

    None 인 인자는 해당 파일을 빈 값으로 기록(예외 던지지 않음). 반환: 기록한 prefix(str).
    """
    os.makedirs(run_dir, exist_ok=True)
    prefix = _trial_prefix(trial_number)

    log_path = os.path.join(run_dir, prefix + "_vllm.log")
    with open(log_path, "w", encoding="utf-8") as fh:
        fh.write(vllm_log_text if vllm_log_text is not None else "")

    profile_path = os.path.join(run_dir, prefix + "_profile.json")
    with open(profile_path, "w", encoding="utf-8") as fh:
        json.dump(profile_dict, fh, ensure_ascii=False, indent=2)

    candidate_path = os.path.join(run_dir, prefix + "_candidate.yaml")
    with open(candidate_path, "w", encoding="utf-8") as fh:
        if _yaml is not None:
            _yaml.safe_dump(candidate_dict, fh, allow_unicode=True, sort_keys=False)
        else:
            json.dump(candidate_dict, fh, ensure_ascii=False, indent=2)

    smoke_path = os.path.join(run_dir, prefix + "_smoke.json")
    with open(smoke_path, "w", encoding="utf-8") as fh:
        json.dump(smoke_dict, fh, ensure_ascii=False, indent=2)

    return prefix


def append_correction(run_dir, record_dict):
    """조정 이력 record(dict)를 correction_history.jsonl 한 줄로 append. 반환: 파일 경로(str)."""
    os.makedirs(run_dir, exist_ok=True)
    path = os.path.join(run_dir, "correction_history.jsonl")
    line = json.dumps(record_dict, ensure_ascii=False)
    with open(path, "a", encoding="utf-8") as fh:
        fh.write(line + "\n")
    return path


def write_summary(run_dir, summary_dict):
    """run_summary.json 을 기록한다. 반환: 파일 경로(str)."""
    os.makedirs(run_dir, exist_ok=True)
    path = os.path.join(run_dir, "run_summary.json")
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(summary_dict, fh, ensure_ascii=False, indent=2)
    return path
