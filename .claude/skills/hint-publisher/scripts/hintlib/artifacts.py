"""hintlib.artifacts — 실제로 쓰인 재현 자산만 페이로드에 싣는다 (plan_26092119 §4.5·§4.6 · SPEC §5.6).

무엇을 푸는가
    옛 `hint_collect.discover_slots` 는 **경로에 파일이 있다는 사실**을 적용 증거로 읽었다. 그 결과 source-build 셀에도
    wheel 스켈레톤 `Dockerfile` 이 실렸고(F2 · 46/46 태그에 같은 blob `dbc6f3ca`), 자기게이트로 skip 한 패치(50·55)가
    "3-signal" 로 배포됐다(F3). 그 3-signal 의 "증거" 는 skip 한 패치 50 자신의 이식 기록 `PROVENANCE.json` 이었다(K3) —
    "이식 기록은 '이식했다' 는 증거이지 '이 빌드가 실행했다' 는 증거가 아니다". 이 모듈은 그 규칙을 **삭제**하고 아래로 바꾼다.

절단선 (날짜 박힌 불변식 — 옛 자리에서 옮겨 왔다)
    - **평면 먼저**: `docker|native`. 다른 평면 파일 ✗ · 쓰이지 않은 Dockerfile ✗(F2). 평면을 파생할 신호가 없으면 차단
      (`HINT_PLANE_UNDERIVABLE`) — "신호 없음 = native" 는 결정 경로의 침묵 폴백이다.
      ⚠ 알려진 한계(2026-09-22 통합 결정 D-d): native 평면 신호를 내는 producer 가 아직 없다(serve phase 의 평면 관측 배선 부재) —
      native 셀은 fail-closed 로 막힌다. 해소는 신호 배선이지 추측이 아니다(S1–S5 범위 밖).
    - **값의 지위 커버리지 = 튜닝 노브**(D-b · 2026-09-22): 전송·신원 노브 `model`·`host`·`port`·`served-model-name` 은 닫힌
      목록(`VALUE_STATUS_EXCLUDED_KNOBS`)으로 후보에서 뺀다 — 그 밖의 서빙 yaml 노브는 전부 value-status 블록을 요구한다.
    - **선택자가 태그보다 먼저**(2026-09-04 실측: `0.18.0-…-wheel` 태그가 0.27.1 소스빌드 내용을 가리켰다 — 태그는 가변
      포인터). 순위 = 빌드 원장 `dockerfile` → 셀 env `BUILD_DOCKERFILE` → compose 기본값(`slave_forward.compose_default`).
      옛 태그 휴리스틱(`-source` 포함 여부)은 삭제했다(코드맵 hint_collect §3.5 "decision-path silent fallback").
    - **빌드 레시피 = 쓰인 Dockerfile 1종 + 그 Dockerfile 이 COPY 하는 파일**. 목록을 손으로 적지 않고 Dockerfile 텍스트에서
      파생한다(render_dockerfile `_ensure_copy_context_dirs` 와 같은 관용). 그래서 source-build 도 `requirements.txt` 를
      싣는다 — `Dockerfile.source-build:68` 이 그것을 `/etc/pip/constraint.txt` 로 쓴다(태그2 이미지 history 의
      `COPY requirements.txt` 층으로 확인 · 2026-09-22). 빼면 수신자의 빌드가 COPY 에서 죽는다. **통합 결정 D-a**(2026-09-22):
      규칙은 "쓰인 것만" 이지 "wheel 트랙만" 이 아니다 — SPEC §5.6 의 "requirements(wheel 트랙에서만)" 문구를 이 결정이 대체한다.
    - **적용 집합 순위**(SPEC X9·X12): ① serve 시점 캡처 원장(attestation) ② 메인 로컬 이미지 원장
      (`/opt/easy-vllm/build_ledger.json` · 서브 스캔 ✗ — 헌법 노드제어 ①) ③ 옛 이미지(원장 없음) = `unobservable` + 라벨된
      재구성 v2 ④ 재구성도 못 하면 `적용 미관측` 으로 싣는다. 재구성이 skip 이라 말한 패치는 **싣지 않는다**.
      각 순위의 탐침 결과는 `applied_set.probes[]` 에 남긴다 — 조용한 강등 ✗.
      2026-09-22 S2 round 2: ② 는 실행기가 이미지 탐침 능력(`image_probe`)을 선언하면 **시작하지 않는** 컨테이너에서
      `docker create`(--pull never · --network none) → `docker cp` → `docker rm`(finally) 로 읽는다(옛 `docker run … cat` 은
      컨테이너를 시작한다 — 능력을 선언하지 않은 주입 실행기에만 남긴 옛 경로). ③ 재구성 v2 = (i) 자기게이트 × history 빌드 인자
      → skipped · (ii) skip 경로 없는 스크립트 × fail-loud 루프 × 측정 이미지 실재 × 판정한 바이트 = 이미지에 구워진 바이트(탐침 ·
      없으면 미관측 — 리뷰 교정: 탐침 없이 작업트리 바이트로 판정하면 이미지에 없는 스크립트가 applied 가 됐다) → applied
      `reconstructed(fail-loud+built)` ·
      (iii) 내용 조건부 skip → 이미지 탐침(구워진 스크립트 sha × 선언 마커 전부 실재)이 가르면 applied `image-probe(script-sha+marker)`
      · 그 밖 미관측. round 1 은 9개 중 7개를 미관측으로 실었고 기계 채점기는 같은 관측으로 6개를 적용 · 1개를 판정 불가로 갈랐다.
    - **실린 Dockerfile = 이미지를 지은 개정**(F1 · 2026-09-22): 작업트리가 측정 이미지 docker history 의 RUN·COPY 원문과 전수
      일치하지 않으면 `git log -1 --before=<Created>` 개정을 대조해 더 잘 맞을 때만 그 바이트(git show)를 싣는다
      (`slots.build_recipe.evidence.selected_revision`). COPY 대상 추적 파일은 이미지 안 바이트로 대조한다.
    - **측정 뒤 재생성된 env 형상은 싣지 않는다**(F2 · 2026-09-22 · S2 round 3 강화): mtime > 측정 시각(인증서 measured_utc →
      스윕 generated_utc) 이면 형상 파일 자체를 싣지 않고 `slots.compose.excluded`(`why` = `post-measurement-regenerated(…)` ·
      `reason` = `post-measurement-regenerated`)와 `post_measurement_regenerated` 에 적는다. round 2 는 값만 가리고 키는 실었는데,
      재생성 시점 렌더러가 측정 **뒤** 더한 키(태그2: 09-17 커밋 782fd70 의 `NCCL_NET` · fe1bc42 의 `NCCL_DMABUF_ENABLE`)가 이 셀의
      형상인 척 실렸다(오도 바이트 1,112 B 의 대부분). 측정 당시 env 는 evidence 의 `facts.measurement_env_observed`(엔진 로그
      관측)가 사실로 대신한다.
    - **compose 슬롯 추적 파일 = 측정 당시 개정**(2026-09-22 · S2 round 3): docker-compose.yaml · 서빙 러너 정본(asset)의 작업트리가
      측정 **뒤** 바뀌었으면(mtime > 측정 시각) `git log -1 --before=<측정>` 개정의 바이트를 싣고 선택 방법·교차대조(측정 뒤 커밋 ·
      HEAD 대비 수정본)를 `slots.compose.evidence.selected_revision(s)` 에 적는다. round 2 는 09-22 에 주석이 고쳐진 작업트리 compose
      를 실었다(주석이 측정 당시 NCCL 키 수를 틀리게 말했다). 서빙 경로 파생(러너 · env_file 참조 · 서브 전달 목록)도 그 개정 텍스트로
      한다 — 측정 뒤 더해진 참조를 이 셀의 입력으로 세지 않는다.
    - **arm_patch.sh 는 무장하는 셀에만**(F3 · 2026-09-22) — `<CONFIG_FILE>_patch.py` 가 없으면 no-op 이라 `not-armed` 로 뺀다.
    - **탐침 기본 ON**(2026-09-22 · S2 round 3 · hint.py 기본값과 같은 의미): `runner=None` 이면 이 모듈의 **탐침 전용 기본 실행기**
      (`_probe_only_runner` — image inspect · history · 시작하지 않는 create(`--pull never` · `--network none`) · 자기가 만든 컨테이너의
      cp · rm 만 · run/start/exec/build/pull = rc 125 거부)를 쓰고 탐침한다. 주입 실행기는 `image_probe = True` 를 선언할 때만
      탐침한다 — 선언이 없거나 False 면 **읽기 전용 실행기**로 본다(hint.py `--docker-read-only` · 자체검사 대역). 계약: hint.py 는 자기
      실행기를 넘기고, 그 기본 실행기가 이 능력을 선언한다. round 2 의 기본(None)은 탐침은 켰지만 **제한 없는** subprocess 였다.
      탐침 컨테이너는 `rm -v` 로 지운다(create 가 만든 익명 볼륨까지 · 2026-09-22 round 3 적대 리뷰).
    - **render 단계는 렌더러 개정을 말한다**(2026-09-22 · S2 round 3 적대 리뷰 · 2차 바이트 채점 X 01:384): render_dockerfile.py 의
      측정 전 마지막 커밋 바이트가 작업트리와 다르면(git 관측) 재현 절차 render 명령 앞에 그 개정 · 측정 뒤 커밋 수를 셸 주석으로 달고
      단계 행에 `renderer_revision` 을 싣는다 — 지금 렌더러로 만든 env 형상은 측정 당시 형상이 아니다(F2 와 같은 사실의 다른 자리).
    - **이미지 관측은 측정 digest 로만**(2026-09-11 R5 "태그는 이름" · 2026-09-22 감사): ②·③·Created 는 evidence 가 묶은
      `build_identity.image_digest` 를 docker 참조로 쓴다. 태그는 측정 뒤 재빌드되면 다른 이미지를 가리키므로, digest 이미지가
      로컬에 없으면 태그로 대체하지 않고 미관측으로 남긴다. 재구성의 게이트 실효값은 셸 `:=` 의미(빈 값도 기본값으로 채움)로 읽는다.
    - **wheel 트랙은 build_patches* 를 한 번도 실행하지 않는다**(2026-09-04: 렌더만 해 두고 wheel Dockerfile 은 COPY 하지
      않는다 — 디렉터리만 보면 "있음" 이 되어 먹지 않은 패치가 재현지침으로 배포됐다). 쓰인 Dockerfile 이 COPY 하지 않는
      패치 디렉터리는 `excluded_by_recipe` 로 기재하고 싣지 않는다(침묵 배제 ✗).
    - **`.gitkeep` 은 패치가 아니다**(2026-09-01 실물 결함: `glob('*')` 가 `.gitkeep` 을 빌드 패치로 셌다) — 알려진 비-패치
      (`.gitkeep`·`PROVENANCE.json`)는 명시적으로 건너뛰고, 그 밖의 규약 밖 이름은 **소리내어** 기재한다(부재 ≠ 결측).
    - **post 패치는 공유 이미지 입력이다**(SPEC X10 · 이미지 네이밍 불변식 "이미지 하나가 모든 모델"): 적용됐으면 싣고
      `model-trigger` 헤더로 "이 모델의 요구가 아니라 공유 이미지의 일부" 라벨을 단다.
    - **env 형상은 키를 전부 남기고 값만 치환한다**(2026-09-06 plan_26090616 Q9: "키를 지우면 수신자는 그 변수의 존재 자체를
      모른다 — 멀티 클러스터 5변수가 그렇게 빠졌다"). `.env.cluster`·`.env.interconnect` 는 render_dockerfile 의 3층
      (`env_tier(key)` · 부재 시 같은 모듈의 프리셋/불변 상수에서 층을 파생하고 그 사실을 헤더에 적는다), 그 밖의 `.env` 는
      옛 키-축 규칙(값의 모양이 아니라 **키의 신원성**). 형상화 뒤 PII 4종 + 용어 백스톱이 걸리면 **덧칠하지 않고 차단**.
    - **서브 전달 목록은 upstream `slave_forward.py` 가 유일하게 파생한다**(D3 · 목록이 두 곳에 손으로 적히면 갈라진다 —
      2026-07-24~09-09 다섯 번의 침묵 누락). 여기서 키 목록을 다시 적지 않는다. 그 값 중 운영자 경로(mount)는 싣지 않는다.
    - **서브를 다시 스캔하지 않는다** — 스모크가 회수한 attestation 만 읽는다(plan §4.5 의 직접 ssh 관측 설계 교정).
    - **시각은 주입 또는 관측만**: 벽시계 ✗. 재현 절차의 소요 시간은 관측 가능한 시각(이미지 Created · 블랙박스 예산 선언 ·
      측정 date · 스윕 generated_utc · 캠페인 phase)에서만 계산하고, 못 보면 "미관측" 이라 적는다.

import 부수효과 0 — 정규식 컴파일만 한다. docker 는 읽기(`image inspect`·`history`)와 **시작하지 않는** 컨테이너의
`create`·`cp`·`rm` 만 부른다(능력을 선언하지 않은 주입 실행기에는 옛 `run --network none` 의 원장 cat 만 — 읽기 전용 실행기는 그것을
거부한다 · 기본 실행기는 `run` 을 부르지 않고 불러도 거부한다). 호출부가 `runner` 로 바꿀 수 있다(자체검사는 가짜 runner 를 쓴다).
"""
from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from dataclasses import dataclass, field
from pathlib import Path, PurePath
from typing import Any, Callable

from . import core

SLOTS = (
    "triplet", "runtime_patch", "build_patch_pre", "build_patch_post",
    "build_recipe", "compose", "fork_pin",
)

LEDGER_IN_IMAGE = "/opt/easy-vllm/build_ledger.json"      # owner upstream-version-watch (build_plane.md §2)
REL_ARCH_VARIANT_LEDGER = ".claude/policies/arch_variant_ledger.json"
REL_SMOKE_MULTI = ".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh"
REL_SMOKE_SINGLE = ".claude/skills/upstream-version-watch/scripts/single_serve_up.sh"
PATCH_DIRS = {"pre": "build_patches_src", "post": "build_patches"}
# 알려진 비-패치(2026-09-01 `.gitkeep` 결함 · PROVENANCE.json 은 매 실행 "규약 밖" 소음이었다 — 코드맵 hint_collect §3.6).
KNOWN_NON_PATCH = frozenset({".gitkeep", "PROVENANCE.json"})
DOCKER_TIMEOUT_S = 60

_PATCH_NAME = re.compile(r"^\d+-.*\.sh$")
_MODEL_TRIGGER = re.compile(r"^\s*#\s*model-trigger\s*:\s*(.+?)\s*$", re.MULTILINE | re.IGNORECASE)
# 자기게이트(build_plane.md §1.6): `: "${VAR:=0}"` 기본값 선언 + `if [ "${VAR}" != "1" ]; then … exit 0`.
# 실물은 따옴표를 두른다(50:128 · 55:48-49) — 옛 픽스처는 따옴표 없는 꼴만 써서 실물을 못 봤다(픽스처가 실물보다 좁다).
_GATE_DEFAULT = re.compile(r'^\s*:\s*"?\$\{([A-Z][A-Z0-9_]*):=([^}]*)\}"?\s*$', re.MULTILINE)
_GATE_SKIP_IF = re.compile(r'^\s*if\s+\[\s*"?\$\{?([A-Z][A-Z0-9_]*)\}?"?\s*!=\s*"?1"?\s*\]\s*;\s*then\s*$')
# 게이트 **앞**에 허용되는 부수효과 없는 줄(주석·빈 줄 제외). 이 밖의 줄이 게이트보다 먼저 오면 "skip = 아무것도 안 했다" 를
# 단정할 수 없다 → 재구성하지 않는다(보수). 명령 치환(`$(`·백틱)은 어느 꼴에서도 금지.
_PRE_GATE_OK = (re.compile(r"^\s*set\s+-[A-Za-z]+(?:\s+[a-z]+)*\s*$"),
                re.compile(r"""^\s*[A-Za-z_][A-Za-z0-9_]*=(?:"[^"`]*"|'[^']*'|[^\s`;|&]*)\s*$"""),
                re.compile(r'^\s*:\s*"?\$\{[A-Za-z_][A-Za-z0-9_]*:=[^}]*\}"?\s*$'),
                re.compile(r"^\s*if\s+\[.*\]\s*;\s*then\s*$"), re.compile(r"^\s*fi\s*$"),
                re.compile(r"^\s*echo\s.*$"))
_HISTORY_RUN_ARGS = re.compile(r"^RUN \|(\d+) (.*)$")
_DOCKERFILE_COPY = re.compile(r"^\s*(COPY|ADD)\s+(.+?)\s*$", re.MULTILINE | re.IGNORECASE)
_CONFIGS_REF = re.compile(r"/app/configs/([A-Za-z0-9_.${}:-]+\.sh)")
_SOURCED = re.compile(r"(?:^\s*|[;&|]\s*|\bthen\s+)(?:source|\.)\s+\"?/app/configs/([A-Za-z0-9_.-]+\.sh)", re.MULTILINE)
_ENV_FILE_REF = re.compile(r"(?<![\w/.-])envs/(\.env\.[A-Za-z0-9_.-]+)")
_RUNNER_GPU_EQ = re.compile(r'"\$\{GPU_COUNT\}"\s*=\s*"(\d+)"')
_RUNNER_GPU_RAY = re.compile(r"(\d+)\.0/(\d+)\.0 GPU")
_LOCAL_DATE = re.compile(r"^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})$")

# ── 2026-09-22 · plan_26092119 S2 round 2 — 이미지를 지은 레시피 · 측정 뒤 재생성 · 옛 이미지 재구성 v2 ──────────────
# 왜: round 1 채점에서 오도 바이트 12.7KB 의 셋이 모두 여기서 나왔다 — ① 실린 Dockerfile 이 이미지 빌드 **뒤** 원장 스탠자가
#   붙은 작업트리 수정본이었다(history 와 맞는 것은 커밋 dca0a0f) ② `.env.interconnect` 가 측정(09-12) 뒤 09-17 에 NCCL_NET=Socket
#   으로 재생성됐는데 "값 유지 — 재현 사실" 로 실렸다(측정 엔진 로그는 NET/IB) ③ 무장하지 않은 셀에 arm_patch.sh 가 실렸다.
#   그리고 9개 패치 중 7개가 "적용 미관측" 이었는데 기계 채점기는 같은 관측(docker history · 이미지 파일)으로 6개를 적용, 1개를
#   판정 불가로 갈랐다 — 관측할 수 있었던 것을 관측하지 않은 것이다.
# 이미지 탐침: 측정 digest 로 컨테이너를 **만들기만**(start ✗ · `--pull never` · `--network none`) 하고 `docker cp` 로 파일을 꺼낸 뒤
#   finally 에서 반드시 `docker rm` 한다. 주입 실행기는 이 능력을 **선언**해야만 쓴다(속성 `image_probe = True`) — hint.py
#   `--docker-read-only` 실행기와 자체검사 대역은 선언하지 않으므로 탐침을 건너뛰고 그 사실을 탐침 기록에 남긴다.
#   2026-09-22 · plan_26092119 S2 round 3: hint.py 의 기본 publish 는 탐침을 켠다(읽기 전용 실행기를 주입할 때만 끈다) — 그 계약은 hint.py 가
#   **자기 실행기를 넘기고** 그 실행기가 이 속성을 선언하는 것이다. `runner=None`(라이브러리 직접 호출)은 이 모듈의 탐침 전용 기본 실행기
#   (`_probe_only_runner` · 선언함)로 같은 의미가 된다 — round 2 에는 None 이 제한 없는 subprocess 였다(두 기본값이 갈렸다).
PROBE_ATTR = "image_probe"
# value-status 후보: yaml 줄 주석의 "필수"·"요구" 는 **저작자의 말**이지 관측된 요구가 아니다(2026-09-22 · S2 round 3 — 태그2
#   `async-scheduling: false  # … 명시 필수` 를 declared-requirement 로 제안했으나 그 요구를 뒷받침한 관측 실패는 계보에 없었다).
#   주석 단어만 있으면 inherited 로 제안하고 이 문구를 단다 — 관측된 실패가 있으면 저작자가 declared-requirement 로 올린다.
COMMENT_ONLY_REQUIREMENT_NOTE = "yaml 주석만 있음 — 관측된 실패가 있으면 declared-requirement"
# compose 슬롯의 측정 뒤 재생성 env 형상 제외 사유(공유 계약 문자열 — 소비자가 글자 그대로 대조한다).
POST_MEASUREMENT_REGENERATED = "post-measurement-regenerated"
# 측정 당시 env 값의 **수신자 좌표**(2026-09-22 · plan_26092119 S2 round 3 통합): 옛 문구는 발행기 내부 키 `facts.measurement_env_observed`
#   를 배포 본문(01 §1.1 싣지 않은 파일 · 01 §1.4 render 주석 · sub_recipe.json)에 실었다 — 수신자 zip 에 `facts` 는 없다. 좌표는 zip 안의
#   자리(01 §1.4 표 · PAYLOAD.json 최상위 키)로 적는다.
ENV_OBSERVED_POINTER = "01 §1.4 '측정 실행의 env 관측' 표 · PAYLOAD.json measurement_env_observed"
# 재현 절차 build 행의 관측 구간 한계(bound) — docker history 층 CreatedAt 첫 층 → 끝 층(2026-09-22 · S2 round 3 공유 계약).
LAYER_WINDOW = "layer-window"
# 런타임 패치 무장 러너(닫힌 tripwire · render_dockerfile.RUNNER_SCRIPTS 의 원소여야 한다 — 자체검사가 교차대조).
#   `/app/configs/${CONFIG_FILE}_patch.py` 가 없으면 no-op 이라, 무장하지 않은 셀에 실으면 "적용된 것만" 약속이 깨진다(F3).
RUNTIME_PATCH_ARMER = "arm_patch.sh"
REGENERATED_VALUE = "<측정 후 재생성 — 측정 당시 값 미관측>"
_PLACEHOLDER_VALUE = re.compile(r"^<[^<>\s][^<>]*>$")
_HEREDOC = re.compile(r"(?<!<)<<(-?)\s*(['\"]?)([A-Za-z_][A-Za-z0-9_]*)\2")
_RUN_FLAG = re.compile(r"^--(?:mount|network|security)=\S+\s+")
_SHELL_C = re.compile(r"^(?:\S*/)?(?:ba|da)?sh\s+-c\s+")
# 패치 스크립트 정적 분석(F12). 셸 층 `exit` — 코드가 0·생략·변수면 "성공 종료 가능 경로" 다(`bash "$p" || exit 1` 루프가 못 잡는다).
#   따옴표 안(`bash -c 'exit 0'`)도 센다 — 보수 쪽으로만 틀린다(2026-09-22 S2 round 2 리뷰).
_SH_EXIT = re.compile(r"(?:^|[;&|{(\s'\"])exit(?:\s+([^\s;&|)}#'\"]+))?\s*(?=$|[;&|)}#'\"])")
# 실패 삼킴(2026-09-22 S2 round 2 리뷰): `cmd || true` · `cmd || :` 는 그 명령의 실패를 set -e 로부터 숨긴다 — `set +e` 를 이미
#   (ii) 불성립 사유로 보는 것과 같은 이유다. 패치 단계 자체(`python3 - <<PY … PY || true`)에 붙으면 "성공 종료 = 적용" 이 거짓이 된다.
#   왼쪽이 정리 명령(닫힌 목록 · 실패해도 적용 여부와 무관)이거나 `VAR=$(…) || true`(탐침 값 · 뒤 검사가 쓴다)면 세지 않는다.
_SH_SWALLOW = re.compile(r"\|\|\s*(?:true|:)\s*(?=$|[;&)}#])")
_SWALLOW_OK_CMDS = frozenset({"rm", "rmdir", "unlink", "find"})
_SH_TRAP = re.compile(r"(?:^|[;&|{(\s])trap\s")
# 내장 파이썬의 성공 조기 종료(`sys.exit()`·`sys.exit(0)`·맨 `raise SystemExit`) — 파이썬만 끝나고 셸은 계속 가므로 뒤따르는 검증이
#   없으면 조용한 skip 이 된다(보수: 경로로 센다). `sys.exit(0 if … else 1)` 같은 조건식은 여기 걸리지 않는다(값이 0 리터럴이 아니다).
_PY_EXIT0 = re.compile(r"\b(?:sys\.exit|os\._exit|quit|exit)\(\s*(?:0\s*)?\)|\braise\s+SystemExit\s*(?:\(\s*(?:0\s*)?\))?\s*(?:$|;)")
_SET_E = re.compile(r"^\s*set\s+(?:-[A-Za-z]*e[A-Za-z]*\b|-o\s+errexit\b)")
_SET_NO_E = re.compile(r"^\s*set\s+(?:\+[A-Za-z]*e[A-Za-z]*\b|\+o\s+errexit\b)")
# skip 문구 — 빌드 원장 스탠자는 로그의 `] skip —`/`— skip` 토큰(render_dockerfile.SKIP_TOKEN)을 skipped 로 분류한다. 여기서는 그
#   정규식을 복제하지 않고 **상위집합**(단어 `skip` 이 나오는 실행 줄)을 skip 가능 경로로 센다 — 보수 쪽으로만 틀린다.
_SKIP_WORD = re.compile(r"(?i)\bskip")
_SH_ASSIGN = re.compile(r"^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$")
_SH_VAR = re.compile(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)")
_GREP_Q = re.compile(r"\bgrep\s+((?:-[A-Za-z]+\s+)+)(?:--\s+)?(?:'([^']*)'|\"([^\"$`\\]*)\")\s+(\"[^\"]*\"|'[^']*'|[^\s;|&)]+)")
_ASSERT_IN = re.compile(r"\bassert\s+(['\"])(.+?)\1\s+in\s+[A-Za-z_]")
_MODULE_AS = re.compile(r"\bimport\s+([A-Za-z_][\w.]*)\s+as\s+([A-Za-z_]\w*)")
_BRE_SPECIAL = set(".[]*^$\\")
_BENCH_JSON_KEYS = ("backend", "num_prompts", "max_concurrency", "date")

# 옛 env 형상화의 키 분류(2026-09-06 · plan_26090616 Q9 · 사용자 결정 "절대경로보다 신원류 형식").
# 종전 규칙은 "값이 `/` 로 시작하면 가린다" 하나뿐이라 운영자 호스트 IP·계정은 그대로 실렸고, `MAX_JOBS=8` 같은 재현 필수
# 튜닝값은 "우연히 `/` 로 시작하지 않아서" 남았다. 판정 축을 **키의 신원성**으로 바꾼다 — 새 변수도 이름만 보고 갈린다.
IDENTITY_KEY_AXIS = ("_HOST_IP", "_HOSTNAME", "_HOST", "_USER", "_IP", "_ADDR", "_ADDRESS",
                     "_SSH", "_KEY", "_TOKEN", "_SECRET", "_PASSWORD", "_ACCOUNT", "_MAIL")
# 튜닝류 — 값 자체가 재현에 필요한 사실이지 환경 지문이 아니다. **닫힌 tripwire**: 새 튜닝 변수가 생기면 여기 등재하며
# 그때 사람이 "정말 지문이 아닌가" 를 한 번 본다. (.env.cluster·.env.interconnect 는 이 목록이 아니라 render 3층을 쓴다.)
TUNING_KEYS = frozenset({
    "RAY_PORT", "MAX_JOBS", "PYTORCH_CUDA_ALLOC_CONF", "RAY_OBJECT_STORE_MEMORY",
    "VLLM_VERSION", "IMAGE_TAG", "TENSOR_PARALLEL_SIZE", "GPU_MEMORY_UTILIZATION",
    "MAX_MODEL_LEN", "MAX_NUM_SEQS", "KV_CACHE_DTYPE", "MOE_BACKEND", "ATTENTION_BACKEND",
})
ENV_TIERS = ("env", "preset", "invariant", "unknown")   # render_dockerfile.env_tier 의 반환 어휘(unknown = 3층 밖 키)
_TIER_ENV_KINDS = {".env.cluster": "cluster", ".env.interconnect": "interconnect"}

# lockset 예외 노브 출처(`*_source`) → 서빙 yaml 노브. 키 집합의 소유자는 recipe-explorer / campaign_template_validator
# `LOCKSET_KNOB_SOURCES` 다 — 이 표는 **노브 이름과의 대응만** 적는 닫힌 tripwire 이고 자체검사가 소유자 키와 교차대조한다
# (정적 목록끼리는 한쪽이 다른 쪽을 생성할 수 없다 — workflow.md 결정론 규율 "교차검증이 차선").
LOCKSET_SOURCE_KNOB = {"batch_source": "max-num-seqs", "gmu_source": "gpu-memory-utilization",
                       "kv_source": "kv-cache-memory-bytes"}
# 출처 어휘 → 값의 지위 **후보**(최종 판정은 01 PROMPT 저작자 · hint-event value-status).
_SOURCE_STATUS = {"measured-clamp": "tuned", "kv-fit-measured": "tuned", "declared-requirement": "declared-requirement",
                  "target_gmu": "declared-requirement", "hand": "inherited", "hand-lever": "inherited"}
# 값의 지위(value-status) 커버리지에서 **빼는** 서빙 yaml 노브 — 닫힌 tripwire(2026-09-22 통합 결정 D-b). 튜닝이 아니라
#   전송·신원 노브다: 어느 체크포인트를 어느 주소로 내보내느냐이지 "이 값이 조정됐나·승계됐나" 를 물을 대상이 아니다.
#   태그2 에서 모든 최상위 노브(12개 · model/host/port 포함)에 블록을 요구하자 저작 부담만 늘고 증류 원자는 늘지 않았다
#   (wave-1 template 보고). 목록 밖 노브는 전부 블록을 요구한다 — 새 노브를 여기 넣는 것은 사람 편집이다(리뷰 강제).
#   비교는 `-`/`_` 표기 차를 접는다(vLLM yaml 은 두 표기를 모두 받는다).
VALUE_STATUS_EXCLUDED_KNOBS = frozenset({"model", "host", "port", "served-model-name"})
# yaml 줄 주석의 단어 → 지위 후보(닫힌 목록 · 후보일 뿐). 순서 = 우선순위. declared-requirement 로 가는 단어(필수 · 요구)는
#   주석만으로는 inherited + `COMMENT_ONLY_REQUIREMENT_NOTE` 로 낮춰 제안한다(2026-09-22 S2 round 3 · value_status_candidates).
_COMMENT_STATUS = (("음성대조", "negative-control"), ("negative", "negative-control"), ("승계", "inherited"),
                   ("필수", "declared-requirement"), ("요구", "declared-requirement"), ("수렴", "tuned"),
                   ("측정", "tuned"))


# ── 작은 도우미 ──────────────────────────────────────────────────────────────────────────────
def _get(obj: Any, name: str, default: Any = None) -> Any:
    if obj is None:
        return default
    if isinstance(obj, dict):
        return obj.get(name, default)
    return getattr(obj, name, default)


def _read(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError as exc:
        core.fail("HINT_ARTIFACT_READ_FAILED", f"아티팩트 읽기 실패({path.name}): {exc}")


def _json_file(path: Path) -> dict | None:
    if not path.is_file():
        return None
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return value if isinstance(value, dict) else None


def _topology(ev: Any) -> str:
    decl = _get(ev, "declaration")
    control = decl.get("control_variables") if isinstance(decl, dict) else None
    ident = _get(ev, "identity")
    for cand in (_get(ev, "topology"), control.get("topology") if isinstance(control, dict) else None,
                 ident.get("topology") if isinstance(ident, dict) else None):
        if cand in ("single", "multi"):
            return cand
    core.fail("HINT_TOPOLOGY_UNDERIVABLE", "셀 증거에서 single|multi 토폴로지를 해소하지 못했다.",
              "evidence 가 manifest/캠페인 선언에서 topology 를 채워 넘긴다(브랜치로 추론 ✗).")


def _cell(ev: Any) -> str:
    cell = str(_get(ev, "cell") or _get(ev, "cell_id") or "")
    if not cell or "/" in cell or cell in (".", "..") or cell.startswith("."):
        core.fail("HINT_CELL_INVALID", f"셀 id 가 비었거나 경로를 포함한다: {cell!r}")
    return cell


def _utc_z(raw: str | None) -> str | None:
    """`2026-09-09T23:24:35.288+09:00` · `…Z` → `YYYY-MM-DDTHH:MM:SSZ`. 해석 불가 = None(합성 ✗)."""
    if not isinstance(raw, str) or not raw:
        return None
    import datetime as _dt
    s = raw.strip()
    m = re.match(r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})(?:\.\d+)?(Z|[+-]\d{2}:\d{2})$", s)
    if not m:
        return None
    tz = "+00:00" if m.group(2) == "Z" else m.group(2)
    try:
        d = _dt.datetime.fromisoformat(m.group(1) + tz)
    except ValueError:
        return None
    return d.astimezone(_dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _secs_between(a: str | None, b: str | None) -> int | None:
    if not a or not b:
        return None
    try:
        return int((core.parse_utc(b) - core.parse_utc(a)).total_seconds())
    except core.HintError:
        return None


def _fmt_duration(secs: int | None, bound: str) -> str:
    if secs is None:
        return "미관측"
    mm, ss = divmod(abs(secs), 60)
    # 한 시간 이상은 시간을 뗀다(2026-09-22 S2 round 3: 층 창이 하루를 넘으면 "1592분" 이 되어 읽히지 않았다).
    txt = f"{mm // 60}시간 {mm % 60}분 {ss}초" if mm >= 60 else f"{mm}분 {ss}초"
    return {"exact": txt, "upper": f"≤ {txt}(상한)",
            # 이미지 층 CreatedAt 첫 층 → 끝 층(2026-09-22 S2 round 3): 캐시 재사용 층은 **이전 빌드의 시각**을 가진다 — 창이지 소요가 아니다.
            LAYER_WINDOW: f"층 창 {txt}(docker history 첫 층 → 끝 층 CreatedAt · 캐시 재사용 층 포함 — 빌드 소요 아님)"}.get(bound, txt)


# ── 교차 스킬 소유 모듈 ──────────────────────────────────────────────────────────────────────
def _forward_module(repo: Path, module: Any | None = None):
    return module or core.load_owner_module(repo, core.REL_SLAVE_FORWARD, "hint_upstream_slave_forward",
                                            add_dir_to_path=False)


def _render_module(repo: Path, module: Any | None = None):
    return module or core.load_owner_module(repo, core.REL_RENDER_DOCKERFILE, "hint_upstream_render_dockerfile",
                                            add_dir_to_path=False)


def _read_env(sf, path: Path | None) -> dict[str, str]:
    """env 파서는 slave_forward 한 벌(스모크 `val()` 의미: 첫 선언 · 첫 `=` 뒤 원문)을 쓴다 — 저장소의 5번째 사본을 만들지
    않는다(build_plane.md §8 "KEY=VALUE env parsers"). 값 양끝 공백만 떼어 선택자 비교에 쓴다."""
    if path is None or not path.is_file():
        return {}
    return {k: str(v).strip() for k, v in sf.read_env_first(path).items()}


# ── 문맥(한 번 관측한 것은 다시 부르지 않는다) ─────────────────────────────────────────────────────
DockerRunner = Callable[..., subprocess.CompletedProcess]


def _flag_value(args: list[str], flag: str) -> str | None:
    """`--flag value` · `--flag=value` 의 값(없으면 None)."""
    for i, tok in enumerate(args):
        if tok == flag and i + 1 < len(args):
            return args[i + 1]
        if tok.startswith(flag + "="):
            return tok.split("=", 1)[1]
    return None


def _probe_only_runner(run: Callable[..., subprocess.CompletedProcess] | None = None) -> DockerRunner:
    """`runner=None` 의 기본 실행기 — 탐침 전용(2026-09-22 · plan_26092119 S2 round 3).

    왜: hint.py 기본 publish 가 탐침을 켜게 되면서(읽기 전용 실행기를 주입할 때만 끈다) 라이브러리 기본값도 같은 의미여야 한다.
    round 2 의 기본은 **제한 없는** `subprocess.run` 이었다 — 탐침은 켰지만 어떤 docker 호출이든 통과시켰다(시작하는 `run` 포함).
    허용: `image inspect` · `history` · `create`(`--pull never` + `--network none` 필수 — 이미지를 받지 않고 시작하지 않는다) ·
    `cp`(이 실행기가 만든 컨테이너에서 꺼내기만) · `rm`(이 실행기가 만든 컨테이너만). 그 밖(`run` · `start` · `exec` · `build` · `pull` ·
    `compose` · 남이 만든 컨테이너의 cp/rm …)은 **부르지 않고** docker 자신의 실패(rc 125)로 돌려준다. `run` = 주입 subprocess(자체검사용)."""
    made: set[str] = set()
    real = run or subprocess.run

    def refuse(argv: list[str], why: str) -> subprocess.CompletedProcess:
        return subprocess.CompletedProcess(list(argv), 125, "", f"hintlib.artifacts 기본 실행기(탐침 전용): {why}")

    def runner(argv: list[str], timeout: int = DOCKER_TIMEOUT_S) -> subprocess.CompletedProcess:
        a = [str(x) for x in argv[1:]]

        def call() -> subprocess.CompletedProcess:
            return real(list(argv), capture_output=True, text=True, timeout=timeout)
        if a[:2] == ["image", "inspect"] or a[:1] == ["history"]:
            return call()
        if a[:1] == ["create"]:
            if _flag_value(a, "--pull") != "never" or _flag_value(a, "--network") != "none":
                return refuse(argv, "create 는 `--pull never --network none` 으로만(이미지를 받지 않고 네트워크 없이 · 시작 ✗)")
            r = call()
            out = (r.stdout if isinstance(r.stdout, str) else "").strip().splitlines()
            if r.returncode == 0 and out and re.fullmatch(r"[0-9a-f]{12,64}", out[-1].strip()):
                made.add(out[-1].strip())
            return r
        if a[:1] == ["cp"]:
            srcs = [x for x in a[1:] if not x.startswith("-")]
            cid = srcs[0].split(":", 1)[0] if srcs and ":" in srcs[0] else ""
            if cid not in made:
                return refuse(argv, "cp 는 이 실행기가 만든(시작하지 않은) 컨테이너에서 꺼내는 것만")
            return call()
        if a[:1] == ["rm"]:
            ids = [x for x in a[1:] if not x.startswith("-")]
            if not ids or any(x not in made for x in ids):
                return refuse(argv, "rm 은 이 실행기가 만든 컨테이너만")
            r = call()
            if r.returncode == 0:
                made.difference_update(ids)
            return r
        return refuse(argv, f"`docker {' '.join(a[:1])}` 거부(컨테이너를 시작하거나 이미지·상태를 바꾸는 호출 ✗)")

    setattr(runner, PROBE_ATTR, True)
    return runner


@dataclass
class _Ctx:
    repo: Path
    ev: Any
    runner: DockerRunner | None
    sf: Any
    rd: Any | None
    rd_loader: Any | None
    topo: str = ""
    cell: str = ""
    node: str = ""
    out: Path = Path(".")
    triplet: dict = field(default_factory=dict)
    env: dict = field(default_factory=dict)
    compose_path: Path | None = None
    # 측정 당시 compose 개정의 바이트를 담은 파일(2026-09-22 S2 round 3). 경로를 받는 소유자 함수(slave_forward.derive ·
    #   compose_default)에 넘긴다 — 작업트리가 측정 뒤 바뀌었으면 페이로드에 쓴 개정 사본이다. None = 작업트리(c.compose_path).
    compose_eff: Path | None = None
    default_runner: bool = False          # runner=None → 이 모듈의 탐침 전용 기본 실행기(탐침 기록 문구만 가른다)
    _docker: dict = field(default_factory=dict)
    # 한 번 관측한 것(history 층 · 레시피 개정 선택 · 이미지 탐침 · 이미지 바이트)은 다시 부르지 않는다. 이미지 바이트는
    #   PAYLOAD 로 직렬화되지 않게 여기에만 둔다(2026-09-22 S2 round 2).
    memo: dict = field(default_factory=dict)

    def render(self):
        if self.rd is None:
            self.rd = _render_module(self.repo, self.rd_loader)
        return self.rd

    def docker(self, *args: str, cache: bool = True) -> tuple[str, subprocess.CompletedProcess | None]:
        """(결과 분류, 프로세스). 분류 ∈ ok · rc=<n> · docker-unavailable · timeout. 같은 인자는 한 번만 부른다 —
        단 `cache=False`(create·cp·rm: 부를 때마다 다른 컨테이너가 생기고 지워진다)는 매번 부른다(2026-09-22 S2 round 2)."""
        if cache and args in self._docker:
            return self._docker[args]
        if self.runner is None:
            # 2026-09-22 S2 round 3 리뷰: 기본 실행기는 **한 번만** 만든다 — 호출마다 새로 만들면 create 로 만든 컨테이너 기록(made)이
            #   사라져 같은 탐침의 cp · rm 이 "남이 만든 컨테이너" 로 거부된다(`_ctx` 밖에서 `_Ctx` 를 직접 만든 경우의 함정).
            self.runner, self.default_runner = _probe_only_runner(), True
        run = self.runner
        try:
            proc = run(["docker", *args], timeout=DOCKER_TIMEOUT_S)
            res = ("ok" if proc.returncode == 0 else f"rc={proc.returncode}", proc)
        except FileNotFoundError:
            res = ("docker-unavailable", None)
        except OSError:
            res = ("docker-unavailable", None)
        except subprocess.TimeoutExpired:
            res = ("timeout", None)
        if cache:
            self._docker[args] = res
        return res


def _ctx(repo: Path, ev: Any, *, runner=None, forward_module=None, render_module=None) -> _Ctx:
    repo = Path(repo).resolve()
    # 2026-09-22 S2 round 3: None = 탐침 전용 기본 실행기(hint.py 기본 publish 와 같은 의미 — 탐침 ON · run/start ✗).
    c = _Ctx(repo=repo, ev=ev, runner=runner if runner is not None else _probe_only_runner(),
             sf=_forward_module(repo, forward_module), rd=None, rd_loader=render_module, default_runner=runner is None)
    c.topo, c.cell = _topology(ev), _cell(ev)
    c.node = str(_get(ev, "node") or "")
    c.out = repo / "output" / c.topo
    supplied = _get(ev, "triplet") or {}
    conv = {"yaml": f"output/{c.topo}/configs/{c.cell}.yaml", "sh": f"output/{c.topo}/configs/{c.cell}.sh",
            "env": f"output/{c.topo}/envs/.env.{c.cell}"}
    for key in ("yaml", "sh", "env"):
        raw = supplied.get(key) if isinstance(supplied, dict) else None
        c.triplet[key] = repo / core.rel(repo, raw or conv[key])     # 저장소 밖 = HINT_PATH_OUTSIDE_REPO
    c.env = _read_env(c.sf, c.triplet["env"])
    for name in ("docker-compose.yaml", "docker-compose.yml"):
        if (c.out / name).is_file():
            c.compose_path = c.out / name
            break
    return c


def _image_ref(c: _Ctx) -> tuple[str | None, str]:
    """이미지 관측(원장 cat · history · Created)에 쓸 docker 참조와 그 출처.

    **내용 권위 = 측정 digest**(evidence.build_identity.image_digest · 2026-09-11 R5 "태그는 이름"). 태그는 가변 포인터라
    측정 뒤 재빌드되면 **다른 이미지**를 가리킨다 — 그 이미지의 history·원장으로 skip 을 재구성하면 이번 측정과 무관한 빌드
    인자로 패치를 빼게 된다(2026-09-22 감사: 옛 판은 태그로 탐침했다). digest 가 있으면 digest 만 쓰고, 그 이미지가 로컬에
    없어도 태그로 **대체하지 않는다**(호출부가 탐침 기록에 남긴다). digest 가 없을 때만 태그를 쓰고 출처에 그 사실을 적는다."""
    build = _get(c.ev, "build_identity") or {}
    digest = str(build.get("image_digest") or "").strip() if isinstance(build, dict) else ""
    if digest:
        return digest, "measured image_digest(evidence.build_identity · 내용 권위)"
    tag, tag_src = _image_tag(c)
    if tag:
        return tag, f"{tag_src}(측정 digest 미기재 — 태그로 관측 · 태그는 가변 포인터)"
    return None, "unobserved(측정 digest·이미지 태그 모두 없음)"


def _image_created(c: _Ctx) -> str | None:
    ref, _ = _image_ref(c)
    if not ref:
        return None
    cls, proc = c.docker("image", "inspect", "--format", "{{.Created}}", ref)
    return _utc_z(proc.stdout.strip()) if cls == "ok" and proc is not None else None


def _recipe_vs_image(c: _Ctx, rels: list[str], created: str | None, selected: dict | None = None) -> list[dict]:
    """실을 레시피 파일이 **이미지를 지은 그 바이트인지** 관측 가능한 만큼 적는다(원장이 없는 옛 이미지의 X9 결).
    추적 파일은 git 이 바이트를 든다(GIT_SINGLE_AUTHORITY) — HEAD blob 과 작업트리 대조(수정본?) + 마지막 커밋 시각이 이미지
    Created 뒤인가(빌드 뒤 바뀐 레시피?). 읽기 전용 배관만 쓴다(인덱스를 건드리는 porcelain ✗).

    `selected`(경로 → `_select_revision` 결과 · 2026-09-22 S2 round 2): 실린 바이트가 이미지와 **대조로 확인**됐으면(history 전수
    일치 · 이미지 안 COPY 대상 바이트 일치) 작업트리 경고를 싣지 않고 무엇이 실렸는지만 적는다 — 경고는 "실린 바이트가 이미지를
    지은 바이트라는 보장이 없다" 일 때만 뜻이 있다(round 1: 작업트리 수정본을 싣고 경고만 달았다)."""
    import hashlib
    out: list[dict] = []
    if not (c.repo / ".git").exists():
        return out
    selected = selected or {}
    for rel in rels:
        data = (c.repo / rel).read_bytes()
        blob = hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()
        ls = core.git(c.repo, "-c", "core.quotePath=false", "ls-tree", "HEAD", "--", rel, check=False)
        parts = ls.stdout.split() if ls.returncode == 0 else []
        head_blob = parts[2] if len(parts) >= 3 and parts[1] == "blob" else None
        row: dict = {"path": rel, "tracked": head_blob is not None,
                     "worktree_equals_head": (head_blob == blob) if head_blob else None}
        if head_blob:
            lg = core.git(c.repo, "-c", "log.showSignature=false", "log", "-1", "--format=%ct", "--", rel, check=False)
            ts = lg.stdout.strip() if lg.returncode == 0 else ""
            if ts.isdigit():
                import datetime as _dt
                row["last_commit_utc"] = _dt.datetime.fromtimestamp(int(ts), _dt.timezone.utc).strftime(
                    "%Y-%m-%dT%H:%M:%SZ")
        if created:
            row["image_created_utc"] = created
            late = row.get("last_commit_utc") and row["last_commit_utc"] > created
            if row["worktree_equals_head"] is False or late:
                row["warning"] = ("이미지 빌드 뒤 바뀐 레시피일 수 있다 — " +
                                  ("작업트리 수정본(미커밋) " if row["worktree_equals_head"] is False else "") +
                                  ("마지막 커밋이 이미지 Created 뒤" if late else "")).strip()
        s = selected.get(rel)
        if isinstance(s, dict):
            row["shipped"] = s.get("shipped")
            row["selection_method"] = s.get("method")
            if s.get("history_match") is not None:
                row["history_match"] = s["history_match"]
            hm = s.get("history_match")
            if s.get("verified"):
                # 실린 바이트가 이미지와 대조로 확인됐다 — 작업트리가 수정본이어도 그 경고는 실린 파일의 결함이 아니다.
                was = row.pop("warning", None)
                if was:
                    row["note"] = (f"작업트리 경고({was})는 실린 파일에 해당하지 않는다 — 실린 바이트 = {s.get('shipped')} · "
                                   f"{s.get('method')}")
            elif isinstance(hm, dict) and hm.get("matched", 0) < hm.get("total", 0):
                # 대조했는데 전수 일치가 아니다 — 작업트리 수정 여부와 무관하게 실린 바이트가 이미지를 지은 바이트라는 보장이 없다.
                prev = row.get("warning")
                row["warning"] = (f"실린 바이트({s.get('shipped')})가 측정 이미지 docker history 의 RUN·COPY 와 전수 일치하지 않는다"
                                  f"({hm['matched']}/{hm['total']})" + (f" · {prev}" if prev else ""))
            elif s.get("image_bytes_differ"):
                prev = row.get("warning")
                row["warning"] = ("이미지 안 COPY 대상 바이트가 실린 작업트리 바이트와 다르다(일치하는 개정 없음)"
                                  + (f" · {prev}" if prev else ""))
        out.append(row)
    return out


# ── 이미지를 지은 레시피 개정 선택 (F1 · 2026-09-22 plan_26092119 S2 round 2) ─────────────────────────────────────────
def _ws(text: str) -> str:
    return " ".join(text.split())


def _dockerfile_steps(text: str) -> list[str]:
    """최종 스테이지(마지막 FROM 뒤)의 RUN·COPY·ADD 를 **BuildKit history 모양으로** 정규화한 목록.
    이어짐(`\\`)은 붙이고 그 사이의 주석·빈 줄은 버린다(Dockerfile 파서와 같은 규칙) · heredoc 본문은 원문대로 붙인다(history 도
    `<<'PY'\\n…\\nPY` 를 싣는다) · RUN 의 `--mount=`·`--network=`·`--security=` 는 history 에 남지 않으므로 뗀다 · 공백은 접는다."""
    lines = text.splitlines()
    steps: list[str] = []
    i = 0
    while i < len(lines):
        raw = lines[i]
        i += 1
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        buf = raw.rstrip()
        while buf.endswith("\\") and i < len(lines):
            buf = buf[:-1]
            while i < len(lines) and (not lines[i].strip() or lines[i].lstrip().startswith("#")):
                i += 1
            if i < len(lines):
                buf += lines[i].rstrip()
                i += 1
        m = re.match(r"^\s*([A-Za-z]+)\s+(.*)$", buf)
        if not m:
            continue
        instr, body = m.group(1).upper(), m.group(2)
        if instr == "FROM":
            steps = []                     # 다단계 빌드 — 이미지 history 에 남는 것은 마지막 스테이지뿐
            continue
        if instr not in ("RUN", "COPY", "ADD"):
            continue
        docs = []
        for hm in _HEREDOC.finditer(body):
            dash, delim = hm.group(1) == "-", hm.group(3)
            chunk = []
            while i < len(lines):
                ln = lines[i]
                i += 1
                if (ln.strip() if dash else ln.rstrip()) == delim:
                    break
                chunk.append(ln.lstrip("\t") if dash else ln)
            docs.append("\n".join(chunk) + "\n" + delim)
        if docs:
            body = body + "\n" + "\n".join(docs)
        if instr == "RUN":
            fm = _RUN_FLAG.match(body)
            while fm:
                body = body[fm.end():]
                fm = _RUN_FLAG.match(body)
            if body.lstrip().startswith("["):
                try:
                    body = " ".join(str(t) for t in json.loads(body))
                except ValueError:
                    pass                   # exec 꼴이 아니면 원문 그대로 비교한다(일치하지 않을 뿐 — 가려지지 않는다)
        steps.append(f"{instr} {_ws(body)}")
    return steps


def _history_step(created_by: str) -> str | None:
    """history 한 층의 CreatedBy → `_dockerfile_steps` 와 같은 모양(RUN·COPY·ADD 만 · 그 밖 = None).
    `RUN |N K=V …` 의 빌드 인자 N 개와 셸 접두(`/bin/bash -c`)·BuildKit 꼬리(` # buildkit`)를 뗀다."""
    s = re.sub(r"\s+# buildkit\s*$", "", created_by.strip())
    head, _, rest = s.partition(" ")
    if head == "RUN":
        m = re.match(r"^\|(\d+)\s", rest)
        if m:
            rest = " ".join(rest[m.end():].split(" ")[int(m.group(1)):])
        return f"RUN {_ws(_SHELL_C.sub('', rest.lstrip(), count=1))}"
    if head in ("COPY", "ADD"):
        return f"{head} {_ws(rest)}"
    return None


def _history_layers(c: _Ctx) -> tuple[list[dict] | None, str]:
    """측정 digest 의 층(최신 → 오래된) `{created_by, created_utc}`. `docker history --no-trunc --format '{{json .}}'` —
    heredoc 층은 여러 줄이라 `{{.CreatedBy}}` 줄 나눔으로는 층 경계를 못 가른다. 형식 판독 불가 = None(소리 나게 · 다른 형식으로
    대체하지 않는다). `CreatedSince` 는 벽시계 상대값이라 쓰지 않는다."""
    if "layers" in c.memo:
        return c.memo["layers"]
    image, image_src = _image_ref(c)
    res: tuple[list[dict] | None, str] = (None, f"unobserved({image_src})")
    if image:
        cls, proc = c.docker("history", "--no-trunc", "--format", "{{json .}}", image)
        if cls != "ok" or proc is None:
            res = (None, f"unobserved(docker history {cls})")
        else:
            rows: list[dict] | None = []
            for ln in proc.stdout.splitlines():
                if not ln.strip():
                    continue
                try:
                    d = json.loads(ln)
                except ValueError:
                    rows = None
                    break
                if not isinstance(d, dict) or not isinstance(d.get("CreatedBy"), str):
                    rows = None
                    break
                rows.append({"created_by": d["CreatedBy"], "created_utc": _utc_z(d.get("CreatedAt"))})
            res = ((rows, f"docker history --no-trunc --format '{{{{json .}}}}' {image}") if rows
                   else (None, "unobserved(docker history JSON 판독 불가 — 층 경계를 가르지 못한다)"))
    c.memo["layers"] = res
    return res


def _history_match(steps: list[str], window: list[str]) -> dict:
    """중복 허용 대조: matched = 레시피 단계 중 이미지 층에 있는 수 · total = 레시피 단계 + 레시피에 없는 이미지 층(합집합 크기)."""
    pool: dict[str, int] = {}
    for w in window:
        pool[w] = pool.get(w, 0) + 1
    matched = 0
    for s in steps:
        if pool.get(s):
            pool[s] -= 1
            matched += 1
    return {"matched": matched, "total": len(steps) + sum(pool.values())}


def _git_rev_before(c: _Ctx, rel: str, created: str | None) -> str | None:
    """이미지 Created 이전의 마지막 커밋(`git log -1 --before=<Created> --format=%H -- <path>` · 읽기 전용)."""
    if not created or not (c.repo / ".git").exists():
        return None
    lg = core.git(c.repo, "-c", "log.showSignature=false", "log", "-1", f"--before={created}", "--format=%H", "--", rel,
                  check=False)
    sha = lg.stdout.strip() if lg.returncode == 0 else ""
    return sha if re.fullmatch(r"[0-9a-f]{40}", sha) else None


def _better(a: dict, b: dict) -> bool:
    """a 의 일치율이 b 보다 엄격히 높은가(matched/total · 분모 0 = 0)."""
    return a["total"] > 0 and (b["total"] == 0 or a["matched"] * b["total"] > b["matched"] * a["total"])


def _select_dockerfile(c: _Ctx, dockerfile: str) -> dict:
    """쓰인 Dockerfile 의 **이미지를 지은 개정**(F1). 작업트리가 측정 이미지의 history(RUN·COPY 명령 원문)와 전수 일치하면 작업트리,
    아니면 이미지 Created 이전 마지막 커밋을 후보로 대조해 **더 잘 맞을 때만** 그 개정의 바이트를 싣는다. 어느 쪽도 더 낫지 않으면
    작업트리를 싣고 경고를 남긴다(recipe_vs_image). history 미관측이면 대조하지 않는다(작업트리 · 대조 불가 기재).
    반환 {path, text, bytes, mode, shipped, commit, method, history_match, candidates, verified}."""
    key = ("dockerfile", dockerfile)
    if key in c.memo:
        return c.memo[key]
    path = c.out / dockerfile
    wt = path.read_bytes()
    rel = core.rel(c.repo, path)
    out = {"path": rel, "bytes": wt, "mode": path.stat().st_mode & 0o777, "shipped": "worktree", "commit": None,
           "history_match": None, "candidates": [], "verified": False}
    layers, lsrc = _history_layers(c)
    created = _image_created(c)
    if layers is None:
        out["method"] = f"worktree(이미지 history 미관측 — 개정 대조 불가 · {lsrc})"
    else:
        cands = [("worktree", wt)]
        rev = _git_rev_before(c, rel, created)
        rev_bytes = core.git_bytes(c.repo, "show", f"{rev}:{rel}", check=False) if rev else None
        if rev and rev_bytes is not None and rev_bytes != wt:
            cands.append((rev, rev_bytes))
        steps = {name: _dockerfile_steps(b.decode("utf-8", "replace")) for name, b in cands}
        union = set().union(*steps.values())
        hist = [_history_step(x["created_by"]) for x in layers]
        # 우리 Dockerfile 의 층 = 최신 층부터 **어느 후보든 일치하는 가장 깊은 층**까지(그 아래는 베이스 이미지 층).
        deepest = max((i for i, h in enumerate(hist) if h and h in union), default=-1)
        window = [h for h in hist[:deepest + 1] if h]
        own = sorted(x["created_utc"] for x in layers[:deepest + 1] if x.get("created_utc"))
        if own:
            out["layer_span"] = {"oldest_utc": own[0], "newest_utc": own[-1], "layers": deepest + 1, "source": lsrc,
                                 "note": "우리 Dockerfile 층의 CreatedAt 범위 — 캐시 재사용 층은 이전 빌드 시각을 가진다(소요 아님)"}
        scores = {name: _history_match(st, window) for name, st in steps.items()}
        out["candidates"] = [{"revision": name, **scores[name]} for name, _ in cands]
        w = scores["worktree"]
        best = cands[1][0] if len(cands) > 1 and _better(scores[cands[1][0]], w) else None
        if w["total"] and w["matched"] == w["total"]:
            out.update(method="worktree(측정 이미지 docker history 의 RUN·COPY 와 전수 일치)", history_match=w, verified=True)
        elif best:
            b = dict(cands)[best]
            sb = scores[best]
            out.update(bytes=b, shipped=f"git:{best}", commit=best, history_match=sb,
                       verified=sb["matched"] == sb["total"],
                       method=(f"git log -1 --before={created} -- {rel} · docker history 대조(개정 {sb['matched']}/{sb['total']} "
                               f"> 작업트리 {w['matched']}/{w['total']}) — 이 개정의 바이트를 싣는다(git show)"))
        else:
            out.update(history_match=w,
                       method=(f"worktree(어느 개정도 이미지 history 와 더 맞지 않는다 · 작업트리 {w['matched']}/{w['total']}"
                               + ("" if rev else (" · 대조할 개정 없음(" + ("이미지 Created 미관측" if not created else
                                                                    "이미지 Created 이전 커밋 부재 · 비추적") + ")"))
                               + " — recipe_vs_image 경고 유지)"))
    out["text"] = out["bytes"].decode("utf-8", "replace")
    c.memo[key] = out
    return out


def _copy_dests(text: str) -> dict[str, str]:
    """Dockerfile COPY·ADD 원천(컨텍스트 상대 · 끝 `/` 제거) → 목적지 원문. 상대 목적지(WORKDIR 기준)·원천이 여럿인 줄·`--from`·
    원격 원천은 대응시키지 않는다(모르는 것은 모른다). 이미지 안 경로 해소는 원천의 종류를 아는 호출부가 `_dest_path` 로 한다."""
    out: dict[str, str] = {}
    for m in _DOCKERFILE_COPY.finditer(text):
        body = m.group(2).split("#", 1)[0].strip()
        if body.startswith("["):
            continue
        toks = body.split()
        if any(t.startswith("--from") for t in toks):
            continue
        toks = [t for t in toks if not t.startswith("--")]
        if len(toks) != 2 or not toks[1].startswith("/"):
            continue
        src = toks[0].rstrip("/")
        if src and "://" not in src and not src.startswith("git@"):
            out.setdefault(src, toks[1])
    return out


def _dest_path(src: str, dst: str, src_is_dir: bool) -> str:
    """COPY 의미: 디렉터리 원천은 **내용**이 목적지로 간다(목적지 = 그 디렉터리) · 파일 원천은 목적지가 `/` 로 끝나면 그 아래 원천 이름."""
    if src_is_dir:
        return dst.rstrip("/") or "/"
    return (dst.rstrip("/") + "/" + PurePath(src).name) if dst.endswith("/") else dst


def _select_copied(c: _Ctx, src_rel: str, path: Path, baked: bytes | None, probe_note: str) -> dict:
    """Dockerfile 이 COPY 하는 추적 파일(requirements 등)의 개정 선택 — 명령 원문이 아니라 **바이트**가 증거다: 이미지 안 COPY
    대상 바이트(이미지 탐침)와 작업트리가 같으면 작업트리, 다르면 이미지 Created 이전 개정이 같을 때만 그 개정을 싣는다."""
    wt = path.read_bytes()
    rel = core.rel(c.repo, path)
    out = {"path": rel, "bytes": wt, "mode": path.stat().st_mode & 0o777, "shipped": "worktree", "commit": None,
           "history_match": None, "verified": False}
    if baked is None:
        out["method"] = f"worktree(이미지 안 COPY 대상 바이트 미관측 — {probe_note})"
        return out
    if baked == wt:
        out.update(method="worktree(image-probe: 이미지 안 COPY 대상 바이트와 sha256 일치)", verified=True)
        return out
    created = _image_created(c)
    rev = _git_rev_before(c, rel, created)
    rev_bytes = core.git_bytes(c.repo, "show", f"{rev}:{rel}", check=False) if rev else None
    if rev_bytes is not None and rev_bytes == baked:
        out.update(bytes=rev_bytes, shipped=f"git:{rev}", commit=rev, verified=True,
                   method=f"git log -1 --before={created} -- {rel} · image-probe 바이트 일치(작업트리는 다르다)")
    else:
        out["method"] = ("worktree(이미지 안 바이트와 다르고 이미지 Created 이전 개정도 같지 않다 — recipe_vs_image 경고 · "
                         f"이미지 sha256 {core.sha256_bytes(baked)[:16]})")
        out["image_bytes_differ"] = True
    return out


# ── 측정 시각 · 측정 뒤 재생성 (F2 · 2026-09-22 S2 round 2) ───────────────────────────────────────────────────────────
def _measured_utc(c: _Ctx) -> tuple[str | None, str]:
    """측정 시각 = 인증서 `measured_utc` → 스윕 `generated_utc`(evidence 가 둘이 같을 때만 스윕을 묶는다). 둘 다 없으면 미관측."""
    cert = _get(c.ev, "certificate")
    got = _utc_z(cert.get("measured_utc")) if isinstance(cert, dict) else None
    if got:
        return got, "certificate.measured_utc"
    sweep = _get(c.ev, "sweep") or {}
    got = _utc_z(sweep.get("generated_utc")) if isinstance(sweep, dict) else None
    if got:
        return got, "sweep_index.generated_utc"
    return None, "unobserved(인증서 measured_utc · 스윕 generated_utc 없음)"


def _mtime_utc(path: Path) -> str | None:
    import datetime as _dt
    try:
        return _dt.datetime.fromtimestamp(path.stat().st_mtime, _dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    except (OSError, OverflowError, ValueError):
        return None


def _regenerated(c: _Ctx, path: Path) -> dict | None:
    """파일 mtime(관측)이 측정 시각보다 뒤면 {mtime_utc, measured_utc, measured_source} — 그 파일의 값은 측정 당시 값이 아닐 수
    있다. 측정 시각 미관측이면 None(판단 불가는 호출부가 헤더에 적는다). mtime 은 파일 속성 관측이지 벽시계가 아니다."""
    measured, msrc = _measured_utc(c)
    mt = _mtime_utc(path)
    if measured and mt and mt > measured:
        return {"mtime_utc": mt, "measured_utc": measured, "measured_source": msrc}
    return None


# ── compose 슬롯 추적 파일의 측정 당시 개정 (2026-09-22 · plan_26092119 S2 round 3) ─────────────────────────────────────
# 왜: round 2 는 빌드 레시피(Dockerfile)만 개정을 골랐고 compose 는 작업트리를 그대로 실었다 — 태그2 의 docker-compose.yaml 은 측정
#   (09-12) 뒤 09-22 에 주석이 고쳐져 "NCCL env 19키(①7+②8+③4)" 라 말했는데, 측정 당시 렌더러는 17키였다(기계 채점 X 항목).
#   docker history 같은 이미지 측 증거가 compose 에는 없으므로 기준은 **측정 시각**이다: 작업트리 mtime(관측 속성) ≤ 측정이면 그 바이트가
#   측정 당시 바이트이고, 뒤면 측정 전 마지막 커밋을 싣는다. 교차대조(측정 뒤 커밋 · HEAD 대비 수정본)는 method·cross_check 에 적는다.
_TRACKED_KEYS = ("path", "shipped", "commit", "method", "basis", "verified", "measured_utc", "mtime_utc",
                 "commits_after_measurement", "cross_check", "history_match", "warning")


def _select_tracked(c: _Ctx, path: Path) -> dict:
    """compose 슬롯 추적 파일(docker-compose.yaml · 러너 정본)의 **측정 당시** 개정. 반환 {path, bytes, mode, shipped(worktree|git:<sha>),
    commit, method, basis(mtime≤measured|git-rev-before-measurement|unobserved), verified(측정 당시 바이트임을 관측으로 확인했나),
    measured_utc, mtime_utc, commits_after_measurement[], cross_check[], history_match(None — compose 에는 이미지 history 대조가 없다),
    warning?}. 읽기 전용 배관만 쓴다(git log · ls-tree · show)."""
    import hashlib
    rel = _repo_rel(c, path)
    key = ("tracked", rel or str(path))
    if key in c.memo:
        return c.memo[key]
    wt = path.read_bytes()
    measured, msrc = _measured_utc(c)
    mt = _mtime_utc(path)
    # 저장소 밖 파일(자체검사 픽스처의 소유 모듈 심링크가 가리키는 asset 등)은 경로 원문을 싣지 않는다(운영자 경로 모양 · PII).
    out: dict = {"path": rel or f"<저장소 밖>/{path.name}", "bytes": wt, "mode": path.stat().st_mode & 0o777,
                 "shipped": "worktree", "commit": None,
                 "basis": "unobserved", "verified": False, "measured_utc": measured, "mtime_utc": mt,
                 "commits_after_measurement": [], "cross_check": [], "history_match": None}
    newer = measured is not None and (mt is None or mt > measured)
    if measured is None:
        out["method"] = f"worktree(측정 시각 미관측 — 측정 당시 개정 대조 불가 · {msrc})"
    elif rel is None or not (c.repo / ".git").exists():
        why = "저장소 밖 파일" if rel is None else "git 저장소 아님"
        if newer:
            out["method"] = f"worktree({why} — 측정 당시 개정 대조 불가 · mtime {mt} > 측정 {measured}({msrc}))"
            out["warning"] = (f"측정({measured}) 뒤 mtime {mt} — 측정 전 개정을 찾을 수 없다({why}) · 실린 작업트리 바이트가 측정 당시 "
                              "바이트라는 보장 없음")
        else:
            out.update(basis="mtime≤measured", verified=True,
                       method=f"worktree(mtime {mt} ≤ 측정 {measured}({msrc}) — 측정 당시 바이트 · {why})")
    else:
        rev = _git_rev_before(c, rel, measured)
        rev_bytes = core.git_bytes(c.repo, "show", f"{rev}:{rel}", check=False) if rev else None
        lg = core.git(c.repo, "-c", "log.showSignature=false", "log", f"--after={measured}", "--format=%H", "--", rel,
                      check=False)
        after = [s for s in (lg.stdout.split() if lg.returncode == 0 else []) if re.fullmatch(r"[0-9a-f]{40}", s)]
        ls = core.git(c.repo, "-c", "core.quotePath=false", "ls-tree", "HEAD", "--", rel, check=False)
        parts = ls.stdout.split() if ls.returncode == 0 else []
        head_blob = parts[2] if len(parts) >= 3 and parts[1] == "blob" else None
        wt_blob = hashlib.sha1(b"blob %d\0" % len(wt) + wt).hexdigest()
        cross = [f"측정 뒤 커밋 {len(after)}" + (f"({', '.join(s[:12] for s in after[:5])})" if after else ""),
                 "작업트리 = HEAD" if head_blob == wt_blob else
                 ("작업트리 수정본(HEAD 대비 미커밋)" if head_blob else "HEAD 에 없음(비추적)")]
        out.update(commits_after_measurement=[s[:12] for s in after], cross_check=cross)
        head = f"git log -1 --before={measured} -- {rel}"
        if not newer:
            out.update(basis="mtime≤measured", verified=True)
            if rev and rev_bytes == wt:
                out.update(commit=rev, method=f"worktree(mtime {mt} ≤ 측정 {measured}({msrc}) · 바이트 = 측정 전 마지막 커밋 {rev[:12]})")
            elif rev:
                out["method"] = (f"worktree(mtime {mt} ≤ 측정 {measured}({msrc}) — 측정 당시 작업트리 바이트 · 측정 전 마지막 커밋 "
                                 f"{rev[:12]} 와 다르다: 측정에 미커밋 수정이 쓰였다)")
            else:
                out["method"] = f"worktree(mtime {mt} ≤ 측정 {measured}({msrc}) · 측정 전 커밋 없음)"
        elif rev and rev_bytes is not None:
            if rev_bytes == wt:
                out.update(commit=rev, basis="git-rev-before-measurement",
                           method=(f"worktree({head} → {rev[:12]} · 작업트리 mtime {mt} > 측정({msrc})이지만 바이트가 그 개정과 "
                                   f"같다 · {' · '.join(cross)})"))
            else:
                out.update(bytes=rev_bytes, shipped=f"git:{rev}", commit=rev, basis="git-rev-before-measurement",
                           method=(f"{head} → {rev[:12]} · 작업트리 mtime {mt} > 측정 {measured}({msrc}) · {' · '.join(cross)} — "
                                   "측정 전 마지막 커밋의 바이트를 싣는다(git show · 측정 당시 미커밋 수정이 있었는지는 관측 불가)"))
        else:
            out["method"] = (f"worktree({head} 없음(측정 전 커밋 없음 · 비추적 또는 측정 뒤 첫 커밋) · mtime {mt} > 측정 "
                             f"{measured}({msrc}) · {' · '.join(cross)})")
            out["warning"] = (f"측정({measured}) 뒤 mtime {mt} · 측정 전 개정 없음 — 실린 작업트리 바이트가 측정 당시 바이트라는 "
                              "보장 없음")
    c.memo[key] = out
    return out


def _repo_rel(c: _Ctx, path: Path) -> str | None:
    """저장소 상대 posix 경로 — 심링크를 풀지 않은 경로를 먼저 본다(git 이 드는 것은 저장소 안의 그 경로다). 밖이면 None."""
    p = Path(path)
    for cand in (p if p.is_absolute() else c.repo / p, (p if p.is_absolute() else c.repo / p).resolve()):
        try:
            return cand.relative_to(c.repo).as_posix()
        except ValueError:
            continue
    return None


def _tracked_row(sel: dict) -> dict:
    """선택 결과의 PAYLOAD 행(바이트 ✗)."""
    return {k: sel[k] for k in _TRACKED_KEYS if k in sel}


def _compose_text(c: _Ctx) -> str:
    """서빙 경로 파생에 쓰는 compose 텍스트 = 측정 당시 개정(작업트리가 측정 뒤 바뀌었으면 측정 전 마지막 커밋)."""
    if c.compose_path is None:
        return ""
    return _select_tracked(c, c.compose_path)["bytes"].decode("utf-8", "replace")


def _compose_file(c: _Ctx) -> Path | None:
    """경로를 받는 소유자 함수에 넘길 compose 파일 — 측정 당시 개정 사본이 준비돼 있으면 그것(`compose_eff`)."""
    return c.compose_eff or c.compose_path


# 렌더러 개정(2026-09-22 · plan_26092119 S2 round 3 적대 리뷰): 재현 절차 render 단계는 **작업트리** render_dockerfile.py 의 CLI 를
#   싣는다. 2차 바이트 채점 X 항목(01:384) — 태그2 는 측정(09-12) 뒤 렌더러가 10커밋 바뀌어(09-17 소켓 기준선 782fd70 등) 그 명령을 지금
#   돌리면 측정 당시(936fd06 · NET/IB RoCE)와 다른 env 형상이 나온다. 측정 뒤 재생성된 형상을 싣지 않기로 한 규칙(F2)과 같은 사실을
#   render 명령 자리에서도 말한다 — 판정은 git 관측(측정 전 마지막 커밋 바이트 ≠ 작업트리)으로만 한다(추측으로 경고하지 않는다).
def _renderer_revision(c: _Ctx) -> dict | None:
    """render_dockerfile.py 의 측정 당시 개정(`_select_tracked` 와 같은 규칙 · 바이트는 싣지 않는다). 파일 부재 = None."""
    p = c.repo / core.REL_RENDER_DOCKERFILE
    return _select_tracked(c, p) if p.is_file() else None


def _renderer_drift_lines(rsel: dict | None) -> list[str]:
    """렌더러가 측정 뒤 바뀌었다는 **관측**(측정 전 마지막 커밋의 바이트 ≠ 작업트리 = `shipped` 가 git 개정)이 있을 때만 render 명령
    앞에 붙일 셸 주석 줄. 관측이 없거나(바이트 같음 · mtime ≤ 측정) 대조할 수 없으면(측정 시각 미관측 · 개정 없음) 빈 목록 — 그 사실은
    단계 행의 `renderer_revision` 이 말한다(없는 드리프트를 지어내지 않는다)."""
    if not rsel or not str(rsel.get("shipped") or "").startswith("git:"):
        return []
    rev12 = str(rsel["commit"])[:12]
    after = rsel.get("commits_after_measurement") or []
    return [f"# ⚠ 측정 당시 렌더러 ≠ 작업트리 렌더러: {rsel['path']} 측정 전 마지막 커밋 {rev12} · 측정 뒤 커밋 {len(after)}건"
            + (f"({', '.join(after[:5])}{' …' if len(after) > 5 else ''})" if after else "")
            + " — 아래 명령을 지금 돌리면 측정 당시와 다른 env 형상·러너가 나올 수 있다",
            f"# 측정 당시 env 값 = {ENV_OBSERVED_POINTER}(측정 엔진 로그 관측) · 측정 당시 렌더러 원문 = git show {rev12}:{rsel['path']}"]


# ── 평면 · 빌드 선택자 ───────────────────────────────────────────────────────────────────────
def _plane(c: _Ctx) -> tuple[str, str]:
    declared = str(_get(c.ev, "plane") or "").strip().lower()
    declared = {"container": "docker"}.get(declared, declared)
    build = _get(c.ev, "build_identity") or {}
    observed = source = None
    if c.env.get("IMAGE_TAG") or c.env.get("BUILD_DOCKERFILE"):
        observed, source = "docker", "cell-env(IMAGE_TAG|BUILD_DOCKERFILE)"
    elif isinstance(build, dict) and (build.get("image_tag") or build.get("dockerfile")):
        observed, source = "docker", "evidence.build_identity(image_tag|dockerfile)"
    if declared and declared not in ("docker", "native"):
        core.fail("HINT_PLANE_UNKNOWN", f"평면 어휘 밖: {declared!r}", "docker|native(container=docker) 중 하나.")
    if observed and declared and observed != declared:
        core.fail("HINT_PLANE_MISMATCH", f"선언 plane={declared} 와 관측 plane={observed}({source}) 가 다르다.",
                  "셀 env 의 이미지 선택자와 evidence 의 plane 중 어느 쪽이 틀렸는지 확인한다.")
    if observed:
        return observed, source
    if declared:
        return declared, "evidence.plane(declared)"
    core.fail("HINT_PLANE_UNDERIVABLE",
              "실행 평면을 파생할 신호가 없다(셀 env 에 IMAGE_TAG·BUILD_DOCKERFILE 없음 · evidence.plane 없음).",
              "docker 셀이면 셀 env 에 이미지 선택자(IMAGE_TAG·BUILD_DOCKERFILE)를 둔다. native 셀은 **현재 발행할 수 없다** — "
              "native 평면 신호를 내는 producer 가 아직 없다(알려진 한계 · 2026-09-22 통합 결정 D-d: serve phase 가 평면을 관측해 "
              "기록하는 배선이 서면 evidence 가 `plane` 으로 넘긴다). '신호 없음 = native' 로 추측하지 않는다.")


def plane_of(repo: Path, ev: Any, *, forward_module: Any | None = None) -> str:
    """실행 평면 `docker|native`(SPEC §5.6). 셀 env 선택자 · evidence 관측 · 선언의 일치를 요구한다."""
    return plane_and_source(repo, ev, forward_module=forward_module)[0]


def plane_and_source(repo: Path, ev: Any, *, forward_module: Any | None = None) -> tuple[str, str]:
    """plane_of 의 (평면, 판정 출처) — 이름 평면 토큰(O-N1 · 2026-09-23)의 출처 기록용. 판정은 `_plane` 한 벌이다."""
    return _plane(_ctx(repo, ev, forward_module=forward_module))


def _dockerfile_args(c: _Ctx, text: str) -> list[str]:
    """쓰인 Dockerfile 자신의 `^ARG` 이름 — 소유자 `render_dockerfile.ledger_build_args`(원장 build-arg 목록의 단일 소유자 ·
    evidence._dockerfile_args 와 같은 원천). 정규식 사본을 두지 않는다(옛 판은 여기서 따로 적었다 — 두 목록이 갈라진다)."""
    fn = getattr(c.render(), "ledger_build_args", None)
    if not callable(fn):
        core.fail("HINT_OWNER_MODULE_MISSING", "render_dockerfile.ledger_build_args 부재(원장 build-arg 목록의 소유자).",
                  "upstream-version-watch render_dockerfile.py 의 공개 ledger_build_args(text) 를 확인한다(SPEC §6.2).")
    return sorted(set(fn(text)))


def _dockerfile_copies(text: str) -> list[str]:
    """쓰인 Dockerfile 의 COPY·ADD 원천(빌드 컨텍스트 상대 · 끝 `/` 제거). `--from=` 스테이지 복사와 ADD 의 원격 원천(URL·git)은
    컨텍스트가 아니다. ADD 도 컨텍스트 파일을 싣는다 — COPY 만 보면 ADD 로 들어간 입력이 레시피에서 조용히 빠진다."""
    out: list[str] = []
    for m in _DOCKERFILE_COPY.finditer(text):
        is_add = m.group(1).upper() == "ADD"
        body = m.group(2).split("#", 1)[0].strip()
        if body.startswith("["):
            try:
                toks = [str(t) for t in json.loads(body)]
            except ValueError:
                core.fail("HINT_DOCKERFILE_COPY_UNPARSED", f"COPY JSON 형식을 읽지 못했다: {body[:80]}")
        else:
            toks = body.split()
        if any(t.startswith("--from") for t in toks):
            continue
        toks = [t for t in toks if not t.startswith("--")]
        for src in toks[:-1]:
            if is_add and ("://" in src or src.startswith("git@")):
                continue
            s = src.rstrip("/")
            if s and s not in out:
                out.append(s)
    return out


def _image_tag(c: _Ctx) -> tuple[str | None, str]:
    """이미지 태그와 출처 — 셀 env → evidence.build_identity → compose 기본값. 원장 탐침과 선택자가 **같은 규칙**을 쓴다."""
    if c.env.get("IMAGE_TAG"):
        return c.env["IMAGE_TAG"], "cell-env:IMAGE_TAG"
    build = _get(c.ev, "build_identity") or {}
    if isinstance(build, dict) and build.get("image_tag"):
        return str(build["image_tag"]), "evidence.build_identity.image_tag"
    if c.compose_path:
        dflt = c.sf.compose_default(c.compose_path, "IMAGE_TAG")
        if dflt:
            return dflt, "compose-default:IMAGE_TAG"
    return None, "unobserved"


def _build_selection(c: _Ctx, ledger: dict | None) -> dict:
    """쓰인 Dockerfile · 트랙 · 이미지 태그와 각각의 출처. 원장 ↔ 셀 env 가 다르면 **양성 모순**이라 차단한다
    (같은 이름의 이미지가 다른 레시피로 지어졌다 — plan_26081410 "배선이 다른 이미지가 같은 이름" 결함 클래스)."""
    sel_ledger = str(ledger.get("dockerfile") or "") if isinstance(ledger, dict) else ""
    sel_env = c.env.get("BUILD_DOCKERFILE", "")
    compose_df = c.sf.compose_default(c.compose_path, "BUILD_DOCKERFILE") if c.compose_path else None
    if sel_ledger and sel_env and sel_ledger != sel_env:
        core.fail("HINT_BUILD_SELECTOR_MISMATCH",
                  f"이미지 원장 dockerfile={sel_ledger!r} 와 셀 env BUILD_DOCKERFILE={sel_env!r} 가 다르다.",
                  "셀 env 가 가리키는 레시피로 이미지를 다시 짓거나 셀 env 를 원장에 맞춘다(태그는 가변 포인터).")
    for val, src in ((sel_ledger, "build-ledger.dockerfile"), (sel_env, "cell-env:BUILD_DOCKERFILE"),
                     (compose_df or "", "compose-default:BUILD_DOCKERFILE")):
        if val:
            dockerfile, df_source = val, src
            break
    else:
        core.fail("HINT_BUILD_SELECTOR_UNDERIVABLE", "쓰인 Dockerfile 을 파생하지 못했다(원장·셀 env·compose 기본값 모두 없음).",
                  "셀 env 에 BUILD_DOCKERFILE 을 적거나 compose 의 `dockerfile: ${BUILD_DOCKERFILE:-…}` 기본값을 확인한다.")
    if "/" in dockerfile or dockerfile.startswith("."):
        core.fail("HINT_BUILD_SELECTOR_INVALID", f"Dockerfile 선택자는 빌드 컨텍스트 안 파일 이름이어야 한다: {dockerfile!r}")
    image, image_source = _image_tag(c)
    track = "wheel" if dockerfile == "Dockerfile" else "source-build"
    if isinstance(ledger, dict) and ledger.get("track") and ledger["track"] != track:
        core.fail("HINT_BUILD_TRACK_MISMATCH", f"원장 track={ledger['track']!r} 와 Dockerfile 파생 track={track!r} 가 다르다.")
    return {"dockerfile": dockerfile, "dockerfile_source": df_source, "track": track,
            "image_tag": image or None, "image_tag_source": image_source}


# ── 원장 관측 (① attestation ② 메인 로컬 이미지) ───────────────────────────────────────────────
_ROLE_KEYS = {"main": "main", "master": "main", "sub": "sub", "slave": "sub"}


def _ledgers_in_attestation(att: dict) -> tuple[dict[str, dict], int]:
    """(역할별 원장, 버린 키 수). 모양은 upstream 스모크가 쓰는 v2 하나다(`multinode_serve_smoke.sh _attest_write` ·
    `nodes.{main,sub}.build_ledger`). 키는 **역할 이름으로만** 받는다 — v2 의 각 노드 값에는 `node_id`(manifest 노드 id ·
    호스트 이름일 수 있다)가 들어 있고, 키로 쓴 문자열은 `per_node_patchsets`·출처 문자열을 타고 PAYLOAD 로 나간다.
    옛 판은 확정되지 않은 모양 셋(`ledgers`·`build_ledgers`·`checks[].node`)을 추측으로 더 읽었다 — 그 키는 무엇이든 될 수
    있어 신원 유출 통로였고, 쓰는 쪽이 없는 모양을 읽는 것은 요청 범위 밖 선반영이다(2026-09-22 감사 · 삭제)."""
    out: dict[str, dict] = {}
    dropped = 0
    nodes = att.get("nodes")
    if isinstance(nodes, dict):
        for n, doc in sorted(nodes.items(), key=lambda kv: str(kv[0])):
            role = _ROLE_KEYS.get(str(n))
            if role is None:
                dropped += 1          # 역할 이름이 아닌 키 — 값·키 문자열 모두 싣지 않는다(개수만 탐침에 남긴다)
                continue
            if isinstance(doc, dict) and isinstance(doc.get("build_ledger"), dict):
                out.setdefault(role, doc["build_ledger"])
    return out, dropped


def _attestation_ref(c: _Ctx) -> str:
    """이 셀에 **실제로 묶인** attestation 파일. evidence 는 셀 이름 → 이미지 compose 라벨의 빌드 config 이름 순으로 묶고
    (코드맵 K2 · 태그2 = `attestation_q38fn-nvfp4-mmap.json`) 그 경로를 `_source` 첫 토큰에 적는다. 옛 판은 늘 셀 이름 파일을
    가리켜 **없는 파일**을 출처로 배포했다(2026-09-22 감사: 태그2 sub_recipe.parity_attestation.source)."""
    att = _get(c.ev, "attestation")
    if isinstance(att, dict):
        if isinstance(att.get("_path"), str) and att["_path"].strip():
            return att["_path"].strip()
        if isinstance(att.get("_source"), str) and att["_source"].strip():
            return att["_source"].split(" ", 1)[0]
    seeds = _get(c.ev, "lineage_seeds") or {}
    ref = seeds.get("attestation") if isinstance(seeds, dict) else None
    if isinstance(ref, str) and ref:
        return ref
    return f"output/{c.topo}/benchlog/attestation_{c.cell}.json" + ("" if isinstance(att, dict) else "(부재)")


def _observe_ledger(c: _Ctx) -> tuple[dict | None, str | None, dict[str, dict], list[dict]]:
    """(메인 원장, 출처, 노드별 원장, 탐침 기록)."""
    probes: list[dict] = []
    att = _get(c.ev, "attestation")
    per_node: dict[str, dict] = {}
    ref = _attestation_ref(c)
    if isinstance(att, dict):
        per_node, dropped = _ledgers_in_attestation(att)
        row = {"tier": "attestation", "ref": ref,
               "result": "observed" if per_node else "no-ledger(nodes.{main,sub}.build_ledger 없음 — 옛 이미지 또는 v1)"}
        if dropped:
            row["ignored_node_keys"] = dropped      # 역할 이름 밖 키의 **개수**만(키 문자열은 신원일 수 있다)
        probes.append(row)
    else:
        probes.append({"tier": "attestation", "ref": ref, "result": "absent"})
    want = "sub" if c.node == "sub" else "main"     # cluster 셀의 대표 적용 집합 = main(양 노드 대조는 parity 가 말한다)
    if want in per_node:
        return per_node[want], f"attestation:{ref}#{want}", per_node, probes
    if per_node:
        # 다른 역할의 원장만 있다 — 그 노드가 무엇을 돌렸는지로 이 노드의 적용 집합을 대리하지 않는다.
        probes[-1]["result"] = f"partial(원장 역할 {sorted(per_node)} — {want} 원장 없음 · 대리하지 않는다)"
    # ② 메인 로컬 이미지 — 서브 셀(node=sub)은 서브의 이미지라 메인 로컬 동명 이미지가 대리하지 못한다.
    if c.node == "sub":
        probes.append({"tier": "local-image", "result": "skipped(node=sub — 서브 스캔 ✗ · 문서기반 회수만)"})
        return None, None, per_node, probes
    image, image_src = _image_ref(c)
    if not image:
        probes.append({"tier": "local-image", "result": f"skipped({image_src})"})
        return None, None, per_node, probes
    cls, _ = c.docker("image", "inspect", "--format", "{{.Id}}", image)
    if cls != "ok":
        probes.append({"tier": "local-image", "image": image, "image_source": image_src,
                       "result": ("image-absent-local(측정 이미지가 로컬에 없다 — 태그로 대체하지 않는다)"
                                  if cls.startswith("rc=") else cls)})
        return None, None, per_node, probes
    if _probe_allowed(c)[0]:
        # 2026-09-22 S2 round 2: 이미지 탐침 능력이 있으면 원장도 **시작하지 않는** 컨테이너에서 cp 로 읽는다 — 옛 `docker run … cat`
        #   은 `--network none` 이어도 컨테이너를 시작한다(탐침 규칙 "start/run ✗"). publish 기본 실행기(탐침 · round 3 부터 기본)도
        #   이 경로로 원장을 본다.
        data, got = _cp_from_image(c, image, LEDGER_IN_IMAGE)
        if data is None:
            # 컨테이너 제거 실패(`rm-failed(…)`)는 부재 판정과 별개의 부수효과 사실이다 — "원장 없음" 줄에 접어 버리지 않는다
            #   (2026-09-22 S2 round 2 리뷰: 옛 줄은 absent 로 시작하면 rm 실패 꼬리를 버렸다 — 남은 컨테이너가 기록에서 사라졌다).
            rm_note = got.split(" · ", 1)[1] if " · " in got else ""
            probes.append({"tier": "local-image", "image": image, "image_source": image_src, "method": "docker create+cp(시작 ✗)",
                           "result": (f"no-ledger(이미지에 {LEDGER_IN_IMAGE} 없음 — docker cp)" if got.startswith("absent")
                                      else f"docker-cp-failed({got})") + (f" · ⚠ {rm_note}" if rm_note and
                                                                           got.startswith("absent") else "")})
            return None, None, per_node, probes
        try:
            doc = json.loads(data.decode("utf-8"))
        except (UnicodeDecodeError, ValueError):
            doc = None
        if not isinstance(doc, dict):
            core.fail("HINT_BUILD_LEDGER_UNREADABLE", f"이미지 원장이 JSON 객체가 아니다: {image}:{LEDGER_IN_IMAGE}",
                      "upstream 빌드 원장 스탠자(source-build.md §4.1)를 확인한다.")
        probes.append({"tier": "local-image", "image": image, "image_source": image_src, "method": "docker create+cp(시작 ✗)",
                       "result": "observed" + ("" if got == "ok" else f"({got})")})
        return doc, f"docker-image(main-local):{image}:{LEDGER_IN_IMAGE}", per_node, probes
    cls, proc = c.docker("run", "--rm", "--network", "none", "--pull", "never", "--entrypoint", "cat",
                         image, LEDGER_IN_IMAGE)
    if cls != "ok" or proc is None:
        # docker run 의 125·126·127 은 **docker 자신**의 실패다(데몬·실행 불가) — "원장 없음(옛 이미지)" 으로 적으면 관측 실패가
        # 부재 사실로 둔갑한다. 그 밖의 비-0 은 컨테이너 안 cat 의 종료코드(파일 부재 = 1)다.
        failed = cls in ("rc=125", "rc=126", "rc=127") or not cls.startswith("rc=")
        probes.append({"tier": "local-image", "image": image, "image_source": image_src,
                       "result": f"docker-run-failed({cls})" if failed else f"no-ledger(cat {cls})"})
        return None, None, per_node, probes
    try:
        doc = json.loads(proc.stdout)
    except ValueError:
        doc = None
    if not isinstance(doc, dict):
        # 원장 파일이 있는데 JSON 이 아니다 — 부재가 아니라 결함이다(조용히 unobservable 로 접지 않는다).
        core.fail("HINT_BUILD_LEDGER_UNREADABLE", f"이미지 원장이 JSON 객체가 아니다: {image}:{LEDGER_IN_IMAGE}",
                  "upstream 빌드 원장 스탠자(source-build.md §4.1)를 확인한다.")
    probes.append({"tier": "local-image", "image": image, "image_source": image_src, "result": "observed"})
    return doc, f"docker-image(main-local):{image}:{LEDGER_IN_IMAGE}", per_node, probes


# ── 패치 목록 · 자기게이트 재구성 ──────────────────────────────────────────────────────────────
def _list_patch_dir(d: Path) -> tuple[list[Path], list[str], list[str]]:
    """(패치 `<NN>-*.sh`, 알려진 비-패치, 규약 밖 이름). 디렉터리 부재·비어 있음은 정상 no-op(source-build.md §4)."""
    patches, known, odd = [], [], []
    if not d.is_dir():
        return patches, known, odd
    for p in sorted(d.iterdir(), key=lambda x: x.name):
        if p.name in KNOWN_NON_PATCH:
            known.append(p.name)
        elif p.is_file() and _PATCH_NAME.fullmatch(p.name):
            patches.append(p)
        elif p.is_dir():
            known.append(p.name + "/")          # `files/` = 50 의 이식 페이로드(패치가 아니라 패치의 입력)
        else:
            odd.append(p.name)
    return patches, known, odd


def _patch_trigger(text: str) -> str | None:
    m = _MODEL_TRIGGER.search(text)
    return m.group(1).strip() if m else None


def _self_gate(text: str) -> dict | None:
    """스크립트의 자기게이트. None = 자기게이트 없음(ungated 또는 내용 조건부 skip — 재구성 불가).
    {"vars": 기본값 선언 변수 전부, "guards": `!= 1 → exit 0` 변수} — 55 는 SM12X_PORT=1 이 SRC_DEPS_AUTHORITY 를 켜는
    하위호환 뒤집기가 있어(55:50-53) **선언된 게이트 변수 전부**가 1 이 아니어야 skip 으로 재구성한다(보수)."""
    defaults = dict(_GATE_DEFAULT.findall(text))
    lines = text.splitlines()
    guards: list[str] = []
    guard_lines: set[int] = set()          # 게이트 블록(`if` ~ `fi`)의 1-기준 줄 — 게이트가 **열렸을 때** 그 안의 exit 0 은 닿지 않는다
    first = None
    for i, line in enumerate(lines):
        m = _GATE_SKIP_IF.match(line)
        if not m:
            continue
        for k, body in enumerate(lines[i + 1:i + 12], start=i + 1):
            if re.match(r"^\s*fi\b", body):
                break
            if re.match(r"^\s*exit\s+0\b", body):
                guards.append(m.group(1))
                first = i if first is None else first
                end = next((j for j in range(k, min(len(lines), i + 12)) if re.match(r"^\s*fi\b", lines[j])), k)
                guard_lines |= set(range(i + 1, end + 2))
                break
    if not guards:
        return None
    for line in lines[:first]:
        st = line.strip()
        if not st or st.startswith("#"):
            continue
        if "$(" in st or "`" in st or not any(rx.match(line) for rx in _PRE_GATE_OK):
            return None          # 게이트 앞에 부수효과가 있을 수 있다 — skip 재구성 불가(적용 미관측으로 싣는다)
    return {"vars": sorted(set(defaults) | set(guards)), "guards": sorted(set(guards)), "defaults": defaults,
            "guard_lines": sorted(guard_lines)}


def _history_build_args(c: _Ctx, dockerfile_text: str) -> tuple[dict, str]:
    """docker history 의 `RUN |N K=V …` 에서 **쓰인 Dockerfile 이 선언한 ARG** 만 모은다(베이스 이미지 층의 NGC 인자 제외).
    층마다 값이 다르면 그 키는 신뢰하지 않는다(모순 = 미관측). 이미지 = 측정 digest(`_image_ref`)."""
    image, image_src = _image_ref(c)
    if not image:
        return {}, f"unobserved({image_src})"
    cls, proc = c.docker("history", "--no-trunc", "--format", "{{.CreatedBy}}", image)
    if cls != "ok" or proc is None:
        return {}, f"unobserved(docker history {cls} · {image_src})"
    names = set(_dockerfile_args(c, dockerfile_text))
    seen: dict[str, set] = {}
    for line in proc.stdout.splitlines():
        m = _HISTORY_RUN_ARGS.match(line.strip())
        if not m:
            continue
        pairs = [tok.partition("=") for tok in m.group(2).split(" ")[: int(m.group(1))]]
        # 우리 층의 `|N` 인자는 **쓰인 Dockerfile 이 선언한 ARG 뿐**이다. 하나라도 밖의 이름(NVIDIA_PYTORCH_VERSION 등)이
        # 섞인 줄은 베이스 이미지 층이다 — 같은 이름이 우연히 겹쳐도 그 줄은 통째로 버린다(2026-09-22 태그2 history 실측).
        if not pairs or any(not eq or k not in names for k, eq, _ in pairs):
            continue
        for k, _, v in pairs:
            seen.setdefault(k, set()).add(v)
    args = {k: next(iter(v)) for k, v in sorted(seen.items()) if len(v) == 1}
    conflict = sorted(k for k, v in seen.items() if len(v) > 1)
    src = f"docker history --no-trunc {image}(RUN |N 인자 · 쓰인 Dockerfile 의 ARG 만 · {image_src})"
    return args, src + (f" · 층간 불일치 제외 {conflict}" if conflict else "")


# ── 옛 이미지(원장 없음) 재구성 v2 — 스크립트 정적 분석 · 이미지 탐침 (F12 · 2026-09-22 S2 round 2) ───────────────────
def _sh_literal(raw: str, known: dict) -> str | None:
    """셸 값 원문(따옴표·꼬리 주석 포함 가능) → 리터럴. 명령 치환·백틱·역슬래시·미해소 변수(`${X:-d}` 포함)가 있으면 None."""
    raw = raw.strip()
    if raw.startswith(("'", '"')):
        q = raw[0]
        end = raw.find(q, 1)
        if end < 0:
            return None
        body, rest = raw[1:end], raw[end + 1:]
        if q == "'":
            return body if not rest.strip() or rest.lstrip().startswith("#") else None
    else:
        parts = raw.split(None, 1)
        body, rest = (parts[0], parts[1] if len(parts) > 1 else "") if parts else ("", "")
    if rest.strip() and not rest.lstrip().startswith("#"):
        return None
    if "$(" in body or "`" in body or "\\" in body:
        return None
    miss: list[str] = []

    def sub(m: re.Match) -> str:
        v = known.get(m.group(1) or m.group(2))
        if v is None:
            miss.append(m.group(0))
            return ""
        return v
    out = _SH_VAR.sub(sub, body)
    return None if miss or "$" in out else out


def _script_shape(text: str) -> dict:
    """패치 스크립트의 정적 모양 — 판정에 쓰는 사실만 모은다(추측 ✗). 셸 실행 줄 = 주석·빈 줄·heredoc 본문을 뺀 줄.
    exits: 성공 종료 가능 `exit`(코드 0·생략·변수 · 따옴표 안 포함) · swallow: 정리 명령 밖 `|| true`·`|| :`(실패 삼킴) · traps: `trap`
    줄(2026-09-22 리뷰 추가 — 둘 다 set +e 와 같은 이유로 (ii) 불성립) · py_exit0: 내장 파이썬 성공 조기 종료 · skip_lines: `skip` 문구 줄(셸+heredoc) ·
    markers: 조건문 밖 `grep -q <리터럴> <절대 경로>`(= 스크립트 자신의 fail-loud 검증) · assert_needles: `assert '<리터럴>' in x` ·
    modules: `import <모듈> as a` + `a.__file__`(대상 경로가 모듈 위치로 정해지는 스크립트)."""
    lines = text.splitlines()
    code: list[tuple[int, str]] = []
    doc: list[tuple[int, str]] = []
    pending: list[tuple[bool, str]] = []
    for n, ln in enumerate(lines, 1):
        if pending:
            dash, delim = pending[0]
            if (ln.strip() if dash else ln.rstrip()) == delim:
                pending.pop(0)
            elif ln.strip() and not ln.lstrip().startswith("#"):
                doc.append((n, ln))
            continue
        if not ln.strip() or ln.lstrip().startswith("#"):
            continue
        code.append((n, ln))
        pending += [(hm.group(1) == "-", hm.group(3)) for hm in _HEREDOC.finditer(ln)]
    logical: list[tuple[int, str]] = []
    buf, start = "", 0
    for n, ln in code:
        if not buf:
            start = n
        if ln.rstrip().endswith("\\"):
            buf += ln.rstrip()[:-1] + " "
            continue
        logical.append((start, buf + ln))
        buf = ""
    if buf:
        logical.append((start, buf))
    known: dict[str, str | None] = {}
    for _, ln in code:
        m = _SH_ASSIGN.match(ln)
        if m:
            val = _sh_literal(m.group(2), known)
            # 다시 할당되면(다른 값) 미해소 — 55 의 하위호환 뒤집기처럼 분기 안 재할당이 있다.
            known[m.group(1)] = val if m.group(1) not in known or known[m.group(1)] == val else None
    exits = []
    for n, ln in code:
        for m in _SH_EXIT.finditer(ln):
            v = m.group(1)
            if v is None or not v.isdigit() or int(v) == 0:
                exits.append({"line": n, "code": v if v is not None else "(생략)"})
    swallow: list[int] = []
    for n, ln in logical:
        for m in _SH_SWALLOW.finditer(ln):
            if ln[m.end():].lstrip().startswith(")"):
                continue                    # `$(… || true)`·`<(… || true)` — 치환 안의 값 탐침(뒤 검사가 판정한다 · 40·50 꼴)
            left = re.split(r"&&|\|\||;|\|", ln[:m.start()])[-1].strip()
            if re.match(r"^[A-Za-z_][A-Za-z0-9_]*=\$\(", left):
                continue                    # `VAR=$(…) || true` — 값 탐침(뒤 검사가 판정한다)
            words = [w for w in left.split() if not re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", w)]
            if words and PurePath(words[0]).name in _SWALLOW_OK_CMDS:
                continue                    # 정리 명령(rm · find -delete …)의 실패는 적용 여부와 무관하다
            swallow.append(n)
    traps = [n for n, ln in logical if _SH_TRAP.search(ln)]
    markers = []
    for n, ln in logical:
        if re.match(r"^\s*(?:if|elif|while|until)\b", ln):
            continue                        # 조건문 안 grep 은 선행조건·트립와이어다(있으면 멈춘다 — 적용 마커가 아니다)
        for m in _GREP_Q.finditer(ln):
            if ln[:m.start()].rstrip().endswith("!"):
                continue
            opts = m.group(1).replace("-", "").replace(" ", "")
            needle = m.group(2) if m.group(2) is not None else m.group(3)
            if "q" not in opts or not needle or ("F" not in opts and any(ch in _BRE_SPECIAL for ch in needle)):
                continue                    # 정규식 특수문자가 있는 BRE 패턴은 리터럴 대조로 판정하지 않는다
            target = _sh_literal(m.group(4), known)
            if target and target.startswith("/"):
                markers.append({"kind": "grep", "needle": needle, "path": target, "line": n})
    every = code + doc
    return {
        "errexit": any(_SET_E.match(ln) for _, ln in code) and not any(_SET_NO_E.match(ln) for _, ln in code),
        "exits": exits,
        "swallow": sorted(set(swallow)),
        "traps": traps,
        "py_exit0": [n for n, ln in every if _PY_EXIT0.search(ln)],
        "skip_lines": sorted(n for n, ln in every if _SKIP_WORD.search(ln)),
        "status_protocol": any("EASY_VLLM_PATCH_STATUS" in ln for _, ln in every),
        "markers": markers,
        "assert_needles": [m.group(2) for _, ln in every for m in _ASSERT_IN.finditer(ln)],
        "modules": sorted({m.group(1) for _, ln in every for m in _MODULE_AS.finditer(ln)
                           if f"{m.group(2)}.__file__" in text}),
    }


def _fail_loud_problems(shape: dict, exclude: set | frozenset = frozenset()) -> list[str]:
    """(ii) 규칙이 성립하지 않는 이유 목록 — 빈 목록 = "성공 종료는 곧 끝까지 실행" 이다(빌드 루프 `bash "$p" || exit 1` 와 합쳐
    적용을 뜻한다). `exclude` = 열린 자기게이트 블록 줄(그 안의 exit 0 은 닿지 않는다)."""
    probs = []
    if not shape["errexit"]:
        probs.append("set -e 부재(또는 해제) — 중간 실패가 종료코드에 남지 않을 수 있다")
    ex = [e for e in shape["exits"] if e["line"] not in exclude]
    if ex:
        probs.append("성공 종료 가능 exit " + ", ".join(f"L{e['line']}(exit {e['code']})" for e in ex[:5]))
    sw = [n for n in shape.get("swallow", ()) if n not in exclude]
    if sw:
        probs.append(f"실패 삼킴 `|| true`·`|| :`(정리 명령 밖 — set -e 가 그 실패를 못 본다) L{sw[:5]}")
    tr = [n for n in shape.get("traps", ()) if n not in exclude]
    if tr:
        probs.append(f"trap(종료 경로·종료코드를 바꿀 수 있다) L{tr[:5]}")
    py = [n for n in shape["py_exit0"] if n not in exclude]
    if py:
        probs.append(f"내장 파이썬 성공 조기 종료 L{py[:5]}")
    sk = [n for n in shape["skip_lines"] if n not in exclude]
    if sk:
        probs.append(f"skip 문구(내용 조건부 skip 경로) L{sk[:5]}")
    if shape["status_protocol"]:
        probs.append("EASY_VLLM_PATCH_STATUS 상태 프로토콜(스스로 skipped 를 선언할 수 있다)")
    return probs


def _patch_loop_failloud(c: _Ctx, df_text: str, phase: str) -> tuple[bool | None, str]:
    """측정 이미지 history 의 패치 루프가 비-0 에서 빌드를 멈추는가(옛 `bash "$p" || exit 1` · 원장 판 `[ "$rc" = 0 ] || {…exit 1`)."""
    dests = _copy_dests(df_text)
    d = PATCH_DIRS[phase]
    if d not in dests:
        return None, f"{d}/ COPY 없음"
    cdir = _dest_path(d, dests[d], True)
    image, image_src = _image_ref(c)
    if not image:
        return None, f"unobserved({image_src})"
    cls, proc = c.docker("history", "--no-trunc", "--format", "{{.CreatedBy}}", image)
    if cls != "ok" or proc is None:
        return None, f"unobserved(docker history {cls})"
    for ln in proc.stdout.splitlines():
        if not ln.startswith("RUN ") or f"{cdir}/*.sh" not in ln:
            continue
        if re.search(r'bash\s+"\$p"\s*\|\|\s*exit\s+[1-9]', ln) or \
                re.search(r'\[\s*"\$rc"\s*=\s*0\s*\]\s*\|\|\s*\{[^}]*exit\s+[1-9]', ln):
            return True, f"docker history 루프 `{cdir}/*.sh … || exit 1`(비-0 = 빌드 중단)"
        return False, f"docker history 의 `{cdir}/*.sh` 루프가 비-0 에서 멈추는 꼴이 아니다"
    return None, f"docker history 에 `{cdir}/*.sh` 루프 층이 없다"


def _probe_allowed(c: _Ctx) -> tuple[bool, str]:
    """탐침 여부 = 실행기의 **선언**(2026-09-22 S2 round 3 · hint.py 기본값과 같은 의미: 탐침 ON · 읽기 전용 실행기를 주입할 때만 OFF).
    기본(None)은 `_ctx` 가 탐침 전용 기본 실행기로 바꿔 둔다(선언함). 주입 실행기는 `image_probe = True` 일 때만 — 선언이 없거나
    False 면 읽기 전용이다(추측해서 켜지 않는다: 대역·읽기 전용 실행기에 create 를 보내면 계약 밖 호출이 된다)."""
    if getattr(c.runner, PROBE_ATTR, False) is True:
        return True, ("artifacts 기본 실행기(탐침 전용 — create/cp/rm · run/start ✗)" if c.default_runner
                      else "주입 실행기(image_probe 선언)")
    return False, ("skipped(주입 실행기가 image_probe 를 선언하지 않았다 — 읽기 전용(--docker-read-only · 대역)으로 본다 · "
                   "create/cp/rm 을 부르지 않는다)")


def _python_roots(c: _Ctx, image: str) -> list[str]:
    """이미지 ENV 값(PATH · LD_LIBRARY_PATH …)이 가리키는 `…/dist-packages`·`…/site-packages` 루트(관측 순서 · 중복 제거)."""
    cls, proc = c.docker("image", "inspect", "--format", "{{json .Config.Env}}", image)
    if cls != "ok" or proc is None:
        return []
    try:
        env = json.loads(proc.stdout)
    except ValueError:
        return []
    roots: list[str] = []
    for kv in env if isinstance(env, list) else []:
        for part in str(kv).partition("=")[2].split(":"):
            m = re.match(r"^(/[^\s]*?/(?:dist|site)-packages)(?:/|$)", part)
            if m and m.group(1) not in roots:
                roots.append(m.group(1))
    return roots


def _docker_cp(c: _Ctx, cid: str, src: str, dest: Path) -> str:
    cls, proc = c.docker("cp", "-L", f"{cid}:{src}", str(dest), cache=False)
    if cls == "ok":
        return "ok"
    err = (proc.stderr if proc is not None else "") or ""
    return "absent" if ("Could not find" in err or "No such" in err) else cls


def _cp_from_image(c: _Ctx, image: str, path: str) -> tuple[bytes | None, str]:
    """측정 이미지 안 파일 하나를 **시작하지 않는** 컨테이너에서 꺼낸다(create → cp → finally rm). (바이트|None, 분류) — 분류 ∈
    ok · absent(파일 없음) · failed(create …) · rm-failed(<cid12>)(꺼냈지만 컨테이너를 못 지웠다 · 기록에 남긴다) · 그 밖 docker 분류."""
    import tempfile
    cls, proc = c.docker("create", "--pull", "never", "--network", "none", "--entrypoint", "/bin/true", image, cache=False)
    out_lines = (proc.stdout if proc is not None else "").strip().splitlines()
    cid = out_lines[-1].strip() if out_lines else ""
    if cls != "ok" or not re.fullmatch(r"[0-9a-f]{12,64}", cid):
        return None, f"failed(docker create {cls})"
    tmp = Path(tempfile.mkdtemp(prefix="hint-probe-"))
    data, got = None, "unobserved"
    try:
        got = _docker_cp(c, cid, path, tmp / "f")
        if got == "ok" and (tmp / "f").is_file():
            data = (tmp / "f").read_bytes()
    finally:
        # `-v`(2026-09-22 S2 round 3 적대 리뷰): create 는 이미지 VOLUME 선언마다 익명 볼륨을 만든다 — `-v` 없는 rm 은 그것을 남긴다
        #   (탐침마다 새는 부수효과 · 컨테이너 목록 대조로는 보이지 않는다). 이름 붙은 볼륨은 만들지 않았으므로 지워지는 것은 이 탐침의 것뿐이다.
        rcls, _ = c.docker("rm", "-v", cid, cache=False)
        shutil.rmtree(tmp, ignore_errors=True)
    if rcls != "ok":
        got = f"{got} · rm-failed({cid[:12]} — `docker rm -v {cid[:12]}` 로 지운다)"
    return data, got


def _image_probe(c: _Ctx, df_text: str) -> dict:
    """측정 이미지 안의 패치 스크립트(COPY 대상 디렉터리) · 컨텍스트 COPY 대상 파일 · 스크립트가 선언한 마커의 대상 파일을 꺼낸다.
    `docker create`(start ✗ · `--pull never` · `--network none`) → `docker cp` → finally `docker rm`(항상). 바이트는 메모리에만
    둔다(직렬화 ✗). 반환 {"record": 탐침 기록(PAYLOAD 용) · "scripts": {phase: {name: (bytes, mode)} | None} ·
    "files": {이미지 경로: bytes | None} · "modules": {모듈: 이미지 경로 | None} · "ran": bool}."""
    if "probe" in c.memo:
        return c.memo["probe"]
    import tempfile
    image, image_src = _image_ref(c)
    rec: dict = {"tier": "image-probe", "image": image, "image_source": image_src}
    res: dict = {"record": rec, "scripts": {}, "files": {}, "modules": {}, "ran": False}
    c.memo["probe"] = res
    ok, why = _probe_allowed(c)
    if not ok:
        rec["result"] = why
        return res
    if not image:
        rec["result"] = f"skipped({image_src})"
        return res
    cls, _ = c.docker("image", "inspect", "--format", "{{.Id}}", image)
    if cls != "ok":
        rec["result"] = f"skipped(측정 이미지가 로컬에 없다 · {cls} — 태그로 대체하지 않는다)"
        return res
    cls, proc = c.docker("create", "--pull", "never", "--network", "none", "--entrypoint", "/bin/true", image, cache=False)
    out_lines = (proc.stdout if proc is not None else "").strip().splitlines()
    cid = out_lines[-1].strip() if out_lines else ""
    if cls != "ok" or not re.fullmatch(r"[0-9a-f]{12,64}", cid):
        rec["result"] = f"failed(docker create {cls})"
        return res
    tmp = Path(tempfile.mkdtemp(prefix="hint-probe-"))
    copies: list[dict] = []
    removed = False
    try:
        dests = _copy_dests(df_text)
        for ph, d in PATCH_DIRS.items():
            if d not in dests:
                continue
            cdir = _dest_path(d, dests[d], True)
            got = _docker_cp(c, cid, cdir, tmp / f"dir-{ph}")
            copies.append({"src": cdir, "result": got})
            base = tmp / f"dir-{ph}"
            res["scripts"][ph] = ({p.name: (p.read_bytes(), p.stat().st_mode & 0o777)
                                   for p in sorted(base.iterdir(), key=lambda x: x.name)
                                   if p.is_file() and not p.is_symlink()}
                                  if got == "ok" and base.is_dir() else None)
        targets: list[str] = []
        for src, dst in sorted(dests.items()):
            if src in PATCH_DIRS.values() or not (c.out / src).is_file():
                continue
            targets.append(_dest_path(src, dst, False))
        # 마커 대상: 이미지에 구워진 스크립트(없으면 작업트리)의 텍스트에서 파생한다 — 판정하는 바이트와 같은 바이트.
        modules: set[str] = set()
        for ph, d in PATCH_DIRS.items():
            baked = res["scripts"].get(ph) or {}
            names = set(baked) | {p.name for p in _list_patch_dir(c.out / d)[0]}
            for name in sorted(n for n in names if _PATCH_NAME.fullmatch(n)):
                text = (baked[name][0].decode("utf-8", "replace") if name in baked
                        else _read(c.out / d / name) if (c.out / d / name).is_file() else "")
                sh = _script_shape(text)
                targets += [m["path"] for m in sh["markers"]]
                modules |= set(sh["modules"])
        roots = _python_roots(c, image) if modules else []
        rec["python_roots"] = roots
        for mod in sorted(modules):
            res["modules"][mod] = None
            for root in roots:
                for cand in (f"{root}/{mod.replace('.', '/')}.py", f"{root}/{mod.replace('.', '/')}/__init__.py"):
                    dest = tmp / f"mod-{len(copies)}"
                    got = _docker_cp(c, cid, cand, dest)
                    copies.append({"src": cand, "result": got})
                    if got == "ok" and dest.is_file():
                        res["files"][cand] = dest.read_bytes()
                        res["modules"][mod] = cand
                        break
                if res["modules"][mod]:
                    break
        for t in sorted(set(targets)):
            dest = tmp / f"file-{len(copies)}"
            got = _docker_cp(c, cid, t, dest)
            copies.append({"src": t, "result": got})
            res["files"][t] = dest.read_bytes() if got == "ok" and dest.is_file() else None
        res["ran"] = True
    finally:
        rcls, _ = c.docker("rm", "-v", cid, cache=False)      # `-v` = 익명 볼륨까지(위 _cp_from_image 와 같은 이유)
        removed = rcls == "ok"
        shutil.rmtree(tmp, ignore_errors=True)
    rec["copies"] = copies
    rec["container_removed"] = removed
    rec["result"] = "done" if removed else f"done(⚠ 컨테이너 제거 실패 — `docker rm -v {cid[:12]}` 로 지운다)"
    return res


def _marker_check(shape: dict, probe: dict | None) -> dict:
    """스크립트가 **스스로 선언한** 적용 마커가 이미지 안 대상 파일에 있는가. 선언이 없으면 판정하지 않는다(지어내지 않는다)."""
    if not probe or not probe.get("ran"):
        return {"status": "probe-not-run", "declared": 0, "found": 0}
    files = probe["files"]
    declared = [(m["needle"], [m["path"]]) for m in shape["markers"]]
    mod_paths = [p for p in (probe["modules"].get(m) for m in shape["modules"]) if p]
    declared += [(needle, mod_paths) for needle in shape["assert_needles"]]
    if not declared:
        return {"status": "no-declared-markers", "declared": 0, "found": 0}
    found, missing, where = 0, [], []
    for needle, paths in declared:
        hit = next((p for p in paths if files.get(p) is not None and needle.encode("utf-8") in files[p]), None)
        if hit:
            found += 1
            where.append(hit)
        else:
            missing.append(needle)
    return {"status": "all-found" if not missing else "missing", "declared": len(declared), "found": found,
            "missing": missing[:5], "targets": sorted(set(where))}


def _reconstruct(name: str, text: str, phase: str, args: dict, env: dict | None = None) -> dict:
    """원장 없는 이미지의 패치 한 개 판정(라벨된 재구성). 순서가 의미다:
    (i) 자기게이트 + history 빌드 인자가 게이트를 끈다 → skipped `reconstructed(docker-history+gate)`
    (ii) skip 경로가 아예 없다(set -e · 성공 종료 exit 없음 · skip 문구 없음 · 상태 프로토콜 없음) × history 루프가 fail-loud ×
         측정 이미지 실재 × **판정한 바이트 = 이미지에 구워진 바이트**(탐침) → applied `reconstructed(fail-loud+built)` — 루프가
         `bash "$p" || exit 1` 이라 빌드가 끝났으면 끝까지 돌았다. 탐침이 없으면 미관측(빌드 뒤 추가·수정된 작업트리 스크립트를
         applied 로 지어내지 않는다) · 탐침이 선언 마커 부재를 보면 미관측(반대 증거 · 2026-09-22 S2 round 2 리뷰)
    (iii) 내용 조건부 skip 경로가 있다 → 이미지 탐침이 가른다: 스크립트 바이트가 이미지 바이트이고 **선언 마커 전부**가 대상 파일에
         있으면 applied `image-probe(script-sha+marker)` · 아니면 unobserved.
    env = {"loop": (bool|None, 출처), "image_exists": bool, "probe": `_image_probe` 결과|None, "identity_ok": bool}."""
    env = env or {}
    trig = _patch_trigger(text)
    row = {"phase": phase, "file": name, "declared_model_trigger": trig}
    gate = _self_gate(text)
    exclude: set[int] = set()
    gate_note = ""
    if gate is not None:
        absent = [v for v in gate["vars"] if v not in args]
        if absent:
            return {**row, "result": "unobserved", "result_source": "none",
                    "reason": f"원장 부재 · 게이트 변수 {absent} 의 빌드 인자 미관측(docker history) — 적용 미관측"}
        # 실효값 = 스크립트가 실제로 보는 값. `: "${VAR:=d}"` 는 **unset 뿐 아니라 빈 값도** d 로 채운다(셸 `:=` 의미) —
        # history 의 빈 인자(`SM12X_PORT=`)를 그대로 "1 이 아님 → skip" 으로 읽으면, 기본값이 1 인 게이트에서 실제로는 **돈**
        # 패치를 skip 이라 지어낸다(2026-09-22 감사: 옛 판은 기본값을 모아 두고 쓰지 않았다).
        eff = {v: (args[v] if args[v] != "" or v not in gate["defaults"] else gate["defaults"][v]) for v in gate["vars"]}
        on = [v for v in gate["vars"] if eff[v] == "1"]
        if not on:
            vals = " · ".join(f"{v}={args[v] or '<empty>'}" + (f"(:={eff[v]})" if eff[v] != args[v] else "")
                              for v in gate["vars"])
            return {**row, "result": "skipped", "result_source": "reconstructed(docker-history+gate)",
                    "reason": f"{vals}(docker history) — 자기게이트 `!= 1 → exit 0`"}
        if not all(eff[g] == "1" for g in gate["guards"]):
            # 일부 게이트만 열렸다(55 의 하위호환 뒤집기처럼 변수끼리 얽힐 수 있다) — 어느 가드가 닫히는지 단정하지 않는다.
            return {**row, "result": "unobserved", "result_source": "none",
                    "reason": f"원장 부재 · 게이트 변수 {on}=1 이지만 가드 {gate['guards']} 가 전부 열리지는 않았다 — 실행 결과 미관측"}
        exclude = set(gate["guard_lines"])
        gate_note = f" · 자기게이트 열림({', '.join(f'{g}=1' for g in gate['guards'])} — 게이트 블록의 exit 0 은 닿지 않는다)"
    shape = _script_shape(text)
    probs = _fail_loud_problems(shape, exclude)
    loop_ok, loop_src = env.get("loop") or (None, "루프 미관측")
    probe = env.get("probe")
    identity_ok = bool(env.get("identity_ok"))
    mk = _marker_check(shape, probe) if identity_ok else \
        {"status": "script-identity-unproven", "declared": 0, "found": 0}
    evidence = {"static": probs or ["skip 경로 없음"], "loop": loop_src, "markers": mk}
    # (ii) 는 **판정한 바이트 = 이 빌드가 실행한 바이트** 일 때만 성립한다(2026-09-22 · plan_26092119 S2 round 2 리뷰). 루프
    #   `bash "$p" || exit 1` 가 증명하는 것은 "빌드 당시 디렉터리에 있던 스크립트는 모두 0 으로 끝났다" 이지 "지금 작업트리의 이
    #   스크립트가 그때 있었다" 가 아니다 — 탐침이 없으면(hint.py 기본 publish · --docker-read-only) 빌드 뒤 추가·수정된 작업트리
    #   스크립트가 applied 로 판정됐다(자체검사 63-notbaked: 이미지에 없는데 reconstructed(fail-loud+built)). 스크립트 바이트는
    #   비추적이고(git ✗) mtime 은 재렌더가 덮어 쓰므로(태그2: 같은 바이트인데 mtime 이 Created 뒤) 이미지 탐침만이 동일성 증거다.
    #   또 탐침이 **선언 마커 부재**를 봤으면 그것은 반대 증거다 — 조용히 넘기고 applied 로 적지 않는다(결정 경로의 침묵 무시 ✗).
    if not probs and loop_ok and env.get("image_exists") and identity_ok and mk["status"] != "missing":
        return {**row, "result": "applied", "result_source": "reconstructed(fail-loud+built)", "evidence": evidence,
                "reason": (f"skip 경로 없음(set -e · 성공 종료 exit·skip 문구·상태 프로토콜 없음) × {loop_src} × 측정 이미지 실재 × "
                           f"판정한 바이트 = 이미지에 구워진 바이트 — 빌드가 끝났으므로 끝까지 돌았다{gate_note}"
                           + (f" · 이미지 마커 {mk['found']}/{mk['declared']} 확인" if mk.get("declared") else ""))}
    if not probs and loop_ok and env.get("image_exists") and not identity_ok:
        return {**row, "result": "unobserved", "result_source": "none", "evidence": evidence,
                "reason": ("원장 부재 · 정적 규칙(ii)(skip 경로 없음 × fail-loud 루프 × 이미지 실재)은 성립하나 판정한 스크립트 바이트가 "
                           "이 빌드가 실행한 바이트인지 미확인(이미지 탐침 결과 없음 — 탐침은 publish 기본이다: `--docker-read-only` "
                           "없이 측정 이미지가 있는 호스트에서 다시 publish 하면 가른다 · 탐침 실패면 탐침 기록을 본다 · 작업트리 "
                           f"스크립트는 빌드 뒤 추가·수정됐을 수 있다) — 적용 미관측{gate_note}")}
    if not probs and loop_ok and env.get("image_exists") and mk["status"] == "missing":
        return {**row, "result": "unobserved", "result_source": "none", "evidence": evidence,
                "reason": (f"원장 부재 · 정적 규칙(ii)은 성립하나 이미지 탐침이 선언 마커 부재 {mk.get('missing')} 를 봤다 — fail-loud "
                           "스크립트가 끝까지 돌았다는 추론과 모순(뒤 층이 대상 파일을 덮었거나 대상 경로 해소가 틀렸다) · 적용으로 적지 "
                           f"않는다 — 적용 미관측{gate_note}")}
    if mk["status"] == "all-found":
        return {**row, "result": "applied", "result_source": "image-probe(script-sha+marker)", "evidence": evidence,
                "reason": (f"이미지 탐침: 판정한 스크립트 = 이미지에 구워진 바이트 · 선언 마커 {mk['found']}/{mk['declared']} 가 대상 "
                           f"파일에 있다(패치 **후** 상태 관측 — 이 빌드가 썼는지 베이스가 이미 가졌는지는 가르지 않는다) · "
                           f"정적 판정 불가 사유: {'; '.join(probs) or loop_src}{gate_note}")}
    why_probe = {"probe-not-run": "이미지 탐침 안 함", "script-identity-unproven": "스크립트 바이트가 이미지 바이트인지 미확인",
                 "no-declared-markers": "스크립트가 선언한 적용 마커 없음 — 이미지 탐침으로도 가르지 못한다",
                 "missing": f"선언 마커 부재 {mk.get('missing')}(빌드는 끝났다 — 모순 · 대상 경로 해소를 확인한다)"}.get(
        mk["status"], mk["status"])
    return {**row, "result": "unobserved", "result_source": "none", "evidence": evidence,
            "reason": (f"원장 부재 · 정적 판정 불가({'; '.join(probs) if probs else loop_src}) · {why_probe} — 적용 미관측"
                       f"{gate_note}")}


def _ledger_rows(ledger: dict) -> list[dict]:
    rows = []
    for src_key, phase_default in (("patches", None), ("inline_patches", "inline")):
        for r in ledger.get(src_key) or ():
            if not isinstance(r, dict):
                continue
            phase = phase_default or r.get("phase")
            name = r.get("file") if phase_default is None else (r.get("name") or r.get("file"))
            if phase not in ("pre", "post", "inline") or not name:
                continue
            res = r.get("result")
            # result_source 어휘는 공유 계약(2026-09-22 S2 round 2)의 판정 **출처 층**이다(build-ledger · reconstructed(…) ·
            #   image-probe(…) · none). 원장 안의 분류 근거(status-file · log-token · exit-code)는 ledger_result_source 로 보존한다.
            row = {"phase": phase, "file": str(name), "reason": r.get("reason"),
                   "result_source": "build-ledger", "ledger_result_source": r.get("result_source") or "exit-code",
                   "declared_model_trigger": r.get("declared_model_trigger"),
                   "script_sha256": r.get("script_sha256")}
            if res in ("applied", "skipped"):
                row["result"] = res
            else:           # 원장 어휘 밖 — 버리지 않고 미관측으로 싣는다
                row.update(result="unobserved", reason=f"원장 result 어휘 밖: {res!r}")
            rows.append({k: v for k, v in row.items() if v is not None})
    rows.sort(key=lambda r: ({"pre": 0, "post": 1, "inline": 2}[r["phase"]], r["file"]))
    return rows


def _post_label(row: dict) -> dict:
    """X10: post 패치 = 공유 이미지 입력(이미지 하나가 모든 모델 — 헌법 이미지 네이밍 불변식)."""
    if row.get("phase") != "post":
        return row
    trig = row.get("declared_model_trigger")
    label = ("공유 이미지 입력 — 이 모델의 요구가 아니라 공유 이미지의 일부" + (f"(model-trigger: {trig})" if trig else
             "(model-trigger 헤더 없음)"))
    return {**row, "shared_image_input": True, "label": label}


def _applied(c: _Ctx, *, probe: bool = True) -> tuple[dict, dict, dict | None, dict[str, dict]]:
    """(applied_set, build selection, 메인 원장, 노드별 원장). applied_set 은 SPEC §2.1 모양 + probes/excluded.
    `probe=False` = 패치 이미지 탐침(`_image_probe`)을 부르지 않는다(재현 절차만 필요한 호출). 원장 관측 ②는 여전히 돈다 —
    실행기가 `image_probe` 를 선언하면(또는 실물 기본 실행기) 그것도 시작하지 않는 컨테이너 create/cp/rm 1회다(2026-09-22 리뷰 정정:
    옛 문구 "create/cp/rm 0" 은 사실이 아니었다)."""
    if ("applied", probe) in c.memo:
        return c.memo[("applied", probe)]
    ledger, ledger_source, per_node, probes = _observe_ledger(c)
    sel = _build_selection(c, ledger)
    df_path = c.out / sel["dockerfile"]
    if not df_path.is_file():
        # 빈 텍스트로 계속하면 "이 Dockerfile 은 패치 디렉터리를 COPY 하지 않는다" 가 되어 모든 패치가 excluded_by_recipe 라는
        # **거짓 사유**로 빠진다(결정 경로의 침묵 폴백 · 2026-09-22 감사).
        core.fail("HINT_DOCKERFILE_ABSENT", f"쓰인 Dockerfile 이 없다: output/{c.topo}/{sel['dockerfile']}",
                  f"`render_dockerfile.py` 로 output/{c.topo}/ 를 다시 렌더하거나 셀 env BUILD_DOCKERFILE 을 확인한다.")
    # F1(2026-09-22 S2 round 2): COPY·ARG 판독은 **이미지를 지은 개정**의 텍스트로 한다 — 작업트리가 빌드 뒤 바뀌었으면
    #   (원장 스탠자 추가 등) 그 텍스트는 이 이미지의 빌드 입력이 아니다(history 미관측이면 선택이 작업트리를 돌려준다).
    df_text = _select_dockerfile(c, sel["dockerfile"])["text"]
    copies = _dockerfile_copies(df_text)
    in_recipe = {ph for ph, d in PATCH_DIRS.items() if d in copies}
    excluded: list[dict] = []
    listing: dict[str, list[Path]] = {}
    for ph, d in PATCH_DIRS.items():
        patches, known, odd = _list_patch_dir(c.out / d)
        listing[ph] = patches
        for name in known:
            excluded.append({"phase": ph, "file": name, "why": "known-non-patch(패치가 아님 — 명시적 건너뜀)"})
        for name in odd:
            excluded.append({"phase": ph, "file": name, "why": "nonconforming(<NN>-*.sh 규약 밖 — 싣지 않음)"})
        if ph not in in_recipe:
            for p in patches:
                excluded.append({"phase": ph, "file": p.name,
                                 "why": f"excluded_by_recipe({sel['dockerfile']} 이 {d}/ 를 COPY 하지 않는다 — 실행된 적 없음)"})
    if ledger is not None:
        rows = [_post_label(r) for r in _ledger_rows(ledger)]
        in_ledger = {(r["phase"], r["file"]) for r in rows}
        for ph in sorted(in_recipe):
            for p in listing[ph]:
                if (ph, p.name) not in in_ledger:
                    excluded.append({"phase": ph, "file": p.name,
                                     "why": "not-in-ledger(원장에 없음 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)"})
        applied = {"status": "observed", "source": ledger_source, "patches": rows, "probes": probes}
        if len(per_node) > 1:
            applied["per_node_patchsets"] = {n: sorted(f"{r['phase']}/{r['file']}={r['result']}"
                                                       for r in _ledger_rows(doc)) for n, doc in sorted(per_node.items())}
    else:
        args, args_src = _history_build_args(c, df_text)
        image, _ = _image_ref(c)
        exists = bool(image) and c.docker("image", "inspect", "--format", "{{.Id}}", image)[0] == "ok"
        # 이미지 탐침(F12): 구워진 스크립트 바이트 · 마커 대상 파일. 읽기 전용 실행기면 건너뛰고 그 사실을 기록한다.
        pr = _image_probe(c, df_text) if probe else None
        rows = []
        ship: dict = c.memo.setdefault("ship_patch", {})
        for ph in ("pre", "post"):
            if ph not in in_recipe:
                continue
            loop = _patch_loop_failloud(c, df_text, ph)
            baked = pr["scripts"].get(ph) if pr and pr.get("ran") else None
            wt = {p.name: p for p in listing[ph]}
            names = sorted(set(wt) | {n for n in (baked or {}) if _PATCH_NAME.fullmatch(n)})
            for name in names:
                wp, bb = wt.get(name), (baked or {}).get(name)
                if baked is not None and bb is None:
                    # 이미지의 COPY 대상 디렉터리에 없다 = 이 빌드의 컨텍스트에 없었다(원장 경로의 not-in-ledger 와 같은 결).
                    excluded.append({"phase": ph, "file": name,
                                     "why": "not-in-image(측정 이미지의 패치 디렉터리에 없다 — 빌드 뒤 추가된 파일로 본다 · 싣지 않음)"})
                    continue
                if bb is not None:
                    text = bb[0].decode("utf-8", "replace")
                    if wp is None:
                        ident = "image-probe(작업트리에 없음 — 이미지 바이트로 판정 · 이미지 바이트를 싣는다)"
                        ship[(ph, name)] = bb
                    elif wp.read_bytes() == bb[0]:
                        ident = "image-probe(sha256 일치 — 실린 바이트 = 이미지에 구워진 바이트)"
                    else:
                        ident = "image-probe(작업트리와 다름 — 이미지 바이트로 판정 · 이미지 바이트를 싣는다)"
                        ship[(ph, name)] = bb
                else:
                    text = _read(wp)
                    ident = (f"unobserved({(pr or {}).get('record', {}).get('result') or '이미지 탐침 안 함'}) — 작업트리 바이트가 "
                             "빌드 당시와 같다는 보장 없음")
                row = _reconstruct(name, text, ph, args, {"loop": loop, "image_exists": exists, "probe": pr,
                                                          "identity_ok": bb is not None})
                row["script_identity"] = ident
                if bb is not None:
                    row["image_script_sha256"] = core.sha256_bytes(bb[0])
                rows.append(_post_label(row))
        probes.append({"tier": "reconstruction", "result": "done" if args else "build-args-unobserved",
                       "source": args_src})
        if pr is not None:
            probes.append(pr["record"])
        applied = {"status": "unobservable", "source": "no-build-ledger", "patches": rows, "probes": probes,
                   "reconstruction": {"source": "docker-history+gate-rule · fail-loud-rule · image-probe",
                                      "image": _image_ref(c)[0], "build_args": args, "build_args_source": args_src,
                                      "caveat": ("원장 부재 — 결과는 **라벨된 재구성**이다(관측된 원장이 아니다). skipped = history 빌드 "
                                                 "인자가 자기게이트를 끈다 · applied = 이미지에 구워진 스크립트 바이트(탐침)가 skip 경로 "
                                                 "없음 × fail-loud 루프 × 측정 이미지 실재(reconstructed(fail-loud+built)) 또는 이미지에 "
                                                 "구워진 스크립트 바이트 × 선언 마커 전부 실재(image-probe(script-sha+marker) — 패치 후 상태 "
                                                 "관측) · 그 밖 = 미관측(X9) — 탐침이 없으면 applied 는 나오지 않는다. skipped 는 탐침이 "
                                                 "없을 때 작업트리 바이트로 판정한다(script_identity 가 image-probe 가 아니면 빌드 당시 "
                                                 "바이트와 같다는 보장이 없다).")}}
    applied["excluded"] = sorted(excluded, key=lambda x: (x["phase"], x["file"]))
    c.memo[("applied", probe)] = (applied, sel, ledger, per_node)
    return applied, sel, ledger, per_node


def applied_set(repo: Path, ev: Any, *, runner: DockerRunner | None = None, forward_module: Any | None = None) -> dict:
    """적용 집합(SPEC §5.6): ① attestation 원장 ② 메인 로컬 이미지 원장 ③ X9 재구성 ④ 미관측."""
    return _applied(_ctx(repo, ev, runner=runner, forward_module=forward_module))[0]


# ── env 형상 ────────────────────────────────────────────────────────────────────────────────
def _shape_key_axis(key: str, value: str) -> str:
    """옛 `hint_collect.shape_env_value` 그대로(규칙 순서가 의미다): 튜닝 키 → 값 유지 · `/` 경로 → manifest 필드 ·
    신원 축 키 → 노드 필드 · 그 밖 → 값 유지. 키는 부르는 쪽이 항상 남긴다."""
    upper = key.upper()
    if upper in TUNING_KEYS:
        return value
    if value.startswith("/"):
        return _manifest_ph(key.lower())
    if any(upper.endswith(s) or f"{s}_" in f"_{upper}_" for s in IDENTITY_KEY_AXIS):
        return _manifest_ph(f"nodes[].{key.lower()}")
    return value


def _manifest_ph(field_name: str) -> str:
    from . import pii
    return pii.manifest_placeholder(field_name)


def _tier_fn(rd) -> tuple[Callable[[str], tuple[str, str | None]], str]:
    """(층 판정 함수, 규칙 이름). 정본 = render_dockerfile.env_tier(key). 부재면 **같은 모듈의 프리셋/불변 상수**에서 층을
    파생한다 — 손목록이 아니라 같은 SSOT 이고, 그 사실을 형상 헤더와 결과에 적는다(소리 나는 폴백). 둘 다 없으면 차단."""
    fn = getattr(rd, "env_tier", None)
    if callable(fn):
        return fn, "render_dockerfile.env_tier"
    need = ("NCCL_PRESETS", "NCCL_INVARIANTS", "CLUSTER_PRESETS", "CLUSTER_INVARIANTS")
    if not all(isinstance(getattr(rd, n, None), dict) for n in need):
        core.fail("HINT_ENV_TIER_OWNER_MISSING",
                  "render_dockerfile.env_tier(key) 도 3층 상수(NCCL_/CLUSTER_ PRESETS·INVARIANTS)도 없다.",
                  "upstream-version-watch render_dockerfile.py 에 공개 env_tier(key) 를 둔다(SPEC §6.2).")
    inv = set(rd.NCCL_INVARIANTS) | set(rd.CLUSTER_INVARIANTS)
    pre = set().union(*(set(v) for v in rd.NCCL_PRESETS.values()), *(set(v) for v in rd.CLUSTER_PRESETS.values()))

    def derived(key: str) -> tuple[str, str | None]:
        if key in inv:
            return "invariant", None
        if key in pre:
            return "preset", None
        return "env", None           # 프리셋·불변 밖 = ①환경값(manifest 파생) — 보수적으로 치환한다(unknown 도 치환)
    return derived, "derived(render_dockerfile 3층 상수 · env_tier 부재)"


def _env_shape(c_repo: Path, path: Path, kind: str, *, render_module: Any | None = None,
               sf: Any | None = None, regen: dict | None = None, measured: str | None = None) -> tuple[str, str]:
    """env 형상 텍스트와 규칙 이름. `regen`(F2 · 2026-09-22 S2 round 2) = 이 파일이 측정 **뒤** 다시 쓰였다({mtime_utc,
    measured_utc, measured_source}) — 그러면 리터럴 값은 측정 당시 값이 아닐 수 있으므로 싣지 않고 `REGENERATED_VALUE` 로
    바꾼다(키는 남긴다 · `<manifest.…>` 표지는 값이 아니라 파생 규칙이라 남긴다). round 1: `.env.interconnect` 가 09-17 에
    `NCCL_NET=Socket`·`NCCL_IB_DISABLE=1` 로 재생성됐는데 "값 유지 — 재현 사실" 로 실렸고, 측정 엔진 로그는 `NET/IB` 였다.
    `measured` = 측정 시각(대조했으나 재생성이 아니면 헤더에 그 사실을 적는다 · None = 측정 시각 미관측)."""
    sf = sf or _forward_module(c_repo)
    env = sf.read_env_first(path)
    renderer = {"cluster": "--cluster-envfile --topology multi --manifest output/<t>/manifest.yaml",
                "interconnect": "--nccl-envfile --manifest output/<t>/manifest.yaml"}.get(kind)
    lines = ["# 형상 템플릿 — 발행 시점 형상이다(운영자 실값 ✗). 키는 항상 남는다: 키를 지우면 수신자는 그 변수의",
             "# 존재 자체를 모른다(2026-09-06 · 멀티 클러스터 5변수가 그렇게 빠졌다)."]
    if regen:
        lines += [f"# ⚠ 측정 후 재생성: 이 파일은 측정({regen['measured_utc']} · {regen['measured_source']}) 뒤 "
                  f"{regen['mtime_utc']}(mtime 관측)에 다시 쓰였다.",
                  f"#   측정 당시 값은 관측되지 않았다 — 리터럴 값은 싣지 않고 `{REGENERATED_VALUE}` 로 둔다.",
                  "#   키 목록도 재생성 시점의 형상이다(측정 당시 키 집합과 같다는 보장 없음) · `<manifest.…>` 표지는 파생 규칙이라 남긴다."]
    if regen:
        keep = "리터럴 값 미실림 — 측정 후 재생성"
    elif measured:
        keep = f"값 유지 — 재현 사실(mtime ≤ 측정 {measured} 확인)"
    else:
        keep = "값 유지 — 측정 시각 미관측이라 측정 당시 값인지 대조하지 못했다"
    if kind in ("cluster", "interconnect"):
        rd = _render_module(c_repo, render_module)
        tier, rule = _tier_fn(rd)
        lines += [f"# 값의 정본 = 자기 manifest → `render_dockerfile.py {renderer}` 로 다시 파생한다.",
                  f"# 층 규칙: {rule} (①env=치환 · ②preset·③invariant={keep})"]
        for key, value in env.items():
            try:
                got = tier(key)
            except (KeyError, ValueError) as e:
                core.fail("HINT_ENV_TIER_UNKNOWN", f"{path.name}:{key} 를 env_tier 가 판정하지 못했다: {e}",
                          "render_dockerfile 3층(NCCL_/CLUSTER_ PRESETS·INVARIANTS)에 없는 키다 — 파일이 낡았는지 본다.")
            if not (isinstance(got, tuple) and len(got) == 2 and got[0] in ENV_TIERS):
                core.fail("HINT_ENV_TIER_UNKNOWN", f"{path.name}:{key} 의 env tier 가 규약 밖이다: {got!r}",
                          f"env_tier 는 {ENV_TIERS} 중 하나와 세부(또는 None)를 돌려준다.")
            t, detail = got
            if t == "env":
                # 표지 모양은 slave_forward._value_shape 와 같다(`<manifest.<detail>>`) — 한 레시피 안에서 두 모양 ✗.
                shaped = f"<{detail}>" if detail and str(detail).startswith("manifest") else \
                    _manifest_ph(str(detail) if detail else f"{kind}.{key.lower()}")
            elif t == "unknown":
                # 렌더러 3층 밖 키(손으로 더했거나 낡은 파일) — 층을 모르니 키-축 규칙으로 보수적으로 형상화하고 표시한다.
                shaped = _shape_key_axis(key, value.strip())
            else:
                shaped = value.strip()
            if regen and not _PLACEHOLDER_VALUE.match(shaped):
                shaped = REGENERATED_VALUE
            lines.append(f"{key}={shaped}    # tier={t}" + ("(키-축 규칙)" if t == "unknown" else ""))
    else:
        rule = "key-axis(IDENTITY_KEY_AXIS · TUNING_KEYS · 2026-09-06)"
        lines.append(f"# 층 규칙: {rule} — 값의 정본은 manifest 의 동명 필드다 · 튜닝·그 밖 값 = {keep}.")
        for key, value in env.items():
            shaped = _shape_key_axis(key, value.strip())
            if regen and not _PLACEHOLDER_VALUE.match(shaped):
                shaped = REGENERATED_VALUE
            lines.append(f"{key}={shaped}")
    text = "\n".join(lines) + "\n"
    # 2차 백스톱(2026-09-06 · plan_26090616 Q9): 키 축 판정은 새 이름의 지문 변수를 놓칠 수 있다. 결과를 배포 강도로 한 번
    # 더 훑고 남아 있으면 **가리지 않고 차단**한다 — 조용히 덧칠하면 형상화가 무엇을 놓쳤는지 영영 드러나지 않는다.
    from . import pii
    # 리터럴 없이 스캔하고 통과를 선언하지 않는다 — 용어 파일 부재 = 차단(`require_terms` · 2026-09-22 통합: 옛 `terms or []` 는
    #   부재를 조용히 "리터럴 0개" 로 접었다). 백스톱은 가장 엄격한 강도다(옛 hint_collect: 절번호 예외 ✗ · 형상화 결과엔 § 가 없다).
    hits = pii.scan_text(text, pii.require_terms(c_repo), profile="deploy", section_anchor_exempt=False)
    if hits:
        core.fail("HINT_ENV_SHAPE_PII", f"{path.name} 형상화 뒤에도 배포 금지 패턴이 남았다(덧칠 ✗):\n  "
                  + pii.render_hits(hits, 10),
                  "해당 키를 IDENTITY_KEY_AXIS 에 편입하거나(render 3층 파일이면 env_tier 를) 고친다.")
    return text, rule


def env_shape(repo: Path, path: Path, kind: str, *, render_module: Any | None = None,
              forward_module: Any | None = None, measured_utc: str | None = None) -> str:
    """`.env.cluster`/`.env.interconnect` = render 3층 치환 · 그 밖(`kind` 아무거나) = 키-축 규칙(SPEC §5.6).
    `measured_utc` 가 있고 파일 mtime 이 그 뒤면 측정 후 재생성으로 형상화한다(F2 · 리터럴 값 미실림)."""
    p = Path(path)
    measured = _utc_z(measured_utc) if measured_utc else None
    mt = _mtime_utc(p)
    regen = ({"mtime_utc": mt, "measured_utc": measured, "measured_source": "호출부 measured_utc"}
             if measured and mt and mt > measured else None)
    return _env_shape(Path(repo).resolve(), p, kind, render_module=render_module,
                      sf=_forward_module(Path(repo).resolve(), forward_module), regen=regen, measured=measured)[0]


# ── 서빙 러너 파생 (compose 가 실제로 부르는 것만) ──────────────────────────────────────────────
def _compose_services(text: str) -> dict[str, str]:
    m = re.search(r"(?m)^services:\s*$", text)
    if not m:
        return {}
    body = text[m.end():]
    out: dict[str, str] = {}
    for sm in re.finditer(r"(?ms)^  ([A-Za-z0-9_.-]+):\s*\n(.*?)(?=^  [A-Za-z0-9_.-]+:\s*\n|^\S|\Z)", body):
        out[sm.group(1)] = sm.group(2)
    return out


def _serving_blocks(text: str) -> list[tuple[str, str]]:
    """debug 전용 프로필 서비스를 뺀 서비스 블록(`profiles: [debug]` 는 서빙 경로가 아니다 — debug-init.sh)."""
    out = []
    for name, block in sorted(_compose_services(text).items()):
        pm = re.search(r"profiles:\s*\[([^\]]*)\]", block)
        profiles = {p.strip().strip("'\"") for p in pm.group(1).split(",")} if pm else set()
        if profiles and profiles <= {"debug"}:
            continue
        out.append((name, block))
    return out


def _command_texts(block: str) -> list[str]:
    """서비스 블록의 `command:`·`entrypoint:` 값 — 한 줄 흐름 목록(`["bash", "/app/…"]`)과 블록 목록(`- bash` 줄들) 둘 다.
    한 줄 꼴만 읽으면 블록 꼴 compose 에서 러너가 **조용히** 빠진다(서빙 경로 파생의 침묵 누락)."""
    out: list[str] = []
    lines = block.splitlines()
    for i, ln in enumerate(lines):
        m = re.match(r"^(\s*)(?:command|entrypoint):\s*(.*)$", ln)
        if not m:
            continue
        indent, parts = len(m.group(1)), [m.group(2)]
        for nxt in lines[i + 1:]:
            if nxt.strip() and len(nxt) - len(nxt.lstrip()) <= indent:
                break
            parts.append(nxt.strip())
        out.append(" ".join(parts))
    return out


def _runner_names(c: _Ctx) -> tuple[list[str], list[str]]:
    """(서빙 경로가 부르는 러너 스크립트 이름, 모델 러너 참조) — compose command 와 러너의 `source` 에서 파생."""
    if c.compose_path is None:
        return [], []
    text = _compose_text(c)       # 측정 당시 개정(2026-09-22 S2 round 3 — 측정 뒤 더해진 참조를 이 셀의 러너로 세지 않는다)
    names: list[str] = []
    model_refs: list[str] = []
    for _, block in _serving_blocks(text):
        for body in _command_texts(block):
            for ref in _CONFIGS_REF.findall(body):
                if "${" in ref or ref == f"{c.cell}.sh":
                    model_refs.append(ref)
                elif ref not in names:
                    names.append(ref)
    rd = c.render()
    asset = Path(rd.SHARED_ASSET_DIR)

    def scan(text: str) -> list[str]:
        new = [s for s in _SOURCED.findall(text) if s not in names and s != f"{c.cell}.sh"]
        for s in new:
            if s not in names:
                names.append(s)
        return new

    # 모델 러너(트리플렛 .sh)가 source 하는 것 — single 은 compose 가 모델 러너를 직접 부른다(serve_runner 없음).
    queue = list(names) + (scan(_read(c.triplet["sh"])) if c.triplet["sh"].is_file() else [])
    while queue:
        n = queue.pop(0)
        if (asset / n).is_file():
            queue += scan(_select_tracked(c, asset / n)["bytes"].decode("utf-8", "replace"))   # 측정 당시 개정의 source
    return names, sorted(set(model_refs))


def _runtime_patch_armed(c: _Ctx) -> tuple[bool, str]:
    """이 셀이 런타임 패치를 **실제로** 무장하는가(F3 · 2026-09-22 S2 round 2). arm_patch.sh 는
    `/app/configs/${CONFIG_FILE}_patch.py` 가 있을 때만 무장한다 — master 는 셀 이름, multi slave 는 compose 기본 CONFIG_FILE
    (slave_forward.NEVER_FORWARD · K11). 둘 다 없으면 arm_patch.sh 는 양 노드에서 no-op 이다."""
    master = (c.out / "configs" / f"{c.cell}_patch.py").is_file()
    slave_cfg = None
    if c.topo == "multi" and c.compose_path is not None and "CONFIG_FILE" in getattr(c.sf, "NEVER_FORWARD", ()):
        slave_cfg = c.sf.compose_default(_compose_file(c), "CONFIG_FILE")
    slave = bool(slave_cfg) and (c.out / "configs" / f"{slave_cfg}_patch.py").is_file()
    if master or slave:
        return True, "armed(" + " · ".join(x for x, on in ((f"master {c.cell}_patch.py", master),
                                                          (f"slave {slave_cfg}_patch.py", slave)) if on) + ")"
    return False, (f"output/{c.topo}/configs/{c.cell}_patch.py 없음"
                   + (f" · slave CONFIG_FILE={slave_cfg!r} 의 _patch.py 없음" if c.topo == "multi" else "")
                   + " — arm_patch.sh 는 no-op")


def _runner_gpu_literal(text: str) -> dict:
    eq = [(i + 1, m.group(1)) for i, ln in enumerate(text.splitlines()) for m in _RUNNER_GPU_EQ.finditer(ln)]
    ray = [(i + 1, m.group(2)) for i, ln in enumerate(text.splitlines()) for m in _RUNNER_GPU_RAY.finditer(ln)]
    vals = sorted({v for _, v in eq + ray})
    return {"value": vals[0] if len(vals) == 1 else (vals or None),
            "lines": sorted({ln for ln, _ in eq + ray}),
            "note": "master 는 ray status 의 GPU 합류가 이 리터럴과 같아질 때까지 기다린다 — 노드당 GPU·노드 수가 다른 "
                    "클러스터에서는 이 줄을 고치지 않으면 대기에서 멈춘다(arch 의 <G>g<N>n 과 숨은 결합)."}


# ── 서브 레시피 (multi) ─────────────────────────────────────────────────────────────────────
def _sub_recipe(c: _Ctx, shapes: dict[str, str], not_shipped: list[dict] | None = None) -> dict:
    """`shapes` = 실린 env 형상(이름 → 텍스트) · `not_shipped` = 측정 뒤 재생성이라 싣지 않은 형상 행(2026-09-22 S2 round 3 —
    per_node_values 는 실린 형상에서만 읽는다: 재생성 형상의 키 집합은 측정 당시 키 집합이 아닐 수 있다). 전달 목록은 측정 당시 compose
    개정(`_compose_file`)에서 파생한다."""
    if c.topo != "multi":
        core.fail("HINT_SUB_RECIPE_NOT_MULTI", "sub_recipe 는 multi 셀에만 성립한다.")
    if c.compose_path is None:
        core.fail("HINT_SUB_RECIPE_COMPOSE_ABSENT", f"output/{c.topo}/docker-compose.yaml 부재 — 전달 목록을 파생할 수 없다.")
    sf = c.sf
    compose_file = _compose_file(c)
    try:
        forwards = sf.derive(c.triplet["env"], c.out / ".env", compose_file, c.out / "envs" / ".env.cluster")
        rows = sf.as_sub_recipe(forwards)
    except SystemExit as e:          # 옛 API 의 die() — 원인을 삼키지 않고 번역
        core.fail("HINT_SUB_RECIPE_DERIVE_FAILED", f"slave_forward.derive 가 거부했다(SystemExit {e.code}).",
                  "stderr 의 [slave-forward] FAIL 사유를 고친다(값에 공백·따옴표 등 — 원격 shell prefix 안전 문자 밖).")
    except (OSError, ValueError) as e:   # ForwardError(ValueError · code) — 안전하지 않은 값·compose 모양·렌더러 부재
        core.fail("HINT_SUB_RECIPE_DERIVE_FAILED",
                  f"slave_forward.derive 거부: {getattr(e, 'code', type(e).__name__)}: {getattr(e, 'message', e)}",
                  "사유 코드의 자리(셀 env 값 · compose 모양 · render_dockerfile.env_tier)를 고친다 — 전달 목록을 손으로 적지 않는다.")
    from . import pii
    env_forward, image_keys, per_node = [], [], {}
    for r in rows:
        key, group = str(r.get("key")), str(r.get("group"))
        if "value_shape" in r:
            shaped = str(r["value_shape"])       # 소유자(slave_forward._value_shape)가 형상화한 값 — 다시 적지 않는다
        elif group == "mount":
            # 옛 API(원값 `value`): 운영자 절대경로(NAS·스테이징 루트)다 — manifest 필드 표지로
            shaped = pii.manifest_placeholder(key.lower())
        else:
            shaped = str(r.get("value") or "")   # 셀 env(트리플렛으로 실린다) — 재현 사실
        if group == "mount":
            per_node[key] = shaped              # 메인 .env 값이 서브에 전달된다 — 같은 마운트 경로가 전제(preconditions)
        env_forward.append({"key": key, "group": group, "from": r.get("from") or r.get("source"),
                            "compose_ref": r.get("compose_ref"), "why": r.get("why"), "value": shaped})
        if group == "image_identity":
            image_keys.append(key)
    cluster_shape = shapes.get(".env.cluster")
    if cluster_shape:
        for ln in cluster_shape.splitlines():
            km = re.match(r"^([A-Za-z_][A-Za-z0-9_]*)=(\S+)\s+#\s*tier=env\s*$", ln)
            if km:
                per_node[km.group(1)] = km.group(2)
    if ".env.interconnect" in shapes:
        per_node["interconnect"] = "artifacts/compose/.env.interconnect.template"
    # K11: slave 는 모델 런타임 패치를 무장하지 않는다 — 코드에서 **관측**한다(단언 ✗).
    slave_cfg = sf.compose_default(compose_file, "CONFIG_FILE") if "CONFIG_FILE" in getattr(sf, "NEVER_FORWARD", ()) \
        else None
    master_patch = (c.out / "configs" / f"{c.cell}_patch.py").is_file()
    slave_patch = bool(slave_cfg) and (c.out / "configs" / f"{slave_cfg}_patch.py").is_file()
    rd = c.render()
    runner_asset = Path(rd.SHARED_ASSET_DIR) / "serve_runner.sh"
    # 2026-09-22 S2 round 3 적대 리뷰: 서빙 경로 파생은 측정 당시 개정으로(러너 이름 · env_file 과 같은 규칙) — 작업트리 러너가 측정 뒤
    #   바뀌었으면 GPU 합류 리터럴도 측정 뒤 값이 된다(실린 serve_runner.sh 와 레시피가 서로 다른 말을 한다).
    gpu = (_runner_gpu_literal(_select_tracked(c, runner_asset)["bytes"].decode("utf-8", "replace")) if runner_asset.is_file()
           else {"value": None, "note": "러너 부재"})
    att = _get(c.ev, "attestation")
    parity: dict = {"axes": ["vllm_sha", "torch", "driver", "build_ledger"], "source": _attestation_ref(c)}
    roles = {"main": "main", "master": "main", "sub": "sub", "slave": "sub"}
    if isinstance(att, dict) and isinstance(att.get("parity"), dict) and att["parity"]:
        # v2(SPEC X12 · 스모크 serve 시점 캡처): 축별 판정과 양 노드 값. 노드 id·컨테이너·image id 는 싣지 않는다
        # (노드 id 는 호스트 이름일 수 있다 — 역할 이름 main/sub 로만 말한다 · image digest 는 판정 축이 아니다).
        obs = {}
        for axis in ("vllm_sha", "torch", "driver"):
            row = att["parity"].get(axis)
            if isinstance(row, dict):
                obs[axis] = {k: row.get(k) for k in ("verdict", "main", "sub")}
        bl = att["parity"].get("build_ledger")
        if isinstance(bl, dict):
            obs["build_ledger"] = {k: bl.get(k) for k in ("verdict", "main_identity_sha256", "sub_identity_sha256",
                                                          "identity_exempt_build_args", "compared")}
        parity["observed"] = obs
        parity["blocking"] = bool(att.get("parity_blocking"))    # D-5: 기록 전용(차단 여부는 사람 결정)
    elif isinstance(att, dict) and isinstance(att.get("checks"), list) and att["checks"]:
        parity["observed"] = [{"axis": str(row.get("axis")), "node": roles.get(str(row.get("node")), "<node>"),
                               "observed": str(row.get("observed")), "expected": str(row.get("expected"))}
                              for row in att["checks"] if isinstance(row, dict)
                              and not isinstance(row.get("observed"), (dict, list))]
    else:
        parity["observed"] = "unobservable"
        parity["reason"] = ("attestation 부재" if not isinstance(att, dict) else
                            "attestation 에 checks·parity 가 비었다(K1: 옛 스모크는 wheel 트랙에서만 축을 채웠다)")
    recipe = {
        "schema_version": 1,
        "role_diff": {
            "master": "ray head + vllm serve(트리플렛 source · 모델 러너 실행)",
            "slave": "ray worker(트리플렛 미수령 — 같은 이미지·같은 compose·같은 serve_runner.sh 가 NODE_ROLE 로 분기)",
            "slave_config_file": slave_cfg,
            "master_runtime_patch": "armed" if master_patch else "none(런타임 패치 파일 없음)",
            "slave_runtime_patch": ("armed" if slave_patch else
                                    f"not-armed — slave CONFIG_FILE={slave_cfg!r}(slave_forward.NEVER_FORWARD) → "
                                    f"arm_patch.sh 가 /app/configs/{slave_cfg}_patch.py 를 찾고 없으면 no-op (K11)"),
            "runner_gpu_join_literal": gpu,
            "source": {"slave_config_file": "compose-default:CONFIG_FILE × slave_forward.NEVER_FORWARD",
                       "runtime_patch": f"output/{c.topo}/configs/<CONFIG_FILE>_patch.py 실재 여부",
                       "runner_gpu_join_literal": "render_dockerfile.SHARED_ASSET_DIR/serve_runner.sh(측정 당시 개정 — "
                                                  "slots.compose.evidence.selected_revisions)"},
        },
        "launch_order": ["build(양 노드 병렬)", "master up(head)", "slave up(join)",
                         "master: GPU 합류 확인 → vllm serve"],
        "launch_order_source": REL_SMOKE_MULTI,
        "env_forward": env_forward,
        "env_forward_source": f"{core.REL_SLAVE_FORWARD} derive() — 스모크와 같은 함수(AC8)",
        "image_identity_args": image_keys,
        "per_node_values": per_node,
        "preconditions": ["가중치가 같은 마운트 경로에 실재(메인 .env 의 mount 값이 서브에 전달된다)",
                          "PLE mmap 이면 slave 로컬 스테이징도 실재", "vLLM SHA · torch · driver 동일(동일 이미지가 아니라 동일 ABI)"],
        "parity_attestation": parity,
    }
    if not_shipped:
        # 2026-09-22 S2 round 3: 싣지 않은 재생성 형상 — 노드별 값 표에서 조용히 빠진 이유를 레시피가 스스로 말한다.
        recipe["env_shapes_not_shipped"] = [{"file": r.get("file"), "reason": POST_MEASUREMENT_REGENERATED,
                                             "replaced_by": f"{ENV_OBSERVED_POINTER}(측정 엔진 로그 관측)"}
                                            for r in not_shipped]
    hits = pii.scan_text(core.dumps(recipe), pii.require_terms(c.repo), profile="deploy")   # 부재 = 차단(위와 같은 규율)
    if hits:
        core.fail("HINT_SUB_RECIPE_PII", "sub_recipe 에 배포 금지 패턴:\n  " + pii.render_hits(hits, 10),
                  "slave_forward 의 해당 그룹 값을 표지로 바꾸는 규칙을 고친다(덧칠 ✗).")
    return recipe


def sub_recipe(repo: Path, ev: Any, *, forward_module: Any | None = None, render_module: Any | None = None) -> dict:
    """multi 서브 레시피(plan §4.6 스키마 · SPEC §5.6). 전달 목록 = slave_forward.derive() — 스모크와 같은 함수."""
    import tempfile
    c = _ctx(repo, ev, forward_module=forward_module, render_module=render_module)
    shapes: dict[str, str] = {}
    skipped: list[dict] = []
    for name in (".env.cluster", ".env.interconnect"):
        p = c.out / "envs" / name
        if not p.is_file():
            continue
        if _regenerated(c, p):
            skipped.append({"file": f"envs/{name}"})      # 측정 뒤 재생성 — collect 와 같은 규칙(싣지 않는다)
            continue
        shapes[name] = _env_shape(c.repo, p, _TIER_ENV_KINDS[name], render_module=c.render(), sf=c.sf,
                                  measured=_measured_utc(c)[0])[0]
    if c.compose_path is None or _select_tracked(c, c.compose_path)["shipped"] == "worktree":
        return _sub_recipe(c, shapes, skipped)
    # 측정 당시 compose 개정이 작업트리와 다르다 — 경로를 받는 소유자 함수에 그 바이트를 임시 파일로 넘긴다(끝나면 지운다).
    with tempfile.TemporaryDirectory(prefix="hint-compose-rev-") as td:
        c.compose_eff = Path(td) / c.compose_path.name
        c.compose_eff.write_bytes(_select_tracked(c, c.compose_path)["bytes"])
        try:
            return _sub_recipe(c, shapes, skipped)
        finally:
            c.compose_eff = None


# ── 값의 지위 후보 ──────────────────────────────────────────────────────────────────────────
def _strip_comment(value: str) -> tuple[str, str | None]:
    """`값  # 주석` → (값, 주석). 따옴표 안의 `#` 는 주석이 아니다 · `#` 앞에는 공백이 있어야 한다(YAML)."""
    quote = None
    for i, ch in enumerate(value):
        if quote:
            if ch == quote:
                quote = None
        elif ch in ("'", '"'):
            quote = ch
        elif ch == "#" and (i == 0 or value[i - 1] in " \t"):
            return value[:i].strip(), value[i + 1:].strip() or None
    return value.strip(), None


def _serving_yaml_knobs(path: Path) -> list[tuple[str, str, str | None]]:
    """서빙 yaml 의 **최상위** 노브(들여쓴 줄은 부모 값의 일부 — 옛 파서는 중첩을 평탄화했다 · 코드맵 hint_collect §3.10)."""
    out: list[tuple[str, str, str | None]] = []
    if not path.is_file():
        return out
    cur: list | None = None
    for raw in _read(path).splitlines():
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        if raw[0] in " \t":
            if cur is not None:
                cur[1] = (cur[1] + " " + raw.strip()).strip()
            continue
        m = re.match(r"^([A-Za-z][A-Za-z0-9_-]*)\s*:(.*)$", raw)
        if not m:
            continue
        val, comment = _strip_comment(m.group(2))
        cur = [m.group(1), val.strip("'\"") if len(val) >= 2 and val[0] == val[-1] and val[0] in "'\"" else val,
               comment]
        out.append(cur)                      # type: ignore[arg-type]
    return [(k, v, cm) for k, v, cm in out]


def value_status_candidates(repo: Path, ev: Any, *, forward_module: Any | None = None) -> list[dict]:
    """서빙 yaml 노브 전부(전송·신원 노브 `VALUE_STATUS_EXCLUDED_KNOBS` 제외 · D-b) × 지위 후보(SPEC §5.6). 후보일 뿐 —
    최종 지위는 01 PROMPT 저작자가 hint-event value-status 로 적는다. 근거가 없으면 후보를 지어내지 않는다(candidate_status=None)."""
    c = _ctx(repo, ev, forward_module=forward_module)
    lockset = _get(c.ev, "lockset") or {}
    lockset = lockset if isinstance(lockset, dict) else {}
    by_knob = {knob: key for key, knob in LOCKSET_SOURCE_KNOB.items()}
    rows = []
    for knob, value, comment in _serving_yaml_knobs(c.triplet["yaml"]):
        if knob.replace("_", "-") in VALUE_STATUS_EXCLUDED_KNOBS:
            continue
        status = reason = note = None
        source = "serving-yaml"
        skey = by_knob.get(knob)
        sval = lockset.get(skey) if skey else None
        if isinstance(sval, str) and sval:
            status = _SOURCE_STATUS.get(sval)
            source = f"lockset.{skey}={sval}"
            reason = (f"lockset 출처 {skey}={sval}" + ("" if status else " — 어휘 밖(후보 없음)")
                      + (f" · lockset provenance={lockset.get('provenance')}" if lockset.get("provenance") else ""))
        if status is None and comment:
            for word, st in _COMMENT_STATUS:
                if word in comment:
                    status, source, reason = st, "serving-yaml 줄 주석", f"주석 단어 '{word}': {comment}"
                    if st == "declared-requirement":
                        # 2026-09-22 S2 round 3: 주석 단어만으로는 요구가 **관측**되지 않았다 — inherited 로 제안하고 문구를 단다
                        #   (lockset 출처 declared-requirement·target_gmu 는 기록된 출처라 위 분기에서 그대로 declared-requirement).
                        status, note = "inherited", COMMENT_ONLY_REQUIREMENT_NOTE
                        reason = f"{reason} · {note}"
                    break
        if status is None and value.lower() == "auto":
            status, source, reason = "engine-default", "serving-yaml 값 토큰", "값 `auto` = 엔진이 고른다"
        row = {"knob": knob, "value": value, "candidate_status": status,
               "reason": reason or "근거 없음 — 저작자가 판정한다(후보를 지어내지 않는다)",
               "source": source, "comment": comment}
        if note:
            row["note"] = note
        rows.append(row)
    return rows


# ── 재현 절차 · 관측 소요 ────────────────────────────────────────────────────────────────────
def _events_for(repo: Path, label: str) -> list[dict]:
    """블랙박스 이벤트(`docs/logs/<node>/events/<YYYY-MM>.jsonl`) 중 label 이 같은 것. 비추적 기계 평면 · 읽기만."""
    base = repo / "docs" / "logs"
    out: list[dict] = []
    if not base.is_dir():
        return out
    for f in sorted(base.glob("*/events/*.jsonl")):
        if not re.fullmatch(r"\d{4}-\d{2}\.jsonl", f.name):
            continue
        for ln in f.read_text(encoding="utf-8", errors="replace").splitlines():
            try:
                row = json.loads(ln)
            except ValueError:
                continue
            if isinstance(row, dict) and row.get("label") == label and isinstance(row.get("ts"), str):
                out.append({**row, "_file": f.relative_to(repo).as_posix()})
    return sorted(out, key=lambda r: r["ts"])


def _first_measure_utc(repo: Path, ev: Any) -> tuple[str | None, str | None]:
    """스윕 디렉터리의 측정 JSON `date`(vllm bench · 컨테이너 시계 = UTC 로 읽는다) 중 가장 이른 것."""
    sweep = _get(ev, "sweep") or {}
    sdir = sweep.get("dir") if isinstance(sweep, dict) else None
    if not isinstance(sdir, str) or not (repo / sdir).is_dir():
        return None, None
    best = None
    cell = _get(ev, "cell")
    for f in sorted((repo / sdir).rglob("*.json")):
        if isinstance(cell, str) and not core.level_raw_is_measured_tool(f, cell):
            continue    # 도구를 바꿔 재스윕한 자리에 남은 옛 도구 원시 — 이 측정의 첫 측정이 아니다(2026-09-23 D1)
        doc = _json_file(f)
        m = _LOCAL_DATE.match(str((doc or {}).get("date") or ""))
        if m:
            utc = "{}-{}-{}T{}:{}:{}Z".format(*m.groups())
            if best is None or utc < best[0]:
                best = (utc, f.relative_to(repo).as_posix())
    return best if best else (None, None)


def _phase_span(repo: Path, ev: Any, node: str, phase: str, cell: str) -> tuple[str | None, str | None, str | None]:
    """캠페인 phase 상태의 (시작, 끝, 출처) — 명시 id 만 · ACTIVE 는 읽지 않는다(2026-09-07 Q6). 셀이 다르면 쓰지 않는다
    (`started_utc` 는 셀마다 덮인다 — campaign_init writer_set_phase 주석). `cluster` 셀은 양 노드 디렉터리를 모두 보고
    가장 이른 시작 ~ 가장 늦은 끝을 쓴다(양 노드 병렬 빌드)."""
    # phases/<X> 의 X 는 **노드 id**(campaign.yaml nodes[].node_id)이지 축 이름(main|sub|cluster)이 아니다 — 옛 판은
    # `phases/<축>` 을 열어 노드 id 가 역할 이름과 다른 캠페인에서 조용히 미관측이 됐다(2026-09-22 감사). 축 → 노드 id 해소는
    # evidence 가 이미 했다(`ev.phases[<phase>]` · 배정 노드). cluster 는 선언된 노드 전부의 같은 셀 기록으로 span 을 잡는다.
    cid = _get(ev, "campaign_id")
    if not cid or not node:
        return None, None, None
    own = (_get(ev, "phases") or {}).get(phase) if isinstance(_get(ev, "phases"), dict) else None
    if node != "cluster":
        if isinstance(own, dict) and own.get("started_utc") and own.get("ended_utc"):
            return own["started_utc"], own["ended_utc"], str(own.get("_path") or f"evidence.phases.{phase}")
        return None, None, None
    decl = _get(ev, "declaration") or {}
    ids = sorted({str(n.get("node_id")) for n in (decl.get("nodes") or []) if isinstance(n, dict) and n.get("node_id")}
                 if isinstance(decl, dict) else set())
    base = repo / "campaigns" / str(cid) / "phases"
    docs = []
    for nid in ids:
        if "/" in nid or nid.startswith("."):
            continue
        d = _json_file(base / nid / f"{phase}.status.json")
        if isinstance(d, dict) and d.get("cell_id") == cell and d.get("started_utc") and d.get("ended_utc"):
            docs.append(d)
    if not docs:
        if isinstance(own, dict) and own.get("started_utc") and own.get("ended_utc"):
            return own["started_utc"], own["ended_utc"], str(own.get("_path") or f"evidence.phases.{phase}")
        return None, None, None
    return (min(d["started_utc"] for d in docs), max(d["ended_utc"] for d in docs),
            f"campaigns/{cid}/phases/<선언 노드 {len(docs)}>/{phase}.status.json(같은 셀 · 가장 이른 시작 ~ 가장 늦은 끝)")


def _compose_build_context(c: _Ctx) -> tuple[str | None, str]:
    """compose `build.context`(compose 디렉터리 기준) → 저장소 상대 경로와 출처. 변수·저장소 밖 = None(추측 ✗).
    빌드 평면이라 작업트리 compose 를 읽는다(2026-09-22 S2 round 3 기록: 측정 시각 기준 compose 개정은 **서빙** 파생용이다 — 빌드는
    측정보다 앞서며, 이미지를 지은 입력의 개정은 Dockerfile 선택(`_select_dockerfile` · 이미지 Created 기준)이 가진다)."""
    if c.compose_path is None:
        return None, "compose 부재"
    m = re.search(r"(?m)^([ \t]*)build:[ \t]*\n((?:\1[ \t]+\S.*\n?)+)", _read(c.compose_path))
    cm = re.search(r"(?m)^\s*context:\s*(\S+)", m.group(2)) if m else None
    raw = cm.group(1).strip("'\"") if cm else ""
    if not raw or "$" in raw:
        return None, "compose build.context 미해석"
    try:
        rel = (c.compose_path.parent / raw).resolve().relative_to(c.repo).as_posix() or "."
    except ValueError:
        return None, "compose build.context 가 저장소 밖"
    return rel, f"{core.rel(c.repo, c.compose_path)} build.context={raw}"


def _build_command(c: _Ctx, sel: dict) -> tuple[str, str]:
    """(명령, 명령 출처) — 독립 실행 가능한 `docker build -f <Dockerfile> --build-arg K=V … -t <tag> <context>`(F5 · 2026-09-22
    S2 round 2). 빌드 인자 = 원장 `build_args`(있으면) → 측정 이미지 docker history 의 `RUN |N` 인자(재구성) · 컨텍스트 = compose
    `build.context`. 옛 판은 스모크 사용법(`--build-only`)만 적어 수신자가 인자를 스모크 코드에서 역추적해야 했다."""
    import shlex
    app = c.memo.get(("applied", True)) or c.memo.get(("applied", False))
    args: dict = {}
    asrc = None
    if app:
        applied, _, ledger, _ = app
        if isinstance(ledger, dict) and isinstance(ledger.get("build_args"), dict):
            args = {k: v for k, v in ledger["build_args"].items() if isinstance(v, str)}
            asrc = "build-ledger build_args"
        elif isinstance((applied or {}).get("reconstruction"), dict):
            args = dict(applied["reconstruction"].get("build_args") or {})
            asrc = "docker-history build-args"
    ctx, _csrc = _compose_build_context(c)
    context = ctx or core.rel(c.repo, c.out)
    df = sel["dockerfile"] if context == "." else f"{context}/{sel['dockerfile']}"
    parts = [f"docker build -f {shlex.quote(df)}"] + [f"  --build-arg {shlex.quote(f'{k}={v}')}" for k, v in sorted(args.items())]
    if sel.get("image_tag"):
        parts.append(f"  -t {shlex.quote(str(sel['image_tag']))}")
    parts.append(f"  {shlex.quote(context)}")
    shipped = (c.memo.get(("dockerfile", sel["dockerfile"])) or {}).get("shipped") or "worktree"
    head = [f"# 실린 artifacts/build_recipe/{sel['dockerfile']}(이미지를 지은 개정: {shipped})를 {df} 자리에 두고 컨텍스트에서 짓는다."]
    if c.topo == "multi":
        head.append("# 양 노드에서 각각 짓는다(이미지 save/load 전송 ✗ — 노드마다 자기 이미지 · 요건은 동일 digest 가 아니라 동일 ABI).")
    tail = ([f"# 동등 경로(스모크 사용법): bash {REL_SMOKE_MULTI} {c.cell} --build-only"] if c.topo == "multi" else
            [f"# 동등 경로(compose): docker compose -f output/{c.topo}/docker-compose.yaml --env-file "
             f"output/{c.topo}/envs/.env.{c.cell} build"])
    src = (f"reconstructed({asrc} + compose build)" if asrc and args else "reconstructed(compose build · build-arg 미관측)")
    if ctx is None:
        src += f"(compose build.context 미해석 — {context} 로 둔다)"
    return "\n".join(head + [" \\\n".join(parts)] + tail), src


def _bench_commands(c: _Ctx) -> tuple[str, str]:
    """(명령, 명령 출처) — 셀 스윕의 bench JSON(`vllm bench serve --save-result` 산출) 각각을 그 **필드만으로** 복원한 명령
    (F5 · 옛 판은 "03-benchmark.md §3.x" 자리표시였다). JSON 에 없는 인자는 지어내지 않고 이름만 적는다. 균일 요청 길이면
    random 합성 데이터셋으로 읽는다(추정 — 머리 주석에 밝힌다)."""
    import shlex
    sweep = _get(c.ev, "sweep") or {}
    sdir = sweep.get("dir") if isinstance(sweep, dict) else None
    # 같은 사실의 다른 생산자(evidence.bench_command)와 **같은 묶음 규칙**을 쓴다(2026-09-22 S2 round 2 리뷰): 스윕이 출력 평면에
    #   묶이지 않았으면(simlog 사본 등) evidence 는 "레벨 원시가 이 측정의 것이 아니다" 로 재구성을 거부한다. hint.py 는 이쪽이
    #   `reconstructed…` 를 내면 evidence 쪽을 덮지 않으므로, 여기서 따로 재구성하면 evidence 의 거부가 조용히 뒤집힌다.
    binding = sweep.get("binding") if isinstance(sweep, dict) else None
    if binding is not None and binding != "output":
        return "", f"unobserved(스윕 binding={binding!r} — 출력 평면에 묶인 스윕만 재구성한다 · evidence.bench_command 와 같은 규칙)"
    rows = []
    if isinstance(sdir, str) and (c.repo / sdir).is_dir():
        for f in sorted((c.repo / sdir).rglob("*.json")):
            if not core.level_raw_is_measured_tool(f, c.cell):
                continue    # 옛 도구 원시(2026-09-23 D1) — 이 측정의 레벨 명령이 아니다
            d = _json_file(f)
            if not d or not all(k in d for k in _BENCH_JSON_KEYS):
                continue
            m = _LOCAL_DATE.match(str(d.get("date") or ""))
            rows.append(("".join(m.groups()) if m else "~", f.relative_to(c.repo).as_posix(), d))
    if not rows:
        return "", "unobserved(스윕 디렉터리에 vllm bench serve JSON 없음)"
    rows.sort()
    lines = ["# bench JSON 필드로 복원: backend · model_id→--model · tokenizer_id→--tokenizer · num_prompts · max_concurrency · "
             "request_rate · burstiness · 요청당 입출력 길이 = total_*_tokens ÷ completed(나누어떨어질 때만 — 평균이다 · 요청별 길이는 "
             "JSON 에 없다) · 나누어떨어지면 `--dataset-name random` 으로 추정했다(측정 도구가 그 꼴로 부른다 — 아래 도구 원문).",
             # 2026-09-22 S2 round 2 리뷰: 옛 문구는 "균일할 때만" 이라 했지만 검사는 나눗셈뿐이다(균일은 관측 아님) · lite 두 JSON 은
             #   run_bench.sh 가 아니라 lite_bench.sh 산출이다(도구를 하나로 적으면 수신자가 lite 인자를 잘못 찾는다).
             "# JSON 에 없는 인자(--endpoint · --base-url · --ignore-eos · --num-warmups · --temperature · --seed · --random-range-ratio · "
             "--trust-remote-code)는 측정 도구가 정했다 — level_* = run_bench.sh · lite_cold/lite_warm = lite_bench.sh"
             "(.claude/skills/adversarial-benchmark/scripts/) · 셀 스윕 순서는 sweep_bench.sh."]
    for _, rel, d in rows:
        flags = ["vllm bench serve", f"--backend {shlex.quote(str(d['backend']))}"]
        for key, flag in (("model_id", "--model"), ("tokenizer_id", "--tokenizer")):
            if d.get(key):
                flags.append(f"{flag} {shlex.quote(str(d[key]))}")
        n, ti, to = d.get("completed"), d.get("total_input_tokens"), d.get("total_output_tokens")
        uniform = all(isinstance(x, int) and not isinstance(x, bool) for x in (n, ti, to)) and n > 0 \
            and ti % n == 0 and to % n == 0
        if uniform:
            flags += ["--dataset-name random", f"--random-input-len {ti // n}", f"--random-output-len {to // n}"]
        flags += [f"--num-prompts {d['num_prompts']}", f"--max-concurrency {d['max_concurrency']}"]
        if d.get("request_rate") is not None:
            flags.append(f"--request-rate {shlex.quote(str(d['request_rate']))}")
        if d.get("burstiness") not in (None, 1, 1.0):
            flags.append(f"--burstiness {d['burstiness']}")
        lines.append(f"# {rel} (date {d.get('date')})" + ("" if uniform else " · total 토큰이 completed 로 나누어떨어지지 않는다(요청 길이 불균일) — 데이터셋 인자 복원 ✗"))
        lines.append(" ".join(flags))
    return "\n".join(lines), "reconstructed(bench json)"


def _reproduce(c: _Ctx, sel: dict | None, plane: str) -> list[dict]:
    repo, cell, topo = c.repo, c.cell, c.topo
    steps: list[dict] = []

    def step(name, command, success, start=None, end=None, bound="none", source="미관측", command_source="derived",
             extra=None):
        secs = _secs_between(start, end)
        steps.append({"order": len(steps) + 1, "step": name, "phase": name, "command": command,
                      "command_source": command_source,
                      "observed": {"start_utc": start, "end_utc": end, "seconds": secs, "bound": bound if secs is not None
                                   else "none", **(extra or {})},
                      "duration": _fmt_duration(secs, bound) if secs is not None else
                      ("미관측" if not (start or end) else f"미관측(관측 시각: {start or end})"),
                      "success": success, "source": source})

    rd_rel = core.REL_RENDER_DOCKERFILE
    measure_start, measure_src = _first_measure_utc(repo, c.ev)
    sweep = _get(c.ev, "sweep") or {}
    sweep_end = sweep.get("generated_utc") if isinstance(sweep, dict) else None
    # 측정 JSON 의 `date` 는 시간대 표기가 없다 — 컨테이너 시계(UTC)로 읽는 것은 **가정**이다. 가정이 깨졌으면(첫 측정이 스윕
    # 종료 generated_utc 보다 뒤) 그 시각으로 소요를 계산하지 않는다(합성 ✗ · 미관측으로 적는다).
    if measure_start and isinstance(sweep_end, str) and measure_start > sweep_end:
        measure_start, measure_src = None, f"{measure_src}(date 를 UTC 로 읽으면 generated_utc 뒤 — 시계 가정 불성립 · 버림)"
    if measure_src and measure_start:
        measure_src = f"{measure_src} · date=컨테이너 시계(UTC 가정)"
    if plane == "docker":
        if topo == "multi":
            # 2026-09-22 S2 round 3 적대 리뷰: 렌더러가 측정 뒤 바뀌었으면(git 관측) 명령 앞에 그 사실 · 측정 전 개정을 주석으로 단다.
            rrev = _renderer_revision(c)
            step("render", "\n".join(_renderer_drift_lines(rrev) + [
                f"python3 {rd_rel} --materialize-configs --topology multi",
                f"python3 {rd_rel} --materialize-env --topology multi --manifest output/multi/manifest.yaml",
                f"python3 {rd_rel} --cluster-envfile --topology multi --manifest output/multi/manifest.yaml "
                "-o output/multi/envs/.env.cluster",
                f"python3 {rd_rel} --nccl-envfile --manifest output/multi/manifest.yaml -o output/multi/envs/.env.interconnect"]),
                "러너 3종·.env·.env.cluster·.env.interconnect 생성(렌더러 fail-loud)",
                source=f"{REL_SMOKE_MULTI} 선행조건 안내 · {rd_rel} CLI", command_source="derived(렌더러 CLI)")
            if rrev is not None:
                steps[-1]["renderer_revision"] = _tracked_row(rrev)      # 판정 근거(측정 전 커밋 · 측정 뒤 커밋 · 방법) — 바이트 ✗
            serve_cmd = f"bash {REL_SMOKE_MULTI} {cell} --keep-up"
        else:
            serve_cmd = f"bash {REL_SMOKE_SINGLE} {cell}"
        build_cmd, build_src = _build_command(c, sel) if sel else ("", "unobserved(빌드 선택자 없음)")
        p_start, p_end, p_src = _phase_span(repo, c.ev, c.node, "build", cell)
        created = _image_created(c)
        # 이미지 층 시각(관측): 우리 Dockerfile 층의 가장 오래된 ~ 가장 새 층. 캐시 재사용 층은 **이전 빌드의 시각**을 가진다 —
        #   소요가 아니다(상한도 하한도 아님). 수신자가 "마지막으로 다시 지어진 구간" 을 스스로 읽을 수 있게 싣는다.
        span = (c.memo.get(("dockerfile", sel["dockerfile"])) or {}).get("layer_span") if sel else None
        extra = {"layer_span": span} if span else None
        if p_start and p_end:
            step("build", build_cmd, "빌드 OK · 이미지 실재", p_start, p_end, "exact", p_src, build_src, extra)
        elif span and span.get("oldest_utc") and span.get("newest_utc"):
            # 2026-09-22 S2 round 3(공유 계약): 캠페인 phase 기록이 없으면 관측 구간 = docker history 층 CreatedAt 첫 층 → 끝 층(우리
            #   Dockerfile 층). 한계 `layer-window` — 캐시 재사용 층은 이전 빌드의 시각이라 창이지 소요가 아니다(상한·하한 어느 쪽도 아님).
            step("build", build_cmd, "빌드 OK · 이미지 실재", span["oldest_utc"], span["newest_utc"], LAYER_WINDOW,
                 f"{span.get('source')} · 우리 Dockerfile 층 {span.get('layers')}개의 CreatedAt 첫 층 → 끝 층(캐시 재사용 층은 이전 빌드 "
                 "시각 — 빌드 소요 아님)", build_src, extra)
        else:
            step("build", build_cmd, "빌드 OK · 이미지 실재", None, created, "none",
                 f"docker image inspect {_image_ref(c)[0]} .Created(종료만 관측)" if created else "미관측", build_src, extra)
        events = [e for e in _events_for(repo, f"smoke-{cell}") if e.get("kind") == "budget_declare"]
        if measure_start:
            events = [e for e in events if e["ts"] <= measure_start]
        decl = events[-1] if events and measure_start else None     # 첫 측정 시각이 없으면 어느 선언인지 가를 수 없다
        step("serve", serve_cmd, "health 200 + 추론 1회(스모크 판정)", decl["ts"] if decl else None, measure_start,
             "upper" if decl and measure_start else "none",
             (f"{decl['_file']} budget_declare(label smoke-{cell}) → 첫 측정 date({measure_src})"
              if decl and measure_start else "미관측(예산 선언 또는 첫 측정 시각 없음)"),
             "derived(스모크 사용법)")
    else:
        step("install", "artifacts/build_recipe/native-install.sh + pip-freeze-main.txt + pip-freeze-sub.txt",
             "venv 에서 `python -c 'import vllm'` 성공", source="native producer preserved build_recipe", command_source="shipped-file")
        step("serve", f"bash output/{topo}/configs/{cell}.sh(native --plane native)",
             "health 200 + 추론 1회 + cleanup attestation PASS")
    bench_cmd, bench_src = _bench_commands(c)
    step("bench", bench_cmd, "리포트 발행(+PASS 면 인증서)",
         measure_start, sweep_end, "exact" if measure_start and sweep_end else "none",
         f"{measure_src} date → sweep_index.generated_utc" if measure_start and sweep_end else "미관측", bench_src)
    return steps


def reproduce_steps(repo: Path, ev: Any, *, runner: DockerRunner | None = None,
                    forward_module: Any | None = None) -> list[dict]:
    """명령 순서 + 단계별 **관측** 소요(SPEC §5.6). 관측 불가는 "미관측" 으로 적는다(합성 ✗). 이미지 탐침은 부르지 않는다
    (재현 명령에는 빌드 인자 · 레시피 개정만 필요하다). 원장 관측 ②는 실행기가 `image_probe` 를 선언하면 시작하지 않는 컨테이너
    create/cp/rm 1회를 쓴다(`_applied` 참조 · 2026-09-22 리뷰 정정)."""
    c = _ctx(repo, ev, runner=runner, forward_module=forward_module)
    plane = _plane(c)[0]
    sel = _applied(c, probe=False)[1] if plane == "docker" else None
    return _reproduce(c, sel, plane)


# ── 수집 ────────────────────────────────────────────────────────────────────────────────────
def _copy(payload: Path, source: Path, slot: str, name: str | None = None) -> str:
    if not source.is_file():
        core.fail("HINT_ARTIFACT_ABSENT", f"복사할 아티팩트가 없다: {source.name}")
    dest = payload / "artifacts" / slot / (name or source.name)
    if dest.exists():
        # 같은 슬롯에서 basename 이 겹치면 조용히 덮였다(코드맵 hint_collect §9 PLAUSIBLE) — 소리내어 막는다.
        core.fail("HINT_ARTIFACT_NAME_COLLISION", f"슬롯 {slot} 에 같은 이름이 두 번: {dest.name}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, dest)
    shutil.copymode(source, dest)
    return dest.relative_to(payload).as_posix()


def _write(payload: Path, slot: str, name: str, text: str) -> str:
    dest = payload / "artifacts" / slot / name
    if dest.exists():
        core.fail("HINT_ARTIFACT_NAME_COLLISION", f"슬롯 {slot} 에 같은 이름이 두 번: {name}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(text, encoding="utf-8")
    return dest.relative_to(payload).as_posix()


def _write_bytes(payload: Path, slot: str, name: str, data: bytes, mode: int | None = None) -> str:
    """관측한 바이트(git 개정 · 이미지에 구워진 스크립트)를 그대로 싣는다 — 작업트리 사본이 아니다(F1·F12 · 2026-09-22)."""
    dest = payload / "artifacts" / slot / name
    if dest.exists():
        core.fail("HINT_ARTIFACT_NAME_COLLISION", f"슬롯 {slot} 에 같은 이름이 두 번: {name}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(data)
    if mode is not None:
        dest.chmod(mode & 0o777)
    return dest.relative_to(payload).as_posix()


_EXEMPT_LITERAL = re.compile(r"^(shell-default \S+?:\d+: [a-z0-9-]+):.*$")


def _require_registered(repo: Path, missing: list[str]) -> None:
    """결손 코드는 evidence.MISSING_CODES 사전에서만(SPEC §2.1). 미등재 코드를 그대로 돌려주면 00 결손 표가 "미등록 사유코드"
    한 줄로 조용히 배포한다 — evidence `_assemble` 과 같은 tripwire 로 **소리 내어** 막는다(등재는 evidence 소유자 몫)."""
    try:
        from . import evidence
    except (ImportError, SyntaxError) as e:
        core.fail("HINT_OWNER_MODULE_MISSING", f"hintlib.evidence 적재 실패(결손 코드 사전): {e}")
    codes = getattr(evidence, "MISSING_CODES", None)
    if not isinstance(codes, dict):
        core.fail("HINT_OWNER_MODULE_MISSING", "hintlib.evidence.MISSING_CODES 가 dict 가 아니다(결손 코드 사전의 소유자).")
    unknown = sorted({m for m in missing if m not in codes})
    if unknown:
        core.fail("HINT_MISSING_CODE_UNREGISTERED", f"artifacts 가 등재되지 않은 결손 코드를 냈다: {unknown}",
                  "evidence.MISSING_CODES 에 뜻과 함께 등재한다(tripwire — 조용히 '미등록' 으로 배포되지 않게).")


def _exempt_record(rec: str) -> str:
    """PII 면제 기록에서 **매치 원문**을 뗀다(`… abs-op-path:<원문 경로>` → `… abs-op-path`). 면제 목록은 호출부가
    PAYLOAD·보고에 실을 수 있고, 원문이 실리면 그 자리가 배포 스캔에 다시 걸린다(면제는 compose 의 `${VAR:-…}` 문맥에서만
    성립한다 — 문맥을 떠난 원문은 그냥 운영자 경로 모양이다)."""
    m = _EXEMPT_LITERAL.match(rec)
    return m.group(1) if m else rec


def slot_confidence(row: dict) -> str:
    """3신호 = 파일 × 적용 증거(원장 · 재구성 · `evidence.observed` 관측) × 선언(01 사유 · 저작자). evidence.kind 어휘는
    SPEC §2.1 의 build-ledger|reconstruction|file|none 만 쓴다. 수집 시점엔 선언이 없으므로 최대 2-signal 이고,
    저작 뒤 `rationale` 이 채워지면 호출부가 이 함수로 다시 센다. 옛 `confidence_of` 는 수집 시점 문자열을 덮어써 버렸다."""
    ev = row.get("evidence") or {}
    n = int(bool(row.get("files"))) + int(ev.get("kind") in ("build-ledger", "reconstruction") or bool(ev.get("observed"))) \
        + int(bool(row.get("rationale")))
    return f"{max(n, 1)}-signal"


def _slot(files: list[str], evidence: dict, excluded: list[dict] | None = None) -> dict:
    row = {"files": sorted(files), "applicable": bool(files), "rationale": None, "evidence": evidence}
    if excluded:
        row["excluded"] = excluded
    row["confidence"] = slot_confidence(row)
    return row


def collect(repo: Path, ev: Any, payload_dir: Path, *, forward_module: Any | None = None,
            render_module: Any | None = None, runner: DockerRunner | None = None) -> dict:
    """`payload_dir/artifacts/` 를 만들고 PAYLOAD.slots · applied_set 사실을 돌려준다(SPEC §5.6).
    반환 {"plane","plane_source","track","build","slots","applied_set","build_ledger","missing","files","sub_recipe",
    "value_status_candidates","reproduce_steps","env_shapes","pii_exempt"}. 실패하면 이번 호출이 만든 artifacts/ 를 지운다."""
    c = _ctx(repo, ev, runner=runner, forward_module=forward_module, render_module=render_module)
    made_art = False
    # 출력 경로는 입력 증거보다 먼저 신뢰 경계를 친다. resolve()부터 하면 payload/artifacts가 외부 빈
    # 디렉터리를 가리키는 symlink여도 정상 디렉터리처럼 보이고, 이후 write가 draft 밖으로 샌다.
    payload_raw = Path(payload_dir)
    if payload_raw.is_symlink():
        core.fail("HINT_ARTIFACTS_OUTPUT_SYMLINK", f"payload 출력 루트가 symlink다: {payload_raw}")
    payload = payload_raw.resolve()
    art = payload / "artifacts"
    if os.path.lexists(art) and art.is_symlink():
        core.fail("HINT_ARTIFACTS_OUTPUT_SYMLINK", f"artifacts 출력 루트가 symlink다: {art}")
    if art.exists() and not art.is_dir():
        core.fail("HINT_ARTIFACTS_OUTPUT_NOT_DIR", f"artifacts 출력 루트가 디렉터리가 아니다: {art}")
    if art.exists() and any(art.iterdir()):
        # 이전 실행의 잔재(예: 이제는 skip 판정인 패치)가 남으면 files[] 밖의 파일이 트리에 섞인다 — 덮지 않고 멈춘다.
        core.fail("HINT_ARTIFACTS_DIR_NOT_EMPTY", f"artifacts/ 가 비어 있지 않다: {art}",
                  "새 draft 디렉터리에서 publish 한다(기존 저작물을 덮지 않는다).")
    if not art.exists():
        art.mkdir(parents=True)
        made_art = True
    try:
        return _collect(c, payload, art)
    except BaseException:
        # 시작 시 비어 있고 symlink가 아님을 확인했으므로 artifacts/ 의 모든 것은 이번 호출의 산출물이다.
        # cleanup 직전에도 symlink로 바뀌지 않았는지 확인한다(외부 target 재귀 삭제 금지).
        if made_art and os.path.lexists(art) and not art.is_symlink():
            shutil.rmtree(art, ignore_errors=True)
        raise


def _collect(c: _Ctx, payload: Path, art: Path) -> dict:
    # triplet — 면제 불가(서빙이 구조적으로 요구한다 · 선언 하나로 안전 게이트를 무력화하지 못한다). 평면 파생보다 먼저 본다 —
    # 셀 env 가 없으면 평면 신호도 없어서, 순서를 뒤집으면 진짜 원인(트리플렛 부재) 대신 "평면 파생 불가" 를 말하게 된다.
    absent = [k for k in ("yaml", "sh", "env") if not c.triplet[k].is_file()]
    if absent:
        core.fail("HINT_TRIPLET_ABSENT", f"면제 불가 트리플렛 부재: {absent} (셀 {c.cell})",
                  "트리플렛은 서빙 직후의 작업트리에만 있다 — 셀 서빙 산출물을 복원하거나 발행 대상을 확인한다.")
    plane, plane_source = _plane(c)
    missing: list[str] = []
    files: list[str] = []
    slots: dict[str, dict] = {}
    trip_files = [_copy(payload, c.triplet[k], "triplet") for k in ("yaml", "sh", "env")]
    files += trip_files
    sweep = _get(c.ev, "sweep") or {}
    sidx = sweep.get("index") if isinstance(sweep, dict) else None
    served = isinstance(sidx, dict) and sidx.get("config") == c.cell
    trip_ev = {"kind": "file", "ref": core.rel(c.repo, c.triplet["yaml"]),
               "observed": f"{sweep.get('dir')}/sweep_index.json#config" if served else None,
               "note": "측정이 이 셀 config 로 돌았다(스윕 색인)" if served else "측정 대조 없음"}
    # F2(2026-09-22 S2 round 2): 트리플렛은 면제 불가라 원문 그대로 싣지만, 측정 **뒤** 다시 쓰였으면 그 사실을 적는다(가리지 않는다 —
    #   트리플렛을 비우면 서빙이 불가능하다). 수신자는 이 목록으로 "실린 값 = 측정 당시 값" 인지를 안다.
    measured, _ = _measured_utc(c)
    trip_regen = [core.rel(c.repo, c.triplet[k]) for k in ("yaml", "sh", "env") if _regenerated(c, c.triplet[k])]
    trip_ev["post_measurement_regenerated"] = trip_regen
    trip_ev["measured_utc"] = measured
    if trip_regen:
        trip_ev["note"] += f" · ⚠ 측정({measured}) 뒤 mtime: {trip_regen} — 실린 값이 측정 당시 값과 같다는 보장 없음"
    slots["triplet"] = _slot(trip_files, trip_ev)

    # runtime patch — arming 로그 미배선(plan Q1) → 파일 신호만
    rp = c.out / "configs" / f"{c.cell}_patch.py"
    rp_files = [_copy(payload, rp, "runtime_patch")] if rp.is_file() else []
    files += rp_files
    slots["runtime_patch"] = _slot(rp_files, {"kind": "none", "ref": core.rel(c.repo, rp),
                                              "note": "arming 실증 로그 미배선(plan Q1)" +
                                              (" · multi slave 는 무장하지 않는다(K11 · sub_recipe.role_diff)"
                                               if c.topo == "multi" and rp_files else "")})

    applied = sel = ledger = None
    if plane == "docker":
        applied, sel, ledger, _ = _applied(c)
        if applied["status"] == "unobservable":
            missing.append("HINT_MISSING_BUILD_LEDGER")
        ship = c.memo.get("ship_patch") or {}
        for slot, ph in (("build_patch_pre", "pre"), ("build_patch_post", "post")):
            pf = []
            for row in applied["patches"]:
                if row["phase"] != ph or row["result"] == "skipped":
                    continue
                src = c.out / PATCH_DIRS[ph] / row["file"]
                baked = ship.get((ph, row["file"]))
                if baked is not None:
                    # 이미지에 구워진 바이트가 작업트리와 다르거나(또는 작업트리에 없다) — 이 빌드가 실행한 것은 이미지 바이트다(F12).
                    pf.append(_write_bytes(payload, slot, row["file"], baked[0], baked[1]))
                    continue
                if not src.is_file():
                    missing.append("HINT_MISSING_APPLIED_PATCH_FILE")
                    row["file_absent"] = True
                    continue
                want = row.get("script_sha256")
                if want and core.sha256_file(src) != want:
                    # 원장이 적용했다고 말한 바이트와 실을 바이트가 다르다 = 3신호 모순(양성 검출 → 차단)
                    core.fail("HINT_APPLIED_PATCH_DRIFT", f"{PATCH_DIRS[ph]}/{row['file']} 가 원장 script_sha256 과 다르다.",
                              "이미지를 지은 그 스크립트로 되돌리거나 이미지를 다시 짓는다(다른 스크립트를 재현지침으로 배포 ✗).")
                pf.append(_copy(payload, src, slot))
            files += pf
            obs = applied["status"] == "observed"
            shipped_rows = [r for r in applied["patches"] if r["phase"] == ph and r["result"] != "skipped"]
            decided = [r for r in shipped_rows if r["result"] == "applied"]
            undecided = sorted(r["file"] for r in shipped_rows if r["result"] != "applied")
            if obs:
                kind, note = "build-ledger", "원장 관측"
            else:
                # 라벨된 재구성(F12) — 실린 파일 전부가 applied 로 갈렸을 때만 적용 증거로 센다(하나라도 미관측이면 증거 ✗).
                kind = "reconstruction" if shipped_rows and not undecided else "none"
                by_src: dict[str, int] = {}
                for r in decided:
                    by_src[r["result_source"]] = by_src.get(r["result_source"], 0) + 1
                note = ("원장 부재 — 라벨된 재구성: applied " + str(len(decided))
                        + (f"({' · '.join(f'{k} {v}' for k, v in sorted(by_src.items()))})" if by_src else "")
                        + (f" · 적용 미관측 {undecided}" if undecided else "") + " · skip 판정 = docker-history+gate")
            slots[slot] = _slot(pf, {"kind": kind, "ref": applied["source"], "note": note},
                                [x for x in applied["excluded"] if x["phase"] == ph]
                                + [{"phase": ph, "file": r["file"], "why": f"skipped({r.get('reason')})"}
                                   for r in applied["patches"] if r["phase"] == ph and r["result"] == "skipped"])
        # build recipe = **이미지를 지은** Dockerfile 개정 + 그것이 COPY 하는 파일(패치 디렉터리는 위 슬롯이 싣는다) — F1
        df = c.out / sel["dockerfile"]
        if not df.is_file():
            core.fail("HINT_DOCKERFILE_ABSENT", f"쓰인 Dockerfile 이 없다: output/{c.topo}/{sel['dockerfile']}")
        dsel = _select_dockerfile(c, sel["dockerfile"])
        rf = [_write_bytes(payload, "build_recipe", df.name, dsel["bytes"], dsel["mode"])]
        rf_src = [core.rel(c.repo, df)]
        chosen = {dsel["path"]: dsel}
        pr = c.memo.get("probe")
        probe_note = ((pr or {}).get("record") or {}).get("result") or "이미지 탐침 안 함"
        dests = _copy_dests(dsel["text"])
        notes: list[str] = []
        not_shipped: list[dict] = []
        for src in _dockerfile_copies(dsel["text"]):
            if src in PATCH_DIRS.values():
                continue
            # 컨텍스트 밖(절대·`..`)·글롭 원천은 한 파일로 대응시킬 수 없다 — 싣지 않되 **소리내어** 남긴다(조용한 불완전 레시피 ✗).
            if src.startswith("/") or ".." in PurePath(src).parts or any(ch in src for ch in "*?["):
                not_shipped.append({"source": src, "why": "컨텍스트 밖 또는 글롭 원천 — 파일 하나로 대응 불가"})
                continue
            p = c.out / src
            if p.is_file():
                cpath = _dest_path(src, dests[src], False) if src in dests else None
                ran = bool(pr and pr.get("ran"))
                baked = pr["files"].get(cpath) if cpath and ran else None
                why = ("COPY 목적지를 이미지 경로로 해소하지 못했다(상대 목적지 · 원천 여럿)" if not cpath else
                       probe_note if not ran else f"이미지에 {cpath} 없음(뒤 층이 지웠거나 옮겼다)")
                fsel = _select_copied(c, src, p, baked, why)
                chosen[fsel["path"]] = fsel
                rf.append(_write_bytes(payload, "build_recipe", p.name, fsel["bytes"], fsel["mode"]))
                rf_src.append(core.rel(c.repo, p))
                notes.append(f"{src}: {sel['dockerfile']} 이 COPY" +
                             (" → /etc/pip/constraint.txt(제약 천장 · wheel 설치 입력 아님)"
                              if src == "requirements.txt" and sel["track"] == "source-build" else ""))
            else:
                not_shipped.append({"source": src, "why": "COPY 원천이지만 파일이 아니거나(디렉터리) 부재 — 싣지 않음"})
        files += rf
        led_df = isinstance(ledger, dict) and ledger.get("dockerfile") == sel["dockerfile"]
        rvi = _recipe_vs_image(c, rf_src, _image_created(c), chosen)
        br_ev = {"kind": "build-ledger" if led_df else "file", "ref": sel["dockerfile_source"],
                 "note": " · ".join(notes) or None, "recipe_vs_image": rvi,
                 # 공유 계약(2026-09-22 S2 round 2): {commit|null, method, history_match:{matched,total}} — 쓰인 Dockerfile 의 개정
                 "selected_revision": {"path": dsel["path"], "commit": dsel["commit"], "method": dsel["method"],
                                       "history_match": dsel["history_match"], "candidates": dsel["candidates"]},
                 "selected_revisions": [{k: v for k, v in s.items() if k in ("path", "commit", "method", "history_match",
                                                                              "shipped", "verified")}
                                        for _, s in sorted(chosen.items())]}
        if not_shipped:
            br_ev["context_not_shipped"] = not_shipped      # 수신자 빌드가 이 COPY 에서 멈출 수 있다 — 01 표가 말해야 한다
        slots["build_recipe"] = _slot(rf, br_ev)
        # compose — compose 파일 + 서빙 경로가 부르는 러너(asset 정본 · 마운트본과 바이트 대조) + env 형상.
        #   2026-09-22 S2 round 3: 추적 파일(compose · 러너 정본)은 **측정 당시 개정**을 싣고(`_select_tracked`), 서빙 경로 파생(러너 ·
        #   env_file 참조 · 서브 전달 목록 · slave CONFIG_FILE)도 그 개정 텍스트로 한다. 측정 뒤 재생성된 env 형상은 싣지 않는다.
        cf: list[str] = []
        shapes: dict[str, str] = {}
        env_shapes: list[dict] = []
        regenerated: list[str] = []
        regen_skipped: list[dict] = []
        compose_excluded: list[dict] = []
        if c.compose_path is None:
            core.fail("HINT_COMPOSE_ABSENT", f"output/{c.topo}/docker-compose.yaml 부재 — docker 평면의 기동 입력(A층).")
        csel = _select_tracked(c, c.compose_path)
        cf.append(_write_bytes(payload, "compose", c.compose_path.name, csel["bytes"], csel["mode"]))
        # 경로를 받는 소유자 함수(slave_forward)는 실린 사본(= 측정 당시 개정 바이트)을 읽는다.
        c.compose_eff = payload / cf[-1]
        tracked_sel = [csel]
        rd = c.render()
        names, _ = _runner_names(c)
        allowed = tuple(getattr(rd, "RUNNER_SCRIPTS", ()))
        armed, armed_why = _runtime_patch_armed(c)
        shipped_runners: list[str] = []
        for name in names:
            if name == RUNTIME_PATCH_ARMER and not armed:
                # F3(2026-09-22 S2 round 2): 무장할 런타임 패치가 없으면 arm_patch.sh 는 no-op 이다 — "적용된 것만" 싣는다.
                compose_excluded.append({"file": name, "why": f"not-armed({armed_why})"})
                continue
            shipped_runners.append(name)
            if name not in allowed:
                core.fail("HINT_RUNNER_UNKNOWN", f"서빙 경로가 부르는 러너가 render_dockerfile.RUNNER_SCRIPTS 밖이다: {name}",
                          "러너 정본은 upstream assets/configs 다 — 등재하거나 compose 를 확인한다.")
            canonical, mounted = Path(rd.SHARED_ASSET_DIR) / name, c.out / "configs" / name
            if not canonical.is_file():
                core.fail("HINT_RUNNER_OWNER_MISSING", f"러너 정본 부재: {name}")
            rsel = _select_tracked(c, canonical)
            # 정본은 asset 이다(루트 configs/ 는 낡은 추적 파생본 — build_plane.md §4.1). 컨테이너가 실제로 마운트하는 것은
            # output/<t>/configs/ 사본이므로 둘이 갈리면 배포한 러너가 서빙한 러너가 아니다 → 차단.
            # 2026-09-22 S2 round 3: 마운트본 mtime ≤ 측정(또는 측정 시각 미관측)이면 그것이 **서빙한** 러너다 — 실을 바이트(측정 당시
            #   개정)와 대조한다. 마운트본이 측정 뒤 다시 쓰였으면 서빙 증거가 아니다: 작업트리 정본을 싣는 경우에만 옛 대조(렌더 드리프트)를
            #   유지하고, 측정 전 개정을 싣는 경우는 대조하지 않되 그 사실을 적는다(재생 발행이 측정 뒤 정본 수정 때문에 거짓 차단되지 않게).
            if mounted.is_file():
                mb = mounted.read_bytes()
                mmt = _mtime_utc(mounted)
                served = measured is None or (mmt is not None and mmt <= measured)
                if served or rsel["shipped"] == "worktree":
                    if mb != rsel["bytes"]:
                        core.fail("HINT_RUNNER_RENDER_DRIFT",
                                  f"{'서빙한 마운트본(mtime ≤ 측정)' if served else 'asset 정본'}과 실을 러너 바이트가 갈렸다: {name}"
                                  f"(실을 바이트 = {rsel['shipped']})",
                                  f"`render_dockerfile.py --materialize-configs --topology {c.topo}` 로 다시 materialize 한다"
                                  "(측정 당시 마운트본과 다르면 이 셀의 러너가 아니다 — 발행 대상을 확인한다).")
                    rsel["cross_check"] = list(rsel["cross_check"]) + [
                        f"마운트본 output/{c.topo}/configs/{name}"
                        + (f"(mtime {mmt} ≤ 측정 — 서빙한 러너)" if served and measured else "") + " 바이트 일치"]
                else:
                    rsel["cross_check"] = list(rsel["cross_check"]) + [
                        f"마운트본 output/{c.topo}/configs/{name} 은 측정 뒤(mtime {mmt}) 다시 쓰였다 — 서빙 증거 아님 · 대조 생략"]
            cf.append(_write_bytes(payload, "compose", name, rsel["bytes"], rsel["mode"]))
            tracked_sel.append(rsel)
        compose_text = _compose_text(c)
        # 제외 행: 사유 코드(글자 그대로 · 소비자 대조) + 짧은 이유. 시각은 별도 키(mtime_utc · measured_utc · measured_source)가
        #   든다 — why 에 다시 적으면 01 표가 같은 시각을 두 번 싣는다(template 이 스칼라 키를 함께 싣는다 · 바이트 B 절약).
        def excluded_regen(rel_file: str, regen: dict) -> None:
            compose_excluded.append({"file": rel_file, "reason": POST_MEASUREMENT_REGENERATED,
                                     "why": (f"{POST_MEASUREMENT_REGENERATED}(재생성 시점 렌더러의 키·값 — 측정 당시 형상 아님 · "
                                             f"측정 당시 env = {ENV_OBSERVED_POINTER})"),
                                     "mtime_utc": regen["mtime_utc"], "measured_utc": regen["measured_utc"],
                                     "measured_source": regen["measured_source"]})
        for name in sorted(set(_ENV_FILE_REF.findall(compose_text))):
            if "$" in name or name == f".env.{c.cell}":
                continue                       # 셀 env 는 트리플렛으로 실린다
            p = c.out / "envs" / name
            if not p.is_file():
                # 코드는 등재된 일반 코드 하나(evidence.MISSING_CODES — 2026-09-22 evidence 소유자가 전용 코드 2종을 걷고 이것으로
                # 일원화했다) · **어느 파일**이 없는지는 env_shapes 행이 말한다(00 결손 표 + 01 형상 표).
                missing.append("HINT_MISSING_ENV_SHAPE")
                env_shapes.append({"file": f"envs/{name}", "status": "absent"})
                continue
            kind = _TIER_ENV_KINDS.get(name, "generic")
            regen = _regenerated(c, p)
            if regen:
                regenerated.append(core.rel(c.repo, p))
                excluded_regen(f"envs/{name}", regen)
                regen_skipped.append({"file": f"envs/{name}"})
                env_shapes.append({"file": f"envs/{name}", "kind": kind, "status": "excluded",
                                   "reason": POST_MEASUREMENT_REGENERATED, "post_measurement_regenerated": True, **regen})
                continue
            text, rule = _env_shape(c.repo, p, kind, render_module=rd, sf=c.sf, measured=measured)
            shapes[name] = text
            cf.append(_write(payload, "compose", f"{name}.template", text))
            env_shapes.append({"file": f"envs/{name}", "kind": kind, "rule": rule,
                               "template": f"artifacts/compose/{name}.template", "post_measurement_regenerated": False})
        if (c.out / ".env").is_file():         # compose 변수치환용 프로젝트 env(마운트 경로 4종) — 키-축 규칙
            regen = _regenerated(c, c.out / ".env")
            if regen:
                regenerated.append(core.rel(c.repo, c.out / ".env"))
                excluded_regen(".env", regen)
                env_shapes.append({"file": ".env", "kind": "project", "status": "excluded",
                                   "reason": POST_MEASUREMENT_REGENERATED, "post_measurement_regenerated": True, **regen})
            else:
                text, rule = _env_shape(c.repo, c.out / ".env", "project", render_module=rd, sf=c.sf, measured=measured)
                cf.append(_write(payload, "compose", ".env.template", text))
                env_shapes.append({"file": ".env", "kind": "project", "rule": rule,
                                   "template": "artifacts/compose/.env.template", "post_measurement_regenerated": False})
        else:
            # 부재를 조용히 넘기면 수신자는 마운트 변수(NAS_MODEL_PATH 등)의 존재를 모른다 — `materialize-env` 산출물이다.
            missing.append("HINT_MISSING_ENV_SHAPE")
            env_shapes.append({"file": ".env", "kind": "project", "status": "absent"})
        recipe = None
        if c.topo == "multi":
            recipe = _sub_recipe(c, shapes, regen_skipped)
            cf.append(_write(payload, "compose", "sub_recipe.json", core.dumps(recipe)))
        files += cf
        n_shipped = sum(1 for e in env_shapes if e.get("template"))
        compose_ev = {"kind": "file", "ref": core.rel(c.repo, c.compose_path),
                      "note": (f"서빙 러너 {shipped_runners} · env 형상 실림 {n_shipped}/{len(env_shapes)}"
                               + (f" · compose = 측정 당시 개정 {csel['shipped']}" if csel["shipped"] != "worktree" else "")
                               + (f" · ⚠ 측정({measured}) 뒤 재생성 {sorted(regenerated)} — 형상 미실림"
                                  f"({POST_MEASUREMENT_REGENERATED} · 측정 당시 env 는 {ENV_OBSERVED_POINTER})"
                                  if regenerated else "")),
                      # 공유 계약(2026-09-22 S2 round 2): 측정 시각 뒤 mtime 의 env 파일(저장소 상대 경로). round 3 부터 이 파일들은
                      #   **싣지 않는다**(excluded · reason post-measurement-regenerated) — 목록은 "무엇이 빠졌나" 의 사실로 남긴다.
                      "post_measurement_regenerated": sorted(regenerated), "measured_utc": measured,
                      # 공유 계약(2026-09-22 S2 round 3): build_recipe 와 같은 자리 — compose 파일의 측정 당시 개정 · 추적 파일 전부
                      "selected_revision": _tracked_row(csel),
                      "selected_revisions": [_tracked_row(s) for s in sorted(tracked_sel, key=lambda s: s["path"])]}
        warns = [f"{s['path']}: {s['warning']}" for s in tracked_sel if s.get("warning")]
        if warns:
            compose_ev["revision_warnings"] = warns
        slots["compose"] = _slot(cf, compose_ev, compose_excluded or None)
    else:
        # native 평면: 설치 명령 + 실제 lock(`pip freeze`) — 러너는 트리플렛 `.sh` 로 이미 실린다(다른 이름으로 두 번 싣지
        # 않는다 · "설치 명령" 이라 부르면 서빙 러너가 설치 절차인 척한다). Dockerfile·compose 는 싣지 않는다(F2).
        # Evidence validates these producer paths before artifacts is called. Revalidate
        # relative regular-file shape here because this module also accepts loose Any
        # evidence objects in its public API; a missing signal must not become a partial
        # native payload.
        rf = []
        freezes = _get(c.ev, "pip_freeze_paths")
        if not isinstance(freezes, dict) or set(freezes) != {"main", "sub"}:
            core.fail("HINT_NATIVE_EVIDENCE_MISSING",
                      "native artifacts의 pip_freeze_paths는 정확히 main·sub 보존 경로여야 한다.")
        native_paths = (("native_install_path", _get(c.ev, "native_install_path"), "native-install.sh"),
                        ("pip_freeze_paths.main", freezes.get("main"), "pip-freeze-main.txt"),
                        ("pip_freeze_paths.sub", freezes.get("sub"), "pip-freeze-sub.txt"))
        checked = []
        # 전부 검증한 뒤 복사한다. 중간 항목에서 막힐 때 artifacts/ 일부가 남으면 다음 재시도가
        # 그 잔재를 실물로 오독한다(부분 적용 금지).
        for key, raw, name in native_paths:
            if not isinstance(raw, str) or not raw.strip() or Path(raw).is_absolute():
                core.fail("HINT_NATIVE_EVIDENCE_MISSING", f"native artifacts에 필요한 {key} 보존 경로가 없다.")
            fp = c.repo / core.rel(c.repo, raw)
            if not fp.is_file() or fp.is_symlink():
                core.fail("HINT_NATIVE_EVIDENCE_UNREADABLE", f"native artifacts의 {key} 가 regular file이 아니다: {raw}")
            checked.append((fp, name))
        for fp, name in checked:
            rf.append(_copy(payload, fp, "build_recipe", name))
        files += rf
        slots["build_recipe"] = _slot(rf, {"kind": "file", "ref": "native producer preserved install + pip freeze(main/sub)",
                                           "note": "native 재현 입력(serve proof가 보존한 repo-relative files)"})
        slots["compose"] = _slot([], {"kind": "none", "ref": None, "note": "native 평면 — compose 해당 없음"})
        for s in ("build_patch_pre", "build_patch_post"):
            slots[s] = _slot([], {"kind": "none", "ref": None, "note": "native 평면 — 이미지 빌드 패치 해당 없음"})
        env_shapes, recipe = [], None

    # fork_pin — `VARIANT=` 한 줄(부재 = stock · workflow.md §변종 좌표의 거처 2026-08-13)
    variants = sorted({ln.split("=", 1)[1].strip() for ln in _read(c.triplet["env"]).splitlines()
                       if ln.strip().startswith("VARIANT=")})
    ff: list[str] = []
    if len(variants) > 1:
        core.fail("HINT_VARIANT_AMBIGUOUS", f"셀 env 에 VARIANT 가 여럿이다: {variants}")
    if variants and variants[0]:
        vid = variants[0]
        led = _json_file(c.repo / REL_ARCH_VARIANT_LEDGER) or {}
        table = led.get("source_build_variants") if isinstance(led.get("source_build_variants"), dict) else {}
        # 원장 항목의 키와 안의 `variant_id` 가 다르다(실측: 키 `deepseek-v4-flash-sm12x` · variant_id `source-sm12x-vllm-0.23.0`).
        # workflow.md 는 `VARIANT=<variant_id>` 라 적고 옛 hint_collect 는 키로 찾았다 — 키가 먼저, 없으면 **유일한** variant_id 일치.
        rec = table.get(vid) if not vid.startswith("_") and isinstance(table.get(vid), dict) else None
        if rec is None:
            hits = [v for k, v in sorted(table.items()) if not str(k).startswith("_") and isinstance(v, dict)
                    and v.get("variant_id") == vid]
            rec = hits[0] if len(hits) == 1 else None
        if rec is None:
            core.fail("HINT_FORK_PIN_UNBOUND", f"VARIANT={vid} 가 {REL_ARCH_VARIANT_LEDGER} 에 없다(키·유일 variant_id 모두 불일치).",
                      "변종 좌표는 Band2 원장에 등재한다(workflow.md §변종 좌표의 거처) — 등재 없이 발행하지 않는다.")
        ref = c.env.get("VLLM_REF")
        if rec.get("vllm_ref") and ref and rec["vllm_ref"] != ref:
            core.fail("HINT_FORK_PIN_MISMATCH", f"변종 {vid} 의 vllm_ref={rec['vllm_ref']!r} ≠ 셀 env VLLM_REF={ref!r}")
        ff.append(_write(payload, "fork_pin", "variant.json", core.dumps({"variant_id": vid, **rec})))
    files += ff
    slots["fork_pin"] = _slot(ff, {"kind": "file" if ff else "none", "ref": REL_ARCH_VARIANT_LEDGER,
                                   "note": "VARIANT 줄 부재 = stock" if not ff else "변종 좌표 = Band2 원장"})

    # 배포 전 백스톱 — branch 커밋의 최종 스캔 전에 artifacts 만 먼저 본다(원인 슬롯을 가리키려고)
    from . import pii
    terms = pii.load_pii_terms(c.repo)
    if terms is None:
        missing.append("HINT_MISSING_PII_TERMS")
    exempt: list[str] = []       # 면제는 조용히 넘기지 않는다 — stderr 대신 결과에 싣는다(pii._note_exempt 계약)
    if art.is_dir():
        hits = pii.scan_tree(payload, terms, profile="deploy", rels=["artifacts"], exempt_log=exempt)
        if hits:
            core.fail("HINT_ARTIFACT_PII", "artifacts/ 에 배포 금지 패턴:\n  " + pii.render_hits(hits, 10),
                      "해당 파일의 출처(트리플렛·compose·패치)를 고치거나 형상 규칙을 고친다(덧칠 ✗).")

    _require_registered(c.repo, missing)
    return {
        "plane": plane, "plane_source": plane_source, "track": sel["track"] if sel else "native",
        "build": sel, "slots": slots, "applied_set": applied,
        "build_ledger": {"source": applied["source"], "doc": ledger} if isinstance(ledger, dict) and applied else None,
        "missing": sorted(set(missing)), "files": sorted(set(files)), "sub_recipe": recipe,
        "value_status_candidates": value_status_candidates(c.repo, c.ev, forward_module=c.sf),
        "reproduce_steps": _reproduce(c, sel, plane), "env_shapes": env_shapes,
        "pii_exempt": [_exempt_record(x) for x in exempt],
    }


# ── 자체검사 (격리 임시 저장소 · 라이브 태그·브랜치·캠페인·도커 비의존) ──────────────────────────────
class _FakeTierRender:
    """env_tier 가 있는 render 모듈 대역(정본 경로 시험). SHARED_ASSET_DIR 은 호출부가 채운다."""
    RUNNER_SCRIPTS = ("serve_runner.sh", "debug-init.sh", "arm_patch.sh")
    SHARED_ASSET_DIR = ""

    @staticmethod
    def env_tier(key: str) -> tuple[str, str | None]:
        if key.endswith("_HOST_IP") or key == "SSH_USER" or key.endswith("IFNAME"):
            return "env", f"manifest.fixture.{key.lower()}"
        if key == "RAY_PORT" or key == "NCCL_DEBUG":
            return "invariant", None
        return "preset", "fixture"

    @staticmethod
    def ledger_build_args(text: str) -> list[str]:          # 시험 평면 대역 — 실물 정규식과 같은 모양(`^ARG NAME`)
        return list(dict.fromkeys(re.findall(r"(?m)^ARG\s+([A-Za-z_][A-Za-z0-9_]*)", text)))


def _fake_docker(table: dict) -> DockerRunner:
    def run(argv, timeout=DOCKER_TIMEOUT_S):
        key = tuple(argv[1:])
        for pat, (rc, out) in table.items():
            if key[:len(pat)] == pat:
                return subprocess.CompletedProcess(argv, rc, out, "")
        return subprocess.CompletedProcess(argv, 1, "", "no such object")
    return run


def _fake_image_docker(*, layers: list[tuple[str, str]], created: str, fs: dict | None = None, env: list | None = None,
                       probe: bool = True, calls: list | None = None, removed: set | None = None,
                       crash_on_cp: bool = False) -> DockerRunner:
    """이미지 하나를 흉내 내는 대역(2026-09-22 S2 round 2 자체검사): inspect(.Id · .Created · .Config.Env) · history(평문 ·
    `{{json .}}` 둘 다 같은 층 목록에서) · create/cp/rm(메모리 파일계 `fs` — 값이 dict 면 디렉터리) · run(원장 cat = 없음).
    `probe=True` 면 `image_probe` 능력을 선언한다(False = 읽기 전용 실행기). `crash_on_cp` = cp 가 예외로 죽는다(finally rm 시험)."""
    fs = fs or {}
    cid = "c" * 64
    calls = calls if calls is not None else []
    removed = removed if removed is not None else set()

    def run(argv, timeout=DOCKER_TIMEOUT_S):
        a = list(argv[1:])
        calls.append(a)
        cp = subprocess.CompletedProcess
        if a[:2] == ["image", "inspect"]:
            fmt = a[a.index("--format") + 1] if "--format" in a else ""
            out = {"{{.Id}}": "sha256:fx\n", "{{.Created}}": created + "\n",
                   "{{json .Config.Env}}": json.dumps(env or [])}.get(fmt)
            return cp(argv, 0 if out else 1, out or "", "")
        if a[:1] == ["history"]:
            fmt = a[a.index("--format") + 1]
            if fmt == "{{json .}}":
                return cp(argv, 0, "".join(json.dumps({"CreatedBy": by, "CreatedAt": at, "Comment": "buildkit.dockerfile.v0",
                                                       "CreatedSince": "x"}) + "\n" for by, at in layers), "")
            return cp(argv, 0, "".join(by + "\n" for by, _ in layers), "")
        if a[:1] == ["create"]:
            return cp(argv, 0, cid + "\n", "")
        if a[:1] == ["cp"]:
            if crash_on_cp:
                raise RuntimeError("fixture: cp 가 예외로 죽었다")
            src, dest = a[-2].split(":", 1)[1], Path(a[-1])
            if isinstance(fs.get(src), dict):
                dest.mkdir(parents=True)
                for n, b in fs[src].items():
                    (dest / n).write_bytes(b)
                return cp(argv, 0, "", "")
            if isinstance(fs.get(src), bytes):
                dest.write_bytes(fs[src])
                return cp(argv, 0, "", "")
            return cp(argv, 1, "", f"Error response from daemon: Could not find the file {src} in container {cid}")
        if a[:1] == ["rm"]:
            removed.add(a[-1])
            return cp(argv, 0, a[-1] + "\n", "")
        if a[:1] == ["run"]:
            return cp(argv, 1, "", "cat: /opt/easy-vllm/build_ledger.json: No such file")
        return cp(argv, 1, "", "unexpected docker call")
    if probe:
        run.image_probe = True        # type: ignore[attr-defined]
    return run


def _set_mtime(path: Path, utc: str) -> None:
    """픽스처 파일의 mtime 을 **주입한** 시각으로 둔다(벽시계 ✗ — 측정 뒤 재생성 판정을 결정론으로 시험한다)."""
    import os
    ts = core.parse_utc(utc).timestamp()
    os.utime(path, (ts, ts))


def selftest() -> list[str]:
    import tempfile
    bad: list[str] = []

    def ck(name: str, cond: bool) -> None:
        if not cond:
            bad.append(f"artifacts: {name}")

    def raises(name: str, code: str, fn) -> None:
        try:
            fn()
            ck(f"{name}(차단 안 됨)", False)
        except core.HintError as e:
            ck(f"{name}({e.code})", e.code == code)

    own = core.SKILL_DIR.parents[2]
    with tempfile.TemporaryDirectory(prefix="hint-artifacts-") as tmp:
        repo = Path(tmp).resolve()
        # 소유 모듈 실물(slave_forward · render_dockerfile)을 격리 저장소로 복사 — 대역이 실물보다 좁지 않게.
        for rel_path in (core.REL_SLAVE_FORWARD, core.REL_RENDER_DOCKERFILE):
            (repo / rel_path).parent.mkdir(parents=True, exist_ok=True)
            (repo / rel_path).write_bytes((own / rel_path).read_bytes())
        out = repo / "output" / "multi"
        for d in ("configs", "envs", "build_patches_src/files", "build_patches"):
            (out / d).mkdir(parents=True, exist_ok=True)
        (repo / core.REL_PII_TERMS).parent.mkdir(parents=True, exist_ok=True)
        (repo / core.REL_PII_TERMS).write_text("fixture-secret-literal\n", encoding="utf-8")
        assets = repo / ".claude/skills/upstream-version-watch/assets/configs"
        assets.mkdir(parents=True)
        runner_txt = ('#!/bin/bash\nif [ -f /app/configs/arm_patch.sh ]; then\n    source /app/configs/arm_patch.sh\nfi\n'
                      'GPU_COUNT=$(ray status | head -1)\nif [ "${GPU_COUNT}" = "2" ] || ray status | grep -q "0.0/2.0 GPU"; then :; fi\n')
        for name, text in (("serve_runner.sh", runner_txt), ("arm_patch.sh", "#!/bin/bash\n_cfg=\"${CONFIG_FILE:-default}\"\n"),
                           ("debug-init.sh", "#!/bin/bash\n")):
            (assets / name).write_text(text, encoding="utf-8")
            (out / "configs" / name).write_text(text, encoding="utf-8")
        cell = "c1-mmp"
        (out / "configs" / f"{cell}.yaml").write_text(
            "# 헤더 주석\nmodel: /app/models/X\nhost: localhost\nport: 8000\nserved_model_name: fx\n"
            "max-model-len: 262144\nkv-cache-dtype: auto\n"
            "kv-cache-memory-bytes: 21474836480   # 20GiB 음성대조\nmax-num-seqs: 8\n"
            "enforce-eager: true   # 승계 통제변인\nurl-ish: 'a#b'   # 따옴표 안 # 는 값\n"
            "async-scheduling: false   # MTP+async 금지(명시 필수)\ngpu-memory-utilization: 0.85   # 요구 조건\n"
            "speculative-config:\n  method: mtp\n  num_speculative_tokens: 3\n", encoding="utf-8")
        (out / "configs" / f"{cell}.sh").write_text(
            "#!/bin/bash\nif [ -f /app/configs/arm_patch.sh ]; then source /app/configs/arm_patch.sh; fi\nvllm serve\n",
            encoding="utf-8")
        (out / "configs" / f"{cell}_patch.py").write_text("# runtime patch\n", encoding="utf-8")
        (out / "envs" / f".env.{cell}").write_text(
            "CONFIG_FILE=c1-mmp\nSLAVE_CONTAINER_NAME=c1-slave\nIMAGE_TAG=easy-vllm:fx-source\n"
            "BUILD_DOCKERFILE=Dockerfile.source-build\nVLLM_REF=v0.1.0\nBUILD_JOBS=8\nVLLM_PLE_MMAP=1\n"
            "VLLM_PLE_MMAP_DIR=/app/ple/x\n", encoding="utf-8")
        (out / "Dockerfile.source-build").write_text(
            "FROM base\nARG VLLM_REF=v0.1.0\nARG SM12X_PORT=0\nARG SRC_DEPS_AUTHORITY=0\nARG BUILD_JOBS=16\n"
            "COPY requirements.txt /tmp/requirements.txt\nCOPY build_patches_src/ /tmp/build_patches_src/\n"
            "COPY build_patches/ /tmp/build_patches/\n", encoding="utf-8")
        (out / "Dockerfile").write_text("FROM wheel\nARG VLLM_VERSION=0.0.1\nCOPY requirements.txt /tmp/r.txt\n",
                                        encoding="utf-8")
        (out / "requirements.txt").write_text("transformers<5.15.0\n", encoding="utf-8")
        ip = ".".join(["192", "168", "7", "11"])           # 추적 파일에 IPv4 모양 리터럴을 두지 않는다(2026-08-06)
        mnt = "/" + "mnt"                                   # 같은 이유 — 운영자 경로 모양(abs-op-path) 리터럴도 조립한다
        compose = ("x-gpu-common: &gpu-common\n  image: ${IMAGE_TAG:-easy-vllm:fx-source}\n  build:\n    context: .\n"
                   "    dockerfile: ${BUILD_DOCKERFILE:-Dockerfile.source-build}\n    args:\n"
                   "      VLLM_REPO: ${VLLM_REPO:-repo}\n      VLLM_REF: ${VLLM_REF:-v0.1.0}\n      SM12X_PORT: ${SM12X_PORT:-0}\n"
                   "      SRC_DEPS_AUTHORITY: ${SRC_DEPS_AUTHORITY:-0}\n      BUILD_JOBS: ${BUILD_JOBS:-16}\n"
                   "  volumes:\n    - ${NAS_MODEL_PATH:-" + mnt + "/models}:/app/models:ro\n    - ./configs:/app/configs:ro\n"
                   "services:\n  vllm-debug:\n    <<: *gpu-common\n    profiles: [debug]\n    env_file:\n"
                   "      - envs/.env.interconnect\n    command: [\"bash\", \"--rcfile\", \"/app/configs/debug-init.sh\"]\n"
                   "  vllm-master-serve:\n    <<: *gpu-common\n    profiles: [master]\n    environment:\n"
                   "      NODE_ROLE: master\n      CONFIG_FILE: ${CONFIG_FILE:-default}\n    env_file:\n"
                   "      - envs/.env.interconnect\n      - envs/.env.cluster\n      - path: envs/.env.${CONFIG_FILE:-default}\n"
                   "    command: [\"bash\", \"/app/configs/serve_runner.sh\"]\n"
                   "  vllm-slave-serve:\n    <<: *gpu-common\n    profiles: [slave]\n"
                   "    container_name: ${SLAVE_CONTAINER_NAME:-slave}\n    environment:\n      NODE_ROLE: slave\n"
                   "      CONFIG_FILE: ${CONFIG_FILE:-default}\n      RAY_PORT: ${RAY_PORT:-6379}\n"
                   "      VLLM_PLE_MMAP: ${VLLM_PLE_MMAP:-0}\n      VLLM_PLE_MMAP_DIR: ${VLLM_PLE_MMAP_DIR:-/app/ple}\n"
                   "    env_file:\n      - envs/.env.interconnect\n      - envs/.env.cluster\n"
                   "    command: [\"bash\", \"/app/configs/serve_runner.sh\"]\n")
        (out / "docker-compose.yaml").write_text(compose, encoding="utf-8")
        (out / "envs/.env.cluster").write_text(
            f"# 헤더\nMASTER_HOST_IP={ip}\nSLAVE_HOST_IP=node-sub-fixture\nSSH_USER=opfixture\nMAX_JOBS=4\nRAY_PORT=6379\n",
            encoding="utf-8")
        (out / "envs/.env.interconnect").write_text("NCCL_DEBUG=INFO\nNCCL_SOCKET_IFNAME=enpfixture0\nNCCL_IB_DISABLE=1\n",
                                                    encoding="utf-8")
        (out / ".env").write_text(f"NAS_MODEL_PATH={mnt}/nas-fixture/models\nPLE_MMAP_HOST_PATH={mnt}/nvme-fixture\n",
                                  encoding="utf-8")
        # 측정(스윕 generated_utc 2026-01-01T19:30Z) **전** 에 쓰인 파일로 둔다 — 측정 뒤 재생성 판정(F2)은 mtime 을 본다.
        for p in (out / "envs/.env.cluster", out / "envs/.env.interconnect", out / ".env", out / "configs" / f"{cell}.yaml",
                  out / "configs" / f"{cell}.sh", out / "envs" / f".env.{cell}"):
            _set_mtime(p, "2026-01-01T00:00:00Z")
        # 자기게이트 — **실물 꼴(따옴표 두른 기본값 · if … != "1" … exit 0)** 로 적는다
        (out / "build_patches_src/50-port.sh").write_text(
            '#!/bin/bash\n# model-trigger : OtherModel\n: "${SM12X_PORT:=0}"\nif [ "${SM12X_PORT}" != "1" ]; then\n'
            '    echo "skip"\n    exit 0\nfi\n', encoding="utf-8")
        (out / "build_patches_src/55-deps.sh").write_text(
            '#!/bin/bash\n: "${SM12X_PORT:=0}"\n: "${SRC_DEPS_AUTHORITY:=0}"\n'
            'if [ "${SRC_DEPS_AUTHORITY}" != "1" ] && [ "${SM12X_PORT}" = "1" ]; then\n    SRC_DEPS_AUTHORITY=1\nfi\n'
            'if [ "${SRC_DEPS_AUTHORITY}" != "1" ]; then\n    echo "skip"\n    exit 0\nfi\n', encoding="utf-8")
        (out / "build_patches_src/60-ungated.sh").write_text("#!/bin/bash\n# gate : 없음(ungated)\n", encoding="utf-8")
        # ★게이트 앞에 부수효과가 있는 스크립트 — skip 을 재구성하지 않는다
        (out / "build_patches_src/58-effect-first.sh").write_text(
            '#!/bin/bash\npip install something\n: "${SM12X_PORT:=0}"\nif [ "${SM12X_PORT}" != "1" ]; then\n    exit 0\nfi\n',
            encoding="utf-8")
        (out / "build_patches_src/PROVENANCE.json").write_text("{}", encoding="utf-8")
        (out / "build_patches_src/.gitkeep").write_text("", encoding="utf-8")
        (out / "build_patches_src/stray.txt").write_text("x", encoding="utf-8")
        (out / "build_patches/10-shared.sh").write_text("#!/bin/bash\n# model-trigger : DeepseekV4 범용\n",
                                                       encoding="utf-8")
        history = ("WORKDIR /workspace\nRUN |4 VLLM_REF=v0.1.0 SM12X_PORT=0 SRC_DEPS_AUTHORITY=0 BUILD_JOBS=8 /bin/bash -c "
                   "export SM12X_PORT=\"${SM12X_PORT}\"\nRUN |3 NVIDIA_PYTORCH_VERSION=26.07 SM12X_PORT=1 X=1 /bin/sh -c base\n")
        docker_old = _fake_docker({("image", "inspect", "--format", "{{.Id}}"): (0, "sha256:fx\n"),
                                   ("image", "inspect", "--format", "{{.Created}}"): (0, "2026-01-02T03:04:05.1+09:00\n"),
                                   ("history",): (0, history),
                                   ("run",): (1, "")})
        (repo / "output/multi/benchlog/sweep_c1-mmp/level_01").mkdir(parents=True)
        (repo / "output/multi/benchlog/sweep_c1-mmp/lite_cold.json").write_text(
            json.dumps({"date": "20260101-190000"}), encoding="utf-8")
        (repo / "output/multi/benchlog/sweep_c1-mmp/level_01/bench.json").write_text(
            json.dumps({"date": "20260101-190500", "backend": "openai", "model_id": "fx", "tokenizer_id": "/app/models/X",
                        "num_prompts": 4, "max_concurrency": 1, "request_rate": "inf", "burstiness": 1.0, "completed": 4,
                        "total_input_tokens": 4096, "total_output_tokens": 1024}), encoding="utf-8")
        (repo / "docs/logs/main/events").mkdir(parents=True)
        (repo / "docs/logs/main/events/2026-01.jsonl").write_text(
            json.dumps({"kind": "budget_declare", "label": f"smoke-{cell}", "ts": "2026-01-01T18:30:00Z"}) + "\n"
            + json.dumps({"kind": "budget_declare", "label": f"smoke-{cell}", "ts": "2026-01-01T21:00:00Z"}) + "\n"
            + json.dumps({"kind": "budget_declare", "label": "smoke-other", "ts": "2026-01-01T18:59:00Z"}) + "\n",
            encoding="utf-8")
        ev = {"topology": "multi", "cell": cell, "node": "cluster", "lockset": {"kv_source": "measured-clamp",
                                                                               "batch_source": "hand-lever"},
              "cell_status": {}, "attestation": None, "build_identity": {"image_tag": "easy-vllm:fx-source"},
              "sweep": {"dir": "output/multi/benchlog/sweep_c1-mmp", "generated_utc": "2026-01-01T19:30:00Z",
                        "index": {"config": cell}}}
        fake_rd = _FakeTierRender()
        fake_rd.SHARED_ASSET_DIR = str(assets)

        # ── 적용 집합(원장 부재 · X9 재구성) ──
        ap = applied_set(repo, ev, runner=docker_old)
        by = {r["file"]: r for r in ap["patches"]}
        ck("원장 부재 = unobservable", ap["status"] == "unobservable")
        ck("★자기게이트 skip 재구성(실물 꼴 따옴표 기본값)", by.get("50-port.sh", {}).get("result") == "skipped"
           and by["50-port.sh"]["result_source"] == "reconstructed(docker-history+gate)")
        ck("★뒤집기 게이트(55) 재구성 = 모든 게이트 변수 ≠ 1 일 때만 skip", by.get("55-deps.sh", {}).get("result") == "skipped")
        ck("ungated = 적용 미관측", by.get("60-ungated.sh", {}).get("result") == "unobserved")
        ck("★게이트 앞 부수효과 → skip 재구성 ✗(미관측)", by.get("58-effect-first.sh", {}).get("result") == "unobserved")
        ck("★베이스 층 인자(NVIDIA_*·다른 값) 배제", ap["reconstruction"]["build_args"] == {
            "BUILD_JOBS": "8", "SM12X_PORT": "0", "SRC_DEPS_AUTHORITY": "0", "VLLM_REF": "v0.1.0"})
        ck("X10 post 라벨", by.get("10-shared.sh", {}).get("shared_image_input") is True
           and "model-trigger" in by["10-shared.sh"]["label"])
        exc = {(x["file"], x["why"].split("(")[0]) for x in ap["excluded"]}
        ck("★.gitkeep·PROVENANCE.json·files/ 명시 건너뜀", {(".gitkeep", "known-non-patch"),
                                                        ("PROVENANCE.json", "known-non-patch"),
                                                        ("files/", "known-non-patch")} <= exc)
        ck("★규약 밖 이름은 소리내어 기재", ("stray.txt", "nonconforming") in exc)
        ck("탐침 기록(attestation 부재 → 로컬 이미지 원장 없음 → 재구성 → 이미지 탐침)",
           [p["tier"] for p in ap["probes"]] == ["attestation", "local-image", "reconstruction", "image-probe"]
           and ap["probes"][1]["result"].startswith("no-ledger"))
        ck("★image_probe 를 선언하지 않은 주입 실행기 = 탐침 건너뜀(create/cp/rm 0 · 읽기 전용)",
           ap["probes"][3]["result"].startswith("skipped(주입 실행기가 image_probe"))
        ap_rf = applied_set(repo, ev, runner=_fake_docker({("image", "inspect"): (0, "x"), ("run",): (125, ""),
                                                           ("history",): (0, history)}))
        ck("★docker 자신의 실패(125)는 '원장 없음' 이 아니다", ap_rf["probes"][1]["result"] == "docker-run-failed(rc=125)")
        # ★history 미관측이면 skip 을 단정하지 않는다(옛 코드는 스크립트 기본값 0 으로 skip 을 지어냈다)
        ap2 = applied_set(repo, ev, runner=_fake_docker({}))
        ck("★빌드 인자 미관측 → skip 단정 ✗", all(r["result"] == "unobserved" for r in ap2["patches"]))
        # ★SM12X_PORT=1 이면 재구성 불가(적용 경로) — 싣되 미관측
        hist1 = history.replace("SM12X_PORT=0 SRC", "SM12X_PORT=1 SRC")
        ap3 = applied_set(repo, ev, runner=_fake_docker({("image", "inspect"): (0, "x"), ("history",): (0, hist1)}))
        b3 = {r["file"]: r["result"] for r in ap3["patches"]}
        ck("★게이트 변수 1 → 미관측(55 는 뒤집기로 켜질 수 있다)", b3.get("50-port.sh") == "unobserved"
           and b3.get("55-deps.sh") == "unobserved")
        # ★`:=` 기본값 의미(2026-09-22 감사): 빈 build-arg 는 스크립트 기본값으로 채워진다 — 기본값 1 이면 **돈다**.
        (out / "build_patches_src/57-default-on.sh").write_text(
            '#!/bin/bash\n: "${FX_ON:=1}"\nif [ "${FX_ON}" != "1" ]; then\n    exit 0\nfi\n', encoding="utf-8")
        (out / "Dockerfile.source-build").write_text(
            (out / "Dockerfile.source-build").read_text(encoding="utf-8").replace("ARG BUILD_JOBS=16\n",
                                                                                 "ARG BUILD_JOBS=16\nARG FX_ON=1\n"),
            encoding="utf-8")
        hist_on = history.replace("|4 VLLM_REF=v0.1.0", "|5 FX_ON= VLLM_REF=v0.1.0")
        ap_on = applied_set(repo, ev, runner=_fake_docker({("image", "inspect"): (0, "x"), ("history",): (0, hist_on)}))
        bon = {r["file"]: r for r in ap_on["patches"]}
        ck("★빈 build-arg + `:=1` 기본값 → skip 단정 ✗(실효값 1)", bon.get("57-default-on.sh", {}).get("result") == "unobserved"
           and bon.get("50-port.sh", {}).get("result") == "skipped")
        (out / "build_patches_src/57-default-on.sh").unlink()
        (out / "Dockerfile.source-build").write_text(
            (out / "Dockerfile.source-build").read_text(encoding="utf-8").replace("ARG FX_ON=1\n", ""), encoding="utf-8")
        # ★내용 권위 = 측정 digest(태그는 가변 포인터): 태그가 다른(재빌드된) 이미지를 가리켜도 그 history 로 skip 을 재구성하지
        #   않는다. 측정 digest 이미지가 로컬에 없으면 태그로 **대체하지 않고** 미관측이다(2026-09-22 감사).
        dg = "sha256:" + "d" * 64

        def docker_tag_only(argv, timeout=DOCKER_TIMEOUT_S):
            if argv[-1] == dg or dg in argv:
                return subprocess.CompletedProcess(argv, 1, "", "No such image")
            if argv[1:2] == ["history"]:
                return subprocess.CompletedProcess(argv, 0, history, "")
            if argv[1:3] == ["image", "inspect"]:
                return subprocess.CompletedProcess(argv, 0, "sha256:other\n", "")
            return subprocess.CompletedProcess(argv, 1, "", "")
        ev_dg = dict(ev, build_identity={"image_tag": "easy-vllm:fx-source", "image_digest": dg})
        ap_dg = applied_set(repo, ev_dg, runner=docker_tag_only)
        ck("★태그가 다른 이미지를 가리켜도 측정 digest 로만 관측(skip 재구성 ✗)",
           all(r["result"] == "unobserved" for r in ap_dg["patches"])
           and ap_dg["probes"][1]["result"].startswith("image-absent-local") and ap_dg["probes"][1]["image"] == dg
           and ap_dg["reconstruction"]["image"] == dg)
        ap_dg2 = applied_set(repo, ev_dg, runner=docker_old)       # digest 가 로컬에 있으면 그 history 로 재구성한다
        ck("측정 digest 이미지의 history 로 재구성", {r["file"]: r["result"] for r in ap_dg2["patches"]}.get("50-port.sh")
           == "skipped" and dg in ap_dg2["reconstruction"]["build_args_source"])
        # ★wheel 트랙 = 패치 디렉터리를 COPY 하지 않는다 → 전부 excluded_by_recipe
        ev_w = dict(ev, triplet={"env": "output/multi/envs/.env.wheel", "yaml": f"output/multi/configs/{cell}.yaml",
                                 "sh": f"output/multi/configs/{cell}.sh"})
        (out / "envs/.env.wheel").write_text("IMAGE_TAG=easy-vllm:fx-wheel\nBUILD_DOCKERFILE=Dockerfile\n", encoding="utf-8")
        apw = applied_set(repo, ev_w, runner=docker_old)
        ck("★wheel 트랙 패치 미실행(excluded_by_recipe)", apw["patches"] == []
           and sum(x["why"].startswith("excluded_by_recipe") for x in apw["excluded"]) == 5)
        # ★쓰인 Dockerfile 부재 = 차단(빈 텍스트로 계속하면 모든 패치가 거짓 excluded_by_recipe 사유로 빠진다)
        (out / "envs/.env.nodf").write_text("IMAGE_TAG=easy-vllm:fx-x\nBUILD_DOCKERFILE=Dockerfile.absent\n", encoding="utf-8")
        raises("★쓰인 Dockerfile 부재", "HINT_DOCKERFILE_ABSENT", lambda: applied_set(
            repo, dict(ev_w, triplet=dict(ev_w["triplet"], env="output/multi/envs/.env.nodf")), runner=docker_old))

        # ── 원장 관측(② 메인 로컬 이미지) ──
        ledger = {"schema_version": 1, "dockerfile": "Dockerfile.source-build", "track": "source-build",
                  "patches": [{"phase": "pre", "file": "50-port.sh", "result": "skipped", "reason": "SM12X_PORT=0",
                               "result_source": "log-token"},
                              {"phase": "pre", "file": "60-ungated.sh", "result": "applied", "result_source": "exit-code",
                               "script_sha256": core.sha256_file(out / "build_patches_src/60-ungated.sh")},
                              {"phase": "post", "file": "10-shared.sh", "result": "applied", "result_source": "status-file"}],
                  "inline_patches": [{"name": "strip-hoist", "result": "skipped", "reason": "accepts hoist"}]}
        docker_new = _fake_docker({("image", "inspect", "--format", "{{.Id}}"): (0, "sha256:fx\n"),
                                   ("run", "--rm", "--network", "none", "--pull", "never"): (0, json.dumps(ledger))})
        apn = applied_set(repo, ev, runner=docker_new)
        byn = {(r["phase"], r["file"]): r for r in apn["patches"]}
        ck("원장 관측 = observed · 메인 로컬 출처", apn["status"] == "observed" and apn["source"].startswith("docker-image(main-local)"))
        ck("inline 패치 행", byn.get(("inline", "strip-hoist"), {}).get("result") == "skipped")
        ck("★원장에 없는 파일(55) = not-in-ledger 로 싣지 않음", any(x["file"] == "55-deps.sh" and
                                                             x["why"].startswith("not-in-ledger") for x in apn["excluded"]))
        ck("★서브 셀은 메인 로컬 이미지를 보지 않는다", applied_set(repo, dict(ev, node="sub"), runner=docker_new)
           ["probes"][1]["result"].startswith("skipped(node=sub"))
        # ★원장 ↔ 셀 env 선택자 모순 = 차단
        bad_ledger = dict(ledger, dockerfile="Dockerfile")
        raises("★원장·셀 env 선택자 모순", "HINT_BUILD_SELECTOR_MISMATCH", lambda: applied_set(
            repo, ev, runner=_fake_docker({("image", "inspect"): (0, "x"), ("run",): (0, json.dumps(bad_ledger))})))
        # ① attestation 원장이 로컬보다 먼저 · 노드 인식(main)
        ev_att = dict(ev, attestation={"schema_version": 2, "nodes": {"sub": {"build_ledger": {"patches": []}},
                                                                      "main": {"build_ledger": ledger}}})
        apa = applied_set(repo, ev_att, runner=_fake_docker({}))
        host_like = "spark-" + "a1b2c3"             # 노드 id 가 호스트 이름 꼴이어도 sub_recipe 로 새면 안 된다
        ev_att2 = dict(ev_att, attestation={**ev_att["attestation"], "parity_blocking": False,
                                            "nodes": {"main": {"node_id": host_like, "build_ledger": ledger},
                                                      "sub": {"node_id": host_like, "build_ledger": ledger}},
                                            "parity": {"vllm_sha": {"verdict": "equal", "main": "a" * 40, "sub": "a" * 40},
                                                       "build_ledger": {"verdict": "equal"}}})
        sr2 = sub_recipe(repo, ev_att2, render_module=fake_rd)
        ck("v2 parity 축별 판정 · ★노드 id 비유출", sr2["parity_attestation"]["observed"]["vllm_sha"]["verdict"] == "equal"
           and host_like not in json.dumps(sr2))
        ck("① attestation 원장 우선 · main 노드 선택", apa["status"] == "observed" and apa["source"].endswith("#main")
           and "per_node_patchsets" in apa)
        # ★역할 이름 밖 키(노드 id·호스트 이름)는 원장 키로 받지 않는다 — 개수만 남는다(신원 유출 통로 차단)
        ev_att3 = dict(ev, attestation={"schema_version": 2, "_source": "output/multi/benchlog/attestation_fx-build.json "
                                                                           "(이미지 compose 라벨 vllm_fx-build_project)",
                                        "nodes": {host_like: {"build_ledger": ledger}, "sub": {"build_ledger": ledger}}})
        apa3 = applied_set(repo, ev_att3, runner=_fake_docker({}))
        ck("★비역할 키 무시 · sub 원장으로 main 대리 ✗ · 키 문자열 비유출",
           apa3["status"] == "unobservable" and apa3["probes"][0].get("ignored_node_keys") == 1
           and apa3["probes"][0]["result"].startswith("partial") and host_like not in json.dumps(apa3))
        ck("★attestation 출처 = evidence 가 묶은 파일(셀 이름 파일 ✗)",
           apa3["probes"][0]["ref"] == "output/multi/benchlog/attestation_fx-build.json"
           and sub_recipe(repo, ev_att3, render_module=fake_rd)["parity_attestation"]["source"]
           == "output/multi/benchlog/attestation_fx-build.json")

        # ── 수집(원장 부재 이미지) ──
        payload = repo / "payload"
        res = collect(repo, ev, payload, render_module=fake_rd, runner=docker_old)
        names = set(res["files"])
        ck("쓰인 source Dockerfile 포함", "artifacts/build_recipe/Dockerfile.source-build" in names)
        ck("★쓰이지 않은 wheel Dockerfile 제외", "artifacts/build_recipe/Dockerfile" not in names)
        ck("source-build 가 COPY 하는 requirements(제약 천장) 포함", "artifacts/build_recipe/requirements.txt" in names
           and "constraint" in (res["slots"]["build_recipe"]["evidence"]["note"] or ""))
        ck("★skip 패치 제외", not any("50-port" in p or "55-deps" in p for p in names))
        ck("미관측 패치는 포함", "artifacts/build_patch_pre/60-ungated.sh" in names
           and "artifacts/build_patch_post/10-shared.sh" in names)
        ck("★known non-patch 미포함", not any(p.endswith(("PROVENANCE.json", ".gitkeep", "stray.txt")) for p in names))
        ck("serve_runner·arm_patch(서빙 경로 파생) 포함", {"artifacts/compose/serve_runner.sh",
                                                     "artifacts/compose/arm_patch.sh"} <= names)
        ck("★debug-init(디버그 프로필 전용) 제외", "artifacts/compose/debug-init.sh" not in names)
        ck("★블록 꼴 command 도 러너 참조를 읽는다(한 줄 꼴만 읽으면 러너가 조용히 빠진다)", _CONFIGS_REF.findall(
            " ".join(_command_texts("    command:\n      - bash\n      - /app/configs/serve_runner.sh\n    env_file:\n"
                                    "      - envs/.env.cluster\n"))) == ["serve_runner.sh"])
        ck("env 형상 3종 + sub_recipe", {"artifacts/compose/.env.cluster.template",
                                        "artifacts/compose/.env.interconnect.template",
                                        "artifacts/compose/.env.template", "artifacts/compose/sub_recipe.json"} <= names)
        cl = (payload / "artifacts/compose/.env.cluster.template").read_text(encoding="utf-8")
        ic = (payload / "artifacts/compose/.env.interconnect.template").read_text(encoding="utf-8")
        pj = (payload / "artifacts/compose/.env.template").read_text(encoding="utf-8")
        ck("★cluster 형상: 호스트 실값 제거 · 키 전부 유지", ip not in cl and "node-sub-fixture" not in cl
           and "opfixture" not in cl and all(k + "=" in cl for k in ("MASTER_HOST_IP", "SLAVE_HOST_IP", "SSH_USER",
                                                                     "MAX_JOBS", "RAY_PORT")))
        ck("interconnect 형상: ①env 치환 · ③invariant 유지", "enpfixture0" not in ic and "NCCL_DEBUG=INFO" in ic)
        ck("★project .env: 운영자 경로 제거(키-축)", mnt + "/" not in pj and "<manifest.nas_model_path>" in pj)
        sr = json.loads((payload / "artifacts/compose/sub_recipe.json").read_text(encoding="utf-8"))
        ck("★sub_recipe 에 mount 실값 없음", mnt + "/" not in json.dumps(sr) and
           sr["per_node_values"].get("NAS_MODEL_PATH") == "<manifest.nas_model_path>")
        # RAY_PORT 는 셀 env 가 정할 때만 넘어간다(옛 스모크 `RAYP=$(val RAY_PORT)` 의미 — EFC 값은 slave 도 받는다)
        ck("sub_recipe 전달 목록 = slave_forward(스모크와 같은 함수)", {r["key"] for r in sr["env_forward"]} == {
            "IMAGE_TAG", "BUILD_DOCKERFILE", "VLLM_REF", "BUILD_JOBS", "NAS_MODEL_PATH", "SLAVE_CONTAINER_NAME",
            "VLLM_PLE_MMAP", "VLLM_PLE_MMAP_DIR"})
        ck("K11 관측: slave CONFIG_FILE=default → 런타임 패치 미무장", sr["role_diff"]["slave_config_file"] == "default"
           and sr["role_diff"]["slave_runtime_patch"].startswith("not-armed")
           and sr["role_diff"]["master_runtime_patch"] == "armed")
        ck("serve_runner GPU 리터럴 \"2\"", sr["role_diff"]["runner_gpu_join_literal"]["value"] == "2")
        ck("per_node_values: ①env 표지 · interconnect 템플릿", sr["per_node_values"].get("MASTER_HOST_IP", "").startswith("<")
           and sr["per_node_values"].get("interconnect") == "artifacts/compose/.env.interconnect.template")
        ck("parity = attestation 부재 → unobservable", sr["parity_attestation"]["observed"] == "unobservable")
        ck("결손: 원장 부재 기재", "HINT_MISSING_BUILD_LEDGER" in res["missing"])
        ck("슬롯 rationale 은 기계가 채우지 않는다(선언 = 저작자 몫)", all(r["rationale"] is None for r in res["slots"].values()))
        ck("★원장 부재 패치 슬롯 = 1-signal(적용 미관측은 증거가 아니다)",
           res["slots"]["build_patch_pre"]["confidence"] == "1-signal"
           and res["slots"]["build_patch_pre"]["evidence"]["kind"] == "none"
           and res["slots"]["triplet"]["confidence"] == "2-signal")
        ck("★skip 사유가 슬롯 excluded 에 남는다", any(x["file"] == "50-port.sh" and x["why"].startswith("skipped")
                                                  for x in res["slots"]["build_patch_pre"].get("excluded", [])))
        vs = {r["knob"]: r for r in res["value_status_candidates"]}
        ck("값의 지위: lockset kv_source=measured-clamp → tuned", vs["kv-cache-memory-bytes"]["candidate_status"] == "tuned")
        ck("값의 지위: batch_source=hand-lever → inherited 후보", vs["max-num-seqs"]["candidate_status"] == "inherited")
        ck("값의 지위: 주석 '승계' · auto=engine-default", vs["enforce-eager"]["candidate_status"] == "inherited"
           and vs["kv-cache-dtype"]["candidate_status"] == "engine-default")
        ck("★근거 없으면 후보를 지어내지 않는다", vs["max-model-len"]["candidate_status"] is None)
        # 2026-09-22 S2 round 3: 주석 단어(필수·요구)만으로는 declared-requirement 를 제안하지 않는다 — inherited + 문구
        ck("★주석 단어 '필수' 만 = declared-requirement ✗ → inherited + 문구(note · reason)",
           vs["async-scheduling"]["candidate_status"] == "inherited"
           and vs["async-scheduling"].get("note") == COMMENT_ONLY_REQUIREMENT_NOTE
           and COMMENT_ONLY_REQUIREMENT_NOTE in vs["async-scheduling"]["reason"]
           and vs["gpu-memory-utilization"]["candidate_status"] == "inherited")
        vs_l = {r["knob"]: r for r in value_status_candidates(repo, dict(ev, lockset=dict(ev["lockset"], gmu_source="target_gmu")))}
        ck("★lockset 출처(target_gmu · 기록된 요구)는 declared-requirement 유지 — 주석 규칙이 덮지 않는다 · 문구 없음",
           vs_l["gpu-memory-utilization"]["candidate_status"] == "declared-requirement"
           and "note" not in vs_l["gpu-memory-utilization"])
        ck("★주석 규칙은 다른 단어 후보를 건드리지 않는다(승계 = inherited · 문구 없음 · 음성대조 = negative-control)",
           "note" not in vs["enforce-eager"] and vs["enforce-eager"]["candidate_status"] == "inherited")
        ck("COMMENT_ONLY_REQUIREMENT_NOTE 문구(공유 계약 · 글자 그대로)",
           COMMENT_ONLY_REQUIREMENT_NOTE == "yaml 주석만 있음 — 관측된 실패가 있으면 declared-requirement")
        ck("★중첩 yaml 은 부모 노브 하나 · 따옴표 안 # 는 값", "method" not in vs and vs["url-ish"]["value"] == "a#b"
           and "num_speculative_tokens" in vs["speculative-config"]["value"])
        ck("D-b 전송·신원 노브(model)는 value-status 후보에서 빠진다 · 튜닝 노브는 전부 남는다",
           not ({"model", "host", "port", "served_model_name", "served-model-name"} & set(vs))
           and {"max-model-len", "kv-cache-memory-bytes", "enforce-eager", "speculative-config"} <= set(vs))
        ck("D-b 제외 목록은 닫힌 4종(tripwire)", VALUE_STATUS_EXCLUDED_KNOBS == {"model", "host", "port", "served-model-name"})
        steps = {s["step"]: s for s in res["reproduce_steps"]}
        ck("재현 절차 순서", [s["step"] for s in res["reproduce_steps"]] == ["render", "build", "serve", "bench"])
        ck("빌드 종료 = 이미지 Created(UTC) · ★층 CreatedAt 미관측(history JSON 판독 불가)이면 시작을 지어내지 않는다",
           steps["build"]["observed"]["end_utc"] == "2026-01-01T18:04:05Z" and steps["build"]["observed"]["start_utc"] is None
           and steps["build"]["observed"]["bound"] == "none")
        ck("기동 상한 = 측정 전 마지막 예산 선언 → 첫 측정(다른 label·뒤 선언 제외)",
           steps["serve"]["observed"] == {"start_utc": "2026-01-01T18:30:00Z", "end_utc": "2026-01-01T19:00:00Z",
                                          "seconds": 1800, "bound": "upper"})
        ck("벤치 = 첫 측정 → generated_utc", steps["bench"]["observed"]["seconds"] == 1800)
        # F5(2026-09-22 S2 round 2): 빌드·벤치 = 독립 실행 가능한 재구성 명령 · 자리표시(§) ✗
        bcmd = steps["build"]["command"]
        ck("F5 빌드 = docker build -f <Dockerfile> --build-arg(history) … -t <tag> <context>",
           "docker build -f output/multi/Dockerfile.source-build \\" in bcmd and "--build-arg SM12X_PORT=0" in bcmd
           and "--build-arg BUILD_JOBS=8" in bcmd and "-t easy-vllm:fx-source" in bcmd and "\n  output/multi\n" in bcmd
           and steps["build"]["command_source"] == "reconstructed(docker-history build-args + compose build)")
        ck("F5 벤치 = bench JSON 필드 복원(vllm bench serve)", steps["bench"]["command_source"] == "reconstructed(bench json)"
           and "vllm bench serve --backend openai --model fx --tokenizer /app/models/X --dataset-name random "
               "--random-input-len 1024 --random-output-len 256 --num-prompts 4 --max-concurrency 1 --request-rate inf"
           in steps["bench"]["command"])
        ck("★F5 재현 명령에 자리표시(§) 없음", not any("§" in (s.get("command") or "") for s in res["reproduce_steps"]))
        ck("★PII 면제 기록에 매치 원문 없음(compose `${VAR:-…}` 기본값)", bool(res["pii_exempt"])
           and not any(mnt in x for x in res["pii_exempt"]))
        # ★시계 가정 불성립(첫 측정 date 가 스윕 종료 뒤) → 소요를 계산하지 않는다(합성 ✗)
        rs_clock = {x["step"]: x for x in reproduce_steps(
            repo, dict(ev, sweep=dict(ev["sweep"], generated_utc="2026-01-01T18:00:00Z")), runner=docker_old)}
        ck("★date 시계 가정 불성립 → serve·bench 미관측", rs_clock["serve"]["observed"]["seconds"] is None
           and rs_clock["bench"]["observed"]["seconds"] is None)
        # 캠페인 phase — phases/<X> 의 X 는 **노드 id** 다(축 이름 아님 · 2026-09-22 감사). cluster = 선언 노드 전부의 같은 셀
        # 기록에서 가장 이른 시작 ~ 가장 늦은 끝 · 다른 셀 기록은 쓰지 않는다 · main/sub 축은 evidence 가 해소한 ev.phases 만.
        for nid, st, en, cid_cell in (("gx-a", "2026-01-01T10:00:00Z", "2026-01-01T11:00:00Z", cell),
                                      ("gx-b", "2026-01-01T10:05:00Z", "2026-01-01T11:30:00Z", cell),
                                      ("gx-c", "2026-01-01T01:00:00Z", "2026-01-01T23:00:00Z", "other-cell"),
                                      ("main", "2026-01-01T00:00:00Z", "2026-01-01T23:59:00Z", cell)):
            (repo / f"campaigns/campx/phases/{nid}").mkdir(parents=True, exist_ok=True)
            (repo / f"campaigns/campx/phases/{nid}/build.status.json").write_text(json.dumps(
                {"cell_id": cid_cell, "started_utc": st, "ended_utc": en}), encoding="utf-8")
        decl = {"nodes": [{"node_id": n} for n in ("gx-a", "gx-b", "gx-c")]}
        rs = {x["step"]: x for x in reproduce_steps(repo, dict(ev, campaign_id="campx", declaration=decl),
                                                    runner=docker_old)}
        ck("빌드 = 캠페인 phase(선언 노드 id span · ★다른 셀·선언 밖 디렉터리 제외)", rs["build"]["observed"]["seconds"] == 5400
           and rs["build"]["observed"]["bound"] == "exact")
        ev_main = dict(ev, node="main", campaign_id="campx", declaration=decl,
                       phases={"build": {"started_utc": "2026-01-01T10:00:00Z", "ended_utc": "2026-01-01T10:30:00Z",
                                         "_path": "campaigns/campx/phases/gx-a/build.status.json"}})
        rm = {x["step"]: x for x in reproduce_steps(repo, ev_main, runner=docker_old)}
        ck("main 축 = evidence 가 해소한 배정 노드 기록(ev.phases)", rm["build"]["observed"]["seconds"] == 1800)
        rm2 = {x["step"]: x for x in reproduce_steps(repo, dict(ev_main, phases={}), runner=docker_old)}
        ck("★축 이름 디렉터리(phases/main)를 노드 id 로 추측하지 않는다", rm2["build"]["observed"]["start_utc"] is None)
        # ★재실행은 기존 artifacts 를 덮지 않는다
        raises("★artifacts 잔재 차단", "HINT_ARTIFACTS_DIR_NOT_EMPTY",
               lambda: collect(repo, ev, payload, render_module=fake_rd, runner=docker_old))
        # ★env_tier 부재 → 같은 모듈 3층 상수에서 파생(소리 나는 폴백 · 실물 render_dockerfile)
        res_r = collect(repo, ev, repo / "payload_real", runner=docker_old)
        shp = {e["file"]: e for e in res_r["env_shapes"]}
        real_rd = _render_module(repo)
        want_rule = "render_dockerfile.env_tier" if callable(getattr(real_rd, "env_tier", None)) else "derived("
        ck("실물 render 모듈로 cluster 형상(env_tier 또는 3층 상수 파생)",
           shp["envs/.env.cluster"]["rule"].startswith(want_rule))
        clr = (repo / "payload_real/artifacts/compose/.env.cluster.template").read_text(encoding="utf-8")
        ck("★실물 규칙에서도 호스트 실값 제거", ip not in clr and "opfixture" not in clr and "MAX_JOBS=4" in clr)
        # ★마운트 러너 드리프트 = 차단
        (out / "configs/serve_runner.sh").write_text("drift\n", encoding="utf-8")
        raises("★러너 드리프트", "HINT_RUNNER_RENDER_DRIFT",
               lambda: collect(repo, ev, repo / "p2", render_module=fake_rd, runner=docker_old))
        (out / "configs/serve_runner.sh").write_text(runner_txt, encoding="utf-8")
        # ★원장이 적용했다는 바이트와 실을 바이트가 다르면 차단(3신호 모순)
        drift_ledger = json.loads(json.dumps(ledger))
        drift_ledger["patches"][1]["script_sha256"] = "0" * 64
        raises("★적용 패치 드리프트", "HINT_APPLIED_PATCH_DRIFT", lambda: collect(
            repo, ev, repo / "p3", render_module=fake_rd,
            runner=_fake_docker({("image", "inspect"): (0, "x"), ("run",): (0, json.dumps(drift_ledger))})))
        # ★형상화가 놓친 지문(용어 파일 리터럴)은 덧칠하지 않고 차단
        (out / "envs/.env.interconnect").write_text("NCCL_DEBUG=fixture-secret-literal\n", encoding="utf-8")
        _set_mtime(out / "envs/.env.interconnect", "2026-01-01T00:00:00Z")
        raises("★형상 백스톱(리터럴)", "HINT_ENV_SHAPE_PII",
               lambda: collect(repo, ev, repo / "p4", render_module=fake_rd, runner=docker_old))
        (out / "envs/.env.interconnect").write_text("NCCL_DEBUG=INFO\nNCCL_SOCKET_IFNAME=enpfixture0\n", encoding="utf-8")
        _set_mtime(out / "envs/.env.interconnect", "2026-01-01T00:00:00Z")
        # 레시피 ↔ 이미지: 추적 Dockerfile 을 이미지 Created 뒤에 고쳤으면 경고(옛 이미지의 X9 결 · 읽기 전용 배관)
        genv = {"GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_NOSYSTEM": "1",
                "GIT_AUTHOR_NAME": core.SYNTHETIC_NAME, "GIT_AUTHOR_EMAIL": core.SYNTHETIC_EMAIL,
                "GIT_COMMITTER_NAME": core.SYNTHETIC_NAME, "GIT_COMMITTER_EMAIL": core.SYNTHETIC_EMAIL,
                "GIT_AUTHOR_DATE": core.git_date("2026-01-01T00:00:00Z"),
                "GIT_COMMITTER_DATE": core.git_date("2026-01-01T00:00:00Z")}
        try:
            core.git(repo, "init", "-q", env_extra=genv)
            core.git(repo, "add", "output/multi/Dockerfile.source-build", "output/multi/requirements.txt", env_extra=genv)
            core.git(repo, "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "fx",
                     env_extra=genv)
            (out / "Dockerfile.source-build").write_text(
                (out / "Dockerfile.source-build").read_text(encoding="utf-8") + "# 빌드 뒤 수정\n", encoding="utf-8")
            rg = collect(repo, ev, repo / "p_git", render_module=fake_rd, runner=docker_old)
            rv = {r["path"]: r for r in rg["slots"]["build_recipe"]["evidence"]["recipe_vs_image"]}
            ck("★빌드 뒤 수정된 추적 Dockerfile = 경고", rv["output/multi/Dockerfile.source-build"].get("worktree_equals_head")
               is False and "warning" in rv["output/multi/Dockerfile.source-build"])
            ck("커밋이 이미지보다 이르고 수정 없음 = 경고 없음", "warning" not in rv["output/multi/requirements.txt"]
               and rv["output/multi/requirements.txt"]["last_commit_utc"] == "2026-01-01T00:00:00Z")
            # ── F1(2026-09-22 S2 round 2): 실린 Dockerfile = 이미지를 지은 개정 ──
            #   커밋 개정(= 이미지 history 와 전수 일치) · 작업트리는 빌드 뒤 원장 스탠자·post COPY 가 붙고 루프가 바뀐 수정본.
            #   이어짐 사이 주석 · heredoc · RUN --mount · 빌더 스테이지(마지막 FROM 앞 — history 에 없다)를 정규화가 다뤄야 한다.
            df_rev = ("FROM base AS builder\nRUN echo builder-only\nFROM base\nARG SM12X_PORT=0\nARG BUILD_JOBS=16\n"
                      "COPY requirements.txt /tmp/requirements.txt\n"
                      "RUN --mount=type=cache,target=/root/.ccache pip install \\\n    # 이어짐 사이 주석\n    fx-pkg && \\\n"
                      "    echo done\nRUN python3 - <<'PY'\nimport sys\nprint(\"hoist\")\nPY\n"
                      "COPY build_patches_src/ /tmp/build_patches_src/\n"
                      "RUN for p in $(ls /tmp/build_patches_src/*.sh 2>/dev/null | sort); do bash \"$p\" || exit 1; done\n")
            (out / "Dockerfile.rev").write_text(df_rev, encoding="utf-8")
            core.git(repo, "add", "output/multi/Dockerfile.rev", env_extra=genv)
            core.git(repo, "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "rev",
                     env_extra=genv)
            rev1 = core.git_out(repo, "rev-parse", "HEAD")
            df_wt = df_rev.replace('do bash "$p" || exit 1; done', 'do bash "$p"; echo "$?" > /x; done') + \
                "COPY build_patches/ /tmp/build_patches/\nRUN python3 - <<'PY'\nprint(\"ledger\")\nPY\n"
            (out / "Dockerfile.rev").write_text(df_wt, encoding="utf-8")
            # 이미지 Created 뒤의 커밋(작업트리와 같다) — `--before=<Created>` 가 이것을 고르면 안 된다.
            genv3 = dict(genv, GIT_AUTHOR_DATE=core.git_date("2026-01-03T00:00:00Z"),
                         GIT_COMMITTER_DATE=core.git_date("2026-01-03T00:00:00Z"))
            core.git(repo, "add", "output/multi/Dockerfile.rev", env_extra=genv3)
            core.git(repo, "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "late",
                     env_extra=genv3)
            (out / "envs/.env.rev").write_text("IMAGE_TAG=easy-vllm:fx-rev\nBUILD_DOCKERFILE=Dockerfile.rev\n", encoding="utf-8")
            ev_rev = dict(ev, triplet={"env": "output/multi/envs/.env.rev", "yaml": f"output/multi/configs/{cell}.yaml",
                                       "sh": f"output/multi/configs/{cell}.sh"})
            layers_rev = [
                ('RUN |2 SM12X_PORT=0 BUILD_JOBS=8 /bin/sh -c for p in $(ls /tmp/build_patches_src/*.sh 2>/dev/null | sort); '
                 'do bash "$p" || exit 1; done # buildkit', "2026-01-02T03:04:00+09:00"),
                ("COPY build_patches_src/ /tmp/build_patches_src/ # buildkit", "2026-01-02T03:03:00+09:00"),
                ("RUN |2 SM12X_PORT=0 BUILD_JOBS=8 /bin/sh -c python3 - <<'PY'\nimport sys\nprint(\"hoist\")\nPY # buildkit",
                 "2026-01-02T03:02:00+09:00"),
                ("RUN |2 SM12X_PORT=0 BUILD_JOBS=8 /bin/sh -c pip install     fx-pkg &&     echo done # buildkit",
                 "2026-01-01T09:00:00+09:00"),
                ("COPY requirements.txt /tmp/requirements.txt # buildkit", "2026-01-01T09:00:00+09:00"),
                ("ARG BUILD_JOBS=16", "2026-01-01T09:00:00+09:00"), ("ARG SM12X_PORT=0", "2026-01-01T09:00:00+09:00"),
                ("RUN |2 NVIDIA_X=1 SM12X_PORT=1 /bin/sh -c base-layer # buildkit", "2025-12-01T00:00:00+09:00"),
                ("ADD file:abc in / ", "2025-12-01T00:00:00+09:00")]
            dk_rev = _fake_image_docker(layers=layers_rev, created="2026-01-02T03:04:05+09:00", probe=False)
            rrv = collect(repo, ev_rev, repo / "p_rev", render_module=fake_rd, runner=dk_rev)
            srv = rrv["slots"]["build_recipe"]["evidence"]["selected_revision"]
            ck("★F1 작업트리가 이미지 history 와 다르면 Created 이전 개정을 싣는다(Created 뒤 커밋 ✗)",
               srv["commit"] == rev1 and srv["history_match"] == {"matched": 5, "total": 5}
               and (repo / "p_rev/artifacts/build_recipe/Dockerfile.rev").read_text(encoding="utf-8") == df_rev
               and {"revision": "worktree", "matched": 4, "total": 8} in srv["candidates"])
            rvr = {r["path"]: r for r in rrv["slots"]["build_recipe"]["evidence"]["recipe_vs_image"]}
            ck("F1 대조로 확인된 개정을 실으면 작업트리 경고 대신 무엇이 실렸는지 적는다",
               "warning" not in rvr["output/multi/Dockerfile.rev"] and rvr["output/multi/Dockerfile.rev"]["shipped"] ==
               f"git:{rev1}")
            ck("★F1 COPY 판독 = 이미지를 지은 개정 텍스트(작업트리가 더한 post COPY 는 이 빌드의 입력이 아니다)",
               any(x["file"] == "10-shared.sh" and x["why"].startswith("excluded_by_recipe")
                   for x in rrv["applied_set"]["excluded"]))
            brv = {s["step"]: s for s in rrv["reproduce_steps"]}["build"]
            ck("F1·F5 빌드 명령 = 개정 텍스트의 ARG 로 거른 history 인자 · 층 시각 범위(캐시 층 포함 · 소요 아님)",
               "-f output/multi/Dockerfile.rev" in brv["command"] and "--build-arg BUILD_JOBS=8" in brv["command"]
               and f"git:{rev1}" in brv["command"]
               and brv["observed"].get("layer_span", {}).get("oldest_utc") == "2026-01-01T00:00:00Z"
               and brv["observed"]["layer_span"]["layers"] == 5)
            # 2026-09-22 S2 round 3(공유 계약): phase 기록이 없으면 build 관측 구간 = 층 CreatedAt 첫 층 → 끝 층(layer-window)
            ck("★빌드 관측 구간 = 층 CreatedAt 첫 층 → 끝 층(phase 기록 없음) · 한계 layer-window · 소요 문구가 창임을 말한다",
               brv["observed"]["start_utc"] == "2026-01-01T00:00:00Z" and brv["observed"]["end_utc"] == "2026-01-01T18:04:00Z"
               and brv["observed"]["bound"] == LAYER_WINDOW and brv["observed"]["seconds"] == 65040
               and brv["duration"].startswith("층 창 18시간 4분 0초") and "빌드 소요 아님" in brv["duration"]
               and "첫 층 → 끝 층" in brv["source"])
            rph = {x["step"]: x for x in reproduce_steps(repo, dict(ev_rev, campaign_id="campx", declaration=decl),
                                                         runner=dk_rev)}
            ck("★캠페인 phase 기록(exact)이 층 창보다 먼저 · 층 범위는 부가 사실로 남는다",
               rph["build"]["observed"]["bound"] == "exact" and rph["build"]["observed"]["seconds"] == 5400
               and rph["build"]["observed"].get("layer_span", {}).get("layers") == 5)
            rbad = collect(repo, ev_rev, repo / "p_rev_bad", render_module=fake_rd, runner=_fake_image_docker(
                layers=[("RUN |1 SM12X_PORT=0 /bin/sh -c something-else # buildkit", "2026-01-02T03:00:00+09:00")],
                created="2026-01-02T03:04:05+09:00", probe=False))
            sbad = rbad["slots"]["build_recipe"]["evidence"]["selected_revision"]
            rvb = {r["path"]: r for r in rbad["slots"]["build_recipe"]["evidence"]["recipe_vs_image"]}
            ck("★F1 어느 개정도 더 맞지 않으면 작업트리 + 경고(대조 결과 기재)",
               sbad["commit"] is None and "어느 개정도" in sbad["method"] and "warning" in rvb["output/multi/Dockerfile.rev"]
               and (repo / "p_rev_bad/artifacts/build_recipe/Dockerfile.rev").read_text(encoding="utf-8") == df_wt)
            rnc = collect(repo, ev_rev, repo / "p_rev_nocreated", render_module=fake_rd, runner=_fake_image_docker(
                layers=layers_rev, created="not-a-time", probe=False))
            snc = rnc["slots"]["build_recipe"]["evidence"]["selected_revision"]
            ck("★F1 이미지 Created 미관측 = 개정 후보 없음(작업트리 · 사유 기재 · 개정을 추측하지 않는다)",
               snc["commit"] is None and "Created 미관측" in snc["method"] and len(snc["candidates"]) == 1)
            rplain = collect(repo, ev_rev, repo / "p_rev_plain", render_module=fake_rd, runner=_fake_docker(
                {("image", "inspect", "--format", "{{.Created}}"): (0, "2026-01-02T03:04:05+09:00\n"),
                 ("image", "inspect"): (0, "sha256:fx\n"), ("history",): (0, layers_rev[0][0] + "\n")}))
            ck("★F1 history JSON 판독 불가 = 대조하지 않는다(작업트리 · '미관측' 기재 · 다른 형식으로 대체 ✗)",
               "미관측" in rplain["slots"]["build_recipe"]["evidence"]["selected_revision"]["method"]
               and rplain["slots"]["build_recipe"]["evidence"]["selected_revision"]["commit"] is None)
            (out / "Dockerfile.rev").write_text(df_rev, encoding="utf-8")     # 작업트리 = 이미지(HEAD 는 늦은 커밋 — 수정본)
            rsame = collect(repo, ev_rev, repo / "p_rev_same", render_module=fake_rd, runner=dk_rev)
            ssame = rsame["slots"]["build_recipe"]["evidence"]["selected_revision"]
            rvs = {r["path"]: r for r in rsame["slots"]["build_recipe"]["evidence"]["recipe_vs_image"]}
            ck("F1 작업트리가 history 와 전수 일치 = 작업트리(HEAD 대비 수정본 경고는 실린 파일의 결함이 아니다)",
               ssame["commit"] is None and ssame["method"].startswith("worktree(측정 이미지 docker history")
               and rvs["output/multi/Dockerfile.rev"].get("worktree_equals_head") is False
               and "warning" not in rvs["output/multi/Dockerfile.rev"] and "note" in rvs["output/multi/Dockerfile.rev"])
            ck("정규화: 이어짐 주석·--mount·heredoc·빌더 스테이지", _dockerfile_steps(df_rev) == [
                "COPY requirements.txt /tmp/requirements.txt", "RUN pip install fx-pkg && echo done",
                "RUN python3 - <<'PY' import sys print(\"hoist\") PY", "COPY build_patches_src/ /tmp/build_patches_src/",
                'RUN for p in $(ls /tmp/build_patches_src/*.sh 2>/dev/null | sort); do bash "$p" || exit 1; done']
               and [_history_step(b) for b, _ in layers_rev[:5]] == list(reversed(_dockerfile_steps(df_rev))))
            # ── compose 슬롯 = 측정 당시 개정(2026-09-22 S2 round 3) ──
            #   측정 = 스윕 generated 2026-01-01T19:30Z. compose·러너 정본을 측정 전(01-01T00:00Z)에 커밋 → 측정 뒤 작업트리에 주석과
            #   env_file 참조(.env.late)를 더하고 master 러너를 바꾸고(late_runner.sh — RUNNER_SCRIPTS 밖) slave CONFIG_FILE 기본값을 바꾼 뒤
            #   01-03 에 커밋까지 한다 · 러너 정본도 측정 뒤 고친다(마운트본은 측정 전 그대로). 측정 당시 개정으로 파생하면 셋 다 보이지 않는다.
            asset_rel = ".claude/skills/upstream-version-watch/assets/configs/serve_runner.sh"
            compose0 = (out / "docker-compose.yaml").read_bytes()
            runner0 = (assets / "serve_runner.sh").read_bytes()
            try:
                core.git(repo, "add", "output/multi/docker-compose.yaml", asset_rel, env_extra=genv)
                core.git(repo, "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "compose",
                         env_extra=genv)
                rev_c = core.git_out(repo, "rev-parse", "HEAD")
                compose_late = compose0.replace(b"services:\n", "# 측정 뒤 주석: NCCL 19 keys + envs/.env.late\nservices:\n"
                                                .encode("utf-8"), 1).replace(b"${CONFIG_FILE:-default}", b"${CONFIG_FILE:-latecfg}")
                compose_late = compose_late.replace(b'profiles: [master]', b'profiles: [master]\n    command: ["bash", '
                                                    b'"/app/configs/late_runner.sh"]', 1)
                (out / "docker-compose.yaml").write_bytes(compose_late)
                core.git(repo, "add", "output/multi/docker-compose.yaml", env_extra=genv3)
                core.git(repo, "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "late compose",
                         env_extra=genv3)
                late_c = core.git_out(repo, "rev-parse", "HEAD")
                (assets / "serve_runner.sh").write_bytes(runner0 + "# 측정 뒤 수정\n".encode("utf-8"))
                (out / "envs/.env.late").write_text("LATE_KEY=1\n", encoding="utf-8")
                for p_, t_ in ((out / "docker-compose.yaml", "2026-01-02T00:00:00Z"),
                               (assets / "serve_runner.sh", "2026-01-02T00:00:00Z"),
                               (out / "configs/serve_runner.sh", "2026-01-01T10:00:00Z"),
                               (out / "envs/.env.late", "2026-01-01T00:00:00Z")):
                    _set_mtime(p_, t_)
                rcr = collect(repo, ev, repo / "p_crev", render_module=fake_rd, runner=docker_old)
                cev = rcr["slots"]["compose"]["evidence"]
                csr = cev["selected_revision"]
                ck("★compose 작업트리가 측정 뒤 바뀌었으면 측정 전 마지막 커밋 바이트(측정 뒤 커밋 ✗) · 방법·교차대조 기재",
                   (repo / "p_crev/artifacts/compose/docker-compose.yaml").read_bytes() == compose0
                   and csr["commit"] == rev_c and csr["shipped"] == f"git:{rev_c}"
                   and csr["path"] == "output/multi/docker-compose.yaml"
                   and csr["basis"] == "git-rev-before-measurement" and csr["verified"] is False
                   and csr["commits_after_measurement"] == [late_c[:12]] and "측정 뒤 커밋 1" in " ".join(csr["cross_check"])
                   and "git log -1 --before=2026-01-01T19:30:00Z -- output/multi/docker-compose.yaml" in csr["method"]
                   and csr["history_match"] is None and "bytes" not in csr)
                ck("★측정 뒤 더해진 env_file 참조(.env.late)·러너(late_runner.sh)·slave CONFIG_FILE 기본값은 이 셀의 입력이 아니다"
                   "(측정 당시 개정 텍스트로 파생 · 서브 전달 파생도 그 개정 파일)",
                   "artifacts/compose/.env.late.template" not in rcr["files"]
                   and not any(e["file"] == "envs/.env.late" for e in rcr["env_shapes"])
                   and "artifacts/compose/late_runner.sh" not in rcr["files"]
                   and (rcr["sub_recipe"] or {}).get("role_diff", {}).get("slave_config_file") == "default"
                   and sub_recipe(repo, ev, render_module=fake_rd)["role_diff"]["slave_config_file"] == "default")
                srun = {r["path"]: r for r in cev["selected_revisions"]}
                ck("★러너 정본이 측정 뒤 바뀌어도 측정 전 커밋 = 서빙한 마운트본(mtime ≤ 측정)과 일치 → 차단 ✗ · 실린 러너 = 측정 당시 바이트",
                   (repo / "p_crev/artifacts/compose/serve_runner.sh").read_bytes() == runner0
                   and srun.get(asset_rel, {}).get("commit") == rev_c
                   and any("서빙한 러너" in x for x in srun.get(asset_rel, {}).get("cross_check", []))
                   and set(srun) >= {"output/multi/docker-compose.yaml", asset_rel})
                ck("공유 계약: compose 선택 행 = build_recipe 와 같은 자리(selected_revision · selected_revisions) · 문구에 compose 개정",
                   "compose = 측정 당시 개정 git:" in cev["note"] and isinstance(cev["selected_revisions"], list))
                # ★서빙한 마운트본(mtime ≤ 측정)이 실을 바이트와 다르면 = 이 셀의 러너가 아니다 → 차단
                (out / "configs/serve_runner.sh").write_bytes(runner0 + b"# served-different\n")
                _set_mtime(out / "configs/serve_runner.sh", "2026-01-01T10:00:00Z")
                raises("★서빙한 마운트본(mtime ≤ 측정) ≠ 실을 러너", "HINT_RUNNER_RENDER_DRIFT",
                       lambda: collect(repo, ev, repo / "p_crev_drift", render_module=fake_rd, runner=docker_old))
                (out / "configs/serve_runner.sh").write_bytes(runner0)
                _set_mtime(out / "configs/serve_runner.sh", "2026-01-01T10:00:00Z")
                # ★측정 시각 미관측이면 정본(작업트리)·마운트본을 그대로 대조한다(옛 드리프트 규칙 — 개정을 추측해 차단을 풀지 않는다).
                #   compose 는 측정 전 바이트로 둔다(측정 시각 미관측이면 작업트리 compose 를 파생에 쓴다 — late_runner 가 먼저 막는다).
                (out / "docker-compose.yaml").write_bytes(compose0)
                raises("★측정 시각 미관측 + 정본 ≠ 마운트본", "HINT_RUNNER_RENDER_DRIFT", lambda: collect(
                    repo, dict(ev, sweep={k: v for k, v in ev["sweep"].items() if k != "generated_utc"}), repo / "p_crev_nomeas",
                    render_module=fake_rd, runner=docker_old))
                # 러너 정본을 측정 전 바이트로 되돌린다 — 아래 '측정 시각 미관측' 수집은 정본·마운트본을 그대로 대조한다(옛 드리프트 규칙).
                (assets / "serve_runner.sh").write_bytes(runner0)
                _set_mtime(assets / "serve_runner.sh", "2026-01-01T00:00:00Z")
                # ★mtime ≤ 측정이면 그 바이트가 측정 당시 바이트다(미커밋 수정이 측정에 쓰였다) — 측정 전 커밋으로 되돌리지 않는다
                compose_wt = compose0 + "# 측정 전 미커밋 주석\n".encode("utf-8")
                (out / "docker-compose.yaml").write_bytes(compose_wt)
                _set_mtime(out / "docker-compose.yaml", "2026-01-01T10:00:00Z")
                rcw = collect(repo, ev, repo / "p_crev_wt", render_module=fake_rd, runner=docker_old)
                cw = rcw["slots"]["compose"]["evidence"]["selected_revision"]
                ck("★compose mtime ≤ 측정 = 작업트리(측정 당시 바이트 · verified) · 측정 전 커밋과 다르면 미커밋 사실 기재",
                   (repo / "p_crev_wt/artifacts/compose/docker-compose.yaml").read_bytes() == compose_wt
                   and cw["shipped"] == "worktree" and cw["basis"] == "mtime≤measured" and cw["verified"] is True
                   and cw["commit"] is None and "미커밋" in cw["method"])
                # ★측정 시각 미관측 = 작업트리 · 대조 불가 기재(개정을 추측하지 않는다)
                _set_mtime(out / "docker-compose.yaml", "2026-01-02T00:00:00Z")
                rcn = collect(repo, dict(ev, sweep={k: v for k, v in ev["sweep"].items() if k != "generated_utc"}),
                              repo / "p_crev_none", render_module=fake_rd, runner=docker_old)
                cn = rcn["slots"]["compose"]["evidence"]["selected_revision"]
                ck("★측정 시각 미관측 = compose 작업트리 · basis unobserved · '측정 시각 미관측' 기재",
                   cn["shipped"] == "worktree" and cn["basis"] == "unobserved" and cn["commit"] is None
                   and "측정 시각 미관측" in cn["method"]
                   and (repo / "p_crev_none/artifacts/compose/docker-compose.yaml").read_bytes() == compose_wt)
                # ★mtime 은 측정 뒤지만 바이트가 측정 전 커밋과 같다 = 같은 바이트(작업트리) · 개정 기재
                (out / "docker-compose.yaml").write_bytes(compose0)
                _set_mtime(out / "docker-compose.yaml", "2026-01-02T00:00:00Z")
                rcs = collect(repo, ev, repo / "p_crev_same", render_module=fake_rd, runner=docker_old)
                cs = rcs["slots"]["compose"]["evidence"]["selected_revision"]
                ck("compose mtime 만 뒤(바이트 = 측정 전 커밋) = 작업트리 · commit 기재 · git 개정 선택 방법",
                   cs["shipped"] == "worktree" and cs["commit"] == rev_c and cs["basis"] == "git-rev-before-measurement"
                   and "바이트가 그 개정과 같다" in cs["method"])
                # ★측정 전 커밋이 없는 비추적 파일이 측정 뒤 바뀌었으면 = 작업트리 + 경고(측정 당시 바이트라는 보장 없음)
                c_un = _ctx(repo, ev)
                un = _select_tracked(c_un, out / "envs/.env.late")
                _set_mtime(out / "envs/.env.late", "2026-01-02T00:00:00Z")
                un2 = _select_tracked(_ctx(repo, ev), out / "envs/.env.late")
                ck("★측정 전 개정 없는 파일: mtime ≤ 측정 = verified · 측정 뒤 = 작업트리 + 경고",
                   un["verified"] is True and un["basis"] == "mtime≤measured"
                   and un2["shipped"] == "worktree" and "warning" in un2 and un2["commit"] is None)
                # ── 2026-09-22 S2 round 3 적대 리뷰 ──
                # (a) 마운트본이 측정 **뒤** 다시 쓰였으면 서빙 증거가 아니다: 정본도 측정 뒤 바뀌었으면(측정 전 커밋을 싣는다) 대조를 건너뛰고
                #     그 사실을 적는다 · 정본이 측정 당시 바이트(작업트리)면 옛 렌더 드리프트 대조를 유지한다(★음성대조).
                #     동시에 정본의 GPU 합류 리터럴을 측정 뒤 바꿔 둔다 — sub_recipe 는 측정 당시 개정의 리터럴을 말해야 한다.
                (out / "docker-compose.yaml").write_bytes(compose0)
                _set_mtime(out / "docker-compose.yaml", "2026-01-01T00:00:00Z")
                (assets / "serve_runner.sh").write_bytes(runner0.replace(b'= "2"', b'= "4"').replace(b"0.0/2.0", b"0.0/4.0"))
                _set_mtime(assets / "serve_runner.sh", "2026-01-02T00:00:00Z")
                (out / "configs/serve_runner.sh").write_bytes(runner0 + "# 측정 뒤 재materialize\n".encode("utf-8"))
                _set_mtime(out / "configs/serve_runner.sh", "2026-01-02T00:00:00Z")
                rms = collect(repo, ev, repo / "p_crev_mnt", render_module=fake_rd, runner=docker_old)
                msr = {r["path"]: r for r in rms["slots"]["compose"]["evidence"]["selected_revisions"]}.get(asset_rel, {})
                ck("★마운트본 측정 뒤 재작성 + 정본 측정 뒤 수정 = 차단 ✗(서빙 증거 아님 기재) · 실린 러너 = 측정 전 커밋 바이트",
                   (repo / "p_crev_mnt/artifacts/compose/serve_runner.sh").read_bytes() == runner0
                   and msr.get("commit") == rev_c and any("서빙 증거 아님" in x for x in msr.get("cross_check", [])))
                ck("★sub_recipe GPU 합류 리터럴 = 측정 당시 개정의 러너(정본이 측정 뒤 \"4\" 로 바뀌어도 \"2\")",
                   (rms["sub_recipe"] or {}).get("role_diff", {}).get("runner_gpu_join_literal", {}).get("value") == "2"
                   and sub_recipe(repo, ev, render_module=fake_rd)["role_diff"]["runner_gpu_join_literal"]["value"] == "2")
                (assets / "serve_runner.sh").write_bytes(runner0)
                _set_mtime(assets / "serve_runner.sh", "2026-01-01T00:00:00Z")
                raises("★마운트본 측정 뒤 재작성 + 정본 = 측정 당시 바이트(작업트리) 인데 다르다 → 렌더 드리프트 차단 유지",
                       "HINT_RUNNER_RENDER_DRIFT",
                       lambda: collect(repo, ev, repo / "p_crev_mnt2", render_module=fake_rd, runner=docker_old))
                (out / "configs/serve_runner.sh").write_bytes(runner0)
                _set_mtime(out / "configs/serve_runner.sh", "2026-01-01T00:00:00Z")
                # (b) render 단계: 렌더러가 측정 뒤 바뀌었다는 git 관측이 있을 때만 명령 앞에 경고 주석 · 측정 전 개정(★관측 없으면 경고 ✗)
                rd_path = repo / core.REL_RENDER_DOCKERFILE
                rd0 = rd_path.read_bytes()
                try:
                    _set_mtime(rd_path, "2026-01-02T00:00:00Z")
                    st0 = {s["step"]: s for s in reproduce_steps(repo, ev, runner=docker_old)}["render"]
                    ck("★렌더러 측정 전 개정 없음(비추적 · mtime 만 뒤) = 드리프트 경고 ✗(지어내지 않는다) · 행에 경고 사실",
                       st0["command"].startswith("python3 ") and "⚠" not in st0["command"]
                       and (st0.get("renderer_revision") or {}).get("commit", "x") is None
                       and "warning" in (st0.get("renderer_revision") or {}))
                    core.git(repo, "add", core.REL_RENDER_DOCKERFILE, env_extra=genv)
                    core.git(repo, "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m",
                             "renderer", env_extra=genv)
                    rev_r = core.git_out(repo, "rev-parse", "HEAD")
                    _set_mtime(rd_path, "2026-01-01T00:00:00Z")
                    st1 = {s["step"]: s for s in reproduce_steps(repo, ev, runner=docker_old)}["render"]
                    rr1 = st1.get("renderer_revision") or {}
                    ck("★렌더러 mtime ≤ 측정 = 측정 당시 렌더러 · 경고 ✗", "⚠" not in st1["command"]
                       and rr1.get("basis") == "mtime≤measured" and rr1.get("commit") == rev_r)
                    rd_path.write_bytes(rd0 + "# 측정 뒤 렌더러 수정(소켓 기준선 흉내)\n".encode("utf-8"))
                    core.git(repo, "add", core.REL_RENDER_DOCKERFILE, env_extra=genv3)
                    core.git(repo, "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m",
                             "late renderer", env_extra=genv3)
                    late_r = core.git_out(repo, "rev-parse", "HEAD")
                    _set_mtime(rd_path, "2026-01-02T00:00:00Z")
                    st2 = {s["step"]: s for s in reproduce_steps(repo, ev, runner=docker_old)}["render"]
                    l2 = st2["command"].split("\n")
                    ck("★렌더러가 측정 뒤 바뀌었으면(git 관측) render 명령 앞에 경고 주석 · 측정 전 커밋 · 측정 뒤 커밋 수 · git show",
                       l2[0].startswith("# ⚠ 측정 당시 렌더러 ≠ 작업트리 렌더러") and rev_r[:12] in l2[0]
                       and "측정 뒤 커밋 1건" in l2[0] and late_r[:12] in l2[0]
                       and f"git show {rev_r[:12]}:{core.REL_RENDER_DOCKERFILE}" in l2[1]
                       and ENV_OBSERVED_POINTER in l2[1] and not re.search(r"(?<![A-Za-z_])facts\.", "\n".join(l2))
                       and any(x.startswith("python3 ") and "--nccl-envfile" in x for x in l2[2:])
                       and (st2.get("renderer_revision") or {}).get("commit") == rev_r
                       and (st2.get("renderer_revision") or {}).get("commits_after_measurement") == [late_r[:12]]
                       and "bytes" not in (st2.get("renderer_revision") or {"bytes": 1}))
                finally:
                    rd_path.write_bytes(rd0)
                    _set_mtime(rd_path, "2026-01-01T00:00:00Z")
            finally:
                # 되돌림 — 이후 시험은 측정 전 바이트·mtime 으로 돈다(중간 실패여도 · 잔재가 뒤 시험을 오염시키지 않게)
                (assets / "serve_runner.sh").write_bytes(runner0)
                (out / "configs/serve_runner.sh").write_bytes(runner0)
                (out / "docker-compose.yaml").write_bytes(compose0)
                if (out / "envs/.env.late").exists():
                    (out / "envs/.env.late").unlink()
                for p_ in (out / "docker-compose.yaml", assets / "serve_runner.sh", out / "configs/serve_runner.sh"):
                    _set_mtime(p_, "2026-01-01T00:00:00Z")
        except core.HintError as e:
            bad.append(f"artifacts: 임시 git 픽스처 실패 {e.code}: {e.message[:300]}")
        # ★트리플렛 부재 = 차단(면제 불가) · 실패한 호출은 artifacts/ 를 남기지 않는다
        raises("★트리플렛 부재", "HINT_TRIPLET_ABSENT", lambda: collect(
            repo, dict(ev, triplet={"env": "output/multi/envs/.env.none", "yaml": f"output/multi/configs/{cell}.yaml",
                                    "sh": f"output/multi/configs/{cell}.sh"}),
            repo / "p5", render_module=fake_rd, runner=docker_old))
        ck("★실패한 수집은 artifacts/ 를 남기지 않는다", not (repo / "p2" / "artifacts").exists()
           and not (repo / "p3" / "artifacts").exists())
        # 빌드 컨텍스트 원천: ADD(로컬) 도 싣는다 · 하위 디렉터리 파일도 원 경로로 대조 · 글롭/원격 원천은 싣지 않되 기재
        (out / "Dockerfile.ctx").write_text(
            "FROM base\nARG SM12X_PORT=0\nCOPY requirements.txt /tmp/requirements.txt\nADD extra.txt /x\n"
            "COPY sub/extra2.txt /y\nCOPY *.cfg /z\nADD https://example.invalid/x.tar /w\n"
            "COPY build_patches_src/ /tmp/build_patches_src/\n", encoding="utf-8")
        (out / "extra.txt").write_text("x\n", encoding="utf-8")
        (out / "sub").mkdir(exist_ok=True)
        (out / "sub/extra2.txt").write_text("y\n", encoding="utf-8")
        (out / "envs/.env.ctx").write_text("IMAGE_TAG=easy-vllm:fx-source\nBUILD_DOCKERFILE=Dockerfile.ctx\n", encoding="utf-8")
        rcx = collect(repo, dict(ev, triplet={"env": "output/multi/envs/.env.ctx", "yaml": f"output/multi/configs/{cell}.yaml",
                                              "sh": f"output/multi/configs/{cell}.sh"}),
                      repo / "p_ctx", render_module=fake_rd, runner=docker_old)
        bre = rcx["slots"]["build_recipe"]["evidence"]
        ck("ADD·하위 디렉터리 COPY 원천 실림 · ★글롭은 기재만 · 원격 ADD 는 컨텍스트 아님",
           {"artifacts/build_recipe/extra.txt", "artifacts/build_recipe/extra2.txt",
            "artifacts/build_recipe/Dockerfile.ctx"} <= set(rcx["files"])
           and [x["source"] for x in bre.get("context_not_shipped", [])] == ["*.cfg"]
           and "output/multi/sub/extra2.txt" in {r["path"] for r in bre["recipe_vs_image"]})
        # fork_pin — VARIANT 한 줄 = Band2 원장 좌표(옛 자체검사 ★VARIANT detection 이관 · 키/유일 variant_id)
        (repo / REL_ARCH_VARIANT_LEDGER).parent.mkdir(parents=True, exist_ok=True)
        (repo / REL_ARCH_VARIANT_LEDGER).write_text(json.dumps({"source_build_variants": {
            "_note": "x", "fx-key": {"variant_id": "source-fx-0.1.0", "vllm_repo": "https://example.invalid/v.git",
                                    "vllm_ref": "v0.1.0", "image_tag": "easy-vllm:fx-source"}}}), encoding="utf-8")
        base_env = (out / "envs" / f".env.{cell}").read_text(encoding="utf-8")
        for i, (label, line, code) in enumerate((("키", "VARIANT=fx-key", None),
                                                 ("유일 variant_id", "VARIANT=source-fx-0.1.0", None),
                                                 ("★원장 밖 변종", "VARIANT=nope", "HINT_FORK_PIN_UNBOUND"),
                                                 ("★예약 키", "VARIANT=_note", "HINT_FORK_PIN_UNBOUND"))):
            (out / "envs" / f".env.{cell}").write_text(base_env + line + "\n", encoding="utf-8")
            dest = repo / f"p_fp_{i}"
            if code:
                raises(f"fork_pin {label}", code, lambda d=dest: collect(repo, ev, d, render_module=fake_rd, runner=docker_old))
            else:
                try:
                    rfp = collect(repo, ev, dest, render_module=fake_rd, runner=docker_old)
                    ck(f"fork_pin {label} → variant.json", "artifacts/fork_pin/variant.json" in rfp["files"]
                       and rfp["slots"]["fork_pin"]["applicable"] is True)
                except core.HintError as e:
                    ck(f"fork_pin {label} → variant.json(예외 {e.code})", False)
        (out / "envs" / f".env.{cell}").write_text(base_env.replace("VLLM_REF=v0.1.0", "VLLM_REF=v0.1.9") + "VARIANT=fx-key\n",
                                                   encoding="utf-8")
        raises("★변종 vllm_ref ≠ 셀 env VLLM_REF", "HINT_FORK_PIN_MISMATCH",
               lambda: collect(repo, ev, repo / "p_fp_mm", render_module=fake_rd, runner=docker_old))
        (out / "envs" / f".env.{cell}").write_text(base_env, encoding="utf-8")
        _set_mtime(out / "envs" / f".env.{cell}", "2026-01-01T00:00:00Z")
        # ★평면 파생 불가 = 차단(옛 코드는 신호가 없으면 native 로 추측했다)
        (out / "envs/.env.bare").write_text("CONFIG_FILE=x\n", encoding="utf-8")
        ev_bare = dict(ev, build_identity={}, triplet={"env": "output/multi/envs/.env.bare",
                                                        "yaml": f"output/multi/configs/{cell}.yaml",
                                                        "sh": f"output/multi/configs/{cell}.sh"})
        raises("★평면 파생 불가", "HINT_PLANE_UNDERIVABLE", lambda: plane_of(repo, ev_bare))
        ck("선언 native + 신호 없음 = native", plane_of(repo, dict(ev_bare, plane="native")) == "native")
        # 출력 root symlink는 증거를 읽기 전에 차단한다. 외부 빈 디렉터리여도 draft 밖 write 0.
        outside = repo / "outside-artifacts"; outside.mkdir()
        sy_payload = repo / "p_symlink"; sy_payload.mkdir()
        (sy_payload / "artifacts").symlink_to(outside, target_is_directory=True)
        raises("★artifacts 출력 symlink = draft 밖 write 전 차단", "HINT_ARTIFACTS_OUTPUT_SYMLINK",
               lambda: collect(repo, dict(ev_bare, plane="native"), sy_payload, render_module=fake_rd, runner=docker_old))
        ck("★출력 symlink 차단 뒤 외부 잔재 0", not any(outside.iterdir()))
        (repo / "output/multi/native-install.sh").write_text("python -m pip install vllm\n", encoding="utf-8")
        (repo / "output/multi/native-freeze-main.txt").write_text("vllm==0.1.0\n", encoding="utf-8")
        (repo / "output/multi/native-freeze-sub.txt").write_text("vllm==0.1.0\n", encoding="utf-8")
        native_ev = dict(ev_bare, plane="native", native_install_path="output/multi/native-install.sh",
                         pip_freeze_path="output/multi/native-freeze-main.txt",
                         pip_freeze_paths={"main": "output/multi/native-freeze-main.txt", "sub": "output/multi/native-freeze-sub.txt"})
        rn = collect(repo, native_ev, repo / "p_nat", render_module=fake_rd, runner=docker_old)
        nn = set(rn["files"])
        ck("native 평면: producer install+양 node lock 동봉 · ★Dockerfile·compose·패치 0 · 러너 중복 ✗",
           {"artifacts/build_recipe/native-install.sh", "artifacts/build_recipe/pip-freeze-main.txt",
            "artifacts/build_recipe/pip-freeze-sub.txt"} <= nn and not any(x.startswith(("artifacts/compose/",
                                                                                           "artifacts/build_patch"))
                                                                      or "Dockerfile" in x for x in nn) and rn["applied_set"] is None)
        raises("★native producer install 신호 부재 차단", "HINT_NATIVE_EVIDENCE_MISSING",
               lambda: collect(repo, dict(native_ev, native_install_path=None), repo / "p_nat_missing",
                               render_module=fake_rd, runner=docker_old))
        # ★결손 코드는 evidence.MISSING_CODES 사전에서만(SPEC §2.1): 등재돼 있으면 결손으로 기재되고, 아니면 **소리 내어**
        #   막힌다(HINT_MISSING_CODE_UNREGISTERED) — 어느 쪽이든 "미등록" 으로 조용히 배포되는 경로는 없다. 두 분기를 모두 단언한다
        #   (등재 여부는 evidence 소유자 몫 · 한쪽만 단언하면 등재되는 날 이 시험이 뒤집혀 죽는다).
        from . import evidence as _ev_mod
        raises("★미등재 결손 코드 = tripwire(직접)", "HINT_MISSING_CODE_UNREGISTERED",
               lambda: _require_registered(repo, ["HINT_MISSING_BUILD_LEDGER", "HINT_MISSING_FIXTURE_NOT_A_CODE"]))
        emitted = set(re.findall(r'missing\.append\("([A-Z0-9_]+)"\)', Path(__file__).read_text(encoding="utf-8")))
        ck(f"artifacts 가 내는 결손 코드 전부 evidence.MISSING_CODES 등재: {sorted(emitted - set(_ev_mod.MISSING_CODES))}",
           bool(emitted) and emitted <= set(getattr(_ev_mod, "MISSING_CODES", {})))
        raises("★native pip lock 신호 부재 차단", "HINT_NATIVE_EVIDENCE_MISSING",
               lambda: collect(repo, dict(native_ev, pip_freeze_paths={}), repo / "p_nat2", render_module=fake_rd,
                               runner=docker_old))
        ck("★native evidence 차단 뒤 artifacts 잔재 없음", not (repo / "p_nat2" / "artifacts").exists())
        # ★형상 부재는 결손으로 **기재**한다(조용히 빼지 않는다) — 어느 파일인지는 env_shapes 행이 말한다
        (out / "envs/.env.cluster").rename(out / "envs/.env.cluster.off")
        (out / ".env").rename(out / ".env.off")
        rc_ = collect(repo, ev, repo / "p_nocl", render_module=fake_rd, runner=docker_old)
        absent_rows = sorted(e["file"] for e in rc_["env_shapes"] if e.get("status") == "absent")
        ck("★.env.cluster·프로젝트 .env 부재 = 결손 기재 + 형상 표에 부재 행", "HINT_MISSING_ENV_SHAPE" in rc_["missing"]
           and absent_rows == [".env", "envs/.env.cluster"])
        (out / "envs/.env.cluster.off").rename(out / "envs/.env.cluster")
        (out / ".env.off").rename(out / ".env")
        raises("★선언·관측 평면 모순", "HINT_PLANE_MISMATCH", lambda: plane_of(repo, dict(ev, plane="native")))

        # ── F2(2026-09-22 S2 round 2 → round 3): 측정 뒤 재생성된 env 형상 = **싣지 않는다**(excluded · post-measurement-regenerated) ──
        #   round 2 는 값만 가리고 키를 실었다 — 재생성 시점 렌더러가 측정 뒤 더한 키가 이 셀의 형상인 척 실렸다(태그2 NCCL_NET 등).
        _set_mtime(out / "envs/.env.interconnect", "2026-01-02T00:00:00Z")     # 측정(스윕 generated 2026-01-01T19:30Z) 뒤
        _set_mtime(out / ".env", "2026-01-02T00:00:00Z")
        _set_mtime(out / "configs" / f"{cell}.yaml", "2026-01-02T00:00:00Z")
        rg2 = collect(repo, ev, repo / "p_regen", render_module=fake_rd, runner=docker_old)
        clr2 = (repo / "p_regen/artifacts/compose/.env.cluster.template").read_text(encoding="utf-8")
        exr = {x["file"]: x for x in rg2["slots"]["compose"].get("excluded", [])}
        ck("★F2 측정 뒤 재생성 형상(interconnect · 프로젝트 .env) = 미실림 · 파일 목록·트리 모두에서 없다",
           not {"artifacts/compose/.env.interconnect.template", "artifacts/compose/.env.template"} & set(rg2["files"])
           and not (repo / "p_regen/artifacts/compose/.env.interconnect.template").exists()
           and not (repo / "p_regen/artifacts/compose/.env.template").exists())
        ck("F2 공유 계약: slots.compose.excluded reason = post-measurement-regenerated(why 첫 토큰 같음 · 측정·mtime 기재)",
           exr.get("envs/.env.interconnect", {}).get("reason") == POST_MEASUREMENT_REGENERATED
           and exr.get(".env", {}).get("reason") == POST_MEASUREMENT_REGENERATED
           and exr["envs/.env.interconnect"]["why"].split("(")[0] == POST_MEASUREMENT_REGENERATED
           and exr["envs/.env.interconnect"]["mtime_utc"] == "2026-01-02T00:00:00Z"
           and exr["envs/.env.interconnect"]["measured_utc"] == "2026-01-01T19:30:00Z"
           and "measurement_env_observed" in exr["envs/.env.interconnect"]["why"])
        # 2026-09-22 S2 round 3 통합: 배포 본문의 좌표는 zip 안의 자리다 — 발행기 내부 키(`facts.`)를 수신자에게 싣지 않는다
        sr2 = (repo / "p_regen/artifacts/compose/sub_recipe.json")
        shipped_txt = core.dumps(rg2["slots"]["compose"]) + (sr2.read_text(encoding="utf-8") if sr2.exists() else "")
        ck("★F2 측정 당시 env 좌표 = zip 안의 자리(01 §1.4 · PAYLOAD.json) · 내부 키 `facts.` 를 배포 문구에 싣지 않는다",
           ENV_OBSERVED_POINTER in exr["envs/.env.interconnect"]["why"]
           and not re.search(r"(?<![A-Za-z_])facts\.", shipped_txt)          # `artifacts.` 는 내부 키가 아니다(경계 필수)
           and (not sr2.exists() or ENV_OBSERVED_POINTER in sr2.read_text(encoding="utf-8")))
        ck("★F2 측정 전 파일(cluster) = 실림 · 값 유지 + mtime 대조 사실 기재 · excluded 에 없다",
           "artifacts/compose/.env.cluster.template" in rg2["files"] and "MAX_JOBS=4" in clr2 and "mtime ≤ 측정" in clr2
           and "envs/.env.cluster" not in exr)
        ck("F2 공유 계약: slots.compose.evidence.post_measurement_regenerated(저장소 상대 경로) · env_shapes 행 = excluded",
           rg2["slots"]["compose"]["evidence"]["post_measurement_regenerated"] ==
           ["output/multi/.env", "output/multi/envs/.env.interconnect"]
           and {e["file"]: (e.get("post_measurement_regenerated"), e.get("status")) for e in rg2["env_shapes"]} ==
           {"envs/.env.interconnect": (True, "excluded"), "envs/.env.cluster": (False, None), ".env": (True, "excluded")}
           and "형상 미실림" in rg2["slots"]["compose"]["evidence"]["note"])
        ck("F2 트리플렛은 가리지 않고 재생성 사실만 적는다", rg2["slots"]["triplet"]["evidence"]["post_measurement_regenerated"]
           == [f"output/multi/configs/{cell}.yaml"] and (repo / f"p_regen/artifacts/triplet/{cell}.yaml").read_text(
               encoding="utf-8") == (out / "configs" / f"{cell}.yaml").read_text(encoding="utf-8"))
        ck("★F2 재생성 형상을 싣지 않으면 sub_recipe 가 그 템플릿을 가리키지 않는다 · 빠진 이유를 스스로 말한다 · 실린 cluster 표지는 남는다",
           "interconnect" not in (rg2["sub_recipe"] or {}).get("per_node_values", {})
           and [x["file"] for x in (rg2["sub_recipe"] or {}).get("env_shapes_not_shipped", [])] == ["envs/.env.interconnect"]
           and str((rg2["sub_recipe"] or {}).get("per_node_values", {}).get("MASTER_HOST_IP", "")).startswith("<manifest"))
        ck("F2 공개 sub_recipe() 도 같은 규칙(재생성 interconnect 미참조)",
           "interconnect" not in sub_recipe(repo, ev, render_module=fake_rd)["per_node_values"])
        rg3 = collect(repo, dict(ev, certificate={"measured_utc": "2026-01-03T00:00:00Z"}), repo / "p_regen_cert",
                      render_module=fake_rd, runner=docker_old)
        ck("★F2 측정 시각 = 인증서 measured_utc 우선(그보다 이른 mtime 은 재생성 아님 → 실린다)",
           rg3["slots"]["compose"]["evidence"]["post_measurement_regenerated"] == []
           and "artifacts/compose/.env.interconnect.template" in rg3["files"]
           and not any(x.get("reason") == POST_MEASUREMENT_REGENERATED for x in rg3["slots"]["compose"].get("excluded", [])))
        rg4 = collect(repo, dict(ev, sweep={k: v for k, v in ev["sweep"].items() if k != "generated_utc"}),
                      repo / "p_regen_none", render_module=fake_rd, runner=docker_old)
        ic4 = (repo / "p_regen_none/artifacts/compose/.env.interconnect.template").read_text(encoding="utf-8")
        ck("★F2 측정 시각 미관측 = 재생성 판정 ✗(실린다) · 대조하지 못했다고 적는다", rg4["slots"]["compose"]["evidence"]
           ["post_measurement_regenerated"] == [] and "측정 시각 미관측" in ic4 and "NCCL_DEBUG=INFO" in ic4)
        # 형상화 함수 자체(공개 env_shape)의 재생성 표시는 유지된다 — 싣지 않는 결정은 수집(collect)의 몫이다.
        ish = env_shape(repo, out / "envs/.env.interconnect", "interconnect", render_module=fake_rd,
                        measured_utc="2026-01-01T19:30:00Z")
        ck("env_shape(measured_utc) = 측정 후 재생성 표시 · 리터럴 값 미기재(형상화 함수 · 발행 여부와 별개)",
           f"NCCL_DEBUG={REGENERATED_VALUE}" in ish and "재현 사실" not in ish)
        # 경계(2026-09-22 S2 round 3 적대 리뷰): mtime == 측정 시각(초 단위)은 측정 **뒤** 가 아니다 — 재생성 판정 ✗(엄격히 뒤만 뺀다)
        _set_mtime(out / "envs/.env.interconnect", "2026-01-01T19:30:00Z")
        ck("★F2 경계: mtime == 측정 시각 = 재생성 아님 · 1초 뒤(.env 01-02) = 재생성",
           _regenerated(_ctx(repo, ev), out / "envs/.env.interconnect") is None
           and _regenerated(_ctx(repo, ev), out / ".env") is not None)
        for p in (out / "envs/.env.interconnect", out / ".env", out / "configs" / f"{cell}.yaml"):
            _set_mtime(p, "2026-01-01T00:00:00Z")

        # ── F3(2026-09-22 S2 round 2): arm_patch.sh 는 이 셀이 런타임 패치를 무장할 때만 ──
        (out / "configs" / f"{cell}_patch.py").rename(out / "configs" / f"{cell}_patch.py.off")
        rna = collect(repo, ev, repo / "p_noarm", render_module=fake_rd, runner=docker_old)
        ck("★F3 무장할 런타임 패치 없음 = arm_patch.sh 미실림 · compose excluded not-armed · serve_runner 는 실림",
           "artifacts/compose/arm_patch.sh" not in rna["files"] and "artifacts/compose/serve_runner.sh" in rna["files"]
           and any(x["file"] == "arm_patch.sh" and x["why"].startswith("not-armed")
                   for x in rna["slots"]["compose"].get("excluded", [])))
        (out / "configs/default_patch.py").write_text("# slave runtime patch\n", encoding="utf-8")
        rsa = collect(repo, ev, repo / "p_slavearm", render_module=fake_rd, runner=docker_old)
        ck("F3 slave(CONFIG_FILE=default) 가 무장하면 싣는다", "artifacts/compose/arm_patch.sh" in rsa["files"])
        (out / "configs/default_patch.py").unlink()
        (out / "configs" / f"{cell}_patch.py.off").rename(out / "configs" / f"{cell}_patch.py")
        ck("F3 무장 러너 이름 = 소유자 RUNNER_SCRIPTS 원소(tripwire 교차대조)",
           RUNTIME_PATCH_ARMER in tuple(getattr(_render_module(repo), "RUNNER_SCRIPTS", ())))

        # ── F5 벤치 명령 음성대조 ──
        (repo / "output/multi/benchlog/sweep_nonuni/level_01").mkdir(parents=True)
        (repo / "output/multi/benchlog/sweep_nonuni/level_01/bench.json").write_text(json.dumps(
            {"date": "20260101-190500", "backend": "openai", "num_prompts": 4, "max_concurrency": 2, "completed": 4,
             "total_input_tokens": 4097, "total_output_tokens": 1024}), encoding="utf-8")
        (repo / "output/multi/benchlog/sweep_empty").mkdir(parents=True)
        cnu = _ctx(repo, dict(ev, sweep={"dir": "output/multi/benchlog/sweep_nonuni"}))
        cmd_nu, _src_nu = _bench_commands(cnu)
        ck("★F5 요청 길이 불균일 = 데이터셋 인자를 지어내지 않는다(명령 줄 · 머리 주석은 규칙 설명)",
           not any("--dataset-name" in ln for ln in cmd_nu.splitlines() if not ln.startswith("#"))
           and any(ln.startswith("vllm bench serve") for ln in cmd_nu.splitlines())
           and "--max-concurrency 2" in cmd_nu and "불균일" in cmd_nu)
        cmd_e, src_e = _bench_commands(_ctx(repo, dict(ev, sweep={"dir": "output/multi/benchlog/sweep_empty"})))
        ck("★F5 bench JSON 없음 = 명령 비움 · 출처 unobserved(자리표시 ✗)", cmd_e == "" and src_e.startswith("unobserved("))
        # ★출력 평면에 묶이지 않은 스윕(simlog 사본)은 재구성하지 않는다 — evidence.bench_command 와 같은 규칙(두 생산자가 갈리면
        #   hint.py 가 이쪽을 우선해 evidence 의 거부를 뒤집는다 · 2026-09-22 S2 round 2 리뷰). binding=output 은 재구성한다.
        cmd_b, src_b = _bench_commands(_ctx(repo, dict(ev, sweep=dict(ev["sweep"], binding="simlog-copy"))))
        cmd_o, src_o = _bench_commands(_ctx(repo, dict(ev, sweep=dict(ev["sweep"], binding="output"))))
        ck("★F5 스윕 binding≠output = 재구성 ✗(unobserved) · output = 재구성", cmd_b == "" and "binding='simlog-copy'" in src_b
           and "vllm bench serve" in cmd_o and src_o == "reconstructed(bench json)")

        # ── F12(2026-09-22 S2 round 2): 원장 없는 옛 이미지 재구성 v2 — 정적 분석 × 이미지 탐침(대역 docker) ──
        #   격리 평면(output/single)에 태그2 모양의 패치 집합을 둔다: 50 게이트 skip · 55 게이트 열림 · 56 일부만 열림 ·
        #   60 fail-loud(마커) · 61 작업트리≠이미지 · 62 이미지에만 · 63 작업트리에만 · 10 fail-loud(`sys.exit(0 if…)`) ·
        #   15 set -e 없음 · 16 `exit $rc` · 20 내용 조건부(마커 없음) · 30 내용 조건부(grep -qF 마커) · 40 모듈 대상 assert 마커.
        so = repo / "output" / "single"
        for d in ("configs", "envs", "build_patches_src", "build_patches"):
            (so / d).mkdir(parents=True, exist_ok=True)
        pc = "p1"
        (so / "configs" / f"{pc}.yaml").write_text("model: /app/models/P\nmax-model-len: 4096\n", encoding="utf-8")
        (so / "configs" / f"{pc}.sh").write_text(
            "#!/bin/bash\nif [ -f /app/configs/arm_patch.sh ]; then source /app/configs/arm_patch.sh; fi\nvllm serve\n",
            encoding="utf-8")
        (so / "envs" / f".env.{pc}").write_text("IMAGE_TAG=easy-vllm:fx-probe\nBUILD_DOCKERFILE=Dockerfile.probe\n",
                                                encoding="utf-8")
        (so / "docker-compose.yaml").write_text(
            "x-gpu-common: &gpu-common\n  image: ${IMAGE_TAG:-easy-vllm:fx-probe}\n  build:\n    context: .\n"
            "    dockerfile: ${BUILD_DOCKERFILE:-Dockerfile.probe}\n    args:\n      SM12X_PORT: ${SM12X_PORT:-0}\n"
            "services:\n  vllm:\n    <<: *gpu-common\n    env_file:\n      - path: envs/.env.${CONFIG_FILE:-default}\n"
            f"    command: [\"bash\", \"/app/configs/{pc}.sh\"]\n", encoding="utf-8")
        (so / "Dockerfile.probe").write_text(
            "FROM base\nARG SM12X_PORT=0\nARG FX_OPEN=0\nARG FX_SHUT=0\nCOPY requirements.txt /tmp/requirements.txt\n"
            "COPY build_patches_src/ /tmp/build_patches_src/\n"
            "RUN for p in $(ls /tmp/build_patches_src/*.sh 2>/dev/null | sort); do bash \"$p\" || exit 1; done\n"
            "COPY build_patches/ /tmp/build_patches/\n"
            "RUN for p in $(ls /tmp/build_patches/*.sh 2>/dev/null | sort); do bash \"$p\" || exit 1; done\n", encoding="utf-8")
        req = b"transformers<5.15.0\n"
        (so / "requirements.txt").write_bytes(req)
        loud = '#!/bin/bash\nset -euo pipefail\nDST="/workspace/vllm-src"\necho "[{n}] apply"\n'
        sp = {
            "50-gate.sh": ('#!/bin/bash\nset -euo pipefail\n: "${SM12X_PORT:=0}"\nif [ "${SM12X_PORT}" != "1" ]; then\n'
                           '    echo "[50] skip — SM12X_PORT=0"\n    exit 0\nfi\necho applying\n'),
            "55-open.sh": ('#!/bin/bash\nset -euo pipefail\n: "${FX_OPEN:=0}"\nif [ "${FX_OPEN}" != "1" ]; then\n'
                           '    echo "[55] skip — FX_OPEN=0"\n    exit 0\nfi\necho "[55] applying"\n'),
            "56-two.sh": ('#!/bin/bash\nset -euo pipefail\n: "${FX_OPEN:=0}"\n: "${FX_SHUT:=0}"\n'
                          'if [ "${FX_OPEN}" != "1" ]; then\n    exit 0\nfi\nif [ "${FX_SHUT}" != "1" ]; then\n    exit 0\nfi\n'
                          'echo "[56] applying"\n'),
            "60-loud.sh": ('#!/bin/bash\nset -euo pipefail\nTAG="[60]"\nDST="/workspace/vllm-src"\n'
                           '[ -d "$DST/vllm" ] || { echo "$TAG FAIL" >&2; exit 1; }\npython3 - "$DST" <<\'PY\'\nimport sys\n'
                           'p = sys.argv[1] + "/vllm/a.py"\ns = open(p).read()\nif "anchor" not in s:\n    raise SystemExit(1)\n'
                           'open(p, "w").write(s.replace("anchor", "anchor  # [port:60-fx]"))\nPY\n'
                           'grep -q \'port:60-fx\' "$DST/vllm/a.py" || { echo "$TAG FAIL: 마커 부재" >&2; exit 1; }\n'
                           'if grep -q \'already-upstream\' "$DST/vllm/a.py"; then echo "$TAG FAIL" >&2; exit 1; fi\n'),
            "61-drift.sh": '#!/bin/bash\nset -e\nif [ -f /x ]; then exit 0; fi\necho "[61] apply"\n',
            "63-notbaked.sh": loud.format(n=63),
        }
        pp = {
            "10-loud.sh": ('#!/bin/bash\nset -e\nrm -rf -- /tmp/fx-build 2>/dev/null || true\n'
                           'python3 -c "import importlib.util,sys; sys.exit(0 if importlib.util.find_spec(\'fx\') else 1)" \\\n'
                           '    && echo "[10] OK"\n'),
            "15-noe.sh": '#!/bin/bash\necho "[15] apply"\n',
            "16-var.sh": '#!/bin/bash\nset -e\nrc=0\nexit $rc\n',
            "20-cond.sh": ('#!/bin/bash\nset -e\nLOC=$(python3 -c "print(\'\')" 2>/dev/null || true)\ncase "$LOC" in\n'
                           '    "") echo "[20] 부재 — skip(이미 vendored)" ;;\n    *) pip uninstall -y fx >/dev/null 2>&1 || true ;;\n'
                           'esac\npython3 -c "\nassert has_it(), \'FAIL\'\n"\n'),
            "30-cond.sh": ('#!/bin/bash\nset -e\nF=/workspace/vllm-src/gate.py\n[ -f "$F" ] || { echo FAIL >&2; exit 1; }\n'
                           'python3 - "$F" <<\'PY\'\nimport sys\nf = sys.argv[1]; s = open(f).read()\nif "< (13, 0)" in s:\n'
                           '    print("[30] 이미 완화됨 — skip")\nelif "< (11, 0)" in s:\n'
                           '    open(f, "w").write(s.replace("< (11, 0)", "< (13, 0)"))\nelse:\n    sys.exit(1)\nPY\n'
                           'grep -qF "(9, 0) <= cap < (13, 0)" "$F" && echo "[30] OK"\n'),
            "40-mod.sh": ('#!/bin/bash\nset -e\nF=$(python3 -c "import fxmod.dev as d; print(d.__file__)" 2>/dev/null || true)\n'
                          '[ -n "$F" ] && [ -f "$F" ] || { echo FAIL >&2; exit 1; }\npython3 - "$F" <<\'PY\'\nimport sys\n'
                          'f = sys.argv[1]; s = open(f).read()\nif "[GB10] guard-a" in s:\n'
                          '    print("[40] 이미 가드됨 — skip"); sys.exit(0)\nopen(f, "w").write(s + "\\n# [GB10] guard-a\\n")\nPY\n'
                          'python3 -c "\nimport fxmod.dev as d\nsrc = open(d.__file__).read()\n'
                          'assert \'[GB10] guard-a\' in src, \'guard 미적용\'\n"\n'),
        }
        for name, text in sp.items():
            (so / "build_patches_src" / name).write_text(text, encoding="utf-8")
        for name, text in pp.items():
            (so / "build_patches" / name).write_text(text, encoding="utf-8")
        baked61 = loud.format(n=61).encode("utf-8")
        baked62 = loud.format(n=62).encode("utf-8")
        img_fs = {
            "/tmp/build_patches_src": {**{n: t.encode("utf-8") for n, t in sp.items() if n not in ("61-drift.sh", "63-notbaked.sh")},
                                       "61-drift.sh": baked61, "62-bakedonly.sh": baked62, "PROVENANCE.json": b"{}"},
            "/tmp/build_patches": {n: t.encode("utf-8") for n, t in pp.items()},
            "/tmp/requirements.txt": req,
            "/workspace/vllm-src/vllm/a.py": b"anchor  # [port:60-fx]\n",
            "/workspace/vllm-src/gate.py": b"if (9, 0) <= cap < (13, 0):\n",
            "/usr/lib/pyfx/dist-packages/fxmod/dev.py": b"x = 1\n# [GB10] guard-a\n",
        }
        args3 = "RUN |3 SM12X_PORT=0 FX_OPEN=1 FX_SHUT=0 /bin/sh -c "
        lay_p = [(args3 + 'for p in $(ls /tmp/build_patches/*.sh 2>/dev/null | sort); do bash "$p" || exit 1; done # buildkit',
                  "2026-01-02T03:04:00+09:00"),
                 ("COPY build_patches/ /tmp/build_patches/ # buildkit", "2026-01-02T03:03:00+09:00"),
                 (args3 + 'for p in $(ls /tmp/build_patches_src/*.sh 2>/dev/null | sort); do bash "$p" || exit 1; done '
                  "# buildkit", "2026-01-02T03:02:00+09:00"),
                 ("COPY build_patches_src/ /tmp/build_patches_src/ # buildkit", "2026-01-02T03:01:00+09:00"),
                 ("COPY requirements.txt /tmp/requirements.txt # buildkit", "2026-01-02T03:00:00+09:00"),
                 ("ARG FX_SHUT=0", "2026-01-02T03:00:00+09:00"), ("ARG FX_OPEN=0", "2026-01-02T03:00:00+09:00"),
                 ("RUN |2 NVIDIA_X=1 SM12X_PORT=1 /bin/sh -c base # buildkit", "2025-12-01T00:00:00+09:00")]
        img_env = ["PATH=/usr/bin", "LD_LIBRARY_PATH=/usr/lib/pyfx/dist-packages/torch/lib:/usr/local/nvidia/lib"]
        dg2 = "sha256:" + "e" * 64
        ev_p = {"topology": "single", "cell": pc, "node": "main", "lockset": {}, "attestation": None,
                "build_identity": {"image_digest": dg2, "image_tag": "easy-vllm:fx-probe"}}
        calls_p: list = []
        removed_p: set = set()
        dk_p = _fake_image_docker(layers=lay_p, created="2026-01-02T03:04:05+09:00", fs=img_fs, env=img_env,
                                  calls=calls_p, removed=removed_p)
        rp_ = collect(repo, ev_p, repo / "p_probe", render_module=fake_rd, runner=dk_p)
        bp = {r["file"]: r for r in rp_["applied_set"]["patches"]}
        want = {"50-gate.sh": ("skipped", "reconstructed(docker-history+gate)"),
                "55-open.sh": ("applied", "reconstructed(fail-loud+built)"),
                "56-two.sh": ("unobserved", "none"),
                "60-loud.sh": ("applied", "reconstructed(fail-loud+built)"),
                "61-drift.sh": ("applied", "reconstructed(fail-loud+built)"),
                "62-bakedonly.sh": ("applied", "reconstructed(fail-loud+built)"),
                "10-loud.sh": ("applied", "reconstructed(fail-loud+built)"),
                "15-noe.sh": ("unobserved", "none"), "16-var.sh": ("unobserved", "none"),
                "20-cond.sh": ("unobserved", "none"),
                "30-cond.sh": ("applied", "image-probe(script-sha+marker)"),
                "40-mod.sh": ("applied", "image-probe(script-sha+marker)")}
        got = {f: (r["result"], r["result_source"]) for f, r in bp.items()}
        ck(f"F12 패치별 판정·출처(태그2 모양) {sorted((f, got.get(f)) for f in want if got.get(f) != want[f])}",
           all(got.get(f) == w for f, w in want.items()) and set(got) == set(want))
        ck("★F12 내용 조건부 + 마커 없음(20) = 이미지 탐침으로도 판정 ✗", "마커 없음" in bp["20-cond.sh"]["reason"])
        ck("★F12 set -e 없음(15) · exit $변수(16) · 일부만 열린 게이트(56) = 미관측",
           "set -e" in bp["15-noe.sh"]["reason"] and "exit $rc" in bp["16-var.sh"]["reason"]
           and "전부 열리지는" in bp["56-two.sh"]["reason"])
        ck("★F12 `sys.exit(0 if …)` · `rm … || true` 는 skip 경로가 아니다(10)", bp["10-loud.sh"]["result"] == "applied")
        ck("★F12 조건문 안 grep(트립와이어)은 적용 마커가 아니다", len(_script_shape(sp["60-loud.sh"])["markers"]) == 1
           and bp["60-loud.sh"]["evidence"]["markers"]["found"] == 1)
        ck("★F12 판정은 이미지에 구워진 바이트로(61: 작업트리엔 exit 0 · 이미지 바이트는 fail-loud) · 이미지 바이트를 싣는다",
           bp["61-drift.sh"]["script_identity"].startswith("image-probe(작업트리와 다름")
           and (repo / "p_probe/artifacts/build_patch_pre/61-drift.sh").read_bytes() == baked61
           and (repo / "p_probe/artifacts/build_patch_pre/62-bakedonly.sh").read_bytes() == baked62)
        ck("★F12 이미지에 없는 작업트리 패치(63) = not-in-image · 싣지 않음",
           "63-notbaked.sh" not in bp and any(x["file"] == "63-notbaked.sh" and x["why"].startswith("not-in-image")
                                               for x in rp_["applied_set"]["excluded"])
           and "artifacts/build_patch_pre/63-notbaked.sh" not in rp_["files"])
        prec = next(x for x in rp_["applied_set"]["probes"] if x["tier"] == "image-probe")
        ck("F12 탐침 = create(--pull never · --network none · start ✗) → cp → rm(항상)",
           prec["result"] == "done" and prec["container_removed"] is True and ("c" * 64) in removed_p
           and any(a[:1] == ["create"] and "--pull" in a and "never" in a and "--network" in a for a in calls_p)
           and not any(a[:1] in (["start"], ["exec"], ["run"]) for a in calls_p)
           and all(a[:1] != ["create"] or "/bin/true" in a for a in calls_p)
           # ★rm 은 `-v`(익명 볼륨까지 · 2026-09-22 S2 round 3 적대 리뷰) — -v 없는 rm 이 하나라도 있으면 볼륨이 샌다
           and any(a[:1] == ["rm"] for a in calls_p) and all(a[:1] != ["rm"] or a[1:2] == ["-v"] for a in calls_p))
        ck("★F12 원장 탐침(②)도 시작하지 않는 컨테이너(cp) — 원장 없음 = no-ledger(docker cp)",
           rp_["applied_set"]["probes"][1]["result"].startswith("no-ledger(이미지에")
           and rp_["applied_set"]["probes"][1].get("method", "").startswith("docker create+cp"))
        ck("F12 모듈 대상 해소 = 이미지 ENV 의 dist-packages 루트", prec.get("python_roots") == ["/usr/lib/pyfx/dist-packages"])
        ck("F12 결과 출처 어휘(공유 계약)", all(r["result_source"] in (
            "reconstructed(docker-history+gate)", "reconstructed(fail-loud+built)", "image-probe(script-sha+marker)",
            "build-ledger", "none") for r in rp_["applied_set"]["patches"]))
        ck("F12 applied_set 은 직렬화 가능(이미지 바이트는 PAYLOAD 로 새지 않는다)",
           isinstance(core.dumps(rp_["applied_set"]), str) and "port:60-fx" not in core.dumps(rp_["applied_set"]))
        sp_ = {s["path"]: s for s in rp_["slots"]["build_recipe"]["evidence"]["selected_revisions"]}
        ck("F1 COPY 대상 파일(requirements) = 이미지 안 바이트와 대조 · Dockerfile = history 전수 일치",
           sp_["output/single/requirements.txt"]["verified"] is True
           and sp_["output/single/Dockerfile.probe"]["verified"] is True)
        ck("F3 single: 무장 패치 없음 = arm_patch.sh 미실림", "artifacts/compose/arm_patch.sh" not in rp_["files"])
        bps = {s["step"]: s for s in rp_["reproduce_steps"]}["build"]["command"]
        ck("F5 single 빌드 명령(history 인자 · compose 컨텍스트)",
           "docker build -f output/single/Dockerfile.probe" in bps and "--build-arg FX_OPEN=1" in bps
           and "-t easy-vllm:fx-probe" in bps and "docker compose -f output/single/docker-compose.yaml" in bps)
        ck("F12 슬롯 증거: 전부 applied 인 슬롯만 reconstruction", rp_["slots"]["build_patch_pre"]["evidence"]["kind"] == "none"
           and "적용 미관측" in rp_["slots"]["build_patch_pre"]["evidence"]["note"])
        # ★읽기 전용 실행기(image_probe 미선언) = create/cp/rm 0 · 탐침이 가르던 30·40 은 미관측 · 61 은 작업트리 바이트로 판정
        calls_ro: list = []
        dk_ro = _fake_image_docker(layers=lay_p, created="2026-01-02T03:04:05+09:00", fs=img_fs, env=img_env, probe=False,
                                   calls=calls_ro)
        rro = collect(repo, ev_p, repo / "p_probe_ro", render_module=fake_rd, runner=dk_ro)
        bro = {r["file"]: r for r in rro["applied_set"]["patches"]}
        ck("★F12 읽기 전용 = create/cp/rm 호출 0 · 30·40 미관측 · 61 은 작업트리(exit 0) = 미관측",
           not any(a[:1] in (["create"], ["cp"], ["rm"]) for a in calls_ro)
           and bro["30-cond.sh"]["result"] == "unobserved" and bro["40-mod.sh"]["result"] == "unobserved"
           and bro["61-drift.sh"]["result"] == "unobserved"
           and bro["63-notbaked.sh"]["script_identity"].startswith("unobserved("))
        # ★(ii) 는 판정한 바이트 = 이미지 바이트일 때만(2026-09-22 S2 round 2 리뷰): 탐침이 없으면 이미지에 **없는** 작업트리 스크립트(63)
        #   도 skip 경로가 없어 applied 로 지어졌다 — 탐침 없는 applied 는 0 이고 사유가 처방(--docker-read-only 를 빼고 다시)을 가리킨다.
        #   2026-09-22 S2 round 3 통합: 옛 사유는 `--docker-probe` 를 처방으로 적었다 — 그 플래그는 이제 no-op 별칭이라 처방이 아니다(★음성).
        ck("★F12 탐침 없음 = (ii) applied 0(63-notbaked 는 이미지에 없다) · 사유가 동일성 미확인 · skip 재구성은 유지",
           not any(r["result"] == "applied" for r in bro.values())
           and bro["63-notbaked.sh"]["result"] == "unobserved" and bro["60-loud.sh"]["result"] == "unobserved"
           and "--docker-read-only" in bro["63-notbaked.sh"]["reason"] and "--docker-read-only" in bro["10-loud.sh"]["reason"]
           and bro["50-gate.sh"]["result"] == "skipped"
           and rro["slots"]["build_patch_pre"]["evidence"]["kind"] == "none")
        ck("★F12 미관측 사유에 no-op 옛 플래그(--docker-probe)를 처방으로 적지 않는다",
           not any("--docker-probe" in str(r.get("reason") or "") for r in bro.values()))
        # ── 탐침 기본값(2026-09-22 S2 round 3 · hint.py 기본 publish 와 같은 의미): runner=None = 탐침 전용 기본 실행기(탐침 ON) ·
        #   주입 실행기는 image_probe 선언이 있을 때만 · 선언 없음/False = 읽기 전용(OFF) ──
        c_def = _ctx(repo, ev_p)
        ok_def, why_def = _probe_allowed(c_def)
        ck("runner=None = 탐침 ON(기본 실행기가 image_probe 선언) · 탐침 기록 문구 = 기본 실행기",
           ok_def and c_def.default_runner and getattr(c_def.runner, PROBE_ATTR, False) is True and "기본 실행기" in why_def)

        def undeclared(argv, timeout=DOCKER_TIMEOUT_S):
            return subprocess.CompletedProcess(argv, 1, "", "")

        def declared_false(argv, timeout=DOCKER_TIMEOUT_S):
            return subprocess.CompletedProcess(argv, 1, "", "")
        declared_false.image_probe = False        # type: ignore[attr-defined]
        ck("★선언 없는 주입 실행기 · image_probe=False = 탐침 OFF(읽기 전용으로 본다 · 추측해서 켜지 않는다)",
           not _probe_allowed(_ctx(repo, ev_p, runner=undeclared))[0]
           and not _probe_allowed(_ctx(repo, ev_p, runner=declared_false))[0]
           and _probe_allowed(_ctx(repo, ev_p, runner=dk_p))[1] == "주입 실행기(image_probe 선언)")
        seen_d: list = []

        def fake_sub(argv, **kw):
            seen_d.append(list(argv))
            if list(argv[1:2]) == ["create"]:
                return subprocess.CompletedProcess(argv, 0, "f" * 64 + "\n", "")
            return subprocess.CompletedProcess(argv, 0, "", "")
        dr = _probe_only_runner(run=fake_sub)
        refused = {v: dr(["docker", v, "x"]).returncode for v in ("run", "start", "exec", "build", "pull", "compose", "kill")}
        ck("★기본 실행기: run·start·exec·build·pull·compose·kill = rc 125 · 실물 호출 0",
           set(refused.values()) == {125} and not seen_d)
        ck("★기본 실행기: `--pull never --network none` 없는 create = 거부(실물 호출 0)",
           dr(["docker", "create", "img"]).returncode == 125
           and dr(["docker", "create", "--pull", "never", "img"]).returncode == 125
           and dr(["docker", "create", "--pull", "always", "--network", "none", "img"]).returncode == 125 and not seen_d)
        ck("★기본 실행기: 남이 만든 컨테이너 cp·rm = 거부(실물 호출 0)",
           dr(["docker", "cp", "-L", "a" * 64 + ":/x", "y"]).returncode == 125
           and dr(["docker", "rm", "a" * 64]).returncode == 125 and not seen_d)
        r_cr = dr(["docker", "create", "--pull", "never", "--network", "none", "--entrypoint", "/bin/true", "img"])
        ck("기본 실행기: create(--pull never --network none) → 자기 컨테이너 cp → rm · ★지운 뒤 rm 재시도 = 거부 · inspect·history 통과",
           r_cr.returncode == 0 and dr(["docker", "cp", "-L", "f" * 64 + ":/x", "y"]).returncode == 0
           and dr(["docker", "rm", "f" * 64]).returncode == 0 and dr(["docker", "rm", "f" * 64]).returncode == 125
           and dr(["docker", "image", "inspect", "img"]).returncode == 0 and dr(["docker", "history", "img"]).returncode == 0
           and [a[1] for a in seen_d] == ["create", "cp", "rm", "image", "history"]
           and getattr(dr, PROBE_ATTR, False) is True)
        # ★(ii) 이고 동일성도 확인됐지만 탐침이 선언 마커 부재를 보면(반대 증거) applied 로 적지 않는다
        fs_nomark60 = dict(img_fs, **{"/workspace/vllm-src/vllm/a.py": b"anchor\n"})
        rn60 = collect(repo, ev_p, repo / "p_probe_nm60", render_module=fake_rd, runner=_fake_image_docker(
            layers=lay_p, created="2026-01-02T03:04:05+09:00", fs=fs_nomark60, env=img_env))
        b60 = {r["file"]: r for r in rn60["applied_set"]["patches"]}
        ck("★F12 (ii) + 선언 마커 부재 = 미관측(모순 기재) · 마커를 선언하지 않은 (ii)(10)는 applied 유지",
           b60["60-loud.sh"]["result"] == "unobserved" and "선언 마커 부재" in b60["60-loud.sh"]["reason"]
           and b60["10-loud.sh"]["result"] == "applied")
        # ★실패 삼킴(패치 단계 `|| true`)·trap 은 set +e 와 같다 — (ii) 불성립. 정리 명령(rm·find -delete)과 치환 안 값 탐침
        #   (`V="$(grep … || true)"` · 50 꼴)의 `|| true` 는 skip 경로가 아니다(2026-09-22 S2 round 2 리뷰).
        env_ok = {"loop": (True, "fx 루프"), "image_exists": True, "identity_ok": True,
                  "probe": {"ran": True, "files": {}, "modules": {}, "scripts": {}}}
        r_sw = _reconstruct("70-sw.sh", "#!/bin/bash\nset -e\npython3 - \"$F\" <<'PY' || true\nopen('/x', 'w').write('y')\nPY\n"
                                        "echo done\n", "pre", {}, env_ok)
        r_tr = _reconstruct("71-tr.sh", "#!/bin/bash\nset -e\ntrap 'exit 0' ERR\npython3 /x.py\n", "pre", {}, env_ok)
        r_ok = _reconstruct("72-ok.sh", "#!/bin/bash\nset -e\nrm -rf -- /tmp/b 2>/dev/null || true\n"
                                        "V=\"$(grep -E x /f || true)\"\nfind /w -name '*.pyc' -delete 2>/dev/null || true\n"
                                        "python3 /x.py\n", "pre", {}, env_ok)
        ck("★(ii) 패치 단계 `|| true`·trap = applied ✗ · 정리 명령·치환 안 `|| true` 는 skip 경로가 아니다(applied)",
           r_sw["result"] == "unobserved" and "실패 삼킴" in r_sw["reason"] and r_tr["result"] == "unobserved"
           and "trap" in r_tr["reason"] and r_ok["result"] == "applied"
           and r_ok["result_source"] == "reconstructed(fail-loud+built)")
        ck("★따옴표 안 성공 종료(`eval 'exit 0'`)도 성공 종료 가능 exit 로 센다 · `exit 1` 은 아니다",
           [e["code"] for e in _script_shape("#!/bin/bash\nset -e\neval 'exit 0'\n[ -f /x ] || exit 1\n")["exits"]] == ["0"])
        # ★선언 마커가 이미지에 없으면(모순) 탐침 판정 ✗
        fs_nomark = dict(img_fs, **{"/workspace/vllm-src/gate.py": b"if (9, 0) <= cap < (11, 0):\n"})
        rnm = collect(repo, ev_p, repo / "p_probe_nm", render_module=fake_rd, runner=_fake_image_docker(
            layers=lay_p, created="2026-01-02T03:04:05+09:00", fs=fs_nomark, env=img_env))
        bnm = {r["file"]: r for r in rnm["applied_set"]["patches"]}
        ck("★F12 선언 마커 부재 = 미관측(모순 기재)", bnm["30-cond.sh"]["result"] == "unobserved"
           and "선언 마커 부재" in bnm["30-cond.sh"]["reason"])
        # 이미지에 원장이 있으면 cp 로 읽은 원장이 권위다(재구성 ✗ · `docker run` 0)
        led_p = {"schema_version": 1, "dockerfile": "Dockerfile.probe", "track": "source-build",
                 "build_args": {"SM12X_PORT": "0", "FX_OPEN": "1", "FX_SHUT": "0"},
                 "patches": [{"phase": "pre", "file": "60-loud.sh", "result": "applied", "result_source": "exit-code"}]}
        calls_l: list = []
        removed_l: set = set()
        rl = collect(repo, ev_p, repo / "p_probe_led", render_module=fake_rd, runner=_fake_image_docker(
            layers=lay_p, created="2026-01-02T03:04:05+09:00", env=img_env, calls=calls_l, removed=removed_l,
            fs=dict(img_fs, **{LEDGER_IN_IMAGE: json.dumps(led_p).encode("utf-8")})))
        ck("F12 이미지 원장 = docker create+cp 로 관측(run 0 · rm 됨) · 원장 행 출처 build-ledger",
           rl["applied_set"]["status"] == "observed" and not any(a[:1] == ["run"] for a in calls_l)
           and ("c" * 64) in removed_l and rl["applied_set"]["patches"][0]["result_source"] == "build-ledger"
           and rl["applied_set"]["patches"][0]["ledger_result_source"] == "exit-code"
           and "--build-arg FX_OPEN=1" in {s["step"]: s for s in rl["reproduce_steps"]}["build"]["command"]
           and {s["step"]: s for s in rl["reproduce_steps"]}["build"]["command_source"]
           == "reconstructed(build-ledger build_args + compose build)")
        # ★원장 부재 + 컨테이너 제거 실패 = "원장 없음" 줄에 rm 실패가 남는다(부수효과 사실을 접어 버리지 않는다)
        base_rm = _fake_image_docker(layers=lay_p, created="2026-01-02T03:04:05+09:00", fs=img_fs, env=img_env)

        def rm_fails(argv, timeout=DOCKER_TIMEOUT_S):
            if list(argv[1:2]) == ["rm"]:
                return subprocess.CompletedProcess(argv, 1, "", "Error: cannot remove")
            return base_rm(argv, timeout=timeout)
        rm_fails.image_probe = True        # type: ignore[attr-defined]
        arm = applied_set(repo, ev_p, runner=rm_fails)
        ck("★F12 원장 cp 뒤 docker rm 실패 = no-ledger 줄에 rm-failed 기재 · 이미지 탐침 기록도 제거 실패",
           arm["probes"][1]["result"].startswith("no-ledger(") and "rm-failed(" in arm["probes"][1]["result"]
           and next(x for x in arm["probes"] if x["tier"] == "image-probe")["container_removed"] is False)
        # ★cp 가 예외로 죽어도 컨테이너는 지운다(finally) · 실패한 수집은 artifacts/ 를 남기지 않는다
        removed_c: set = set()
        try:
            collect(repo, ev_p, repo / "p_probe_crash", render_module=fake_rd, runner=_fake_image_docker(
                layers=lay_p, created="2026-01-02T03:04:05+09:00", fs=img_fs, env=img_env, removed=removed_c,
                crash_on_cp=True))
            ck("★F12 cp 예외(전파돼야 한다)", False)
        except RuntimeError:
            ck("★F12 cp 예외에도 docker rm(finally) · artifacts/ 잔재 없음", ("c" * 64) in removed_c
               and not (repo / "p_probe_crash" / "artifacts").exists())
        # ★이미지 안 COPY 대상 바이트가 다르면(일치 개정 없음) 경고
        rrq = collect(repo, ev_p, repo / "p_probe_req", render_module=fake_rd, runner=_fake_image_docker(
            layers=lay_p, created="2026-01-02T03:04:05+09:00", fs=dict(img_fs, **{"/tmp/requirements.txt": b"other\n"}),
            env=img_env))
        rvq = {r["path"]: r for r in rrq["slots"]["build_recipe"]["evidence"]["recipe_vs_image"]}
        sq = {s["path"]: s for s in rrq["slots"]["build_recipe"]["evidence"]["selected_revisions"]}
        ck("★F1 COPY 대상 바이트 불일치 = verified ✗ (추적 파일이면 경고 행)", sq["output/single/requirements.txt"]["verified"]
           is False and "이미지 안 바이트와 다르고" in sq["output/single/requirements.txt"]["method"]
           and ("output/single/requirements.txt" not in rvq or "warning" in rvq["output/single/requirements.txt"]))
        # ★slave_forward 거부(SystemExit)는 HintError 로 번역된다
        (out / "envs" / f".env.{cell}").write_text(
            (out / "envs" / f".env.{cell}").read_text(encoding="utf-8") + "VLLM_REPO=bad value\n", encoding="utf-8")
        import contextlib
        import io
        with contextlib.redirect_stderr(io.StringIO()):      # slave_forward.die() 의 stderr 한 줄은 삼키되 판정은 code 로
            raises("★slave_forward 거부 번역", "HINT_SUB_RECIPE_DERIVE_FAILED",
                   lambda: sub_recipe(repo, ev, render_module=fake_rd))
        # lockset 출처 키 ↔ 소유자(campaign_template_validator.LOCKSET_KNOB_SOURCES) 교차대조(실물 모듈)
        # 소유 모듈을 import 하지 않고 **상수 리터럴만** AST 로 읽는다(import 부수효과·동시 편집 중 모듈의 다른 결함과 분리).
        import ast
        owner = None
        try:
            tree = ast.parse((own / core.REL_CAMPAIGN_VALIDATOR).read_text(encoding="utf-8"))
            for node in tree.body:
                if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == "LOCKSET_KNOB_SOURCES"
                                                        for t in node.targets):
                    owner = ast.literal_eval(node.value)
        except (OSError, SyntaxError, ValueError) as e:
            bad.append(f"artifacts: lockset 소유자 교차대조 불가 — {type(e).__name__}")
        if owner is not None:
            ck("LOCKSET_SOURCE_KNOB 키 = 소유자 키(campaign_template_validator.LOCKSET_KNOB_SOURCES)",
               set(owner) == set(LOCKSET_SOURCE_KNOB))
            ck("출처 어휘 전부에 지위 후보", all(v in _SOURCE_STATUS for vals in owner.values() for v in vals))
        else:
            bad.append("artifacts: LOCKSET_KNOB_SOURCES 리터럴을 찾지 못했다(소유자 이동 — 교차대조 갱신)")
    return bad
