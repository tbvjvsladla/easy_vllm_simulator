#!/usr/bin/env python3
"""hint.py — hint-publisher 단일 CLI (plan_26092119 §4.8·§4.9 · SPEC §4 · 2026-09-22 wave 2 통합).

    hint.py [--repo R] publish  (--campaign ID --cell CELL | --publication TOPIC --replay) [--node main|sub|cluster]
                                --generated-utc U [--lineage-add PATH=REASON]... [--out DIR]
                                [--remote NAME] [--no-remote-check] [--docker-read-only]   # 이미지 탐침 = 기본(--docker-probe = 옛 이름)
    hint.py [--repo R] continue (--campaign ID --cell CELL | --draft DIR) [--node N] --generated-utc U
                                [--approved-by TEXT --approved-utc U] [--factcheck-waiver TEXT] [--remote NAME] [--no-push]
    hint.py [--repo R] lint     --draft DIR [--json]                  # 부수효과 0 · 저작 중 반복 확인
    hint.py [--repo R] refresh  --draft DIR                           # draft 안 파생 블록(00 §0.3 · §0.5)만 다시 쓴다(미리 보기)
    hint.py [--repo R] excerpt  --draft DIR --source <stem|경로|artifacts 파일명|<도구>@<rev12>> [--lines a-b] [--numbered]
    hint.py [--repo R] name     (--campaign ID --cell CELL | --publication TOPIC) [--node N] [--json]   # 읽기 전용
    hint.py [--repo R] verify   --tag TAG [--draft DIR]               # 이 태그 1개 · push 전 로컬 봉인 검증(D10)
    hint.py [--repo R] push     --tag TAG [--remote NAME] [--apply] [--draft DIR]   # 정확한 태그 1개(glob ✗)
    hint.py [--repo R] catalog derive --remote NAME --generated-kst K [--dry-run] [--record-missing] [--allow-empty]
    hint.py [--repo R] match    --vllm V --model M [--arch A] [--include-other] [--json]   # 읽기 전용 · gitless
    hint.py [--repo R] branch-transition --remote R --generated-utc U  # maintenance-only README 전환 · tag/catalog/code worktree ✗
    hint.py --self-test

한 셀 = 한 태그(D9). `publish` 는 증거를 모아 이름·계보·산출물·사실 블록을 채운 **스캐폴드를 만들고 정지**한다(태그·브랜치
부수효과 0). Agent 가 PROMPT 절을 산문·hint-event·원문 발췌로 저작하고(lint 0) **저작자가 아닌 Agent** 가 사실 검증 보고
(`inputs/factcheck.json`)를 남기면 `continue` 가 승인 확인 → 린트 → 사실 검증 게이트 → hint 브랜치 배관 커밋 → 봉인 → 이 태그 1개
로컬 검증 → push(정확한 refspec 1개) → 원격 SHA 대조 → 카탈로그 파생 → 캠페인 publish 위상을 잇는다.

불변식 (날짜 = 사고·결정 · 옛 hint_tag.py·hint_branch.py 에서 옮김 · 삭제 ✗)
    - **부수효과 전 게이트**(2026-07 Phase 3 · plan_26072506 2A): 게이트가 걸린 명령(`HINT_ACTION_FOR_CMD`)은 첫 부수효과 전에
      `completion_gate.py authorize --mode promotion` 을 묻는다. 거부 = 부수효과 0 · 게이트 JSON 을 stdout 에 그대로 + 그 종료코드.
      `publish`·`lint`·`excerpt`·`name`·`match`·`catalog` 은 게이트가 없다(태그·브랜치를 만들지 않는다 · match 는 수신자 평면 · C6).
    - **승인이 먼저**(O6 · X3 · 2026-09-21): `continue` 의 첫 판정은 사람 승인이다. 캠페인 셀 = campaign.yaml `hint_targets[]
      .approval`(단일 원천 · 사전승인) · 캠페인 밖(재생) = `--approved-by "<사람 발화 전사>" --approved-utc`. 무인 자동 태깅 ✗ —
      승인이 없으면 어느 ref 도 움직이지 않는다(제안 전용 · 사람 Y/N 을 받아 전사한다).
    - **ACTIVE 를 읽지 않는다**(2026-09-07 Q6) — 캠페인은 명시 id 만. 캠페인 상태 쓰기는 `evidence.phase_set_publish` 하나다.
    - **게이트 범위 = 이번 발행분**(D10 · F12 · 2026-08-20 "drift 16건이 hy3 7건을 막았다"): verify·push 는 이 태그 1개만 본다.
      과거·타 PC 태그(옛 5세그먼트 · 로컬 오브젝트 없는 원격 태그 · lightweight)는 신규 발행을 막지 않는다(AC6). 카탈로그 파생은
      부재 태그를 행으로 정직하게 싣는다(`record_missing=True` · X15 · 통합 결정 D-i).
    - **push = 태그 1개**(O2 · 2026-08-20 · 2026-09-04 C-3): `refs/tags/<그 태그>:refs/tags/<그 태그>` 정확히 1개 · 브랜치는 발행이
      밀지 않는다(hint 브랜치 push 는 S7 1회 예외 · 사람 소관). push 뒤 원격 태그 오브젝트 SHA == 로컬(불일치 = 차단).
    - **이름은 도구가 전량 파생**(D8 · 2026-08-20 R1): 발행자는 이름을 입력하지 않는다. 충돌 = 차단(X13 · 2026-09-04 사용자 결정
      "충돌은 축이 모자란 것") — 개정판은 새 이름으로만(P1 리콜 금지). 로컬 태그가 **이 draft 가 봉인한 것**이면 재개다(tag.seal).
    - **손 JSON 0**(AC5 · F11): identity·runtime·pii 입력과 promotion_target 은 도구가 쓴다(evidence · evidence_publisher). hint.py 는
      work-manifest 를 직접 고치지 않는다(X6 — writer 1).
    - **시각은 주입만**(docs.md `--now` 선례): `--generated-utc`. 커밋·봉인 시각은 첫 커밋 때 state.json 에 적은 값을 재실행에서도
      그대로 쓴다 — `branch.commit_payload` 의 재개는 메시지·시각이 같을 때만 성립한다(2026-09-22 리뷰: 새 시각 재실행 = 같은 트리의
      중복 커밋).
    - **판정은 code 로만**(감사 ①-② 2026-09-01): 봉인·push 거부는 `tag.problem_codes(e.problems)` 로 분류한다(메시지 substring ✗).
    - **재생은 읽기만**(SPEC §4.1): `--publication --replay` 는 draft 밖에 한 바이트도 쓰지 않는다(발행기 구동 ✗ · 게이트 사전 확인은
      읽기 전용이라 묻는다). 원격 조회 실패는 재생·`--no-remote-check` 에서만 견딘다(그 밖에서는 조회 불가 = 차단).
    - **match 는 gitless**(verify_distribution `gitless_hint_match` · 2026-07-31): git·YAML·게이트 없이 `hints/index.json` 만으로 돈다.
    - **발행 전 독립 사실 검증**(2026-09-22 · plan_26092119 S2 round 2 F11): S2 1차 재생에서 린트 0 · 발췌 무결성 통과인 페이로드가
      독립 검증에서 사실 오류 25건(오답 5 · 오도 16 · 근거 없음 4)을 냈다 — 발췌 무결성은 "인용이 원문과 같은가" 만 보고 "주장이 원문과
      맞는가" 는 보지 못한다. 그래서 continue 는 린트 0 뒤에 `inputs/factcheck.json`(저작자 ≠ 검증자 · 7부류 전부 점검 ·
      열린 오답/오도/근거없음 0)을 요구한다(`HINT_FACTCHECK_ABSENT` · `_INVALID` · `_OPEN`). 탈출구는 사람 결정 하나 —
      `--factcheck-waiver "<사람 발화 전사>"` 는 state.json 과 00 메타 · PAYLOAD.factcheck 에 **면제** 로 남는다(수신자가 안다).
      면제 전사의 모양 결함(빈칸 · 자리표시 · 여러 줄)은 부수효과 전에 `HINT_FACTCHECK_WAIVER_INVALID` 로 거부한다(계약 §6.7).

통합 결정 (2026-09-22 · wave 2 · 리드 검토)
    D-a requirements.txt 는 source-build 트랙에서도 싣는다(Dockerfile.source-build 가 `/etc/pip/constraint.txt` 로 COPY) — 규칙은
        "쓰인 것만" 이다(artifacts 가 쓰인 Dockerfile 의 COPY 줄에서 파생).
    D-b value-status 커버리지는 전송·신원 노브 `model`·`host`·`port`·`served-model-name` 을 뺀다(artifacts `VALUE_STATUS_EXCLUDED_KNOBS`).
    D-c 서명 조각 ≥ 12자(공백 제외) · 마크다운 출처 발췌의 `§<절>` = 그 문서 제목 줄의 정규화 접두(template lint).
    D-d native 평면 신호의 producer 가 아직 없다 → `HINT_PLANE_UNDERIVABLE` fail-closed 유지(알려진 한계 · S1–S5 범위 밖).
    D-e 서브에서만 잰 셀은 메인이 관측 원시를 볼 수 없어 `HINT_QUALIFICATION_UNOBSERVED` 로 막힌다 — 열린 설계 항목(문서기반 회수로
        서브 serve_proof 를 받는 설계가 필요하다). E2E 는 cluster 셀을 쓴다.
    D-f `PAYLOAD.measurement` = 측정 수치(인증서 성능 칸 + 스윕 레벨) · `PAYLOAD.measurement_config` = DISTRIBUTED_MEASUREMENT_KEYS +
        source(evidence 의 분리 그대로 · SPEC §2.1 이 measurement 에 두었던 DISTRIBUTED 키는 measurement_config 로만 간다).
    D-g X18 릴리스 문법에 4마디(`vM.m.p.q`)·`.postN` 포함(naming).
    D-h 비-업스트림 VLLM_REPO(포크)의 릴리스 모양 VLLM_REF 는 SHA 경로(`<직전 릴리스>-g<sha12>`)만 · 없으면 `HINT_AXIS_UNDERIVABLE` ·
        naming facts 가 vllm_repo 를 싣는다.
    D-i 카탈로그는 continue(push 뒤)와 S5 재생성에서 `record_missing=True`.

알려진 한계 (정직 기재)
    - 단독 `verify`·`push` 의 서사 린트는 그 태그를 봉인한 draft(inputs/template_facts.json)가 있어야 돈다 — draft 가 없으면
      `HINT_LINT_DRAFT_ABSENT` 로 검증이 실패한다(검증 범위 = 전송 범위 · 린트를 조용히 건너뛰지 않는다). `--draft` 로 지정한다.
    - 재생 드래프트의 continue 는 옛 발행 기록의 promotion_target 을 새 태그로 다시 적는다(evidence_publisher set-promotion-target ·
      같은 기록의 새 판본). 옛 태그는 판정하지 않는다(P1).
    - `--docker-read-only` 는 이미지 탐침(create · cp · rm)과 이미지 안 원장 탐침을 끈다 — 탐침 기록에 관측 실패 · skipped 로 남긴다
      (관측 실패 ≠ 원장 없음) — 적용 집합은 attestation 원장 또는 라벨된 재구성으로만 말한다(태그2 셀 실측: 9개 중 7개가 `unobserved`).
    - **이미지 탐침은 기본이다**(2026-09-22 · plan_26092119 S2 round 3 · 결정 기록): 플래그 없는 publish 가 **시작하지 않는** 컨테이너의
      create(`--pull never --network none` — 규칙의 단일 소유자 = artifacts `_probe_only_runner`) · cp · rm 으로 측정 이미지 안 패치
      사본 · 적용 표지 · 원장을 본다. 2차 기계 채점: 옵트인 탐침을 잊은 publish 가
      9개 중 7개를 다시 `unobserved` 로 실었다(측정 이미지가 이 호스트에 있었다). `--docker-probe` 는 옛 이름(no-op 별칭)으로 받는다.
    - **측정 도구 스냅샷은 origin 과 바이트 동일할 때만 발췌 출처다**(2026-09-22 round 3 적대 리뷰): draft `inputs/sources/<도구>@<rev12>`
      가 LINEAGE origin(`git show <rev>:<경로>`)과 다르면 `HINT_EXCERPT_SNAPSHOT_DRIFT` · 확인 불가면 `_UNVERIFIABLE`(lint · excerpt 공통).

import 부수효과 0 — 파서·함수 정의와 hintlib(역시 부수효과 0) 적재뿐이다. 바이트코드를 쓰지 않는다(저장소 트리에 캐시를 남기지 않는다).
"""
from __future__ import annotations

import argparse
import contextlib
import dataclasses
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

sys.dont_write_bytecode = True
_SCRIPTS_DIR = Path(__file__).resolve().parent
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

from hintlib import (artifacts, branch, catalog, core, evidence, lineage, naming, pii,  # noqa: E402
                     tag, template)

# ── 계약 상수 ────────────────────────────────────────────────────────────────────────────────
# CLI → 승격 게이트 액션(completion_gate ALLOWED_ACTIONS · 스키마 2종 · 정책 C6 와 짝). publish·lint·excerpt·name·match·catalog 은
# 게이트 없음(태그·브랜치 부수효과 0 · match 는 수신자 평면 — 게이트를 걸면 gitless 배포본에서 죽는다).
HINT_ACTION_FOR_CMD = {"continue": "hint_finalize", "verify": "hint_verify", "push": "hint_push"}
STATE_NAME = "state.json"
STATE_SCHEMA = 1
PAYLOAD_SCHEMA_VERSION = 2
PROVENANCE_SCHEMA_VERSION = 2
APPROVAL_KEYS = ("approved_by", "approved_utc", "source")
# `driver` = 인증서·스윕 meta 의 driver_version(evidence 가 출처와 함께 싣는다 · 2026-09-22 S2 round 2 — 1차 FACT:resolved 는 인증서에
#   580.173.02 가 있는데도 드라이버를 미관측으로 적었다).
_BUILD_KEYS = ("track", "dockerfile", "image_tag", "image_digest", "vllm_repo", "vllm_ref", "vllm_sha", "torch", "cuda",
               "ngc", "cpu_arch", "driver")
_RATIONALE_MAX = 400
# ── 발행 전 독립 사실 검증 보고(`<draft>/inputs/factcheck.json` · 2026-09-22 S2 round 2 F11) ──
#   형식 = {schema_version: 1, tag, author, checker, checked_utc, claims_checked, classes_checked: [1..7],
#           items: [{id, where, claim, verdict, class, truth, source, status, resolution}]}
#   판정: 검증자 ≠ 저작자 · 7부류(template.FACTCHECK_CLASSES) 전부 점검 · 판정이 FACTCHECK_VERDICTS 인 항목은 status=fixed(+resolution)
#   이어야 닫힌다. `unsupported`(근거 없음)도 막는다 — 1차 검증에서 출처 없는 인과(부류 5)가 이 판정으로 나왔고, 수신자에게는 근거 없는
#   단정이 오도와 같은 피해를 낸다(의도적으로 wrong/misleading 보다 넓게 · 결정 기록).
FACTCHECK_NAME = "factcheck.json"
FACTCHECK_SCHEMA = 1
FACTCHECK_VERDICTS = ("wrong", "misleading", "unsupported")
FACTCHECK_STATUSES = ("open", "fixed", "disputed")
_OUT = "[hint]"
_UTC_EXAMPLE = "<UTC YYYY-MM-DDTHH:MM:SSZ>"


def _log(msg: str) -> None:
    print(f"{_OUT} {msg}", file=sys.stderr)


# 게이트를 **실행조차 못 한** 두 경우(옛 hint_tag `_gate_bare_result` 계약 · 2026-07 Phase 3): 게이트 JSON 이 없으므로 같은 모양의
# 결정론 포장을 stdout 에 낸다 — 소비자는 거부든 실행 불가든 stdout 을 json.loads 한다(부수효과 0 · allowed:false).
_GATE_BARE_CODES = ("HINT_GATE_SUBPROCESS_UNAVAILABLE", "HINT_GATE_OUTPUT_UNREADABLE")


def _gate(repo: Path, manifest, cmd: str) -> dict:
    """승격 게이트 1회 — 액션은 `HINT_ACTION_FOR_CMD[cmd]` 에서만 읽는다(액션 이름을 호출부마다 손으로 다시 적으면 매핑과
    실제 호출이 갈라진다 · 2026-09-22 리뷰 · 매직넘버 판정표 "같은 개념이 두 곳 이상에 손으로 적힌 값").
    publish 의 사전 확인은 continue 가 봉인 전에 물을 그 액션(`continue` → hint_finalize)을 미리 묻는다."""
    action = HINT_ACTION_FOR_CMD[cmd]
    try:
        return evidence.authorize(repo, manifest, action)
    except core.HintError as e:
        if e.code in _GATE_BARE_CODES:
            e.gate_json = core.dumps({"schema_version": 1, "mode": "promotion", "action": action, "task_class": None,
                                      "authorization_state": None, "allowed": False, "reason_codes": [e.code],
                                      "messages": {e.code: e.message}, "identity": None, "exit_code": e.exit_code or 2})
        raise


# ── D8 권한 비대칭 (plan_26082009 §4 · 옛 hint_tag `_require_central` 이관 · 2026-09-22 wave 2 통합) ─────────────────────────
# 사용자 제약: "HINTS.md 의 관리 주체는 단 1종으로 한정". 봉인(seal)은 분산, 색인·배포(push)는 중앙집중이다. 마커는 **gitignored**
# 라 배포본에 실리지 않는다 — 배포받은 Contributor 환경에서는 존재할 수 없고, 게이트가 fail-closed 로 닫힌다. 신원 체계 없이
# 결정론으로 집행하는 가장 단순한 수단이다. 옛 CLI 는 push 에 이 검사를 걸었는데 재구성 1차에서 카탈로그(catalog.derive)에만
# 남고 push 에서 빠졌다(2026-09-22 문서 담당 발견 — `--no-push` 를 잊으면 Contributor 체크아웃이 태그를 원격에 쓴다). 되돌린다.
# 안내가 막다른 길로 읽히면 사람도 에이전트도 우회를 택한다(workflow.md D5 · plan_26082017 W7 · 2026-08-20 실증: 맨 `git tag -a` +
# `git push` 로 돌아갔다) — 처방(권위 선언 또는 --no-push 로 봉인까지 → 상류 전달)을 함께 적는다.
def _require_central(repo: Path, action: str) -> None:
    if (repo / core.REL_CENTRAL_FLAG).is_file():
        return
    core.fail("HINT_PUSH_CENTRAL_AUTHORITY_ABSENT",
              f"'{action}' 는 **자기 원격의 색인·배포 권위**를 가진 체크아웃에서만 실행된다 — `{core.REL_CENTRAL_FLAG}` 부재"
              "(D8 권한 비대칭 · plan_26082009 · 2026-08-20).",
              "▸ 이 체크아웃이 자기 원격(origin)의 hint 카탈로그를 소유한다면 권위를 선언하라: "
              f"printf '%s\\n' '이 체크아웃이 자기 원격의 hint 색인·배포 권위다.' > {core.REL_CENTRAL_FLAG} "
              "(비추적 — 각 저장소가 스스로 선언한다 · 선언은 그 원격의 index.json·HINTS.md 정합을 떠안는 책임이다). "
              "▸ 상류에 기여하는 입장이면 `hint.py continue … --no-push` 로 봉인·로컬 검증까지만 하고 그 태그를 상류에 전달하라"
              "(색인·배포는 상류가 한다). ▸ 어느 쪽도 아니면 **우회하지 말고** 어느 쪽인지부터 정하라 — 맨 `git tag -a` + "
              "`git push` 로 만든 태그는 증거 바인딩이 없다.")


@dataclass
class Runtime:
    """부작용 주입구(자체검사 전용 · CLI 기본값 = 실물). docker = evidence·artifacts 의 docker 실행기.
    None 의 뜻은 모듈마다 다르다(2026-09-22 · plan_26092119 S2 round 3 통합 정정 — 옛 문구 "None = 실물 subprocess" 는 artifacts 에서
    틀렸다): evidence 는 읽기 전용 가드(image inspect · history) 뒤의 실물 subprocess · artifacts 는 자기 **탐침 전용 기본 실행기**
    (탐침 ON · run/start ✗). 주입 실행기는 artifacts `PROBE_ATTR`(image_probe) 를 **선언해야만** 탐침된다 — 선언 없음 = 읽기 전용."""
    docker: object = None


def _docker_read_only():
    """`--docker-read-only` 실행기: `image inspect`·`history` 만 실물로 부르고 그 밖(원장 cat 의 `docker run`)은 **docker 자신의 실패
    (rc 125)** 로 돌려준다 — artifacts 가 탐침 기록에 `docker-run-failed` 로 남긴다(관측 실패를 '원장 없음' 으로 둔갑시키지 않는다)."""
    def run(argv, **kw):
        a = list(argv[1:])
        if a[:2] == ["image", "inspect"] or a[:1] == ["history"]:
            kw.setdefault("capture_output", True)
            kw.setdefault("text", True)
            return subprocess.run(argv, **kw)
        return subprocess.CompletedProcess(argv, 125, "", "hint.py --docker-read-only: 이미지 실행(원장 탐침) 거부")
    return run


def _docker_probe_only():
    """publish 의 **기본** 실행기(2026-09-22 S2 round 3 — round 2 에서는 `--docker-probe` 옵트인이었다 · 그 플래그는 이제 no-op 별칭):
    `image inspect`·`history` + **시작하지 않는** 컨테이너의 `create`(`--pull never`
    필수)·`cp`(이 실행기가 만든 컨테이너에서만)·`rm`(이 실행기가 만든 컨테이너만)을 실물로 부르고 `image_probe` 능력을 선언한다
    (artifacts `PROBE_ATTR` — 이미지 안 패치 사본 · 적용 표지를 꺼내 `image-probe(script-sha+marker)` 판정을 낸다). 그 밖(`run` ·
    `start` · `exec` · `build` · `compose` · `pull` …)은 docker 자신의 실패(rc 125)로 돌려준다. 이미지 원장(`/opt/easy-vllm/build_ledger.json`)
    도 artifacts 가 이 능력을 보고 **같은 create+cp** 로 읽으므로(`_observe_ledger`) 이 실행기에서는 원장 관측 ② 가 성립한다 — `docker
    run … cat` 은 부르지 않는다(누가 부르면 거부된다). 2026-09-22 · plan_26092119 S2 round 2 통합 정정: 옛 문구는 "원장 cat 이 거부되어
    관측 실패로 남는다" 였다 — artifacts 가 원장 읽기를 create+cp 로 옮긴 뒤로는 사실이 아니었다(도움말도 같이 고쳤다).
    왜: S2 1차 재생은 9개 패치 중 7개를 '적용 미관측' 으로 실었는데 기계 채점기는 이미지 파일로 6개를
    판정했다(관측할 수 있었던 것) — 컨테이너를 **띄우지 않고** 그 관측을 얻는 경로가 필요했다. 2차 재생은 옵트인 플래그를 쓰지 않아
    같은 7개를 다시 `unobserved` 로 실었다 → 기본값으로 올렸다(끄는 것은 `--docker-read-only` · 정책 자체검사의 docker 호출 계약도
    "시작하지 않는 create · 만든 컨테이너의 cp · rm" 으로 함께 넓혔다 — runtime_selftest `_hint_docker_calls_non_starting`).
    2026-09-22 round 3 적대 리뷰: 이 함수는 artifacts `_probe_only_runner`(라이브러리 기본 실행기)와 **같은 규칙을 손으로 한 벌 더**
    적고 있었고 이미 갈라져 있었다(여기는 create 에 `--pull never` 만 · 저쪽은 `--network none` 도 요구). 규칙의 소유자는 artifacts
    하나다 — 여기서는 그 실행기를 감싸 evidence 의 호출 꼴(`capture_output` · `text` · `timeout` 키워드)만 맞춘다."""
    factory = getattr(artifacts, "_probe_only_runner", None)
    if not callable(factory):
        core.fail("HINT_OWNER_MODULE_MISSING", "hintlib.artifacts._probe_only_runner(탐침 전용 docker 실행기 — 규칙의 단일 소유자)가 없다.",
                  "artifacts 모듈을 복구한다(publish 는 제한 없는 docker 를 부르지 않는다).")
    inner = factory()

    def run(argv, **kw):
        # evidence._docker 는 subprocess.run 꼴(capture_output · text · timeout)로 부른다 — 탐침 실행기는 늘 텍스트로 잡는다.
        return inner(list(argv), timeout=kw.get("timeout", artifacts.DOCKER_TIMEOUT_S))

    setattr(run, artifacts.PROBE_ATTR, getattr(inner, artifacts.PROBE_ATTR, False) is True)   # 선언해야만 탐침한다
    return run


