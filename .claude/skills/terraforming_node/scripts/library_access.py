#!/usr/bin/env python3
"""library_access.py — 서브 wiki-desk 의 서고 접근점(ssh 강제명령 게이트웨이)을 발급·회수한다.

왜(2026-10-19 · plan_26101912 D3·D4·D11): 서고(`__llm-wiki`)는 메인에 하나이고 접근점은 N개다. 서브 wiki-desk 와 서고의
관계는 메인과 같고 **경로만 다르다** — 그 경로가 manifest `library` 접근자다. 멀티노드 설치는 이미 메인↔서브 ssh 를
열어 두므로 그 채널을 쓰되, 서브가 얻는 것은 셸이 아니라 **사서 명령 하나**다:

    command="<node> <repo>/.claude/skills/wiki-desk/src/gateway.ts --node <node_id>",restrict <서브 전용 공개키>

node_id 는 이 줄(메인이 키에 묶은 값)이 정한다 — 서브는 자기 node_id 를 주장할 수 없다.

무엇을 하는가(`--apply` · 사람 승인 전사 `--approved-by` 필수 — 메인 authorized_keys 변경은 매번 HITL):
  1. 사전검사 — 메인: node>=24 · flock · rsync · rrsync · 서고 계약 / 서브: node>=24 · rsync. 부재는 **차단 사유 보고**(자동 설치 ✗).
  2. 서브 전용 키 — 서브에서 생성한다(개인키는 서브를 떠나지 않는다). 이미 있으면 재사용한다.
  3. 메인 호스트 키 → 서브 전용 known_hosts(TOFU ✗ — 메인이 자기 호스트 키를 직접 건넨다).
  4. 메인 `~/.ssh/authorized_keys` 에 강제명령 줄 1개(마커 `wiki-desk-gateway:<node_id>`로 멱등 교체).
  5. manifest `library` 블록 — 메인(transport: local + access_points) · 서브 발급본 `sub_manifest.yaml`(transport: ssh).
`--revoke` 는 4 를 지우고 access_points 에서 뺀다(노드 해제·재설치 때만 · HITL). 주기 회전은 없다(D11).

가상 접근점(`--host-of <실노드>` · 2026-10-19 plan_26101913 S1): 실노드의 호스트·사용자에 node_id 가 다른 **별도 키**를
발급한다(한 호스트에 사서 여럿 — N-writer 실측용). manifest 에는 `access_points.<id>.host_of` 로 출처가 남고, 서브 발급본은
건드리지 않는다(그 노드의 접근자는 그대로다). 가상 사서가 쓸 library 블록은 출력 `virtual_library_block` 으로 건넨다.
`--revoke` 는 가상 키 파일도 그 호스트에서 지운다.

  사용:  library_access.py --topology single --node sub --plan
         library_access.py --topology single --node sub2 --host-of sub --apply --approved-by "<전사>"
         library_access.py --topology single --node sub --apply --approved-by "<사람 발화 전사>"
         library_access.py --topology single --node sub --revoke --approved-by "<전사>"
         library_access.py --self-test
  종료:  0 = 성공 · 2 = 입력 오류 · 3 = 차단(사전검사 실패 · 승인 부재)
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]
GATEWAY_REL = ".claude/skills/wiki-desk/src/gateway.ts"
NODE_MAJOR_MIN = 24
KEY_NAME = "wiki_desk_ed25519"
KNOWN_HOSTS_NAME = "wiki_desk_known_hosts"
MARKER = "wiki-desk-gateway:"
HOST_KEY = Path("/etc/ssh/ssh_host_ed25519_key.pub")
# 다른 위임 전송과 같은 경화(프롬프트 원천 차단) — providers/claude_code.py SSH_HARDENING 과 같은 계약
SSH_HARDENING = ("-F", "/dev/null", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8",
                 "-o", "StrictHostKeyChecking=yes", "-o", "IdentitiesOnly=no", "-o", "ForwardAgent=no",
                 "-o", "ClearAllForwardings=yes", "-o", "RequestTTY=no")
EXIT_USAGE, EXIT_BLOCKED = 2, 3


class Blocked(Exception):
    """사전검사·승인이 성립하지 않는다 — 우회하지 않고 사유를 보고한다."""


def _manifest_path(topology: str) -> Path:
    return REPO / "output" / topology / "manifest.yaml"


def _load_yaml(path: Path) -> dict:
    import yaml  # 계약 리더와 같은 로더(manifest_contract 가 이미 pyyaml 을 요구한다)
    with open(path, encoding="utf-8") as f:
        return yaml.safe_load(f) or {}


def _nodes(man: dict, node_id: str) -> tuple[dict, dict]:
    nodes = man.get("nodes") or []
    main = next((n for n in nodes if n.get("role") == "main"), None)
    subs = [n for n in nodes if n.get("role") == "sub"]
    sub = next((n for n in subs if (n.get("node_id") or "sub") == node_id), None)
    if sub is None and node_id == "sub" and len(subs) == 1:
        sub = subs[0]
    if main is None or sub is None:
        raise Blocked(f"manifest nodes 에서 main/sub({node_id}) 를 찾지 못했다")
    for n, who in ((main, "main"), (sub, node_id)):
        for k in ("host", "ssh_user", "work_dir"):
            if not n.get(k):
                raise Blocked(f"manifest nodes[{who}].{k} 가 비었다")
    return main, sub


def _node_major(version: str) -> int | None:
    m = re.match(r"v?(\d+)\.", version.strip())
    return int(m.group(1)) if m else None


def _ssh(sub: dict, remote: str, stdin: str | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(["ssh", *SSH_HARDENING, f"{sub['ssh_user']}@{sub['host']}", remote],
                          input=stdin, capture_output=True, text=True, timeout=60)


def preflight(sub: dict) -> dict:
    """양쪽 사전검사. 부재는 차단 사유로 모아 한꺼번에 보고한다(하나 고치고 다음 것에 또 막히지 않게)."""
    reasons, facts = [], {}
    node = shutil.which("node")
    facts["main_node"] = node
    if node is None:
        reasons.append("메인에 node 가 없다(>= %d 필요)" % NODE_MAJOR_MIN)
    else:
        v = subprocess.run([node, "--version"], capture_output=True, text=True).stdout
        if (_node_major(v) or 0) < NODE_MAJOR_MIN:
            reasons.append(f"메인 node 가 낮다: {v.strip()} (>= {NODE_MAJOR_MIN})")
    for tool in ("flock", "rsync", "rrsync"):
        if shutil.which(tool) is None:
            reasons.append(f"메인에 {tool} 가 없다(동시성·반입 규정 집행 불가)")
    if not (REPO / "__llm-wiki" / "contract.json").is_file():
        reasons.append("메인 서고가 초기화되지 않았다(__llm-wiki/contract.json 부재 · wiki-desk init 선행)")
    r = _ssh(sub, "bash -lc 'command -v node >/dev/null && node --version; command -v rsync >/dev/null && echo rsync-ok; echo HOME=$HOME'")
    if r.returncode != 0:
        reasons.append(f"서브에 ssh 로 닿지 못했다(rc={r.returncode}): {r.stderr.strip()[-200:]}")
    else:
        out = r.stdout
        m = re.search(r"^(v\d+\.\S+)$", out, re.M)
        facts["sub_node_version"] = m.group(1) if m else None
        if m is None or (_node_major(m.group(1)) or 0) < NODE_MAJOR_MIN:
            reasons.append(f"서브 node 가 없거나 낮다({facts['sub_node_version']}) — Node.js >= {NODE_MAJOR_MIN} 를 사람이 설치한다(자동 설치 ✗)")
        if "rsync-ok" not in out:
            reasons.append("서브에 rsync 가 없다(반입 불가)")
        h = re.search(r"^HOME=(/\S+)$", out, re.M)
        facts["sub_home"] = h.group(1) if h else None
        if facts["sub_home"] is None:
            reasons.append("서브 HOME 을 읽지 못했다")
    facts["blocked"] = reasons
    return facts


def gateway_line(node_bin: str, node_id: str, pubkey: str) -> str:
    """authorized_keys 한 줄 — 셸 메타문자를 담을 수 없는 값만 싣는다(node_id·경로는 닫힌 문법)."""
    if not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,62}", node_id) or node_id == "main":
        raise Blocked(f"node_id 형식 오류: {node_id!r}")
    parts = pubkey.split()
    if len(parts) < 2 or parts[0] != "ssh-ed25519":
        raise Blocked("서브 공개키가 ed25519 한 줄이 아니다")
    cmd = f"{node_bin} {REPO / GATEWAY_REL} --node {node_id}"
    if '"' in cmd or "\\" in cmd:
        raise Blocked("게이트웨이 명령 경로에 인용 문자가 있다")
    return f'command="{cmd}",restrict {parts[0]} {parts[1]} {MARKER}{node_id}'


def replace_marked(text: str, node_id: str, line: str | None) -> str:
    """마커가 같은 줄을 지우고(있으면) 새 줄을 붙인다. line=None 이면 회수(지우기만)."""
    kept = [ln for ln in text.splitlines() if not ln.rstrip().endswith(MARKER + node_id)]
    if line is not None:
        kept.append(line)
    return "\n".join(kept) + ("\n" if kept else "")


def replace_top_block(text: str, key: str, block: str | None) -> str:
    """YAML 최상위 `key:` 블록(다음 최상위 키 전까지)을 지우고 block 을 끝에 붙인다 — 주석·다른 키는 건드리지 않는다."""
    lines = text.splitlines()
    out, skipping = [], False
    for ln in lines:
        if re.match(rf"^{re.escape(key)}:", ln):
            skipping = True
            continue
        if skipping and (ln.startswith((" ", "\t")) or ln.strip() == ""):
            continue
        skipping = False
        out.append(ln)
    while out and out[-1].strip() == "":
        out.pop()
    body = "\n".join(out) + "\n"
    return body + ("\n" + block if block else "")


def key_name(node_id: str, virtual: bool) -> str:
    """실노드는 고정 이름(재발급 시 재사용) · 가상 접근점은 node_id 별 이름(같은 호스트의 키끼리 섞이지 않는다)."""
    return f"wiki_desk_{node_id}_ed25519" if virtual else KEY_NAME


def main_library_block(main: dict, access_points: dict) -> str:
    lines = ["library:   # 서고 접근자(wiki-desk) — 메인은 local · 서브 접근점은 library_access.py 가 발급한다(plan_26101912)",
             "  transport: local", "  wiki_dir: __llm-wiki", f"  repo_root: {json.dumps(main['work_dir'])}"]
    if access_points:
        lines.append("  access_points:")
        for nid in sorted(access_points):
            ap = access_points[nid]
            lines += [f"    {nid}:", f"      identity_file: {json.dumps(ap['identity_file'])}",
                      f"      known_hosts: {json.dumps(ap['known_hosts'])}"]
            if ap.get("host_of"):
                lines.append(f"      host_of: {ap['host_of']}")
    return "\n".join(lines) + "\n"


def sub_library_block(main: dict, node_id: str, ap: dict) -> str:
    return "\n".join([
        "library:   # 서고 접근자 — 메인 서고에 ssh 강제명령 게이트웨이로 닿는다(경로만 다름 · 메인이 발급 · plan_26101912)",
        "  transport: ssh", f"  host: {json.dumps(main['host'])}", f"  user: {json.dumps(main['ssh_user'])}",
        f"  repo_root: {json.dumps(main['work_dir'])}", "  wiki_dir: __llm-wiki",
        f"  identity_file: {json.dumps(ap['identity_file'])}", f"  known_hosts: {json.dumps(ap['known_hosts'])}",
        f"  node_id: {node_id}"]) + "\n"


def run(args) -> int:
    man_path = _manifest_path(args.topology)
    man = _load_yaml(man_path)
    aps = dict(((man.get("library") or {}).get("access_points") or {}))
    # 회수는 --host-of 없이도 manifest 의 출처(host_of)로 가상 접근점을 알아본다
    host_of = args.host_of or ((aps.get(args.node) or {}).get("host_of") if args.revoke else None)
    virtual = bool(host_of)
    if virtual and host_of == args.node:
        raise Blocked("--host-of 는 다른 실노드여야 한다")
    main, sub = _nodes(man, host_of or args.node)
    sub_man_path = man_path.parent / "sub_manifest.yaml"
    if args.plan:
        facts = preflight(sub)
        print(json.dumps({"mode": "plan", "node_id": args.node, "preflight": facts,
                          "will": ["서브 전용 키 생성(없으면) · 공개키 회수", "메인 호스트 키 → 서브 known_hosts",
                                   f"메인 authorized_keys 강제명령 줄({MARKER}{args.node})",
                                   f"manifest library 블록: {man_path.relative_to(REPO)} · {sub_man_path.relative_to(REPO)}"]},
                         ensure_ascii=False, indent=2))
        return EXIT_BLOCKED if facts["blocked"] else 0
    if not (args.approved_by or "").strip():
        raise Blocked("메인 authorized_keys 변경은 사람 승인 전사(--approved-by)가 있어야 한다(HITL · D11)")
    ak = Path.home() / ".ssh" / "authorized_keys"
    if args.revoke:
        if ak.is_file():
            ak.write_text(replace_marked(ak.read_text(encoding="utf-8"), args.node, None), encoding="utf-8")
        gone = aps.pop(args.node, None) or {}
        if virtual:
            kf = gone.get("identity_file") or f"~/.ssh/{key_name(args.node, True)}"
            r = _ssh(sub, f"rm -f {shlex.quote(kf)} {shlex.quote(kf + '.pub')}")
            if r.returncode != 0:
                raise Blocked(f"가상 접근점 키 삭제 실패: {r.stderr.strip()[-200:]}")
        man_path.write_text(replace_top_block(man_path.read_text(encoding="utf-8"), "library", main_library_block(main, aps)), encoding="utf-8")
        print(json.dumps({"mode": "revoke", "node_id": args.node, "approved_by": args.approved_by}, ensure_ascii=False))
        return 0
    facts = preflight(sub)
    if facts["blocked"]:
        raise Blocked("; ".join(facts["blocked"]))
    home = facts["sub_home"]
    key = f"{home}/.ssh/{key_name(args.node, virtual)}"
    r = _ssh(sub, f"umask 077; mkdir -p ~/.ssh; test -f {shlex.quote(key)} || ssh-keygen -q -t ed25519 -N '' "
                  f"-C {shlex.quote(MARKER + args.node)} -f {shlex.quote(key)}; cat {shlex.quote(key + '.pub')}")
    if r.returncode != 0:
        raise Blocked(f"서브 키 생성·회수 실패: {r.stderr.strip()[-200:]}")
    pub = r.stdout.strip().splitlines()[-1]
    if not HOST_KEY.is_file():
        raise Blocked(f"메인 호스트 키가 없다: {HOST_KEY}")
    hk = HOST_KEY.read_text(encoding="utf-8").split()
    kh = f"{home}/.ssh/{KNOWN_HOSTS_NAME}"
    # 같은 파일을 두 토폴로지(서로 다른 메인 주소)가 함께 쓴다 — 그 host 줄만 바꾸고 나머지는 보존한다
    q = shlex.quote(kh)
    r = _ssh(sub, f"umask 077; touch {q}; grep -v -F {shlex.quote(main['host'] + ' ')} {q} > {q}.tmp || true; "
                  f"cat >> {q}.tmp; mv {q}.tmp {q}", stdin=f"{main['host']} {hk[0]} {hk[1]}\n")
    if r.returncode != 0:
        raise Blocked(f"서브 known_hosts 기록 실패: {r.stderr.strip()[-200:]}")
    line = gateway_line(facts["main_node"], args.node, pub)
    ak.parent.mkdir(mode=0o700, exist_ok=True)
    before = ak.read_text(encoding="utf-8") if ak.is_file() else ""
    ak.write_text(replace_marked(before, args.node, line), encoding="utf-8")
    os.chmod(ak, 0o600)
    aps[args.node] = {"identity_file": key, "known_hosts": kh, **({"host_of": host_of} if virtual else {})}
    man_path.write_text(replace_top_block(man_path.read_text(encoding="utf-8"), "library", main_library_block(main, aps)), encoding="utf-8")
    if virtual:
        print(json.dumps({"mode": "apply", "node_id": args.node, "host_of": host_of, "approved_by": args.approved_by,
                          "authorized_keys_marker": MARKER + args.node,
                          "virtual_library_block": sub_library_block(main, args.node, aps[args.node])}, ensure_ascii=False, indent=2))
        return 0
    if sub_man_path.is_file():
        sub_man_path.write_text(replace_top_block(sub_man_path.read_text(encoding="utf-8"), "library",
                                                  sub_library_block(main, args.node, aps[args.node])), encoding="utf-8")
    print(json.dumps({"mode": "apply", "node_id": args.node, "approved_by": args.approved_by, "preflight": facts,
                      "authorized_keys_marker": MARKER + args.node, "sub_manifest_updated": sub_man_path.is_file(),
                      "next": "render_sub_env.py --sub-manifest + sync_to_sub.sh --provision 로 서브 manifest·wiki-desk 를 배달한다"},
                     ensure_ascii=False, indent=2))
    return 0


def _self_test() -> int:
    ok = True

    def chk(c, what):
        nonlocal ok
        print(("  [PASS] " if c else "  [FAIL] ") + what)
        ok = ok and bool(c)
    pub = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIexample host"
    line = gateway_line("/usr/bin/node", "sub", pub)
    chk(line.startswith('command="/usr/bin/node ') and ",restrict ssh-ed25519 " in line and line.endswith(MARKER + "sub"),
        "강제명령 줄 = command + restrict + 마커")
    chk(" --node sub\"" in line, "node_id 는 메인이 줄에 묶는다")
    for bad in ("main", "Sub", "a;rm", ""):
        try:
            gateway_line("/usr/bin/node", bad, pub)
            chk(False, f"잘못된 node_id 거부: {bad!r}")
        except Blocked:
            chk(True, f"잘못된 node_id 거부: {bad!r}")
    try:
        gateway_line("/usr/bin/node", "sub", "ssh-rsa AAAA x")
        chk(False, "ed25519 외 키 거부")
    except Blocked:
        chk(True, "ed25519 외 키 거부")
    ak = "ssh-ed25519 AAAA me@main\n" + 'command="x",restrict ssh-ed25519 OLD wiki-desk-gateway:sub\n'
    new = replace_marked(ak, "sub", line)
    chk(new.count(MARKER + "sub") == 1 and "OLD" not in new and "me@main" in new, "같은 마커 줄만 교체(멱등) · 남의 줄 보존")
    chk(MARKER + "sub" not in replace_marked(new, "sub", None) and "me@main" in replace_marked(new, "sub", None), "회수 = 그 줄만 삭제")
    y = "self_role: main\n# 주석\nlibrary:\n  transport: old\n  wiki_dir: x\nnodes:\n  - role: main\n"
    out = replace_top_block(y, "library", "library:\n  transport: local\n")
    chk("transport: old" not in out and "# 주석" in out and "nodes:\n  - role: main" in out and out.rstrip().endswith("transport: local"),
        "manifest library 블록만 교체(주석·다른 키 보존)")
    blk = sub_library_block({"host": "203.0.113.10", "ssh_user": "u", "work_dir": "/w/r"}, "sub",
                            {"identity_file": "/h/.ssh/k", "known_hosts": "/h/.ssh/kh"})
    with tempfile.TemporaryDirectory() as tmp:
        p = Path(tmp) / "m.yaml"
        p.write_text(blk, encoding="utf-8")
        import yaml
        lib = yaml.safe_load(p.read_text(encoding="utf-8"))["library"]
    chk(lib == {"transport": "ssh", "host": "203.0.113.10", "user": "u", "repo_root": "/w/r", "wiki_dir": "__llm-wiki",
                "identity_file": "/h/.ssh/k", "known_hosts": "/h/.ssh/kh", "node_id": "sub"},
        "서브 library 블록 = wiki-desk accessor.ts 스키마(transport·host·user·repo_root·wiki_dir·identity_file·known_hosts·node_id)")
    # 사전검사 음성대조: 서브에 node 가 없으면(또는 낮으면) 차단 사유가 실린다 — 자동 설치 ✗(실 ssh 대신 응답을 주입한다)
    global _ssh
    real = _ssh
    try:
        for out, want in (("rsync-ok\nHOME=/h\n", "서브 node 가 없거나 낮다"),
                          ("v22.1.0\nrsync-ok\nHOME=/h\n", "서브 node 가 없거나 낮다"),
                          ("v26.7.0\nHOME=/h\n", "서브에 rsync 가 없다")):
            _ssh = lambda sub, remote, stdin=None, _o=out: subprocess.CompletedProcess([], 0, _o, "")
            facts = preflight({"ssh_user": "u", "host": "h"})
            chk(any(want in r for r in facts["blocked"]), f"사전검사 차단 사유: {want} ({out.split()[0]})")
        _ssh = lambda sub, remote, stdin=None: subprocess.CompletedProcess([], 0, "v26.7.0\nrsync-ok\nHOME=/h\n", "")
        chk(not [r for r in preflight({"ssh_user": "u", "host": "h"})["blocked"] if "서브" in r], "서브 조건 충족 → 서브 쪽 차단 사유 없음")
    finally:
        _ssh = real
    chk(key_name("sub", False) == KEY_NAME and key_name("sub2", True) == "wiki_desk_sub2_ed25519", "가상 접근점 키 이름 = node_id 별")
    mb = main_library_block({"work_dir": "/w/r"}, {"sub": {"identity_file": "/k", "known_hosts": "/kh"},
                                                    "sub2": {"identity_file": "/k2", "known_hosts": "/kh", "host_of": "sub"}})
    chk("    sub2:\n" in mb and "      host_of: sub\n" in mb and mb.count("host_of") == 1, "가상 접근점은 host_of 출처를 남긴다(실노드는 없음)")
    print("[library_access] self-test " + ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--topology", choices=("single", "multi"))
    ap.add_argument("--node", default="sub")
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--plan", action="store_true")
    g.add_argument("--apply", action="store_true")
    g.add_argument("--revoke", action="store_true")
    g.add_argument("--self-test", action="store_true")
    ap.add_argument("--approved-by")
    ap.add_argument("--host-of", help="가상 접근점: 이 실노드의 호스트에 --node 로 별도 키를 발급한다")
    args = ap.parse_args(argv)
    if args.self_test:
        return _self_test()
    if not args.topology or not (args.plan or args.apply or args.revoke):
        ap.print_usage(sys.stderr)
        return EXIT_USAGE
    try:
        return run(args)
    except Blocked as exc:
        print(f"[library_access] BLOCKED: {exc}", file=sys.stderr)
        return EXIT_BLOCKED


if __name__ == "__main__":
    raise SystemExit(main())
