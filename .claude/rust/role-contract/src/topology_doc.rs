//! Generated source-compatibility metadata: the module docstring of `topology_parity.py` (the owner CLI prints it as the
//! argparse description). Generator: tools/gen_topology_doc.py reading the canonical file as DATA (nothing executed).
//! Original SHA-256 of the file it was read from: b8b20e90c03219f05d47479b0c48ff2db7546f6128a056423285fa9b1e06741a

pub const DESCRIPTION: &str = r##"topology_parity.sh -- 브랜치 ↔ 특화헌법 ↔ manifest ↔ 활성캠페인 **4자일치** 술어 (단일 소유)

사용법:
  bash   .claude/policies/runtime/topology_parity.sh evaluate       --repo . [--expect single|multi] [--format json|value]

왜 이 파일이 있나
-----------------
헌법은 "토폴로지는 manifest 가 권위이고 브랜치로 추론하지 않는다"고 선언해 왔지만 **그 선언을
집행하는 실행자가 0** 이었다(`docs/report/audit_26091123` §3: 반대 방향으로 파생하는 코드는 13곳).
그래서 2026-09-11 에 멀티 캠페인 14셀이 `single-node` 체크아웃에서 돌았고, 울린 트립와이어는 사람뿐이었다.

이 술어는 브랜치를 **권위가 아니라 필터**로 되돌린다 -- 브랜치는 "어느 헌법 특화층과 어느
`output/<t>/` 를 읽는가"를 고를 뿐이고, 그 선택이 나머지 셋과 어긋나면 fail-closed 로 막는다.
정책 `BRANCH_CONSTITUTION_LAYERING` C5.

**자동 교정은 하지 않는다.** 어긋남의 해소가 체크아웃 전환인지 선언 수정인지는 사람이 정한다 --
자동으로 한쪽에 맞추면 그 순간 권위가 다시 뒤집힌다.

네 다리
-------
| 다리 | 입력 | 성격 |
|---|---|---|
| branch   | `git symbolic-ref --short HEAD` | 필터(통로 선택기) |
| layer    | `git ls-files -- '*.topology.md'` 각 파일 머리의 `**topology: …**` 줄 | 특화헌법 자기선언 |
| manifest | `output/<t>/manifest.yaml` 의 `topology` | **토폴로지 사실의 권위** |
| campaign | 활성 `campaigns/<ACTIVE>/campaign.yaml` 의 `control_variables.topology` + `nodes[].topology` | 실행 통제변인 |

**부재와 불일치를 가른다**(2026-09-12 · plan_26091210 A1). 이 술어가 보는 입력 중 `manifest` 와
`campaign` 은 **비추적**이라 fresh clone 과 `git worktree` 체크아웃에는 아예 없다. 부재를 위반으로
세면 미테라포밍 클론이 커밋조차 못 하고 종료 시퀀스가 자기 커밋을 막는다. 절단선은 이렇다:

> **존재는 `policy:TERRAFORM_FLAG_GATE` 의 몫이고, 일치는 이 정책의 몫이다.**
> 추적 입력(특화 파일)의 부재는 RED. 비추적 입력의 부재는 `absent` 로 **표시**하되 위반이 아니다.
> 있는데 어긋나면 RED.

표시는 침묵이 아니다 -- 출력이 몇 개의 다리로 판정했는지(`legs_checked`)를 항상 밝히므로,
3자로 판정한 것이 4자로 판정한 것처럼 읽히지 않는다.

종료코드: 0 = PASS · 2 = usage · 5 = 계약 위반(RED). `node_role_contract.sh` 규약과 같다.
stdout 은 JSON(또는 `--format value` 의 한 줄) 전용이고 진단은 stderr 다 -- 셸 소비자가 stdout 을 파싱한다.
"##;
