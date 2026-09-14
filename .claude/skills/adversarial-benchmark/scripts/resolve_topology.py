#!/usr/bin/env python3
# resolve_topology.py — 벤치 스크립트가 `--topology` 없이 불렸을 때 **토폴로지를 정하는 호출부 공용 해소기**
#   (2026-09-14 · plan_26091407 §9 ⑧ 분석 발견 T8 · 헌법 불변식 "토폴로지는 manifest 에서 읽고 브랜치로 추론하지 않는다")
#
# ★ 왜 있는가. 벤치 6종(broad_search·judge_bench·lite_bench·run_bench·sweep_bench·max_envelope)은 `--topology` 가 없으면
#   `git rev-parse --abbrev-ref HEAD` 의 **브랜치 이름**으로 토폴로지를 골랐고, 알 수 없는 이름은 조용히 `single` 로
#   떨어졌다. 헌법이 금지한 방향 그대로이고, 그 침묵 기본값은 두 자리에서 틀린다 —
#     ① 서브 워크스페이스의 로컬 브랜치는 `single`/`multi` 라 **항상** unknown→single 이다(싱글 서브는 우연히 맞았다).
#     ② 메인의 detached HEAD·임의 이름 브랜치는 multi 체크아웃이어도 single 통로(output/single)를 읽는다.
#   같은 관용구가 6곳에 손으로 적혀 있었다(같은 개념 여러 곳 = 4종 안티패턴 하드코딩 결함 칸). 여기 한 곳으로 모은다.
#
# ★ 이 파일은 **판정하지 않는다** — 이미 있는 두 단일 소유자를 부르고 그 답을 옮긴다(파생 복제 ✗):
#   · 서명된 `Agent_Card.json` 이 있는 노드(메인이 프로비저닝한 서브): 정체성 권위는 카드다
#     (`node_role_contract.IDENTITY_AUTHORITY_BY_TOPOLOGY` single → agent-card). 검증은 `agent_card_contract.py
#     prove-identity`(run_bench·lite_bench 게이트와 같은 두-후보 경로)가 하고, 여기서는 그 출력의 `topology` 를 읽는다.
#     그 값은 서브 자기 manifest(`output/<t>/manifest.yaml` · 메인 발급)의 `topology` 와 **같아야** 한다 — 카드는
#     정체성 권위이고 manifest 는 사실 권위라 둘이 갈리면 어느 쪽도 고르지 않는다(5).
#   · 카드가 없는 노드(메인): 4자일치 술어 `topology_parity.py evaluate` — 브랜치는 **필터**(어느 output/<t> 를 읽나)
#     이고 사실 권위는 manifest 이며 둘의 일치는 그 술어가 판정한다(policy:BRANCH_CONSTITUTION_LAYERING C5).
#     술어가 PASS 여도 그 통로의 manifest 다리가 `absent` 면 토폴로지 사실이 없는 것이다 — 필터만으로 고르지 않는다(3).
#   · 둘 다 아니면 fail-loud. 호출부는 `--topology` 로 명시할 수 있다(환경 주입 > manifest — 헌법 serve-time 우선순위 방향).
#
# ★ 부재와 불일치를 가른다(topology_parity A1 과 같은 절단선): manifest 가 **한 장도 없는** 트리(fresh clone ·
#   git 밖 export · 워크트리)는 미테라포밍이다 — 존재는 policy:TERRAFORM_FLAG_GATE 의 몫이므로 3 으로 알리고
#   호출부가 자기 Flag 게이트 의미로 옮긴다(run_bench·lite_bench = exit 4 · smoke_clone A8 계약 보존).
#   manifest 가 있는데 정하지 못하면(술어 RED·카드 검증 실패·카드↔manifest 불일치·판정자 부재) 5 다.
#
# 사용: resolve_topology.py --repo REPO      → stdout 에 `single|multi` 한 줄 · stderr 에 출처 한 줄
#       resolve_topology.py --self-test      (해소기 단위 R* · 벤치 6종 바이트 사본 실행 S* · 음성대조 포함)
# 종료: 0=해소 · 2=인자 오류 · 3=미테라포밍(판정할 manifest 가 없다) · 5=해소 불가(계약 위반·모호·판정자 부재)
import argparse
import glob
import json
import os
import subprocess
import sys

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_UNTERRAFORMED = 3
EXIT_UNRESOLVED = 5

