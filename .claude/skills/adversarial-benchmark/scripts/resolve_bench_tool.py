#!/usr/bin/env python3
# resolve_bench_tool.py — 측정 도구 컨테이너 핀 해소 (plan_26090415 §3.5 · CP3)
#
# 핀 파일(`../bench_tool_pin.json`)이 선언한 이미지가 **이 노드에 실제로 있고 그 내용이 핀과
# 같은지**를 확인한다. 판정 권위는 digest 단일이다 — 태그는 사람이 읽는 표기이고, 같은 태그가
# 메인과 서브에서 다른 이미지를 가리킨 실측 선례가 있다.
#
# ★ 부재 시 **자동 pull 하지 않는다.** `run_bench.sh` 헤더가 airgap-safe 를 명시 불변식으로
#   선언하는데, full 을 별도 컨테이너로 옮기면서 조용한 pull 을 넣으면 그 불변식이 깨진다.
#   계약은 **사전 스테이징 + 부재 시 fail-closed** 이며, 이 스크립트는 부재를 exit 3 으로
#   알리고 사람이 실행할 정확한 명령을 출력한다(조용한 진행 ✗ · 조용한 네트워크 접근 ✗).
#
# ★ 2026-09-05(plan_26090516 ③ 3-8 · audit G-C3): **드리프트 게이트(exit 4)는 삭제됐다.**
#   종전에는 기록과 다른 digest 면 실행을 거부했는데, 그 규칙은 "기본은 최신 릴리즈" 라는 결정과
#   충돌해 버전을 올릴 때마다 하네스를 고쳐야 했다(핀=버전고정). digest 는 이제 **관문이 아니라
#   측정 provenance** 다 — 실제로 돈 이미지의 digest 를 결과에 실어 리포트·인증서가 그것을 적는다.
#   재현성은 "고정했다" 가 아니라 "무엇으로 쟀는지 적혀 있다" 로 성립한다.
#
# 사용: resolve_bench_tool.py [--tool guidellm] [--tool-version 0.7.3] [--pin PATH] [--json]
#       resolve_bench_tool.py --self-test
# 종료: 0=해소(로컬에 있다) · 2=인자/기록 오류 · 3=이미지 부재(사전 스테이징 필요)
import argparse
import json
import os
import subprocess
import sys

DEFAULT_TOOL = "guidellm"
PIN_RELATIVE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "bench_tool_pin.json")

# 종료코드는 **개별 대입**으로 둔다 — 튜플 대입은 policy_registry 의 식별자 추출기가 잡지 못해
# 정책 증거로 배선할 수 없다(추출기가 인정하는 것은 변수 대입·함수명·--longopt case 뿐).
RC_OK = 0        # 로컬에 있다 — digest 를 기록해 진행한다
RC_ARGS = 2      # 인자·기록 파일 오류
RC_ABSENT = 3    # 이미지 부재 — 사전 스테이징 필요(자동 pull ✗ · airgap-safe 불변식)


class PinError(ValueError):
    """기록이 판정 가능한 형태가 아니다."""


def load_pin(path, tool, version=None):
    """기록에서 이 도구·버전의 항목을 읽는다(schema_version 2 · `versions{}` 형태).

    version 미지정이면 `default_version` 을 쓴다 — **그것이 마지막으로 스테이징한 버전**이며,
    최신 릴리즈 해소는 실행 평면이 아니라 `resolve_guidellm.py`(스테이징 평면)가 한다.
    벤치 도중에 상류를 조회하지 않는다(airgap-safe).
    """
    try:
        with open(path, encoding="utf-8") as handle:
            doc = json.load(handle)
    except (OSError, ValueError) as exc:
        raise PinError("기록 파일을 읽을 수 없다(%s): %s" % (path, exc)) from exc
    tools = doc.get("tools")
    if not isinstance(tools, dict) or tool not in tools:
        raise PinError("기록에 도구 %r 이 없다(있는 것: %s)"
                       % (tool, sorted(tools) if isinstance(tools, dict) else "없음"))
    entry = tools[tool]
    versions = entry.get("versions")
    if not isinstance(versions, dict) or not versions:
        raise PinError("기록 %s.versions 가 비었다 — 스테이징한 버전이 없다"
                       "(resolve_guidellm.py --record 로 먼저 해소·스테이징하라)" % tool)
    want = version or entry.get("default_version")
    if want not in versions:
        raise PinError("기록에 %s 버전 %r 이 없다(있는 것: %s) — 버전을 스테이징하라"
                       % (tool, want, sorted(versions)))
    row = dict(versions[want])
    row["version"] = want
    if not isinstance(row.get("image_ref"), str) or not row["image_ref"].strip():
        raise PinError("기록 %s.%s.image_ref 가 비었다" % (tool, want))
    idx = row.get("index_digest")
    if idx is not None and not (isinstance(idx, str) and idx.startswith("sha256:")):
        raise PinError("기록 %s.%s.index_digest 가 sha256: 로 시작하지 않는다(%r)" % (tool, want, idx))
    return row


