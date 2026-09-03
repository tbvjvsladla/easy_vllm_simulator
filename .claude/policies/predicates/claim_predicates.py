"""tests/harness/test_policy_claim_predicates.py -- Phase4 cycle4 dedicated claim predicates.

Context (cycle3 reliability finding 2 / subagent-summary-1 finding 2): `.claude/policies/
claim_bindings.json` previously bound every clause to whichever *real, digest-matching* production
identifiers a `registry.yaml` evidence entry happened to cite -- a semantically unrelated clause
backed by `.claude/policies/runtime/policy_registry.py:evaluate_lifecycle` validated with zero violations, because
the checker only asked "does this identifier exist in this tracked file", never "does this
identifier actually demonstrate THIS clause's specific behavior". 191 such tuples existed across
56 clauses, with exactly one clause (TERRAFORM_FLAG_GATE.C4) backed by a real, dedicated test.

This module closes that gap: **one explicit, top-level predicate function per real clause_id**,
named `predicate_<POLICY_ID>_C<n>` (a deterministic, mechanical transform of the clause_id --
`.` -> `_`), each of which inspects or *executes* the specific implementation behavior that clause
claims, asserting every material semantic atom in the registry statement (not generic existence,
not keyword overlap, not a restatement of the clause's own prose).

Execution strategy per clause (documented again inline, at each predicate):
  - Pure Python functions (recipe.py, manifest_contract.py, scan_node.py, preload_ram_gate.py,
    render_dockerfile.py, render_sub_env.py, classify_failure.py, resolve_ngc_tag.py,
    crosscheck_model_card.py, hint_tag.py, cleanup_docker.py) are imported and CALLED directly with
    controlled inputs (real repo state or a hermetic tempdir), asserting on real return values --
    not merely that the function exists.
  - Bash-only logic that is safe to isolate (no SSH/network/docker side effects) is executed for
    real: the exact function body is extracted verbatim from its source file (never retyped) via
    brace-depth matching, combined with the module-level arrays/constants it references, and run
    through `bash -c` in a hermetic tempdir (real `git`/`rsync`/`awk` -- no mocks of the shell
    itself). This is used for sync_to_sub.sh's `assert_band_classification` /
    `assert_sub_delegation_authorized` / `_band2_filters`, and for a full literal execution of
    `configs/arm_patch.sh`.
  - Logic that inherently requires a live docker daemon, a real SSH peer, or a live vLLM serve
    (mem_watchdog.sh's `targets()` docker-ps loop, multinode_serve_smoke.sh's orchestration,
    install_host_safety.sh's `--apply` root-owned installation, run_bench.sh's live bench) is
    verified via AST/regex source-control-flow inspection instead -- exact literal tokens,
    argument-parsing defaults, and source-order invariants, never fuzzy keyword matching.
  - Where a clause bundles a persona/workflow-level fact alongside a code-level one (e.g. hint-tag
    activation being "proposal-only"), the code-level half is executed/inspected and the
    persona-level half is grounded by reading the actual committed `.claude/rules/workflow.md`
    text at test time (never a hardcoded restatement that could drift from the real file).

Every predicate raises `PredicateFailure` through explicit `_require()` checks and returns normally
on success.  Explicit checks remain active under `python -O`; this module is production policy
authority rather than a development test file.
"""
from __future__ import annotations

import ast
import contextlib
import hashlib
import importlib
import inspect
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]

# Preserve fail-closed `-S` verification while exposing only this interpreter's declared global
# dependency directories (never user-site and never a filesystem glob).  `site` is safe to import
# under `-S`: its automatic path mutation stays disabled, while getsitepackages() reports the
# platform's configured global roots, including Debian's /usr/lib/python3/dist-packages.
if sys.flags.no_site:
    import site as _site
    import sysconfig as _sysconfig
    _declared_sites = list(_site.getsitepackages())
    _paths = _sysconfig.get_paths()
    _declared_sites.extend([_paths.get("purelib", ""), _paths.get("platlib", "")])
    for _raw_site in reversed(_declared_sites):
        _declared_site = Path(_raw_site)
        if _raw_site and _declared_site.is_dir() and str(_declared_site) not in sys.path:
            sys.path.insert(0, str(_declared_site))


class PredicateFailure(RuntimeError):
    """A material clause requirement failed closed."""


def _require(condition, message: object) -> None:
    if not condition:
        raise PredicateFailure(str(message))


def _extract_python_function(src: str, name: str) -> str:
    """소스에서 함수 하나의 본문 텍스트를 뽑는다(AST 라인 범위 — 정규식 추측 금지)."""
    tree = ast.parse(src)
    lines = src.splitlines(keepends=True)
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name == name:
            return "".join(lines[node.lineno - 1:node.end_lineno])
    raise AssertionError(f"function {name!r} not found")


def _read(rel_path: str) -> str:
    return (REPO_ROOT / rel_path).read_text(encoding="utf-8")


def _shared_asset(name: str) -> str:
    return _read(f".claude/skills/upstream-version-watch/assets/configs/{name}")


def _rendered(kind: str, topology: str = "multi") -> str:
    manifest = {"cpu_arch": "aarch64", "nas_model_path": "/models"}
    return render_dockerfile.render_shared(kind, topology, manifest)


def _import(rel_dir: str, name: str):
    d = str(REPO_ROOT / rel_dir)
    if d not in sys.path:
        sys.path.insert(0, d)
    return importlib.import_module(name)


def _import_hint_tag():
    if (REPO_ROOT / ".git").exists():
        return _import(".claude/skills/hint-publisher/scripts", "hint_tag")
    # Clean-index exports intentionally have no .git.  hint_tag resolves ROOT at import time;
    # substitute only that read-only rev-parse call so pure scanners/parsers and source
    # inspection remain testable without weakening hint_tag's production fail-closed behavior.
    import importlib.util
    path = REPO_ROOT / ".claude" / "skills" / "hint-publisher" / "scripts" / "hint_tag.py"
    spec = importlib.util.spec_from_file_location("_policy_predicate_hint_tag", path)
    _require(spec is not None and spec.loader is not None, 'predicate requirement failed at original line 98')
    module = importlib.util.module_from_spec(spec)
    original_run = subprocess.run
    def scoped_run(args, *a, **kw):
        if list(args) == ["git", "rev-parse", "--show-toplevel"]:
            return subprocess.CompletedProcess(args, 0, stdout=str(REPO_ROOT) + "\n", stderr="")
        return original_run(args, *a, **kw)
    subprocess.run = scoped_run
    try:
        spec.loader.exec_module(module)
    finally:
        subprocess.run = original_run
    return module


# ---------------------------------------------------------------------------
# Shared module imports (used by multiple predicates below).
# ---------------------------------------------------------------------------
preload_ram_gate = _import(".claude/skills/vllm-recipe-explorer/scripts", "preload_ram_gate")
manifest_contract = _import(".claude/skills/terraforming_node/scripts", "manifest_contract")
scan_node = _import(".claude/skills/terraforming_node/scripts", "scan_node")
render_sub_env = _import(".claude/skills/terraforming_node/scripts", "render_sub_env")
classify_failure = _import(".claude/skills/upstream-version-watch/scripts", "classify_failure")
resolve_ngc_tag = _import(".claude/skills/upstream-version-watch/scripts", "resolve_ngc_tag")
render_dockerfile = _import(".claude/skills/upstream-version-watch/scripts", "render_dockerfile")
crosscheck_model_card = _import(".claude/skills/vllm-recipe-explorer/scripts", "crosscheck_model_card")
gen_recipe_set = _import(".claude/skills/vllm-recipe-explorer/scripts", "gen_recipe_set")
cleanup_docker = _import(".claude/skills/vllm-recipe-explorer/scripts", "cleanup_docker")
hint_tag = _import_hint_tag()
policy_registry = _import(".claude/policies/runtime", "policy_registry")
recipe = _import(".claude/skills/vllm-recipe-explorer", "recipe")


# ---------------------------------------------------------------------------
# Bash extraction/execution helpers -- run REAL shell function bodies (never retyped) in a
# hermetic subprocess, without sourcing the whole parent script (which has network/SSH side
# effects this module must never trigger).
# ---------------------------------------------------------------------------

def _extract_bash_function(source: str, name: str) -> str:
    m = re.search(r"^" + re.escape(name) + r"\s*\(\)\s*\{", source, re.MULTILINE)
    _require(m, f'bash function {name!r} not found in source')
    depth = 0
    start = m.start()
    for j in range(m.end() - 1, len(source)):
        c = source[j]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return source[start:j + 1]
    raise AssertionError(f"unterminated bash function {name!r}")


def _extract_bash_array(source: str, name: str) -> str:
    m = re.search(r"^" + re.escape(name) + r"=\([^)]*\)", source, re.MULTILINE)
    _require(m, f'bash array {name!r} not found in source')
    return m.group(0)


def _run_bash(script: str) -> subprocess.CompletedProcess:
    return subprocess.run(["bash", "-c", script], capture_output=True, text=True, timeout=30)


# =============================================================================
# HOST_SAFETY_LAYERED_DEFENSE (8 clauses)
# =============================================================================

def predicate_HOST_SAFETY_LAYERED_DEFENSE_C1():
    """C1: layered defense = systemd mem_watchdog(broad @vllm) + harness-scoped watchdog
    auto-started by run_trial/multinode_serve_smoke.sh + container oom_score_adj=800 + earlyoom
    backstop + pre-load RAM gate."""
    mw = _read(".claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh")
    _require('FILTER="${1:-@vllm}"' in mw, 'mem_watchdog.sh must default to the broad @vllm filter')
    _require('THRESH_MIB="${2:-10240}"' in mw, 'default MemAvailable threshold must be 10240 MiB')
    _require('tolower($0) ~ /vllm/' in mw, 'broad-mode target enumeration must case-insensitively match vllm')

    mn = _read(".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh")
    _require('MAIN_WATCHDOG="$REPO/.claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh"' in mn and
             'bash "$MAIN_WATCHDOG"' in mn and 'WD_MAIN_PID=$!' in mn,
             'multinode_serve_smoke.sh must resolve and auto-start the owner-local watchdog')

    rt = _read(".claude/skills/vllm-recipe-explorer/scripts/run_trial.py")
    rt_tree = ast.parse(rt)
    start_fn = next(n for n in rt_tree.body if isinstance(n, ast.FunctionDef) and n.name == "_start_memwatch")
    popen_calls = [n for n in ast.walk(start_fn) if isinstance(n, ast.Call)
                   and isinstance(n.func, ast.Attribute)
                   and isinstance(n.func.value, ast.Name)
                   and n.func.value.id == "subprocess" and n.func.attr == "Popen"]
    _require(len(popen_calls) == 1, '_start_memwatch must launch exactly one scoped watchdog subprocess')
    launch = ast.unparse(popen_calls[0])
    _require('script' in launch and 'container_name' in launch, "watchdog Popen must bind the resolved script and this trial's container identity")

    compose = _rendered("compose")
    _require('oom_score_adj: 800' in compose, 'container oom_score_adj must be 800 (kernel OOM-killer targeting)')

    ihs = _read(".claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh")
    _require('earlyoom' in ihs and '-m 4' in ihs, 'earlyoom backstop must be installed at the -m 4 threshold')

    # pre-load RAM gate: the module is real and callable -- checkpoint size unknown => skip, never
    # a false refusal (the documented "negative-honesty" contract).
    res = preload_ram_gate.gate(None, tp=1)
    _require(res['ok'] is True and res['skipped'] is True, 'predicate requirement failed at original line 201')

    # The clause's own core claim -- "compares checkpoint total_size/TP plus a floor against
    # MemAvailable" -- executed for REAL with controlled inputs (never a local-arithmetic
    # substitute): holding checkpoint size and MemAvailable fixed, only TP differs and flips the
    # verdict; only floor_mib differs and flips the verdict too. A regression that dropped the
    # "// tp" division or the "+ floor_mib" addition would make one of these two flips disappear.
    MIB = preload_ram_gate.MIB
    orig_mem2 = preload_ram_gate.mem_available_mib
    try:
        preload_ram_gate.mem_available_mib = lambda: 5000
        r_tp1 = preload_ram_gate.gate(8192 * MIB, tp=1, floor_mib=1024, auto_drop=False)
        r_tp4 = preload_ram_gate.gate(8192 * MIB, tp=4, floor_mib=1024, auto_drop=False)
        _require(r_tp1['ok'] is False and r_tp1['required_mib'] == 9216, 'tp=1: ceil(8192/1)+1024=9216 > 5000 MemAvailable must refuse')
        _require(r_tp4['ok'] is True and r_tp4['required_mib'] == 3072, 'tp=4: ceil(8192/4)+1024=3072 <= 5000 MemAvailable must pass -- proves TP division is real')

        r_floor0 = preload_ram_gate.gate(4096 * MIB, tp=1, floor_mib=0, auto_drop=False)
        r_floor_big = preload_ram_gate.gate(4096 * MIB, tp=1, floor_mib=1500, auto_drop=False)
        _require(r_floor0['ok'] is True and r_floor0['required_mib'] == 4096, 'predicate requirement failed at original line 221')
        _require(r_floor_big['ok'] is False and r_floor_big['required_mib'] == 5596, 'same checkpoint/tp, only floor_mib raised, must flip pass->refuse -- proves the floor is additive')
    finally:
        preload_ram_gate.mem_available_mib = orig_mem2


def predicate_HOST_SAFETY_LAYERED_DEFENSE_C2():
    """C2: installation is offered only as a terraforming session-final opt-in, informative not
    mandatory, via human-executed sudo (--apply), identically on main and sub."""
    src = _read(".claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh")
    tree = ast.parse("APPLY=0")  # placeholder to keep ast imported for other predicates; unused here
    del tree
    # 2026-07-31: the anchor used to be 'APPLY=0; WITH_KDUMP=0'. --with-kdump was retired (C7), so
    # the literal moved -- C2 itself never claimed anything about kdump, it claims a dry-run default.
    # Re-anchor on the surviving declaration; the assertion strength is unchanged.
    _require('APPLY=0; EARLYOOM_DEB=""' in src, 'APPLY must default to 0 (dry-run) unless --apply is passed')
    _require('--apply) APPLY=1 ;;' in src, '--apply is the sole toggle to 1')
    _require('if [ "$APPLY" = "1" ] && [ "$(id -u)" -ne 0 ]; then' in src, '--apply must require root (HITL sudo), never a passwordless/automatic escalation')
    _require('say "DRY-RUN 종료' in src, 'default path must end in an informative DRY-RUN message, not an install')
    # the DRY-RUN message must sit in the trailing `else` of the top-level APPLY guard (never
    # reachable from the APPLY=1 branch) -- proves the informative-default ordering structurally,
    # not just that the string exists somewhere in the file.
    final_if_idx = src.rindex('if [ "$APPLY" = "1" ]; then')
    summary_idx = src.index('say "설치 요약', final_if_idx)
    else_idx = src.index("else", summary_idx)
    dryrun_idx = src.index('say "DRY-RUN 종료', else_idx)
    _require(final_if_idx < summary_idx < else_idx < dryrun_idx, 'the DRY-RUN message must live in the else-branch of the APPLY guard, after the apply summary')
    # "identically on main and sub": the script takes no node-role branch at all -- it is the same
    # code invoked on either node (no `if [ role = sub ]` special-case exists).
    _require(re.search('\\brole\\b', src) is None, 'predicate requirement failed at original line 250')

    # NEW atoms: "session-final opt-in ... surfaced after the completion Flag has already been
    # issued" and "informative rather than mandatory tone" are persona-level facts -- grounded by
    # reading the actual committed terraforming_node/SKILL.md text (never a hardcoded restatement).
    skill_src = _read(".claude/skills/terraforming_node/SKILL.md")
    _require('## 2.6 호스트 안전체계 — 세션 최종 선택조항' in skill_src, 'host-safety must be documented as the session-FINAL opt-in step')
    _require('Flag 발급 이후' in skill_src and '독립 Y/N 선택조항' in skill_src, 'must be documented as coming strictly AFTER Flag issuance, as an independent choice')
    # the section explicitly names its own entry point as coming right after the §1S manifest+Flag
    # step (single-node) -- the concrete textual anchor for "surfaced after the completion Flag has
    # already been issued".
    _require('single 은 §1S manifest+Flag 기입 직후' in skill_src and '이 절로 온다' in skill_src, 'predicate requirement failed at original line 265')


def predicate_HOST_SAFETY_LAYERED_DEFENSE_C3():
    """C3: host-safety install status is independent of the completion Flag; manifest
    host_safety.installed:false is a neutral record; opt-out gets exactly one chat-only warning
    line on unified-memory serve only, with no serve-script/log banner code."""
    scan_src = _read(".claude/skills/terraforming_node/scripts/scan_node.py")
    result = {
        "topology_declared": "single", "cpu_arch": "aarch64", "cuda_version": "132",
        "gpus_per_node": 1,
        "interconnect": {"type": "generic-ethernet", "hca_devices": [], "gid_index": None,
                          "socket_iface": None, "bandwidth_gbps": None, "platform_preset": None},
    }
    block = scan_node.emit_manifest_block(result)
    # the Flag attestation block is emitted unconditionally here -- host_safety is NOT one of the
    # keys this function writes at all (it is a separate, later, session-final interview field).
    _require('terraforming:' in block and 'complete: true' in block, 'predicate requirement failed at original line 282')
    _require('host_safety:' not in block, 'predicate requirement failed at original line 283')
    _require('host_safety.installed' in scan_src and 'Flag 와 독립' in scan_src, 'predicate requirement failed at original line 284')

    # NEW atom: the completion Flag genuinely stays valid regardless of host_safety.installed --
    # execute the REAL gate function both ways (evaluate_contract must never even read/branch on
    # the key at all -- true independence, not a coincidental pass).
    base = {"topology": "single", "gpus_per_node": 1, "model_source": "managed",
            "terraforming": {"complete": True, "branch_verified": True}}
    r_installed = manifest_contract.evaluate_contract({**base, "host_safety": {"installed": True}}, "single")
    r_optout = manifest_contract.evaluate_contract({**base, "host_safety": {"installed": False}}, "single")
    _require(r_installed['flag'] is True and r_optout['flag'] is True, 'the completion Flag must stay valid whether or not host_safety is installed')
    _require('host_safety' not in r_installed and 'host_safety' not in r_optout, 'evaluate_contract must never echo/branch on host_safety at all -- true independence')

    # NEW atom: host_safety.installed:false is recorded as a neutral record in the real manifest
    # template (never a rebuke), and the exactly-one-line, unified-memory-only, no-code-banner
    # warning is real documented behavior, not fabricated.
    tmpl = _read("manifest.template.yaml")
    _require(re.search('^host_safety:', tmpl, re.M), 'manifest must carry a host_safety field')
    _require('통합메모리 노드면 이후 서빙 기동 시 에이전트 채팅 1줄 경고' in tmpl, 'opt-out unified-memory follow-up must be documented as exactly one chat-only line')
    skill_src = _read(".claude/skills/terraforming_node/SKILL.md")
    _require('discrete GPU 노드는 무경고' in skill_src, 'the warning must be scoped to unified memory only')
    _require('serve 스크립트/로그 배너 코드변경 ✗' in skill_src, 'must be documented as explicitly excluding any serve-script/log banner code change')

    # NEW atom: absence from serve logs/scripts -- no tracked serve-facing script in this repo
    # actually carries a code-level banner for this warning (real absence, not just documentation).
    banned_phrase = "워치독 미설치 상태"
    for text in (_shared_asset("serve_runner.sh"), _shared_asset("debug-init.sh"),
                 _shared_asset("arm_patch.sh"), _rendered("compose")):
        _require(banned_phrase not in text, 'shared runtime sources must never carry a serve-script banner for this warning')


def predicate_HOST_SAFETY_LAYERED_DEFENSE_C4():
    """C4: only the harness-scoped watchdog is installation-independent; when the drop-caches
    helper is missing, try_drop_caches gracefully returns False (no exception, truthfully
    reported), but the gate's own refusal is preserved regardless."""
    orig_helper = preload_ram_gate.DROP_HELPER
    orig_mem = preload_ram_gate.mem_available_mib
    try:
        preload_ram_gate.DROP_HELPER = "/nonexistent/vllm-drop-caches"
        _require(preload_ram_gate.try_drop_caches() is False, 'predicate requirement failed at original line 327')  # graceful skip, no exception

        preload_ram_gate.mem_available_mib = lambda: 100  # persistently scarce, drop-caches absent
        res = preload_ram_gate.gate(500 * 1024 ** 3, tp=1, floor_mib=10240, auto_drop=True)
        _require(res['ok'] is False, 'refusal must be preserved even when the drop helper is absent')
        _require(res['dropped'] is False and res['skipped'] is False, 'predicate requirement failed at original line 332')

        # NEW atoms: the promised CLI exit code 7, and a truthful user-visible helper-absence
        # report -- executed for real via main() (not merely the library-level gate() return dict).
        orig_argv = sys.argv
        orig_stderr = sys.stderr
        sys.argv = ["preload_ram_gate.py", "--checkpoint-bytes", str(500 * 1024 ** 3), "--tp", "1"]
        captured = io.StringIO()
        sys.stderr = captured
        try:
            try:
                preload_ram_gate.main()
                raise AssertionError("main() must sys.exit(7) on refusal")
            except SystemExit as e:
                _require(e.code == 7, f'CLI must exit 7 on insufficient memory, got {e.code!r}')
        finally:
            sys.stderr = orig_stderr
            sys.argv = orig_argv
        report = captured.getvalue()
        _require('drop-caches 헬퍼 미가용' in report, 'the CLI must truthfully report the missing drop-caches helper, never silently proceed')
        _require('REFUSE' in report and 'PASS' not in report, 'the CLI must truthfully report REFUSE, never a fabricated PASS')
    finally:
        preload_ram_gate.DROP_HELPER = orig_helper
        preload_ram_gate.mem_available_mib = orig_mem


def predicate_HOST_SAFETY_LAYERED_DEFENSE_C5():
    """C5: the standing watchdog daemon (observation + protective-kill only) is a deliberate
    exception to the no-unattended-autorun policy -- it never triggers a build/bump/download."""
    ihs = _read(".claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh")
    _require('systemctl enable --now easy-vllm-memwatch.service' in ihs, 'the daemon must be a persistent, auto-starting systemd unit (the exception itself)')
    mw = _read(".claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh")
    for banned in ("docker build", "docker pull", "docker run", "docker compose"):
        _require(banned not in mw, f'mem_watchdog.sh must never itself trigger {banned!r}')
    _require('docker kill' in mw, "the daemon's only destructive action must be a protective kill")


def predicate_HOST_SAFETY_LAYERED_DEFENSE_C6():
    """C6: sudo delegation uses exactly one fixed helper path via a single sudoers NOPASSWD entry;
    watchdog shutdown is PID-based only -- a blanket process-name kill is forbidden."""
    ihs = _read(".claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh")
    _require(ihs.count('NOPASSWD: /usr/local/sbin/vllm-drop-caches') == 1, 'exactly one fixed sudoers NOPASSWD helper path')
    _require(ihs.count('/usr/local/sbin/vllm-drop-caches') >= 2, 'predicate requirement failed at original line 378')  # install target + sudoers grant

    mw = _read(".claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh")
    # "pkill -f" appears once, but only inside a comment PROHIBITING it -- assert no non-comment
    # (actually executed) line ever invokes it.
    for line in mw.splitlines():
        if line.strip().startswith("#"):
            continue
        _require('pkill -f' not in line, 'blanket process-name kill is forbidden (self-referential exit144 class)')
    mn = _read(".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh")
    for line in mn.splitlines():
        if line.strip().startswith("#"):
            continue
        _require('pkill -f' not in line, 'predicate requirement failed at original line 391')
    _require('kill "$WD_MAIN_PID"' in mn, 'watchdog shutdown must be PID-based')

    # ★ 2026-09-03 (적대검증 MAJOR ②): 이 절의 사정거리가 install_host_safety.sh 하나였다.
    #   그런데 **node_blackbox 설치기도 같은 단일 헬퍼 경로로 같은 sudoers 엔트리를 쓴다** —
    #   그쪽 배치는 rc 를 버린 채 다음 줄에서 "✓ sudoers" 를 조건 없이 찍던 fail-open 이었고,
    #   그 형태로 되돌려도 하네스 6종이 전부 초록이었다(앵커 0). 여기서 사정거리를 넓힌다.
    #   판정은 "명령이 돌았다"가 아니라 **그 자리에 유효한 조각이 있는가**여야 한다.
    nbi = _read(".claude/skills/terraforming_node/scripts/node_blackbox/install_node_blackbox.sh")
    _require(nbi.count("NOPASSWD: %s/vllm-drop-caches") == 1,
             'the node_blackbox installer must grant exactly one fixed NOPASSWD helper path')
    _require('SUDOERS_F=/etc/sudoers.d/easy-vllm-host-safety' in nbi,
             'the sudoers fragment path must be a single named constant, not re-typed per use')
    _require('if run install -m 0440 "$T" "$SUDOERS_F" \\' in nbi,
             'the fragment must be PLACED through run() so a failed placement reaches the FAIL verdict')
    _require('&& [ "$(stat -c %a "$SUDOERS_F" 2>/dev/null)" = "440" ] \\' in nbi,
             'the placed fragment must be re-read for mode 0440 — sudo silently ignores any other mode')
    _require('&& visudo -cf "$SUDOERS_F" >/dev/null 2>&1; then' in nbi,
             'the PLACED fragment (not merely the mktemp candidate) must re-pass visudo')
    # 되돌림 방지: 수리 전의 **무조건 ✓** 형태가 되살아나면 즉시 FAIL 이다.
    _require('install -m 0440 "$T" /etc/sudoers.d/easy-vllm-host-safety' not in nbi,
             'the pre-repair unconditional placement (rc discarded, ✓ printed regardless) must not return')
    _require('say "   ✓ sudoers(visudo 검증 통과)"' not in nbi,
             'a sudoers success line must never be printed without re-reading the placed fragment')
    _sud_place = nbi.index('if run install -m 0440 "$T" "$SUDOERS_F"')
    _sud_ok = nbi.index('say "   ✓ sudoers(visudo 검증 통과 · $SUDOERS_F 0440 배치 확인)"', _sud_place)
    _sud_bad = nbi.index('say "   ✗ sudoers 배치 실패: $SUDOERS_F', _sud_ok)
    _sud_fail = nbi.index("FAIL=1", _sud_bad)
    _require(_sud_place < _sud_ok < _sud_bad < _sud_fail,
             'the ✓ line must sit inside the verified branch and the ✗ branch must raise FAIL')


def predicate_HOST_SAFETY_LAYERED_DEFENSE_C7():
    """C7: post-mortem capture is efi_pstore, not kdump. node_blackbox L3 removes crashkernel/
    ramoops and disarms kdump-tools; legacy --with-kdump is refused with its reason; capture is
    claimed only via verify_node_blackbox.sh's crash-test/post-crash writing capture_verified.
    Grounded in docs/testlog/testlog_26073114 (7 forced panics, both nodes)."""
    nb = _read(".claude/skills/terraforming_node/scripts/node_blackbox/install_node_blackbox.sh")
    vb = _read(".claude/skills/terraforming_node/scripts/node_blackbox/verify_node_blackbox.sh")
    ihs = _read(".claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh")

    # L3 disarms kdump rather than installing it.
    _require("USE_KDUMP=0" in nb, "L3 must set USE_KDUMP=0 (disarm), never 1")
    _require("systemctl disable --now kdump-tools" in nb, "L3 must disable kdump-tools")
    _require("kdump-config unload" in nb, "L3 must unload any already-loaded kexec image")

    # The GRUB drop-in L3 emits carries NO reservation tokens -- it is deliberately empty.
    #   Checking the whole file for the substring is wrong: the --suggest-ramoops diagnostic and the
    #   design rationale legitimately *mention* reserve_mem/ramoops in prose and say-lines. What must
    #   be free of reservations is the heredoc actually written to the GRUB drop-in. Extract it.
    _m = re.search(r"cat > /etc/default/grub\.d/zz-easy-vllm-blackbox\.cfg <<'?EOF'?\n(.*?)\nEOF",
                   nb, re.S)
    _require(_m is not None, "L3 must write the GRUB drop-in via a heredoc we can inspect")
    _emitted = _m.group(1) if _m else ""
    for _tok in ("crashkernel=", "reserve_mem=", "ramoops."):
        _require(_tok not in _emitted,
                 "L3 GRUB drop-in must emit no %s (kdump blocks pstore; ramoops cannot survive "
                 "reset on this platform)" % _tok)
    _require("GRUB_CMDLINE_LINUX_DEFAULT" not in _emitted,
             "L3 GRUB drop-in must not re-append kernel parameters at all")
    _require("rm -f /etc/modules-load.d/easy-vllm-ramoops.conf" in nb,
             "L3 must withdraw the ramoops autoload config")

    # efi_pstore path is actively secured: archival on, hang->panic promotion retained.
    _require("systemctl enable systemd-pstore" in nb,
             "systemd-pstore archival required (else EFI NVRAM fills and capture fails)")
    _require("kernel.hung_task_panic=1" in nb and "kernel.softlockup_panic=1" in nb,
             "hang->panic promotion retained so silent hangs reach kmsg_dump")
    _require("재부팅 1회" in nb, "L3 is documented as requiring exactly one reboot")
    # NEW (plan_26082410): 재부팅은 HITL(사람 수행)임을 단언 — 스크립트가 reboot 를 직접 실행하지 않고
    # "다음 단계(사람)"으로 위임(무인 자동 재부팅 금지). 문서전용이 아닌 "사람 위임" 계약으로 강화.
    _require('다음 단계(사람)' in nb, 'L3 reboot must be documented as a human-performed step, never unattended')
    _require(not re.search(r'^\s*(sudo\s+)?(systemctl\s+)?reboot\b', nb, re.M), 'the install script must never itself execute a reboot — HITL only')

    # Legacy flag is refused WITH ITS REASON, not silently deleted (discoverability).
    _require("--with-kdump)" in ihs, "legacy flag must remain recognised so the refusal is reachable")
    _require("거부: --with-kdump 는 폐지됐습니다" in ihs, "legacy flag must refuse explicitly")
    _require("KDUMP_CRASHKERNEL=" not in ihs, "legacy installer must no longer reserve crashkernel")

    # Capture is a verified verdict, never an installation claim.
    _require("capture_verified" in vb, "verifier owns the capture_verified verdict file")
    _require("--crash-test" in vb and "--post-crash" in vb, "crash-test/post-crash modes required")
    _require("Kernel panic" in vb, "post-crash must confirm record CONTENT, not record count")

    # ★ 2026-09-03 (적대검증 MAJOR ②-④): "capture is claimed only after verification" 은 **설치기
    #   자신에게도** 적용된다. 예전 L3 블록은 네 자리(kdump 무장해제 · hang→panic 승격 · systemd-
    #   pstore 활성 · 예약 0) 전부를 **결과를 한 번도 읽지 않고** ✓ 로 찍었고, 바로 아래에서
    #   "INSTALL PASS" 가 나갔다 — 숫자를 보여 주는 것은 판정이 아니다. 네 자리 모두 결과 상태로
    #   판정해야 하며, **읽지 못한 것은 clean 이 아니라 판정 불가(FAIL)** 다.
    #   앵커가 없으면 이 넷을 수리 전 형태로 되돌려도 하네스가 전부 초록이다(실측).
    #   ⓐ kdump 무장 = kexec_crash_loaded (crash_kexec_post_notifiers=N 이라 적재돼 있으면
    #     panic() 이 kmsg_dump 보다 먼저 kexec 로 점프한다 = pstore 원천 차단)
    _require('_KL="$(cat /sys/kernel/kexec_crash_loaded 2>/dev/null || echo \'\')"' in nb,
             'kdump disarm must be judged from kexec_crash_loaded, not from the disable command rc')
    _require('if [ -z "$_KL" ]; then' in nb and 'elif [ "$_KL" = "0" ]; then' in nb,
             'an unreadable kexec_crash_loaded must be undecidable (FAIL), never reported as disarmed')
    _require('say "   ✓ kdump 무장 해제(USE_KDUMP=0 · 서비스 disable · kexec unload)"' not in nb,
             'the pre-repair unconditional kdump ✓ (result never read) must not return')
    #   ⓑ hang→panic 승격 = 방금 쓴 파일에서 파생한 기대값 대 실제 sysctl 값
    _require('_got="$(sysctl -n "$_k" 2>/dev/null)"' in nb,
             'panic-promotion must be judged by re-reading each key, not by the `sysctl -p` rc alone')
    _require('done < /etc/sysctl.d/99-easy-vllm-panic-promote.conf' in nb,
             'expected values must be DERIVED from the file just written (hand-restating them splits the two)')
    _require('if [ -n "$_PP_BAD" ]; then' in nb and 'elif [ "$_SP_RC" -ne 0 ]; then' in nb,
             'both a value mismatch and a failed `sysctl -p` (reboot persistence) must raise FAIL')
    _require('say "   ✓ hang→panic 승격 sysctl (이제 efi_pstore 를 먹인다)"' not in nb,
             'the pre-repair unconditional sysctl ✓ must not return')
    #   ⓒ systemd-pstore = is-enabled 결과 상태(enable rc 는 이미 enabled 인 노드에서 위양성)
    _require('_PS_EN="$(systemctl is-enabled systemd-pstore 2>/dev/null)"' in nb,
             'systemd-pstore must be judged by is-enabled, never by the enable rc alone')
    _require('case "$_PS_EN" in' in nb and 'enabled|enabled-runtime|static|indirect|generated)' in nb,
             'the is-enabled verdict must enumerate the accepting states explicitly')
    _require('say "   ✓ systemd-pstore 아카이브 활성(NVRAM 누적 방지)"' not in nb,
             'the pre-repair unconditional systemd-pstore ✓ must not return')
    #   ⓓ 예약 잔존 = kexec_crash_size 를 **판정**한다(출력만 하면 아무도 판단하지 않는다)
    _require('CKS_NOW="$(cat /sys/kernel/kexec_crash_size 2>/dev/null || echo \'\')"' in nb,
             'the reservation must be read into a named result, not interpolated into a say-line')
    _require('elif [ "$CKS_NOW" -eq 0 ] 2>/dev/null; then' in nb,
             'the reservation must be JUDGED == 0, not merely printed for a human to read')
    _require('현재 커널의 kdump 예약: $(cat /sys/kernel/kexec_crash_size' not in nb,
             'the pre-repair print-only reservation line (no verdict) must not return')
    #   네 자리 모두 "읽지 못함 = 판정 불가 = FAIL" 을 갖는다.
    for _undecidable in ('say "   ✗ /sys/kernel/kexec_crash_loaded 를 읽지 못했다',
                         'say "  ✗ /sys/kernel/kexec_crash_size 를 읽지 못했다'):
        _require(_undecidable in nb,
                 f'an unreadable kernel fact must be reported as undecidable: {_undecidable[:60]}')



