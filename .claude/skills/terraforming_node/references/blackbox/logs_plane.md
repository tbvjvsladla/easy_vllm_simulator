# 블랙박스 — `docs/logs/` 기계판독 데이터 평면

> terraforming_node 스킬 reference — docs.md §기계판독 데이터 평면의 경로·포맷·수명 상세. 데이터 평면 규칙의 정본.
> 이관 전 원문: `git show dcb713a:.claude/rules/docs.md` (plan_26093022 Step 4 — 본문 바이트는 그대로 옮겼다). 불변식은 docs.md 에 남는다.

## 기계판독 데이터 평면 — `docs/logs/` (8번째, 유일한 비-문서)

> 근거 `plan_26073109`(노드블랙박스 승격) · 소유 `terraforming_node/scripts/node_blackbox/`.
> **사람 가독성을 고려하지 않는다**(사용자 결정) — 열람이 필요하면 그때 비패턴 업무로 md/html 변환한다.
> 산문 7종과 성격이 다르므로 §명명 SSOT·§공통 발행 계약·evidence chain 규약을 적용하지 않는다.

| 경로 | 내용 | 포맷 근거 | 수명 |
|---|---|---|---|
| `docs/logs/<node_id>/samples/<YYYY-MM-DD>.csv` | 1초 원시 시계열 | 키 반복이 없어 JSONL 대비 약 1/3 용량 | 7일 → 압축 30일 → 삭제 |
| `docs/logs/<node_id>/events/<YYYY-MM>.jsonl` | 희소·이질 이벤트(트립·킬·부정클린부팅) | 자기서술 필요, 양이 적음 | **영구** |
| `docs/logs/<node_id>/rollup/<YYYY-MM-DD>.json` | 일별 포락선 통계 | 학습의 실제 입력 | **영구** |
| `docs/logs/<node_id>/envelope.json` | 현재 포락선 + ETA 상수 | **에이전트가 폴링마다 읽는 유일한 파일**(수백 토큰) | 갱신 |
| `docs/logs/<node_id>/campaign_brief.json` | 캠페인 진행 요약(셀·phase·여정·`last_utc`) | **메인이 서브 진행을 읽는 유일한 자리**(2026-09-08 · `plan_26090813` §4.2). 이 자리가 없어서 메인이 서브를 ssh 로 32회 직접 관측했다 — 채널이 없으면 사람은 우회를 만든다 | 갱신(phase 전이·publish 마다) |
| `docs/logs/<node_id>/capture_verified.json` | proof-of-capture 판정 | 상태 권위(`installed` 아님) | 갱신 |
| `docs/logs/<node_id>/seed/` | 레거시 저널 수확분 | 15초 해상도 재구성(canonical 아님) | 보존 |

- **수명 집행 순서가 곧 안전장치다**: rollup(통계 확정) → 압축 → 나이삭제 → 용량삭제. 원시를 버려도
  학습 입력은 남는다. 노드당 총량 상한(기본 512 MiB) 초과 시 **오래된 samples 부터** 삭제한다.
- **침묵 삭제 금지**: 모든 삭제·압축은 `events` 에 `log_evicted`/`log_compressed` 로 남긴다 —
  조용한 삭제는 "기록이 원래 없었던 것"과 구분되지 않는다.
- **부재와 결측의 구분**: GB10 통합메모리는 GPU 메모리 지표가 존재하지 않으므로(`nvidia-smi
  memory.used` = `[N/A]`) `gpu_mem` 열은 상시 빈 칸이며 이는 정상이다.
- 시각은 `--now` 주입만 사용한다(벽시계 금지 — `staleness_gate.py --max-age-days` 선례 정합).
