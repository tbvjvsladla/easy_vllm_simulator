# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 4건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/testlog/testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b.md` | testlog | 26101015 | 0 | 3760 | 0.26.0 · c-tq4nc · easy-vllm:0.26.0-cu132-aarch64-source · tq4nc | — |
| 2 | `docs/devlog/devlog_26101108_c-tq4nc_셀D_서빙_측정_knob해소.md` | devlog | 26101108 | 0 | 3766 | 0.26.0 · c-tq4nc · easy-vllm:0.26.0-cu132-aarch64-source · tq4nc | — |
| 3 | `docs/benchmark/bench_report_26101015_qwen3-4b_GB10_0.26.0.md` | bench_report | 26101015 | 0 | 7032 | 0.26.0 · easy-vllm:0.26.0-cu132-aarch64-source | — |
| 4 | `docs/plan/plan_26101104_hint발행체제_개편_서브자체발행.md` | plan | 26101104 | 0 | 11774 | — | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

이 계보는 모델을 처음 띄우는 이야기가 아니다. 계보 문서 4건 가운데 이 셀의 서빙·측정 서사를 담은 것은 testlog 와 devlog 이고(plan 은 hint 발행 체제 개편이라 서빙 계획이 아니다), 둘 다 날짜가 2026-10-10 ~ 10-11 이다. **이 계보에는 모델·형상을 처음 정한 plan 이 없다** — 아래 (1)~(5) 는 testlog 와 devlog 에 적힌 것만 옮기며, 없는 것은 없다고 적는다.

(1) 목표. 사실 블록의 셀 정체성이 말하는 형상은 Qwen3-4B(bf16)를 GB10 ×1 · TP=1 에서 KV dtype `turboquant_4bit_nc` · 컨텍스트 32768 로 서빙하고 full 벤치로 재는 것이다. 이번 실행의 직접 목적은 full-bench knob 프리셋 라이브 E2E 의 셀 D 였다(testlog 제목 · 머리 줄). 아래 발췌가 목표 전환을 적는다: 지시의 1순위 config 둘이 이 노드에 없어, 같은 모델의 과거 서빙 성공 config 로 바꿨다.

> [원문] testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §2. 모델/config 선택
> ## 2. 모델/config 선택 (지시 1순위에서 변경)
> - 1순위 config `a-kvauto`/`b-kvfp8` 는 이 노드에 **없음**(`output/single/configs/` 에 부재, 빌드 ✗). 같은 모델(Qwen3-4B)의 과거 서빙 성공 config **`c-tq4nc`**(kv-cache-dtype turboquant_4bit_nc, max-model-len 32768, max-num-seqs 8, kv 11 GiB)로 대체. 이미지 `easy-vllm:0.26.0-cu132-aarch64-source`(로컬 보유, vllm 0.26.1.dev0+g568afb3a1).
> - 과거 GuideLLM 기록(camp-26090814 c-tq4nc 셀): 측정 형상 input_len 8192 / output_len 1024 / n=32 / bench_budget 2048 MiB. 이번은 프리셋 기본(1024/256/16)이라 **형상이 다르다** — 과거 수치와 직접 비교 불가.
> - `--overhead-mib 18432`(10-01 값을 알 수 없어 지시 기본값) · `--ready-max 1200` → **READY 140 s**. floor 87240 / arm 84168 MiB.

(2) HW 출발 조건. GPU 는 NVIDIA GB10 한 장 · 노드 1 개 · TP=1 이고 분산 방식은 없다(사실 블록 0.2). 노드 메모리 총량은 블랙박스 예산 선언 행의 `mem_total_mib=124608`(01 §1.4 이벤트 표 19번 행 — 선언 행의 필드이며 별도 실측 표기는 없다)이고, 엔진 로그는 `Initial free memory 114.16 GiB`(L77)와 `Available RAM: 105.82 GiB`(L61)를 보고했다. 통합메모리 여부는 서빙 yaml 주석(`통합메모리 ≤0.90`)이 전제로만 적고 있다. 인터커넥트는 단일 노드라 해당 없음. 드라이버 580.173.02 는 manifest 선언을 옮긴 값이다(0.4 표).

