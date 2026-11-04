#!/usr/bin/env python3
"""Query adapter between the Rust classify_cell port and the pending-S3 Python owner `node_role_contract.py`.

DRAFT (temp, for root review). The adapter exists ONLY because the owner of "which nodes take part in a multi-node
serving and is the manifest a valid contract" has not been ported. It contains NO classifier or role rule: it loads
the owner file exactly as the original `classify_cell._node_role_contract` did, calls the owner's own `load_yaml` and
`evaluate_manifest` and KEEPS their results in this process. The Rust program then reads them lazily with mechanical
primitives (dict get, iteration, type/length) - only what the original classifier's code touches. Nothing about a
document is copied unless asked, so metadata the classifier never reads (a huge integer key, a self-referencing
anchor) cannot affect a plan. Which of the original's reports an owner failure becomes is the Rust program's decision.

Protocol: one JSON object per line on stdin, one JSON answer per line on stdout.
  {"op":"load","path":P}              -> {"ok":true,"loaded":true}  |  {"ok":true,"loaded":false,"type":T,"message":M}
        (the module executed / its top level raised - the original swallowed that into `None` = "owner absent")
  {"op":"load_yaml","path":P}         -> {"ok":true,"value":D}  |  {"ok":false,"type":T,"message":M}     contract.load_yaml(P)
  {"op":"evaluate","topology":X}      -> {"ok":true,"value":D}  |  {"ok":false,"type":T,"message":M}
        contract.evaluate_manifest(<the document load_yaml returned>, topology=X)
  {"op":"get","handle":H,"key":K}     -> {"ok":true,"found":bool,"value":D}      dict held under H: K in d / d[K]
  {"op":"items","handle":H}           -> {"ok":true,"value":[D,..]}              list/tuple elements, dict keys
  {"op":"materialize","handle":H}     -> {"ok":true,"value":TYPED}               the whole value (fails on a cycle)
  Any failure of get/items/materialize answers {"ok":false,...}; the caller treats it as a failure of this channel.
The interpreter's settings are inherited, never changed here: the owner's module load, load_yaml and evaluate_manifest
run under the same integer-string limit as the original (integers cross the wire in hexadecimal, so this adapter's own
serialising needs no decimal conversion and no limit change).
A descriptor D is a typed scalar, or ["ref", handle, "list"|"tuple"|"dict", len] for a container held here.
Typed values: every node is [tag, ...]: ["none"] ["bool",b] ["int","hex"] ["float","repr"] ["str",s]
  ["list",[..]] ["tuple",[..]] ["dict",[[key,value],..]] (a key is typed, or ["other",identity,repr] when it is not a str)
  ["bytes","hex"] ["opaque",type_name,repr,truthy].
"""
import importlib.util
import json
import sys

sys.dont_write_bytecode = True
_MODULE = None
_DOC = None
_HANDLES = []


def _answer(payload):
    sys.stdout.write(json.dumps(payload) + "\n")
    sys.stdout.flush()


def _failure(exc):
    _answer({"ok": False, "type": type(exc).__name__, "message": str(exc)})


def _load(path):
    """`classify_cell._node_role_contract` after its `is_file` check (the caller did that)."""
    spec = importlib.util.spec_from_file_location("_classify_cell_node_role_contract", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _scalar(value):
    if value is None:
        return ["none"]
    if isinstance(value, bool):
        return ["bool", value]
    if isinstance(value, int):
        return ["int", format(value, "x")]   # hexadecimal: a power-of-two base is not subject to the decimal digit limit
    if isinstance(value, float):
        return ["float", repr(value)]
    if isinstance(value, str):
        return ["str", value]
    if isinstance(value, bytes):
        return ["bytes", value.hex()]
    return ["opaque", type(value).__name__, repr(value), bool(value)]


def _describe(value):
    for kind in (list, tuple, dict):
        if type(value) is kind:
            _HANDLES.append(value)
            return ["ref", len(_HANDLES) - 1, kind.__name__, len(value)]
    return _scalar(value)


def _key(key):
    """A dict key that is not a str: equal Python keys share an identity - exactly, with no float conversion."""
    if isinstance(key, (bool, int)):
        return ["other", "number:" + format(int(key), "x"), repr(key)]
    if isinstance(key, float):
        if key != key:
            return ["other", "nan:%d" % id(key), repr(key)]      # a nan never equals another key
        if key in (float("inf"), float("-inf")) or not key.is_integer():
            return ["other", "number:" + repr(key), repr(key)]
        return ["other", "number:" + format(int(key), "x"), repr(key)]   # 1.0 is the key 1
    return ["other", type(key).__name__ + ":" + repr(key), repr(key)]


def _typed(value, active):
    for kind in (list, tuple, dict):
        if type(value) is kind:
            if id(value) in active:
                raise ValueError("a self-referencing value cannot be materialised")
            active = active | {id(value)}
            if kind is dict:
                return ["dict", [[["str", k] if isinstance(k, str) else _key(k), _typed(v, active)] for k, v in value.items()]]
            return [kind.__name__, [_typed(item, active) for item in value]]
    return _scalar(value)


def _held(request, kinds):
    value = _HANDLES[request["handle"]]
    if not isinstance(value, kinds):
        raise TypeError("handle %d holds a %s" % (request["handle"], type(value).__name__))
    return value


def _handle(request):
    global _MODULE, _DOC
    op = request["op"]
    if op == "load":
        try:
            _MODULE = _load(request["path"])
        except Exception as exc:  # noqa: BLE001 - the original swallowed any load failure into None
            _MODULE = None
            return {"ok": True, "loaded": False, "type": type(exc).__name__, "message": str(exc)}
        return {"ok": True, "loaded": True}
    if op == "load_yaml":
        _DOC = _MODULE.load_yaml(request["path"])
        return {"ok": True, "value": _describe(_DOC)}
    if op == "evaluate":
        return {"ok": True, "value": _describe(_MODULE.evaluate_manifest(_DOC, topology=request["topology"]))}
    if op == "get":
        held = _held(request, dict)
        key = request["key"]
        return {"ok": True, "found": key in held, "value": _describe(held[key]) if key in held else None}
    if op == "items":
        held = _held(request, (list, tuple, dict))
        return {"ok": True, "value": [_describe(item) for item in held]}
    if op == "materialize":
        return {"ok": True, "value": _typed(_held(request, (list, tuple, dict)), frozenset())}
    raise ValueError("unknown op %r" % (op,))


def main():
    for line in sys.stdin:
        if not line.strip():
            continue
        try:
            _answer(_handle(json.loads(line)))
        except Exception as exc:  # noqa: BLE001 - relayed, class name + message
            _failure(exc)


if __name__ == "__main__":
    main()
