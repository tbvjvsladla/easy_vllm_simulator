#!/usr/bin/env python3
# regen_build_patches_src.py — `build_patches_src/files/` 재생성기 · 번들러 · 검증기
#
# what : 자체-이식 변종(arch-wall 사다리 3번째 칸)의 **파생 payload**(vLLM 소스 벤더링 N파일)를
#        (a) 상류 좌표에서 **재파생**하고 (b) 결정론 **번들**로 묶어 (c) egress-restricted 서브에
#        배달·**해체·검증**한다. 손작성 정본(`*.sh`·`PROVENANCE.json`·resolutions)은 여전히 git 추적물이며
#        payload 는 비추적이다 — 이 도구가 그 비대칭을 성립시키는 **생성엔진**이다.
#
# why  : `.gitignore` 는 `output/*/build_patches_src/files/` 를 비추적으로 못박으면서 근거를
#        *"PROVENANCE.json 의 유도식으로 재생성하고 파일별 sha256 으로 검증한다"* 라고 적었다.
#        그런데 **그 재생성 도구가 존재하지 않았다**(2026-08-14 B-5). 전제가 없는 규정은 규정이 아니라
#        구멍이다 — 서브(egress-restricted)는 상류를 clone 할 수 없으므로 payload 를 **영원히 얻지 못한다**.
#        D3 법칙대로 우회(추적 승격·수작업 scp)가 아니라 **경로를 만든다**.
#
# 재파생이 왜 결정론인가 — 세 종류의 파일이 있고 셋 다 좌표에서 유도된다:
#   ① mechanical    : `git diff <pr_base>..<fork_head> -- <scope>` 를 base 태그에 `git apply --3way`
#                     → 충돌 없이 붙는 파일. 유도식 그대로다.
#   ② fork-verbatim : 충돌했고 **PR본 통째 채택**으로 판정한 파일 → `git show <fork_head>:<path>`.
#                     바이트를 적지 않고 **결정**만 적는다(결정론 규율 §하드코딩: 파생 가능한 것을
#                     손으로 적지 않는다).
#   ③ residual-patch: 헝크 판정·드리프트 되돌림·폐포 최소추출처럼 **사람의 판단이 들어간 잔차**.
#                     이것만 손작성 추적물(`resolutions.patch`)이며 실측 328줄/14KB 다.
#   최종 게이트는 언제나 `PROVENANCE.json` 의 파일별 sha256 이다 — ①②③ 어디서 어긋나도 fail-loud.
#
# ⚠ ③ 은 `git apply --3way` 의 **충돌 마커 바이트**에 의존한다. git 구현이 마커를 바꾸면 잔차가 안 붙는다.
#    그래서 resolutions.json 에 생성 당시 git 버전을 기록하고, 적용 실패 시 그 사실을 지목해 실패한다
#    (침묵 폴백 금지 — 규율 §4종 안티패턴 '폴백' 결함 칸).
#
# 권위 평면(정본 = terraforming_node/SKILL.md §2.7.4 권위 평면 계약 — 2026-08-15 workflow.md 에서 이관.
#           "새 도구는 이 표에 행을 추가한다"는 규약에 따른 이 도구의 행):
#   이 도구 = **파일시스템 payload + 인덱스 PROVENANCE 검증**. 즉 판정 권위는 추적물(인덱스)에 있고
#   바이트만 파일시스템에서 온다. 그래서 비추적 payload 를 배달해도 "무엇이 배달됐는지"는 추적물이 정한다.
#
# usage:
#   verify              --root <build_patches_src>          # payload ↔ PROVENANCE sha256 대조
#   derive              --root <build_patches_src> --work <dir>   # 상류 좌표에서 재파생(네트워크 필요)
#   bundle              --root <build_patches_src> --out <tar.gz> # verify 통과분만 결정론 번들
#   unbundle --bundle <tar.gz> --root <build_patches_src>         # 해체 후 verify(서브에서 실행)
#   snapshot-resolutions --root <build_patches_src> --work <dir>  # 잔차 자산 재생성(저작 보조·메인 전용)
#
# plan : plan_26081418(R2 스코프) · CLAUDE.md 배포단위=스켈레톤+생성엔진 · workflow.md D3

