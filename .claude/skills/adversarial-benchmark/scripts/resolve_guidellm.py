#!/usr/bin/env python3
# resolve_guidellm.py — 측정 도구(GuideLLM) **버전 해소**. plan_26090516 ③ 3-8 · 인터뷰 축 A.
#
# 왜 신설인가: 종전 규약은 `bench_tool_pin.json` 이 v0.7.3 을 digest 로 고정하고, 실행 시점에
# 그 digest 와 다르면 **실행을 거부**했다("latest 는 움직이는 표적이라 재현성 인증서의 입력이 될 수
# 없다"). 그런데 사용자 결정은 반대다 — **기본은 최신 릴리즈**이고, 특정 버전 지시가 있으면 그
# 버전의 독립 컨테이너로 잰다. 핀=버전고정 게이트는 그 결정과 충돌하므로 게이트를 걷어내고,
# 대신 **런별로 무엇을 썼는지 기록**한다(리포트·인증서). 재현성은 "고정했다"가 아니라 "무엇으로
# 쟀는지 적혀 있다"로 성립한다.
#
# 평면 분리(중요):
#   · 이 스크립트 = **스테이징 평면**. 네트워크로 최신 릴리즈를 해소하고 pull 명령을 낸다.
#   · `resolve_bench_tool.py` = **실행 평면**. 네트워크를 쓰지 않고 로컬 이미지를 확인만 한다
#     (airgap-safe 불변식 · 자동 pull ✗). 벤치 도중에 최신을 조회하지 않는다.
#
# 사용:
#   resolve_guidellm.py                      # 최신 릴리즈 해소(네트워크)
#   resolve_guidellm.py --version 0.7.3      # 선언 override(네트워크 ✗)
#   resolve_guidellm.py --record             # 해소 결과를 bench_tool_pin.json 기록에 병합
#   resolve_guidellm.py --self-test
# 종료: 0=해소 · 2=인자/응답 오류 · 3=네트워크로 해소 불가(버전을 선언하라)
import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.request

REPO_DEFAULT = "vllm-project/guidellm"
IMAGE_HOST = "ghcr.io"
LATEST_API = "https://api.github.com/repos/%s/releases/latest"
RECORD_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "bench_tool_pin.json")

RC_OK = 0
RC_ARGS = 2
RC_UNRESOLVED = 3

_VERSION_RE = re.compile(r"^\d+(?:\.\d+){1,3}(?:[-.a-zA-Z0-9]*)$")


def version_from_tag(tag):
    """릴리즈 태그 → 버전 문자열. 추측하지 않는다 — 형태가 아니면 예외."""
    if not isinstance(tag, str) or not tag.strip():
        raise ValueError("릴리즈 응답에 tag_name 이 없다")
    v = tag.strip()
    v = v[1:] if v.startswith("v") else v
    if not _VERSION_RE.match(v):
        raise ValueError("태그 %r 에서 버전을 읽지 못했다(명명 스킴 변경 가능성 — 사람이 확인)" % tag)
    return v


def image_ref(version, repo=REPO_DEFAULT):
    return "%s/%s:v%s" % (IMAGE_HOST, repo, version)


def resolution(version, source, repo=REPO_DEFAULT, published_utc=None):
    return {
        "tool": "guidellm",
        "repo": repo,
        "version": version,
        "version_source": source,
        "published_utc": published_utc,      # 상류가 말한 시각. 벽시계를 읽지 않는다.
        "image_ref": image_ref(version, repo),
        "pull_command": "docker pull %s" % image_ref(version, repo),
    }


def fetch_latest(repo=REPO_DEFAULT, opener=None):
    url = LATEST_API % repo
    req = urllib.request.Request(
        url, headers={"Accept": "application/vnd.github+json", "User-Agent": "adversarial-benchmark"})
    with (opener or urllib.request.urlopen)(req, timeout=20) as handle:
        return json.load(handle)