def predicate_HOST_SAFETY_LAYERED_DEFENSE_C8():
    """C8: cleanup_docker.py always prints a dry-run table against a deterministic preserve-set
    first; only deletes once a human explicitly re-invokes with --apply; never a periodic job."""
    src = _read(".claude/skills/vllm-recipe-explorer/scripts/cleanup_docker.py")
    tree = ast.parse(src)
    for node in ast.walk(tree):
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute) and node.func.attr == "add_argument":
            str_args = [a.value for a in node.args if isinstance(a, ast.Constant) and isinstance(a.value, str)]
            if "--apply" in str_args:
                for kw in node.keywords:
                    _require(not (kw.arg == 'default' and isinstance(kw.value, ast.Constant) and (kw.value.value is True)), '--apply must never default to True')
    dry_run_idx = src.index("if not args.apply:")
    apply_section_idx = src.index("APPLY (사람 승인 후)")
    rmi_idx = src.index('"docker", "rmi"')
    _require(dry_run_idx < apply_section_idx < rmi_idx, 'the dry-run early-return must precede the human-approved delete section in source order')
    _require('트리거(자동 주기 없음): bump S4 종료 루틴 + 세션말 사람 질의' in src, 'predicate requirement failed at original line 450')
    imported = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            imported.update(a.name.split(".")[0] for a in node.names)
        elif isinstance(node, ast.ImportFrom) and node.module:
            imported.add(node.module.split(".")[0])
    _require(not imported & {'schedule', 'apscheduler', 'crontab', 'croniter'}, 'no scheduling library may be imported -- this routine is never periodic')

    # NEW: execute the REAL deterministic preserve-set against the actual tracked repo tree (no
    # docker daemon touched -- preserve_set()'s own docker-ps step degrades gracefully when absent),
    # then drive main()'s dry-run candidate table with a synthetic image list (list_images() and the
    # `docker system df` call are the only docker-dependent parts, monkeypatched here) and prove:
    # (a) a real, currently-tracked preserved ref (the Dockerfile FROM base) never appears as a
    # deletion candidate, (b) an unrelated debris image genuinely does, and (c) `docker rmi` is
    # never invoked at all without --apply.
    real_keep, _real_why = cleanup_docker.preserve_set(canonical_only=True)
    dockerfile_from = re.search(r"^FROM\s+(\S+)", _rendered("dockerfile"), re.M).group(1)
    _require(dockerfile_from in real_keep, "preserve_set() must really collect the tracked Dockerfile's FROM base as a preserved ref")

    repo_part, _, tag_part = dockerfile_from.partition(":")
    debris_ref = "easy-vllm:leftover-debris-9999"
    synthetic_images = [
        {"Repository": repo_part, "Tag": tag_part or "latest", "ID": "sha256:keep",
         "Size": "1GB", "CreatedSince": "1 day ago"},
        {"Repository": "easy-vllm", "Tag": "leftover-debris-9999", "ID": "sha256:debris",
         "Size": "2GB", "CreatedSince": "40 days ago"},
    ]
    rmi_calls = []

    def fake_run(cmd, timeout=60):
        if list(cmd[:2]) == ["docker", "rmi"]:
            rmi_calls.append(cmd)
            raise AssertionError("docker rmi must never be invoked without --apply")
        return 1, "docker unavailable in test harness"

    orig_list_images, orig_run, orig_argv, orig_stdout = (
        cleanup_docker.list_images, cleanup_docker._run, sys.argv, sys.stdout)
    stdout_capture = io.StringIO()
    try:
        cleanup_docker.list_images = lambda: synthetic_images
        cleanup_docker._run = fake_run
        sys.argv = ["cleanup_docker.py"]
        sys.stdout = stdout_capture
        cleanup_docker.main(canonical_only=True)
    finally:
        cleanup_docker.list_images, cleanup_docker._run = orig_list_images, orig_run
        sys.argv, sys.stdout = orig_argv, orig_stdout

    report = stdout_capture.getvalue()
    rm_lines = [ln for ln in report.splitlines() if ln.strip().startswith("RM")]
    keep_lines = [ln for ln in report.splitlines() if ln.strip().startswith("KEEP")]
    _require(not rmi_calls, 'dry-run must never call docker rmi')
    _require(any((debris_ref in ln for ln in rm_lines)), 'an unpreserved debris image must appear as an RM candidate')
    _require(not any((dockerfile_from in ln for ln in rm_lines)), 'the preserved Dockerfile FROM base must never appear as an RM candidate')
    _require(any((dockerfile_from in ln for ln in keep_lines)), 'the preserved Dockerfile FROM base must appear in the KEEP table')
    _require('DRY-RUN 종료' in report, 'predicate requirement failed at original line 510')


def predicate_HOST_SAFETY_LAYERED_DEFENSE_C9():
    """C9: purge_host_safety.sh — 이 저장소에서 가장 파괴적인 스크립트. 2026-09-03(감사 §5 C-2)
    까지 registry/claim_bindings 어느 쪽에도 **바인딩이 없던** 유일한 파괴 스크립트였다.

    정적 단언만으로는 부족하다(이 프로젝트가 반복해 당한 결함: 단언만 있고 술어가 없다) — 그래서
    아래는 **실제로 스크립트를 두 번 기동**해 ⓐ 시드 게이트가 정말 거부하는지 ⓑ dry-run 이 정말
    아무것도 실행하지 않는지를 관측한다. 두 기동 모두 파괴 인자(--apply) 없이 돌므로 호스트를
    건드리지 않는다. node_id 는 `--node-id=` 명시 override 로 고정해 환경 의존을 없앤다."""
    rel = ".claude/skills/terraforming_node/scripts/node_blackbox/purge_host_safety.sh"
    src = _read(rel)

    # ── 정적: 파괴는 --apply 뒤에만, 그리고 root 로만 ────────────────────────────
    _require("APPLY=0;" in src or re.search(r"^APPLY=0\b", src, re.M),
             "--apply must default to off (APPLY=0)")
    _require(re.search(r'run\(\)\{\s*\n\s*if \[ "\$APPLY" = "1" \]', src),
             "the destructive wrapper run() must execute only under --apply")
    _require('say "FAIL: --apply 는 root 필요' in src, "--apply must refuse to proceed as non-root")
    # 설치와 제거를 한 스크립트에 섞지 않는다(부분 적용 상태 금지).
    for line in src.splitlines():
        stripped = line.strip()
        if stripped.startswith("#"):
            continue
        _require("install_node_blackbox.sh" not in stripped and "install_host_safety.sh" not in stripped,
                 "the purger must never invoke an installer — purge and install stay separate scripts")

    # ── 정적: rc 전파(B5) — 실패한 제거 위에서 'PURGE PASS' 가 찍히면 안 된다 ────
    _require('if [ "$_rc" -ne 0 ]; then say "   ✗ 실패(rc=$_rc): $*"; FAIL=1; fi' in src,
             "run() must propagate a non-zero rc into the single FAIL verdict")
    _require('if [ "$FAIL" = "0" ]; then say "PURGE PASS' in src and 'exit "$FAIL"' in src,
             "the summary verdict and the exit code must both be derived from FAIL")

    # ── 정적: 블랭킷 이름매칭 킬 금지(자기참조 사망 선례 devlog_26062718) ────────
    # ⚠ 부분문자열 금지는 여기서 위양성이다 — 스크립트는 `say "… pkill -f 금지"` 로 그 금지를
    #   **선언**한다. 금지 대상은 *언급*이 아니라 *호출*이므로 명령 위치만 본다.
    _invocation = re.compile(r"(?:^|[;&|(]|\$\(|\bthen\b|\belse\b|\bdo\b)\s*(?:pkill|pgrep)\b")
    for line in src.splitlines():
        if line.strip().startswith("#"):
            continue
        _require(_invocation.search(line) is None,
                 "blanket process-name matching is forbidden on executable lines (2026-08-02 false positive)")
    _require("awk '$2 ~ /(^|\\/)bash$/ && $3 ~ /(^|\\/)mem_watchdog\\.sh$/ {print $1}'" in src,
             "wd_pids must resolve PIDs by argv field anchors, not by a whole-line substring match")

    # ★ 2026-09-03 (적대검증 MAJOR ②-③): "terminated by PID" 는 **kill 을 보냈다**가 아니라
    #   **실제로 사라졌다**여야 한다. kill 의 rc 는 판정 근거가 못 된다(대상이 이미 죽었으면
    #   비-0 인데 그것이 우리가 원하던 결과다) — 그래서 판정은 사후 소멸 확인이 한다.
    #   그 확인 루프에는 off-by-one 이 있었다: 매 회 `확인 → sleep 1` 순서라 **마지막 sleep 뒤
    #   재확인이 없어** 실판정 지평이 5초가 아니라 4초였고, 5초째에 죽은 프로세스를 "살아 있다"
    #   (FAIL=1)로 오보했다. 이 절에 앵커가 없어 그 형태로 되돌려도 하네스가 전부 초록이었다.
    _require('for _i in 0 1 2 3 4 5; do' in src,
             'the disappearance probe must sample t=0..5 — six checkpoints for a 5s horizon')
    _require('[ "$_i" = "0" ] || sleep 1' in src,
             'the first probe must precede any sleep and the last must FOLLOW the 5th sleep '
             '(the old check→sleep order wasted the final second: real horizon 4s, not 5s)')
    _require('kill -0 "$p" 2>/dev/null || { _gone=1; break; }' in src,
             'disappearance must be observed with kill -0, not inferred from the kill rc')
    _require('case "$(ps -o stat= -p "$p" 2>/dev/null)" in Z*) _gone=1; break ;; esac' in src,
             'a zombie must count as gone — kill -0 succeeds on zombies (H83)')
    _gone_ok = src.index('if [ "$_gone" = "1" ]; then say "   ✓ PID $p 종료 확인"')
    _gone_bad = src.index('✗ PID $p 가 kill 후에도 살아 있다', _gone_ok)
    _require(_gone_ok < _gone_bad < src.index("FAIL=1", _gone_bad),
             'a survivor must raise FAIL; the ✓ line must be reachable only from _gone=1')
    _require('    run kill "$p"' not in src,
             'the pre-repair `run kill` (rc-based verdict, no disappearance check) must not return')

    # 같은 수리 계열: 파괴 단계는 **결과 상태**로 재판정한다(rc 0 은 "명령이 돌았다"일 뿐이고
    # apt-get/sysctl 은 부분 성공·무시된 요청에도 0 을 낸다).
    _require('_st="$(dpkg-query -W -f=\'${Status}\' "$1" 2>/dev/null)"' in src,
             'package removal must be re-read from dpkg-query, not trusted from the apt-get rc')
    _require('*"install ok installed"*) say "   ✗ 패키지 잔존: $1 (status=$_st)"; FAIL=1 ;;' in src,
             'a still-installed package must raise FAIL')
    _require('_v="$(sysctl -n "$1" 2>/dev/null)"' in src,
             'sysctl revert must be re-read from the live key, not trusted from the `sysctl -w` rc')
    _require('if [ -z "$_v" ]; then say "   ✗ $1 을 읽지 못했다 — 되돌림 여부를 **판정할 수 없다**"; FAIL=1' in src,
             'an unreadable sysctl key must be undecidable (FAIL), never reported as reverted')

    # ── 정적: GRUB 백업 검증 후에만 재생성 · 읽지 못한 것 ≠ 없는 것 ─────────────
    _require("GRUB_BAK=/boot/grub/grub.cfg.easy-vllm-purge-backup" in src, "the GRUB backup path must be fixed")
    _require('cmp -s /boot/grub/grub.cfg "$GRUB_BAK"' in src,
             "the GRUB backup must be byte-verified against the CURRENT grub.cfg, not merely created")
    _require('if [ "$GRUB_OK" != "1" ]; then' in src,
             "GRUB regeneration must be skipped when no verified backup was secured")
    _bak_idx, _regen_idx = src.index("GRUB_BAK=/boot/grub"), src.index("GRUB 재생성 (crashkernel")
    _require(_bak_idx < _regen_idx, "the backup must be taken before regeneration, in source order")
    _require("if [ ! -r /boot/grub/grub.cfg ]; then" in src
             and "crashkernel 잔재를 **판정할 수 없다**" in src,
             "an unreadable grub.cfg must be reported as undecidable (FAIL), never as clean")

    # ── 행동: ⓐ Phase 0 시드 게이트가 실제로 거부한다 ───────────────────────────
    probe = ["bash", str(REPO_ROOT / rel), "--node-id=predicate-probe"]
    gated = subprocess.run(probe + ["--seed-dir=/nonexistent-predicate-probe"],
                           cwd=REPO_ROOT, capture_output=True, text=True, timeout=180)
    _require(gated.returncode == 1, f"missing Phase 0 seed must refuse with rc 1 (got {gated.returncode})")
    _require("저널 수확 전 제거 금지" in gated.stdout + gated.stderr,
             "the seed gate must state why it refuses (envelope's only initial data)")
    _require("(dry-run)" not in gated.stdout,
             "the seed gate must refuse BEFORE any removal step is even enumerated")

    # ── 행동: ⓑ 기본 모드는 정말로 아무것도 실행하지 않는다 ─────────────────────
    dry = subprocess.run(probe + ["--no-require-seed"],
                         cwd=REPO_ROOT, capture_output=True, text=True, timeout=180)
    _require(dry.returncode == 0, f"a waived dry-run must succeed without touching the host (rc {dry.returncode})")
    _require("모드: DRY-RUN" in dry.stdout, "the run must announce itself as DRY-RUN")
    _require("DRY-RUN 종료" in dry.stdout and "PURGE PASS" not in dry.stdout,
             "a dry-run must end at the DRY-RUN exit and never emit a purge verdict")
    emitted = [ln for ln in dry.stdout.splitlines() if ln.startswith("        ")]
    _require(emitted, "the dry-run must actually enumerate removal steps (else the probe proves nothing)")
    for ln in emitted:
        _require("(dry-run) " in ln,
                 f"every enumerated removal step must be marked (dry-run), got: {ln.strip()[:80]}")


# =============================================================================
# VARIANT_IMAGE_BUILD_VS_SERVE_PLANE (3 clauses)
# =============================================================================

def predicate_VARIANT_IMAGE_BUILD_VS_SERVE_PLANE_C1():
    """C1: a non-default image variant is cluster-wide on the build plane -- every node, including
    Band2-only slaves, must build/run the exact same image identity (IMAGE_TAG/VLLM_REPO/VLLM_REF)."""
    mn = _read(".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh")
    _require('val(){ grep -E "^$1=" "$EF" | head -1 | cut -d= -f2-; }' in mn, 'image-identity vars must be extracted from the SAME model env file ($EF) master build/up loads')
    _require('IMG=$(val IMAGE_TAG); VREPO=$(val VLLM_REPO); VREF=$(val VLLM_REF); BDF=$(val BUILD_DOCKERFILE)' in mn, 'predicate requirement failed at original line 523')
    # 2026-08-02: VLLM_PRETEND_VERSION 이 이미지 정체성에 추가됐다(포크 태그가 semver 가 아닐 때
    #   setuptools_scm 우회값). 마스터만 갖고 슬레이브가 못 받으면 **슬레이브만 빌드가 죽는다** —
    #   BUILD_DOCKERFILE 이 과거에 잠복했던 것과 동일한 전파 구멍이라 불변식을 확장한다.
    _require('VPV=$(val VLLM_PRETEND_VERSION)' in mn, 'VLLM_PRETEND_VERSION must be extracted from the same model env file')
    # 2026-08-14: SM12X_PORT 도 이미지 정체성에 추가됐다(plan_26081418 G-4). build_patches_src/ 의
    #   소스 이식 패치를 켜는 **변종 게이트**이므로, 빠지면 마스터만 이식본이 되고 슬레이브는 stock
    #   으로 빌드된다 — BUILD_DOCKERFILE·VLLM_PRETEND_VERSION 과 동일 부류의 전파 구멍이다.
    #   `${SMPORT:+...}` 조건부인 이유는 **부재 = stock** 이 기본이기 때문이다(빈 값이면 prefix 자체가
    #   사라져야 하며, `SM12X_PORT=` 를 빈 값으로 흘리면 안 된다). 아래 반증실험이 그 조건을 실증한다.
    _require('SMPORT=$(val SM12X_PORT)' in mn, 'SM12X_PORT must be extracted from the same model env file')
    # 2026-08-15: SRC_DEPS_AUTHORITY 가 네 번째로 이미지 정체성에 추가됐다(R3 포크 핀이 노출).
    #   flashinfer(python+cubin) 의존 승격 게이트다. SM12X_PORT 에서 갈라낸 이유는 두 관심사가
    #   달라서다 — 포크 핀 칸은 **소스 이식이 불요한데 의존 승격은 필요**하고, 게이트가 하나면
    #   그 칸을 켤 수도 끌 수도 없다. 빠지면 마스터만 0.6.17, 슬레이브는 0.6.16.post3 이 되어
    #   BUILD_DOCKERFILE·VLLM_PRETEND_VERSION·SM12X_PORT 와 **동일 부류의 전파 구멍**이 된다.
    #   `${SDA:+...}` 조건부인 이유도 앞의 셋과 같다: **부재 = stock** 이 기본이어야 한다.
    _require('SDA=$(val SRC_DEPS_AUTHORITY)' in mn, 'SRC_DEPS_AUTHORITY must be extracted from the same model env file')
    _require('SLAVE_IMGVARS="${IMG:+IMAGE_TAG=$IMG }${BDF:+BUILD_DOCKERFILE=$BDF }${VREPO:+VLLM_REPO=$VREPO }${VPV:+VLLM_PRETEND_VERSION=$VPV }${SMPORT:+SM12X_PORT=$SMPORT }${SDA:+SRC_DEPS_AUTHORITY=$SDA }${VREF:+VLLM_REF=$VREF}"' in mn, 'predicate requirement failed at original line 524')
    build_line = next(ln for ln in mn.splitlines() if "--profile slave build" in ln)
    _require('$SLAVE_IMGVARS' in build_line, 'slave build invocation must carry the image-identity vars')

    # NEW: "every node ... must build AND run" -- covers master too, and covers `up` not just
    # `build`. The master must build/run from the SAME env file ($EF) the vars were extracted from,
    # and the slave must carry those same vars into its `up` invocation, not just its `build` one.
    master_build_line = next(ln for ln in mn.splitlines() if "--profile master build" in ln)
    master_up_line = next(ln for ln in mn.splitlines() if "--profile master up" in ln)
    slave_up_line = next(ln for ln in mn.splitlines() if "--profile slave up" in ln)
    _require('--env-file "$EF"' in master_build_line, 'master must build from the same model env file the vars came from')
    _require('--env-file "$EF"' in master_up_line, 'master must also RUN (not just build) from that same env file')
    _require('$SLAVE_IMGVARS' in slave_up_line, 'slave must also RUN (not just build) with the same image-identity vars')

    # Execute the REAL extraction+propagation for real (not a static grep of variable names): run
    # val() and the SLAVE_IMGVARS assignment verbatim in bash against a synthetic combo env file,
    # proving the slave genuinely receives the exact same image identity master reads from $EF.
    val_fn = 'val(){ grep -E "^$1=" "$EF" | head -1 | cut -d= -f2-; }'
    assign_line = ('IMG=$(val IMAGE_TAG); VREPO=$(val VLLM_REPO); VREF=$(val VLLM_REF); BDF=$(val BUILD_DOCKERFILE)\n'
                   'VPV=$(val VLLM_PRETEND_VERSION); SMPORT=$(val SM12X_PORT)')
    slave_imgvars_line = next(ln for ln in mn.splitlines() if ln.strip().startswith("SLAVE_IMGVARS="))
    base_env = ("IMAGE_TAG=easy-vllm:0.25.1-cu132-aarch64-source-sm12x\n"
                "VLLM_REPO=https://github.com/jasl/vllm.git\n"
                "VLLM_REF=b5c0d43b967c\n"
                "BUILD_DOCKERFILE=Dockerfile.source-build\n"
                "VLLM_PRETEND_VERSION=0.26.1\n")

    def _imgvars_for(env_text: str) -> str:
        with tempfile.TemporaryDirectory() as tmp:
            ef = Path(tmp) / "combo.env"
            ef.write_text(env_text)
            script = f'EF="{ef}"\n{val_fn}\n{assign_line}\n{slave_imgvars_line.strip()}\necho "$SLAVE_IMGVARS"\n'
            proc = _run_bash(script)
            _require(proc.returncode == 0, proc.stderr)
            return proc.stdout.strip()

    # (a) 변종 콤보: 이식 게이트가 켜져 있으면 슬레이브가 그 값을 그대로 받는다.
    _require(_imgvars_for(base_env + "SM12X_PORT=1\n") == 'IMAGE_TAG=easy-vllm:0.25.1-cu132-aarch64-source-sm12x BUILD_DOCKERFILE=Dockerfile.source-build VLLM_REPO=https://github.com/jasl/vllm.git VLLM_PRETEND_VERSION=0.26.1 SM12X_PORT=1 VLLM_REF=b5c0d43b967c', 'the slave must receive the exact SM12X_PORT gate value the master reads from $EF')
    # (b) 반증실험 — stock 콤보(키 부재): `${SMPORT:+...}` 조건이 prefix 를 통째로 지워야 한다.
    #     빈 `SM12X_PORT=` 가 새면 compose `${SM12X_PORT:-0}` 기본값이 **빈 문자열로 덮여** stock 게이트
    #     비교(`= "1"`)가 아니라 build-arg 자체가 갈리므로, 부재/빈값 구분이 정책 보호의 일부다.
    stock_out = _imgvars_for(base_env)
    _require('SM12X_PORT' not in stock_out, 'an absent SM12X_PORT must vanish from SLAVE_IMGVARS entirely (absence = stock), never leak as an empty assignment')
    _require(stock_out == 'IMAGE_TAG=easy-vllm:0.25.1-cu132-aarch64-source-sm12x BUILD_DOCKERFILE=Dockerfile.source-build VLLM_REPO=https://github.com/jasl/vllm.git VLLM_PRETEND_VERSION=0.26.1 VLLM_REF=b5c0d43b967c', stock_out)


def predicate_VARIANT_IMAGE_BUILD_VS_SERVE_PLANE_C2():
    """C2: strictly a build-plane invariant -- it does not relax the serve-plane invariant that a
    TP slave's runtime env stays Band2-only (.env.cluster + .env.interconnect, no per-model
    CONFIG_FILE env_file entry). Note: the plain `CONFIG_FILE` environment variable IS still set
    on the slave (defaulting to the neutral literal "default") -- what must be absent is an
    env_file PATH keyed off it (i.e. `envs/.env.${CONFIG_FILE}`), which only the master loads."""
    compose = _rendered("compose")
    m = re.search(r"vllm-slave-serve:.*?(?=\n  vllm|\Z)", compose, re.S)
    _require(m, 'vllm-slave-serve service block not found')
    slave_block = m.group(0)
    _require('envs/.env.interconnect' in slave_block and 'envs/.env.cluster' in slave_block, 'predicate requirement failed at original line 570')
    _require('envs/.env.${CONFIG_FILE' not in slave_block, 'slave must never load a model-keyed env_file path off CONFIG_FILE')
    m2 = re.search(r"vllm-master-serve:.*?(?=\n  vllm-slave|\Z)", compose, re.S)
    _require('envs/.env.${CONFIG_FILE' in m2.group(0), 'only the master gets the model-keyed env file')


def predicate_VARIANT_IMAGE_BUILD_VS_SERVE_PLANE_C3():
    """C3: the two planes are independent -- a slave can/must build a cluster-wide variant image
    while still receiving zero model-serve configuration at the container env_file level."""
    mn = _read(".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh")
    master_up = next(ln for ln in mn.splitlines() if "--profile master up" in ln)
    slave_up = next(ln for ln in mn.splitlines() if "--profile slave up" in ln)
    _require(master_up.count('--env-file') == 2, 'master receives BOTH cluster (EFC) and model (EF) env files')
    _require('--env-file "$EF"' in master_up, 'predicate requirement failed at original line 584')
    _require(slave_up.count('--env-file') == 1, 'slave receives ONLY the cluster env file (EFC)')
    _require('$EFC' in slave_up and '"$EF"' not in slave_up and ("'$EF'" not in slave_up), 'predicate requirement failed at original line 586')


# =============================================================================
# MODEL_TRIPLET_NO_SUB_PROPAGATION (3 clauses)
# =============================================================================

def _sync_to_sub_src() -> str:
    return _read(".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh")


def _stem_allowlist(src: str) -> list[str]:
    """BAND2_RUNTIME_PATCH_STEMS 선언을 sync_to_sub.sh 소스에서 파싱해, *구조적 성질*을
    단언하고 stem 목록을 돌려준다.

    F4 교정(2026-07-30): 옛 술어는 이 목록이 `(exaone45-33b hy3)` 와 *리터럴 동일*함을 요구해
    모델 이름이 거버넌스 술어에 결박돼 있었다 — 헌법의 "모델-키잉 아님" 취지와 정면충돌이며,
    새 모델 패치 추가/은퇴 때마다 술어까지 고쳐야 하는 결합이었다. 단언해야 할 것은 특정
    모델명이 아니라 allowlist 의 *성질*이다:
      (a) 명시적 괄호 목록으로 존재하고(와일드카드 권위가 아님 — 그건 별도 _require 가 금지),
      (b) 비어 있지 않으며,
      (c) 각 stem 이 평범한 식별자다(글롭·경로·공백 문자 없음 = 닫힌 목록).
    """
    decl = _extract_bash_array(src, "BAND2_RUNTIME_PATCH_STEMS")
    inner = decl[decl.index("(") + 1: decl.rindex(")")]
    stems = inner.split()
    _require(stems, 'runtime patch stem allowlist must be a non-empty explicit list')
    _require(all(re.fullmatch(r"[a-z0-9][a-z0-9.-]*", s) for s in stems),
             f'every runtime patch stem must be a plain identifier (no glob/path chars): {stems}')
    return stems


def predicate_MODEL_TRIPLET_NO_SUB_PROPAGATION_C1():
    """C1: the per-model triplet (<model>.{yaml,sh} and .env.<model>) is never propagated
    main-to-sub -- structurally excluded from sync_to_sub.sh's rsync filter set (the runtime patch
    owner-local allowlisted <model>_patch.py is the sole model-keyed exception). Verified by running the REAL
    `_band2_filters` rsync filter (extracted verbatim) against a synthetic tree containing both
    Band2 infra and a Band3 model triplet."""
    src = _sync_to_sub_src()
    band2_configs = _extract_bash_array(src, "BAND2_CONFIGS")
    band2_envs = _extract_bash_array(src, "BAND2_ENVS")
    patch_stems = _extract_bash_array(src, "BAND2_RUNTIME_PATCH_STEMS")
    stems = _stem_allowlist(src)
    fn = _extract_bash_function(src, "_band2_filters")
    _require("'/configs/*_patch.py'" not in fn, 'runtime patch wildcard authority is forbidden')

    with tempfile.TemporaryDirectory() as tmp:
        srcdir = Path(tmp) / "src" / "output" / "multi"
        dstdir = Path(tmp) / "dst"
        (srcdir / "configs").mkdir(parents=True)
        (srcdir / "envs").mkdir(parents=True)
        dstdir.mkdir()
        (srcdir / "configs" / "serve_runner.sh").write_text("x")
        (srcdir / "configs" / "mymodel.sh").write_text("x")       # Band3 triplet -- must NOT propagate
        (srcdir / "configs" / "mymodel.yaml").write_text("x")
        (srcdir / "configs" / f"{stems[0]}_patch.py").write_text("x")
        (srcdir / "configs" / f"{stems[0]}_patch.provenance.json").write_text("x")
        (srcdir / "envs" / ".env.interconnect").write_text("x")
        (srcdir / "envs" / ".env.mymodel").write_text("x")        # Band3 model env -- must NOT propagate
        script = f"""
set -e
{band2_configs}
{band2_envs}
{patch_stems}
{fn}
_band2_filters
rsync -a --itemize-changes "${{FILT[@]}}" "{srcdir}/" "{dstdir}/"
"""
        proc = _run_bash(script)
        _require(proc.returncode == 0, proc.stderr)
        delivered = {p.relative_to(dstdir) for p in dstdir.rglob("*") if p.is_file()}
        delivered_names = {str(p) for p in delivered}
        _require('configs/serve_runner.sh' in delivered_names, 'predicate requirement failed at original line 632')
        _require('envs/.env.interconnect' in delivered_names, 'predicate requirement failed at original line 633')
        _require('configs/mymodel.sh' not in delivered_names, 'model triplet .sh leaked to sub')
        _require('configs/mymodel.yaml' not in delivered_names, 'model triplet .yaml leaked to sub')
        _require('envs/.env.mymodel' not in delivered_names, 'model env leaked to sub')
        _require(f'configs/{stems[0]}_patch.py' in delivered_names,
                 'owner-allowlisted runtime patch was not delivered')
        _require(f'configs/{stems[0]}_patch.provenance.json' in delivered_names,
                 'runtime patch provenance was not delivered')


