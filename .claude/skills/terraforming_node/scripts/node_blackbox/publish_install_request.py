#!/usr/bin/env python3
"""publish_install_request.py — 노드블랙박스 설치 절차를 `docs/request/` 수행지시서로 발행한다.

  근거 `plan_26081716`. 발의 계기는 **배선 부재**였다 — 규칙(`.claude/rules/docs.md` §request)은
  "에이전트 실행평면 밖에서만 완수되는 과업은 request 로 사람에게 위임한다"고 이미 말하고 있는데,
  이 스킬에는 그 발행을 수행하는 코드가 0개였다(2026-08-17 실측: SKILL.md 언급 0 · 스크립트 0 ·
  실재 문서 0). 재료(레벨별 명령·검증기·회수물)는 전부 있었고 **발행 스텝만 없었다.**

  ★ 왜 채팅이 아니라 문서인가(plan §2.3): L3 는 **재부팅 1회**를 요구하는데 에이전트는 그 호스트
    위에서 돌고 있다. 재부팅과 함께 세션이 소멸하고 컨텍스트(어느 레벨까지 했는지·무엇을 검증할지·
    회수물이 무엇인지)가 함께 사라진다. **문서는 디스크에 남아 재부팅을 생존한다.** 멀티노드면
    사람이 두 노드를 오가야 하므로 이득이 두 배다.

  ★ 발행 범위 = L3 포함 시만(plan D1-b). 재부팅이 없으면 세션이 죽지 않아 문서의 존재 이유가
    약하고, §2.6 의 "강제·차단·반복 잔소리 금지" 톤과도 맞다. L1/L2 만 필요하면 채팅 안내로 족하다.
    그래도 내겠다면 `--force` — 침묵하지 않고 문서에 그 사실을 적는다.

  ★ 시각은 `--generated-utc` 주입 전용이다(벽시계 금지 — `staleness_gate.py --max-age-days` 선례).
    `doc_naming.kst_tokens` 는 파싱 실패를 'NA' 센티넬로 fail-soft 하는 순수 라이브러리이므로,
    **필수 인자로서의 거부는 이 호출부의 책임**이다(그 docstring 이 명시).

  ★ R2(정본 이중화) 대응: 명령 문자열의 정본은 설치자/검증기 스크립트다. 이 발행기는 명령을 손으로
    베끼지 않고 **경로와 인터페이스를 실물 대조한 뒤 조립**한다 — 스크립트가 사라지거나 `--level`·
    `--apply` 를 더 이상 받지 않으면 발행이 fail-closed 로 죽는다(조용히 낡은 지시서를 내지 않는다).

  사용:
    python3 publish_install_request.py --generated-utc 2026-08-18T05:00:00Z --level L3
    python3 publish_install_request.py --generated-utc <UTC> --level L3 --topology multi
    python3 publish_install_request.py --generated-utc <UTC> --level L1 --force

  종료코드: 0=발행 · 1=인자/전제 오류 · 2=명명 충돌 소진 · 3=대상 스크립트 인터페이스 불일치.
"""
import argparse
import os
import sys

_SDIR = os.path.dirname(os.path.abspath(__file__))


def _die(code, *lines):
    """문서화한 종료코드를 실제로 집행한다.

    `SystemExit("문자열")` 은 메시지를 stderr 로 내보내되 **항상 1 로 끝난다** — 계약에
    2·3 을 적어놓고 그 경로로 죽으면 호출부가 구분할 수 없다(2026-08-18 자기검증에서 발견).
    """
    for line in lines:
        print(line, file=sys.stderr)
    raise SystemExit(code)


def _find_repo(start):
    """리포 루트를 마커로 찾는다 — 고정 상대깊이로 세지 않는다.

    메인에서는 이 파일이 .claude/skills/terraforming_node/scripts/node_blackbox/ 에 있지만
    서브에는 .claude/runtime/node_blackbox/ 로 배달된다(install_node_blackbox.sh 와 동일 사유).
    깊이가 다르므로 ../../../../.. 는 서브에서 엉뚱한 곳을 가리킨다.
    """
    d = start
    while d and d != os.path.sep:
        if os.path.isdir(os.path.join(d, ".claude")) and os.path.isdir(os.path.join(d, "docs")):
            return d
        d = os.path.dirname(d)
    return None


