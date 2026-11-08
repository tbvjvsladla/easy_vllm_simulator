"""JSON transport for Python consumers of the Rust repeat owner.

Results and exception class names/messages cross this boundary. Original exception constructor
arguments and Python-specific object attributes are not part of the JSON transport.
"""
from __future__ import annotations

import ast
import builtins
import json
from pathlib import Path
import subprocess

_LAUNCHER = Path(__file__).resolve().with_name("repeat_axis_api.sh")
REPO_ROOT = Path(__file__).resolve().parents[4]


class RepeatAxisError(ValueError):
    """The native repeat owner rejected a repeat declaration or input."""


def _run(argv, payload=None):
    process = subprocess.run(["bash", str(_LAUNCHER), *argv], input=payload,
                             capture_output=True, text=True)
    if process.returncode != 0:
        raise RuntimeError("repeat native transport failed (%s): %s" %
                           (process.returncode, process.stderr.strip()))
    return process.stdout


def _reported_error(base, name, message):
    # Some builtins (notably UnicodeDecodeError) require original byte/string constructor
    # arguments that this protocol does not carry. Preserve their catch hierarchy, reported
    # class name and message without inventing those original bytes or offsets.
    reported_type = type(name, (base,), {"__str__": lambda self: self.reported_message,
                                       "__module__": base.__module__})
    error = reported_type.__new__(reported_type)
    BaseException.__init__(error, message)
    error.reported_message = message
    return error


def _raise(record):
    name, message = record["type"], record["message"]
    if name == "RepeatAxisError":
        raise RepeatAxisError(message)
    base = getattr(builtins, name, None)
    if not isinstance(base, type) or not issubclass(base, Exception):
        raise RuntimeError("repeat native owner reported %s: %s" % (name, message))
    if base is KeyError:
        try:
            argument = ast.literal_eval(message)
        except (ValueError, SyntaxError):
            argument = message
        raise KeyError(argument)
    try:
        error = base(message)
    except TypeError:
        error = _reported_error(base, name, message)
    raise error


def _call(operation, arguments):
    encoded = {name: json.dumps(str(value) if name == "repo_root" and isinstance(value, Path) else value, ensure_ascii=True)
               for name, value in arguments.items()}
    request = {"id": "bridge", "operation": operation, "arguments_json": encoded, "result_json": True}
    lines = _run(["evaluate-jsonl"], json.dumps(request, ensure_ascii=True) + "\n").splitlines()
    if len(lines) != 1:
        raise RuntimeError("repeat native transport did not return one record")
    record = json.loads(lines[0])
    if record.get("id") != "bridge" or record.get("encoding") != "json-values-v1":
        raise RuntimeError("repeat native binary lacks the required JSON transport; rebuild it")
    if record.get("kind") == "value":
        return json.loads(record["value_json"])
    if record.get("kind") != "exception":
        raise RuntimeError("repeat native transport returned an unknown record kind")
    _raise(record)


def load_campaign_budgets(campaign_id=None, *, repo_root=REPO_ROOT):
    return tuple(_call("load_campaign_budgets", {"campaign_id": campaign_id, "repo_root": repo_root}))


def resolve_repeats(argv_value=None, *, argv_source=None, campaign_id=None,
                    repo_root=REPO_ROOT, preset_default=None, preset_source=None):
    return tuple(_call("resolve_repeats", {"argv_value": argv_value, "argv_source": argv_source,
                                         "campaign_id": campaign_id, "repo_root": repo_root,
                                         "preset_default": preset_default, "preset_source": preset_source}))


def summarize(levels, *, requested=None, requested_source=None, kind=None, clamp_run=None):
    return _call("summarize", {"levels": levels, "requested": requested,
                               "requested_source": requested_source, "kind": kind, "clamp_run": clamp_run})


def repetition_established(summary, judgment_level):
    return tuple(_call("repetition_established", {"summary": summary, "judgment_level": judgment_level}))


try:
    _bindings = json.loads(_run(["constants"]))
    if not isinstance(_bindings, dict):
        raise RuntimeError("repeat native constants are not an object")
    for _name in ("FULL_REPEATS_MIN", "REPEAT_KIND_WARM", "REPEAT_KIND_COLD", "RUN_META_NAME",
                  "SIGNAL_RUN_FAILED", "STOP_KIND_REPEAT_BREAK", "STOP_KIND_CLAMP", "BAND_METRIC",
                  "BAND_FORMULA", "CAMPAIGN_INIT_REL", "SCHEMA_REL", "RUN_DIR_PATTERN"):
        globals()[_name] = _bindings[_name]
    for _name in ("REPEAT_KINDS", "IMPLEMENTED_REPEAT_KINDS", "RUN_METRIC_KEYS", "AGGREGATE_KEYS"):
        globals()[_name] = tuple(_bindings[_name])
except (RuntimeError, ValueError, KeyError, TypeError) as error:
    raise ImportError(str(error)) from error
