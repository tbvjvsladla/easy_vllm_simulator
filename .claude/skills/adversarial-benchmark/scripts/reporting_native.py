"""JSON transport bridge: the module surface Python consumers import from the five
reporting/naming owners (`doc_naming`, `render_report`, `render_sweep_map`, `render_max_report`,
`publish_benchmark_record`), answered by the Rust `bench_reporting_api`. It computes nothing: every call is one
`evaluate-jsonl` row. Exceptions cross as class name + message; the owners' own exception classes are defined once
here (`NamingCollisionExhausted`, `NamingSourceUnreadable`, `RenderError`, `LiteIdentityError`, all `ValueError`s),
so a module that re-exports them (the runtime `doc_naming.py`) shares one identity. A `SystemExit` row writes its
`stderr` first, then raises `SystemExit(int(code))`. Tuples come back as tuples where the original returns them
(`kst_tokens`, the pairs of `manifest_path_labels`)."""
from __future__ import annotations

import ast
import builtins
import json
import os
from pathlib import Path
import subprocess
import sys

_LAUNCHER = Path(os.environ.get("BENCH_REPORTING_API_LAUNCHER") or Path(__file__).resolve().with_name("bench_reporting_api.sh"))


class NamingCollisionExhausted(ValueError):
    """Both the unsuffixed name and the single `_MM_SS` candidate belong to other measurements."""


class NamingSourceUnreadable(ValueError):
    """A candidate file's measurement time could not be read."""


class RenderError(ValueError):
    """The map renderer rejected its input or found a ranking field."""


class LiteIdentityError(ValueError):
    """No strong naming key could be built for a lite run."""


_OWN = {
    "NamingCollisionExhausted": NamingCollisionExhausted,
    "NamingSourceUnreadable": NamingSourceUnreadable,
    "RenderError": RenderError,
    "LiteIdentityError": LiteIdentityError,
}


def _reported_error(base, name, message):
    reported = type(name, (base,), {"__str__": lambda self: self.reported_message, "__module__": base.__module__})
    error = reported.__new__(reported)
    BaseException.__init__(error, message)
    error.reported_message = message
    return error


def _raise(record):
    name, message = record["type"], record["message"]
    if record.get("stderr"):
        sys.stderr.write(record["stderr"])
    if name == "SystemExit":
        raise SystemExit(int(message) if message.lstrip("-").isdigit() else message)
    if name in _OWN:
        raise _OWN[name](message)
    base = getattr(builtins, name, None)
    if not isinstance(base, type) or not issubclass(base, Exception):
        raise RuntimeError("reporting native owner reported %s: %s" % (name, message))
    if base is KeyError:
        try:
            raise KeyError(ast.literal_eval(message))
        except (ValueError, SyntaxError):
            raise KeyError(message) from None
    try:
        error = base(message)
    except TypeError:
        error = _reported_error(base, name, message)
    raise error


def _path(value):
    return os.fspath(value) if isinstance(value, os.PathLike) else value


def _run(command, payload=None):
    process = subprocess.run(["bash", str(_LAUNCHER), command], input=payload, capture_output=True, text=True)
    if process.returncode != 0 or process.stderr:
        raise RuntimeError("reporting native transport failed (%s): %s" % (process.returncode, process.stderr.strip()))
    return process.stdout


def _load_constants():
    constants = json.loads(_run("constants"))
    if not isinstance(constants, dict):
        raise RuntimeError("reporting native constants are not an object")
    return constants


def _call(owner, function, **arguments):
    # Only source filesystem parameters acquire PathLike transport. Other value types stay untouched.
    for key in ("path", "start", "manifest_path", "out_dir"):
        if key in arguments:
            arguments[key] = _path(arguments[key])
    if isinstance(arguments.get("raw"), dict):
        raw = dict(arguments["raw"])
        for key in ("config_yaml", "env_file", "manifest", "engine_log"):
            if key in raw:
                raw[key] = _path(raw[key])
        arguments["raw"] = raw
    row = json.dumps({"id": "call", "operation": "%s.%s" % (owner, function), "arguments": arguments}, ensure_ascii=True)
    records = [line for line in _run("evaluate-jsonl", row + "\n").splitlines() if line.strip()]
    if len(records) != 1:
        raise RuntimeError("reporting native transport returned %s rows, expected one" % len(records))
    record = json.loads(records[0])
    if record.get("id") != "call" or record.get("kind") not in ("value", "exception"):
        raise RuntimeError("reporting native transport returned the wrong row")
    if record["kind"] == "exception":
        _raise(record)
    if "value" not in record:
        raise RuntimeError("reporting native transport returned no value")
    if record.get("stderr"):
        sys.stderr.write(record["stderr"])
    return record["value"]


