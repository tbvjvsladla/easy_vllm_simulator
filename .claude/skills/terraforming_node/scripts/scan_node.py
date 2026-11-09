#!/usr/bin/env python3
# scan_node.py — terraforming_node 진입 루틴의 결정론 스캔 코어. **P1 네이티브화 단계의 얇은 import 어댑터**(전체 스캐너 아님 · Python-free 아님).
#
# P1 의 업무(공개 16개 이름과 도달 가능한 헬퍼: `valid_host` · `_ver_tuple` · `_yaml_str` · `parse_gpu_memory_line` · `check_model_env` ·
# `derive_platform_preset` · `assert_homogeneity` · `default_env_interconnect_path` · `emit_gate` · `evaluate_gate` ·
# `emission_blockers` · `build_sub_manifest` · `emit_manifest_block` · `cross_validate` · `read_manifest_field` · `read_manifest_topology` ·
# `_read_manifest_line` · `_parse_compose_nccl` · `_parse_env_file_nccl` · `_parse_nccl_lines`)는 Rust 가 한다(순수 업무는 공유 T
# `role-contract::scan_node*`, 파일·시계 경계와 비공개 브리지는 `adversarial-benchmark/rust` 의 `scan_node` 실행파일). 이 파일은 **기존 Python
# importer** 가 같은 이름·서명으로 계속 부르게 하는 임시 어댑터이며 업무 로직이 없다: 값의 타입 전달(marshalling), 호출자 객체 참조의 기계적 조립
# (결과 plan 의 `argref` → 호출자의 실제 객체), 정확한 예외(클래스·인자·`AttributeError.name/obj`·`__cause__`) 재구성, 서명, 그리고 아래의 기계적
# export 배관만 가진다.
#
# P1 밖(하드웨어 프로브·SSH/네트워크 수집·인자 파서·`main`·`_self_test` 등 나머지 전부)은 **명시적 compat 경로**다: 모듈을 올릴 때 정확한 원본 소유자
# `scan_node_original.py`(바이트 동일, sha256 2d9e8a86…)를 모듈 이름 `scan_node` 로 실행해(스택 프레임의 파일은 원본 그대로) 그 공개·비공개 이름을 이
# 모듈의 이름공간에 **배관**한다(`from scan_node import *` 를 포함한 보통의 export 가 원본과 같다 · 이 어댑터가 정의한 P1 이름만 네이티브다). 원본 소유자
# 안의 P1 함수는 원본 Python 그대로 돈다(compat 의 전체 CLI·`--self-test` 는 P1 네이티브의 증거가 아니다). `ManifestUnreadable` 은 공개 클래스 하나이고
# compat 쪽 전역에도 같은 클래스가 꽂힌다(원본 소스는 바꾸지 않는다). 네이티브 호출이 실패하면 **Python 으로 재시도하지 않는다**(실행파일 부재 = 상태 2 와
# 빌드 안내로 눈에 띄게 실패). `SCAN_NODE_ROUTE_OBSERVER` 에 경로가 남는다(`NativeBridge` · `CompatOriginalLoad` · `CompatOriginalCli`).
#
# 남은 잔여(숨기지 않는다): ① 어댑터가 전달하지 않는 값(정확한 str·int·bool·float·None·bytes·list·tuple·문자열 키 dict 밖의 객체, 자기 참조 컨테이너)은
# 업무 전에 `NotImplementedError` 로 거절한다. ② in-process 예외의 traceback 프레임(원본 소스 줄)은 재현하지 않는다(U2 · 열림 · 위조하지 않는다).
# """
# (The module docstring and `__doc__` are the exact original owner's, plumbed below.)
from __future__ import annotations
import importlib.util as _sn_importlib_util
import json as _sn_json
import os as _sn_os
import subprocess as _sn_subprocess
import sys as _sn_sys
import types as _sn_types