import argparse
import gzip
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile

TAG = "[regen-src]"
EXEC_MODE = 0o755
FILE_MODE = 0o644


def log(msg):
    print(f"{TAG} {msg}", flush=True)


def fail(msg, code=1):
    print(f"{TAG} FAIL: {msg}", file=sys.stderr, flush=True)
    raise SystemExit(code)


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def repo_root_default():
    # scripts/ → upstream-version-watch/ → skills/ → .claude/ → repo
    # ⚠ 이 도구는 서브에서 **stdin 으로** 실행된다(`python3 - unbundle …`) — 그때 __file__ 은 '<stdin>'
    #   이고 실체가 없다. 그 경우 cwd 로 떨어진다. verify/unbundle 은 repo 를 쓰지 않으므로 무해하고,
    #   derive/snapshot 은 repo 안에서만 돌므로 cwd 가 곧 정답이다.
    if os.path.isfile(__file__):
        here = os.path.dirname(os.path.abspath(__file__))
        cand = os.path.abspath(os.path.join(here, "..", "..", "..", ".."))
        if os.path.isdir(os.path.join(cand, ".claude")):
            return cand
    return os.getcwd()


def load_provenance(root, override):
    path = override or os.path.join(root, "PROVENANCE.json")
    if not os.path.isfile(path):
        fail(f"PROVENANCE.json 부재: {path} — 이식 변종이 아니면 이 도구를 부르지 않는다")
    with open(path, "rb") as fh:
        man = json.load(fh)
    for key in ("source", "files"):
        if key not in man:
            fail(f"PROVENANCE.json 스키마 위반: '{key}' 부재 ({path})")
    declared = man.get("counts", {}).get("files")
    if declared is not None and declared != len(man["files"]):
        fail(f"PROVENANCE.json 자기모순: counts.files={declared} 인데 files 항목은 {len(man['files'])}개")
    return man, path


def run(cmd, cwd=None, check=True, capture=True):
    proc = subprocess.run(
        cmd, cwd=cwd, check=False,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.PIPE if capture else None,
        text=True,
    )
    if check and proc.returncode != 0:
        detail = (proc.stderr or proc.stdout or "").strip()
        fail(f"명령 실패(rc={proc.returncode}): {' '.join(cmd)}\n    {detail}")
    return proc


# ── 검증 ─────────────────────────────────────────────────────────────────────
def verify(files_root, man, quiet=False):
    """0=통과. missing/mismatch/extra 를 모두 센다 — 하나라도 있으면 비-0."""
    if not os.path.isdir(files_root):
        print(f"{TAG} FAIL: payload 디렉터리 부재: {files_root}", file=sys.stderr)
        return 1
    missing, mismatch = [], []
    for rel, want in sorted(man["files"].items()):
        path = os.path.join(files_root, rel)
        if not os.path.isfile(path) or os.path.islink(path):
            missing.append(rel)
            continue
        if sha256_file(path) != want:
            mismatch.append(rel)
    # extra 도 결함이다: 선언되지 않은 바이트가 소스트리에 덮이면 PROVENANCE 가 거짓말이 된다.
    extra = []
    for base, _dirs, names in os.walk(files_root):
        for name in names:
            rel = os.path.relpath(os.path.join(base, name), files_root)
            if rel not in man["files"]:
                extra.append(rel)
    total = len(man["files"])
    ok = not (missing or mismatch or extra)
    line = (f"무결성 {'OK' if ok else 'FAIL'} — declared={total} "
            f"missing={len(missing)} mismatch={len(mismatch)} extra={len(extra)}")
    if ok:
        if not quiet:
            log(line)
        return 0
    print(f"{TAG} {line}", file=sys.stderr)
    for label, items in (("MISSING", missing), ("MISMATCH", mismatch), ("EXTRA", extra)):
        for rel in items[:40]:
            print(f"{TAG}   {label} {rel}", file=sys.stderr)
        if len(items) > 40:
            print(f"{TAG}   {label} … 외 {len(items) - 40}건", file=sys.stderr)
    return 1


