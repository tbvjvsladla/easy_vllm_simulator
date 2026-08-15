#!/usr/bin/env python3
"""Fail-closed acceptance for the five-skill self-contained distribution.

This is a production distribution contract, not a development test runner.  It proves that a
fresh gitless checkout contains exactly the five public skills, no top-level tests/scripts
control plane, no trust artifact pointing back to those removed roots, and that each public
skill's deterministic entry path still starts or returns its documented pre-terraform gate.
"""
from __future__ import annotations

import argparse
import ast
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
EXPECTED_SKILLS = {
    "adversarial-benchmark",
    "terraforming_node",
    "upstream-version-watch",
    "vllm-recipe-explorer",
    "wiki-desk",
}
TRUST_FILES = (
    ".claude/policies/registry.yaml",
    ".claude/policies/claim_bindings.json",
    ".claude/policies/evidence_manifest.json",
    ".claude/policies/tracked_index.json",
)
LOCAL_TOMBSTONES = {
    "scripts/agent_control.py", "scripts/cleanup_docker.py", "scripts/completion_gate.py",
    "scripts/doc_naming.py", "scripts/engine_liveness_watchdog.sh",
    "scripts/evidence_publisher.py", "scripts/harness_verify.py", "scripts/hint_tag.py",
    "scripts/host/vllm-drop-caches.sh", "scripts/install_host_safety.sh",
    "scripts/mem_watchdog.sh", "scripts/policy_registry.py", "scripts/providers/claude_code.py",
    "scripts/smoke_clone.sh", "scripts/sync_branches.sh",
    "scripts/systemd/easy-vllm-memwatch.service",
    "scripts/templates/hint_recipe.template.md",
}
LOCAL_REPLACEMENTS = {
    ".claude/policies/runtime/agent_control.py",
    ".claude/skills/vllm-recipe-explorer/scripts/cleanup_docker.py",
    ".claude/policies/runtime/completion_gate.py",
    ".claude/skills/wiki-desk/scripts/doc_naming.py",
    ".claude/skills/vllm-recipe-explorer/scripts/engine_liveness_watchdog.sh",
    ".claude/policies/runtime/evidence_publisher.py",
    ".claude/policies/runtime/harness_verify.py",
    ".claude/skills/upstream-version-watch/scripts/hint_tag.py",
    ".claude/skills/terraforming_node/scripts/host_safety/host/vllm-drop-caches.sh",
    ".claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh",
    ".claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh",
    ".claude/policies/runtime/policy_registry.py",
    ".claude/policies/runtime/providers/claude_code.py",
    ".claude/skills/upstream-version-watch/scripts/smoke_clone.sh",
    ".claude/skills/upstream-version-watch/scripts/sync_branches.sh",
    ".claude/skills/terraforming_node/scripts/host_safety/systemd/easy-vllm-memwatch.service",
    ".claude/skills/upstream-version-watch/templates/hint_recipe.template.md",
}
SUB_TOMBSTONES = {
    ".claude/rules/references.md", "scripts/install_host_safety.sh",
    "scripts/mem_watchdog.sh", "scripts/host/vllm-drop-caches.sh",
    "scripts/systemd/easy-vllm-memwatch.service", "scripts/smoke_clone.sh",
    "scripts/sync_branches.sh",
}
SUB_RELOCATION_TOMBSTONES = {
    ".claude/rules/references.md", "scripts/install_host_safety.sh",
    "scripts/mem_watchdog.sh", "scripts/host/vllm-drop-caches.sh",
    "scripts/systemd/easy-vllm-memwatch.service",
}
SUB_RETIREMENT_TOMBSTONES = {"scripts/smoke_clone.sh", "scripts/sync_branches.sh"}


def _child_python() -> list[str]:
    """Preserve security-relevant interpreter flags for the production runtime self-test."""
    flags = []
    if sys.flags.isolated:
        flags.append("-I")
    if sys.flags.no_site:
        flags.append("-S")
    if sys.flags.optimize:
        flags.append("-" + "O" * sys.flags.optimize)
    if sys.flags.dont_write_bytecode:
        flags.append("-B")
    return [sys.executable, *flags]


