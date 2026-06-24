# simlog 예시 — VRAM 시뮬레이터 trial-loop 증거 vault (`recipe.py simulate` 산출)

> 이 파일은 **추적되는 스켈레톤**(docs.md §simlog 규약의 예시). 실제 run 은 아래 구조의
> **디렉토리 1개 = 1 run** 으로 떨어진다(`simlog_writer.py` 가 기록). simlog 만 예외 —
> 파일이 아니라 **run 디렉토리**(`<type>_` 접두사 없음). seq·주제 명명 규칙은 동일.

## 명명 (docs.md §1)
```
경로:  docs/simlog/<YYYYMMDDHH>_<seq>_<주제>/   ← run 디렉토리 (파일 아님)
```
예: `docs/simlog/2026062121_1_vLLM0.22.1_KV클램프_시뮬/`

## run 디렉토리 내용 (trialNN 은 `01` 부터)
- `trialNN_vllm.log`        — 컨테이너 docker logs 원문(`VLLM_LOGGING_CONFIG_PATH` 로 `/app/simlog` 캡처).
- `trialNN_profile.json`    — `parse_vllm_log` 실측(weights/kv/overhead GiB, 백엔드 등).
- `trialNN_candidate.yaml`  — 그 trial 의 lock-set + 설정한 `kv_cache_memory_bytes`.
- `trialNN_smoke.json`      — `functional_smoke` 결과(completion/tool_call/reasoning).
- `correction_history.jsonl`— trial 간 결정론 조정·soft 변수 폴백 1줄/건.
- `run_summary.json`        — 수렴 여부·trial_count·최종 채택 candidate·실측 분해.

## 참조 체인
`simlog`(원시 증거) → `testlog`(인용·종합 보고서·판정) → `devlog`(서사).
simulate run 이면 testlog 본문에 이 simlog run 경로를 명기한다(사람은 testlog 를 읽고, 필요 시 이 vault 를 인용 확인).
