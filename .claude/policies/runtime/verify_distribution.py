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
    # hint 발행 sidecar(2026-08-20 · plan_26082009). 상태를 전이하지 않으므로 spine 이 아니라
    # wiki-desk 와 같은 sidecar 다. 승격 근거: 발행 조건 A/B 가 explorer·benchmark 소유라
    # 엔진만 upstream 에 세들어 있던 배치 오류였고, Contributor 진입점이 없었다.
    "hint-publisher",
    "terraforming_node",
    "upstream-version-watch",
    "vllm-recipe-explorer",
    "wiki-desk",
}
TRUST_FILES = (
    ".claude/policies/registry.yaml",
    ".claude/policies/claim_bindings.json",
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
    ".claude/skills/hint-publisher/scripts/hint_tag.py",
    ".claude/skills/terraforming_node/scripts/host_safety/host/vllm-drop-caches.sh",
    ".claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh",
    ".claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh",
    ".claude/policies/runtime/policy_registry.py",
    ".claude/policies/runtime/providers/claude_code.py",
    ".claude/skills/upstream-version-watch/scripts/smoke_clone.sh",
    ".claude/skills/upstream-version-watch/scripts/sync_branches.sh",
    ".claude/skills/terraforming_node/scripts/host_safety/systemd/easy-vllm-memwatch.service",
    ".claude/skills/hint-publisher/templates/hint_recipe.template.md",
}
SUB_TOMBSTONES = {
    ".claude/rules/references.md", "scripts/install_host_safety.sh",
    "scripts/mem_watchdog.sh", "scripts/host/vllm-drop-caches.sh",
    "scripts/systemd/easy-vllm-memwatch.service", "scripts/smoke_clone.sh",
    "scripts/sync_branches.sh",
    # 2026-09-06: 루트 `tasks/` 가 `campaigns/<id>/relay/` 로 대체되면서 서브에 남은 스캐폴드
    #   마커를 은퇴시켰다(커밋 3263339). 오버레이는 **가산**이라 tombstone 없이는 서브에
    #   영구 잔존한다. 여기 목록을 같이 올리지 않아 이 검사가 그 커밋 이후 계속 FAIL 이었다.
    "tasks/.gitkeep",
    # 2026-09-08(커밋 f7ec026): 메인의 해소 결과는 싱글 서브에 가지 않는다 — 남으면 서브가
    #   자기 HW 로 해소할 이유가 없어진다. 위 `tasks/.gitkeep` 주석이 경고한 그대로 **이 목록을
    #   같이 올리지 않아** 이 검사가 그 커밋 이후 계속 FAIL 이었다(같은 실패의 두 번째 발현).
    ".claude/skills/upstream-version-watch/assets/current-production-resolution.json",
}
SUB_RELOCATION_TOMBSTONES = {
    ".claude/rules/references.md", "scripts/install_host_safety.sh",
    "scripts/mem_watchdog.sh", "scripts/host/vllm-drop-caches.sh",
    "scripts/systemd/easy-vllm-memwatch.service",
    # 릴레이 원장 정본이 campaigns/<camp-id>/relay/ 로 이관(2026-09-06). 대체 자리가 실재하므로
    # 은퇴가 아니다 — 은퇴로 두면 활성 소비자 감사가 서브의 살아 있는 원장에 걸려 배달이 막힌다.
    "tasks/.gitkeep",
}
SUB_RETIREMENT_TOMBSTONES = {
    "scripts/smoke_clone.sh", "scripts/sync_branches.sh",
    # 대체 자리가 없는 삭제 = 은퇴(이관 ✗). 서브는 자기 HW 로 스스로 해소한다.
    ".claude/skills/upstream-version-watch/assets/current-production-resolution.json",
}


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
        # ── 앵커 이설 (2026-09-03 · F-1f) ────────────────────────────────────────
        # 옛 중간 앵커 둘은 **삭제 대상이던 git-대-git blob 비교의 FAIL 메시지 리터럴**이었다
        # (`source→destination Git object/mode mismatch` · `materialized replacement byte/mode
        #  mismatch`). 그 두 비교는 git 이 방금 checkout 으로 만든 바이트를 다시 옮겨적어 대조하는
        # 중복층이라 걷어냈고(sync_branches.sh), 그러면서 앵커가 함께 사라지면 이 배포검증이
        # 즉시 ok=False 가 된다 — 그래서 **삭제 전에** 앵커를 삭제되지 않는 코드 토큰으로 옮겼다.
        # 새 앵커가 지키는 불변식은 그대로다: checkout → (커버리지·존재·모드 검증) → tombstone 삭제.
        local_order = ("git checkout ", "covered=0", 'source_meta="$(git ls-tree',
                       'materialized_mode="$(stat -c', "git rm --ignore-unmatch")
        checks += [
            {"name": "local_exact_relocation_tombstones",
             "ok": local_actual == LOCAL_TOMBSTONES,
             "actual": sorted(local_actual), "expected": sorted(LOCAL_TOMBSTONES)},
            {"name": "local_exact_relocation_replacements",
             "ok": replacements == LOCAL_REPLACEMENTS and len(replacements) == len(local_actual),
             "actual": sorted(replacements), "expected": sorted(LOCAL_REPLACEMENTS)},
            # 종료 앵커만 메시지 문자열이다 — `git rm --ignore-unmatch` 뒤로 **코드가 없다**
            # (파일 끝이 안내 echo 5줄). 유일 출현이라 구간 상한이 판정을 좌우하지도 않는다.
            _order_check("local_replacement_integrity_before_tombstone", local_text,
                         'git checkout "$SRC_BRANCH" -- "${PATHS[@]}"',
                         "echo \"[sync-branches] 완료", local_order),
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
            _ordered_between_detail(sub_text, 'begin_remote_transaction "$t" 0',
                                    'deliver_build "$t" 0',
                                    ('begin_remote_transaction "$t" 0', 'git checkout -q $t')),
            _ordered_between_detail(sub_text, 'PROVISION" != "1"',
                                    "if [ $HAS_GIT = 0 ]; then",
                                    ('begin_remote_transaction "${BOOTSTRAP_POPULATE:-${TARGETS[0]}}" 1',
                                     "sub_run_mk")),
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
            # ── 앵커 규약: **구간 앵커는 코드 토큰만 쓴다** ─────────────────────────────
            # (2026-08-15 처방 B → 2026-08-16 전 체크로 확대 · `request_26081521_01_04`)
            # 옛 앵커 "# 서브를 기본 운용 브랜치" 는 주석이었고, 2026-08-13 배달경로 교정이
            # 그 주석을 다시 쓰면서 소실됐다 — 주석은 문서 교정 때 자유롭게 바뀌므로 앵커로
            # 부적합하다. 처방 B 는 깨진 한 곳(`REST_BRANCH=`)만 고쳤는데, **쌍둥이 체크의
            # 앵커 4개도 같은 주석 계열이었다**(그때는 우연히 생존해 PASS 였을 뿐 — F7).
            # 살아 있다는 이유로 두면 다음 리팩터 때 같은 사고가 난다.
            # 코드 토큰도 바뀔 수는 있다. 다만 바뀐다는 것은 **배달 로직이 바뀌었다**는 뜻이라
            # 그때는 판정기도 함께 리뷰돼야 하는 것이 맞다 — 이것이 주석과의 결정적 차이다.
            #   B0 구간: `deliver_build multi 0`(부트스트랩 Band2 배달 시작)
            #            → `REMOTES="$(sub_run`(배달·검증·tombstone·커밋 뒤 origin 부재 확증)
            #   B1 구간: `deliver_build "$t" 0`(증분 배달 시작) → `REST_BRANCH=`(복귀 브랜치 결정)
            # ⚠ 시작 앵커가 **첫 순서토큰 자신인** 두 체크(`local_…`·`sub_transaction_…`)는
            #   그 토큰이 구간 밖으로 밀리면 `token_out_of_order` 가 아니라 `token_absent` 로
            #   보고된다(구간이 앵커에서 시작하므로 앞쪽을 볼 수 없다). 빨간불은 정확히 켜지고
            #   지목 토큰도 맞지만, 사유 문자열만으로 두 경우를 가르지는 못한다 — 2026-08-16 실측.
            _order_check("sub_bootstrap_replacement_before_tombstone", sub_text,
                         "deliver_build multi 0", 'REMOTES="$(sub_run', order),
            _order_check("sub_incremental_replacement_before_tombstone", sub_text,
                         'deliver_build "$t" 0', "REST_BRANCH=", order),
            {"name": "sub_render_uses_transactional_source",
             "ok": all(token in sub_text for token in (
                 "prepare_transactional_source", "mktemp -d", "CANONICAL_SRC",
                 'SRC="$TRANSACTIONAL_SRC/"'))
             and sub_text.find("prepare_transactional_source\n")
             < sub_text.find("# ═══════════════════════ DRY-RUN")},
            # 2026-09-05(②-b): 파일시스템 예외가 manifest.yaml **하나**였을 때는 그 리터럴이 앵커였다.
            #   카드 서명키·서브 manifest 도 render 입력이 되면서 예외가 셋이 됐고, 앵커를 **닫힌 목록**
            #   자체로 옮긴다 — 목록이 늘면 이 검사가 빨간불이 되어 리뷰를 강제한다(tripwire 형 하드코딩).
            #   음성 앵커(디렉터리 통째 복사 금지)는 그대로 둔다: 예외는 파일 단위여야 한다.
            {"name": "sub_transactional_source_uses_git_index",
             "ok": ("checkout-index -z --stdin" in sub_text
                    and "filesystem bytes are excluded in favor of index authority" in sub_text
                    and "ls-files -z -- .claude CLAUDE.md .gitignore campaigns output/multi output/single"
                    in sub_text  # campaigns = 2026-09-06 신설 루트(커밋 4043ff4)
                    and ("for render_input in manifest.yaml a2a_signing/main_ed25519.pem "
                         "sub_manifest.yaml") in sub_text
                    and 'install -m 0600 "${CANONICAL_SRC}output/$topology/$render_input"' in sub_text
                    and '"${CANONICAL_SRC}output/$topology/"' not in sub_text)},
            {"name": "sub_runtime_patch_transfer_is_owner_allowlisted",
             "ok": ("BAND2_RUNTIME_PATCH_STEMS=(exaone45-33b hy3)" in sub_text
                    and "--include='/configs/*_patch.py'" not in sub_text
                    and '"$build_assets"/runtime_patches/*' in sub_text)},
            {"name": "sub_retirement_consumer_audit_before_tombstone",
             # ★ 2026-09-11 앵커 교정(plan_26091108 S3 부수): 종전 앵커는 **무인자 호출**
             #   `verify_destination_retirement_consumers ||` 를 셌는데, 그 함수는 `$1=topology`
             #   를 받도록 리팩터됐고 호출부는 `... multi ||` · `... "$t" ||` 다. 앵커가 리팩터를
             #   따라가지 못해 이 검사가 계속 FAIL 이었다(이 저장소의 반복 결함: 앵커는 리팩터를
             #   따라간다). 불변인 것은 **두 자리에서 fail-closed 로 부른다**는 사실이므로 그것을 센다.
             "ok": (len(re.findall(r"verify_destination_retirement_consumers\s+\S+\s*\|\|",
                                   sub_text)) == 2
                    and "retirement_consumer_scan" in sub_text
                    and 'owner=".claude/skills/upstream-version-watch/"+stale' in sub_text
                    and 'while lo and line[lo-1] in chars' in sub_text
                    and "scanner/transport failed for $stale" in sub_text)},
            # ── 토폴로지-aware 은퇴 (2026-09-11 · plan_26091108 후속 · 서브 클린) ──────────
            #   오버레이는 가산이라 정본에서 사라진 것이 서브에 영구 잔존한다. 종전 비석은
            #   **평면 목록**이라 "single 엔 있어야 하고 multi 엔 없어야 한다" 를 적을 자리가
            #   없었다(같은 경로가 브랜치마다 정본이기도 잔재이기도 하다).
            {"name": "sub_constitution_runtime_block_is_derived",
             # 서브 헌법이 런타임블럭을 **하드코딩**하면 tool_plane 과 갈라진다. 실측 2026-09-11:
             #   multi 헌법이 59행에서 "vllm-recipe-explorer 를 실행한다" 고 하고 95행에서
             #   "런타임블럭: 없다" 고 해 **자기모순**이었다(자동 로드되는 쪽이 틀렸다).
             "ok": (lambda t: ("{{ TOOL_PLANE_JSON }}" in t
                               and '"skills_loaded": ["vllm-recipe-explorer"]' not in t))(
                 (REPO / ".claude/skills/terraforming_node/sub_node/CLAUDE.template.md")
                 .read_text(encoding="utf-8")
                 if (REPO / ".claude/skills/terraforming_node/sub_node/CLAUDE.template.md").is_file()
                 else "")},
            {"name": "sub_topology_aware_retirement_wired",
             "ok": ("retire_runtime_block_residue()" in sub_text
                    # 대상은 손목록이 아니라 **파생**이다 — 정본이 이 토폴로지에 무엇을 주는가가 정한다.
                    and "RUNTIME_BLOCK_OWNED_ROOTS[@]" in sub_text
                    # 대량 삭제의 문 = 사람이 숫자를 말한다(ALLOW_DELETE 와 같은 idiom).
                    #   ★ 앵커는 **게이트 표현식**이다. 종전 초안은 `"RETIRE_ALLOW" in sub_text`
                    #     였는데, 그건 `RETIRE_ALLOW_REMOVED` 에도 부분일치해 음성대조가
                    #     조용히 통과했다(2026-09-11 음성대조가 자기 앵커의 결함을 잡았다).
                    and '[ "${RETIRE_ALLOW:-}" != "$n" ]' in sub_text
                    and "--retire-residue) RETIRE_RESIDUE=1" in sub_text
                    # 기본 미집행 — 잔재 삭제는 배달의 부수효과가 아니다.
                    and "RETIRE_RESIDUE=0" in sub_text
                    # 브랜치를 명시해 읽는다: DRY-RUN 은 서브를 checkout 하지 않으므로
                    #   `git ls-files` 는 **다른 브랜치**의 인덱스를 본다(첫 실행 위양성 104건).
                    #   그리고 `core.quotePath=false` — 기본값은 비-ASCII 경로를 이스케이프해
                    #   돌려주고, 그 문자열로 rm 을 부르면 조용히 아무것도 안 지운다(실측 1건 생존).
                    and "git -c core.quotePath=false ls-tree -r --name-only '$t'" in sub_text
                    # 렌더 실패를 잔재로 읽지 않는다(스테이징 0건 = fail-closed).
                    and "렌더 실패를 잔재로 읽지 않는다" in sub_text)},
            {"name": "no_active_sub_retirement_consumers",
             "ok": not _active_retirement_consumers(),
             "details": _active_retirement_consumers()},
            {"name": "sub_invocation_rollback_transaction",
             "ok": all(token in sub_text for token in (
                 # 2026-09-03(P3): 부트스트랩이 채우는 토폴로지가 타겟에 따라 갈리므로 리터럴
                 #   "multi" 가 아니다. 불변인 것은 **트랜잭션이 먼저 열린다**는 사실이다.
                 "begin_remote_transaction \"$_bs_t\" 1", 'begin_remote_transaction "$t" 0',
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
    # ── build_plane 자산의 **내용** 단언 (2026-08-18 신설 · plan_26081810) ────────────────────
    # 바로 위 단언은 존재·비심링크·비어있지않음·비실행뿐이다. 그 한계는 이미 requirements.txt 동결
    # 사고로 자백돼 있는데(윗 주석), **같은 검사 안에서 같은 실패 모드가 재발했다**: `.env.hy3` 가
    # Band2 클러스터 9키를 통째로 복제한 채(그중 노드 IP 2 + SSH 계정 1) 양 브랜치로 배포됐다.
    # 사람만이 판정 주체였기 때문이다 — `.env.exaone45-33b` 는 맞게, `.env.hy3` 는 틀리게 들어왔다.
    #
    # 여기서 단언하는 두 가지:
    #   (a) Band3 모델 env 에 **노드 정체성 키가 없다**. `.env.<model>` 은 policy:
    #       MODEL_TRIPLET_NO_SUB_PROPAGATION 의 Band3 이고, 노드 좌표의 정본은 manifest.nodes[] →
    #       render_dockerfile.py --cluster-envfile → `.env.cluster`(Band2) 다. 두 곳에 손으로 적히면
    #       workflow.md §4종 판정표의 매직넘버 **결함** 칸("같은 개념이 두 곳 이상에 손으로 적힌 값")
    #       이고, per-model env 가 cluster env 를 **이기므로**(compose env_file 순서 · smoke 의
    #       --env-file 후순 우선) 배포본을 받은 제3자의 manifest 를 조용히 덮어쓴다.
    #   (b) build_plane 자산 전체에 **배포 PII 4종이 0건**이다. docs.md §PII 적용범위 표에서
    #       `.claude/**` 추적 템플릿은 "배포 산출물 · 4종 전부" 행이다(면제는 기계생성 원시 평면뿐).
    #
    # ⚠ 금지 대상을 노드 정체성 3키로 **한정**한다. `RAY_PORT`(③불변)·`MAX_JOBS`(②프리셋)는
    #   CLUSTER_PRESETS 주석이 "모델별 override 필요시 .env.<model>(master) 에서" 로 명시 인가한
    #   경로이고 실사용 output env 13종 중 13/2 건이 그 형태다 — 함께 금지하면 정상을 사살한다.
    NODE_IDENTITY_KEYS = ("MASTER_HOST_IP", "SLAVE_HOST_IP", "SSH_USER")
    BUILD_PLANE_PII = (
        ("private-ipv4", re.compile(r"\b(?:192\.168\.|10\.\d{1,3}\.|172\.(?:1[6-9]|2\d|3[01])\.)\d{1,3}(?:\.\d{1,3})?")),
        ("email", re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")),
        ("abs-op-path", re.compile(r"/(?:mnt|home)/[A-Za-z0-9._/-]+")),
        ("spark-host", re.compile(r"spark-[0-9a-f]{3,}")),
    )
    content_defects: list[str] = []
    try:
        for p in sorted((build_asset_root / "model_inputs/envs").glob(".env.*")):
            rel = str(p.relative_to(REPO))
            for lineno, line in enumerate(p.read_text(encoding="utf-8").splitlines(), 1):
                key = line.split("=", 1)[0].strip()
                if key in NODE_IDENTITY_KEYS:
                    content_defects.append(
                        f"{rel}:{lineno} Band2 노드정체성 키 '{key}' — 정본은 manifest.nodes[] → .env.cluster")
        for p in sorted((build_asset_root / "model_inputs").rglob("*")):
            if not p.is_file():
                continue
            rel = str(p.relative_to(REPO))
            text = p.read_text(encoding="utf-8", errors="replace")
            for lineno, line in enumerate(text.splitlines(), 1):
                for name, pat in BUILD_PLANE_PII:
                    m = pat.search(line)
                    if m:
                        content_defects.append(f"{rel}:{lineno} 배포 PII {name}: {m.group(0)}")
    except (OSError, UnicodeError) as exc:
        content_defects.append(f"{type(exc).__name__}: {exc}")
    checks.append({"name": "upstream_owner_build_plane_content",
                   "ok": not content_defects,
                   "defects": content_defects})
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
    # ── 공허통과 폐쇄 (2026-09-03 · 감사 §4 "verify_distribution.py:529 상시 ok:true") ──────────
    # 초판은 `if patch_root.is_dir():` 하나로 전체 몸통을 감쌌다. 정본 디렉터리는 0.26.0 bump 때
    # C2 준수로 비워졌고(`b59f156` 에서 2쌍 추가 → 이후 회수), 그 뒤로 이 검사는 **아무 술어도
    # 평가하지 않은 채** `ok:true` 를 냈다 — 4종 안티패턴 표의 *결함* 칸(결정·게이트 경로에서
    # 원인을 삼키는 침묵 폴백)에 정확히 해당한다. 처방은 삭제가 아니라 배선이다(D3):
    #   ⓐ `subject_state` 로 **무엇을 평가했는지**를 데이터에 남긴다(§결정론 규율 출처 표시).
    #   ⓑ 정본이 비어 있어도 **항상 평가되는 술어 두 개**를 둔다:
    #      · 오배치 검사 — build_plane 어디에도 runtime_patches/ 밖의 `*_patch.py` /
    #        `*.provenance.json` 이 있으면 안 된다. sync_to_sub.sh:614 는 `runtime_patches/*` 만
    #        glob 하므로 오배치는 **조용히 배달되지 않는다**(침묵 누락).
    #      · 규정된 공상태 생존성 — "패치 0건 = 정상"이 참이려면 배달부가 그 상태를 견뎌야 한다.
    #        옛 무조건 glob 는 `install: cannot stat …/*` 로 죽었다. 그 가드(compgen -G)가 실재하는지
    #        여기서 단언한다. 가드가 사라지면 정본이 비는 순간 배달이 깨지므로 이 검사는 그때 RED 다.
    # ⚠ F-5(런타임 패치 사이드카)는 이월됐다. 그러나 이 검사의 대상은 **Band2 정본 build_plane** 이며
    #   F-5 가 다루는 output/<t>/configs 사이드카 평면이 아니다 — 이월과 무관하게 여기는 배선한다.
    patch_root = build_asset_root / "runtime_patches"
    patch_defects: list[str] = []

    def _well_formed(p: Path) -> bool:
        return (p.is_file() and not p.is_symlink() and p.stat().st_size > 0
                and stat.S_IMODE(p.stat().st_mode) & 0o111 == 0)

    pairs_checked = 0
    if patch_root.is_dir():
        for py in sorted(patch_root.glob("*_patch.py")):
            pairs_checked += 1
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
    # ⓑ-1 오배치 — 정본 디렉터리 유무와 무관하게 언제나 평가된다.
    if build_asset_root.is_dir():
        for stray in sorted(build_asset_root.rglob("*_patch.py")):
            if stray.parent != patch_root:
                patch_defects.append(f"{stray.relative_to(REPO)}:misplaced-outside-runtime_patches")
        for stray in sorted(build_asset_root.rglob("*_patch.provenance.json")):
            if stray.parent != patch_root:
                patch_defects.append(f"{stray.relative_to(REPO)}:misplaced-outside-runtime_patches")
    # ⓑ-2 규정된 공상태 생존성 — 배달부의 0건 가드가 실재하는지.
    empty_state_guard = False
    try:
        _sub_text = sub_sync.read_text(encoding="utf-8")
        empty_state_guard = ('compgen -G "$build_assets/runtime_patches/*"' in _sub_text
                             and '"$build_assets"/runtime_patches/*' in _sub_text)
    except (OSError, UnicodeError) as exc:
        patch_defects.append(f"sync_to_sub.sh:unreadable:{type(exc).__name__}")
    if not empty_state_guard:
        patch_defects.append(
            "sync_to_sub.sh:regulated-empty-guard-missing — "
            "policy:RUNTIME_PATCH_NO_CARRY_FORWARD.C2 의 '0건 = 정상' 이 배달부에서 깨진다")
    checks.append({"name": "upstream_owner_runtime_patch_pairing",
                   "ok": not patch_defects, "defects": patch_defects,
                   # 출처 표시: 이 검사가 이번 실행에서 무엇을 평가했는지를 데이터로 남긴다.
                   # 'regulated-empty' 는 공허통과가 아니라 ⓑ 두 술어만 평가했다는 뜻이다.
                   "subject_state": "evaluated" if pairs_checked else "regulated-empty",
                   "pairs_checked": pairs_checked,
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
        # 2026-09-04: hint-publisher 의 두 스크립트에는 자체검사가 **아예 없었고**, 그래서 push
        #   자격증명 배선이 통째로 빠진 것을 아무도 묻지 않았다(세션마다 `could not read Username`
        #   으로 재발). 위 terraforming 계열과 같은 결함(호출자 없는/존재하지 않는 자체검사)이다.
        _run("hint_tag_selftest", [sys.executable,
             ".claude/skills/hint-publisher/scripts/hint_tag.py", "--self-test"], {0}),
        _run("hint_collect_selftest", [sys.executable,
             ".claude/skills/hint-publisher/scripts/hint_collect.py", "--self-test"], {0}),
        _run("terraform_scan_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/scan_node.py", "--self-test"], {0}),
        _run("terraform_render_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/render_sub_env.py", "--self-test"], {0}),
        # 2026-09-03(S3 · plan_26090317 P1): SKILL.md §3 이 "회귀 고정 6종" 을 선언하는데 실행자는 위 2종
        #   뿐이었다. 특히 library_exchange 는 **헌법 불변식 B(그라운딩 누락 판정)의 유일한 기계 집행점**
        #   이고, node_role_contract 는 불변식 A(토폴로지 축)의 판정기다 — 그 둘이 깨져도 아무도 몰랐다.
        #   이 파일이 이미 recipe·benchmark 쪽에서 세 번 고친 결함(호출자 없는 자체검사 = L1 산문)과 동형.
        _run("terraform_node_role_contract_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/node_role_contract.py", "--self-test"], {0}),
        _run("terraform_library_exchange_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/library_exchange.py", "--self-test"], {0}),
        _run("terraform_staleness_gate_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/staleness_gate.py", "--self-test"], {0}),
        _run("terraform_manifest_contract_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/manifest_contract.py", "--self-test"], {0}),
        _run("antipattern_scan_selftest", [sys.executable,
             ".claude/policies/runtime/antipattern_scan.py", "--self-test"], {0}),
        _run("terraform_turn_budget_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/turn_budget.py", "--self-test"], {0}),
        _run("terraform_library_relay_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/library_relay.py", "--self-test"], {0}),
        _run("terraform_relay_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/relay.py", "--self-test"], {0}),
        _run("terraform_bootstrap_canary_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/bootstrap_canary.py", "--self-test"], {0}),
        _run("terraform_agent_guard_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/node_blackbox/agent_guard.py", "--self-test"], {0}),
        _run("terraform_node_identity_selftest", ["bash",
             ".claude/skills/terraforming_node/scripts/node_blackbox/node_identity.sh",
             "--self-test"], {0}),
        _run("runtime_regression_selftest", [*_child_python(),
             ".claude/policies/runtime/runtime_selftest.py"], {0}),
        _run("gitless_hint_match", [sys.executable,
             ".claude/skills/hint-publisher/scripts/hint_tag.py", "match",
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
    # serve 노브 파리티 — `gen_recipe_set.assert_serve_knob_parity` 의 **집행**(2026-08-16 배선).
    #   그 tripwire 는 "run_trial 이 candidate 에서 읽는 필드는 3종 세트까지 도달해야 한다"(= 검증한
    #   레시피와 배포된 레시피가 갈리지 않는다)를 지키려고 만들어졌으나 **호출자가 0 개**여서,
    #   실제 위반이 커밋과 이 검증기를 그대로 통과했다(2026-08-16 실측). 검사를 만든 것과 검사가
    #   도는 것은 다르다 — 여기서 매 검증마다 돌린다. 미테라포밍 레포에서도 순수 정적 검사라 안전하다.
    checks.append(_run("recipe_serve_knob_parity", [sys.executable,
                       ".claude/skills/vllm-recipe-explorer/scripts/gen_recipe_set.py",
                       "--check-parity"], {0}))
    checks += [
        _run("benchmark_verdict_fixture", [sys.executable,
             ".claude/skills/adversarial-benchmark/scripts/verdict_rule.py", "--measured",
             ".claude/skills/adversarial-benchmark/fixtures/measured_pass.json", "--roofline",
             ".claude/skills/adversarial-benchmark/fixtures/roofline_sample.json"], {0}),
        # 공허 PASS 게이트의 **집행** (plan_26082219 B2 · 2026-08-22 배선).
        #   `verdict_rule.py --self-test`(T1~T15) 는 "--target-tps 0 으로 PASS 를 만들 수 있는 경로가
        #   코드 어디에도 없다"를 단언한다. 그러나 **자체검사에 호출자가 없으면 그것은 L2 가 아니라
        #   L1(산문)** 이다 — 2026-08-16 실측(`gen_recipe_set --check-parity` tripwire 가 호출자 0 개라
        #   실제 위반이 커밋과 이 검증기를 그대로 통과했다)이 그 실증이다. 여기서 매 검증마다 돌린다.
        #   순수 결정론 단위검사(픽스처 파일·네트워크·서빙 불요)라 미테라포밍 레포에서도 안전하다.
        _run("benchmark_verdict_selftest", [sys.executable,
             ".claude/skills/adversarial-benchmark/scripts/verdict_rule.py", "--self-test"], {0}),
        # recipe-explorer 결정론 자체검사의 **집행** (plan_26082223 §4 · 2026-08-23 배선).
        #   같은 이유다 — 호출자 없는 자체검사는 L2 가 아니라 L1(산문)이다. 셋 다 순수
        #   결정론 단위검사(모델·NAS·docker·GPU 불요)라 미테라포밍 레포에서도 안전하다.
        #   · parse/estimate = 하이브리드 KV 층수 인지(결함 A — 262k KV 16 GiB vs 전층 64 GiB)
        #   · run_trial      = 컨테이너 생존검사(결함 C) + 호스트 바닥 gmu 캡(결함 B)
        _run("recipe_parse_config_selftest", [sys.executable,
             ".claude/skills/vllm-recipe-explorer/scripts/parse_model_config.py", "--self-test"], {0}),
        _run("recipe_estimate_vram_selftest", [sys.executable,
             ".claude/skills/vllm-recipe-explorer/scripts/estimate_vram.py", "--self-test"], {0}),
        _run("recipe_run_trial_selftest", [sys.executable,
             ".claude/skills/vllm-recipe-explorer/scripts/run_trial.py", "--self-test"], {0}),
        # 캠페인 아티팩트 체인 3종의 **집행** (2026-09-06 배선). 이 셋은 2026-09-06 에
        #   자체검사와 함께 태어났지만 **부르는 코드가 없었다** — 이 저장소가 이미 이름 붙인
        #   "호출자 없는 자체검사 = L1 산문"의 재발이다(위 sweep meta 주석과 같은 사유).
        #   무엇을 지키나: 캠페인 선언·purge 게이트·예산 선언 floor 는 캠페인 전체가 그 위에
        #   서는 계약이고, 갈라져도 라이브 캠페인 중반까지 안 보인다.
        #   ⚠ 플래그가 갈린다(`--selftest` 대 `--self-test`). 통일은 별건이며, 여기서는
        #   각자가 실제로 받는 철자를 쓴다 — 틀린 철자는 usage 로 죽어 위양성이 된다.
        _run("campaign_init_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/campaign_init.py", "--selftest"], {0}),
        _run("campaign_template_validator_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/campaign_template_validator.py",
             "--selftest"], {0}),
        _run("blackbox_session_selftest", [sys.executable,
             ".claude/skills/terraforming_node/scripts/node_blackbox/blackbox_session.py",
             "--self-test"], {0}),
        # sweep meta 추출의 **집행** (plan_26082322 §3.3 · 2026-08-23 배선). 같은 이유다 —
        #   호출자 없는 자체검사는 L2 가 아니라 L1(산문)이다.
        #   무엇을 지키나: `quantization`·`kv_cache_dtype` 를 config yaml 에서만 읽던 시절,
        #   serve-plane CLI(축 F/H)로 들어온 fp8 이 "N/A" 로 발행됐다(2026-08-23 R7). "N/A" 는
        #   사람에게 **"양자화 없음"**으로 읽히므로, PASS 였다면 거짓 계약이 인증서로 배포됐다.
        #   이 검사는 sweep_bench.sh 의 조립 heredoc 을 **그대로 뽑아 실행**하므로(파서 복제 없음)
        #   배포되는 코드 자체를 친다. 순수 정적 픽스처 — 서빙·docker·모델 불요.
        _run("benchmark_sweep_meta_selftest", [sys.executable,
             ".claude/skills/adversarial-benchmark/scripts/selftest_sweep_meta.py"], {0}),
        # Broad Search 정지 조건 평가기의 **집행** (plan_26090415 §4.7 · CP1 · 2026-09-04).
        #   같은 이유다 — 호출자 없는 자체검사는 L2 가 아니라 L1(산문)이다. 이 평가기는 예산을
        #   선언 없이 판정하지 않는 fail-closed 이고(§4.8), 그 거부 경로가 살아 있는지는 음성
        #   사례 6건이 지킨다. 순수 결정론(파일·시계·서빙 불요 — 시각은 주입만 받는다).
        _run("benchmark_sweep_stop_selftest", [sys.executable,
             ".claude/skills/adversarial-benchmark/scripts/sweep_stop.py", "--self-test"], {0}),
        # 루브릭 권한 통로의 fail-closed (plan_26090415 §1.2 · CP2 · 2026-09-04).
        #   `--authority` 를 넘기는 실행 코드가 0건이었던 것이 explore 인증서 0건의 원인이다.
        #   통로를 만들었으니 그 통로가 **권한을 발명하지 않는지**를 여기서 집행한다 —
        #   기본값이 생기는 순간 "사용자가 골랐다"와 "아무도 안 골랐다"가 다시 구분 불가가 된다.
        #   서빙·docker·모델 불요(인자 검사에서 즉시 거부되는 경로).
        #   음성·양성 **쌍**으로 둔다. 음성만 두면 기본값을 넣어도 rc 가 안 바뀌는 다른 이유
        #   (산출물 부재도 exit 2)로 통과해 가드가 틀린 이유로 초록이 된다 — 그래서 파일 전제를
        #   타지 않는 `--check-args` 로 인자 평면만 친다.
        _run("benchmark_judge_authority_required",
             ["bash", ".claude/skills/adversarial-benchmark/scripts/judge_bench.sh",
              "_probe", "--check-args"], {2}),
        _run("benchmark_judge_authority_accepted",
             ["bash", ".claude/skills/adversarial-benchmark/scripts/judge_bench.sh",
              "_probe", "--authority", "weak", "--check-args"], {0}),
        # 측정 도구 핀 해소기 (plan_26090415 §3.5 · CP3 · 2026-09-04). 순수 비교 함수 자체검사이며
        #   docker·네트워크 불요다. P8 이 **배포되는 실제 핀**을 스키마로 검사하므로 픽스처만 보고
        #   초록이 되지 않는다(픽스처가 실물보다 좁다 — 하루에 네 번 겪은 계열).
        _run("benchmark_bench_tool_pin_selftest", [sys.executable,
             ".claude/skills/adversarial-benchmark/scripts/resolve_bench_tool.py", "--self-test"], {0}),
        # GuideLLM 산출물 파서 (CP4). 실측 산출물 픽스처를 함께 검사하므로 합성 픽스처만 보고
        #   초록이 되지 않는다.
        _run("benchmark_parse_guidellm_selftest", [sys.executable,
             ".claude/skills/adversarial-benchmark/scripts/parse_guidellm.py", "--self-test"], {0}),
        # 광의의 탐색 결정론 3종 (CP6). 셀 종결 분류·지도 렌더러가 각각 음성 사례를 갖는다.
        #   특히 render_sweep_map 의 R8~R10 은 **순위 금지**를 산문이 아니라 결정론으로 지킨다 —
        #   산문으로만 적으면 다음 편집이 조용히 정렬 한 줄을 넣는다.
        _run("benchmark_classify_cell_selftest", [sys.executable,
             ".claude/skills/adversarial-benchmark/scripts/classify_cell.py", "--self-test"], {0}),
        _run("benchmark_sweep_map_selftest", [sys.executable,
             ".claude/skills/adversarial-benchmark/scripts/render_sweep_map.py", "--self-test"], {0}),
        # Broad Search 이중 게이트의 **집행**: --confirm-risk 없이는 셀이 돌지 않는다(exit 5).
        # single 컨테이너 관리 진입점의 **인자 평면** fail-closed (plan_26090419 P1 · 2026-09-04).
        #   기동 경로는 예산선언·워치독 무장을 품고 있어 잘못 불리면 무보호 로드가 된다. 여기서는
        #   docker·NAS·/proc 에 의존하지 않는 인자 검사만 친다(그 층은 어느 노드에서도 같다).
        _run("upstream_single_up_requires_config",
             ["bash", ".claude/skills/upstream-version-watch/scripts/single_serve_up.sh"], {3}),
        _run("upstream_single_up_rejects_unknown_arg",
             ["bash", ".claude/skills/upstream-version-watch/scripts/single_serve_up.sh",
              "_probe", "--bogus"], {3}),
        _run("upstream_inventory_rejects_unknown_topology",
             ["bash", ".claude/skills/upstream-version-watch/scripts/container_inventory.sh",
              "--topology", "bogus"], {2}),
        # ── plan_26091108 하네스 교정 R1~R7 의 자체검사 (2026-09-11) ───────────────────────
        #   셋 다 **역채점**(직전 캠페인 실측 회귀)과 **음성대조**(가드를 깨뜨려 빨간불)를 품는다.
        #   음성대조 없는 교정은 완료로 치지 않는다 — 이 저장소의 반복 결함이다.
        _run("upstream_budget_preflight_selftest", [sys.executable,
             ".claude/skills/upstream-version-watch/scripts/budget_preflight.py", "--self-test"], {0}),
        _run("recipe_preload_ram_gate_selftest", [sys.executable,
             ".claude/skills/vllm-recipe-explorer/scripts/preload_ram_gate.py", "--self-test"], {0}),
        _run("recipe_escalation_predicate_selftest", [sys.executable,
             ".claude/skills/vllm-recipe-explorer/scripts/escalation_predicate.py", "--self-test"], {0}),
        # R8 — 컨텍스트 확장 선언을 serve 인자로 번역한다. Y3(병합)·Y11(음성대조)이 본체다:
        #   rope 딕셔너리를 갈아끼우면 mrope·partial rotary 가 조용히 사라져 **다른 모델**이 된다.
        _run("recipe_rope_translate_selftest", [sys.executable,
             ".claude/skills/vllm-recipe-explorer/scripts/rope_scaling_translate.py",
             "--self-test"], {0}),
        _run("benchmark_broad_search_confirm_gate",
             ["bash", ".claude/skills/adversarial-benchmark/scripts/broad_search.sh", "cell",
              "--state", "/nonexistent/bs.json", "--now-utc", "2026-01-01T00:00:00Z",
              "--cell-key", "k", "--config", "c", "--axis-citation", "x",
              "--bench-budget-mib", "1"], {5}),
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
        # Pinned, never wall-clock (this project injects dates; it never reads the host clock).
        # It is therefore a tripwire, not a magic number: a policy whose added_at/last_reviewed_at
        # is newer than this date fails closed here until the date is deliberately moved forward.
        # Moved 2026-07-27 -> 2026-09-03 when GIT_SINGLE_AUTHORITY was registered (plan_26090222).
        # Moved 2026-09-03 -> 2026-09-06 when ROOT_SURFACE_REGISTRY was registered (plan_26090616).
        # ★ `--as-of` 는 **벽시계가 아니라 핀**이다(헌법: 시각은 주입만). 그래서 정책이 추가될
        #   때마다 사람이 함께 민다 — 그 편집이 리뷰를 강제하는 것이 이 하드코딩의 정당 근거다
        #   (§4종 안티패턴 판정표 '하드코딩·정당' = tripwire). 밀지 않으면 새 정책이 "미래 날짜"로
        #   읽혀 하네스가 RED 로 남는다. 2026-09-11 실측: 핀 2026-09-06 이
        #   LIBRARY_GROUNDING_FAIL_CLOSED(2026-09-08 등재)를 미래로 읽어 위반 3건이 서 있었다.
        checks.append(_run("policy_registry_verify", [sys.executable, str(policy_runner),
                           "verify", "--as-of", "2026-09-12", "--repo-root", str(REPO)], {0}))
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
