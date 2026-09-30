# 블랙박스 — 호스트 안전체계 Y/N · L3 수행지시서 · 자산 표

> terraforming_node 스킬 reference — SKILL.md §2.6 · §2.6.1 · 옛 §4 호스트 안전체계 의 본문이다. SKILL.md(라우터)는 § 번호·제목·포인터만 든다.
> 이관 전 원문: `git show dcb713a:.claude/skills/terraforming_node/SKILL.md` (plan_26093022 Step 4 — 본문 바이트는 그대로 옮겼다).

## 목차

- 2.6 호스트 안전체계 — 세션 최종 선택조항 (Y/N · 양 토폴로지 공통 · plan_26071115)
- 2.6.1 노드블랙박스 설치 — `docs/request/` 수행지시서로 위임 (2026-08-18 신설 · `plan_26081716`)
- 호스트 안전체계 자산 (옛 §4 에서 이관)

## 2.6 호스트 안전체계 — 세션 최종 선택조항 (Y/N · 양 토폴로지 공통 · plan_26071115)

> **양 토폴로지 공통 최종 스텝**: single 은 §1S manifest+Flag 기입 직후 · multi 는 §2.5 카나리 통과 직후 이 절로 온다.
> **Flag 발급 이후**에 오는 **독립 Y/N 선택조항**이다 — 표준 온보딩(스캔·게이트·manifest·Flag)은 이미 완수됐고, 안전체계는 **선택**이다(설치 안 해도 Flag valid). 헌법 §호스트 안전체계 따름정리(선택화).

- **성격**: 절차적 필수 스텝이 아니라 **선택(Y/N)**이다 — 설치를 강제하지 않고, 혜택을 서술해 권하되 거부는 존중한다.
- **① 혜택 서술(talking point 예시 — 실문구는 재량)**:
  - "768k prefill 사건에서 워치독이 컨테이너를 먼저 정리해 호스트를 지킨 실적이 있습니다 — 설치하면 이 보호막이 상시 작동합니다."
  - "설치하면 **하드다운의 블랙박스**가 남습니다 — 하드다운은 디스크에 로그를 쓸 시간조차 주지 않고 끝나는데, efi_pstore(단일)·netconsole(멀티)이 그 순간의 커널 메시지를 노드 **밖**·**전원 밖**에 남깁니다. 안 남기면 원인 추적이 원천 불가입니다."
  - "1초 샘플 시계열과 포락선이 쌓여 **로드 ETA 를 예측**하게 됩니다 — 정상 로드를 사고로 오인해 죽이는 일이 줄어듭니다."
  - (통합메모리 GPU 한정) "이 GPU 는 시스템 메모리를 공유해 GPU OOM 이 곧 호스트 다운입니다 — 워치독이 그 직전에 개입합니다."