# ── 경로 · 상태 ───────────────────────────────────────────────────────────────────────────────
def _show(repo: Path, p: Path) -> str:
    try:
        return Path(p).resolve().relative_to(repo.resolve()).as_posix()
    except ValueError:
        return str(Path(p).resolve())


def _cli(repo: Path) -> str:
    return f"python3 {core.REL_HINT_CLI}" if (repo / core.REL_HINT_CLI).is_file() else f"python3 {Path(__file__).resolve()}"


def _require_fresh_draft(draft: Path) -> None:
    """draft 자리는 비어 있어야 한다(기존 저작물을 덮지 않는다). 예외: state.json 없이 도구 산출(`inputs/`)과 빈 `payload/` 만
    있으면(끊긴 publish) 이어서 쓴다 — inputs 는 발행기 구동이 다시 쓰는 도구 JSON 이다(손 JSON 0)."""
    if not draft.exists():
        return
    if not draft.is_dir():
        core.fail("HINT_DRAFT_EXISTS", f"draft 자리가 디렉터리가 아니다: {draft}")
    if (draft / STATE_NAME).exists():
        core.fail("HINT_DRAFT_EXISTS", f"이미 publish 된 draft 다: {draft}",
                  "이어서 저작하려면 `hint.py continue --draft <draft>` · 새로 시작하려면 draft 를 지우고 publish 를 다시 한다.")
    extra = [p.name for p in draft.iterdir() if p.name not in ("inputs", "payload")]
    payload_files = [p for p in (draft / "payload").rglob("*") if p.is_file()] if (draft / "payload").is_dir() else []
    if extra or payload_files:
        core.fail("HINT_DRAFT_EXISTS", f"draft 자리가 비어 있지 않다(끊긴 publish 의 잔재): {draft} · {sorted(extra)[:5]} "
                                       f"· payload 파일 {len(payload_files)}건",
                  "잔재를 확인하고 draft 디렉터리를 지운 뒤 publish 를 다시 한다(잔재가 페이로드에 섞이지 않게). 캠페인 셀은 "
                  "발행 기록이 첫 시각으로 이미 묶였을 수 있다 — 끊긴 publish 와 **같은** --generated-utc 로 다시 실행한다.")
    if (draft / "payload").is_dir():
        shutil.rmtree(draft / "payload")


def _load_state(draft: Path) -> dict:
    st = core.read_json(draft / STATE_NAME, code="HINT_DRAFT_STATE_UNREADABLE")
    if not isinstance(st, dict) or st.get("schema_version") != STATE_SCHEMA or not st.get("tag") or not st.get("topic"):
        core.fail("HINT_DRAFT_STATE_UNREADABLE", f"state.json 모양이 아니다: {draft / STATE_NAME}",
                  "hint.py publish 가 만든 draft 에서 실행한다(state.json 은 도구가 쓴다 · 손으로 고치지 않는다).")
    st.setdefault("steps", {})
    return st


def _save_state(draft: Path, st: dict) -> None:
    core.write_json(draft / STATE_NAME, st)


def _drafts(repo: Path) -> list[tuple[Path, dict]]:
    root = repo / core.REL_DRAFTS
    out = []
    if root.is_dir():
        for sp in sorted(root.glob(f"*/{STATE_NAME}")):
            try:
                doc = json.loads(sp.read_text(encoding="utf-8"))
            except (OSError, ValueError):
                continue
            if isinstance(doc, dict) and doc.get("schema_version") == STATE_SCHEMA:
                out.append((sp.parent, doc))
    return out


def _find_draft(repo: Path, *, draft: str | None, campaign: str | None, cell: str | None, node: str | None) -> Path:
    if draft:
        d = Path(draft).resolve()
        if not (d / STATE_NAME).is_file():
            core.fail("HINT_DRAFT_ABSENT", f"state.json 이 없다 — publish 된 draft 가 아니다: {d}",
                      "hint.py publish 를 먼저 실행한다(출력의 draft 경로를 --draft 로 넘긴다).")
        return d
    if not (campaign and cell):
        core.fail("HINT_SELECTOR_ABSENT", "continue 대상이 없다 — `--draft DIR` 또는 `--campaign ID --cell CELL`.")
    hits = [d for d, st in _drafts(repo) if (st.get("selectors") or {}).get("campaign") == campaign
            and (st.get("selectors") or {}).get("cell") == cell
            and (node is None or (st.get("selectors") or {}).get("node") == node)]
    if not hits:
        core.fail("HINT_DRAFT_ABSENT", f"{core.REL_DRAFTS}/ 에 캠페인 {campaign} 셀 {cell} 의 draft 가 없다.",
                  f"`hint.py publish --campaign {campaign} --cell {cell} --generated-utc <UTC>` 를 먼저 실행한다.")
    if len(hits) > 1:
        core.fail("HINT_DRAFT_AMBIGUOUS", f"draft 가 {len(hits)}개다: {[_show(repo, h) for h in hits]}",
                  "--node 또는 --draft 로 하나를 고른다.")
    return hits[0]


def _find_draft_for_tag(repo: Path, tagname: str) -> Path | None:
    hits = [d for d, st in _drafts(repo) if st.get("tag") == tagname]
    if len(hits) > 1:
        core.fail("HINT_DRAFT_AMBIGUOUS", f"태그 {tagname} 의 draft 가 {len(hits)}개다 — --draft 로 고른다.")
    return hits[0] if hits else None


def _step(st: dict, name: str) -> dict | None:
    return (st.get("steps") or {}).get(name)


def _mark(draft: Path, st: dict, name: str, **data) -> None:
    st.setdefault("steps", {})[name] = data
    st["stage"] = name
    _save_state(draft, st)


# ── 증거 · 이름 ────────────────────────────────────────────────────────────────────────────────
def _evidence(repo: Path, *, campaign, cell, publication, node, rt: Runtime):
    if publication is not None:
        return evidence.from_publication(repo, publication, node, docker=rt.docker)
    return evidence.from_campaign(repo, campaign, cell, node, docker=rt.docker)


def _check_selectors(*, campaign, cell, publication, replay, require_replay: bool) -> None:
    if publication is not None:
        if campaign or cell:
            core.fail("HINT_SELECTOR_CONFLICT", "--publication 은 --campaign/--cell 과 함께 쓰지 않는다.")
        if require_replay and not replay:
            core.fail("HINT_REPLAY_FLAG_REQUIRED", "--publication 은 발행 기록 재생이다 — `--replay` 를 명시한다(읽기 전용 경로).")
        return
    if replay:
        core.fail("HINT_SELECTOR_CONFLICT", "--replay 는 --publication 과 함께만 쓴다.")
    if not (campaign and cell):
        core.fail("HINT_SELECTOR_ABSENT", "발행 대상이 없다 — `--campaign ID --cell CELL` 또는 `--publication TOPIC --replay`.")


def _collision(repo: Path, tagname: str, *, remote: str, remote_check: bool, replay: bool) -> str:
    """X13 이름 충돌(로컬·원격 ls-remote 읽기). 원격 조회 실패는 재생·--no-remote-check 에서만 견딘다(그 밖에서는 차단 —
    '조회 불가' 를 '충돌 없음' 으로 읽지 않는다)."""
    if not remote_check:
        hit, note = tag.name_collision(repo, tagname, None), "skipped(--no-remote-check · 로컬만 확인)"
    else:
        try:
            hit, note = tag.name_collision(repo, tagname, remote), f"{remote}: 조회함"
        except core.HintError as e:
            if not (replay and e.code == "HINT_REMOTE_QUERY_FAILED"):
                raise
            hit, note = tag.name_collision(repo, tagname, None), f"unreachable({remote} · 재생 모드라 로컬만 확인)"
    if hit:
        core.fail("HINT_NAME_COLLISION", f"파생 이름이 이미 있다({hit}): {tagname}",
                  "태그는 불변이다(P1 리콜 금지) — 같은 셀의 개정판이 필요하면 축이 모자란 것이다(2026-09-04 사용자 결정). "
                  "이 draft 가 봉인한 로컬 태그의 재개는 `hint.py continue` 가 한다.")
    return note


def _require_name_pii_clean(repo: Path, tagname: str) -> None:
    """파생 이름이 배포 PII 스캔(deploy)에 걸리면 **여기서** 멈춘다(부수효과 0). 태그 이름은 annotation footer · PAYLOAD.tag ·
    PROVENANCE.tag · 00 사실 블록에 실리므로 커밋(`branch.commit_payload` PII 게이트)·봉인에서 반드시 거부된다 — 그 거부를 저작이
    끝난 뒤에야 만나지 않게 한다(2026-09-22 리뷰 실측: D-g 4마디 릴리스 `0.10.x.y` 의 `10.x.y` 가 private-ipv4 패턴의 3옥텟 접두
    매치에 걸린다). 판정만 하고 PII-clean 을 인증하지 않는다 — 인증은 커밋·봉인 게이트(리터럴 필수)가 한다."""
    hits = pii.scan_text(tagname, pii.load_pii_terms(repo) or [], profile="deploy")
    if hits:
        core.fail("HINT_TAG_NAME_PII",
                  f"파생 이름이 배포 PII 스캔에 걸린다({', '.join(sorted({h.pattern for h in hits}))}) — 이 이름은 커밋·봉인에서 "
                  f"반드시 거부된다: {tagname}. 아무것도 쓰지 않았다.",
                  "4마디 vLLM 릴리스(0.10.x.y 등)는 private-ipv4 패턴의 3옥텟 접두 매치에 걸린다 — 스캐너 체계 변경은 사람 결정이다"
                  "(P3 · plan_26092119). pii_terms 리터럴이면 그 값이 이름(모델 슬러그 등)에 들어온 경로를 확인한다.")


