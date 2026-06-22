#!/usr/bin/env python3
"""③ torch 핀(+SM arch) → 빌드트랙 제안 + torch_cuda_arch (제안자형, oracle 아님)

render_dockerfile.py 가 소비하지만 아무 스크립트도 생산하지 않던
resolved["build_track"]["decision"] · resolved["source_build"]["torch_cuda_arch"] 의
**생산자**다(침묵 소비 종료). resolve_torch_pin.py(①) → resolve_ngc_tag.py(②) 다음 단계.

⚠️ 순수 결정론이 아니다 — 두 출력의 성질이 다르다:
  • build_track.decision = **제안(proposal)**. 휴리스틱: torch 2.10대→'wheel' / 2.11+→'source-build'.
    하드 ABI 벽(_C 가 NGC torch 에 링크)이 근거지만, 최종 변별자는 **(NGC 베이스/실-링크 torch) × vLLM
    source version** 이며 진짜 합격/불합격은 **스모크가 중재**한다(arbiter="smoke"). NGC 오버라이드도 경험적.
  • source_build.torch_cuda_arch = **결정론**. manifest 의 GPU compute-capability(scan 산물; 없으면 인자)에서
    '12.1a'(sm_121a) 같은 TORCH_CUDA_ARCH_LIST 값을 단순 매핑으로 환산. --sm-arch 인자가 manifest 보다 우선.

출력(JSON, resolved.json 머지용):
  {"build_track":{"decision":"wheel|source-build","rationale":...,"arbiter":"smoke"},
   "source_build":{"torch_cuda_arch":"12.1a"}}

stdlib 만 사용(폐쇄망 — 외부 네트워크 없음). manifest.yaml 은 pyyaml 있으면 사용, 없으면 flat 미니파서.
사용:
  python3 resolve_build_track.py --torch-pin 2.11.0 --sm-arch sm_121a
  python3 resolve_build_track.py --torch-pin 2.10.0 --manifest manifest.yaml
  python3 resolve_build_track.py --self-test
"""
import sys
import json
import argparse

# build_track 제안 휴리스틱(검증된 ABI 벽 경계, memory: torch 2.10→wheel / 2.11+→source-build).
# 이건 **제안**이며 스모크가 최종 중재한다. 경계는 packaging 없이 (major, minor) 튜플 비교.
SOURCE_BUILD_MIN = (2, 11)  # torch 이 이상이면 source-build 제안(하드 ABI 벽)

# SM arch(compute-capability) 표기 → TORCH_CUDA_ARCH_LIST 토큰(결정론 매핑).
#   'sm_121a' → '12.1a' · 'sm_90a' → '9.0a' · '8.9' → '8.9'(이미 dot 표기면 그대로).
#   접미 'a'(arch-conditional, Hopper+ PTX) 는 보존. 매핑 미스 시 인자 그대로 통과(fail-soft, rationale 명기).


def norm_release(v):
    """'2.11.0a0+a6c236b' 같은 torch 핀 → (major, minor) 튜플. 실패 시 None."""
    if not v:
        return None
    head = str(v).strip()
    # 빌드메타/프리릴리스 접미 잘라내기: 첫 토큰의 숫자.숫자 만 취함
    digits = []
    cur = ""
    for ch in head:
        if ch.isdigit():
            cur += ch
        elif ch == "." and cur:
            digits.append(int(cur))
            cur = ""
            if len(digits) == 2:
                break
        else:
            break
    if cur and len(digits) < 2:
        digits.append(int(cur))
    if len(digits) >= 2:
        return (digits[0], digits[1])
    if len(digits) == 1:
        return (digits[0], 0)
    return None


def propose_track(torch_pin):
    """torch 핀 → {'decision','rationale'} (제안). arbiter 는 호출부에서 'smoke' 부착."""
    rel = norm_release(torch_pin)
    if rel is None:
        return {
            "decision": "source-build",
            "rationale": (f"torch 핀 '{torch_pin}' 파싱 실패 — 보수적으로 source-build 제안. "
                          f"스모크가 최종 중재."),
        }
    if rel >= SOURCE_BUILD_MIN:
        return {
            "decision": "source-build",
            "rationale": (f"torch {rel[0]}.{rel[1]} >= {SOURCE_BUILD_MIN[0]}.{SOURCE_BUILD_MIN[1]} "
                          f"→ 하드 ABI 벽(_C 를 NGC torch 에 링크) → source-build 제안. "
                          f"최종 변별자=(NGC 베이스/실-링크 torch)×vLLM source version, 스모크가 중재."),
        }
    return {
        "decision": "wheel",
        "rationale": (f"torch {rel[0]}.{rel[1]} < {SOURCE_BUILD_MIN[0]}.{SOURCE_BUILD_MIN[1]} "
                      f"→ pre-built wheel 트랙 제안(--no-deps 설치). 스모크가 중재."),
    }


def map_torch_cuda_arch(sm):
    """compute-capability 표기 → TORCH_CUDA_ARCH_LIST 토큰(결정론).
    'sm_121a'→'12.1a' · 'sm_90a'→'9.0a' · '12.1a'/'8.9'(이미 변환됨)→그대로."""
    if not sm:
        return None
    s = str(sm).strip()
    if s.lower().startswith("sm_"):
        s = s[3:]
    # arch-conditional 접미 'a'(또는 'f') 분리
    suffix = ""
    if s and s[-1].isalpha():
        suffix = s[-1]
        s = s[:-1]
    if "." in s:  # 이미 dot 표기(예 '12.1')
        return s + suffix
    if s.isdigit():
        # '121' → '12.1' · '90' → '9.0' · '89' → '8.9' (마지막 한 자리 = minor)
        if len(s) >= 2:
            return f"{int(s[:-1])}.{s[-1]}{suffix}"
        return f"{s}.0{suffix}"
    return str(sm).strip()  # 미인식 → 원문 통과(fail-soft)