- **② Y 분기(설치)**: 승인 시 **사용자 실행** — 설치자는 **노드블랙박스**다(아래 ⚠ 승계 참고):

  ```bash
  sudo bash .claude/skills/terraforming_node/scripts/node_blackbox/install_node_blackbox.sh --level L3           # dry-run(기본)
  sudo bash .claude/skills/terraforming_node/scripts/node_blackbox/install_node_blackbox.sh --apply --level=L3   # 적용
  ```

  → **에이전트 무인 sudo 실행 ✗**(실행 주체는 사람). dry-run 선행으로 무엇을·왜·트레이드오프를 고지한다.
  **L3 를 포함하면 채팅이 아니라 `docs/request/` 수행지시서로 위임한다(§2.6.1).**
  - **레벨**(재부팅 필요 여부가 자연 경계 · 배치와 활성화는 분리 — 파일·유닛은 레벨 무관하게 항상 도착):
    - `L1` **무재부팅** — 수집기(1초 샘플) · **ETA 워치독**(RAM 축) · **열·전력 워치독**(`thermal_watchdog.sh` + 상수 emit `blackbox_thermal.py` → 유닛 `easy-vllm-blackbox-thermal` · 누설 버킷 지속성 판정 · hard-lockup 예방) · 이벤트 통합 · 로그 수명 집행 · sudoers 단일 헬퍼(`vllm-drop-caches` 경로 1개만 NOPASSWD) · **earlyoom**(프로세스-레벨 최후선 — 빌드 평면까지 커버)
    - `L2` **무재부팅·peer 필요** — netconsole 교차 스트리밍(**multi 전용** · single 은 `N/A` 로 정직 기록)
    - `L3` **재부팅 1회** — 사후 포착 = **efi_pstore** 확보. **crashkernel(2.25 GiB 예약)·ramoops 를 설정하는 게 아니라 제거한다.**
  - **검증(결정론)**: `bash .claude/skills/terraforming_node/scripts/node_blackbox/verify_node_blackbox.sh --check`.
    **상태 권위는 `installed: true` 가 아니라 `docs/logs/<node_id>/capture_verified.json` 이다** — "ready" 는 주장이지 증거가 아니다(2026-07-30 하드다운 2회에서 kdump 는 양노드 `ready to kdump` 였는데 vmcore 0건).
  - → manifest `host_safety.installed: true`.

  > ⚠ **승계(2026-08-18 개정 · `plan_26073109`)** — 이 절은 예전에 `host_safety/install_host_safety.sh
  > --apply [--with-kdump]` 를 지시했다. 그것이 만들던 것(mem_watchdog·earlyoom·kdump·netconsole·임시
  > telemetry)은 **노드블랙박스 단일 패키지로 승격**됐다: `purge_host_safety.sh` 가 구 설치물을 잔재
  > 없이 걷어낸 뒤 `install_node_blackbox.sh` 가 설치한다.
  >
  > **특히 `--with-kdump` 는 이제 권하지 않는다 — 해로운 것으로 판정됐다.** 강제 크래시 5회
  > (`testlog_26073113`)가 뒤집은 결과: ① vmcore 0/4(makedumpfile 이 커널 6.17 미지원 — 구조적)
  > ② **kdump 가 무장돼 있으면 pstore 가 원천 차단된다**(`crash_kexec_post_notifiers=N` 이라 `panic()`
  > 이 `kmsg_dump` 보다 먼저 kexec 로 점프해 돌아오지 않는다) ③ 그 대가로 2.25 GiB 를 **상시** 예약한다.
  > 즉 kdump 는 *유일하게 작동하는 사후 포착 수단을 죽이면서* 메모리를 먹었다. ramoops 도 이 플랫폼에선
  > 불가(정상 재부팅만으로 헤더가 깨진다 = 펌웨어가 리셋 때 DRAM 을 초기화). **efi_pstore 는 kdump 를
  > 내린 상태에서 패닉 19 레코드를 포착했다(1/1).**
  >
  > **이 절의 Y/N 선택조항 성격·무인 sudo 금지는 그대로 유효하다** — 설치 대상만 바뀌었다.
  > 레거시 설치물이 남아 있는 노드는 `purge_host_safety.sh --require-seed` 를 **먼저** 돌린다(저널 수확
  > 선행 게이트 — mem_watchdog 저널은 포락선의 유일한 초기 데이터이고 유닛 제거 후 vacuum 되면 복구 불가).
- **③ N 분기(미설치·opt-out)**: 서빙은 정상 진행. manifest `host_safety.installed: false` 로 기록한다. 재권유는 **세션당 1회 이하**.
  - **통합메모리 노드 한정 후속 경고**: opt-out + 통합메모리(GPU OOM=호스트 하드다운 위험) 노드는, 이후 서빙 기동 직전 **에이전트 채팅창 1줄** 안내만 한다("워치독 미설치 상태 — 통합메모리라 OOM 시 호스트 다운 위험, `install_node_blackbox.sh` 로 언제든 보강 가능"). **serve 스크립트/로그 배너 코드변경 ✗**(시끄러운 경험 방지 — D31·NG-5). **discrete GPU 노드는 무경고.**