def accepted_digests(entry):
    """기록이 **아는** digest 집합 = 인덱스 + 모든 플랫폼 매니페스트. 판정이 아니라 대조용이다.

    로컬 docker 는 매니페스트 리스트를 태그로 당기면 RepoDigests 에 **인덱스** digest 를 적고,
    플랫폼 매니페스트를 직접 당기면 그 digest 를 적는다. 둘 다 같은 핀을 가리키므로 둘 다 받는다
    — 하나만 받으면 정상 노드가 드리프트로 오판된다.

    플랫폼으로 **좁히지 않는** 이유: 이 함수가 답하는 질문은 "이것이 핀한 도구인가"이지 "아키텍처가
    맞는가"가 아니다. 후자는 docker 가 스스로 강제한다(외래 아키텍처 이미지는 애초에 안 돈다).
    여기서 좁히면 가드가 답할 수 없는 질문에 답하려다 정상 노드를 드리프트로 오판한다.
    """
    digests = {entry["index_digest"]} if entry.get("index_digest") else set()
    per_platform = entry.get("platform_digests")
    if isinstance(per_platform, dict):
        digests.update(v for v in per_platform.values() if isinstance(v, str) and v)
    return digests


def classify(entry, repo_digests):
    """로컬 관측 → (rc, 관측 digest, 기록 일치 여부). 순수 함수(docker 를 부르지 않는다).

    2026-09-05: 기록과 다른 digest 도 **막지 않는다** — 그 사실을 세 번째 값으로 돌려주고
    호출자가 결과에 적는다. 부재만 fail-closed 다(자동 pull ✗).
    """
    if repo_digests is None:
        return RC_ABSENT, None, None
    observed = {d.split("@", 1)[1] for d in repo_digests if "@" in d}
    known = accepted_digests(entry)
    hit = sorted(observed & known)
    if hit:
        return RC_OK, hit[0], True
    seen = sorted(observed)[0] if observed else None
    return RC_OK, seen, (False if known else None)


def _inspect(image_ref):
    """로컬 이미지의 RepoDigests. 부재면 None. **pull 하지 않는다.**"""
    proc = subprocess.run(
        ["docker", "image", "inspect", image_ref, "--format", "{{json .RepoDigests}}"],
        capture_output=True, text=True, timeout=120)
    if proc.returncode != 0:
        return None
    try:
        value = json.loads(proc.stdout.strip() or "null")
    except ValueError:
        return None
    return value if isinstance(value, list) else None


