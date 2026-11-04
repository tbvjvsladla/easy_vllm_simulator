"""JSON transport for Python callers of the native lite metrics owner.

This bridge preserves reported exception classes/messages. Python-specific exception
attributes and constructor arguments are outside the JSON protocol.
"""
from __future__ import annotations

import ast
import builtins
import json
from pathlib import Path
import subprocess

_LAUNCHER = Path(__file__).resolve().with_name("lite_metrics_api.sh")


def _run(argv, payload=None):
    process = subprocess.run(["bash", str(_LAUNCHER), *argv], input=payload,
                             capture_output=True, text=True)
    if process.returncode != 0 or process.stderr:
        raise RuntimeError("lite metrics native transport failed (%s): %s" %
                           (process.returncode, process.stderr.rstrip()))
    return process.stdout


def _raise(record):
    name, message = record["type"], record["message"]
    base = getattr(builtins, name, None)
    if not isinstance(base, type) or not (issubclass(base, Exception) or base is SystemExit):
        raise RuntimeError("lite metrics native owner reported %s: %s" % (name, message))
    if base is KeyError:
        try:
            argument = ast.literal_eval(message)
        except (ValueError, SyntaxError):
            argument = message
        raise KeyError(argument)
    try:
        error = base(message)
    except TypeError:
        # Unicode errors need original byte/string arguments absent from this protocol.
        reported = type(name, (base,), {"__str__": lambda self: self.reported_message,
                                       "__module__": base.__module__})
        error = reported.__new__(reported)
        BaseException.__init__(error, message)
        error.reported_message = message
    raise error


def _call(operation, arguments):
    request = {"id": "python-consumer", "operation": operation,
               "arguments_json": {name: json.dumps(value, ensure_ascii=True)
                                  for name, value in arguments.items()},
               "result_json": True}
    lines = _run(["evaluate-jsonl"], json.dumps(request, ensure_ascii=True) + "\n").splitlines()
    if len(lines) != 1:
        raise RuntimeError("lite metrics native transport did not return one record")
    record = json.loads(lines[0])
    if record.get("id") != request["id"] or record.get("encoding") != "json-values-v1":
        raise RuntimeError("lite metrics native binary lacks the required JSON transport; rebuild it")
    if record.get("kind") == "value":
        return json.loads(record["value_json"])
    if record.get("kind") != "exception":
        raise RuntimeError("lite metrics native transport returned an unknown record kind")
    _raise(record)


def build(raw):
    return _call("build", {"raw": raw})


def judge_lite(raw, built):
    return _call("judge_lite", {"raw": raw, "built": built})


def read_lite_verdict(raw):
    return _call("read_lite_verdict", {"raw": raw})


def _load_json(path):
    return _call("_load_json", {"path": str(path) if isinstance(path, Path) else path})


try:
    _bindings = json.loads(_run(["constants"]))
    LITE_VERDICTS = tuple(_bindings["LITE_VERDICTS"])
    LITE_EXIT = _bindings["LITE_EXIT"]
except (RuntimeError, ValueError, KeyError, TypeError) as error:
    raise ImportError(str(error)) from error