def _require_unbound_publication_time(repo: Path, topic: str, utc: str) -> None:
    """끊긴 publish 의 재실행(부수효과 0): 셀 발행 기록이 이미 있고 다른 generated_utc 로 묶였으면 evidence_publisher `init` 이
    INIT_IMMUTABLE_REBIND 로 거부한다(발행 기록의 시각은 불변). 그 거부는 발행기 구동 **도중**(첫 draft 밖 쓰기 뒤)에 나고
    메시지가 무엇을 해야 하는지 말하지 않는다 — 여기서 먼저 막고 처방(같은 시각)을 적는다(2026-09-22 리뷰 실측: draft 를 지우고
    새 시각으로 다시 publish 하면 막다른 길이었다). 기록을 읽지 못하면 판정하지 않고 발행기의 판정에 맡긴다(그쪽이 fail-loud)."""
    rec = repo / core.REL_EVIDENCE_DIR / f"{topic}.json"
    if not rec.is_file():
        return
    try:
        doc = json.loads(rec.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return
    bound = doc.get("generated_utc") if isinstance(doc, dict) else None
    if isinstance(bound, str) and bound and bound != utc:
        core.fail("HINT_PUBLICATION_TIME_BOUND",
                  f"셀 발행 기록 `{core.REL_EVIDENCE_DIR}/{topic}.json` 이 이미 generated_utc={bound} 로 묶여 있다 — 다른 시각"
                  f"({utc})으로는 발행기 init 이 INIT_IMMUTABLE_REBIND 로 거부한다(발행 기록의 시각은 불변). 아무것도 쓰지 않았다.",
                  f"끊긴 publish 를 잇는 것이면 같은 시각으로 다시 실행한다: `--generated-utc {bound}`.")


def _bench_source(repo: Path, ev) -> str | None:
    """03 부하 곡선의 **원천**(lint 가 같은 원천으로 재렌더한다): 바인딩된 bench_report → 묶인 스윕 색인 → 없음."""
    rep = ev.bench_report_path
    if rep and (repo / rep).is_file():
        return rep
    sw = ev.sweep or {}
    if sw.get("dir") and (repo / sw["dir"] / "sweep_index.json").is_file():
        return f"{sw['dir']}/sweep_index.json"
    return None


def _with_bench_command(steps, bc) -> list:
    """재현 절차의 bench 단계에 evidence.bench_command(판정점 레벨 bench JSON 필드로 재구성한 `vllm bench serve …`)를 싣는다
    (2026-09-22 S2 round 2 · 공유 사실 계약). 1차 FACT:reproduce 의 bench 명령은 `03-benchmark.md §3.x` 자리표시였다. 산출물 쪽이 이미
    재구성 명령을 실었으면(`command_source` 가 `reconstructed…`) 건드리지 않는다 · 재구성 불가(None)면 원래 단계 그대로다(린터가
    자리표시를 막는다). 결과 JSON 이 들지 않는 인자는 명령 아래 주석으로 **모른다** 고 적는다(합성 ✗)."""
    out = json.loads(json.dumps(steps or []))
    if not isinstance(bc, dict) or not isinstance(bc.get("command"), str) or not bc["command"].strip():
        return out
    for s in out:
        if isinstance(s, dict) and s.get("phase") == "bench" and not str(s.get("command_source") or "").startswith("reconstructed"):
            lines = [bc["command"].strip()]
            if bc.get("unrecorded"):
                lines.append("# 결과 JSON 이 들지 않아 재구성하지 못한 인자(관측 불가): " + " · ".join(str(x) for x in bc["unrecorded"]))
            if bc.get("source"):
                lines.append(f"# {bc['source']}")
            s["command"], s["command_source"] = "\n".join(lines), "reconstructed(bench json)"
    return out


def _build_doc(ev, art: dict) -> dict:
    """PAYLOAD.build(SPEC §2.1) — evidence 관측 + artifacts 선택자(원장 > 셀 env > compose 기본값)."""
    b = ev.build_identity or {}
    bsrc = b.get("source") or {}
    build = {k: b.get(k) for k in _BUILD_KEYS}
    src = {k: bsrc[k] for k in _BUILD_KEYS if k in bsrc}
    sel = art.get("build") or {}
    if art.get("plane") == "native":
        build["track"], src["track"] = "native", art.get("plane_source") or "artifacts.plane_of"
        build["dockerfile"] = None
    elif sel.get("dockerfile"):
        build["dockerfile"], src["dockerfile"] = sel["dockerfile"], sel.get("dockerfile_source") or src.get("dockerfile")
        build["track"], src["track"] = sel.get("track"), f"artifacts 선택자({sel.get('dockerfile_source')})"
    build["source"] = src
    return build


# ── 승인 · 사실 갱신 ───────────────────────────────────────────────────────────────────────────
def _approval(repo: Path, st: dict, approved_by: str | None, approved_utc: str | None) -> dict:
    """O6/X3 — 부수효과 0. 캠페인 셀 = hint_targets 단일 원천 · 캠페인 밖 = CLI 의 사람 발화 전사."""
    sel = st.get("selectors") or {}
    if st.get("mode") == "campaign":
        if approved_by or approved_utc:
            core.fail("HINT_APPROVAL_SOURCE_CONFLICT", "캠페인 셀의 승인 원천은 campaign.yaml hint_targets 하나다(O6) — "
                                                       "--approved-by/--approved-utc 는 캠페인 밖(재생) 발행용이다.",
                      "캠페인 승인은 `campaign_init.py --hint-approve` 로 적는다(선언 확인·발행 팝업의 사람 발화 전사).")
        ap = evidence.approval_for(repo, sel.get("campaign"), sel.get("cell"), sel.get("node"))
        if ap is None:
            core.fail("HINT_APPROVAL_ABSENT",
                      f"캠페인 {sel.get('campaign')} 셀 {sel.get('cell')} 의 발행 승인이 없다(hint_targets[].approval) — 어느 ref 도 "
                      "움직이지 않았다.",
                      f"사람 Y/N 을 받아(제안 전용 · 무인 자동 승인 ✗) `python3 {core.REL_CAMPAIGN_INIT} --hint-approve "
                      f"--campaign-id {sel.get('campaign')} --node <배정 노드> --cells {sel.get('cell')} "
                      "--approved-by \"<사람 발화 전사>\" --utc <UTC>` 로 적은 뒤 continue 를 다시 실행한다.")
        return {k: ap[k] for k in APPROVAL_KEYS}
    text = (approved_by or "").strip()
    if not text or not approved_utc:
        core.fail("HINT_APPROVAL_ABSENT", "캠페인 밖 발행은 사람 승인 전사가 필요하다(--approved-by · --approved-utc) — 어느 ref 도 "
                                          "움직이지 않았다.",
                  "사람 Y/N 을 받아 그 발화를 전사해 `--approved-by \"<사람 발화 전사>\" --approved-utc <UTC>` 로 넘긴다(제안 전용).")
    text = _one_line_utterance(text, "HINT_APPROVAL_INVALID", "--approved-by")
    return {"approved_by": text, "approved_utc": core.require_utc(approved_utc, "--approved-utc"), "source": "cli:--approved-by"}


def _one_line_utterance(text: str | None, code: str, what: str) -> str:
    """사람 발화 전사 한 줄(승인 · 사실 검증 면제 공용 모양 검사) — 빈칸 · 여러 줄 · 자리표시 ✗."""
    s = (text or "").strip()
    if not s or "\n" in s or "\r" in s or s.startswith(("<<", "__")) or s.endswith((">>", "__")):
        core.fail(code, f"{what} 는 한 줄의 사람 발화 전사다(빈칸 · 자리표시 · 여러 줄 ✗).",
                  "사람 Y/N(결정)을 받아 그 발화를 역할과 요지로 전사한다 — 에이전트가 스스로 적지 않는다.")
    return s


def _factcheck_problems(doc, st: dict) -> list[str]:
    """factcheck.json 모양 검사(결정론 · 부수효과 0). 반환 = 결함 목록(빈 목록 = 모양 통과)."""
    if not isinstance(doc, dict):
        return ["최상위가 객체가 아니다"]
    bad: list[str] = []
    if doc.get("schema_version") != FACTCHECK_SCHEMA:
        bad.append(f"schema_version 이 {FACTCHECK_SCHEMA} 가 아니다: {doc.get('schema_version')!r}")
    if doc.get("tag") != st.get("tag"):
        bad.append(f"tag {doc.get('tag')!r} ≠ 이 draft 의 태그 {st.get('tag')!r}(다른 draft 의 보고를 옮기지 않는다)")
    who = {k: doc.get(k) for k in ("author", "checker")}
    for k, v in who.items():
        if not isinstance(v, str) or not v.strip():
            bad.append(f"{k} 가 비었다(역할 이름으로 적는다 — 사람 이름 ✗)")
    if all(isinstance(v, str) and v.strip() for v in who.values()) and \
            who["author"].strip().casefold() == who["checker"].strip().casefold():
        bad.append("checker == author — 사실 검증은 저작자가 아닌 Agent 가 한다(독립성)")
    try:
        core.require_utc(doc.get("checked_utc"), "checked_utc")
    except core.HintError as e:
        bad.append(e.message)
    n = doc.get("claims_checked")
    if isinstance(n, bool) or not isinstance(n, int) or n < 1:
        bad.append(f"claims_checked 는 1 이상의 정수다: {n!r}")
    cls = doc.get("classes_checked")
    want = sorted(template.FACTCHECK_CLASSES)
    if not isinstance(cls, list) or sorted(x for x in cls if isinstance(x, int) and not isinstance(x, bool)) != want \
            or len(cls) != len(want):
        bad.append(f"classes_checked 는 7부류 전부 {want} 다(부류 = template.FACTCHECK_CLASSES): {cls!r}")
    items = doc.get("items")
    if not isinstance(items, list):
        bad.append("items 는 목록이다(지적 0건이면 빈 목록)")
        items = []
    seen: set[str] = set()
    for i, it in enumerate(items):
        if not isinstance(it, dict):
            bad.append(f"items[{i}] 가 객체가 아니다")
            continue
        iid = it.get("id")
        if not isinstance(iid, str) or not iid.strip() or iid in seen:
            bad.append(f"items[{i}].id 가 비었거나 중복이다: {iid!r}")
        else:
            seen.add(iid)
        if it.get("verdict") not in FACTCHECK_VERDICTS:
            bad.append(f"items[{i}].verdict ∈ {FACTCHECK_VERDICTS}: {it.get('verdict')!r}")
        c = it.get("class")
        if isinstance(c, bool) or not isinstance(c, int) or not (0 <= c <= max(want)):
            bad.append(f"items[{i}].class 는 0(기타) 또는 부류 번호 1..{max(want)} 다: {c!r}")
        for k in ("where", "claim"):
            if not isinstance(it.get(k), str) or not it[k].strip():
                bad.append(f"items[{i}].{k} 가 비었다")
        if it.get("status") not in FACTCHECK_STATUSES:
            bad.append(f"items[{i}].status ∈ {FACTCHECK_STATUSES}: {it.get('status')!r}")
        elif it.get("status") == "fixed" and (not isinstance(it.get("resolution"), str) or not it["resolution"].strip()):
            bad.append(f"items[{i}] 가 fixed 인데 resolution(무엇을 어떻게 고쳤나)이 없다")
    return bad


def _factcheck_eval(draft: Path, st: dict) -> tuple[dict, core.HintError | None]:
    """발행 전 독립 사실 검증 보고 판정(부수효과 0). 반환 (facts.factcheck 요약, 막을 오류 | None)."""
    p = draft / "inputs" / FACTCHECK_NAME
    rel = f"inputs/{FACTCHECK_NAME}"
    if not p.is_file():
        return ({"status": "absent", "reason_code": "HINT_FACTCHECK_ABSENT", "open": [], "source": rel},
                core.HintError("HINT_FACTCHECK_ABSENT",
                               f"발행 전 독립 사실 검증 보고가 없다(draft {rel}) — 린트 0 은 인용 무결성만 본다(주장의 참 ✗).",
                               "저작자가 아닌 Agent 가 페이로드의 사실 주장을 전부 뽑아 인용 출처와 대조하고(7부류 체크리스트 · SKILL.md §2) "
                               f"{rel} 로 남긴다 → 저작자가 고친 뒤 lint → continue. 사람이 면제를 결정했으면 "
                               "`--factcheck-waiver \"<사람 발화 전사>\"`."))
    try:
        doc = json.loads(p.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, ValueError) as e:
        doc = e
    probs = [f"읽을 수 없다: {doc}"] if isinstance(doc, Exception) else _factcheck_problems(doc, st)
    if probs:
        return ({"status": "invalid", "reason_code": "HINT_FACTCHECK_INVALID", "open": [], "source": rel},
                core.HintError("HINT_FACTCHECK_INVALID", f"{rel} 모양 결함 {len(probs)}건: " + " · ".join(probs[:8]),
                               "SKILL.md §2 의 factcheck.json 형식대로 검증 Agent 가 다시 쓴다(저작자가 대신 쓰지 않는다)."))
    items = doc.get("items") or []
    open_ids = [it["id"] for it in items if it.get("verdict") in FACTCHECK_VERDICTS and it.get("status") != "fixed"]
    base = {"checker": doc["checker"].strip(), "author": doc["author"].strip(), "checked_utc": doc["checked_utc"],
            "claims_checked": doc["claims_checked"], "items": len(items),
            "fixed": sum(1 for it in items if it.get("status") == "fixed"), "open": open_ids, "source": rel}
    if open_ids:
        return ({**base, "status": "open", "reason_code": "HINT_FACTCHECK_OPEN"},
                core.HintError("HINT_FACTCHECK_OPEN",
                               f"사실 검증의 열린 지적 {len(open_ids)}건(오답 · 오도 · 근거 없음 중 status≠fixed): {open_ids[:12]}",
                               "저작자가 각 지적을 페이로드에서 고치고(또는 주장을 빼고) 그 항목을 status=fixed + resolution 으로 닫은 뒤 "
                               "lint → continue. 이견(disputed)은 열린 채다 — 사람이 결정한다(--factcheck-waiver)."))
    return {**base, "status": "passed", "reason_code": None}, None


def _factcheck_gate(draft: Path, st: dict, waiver: str | None, utc: str) -> tuple[dict, core.HintError | None]:
    """판정 + 사람 면제(있으면). 면제 전사의 모양은 호출부가 **부수효과 전에** 이미 검사했다."""
    summary, err = _factcheck_eval(draft, st)
    if err is None:
        if waiver:
            _log("--factcheck-waiver 는 쓰지 않았다 — 사실 검증 보고가 통과했다.")
        return summary, None
    if not waiver:
        return summary, err
    return ({**summary, "status": "waived", "reason_code": err.code, "waiver": waiver, "waived_utc": utc}, None)


def _slot_rationales(payload: Path, slots: dict) -> dict:
    """01 §1.2 의 `### <슬롯 또는 파일명>` 소제목 → 그 첫 문단 요약을 slots[*].rationale 로 · confidence 재계산
    (artifacts.slot_confidence · 3신호 = 파일 × 적용 증거 × 선언). 수집 시점엔 선언이 없어 최대 2-signal 이었다."""
    subs = template.subsections(payload, "01-artifacts", "1.2")
    out = json.loads(json.dumps(slots or {}))
    for name, row in out.items():
        if not isinstance(row, dict):
            continue
        keys = {name} | {Path(str(f)).name for f in (row.get("files") or [])}
        texts = [s["text"] for s in subs if (s["key"] in keys or s["title"] in keys) and s["text"]]
        rat = " · ".join(texts) if texts else None
        if rat and len(rat) > _RATIONALE_MAX:
            rat = rat[:_RATIONALE_MAX - 1].rstrip() + "…"
        row["rationale"] = rat
        row["confidence"] = artifacts.slot_confidence(row)
    return out


def _update_payload_json(payload: Path, **fields) -> None:
    p = payload / branch.PAYLOAD_JSON
    doc = core.read_json(p, code="HINT_PAYLOAD_JSON_UNREADABLE")
    doc.update(fields)
    core.write_json(p, doc)


def _finalize_provenance(repo: Path, payload: Path, st: dict) -> None:
    """PROVENANCE.source_anchor(= publish 때 메인 체크아웃 HEAD — 산출물을 모은 소스 커밋) · source_anchor_is_head(= 지금도 HEAD 인가) ·
    assembly_branch · payload_files(실물). publish 는 앵커를 비워 둔다(SPEC §4.1 ⑦) — 커밋 직전에 한 번 채운다."""
    p = payload / branch.PROVENANCE_JSON
    prov = core.read_json(p, code="HINT_PROVENANCE_UNREADABLE")
    head = core.git(repo, "rev-parse", "--verify", "--quiet", "HEAD", check=False).stdout.strip() or None
    sa = st.get("source_anchor")
    prov["source_anchor"] = sa
    prov["source_anchor_is_head"] = (sa == head) if sa else None
    br = core.git(repo, "rev-parse", "--abbrev-ref", "HEAD", check=False)
    prov["assembly_branch"] = br.stdout.strip() if br.returncode == 0 and br.stdout.strip() else None
    core.write_json(p, prov)
    prov["payload_files"] = branch.payload_files(payload)
    core.write_json(p, prov)


def _require_draft_equals_anchor(repo: Path, payload: Path, anchor: str) -> None:
    """재실행: 커밋 뒤 draft 가 바뀌지 않았는가(git 이 든 바이트 = 디스크 바이트). 바뀌었으면 hint 브랜치는 앞으로만 간다 —
    같은 이름으로 다른 내용을 봉인하지 않는다."""
    want = {e["path"]: e["sha"] for e in branch.tree_entries(repo, anchor)}
    rels = branch.payload_files(payload)
    got: dict[str, str] = {}
    if rels:
        # 커밋 때와 같은 규칙(`--no-filters` · 쓰기 ✗): .gitattributes 가 바이트를 바꾸면 같은 파일이 다른 해시가 된다.
        out = core.git(repo, "hash-object", "--no-filters", "--stdin-paths",
                       input_text="".join(f"{payload / r}\n" for r in rels)).stdout.split()
        got = dict(zip(rels, out))
    if got != want:
        diff = sorted(set(got.items()) ^ set(want.items()))
        core.fail("HINT_DRAFT_CHANGED_AFTER_COMMIT",
                  f"커밋({anchor[:12]}) 뒤 draft 페이로드가 바뀌었다: {[p for p, _ in diff][:10]}",
                  "hint 브랜치는 앞으로만 간다 — 개정이 필요하면 새 publish(새 이름)로 낸다. 커밋한 내용으로 되돌리려면 "
                  "branch.export_tree 로 앵커 트리를 풀어 draft 를 맞춘다.")


def _manifest_doc(repo: Path, st: dict) -> dict:
    return core.read_json(repo / st["manifest"], code="HINT_MANIFEST_UNREADABLE")


def _require_binding_artifact(man: dict, repo: Path, *, before_commit: bool) -> str:
    """footer `certificate_ref`(evidence.binding_artifact_path — 단일 결정자). 없으면 봉인할 수 없다."""
    ref = evidence.binding_artifact_path(man, repo)
    if not ref:
        core.fail("HINT_CERTIFICATE_BINDING_ABSENT",
                  "footer certificate_ref 로 묶을 인증서·리포트가 없다(binding_artifact_path = None)"
                  + (" — 커밋하지 않았다." if before_commit else "."),
                  "full PASS 면 인증서를, waiver·explore·map_only 면 bench_report 를 발행 기록에 바인딩한다(evidence_publisher).")
    return ref


# ── 린트 ───────────────────────────────────────────────────────────────────────────────────────
def _subst_table(repo: Path, st: dict) -> list:
    return pii.substitution_table(repo, evidence.output_manifest(repo, st["topology"]))


def _lint(repo: Path, payload: Path, draft: Path, st: dict, *, sealed: bool) -> list[dict]:
    """template.lint — publish 때와 같은 입력(스냅샷 facts · LINEAGE · 치환표 · work-manifest · 벤치 원천)."""
    lin = core.read_json(payload / "LINEAGE.json", code="HINT_LINEAGE_UNREADABLE")
    facts = template.load_snapshot(draft / "inputs" / template.FACTS_SNAPSHOT)
    man = _manifest_doc(repo, st) if (repo / st["manifest"]).is_file() else None
    # draft_dir = 측정 도구 스냅샷(`inputs/sources/`)의 자리 — payload 는 임시 사본(lint) · 봉인 트리를 푼 자리(verify)일 수 있다(2026-09-22 round 3)
    return template.lint(repo, payload, lineage=lin, subst_table=_subst_table(repo, st), manifest=man,
                         bench_report=st.get("bench_source"), facts=facts, sealed=sealed, draft_dir=draft)


def _lint_fn(draft: Path | None, st: dict | None):
    """tag.verify_local 의 서사 린터 주입구: 봉인된 **git 오브젝트** 트리를 임시 디렉터리에 풀어(branch.export_tree) sealed 린트."""
    def fn(*, repo, anchor, tag):  # noqa: A002 — tag.verify_local 의 키워드 계약
        if draft is None or st is None:
            return [{"code": "HINT_LINT_DRAFT_ABSENT",
                     "message": f"{tag} 를 봉인한 draft(inputs/template_facts.json)를 찾지 못했다 — 서사 린트를 건너뛰지 않는다 "
                                "(--draft 로 지정)"}]
        with tempfile.TemporaryDirectory(prefix="hint-verify-") as td:
            dst = Path(td) / "payload"
            branch.export_tree(Path(repo), anchor, dst)
            return _lint(Path(repo), dst, draft, st, sealed=True)
    return fn


def _fail_lint(issues: list[dict], sealed: bool) -> None:
    codes = sorted({i["code"] for i in issues})
    lines = [f"{i['code']}  {i['file']}:{i['line']}  {i['message']}" for i in issues[:40]]
    core.fail("HINT_LINT_FAILED", f"{'봉인 뒤 ' if sealed else ''}린트 {len(issues)}건({', '.join(codes)}) — 커밋하지 않았다:\n  "
              + "\n  ".join(lines) + ("\n  …" if len(issues) > 40 else ""),
              "PROMPT 지시대로 저작을 고친 뒤 `hint.py lint --draft <draft>` 로 확인하고 continue 를 다시 실행한다.")


# ── publish ────────────────────────────────────────────────────────────────────────────────────
def publish(repo: Path, *, campaign=None, cell=None, publication=None, replay=False, node=None, generated_utc,
            lineage_add=(), out=None, remote="origin", remote_check=True, rt: Runtime | None = None) -> dict:
    """셀 1개 → 스캐폴드 → 정지(SPEC §4.1). 반환 = 요약(태그 · draft · PROMPT · 읽기 목록 · 다음 명령)."""
    rt = rt or Runtime()
    repo = Path(repo).resolve()
    utc = core.require_utc(generated_utc)
    _check_selectors(campaign=campaign, cell=cell, publication=publication, replay=replay, require_replay=True)
    out_dir = Path(out).resolve() if out else None
    if out_dir is not None:
        _require_fresh_draft(out_dir)
    # ① 증거(명시 id 만 · ACTIVE ✗) ② 발행 자격 = 관측(X8 · 불성립 = 차단)
    ev = _evidence(repo, campaign=campaign, cell=cell, publication=publication, node=node, rt=rt)
    qual = evidence.qualification(ev)
    ident = evidence.identity(ev)
    label = evidence.topology_label(ident)
    # ③ 이름(도구 전량 파생 · 충돌 = 차단) — 발행기 구동(첫 draft 밖 쓰기) **전에** 끝낸다: 이름이 막히면 부수효과 0.
    dn = naming.derive_name(evidence.naming_facts(repo, ev), naming.load_vocab(repo))
    tag.check_ref_format(repo, dn.tag)
    _require_name_pii_clean(repo, dn.tag)
    remote_note = _collision(repo, dn.tag, remote=remote, remote_check=remote_check, replay=replay)
    topic = evidence.topic_for(ev)
    draft = out_dir or (repo / core.REL_DRAFTS / (topic if ev.mode == "campaign" else f"replay__{topic}"))
    _require_fresh_draft(draft)
    payload = draft / "payload"
    # ④ 발행기 구동(campaign) + 게이트 사전 확인 · 재생 = 기존 기록 읽기만
    if ev.mode == "campaign":
        _require_unbound_publication_time(repo, topic, utc)
        manifest_path = evidence.drive_publisher(repo, ev, topic=topic, generated_utc=utc, draft_dir=draft)
        _gate(repo, manifest_path, "continue")
        gate = "allowed(hint_finalize 사전 확인 · 부수효과 전)"
    else:
        mp = (ev.publication or {}).get("manifest_path")
        if not mp:
            core.fail("HINT_PUBLICATION_MANIFEST_ABSENT", f"발행 기록 {topic} 의 work-manifest 가 없다 — 재생할 승격 기록이 없다.")
        manifest_path = repo / mp
        # 게이트 사전 확인은 읽기(completion_gate authorize 는 판정을 stdout 에 낼 뿐 쓰지 않는다)라 재생에서도 묻는다 — 옛 기록이
        #   승격 불가면 저작을 다 한 뒤 continue 에서야 알게 된다(2026-09-22 리뷰).
        _gate(repo, manifest_path, "continue")
        gate = "allowed(hint_finalize 사전 확인 · 읽기 전용 · 재생)"
    manifest_rel = core.rel(repo, manifest_path)
    man = core.read_json(manifest_path, code="HINT_MANIFEST_UNREADABLE")
    task_class = man.get("task_class")
    bench = man.get("benchmark") if isinstance(man.get("benchmark"), dict) else {}
    # ⑤ 계보(X1 · X17) ⑥ 산출물(적용된 것만 · §4.5)
    # 2026-09-22 · plan_26092119 S2 round 3(공유 사실 계약): 측정 도구 원문 스냅샷(draft `inputs/sources/<이름>@<rev12>` · git show 바이트)과
    #   측정 env 관측은 계보보다 **먼저** 만든다 — 생산자(evidence)가 그 경로 · 로그를 ev.lineage_seeds 의 evidence_candidates 에 올려야
    #   LINEAGE 가 그것을 발췌 출처로 싣는다(뒤에 부르면 발췌가 HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE). 쓰기는 draft 안뿐이다(재생도 같다).
    tool_snaps = evidence.tool_snapshots(repo, ev, draft)
    env_observed = evidence.measurement_env_observed(repo, ev)
    att_scope = evidence.attestation_scope(repo, ev)
    lin = lineage.derive(repo, ev, base_slug=ident["base_slug"], publish_kst=core.kst_token(utc),
                         declared=list(lineage_add or ()), records=evidence.publication_records(repo), this_topic=topic)
    art = artifacts.collect(repo, ev, payload, runner=rt.docker)
    bench_src = _bench_source(repo, ev)
    bench_md = template.render_bench_source(repo, bench_src) if bench_src else None
    # ⑦ 사실 조립 → 스캐폴드
    build = _build_doc(ev, art)
    measurement = evidence.measurement(ev)                 # D-f 측정 수치
    measurement_config = evidence.measurement_config(repo, ev)   # D-f DISTRIBUTED_MEASUREMENT_KEYS + source
    # 2026-09-22 S2 round 2(공유 사실 계약): 블랙박스 이벤트 원장의 이 셀 행 · 현행 full 정의(docs.md 원문) — 1차 저작자가 "두 번 띄웠는지
    #   미기록"(원장에 선언 2건) · "현행 full 정의 미기록"(docs.md 에 있다)으로 적은 두 빈칸을 기계가 채운다(생산자 = evidence).
    event_timeline = evidence.event_timeline(repo, ev)
    bench_definition = evidence.bench_definition(repo, ev)
    missing = sorted(set(ev.missing) | set(art.get("missing") or ()))
    naming_doc = {**dn.to_payload(), "vllm_observed": evidence.vllm_observed(ev, repo)}
    camp = {"id": ev.campaign_id, "cell": ev.cell, "node": ev.node, "mode": ev.mode}
    pub = {"topic": topic, "manifest_ref": manifest_rel, "task_class": task_class}
    facts = {
        "tag": dn.tag, "generated_utc": utc, "naming": naming_doc, "identity": ident, "build": build,
        "plane": art.get("plane"), "applied_set": art.get("applied_set"), "slots": art.get("slots"),
        "qualification": qual, "measurement": measurement, "measurement_config": measurement_config, "missing": missing,
        "lineage_reading_list": lineage.reading_list(lin), "value_status_candidates": art.get("value_status_candidates"),
        "reproduce_steps": _with_bench_command(art.get("reproduce_steps"), evidence.bench_command(repo, ev)),
        "sub_recipe": art.get("sub_recipe"), "bench_section_md": bench_md,
        "task_class": task_class, "perf_waiver": bench.get("perf_waiver") or None, "campaign": camp, "approval": None,
        "publication": pub, "event_timeline": event_timeline, "bench_definition": bench_definition, "factcheck": None,
        "measurement_env_observed": env_observed, "tool_snapshots": tool_snaps, "attestation_scope": att_scope,
    }
    prompts = template.render_scaffold(repo, facts, payload)
    core.write_json(payload / "LINEAGE.json", lin)
    core.write_json(payload / branch.PAYLOAD_JSON, {
        "schema_version": PAYLOAD_SCHEMA_VERSION, "format": branch.PAYLOAD_FORMAT, "tag": dn.tag, "generated_utc": utc,
        "campaign": camp, "identity": ident, "naming": naming_doc, "plane": art.get("plane"), "build": build,
        "applied_set": art.get("applied_set"), "slots": art.get("slots"),
        "qualification": {k: qual.get(k) for k in ("health_200", "inference_observed", "sources", "method")},
        "measurement": measurement, "measurement_config": measurement_config, "missing": missing, "approval": None,
        "evidence_pointers": [{k: p.get(k) for k in ("kind", "path", "cell_id", "node_id")} for p in ev.pointers],
        "publication": pub, "bench_definition": bench_definition, "factcheck": None,
        # 2026-09-22 S2 round 3: Agent 표면에도 같은 사실(측정 env 관측 · 도구 스냅샷 좌표 · attestation 범위) — FACT 블록과 같은 값
        "measurement_env_observed": env_observed, "tool_snapshots": tool_snaps, "attestation_scope": att_scope,
    })
    prov = {"schema_version": PROVENANCE_SCHEMA_VERSION, "tag": dn.tag, "source_anchor": None,
            "source_anchor_is_head": None, "assembly_branch": None, "payload_files": [], "generated_utc": utc}
    core.write_json(payload / branch.PROVENANCE_JSON, prov)
    prov["payload_files"] = branch.payload_files(payload)
    core.write_json(payload / branch.PROVENANCE_JSON, prov)
    head = core.git(repo, "rev-parse", "--verify", "--quiet", "HEAD", check=False).stdout.strip() or None
    approval_note = _approval_probe(repo, ev)
    st = {"schema_version": STATE_SCHEMA, "stage": "scaffolded", "mode": ev.mode,
          "selectors": {"campaign": ev.campaign_id, "cell": ev.cell, "node": ev.node,
                        "publication": topic if ev.mode != "campaign" else None},
          "tag": dn.tag, "topic": topic, "manifest": manifest_rel, "topology": ev.topology, "topology_label": label,
          "generated_utc": utc, "bench_source": bench_src, "source_anchor": head, "remote_check": remote_note,
          "gate_precheck": gate, "approval_at_publish": approval_note, "prompts": len(prompts),
          "lineage_documents": len(lin.get("documents") or ()),
          "steps": {"publish": {"utc": utc, "missing": missing}}}
    _save_state(draft, st)
    return {"tag": dn.tag, "draft": draft, "payload": payload, "topic": topic, "mode": ev.mode, "manifest": manifest_rel,
            "prompts": prompts, "reading_list": lineage.reading_list(lin), "missing": missing, "remote_check": remote_note,
            "gate": gate, "approval": approval_note, "files": branch.payload_files(payload), "state": st,
            "applied": _applied_summary(art.get("applied_set")),
            "tool_snapshots": [Path(str(x.get("snapshot_rel"))).name for x in tool_snaps
                               if isinstance(x, dict) and x.get("snapshot_rel")]}


def _applied_summary(aset) -> dict:
    """publish 출력용 적용 판정 요약(읽기 전용 · 판정 ✗ — 판정은 artifacts 가 했다). 이미지 탐침이 돌지 않은 채 미관측이 남으면 그 사실과
    처방을 출력이 말한다(2026-09-22 S2 round 2 적대 리뷰: 탐침 없는 publish 는 태그2 셀에서 9개 중 7개를 다시 `unobserved` 로 실었다 —
    FACT 의 탐침 표에만 `skipped` 로 남고 발행자에게는 조용했다). round 3 부터 탐침은 기본이라 처방은 "`--docker-read-only` 를 뺀다" 다."""
    aset = aset if isinstance(aset, dict) else {}
    patches = [x for x in (aset.get("patches") or []) if isinstance(x, dict)]
    probe = next((str(x.get("result")) for x in (aset.get("probes") or []) if isinstance(x, dict) and x.get("tier") == "image-probe"),
                 None)
    return {"total": len(patches), "unobserved": sum(1 for x in patches if str(x.get("result") or "").startswith("unobserved")),
            "image_probe": probe}


def _approval_probe(repo: Path, ev) -> str:
    """publish 출력용 승인 상태(읽기 전용 · 판정 ✗ — 판정은 continue 의 첫 단계다). 모르는 것은 모른다고 적는다."""
    if ev.mode != "campaign":
        return "캠페인 밖 — continue 에 --approved-by/--approved-utc(사람 발화 전사)"
    try:
        ap = evidence.approval_for(repo, ev.campaign_id, ev.cell, ev.node)
    except core.HintError as e:
        return f"기록 결함 {e.code} — continue 가 막힌다(campaign_init --hint-approve 로 다시 적는다)"
    if ap is None:
        return "없음 — continue 전에 사람 Y/N 을 받아 campaign_init --hint-approve 로 적는다"
    return f"있음({ap.get('hitl_source')} · {ap.get('approved_utc')})"


def _print_publish(repo: Path, res: dict) -> None:
    d = _show(repo, res["draft"])
    print(f"{_OUT} publish — 스캐폴드 완료 · 정지(태그·브랜치 부수효과 0)")
    print(f"  태그     {res['tag']}")
    print(f"  draft    {d}")
    print(f"  모드     {res['mode']} · 토픽 {res['topic']} · work-manifest {res['manifest']}")
    print(f"  원격 충돌 확인 {res['remote_check']} · 게이트 {res['gate']}")
    print(f"  승인     {res['approval']}")
    print(f"  결손     {', '.join(res['missing']) or '없음'}")
    ap = res.get("applied") or {}
    if ap.get("total"):
        print(f"  적용 판정 빌드 패치 {ap['total']}개 중 미관측 {ap['unobserved']}개 · 이미지 탐침 {ap.get('image_probe') or '기록 없음'}")
        if ap["unobserved"] and not str(ap.get("image_probe") or "").startswith("done"):
            print("  ⚠ 이미지 탐침이 돌지 않은 채 판정했다 — 탐침(시작하지 않는 컨테이너의 create · cp · rm)은 기본이다: `--docker-read-only` "
                  "로 껐다면 빼고 다시 publish 한다(새 --out 또는 draft 를 지운 뒤) · 측정 이미지가 이 호스트에 없으면 탐침할 수 없다"
                  "(미관측 그대로 기재 · 이미지를 받거나 다시 지을지는 사람 결정).")
    print(f"  페이로드 파일 {len(res['files'])}개")
    todo = [p for p in res["prompts"] if not p.get("optional")]
    print(f"\n채워야 할 PROMPT {len(todo)}개(선택 {len(res['prompts']) - len(todo)}개) — `<<AGENT:` 줄을 지시대로 바꾼다:")
    for p in res["prompts"]:
        extra = []
        if p.get("blocks"):
            extra.append("블록 " + ", ".join(f"{k}≥{v}" if v else k for k, v in p["blocks"].items()))
        if p.get("excerpts_required"):
            extra.append(f"발췌 ≥{p['excerpts_required']}")
        if p.get("optional"):
            extra.append("선택")
        print(f"  {p['file']} §{p['section']} {p.get('title', '')} — {p['question']}" + (f"  [{' · '.join(extra)}]" if extra else ""))
    rl = res["reading_list"]
    total = sum(int(x.get("bytes") or 0) for x in rl)
    print(f"\nLINEAGE 읽기 목록 {len(rl)}건 · {total:,} B(계층 → 깊이 → 관련도 → 날짜 · 모호 해소 문서는 뒤):")
    for x in rl[:60]:
        print(f"  d{x.get('depth')} {x.get('kind') or '?':<10} {x['path']}  ({int(x.get('bytes') or 0):,} B"
              + (" · 모호 해소" if x.get("tier") == "ambiguous" else "") + ")")
    if len(rl) > 60:
        print(f"  … 외 {len(rl) - 60}건 — {d}/payload/LINEAGE.json")
    cli = _cli(repo)
    print("\n발췌는 **치환 후 원문**에서 그대로 옮긴다(자리표시를 추측하지 않는다):")
    # 2026-09-22 · plan_26092119 S2 round 3 적대 리뷰: 저작 Agent 가 읽는 안내가 새 출처(측정 도구 스냅샷)와 `--numbered` 를 말하지 않으면
    #   §L 번호를 다시 원본 `grep -n` 으로 얻는다(2차 저작자의 번호 어긋남 — 그 기능이 생긴 이유). 안내가 기능을 가리킨다.
    print(f"  {cli} excerpt --draft {d} --source <문서 stem|경로|artifacts 파일명|<도구>@<rev12>> [--lines a-b] [--numbered]")
    print("  (비-마크다운 출처의 발췌 머리 `§L<a>-<b>` 는 --numbered 가 줄마다 붙이는 `L<n>` 번호 그대로)")
    snaps = [str(x) for x in res.get("tool_snapshots") or ()]
    print("  측정 도구 원문 스냅샷(발췌 출처 토큰 · 03 §3.4): " + (" · ".join(snaps) if snaps else
                                                           "없음 — 도구가 정한 인자는 원문 없이 단정하지 않는다"))
    print("저작 중 확인(부수효과 0):")
    print(f"  {cli} lint --draft {d}")
    nxt = f"{cli} continue --draft {d} --generated-utc {_UTC_EXAMPLE}"
    if res["mode"] != "campaign":
        nxt += ' --approved-by "<사람 발화 전사>" --approved-utc <UTC>'
    print(f"다음:\n  {nxt}")


def _publish_runner(a):
    """publish 의 docker 실행기. 이미지 탐침이 기본이다(2026-09-22 · plan_26092119 S2 round 3) — `--docker-read-only` 만 끈다 ·
    `--docker-probe` 는 옛 이름(no-op 별칭). None 을 넘기지 않는다 — evidence 와 artifacts 가 **같은 실행기 하나**를 지나게 한다
    (evidence 의 None = 가드된 실물 subprocess · artifacts 의 None = 자기 탐침 전용 기본 실행기로 뜻이 갈린다). 두 실행기 모두 run ·
    start · build 를 거부한다. 2026-09-22 S2 round 3 통합 정정: 옛 문구 "None 이면 artifacts 가 제한 없는 실물 docker 를 부른다" 는
    artifacts 의 None 이 탐침 전용 기본 실행기가 된 뒤로 거짓이었다. 선언 없는 주입 실행기(`--docker-read-only` · 자체검사 대역)는
    artifacts 가 **읽기 전용**으로 본다(탐침 ✗)."""
    return _docker_read_only() if a.docker_read_only else _docker_probe_only()


def cmd_publish(a) -> int:
    repo = core.resolve_repo(a.repo)
    rt = Runtime(docker=_publish_runner(a))
    res = publish(repo, campaign=a.campaign, cell=a.cell, publication=a.publication, replay=a.replay, node=a.node,
                  generated_utc=a.generated_utc, lineage_add=a.lineage_add, out=a.out, remote=a.remote,
                  remote_check=not a.no_remote_check, rt=rt)
    _print_publish(repo, res)
    return 0


# ── continue ───────────────────────────────────────────────────────────────────────────────────
def continue_(repo: Path, *, draft: Path, generated_utc, approved_by=None, approved_utc=None, remote="origin",
              no_push=False, factcheck_waiver=None, rt: Runtime | None = None) -> dict:
    """승인 → 사실 갱신 → 린트 → 봉인 PROMPT → 게이트 → 커밋 → 승격 목표 → 봉인 → 로컬 검증 → (push → 원격 SHA → 카탈로그 →
    publish 위상). 각 단계는 state.json 에 남고 재실행은 끝난 단계를 확인만 한다(SPEC §4.2)."""
    rt = rt or Runtime()
    repo, draft = Path(repo).resolve(), Path(draft).resolve()
    utc_arg = core.require_utc(generated_utc)
    st = _load_state(draft)
    payload = draft / "payload"
    tagname, topic, label = st["tag"], st["topic"], st["topology_label"]
    # ① 승인 — 부수효과 0(O6 · 어느 ref 도 움직이기 전) · 사실 검증 면제 전사의 모양도 여기서(쓰기 전) 본다
    approval = _approval(repo, st, approved_by, approved_utc)
    waiver = (_one_line_utterance(factcheck_waiver, "HINT_FACTCHECK_WAIVER_INVALID", "--factcheck-waiver")
              if factcheck_waiver is not None else None)
    # ①' push 권위(D8) — push 까지 갈 실행이면 커밋·봉인 **전에** 본다(권위 없는 체크아웃이 hint 브랜치에 커밋·봉인까지 한 뒤에야
    #   push 에서 멈추면 되감지 않는 브랜치에 남는다). `--no-push` 는 봉인까지라 권위가 필요 없다(봉인은 분산 · D8).
    if not no_push:
        _require_central(repo, "continue(push 포함)")
    commit_step = _step(st, "commit")
    if commit_step is None:
        # 이름 충돌 재확인(X13 · 부수효과 0): publish 뒤 그 이름이 로컬·원격에 생겼을 수 있다(다른 draft · 타 PC · P2). 이 draft 는
        #   아직 커밋 전이라 봉인한 태그가 없다 — 있으면 남의 것이다. 커밋 **전에** 막는다(hint 브랜치는 되감지 않는다).
        #   원격 조회 실패는 --no-push 에서만 견딘다(push 할 원격을 못 보는데 봉인까지 가면 그 뒤에야 안다).
        _collision(repo, tagname, remote=remote, remote_check=True, replay=bool(no_push))
    else:
        # 재실행: 커밋 뒤 draft 가 그대로인가를 **먼저** 본다(부수효과 0) — 바뀐 draft 에서 brief·게이트로 가면 엉뚱한 사유로 멈춘다.
        _require_draft_equals_anchor(repo, payload, commit_step["anchor"])
    utc = st.get("commit_utc") or utc_arg
    if st.get("commit_utc") and st["commit_utc"] != utc_arg:
        _log(f"커밋 시각은 첫 커밋 때 적은 {st['commit_utc']} 를 다시 쓴다(재개 · 같은 메시지·시각이어야 중복 커밋이 없다).")
    # ② 기계 사실 갱신(승인 · 슬롯 적용 사유) → ③ refresh → 린트 → 봉인 → 린트(봉인 뒤)
    if commit_step is None:
        facts = template.load_snapshot(draft / "inputs" / template.FACTS_SNAPSHOT)
        slots = _slot_rationales(payload, facts.get("slots") or {})
        # 사실 검증 판정은 읽기뿐이다(부수효과 0) — 요약을 00 메타 · PAYLOAD 에 싣고, 막을 오류는 **린트 뒤에** 낸다(절차 순서 =
        #   저작 → 린트 0 → 독립 사실 검증 → continue · 미저작 draft 에는 린트 결함이 먼저 보여야 한다).
        fc_summary, fc_err = _factcheck_gate(draft, st, waiver, utc_arg)
        changed = template.update_facts(repo, payload, {"approval": approval, "slots": slots, "factcheck": fc_summary})
        _update_payload_json(payload, approval=approval, slots=slots, factcheck=fc_summary)
        _mark(draft, st, "approval", approval=approval, facts_changed=changed)
        template.refresh(payload)
        issues = _lint(repo, payload, draft, st, sealed=False)
        if issues:
            _mark(draft, st, "lint", ok=False, codes=sorted({i["code"] for i in issues}))
            _fail_lint(issues, sealed=False)
        _mark(draft, st, "lint", ok=True, sealed=False)     # 봉인 전 린트 통과(사실 검증 게이트에서 멈춰도 상태가 참을 말한다)
        _mark(draft, st, "factcheck", ok=fc_err is None, **{k: v for k, v in fc_summary.items() if k != "source"})
        if fc_err is not None:
            raise fc_err
        template.seal_prompts(payload)
        issues = _lint(repo, payload, draft, st, sealed=True)
        if issues:
            _mark(draft, st, "lint", ok=False, sealed=True, codes=sorted({i["code"] for i in issues}))
            _fail_lint(issues, sealed=True)
        _mark(draft, st, "lint", ok=True, sealed=True)
    brief = template.brief(payload)
    manifest = repo / st["manifest"]
    # ④ 게이트 → (footer 에 묶을 계측 산출물이 있는가) → 커밋(합성 신원 · 주입 시각 · CAS)
    _gate(repo, manifest, "continue")
    # footer certificate_ref 는 커밋 **전에** 이미 정해져 있다(바인딩은 publish 의 발행기 구동이 한다 · set-promotion-target 은
    #   바꾸지 않는다). 커밋 뒤에야 알면 hint 브랜치(되감지 않는다)에 태그 없는 페이로드 커밋이 남는다(2026-09-22 리뷰).
    _require_binding_artifact(_manifest_doc(repo, st), repo, before_commit=True)
    if commit_step is not None:
        anchor = commit_step["anchor"]
    else:
        _finalize_provenance(repo, payload, st)
        st["commit_utc"] = utc
        _save_state(draft, st)                     # 시각을 **커밋 전에** 적는다 — 커밋 직후 끊겨도 재실행이 같은 커밋으로 재개한다
        message = branch.commit_message(payload, generated_utc=utc)
        anchor = branch.commit_payload(repo, payload, message=message, generated_utc=utc)
        _mark(draft, st, "commit", anchor=anchor, utc=utc)
    # ⑤ 승격 목표(X6 · 발행 기록 → finalize 방출) → 게이트 재확인
    if _step(st, "target") is None:
        if st.get("mode") != "campaign" and not (draft / "inputs" / "pii.json").is_file():
            sel = st.get("selectors") or {}
            ev = evidence.from_publication(repo, sel.get("publication") or topic, sel.get("node"), docker=rt.docker)
            evidence.publisher_inputs(repo, ev, draft_dir=draft)
        evidence.set_promotion_target(repo, topic=topic, tag=tagname, topology=label, anchor=anchor,
                                      generated_utc=utc, draft_dir=draft)
        _mark(draft, st, "target", anchor=anchor)
    man = _manifest_doc(repo, st)
    probs = evidence.promotion_binding_problems(man, tag=tagname, topology=label, anchor=anchor)
    if probs:
        core.fail("HINT_PROMOTION_BINDING_MISMATCH", "work-manifest 의 promotion_target 이 봉인 대상과 다르다: "
                  + "; ".join(f"{c}: {m}" for c, m in probs),
                  "evidence_publisher set-promotion-target 이 쓴 목표를 확인한다(hint.py 는 manifest 를 직접 고치지 않는다).")
    _gate(repo, manifest, "continue")
    cert_ref = _require_binding_artifact(man, repo, before_commit=False)
    # ⑥ 봉인 → 이 태그 1개 로컬 검증(서사 린트 포함 — 봉인된 오브젝트에서)
    fields = {"version": tag.FOOTER_VERSION, "tag": tagname, "topology": label, "anchor": anchor,
              "manifest_ref": st["manifest"], "certificate_ref": cert_ref}
    message = tag.annotation(brief, fields)
    try:
        obj = tag.seal(repo, tagname, anchor, message, generated_utc=utc)
    except tag.HintProblemsError as e:
        _mark(draft, st, "seal", ok=False, codes=tag.problem_codes(e.problems))
        raise
    lint_fn = _lint_fn(draft, st)
    vprobs = tag.verify_local(repo, tagname, lint_fn=lint_fn)
    if vprobs:
        codes = tag.problem_codes(vprobs)
        _mark(draft, st, "verify", ok=False, codes=codes)
        core.fail("HINT_LOCAL_VERIFY_FAILED", f"봉인한 태그의 로컬 검증 {len(vprobs)}건({', '.join(codes)}):\n  "
                  + "\n  ".join(vprobs[:30]), "각 code 의 자리를 고친다 — 검증되지 않은 태그는 밀지 않는다(D10).")
    _mark(draft, st, "verify", ok=True, tag_object=obj)
    result = {"tag": tagname, "anchor": anchor, "tag_object": obj, "manifest": st["manifest"], "pushed": False,
              "draft": draft}
    if no_push:
        # 이어가기는 continue 재실행이다 — 단독 `push` 는 태그만 밀고 카탈로그 파생·캠페인 publish 위상을 하지 않는다(침묵 누락 ✗).
        again = f"hint.py continue --draft {_show(repo, draft)} --generated-utc <UTC>"
        if st.get("mode") != "campaign":
            again += ' --approved-by "<같은 사람 발화 전사>" --approved-utc <UTC>'     # 캠페인 밖은 매 실행이 승인을 먼저 본다
        _log(f"--no-push — 봉인·로컬 검증까지. 이어서 `{again}` 를 다시 실행하면 push → 원격 SHA 대조 → 카탈로그 → "
             "(캠페인 셀) publish 위상까지 잇는다.")
        return result
    # ⑦ push(정확한 refspec 1개) → 원격 SHA == 로컬
    _gate(repo, manifest, "push")
    try:
        pres = tag.push_tag(repo, remote, tagname, lint_fn=lint_fn)
    except tag.HintProblemsError as e:
        _mark(draft, st, "push", ok=False, remote=remote, codes=tag.problem_codes(e.problems))
        raise
    local_obj = tag.read_tag(repo, tagname)["object_sha"]
    remote_obj = tag.remote_tag_object(repo, remote, tagname)
    if remote_obj != local_obj:
        core.fail("HINT_REMOTE_SHA_MISMATCH", f"원격 {remote} 의 태그 오브젝트 {str(remote_obj)[:12]} ≠ 로컬 {local_obj[:12]}")
    # 시각(주입만): 커밋·봉인은 첫 커밋 시각(재개 정합)이지만 push 가 **일어난** 시각은 그 push 를 한 호출의 주입 시각이다 —
    #   재실행이 이미 원격에 있는 태그를 확인만 했으면 앞선 push 기록의 시각을 유지한다(재실행 시각으로 덮지 않는다). 카탈로그는
    #   이 호출이 지금 다시 파생하므로 이 호출의 시각이다(2026-09-22 리뷰: 첫 커밋 시각으로 적으면 진행표·카탈로그가 거짓 시각).
    prev_push = _step(st, "push") or {}
    push_utc = (prev_push["utc"] if pres.get("status") == "already-on-remote" and prev_push.get("ok") and prev_push.get("utc")
                else utc_arg)
    _mark(draft, st, "push", ok=True, remote=remote, status=pres.get("status"), refspec=pres.get("refspec"),
          remote_object=remote_obj, utc=push_utc)
    # ⑧ 카탈로그(원격 발행 태그에서 파생 · 부재는 행으로 · D-i) ⑨ 캠페인 publish 위상
    catalog.derive(repo, remote, core.kst_iso(utc_arg), record_missing=True)
    _mark(draft, st, "catalog", ok=True, remote=remote, generated_kst=core.kst_iso(utc_arg))
    if st.get("mode") == "campaign":
        sel = st.get("selectors") or {}
        evidence.phase_set_publish(repo, sel.get("campaign"), sel.get("cell"), sel.get("node"), tagname, remote_obj,
                                   push_utc)
        _mark(draft, st, "phase", ok=True, proof_source=f"refs/tags/{tagname}@{remote_obj}")
    _mark(draft, st, "done", ok=True, remote_object=remote_obj)
    result.update(pushed=True, remote=remote, remote_object=remote_obj, push_status=pres.get("status"))
    return result


def cmd_continue(a) -> int:
    repo = core.resolve_repo(a.repo)
    draft = _find_draft(repo, draft=a.draft, campaign=a.campaign, cell=a.cell, node=a.node)
    res = continue_(repo, draft=draft, generated_utc=a.generated_utc, approved_by=a.approved_by,
                    approved_utc=a.approved_utc, remote=a.remote, no_push=a.no_push, factcheck_waiver=a.factcheck_waiver)
    print(f"{_OUT} continue — {res['tag']}")
    print(f"  페이로드 커밋 {res['anchor']} · 태그 오브젝트 {res['tag_object']}")
    if res["pushed"]:
        print(f"  push {res['remote']} ({res['push_status']}) · 원격 오브젝트 = 로컬 {res['remote_object']}")
        print("  카탈로그 파생 완료(record-missing) · 캠페인 publish 위상 기록(캠페인 셀이면)")
    else:
        print("  push 생략(--no-push) — 봉인·로컬 검증까지 끝났다")
    return 0


# ── lint · excerpt ─────────────────────────────────────────────────────────────────────────────
def lint_draft(repo: Path, draft: Path) -> list[dict]:
    """부수효과 0 — draft 페이로드를 임시 사본에서 refresh 한 뒤 lint(continue 가 볼 그 판정). draft 는 바꾸지 않는다."""
    st = _load_state(draft)
    if not (draft / "payload").is_dir():
        core.fail("HINT_DRAFT_PAYLOAD_ABSENT", f"draft 에 payload/ 가 없다: {draft}",
                  "hint.py publish 가 만든 draft 에서 실행한다(끊긴 publish 면 draft 를 지우고 같은 --generated-utc 로 다시).")
    with tempfile.TemporaryDirectory(prefix="hint-lint-") as td:
        copy = Path(td) / "payload"
        shutil.copytree(draft / "payload", copy)
        template.refresh(copy)
        return _lint(repo, copy, draft, st, sealed=False)


def cmd_lint(a) -> int:
    repo = core.resolve_repo(a.repo)
    draft = Path(a.draft).resolve()
    issues = lint_draft(repo, draft)
    if a.json:
        print(json.dumps(issues, ensure_ascii=False, indent=2))
    else:
        for i in issues:
            print(f"{i['code']}  {i['file']}:{i['line']}  {i['message']}")
        counts: dict[str, int] = {}
        for i in issues:
            counts[i["code"]] = counts.get(i["code"], 0) + 1
        print(f"{_OUT} lint {'통과(0건)' if not issues else f'{len(issues)}건'}"
              + (" — " + " · ".join(f"{c}×{n}" for c, n in sorted(counts.items())) if counts else "")
              + " (파생 블록은 continue 가 refresh 한다 — 여기서는 사본에서 미리 반영해 판정했다 · 보려면 `refresh`)")
        fc, err = _factcheck_eval(draft, _load_state(draft))
        print(f"{_OUT} 사실 검증 보고 {fc['status']}" + (f" — continue 는 {err.code} 로 멈춘다" if err else " — continue 게이트 통과")
              + " (린트와 별개 · 저작자가 아닌 Agent 의 inputs/factcheck.json)")
    return 0 if not issues else 1


def refresh_draft(repo: Path, draft: Path) -> list[str]:
    """draft 페이로드의 파생 블록(00 §0.3 wall_map · §0.5 knob_status)을 현재 hint-event 로 다시 쓴다(draft 안 쓰기만 · 2026-09-22
    S2 round 2 F10). 1차 저작자가 "생성된 요약을 볼 수 없어 0.5 산문이 요약과 맞는지 확인할 수 없다" 고 적었다 — lint 는 계속 사본을
    refresh 해 판정한다(이 명령은 미리 보기다). 커밋 뒤에는 거부한다(커밋한 바이트와 draft 가 갈라지면 재실행이 막힌다)."""
    st = _load_state(draft)
    if _step(st, "commit") is not None:
        core.fail("HINT_DRAFT_ALREADY_COMMITTED", f"이미 hint 브랜치에 커밋한 draft 다({_step(st, 'commit').get('anchor', '')[:12]}) — "
                                                  "파생 블록을 다시 쓰면 커밋한 바이트와 갈라진다.",
                  "커밋 뒤 개정은 새 publish(새 이름)다 — continue 재실행은 커밋한 내용 그대로 잇는다.")
    if not (draft / "payload").is_dir():
        core.fail("HINT_DRAFT_PAYLOAD_ABSENT", f"draft 에 payload/ 가 없다: {draft}")
    return template.refresh(draft / "payload")


def cmd_refresh(a) -> int:
    repo = core.resolve_repo(a.repo)
    draft = _find_draft(repo, draft=a.draft, campaign=None, cell=None, node=None)
    changed = refresh_draft(repo, draft)
    print(f"{_OUT} refresh — {'다시 쓴 파생 블록 ' + ', '.join(changed) if changed else '바뀐 파생 블록 없음(이미 최신)'} "
          f"· {_show(repo, draft / 'payload' / '00-hint.md')} §0.3 · §0.5 에서 확인한다")
    return 0


def excerpt_text(repo: Path, draft: Path, source: str, lines: str | None = None, *, numbered: bool = False) -> str:
    """발췌 저작 도우미: 린터가 대조하는 **치환 후 원문**(template.substituted_source · 같은 치환표 · 같은 해소기).
    `numbered=True` 면 줄마다 `L<n>\t` 를 붙인다 — n 은 린터가 `§L<a>-<b>` 를 대조하는 **같은** 번호(정규화 · 치환 뒤 `\n` 분할 ·
    1-기반 · `--lines a-b` 와 함께면 a 부터). 2026-09-22 · plan_26092119 S2 round 3: 2차 저작자가 §L 번호를 얻으려 원본을 따로 `grep -n`
    했고, 파이썬 유니버설 개행으로 읽으면 `\r` 가 줄이 되어 번호가 어긋났다 — 번호의 원천을 린터와 한 자리로 둔다."""
    st = _load_state(draft)
    payload = draft / "payload"
    lin = core.read_json(payload / "LINEAGE.json", code="HINT_LINEAGE_UNREADABLE")
    text = template.substituted_source(repo, payload, source, lineage=lin, subst_table=_subst_table(repo, st), draft_dir=draft)
    rows = text.split("\n")
    first = 1
    if lines:
        m = re.fullmatch(r"\s*(\d+)\s*-\s*(\d+)\s*", lines)
        if not m or int(m.group(1)) < 1 or int(m.group(2)) < int(m.group(1)):
            core.fail("HINT_EXCERPT_LINES_SHAPE", f"--lines 는 `a-b`(1 ≤ a ≤ b)다: {lines!r}")
        if int(m.group(2)) > len(rows):
            # 범위 밖을 빈 출력으로 돌려주면 저작자는 "빈 줄" 로 읽는다 — 린터(HINT_EXCERPT_LINES_OUT_OF_RANGE)와 같은 판정으로 멈춘다
            #   (2026-09-22 S2 round 2 적대 리뷰).
            core.fail("HINT_EXCERPT_LINES_OUT_OF_RANGE", f"--lines {lines} 가 출처(정규화 뒤 {len(rows)}줄)의 범위 밖이다")
        first = int(m.group(1))
        rows = rows[first - 1:int(m.group(2))]
    if numbered:
        if not lines and rows and rows[-1] == "":
            rows = rows[:-1]                 # 끝 개행 뒤의 빈 조각은 줄이 아니다(grep -n 과 같은 줄 수로 보인다)
        return "\n".join(f"L{first + i}\t{row}" for i, row in enumerate(rows))
    return "\n".join(rows)


def cmd_excerpt(a) -> int:
    repo = core.resolve_repo(a.repo)
    sys.stdout.write(excerpt_text(repo, Path(a.draft).resolve(), a.source, a.lines, numbered=a.numbered).rstrip("\n") + "\n")
    return 0


# ── name ───────────────────────────────────────────────────────────────────────────────────────
def cmd_name(a) -> int:
    repo = core.resolve_repo(a.repo)
    _check_selectors(campaign=a.campaign, cell=a.cell, publication=a.publication, replay=False, require_replay=False)
    ev = _evidence(repo, campaign=a.campaign, cell=a.cell, publication=a.publication, node=a.node, rt=Runtime())
    dn = naming.derive_name(evidence.naming_facts(repo, ev), naming.load_vocab(repo))
    doc = {"tag": dn.tag, **dn.to_payload()}
    if a.json:
        print(json.dumps(doc, ensure_ascii=False, indent=2, sort_keys=True))
        return 0
    print(dn.tag)
    for k in ("vllm", "model", "arch", "recipe"):
        s = dn.segments[k]
        print(f"  세그먼트 {k:<7} {s.value:<40} ← {s.source}")
    for k in naming.ARCH_AXES + (naming.PLANE_AXIS,) + naming.RECIPE_AXES:
        x = dn.axes[k]
        print(f"  축 {k:<13} {x.value:<40} ← {x.source}")
    print(f"  빌드 입력 {json.dumps(dn.vllm_build_input, ensure_ascii=False, sort_keys=True)}")
    return 0


# ── verify · push ──────────────────────────────────────────────────────────────────────────────
def _footer_of(repo: Path, tagname: str) -> dict | None:
    obj = tag.read_tag(repo, tagname)
    if obj is None:
        return None
    try:
        return tag.parse_annotation(obj["body"])["footer"]
    except core.HintError:
        try:
            return tag.parse_footer(obj["body"])
        except core.HintError:
            return None


def _draft_state_for(repo: Path, tagname: str, draft_arg: str | None) -> tuple[Path | None, dict | None]:
    d = Path(draft_arg).resolve() if draft_arg else _find_draft_for_tag(repo, tagname)
    if d is None:
        return None, None
    st = _load_state(d)
    if st.get("tag") != tagname:
        core.fail("HINT_DRAFT_TAG_MISMATCH", f"draft 의 태그 {st.get('tag')!r} ≠ {tagname!r}")
    return d, st


def verify_tag(repo: Path, tagname: str, *, draft: str | None = None) -> list[str]:
    """이 태그 1개(D10). footer 의 manifest 로 hint_verify 게이트를 묻고(읽기 전용) verify_local(서사 린트 주입)."""
    tag.check_ref_format(repo, tagname)
    footer = _footer_of(repo, tagname)
    if footer is None:
        return tag.verify_local(repo, tagname)        # 태그·footer 부재 — 게이트를 물을 manifest 가 없다(결함 목록만)
    _gate(repo, footer["manifest_ref"], "verify")
    d, st = _draft_state_for(repo, tagname, draft)
    return tag.verify_local(repo, tagname, lint_fn=_lint_fn(d, st))


def cmd_verify(a) -> int:
    repo = core.resolve_repo(a.repo)
    probs = verify_tag(repo, a.tag, draft=a.draft)
    if probs:
        print(f"{_OUT} verify FAIL {a.tag} — {len(probs)}건 · codes {', '.join(tag.problem_codes(probs))}")
        for p in probs:
            print(f"  {p}")
        return 1
    print(f"{_OUT} verify PASS {a.tag}(이 태그 1개 · 로컬 봉인 · 서사 린트 포함)")
    return 0


def cmd_push(a) -> int:
    repo = core.resolve_repo(a.repo)
    _require_central(repo, "push")          # D8 — 옛 cmd_push 첫 줄과 같은 자리(dry-run 도 배포 리허설이라 권위 체크아웃 전용)
    tag.check_ref_format(repo, a.tag)
    footer = _footer_of(repo, a.tag)
    if footer is None:
        probs = tag.verify_local(repo, a.tag)
        core.fail("HINT_PUSH_UNVERIFIED", f"footer 를 읽을 수 없는 태그는 밀지 않는다: {probs[:5]}")
    _gate(repo, footer["manifest_ref"], "push")
    d, st = _draft_state_for(repo, a.tag, a.draft)
    res = tag.push_tag(repo, a.remote, a.tag, dry_run=not a.apply, lint_fn=_lint_fn(d, st))
    print(json.dumps(res, ensure_ascii=False, indent=2, sort_keys=True))
    if a.apply:
        _log(f"단독 push — 카탈로그는 `hint.py catalog derive --remote {a.remote} --generated-kst <KST> --record-missing` 로 "
             "파생한다(continue 는 스스로 한다).")
        if st is not None and st.get("mode") == "campaign":
            # 단독 push 는 캠페인 publish 위상을 쓰지 않는다 — 진행표가 '발행 안 됨' 으로 남는 침묵 누락을 말로 드러낸다.
            _log(f"⚠ 캠페인 셀 draft({_show(repo, d)}) — publish 위상은 기록되지 않았다. `hint.py continue --draft "
                 f"{_show(repo, d)} --generated-utc <UTC>` 를 실행하면 원격 확인(이미 반영 · 멱등) → 카탈로그 → 위상까지 잇는다.")
    return 0


# ── catalog · match ────────────────────────────────────────────────────────────────────────────
def cmd_catalog_derive(a) -> int:
    repo = core.resolve_repo(a.repo)
    return catalog.derive(repo, a.remote, a.generated_kst, dry_run=a.dry_run, record_missing=a.record_missing,
                          allow_empty=a.allow_empty)


def cmd_match(a) -> int:
    repo = core.resolve_repo(a.repo, require_git=False)       # gitless 배포본에서도 돈다(hints/index.json 만)
    idx = catalog.load_index(repo)
    res = catalog.match(idx, vllm=a.vllm, model=a.model, arch=a.arch, include_other=a.include_other)
    if a.json:
        print(json.dumps(res, ensure_ascii=False, indent=2))
    else:
        print(catalog.format_match(idx, res, model=a.model, include_other=a.include_other))
    return 0


def cmd_branch_transition(a) -> int:
    """유일한 maintenance-only branch transition 진입점. 태그·catalog·code worktree를 건드리지 않는다."""
    repo = core.resolve_repo(a.repo)
    print(json.dumps(branch.branch_transition(repo, remote=a.remote, generated_utc=a.generated_utc),
                     ensure_ascii=False, indent=2, sort_keys=True))
    return 0


# ── 파서 · main ───────────────────────────────────────────────────────────────────────────────
def _build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="hint.py", description="hint-publisher 단일 CLI — 셀 1개 = 태그 1개(plan_26092119).")
    p.add_argument("--repo", help="저장소 루트(기본: 현재 디렉터리의 git 최상위) — 서브명령 앞에 둔다")
    p.add_argument("--self-test", action="store_true", help="전 모듈 자체검사 + 파서 검사 + 격리 E2E(라이브 비의존)")
    sub = p.add_subparsers(dest="cmd", metavar="<command>")

    s = sub.add_parser("publish", help="셀 1개 → 스캐폴드 → 정지(태그·브랜치 부수효과 0)")
    s.add_argument("--campaign", help="캠페인 id(명시 · ACTIVE 를 읽지 않는다)")
    s.add_argument("--cell", help="셀 id")
    s.add_argument("--publication", help="발행 기록 토픽(docs/_evidence/<topic>.json) — --replay 와 함께")
    s.add_argument("--replay", action="store_true", help="발행 기록 재생(읽기 전용 · draft 밖 쓰기 0)")
    s.add_argument("--node", choices=evidence.NODE_AXIS, help="측정 노드 축(기본: 인증서 measured_node 에서 해소)")
    s.add_argument("--generated-utc", required=True, help="주입 UTC YYYY-MM-DDTHH:MM:SSZ(벽시계 ✗)")
    s.add_argument("--lineage-add", action="append", default=[], metavar="PATH=REASON",
                   help="간선이 끊긴 계보 문서를 사람이 보충한다(source: declared · 사유 필수)")
    s.add_argument("--out", help="draft 디렉터리(기본 hints/.drafts/<draft_id>/)")
    s.add_argument("--remote", default="origin", help="이름 충돌을 확인할 원격(ls-remote 읽기)")
    s.add_argument("--no-remote-check", action="store_true", help="원격 이름 충돌 조회를 건너뛴다(로컬만 · 오프라인)")
    dk = s.add_mutually_exclusive_group()
    dk.add_argument("--docker-read-only", action="store_true",
                    help="이미지 탐침을 끈다 — docker 는 image inspect·history 만(시작하지 않는 컨테이너의 create·cp·rm 도 ✗ · 이미지 안 "
                         "원장 · 패치 사본 관측 실패로 남긴다)")
    dk.add_argument("--docker-probe", action="store_true",
                    help="옛 이름(no-op) — 이미지 탐침은 기본이다: docker 는 image inspect·history + 시작하지 않는 컨테이너의 "
                         "create(--pull never)·cp·rm 만 — 이미지 파일 탐침 ✓ · 이미지 원장도 같은 create+cp 로 읽는다 · run/start ✗")
    s.set_defaults(fn=cmd_publish)

    c = sub.add_parser("continue", help="승인 → 린트 → 커밋 → 봉인 → 검증 → push → 카탈로그")
    c.add_argument("--campaign")
    c.add_argument("--cell")
    c.add_argument("--draft", help="publish 가 만든 draft 디렉터리")
    c.add_argument("--node", choices=evidence.NODE_AXIS, help="--campaign/--cell 로 draft 를 찾을 때 노드 축으로 좁힌다")
    c.add_argument("--generated-utc", required=True, help="주입 UTC(첫 커밋 뒤 재실행은 state.json 의 커밋 시각을 쓴다)")
    c.add_argument("--approved-by", help="캠페인 밖 발행: 사람 발화 전사(한 줄)")
    c.add_argument("--approved-utc", help="캠페인 밖 발행: 승인 시각 UTC")
    c.add_argument("--remote", default="origin", help="push·원격 SHA 대조·카탈로그 파생 원격")
    c.add_argument("--no-push", action="store_true", help="봉인·로컬 검증까지만")
    c.add_argument("--factcheck-waiver", help="발행 전 독립 사실 검증을 사람이 면제했다: 그 발화 전사(한 줄 · state·00 메타에 남는다)")
    c.set_defaults(fn=cmd_continue)

    li = sub.add_parser("lint", help="draft 린트(부수효과 0)")
    li.add_argument("--draft", required=True)
    li.add_argument("--json", action="store_true")
    li.set_defaults(fn=cmd_lint)

    rf = sub.add_parser("refresh", help="draft 안 파생 블록(00 §0.3 · §0.5)을 다시 쓴다(미리 보기 · 태그·브랜치 부수효과 0)")
    rf.add_argument("--draft", required=True)
    rf.set_defaults(fn=cmd_refresh)

    ex = sub.add_parser("excerpt", help="발췌 저작 도우미 — 치환 후 원문 출력(부수효과 0)")
    ex.add_argument("--draft", required=True)
    ex.add_argument("--source", required=True, help="LINEAGE 문서 stem · 저장소 상대 경로 · artifacts 파일명")
    ex.add_argument("--lines", help="a-b (1-기반 · 포함)")
    ex.add_argument("--numbered", action="store_true",
                    help="줄마다 정규화 뒤 번호를 `L<n>` 으로 붙인다 — 비-마크다운 발췌 머리 `§L<a>-<b>` 의 번호(발췌 본문에는 번호를 옮기지 않는다)")
    ex.set_defaults(fn=cmd_excerpt)

    n = sub.add_parser("name", help="파생 이름 · 축별 출처(읽기 전용)")
    n.add_argument("--campaign")
    n.add_argument("--cell")
    n.add_argument("--publication")
    n.add_argument("--node", choices=evidence.NODE_AXIS)
    n.add_argument("--json", action="store_true")
    n.set_defaults(fn=cmd_name)

    v = sub.add_parser("verify", help="이 태그 1개 · push 전 로컬 봉인 검증(D10)")
    v.add_argument("--tag", required=True)
    v.add_argument("--draft", help="그 태그를 봉인한 draft(기본: hints/.drafts 에서 태그로 찾는다)")
    v.set_defaults(fn=cmd_verify)

    pu = sub.add_parser("push", help="태그 1개 push(정확한 refspec · 기본 dry-run)")
    pu.add_argument("--tag", required=True)
    pu.add_argument("--remote", default="origin")
    pu.add_argument("--apply", action="store_true", help="실제 push(없으면 dry-run — 실패를 삼키지 않는다)")
    pu.add_argument("--draft")
    pu.set_defaults(fn=cmd_push)

    ca = sub.add_parser("catalog", help="원격 발행 태그 → hints/index.json + HINTS.md")
    csub = ca.add_subparsers(dest="catalog_cmd", metavar="<catalog-command>", required=True)
    d = csub.add_parser("derive")
    d.add_argument("--remote", required=True, help="진실원천 원격(ls-remote)")
    d.add_argument("--generated-kst", required=True, help="주입 KST YYYY-MM-DDTHH:MM:SS")
    d.add_argument("--dry-run", action="store_true")
    d.add_argument("--record-missing", action="store_true",
                   help="로컬 오브젝트 없는 원격 태그를 `absent-local` 행으로 싣고 rc 0(X15 · continue·S5 재생성)")
    d.add_argument("--allow-empty", action="store_true", help="원격 hint 태그 0건을 정상으로 선언")
    d.set_defaults(fn=cmd_catalog_derive)

    m = sub.add_parser("match", help="수신자 근-미스 발견(읽기 전용 · gitless)")
    m.add_argument("--vllm", required=True)
    m.add_argument("--model", required=True)
    m.add_argument("--arch")
    m.add_argument("--include-other", action="store_true")
    m.add_argument("--json", action="store_true")
    m.set_defaults(fn=cmd_match)

    bt = sub.add_parser("branch-transition", help="maintenance-only: README 하나의 hint 브랜치 형식 전환(CAS·정확한 branch push)")
    bt.add_argument("--remote", required=True, help="live hint tip을 대조하고 refs/heads/hint 하나만 밀 원격")
    bt.add_argument("--generated-utc", required=True, help="주입 UTC YYYY-MM-DDTHH:MM:SSZ(벽시계 ✗)")
    bt.set_defaults(fn=cmd_branch_transition)
    return p