def merge_record(path, res, digest=None):
    """해소 결과를 기록 파일에 **병합**한다(게이트 아님 · 관측된 사실의 보관소).

    digest 는 맹점층이라 유지한다(`policy:GIT_SINGLE_AUTHORITY` 2문항: git 이 이 바이트를 들지
    않고 우리가 재생성하지도 못한다). 다만 그것으로 **실행을 막지는 않는다** — 무엇으로 쟀는지를
    남기는 증거일 뿐이다.
    """
    with open(path, encoding="utf-8") as handle:
        doc = json.load(handle)
    tools = doc.setdefault("tools", {})
    entry = tools.setdefault("guidellm", {})
    versions = entry.setdefault("versions", {})
    row = versions.setdefault(res["version"], {})
    row["image_ref"] = res["image_ref"]
    row["version_source"] = res["version_source"]
    if res.get("published_utc"):
        row["published_utc"] = res["published_utc"]
    if digest:
        row["index_digest"] = digest
        row["digest_source"] = "measured(docker image inspect .RepoDigests)"
    entry["default_version"] = res["version"]
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(doc, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    return path


def _self_test():
    failures = []

    def check(name, cond, detail=""):
        if cond:
            print("  [PASS] %s" % name)
        else:
            print("  [FAIL] %s %s" % (name, detail)); failures.append(name)

    check("T1 태그 v0.7.3 → 0.7.3", version_from_tag("v0.7.3") == "0.7.3")
    check("T2 v 없는 태그도 읽는다", version_from_tag("0.8.0") == "0.8.0")
    for bad in ("", None, "release-candidate", "v", "latest"):
        try:
            version_from_tag(bad)
            check("T3 형태가 아니면 거부(%r)" % (bad,), False, "(예외가 나지 않았다)")
        except (ValueError, TypeError):
            check("T3 형태가 아니면 거부(%r)" % (bad,), True)
    check("T4 이미지 참조 조립", image_ref("0.7.3") == "ghcr.io/vllm-project/guidellm:v0.7.3")

    # 최신 해소는 상류 응답에서만 읽는다(추측 ✗). 네트워크는 주입으로 대체한다.
    class _Fake:
        def __init__(self, payload):
            self.payload = payload

        def __enter__(self):
            import io as _io
            return _io.StringIO(json.dumps(self.payload))

        def __exit__(self, *a):
            return False

    rel = fetch_latest(opener=lambda req, timeout=None: _Fake(
        {"tag_name": "v0.9.1", "published_at": "2026-08-30T12:00:00Z"}))
    res = resolution(version_from_tag(rel["tag_name"]),
                     "measured(GitHub Releases latest)", published_utc=rel.get("published_at"))
    check("T5 최신 릴리즈 해소 → 버전·이미지·출처",
          res["version"] == "0.9.1" and res["image_ref"].endswith(":v0.9.1")
          and res["published_utc"] == "2026-08-30T12:00:00Z"
          and res["version_source"].startswith("measured("))
    dec = resolution("0.7.3", "declared(--version)")
    check("T6 선언 override 는 출처가 declared 다", dec["version_source"] == "declared(--version)"
          and dec["published_utc"] is None)

    # 기록 병합 — 기존 항목을 지우지 않고 더한다.
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        path = os.path.join(d, "rec.json")
        with open(path, "w", encoding="utf-8") as handle:
            json.dump({"schema_version": 1, "tools": {"guidellm": {"versions": {
                "0.7.3": {"image_ref": "ghcr.io/x:v0.7.3", "index_digest": "sha256:old"}}}}}, handle)
        merge_record(path, res, digest="sha256:new")
        doc = json.load(open(path, encoding="utf-8"))
        vs = doc["tools"]["guidellm"]["versions"]
        check("T7 기록 병합이 옛 버전을 지우지 않는다", "0.7.3" in vs and "0.9.1" in vs)
        check("T8 새 기본 버전과 digest 가 기록된다",
              doc["tools"]["guidellm"]["default_version"] == "0.9.1"
              and vs["0.9.1"]["index_digest"] == "sha256:new")

    # 배포 기록 파일이 이 스키마를 만족하는지(픽스처만 보고 통과하는 것을 막는다)
    try:
        real = json.load(open(RECORD_PATH, encoding="utf-8"))
        gl = real["tools"]["guidellm"]
        check("T9 배포 기록이 versions/default_version 형태다",
              isinstance(gl.get("versions"), dict) and gl.get("default_version") in gl["versions"])
    except (OSError, ValueError, KeyError, TypeError) as exc:
        check("T9 배포 기록이 versions/default_version 형태다", False, "(%s)" % exc)

    if failures:
        sys.stderr.write("[resolve_guidellm --self-test] FAIL %d 건: %s\n" % (len(failures), failures))
        return 1
    print("[resolve_guidellm --self-test] OK — T1~T9 전부 통과")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description="GuideLLM 버전 해소(기본 = 최신 릴리즈 · 선언 override)")
    ap.add_argument("--version", default=None, help="이 버전을 쓴다(네트워크 조회 ✗)")
    ap.add_argument("--repo", default=REPO_DEFAULT)
    ap.add_argument("--record", action="store_true", help="해소 결과를 bench_tool_pin.json 에 병합")
    ap.add_argument("--record-path", default=RECORD_PATH)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)
    if args.self_test:
        return _self_test()

    if args.version:
        res = resolution(args.version, "declared(--version)", repo=args.repo)
    else:
        try:
            rel = fetch_latest(args.repo)
            res = resolution(version_from_tag(rel.get("tag_name")),
                             "measured(GitHub Releases latest · %s)" % (args.repo,),
                             repo=args.repo, published_utc=rel.get("published_at"))
        except (urllib.error.URLError, OSError, TimeoutError) as exc:
            sys.stderr.write(
                "[resolve_guidellm] UNRESOLVED 최신 릴리즈를 조회하지 못했다(%s: %s).\n"
                "  → 조용히 옛 버전으로 떨어지지 않는다. 쓸 버전을 선언하라: --version <x.y.z>\n"
                % (type(exc).__name__, exc))
            return RC_UNRESOLVED
        except ValueError as exc:
            sys.stderr.write("[resolve_guidellm] ERROR %s\n" % exc)
            return RC_ARGS

    if args.record:
        res["record_path"] = merge_record(args.record_path, res)
    if args.json:
        print(json.dumps(res, ensure_ascii=False, indent=2))
    else:
        print("guidellm %s (%s)\n  image: %s\n  %s"
              % (res["version"], res["version_source"], res["image_ref"], res["pull_command"]))
    return RC_OK


if __name__ == "__main__":
    sys.exit(main())
