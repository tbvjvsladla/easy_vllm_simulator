#!/usr/bin/env python3
"""adversarial_stress.py — 방어체계 시험용 **가압기** (plan_26080121 램프 구동).

★ 이것은 벤치마커가 아니다. `sweep_bench.sh` 는 처리량을 **측정**하려고 적응 상한 클램프를 두고
  레벨 실패 시 상위를 중단한다 — 목적이 정반대다. 여기는 방어가 발동하는지 보려고 **압박을 키운다**.

★ 무엇을 시험하는가: 모델 서빙은 치구이고 피시험자는 노드블랙박스 + 에이전트 예방 가드다.
  따라서 이 가압기의 성공은 "높은 t/s"가 아니라 **방어층이 실제로 개입하게 만드는 것**이다.
  개입 없이 끝나면 그것도 산출물이다(= 이 구성에서는 압박이 트립 지점에 못 닿는다는 실측).

★ stop-file 계약(필수): agent_guard 의 사다리 ① halt_load 가 이 파일을 만든다. 가압기는 매
  요청 발행 전에 확인하고 즉시 발행을 멈춘다. **이 배선이 없으면 가드의 첫 행동이 무효**이고,
  그러면 사다리는 캐시 회수부터 시작하게 된다(수요를 안 줄이고 공급만 회수 = 설계 위반).

압박 축(모델마다 다르게 조합한다 — 사용자 지시 "더 다양한 환경으로 가혹하게"):
  · concurrency  : 동시 요청 수. 엔진이 실제로 잡을 수 있는 값을 넘겨 큐를 쌓는다(과다구독).
  · prompt_tokens: 프리필 활성메모리. KV 절대클램프가 **덮지 않는** 축이다.
  · max_tokens   : 디코드 길이 → KV 점유 지속시간.
  · multimodal   : 이미지 페이로드. 인코더 캐시·전처리는 호스트측이라 KV 클램프 밖이다.

종료코드: 0=정상 종료 · 2=stop-file 로 중단 · 3=엔드포인트 소실(서빙 사망 의심)
"""
from __future__ import annotations

import argparse
import base64
import json
import os
import random
import sys
import threading
import time
import urllib.error
import urllib.request

_stop = threading.Event()
_lock = threading.Lock()
_stat = {"sent": 0, "ok": 0, "err": 0, "conn_err": 0, "last_err": ""}

# 1x1 PNG. 멀티모달 경로를 **깨우는 것**이 목적이지 이미지 내용이 목적이 아니다.
_PNG_1X1 = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
)


def _filler(tokens: int, seed: int) -> str:
    """대략 <tokens> 토큰 분량의 비반복 텍스트. 반복문자열은 엔진 prefix-cache 에 먹혀
    프리필 압박이 사라지므로 **일부러 랜덤화**한다(압박이 목적이니까)."""
    rnd = random.Random(seed)
    words = ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel",
             "india", "juliet", "kilo", "lima", "mike", "november", "oscar", "papa"]
    return " ".join(rnd.choice(words) for _ in range(max(1, int(tokens * 0.75))))


def _payload(args, seed: int) -> dict:
    content: object = _filler(args.prompt_tokens, seed)
    if args.multimodal:
        b64 = base64.b64encode(_PNG_1X1).decode()
        content = [
            {"type": "text", "text": _filler(args.prompt_tokens, seed)},
            {"type": "image_url", "image_url": {"url": "data:image/png;base64," + b64}},
        ]
    return {
        "model": args.model,
        "messages": [{"role": "user", "content": content}],
        "max_tokens": args.max_tokens,
        "temperature": 0.8,
        "stream": False,
    }


def _worker(args, wid: int) -> None:
    url = args.endpoint.rstrip("/") + "/v1/chat/completions"
    seed = wid * 7919
    while not _stop.is_set():
        # ★ 발행 직전에 stop-file 확인 — 가드의 halt_load 가 즉시 먹히는 지점이다.
        if args.stop_file and os.path.exists(args.stop_file):
            _stop.set()
            break
        seed += 1
        body = json.dumps(_payload(args, seed)).encode()
        req = urllib.request.Request(url, data=body,
                                     headers={"Content-Type": "application/json"})
        with _lock:
            _stat["sent"] += 1
        try:
            with urllib.request.urlopen(req, timeout=args.timeout_s) as r:
                r.read()
            with _lock:
                _stat["ok"] += 1
        except urllib.error.HTTPError as e:
            with _lock:
                _stat["err"] += 1
                _stat["last_err"] = "HTTP %s" % e.code
        except (urllib.error.URLError, OSError, TimeoutError) as e:
            with _lock:
                _stat["conn_err"] += 1
                _stat["last_err"] = str(e)[:120]