# ── 번들(결정론) ──────────────────────────────────────────────────────────────
def _tarinfo(rel, path):
    st = os.stat(path)
    info = tarfile.TarInfo("files/" + rel)
    info.size = st.st_size
    info.mtime = 0                      # 벽시계 금지(docs.md §logs) · 재현성
    info.mode = EXEC_MODE if st.st_mode & 0o100 else FILE_MODE   # git 의 두 모드로 정규화
    info.uid = info.gid = 0
    info.uname = info.gname = ""
    info.type = tarfile.REGTYPE
    return info


def build_bundle(files_root, man, out_path):
    """같은 payload → 같은 바이트. 정렬·mtime0·uid0·모드정규화·gzip mtime0."""
    tmp = out_path + ".partial"
    with open(tmp, "wb") as raw:
        gz = gzip.GzipFile(filename="", mode="wb", compresslevel=9, fileobj=raw, mtime=0)
        try:
            with tarfile.open(fileobj=gz, mode="w", format=tarfile.PAX_FORMAT) as tar:
                for rel in sorted(man["files"]):
                    path = os.path.join(files_root, rel)
                    with open(path, "rb") as fh:
                        tar.addfile(_tarinfo(rel, path), fh)
        finally:
            gz.close()
    os.replace(tmp, out_path)
    return sha256_file(out_path)


def extract_bundle(bundle_path, dest_root):
    """dest_root(=files/) 를 새로 만든다. 멤버 경로는 fail-closed 로 검사한다."""
    if os.path.exists(dest_root):
        shutil.rmtree(dest_root)
    os.makedirs(dest_root, exist_ok=True)
    count = 0
    with gzip.GzipFile(bundle_path, "rb") as gz:
        with tarfile.open(fileobj=gz, mode="r|") as tar:
            for member in tar:
                name = member.name
                if not name.startswith("files/"):
                    fail(f"번들 멤버가 files/ 밖이다: {name}")
                if not member.isreg():
                    fail(f"번들에 비정규 멤버: {name} (type={member.type!r})")
                rel = name[len("files/"):]
                if not rel or rel.startswith("/") or os.path.isabs(rel) \
                        or any(part in ("..", "") for part in rel.split("/")):
                    fail(f"번들 멤버 경로 거부: {name}")
                target = os.path.join(dest_root, rel)
                if os.path.relpath(target, dest_root).startswith(".."):
                    fail(f"번들 멤버가 대상 밖으로 탈출한다: {name}")
                os.makedirs(os.path.dirname(target), exist_ok=True)
                src = tar.extractfile(member)
                if src is None:
                    fail(f"번들 멤버를 읽을 수 없다: {name}")
                with open(target, "wb") as out:
                    shutil.copyfileobj(src, out)
                os.chmod(target, EXEC_MODE if member.mode & 0o100 else FILE_MODE)
                count += 1
    return count


# ── 재파생 ───────────────────────────────────────────────────────────────────
def resolutions_paths(man, repo, must_exist=True):
    ref = man.get("resolutions")
    if not ref:
        fail("PROVENANCE.json 에 resolutions 포인터가 없다 — 잔차 자산 없이는 재파생이 결정론이 아니다")
    spec = os.path.join(repo, ref["spec"])
    patch = os.path.join(repo, ref["patch"])
    if must_exist and not os.path.isfile(spec):
        fail(f"resolutions 스펙 부재: {spec} — `snapshot-resolutions` 로 먼저 발행하라")
    return spec, patch


