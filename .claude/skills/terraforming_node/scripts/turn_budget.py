#!/usr/bin/env python3
"""turn_budget.py — 턴 예산의 **선언 검증기**(값의 소유자가 아니다). 얇은 import 어댑터.

검증·판정·메시지·상한 읽기는 모두 Rust(`adversarial-benchmark/rust` 의 `turn_budget` 실행파일; 순수 업무는 공유 T
`role-contract::turn_budget`)가 한다. 이 파일은 **기존 Python importer**(relay·bootstrap_canary·selftest_harness_switch)가
`schema_cap`·`schema_max_turns`·`declare`·`REPO`·`REQUEST_SCHEMA` 를 같은 이름·서명으로 계속 부르게 하는 임시 어댑터이며
업무 로직이 없다: 지원되는 값의 타입 전달(marshalling)·정확한 예외 클래스 재구성·서명·어휘적(lexical) 출처 사실만 가진다.

규약(원본 유지): 요청 스키마가 상한의 **단일 권위**이고 **호출마다 다시 읽는다**(상수·캐시 ✗). 기본 REPO 는 이 파일 경로
(`os.path.abspath(__file__)`, 심볼릭 링크 해소 ✗)에서 네 단계 위다. 예산은 부르는 쪽이 선언한다(기본값 ✗).

경로:
  · CLI(`python3 turn_budget.py ...`) → `turn_budget.sh` 로 exec(같은 argv·환경·PID; 이 파일 경로를 출처로 전달).
  · API(import) → 같은 실행파일의 비공개 브리지(`TURN_BUDGET_BRIDGE`, 타입 태그 JSON, 정수는 16진 전송).
Rust 실행파일이 없으면 상태 2 와 빌드 안내로 **눈에 띄게** 실패한다(Python 대체 경로 없음 · 업무가 시작된 뒤 재시도 없음).

남은 잔여(숨기지 않는다): ① 이 어댑터가 전달하지 않는 값(정확한 str·int·bool·float·None·bytes·list·tuple·문자열 키 dict·
`os.PathLike` 밖의 객체, int 경로=파일 디스크립터)은 업무 전에 `NotImplementedError` 로 거절한다. ② in-process 예외의
traceback 바이트(원본 소스 프레임)는 재현하지 않는다(U2 · 열림).
"""
from __future__ import annotations

import argparse  # noqa: F401 — the original's public import-time dependency (part of its module facts); the adapter does not use it
import json
import os
import subprocess
import sys

# 요청 스키마가 상한의 **단일 권위**다 — 여기 숫자를 다시 적지 않는다(두 자리가 갈라진다).
REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", ".."))
REQUEST_SCHEMA = os.path.join(REPO, ".claude", "schemas", "agent-control-request.schema.json")

_LAUNCHER = os.path.join(os.path.dirname(os.path.abspath(__file__)), "turn_budget.sh")
_PROTOCOL = "turn-budget-bridge/1"
_DEFAULT_SCHEMA = REQUEST_SCHEMA  # 원본의 `path=REQUEST_SCHEMA` 는 정의 시점에 묶인다 — declare 의 기본 스키마도 같다
_EXCEPTIONS = {
    "SystemExit": SystemExit, "FileNotFoundError": FileNotFoundError, "PermissionError": PermissionError,
    "IsADirectoryError": IsADirectoryError, "NotADirectoryError": NotADirectoryError, "OSError": OSError,
    "UnicodeDecodeError": UnicodeDecodeError, "UnicodeEncodeError": UnicodeEncodeError,
    "ValueError": ValueError, "TypeError": TypeError, "AttributeError": AttributeError, "KeyError": KeyError,
    "RecursionError": RecursionError, "StopIteration": StopIteration, "JSONDecodeError": json.JSONDecodeError,
}


# ── typed marshalling (no decision is taken here) ──────────────────────────────────────────

def _unsupported(what):
    raise NotImplementedError("turn_budget bridge does not carry %s (the original's behaviour is not reproduced)" % what)


def _wire(value, _open=()):
    kind = type(value)
    if value is None:
        return {"t": "none"}
    if kind is bool:
        return {"t": "bool", "v": value}
    if kind is int:
        return {"t": "int", "hex": ("-" if value < 0 else "+") + format(abs(value), "x")}
    if kind is float:
        return {"t": "float", "repr": repr(value)}
    if kind is str:
        return {"t": "str", "v": value}
    if kind is bytes:
        return {"t": "bytes", "hex": value.hex()}
    if kind in (list, tuple, dict):
        if id(value) in _open:
            _unsupported("a self-referencing container")
        inner = _open + (id(value),)
        if kind is dict:
            if any(type(key) is not str for key in value):
                _unsupported("a dict with non-str keys")
            return {"t": "dict", "v": [[_wire(key), _wire(item, inner)] for key, item in value.items()]}
        return {"t": "list" if kind is list else "tuple", "v": [_wire(item, inner) for item in value]}
    _unsupported("a value of type %s" % kind.__name__)


