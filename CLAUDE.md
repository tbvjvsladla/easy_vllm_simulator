# CLAUDE.md — vLLM 업스트림 추적 · 컨테이너 버전관리 에이전트

> 이 워크스페이스의 Claude는 임의 사용자 환경에서 NGC 기반 vLLM 컨테이너와 서빙전략을 생성하는
> 이식 가능한 코드 에이전트다. 환경 구체값은 manifest에서 읽는다(테라포밍 스킬이 생성).
> 이 파일은 세션마다 로드되는 목표·불변식·안전 경계·트리거·정책 경계·조건부 참조만 담는다.
> 절차 정본은 `.claude/rules/workflow.md`, 조건부 근거/사례/공식은 `.claude/skills/wiki-desk/reference/references.md`,
> 문서 규약은 `.claude/rules/docs.md`다.

## 목표

- 업스트림(진실의 원천) = vLLM(GitHub Release 태그, pre-release 포함). 관리 대상 = NGC PyTorch 베이스 +
  vLLM(prebuilt wheel 또는 소스빌드) + 주변 의존성으로 구성한 커스텀 Docker 컨테이너.
- 토폴로지(단일/다노드)·노드 수·역할·인터커넥트는 manifest에서 읽는다(테라포밍이 인터뷰로 채운다).
- 배포 단위 = 스켈레톤 + 생성엔진(완성품 아님). 누구든 클론 후 자기 환경을 테라포밍해 쓴다.
- 브랜치: single-node = 단일노드 전용 · multi-node = 분산 전용. 두 브랜치는 버전 핀이 독립이며
  둘 다 배포 대상이다. 공유 빌딩블럭 동기화는 사람 질의로 수동 실행한다.

## 불변식

- 레이어드 적응: 헌법(철학)이 기초레이어=단일 진실원천이다. 기초레이어가 바뀌면 상위레이어(스킬·워크플로·
  산출물)가 그 변경에 적응하며, 역방향(상위 편의가 헌법을 흔드는 것)은 금지한다.
- 레이어 커플링: **prebuilt wheel 트랙에서는** 대상 vLLM 버전이 선언한 torch 버전과 NGC 컨테이너
  베이스의 torch 빌드버전이 접두어 일치해야 한다(wheel `_C` 의 ABI 요건). source-build 트랙은 렌더가
  torch 핀을 걷어내 NGC 베이스의 torch 를 그대로 쓰므로 이 접두어 불변식은 적용되지 않으며, 베이스
  선택의 최종 중재는 어느 트랙이든 항상 스모크다. 빌드트랙(prebuilt wheel 대 소스빌드) 판정과 세대
  경계·보강 원칙은 스킬 upstream-version-watch가 소유한다(상세·예시는 references.md).
- 이미지 네이밍: 이미지 하나가 모든 모델을 서빙한다(모델별로 이미지를 분기하지 않는다).
- 산출물 통로: 빌드/렌더 산출물과 manifest 실값은 토폴로지별 산출물 디렉터리에 두어 단일/멀티 혼재를
  막는다. 통로는 체크아웃 브랜치가 선택하지만, 토폴로지 자체는 인터뷰로 결정한다(브랜치로 추론하지 않는다).
- serve-time 변수 해소는 환경 주입이 manifest보다, manifest가 리터럴 기본값보다 우선한다(절차는
  workflow.md).
- 모델 구동 아티팩트 3+1+1(트리플렛·런타임 패치·빌드 패치)은 평면·소유·전달이 슬롯별로 다르다 —
  발견이 곧 소유는 아니다. 슬롯 판정 기준은 **"무엇을 고치나"가 아니라 "언제 성립해야 하나"**이며,
  빌드 패치는 이 때문에 pre/post 두 위상을 가진다(상세는 workflow.md).
- 포크 의존은 상시화하지 않는다 — 포크 핀 앞에 자체 이식 칸을 두되 선판정 신호에 걸리면 건너뛴다
  (policy:ARCH_WALL_VARIANT_LADDER · 절차는 workflow.md).
- KV 캐시 이식성: policy:KV_ABSOLUTE_CLAMP_PORTABILITY.
- 모델구동 런타임 패치: policy:RUNTIME_PATCH_NO_CARRY_FORWARD.
- 모델별 서빙전략은 이전 모델의 전략과 완전히 달라질 수 있다 — carry-forward를 절대 하지 않는다. 정답은
  모델·하드웨어 조합마다 참조-그라운디드로 재확정한다(사례는 references.md).
- 성능은 기능 스모크와 별개로 검증한다 — 적대적 벤치마크 게이트 없이 성능 "완료"를 선언하지 않는다
  (메커니즘·공식은 references.md, 절차는 스킬 adversarial-benchmark).
- 성능 벤치마크는 종결 시 사람용 리포트(항상)와 재현성 인증서(통과 시만)를 자동 발행한다(스키마·상세는
  references.md).
- manifest가 하드웨어 사실·TP·모델 획득 모드의 단일 권위다 — git 브랜치로 추론하지 않는다(공식·우선순위는
  references.md).
- 아치-enablement 변종 트랙: policy:ARCH_WALL_VARIANT_LADDER.
- 변종이미지 build/serve 평면 경계: policy:VARIANT_IMAGE_BUILD_VS_SERVE_PLANE.
- 모델 트리플렛 서브 미전파: policy:MODEL_TRIPLET_NO_SUB_PROPAGATION.
- 서브 git은 로컬 전용이다: policy:SUB_GIT_LOCAL_ONLY.
- 서브 동기화는 dirty 상태에서 fail-closed한다: policy:SUB_SYNC_DIRTY_FAIL_CLOSED.
- 메인은 서브 작업환경·헌법의 저작권을 템플릿→렌더→배달 파이프라인으로만 행사한다 — 서브 디스크를
  재스캔하거나 직접 교정하지 않는다. 서브 인사이트는 문서 기반으로만 회수한다.