def _main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="방어체계 시험용 가압기 (plan_26080121)")
    # ★ --self-test 는 하드웨어·엔드포인트 불요여야 한다(node_blackbox 도구 공통 계약).
    #   required=True 면 자체시험조차 라이브 서빙을 요구해 배포검증에서 못 돌린다.
    ap.add_argument("--endpoint", help="예: http://localhost:8951 (--self-test 외 필수)")
    ap.add_argument("--model", help="served-model-name (--self-test 외 필수)")
    ap.add_argument("--concurrency", type=int, default=8)
    ap.add_argument("--prompt-tokens", type=int, default=4096)
    ap.add_argument("--max-tokens", type=int, default=256)
    ap.add_argument("--multimodal", action="store_true", help="이미지 페이로드 동봉(KV 클램프 밖 축)")
    ap.add_argument("--duration-s", type=float, default=300)
    ap.add_argument("--timeout-s", type=float, default=600)
    ap.add_argument("--stop-file", help="이 파일이 생기면 즉시 발행 중단(agent_guard halt_load 계약)")
    ap.add_argument("--report-s", type=float, default=15, help="진행 보고 주기")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args(argv)

    if a.self_test:
        ok = True

        def chk(c, label):
            nonlocal ok
            print("  [%s] %s" % ("PASS" if c else "FAIL", label))
            ok = ok and bool(c)

        f1, f2 = _filler(100, 1), _filler(100, 2)
        chk(f1 != f2, "seed 별 프롬프트가 다르다(prefix-cache 회피)")
        chk(len(f1.split()) == 75, "prompt_tokens→단어수 근사(0.75배)")
        ns = argparse.Namespace(model="m", prompt_tokens=32, max_tokens=8, multimodal=False)
        chk(isinstance(_payload(ns, 1)["messages"][0]["content"], str), "텍스트 모드 = 문자열 content")
        ns.multimodal = True
        c = _payload(ns, 1)["messages"][0]["content"]
        chk(isinstance(c, list) and any(x.get("type") == "image_url" for x in c),
            "멀티모달 모드 = image_url 포함")
        chk(len(_PNG_1X1) > 0 and _PNG_1X1[:8] == b"\x89PNG\r\n\x1a\n", "내장 PNG 시그니처 유효")
        print("self-test: %s" % ("PASS" if ok else "FAIL"))
        return 0 if ok else 2

    missing = [f for f, v in (("--endpoint", a.endpoint), ("--model", a.model)) if not v]
    if missing:
        raise SystemExit("실행 모드에는 %s 가 필요하다(--self-test 는 예외)" % ", ".join(missing))

    if a.stop_file and os.path.exists(a.stop_file):
        os.remove(a.stop_file)          # 이전 런의 잔재로 즉시 멈추는 걸 막는다

    print("[stress] endpoint=%s model=%s conc=%d prompt≈%dtok max_tokens=%d mm=%s dur=%.0fs"
          % (a.endpoint, a.model, a.concurrency, a.prompt_tokens, a.max_tokens,
             a.multimodal, a.duration_s))
    threads = [threading.Thread(target=_worker, args=(a, i), daemon=True)
               for i in range(a.concurrency)]
    for t in threads:
        t.start()

    t0 = time.monotonic()
    last = t0
    while time.monotonic() - t0 < a.duration_s and not _stop.is_set():
        time.sleep(0.5)
        if time.monotonic() - last >= a.report_s:
            last = time.monotonic()
            with _lock:
                s = dict(_stat)
            mem = "?"
            try:
                with open("/proc/meminfo", encoding="utf-8") as fh:
                    for line in fh:
                        if line.startswith("MemAvailable:"):
                            mem = "%d" % (int(line.split()[1]) // 1024)
                            break
            except OSError:
                pass
            print("[stress] t=%4.0fs sent=%d ok=%d err=%d conn_err=%d mem=%sMiB %s"
                  % (time.monotonic() - t0, s["sent"], s["ok"], s["err"],
                     s["conn_err"], mem, s["last_err"][:60]))
            sys.stdout.flush()

    stopped_by_file = _stop.is_set()
    _stop.set()
    for t in threads:
        t.join(timeout=5)
    with _lock:
        s = dict(_stat)
    print("[stress] 종료 sent=%d ok=%d err=%d conn_err=%d%s"
          % (s["sent"], s["ok"], s["err"], s["conn_err"],
             "  ← stop-file 로 중단(가드 halt_load)" if stopped_by_file else ""))
    if stopped_by_file:
        return 2
    # 연결오류가 성공보다 많으면 엔드포인트 소실 의심 — 조용히 0 을 반환하지 않는다.
    if s["conn_err"] > s["ok"]:
        print("[stress] WARN: 연결오류(%d) > 성공(%d) — 서빙 사망 의심" % (s["conn_err"], s["ok"]),
              file=sys.stderr)
        return 3
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())