CARD_REL = "Agent_Card.json"
# 두-후보 순서는 run_bench.sh·lite_bench.sh 게이트(`CARD_VERIFIER`)와 같다: 서브 배달분 → 메인 정본.
CARD_VERIFIER_RELS = (os.path.join(".claude", "runtime", "a2a", "agent_card_contract.py"),
                      os.path.join(".claude", "skills", "terraforming_node", "scripts", "agent_card_contract.py"))
PARITY_REL = os.path.join(".claude", "skills", "terraforming_node", "scripts", "topology_parity.py")
TAG = "[resolve_topology]"


def _manifests(repo):
    return sorted(p for p in glob.glob(os.path.join(repo, "output", "*", "manifest.yaml")) if os.path.isfile(p))


def _manifest_topology_line(path):
    """최상위 `topology:` 한 줄의 값(주석·따옴표 제거). yaml 파서를 부르지 않는다 — 읽는 것은 닫힌 한 줄이다
    (node_identity.sh `self_role` 판독과 같은 규율)."""
    try:
        with open(path, encoding="utf-8") as handle:
            for line in handle:
                if line.startswith("topology:"):
                    value = line.split(":", 1)[1].split("#", 1)[0].strip().strip("'\"")
                    return value or None
    except OSError:
        return None
    return None


def _run(argv, timeout=60):
    try:
        proc = subprocess.run(argv, capture_output=True, text=True, timeout=timeout)
    except (OSError, subprocess.SubprocessError) as exc:
        return None, "", str(exc)
    return proc.returncode, proc.stdout, proc.stderr