(3) 모델 구조. 체크포인트는 HF `Qwen/Qwen3-4B` 이고 revision 은 0.2 사실 블록에 있다. 엔진 로그: 체크포인트 크기 7.49 GiB(L61), 적재 후 `Model loading took 7.56 GiB`(L70). **층 구성 · 총/활성 파라미터 · 네이티브 컨텍스트에 관한 기록은 계보 문서에 없다**(체크포인트 config.json 에서 읽은 것은 `torch_dtype` bfloat16 · `quantization_config`/`hf_quant_config.json` 부재 · PLE 키 부재 관측이며 0.2·0.4·0.9 사실 블록에 있다. 그 밖에 루프라인 산출물 `roofline.json` 이 dense(`is_moe` false)와 `active_bytes` 8044936192 (7.492 GiB)를 기록했다. 층 수 · 총 파라미터 수 · 네이티브 컨텍스트는 읽지 않았다). 양자화 층 구성: 가중치는 bf16 비양자화이고(0.4 양자화 표), 양자화는 가중치가 아니라 KV 캐시(`turboquant_4bit_nc`)에만 있다. PLE 등 특수 가중치는 해당 없음(ple_mode none 선언).

(4) stock 상태에서 예상·관측된 장벽. 서빙 자체의 장벽은 관측되지 않았다 — 엔진은 첫 시도에 기동했고(기동 시도 1) 사살·트립이 없다. 관측된 것은 경고 둘(W1 알 수 없는 환경변수, W2 TurboQuant 의 FlashAttention 비호환 자동 하향)과 측정 운영 장벽 둘(W3 backend 값 오기, W4 하네스 시간 제한)이다.

(5) 외부 선례와 사람의 결정. 이번 발행을 위해 외부 지식을 검색하지 않았다. 사람의 결정은 계보에 한 건 있다: 1순위 config 가 없을 때 같은 모델의 과거 성공 config 로 대체해 진행한다는 것(위 발췌). 24GB 카드를 흉내 낸 서빙 설정이라는 출신은 서빙 yaml 머리 주석이 적는다.

> [원문] c-tq4nc.yaml §L1-9
> # Qwen3/Qwen3-4B 서빙 설정 (vllm-recipe-explorer 생성)
> # 레시피: quant=None max_model_len=32768 gpu_mem_util=0.9
> # ── VRAM 분해 (Phase 2 절대 KV 클램프 기준) ──
> #   weights: 7.56 GiB
> #   kv_cache: 11.0 GiB
> #   budget: 24.0 GiB
> # ── 타겟-GPU 이식 클램프 (host≠target, gpu_model=rtx4090-24gb-sim) ──
> # host-측정 weights/overhead 이전값. target≠host arch 면 target-margin 이 쿠션(정량보증 아님).
> # 가능하면 타겟에서 재측정(Phase-2)을 권장. per-token-KV·weights 는 GPU-불변, overhead 는 런타임 성질 일부 포함.

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-10T05:33:14Z(`camp-26101104-hint-cd` · evidence.event_campaign_boundary): campaigns/camp-26101104-hint-cd/campaign.yaml declared_utc 2026-10-10T19:53:02Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-10T05:33:14Z → 경계 = 앞선 값 2026-10-10T05:33:14Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): sub = 2026-06-13T22:25:01Z → 2026-10-10T15:17:46Z(`docs/logs/sub/events/2026-06.jsonl` · `docs/logs/sub/events/2026-07.jsonl` · `docs/logs/sub/events/2026-09.jsonl` · `docs/logs/sub/events/2026-10.jsonl` · `docs/logs/sub/events/thermal.jsonl` · `docs/logs/sub/events/watchdog.jsonl`)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | sub | `serve-c-tq4nc` | 2026-10-10T05:33:14Z | 2회 · 첫 2026-10-10T05:33:15Z | 없음 | `budget_clear` 2026-10-10T06:26:39Z | 10 |

