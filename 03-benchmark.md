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
| `HINT_MISSING_SLAVE_ATTESTATION` | 슬레이브 ABI attestation 부재 — 멀티에서 두 노드가 같은 것을 돌렸다는 증거가 성공 경로에 보존되지 않았다. |

_동시성별 곡선도 없다(벤치 리포트 부재)._

## like-with-like 한정자 (Agent)

**말할 수 있는 것.** 이 수치는 `nv4-bf-262k-res` 한 형상의 관측이다 — vLLM 0.29.0rc6 + 자체이식 3종,
NVFP4 가중치, KV `auto(BF16)`, `max-model-len 262144`, TP=2 Ray executor, GB10 2노드, 컨테이너
profile 은 eager · async off, speculative 는 **off**. `32.53` t/s 는 그 조건에서 나온 값이고,
같은 조건을 세운 사람은 이 값 근처를 기대해도 된다. 판정은 `verdict=PASS · authority=explore · floor=3.10` 이다.

**말할 수 없는 것.** 인증서가 없다 — full 모드 verdict 로 봉인된 계측이 아니므로 이 수치를
**baseline 이나 권고로 승격하지 마라**. 부재는 '느렸다' 가 아니라 '그 형태로 판정되지 않았다' 는
뜻이다. 슬레이브 ABI attestation 도 성공 경로에 보존되지 않아, 두 노드가 같은 것을 돌렸다는 증거는
이미지 태그 동일성까지만이다.

**비교하려면 맞춰야 하는 축.** 동시성(이 캠페인은 conc1 을 대표값으로 적었다) · speculative 유무 ·
KV dtype · `max-model-len` · TP · 그리고 **측정 도구**. 특히 speculative 는 동시성에 따라 부호가
뒤집힌다 — 같은 캠페인에서 MTP 를 켠 셀이 conc1 에서는 앞서고(35.39 vs 32.53) conc2 부터는
뒤진다(29.53 vs 31.40) (../testlog/testlog_26091009_qwen38fn_24셀_판정.md §measured 셀 동시성 벡터). 한 점만 보고 레버의 우열을 말하면 틀린다.

**이 셀의 동시성 벡터.** 1=32.53 · 2=31.40 · 4=24.40 · 8=18.91 · 16=11.81

**accept_len.** 해당 없음 — speculative 를 끈 형상이다.

**여정으로서의 값.** 이 태그가 나르는 가장 싼 정보는 수치가 아니라 **21셀이 어떻게 무너졌는가**다.
§2 의 세 원인은 같은 하드웨어에서 같은 매트릭스를 짜려는 사람이 그대로 피할 수 있는 벽이다.