def resolve(repo, python=None):
    """→ (rc, topology | None, 출처 또는 사유). 부작용 없음 — 읽기만 한다."""
    python = python or sys.executable
    repo = os.path.abspath(repo)
    manifests = _manifests(repo)

    card = os.path.join(repo, CARD_REL)
    if os.path.isfile(card):
        verifier = next((os.path.join(repo, rel) for rel in CARD_VERIFIER_RELS
                         if os.path.isfile(os.path.join(repo, rel))), None)
        if verifier is None:
            return EXIT_UNRESOLVED, None, ("Agent_Card.json 은 있는데 검증기가 없다(%s) — 서명을 확인하지 못한 카드의 "
                                           "topology 를 쓰지 않는다. 메인의 재배달이 필요하다" % " | ".join(CARD_VERIFIER_RELS))
        rc, out, err = _run([python, verifier, "prove-identity", "--repo-root", repo])
        if rc != 0:
            return EXIT_UNRESOLVED, None, ("정체성 증명 실패(rc=%s) — 검증되지 않은 카드의 topology 를 쓰지 않는다: %s"
                                           % (rc, (err or out).strip()[-300:]))
        try:
            proof = json.loads(out.strip() or "{}")
        except ValueError:
            proof = None
        topo = proof.get("topology") if isinstance(proof, dict) else None
        if not isinstance(topo, str) or not topo or os.path.basename(topo) != topo or topo.startswith("."):
            return EXIT_UNRESOLVED, None, "검증된 카드가 통로 이름으로 쓸 수 있는 topology 를 싣지 않는다(%r)" % (topo,)
        channel = os.path.join(repo, "output", topo, "manifest.yaml")
        if not os.path.isfile(channel):
            if manifests:
                return EXIT_UNRESOLVED, None, ("카드는 topology=%s 라고 말하는데 그 통로의 manifest(output/%s/manifest.yaml)가 "
                                               "없고 다른 통로 manifest 만 있다(%s) — 정체성과 사실이 갈린다"
                                               % (topo, topo, ", ".join(os.path.relpath(m, repo) for m in manifests)))
            return EXIT_UNTERRAFORMED, None, ("카드는 topology=%s 라고 말하지만 이 노드의 manifest 가 없다 — "
                                              "서브 manifest 는 메인의 terraforming 이 발급·배달한다" % topo)
        declared = _manifest_topology_line(channel)
        if declared != topo:
            return EXIT_UNRESOLVED, None, ("카드 topology=%s ≠ output/%s/manifest.yaml#topology=%r — 정체성 권위(카드)와 "
                                           "사실 권위(manifest)가 갈리면 어느 쪽도 고르지 않는다" % (topo, topo, declared))
        return EXIT_OK, topo, ("agent-card(prove-identity 서명 검증 · kid=%s) = output/%s/manifest.yaml#topology"
                               % (proof.get("kid"), topo))

    if not manifests:
        return EXIT_UNTERRAFORMED, None, ("output/*/manifest.yaml 이 한 장도 없다(토폴로지 사실 부재 · "
                                          "존재는 policy:TERRAFORM_FLAG_GATE 의 몫). terraforming_node 로 manifest 를 "
                                          "채우거나 --topology 를 명시하라")
    parity = os.path.join(repo, PARITY_REL)
    if not os.path.isfile(parity):
        return EXIT_UNRESOLVED, None, ("카드도 4자일치 판정자(%s)도 없다 — manifest 는 있는데 어느 통로가 이 노드의 "
                                       "사실인지 정할 소유자가 없다(서브라면 정체성 증명 Agent_Card 배달 결손)" % PARITY_REL)
    rc, out, err = _run([python, parity, "evaluate", "--repo", repo, "--format", "json"])
    try:
        verdict = json.loads(out) if out.strip() else None
    except ValueError:
        verdict = None
    if rc not in (0, 5) or not isinstance(verdict, dict):
        return EXIT_UNRESOLVED, None, "4자일치 판정자가 판정하지 못했다(rc=%s): %s" % (rc, (err or out).strip()[-300:])
    if verdict.get("verdict") != "PASS" or not verdict.get("topology"):
        return EXIT_UNRESOLVED, None, ("4자일치 RED — %s (자동 교정하지 않는다 · 체크아웃 전환인지 선언 수정인지는 사람이 "
                                       "정한다 · 급하면 --topology 명시)" % " / ".join(verdict.get("reasons") or ["사유 없음"]))
    topo = verdict["topology"]
    leg = (verdict.get("legs") or {}).get("manifest") or {}
    if leg.get("status") != "ok":
        return EXIT_UNTERRAFORMED, None, ("체크아웃 통로 output/%s 의 manifest 다리가 %r 다 — 브랜치 필터만으로는 토폴로지를 "
                                          "고르지 않는다(사실 권위 부재). terraforming_node 로 채우거나 --topology 를 명시하라"
                                          % (topo, leg.get("status")))
    return EXIT_OK, topo, ("topology_parity(4자일치 PASS · 판정 다리 %s · manifest=%s)"
                           % (",".join(verdict.get("legs_judged") or []), leg.get("path")))


# ─────────────────────────────── self-test ───────────────────────────────