# doc_naming 은 명명 SSOT 다. 각자 파일명을 조립하지 않는다(.claude/rules/docs.md §명명 SSOT).
def _load_doc_naming(repo):
    path = os.path.join(repo, ".claude", "skills", "wiki-desk", "scripts")
    if path not in sys.path:
        sys.path.insert(0, path)
    try:
        import doc_naming  # noqa: E402
    except ImportError as exc:  # 부재는 조용히 넘기지 않는다 — 배달 결손이다
        raise SystemExit(
            "[bb-request] FAIL: doc_naming 을 불러오지 못했다 (%s). 기대 경로: %s" % (exc, path)
        )
    return doc_naming


LEVELS = ("L1", "L2", "L3")

# 설치자/검증기의 인터페이스 계약. 값은 "그 파일 안에 반드시 있어야 하는 리터럴"이며,
# 없으면 발행을 멈춘다 — 낡은 지시서를 내느니 발행 실패가 낫다(R2 tripwire).
_CONTRACT = {
    "install_node_blackbox.sh": ("--apply", "--level", "L1", "L2", "L3"),
    "verify_node_blackbox.sh": ("--check", "--post-crash"),
}


def _assert_contract(sdir):
    """대상 스크립트의 실재와 인터페이스를 실물 대조한다."""
    missing = []
    for fname, needles in _CONTRACT.items():
        fpath = os.path.join(sdir, fname)
        if not os.path.isfile(fpath):
            missing.append("%s 부재" % fname)
            continue
        with open(fpath, encoding="utf-8", errors="replace") as fh:
            body = fh.read()
        for needle in needles:
            if needle not in body:
                missing.append("%s 에 %r 없음" % (fname, needle))
    if missing:
        _die(3,
             "[bb-request] FAIL(3): 설치자/검증기 인터페이스가 이 발행기의 전제와 다르다 — "
             + " · ".join(missing),
             "[bb-request]        지시서의 명령 정본은 그 스크립트다. 발행기를 먼저 맞춰라.")


# ── 레벨별 본문 조각 ────────────────────────────────────────────────────────────
_LEVEL_DESC = {
    "L1": ("재부팅 불필요",
           "수집기(1초 샘플) · ETA 워치독 · 이벤트 통합 · 로그 수명 집행 · drop-caches · earlyoom"),
    "L2": ("재부팅 불필요 (peer 필요)",
           "netconsole 교차 스트리밍 — **멀티노드 전용**(단일은 `N/A` 로 정직 기록)"),
    "L3": ("**재부팅 1회**",
           "사후 포착 = efi_pstore 확보 (crashkernel·ramoops **제거**)"),
}

_INSTALLER = ".claude/skills/terraforming_node/scripts/node_blackbox/install_node_blackbox.sh"
_VERIFIER = ".claude/skills/terraforming_node/scripts/node_blackbox/verify_node_blackbox.sh"
_SUB_INSTALLER = ".claude/runtime/node_blackbox/install_node_blackbox.sh"


def _levels_upto(level):
    return LEVELS[: LEVELS.index(level) + 1]


