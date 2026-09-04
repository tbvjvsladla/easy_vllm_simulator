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
# ★ 드리프트도 fail-closed(exit 4)다. "태그는 맞는데 내용이 다르다" 를 통과시키면 재현성
#   인증서가 **어느 도구로 쟀는지 모르는 상태**로 발행된다. 핀을 갱신하려면 핀 파일을 고치고
#   그 변경을 리뷰에 태운다 — 실행 시점에 조용히 흡수하지 않는다.
#
# 사용: resolve_bench_tool.py [--tool guidellm] [--pin PATH] [--platform linux/arm64] [--json]
#       resolve_bench_tool.py --self-test
# 종료: 0=일치 · 2=인자/핀 오류 · 3=이미지 부재(사전 스테이징 필요) · 4=digest 드리프트
import argparse
import json
import os
import subprocess
import sys

DEFAULT_TOOL = "guidellm"
PIN_RELATIVE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "bench_tool_pin.json")

# 종료코드는 **개별 대입**으로 둔다 — 튜플 대입은 policy_registry 의 식별자 추출기가 잡지 못해
# 정책 증거로 배선할 수 없다(추출기가 인정하는 것은 변수 대입·함수명·--longopt case 뿐).
RC_OK = 0        # 핀과 일치
RC_ARGS = 2      # 인자·핀 파일 오류
RC_ABSENT = 3    # 이미지 부재 — 사전 스테이징 필요(자동 pull ✗)
RC_DRIFT = 4     # 태그는 같은데 내용이 다르다


class PinError(ValueError):
    """핀 선언이 판정 가능한 형태가 아니다."""


def load_pin(path, tool):
    try:
        with open(path, encoding="utf-8") as handle:
            doc = json.load(handle)
    except (OSError, ValueError) as exc:
        raise PinError("핀 파일을 읽을 수 없다(%s): %s" % (path, exc)) from exc
    tools = doc.get("tools")
    if not isinstance(tools, dict) or tool not in tools:
        raise PinError("핀 파일에 도구 %r 이 없다(있는 것: %s)"
                       % (tool, sorted(tools) if isinstance(tools, dict) else "없음"))
    entry = tools[tool]
    for key in ("image_ref", "index_digest", "version", "digest_source"):
        value = entry.get(key)
        if not isinstance(value, str) or not value.strip():
            raise PinError("핀 %s.%s 이 비었다 — 출처 없는 핀은 증거가 아니다" % (tool, key))
    if not entry["index_digest"].startswith("sha256:"):
        raise PinError("핀 %s.index_digest 가 sha256: 로 시작하지 않는다(%r)"
                       % (tool, entry["index_digest"]))
    return entry


def accepted_digests(entry):
    """핀이 인정하는 digest 집합 = 인덱스 + **모든** 플랫폼 매니페스트.

    로컬 docker 는 매니페스트 리스트를 태그로 당기면 RepoDigests 에 **인덱스** digest 를 적고,
    플랫폼 매니페스트를 직접 당기면 그 digest 를 적는다. 둘 다 같은 핀을 가리키므로 둘 다 받는다
    — 하나만 받으면 정상 노드가 드리프트로 오판된다.

    플랫폼으로 **좁히지 않는** 이유: 이 함수가 답하는 질문은 "이것이 핀한 도구인가"이지 "아키텍처가
    맞는가"가 아니다. 후자는 docker 가 스스로 강제한다(외래 아키텍처 이미지는 애초에 안 돈다).
    여기서 좁히면 가드가 답할 수 없는 질문에 답하려다 정상 노드를 드리프트로 오판한다.
    """
    digests = {entry["index_digest"]}
    per_platform = entry.get("platform_digests")
    if isinstance(per_platform, dict):
        digests.update(v for v in per_platform.values() if isinstance(v, str) and v)
    return digests