def _self_test():
    import shutil
    import tempfile

    failures = []

    def ck(name, cond, detail=""):
        print(("  [PASS] " if cond else "  [FAIL] ") + name + ("" if cond else "  → " + str(detail)[-600:]))
        if not cond:
            failures.append(name)

    here = os.path.dirname(os.path.abspath(__file__))
    real_repo = os.path.normpath(os.path.join(here, "..", "..", "..", ".."))
    real_parity = os.path.join(real_repo, PARITY_REL)

    def write(path, text, mode=None):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(text)
        if mode:
            os.chmod(path, mode)

    def git(root, *args):
        return subprocess.run(["git", "-C", root, *args], capture_output=True, text=True, timeout=60)

    stub_verifier = ("import json, os, sys\n"
                     "mode = os.environ.get('RT_STUB_CARD', 'single')\n"
                     "if mode == 'fail':\n    sys.stderr.write('stub: signature broken\\n'); sys.exit(3)\n"
                     "print(json.dumps({'kid': 'stub-kid', 'role': 'a2a-agent', 'topology': mode}))\n")

    # ── R: 해소기 단위(카드 경로 = 검증기 스텁 · 테스트 평면의 외부 의존 차단 · 메인 경로 = 배포되는 술어 그대로) ──
    with tempfile.TemporaryDirectory(prefix="resolve_topology_R.") as td:
        sub = os.path.join(td, "sub")
        write(os.path.join(sub, CARD_REL), "{}")
        write(os.path.join(sub, CARD_VERIFIER_RELS[0]), stub_verifier)
        write(os.path.join(sub, "output", "single", "manifest.yaml"), "self_role: sub\ntopology: single\n")
        os.environ["RT_STUB_CARD"] = "single"
        rc, topo, why = resolve(sub)
        ck("R1 서브(카드 검증 통과 · manifest 일치) → single · 출처 agent-card", rc == 0 and topo == "single"
           and why.startswith("agent-card("), (rc, topo, why))
        os.environ["RT_STUB_CARD"] = "fail"
        rc, topo, why = resolve(sub)
        ck("★R2 음성대조: 카드 서명 검증 실패 → 5(검증 안 된 카드의 topology 를 쓰지 않는다)",
           rc == EXIT_UNRESOLVED and topo is None and "정체성 증명 실패" in why, (rc, why))
        os.environ["RT_STUB_CARD"] = "multi"
        rc, topo, why = resolve(sub)
        ck("★R3 음성대조: 카드 multi ∧ manifest 는 single 통로에만 → 5(정체성과 사실이 갈린다)",
           rc == EXIT_UNRESOLVED and "갈린다" in why, (rc, why))
        write(os.path.join(sub, "output", "single", "manifest.yaml"), "self_role: sub\ntopology: multi\n")
        os.environ["RT_STUB_CARD"] = "single"
        rc, topo, why = resolve(sub)
        ck("★R4 음성대조: 카드 single ≠ manifest#topology multi → 5", rc == EXIT_UNRESOLVED and "≠" in why, (rc, why))
        os.remove(os.path.join(sub, CARD_VERIFIER_RELS[0]))
        rc, topo, why = resolve(sub)
        ck("★R5 음성대조: 카드는 있는데 검증기 부재 → 5", rc == EXIT_UNRESOLVED and "검증기가 없다" in why, (rc, why))
        os.environ.pop("RT_STUB_CARD", None)

        bare = os.path.join(td, "bare")
        os.makedirs(bare)
        rc, topo, why = resolve(bare)
        ck("R6 카드·manifest 모두 없음(fresh export) → 3 미테라포밍(판정자 유무와 무관)", rc == EXIT_UNTERRAFORMED, (rc, why))
        write(os.path.join(bare, "output", "single", "manifest.yaml"), "self_role: sub\ntopology: single\n")
        rc, topo, why = resolve(bare)
        ck("★R7 음성대조: manifest 는 있는데 카드도 판정자도 없음 → 5(unknown→single 침묵 기본값 ✗)",
           rc == EXIT_UNRESOLVED and topo is None, (rc, why))

    have_git = shutil.which("git") is not None
    if not have_git:
        ck("P0 git 이 있어야 메인 경로(4자일치 술어)를 실행으로 친다", False, "git 없음")
    else:
        with tempfile.TemporaryDirectory(prefix="resolve_topology_P.") as td:
            def main_repo(name, branch, header, channels):
                root = os.path.join(td, name)
                os.makedirs(root)
                git(root, "init", "-q")
                git(root, "symbolic-ref", "HEAD", "refs/heads/" + branch)
                write(os.path.join(root, PARITY_REL), open(real_parity, encoding="utf-8").read())
                write(os.path.join(root, ".claude", "rules", "strategy.topology.md"),
                      "# t\n\n**topology: %s** · layer: topology\n" % header)
                git(root, "add", "-A")
                for ch, declared in channels.items():
                    write(os.path.join(root, "output", ch, "manifest.yaml"),
                          "self_role: main\ntopology: %s\n" % declared)
                return root

            r = main_repo("p1", "multi-node", "multi", {"multi": "multi", "single": "single"})
            rc, topo, why = resolve(r)
            ck("P1 메인 multi-node 체크아웃 · 특화 헤더 multi · output/multi manifest 일치 → multi · 출처 4자일치",
               rc == 0 and topo == "multi" and why.startswith("topology_parity("), (rc, topo, why))
            r = main_repo("p2", "feature-x", "multi", {"multi": "multi"})
            rc, topo, why = resolve(r)
            ck("★P2 음성대조: 운영 브랜치가 아닌 이름 → 5(종전 관용구는 여기서 조용히 single)",
               rc == EXIT_UNRESOLVED and topo is None and "BRANCH_UNKNOWN" in why, (rc, why))
            r = main_repo("p3", "multi-node", "single", {"multi": "multi"})
            rc, topo, why = resolve(r)
            ck("★P3 음성대조: 특화 헤더가 반대 토폴로지 → 5", rc == EXIT_UNRESOLVED and "LAYER_HEADER_MISMATCH" in why, (rc, why))
            r = main_repo("p4", "multi-node", "multi", {"single": "single"})
            rc, topo, why = resolve(r)
            ck("★P4 통로 manifest 부재(반대 통로 manifest 만 있다) → 3(브랜치 필터만으로 고르지 않는다)",
               rc == EXIT_UNTERRAFORMED and "absent" in why, (rc, why))
            r = main_repo("p5", "single-node", "single", {"single": "multi"})
            rc, topo, why = resolve(r)
            ck("★P5 음성대조: 통로 manifest 가 다른 topology 를 말한다 → 5", rc == EXIT_UNRESOLVED
               and "MANIFEST_MISMATCH" in why, (rc, why))

    # ── S: 벤치 6종 바이트 사본 실행(호출이 실제 진입 경로에 서 있는가 · 호출자 없는 자체검사 = L1 ✗) ──
    scripts = ("broad_search.sh", "judge_bench.sh", "lite_bench.sh", "run_bench.sh", "sweep_bench.sh", "max_envelope.sh")
    # 브랜치 이름 판독 형태는 하나가 아니다(2026-09-14 리뷰 정정 — `symbolic-ref` 는 topology_parity 가 이미 브랜치 판독에 쓴다).
    #   리터럴 하나만 보면 다른 형태로 돌아온 추론이 초록이다. 운영 브랜치 이름을 case 가지로 고르는 모양도 함께 본다.
    branch_forms = ("abbrev-ref", "symbolic-ref", "show-current", "branch --show", "multi-node)", "single-node)",
                    "refs/heads/")
    for name in scripts:
        text = open(os.path.join(here, name), encoding="utf-8").read()
        found = [form for form in branch_forms if form in text]
        ck("S0 %s 에 브랜치 이름 추론 형태가 없다(%s 0) · 해소기를 부른다" % (name, "·".join(branch_forms)),
           not found and "resolve_topology.py" in text, found)
    _probe = 'BR="$(git symbolic-ref --short HEAD)"\ncase "$BR" in multi-node) TOPO=multi;; esac\n'
    ck("★S0b 음성대조: 다른 형태의 브랜치 추론(symbolic-ref · case multi-node)) 은 S0 형태 목록에 걸린다",
       [form for form in branch_forms if form in _probe] == ["symbolic-ref", "multi-node)"])
    if have_git:
        with tempfile.TemporaryDirectory(prefix="resolve_topology_S.") as td:
            root = os.path.join(td, "repo")
            ab = os.path.join(root, ".claude", "skills", "adversarial-benchmark", "scripts")
            os.makedirs(ab)
            for name in scripts + ("resolve_topology.py", "repeat_axis.py"):
                shutil.copy2(os.path.join(here, name), os.path.join(ab, name))
            git(root, "init", "-q")
            git(root, "symbolic-ref", "HEAD", "refs/heads/multi-node")
            write(os.path.join(root, PARITY_REL), open(real_parity, encoding="utf-8").read())
            write(os.path.join(root, ".claude", "rules", "strategy.topology.md"), "**topology: multi** · layer: topology\n")
            git(root, "add", "-A")
            write(os.path.join(root, "output", "multi", "manifest.yaml"), "self_role: main\ntopology: multi\n")
            write(os.path.join(root, "output", "multi", "configs", "_probe.yaml"), "model: x\nmax-model-len: 8192\n")
            marker = os.path.join(td, "mc_argv.txt")
            # run_bench·lite_bench 의 Flag 게이트 소유자 자리에 argv 를 기록하는 스텁(테스트 평면 격리) — 해소된 TOPO 가
            #   게이트까지 **실제로 흘러가는지**를 본다. 스텁은 미발급(3)을 돌려 두 스크립트가 exit 4 에서 멈추게 한다.
            write(os.path.join(root, ".claude", "skills", "terraforming_node", "scripts", "manifest_contract.py"),
                  "import sys\nopen(%r, 'a').write(' '.join(sys.argv[1:]) + '\\n')\nsys.exit(3)\n" % marker)
            env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")

            def run(name, *args):
                p = subprocess.run(["bash", os.path.join(ab, name), *args], cwd=root, capture_output=True, text=True,
                                   timeout=120, env=env)
                return p.returncode, p.stdout + p.stderr

            def mc_topology():
                if not os.path.isfile(marker):
                    return None
                line = open(marker, encoding="utf-8").read().splitlines()[-1].split()
                return line[line.index("--topology") + 1] if "--topology" in line else None

            state = os.path.join(td, "bs_state.json")
            init_args = ("init", "--sweep-id", "s", "--state", state, "--cells", "c1", "--control-variable", "x",
                         "--max-cells", "1", "--wall-clock-budget-s", "60", "--consecutive-failure-limit", "1",
                         "--declared-by", "selftest", "--basis", "selftest", "--authority", "explore",
                         "--now-utc", "2026-09-14T00:00:00Z", "--repeats", "3")

            rc, out = run("judge_bench.sh", "_probe", "--authority", "weak")
            ck("S1 judge_bench(--topology 없음) → manifest 통로 multi 로 해소(output/multi 경로에서 멈춘다)",
               rc == 2 and "output/multi/benchlog" in out and TAG in out, (rc, out))
            rc, out = run("sweep_bench.sh", "_probe", "--backend", "openai", "--dry-run")
            ck("S2 sweep_bench --dry-run(--topology 없음) → topo=multi", rc == 0 and "topo=multi" in out, (rc, out))
            rc, out = run("lite_bench.sh", "_probe")
            ck("S3 lite_bench(--topology 없음) → 해소된 multi 가 Flag 게이트까지 흘러간다(exit 4)",
               rc == 4 and mc_topology() == "multi", (rc, mc_topology(), out))
            os.remove(marker)
            rc, out = run("run_bench.sh", "_probe")
            ck("S4 run_bench(--topology 없음) → 해소된 multi 가 Flag 게이트까지 흘러간다(exit 4)",
               rc == 4 and mc_topology() == "multi", (rc, mc_topology(), out))
            os.remove(marker)
            rc, out = run("broad_search.sh", *init_args)
            st = json.load(open(state, encoding="utf-8")) if os.path.isfile(state) else {}
            ck("S5 broad_search init(--topology 없음) → 상태 파일 topology=multi", rc == 0 and st.get("topology") == "multi",
               (rc, st.get("topology"), out))
            rc, out = run("max_envelope.sh", "_probe", "--dry-run")
            ck("S6 max_envelope --dry-run(--topology 없음) → topo=multi", rc == 0 and "topo=multi" in out, (rc, out))
            rc, out = run("broad_search.sh", "cell", "--state", "/nonexistent/bs.json", "--now-utc", "2026-01-01T00:00:00Z",
                          "--cell-key", "k", "--config", "c", "--axis-citation", "x", "--bench-budget-mib", "1")
            ck("S7 broad_search cell 의 --confirm-risk 게이트(exit 5)는 토폴로지 해소보다 앞선다(인자 평면 가드 보존)",
               rc == 5 and TAG not in out, (rc, out))

            # 음성대조 — 운영 브랜치가 아닌 이름으로 옮긴다: 종전 관용구는 여기서 **조용히 single** 이었다.
            git(root, "symbolic-ref", "HEAD", "refs/heads/feature-x")
            os.remove(state)
            for name, args, want in (("judge_bench.sh", ("_probe", "--authority", "weak"), 2),
                                     ("sweep_bench.sh", ("_probe", "--backend", "openai", "--dry-run"), 2),
                                     ("lite_bench.sh", ("_probe",), 4),
                                     ("run_bench.sh", ("_probe",), 4),
                                     ("broad_search.sh", init_args, 2),
                                     ("max_envelope.sh", ("_probe", "--dry-run"), 2)):
                rc, out = run(name, *args)
                ck("★S8 음성대조 %s: 브랜치 feature-x → exit %d · 해소 불가 사유(BRANCH_UNKNOWN) · single 로 접지 않는다"
                   % (name, want), rc == want and "BRANCH_UNKNOWN" in out and "topo=single" not in out
                   and not os.path.isfile(marker) and not os.path.isfile(state), (rc, out))
            rc, out = run("judge_bench.sh", "_probe", "--authority", "weak", "--topology", "single")
            ck("S9 --topology 명시는 해소기를 부르지 않는다(명시 > 파생) · output/single 경로",
               rc == 2 and "output/single/benchlog" in out and TAG not in out, (rc, out))
            git(root, "symbolic-ref", "HEAD", "refs/heads/multi-node")
            shutil.rmtree(os.path.join(root, "output"))
            rc, out = run("run_bench.sh", "_probe")
            ck("S10 manifest 가 한 장도 없는 트리 → run_bench exit 4(미테라포밍 info-only · smoke_clone A8 계약 보존)",
               rc == 4 and "미테라포밍" in out and not os.path.isfile(marker), (rc, out))
            rc, out = run("judge_bench.sh", "_probe", "--authority", "weak")
            ck("S11 manifest 부재 트리 → judge_bench exit 2(fail-loud · --topology 명시 안내)",
               rc == 2 and "--topology" in out, (rc, out))

    if failures:
        sys.stderr.write("%s --self-test FAIL %d 건: %s\n" % (TAG, len(failures), failures))
        return 1
    print("%s --self-test OK — R1~R7 · P1~P5 · S0·S0b~S11 전부 통과" % TAG)
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description="벤치 스크립트 공용 토폴로지 해소기(브랜치 이름 추론 ✗)")
    ap.add_argument("--repo")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)
    if args.self_test:
        return _self_test()
    if not args.repo:
        sys.stderr.write("%s ERROR --repo 가 필요하다\n" % TAG)
        return EXIT_USAGE
    rc, topo, why = resolve(args.repo)
    if rc == EXIT_OK:
        sys.stderr.write("%s topology=%s ← %s\n" % (TAG, topo, why))
        print(topo)
    else:
        sys.stderr.write("%s %s — %s\n" % (TAG, "미테라포밍" if rc == EXIT_UNTERRAFORMED else "해소 불가", why))
    return rc


if __name__ == "__main__":
    sys.exit(main())