def _render(level, topology, forced, generated_utc, basename):
    levels = _levels_upto(level)
    multi = topology == "multi"
    has_l3 = "L3" in levels
    nodes = ["메인", "서브"] if multi else ["이 노드"]

    L = []
    add = L.append

    add("# %s" % basename[: -len(".md")].replace("_", " ", 1))
    add("")
    add("> **수행 주체: 사람**. 에이전트는 무인 `sudo` 를 실행하지 않는다(`terraforming_node`"
        " SKILL.md §2.6 · 헌법 §안전 경계).")
    add("> **발행 시각(UTC)**: `%s` · **대상 레벨**: `%s` · **토폴로지**: `%s`"
        % (generated_utc, " → ".join(levels), topology))
    add("> **발행 근거**: `plan_26081716` · **명명**: `doc_naming.dated_doc_basename`(결정론)")
    if forced:
        add(">")
        add("> ⚠ **`--force` 발행분** — 이 레벨 조합에는 재부팅이 없어 원래는 채팅 안내로 족하다"
            "(`plan_26081716` D1-b). 요청에 따라 문서로 낸다.")
    add("")
    add("---")
    add("")

    # ── ① 전제·준비물 ─────────────────────────────────────────────
    add("## 1. 전제 · 준비물")
    add("")
    add("| 항목 | 요구 |")
    add("|---|---|")
    add("| 권한 | 대상 노드의 `sudo` (암호 입력 가능한 세션) |")
    add("| 위치 | 리포 루트에서 실행 — `cd <레포 절대경로>` |")
    if has_l3:
        add("| 재부팅 | **1회 필요**(L3). 서빙 중이면 먼저 내려라 — 「부록 B」 참고 |")
    if multi:
        add("| 노드 | **메인·서브 각각** 수행. 메인이 서브의 `sudo` 를 대행하지 않는다(A2A 경계) |")
        add("| 서브 경로 | 서브에서는 `%s` (런타임 배달분) |" % _SUB_INSTALLER)
    add("| 사전 확인 | 이 문서가 가리키는 스크립트가 실재하는지 — 발행 시점에 대조됨 |")
    add("")
    add("**이번에 활성화되는 것**")
    add("")
    add("| 레벨 | 재부팅 | 무엇을 |")
    add("|---|---|---|")
    for lv in levels:
        reboot, what = _LEVEL_DESC[lv]
        add("| **%s** | %s | %s |" % (lv, reboot, what))
    add("")
    if "L2" in levels and not multi:
        add("> ℹ **L2 는 단일노드에서 `N/A` 로 기록된다** — peer 가 없으면 netconsole 이 성립하지"
            " 않는다. 이는 실패가 아니라 정직한 부재 기록이다.")
        add("")

    # ── ② 단계별 명령 + ③ 성공 판정 + ④ 실패 분기 ───────────────
    add("---")
    add("")
    add("## 2. 수행 절차")
    add("")
    add("> 각 단계는 **명령 → 성공 판정 → 실패 시** 순서다. 판정을 통과하지 못하면 다음 단계로"
        " 넘어가지 말고 §4 로 가라.")
    add("")

    step = 0
    for node in nodes:
        if multi:
            add("### %s 노드" % node)
            add("")
        installer = _SUB_INSTALLER if (multi and node == "서브") else _INSTALLER
        for lv in levels:
            step += 1
            add("#### 단계 %d — %s 적용 (%s)" % (step, lv, node))
            add("")
            add("```bash")
            add("# 먼저 dry-run 으로 무엇을·왜 바꾸는지 확인한다 (기본이 dry-run)")
            add("sudo bash %s --level %s" % (installer, lv))
            add("")
            add("# 확인했으면 적용")
            add("sudo bash %s --apply --level %s" % (installer, lv))
            add("```")
            add("")
            add("- **성공 판정**: 종료코드 `0`. dry-run 출력의 변경 목록과 적용 결과가 일치.")
            if lv == "L3":
                add("- ⚠ **여기서 재부팅이 필요하다.** 적용 직후 아래를 실행한다:")
                add("")
                add("  ```bash")
                add("  sudo reboot")
                add("  ```")
                add("")
                add("  **재부팅하면 이 작업을 지시한 에이전트 세션은 사라진다.** 돌아온 뒤에는"
                    " 이 문서의 §3 체크박스를 보고 이어서 진행하면 된다.")
            add("- **실패 시**: 종료코드 `1`=전제 실패 · `2`=설치/검증 실패 → §4")
            add("")

        step += 1
        verifier = (_SUB_INSTALLER.replace("install_node_blackbox.sh", "verify_node_blackbox.sh")
                    if (multi and node == "서브") else _VERIFIER)
        add("#### 단계 %d — 검증 (%s)" % (step, node))
        add("")
        add("```bash")
        add("bash %s --check" % verifier)
        add("```")
        add("")
        add("- **성공 판정**: 종료코드 `0`(전항 통과). `pending` 항목은 정상 —"
            " 크래시가 있어야 증명되는 항목이다.")
        add("- **실패 시**: 종료코드 `2`=검사 실패 → §4")
        if has_l3:
            add("")
            add("> 🔬 **선택 — 사후 포착 증명**: `--check` 는 *준비 상태*만 본다. 실제 포착을"
                " 증명하려면 `--crash-test`(★파괴적★ 강제 커널 패닉 → 재부팅) 후"
                " `--post-crash` 로 확정한다. **일상 설치에는 불필요하다** — 증명이 필요할 때만.")
        add("")

    # ── 진행 체크박스 ─────────────────────────────────────────────
    add("---")
    add("")
    add("## 3. 진행 체크박스 — 재부팅을 건너 살아남는 상태")
    add("")
    add("> 단순 명령 나열은 재부팅을 **텍스트로만** 생존하지 **상태로는** 생존하지 못한다."
        " 한 단계 끝낼 때마다 `[ ]` 를 `[x]` 로 바꿔라. 재부팅 후 이 절만 보면 어디부터인지"
        " 자명해진다(새 에이전트 세션에 이 문서를 물려줘도 같다).")
    add("")
    for node in nodes:
        if multi:
            add("**%s 노드**" % node)
            add("")
        for lv in levels:
            mark = " → **재부팅 필요** ← 여기서 세션이 끊긴다" if lv == "L3" else ""
            add("- [ ] %s 적용%s" % (lv, mark))
        if has_l3:
            add("- [ ] 재부팅 완료")
        add("- [ ] `verify_node_blackbox.sh --check` 통과")
        add("")
    add("- [ ] 회수물 2종 제출 (§5)")
    add("- [ ] 에이전트에게 완료 통지")
    add("")

    # ── ④ 실패 시 분기 ────────────────────────────────────────────
    add("---")
    add("")
    add("## 4. 실패 시 분기")
    add("")
    add("| 증상 | 종료코드 | 처방 |")
    add("|---|---|---|")
    add("| 설치자가 전제 실패로 멈춤 | `1` | 출력이 지목한 부재물을 먼저 해소."
        " `node_identity.sh` 부재면 배달 결손이다 |")
    add("| 설치/검증 실패 | `2` | dry-run 을 다시 돌려 무엇이 어긋났는지 대조 |")
    add("| 검증에서 `pending` 만 남음 | `0` | **정상**. 크래시 없이는 증명 불가한 항목이다 |")
    add("| 재부팅 후 상태를 모르겠다 | — | §3 체크박스 → 마지막 `[x]` 다음 단계부터 |")
    add("")
    add("> 🚫 **우회하지 마라.** 가드가 막는 것은 대개 *정상 차단*이다. 규칙대로 해소하고,"
        " 정말 배선이 없다고 판단되면 그 사실을 회수물에 적어 에이전트에게 돌려보내라"
        " (`.claude/rules/workflow.md` §막힘 3분류).")
    add("")

    # ── ⑤ 회수물 ─────────────────────────────────────────────────
    add("---")
    add("")
    add("## 5. 회수물 — 이것이 돌아와야 완결이다")
    add("")
    add("수행이 끝나면 아래를 **그 자리에 둔 채** 에이전트에게 \"완료\"만 알리면 된다."
        " 에이전트가 직접 읽는다(채팅에 붙여넣지 않아도 된다).")
    add("")
    add("| # | 회수물 | 경로 | 무엇을 증명하나 |")
    add("|---|---|---|---|")
    add("| 1 | 검증 출력 | 터미널 출력 또는 리다이렉트 파일 | 각 항목의 pass/fail/pending |")
    add("| 2 | proof-of-capture 판정 | `docs/logs/<node_id>/capture_verified.json` |"
        " 상태 권위 — `installed:true` 가 아니라 **실제 포착 증명** |")
    add("")
    add("> `<node_id>` 는 `node_identity.sh` 가 해소한다(각자 파싱 금지)."
        " 멀티노드면 **노드마다 각각** 생성된다.")
    add("")
    add("> ⚠ **이 문서는 evidence chain 밖이다** — request 는 판정도 계측도 아니라"
        " `completion_gate` 를 타지 않는다(`.claude/rules/docs.md` §request)."
        " 회수물이 돌아오면 그때 testlog/devlog 로 chain 에 편입한다.")
    add("")

    # ── ⑥ 예상 소요·비용 ─────────────────────────────────────────
    add("---")
    add("")
    add("## 6. 예상 소요 · 비용")
    add("")
    add("| 항목 | 값 |")
    add("|---|---|")
    n_nodes = len(nodes)
    add("| 명령 실행 | 노드당 수 분 (레벨 %d개 × %d노드)" % (len(levels), n_nodes) + " |")
    if has_l3:
        add("| 재부팅 | 노드당 1회 — 머신 부팅 시간에 종속 |")
        add("| 서빙 중단 | **필요** — 재부팅 전 서빙을 내려야 한다 |")
    else:
        add("| 재부팅 | **없음** |")
        add("| 서빙 중단 | 불요 |")
    add("| 디스크 | 로그 평면 노드당 상한 기본 512 MiB (`logs_lifecycle.py` 가 집행) |")
    if has_l3:
        add("| RAM 회수 | crashkernel 예약 **해제** — 약 2.25 GiB 반환 |")
    add("")

    add("---")
    add("")
    add("## 7. 참조")
    add("")
    add("- `plan_26081716` — 이 발행 배선의 근거")
    add("- `.claude/rules/docs.md` §request(7번째 산문형) · §명명 SSOT")
    add("- `.claude/skills/terraforming_node/SKILL.md` §2.6 — 호스트 안전체계 Y/N 선택조항")
    add("- `%s` — 설치 명령의 **정본**(이 문서는 여기서 조립됐다)" % _INSTALLER)
    add("- `%s` — 검증 명령의 정본" % _VERIFIER)
    add("")
    return "\n".join(L) + "\n"


