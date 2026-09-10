# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> **인증서가 없다.** 이 절은 인증서 없이 발행된다 — 그것이 정책이다(강행 발행).
> 인증서는 full 모드 verdict==PASS 일 때만 나오며, 그 발행은 `adversarial-benchmark` 의
> 책임이다. 부재는 '성능이 나빴다' 가 아니라 '**그 형태로 판정되지 않았다**' 는 뜻이다.

> ⚠ 아래 정보가 **부재한 채로 발행**됐다. 부재는 실패가 아니라 **기록**이며, 이 태그를
> 소비할 때 그만큼을 모른 채 소비한다는 뜻이다(강행 발행 정책 · plan_26090616 Q7/Q8).

| 사유코드 | 뜻 |
|---|---|
| `HINT_MISSING_CERTIFICATE` | 인증서 부재 — full PASS 가 아니었거나 벤치마커가 발행하지 않았다. 인증서 발행은 adversarial-benchmark 의 책임이지 발행기의 책임이 아니다. |
| `HINT_MISSING_BENCH_REPORT` | 벤치 리포트 부재 — 동시성별 곡선을 실을 수 없다. |
| `HINT_MISSING_LITE` | lite 관측 부재. |

_동시성별 곡선도 없다(벤치 리포트 부재)._

## like-with-like 한정자 (Agent)

OBSERVATION-ONLY — 아래 수치는 **관측 게재**이지 baseline 이 아니다. 이 태그는 성능 우위를
주장하지 않으며 baseline 승격 통로는 발행기가 막고 있다(`task_class=hint_map_only`).

**말할 수 있는 것**
- 같은 11.0 GiB 에서 이 dtype 이 연 KV 풀 = **232,992 토큰(2.91×)** — 엔진 보고 실측이고,
  같은 모델·같은 max-model-len 이면 다른 HW 에서도 같은 비가 나온다(**arch-invariant**).
- 8192+1024 워크로드의 상주 요청 상한 **25건**, 확보 가능한 최대 context **131K**.
- 스트림당 decode tok/s(동시성 1/2/4) = **18.56 / 18.75 / 14.59** · judge PASS (explore · floor 10.1 · expected_achievable 11.88).

**말할 수 없는 것**
- 실제 24GB 디스크리트 카드에서의 절대 성능. 아키텍처를 모의하지 않았다(**arch-scaled**).
- 품질(정확도) 영향. 이번 범위 밖이며 KV 를 3bit 까지 내리고도 품질을 안 쟀다는 사실은 **결손**이다.
- 동시성 8 이상의 곡선. 열 보호가 먼저 걸려 측정 자체가 성립하지 않았다.

**like-with-like 로 비교하려면** 같은 `--kv-cache-memory-bytes`, 같은 `max-model-len`, 같은 입출력
길이, 같은 prefix-hit 상한(≤2%)을 맞춰라. 넷 중 하나라도 다르면 이 표와 비교하지 마라.

PERF-WARNING: 인증서 없음(authority=explore 는 인증서를 발행하지 않는다) · 서브 셀의 judge 원본 JSON 미회수 — 성능 수치는 회수된 서브 testlog 의 관측이며 이 태그는 인증서급 재현성·외부 대비 우위를 주장하지 않는다