def ensure_source_clone(work, man):
    src = man["source"]
    repo_dir = os.path.join(work, "vllm-src")
    shas = [src["upstream_base_sha"], src["pr_base_sha"], src["fork_head_sha"]]
    if not os.path.isdir(os.path.join(repo_dir, ".git")):
        os.makedirs(work, exist_ok=True)
        log(f"clone {src['upstream_repo']} → {repo_dir} (수백 MB · 최초 1회)")
        run(["git", "clone", "--quiet", src["upstream_repo"], repo_dir], capture=False)
    have = all(run(["git", "cat-file", "-e", f"{s}^{{commit}}"], cwd=repo_dir, check=False).returncode == 0
               for s in shas)
    if not have:
        pr = src.get("fork_pr")
        log(f"fetch refs/pull/{pr}/head + 태그(좌표 SHA 확보)")
        run(["git", "fetch", "--quiet", "origin", f"+refs/pull/{pr}/head:refs/regen/pr{pr}"], cwd=repo_dir)
        run(["git", "fetch", "--quiet", "--tags", "origin"], cwd=repo_dir)
    for s in shas:
        if run(["git", "cat-file", "-e", f"{s}^{{commit}}"], cwd=repo_dir, check=False).returncode != 0:
            fail(f"좌표 SHA 를 상류에서 찾을 수 없다: {s} — PROVENANCE.source 를 확인하라")
    return repo_dir


def mechanical_tree(work, repo_dir, man, tree_name="derive-tree"):
    """base 태그 워크트리에 PR 스코프 델타를 3-way 적용. 충돌은 정상(마커가 남는다)."""
    src = man["source"]
    tree = os.path.join(work, tree_name)
    if os.path.exists(tree):
        run(["git", "worktree", "remove", "--force", tree], cwd=repo_dir, check=False)
        shutil.rmtree(tree, ignore_errors=True)
    run(["git", "worktree", "prune"], cwd=repo_dir, check=False)
    run(["git", "worktree", "add", "--detach", "--quiet", tree, src["upstream_base_sha"]], cwd=repo_dir)

    additions = set(man.get("scope_additions_beyond_pr_delta", {}))
    scope = sorted(set(man["files"]) - additions)
    declared = man.get("measurements", {}).get("port_scope", {}).get("files")
    if declared is not None and declared != len(scope):
        fail(f"스코프 자기모순: measurements.port_scope.files={declared} 인데 유도값은 {len(scope)}")
    log(f"PR 스코프 {len(scope)}파일 = files({len(man['files'])}) − 폐포/드리프트추출({len(additions)})")

    patch_path = os.path.join(work, "pr_scope.regen.patch")
    proc = run(["git", "diff", f"{src['pr_base_sha']}..{src['fork_head_sha']}", "--"] + scope, cwd=repo_dir)
    with open(patch_path, "w") as fh:
        fh.write(proc.stdout)

    apply_proc = run(["git", "apply", "--3way", patch_path], cwd=tree, check=False)
    stderr = apply_proc.stderr or ""
    conflicts = sorted(
        line.split(None, 1)[1]
        for line in run(["git", "status", "--porcelain"], cwd=tree).stdout.splitlines()
        if line.startswith("UU ") or line.startswith("AA ")
    )
    if apply_proc.returncode != 0 and not conflicts:
        fail(f"3-way 적용이 충돌이 아닌 이유로 실패했다:\n    {stderr.strip()}")
    declared_conf = man.get("measurements", {}).get("conflict_files")
    if declared_conf is not None and declared_conf != len(conflicts):
        # 좌표가 같은데 충돌 수가 다르면 git 동작이 달라진 것 — 잔차가 안 붙을 신호다. 먼저 말한다.
        log(f"⚠ 충돌 파일 수 불일치: PROVENANCE={declared_conf} 실측={len(conflicts)} "
            f"(잔차 패치 적용이 실패할 수 있다)")
    else:
        log(f"3-way 적용 완료 — 충돌 {len(conflicts)}파일(예상대로)")
    return tree, conflicts


def apply_fork_verbatim(tree, repo_dir, man, spec):
    fork = man["source"]["fork_head_sha"]
    rels = sorted(r for r, v in spec["files"].items() if v["strategy"] == "fork-verbatim")
    for rel in rels:
        blob = run(["git", "show", f"{fork}:{rel}"], cwd=repo_dir).stdout
        target = os.path.join(tree, rel)
        os.makedirs(os.path.dirname(target), exist_ok=True)
        with open(target, "w") as fh:
            fh.write(blob)
        mode = run(["git", "ls-tree", fork, "--", rel], cwd=repo_dir).stdout.split()[0]
        os.chmod(target, EXEC_MODE if mode == "100755" else FILE_MODE)
    log(f"fork-verbatim {len(rels)}파일 배치(바이트 아닌 **결정**을 기록한다)")
    return rels