def _wire_path(path):
    kind = type(path)
    if kind is not str and kind is not bytes and path is not None and hasattr(kind, "__fspath__"):
        fspath = os.fspath(path)
        return {"t": "pathlike", "fspath": _wire(fspath), "text": _wire(format(path, ""))}
    return _wire(path)


def _unwire(item):
    tag = item["t"]
    if tag == "none":
        return None
    if tag == "bool":
        return item["v"]
    if tag == "int":
        return int(item["hex"], 16)
    if tag == "float":
        return float(item["repr"])
    if tag == "str":
        return item["v"]
    if tag == "bytes":
        return bytes.fromhex(item["hex"])
    if tag == "list":
        return [_unwire(x) for x in item["v"]]
    if tag == "tuple":
        return tuple(_unwire(x) for x in item["v"])
    if tag == "dict":
        return {_unwire(key): _unwire(value) for key, value in item["v"]}
    raise RuntimeError("turn_budget bridge answered an unknown wire type %r" % tag)


def _rebuild(exc):
    cls = _EXCEPTIONS.get(exc["cls"])
    if cls is None:
        raise RuntimeError("turn_budget bridge reported an unknown exception class %r" % exc["cls"])
    error = cls(*[_unwire(x) for x in exc["ctor"]], **{k: _unwire(v) for k, v in exc["ctor_kwargs"].items()})
    if exc["context"] is not None:
        error.__context__ = _rebuild(exc["context"])
    error.__suppress_context__ = exc["suppress_context"]
    return error


def _ask(request):
    """One request to the private bridge; the answer record (never an exception of the original)."""
    env = dict(os.environ, TURN_BUDGET_BRIDGE="1", TURN_BUDGET_OWNER_PATH=os.path.abspath(__file__))
    request["protocol"] = _PROTOCOL
    process = subprocess.run(["/bin/bash", _LAUNCHER], input=json.dumps(request, ensure_ascii=True).encode("ascii"),
                             capture_output=True, env=env)
    if process.returncode != 0:
        raise RuntimeError("[turn_budget] native bridge failed (status %d): %s"
                           % (process.returncode, process.stderr.decode("utf-8", "replace").rstrip()))
    answer = json.loads(process.stdout.decode("ascii"))
    if answer.get("protocol") != _PROTOCOL:
        raise RuntimeError("[turn_budget] native bridge speaks another protocol; rebuild it")
    return answer


def _finish(answer):
    status = answer["status"]
    if status == "raised":
        raise _rebuild(answer["exc"])
    if status == "unsupported":
        _unsupported(answer["reason"])
    return _unwire(answer["value"])


# ── the original API (same names, signatures, constants) ───────────────────────────────────────

def schema_cap(field: str, path: str = REQUEST_SCHEMA) -> int:
    """agent-control 요청 스키마에서 그 필드의 상한을 읽는다. 못 읽으면 fail-loud(Rust 가 호출마다 읽는다)."""
    request = {"op": "schema_cap", "field": _wire(field), "path": _wire_path(path)}
    return _finish(_ask(request))


def schema_max_turns(path: str = REQUEST_SCHEMA) -> int:
    return schema_cap("max_turns", path)


def declare(max_turns, timeout_seconds, *, source: str, caps: dict = None,
            unit: str = "native_turns", max_dispatches: int | None = None) -> dict:
    """**선언된** 예산을 검증해 그대로 돌려준다. 값을 만들지 않는다 — 없으면 fail-loud. (`max_dispatches` 는 원본처럼 무시한다.)"""
    request = {"op": "declare", "max_turns": _wire(max_turns), "timeout_seconds": _wire(timeout_seconds),
               "source": _wire(source), "caps": _wire(caps), "unit": _wire(unit),
               "default_schema": _wire_path(_DEFAULT_SCHEMA)}
    return _finish(_ask(request))


def _module_facts() -> str:
    """The key names of this module's namespace, as a fact for the self-test's tripwire (Rust owns the list and the verdict)."""
    return "\n".join(globals())


def _self_test() -> int:
    # a closed standard descriptor leaves `sys.stdout` as None: there is nothing to flush (a real stream's flush error still propagates)
    if sys.stdout is not None:
        sys.stdout.flush()
    return subprocess.call(["/bin/bash", _LAUNCHER, "--self-test"],
                           env=dict(os.environ, TURN_BUDGET_OWNER_PATH=os.path.abspath(__file__),
                                    TURN_BUDGET_MODULE_NAMES=_module_facts()))


def main() -> int:
    """CLI: the launcher replaces this process (same argv, environment and PID); it only returns on a failed exec."""
    env = {k: v for k, v in os.environ.items() if k != "TURN_BUDGET_BRIDGE"}
    env["TURN_BUDGET_OWNER_PATH"] = os.path.abspath(__file__)
    env["TURN_BUDGET_MODULE_NAMES"] = _module_facts()
    # an absent stream (closed fd 1/2: `sys.stdout`/`sys.stderr` is None) is skipped; a present stream's flush error is not swallowed
    for stream in (sys.stdout, sys.stderr):
        if stream is not None:
            stream.flush()
    os.execve("/bin/bash", ["bash", _LAUNCHER, *sys.argv[1:]], env)
    return 1


if __name__ == "__main__":
    sys.exit(main())
