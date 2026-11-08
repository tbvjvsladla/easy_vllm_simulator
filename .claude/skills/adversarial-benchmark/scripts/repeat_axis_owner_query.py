#!/usr/bin/env python3
"""Query adapter between the Rust repeat_axis port and the pending-S4 Python owner `campaign_init.py`.

The adapter exists ONLY because the owner of "which campaign is active /
where a campaign lives / which ids are reserved" has not been ported. It contains NO repeat logic: it does not
load budgets, resolve precedence, summarize or aggregate. It loads the owner file exactly as the original
`repeat_axis._campaign_init_module` did, and answers single queries by calling the owner's own functions or reading its
attributes, relaying the result or the exception (class name + str(exception)).

Protocol: one JSON object per line on stdin, one JSON answer per line on stdout.
  {"op":"load","path":P}                         -> {"ok":true}
  {"op":"active_campaign_id"}                    -> {"ok":true,"value":mod.active_campaign_id()}
  {"op":"attribute","name":N}                    -> {"ok":true,"value":getattr(mod,N)}   (AttributeError relayed)
  {"op":"attribute_or_empty","name":N}           -> {"ok":true,"value":getattr(mod,N,())}
  {"op":"derive_root_path","camp_id":JSON}       -> {"ok":true,"value":str(base)} with base = mod.derive_path("root", camp_id=JSON);
        `base / "campaign.yaml"` and `base.is_dir` are touched first so that a result of the wrong type fails here exactly as the
        original's next two operations on it did. The directory test itself is the caller's.
Failure: {"ok":false,"type":ClassName,"message":str(exc)}.
"""
import importlib.util
import json
import sys
from pathlib import Path

sys.dont_write_bytecode = True
_MODULE = None


def _answer(payload):
    sys.stdout.write(json.dumps(payload) + "\n")
    sys.stdout.flush()


def _failure(exc):
    _answer({"ok": False, "type": type(exc).__name__, "message": str(exc)})


def _load(path):
    """`repeat_axis._campaign_init_module` after the `is_file` check (the caller did that)."""
    spec = importlib.util.spec_from_file_location("_repeat_axis_campaign_init", Path(path))
    if spec is None or spec.loader is None:
        raise ImportError("no import spec for %s" % path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _handle(request):
    global _MODULE
    op = request["op"]
    if op == "load":
        _MODULE = _load(request["path"])
        return {"ok": True}
    if op == "active_campaign_id":
        return {"ok": True, "value": _MODULE.active_campaign_id()}
    if op == "attribute":
        return {"ok": True, "value": getattr(_MODULE, request["name"])}
    if op == "attribute_or_empty":
        return {"ok": True, "value": getattr(_MODULE, request["name"], ())}
    if op == "derive_root_path":
        base = _MODULE.derive_path("root", camp_id=request["camp_id"])
        # The original's next two operations on the owner's return value: `base / "campaign.yaml"` and
        # `base.is_dir()`. Their failures (a str result, an object without is_dir) are the owner's type contract.
        base / "campaign.yaml"
        base.is_dir
        return {"ok": True, "value": str(base)}
    raise ValueError("unknown adapter operation %r" % (op,))


def main():
    for line in sys.stdin:
        if not line.strip():
            continue
        try:
            reply = _handle(json.loads(line))
            text = json.dumps(reply)
        except Exception as exc:  # every owner exception is relayed, not interpreted
            _failure(exc)
            continue
        sys.stdout.write(text + "\n")
        sys.stdout.flush()
    return 0


if __name__ == "__main__":
    sys.exit(main())