def main(argv=None):
    ap = argparse.ArgumentParser(
        description="노드블랙박스 설치 절차를 docs/request/ 수행지시서로 발행한다.")
    ap.add_argument("--generated-utc", required=True,
                    help="발행 시각 'YYYY-MM-DDTHH:MM:SSZ' (주입 전용 — 벽시계 금지)")
    ap.add_argument("--level", default="L3", choices=LEVELS,
                    help="목표 레벨(누적 적용: L3 이면 L1→L2→L3). 기본 L3")
    ap.add_argument("--topology", default=None, choices=("single", "multi"),
                    help="미지정 시 manifest 에서 읽는다")
    ap.add_argument("--force", action="store_true",
                    help="L3 미포함이어도 발행(plan D1-b 예외 — 문서에 사실을 적는다)")
    ap.add_argument("--repo", default=None, help="리포 루트(기본: 마커 탐색)")
    ap.add_argument("--out-dir", default=None, help="출력 디렉터리(기본: <repo>/docs/request)")
    ap.add_argument("--print-path-only", action="store_true",
                    help="발행 후 경로만 stdout 으로(스크립트 소비용)")
    a = ap.parse_args(argv)

    repo = a.repo or _find_repo(_SDIR)
    if not repo:
        raise SystemExit("[bb-request] FAIL(1): 리포 루트를 찾지 못했다(.claude + docs 마커).")

    doc_naming = _load_doc_naming(repo)

    # 시각: 순수 라이브러리는 fail-soft 하므로 **필수 인자로서의 거부는 여기서** 한다.
    yymmddhh, _mm, _ss = doc_naming.kst_tokens(a.generated_utc)
    if yymmddhh == "NA":
        raise SystemExit(
            "[bb-request] FAIL(1): --generated-utc 가 'YYYY-MM-DDTHH:MM:SSZ' 형식이 아니다: %r\n"
            "[bb-request]        예: 2026-08-18T05:00:00Z (마이크로초·오프셋 표기 불가)"
            % (a.generated_utc,))

    _assert_contract(_SDIR)

    topology = a.topology
    if topology is None:
        topology = _read_topology(repo)

    levels = _levels_upto(a.level)
    if "L3" not in levels and not a.force:
        raise SystemExit(
            "[bb-request] 중단(1): 레벨 %s 에는 재부팅이 없어 지시서를 발행하지 않는다"
            " (plan_26081716 D1-b — 세션이 죽지 않으면 문서의 존재 이유가 약하고 §2.6 의"
            " '잔소리 금지' 톤과도 맞다).\n"
            "[bb-request]        채팅 안내로 진행하거나, 정말 문서가 필요하면 --force." % a.level)

    out_dir = a.out_dir or os.path.join(repo, "docs", "request")
    os.makedirs(out_dir, exist_ok=True)
    existing = [n for n in os.listdir(out_dir) if n.endswith(".md")]

    topic = "노드블랙박스_%s_설치_수행절차" % a.level
    try:
        basename = doc_naming.dated_doc_basename("request", a.generated_utc, topic, existing)
    except doc_naming.NamingCollisionExhausted as exc:
        _die(2, "[bb-request] FAIL(2): %s" % exc)

    path = os.path.join(out_dir, basename)
    body = _render(a.level, topology, a.force, a.generated_utc, basename)
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(body)

    rel = os.path.relpath(path, repo)
    if a.print_path_only:
        print(rel)
    else:
        print("[bb-request] 발행: %s" % rel)
        print("[bb-request]   레벨 %s · 토폴로지 %s%s"
              % (" → ".join(levels), topology, " · --force" if a.force else ""))
        print("[bb-request]   회수물: verify 출력 · docs/logs/<node_id>/capture_verified.json")
    return 0