def apply_residual(tree, patch_path, spec):
    rels = sorted(r for r, v in spec["files"].items() if v["strategy"] == "residual-patch")
    if not rels:
        return []
    if not os.path.isfile(patch_path):
        fail(f"잔차 패치 부재: {patch_path} (스펙은 {len(rels)}파일을 요구한다)")
    proc = run(["git", "apply", "-p1", patch_path], cwd=tree, check=False)
    if proc.returncode != 0:
        fail("잔차 패치 적용 실패 — 대개 `git apply --3way` 의 충돌 마커가 달라진 경우다.\n"
             f"    스펙 생성 git: {spec.get('derived_with_git', 'unknown')} · 현재 git: {git_version()}\n"
             f"    {(proc.stderr or '').strip()}\n"
             "    → 같은 git 으로 재시도하거나 `snapshot-resolutions` 로 잔차를 재발행하라.")
    log(f"잔차 패치 적용 {len(rels)}파일(사람 판단이 든 유일한 부분)")
    return rels


def git_version():
    return run(["git", "--version"], check=False).stdout.strip()


def materialize(tree, files_root, man):
    if os.path.exists(files_root):
        shutil.rmtree(files_root)
    for rel in sorted(man["files"]):
        src = os.path.join(tree, rel)
        if not os.path.isfile(src):
            fail(f"재파생 트리에 파일이 없다: {rel}")
        dst = os.path.join(files_root, rel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copyfile(src, dst)
        os.chmod(dst, EXEC_MODE if os.stat(src).st_mode & 0o100 else FILE_MODE)


# ── 서브커맨드 ───────────────────────────────────────────────────────────────
def cmd_verify(args):
    man, path = load_provenance(args.root, args.provenance)
    files_root = args.files_root or os.path.join(args.root, "files")
    log(f"authority={path}")
    return verify(files_root, man, quiet=args.quiet)


def cmd_bundle(args):
    man, path = load_provenance(args.root, args.provenance)
    files_root = args.files_root or os.path.join(args.root, "files")
    if verify(files_root, man, quiet=True) != 0:
        fail("payload 무결성 실패 — 번들을 만들지 않는다(검증되지 않은 바이트는 배달하지 않는다)")
    os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
    digest = build_bundle(files_root, man, args.out)
    size = os.path.getsize(args.out)
    log(f"번들 발행 {args.out} files={len(man['files'])} bytes={size} sha256={digest}")
    log(f"authority={path}")
    print(digest)
    return 0


def cmd_unbundle(args):
    man, path = load_provenance(args.root, args.provenance)
    files_root = args.files_root or os.path.join(args.root, "files")
    if not os.path.isfile(args.bundle):
        fail(f"번들 부재: {args.bundle}")
    digest = sha256_file(args.bundle)
    if args.expect_sha256 and digest != args.expect_sha256:
        fail(f"번들 전송 무결성 실패: expect={args.expect_sha256} actual={digest}")
    staging = os.path.join(os.path.dirname(os.path.abspath(files_root)), ".files.incoming")
    count = extract_bundle(args.bundle, staging)
    rc = verify(staging, man, quiet=True)
    if rc != 0:
        shutil.rmtree(staging, ignore_errors=True)
        fail("해체분이 PROVENANCE 와 불일치 — 기존 payload 를 건드리지 않고 중단한다")
    # ⚠ 침묵 삭제 금지(docs.md §log_evicted 와 동형): payload 는 **전량 교체**되므로 이전 이식본의
    #   잔재가 사라진다. 사라지는 것이 몇 건인지 말하지 않으면 "원래 없었던 것"과 구분되지 않는다.
    previous = set()
    if os.path.isdir(files_root):
        for base, _dirs, names in os.walk(files_root):
            for name in names:
                previous.add(os.path.relpath(os.path.join(base, name), files_root))
    declared = set(man["files"])
    evicted, added = sorted(previous - declared), sorted(declared - previous)
    if os.path.exists(files_root):
        shutil.rmtree(files_root)
    os.replace(staging, files_root)
    if previous:
        log(f"payload 교체 — 이전 {len(previous)}파일 → 선언 {len(declared)}파일 "
            f"(제거 {len(evicted)} · 신규 {len(added)} · 유지 {len(previous & declared)})")
        for rel in evicted[:20]:
            log(f"  evicted {rel}")
        if len(evicted) > 20:
            log(f"  evicted … 외 {len(evicted) - 20}건")
    log(f"해체·검증 OK — {count}파일 → {files_root} (bundle sha256={digest})")
    log(f"authority={path}")
    return verify(files_root, man)


def cmd_derive(args):
    man, path = load_provenance(args.root, args.provenance)
    files_root = args.files_root or os.path.join(args.root, "files")
    spec_path, patch_path = resolutions_paths(man, args.repo)
    with open(spec_path) as fh:
        spec = json.load(fh)
    work = os.path.abspath(args.work)
    repo_dir = ensure_source_clone(work, man)
    tree, _conflicts = mechanical_tree(work, repo_dir, man)
    apply_fork_verbatim(tree, repo_dir, man, spec)
    apply_residual(tree, patch_path, spec)
    materialize(tree, files_root, man)
    rc = verify(files_root, man)
    if rc == 0:
        log(f"재파생 완료 — {len(man['files'])}파일이 좌표에서 결정론적으로 복원됐다")
        log(f"authority={path}")
    else:
        fail("재파생 결과가 PROVENANCE 와 불일치 — 좌표·잔차·git 버전 중 하나가 어긋났다")
    if not args.keep:
        run(["git", "worktree", "remove", "--force", tree], cwd=repo_dir, check=False)
    return rc


def cmd_snapshot_resolutions(args):
    """현재 payload 를 정답으로 놓고 ①②③ 분류를 **자동 판정**해 잔차 자산을 재발행한다(메인 전용 저작)."""
    man, _path = load_provenance(args.root, args.provenance)
    files_root = args.files_root or os.path.join(args.root, "files")
    if verify(files_root, man, quiet=True) != 0:
        fail("현재 payload 가 PROVENANCE 와 불일치 — 잔차를 뜰 기준이 없다")
    spec_path, patch_path = resolutions_paths(man, args.repo, must_exist=False)
    work = os.path.abspath(args.work)
    repo_dir = ensure_source_clone(work, man)
    tree, conflicts = mechanical_tree(work, repo_dir, man, tree_name="snapshot-tree")
    fork = man["source"]["fork_head_sha"]

    classes, fork_rels, residual_rels = {}, [], []
    for rel in sorted(man["files"]):
        final = os.path.join(files_root, rel)
        cur = os.path.join(tree, rel)
        cur_sha = sha256_file(cur) if os.path.isfile(cur) else None
        if cur_sha == man["files"][rel]:
            classes[rel] = {"strategy": "mechanical"}
            continue
        blob = run(["git", "show", f"{fork}:{rel}"], cwd=repo_dir, check=False)
        if blob.returncode == 0 and hashlib.sha256(blob.stdout.encode()).hexdigest() == man["files"][rel]:
            classes[rel] = {"strategy": "fork-verbatim",
                            "reason": "충돌 판정 = PR본 통째 채택(드리프트가 PR 재작성 대비 미미)"}
            fork_rels.append(rel)
            continue
        classes[rel] = {"strategy": "residual-patch",
                        "reason": "헝크 판정·드리프트 되돌림·폐포 최소추출 등 사람 판단"}
        residual_rels.append(rel)

    spec = {
        "schema_version": 1,
        "variant_id": man.get("resolutions", {}).get("variant_id", "unknown"),
        "_note": "① mechanical=유도식 그대로 · ② fork-verbatim=결정만 기록(바이트 ✗) · "
                 "③ residual-patch=사람 판단이 든 유일한 손작성 잔차. 최종 게이트는 PROVENANCE sha256.",
        "derived_with_git": git_version(),
        "mechanical_conflict_files": conflicts,
        "counts": {"mechanical": len(man["files"]) - len(fork_rels) - len(residual_rels),
                   "fork_verbatim": len(fork_rels), "residual_patch": len(residual_rels)},
        "files": classes,
    }

    # fork-verbatim 을 먼저 놓아야 잔차가 '그 위의 차이'만 담는다.
    for rel in fork_rels:
        blob = run(["git", "show", f"{fork}:{rel}"], cwd=repo_dir).stdout
        target = os.path.join(tree, rel)
        os.makedirs(os.path.dirname(target), exist_ok=True)
        with open(target, "w") as fh:
            fh.write(blob)

    chunks = []
    for rel in residual_rels:
        cur = os.path.join(tree, rel)
        if not os.path.isfile(cur):
            fail(f"잔차 기준 파일이 재파생 트리에 없다(폐포 누락?): {rel}")
        proc = run(["diff", "-u", "--label", f"a/{rel}", "--label", f"b/{rel}",
                    cur, os.path.join(files_root, rel)], check=False)
        if proc.returncode not in (0, 1):
            fail(f"diff 실패: {rel}")
        chunks.append(proc.stdout)

    os.makedirs(os.path.dirname(spec_path), exist_ok=True)
    with open(spec_path, "w") as fh:
        json.dump(spec, fh, ensure_ascii=False, indent=1)
        fh.write("\n")
    with open(patch_path, "w") as fh:
        fh.write("".join(chunks))
    log(f"잔차 자산 발행 — mechanical={spec['counts']['mechanical']} "
        f"fork-verbatim={spec['counts']['fork_verbatim']} residual={spec['counts']['residual_patch']}")
    log(f"  {spec_path}")
    log(f"  {patch_path} ({sum(c.count(chr(10)) for c in chunks)}줄)")
    if not args.keep:
        run(["git", "worktree", "remove", "--force", tree], cwd=repo_dir, check=False)
    return 0


def main():
    ap = argparse.ArgumentParser(description="build_patches_src/files 재생성기·번들러·검증기")
    ap.add_argument("--repo", default=repo_root_default(), help="리포 루트(resolutions 자산 해소용)")
    sub = ap.add_subparsers(dest="cmd", required=True)

    def common(p, work=False):
        p.add_argument("--root", required=True, help="build_patches_src 디렉터리")
        p.add_argument("--provenance", help="PROVENANCE.json 경로 override(예: 인덱스 스냅샷)")
        p.add_argument("--files-root", help="payload 디렉터리 override(기본 <root>/files)")
        if work:
            p.add_argument("--work", required=True, help="재파생 작업 디렉터리(비추적)")
            p.add_argument("--keep", action="store_true", help="작업 워크트리 보존")

    p = sub.add_parser("verify", help="payload ↔ PROVENANCE sha256 대조")
    common(p); p.add_argument("--quiet", action="store_true")
    p.set_defaults(func=cmd_verify)

    p = sub.add_parser("bundle", help="verify 통과분만 결정론 번들로 발행")
    common(p); p.add_argument("--out", required=True)
    p.set_defaults(func=cmd_bundle)

    p = sub.add_parser("unbundle", help="번들 해체 후 verify(서브 실행 대상)")
    common(p)
    p.add_argument("--bundle", required=True)
    p.add_argument("--expect-sha256", help="전송 무결성 대조용 번들 해시")
    p.set_defaults(func=cmd_unbundle)

    p = sub.add_parser("derive", help="상류 좌표에서 payload 재파생(네트워크 필요)")
    common(p, work=True)
    p.set_defaults(func=cmd_derive)

    p = sub.add_parser("snapshot-resolutions", help="잔차 자산 재발행(메인 전용 저작)")
    common(p, work=True)
    p.set_defaults(func=cmd_snapshot_resolutions)

    args = ap.parse_args()
    if not hasattr(args, "quiet"):
        args.quiet = False
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
