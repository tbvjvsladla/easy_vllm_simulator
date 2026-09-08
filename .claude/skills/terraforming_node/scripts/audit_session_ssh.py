#!/usr/bin/env python3
"""audit_session_ssh.py — 세션 transcript 의 **무단 서브 스캔** 감사 (술어 A1).

왜 있는가(2026-09-08 · plan_26090813 §4.7 · camp-26090721 사후감사 F3):
    헌법 노드제어 ① 은 *"메인은 서브를 무단 스캔·직접 교정하지 않는다 — 상향 회수는 문서기반"*
    이라고 말한다. 그런데 그 규칙을 **집행하는 것이 아무것도 없었다**. 2026-09-07 캠페인에서
    메인은 서브를 `ssh` 로 32회 직접 관측했다(파일 내용 읽기 12 · 프로세스 조회 11 · 존재 확인 6
    · 폴링 2). 규칙은 있었고, 그것을 어겼는지 **묻는 자리가 없었다**.

    직접 원인은 규율이 아니라 **채널 부재**였다: attempt 사이에 허가된 진행 관측면이 없었다.
    그 채널은 `sub.campaign.brief` 로 신설됐고(§2.7.7), 이 감사는 그 채널이 실제로 쓰였는지를
    **사후에** 묻는다. 채널을 만들고 감사하지 않으면 다음 사람은 다시 ssh 를 연다.

무엇이 허가인가:
    ssh 자체가 금지가 아니다 — **허가된 스크립트를 통한 ssh** 는 계약이다(sync·회수·릴레이는
    ssh 로 돈다). 금지는 그 스크립트 **밖에서** 세션이 직접 여는 ssh 다. 그래서 이 감사는
    "명령 문자열이 허가 스크립트를 부르는가" 를 묻고, 그렇지 않은 채 서브를 향하면 위반이다.

    허가 스크립트: relay.py · agent_control.py · sync_to_sub.sh · fetch_sub_docs.sh ·
                   library_relay.py · multinode_*_smoke.sh · scan_node.py(승인된 온보딩 스캔)

  사용:  audit_session_ssh.py --transcript <session.jsonl> --sub-host spark-xxxx
  종료:  0 = 위반 0 · 1 = 위반 있음 · 2 = 입력 오류
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

# 허가 통로. **닫힌 목록**이고 새 오케스트레이션 스크립트가 생기면 여기 분류를 강제한다
# (workflow.md §4종 안티패턴 판정표의 '하드코딩 정당' 칸 = tripwire).
SANCTIONED = (
    "relay.py",
    "agent_control.py",
    "sync_to_sub.sh",
    "fetch_sub_docs.sh",
    "library_relay.py",
    "multinode_comms_smoke.sh",
    "multinode_serve_smoke.sh",
    "scan_node.py",
)
# ★ `ssh` 를 **실행하는 자리**만 잡는다. 종전 초안은 문자열에 `ssh` 가 들어 있기만 해도 걸었고,
#   그 결과 "지난 세션의 ssh 를 세는 분석 스크립트" 가 위반으로 잡혔다 — 가드가 틀린 이유로
#   발화한 것이다(이 저장소가 여러 번 만난 형태). 명령 위치 = 문장 시작 또는 셸 구분자 뒤이며,
#   앞에 sudo·timeout·env 대입이 붙는 것까지만 허용한다.
SSH_RE = re.compile(
    r"(?:^|[;&|(\n`]|\$\()\s*"
    r"(?:sudo\s+|timeout\s+\S+\s+|[A-Za-z_][A-Za-z0-9_]*=\S*\s+)*"
    r"ssh\b")


def commands(transcript: Path):
    """transcript 에서 셸 명령 문자열만 뽑는다. 형식 변화에 관대하게 — 이 감사가 포맷 하나
    때문에 죽으면 그 순간 감사가 없는 것과 같다(빈 결과를 '위반 0' 으로 접지 않는다)."""
    seen = 0
    for line in transcript.read_text(encoding="utf-8", errors="replace").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            row = json.loads(line)
        except ValueError:
            continue
        stack = [row]
        while stack:
            node = stack.pop()
            if isinstance(node, dict):
                if node.get("type") == "tool_use" and isinstance(node.get("input"), dict):
                    cmd = node["input"].get("command")
                    if isinstance(cmd, str):
                        seen += 1
                        yield cmd, node.get("name")
                stack.extend(node.values())
            elif isinstance(node, list):
                stack.extend(node)
    if seen == 0:
        raise SystemExit(f"[ssh-audit] FAIL: transcript 에서 명령을 하나도 읽지 못했다 — "
                         f"{transcript}. 판독 실패를 '위반 0' 으로 접지 않는다(형식 확인 필요).")


def violations(transcript: Path, sub_host: str) -> list:
    out = []
    for cmd, tool in commands(transcript):
        if not SSH_RE.search(cmd):
            continue
        if sub_host and sub_host not in cmd:
            continue                       # 서브를 향하지 않는 ssh 는 이 감사의 대상이 아니다
        if any(s in cmd for s in SANCTIONED):
            continue                       # 허가 통로를 통한 ssh 는 계약이다
        out.append({"tool": tool, "command": " ".join(cmd.split())[:220]})
    return out


def _selftest() -> int:
    """판별기가 **실행하는 ssh** 와 **말하는 ssh** 를 가르는지 시험한다. 음성대조가 없으면
    이 감사는 위반을 세는 대신 단어를 센다."""
    ok = True

    def ck(label: str, cond: bool) -> None:
        nonlocal ok
        print(("  PASS " if cond else "  FAIL ") + label)
        ok = ok and bool(cond)

    hit = lambda c: bool(SSH_RE.search(c))
    ck("문장 첫 자리의 ssh 는 실행이다", hit("ssh user@node 'ls'"))
    ck("구분자 뒤의 ssh 도 실행이다",
       hit("echo x; ssh h 'ls'") and hit("a && ssh h 'ls'") and hit("a | ssh h 'ls'"))
    ck("timeout·env 접두가 붙어도 실행이다",
       hit("timeout 30 ssh h 'ls'") and hit("FOO=1 ssh h 'ls'"))
    ck("명령치환 안의 ssh 도 실행이다", hit("X=$(ssh h 'ls')"))
    ck("★음성대조: 산문·문자열 안의 ssh 는 실행이 아니다",
       not hit('print("=== ssh to sub ===")') and not hit("grep -c 'ssh' file")
       and not hit("audit_session_ssh.py --transcript x"))
    ck("★음성대조: 경로 조각의 ssh 는 실행이 아니다",
       not hit("cat ~/.ssh/config") and not hit("ls /usr/bin/sshd"))
    print("[ssh-audit] self-test " + ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


def main(argv: list | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--transcript", help="세션 transcript(.jsonl)")
    ap.add_argument("--sub-host", default="",
                    help="서브 호스트(문자열 포함 검사). manifest nodes[role=sub].host 와 같은 값")
    ap.add_argument("--limit", type=int, default=12, help="출력할 위반 수(전수는 개수로 센다)")
    a = ap.parse_args(argv)
    if a.self_test:
        return _selftest()
    if not (a.transcript and a.sub_host):
        print("[ssh-audit] FAIL: --transcript 와 --sub-host 는 필수다(--self-test 는 예외)",
              file=sys.stderr)
        return 2
    path = Path(a.transcript)
    if not path.is_file():
        print(f"[ssh-audit] FAIL: transcript 부재 — {path}", file=sys.stderr)
        return 2
    bad = violations(path, a.sub_host)
    if not bad:
        print(f"[ssh-audit] PASS — 허가 통로 밖 ssh 0건 ({path.name} · sub={a.sub_host})")
        return 0
    print(f"[ssh-audit] FAIL — 허가 통로 밖 ssh {len(bad)}건 "
          f"({path.name} · sub={a.sub_host}). 상향 회수는 문서기반이다(헌법 노드제어 ①).",
          file=sys.stderr)
    for row in bad[:a.limit]:
        print(f"  - [{row['tool']}] {row['command']}", file=sys.stderr)
    if len(bad) > a.limit:
        print(f"  … 그 외 {len(bad) - a.limit}건", file=sys.stderr)
    print("  → 진행 관측이 필요하면 `sub.campaign.brief`(회수 미러의 campaign_brief.json)를 "
          "읽어라. 채널이 있는데도 ssh 를 열면 그것은 우회다(D3).", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