def main(argv=None) -> int:
    p = _build_parser()
    a = p.parse_args(argv)
    if a.self_test:
        return self_test()
    if not getattr(a, "cmd", None):
        p.print_help(sys.stderr)
        return 2
    try:
        return int(a.fn(a) or 0)
    except core.HintError as e:
        if e.code == "HINT_GATE_DENIED":
            # 게이트 거부 = 게이트 JSON 원문을 stdout 에 그대로(옛 계약 · 소비자가 json.loads 한다) + 그 종료코드. 부수효과 0.
            sys.stdout.write(e.message if e.message.endswith("\n") else e.message + "\n")
            _log(f"승격 게이트 거부 — {e.remedy or ''}")
            return e.exit_code or 1
        if getattr(e, "gate_json", None):
            # 게이트 실행 불가·출력 판독 불가 = 결정론 포장 JSON(stdout) + 사유(stderr) · exit 2(옛 계약 그대로).
            sys.stdout.write(e.gate_json)
            print(e.render(), file=sys.stderr)
            return e.exit_code or 2
        print(e.render(), file=sys.stderr)
        if isinstance(e, tag.HintProblemsError):
            _log(f"하위 code: {', '.join(tag.problem_codes(e.problems))}")
        return e.exit_code or 1


# ══ 자체검사 ═══════════════════════════════════════════════════════════════════════════════════
# 픽스처 문자열은 조각으로 조립한다(추적 파일에 IPv4 모양 · 운영자 경로 모양 리터럴을 쓰지 않는다 — 2026-08-06 자기스캔 사고).
_MODULES = ("core", "naming", "pii", "branch", "tag", "catalog", "lineage", "artifacts", "template", "evidence")
_FX_PUBLISH_UTC = "2026-01-02T06:00:00Z"
_FX_REPLAY_UTC = "2026-01-02T06:30:00Z"
_FX_CONTINUE_UTC = "2026-01-02T07:00:00Z"
_FX_ENV_MTIME_EPOCH = 1767225600          # 2026-01-01T00:00:00Z — 픽스처 측정(2026-01-02) 앞(env 형상 mtime 고정 · 벽시계 ✗)
_FX_TESTLOG = "docs/testlog/testlog_26010213_fixture.md"
_FX_WALL_SIG = "ValueError: fixture backend is disabled for this configuration."
_FX_MISDIAG_SIG = "당시 판정: hang 은 모델 결함이다"
# 2026-09-22 S2 round 2(F8): ANSI 색 · tqdm `\r` 진행 조각 · CRLF · NIC 장치 이름이 섞인 엔진 로그(스윕 디렉터리 → LINEAGE engine_log 후보).
#   파일 이름은 evidence 의 기동 표지 원천(lite_engine_* · level_*/engine_*)과 겹치지 않게 한다(시각 줄도 두지 않는다).
_FX_LOG_REL = "output/multi/benchlog/sweep_c1-a/master_c1-a.log"
_FX_NIC_IFACE, _FX_NIC_HCA = "enpfx7s0", "rocefx7p1"
_FX_LOG = ("INFO start\n\x1b[36m(RayWorkerProc pid=7)\x1b[0m worker ready\r\nLoading 10%\rLoading 50%\rLoading 100%\n"
           f"NCCL INFO NET/IB : Using [0]{_FX_NIC_HCA}:1/RoCE ; OOB {_FX_NIC_IFACE}\n"
           "AttributeError: 'tuple' object has no attribute 'start'\n")


