#!/usr/bin/env python3
"""functional_smoke.py — 기능 스모크 검사기 (vllm-recipe-explorer 스킬 Phase 2).

CONTRACT(FROZEN) functional_smoke.py 절 준수. OpenAI 호환 /v1/chat/completions 로
서빙 중인 후보를 두드려 (완성·tool_call·reasoning) 동작을 판정한다.
importable 함수 `smoke(base_url, served_model_name, candidate)` + __main__ CLI 둘 다 제공.

능력 게이팅: candidate.model_capabilities.tool_call False면 tool_call_ok=null(스킵),
reasoning 도 동일. 능력이 있을 때만 해당 검사를 수행한다.

HTTP 는 stdlib urllib 만 사용(requests 금지). 네트워크 타임아웃·연결실패는
completion_ok=False 로 처리하고 예외로 죽지 않는다.
stdlib(json,os,sys,argparse,urllib) 만 사용. 외부 네트워크 호출은 base_url 로컬 서빙만.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.request

# scripts/ 디렉토리를 import 경로에 보장(직접 실행/타 스크립트에서 import 양쪽 대응).
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

# 검사용 고정 프롬프트(결정론 — 매 트라이얼 동일 입력).
_COMPLETION_PROMPT = "Say hello in one short sentence."
_REASONING_PROMPT = "What is 17 plus 26? Think step by step, then give the answer."
_TOOL_PROMPT = "What is the weather in Seoul right now? Use the available tool."

# tool_call 검사용 도구 스키마(OpenAI 호환).
_WEATHER_TOOL = {
    "type": "function",
    "function": {
        "name": "get_weather",
        "description": "Get the current weather for a given city.",
        "parameters": {
            "type": "object",
            "properties": {
                "city": {"type": "string", "description": "City name."},
            },
            "required": ["city"],
        },
    },
}


def _post_chat(base_url: str, payload: dict, timeout: float = 60.0) -> dict:
    """/v1/chat/completions 에 POST → 응답 JSON dict 반환.

    네트워크/HTTP 오류는 던지지 않고 {"_error": "..."} 형태로 돌려준다(호출자가
    completion_ok=False 로 처리). 정상 응답은 파싱된 JSON dict.
    """
    url = base_url.rstrip("/") + "/v1/chat/completions"
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        url, data=data, headers={"Content-Type": "application/json"}, method="POST"
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            body = resp.read().decode("utf-8")
        return json.loads(body)
    except urllib.error.HTTPError as e:  # 4xx/5xx
        try:
            detail = e.read().decode("utf-8")
        except Exception:
            detail = ""
        return {"_error": f"http_{e.code}: {detail[:300]}"}
    except urllib.error.URLError as e:  # 연결 실패·타임아웃
        return {"_error": f"urlerror: {e.reason}"}
    except Exception as e:  # noqa: BLE001  — 어떤 예외도 검사 실패로만 흡수
        return {"_error": f"exception: {type(e).__name__}: {e}"}


def _first_choice(resp: dict) -> dict:
    """응답 dict 에서 첫 choice 를 안전하게 꺼낸다(없으면 {})."""
    if not isinstance(resp, dict):
        return {}
    choices = resp.get("choices")
    if not isinstance(choices, list) or not choices:
        return {}
    first = choices[0]
    return first if isinstance(first, dict) else {}


def _check_completion(choice: dict) -> bool:
    """완성 검사 = message.content 가 비어있지 않은 문자열."""
    msg = choice.get("message") if isinstance(choice, dict) else None
    if not isinstance(msg, dict):
        return False
    content = msg.get("content")
    return isinstance(content, str) and content.strip() != ""


def _check_reasoning(choice: dict) -> bool:
    """reasoning 검사 = 분리된 reasoning 필드가 비어있지 않게 존재.
    필드명은 vLLM 버전별로 다르다: reasoning_content(구) vs reasoning(신 — gpt-oss harmony/0.18.0). 둘 다 수용."""
    msg = choice.get("message") if isinstance(choice, dict) else None
    if not isinstance(msg, dict):
        return False
    rc = msg.get("reasoning_content") or msg.get("reasoning")
    return isinstance(rc, str) and rc.strip() != ""


def _check_tool_call(choice: dict) -> bool:
    """tool_call 검사 = finish_reason=="tool_calls" 또는 message.tool_calls 존재."""
    if not isinstance(choice, dict):
        return False
    if choice.get("finish_reason") == "tool_calls":
        return True
    msg = choice.get("message")
    if isinstance(msg, dict):
        tc = msg.get("tool_calls")
        if isinstance(tc, list) and len(tc) > 0:
            return True
    return False


def smoke(
    base_url: str,
    served_model_name: str,
    candidate: dict,
    timeout: float = 60.0,
    mock_responses: dict | None = None,
) -> dict:
    """후보 서빙에 대한 기능 스모크.

    candidate.model_capabilities = {tool_call: bool, reasoning: bool} 로 검사 게이팅.
    능력 False(또는 미지정)면 해당 *_ok = None(N/A, 스킵).

    반환 keys:
        completion_ok(bool), tool_call_ok(bool|None), reasoning_ok(bool|None),
        passed(bool = completion_ok and tool_call_ok in {True,None}
                       and reasoning_ok in {True,None}),
        evidence(dict).

    mock_responses 가 주어지면 실제 HTTP 대신 그 응답으로 판정한다(검증·테스트용).
        형식: {"completion": <resp>, "reasoning": <resp>, "tool_call": <resp>}.
        키가 없으면 해당 검사는 빈 응답으로 처리(False).
    """
    caps = candidate.get("model_capabilities") or {}
    want_tool = bool(caps.get("tool_call"))
    want_reasoning = bool(caps.get("reasoning"))

    evidence: dict = {}

    # ── 완성 검사(항상 수행) ────────────────────────────────────────────
    if mock_responses is not None:
        comp_resp = mock_responses.get("completion", {})
    else:
        comp_resp = _post_chat(
            base_url,
            {
                "model": served_model_name,
                "messages": [{"role": "user", "content": _COMPLETION_PROMPT}],
                # reasoning 모델은 analysis 채널이 토큰을 소모 → content(최종 채널) 도달 전 length 절단됨.
                # 능력에 reasoning 있으면 충분히 줘 finish_reason=stop 유도(없으면 짧게).
                # ★ 1024 → 4096 (2026-08-02 실측). Qwen3.5-122B-NVFP4 가 "Say hello in one short
                #   sentence." 에 **1024 를 다 쓰고도 finish=length** 였다. 같은 프롬프트를
                #   2048/4096 으로 재요청하니 각각 273/148 토큰에 stop — 즉 사고 길이가 요청마다
                #   크게 흔들린다(148~273, 간헐 1024 초과). 상한을 사고 분산에 맞춰 잡아야
                #   **모델이 멀쩡한데 도구가 실패로 판정**하는 위양성을 막는다.
                #   토큰 상한은 비용이 아니라 **판정 정확도**의 문제다 — stop 이면 실제 사용량만 든다.
                "max_tokens": 4096 if want_reasoning else 256,
                "temperature": 0.0,
            },
            timeout=timeout,
        )
    completion_ok = _check_completion(_first_choice(comp_resp))
    evidence["completion"] = comp_resp

    # ── reasoning 검사(능력 있을 때만) ──────────────────────────────────
    reasoning_ok: bool | None
    if not want_reasoning:
        reasoning_ok = None  # N/A
    else:
        if mock_responses is not None:
            reas_resp = mock_responses.get("reasoning", {})
        else:
            reas_resp = _post_chat(
                base_url,
                {
                    "model": served_model_name,
                    "messages": [{"role": "user", "content": _REASONING_PROMPT}],
                    # reasoning 모델은 충분한 토큰을 줘 finish_reason=stop 유도.
                    "max_tokens": 1024,
                    "temperature": 0.0,
                },
                timeout=timeout,
            )
        reasoning_ok = _check_reasoning(_first_choice(reas_resp))
        evidence["reasoning"] = reas_resp

    # ── tool_call 검사(능력 있을 때만) ──────────────────────────────────
    tool_call_ok: bool | None
    if not want_tool:
        tool_call_ok = None  # N/A
    else:
        if mock_responses is not None:
            tool_resp = mock_responses.get("tool_call", {})
        else:
            tool_resp = _post_chat(
                base_url,
                {
                    "model": served_model_name,
                    "messages": [{"role": "user", "content": _TOOL_PROMPT}],
                    "tools": [_WEATHER_TOOL],
                    "tool_choice": "auto",
                    "max_tokens": 256,
                    "temperature": 0.0,
                },
                timeout=timeout,
            )
        tool_call_ok = _check_tool_call(_first_choice(tool_resp))
        evidence["tool_call"] = tool_resp

    passed = bool(
        completion_ok
        and tool_call_ok in {True, None}
        and reasoning_ok in {True, None}
    )

    return {
        "completion_ok": bool(completion_ok),
        "tool_call_ok": tool_call_ok,
        "reasoning_ok": reasoning_ok,
        "passed": passed,
        "evidence": evidence,
    }


def _main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(
        description="기능 스모크 검사기 (vllm-recipe-explorer Phase 2)."
    )
    p.add_argument(
        "--base-url",
        default="http://127.0.0.1:8903",
        help="서빙 base_url (예: http://127.0.0.1:8903)",
    )
    p.add_argument("--model", required=True, help="served_model_name")
    p.add_argument(
        "--candidate",
        default=None,
        help="candidate(lock-set) JSON 경로 — model_capabilities 게이팅에 사용",
    )
    p.add_argument("--timeout", type=float, default=60.0, help="HTTP 타임아웃(초)")
    p.add_argument(
        "--mock",
        default=None,
        help="실제 HTTP 대신 사용할 mock 응답 JSON 경로 "
        "({completion,reasoning,tool_call: <resp>}).",
    )
    args = p.parse_args(argv)

    if args.candidate:
        with open(args.candidate, "r", encoding="utf-8") as f:
            candidate = json.load(f)
    else:
        candidate = {}

    mock_responses = None
    if args.mock:
        with open(args.mock, "r", encoding="utf-8") as f:
            mock_responses = json.load(f)

    result = smoke(
        args.base_url,
        args.model,
        candidate,
        timeout=args.timeout,
        mock_responses=mock_responses,
    )
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())
