"""JSON transport for callers of the native classifier owner.

Decision rules live in Rust. Exceptions preserve their reported class/message;
Python-specific constructor arguments and attributes are outside this protocol.
Datetime values cross the wire as the owner's documented UTC ISO objects.
"""
from __future__ import annotations

import ast
import builtins
import datetime as _dt
import json
from pathlib import Path
import subprocess
import sys

_LAUNCHER = Path(__file__).resolve().with_name("classify_cell_api.sh")


class ClassifyError(ValueError):
    """The classifier rejected a time declaration or classification input."""


def _run(argv, payload=None):
    process = subprocess.run(["bash", str(_LAUNCHER), *argv], input=payload,
                             capture_output=True, text=True)
    if process.returncode != 0 or process.stderr:
        raise RuntimeError("classifier native transport failed (%s): %s" %
                           (process.returncode, process.stderr.rstrip()))
    return process.stdout


def _raise(record):
    name, message = record["type"], record["message"]
    if name == "ClassifyError":
        raise ClassifyError(message)
    base = getattr(builtins, name, None)
    if not isinstance(base, type) or not (issubclass(base, Exception) or base is SystemExit):
        raise RuntimeError("classifier native owner reported %s: %s" % (name, message))
    if base is KeyError:
        try:
            argument = ast.literal_eval(message)
        except (ValueError, SyntaxError):
            argument = message
        raise KeyError(argument)
    try:
        error = base(message)
    except TypeError:
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
        raise RuntimeError("classifier native transport did not return one record")
    record = json.loads(lines[0])
    if record.get("id") != request["id"] or record.get("encoding") != "json-values-v1":
        raise RuntimeError("classifier native binary lacks the required JSON transport; rebuild it")
    if record.get("stderr"):
        sys.stderr.write(record["stderr"])
    if record.get("kind") == "value":
        return json.loads(record["value_json"])
    if record.get("kind") != "exception":
        raise RuntimeError("classifier native transport returned an unknown record kind")
    _raise(record)


def _path(value):
    return str(value) if isinstance(value, Path) else value


def _datetime_wire(value):
    if isinstance(value, _dt.datetime):
        return {"datetime_utc": value.isoformat()}
    if isinstance(value, dict) and set(value) == {"datetime_utc"}:
        # A source dict remains a dict operand, instead of acquiring the reserved wire type.
        return {"non_datetime_dict": None}
    return value


def _datetime_value(value):
    return None if value is None else _dt.datetime.fromisoformat(value["datetime_utc"])


def classify(serve_rc, measure_rc, kill_hits, events_scanned, correlation=None, lite=None):
    return _call("classify", dict(serve_rc=serve_rc, measure_rc=measure_rc, kill_hits=kill_hits,
                                 events_scanned=events_scanned, correlation=correlation, lite=lite))


def events_scan_summary(scan_doc):
    return _call("events_scan_summary", {"scan_doc": scan_doc})


def bench_mode_record_path(sweep_index_path):
    return _call("bench_mode_record_path", {"sweep_index_path": _path(sweep_index_path)})


def read_bench_mode_record(sweep_index_path, index_doc, record_path=None):
    return tuple(_call("read_bench_mode_record", dict(sweep_index_path=_path(sweep_index_path),
                      index_doc=index_doc, record_path=_path(record_path))))


def write_bench_mode_record(sweep_index_path, record):
    return _call("write_bench_mode_record", dict(sweep_index_path=_path(sweep_index_path), record=record))


def bench_mode_kind(record):
    return _call("bench_mode_kind", {"record": record})


def declared_lite_record(detail):
    return _call("declared_lite_record", {"detail": detail})


def events_node_plan(topology, measured_node=None, measured_node_source=None, manifest_path=None, repo=None):
    return _call("events_node_plan", dict(topology=topology, measured_node=measured_node,
                 measured_node_source=measured_node_source, manifest_path=_path(manifest_path), repo=_path(repo)))


def scan_node_events(repo, plan):
    return _call("scan_node_events", dict(repo=_path(repo), plan=plan))


def resolve_self_node(repo):
    return tuple(_call("resolve_self_node", {"repo": _path(repo)}))


def _utc(value, field):
    return _datetime_value(_call("_utc", dict(value=value, field=field)))


def kill_events_in_window(event_lines, started, ended, tolerance_s=0):
    return _call("kill_events_in_window", dict(event_lines=event_lines, started=_datetime_wire(started),
                 ended=_datetime_wire(ended), tolerance_s=tolerance_s))


def correlate_nodes(node_scans, lo, hi, tolerance_s=0, cross_node_tolerance_s=None, cross_node_tolerance_source=None):
    return tuple(_call("correlate_nodes", dict(node_scans=node_scans, lo=_datetime_wire(lo), hi=_datetime_wire(hi),
                 tolerance_s=tolerance_s, cross_node_tolerance_s=cross_node_tolerance_s,
                 cross_node_tolerance_source=cross_node_tolerance_source)))


def read_cell_lite(raw_path, started):
    return _call("read_cell_lite", dict(raw_path=_path(raw_path), started=_datetime_wire(started)))


def classify_bench_mode(index, event_lines, events_scanned, tolerance_s=0, node_scans=None, plan=None,
                        cross_node_tolerance_s=None, cross_node_tolerance_source=None):
    return _call("classify_bench_mode", dict(index=index, event_lines=event_lines, events_scanned=events_scanned,
                 tolerance_s=tolerance_s, node_scans=node_scans, plan=plan,
                 cross_node_tolerance_s=cross_node_tolerance_s, cross_node_tolerance_source=cross_node_tolerance_source))


try:
    _bindings = json.loads(_run(["constants"]))
    for _name, _value in _bindings.items():
        globals()[_name] = tuple(_value) if isinstance(_value, list) else _value
except (RuntimeError, ValueError, KeyError, TypeError) as error:
    raise ImportError(str(error)) from error