def _selftest_modules() -> list[str]:
    import importlib
    bad = []
    for name in _MODULES:
        mod = importlib.import_module(f"hintlib.{name}")
        try:
            with contextlib.redirect_stderr(io.StringIO()):
                res = mod.selftest()
        except Exception as e:  # noqa: BLE001 — 자체검사 사고도 실패로 센다(통과로 접지 않는다)
            res = [f"{name}: selftest 가 예외로 죽었다 {type(e).__name__}: {e}"]
        print(f"  {'ok  ' if not res else 'FAIL'} hintlib.{name}.selftest ({len(res)} 실패)")
        bad += res
    return bad


def _selftest_parser(ck) -> None:
    import inspect
    p = _build_parser()
    cases = {
        "publish": ["publish", "--campaign", "c", "--cell", "x", "--generated-utc", _FX_PUBLISH_UTC],
        "continue": ["continue", "--draft", "d", "--generated-utc", _FX_CONTINUE_UTC],
        "lint": ["lint", "--draft", "d"],
        "refresh": ["refresh", "--draft", "d"],
        "excerpt": ["excerpt", "--draft", "d", "--source", "s", "--lines", "1-2"],
        "name": ["name", "--publication", "t"],
        "verify": ["verify", "--tag", "hint/a/b/c/d"],
        "push": ["push", "--tag", "hint/a/b/c/d"],
        "catalog": ["catalog", "derive", "--remote", "r", "--generated-kst", "2026-01-02T15:00:00", "--record-missing"],
        "match": ["match", "--vllm", "0.9.0", "--model", "m"],
        "branch-transition": ["branch-transition", "--remote", "r", "--generated-utc", _FX_CONTINUE_UTC],
    }
    for name, argv in cases.items():
        try:
            a = p.parse_args(["--repo", "/nonexistent", *argv])
            ok = a.cmd == name and callable(getattr(a, "fn", None)) and a.repo == "/nonexistent"
        except SystemExit:
            ok = False
        ck(f"파서: `{name}` 최소 인자 · 전역 --repo 는 서브명령 앞", ok)
    ck("파서: continue·push 의 --remote 기본값 origin",
       p.parse_args(cases["continue"]).remote == "origin" and p.parse_args(cases["push"]).remote == "origin")
    ck("파서: _build_parser 소스에 `default=\"origin\"` 리터럴(정책 LAST_GOOD C3)",
       'default="origin"' in inspect.getsource(_build_parser))
    ck("게이트 매핑 = continue·verify·push 3종(match·catalog·publish 무게이트 · C6)",
       HINT_ACTION_FOR_CMD == {"continue": "hint_finalize", "verify": "hint_verify", "push": "hint_push"})
    pr = _docker_probe_only()
    ck("탐침 실행기만 image_probe 능력을 선언한다(artifacts.PROBE_ATTR · --docker-read-only 는 탐침 ✗)",
       getattr(pr, artifacts.PROBE_ATTR, False) is True and getattr(_docker_read_only(), artifacts.PROBE_ATTR, False) is False)
    # 2026-09-22 · plan_26092119 S2 round 3: 이미지 탐침 = 기본 · `--docker-read-only` 만 끈다 · `--docker-probe` = 옛 이름(no-op).
    #   행동으로 대조한다(옛 판은 cmd_publish 소스 문자열을 grep 했다 — 배선을 옮기면 공허해진다).
    runners = {tuple(fl): _publish_runner(p.parse_args(cases["publish"] + list(fl)))
               for fl in ((), ("--docker-probe",), ("--docker-read-only",))}
    ck("★publish 실행기: 플래그 없음 = 탐침(image_probe) · --docker-probe = 같은 탐침(no-op 별칭) · --docker-read-only = 탐침 ✗ · None ✗",
       all(r is not None for r in runners.values())
       and getattr(runners[()], artifacts.PROBE_ATTR, False) is True
       and getattr(runners[("--docker-probe",)], artifacts.PROBE_ATTR, False) is True
       and getattr(runners[("--docker-read-only",)], artifacts.PROBE_ATTR, False) is False)
    ck("★기본 실행기도 컨테이너를 시작하지 않는다(run · start 거부 rc 125)", all(
        runners[()](["docker", c, "x"]).returncode == 125 for c in ("run", "start")))
    ck("★--docker-probe: run · start · exec · build 는 실행하지 않고 rc 125", all(
        pr(["docker", c, "x"]).returncode == 125 for c in ("run", "start", "exec", "build", "pull", "compose")))
    ck("★--docker-probe: `--pull never` 없는 create 거부 · 만들지 않은 컨테이너의 cp · rm 거부(rc 125 · 실행 0)",
       pr(["docker", "create", "img"]).returncode == 125
       and pr(["docker", "cp", "-L", "deadbeefdeadbeef:/x", "/tmp/y"]).returncode == 125
       and pr(["docker", "rm", "-f", "deadbeefdeadbeef"]).returncode == 125)
    # 2026-09-22 round 3 적대 리뷰: 규칙의 소유자는 artifacts 실행기 하나(여기서 손으로 한 벌 더 적던 규칙은 `--network none` 을 빠뜨렸다)
    ck("★탐침 실행기 = artifacts 규칙 그대로: `--network none` 없는 create 도 거부(rc 125 · 실행 0)",
       pr(["docker", "create", "--pull", "never", "--entrypoint", "/bin/true", "img"]).returncode == 125
       and "hintlib.artifacts" in pr(["docker", "create", "img"]).stderr)
    # 2026-09-22 · plan_26092119 S2 round 2 통합: `--docker-probe` 도움말 · 실행기 docstring 이 artifacts 의 원장 읽기 경로와 같은 말을
    #   한다 — image_probe 실행기의 원장은 create+cp 로 읽힌다(artifacts `_observe_ledger` · 행동 검사는 artifacts 자체검사 F12 "run 0").
    #   옛 문구("원장 cat 은 관측 실패로 남는다")는 경로가 바뀐 뒤에도 남아 운영자에게 원장 관측 ② 가 없다고 말했다. 판정은 도움말
    #   원문 대조다(부정 표현 목록 · 닫힌 tripwire) — 음성대조는 옛 문구를 넣어 같은 술어가 RED 를 내는지 본다.
    def _probe_help_ok(text: str) -> bool:
        t = str(text or "")
        return "create+cp" in t and not any(x in t for x in ("원장 cat 은 관측 실패", "원장 cat 의 `docker run` 도 거부"))
    sub = next(x for x in p._actions if isinstance(x, argparse._SubParsersAction)).choices["publish"]
    probe_help = next((x.help for x in sub._actions if "--docker-probe" in x.option_strings), "")
    ck("--docker-probe 도움말 · 실행기 docstring = 원장도 create+cp 로 읽는다(옛 '원장 cat 관측 실패' 문구 ✗)",
       _probe_help_ok(probe_help) and _probe_help_ok(_docker_probe_only.__doc__))
    ck("★음성대조: 옛 도움말 문구('원장 cat 은 관측 실패로 남는다')는 같은 술어가 거부한다",
       not _probe_help_ok("docker 는 … create+cp · rm 만 — run/start ✗(원장 cat 은 관측 실패로 남는다)"))
    try:
        with contextlib.redirect_stderr(io.StringIO()):
            p.parse_args(["publish", "--campaign", "c", "--cell", "x", "--generated-utc", _FX_PUBLISH_UTC,
                          "--docker-read-only", "--docker-probe"])
        ck("★--docker-read-only 와 --docker-probe 는 함께 쓰지 않는다", False)
    except SystemExit:
        ck("★--docker-read-only 와 --docker-probe 는 함께 쓰지 않는다", True)
    try:
        with contextlib.redirect_stderr(io.StringIO()):
            p.parse_args(["catalog"])
        ck("★catalog 는 하위 명령 필수", False)
    except SystemExit:
        ck("★catalog 는 하위 명령 필수", True)
    # 핸들러가 읽는 속성 = 파서가 만든 속성(옵션 문자열을 두 번 적지 않는다 · 2026-09-07 `--payload` 배선 누락 선례)
    reads = {"publish": ("campaign", "cell", "publication", "replay", "node", "generated_utc", "lineage_add", "out", "remote",
                         "no_remote_check", "docker_read_only", "docker_probe"),
             "continue": ("draft", "campaign", "cell", "node", "generated_utc", "approved_by", "approved_utc", "remote",
                          "no_push", "factcheck_waiver"),
             "refresh": ("draft",),
             "push": ("tag", "remote", "apply", "draft"), "verify": ("tag", "draft"), "lint": ("draft", "json"),
             "excerpt": ("draft", "source", "lines", "numbered"), "name": ("campaign", "cell", "publication", "node", "json"),
             "catalog": ("remote", "generated_kst", "dry_run", "record_missing", "allow_empty"),
             "match": ("vllm", "model", "arch", "include_other", "json"),
             "branch-transition": ("remote", "generated_utc")}
    for name, attrs in reads.items():
        a = p.parse_args(cases[name])
        miss = [x for x in attrs if not hasattr(a, x)]
        ck(f"파서 배선: `{name}` 핸들러가 읽는 속성 전부 존재{miss or ''}", not miss)


# ── E2E 픽스처 ─────────────────────────────────────────────────────────────────────────────────
def _fx_git(repo: Path, *args: str, env: dict | None = None, check: bool = True) -> subprocess.CompletedProcess:
    e = dict(os.environ)
    mail = "@".join(["fixture", "example.invalid"])        # 메일 모양 리터럴을 추적 파일에 두지 않는다(배포 4종 자기스캔)
    e.update({"GIT_AUTHOR_NAME": "fixture", "GIT_AUTHOR_EMAIL": mail, "GIT_COMMITTER_NAME": "fixture",
              "GIT_COMMITTER_EMAIL": mail, "GIT_AUTHOR_DATE": "@1767225600 +0000",
              "GIT_COMMITTER_DATE": "@1767225600 +0000", "GIT_TERMINAL_PROMPT": "0"})
    e.update(env or {})
    r = subprocess.run(["git", "-c", "tag.gpgSign=false", "-c", "commit.gpgSign=false", "-c", "init.defaultBranch=multi-node",
                        *args], cwd=str(repo), env=e, capture_output=True, text=True)
    if check and r.returncode != 0:
        raise RuntimeError(f"fixture git {args[:3]} rc={r.returncode}: {r.stderr[-400:]}")
    return r


def _fx_docker(state: dict):
    """읽기 전용 docker 대역(evidence·artifacts 공용). `run`(원장 cat) = 원장 없는 옛 이미지(cat rc 1)."""
    def run(argv, **kw):
        state.setdefault("calls", []).append(list(argv))
        a = list(argv[1:])
        if a[:2] == ["image", "inspect"]:
            ref = a[-1]
            img = state["images"].get(ref)
            if img is None:
                return subprocess.CompletedProcess(argv, 1, "", "No such image")
            if "--format" in a:
                fmt = a[a.index("--format") + 1]
                val = {"{{.Id}}": img.get("Id"), "{{.Created}}": img.get("Created")}.get(fmt)
                return subprocess.CompletedProcess(argv, 0 if val else 1, f"{val}\n" if val else "", "")
            return subprocess.CompletedProcess(argv, 0, json.dumps([img]), "")
        if a[:1] == ["history"]:
            h = state["history"].get(a[-1])
            return subprocess.CompletedProcess(argv, 0 if h is not None else 1, h or "", "")
        if a[:1] == ["run"]:
            return subprocess.CompletedProcess(argv, 1, "", "cat: /opt/easy-vllm/build_ledger.json: No such file")
        return subprocess.CompletedProcess(argv, 1, "", "unexpected docker call")
    return run