_SN_HERE = _sn_os.path.dirname(_sn_os.path.abspath(__file__))
_SN_LAUNCHER = _sn_os.path.join(_SN_HERE, "scan_node.sh")
_SN_ORIGINAL_PATH = _sn_os.path.join(_SN_HERE, "scan_node_original.py")
_SN_PROTOCOL = "scan-node-bridge/1"
_SN_BUILTIN_EXCEPTIONS = {
    "OSError": OSError, "UnicodeDecodeError": UnicodeDecodeError, "ValueError": ValueError, "TypeError": TypeError,
    "AttributeError": AttributeError, "KeyError": KeyError, "OverflowError": OverflowError,
}


class ManifestUnreadable(Exception):
    """(the original's docstring is carried over from the exact original owner when it is loaded below)"""


_SN_EXCEPTIONS = dict(_SN_BUILTIN_EXCEPTIONS, ManifestUnreadable=ManifestUnreadable)


def _sn_observe(event, route, shape, outcome=None):
    """One JSON line of the route observation (the native side writes its own for bridge calls)."""
    path = _sn_os.environ.get("SCAN_NODE_ROUTE_OBSERVER")
    if not path:
        return
    record = {"event": event, "route": route, "pid": _sn_os.getpid(), "ppid": _sn_os.getppid(),
              "owner_path": _sn_os.path.abspath(__file__), "shape": shape, "outcome": outcome}
    with open(path, "a", encoding="utf-8") as sink:
        sink.write(_sn_json.dumps(record, ensure_ascii=True) + "\n")


# ── typed marshalling with shared identity (no decision is taken here) ──────────────────────

def _sn_unsupported(what):
    raise NotImplementedError("scan_node bridge does not carry %s (the original's behaviour is not reproduced)" % what)


class _SnCall:
    """The caller's own objects that carry identity (containers and NaN floats), numbered in traversal order for one bridge call.
    The answer plan refers to them by number (`argref`); the actual caller objects are what is handed back."""

    def __init__(self):
        self.ids = {}
        self.objs = []

    def plain(self, value):
        """A dict key: the text only (a key's object identity is not carried)."""
        if type(value) is not str:
            _sn_unsupported("a dict with non-str keys")
        return {"t": "str", "v": value}

    def wire(self, value, _open=()):
        kind = type(value)
        if value is None:
            return {"t": "none"}
        if kind is bool:
            return {"t": "bool", "v": value}
        if kind not in (int, str, bytes, float, list, tuple, dict):
            _sn_unsupported("a value of type %s" % kind.__name__)
        # every other object is numbered (identity travels): a repeat visit is a `ref`; the answer plan may hand the object back
        known = self.ids.get(id(value))
        if known is not None:
            if id(value) in _open:
                _sn_unsupported("a self-referencing container")
            return {"t": "ref", "id": known}
        number = len(self.objs)
        self.ids[id(value)] = number
        self.objs.append(value)
        if kind is int:
            return {"t": "int", "hex": ("-" if value < 0 else "+") + format(abs(value), "x"), "id": number}
        if kind is str:
            return {"t": "str", "v": value, "id": number}
        if kind is bytes:
            return {"t": "bytes", "hex": value.hex(), "id": number}
        if kind is float:
            return {"t": "float", "repr": repr(value), "id": number}
        inner = _open + (id(value),)
        if kind is dict:
            return {"t": "dict", "id": number,
                    "v": [[self.plain(key), self.wire(item, inner)] for key, item in value.items()]}
        return {"t": "list" if kind is list else "tuple", "id": number,
                "v": [self.wire(item, inner) for item in value]}

    def unwire(self, item):
        tag = item["t"]
        if tag == "argref":
            return self.objs[item["id"]]
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
            return [self.unwire(x) for x in item["v"]]
        if tag == "tuple":
            return tuple(self.unwire(x) for x in item["v"])
        if tag == "dict":
            return {self.unwire(key): self.unwire(value) for key, value in item["v"]}
        raise RuntimeError("scan_node bridge answered an unknown wire type %r" % tag)

    def rebuild(self, exc):
        cls = _SN_EXCEPTIONS.get(exc["cls"])
        if cls is None:
            raise RuntimeError("scan_node bridge reported an unknown exception class %r" % exc["cls"])
        error = cls(*[self.unwire(x) for x in exc["ctor"]], **{k: self.unwire(v) for k, v in exc["ctor_kwargs"].items()})
        if exc["cause"] is not None:
            cause = self.rebuild(exc["cause"])
            # `raise X from cause` inside the handler of `cause`: the cause and the context are the same object, context suppressed
            error.__cause__ = cause
            error.__context__ = cause
        return error