def _run(name: str, argv: list[str], expected: set[int], cwd: Path = REPO,
         env_overrides: dict[str, str] | None = None) -> dict:
    env = os.environ.copy()
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    if env_overrides:
        env.update(env_overrides)
    try:
        proc = subprocess.run(argv, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE, timeout=180)
        return {"name": name, "ok": proc.returncode in expected, "rc": proc.returncode,
                "expected_rc": sorted(expected), "stdout_tail": proc.stdout[-1000:],
                "stderr_tail": proc.stderr[-1000:]}
    except (OSError, subprocess.TimeoutExpired) as exc:
        return {"name": name, "ok": False, "rc": None, "expected_rc": sorted(expected),
                "error": f"{type(exc).__name__}: {exc}"}


def _walk_strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, list):
        for item in value:
            yield from _walk_strings(item)
    elif isinstance(value, dict):
        for key, item in value.items():
            yield from _walk_strings(key)
            yield from _walk_strings(item)


def _shell_array(text: str, name: str) -> set[str]:
    match = re.search(rf"(?m)^{re.escape(name)}=\(\n(.*?)^\)$", text, re.DOTALL)
    if match is None:
        return set()
    return {line.split("#", 1)[0].strip().strip("'\"")
            for line in match.group(1).splitlines()
            if line.split("#", 1)[0].strip()}


def _ordered_between_detail(text: str, marker: str, end_marker: str,
                            tokens: tuple[str, ...]) -> dict:
    """구간 내 토큰 순서를 검사하되 **실패 사유를 구분해** 돌려준다.

    2026-08-15 신설(`request_26081521_01_04` 처방 C). 이전 판본은 bool 하나만 돌려서
    **앵커 부재와 순서 위반이 같은 값(False)으로 뭉개졌다**. 2026-08-13 배달경로 교정이
    종료 앵커로 쓰이던 주석을 바꾸자 `sub_incremental_replacement_before_tombstone` 이
    FAIL 로 떨어졌는데, `ok:false` 만으로는 "가드가 제 일을 했다(순서가 깨졌다)"와
    "판정기가 자기 앵커를 잃었다"를 구분할 수 없어 진단 비용의 대부분이 거기서 났다.
    `sync_to_sub.sh` 는 서브 파괴 권한을 가진 스크립트이므로, 이 구분이 없으면
    "가드를 끄는 처방"과 "앵커를 고치는 처방"도 겉보기에 같아진다.

    reason: ``start_anchor_missing`` | ``end_anchor_missing`` | ``token_absent``
            | ``token_out_of_order`` | ``None``(통과)
    """
    start = text.find(marker)
    if start < 0:
        return {"ok": False, "reason": "start_anchor_missing", "missing": marker}
    end = text.find(end_marker, start + len(marker))
    if end < 0:
        return {"ok": False, "reason": "end_anchor_missing", "missing": end_marker}
    block = text[start:end]
    cursor = 0
    for token in tokens:
        found = block.find(token, cursor)
        if found < 0:
            # 순차 검색이라 "구간에 아예 없음"과 "앞 토큰보다 먼저 나옴"이 같은 -1 로 돌아온다.
            # 구간 전체를 다시 훑어야만 그 둘이 갈린다 — 순서 위반은 여기서만 관측된다.
            reason = "token_absent" if block.find(token) < 0 else "token_out_of_order"
            return {"ok": False, "reason": reason, "missing": token}
        cursor = found + len(token)
    return {"ok": True, "reason": None}


def _ordered_between(text: str, marker: str, end_marker: str,
                     tokens: tuple[str, ...]) -> bool:
    """`_ordered_between_detail` 의 bool 축약.

    체크 본문은 `_order_check` 를 쓰므로 여기서는 호출되지 않지만, 수행지시서
    (`request_26081521_01_04` §2 Step 3 양성 대조)가 이 심볼을 직접 호출한다 —
    제거하면 그 재현 절차가 깨지고, 그것이 바로 이 함수가 고치려는 종류의 사고다.
    """
    return _ordered_between_detail(text, marker, end_marker, tokens)["ok"]


