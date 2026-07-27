#!/usr/bin/env python3
"""cleanup_docker.py — docker 찌꺼기 결정론 정리 (plan_26071019 §4.2).

무차별 prune 금지: ccache 빌드캐시는 의도적 자산(재빌드 2분 vs 풀빌드 수시간 —
devlog_26070213 cost-smart peel), 구버전 이미지 일부는 롤백 앵커. 따라서
① 보존리스트를 결정론 산출(IMAGE_TAG env·resolved.json·Dockerfile FROM·가동 컨테이너)
② 삭제 후보를 dry-run 표(용량 포함)로 제시 ③ --apply 는 사람 승인 후에만.

트리거(자동 주기 없음): bump S4 종료 루틴 + 세션말 사람 질의(sync_branches.sh 동형 관행).
stdlib-only. 종료코드: 0=성공(dry-run 포함) · 2=docker 조회 실패.
"""
import argparse
import json
import os
import re
import subprocess
import sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))


def _run(cmd, timeout=60):
    try:
        r = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)
        return r.returncode, r.stdout.decode("utf-8", errors="replace")
    except Exception as e:  # docker 부재/데몬 다운 — fail-loud
        return 1, str(e)


def preserve_set(canonical_only=False):
    """Collect deterministic never-delete refs.

    ``canonical_only`` is used by policy verification so branch-local delivered outputs cannot
    become trust inputs; production cleanup may additionally preserve refs from delivered outputs.
    """
    keep, why = set(), {}

    def add(ref, reason):
        ref = ref.strip()
        if ref:
            keep.add(ref)
            why.setdefault(ref, reason)

    # ① envs/.env.* 의 IMAGE_TAG= (easy-vllm:<tag> 조립은 compose 관례 — 값이 full ref 인 경우도 수용)
    for topo in (() if canonical_only else ("multi", "single")):
        envdir = os.path.join(REPO, "output", topo, "envs")
        if os.path.isdir(envdir):
            for fn in os.listdir(envdir):
                if not fn.startswith(".env"):
                    continue
                try:
                    text = open(os.path.join(envdir, fn), encoding="utf-8", errors="replace").read()
                except OSError:
                    continue
                for m in re.finditer(r"^IMAGE_TAG=(\S+)", text, re.M):
                    v = m.group(1)
                    add(v if ":" in v or "/" in v else "easy-vllm:" + v, "env %s/%s" % (topo, fn))
    # ② resolved.json (루트/output) 의 image/tag 류 문자열
    resolved_candidates = () if canonical_only else (
        os.path.join(REPO, "resolved.json"),
        os.path.join(REPO, "output", "multi", "resolved.json"),
        os.path.join(REPO, "output", "single", "resolved.json"),
    )
    for p in resolved_candidates:
        if os.path.isfile(p):
            try:
                blob = json.load(open(p, encoding="utf-8"))
            except Exception:
                continue
            for s in re.findall(r'"((?:easy-vllm|nvcr\.io/nvidia/pytorch)[^"]*)"', json.dumps(blob)):
                add(s, "resolved.json")
    # ③ Canonical shared templates are the topology-neutral authority; delivered outputs are
    # optional additional keep hints. Ignore unresolved template FROM tokens fail-closed.
    canonical = os.path.join(
        REPO, ".claude", "skills", "upstream-version-watch", "templates")
    resolution_path = os.path.join(
        REPO, ".claude", "skills", "upstream-version-watch", "assets",
        "current-production-resolution.json")
    try:
        with open(resolution_path, encoding="utf-8") as fh:
            resolution = json.load(fh)
        ngc_tag = resolution["ngc_base"]["tag"]
        if not isinstance(ngc_tag, str) or not ngc_tag.strip():
            raise ValueError("empty ngc_base.tag")
        add("nvcr.io/nvidia/pytorch:" + ngc_tag.strip(), "shared production resolution")
    except (OSError, UnicodeError, json.JSONDecodeError, KeyError, TypeError, ValueError) as exc:
        raise RuntimeError(f"canonical shared production resolution invalid: {exc}") from exc
    canonical_dockerfiles = [
        (os.path.join(canonical, "Dockerfile.template"), "shared/Dockerfile.template"),
        (os.path.join(canonical, "Dockerfile.source-build.template"),
         "shared/Dockerfile.source-build.template"),
    ]
    for path, label in canonical_dockerfiles:
        try:
            with open(path, encoding="utf-8", errors="strict") as fh:
                text = fh.read()
        except (OSError, UnicodeError) as exc:
            raise RuntimeError(f"canonical shared template invalid ({label}): {exc}") from exc
        if not text.strip() or not re.search(r"^FROM\s+\S+", text, re.M):
            raise RuntimeError(f"canonical shared template invalid ({label}): missing FROM instruction")
        for m in re.finditer(r"^FROM\s+(\S+)", text, re.M):
            ref = m.group(1)
            if not ref.startswith("$") and "{{" not in ref:
                add(ref, "FROM %s" % label)

    # Delivered branch outputs are optional preservation hints only; malformed/missing hints cannot
    # replace or hide mandatory canonical validation above.
    dockerfiles = []
    for topo in (() if canonical_only else ("multi", "single")):
        for dfn in ("Dockerfile", "Dockerfile.source-build"):
            dockerfiles.append((os.path.join(REPO, "output", topo, dfn),
                                "%s/%s" % (topo, dfn)))
    for path, label in dockerfiles:
        if not os.path.isfile(path):
            continue
        with open(path, encoding="utf-8", errors="replace") as fh:
            text = fh.read()
        for m in re.finditer(r"^FROM\s+(\S+)", text, re.M):
            ref = m.group(1)
            if not ref.startswith("$") and "{{" not in ref:
                add(ref, "FROM %s" % label)
    # ④ 가동/존재 컨테이너의 이미지
    rc, out = _run(["docker", "ps", "-a", "--format", "{{.Image}}"])
    if rc == 0:
        for line in out.splitlines():
            add(line, "container in use")
    return keep, why


