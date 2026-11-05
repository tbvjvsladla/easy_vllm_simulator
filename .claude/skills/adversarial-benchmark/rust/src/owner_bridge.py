"""Closed-operation transport to the Python owners the lite and sweep-meta verifiers still depend on.

Transitional by design: the hint reader (`render_bench_section.py`), the evidence publisher
(`evidence_publisher.py`) and the completion gate (`completion_gate.py`) are Python programs that
have not been ported. This file holds no
verification logic. It loads those files the way the originals did, calls the owner's own
functions for a fixed list of operations, and sends results or exceptions back as JSON lines.
The Rust verifier owns every check, fixture and comparison.

Channel: a socket the verifier created and left open in this process under the descriptor number
given as the first argument. The process's standard input, output and error are the verifier's own
(inherited), so raw `os.write`, children that inherit descriptors 0-2, and reads of standard input
behave exactly as they did in the original process.

Python-level output is not buffered here. `sys.stdout` and `sys.stderr` are replaced by streams that
send every `write` and `flush` to the verifier as an event and wait for its `ack` before returning,
so the owner can not perform its next raw descriptor write before the verifier has applied the
event. The verifier applies stdout events to its own Python-faithful buffer (block, line or
unbuffered, as that process's stream would be) and writes stderr events at once; owner text and the
verifier's own lines are therefore ordered as in one process. The only text kept back here is stderr
text without a line break, as a line-buffered `sys.stderr` keeps it; it goes out at the end of the
operation if the owner does not flush it earlier.

There is no evaluation of caller-supplied code: every operation is a fixed call of an owner's own function.
"""
import importlib.util
import io
import json
import os
import socket
import sys

_CHANNEL = socket.socket(fileno=int(sys.argv[1]))
os.set_inheritable(_CHANNEL.fileno(), False)  # children of an operation must not hold the channel
_LINES = _CHANNEL.makefile("r", encoding="utf-8", newline="\n")
# `python -u` / PYTHONUNBUFFERED: no stream keeps text back.
_UNBUFFERED = bool(os.environ.get("PYTHONUNBUFFERED"))


def _send(record):
    _CHANNEL.sendall((json.dumps(record, ensure_ascii=True) + "\n").encode("ascii"))


def _event(record):
    """Sends one stream event and waits for the verifier's acknowledgement (or its refusal)."""
    _send(record)
    answer = _LINES.readline()
    if not answer:
        raise OSError("the verifier closed the owner session channel")
    answer = json.loads(answer)
    if answer.get("error") is not None:
        raise OSError(answer["error"])


class _Buffer:
    """The binary side of an event stream (`sys.stdout.buffer.write(bytes)`)."""

    def __init__(self, stream):
        self._stream = stream

    def write(self, data):
        self._stream.write(bytes(data).decode("utf-8", "surrogateescape"))
        return len(data)

    def flush(self):
        self._stream.flush()


class _EventStream(io.TextIOBase):
    """`sys.stdout` / `sys.stderr` of an operation: every write is an event for the verifier."""

    encoding = "utf-8"
    errors = "strict"

    def __init__(self, name, descriptor, line_buffered):
        self._name = name
        self._descriptor = descriptor
        self._line_buffered = line_buffered and not _UNBUFFERED
        self._pending = ""
        self.buffer = _Buffer(self)

    def writable(self):
        return True

    def isatty(self):
        return os.isatty(self._descriptor)

    def fileno(self):
        return self._descriptor

    def write(self, text):
        if not isinstance(text, str):
            raise TypeError("write() argument must be str, not %s" % type(text).__name__)
        if not self._line_buffered:
            _event({"event": self._name, "text": text})
            return len(text)
        self._pending += text
        if "\n" in text or "\r" in text:
            self.flush_pending()
        return len(text)

    def flush_pending(self):
        if self._pending:
            text, self._pending = self._pending, ""
            _event({"event": self._name, "text": text})

    def flush(self):
        self.flush_pending()
        _event({"event": "flush", "stream": self._name})


_STDOUT = _EventStream("out", 1, False)  # the verifier's own buffer models stdout buffering
_STDERR = _EventStream("err", 2, True)   # a line-buffered stream (write-through when unbuffered)

_MODULES = {}
_MODULE_NAMES = {"rbs": "_lite_rbs", "ep": "_lite_evidence_publisher", "gate": "_gate_cg"}
_RBS_CONSTANTS = ("MEASUREMENT_CONFIG_TITLE", "MEASUREMENT_CONFIG_KEYS", "LITE_SECTION_TITLE",
                  "REPORT_HEADER")
_EP_CONSTANTS = ("LITE_BENCH_MODE",)