def _fx_tree(root: Path) -> dict:
    out = {}
    for p in sorted(root.rglob("*")):
        if ".git" in p.relative_to(root).parts or p.name == "ACTIVE":
            continue
        if p.is_file() and not p.is_symlink():
            out[p.relative_to(root).as_posix()] = p.read_bytes()
    return out


def _fx_repo(td: Path) -> tuple[Path, dict]:
    """evidence 의 격리 캠페인 픽스처(c1 · 셀 c1-a · cluster · 소유 모듈 심링크) 위에 발행 전 경로가 도는 입력을 더한다:
    git 저장소 · 템플릿 · 명명 SSOT · 산출물(compose · 러너 · env 형상 · Dockerfile · 패치) · 계보 문서 · bare 원격 · 중앙 권위."""
    repo, fx = evidence._fixture(td)
    code_root = core.HINTLIB_DIR.parents[4]
    for rel in (core.REL_DOC_NAMING_WIKI, core.REL_DOC_NAMING_BENCH):
        dst = repo / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        os.symlink(code_root / rel, dst)
    shutil.copytree(core.TEMPLATES_DIR, repo / core.REL_SKILL / "templates")
    out = repo / "output/multi"
    models = td / "models"
    man = (out / "manifest.yaml").read_text(encoding="utf-8")
    (out / "manifest.yaml").write_text(man + f"quant_model_path: {models}\ninterconnect:\n  socket_iface: {_FX_NIC_IFACE}\n"
                                       f"  hca_devices: [{_FX_NIC_HCA}]\n", encoding="utf-8")
    (repo / _FX_LOG_REL).write_bytes(_FX_LOG.encode("utf-8"))      # 바이트 그대로(개행 변환 ✗ — \r·CRLF 를 지킨다)
    # 판정점 레벨 벤치 결과 = `vllm bench serve` 결과 모양(evidence.bench_command 가 필드에서 명령을 재구성한다 · 2026-09-22 S2 round 2).
    lvl = repo / "output/multi/benchlog/sweep_c1-a/level_01/bench_c1-a.json"
    core.write_json(lvl, {**json.loads(lvl.read_text(encoding="utf-8")), "backend": "openai", "endpoint": "/v1/completions",
                          "model_id": "/app/quant_models/Org/Fixture-Model-NVFP4", "num_prompts": 4, "max_concurrency": 1,
                          "total_input_tokens": 4096, "total_output_tokens": 1024})
    compose = ("x-gpu-common: &gpu-common\n  image: ${IMAGE_TAG:-fixture:0.9.0-source}\n  build:\n    context: .\n"
               "    dockerfile: ${BUILD_DOCKERFILE:-Dockerfile.source-build}\n    args:\n"
               "      VLLM_REPO: ${VLLM_REPO:-fixture-repo}\n      VLLM_REF: ${VLLM_REF:-v0.9.0}\n"
               "      SM12X_PORT: ${SM12X_PORT:-0}\n      BUILD_JOBS: ${BUILD_JOBS:-16}\n  volumes:\n"
               "    - ${QUANT_MODEL_PATH:-/srv/fixture-models}:/app/quant_models:ro\n    - ./configs:/app/configs:ro\n"
               "services:\n  vllm-master-serve:\n    <<: *gpu-common\n    profiles: [master]\n    environment:\n"
               "      NODE_ROLE: master\n      CONFIG_FILE: ${CONFIG_FILE:-default}\n    env_file:\n"
               "      - envs/.env.interconnect\n      - envs/.env.cluster\n      - path: envs/.env.${CONFIG_FILE:-default}\n"
               "    command: [\"bash\", \"/app/configs/serve_runner.sh\"]\n"
               "  vllm-slave-serve:\n    <<: *gpu-common\n    profiles: [slave]\n"
               "    container_name: ${SLAVE_CONTAINER_NAME:-slave}\n    environment:\n      NODE_ROLE: slave\n"
               "      CONFIG_FILE: ${CONFIG_FILE:-default}\n      RAY_PORT: ${RAY_PORT:-6379}\n"
               "      VLLM_PLE_MMAP: ${VLLM_PLE_MMAP:-0}\n    env_file:\n      - envs/.env.interconnect\n"
               "      - envs/.env.cluster\n    command: [\"bash\", \"/app/configs/serve_runner.sh\"]\n")
    (out / "docker-compose.yaml").write_text(compose, encoding="utf-8")
    ip = ".".join(["192", "168", "7", "11"])
    (out / "envs/.env.cluster").write_text(f"MASTER_HOST_IP={ip}\nSLAVE_HOST_IP=node-sub-fixture\nSSH_USER=opfixture\n"
                                           "MAX_JOBS=4\nRAY_PORT=6379\n", encoding="utf-8")
    (out / "envs/.env.interconnect").write_text("NCCL_DEBUG=INFO\nNCCL_SOCKET_IFNAME=enpfixture0\nNCCL_IB_DISABLE=1\n",
                                                encoding="utf-8")
    # env 형상의 mtime 을 측정(2026-01-02) **앞**의 고정 시각으로 둔다(2026-09-22 · plan_26092119 S2 round 3): artifacts 는 측정 뒤
    #   mtime 의 env 형상을 싣지 않는다(post-measurement-regenerated) — 픽스처를 쓰는 순간의 시계로 mtime 이 정해지면 AC8(멀티 형상이
    #   실린다) 경로가 실행 시각에 따라 사라진다. 고정 epoch 는 벽시계가 아니다(재생성 · 제외 경로는 artifacts · template 자체검사 몫).
    for env_p in [*(out / "envs").glob(".env*"), out / ".env"]:
        if env_p.is_file():
            os.utime(env_p, (_FX_ENV_MTIME_EPOCH, _FX_ENV_MTIME_EPOCH))
    assets = code_root / ".claude/skills/upstream-version-watch/assets/configs"
    for name in ("serve_runner.sh", "arm_patch.sh", "debug-init.sh"):
        if (assets / name).is_file():
            shutil.copy2(assets / name, out / "configs" / name)     # 마운트본 = asset 정본(바이트 동일)
    (out / "Dockerfile.source-build").write_text(
        "FROM base\nARG VLLM_REPO=x\nARG VLLM_REF=y\nARG SM12X_PORT=0\nARG BUILD_JOBS=16\n"
        "COPY requirements.txt /tmp/requirements.txt\nCOPY build_patches_src/ /tmp/build_patches_src/\n"
        "COPY build_patches/ /tmp/build_patches/\n", encoding="utf-8")
    (out / "Dockerfile").write_text("FROM wheel\nARG VLLM_VERSION=0.0.1\nCOPY requirements.txt /tmp/r.txt\n", encoding="utf-8")
    (out / "requirements.txt").write_text("transformers<5.15.0\n", encoding="utf-8")
    for d in ("build_patches_src", "build_patches"):
        (out / d).mkdir(exist_ok=True)
    (out / "build_patches_src/50-port.sh").write_text(
        '#!/bin/bash\n# model-trigger : OtherModel\n: "${SM12X_PORT:=0}"\nif [ "${SM12X_PORT}" != "1" ]; then\n'
        '    echo "skip"\n    exit 0\nfi\n', encoding="utf-8")
    (out / "build_patches_src/60-fixture-patch.sh").write_text("#!/bin/bash\n# fixture patch: PLE 게이트 완화\n# gate : 없음\n",
                                                               encoding="utf-8")
    (out / "build_patches/10-shared.sh").write_text("#!/bin/bash\n# model-trigger : 공유 이미지 arch-enablement\n",
                                                   encoding="utf-8")
    fx["docker"]["images"][evidence._DIGEST]["Created"] = "2026-01-01T00:00:00Z"
    docs = {
        fx["plan"]: "# plan_26010200 — 픽스처 계획\n\n## 1. 목표\n픽스처 모델을 GB10 두 노드에서 4096 컨텍스트로 서빙한다.\n",
        fx["devlog"]: "# devlog_26010213 — 픽스처 서사\n\n## 1. 경과\n벽 하나를 넘고 측정까지 갔다.\n",
        fx["testlog"]: ("# testlog_26010213 — 픽스처 셀 판정\n\n## 1. 판정\n픽스처 셀 c1-a 는 health 200 과 추론 1회로 PASS 다.\n\n"
                        f"## 2. 벽\n회차 3: {_FX_WALL_SIG}\n백엔드 명시를 걷자 통과했다.\n\n"
                        f"## 3. 오진\n{_FX_MISDIAG_SIG}.\n정정: 호스트 워치독이 사살했다.\n\n"
                        "## 4. 측정\nvllm bench serve --max-concurrency 1 --num-prompts 4\n"),
    }
    for rel, body in docs.items():
        (repo / rel).write_text(body, encoding="utf-8")
    (repo / core.REL_CENTRAL_FLAG).write_text("픽스처 — 이 체크아웃이 자기 원격의 hint 색인·배포 권위다.\n", encoding="utf-8")
    _fx_git(repo, "init", "-q")
    (repo / "README.fixture").write_text("fixture\n", encoding="utf-8")
    _fx_git(repo, "add", "README.fixture")
    _fx_git(repo, "commit", "-q", "-m", "fixture root")
    bare = td / "remote.git"
    _fx_git(td, "init", "-q", "--bare", str(bare))
    _fx_git(repo, "remote", "add", "origin", str(bare))
    # https 로 선언된 원격(토큰 없음 음성대조) — insteadOf 로 로컬 bare 에 돌린다(네트워크 0)
    _fx_git(repo, "remote", "add", "gh", "https://fixture.invalid/hint.git")
    _fx_git(repo, "config", f"url.{bare}.insteadOf", "https://fixture.invalid/hint.git")
    fx.update(bare=bare, models=models)
    return repo, fx


def _fx_legacy_remote(td: Path, repo: Path, bare: Path) -> dict:
    """AC6 — 과거·타 PC 태그 공존: ① 옛 5세그먼트(노드축 옛 문법) annotated 태그(로컬·원격) ② 로컬 오브젝트 없는 원격 전용 태그
    ③ 원격 lightweight 태그. 신규 발행은 이들에 막히지 않아야 한다(D10 · F12)."""
    old = "hint/0.9.0/fixture-model-nvfp4/gb10x2-cluster-native/len4096-kvauto-plemmap"
    _fx_git(repo, "tag", "-a", "-m", "옛 brief — 노드축 옛 문법", old, "HEAD")
    _fx_git(repo, "push", "-q", "origin", f"refs/tags/{old}:refs/tags/{old}")
    foreign = td / "foreign"
    _fx_git(td, "clone", "-q", str(bare), str(foreign))
    (foreign / "x.txt").write_text("타 PC\n", encoding="utf-8")
    _fx_git(foreign, "add", "x.txt")
    _fx_git(foreign, "commit", "-q", "-m", "foreign")
    remote_only = "hint/0.9.0/other-model/gb10-1g1n-main-native/qbf16-len2048-kvauto-plenone-specoff-graph"
    lightweight = "hint/0.8.0/lw-model/gb10-sim-h100/qfp8-len1024-kvfp8"
    _fx_git(foreign, "tag", "-a", "-m", "타 PC 발행", remote_only, "HEAD")
    _fx_git(foreign, "tag", lightweight, "HEAD")
    _fx_git(foreign, "push", "-q", "origin", f"refs/tags/{remote_only}:refs/tags/{remote_only}",
            f"refs/tags/{lightweight}:refs/tags/{lightweight}")
    return {"old": old, "remote_only": remote_only, "lightweight": lightweight}


def _fx_quote(val: str) -> str:
    if val == "":
        return "(빈 값)"
    if any(ch in val for ch in ",:#\"'") or val[:1] in "[{|>&*!":
        q = "'" if "'" not in val else '"'
        return f"{q}{val}{q}"
    return val


def _fx_author(repo: Path, draft: Path, st: dict, *, factcheck: bool = True) -> None:
    """PROMPT 절의 프로그램 저작(자체검사 전용) — 템플릿 지시(블록 kind·최소 개수·필수 발췌)를 읽어 유효 hint-event 와 **치환 후 원문**
    발췌를 만든다. 템플릿이 바뀌어도 이 생성기가 따라간다(지시를 여기 다시 적지 않는다).
    `factcheck=True`(기본)면 저작 뒤 **독립 검증 대역**의 보고(`_fx_factcheck` · author ≠ checker 역할 이름)까지 남긴다 — 발행 절차의
    두 행위자(저작 → 독립 사실 검증)를 한 픽스처 호출로 대신한다(2026-09-22 S2 round 2 · 이 저작기를 부르는 정책 자체검사
    `runtime_selftest` 의 CLI E2E 가 사실 검증 게이트를 지나 continue 까지 가도록). 게이트의 음성대조는 이 보고를 지우거나 망가뜨려 만든다."""
    payload = draft / "payload"
    facts = template.load_snapshot(draft / "inputs" / template.FACTS_SNAPSHOT)
    stem = Path(_FX_TESTLOG).stem
    src = excerpt_text(repo, draft, stem)
    wall_line = next(ln for ln in src.split("\n") if _FX_WALL_SIG in ln)
    diag_line = next(ln for ln in src.split("\n") if _FX_MISDIAG_SIG in ln)
    counter = {"W": 0, "R": 0, "M": 0, "V": 0, "Q": 0}

    def nid(prefix: str) -> str:
        counter[prefix] += 1
        return f"{prefix}{counter[prefix]}"

    def event(kind: str) -> list[str]:
        if kind == "value-status":
            out = []
            for c in facts.get("value_status_candidates") or []:
                status = c.get("candidate_status") or "inherited"
                # declared-requirement 는 관측된 실패(W id)를 근거로 든다(2026-09-22 S2 round 2 린트 규칙 ⑩a)
                why = "픽스처 — 빼면 W1 이 다시 난다" if status == "declared-requirement" else "픽스처 — 이 셀에서 조정 기록 없음"
                out.append("```hint-event\n" + "\n".join([
                    f"id: {nid('V')}", "kind: value-status", f"노브: {c['knob']}",
                    f"값: {_fx_quote(str(c.get('value') if c.get('value') is not None else ''))}",
                    f"지위: {status}", f"근거: {why}", f"출처: [{stem} §2]"]) + "\n```")
            return out
        body = {
            "wall": ["kind: wall", "증상: 기동 중 백엔드 선택에서 실패", f'서명: "{_FX_WALL_SIG}"', "원인: 명시한 백엔드가 가드와 충돌",
                     "해소: 백엔드 명시 제거", f"검증: {stem} §2", "전이등급: arch-scaled", f"출처: [{stem} §2]"],
            "misdiagnosis": ["kind: misdiagnosis", "증상: hang", f'서명: "{_FX_MISDIAG_SIG}"', "원인: 워치독을 못 봤다",
                             "해소: 호스트 워치독 사살로 정정", f"검증: {stem} §3", "전이등급: judgment", "현재지위: 반증",
                             f"출처: [{stem} §3]"],
            "rejected": ["kind: rejected", "시도: 백엔드 명시", "기각사유: 가드와 충돌", f"출처: [{stem} §2]"],
            "open-question": ["kind: open-question", "물음: cold TTFT 이상치", "현재상태: 미설명", f"출처: [{stem} §4]"],
        }[kind]
        return ["```hint-event\n" + "\n".join([f"id: {nid({'wall': 'W', 'misdiagnosis': 'M', 'rejected': 'R', 'open-question': 'Q'}[kind])}",
                                                *body]) + "\n```"]

    excerpt = f"> [원문] {stem} §2. 벽\n> {wall_line}"
    # 비-마크다운 출처 발췌 = `§L<a>-<b>`(excerpt 도우미의 정규화 줄 번호 그대로 · 2026-09-22 S2 round 2 F8). 이 픽스처 로그는
    #   `_fx_repo` 만 만든다 — 다른 픽스처(정책 runtime_selftest 의 CLI E2E)에서 부르면 계보에 없으므로 이 발췌만 싣지 않는다
    #   (저작 대역의 선택이지 발행 경로의 폴백이 아니다 · 그 픽스처의 린트 판정은 그대로다).
    log_excerpt = None
    try:
        log_rows = excerpt_text(repo, draft, Path(_FX_LOG_REL).name).split("\n")
    except core.HintError:
        log_rows = []
    if "Loading 100%" in log_rows:
        la = log_rows.index("Loading 100%") + 1
        log_excerpt = f"> [원문] {Path(_FX_LOG_REL).name} §L{la}-{la + 1}\n> {log_rows[la - 1]}\n> {log_rows[la]}"
    todo = {(p["file"], p["section"]): p for p in template.prompts(payload)}
    for name in template.PAYLOAD_DOCS:
        path = payload / name
        lines = path.read_text(encoding="utf-8").split("\n")
        chapter = None
        out = []
        for ln in lines:
            m = re.match(r"^## (\d+\.\d+) ", ln)
            if m:
                chapter = m.group(1)
            if not ln.startswith(template.AGENT_MARK):
                out.append(ln)
                continue
            p = todo.get((name, chapter)) or {"blocks": {}, "excerpts_required": 0}
            if chapter == "0.1":
                # brief 수치는 사실 블록에 글자 그대로 있는 값만(2026-09-22 S2 round 2 린트 규칙) — 셀 형태(full · lite)마다 다른
                #   측정 칸에서 고른다(값을 여기 손으로 적지 않는다).
                m = facts.get("measurement") or {}
                v = m.get("decode_tps_conc1") if m.get("decode_tps_conc1") is not None else (m.get("lite") or {}).get("gen_tps")
                num = f"동시성 1 decode {v} t/s" if v is not None else "판정점 수치는 03 표"
                out += [f"픽스처 모델을 GB10 1g2n cluster(TP=2)·v0.9.0 소스빌드로 서빙했다 — {num}.",
                        "가장 비싼 벽은 백엔드 가드 충돌(W1)이었고 백엔드 명시를 걷어 넘었다."]
                continue
            parts = [f"픽스처 저작 — {name} §{chapter}: 계보 testlog 를 읽고 이 절의 질문에 답했다."]
            if chapter == "1.2":
                parts.append("### 60-fixture-patch.sh\n재구성 결과라 적용 미관측이다 — PLE 게이트를 완화하는 픽스처 패치(W1 과 무관 · "
                             "폐기 조건: 업스트림 정식 지원).")
            if chapter == "1.4" and log_excerpt:
                parts.append(log_excerpt)
            for kind, need in (p.get("blocks") or {}).items():
                for _ in range(max(int(need or 0), 1) if kind != "value-status" else 1):
                    parts += event(kind)
            for _ in range(int(p.get("excerpts_required") or 0)):
                parts.append(excerpt)
            if chapter == "2.4" and "misdiagnosis" in (p.get("blocks") or {}):
                parts.append(f"> [원문] {stem} §3\n> {diag_line}")
            out += "\n\n".join(parts).split("\n")
        path.write_text("\n".join(out), encoding="utf-8")
    if factcheck:
        _fx_factcheck(draft, st)


def _fx_factcheck(draft: Path, st: dict, **over) -> dict:
    """유효한 발행 전 사실 검증 보고(자체검사 전용 · 검증 Agent 역할 대역). over 로 한 칸씩 망가뜨려 음성대조를 만든다."""
    doc = {"schema_version": FACTCHECK_SCHEMA, "tag": st["tag"], "author": "hint-author-agent", "checker": "hint-factcheck-agent",
           "checked_utc": _FX_CONTINUE_UTC, "claims_checked": 12, "classes_checked": sorted(template.FACTCHECK_CLASSES),
           "items": [{"id": "F1", "where": "02-narrative.md §2.2", "claim": "벽 W1 의 원인", "verdict": "misleading", "class": 5,
                      "truth": "원인 문장에 출처가 없었다", "source": Path(_FX_TESTLOG).stem, "status": "fixed",
                      "resolution": "원인을 출처 문장으로 바꿨다"}]}
    doc.update(over)
    core.write_json(draft / "inputs" / FACTCHECK_NAME, doc)
    return doc


def _raises(fn, code: str) -> tuple[bool, str]:
    try:
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            fn()
    except core.HintError as e:
        return e.code == code, f"{e.code}: {e.message[:300]}"
    return False, "예외 없음"


def _refs(repo: Path) -> str:
    return _fx_git(repo, "for-each-ref", "--format=%(refname) %(objectname)").stdout