def _self_test():
    failures = []

    def check(name, condition, detail=""):
        if condition:
            print("  [PASS] %s" % name)
        else:
            print("  [FAIL] %s %s" % (name, detail)); failures.append(name)

    entry = {"version": "0.7.3", "image_ref": "example/tool:v1", "digest_source": "measured(x)",
             "index_digest": "sha256:aaa",
             "platform_digests": {"linux/arm64": "sha256:bbb", "linux/amd64": "sha256:ccc"}}

    rc, seen, known = classify(entry, None)
    check("P1 이미지 부재 → RC_ABSENT(자동 pull ✗)", rc == RC_ABSENT and seen is None)

    rc, seen, known = classify(entry, ["example/tool@sha256:aaa"])
    check("P2 인덱스 digest 일치 → OK·기록일치", rc == RC_OK and seen == "sha256:aaa" and known is True)

    rc, seen, known = classify(entry, ["example/tool@sha256:bbb"])
    check("P3 플랫폼(arm64) digest 일치 → OK", rc == RC_OK and seen == "sha256:bbb")

    rc, seen, known = classify(entry, ["example/tool@sha256:ccc"])
    check("P4 다른 플랫폼(amd64) digest 도 기록 안이면 일치(아키텍처는 docker 가 강제한다)",
          rc == RC_OK and known is True)

    rc, seen, known = classify(entry, ["example/tool@sha256:dead"])
    check("P5 ★기록 밖 digest 도 **막지 않는다** — 관측을 그대로 싣고 불일치를 표시한다"
          "(버전 갱신이 하네스 수정을 부르지 않는다)",
          rc == RC_OK and seen == "sha256:dead" and known is False)

    rc, seen, known = classify(entry, [])
    check("P6 RepoDigests 가 비면 관측 digest 는 없음(로컬 빌드 등) — 그 사실만 남는다",
          rc == RC_OK and seen is None and known is False)

    rc, seen, known = classify({"image_ref": "x", "version": "9"}, ["x@sha256:zzz"])
    check("P6b 기록에 digest 가 아예 없으면 대조 자체가 성립하지 않는다(None)", known is None)

    for broken, fragment in (
            ({"tools": {}}, "도구"),
            ({"tools": {"t": {"versions": {}}}}, "비었다"),
            ({"tools": {"t": {"default_version": "1", "versions": {"1": {"image_ref": "a",
                                                                        "index_digest": "x"}}}}},
             "sha256:")):
        path = os.path.join(os.environ.get("TMPDIR", "/tmp"), "_pin_selftest.json")
        with open(path, "w", encoding="utf-8") as handle:
            json.dump(broken, handle)
        try:
            load_pin(path, "t")
        except PinError as exc:
            check("P7 불량 핀 거부(%s)" % fragment, fragment in str(exc), "(실제 %r)" % str(exc))
        else:
            check("P7 불량 핀 거부(%s)" % fragment, False, "(예외가 나지 않았다)")
        os.unlink(path)

    # 배포되는 실제 기록이 스키마를 만족하는지 — 픽스처만 보고 통과하는 것을 막는다.
    try:
        real = load_pin(PIN_RELATIVE, DEFAULT_TOOL)
        check("P8 배포 기록이 스키마를 만족한다(default_version 항목)",
              real["image_ref"].startswith("ghcr.io/") and real["version"])
    except PinError as exc:
        check("P8 배포 기록이 스키마를 만족한다", False, "(%s)" % exc)
    try:
        load_pin(PIN_RELATIVE, DEFAULT_TOOL, version="99.99.99")
        check("P9 스테이징하지 않은 버전 요청은 fail-loud", False, "(예외가 나지 않았다)")
    except PinError as exc:
        check("P9 스테이징하지 않은 버전 요청은 fail-loud", "스테이징하라" in str(exc))

    if failures:
        sys.stderr.write("[resolve_bench_tool --self-test] FAIL %d 건: %s\n" % (len(failures), failures))
        return 1
    print("[resolve_bench_tool --self-test] OK — P1~P9 전부 통과")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description="측정 도구 컨테이너 핀 해소(digest 단일 권위)")
    ap.add_argument("--tool", default=DEFAULT_TOOL)
    ap.add_argument("--tool-version", default=None,
                    help="쓸 버전(미지정 시 기록의 default_version = 마지막 스테이징분). "
                         "최신 해소는 스테이징 평면의 resolve_guidellm.py 가 한다.")
    ap.add_argument("--pin", default=PIN_RELATIVE)
    ap.add_argument("--platform", default=None,
                    help="예 linux/arm64 — 출력 주석용. 판정을 좁히지 않는다(위 accepted_digests 주석)")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()

    try:
        entry = load_pin(args.pin, args.tool, args.tool_version)
    except PinError as exc:
        sys.stderr.write("[resolve_bench_tool] ERROR %s\n" % exc)
        return RC_ARGS

    rc, observed, matches_record = classify(entry, _inspect(entry["image_ref"]))
    result = {
        "tool": args.tool,
        "version": entry["version"],
        "version_source": entry.get("version_source"),
        "image_ref": entry["image_ref"],
        "recorded_digest": entry.get("index_digest"),
        "observed_digest": observed,
        "digest_source": "measured(docker image inspect .RepoDigests)",
        "matches_record": matches_record,
        "platform_note": args.platform,
        "status": {RC_OK: "resolved", RC_ABSENT: "absent"}[rc],
    }
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))

    if rc == RC_ABSENT:
        sys.stderr.write(
            "[resolve_bench_tool] ABSENT %s 가 이 노드에 없다. 자동 pull 하지 않는다"
            "(airgap-safe 불변식). 사전 스테이징:\n  docker pull %s\n"
            % (entry["image_ref"], entry["image_ref"]))
    elif matches_record is False:
        sys.stderr.write(
            "[resolve_bench_tool] NOTE %s 의 로컬 digest(%s)가 기록(%s)과 다르다. "
            "막지 않는다 — 실제로 돈 digest 를 결과·리포트에 적는다(측정 provenance). "
            "기록을 갱신하려면 resolve_guidellm.py --record 를 쓴다.\n"
            % (entry["image_ref"], observed, entry.get("index_digest")))
    return rc


if __name__ == "__main__":
    sys.exit(main())