def _sn_call(op, **params):
    """One request to the private bridge. The answer is returned or raised as the original's own outcome; a transport failure is a
    `RuntimeError` (there is no Python route and no retry)."""
    call = _SnCall()
    request = {"protocol": _SN_PROTOCOL, "op": op, "params": {name: call.wire(value) for name, value in params.items()}}
    # a caller's availability-probe variable is not meant for the bridge child (the launcher also ignores it in bridge mode)
    env = {k: v for k, v in _sn_os.environ.items() if k != "SCAN_NODE_VALIDATE_ONLY"}
    env.update(SCAN_NODE_BRIDGE="1", SCAN_NODE_OWNER_PATH=_sn_os.path.abspath(__file__))
    process = _sn_subprocess.run(["/bin/bash", _SN_LAUNCHER], input=_sn_json.dumps(request, ensure_ascii=True).encode("ascii"),
                                 capture_output=True, env=env)
    if process.returncode != 0:
        raise RuntimeError("[scan_node] native bridge failed (status %d): %s"
                           % (process.returncode, process.stderr.decode("utf-8", "replace").rstrip()))
    answer = _sn_json.loads(process.stdout.decode("ascii"))
    if answer.get("protocol") != _SN_PROTOCOL:
        raise RuntimeError("[scan_node] native bridge speaks another protocol; rebuild it")
    status = answer["status"]
    if status == "raised":
        raise call.rebuild(answer["exc"])
    if status == "unsupported":
        _sn_unsupported(answer["reason"])
    return call.unwire(answer["value"])


# ── P1: the original's public names and signatures ──────────────────────────────────────────

def valid_host(value) -> bool:
    return _sn_call("valid_host", value=value)


def _ver_tuple(s):
    return _sn_call("_ver_tuple", s=s)


def _yaml_str(value) -> str:
    return _sn_call("_yaml_str", value=value)


def parse_gpu_memory_line(line: str) -> dict:
    return _sn_call("parse_gpu_memory_line", line=line)


def check_model_env(model_source: str | None, egress: str | None,
                    nas_reachable: bool | None = None, hf_token_present: bool | None = None) -> dict:
    return _sn_call("check_model_env", model_source=model_source, egress=egress, nas_reachable=nas_reachable,
                    hf_token_present=hf_token_present)


def read_manifest_field(path: str, key: str) -> str | None:
    return _sn_call("read_manifest_field", path=path, key=key)


def assert_homogeneity(local: dict, peer: dict) -> dict:
    return _sn_call("assert_homogeneity", local=local, peer=peer)


def read_manifest_topology(path: str = "manifest.yaml") -> str | None:
    return _sn_call("read_manifest_topology", path=path)


def derive_platform_preset(gpu_model) -> tuple[str, str]:
    return _sn_call("derive_platform_preset", gpu_model=gpu_model)


def build_sub_manifest(result: dict, peer_hw: dict, main_manifest: dict, scanned_at: str | None = None,
                       egress_peer: str | None = None) -> dict:
    return _sn_call("build_sub_manifest", result=result, peer_hw=peer_hw, main_manifest=main_manifest,
                    scanned_at=scanned_at, egress_peer=egress_peer)


def emit_manifest_block(result: dict) -> str:
    return _sn_call("emit_manifest_block", result=result)


def evaluate_gate(*, declared, ic_present, peer_given, peer_reachable, bandwidth, bw_floor,
                  per_port_gbps=None, per_port_floor=None,
                  branch=None, branch_topo=None, mani_topo=None, mani_path="manifest.yaml",
                  peer_ip=None, egress_self=None, egress_peer=None,
                  model_env_ok=None, model_env_reason=None, manifest_read_error=None):
    return _sn_call("evaluate_gate", declared=declared, ic_present=ic_present, peer_given=peer_given,
                    peer_reachable=peer_reachable, bandwidth=bandwidth, bw_floor=bw_floor, per_port_gbps=per_port_gbps,
                    per_port_floor=per_port_floor, branch=branch, branch_topo=branch_topo, mani_topo=mani_topo,
                    mani_path=mani_path, peer_ip=peer_ip, egress_self=egress_self, egress_peer=egress_peer,
                    model_env_ok=model_env_ok, model_env_reason=model_env_reason,
                    manifest_read_error=manifest_read_error)