def predicate_MODEL_TRIPLET_NO_SUB_PROPAGATION_C2():
    """C2: a multi-node TP slave is Band2-only -- must boot from .env.cluster (incl. MoE-JIT
    MAX_JOBS) + .env.interconnect alone."""
    compose = _rendered("compose")
    m = re.search(r"vllm-slave-serve:.*?(?=\n  vllm|\Z)", compose, re.S)
    slave_block = m.group(0)
    env_files = re.findall(r"- envs/(\.env\.\S+)", slave_block)
    _require(env_files == ['.env.interconnect', '.env.cluster'], f'slave env_file list must be exactly [.env.interconnect, .env.cluster], got {env_files}')

    # NEW: the claimed MAX_JOBS content -- executed for REAL against the production renderer (not
    # merely grepped from the static output file) -- proving MAX_JOBS genuinely originates from the
    # same deterministic Band2 cluster-env builder the slave's .env.cluster is generated from.
    manifest = {
        "nodes": [{"role": "main", "host": "10.0.0.1", "ssh_user": "u"},
                  {"role": "sub", "host": "10.0.0.2"}],
        "interconnect": {"platform_preset": "dgx-spark-gb10"},
    }
    cluster_env = render_dockerfile.build_cluster_env(manifest)
    _require(cluster_env['MAX_JOBS'] == '4', 'MoE-JIT MAX_JOBS must be part of the Band2 cluster env the slave loads')
    rendered = render_dockerfile.render_cluster_envfile(manifest)
    _require('MAX_JOBS=4' in rendered, 'predicate requirement failed at original line 660')

    # The generated env file is intentionally ignored.  Its contract is therefore proved by the
    # tracked compose consumer above plus this deterministic renderer output; requiring a local
    # materialization would make clean-index verification depend on ambient runtime artifacts.


def predicate_MODEL_TRIPLET_NO_SUB_PROPAGATION_C3():
    """C3: a slave depending on .env.<model> to start is itself the signal that this invariant has
    been violated by a direct-rsync workaround, which is explicitly forbidden -- BAND2_ENVS is a
    fixed, closed literal set (no wildcard that could admit a model env)."""
    src = _sync_to_sub_src()
    band2_configs = _extract_bash_array(src, "BAND2_CONFIGS")
    band2_envs_line = _extract_bash_array(src, "BAND2_ENVS")
    band2_top = _extract_bash_array(src, "BAND2_TOP")
    names = re.findall(r"\.env\.\S+", band2_envs_line.split(")")[0])
    _require(set(names) == {'.env.interconnect', '.env.cluster'}, 'predicate requirement failed at original line 676')
    _require('*' not in band2_envs_line.split('=', 1)[1], 'BAND2_ENVS must contain no wildcard entries')
    # the terminal exclude in _band2_filters catches anything not explicitly allow-listed above.
    fn = _extract_bash_function(src, "_band2_filters")
    _require(fn.rstrip().endswith("--exclude='/*')") or "--exclude='/envs/*'" in fn, 'predicate requirement failed at original line 680')

    # NEW: feed the EXACT violating input this clause describes -- a slave-facing .env.<model> with
    # no matching configs/<model>.{sh,yaml} pair (precisely what a direct-rsync workaround bypassing
    # the normal render pipeline would produce) -- and prove the real S4 classification gate (the
    # mechanism BAND2_ENVS' closedness feeds into) genuinely refuses it, non-zero exit included.
    classify_fn = _extract_bash_function(src, "assert_band_classification")
    with tempfile.TemporaryDirectory() as tmp:
        odir = Path(tmp) / "output" / "multi"
        cdir, edir = odir / "configs", odir / "envs"
        cdir.mkdir(parents=True); edir.mkdir(parents=True)
        for f in ("serve_runner.sh", "debug-init.sh", "arm_patch.sh"):
            (cdir / f).write_text("x")
        for f in (".env.interconnect", ".env.cluster"):
            (edir / f).write_text("x")
        (cdir / "goodmodel.sh").write_text("x")
        (cdir / "goodmodel.yaml").write_text("x")
        (edir / ".env.goodmodel").write_text("x")

        def run_classify():
            script = (f'SRC="{tmp}/"\n{band2_configs}\n{band2_envs_line}\n{band2_top}\n'
                      f'{classify_fn}\nassert_band_classification multi\necho "RC=$?"\n')
            return _run_bash(script)

        proc_clean = run_classify()
        _require('RC=0' in proc_clean.stdout, f'a fully-classified Band2+Band3 tree must pass: {proc_clean.stdout} {proc_clean.stderr}')

        (edir / ".env.roguemodel").write_text("x")  # orphan model env: no configs/roguemodel.{sh,yaml}
        proc_bad = run_classify()
        _require('RC=1' in proc_bad.stdout, 'an orphan model env (the direct-rsync-workaround signature) must be refused, not admitted')
        _require('FAIL(S4 미분류)' in proc_bad.stderr and '.env.roguemodel' in proc_bad.stderr, 'predicate requirement failed at original line 712')


# =============================================================================
# HINT_TAG_ACTIVATION_GATE (6 clauses)
# =============================================================================

def predicate_HINT_TAG_ACTIVATION_GATE_C1():
    """C1: validated recipes are published as hint tags containing distilled knowledge only --
    HEAD stays a pure skeleton (no recipe files at HEAD, only the index); the tag body is the sole
    object. Verified: `cmd_create`'s scaffold is written only to the gitignored `.drafts` staging
    dir, and `cmd_finalize` streams the recipe body straight into the git tag message (`-F -`)
    without ever writing recipe content into a tracked path."""
    _require(hint_tag.DRAFTS_DIR == hint_tag.ROOT / 'hints' / '.drafts', 'predicate requirement failed at original line 725')
    gitignore = _read(".gitignore")
    _require('hints/.drafts/' in gitignore, 'the scaffold staging dir must be gitignored (never at HEAD)')

    src = inspect.getsource(hint_tag.cmd_finalize)
    _require('git(' in src and '"-F", "-"' in src, 'the tag message must be streamed via stdin, not a tracked file')
    tree = ast.parse(inspect.getsource(hint_tag))
    finalize_writes_tracked_file = False
    for node in ast.walk(tree):
        if isinstance(node, ast.FunctionDef) and node.name == "cmd_finalize":
            for sub in ast.walk(node):
                if isinstance(sub, ast.Call) and isinstance(sub.func, ast.Attribute) and sub.func.attr == "write_text":
                    finalize_writes_tracked_file = True
    _require(not finalize_writes_tracked_file, 'finalize must never write recipe content to a tracked file')

    # NEW: "distilled knowledge only ... never a finished, copy-paste-ready recipe payload" is
    # enforced in code by the real B1 backstop -- feed it the exact violating input (an absolute
    # host-scoped number with NO re-measure qualifier, i.e. a value ready to copy-paste verbatim)
    # and prove it's genuinely refused; the same value WITH a re-measure qualifier is accepted.
    copy_paste_ready_body = "권장: kv-cache-memory-bytes 17179869184, gmu 0.80 그대로 사용하세요."
    try:
        hint_tag._assert_remeasure(copy_paste_ready_body)
        raise AssertionError("a copy-paste-ready absolute host number with no re-measure qualifier must be refused")
    except SystemExit as e:
        _require(e.code == 1, 'predicate requirement failed at original line 749')

    distilled_body = "시작점: kv-cache-memory-bytes ≈ 17179869184 bytes — 반드시 재측정(measure) 후 대입, 비이식."
    hint_tag._assert_remeasure(distilled_body)  # must NOT raise -- a genuinely distilled, re-measured value


def predicate_HINT_TAG_ACTIVATION_GATE_C2():
    """C2: PII strip/scan over the full tag object AND tagger identity is fail-closed -- a scan
    failure blocks tag creation/finalization outright. Executes the real `scan_text` detector."""
    terms = ["forbidden-secret-token"]
    # Single RFC1918 fixture literal for this predicate -- reused by every positive case below so
    # the tracked deployment file gains no further private-range literals (plan_26081514 §6.4 note).
    genuine_ip = '192.168.1.5'  # pii-scan-fixture: 탐지기 양성 케이스 — 삭제하면 시험이 죽는다
    _require(hint_tag.scan_text('this text contains forbidden-secret-token here', terms) != [], 'predicate requirement failed at original line 759')
    _require(hint_tag.scan_text(f'{genuine_ip} is a private ip', terms) != [], 'predicate requirement failed at original line 760')
    _require(hint_tag.scan_text('nothing sensitive here at all', terms) == [], 'predicate requirement failed at original line 761')

    # NEW (plan_26081514 §6.4 tripwire, merged plan_26081516 H1): the ipv4 branch cannot tell a
    # document section number from an address on shape alone, so `scan_text` excludes matches
    # anchored by `§` or a heading marker. This NEGATIVE half is the tripwire: with only the
    # positive fixture above, the whole exclusion could be deleted and the self-test would stay
    # green. Positive and negative must move together or review is not forced.
    # The section number is assembled from fragments on purpose: this file is a TRACKED deployment
    # artifact where all four PII patterns are enforced, so spelling an ipv4-shaped literal here
    # would make the fixture trip the very gate it guards (docs.md already states the same rule for
    # its own prose). Not obfuscation -- a self-reference guard.
    sec = '10.' + '1.2'
    _require(hint_tag.scan_text(f'본문 §{sec} 를 참조', terms) == [],
             'a §-anchored document section number must not be flagged as a private ipv4')
    _require(hint_tag.scan_text(f'#### {sec} 관측된 부작용', terms) == [],
             'a heading-anchored document section number must not be flagged as a private ipv4')
    # ...and the exclusion must not SWALLOW a genuine address that follows a section anchor -- the
    # failure mode a `search`-stops-at-first-match implementation would have introduced.
    after_anchor = hint_tag.scan_text(f'§{sec} 요약\n서브 노드 {genuine_ip} 도달', terms)
    _require([h for h in after_anchor if h.startswith('private-ipv4:')] != [],
             'a genuine private ipv4 following a section anchor must still be flagged')
    # tagger identity check explicitly skips the 'email' generic pattern (a tagger MUST have one)
    # but still catches a known PII literal.
    hits = hint_tag.scan_text("Alice <alice@example.com>", ["Alice"], skip_generic=frozenset({"email"}))
    _require(any((h.startswith('term:') for h in hits)), 'predicate requirement failed at original line 765')
    no_email_flag = hint_tag.scan_text("bob@example.com", [], skip_generic=frozenset({"email"}))
    _require(not any((h.startswith('email:') for h in no_email_flag)), 'predicate requirement failed at original line 767')

    fin_src = inspect.getsource(hint_tag.cmd_finalize)
    scan_body_idx = fin_src.index("hits = scan_text(body, terms)")
    die_idx = fin_src.index("if hits:")
    tag_idx = fin_src.index('"tag", "-a", a.tag')
    _require(scan_body_idx < die_idx < tag_idx, 'a PII hit must die() BEFORE the tag object is created')

    # NEW: "the tagger identity that ships inside the pushed tag" must ALSO be fail-closed BEFORE
    # both tag creation and the rest of finalization (index.json/HINTS.md writes) -- not just the
    # body scan checked above.
    idhits_idx = fin_src.index("idhits = scan_text(")
    idhits_die_idx = fin_src.index("if idhits:")
    index_write_idx = fin_src.index("idx = _load_index()")
    _require(idhits_idx < idhits_die_idx < tag_idx < index_write_idx, 'a tagger-identity PII hit must die() before the tag object is created AND before index.json/HINTS.md are written -- blocking both creation and finalization outright')


def predicate_HINT_TAG_ACTIVATION_GATE_C3():
    """C3: every hint carries a mandatory carry-forward revalidation header (a map, not an answer)
    so a stale hint can never be treated as executable without re-verification. Round-trips the
    real footer builder/parser and confirms a missing field is rejected, not silently accepted."""
    # 6-field footer contract (plan_26090222 F-6a): the three content digests were removed --
    # the footer binds an evidence ADDRESS (anchor + refs), integrity is git's job.
    fields = {
        "version": "1", "tag": "hint/0.25.1/gpt-oss-120b/gb10", "topology": "single",
        "anchor": "a" * 40, "manifest_ref": "docs/_evidence/x.json",
        "certificate_ref": "docs/benchmark/cert.yaml",
    }
    _require(tuple(hint_tag._FOOTER_FIELDS) == tuple(fields),
             'footer contract must be exactly the 6 address fields, in order')
    footer_text = hint_tag._build_evidence_footer(fields)
    parsed = hint_tag._parse_evidence_footer(footer_text.split("\n\n", 1)[-1] if "\n\n" in footer_text else
                                              "\n" + footer_text)
    _require(parsed == fields, 'round-tripped footer must equal the original fields exactly')

    broken = footer_text.replace("anchor: " + "a" * 40 + "\n", "")
    try:
        hint_tag._parse_evidence_footer("\n" + broken)
        raise AssertionError("a footer missing a required field must raise HintEvidenceBindingError")
    except hint_tag.HintEvidenceBindingError as e:
        _require(e.code == 'HINT_EVIDENCE_BINDING_MALFORMED', 'predicate requirement failed at original line 806')

    # NEW: "a map, not an answer" + mandatory re-verification against the CONSUMER's OWN smoke --
    # grounded in the actual committed template every hint body is built from (cmd_create reads
    # this exact file; these fixed sentences are outside every {{...}}/TODO(judgment) slot so they
    # survive verbatim into every finalized hint, since finalize only rejects leftover TODO markers,
    # never strips the surrounding fixed prose).
    tpl = _read(".claude/skills/hint-publisher/templates/hint_recipe.template.md")
    fixed_lines = (
        "이 자료는 **지도이지 정답이 아니다.**",
        "네 환경에서 반드시 **스모크 통과까지 재검증**. 최종 판정 = 네 스모크(린트·이슈글 ≠ 서빙됨).",
        "복붙 ✗ = 전략을 **다시 세워라**(carry-forward 금지 · 지도 not 정답).",
    )
    for fixed_line in fixed_lines:
        _require(fixed_line in tpl, f'template must carry the fixed carry-forward-revalidation line: {fixed_line!r}')
        _require('{{' not in fixed_line and 'TODO(judgment' not in fixed_line, 'this must be FIXED prose (never a {{...}}-substituted or judgment-authored slot) so it survives verbatim into every finalized hint')
    _require(hint_tag.TEMPLATE_FILE == hint_tag.ROOT / '.claude' / 'skills' /
             'hint-publisher' / 'templates' / 'hint_recipe.template.md',
             'hint tagger must consume its owner-local template')
    create_src = inspect.getsource(hint_tag.cmd_create)
    _require('TEMPLATE_FILE.read_text' in create_src, 'every hint scaffold must originate from this exact template')


def predicate_HINT_TAG_ACTIVATION_GATE_C4():
    """C4: publication is main-only, the same plane as references.md, independent of sub egress
    state -- sub nodes never author or publish hints. Verified structurally: hint_tag.py contains
    no SSH/sub-node delivery mechanism at all (it only ever touches the LOCAL git repo it was
    invoked in)."""
    src = inspect.getsource(hint_tag)
    for banned in ("SUB_HOST", "ssh ", '"ssh"', "sub_host", "paramiko"):
        _require(banned not in src, f'hint_tag.py must never reference a sub-node delivery mechanism ({banned!r})')
    _require(hint_tag.repo_root.__doc__ is None or 'git' in inspect.getsource(hint_tag.repo_root), 'predicate requirement failed at original line 837')
    _require('"rev-parse", "--show-toplevel"' in inspect.getsource(hint_tag.repo_root), 'predicate requirement failed at original line 838')

    # NEW: "independent of sub egress state" -- hint_tag.py never even references the A2A
    # delegation/egress concepts that gate sub-facing behavior elsewhere in this project, so its
    # publication path structurally cannot branch on them at all.
    for egress_token in ("egress", "delegation", "a2a", "A2A"):
        _require(egress_token not in src, f'hint_tag.py must never reference sub egress/A2A state ({egress_token!r})')

    # NEW: "main-only ... sub nodes never author or publish hints" -- the only two main→sub
    # delivery scripts in this project never deliver the hint-tag engine or the hints/ tree at all,
    # so a sub node structurally has no path to author or publish a hint tag.
    sync_src = _sync_to_sub_src()
    _require('hint_tag' not in sync_src and 'hints/' not in sync_src, 'sync_to_sub.sh must never deliver the hint-tag engine or hints/ tree to a sub node')
    render_sub_env_src = _read(".claude/skills/terraforming_node/scripts/render_sub_env.py")
    _require('hint_tag' not in render_sub_env_src and '"hints"' not in render_sub_env_src, 'render_sub_env.py must never stage the hint-tag engine/tree for sub delivery')


def predicate_HINT_TAG_ACTIVATION_GATE_C5():
    """C5: push is selective -- the refspec is the literal refs/tags/hint/* and --tags is
    forbidden -- so no ref outside the hint namespace reaches a public origin.

    2026-09-03 (plan_26090222 F-6c): the origin-side `last-good-*` ls-remote existence check that
    cmd_verify used to run was DELETED and is no longer pinned here -- the rollback anchor is a
    local COMMIT, never a tag, so that check asserted a condition nothing in this repo can create.
    What is NOT deleted is the namespace guard inside cmd_push: it is a general refspec-leak guard,
    not a last-good-specific one (`--tag '*'` would otherwise render `refs/tags/*` and push every
    local tag to a public origin), so it is pinned below alongside the refspec literal."""
    push_src = inspect.getsource(hint_tag.cmd_push)
    _require('refspec = "refs/tags/hint/*"' in push_src, 'predicate requirement failed at original line 861')
    _require('"--tags"' not in push_src and "'--tags'" not in push_src, 'predicate requirement failed at original line 862')
    # the literal --tags flag must never appear as an actual `git(...)` call argument anywhere in
    # this file (its only other appearances are prose/help-text explaining the prohibition, or the
    # unrelated read-only `ls-remote --tags` existence check inside cmd_verify).
    full_src = inspect.getsource(hint_tag)
    for line in full_src.splitlines():
        if "git(" in line and "push" in line:
            _require('--tags' not in line, f'a push invocation line must never include --tags: {line!r}')
    # the namespace guard itself -- without it `--tag '*'` renders refspec `refs/tags/*`.
    _require('if not a.tag.startswith("hint/")' in push_src,
             'cmd_push must refuse any --tag outside the hint/ namespace (refspec-leak guard)')
    # and the deleted check must stay deleted -- a re-added origin-side scan would be an
    # unenforceable assertion about a tag this repo never creates.
    verify_src = inspect.getsource(hint_tag.cmd_verify)
    _require('last-good' not in verify_src,
             'cmd_verify must not re-introduce an origin-side last-good-* scan (F-6c)')


def predicate_HINT_TAG_ACTIVATION_GATE_C6():
    """C6: activation is proposal-only after all prior work is finished; unattended auto-tagging
    is never allowed; a duplicate triple gets a reverify stamp only, never a fresh tag. Verified:
    `match` (discovery) is the only subcommand absent from the promotion-authorization action map
    (every mutating subcommand requires it), `reverify` never creates a git tag, and re-creating an
    disposable real-Git fixture tag name is refused by `validate_name`."""
    _require('match' not in hint_tag.HINT_ACTION_FOR_CMD, 'match must stay ungated/read-only (proposal step)')
    for cmd in ("create", "finalize", "verify", "reindex", "push", "reverify"):
        _require(cmd in hint_tag.HINT_ACTION_FOR_CMD, f'{cmd} must require promotion authorization')

    reverify_src = inspect.getsource(hint_tag.cmd_reverify)
    _require('"tag", "-a"' not in reverify_src, 'reverify must never create a new git tag')
    _require('entry["last_verified"] = date.today().isoformat()' in reverify_src, 'predicate requirement failed at original line 887')

    # "after all prior work ... is finished" + "proposes (Y/N)" -- grounded in the actual committed
    # skill doc that owns this activation trigger (never a hardcoded restatement).
    skill_src = _read(".claude/skills/upstream-version-watch/SKILL.md")
    _require('전작업 완료 후' in skill_src and '서빙성공+커밋+문서+전파까지 끝난' in skill_src, 'activation must be documented as occurring only after ALL prior work (serving+commit+docs+propagation) is finished')
    _require('**제안(Y/N)** 한다' in skill_src, 'activation must be documented as a Y/N proposal, never automatic')
    _require('무인 자동 태깅 ✗' in skill_src, 'unattended auto-tagging must be documented as never allowed')

    fixture_tag = "hint/0.23.0/deepseek-v4-flash/sm121"
    with tempfile.TemporaryDirectory(prefix="hint-duplicate-predicate.") as tmp:
        def fixture_git(*args: str) -> subprocess.CompletedProcess:
            proc = subprocess.run(["git", "-C", tmp, *args], capture_output=True, text=True)
            _require(proc.returncode == 0, proc.stderr)
            return proc

        fixture_git("init", "-q")
        fixture_git("config", "user.name", "predicate")
        fixture_git("config", "user.email", "predicate@local.invalid")
        (Path(tmp) / "fixture.txt").write_text("duplicate-tag-fixture\n", encoding="utf-8")
        fixture_git("add", "fixture.txt")
        fixture_git("commit", "-q", "-m", "fixture")
        fixture_git("tag", fixture_tag)
        original_root = getattr(hint_tag, "ROOT")
        try:
            setattr(hint_tag, "ROOT", Path(tmp))
            _require(hint_tag.existing_hint_tags() == [fixture_tag],
                     "fixture hint tag must be discoverable")
            try:
                hint_tag.validate_name(fixture_tag)
                raise AssertionError("re-creating an existing tag name must be refused")
            except SystemExit:
                pass
        finally:
            setattr(hint_tag, "ROOT", original_root)


# =============================================================================
# LAST_GOOD_ROLLBACK_ANCHOR (3 clauses)
# =============================================================================

def predicate_LAST_GOOD_ROLLBACK_ANCHOR_C1():
    """C1: the rollback anchor is always the last local commit that passed smoke -- never a
    partially-applied or uncommitted working tree. Executes real git: a tag pinned to a commit
    resolves to that commit regardless of later, uncommitted dirt in the working tree."""
    # This project has no separate deterministic SCRIPT that creates last-good commits/tags -- S4
    # is a documented HITL git workflow (workflow.md), not a script this test can execute without
    # inventing out-of-scope production code. The real, inspectable production artifact for the
    # "selects only a smoke-passed commit" half is the file itself, and its ORDER: the smoke HITL
    # gate must textually precede the S4 last-good-commit step, never follow it.
    workflow_md = _read(".claude/rules/workflow.md")
    gate_idx = workflow_md.index("HITL 게이트 ③ : 스모크 결과(+분류·risk-memo)를 사람이 확인 (smoke-before-commit)")
    s4_idx = workflow_md.index("S4 commit   → 스모크 통과분만 로컬 last-good 커밋")
    _require(gate_idx < s4_idx, 'the smoke-before-commit HITL gate must be documented as preceding the S4 last-good commit step -- never a partially-applied/uncommitted state getting anchored first')
    _require('스모크 통과분만 로컬 last-good 커밋' in workflow_md, 'only smoke-passed work may become a local last-good commit')

    # code-level half: real git -- a tag pinned to a commit resolves to that commit regardless of
    # later, uncommitted dirt in the working tree (the disposability half of the clause).
    with tempfile.TemporaryDirectory() as tmp:
        env = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@t", GIT_COMMITTER_NAME="t",
                   GIT_COMMITTER_EMAIL="t@t")

        def git(*args):
            r = subprocess.run(["git", "-C", tmp, *args], capture_output=True, text=True, env=env)
            _require(r.returncode == 0, r.stderr)
            return r.stdout.strip()

        git("init", "-q")
        (Path(tmp) / "f.txt").write_text("smoke-passed-state")
        git("add", "f.txt")
        git("commit", "-q", "-m", "last-good commit")
        anchor_sha = git("rev-parse", "HEAD")
        git("tag", "-a", "last-good-test", "-m", "anchor")
        (Path(tmp) / "f.txt").write_text("uncommitted dirty change")  # disposable working-tree state
        dirty = git("status", "--porcelain")
        _require(dirty, 'the working tree must be dirty after this edit')
        resolved = git("rev-parse", "last-good-test^{commit}")
        _require(resolved == anchor_sha, 'the anchor tag must still resolve to the committed SHA, not the dirty tree')


def predicate_LAST_GOOD_ROLLBACK_ANCHOR_C2():
    """C2: single-node and multi-node roll back independently -- a multi-node-only failure never
    reverts a passing single-node state. sync_branches.sh (the only script that moves content
    between the two branches) never itself moves/creates a tag or hard-resets a branch, so the
    two branches' last-good anchors can never cross-contaminate through it."""
    src = _read(".claude/skills/upstream-version-watch/scripts/sync_branches.sh")
    _require('git tag' not in src, 'predicate requirement failed at original line 970')
    _require('git reset --hard' not in src, 'predicate requirement failed at original line 971')
    _require('CUR_BRANCH="$(git rev-parse --abbrev-ref HEAD)"' in src, 'predicate requirement failed at original line 972')
    _require('if [ "$CUR_BRANCH" != "$DST_BRANCH" ]; then' in src, 'the script must refuse to operate on any branch other than its declared destination')
    checkout_line = next(ln for ln in src.splitlines() if ln.strip().startswith('git checkout "$SRC_BRANCH"'))
    _require(checkout_line.strip() == 'git checkout "$SRC_BRANCH" -- "${PATHS[@]}"', 'the only cross-branch content transfer must be a pathspec-scoped checkout (working-tree/index only), never a ref-moving operation')

    # NEW: execute the EXACT git primitive this script relies on (a pathspec-scoped
    # `git checkout <src> -- <paths>` while resident on <dst>) for real, and prove it cannot move
    # EITHER branch's commit tip -- the concrete mechanism by which single-node and multi-node
    # last-good anchors stay independent of one another (a multi-node-only failure never reverts a
    # passing single-node commit, because this operation structurally cannot touch either tip).
    with tempfile.TemporaryDirectory() as tmp:
        env = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@t", GIT_COMMITTER_NAME="t",
                   GIT_COMMITTER_EMAIL="t@t")

        def git(*args):
            r = subprocess.run(["git", "-C", tmp, *args], capture_output=True, text=True, env=env)
            _require(r.returncode == 0, r.stderr)
            return r.stdout.strip()

        git("init", "-q", "-b", "single-node")
        (Path(tmp) / "shared.txt").write_text("v1")
        git("add", "shared.txt")
        git("commit", "-q", "-m", "single last-good")
        single_anchor = git("rev-parse", "HEAD")

        git("checkout", "-q", "-b", "multi-node")
        (Path(tmp) / "shared.txt").write_text("v2-broken-multi-attempt")
        git("add", "shared.txt")
        git("commit", "-q", "-m", "multi last-good (pre-failure)")
        multi_anchor = git("rev-parse", "HEAD")

        # resident on multi-node, pull single-node's file content via a pathspec-scoped checkout --
        # exactly what sync_branches.sh does (no branch switch, no ref move).
        git("checkout", "single-node", "--", "shared.txt")
        _require(git('rev-parse', '--abbrev-ref', 'HEAD') == 'multi-node', 'a pathspec-scoped checkout must never switch the current branch')
        _require(git('rev-parse', 'single-node') == single_anchor, "single-node's own last-good anchor must be untouched by a sync run against multi-node")
        _require(git('rev-parse', 'multi-node') == multi_anchor, "multi-node's own committed anchor must also stay untouched (only the working tree/index changed -- the human commits deliberately, per the S4 HITL gate)")
        _require(git('status', '--porcelain'), 'the pathspec checkout must genuinely stage a change, not silently no-op')


def predicate_LAST_GOOD_ROLLBACK_ANCHOR_C3():
    """C3: recovery restores the working tree to the anchor commit; origin, when configured, comes
    from the user's own environment value (manifest.origin_url) -- no hardcoded remote."""
    tmpl = _read("manifest.template.yaml")
    _require(re.search('^origin_url:\\s*""', tmpl, re.M), 'origin_url must default empty (user-supplied)')
    for rel in (".claude/skills/hint-publisher/scripts/hint_tag.py", ".claude/skills/upstream-version-watch/scripts/sync_branches.sh"):
        src = _read(rel)
        _require('github.com/' not in src and 'git@github.com' not in src, f'{rel} must not hardcode a project remote URL')
    push_src = inspect.getsource(hint_tag.cmd_push)
    _require('"--remote", default="origin"' not in push_src, 'predicate requirement failed at original line 1029')  # not asserting the arg literally this way
    parser_src = inspect.getsource(hint_tag.main)
    _require('default="origin"' in parser_src, 'the remote name defaults to the generic git alias, not a URL')

    # NEW: "recovery restores the working tree to that anchor commit" -- performed for REAL (not
    # merely SHA resolution, which C1 already covers): commit past the anchor with a broken
    # follow-up, then actually restore the working tree, and verify the content genuinely reverts.
    with tempfile.TemporaryDirectory() as tmp:
        env = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@t", GIT_COMMITTER_NAME="t",
                   GIT_COMMITTER_EMAIL="t@t")

        def git(*args):
            r = subprocess.run(["git", "-C", tmp, *args], capture_output=True, text=True, env=env)
            _require(r.returncode == 0, r.stderr)
            return r.stdout.strip()

        git("init", "-q")
        (Path(tmp) / "f.txt").write_text("smoke-passed-state")
        git("add", "f.txt")
        git("commit", "-q", "-m", "last-good")
        git("tag", "-a", "last-good-recover-test", "-m", "anchor")

        (Path(tmp) / "f.txt").write_text("broken-followup-change")
        git("add", "f.txt")
        git("commit", "-q", "-m", "broken follow-up")
        _require((Path(tmp) / 'f.txt').read_text() == 'broken-followup-change', 'predicate requirement failed at original line 1054')

        git("checkout", "-q", "last-good-recover-test", "--", "f.txt")
        _require((Path(tmp) / 'f.txt').read_text() == 'smoke-passed-state', 'recovery must actually restore the working tree content to the anchor commit')


# =============================================================================
# MODEL_ACQUISITION_TERNARY_GATE (5 clauses)
# =============================================================================