def _selftest_e2e(ck) -> None:
    saved = {k: os.environ.pop(k, None) for k in ("GITHUB_TOKEN", "QUANT_MODEL_PATH", "GIT_DIR", "GIT_WORK_TREE",
                                                  "GIT_INDEX_FILE")}
    try:
        with tempfile.TemporaryDirectory(prefix="hint-e2e-") as tds:
            td = Path(tds)
            repo, fx = _fx_repo(td)
            dstate = fx["docker"]
            rt = Runtime(docker=_fx_docker(dstate))
            bare = fx["bare"]
            legacy = _fx_legacy_remote(td, repo, bare)

            def quiet(fn, *a, **kw):
                with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                    return fn(*a, **kw)

            # ── publish(campaign) ──
            try:
                res = quiet(publish, repo, campaign="c1", cell="c1-a", generated_utc=_FX_PUBLISH_UTC, rt=rt)
            except core.HintError as e:
                ck(f"publish(campaign) 성공 — {e.code}: {e.message[:600]} → {e.remedy}", False)
                return
            tagname, draft = res["tag"], res["draft"]
            payload = draft / "payload"
            ck(f"publish 이름 = 도구 파생 v6({tagname})", tagname == "hint/0.9.0/fixture-model-nvfp4/gb10-1g2n-cluster-native/"
                                                         "qnvfp4-len4096-kvauto-plemmap-spec2-eager")
            ck("publish 는 태그·hint 브랜치를 만들지 않는다", tag.tag_ref_kind(repo, tagname) is None
               and branch.hint_tip(repo) is None)
            # 2026-09-22 S2 round 2 적대 리뷰: 탐침이 꺼진 채 미관측이 남으면 publish 출력이 처방을 말한다(조용한 강등 ✗) · round 3: 탐침이
            #   기본이 되어 처방 = `--docker-read-only` 를 뺀다(옛 옵트인 처방 `--docker-probe` ✗)
            ck("publish 요약: 적용 판정 수 · 이미지 탐침 결과", _applied_summary(
                {"patches": [{"result": "unobserved(적용 미관측)"}, {"result": "applied"}],
                 "probes": [{"tier": "image-probe", "result": "skipped(x)"}]}) == {"total": 2, "unobserved": 1, "image_probe": "skipped(x)"}
               and isinstance(res.get("applied"), dict) and res["applied"].get("total", 0) >= 1)
            outs = []
            for ap in ({"total": 9, "unobserved": 7, "image_probe": "skipped(주입 실행기가 image_probe 를 선언하지 않았다)"},
                       {"total": 9, "unobserved": 1, "image_probe": "done"}):
                buf = io.StringIO()
                with contextlib.redirect_stdout(buf):
                    _print_publish(repo, {**res, "applied": ap})
                outs.append(buf.getvalue())
            ck("★탐침 꺼짐 + 미관측 = publish 출력이 처방(`--docker-read-only` 를 뺀다)을 말한다 · 탐침이 돌았으면 처방 줄 없음 · 옛 옵트인 처방 ✗",
               "`--docker-read-only`" in outs[0] and "미관측 7개" in outs[0] and "탐침이 돌지 않은" not in outs[1]
               and "`--docker-probe`" not in outs[0])
            # 2026-09-22 round 3 적대 리뷰: 저작 안내가 스냅샷 출처 토큰과 `--numbered`(§L 번호의 원천)를 말한다 — 생산자가 준 스냅샷 이름 그대로
            buf_s = io.StringIO()
            with contextlib.redirect_stdout(buf_s):
                _print_publish(repo, {**res, "tool_snapshots": ["run_bench.sh@0123456789ab"]})
            ck("★publish 저작 안내: excerpt 출처에 `<도구>@<rev12>` · `--numbered`(§L 번호) · 스냅샷 토큰 목록(없으면 '없음')",
               "<도구>@<rev12>" in buf_s.getvalue() and "[--numbered]" in buf_s.getvalue()
               and "스냅샷(발췌 출처 토큰 · 03 §3.4): run_bench.sh@0123456789ab" in buf_s.getvalue()
               and isinstance(res.get("tool_snapshots"), list)
               and all(isinstance(x, str) and "@" in x for x in res["tool_snapshots"]))
            ck("draft = hints/.drafts/<topic>/ · state.json scaffolded", draft.parent == repo / core.REL_DRAFTS
               and _load_state(draft)["stage"] == "scaffolded")
            files = set(res["files"])
            ck("페이로드 최상위 8종", set(branch.PAYLOAD_ALLOWLIST_TOP) <= files)
            ck("적용된 것만: skip 패치(50)·쓰이지 않은 wheel Dockerfile 미포함 · 60·post 10 포함",
               "artifacts/build_patch_pre/50-port.sh" not in files and "artifacts/build_recipe/Dockerfile" not in files
               and "artifacts/build_patch_pre/60-fixture-patch.sh" in files and "artifacts/build_patch_post/10-shared.sh" in files)
            ck("D-a source-build 가 COPY 하는 requirements.txt 는 싣는다", "artifacts/build_recipe/requirements.txt" in files)
            ck("멀티: sub_recipe.json · serve_runner.sh · cluster/interconnect 형상(AC8)",
               {"artifacts/compose/sub_recipe.json", "artifacts/compose/serve_runner.sh",
                "artifacts/compose/.env.cluster.template", "artifacts/compose/.env.interconnect.template"} <= files)
            pdoc = json.loads((payload / "PAYLOAD.json").read_text(encoding="utf-8"))
            ck("PAYLOAD schema 2 · format v6 · naming 재조립 = 태그", pdoc.get("schema_version") == 2
               and pdoc.get("format") == branch.PAYLOAD_FORMAT and naming.compose_from_payload(pdoc["naming"]) == tagname)
            # 2026-09-22 S2 round 3: 공유 사실 계약의 새 키가 facts 스냅샷 · PAYLOAD 에 같은 값으로 실리고 사실 블록 표지가 문서에 있다
            snap = template.load_snapshot(draft / "inputs" / template.FACTS_SNAPSHOT)
            new_keys = ("measurement_env_observed", "tool_snapshots", "attestation_scope")
            ck("R3 facts · PAYLOAD: measurement_env_observed · tool_snapshots(list) · attestation_scope 키 — 같은 값",
               all(k in snap and k in pdoc and snap[k] == pdoc[k] for k in new_keys)
               and isinstance(snap["measurement_env_observed"], list) and isinstance(snap["tool_snapshots"], list)
               and all(f"<!-- FACT:{fid} -->" in (payload / doc).read_text(encoding="utf-8") for fid, doc in (
                   ("measurement_env", "01-artifacts.md"), ("measurement_env_nodes", "01-artifacts.md"),
                   ("tool_snapshots", "03-benchmark.md"))))
            vobs = pdoc["naming"].get("vllm_observed") or {}
            ck("R3 vllm_observed = 키별 생산자(엔진 자기보고 · 인증서 · wheel 각각 출처 칸)", all(
                f"{k}_source" in vobs for k in ("engine_self_report", "certificate_vllm_version", "wheel_meta")))
            ck("D-f measurement = 수치 · measurement_config = 배포 키 + source",
               "decode_tps_conc1" in pdoc["measurement"] and set(pdoc["measurement_config"]) ==
               set(evidence.DISTRIBUTED_MEASUREMENT_KEYS) | {"source"})
            ck("D-b value-status 후보에서 model 제외", "model" not in {c["knob"] for c in
                                                                 template.load_snapshot(draft / "inputs" / template.FACTS_SNAPSHOT)
                                                                 ["value_status_candidates"]})
            ck("PROVENANCE 앵커는 커밋 전까지 비어 있다", json.loads((payload / "PROVENANCE.json").read_text(encoding="utf-8"))
               .get("source_anchor") is None)
            ck("docker 는 읽기(inspect·history)와 원장 cat(--network none)만", all(
                c[1] in ("image", "history") or (c[1] == "run" and "--network" in c and "none" in c) for c in dstate["calls"]))

            # ── ★배포 PII 에 걸리는 파생 이름 = publish 에서 멈춘다(커밋·봉인에서 반드시 거부될 이름 · 조각 조립 — 자기스캔) ──
            ipish = ".".join(["0", "10", "1", "1"])
            ck("★4마디 릴리스가 사설 IPv4 모양이면 HINT_TAG_NAME_PII(저작 전에 멈춘다) · 정상 이름은 통과",
               _raises(lambda: _require_name_pii_clean(repo, tagname.replace("/0.9.0/", f"/{ipish}/")), "HINT_TAG_NAME_PII")[0]
               and _raises(lambda: _require_name_pii_clean(repo, tagname), "HINT_TAG_NAME_PII")[1] == "예외 없음")
            real_derive = naming.derive_name
            naming.derive_name = lambda f, v: dataclasses.replace(real_derive(f, v), tag=tagname.replace("/0.9.0/", f"/{ipish}/"))
            tree_p = _fx_tree(repo)
            try:
                ok, why = _raises(lambda: publish(repo, campaign="c1", cell="c1-a", generated_utc=_FX_PUBLISH_UTC,
                                                  out=td / "namepii", rt=rt), "HINT_TAG_NAME_PII")
            finally:
                naming.derive_name = real_derive
            ck(f"★publish 가 이름 PII 를 발행기 구동 전에 막는다(쓰기 0)({why[:60]})", ok and _fx_tree(repo) == tree_p
               and not any((td / "namepii").rglob("*")))

            # ── ★끊긴 publish 를 다른 시각으로 다시 = 발행기 구동 전에 막힌다(발행 기록 시각 불변 · 부수효과 0) ──
            rec_p = repo / core.REL_EVIDENCE_DIR / f"{res['topic']}.json"
            rec0, tree0 = rec_p.read_bytes(), _fx_tree(repo)
            ok, why = _raises(lambda: publish(repo, campaign="c1", cell="c1-a", generated_utc="2026-01-02T06:05:00Z",
                                              out=td / "timebound", rt=rt), "HINT_PUBLICATION_TIME_BOUND")
            ck(f"★발행 기록이 다른 시각으로 묶임 = HINT_PUBLICATION_TIME_BOUND · 쓰기 0({why[:90]})",
               ok and rec_p.read_bytes() == rec0 and _fx_tree(repo) == tree0 and _FX_PUBLISH_UTC in why
               and not any((td / "timebound").rglob("*")))

            # ── ★재생은 draft 밖에 쓰지 않는다 ──
            before, refs0 = _fx_tree(repo), _refs(repo)
            rout = td / "replay"
            try:
                rres = quiet(publish, repo, publication=res["topic"], replay=True, generated_utc=_FX_REPLAY_UTC, out=rout, rt=rt)
                rok = True
            except core.HintError as e:
                rres, rok = None, False
                ck(f"재생 publish 성공 — {e.code}: {e.message[:400]}", False)
            if rok:
                ck("★재생 = 저장소 트리·ref 불변(쓰기는 --out draft 안뿐)", _fx_tree(repo) == before and _refs(repo) == refs0)
                ck("재생 = 같은 셀 → 같은 이름 · 모드 publication-replay", rres["tag"] == tagname
                   and rres["mode"] == "publication-replay" and (rout / STATE_NAME).is_file())
            ck("★--replay 없는 --publication 거부", _raises(lambda: publish(repo, publication=res["topic"], generated_utc=_FX_REPLAY_UTC,
                                                                        out=td / "r2", rt=rt), "HINT_REPLAY_FLAG_REQUIRED")[0])

            # ── ★승인 부재 = 어느 ref 도 움직이지 않는다 ──
            decl_p = repo / "campaigns/c1/campaign.yaml"
            decl_text = decl_p.read_text(encoding="utf-8")
            core.write_json(decl_p, {**fx["decl"], "hint_targets": []})
            ok, why = _raises(lambda: continue_(repo, draft=draft, generated_utc=_FX_CONTINUE_UTC, rt=rt), "HINT_APPROVAL_ABSENT")
            ck(f"★승인 부재 = HINT_APPROVAL_ABSENT({why[:120]})", ok)
            ck("★승인 부재 = ref 쓰기 0", _refs(repo) == refs0 and branch.hint_tip(repo) is None)
            decl_p.write_text(decl_text, encoding="utf-8")
            ck("★캠페인 셀에 CLI 승인 = 원천 충돌", _raises(lambda: continue_(
                repo, draft=draft, generated_utc=_FX_CONTINUE_UTC, approved_by="x", approved_utc=_FX_CONTINUE_UTC, rt=rt),
                "HINT_APPROVAL_SOURCE_CONFLICT")[0])

            # ── ★린트 실패(미저작) = 커밋 전 차단 ──
            ok, why = _raises(lambda: continue_(repo, draft=draft, generated_utc=_FX_CONTINUE_UTC, rt=rt), "HINT_LINT_FAILED")
            ck(f"★미저작 = HINT_LINT_FAILED(커밋 전) — {why[:160]}", ok)
            ck("★린트 실패 = hint 브랜치·태그 없음", branch.hint_tip(repo) is None and tag.tag_ref_kind(repo, tagname) is None)
            ck("lint 도 미저작을 잡는다(부수효과 0)", {"HINT_AGENT_PLACEHOLDER_RESIDUE", "HINT_SECTION_UNAUTHORED"}
               <= {i["code"] for i in quiet(lint_draft, repo, draft)})

            # ── 저작(프로그램) → lint 0 ──
            _fx_author(repo, draft, _load_state(draft))
            issues = quiet(lint_draft, repo, draft)
            ck(f"저작 뒤 lint 0건: {[(i['code'], i['file'], i['message'][:80]) for i in issues[:4]]}", issues == [])
            ex = excerpt_text(repo, draft, Path(_FX_TESTLOG).stem, "7-8")
            ck("excerpt 도우미 --lines(치환 후 원문 · 1-기반)", _FX_WALL_SIG in ex and ex.count("\n") == 1)
            ok, why = _raises(lambda: excerpt_text(repo, draft, Path(_FX_TESTLOG).stem, "9999-9999"), "HINT_EXCERPT_LINES_OUT_OF_RANGE")
            ck(f"★excerpt --lines 범위 밖 = HINT_EXCERPT_LINES_OUT_OF_RANGE(빈 출력으로 조용히 돌려주지 않는다)({why[:50]})", ok)

            # ── F8(2026-09-22 S2 round 2): 출처 정규화 · NIC 치환 · 비-마크다운 §L 표지 ──
            log_name = Path(_FX_LOG_REL).name
            norm_log = excerpt_text(repo, draft, log_name)
            raw_rows = (repo / _FX_LOG_REL).read_bytes().split(b"\n")
            att = next(i for i, r in enumerate(raw_rows) if r.startswith(b"AttributeError"))
            ck("excerpt 줄 번호 = 원본 `\\n` 줄 번호(grep -n) · ANSI ✗ · `\\r` 조각은 마지막만",
               len(norm_log.split("\n")) == len(raw_rows) and norm_log.split("\n")[att].startswith("AttributeError")
               and "\x1b" not in norm_log and "\r" not in norm_log and "Loading 50%" not in norm_log
               and excerpt_text(repo, draft, log_name, f"{att + 1}-{att + 1}").startswith("AttributeError"))
            # 2026-09-22 · plan_26092119 S2 round 3: `--numbered` = 린터가 대조하는 §L 번호(정규화 · 치환 뒤 `\n` 분할)를 붙인다
            num_all = excerpt_text(repo, draft, log_name, numbered=True).split("\n")
            num_rng = excerpt_text(repo, draft, log_name, f"{att + 1}-{att + 2}", numbered=True).split("\n")
            ck("excerpt --numbered: `L<n>\\t` = grep -n 번호 · --lines 와 함께면 a 부터 · 번호 뗀 본문 = 번호 없는 출력",
               num_all[att] == f"L{att + 1}\t" + norm_log.split("\n")[att] and num_rng[0].startswith(f"L{att + 1}\tAttributeError")
               and num_rng[1].startswith(f"L{att + 2}\t") and [r.split("\t", 1)[1] for r in num_rng]
               == excerpt_text(repo, draft, log_name, f"{att + 1}-{att + 2}").split("\n"))
            buf_n = io.StringIO()
            with contextlib.redirect_stdout(buf_n):
                rc_n = main(["--repo", str(repo), "excerpt", "--draft", str(draft), "--source", log_name, "--numbered",
                             "--lines", f"{att + 1}-{att + 1}"])
            ck("★excerpt --numbered CLI 배선(플래그가 핸들러까지 간다 · 번호 없는 출력과 다르다)", rc_n == 0
               and buf_n.getvalue().startswith(f"L{att + 1}\t") and not excerpt_text(repo, draft, log_name).startswith("L1\t"))
            ck("excerpt NIC 치환: manifest 최상위 interconnect 장치 이름 → <nic:cluster>(두 노드 공통 · <nic:main> ✗)",
               "Using [0]<nic:cluster>:1/RoCE ; OOB <nic:cluster>" in norm_log and _FX_NIC_HCA not in norm_log
               and _FX_NIC_IFACE not in norm_log)
            ck("비-마크다운 §L 발췌가 실린 저작 = lint 0(위 판정에 포함)", f"> [원문] {log_name} §L" in
               (payload / "01-artifacts.md").read_text(encoding="utf-8"))

            def lint_variant(tag: str, old: str, new: str) -> set[str]:
                vd = td / "lintvar" / tag
                shutil.copytree(draft, vd)
                f = vd / "payload" / "01-artifacts.md"
                txt = f.read_text(encoding="utf-8")
                if old not in txt:
                    raise RuntimeError(f"변형 앵커 없음: {old[:40]!r}")
                f.write_text(txt.replace(old, new, 1), encoding="utf-8")
                return {i["code"] for i in quiet(lint_draft, repo, vd)}

            head = next(ln for ln in (payload / "01-artifacts.md").read_text(encoding="utf-8").split("\n")
                        if ln.startswith(f"> [원문] {log_name} §L"))
            ck("★E2E: 로그 발췌 머리를 제목식(§엔진 로그)으로 = HINT_EXCERPT_SECTION_SHAPE",
               "HINT_EXCERPT_SECTION_SHAPE" in lint_variant("shape", head, f"> [원문] {log_name} §엔진 로그"))
            ck("★E2E: 로그 발췌 줄 범위가 인용 밖(§L1-2) = HINT_EXCERPT_OUTSIDE_LINES",
               "HINT_EXCERPT_OUTSIDE_LINES" in lint_variant("outside", head, f"> [원문] {log_name} §L1-2"))
            ck("★E2E: ANSI 를 되살린 인용 = HINT_EXCERPT_MISMATCH", "HINT_EXCERPT_MISMATCH" in lint_variant(
                "ansi", "> Loading 100%", "> \x1b[36mLoading 100%\x1b[0m"))

            # 2026-09-22 · plan_26092119 S2 round 3: 측정 도구 스냅샷 발췌가 **lint_draft 의 임시 사본 경로**에서도 해소된다(라이브 재생에서
            #   사본 옆에 inputs/ 가 없어 계보 밖으로 떨어졌다 — draft_dir 명시로 고쳤다). 스냅샷 · LINEAGE 후보는 생산자 모양 그대로 심는다.
            # 2026-09-22 round 3 적대 리뷰: 스냅샷은 LINEAGE origin(`git show <rev>:<path>`)과 바이트가 같아야 출처다 — 픽스처 origin 은
            #   **실재하는** 커밋이어야 한다. ref 를 움직이지 않는 매달린 커밋(hash-object · mktree · commit-tree)으로 만든다(뒤 시험의 HEAD ·
            #   refs 불변 · 결정론 날짜).
            snap_bytes = "#!/bin/bash\nvllm bench serve --ignore-eos --temperature 0\n"
            blob_src = td / "snap_blob.sh"
            blob_src.write_text(snap_bytes, encoding="utf-8")
            blob = _fx_git(repo, "hash-object", "-w", str(blob_src)).stdout.strip()
            tree = core.git(repo, "mktree", input_text=f"100644 blob {blob}\trun_bench.sh\n").stdout.strip()
            snap_commit = _fx_git(repo, "commit-tree", tree, "-m", "fixture tool snapshot origin").stdout.strip()
            snap_name = f"run_bench.sh@{snap_commit[:12]}"

            def snapshot_draft(tag: str, *, listed: bool, body: str = snap_bytes) -> Path:
                vd = td / "snapvar" / tag
                shutil.copytree(draft, vd)
                (vd / "inputs" / "sources").mkdir(parents=True, exist_ok=True)
                (vd / "inputs" / "sources" / snap_name).write_text(body, encoding="utf-8")
                if listed:
                    lp = vd / "payload" / "LINEAGE.json"
                    lin_doc = json.loads(lp.read_text(encoding="utf-8"))
                    lin_doc.setdefault("evidence_candidates", []).append(
                        {"path": f"inputs/sources/{snap_name}", "kind": "tool-source@rev", "origin": f"git:{snap_commit}:run_bench.sh"})
                    lp.write_text(json.dumps(lin_doc, ensure_ascii=False, indent=2), encoding="utf-8")
                quote = body.split("\n")[1]
                f = vd / "payload" / "01-artifacts.md"
                f.write_text(f.read_text(encoding="utf-8").replace(
                    head, f"> [원문] {snap_name} §L2-2\n> {quote}\n\n{head}", 1), encoding="utf-8")
                return vd

            refs_snap = _refs(repo)
            sd = snapshot_draft("listed", listed=True)
            sd_codes = {i["code"] for i in quiet(lint_draft, repo, sd)}
            ck(f"R3 E2E: 스냅샷 발췌(`<도구>@<rev12>` §L · origin = 실재 커밋) = lint_draft(임시 사본) 통과({sorted(sd_codes)})",
               not sd_codes and len(snap_commit) == 40)
            ck("R3 E2E: excerpt 도우미도 스냅샷을 draft 에서 읽는다", excerpt_text(repo, sd, snap_name, "2-2")
               == "vllm bench serve --ignore-eos --temperature 0")
            ck("★R3 E2E: LINEAGE 에 등재하지 않은 스냅샷 = HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE", "HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE" in {
                i["code"] for i in quiet(lint_draft, repo, snapshot_draft("unlisted", listed=False))})
            tampered = snapshot_draft("tampered", listed=True, body=snap_bytes.replace("--temperature 0", "--temperature 1"))
            ck("★R3 E2E: 스냅샷 파일을 고치고 발췌를 거기 맞추면 = HINT_EXCERPT_SNAPSHOT_DRIFT(실물 git show 대조) · excerpt 도 거부",
               "HINT_EXCERPT_SNAPSHOT_DRIFT" in {i["code"] for i in quiet(lint_draft, repo, tampered)}
               and _raises(lambda: excerpt_text(repo, tampered, snap_name), "HINT_EXCERPT_SNAPSHOT_DRIFT")[0])
            ck("R3 E2E: 픽스처 origin 커밋은 ref 를 움직이지 않았다(매달린 커밋)",
               _refs(repo) == refs_snap)

            # ── F10: refresh 서브명령(draft 안 파생 블록만 · 미리 보기) ──
            h00 = payload / "00-hint.md"
            before00 = h00.read_text(encoding="utf-8")
            rc_r = quiet(main, ["--repo", str(repo), "refresh", "--draft", str(draft)])
            after00 = h00.read_text(encoding="utf-8")
            ck("CLI refresh rc 0 · 00 §0.3 벽 지도 · §0.5 노브 요약이 저작 내용으로 채워진다(그 밖 바이트 불변)", rc_r == 0
               and "W1" in template.fact_blocks(after00)["wall_map"] and "`V1`" not in before00.split("<!-- FACT:knob_status -->")[1][:40]
               and before00.split("<!-- FACT:wall_map -->")[0] == after00.split("<!-- FACT:wall_map -->")[0])
            ck("refresh 멱등(두 번째 = 바뀐 블록 없음) · refresh 뒤에도 lint 0", refresh_draft(repo, draft) == []
               and quiet(lint_draft, repo, draft) == [])
            ok, why = _raises(lambda: cmd_refresh(argparse.Namespace(repo=str(repo), draft=str(td / "no-draft"))), "HINT_DRAFT_ABSENT")
            ck(f"★refresh: publish 된 draft 가 아니면 HINT_DRAFT_ABSENT({why[:60]})", ok)

            # ── F11: 발행 전 독립 사실 검증 게이트(린트 0 뒤 · 봉인·커밋 전 · ref 쓰기 0) ──
            #   저작 대역이 남긴 독립 검증 대역의 보고를 **지우고** 부재부터 본다(음성대조는 보고를 빼거나 망가뜨려 만든다).
            ck("저작 대역은 독립 검증 대역의 보고까지 남긴다(author ≠ checker 역할 이름)",
               (draft / "inputs" / FACTCHECK_NAME).is_file())
            (draft / "inputs" / FACTCHECK_NAME).unlink()
            refs_fc = _refs(repo)
            ok, why = _raises(lambda: continue_(repo, draft=draft, generated_utc=_FX_CONTINUE_UTC, rt=rt), "HINT_FACTCHECK_ABSENT")
            st_fc = _load_state(draft)
            ck(f"★사실 검증 보고 없음 = HINT_FACTCHECK_ABSENT · ref 쓰기 0 · 봉인 전({why[:70]})", ok and _refs(repo) == refs_fc
               and branch.hint_tip(repo) is None and "<!-- PROMPT" in h00.read_text(encoding="utf-8")
               and (_step(st_fc, "factcheck") or {}).get("ok") is False and (_step(st_fc, "lint") or {}).get("ok") is True
               and (_step(st_fc, "lint") or {}).get("sealed") is False)
            ck("사실 검증 미통과 상태가 00 메타에 기계 기재(미통과 · 사유코드)", "미통과(`HINT_FACTCHECK_ABSENT`)" in
               template.fact_blocks(h00.read_text(encoding="utf-8"))["meta"])
            st0 = _load_state(draft)
            bad_cases = {
                "checker == author": ({"checker": "hint-author-agent"}, "HINT_FACTCHECK_INVALID"),
                "7부류 중 하나 빠짐": ({"classes_checked": [1, 2, 3, 4, 5, 6]}, "HINT_FACTCHECK_INVALID"),
                "다른 draft 의 태그": ({"tag": "hint/x/y/z/w"}, "HINT_FACTCHECK_INVALID"),
                "fixed 인데 resolution 없음": ({"items": [{"id": "F1", "where": "w", "claim": "c", "verdict": "wrong", "class": 1,
                                                        "status": "fixed"}]}, "HINT_FACTCHECK_INVALID"),
                "열린 오답": ({"items": [{"id": "F1", "where": "w", "claim": "c", "verdict": "wrong", "class": 7,
                                        "status": "open"}]}, "HINT_FACTCHECK_OPEN"),
                "이견(disputed)도 열림": ({"items": [{"id": "F1", "where": "w", "claim": "c", "verdict": "misleading", "class": 6,
                                                  "status": "disputed"}]}, "HINT_FACTCHECK_OPEN"),
                "근거 없음(unsupported)도 막는다": ({"items": [{"id": "F1", "where": "w", "claim": "c", "verdict": "unsupported",
                                                        "class": 5, "status": "open"}]}, "HINT_FACTCHECK_OPEN"),
            }
            for label, (over, code) in bad_cases.items():
                _fx_factcheck(draft, st0, **over)
                ok, why = _raises(lambda: continue_(repo, draft=draft, generated_utc=_FX_CONTINUE_UTC, rt=rt), code)
                ck(f"★사실 검증 {label} = {code} · ref 쓰기 0({why[:60]})", ok and _refs(repo) == refs_fc)
            st_w = _load_state(draft)
            ok, why = _raises(lambda: continue_(repo, draft=draft, generated_utc=_FX_CONTINUE_UTC, factcheck_waiver="<<FILL>>", rt=rt),
                              "HINT_FACTCHECK_WAIVER_INVALID")
            ck(f"★사실 검증 면제 자리표시 = HINT_FACTCHECK_WAIVER_INVALID · 쓰기 전 거부(state 불변)({why[:50]})",
               ok and _load_state(draft) == st_w and _refs(repo) == refs_fc)
            _fx_factcheck(draft, st0)                       # 유효 보고(지적 1건 · fixed) — 이후 흐름은 게이트를 통과한다

            # ── ★footer 에 묶을 계측 산출물이 없으면 커밋 **전에** 막힌다(hint 브랜치는 되감지 않는다) ──
            real_bap = evidence.binding_artifact_path
            evidence.binding_artifact_path = lambda *a, **kw: None
            try:
                ok, why = _raises(lambda: continue_(repo, draft=draft, generated_utc=_FX_CONTINUE_UTC, rt=rt),
                                  "HINT_CERTIFICATE_BINDING_ABSENT")
            finally:
                evidence.binding_artifact_path = real_bap
            ck(f"★계측 산출물 바인딩 없음 = 커밋 전 차단 · hint 브랜치 없음({why[:80]})", ok and branch.hint_tip(repo) is None
               and _step(_load_state(draft), "commit") is None)

            # ── ★커밋 직후 끊김(update-ref 뒤 · 상태 기록 전) → 새 시각 재실행도 같은 커밋으로 재개(중복 커밋 ✗) ──
            #   끊김은 branch.commit_payload **반환 직후**에 넣는다 — 커밋 시각을 커밋 전에 적지 않으면 재실행이 새 시각으로 두 번째
            #   커밋을 만든다(2026-09-22 리뷰 변이: 시각 기록을 커밋 뒤로 옮기면 이 검사가 붉어져야 한다).
            real_commit = branch.commit_payload

            def crash_after_commit(*a, **kw):
                real_commit(*a, **kw)
                raise RuntimeError("fixture: 커밋 직후 끊김")

            branch.commit_payload = crash_after_commit
            try:
                quiet(continue_, repo, draft=draft, generated_utc="2026-01-02T06:55:00Z", no_push=True, rt=rt)
                crashed = False
            except RuntimeError:
                crashed = True
            finally:
                branch.commit_payload = real_commit
            crash_tip = branch.hint_tip(repo)
            ck("커밋 직후 끊김 픽스처: 커밋은 됐고 상태엔 앵커가 없다(커밋 시각만 먼저 적혔다)", crashed and bool(crash_tip)
               and _step(_load_state(draft), "commit") is None and _load_state(draft).get("commit_utc") == "2026-01-02T06:55:00Z")

            # ── ★push 권위(D8) 없음 = 커밋·봉인 전에 차단 · ref·원격 불변(2026-09-22 통합 · 옛 hint_tag push 게이트 복원) ──
            central_p = repo / core.REL_CENTRAL_FLAG
            central_text = central_p.read_text(encoding="utf-8")
            central_p.unlink()
            refs_d8 = _refs(repo)
            remote_d8 = _fx_git(repo, "ls-remote", str(bare)).stdout
            st_d8 = _load_state(draft)
            try:
                ok, why = _raises(lambda: continue_(repo, draft=draft, generated_utc=_FX_CONTINUE_UTC, rt=rt),
                                  "HINT_PUSH_CENTRAL_AUTHORITY_ABSENT")
                ok_cli, why_cli = _raises(lambda: cmd_push(argparse.Namespace(repo=str(repo), tag=tagname, remote="origin",
                                                                              apply=True, draft=None)),
                                          "HINT_PUSH_CENTRAL_AUTHORITY_ABSENT")
            finally:
                central_p.write_text(central_text, encoding="utf-8")
            try:
                _require_central(td, "x")
                remedy = ""
            except core.HintError as e:
                remedy = e.remedy or ""
            ck(f"★권위 마커 없음 = continue(push) 가 커밋·봉인 전에 HINT_PUSH_CENTRAL_AUTHORITY_ABSENT({why[:70]})", ok
               and "--no-push" in remedy and core.REL_CENTRAL_FLAG in remedy)
            ck("★권위 마커 없음 = ref·원격·draft 상태 불변(커밋·봉인·push 0)", _refs(repo) == refs_d8
               and _fx_git(repo, "ls-remote", str(bare)).stdout == remote_d8 and _load_state(draft) == st_d8)
            ck(f"★권위 마커 없음 = 단독 push --apply 도 차단({why_cli[:60]})", ok_cli)

            # ── ★https 원격 + 토큰 없음 = push 에서 차단(봉인·검증까지는 끝난다) ──
            ok, why = _raises(lambda: continue_(repo, draft=draft, generated_utc=_FX_CONTINUE_UTC, remote="gh", rt=rt),
                              "HINT_PUSH_CREDENTIAL_ABSENT")
            ck(f"★https 원격 토큰 없음 = HINT_PUSH_CREDENTIAL_ABSENT({why[:160]})", ok)
            st = _load_state(draft)
            anchor = (_step(st, "commit") or {}).get("anchor")
            ck("커밋·봉인·로컬 검증은 이미 끝났다(원격엔 없다)", bool(anchor) and branch.hint_tip(repo) == anchor
               and (_step(st, "verify") or {}).get("ok") is True
               and tag.remote_tag_object(repo, "origin", tagname) is None)
            ck("페이로드 커밋 = 합성 신원", branch.commit_identity(repo, anchor).get("author", {}).get("ident")
               == branch.synthetic_identity())
            ck("★끊김 뒤 재실행 = 같은 커밋(첫 커밋 시각 재사용 · hint 브랜치 커밋 1개)", anchor == crash_tip
               and _fx_git(repo, "rev-list", "--count", "refs/heads/hint").stdout.strip() == "1")

            # ── ★커밋 뒤 draft 가 바뀌면 재실행이 막힌다(같은 이름으로 다른 내용 ✗) ──
            nar = payload / "02-narrative.md"
            nar_text = nar.read_text(encoding="utf-8")
            nar.write_text(nar_text + "\n커밋 뒤 덧붙인 줄\n", encoding="utf-8")
            refs_c = _refs(repo)
            ok, why = _raises(lambda: continue_(repo, draft=draft, generated_utc=_FX_CONTINUE_UTC, rt=rt),
                              "HINT_DRAFT_CHANGED_AFTER_COMMIT")
            ck(f"★커밋 뒤 draft 변경 = HINT_DRAFT_CHANGED_AFTER_COMMIT · ref 불변({why[:80]})", ok and _refs(repo) == refs_c)
            nar.write_text(nar_text, encoding="utf-8")

            # ── continue 재실행(origin) — 커밋 재개 · 봉인 재개 · push · 카탈로그 · 위상 ──
            try:
                cres = quiet(continue_, repo, draft=draft, generated_utc="2026-01-02T09:00:00Z", rt=rt)
            except core.HintError as e:
                ck(f"continue(origin) 성공 — {e.code}: {e.message[:600]}", False)
                return
            ck("재실행: 같은 페이로드 커밋(중복 커밋 없음 · 첫 커밋 시각 재사용)", cres["anchor"] == anchor
               and branch.hint_tip(repo) == anchor)
            local_obj = tag.read_tag(repo, tagname)["object_sha"]
            ck("원격 태그 오브젝트 = 로컬(AC9)", tag.remote_tag_object(repo, "origin", tagname) == local_obj == cres["remote_object"])
            remote_heads = _fx_git(repo, "ls-remote", "--heads", str(bare)).stdout
            ck("발행 push 는 브랜치를 밀지 않는다(O2)", "refs/heads/hint" not in remote_heads)
            ck("로컬 봉인 검증 PASS(verify · 서사 린트 포함)", quiet(verify_tag, repo, tagname) == [])
            idx = json.loads((repo / core.REL_INDEX).read_text(encoding="utf-8"))
            row = next((h for h in idx.get("hints", []) if h.get("tag") == tagname), None)
            ck("카탈로그 행 · grammar v6 · base_model", row is not None and row.get("grammar") == naming.GRAMMAR_V6
               and row.get("base_model") == "Org/Fixture-Model")
            rows = {h.get("tag"): h for h in idx.get("hints", [])}
            ck("★AC6 옛 태그·원격 전용·lightweight 가 공존해도 신규 발행 PASS · 셋 다 행으로 실림",
               all(legacy[k] in rows for k in ("old", "remote_only", "lightweight")))
            ck("AC6 원격 전용 = absent-local 행(합성 ✗)", rows.get(legacy["remote_only"], {}).get("object")
               == catalog.OBJECT_ABSENT_LOCAL)
            ph = repo / "campaigns/c1/phases/main/publish.status.json"
            phd = json.loads(ph.read_text(encoding="utf-8")) if ph.is_file() else {}
            ck("캠페인 publish 위상 = 원격 SHA 출처", phd.get("state") == "done"
               and (phd.get("proof") or {}).get("source") == f"refs/tags/{tagname}@{local_obj}")
            tree = branch.tree_paths(repo, anchor)
            ptree = json.loads(branch.read_blob(repo, anchor, "PAYLOAD.json"))
            ck("PAYLOAD.approval = 캠페인 사전승인 3키", ptree.get("approval", {}).get("source") == "campaign:hint_targets"
               and set(ptree["approval"]) == set(APPROVAL_KEYS))
            ck("PAYLOAD.factcheck = 통과(검증자 ≠ 저작자 · 열림 0) · 00 메타 '통과' · 저작 자기점검은 배포 ✗",
               (ptree.get("factcheck") or {}).get("status") == "passed" and (ptree["factcheck"].get("open") == [])
               and "통과 — 검증자 `hint-factcheck-agent`" in (branch.read_blob(repo, anchor, "00-hint.md") or b"").decode("utf-8")
               and all(b"SELFCHECK" not in (branch.read_blob(repo, anchor, n) or b"SELFCHECK") for n in template.PAYLOAD_DOCS))
            ck("PAYLOAD.bench_definition = evidence 가 읽은 docs.md 현행 full 정의(키 있음)", "bench_definition" in ptree)
            ok, why = _raises(lambda: refresh_draft(repo, draft), "HINT_DRAFT_ALREADY_COMMITTED")
            ck(f"★커밋 뒤 refresh = HINT_DRAFT_ALREADY_COMMITTED(커밋한 바이트와 갈라지지 않게)({why[:50]})", ok)
            ck("슬롯 사유(01 §1.2 소제목) 반영 · 신호 재계산", any(
                (r or {}).get("rationale") for r in (ptree.get("slots") or {}).values()))
            prov = json.loads(branch.read_blob(repo, anchor, "PROVENANCE.json"))
            ck("PROVENANCE 소스 앵커 = publish 때 HEAD", prov.get("source_anchor") == _fx_git(repo, "rev-parse", "HEAD").stdout.strip()
               and "README.md" in tree)
            body = tag.read_tag(repo, tagname)["body"]
            ck("annotation = brief + 포인터 + footer(zip 단일화 · D4)", tag.ANNOTATION_POINTER in body
               and tag.parse_annotation(body)["footer"]["anchor"] == anchor)
            ck("continue 재실행 = 멱등(이미 원격 · 같은 오브젝트)", quiet(continue_, repo, draft=draft, generated_utc=_FX_CONTINUE_UTC,
                                                               rt=rt)["remote_object"] == local_obj)

            # ── ★이름 충돌(로컬 → 원격) — 발행기 구동 전에 막힌다(부수효과 0) ──
            rec = repo / core.REL_EVIDENCE_DIR / f"{res['topic']}.json"
            rec_bytes = rec.read_bytes()
            ok, why = _raises(lambda: publish(repo, campaign="c1", cell="c1-a", generated_utc="2026-01-02T10:00:00Z",
                                              out=td / "again", rt=rt), "HINT_NAME_COLLISION")
            ck(f"★같은 셀 재발행 = 로컬 이름 충돌({why[:100]})", ok and rec.read_bytes() == rec_bytes)
            _fx_git(repo, "tag", "-d", tagname)
            ok, why = _raises(lambda: publish(repo, campaign="c1", cell="c1-a", generated_utc="2026-01-02T10:00:00Z",
                                              out=td / "again2", rt=rt), "HINT_NAME_COLLISION")
            ck(f"★로컬에 없고 원격에만 있어도 충돌(ls-remote)({why[:100]})", ok and "remote" in why)

            # ── ★continue 도 커밋 전에 이름 충돌을 다시 본다(재생 draft 는 같은 셀 = 같은 이름) ──
            if rok:
                tip0 = branch.hint_tip(repo)
                ok, why = _raises(lambda: continue_(repo, draft=rout, generated_utc="2026-01-02T11:00:00Z",
                                                    approved_by="사용자 발화 전사 — 픽스처", approved_utc="2026-01-02T11:00:00Z",
                                                    rt=rt), "HINT_NAME_COLLISION")
                ck(f"★continue 커밋 전 이름 충돌 재확인 = ref 쓰기 0({why[:90]})", ok and branch.hint_tip(repo) == tip0)

            # ── CLI 표면: 종료코드 · 게이트 JSON · catalog rc 4 ──
            def run_main(argv):
                o, e = io.StringIO(), io.StringIO()
                with contextlib.redirect_stdout(o), contextlib.redirect_stderr(e):
                    rc = main(["--repo", str(repo), *argv])
                return rc, o.getvalue(), e.getvalue()

            _fx_git(repo, "fetch", "-q", "origin", f"refs/tags/{tagname}:refs/tags/{tagname}")
            rc, o, _ = run_main(["verify", "--tag", tagname])
            ck(f"CLI verify rc 0(PASS){'' if rc == 0 else ' · ' + o[-300:]}", rc == 0 and "PASS" in o)
            rc, o, _ = run_main(["lint", "--draft", str(draft), "--json"])
            ck("CLI lint --json rc 0 · [] (봉인된 draft 도 통과)", rc == 0 and json.loads(o) == [])
            rc, o, e = run_main(["catalog", "derive", "--remote", "origin", "--generated-kst", "2026-01-02T20:00:00"])
            ck("CLI catalog derive 기본 모드 = 로컬 오브젝트 부재 rc 4 + 조회한 원격 fetch 안내",
               rc == catalog.EXIT_LOCAL_TAG_OBJECT_MISSING and "fetch" in e and "origin" in e)
            mp = repo / res["manifest"]
            mbytes = mp.read_bytes()
            mdoc = json.loads(mbytes)
            mdoc["task_class"] = "bogus_class"
            mp.write_text(json.dumps(mdoc), encoding="utf-8")
            rc, o, e = run_main(["verify", "--tag", tagname])
            try:
                gj = json.loads(o)
            except ValueError:
                gj = None
            ck(f"CLI 게이트 거부 = 게이트 JSON 원문 stdout + 게이트 종료코드(rc={rc})", rc != 0 and isinstance(gj, dict)
               and gj.get("allowed") is False and gj.get("action") == "hint_verify")
            mp.write_bytes(mbytes)
            gate_p = repo / core.REL_COMPLETION_GATE
            gate_link = os.readlink(gate_p) if gate_p.is_symlink() else None
            gate_p.unlink()
            gate_p.write_text("print('not json')\n", encoding="utf-8")
            try:
                rc, o, e = run_main(["verify", "--tag", tagname])
            finally:
                gate_p.unlink()
                if gate_link is not None:
                    os.symlink(gate_link, gate_p)
            try:
                bj = json.loads(o)
            except ValueError:
                bj = None
            ck(f"CLI 게이트 판독 불가 = 결정론 포장 JSON stdout(allowed:false) · rc 2(옛 계약)(rc={rc})", rc == 2
               and isinstance(bj, dict) and bj.get("allowed") is False and bj.get("action") == "hint_verify"
               and bj.get("reason_codes") == ["HINT_GATE_OUTPUT_UNREADABLE"])
            rc, o, e = run_main(["continue", "--draft", str(draft / "nope"), "--generated-utc", _FX_CONTINUE_UTC])
            ck("CLI HintError = render(메시지+remedy) stderr · rc 1", rc == 1 and "[hint] FAIL HINT_DRAFT_ABSENT" in e and "→" in e)

            # ── gitless match(배포 아카이브 모양 · git·PATH 없음) ──
            gl = td / "gitless"
            skill = gl / core.REL_SKILL
            shutil.copytree(core.SCRIPTS_DIR, skill / "scripts", ignore=shutil.ignore_patterns("__pycache__"))
            shutil.copytree(core.TEMPLATES_DIR, skill / "templates")
            (gl / "hints").mkdir(parents=True)
            shutil.copy2(repo / core.REL_INDEX, gl / core.REL_INDEX)
            r = subprocess.run([sys.executable, "-B", str(skill / "scripts/hint.py"), "match", "--vllm", "0.9.0", "--model",
                                "fixture-model-nvfp4", "--json"], cwd=str(gl), capture_output=True, text=True,
                               env={"PATH": "/nonexistent", "PYTHONDONTWRITEBYTECODE": "1"}, timeout=60)
            try:
                got = json.loads(r.stdout) if r.returncode == 0 else None
            except ValueError:
                got = None
            ck(f"gitless match(PATH 없음 · .git 없음) rc 0 · 새 태그 발견{'' if got else ' · ' + r.stderr[-300:]}",
               isinstance(got, list) and any(x.get("tag") == tagname for x in got))
    finally:
        for k, v in saved.items():
            if v is not None:
                os.environ[k] = v