def _given(**arguments):
    return {name: value for name, value in arguments.items() if value is not _MISSING}


_MISSING = object()


# ── doc_naming ────────────────────────────────────────────────────────────────
_CONSTANTS = _load_constants()
BENCH_KIND_DEFAULT_EXT = _CONSTANTS["BENCH_KIND_DEFAULT_EXT"]
MEASUREMENT_CONFIG_TITLE = _CONSTANTS["MEASUREMENT_CONFIG_TITLE"]
MEASUREMENT_CONFIG_KEYS = tuple(_CONSTANTS["MEASUREMENT_CONFIG_KEYS"])
LITE_BENCH_TOOL = _CONSTANTS["LITE_BENCH_TOOL"]
FORBIDDEN_KEYS = frozenset(_CONSTANTS["FORBIDDEN_KEYS"])
FORBIDDEN_HEADERS = tuple(_CONSTANTS["FORBIDDEN_HEADERS"])
ENTRY_BETA, ENTRY_GAMMA = _CONSTANTS["ENTRY_BETA"], _CONSTANTS["ENTRY_GAMMA"]
LITE_CERT_SOFT_KEYS = tuple(_CONSTANTS["LITE_CERT_SOFT_KEYS"])


def kst_tokens(generated_utc):
    return tuple(_call("doc_naming", "kst_tokens", generated_utc=generated_utc))


def gpu_key(gpu_model):
    return _call("doc_naming", "gpu_key", gpu_model=gpu_model)


def measurement_ts(text):
    return _call("doc_naming", "measurement_ts", text=text)


def scan_bench_dir(out_dir, kind=_MISSING):
    return _call("doc_naming", "scan_bench_dir", out_dir=out_dir, **_given(kind=kind))


def bench_filename(kind, meta, generated_utc, existing=_MISSING, ext=_MISSING):
    return _call("doc_naming", "bench_filename", kind=kind, meta=meta, generated_utc=generated_utc, **_given(existing=existing, ext=ext))


# ── render_report ─────────────────────────────────────────────────────────────
def load(path, label, _owner="render_report"):
    return _call(_owner, "load", path=path, label=label)


def repo_root(start, _owner="render_report"):
    return _call(_owner, "repo_root", start=start)


def na(v, suffix=_MISSING, _owner="render_report"):
    return _call(_owner, "na", v=v, **_given(suffix=suffix))


def fnum(v, fmt=_MISSING, suffix=_MISSING):
    return _call("render_report", "fnum", v=v, **_given(fmt=fmt, suffix=suffix))


def build_md(index, verdict, roofline, bench_mode_record=_MISSING, bench_mode_status=_MISSING, path_labels=_MISSING):
    return _call("render_report", "build_md", index=index, verdict=verdict, roofline=roofline,
                 **_given(bench_mode_record=bench_mode_record, bench_mode_status=bench_mode_status, path_labels=path_labels))


def manifest_path_labels(manifest_path):
    return [tuple(pair) for pair in _call("render_report", "manifest_path_labels", manifest_path=manifest_path)]


def lite_identity(raw, environ=_MISSING):
    return _call("render_report", "lite_identity", raw=raw, **_given(environ=environ))


def lite_measurement_config():
    return _call("render_report", "lite_measurement_config")


def build_lite_md(raw, lite, identity):
    return _call("render_report", "build_lite_md", raw=raw, lite=lite, identity=identity)


def measurement_config_section(values):
    return _call("render_report", "measurement_config_section", values=values)


def full_measurement_config(index, bench_mode_record, bench_mode_status):
    return _call("render_report", "full_measurement_config", index=index, bench_mode_record=bench_mode_record, bench_mode_status=bench_mode_status)


# ── render_sweep_map ──────────────────────────────────────────────────────────
def assert_no_ranking(payload, path=_MISSING):
    return _call("render_sweep_map", "assert_no_ranking", payload=payload, **_given(path=path))


def build(state, stop):
    return _call("render_sweep_map", "build", state=state, stop=stop)


def render_markdown(doc):
    return _call("render_sweep_map", "render_markdown", doc=doc)


# ── publish_benchmark_record ──────────────────────────────────────────────────
def scalar(v):
    return _call("publish_benchmark_record", "scalar", v=v)


def build_yaml(index, verdict):
    return _call("publish_benchmark_record", "build_yaml", index=index, verdict=verdict)


def build_lite_yaml(meta, lite, measured_utc, entry_path, verdict_source):
    return _call("publish_benchmark_record", "build_lite_yaml", meta=meta, lite=lite, measured_utc=measured_utc,
                 entry_path=entry_path, verdict_source=verdict_source)


# ── render_max_report ─────────────────────────────────────────────────────────
def max_report_build_md(idx):
    return _call("render_max_report", "build_md", idx=idx)
