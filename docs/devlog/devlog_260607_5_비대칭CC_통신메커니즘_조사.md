# devlog 260607-5 — 멀티노드 비대칭-CC 아키텍처 도출 + sync_to_sub.sh

## 작업 요약
멀티노드 확장 설계를 진전. 멀티노드 미결 4건 확정 → 전달 스크립트(`sync_to_sub.sh`) 구현·검증 →
사용자 아키텍처 전환(비대칭 자율 서브-CC) → CC 공식문서 조사로 통신 메커니즘 확정.
설계 상세: `docs/plan/plan_260607_5`. 근거: Seed `seed_8f97998bdec0`, 메모리 `multi-node-cc-architecture`/`multi-node-environment`.

## 1. 멀티노드 미결 4건 확정 (사용자)
- 병렬화 = **최소**(2노드 동시기동[Ray 흡수] + 서브 원격작업만 비동기). 과병렬 회피.
- 전달 캡슐화 = **스크립트** `sync_to_sub.sh`.
- 배치 = **upstream-version-watch 통합**(별도 스킬 아님).
- 도출 아키텍처 이견 없음.

## 2. sync_to_sub.sh 구현·검증
- 검증된 rsync 채널: `rsync -az --delete -e "ssh -o BatchMode=yes" --exclude .git/.claude/seed/docs/__pycache__`.
- HITL 내장: 기본 **DRY-RUN**, 실제 전송은 `--apply`. SSH pre-flight + 전송 후 체크섬 검증(Dockerfile·compose·requirements).
- DRY-RUN 검증: SSH 통과 ✅, 제외 경로 보호 ✅(.git/.claude/seed/docs/__pycache__ 미포함), itemize 정상.
- ⚠️ 안전: 현재 메인 워킹트리가 `main`(단일노드)이라 `--apply` 시 서브 multi-node 덮어쓰기 위험 → 실 sync는 `multi-node` 브랜치 상태에서. 스크립트는 정상, 호출 브랜치가 전제.

## 3. 아키텍처 전환 — 비대칭 자율 서브-CC (사용자 통찰)
- 기존 가정("메인이 서브를 SSH로 일일이 조종 + 출력 파싱")을 폐기.
- 신규: 서브노드에 **성격이 다른 독립 CC**(빌드워커)를 두고, 메인은 코드 전달 + 트리거만, 서브가 자기 일을 책임지고 **깔끔한 리포트**만 반환.
- 코드 범위 확정: 메인은 컨테이너 구현 코드만 전달, **모델별 envs/configs는 서브 CC 자작**, 단 **아카이브는 메인**(중앙 정본).

## 4. CC 공식문서 조사 (claude-code-guide 2회)
- **결론: SSH + `claude -p --output-format json`** 이 머신 2대 협업의 정석. Agent SDK Multiagent는 같은 샌드박스/클라우드용 → 부적합(공식 명시).
- 무프롬프트 자동화 필수: `--bare` + `--permission-mode dontAsk` + 명시적 `--allowedTools` (없으면 SSH 권한 프롬프트 hang).
- 서브 페르소나 = 서브노드 자체 `CLAUDE.md`(+`--append-system-prompt`). 구조화 리포트 = `--output-format json` (+`--json-schema`).
- 1차 조사(전 세션)에서는 "전달=스킬/스크립트(MCP 아님), 오케스트레이션=스킬단계+서브에이전트" 확인 → 메모리 `multi-node-cc-architecture`. 2차 조사로 *머신 간* 통신을 헤드리스 CC로 정밀화.

## 4.5. 통신 실증 (2026-06-07, 성공) ✅
서브노드 워크스페이스 셋업 + 첫 교차-에이전트 헤드리스 호출 검증.
- 셋업: 서브 origin 제거(원격 분리, 로컬 이력 보존) · 빌드워커 CLAUDE.md 배포(정본 메인 `.claude/.../sub_node/`, 체크섬 일치) · sync_to_sub.sh 에 CLAUDE.md 제외 추가 · 서브 claude=`/home/cona/.local/bin/claude` v2.1.168(**login shell PATH only** → `bash -lc` 필수).
- 호출: `ssh sub 'bash -lc "cd ~/ws_docker/vllm_serving_server && claude -p \"...\" --output-format json --permission-mode dontAsk --allowedTools \"Bash(docker --version),Read\""'`
- 결과: exit 0, `subtype:success`. 서브가 **CLAUDE.md 페르소나 읽음**(slave/192.168.100.11/GB10 보고) + **리포트 스키마 JSON 반환** + docker 29.2.1 실측. nvidia-smi·ip 는 allowedTools 밖이라 **auto-deny(hang 없음)** → `status:partial` + "메인이 nvidia-smi 를 allowedTools 에 추가해 재호출 요망" 센스있게 제안.
- 학습: `bash -lc` PATH 해결 · allowedTools 는 체크리스트 필요 커맨드를 정확히(덜 주면 partial 로 안내) · 서브 모델 sonnet-4-6, $0.13/호출, session_id 재개 가능 · 엄격 구조화는 `--json-schema` 로 `structured_output` 직접.
- **결론: 비대칭 분산-CC 통신 메커니즘 실증 완료.** 메인=오케스트레이터, 서브=자율 빌드워커+클린 리포터.

## 5. 산출물 / 다음
- 발행: `docs/plan/plan_260607_5`(설계), 이 devlog. 코드: `.claude/skills/upstream-version-watch/scripts/sync_to_sub.sh`.
- 다음 산출물(순서): ① 서브노드 `CLAUDE.md`(빌드워커 페르소나) → ② JSON 리포트 스키마 → ③ `ssh sub claude -p` 오케스트레이션 래퍼 → ④ 서브 워크스페이스 리셋 절차 → ⑤ 멀티노드 path 통합 + Seed brownfield 반영.
- 실 검증은 멀티노드 모델 NAS 사전배치 후 end-to-end(서브빌드 ~9GB + 2노드 Ray 서빙 + multi-smoke).
