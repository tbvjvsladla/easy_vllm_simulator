#!/usr/bin/env python3
"""manifest_contract.py — 테라포밍-완수 Flag 게이트의 결정론 리더 (plan_26063018).

헌법 §테라포밍-완수 Flag 게이트 따름정리의 **결정론 백스톱**. 3 런타임 스킬
(upstream-version-watch · vllm-recipe-explorer · adversarial-benchmark)의 *작업 스크립트*가
진입 시 이걸 호출해 Flag(terraforming.complete)를 확인한다 — 미발급이면 비0종료(작업 거부 → 에이전트 info-only).

설계:
- 단일계약: `output/<topology>/manifest.yaml` 를 읽는다(topology = 현재 git 브랜치가 선택하는 *통로*; --topology 로 override).
- **보수적**: `terraforming.complete == true` AND `terraforming.branch_verified == true` AND 필수 HW사실(topology·gpus_per_node) 존재해야 통과(fail-closed-on-flag).
- **TP**(manifest 파생): topology=single→gpus_per_node(노드 배수 1 고정 — sub 는 control 피어) ·
  그 외 nodes 있으면 len(nodes)×gpus_per_node · nodes 비면 None
  (roofline.py:196-209 동일 패턴 — **git 브랜치 폴백 없음**; config override·최종폴백 1 은 *호출자*가 적용).
- stdlib + yaml 만 · 외부 네트워크 호출 ✗(결정론 스크립트 평면 — 헌법 §금지).

CLI:
  manifest_contract.py [--topology single|multi] [--repo .] [--require-flag] [--json]
  exit 0 = Flag valid · 3 = manifest 부재 · 4 = Flag 미발급/미검증 · 5 = 필수필드 누락 · 2 = 사용오류

호출자 관용구(런타임 스킬 작업스크립트 진입):
  python3 .../manifest_contract.py --topology "$T" --repo "$REPO" --require-flag || {
      echo "Flag 미발급 — 테라포밍 먼저(info-only)"; exit 4; }
"""
import argparse  # noqa: F401 — the original's public import-time dependency (its module name is part of the module facts); the adapter itself does not use it
import json
import os
import subprocess
import sys

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_NO_MANIFEST = 3
EXIT_NO_FLAG = 4
EXIT_MISSING_FIELD = 5
#: 인자 topology 와 **실제 사실**(manifest 자신의 선언 · 체크아웃 브랜치)이 어긋난다.
#: 2026-09-12 신설(plan_26091210 A2 · policy BRANCH_CONSTITUTION_LAYERING C5). 종전에는 두 구멍이
#: 열려 있었다 — ⓐ `evaluate_contract` 가 manifest 안의 `topology` 를 인자와 **한 번도 대조하지
#: 않았고** ⓑ `multi-node` 체크아웃에서 `--topology single --require-flag` 가 exit 0 을 냈다(실측).
EXIT_TOPOLOGY_MISMATCH = 6

VALID_MODES = ("managed", "ephemeral", "custom")

# --- 호출자용 정본 redirect 템플릿 (Flag 미발급 시 페르소나가 출력) ---
REDIRECT_TEMPLATE = (
    "HW스캔이 덜 되어(Flag 미발행) HW 스펙(GPU·OS)을 알기 어려워, 모델의 정확한 서빙전략을 "
    "세우기 어렵습니다. terraforming_node 로 ① HW스캔 + ② 모델 다운로드 전략(관리 NAS 경로? "
    "컨테이너 임시 다운로드(컨테이너 down 시 삭제)? 특정 경로 저장·마운트?)을 먼저 정합시다."
)