def _order_check(name: str, text: str, marker: str, end_marker: str,
                 tokens: tuple[str, ...]) -> dict:
    """순서 불변식 체크 하나를 진단(`order_detail`)과 함께 만든다."""
    detail = _ordered_between_detail(text, marker, end_marker, tokens)
    return {"name": name, "ok": detail["ok"], "order_detail": detail}


def _active_retirement_consumers() -> list[str]:
    hits: list[str] = []
    roots = [REPO / ".claude/rules", REPO / "CLAUDE.md", REPO / "HINTS.md"]
    token_chars = set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_./-")
    for root in roots:
        files = sorted(root.rglob("*")) if root.is_dir() else [root]
        for file in files:
            if not file.is_file():
                continue
            try:
                text = file.read_text(encoding="utf-8")
            except UnicodeDecodeError:
                continue
            for stale in sorted(SUB_RETIREMENT_TOMBSTONES):
                pattern = re.compile(rf"(?<![A-Za-z0-9_.-]){re.escape(stale)}(?![A-Za-z0-9_./-])")
                owner = f".claude/skills/upstream-version-watch/{stale}"
                for match in pattern.finditer(text):
                    lo, hi = match.start(), match.end()
                    while lo and text[lo - 1] in token_chars:
                        lo -= 1
                    while hi < len(text) and text[hi] in token_chars:
                        hi += 1
                    token = text[lo:hi]
                    while token.startswith("./"):
                        token = token[2:]
                    if token != owner:
                        hits.append(str(file.relative_to(REPO)))
                        break
                if hits and hits[-1] == str(file.relative_to(REPO)):
                    break
    return hits


def _terraform_flag_issued() -> bool:
    """info-only(미테라포밍) 게이트의 *적용 가능 여부*를 결정론으로 판별한다.

    권위는 소유 스크립트에 위임한다(계약 중복 금지): run_bench.sh 와 동일하게
    terraforming_node 의 manifest_contract.py --require-flag 를 실제 실행하고,
    A2A 면제 2경로(양성 키·테스트 env)는 recipe.py _require_terraform_flag 와 동형으로 읽는다.
    fail-closed: 판별 불가·부정이면 False — 그 경우 검사를 *실행*하는 안전 방향으로 떨어진다.
    """
    if os.environ.get("EASY_VLLM_A2A_DELEGATED") == "1":
        return True
    key = REPO / ".claude" / "a2a_delegation.json"
    try:
        if key.is_file():
            kd = json.loads(key.read_text(encoding="utf-8"))
            if kd.get("delegation") == "main_cluster_flag" and kd.get("issued_to") == "sub":
                return True
    except Exception:
        pass  # 손상/비유효 키 → 면제 안 함(fail-closed)
    mc = REPO / ".claude/skills/terraforming_node/scripts/manifest_contract.py"
    if not mc.is_file():
        return False
    try:
        branch = subprocess.run(["git", "-C", str(REPO), "branch", "--show-current"],
                                capture_output=True, text=True).stdout.strip()
    except OSError:
        branch = ""
    topo = "multi" if branch == "multi-node" else "single"
    try:
        proc = subprocess.run([sys.executable, str(mc), "--topology", topo,
                               "--repo", str(REPO), "--require-flag"],
                              capture_output=True)
        return proc.returncode == 0
    except OSError:
        return False


