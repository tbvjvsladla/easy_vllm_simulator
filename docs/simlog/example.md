# simlog/ — serve/시뮬레이션 원시증거 vault (모든 trial-loop run 산출)

> 이 `example.md`는 **추적되는 스켈레톤**이다(폴더 구조 + 작성 규칙 배포용). 실제 run은
> 이 폴더에 **run 디렉토리**로 추가하되 **git 추적 대상이 아니다**(gitignore — 브랜치 간 persist·통합). 작성 규칙 정본: `.claude/rules/docs.md`.

## 역할
- trial-loop **한 run의 원시 증거 적재함**("실측이 정확히 무엇이었나").
- 적용 범위 = `recipe.py simulate` 산출 **+ 수동 serve 스윕/bump 난항의 trial 반복**(반복 serve 실험이면
  어느 평면이든 run 디렉토리로 적재 — **per-trial config 사본 필수**: 과거 trial 설정의 in-place 변이 유실 방지).
- testlog가 사람용 종합 보고서라면, simlog는 그 보고서가 인용하는 **기계 생성 raw 증거**다.
- 참조 체인: simlog(원시) → testlog(인용·판정) → devlog(서사).

## 명명 규칙 (simlog만 예외 — 파일이 아니라 run 디렉토리)
```
docs/simlog/<YYYYMMDDHH>_<seq>_<주제>/
```
- `<type>_` 파일 접두사 없음. **run 1개 = 디렉토리 1개**.
- `YYYYMMDDHH`: 작성 일시 절대표기 (예 `2026062121` = 2026-06-21 21시). 상대날짜 금지.
- `seq`: 같은 일시 내 일련번호(1부터).
- `주제`: 한국어, 밑줄(`_`) 구분, 버전·대상 포함 권장.
- 예: `docs/simlog/2026062121_1_vLLM0.22.1_KV클램프_시뮬/`

## 수동 serve 스윕 run 내용 (에이전트가 동형 구조로 적재)
- `serve_logs/` — trial 별 docker logs 원문(`trialNN_<svc>.log`).
- `key_files_snapshot/` — 그 시점 config yaml/플래그·build_patches 등 판정에 걸린 파일 사본.
- `run_summary.md`(또는 .json) — trial 표(설정→결과)·판정 요약(상세 판정은 testlog 가 정본).

## simulate run 디렉토리 내용 (`simlog_writer.py`가 기록, trial NN은 `01`부터)
- `trialNN_vllm.log` — 컨테이너 docker logs 원문(`VLLM_LOGGING_CONFIG_PATH`로 `/app/simlog` 캡처).
- `trialNN_profile.json` — `parse_vllm_log` 실측(weights/kv/overhead GiB, 백엔드 등).
- `trialNN_candidate.yaml` — 그 trial의 lock-set + 설정한 `kv_cache_memory_bytes`.
- `trialNN_smoke.json` — `functional_smoke` 결과(completion/tool_call/reasoning).
- `correction_history.jsonl` — trial 간 결정론 조정·soft 변수 폴백 1줄/건.
- `run_summary.json` — 수렴 여부·trial_count·최종 채택 candidate·실측 분해.
