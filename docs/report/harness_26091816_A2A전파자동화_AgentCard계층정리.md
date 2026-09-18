# A2A 전파 자동화 및 Agent Card 계층 정리

> 대상: 이 저장소를 CI/CD 및 노드 프로비저닝에 사용하는 배포자  
> 상태: `single-node`·`multi-node` 운영 브랜치 반영 및 검증 완료  
> 발행 시점: 2026-09-18 16 KST

## 변경 요약

메인→서브 전파를 매 실행마다 새 사람 승인을 요구하는 단발 작업에서, **최초 승인된 동일 범위 안의 비파괴 reconciliation**으로 전환했습니다. 동시에 Agent Card에 중복으로 얹혀 있던 애플리케이션 서명 정체성 계층을 제거하고, A2A 본래 역할인 capability/discovery metadata로 정리했습니다.

## 최종 권한 계층

| 계층 | 권위 | 역할 |
|---|---|---|
| Endpoint authentication | OpenSSH public key + `known_hosts` | 승인된 원격 endpoint인지 확인 |
| Node facts/readiness | Terraforming manifest + Flag | topology, role/rank, HW, model source, sub issuer 확인 |
| Capability discovery | unsigned Agent Card | interface, skills, sub mode, tool/delivery plane 설명 |
| Action authorization | 승인 plan + approval atoms + propagation scope | 어떤 대상·경로·plane에 어떤 작업을 허용했는지 제한 |
| Attribution | Git identity + HITL record | 변경 주체와 승인 근거 기록 |

Agent Card는 endpoint credential이나 execution token이 아닙니다. Card의 `signatures` 필드는 현재 배포 계약에서 허용되지 않습니다.

## 자동 전파가 허용되는 조건

다음 조건이 모두 같은 경우 후속 전파는 추가 HITL 없이 AUTO로 진행할 수 있습니다.

- 기존에 승인된 topology·node ID·SSH endpoint·work directory
- 요청 plane이 승인 plane과 정확히 일치
- 원격 checkout이 이미 대상 topology branch
- 신규 경로 생성 없음
- 삭제·retirement·tombstone 없음
- strict host-key verification 성공
- multi의 deletion preview가 0

Topology별 전파 범위:

- `single`: agent environment overlay만 전파, build kit은 dormant
- `multi`: Band2 build/runtime + overlay, model-specific Band3는 제외

다음은 계속 새 승인 또는 fail-closed 대상입니다.

- endpoint·work directory·topology·plane 변경
- branch transition
- bootstrap/provision
- 파일 삭제·retirement
- host-key 부재 또는 변경
- 승인 plan과 실제 destination 불일치

## CI/CD 영향

### 제거된 항목

- Agent Card JWS/Ed25519 key generation 및 sign/verify
- trusted-key store
- `prove-identity` runtime command
- sub runtime Card verifier 배달
- delegation file 또는 environment variable을 통한 readiness 면제
- Card signature 기반 topology/readiness 판정

### 유지된 항목

- unsigned Agent Card 렌더 및 구조 검증
- manifest/Flag fail-closed readiness
- sub manifest의 main issuer 확인
- topology parity
- Git-index 기반 transactional source
- 원격 rollback transaction
- single/multi specialized constitution 분리
- sub-local Git commit과 docs-only 상향 회수

## 검증 결과

| 검증 | 결과 |
|---|---|
| Runtime harness self-test | PASS |
| Distribution verifier | PASS |
| Policy predicates | 73/73 PASS |
| Policy registry | PASS |
| Topology parity | single/multi PASS |
| Terraform scan self-test | 55/55 PASS |
| Manifest contract self-test | 27/27 PASS |
| Agent Card metadata self-test | PASS |
| Sub renderer self-test | PASS |
| Node-role contract | 30/30 PASS |
| Benchmark topology resolver | PASS |
| Recipe regression | PASS |
| Shell/Python syntax 및 diff check | PASS |

## Live sub 검증

### Single topology

- overlay 전파: 166/166 일치
- build-kit 전파: topology 계약에 따라 생략
- unsigned Agent Card: PASS
- main-issued sub manifest readiness: PASS
- retired signed-Card runtime 파일 부재: PASS
- sub mode: A2A agent
- tool plane: runtime skills 3종
- delivery plane: dormant

### Multi topology

- Band2 checksum: PASS
- overlay 전파: 53/53 일치
- deletion: 0
- unsigned Agent Card: PASS
- main-issued sub manifest readiness: PASS
- retired signed-Card runtime 파일 부재: PASS
- sub mode: Ray worker
- tool plane: empty
- delivery plane: active

모델 다운로드, 이미지 build, serve, benchmark는 이번 제어평면 안정화에서 실행하지 않았습니다. 기존 topology별 E2E 판정과 독립된 변경입니다.

## 배포자 조치

1. 운영 브랜치 중 배포 topology와 일치하는 브랜치를 사용합니다.
2. Terraforming manifest와 Flag를 먼저 확정합니다.
3. 최초 sub propagation scope는 승인 plan에 endpoint·work directory·plane을 명시해 결속합니다.
4. 이후 동일 범위의 비파괴 전파는 자동 reconciliation을 사용합니다.
5. Agent Card signature/trust-store를 복구하거나 새 credential 계층으로 사용하지 마십시오.

## 외부 근거

- A2A Protocol v1.0.0: https://a2a-protocol.org/v1.0.0/specification/
- A2A Agent Discovery: https://a2a-protocol.org/latest/documentation/agent-discovery/
- GoogleCloudPlatform Knowledge Catalog: https://github.com/GoogleCloudPlatform/knowledge-catalog

A2A 명세가 이 저장소의 AUTO 정책을 직접 규정하는 것은 아닙니다. 명세의 capability discovery, transport authentication, authorization 분리, 비동기 task/artifact 모델을 바탕으로 이 저장소의 승인 범위와 reconciliation 정책을 설계했습니다.