def emit_gate(emit_manifest: bool, topology: str) -> int:
    return _sn_call("emit_gate", emit_manifest=emit_manifest, topology=topology)


def emission_blockers(result: dict) -> list[str]:
    return _sn_call("emission_blockers", result=result)


def default_env_interconnect_path(compose_path: str) -> str:
    return _sn_call("default_env_interconnect_path", compose_path=compose_path)


def cross_validate(ic: dict, compose_path: str, env_interconnect_path: str | None = None) -> dict:
    return _sn_call("cross_validate", ic=ic, compose_path=compose_path, env_interconnect_path=env_interconnect_path)


def _read_manifest_line(path: str, prefix: str) -> str | None:
    return _sn_call("_read_manifest_line", path=path, prefix=prefix)


def _parse_compose_nccl(path: str) -> dict:
    return _sn_call("_parse_compose_nccl", path=path)


def _parse_env_file_nccl(path: str) -> dict:
    return _sn_call("_parse_env_file_nccl", path=path)


def _parse_nccl_lines(path: str, sep: str) -> dict:
    return _sn_call("_parse_nccl_lines", path=path, sep=sep)


# ── everything outside P1: the exact original owner, plumbed in before any business (no retry, no native fallback) ────────────

def _sn_load_legacy():
    """Runs the exact original owner under the module name `scan_node` (its frames keep the original file) and returns its namespace."""
    _sn_observe("start", "CompatOriginalLoad", "namespace")
    legacy = _sn_types.ModuleType("scan_node")
    legacy.__file__ = _SN_ORIGINAL_PATH
    with open(_SN_ORIGINAL_PATH, "rb") as handle:
        code = compile(handle.read(), _SN_ORIGINAL_PATH, "exec")
    exec(code, legacy.__dict__)
    return legacy


_SN_LEGACY = _sn_load_legacy()
# one public `ManifestUnreadable`: the original's docstring, and the same class for the legacy globals (the original source is untouched)
ManifestUnreadable.__doc__ = _SN_LEGACY.ManifestUnreadable.__doc__
_SN_LEGACY.ManifestUnreadable = ManifestUnreadable
_sn_plumbed = 0
for _sn_name, _sn_value in list(vars(_SN_LEGACY).items()):
    # the ordinary exports of the original (functions, constants, imported modules, private helpers), except what P1 ports
    if not (_sn_name.startswith("__") and _sn_name.endswith("__")) and _sn_name not in globals():
        globals()[_sn_name] = _sn_value
        _sn_plumbed += 1
# the ported functions carry the exact original function docs (mechanical: the original owner's `__doc__`, or none when it has none)
for _sn_own_name, _sn_own in list(globals().items()):
    _sn_original = vars(_SN_LEGACY).get(_sn_own_name)
    if (isinstance(_sn_own, _sn_types.FunctionType) and isinstance(_sn_original, _sn_types.FunctionType)
            and _sn_own is not _sn_original):
        _sn_own.__doc__ = _sn_original.__doc__
del _sn_own_name, _sn_own, _sn_original
__doc__ = _SN_LEGACY.__doc__
_sn_observe("end", "CompatOriginalLoad", "namespace", "plumbed:%d" % _sn_plumbed)
del _sn_name, _sn_value


def main() -> int:
    # CLI: not native in this phase. The exact original owner runs it (same argv, environment and process). (No docstring: the
    # original's `main` has none, and the function docs are the original's.)
    _sn_observe("start", "CompatOriginalCli", "main")
    try:
        status = _SN_LEGACY.main()
    except SystemExit as stop:
        _sn_observe("end", "CompatOriginalCli", "main", "exit:%s" % (stop.code,))
        raise
    _sn_observe("end", "CompatOriginalCli", "main", "exit:%s" % status)
    return status


if __name__ == "__main__":
    _sn_sys.exit(main())