def predicate_MODEL_ACQUISITION_TERNARY_GATE_C1():
    """C1: model_source has exactly three modes (managed/ephemeral/custom), each with distinct
    storage/deletion/mount semantics; per-node override resolves node-value-if-present else the
    cluster default; the runtime mount plane is read-only regardless of mode."""
    _require(manifest_contract.VALID_MODES == ('managed', 'ephemeral', 'custom'), 'predicate requirement failed at original line 1069')
    _require(manifest_contract.effective_model_source({'model_source': 'ephemeral'}, 'managed') == 'ephemeral', 'predicate requirement failed at original line 1070')
    _require(manifest_contract.effective_model_source({}, 'managed') == 'managed', 'predicate requirement failed at original line 1071')
    _require(manifest_contract.effective_model_source(None, 'custom') == 'custom', 'predicate requirement failed at original line 1072')

    # Code-level: evaluate_contract (the real manifest-reading gate every runtime skill validates
    # against) surfaces the mode-specific storage-location fields as distinct, real keys -- managed's
    # nas_model_path and custom's custom_model_paths are not collapsed into one undifferentiated blob.
    man = {
        "topology": "single", "gpus_per_node": 1, "model_source": "custom",
        "nas_model_path": "/mnt/models", "custom_model_paths": {"my-model": "/data/models/my-model"},
        "nodes": [], "terraforming": {"complete": True, "branch_verified": True},
    }
    res = manifest_contract.evaluate_contract(man, "single")
    _require(res['flag'] is True, 'predicate requirement failed at original line 1083')
    _require(res['nas_model_path'] == '/mnt/models', 'predicate requirement failed at original line 1084')
    _require(res['custom_model_paths'] == {'my-model': '/data/models/my-model'}, 'predicate requirement failed at original line 1085')

    # Code-level: the tracked, hand-authored production docker-compose.yaml (multi's canonical
    # container definition, CLAUDE.md's one tracked exception to generated output) is the real
    # mount-semantics artifact. managed/custom both bind-mount a host path read-only (":ro") -- "no
    # unauthorized download" is a mount-permission fact here, not prose. Ephemeral has NO entry at
    # all: the only volume lines are the three NAS/QUANT/TIKTOKEN host mounts + configs, and there is
    # no top-level (Docker-managed, persists across `down`) `volumes:` block anywhere in the file --
    # so an ephemeral in-container HF-cache download lives solely in the container's writable layer
    # and is destroyed when the container is removed.
    compose = _rendered("compose")
    _require(re.search('NAS_MODEL_PATH.*:/app/models:ro', compose), 'managed/custom model mount must be read-only')
    _require('(managed/custom 마운트원)' in compose, 'the read-only host mount must be documented as the managed/custom storage location')
    _require(not re.search('^volumes:\\s*$', compose, re.M), "no top-level Docker-managed named volume may back model storage -- ephemeral's container-internal cache must be deletable by container removal alone, never persisted by a volume")

    # Persona-level grounding (never a hardcoded restatement -- read live): the actual committed
    # manifest.template.yaml literally documents each mode's distinct storage/deletion/mount
    # semantics, at the single field terraforming's interview fills.
    tmpl = _read("manifest.template.yaml")
    _require('컨테이너 내부 HF 캐시에 임시 다운로드(컨테이너 down→삭제)' in tmpl, 'ephemeral must be documented as container-cache download, deleted on container down')
    _require('사용자 지정 경로에 모델 저장·볼륨마운트' in tmpl and 'custom_model_paths 매핑 사용' in tmpl, 'custom must be documented as a user-specified path, volume-mounted')
    _require('관리 NAS read-only 마운트 기본·권장' in tmpl, 'managed must be documented as a read-only NAS mount')
    _require('서빙 런타임 마운트는 read-only' in tmpl, 'the cross-mode invariant (runtime mount is always read-only) must be documented')


def predicate_MODEL_ACQUISITION_TERNARY_GATE_C2():
    """C2: any external fetch requires per-event user approval -- check_smoke_model.py (the model
    presence gate every serve path runs through) contains no download machinery at all and never
    branches on model_source (so selecting a mode is structurally incapable of standing-authorizing
    a download at the one place models get consumed for serving); its STOP-and-exit(2) path literally
    states the no-download rule; and the one real network-reaching function in the acquisition
    surface (crosscheck_model_card's HF lookup) demands an explicit repo id on every single
    invocation -- no persisted/default repo a blanket approval could attach to, and no auto-approve
    flag anywhere in that CLI that could let one approval silently cover future fetches."""
    src = _read(".claude/skills/upstream-version-watch/scripts/check_smoke_model.py")
    for banned in ("urllib", "requests", "snapshot_download", "huggingface_hub", "wget", "curl",
                   "http.client", "socket", "httpx", "aiohttp", "os.system", "os.popen", "Popen"):
        _require(banned not in src, f'no download/shell mechanism {banned!r} may appear in check_smoke_model.py')
    # subprocess 는 로컬 스펙스크립트(sys.executable) 실행 전용 — 다운로드 지시 명령 전달 금지.
    _require('subprocess.run([sys.executable, spec_script' in src, 'the only subprocess call must run sys.executable on the local spec script, never a download command')
    _require('model_source' not in src, 'the serve-time model-presence gate must never branch on model_source -- selecting a mode (even complete+ephemeral) must not, by itself, be able to authorize anything at this gate')
    stop_idx = src.index('f"[NAS-check] STOP: 모델 부재')
    exit_idx = src.index("sys.exit(2)", stop_idx)
    segment = src[stop_idx:exit_idx]
    _require('다운로드하지 않음' in segment, 'the exit(2) STOP path must literally state the no-download rule')

    ccsrc = _read(".claude/skills/vllm-recipe-explorer/scripts/crosscheck_model_card.py")
    _require(re.search('add_argument\\("--hf-repo-id",\\s*default=None', ccsrc), '--hf-repo-id must have no default -- each fetch names its target explicitly, per-event')
    for banned_flag in ("--yes", "--auto-approve", "--no-confirm", "--skip-confirm", "--force"):
        _require(banned_flag not in ccsrc, f'no blanket-authorization flag ({banned_flag}) may exist on the external-fetch CLI')
    main_src = inspect.getsource(crosscheck_model_card.main)
    _require('--hf-repo-id 필수' in main_src, 'missing --hf-repo-id on --ephemeral-estimate must abort rather than silently proceeding with an implicit/blanket target')
    missing_idx = main_src.index("--hf-repo-id 필수")
    exit3_idx = main_src.index("sys.exit(3)", missing_idx)
    _require(exit3_idx > missing_idx, 'the missing-repo-id message must precede a real abort(exit 3)')


def predicate_MODEL_ACQUISITION_TERNARY_GATE_C3():
    """C3: external cross-check of a serving strategy against BOTH the HF model card and vLLM
    GitHub is always required regardless of acquisition mode -- the HF-card half is a real,
    mode-agnostic function (no model_source parameter anywhere in its signature) that genuinely
    detects a coarse-label precision mismatch; the vLLM-GitHub half has no deterministic script (it
    is explicitly documented as an agent-performed web search, not a script constraint -- SKILL.md
    L181), so it is grounded by reading the actual committed SKILL.md text at test time, which
    orchestrates both halves together and ties the requirement explicitly to being independent of
    the acquisition-mode isolation plane."""
    sig = inspect.signature(crosscheck_model_card.crosscheck)
    _require('model_source' not in sig.parameters and 'mode' not in sig.parameters, 'predicate requirement failed at original line 1164')

    cfg = {"quantization_config": {"quant_method": "fp8"}}
    cprec = crosscheck_model_card.config_precision(cfg)
    _require(cprec.startswith('fp8'), 'predicate requirement failed at original line 1168')
    card = "| DeepSeek-V4-Flash | FP4 + FP8 Mixed (experts FP4) |\n"
    cardp = crosscheck_model_card.card_precision(card, "DeepSeek-V4-Flash")
    _require(cardp.get('mixed_note') or cardp.get('table_row'), 'predicate requirement failed at original line 1171')

    skill_md = _read(".claude/skills/vllm-recipe-explorer/SKILL.md")
    orchestration_idx = skill_md.index(
        "서빙전략의 외부 교차검증(HF 모델카드·vLLM GitHub)은 허용·의무")
    isolation_idx = skill_md.index("모델획득 격리 한정", orchestration_idx)
    _require(orchestration_idx < isolation_idx, "the HF-card + vLLM-GitHub cross-check orchestration must be documented as mandatory ('허용·의무'), immediately scoped as independent of acquisition-mode isolation")
    _require('외부접속 전반 차단 ✗' in skill_md[isolation_idx:isolation_idx + 40], 'the doc must make explicit that this is NOT blocked by the acquisition-mode egress restriction plane -- it always runs, regardless of managed/ephemeral/custom')


def predicate_MODEL_ACQUISITION_TERNARY_GATE_C4():
    """C4: an hf_token, when needed, is registered as a manifest file pointer only -- the raw token
    is never tracked, and the one real production consumer of that pointer (scan_node.py's model-env
    check) is AST-verified to only ever test the pointed-at file's existence (bool/os.path.isfile) --
    it never opens/reads the file's content, so the raw token value never enters process memory or
    any structure that gets logged/tracked."""
    tmpl = _read("manifest.template.yaml")
    _require(re.search('hf_token_env_file:\\s*""', tmpl), 'must default to an empty pointer field')
    gitignore = _read(".gitignore")
    _require('.env' in gitignore.splitlines() and 'envs/.env.*' in gitignore.splitlines(), 'env files (where a real hf token would live) must be gitignored')

    # Code-level: read_manifest_field (real production function) resolves the pointer -- a path
    # string, never file content -- from a controlled, hermetic manifest.
    with tempfile.TemporaryDirectory() as tmp:
        mpath = os.path.join(tmp, "manifest.yaml")
        with open(mpath, "w", encoding="utf-8") as f:
            f.write('hf_token_env_file: /some/host/path/token.env\n')
        _require(scan_node.read_manifest_field(mpath, 'hf_token_env_file') == '/some/host/path/token.env', 'predicate requirement failed at original line 1203')

    # Code-level (AST, mutation-sensitive): every call site in scan_node.py that passes the
    # `hf_token_env_file` variable as an argument must be an existence check (bool()/os.path.isfile())
    # -- never open()/read_text()/Path(...).open() or any content-reading call. A regression that
    # added a content read of the token file would be caught here even if it never printed anything.
    sn_src = _read(".claude/skills/terraforming_node/scripts/scan_node.py")
    tree = ast.parse(sn_src)
    call_names = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Call):
            args = list(node.args) + [kw.value for kw in node.keywords]
            if any(isinstance(a, ast.Name) and a.id == "hf_token_env_file" for a in args):
                fn = node.func
                name = fn.attr if isinstance(fn, ast.Attribute) else getattr(fn, "id", ast.dump(fn))
                call_names.append(name)
    _require(call_names, 'hf_token_env_file must actually be consumed somewhere (not dead code)')
    # 2026-09-03(S6 · plan_26090317 P1): 서브의 토큰 존재 여부는 **서브에서** 봐야 한다(메인 fs 를
    #   보던 것이 S6 결함이다). `collect_peer_model_env` 는 경로를 `shlex.quote` 해 원격 `[ -f ]` 로만
    #   넘기고 **내용을 읽지 않는다** — 존재검사의 원격판이므로 허용 집합에 넣는다. 그 함수가 내용을
    #   읽도록 바뀌면 아래 소스 단언(open/read_text 금지)이 잡는다.
    _ALLOWED_TOKEN_SINKS = {'bool', 'isfile', 'quote', 'collect_peer_model_env'}
    _require(set(call_names) <= _ALLOWED_TOKEN_SINKS,
             f'hf_token_env_file may only be passed to existence checks (local or remote), found: {call_names}')
    _peer_src = _extract_python_function(sn_src, 'collect_peer_model_env')
    _require('open(' not in _peer_src and 'read_text' not in _peer_src and 'cat ' not in _peer_src,
             'the remote existence probe must never read the token file content')
    _require('[ -f ' in _peer_src, 'the remote probe must be an existence test, not a content read')
    _require('open(hf_token_env_file' not in sn_src and 'read_text' not in sn_src, "the raw token file's content must never be read")


def predicate_MODEL_ACQUISITION_TERNARY_GATE_C5():
    """C5: for ephemeral/custom approval, a pre-download size estimate must accompany the ask; a
    failed lookup is reported honestly, never replaced by a fabricated substitute value. Both halves
    of that contract are executed for real: the failure half (existing) and, NEW, the success half --
    a real HF-API response (network I/O stubbed at the one function boundary that performs it;
    everything downstream is genuine production arithmetic) must actually produce a non-fabricated
    GiB estimate, and SKILL.md is read live to ground the "must accompany the approval ask" half."""
    r = crosscheck_model_card.crosscheck_external_vram(None, None, None)
    _require(r['verdict'] == 'UNAVAILABLE', 'predicate requirement failed at original line 1234')
    _require(r['external_total'] is None, 'no fabricated external total on lookup failure')
    _require('repo_id' in r['reason'] or '미지정' in r['reason'], 'predicate requirement failed at original line 1236')

    # success half: stub only fetch_hf_safetensors_total (the sole network boundary) so the rest of
    # crosscheck_external_vram's ephemeral-estimate arithmetic runs for real.
    orig_fetch = crosscheck_model_card.fetch_hf_safetensors_total
    try:
        crosscheck_model_card.fetch_hf_safetensors_total = lambda repo_id, timeout=8.0: {
            "ok": True, "total": 1_000_000, "parameters": {"BF16": 500_000, "F8_E4M3": 500_000}}
        r_ok = crosscheck_model_card.crosscheck_external_vram(None, None, "org/model")
    finally:
        crosscheck_model_card.fetch_hf_safetensors_total = orig_fetch
    _require(r_ok['verdict'] == 'OK' and r_ok['mode'] == 'ephemeral-estimate', 'predicate requirement failed at original line 1247')
    _require(r_ok['external_total'] == 1000000, 'predicate requirement failed at original line 1248')
    _require(r_ok['external_native_weight_bytes_est'] is not None, 'a successful ephemeral lookup must produce a real (non-None) pre-download size estimate')
    _require(r_ok['external_native_weight_gib_est'] is not None, 'predicate requirement failed at original line 1251')

    # persona-level grounding (read live, never a hardcoded restatement): the actual committed
    # SKILL.md mandates that this estimate be surfaced together with the ephemeral approval ask.
    skill_md = _read(".claude/skills/vllm-recipe-explorer/SKILL.md")
    _require('ephemeral 다운로드 승인 요청 시 이' in skill_md, 'the doc must tie the pre-download estimate to the ephemeral approval ask')
    _require('사전추정 결과(대략 GiB)를 사용자에게 함께 제시할 것' in skill_md, 'the doc must mandate presenting the estimate (in GiB) to the user at approval time')


# =============================================================================
# KV_ABSOLUTE_CLAMP_PORTABILITY (4 clauses)
# =============================================================================

def predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C1():
    """C1: a final recipe sizes KV cache by a measured, absolute per-target-GPU byte value, never
    by gpu-memory-utilization alone. `_reject_target_gpu_phase1` enforces this by refusing Phase-1
    (gmu-only) whenever config.target_gpu is defined; and the FINAL emission function
    (gen_recipe_set._build_yaml -- the actual generator of the triplet yaml that becomes the vLLM
    --kv-cache-memory-bytes CLI flag) is executed for real and proven to emit the exact absolute
    integer byte value verbatim, and to never emit that line (no gmu-derived substitute) when no
    absolute measurement exists."""
    try:
        recipe._reject_target_gpu_phase1({"target_gpu": {"gpu_model": "x"}}, "estimate")
        raise AssertionError("must die() when target_gpu is defined under Phase-1")
    except SystemExit as e:
        _require(e.code == 7, 'predicate requirement failed at original line 1278')
    recipe._reject_target_gpu_phase1({}, "estimate")  # no target_gpu -> no-op, must not raise

    # Final-emission proof: real gen_recipe_set._build_yaml, the function whose output literally
    # becomes configs/<name>.yaml (which `vllm serve --config` reads as --kv-cache-memory-bytes).
    parsed = {"model_path_container": "/app/models/org/model", "model_id": "org/model"}
    recipe_with_kv = {"quantization": "native", "gpu_memory_utilization": 0.85,
                      "max_model_len": 8192, "kv_cache_memory_bytes": 123456789}
    yaml_text = gen_recipe_set._build_yaml(parsed, recipe_with_kv, "served")
    _require('kv-cache-memory-bytes: 123456789' in yaml_text.splitlines(), 'the measured absolute byte value must be emitted verbatim, unmodified')

    recipe_no_kv = {"quantization": "native", "gpu_memory_utilization": 0.85, "max_model_len": 8192}
    yaml_text_no_kv = gen_recipe_set._build_yaml(parsed, recipe_no_kv, "served")
    _require('kv-cache-memory-bytes' not in yaml_text_no_kv, 'without a measured absolute value, no kv-cache-memory-bytes line (gmu-derived or otherwise) may appear -- KV sizing is never approximated from the gmu ratio alone')


def predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C2():
    """C2: gmu is still emitted as the startup free-memory gate/cap, clamped to <= 0.90 on
    unified-memory targets; discrete targets are in-scope with no hard clamp. AND: the final
    emission (gen_recipe_set._build_yaml, same real generator as C1) co-emits gpu-memory-utilization
    ALONGSIDE kv-cache-memory-bytes (never one without the other once an absolute KV value exists),
    with the real generated comment marking gmu's role as the free-memory gate/cap only -- proving
    the co-emission and the role division are both actual generator behavior, not documentation."""
    per_card, gmu, model = recipe.resolve_target_gpu_budget(
        {"target_gpu": {"gpu_model": "NVIDIA RTX PRO 6000", "target_gmu": 0.95}}, tp=1)
    _require(per_card == 96.0 and gmu == 0.95 and (model == 'NVIDIA RTX PRO 6000'), 'discrete GPU: no hard clamp, per-card VRAM from references.md')

    per_card2, gmu2, model2 = recipe.resolve_target_gpu_budget(
        {"target_gpu": {"gpu_model": "NVIDIA GB10", "target_gmu": 0.95, "per_card_vram_gib": 120}}, tp=1)
    _require(gmu2 == 0.9, 'unified-memory GPU (GB10) must hard-clamp target_gmu to 0.90')

    # Co-emission proof: with an absolute KV value present, BOTH lines appear together, plus the
    # real generated comment stating gmu is only the startup free-memory gate/total cap once the
    # absolute clamp is set.
    parsed = {"model_path_container": "/app/models/org/model", "model_id": "org/model"}
    recipe_with_kv = {"quantization": "native", "gpu_memory_utilization": gmu2,
                      "max_model_len": 8192, "kv_cache_memory_bytes": 987654321}
    lines = gen_recipe_set._build_yaml(parsed, recipe_with_kv, "served").splitlines()
    _require('gpu-memory-utilization: 0.9' in lines and 'kv-cache-memory-bytes: 987654321' in lines, 'gpu-memory-utilization must be co-emitted alongside kv-cache-memory-bytes, never dropped')
    _require(any(('startup free-memory 게이트' in ln and 'cap' in ln for ln in lines)), 'the generated yaml must carry the real comment marking gmu as gate/cap only once the absolute KV clamp controls sizing')

    # Without an absolute KV value, gmu is still emitted (it is the sole safety mechanism in that
    # mode) but the gate/cap-only annotation comment must NOT appear (that framing only applies once
    # kv-cache-memory-bytes exists and gmu stops controlling KV sizing).
    recipe_no_kv = {"quantization": "native", "gpu_memory_utilization": 0.85, "max_model_len": 8192}
    lines_no_kv = gen_recipe_set._build_yaml(parsed, recipe_no_kv, "served").splitlines()
    _require('gpu-memory-utilization: 0.85' in lines_no_kv, 'predicate requirement failed at original line 1331')
    _require(not any(('startup free-memory 게이트' in ln for ln in lines_no_kv)), 'predicate requirement failed at original line 1332')


def predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C3():
    """C3: when porting host-measured weights/overhead/per-token-KV to a different target GPU, the
    budget is the target GPU's VRAM times its target_gmu -- an invariant TRANSFER, not an
    approximation, and not a reversal of any prior verdict. Proven by actually CALLING the real
    downstream production function that performs this transfer (recipe._resolve_clamp_kv, which
    internally calls estimate_vram.max_safe_kv_bytes) -- never by reimplementing the
    per_card*target_gmu multiplication locally in this test (that would only prove
    resolve_target_gpu_budget's return values, not that the transfer is actually applied downstream).

    The scenario: an IDENTICAL host-measured profile (weights=40GiB, overhead=4GiB) is ported to the
    SAME target budget (50GiB @ 0.90) at two different tp_divisors (1 vs 2, i.e. TP=1 vs TP=2 on the
    target). Dividing the host-measured invariants by TP=2 frees enough of the target's own budget
    that an identical required-KV request flips from infeasible to feasible -- a genuine, freshly
    recomputed verdict driven purely by the arithmetic, not a memoized/carried-forward one (proven by
    re-running the tp_divisor=1 case again afterward and getting the identical FAIL back)."""
    cfg = {"target_gpu": {"gpu_model": "NVIDIA RTX PRO 6000", "target_gmu": 0.90,
                          "per_card_vram_gib": 50.0}}
    budget, margin, model = recipe.resolve_target_gpu_budget(cfg, tp=1)
    _require((budget, margin, model) == (50.0, 0.9, 'NVIDIA RTX PRO 6000'), 'predicate requirement failed at original line 1353')

    GIB = recipe.GIB
    # host-measured invariants (weights/overhead) + a measured per-token KV rate (kv_cache_gib over
    # kv_cache_tokens, from a real trial) -- NOT the dims-formula fallback, to keep this scenario
    # anchored in "measured", per the policy statement's own words.
    profile = {"weights_gib": 40.0, "non_kv_overhead_gib": 4.0,
              "kv_cache_gib": 1.0, "kv_cache_tokens": 1000}
    # required = (kv_cache_gib*GIB/kv_cache_tokens) * max_model_len * batch * KV_BLOCK_ALIGN_BUFFER
    #          = (GIB/1000)*10000*1 * buffer = 10*GIB*buffer -- by construction, independent of `parsed`.
    # The buffer is READ FROM PRODUCTION (never re-spelled here): vLLM allocates KV in fixed blocks
    # (block_size=16 tokens), so the linear estimate can land a hair under vLLM's own
    # `_check_enough_kv_cache_memory` threshold -- measured 2026-08-11 on gemma-4-e2b-it and
    # qwen3-4b ("needed X > available X": a display tie that is an internal overshoot).
    # Hard-coding 10*GIB here would re-assert a stale arithmetic and break the moment production
    # legitimately re-tunes the buffer -- which is exactly how this predicate went stale.
    candidate = {"max_model_len": 10000, "batch": 1}
    expected_kv = int(10 * GIB * recipe.KV_BLOCK_ALIGN_BUFFER)

    r_tp1 = recipe._resolve_clamp_kv({}, candidate, profile, budget, margin, 2, tp_divisor=1)
    # tp_divisor=1: max_safe = int(50*0.90*GIB) - 40*GIB - 4*GIB = 45*GIB - 44*GIB = 1*GIB.
    # required(~10.2*GIB) > max_safe(1*GIB) -> infeasible: the FULL (undivided) host weights/overhead
    # do not leave room on the target for the requested KV.
    _require(r_tp1['kv'] is None and r_tp1['fail']['failure_class'] == 'vram_infeasible', "tp_divisor=1: undivided host weights/overhead must exceed the target budget for this required KV -- if this doesn't fail, the transfer isn't using the real host measurement")

    r_tp2 = recipe._resolve_clamp_kv({}, candidate, profile, budget, margin, 2, tp_divisor=2)
    # tp_divisor=2: weights/overhead are the SAME host invariants, transferred (halved, not
    # re-measured/approximated) -> max_safe = 45*GIB - 20*GIB - 2*GIB = 23*GIB >= required(~10.2*GIB).
    _require(r_tp2['fail'] is None and r_tp2['kv'] == expected_kv, 'tp_divisor=2: dividing the SAME host-measured weights/overhead by the target TP must free enough budget to satisfy the identical KV request -- proving the transfer actually applies the host invariants (not a reversal/ignoring of them, not an independent re-approximation)')

    # not a reversal of any prior verdict / not memoized: an identical repeat of the tp_divisor=1
    # call, run AFTER the tp_divisor=2 call, must reproduce the exact same FAIL -- proving each call
    # is a fresh, independent recomputation from (budget, margin, tp_divisor), never cached/leaked
    # state from the previous port.
    r_tp1_again = recipe._resolve_clamp_kv({}, candidate, profile, budget, margin, 2, tp_divisor=1)
    _require(r_tp1_again == r_tp1, 'porting the identical host profile through a different tp_divisor and back must not leak state -- each port is an independent recomputation, not a carried-forward verdict')


def predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C4():
    """C4: target hardware is config-time intent (vllm-recipe-explorer's own config.yaml),
    orthogonal to manifest-observed hardware facts -- `_lookup_gpu_spec` reads ONLY
    references.md, never any manifest.yaml path -- AND `resolve_target_gpu_budget` (the function
    that actually computes the config-time budget consumed downstream) is likewise manifest-free by
    source inspection, in explicit CONTRAST to `_target_tp` (a different, real function in the same
    module) which DOES read manifest state -- proving the config-intent/manifest-observed separation
    is a deliberate architectural split between two distinct functions, not merely an absent check."""
    src = inspect.getsource(recipe._lookup_gpu_spec)
    _require('manifest' not in src.lower(), '_lookup_gpu_spec must never read manifest state')
    per_card, is_unified = recipe._lookup_gpu_spec("NVIDIA GB10")
    _require(is_unified is True and per_card is None, "GB10's references.md section is marked unified-memory but has no per-card VRAM line (quick-win override required) -- proves this is a static reference lookup, not a live scan")
    per_card2, is_unified2 = recipe._lookup_gpu_spec("nonexistent-gpu-xyz")
    _require(per_card2 is None and is_unified2 is None, 'predicate requirement failed at original line 1406')

    budget_src = inspect.getsource(recipe.resolve_target_gpu_budget)
    _require('manifest' not in budget_src.lower() and '_read_manifest' not in budget_src, 'resolve_target_gpu_budget (config-time intent plane) must never read manifest state -- it is a pure function of cfg.target_gpu + the static references.md lookup')

    # Contrast: _target_tp is a genuinely different function in the SAME module that DOES read
    # manifest.yaml (via _read_manifest) -- for validating TP consistency, a distinct concern from
    # budget computation. Its presence proves manifest-observed facts live on their own plane,
    # reached only through a different, clearly-separated function -- not that the module simply
    # never touches manifest state at all.
    tp_src = inspect.getsource(recipe._target_tp)
    _require('_read_manifest' in tp_src, "_target_tp must read manifest state (node_count) -- its contrast with resolve_target_gpu_budget's manifest-free source is the proof of a deliberate split, not an accidental absence of manifest access anywhere in the module")


# =============================================================================
# RUNTIME_PATCH_NO_CARRY_FORWARD (4 clauses)
# =============================================================================

def predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C1():
    """C1: a model that cannot boot on stock is fixed with stock image + a volatile, untracked
    runtime patch (<model>_patch.py) -- never a derived image. sync_to_sub.sh's Band2 filter admits
    `*_patch.py` as the sole model-keyed file (proving it travels alongside the stock image rather
    than requiring a rebuilt/derived one). Extended: (a) "never a derived image" is proven by
    `build_context` (the ONE function that actually computes the IMAGE_TAG/Dockerfile selection
    docker-compose builds from) taking no model-identifying input at all, and by arm_patch.sh
    resolving the patch's identity from a runtime env var rather than a name baked in at build time;
    (b) "volatile, untracked" is proven at the git level: the exact directory the patch and the
    compose bind-mount both live in (output/<topology>/configs/) sits under a blanket gitignore
    whose carve-out set is closed and never re-includes configs/ at all."""
    src = _sync_to_sub_src()
    fn = _extract_bash_function(src, "_band2_filters")
    _stem_allowlist(src)
    _require("'/configs/*_patch.py'" not in fn, 'wildcard runtime-patch transfer is forbidden')
    band2_configs = _extract_bash_array(src, "BAND2_CONFIGS")
    _require('serve_runner.sh' in band2_configs and 'debug-init.sh' in band2_configs, 'predicate requirement failed at original line 1444')
    _require('_patch.py' not in band2_configs, 'runtime patches must remain separate from generic runner configs')

    # NEW atom -- "never a derived image": the image-tag/Dockerfile-selection builder takes no
    # model/patch input at all -- structurally incapable of keying the image to a model's patch.
    sig = inspect.signature(render_dockerfile.build_context)
    _require(list(sig.parameters) == ['manifest', 'resolved'], 'the image-tag builder must never accept a model/patch parameter -- a model-derived image would require exactly that, and its absence is what forces the stock-image, no-rebuild path')
    sample_ctx = render_dockerfile.build_context(
        {"cpu_arch": "aarch64"},
        {"vllm_version": "0.25.1", "ngc_base": {"tag": "26.05-py3", "cuda_version": "13.2"},
         "build_track": {"decision": "source-build"}})
    _require(sample_ctx['IMAGE_TAG'] == '0.25.1-cu132-aarch64-source', sample_ctx['IMAGE_TAG'])
    _require('arm_patch.sh' in render_dockerfile.RUNNER_SCRIPTS, "arm_patch.sh must be one of the generic, model-agnostic runner scripts materialized into the stock image's output pathway regardless of which model is later served")

    # NEW atom -- the arming script itself resolves the patch's identity from a runtime env var
    # (CONFIG_FILE), never a name baked in at image-build time -- one stock script serves any model.
    arm_src = _shared_asset("arm_patch.sh")
    _require('_cfg="${CONFIG_FILE:-default}"' in arm_src and '_patch="/app/configs/${_cfg}_patch.py"' in arm_src, 'the patch path must be resolved from a runtime env var, never a name baked in at build time')

    # NEW atom -- "volatile, untracked": the REAL git-level enforcement is the output/<topology>/*
    # blanket ignore, whose carve-out set is closed and never re-includes configs/ (where the
    # compose bind-mount and the runtime patch both live) -- not mere operator discipline.
    gitignore = _read(".gitignore")
    output_block_start = gitignore.index("output/*/*")
    output_block_end = gitignore.index("\n\n", output_block_start)
    output_block = gitignore[output_block_start:output_block_end]
    reincludes = re.findall(r"^!(\S+)", output_block, re.M)
    # Closed carve-out set (tripwire): any addition must be reviewed here, not slipped into
    # .gitignore alone. Updated 2026-08-13 (plan_26081313 / plan_26081310):
    #   - single-통로 빌드킷 예외는 8/11(02b22d4)에 추가됐으나 이 목록이 갱신되지 않아, 그때부터
    #     이 술어가 계속 FAIL 이었다(검증기 BLOCKED 방치). 그 누락분을 함께 정합화한다.
    #   - build_patches_src/(빌드패치 pre 위상)는 배달 경로 신설분(plan_26081310 A).
    #   - 같은 날 2차 축소: `build_patches_src/**` → `*.sh` + `PROVENANCE.json`. `**` 는 payload
    #     `files/`(업스트림 vLLM 소스 벤더링 92파일·61,846줄)까지 추적물로 끌어들였는데, 그것은
    #     손작성 정본이 아니라 **파생 산출물**이라 CLAUDE.md 의 추적 규정(빌딩블럭=추적·생성물=비추적)에
    #     어긋난다. payload 는 .gitignore 의 `output/*/build_patches_src/files/` 로 명시 제외한다 —
    #     `output/*/*` 의 `*` 가 `/` 를 넘지 않아 4단계 경로를 덮지 못하기 때문에 **명시 제외가 필수**다.
    #     ★ 이 tripwire 는 그 축소를 정확히 잡아 리뷰를 강제했다(설계대로 발화한 실례).
    # 본질 불변식은 바로 아래 줄이다: configs/ 는 어떤 형태로도 재포함되지 않는다.
    _require(reincludes == [
        'output/*/.gitkeep',
        'output/multi/Dockerfile', 'output/multi/Dockerfile.source-build',
        'output/multi/Dockerfile.source-build-upstage', 'output/multi/docker-compose.yaml',
        'output/multi/requirements.txt',
        'output/single/Dockerfile', 'output/single/Dockerfile.source-build',
        'output/single/Dockerfile.source-build-upstage', 'output/single/docker-compose.yaml',
        'output/single/requirements.txt',
        'output/multi/build_patches/', 'output/multi/build_patches/*',
        'output/single/build_patches/', 'output/single/build_patches/*',
        'output/multi/build_patches_src/', 'output/multi/build_patches_src/*.sh',
        'output/multi/build_patches_src/PROVENANCE.json',
        'output/single/build_patches_src/', 'output/single/build_patches_src/*.sh',
        'output/single/build_patches_src/PROVENANCE.json',
    ], f'unexpected output/ gitignore carve-outs -- must never re-include configs/: {reincludes}')
    # payload 제외가 실제로 걸려 있는지 — allowlist 만으로는 files/ 가 기본 추적으로 남는다(위 주석).
    _require('output/*/build_patches_src/files/' in gitignore,
             'build_patches_src payload(files/) 명시 제외가 없다 — allowlist 축소만으로는 vendored 소스가 추적된다')
    _require(not any(('configs' in r for r in reincludes)), 'output/<topology>/configs/ (where the compose bind-mount and the runtime patch actually live) must have NO re-inclusion carve-out -- proving it is genuinely, structurally untracked')
    compose = _rendered("compose")
    _require('- ./configs:/app/configs:ro' in compose, 'the container must bind-mount the SAME blanket-ignored configs/ directory the patch lives in')


def predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C2():
    """C2: the patch is authored main-side only and delivered downward; re-derived per environment
    rather than carried forward -- confirmed by sync_to_sub.sh being a strictly downward (main to
    sub) delivery script with no upward patch-collection path. Extended: (a) the ONLY upward
    (sub->main) script in this repo, fetch_sub_docs.sh, is proven to structurally reach nothing but
    the sub's docs/ subtree -- it cannot pull configs/*_patch.py back from a sub, which is what
    "main-authored only" actually rests on; (b) "re-derived per environment, not carried forward" is
    proven by running the REAL `_band2_filters` wildcard against two DIFFERENT model-keyed patch
    files (standing in for two different environments/bump-cycles) and confirming BOTH travel
    generically -- the mechanism re-admits whatever the CURRENT environment freshly authored, never
    a single carried-forward name."""
    src = _sync_to_sub_src()
    _require('deliver_build' in src and 'deliver_overlay' in src, 'predicate requirement failed at original line 1503')
    executable = "\n".join(line for line in src.splitlines()
                           if line.strip() and not line.lstrip().startswith("#"))
    for banned in ("fetch_sub_docs", "git pull", "collect_patch"):
        _require(banned not in executable, 'predicate requirement failed at original line 1507')
    _require('BAND2_RUNTIME_PATCH_STEMS' in src, 'explicit owner-local runtime patch allowlist is required')

    # NEW atom -- "main-authored only, no upward collection path": the ONE upward (sub->main) script
    # in this repo is fetch_sub_docs.sh, and both its dry-run and --apply rsync invocations pull
    # FROM the sub's docs/ subtree only -- it never references configs/ at all, so it structurally
    # cannot reach configs/*_patch.py.
    fetch_src = _read(".claude/skills/upstream-version-watch/scripts/fetch_sub_docs.sh")
    fetch_executable = "\n".join(line for line in fetch_src.splitlines()
                                  if line.strip() and not line.lstrip().startswith("#"))
    _require('configs' not in fetch_executable, 'the sole upward (sub->main) script must never execute a configs/ transfer -- proving the runtime patch has no path back from a sub, i.e. it can only be authored main-side')
    dryrun_call = re.search(r'rsync -an[^\n]*"\$SUB_HOST:\$SUB_WORK_DIR/docs/"[^\n]*"\$DEST/"', fetch_src)
    apply_call = re.search(r'rsync -az[^\n]*"\$SUB_HOST:\$SUB_WORK_DIR/docs/"[^\n]*"\$DEST/"', fetch_src)
    _require(dryrun_call and apply_call, "both the dry-run and --apply rsync invocations must pull FROM the sub's docs/ subtree only")

    # "re-derived per environment rather than carried forward" is enforced by the production
    # cryptographic provenance validator, not inferred from a wildcard filename. Stamp binds patch
    # bytes + topology + current model inputs + canonical production resolution; any drift rejects.
    validator = REPO_ROOT / ".claude/skills/upstream-version-watch/scripts/validate_runtime_patch.py"
    resolution = REPO_ROOT / ".claude/skills/upstream-version-watch/assets/current-production-resolution.json"
    fn = _extract_bash_function(src, "_band2_filters")
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp); configs = root / "configs"; envs = root / "envs"
        configs.mkdir(); envs.mkdir()
        patch = configs / "modelA_patch.py"
        patch.write_text("# freshly derived patch\n", encoding="utf-8")
        (configs / "modelA.yaml").write_text("model: A\nversion: current\n", encoding="utf-8")
        (envs / ".env.modelA").write_text("MODEL=A\n", encoding="utf-8")
        common = ["--patch", str(patch), "--topology", "multi", "--config-dir", str(configs),
                  "--env-dir", str(envs), "--resolution", str(resolution)]
        stamped = subprocess.run([sys.executable, str(validator), "stamp", *common],
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        _require(stamped.returncode == 0, stamped.stderr)
        verified = subprocess.run([sys.executable, str(validator), "verify", *common],
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        _require(verified.returncode == 0, verified.stderr)
        patch.write_text("# stale carried-forward patch with no current derivation\n", encoding="utf-8")
        stale = subprocess.run([sys.executable, str(validator), "verify", *common],
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        _require(stale.returncode != 0, 'stale patch bytes must fail closed despite a generic *_patch.py name')

    _require('validate_runtime_patches "$1"' in src, 'predicate requirement failed at original line 1551')
    _require('_patch.provenance.json' in fn, 'rsync filter must transfer provenance with each allowlisted patch')


def predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C3():
    """C3: deterministic arming (.pth auto-apply via arm_patch.sh) applies the patch at container
    start. Inspects the tracked arming script's exact guard → purelib lookup → writable check →
    `.pth` emission ordering and verifies the emitted loader executes the selected patch module.
    Extended: proves the REAL, tracked serve_runner.sh -- the exact script docker-compose's
    `command:` runs for both the master and slave services -- sources arm_patch.sh unconditionally
    before either role does anything else (ray head start/model source on master, ray worker join
    on slave), so container startup genuinely invokes arming rather than the mechanism merely
    existing, unreferenced, on disk."""
    src = _shared_asset("arm_patch.sh")
    guard = src.index('if [ -f "$_patch" ]; then')
    purelib = src.index('sysconfig.get_paths()["purelib"]')
    writable = src.index('[ -w "$_sp" ]')
    emit = src.index('> "${_sp}/zzz_${_cfg}_patch.pth"')
    _require(guard < purelib < writable < emit, 'predicate requirement failed at original line 1569')
    _require("spec_from_file_location('__cfgpatch'" in src, 'predicate requirement failed at original line 1570')
    _require('s.loader.exec_module(m)' in src, 'the emitted .pth must load and execute the exact runtime patch at Python site-init')

    # NEW atom -- "container startup/serve runner actually invokes arming": ordering-sensitive proof
    # against the REAL, tracked serve_runner.sh that arming is sourced before either role's own
    # startup action -- arming after ray start would miss the Ray/TP workers entirely.
    runner = _shared_asset("serve_runner.sh")
    arm_guard_idx = runner.index('if [ -f /app/configs/arm_patch.sh ]; then')
    arm_source_idx = runner.index("source /app/configs/arm_patch.sh")
    master_branch_idx = runner.index('if [ "${NODE_ROLE}" = "master" ]; then')
    master_ray_start_idx = runner.index("ray start --head")
    master_model_source_idx = runner.index('source "${MODEL_SH}"')
    slave_branch_idx = runner.index('elif [ "${NODE_ROLE}" = "slave" ]; then')
    slave_ray_start_idx = runner.index("exec ray start --address=")
    _require(arm_guard_idx < arm_source_idx < master_branch_idx, 'arming must be sourced before the master/slave role branch splits at all -- both roles must inherit it unconditionally')
    _require(arm_source_idx < master_ray_start_idx < master_model_source_idx, 'on master, arming must run before ray head start AND before the model script is sourced')
    _require(arm_source_idx < slave_branch_idx < slave_ray_start_idx, 'on slave, arming must run before the ray worker itself joins the cluster')

    # NEW atom -- this really is the script docker-compose's container `command:` executes for BOTH
    # the master and slave services (not merely a script that exists on disk, unreferenced) -- the
    # ordering proven above is only real if THIS is the actual entrypoint.
    compose = _rendered("compose")
    master_cmd = re.search(r"vllm-master-serve:.*?command:\s*(\[.*?\])", compose, re.S).group(1)
    slave_cmd = re.search(r"vllm-slave-serve:.*?command:\s*(\[.*?\])", compose, re.S).group(1)
    _require('/app/configs/serve_runner.sh' in master_cmd and '/app/configs/serve_runner.sh' in slave_cmd, 'docker-compose must actually invoke serve_runner.sh as the container command on both master and slave')


def predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C4():
    """C4: a source-build-time patch is the one exception -- frozen into the tracked
    Dockerfile.source-build for reproducibility. `_patch_guard` fails the build (exit 1) for an
    unvalidated (ngc_tag, vllm_version) key by default, and the ONLY way to "freeze" a new key is
    an actual source-code edit to VALIDATED_SOURCE_BUILD_KEYS -- the runtime HITL-override flag
    never mutates that set itself. Extended: proves the exception is really baked into the tracked,
    buildable output/multi/Dockerfile.source-build (not merely that the Python catalog function
    could produce such text in the abstract) -- the guard is the sole RUN between FROM and the
    first real build step, is a proceeding guard, and its exact NGC/vLLM pair is accepted by the
    production catalog guard."""
    _require(('26.05-py3', '0.24.0') in render_dockerfile.VALIDATED_SOURCE_BUILD_KEYS, 'predicate requirement failed at original line 1614')
    guarded = render_dockerfile._patch_guard("26.05-py3", "0.24.0")
    _require('validated patch set' in guarded and 'exit 1' not in guarded, 'predicate requirement failed at original line 1616')

    before = set(render_dockerfile.VALIDATED_SOURCE_BUILD_KEYS)
    orig_allow = render_dockerfile.ALLOW_UNVALIDATED
    try:
        render_dockerfile.ALLOW_UNVALIDATED = True
        warned = render_dockerfile._patch_guard("99.99-py3", "9.9.9", )
        _require('UNVALIDATED' in warned and 'HITL' in warned, 'predicate requirement failed at original line 1623')
        _require(set(render_dockerfile.VALIDATED_SOURCE_BUILD_KEYS) == before, 'even an approved attempt-build must never silently mutate the frozen validated set')
    finally:
        render_dockerfile.ALLOW_UNVALIDATED = orig_allow
    default_refused = render_dockerfile._patch_guard("99.99-py3", "9.9.9")
    _require(default_refused.rstrip().endswith('exit 1'), 'predicate requirement failed at original line 1629')

    # NEW atom -- "frozen into the tracked Dockerfile.source-build": inspect the REAL, buildable
    # artifact docker-compose's Dockerfile.source-build target actually is, not the abstract catalog.
    dockerfile = _rendered("source-build")
    from_idx = dockerfile.index("FROM nvcr.io/nvidia/pytorch:")
    from_line_end = dockerfile.index("\n", from_idx)
    ngc_tag = dockerfile[from_idx:from_line_end].split(":", 1)[1].strip()
    guard_idx = dockerfile.index('RUN echo "[guard] source-build key', from_line_end)
    next_run_idx = dockerfile.index("\nRUN ", guard_idx + 1)
    guard_block = dockerfile[guard_idx:next_run_idx]
    _require(guard_block.count('RUN echo') == 1, "the frozen guard must be the sole RUN instruction between FROM and the first real build step -- matching the template's own FROM-then-guard placement contract")
    _require('exit 1' not in guard_block, 'the guard actually baked into the tracked, buildable Dockerfile must be a validated/proceeding guard, not a refusal -- proving THIS specific combo was really graduated, not merely theorized in the abstract Python catalog')
    vllm_ref_idx = dockerfile.index("ARG VLLM_REF=v")
    vllm_ref_line_end = dockerfile.index("\n", vllm_ref_idx)
    tracked_vllm_version = dockerfile[vllm_ref_idx:vllm_ref_line_end].split("=v", 1)[1].strip()
    tracked_key = (ngc_tag, tracked_vllm_version)
    _require(tracked_key in render_dockerfile.VALIDATED_SOURCE_BUILD_KEYS, f'tracked source-build key {tracked_key!r} must be graduated in the production catalog')
    exact_guard = render_dockerfile._patch_guard(*tracked_key)
    _require('validated patch set' in exact_guard and 'exit 1' not in exact_guard, 'production guard must accept the exact tracked Dockerfile source-build pair')


# =============================================================================
# SUB_SYNC_DIRTY_AUTOSAVE (4 clauses)
# =============================================================================

def predicate_SUB_SYNC_DIRTY_AUTOSAVE_C1():
    """C1: before delivering anything, sync_to_sub.sh checks the sub's working-tree cleanliness
    via `git status --porcelain` -- the exact command `sub_dirty()` wraps for remote execution,
    executed here for real against a local dirty tree."""
    src = _sync_to_sub_src()
    fn = _extract_bash_function(src, "sub_dirty") if re.search(r"^sub_dirty\s*\(\)", src, re.M) else None
    _require('sub_dirty() { sub_run "git status --porcelain"; }' in src,
             'sub_dirty must wrap exactly `git status --porcelain` for remote execution')
    # 2026-09-03(F1 · plan_26090317 P1): 이전 정의는 `2>/dev/null` 로 실패 사유를 지웠고, 호출부의
    #   `|| true` 와 합쳐져 **판독 실패가 clean 으로 접혔다**. 보존 게이트의 목적이 소실 방지인데
    #   "모르는 상태" 를 "깨끗함" 으로 읽으면 그 목적이 정확히 뒤집힌다. 사유 보존을 원자로 고정한다.
    _require('2>/dev/null' not in _extract_bash_function(src, "sub_dirty"),
             'sub_dirty must not discard stderr -- an unreadable sub tree must surface its cause, '
             'not be folded into "clean"')
    with tempfile.TemporaryDirectory() as tmp:
        subprocess.run(["git", "init", "-q", tmp], check=True)
        (Path(tmp) / "f").write_text("untracked")
        out = subprocess.run(["git", "-C", tmp, "status", "--porcelain"], capture_output=True, text=True)
        _require(out.stdout.strip(), 'an untracked/dirty file must be reported by the exact command sub_dirty wraps')


def predicate_SUB_SYNC_DIRTY_AUTOSAVE_C2():
    """C2: on dirty, main PRESERVES the sub's work as an `[improve]` commit in the sub's local git --
    never a stash. A stash is volatile; sub git exists precisely so main can track sub work history,
    so discarding that history to unblock delivery would defeat the reason the gate exists."""
    src = _sync_to_sub_src()
    b1_section = src[src.index("# ── B1 per-branch"):]
    dirt_idx = b1_section.index('DIRT="$(sub_dirty)"')
    stage_idx = b1_section.index('sub_run "git add -A"', dirt_idx)
    commit_idx = b1_section.index('sub_commit "[improve] pre-sync autosave', stage_idx)
    _require(dirt_idx < stage_idx < commit_idx,
             'the dirty branch must stage then commit the sub work, in that order')
    # No stash anywhere in the dirty-handling branch: preservation must be durable, not volatile.
    branch = b1_section[dirt_idx:b1_section.index("보존 완료", commit_idx)]
    commands = [line.strip() for line in branch.splitlines()
                if line.strip() and not line.lstrip().startswith("#")]
    _require(not any(re.search('(^|sub_run\\s+["\\u0027])git\\s+stash\\b', line, re.I) for line in commands),
             'the dirty-handling branch must never stash -- preservation is by commit, so the work survives in history')


def predicate_SUB_SYNC_DIRTY_AUTOSAVE_C3():
    """C3: the preservation commit strictly precedes any checkout or rsync -- proven against REAL
    control flow, so no sub-authored change can be overwritten before it is recorded. This is the
    clause that carries the gate's ORIGINAL purpose (loss prevention); what was dropped in the
    2026-08-13 correction was only the consent demand, which addressed a subject that does not
    exist on the main<->sub plane."""
    src = _sync_to_sub_src()
    loop = src[src.index('for t in "${TARGETS[@]}"; do\n    if [ "$t" = "single"'):]
    dirt_idx = loop.index('DIRT="$(sub_dirty)"')
    if_idx = loop.index('if [ -n "$DIRT" ]; then', dirt_idx)
    stage_idx = loop.index('sub_run "git add -A"', if_idx)
    commit_idx = loop.index('sub_commit "[improve] pre-sync autosave', stage_idx)
    checkout_idx = loop.index('sub_run "git checkout -q $t"', commit_idx)
    build_idx = loop.index('deliver_build "$t" 0', checkout_idx)
    _require(dirt_idx < if_idx < stage_idx < commit_idx < checkout_idx < build_idx,
             'preservation (stage+commit) must live inside the dirty branch and strictly precede '
             'checkout, which must strictly precede delivery -- otherwise sub work could be '
             'clobbered before it is recorded')

    # Re-verification after preservation: the tree must actually be clean before delivery proceeds,
    # so a partial/failed preservation cannot silently pass through into an overwrite.
    reverify_idx = loop.index('RE_DIRT="$(sub_dirty)"', commit_idx)
    _require(commit_idx < reverify_idx < checkout_idx,
             'the post-preservation cleanliness re-check must sit between the commit and any checkout')

    # No CLI escape hatch may bypass the dirty handling: the script's own argument parser is a
    # CLOSED enumeration (`case "$1" in ... *) unknown argument -> exit 2 ... esac`).
    case_start = src.index('case "$1" in')
    case_end = src.index("esac", case_start)
    arg_parser = src[case_start:case_end]
    _require(re.search('force|skip|dirty|override|bypass', arg_parser, re.I) is None,
             'the closed CLI flag case-block must contain no dirty-handling bypass flag of any spelling')
    _require('*) echo "[sync] FAIL: unknown argument: $1" >&2; exit 2 ;;' in arg_parser,
             'an unrecognized flag must itself be rejected fail-closed -- proving the enumerated flag set is genuinely closed')

    # Exactly two live probes per invocation and no more: the initial one and the post-preservation
    # re-verification. Neither is cached across runs. Word-boundary match -- a bare `.count()` on
    # `DIRT=` also matches inside `RE_DIRT=` and would silently miscount.
    _require(len(re.findall(r'(?<![A-Z_])DIRT="\$\(sub_dirty\)"', src)) == 1,
             'the initial dirty probe must appear exactly once, computed live against the sub tree')
    _require(len(re.findall(r'RE_DIRT="\$\(sub_dirty\)"', src)) == 1,
             'the post-preservation re-verification probe must appear exactly once, recomputed live '
             '(never reusing the pre-preservation result, which would mask a failed preservation)')


def predicate_SUB_SYNC_DIRTY_AUTOSAVE_C4():
    """C4: delivery is refused (non-zero exit) ONLY when preservation fails or dirt remains after
    it. Delivering without preservation is the sole real loss risk, so that -- not the absence of
    the sub's consent -- is what fail-closed must guard."""
    src = _sync_to_sub_src()
    loop = src[src.index("for t in \"${TARGETS[@]}\"; do\n    if [ \"$t\" = \"single\""):]
    if_idx = loop.index('if [ -n "$DIRT" ]; then')
    checkout_idx = loop.index('sub_run "git checkout -q $t"', if_idx)
    dirty_branch = loop[if_idx:checkout_idx]

    # Every terminal exit inside the dirty branch must be a preservation-failure path.
    exits = [m.start() for m in re.finditer(r"exit 8", dirty_branch)]
    # 2026-09-03(F1): 네 번째 거부 경로 = 보존 후 **재판독 실패**. "판독 불가" 는 clean 이 아니라
    #   보존 실패와 같은 급이며, 그렇게 읽지 않으면 게이트 전체가 fail-open 이 된다.
    _require(len(exits) == 4,
             'the dirty branch must have exactly four refusal paths: stage failure, commit failure, '
             'residual dirt after preservation, and an unreadable post-preservation re-probe')
    for pos in exits:
        line_start = dirty_branch.rfind("\n", 0, pos) + 1
        stmt = dirty_branch[line_start:dirty_branch.index("\n", pos)]
        _require("STOP" in stmt and ("보존" in stmt or "배달 거부" in stmt),
                 'each refusal inside the dirty branch must be a preservation-failure stop, not a consent demand')

    # The gate must not refuse merely because the tree was dirty: a clean exit path past
    # preservation has to exist, i.e. the branch falls through to checkout.
    _require("보존 완료" in dirty_branch,
             'the dirty branch must have a success path that proceeds to delivery after preserving')


# =============================================================================
# SUB_GIT_LOCAL_ONLY (3 clauses)
# =============================================================================

def predicate_SUB_GIT_LOCAL_ONLY_C1():
    """C1: a sub node's git workspace is local-only -- origin is permanently unconfigured. The B0
    bootstrap explicitly asserts zero remotes after `git init`, never adding one."""
    src = _sync_to_sub_src()
    _require('sub_run "git init -q"' in src, 'predicate requirement failed at original line 1767')
    _require('git remote add' not in src, 'sync_to_sub.sh must never itself add a remote to the sub')
    # 2026-09-03(F9 · plan_26090317 P1): `|| true` 는 **판독 실패를 "원격 0" 으로** 만들었다 —
    #   로컬 전용 불변식이 읽지 못한 사실 위에 서 있었다. 이제 실패는 exit 7 이고, 확증은 실제 판독일 때만 선다.
    m = re.search(r'REMOTES="\$\(sub_run \'git remote\'\)"', src)
    _require(m, "the origin-zero attestation must read `git remote` for real -- a swallowed read "
                "failure must never be able to print the local-only confirmation")
    confirm_idx = src.index('origin 0 (로컬 전용 확증)')
    _require(confirm_idx > m.start(), 'predicate requirement failed at original line 1772')


def predicate_SUB_GIT_LOCAL_ONLY_C2():
    """C2: push/pull/fetch/remote/clone are denied at the persona layer -- the real
    `CLAUDE.template.md` that `render_sub_env.render_tree` maps 1:1 onto the sub's actual, delivered
    `CLAUDE.md` (never a decorative, unread document) -- and that denial is backed by mutation-
    sensitive inspection of the real dispatch channel: every literal command string sync_to_sub.sh
    ever passes through `sub_run` (its one mechanism for issuing a command to the sub over SSH) is
    extracted and checked -- none is a push/pull/fetch/clone invocation, and the sole `git remote`
    dispatch found is the bare, read-only zero-remote confirmation, never a remote-mutating subform
    such as `git remote add`."""
    tmpl = _read(".claude/skills/terraforming_node/sub_node/CLAUDE.template.md")
    denylist = "단 **`git push`·`git pull`·`git fetch`·`git remote`·`git clone` 금지**(origin 영구 없음)."
    allowlist = "`git add/commit/checkout/switch/branch/status/diff/log/stash` **허용**"
    _require(denylist in tmpl, 'the exact push/pull/fetch/remote/clone denial must be verbatim in the sub persona')
    _require(allowlist in tmpl, 'the exact local-only allowlist must be verbatim in the sub persona')

    # Not decorative: this exact template is render_sub_env's own production input for the sub's
    # real CLAUDE.md -- otherwise the persona denial above would be prose nobody's Claude Code reads.
    tree_src = inspect.getsource(render_sub_env.render_tree)
    _require('_render_file("CLAUDE.template.md", "CLAUDE.md", "md")' in tree_src, "CLAUDE.template.md must be the exact file render_tree writes as the sub's real CLAUDE.md")

    # Structural backing: extract every literal command string ever dispatched through sub_run and
    # inspect actual command content -- not mere absence of banned words anywhere in the file.
    src = _sync_to_sub_src()
    dispatched = re.findall(r'sub_run\s+"([^"]*)"', src) + re.findall(r"sub_run\s+'([^']*)'", src)
    _require(len(dispatched) >= 10, 'expected many real git/shell dispatches routed through sub_run')
    mutating = re.compile(r"git\s+(push|pull|fetch|clone)\b")
    remote_mutating = re.compile(r"git\s+remote\s+(add|set-url|rename|remove|rm)\b")
    offenders = [c for c in dispatched if mutating.search(c) or remote_mutating.search(c)]
    _require(not offenders, f'sub_run dispatched a banned git subcommand: {offenders}')
    remote_dispatches = [c for c in dispatched if re.search(r"git\s+remote\b", c)]
    _require(remote_dispatches == ['git remote'], f'the only git-remote dispatch must be the bare read-only zero-remote confirmation, got: {remote_dispatches}')


def predicate_SUB_GIT_LOCAL_ONLY_C3():
    """C3: git on a sub node exists only to switch between the single/multi branches for
    model-load-strategy selection and to keep local history/rollback -- never a recovery or
    publication vehicle back to main. Proven three ways: (1) the real B0 bootstrap git sequence
    executed verbatim against a real repo, switching between exactly multi/single via the exact
    read-only command `sub_branch_current` wraps (not a generic temp-git branch demo); (2)
    sync_to_sub.sh's own `--branch` argument is a closed enumeration of exactly multi/single/both
    (fail-closed `exit 6` otherwise), and its delivery functions dispatch strictly main-to-sub
    (SUB_HOST is never the rsync SOURCE); (3) the real, sole sub-to-main upward channel,
    fetch_sub_docs.sh, never invokes git at all -- it rsyncs only the sub's docs/ subtree, read-only
    on the sub, in both its dry-run and apply paths."""
    src = _sync_to_sub_src()

    # (1) exact literal B0 sequence, executed for real -- the exact commands sync_to_sub.sh issues
    # to bootstrap the sub's local-only repo, mirrored bit-for-bit rather than an arbitrary demo.
    for literal in ('sub_run "git init -q"', 'sub_run "git branch -m multi"',
                    'sub_run "git branch single"', 'sub_run "git checkout -q multi"',
                    'sub_branch_current() { sub_run "git rev-parse --abbrev-ref HEAD"; }'):
        _require(literal in src, f'B0 bootstrap must issue the exact literal: {literal}')
    with tempfile.TemporaryDirectory() as tmp:
        env = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@t", GIT_COMMITTER_NAME="t",
                   GIT_COMMITTER_EMAIL="t@t")

        def run(*args):
            subprocess.run(["git", "-C", tmp, *args], check=True, env=env, capture_output=True, text=True)

        def head():
            return subprocess.run(["git", "-C", tmp, "rev-parse", "--abbrev-ref", "HEAD"],
                                   capture_output=True, text=True, env=env).stdout.strip()

        run("init", "-q")
        (Path(tmp) / "f").write_text("x")
        run("add", "f")
        run("commit", "-q", "-m", "x")
        run("branch", "-m", "multi")   # exact B0 literal: sub_run "git branch -m multi"
        run("branch", "single")        # exact B0 literal: sub_run "git branch single"
        run("checkout", "-q", "multi")
        _require(head() == 'multi', 'predicate requirement failed at original line 1847')
        run("checkout", "-q", "single")
        _require(head() == 'single', 'the real B0 sequence must switch strictly between multi and single')

    # (2) branch selection is a closed enumeration, fail-closed on anything else -- git on a sub is
    # never asked to switch to an arbitrary branch outside model-load-strategy selection.
    _require('case "$BRANCH" in multi|single|both) ;; *) echo "[sync] FAIL: --branch must be multi|single|both" >&2; exit 6 ;; esac' in src, 'predicate requirement failed at original line 1853')
    _require('TARGETS=(); case "$BRANCH" in multi) TARGETS=(multi);; single) TARGETS=(single);; both) TARGETS=(multi single);; esac' in src, 'predicate requirement failed at original line 1855')

    # Delivery is strictly main-to-sub: SUB_HOST is always the rsync DESTINATION in both delivery
    # functions, never the source -- so this script cannot double as an upward recovery conduit.
    deliver_build_fn = _extract_bash_function(src, "deliver_build")
    deliver_overlay_fn = _extract_bash_function(src, "deliver_overlay")
    _require('"$src" "$SUB_HOST:$dst"' in deliver_build_fn, 'predicate requirement failed at original line 1862')
    _require('"$st/" "$SUB_HOST:$DEST"' in deliver_overlay_fn, 'predicate requirement failed at original line 1863')
    _require(re.search('"\\$SUB_HOST:[^"]*"\\s+"\\$(src|st)"', deliver_build_fn + deliver_overlay_fn) is None, 'SUB_HOST must never appear as the rsync SOURCE (reversed direction) in either delivery function')

    # (3) the real, sole upward (sub-to-main) channel never touches git at all, and is scoped only
    # to the sub's docs/ subtree, read-only on the sub -- recovery is doc-based, never git-based.
    fetch_src = _read(".claude/skills/upstream-version-watch/scripts/fetch_sub_docs.sh")
    _require(re.search('\\bgit\\b', fetch_src, re.I) is None, 'the sole sub-to-main upward channel must never mention git -- recovery is doc-rsync only')
    _require(fetch_src.count('"$SUB_HOST:$SUB_WORK_DIR/docs/" "$DEST/"') == 2, "both dry-run and apply paths must rsync ONLY the sub's docs/ subtree back to main")

    # Persona + doc-contract grounding (real committed text, not a hardcoded restatement).
    docs_md = _read(".claude/rules/docs.md")
    _require('**상향 회수(서브→메인) = 문서기반 only**' in docs_md, 'predicate requirement failed at original line 1877')
    tmpl = _read(".claude/skills/terraforming_node/sub_node/CLAUDE.template.md")
    _require('코드/설정 patch 를 보내지 마라(회수는 문서기반 only)' in tmpl, 'predicate requirement failed at original line 1879')


# =============================================================================
# TERRAFORM_FLAG_GATE (4 clauses)
# =============================================================================

def predicate_TERRAFORM_FLAG_GATE_C1():
    """C1: a completion Flag attestation is written into the manifest only on genuine completion --
    topology CONFIRMED (never speculative/auto) AND branch-topology match VERIFIED, conservatively
    fail-closed, never issued speculatively. Proven in two real, independently-executed layers:

    Layer A (write-gate): `scan_node.evaluate_gate`/`emit_gate` are the actual pure functions that
    decide whether `emit_manifest_block` (the function that stamps complete:true/branch_verified:true
    into the manifest) is ever invoked at all -- structurally confirmed by reading the exact literal
    guard condition live from scan_node.py's main(). Each prerequisite (topology confirmation, the
    declared-vs-branch match, the declared-vs-on-disk-manifest match, and -- for multi -- interconnect
    presence, peer reachability, and measured bandwidth) is flipped ONE AT A TIME off a genuinely
    passing baseline, proving each is independently load-bearing (never jointly required to matter).

    Layer B (read-gate): `manifest_contract.evaluate_contract`, the function every downstream
    consumer validates an already-written attestation against, is exercised the same way -- complete,
    branch_verified, a wholly-absent terraforming block, missing HW fields, and an invalid
    model_source are each flipped alone off the same passing baseline."""
    scan_src = _read(".claude/skills/terraforming_node/scripts/scan_node.py")
    _require('if args.emit_manifest and result["gate"]["status"] == "ok" and args.topology in ("single", "multi"):' in scan_src, "the Flag-attestation emit call must be gated on gate.status=='ok' AND a CONFIRMED (non-auto) topology -- a regression loosening this condition would let emit_manifest_block stamp complete:true/branch_verified:true on a blocked or speculative gate")

    # --- Layer A: single-topology matrix (baseline + 3 independent flips) ---
    base_single = dict(declared="single", ic_present=False, peer_given=False, peer_reachable=None,
                        bandwidth=None, bw_floor=180.0, branch="single-node", branch_topo="single",
                        mani_topo="single")
    _, gate, exit_code = scan_node.evaluate_gate(**base_single)
    _require(gate['branch'] == 'alpha' and gate['status'] == 'ok' and (exit_code == 0), 'the genuinely-consistent single baseline must pass -- control for the flips below')

    # (1) topology NOT confirmed (speculative/auto) -- must never itself reach status=='ok', and
    # emit_gate must independently refuse --emit-manifest while topology stays auto.
    _, gate_auto, _ = scan_node.evaluate_gate(**{**base_single, "declared": "auto"})
    _require(gate_auto['branch'] == 'auto' and gate_auto['status'] == 'report-only', "an unconfirmed topology declaration must never produce an 'ok' gate")
    _require(scan_node.emit_gate(True, 'auto') != 0, 'emit_gate must fail-closed on --emit-manifest while topology is still speculative')
    _require(scan_node.emit_gate(True, 'single') == 0 and scan_node.emit_gate(True, 'multi') == 0, 'control: emit_gate must NOT block once topology is genuinely confirmed')

    # (2) branch-topology match broken ALONE (manifest already agrees with the branch; only the
    # declared topology disagrees with the branch).
    _, gate_branch_mismatch, exit_bm = scan_node.evaluate_gate(
        **{**base_single, "branch_topo": "multi", "mani_topo": "multi"})
    _require(gate_branch_mismatch['branch'] == 'alpha' and gate_branch_mismatch['status'] == 'blocked' and (exit_bm == 2), 'declared topology disagreeing with the git branch must block, alone')

    # (3) branch-topology match broken the OTHER way ALONE (declared agrees with branch; only the
    # on-disk manifest disagrees).
    _, gate_mani_mismatch, exit_mm = scan_node.evaluate_gate(
        **{**base_single, "branch_topo": "single", "mani_topo": "multi"})
    _require(gate_mani_mismatch['branch'] == 'alpha' and gate_mani_mismatch['status'] == 'blocked' and (exit_mm == 2), 'an on-disk manifest topology disagreeing with branch/declared must block, alone')

    # --- Layer A: multi-topology matrix (baseline + 4 independent flips) ---
    base_multi = dict(declared="multi", ic_present=True, peer_given=True, peer_reachable=True,
                       bandwidth=208.2, bw_floor=180.0, branch="multi-node", branch_topo="multi",
                       mani_topo="multi")
    _, gate_m, exit_m = scan_node.evaluate_gate(**base_multi)
    _require(gate_m['branch'] == 'multi-ready' and gate_m['status'] == 'ok' and (exit_m == 0), 'the genuinely-consistent multi baseline must pass -- control for the flips below')

    _, gate_no_ic, exit_no_ic = scan_node.evaluate_gate(**{**base_multi, "ic_present": False})
    _require(gate_no_ic['branch'] == 'gamma' and gate_no_ic['status'] == 'blocked' and (exit_no_ic == 2), 'missing interconnect alone must block multi')

    _, gate_unreach, exit_unreach = scan_node.evaluate_gate(
        **{**base_multi, "peer_reachable": False, "peer_ip": "x"})
    _require(gate_unreach['branch'] == 'gamma' and gate_unreach['status'] == 'blocked' and (exit_unreach == 2), 'an unreachable declared peer alone must block multi')

    _, gate_slow, exit_slow = scan_node.evaluate_gate(**{**base_multi, "bandwidth": 50.0})
    _require(gate_slow['branch'] == 'gamma' and gate_slow['status'] == 'blocked' and (exit_slow == 2), 'below-floor measured bandwidth alone must block multi')

    # (7) missing/pending measurement -- an untaken bandwidth reading must stay pending, never
    # silently treated as passing/complete.
    _, gate_pending, exit_pending = scan_node.evaluate_gate(**{**base_multi, "bandwidth": None})
    # 2026-09-03(B4 · plan_26090317 P1): 이 단언은 `exit_pending == 0` 이었다 — 술어 자신의 산문
    #   ("never silently treated as passing/complete")과 **정반대**다. 종료코드 0 은 온보딩 러너의
    #   `scan_node.py … || abort` 관용구에서 곧 "통과" 로 읽히므로, 성능 미측정이 조용히 통과했다.
    #   SKILL.md §1 머리의 "성능 미검증 멀티 진행 금지(fail-closed)" 와도 어긋났다.
    _require(gate_pending['branch'] == 'multi-ready-candidate' and gate_pending['status'] == 'pending-perf'
             and (gate_pending['status'] != 'ok') and (exit_pending == 2),
             "an untaken bandwidth measurement must stay pending-perf AND exit non-zero -- exit 0 is "
             "read as success by `scan_node.py || abort` runners")

    # --- Layer B: manifest_contract's read-gate, same one-flip-at-a-time discipline ---
    base = {"topology": "single", "gpus_per_node": 1, "model_source": "managed",
            "terraforming": {"complete": True, "branch_verified": True}}
    res_ok = manifest_contract.evaluate_contract(base, "single")
    _require(res_ok['flag'] is True and res_ok['exit_code'] == manifest_contract.EXIT_OK, 'the genuinely-complete baseline must pass -- control for the flips below')

    incomplete = {**base, "terraforming": {"complete": False, "branch_verified": True}}
    res_incomplete = manifest_contract.evaluate_contract(incomplete, "single")
    _require(res_incomplete['flag'] is False and res_incomplete['exit_code'] == manifest_contract.EXIT_NO_FLAG, 'predicate requirement failed at original line 1979')

    unverified = {**base, "terraforming": {"complete": True, "branch_verified": False}}
    res_unverified = manifest_contract.evaluate_contract(unverified, "single")
    _require(res_unverified['flag'] is False and res_unverified['exit_code'] == manifest_contract.EXIT_NO_FLAG, 'predicate requirement failed at original line 1983')

    # speculative/missing case: the terraforming block never even attempted (not merely false).
    missing_block = {k: v for k, v in base.items() if k != "terraforming"}
    res_missing_block = manifest_contract.evaluate_contract(missing_block, "single")
    _require(res_missing_block['flag'] is False and res_missing_block['exit_code'] == manifest_contract.EXIT_NO_FLAG, 'a wholly absent terraforming block must fail exactly like an explicit false -- never a silent pass by omission')

    missing_hw = {**base, "gpus_per_node": None}
    res_missing_hw = manifest_contract.evaluate_contract(missing_hw, "single")
    _require(res_missing_hw['flag'] is False and res_missing_hw['exit_code'] == manifest_contract.EXIT_MISSING_FIELD, 'predicate requirement failed at original line 1994')

    bad_source = {**base, "model_source": "nas-bogus"}
    res_bad_source = manifest_contract.evaluate_contract(bad_source, "single")
    _require(res_bad_source['flag'] is False and res_bad_source['exit_code'] == manifest_contract.EXIT_MISSING_FIELD, 'predicate requirement failed at original line 1998')


def predicate_TERRAFORM_FLAG_GATE_C2():
    """C2: Flag absence keeps all three runtime skills (upstream, recipe, adversarial) info/
    advisory/HF-lookup capable, but NONE of the three can produce an environment-specific
    deliverable, and no HW value is ever fabricated to paper over the gap. Bound to each skill's
    real entrypoint, not to a name check:

    - recipe (vllm-recipe-explorer): the real end-to-end CLI dispatch (`recipe.main()`, argv->parse->
      gate->dispatch -- not merely the inner helper) is executed for all three deliverable
      subcommands against a hermetic Flag-absent repo; each must SystemExit(4). The exact dispatch
      condition binding {estimate, generate, simulate} to the gate is read live from main()'s source.
    - adversarial (adversarial-benchmark): run_bench.sh's second gate layer literally shells out to
      `manifest_contract.py --require-flag`; that SAME production entrypoint (`manifest_contract.main`,
      never a re-implementation) is executed here against a hermetic Flag-absent repo and proven
      non-zero, then the literal invocation line is confirmed to exist verbatim in run_bench.sh.
    - upstream (upstream-version-watch): has no single build script (main-only, distributed); its
      documented backstop is the SAME manifest_contract.py --require-flag command, executed for real
      again here, PLUS its "no fabricated HW" boundary is the real deliverable-assembly function
      `render_dockerfile.build_context`, which reads manifest HW facts directly and raises -- never
      defaults/guesses -- when they are absent.
    - the informational/HF-lookup half of the surface (crosscheck_model_card's config/card precision
      cross-check) is executed successfully with NO manifest, Flag, or repo context at all, proving
      it never even touches the gate."""
    with tempfile.TemporaryDirectory() as tmp:
        orig_repo_root, orig_argv = recipe.REPO_ROOT, sys.argv
        orig_a2a_env = os.environ.pop("EASY_VLLM_A2A_DELEGATED", None)
        recipe.REPO_ROOT = tmp
        try:
            for cmd, extra in (
                ("estimate", ["--auto"]),
                ("generate", ["--recipe-id", "r1"]),
                ("simulate", ["--candidate", os.path.join(tmp, "nope.json")]),
            ):
                sys.argv = ["recipe.py", cmd, "--config", os.path.join(tmp, "config.yaml"), *extra]
                try:
                    recipe.main()
                    raise AssertionError(
                        f"recipe.py {cmd} must refuse without a Flag, never fabricate a deliverable")
                except SystemExit as e:
                    _require(e.code == 4, f'recipe.py {cmd} must die() with exit 4, got {e.code!r}')
        finally:
            recipe.REPO_ROOT = orig_repo_root
            sys.argv = orig_argv
            if orig_a2a_env is not None:
                os.environ["EASY_VLLM_A2A_DELEGATED"] = orig_a2a_env

        main_src = inspect.getsource(recipe.main)
        _require('in ("estimate", "generate", "simulate")' in main_src and '_require_terraform_flag(REPO_ROOT)' in main_src, 'the live dispatch source must gate exactly {estimate, generate, simulate} -- not a wider or narrower set -- through the real _require_terraform_flag call')

        # adversarial's second layer: the SAME production entrypoint run_bench.sh's bash shells out
        # to, executed here for real (never re-implemented).
        rc = manifest_contract.main(["--topology", "single", "--repo", tmp, "--require-flag"])
        _require(rc == manifest_contract.EXIT_NO_MANIFEST, 'manifest_contract.py --require-flag must be non-zero against a Flag-absent repo')

    run_bench_src = _read(".claude/skills/adversarial-benchmark/scripts/run_bench.sh")
    _require('python3 "$MC" --topology "$TOPO" --repo "$REPO" --require-flag' in run_bench_src, 'run_bench.sh must invoke the exact command just executed above, verbatim')
    _require('테라포밍-완수 Flag 미발급 — info-only' in run_bench_src, 'run_bench.sh must fall back to an info-only refusal, never a fabricated deliverable')
    # Execute the production script itself against hermetic fake curl/docker/vllm commands.  The
    # fake docker executes the real inner `bash -c` payload, while fake vllm records only argv it
    # actually receives.  This rejects source-text decoys (comments/heredocs/separate commands)
    # and contradictory duplicate options, not merely absence of a convenient literal substring.
    with tempfile.TemporaryDirectory() as bench_tmp:
        bench_root = Path(bench_tmp)
        script_dir = bench_root / ".claude/skills/adversarial-benchmark/scripts"
        script_dir.mkdir(parents=True)
        shutil.copy2(REPO_ROOT / ".claude/skills/adversarial-benchmark/scripts/run_bench.sh",
                     script_dir / "run_bench.sh")
        (bench_root / "output/single/envs").mkdir(parents=True)
        (bench_root / "output/single/configs").mkdir(parents=True)
        (bench_root / "output/single/envs/.env.fixture").write_text(
            "SERVING_PORT=18000\nSERVING_MODEL_NAME=fixture-model\n"
            "CONTAINER_NAME=fixture-container\nCONFIG_FILE=fixture\n", encoding="utf-8")
        (bench_root / "output/single/configs/fixture.yaml").write_text(
            "model: /fixture/model\n", encoding="utf-8")
        (bench_root / "output/single/manifest.yaml").write_text(
            "topology: single\n", encoding="utf-8")
        contract = bench_root / ".claude/skills/terraforming_node/scripts/manifest_contract.py"
        contract.parent.mkdir(parents=True)
        contract.write_text("# hermetic gate fixture; fake python3 returns success\n", encoding="utf-8")

        fake_bin = bench_root / "fake-bin"
        fake_bin.mkdir()
        capture = bench_root / "vllm-argv.json"
        route_capture = bench_root / "docker-route.json"
        fake_python = fake_bin / "python3"
        fake_python.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
        fake_curl = fake_bin / "curl"
        fake_curl.write_text("#!/bin/sh\nprintf 200\n", encoding="utf-8")
        fake_vllm = fake_bin / "vllm"
        fake_vllm.write_text(
            f"#!{sys.executable}\n"
            "import json, os, pathlib, sys\n"
            "args = sys.argv[1:]\n"
            "record = {'argv': args, 'docker_route': os.environ.get('VLLM_FAKE_DOCKER_ROUTE')}\n"
            "with pathlib.Path(os.environ['VLLM_ARGV_CAPTURE']).open('a') as fh:\n"
            "    fh.write(json.dumps(record) + '\\n')\n"
            "out = pathlib.Path(args[args.index('--result-dir') + 1]) / args[args.index('--result-filename') + 1]\n"
            "out.write_text('{}')\n", encoding="utf-8")
        fake_docker = fake_bin / "docker"
        fake_docker.write_text(
            f"#!{sys.executable}\n"
            "import json, os, pathlib, secrets, subprocess, sys\n"
            "args = sys.argv[1:]\n"
            "if args[0] == 'ps':\n"
            "    print('fixture-container-id')\n"
            "elif args[0] == 'logs':\n"
            "    pass\n"
            "elif args[0] == 'exec' and args[2:4] == ['bash', '-lc']:\n"
            "    nonce = secrets.token_hex(32)\n"
            "    route = {'nonce': nonce, 'container': args[1], 'shell': args[2:4]}\n"
            "    with pathlib.Path(os.environ['VLLM_DOCKER_ROUTE_CAPTURE']).open('a') as fh:\n"
            "        fh.write(json.dumps(route) + '\\n')\n"
            "    child_env = os.environ.copy()\n"
            "    child_env['VLLM_FAKE_DOCKER_ROUTE'] = nonce\n"
            "    subprocess.run(['/bin/bash', '-c', args[4]], check=True, env=child_env)\n"
            "elif args[0] == 'exec' and args[2] == 'cat':\n"
            "    sys.stdout.write(pathlib.Path(args[3]).read_text())\n"
            "else:\n"
            "    raise SystemExit('unexpected docker argv: ' + repr(args))\n", encoding="utf-8")
        for fake in (fake_python, fake_curl, fake_vllm, fake_docker):
            fake.chmod(0o755)

        env = os.environ.copy()
        env["PATH"] = f"{fake_bin}:{env['PATH']}"
        env["VLLM_ARGV_CAPTURE"] = str(capture)
        env["VLLM_DOCKER_ROUTE_CAPTURE"] = str(route_capture)
        completed = subprocess.run(
            ["bash", str(script_dir / "run_bench.sh"), "fixture", "--topology", "single",
             "--concurrency", "1", "--input-len", "16", "--output-len", "8",
             "--num-prompts", "1", "--warmups", "0"],
            cwd=bench_root, env=env, text=True, capture_output=True)
        _require(completed.returncode == 0, completed.stdout + completed.stderr)
        captures = [json.loads(line) for line in capture.read_text(encoding="utf-8").splitlines()]
        routes = [json.loads(line) for line in route_capture.read_text(encoding="utf-8").splitlines()]
        _require(len(routes) == 1 and routes[0]['container'] == 'fixture-container' and (routes[0]['shell'] == ['bash', '-lc']), routes)
        _require(len(captures) == 1 and captures[0]['docker_route'] == routes[0]['nonce'], f'exactly one vllm invocation must execute through docker exec bash -lc; routes={routes!r} captures={captures!r}')
        bench_argv = captures[0]["argv"]
        _require(bench_argv[:2] == ['bench', 'serve'], bench_argv)
        temp_positions = [i for i, arg in enumerate(bench_argv) if arg == "--temperature"]
        _require(len(temp_positions) == 1 and bench_argv[temp_positions[0] + 1] == '0', f'the actually executed vllm bench serve argv must contain exactly one greedy temperature pin; argv={bench_argv!r}')

    # upstream: same documented backstop entrypoint, executed for real in a fresh hermetic
    # Flag-absent repo (upstream has no single script gate of its own -- SKILL.md documents this
    # exact command as its canonical deterministic backstop; grounded live below).
    with tempfile.TemporaryDirectory() as tmp2:
        rc2 = manifest_contract.main(["--topology", "single", "--repo", tmp2, "--require-flag"])
        _require(rc2 == manifest_contract.EXIT_NO_MANIFEST, 'predicate requirement failed at original line 2157')
    upstream_skill = _read(".claude/skills/upstream-version-watch/SKILL.md")
    _require('manifest_contract.py --topology' in upstream_skill and '--require-flag' in upstream_skill and ('결정론 백스톱' in upstream_skill), "upstream's SKILL.md must document this exact executed command as its deterministic backstop")
    _require('렌더·빌드·bump ✗' in upstream_skill and 'vLLM GitHub 릴리즈 조회·버전해소 *설명* OK' in upstream_skill, 'upstream must be documented as blocking render/build/bump specifically, while release lookups and version-resolution explanation stay allowed')

    # NEW (plan_26082410): upstream render deliverable 은 이제 bake-in 게이트로 강제(recipe.py 패턴) —
    # 실행: Flag-absent manifest 는 exit 4, Flag-valid 는 통과. 구조: main() 이 render 경로 전 호출.
    with tempfile.TemporaryDirectory() as tmp3:
        absent_man = os.path.join(tmp3, "m.yaml")
        try:
            render_dockerfile._require_terraform_flag(absent_man)
            raise AssertionError("upstream render Flag-absent manifest must refuse, not proceed")
        except SystemExit as e:
            _require(e.code == 4, f'upstream render Flag-absent must exit 4, got {e.code!r}')
        with open(absent_man, "w", encoding="utf-8") as f:
            f.write("topology: single\ngpus_per_node: 1\nmodel_source: managed\n"
                    "terraforming:\n  complete: true\n  branch_verified: true\n")
        render_dockerfile._require_terraform_flag(absent_man)  # Flag-valid must not raise
    render_src = _read(".claude/skills/upstream-version-watch/scripts/render_dockerfile.py")
    _require('def _require_terraform_flag(' in render_src, 'upstream render must define the Flag gate function')
    _require('if a.canonical_kind or a.template:' in render_src and '_require_terraform_flag(a.manifest)' in render_src, 'the render deliverable paths (canonical-kind/template) must invoke the Flag gate before rendering')

    # upstream's "no fabricated HW" boundary: the real deliverable-assembly function must raise
    # (never default/guess) when a required manifest HW fact is missing.
    resolved_ok = {"vllm_version": "0.25.1", "torch": {"pin": "2.9.0"},
                   "ngc_base": {"tag": "26.05-py3", "cuda_version": "13.2"},
                   "build_track": {"decision": "wheel"}, "wheel": {"cuda": "132", "manylinux": "2_31"}}
    try:
        render_dockerfile.build_context({}, resolved_ok)
        raise AssertionError("build_context must raise, never fabricate a default cpu_arch")
    except KeyError:
        pass
    ctx_ok = render_dockerfile.build_context({"cpu_arch": "aarch64"}, resolved_ok)
    _require(ctx_ok['CPU_ARCH'] == 'aarch64', 'with a real manifest fact present, the real value is used')

    # info-only/advisory/HF-lookup capability survives Flag absence entirely.
    cprec = crosscheck_model_card.config_precision({"quantization_config": {"quant_method": "fp8"}})
    _require(cprec.startswith('fp8'), 'informational config-precision cross-check needs zero HW/Flag context')
    cardp = crosscheck_model_card.card_precision(
        "| DeepSeek-V4-Flash | FP4 + FP8 Mixed (experts FP4) |\n", "DeepSeek-V4-Flash")
    _require(cardp.get('mixed_note') or cardp.get('table_row'), 'predicate requirement failed at original line 2184')


def predicate_TERRAFORM_FLAG_GATE_C3():
    """C3: enforcement is two-layered -- BOTH deterministic script-level gates are executed for
    real (recipe.py's own end-to-end dispatch, and run_bench.sh's own manifest_contract.py
    --require-flag invocation, executed via the real `manifest_contract.main`, not a
    re-implementation) -- plus the persona-level redirect is proven to be the literal string the
    real --json CLI path emits at runtime (never a hand-typed restatement), tied to the constitution's
    own committed wording, so a generic verbal command has no alternate phrasing to fall back on."""
    import json

    with tempfile.TemporaryDirectory() as tmp:
        # Layer 1a: recipe.py's script-level gate via its real end-to-end dispatch (a regression
        # that removed only the gate CALL from main(), while leaving the helper itself intact,
        # would slip past a test that called the helper directly instead).
        orig_repo_root, orig_argv = recipe.REPO_ROOT, sys.argv
        orig_a2a_env = os.environ.pop("EASY_VLLM_A2A_DELEGATED", None)
        recipe.REPO_ROOT = tmp
        try:
            sys.argv = ["recipe.py", "estimate", "--config", os.path.join(tmp, "config.yaml"), "--auto"]
            try:
                recipe.main()
                raise AssertionError("recipe.py's script-level layer must refuse without a Flag")
            except SystemExit as e:
                _require(e.code == 4, 'predicate requirement failed at original line 2209')
        finally:
            recipe.REPO_ROOT = orig_repo_root
            sys.argv = orig_argv
            if orig_a2a_env is not None:
                os.environ["EASY_VLLM_A2A_DELEGATED"] = orig_a2a_env

        # Layer 1b: run_bench.sh's second-layer command, manifest_contract.py --require-flag,
        # executed for real (the actual production entrypoint) against the same hermetic
        # Flag-absent repo.
        rc = manifest_contract.main(["--topology", "single", "--repo", tmp, "--require-flag"])
        _require(rc == manifest_contract.EXIT_NO_MANIFEST, 'predicate requirement failed at original line 2220')

        # Layer 2: the persona-level redirect actually rendered at runtime by the real --json CLI
        # path (not a hardcoded restatement of the constant).
        orig_stdout = sys.stdout
        sys.stdout = captured = io.StringIO()
        try:
            manifest_contract.main(["--topology", "single", "--repo", tmp, "--json"])
        finally:
            sys.stdout = orig_stdout
        emitted = json.loads(captured.getvalue())
        _require(emitted['flag'] is False and emitted['redirect'] == manifest_contract.REDIRECT_TEMPLATE, 'the real --json CLI path must emit the exact production REDIRECT_TEMPLATE constant, not a re-typed lookalike string')

    run_bench_src = _read(".claude/skills/adversarial-benchmark/scripts/run_bench.sh")
    _require('python3 "$MC" --topology "$TOPO" --repo "$REPO" --require-flag' in run_bench_src, 'run_bench.sh must invoke the exact same manifest_contract.py --require-flag command executed as layer 1b above -- not a lookalike/rewritten invocation')

    # the fragment actually emitted at runtime above must be verbatim, in the constitution's own
    # committed wording -- so no alternate/looser phrasing exists for a generic verbal command to
    # substitute and slip past the persona-level redirect.
    workflow_md = _read(".claude/rules/workflow.md")
    shared_fragment = ("① HW스캔 + ② 모델 다운로드 전략(관리 NAS 경로? 컨테이너 임시 다운로드(컨테이너 down 시 삭제)? "
                        "특정 경로 저장·마운트?)을 먼저 정합시다.")
    _require(shared_fragment in workflow_md, 'predicate requirement failed at original line 2246')
    _require(shared_fragment in manifest_contract.REDIRECT_TEMPLATE, 'predicate requirement failed at original line 2247')
    _require(shared_fragment in emitted['redirect'], 'the fragment proven live in workflow.md must be the SAME text the live CLI run actually emitted above, not merely present in the static constant somewhere else')


def predicate_TERRAFORM_FLAG_GATE_C4():
    """C4: wiki-desk stays ACTUALLY callable end-to-end, identically, whether the completion Flag is
    absent or present -- not merely "no gate token appears in its source" (a differently-named or
    indirect check could dodge that). Proven by executing wiki-desk's real query entrypoint
    (`smoke_query.main()`, the tool an agent actually runs) against a hermetic registry, twice: once
    with a genuinely Flag-absent repo (independently confirmed via manifest_contract) and once with a
    genuinely Flag-valid one sitting right next to it (also independently confirmed) -- both runs
    must reach the identical PASS verdict and exit 0, proving wiki-desk never even reads that state.
    Source-level absence of the gate machinery is kept as a secondary corroborating signal only."""
    import json

    wiki_dir = REPO_ROOT / ".claude" / "skills" / "wiki-desk" / "scripts"
    py_files = sorted(wiki_dir.glob("*.py"))
    _require(py_files, 'predicate requirement failed at original line 2266')
    banned = ("manifest_contract", "_require_terraform_flag", "terraforming.complete", "a2a_delegation")
    for f in py_files:
        text = f.read_text(encoding="utf-8")
        for token in banned:
            _require(token not in text, f'{f.name} must never reference {token!r}')

    smoke_query = _import(".claude/skills/wiki-desk/scripts", "smoke_query")

    def _run_query(wiki_root):
        registry = {
            "entries": [{
                "source_id": "s1", "stem": "devlog_26072508",
                "source_path": "docs/devlog/devlog_26072508_x.md", "document_type": "devlog",
                "authority_rank": 3, "confidence": "high", "topical_summary": "flag gate probe",
                "title_hint": "flag gate probe", "lifecycle_state": "active", "topic_slug": "flag-gate",
            }],
            "edges": [], "retrieval": {"min_authority": 0, "min_score": 1},
        }
        (wiki_root / "sources").mkdir(parents=True)
        (wiki_root / "sources" / "registry.json").write_text(json.dumps(registry), encoding="utf-8")
        orig_argv, orig_stdout = sys.argv, sys.stdout
        sys.argv = ["smoke_query.py", "--wiki-root", str(wiki_root), "--query", "devlog_26072508"]
        sys.stdout = captured = io.StringIO()
        try:
            rc = smoke_query.main()
        finally:
            sys.argv, sys.stdout = orig_argv, orig_stdout
        return rc, captured.getvalue()

    with tempfile.TemporaryDirectory() as tmp:
        tmp_path = Path(tmp)

        # Flag-ABSENT control: no manifest anywhere near this repo root at all.
        absent_repo = tmp_path / "absent"
        absent_repo.mkdir()
        mc_absent = manifest_contract.main(["--topology", "single", "--repo", str(absent_repo), "--json"])
        _require(mc_absent == manifest_contract.EXIT_NO_MANIFEST, 'control: this repo state must genuinely be Flag-absent')
        rc_absent, out_absent = _run_query(absent_repo / "__llm-wiki")

        # Flag-PRESENT control: a real, passing manifest sits right next to the wiki root.
        present_repo = tmp_path / "present"
        (present_repo / "output" / "single").mkdir(parents=True)
        (present_repo / "output" / "single" / "manifest.yaml").write_text(
            "topology: single\ngpus_per_node: 1\nmodel_source: managed\nnodes: []\n"
            "terraforming:\n  complete: true\n  branch_verified: true\n", encoding="utf-8")
        mc_present = manifest_contract.main(
            ["--topology", "single", "--repo", str(present_repo), "--require-flag"])
        _require(mc_present == manifest_contract.EXIT_OK, 'control: this repo state must genuinely be Flag-valid')
        rc_present, out_present = _run_query(present_repo / "__llm-wiki")

    _require(rc_absent == 0 and rc_present == 0, f"wiki-desk's real query entrypoint must succeed identically regardless of Flag state, got exit {rc_absent} (Flag-absent) vs {rc_present} (Flag-present)")
    _require('PASS' in out_absent and 'PASS' in out_present, 'both runs must reach the same genuine PASS verdict, not merely a non-crashing exit code')


# =============================================================================
# A2A_DELEGATION_KEY_FAIL_CLOSED (4 clauses)
# =============================================================================

def predicate_A2A_DELEGATION_KEY_FAIL_CLOSED_C1():
    """C1: a sub node stays info-only until main issues a key only after asserting HW homogeneity --
    cpu_arch, gpu_model, gpus_per_node EACH exact-match, checked one dimension at a time (a flip in
    any single one of the three, alone, must block; the other two staying silent) and a missing
    (not merely differing) reading on any one of them must block too, never pass by omission.
    Then executes the real main-only key-issuance sequence end-to-end: scan_node's own live source
    is read to prove the operator-facing `hw_verified` suggestion is gated on `assert_homogeneity`'s
    real verdict, and render_sub_env's real `parse_manifest`/`build_placeholders`/`render_tree` (the
    production fixture `FIXTURE_MANIFEST`, not a hand-rolled stand-in) are run twice: once with no
    hw_verified stamp (key must never appear) and once with it genuinely present (key must appear,
    with the exact distinct role marker)."""
    base = {"cpu_arch": "aarch64", "gpu_model": "NVIDIA GB10", "gpus_per_node": 1,
            "cuda": "13.2", "driver": "565.57.01"}
    baseline = scan_node.assert_homogeneity(base, dict(base))
    _require(baseline['verified'] is True and baseline['blocks'] == [], 'a genuinely identical peer must verify clean -- control for the flips below')

    dims = {"cpu_arch": "x86_64", "gpu_model": "NVIDIA RTX 5090", "gpus_per_node": 2}
    for field, bad_value in dims.items():
        mismatched = {**base, field: bad_value}
        res = scan_node.assert_homogeneity(base, mismatched)
        _require(res['verified'] is False and any((field in b for b in res['blocks'])), f'{field} mismatch alone must block homogeneity')
        others = [f for f in dims if f != field]
        _require(not any((any((o in b for o in others)) for b in res['blocks'])), f'flipping {field} alone must not spuriously implicate {others}')

    for field in dims:
        missing = {**base, field: None}
        res = scan_node.assert_homogeneity(base, missing)
        _require(res['verified'] is False and any((field in b for b in res['blocks'])), f'{field} missing on the peer must block, never pass by omission')

    # ---- main-only key-issuance sequence: real gating source + real downstream execution ----
    scan_src = _read(".claude/skills/terraforming_node/scripts/scan_node.py")
    # 2026-09-03(⑬ · plan_26090317 P1): hw_verified 안내는 **별도의 "병합 지시" 블록**에 있었고 그
    #   블록이 `homogeneity.verified` 로 게이트돼 있었다. 같은 출력 안에 "이 블록으로 덮어써라" 와
    #   "sub 항목에 이걸 추가해라" 가 공존해 사람이 무엇을 반영해야 하는지 모호했으므로, emit 본문
    #   하나로 합쳤다. 게이트 자체는 그대로다 — 이제 **emit 산출물을 실행해** 확인한다(더 강하다).
    _require('hw_verified' in scan_src, 'emit must speak about hw_verified at all')
    _verified_block = scan_node.emit_manifest_block({
        "topology_declared": "multi", "cpu_arch": "aarch64", "cuda_version": "132",
        "gpus_per_node": 1, "gpu_model": "NVIDIA GB10",
        "nodes": [{"role": "main", "host": "a", "hostname": "a", "ssh_user": "u", "work_dir": "/w"},
                  {"role": "sub", "host": "b", "hostname": "b", "ssh_user": "u", "work_dir": "/w"}],
        "homogeneity": {"verified": True, "peer": {"gpu_model": "NVIDIA GB10", "driver": "1", "cuda": "13.2"}},
        "interconnect": {"type": "RoCE v2", "hca_devices": [], "gid_index": None, "socket_iface": None,
                         "bandwidth_gbps": None, "platform_preset": None}})
    _unverified_block = scan_node.emit_manifest_block({
        "topology_declared": "multi", "cpu_arch": "aarch64", "cuda_version": "132",
        "gpus_per_node": 1, "gpu_model": "NVIDIA GB10",
        "nodes": [{"role": "main", "host": "a", "hostname": "a", "ssh_user": "u", "work_dir": "/w"},
                  {"role": "sub", "host": "b", "hostname": "b", "ssh_user": "u", "work_dir": "/w"}],
        "interconnect": {"type": "RoCE v2", "hca_devices": [], "gid_index": None, "socket_iface": None,
                         "bandwidth_gbps": None, "platform_preset": None}})
    _require('hw_verified: true' in _verified_block,
             'a genuinely homogeneity-verified scan must stamp hw_verified: true')
    _require('hw_verified: true' not in _unverified_block and 'hw_verified: false' in _unverified_block,
             'without an assert_homogeneity verdict the emit must say hw_verified: false -- never true, '
             'and never silently omit the field (omission reads as "unknown" to a human)')

    with tempfile.TemporaryDirectory() as tmp:
        mpath = os.path.join(tmp, "manifest.yaml")
        with open(mpath, "w", encoding="utf-8") as f:
            f.write(render_sub_env.FIXTURE_MANIFEST)   # real production self-test fixture, not invented here

        data_noverify = render_sub_env.parse_manifest(mpath)
        ph_noverify, _ = render_sub_env.build_placeholders(data_noverify)
        _require(ph_noverify['SUB_HW_VERIFIED'] == '', "control: the fixture's sub node carries no hw_verified stamp -- nothing to issue from")
        out_noverify = os.path.join(tmp, "noverify")
        render_sub_env.render_tree(ph_noverify, out_noverify, copy_runtime_block=False)
        _require(not os.path.exists(os.path.join(out_noverify, '.claude', 'a2a_delegation.json')), 'without a genuine homogeneity-verified hw_verified stamp, the key must never be issued')

        homo = scan_node.assert_homogeneity(base, dict(base))   # main runs the real check FIRST
        _require(homo['verified'] is True, 'predicate requirement failed at original line 2384')
        data_verified = render_sub_env.parse_manifest(mpath)
        for n in data_verified["nodes"]:
            if n["role"] == "sub":
                n["hw_verified"] = "true"                        # ...only THEN does the stamp get set
        ph_verified, _ = render_sub_env.build_placeholders(data_verified)
        _require(ph_verified['SUB_HW_VERIFIED'] == 'true', 'predicate requirement failed at original line 2390')
        out_verified = os.path.join(tmp, "verified")
        render_sub_env.render_tree(ph_verified, out_verified, copy_runtime_block=False)
        keyp = os.path.join(out_verified, ".claude", "a2a_delegation.json")
        _require(os.path.isfile(keyp), 'predicate requirement failed at original line 2394')
        with open(keyp, encoding="utf-8") as f:
            kd = __import__("json").load(f)
        _require(kd['delegation'] == 'main_cluster_flag' and kd['issued_to'] == 'sub', 'predicate requirement failed at original line 2397')


def predicate_A2A_DELEGATION_KEY_FAIL_CLOSED_C2():
    """C2: absence, malformed content, AND wrong-role placement of the key are each -- alone --
    never treated as exemption (fail-open forbidden), proven by executing the real
    `assert_sub_delegation_authorized` bash gate against four manifests (absent / malformed literal
    / hw_verified stamped on the wrong role / genuine positive control). Then proves the sub
    structurally cannot self-scan or self-issue: `render_sub_env.RUNTIME_BLOCKS` (the exact, closed
    set of skills ever copied to a sub) is read live and must be exactly
    {vllm-recipe-explorer, adversarial-benchmark} -- never terraforming_node -- and sync_to_sub.sh's
    own `verify_checksums` (the function enumerating every file the overlay literally delivers) is
    read live and must carry the real delivered runtime file `recipe.py` and the key itself, while
    never carrying any terraforming_node path. The persona-level half of the same fact -- that
    terraforming_node is a main-only building block, never sub-delivered -- is grounded by reading
    the actual committed SKILL.md wording."""
    src = _sync_to_sub_src()
    fn = _extract_bash_function(src, "assert_sub_delegation_authorized")
    resolver = _extract_bash_function(src, "_resolve_sub_hw_verified")

    def _gate(manifest_body):
        with tempfile.TemporaryDirectory() as tmp:
            srcdir = Path(tmp) / "output" / "multi"
            srcdir.mkdir(parents=True)
            (srcdir / "manifest.yaml").write_text(manifest_body)
            script = f'SRC="{tmp}/"\n{resolver}\n{fn}\nassert_sub_delegation_authorized multi\necho "RC=$?"\n'
            return _run_bash(script)

    # (a) absence -- no hw_verified line anywhere.
    proc_absent = _gate("topology: multi\nnodes:\n  - role: main\n  - role: sub\n    host: 1.2.3.4\n")
    _require('RC=1' in proc_absent.stdout and 'STOP' in proc_absent.stderr, 'absence of hw_verified must refuse, not exempt')

    # (b) malformed -- present but not the exact literal 'true'.
    for bad in ("false", "True", "1", "yes"):
        proc_bad = _gate(f"topology: multi\nnodes:\n  - role: main\n  - role: sub\n    hw_verified: {bad}\n")
        _require('RC=1' in proc_bad.stdout, f'hw_verified: {bad!r} must refuse, not exempt (malformed)')

    # (c) wrong-role -- hw_verified:true stamped on role:main must never satisfy the sub gate.
    proc_wrongrole = _gate(
        "topology: multi\nnodes:\n  - role: main\n    hw_verified: true\n  - role: sub\n    host: 1.2.3.4\n")
    _require('RC=1' in proc_wrongrole.stdout, 'hw_verified on role:main must not exempt role:sub')

    # control: the genuine positive case must pass.
    proc_ok = _gate("topology: multi\nnodes:\n  - role: main\n  - role: sub\n    hw_verified: true\n")
    _require('RC=0' in proc_ok.stdout, 'hw_verified:true on role:sub, alone, must be admitted')

    # ---- sub cannot self-scan/self-issue: terraforming_node is structurally never delivered ----
    delivered_names = {os.path.basename(rb) for rb in render_sub_env.RUNTIME_BLOCKS}
    _require(delivered_names == {'vllm-recipe-explorer', 'adversarial-benchmark'}, f"the sub's runtime-block copy set must be exactly this closed pair, got {delivered_names} -- terraforming_node must never join it")

    checksum_fn = _extract_bash_function(src, "verify_checksums")
    _require('.claude/skills/vllm-recipe-explorer/recipe.py' in checksum_fn, "control: the real delivered runtime file must appear in the overlay's own checksum list")
    _require('.claude/a2a_delegation.json' in checksum_fn, 'control: the key itself is delivered -- proving the list was read/extracted correctly')
    _require('.claude/skills/terraforming_node' not in checksum_fn, 'the overlay-delivery enumeration itself must never name a terraforming_node path')

    skill_md = _read(".claude/skills/terraforming_node/SKILL.md")
    _require('**빌딩블럭**(메인 전용, 서브 전달 ✗): `terraforming_node`·`upstream-version-watch`.' in skill_md, 'the committed persona-level fact -- terraforming_node is main-only, never sub-delivered -- must still say so verbatim')


def predicate_A2A_DELEGATION_KEY_FAIL_CLOSED_C3():
    """C3: enforcement is fail-closed at BOTH points named by the clause, each proven with a
    positive/absent/malformed/wrong-role matrix (never a single positive path):
    (i) the multi-node sync_to_sub propagation gate (positive control here; the negative triad is
    proven exhaustively in C2's own matrix over the same real bash gate);
    (ii) the single-node sub-control serve gate, bound to BOTH of its real, independent
    implementations -- recipe.py's `_require_terraform_flag` (executed directly) and
    run_bench.sh's own inline gate block (extracted verbatim -- never retyped -- and executed for
    real via bash against a real, hermetic `manifest_contract.py` copy), each run through: absent
    (both credentials missing), malformed (unparseable/garbage key), wrong-role (valid JSON, wrong
    marker), and positive (via the key alone, and separately via the manifest/Flag alone)."""
    import json

    src = _sync_to_sub_src()
    fn = _extract_bash_function(src, "assert_sub_delegation_authorized")
    resolver = _extract_bash_function(src, "_resolve_sub_hw_verified")

    def _sync_gate(manifest_body):
        with tempfile.TemporaryDirectory() as tmp:
            srcdir = Path(tmp) / "output" / "multi"
            srcdir.mkdir(parents=True)
            (srcdir / "manifest.yaml").write_text(manifest_body)
            script = f'SRC="{tmp}/"\n{resolver}\n{fn}\nassert_sub_delegation_authorized multi\necho "RC=$?"\n'
            return _run_bash(script)

    # (i) multi-node propagation gate -- positive control (C2 owns the absent/malformed/wrong-role triad).
    proc_pos = _sync_gate("topology: multi\nnodes:\n  - role: main\n  - role: sub\n    hw_verified: true\n")
    _require('RC=0' in proc_pos.stdout, 'hw_verified:true must be admitted by the propagation gate')

    # (ii-a) single-node sub-control serve gate, implementation 1: recipe.py's _require_terraform_flag.
    def _recipe_gate(tmp_dir, key_content):
        claude_dir = Path(tmp_dir) / ".claude"
        claude_dir.mkdir(exist_ok=True)
        if key_content is not None:
            (claude_dir / "a2a_delegation.json").write_text(key_content)
        recipe._require_terraform_flag(tmp_dir)

    orig_a2a_env = os.environ.pop("EASY_VLLM_A2A_DELEGATED", None)
    try:
        with tempfile.TemporaryDirectory() as tmp_pos:
            _recipe_gate(tmp_pos, json.dumps({"delegation": "main_cluster_flag", "issued_to": "sub"}))  # must NOT raise

        with tempfile.TemporaryDirectory() as tmp_absent:
            try:
                _recipe_gate(tmp_absent, None)
                raise AssertionError("absence of both key and manifest must refuse, not exempt")
            except SystemExit as e:
                _require(e.code == 4, 'predicate requirement failed at original line 2512')

        with tempfile.TemporaryDirectory() as tmp_mal:
            try:
                _recipe_gate(tmp_mal, "{not valid json")
                raise AssertionError("a malformed key file must refuse, not exempt")
            except SystemExit as e:
                _require(e.code == 4, 'predicate requirement failed at original line 2519')

        for bad_role in ({"delegation": "main_cluster_flag", "issued_to": "main"},
                          {"delegation": "main_cluster_flag", "issued_to": "nobody"},
                          {"delegation": "bogus", "issued_to": "sub"}):
            with tempfile.TemporaryDirectory() as tmp_wr:
                try:
                    _recipe_gate(tmp_wr, json.dumps(bad_role))
                    raise AssertionError(f"wrong-role key {bad_role} must refuse, not exempt")
                except SystemExit as e:
                    _require(e.code == 4, 'predicate requirement failed at original line 2529')
    finally:
        if orig_a2a_env is not None:
            os.environ["EASY_VLLM_A2A_DELEGATED"] = orig_a2a_env

    # (ii-b) single-node sub-control serve gate, implementation 2: run_bench.sh's OWN inline gate,
    # extracted verbatim (never retyped) and executed for real.
    rb_src = _read(".claude/skills/adversarial-benchmark/scripts/run_bench.sh")
    start = rb_src.index('MC="$REPO/.claude/skills/terraforming_node/scripts/manifest_contract.py"')
    end = rb_src.index('PORT="${SERVING_PORT:?SERVING_PORT')
    gate_block = rb_src[start:end]
    _require('KEY_OK' in gate_block and 'exit 4' in gate_block and ('exit 2' in gate_block), 'predicate requirement failed at original line 2540')

    mc_src = _read(".claude/skills/terraforming_node/scripts/manifest_contract.py")

    def _run_bench_gate(repo, key_content=None, with_mc=False, manifest_body=None):
        repo_p = Path(repo)
        if key_content is not None:
            (repo_p / ".claude").mkdir(parents=True, exist_ok=True)
            (repo_p / ".claude" / "a2a_delegation.json").write_text(key_content)
        if with_mc:
            mcdir = repo_p / ".claude" / "skills" / "terraforming_node" / "scripts"
            mcdir.mkdir(parents=True, exist_ok=True)
            (mcdir / "manifest_contract.py").write_text(mc_src)
        if manifest_body is not None:
            outdir = repo_p / "output" / "single"
            outdir.mkdir(parents=True, exist_ok=True)
            (outdir / "manifest.yaml").write_text(manifest_body)
        script = f'REPO="{repo}"\nTOPO="single"\nCONFIG="test"\n{gate_block}\n'
        env = dict(os.environ)
        env.pop("EASY_VLLM_A2A_DELEGATED", None)
        return subprocess.run(["bash", "-c", script], capture_output=True, text=True, timeout=30, env=env)

    complete_manifest = ("topology: single\ngpus_per_node: 1\nmodel_source: managed\nnodes: []\n"
                         "terraforming:\n  complete: true\n  branch_verified: true\n")
    incomplete_manifest = ("topology: single\ngpus_per_node: 1\nmodel_source: managed\nnodes: []\n"
                           "terraforming:\n  complete: false\n  branch_verified: true\n")

    with tempfile.TemporaryDirectory() as tmp:
        proc = _run_bench_gate(tmp)   # absent: no key, no MC script at all
        _require(proc.returncode == 4 and '모두 부재' in proc.stderr, (proc.returncode, proc.stderr))

    with tempfile.TemporaryDirectory() as tmp:
        proc = _run_bench_gate(tmp, key_content=json.dumps({"delegation": "main_cluster_flag", "issued_to": "sub"}))
        _require(proc.returncode == 2, f'a genuine key must let the gate through, got {proc.returncode}: {proc.stderr}')

    with tempfile.TemporaryDirectory() as tmp:
        proc = _run_bench_gate(tmp, with_mc=True, manifest_body=complete_manifest)
        _require(proc.returncode == 2, f'a genuine Flag-complete manifest must let the gate through, got {proc.returncode}: {proc.stderr}')

    with tempfile.TemporaryDirectory() as tmp:
        proc = _run_bench_gate(tmp, key_content="{not valid json", with_mc=True, manifest_body=incomplete_manifest)
        _require(proc.returncode == 4 and '테라포밍-완수 Flag 미발급' in proc.stderr, 'a malformed key must not bypass -- the manifest/MC path must still be genuinely consulted')

    with tempfile.TemporaryDirectory() as tmp:
        proc = _run_bench_gate(
            tmp, key_content=json.dumps({"delegation": "main_cluster_flag", "issued_to": "main"}),
            with_mc=True, manifest_body=incomplete_manifest)
        _require(proc.returncode == 4 and '테라포밍-완수 Flag 미발급' in proc.stderr, 'a wrong-role key must not bypass either')


def predicate_A2A_DELEGATION_KEY_FAIL_CLOSED_C4():
    """C4: the main completion Flag and the sub delegation key are deliberately distinct, UNIQUE
    credentials that never cross-substitute, proven in three directions (no forbidden `or True`
    escape hatch anywhere in this proof):
    (1) a genuinely-true main Flag sitting in the SAME manifest does not, alone, satisfy the
        propagation gate that specifically requires the sub key (`nodes[sub].hw_verified`);
    (2) a genuinely-valid sub key sitting right next to a Flag-absent manifest does not cause the
        main Flag's own deterministic reader (`manifest_contract`, which never even references the
        key file) to emit/report a completion Flag;
    (3) wrong credential shapes fail across the plane boundary in both directions -- a main-Flag-
        shaped payload offered as the sub key is rejected by `_require_terraform_flag`, and a
        sub-key-shaped payload substituted into the manifest's own `terraforming` block is rejected
        by `evaluate_contract`."""
    import json

    render_src = _read(".claude/skills/terraforming_node/scripts/render_sub_env.py")
    idx = render_src.index('"delegation": "main_cluster_flag"')
    block = render_src[idx: idx + 400]
    _require('"issued_to": "sub"' in block, 'predicate requirement failed at original line 2612')
    _require('terraforming.complete' not in block, 'predicate requirement failed at original line 2613')

    require_src = inspect.getsource(recipe._require_terraform_flag)
    _require('kd.get("delegation") == "main_cluster_flag" and kd.get("issued_to") == "sub"' in require_src, 'predicate requirement failed at original line 2616')
    _require('terra.get("complete")' in require_src, 'predicate requirement failed at original line 2617')
    _require('UNIQUE' in inspect.getdoc(recipe._require_terraform_flag), "the gate's own documented contract must name the two credentials UNIQUE -- not merely happen to keep them structurally separate")

    # (1) main Flag alone cannot stand in for the sub key where the key is specifically required.
    src = _sync_to_sub_src()
    fn = _extract_bash_function(src, "assert_sub_delegation_authorized")
    resolver = _extract_bash_function(src, "_resolve_sub_hw_verified")
    with tempfile.TemporaryDirectory() as tmp:
        srcdir = Path(tmp) / "output" / "multi"
        srcdir.mkdir(parents=True)
        (srcdir / "manifest.yaml").write_text(
            "topology: multi\nterraforming:\n  complete: true\n  branch_verified: true\n"
            "nodes:\n  - role: main\n  - role: sub\n    host: 1.2.3.4\n")
        script = f'SRC="{tmp}/"\n{resolver}\n{fn}\nassert_sub_delegation_authorized multi\necho "RC=$?"\n'
        proc = _run_bash(script)
        _require('RC=1' in proc.stdout, 'a genuinely-true main completion Flag in the same manifest must NOT stand in for the missing sub delegation key -- the two credentials are checked independently')

    # (2) sub key cannot become/emit the main completion Flag.
    mc_src = _read(".claude/skills/terraforming_node/scripts/manifest_contract.py")
    _require('a2a' not in mc_src.lower() and 'delegation' not in mc_src.lower(), "the main Flag's own deterministic reader must never reference the sub key file -- structurally incapable of treating it as a Flag source")
    with tempfile.TemporaryDirectory() as tmp2:
        claude_dir = Path(tmp2) / ".claude"
        claude_dir.mkdir()
        (claude_dir / "a2a_delegation.json").write_text(
            json.dumps({"delegation": "main_cluster_flag", "issued_to": "sub", "topology": "multi"}))
        rc = manifest_contract.main(["--topology", "single", "--repo", tmp2, "--require-flag"])
        _require(rc == manifest_contract.EXIT_NO_MANIFEST, 'a genuinely valid sub delegation key sitting next to a Flag-absent manifest must NOT cause the main Flag reader to emit/report a completion Flag')

    # (3) wrong credential shapes fail across the plane boundary, both directions.
    orig_repo_root = recipe.REPO_ROOT
    orig_a2a_env = os.environ.pop("EASY_VLLM_A2A_DELEGATED", None)
    try:
        with tempfile.TemporaryDirectory() as tmp3:
            recipe.REPO_ROOT = tmp3
            claude_dir3 = Path(tmp3) / ".claude"
            claude_dir3.mkdir()
            (claude_dir3 / "a2a_delegation.json").write_text(json.dumps({"complete": True, "branch_verified": True}))
            try:
                recipe._require_terraform_flag(tmp3)
                raise AssertionError("a main-Flag-shaped payload in the sub key file must not exempt")
            except SystemExit as e:
                _require(e.code == 4, 'predicate requirement failed at original line 2666')
    finally:
        recipe.REPO_ROOT = orig_repo_root
        if orig_a2a_env is not None:
            os.environ["EASY_VLLM_A2A_DELEGATED"] = orig_a2a_env

    sub_shaped = {"topology": "single", "gpus_per_node": 1, "model_source": "managed",
                  "terraforming": {"delegation": "main_cluster_flag", "issued_to": "sub"}}
    res = manifest_contract.evaluate_contract(sub_shaped, "single")
    _require(res['flag'] is False and res['exit_code'] == manifest_contract.EXIT_NO_FLAG, "a sub-key-shaped payload substituted into the manifest's own terraforming block must not be accepted as the main completion Flag")


# =============================================================================
# ARCH_WALL_VARIANT_LADDER (5 clauses)
# =============================================================================

def _arch_contract_repo(tmp: str) -> Path:
    """A CLEAN-INDEX EXPORT of just the arch-contract inputs -- intentionally no `.git` (same
    contract as `_hint_tag` above; do not `git init` here).  `resolve_evidence_path` is git-native
    since G2-a, so every `_arch_codes` call runs inside `_arch_fixture_git`, which stubs git's
    index oracle for this root only.  The two digest ledgers this list used to copy
    (evidence_manifest.json / tracked_index.json) were deleted in G2-b -- Git is the authority."""
    root = Path(tmp)
    for rel in (".claude/policies/arch_variant_ledger.json",
                ".claude/skills/upstream-version-watch/templates/Dockerfile.source-build.template",
                ".claude/policies/arch_variant_evidence/source-sm12x-vllm-0.23.0-approval.json",
                ".claude/policies/provenance/plan_26062818_RouteB_jasl-fork_SM12x_DeepSeek-V4-Flash_2노드서빙.md",
                # schema v2: build_patch_selectors 검증이 선택자의 **클러스터-와이드 배선**을 여기서
                #   교차확인한다(원장 선언 ↔ 슬레이브 전달). 정적 파일끼리는 단일 소유가 불가능하므로
                #   교차검증이 차선이다(workflow.md §결정론 규율).
                ".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh",
                ".claude/rules/workflow.md", ".claude/skills/upstream-version-watch/SKILL.md"):
        dst = root / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO_ROOT / rel, dst)
    return root


def _write_arch_regression_artifact(root: Path, entry: dict) -> None:
    rel = ".claude/policies/arch_variant_evidence/test-regression.json"
    artifact = {
        "schema_version": 1, "kind": "arch_variant_regression", "result": "PASS",
        "variant_id": entry["variant_id"], "vllm_repo": entry["vllm_repo"],
        "vllm_ref": entry["vllm_ref"], "track": entry["track"],
        "architecture": entry["architecture"], "image_tag": entry["image_tag"],
        "existing_models": ["fixture-existing-model"],
    }
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(artifact, sort_keys=True), encoding="utf-8")
    # 포인터는 경로 하나뿐이다(G2-b, plan_26090222 F-1g). 예전에는 여기서 sha256 을 계산해
    # evidence_manifest·tracked_index 두 원장에 되박아야 픽스처가 자기정합을 유지했는데, 그
    # 재결속 자체가 걷어낸 중복층이었다 — 지금은 Git(픽스처에서는 _arch_fixture_git 스텁)이
    # 무결성 권위이므로 파일을 쓰는 것으로 끝난다.
    entry["regression_evidence"] = {"path": rel}