# ── manifest 로더 (render_dockerfile.py 와 동일 관례: pyyaml 또는 flat 미니파서) ──
def load_manifest(path):
    try:
        import yaml  # type: ignore
        with open(path, encoding="utf-8") as f:
            return yaml.safe_load(f) or {}
    except ImportError:
        return _load_yaml_flat(path)
    except FileNotFoundError:
        return {}


def _load_yaml_flat(path):
    out = {}
    try:
        f = open(path, encoding="utf-8")
    except FileNotFoundError:
        return {}
    with f:
        for line in f:
            s = line.rstrip("\n")
            if not s.strip() or s.lstrip().startswith("#"):
                continue
            if s[0] in (" ", "\t", "-"):
                continue
            if ":" not in s:
                continue
            key, val = s.split(":", 1)
            key = key.strip()
            val = _strip_inline_comment(val).strip()
            if val in ("[]", ""):
                out[key] = [] if val == "[]" else ""
                continue
            if (val[0] == val[-1]) and val[0] in ("'", '"'):
                val = val[1:-1]
            out[key] = val
    return out


def _strip_inline_comment(val):
    in_q = ""
    for i, ch in enumerate(val):
        if ch in ("'", '"'):
            in_q = "" if in_q == ch else (in_q or ch)
        elif ch == "#" and not in_q and i > 0 and val[i - 1] == " ":
            return val[:i]
    return val


# manifest 에서 compute-capability 를 읽을 때 허용하는 키 별칭(scan 산물 — 미확정이라 관용).
_MANIFEST_SM_KEYS = ("compute_capability", "sm_arch", "gpu_compute_capability", "cuda_arch")


def sm_from_manifest(manifest):
    for k in _MANIFEST_SM_KEYS:
        v = manifest.get(k)
        if v:
            return str(v), k
    return None, None


def resolve(torch_pin, sm_arch):
    """torch 핀 + SM arch → resolved.json 머지 조각."""
    track = propose_track(torch_pin)
    track["arbiter"] = "smoke"  # 최종 중재자 명기(이 스크립트는 proposer)
    return {
        "build_track": track,
        "source_build": {"torch_cuda_arch": map_torch_cuda_arch(sm_arch)},
    }


# ── self-test ────────────────────────────────────────────────────────────────
def _self_test():
    # track 제안 휴리스틱
    assert resolve("2.11.0", None)["build_track"]["decision"] == "source-build"
    assert resolve("2.11.0a0+a6c236b", None)["build_track"]["decision"] == "source-build"
    assert resolve("2.12.0", None)["build_track"]["decision"] == "source-build"
    assert resolve("2.10.0", None)["build_track"]["decision"] == "wheel"
    assert resolve("2.9.1", None)["build_track"]["decision"] == "wheel"
    # arbiter 는 항상 smoke
    assert resolve("2.11.0", None)["build_track"]["arbiter"] == "smoke"
    # SM arch 결정론 매핑
    assert map_torch_cuda_arch("sm_121a") == "12.1a"
    assert map_torch_cuda_arch("121a") == "12.1a"
    assert map_torch_cuda_arch("sm_90a") == "9.0a"
    assert map_torch_cuda_arch("89") == "8.9"
    assert map_torch_cuda_arch("12.1a") == "12.1a"
    assert map_torch_cuda_arch(None) is None
    # 파싱 실패 → 보수적 source-build
    assert resolve("garbage", None)["build_track"]["decision"] == "source-build"
    print("[build_track] self-test OK — track 제안(wheel/source) + torch_cuda_arch 결정론 매핑 정상")


def main():
    ap = argparse.ArgumentParser(
        description="resolve_build_track.py — build_track 제안 + torch_cuda_arch (proposer; 스모크가 최종 중재)")
    ap.add_argument("--torch-pin", help="resolve_torch_pin.py 의 torch_pin (예: 2.11.0)")
    ap.add_argument("--sm-arch", default=None,
                    help="GPU compute-capability (예: sm_121a | 121a | 12.1a). manifest 보다 우선")
    ap.add_argument("--manifest", default="manifest.yaml",
                    help="--sm-arch 미지정 시 여기서 compute-capability 읽음")
    ap.add_argument("--self-test", action="store_true", help="내장 self-test")
    a = ap.parse_args()

    if a.self_test:
        _self_test()
        return

    if not a.torch_pin:
        print(json.dumps({"error": "--torch-pin 필요 (resolve_torch_pin.py 의 torch_pin)"},
                         ensure_ascii=False), file=sys.stderr)
        sys.exit(2)

    sm_arch = a.sm_arch
    sm_source = "--sm-arch"
    if not sm_arch:
        manifest = load_manifest(a.manifest)
        sm_arch, key = sm_from_manifest(manifest)
        sm_source = f"manifest[{key}]" if key else None

    out = resolve(a.torch_pin, sm_arch)
    # 출처 메타(디버그용 — render 는 무시). torch_cuda_arch 없으면 fail-loud 힌트만 남김(중단 X).
    out["source_build"]["sm_arch_input"] = sm_arch
    out["source_build"]["sm_arch_source"] = sm_source
    if out["source_build"]["torch_cuda_arch"] is None:
        out["source_build"]["note"] = (
            "torch_cuda_arch 미해소 — --sm-arch 또는 manifest compute_capability 필요 "
            "(source-build 트랙이면 TORCH_CUDA_ARCH_LIST 빈값 → 렌더 fail-loud)")
    print(json.dumps(out, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