- **④ 파급 정밀화(opt-out 이어도 보호 일부 유지)**: 하네스 **협역 워치독**(`run_trial`·`multinode_serve_smoke.sh` 자동 기동)은 레포 내장 스크립트라 **설치와 무관하게 계속 작동**(opt-out 사용자도 trial 중 보호 유지). 로드-전 RAM 게이트(⑤.5)의 `vllm-drop-caches` 자동 드랍만 헬퍼 부재로 skip 되며, 게이트는 이를 **음성정직으로 보고**(드랍 없이 재측정 → 부족 시 기동 거부 exit 7 유지 — `preload_ram_gate.try_drop_caches` 기구현 graceful).
- **⑤ 멀티노드 변형**: 양노드(메인+서브) 각각 동일 Y/N. **서브 설치는 렌더 배달분**(`.claude/runtime/node_blackbox/install_node_blackbox.sh` — `render_sub_env.py` §4.6 이 `node_identity.sh` 를 포함해 배달한다)으로 **서브에서 사용자가 실행**(A2A 경계 — 메인 sudo 대행 ✗). manifest `nodes[].host_safety.installed` 로 **노드별 독립** 기록. **L2(netconsole)는 멀티에서만 성립**하므로 양노드 peer 지정이 필요하다.

### 2.6.1 노드블랙박스 설치 — `docs/request/` 수행지시서로 위임 (2026-08-18 신설 · `plan_26081716`)

> §2.6 Y 분기의 **레벨 정의·설치 명령은 그 절이 소유**한다. 이 절은 그중 **L3(재부팅 포함)** 를
> 채팅이 아니라 문서로 위임하는 배선만 다룬다.

- **① 발행 스텝(L3 포함 시)**: 사용자가 Y 로 답하고 그 범위에 **L3 가 포함되면**, 채팅으로 명령을
  나열하는 대신 **수행지시서를 발행**한다:

  ```bash
  python3 .claude/skills/terraforming_node/scripts/node_blackbox/publish_install_request.py \
      --generated-utc <YYYY-MM-DDTHH:MM:SSZ> --level L3   # --topology 는 manifest 에서 읽는다
  ```

  → `docs/request/request_<YYMMDDHH>_노드블랙박스_L3_설치_수행절차.md`(명명은 `doc_naming` 결정론).
  **시각은 주입 전용이다**(벽시계 금지 — `staleness_gate.py --max-age-days` 선례). 발행기는 설치자·
  검증기의 인터페이스를 실물 대조하고 어긋나면 **fail-closed**(exit 3)로 죽는다 — 낡은 지시서를
  내지 않는다.
- **왜 문서인가**: L3 는 재부팅을 요구하는데 **에이전트는 그 호스트 위에서 돈다.** 재부팅과 함께
  세션이 소멸하고 컨텍스트(어디까지 했는지·무엇을 검증할지·회수물이 무엇인지)가 함께 사라진다.
  **문서는 디스크에 남아 재부팅을 생존한다.** 멀티노드면 사람이 두 노드를 오가야 하므로 이득이 두 배다.
  지시서는 **진행 체크박스**를 자기 안에 담아 *상태*까지 생존시킨다.
- **② 발행하지 않는 경우**: `L1`·`L2` 만이면 발행하지 않는다(발행기가 exit 1 로 거절 —
  `--force` 로만 우회). 재부팅이 없으면 세션이 죽지 않아 문서의 존재 이유가 약하고, §2.6 의
  선택(Y/N) 성격과도 맞다. 채팅 안내로 족하다.
- **③ 회수 스텝**: 사람이 "완료"를 알리면 에이전트가 **정해진 경로에서 직접 읽는다**(채팅 붙여넣기
  요구 ✗ — 재부팅 생존 목적과 정합):

  | # | 회수물 | 경로 |
  |---|---|---|
  | 1 | 검증 출력 | `verify_node_blackbox.sh --check` 재실행으로 재현 가능 |
  | 2 | proof-of-capture 판정 | `docs/logs/<node_id>/capture_verified.json` |

  `<node_id>` 해소는 `node_identity.sh` 단일 소유(§2.7.6 — 각자 파싱 금지). 멀티노드면 **노드마다**
  생성된다. **`installed: true` 가 아니라 `capture_verified` 가 상태 권위**다(주장 ≠ 증거).
- **④ evidence chain 밖이다**: request 는 판정도 계측도 아니라 `completion_gate` 를 타지 않는다
  (`.claude/rules/docs.md` §request). 회수물이 돌아오면 그때 testlog/devlog 로 chain 에 편입한다.