def _mutate_arch_artifact(root: Path, entry: dict, field: str, mutate) -> None:
    """Rewrite the pointed-to artifact in place.  No digest re-binding: the pointer carries only
    a path now, and the fixture's simulated index (`_arch_fixture_git`) is *defined* as the bytes
    on disk -- which is exactly the self-consistency the deleted two-ledger re-stamp bought."""
    pointer = entry[field]
    path = root / pointer["path"]
    artifact = json.loads(path.read_text())
    mutate(artifact)
    path.write_text(json.dumps(artifact, sort_keys=True), encoding="utf-8")


def _fixture_blob_sha1(path: Path) -> str:
    payload = path.read_bytes()
    return hashlib.sha1(
        b"blob " + str(len(payload)).encode("ascii") + b"\0" + payload).hexdigest()


@contextlib.contextmanager
def _arch_fixture_git(root: Path):
    """`_arch_contract_repo` builds a CLEAN-INDEX EXPORT -- intentionally no `.git`, same contract
    as `_hint_tag`'s `scoped_run` above.  Since G2-a `resolve_evidence_path` is git-native, so an
    un-stubbed fixture would answer `git_unavailable` for every pointer and every arch predicate
    would drown in ARCH_VARIANT_ARTIFACT_UNBOUND instead of exercising the ladder.

    We therefore stub ONLY git's index oracle, and only for this fixture root: the resolution
    ladder itself (absolute/`~`, path escape, symlink component, missing file) still runs for
    real, and the simulated index is "the bytes currently on disk", so a mutation-then-check
    fixture stays self-consistent without re-stamping any ledger.  Any other repo_root -- above
    all the real one -- passes through to the real git untouched, so this never weakens a
    production verdict."""
    original_available = policy_registry._git_available
    original_run = subprocess.run

    def scoped_available(repo_root):
        return True if Path(repo_root) == root else original_available(repo_root)

    def scoped_run(args, *a, **kw):
        argv = list(args)
        if argv[:1] == ["git"] and kw.get("cwd") is not None and Path(kw["cwd"]) == root:
            if argv[1:] == ["rev-parse", "--is-inside-work-tree"]:
                return subprocess.CompletedProcess(argv, 0, stdout="true\n", stderr="")
            if argv[1:3] == ["ls-files", "--stage"] and argv[3:4] == ["--"]:
                target = root / argv[4]
                if not target.is_file():
                    return subprocess.CompletedProcess(argv, 0, stdout="", stderr="")
                return subprocess.CompletedProcess(
                    argv, 0, stdout=f"100644 {_fixture_blob_sha1(target)} 0\t{argv[4]}\n", stderr="")
            if argv[1:3] == ["hash-object", "--"]:
                target = root / argv[3]
                if not target.is_file():
                    return subprocess.CompletedProcess(argv, 128, stdout="", stderr="no such path\n")
                return subprocess.CompletedProcess(
                    argv, 0, stdout=_fixture_blob_sha1(target) + "\n", stderr="")
        return original_run(args, *a, **kw)

    policy_registry._git_available = scoped_available
    subprocess.run = scoped_run
    try:
        yield
    finally:
        subprocess.run = original_run
        policy_registry._git_available = original_available