def _load(role, path):
    spec = importlib.util.spec_from_file_location(_MODULE_NAMES[role], path)
    module = importlib.util.module_from_spec(spec)
    sys.path.insert(0, os.path.dirname(path))
    spec.loader.exec_module(module)
    _MODULES[role] = module


def _operation(request):
    op = request["op"]
    if op == "start":
        # Running a script puts its directory first on the path; a `-c` session has the working directory there.
        sys.path[0] = request["scripts_dir"]
        return None
    if op == "load":
        _load(request["role"], request["path"])
        return None
    if op == "rbs_parse_measurement_config":
        return _MODULES["rbs"].parse_measurement_config(request["text"])
    if op == "rbs_is_lite_report":
        return _MODULES["rbs"].is_lite_report(request["text"])
    if op == "rbs_lite_metrics_present":
        return _MODULES["rbs"].lite_metrics_present(request["text"])
    if op == "rbs_render_lite":
        rbs = _MODULES["rbs"]
        return rbs.render_lite(rbs.parse_lite_report(request["text"]), request["name"])
    if op == "rbs_parse_report_level_count":
        return len(_MODULES["rbs"].parse_report(request["text"])["levels"])
    if op == "rbs_parse_sweep_report":
        # The late import of the repeat-axis verifier's N4: module name `render_bench_section`, its directory inserted
        # first on `sys.path` at the moment of the call (not at session start, and not under `_lite_rbs`).
        sys.path.insert(0, request["directory"])
        import render_bench_section
        parsed = render_bench_section.parse_report(request["text"])
        levels = [lv.get("level") for lv in parsed["levels"]]
        has_truncated = "truncated" in parsed
        return {"levels": levels, "has_truncated": has_truncated,
                "truncated": parsed["truncated"] if has_truncated else None, "shown": repr(parsed)}
    if op == "rbs_constant":
        if request["name"] not in _RBS_CONSTANTS:
            raise ValueError("not an owner constant of the reader: %r" % request["name"])
        return getattr(_MODULES["rbs"], request["name"])
    if op == "ep_bench_name_matches":
        return _MODULES["ep"]._BENCH_SRC_NAME_RE[request["kind"]].match(request["name"]) is not None
    if op == "ep_report_measurement_config":
        return _MODULES["ep"].report_measurement_config(request["text"])[0]
    if op == "ep_constant":
        if request["name"] not in _EP_CONSTANTS:
            raise ValueError("not an owner constant of the publisher: %r" % request["name"])
        return getattr(_MODULES["ep"], request["name"])
    if op == "gate_parse_flat_certificate":
        fields, ok = _MODULES["gate"].parse_flat_certificate(request["text"])
        return [fields, ok]
    if op == "gate_certificate_run_key_present":
        return _MODULES["gate"].certificate_run_key(request["fields"]) is not None
    raise ValueError("unknown operation: %r" % op)


def _exit_payload(code):
    """What `SystemExit(code)` does to the process: None or an integer (bool included) is a status, anything else
    is printed as its `str`."""
    if code is None:
        return {"status": 0}
    if isinstance(code, int):
        return {"status": int(code)}
    return {"message": str(code)}


def _failure(error):
    kind = type(error)
    # What a traceback prints: module-qualified unless the class is a builtin or lives in `__main__`.
    qualified = kind.__qualname__ if kind.__module__ in ("builtins", "__main__") else (
        kind.__module__ + "." + kind.__qualname__)
    if isinstance(error, SystemExit):
        code = error.code
        return {"class": "SystemExit", "message": code if isinstance(code, str) else "",
                "repr": repr(error), "qualified": qualified,
                "is_exception": isinstance(error, Exception), "is_system_exit": isinstance(error, SystemExit),
                "exit": _exit_payload(code)}
    return {"class": kind.__name__, "message": str(error), "repr": repr(error), "qualified": qualified,
            "is_exception": isinstance(error, Exception), "is_system_exit": isinstance(error, SystemExit), "exit": None}


def _serve():
    sys.stdout, sys.stderr = _STDOUT, _STDERR
    while True:
        line = _LINES.readline()
        if not line:
            return
        request = json.loads(line)
        try:
            try:
                reply = {"ok": True, "value": _operation(request)}
            except BaseException as error:  # SystemExit and every exception travel back to the verifier
                reply = {"ok": False, "exc": _failure(error)}
            _STDERR.flush_pending()  # text a line-buffered stream still holds goes out with the operation
            text = json.dumps(reply, ensure_ascii=True)
        except (TypeError, ValueError, OSError) as error:  # an owner result JSON cannot carry, or a refused event
            reply = {"ok": False, "exc": _failure(error)}
            text = json.dumps(reply, ensure_ascii=True)
        _send_text(text)


def _send_text(text):
    _CHANNEL.sendall((text + "\n").encode("ascii"))


_serve()
