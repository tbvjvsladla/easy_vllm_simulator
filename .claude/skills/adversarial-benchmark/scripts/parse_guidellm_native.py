"""JSON transport for unmigrated Python consumers of the native GuideLLM owner."""
from __future__ import annotations

import ast
import builtins
import json
from pathlib import Path
import subprocess

_LAUNCHER = Path(__file__).resolve().with_name("parse_guidellm.sh")


def _run(argv, payload=None):
    process = subprocess.run(["bash", str(_LAUNCHER), *argv], input=payload,
                             capture_output=True, text=True)
    if process.returncode != 0:
        raise RuntimeError("GuideLLM native owner failed (exit %d): %s" %
                           (process.returncode, process.stderr.rstrip()))
    if process.stderr:
        raise RuntimeError("GuideLLM native transport returned diagnostics: %s" % process.stderr.rstrip())
    return process.stdout


def _call(operation, arguments):
    # Encode each existing JSON argument before adding the envelope. Otherwise the envelope can push an
    # accepted deep document over Python's encoder limit before the native owner sees it.
    request = {"id": "python-consumer", "operation": operation,
               "arguments_json": {name: json.dumps(value, ensure_ascii=True) for name, value in arguments.items()},
               "result_json": True}
    raw = _run(["evaluate-jsonl"], json.dumps(request, ensure_ascii=True) + "\n")
    lines = raw.splitlines()
    if len(lines) != 1:
        raise RuntimeError("GuideLLM native transport did not return one record")
    record = json.loads(lines[0])
    if record.get("id") != request["id"]:
        raise RuntimeError("GuideLLM native transport returned a different record identity")
    if record.get("encoding") != "json-values-v1":
        raise RuntimeError("GuideLLM native owner lacks the required JSON transport encoding; rebuild it")
    if record.get("kind") == "value":
        return json.loads(record["value_json"])
    if record.get("kind") != "exception":
        raise RuntimeError("GuideLLM native transport returned an unknown record kind")
    name, message = record["type"], record["message"]
    exception = getattr(builtins, name, None)
    if not isinstance(exception, type) or not issubclass(exception, Exception):
        raise RuntimeError("GuideLLM native owner returned an unknown exception: %s: %s" % (name, message))
    if exception is KeyError:
        # KeyError renders repr(args[0]); reconstruct that argument rather than quoting its rendered message twice.
        try:
            argument = ast.literal_eval(message)
        except (ValueError, SyntaxError):
            argument = message
        raise KeyError(argument)
    raise exception(message)


def build(doc, benchmark_index=0, engine_max=None, spec=None, max_error_rate=0.0,
          server_alive_at_bench_end=None):
    return _call("build", {"doc": doc, "benchmark_index": benchmark_index,
                          "engine_max": engine_max, "spec": spec, "max_error_rate": max_error_rate,
                          "server_alive_at_bench_end": server_alive_at_bench_end})


def inherit_accept_len(src, path):
    return _call("inherit_accept_len", {"src": src, "path": path})


def classify_errors(run, benchmark_index=0, server_alive_at_bench_end=None):
    return _call("classify_errors", {"run": run, "benchmark_index": benchmark_index,
                                    "server_alive_at_bench_end": server_alive_at_bench_end})


try:
    _binding = _run(["bench-tool"]).splitlines()
    if len(_binding) != 1 or not _binding[0]:
        raise RuntimeError("GuideLLM native owner did not return one binding token")
    BENCH_TOOL = _binding[0]
except RuntimeError as error:
    # Existing import consumers treat an unavailable owner as undeterminable.
    raise ImportError(str(error)) from error