def verify() -> dict:
    checks: list[dict] = []
    for root_name in ("tests", "scripts"):
        checks.append({"name": f"root_absent:{root_name}",
                       "ok": not (REPO / root_name).exists()})

    skills = {p.parent.name for p in (REPO / ".claude/skills").glob("*/SKILL.md")}
    checks.append({"name": "exact_public_skill_set", "ok": skills == EXPECTED_SKILLS,
                   "actual": sorted(skills), "expected": sorted(EXPECTED_SKILLS)})

    for rel in TRUST_FILES:
        path = REPO / rel
        try:
            doc = json.loads(path.read_text(encoding="utf-8"))
            stale = sorted({s for s in _walk_strings(doc)
                            if s.startswith("tests/") or s.startswith("scripts/")})
            checks.append({"name": f"trust_owner_paths:{rel}", "ok": not stale,
                           "stale_root_paths": stale})
        except (OSError, UnicodeError, json.JSONDecodeError) as exc:
            checks.append({"name": f"trust_owner_paths:{rel}", "ok": False,
                           "error": f"{type(exc).__name__}: {exc}"})

    try:
        tracked = json.loads((REPO / ".claude/policies/tracked_index.json")
                             .read_text(encoding="utf-8"))["entries"]
        drift = []
        for rel, expected in sorted(tracked.items()):
            path = REPO / rel
            if path.is_symlink() or not path.is_file():
                drift.append(f"{rel}:missing-or-symlink")
                continue
            data = path.read_bytes()
            actual = hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()
            if actual != expected:
                drift.append(f"{rel}:{actual}!={expected}")
        checks.append({"name": "tracked_index_all_entry_bytes", "ok": not drift,
                       "drift": drift})
    except (OSError, UnicodeError, json.JSONDecodeError, KeyError, TypeError) as exc:
        checks.append({"name": "tracked_index_all_entry_bytes", "ok": False,
                       "error": f"{type(exc).__name__}: {exc}"})

    bare_asserts = []
    for path in sorted((REPO / ".claude").rglob("*.py")):
        rel = str(path.relative_to(REPO))
        try:
            if path.is_symlink():
                raise OSError("shipped Python symlink is forbidden")
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=rel)
            bare_asserts.extend(f"{rel}:{node.lineno}" for node in ast.walk(tree)
                                if isinstance(node, ast.Assert))
        except (OSError, UnicodeError, SyntaxError) as exc:
            bare_asserts.append(f"{rel}:{type(exc).__name__}:{exc}")
    checks.append({"name": "shipped_python_has_no_bare_assert",
                   "ok": not bare_asserts, "violations": bare_asserts})

    local_sync = REPO / ".claude/skills/upstream-version-watch/scripts/sync_branches.sh"
    sub_sync = REPO / ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh"
    try:
        local_text = local_sync.read_text(encoding="utf-8")
        local_actual = _shell_array(local_text, "ROOT_RELOCATION_TOMBSTONES")
        replacements = _shell_array(local_text, "ROOT_RELOCATION_REPLACEMENTS")
        local_order = ("git checkout ", "source→destination Git object/mode mismatch",
                       "materialized replacement byte/mode mismatch", "git rm --ignore-unmatch")
        checks += [
            {"name": "local_exact_relocation_tombstones",
             "ok": local_actual == LOCAL_TOMBSTONES,
             "actual": sorted(local_actual), "expected": sorted(LOCAL_TOMBSTONES)},
            {"name": "local_exact_relocation_replacements",
             "ok": replacements == LOCAL_REPLACEMENTS and len(replacements) == len(local_actual),
             "actual": sorted(replacements), "expected": sorted(LOCAL_REPLACEMENTS)},
            _order_check("local_replacement_integrity_before_tombstone", local_text,
                         "# ── apply:", "echo \"[sync-branches] 완료", local_order),
        ]
    except (OSError, UnicodeError) as exc:
        checks.append({"name": "local_exact_relocation_tombstones", "ok": False,
                       "error": f"{type(exc).__name__}: {exc}"})

    try:
        sub_text = sub_sync.read_text(encoding="utf-8")
        sub_actual = _shell_array(sub_text, "OVERLAY_STALE_PATHS")
        sub_relocations = _shell_array(sub_text, "OVERLAY_RELOCATION_STALE_PATHS")
        sub_retirements = _shell_array(sub_text, "OVERLAY_RETIREMENT_STALE_PATHS")
        additive_match = re.search(r"(?ms)^deliver_overlay\(\).*?^}\n", sub_text)
        additive_body = additive_match.group(0) if additive_match else ""
        order = ("verify_checksums ", "verify_destination_runner_modes ",
                 "verify_destination_host_safety_modes", "verify_destination_retirement_consumers",
                 "apply_overlay_tombstones", "git add -A")
        # 두 구간을 and 로 묶는 체크라 진단도 둘 다 보존한다 — 어느 쪽이 깨졌는지 모르면
        # `ok:false` 하나로 뭉개지는 것은 마찬가지다.
        tx_before_mutation = [
            _ordered_between_detail(sub_text, "# (2) transaction before checkout",
                                    "# (3) rsync(빌드 + 오버레이)",
                                    ('begin_remote_transaction "$t" 0', 'git checkout -q $t')),
            _ordered_between_detail(sub_text, "# R2 HITL 게이트",
                                    "# ── B0 멱등 self-bootstrap",
                                    ("begin_remote_transaction multi 1", "sub_run_mk")),
        ]
        checks += [
            {"name": "sub_exact_relocation_tombstones", "ok": sub_actual == SUB_TOMBSTONES,
             "actual": sorted(sub_actual), "expected": sorted(SUB_TOMBSTONES)},
            {"name": "sub_relocation_retirement_partition",
             "ok": (sub_relocations == SUB_RELOCATION_TOMBSTONES
                    and sub_retirements == SUB_RETIREMENT_TOMBSTONES
                    and sub_relocations.isdisjoint(sub_retirements)
                    and sub_relocations | sub_retirements == SUB_TOMBSTONES),
             "actual": {"relocations": sorted(sub_relocations),
                        "retirements": sorted(sub_retirements)}},
            {"name": "sub_additive_overlay_has_no_apply_delete",
             "ok": bool(additive_body) and "rm -f" not in additive_body},
            _order_check("sub_bootstrap_replacement_before_tombstone", sub_text,
                         "# multi 초기 Band2 배달", "# ── B1 per-branch 증분 싱크", order),
            # 종료 앵커는 **코드 토큰**이다(2026-08-15 · `request_26081521_01_04` 처방 B).
            # 옛 앵커 "# 서브를 기본 운용 브랜치" 는 주석이었고, 2026-08-13 배달경로 교정이
            # 그 주석을 다시 쓰면서 소실됐다 — 주석은 문서 교정 때 자유롭게 바뀌므로 앵커로
            # 부적합하다. `REST_BRANCH=` 는 B1 루프가 끝나고 복귀 브랜치를 정하는 자리이며,
            # 그 교정의 산물 자체다. 이 토큰이 사라지는 변경은 복귀 로직이 바뀌었다는 뜻이라
            # 그때는 판정기도 함께 리뷰돼야 하는 것이 맞다.
            _order_check("sub_incremental_replacement_before_tombstone", sub_text,
                         "# (3) rsync(빌드 + 오버레이)", "REST_BRANCH=", order),
            {"name": "sub_render_uses_transactional_source",
             "ok": all(token in sub_text for token in (
                 "prepare_transactional_source", "mktemp -d", "CANONICAL_SRC",
                 'SRC="$TRANSACTIONAL_SRC/"'))
             and sub_text.find("prepare_transactional_source\n")
             < sub_text.find("# ═══════════════════════ DRY-RUN")},
            {"name": "sub_transactional_source_uses_git_index",
             "ok": ("checkout-index -z --stdin" in sub_text
                    and "filesystem bytes are excluded in favor of index authority" in sub_text
                    and "ls-files -z -- .claude CLAUDE.md .gitignore output/multi output/single" in sub_text
                    and 'install -m 0600 "${CANONICAL_SRC}output/$topology/manifest.yaml"' in sub_text
                    and '"${CANONICAL_SRC}output/$topology/"' not in sub_text)},
            {"name": "sub_runtime_patch_transfer_is_owner_allowlisted",
             "ok": ("BAND2_RUNTIME_PATCH_STEMS=(exaone45-33b hy3)" in sub_text
                    and "--include='/configs/*_patch.py'" not in sub_text
                    and '"$build_assets"/runtime_patches/*' in sub_text)},
            {"name": "sub_retirement_consumer_audit_before_tombstone",
             "ok": (sub_text.count("verify_destination_retirement_consumers ||") == 2
                    and "retirement_consumer_scan" in sub_text
                    and 'owner=".claude/skills/upstream-version-watch/"+stale' in sub_text
                    and 'while lo and line[lo-1] in chars' in sub_text
                    and "scanner/transport failed for $stale" in sub_text)},
            {"name": "no_active_sub_retirement_consumers",
             "ok": not _active_retirement_consumers(),
             "details": _active_retirement_consumers()},
            {"name": "sub_invocation_rollback_transaction",
             "ok": all(token in sub_text for token in (
                 "begin_remote_transaction multi 1", 'begin_remote_transaction "$t" 0',
                 "rollback_remote_transactions", "finalize_remote_transactions",
                 "workdir-absent", "workdir-backup",
                 "trap 'transactional_exit $?' EXIT", "rollback failed; recovery backups retained",
                 "rollback_remote_transactions || rc=11"))},
            {"name": "sub_transaction_precedes_remote_mutation",
             "ok": tx_before_mutation[0]["ok"] and tx_before_mutation[1]["ok"],
             "order_detail": tx_before_mutation},
            {"name": "sub_deletion_inventory_and_brake_fail_closed",
             "ok": all(token in sub_text for token in (
                 "deletion inventory dry-run failed", "deletion brake dry-run failed before apply",
                 "validate_remote_deletion_tree"))},
            {"name": "sub_path_protocol_closed",
             "ok": all(token in sub_text for token in (
                 "validate_inventory_relative_path", "[[:space:][:cntrl:]]",
                 "empty directories are undeclared transfer artifacts"))},
            _order_check("sub_cleanup_is_postsuccess_best_effort", sub_text,
                         "finalize_remote_transactions()", "transactional_exit()",
                         ("REMOTE_TX_ACTIVE=0", "successful sync left recovery backup")),
        ]
    except (OSError, UnicodeError) as exc:
        checks.append({"name": "sub_relocation_contract", "ok": False,
                       "error": f"{type(exc).__name__}: {exc}"})

    skeleton_root = REPO / ".claude/skills/terraforming_node/templates/document_skeletons"
    skeletons = [skeleton_root / kind / "example.md"
                 for kind in ("plan", "devlog", "testlog", "simlog", "benchmark")]
    checks.append({"name": "terraform_owner_doc_skeletons",
                   "ok": all(p.is_file() and p.stat().st_size > 0 for p in skeletons),
                   "paths": [str(p.relative_to(REPO)) for p in skeletons]})
    build_asset_root = REPO / ".claude/skills/upstream-version-watch/assets/build_plane"
    # requirements.txt 는 2026-08-13 이 목록에서 빠졌다 — Band1 정적 사본을 render_topology 가 인덱스
    # 정본 위에 덮어썼는데 갱신 소유자가 없어 vLLM 0.18.0 METADATA 에 얼어붙었다(서브 0.27.0 빌드 사망).
    # 여기 단언은 존재·비어있지않음·비실행뿐이라 **내용 신선도**를 볼 수 없었다 — 존재 단언은 갱신되지 않는
    # 사본을 살려두는 근거가 되기도 한다. 소유자는 per-topology output/<t>/requirements.txt 하나다.
    build_assets = [
        build_asset_root / "Dockerfile.source-build-upstage",
        *[build_asset_root / "model_inputs/configs" / f"{stem}.{suffix}"
          for stem in ("exaone45-33b", "hy3") for suffix in ("sh", "yaml")],
        *[build_asset_root / "model_inputs/envs" / f".env.{stem}"
          for stem in ("exaone45-33b", "hy3")],
    ]
    checks.append({"name": "upstream_owner_build_plane_assets",
                   "ok": all(p.is_file() and not p.is_symlink() and p.stat().st_size > 0
                             and stat.S_IMODE(p.stat().st_mode) & 0o111 == 0 for p in build_assets),
                   "paths": [str(p.relative_to(REPO)) for p in build_assets]})
    # 모델구동 런타임 패치는 policy:RUNTIME_PATCH_NO_CARRY_FORWARD.C1/C2 가 "휘발(volatile)·비추적 성격이며
    # 환경 또는 **버전 bump 마다 재유도**한다"고 규정한 산출물이다. 그러므로 특정 모델 stem 의 패치가
    # *존재한다*를 배포 불변식으로 단언하면 정책과 정면 모순한다 — bump 직후 정본이 비어 있는 것은
    # 규정된 정상 상태이기 때문이다(2026-07-30 vLLM 0.26.0 bump 에서 실제로 표면화: 옛 단언이 C2 준수를
    # 자산 손실로 오판해 FAIL 했다).
    # ∴ 여기서 단언하는 것은 존재가 아니라 **있을 때의 정합성**이다:
    #   (a) <stem>_patch.py 에는 짝 provenance 사이드카가 있어야 하고(고아 금지 — 낡은 권위 배달 차단),
    #   (b) 양쪽 모두 정규파일·심링크아님·비어있지않음·비실행이어야 한다.
    # 배달 허용 stem 의 allowlist 는 여전히 sync_to_sub.sh 의 BAND2_RUNTIME_PATCH_STEMS 가 소유한다
    # (허용목록 ≠ 존재단언 — 이 둘의 혼동이 원래 모순의 원인이었다).
    patch_root = build_asset_root / "runtime_patches"
    patch_defects: list[str] = []

    def _well_formed(p: Path) -> bool:
        return (p.is_file() and not p.is_symlink() and p.stat().st_size > 0
                and stat.S_IMODE(p.stat().st_mode) & 0o111 == 0)

    if patch_root.is_dir():
        for py in sorted(patch_root.glob("*_patch.py")):
            sidecar = py.with_name(py.name[: -len(".py")] + ".provenance.json")
            if not sidecar.is_file():
                patch_defects.append(f"{py.relative_to(REPO)}:missing-provenance")
                continue
            patch_defects += [str(p.relative_to(REPO)) + ":malformed"
                              for p in (py, sidecar) if not _well_formed(p)]
        for sidecar in sorted(patch_root.glob("*_patch.provenance.json")):
            py = sidecar.with_name(sidecar.name[: -len(".provenance.json")] + ".py")
            if not py.is_file():
                patch_defects.append(f"{sidecar.relative_to(REPO)}:orphan-provenance")
    checks.append({"name": "upstream_owner_runtime_patch_pairing",
                   "ok": not patch_defects, "defects": patch_defects,
                   "present": sorted(p.name for p in patch_root.glob("*_patch.py"))
                              if patch_root.is_dir() else []})
    provenance = REPO / ".claude/policies/provenance/plan_26062818_RouteB_jasl-fork_SM12x_DeepSeek-V4-Flash_2노드서빙.md"
    checks.append({"name": "shipped_arch_approval_provenance",
                   "ok": provenance.is_file() and provenance.stat().st_size > 0})
    terraforming_contract = REPO / ".claude/skills/terraforming_node/SKILL.md"
    skeleton_text = "\n".join(p.read_text(encoding="utf-8") for p in skeletons if p.is_file())
    terraforming_text = terraforming_contract.read_text(encoding="utf-8")
    checks += [
        {"name": "owner_host_safety_documentation",
         "ok": ".claude/skills/terraforming_node/scripts/host_safety/" in terraforming_text
               and "호스트 안전체계(레포 루트 `scripts/`)" not in terraforming_text},
        {"name": "owner_doc_naming_documentation",
         "ok": "`scripts/doc_naming.py`" not in skeleton_text
               and ".claude/skills/wiki-desk/scripts/doc_naming.py" in skeleton_text},
    ]

    checks += [
        _run("terraform_scan_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/scan_node.py", "--self-test"], {0}),
        _run("terraform_render_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/render_sub_env.py", "--self-test"], {0}),
        _run("runtime_regression_selftest", [*_child_python(),
             ".claude/policies/runtime/runtime_selftest.py"], {0}),
        _run("gitless_hint_match", [sys.executable,
             ".claude/skills/upstream-version-watch/scripts/hint_tag.py", "match",
             "--vllm", "0.24.0", "--model", "deepseek-v4-flash", "--arch", "gb10"],
             {0}, env_overrides={"PATH": "/nonexistent"}),
    ]
    for filename in ("resolve_torch_pin.py", "resolve_ngc_tag.py", "resolve_build_track.py",
                     "resolve_wheel.py", "render_dockerfile.py", "regen_requirements.py",
                     "classify_failure.py", "check_smoke_model.py"):
        checks.append(_run(f"upstream_help:{filename}", [sys.executable,
                           f".claude/skills/upstream-version-watch/scripts/{filename}", "--help"], {0}))
    # info-only(미테라포밍) 게이트 2건은 **조걜부**다: exit 4 는 Flag 미발급 환경에서만 발화하므로,
    # Flag 발급이 완료된 배포 레포에서는 구조적으로 통과할 수 없다(2026-07-30 F2 교정 — 두 검사는 본래
    # fresh-clone 하네스 소유인데 로컬 배포 검증기에 놓여 영구 FAIL 하고 있었다).
    # ∴ Flag 발급 완료 시 not-applicable 로 보고하고, 미발급(fresh clone)에서만 exit 4 를 단언한다.
    # fresh-clone 상태에서의 실제 실행 검사는 smoke_clone.sh A8 이 소유한다(보호 약화 아님).
    if _terraform_flag_issued():
        checks += [
            {"name": "recipe_info_only_gate", "ok": True,
             "not_applicable": "terraform Flag 발급 완료 — exit 4 게이트는 미테라포밍 환경에서만 발화. "
                               "fresh-clone 실행 검사는 smoke_clone.sh A8 소유"},
            {"name": "benchmark_info_only_gate", "ok": True,
             "not_applicable": "terraform Flag 발급 완료 — exit 4 게이트는 미테라포밍 환경에서만 발화. "
                               "fresh-clone 실행 검사는 smoke_clone.sh A8 소유"},
        ]
    else:
        checks += [
            _run("recipe_info_only_gate", [sys.executable,
                 ".claude/skills/vllm-recipe-explorer/recipe.py", "estimate", "--auto"], {4}),
            _run("benchmark_info_only_gate", ["bash",
                 ".claude/skills/adversarial-benchmark/scripts/run_bench.sh", "freshclone-probe"], {4}),
        ]
    checks += [
        _run("benchmark_verdict_fixture", [sys.executable,
             ".claude/skills/adversarial-benchmark/scripts/verdict_rule.py", "--measured",
             ".claude/skills/adversarial-benchmark/fixtures/measured_pass.json", "--roofline",
             ".claude/skills/adversarial-benchmark/fixtures/roofline_sample.json"], {0}),
    ]

    with tempfile.TemporaryDirectory(prefix="easy-vllm-wiki-distribution.") as wiki:
        checks += [
            _run("wiki_init", [sys.executable, ".claude/skills/wiki-desk/scripts/init_wiki_desk.py",
                 "--project-root", str(REPO), "--wiki-root", wiki, "--answers",
                 ".claude/skills/wiki-desk/fixtures/project_init_answers.yaml"], {0}),
            _run("wiki_lint", [sys.executable, ".claude/skills/wiki-desk/scripts/lint_wiki.py",
                 "--project-root", str(REPO), "--wiki-root", wiki], {0}),
            _run("wiki_query_negative_honesty", [sys.executable,
                 ".claude/skills/wiki-desk/scripts/smoke_query.py", "--wiki-root", wiki,
                 "--query", "terraforming manifest"], {0, 2}),
        ]

    policy_runner = REPO / ".claude/policies/runtime/policy_registry.py"
    if policy_runner.is_file():
        checks.append(_run("policy_registry_verify", [sys.executable, str(policy_runner),
                           "verify", "--as-of", "2026-07-27", "--repo-root", str(REPO)], {0}))
    else:
        checks.append({"name": "policy_registry_verify", "ok": False,
                       "error": "missing .claude/policies/runtime/policy_registry.py"})

    predicate_dir = REPO / ".claude/policies/predicates"
    if predicate_dir.is_dir():
        checks.append(_run("production_claim_predicates", [sys.executable,
                           str(predicate_dir / "claim_predicates.py")], {0}))
        checks.append(_run("production_companion_predicates", [sys.executable, "-m", "unittest",
                           "discover", "-s", str(predicate_dir), "-p", "*_predicate.py"], {0}))
    else:
        checks.append({"name": "production_claim_predicates", "ok": False,
                       "error": "missing .claude/policies/predicates"})

    return {"schema_version": 1, "repo": str(REPO),
            "verdict": "PASS" if all(c.get("ok") for c in checks) else "FAIL",
            "checks": checks}


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json-out")
    args = ap.parse_args(argv)
    result = verify()
    text = json.dumps(result, ensure_ascii=False, indent=2)
    print(text)
    if args.json_out:
        Path(args.json_out).write_text(text + "\n", encoding="utf-8")
    return 0 if result["verdict"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