**캠페인 경계 이전 기동 2회**(캠페인 경계 2026-10-10T05:33:14Z 이전 — 이 셀 라벨로 묶인 1회 · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 1회 · 어느 쪽도 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | sub | `serve-c-tq4nc` | 2026-09-07T20:06:36Z | 3회 · 첫 2026-09-07T20:06:37Z | 없음 | `budget_clear` 2026-09-07T21:12:30Z | 4 |
| 앞선-2 | sub | `camp-26090814.cell-c-tq4nc.rerun`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-09-08T06:39:09Z | 3회 · 첫 2026-09-08T06:39:10Z | 없음 | `budget_clear` 2026-09-08T07:41:39Z | 4 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

> 아래 네 항목 중 **서빙을 막은 것은 없다.** W1 · W2 는 기동 로그의 경고이고, W3 · W4 는 측정을 돌리던 운영 쪽 장벽이다. 이 셀의 기동 시도는 1회이며 사살·트립·거부가 없다(위 표).

### W1 쉘의 `VLLM_ATTENTION_BACKEND` 핀을 엔진이 모르는 변수라 보고했다
서빙 러너 `c-tq4nc.sh` 는 `export VLLM_ATTENTION_BACKEND=FLASH_ATTN` 으로 attention backend 를 고정하려 한다(L15). 엔진은 같은 기동에서 이 변수를 알 수 없는 vLLM 환경변수로 보고했고, 그와 별개로 같은 기동에서 백엔드 선택 줄을 **둘** 남겼다 — L58 은 후보 4종 중 FLASH_ATTN, L60 은 후보가 TURBOQUANT 하나뿐인 목록에서 TURBOQUANT 다. **이 핀이 선택을 만들었는지 엔진의 자체 선택인지, 두 줄 중 어느 쪽이 KV 경로의 실제 백엔드인지는 이 로그로 가르지 못한다.** 이 셀은 핀을 고치지 않고 그대로 두었다 — 해소가 아니라 관찰이다. 변수가 왜 '알 수 없는' 것이 되었는지(이름 변경 여부)는 조사하지 않았다.

> [원문] lite_engine_c-tq4nc.log §L43-43
> (APIServer pid=147) WARNING 10-10 05:33:29 [envs.py:2096] Unknown vLLM environment variable detected: VLLM_ATTENTION_BACKEND
> [원문] lite_engine_c-tq4nc.log §L58-58
> (EngineCore pid=240) INFO 10-10 05:33:39 [cuda.py:482] Using FLASH_ATTN attention backend out of potential backends: ['FLASH_ATTN', 'FLASHINFER', 'TRITON_ATTN', 'FLEX_ATTENTION'].

```hint-event
id: W1
kind: wall
증상: 러너가 고정한 attention backend 환경변수를 엔진이 알 수 없는 변수라고 경고(서빙은 정상 기동)
서명: "Unknown vLLM environment variable detected: VLLM_ATTENTION_BACKEND"
원인: 엔진이 이 변수명을 인식하지 않는다고 보고했다(경고 원문까지만 확인 — 변수가 제거·개명되었는지는 조사하지 않음)
해소: 해소하지 않음 — 러너의 핀을 그대로 두었고, 엔진 로그는 백엔드 선택 줄을 둘 남겼다(L58 FLASH_ATTN · L60 TURBOQUANT)
검증: lite_engine_c-tq4nc.log §L58-60
전이등급: judgment
출처: [lite_engine_c-tq4nc.log §L43-43, c-tq4nc.sh]
```

### W2 TurboQuant KV 는 FlashAttention 3 이상과 호환되지 않아 엔진이 FA2 로 내렸다
KV dtype 을 `turboquant_4bit_nc` 로 두면 엔진이 flash_attn 버전을 2 로 덮어쓴다는 경고를 낸다. 서빙은 막히지 않았고 FlashAttention version 2 와 TURBOQUANT attention backend 가 함께 선택되었다는 로그가 이어진다(L59 · L60). 경고 문구가 침묵시키는 방법(`--attention-config.flash_attn_version=2`)을 직접 적고 있으나 이 셀에서는 적용하지 않았다(yaml · 러너에 없음).

> [원문] lite_engine_c-tq4nc.log §L46-46
> (APIServer pid=147) WARNING 10-10 05:33:30 [arg_utils.py:2267] TurboQuant is not yet compatible with FlashAttention >= 3. Overriding flash_attn_version to 2. To silence this warning, pass --attention-config.flash_attn_version=2
> [원문] lite_engine_c-tq4nc.log §L59-60
> (EngineCore pid=240) INFO 10-10 05:33:39 [flash_attn.py:776] Using FlashAttention version 2
> (EngineCore pid=240) INFO 10-10 05:33:39 [cuda.py:482] Using TURBOQUANT attention backend out of potential backends: ['TURBOQUANT'].

```hint-event
id: W2
kind: wall
증상: TurboQuant KV 사용 시 엔진이 flash_attn 버전을 2 로 덮어쓰는 경고를 낸다(기동은 성공)
서명: "TurboQuant is not yet compatible with FlashAttention >= 3. Overriding flash_attn_version to 2."
원인: 엔진 경고 원문이 말하는 비호환(TurboQuant 대 FlashAttention 3 이상) — 비호환의 내부 기전은 확인하지 않음
해소: 엔진이 자동으로 FA2 로 덮어씀 — 이 셀은 명시 플래그를 주지 않았다
검증: lite_engine_c-tq4nc.log §L59-60
전이등급: arch-invariant
출처: [lite_engine_c-tq4nc.log §L46-46, c-tq4nc.yaml]
```

### W3 측정 호출의 backend 값에 reason 코드가 붙어 거부되었다
첫 측정 호출은 `--backend openai-chat+chat_template` 이었고 도구가 거부했다. 지시문의 `+chat_template` 은 backend 값이 아니라 **reason 코드**였다. `--backend openai-chat` 과 `--knob-reason backend=chat_template` 로 나눠 다시 호출해 통과했다. 지시문 쪽 결함이므로 모델·서빙과 무관하다.

> [원문] testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §4. 운영 사건
> ## 4. 운영 사건 (정직 기록)
> - 첫 호출 `--backend openai-chat+chat_template` 은 `알 수 없는 --backend (openai|openai-chat)` 로 거부됨. 지시문의 `+chat_template` 은 backend 값이 아니라 reason 코드 → `--backend openai-chat --knob-reason backend=chat_template` 로 수정.

```hint-event
id: W3
kind: wall
증상: 풀 벤치 첫 호출이 backend 인자 검증에서 거부됨(측정 전)
서명: "알 수 없는 --backend (openai|openai-chat)"
원인: 지시문이 backend 값과 reason 코드를 `+` 로 합쳐 적었다 — `+chat_template` 은 backend 값이 아니라 reason 코드
해소: `--backend openai-chat` 과 `--knob-reason backend=chat_template` 로 분리해 재호출
검증: testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §5.
전이등급: arch-invariant
출처: [testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §4., devlog_26101108_c-tq4nc_셀D_서빙_측정_knob해소 §지시문 결함]
```

### W4 첫 풀 스윕이 하네스의 백그라운드 10분 제한으로 레벨 8 도중 끊겼다
첫 풀 스윕은 레벨 8 을 재던 중 하네스의 백그라운드 실행 제한으로 종료되었다. 같은 설정을 `setsid nohup` 으로 다시 띄워 완주했고, 서빙은 유지한 채 재측정했다. **이 셀의 수치는 전부 재실행분이다**(testlog). 끊긴 첫 스윕의 수치는 쓰지 않았다(R2).

> [원문] testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §4. 운영 사건
> - 첫 풀 스윕이 레벨 8 도중 하네스 백그라운드 10분 제한으로 종료됨 → 같은 설정으로 `setsid nohup` 재실행하여 완주(최종 수치는 재실행분). 서빙은 유지한 채 재측정.

```hint-event
id: W4
kind: wall
증상: 풀 스윕이 레벨 8 도중 종료됨(측정 미완주)
서명: "첫 풀 스윕이 레벨 8 도중 하네스 백그라운드 10분 제한으로 종료됨"
원인: 하네스의 백그라운드 실행 10분 제한 — 서빙 · 모델 쪽 원인이 아님
해소: 같은 설정을 `setsid nohup` 으로 재실행해 완주(서빙 유지 · 최종 수치는 재실행분)
검증: testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §3.
전이등급: arch-invariant
출처: [testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §4.]
```

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: 지시의 1순위 config `a-kvauto` / `b-kvfp8` 로 이 셀을 서빙·측정하기
기각사유: 이 노드 `output/single/configs/` 에 두 config 가 없어 착수하지 못했다(빌드 ✗) — 서빙을 시도해 실패한 것이 아니라 **시도 0** 이다. 같은 모델의 과거 서빙 성공 config c-tq4nc 로 대체
재개조건: 두 config 가 이 노드에 만들어지면 KV dtype 대조(auto · fp8 대 turboquant_4bit_nc)를 같은 프리셋으로 잴 수 있다
출처: [testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §2.]
```

```hint-event
id: R2
kind: rejected
시도: 하네스 10분 제한으로 레벨 8 도중 끊긴 첫 풀 스윕의 수치를 이 셀의 결과로 쓰기
기각사유: 스윕이 완주하지 못했다 — 최종 수치는 같은 설정의 재실행분이다. 끊긴 쪽의 클라이언트 수치는 계보 문서에 적혀 있지 않다(서버 쪽 로그 줄은 엔진 로그에 남아 있다 — M1)
출처: [testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §4.]
```

기각된 서빙 형상은 이 계보에 없다. 이 셀의 형상(KV `turboquant_4bit_nc` · 32768 · 시퀀스 8 · 절대 KV 클램프)을 다른 형상과 비교한 기록이 계보 문서에 없으므로, 무엇이 형상을 좁혔는가에 대한 답은 "형상은 승계되었고 좁혀진 적이 없다" 이다(V1~V5).

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

이 계보에서 **서빙 벽을 잘못 진단한 기록은 없다.** 하나만 적는다 — testlog 는 엔진 로그의 gen max 와 클라이언트 decode 의 비율 7~8 에 "합산/순간 피크 지표" 라는 설명을 붙이고 "원인 미조사" 라고도 적었다. 그러나 **대조 근거가 원장에 있다**: 레벨 1·2·4·8 의 `measured.json` 은 모두 같은 `engine_gen_throughput_max` 184.8 을 담고, `client_engine_agreement.ratio` 는 8.24 / 7.04 / 7.39 / 8.12 다 — 분자는 레벨별 엔진 값이 아니라 **로그 전체의 최댓값 하나**이고 분모만 레벨별 클라이언트 decode 다. 그 184.8 줄은 엔진 로그 L498(05:55:13, `Running: 8 reqs`)이며, 최종 재실행의 레벨 1 시작(06:05:54Z)보다 앞서 하네스 제한으로 끊긴 첫 스윕(W4)의 구간이다. 따라서 "엔진 쪽 합산 최댓값이라 클라이언트 decode 와 비교 대상이 아니다" 라는 설명은 원장으로 뒷받침되고(지위 확증), "원인 미조사/대조 없음" 이 틀렸다. 레벨 1 의 decode 를 동시성 8 구간의 값으로 나눈 비율이 7~8(실제 범위 7.04~8.24)로 보인 것이다(Q2).

> [원문] testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §3. 측정
> - 주의: 엔진 로그 gen max 184.8 t/s 와 클라이언트 decode 의 ratio 7~8 은 엔진 합산/순간 피크 지표라 비교 대상이 아니다(셀 C 의 ratio 1.01 과 의미가 다름 — 원인 미조사).

```hint-event
id: M1
kind: misdiagnosis
증상: 엔진 로그 gen max 와 클라이언트 decode 의 비율이 7~8 로 보였다(셀 C 의 비율과 의미가 다르다고 적힘)
서명: "엔진 로그 gen max 184.8 t/s 와 클라이언트 decode 의 ratio 7~8 은 엔진 합산/순간 피크 지표라 비교 대상이 아니다"
원인: 당시 설명(엔진 합산/순간 피크 지표)은 맞았고 "원인 미조사" 가 틀렸다 — 분자 184.8 은 레벨 공통의 로그 전체 최댓값(L498 · 동시 8요청 구간 · 최종 재실행 이전의 끊긴 첫 스윕)이고 분모는 레벨별 클라이언트 decode 라 비율 7.04~8.24 는 서로 다른 실행·동시성의 수를 나눈 값
해소: 이 비율은 서빙 이상의 신호가 아니다 — 같은 동시성·같은 실행의 엔진 값과 비교하지 않는 한 쓰지 않는다. 셀 C 의 1.01 과의 차이 원인은 확인하지 않았다(Q2)
검증: testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §3. · devlog_26101108_c-tq4nc_셀D_서빙_측정_knob해소 §불확실
전이등급: judgment
현재지위: 확증
출처: [testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §3., devlog_26101108_c-tq4nc_셀D_서빙_측정_knob해소 §불확실, lite_engine_c-tq4nc.log §L498-498]
```

교정 묶음: 이 계보에 항목 id 가 붙은 교정 묶음 문서는 없다(검토한 문서 4건 가운데 해당 0건).

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

이 셀의 다섯 노브는 전부 **승계값**이다 — 이 계보에서 값을 바꿔 가며 잰 기록이 없다(V1~V5). 값이 정해진 경위는 서빙 yaml 머리 주석이 전부다: 24GB 카드(`rtx4090-24gb-sim`)를 흉내 내는 이식 클램프로, 가중치 7.56 GiB · KV 11.0 GiB · 예산 24.0 GiB 가 적혀 있다(발췌). 설정 파일 안의 이 분해는 **선언**이고, 이번 기동의 엔진 로그가 보고한 쪽은 `Model loading took 7.56 GiB`(L70) · 예약된 KV 11.0 GiB(L77)다. 두 쪽의 숫자가 일치하지만 그 이유를 설명하는 계보 문서는 없다.

> [원문] c-tq4nc.yaml §L1-9
> # Qwen3/Qwen3-4B 서빙 설정 (vllm-recipe-explorer 생성)
> # 레시피: quant=None max_model_len=32768 gpu_mem_util=0.9
> # ── VRAM 분해 (Phase 2 절대 KV 클램프 기준) ──
> #   weights: 7.56 GiB
> #   kv_cache: 11.0 GiB
> #   budget: 24.0 GiB
> # ── 타겟-GPU 이식 클램프 (host≠target, gpu_model=rtx4090-24gb-sim) ──
> # host-측정 weights/overhead 이전값. target≠host arch 면 target-margin 이 쿠션(정량보증 아님).
> # 가능하면 타겟에서 재측정(Phase-2)을 권장. per-token-KV·weights 는 GPU-불변, overhead 는 런타임 성질 일부 포함.

- 처음 값 → 지금 값. **max-model-len 32768 · max-num-seqs 8 · gmu 0.9 · kv-cache-memory-bytes 11811160064 · kv-cache-dtype `turboquant_4bit_nc`** 모두 바뀐 적이 없다. 스윕을 계획했다는 기록도 없다(sweep map 이 이 계보에 없다 — 계획 N · 시도 0 의 구분도 불가).
- KV 필요량 대비 배수. 이 발행은 필요량 산식을 새로 계산하지 않는다. 엔진 로그가 같은 기동에서 보고한 두 줄만 옮긴다: `GPU KV cache size: 232,992 tokens`(L78), `Maximum concurrency for 32,768 tokens per request: 7.11x`(L79). **max-num-seqs 8 은 32768 토큰 요청 기준 동시성 7.11x 보다 크다** — 모든 시퀀스가 최대 길이이면 KV 가 모자랄 수 있다는 뜻이지만, 이 계보의 부하는 입력 1024 · 출력 256 (testlog §5)의 짧은 요청이라 그 조건은 잰 적이 없다.
- 선택 이유. 시퀀스 8 · 클램프 11 GiB 가 왜 이 값인지의 설명은 계보 문서에 없고 yaml 주석의 "Phase 2 절대 KV 클램프 기준"뿐이다. 최적이라는 주장은 없다.
- 판정 산정에 손으로 승계된 입력: 없음. spec 이 꺼져 accept_len 은 선언 부재로 1.0(03 사실 블록 `accept_len` 행 — 선언값이지 측정이 아니다).

> [원문] lite_engine_c-tq4nc.log §L77-79
> (EngineCore pid=240) INFO 10-10 05:34:29 [gpu_worker.py:479] Initial free memory 114.16 GiB, reserved 11.0 GiB memory for KV Cache as specified by kv_cache_memory_bytes config and skipped memory profiling. This does not respect the gpu_memory_utilization config. Only use kv_cache_memory_bytes config when you want manual control of KV cache memory size. If OOM'ed, check the difference of initial free memory between the current run and the previous run where kv_cache_memory_bytes is suggested and update it correspondingly.
> (EngineCore pid=240) INFO 10-10 05:34:29 [kv_cache_utils.py:2177] GPU KV cache size: 232,992 tokens
> (EngineCore pid=240) INFO 10-10 05:34:29 [kv_cache_utils.py:2178] Maximum concurrency for 32,768 tokens per request: 7.11x

**버티는 기전 대 실패한 기전.** 이 계보는 같은 셀의 다른 값·형상을 시도한 적이 없다. 따라서 "이 형상이 버틴 기전"은 설명되지 않았고 — 버텼다는 사실만 있다(기동 1회에 READY, 사살 없음). KV 를 절대 바이트로 고정하면 엔진이 메모리 프로파일링을 건너뛰고 그 바이트를 예약한다는 것만 L77 이 직접 말한다(지위: 확정 — 엔진 로그 원문). gmu 가 그 위에서 무엇을 하는지는 yaml 주석의 선언이며 시험되지 않았다(지위: 미결).

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- 판정점 runs 3회 동일(22.44 ×3 · 밴드 0.0 %)을 **재현성 증거로 쓰지 말 것** — 원인 미확정이고 같은 실행들의 다른 지표(output_throughput · tpot)는 서로 다르다 — Q1 · bench_report_26101015_qwen3-4b_GB10_0.26.0 관측 주의 — 재현성은 다른 레벨의 밴드(0.04~0.12 %)와 별도 반복으로 본다.
- 엔진 gen max 와 클라이언트 decode 의 비율 7~8 로 서빙 이상을 판정하지 말 것 — M1 · Q2 — 분자는 로그 전체의 최댓값 하나(L498)라 레벨·실행이 다르다. 비교는 같은 실행·같은 동시성에서만 한다.
- `VLLM_ATTENTION_BACKEND` 핀이 backend 를 고정했다고 믿지 말 것 — W1 — 엔진 로그에서 `Using ... attention backend` 줄은 둘(L58 FLASH_ATTN · L60 TURBOQUANT)이니 한 줄만 보고 판단하지 말고 두 줄을 함께 본다(어느 쪽이 KV 경로의 실제 백엔드인지는 이 계보가 가르지 않았다).
- TurboQuant KV 와 FlashAttention 3 이상을 같이 요구하지 말 것 — W2 — 엔진의 FA2 하향을 받아들이거나 `--attention-config.flash_attn_version=2` 를 명시한다(후자는 이 셀에서 시험하지 않았다).
- `--backend` 에 reason 코드를 붙이지 말 것 — W3 — backend 값과 `--knob-reason` 을 분리한다.
- 풀 스윕을 하네스 백그라운드 10분 제한 아래에서 돌리지 말 것 — W4 — `setsid nohup` 으로 분리 실행하고 상태를 직접 폴링한다.
- 24GB 시뮬 설정의 KV 클램프 · gmu 를 다른 GPU 에 그대로 복사하지 말 것 — V2 · V4 · devlog_26101108_c-tq4nc_셀D_서빙_측정_knob해소 §불확실 — 엔진 로그의 토큰 수와 free memory 로 다시 도출한다.
- 이 셀의 수치를 입력 8192 · 출력 1024 · n=32 형상의 과거 측정과 직접 비교하지 말 것 — testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §2 — 같은 형상으로 다시 재서 비교한다.
- 기동 로그의 `NVIDIA Driver was not detected` 배너(L30)를 GPU 부재로 읽지 말 것 — 같은 로그가 이어서 모델 적재와 KV 예약에 성공했다 — 이 배너의 원인은 조사하지 않았다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: 판정점(레벨 1) runs 3회가 22.44 / 22.44 / 22.44 로 소수 2자리까지 완전히 같다(밴드 0.0 %). 왜 같은가?
현재상태: 세 실행의 원시 파일은 서로 다른 파일이다. 같은 세 실행의 output_throughput 은 22.4753 / 22.4473 / 22.4434, tpot_ms_median 은 44.558 / 44.573 / 44.567 로 다르다. decode_tps 칸의 해상도(소수 2자리) 효과일 가능성이 있으나 확인하지 않은 가설이다. 다른 레벨의 밴드는 0.0381 % · 0.1201 % · 0.0439 % 로 0 이 아니다
다음관측: decode_tps 가 어느 원시 필드에서 어떻게 산출되는지 도구 소스를 대조하고, 같은 레벨을 소수점 4자리 이상으로 다시 읽는다
출처: [bench_report_26101015_qwen3-4b_GB10_0.26.0 §관측 주의, testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §3.]
```

```hint-event
id: Q2
kind: open-question
물음: 같은 실행·같은 동시성에서 엔진 쪽 생성 처리량과 클라이언트 decode 는 어떤 비율인가? 셀 C 의 1.01 과 왜 다른가?
현재상태: 이 셀의 비율 7.04~8.24 는 레벨 공통의 로그 최댓값 184.8(L498 · `Running: 8 reqs` · 끊긴 첫 스윕 구간)을 레벨별 decode 로 나눈 값이라 같은 실행의 비교가 아니다(M1). 셀 C 의 1.01 이 어떤 분자로 계산됐는지는 이 계보에 없다
다음관측: 레벨별 엔진 로그(level_*/engine_*.log)에서 그 레벨의 구간 최댓값을 뽑아 같은 레벨 decode 와 나눈다
출처: [testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §3.]
```

```hint-event
id: Q3
kind: open-question
물음: 레벨 4 · 8 의 completed 가 18 로 num_prompts 16 보다 큰 이유는 무엇인가?
현재상태: 3.2 부하 곡선 표에 18/0 으로 기록되어 있다. testlog 는 셀 C 와 같은 현상이라 적고 원인 미조사라 했다. 레벨 1 · 2 는 16/0 이다
다음관측: 도구 산출물 JSON 의 요청 단위 기록에서 warmup 요청이 집계에 포함되는지 확인한다
출처: [testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §3., bench_report_26101015_qwen3-4b_GB10_0.26.0 §부하 스윕 곡선]
```

```hint-event
id: Q4
kind: open-question
물음: 엔진 자기보고 버전 `0.26.1.dev0+g568afb3a1.d20260907` 은 빌드 입력 `v0.26.0` 과 어떤 관계인가?
현재상태: 빌드 입력은 docker history build-arg 로 관측된 `VLLM_REF=v0.26.0` 이고 VLLM_PRETEND_VERSION 은 빈 값이다. 엔진 문자열은 dev 접미와 커밋 단축 해시를 달고 있다. 소스 40자 SHA 는 관측되지 않았다(0.4 사실 블록) — 따라서 두 값이 같은 소스를 가리키는지 확인되지 않았다
다음관측: 이미지 안 vLLM 소스의 git HEAD 40자 SHA 를 읽는다
출처: [devlog_26101108_c-tq4nc_셀D_서빙_측정_knob해소 §서빙, testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §2.]
```

```hint-event
id: Q5
kind: open-question
물음: 24GB 카드 위에서 같은 설정은 어떤 성능을 내는가?
현재상태: 서빙 yaml 은 24GB 시뮬 예산으로 사이징되었고 측정은 GB10 한 장에서 이루어졌다. yaml 주석은 per-token-KV · weights 가 GPU 불변이라 적지만 이 계보에서 24GB 카드로 잰 적이 없다
다음관측: 실제 24GB 카드에서 같은 yaml 로 기동해 KV 토큰 수(엔진 로그)와 decode 를 잰다
출처: [devlog_26101108_c-tq4nc_셀D_서빙_측정_knob해소 §불확실, c-tq4nc.yaml §L1-9]
```

```hint-event
id: Q6
kind: open-question
물음: 새로 띄운 서버의 첫 요청에서 Triton 커널 JIT 컴파일(`_tq_decode_stage1` 등)이 만드는 지연은 얼마인가?
현재상태: 엔진 로그는 JIT 컴파일이 05:36:01 의 첫 요청 구간에 일어났고 지연 급등을 만들 수 있다고 경고했다(L144 · L145). lite cold 측정은 06:05:11Z 에 시작해 그보다 약 29분 뒤이므로 기록된 cold TTFT 55.1 ms 는 이 JIT 이후의 값이다 — 즉 JIT 가 55.1 ms 를 부풀렸다는 가설은 시간순으로 성립하지 않는다. JIT 구간(05:35:53~05:36:01)의 첫 요청 지연 자체는 이 계보가 재지 않았다
다음관측: 새 서버의 첫 요청 TTFT 를 직접 재서 JIT 지연을 분리한다
출처: [lite_engine_c-tq4nc.log §L144-145]
```

> [원문] lite_engine_c-tq4nc.log §L144-145
> (EngineCore pid=240) WARNING 10-10 05:36:01 [jit_monitor.py:135] Triton kernel JIT compilation during inference: _tq_decode_stage1. This causes a latency spike; consider extending warmup to cover this shape/config.
> (EngineCore pid=240) WARNING 10-10 05:36:01 [jit_monitor.py:135] Triton kernel JIT compilation during inference: _fwd_kernel_stage2. This causes a latency spike; consider extending warmup to cover this shape/config.

열린 물음 외에, 이 계보는 상위 vLLM 버전에서 이 구성이 달라지는지 관측하지 않았다(자체 패치 슬롯이 비어 있어 폐기 판정 대상도 없다).