def _arch_codes(root: Path) -> set[str]:
    with _arch_fixture_git(root):
        return {v.reason_code for v in policy_registry.arch_variant_contract_violations(root)}

def predicate_ARCH_WALL_VARIANT_LADDER_C1():
    """C1: execute the classifier controls and the production no-skip ladder validator."""
    patterns = classify_failure.load_patterns(
        str(REPO_ROOT / ".claude/skills/upstream-version-watch/failure_patterns.yaml"))
    def classify(text):
        return next((p["class"] for p in patterns if re.search(p["signature"], text)), "unknown")
    _require(classify("ModuleNotFoundError: No module named 'uvloop'") == 'requirements-fixable', 'predicate requirement failed at original line 2753')
    _require(classify("No module named 'torch._opaque_base'") == 'source-build-class', 'predicate requirement failed at original line 2754')
    with tempfile.TemporaryDirectory() as tmp:
        root = _arch_contract_repo(tmp)
        _require(_arch_codes(root) == set(), 'predicate requirement failed at original line 2757')
        workflow = root / ".claude/rules/workflow.md"
        workflow.write_text(workflow.read_text().replace(
            "deps-패치 → 소스-게이트 패치 → **자체 이식** → "
            "**소스-repo 오버라이드(포크 핀)** → 체크포인트-교체",
            # 반증실험: 한 칸(자체 이식)을 빼면 위반이 잡혀야 한다.
            "deps-패치 → 소스-게이트 패치 → **소스-repo 오버라이드(포크 핀)** → 체크포인트-교체"))
        _require('ARCH_VARIANT_LADDER_ORDER_MISSING' in _arch_codes(root), 'predicate requirement failed at original line 2762')
        shutil.copy2(REPO_ROOT / ".claude/rules/workflow.md", workflow)
        dockerfile = root / ".claude/skills/upstream-version-watch/templates/Dockerfile.source-build.template"
        dockerfile.write_text(dockerfile.read_text().replace("checkout --detach ${VLLM_REF}", "checkout main"))
        _require('ARCH_VARIANT_SOURCE_OVERRIDE_UNWIRED' in _arch_codes(root), 'predicate requirement failed at original line 2766')


def predicate_ARCH_WALL_VARIANT_LADDER_C2():
    """C2: real renderer ignores model identity; production ledger validator rejects model-keyed/non-superset tags."""
    manifest = {"cpu_arch": "aarch64", "nas_model_path": ""}
    resolved = {"vllm_version": "0.25.1", "torch": {"pin": "2.11.0"},
                "ngc_base": {"tag": "26.05-py3", "cuda_version": "13.2"},
                "build_track": {"decision": "source-build"}, "wheel": {}}
    ctx = render_dockerfile.build_context(manifest, resolved)
    _require(ctx['IMAGE_TAG'] == '0.25.1-cu132-aarch64-source', 'predicate requirement failed at original line 2776')
    manifest["model_name"] = "deepseek-v4-flash"
    _require(render_dockerfile.build_context(manifest, resolved)['IMAGE_TAG'] == ctx['IMAGE_TAG'], 'predicate requirement failed at original line 2778')
    with tempfile.TemporaryDirectory() as tmp:
        root = _arch_contract_repo(tmp)
        doc = json.loads((root / ".claude/policies/arch_variant_ledger.json").read_text())
        entry = doc["source_build_variants"]["deepseek-v4-flash-sm12x"]
        entry["image_tag"] = "easy-vllm:deepseek-v4-flash-source-sm12x"
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_MODEL_KEYED_IMAGE' in _arch_codes(root), 'predicate requirement failed at original line 2785')
        doc["source_build_variants"]["track-sm12x"] = doc["source_build_variants"].pop(
            "deepseek-v4-flash-sm12x")
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_MODEL_KEYED_IMAGE' in _arch_codes(root), 'renaming the attacker-controlled ledger key must not hide a model-keyed image')
        entry = doc["source_build_variants"]["track-sm12x"]
        entry["image_tag"] = "easy-vllm:0.23.0-cu132-aarch64-source-wrong"
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_NOT_SUPERSET_TAG' in _arch_codes(root), 'predicate requirement failed at original line 2794')