# ── 이 파일은 얇은 IMPORT 어댑터다 ─────────────────────────────────────────────────────────────────
# 판정(Flag·reason·exit code·TP·경고·역할 키·topology 비교·git 브랜치·main 인자 분류)은 모두 Rust(`manifest_contract` 실행파일의
# 비공개 브리지 · 순수 업무는 role-contract)가 한다. 이 파일은 호출자의 ORIGINAL 객체를 타입 그래프로 전달하고(별칭·순환·NaN 키·
# YAML 전용 타입 보존), 돌려받은 결과 계획을 **기계적으로** 조립하며, 기록된 예외를 같은 클래스·인자로 다시 일으킨다.
# 원본 바이트는 옆의 manifest_contract_compat_owner.py 에 그대로 있다: `-h`·`--self-test`·약어·사용 오류 같은 사전 분류된 레거시
# 경로와 업무 없는 이름(`_git_branch`·`branch_topology_conflict`·`_self_test`)만 그 파일로 간다(네이티브 실패 뒤 Python 재시도 없음).
# 잔여: U2(in-process traceback 프레임은 이 어댑터의 것) · U3(아래 타입 집합 밖은 업무 전에 NotImplementedError).
def _build():
    """Creates the nine functions of the original API together with every implementation helper, so that the helpers live in the CLOSURE of
    the returned functions and not in the module namespace (the module then carries exactly the names, imports and constants of the original). Nothing is
    intercepted: the module has no `__getattr__`/`__dir__`, the functions are ordinary module-level function objects, and this builder is deleted after it ran."""
    _HERE = os.path.dirname(os.path.abspath(__file__))
    _LAUNCHER = os.path.join(_HERE, "manifest_contract.sh")
    _COMPAT_OWNER = os.path.join(_HERE, "manifest_contract_compat_owner.py")
    _PROTOCOL = "manifest-contract-bridge/1"
    _compat_module = None


    def _unsupported(what):
        raise NotImplementedError("manifest_contract bridge does not carry %s (the original's behaviour is not reproduced here)" % what)


    def _hex_text(text):
        return text.encode("utf-8", "surrogatepass").hex()


    class _Wire:
        """Python object -> typed graph wire (traversal-order ids; a repeated container is a ref: sharing and cycles stay visible)."""

        def __init__(self):
            self.ids = {}
            self.keep = []
            self.nans = {}

        def _new(self, value):
            self.keep.append(value)
            self.ids[id(value)] = len(self.ids) + 1
            return self.ids[id(value)]

        def __call__(self, value):
            kind = type(value)
            if value is None:
                return {"t": "none"}
            if kind is bool:
                return {"t": "bool", "v": value}
            if kind is int:
                return {"t": "int", "sign": "-" if value < 0 else "+", "hex": format(abs(value), "x")}
            if kind is float:
                wire = {"t": "float", "repr": repr(value)}
                if value != value:
                    if id(value) not in self.nans:
                        self.keep.append(value)
                        self.nans[id(value)] = len(self.nans) + 1
                    wire["nan_id"] = self.nans[id(value)]
                return wire
            if kind is str:
                return {"t": "str", "utf8_surrogatepass_hex": _hex_text(value), "codepoints": len(value)}
            if kind is bytes:
                return {"t": "bytes", "hex": value.hex()}
            import datetime
            if kind is datetime.datetime:
                return {"t": "datetime", "iso": value.isoformat()}
            if kind is datetime.date:
                return {"t": "date", "iso": value.isoformat()}
            if kind in (list, tuple, dict, set):
                if id(value) in self.ids:
                    return {"t": "ref", "id": self.ids[id(value)]}
                ident = self._new(value)
                if kind is dict:
                    return {"t": "dict", "id": ident, "pairs": [[self(key), self(item)] for key, item in list(value.items())]}
                if kind is set:
                    return {"t": "set", "id": ident, "members": [self(member) for member in value]}
                return {"t": "list" if kind is list else "tuple", "id": ident, "v": [self(item) for item in value]}
            _unsupported("a value of type %s" % kind.__name__)


    def _path_wire(wire, path):
        """A path argument: str, or an os.PathLike whose fspath is str (bytes and file descriptors are not carried)."""
        kind = type(path)
        if kind is not str and kind is not bytes and path is not None and hasattr(kind, "__fspath__"):
            path = os.fspath(path)
        if type(path) is not str:
            _unsupported("a path of type %s" % type(path).__name__)
        return wire(path)


    class _Unwire:
        """typed graph wire -> Python objects (a ref resolves to the object registered under that id: sharing and cycles are real)."""

        def __init__(self):
            self.objects = {}
            self.nans = {}

        def __call__(self, wire):
            tag = wire["t"]
            if tag == "none":
                return None
            if tag == "bool":
                return wire["v"]
            if tag == "int":
                value = int(wire["hex"], 16)
                return -value if wire["sign"] == "-" else value
            if tag == "float":
                if "nan_id" in wire:
                    return self.nans.setdefault(wire["nan_id"], float("nan"))
                return float(wire["repr"])
            if tag == "str":
                return bytes.fromhex(wire["utf8_surrogatepass_hex"]).decode("utf-8", "surrogatepass")
            if tag == "bytes":
                return bytes.fromhex(wire["hex"])
            if tag == "date":
                import datetime
                return datetime.date.fromisoformat(wire["iso"])
            if tag == "datetime":
                import datetime
                return datetime.datetime.fromisoformat(wire["iso"])
            if tag == "ref":
                return self.objects[wire["id"]]
            if tag == "list":
                out = self.objects[wire["id"]] = []
                out.extend(self(item) for item in wire["v"])
                return out
            if tag == "dict":
                out = self.objects[wire["id"]] = {}
                for key, item in wire["pairs"]:
                    out[self(key)] = self(item)
                return out
            if tag == "set":
                out = self.objects[wire["id"]] = set()
                for member in wire["members"]:
                    out.add(self(member))
                return out
            if tag == "tuple":
                out = tuple(self(item) for item in wire["v"])
                self.objects[wire["id"]] = out
                return out
            raise RuntimeError("manifest_contract bridge answered an unknown wire type %r" % tag)


    def _ask(op, wires):
        """One request to the private bridge: the answer record (an exception of the original is DATA in it, never a transport failure).

    The request travels in a private temporary file (mode 0600, removed right after the answer): the bridge gets NO descriptor of the
    request and its standard descriptors 0-2 stay the CALLER's, which is what the original's `git` subprocess inherited (the original ran
    with the interpreter's `close_fds`, so it inherited nothing else either). The environment is the caller's plus the two private
    bridge variables; the caller's own value (or unset state) of exactly the plumbing variables this call mutates, and of the others the
    launcher knows, travels as `MANIFEST_CONTRACT_CALLER_<name>` so the native `git` child gets the caller's environment back. Only
    those plumbing names are read; no other variable of the caller is copied anywhere."""
        import tempfile
        data = json.dumps({"v": 1, "op": op, "args": wires}, ensure_ascii=True).encode("ascii")
        env = dict(os.environ)
        env["MANIFEST_CONTRACT_CALLER_ENV"] = "1"
        for suffix in ("BRIDGE", "BRIDGE_REQUEST", "COMPAT_OWNER", "PUBLIC_ARGV0", "PYTHON", "VALIDATE_ONLY"):
            carrier = "MANIFEST_CONTRACT_CALLER_" + suffix
            if "MANIFEST_CONTRACT_" + suffix in os.environ:
                env[carrier] = os.environ["MANIFEST_CONTRACT_" + suffix]
            else:
                env.pop(carrier, None)
        handle, request_path = tempfile.mkstemp(prefix="manifest-contract-request-", suffix=".json")
        try:
            with os.fdopen(handle, "wb") as sink:
                sink.write(data)
            env["MANIFEST_CONTRACT_BRIDGE"] = "1"
            env["MANIFEST_CONTRACT_BRIDGE_REQUEST"] = request_path
            process = subprocess.Popen(["/bin/bash", _LAUNCHER], stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
            stdout, stderr = process.communicate()
        finally:
            try:
                os.unlink(request_path)
            except OSError:
                pass
        if process.returncode != 0:
            raise RuntimeError("[manifest_contract] native bridge failed (status %d): %s"
                               % (process.returncode, stderr.decode("utf-8", "replace").rstrip()))
        answer = json.loads(stdout.decode("ascii"))
        if answer.get("protocol") != _PROTOCOL:
            raise RuntimeError("[manifest_contract] native bridge speaks another protocol; rebuild it")
        return answer["answer"]


    def _walk(path, args):
        """The ORIGINAL object an AttributeError names: arguments[i], then keys/indices of it."""
        obj = None
        for step in path:
            if "arg" in step:
                obj = args[step["arg"]]
            elif "key" in step:
                obj = obj.get(step["key"])
            else:
                obj = obj[step["index"]]
        return obj


    _CLASSES = {name: getattr(__import__("builtins"), name) for name in (
        "AttributeError", "TypeError", "ValueError", "RecursionError", "OverflowError", "KeyError", "OSError")}
    _YAML_MODULES = {"ComposerError": "composer", "ConstructorError": "constructor", "ParserError": "parser",
                     "ScannerError": "scanner", "ReaderError": "reader"}


    def _mark(yaml, item, unwire):
        if item is None:
            return None
        return yaml.error.Mark(unwire(item["name"]), item["index"], item["line"], item["column"],
                               None if item["buffer"] is None else unwire(item["buffer"]), item["pointer"])


    def _finish(answer, args=()):
        """The value of a `returned` answer is read by the caller; here: raise what the bridge recorded, or stop on `unsupported`."""
        status = answer["status"]
        if status == "unsupported":
            raise NotImplementedError("manifest_contract bridge: %s (%s); no Python fallback" % (answer["boundary"], answer["detail"]))
        if status != "raised":
            return answer
        unwire = _Unwire()
        kind = answer["kind"]
        if kind == "os":
            raise OSError(answer["errno"], unwire(answer["strerror"]), unwire(answer["filename"]))
        if kind == "decode":
            raise UnicodeDecodeError(answer["encoding"], bytes.fromhex(answer["object"]), answer["start"], answer["end"], answer["reason"])
        if kind in ("yaml_marked", "yaml_reader"):
            import yaml
            import importlib
            module = importlib.import_module("yaml." + _YAML_MODULES[answer["cls"]])
            cls = getattr(module, answer["cls"])
            if kind == "yaml_reader":
                raise cls(unwire(answer["source_name"]), answer["position"], answer["character"], answer["encoding"], answer["reason"])
            text = lambda item: None if item is None else unwire(item)
            context_mark = _mark(yaml, answer["context_mark"], unwire)
            # PyYAML passed ONE Mark object for both when `context_mark is problem_mark` (decided by the parser's provenance, not by equal positions)
            problem_mark = context_mark if answer["marks_shared"] else _mark(yaml, answer["problem_mark"], unwire)
            # PyYAML's own raise sites pass exactly the positional arguments of the original construction: four for every composer/constructor/parser/scanner error
            # (no site passes `note`), five only when a note is really supplied; the bridge says which (`args_arity`), so `exception.args` has the original length
            parts = (text(answer["context"]), context_mark, text(answer["problem"]), problem_mark, text(answer["note"]))
            raise cls(*parts[:answer["args_arity"]])
        cls = _CLASSES.get(answer["cls"])
        if cls is None:
            raise RuntimeError("manifest_contract bridge reported an unknown exception class %r" % answer["cls"])
        message = unwire(answer["message"])
        if cls is AttributeError and answer["name"] is not None:
            raise AttributeError(message, name=answer["name"], obj=None if answer["obj"] is None else _walk(answer["obj"], args))
        raise cls(message)


    def _compat():
        """The reviewed original, loaded on demand for the explicit legacy routes only."""
        nonlocal _compat_module
        if _compat_module is None:
            import importlib.util
            spec = importlib.util.spec_from_file_location("manifest_contract_compat_owner", _COMPAT_OWNER)
            _compat_module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(_compat_module)
        return _compat_module


    # ── the original API (same names, signatures, constants) ──────────────────────────────────────

    def _git_branch(repo):
        return _compat()._git_branch(repo)


    def resolve_topology(repo, explicit):
        """topology = --topology 우선 · 없으면 git 브랜치가 *통로*를 선택(single-node→single / multi-node→multi)."""
        wire = _Wire()
        answer = _finish(_ask("resolve_topology", [_path_wire(wire, repo), wire(explicit)]), (repo, explicit))
        if answer["which"] == "explicit":
            return explicit
        return _Unwire()(answer["value"])


    def branch_topology_conflict(repo, explicit):
        """명시된 topology 가 **체크아웃 브랜치가 고르는 통로**와 어긋나면 사유를, 아니면 None.

    ★ 발화 조건은 **브랜치가 알려진 토폴로지로 해소될 때만**이다. 비-git 트리(배포 클론 스모크가
      쓰는 임시 디렉터리)·detached HEAD 에서는 종전 동작을 그대로 둔다 — 거기서 새로 죽으면
      배포 검증이 기대하는 종료코드가 바뀐다.

    브랜치는 권위가 아니라 필터다(policy BRANCH_CONSTITUTION_LAYERING). 그러므로 이 함수는
    "브랜치가 맞다" 고 말하지 않는다 — **둘이 어긋났다는 사실**만 말하고 어느 쪽을 고칠지는
    사람이 정한다(자동 교정 금지).
    """
        return _compat().branch_topology_conflict(repo, explicit)


    def effective_model_source(node, top_level_ms):
        """per-node model_source 해소 규칙(결정론 — plan_26070809_46_57 §4.4):
    node.model_source(있으면) > top-level model_source > None(오류, 호출자 판단)."""
        # the decision is the Rust owner's (`effective_model_source_which`); this assembles the caller's own value
        wire = _Wire()
        answer = _finish(_ask("effective_model_source", [wire(node), wire(top_level_ms)]), (node, top_level_ms))
        return node["model_source"] if answer["which"] == "node_model_source" else top_level_ms


    def manifest_tp(man, topology):
        """manifest 파생 TP (roofline.py:196-209 패턴). config override·최종폴백 1 은 호출자 책임.

    **topology=single → 노드 배수 1 고정**(TP = gpus_per_node): single 의 nodes[role=sub] 는
    sub-control 피어이지 텐서 워커가 아니다. 이 배수를 안 걷으면 서브가 등록된 single manifest 가
    1-GPU 노드에 TP=2 를 요구한다(δ1-1 라이브 E2E 실버그 · check_smoke_model.read_manifest_tp 와
    recipe._target_cards 가 이미 쓰는 계약).
    그 외: nodes 있으면 len(nodes)×gpus_per_node · nodes 비면 None.

    **gpus_per_node 부재·비정수·1 미만 → ValueError**(2026-09-05 · audit_26090515 B4). 종전 `or 1` 은
    HW 사실을 기본값으로 대체하는 침묵 폴백이었다 — 계약의 소유자가 그 폴백을 갖고 있으면 하류 4사이트가
    같은 것을 복제한다. 호출자(evaluate_contract)는 예외를 잡아 EXIT_MISSING_FIELD 로 돌린다."""
        # the decision is the Rust owner's (`manifest_tp_answer`)
        wire = _Wire()
        answer = _finish(_ask("manifest_tp", [wire(man), wire(topology)]), (man, topology))
        return _Unwire()(answer["value"])


    def evaluate_contract(man, topology):
        """순수 게이트 함수(파일 IO 없음 — --self-test 가 이걸 친다).
    returns dict: {flag, reason, topology, gpus_per_node, nodes, manifest_tp, model_source,
                   nas_model_path, custom_model_paths, quant_model_path, tiktoken_host_path, exit_code}.
    """
        # the decision is the Rust owner's (`evaluate_contract_plan`); the result dict is assembled mechanically from the caller's own objects
        wire = _Wire()
        answer = _finish(_ask("evaluate_contract", [wire(man), wire(topology)]), (man, topology))
        unwire = _Unwire()
        res = {}
        for entry in answer["plan"]:
            if entry["from"] == "input":
                res[entry["key"]] = man.get(entry["name"])
            elif entry["from"] == "value":
                res[entry["key"]] = unwire(entry["v"])
            else:
                nodes, top = res["nodes"], res["model_source"]
                mapped = {}
                for item in entry["entries"]:
                    node = nodes[item["node"]]
                    role = node.get("role") if item["role"] == "node_role" else item["role"]["text"]
                    mapped[role] = node["model_source"] if item["value"] == "node_model_source" else top
                res[entry["key"]] = mapped
        return res


    def _load_manifest(path):
        import yaml
        wire = _Wire()
        answer = _finish(_ask("load_manifest", [_path_wire(wire, path)]), (path,))
        return _Unwire()(answer["value"])


    def _self_test():
        return _compat()._self_test()


    def main(argv=None):
        args = sys.argv[1:] if argv is None else argv
        if type(args) is not list or any(type(item) is not str for item in args):
            _unsupported("an argv that is not a list of str")
        which = _finish(_ask("classify_main", [_Wire()(args)]), (args,))["which"]
        if which == "legacy":
            # decided BEFORE any business: the reviewed original answers (usage text, -h, --self-test, abbreviations, usage errors)
            return _compat().main(argv)
        answer = _finish(_ask("main", [_Wire()(args)]), (args,))
        unwire = _Unwire()
        if answer["stdout"] is not None:
            print(unwire(answer["stdout"]))
        if answer["stderr"] is not None:
            sys.stderr.write(unwire(answer["stderr"]))
        return answer["rc"]

    exports = (_git_branch, resolve_topology, branch_topology_conflict, effective_model_source, manifest_tp, evaluate_contract, _load_manifest, _self_test, main)
    for function in exports:
        # the functions are created inside the builder; once it is deleted their export name IS their plain name (qualname is a plain mechanical attribute here)
        function.__qualname__ = function.__name__
    return exports


_exports = _build()
_git_branch = _exports[0]
resolve_topology = _exports[1]
branch_topology_conflict = _exports[2]
effective_model_source = _exports[3]
manifest_tp = _exports[4]
evaluate_contract = _exports[5]
_load_manifest = _exports[6]
_self_test = _exports[7]
main = _exports[8]
del _exports, _build

if __name__ == "__main__":
    # Flush what the interpreter holds before the image is replaced; a closed standard descriptor leaves `sys.stdout`/`sys.stderr` as None,
    # which has nothing to flush (an absent stream is skipped, a real flush error still raises as before).
    for _stream in (sys.stdout, sys.stderr):
        if _stream is not None:
            _stream.flush()
    # PUBLIC ENTRY PLUMBING ONLY (no decision, no business). When the node-local executable is present the interpreter replaces itself with it DIRECTLY and does
    # not start a shell: a shell started in a working directory that no longer exists (the caller removed it after this interpreter started) writes its own
    # `shell-init`/`chdir` getcwd complaints to stderr before the executable runs - bytes the original program (python started directly) never had. The environment
    # handed over is exactly the one `manifest_contract.sh` builds (same carriers, same defaults, same unsets); anything else - the executable missing or not
    # executable, `MANIFEST_CONTRACT_VALIDATE_ONLY=1`, a failing exec - continues to the launcher below, which stays the single place of the guidance (status 2) and of the
    # availability answer. `manifest_contract.sh` itself is unchanged (it remains bash -> Rust for its own callers).
    _here = os.path.dirname(os.path.abspath(__file__))
    _bin = os.environ.get("MANIFEST_CONTRACT_BIN") or _here + "/../../adversarial-benchmark/rust/target/release/manifest_contract"
    if os.path.isfile(_bin) and os.access(_bin, os.X_OK):
        _env = dict(os.environ)
        try:
            if "MANIFEST_CONTRACT_BRIDGE" in _env:
                os.execve(_bin, [_bin, *sys.argv[1:]], _env)
            elif _env.get("MANIFEST_CONTRACT_VALIDATE_ONLY") != "1":
                _env["MANIFEST_CONTRACT_CALLER_ENV"] = "1"
                for _suffix in ("BRIDGE_REQUEST", "COMPAT_OWNER", "PUBLIC_ARGV0", "PYTHON", "VALIDATE_ONLY"):
                    if "MANIFEST_CONTRACT_" + _suffix in os.environ:
                        _env["MANIFEST_CONTRACT_CALLER_" + _suffix] = os.environ["MANIFEST_CONTRACT_" + _suffix]
                    else:
                        _env.pop("MANIFEST_CONTRACT_CALLER_" + _suffix, None)
                _env.pop("MANIFEST_CONTRACT_CALLER_BRIDGE", None)
                _env.pop("MANIFEST_CONTRACT_VALIDATE_ONLY", None)
                if not _env.get("MANIFEST_CONTRACT_COMPAT_OWNER"):
                    _env["MANIFEST_CONTRACT_COMPAT_OWNER"] = _here + "/manifest_contract_compat_owner.py"
                if not _env.get("MANIFEST_CONTRACT_PUBLIC_ARGV0"):
                    _env["MANIFEST_CONTRACT_PUBLIC_ARGV0"] = _here + "/manifest_contract.py"
                if not _env.get("MANIFEST_CONTRACT_PYTHON"):
                    _env.pop("MANIFEST_CONTRACT_PYTHON", None)
                os.execve(_bin, [_bin, *sys.argv[1:]], _env)
        except OSError:
            pass
    os.execve("/bin/bash", ["bash", _here + "/manifest_contract.sh", *sys.argv[1:]], dict(os.environ))