def list_images():
    rc, out = _run(["docker", "image", "ls", "-a", "--format", "{{json .}}"])
    if rc != 0:
        print("[cleanup] FAIL: docker image ls 실패 — %s" % out.strip(), file=sys.stderr)
        sys.exit(2)
    return [json.loads(l) for l in out.splitlines() if l.strip()]


def main(canonical_only=False):
    ap = argparse.ArgumentParser(description="docker 찌꺼기 결정론 정리 (dry-run 기본)")
    ap.add_argument("--apply", action="store_true", help="dry-run 표의 항목을 실제 삭제(사람 승인 후)")
    ap.add_argument("--cache-age-hours", type=int, default=336,
                    help="빌드캐시 prune 기준(기본 336h=14일 미접근 — 활성 트랙 ccache 보존)")
    ap.add_argument("--keep", action="append", default=[],
                    help="추가 보존 이미지 ref(반복 지정 가능)")
    args = ap.parse_args()

    keep, why = preserve_set(canonical_only=canonical_only)
    for k in args.keep:
        keep.add(k)
        why.setdefault(k, "--keep 수동")

    print("[cleanup] 보존리스트(%d):" % len(keep))
    for r in sorted(keep):
        print("  KEEP  %-60s (%s)" % (r, why.get(r, "")))

    # 이미지 후보: 보존 미포함 전부(dangling 포함)
    cand = []
    for im in list_images():
        ref = "%s:%s" % (im.get("Repository", ""), im.get("Tag", ""))
        dangling = im.get("Repository") == "<none>" or im.get("Tag") == "<none>"
        target = im.get("ID") if dangling else ref
        if not dangling and ref in keep:
            continue
        cand.append({"ref": ref, "id": im.get("ID"), "size": im.get("Size"), "created": im.get("CreatedSince"),
                     "target": target, "dangling": dangling})

    print("\n[cleanup] 이미지 삭제 후보(%d) — 보존리스트 밖:" % len(cand))
    if not cand:
        print("  (없음)")
    for c in cand:
        print("  RM    %-60s %-10s %s%s" % (c["ref"], c["size"], c["created"], "  [dangling]" if c["dangling"] else ""))

    rc, out = _run(["docker", "system", "df"])
    print("\n[cleanup] 현 사용량:\n" + "\n".join("  " + l for l in out.splitlines()))
    print("\n[cleanup] 빌드캐시: %dh 미접근분 prune 대상 (docker builder prune --filter unused-for=%dh)"
          % (args.cache_age_hours, args.cache_age_hours))

    if not args.apply:
        print("\n[cleanup] DRY-RUN 종료 — 표 검토 후: python3 .claude/skills/vllm-recipe-explorer/scripts/cleanup_docker.py --apply [--keep <ref>]")
        return

    # ── APPLY (사람 승인 후) ──
    fails = 0
    for c in cand:
        rc, out = _run(["docker", "rmi", c["target"]], timeout=120)
        tag = "OK" if rc == 0 else "FAIL"
        if rc != 0:
            fails += 1
        print("[cleanup] rmi %-60s %s %s" % (c["target"], tag, out.strip().splitlines()[-1] if out.strip() else ""))
    rc, out = _run(["docker", "builder", "prune", "-f", "--filter", "unused-for=%dh" % args.cache_age_hours],
                   timeout=600)
    print("[cleanup] builder prune: %s" % ("OK" if rc == 0 else "FAIL"))
    for l in out.strip().splitlines()[-3:]:
        print("  " + l)
    rc, out = _run(["docker", "system", "df"])
    print("[cleanup] 정리 후 사용량:\n" + "\n".join("  " + l for l in out.splitlines()))
    if fails:
        print("[cleanup] ⚠ rmi 실패 %d건(자식 레이어 참조 등) — 표 재검토" % fails)


if __name__ == "__main__":
    main()