def predicate_ARCH_WALL_VARIANT_LADDER_C3():
    """C3: production validator accepts zero/one active track and rejects two active tracks."""
    with tempfile.TemporaryDirectory() as tmp:
        root = _arch_contract_repo(tmp)
        doc = json.loads((root / ".claude/policies/arch_variant_ledger.json").read_text())
        base = doc["source_build_variants"]["deepseek-v4-flash-sm12x"]
        base["status"] = "VALIDATED"
        _write_arch_regression_artifact(root, base)
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_MULTIPLE_ACTIVE_TRACKS' not in _arch_codes(root), 'predicate requirement failed at original line 2806')
        other = dict(base, image_tag="easy-vllm:0.23.0-cu132-aarch64-source-sm121a", track="source-sm121a")
        doc["source_build_variants"]["other-model-sm121a"] = other
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_MULTIPLE_ACTIVE_TRACKS' in _arch_codes(root), 'predicate requirement failed at original line 2810')

    with tempfile.TemporaryDirectory() as tmp:
        root = _arch_contract_repo(tmp)
        doc = json.loads((root / ".claude/policies/arch_variant_ledger.json").read_text())
        hidden = doc["source_build_variants"].pop("deepseek-v4-flash-sm12x")
        hidden["status"] = "VALIDATED"
        hidden.pop("regression_evidence", None)
        hidden["image_tag"] = "easy-vllm:deepseek-v4-flash-source-sm12x"
        doc["source_build_variants"]["_hidden_active_variant"] = hidden
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_METADATA_KEY_INVALID' in _arch_codes(root), 'predicate requirement failed at original line 2821')

    with tempfile.TemporaryDirectory() as tmp:
        root = _arch_contract_repo(tmp)
        doc = json.loads((root / ".claude/policies/arch_variant_ledger.json").read_text())
        entry = doc["source_build_variants"]["deepseek-v4-flash-sm12x"]
        entry["status"] = "UNVALIDATED"
        _write_arch_regression_artifact(root, entry)
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_ACTIVE_NOT_VALIDATED' in _arch_codes(root), 'predicate requirement failed at original line 2830')

    with tempfile.TemporaryDirectory() as tmp:
        root = _arch_contract_repo(tmp)
        doc = json.loads((root / ".claude/policies/arch_variant_ledger.json").read_text())
        entry = doc["source_build_variants"]["deepseek-v4-flash-sm12x"]
        # v4(2026-09-03): 퇴역 판정이 `SUPERSEDED@<major.minor.patch>` **토큰 문법**으로 바뀌었다.
        #   폐기된 것은 "메타키 버전 + image_tag 버전 + 문장 전문" 3중 재구성 완전일치이고, 남은
        #   불변식은 하나다 -- 맨 마커로 활성 후보를 숨길 수 없다. 그래서 음성대조를 문법 위반
        #   4종으로 건다(버전 없음 · 접미사 오염 · 2-컴포넌트 절단 · 4-컴포넌트 과잉).
        #   STATUS_INVALID 는 단독 발화가 아니라 ACTIVE_NOT_VALIDATED 와 **동반 발화**가 정상이다 --
        #   문법을 못 갖춘 선언은 퇴역으로 인정되지 않고 활성 트랙 게이트로 떨어지기 때문이다.
        for bad_status in ("SUPERSEDED_PENDING", "SUPERSEDED", "SUPERSEDED@0.24", "SUPERSEDED@0.24.0.1"):
            entry["status"] = bad_status
            entry.pop("regression_evidence", None)
            (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
            codes = _arch_codes(root)
            _require('ARCH_VARIANT_STATUS_INVALID' in codes,
                     f'{bad_status!r} must be refused as a malformed retirement marker')
            _require('ARCH_VARIANT_ACTIVE_NOT_VALIDATED' in codes,
                     f'{bad_status!r} must fall through to the active-track gate, never be silently retired')
        # 양성 대조: 문법을 갖춘 선언은 뒤따르는 산문이 무엇이든 통과한다(산문은 더 이상 게이트가 아니다).
        entry["status"] = "SUPERSEDED@0.24.0; whatever prose the maintainer chooses to write here"
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        codes = _arch_codes(root)
        _require('ARCH_VARIANT_STATUS_INVALID' not in codes,
                 'a well-formed retirement marker must be admitted regardless of its trailing prose')
        _require('ARCH_VARIANT_ACTIVE_NOT_VALIDATED' not in codes,
                 'a retired variant must not be evaluated as an active track')

    with tempfile.TemporaryDirectory() as tmp:
        root = _arch_contract_repo(tmp)
        ledger_path = root / ".claude/policies/arch_variant_ledger.json"
        doc = json.loads(ledger_path.read_text())
        doc["schema_version"] = []
        doc["unexpected"] = True
        entry = doc["source_build_variants"]["deepseek-v4-flash-sm12x"]
        entry["unexpected"] = True
        ledger_path.write_text(json.dumps(doc))
        codes = _arch_codes(root)
        _require('ARCH_VARIANT_LEDGER_SHAPE_INVALID' in codes, 'predicate requirement failed at original line 2856')
        _require('ARCH_VARIANT_ENTRY_UNKNOWN_FIELD' in codes, 'predicate requirement failed at original line 2857')


def predicate_ARCH_WALL_VARIANT_LADDER_C4():
    """C4: unapproved build guard refuses; validator rejects missing evidence and broken HITL order."""
    guard = render_dockerfile._patch_guard("99.99-py3", "9.9.9")
    _require('HITL discovery loop' in guard, 'predicate requirement failed at original line 2863')
    _require(guard.rstrip().endswith('exit 1'), 'predicate requirement failed at original line 2864')
    _require('UNVALIDATED, not IMPOSSIBLE' in guard, 'predicate requirement failed at original line 2865')
    with tempfile.TemporaryDirectory() as tmp:
        root = _arch_contract_repo(tmp)
        doc = json.loads((root / ".claude/policies/arch_variant_ledger.json").read_text())
        entry = doc["source_build_variants"]["deepseek-v4-flash-sm12x"]
        original_pointer = dict(entry["evidence"])
        entry["evidence"] = {"path": "missing-approval.json"}
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_ARTIFACT_UNBOUND' in _arch_codes(root), 'predicate requirement failed at original line 2873')

        entry["evidence"] = original_pointer
        artifact_path = root / original_pointer["path"]
        artifact = json.loads(artifact_path.read_text())
        artifact["result"] = "FAIL"
        artifact_path.write_text(json.dumps(artifact, sort_keys=True), encoding="utf-8")
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_ARTIFACT_BINDING_MISMATCH' in _arch_codes(root), 'predicate requirement failed at original line 2893')

        entry["evidence"] = ""
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_ARTIFACT_POINTER_INVALID' in _arch_codes(root), 'predicate requirement failed at original line 2897')
        workflow = root / ".claude/rules/workflow.md"
        workflow.write_text(workflow.read_text().replace("testlog 기록 + 사람 승인", "사람 승인"))
        _require('ARCH_VARIANT_HITL_EVIDENCE_ORDER_INVALID' in _arch_codes(root), 'predicate requirement failed at original line 2900')

    with tempfile.TemporaryDirectory() as tmp:
        root = _arch_contract_repo(tmp)
        doc = json.loads((root / ".claude/policies/arch_variant_ledger.json").read_text())
        entry = doc["source_build_variants"]["deepseek-v4-flash-sm12x"]
        _mutate_arch_artifact(
            root, entry, "evidence", lambda artifact: artifact.__setitem__("approved_by", "HITL:AUTOMATION:unattended"))
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_APPROVAL_IDENTITY_MISSING' in _arch_codes(root), 'predicate requirement failed at original line 2909')

    with tempfile.TemporaryDirectory() as tmp:
        root = _arch_contract_repo(tmp)
        doc = json.loads((root / ".claude/policies/arch_variant_ledger.json").read_text())
        entry = doc["source_build_variants"]["deepseek-v4-flash-sm12x"]
        def remove_grounding(artifact):
            artifact["source_evidence"] = "x"
            artifact["approved_scope"] = "x"
        _mutate_arch_artifact(root, entry, "evidence", remove_grounding)
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_APPROVAL_GROUNDING_MISSING' in _arch_codes(root), 'predicate requirement failed at original line 2920')

    for field, value in (
        ("source_evidence", "plan_26062818 §999 R999 xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"),
        ("approved_scope", "This explicitly denies that serving smoke remains final arbitration for this variant."),
    ):
        with tempfile.TemporaryDirectory() as tmp:
            root = _arch_contract_repo(tmp)
            doc = json.loads((root / ".claude/policies/arch_variant_ledger.json").read_text())
            entry = doc["source_build_variants"]["deepseek-v4-flash-sm12x"]
            _mutate_arch_artifact(
                root, entry, "evidence", lambda artifact, f=field, v=value: artifact.__setitem__(f, v))
            (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
            _require('ARCH_VARIANT_APPROVAL_GROUNDING_MISSING' in _arch_codes(root), 'predicate requirement failed at original line 2933')

    with tempfile.TemporaryDirectory() as tmp:
        root = _arch_contract_repo(tmp)
        (root / ".claude/policies/provenance/plan_26062818_RouteB_jasl-fork_SM12x_DeepSeek-V4-Flash_2노드서빙.md").unlink()
        _require('ARCH_VARIANT_APPROVAL_GROUNDING_MISSING' in _arch_codes(root), 'predicate requirement failed at original line 2938')

    with tempfile.TemporaryDirectory() as tmp:
        root = _arch_contract_repo(tmp)
        doc = json.loads((root / ".claude/policies/arch_variant_ledger.json").read_text())
        entry = doc["source_build_variants"]["deepseek-v4-flash-sm12x"]
        def corrupt_shape(artifact):
            artifact.pop("schema_version", None)
            artifact["unexpected"] = True
        _mutate_arch_artifact(root, entry, "evidence", corrupt_shape)
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_ARTIFACT_SHAPE_INVALID' in _arch_codes(root), 'predicate requirement failed at original line 2949')


def predicate_ARCH_WALL_VARIANT_LADDER_C5():
    """C5: an active/default candidate cannot validate without existing-model regression evidence."""
    with tempfile.TemporaryDirectory() as tmp:
        root = _arch_contract_repo(tmp)
        doc = json.loads((root / ".claude/policies/arch_variant_ledger.json").read_text())
        entry = doc["source_build_variants"]["deepseek-v4-flash-sm12x"]
        entry["status"] = "VALIDATED"
        entry.pop("regression_evidence", None)
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_REGRESSION_EVIDENCE_MISSING' in _arch_codes(root), 'predicate requirement failed at original line 2961')
        _write_arch_regression_artifact(root, entry)
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_REGRESSION_EVIDENCE_MISSING' not in _arch_codes(root), 'predicate requirement failed at original line 2964')
        regression_path = root / entry["regression_evidence"]["path"]
        regression = json.loads(regression_path.read_text())
        regression["existing_models"] = []
        regression_path.write_text(json.dumps(regression, sort_keys=True), encoding="utf-8")
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_REGRESSION_MODELS_MISSING' in _arch_codes(root), 'predicate requirement failed at original line 2982')
        _mutate_arch_artifact(
            root, entry, "regression_evidence", lambda artifact: artifact.__setitem__("existing_models", True))
        (root / ".claude/policies/arch_variant_ledger.json").write_text(json.dumps(doc))
        _require('ARCH_VARIANT_REGRESSION_MODELS_MISSING' in _arch_codes(root), 'predicate requirement failed at original line 2986')
        skill = root / ".claude/skills/upstream-version-watch/SKILL.md"
        skill.write_text(skill.read_text().replace("기존모델 회귀 재스모크", "기존모델 확인"))
        _require('ARCH_VARIANT_PROMOTION_GUARDS_MISSING' in _arch_codes(root), 'predicate requirement failed at original line 2989')


# =============================================================================
# GIT_SINGLE_AUTHORITY (2026-09-03, plan_26090222) -- the two tripwires that keep git the only
# place a tracked byte's identity is written down.  Both predicates drive the REAL production
# functions in runtime_selftest.py, but against a purpose-built canonical-looking fixture repo
# rather than this clone: a live repo that is currently green gives a mutation test nothing to
# fail on, and one that is red would make the predicate report the working tree's state instead
# of the guard's behaviour (the reverse-oracle trap this project has hit three times).
# =============================================================================

_GIT_FIXTURE_ENV = {"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t",
                    "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"}


def _runtime_selftest():
    """Imported lazily: the production tripwires live in the runtime package, and this predicate
    file must stay importable even while that package is mid-edit."""
    return _import(".claude/policies/runtime", "runtime_selftest")


def _fixture_git(root: Path, *args: str) -> str:
    env = dict(os.environ, **_GIT_FIXTURE_ENV)
    r = subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True, env=env)
    _require(r.returncode == 0, f"fixture git {' '.join(args)} failed: {r.stderr.strip()}")
    return r.stdout.strip()


def _canonical_fixture_repo(tmp: str) -> Path:
    """A repo carrying runtime_selftest's three canonical markers, so `_is_canonical_repo` answers
    yes and the repository-state assertions actually execute -- they short-circuit to a silent
    no-op anywhere else, which would make every RED below a false GREEN."""
    root = Path(tmp)
    (root / ".claude" / "rules").mkdir(parents=True, exist_ok=True)
    (root / ".claude" / "policies").mkdir(parents=True, exist_ok=True)
    (root / "CLAUDE.md").write_text("fixture constitution\n", encoding="utf-8")
    (root / ".claude" / "rules" / "workflow.md").write_text("fixture workflow\n", encoding="utf-8")
    (root / ".claude" / "policies" / "registry.yaml").write_text(
        '{"schema_version": 2, "policies": []}\n', encoding="utf-8")
    (root / ".gitignore").write_text("*.log\n", encoding="utf-8")
    _fixture_git(root, "init", "-q", "-b", "single-node")
    _fixture_git(root, "add", "-A")
    _fixture_git(root, "commit", "-qm", "fixture")
    return root


def _tripwire_raises(fn, root: Path) -> bool:
    """True when the production tripwire fails closed on `root`."""
    try:
        fn(root)
    except _runtime_selftest().RuntimeSelftestFailure:
        return True
    return False


def predicate_GIT_SINGLE_AUTHORITY_C1():
    """C1: backup practice is forbidden outright rather than hidden.  Drives tripwire (1)
    `_test_no_backup_artifacts` through one RED->GREEN cycle per material atom of the clause:
    a `.bak`/`.orig` suffixed copy, a `backup`/`백업` named path, a branch outside
    {single-node, multi-node, hint}, a tag outside `hint/`, and a `.gitignore` line that would
    hide the first atom from `git status`."""
    rs = _runtime_selftest()
    _require(rs._ALLOWED_BRANCHES == frozenset({"single-node", "multi-node", "hint"}),
             f"refs/heads allowlist must be exactly the three operating branches: {sorted(rs._ALLOWED_BRANCHES)}")
    _require(rs._ALLOWED_TAG_PREFIX == "hint/",
             f"tags must be confined to the hint namespace, not {rs._ALLOWED_TAG_PREFIX!r}")
    _require(tuple(rs._BACKUP_SUFFIXES) == (".bak", ".orig"),
             f"the backup suffix set must stay (.bak, .orig): {rs._BACKUP_SUFFIXES}")
    _require(set(rs._BACKUP_PATH_TOKENS) == {"backup", "백업"},
             "the path-token set must cover this repo's actual Korean backup naming, not ASCII only")

    fn = rs._test_no_backup_artifacts
    with tempfile.TemporaryDirectory() as tmp:
        root = _canonical_fixture_repo(tmp)
        _require(not _tripwire_raises(fn, root),
                 "a clean canonical fixture repo must pass tripwire (1) -- GREEN baseline")

        copy = root / "Dockerfile.bak"
        copy.write_text("x", encoding="utf-8")
        _require(_tripwire_raises(fn, root), "a *.bak working-tree copy must fail tripwire (1)")
        copy.unlink()
        _require(not _tripwire_raises(fn, root),
                 "deleting the .bak copy must return tripwire (1) to GREEN (the check is live, not sticky)")

        korean = root / "이전 plan 백업"
        korean.mkdir()
        (korean / "a.md").write_text("x", encoding="utf-8")
        _require(_tripwire_raises(fn, root),
                 "a 백업-named directory must fail tripwire (1) -- an ASCII-only token had zero detection power here")
        shutil.rmtree(korean)

        _fixture_git(root, "branch", "wip-anchor")
        _require(_tripwire_raises(fn, root),
                 "a branch outside {single-node, multi-node, hint} must fail tripwire (1)")
        _fixture_git(root, "branch", "-D", "wip-anchor")
        _require(not _tripwire_raises(fn, root), "deleting the stray branch must return tripwire (1) to GREEN")

        _fixture_git(root, "tag", "last-good-26090301")
        _require(_tripwire_raises(fn, root),
                 "a last-good-* tag must fail tripwire (1) -- the rollback anchor is a commit, never a tag")
        _fixture_git(root, "tag", "-d", "last-good-26090301")

        (root / ".gitignore").write_text("*.log\n*.bak\n*.orig\n", encoding="utf-8")
        _require(_tripwire_raises(fn, root),
                 "resurrecting the *.bak/*.orig ignore lines must fail tripwire (1) -- hiding is not forbidding")
        (root / ".gitignore").write_text("*.log\n", encoding="utf-8")
        _require(not _tripwire_raises(fn, root), "restoring the ignore file must return tripwire (1) to GREEN")


def predicate_GIT_SINGLE_AUTHORITY_C2():
    """C2: no tracked .json/.yaml/.yml may restate a digest of bytes git already carries.  Drives
    tripwire (2) `_test_no_tracked_digest_rewrite`: both the git blob sha1 and the sha256 of the
    SAME tracked bytes fail, while a digest of bytes git does not carry passes -- the second half
    is the blind layer this policy deliberately keeps (out of reach by construction, not an
    allowlisted exemption)."""
    rs = _runtime_selftest()
    _require(rs._HEX_CONST_RE.findall("a" * 40) == ["a" * 40] and rs._HEX_CONST_RE.findall("b" * 64) == ["b" * 64],
             "the derived predicate must recognise both git blob sha1 (40-hex) and sha256 (64-hex) constants")

    fn = rs._test_no_tracked_digest_rewrite
    with tempfile.TemporaryDirectory() as tmp:
        root = _canonical_fixture_repo(tmp)
        _require(not _tripwire_raises(fn, root),
                 "a fixture repo with no transcribed digest must pass tripwire (2) -- GREEN baseline")

        blob_sha1 = _fixture_git(root, "rev-parse", "HEAD:CLAUDE.md")
        blob_sha256 = hashlib.sha256((root / "CLAUDE.md").read_bytes()).hexdigest()
        ledger = root / "ledger.json"

        for label, digest in (("git blob sha1", blob_sha1), ("sha256", blob_sha256)):
            ledger.write_text(json.dumps({"CLAUDE.md": digest}), encoding="utf-8")
            _fixture_git(root, "add", "-A")
            _require(_tripwire_raises(fn, root),
                     f"a tracked ledger restating the {label} of a tracked blob must fail tripwire (2)")

        outside = hashlib.sha256(b"upstream payload git does not carry").hexdigest()
        ledger.write_text(json.dumps({"upstream_payload": outside}), encoding="utf-8")
        _fixture_git(root, "add", "-A")
        _require(not _tripwire_raises(fn, root),
                 "a digest of bytes git does not carry is outside the predicate by construction -- "
                 "the blind layer stays, and it is not maintained as an allowlist")

        ledger.unlink()
        _fixture_git(root, "add", "-A")
        _require(not _tripwire_raises(fn, root),
                 "removing the transcription must return tripwire (2) to GREEN")


# =============================================================================
# Exact clause_id -> predicate function mapping (59 entries -- parity asserted in the test class).
# =============================================================================

PREDICATES = {
    "HOST_SAFETY_LAYERED_DEFENSE.C1": predicate_HOST_SAFETY_LAYERED_DEFENSE_C1,
    "HOST_SAFETY_LAYERED_DEFENSE.C2": predicate_HOST_SAFETY_LAYERED_DEFENSE_C2,
    "HOST_SAFETY_LAYERED_DEFENSE.C3": predicate_HOST_SAFETY_LAYERED_DEFENSE_C3,
    "HOST_SAFETY_LAYERED_DEFENSE.C4": predicate_HOST_SAFETY_LAYERED_DEFENSE_C4,
    "HOST_SAFETY_LAYERED_DEFENSE.C5": predicate_HOST_SAFETY_LAYERED_DEFENSE_C5,
    "HOST_SAFETY_LAYERED_DEFENSE.C6": predicate_HOST_SAFETY_LAYERED_DEFENSE_C6,
    "HOST_SAFETY_LAYERED_DEFENSE.C7": predicate_HOST_SAFETY_LAYERED_DEFENSE_C7,
    "HOST_SAFETY_LAYERED_DEFENSE.C8": predicate_HOST_SAFETY_LAYERED_DEFENSE_C8,
    "HOST_SAFETY_LAYERED_DEFENSE.C9": predicate_HOST_SAFETY_LAYERED_DEFENSE_C9,
    "VARIANT_IMAGE_BUILD_VS_SERVE_PLANE.C1": predicate_VARIANT_IMAGE_BUILD_VS_SERVE_PLANE_C1,
    "VARIANT_IMAGE_BUILD_VS_SERVE_PLANE.C2": predicate_VARIANT_IMAGE_BUILD_VS_SERVE_PLANE_C2,
    "VARIANT_IMAGE_BUILD_VS_SERVE_PLANE.C3": predicate_VARIANT_IMAGE_BUILD_VS_SERVE_PLANE_C3,
    "MODEL_TRIPLET_NO_SUB_PROPAGATION.C1": predicate_MODEL_TRIPLET_NO_SUB_PROPAGATION_C1,
    "MODEL_TRIPLET_NO_SUB_PROPAGATION.C2": predicate_MODEL_TRIPLET_NO_SUB_PROPAGATION_C2,
    "MODEL_TRIPLET_NO_SUB_PROPAGATION.C3": predicate_MODEL_TRIPLET_NO_SUB_PROPAGATION_C3,
    "HINT_TAG_ACTIVATION_GATE.C1": predicate_HINT_TAG_ACTIVATION_GATE_C1,
    "HINT_TAG_ACTIVATION_GATE.C2": predicate_HINT_TAG_ACTIVATION_GATE_C2,
    "HINT_TAG_ACTIVATION_GATE.C3": predicate_HINT_TAG_ACTIVATION_GATE_C3,
    "HINT_TAG_ACTIVATION_GATE.C4": predicate_HINT_TAG_ACTIVATION_GATE_C4,
    "HINT_TAG_ACTIVATION_GATE.C5": predicate_HINT_TAG_ACTIVATION_GATE_C5,
    "HINT_TAG_ACTIVATION_GATE.C6": predicate_HINT_TAG_ACTIVATION_GATE_C6,
    "LAST_GOOD_ROLLBACK_ANCHOR.C1": predicate_LAST_GOOD_ROLLBACK_ANCHOR_C1,
    "LAST_GOOD_ROLLBACK_ANCHOR.C2": predicate_LAST_GOOD_ROLLBACK_ANCHOR_C2,
    "LAST_GOOD_ROLLBACK_ANCHOR.C3": predicate_LAST_GOOD_ROLLBACK_ANCHOR_C3,
    "MODEL_ACQUISITION_TERNARY_GATE.C1": predicate_MODEL_ACQUISITION_TERNARY_GATE_C1,
    "MODEL_ACQUISITION_TERNARY_GATE.C2": predicate_MODEL_ACQUISITION_TERNARY_GATE_C2,
    "MODEL_ACQUISITION_TERNARY_GATE.C3": predicate_MODEL_ACQUISITION_TERNARY_GATE_C3,
    "MODEL_ACQUISITION_TERNARY_GATE.C4": predicate_MODEL_ACQUISITION_TERNARY_GATE_C4,
    "MODEL_ACQUISITION_TERNARY_GATE.C5": predicate_MODEL_ACQUISITION_TERNARY_GATE_C5,
    "KV_ABSOLUTE_CLAMP_PORTABILITY.C1": predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C1,
    "KV_ABSOLUTE_CLAMP_PORTABILITY.C2": predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C2,
    "KV_ABSOLUTE_CLAMP_PORTABILITY.C3": predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C3,
    "KV_ABSOLUTE_CLAMP_PORTABILITY.C4": predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C4,
    "RUNTIME_PATCH_NO_CARRY_FORWARD.C1": predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C1,
    "RUNTIME_PATCH_NO_CARRY_FORWARD.C2": predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C2,
    "RUNTIME_PATCH_NO_CARRY_FORWARD.C3": predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C3,
    "RUNTIME_PATCH_NO_CARRY_FORWARD.C4": predicate_RUNTIME_PATCH_NO_CARRY_FORWARD_C4,
    "SUB_SYNC_DIRTY_AUTOSAVE.C1": predicate_SUB_SYNC_DIRTY_AUTOSAVE_C1,
    "SUB_SYNC_DIRTY_AUTOSAVE.C2": predicate_SUB_SYNC_DIRTY_AUTOSAVE_C2,
    "SUB_SYNC_DIRTY_AUTOSAVE.C3": predicate_SUB_SYNC_DIRTY_AUTOSAVE_C3,
    "SUB_SYNC_DIRTY_AUTOSAVE.C4": predicate_SUB_SYNC_DIRTY_AUTOSAVE_C4,
    "SUB_GIT_LOCAL_ONLY.C1": predicate_SUB_GIT_LOCAL_ONLY_C1,
    "SUB_GIT_LOCAL_ONLY.C2": predicate_SUB_GIT_LOCAL_ONLY_C2,
    "SUB_GIT_LOCAL_ONLY.C3": predicate_SUB_GIT_LOCAL_ONLY_C3,
    "TERRAFORM_FLAG_GATE.C1": predicate_TERRAFORM_FLAG_GATE_C1,
    "TERRAFORM_FLAG_GATE.C2": predicate_TERRAFORM_FLAG_GATE_C2,
    "TERRAFORM_FLAG_GATE.C3": predicate_TERRAFORM_FLAG_GATE_C3,
    "TERRAFORM_FLAG_GATE.C4": predicate_TERRAFORM_FLAG_GATE_C4,
    "A2A_DELEGATION_KEY_FAIL_CLOSED.C1": predicate_A2A_DELEGATION_KEY_FAIL_CLOSED_C1,
    "A2A_DELEGATION_KEY_FAIL_CLOSED.C2": predicate_A2A_DELEGATION_KEY_FAIL_CLOSED_C2,
    "A2A_DELEGATION_KEY_FAIL_CLOSED.C3": predicate_A2A_DELEGATION_KEY_FAIL_CLOSED_C3,
    "A2A_DELEGATION_KEY_FAIL_CLOSED.C4": predicate_A2A_DELEGATION_KEY_FAIL_CLOSED_C4,
    "ARCH_WALL_VARIANT_LADDER.C1": predicate_ARCH_WALL_VARIANT_LADDER_C1,
    "ARCH_WALL_VARIANT_LADDER.C2": predicate_ARCH_WALL_VARIANT_LADDER_C2,
    "ARCH_WALL_VARIANT_LADDER.C3": predicate_ARCH_WALL_VARIANT_LADDER_C3,
    "ARCH_WALL_VARIANT_LADDER.C4": predicate_ARCH_WALL_VARIANT_LADDER_C4,
    "ARCH_WALL_VARIANT_LADDER.C5": predicate_ARCH_WALL_VARIANT_LADDER_C5,
    "GIT_SINGLE_AUTHORITY.C1": predicate_GIT_SINGLE_AUTHORITY_C1,
    "GIT_SINGLE_AUTHORITY.C2": predicate_GIT_SINGLE_AUTHORITY_C2,
}


def _load_real_registry_clause_ids() -> set:
    import json
    doc = json.loads((REPO_ROOT / ".claude" / "policies" / "registry.yaml").read_text(encoding="utf-8"))
    return {c["clause_id"] for p in doc["policies"] for c in p.get("clauses", [])}


class TestAllClausePredicatesExecute(unittest.TestCase):
    """Enumerates the exact {clause_id: predicate function} mapping, asserts exact parity with the
    real registry's 59 clause_ids, and executes every predicate under subTest -- a single
    predicate raising AssertionError fails only that clause's subTest, not the whole run."""

    def test_mapping_has_exactly_59_entries(self):
        self.assertEqual(len(PREDICATES), 59)

    def test_mapping_matches_real_registry_clause_ids_exactly(self):
        self.assertEqual(set(PREDICATES), _load_real_registry_clause_ids())

    def test_all_predicate_functions_are_distinct(self):
        funcs = list(PREDICATES.values())
        self.assertEqual(len(funcs), len(set(funcs)), "no clause may share a predicate function with another")
        names = {f.__name__ for f in funcs}
        self.assertEqual(len(names), len(funcs))

    def test_every_predicate_source_has_a_real_assertion_beyond_helper_calls(self):
        """A predicate whose body is ONLY a call to some other helper (no direct `assert` in its
        own source) would let a genuinely broken helper silently no-op past detection -- every
        predicate must contain at least one literal `assert` statement in ITS OWN source."""
        for clause_id, fn in PREDICATES.items():
            with self.subTest(clause=clause_id):
                src = inspect.getsource(fn)
                tree = ast.parse(src)
                fn_node = tree.body[0]
                assert_count = sum(1 for n in ast.walk(fn_node) if isinstance(n, ast.Assert))
                self.assertGreaterEqual(assert_count, 1,
                                         f"{fn.__name__} has no direct assert statement in its own body")

    def test_all_59_clause_predicates_execute(self):
        for clause_id, fn in sorted(PREDICATES.items()):
            with self.subTest(clause=clause_id):
                fn()  # raises AssertionError/SystemExit-mismatch on genuine failure

    def test_canonical_registry_and_binding_map_cannot_collude_on_unrelated_identifier(self):
        cid = "HOST_SAFETY_LAYERED_DEFENSE.C1"
        policies = [{
            "policy_id": "HOST_SAFETY_LAYERED_DEFENSE",
            "clauses": [{"clause_id": cid, "statement": "unrelated"}],
            "evidence": [{"path": ".claude/policies/runtime/policy_registry.py", "supports": [cid],
                          "assertion_ids": ["evaluate_lifecycle"]}],
        }]
        colluding = {"schema_version": 1, "bindings": {cid: [{"path": ".claude/policies/runtime/policy_registry.py",
                                                               "assertion_ids": ["evaluate_lifecycle"]}]}}
        violations = policy_registry.claim_binding_violations(policies, colluding)
        self.assertEqual([v.reason_code for v in violations],
                         ["CLAIM_BINDING_MISSING_DEDICATED_PREDICATE"])
        self.assertEqual(violations[0].clause_id, cid)

    def test_canonical_claim_bindings_cannot_be_empty_or_malformed(self):
        cid = "HOST_SAFETY_LAYERED_DEFENSE.C1"
        policies = [{"policy_id": "HOST_SAFETY_LAYERED_DEFENSE",
                     "clauses": [{"clause_id": cid}], "evidence": []}]
        for malformed in ({}, {"schema_version": 999, "bindings": {}, "unknown": True}):
            with self.subTest(malformed=malformed):
                violations = policy_registry.claim_binding_violations(policies, malformed)
                self.assertTrue(violations)
                self.assertIn(violations[0].reason_code,
                              {"CLAIM_BINDINGS_INVALID", "CLAIM_BINDINGS_INCOMPLETE"})


def run_all_predicates() -> int:
    """Execute the exact registry mapping and emit a stable production verdict."""
    failures = []
    registry_ids = _load_real_registry_clause_ids()
    if len(PREDICATES) != 59 or set(PREDICATES) != registry_ids:
        failures.append({"clause_id": "__mapping__", "error":
                         f"predicate/registry mismatch predicates={len(PREDICATES)} registry={len(registry_ids)}"})
    funcs = list(PREDICATES.values())
    if len(funcs) != len(set(funcs)) or len({fn.__name__ for fn in funcs}) != len(funcs):
        failures.append({"clause_id": "__mapping__", "error": "predicate functions must be distinct"})
    for clause_id, fn in sorted(PREDICATES.items()):
        captured_out = io.StringIO()
        captured_err = io.StringIO()
        try:
            tree = ast.parse(inspect.getsource(fn))
            direct_requires = sum(
                1 for node in ast.walk(tree)
                if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)
                and node.func.id == "_require")
            _require(direct_requires >= 1, f"{fn.__name__} has no direct explicit _require call")
            with contextlib.redirect_stdout(captured_out), contextlib.redirect_stderr(captured_err):
                fn()
        except Exception as exc:  # fail closed per clause while still reporting the full set
            failures.append({"clause_id": clause_id,
                             "error": f"{type(exc).__name__}: {exc}",
                             "stdout_tail": captured_out.getvalue()[-500:],
                             "stderr_tail": captured_err.getvalue()[-500:]})
    payload = {
        "schema_version": 1,
        "predicate_count": len(PREDICATES),
        "failed_count": len(failures),
        "failures": failures,
        "verdict": "PASS" if not failures else "FAIL",
    }
    print(json.dumps(payload, ensure_ascii=False, sort_keys=True, indent=2))
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(run_all_predicates())