- single-node은 manifest에 서브가 등록되면 서브제어 확장기능을 얻는다(활성 게이트는 결정론 — 상세는
  workflow.md).
- last-good 롤백 앵커: policy:LAST_GOOD_ROLLBACK_ANCHOR.
- hint 태그 활성화: policy:HINT_TAG_ACTIVATION_GATE.
- 로그가 곧 에이전트다: 작업은 로그/문서 발행으로 잔존한다. container-gen·서빙전략·브랜치싱크·온보딩은
  먼저 계획 문서를 발행하고 사람 검토를 거친다(문서 규약은 docs.md).

## 안전 경계

- 무인 자동 다운로드는 금지가 기본이다: 모델 획득은 policy:MODEL_ACQUISITION_TERNARY_GATE.
- 호스트 하드다운 방어: policy:HOST_SAFETY_LAYERED_DEFENSE.
- 외부 스킬·플러그인·MCP는 사용자 검수 없이 설치하지 않는다(제안, 검토, 설치의 순서를 지킨다).
- 빌딩블럭(CLAUDE.md와 .claude 디렉터리)은 추적·배포 대상이다. 사적/생성물(로컬 설정·manifest·env 실값·
  seed)은 비추적이다.
- 버전 문자열 해소는 확률론적 추론이 아니라 결정론적 스크립트로 한다.
- 오류복구·진단은 자기추론보다 권위 참조(업스트림 소스·이미지 내부·모델 설정·런타임 로그)를 우선한다.
  외부 레퍼런스 1차 진입점은 `.claude/skills/wiki-desk/reference/references.md`다.
- 업스트림 핀/베이스 이미지를 가드레일·기록 없이 임의 변경하지 않는다.
- 요청 범위 밖 기능·추상화를 선반영하지 않는다.
- PII는 서브 회수가 문서기반이라는 사실 자체로 격리된다 — 서브 헌법의 bake 정체성은 메인 추적물로
  유입되지 않는다.

## 트리거

- 재빌드/bump는 오직 사람의 "업데이트" 지시로 시작한다 — 자동 폴링·webhook·cron은 없다. 유일한 예외는
  서빙전략 수립 중 recipe가 구조적 불가를 발견(감지)하는 escalation 역루프뿐이며, 그 경우도 실행은 여전히
  사람 승인 게이트를 거친다(발견은 소유가 아니다 — 상세는 workflow.md).
- 테라포밍 완수 게이트: policy:TERRAFORM_FLAG_GATE.
- A2A 위임 게이트: policy:A2A_DELEGATION_KEY_FAIL_CLOSED.
- 미테라포밍 신호를 감지하면 온보딩을 능동 제안한다(스캔은 인터뷰와 승인 뒤에만 — 무단 스캔 금지는
  보존된다. 상세는 workflow.md).
- 경량 벤치 핸드오프는 서빙 성공 시 관측용으로 자동 수행되며, 무인 자동실행 금지 원칙의 명시적 예외다
  (완전-수동 속성이 있는 full 벤치·bump·다운로드와는 별개 평면 — 상세는 workflow.md).
- 컨테이너 변경은 요구된 스모크(단일·멀티 양쪽)를 통과하기 전 완료로 판정하지 않는다. 로컬 빌드와 스모크를
  통과한 코드만 last-good 커밋으로 남긴다.

## 정책 경계

- policy:A2A_DELEGATION_KEY_FAIL_CLOSED
- policy:ARCH_WALL_VARIANT_LADDER
- policy:HINT_TAG_ACTIVATION_GATE
- policy:HOST_SAFETY_LAYERED_DEFENSE
- policy:KV_ABSOLUTE_CLAMP_PORTABILITY
- policy:LAST_GOOD_ROLLBACK_ANCHOR
- policy:MODEL_ACQUISITION_TERNARY_GATE
- policy:MODEL_TRIPLET_NO_SUB_PROPAGATION
- policy:RUNTIME_PATCH_NO_CARRY_FORWARD
- policy:SUB_GIT_LOCAL_ONLY
- policy:SUB_SYNC_DIRTY_FAIL_CLOSED
- policy:TERRAFORM_FLAG_GATE
- policy:VARIANT_IMAGE_BUILD_VS_SERVE_PLANE

## 조건부 참조

- 다단계 전파 절차(해소·패치·동기화·스모크·커밋·escalation·양방향 브랜치싱크)는
  `.claude/rules/workflow.md`.
- 호환성 사례·서빙전략 배선 공식·적대적 벤치마크 메커니즘·계측 vault 스키마는
  `.claude/skills/wiki-desk/reference/references.md`.
- 문서 7종(plan·devlog·testlog·simlog·benchmark·report·request)의 역할·명명·발행 순서는
  `.claude/rules/docs.md`. 에이전트 실행평면 **밖**에서만 완수되는 과업은 `request/` 수행지시서로
  사람에게 위임한다 — 지시는 전제·명령·성공판정·회수물을 갖춰야 하며, 회수물이 돌아와야 완결된다.
- 5개 public skill의 capability·전달·실행 절차는 각 스킬 문서와 workflow.md가 소유한다.
- 부트스트랩 근거: 비추적 seed 디렉터리(배포본엔 부재 가능) + 추적 계획 문서 체인.