def classify(entry, repo_digests):
    """로컬 관측 → 판정. 순수 함수(docker 를 부르지 않는다)."""
    if repo_digests is None:
        return RC_ABSENT, None
    observed = {d.split("@", 1)[1] for d in repo_digests if "@" in d}
    accepted = accepted_digests(entry)
    hit = sorted(observed & accepted)
    if hit:
        return RC_OK, hit[0]
    return RC_DRIFT, (sorted(observed)[0] if observed else None)


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

    rc, seen = classify(entry, None)
    check("P1 이미지 부재 → RC_ABSENT(자동 pull ✗)", rc == RC_ABSENT and seen is None)

    rc, seen = classify(entry, ["example/tool@sha256:aaa"])
    check("P2 인덱스 digest 일치 → OK", rc == RC_OK and seen == "sha256:aaa")

    rc, seen = classify(entry, ["example/tool@sha256:bbb"])
    check("P3 플랫폼(arm64) digest 일치 → OK", rc == RC_OK and seen == "sha256:bbb")

    rc, seen = classify(entry, ["example/tool@sha256:ccc"])
    check("P4 다른 플랫폼(amd64) digest 도 핀 안이면 OK(아키텍처는 docker 가 강제한다)",
          rc == RC_OK)

    rc, seen = classify(entry, ["example/tool@sha256:dead"])
    check("P5 핀 밖 digest → RC_DRIFT(태그가 같아도 통과 ✗)",
          rc == RC_DRIFT and seen == "sha256:dead")

    rc, _ = classify(entry, [])
    check("P6 RepoDigests 빈 리스트 → 드리프트(로컬 빌드 등 신원 불명)", rc == RC_DRIFT)

    for broken, fragment in (
            ({"tools": {}}, "도구"),
            ({"tools": {"t": {"image_ref": "a", "index_digest": "sha256:x", "version": "1"}}},
             "digest_source"),
            ({"tools": {"t": {"image_ref": "a", "index_digest": "x", "version": "1",
                              "digest_source": "m"}}}, "sha256:")):
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

    # 배포되는 실제 핀이 스키마를 만족하는지 — 픽스처만 보고 통과하는 것을 막는다.
    try:
        real = load_pin(PIN_RELATIVE, DEFAULT_TOOL)
        check("P8 배포 핀이 스키마를 만족한다", real["image_ref"].startswith("ghcr.io/"))
    except PinError as exc:
        check("P8 배포 핀이 스키마를 만족한다", False, "(%s)" % exc)

    if failures:
        sys.stderr.write("[resolve_bench_tool --self-test] FAIL %d 건: %s\n" % (len(failures), failures))
        return 1
    print("[resolve_bench_tool --self-test] OK — P1~P8 전부 통과")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description="측정 도구 컨테이너 핀 해소(digest 단일 권위)")
    ap.add_argument("--tool", default=DEFAULT_TOOL)
    ap.add_argument("--pin", default=PIN_RELATIVE)
    ap.add_argument("--platform", default=None,
                    help="예 linux/arm64 — 출력 주석용. 판정을 좁히지 않는다(위 accepted_digests 주석)")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()

    try:
        entry = load_pin(args.pin, args.tool)
    except PinError as exc:
        sys.stderr.write("[resolve_bench_tool] ERROR %s\n" % exc)
        return RC_ARGS

    rc, observed = classify(entry, _inspect(entry["image_ref"]))
    result = {
        "tool": args.tool,
        "version": entry["version"],
        "image_ref": entry["image_ref"],
        "pinned_digest": entry["index_digest"],
        "observed_digest": observed,
        "digest_source": "measured(docker image inspect .RepoDigests)",
        "pin_digest_source": entry["digest_source"],
        "platform_note": args.platform,
        "status": {RC_OK: "match", RC_ABSENT: "absent", RC_DRIFT: "drift"}[rc],
    }
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))

    if rc == RC_ABSENT:
        sys.stderr.write(
            "[resolve_bench_tool] ABSENT %s 가 이 노드에 없다. 자동 pull 하지 않는다"
            "(airgap-safe 불변식). 사전 스테이징:\n  docker pull %s\n"
            % (entry["image_ref"], entry["image_ref"]))
    elif rc == RC_DRIFT:
        sys.stderr.write(
            "[resolve_bench_tool] DRIFT %s 의 내용이 핀과 다르다(관측 %s · 핀 %s). "
            "실행하지 않는다 — 핀을 갱신하려면 bench_tool_pin.json 을 고쳐 리뷰에 태워라.\n"
            % (entry["image_ref"], observed, entry["index_digest"]))
    return rc


if __name__ == "__main__":
    sys.exit(main())