- **⑤ 무인 sudo 금지는 그대로다**: 발행은 에이전트가, **실행은 사람이** 한다. 멀티노드에서 메인이
  서브의 `sudo` 를 대행하지 않는 경계(§2.6 ⑤)도 유지된다 — 지시서가 양노드 절차를 한 문서에 담을 뿐
  실행 주체를 바꾸지 않는다.


## 호스트 안전체계 자산 (옛 §4 에서 이관)

- **호스트 안전체계 — 설치자는 승계됐고 협역 워치독은 정본으로 남았다**(2026-08-18 정밀화). 두 디렉터리를 뭉뚱그리면 살아 있는 자산을 레거시로 오인해 **보호층을 걷어내게 된다.** 두 뿌리는 `.claude/skills/terraforming_node/scripts/node_blackbox/` 와 `.claude/skills/terraforming_node/scripts/host_safety/` 이며, 레포 루트 `scripts/` 가 아니다(자기완결 재배치 완료 — 그 경로는 tombstone).

  | 자산 | 상태 | 소유·호출자 |
  |---|---|---|
  | `node_blackbox/install_node_blackbox.sh` · `verify_node_blackbox.sh` | **현행 설치자·검증기**(L1/L2/L3) | §2.6 Y 분기 · HITL sudo |
  | `node_blackbox/publish_install_request.py` | **현행** — L3 지시서 발행 | §2.6.1 |
  | `node_blackbox/mem_watchdog_eta.sh`(RAM) · `thermal_watchdog.sh`(열·전력) · `blackbox_*.py` · `logs_lifecycle.py` · `regen_envelope.py` · `node_identity.sh` | **현행** — systemd 상시층·데이터 평면 | 설치자가 배치 |
  | `node_blackbox/agent_guard.py` | **현행** — 3단 응답의 1단(에이전트 예방 트리거 · 데몬 SIGKILL 보다 앞서 개입) | 서빙 중 에이전트 |
  | `node_blackbox/budget_renew_loop.sh` | **현행** — 상주 서빙의 예산 선언 갱신 사이드카 | `single_serve_up.sh` · `multinode_serve_smoke.sh` |
  | `node_blackbox/adversarial_stress.py` | **시험용 가압기** — 방어층 발동 시험(벤치마커 ✗) | 사람 지시 시험 |
  | `node_blackbox/seed_from_journal.py` | **Phase 0 수확기** — 레거시 저널 → `docs/logs/<node>/seed/` | 안전체계 제거 전 1회 |
  | `host_safety/mem_watchdog.sh` | ★ **정본으로 유지** — **협역(harness-scoped) 워치독**. `policy:HOST_SAFETY_LAYERED_DEFENSE.C1` 이 요구하는 계층이며, `multinode_serve_smoke.sh:274-276` 이 이를 *canonical* 로 부르고 **부재 시 exit 2** 로 죽는다 | `run_trial` · `multinode_serve_smoke.sh` 가 자동 기동 |
  | `host_safety/install_host_safety.sh` · `install_netconsole.sh` · `systemd/` · `host/` | **승계됨**(`plan_26073109`) | 잔재는 `node_blackbox/purge_host_safety.sh --require-seed` 로 제거 |

  - 서브 materialization: 블랙박스 = `.claude/runtime/node_blackbox/` · 협역 워치독 = `.claude/runtime/host_safety/mem_watchdog.sh`(둘 다 `render_sub_env.py` 가 배달).
  - ⚠ **`--keep-up` 상주 서빙 중에는 협역 워치독이 `PPID=1` 로 떠 있는 것이 정상이다** — 스모크 스크립트가 워치독만 남기고 종료하는 설계다. **`docker ps` 로 그 필터에 매칭되는 실행 중 컨테이너가 있으면 좀비가 아니다.** 정리는 `--down`(정식 teardown 5단계) 또는 `reap_stale_watchdogs` 가 하며, **손으로 `kill` 하면 살아 있는 서빙의 보호층이 사라진다.**
  - 근거 = `plan_26071019`(계층 방어) · `plan_26073109`(블랙박스 승격) · `testlog_26073113`(kdump 반증).
