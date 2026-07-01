# devlog/ — 작업 로그 (작업 **중·후**)

> 이 `example.md`는 **추적되는 스켈레톤**이다(폴더 구조 + 작성 규칙 배포용). 실제 작업 로그는
> 이 폴더에 추가하되 **git 추적 대상이 아니다**(gitignore — 브랜치 간 persist·통합). 작성 규칙 정본: `.claude/rules/docs.md`.

## 역할
- 실제 수행한 **작업 내역·결정·전파의 서사**("무엇을 했나").
- 담는 것: 작업 흐름, 핵심 사건/결정(+근거), 교훈, **최종 상태**(미커밋·다음 작업 명시).
- 검증을 동반한 작업이면 보통 devlog + testlog 한 쌍. 검증 *증거/판정*은 testlog로 분리.

## 원시맥락 범주 (다음 세션 warm-start 용 — 서사 요약만으로 복구 불가한 것들)
- **시도-폐기 경로**: 무엇을 시도했고 왜 버렸나 — 음성결과 포함("안 된 것"이 다음 세션의 지도).
- **재현 커맨드 verbatim**: 핵심 커맨드는 요약하지 말고 그대로(복붙 재실행 가능해야 함). 대용량/가변
  로그 원문은 simlog run 에 적재하고 경로 인용.
- **판정 어휘**: 단정 결론엔 유효맥락 한정자 병기(예 "비가용[맥락: <이미지>·<HW>·<일자> 시점]").
  파킹은 `- PARKED:` 접두사, 후속 예고는 `- [ ] 후속:` 체크박스(닫을 때 해소 문서 경로 채움). 정본: docs.md §3.

## 미완결 세션 devlog 필수 최종 섹션 — 미니 예시 body (합성값)
````markdown
## N. ★ 최종 상태 + 재개 지침 (다음 세션 필독)
### N.1 디스크/노드 실상태
- 이미지: easy-vllm:<vllm>-cu<cuda>-<arch>-source (양노드 빌드됨) · build_patches: 10-,20-,40-
- 미커밋: output/multi/build_patches/15-foo.sh (신규 · 미스모크)
### N.2 의사결정 대기 (후보별 정확 값)
- 후보A = upstream nv_dev(SHA `abc1234def...`) / 후보B = 포크 유지 — 판단 근거 = testlog_<YYYYMMDDHH>_<seq> §3
### N.3 재개 커맨드 (verbatim)
    READY_MAX=300 bash .claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh <config> --build --keep-up
    # 크래시 진단: docker logs <svc> 2>&1 | grep -nE "undefined symbol|CUDA error"
### N.4 예상 벽
- <다음 시도에서 부딪힐 것으로 예상되는 지점 + 근거 1줄>
### N.5 잔여/파킹
- [ ] 후속: <검증 예정 항목>
- PARKED: <미결 항목> (사유 1줄)
````
(완결 세션은 N.1 수준이면 충분 — 정본: docs.md §2 devlog.)

## 파일명 규칙
```
docs/devlog/devlog_<YYYYMMDDHH>_<seq>_<주제>.md
```
- `YYYYMMDDHH`: 작성 일시 절대표기 (예 `2026060814` = 2026-06-08 14시). 상대날짜 금지.
- `seq`: 같은 일시·같은 type 내 일련번호(1부터).
- `주제`: 한국어, 밑줄(`_`) 구분, 버전·대상 포함 권장.
- 예: `devlog_2026060814_1_멀티노드_소스빌드_확장_검증.md`