def _read_topology(repo):
    """topology 해소는 `manifest_contract.resolve_topology` **단일 소유**에 위임한다.

    ★ 2026-08-18 자기검증에서 잡힌 결함: 처음엔 여기서 `output/{single,multi}/manifest.yaml` 을
      순회해 **먼저 찾은 쪽**을 채택했다. 그런데 두 통로의 manifest 는 **동시에 존재하는 것이
      정상**이고 둘 다 `complete: true` 일 수 있다 — 그래서 multi-node 체크아웃에서 `single` 을
      조용히 돌려줬다(침묵 폴백 · 노드 수가 달라지는 **틀린 지시서**를 낳는다).

      정본은 브랜치를 *통로 선택자*로 쓰고 불명이면 `None` 을 돌려 호출자가 fail-closed 하게
      한다. 같은 개념을 두 곳에 손으로 적은 것이 원인이므로(4종 안티패턴 §매직넘버/중복),
      구현을 지우고 정본을 부른다.
    """
    path = os.path.join(repo, ".claude", "skills", "terraforming_node", "scripts")
    if path not in sys.path:
        sys.path.insert(0, path)
    try:
        import manifest_contract
    except ImportError as exc:
        _die(1, "[bb-request] FAIL(1): manifest_contract 를 불러오지 못했다 (%s). 기대 경로: %s"
                % (exc, path))
    topo = manifest_contract.resolve_topology(repo, None)
    if topo not in ("single", "multi"):
        _die(1,
             "[bb-request] FAIL(1): topology 를 해소하지 못했다(브랜치 불명).",
             "[bb-request]        --topology single|multi 로 명시하라 — 추측하지 않는다.")
    return topo


if __name__ == "__main__":
    sys.exit(main())