def _selftest_replay_continue(ck) -> None:
    """캠페인 밖 발행(재생 draft → continue --approved-by) 전 경로 — 발행기 입력을 draft 안에 관측해 쓰고(publisher_inputs),
    옛 발행 기록의 승격 목표를 새 태그로 적고, 봉인·로컬 검증까지(--no-push). 별도 격리 저장소(이름이 캠페인 흐름과 겹치지 않게)."""
    # git 환경 변수도 비운다(E2E 와 같은 목록) — 훅·임시 인덱스 아래서 돌면 픽스처 저장소의 git 이 바깥 인덱스를 읽는다
    #   (2026-09-22 통합: GIT_INDEX_FILE 이 새어 픽스처 커밋이 "invalid object" 로 죽은 것을 재현).
    saved = {k: os.environ.pop(k, None) for k in ("GITHUB_TOKEN", "QUANT_MODEL_PATH", "GIT_DIR", "GIT_WORK_TREE",
                                                  "GIT_INDEX_FILE")}
    try:
        with tempfile.TemporaryDirectory(prefix="hint-e2e-replay-") as tds:
            td = Path(tds)
            repo, fx = _fx_repo(td)
            rt = Runtime(docker=_fx_docker(fx["docker"]))
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                cres = publish(repo, campaign="c1", cell="c1-a", generated_utc=_FX_PUBLISH_UTC, rt=rt)
                out = td / "replay"
                rres = publish(repo, publication=cres["topic"], replay=True, generated_utc=_FX_REPLAY_UTC, out=out, rt=rt)
            ok, why = _raises(lambda: continue_(repo, draft=out, generated_utc=_FX_CONTINUE_UTC, rt=rt), "HINT_APPROVAL_ABSENT")
            ck(f"★재생 continue 승인 전사 없음 = HINT_APPROVAL_ABSENT({why[:80]})", ok and branch.hint_tip(repo) is None)
            ok, why = _raises(lambda: continue_(repo, draft=out, generated_utc=_FX_CONTINUE_UTC, approved_by="<<FILL>>",
                                                approved_utc=_FX_CONTINUE_UTC, rt=rt), "HINT_APPROVAL_INVALID")
            ck("★자리표시 승인 = HINT_APPROVAL_INVALID", ok)
            _fx_author(repo, out, _load_state(out), factcheck=False)      # 사실 검증 보고 없이 — 사람 면제 경로를 밟는다
            try:
                with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                    res = continue_(repo, draft=out, generated_utc=_FX_CONTINUE_UTC, approved_by="사용자 발화 전사 — 재생 픽스처",
                                    approved_utc="2026-01-02T06:45:00Z", no_push=True, rt=rt,
                                    factcheck_waiver="사용자 발화 전사 — 재생 픽스처는 사실 검증을 면제한다")
            except core.HintError as e:
                ck(f"재생 continue(--no-push) 성공 — {e.code}: {e.message[:500]}", False)
                return
            ck("재생 continue: 봉인·로컬 검증까지 · push 없음", res["pushed"] is False and tag.tag_ref_kind(repo, rres["tag"]) == "tag"
               and tag.remote_tag_object(repo, "origin", rres["tag"]) is None)
            ck("재생 continue: 발행기 입력은 draft 안에 도구가 썼다(손 JSON 0)", all((out / "inputs" / f).is_file()
                                                                       for f in ("pii.json", "runtime.json")))
            man = json.loads((repo / rres["manifest"]).read_text(encoding="utf-8"))
            ck("재생 continue: 발행 기록의 승격 목표 = 새 태그 · 페이로드 커밋", (man.get("promotion_target") or {}).get("tag")
               == rres["tag"] and man["promotion_target"].get("anchor") == res["anchor"])
            ptree = json.loads(branch.read_blob(repo, res["anchor"], "PAYLOAD.json"))
            ck("재생 continue: PAYLOAD.approval = CLI 전사 3키", (ptree.get("approval") or {}).get("source") == "cli:--approved-by")
            fcw = ptree.get("factcheck") or {}
            ck("재생 continue: 사실 검증 사람 면제 = PAYLOAD · state · 00 메타에 '면제' 로 남는다(수신자가 안다)",
               fcw.get("status") == "waived" and fcw.get("reason_code") == "HINT_FACTCHECK_ABSENT"
               and (_step(_load_state(out), "factcheck") or {}).get("status") == "waived"
               and "**사람 면제**" in (branch.read_blob(repo, res["anchor"], "00-hint.md") or b"").decode("utf-8"))
    finally:
        for k, v in saved.items():
            if v is not None:
                os.environ[k] = v


def _branch_readme_drift(text: str) -> list[str]:
    """정적 hint 브랜치 README(`templates/hint-branch-README.md` · S7 형식 전환 커밋의 원천)가 배너 상수·형식 문자열과 갈라졌는가.
    정적 파일 둘은 한쪽이 다른 쪽을 생성할 수 없다 — 단일 소유가 불가능하면 교차검증이 차선이다(workflow.md §결정론 규율 ·
    2026-09-22 통합: 문서 담당이 "README 가 상수를 복제한다" 고 알렸다). 반환 = 빠진 조각 목록."""
    need = [*template.BANNER_LINES, branch.PAYLOAD_FORMAT]
    return [x for x in need if x not in text]


def _selftest_static_docs(ck) -> None:
    rd = core.TEMPLATES_DIR / "hint-branch-README.md"
    text = rd.read_text(encoding="utf-8") if rd.is_file() else ""
    miss = _branch_readme_drift(text)
    ck(f"hint 브랜치 README = 배너 {len(template.BANNER_LINES)}줄 + 형식 `{branch.PAYLOAD_FORMAT}` 글자 그대로(빠짐 {miss[:2]})",
       bool(text) and not miss)
    ck("★README 에서 배너 한 줄을 지우면 교차검증이 잡는다",
       bool(text) and _branch_readme_drift(text.replace(template.BANNER_LINES[0], "")) == [template.BANNER_LINES[0]])


def self_test() -> int:
    bad: list[str] = []
    ran = [0]

    def ck(name: str, cond) -> None:
        ran[0] += 1
        print(f"  {'ok  ' if cond else 'FAIL'} {name}")
        if not cond:
            bad.append(name)

    # 바깥 git 환경(훅·임시 인덱스 · GIT_DIR)이 픽스처 저장소로 새지 않게 **전 구간**에서 비운다 — 모듈 자체검사(catalog·lineage·
    #   artifacts)도 임시 저장소에 git 을 부른다(2026-09-22 통합: GIT_INDEX_FILE 누수 재현 시 셋이 붉어졌다).
    git_env = {k: os.environ.pop(k, None) for k in _GIT_ENV_LEAKS}
    try:
        _self_test_body(ck, bad, ran)
    finally:
        for k, v in git_env.items():
            if v is not None:
                os.environ[k] = v
    print(f"{_OUT} self-test {'PASS' if not bad else f'FAIL({len(bad)})'}")
    return 0 if not bad else 1


_GIT_ENV_LEAKS = ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES",
                  "GIT_COMMON_DIR", "GIT_NAMESPACE", "GIT_PREFIX")


def _self_test_body(ck, bad: list[str], ran: list[int]) -> None:
    print(f"{_OUT} self-test — hintlib 모듈 자체검사")
    bad += _selftest_modules()
    print(f"{_OUT} self-test — CLI 파서")
    _selftest_parser(ck)
    print(f"{_OUT} self-test — 정적 문서 교차검증")
    _selftest_static_docs(ck)
    print(f"{_OUT} self-test — 격리 E2E(임시 저장소 · bare 원격 · 가짜 docker · 라이브 비의존)")
    try:
        _selftest_e2e(ck)
        _selftest_replay_continue(ck)
    except Exception as e:  # noqa: BLE001 — 픽스처 사고도 실패로 센다
        import traceback
        traceback.print_exc()
        ck(f"E2E 픽스처 사고: {type(e).__name__}: {e}", False)
    # 2026-09-22 S2 round 2: 하한 = 새 E2E 음성대조 포함(114 실측) — 사실 검증·refresh·§L 묶음이 조용히 빠지면 붉어진다.
    # 2026-09-22 S2 round 3: 탐침 기본 · excerpt --numbered · 새 사실 키 묶음(130 실측) → 하한 126.
    # 2026-09-22 S2 round 3 적대 리뷰: 탐침 규칙 단일 소유 · 저작 안내 · 스냅샷 origin(실재 커밋 · 변조) 묶음(137 실측) → 하한 134.
    ck(f"검사 전수 실행({ran[0]}) — 중도 반환으로 시험이 조용히 줄지 않게", ran[0] >= 134)


if __name__ == "__main__":
    sys.exit(main())
